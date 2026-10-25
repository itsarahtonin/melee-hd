//! The player library (`melee/pl`): the six player blocks (`player_slots`) that outlive a
//! match's fighters, holding each slot's character, stocks, damage, KOs and the statistics
//! the results screen and bonuses read, and the match statistics fighters report there.

pub mod bonus;
pub mod death;
pub mod player;
pub mod stats;
pub mod tally;
pub mod trick;

use melee_data::archive::DataRef;

use super::state::Running;
use super::structs::plActionStats;
use gekko_fp as fp;

/// The player library's globals.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// `PdPm.dat`'s `plLoadCommonData`, whose first pointer `Player_80036DD8` keeps in
    /// `pl_804D6470`.
    pub load_common: Option<DataRef>,
    /// The attack instance counter (`staleAttackInstance`): each new attack takes the next.
    pub stale_attack_instance: u16,
    /// The attack record counter (`unk_804D6480`), drawn when a state's attack flags change.
    pub attack_instance: u16,
}

impl Running<'_> {
    /// `pl_804D6470`.
    pub(crate) fn pl_data(&self) -> DataRef {
        let base = self.game.pl.load_common.expect("PdPm.dat isn't loaded");
        self.data.ptr(base).expect("plLoadCommonData's data")
    }

    /// `pl_800411C4`: the frame's counters end; repeated inputs count toward a bonus, and an
    /// entry for another player marked with a count of one gives that player bonus 0x4C.
    #[doc(alias = "pl_800411C4")]
    pub fn pl_800411c4(&mut self, slot: i32, sub: i32) {
        if sub == 1 {
            return;
        }
        self.pl_8004138c(slot, sub);
        for i in 0..6 {
            let e = &self.game.player_unk6a8(slot).x10[i];
            // Bonus 0x4C goes to the player the entry is for.
            if e.x12_b0 && e.x4 == 1 {
                self.count_bonus(i as i32, 0x4C);
            }
            let e = &mut self.game.player_unk6a8(slot).x10[i];
            e.x0 = 0.0;
            e.x4 = 0;
            e.x8 = 0;
            e.xC = 0;
            e.x12_b0 = false;
        }
    }

    /// `pl_80041280`: grounded, the frame's air-time counters clear (Nana's don't count).
    #[doc(alias = "pl_80041280")]
    pub fn clear_air_counters(&mut self, slot: i32, sub: i32) {
        if sub == 1 {
            return;
        }
        for e in &mut self.game.player_unk6a8(slot).x10 {
            e.x10 = 0;
        }
    }

    /// `fn_8004138C`: a bonus for one input dominating the frame's counters.
    #[doc(alias = "fn_8004138C")]
    fn pl_8004138c(&mut self, slot: i32, sub: i32) {
        if sub == 1 {
            return;
        }
        let (mut total, mut best) = (0.0, 0.0);
        for i in 0..6 {
            let v = crate::fpu::reg(self.game.player_unk6a8(slot).x10[i].x0);
            if best < v {
                best = v;
            }
            total = fp::fadds(total, v);
        }
        if total != 0.0 {
            let avg = fp::fdivs(fp::fsubs(total, best), total);
            let bar = crate::fpu::reg(self.data.f32(self.pl_data().at(0xD4)));
            if avg >= bar {
                self.set_point(slot, 0x58, 1);
            }
        }
    }

    /// Counts a point bonus for the player, `pl_80038824`. The original asserts the bonus
    /// counts points (`gmDecisionGetType`), which its callers' bonuses do.
    #[doc(alias = "pl_80038824")]
    pub(crate) fn count_bonus(&mut self, slot: i32, kind: usize) {
        assert!((0..6).contains(&slot), "player slot {slot}");
        let n = &mut self.game.players[slot as usize].stale_moves.x904[kind];
        *n = n.wrapping_add(1);
    }

    /// `pl_8003FE1C`: bonus 10, for either Ice Climber.
    #[doc(alias = "pl_8003FE1C")]
    pub fn pl_8003fe1c(&mut self, slot: i32, _sub: i32) {
        self.count_bonus(slot, 10);
    }

    /// Keeps the lowest of a value per player (`pl_StaleMoveTableExt_t::xCF8`, the word after
    /// the stale-move table), `pl_80040330`; Nana's don't count.
    #[doc(alias = "pl_80040330")]
    pub fn lower_xcf8(&mut self, slot: i32, sub: i32, v: f32) {
        if sub != 0 {
            return;
        }
        assert!((0..6).contains(&slot), "player slot {slot}");
        let bytes = &mut self.game.players[slot as usize].xDB0[4..8];
        let cur = f32::from_be_bytes(bytes.try_into().unwrap());
        if v < cur {
            bytes.copy_from_slice(&v.to_be_bytes());
        }
    }

    /// Counts an attack the fighter's item makes (its attack flags `ev`, `Struct2070`) in the
    /// fighter's player's statistics, `pl_80037DF4`.
    #[doc(alias = "pl_80037DF4")]
    pub fn count_item_attack(&mut self, gobj: crate::hsd::gobj::GObjId, ev: [u8; 4]) {
        let slot = self.game.fighter(gobj).player_idx as usize;
        assert!(slot < 6, "player slot {slot}");
        let a = &mut self.game.players[slot]
            .stale_moves
            .total_attack_count_struct
            .attacks;
        a.total = a.total.wrapping_add(1);
        let id = ev[3] as usize;
        assert!(
            id < a.by_attack_counts.len(),
            "item attack id {id} past the counts"
        );
        a.by_attack_counts[id] = a.by_attack_counts[id].wrapping_add(1);
        for (mask, n) in [
            (0x08, &mut a.thrown_item_count),
            (0x04, &mut a.aerials_count),
            (0x01, &mut a.specials_count),
            (0x02, &mut a.x1A0_count),
            (0x10, &mut a.x1A4_count),
        ] {
            if ev[2] & mask != 0 {
                *n = n.wrapping_add(1);
            }
        }
    }

    /// Counts the fighter's attack in its player's match statistics as its action state
    /// starts, `pl_80037C60` (`prev` is the last state's attack flags, `Fighter::x2070`).
    #[doc(alias = "pl_80037C60")]
    #[doc(alias = "Player_GetActionStats")]
    pub fn count_attack(&mut self, gobj: crate::hsd::gobj::GObjId, prev: [u8; 4]) {
        let fp = self.game.fighter(gobj);
        let (slot, sub, cur) = (fp.player_idx as usize, fp.is_sub_fighter, fp.x2070);
        assert!(slot < 6, "player slot {slot}");
        let acp = &mut self.game.players[slot]
            .stale_moves
            .total_attack_count_struct;
        if cur[2] & 0x20 != 0 {
            acp.x5BC_b0 = true;
        }
        if cur[2] & 0x40 != 0 {
            acp.x5BC_b1 = true;
        }
        if cur[2] & 0x80 != 0 {
            acp.x5BC_b2 = true;
        }
        if cur[1] & 0x01 != 0 {
            acp.x5BC_b3 = true;
        }
        let id = cur[3];
        if id == 0 || id == prev[3] {
            return;
        }
        // `StatsAttack_Count`, 0x64.
        if id >= 100 {
            if !sub {
                // `x358.x0.by_attack_hi[id]`, which the id indexes from the array's start:
                // past it, the words after.
                add_action_stat(acp, 0x358 + 0x90 + 4 * u32::from(id));
            }
            return;
        }
        let a = &mut acp.attacks;
        a.total = a.total.wrapping_add(1);
        a.by_attack_counts[id as usize] = a.by_attack_counts[id as usize].wrapping_add(1);
        for (mask, n) in [
            (0x08, &mut a.thrown_item_count),
            (0x04, &mut a.aerials_count),
            (0x01, &mut a.specials_count),
            (0x02, &mut a.x1A0_count),
            (0x10, &mut a.x1A4_count),
        ] {
            if cur[2] & mask != 0 {
                *n = n.wrapping_add(1);
            }
        }
    }
}

/// Adds one to the word at `off` in a player's action statistics.
fn add_action_stat(s: &mut plActionStats, off: u32) {
    let inc = |b: &mut [u8]| {
        let v = u32::from_be_bytes(b[..4].try_into().unwrap()).wrapping_add(1);
        b[..4].copy_from_slice(&v.to_be_bytes());
    };
    match off {
        0x358..0x504 => inc(&mut s.x358[(off - 0x358) as usize..]),
        0x504..0x568 => inc(&mut s.x504[(off - 0x504) as usize..]),
        0x568..0x598 => {
            let w = match off {
                0x568 => &mut s.x568,
                0x56C => &mut s.x56C,
                0x570 => &mut s.x570,
                0x574 => &mut s.x574,
                0x578 => &mut s.x578,
                0x57C => &mut s.x57C,
                0x580 => &mut s.x580,
                0x584 => &mut s.x584,
                0x588 => &mut s.x588,
                0x58C => &mut s.x58C,
                0x590 => &mut s.x590,
                _ => &mut s.x594,
            };
            *w = w.wrapping_add(1);
        }
        0x598..0x5B8 => {
            let w = &mut s.x598[((off - 0x598) / 4) as usize];
            *w = w.wrapping_add(1);
        }
        _ => panic!("an attack statistic at {off:#x} isn't modeled"),
    }
}
