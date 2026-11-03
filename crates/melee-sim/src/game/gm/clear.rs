//! A single-player stage's clear screen (`melee/gm/gmregclear.c`): after a 1P run's match is
//! won, the frozen match fades out behind a blur of itself (`lbspdisplay.c`'s screen copy,
//! [`crate::lb::blur`]) while the screen's model shows the score: the run's score before the
//! match counting up to its new total, the time bonus (seconds left by 100, or the targets'
//! count), the damage bonus, the bonuses' total, and the list of bonuses earned, seven rows at
//! a time, scrolled by the stick and by itself every half second. A skips the count, Start
//! ends the screen once it has shown for 0x3F frames; the match's exit then hands the score
//! back to the run (`fn_8017F294`).

#![allow(non_snake_case)]

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::code::ParticleHooks;
use super::super::handles::CodeRef;
use super::super::hud::ModelDesc;
use super::super::pl::player::PKIND_HUMAN;
use super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::super::structs::{HSD_ImageDesc, MatchEnd};
use super::decisions::{decision_flags, decision_kind, decision_text};
use super::vs::{OUTCOME_1P_BONUS_STAGE_END, OUTCOME_NO_CONTEST, OUTCOME_RETRY};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, HIDDEN, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// `fn_8017FF1C`, the screen model's process.
pub const PROC: u32 = 0x8017_FF1C;
/// `fn_8017FE54`, the blur's callback before each draw.
pub const BLUR_CALLBACK: u32 = 0x8017_FE54;

/// `lbl_803D8B88`: the model's joints the screen keeps (`x4` to `x28`): the score plate's
/// texture, the bonuses' total, the score, the time bonus's label, the list, the damage
/// bonus's label, the targets' row, the list's arrows down and up, and one more.
const JOINTS: [i32; 10] = [0x18, 0x16, 0x12, 0x3, 0x5, 0x4, 0x6, 0x1A, 0x19, 0x7];

/// The bonus decisions the list walks (`lbl_803D5A4C`'s 0x101 entries).
const DECISIONS: i32 = 0x101;
/// The decisions the player library counts; the rest come from the results.
const COUNTED: i32 = 0xD7;

/// `HSD_PAD_A`, Start, and the stick's and the pad's up and down.
const PAD_A: u32 = 0x100;
const PAD_START: u32 = 0x1000;
const UP: u64 = 0x8 | 0x1_0000;
const DOWN: u64 = 0x4 | 0x2_0000;

/// `lbl_80472D28`, the screen's state, and `lbl_804D65C0`, the score's step.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Clear {
    /// `x0`: the model's GObj; `x4` to `x28`: its joints ([`JOINTS`]).
    pub gobj: Option<GObjId>,
    pub joints: [Option<JObjId>; 10],
    /// `x2C`: the blur's GObj; `x30`: the frame's copy it draws, and its pixels' block.
    pub blur: Option<GObjId>,
    pub image: HSD_ImageDesc,
    pub buffer: u32,
    /// `x48`: `GmRegClr.dat`, and its two blocks in the scene's heap.
    pub archive: Option<ArchiveId>,
    pub blocks: (u32, u32),
    /// `x4C`: the scene's model, and `x5C` to `x68` its lights, its two cameras and its fog
    /// (`fn_80168A6C`'s copy of the scene description).
    pub model: ModelDesc,
    pub lights: Option<DataRef>,
    pub cameras: [Option<DataRef>; 2],
    pub fog: Option<DataRef>,
    /// `x6C` to `x80`: the time bonus's and the damage bonus's labels, their values, the
    /// bonuses' total and the score; `x84`: the list's points.
    pub texts: [Option<TextId>; 6],
    pub points: Option<TextId>,
    /// `x88` to `xA0`: the list's names; `xA4` to `xBC`: the points each row shows.
    pub rows: [Option<TextId>; 7],
    pub shown: [i32; 7],
    /// `xC0`: the list's first row's decision; `xC4`: the frame the list last moved; `xC8`:
    /// moved (the list no longer scrolls by itself).
    pub first: u16,
    pub moved_at: u32,
    pub moved: u8,
    /// `xCC`: the match's points; `xD0`: the time bonus, `xD4` the one shown and `xD8` the
    /// frames it has shown; `xDC` to `xE4`: the damage bonus's; `xE8`: the bonuses' points;
    /// `xEC`: never read; `xF0`: the bonuses' total, `xF4` the one shown, `xF8` the frames
    /// it has; `xFC`: the score shown, `x100` the one drawn; `x104`: the new score.
    pub xCC: i32,
    pub xD0: i32,
    pub xD4: i32,
    pub xD8: i32,
    pub xDC: i32,
    pub xE0: i32,
    pub xE4: i32,
    pub xE8: i32,
    pub xEC: i32,
    pub xF0: i32,
    pub xF4: i32,
    pub xF8: i32,
    pub xFC: i32,
    pub x100: i32,
    pub x104: i32,
    /// `x108`, `x10A`: the time and damage bonuses' multipliers; `x10C`: the blur's growth;
    /// `x110`: the frames shown.
    pub x108: i16,
    pub x10A: i16,
    pub x10C: f32,
    pub x110: u32,
    /// `x114`: the match's continue flag (the plate's texture frame); `x115`: the score's
    /// count done; `x116`: ended with Start; `x117`: a bonus list; `x118`: the race to the
    /// finish's (no time bonus); `x119`: a boss's; `x11A`: a targets stage's; `x11B`: every
    /// target broken... none left (`sp60 == 0`); `x11C`: the targets broken; `x11D`: the
    /// targets' icons shown; `x11E`, `x11F`: the list's up and down arrows hidden.
    pub x114: u8,
    pub x115: u8,
    pub x116: u8,
    pub x117: u8,
    pub x118: u8,
    pub x119: u8,
    pub x11A: u8,
    pub x11B: u8,
    pub x11C: u8,
    pub x11D: u8,
    pub x11E: u8,
    pub x11F: u8,
    /// `lbl_804D65C0`: the score's step a frame.
    pub step: i32,
}

/// The string `vsnprintf` makes of `%d`.
fn int(v: i32) -> Vec<u8> {
    v.to_string().into_bytes()
}

/// `－`, `＋` and `Ｘ` in the font's Shift JIS.
const MINUS: &[u8] = b"\x81\x7C";
const PLUS: &[u8] = b"\x81\x7B";
const TIMES: &[u8] = b"\x82\x77";

impl Running<'_> {
    fn clear(&self) -> &Clear {
        &self.game.one_player.clear
    }

    fn clear_mut(&mut self) -> &mut Clear {
        &mut self.game.one_player.clear
    }

    /// The bonuses the mode's clear lists, `fn_8017F008`: a mask of the decisions' match
    /// kinds by the run's progress (`fn_80171A88`).
    #[doc(alias = "fn_8017F008")]
    fn clear_mask(&self) -> u8 {
        match self.game.single_player_progress() {
            3 => 4,
            1 => 1,
            2 => 8,
            7 => 0x20,
            6 => 0x80,
            8 => 0x40,
            4 => 0x10,
            _ => 0xFD,
        }
    }

    /// Whether the time bonus shows its value (not for the race, a targets stage with
    /// targets left, or a timer counting up), inline in `fn_8017F09C` and `fn_8017F608`.
    fn clear_time_shown(&self) -> bool {
        let c = self.clear();
        !(c.x118 != 0 || (c.x11A != 0 && c.x11B == 0) || self.game.vs.start.timer_counts_up)
    }

    /// The time bonus, `fn_8017F09C`: the seconds left by the multiplier, in a match whose
    /// rules give it (`x4_6`).
    #[doc(alias = "fn_8017F09C")]
    fn clear_time_bonus(&self) -> i32 {
        if !self.game.vs.start.x4_6 || !self.clear_time_shown() {
            return 0;
        }
        let p =
            (self.game.vs.state.timer_seconds as i32).wrapping_mul(i32::from(self.clear().x108));
        p.max(0)
    }

    /// The damage bonus, `fn_8017F14C`: the player's damage dealt (`x98`, at most 9999) by
    /// the multiplier, in a match whose rules give it (`x4_7`).
    #[doc(alias = "fn_8017F14C")]
    fn clear_damage_bonus(&self, r: &MatchEnd) -> i32 {
        if !self.game.vs.start.x4_7 {
            return 0;
        }
        // `x98`: the first player's damage dealt (`player_standings[0].x40`).
        let v = r.player_standings[0].x40 as i32;
        v.min(9999).wrapping_mul(i32::from(self.clear().x10A))
    }

    /// Whether decision `i` counts for player `player` under `mask`: in the match kinds
    /// `mask` names, and earned (counted by the player library below [`COUNTED`], else
    /// from the results), as each of `gm_16F1.c`'s list walkers asks.
    fn clear_earned(&self, r: &MatchEnd, i: i32, mask: u8, player: u8) -> bool {
        if mask & decision_flags(i) == 0 {
            return false;
        }
        if decision_kind(i) < COUNTED {
            self.game.bonus_count(i32::from(player), i) != 0
        } else {
            self.standing_decision(r, i32::from(player), i) != 0
        }
    }

    /// Each player's bonuses tallied, `fn_8016F344`.
    #[doc(alias = "fn_8016F344")]
    fn tally_all_bonuses(&mut self) {
        for i in 0..4 {
            if self.game.vs.state.x24C.player_standings[i].pkind != 3 {
                self.tally_bonuses(i as i32);
            }
        }
    }

    /// The first `n` bonuses from `from` that `player` earned, each one's name put on the
    /// next of `rows`, `fn_8016F39C`; how many.
    #[doc(alias = "fn_8016F39C")]
    fn bonus_names(&mut self, rows: &[Option<TextId>], from: u16, mask: u8, player: u8) -> i32 {
        if i32::from(from) >= DECISIONS {
            return -1;
        }
        let r = self.game.vs.state.x24C.clone();
        let us = super::super::gm::language_setting_is_us();
        let mut count = 0;
        for idx in i32::from(from)..DECISIONS {
            if self.clear_earned(&r, idx, mask, player) {
                let t = rows[count].expect("a list row");
                let text = i32::from(decision_text(idx, us) as u16);
                self.game.sis.reset_text(self.data, t, text);
                count += 1;
                if count == rows.len() {
                    break;
                }
            }
        }
        count as i32
    }

    /// The first bonus from `from` the player earned, else the last before it, `fn_8016F548`;
    /// -1 for none.
    #[doc(alias = "fn_8016F548")]
    fn bonus_at_or_before(&self, r: &MatchEnd, from: u16, mask: u8, player: u8) -> i32 {
        if i32::from(from) >= DECISIONS {
            return -1;
        }
        (i32::from(from)..DECISIONS)
            .chain((0..DECISIONS).rev())
            .find(|&i| self.clear_earned(r, i, mask, player))
            .unwrap_or(-1)
    }

    /// The next bonus after `from` the player earned, `fn_8016F740`; -1 for none.
    #[doc(alias = "fn_8016F740")]
    fn bonus_after(&self, r: &MatchEnd, from: u16, mask: u8, player: u8) -> i32 {
        let start = u32::from(from) + 1;
        if start >= DECISIONS as u32 {
            return -1;
        }
        (start as i32..DECISIONS)
            .find(|&i| self.clear_earned(r, i, mask, player))
            .unwrap_or(-1)
    }

    /// The last bonus before `from` the player earned, `fn_8016F870`; -1 for none.
    #[doc(alias = "fn_8016F870")]
    fn bonus_before(&self, r: &MatchEnd, from: u16, mask: u8, player: u8) -> i32 {
        if i32::from(from) > DECISIONS || from == 0 {
            return -1;
        }
        (0..i32::from(from))
            .rev()
            .find(|&i| self.clear_earned(r, i, mask, player))
            .unwrap_or(-1)
    }

    /// How many bonuses from `from` on the player earned, `fn_8016F9A8`; -1 past the list.
    #[doc(alias = "fn_8016F9A8")]
    fn bonuses_from(&self, r: &MatchEnd, from: u16, mask: u8, player: u8) -> i32 {
        if i32::from(from) >= DECISIONS {
            return -1;
        }
        (i32::from(from)..DECISIONS)
            .filter(|&i| self.clear_earned(r, i, mask, player))
            .count() as i32
    }

    /// The bonuses' points, `fn_8017F1B8`: each one the human earned noted in the save
    /// (`fn_8016F140`), then their total (`fn_8016FFD4`).
    #[doc(alias = "fn_8017F1B8")]
    fn clear_bonus_points(&mut self) -> i32 {
        let r = self.game.vs.state.x24C.clone();
        let mask = self.clear_mask();
        for i in 0..0x100 {
            if mask & decision_flags(i) == 0 {
                continue;
            }
            let earned = if decision_kind(i) < COUNTED {
                self.game.bonus_count(0, i) != 0
            } else {
                self.standing_decision(&r, 0, i) != 0
            };
            if earned {
                self.game.note_bonus_seen(i as usize);
            }
        }
        self.bonus_total(&r, i32::from(mask), 0)
    }

    /// The list's texts, `fn_8017F2A4`: font 3's canvas with `SdDec`'s strings, the points'
    /// text (seven blank rows and the total's) at (`x`, `y`), and seven name rows beside it.
    #[doc(alias = "fn_8017F2A4")]
    fn clear_list_texts(&mut self, x: f32, y: f32) {
        let canvas = self.add_canvas(3, None, 9, 0xD, 0, 0x14, 0, 0x13);
        let name = if self.game.gm.unlocks.is_us() {
            "SdDec.usd"
        } else {
            "SdDec.dat"
        };
        self.load_font_named(3, name, "SIS_DecisionData");
        let t = self.create_text_buffer(3, canvas);
        let tx = self.game.sis.text_mut(t);
        tx.default_kerning = 1;
        tx.x34 = [0.56, 0.6];
        tx.default_alignment = 2;
        self.clear_mut().points = Some(t);
        let x_end = fp::fadds(281.0, reg(x));
        for i in 0..7 {
            let row_y = fp::fmadds(
                reg(0.6),
                fp::fmuls(32.0, fp::frsp(f64::from(i as i32))),
                reg(y),
            );
            let row = self.create_text(3, 0, Vec3::new(x, mem(row_y), 0.0), 2.0, 2.0);
            let rx = self.game.sis.text_mut(row);
            rx.x34 = [0.56, 0.6];
            rx.default_kerning = 1;
            self.clear_mut().rows[i as usize] = Some(row);
            let at = fp::fadds(fp::fneg(12.0), row_y);
            self.game.sis.add_string(t, x_end, at, Some(b"          "));
        }
        let at = fp::fadds(9.0, fp::fadds(reg(134.40001), reg(y)));
        self.game
            .sis
            .add_string(t, x_end, at, Some(b"               "));
    }

    /// Writes `v` into the points' entry `i`, a negative one after a minus.
    fn clear_points(&mut self, t: TextId, i: i32, v: i32) {
        let s = if v < 0 {
            [MINUS, &int(v.wrapping_neg())].concat()
        } else {
            int(v)
        };
        self.game.sis.replace_string(self.data, t, i, Some(&s));
    }

    /// The list's rows from bonus `from`, `fn_8017F47C`: each row's name and its points
    /// where they changed, and the bonuses' total; the first bonus at or before `from`.
    #[doc(alias = "fn_8017F47C")]
    fn clear_list(&mut self, from: u16) -> u16 {
        let points = self.clear().points.expect("the list's points");
        let rows = self.clear().rows;
        let mask = self.clear_mask();
        self.bonus_names(&rows, from, mask, 0);
        let r = self.game.vs.state.x24C.clone();
        let (mut entry, mut prev) = (i32::from(from), -999);
        for i in 0..7 {
            let idx = self.bonus_at_or_before(&r, entry as u16, mask, 0);
            let val = self.game.decision_score(&r, idx, i32::from(mask), 0);
            if prev == idx || idx < 0 {
                break;
            }
            if self.clear().shown[i] != val {
                self.clear_points(points, i as i32, val);
                self.clear_mut().shown[i] = val;
            }
            prev = idx;
            entry = idx + 1;
        }
        let total = self.bonus_total(&r, i32::from(mask), 0);
        self.clear_points(points, 7, total);
        self.bonus_at_or_before(&r, from, mask, 0) as u16
    }

    /// Moves a text to `at`.
    fn clear_place(&mut self, t: Option<TextId>, at: Vec3) {
        if let Some(t) = t {
            self.game.sis.text_mut(t).pos = at;
        }
    }

    /// A new value text in place of `old` (font 0, the screen's canvas), aligned right.
    fn clear_value_text(&mut self, old: Option<TextId>) -> TextId {
        if let Some(t) = old {
            self.destroy_text(t);
        }
        let t = self.create_text_buffer(0, 0);
        self.game.sis.text_mut(t).default_alignment = 2;
        t
    }

    /// The time and damage bonuses' texts, `fn_8017F608`: their labels, and each value
    /// redrawn while it changes, as the seconds by the multiplier for its first second;
    /// all placed by the time bonus's label's joint.
    #[doc(alias = "fn_8017F608")]
    fn clear_bonus_texts(&mut self) {
        let data = self.data;
        if self.clear().texts[0].is_none() {
            let t = self.create_text(0, 0, Vec3::new(0.0, 0.0, 0.0), 6.0, 300.0);
            self.game.sis.reset_text(data, t, 2);
            self.clear_mut().texts[0] = Some(t);
        }
        if self.clear().texts[1].is_none() {
            let t = self.create_text(0, 0, Vec3::new(0.0, 0.0, 0.0), 6.0, 300.0);
            self.clear_mut().texts[1] = Some(t);
            let msg = if self.clear().x11A != 0 { 4 } else { 3 };
            self.game.sis.reset_text(data, t, msg);
        }
        let c = self.clear();
        if c.xD0 != c.xD4 {
            self.clear_mut().xD8 += 1;
            let t = self.clear_value_text(self.clear().texts[2]);
            self.clear_mut().texts[2] = Some(t);
            self.game.sis.text_mut(t).pos.z = -10.0;
            let c = self.clear();
            let (xd0, xd8, x108) = (c.xD0, c.xD8, i32::from(c.x108));
            if !self.clear_time_shown() {
                let s = self.game.sis.add_string(t, 0.0, 0.0, Some(MINUS));
                self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.065));
                self.clear_mut().xD4 = xd0;
            } else if xd8 < 0x3C {
                self.game.sis.text_mut(t).default_kerning = 1;
                let v = [int(xd0 / x108), TIMES.to_vec(), int(x108)].concat();
                let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&v));
                self.game
                    .sis
                    .set_scale(data, t, s, reg(0.089_999_996), reg(0.065));
            } else {
                let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&int(xd0)));
                self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.065));
                self.clear_mut().xD4 = xd0;
            }
        }
        let c = self.clear();
        if c.x11A == 0 && c.xDC != c.xE0 {
            self.clear_mut().xE4 += 1;
            let t = self.clear_value_text(self.clear().texts[3]);
            self.clear_mut().texts[3] = Some(t);
            let c = self.clear();
            let (xdc, xe4, x10a) = (c.xDC, c.xE4, i32::from(c.x10A));
            if xe4 < 0x3C {
                self.game.sis.text_mut(t).default_kerning = 1;
                let v = [int(xdc / x10a), TIMES.to_vec(), int(x10a)].concat();
                let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&v));
                self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.065));
            } else {
                let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&int(xdc)));
                self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.065));
                self.clear_mut().xE0 = xdc;
            }
        }
        let j = self.clear().joints[3];
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            data,
            j,
            Some(Vec3::new(-41.0, -0.25, 0.0)),
        );
        let (x, ny, z) = (reg(p.x), fp::fneg(reg(p.y)), p.z);
        let t = self.clear().texts;
        self.clear_place(t[1], Vec3::new(mem(x), mem(ny), z));
        let at = |dx: f64, dy: f64| Vec3::new(mem(fp::fadds(dx, x)), mem(fp::fsubs(ny, dy)), z);
        self.clear_place(t[0], at(reg(0.1), 6.0));
        self.clear_place(t[2], at(6.5, 33.5));
        self.clear_place(t[3], at(6.5, 27.5));
    }

    /// The score's count, `fn_8017FA1C`: once its joint shows, toward the new score by the
    /// step a frame (done once within a step), redrawn while it changes and placed by the
    /// joint; 0 while hidden.
    #[doc(alias = "fn_8017FA1C")]
    fn clear_score_text(&mut self) -> i32 {
        let data = self.data;
        let j = self.clear().joints[2].expect("the score's joint");
        if self.game.objs.jobjs[j].flags & HIDDEN != 0 {
            if let Some(t) = self.clear_mut().texts[5].take() {
                self.destroy_text(t);
            }
            let c = self.clear_mut();
            c.x100 = c.xFC.wrapping_sub(1);
            return 0;
        }
        let c = self.clear_mut();
        let target = c.x104;
        let step = c.step;
        let diff = target.wrapping_sub(c.xFC).wrapping_abs();
        if diff <= step.wrapping_abs() {
            c.xFC = target;
            c.x115 = 1;
        } else {
            c.xFC = c.xFC.wrapping_add(step);
            c.x115 = 0;
        }
        if c.xFC != c.x100 {
            let t = self.clear_value_text(self.clear().texts[5]);
            self.clear_mut().texts[5] = Some(t);
            let v = self.clear().xFC;
            let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&int(v)));
            self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.08));
            self.clear_mut().x100 = v;
        }
        if self.clear().texts[5].is_some() {
            let p = crate::lb::joint::world_position(&mut self.game.objs, data, Some(j), None);
            let y = fp::fsubs(fp::fneg(reg(p.y)), reg(30.6));
            let x = fp::fadds(16.0, reg(p.x));
            let t = self.clear().texts[5];
            self.clear_place(t, Vec3::new(mem(x), mem(y), p.z));
        }
        1
    }

    /// The bonuses' total, `fn_8017FBA4`: once its joint shows, the match's points for 12
    /// frames, then the two bonuses' sum to frame 0x20, then the total, scaled with the
    /// joint's parent and placed by the joint.
    #[doc(alias = "fn_8017FBA4")]
    fn clear_total_text(&mut self) {
        let data = self.data;
        let j = self.clear().joints[1].expect("the total's joint");
        if self.game.objs.jobjs[j].flags & HIDDEN != 0 {
            if let Some(t) = self.clear_mut().texts[4].take() {
                self.destroy_text(t);
            }
            let c = self.clear_mut();
            c.xF4 = c.xF0.wrapping_sub(1);
            return;
        }
        let c = self.clear();
        if c.xF0 != c.xF4 {
            let old = c.texts[4];
            if let Some(t) = old {
                self.destroy_text(t);
            }
            let t = self.create_text_buffer(0, 0);
            self.clear_mut().texts[4] = Some(t);
            let v = self.clear().xF0;
            let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&int(v)));
            self.game.sis.set_scale(data, t, s, reg(0.1), reg(0.065));
            self.clear_mut().xF4 = v;
        }
        let parent = self.game.objs.jobjs[j].parent.expect("jobj.h: jobj");
        let (sx, sy) = {
            let p = &self.game.objs.jobjs[parent];
            (reg(p.scale.x), reg(p.scale.y))
        };
        let t = self.clear_value_text(self.clear().texts[4]);
        self.clear_mut().texts[4] = Some(t);
        let c = self.clear_mut();
        c.xF8 = c.xF8.wrapping_add(1);
        let (xf8, xcc, sum, xf0) = (c.xF8, c.xCC, c.xDC.wrapping_add(c.xD0), c.xF0);
        let plus = |v: i32| [PLUS, b" ", &int(v)].concat();
        let s = if xf8 < 0xC {
            plus(xcc)
        } else if xf8 < 0x20 {
            plus(sum)
        } else if xf0 >= 0 {
            plus(xf0)
        } else {
            int(xf0)
        };
        let s = self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
        let (scx, scy) = (fp::fmuls(reg(0.1), sx), fp::fmuls(reg(0.065), sy));
        self.game.sis.set_scale(data, t, s, scx, scy);
        let p = crate::lb::joint::world_position(&mut self.game.objs, data, Some(j), None);
        let y = fp::fsubs(fp::fneg(reg(p.y)), 31.0);
        let x = fp::fadds(8.5, reg(p.x));
        self.clear_place(Some(t), Vec3::new(mem(x), mem(y), p.z));
    }

    /// The blur's callback before each draw, `fn_8017FE54`: the frame copied (and cleared),
    /// blurred wider as the screen grows (`lb_800138D8`), tinted more as it shows.
    #[doc(alias = "fn_8017FE54")]
    pub fn clear_blur_callback(&mut self, _gobj: GObjId) {
        // `lb_800122C8(ev->x1C, 0, 0, true)`: the GPU's pixels in the heap's image.
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::BlurCopy);
        let buffer = self.clear().buffer;
        if buffer != 0 {
            let len = crate::hsd::heap::tex_buffer_size(640, 480, 5).next_multiple_of(32);
            crate::hsd::heap::with(|h| {
                h.image
                    .hold(buffer, buffer + len, crate::hsd::image::Src::RenderTarget)
            });
        }
        let c = self.clear();
        let size = fp::fctiwz(fp::fmuls(120.0, reg(c.x10C))).wrapping_add(1) as i8;
        let tint = fp::fmsubs(reg(0.0225), fp::frsp(f64::from(c.x110)), reg(0.175));
        let b = self.game.blur.as_mut().expect("the blur's data");
        b.mode = 1;
        b.blur_size = size as u8;
        let mut t = mem(tint);
        if reg(t) < reg(0.05) {
            t = 0.0;
        }
        if reg(t) > 1.0 {
            t = 1.0;
        }
        b.tint = Some(t);
    }

    /// The screen model's process, `fn_8017FF1C`: its animation, the bonuses' and the
    /// score's texts, the list (from frame 0x2A: its rows, its arrows, scrolling down every
    /// 30 frames until moved, and up and down by the stick or the pad), the targets' icons
    /// one every other frame from frame 0x14, the count's tick, A to finish the count, Start
    /// after frame 0x3E to end the screen, the blur's growth and the frame count.
    #[doc(alias = "fn_8017FF1C")]
    pub fn clear_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("the clear screen's model holds {other:?}"),
        };
        jobj::anim_all(&mut self.game.objs, data, &mut ParticleHooks, Some(j));
        if self.clear().x118 == 0 {
            self.clear_bonus_texts();
        }
        let result = self.clear_score_text();
        self.clear_total_text();
        let r = self.game.vs.state.x24C.clone();
        if self.clear().x117 != 0 && self.clear().x110 > 0x29 {
            let first = self.clear_list(self.clear().first);
            self.clear_mut().first = first;
            let mask = self.clear_mask();
            let below = self.bonuses_from(&r, first, mask, 0) > 7;
            self.clear_mut().x11F = u8::from(!below);
            let above = self.bonus_before(&r, first, mask, 0) < 0;
            self.clear_mut().x11E = u8::from(above);
            let c = self.clear();
            if c.x110 % 30 == 0 && c.moved == 0 {
                if self.bonuses_from(&r, c.first, mask, 0) > 7 {
                    self.clear_mut().first = c.first.wrapping_add(1);
                } else {
                    let c = self.clear_mut();
                    c.moved = 1;
                    c.moved_at = c.x110;
                }
            }
            let port = self.game.player_port(0) as u8;
            let pressed = self.game.buttons_triggered(port) | self.game.buttons_repeated(port);
            let first = self.clear().first;
            if pressed & DOWN != 0 {
                if self.bonus_after(&r, first, mask, 0) > 0
                    && self.bonuses_from(&r, first, mask, 0) > 7
                {
                    let next = self.bonus_after(&r, first, mask, 0) as u16;
                    let c = self.clear_mut();
                    c.first = next;
                    c.moved = 1;
                    c.moved_at = c.x110;
                }
            } else if pressed & UP != 0 && self.bonus_before(&r, first, mask, 0) >= 0 {
                let prev = self.bonus_before(&r, first, mask, 0) as u16;
                let c = self.clear_mut();
                c.first = prev;
                c.moved = 1;
                c.moved_at = c.x110;
            }
        }
        let c = self.clear();
        if c.x11A != 0
            && c.x110 > 0x14
            && c.x11D < c.x11C
            && fp::fctiwz(fp::fmuls(reg(0.5), fp::frsp(f64::from(c.x110 - 0x14))))
                > i32::from(c.x11D)
        {
            let [icon] =
                super::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[i32::from(c.x11D) + 7]);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, icon, HIDDEN);
            self.clear_mut().x11D += 1;
        }
        if self.clear().x115 == 0 && result != 0 {
            self.count_tick();
        }
        let human = |r: &Running, i: i32| r.game.player_pkind(i) == PKIND_HUMAN;
        let trigger = |r: &Running, i: i32| {
            let port = r.game.player_port(i) as u8;
            r.game.pads.master[usize::from(port)].trigger
        };
        for i in 0..6 {
            if human(self, i) && trigger(self, i) & PAD_A != 0 {
                let c = self.clear_mut();
                c.xFC = c.x104;
                c.x115 = 1;
                break;
            }
        }
        if self.clear().x110 > 0x3E {
            for i in 0..6 {
                if human(self, i) && trigger(self, i) & PAD_START != 0 {
                    let c = self.clear_mut();
                    c.xFC = c.x104;
                    c.x116 = 1;
                    self.lb_audio_call(1);
                    break;
                }
            }
        }
        let c = self.clear();
        let (up, down) = (c.joints[7], c.joints[8]);
        let (hide_up, hide_down) = (c.x11E != 0, c.x11F != 0);
        let jobjs = &mut self.game.objs.jobjs;
        for (j, hide) in [(up, hide_up), (down, hide_down)] {
            if hide {
                jobj::set_flags_all(jobjs, j, HIDDEN);
            } else {
                jobj::clear_flags_all(jobjs, j, HIDDEN);
            }
        }
        let c = self.clear_mut();
        if reg(c.x10C) < 1.0 {
            c.x10C = mem(fp::fadds(reg(c.x10C), reg(0.05)));
        } else {
            c.x10C = 1.0;
        }
        if c.x110.wrapping_add(0x1_0000) != 0xFFFF {
            c.x110 = c.x110.wrapping_add(1);
        }
    }

    /// The count's tick, each fourth frame (`fn_80168F2C`, `lbl_804D659A`).
    #[doc(alias = "fn_80168F2C")]
    pub fn count_tick(&mut self) {
        let w = &mut self.game.endings.tick_wait;
        if *w == 0 {
            *w = 3;
            self.sfx_play_track(0x7E, 0x7F, 0x40, 1);
        } else {
            *w -= 1;
        }
    }

    /// The screen's model, `fn_801803FC`: its GObj with the process, the scene's model with
    /// its animations at frame 0, its joints, the list's parts hidden without a list, the
    /// targets' icons or the time bonus's parts hidden by the stage, the plate's texture at
    /// the continue flag's frame, held, and the texts' first frame.
    #[doc(alias = "fn_801803FC")]
    #[doc(alias = "fn_801689E4")]
    fn clear_model(&mut self) {
        let data = self.data;
        let gm = &mut *self.game;
        let gobj = gm.world.create(0xE, 0xE, 0);
        gm.one_player.clear.gobj = Some(gobj);
        let model = gm.one_player.clear.model;
        let j = jobj::load_joint(&mut gm.objs, data, model.joint).unwrap_or_else(|| {
            panic!(
                "the original's assertion failed: gmregclear.c:1074 (a jobj for gmRegClearAddModel)"
            )
        });
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        gm.world.setup_proc(gobj, CodeRef(PROC), 0x11);
        let first = |l: Option<DataRef>| l.and_then(|l| data.ptr(l));
        jobj::add_anim_all(
            &mut gm.objs,
            data,
            Some(j),
            first(model.anims),
            first(model.matanims),
            first(model.shapeanims),
        );
        jobj::req_anim_all(&mut gm.objs, Some(j), 0.0);
        jobj::anim_all(&mut gm.objs, data, &mut ParticleHooks, Some(j));
        let joints = super::pause::tree_nodes(&gm.objs.jobjs, Some(j), &JOINTS);
        let c = &mut gm.one_player.clear;
        c.joints = joints;
        let jobjs = &mut gm.objs.jobjs;
        if c.x117 == 0 {
            for k in [4, 7, 8] {
                jobj::set_flags_all(jobjs, joints[k], HIDDEN);
            }
        }
        if c.x11A != 0 {
            for i in 0..10 {
                let [icon] = super::pause::tree_nodes(jobjs, Some(j), &[i + 7]);
                jobj::set_flags_all(jobjs, icon, HIDDEN);
            }
            jobj::set_flags_all(jobjs, joints[3], HIDDEN);
        } else {
            jobj::set_flags_all(jobjs, joints[6], HIDDEN);
            if c.x118 != 0 {
                jobj::set_flags_all(jobjs, joints[5], HIDDEN);
            }
        }
        let frame = if c.x114 != 0 { 1.0 } else { 0.0 };
        let plate = joints[0].expect("the plate's joint");
        let objs = &mut gm.objs;
        let d = objs.jobjs[plate].dobj().expect("the plate's dobj");
        let m = objs.dobjs[d].mobj.expect("the plate's mobj");
        let t = objs.mobjs[m].tobj.expect("the plate's tobj");
        let a = objs.tobjs[t].aobj;
        crate::hsd::aobj::set_current_frame(&mut objs.aobjs, &mut objs.fobjs, a, frame);
        crate::hsd::aobj::set_rate(&mut objs.aobjs, a, 0.0);
        if self.clear().x118 == 0 {
            self.clear_bonus_texts();
        }
        self.clear_total_text();
        self.clear_score_text();
    }

    /// The stage clear screen's setup, `fn_80180630`: its state from the match (the stage's
    /// kind `kind`: 1 a targets stage, 3 the race, 2 a boss's), the bonuses tallied and
    /// listed, the score's parts and the new score (at most 999,999,999) with its step, the
    /// scene's archive, light, camera and text canvas, the model, the jingle, the camera's
    /// swing, the frame's copy and its blur, the coins counted, and the list's texts.
    #[doc(alias = "fn_80180630")]
    #[doc(alias = "fn_80168A6C")]
    #[doc(alias = "lb_800121FC")]
    pub fn clear_setup(&mut self, score: i32, points: i32, kind: i32, cont: bool) {
        let special = 0;
        let coins = self.game.vs.state.x24C.player_standings[0].xE;
        self.game.one_player.clear = Clear {
            xD4: -1,
            xE0: -1,
            xEC: -1,
            xF4: -1,
            x100: -1,
            x11E: 1,
            x11F: 1,
            x108: 100,
            x10A: 100,
            x114: u8::from(cont),
            ..Clear::default()
        };
        match kind {
            1 => {
                // `Ground_801C1DE4`: the targets left and the stage's targets.
                let (left, total) = (
                    i32::from(self.game.stage_info.x6D4),
                    i32::from(self.game.stage_info.x6D2),
                );
                let c = self.clear_mut();
                c.x11A = 1;
                c.x11C = total.wrapping_sub(left) as u8;
                c.x108 = 200;
                if left == 0 {
                    c.x11B = 1;
                }
            }
            3 => {
                self.clear_mut().x118 = 1;
                if self.game.vs.state.match_result == OUTCOME_1P_BONUS_STAGE_END {
                    super::super::code::unported_call(
                        "the race's clear (grPushOn_80219204's score and coins)",
                    );
                }
            }
            2 => self.clear_mut().x119 = 1,
            _ => {}
        }
        self.tally_all_bonuses();
        let r = self.game.vs.state.x24C.clone();
        let c = self.clear();
        let list = c.x11A == 0
            && c.x118 == 0
            && c.x119 == 0
            && self.bonuses_from(&r, 0, self.clear_mask(), 0) != 0;
        let c = self.clear_mut();
        c.x117 = u8::from(list);
        c.moved = u8::from(!list);
        if c.x118 == 0 {
            let d0 = self.clear_time_bonus();
            self.clear_mut().xD0 = d0;
            let dc = self.clear_damage_bonus(&r);
            self.clear_mut().xDC = dc;
        }
        if self.clear().x117 != 0 {
            let e8 = self.clear_bonus_points();
            self.clear_mut().xE8 = e8;
        }
        let c = self.clear_mut();
        c.xF0 = c
            .xE8
            .wrapping_add(c.xDC.wrapping_add(c.xD0).wrapping_add(special));
        c.xFC = score.wrapping_add(points);
        c.xCC = points;
        let total = score.wrapping_add(c.xCC).wrapping_add(c.xF0);
        c.x104 = total.clamp(0, 999_999_999);
        c.step = c.x104.wrapping_sub(score.wrapping_add(points)) / 10;
        // The scene's archive and its description (`fn_80180630`'s, `fn_80168A6C`'s copy).
        let (a, file, block) = self.load_archive_blocks("GmRegClr.dat");
        let scene = self
            .data
            .get(a)
            .public("ScGamRegClear_scene_data")
            .map(|offset| DataRef { archive: a, offset })
            .expect("GmRegClr.dat's ScGamRegClear_scene_data");
        let data = self.data;
        let models = data.ptr(scene);
        let model = models
            .and_then(|m| data.ptr(m))
            .map_or(ModelDesc::default(), |m| ModelDesc {
                joint: data.ptr(m),
                anims: data.ptr(m.at(4)),
                matanims: data.ptr(m.at(8)),
                shapeanims: data.ptr(m.at(0xC)),
            });
        let cameras = data.ptr(scene.at(4));
        let c = self.clear_mut();
        c.archive = Some(a);
        c.blocks = (file, block);
        c.model = model;
        c.lights = data.ptr(scene.at(8));
        c.cameras = [
            cameras.and_then(|l| data.ptr(l)),
            cameras.and_then(|l| data.ptr(l.at(4))),
        ];
        c.fog = data.ptr(scene.at(0xC));
        // The light and the camera.
        let gm = &mut *self.game;
        let light = gm.world.create(0xB, 3, 0);
        let lights = gm.one_player.clear.lights.expect("the scene's lights");
        let l = crate::game::gr::setup::load_light_list(gm, data, lights);
        gm.world
            .set_obj(light, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        gm.world
            .setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 0xA, 0);
        let cam = gm.world.create(0xE, 0xE, 0);
        let desc = gm.one_player.clear.cameras[0];
        let co = crate::hsd::cobj::load_desc(&mut gm.objs, data, desc).expect("a camera");
        gm.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(co));
        gm.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 8, false);
        gm.world.gobj_mut(cam).gxlink_prios = 0x4C00;
        self.add_canvas(0, Some(cam), 9, 0xD, 0, 0xE, 0, 0x13);
        let name = if self.game.gm.unlocks.is_us() {
            "SdClr.usd"
        } else {
            "SdClr.dat"
        };
        self.load_font_named(0, name, "SIS_ClearData");
        self.clear_model();
        self.game.endings.tick_wait = 0;
        let song = if self.game.objs.rng.below(2) != 0 {
            10
        } else {
            11
        };
        self.lb_play_music(song);
        super::super::camera::set_up_clear(self.game, 0);
        // The frame's copy (`lb_800121FC`: 640 by 480 RGB5A3, from the heap) and its blur.
        let len = crate::hsd::heap::tex_buffer_size(640, 480, 5).next_multiple_of(32);
        let c = self.clear_mut();
        c.image = HSD_ImageDesc {
            image_ptr: super::super::hud::magnify::TexBuffer(true),
            width: 640,
            height: 480,
            format: 5,
            mipmap: 0,
            minLOD: 0.0,
            maxLOD: 0.0,
        };
        c.buffer = crate::hsd::heap::mem_alloc(len);
        let blur = self.blur_create(2, 0x32, [0.0, 0.0], [1.0, 1.0]);
        self.clear_mut().blur = Some(blur);
        let b = self.game.blur.as_mut().expect("the blur's data");
        b.mode = 1;
        b.blur_size = 1;
        b.callback = Some(CodeRef(BLUR_CALLBACK));
        if self.game.vs.start.x1_1 && coins != 0 {
            if self.clear().x118 == 0 {
                self.game
                    .start_coin_counter(0x5A, 0x1AE, i32::from(coins), 5);
            } else {
                self.game
                    .start_coin_counter(0x86, 0xC8, i32::from(coins), 5);
            }
        }
        self.game.vs.state.x24C.player_standings[0].xE = coins;
        self.clear_list_texts(264.0, 211.0);
    }

    /// Whether Start ended the screen, `fn_80180AC0`.
    #[doc(alias = "fn_80180AC0")]
    pub fn clear_done(&self) -> bool {
        self.clear().x116 == 1
    }

    /// The clear's start in `fn_8016D634`, once a 1P stage is won: the hazard sign paused,
    /// the HUD freed (`ifAll_802F3A64`), the match counted for the mode, the results compiled
    /// into the match's own (with the mode's, and the players' bonuses but in a match without
    /// a contest) and the match marked over, then the clear screen.
    pub fn clear_start(&mut self) {
        self.game.pause_hazard();
        self.free_hud();
        self.game.clear_db_pause_flag(4);
        self.game.set_db_pause_flag(5);
        self.game.vs.state.unk_9 = 1;
        if self.game.vs.state.match_over == 0 {
            let mut r = std::mem::take(&mut self.game.vs.state.x24C);
            r.is_teams = self.game.vs.start.is_teams;
            r.outcome = self.game.vs.state.match_result;
            self.compile_results_into(&mut r);
            self.single_player_results_into(&mut r);
            let o = self.game.vs.state.match_result;
            if o != OUTCOME_NO_CONTEST && o != OUTCOME_RETRY {
                self.match_bonuses(&mut r);
            }
            self.game.vs.state.x24C = r;
            self.game.vs.state.match_over = 1;
        }
        let s = &self.game.vs.start;
        let (score, kind, cont) = (s.x18 as i32, i32::from(s.x9), s.x4_5);
        self.clear_setup(score, 0, kind, cont);
    }
}
