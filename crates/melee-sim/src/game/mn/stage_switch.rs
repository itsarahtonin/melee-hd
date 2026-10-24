//! The random stage switch (`melee/mn/mnstagesw.c`): the 29 stages in two columns, each in or
//! out of the random stage select, the locked ones hidden ("?"). A toggles the hovered stage
//! (but not the last one in), writing the preferences' stage mask at once; B and Start save
//! (`lbCardGame_SaveChanges`).

use std::collections::BTreeMap;

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{
    add_child, add_ty_madd, al, anim, child, frame_of, game_mode, hide, load_model, next, node,
    req, set_frame_all, set_tx, set_ty, show, tx, ty,
};
use super::main::{IN_A, IN_BACK, IN_START};
use super::rules::{MODEL_CON_SS, MODEL_CURSOR_SS, STAGE_SWITCH};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes.
pub const THINK: u32 = 0x8023_5F80;
pub const LIST_PROC: u32 = 0x8023_6998;

const STAGES: usize = 29;

/// The model's joints the screen keeps (`x20` on): the cursor, the first column, the first
/// column's second row (for the rows' spacing) and the second column.
const X28: usize = 2;
const X2C: usize = 3;
const X30: usize = 4;
const X34: usize = 5;

/// `mnStageSw_803ED488`: a hovered stage's loop, then the screen's ways in and out.
const HOVER: AnimLoop = al(0.0, 199.0, 0.0);
const WAYS: [AnimLoop; 4] = [
    al(0.0, 9.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
];
/// `mnStageSw_803ED4C4`: each place's stage select entry.
const ORDER: [u8; STAGES] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0B, 0x0A, 0x18, 0x1A, 0x1C, 0x0C,
    0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1B,
];
/// `mnStageSw_stageIcons`: each stage select entry's name, by the entry.
const NAMES: [u8; STAGES] = [
    0x08, 0x09, 0x11, 0x0A, 0x0C, 0x06, 0x0B, 0x07, 0x0E, 0x0D, 0x1D, 0x17, 0x0F, 0x10, 0x12, 0x13,
    0x14, 0x15, 0x1A, 0x1B, 0x1C, 0x16, 0x18, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24,
];
/// A locked stage's name.
const LOCKED: i32 = 0x25;
/// `mnStageSw_804D4BB8`: a stage's switch, out and in.
const SWITCH: [f32; 2] = [0.0, 1.0];

/// The random stage switch's user data (`MnStageSwData`, 0xB4 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct StageSwitchData {
    pub menu_kind: u8,
    pub hovered: u8,
    /// `x2`: each place's stage in or out (locked ones out).
    pub stages: [u8; STAGES],
    /// `x1F`: the screen's state (`MenuState`).
    pub state: u8,
    /// `x20` to `x34`: the model's first six joints.
    pub jobjs: [Option<JObjId>; 6],
    /// `x40`: each place's name, once in.
    pub texts: [Option<TextId>; STAGES],
}

/// `mnstagesw.c`'s statics: the screen (`mnStageSw_804D6BF0`), its inputs held off while it
/// comes in (`mnStageSw_804D6BF4`), and the screens' user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StageSwitch {
    pub gobj: Option<GObjId>,
    pub coming_in: i8,
    pub data: BTreeMap<u32, StageSwitchData>,
}

impl Running<'_> {
    fn stages_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::StageSwitch(a)) => a,
            other => panic!("the stage switch holds {other:?}"),
        }
    }

    fn stages_data(&self, g: GObjId) -> StageSwitchData {
        self.game.vs_menus.stages.data[&self.stages_addr(g)]
    }

    fn stages_mut(&mut self, g: GObjId) -> &mut StageSwitchData {
        let a = self.stages_addr(g);
        self.game
            .vs_menus
            .stages
            .data
            .get_mut(&a)
            .expect("the stage switch's data")
    }

    /// Whether place `i`'s stage is unlocked (`gm_80164430(gm_801641CC(...))`).
    fn place_unlocked(&self, i: usize) -> bool {
        self.stage_unlocked(super::super::gm::opening::stage_select_kind(ORDER[i]))
    }

    /// `gm_801641E4`: a stage select entry in or out of the preferences' stage mask.
    #[doc(alias = "gm_801641E4")]
    pub(crate) fn set_stage_enabled(&mut self, entry: u8, on: bool) {
        let bit = (1i32 << entry) as u32;
        let p = &mut self.game.game_data.thing.save_data.x1CB0;
        if on {
            p.stage_mask |= bit;
        } else {
            p.stage_mask &= !bit;
        }
    }

    /// `gm_IsStageUnlocked`: whether the preferences' stage mask has an entry in.
    #[doc(alias = "gm_IsStageUnlocked")]
    pub(crate) fn stage_enabled(&self, entry: u8) -> bool {
        (1i32 << entry) as u32 & self.game.game_data.thing.save_data.x1CB0.stage_mask != 0
    }

    /// `mnStageSw_8023593C`: every unlocked place's switch into the stage mask.
    #[doc(alias = "mnStageSw_8023593C")]
    fn stages_commit(&mut self, g: GObjId) {
        let d = self.stages_data(g);
        for (i, &entry) in ORDER.iter().enumerate() {
            if self.place_unlocked(i) {
                self.set_stage_enabled(entry, d.stages[i] != 0);
            }
        }
    }

    /// `mnStageSw_802359C8`: each place's name beside its column (a locked one's "?").
    #[doc(alias = "mnStageSw_802359C8")]
    fn stages_texts(&mut self, addr: u32) {
        let d = self.game.vs_menus.stages.data[&addr];
        let reg = crate::fpu::reg;
        let start = gekko_fp::fadds(reg(-1.6f32), reg(ty(self, d.jobjs[X2C])));
        let step = reg(ty(self, d.jobjs[X2C]));
        let delta = gekko_fp::fsubs(reg(ty(self, d.jobjs[X30])), step);
        let canvas = i32::from(self.game.menu.canvas_erase);
        for i in 0..STAGES {
            let (col, row) = if i < 15 {
                (d.jobjs[X2C], i)
            } else {
                (d.jobjs[X34], i - 15)
            };
            let x = crate::fpu::mem(gekko_fp::fadds(1.0, reg(tx(self, col))));
            let y = crate::fpu::mem(gekko_fp::fnmadds(delta, f64::from(row as f32), start));
            let t = self.create_text(0, canvas, Vec3::new(x, y, 17.5), 160.0, 300.0);
            self.game
                .vs_menus
                .stages
                .data
                .get_mut(&addr)
                .expect("the data")
                .texts[i] = Some(t);
            let unlocked = self.place_unlocked(i);
            let s = &mut self.game.sis;
            let tx_ = s.text_mut(t);
            tx_.font_size = [0.0521, 0.0521];
            tx_.default_alignment = 2;
            tx_.default_fitting = 1;
            let id = if unlocked {
                i32::from(NAMES[usize::from(ORDER[i])])
            } else {
                LOCKED
            };
            s.reset_text(self.data, t, id);
        }
    }

    /// `mnStageSw_80235C58`: the unlocked place nearest `at` in its column (above first at
    /// each distance), `at` itself if unlocked, none (-1) when the column has none.
    #[doc(alias = "mnStageSw_80235C58")]
    fn nearest_place(&self, at: u8) -> i32 {
        let (low, high) = if at < 15 { (0, 14) } else { (15, 28) };
        if !(low..=high).any(|i| self.place_unlocked(i)) {
            return -1;
        }
        if usize::from(at) < STAGES && self.place_unlocked(usize::from(at)) {
            return i32::from(at);
        }
        let at = i32::from(at);
        let mut i = 1;
        loop {
            let prev = at - i;
            if low as i32 <= prev && self.place_unlocked(prev as u8 as usize) {
                return prev;
            }
            let n = at + i;
            if n <= high as i32 && self.place_unlocked(n as u8 as usize) {
                return n;
            }
            i += 1;
        }
    }

    /// `mnStageSw_80235DC8`: up and down through a column's unlocked places, wrapping; left
    /// and right to the nearest unlocked place of the other column. The flow's value the
    /// place's switch.
    #[doc(alias = "mnStageSw_80235DC8")]
    fn stages_move(&mut self, d: &StageSwitchData, buttons: u32) {
        let mut idx = self.game.menu.flow.hovered_selection as u8;
        if buttons & 3 != 0 {
            loop {
                let n = if buttons & 1 != 0 {
                    match idx {
                        0 => 14,
                        15 => 28,
                        i => i - 1,
                    }
                } else {
                    match idx {
                        14 => 0,
                        28 => 15,
                        i => i + 1,
                    }
                };
                self.game.menu.flow.hovered_selection = u16::from(n);
                idx = n;
                if self.place_unlocked(usize::from(idx)) {
                    break;
                }
            }
            self.set_confirmed(Some(d.stages[usize::from(idx)]));
            return;
        }
        let target = if buttons & 4 != 0 {
            if !(15..STAGES as u8).contains(&idx) {
                return;
            }
            idx - 15
        } else if buttons & 8 != 0 && idx < 15 {
            idx + 15
        } else {
            return;
        };
        let s = self.nearest_place(target);
        if s != -1 {
            self.game.menu.flow.hovered_selection = u16::from(s as u8);
            self.set_confirmed(Some(d.stages[usize::from(s as u8)]));
        }
    }

    /// The switches into the stage mask, then the save (`saveSettings`).
    fn stages_save(&mut self) {
        let g = self.game.vs_menus.stages.gobj.expect("the stage switch");
        self.stages_commit(g);
        self.card_game_mark_changes();
    }

    /// `fn_80235F80`, the stage switch's think: B saves and goes back to the extra rules; once
    /// the screen is in, A toggles the hovered stage (refused for the last one in) and writes
    /// the mask, Start saves and goes on to the character select, the directions move.
    #[doc(alias = "fn_80235F80")]
    pub fn stage_switch_think(&mut self, gobj: GObjId) {
        let screen = self.game.vs_menus.stages.gobj.expect("the stage switch");
        let d = self.stages_data(screen);
        let buttons = self.list_inputs();
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.stages_save();
            self.game.menu.cooldown = 5;
            self.rule_plus_open();
            gobj::free(self, gobj);
            return;
        }
        if self.game.vs_menus.stages.coming_in != 0 {
            return;
        }
        if buttons & IN_A != 0 {
            if usize::from(self.game.menu.flow.hovered_selection) < STAGES {
                if self.confirmed_read() != 0 {
                    let on = d.stages.iter().filter(|&&s| s != 0).count();
                    if on > 1 {
                        self.lb_audio_call(2);
                        self.set_confirmed(Some(0));
                    } else {
                        self.lb_audio_call(3);
                    }
                } else {
                    self.lb_audio_call(2);
                    self.set_confirmed(Some(1));
                }
                self.stages_commit(screen);
                return;
            }
        } else if buttons & IN_START != 0 {
            self.lb_audio_call(1);
            self.stages_save();
            if game_mode(self) == super::super::gm::machine::GM_MENU {
                self.menu_exit_to(super::super::gm::GM_VS);
                return;
            }
            self.rules_leave();
            return;
        }
        if buttons & 0xF != 0 {
            self.lb_audio_call(2);
            self.stages_move(&d, buttons);
        }
    }

    /// `mnStageSw_80236178`: the cursor shown at place `idx`.
    #[doc(alias = "mnStageSw_80236178")]
    fn stages_place_cursor(&mut self, d: &StageSwitchData, idx: u8) {
        let c = d.jobjs[X28];
        show(self, c);
        self.stages_move_cursor(d, idx);
    }

    fn stages_move_cursor(&mut self, d: &StageSwitchData, idx: u8) {
        let c = d.jobjs[X28];
        let reg = crate::fpu::reg;
        let y4 = ty(self, d.jobjs[X2C]);
        let delta = gekko_fp::fsubs(reg(ty(self, d.jobjs[X30])), reg(y4));
        let (x, row) = if idx < 15 {
            (tx(self, d.jobjs[X2C]), i32::from(idx))
        } else {
            (tx(self, d.jobjs[X34]), i32::from(idx) - 15)
        };
        set_tx(self, c, x);
        let y4 = ty(self, d.jobjs[X2C]);
        let y = gekko_fp::fmadds(delta, f64::from(row as f32), reg(y4));
        set_ty(self, c, crate::fpu::mem(y));
    }

    /// `mnStageSw_802364A0`: place `idx`'s cursor among its column's.
    #[doc(alias = "mnStageSw_802364A0")]
    fn stage_cursor(&self, d: &StageSwitchData, idx: u8) -> Option<JObjId> {
        let (mut cur, from) = if idx >= 15 {
            (child(self, Some(d.jobjs[X34].expect("a column"))), 15)
        } else {
            (child(self, Some(d.jobjs[X2C].expect("a column"))), 0)
        };
        for _ in from..idx {
            cur = next(self, Some(cur.expect("a stage's cursor")));
        }
        cur
    }

    /// `mnStageSw_80236548`: when the place changed, the old place's highlight hidden and the
    /// new one's shown at its frame, the cursor moved; when the value changed, the place's
    /// switch shown; then the place's highlight a frame on.
    #[doc(alias = "mnStageSw_80236548")]
    fn stages_update(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let d = self.stages_data(gobj);
        let h = self.game.menu.flow.hovered_selection as u8;
        if sel_changed {
            let j = self.stage_cursor(&d, d.hovered);
            let a = node(self, j, 3);
            hide(self, a);
            let f = frame_of(self, a);
            let j = self.stage_cursor(&d, h);
            let a = node(self, j, 3);
            show(self, a);
            set_frame_all(self, a, f);
            self.stages_place_cursor(&d, h);
        }
        if val_changed {
            let c = self.confirmed_read();
            let j = self.stage_cursor(&d, h);
            let s = node(self, j, 2);
            set_frame_all(self, s, SWITCH[usize::from(c)]);
        }
        let sel = if sel_changed { h } else { d.hovered };
        let j = self.stage_cursor(&d, sel);
        let a = node(self, j, 3);
        self.anim_loop(a.expect("a stage's highlight"), HOVER);
    }

    fn stages_way(state: u8) -> AnimLoop {
        match state {
            1 => WAYS[0],
            2 => WAYS[2],
            3 => WAYS[1],
            4 => WAYS[3],
            s => unreachable!("the stage switch in state {s} has no way in or out"),
        }
    }

    /// `fn_80236998`, the stage switch's process: out when the flow leaves it; its way in to
    /// its end (then the names, the columns shown, the other places' highlights hidden, the
    /// cursor placed and the inputs let in), or out (then the names and the screen gone); the
    /// places for what changed.
    #[doc(alias = "fn_80236998")]
    pub fn stage_switch_proc(&mut self, gobj: GObjId) {
        let d = self.stages_data(gobj);
        let flow = self.game.menu.flow;
        if matches!(d.state, 0 | 1 | 3) && d.menu_kind != flow.cur_menu {
            let s = if flow.entering_menu != 0 { 4 } else { 2 };
            self.stages_mut(gobj).state = s;
            req(self, d.jobjs[1], Self::stages_way(s).start);
            anim(self, d.jobjs[1]);
        }
        let s = self.stages_data(gobj).state;
        if s != 0 {
            let j = d.jobjs[1];
            if crate::fpu::reg(frame_of(self, j)) >= crate::fpu::reg(Self::stages_way(s).end) {
                match s {
                    1 | 3 => {
                        self.stages_mut(gobj).state = 0;
                        let addr = self.stages_addr(gobj);
                        self.stages_texts(addr);
                        show(self, d.jobjs[X2C]);
                        show(self, d.jobjs[X34]);
                        for i in 0..STAGES as u8 {
                            let c = self.stage_cursor(&d, i);
                            if i != d.hovered {
                                let a = node(self, c, 3);
                                hide(self, a);
                            }
                        }
                        self.stages_place_cursor(&d, d.hovered);
                        self.game.vs_menus.stages.coming_in = 0;
                        return;
                    }
                    _ => {
                        for t in self.stages_data(gobj).texts {
                            self.destroy_text(t.expect("a stage's name"));
                        }
                        gobj::free(self, gobj);
                        return;
                    }
                }
            }
            anim(self, j);
        }
        let d = self.stages_data(gobj);
        let h = self.game.menu.flow.hovered_selection;
        let mut sel_changed = false;
        let mut val_changed = false;
        if matches!(d.state, 0 | 1 | 3) {
            if u16::from(d.hovered) != h {
                sel_changed = true;
            }
            if Some(d.stages[usize::from(h)]) != self.confirmed() {
                val_changed = true;
            }
        }
        self.stages_update(gobj, sel_changed, val_changed);
        if sel_changed {
            self.stages_mut(gobj).hovered = h as u8;
        }
        if val_changed {
            let c = self.confirmed_read();
            self.stages_mut(gobj).stages[usize::from(h)] = c;
        }
    }

    /// `mnStageSw_80236CBC`: the stage switch, coming in as `state` says, on render link 6:
    /// its user data from the preferences (a locked place out), the flow's value the hovered
    /// place's, every place's cursor down its column (its switch, the hovered one's highlight,
    /// a locked one hidden), the first column hidden, the cursor placed and hidden.
    #[doc(alias = "mnStageSw_80236CBC")]
    fn stage_switch_screen(&mut self, state: i8) -> GObjId {
        let m = self.game.menu.models[MODEL_CON_SS];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.stages.gobj = Some(gobj);
        let j = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("the switch");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        crate::hsd::jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        set_frame_all(self, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0xB4);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::StageSwitch(addr));
        let f = self.game.menu.flow;
        let mut d = StageSwitchData {
            menu_kind: f.cur_menu,
            hovered: f.hovered_selection as u8,
            stages: [0; STAGES],
            state: state as u8,
            jobjs: [None; 6],
            texts: [None; STAGES],
        };
        for (i, &entry) in ORDER.iter().enumerate() {
            d.stages[i] = if self.place_unlocked(i) {
                u8::from(self.stage_enabled(entry))
            } else {
                0
            };
        }
        for i in 0..6 {
            d.jobjs[i] = node(self, Some(j), i as i32);
        }
        self.game.vs_menus.stages.data.insert(addr, d);
        self.set_confirmed(Some(d.stages[usize::from(d.hovered)]));
        let reg = crate::fpu::reg;
        let y4 = ty(self, d.jobjs[X2C]);
        let ysp = gekko_fp::fsubs(reg(ty(self, d.jobjs[X30])), reg(y4));
        let cm = self.game.menu.models[MODEL_CURSOR_SS];
        for i in 0..STAGES {
            let hovered = self.game.menu.flow.hovered_selection as u8;
            let c = load_model(self, cm);
            let s = node(self, Some(c), 2);
            set_frame_all(self, s, SWITCH[usize::from(d.stages[i])]);
            let a = node(self, Some(c), 3);
            if i as u8 == hovered {
                set_frame_all(self, a, HOVER.start);
            } else {
                hide(self, a);
            }
            if !self.place_unlocked(i) {
                hide(self, Some(c));
            }
            let (col, row) = if i < 15 {
                (d.jobjs[X2C], i)
            } else {
                (d.jobjs[X34], i - 15)
            };
            add_child(self, col, Some(c));
            add_ty_madd(self, Some(c), ysp, f64::from(row as f32));
        }
        hide(self, d.jobjs[X2C]);
        hide(self, d.jobjs[X34]);
        let c = d.jobjs[X28];
        show(self, c);
        self.stages_move_cursor(&d, d.hovered);
        hide(self, c);
        gobj
    }

    /// `mnStageSw_80237410`, from the extra rules: the flow on the stage switch at the first
    /// column's first unlocked place, the inputs held off while it comes in, the screen
    /// (already run) and its think.
    #[doc(alias = "mnStageSw_80237410")]
    pub(crate) fn stage_switch_open(&mut self) {
        let first = self.nearest_place(0);
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = STAGE_SWITCH;
        f.hovered_selection = first as u16;
        self.game.vs_menus.stages.coming_in = 1;
        let g = self.stage_switch_screen(1);
        self.game.world.mark_ran(g);
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }
}
