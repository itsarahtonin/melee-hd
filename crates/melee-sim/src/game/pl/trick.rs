//! What a landed attack records (`pltrick.c`'s `pl_80038144` and the bonus checks it runs):
//! the attacker's hit counts and the victim's record of the hit, its damage result's
//! `x18d8` (the attack's record, `ft_800898B4_t`) taken from the attacker's.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::state::Running;
use super::super::structs::{ft_800898B4_t, plAttackStats};
use super::bonus::Flag;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// Where an attack's record (`ft_800898B4_t`) lives: a fighter's (`Fighter::x2074`) or an
/// item's (`Item::xD94`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackRecord {
    Fighter(GObjId),
    Item(GObjId),
}

/// `ftCo_MS_AttackS3Hi` to `ftCo_MS_AttackS4LwS`, `ftCo_MS_AttackS3LwS`, `ftCo_MS_AttackS3S`,
/// and the motion state `fn_80037F00` marks (`x11_b2`).
const MS_ATTACK_S3_HI: i32 = 0x33;
const MS_ATTACK_S4_LW_S: i32 = 0x3D;
const MS_ATTACK_S3_LW_S: i32 = 0x36;
const MS_ATTACK_S3_S: i32 = 0x35;
const MS_X11_B2: i32 = 253;
/// `FighterKind`s `ft_80089914` treats apart.
const KIND_FOX: i32 = 1;
const KIND_FALCO: i32 = 22;
const KIND_MEWTWO: i32 = 16;

/// A record's flag byte as bits, `b0` the top one.
fn flag_bits(b: u8) -> [bool; 8] {
    std::array::from_fn(|i| b & (0x80 >> i) != 0)
}

/// Counts a hit or an attack in a set of attack statistics, `pl_80037BC0`.
#[doc(alias = "pl_80037BC0")]
fn count_stats(s: &mut plAttackStats, ev: [u8; 4]) {
    s.total = s.total.wrapping_add(1);
    let id = usize::from(ev[3]);
    s.by_attack_counts[id] = s.by_attack_counts[id].wrapping_add(1);
    for (mask, n) in [
        (0x08, &mut s.thrown_item_count),
        (0x04, &mut s.aerials_count),
        (0x01, &mut s.specials_count),
        (0x02, &mut s.x1A0_count),
        (0x10, &mut s.x1A4_count),
    ] {
        if ev[2] & mask != 0 {
            *n = n.wrapping_add(1);
        }
    }
}

/// [`count_stats`] on the statistics `x358` keeps as bytes (`x358_hits`), with its `x1A8`.
fn count_stats_bytes(b: &mut [u8], ev: [u8; 4], x1a8: bool) {
    let mut inc = |off: usize| {
        let v = u32::from_be_bytes(b[off..off + 4].try_into().unwrap()).wrapping_add(1);
        b[off..off + 4].copy_from_slice(&v.to_be_bytes());
    };
    inc(0);
    inc(4 + 4 * usize::from(ev[3]));
    for (mask, off) in [
        (0x08, 0x194),
        (0x04, 0x198),
        (0x01, 0x19C),
        (0x02, 0x1A0),
        (0x10, 0x1A4),
    ] {
        if ev[2] & mask != 0 {
            inc(off);
        }
    }
    if x1a8 {
        inc(0x1A8);
    }
}

fn word_at(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
}

impl Running<'_> {
    /// An attack's record as `ft_800898B4_t`.
    fn attack_record(&self, r: AttackRecord) -> ft_800898B4_t {
        let (x0, x4, kb, xc, flags) = match r {
            AttackRecord::Fighter(g) => {
                let a = &self.game.fighter(g).x2074;
                (
                    a.x2074_vec.x,
                    a.x2074_vec.y,
                    a.x207C.x,
                    a.x207C.y,
                    [a.x10[0], a.x10[1]],
                )
            }
            AttackRecord::Item(g) => {
                let ip = super::super::it::item(self.game, g);
                let w = ip.xDA4_word.to_be_bytes();
                (ip.xD94.x, ip.xD94.y, ip.xD9C.x, ip.xD9C.y, [w[0], w[1]])
            }
        };
        let (a, b) = (flag_bits(flags[0]), flag_bits(flags[1]));
        ft_800898B4_t {
            x0,
            x4,
            kb_applied1: kb,
            xC: xc,
            x10_b0: a[0],
            x10_b1: a[1],
            x10_b2: a[2],
            x10_b3: a[3],
            x10_b4: a[4],
            x10_b5: a[5],
            x10_b6: a[6],
            x10_b7: a[7],
            x11_b0: b[0],
            x11_b1: b[1],
            x11_b2: b[2],
            x11_b3: b[3],
            x11_b4: b[4],
        }
    }

    /// Marks the attack's record as one that landed (`x10_b7`).
    fn mark_record_landed(&mut self, r: AttackRecord) {
        match r {
            AttackRecord::Fighter(g) => self.game.fighter_mut(g).x2074.x10[0] |= 0x01,
            AttackRecord::Item(g) => {
                super::super::it::item_mut(self.game, g).xDA4_word |= 0x0100_0000;
            }
        }
    }

    /// What a landed attack records, `pl_80038144`: from the attacker (a fighter, when there
    /// is one) its kind of attack against the victim (`pl_800410F4`) and whether it came from
    /// behind; for a new attack instance the victim keeps its flags and instance, and the
    /// attacker's statistics count the hit, its best hits, from behind or not, whom, and the
    /// bonus checks; then the victim's damage result takes the attack's record
    /// (`fn_80037F00`).
    #[doc(alias = "pl_80038144")]
    #[allow(clippy::too_many_arguments)]
    pub fn record_attack(
        &mut self,
        attacker: Option<GObjId>,
        victim: GObjId,
        x2070: u32,
        record: AttackRecord,
        instance: u16,
        prev_source: i32,
    ) {
        let ev = x2070.to_be_bytes();
        let id = ev[3];
        let vf = self.game.fighter(victim);
        let (vslot, vsub) = (i32::from(vf.player_idx), i32::from(vf.is_sub_fighter));
        let mut behind = false;
        let a = attacker.map(|g| {
            let af = self.game.fighter(g);
            (
                i32::from(af.player_idx),
                i32::from(af.is_sub_fighter),
                af.team,
            )
        });
        if let (Some(g), Some((aslot, asub, _))) = (attacker, a)
            && id != 0
        {
            let facing = reg(self.game.fighter(victim).facing_dir);
            let ax = reg(self.game.fighter(g).cur_pos.x);
            let vx = reg(self.game.fighter(victim).cur_pos.x);
            if fp::fmuls(facing, ax) < fp::fmuls(facing, vx)
                && !(id == 0x60 || id == 0x61 || (0x33..=0x3D).contains(&id))
            {
                behind = true;
            }
            self.count_attack_kind(aslot, asub, vslot, vsub, i32::from(id));
        }
        let rec = self.attack_record(record);
        let vf = self.game.fighter_mut(victim);
        if instance == 0 || vf.dmg.x18ec_instancehitby != instance {
            vf.dmg.x18d4 = ev;
            vf.dmg.x18ec_instancehitby = instance;
            let vteam = vf.team;
            if let Some((aslot, asub, ateam)) = a
                && id != 0
            {
                if self.game.vs_is_teams() && self.game.vs_friendly_fire() && ateam == vteam {
                    // `pl_80040D8C`.
                    let t = self.game.player_stale_moves(aslot);
                    t.xCB0 = t.xCB0.wrapping_add(1);
                }
                if id < 100 {
                    let percent = self.game.fighter(victim).dmg.x1830_percent;
                    self.count_hit_stats(aslot, asub, vslot, vsub, percent, ev, &rec, behind);
                }
            }
        }
        self.take_attack_record(attacker, victim, record, rec, behind, prev_source);
    }

    /// The attacker's statistics for a hit with attack `ev` (`pl_80038144`'s body).
    #[allow(clippy::too_many_arguments)]
    fn count_hit_stats(
        &mut self,
        aslot: i32,
        asub: i32,
        vslot: i32,
        vsub: i32,
        percent: f32,
        ev: [u8; 4],
        rec: &ft_800898B4_t,
        behind: bool,
    ) {
        let id = ev[3];
        let s = &mut self
            .game
            .player_stale_moves(aslot)
            .total_attack_count_struct;
        count_stats(&mut s.hits, ev);
        if rec.x10_b0 {
            s.hits.x1A8 = s.hits.x1A8.wrapping_add(1);
        }
        if id == 99 {
            // `pl_8003FE40`.
            self.count_bonus(aslot, 75);
        }
        let s = &mut self
            .game
            .player_stale_moves(aslot)
            .total_attack_count_struct;
        let best = s.attacks.by_attack_counts[usize::from(id)];
        if !rec.x10_b7 && best > word_at(&s.x358, 4 + 4 * usize::from(id)) {
            count_stats_bytes(&mut s.x358, ev, rec.x10_b0);
            self.count_best_hit(aslot, asub, i32::from(id));
        }
        let s = &mut self
            .game
            .player_stale_moves(aslot)
            .total_attack_count_struct;
        if behind {
            s.x56C = s.x56C.wrapping_add(1);
        } else {
            s.x568 = s.x568.wrapping_add(1);
        }
        if id == 8 || id == 0xB {
            s.x574 = s.x574.wrapping_add(1);
        } else {
            s.x570 = s.x570.wrapping_add(1);
        }
        assert!((0..8).contains(&vslot), "0 <= h_player && h_player < 8");
        let s = &mut self
            .game
            .player_stale_moves(aslot)
            .total_attack_count_struct;
        s.x504[usize::from(id)] |= 1 << (vslot as u8);
        self.first_hit_bonus(aslot);
        self.average_victim_percent(aslot, vslot, percent);
        self.self_hit_bonus(aslot, vslot, vsub, rec.xC);
        if vsub == 0 && (84..=87).contains(&id) {
            // `pl_800403FC`.
            self.game.player_stale_moves(vslot).xCAC = aslot as u32;
        }
        self.count_hits_on(aslot, vslot, vsub, i32::from(id));
    }

    /// A hit with attack `id` on the victim's frame counters (`pl_800410F4`): the same attack
    /// again counts up, Fox's and Falco's laser (90) to a bonus.
    #[doc(alias = "pl_800410F4")]
    fn count_attack_kind(&mut self, aslot: i32, _asub: i32, vslot: i32, vsub: i32, id: i32) {
        if vsub == 1 {
            return;
        }
        let limit = self.bonus_param(0x17C) as i32;
        let e = &mut self.game.player_unk6a8(vslot).x10[aslot as usize];
        if e.xC != 0 && e.xC == id {
            e.x8 = e.x8.wrapping_add(1);
            if e.xC == 90 && i32::from(e.x8) >= limit {
                self.set_flag(aslot, Flag::Bonus(195), 1);
            }
        } else {
            e.x8 = 1;
            e.xC = id;
        }
    }

    /// A hit's count on the victim's frame counters (`pl_80040FBC`): its hit while the victim
    /// shields (`xD50`), and hits but by a few attacks, past `x4` a bonus unless teammates.
    #[doc(alias = "pl_80040FBC")]
    fn count_hits_on(&mut self, aslot: i32, vslot: i32, vsub: i32, id: i32) {
        if vsub == 1 {
            return;
        }
        let shielding = self.game.player_mut(vslot).ext_u32(0xD50) != 0;
        let limit = self.bonus_param(4) as i32;
        let e = &mut self.game.player_unk6a8(vslot).x10[aslot as usize];
        if id == 1 && shielding {
            e.x12_b0 = true;
        }
        if !matches!(id, 76 | 90 | 91 | 92 | 96) {
            e.x10 = e.x10.wrapping_add(1);
        }
        if i32::from(e.x10) >= limit {
            let same = self.game.vs_is_teams()
                && self.game.player_team(aslot) == self.game.player_team(vslot);
            if !same {
                self.set_flag(aslot, Flag::Bonus(2), 1);
            }
        }
    }

    /// `pl_8003FE64`: the first hit of the match, bonus 26, if no player has it yet.
    #[doc(alias = "pl_8003FE64")]
    fn first_hit_bonus(&mut self, aslot: i32) {
        if (0..6).any(|i| self.game.player_stale_moves(i).x904[26] != 0) {
            return;
        }
        self.set_flag(aslot, Flag::Bonus(26), 1);
    }

    /// The running average of the victims' percent when hit (`xCA0`, `pl_8003ED0C`), other
    /// players' who aren't teammates.
    #[doc(alias = "pl_8003ED0C")]
    fn average_victim_percent(&mut self, aslot: i32, vslot: i32, percent: f32) {
        if aslot == vslot {
            return;
        }
        if self.game.vs_is_teams() && self.game.player_team(vslot) == self.game.player_team(aslot) {
            return;
        }
        let t = self.game.player_stale_moves(aslot);
        t.xCA4 = t.xCA4.wrapping_add(1);
        let n = t.xCA4;
        let sum = fp::fmadds(
            reg(t.xCA0),
            fp::frsp(f64::from(n.wrapping_sub(1))),
            reg(percent),
        );
        let total = fp::frsp(f64::from(n));
        assert!(
            total != 0.0,
            "the original's assertion failed: total != 0.0F"
        );
        t.xCA0 = mem(fp::fdivs(sum, total));
    }

    /// `pl_8003EA40`: bonus 155 when the record names the victim's own slot.
    #[doc(alias = "pl_8003EA40")]
    fn self_hit_bonus(&mut self, aslot: i32, vslot: i32, vsub: i32, xc: i32) {
        if vsub != 0 || vslot != xc {
            return;
        }
        self.count_bonus(aslot, 155);
    }

    /// A hit that beats the attack's best (`pl_8003DFF4`): the combo order (`pl_8003FC88`), a
    /// charged smash (`pl_8003FF44`) and the Ray Gun's (`pl_8003EAAC`).
    #[doc(alias = "pl_8003DFF4")]
    fn count_best_hit(&mut self, slot: i32, sub: i32, id: i32) {
        self.count_combo_order(slot, sub, id);
        // `pl_8003FF44`.
        if sub == 0 && (1..=16).contains(&id) {
            let g = self
                .game
                .player_entity_at(slot, sub)
                .expect("the player's fighter");
            let held = reg(self.game.fighter(g).smash_attrs.x2138_smashSinceHitbox);
            if held > reg(self.bonus_param_f32(0x38)) {
                let p = self.game.player_mut(slot);
                p.set_ext_u8(0xDD1, p.ext_u8(0xDD1) | 0x08);
            }
        }
        // `pl_8003EAAC`.
        if sub == 0 && id == 0x59 {
            let t = self.game.player_stale_moves(slot);
            t.xCA8 = t.xCA8.wrapping_add(1);
            unported_call("it_8026C1D4 (the Ray Gun's timer, pl_8003EAAC)");
        }
    }

    /// `pl_8003FC88`: jab, jab, jab and a dash or tilt in order, bonus 0x56.
    #[doc(alias = "pl_8003FC88")]
    fn count_combo_order(&mut self, slot: i32, sub: i32, id: i32) {
        if sub == 1 {
            return;
        }
        let t = self.game.player_stale_moves(slot);
        let step = t.xC90;
        let next = match step {
            0 => (1..=3).contains(&id),
            1 => (6..=8).contains(&id),
            2 => (9..=11).contains(&id),
            3 => (17..=48).contains(&id),
            _ => false,
        };
        if next {
            if step == 3 {
                t.xC90 = 0;
                self.count_bonus(slot, 0x56);
            } else {
                t.xC90 = step.wrapping_add(1);
            }
        } else {
            t.xC90 = 0;
        }
    }

    /// The victim's damage result takes the attack's record, `fn_80037F00`: its percent then,
    /// whether from behind, off screen or in the state `x11_b2` marks, whether a strong hit
    /// took it from another attacker, and whether a forward tilt or smash by someone not
    /// grabbing it; the attacker's record is then marked landed.
    #[doc(alias = "fn_80037F00")]
    fn take_attack_record(
        &mut self,
        attacker: Option<GObjId>,
        victim: GObjId,
        record: AttackRecord,
        rec: ft_800898B4_t,
        behind: bool,
        prev_source: i32,
    ) {
        let strong = reg(self.bonus_param_f32(0x14));
        let slot = i32::from(self.game.fighter(victim).player_idx);
        let edge = self.magnify_edge(slot);
        let fp = self.game.fighter_mut(victim);
        let prev_kb = if fp.x221C_b6 {
            reg(fp.dmg.x18d8.kb_applied1)
        } else {
            0.0
        };
        let d = &mut fp.dmg.x18d8;
        *d = rec;
        d.x4 = fp::fctiwz(reg(fp.dmg.x1830_percent));
        let fp = self.game.fighter_mut(victim);
        fp.dmg.x18d8.x11_b0 = behind;
        if edge != 0 {
            fp.dmg.x18d8.x11_b1 = true;
        }
        if fp.motion_id == MS_X11_B2 {
            fp.dmg.x18d8.x11_b2 = true;
        }
        if fp.dmg.x18c4_source_ply != prev_source && fp.dmg.x18d8.x10_b0 && prev_kb >= strong {
            fp.dmg.x18d8.x11_b3 = true;
        }
        if let Some(a) = attacker {
            let msid = i32::from(fp.dmg.x18d4[3]);
            let grabbed_by = fp.victim_gobj;
            if self.forward_attack(a, msid) && grabbed_by != Some(a) {
                self.game.fighter_mut(victim).dmg.x18d8.x11_b4 = true;
            }
            self.mark_record_landed(record);
        }
    }

    /// Whether a state is a forward tilt or smash, Fox's, Falco's and Mewtwo's own aside,
    /// `ft_80089914`.
    #[doc(alias = "ft_80089914")]
    fn forward_attack(&self, gobj: GObjId, msid: i32) -> bool {
        if !(MS_ATTACK_S3_HI..=MS_ATTACK_S4_LW_S).contains(&msid) {
            return false;
        }
        let kind = self.game.fighter(gobj).kind;
        let low = (msid - MS_ATTACK_S3_LW_S) as u32 <= 2;
        if (kind == KIND_FOX || kind == KIND_FALCO) && low {
            return false;
        }
        if kind == KIND_MEWTWO && msid == MS_ATTACK_S3_S {
            return false;
        }
        true
    }
}
