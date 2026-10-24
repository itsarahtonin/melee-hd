//! The extra rules (`melee/mn/mnruleplus.c`): six options in a list like the rules' (the
//! stock match's time limit with its clock, friendly fire, pause, the score display, the
//! sudden death penalty and the random stage switch, a screen of its own), the score display
//! and the switch shown once unlocked. A change is written into the game's rules
//! (`gmMainLib_GetGameRules`) as the list sees it; B and Start write them too.

use std::collections::BTreeMap;

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{
    add_child, al, anim, anim_all, frame_of, game_mode, hide, load_model, node, req_all, set_frame,
    set_frame_all, show,
};
use super::main::{IN_A, IN_BACK, IN_DOWN, IN_START, IN_UP};
use super::rules::{MODEL_CON_RL, MODEL_CURSOR_RL, MODEL_NM_RL, MODEL_TR01, RULES_PLUS};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes.
pub const THINK: u32 = 0x8023_201C;
pub const LIST_PROC: u32 = 0x8023_2F44;

const IN_LEFT: u32 = 1 << 2;
const IN_RIGHT: u32 = 1 << 3;

/// `mn_803EB6B0[15].selection_count`.
const OPTIONS: u8 = 6;

/// `mn_803ED1D0`: the list model's joint for each shown option, how many joints each value
/// model keeps, each option's label (unhovered, hovered), the cursors' loops.
const OPTION_NODES: [usize; 7] = [3, 4, 5, 6, 7, 8, 9];
const VALUE_JOINTS: [usize; 6] = [7, 2, 2, 2, 2, 0];
const fn label(i: u8, hovered: bool) -> f32 {
    (20 + 2 * i + hovered as u8) as f32
}
const HIGHLIGHT: AnimLoop = al(0.0, 99.0, 0.0);
const VALUE_ARROW: AnimLoop = al(200.0, 399.0, 200.0);
const OPTION_ANIMS: [AnimLoop; 2] = [al(100.0, 124.0, -0.1), al(125.0, 149.0, -0.1)];
const SWITCH_ANIMS: [AnimLoop; 2] = [al(150.0, 174.0, -0.1), al(175.0, 199.0, -0.1)];
const SUBMENU_ARROW: AnimLoop = al(0.0, 99.0, 60.0);
/// `mn_803ED270`: a value at rest, by value less one (the last, value 0).
const VALUE_AT: [AnimLoop; 3] = [
    al(0.0, 3.0, -0.1),
    al(20.0, 23.0, -0.1),
    al(40.0, 43.0, -0.1),
];
/// `mn_803ED294`: a value turning back (the first three), and the list's ways in and out.
const VALUE_BACK: [AnimLoop; 7] = [
    al(100.0, 103.0, -0.1),
    al(120.0, 123.0, -0.1),
    al(140.0, 143.0, -0.1),
    al(30.0, 49.0, -0.1),
    al(70.0, 89.0, -0.1),
    al(90.0, 109.0, -0.1),
    al(50.0, 69.0, -0.1),
];
/// `mn_803ED2E8`: each option's bounds and descriptions by value.
const BOUNDS: [[u8; 2]; 6] = [[0, 99], [0, 1], [0, 1], [0, 1], [0, 2], [0, 0]];
const DESCRIPTIONS: [[u8; 3]; 6] = [
    [0x37, 0, 0],
    [0x38, 0x39, 0],
    [0x3A, 0x3B, 0],
    [0x3D, 0x3C, 0],
    [0x3F, 0x3E, 0x40],
    [0x41, 0, 0],
];
/// `mnRulePlus_TimeLimitOffFrame`: the clock's frame with no time limit.
const CLOCK_OFF: f32 = 1.0;

/// The extra rules' list's user data (`MenuRulesPlusData`, 0xE0 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct RulePlusData {
    pub menu_kind: u8,
    pub hovered: u8,
    /// `rule_values`: the time limit, friendly fire, pause, the score display and the sudden
    /// death penalty, then `x7`, which nothing writes (`None`): hovering the random stage
    /// switch copies it into the flow's value, which nothing then reads.
    pub values: [Option<u8>; 6],
    /// `state` (`MenuState`).
    pub state: u8,
    /// `xC`: the list model's first ten joints.
    pub tree: [Option<JObjId>; 10],
    /// `x34`: each option's value model's joints, as many as it keeps.
    pub value_joints: [[Option<JObjId>; 7]; 6],
    pub description: Option<TextId>,
}

/// `mnruleplus.c`'s statics: the list (`mn_804D6BE0`) and the lists' user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RulePlus {
    pub list: Option<GObjId>,
    pub data: BTreeMap<u32, RulePlusData>,
}

impl Running<'_> {
    fn plus_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::RulePlusList(a)) => a,
            other => panic!("the extra rules' list holds {other:?}"),
        }
    }

    fn plus_data(&self, g: GObjId) -> RulePlusData {
        self.game.vs_menus.plus.data[&self.plus_addr(g)]
    }

    fn plus_mut(&mut self, g: GObjId) -> &mut RulePlusData {
        let a = self.plus_addr(g);
        self.game
            .vs_menus
            .plus
            .data
            .get_mut(&a)
            .expect("the extra rules' data")
    }

    /// The list's values into the game's rules (`mnRulePlus_SaveRules`), from `g`'s data.
    fn plus_save_from(&mut self, g: GObjId) {
        let d = self.plus_data(g);
        let v = |i: usize| d.values[i].expect("an extra rule's value");
        let r = &mut self.game.game_data.x1850;
        r.stock_time_limit = v(0);
        r.friendly_fire = v(1);
        r.pause = v(2);
        r.score_display = v(3);
        r.unk_xc = v(4);
    }

    fn plus_save(&mut self) {
        self.plus_save_from(self.game.vs_menus.plus.list.expect("the extra rules' list"));
    }

    /// `fn_8023201C`, the extra rules' think: A on the random stage switch opens it, Start
    /// goes on to the character select, B back to the rules (each writing the rules), up and
    /// down choose a shown option and left and right change its value.
    #[doc(alias = "fn_8023201C")]
    pub fn rule_plus_think(&mut self, gobj: GObjId) {
        let list = self.game.vs_menus.plus.list.expect("the extra rules' list");
        let d = self.plus_data(list);
        let buttons = self.list_inputs();
        let h = self.game.menu.flow.hovered_selection;
        if buttons & IN_A != 0 {
            if h == 5 {
                self.lb_audio_call(1);
                self.game.menu.flow.entering_menu = 1;
                self.plus_save();
                self.game.menu.cooldown = 5;
                self.stage_switch_open();
                gobj::free(self, gobj);
            }
        } else if buttons & IN_START != 0 {
            self.lb_audio_call(1);
            self.plus_save();
            if game_mode(self) == super::super::gm::machine::GM_MENU {
                self.menu_exit_to(super::super::gm::GM_VS);
                return;
            }
            self.rules_leave();
        } else if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.plus_save();
            self.game.menu.cooldown = 5;
            self.rules_back_from_sub();
            gobj::free(self, gobj);
        } else if buttons & (IN_UP | IN_DOWN) != 0 {
            self.lb_audio_call(2);
            loop {
                let f = &mut self.game.menu.flow;
                f.hovered_selection = if buttons & IN_UP != 0 {
                    if f.hovered_selection == 0 {
                        5
                    } else {
                        f.hovered_selection - 1
                    }
                } else if f.hovered_selection == 5 {
                    0
                } else {
                    f.hovered_selection + 1
                };
                let h = f.hovered_selection as u8;
                if self.rule_plus_shown(h) {
                    break;
                }
            }
            let h = self.game.menu.flow.hovered_selection;
            self.set_confirmed(d.values[usize::from(h)]);
        } else if h != 5 {
            let b = BOUNDS[usize::from(h)];
            if buttons & (IN_LEFT | IN_RIGHT) == 0 {
                return;
            }
            self.lb_audio_call(2);
            let c = self.confirmed_read();
            let c = if buttons & IN_LEFT != 0 {
                if c > b[0] { c - 1 } else { b[1] }
            } else if c < b[1] {
                c + 1
            } else {
                b[0]
            };
            self.set_confirmed(Some(c));
        }
    }

    /// `mn_80232458`: a value model's loop for `value`: at rest (`direction` 1) or turning
    /// back; none for the time limit and the switch.
    #[doc(alias = "mn_80232458")]
    fn plus_value_loop(option: u8, value: u8, direction: bool) -> AnimLoop {
        let count = BOUNDS[usize::from(option)][1];
        if direction {
            if value == 0 {
                VALUE_AT[usize::from(count)]
            } else {
                VALUE_AT[usize::from(value - 1)]
            }
        } else {
            VALUE_BACK[usize::from(count - value)]
        }
    }

    /// `mn_802324E4`: the time limit's clock: hidden digits and the clock off for none, else
    /// the minutes' two digits and zero seconds.
    #[doc(alias = "mn_802324E4")]
    fn plus_clock(&mut self, time: u8, addr: u32) {
        let j = self.game.vs_menus.plus.data[&addr].value_joints[0];
        if time == 0 {
            for i in [2, 3, 5, 6] {
                hide(self, j[i]);
            }
            set_frame_all(self, j[4], CLOCK_OFF);
            return;
        }
        for i in [2, 3, 5, 6] {
            show(self, j[i]);
        }
        // `mn_804D6BE4`, which nothing writes.
        set_frame_all(self, j[4], 0.0);
        set_frame_all(self, j[2], f32::from(time / 10));
        set_frame_all(self, j[3], f32::from(time % 10));
        set_frame_all(self, j[5], 0.0);
        set_frame_all(self, j[6], 0.0);
    }

    /// `mn_80232660`: a value model's turn a frame on, in whichever loop its frame is in.
    // The loops' tests are the original's (a NaN frame in none).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    #[doc(alias = "mn_80232660")]
    fn plus_value_anim(&mut self, j: Option<JObjId>, option: u8) {
        if !matches!(option, 1..=4) {
            return;
        }
        let reg = crate::fpu::reg;
        let f = reg(frame_of(self, j));
        let mut l = VALUE_AT[2];
        for i in 0..3 {
            l = VALUE_BACK[i];
            if reg(l.start) <= f && f <= reg(l.end) {
                break;
            }
            l = VALUE_AT[i];
            if reg(l.start) <= f && f <= reg(l.end) {
                break;
            }
        }
        self.anim_loop(j.expect("a value model"), l);
    }

    fn plus_parts(&self, root: Option<JObjId>) -> [Option<JObjId>; 17] {
        let idx: [i32; 17] = std::array::from_fn(|i| i as i32);
        super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, root, &idx)
    }

    /// `mn_802327A4`: the cursors as the rules' (`mn_80230274`): the old one dimmed and the
    /// new one lit, the changed value shown, then every shown cursor a frame on.
    #[doc(alias = "mn_802327A4")]
    fn plus_cursors(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let addr = self.plus_addr(gobj);
        let d = self.game.vs_menus.plus.data[&addr];
        let mut roots = [None; 6];
        for i in 0..OPTIONS {
            if self.rule_plus_shown(i) {
                let k = (0..i).filter(|&j| self.rule_plus_shown(j)).count();
                roots[usize::from(i)] = super::common::child(self, d.tree[OPTION_NODES[k]]);
            }
        }
        if sel_changed {
            let s = d.hovered;
            let p = self.plus_parts(roots[usize::from(s)]);
            hide(self, p[16]);
            hide(self, p[13]);
            let f = if s == 5 {
                SWITCH_ANIMS[0].start
            } else {
                OPTION_ANIMS[0].start
            };
            set_frame_all(self, p[2], f);
            set_frame(self, p[7], label(s, false));
            hide(self, p[8]);
            let n = self.game.menu.flow.hovered_selection as u8;
            let p = self.plus_parts(roots[usize::from(n)]);
            show(self, p[16]);
            req_all(self, p[16], HIGHLIGHT.start);
            if n != 5 {
                show(self, p[13]);
                req_all(self, p[13], VALUE_ARROW.start);
                set_frame_all(self, p[2], OPTION_ANIMS[1].start);
            } else {
                set_frame_all(self, p[2], SWITCH_ANIMS[1].start);
            }
            set_frame(self, p[7], label(n, true));
            if n == 5 {
                show(self, p[8]);
                set_frame_all(self, p[8], SUBMENU_ARROW.start);
            }
        }
        if val_changed {
            let h = self.game.menu.flow.hovered_selection as u8;
            let tree = d.value_joints[usize::from(h)][0];
            match h {
                0 => {
                    let c = self.confirmed_read();
                    self.plus_clock(c, addr);
                }
                1..=4 => {
                    let c = self.confirmed_read();
                    let back = self.game.menu.flow.buttons & u64::from(IN_LEFT) != 0;
                    let l = Self::plus_value_loop(h, c, !back);
                    set_frame_all(self, tree, l.start);
                }
                _ => {}
            }
        }
        let sel = if sel_changed {
            self.game.menu.flow.hovered_selection as u8
        } else {
            d.hovered
        };
        for i in 0..OPTIONS {
            if !self.rule_plus_shown(i) {
                continue;
            }
            let p = self.plus_parts(roots[usize::from(i)]);
            if i == sel {
                self.anim_loop(p[16].expect("a cursor's highlight"), HIGHLIGHT);
            }
            if i == sel && i != 5 {
                self.anim_loop(p[13].expect("a cursor's arrow"), VALUE_ARROW);
            }
            let l = if i == 5 {
                SWITCH_ANIMS[usize::from(i == sel)]
            } else {
                OPTION_ANIMS[usize::from(i == sel)]
            };
            self.anim_loop(p[2].expect("a cursor's plate"), l);
            self.anim_loop(p[8].expect("a cursor's sub-menu arrow"), SUBMENU_ARROW);
            self.plus_value_anim(d.value_joints[usize::from(i)][0], i);
        }
    }

    /// The hovered option's description: by value but for the time limit and the switch.
    fn plus_description(&mut self, addr: u32, sel: u8) {
        if let Some(t) = self.game.vs_menus.plus.data[&addr].description {
            self.destroy_text(t);
            self.game
                .vs_menus
                .plus
                .data
                .get_mut(&addr)
                .expect("the data")
                .description = None;
        }
        let id = if sel == 0 || sel == 5 {
            DESCRIPTIONS[usize::from(sel)][0]
        } else {
            DESCRIPTIONS[usize::from(sel)][usize::from(self.confirmed_read())]
        };
        let t = self.create_text(0, 1, Vec3::new(-9.5, 8.0, 17.0), 364.683_32, 76.775_44);
        self.game
            .vs_menus
            .plus
            .data
            .get_mut(&addr)
            .expect("the data")
            .description = Some(t);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, i32::from(id));
    }

    /// `mn_80232D4C`: the description gone while the list leaves, made once it has come in,
    /// and remade on a new option or a new value but the time limit's.
    #[doc(alias = "mn_80232D4C")]
    fn plus_description_update(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let addr = self.plus_addr(gobj);
        let d = self.game.vs_menus.plus.data[&addr];
        let sel = if sel_changed {
            self.game.menu.flow.hovered_selection as u8
        } else {
            d.hovered
        };
        match d.state {
            2 | 4 => {
                if let Some(t) = d.description {
                    self.destroy_text(t);
                    self.game
                        .vs_menus
                        .plus
                        .data
                        .get_mut(&addr)
                        .expect("the data")
                        .description = None;
                }
            }
            1 | 3 => {
                if d.description.is_none() {
                    self.plus_description(addr, sel);
                }
            }
            0 if (sel_changed || (val_changed && sel != 0 && sel != 5)) => {
                self.plus_description(addr, sel);
            }
            _ => {}
        }
    }

    fn plus_way(state: u8) -> AnimLoop {
        match state {
            1 => VALUE_BACK[3],
            2 => VALUE_BACK[5],
            3 => VALUE_BACK[4],
            4 => VALUE_BACK[6],
            s => unreachable!("the extra rules' list in state {s} has no way in or out"),
        }
    }

    /// `fn_80232F44`, the extra rules' list's process: as the rules' (`fn_802309F0`), out
    /// toward the stage switch when the flow enters it, else back.
    #[doc(alias = "fn_80232F44")]
    pub fn rule_plus_list_proc(&mut self, gobj: GObjId) {
        // The way out never leaves the list in a state that would mark everything changed.
        let menu_changed = false;
        let mut sel_changed = false;
        let mut val_changed = false;
        let d = self.plus_data(gobj);
        let cur = self.game.menu.flow.cur_menu;
        if matches!(d.state, 0 | 1 | 3) && d.menu_kind != cur {
            let s = if self.game.menu.flow.entering_menu != 0 {
                4
            } else {
                2
            };
            self.plus_mut(gobj).state = s;
            set_frame(self, d.tree[2], Self::plus_way(s).start);
        }
        let s = self.plus_data(gobj).state;
        if s != 0 {
            let j = d.tree[2];
            if crate::fpu::reg(frame_of(self, j)) >= crate::fpu::reg(Self::plus_way(s).end) {
                match s {
                    1 | 3 => self.plus_mut(gobj).state = 0,
                    _ => {
                        gobj::free(self, gobj);
                        return;
                    }
                }
            }
            anim(self, j);
        }
        let d = self.plus_data(gobj);
        let h = self.game.menu.flow.hovered_selection;
        if matches!(d.state, 0 | 1 | 3) {
            if u16::from(d.hovered) != h {
                sel_changed = true;
            }
            if d.values[usize::from(h)] != self.confirmed() {
                val_changed = true;
            }
        }
        self.plus_cursors(gobj, sel_changed, val_changed);
        self.plus_description_update(gobj, sel_changed, val_changed);
        if menu_changed {
            self.plus_mut(gobj).menu_kind = cur;
        }
        if sel_changed {
            self.plus_mut(gobj).hovered = h as u8;
        }
        if val_changed {
            let c = self.confirmed();
            let d = self.plus_mut(gobj);
            d.values[usize::from(d.hovered)] = c;
            self.plus_save_from(gobj);
        }
    }

    /// `mn_80233218`: the extra rules' list, coming in as `state` says: as the rules'
    /// (`mn_80230E38`), from the game's rules, each shown option's cursor and value model,
    /// and the hovered option's description.
    #[doc(alias = "mn_80233218")]
    fn rule_plus_list(&mut self, state: u8) -> GObjId {
        let selected = self.game.menu.flow.hovered_selection as u8;
        let models = self.game.menu.models;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.plus.list = Some(gobj);
        let root = crate::hsd::jobj::load_joint(&mut g.objs, data, models[MODEL_CON_RL].joint)
            .expect("the extra rules' list");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        let m = models[MODEL_CON_RL];
        crate::hsd::jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        set_frame_all(self, Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0xE0);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::RulePlusList(addr));
        let r = &self.game.game_data.x1850;
        let f = self.game.menu.flow;
        let mut d = RulePlusData {
            menu_kind: f.cur_menu,
            hovered: selected,
            values: [
                Some(r.stock_time_limit),
                Some(r.friendly_fire),
                Some(r.pause),
                Some(r.score_display),
                Some(r.unk_xc),
                None,
            ],
            state,
            tree: [None; 10],
            value_joints: [[None; 7]; 6],
            description: None,
        };
        for i in 0..10 {
            d.tree[i] = node(self, Some(root), i as i32);
        }
        self.game.vs_menus.plus.data.insert(addr, d);
        self.set_confirmed(d.values[usize::from(selected)]);
        if state != 0 {
            let l = match state {
                1 => VALUE_BACK[3],
                3 => VALUE_BACK[4],
                s => unreachable!("the extra rules' list made in state {s}"),
            };
            set_frame(self, d.tree[2], l.start);
        }
        let total = (0..6).filter(|&j| self.rule_plus_shown(j)).count();
        for i in 0..OPTIONS {
            let k = (0..i).filter(|&j| self.rule_plus_shown(j)).count();
            let option = d.tree[OPTION_NODES[k]];
            set_frame(self, option, total as f32);
            if !self.rule_plus_shown(i) {
                continue;
            }
            let cursor = load_model(self, models[MODEL_CURSOR_RL]);
            set_frame_all(self, Some(cursor), 0.0);
            let p = self.plus_parts(Some(cursor));
            set_frame(self, p[7], label(i, selected == i));
            if selected != i {
                hide(self, p[16]);
            }
            if selected != i || i == 5 {
                hide(self, p[13]);
            } else {
                req_all(self, p[13], VALUE_ARROW.start);
            }
            if selected != i || i != 5 {
                hide(self, p[8]);
            }
            let l = if i == 5 {
                SWITCH_ANIMS[usize::from(i == selected)]
            } else {
                OPTION_ANIMS[usize::from(i == selected)]
            };
            set_frame_all(self, p[2], l.start);
            add_child(self, option, Some(cursor));
            if i == 5 {
                continue;
            }
            let model = match i {
                0 => MODEL_TR01,
                1 => MODEL_TR01 + 1,
                2 | 3 => MODEL_TR01 + 2,
                _ => MODEL_TR01 + 3,
            };
            let value = load_model(self, models[model]);
            set_frame_all(self, Some(value), 0.0);
            for j in 0..VALUE_JOINTS[usize::from(i)] {
                let n = node(self, Some(value), j as i32);
                self.game
                    .vs_menus
                    .plus
                    .data
                    .get_mut(&addr)
                    .expect("the data")
                    .value_joints[usize::from(i)][j] = n;
            }
            if i == 0 {
                let vj = self.game.vs_menus.plus.data[&addr].value_joints[0];
                for idx in [2, 3, 5, 6] {
                    let t = load_model(self, models[MODEL_NM_RL]);
                    add_child(self, vj[idx], Some(t));
                }
                let t = d.values[0].expect("the time limit");
                self.plus_clock(t, addr);
            } else {
                let v = d.values[usize::from(i)].expect("an extra rule's value");
                let l = Self::plus_value_loop(i, v, true);
                req_all(self, Some(value), l.end);
                anim_all(self, Some(value));
            }
            add_child(self, option, Some(value));
        }
        self.plus_description(addr, selected);
        gobj
    }

    /// `mn_802339FC`, from the rules (or back from the stage switch): the flow on the extra
    /// rules at their first option (or the switch), the list coming in, and its think.
    #[doc(alias = "mn_802339FC")]
    pub(crate) fn rule_plus_open(&mut self) {
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = RULES_PLUS;
        let state = if f.prev_menu == super::rules::STAGE_SWITCH {
            f.hovered_selection = 5;
            3
        } else {
            f.hovered_selection = 0;
            1
        };
        self.rule_plus_list(state);
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }
}
