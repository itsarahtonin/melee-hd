//! The screen options (`melee/mn/mndeflicker.c`): once the screen is in, A turns the
//! deflicker filter on or off, at once (the video mode, `gmMainLib_8015F588`) and in the save;
//! B leaves for the Options list and the save is written.

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_A, IN_BACK};
use super::options::{
    EXIT_TO, JOBJ_MASK, OptionMenu, SCREEN, SETTINGS, SFX_BACK, SFX_MOVE, TOBJ_MASK, al,
};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;

/// The processes: the input, the screen going out, idle and coming in.
pub const INPUT: u32 = 0x8024_A168;
pub const OUT: u32 = 0x8024_A2E8;
pub const IDLE: u32 = 0x8024_A344;
pub const IN: u32 = 0x8024_A3E8;

/// `mnDeflicker_803EEFD8`, `803EEFE4`, `803EEFF0`: the screen coming in, going out, and the
/// switch's idle loop.
const COME_IN: AnimLoop = al(0.0, 19.0, -0.1);
const GO_OUT: AnimLoop = al(20.0, 30.0, -0.1);
const IDLE_LOOP: AnimLoop = al(50.0, 350.0, 50.0);

/// `mndeflicker.c`'s statics: the model (`mnDeflicker_804A08B8`), the screen's GObj
/// (`mnDeflicker_804D6C38`) and whether it's in, taking A (`mnDeflicker_804D6C3C`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Deflicker {
    pub model: super::super::gm::title::ModelDesc,
    pub gobj: Option<GObjId>,
    pub ready: u8,
}

impl Running<'_> {
    fn deflicker_gobj(&self) -> GObjId {
        self.game
            .options
            .deflicker
            .gobj
            .expect("the screen options")
    }

    /// `gmMainLib_8015F4F4`: the save's deflicker option.
    #[doc(alias = "gmMainLib_8015F4F4")]
    pub fn set_deflicker(&mut self, on: u8) {
        self.game.game_data.thing.save_data.x1CB0.deflicker = on;
    }

    /// The switch at `on`, its texture held there, and the video mode with the filter or
    /// without.
    fn deflicker_show(&mut self, on: u8) {
        let j = self.options_jobj(self.deflicker_gobj());
        let s = self.options_node(Some(j), 5);
        self.options_set_frame_stopped(s, f32::from(on), JOBJ_MASK);
        self.set_video_mode(on != 0);
    }

    /// `mnDeflicker_8024A168`, the screen's input: B leaves (the option saved, the save
    /// written); once the screen is in, A turns the filter on or off.
    #[doc(alias = "mnDeflicker_8024A168")]
    pub fn deflicker_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        let g = self.deflicker_gobj();
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            let on = self.option_menu(g).1.cursor;
            self.set_deflicker(on);
            self.card_game_save_changes();
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(SETTINGS, 2, EXIT_TO);
        } else if self.game.options.deflicker.ready != 0 && ev & IN_A != 0 {
            self.lb_audio_call(SFX_MOVE);
            let m = self.option_menu_mut(g);
            m.cursor = u8::from(m.cursor == 0);
            let on = m.cursor;
            self.deflicker_show(on);
            self.set_deflicker(on);
        }
    }

    /// `mnDeflicker_8024A2E8`: the screen going out, then gone.
    #[doc(alias = "mnDeflicker_8024A2E8")]
    pub fn deflicker_out(&mut self, gobj: GObjId) {
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, GO_OUT, TOBJ_MASK) == crate::fpu::reg(GO_OUT.end) {
            gobj::free(self, gobj);
        }
    }

    /// The screen's way out when the flow leaves it, its description freed.
    fn deflicker_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == SCREEN {
            return false;
        }
        self.options_swap_proc(gobj, OUT);
        if let Some(t) = self.option_menu(gobj).1.text {
            self.destroy_text(t);
        }
        true
    }

    /// `mnDeflicker_8024A344`, the screen in: the switch's idle loop.
    #[doc(alias = "mnDeflicker_8024A344")]
    pub fn deflicker_idle(&mut self, gobj: GObjId) {
        if self.deflicker_leave(gobj) {
            return;
        }
        let j = self.options_jobj(gobj);
        let s = self.options_node(Some(j), 6).expect("the switch's light");
        self.anim_loop(s, IDLE_LOOP);
    }

    /// `mnDeflicker_8024A3E8`, the screen coming in; at its end the screen takes A.
    #[doc(alias = "mnDeflicker_8024A3E8")]
    pub fn deflicker_in(&mut self, gobj: GObjId) {
        if self.deflicker_leave(gobj) {
            return;
        }
        let j = self.options_jobj(gobj);
        if self.anim_loop_stopped(j, COME_IN, TOBJ_MASK) == crate::fpu::reg(COME_IN.end) {
            self.options_swap_proc(gobj, IDLE);
            self.game.options.deflicker.ready = 1;
        }
    }

    /// `mnDeflicker_8024A4BC`: the screen on render link 4, its user data (the save's option)
    /// and process, the switch and the video mode as the option says, and the description.
    #[doc(alias = "mnDeflicker_8024A4BC")]
    fn deflicker_make(&mut self) {
        let m = self.game.options.deflicker.model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.deflicker.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the screen options");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        self.options_set_frame(Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(8);
        let on = self.game.game_data.thing.save_data.x1CB0.deflicker;
        let g = &mut *self.game;
        g.options.menus.insert(
            addr,
            OptionMenu {
                cursor: on,
                ..OptionMenu::default()
            },
        );
        g.world.init_user_data(gobj, 0, UserData::OptionMenu(addr));
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        self.deflicker_show(on);
        self.options_set_frame_stopped(Some(j), 0.0, TOBJ_MASK);
        let t = self.menu_center_text(1, 0xBD);
        self.option_menu_mut(gobj).text = Some(t);
    }

    /// `mnDeflicker_8024A6C4`, from the Options list: the flow on the screen options (0x15),
    /// the screen not yet taking A, its model from the menus' archive, the screen and its
    /// input process.
    #[doc(alias = "mnDeflicker_8024A6C4")]
    pub fn deflicker_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.hovered_selection = 0;
        self.game.options.deflicker.ready = 0;
        self.game.options.deflicker.model = self.menu_model("MenMainConDf_Top");
        self.deflicker_make();
        self.options_input_proc(INPUT);
    }
}
