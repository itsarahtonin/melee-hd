//! The VS records' statistics (`melee/mn/mndiagram2.c`): one character's or name's
//! statistics, ten rows at a time; up and down scroll, left and right choose the character or
//! name, X or Y switch between characters and names, L and R turn to the grid and the
//! rankings, B leaves for the Records list. The rankings' sorts are here too.

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::title::AnimLoop;
use super::super::gm::unlocks::SELKIND_TO_CKIND;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::diagram::{
    IN_L, IN_R, IN_XY, SCREEN, format_decimal, format_time, int_to_str, name_format,
};
use super::main::{IN_BACK, IN_DOWN, IN_LEFT, IN_RIGHT, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input, the page's, and its going out.
pub const INPUT: u32 = 0x8024_3D40;
pub const THINK: u32 = 0x8024_4F68;
pub const OUT: u32 = 0x8024_4D20;

/// The Records list's `MenuKind`.
const RECORDS: u8 = 0x1C;

/// `mnDiagram2_803EEAD0`: where a row's parts sit (the header's, the label's, the value's and
/// the icon's offsets), each statistic's label and its unit's glyph (0xFFFF none).
const HEADER_POS: Vec3 = Vec3::new(-2.5, 0.3, 0.0);
const LABEL_POS: Vec3 = Vec3::new(-2.2, 0.5, 0.0);
const VALUE_POS: Vec3 = Vec3::new(-1.0, 0.5, 0.0);
const ICON_POS: Vec3 = Vec3::new(-2.0, 0.0, 0.0);
const LABELS: [u16; 24] = [
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
];
const UNITS: [u16; 24] = [
    122, 122, 122, 124, 124, 124, 124, 124, 122, 122, 122, 0xFFFF, 124, 123, 126, 126, 126, 126,
    125, 125, 125, 0xFFFF, 0xFFFF, 0xFFFF,
];

/// `mnDiagram2_803EEB60`: the page going out, and the arrows' loop.
const OUT_ANIM: AnimLoop = al(10.0, 19.0, -0.1);
const ARROW: AnimLoop = al(0.0, 199.0, 0.0);

/// `mnDiagram2_804D4FB8` and `mnDiagram2_804D4FBC`: the header's colour and the values'.
const HEADER_COLOR: [u8; 4] = [0, 0, 0, 0xFF];
const VALUE_COLOR: [u8; 4] = [0xFF, 0xC8, 0, 0xFF];

/// The statistics (`VSRecordsStatType`) the screens test.
pub(crate) mod stat {
    pub const TOTAL_KOS: u8 = 0;
    pub const TOTAL_FALLS: u8 = 1;
    pub const SD_COUNT: u8 = 2;
    pub const HIT_PERCENTAGE: u8 = 3;
    pub const DAMAGE_DEALT: u8 = 4;
    pub const DAMAGE_TAKEN: u8 = 5;
    pub const DAMAGE_RECOVERED: u8 = 6;
    pub const PEAK_DAMAGE: u8 = 7;
    pub const MATCH_COUNT: u8 = 8;
    pub const VICTORIES: u8 = 9;
    pub const LOSSES: u8 = 10;
    pub const PLAY_TIME: u8 = 11;
    pub const PLAY_PERCENTAGE: u8 = 12;
    pub const AVG_PLAYERS: u8 = 13;
    pub const WALK_DISTANCE: u8 = 14;
    pub const RUN_DISTANCE: u8 = 15;
    pub const FALL_DISTANCE: u8 = 16;
    pub const PEAK_HEIGHT: u8 = 17;
    pub const COINS_COLLECTED: u8 = 18;
    pub const COINS_SWIPED: u8 = 19;
    pub const COINS_LOST: u8 = 20;
    pub const MOST_PLAYED: u8 = 21;
    pub const SECOND_PLAYED: u8 = 22;
    pub const LEAST_PLAYED: u8 = 23;
}

/// `mnDiagram2_IsTimeStat`.
#[doc(alias = "mnDiagram2_IsTimeStat")]
pub(crate) fn is_time(s: u8) -> bool {
    s == stat::PLAY_TIME
}

/// `mnDiagram2_IsDistanceStat`.
#[doc(alias = "mnDiagram2_IsDistanceStat")]
pub(crate) fn is_distance(s: u8) -> bool {
    (stat::WALK_DISTANCE..=stat::PEAK_HEIGHT).contains(&s)
}

/// `mnDiagram2_IsPercentageStat`.
#[doc(alias = "mnDiagram2_IsPercentageStat")]
pub(crate) fn is_percentage(s: u8) -> bool {
    matches!(
        s,
        stat::HIT_PERCENTAGE | stat::PLAY_PERCENTAGE | stat::AVG_PLAYERS
    )
}

/// `mnDiagram2_IsIconOnlyStat`.
#[doc(alias = "mnDiagram2_IsIconOnlyStat")]
pub(crate) fn is_icon_only(s: u8) -> bool {
    (stat::MOST_PLAYED..=stat::LEAST_PLAYED).contains(&s)
}

/// `Diagram2` (0xC8 bytes of the heap): the flow it came from, whether its intro plays, the
/// model's first 15 joints in pre-order (`x8` to `right_arrow`), the first statistic shown,
/// the character's and the name's places in the order shown, which it shows, each row's label,
/// value and unit, and the header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Diagram2Data {
    pub saved_menu: u8,
    pub saved_selection: u16,
    pub anim_state: u8,
    pub jobjs: [Option<JObjId>; 15],
    pub scroll: u16,
    pub fighter: u8,
    pub name: u8,
    pub name_mode: u8,
    pub labels: [Option<TextId>; 10],
    pub values: [Option<TextId>; 10],
    pub units: [Option<TextId>; 10],
    pub header: Option<TextId>,
}

/// The joints `Diagram2` names (`jobjs` from `x8`).
mod j {
    pub const X10: usize = 2;
    pub const FIGHTER_HEADER: usize = 3;
    pub const ICON_SLOT: usize = 4;
    pub const FIGHTER_TEXT: usize = 5;
    pub const NAME_HEADER: usize = 6;
    pub const NAME_TEXT: usize = 7;
    pub const ICONS: usize = 8;
    pub const ROW0: usize = 9;
    pub const ROW1: usize = 10;
    pub const DOWN: usize = 11;
    pub const UP: usize = 12;
    pub const LEFT: usize = 13;
    pub const RIGHT: usize = 14;
}

/// `mndiagram2.c`'s statics: the page's GObj (`mnDiagram2_804D6C18`) and the user data by
/// block.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Diagram2 {
    pub gobj: Option<GObjId>,
    pub data: BTreeMap<u32, Diagram2Data>,
}

impl Running<'_> {
    fn d2_gobj(&self) -> GObjId {
        self.game
            .data_menus
            .diagram2
            .gobj
            .expect("the VS records' statistics")
    }

    fn d2_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Diagram2(a)) => a,
            other => panic!("the VS records' statistics hold {other:?}"),
        }
    }

    fn d2(&self, gobj: GObjId) -> Diagram2Data {
        self.game.data_menus.diagram2.data[&self.d2_addr(gobj)]
    }

    fn d2_mut(&mut self, gobj: GObjId) -> &mut Diagram2Data {
        let a = self.d2_addr(gobj);
        self.game
            .data_menus
            .diagram2
            .data
            .get_mut(&a)
            .expect("the VS records' statistics' data")
    }

    /// `mnDiagram2_ClearStatRows`: every row's texts freed, the icons dropped.
    #[doc(alias = "mnDiagram2_ClearStatRows")]
    fn d2_clear_rows(&mut self, gobj: GObjId) {
        for i in 0..10 {
            let d = self.d2(gobj);
            if let Some(t) = d.labels[i] {
                self.destroy_text(t);
                self.d2_mut(gobj).labels[i] = None;
            }
            if let Some(t) = d.values[i] {
                self.destroy_text(t);
                self.d2_mut(gobj).values[i] = None;
            }
            if let Some(t) = d.units[i] {
                self.destroy_text(t);
                self.d2_mut(gobj).units[i] = None;
            }
        }
        let icons = self.d2(gobj).jobjs[j::ICONS];
        let c = common::child(self, icons);
        if c.is_some() {
            jobj::remove_all(&mut self.game.objs, c);
        }
    }

    /// `mnDiagram2_UpdateHeader`: the character's icon (characters only) and the header's
    /// name, made anew.
    #[doc(alias = "mnDiagram2_UpdateHeader")]
    fn d2_header(&mut self, gobj: GObjId, name_mode: u8, idx: u8) {
        let d = self.d2(gobj);
        let who = if name_mode != 0 {
            self.name_at(i32::from(idx))
        } else {
            self.fighter_at(i32::from(idx))
        };
        if name_mode == 0 {
            let c = common::child(self, d.jobjs[j::ICON_SLOT]);
            if c.is_some() {
                jobj::remove_all(&mut self.game.objs, c);
            }
            let icon = self.diagram_icon(i32::from(who), false);
            common::add_child(self, d.jobjs[j::ICON_SLOT], Some(icon));
        }
        if let Some(t) = d.header {
            self.destroy_text(t);
        }
        let t = self.create_text_buffer(0, 1);
        self.d2_mut(gobj).header = Some(t);
        let at = if name_mode != 0 {
            d.jobjs[j::NAME_TEXT]
        } else {
            d.jobjs[j::FIGHTER_TEXT]
        };
        let p =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, at, Some(HEADER_POS));
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.045, 0.045];
        x.text_color = HEADER_COLOR;
        x.pos = Vec3::new(p.x, -p.y, p.z);
        x.default_alignment = 1;
        if name_mode != 0 {
            let n = self.name_of(who);
            self.game.sis.add_string(t, 0.0, 0.0, Some(name_format(&n)));
        } else {
            self.character_name_text(t, SELKIND_TO_CKIND[usize::from(who)]);
        }
    }

    fn d2_entity(d: &Diagram2Data) -> u8 {
        if d.name_mode != 0 { d.name } else { d.fighter }
    }

    /// The rows made anew (`mnDiagram2_RefreshStatRows`).
    fn d2_refresh(&mut self) {
        let g = self.d2_gobj();
        self.d2_clear_rows(g);
        let d = self.d2(g);
        self.d2_rows(g, d.scroll as u8, d.name_mode, Self::d2_entity(&d));
    }

    /// The page's selection saved in the rules (`saveDiagramSelection`).
    fn d2_save(&mut self) {
        let d = self.d2(self.d2_gobj());
        let r = &mut self.game.game_data.x1850;
        r.x12 = d.fighter;
        r.x13 = d.name;
        r.xD = d.name_mode;
    }

    /// The characters' or names' header shown, and its name (`updateDiagramMode`).
    fn d2_mode(&mut self, name_mode: u8) {
        let g = self.d2_gobj();
        let d = self.d2(g);
        if name_mode != 0 {
            common::hide(self, d.jobjs[j::FIGHTER_HEADER]);
            common::show(self, d.jobjs[j::NAME_HEADER]);
        } else {
            common::show(self, d.jobjs[j::FIGHTER_HEADER]);
            common::hide(self, d.jobjs[j::NAME_HEADER]);
        }
        self.d2_header(g, name_mode, Self::d2_entity(&d));
    }

    /// `mnDiagram2_HandleInput`, the page's input: B leaves for the Records list, L turns to
    /// the grid and R to the rankings, X or Y switch between characters and names (with names
    /// saved), up and down scroll, left and right choose the character or name.
    #[doc(alias = "mnDiagram2_HandleInput")]
    pub fn d2_input(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        let g = self.d2_gobj();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.d2_save();
            self.go_to_list(RECORDS, 0, 3);
            self.d2_clear_rows(g);
            return;
        }
        if b & (IN_L | IN_R) != 0 {
            self.lb_audio_call(1);
            self.d2_save();
            self.d2_clear_rows(g);
            gobj::free(self, gobj);
            if b & IN_L != 0 {
                self.diagram_open(false, 0);
                return;
            }
            self.d3_open();
            return;
        }

        let d = self.d2(g);
        if b & IN_XY != 0 {
            if self.name_count() == 0 {
                self.lb_audio_call(3);
                return;
            }
            self.lb_audio_call(1);
            let mode = u8::from(d.name_mode == 0);
            let dm = self.d2_mut(g);
            dm.name_mode = mode;
            if mode == 0 && i32::from(dm.scroll) + 10 > 0x15 {
                dm.scroll = 0;
            }
            self.d2_refresh();
            let d = self.d2(g);
            self.d2_header(g, d.name_mode, Self::d2_entity(&d));
            self.d2_mode(mode);
            return;
        }
        let names = d.name_mode != 0;
        let limit = if names { 0x18 } else { 0x15 };
        if b & IN_UP != 0 {
            if d.scroll != 0 {
                self.lb_audio_call(2);
                self.d2_mut(g).scroll -= 1;
                self.d2_refresh();
            }
        } else if b & IN_DOWN != 0 {
            if i32::from(d.scroll) + 10 < limit {
                self.lb_audio_call(2);
                self.d2_mut(g).scroll += 1;
                self.d2_refresh();
            }
        } else if b & (IN_LEFT | IN_RIGHT) != 0 {
            let cur = Self::d2_entity(&d);
            let left = b & IN_LEFT != 0;
            let s = match (names, left) {
                (true, true) => self.prev_name(i32::from(cur)),
                (true, false) => self.next_name(i32::from(cur)),
                (false, true) => self.prev_fighter(i32::from(cur)),
                (false, false) => self.next_fighter(i32::from(cur)),
            };
            if s != cur {
                self.lb_audio_call(2);
                let dm = self.d2_mut(g);
                if names {
                    dm.name = s;
                } else {
                    dm.fighter = s;
                }
                self.d2_refresh();
                let d = self.d2(g);
                self.d2_header(g, d.name_mode, s);
            }
        }
    }

    /// `mnDiagram2_GetStatValue`: a statistic of a character or name tag (by its selectable
    /// kind or slot).
    #[doc(alias = "mnDiagram2_GetStatValue")]
    pub(crate) fn stat_value(&self, name_mode: u8, s: u8, i: u8) -> i32 {
        let names = name_mode != 0;
        let st = if names {
            &self.name_tag(i).stats
        } else {
            &self.fighter_record(i).stats
        };
        match s {
            stat::TOTAL_KOS => {
                if names {
                    self.name_total_kos(i)
                } else {
                    self.fighter_total_kos(i)
                }
            }
            stat::TOTAL_FALLS => {
                if names {
                    self.name_sum_kos(i, true)
                } else {
                    self.fighter_total_falls(i)
                }
            }
            stat::SD_COUNT => i32::from(st.sd_count),
            stat::HIT_PERCENTAGE => self.hit_percentage(name_mode, i),
            stat::DAMAGE_DEALT => st.damage_dealt,
            stat::DAMAGE_TAKEN => st.damage_taken,
            stat::DAMAGE_RECOVERED => st.damage_recovered,
            stat::PEAK_DAMAGE => i32::from(st.peak_damage),
            stat::MATCH_COUNT => i32::from(st.match_count),
            stat::VICTORIES => i32::from(st.victories),
            stat::LOSSES => i32::from(st.losses),
            stat::PLAY_TIME => st.play_time as i32,
            stat::PLAY_PERCENTAGE => self.play_percentage(name_mode, i),
            stat::AVG_PLAYERS => self.average_players(name_mode, i),
            stat::WALK_DISTANCE => st.walk_distance,
            stat::RUN_DISTANCE => st.run_distance,
            stat::FALL_DISTANCE => st.fall_distance,
            stat::PEAK_HEIGHT => st.peak_height,
            stat::COINS_COLLECTED => st.coins_collected,
            stat::COINS_SWIPED => st.coins_swiped,
            stat::COINS_LOST => st.coins_lost,
            stat::MOST_PLAYED => i32::from(self.name_ranked_fighter(0, i32::from(i))),
            stat::SECOND_PLAYED => i32::from(self.name_ranked_fighter(1, i32::from(i))),
            stat::LEAST_PLAYED => i32::from(self.name_least_played(i)),
            _ => super::super::code::unported_call(&format!(
                "mnDiagram2_GetStatValue for statistic {s} (a register it never sets)"
            )),
        }
    }

    /// `mnDiagram2_CreateStatRow`: row `row` shows statistic `s` of entity `who`: its label,
    /// its unit, and its value (a time, a distance, a percentage, a character's icon, or a
    /// number, each held at its most).
    #[doc(alias = "mnDiagram2_CreateStatRow")]
    fn d2_row(&mut self, gobj: GObjId, name_mode: u8, s: u8, row: u8, who: u8) {
        let d = self.d2(gobj);
        let ty = |r: &Self, k: usize| reg(common::ty(r, d.jobjs[k]));
        let f31 = ty(self, j::ROW0);
        let f30 = fp::fsubs(ty(self, j::ROW1), f31);
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[j::ROW0],
            Some(LABEL_POS),
        );
        let off = fp::fmuls(fp::fneg(f30), fp::frsp(f64::from(row)));
        let ny = fp::fneg(reg(p.y));
        let t = self.create_text(
            0,
            1,
            Vec3::new(p.x, mem(fp::fadds(ny, off)), p.z),
            320.0,
            240.0,
        );
        self.d2_mut(gobj).labels[usize::from(row)] = Some(t);
        self.game
            .sis
            .reset_text(self.data, t, i32::from(LABELS[usize::from(s)]));
        let unit = UNITS[usize::from(s)];
        if unit != 0xFFFF {
            let p = crate::lb::joint::world_position(
                &mut self.game.objs,
                self.data,
                d.jobjs[j::ROW0],
                Some(LABEL_POS),
            );
            let t = self.create_text(
                0,
                1,
                Vec3::new(
                    mem(fp::fadds(12.0, reg(p.x))),
                    mem(fp::fadds(fp::fneg(reg(p.y)), off)),
                    p.z,
                ),
                1.0,
                1.0,
            );
            self.d2_mut(gobj).units[usize::from(row)] = Some(t);
            let x = self.game.sis.text_mut(t);
            x.default_alignment = 1;
            x.text_color = VALUE_COLOR;
            let id = if is_distance(s)
                && self.distance_overflows(self.stat_value(name_mode, s, who) as u32)
            {
                0x7F
            } else {
                i32::from(unit)
            };
            self.game.sis.reset_text(self.data, t, id);
        }
        if is_icon_only(s) && (self.stat_value(name_mode, s, who) as u32) < 25 {
            let k = self.stat_value(name_mode, s, who);
            let icon = self.diagram_icon(k, false);
            common::set_tx(self, Some(icon), ICON_POS.x);
            let y = fp::fmadds(f30, fp::frsp(f64::from(row)), reg(ICON_POS.y));
            common::set_ty(self, Some(icon), mem(y));
            jobj::set_srt(&mut self.game.objs.jobjs, icon, |o| {
                o.translate.z = ICON_POS.z
            });
            common::add_child(self, d.jobjs[j::ICONS], Some(icon));
            return;
        }
        let t = self.create_text_buffer(0, 1);
        self.d2_mut(gobj).values[usize::from(row)] = Some(t);
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[j::ICONS],
            Some(VALUE_POS),
        );
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.03, 0.035];
        x.pos = Vec3::new(p.x, mem(fp::fadds(fp::fneg(reg(p.y)), off)), p.z);
        x.text_color = VALUE_COLOR;
        x.default_alignment = 2;
        let v = self.stat_value(name_mode, s, who);
        let text = if is_time(s) {
            format_time(if v as u32 > 0x927BF { 0x927BF } else { v })
        } else if is_distance(s) {
            int_to_str(self.distance_shown(v as u32).min(0x98_967F))
        } else if is_percentage(s) {
            format_decimal((if v as u32 > 0xF423F { 0xF423F } else { v }) as u32, 2)
        } else if is_icon_only(s) {
            b"\x81|".to_vec()
        } else {
            format_decimal((if v as u32 > 0x98_967F { 0x98_967F } else { v }) as u32, 0)
        };
        self.game.sis.add_string(t, 0.0, 0.0, Some(&text));
    }

    /// `mnDiagram2_PopulateStatRows`: the ten rows from statistic `scroll` on, wrapping.
    #[doc(alias = "mnDiagram2_PopulateStatRows")]
    fn d2_rows(&mut self, gobj: GObjId, scroll: u8, name_mode: u8, idx: u8) {
        let who = if name_mode != 0 {
            self.name_at(i32::from(idx))
        } else {
            self.fighter_at(i32::from(idx))
        };
        let limit = if name_mode != 0 { 0x18 } else { 0x15 };
        for i in 0..10u8 {
            let k = i32::from(scroll) + i32::from(i);
            let s = if k >= limit { k - limit } else { k };
            self.d2_row(gobj, name_mode, s as u8, i, who);
        }
    }

    /// `mnDiagram2_OnAnimComplete`, the page going out: gone at its end.
    #[doc(alias = "mnDiagram2_OnAnimComplete")]
    pub fn d2_out(&mut self, gobj: GObjId) {
        let jx = self.d2(gobj).jobjs[1].expect("the page's frame");
        if self.anim_loop(jx, OUT_ANIM) >= reg(OUT_ANIM.end) {
            gobj::free(self, gobj);
        }
    }

    /// `mnDiagram2_UpdateScrollArrows`: each arrow animated and shown where the page can go
    /// that way.
    #[doc(alias = "mnDiagram2_UpdateScrollArrows")]
    fn d2_arrows(&mut self, gobj: GObjId) {
        let d = self.d2(gobj);
        let names = d.name_mode != 0;
        let shown = |r: &mut Self, k: usize, v: bool| {
            let x = d.jobjs[k].expect("an arrow");
            r.anim_loop(x, ARROW);
            if v {
                common::show(r, Some(x));
            } else {
                common::hide(r, Some(x));
            }
        };
        let limit = if names { 0x18 } else { 0x15 };
        shown(self, j::DOWN, i32::from(d.scroll) + 10 < limit);
        shown(self, j::UP, d.scroll != 0);
        let cur = Self::d2_entity(&d);
        shown(self, j::LEFT, cur != 0);
        let next = if names {
            self.next_name(i32::from(cur))
        } else {
            self.next_fighter(i32::from(cur))
        };
        shown(self, j::RIGHT, cur != next);
    }

    /// `mnDiagram2_Think`, the page's process: gone when another page took its place, going
    /// out (its header freed) when the flow left the records; the arrows.
    #[doc(alias = "mnDiagram2_Think")]
    pub fn d2_think(&mut self, gobj: GObjId) {
        let f = self.game.menu.flow;
        if f.cur_menu != SCREEN || f.x10 != 1 {
            if f.cur_menu == SCREEN {
                gobj::free(self, gobj);
            } else {
                self.options_swap_proc(gobj, OUT);
                let d = self.d2(gobj);
                common::hide(self, d.jobjs[j::X10]);
                if let Some(t) = d.header {
                    self.destroy_text(t);
                    self.d2_mut(gobj).header = None;
                }
            }
            return;
        }
        self.d2_arrows(gobj);
    }

    /// `mnDiagram2_FreeUserData`, the page's user data's release: the header freed, then the
    /// block (`HSD_Free`).
    #[doc(alias = "mnDiagram2_FreeUserData")]
    pub(crate) fn d2_free(&mut self, addr: u32) {
        let d = self.game.data_menus.diagram2.data[&addr];
        if let Some(t) = d.header {
            self.destroy_text(t);
        }
        crate::hsd::heap::mem_free(addr);
        self.game.data_menus.diagram2.data.remove(&addr);
    }

    /// `mnDiagram2_Create`: the page on render link 6, its user data (`mnDiagram2_InitUserData`:
    /// the first character and name, in names when the rules say so) and process, its rows,
    /// its header.
    #[doc(alias = "mnDiagram2_Create")]
    #[doc(alias = "mnDiagram2_InitUserData")]
    fn d2_create(&mut self) {
        let m = self.game.data_menus.diagram.con_b2;
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.diagram2.gobj = Some(gobj);
        let data = self.data;
        let jt = jobj::load_joint(&mut self.game.objs, data, m.joint).expect("the statistics");
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(jt));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        jobj::add_anim_all(
            &mut self.game.objs,
            data,
            Some(jt),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        common::req_all(self, Some(jt), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0xC8);
        let f = self.game.menu.flow;
        let mut jobjs = [None; 15];
        for (i, x) in jobjs.iter_mut().enumerate() {
            *x = common::node(self, Some(jt), i as i32);
        }
        let d = Diagram2Data {
            saved_menu: f.cur_menu,
            saved_selection: f.hovered_selection,
            anim_state: 1,
            jobjs,
            name_mode: self.game.game_data.x1850.xD,
            ..Diagram2Data::default()
        };
        self.game.data_menus.diagram2.data.insert(addr, d);
        let w = &mut self.game.world;
        w.init_user_data(gobj, 0, UserData::Diagram2(addr));
        w.setup_proc(gobj, CodeRef(THINK), 0);
        self.d2_rows(gobj, d.scroll as u8, d.name_mode, Self::d2_entity(&d));
        self.d2_mode(d.name_mode);
    }

    /// `mnDiagram2_Init`: the records' page 1 (`x10`), the page and the input's process.
    #[doc(alias = "mnDiagram2_Init")]
    pub fn d2_open(&mut self) {
        let f = &mut self.game.menu.flow;
        f.x10 = 1;
        f.hovered_selection = 0;
        self.d2_create();
        self.options_input_proc(INPUT);
    }

    /// `mnDiagram2_GetRankedFighter`: the character at place `rank` by a statistic, most
    /// first, the locked ones last (as all ones, `-1`), or 25 for a locked one there. The
    /// statistic compares as the low word of a 64-bit key, so a negative one ranks first.
    #[doc(alias = "mnDiagram2_GetRankedFighter")]
    pub(crate) fn ranked_fighter(&self, s: u8, rank: u8) -> u8 {
        let mut e: [(u8, u64); 25] = std::array::from_fn(|i| {
            let k = self.fighter_at(i as i32);
            let v = if self.selkind_unlocked(k) {
                u64::from(self.stat_value(0, s, k) as u32)
            } else {
                u64::MAX
            };
            (k, v)
        });
        for i in 0..25 {
            let mut best = i;
            for k in i + 1..25 {
                if e[k].1 != u64::MAX && (e[k].1 > e[best].1 || e[i].1 == u64::MAX) {
                    best = k;
                }
            }
            if best != i {
                let t = e[best];
                for j in (i + 1..=best).rev() {
                    e[j] = e[j - 1];
                }
                e[i] = t;
            }
        }
        let r = e[usize::from(rank)];
        if r.1 == u64::MAX { 25 } else { r.0 }
    }

    /// `mnDiagram2_GetRankedName`: the name at place `rank` by a statistic, most first (the
    /// statistic as an unsigned word), or 120 past the names.
    #[doc(alias = "mnDiagram2_GetRankedName")]
    pub(crate) fn ranked_name(&self, s: u8, rank: u8) -> u8 {
        let count = self.name_count() as usize;
        if usize::from(rank) >= count {
            return 0x78;
        }
        let mut e: Vec<(u8, u64)> = (0..count)
            .map(|i| {
                let n = self.name_at(i as i32);
                (n, u64::from(self.stat_value(1, s, n) as u32))
            })
            .collect();
        for i in 0..count {
            let mut best = i;
            for k in i + 1..count {
                if e[best].1 < e[k].1 {
                    best = k;
                }
            }
            if best != i {
                let t = e[best];
                for j in (i + 1..=best).rev() {
                    e[j] = e[j - 1];
                }
                e[i] = t;
            }
        }
        e[usize::from(rank)].0
    }

    /// `mnDiagram2_GetAggregatedFighterRank`: over every name, how many have each character as
    /// their most played (0x15), second (0x16) or least played (0x17); the character at place
    /// `idx` by that count with it, its character 25 for a count of 0.
    #[doc(alias = "mnDiagram2_GetAggregatedFighterRank")]
    pub(crate) fn aggregated_rank(&self, ty: u8, idx: u8) -> (u8, u64) {
        let mut e: [(u8, u64); 25] = std::array::from_fn(|i| (self.fighter_at(i as i32), 0));
        for i in 0..self.name_count() {
            let res = match ty {
                0x15 => self.name_ranked_fighter(0, i),
                0x16 => self.name_ranked_fighter(1, i),
                0x17 => self.name_least_played(i as u8),
                _ => super::super::code::unported_call(&format!(
                    "mnDiagram2_GetAggregatedFighterRank for statistic {ty} (a register it never sets)"
                )),
            };
            if res != 25
                && let Some(k) = e.iter().position(|x| x.0 == res)
            {
                e[k].1 += 1;
            }
        }
        for a in 0..25 {
            for b in a + 1..25 {
                if e[a].1 < e[b].1 {
                    e.swap(a, b);
                }
            }
        }
        let r = e[usize::from(idx)];
        if r.1 != 0 { r } else { (25, r.1) }
    }
}
