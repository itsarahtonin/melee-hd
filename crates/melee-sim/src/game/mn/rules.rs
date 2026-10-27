//! The VS rules (`melee/mn/mnmainrule.c`): seven options in a list, each a cursor with its
//! label and, for the five set here, a value model (the mode, the time limit or stock count
//! with its digits, the handicap, the damage ratio with its digits, the stage selection); the
//! item switch and the extra rules are screens of their own. Left and right change the
//! hovered value, written into the game's rules (`gmMainLib_GetGameRules`) as the list sees it
//! change; B and Start write them too. The screens' statics for the rules and names menus
//! (`mnmainrule.c` to `mnnamenew.c`) are [`RulesMenus`].

use std::collections::BTreeMap;

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{
    GM_TOURNAMENT, add_child, al, anim, anim_all, child, frame_of, game_mode, hide, load_model,
    node, req_all, set_frame, set_frame_all, show,
};
use super::main::{IN_A, IN_BACK, IN_DOWN, IN_START, IN_UP};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes.
pub const THINK: u32 = 0x8022_F538;
pub const LIST_PROC: u32 = 0x8023_09F0;

/// `MenuKind`s: the rules, the extra rules, the item switch, the names, the stage switch.
pub const RULES: u8 = 0xD;
pub const RULES_PLUS: u8 = 0xF;
pub const ITEM_SWITCH: u8 = 0x10;
pub const STAGE_SWITCH: u8 = 0x11;
pub const NAMES: u8 = 0x12;

const IN_LEFT: u32 = 1 << 2;
const IN_RIGHT: u32 = 1 << 3;

/// `mn_803EB6B0[13].selection_count`.
const OPTIONS: u8 = 7;

/// `mn_803EC600`: the list model's joint for each shown option.
const OPTION_NODES: [usize; 8] = [3, 4, 5, 6, 7, 8, 9, 0];
/// `rule_label_frames`: each option's label, unhovered and hovered.
const LABEL_FRAMES: [[f32; 2]; 7] = [
    [0.0, 1.0],
    [2.0, 3.0],
    [6.0, 7.0],
    [8.0, 9.0],
    [10.0, 11.0],
    [12.0, 13.0],
    [14.0, 15.0],
];
/// `mn_804D4B88`: the stock count's label (option 1 in a stock match).
const STOCK_LABEL: [f32; 2] = [4.0, 5.0];
const HIGHLIGHT: AnimLoop = al(0.0, 99.0, 0.0);
const VALUE_ARROW: AnimLoop = al(0.0, 199.0, 0.0);
const OPTION_ANIMS: [AnimLoop; 2] = [al(0.0, 24.0, -0.1), al(25.0, 49.0, -0.1)];
const SUBMENU_ANIMS: [AnimLoop; 2] = [al(50.0, 74.0, -0.1), al(75.0, 99.0, -0.1)];
const SUBMENU_ARROW: AnimLoop = al(0.0, 99.0, 60.0);
/// `mn_803EC734`: a value at rest, by value less one (the last, value 0).
const VALUE_AT: [AnimLoop; 5] = [
    al(0.0, 3.0, -0.1),
    al(20.0, 23.0, -0.1),
    al(40.0, 43.0, -0.1),
    al(60.0, 63.0, -0.1),
    al(80.0, 83.0, -0.1),
];
/// `mn_803EC770`: a value turning back (the first five), and the list's ways in and out.
const VALUE_BACK: [AnimLoop; 9] = [
    al(100.0, 103.0, -0.1),
    al(120.0, 123.0, -0.1),
    al(140.0, 143.0, -0.1),
    al(160.0, 163.0, -0.1),
    al(180.0, 183.0, -0.1),
    al(30.0, 49.0, -0.1),
    al(70.0, 89.0, -0.1),
    al(90.0, 109.0, -0.1),
    al(50.0, 69.0, -0.1),
];
/// `mn_803EC7DC`: each option's bounds; `mn_StockCountLimits`, the stock count's.
const LIMITS: [[u8; 2]; 8] = [
    [0, 3],
    [0, 0x63],
    [0, 2],
    [5, 0x14],
    [0, 4],
    [0, 0],
    [0, 0],
    [0, 0],
];
const STOCK_LIMITS: [u8; 2] = [1, 0x63];
/// `mn_803EC808`: how many joints each value model keeps.
const VALUE_JOINTS: [usize; 8] = [2, 9, 2, 4, 2, 0, 0, 0];
/// `mn_803EC818`: each option's descriptions, by value; `mn_StockCountTextId`.
const DESCRIPTIONS: [[u8; 5]; 7] = [
    [0x26, 0x27, 0x28, 0x29, 0x00],
    [0x2A, 0, 0, 0, 0],
    [0x2D, 0x2E, 0x2C, 0, 0],
    [0x2F, 0, 0, 0, 0],
    [0x30, 0x31, 0x32, 0x33, 0x34],
    [0x35, 0, 0, 0, 0],
    [0x36, 0, 0, 0, 0],
];
const STOCK_DESCRIPTION: u8 = 0x2B;
/// `mn_804D4B90`: the clock's frame with no time limit.
const CLOCK_OFF: f32 = 1.0;

/// The list's states (`MenuState`).
const IDLE: u8 = 0;

/// The models (`MenMainConRl_Top` and the rest) in `main::MODELS`'s order.
pub(crate) const MODEL_CON_RL: usize = 4;
pub(crate) const MODEL_CURSOR_RL: usize = 5;
pub(crate) const MODEL_NM_RL: usize = 6;
pub(crate) const MODEL_TR01: usize = 7;
const MODEL_RL01: usize = 11;
pub(crate) const MODEL_CON_IS: usize = 16;
pub(crate) const MODEL_CURSOR_IS: usize = 17;
pub(crate) const MODEL_CON_SS: usize = 18;
pub(crate) const MODEL_CURSOR_SS: usize = 19;

/// The rules' list's user data (`struct mn_802307F8_t`, 0x134 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct RulesData {
    /// `x0`: the list it shows; `x1`: the hovered option.
    pub menu_kind: u8,
    pub hovered: u8,
    /// `x2` to `x8` (`indexed.values`): the mode, the time limit, the handicap, the damage
    /// ratio and the stage selection, then two bytes nothing writes, which hold what the
    /// heap block held (`None`): hovering the item switch or the extra rules copies them into
    /// the flow's value, which nothing then reads (docs/engine.md, uninitialized values).
    pub values: [Option<u8>; 7],
    /// `x9`: the stock count.
    pub stock_count: u8,
    /// `xA`: the list's state (`MenuState`).
    pub state: u8,
    /// `xC`: the list model's first ten joints.
    pub tree: [Option<JObjId>; 10],
    /// `x34`: each option's value model's joints, as many as it keeps (`mn_803EC808`), the
    /// rest unwritten.
    pub value_joints: [[Option<JObjId>; 9]; 7],
    /// `x130`: the hovered option's description.
    pub text: Option<TextId>,
}

/// The rules' and names' screens' statics (`mnmainrule.c` to `mnnamenew.c`) and user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RulesMenus {
    /// `mn_804D6BD0`: the rules' list.
    pub list: Option<GObjId>,
    /// `mn_804D6BD4`: whether the handicap may be automatic (1 from the VS menu).
    pub auto_handicap: i32,
    /// The flow's value (`mn_804A04F0.confirmed_selection`) holds a byte of a list's heap
    /// block nothing wrote ([`RulesData::values`]); cleared by any write of it.
    pub confirmed_stale: bool,
    /// The lists' user data, by their blocks' addresses.
    pub data: BTreeMap<u32, RulesData>,
    /// The extra rules (`mnruleplus.c`).
    pub plus: super::rule_plus::RulePlus,
    /// The item switch (`mnitemsw.c`).
    pub items: super::item_switch::ItemSwitch,
    /// The random stage switch (`mnstagesw.c`).
    pub stages: super::stage_switch::StageSwitch,
    /// The names and the new name (`mnname.c`, `mnnamenew.c`).
    pub names: super::names::Names,
}

impl Running<'_> {
    /// The flow's value, none while it holds an unwritten heap byte.
    pub(crate) fn confirmed(&self) -> Option<u8> {
        (!self.game.vs_menus.confirmed_stale).then_some(self.game.menu.flow.confirmed_selection)
    }

    /// The flow's value, which the caller reads.
    pub(crate) fn confirmed_read(&self) -> u8 {
        assert!(
            !self.game.vs_menus.confirmed_stale,
            "a read of the flow's value, an unwritten heap byte (docs/engine.md)"
        );
        self.game.menu.flow.confirmed_selection
    }

    /// Writes the flow's value; a byte the original copies from an unwritten one stays stale.
    pub(crate) fn set_confirmed(&mut self, v: Option<u8>) {
        self.game.vs_menus.confirmed_stale = v.is_none();
        self.game.menu.flow.confirmed_selection = v.unwrap_or(0);
    }

    fn hovered(&self) -> u16 {
        self.game.menu.flow.hovered_selection
    }

    fn rules_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::RulesList(a)) => a,
            other => panic!("the rules' list holds {other:?}"),
        }
    }

    fn rules_data(&self, g: GObjId) -> RulesData {
        self.game.vs_menus.data[&self.rules_addr(g)]
    }

    fn rules_mut(&mut self, g: GObjId) -> &mut RulesData {
        let a = self.rules_addr(g);
        self.game
            .vs_menus
            .data
            .get_mut(&a)
            .expect("the rules' data")
    }

    /// The list's data, through `mn_804D6BD0`.
    fn rules_list_data(&self) -> RulesData {
        self.rules_data(self.game.vs_menus.list.expect("the rules' list"))
    }

    /// The list's values into the game's rules.
    fn rules_save(&mut self) {
        let d = self.rules_list_data();
        let v = |i: usize| d.values[i].expect("a rule's value");
        let r = &mut self.game.game_data.x1850;
        r.mode = v(0);
        r.time_limit = v(1);
        r.handicap = v(2);
        r.damage_ratio = v(3);
        r.stage_sel = v(4);
        r.stock_count = d.stock_count;
    }

    /// An option's value: the stock count for option 1 in a stock match, else its byte.
    fn rules_value(d: &RulesData, sel: u16, stock_mode: bool) -> Option<u8> {
        if sel == 1 && stock_mode {
            Some(d.stock_count)
        } else {
            d.values[usize::from(sel)]
        }
    }

    /// Whether an option is shown: all but the stage selection in Tournament mode.
    fn rules_shown(&self, i: u8) -> bool {
        !(game_mode(self) == GM_TOURNAMENT && i == 4)
    }

    fn rules_shown_before(&self, n: u8) -> usize {
        (0..n).filter(|&i| self.rules_shown(i)).count()
    }

    /// `fn_8022F538`, the rules' think: A on the item switch or the extra rules opens it,
    /// Start goes on to the character select, B back to the VS list (each writing the rules),
    /// up and down choose an option and left and right change its value.
    #[doc(alias = "fn_8022F538")]
    pub fn rules_think(&mut self, gobj: GObjId) {
        let buttons = self.list_inputs();
        if buttons & IN_A != 0 {
            let h = self.hovered();
            if h == 5 || h == 6 {
                self.lb_audio_call(1);
                self.game.menu.flow.entering_menu = 1;
                self.game.menu.cooldown = 5;
                if h == 5 {
                    self.item_switch_open();
                } else {
                    self.rule_plus_open();
                }
                gobj::free(self, gobj);
                self.rules_save();
                return;
            }
        } else if buttons & IN_START != 0 {
            self.lb_audio_call(1);
            self.rules_save();
            if game_mode(self) == super::super::gm::machine::GM_MENU {
                self.menu_exit_to(super::super::gm::GM_VS);
                return;
            }
            self.rules_leave();
            return;
        }
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.rules_save();
            self.rules_leave();
            return;
        }
        let d = self.rules_list_data();
        let stock_mode = d.values[0] == Some(1);
        if buttons & (IN_UP | IN_DOWN) != 0 {
            self.lb_audio_call(2);
            loop {
                let f = &mut self.game.menu.flow;
                f.hovered_selection = if buttons & IN_UP != 0 {
                    if f.hovered_selection == 0 {
                        6
                    } else {
                        f.hovered_selection - 1
                    }
                } else if f.hovered_selection == 6 {
                    0
                } else {
                    f.hovered_selection + 1
                };
                if self.rules_shown(self.hovered() as u8) {
                    break;
                }
            }
            let v = Self::rules_value(&d, self.hovered(), stock_mode);
            self.set_confirmed(v);
            return;
        }
        let h = self.hovered();
        if h == 5 || h == 6 {
            return;
        }
        let lim = if h == 1 && stock_mode {
            STOCK_LIMITS
        } else {
            LIMITS[usize::from(h)]
        };
        let tournament = game_mode(self) == GM_TOURNAMENT;
        let auto = self.game.vs_menus.auto_handicap;
        if buttons & (IN_LEFT | IN_RIGHT) == 0 {
            return;
        }
        self.lb_audio_call(2);
        let mut c = self.confirmed_read();
        if buttons & IN_LEFT != 0 {
            if c > lim[0] {
                c -= 1;
                if c == lim[0] && h == 1 {
                    self.game.delay_repeat(4, 0x19);
                }
            } else {
                c = lim[1];
            }
            if auto == 0 && h == 2 && c == 1 {
                c = 0;
            }
            if tournament && h == 1 && !stock_mode && c == lim[0] {
                c = lim[1];
            }
        } else {
            if c < lim[1] {
                c += 1;
                if c == lim[1] && h == 1 {
                    self.game.delay_repeat(4, 0x19);
                }
            } else {
                c = lim[0];
            }
            if auto == 0 && h == 2 && c == 1 {
                c = 2;
            }
            if tournament && h == 1 && !stock_mode && c == lim[0] {
                c = lim[0] + 1;
            }
        }
        self.set_confirmed(Some(c));
        if h == 2 {
            self.reset_handicaps();
            self.charsel_reset_handicaps_next();
        }
    }

    /// `mn_8022F4CC`: leaving the rules (or a screen under them) by B or Start: back to the
    /// VS list from the menu, or the texts and the menus' GObjs freed and back to
    /// Tournament's setup or the character select's screen (`mnCharSel_802640A0`).
    #[doc(alias = "mn_8022F4CC")]
    pub(crate) fn rules_leave(&mut self) {
        match game_mode(self) {
            GM_TOURNAMENT => {
                self.sis_reset();
                self.menu_free_gobjs();
                self.tou_back_from_rules();
            }
            super::super::gm::machine::GM_MENU => self.go_to_list(2, 3, 3),
            _ => {
                self.sis_reset();
                self.menu_free_gobjs();
                self.charsel_setup();
            }
        }
    }

    /// `gmMainLib_8015EA80`: every player's name tag and handicap back to their defaults in
    /// each VS mode (`gmMainLib_8015CDEC`'s name tags to 5, the handicaps to 9).
    #[doc(alias = "gmMainLib_8015EA80")]
    fn reset_handicaps(&mut self) {
        let m = &mut self.game.game_data.modes;
        m.nametags.fill(5);
        // `GmVsMode_Melee` to `SingleButton`, then `Tiny` to `Multiman` (Training's kept).
        for k in [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12] {
            for p in &mut m.table[k].start.players {
                p.handicap = 9;
            }
        }
    }

    /// `mn_8022FB88`: the time limit's clock: hidden digits and the clock off for none, else
    /// the minutes' two digits and zero seconds.
    #[doc(alias = "mn_8022FB88")]
    fn rules_clock(&mut self, time: u8, addr: u32) {
        let j = self.game.vs_menus.data[&addr].value_joints[1];
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
        // `mn_804D6BD8`, which nothing writes.
        set_frame_all(self, j[4], 0.0);
        set_frame_all(self, j[2], f32::from(time / 10));
        set_frame_all(self, j[3], f32::from(time % 10));
        set_frame_all(self, j[5], 0.0);
        set_frame_all(self, j[6], 0.0);
    }

    /// `mn_8022FD18`: option 1's value as the time limit's clock, or as the stock count's
    /// two digits.
    #[doc(alias = "mn_8022FD18")]
    fn rules_time_or_stock(&mut self, time: bool) {
        let addr = self.rules_addr(self.game.vs_menus.list.expect("the rules' list"));
        let d = self.game.vs_menus.data[&addr];
        let j = d.value_joints[1];
        if time {
            for i in [7, 8] {
                hide(self, j[i]);
            }
            for i in [2, 3, 4, 5, 6] {
                show(self, j[i]);
            }
            self.rules_clock(d.values[1].expect("the time limit"), addr);
            return;
        }
        for i in [7, 8] {
            show(self, j[i]);
        }
        for i in [2, 3, 4, 5, 6] {
            hide(self, j[i]);
        }
        set_frame_all(self, j[7], f32::from(d.stock_count / 10));
        set_frame_all(self, j[8], f32::from(d.stock_count % 10));
    }

    /// `mn_8022FEC8`: an option's value model shows `value`: the clock or stock digits, the
    /// ratio's digits, or the others' turning (back, with left).
    #[doc(alias = "mn_8022FEC8")]
    fn rules_value_shown(&mut self, gobj: GObjId, j: Option<JObjId>, kind: u8, value: u8) {
        let addr = self.rules_addr(gobj);
        let d = self.game.vs_menus.data[&addr];
        match kind {
            1 => {
                if self.rules_list_data().values[0] != Some(1) {
                    self.rules_clock(value, addr);
                    return;
                }
                let v = d.value_joints[1];
                set_frame_all(self, v[7], f32::from(value / 10));
                set_frame_all(self, v[8], f32::from(value % 10));
            }
            3 => {
                let v = d.value_joints[3];
                let tens = child(self, Some(v[2].expect("the ratio's digit")));
                set_frame_all(self, tens, f32::from(value / 10));
                let ones = child(self, Some(v[3].expect("the ratio's digit")));
                set_frame_all(self, ones, f32::from(value % 10));
            }
            0 | 2 | 4 => {
                let default = LIMITS[usize::from(kind)][1];
                let l = if self.game.menu.flow.buttons & u64::from(IN_LEFT) != 0 {
                    VALUE_BACK[usize::from(default - value)]
                } else if value == 0 {
                    VALUE_AT[usize::from(default)]
                } else {
                    VALUE_AT[usize::from(value - 1)]
                };
                req_all(self, j, l.start);
                anim_all(self, j);
            }
            _ => {}
        }
    }

    /// `mn_80230198`: a value model's turn a frame on, in whichever loop its frame is in.
    // The loops' tests are the original's (a NaN frame in none).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    #[doc(alias = "mn_80230198")]
    fn rules_value_anim(&mut self, j: Option<JObjId>, kind: u8) {
        if !matches!(kind, 0 | 2 | 4) {
            return;
        }
        let f = crate::fpu::reg(frame_of(self, j));
        let reg = crate::fpu::reg;
        let mut l = VALUE_AT[4];
        for i in 0..5 {
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

    /// The cursor parts of option `i`'s cursor (`lb_8001204C` with 0 to 16).
    fn rules_parts(&self, root: Option<JObjId>) -> [Option<JObjId>; 17] {
        let idx: [i32; 17] = std::array::from_fn(|i| i as i32);
        super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, root, &idx)
    }

    /// `mn_80230274`: when the hovered option changed, the old cursor dimmed and the new one
    /// lit; when the value changed, its model shown; then every shown cursor's highlight,
    /// arrow, plate and value a frame on, and option 1's label and value by the mode while the
    /// mode is hovered.
    #[doc(alias = "mn_80230274")]
    fn rules_cursors(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let addr = self.rules_addr(gobj);
        let d = self.game.vs_menus.data[&addr];
        let mut roots = [None; 8];
        for i in 0..OPTIONS {
            if self.rules_shown(i) {
                let k = self.rules_shown_before(i);
                roots[usize::from(i)] = child(self, d.tree[OPTION_NODES[k]]);
            }
        }
        let stock_mode = self.rules_list_data().values[0] == Some(1);
        let hovered = self.hovered() as u8;
        if sel_changed {
            let s = d.hovered;
            let p = self.rules_parts(roots[usize::from(s)]);
            hide(self, p[16]);
            hide(self, p[13]);
            let f = if s.wrapping_sub(5) <= 1 {
                SUBMENU_ANIMS[0].start
            } else {
                OPTION_ANIMS[0].start
            };
            set_frame_all(self, p[2], f);
            let f = if s == 1 && stock_mode {
                STOCK_LABEL[0]
            } else {
                LABEL_FRAMES[usize::from(s)][0]
            };
            set_frame(self, p[7], f);
            hide(self, p[8]);
            let p = self.rules_parts(roots[usize::from(hovered)]);
            show(self, p[16]);
            req_all(self, p[16], HIGHLIGHT.start);
            if hovered != 5 && hovered != 6 {
                show(self, p[13]);
                req_all(self, p[13], VALUE_ARROW.start);
                set_frame_all(self, p[2], OPTION_ANIMS[1].start);
            } else {
                set_frame_all(self, p[2], SUBMENU_ANIMS[1].start);
            }
            let f = if hovered == 1 && stock_mode {
                STOCK_LABEL[1]
            } else {
                LABEL_FRAMES[usize::from(hovered)][1]
            };
            set_frame(self, p[7], f);
            if hovered.wrapping_sub(5) <= 1 {
                show(self, p[8]);
                set_frame_all(self, p[8], SUBMENU_ARROW.start);
            }
        }
        if val_changed {
            let j = d.value_joints[usize::from(hovered)][0];
            let c = self.confirmed_read();
            self.rules_value_shown(gobj, j, hovered, c);
        }
        let focus = if sel_changed { hovered } else { d.hovered };
        for i in 0..OPTIONS {
            if !self.rules_shown(i) {
                continue;
            }
            let p = self.rules_parts(roots[usize::from(i)]);
            if i == focus {
                self.anim_loop(p[16].expect("a cursor's highlight"), HIGHLIGHT);
            }
            if i == focus && i != 5 && i != 6 {
                self.anim_loop(p[13].expect("a cursor's arrow"), VALUE_ARROW);
            }
            let l = if i.wrapping_sub(5) <= 1 {
                SUBMENU_ANIMS[usize::from(i == focus)]
            } else {
                OPTION_ANIMS[usize::from(i == focus)]
            };
            self.anim_loop(p[2].expect("a cursor's plate"), l);
            self.anim_loop(p[8].expect("a cursor's sub-menu arrow"), SUBMENU_ARROW);
            self.rules_value_anim(d.value_joints[usize::from(i)][0], i);
            if i == 1 && val_changed && focus == 0 {
                let c = self.confirmed_read();
                let f = if c == 1 {
                    STOCK_LABEL[0]
                } else {
                    LABEL_FRAMES[1][0]
                };
                set_frame(self, p[7], f);
                self.rules_time_or_stock(c != 1);
            }
        }
    }

    /// `mn_802307F8`: the hovered option's description for its value (one for the stock
    /// count, one each for the time limit, the ratio and the two screens), the old one freed.
    #[doc(alias = "mn_802307F8")]
    fn rules_description(&mut self, addr: u32, kind: u8, value: Option<u8>) {
        if let Some(t) = self.game.vs_menus.data[&addr].text {
            self.destroy_text(t);
            self.game
                .vs_menus
                .data
                .get_mut(&addr)
                .expect("the rules' data")
                .text = None;
        }
        let d = self.game.vs_menus.data[&addr];
        let id = if kind == 1 && d.values[0] == Some(1) {
            STOCK_DESCRIPTION
        } else if matches!(kind, 1 | 3 | 5 | 6) {
            DESCRIPTIONS[usize::from(kind)][0]
        } else {
            let v = value.expect("a read of the flow's value, an unwritten heap byte");
            DESCRIPTIONS[usize::from(kind)][usize::from(v)]
        };
        let t = self.create_text(0, 1, Vec3::new(-9.5, 8.0, 17.0), 364.683_32, 76.775_44);
        self.game
            .vs_menus
            .data
            .get_mut(&addr)
            .expect("the rules' data")
            .text = Some(t);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, i32::from(id));
    }

    /// `mn_802308F0`: the description gone while the list leaves, made once it has come in,
    /// and remade on a new option or a new value of the mode, handicap or stage selection.
    #[doc(alias = "mn_802308F0")]
    fn rules_description_update(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let addr = self.rules_addr(gobj);
        let d = self.game.vs_menus.data[&addr];
        let kind = if sel_changed {
            self.hovered() as u8
        } else {
            d.hovered
        };
        match d.state {
            2 | 4 => {
                if let Some(t) = d.text {
                    self.destroy_text(t);
                    self.game
                        .vs_menus
                        .data
                        .get_mut(&addr)
                        .expect("the rules' data")
                        .text = None;
                }
            }
            1 | 3 => {
                if d.text.is_none() {
                    self.rules_description(addr, kind, self.confirmed());
                }
            }
            0 if (sel_changed || (val_changed && matches!(kind, 0 | 2 | 4))) => {
                self.rules_description(addr, kind, self.confirmed());
            }
            _ => {}
        }
    }

    /// The list's way in or out for its state (`mn_803EC770[5]` to `[8]`).
    fn rules_way(state: u8) -> AnimLoop {
        match state {
            1 => VALUE_BACK[5],
            2 => VALUE_BACK[7],
            3 => VALUE_BACK[6],
            4 => VALUE_BACK[8],
            s => unreachable!("the rules' list in state {s} has no way in or out"),
        }
    }

    /// `fn_802309F0`, the rules' list's process: when the flow leaves it, its way out (toward
    /// a screen under it or back); its way in or out to its end (then idle, or gone); the
    /// cursors and description for what changed; the new option and value kept, and the
    /// values written into the game's rules.
    #[doc(alias = "fn_802309F0")]
    pub fn rules_list_proc(&mut self, gobj: GObjId) {
        let mut menu_changed = false;
        let mut sel_changed = false;
        let mut val_changed = false;
        let d = self.rules_data(gobj);
        let cur = self.game.menu.flow.cur_menu;
        if matches!(d.state, 0 | 1 | 3) && d.menu_kind != cur {
            let s = if cur.wrapping_sub(0xF) <= 1 { 4 } else { 2 };
            self.rules_mut(gobj).state = s;
            set_frame(self, d.tree[2], Self::rules_way(s).start);
            if matches!(s, 0 | 1 | 3) {
                menu_changed = true;
                sel_changed = true;
                val_changed = true;
            }
        }
        let s = self.rules_data(gobj).state;
        if s != 0 {
            let j = d.tree[2];
            if crate::fpu::reg(frame_of(self, j)) >= crate::fpu::reg(Self::rules_way(s).end) {
                match s {
                    1 | 3 => self.rules_mut(gobj).state = IDLE,
                    _ => {
                        gobj::free(self, gobj);
                        return;
                    }
                }
            }
            anim(self, j);
        }
        let d = self.rules_data(gobj);
        let h = self.hovered();
        if matches!(d.state, 0 | 1 | 3) {
            sel_changed = u16::from(d.hovered) != h;
            let stock_mode = d.values[0] == Some(1);
            if Self::rules_value(&d, h, stock_mode) != self.confirmed() {
                val_changed = true;
            }
        }
        self.rules_cursors(gobj, sel_changed, val_changed);
        self.rules_description_update(gobj, sel_changed, val_changed);
        if menu_changed {
            self.rules_mut(gobj).menu_kind = cur;
        }
        if sel_changed {
            self.rules_mut(gobj).hovered = h as u8;
        }
        if val_changed {
            let c = self.confirmed();
            let d = self.rules_mut(gobj);
            if d.hovered == 1 && d.values[0] == Some(1) {
                d.stock_count = c.expect("a stock count");
            } else {
                d.values[usize::from(d.hovered)] = c;
            }
            let d = self.rules_data(gobj);
            let v = |i: usize| d.values[i].expect("a rule's value");
            let r = &mut self.game.game_data.x1850;
            r.mode = v(0);
            r.time_limit = v(1);
            r.handicap = v(2);
            r.damage_ratio = v(3);
            r.stage_sel = v(4);
            r.stock_count = d.stock_count;
        }
    }

    /// `mn_80230E38`: the rules' list, coming in as `state` says: its model on render link 4
    /// with its process, its user data from the game's rules (`mn_80230D18`; no automatic
    /// handicap unless the menu allows it, no unlimited time in Tournament mode), the flow's
    /// value the hovered option's, a cursor with its label on each shown option, the five
    /// values' models with their digits, and the hovered option's description.
    #[doc(alias = "mn_80230E38")]
    #[doc(alias = "mn_80230D18")]
    #[doc(alias = "mn_80231634")]
    fn rules_list(&mut self, state: u8) -> GObjId {
        let selected = self.hovered() as u8;
        let models = self.game.menu.models;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.list = Some(gobj);
        let data = self.data;
        let root = crate::hsd::jobj::load_joint(&mut g.objs, data, models[MODEL_CON_RL].joint)
            .expect("the rules' list");
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
        let addr = crate::hsd::heap::mem_alloc(0x134);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::RulesList(addr));
        let f = self.game.menu.flow;
        let r = &self.game.game_data.x1850;
        let mut handicap = r.handicap;
        if self.game.vs_menus.auto_handicap == 0 && handicap == 1 {
            handicap = 0;
        }
        let mut time_limit = r.time_limit;
        if game_mode(self) == GM_TOURNAMENT && time_limit == 0 {
            time_limit = 0x63;
        }
        let mut d = RulesData {
            menu_kind: f.cur_menu,
            hovered: f.hovered_selection as u8,
            values: [
                Some(r.mode),
                Some(time_limit),
                Some(handicap),
                Some(r.damage_ratio),
                Some(r.stage_sel),
                None,
                None,
            ],
            stock_count: r.stock_count,
            state,
            tree: [None; 10],
            value_joints: [[None; 9]; 7],
            text: None,
        };
        for i in 0..10 {
            d.tree[i] = node(self, Some(root), i as i32);
        }
        self.game.vs_menus.data.insert(addr, d);
        let stock_mode = d.values[0] == Some(1);
        let v = Self::rules_value(&d, u16::from(d.hovered), stock_mode);
        self.set_confirmed(v);
        if state != 0 {
            let l = match state {
                1 => VALUE_BACK[5],
                3 => VALUE_BACK[6],
                s => unreachable!("the rules' list made in state {s}"),
            };
            set_frame(self, d.tree[2], l.start);
        }
        let shown_total = self.rules_shown_before(7);
        for i in 0..OPTIONS {
            let k = self.rules_shown_before(i);
            let option = d.tree[OPTION_NODES[k]];
            set_frame(self, option, shown_total as f32);
            if !self.rules_shown(i) {
                continue;
            }
            let cursor = load_model(self, models[MODEL_CURSOR_RL]);
            set_frame_all(self, Some(cursor), 0.0);
            let p = self.rules_parts(Some(cursor));
            let hov = usize::from(selected == i);
            let f = if i == 1 && stock_mode {
                STOCK_LABEL[hov]
            } else {
                LABEL_FRAMES[usize::from(i)][hov]
            };
            set_frame(self, p[7], f);
            if selected != i {
                hide(self, p[16]);
            }
            if selected != i || i.wrapping_sub(5) <= 1 {
                hide(self, p[13]);
            }
            let l = if i.wrapping_sub(5) <= 1 {
                SUBMENU_ANIMS[hov]
            } else {
                OPTION_ANIMS[hov]
            };
            set_frame_all(self, p[2], l.start);
            if selected != i || (i != 5 && i != 6) {
                hide(self, p[8]);
            }
            add_child(self, option, Some(cursor));
            if i == 5 || i == 6 {
                continue;
            }
            let value = load_model(self, models[MODEL_RL01 + usize::from(i)]);
            set_frame_all(self, Some(value), 0.0);
            for j in 0..VALUE_JOINTS[usize::from(i)] {
                let n = node(self, Some(value), j as i32);
                self.game
                    .vs_menus
                    .data
                    .get_mut(&addr)
                    .expect("the rules' data")
                    .value_joints[usize::from(i)][j] = n;
            }
            let vj = self.game.vs_menus.data[&addr].value_joints[usize::from(i)];
            match i {
                1 => {
                    for idx in [2, 3, 5, 6, 7, 8] {
                        let t = load_model(self, models[MODEL_NM_RL]);
                        add_child(self, vj[idx], Some(t));
                    }
                    let mode = self.rules_list_data().values[0];
                    self.rules_time_or_stock(mode != Some(1));
                }
                3 => {
                    for idx in [2, 3] {
                        let t = load_model(self, models[MODEL_NM_RL]);
                        add_child(self, vj[idx], Some(t));
                    }
                    let ratio = d.values[3].expect("the damage ratio");
                    let tens = child(self, vj[2]);
                    set_frame_all(self, tens, f32::from(ratio / 10));
                    let ones = child(self, vj[3]);
                    set_frame_all(self, ones, f32::from(ratio % 10));
                }
                _ => {
                    let value = d.values[usize::from(i)].expect("a rule's value");
                    let default = LIMITS[usize::from(i)][1];
                    let l = if value == 0 {
                        VALUE_AT[usize::from(default)]
                    } else {
                        VALUE_AT[usize::from(value - 1)]
                    };
                    set_frame_all(self, vj[0], l.end);
                }
            }
            add_child(self, option, Some(value));
        }
        let c = self.confirmed();
        self.rules_description(addr, d.hovered, c);
        gobj
    }

    /// `mn_8023164C`, back from the item switch or the extra rules: the flow on the rules
    /// (at the screen's option), their think, and the list coming back in (already run).
    #[doc(alias = "mn_8023164C")]
    pub(crate) fn rules_back_from_sub(&mut self) {
        let state = self.rules_enter_flow();
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
        let g = self.rules_list(state);
        self.game.world.mark_ran(g);
    }

    /// The flow on the rules, at the option of the screen it comes from.
    fn rules_enter_flow(&mut self) -> u8 {
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = RULES;
        match f.prev_menu {
            RULES_PLUS => {
                f.hovered_selection = 6;
                3
            }
            ITEM_SWITCH => {
                f.hovered_selection = 5;
                3
            }
            _ => {
                f.hovered_selection = 0;
                1
            }
        }
    }

    /// `mn_80231804` (`mn_802317E4`), from Tournament's setup or the character select: the
    /// inputs' cooldown, the flow on the rules, the texts reset, the menus' scene from the
    /// screen's `MnExtAll` (all its parts but the main lists'), the automatic handicap
    /// allowed or not, the think and the rules coming in, and the menus' song.
    #[doc(alias = "mn_80231804")]
    #[doc(alias = "mn_802317E4")]
    pub(crate) fn rules_from_screen(&mut self, archive: melee_data::archive::ArchiveId, auto: i32) {
        let m = &mut self.game.menu;
        m.cooldown = 0x14;
        m.input_x2 = 0;
        m.input_x4 = 0;
        let f = &mut m.flow;
        f.buttons = 0;
        f.prev_menu = f.cur_menu;
        f.cur_menu = RULES;
        f.hovered_selection = 0;
        self.sis_reset();
        let parts: Vec<usize> = (0..20).filter(|&i| i != 2 && i != 3).collect();
        self.game.vs_menus.auto_handicap = auto;
        self.menu_from_archive(archive, &parts);
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
        let g = self.rules_list(1);
        self.game.world.mark_ran(g);
        let bgm = self.game.game_data.x1850.bgm;
        self.lb_play_music(i32::from(bgm));
    }

    /// `mn_80231714`, from the VS list: the rules, as [`Self::rules_back_from_sub`] opens
    /// them, the automatic handicap allowed.
    #[doc(alias = "mn_80231714")]
    pub fn rules_open(&mut self) {
        self.rules_back_from_sub();
        self.game.vs_menus.auto_handicap = 1;
    }

    /// `mn_80231F80`: whether an extra rule is shown: all but the stock time limit in
    /// Tournament mode, the score display and the random stage switch once unlocked
    /// (`gmMainLib_8015EE0C`, `gmMainLib_8015EE44`).
    #[doc(alias = "mn_80231F80")]
    #[doc(alias = "gmMainLib_8015EE0C")]
    #[doc(alias = "gmMainLib_8015EE44")]
    pub(crate) fn rule_plus_shown(&self, i: u8) -> bool {
        let x186c = self.game.game_data.thing.save_data.x186C;
        if game_mode(self) == GM_TOURNAMENT && i == 1 {
            return false;
        }
        match i {
            3 => x186c & 1 != 0,
            5 => x186c & 2 != 0,
            _ => true,
        }
    }

    /// Runs the rules' and names' screens' processes.
    pub fn run_rules_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            THINK => self.rules_think(gobj),
            LIST_PROC => self.rules_list_proc(gobj),
            super::rule_plus::THINK => self.rule_plus_think(gobj),
            super::rule_plus::LIST_PROC => self.rule_plus_list_proc(gobj),
            super::item_switch::THINK => self.item_switch_think(gobj),
            super::item_switch::LIST_PROC => self.item_switch_proc(gobj),
            super::stage_switch::THINK => self.stage_switch_think(gobj),
            super::stage_switch::LIST_PROC => self.stage_switch_proc(gobj),
            _ => return self.run_names_proc(gobj, f),
        }
        true
    }

    /// Releases a rules' or names' screen's user data (`HSD_Free`, or `fn_8023D0F8` for a
    /// glyph's variants, which frees their text first).
    pub(crate) fn rules_release(&mut self, data: UserData) -> bool {
        let r = &mut self.game.vs_menus;
        let addr = match data {
            UserData::RulesList(a) => {
                r.data.remove(&a);
                a
            }
            UserData::RulePlusList(a) => {
                r.plus.data.remove(&a);
                a
            }
            UserData::ItemSwitch(a) => {
                r.items.data.remove(&a);
                a
            }
            UserData::StageSwitch(a) => {
                r.stages.data.remove(&a);
                a
            }
            UserData::NameList(a) => {
                r.names.lists.remove(&a);
                a
            }
            UserData::NameNew(a) => {
                r.names.entries.remove(&a);
                a
            }
            UserData::GlyphVariants(a) => {
                let v = r.names.variants.remove(&a).expect("a glyph's variants");
                if let Some(t) = v.text {
                    self.destroy_text(t);
                }
                a
            }
            _ => return false,
        };
        crate::hsd::heap::mem_free(addr);
        true
    }
}
