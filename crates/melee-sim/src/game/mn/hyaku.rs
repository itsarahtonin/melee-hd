//! The Multi-Man Melee list (`melee/mn/mnhyaku.c`): from the Stadium list, the six modes on a
//! panel turned with left and right, each with its description; A starts the mode, B goes
//! back to the Stadium list.

use super::super::gm::title::{AnimLoop, anim_frame};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_BACK, IN_CONFIRM, IN_LEFT, IN_RIGHT, STADIUM};
use super::options::{EXIT_TO, MOBJ_MASK, OptionMenu, SFX_BACK, SFX_FORWARD, SFX_MOVE, al};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};

/// The processes: the input, the panel going out, idle and coming in.
pub const INPUT: u32 = 0x8024_C68C;
pub const OUT: u32 = 0x8024_C9F0;
pub const IDLE: u32 = 0x8024_CA50;
pub const IN: u32 = 0x8024_CAC8;

/// The flow's `MenuKind` for the list (`MENU_KIND_MULTI_VS`).
pub const LIST: u8 = 0x21;

/// `mnHyaku_803EF668` and `803EF674`: the panel coming in and going out.
const COME_IN: AnimLoop = al(0.0, 19.0, -0.1);
const GO_OUT: AnimLoop = al(20.0, 29.0, -0.1);

/// `mnHyaku_803EF680`: each mode's frame of the panel's material animations.
const FRAMES: [f32; 6] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];

/// `vals`: each mode's description among the menus' strings.
const TEXTS: [i32; 6] = [0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0];

/// `mnhyaku.c`'s statics: the panel's model (`mnHyaku_804A08E8`) and its GObj
/// (`mnHyaku_804D6C58`). Its user data is the Options screens' `Menu` block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Hyaku {
    pub model: super::super::gm::title::ModelDesc,
    pub gobj: Option<GObjId>,
}

impl Running<'_> {
    fn hyaku_gobj(&self) -> GObjId {
        self.game.options.hyaku.gobj.expect("the Multi-Man list")
    }

    /// The panel on mode `cursor`'s frame of its material animations, the rest of its
    /// animations where they were (the inline `mnHyaku_8024C68C_inline`).
    fn hyaku_show(&mut self, j: JObjId, cursor: u8) {
        let cur = anim_frame(self, j);
        jobj::req_anim_all(&mut self.game.objs, Some(j), FRAMES[usize::from(cursor)]);
        self.options_anim_all(Some(j));
        jobj::req_anim_all(&mut self.game.objs, Some(j), cur);
        self.options_stop(Some(j), MOBJ_MASK);
        self.options_anim_all(Some(j));
    }

    /// The description of the mode the cursor is on, the old one freed (the inline
    /// `mnHyaku_8024C68C_inline_2`).
    fn hyaku_text(&mut self, gobj: GObjId) {
        let m = self.option_menu(gobj).1;
        if let Some(t) = m.text {
            self.destroy_text(t);
        }
        let t = self.menu_center_text(1, TEXTS[usize::from(m.cursor)]);
        self.option_menu_mut(gobj).text = Some(t);
    }

    /// `mnHyaku_8024C68C`, the list's input: B back to the Stadium list on Multi-Man; A the
    /// confirming port kept for the mode and the scene ended for the mode the cursor is on;
    /// left and right turn the panel, wrapping.
    #[doc(alias = "mnHyaku_8024C68C")]
    pub fn hyaku_input(&mut self, _gobj: GObjId) {
        let g = self.hyaku_gobj();
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(STADIUM, 2, EXIT_TO);
            return;
        }
        if ev & IN_CONFIRM != 0 {
            self.lb_audio_call(SFX_FORWARD);
            // `mn_802295AC`'s port is `gm_801677E8`'s argument: the call passes on the r3
            // `mn_802295AC` returned.
            self.note_confirm_port();
            let cursor = self.option_menu(g).1.cursor;
            if cursor < 6 {
                self.menu_exit_to(super::super::gm::GM_10MAN_VS + cursor);
            }
            return;
        }
        let step = if ev & IN_LEFT != 0 {
            -1
        } else if ev & IN_RIGHT != 0 {
            1
        } else {
            return;
        };
        self.lb_audio_call(SFX_MOVE);
        let m = self.option_menu_mut(g);
        m.cursor = match (step, m.cursor) {
            (-1, 0) => 5,
            (-1, c) => c - 1,
            (_, 5) => 0,
            (_, c) => c + 1,
        };
        let cursor = m.cursor;
        let j = self.options_jobj(g);
        self.hyaku_show(j, cursor);
        self.hyaku_text(g);
    }

    /// `mnHyaku_8024C9F0`: the panel going out, then gone.
    #[doc(alias = "mnHyaku_8024C9F0")]
    pub fn hyaku_out(&mut self, gobj: GObjId) {
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, GO_OUT, 0x80) >= crate::fpu::reg(GO_OUT.end) {
            gobj::free(self, gobj);
        }
    }

    /// The panel's way out once the flow leaves the list: its description freed (its
    /// handle kept, as the original keeps its pointer).
    fn hyaku_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == LIST {
            return false;
        }
        self.options_swap_proc(gobj, OUT);
        if let Some(t) = self.option_menu(gobj).1.text {
            self.destroy_text(t);
        }
        true
    }

    /// `mnHyaku_8024CA50`, the panel in.
    #[doc(alias = "mnHyaku_8024CA50")]
    pub fn hyaku_idle(&mut self, gobj: GObjId) {
        self.hyaku_leave(gobj);
    }

    /// `mnHyaku_8024CAC8`, the panel coming in; at its end, idle.
    #[doc(alias = "mnHyaku_8024CAC8")]
    pub fn hyaku_in(&mut self, gobj: GObjId) {
        if self.hyaku_leave(gobj) {
            return;
        }
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, COME_IN, 0x80) == crate::fpu::reg(COME_IN.end) {
            self.options_swap_proc(gobj, IDLE);
        }
    }

    /// `mnHyaku_8024CB94`: the panel on render link 4 at its first frame, its user data
    /// (the cursor at `cursor`), on that mode's frame, its process and the description.
    #[doc(alias = "mnHyaku_8024CB94")]
    fn hyaku_make(&mut self, cursor: u8) {
        let m = self.game.options.hyaku.model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.hyaku.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the Multi-Man list");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        self.options_set_frame(Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(8);
        let g = &mut *self.game;
        g.options.menus.insert(
            addr,
            OptionMenu {
                cursor,
                ..OptionMenu::default()
            },
        );
        g.world.init_user_data(gobj, 0, UserData::OptionMenu(addr));
        self.hyaku_show(j, cursor);
        let w = &mut self.game.world;
        let p = w.setup_proc(gobj, CodeRef(IN), 0);
        w.proc_mut(p).flags_3 = w.phase;
        self.hyaku_text(gobj);
    }

    /// `mnHyaku_8024CD64`, from the Stadium list (on 10-Man) or the menu's entry back from a
    /// mode (on that mode): the flow on the list, the announcer's other bank (bank 2's 8)
    /// wanted and waited for, the panel's model from the menus' archive, the panel, the
    /// input's process, and the list's voice (0x7530).
    #[doc(alias = "mnHyaku_8024CD64")]
    pub fn hyaku_open(&mut self, cursor: u8) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = LIST;
        f.hovered_selection = 0;
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 8);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        self.game.options.hyaku.model = self.menu_model("MenMainConKm_Top");
        self.hyaku_make(cursor);
        self.options_input_proc(INPUT);
        self.sfx_play(0x7530, 0x7F, 0x40);
    }

    /// Runs the list's processes.
    pub fn hyaku_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            INPUT => self.hyaku_input(gobj),
            OUT => self.hyaku_out(gobj),
            IDLE => self.hyaku_idle(gobj),
            IN => self.hyaku_in(gobj),
            _ => return false,
        }
        true
    }
}
