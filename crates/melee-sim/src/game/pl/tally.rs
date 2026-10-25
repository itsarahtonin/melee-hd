//! The bonuses a player earned, decided once the match ends (`plbonus.c`'s `pl_80039450` and
//! the seven checks it runs): flags in the player's bonus counts (`x904`) from the match's
//! statistics (the attack and hit counts, the records `pl_8003FAA8` kept, the items' log)
//! and from how the fighter stands at the end.

use gekko_fp as fp;

use super::super::gm::decisions::{DEC_FLAG, decision_type};
use super::super::state::{Game, Running};
use super::super::structs::plActionStats;
use crate::fpu::reg;

/// `pl_803BCE70`: the animation each normal attack needs for its own count (-1 none), by
/// attack from 1.
const ATTACK_ANIMS: [i32; 16] = [
    -1, 0x2F, 0x30, 0x31, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
];

/// `StatsAttack_Catch`, `StatsAttack_CatchAttack`, `StatsAttack_LedgeAttackSlow`,
/// `StatsAttack_ThrowF`, `StatsAttack_CargoThrowF`.
const STATS_CATCH: usize = 0x33;
const STATS_CATCH_ATTACK: usize = 0x34;
const STATS_LEDGE_ATTACK_SLOW: usize = 0x3E;
const STATS_THROW_F: usize = 0x35;
const STATS_CARGO_THROW_F: usize = 0x39;

/// `pl_CalculateAverage` on two values in registers.
fn average(val: f64, total: f64) -> f64 {
    assert!(
        total != 0.0,
        "the original's assertion failed: total != 0.0F"
    );
    fp::fdivs(val, total)
}

/// An unsigned count as a single, as `(f32)` converts it.
fn single(v: u32) -> f64 {
    fp::frsp(f64::from(v))
}

/// The bits of a byte of the records (`xDD0`, `xDD1`), `bit0` the top one.
fn bit(b: u8, n: u8) -> bool {
    b & (0x80 >> n) != 0
}

/// A word of the `x358_hits` statistics, which the mirror keeps as bytes.
fn x358_word(s: &plActionStats, off: usize) -> u32 {
    u32::from_be_bytes(s.x358[off..off + 4].try_into().unwrap())
}

/// `x358_hits.by_attack_counts[i]`.
fn x358_count(s: &plActionStats, i: usize) -> u32 {
    x358_word(s, 4 + 4 * i)
}

/// A count of attacks by id from 100 on, as `pl_800386D8` reads it (see
/// [`super::super::ft::fox::appeal::by_attack_hi`]).
fn by_attack_hi(s: &plActionStats, id: i32) -> u32 {
    super::super::ft::fox::appeal::by_attack_hi(s, id)
}

impl Game {
    /// Sets a flag bonus, `setFlag`: asserts the player and that the bonus is a flag.
    fn set_bonus_flag(&mut self, player: i32, kind: i32) {
        assert!(
            player != 6,
            "the original's assertion failed: player != Gm_Player_Other"
        );
        assert!(
            decision_type(kind) == DEC_FLAG,
            "the original's assertion failed: gmDecisionGetType(kind) == Gm_DecType_Flag"
        );
        self.player_mut(player).stale_moves.x904[kind as usize] = 1;
    }

    /// The flag bonuses cleared before the tally, `resetBonuses`.
    fn reset_bonus_flags(&mut self, player: i32) {
        for i in 0..215 {
            if decision_type(i) == DEC_FLAG {
                self.player_mut(player).stale_moves.x904[i as usize] = 0;
            }
        }
    }

    /// Whether a player hit victim `h_player` with normal attack `idx`, `pl_80037B2C`.
    #[doc(alias = "pl_80037B2C")]
    fn hit_victim_with(&self, player: i32, h_player: i32, idx: i32) -> bool {
        assert!(
            (0..8).contains(&h_player),
            "the original's assertion failed: 0 <= h_player && h_player < 8"
        );
        assert!(idx < 0x64, "pl_80037B2C past its table");
        let s = &self.player(player).stale_moves.total_attack_count_struct;
        s.x504[idx as usize] & (1u8 << (h_player as u8)) != 0
    }

    /// The sum of attack counts `from` to `to` in statistics laid out as `plAttackStats`,
    /// `fn_80038700` (`by_attack_counts[i]`, the words from `x4`).
    #[doc(alias = "fn_80038700")]
    fn attack_count_sum(counts: &dyn Fn(usize) -> u32, from: usize, to: usize) -> u32 {
        (from..=to).fold(0u32, |s, i| s.wrapping_add(counts(i)))
    }

    /// The player's average ... (`x5EC`'s `x8` over `xC`), `pl_800414C0`.
    #[doc(alias = "pl_800414C0")]
    fn x5ec_average(&self, player: i32) -> f64 {
        let t = &self.player(player).stale_moves.x5EC;
        if t.xC != 0 {
            return fp::fdivs(single(t.x8), single(t.xC));
        }
        0.0
    }

    /// The item log's count of kind `kind` for a player, `pl_8003E2CC`.
    #[doc(alias = "pl_8003E2CC")]
    pub fn items_logged(&self, player: i32, kind: usize) -> u32 {
        assert!(
            kind < 39,
            "the original's assertion failed: pl_itemlog_kind < Pl_ItemLog_Terminate"
        );
        self.player(player).stale_moves.x674[kind]
    }

    /// The item log's second count of kind `kind`, `pl_8003E334`.
    #[doc(alias = "pl_8003E334")]
    pub fn items_logged2(&self, player: i32, kind: usize) -> u32 {
        assert!(
            kind < 39,
            "the original's assertion failed: pl_itemlog_kind < Pl_ItemLog_Terminate"
        );
        self.player(player).stale_moves.x710[kind] as u32
    }

    /// The item log's total, `pl_8003E39C`.
    #[doc(alias = "pl_8003E39C")]
    pub fn items_logged_total(&self, player: i32) -> i32 {
        (0..39).fold(0i32, |s, k| {
            s.wrapping_add(self.items_logged(player, k) as i32)
        })
    }

    /// The item log's second total, `pl_8003E420`.
    #[doc(alias = "pl_8003E420")]
    pub fn items_logged2_total(&self, player: i32) -> u32 {
        (0..39).fold(0u32, |s, k| s.wrapping_add(self.items_logged2(player, k)))
    }

    /// How often a Pokémon came out of the player's Poké Balls, `pl_8003E7D4`. The
    /// original indexes the attack statistics' `x598` by the Pokémon's kind, which from
    /// the first Pokémon's (`It_PKind_Start`) lands in the stale-move table's `x848`.
    #[doc(alias = "pl_8003E7D4")]
    pub fn pokemon_logged(&self, player: i32, kind: i32) -> u32 {
        assert!(
            (0xA1..0xBF).contains(&kind),
            "the original's assertion failed: It_PKind_Start <= kind && kind < It_PKind_Terminate"
        );
        self.player(player).stale_moves.x848[(kind - 0xA1) as usize] as u32
    }
}

impl Running<'_> {
    /// A float of the player library's common data (`pl_804D6470`).
    fn plco_f32(&self, off: u32) -> f64 {
        reg(self.data.f32(self.pl_data().at(off)))
    }

    fn plco_u32(&self, off: u32) -> u32 {
        self.data.u32(self.pl_data().at(off))
    }

    /// Whether the player's character has normal attack `kind` (its animation), `pl_80038628`.
    #[doc(alias = "pl_80038628")]
    fn has_normal_attack(&self, gobj: crate::hsd::gobj::GObjId, kind: i32) -> bool {
        assert!(
            (1..=16).contains(&kind),
            "the original's assertion failed: PlATK_AttackNormal_Start <= kind && kind <= PlATK_AttackNormal_End"
        );
        let anim = ATTACK_ANIMS[(kind - 1) as usize];
        if anim == -1 {
            return true;
        }
        let f = self.game.fighter_id(gobj);
        self.data.u32(self.anim_entry(f, anim).at(8)) != 0
    }

    /// The bonuses a player earned, decided as the match ends, `pl_80039450`: the flag
    /// bonuses cleared, then the seven checks, and three of the records' own.
    #[doc(alias = "pl_80039450")]
    pub fn tally_bonuses(&mut self, player: i32) {
        self.game.reset_bonus_flags(player);
        self.tally_attacks(player);
        self.tally_movement(player);
        self.tally_grabs(player);
        self.tally_time(player);
        self.tally_items(player);
        self.tally_ending(player);
        self.tally_variety(player);
        let p = self.game.player(player);
        let (dd0, dd1, dcc) = (p.ext_u8(0xDD0), p.ext_u8(0xDD1), p.ext_u32(0xDCC));
        if !bit(dd0, 1) {
            self.game.set_bonus_flag(player, 0x4F);
        }
        if !self.game.vs_single_button() && !bit(dd0, 2) {
            self.game.set_bonus_flag(player, 0x55);
        }
        if dcc != 0 && bit(dd1, 5) {
            self.game.set_bonus_flag(player, 0x4E);
        }
    }

    /// The attacks' and hits' bonuses, `fn_80039618`.
    #[doc(alias = "fn_80039618")]
    fn tally_attacks(&mut self, player: i32) {
        let g = &*self.game;
        let gobj = g.player_entity(player).expect("the player's fighter");
        let p = g.player(player);
        let t = &p.stale_moves;
        let stats = &t.total_attack_count_struct;
        let aerials = stats.hits.aerials_count;
        let atk_thrown = stats.attacks.thrown_item_count;
        let atk_x1a8 = stats.attacks.x1A8;
        let hit_counts = stats.hits.by_attack_counts;
        let atk_counts = stats.attacks.by_attack_counts;
        let hit_thrown = stats.hits.thrown_item_count;
        let hit_specials = stats.hits.specials_count;
        let hit_x1a0 = stats.hits.x1A0_count;
        let hit_x1a8 = stats.hits.x1A8;
        let x358_x1a8 = x358_word(stats, 0x1A8);
        let attacks_total = stats.attacks.total;
        let hits_total = stats.hits.total;
        let x358_total = x358_word(stats, 0);
        let x358_counts: [u32; 100] = core::array::from_fn(|i| x358_count(stats, i));
        let (x56c, x574) = (stats.x56C, stats.x574);
        let (xc60, xc64, xc74, xc94) = (t.xC60, t.xC64, t.xC74, t.xC94);
        let (xd04, xd0c, xdc8, dd1) = (
            p.ext_u32(0xD04),
            p.ext_u32(0xD0C),
            p.ext_u32(0xDC8),
            p.ext_u8(0xDD1),
        );
        let mut flags = Vec::new();
        if aerials != 0 && aerials == hits_total {
            flags.push(0);
        }
        if hit_specials != 0 && hit_specials == hits_total {
            flags.push(0x21);
        }
        if hit_x1a0 != 0 && hit_x1a0 == hits_total {
            flags.push(0x22);
        }
        if hits_total != 0 && (1..100).all(|i| (0x33..=0x3D).contains(&i) || hit_counts[i] == 0) {
            flags.push(0x24);
        }
        let mut sum = 0u32;
        let mut sub_sum = 0u32;
        for (i, &count) in hit_counts.iter().enumerate().take(0x63).skip(0x40) {
            sum = sum.wrapping_add(count);
            if matches!(i, 0x42 | 0x46 | 0x4A | 0x4E | 0x56) {
                sub_sum = sub_sum.wrapping_add(count);
            }
        }
        if sum != 0 {
            if hits_total == hit_counts[0x62] {
                flags.push(0x95);
            } else if hits_total == sub_sum {
                flags.push(0x96);
            } else if hits_total == sum {
                flags.push(0x94);
            }
        }
        for f in flags.drain(..) {
            self.game.set_bonus_flag(player, f);
        }
        // Every normal attack on one victim.
        for j in 0..6 {
            let all = (1..=16).all(|k| {
                self.game.hit_victim_with(player, j, k) || !self.has_normal_attack(gobj, k)
            });
            if all {
                self.game.set_bonus_flag(player, 0x11);
                break;
            }
        }
        if self.game.player(player).stale_moves.x904[0x11] == 0 {
            let low =
                (1..=11).all(|k| hit_counts[k] != 0 || !self.has_normal_attack(gobj, k as i32));
            let high =
                (12..=16).all(|k| hit_counts[k] != 0 || !self.has_normal_attack(gobj, k as i32));
            if low && high {
                self.game.set_bonus_flag(player, 0x10);
            } else if low {
                self.game.set_bonus_flag(player, 0xE);
            } else if high {
                self.game.set_bonus_flag(player, 0xF);
            }
        }
        // Even counts of every attack hit.
        {
            let (mut count, mut min, mut max) = (0i32, 0i32, 0i32);
            let mut skipped = false;
            for &c in &x358_counts[1..100] {
                if c != 0 {
                    count = count.wrapping_add(1);
                    if c > max as u32 {
                        max = c as i32;
                    }
                    if min == 0 || c < min as u32 {
                        min = c as i32;
                    }
                }
                if max != 0
                    && min != 0
                    && fp::frsp(f64::from(max.wrapping_sub(min)))
                        >= fp::fmuls(fp::frsp(f64::from(max)), self.plco_f32(0x28))
                {
                    skipped = true;
                    break;
                }
            }
            if !skipped && count >= self.plco_u32(0x24) as i32 {
                self.game.set_bonus_flag(player, 0x12);
            }
        }
        if hits_total != 0 {
            if hit_thrown == hits_total {
                self.game.set_bonus_flag(player, 0x15);
            } else if average(single(hit_thrown), single(hits_total)) >= self.plco_f32(0x34) {
                self.game.set_bonus_flag(player, 0x14);
            }
        }
        if atk_thrown == 0 {
            self.game.set_bonus_flag(player, 0x16);
        }
        {
            let mut n = 0;
            let mut skipped = false;
            for (i, &c) in hit_counts.iter().enumerate().skip(1) {
                if c != 0 {
                    if !(0x11..=0x30).contains(&i) {
                        skipped = true;
                        break;
                    }
                    n += 1;
                }
            }
            if !skipped && n != 0 {
                self.game
                    .set_bonus_flag(player, if n == 1 { 0x18 } else { 0x17 });
            }
        }
        if hits_total != 0 && average(single(x56c), single(hits_total)) >= self.plco_f32(0x8) {
            self.game.set_bonus_flag(player, 3);
        }
        if hits_total != 0 && x574 == hits_total {
            self.game.set_bonus_flag(player, 5);
        } else if hits_total >= self.plco_u32(0xC)
            && average(single(x574), single(hits_total)) >= self.plco_f32(0x10)
        {
            self.game.set_bonus_flag(player, 4);
        }
        if attacks_total == 0 && xc60 == 0.0 {
            self.game.set_bonus_flag(player, 0x64);
        } else {
            if attacks_total == 0 {
                self.game.set_bonus_flag(player, 0x60);
            } else if xd04 >= self.plco_u32(0x54) {
                self.game.set_bonus_flag(player, 0x2F);
            } else if xd04 >= self.plco_u32(0x50) {
                self.game.set_bonus_flag(player, 0x2E);
            }
            if xc60 == 0.0 {
                self.game.set_bonus_flag(player, 0x62);
            } else if xd0c >= self.plco_u32(0xF0) {
                self.game.set_bonus_flag(player, 0x61);
            }
        }
        let falls = (self.game.player_falls(player) as u32)
            .wrapping_add(self.game.one_player_falls() as u32);
        if falls == 0 {
            self.game.set_bonus_flag(player, 0x63);
        } else if falls == xc94 as u32 {
            self.game.set_bonus_flag(player, 0x66);
        }
        if bit(dd1, 0) && bit(dd1, 1) && bit(dd1, 2) && bit(dd1, 3) {
            self.game.set_bonus_flag(player, 0x86);
        }
        if atk_x1a8 != 0
            && atk_x1a8 == x358_x1a8
            && hit_x1a8 == self.game.player(player).stale_moves.x904[6]
        {
            self.game.set_bonus_flag(player, 8);
        }
        if !self.game.bonus_teams() {
            let dmg = f64::from(fp::fctiwz(reg(xc64)));
            let dmg = fp::frsp(dmg);
            let f = if dmg >= 400.0 {
                Some(0x20)
            } else if dmg >= 350.0 {
                Some(0x1F)
            } else if dmg >= 300.0 {
                (!super::super::gm::is_super_sudden_death(self.game)).then_some(0x1E)
            } else if dmg >= 250.0 {
                Some(0x1D)
            } else if dmg >= 200.0 {
                Some(0x1C)
            } else if dmg >= 150.0 {
                Some(0x1B)
            } else {
                None
            };
            if let Some(f) = f {
                self.game.set_bonus_flag(player, f);
            }
        }
        if xdc8 != 0 {
            if x358_total == attacks_total {
                self.game.set_bonus_flag(player, 0xD);
            } else if average(single(x358_total), single(attacks_total)) >= self.plco_f32(0x20) {
                self.game.set_bonus_flag(player, 0xC);
            }
        }
        {
            let sum_attacks = Game::attack_count_sum(&|i| atk_counts[i], 1, 0x10);
            let sum_x358 = Game::attack_count_sum(&|i| x358_counts[i], 1, 0x10);
            if sum_x358 != 0 && sum_attacks == sum_x358 && !bit(dd1, 4) {
                self.game.set_bonus_flag(player, 0x23);
            }
        }
        if reg(xc74) >= self.plco_f32(0x160) {
            self.game.set_bonus_flag(player, 0xAA);
        }
        if self.game.x5ec_average(player) >= self.plco_f32(0x0) {
            self.game.set_bonus_flag(player, 1);
        }
        if self.has_normal_attack(gobj, 2) {
            let atk1 = atk_counts[1];
            if atk1 != 0 && atk1 == atk_counts[2] {
                let hit1 = x358_counts[1];
                if hit1 == x358_counts[2] && atk1 == hit1 {
                    if self.has_normal_attack(gobj, 3) {
                        if atk1 == atk_counts[3] && atk1 == x358_counts[3] {
                            self.game.set_bonus_flag(player, 0x19);
                        }
                    } else {
                        self.game.set_bonus_flag(player, 0x19);
                    }
                }
            }
        }
        {
            let mut max = 0.0f64;
            let mut sum = 0.0f64;
            for i in 0..6 {
                if i != player {
                    let v = reg(self.game.player(i).stale_moves.xC78[player as usize]);
                    if max < v {
                        max = v;
                    }
                    sum = fp::fadds(sum, v);
                }
            }
            if sum != 0.0 {
                let n = self.game.player_count();
                if (n == 3 && average(max, sum) >= self.plco_f32(0xF8))
                    || (n >= 4 && average(max, sum) >= self.plco_f32(0xFC))
                {
                    self.game.set_bonus_flag(player, 0x67);
                }
            }
        }
        if self.game.player_count() >= 3 {
            let mut streak = false;
            for i in 0..6 {
                if i == player {
                    continue;
                }
                let kos = self.game.player_kos(player, i) as i32;
                if kos != 0 {
                    if fp::frsp(f64::from(kos)) >= self.plco_f32(0x100) && !streak {
                        streak = true;
                    } else {
                        streak = false;
                        break;
                    }
                }
            }
            if streak {
                self.game.set_bonus_flag(player, 0x68);
            }
        }
    }

    /// The movement bonuses: the jumps, the dodges, the ledge, the air and the distance,
    /// `fn_8003B044`.
    #[doc(alias = "fn_8003B044")]
    fn tally_movement(&mut self, player: i32) {
        let g = &*self.game;
        let p = g.player(player);
        let t = &p.stale_moves;
        let s = &t.total_attack_count_struct;
        let (b0, b2, b3) = (s.x5BC_b0, s.x5BC_b2, s.x5BC_b3);
        let hi = |id| by_attack_hi(s, id);
        let (h65, h67, h68, h6c, h6d, h6e) =
            (hi(0x65), hi(0x67), hi(0x68), hi(0x6C), hi(0x6D), hi(0x6E));
        let (xce4, xce8, xc60, xc98, xca0) = (t.xCE4, t.xCE8, t.xC60, t.xC98, t.xCA0);
        let (xd14, xd2c, xd8c, xd94, dd0, dd1) = (
            p.ext_u32(0xD14),
            p.ext_u32(0xD2C),
            p.ext_f32(0xD8C),
            p.ext_f32(0xD94),
            p.ext_u8(0xDD0),
            p.ext_u8(0xDD1),
        );
        let mut flags = Vec::new();
        if !b0 {
            flags.push(0x31);
        }
        if h65 == 0 {
            flags.push(0x32);
        }
        if xd14 == 0 {
            flags.push(0x33);
        }
        if h68 == 0 && h67 == 0 {
            flags.push(0x35);
        }
        if !b2 {
            flags.push(if b3 { 0x38 } else { 0x3A });
        }
        if !bit(dd0, 0) && b3 {
            flags.push(0x39);
        }
        if h6c != 0 && h6d == 0 {
            flags.push(0x51);
        }
        if h6c == 0 && h6d != 0 {
            flags.push(0x52);
        }
        if h6c == 0 && h6d == 0 {
            flags.push(0x50);
            if h6e != 0 {
                flags.push(0x53);
            }
        }
        for f in flags.drain(..) {
            self.game.set_bonus_flag(player, f);
        }
        if xce4 != 0 && average(single(xce8), single(xce4)) >= self.plco_f32(0x9C) {
            self.game.set_bonus_flag(player, 0x47);
        }
        if reg(xc60) >= self.plco_f32(0xD0) && average(reg(xc98), reg(xc60)) >= self.plco_f32(0xCC)
        {
            self.game.set_bonus_flag(player, 0x57);
        }
        if !self.game.bonus_teams() && xd2c == 0 {
            self.game.set_bonus_flag(player, 0x6B);
        }
        if !super::super::gm::is_super_sudden_death(self.game) && self.game.player_count() >= 3 {
            let mut sum = 0.0f64;
            let mut n = 0i32;
            for i in 0..6 {
                if self.game.player_awake(i) {
                    sum = fp::fadds(sum, reg(self.game.player(i).stale_moves.xC9C));
                    n = n.wrapping_add(1);
                }
            }
            if sum != 0.0 && n != 0 {
                let avg = average(sum, fp::frsp(f64::from(n)));
                if reg(xca0) >= fp::fmuls(avg, self.plco_f32(0xF4)) {
                    self.game.set_bonus_flag(player, 0x65);
                }
            }
        }
        let width = reg(super::super::camera::average_width(&self.game.camera.cam));
        if self.game.vs_frame_count() >= self.plco_u32(0x108) {
            let half = fp::fmuls(0.5, width);
            if reg(xd8c) >= fp::fmuls(self.plco_f32(0x104), half) {
                self.game.set_bonus_flag(player, 0x69);
            }
        }
        if self.game.vs_frame_count() >= self.plco_u32(0x110) && reg(xd8c) < self.plco_f32(0x10C) {
            self.game.set_bonus_flag(player, 0x6A);
        }
        let half = fp::fmuls(0.5, width);
        if reg(xd94) <= fp::fmuls(self.plco_f32(0x114), half) {
            self.game.set_bonus_flag(player, 0x6C);
        }
        if bit(dd1, 6) && !bit(dd1, 7) {
            self.game.set_bonus_flag(player, 0x5A);
        }
    }

    /// The grabs' and throws' bonuses, `fn_8003B9A4`.
    #[doc(alias = "fn_8003B9A4")]
    fn tally_grabs(&mut self, player: i32) {
        let t = &self.game.player(player).stale_moves;
        let s = &t.total_attack_count_struct;
        let attack = s.attacks.by_attack_counts;
        let hit = s.hits.by_attack_counts;
        let (xc6c, xc70) = (t.xC6C, t.xC70);
        if attack[STATS_CATCH] != 0 {
            if attack[STATS_CATCH] >= self.plco_u32(0x3C) && hit[STATS_CATCH] == 0 {
                self.game.set_bonus_flag(player, 0x26);
            } else if average(single(hit[STATS_CATCH]), single(attack[STATS_CATCH]))
                <= self.plco_f32(0x44)
            {
                self.game.set_bonus_flag(player, 0x25);
            }
        }
        if hit[STATS_CATCH] != 0 {
            let only = (STATS_CATCH..STATS_LEDGE_ATTACK_SLOW)
                .all(|i| i == STATS_CATCH || i == STATS_CATCH_ATTACK || attack[i] == 0);
            if only {
                if hit[STATS_CATCH_ATTACK] == 0 {
                    if hit[STATS_CATCH] >= self.plco_u32(0x40) {
                        self.game.set_bonus_flag(player, 0x27);
                    }
                } else {
                    self.game.set_bonus_flag(player, 0x2C);
                }
            }
        }
        if (STATS_THROW_F..STATS_CARGO_THROW_F).all(|i| attack[i] != 0) {
            self.game.set_bonus_flag(player, 0x28);
        }
        if xc6c > 0.0 && average(reg(xc70), reg(xc6c)) >= self.plco_f32(0x48) {
            self.game.set_bonus_flag(player, 0x2B);
        }
    }

    /// The time-shared bonuses: the time spent in the air, on the ledge, shielding and
    /// the like over the match's frames, `fn_8003BD60`.
    #[doc(alias = "fn_8003BD60")]
    fn tally_time(&mut self, player: i32) {
        let p = self.game.player(player);
        let w = |off| p.ext_u32(off);
        let (xd14, xd1c, xd20, xd24, xd28, xd2c, xd30, xd38, xd3c) = (
            w(0xD14),
            w(0xD1C),
            w(0xD20),
            w(0xD24),
            w(0xD28),
            w(0xD2C),
            w(0xD30),
            w(0xD38),
            w(0xD3C),
        );
        let dd0 = p.ext_u8(0xDD0);
        let frames = self.game.vs_frame_count();
        if frames != 0 {
            let f = single(frames);
            if average(single(xd14), f) >= self.plco_f32(0x60) {
                self.game.set_bonus_flag(player, 0x34);
            }
            if average(single(xd1c), f) >= self.plco_f32(0x68) {
                self.game.set_bonus_flag(player, 0x36);
            }
            if average(single(xd20), f) >= self.plco_f32(0x6C) {
                self.game.set_bonus_flag(player, 0x37);
            }
            if xd24 != 0 && average(single(xd28), single(xd24)) >= self.plco_f32(0x5C) {
                self.game.set_bonus_flag(player, 0x30);
            }
            if !self.game.bonus_teams() && average(single(xd2c), f) >= self.plco_f32(0x78) {
                self.game.set_bonus_flag(player, 0x3C);
            }
            if average(single(xd30), f) >= self.plco_f32(0xE8) {
                self.game.set_bonus_flag(player, 0x5D);
            }
        }
        if bit(dd0, 3) && bit(dd0, 4) && bit(dd0, 5) && bit(dd0, 6) {
            self.game.set_bonus_flag(player, 0x5E);
        }
        if xd38 >= self.plco_u32(0x164) {
            self.game.set_bonus_flag(player, 0xAB);
        }
        if xd3c >= self.plco_u32(0x16C) {
            self.game.set_bonus_flag(player, 0xB4);
        }
    }

    /// The items' bonuses, from the item log, `fn_8003C340`.
    #[doc(alias = "fn_8003C340")]
    fn tally_items(&mut self, player: i32) {
        let g = &*self.game;
        let total = g.items_logged_total(player);
        if total == 0 {
            self.game.set_bonus_flag(player, 0x93);
            return;
        }
        let log = |k: usize| self.game.items_logged(player, k);
        let mut flags = Vec::new();
        if total as u32 == log(7) {
            flags.push(0xB3);
        }
        if g.items_logged2_total(player) == g.it.spawned as u32 {
            flags.push(0x9F);
        }
        let x6d08 = g.it.x6d08;
        if x6d08 != 0 && x6d08 as u32 == g.items_logged2(player, 18) {
            flags.push(0xA0);
        }
        let kinds = (0..39)
            .filter(|&i| i >= 6 && i != 18 && log(i) != 0)
            .count() as u32;
        if kinds >= self.plco_u32(0x144) {
            flags.push(0x9E);
        }
        // The log's kinds by group: thrown, swung, shot, eaten, ... (`var_r24` to `var_r30`).
        let mut sums = [0u32; 7];
        for i in 0..39 {
            let group = match i {
                11 | 12 | 13 | 22 | 23 | 24 | 28 => 0,
                16 | 21 | 25 => 1,
                8 | 9 | 35 | 37 => 2,
                18 => 3,
                26 | 27 | 31 | 32 | 33 => 4,
                0..=7 | 14 | 15 | 17 | 19 | 20 | 30 | 34 | 36 | 38 => 5,
                10 | 29 => 6,
                _ => continue,
            };
            sums[group] = sums[group].wrapping_add(log(i));
        }
        for (group, (off, flag)) in [
            (0x148, 0xA2),
            (0x14C, 0xA3),
            (0x150, 0xA4),
            (0x154, 0xA1),
            (0x158, 0xA5),
            (0x15C, 0xA6),
        ]
        .into_iter()
        .enumerate()
        {
            let only = (0..7).all(|k| k == group || sums[k] == 0);
            if only && sums[group] >= self.plco_u32(off) {
                flags.push(flag);
            }
        }
        if log(26).wrapping_add(log(27)) >= self.plco_u32(0x168) {
            flags.push(0xAE);
        }
        let only_three = (0..39).all(|i| match i {
            10 | 25 | 26 => log(i) != 0,
            _ => log(i) == 0,
        });
        if only_three {
            flags.push(0xAF);
        }
        if log(7) >= self.plco_u32(0x170) {
            flags.push(0xB5);
        }
        if log(9) >= self.plco_u32(0x174) {
            flags.push(0xBC);
        }
        if log(8) >= self.plco_u32(0x178) {
            flags.push(0xBD);
        }
        for f in flags {
            self.game.set_bonus_flag(player, f);
        }
        if self.game.pokemon_logged(player, 0xB7) != 0 {
            self.game.set_bonus_flag(player, 0xCC);
        }
        if self.game.pokemon_logged(player, 0xB8) != 0 {
            self.game.set_bonus_flag(player, 0xCD);
        }
    }

    /// The bonuses of how the fighter stands as the match ends (on the ledge, taunting,
    /// its damage), `fn_8003CC84`, once the windowed checks' last windows are counted.
    #[doc(alias = "fn_8003CC84")]
    fn tally_ending(&mut self, player: i32) {
        if self.game.vs_in_play() {
            return;
        }
        let gobj = self.game.player_entity(player);
        self.count_streaks(player);
        self.game.reset_x8c0(player);
        self.count_bombhei_misses(player);
        self.game.reset_x8d8(player);
        if !self.game.player_awake(player) {
            return;
        }
        let gobj = gobj.expect("the player's fighter");
        let mut flags = Vec::new();
        match self.game.ft_motion_id(gobj) {
            0xC | 0xD => flags.push(0x89),
            0..=2 if self.game.is_1p_run() || !self.game.bonus_stock() => flags.push(0x8C),
            4 if self.game.is_1p_run() || !self.game.bonus_stock() => flags.push(0x8D),
            _ => {}
        }
        if !self.game.vs_untimed() && !self.game.vs_timer_counts_up() {
            match self.game.vs_timer_seconds() {
                0 => flags.push(0x8F),
                7 => flags.push(0x8E),
                0xD5 => flags.push(0x90),
                _ => {}
            }
        }
        for f in flags.drain(..) {
            self.game.set_bonus_flag(player, f);
        }
        if self.game.ft_is_dead(gobj) {
            return;
        }
        if self.taunted_last(gobj) {
            self.game.set_bonus_flag(player, 0x8B);
        }
        if self.game.player(player).ext_u8(0xDD0) & 1 != 0 {
            self.game.set_bonus_flag(player, 0x8A);
        }
        if self.parasol_moving(gobj) {
            self.game.set_bonus_flag(player, 0xA7);
        }
        for i in 0..6 {
            if player as u32 == self.game.player(i).stale_moves.xCAC {
                self.game.set_bonus_flag(player, 0xA8);
            }
        }
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x1980.is_some() {
            self.game.set_bonus_flag(player, 0xA9);
        }
        if self.game.fighters[f.0].x221D_b6 {
            self.game.set_bonus_flag(player, 0xBE);
        }
        let dmg = self.game.player_damage(player);
        if dmg == 0 {
            self.game.set_bonus_flag(player, 0x92);
        }
        let h = dmg / 100;
        let t = (dmg - h * 100) / 10;
        let o = dmg - h * 100 - t * 10;
        if (h != 0 && h == t && t == o) || (h == 0 && t != 0 && t == o) {
            self.game.set_bonus_flag(player, 0x91);
        }
    }

    /// Whether the fighter's last attack was a taunt (motions 113, 114 and 99) or Fox's
    /// side taunt counted, `ft_800898C0`.
    #[doc(alias = "ft_800898C0")]
    fn taunted_last(&mut self, gobj: crate::hsd::gobj::GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let k = self.game.fighters[f.0].x2070[3];
        matches!(k, 113 | 114 | 99) || self.fox_appeal_s_used(f)
    }

    /// The bonus for one attack used throughout, `fn_8003D2EC`, which the bonuses of only
    /// one kind of hit rule out.
    #[doc(alias = "fn_8003D2EC")]
    fn tally_variety(&mut self, player: i32) {
        let t = &self.game.player(player).stale_moves;
        let counts = t.total_attack_count_struct.attacks.by_attack_counts;
        let total = t.total_attack_count_struct.attacks.total;
        if self.game.vs_single_button()
            || [0, 5, 0x15, 0x17, 0x18, 0x21, 0x22, 0x24, 0x96, 0x95, 0x94]
                .iter()
                .any(|&k| t.x904[k] != 0)
        {
            return;
        }
        if total >= self.plco_u32(0xDC) {
            for &c in &counts[1..100] {
                if average(single(c), single(total)) >= self.plco_f32(0xD8) {
                    self.game.set_bonus_flag(player, 0x59);
                    break;
                }
            }
        }
    }

    /// Counts the attack streaks' bonuses from the log, `pl_8004182C` (`fn_80041770` on each).
    #[doc(alias = "pl_8004182C")]
    #[doc(alias = "fn_80041770")]
    fn count_streaks(&mut self, player: i32) {
        let entries = self.game.player(player).stale_moves.x8C0.x4.clone();
        for e in &entries {
            let n = e.x2;
            if n >= 5 {
                self.count_bonus(player, 0x7D);
            } else if n >= 4 {
                self.count_bonus(player, 0x7C);
            } else if n >= 3 {
                self.count_bonus(player, 0x7B);
            } else if n >= 2 {
                self.count_bonus(player, 0x7A);
            }
            if e.x3_b0 && n != 0 {
                self.count_bonus(player, 0x78);
            }
        }
    }

    /// Counts the Bob-ombs thrown and missed from the log, `pl_80041BFC`.
    #[doc(alias = "pl_80041BFC")]
    fn count_bombhei_misses(&mut self, player: i32) {
        let entries = self.game.player(player).stale_moves.x8D8.x4.clone();
        for e in &entries {
            self.count_missed_bombhei(player, e.x4_b0, e.x4_b1);
        }
    }
}

impl Game {
    /// Whether the bonuses count teams, `gm_801720B4` (a single-player run's own rule
    /// otherwise, `fn_8017E160`).
    #[doc(alias = "gm_801720B4")]
    pub fn bonus_teams(&self) -> bool {
        if self.is_1p_run() {
            super::super::code::unported_call("gm_801720B4 in a single-player run (fn_8017E160)");
        }
        self.vs.start.is_teams == 1
    }

    /// Whether the bonuses take the match as a stock match, `gm_801720F8`.
    #[doc(alias = "gm_801720F8")]
    pub fn bonus_stock(&self) -> bool {
        if self.is_1p_run() {
            return true;
        }
        self.vs.start.match_kind == 1
    }

    /// The single-player modes' falls the bonuses add, `gm_80172140` (`fn_8017DF28`'s data,
    /// none outside them).
    #[doc(alias = "gm_80172140")]
    pub fn one_player_falls(&self) -> i32 {
        if super::super::gm::is_1p_mode(self) {
            super::super::code::unported_call("gm_80172140 in a single-player mode (fn_8017DF28)");
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::gm::decisions::DEC_POINT;
    use super::*;

    #[test]
    fn decision_types_split_flags_from_counts() {
        assert_eq!(decision_type(0), DEC_FLAG);
        assert_eq!(decision_type(2), DEC_POINT);
        assert_eq!(decision_type(0x7D), DEC_POINT);
    }
}
