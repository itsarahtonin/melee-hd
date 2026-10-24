//! The language options (`melee/mn/mnlanguage.c`): once the screen is in, left and right
//! choose English or Japanese; A on the other language saves it, reloads the announcer's
//! voices for it and starts the menu again in it, on the Options list; B leaves for the
//! Options list.

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_A, IN_BACK};
use super::options::{
    EXIT_TO, JOBJ_MASK, LANGUAGE, MOBJ_MASK, OptionMenu, SETTINGS, SFX_BACK, SFX_MOVE, al,
};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;

/// The processes: the input, the screen going out, idle and coming in.
pub const INPUT: u32 = 0x8024_BFE0;
pub const OUT: u32 = 0x8024_C210;
pub const IDLE: u32 = 0x8024_C270;
pub const IN: u32 = 0x8024_C2E8;

/// `lang_jobj_frames`: the switch's frame for each language (`LANG_JP`, `LANG_US`).
const LANG_FRAMES: [f32; 2] = [1.0, 0.0];
/// `mnLanguage_803EF5A0`, `mnLanguage_803EF5AC`: the screen coming in and going out.
const COME_IN: AnimLoop = al(0.0, 19.0, -0.1);
const GO_OUT: AnimLoop = al(20.0, 29.0, -0.1);

/// `mnlanguage.c`'s statics: the model (`model_desc`, at 0x804A08D8) and the screen's GObj
/// (`mn_gobj`, at 0x804D6C50).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Language {
    pub model: super::super::gm::title::ModelDesc,
    pub gobj: Option<GObjId>,
}

impl Running<'_> {
    fn language_gobj(&self) -> GObjId {
        self.game
            .options
            .language
            .gobj
            .expect("the language options")
    }

    /// The switch on the cursor's language.
    fn language_show(&mut self, lang: u8) {
        let j = self.options_jobj(self.language_gobj());
        let s = self.options_node(Some(j), 1);
        self.options_set_frame_stopped(s, LANG_FRAMES[usize::from(lang)], JOBJ_MASK);
    }

    /// `mnLanguage_8024BFE0`, the screen's input: B leaves; A on the other language changes
    /// to it; once the screen is in, left or right switch the cursor.
    #[doc(alias = "mnLanguage_8024BFE0")]
    pub fn language_input(&mut self, _gobj: GObjId) {
        let g = self.language_gobj();
        let m = self.option_menu(g).1;
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(SETTINGS, 4, EXIT_TO);
            return;
        }
        if ev & IN_A != 0 {
            let m = self.option_menu(g).1;
            if m.unk1 != Some(m.cursor) {
                self.language_change(m.cursor);
            }
        } else if m.unk2.expect("the language screen's state") != 0
            && ev & (super::main::IN_LEFT | super::main::IN_RIGHT) != 0
        {
            self.lb_audio_call(SFX_MOVE);
            let m = self.option_menu_mut(g);
            m.cursor = u8::from(m.cursor == 0);
            let lang = m.cursor;
            self.language_show(lang);
        }
    }

    /// The saved language changed to `lang`: the save written, the announcer's voices
    /// loaded for it (`lbAudioAx_80027AB0`, the change's sound), and the menu's mode ended
    /// for the menu again (`mn_80229860`), every process of p-links 1 to 9 held for
    /// the rest of the tick (`mn_8022F1A8`), the flow on the Options list's language.
    fn language_change(&mut self, lang: u8) {
        self.game.game_data.set_saved_language(lang);
        self.game.sync_unlocks();
        self.card_game_save_changes();
        self.lb_audio_language(0xAD);
        self.menu_restart_mode(super::super::gm::machine::GM_MENU);
        self.mark_links_ran(1, 9);
        let f = &mut self.game.menu.flow;
        f.cur_menu = SETTINGS;
        f.hovered_selection = 4;
    }

    /// `mn_80229860`: the scene ends for mode `mode`.
    #[doc(alias = "mn_80229860")]
    pub(crate) fn menu_restart_mode(&mut self, mode: u8) {
        self.game.menu_mode.pending_mode = mode as i8;
        self.game.end_scene();
    }

    /// `mn_8022F1A8`: every process of p-links `from` to `to` marked as run this pass
    /// (`HSD_GObj_80390CD4`), so none runs again before the scene ends.
    #[doc(alias = "mn_8022F1A8")]
    pub(crate) fn mark_links_ran(&mut self, from: u8, to: u8) {
        for link in from..=to {
            let mut cur = self.game.world.first_in(link);
            while let Some(g) = cur {
                let next = self.game.world.gobj(g).next;
                self.game.world.mark_ran(g);
                cur = next;
            }
        }
    }

    /// `fn_8024C210`: the screen going out, then gone.
    #[doc(alias = "fn_8024C210")]
    pub fn language_out(&mut self, gobj: GObjId) {
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, GO_OUT, MOBJ_MASK) >= crate::fpu::reg(GO_OUT.end) {
            gobj::free(self, gobj);
        }
    }

    /// The screen's way out when the flow leaves it, its description freed.
    fn language_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == LANGUAGE {
            return false;
        }
        self.options_swap_proc(gobj, OUT);
        if let Some(t) = self.option_menu(gobj).1.text {
            self.destroy_text(t);
        }
        true
    }

    /// `fn_8024C270`, the screen in.
    #[doc(alias = "fn_8024C270")]
    pub fn language_idle(&mut self, gobj: GObjId) {
        self.language_leave(gobj);
    }

    /// `fn_8024C2E8`, the screen coming in; at its end the cursor moves.
    #[doc(alias = "fn_8024C2E8")]
    pub fn language_in(&mut self, gobj: GObjId) {
        if self.language_leave(gobj) {
            return;
        }
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, COME_IN, MOBJ_MASK) == crate::fpu::reg(COME_IN.end) {
            self.options_swap_proc(gobj, IDLE);
            self.option_menu_mut(gobj).unk2 = Some(1);
        }
    }

    /// `mnLanguage_8024C3C4`: the screen on render link 4, its user data (the cursor on the
    /// saved language) and process, the switch, and the description.
    #[doc(alias = "mnLanguage_8024C3C4")]
    fn language_make(&mut self) {
        let m = self.game.options.language.model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.language.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the language options");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        self.options_set_frame(Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(8);
        let lang = self.game.game_data.saved_language();
        let g = &mut *self.game;
        g.options.menus.insert(
            addr,
            OptionMenu {
                cursor: lang,
                unk1: Some(lang),
                unk2: Some(0),
                unk3: None,
                text: None,
            },
        );
        g.world.init_user_data(gobj, 0, UserData::OptionMenu(addr));
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        self.language_show(lang);
        self.options_set_frame_stopped(Some(j), 0.0, MOBJ_MASK);
        let t = self.menu_center_text(1, 0xBF);
        self.option_menu_mut(gobj).text = Some(t);
    }

    /// `mnLanguage_8024C5C0`, from the Options list: the flow on the language options
    /// (0x17), its model from the menus' archive, the screen and its input process.
    #[doc(alias = "mnLanguage_8024C5C0")]
    pub fn language_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = LANGUAGE;
        f.hovered_selection = 0;
        self.game.options.language.model = self.menu_model("MenMainConLa_Top");
        self.language_make();
        self.options_input_proc(INPUT);
    }
}
