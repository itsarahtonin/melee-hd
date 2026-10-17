//! A player's per-frame bonus records (`plbonuslib.c`'s `pl_8003FAA8` and the windowed
//! checks of `plbonus.c`): the counters, the bonuses a frame window decides, the inputs and
//! the distances the results screen's bonuses are scored from. Each record is a field of
//! `pl_StaleMoveTableExt_t`, by its offset there (see [`super::stats`]).

use gekko_fp as fp;

use super::super::state::{Game, Running};
use super::super::structs::_StaticPlayer;
use crate::fpu::{mem, reg};
use crate::vec::Vec3;

/// A bit of `xDD0` or `xDD1`, `bit0` the top one.
const fn bit(n: u8) -> u8 {
    0x80 >> n
}

/// `It_Kind_ScBall` and `It_Kind_Dosei`.
const IT_KIND_SCBALL: i32 = 0x1E;
const IT_KIND_DOSEI: i32 = 7;
/// `It_Kind_L_Gun_Ray`, past the common items.
const IT_KIND_L_GUN_RAY: i32 = 0x23;

/// `HSD_PAD_A`, `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;

/// Whether the match counts frames for the records, `pl_Verify_gm_8016AEDC`: its frame
/// count is neither 0 nor -2.
fn counting(game: &Game) -> bool {
    let n = game.vs_frame_count();
    n != 0 && n != (-2i32) as u32
}

/// `pl_CalculateAverage`.
fn average(val: f64, total: f64) -> f32 {
    assert!(
        total != 0.0,
        "the original's assertion failed: total != 0.0F"
    );
    mem(fp::fdivs(val, total))
}

/// The item log's kind for an item, `match_item_kind`, or -1 for one it doesn't log.
fn match_item_kind(kind: i32) -> i32 {
    if (0..IT_KIND_L_GUN_RAY).contains(&kind) {
        return kind;
    }
    match kind {
        205 => 0x23,
        225 => 0x24,
        226 => 0x25,
        40 => 0x26,
        _ => -1,
    }
}

fn inc(p: &mut _StaticPlayer, off: usize) {
    p.set_ext_u32(off, p.ext_u32(off).wrapping_add(1));
}

fn dec_to_zero(p: &mut _StaticPlayer, off: usize) {
    if p.ext_u32(off) != 0 {
        p.set_ext_u32(off, p.ext_u32(off).wrapping_sub(1));
    }
}

fn set_bit(p: &mut _StaticPlayer, off: usize, n: u8, on: bool) {
    let b = p.ext_u8(off);
    p.set_ext_u8(off, if on { b | bit(n) } else { b & !bit(n) });
}

fn add_f32(p: &mut _StaticPlayer, off: usize, v: f64) {
    p.set_ext_f32(off, mem(fp::fadds(reg(p.ext_f32(off)), v)));
}

/// `|v|` as the original writes it, on a register.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

/// Where a windowed check keeps its flag: a point bonus's count (`x904[kind]`), or a word
/// of the records.
#[derive(Clone, Copy, melee_derive::StateEq)]
pub(super) enum Flag {
    Bonus(i32),
    Record(usize),
}

impl Game {
    /// Records that the player's fighter went airborne or landed with a jump left
    /// (`xDD0` bit 0), `pl_8003FC44`; Nana's don't count.
    #[doc(alias = "pl_8003FC44")]
    pub fn record_jump_left(&mut self, slot: i32, sub: i32) {
        let p = self.player_mut(slot);
        if sub != 0 {
            return;
        }
        set_bit(p, 0xDD0, 0, true);
    }
}

impl Running<'_> {
    /// A broken shield, bonus 0x40 (Nana's don't count), `pl_8003E0E8`.
    // Ports `pl_8003E0E8`; aliased once a check reaches it.
    pub fn record_shield_break(&mut self, slot: i32, sub: i32) {
        if sub != 0 {
            return;
        }
        self.count_bonus(slot, 0x40);
    }

    /// A shield broken by an attack, `pl_8003E058`: bonus 0x3F for its player, and 0x3E for
    /// the attacker's (`by`, the fighter's `x19BC`) unless none or a teammate's.
    #[doc(alias = "pl_8003E058")]
    pub fn record_shield_broken_by(&mut self, by: i32, slot: i32) {
        self.count_bonus(slot, 0x3F);
        if by == 6 {
            return;
        }
        if self.game.vs_is_teams() && self.game.player_team(by) == self.game.player_team(slot) {
            return;
        }
        self.count_bonus(by, 0x3E);
    }

    /// A shield dropped at no more than `x84` health, bonus 0x41, `pl_8003E114`.
    // Ports `pl_8003E114`; aliased once a check reaches it.
    pub fn record_shield_drop(&mut self, slot: i32, health: f32) {
        if reg(health) <= reg(self.bonus_param_f32(0x84)) {
            self.set_flag(slot, Flag::Bonus(0x41), 1);
        }
    }

    /// A hit's knockback of at least `pl_804D6470` x98 counts in the player's stats
    /// (`xCE4`), `pl_80040270`; a sub-fighter's doesn't.
    #[doc(alias = "pl_80040270")]
    pub fn record_knockback(&mut self, slot: i32, sub: bool, kb: f32) {
        if sub {
            return;
        }
        if reg(kb) >= reg(self.bonus_param_f32(0x98)) {
            let t = self.game.player_stale_moves(slot);
            t.xCE4 = t.xCE4.wrapping_add(1);
        }
    }

    /// A ledge another fighter holds, `pl_8003FFDC`: unless it's the player's own, bonus 0x46
    /// for the holder when the other last caught a different ledge (`ft_80087A98`).
    #[doc(alias = "pl_8003FFDC")]
    pub fn record_ledge_steal(
        &mut self,
        slot: i32,
        _sub: i32,
        other: i32,
        other_sub: i32,
        ledge: i32,
    ) {
        let g = self
            .game
            .player_entity_at(other, other_sub)
            .expect("a ledge steal without the other fighter");
        if slot != other && ledge != self.game.fighter(g).x213C {
            self.count_bonus(slot, 0x46);
        }
    }

    /// A powershield counts in the player's stats (`xCD8`), `pl_8003E150`, Nana's too.
    #[doc(alias = "pl_8003E150")]
    pub fn record_powershield(&mut self, slot: i32) {
        let t = self.game.player_stale_moves(slot);
        t.xCD8 = t.xCD8.wrapping_add(1);
    }

    /// A throw that came within `pl_804D6470` x4C of the victim's mash timer sets the quick
    /// throw's bonus (`pl_80040614`'s test, `pl_80038788` with 0x2D).
    pub fn record_throw_timer(&mut self, slot: i32, timer: f32) {
        if reg(timer) <= reg(self.bonus_param_f32(0x4C)) {
            self.set_flag(slot, Flag::Bonus(0x2D), 1);
        }
    }

    /// A Bob-omb the player threw or caught joins its last five (`x8D8`, a ring) by its spawn
    /// number, unless it's there already, `pl_800419AC`: the one it pushes out, if it never
    /// hit, counts in the bonuses (`fn_80041BC8`).
    #[doc(alias = "pl_800419AC")]
    pub fn bombhei_thrown(&mut self, slot: i32, _sub: i32, spawn: u16) {
        let t = &self.game.player_stale_moves(slot).x8D8;
        if t.x4.iter().any(|e| e.x4_b1 && u32::from(spawn) == e.x0) {
            return;
        }
        let at = t.x0 as usize;
        let (hit, used) = (t.x4[at].x4_b0, t.x4[at].x4_b1);
        self.count_missed_bombhei(slot, hit, used);
        let t = &mut self.game.player_stale_moves(slot).x8D8;
        let e = &mut t.x4[at];
        e.x0 = u32::from(spawn);
        e.x4_b0 = false;
        e.x4_b1 = true;
        t.x0 = if t.x0 == 4 { 0 } else { t.x0.wrapping_add(1) };
    }

    /// A Bob-omb of the player's that never hit counts in the bonuses (0xC8), `fn_80041BC8`.
    #[doc(alias = "fn_80041BC8")]
    pub fn count_missed_bombhei(&mut self, slot: i32, hit: bool, used: bool) {
        if used && !hit {
            self.count_bonus(slot, 0xC8);
        }
    }

    /// The player's run of containers giving each kind (`x7AC`, by the kind's log entry), from
    /// the kinds one container gave, `pl_8003E4A4`: a kind given again extends its run, and a
    /// run reaching `pl_804D6470` x138 scores bonus 0x9D; every other kind's run ends. (The
    /// original's kind outside the log, -1, passes its bound check and marks the stack word
    /// before its marks, which nothing reads; no caller hands it one.)
    #[doc(alias = "pl_8003E4A4")]
    pub fn count_items_got(&mut self, slot: i32, kinds: &[i32]) {
        // `match_item_kind`: the common items, and four others past them.
        let entry = |k: i32| -> i32 {
            if (0..IT_KIND_L_GUN_RAY).contains(&k) {
                k
            } else {
                match k {
                    205 => 0x23,
                    225 => 0x24,
                    226 => 0x25,
                    40 => 0x26,
                    _ => -1,
                }
            }
        };
        let mut seen = [false; 39];
        for &k in kinds {
            let e = entry(k);
            if (0..39).contains(&e) {
                seen[e as usize] = true;
            }
        }
        let run = self.bonus_param(0x138);
        for (i, &s) in seen.iter().enumerate() {
            let t = &mut self.game.player_stale_moves(slot).x7AC[i];
            if s {
                *t = t.wrapping_add(1);
                if *t as u32 == run {
                    self.set_point(slot, 0x9D, 1);
                    self.game.player_stale_moves(slot).x7AC[i] = 0;
                }
            } else {
                *t = 0;
            }
        }
    }

    /// `pl_804D6470`'s word at `off`.
    pub(crate) fn bonus_param(&self, off: u32) -> u32 {
        self.data.u32(self.pl_data().at(off))
    }

    pub(crate) fn bonus_param_f32(&self, off: u32) -> f32 {
        self.data.f32(self.pl_data().at(off))
    }

    /// A point bonus's flag (`pl_80039418`), or a record's word.
    fn flag(&mut self, slot: i32, flag: Flag) -> u32 {
        let p = self.game.player_mut(slot);
        match flag {
            Flag::Bonus(kind) => p.stale_moves.x904[kind as usize],
            Flag::Record(off) => p.ext_u32(off),
        }
    }

    /// Sets a point bonus's flag (`setPointValue`; the original asserts the slot is a player's
    /// and the bonus counts points, which every caller's is), or a record's word.
    pub(super) fn set_flag(&mut self, slot: i32, flag: Flag, v: u32) {
        let p = self.game.player_mut(slot);
        match flag {
            Flag::Bonus(kind) => p.stale_moves.x904[kind as usize] = v,
            Flag::Record(off) => p.set_ext_u32(off, v),
        }
    }

    /// A bonus decided over windows of `period` frames, `pl_8003906C`: through the first
    /// window the flag goes up once `value` reaches `threshold`; at each later window's end it
    /// comes down unless `value` grew by `threshold` since the last (`last`, a record). Whether
    /// a window ended.
    #[doc(alias = "pl_8003906C")]
    #[allow(clippy::too_many_arguments)]
    fn bonus_window(
        &mut self,
        slot: i32,
        flag: Flag,
        period: u32,
        threshold: u32,
        value: u32,
        last: usize,
    ) -> bool {
        if !counting(self.game) {
            return false;
        }
        let n = self.game.vs_frame_count();
        if n <= period {
            if value >= threshold {
                self.set_flag(slot, flag, 1);
            }
            if period == n {
                self.game.player_mut(slot).set_ext_u32(last, value);
                return true;
            }
        } else if n.checked_rem(period).unwrap_or(n) == 0 {
            let cur = self.flag(slot, flag);
            let before = self.game.player_mut(slot).ext_u32(last);
            if cur as i32 != 0 && value.wrapping_sub(before) < threshold {
                self.set_flag(slot, flag, 0);
            }
            self.game.player_mut(slot).set_ext_u32(last, value);
            return true;
        }
        false
    }

    /// [`Self::bonus_window`] on a float, `pl_80039238`.
    #[doc(alias = "pl_80039238")]
    fn bonus_window_f32(
        &mut self,
        slot: i32,
        flag: Flag,
        period: u32,
        last: usize,
        threshold: f64,
        value: f64,
    ) -> bool {
        if !counting(self.game) {
            return false;
        }
        let n = self.game.vs_frame_count();
        if n <= period {
            if value >= threshold {
                self.set_flag(slot, flag, 1);
            }
            if period == n {
                self.game.player_mut(slot).set_ext_f32(last, mem(value));
                return true;
            }
        } else if n.checked_rem(period).unwrap_or(n) == 0 {
            let cur = self.flag(slot, flag) as i32;
            let before = reg(self.game.player_mut(slot).ext_f32(last));
            if cur != 0 && fp::fsubs(value, before) < threshold {
                self.set_flag(slot, flag, 0);
            }
            self.game.player_mut(slot).set_ext_f32(last, mem(value));
            return true;
        }
        false
    }

    /// The frame's bonus records for a player's fighter (`index` 1 is Nana, whose records
    /// mostly don't count), `pl_8003FAA8`: the counters, the windowed bonuses, the inputs,
    /// the distances, and the running average of the percent.
    #[doc(alias = "pl_8003FAA8")]
    pub fn frame_stats(&mut self, slot: i32, index: i32, pos: Vec3, prev: Vec3) {
        self.frame_counters(slot, index);
        self.frame_windows(slot, index);
        self.frame_inputs(slot, index);
        self.frame_distances(slot, index, pos, prev);
        if counting(self.game) && index == 0 {
            let g = self
                .game
                .player_entity_at(slot, index)
                .expect("the player's fighter");
            let percent = self.game.ft_percent(g);
            let n = self.game.vs_frame_count();
            let t = self.game.player_stale_moves(slot);
            let sum = fp::fmadds(
                reg(t.xC9C),
                fp::frsp(f64::from(n.wrapping_sub(1))),
                fp::frsp(f64::from(percent)),
            );
            t.xC9C = average(sum, fp::frsp(f64::from(n)));
        }
    }

    /// The frame's counters, `fn_8003EE2C`: frames since the last hit and the longest such
    /// run, frames in the air and on the ground, on steep slopes, nudged, off screen (and at
    /// which edge), holding a crate, the Screw Attack, Mr. Saturn or another logged item,
    /// and resized; the timers count down.
    #[doc(alias = "fn_8003EE2C")]
    fn frame_counters(&mut self, slot: i32, index: i32) {
        let g = self.game.player_entity_at(slot, index);
        if index == 1 || !counting(self.game) {
            return;
        }
        let g = g.expect("the player's fighter");
        let streak = self.bonus_param_f32(0x64);
        let steep = reg(self.bonus_param_f32(0x58));
        let edge = self.magnify_edge(slot);
        let heavy = self.ft_holds_heavy(g);
        let game = &*self.game;
        let fp = game.fighter(g);
        let airborne = fp.ground_or_air == super::super::ft::common::GA_AIR;
        let (x2072_b1, x2073) = (fp.x2070[2] & 0x40 != 0, fp.x2070[3]);
        let slope = (!airborne).then(|| reg(game.ft_ground_slope_angle(g)));
        let nudged = game.ft_is_being_nudged(g);
        let scball = game.ft_holds_item_kind(g, IT_KIND_SCBALL);
        let dosei = game.ft_holds_item_kind(g, IT_KIND_DOSEI);
        let item = fp.item_gobj.map(|it| {
            let kind = super::super::it::item(game, it).kind;
            match_item_kind(kind) != -1
                && super::super::it::lib26b1::use_kind(game, self.data, it) != 5
        });
        let changed = game.ft_80087aec(g);
        let p = self.game.player_mut(slot);
        inc(p, 0xD08);
        if p.ext_u32(0xD08) > p.ext_u32(0xD04) {
            p.set_ext_u32(0xD04, p.ext_u32(0xD08));
        }
        inc(p, 0xD10);
        if p.ext_u32(0xD10) > p.ext_u32(0xD0C) {
            p.set_ext_u32(0xD0C, p.ext_u32(0xD10));
        }
        inc(p, if airborne { 0xD20 } else { 0xD24 });
        if x2072_b1 {
            inc(p, 0xD18);
            if fp::frsp(f64::from(p.ext_u32(0xD18))) >= reg(streak) {
                inc(p, 0xD14);
            }
        } else {
            p.set_ext_u32(0xD18, 0);
        }
        if x2073 == 0x68 {
            inc(p, 0xD1C);
        }
        if let Some(s) = slope
            && abs(s) >= steep
        {
            inc(p, 0xD28);
        }
        if nudged {
            inc(p, 0xD2C);
        }
        if edge != 0 {
            inc(p, 0xD30);
        }
        set_bit(p, 0xDD0, 7, edge != 0);
        match edge {
            1 => set_bit(p, 0xDD0, 5, true),
            2 => set_bit(p, 0xDD0, 4, true),
            3 => set_bit(p, 0xDD0, 6, true),
            4 => set_bit(p, 0xDD0, 3, true),
            _ => {}
        }
        if heavy {
            inc(p, 0xD34);
        }
        if scball {
            inc(p, 0xD38);
        }
        if dosei {
            inc(p, 0xD3C);
        }
        match item {
            Some(logged) => {
                if logged {
                    inc(p, 0xD44);
                    if p.ext_u32(0xD44) > p.ext_u32(0xD40) {
                        p.set_ext_u32(0xD40, p.ext_u32(0xD44));
                    }
                }
            }
            None => {
                let held = p.ext_u32(0xD44);
                if held != 0 {
                    let shortest = p.ext_u32(0xD48);
                    if held < shortest || shortest == 0 {
                        p.set_ext_u32(0xD48, held);
                    }
                    p.set_ext_u32(0xD44, 0);
                }
            }
        }
        dec_to_zero(p, 0xD4C);
        dec_to_zero(p, 0xD50);
        dec_to_zero(p, 0xD54);
        inc(p, 0xD58);
        dec_to_zero(p, 0xD60);
        if changed {
            inc(p, 0xD68);
        }
    }

    /// The bonuses decided over frame windows, `fn_8003F294`: attacks of three kinds, hits
    /// landed and taken, attacks made, `xD70`, crates held, `xCD8`, the smash DI's distance,
    /// and A/B presses (whose window also settles the quickest press, `xDD1` bit 5).
    #[doc(alias = "fn_8003F294")]
    fn frame_windows(&mut self, slot: i32, index: i32) {
        if index == 1 {
            return;
        }
        let s = &self.game.player_stale_moves(slot).total_attack_count_struct;
        // `pl_800386D8`: `x358`'s `by_attack_hi`, at `0x3E8 + 4 * attack`, which runs past it
        // into the words that follow.
        let (hi_107, hi_112, hi_111) = (s.x594, s.x598[4], s.x598[3]);
        // `pl_800386E8`: the words at 0x190, 0x5AC and 0x5B0.
        let e8 = s.attacks.by_attack_counts[99]
            .wrapping_add(s.x598[5])
            .wrapping_add(s.x598[6]);
        let (hits, attacks) = (s.hits.total, s.attacks.total);
        let p = self.game.player_mut(slot);
        let (xd70, xd34, xcd8) = (p.ext_u32(0xD70), p.ext_u32(0xD34), p.stale_moves.xCD8);
        let (sdi_x, sdi_y) = (reg(p.stale_moves.xCDC), reg(p.stale_moves.xCE0));
        let presses = p.ext_u32(0xCF4);
        let windows = [
            (Flag::Bonus(0x3B), 0x70, hi_107, 0xD9C),
            (Flag::Bonus(0x45), 0x90, hi_112, 0xDA0),
            (Flag::Bonus(0x49), 0xA8, e8, 0xDA4),
            (Flag::Bonus(0x54), 0xC4, hi_111, 0xDA8),
            (Flag::Record(0xDC8), 0x18, hits, 0xDAC),
            (Flag::Bonus(0x13), 0x2C, attacks, 0xDB0),
            (Flag::Bonus(0x5C), 0xE0, xd70, 0xDB4),
            (Flag::Bonus(0x99), 0x130, xd34, 0xDB8),
            (Flag::Bonus(0x3D), 0x7C, xcd8 as u32, 0xDBC),
        ];
        for (flag, param, value, last) in windows {
            let (period, threshold) = (self.bonus_param(param), self.bonus_param(param + 4));
            self.bonus_window(slot, flag, period, threshold, value, last);
        }
        let distance =
            crate::math::sqrt_reg(fp::fadds(fp::fmuls(sdi_x, sdi_x), fp::fmuls(sdi_y, sdi_y)));
        let period = self.bonus_param(0x88);
        let threshold = reg(self.bonus_param_f32(0x8C));
        self.bonus_window_f32(slot, Flag::Bonus(0x44), period, 0xDC0, threshold, distance);
        let (period, threshold) = (self.bonus_param(0xB8), self.bonus_param(0xBC));
        if self.bonus_window(slot, Flag::Record(0xDCC), period, threshold, presses, 0xDC4) {
            let n = self.game.vs_frame_count();
            let p = self.game.player_mut(slot);
            let quickest = p.ext_u32(0xD5C);
            if period == n {
                if quickest <= period {
                    set_bit(p, 0xDD1, 5, true);
                }
            } else if p.ext_u8(0xDD1) & bit(5) != 0 && quickest > period {
                set_bit(p, 0xDD1, 5, false);
            }
            p.set_ext_u32(0xD5C, u32::MAX);
        }
    }

    /// The frame's inputs, `fn_8003F53C`: whether A and B were ever both let go (`xDD0`
    /// bit 1), whether L or R was ever pressed (bit 2), and A/B presses with the quickest
    /// frames between them.
    #[doc(alias = "fn_8003F53C")]
    fn frame_inputs(&mut self, slot: i32, index: i32) {
        let g = self.game.player_entity_at(slot, index);
        if index == 1 {
            return;
        }
        let g = g.expect("the player's fighter");
        let game = &*self.game;
        // `ftLib_IsInputDisabled`.
        if game.fighter(g).input_disabled {
            return;
        }
        let released = !game.ft_holds(g, PAD_A) && !game.ft_holds(g, PAD_B);
        let lr = game.ft_holds(g, PAD_LR);
        let pressed = game.ft_pressed(g, PAD_A) || game.ft_pressed(g, PAD_B);
        let p = self.game.player_mut(slot);
        if released {
            set_bit(p, 0xDD0, 1, true);
        }
        if lr {
            set_bit(p, 0xDD0, 2, true);
        }
        if pressed {
            inc(p, 0xCF4);
            let since = p.ext_u32(0xD58);
            if since < p.ext_u32(0xD5C) {
                p.set_ext_u32(0xD5C, since);
            }
            p.set_ext_u32(0xD58, 0);
        }
    }

    /// The frame's distances, `fn_8003F654`: how far knockback carried the fighter (and the
    /// farthest, for its attacker too), how far it walked, rose and fell under its own power,
    /// its average distance from its opponents and from the camera's center.
    #[doc(alias = "fn_8003F654")]
    fn frame_distances(&mut self, slot: i32, index: i32, pos: Vec3, prev: Vec3) {
        let g = self.game.player_entity_at(slot, index);
        if !counting(self.game) || index == 1 {
            return;
        }
        let g = g.expect("the player's fighter");
        if self.game.ft_is_dead(g) {
            return;
        }
        let (x, y) = (reg(pos.x), reg(pos.y));
        let (px, py) = (reg(prev.x), reg(prev.y));
        match self.game.ft_80087b34(g) {
            1 => {
                let dx = fp::fsubs(px, x);
                let dy = fp::fsubs(py, y);
                let dist = crate::math::sqrt_reg(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)));
                let attacker = self.game.ft_last_attacker(g);
                let p = self.game.player_mut(slot);
                add_f32(p, 0xD80, dist);
                if dist > reg(p.ext_f32(0xD84)) {
                    p.set_ext_f32(0xD84, mem(dist));
                }
                if attacker != 6 {
                    let o = self.game.player_mut(attacker);
                    if dist > reg(o.ext_f32(0xD88)) {
                        o.set_ext_f32(0xD88, mem(dist));
                    }
                }
            }
            0 => {
                let grounded =
                    self.game.fighter(g).ground_or_air == super::super::ft::common::GA_GROUND;
                let p = self.game.player_mut(slot);
                if grounded {
                    add_f32(p, 0xD74, abs(fp::fsubs(x, px)));
                } else {
                    let dy = fp::fsubs(y, py);
                    add_f32(p, if dy > 0.0 { 0xD78 } else { 0xD7C }, abs(dy));
                }
            }
            _ => {}
        }
        let mut sum = 0.0;
        let mut count = 0i32;
        for i in 0..6 {
            // `pl_CheckIfSameTeam`.
            let same_team =
                self.game.vs_is_teams() && self.game.player_team(i) == self.game.player_team(slot);
            if i != slot
                && !same_team
                && self.game.player_awake(i)
                && !self
                    .game
                    .ft_is_dead(self.game.player_entity(i).expect("the player's fighter"))
            {
                let other = self.game.player_pos(i);
                sum = fp::fadds(sum, abs(fp::fsubs(x, reg(other.x))));
                count = count.wrapping_add(1);
            }
        }
        let p = self.game.player_mut(slot);
        if count != 0 {
            let n = p.ext_u32(0xD90).wrapping_add(1);
            p.set_ext_u32(0xD90, n);
            let avg = reg(average(sum, fp::frsp(f64::from(count))));
            let total = fp::fmadds(
                reg(p.ext_f32(0xD8C)),
                fp::frsp(f64::from((n as i32).wrapping_sub(1))),
                avg,
            );
            p.set_ext_f32(0xD8C, average(total, fp::frsp(f64::from(n as i32))));
        }
        let n = p.ext_u32(0xD98).wrapping_add(1);
        p.set_ext_u32(0xD98, n);
        let cam = super::super::gr::stage::cam_center(self.game);
        let off = abs(fp::fsubs(x, reg(cam.x)));
        let p = self.game.player_mut(slot);
        let total = fp::fmadds(
            reg(p.ext_f32(0xD94)),
            fp::frsp(f64::from((n as i32).wrapping_sub(1))),
            off,
        );
        p.set_ext_f32(0xD94, average(total, fp::frsp(f64::from(n as i32))));
    }
}
