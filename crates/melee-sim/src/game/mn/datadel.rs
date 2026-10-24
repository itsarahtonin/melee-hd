//! Erasing data (`melee/mn/mndatadel.c`): a list of six (the characters, the stages, the
//! records, the names and their records, the trophies, all data), each erasable once a visit:
//! A asks with a warning, yes erases and saves; erasing all data asks twice. B leaves for the
//! Options list.

use super::super::gm::title::{AnimLoop, anim_frame};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_BACK, IN_CONFIRM, IN_DOWN, IN_LEFT, IN_RIGHT, IN_UP};
use super::options::{ERASE, EXIT_TO, HIDDEN, MOBJ_MASK, SETTINGS, SFX_BACK, SFX_MOVE, al};
use crate::fpu::reg;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the warning's, its second warning's input, its input, the list's input,
/// the screen going out, idle and coming in.
pub const WARNING: u32 = 0x8024_ECCC;
pub const CONFIRM: u32 = 0x8024_F1D4;
pub const WARNING_INPUT: u32 = 0x8024_F318;
pub const INPUT: u32 = 0x8024_F840;
pub const OUT: u32 = 0x8024_FBA4;
pub const IDLE: u32 = 0x8024_FC48;
pub const IN: u32 = 0x8024_FD40;

/// `mnDataDel_803EF870`: the screen coming in and going out; `803EF888` and `803EF894` an
/// entry's panel hovered and not; `803EF8A0` the warning coming in.
const COME_IN: AnimLoop = al(0.0, 19.0, -0.1);
const GO_OUT: AnimLoop = al(20.0, 29.0, -0.1);
const HOVERED: AnimLoop = al(0.0, 5.0, -0.1);
const UNHOVERED: AnimLoop = al(0.0, 0.0, -0.1);
const WARN_IN: AnimLoop = al(0.0, 9.0, -0.1);
/// `mnDataDel_803EF8AC`: each entry's place among the screen's joints.
const ENTRY_JOINTS: [usize; 7] = [1, 2, 3, 4, 5, 6, 7];
/// `mnDataDel_803EF8C8`: each entry's description.
const DESCRIPTIONS: [i32; 6] = [0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC6];
/// `lbl_803EF8D4`: where the warning's text goes.
const WARN_TEXT: Vec3 = Vec3 {
    x: -5.5,
    y: -2.8,
    z: 23.0,
};

/// `WarnCommonJoints`: the warning's parts.
const WARN_BACKGROUND: i32 = 1;
const WARN_PANEL: i32 = 2;
const WARN_PANEL_TEXT_BOTTOM: i32 = 4;
const WARN_CURSOR_YES: i32 = 6;
const WARN_CURSOR_NO: i32 = 7;
const WARN_EXCLAIM: i32 = 8;
const WARN_EXCLAIM_ARROW: i32 = 10;
const WARN_PROGRESS_BAR: i32 = 11;

/// The entry erasing all data.
const ALL: u8 = 5;

/// `MnDataDelGObjUserData`, the screen's user data (0x30 bytes of the heap); the warning
/// reads its first three bytes as its own (`WarnCmnData`: the entry, whether the warning is
/// up, 1 first or 2 the second, and its cursor on yes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct DataDelData {
    pub cursor: u8,
    pub warning: u8,
    pub yes: u8,
    /// `x3`: each entry erased this visit.
    pub erased: [u8; 6],
    pub text: Option<TextId>,
    /// `x10`: the screen's first eight joints in pre-order.
    pub jobjs: [Option<JObjId>; 8],
}

/// `mndatadel.c`'s statics: the models (`mnDataDel_804A0918` the screen,
/// `mnDataDel_804A0928` an entry's panel, `mnDataDel_804A0938` the warning), the screen's GObj
/// (`mnDataDel_804D6C68`), the warning's text (`mnDataDel_804D6C6C`), and the user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DataDel {
    pub models: [super::super::gm::title::ModelDesc; 3],
    pub gobj: Option<GObjId>,
    pub warn_text: Option<TextId>,
    pub data: std::collections::BTreeMap<u32, DataDelData>,
}

/// `mn_80231634`: a joint's first child, none for no joint.
#[doc(alias = "mn_80231634")]
fn first_child(r: &Running, j: Option<JObjId>) -> Option<JObjId> {
    j.and_then(|j| r.game.objs.jobjs[j].child)
}

impl Running<'_> {
    fn datadel_addr(&self) -> u32 {
        let g = self.game.options.datadel.gobj.expect("the erasing screen");
        match self.game.world.gobj(g).user_data {
            Some(UserData::DataDel(a)) => a,
            other => panic!("the erasing screen holds {other:?}"),
        }
    }

    fn datadel(&self) -> DataDelData {
        self.game.options.datadel.data[&self.datadel_addr()]
    }

    fn datadel_mut(&mut self) -> &mut DataDelData {
        let a = self.datadel_addr();
        self.game
            .options
            .datadel
            .data
            .get_mut(&a)
            .expect("the erasing screen's data")
    }

    /// Entry `i`'s panel (`mn_80231634` of its joint).
    fn datadel_panel(&self, i: u8) -> Option<JObjId> {
        let d = self.datadel();
        first_child(self, d.jobjs[ENTRY_JOINTS[usize::from(i)]])
    }

    /// An entry's panel greyed: its lamp's frame 1 for its materials, the rest held where
    /// it was (`mnDataDel_8024E940_inline` and the erase cases).
    fn datadel_grey(&mut self, i: u8) {
        let p = self.datadel_panel(i);
        let lamp = self.options_node(p, 1);
        let f = anim_frame(self, lamp.expect("an entry's lamp"));
        self.options_set_frame_stopped(lamp, 1.0, MOBJ_MASK);
        self.options_set_frame_stopped(lamp, f, 0x480);
        self.datadel_mut().erased[usize::from(i)] = 1;
    }

    /// `mnDataDel_8024E940`: the stages' entry greyed and the stages erased.
    #[doc(alias = "mnDataDel_8024E940")]
    fn datadel_erase_stages(&mut self) {
        self.datadel_grey(1);
        self.erase_stages();
    }

    /// `mnDataDel_8024EA6C`: every entry greyed and all data erased.
    #[doc(alias = "mnDataDel_8024EA6C")]
    fn datadel_erase_all(&mut self) {
        for i in 0..6 {
            let p = self.datadel_panel(i);
            let lamp = self.options_node(p, 1);
            let f = anim_frame(self, lamp.expect("an entry's lamp"));
            self.options_set_frame_stopped(lamp, 1.0, MOBJ_MASK);
            self.options_set_frame_stopped(lamp, f, 0x480);
            self.datadel_mut().erased[usize::from(i)] = 1;
        }
        self.erase_all();
    }

    /// `mnDataDel_8024EBC8`: an entry's panel hovered (`hovered`) or not, at its loop's end
    /// when `at_end`, else its start.
    #[doc(alias = "mnDataDel_8024EBC8")]
    fn datadel_hover(&mut self, root: Option<JObjId>, hovered: bool, at_end: bool) {
        let t = self.options_node(root, WARN_PANEL_TEXT_BOTTOM);
        self.options_set_frame_stopped(t, if hovered { 1.0 } else { 0.0 }, 0x400);
        let b = self.options_node(root, WARN_BACKGROUND);
        let l = if hovered { HOVERED } else { UNHOVERED };
        let f = if at_end { l.end } else { l.start };
        self.options_set_frame_stopped(b, f, 0x400);
    }

    /// The warning's text, the first warning's or the second's (`HSD_SisLib_803A5ACC` at
    /// [`WARN_TEXT`], centered), the old one freed.
    fn datadel_warn_text(&mut self, which: u8) {
        if let Some(t) = self.game.options.datadel.warn_text {
            self.destroy_text(t);
        }
        let id = if which == 1 { 0x13E } else { 0x13F };
        let t = self.create_text(0, 1, WARN_TEXT, 250.0, 5.0);
        self.game.options.datadel.warn_text = Some(t);
        let s = &mut self.game.sis;
        let text = s.text_mut(t);
        text.font_size = [0.05, 0.05];
        text.default_alignment = 1;
        s.reset_text(self.data, t, id);
    }

    /// The warning's cursor on yes or no.
    fn datadel_warn_cursor(&mut self, root: JObjId, yes: bool) {
        let y = self.options_node(Some(root), WARN_CURSOR_YES);
        let n = self.options_node(Some(root), WARN_CURSOR_NO);
        let (fy, fn_) = if yes { (1.0, 0.0) } else { (0.0, 1.0) };
        jobj::req_anim_all(&mut self.game.objs, y, fy);
        jobj::req_anim_all(&mut self.game.objs, n, fn_);
        self.options_anim_all(y);
        self.options_anim_all(n);
    }

    /// `fn_8024ECCC`, the warning's process: coming in, its text once in; then its cursor;
    /// gone with its text when it's closed.
    #[doc(alias = "fn_8024ECCC")]
    pub fn datadel_warning_proc(&mut self, gobj: GObjId) {
        let root = self.options_jobj(gobj);
        let d = self.datadel();
        let panel = self.options_node(Some(root), WARN_PANEL);
        if d.warning == 0 {
            if let Some(t) = self.game.options.datadel.warn_text.take() {
                self.destroy_text(t);
            }
            gobj::free(self, gobj);
            return;
        }
        let f = reg(anim_frame(self, root));
        if reg(WARN_IN.start) <= f && f < reg(WARN_IN.end) {
            let f = self.anim_loop_one(panel.expect("the warning's panel"), WARN_IN, None);
            let ex = self.options_node(Some(root), WARN_EXCLAIM);
            self.anim_loop_one(ex.expect("the warning's mark"), WARN_IN, None);
            if f >= reg(WARN_IN.end) {
                self.datadel_warn_text(d.warning);
            }
        } else {
            self.datadel_warn_cursor(root, d.yes != 0);
        }
    }

    /// `mnDataDel_8024EEC0`: the warning on render link 6 with its process, its arrow and
    /// progress bar hidden, its cursor; in English yes and no change places.
    #[doc(alias = "mnDataDel_8024EEC0")]
    fn datadel_warning(&mut self) {
        let m = self.game.options.datadel.models[2];
        let data = self.data;
        let yes = self.datadel().yes != 0;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        let root = jobj::load_joint(&mut g.objs, data, m.joint).expect("the warning");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        g.world.setup_proc(gobj, CodeRef(WARNING), 0);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        self.options_set_frame(Some(root), WARN_IN.start);
        for part in [WARN_EXCLAIM_ARROW, WARN_PROGRESS_BAR] {
            let c = self.options_node(Some(root), part);
            jobj::set_flags_all(&mut self.game.objs.jobjs, c, HIDDEN);
        }
        self.datadel_warn_cursor(root, yes);
        if self.game.game_data.saved_language() == super::super::gm::main_lib::LANG_US {
            let y = self.options_node(Some(root), WARN_CURSOR_YES).expect("yes");
            let n = self.options_node(Some(root), WARN_CURSOR_NO).expect("no");
            let jobjs = &mut self.game.objs.jobjs;
            let (xy, xn) = (jobjs[y].translate.x, jobjs[n].translate.x);
            jobj::set_srt(jobjs, y, |o| o.translate.x = xn);
            jobj::set_srt(jobjs, n, |o| o.translate.x = xy);
        }
    }

    /// The warning closed and the list's input back.
    fn datadel_back_to_list(&mut self, gobj: GObjId) {
        self.options_swap_proc(gobj, INPUT);
    }

    /// `fn_8024F1D4`, the second warning's input (erasing all data): A on yes erases it, on
    /// no goes back; B goes back; left or right move the cursor.
    #[doc(alias = "fn_8024F1D4")]
    pub fn datadel_confirm(&mut self, gobj: GObjId) {
        let ev = self.list_inputs();
        if ev & IN_CONFIRM != 0 {
            if self.datadel().yes != 0 {
                self.datadel_erase_all();
            } else {
                self.lb_audio_call(SFX_BACK);
            }
            self.game.menu.cooldown = 5;
            self.datadel_mut().warning = 0;
            self.datadel_back_to_list(gobj);
        } else if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.cooldown = 5;
            self.datadel_mut().warning = 0;
            self.datadel_back_to_list(gobj);
        } else if ev & (IN_RIGHT | IN_LEFT) != 0 {
            self.lb_audio_call(SFX_MOVE);
            let d = self.datadel_mut();
            d.yes = u8::from(d.yes == 0);
        }
    }

    /// `fn_8024F318`, the warning's input: A on yes erases the entry (all data asks again),
    /// on no goes back; B goes back; left or right move the cursor.
    #[doc(alias = "fn_8024F318")]
    pub fn datadel_warning_input(&mut self, gobj: GObjId) {
        let ev = self.list_inputs();
        let d = self.datadel();
        if ev & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            if d.cursor == ALL {
                let w = &mut self.game.world;
                let p = w.current_proc().expect("the running process");
                w.remove_proc(p);
                if d.yes != 0 {
                    let dm = self.datadel_mut();
                    dm.warning = 2;
                    dm.yes = 0;
                    let w = &mut self.game.world;
                    let p = w.setup_proc(gobj, CodeRef(CONFIRM), 0);
                    w.proc_mut(p).flags_3 = w.phase;
                    self.datadel_warn_text(2);
                    self.game.menu.cooldown = 10;
                    self.sfx_play(0xBD, 0x7F, 0x40);
                    return;
                }
                self.lb_audio_call(SFX_BACK);
                let dm = self.datadel_mut();
                dm.warning = 0;
                dm.yes = 0;
                let w = &mut self.game.world;
                let p = w.setup_proc(gobj, CodeRef(INPUT), 0);
                w.proc_mut(p).flags_3 = w.phase;
                return;
            }
            if d.yes != 0 {
                match d.cursor {
                    0 => {
                        self.datadel_grey(0);
                        self.erase_characters();
                    }
                    1 => self.datadel_erase_stages(),
                    2 => {
                        self.datadel_grey(2);
                        self.erase_records();
                    }
                    3 => {
                        self.datadel_grey(3);
                        self.erase_names();
                    }
                    4 => {
                        self.datadel_grey(4);
                        self.erase_trophies();
                    }
                    _ => {}
                }
                self.sfx_play(0xBE, 0x7F, 0x40);
            } else {
                self.lb_audio_call(SFX_BACK);
            }
            self.datadel_mut().warning = 0;
            self.datadel_back_to_list(gobj);
            return;
        }
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.cooldown = 5;
            self.datadel_mut().warning = 0;
            self.datadel_back_to_list(gobj);
            return;
        }
        if ev & (IN_RIGHT | IN_LEFT) != 0 {
            self.lb_audio_call(SFX_MOVE);
            let d = self.datadel_mut();
            d.yes = u8::from(d.yes == 0);
        }
    }

    /// The hovered entry's description (`mnDataDel_UpdateDescription`), the old one freed.
    fn datadel_description(&mut self) {
        if let Some(t) = self.datadel().text {
            self.destroy_text(t);
        }
        let id = DESCRIPTIONS[usize::from(self.datadel().cursor)];
        let t = self.create_text(0, 0, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
        self.datadel_mut().text = Some(t);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, id);
    }

    /// The cursor one entry up or down, wrapping: the old entry's panel and the new one's,
    /// and the description.
    fn datadel_move(&mut self, down: bool) {
        self.lb_audio_call(SFX_MOVE);
        let c = self.datadel().cursor;
        let p = self.datadel_panel(c);
        self.datadel_hover(p, false, false);
        let n = if down {
            if c == ALL { 0 } else { c + 1 }
        } else if c != 0 {
            c - 1
        } else {
            ALL
        };
        self.datadel_mut().cursor = n;
        let p = self.datadel_panel(n);
        self.datadel_hover(p, true, false);
        self.datadel_description();
    }

    /// `fn_8024F840`, the list's input: B leaves; A on an entry not yet erased brings up its
    /// warning; up and down move the cursor.
    #[doc(alias = "fn_8024F840")]
    pub fn datadel_input(&mut self, gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        if ev & IN_BACK != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(SETTINGS, 5, EXIT_TO);
            return;
        }
        let d = self.datadel();
        if ev & IN_CONFIRM != 0 && d.erased[usize::from(d.cursor)] == 0 {
            let dm = self.datadel_mut();
            dm.warning = 1;
            dm.yes = 0;
            self.options_swap_proc(gobj, WARNING_INPUT);
            self.datadel_warning();
            self.game.menu.cooldown = 10;
            self.sfx_play(0xBC, 0x7F, 0x40);
            return;
        }
        if ev & IN_UP != 0 {
            self.datadel_move(false);
            return;
        }
        if ev & IN_DOWN != 0 {
            self.datadel_move(true);
        }
    }

    /// The screen's tree and its entries' panels, one animation frame through `l`.
    fn datadel_anim(&mut self, gobj: GObjId, l: AnimLoop) -> f64 {
        let j = self.options_jobj(gobj);
        let f = self.anim_loop_one(j, l, Some(0x480));
        let d = self.datadel();
        for k in ENTRY_JOINTS.iter().take(6) {
            let e = d.jobjs[*k].expect("an entry's joint");
            self.anim_loop_one(e, l, Some(0x480));
        }
        f
    }

    /// `fn_8024FBA4`: the screen going out, then gone.
    #[doc(alias = "fn_8024FBA4")]
    pub fn datadel_out(&mut self, gobj: GObjId) {
        if self.datadel_anim(gobj, GO_OUT) >= reg(GO_OUT.end) {
            gobj::free(self, gobj);
        }
    }

    /// The screen's way out when the flow leaves it, its description freed.
    fn datadel_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == ERASE {
            return false;
        }
        self.options_swap_proc(gobj, OUT);
        if let Some(t) = self.datadel().text {
            self.destroy_text(t);
        }
        true
    }

    /// `fn_8024FC48`, the screen in: each entry's panel lit or not as the cursor is on it.
    #[doc(alias = "fn_8024FC48")]
    pub fn datadel_idle(&mut self, gobj: GObjId) {
        if self.datadel_leave(gobj) {
            return;
        }
        for i in 0..6u8 {
            let p = self.datadel_panel(i);
            let b = self.options_node(p, 1).expect("an entry's panel");
            let l = if self.datadel().cursor == i {
                HOVERED
            } else {
                UNHOVERED
            };
            self.anim_loop_stopped(b, l, 0x400);
        }
    }

    /// `fn_8024FD40`, the screen coming in; at its end the screen's idle process.
    #[doc(alias = "fn_8024FD40")]
    pub fn datadel_in(&mut self, gobj: GObjId) {
        if self.datadel_leave(gobj) {
            return;
        }
        if self.datadel_anim(gobj, COME_IN) == reg(COME_IN.end) {
            self.options_swap_proc(gobj, IDLE);
        }
    }

    /// `mnDataDel_8024FE4C`: the screen on render link 4, its user data (the cursor on
    /// `cursor`, nothing erased) and joints, its process, each entry's panel (hovered, lit
    /// as erased or not), and the description.
    #[doc(alias = "mnDataDel_8024FE4C")]
    fn datadel_make(&mut self, cursor: u8) {
        let m = self.game.options.datadel.models[0];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.datadel.gobj = Some(gobj);
        let root = jobj::load_joint(&mut g.objs, data, m.joint).expect("the erasing screen");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        self.options_set_frame(Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x30);
        let mut jobjs = [None; 8];
        for (i, j) in jobjs.iter_mut().enumerate() {
            *j = self.options_node(Some(root), i as i32);
        }
        let g = &mut *self.game;
        g.options.datadel.data.insert(
            addr,
            DataDelData {
                cursor,
                jobjs,
                ..DataDelData::default()
            },
        );
        g.world.init_user_data(gobj, 0, UserData::DataDel(addr));
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        let pm = self.game.options.datadel.models[1];
        for i in 0..6u8 {
            let o = &mut self.game.objs;
            let j = jobj::load_joint(o, data, pm.joint).expect("an entry's panel");
            jobj::add_anim_all(o, data, Some(j), pm.anim, pm.matanim, pm.shapeanim);
            self.options_set_frame(Some(j), f32::from(i));
            jobj::add_child(
                &mut self.game.objs.jobjs,
                jobjs[ENTRY_JOINTS[usize::from(i)]],
                Some(j),
            );
            self.datadel_hover(Some(j), i == cursor, true);
            let erased = self.datadel().erased[usize::from(i)];
            let lamp = self.options_node(Some(j), 1);
            let f = anim_frame(self, lamp.expect("an entry's lamp"));
            self.options_set_frame_stopped(lamp, f32::from(u8::from(erased != 0)), MOBJ_MASK);
            self.options_set_frame_stopped(lamp, f, 0x480);
        }
        self.datadel_description();
    }

    /// `mnDataDel_80250170`, from the Options list: the flow on erasing data (0x18), no
    /// warning text, the models from the menus' archive, the screen and its input process.
    #[doc(alias = "mnDataDel_80250170")]
    pub fn datadel_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = ERASE;
        f.hovered_selection = 0;
        self.game.options.datadel.warn_text = None;
        let models = [
            "MenMainConDl_Top",
            "MenMainCursorDl_Top",
            "MenMainWarCmn_Top",
        ];
        for (i, p) in models.into_iter().enumerate() {
            self.game.options.datadel.models[i] = self.menu_model(p);
        }
        self.datadel_make(0);
        self.options_input_proc(INPUT);
    }
}
