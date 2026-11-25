//! A player's per-frame bonus records (`plbonuslib.c`'s `pl_8003FAA8` and the windowed
//! checks of `plbonus.c`): the counters, the bonuses a frame window decides, the inputs and
//! the distances the results screen's bonuses are scored from. Each record is a field of
//! the stats tail ([`StatsExt`], the rest of `pl_StaleMoveTableExt_t`).

use gekko_fp as fp;

use super::super::state::{Game, Running};
use super::types::StatsExt;
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::vec::Vec3;

/// A bit of `xDD0` or `xDD1`, `bit0` the top one.
const fn bit(n: u8) -> u8 {
    0x80 >> n
}

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
    if (0..ItKind::COMMON_END.id()).contains(&kind) {
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

fn inc(v: &mut u32) {
    *v = v.wrapping_add(1);
}

fn dec_to_zero(v: &mut u32) {
    if *v != 0 {
        *v = v.wrapping_sub(1);
    }
}

fn set_bit(b: &mut u8, n: u8, on: bool) {
    *b = if on { *b | bit(n) } else { *b & !bit(n) };
}

fn add_f32(v: &mut f32, x: f64) {
    *v = mem(fp::fadds(reg(*v), x));
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
    /// The hits' window's (`StatsExt::hits_flag`, `xDC8`).
    Hits,
    /// The presses' window's (`StatsExt::presses_flag`, `xDCC`).
    Presses,
}

/// A frame window's last value, a record of the stats tail (`StatsExt::window_*`).
type Last<T> = fn(&mut StatsExt) -> &mut T;

impl Game {
    /// Records that the player's fighter went airborne or landed with a jump left
    /// (`xDD0` bit 0), `pl_8003FC44`; Nana's don't count.
    #[doc(alias = "pl_8003FC44")]
    pub fn record_jump_left(&mut self, slot: i32, sub: i32) {
        let p = self.player_mut(slot);
        if sub != 0 {
            return;
        }
        set_bit(&mut p.ext.frame_flags, 0, true);
    }
}

impl Running<'_> {
    /// A broken shield, bonus 0x40 (Nana's don't count), `pl_8003E0E8`.
    #[doc(alias = "pl_8003E0E8")]
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
    #[doc(alias = "pl_8003E114")]
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
            t.strong_hits_taken = t.strong_hits_taken.wrapping_add(1);
        }
    }

    /// A ledge another fighter holds, `pl_8003FFDC`: unless it's the player's own, bonus 0x46
    /// for the holder when the other last caught a different ledge (`ft_80087A98`).
    #[doc(alias = "ft_80087A98")]
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
        if slot != other && ledge != self.game.fighter(g).ledge_caught {
            self.count_bonus(slot, 0x46);
        }
    }

    /// A powershield counts in the player's stats (`xCD8`), `pl_8003E150`, Nana's too.
    #[doc(alias = "pl_8003E150")]
    pub fn record_powershield(&mut self, slot: i32) {
        let t = self.game.player_stale_moves(slot);
        t.powershields = t.powershields.wrapping_add(1);
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
        let t = &self.game.player_stale_moves(slot).bombhei_ring;
        if t.entries
            .iter()
            .any(|e| e.used && u32::from(spawn) == e.spawn)
        {
            return;
        }
        let at = t.next as usize;
        let (hit, used) = (t.entries[at].hit, t.entries[at].used);
        self.count_missed_bombhei(slot, hit, used);
        let t = &mut self.game.player_stale_moves(slot).bombhei_ring;
        let e = &mut t.entries[at];
        e.spawn = u32::from(spawn);
        e.hit = false;
        e.used = true;
        t.next = if t.next == 4 {
            0
        } else {
            t.next.wrapping_add(1)
        };
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
            if (0..ItKind::COMMON_END.id()).contains(&k) {
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
            let t = &mut self.game.player_stale_moves(slot).container_runs[i];
            if s {
                *t = t.wrapping_add(1);
                if *t as u32 == run {
                    self.set_point(slot, 0x9D, 1);
                    self.game.player_stale_moves(slot).container_runs[i] = 0;
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
            Flag::Bonus(kind) => p.stale_moves.bonuses[kind as usize],
            Flag::Hits => p.ext.hits_flag,
            Flag::Presses => p.ext.presses_flag,
        }
    }

    /// Sets a point bonus's flag (`setPointValue`; the original asserts the slot is a player's
    /// and the bonus counts points, which every caller's is), or a record's word.
    pub(super) fn set_flag(&mut self, slot: i32, flag: Flag, v: u32) {
        let p = self.game.player_mut(slot);
        match flag {
            Flag::Bonus(kind) => p.stale_moves.bonuses[kind as usize] = v,
            Flag::Hits => p.ext.hits_flag = v,
            Flag::Presses => p.ext.presses_flag = v,
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
        last: Last<u32>,
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
                *last(&mut self.game.player_mut(slot).ext) = value;
                return true;
            }
        } else if n.checked_rem(period).unwrap_or(n) == 0 {
            let cur = self.flag(slot, flag);
            let before = *last(&mut self.game.player_mut(slot).ext);
            if cur as i32 != 0 && value.wrapping_sub(before) < threshold {
                self.set_flag(slot, flag, 0);
            }
            *last(&mut self.game.player_mut(slot).ext) = value;
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
        last: Last<f32>,
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
                *last(&mut self.game.player_mut(slot).ext) = mem(value);
                return true;
            }
        } else if n.checked_rem(period).unwrap_or(n) == 0 {
            let cur = self.flag(slot, flag) as i32;
            let before = reg(*last(&mut self.game.player_mut(slot).ext));
            if cur != 0 && fp::fsubs(value, before) < threshold {
                self.set_flag(slot, flag, 0);
            }
            *last(&mut self.game.player_mut(slot).ext) = mem(value);
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
                reg(t.average_percent),
                fp::frsp(f64::from(n.wrapping_sub(1))),
                fp::frsp(f64::from(percent)),
            );
            t.average_percent = average(sum, fp::frsp(f64::from(n)));
        }
    }

    /// The frame's counters, `fn_8003EE2C`: frames since the last hit and the longest such
    /// run, frames in the air and on the ground, on steep slopes, nudged, off screen (and at
    /// which edge), holding a crate, the Screw Attack, Mr. Saturn or another logged item,
    /// and resized; the timers count down.
    #[doc(alias = "ftLib_GetItem")]
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
        let (streak_bit, attack_kind) = (fp.attack_flags[2] & 0x40 != 0, fp.attack_flags[3]);
        let slope = (!airborne).then(|| reg(game.ft_ground_slope_angle(g)));
        let nudged = game.ft_is_being_nudged(g);
        let scball = game.ft_holds_item_kind(g, ItKind::ScrewAttack.id());
        let dosei = game.ft_holds_item_kind(g, ItKind::MrSaturn.id());
        let item = fp.item_gobj.map(|it| {
            let kind = super::super::it::item(game, it).kind;
            match_item_kind(kind) != -1
                && super::super::it::lib26b1::use_kind(game, self.data, it) != 5
        });
        let changed = game.ft_is_altered(g);
        let p = self.game.player_mut(slot);
        inc(&mut p.ext.since_dealing);
        if p.ext.since_dealing > p.ext.longest_without_dealing {
            p.ext.longest_without_dealing = p.ext.since_dealing;
        }
        inc(&mut p.ext.since_taking);
        if p.ext.since_taking > p.ext.longest_without_taking {
            p.ext.longest_without_taking = p.ext.since_taking;
        }
        inc(if airborne {
            &mut p.ext.air_frames
        } else {
            &mut p.ext.ground_frames
        });
        if streak_bit {
            inc(&mut p.ext.streak);
            if fp::frsp(f64::from(p.ext.streak)) >= reg(streak) {
                inc(&mut p.ext.long_streak_frames);
            }
        } else {
            p.ext.streak = 0;
        }
        if attack_kind == 0x68 {
            inc(&mut p.ext.attack_0x68_frames);
        }
        if let Some(s) = slope
            && abs(s) >= steep
        {
            inc(&mut p.ext.steep_frames);
        }
        if nudged {
            inc(&mut p.ext.nudged_frames);
        }
        if edge != 0 {
            inc(&mut p.ext.edge_frames);
        }
        set_bit(&mut p.ext.frame_flags, 7, edge != 0);
        match edge {
            1 => set_bit(&mut p.ext.frame_flags, 5, true),
            2 => set_bit(&mut p.ext.frame_flags, 4, true),
            3 => set_bit(&mut p.ext.frame_flags, 6, true),
            4 => set_bit(&mut p.ext.frame_flags, 3, true),
            _ => {}
        }
        if heavy {
            inc(&mut p.ext.heavy_frames);
        }
        if scball {
            inc(&mut p.ext.screw_attack_frames);
        }
        if dosei {
            inc(&mut p.ext.saturn_frames);
        }
        match item {
            Some(logged) => {
                if logged {
                    inc(&mut p.ext.hold_frames);
                    if p.ext.hold_frames > p.ext.longest_hold {
                        p.ext.longest_hold = p.ext.hold_frames;
                    }
                }
            }
            None => {
                let held = p.ext.hold_frames;
                if held != 0 {
                    let shortest = p.ext.shortest_hold;
                    if held < shortest || shortest == 0 {
                        p.ext.shortest_hold = held;
                    }
                    p.ext.hold_frames = 0;
                }
            }
        }
        dec_to_zero(&mut p.ext.their_taunt_window);
        dec_to_zero(&mut p.ext.own_taunt_window);
        dec_to_zero(&mut p.ext.ko_window);
        inc(&mut p.ext.since_press);
        dec_to_zero(&mut p.ext.respawn_window);
        if changed {
            inc(&mut p.ext.altered_frames);
        }
    }

    /// The bonuses decided over frame windows, `fn_8003F294`: attacks of three kinds, hits
    /// landed and taken, attacks made, `xD70`, crates held, `xCD8`, the smash DI's distance,
    /// and A/B presses (whose window also settles the quickest press, `xDD1` bit 5).
    #[doc(alias = "pl_800386E8")]
    #[doc(alias = "fn_8003F294")]
    fn frame_windows(&mut self, slot: i32, index: i32) {
        if index == 1 {
            return;
        }
        let s = &self.game.player_stale_moves(slot).actions;
        // `pl_800386D8`: `x358`'s `by_attack_hi`, at `0x3E8 + 4 * attack`, which runs past it
        // into the words that follow.
        let (hi_107, hi_112, hi_111) = (s.wall_bounces, s.more_actions[4], s.more_actions[3]);
        // `pl_800386E8`: the words at 0x190, 0x5AC and 0x5B0.
        let e8 = s.attacks.by_attack_counts[99]
            .wrapping_add(s.more_actions[5])
            .wrapping_add(s.more_actions[6]);
        let (hits, attacks) = (s.hits.total, s.attacks.total);
        let p = self.game.player_mut(slot);
        let (crowd_reactions, heavy_frames, powershields) = (
            p.ext.crowd_reactions,
            p.ext.heavy_frames,
            p.stale_moves.powershields,
        );
        let (sdi_x, sdi_y) = (
            reg(p.stale_moves.sdi_distance_x),
            reg(p.stale_moves.sdi_distance_y),
        );
        let presses = p.ext.presses;
        let windows: [(Flag, u32, u32, Last<u32>); 9] = [
            (Flag::Bonus(0x3B), 0x70, hi_107, |e| {
                &mut e.window_wall_bounces
            }),
            (Flag::Bonus(0x45), 0x90, hi_112, |e| {
                &mut e.window_action_112
            }),
            (Flag::Bonus(0x49), 0xA8, e8, |e| &mut e.window_action_99),
            (Flag::Bonus(0x54), 0xC4, hi_111, |e| {
                &mut e.window_action_111
            }),
            (Flag::Hits, 0x18, hits, |e| &mut e.window_hits),
            (Flag::Bonus(0x13), 0x2C, attacks, |e| &mut e.window_attacks),
            (Flag::Bonus(0x5C), 0xE0, crowd_reactions, |e| {
                &mut e.window_crowd
            }),
            (Flag::Bonus(0x99), 0x130, heavy_frames, |e| {
                &mut e.window_heavy
            }),
            (Flag::Bonus(0x3D), 0x7C, powershields as u32, |e| {
                &mut e.window_powershields
            }),
        ];
        for (flag, param, value, last) in windows {
            let (period, threshold) = (self.bonus_param(param), self.bonus_param(param + 4));
            self.bonus_window(slot, flag, period, threshold, value, last);
        }
        let distance =
            crate::math::sqrt_reg(fp::fadds(fp::fmuls(sdi_x, sdi_x), fp::fmuls(sdi_y, sdi_y)));
        let period = self.bonus_param(0x88);
        let threshold = reg(self.bonus_param_f32(0x8C));
        self.bonus_window_f32(
            slot,
            Flag::Bonus(0x44),
            period,
            |e| &mut e.window_sdi,
            threshold,
            distance,
        );
        let (period, threshold) = (self.bonus_param(0xB8), self.bonus_param(0xBC));
        if self.bonus_window(slot, Flag::Presses, period, threshold, presses, |e| {
            &mut e.window_presses
        }) {
            let n = self.game.vs_frame_count();
            let p = self.game.player_mut(slot);
            let quickest = p.ext.quickest_press;
            if period == n {
                if quickest <= period {
                    set_bit(&mut p.ext.ko_flags, 5, true);
                }
            } else if p.ext.ko_flags & bit(5) != 0 && quickest > period {
                set_bit(&mut p.ext.ko_flags, 5, false);
            }
            p.ext.quickest_press = u32::MAX;
        }
    }

    /// The frame's inputs, `fn_8003F53C`: whether A and B were ever both let go (`xDD0`
    /// bit 1), whether L or R was ever pressed (bit 2), and A/B presses with the quickest
    /// frames between them.
    #[doc(alias = "ftLib_IsInputDisabled")]
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
            set_bit(&mut p.ext.frame_flags, 1, true);
        }
        if lr {
            set_bit(&mut p.ext.frame_flags, 2, true);
        }
        if pressed {
            inc(&mut p.ext.presses);
            let since = p.ext.since_press;
            if since < p.ext.quickest_press {
                p.ext.quickest_press = since;
            }
            p.ext.since_press = 0;
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
        match self.game.ft_strongest_motion(g) {
            1 => {
                let dx = fp::fsubs(px, x);
                let dy = fp::fsubs(py, y);
                let dist = crate::math::sqrt_reg(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)));
                let attacker = self.game.ft_last_attacker(g);
                let p = self.game.player_mut(slot);
                add_f32(&mut p.ext.knocked, dist);
                if dist > reg(p.ext.farthest_knocked) {
                    p.ext.farthest_knocked = mem(dist);
                }
                if attacker != 6 {
                    let o = self.game.player_mut(attacker);
                    if dist > reg(o.ext.farthest_knocked_away) {
                        o.ext.farthest_knocked_away = mem(dist);
                    }
                }
            }
            0 => {
                let grounded =
                    self.game.fighter(g).ground_or_air == super::super::ft::common::GA_GROUND;
                let p = self.game.player_mut(slot);
                if grounded {
                    add_f32(&mut p.ext.walked, abs(fp::fsubs(x, px)));
                } else {
                    let dy = fp::fsubs(y, py);
                    let v = if dy > 0.0 {
                        &mut p.ext.rose
                    } else {
                        &mut p.ext.fell
                    };
                    add_f32(v, abs(dy));
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
            let n = p.ext.opponent_distance_frames.wrapping_add(1);
            p.ext.opponent_distance_frames = n;
            let avg = reg(average(sum, fp::frsp(f64::from(count))));
            let total = fp::fmadds(
                reg(p.ext.opponent_distance),
                fp::frsp(f64::from((n as i32).wrapping_sub(1))),
                avg,
            );
            p.ext.opponent_distance = average(total, fp::frsp(f64::from(n as i32)));
        }
        let n = p.ext.center_distance_frames.wrapping_add(1);
        p.ext.center_distance_frames = n;
        let cam = super::super::gr::stage::cam_center(self.game);
        let off = abs(fp::fsubs(x, reg(cam.x)));
        let p = self.game.player_mut(slot);
        let total = fp::fmadds(
            reg(p.ext.center_distance),
            fp::frsp(f64::from((n as i32).wrapping_sub(1))),
            off,
        );
        p.ext.center_distance = average(total, fp::frsp(f64::from(n as i32)));
    }
}
