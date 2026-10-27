//! The VS records' rankings (`melee/mn/mndiagram3.c`): the statistics listed ten at a time,
//! and the five characters or names that lead the hovered one; up and down move the cursor,
//! scrolling at an edge, X or Y switch between characters and names, L and R turn to the
//! statistics and the grid, B leaves for the Records list.

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::diagram::{
    IN_L, IN_R, IN_XY, SCREEN, format_decimal, format_time, int_to_str, name_format,
};
use super::diagram2::{is_distance, is_icon_only, is_percentage, is_time};
use super::main::{IN_BACK, IN_DOWN, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input, the page's, and its going out.
pub const INPUT: u32 = 0x8024_61BC;
pub const THINK: u32 = 0x8024_6E64;
pub const OUT: u32 = 0x8024_6E04;

/// The Records list's `MenuKind`.
const RECORDS: u8 = 0x1C;

/// `mnDiagram3_803EEC10` and `mnDiagram3_803EEC1C`: the page going out, and the arrows' loop.
const OUT_ANIM: AnimLoop = al(10.0, 19.0, -0.1);
const ARROW: AnimLoop = al(0.0, 199.0, 0.0);

/// `mnDiagram3_803EEC28`: where the labels, the ranked names and the values sit.
const TITLE_POS: Vec3 = Vec3::new(3.3, 0.5, 0.0);
const RANK_POS: Vec3 = Vec3::new(-2.0, 0.57, 0.0);
const VALUE_POS: Vec3 = Vec3::new(8.0, 0.57, 0.0);

/// `mnDiagram3_803EEC4C`: each statistic's label and its unit's glyph (0xFFFF none).
const LABELS: [u16; 24] = [
    0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x71,
    0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
];
const UNITS: [u16; 24] = [
    0x7A, 0x7A, 0x7A, 0x7C, 0x7C, 0x7C, 0x7C, 0x7C, 0x7A, 0x7A, 0x7A, 0xFFFF, 0x7C, 0x7B, 0x7E,
    0x7E, 0x7E, 0x7E, 0x7D, 0x7D, 0x7D, 0x7B, 0x7B, 0x7B,
];

/// `mn_804D4B64`: the values' colour.
const VALUE_COLOR: [u8; 4] = [0xFF, 0xC8, 0x00, 0xFF];

/// `Diagram3` (0x78 bytes of the heap): the flow it came from, the cursor's row, the first
/// statistic shown, whether its intro plays, which it ranks, the model's first ten joints in
/// pre-order, each row's label, the ranked names, the values, the units, and the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Diagram3Data {
    pub saved_menu: u8,
    pub cursor_row: u8,
    pub scroll: u8,
    pub anim_state: u8,
    pub name_mode: u8,
    pub jobjs: [Option<JObjId>; 10],
    pub labels: [Option<TextId>; 10],
    pub title: Option<TextId>,
    pub value: Option<TextId>,
    pub units: [Option<TextId>; 5],
    pub popup: Option<GObjId>,
}

/// `mndiagram3.c`'s statics: the page's GObj (`mnDiagram3_804D6C20`) and the user data by
/// block.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Diagram3 {
    pub gobj: Option<GObjId>,
    pub data: BTreeMap<u32, Diagram3Data>,
}

impl Running<'_> {
    fn d3_gobj(&self) -> GObjId {
        self.game
            .data_menus
            .diagram3
            .gobj
            .expect("the VS records' rankings")
    }

    fn d3_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Diagram3(a)) => a,
            other => panic!("the VS records' rankings hold {other:?}"),
        }
    }

    fn d3(&self, gobj: GObjId) -> Diagram3Data {
        self.game.data_menus.diagram3.data[&self.d3_addr(gobj)]
    }

    fn d3_mut(&mut self, gobj: GObjId) -> &mut Diagram3Data {
        let a = self.d3_addr(gobj);
        self.game
            .data_menus
            .diagram3
            .data
            .get_mut(&a)
            .expect("the VS records' rankings' data")
    }

    /// The statistic `scroll` and `row` name, wrapping (`mnDiagram3_GetRowStat`).
    fn d3_stat(d: &Diagram3Data, scroll: u8, row: u8) -> u8 {
        let limit = if d.name_mode != 0 { 24 } else { 21 };
        let v = i32::from(scroll) + i32::from(row);
        if v >= limit {
            (v - limit) as u8
        } else {
            v as u8
        }
    }

    /// `mnDiagram2_ClearDetailView`: the rankings' texts freed, their icons dropped.
    #[doc(alias = "mnDiagram2_ClearDetailView")]
    fn d3_clear(&mut self, gobj: GObjId) {
        let d = self.d3(gobj);
        if let Some(t) = d.title {
            self.destroy_text(t);
            self.d3_mut(gobj).title = None;
        }
        for i in 0..5 {
            if let Some(t) = d.units[i] {
                self.destroy_text(t);
                self.d3_mut(gobj).units[i] = None;
            }
        }
        if let Some(t) = d.value {
            self.destroy_text(t);
            self.d3_mut(gobj).value = None;
        }
        let c = common::child(self, d.jobjs[6]);
        if c.is_some() {
            jobj::remove_all(&mut self.game.objs, c);
        }
    }

    /// Every row's label freed (`mnDiagram3_ClearRowLabels`).
    fn d3_clear_labels(&mut self, gobj: GObjId) {
        for i in 0..10 {
            if let Some(t) = self.d3(gobj).labels[i] {
                self.destroy_text(t);
                self.d3_mut(gobj).labels[i] = None;
            }
        }
    }

    /// Each row's label from the first statistic shown (`mnDiagram3_RebuildRowLabels`,
    /// `mnDiagram3_SetupRows`).
    fn d3_labels(&mut self, gobj: GObjId) {
        let d = self.d3(gobj);
        let ty = |r: &Self, k: usize| reg(common::ty(r, d.jobjs[k]));
        let spacing = fp::fsubs(ty(self, 9), ty(self, 8));
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[8],
            Some(TITLE_POS),
        );
        for i in 0..10u8 {
            let x = mem(fp::fsubs(reg(p.x), reg(6.5)));
            let y = mem(fp::fmadds(
                fp::fneg(spacing),
                fp::frsp(f64::from(i)),
                fp::fneg(reg(p.y)),
            ));
            let t = self.create_text(0, 1, Vec3::new(x, y, p.z), 6.5, 240.0);
            self.d3_mut(gobj).labels[usize::from(i)] = Some(t);
            let s = Self::d3_stat(&d, d.scroll, i);
            self.game
                .sis
                .reset_text(self.data, t, i32::from(LABELS[usize::from(s)]));
        }
    }

    /// `mnDiagram3_PopulateRankings`: the hovered statistic's five leaders: characters' icons
    /// or names, their values (or for the most and least played, how many names have them),
    /// and the units.
    #[doc(alias = "mnDiagram3_PopulateRankings")]
    fn d3_rankings(&mut self, gobj: GObjId) {
        let d = self.d3(gobj);
        let s = Self::d3_stat(&d, d.scroll, d.cursor_row);
        let names = d.name_mode != 0;
        let ty = |r: &Self, k: usize| reg(common::ty(r, d.jobjs[k]));
        let divider = ty(self, 6);
        let spacing = fp::fsubs(ty(self, 7), divider);
        let neg = fp::fneg(spacing);
        let mut title: Option<TextId> = None;
        let mut value: Option<TextId> = None;
        let mut entity = 0u8;
        let row_y = |i: i32| fp::fdivs(fp::fmuls(neg, fp::frsp(f64::from(i))), reg(0.035));
        for i in 0..5i32 {
            if names {
                if !is_icon_only(s) {
                    if i == 0 {
                        let p = crate::lb::joint::world_position(
                            &mut self.game.objs,
                            self.data,
                            d.jobjs[6],
                            Some(RANK_POS),
                        );
                        let t = self.create_text_buffer(0, 1);
                        self.d3_mut(gobj).title = Some(t);
                        let x = self.game.sis.text_mut(t);
                        x.font_size = [0.035, 0.035];
                        x.pos = Vec3::new(p.x, -p.y, p.z);
                        title = Some(t);
                    }
                    entity = self.ranked_name(s, i as u8);
                    if entity == 0x78 {
                        continue;
                    }
                    let n = self.name_of(entity);
                    let t = title.expect("the ranked names' text");
                    self.game
                        .sis
                        .add_string(t, 0.0, row_y(i), Some(name_format(&n)));
                }
            } else {
                entity = self.ranked_fighter(s, i as u8);
                let icon = self.diagram_icon(i32::from(entity), false);
                common::set_ty(
                    self,
                    Some(icon),
                    mem(fp::fmuls(spacing, fp::frsp(f64::from(i)))),
                );
                common::add_child(self, d.jobjs[6], Some(icon));
            }
            if i == 0 {
                let p = crate::lb::joint::world_position(
                    &mut self.game.objs,
                    self.data,
                    d.jobjs[6],
                    Some(VALUE_POS),
                );
                let t = self.create_text_buffer(0, 1);
                self.d3_mut(gobj).value = Some(t);
                let x = self.game.sis.text_mut(t);
                x.font_size = [0.035, 0.035];
                x.pos = Vec3::new(p.x, -p.y, p.z);
                x.default_alignment = 2;
                x.text_color = VALUE_COLOR;
                value = Some(t);
            }
            let vt = value.expect("the rankings' values' text");
            if is_icon_only(s) && self.aggregated_rank(s, i as u8).0 != 25 {
                let (k, _) = self.aggregated_rank(s, i as u8);
                let icon = self.diagram_icon(i32::from(k), false);
                common::set_ty(
                    self,
                    Some(icon),
                    mem(fp::fmuls(spacing, fp::frsp(f64::from(i)))),
                );
                common::add_child(self, d.jobjs[6], Some(icon));
                let (_, count) = self.aggregated_rank(s, i as u8);
                let txt = format_decimal(count as u32, 0);
                self.game.sis.add_string(vt, 0.0, row_y(i), Some(&txt));
            } else {
                if is_icon_only(s) {
                    continue;
                }
                if !names && entity == 25 {
                    super::super::code::unported_call(
                        "mnDiagram3_PopulateRankings for a rank past the unlocked characters (it reads past the records)",
                    );
                }
                let v = self.stat_value(d.name_mode, s, entity);
                let txt = if is_time(s) {
                    format_time(if v as u32 > 0x5B_8D7F { 0x5B_8D7F } else { v })
                } else if is_distance(s) {
                    int_to_str(self.distance_shown(v as u32).min(0x5F5_E0FF))
                } else if is_percentage(s) {
                    format_decimal((v as u32).min(0x98_967F), 2)
                } else {
                    format_decimal((v as u32).min(0x5F5_E0FF), 0)
                };
                self.game.sis.add_string(vt, 0.0, row_y(i), Some(&txt));
            }
            let unit = UNITS[usize::from(s)];
            if unit == 0xFFFF {
                continue;
            }
            let p = crate::lb::joint::world_position(
                &mut self.game.objs,
                self.data,
                d.jobjs[6],
                Some(VALUE_POS),
            );
            let x = mem(fp::fadds(reg(1.5), reg(p.x)));
            let y = mem(fp::fmadds(neg, fp::frsp(f64::from(i)), fp::fneg(reg(p.y))));
            let t = self.create_text(0, 1, Vec3::new(x, y, p.z), 1.0, 1.0);
            self.d3_mut(gobj).units[i as usize] = Some(t);
            let x = self.game.sis.text_mut(t);
            x.default_alignment = 1;
            x.text_color = VALUE_COLOR;
            if is_distance(s)
                && self.distance_overflows(self.stat_value(d.name_mode, s, entity) as u32)
            {
                self.game.sis.reset_text(self.data, t, 0x7F);
                continue;
            }
            self.game.sis.reset_text(self.data, t, i32::from(unit));
        }
    }

    /// The rankings made anew (`mnDiagram3_RefreshRankings`).
    fn d3_refresh(&mut self) {
        let g = self.d3_gobj();
        self.d3_clear(g);
        self.d3_rankings(g);
    }

    /// The cursor on row `n` (`mnDiagram3_PositionPopup`).
    fn d3_place_cursor(&mut self, n: u8) {
        let g = self.d3_gobj();
        let d = self.d3(g);
        let popup = common::jobj_of(self, d.popup.expect("the rankings' cursor"));
        let row0 = d.jobjs[8];
        let spacing = fp::fsubs(
            reg(common::ty(self, d.jobjs[9])),
            reg(common::ty(self, row0)),
        );
        let x = common::tx(self, row0);
        common::set_tx(self, Some(popup), x);
        let y = fp::fmadds(spacing, fp::frsp(f64::from(n)), reg(common::ty(self, row0)));
        common::set_ty(self, Some(popup), mem(y));
        let z = self.game.objs.jobjs[row0.expect("the first row")]
            .translate
            .z;
        jobj::set_srt(&mut self.game.objs.jobjs, popup, |o| o.translate.z = z);
    }

    /// The page leaving: the names' mode saved in the rules, the rankings and the cursor
    /// freed, the labels freed.
    fn d3_leave(&mut self) {
        let g = self.d3_gobj();
        self.game.game_data.x1850.xD = self.d3(g).name_mode;
        self.d3_clear(g);
        if let Some(p) = self.d3(g).popup {
            gobj::free(self, p);
        }
        self.d3_clear_labels(g);
    }

    /// `mnDiagram3_HandleInput`, the page's input: B leaves for the Records list, L turns to
    /// the statistics and R to the grid, X or Y switch between characters and names (with
    /// names saved), up and down move the cursor, scrolling at an edge.
    #[doc(alias = "mnDiagram3_HandleInput")]
    pub fn d3_input(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        let g = self.d3_gobj();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.d3_leave();
            self.go_to_list(RECORDS, 0, 3);
            return;
        }
        if b & (IN_L | IN_R) != 0 {
            self.lb_audio_call(1);
            self.d3_leave();
            gobj::free(self, gobj);
            if b & IN_L != 0 {
                self.d2_open();
            } else {
                self.diagram_open(false, 0);
            }
            return;
        }
        let d = self.d3(g);
        if b & IN_XY != 0 {
            if self.name_count() == 0 {
                self.lb_audio_call(3);
                return;
            }
            self.lb_audio_call(1);
            let mode = u8::from(d.name_mode == 0);
            let dm = self.d3_mut(g);
            dm.name_mode = mode;
            if mode == 0 && i32::from(dm.scroll) + 0xA >= 0x15 {
                dm.scroll = 0;
            }
            self.d3_clear_labels(g);
            self.d3_labels(g);
            self.d3_refresh();
            return;
        }
        if b & IN_UP != 0 {
            if d.cursor_row != 0 {
                self.lb_audio_call(2);
                self.d3_mut(g).cursor_row -= 1;
                self.d3_place_cursor(d.cursor_row - 1);
                self.d3_refresh();
                return;
            }
            if d.scroll != 0 {
                self.lb_audio_call(2);
                self.d3_mut(g).scroll -= 1;
                self.d3_clear_labels(g);
                self.d3_labels(g);
                self.d3_refresh();
            }
        } else if b & IN_DOWN != 0 {
            if d.cursor_row < 9 {
                self.lb_audio_call(2);
                self.d3_mut(g).cursor_row += 1;
                self.d3_place_cursor(d.cursor_row + 1);
                self.d3_refresh();
                return;
            }
            let limit = if d.name_mode != 0 { 0x18 } else { 0x15 };
            if u32::from(d.scroll) + 0xA < limit {
                self.lb_audio_call(2);
                self.d3_mut(g).scroll += 1;
                self.d3_clear_labels(g);
                self.d3_labels(g);
                self.d3_refresh();
            }
        }
    }

    /// `mnDiagram3_UpdateScrollArrows`: the arrows animated and shown where the list goes on.
    #[doc(alias = "mnDiagram3_UpdateScrollArrows")]
    fn d3_arrows(&mut self, gobj: GObjId) {
        let d = self.d3(gobj);
        let down = d.jobjs[3].expect("the down arrow");
        self.anim_loop(down, ARROW);
        let limit = if d.name_mode != 0 { 0x18 } else { 0x15 };
        if i32::from(d.scroll) + 10 < limit {
            common::show(self, Some(down));
        } else {
            common::hide(self, Some(down));
        }
        let up = d.jobjs[4].expect("the up arrow");
        self.anim_loop(up, ARROW);
        if d.scroll != 0 {
            common::show(self, Some(up));
        } else {
            common::hide(self, Some(up));
        }
    }

    /// `mnDiagram3_OnAnimComplete`, the page going out: gone at its end.
    #[doc(alias = "mnDiagram3_OnAnimComplete")]
    pub fn d3_out(&mut self, gobj: GObjId) {
        let jx = self.d3(gobj).jobjs[1].expect("the page's frame");
        if self.anim_loop(jx, OUT_ANIM) >= reg(OUT_ANIM.end) {
            gobj::free(self, gobj);
        }
    }

    /// `mnDiagram3_Think`, the page's process: gone when another page took its place, going
    /// out when the flow left the records; the arrows.
    #[doc(alias = "mnDiagram3_Think")]
    pub fn d3_think(&mut self, gobj: GObjId) {
        let f = self.game.menu.flow;
        if f.cur_menu != SCREEN || f.x10 != 2 {
            if f.cur_menu == SCREEN {
                gobj::free(self, gobj);
            } else {
                self.options_swap_proc(gobj, OUT);
                let d = self.d3(gobj);
                common::hide(self, d.jobjs[2]);
            }
        } else {
            self.d3_arrows(gobj);
        }
    }

    /// `mnDiagram3_FreeUserData`: `HSD_Free`.
    #[doc(alias = "mnDiagram3_FreeUserData")]
    pub(crate) fn d3_free(&mut self, addr: u32) {
        crate::hsd::heap::mem_free(addr);
        self.game.data_menus.diagram3.data.remove(&addr);
    }

    /// `mnDiagram3_Create`: the page on render link 6 with its user data
    /// (`mnDiagram3_InitUserData`: the cursor on the hovered row, in names when the rules say
    /// so) and process.
    #[doc(alias = "mnDiagram3_Create")]
    #[doc(alias = "mnDiagram3_InitUserData")]
    fn d3_create(&mut self) -> GObjId {
        let m = self.game.data_menus.diagram.con_b3;
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.diagram3.gobj = Some(gobj);
        let jt = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(jt));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        common::set_frame_all(self, Some(jt), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x78);
        let f = self.game.menu.flow;
        let mut jobjs = [None; 10];
        for (i, x) in jobjs.iter_mut().enumerate() {
            *x = common::node(self, Some(jt), i as i32);
        }
        self.game.data_menus.diagram3.data.insert(
            addr,
            Diagram3Data {
                saved_menu: f.cur_menu,
                cursor_row: f.hovered_selection as u8,
                anim_state: 1,
                name_mode: self.game.game_data.x1850.xD,
                jobjs,
                ..Diagram3Data::default()
            },
        );
        let w = &mut self.game.world;
        w.init_user_data(gobj, 0, UserData::Diagram3(addr));
        w.setup_proc(gobj, CodeRef(THINK), 0);
        gobj
    }

    /// `mnDiagram3_Init`: the records' page 2 (`x10`), the page, its cursor on render link 4
    /// at the first row, the labels, the rankings and the input's process.
    #[doc(alias = "mnDiagram3_Init")]
    pub fn d3_open(&mut self) {
        let f = &mut self.game.menu.flow;
        f.x10 = 2;
        f.hovered_selection = 0;
        let gobj = self.d3_create();
        let m = self.game.data_menus.diagram.cursor_b3;
        let popup = self.game.world.create(6, 7, 0x80);
        self.d3_mut(gobj).popup = Some(popup);
        let pj = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(popup, OBJ_JOBJ, Obj::JObj(pj));
        w.setup_gx_link(popup, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        common::set_frame_all(self, Some(pj), 0.0);
        let d = self.d3(gobj);
        let row0 = d.jobjs[8];
        let spacing = fp::fsubs(
            reg(common::ty(self, d.jobjs[9])),
            reg(common::ty(self, row0)),
        );
        let x = common::tx(self, row0);
        common::set_tx(self, Some(pj), x);
        let y = fp::fmadds(spacing, 0.0, reg(common::ty(self, row0)));
        common::set_ty(self, Some(pj), mem(y));
        let z = self.game.objs.jobjs[row0.expect("the first row")]
            .translate
            .z;
        jobj::set_srt(&mut self.game.objs.jobjs, pj, |o| o.translate.z = z);
        self.d3_labels(gobj);
        self.d3_rankings(gobj);
        self.options_input_proc(INPUT);
    }
}
