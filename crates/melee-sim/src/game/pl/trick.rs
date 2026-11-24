//! What a landed attack records (`pltrick.c`'s `pl_80038144` and the bonus checks it runs):
//! the attacker's hit counts and the victim's record of the hit, its damage result's
//! `x18d8` (the attack's record, `ft_800898B4_t`) taken from the attacker's.

use gekko_fp as fp;

use super::super::state::Running;
use super::super::structs::ft_800898B4_t;
use super::bonus::Flag;
use super::types::AttackStats;
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::hsd::gobj::GObjId;

/// Where an attack's record (`ft_800898B4_t`) lives: a fighter's (`Fighter::x2074`) or an
/// item's (`Item::xD94`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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

/// A record's flag byte as bits, `b0` the top one.
fn flag_bits(b: u8) -> [bool; 8] {
    std::array::from_fn(|i| b & (0x80 >> i) != 0)
}

/// Counts a hit or an attack in a set of attack statistics, `pl_80037BC0`.
#[doc(alias = "pl_80037BC0")]
fn count_stats(s: &mut AttackStats, ev: [u8; 4]) {
    s.total = s.total.wrapping_add(1);
    let id = usize::from(ev[3]);
    s.by_attack_counts[id] = s.by_attack_counts[id].wrapping_add(1);
    for (mask, n) in [
        (0x08, &mut s.thrown_item_count),
        (0x04, &mut s.aerials_count),
        (0x01, &mut s.specials_count),
        (0x02, &mut s.strikes_count),
        (0x10, &mut s.reflectors_count),
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
                let a = &self.game.fighter(g).attack_record;
                (
                    a.percents.x,
                    a.percents.y,
                    a.kb_and_thrower.x,
                    a.kb_and_thrower.y,
                    [a.flags[0], a.flags[1]],
                )
            }
            AttackRecord::Item(g) => {
                let ip = super::super::it::item(self.game, g);
                let w = ip.attack_record_word.to_be_bytes();
                (
                    ip.attack_record_a.x,
                    ip.attack_record_a.y,
                    ip.attack_record_b.x,
                    ip.attack_record_b.y,
                    [w[0], w[1]],
                )
            }
        };
        let (a, b) = (flag_bits(flags[0]), flag_bits(flags[1]));
        ft_800898B4_t {
            attacker_percent: x0,
            victim_percent: x4,
            kb_applied1: kb,
            item_thrower: xc,
            spike: a[0],
            metal: a[1],
            bunny_hood: a[2],
            starman: a[3],
            cloak: a[4],
            giant: a[5],
            tiny: a[6],
            landed: a[7],
            from_behind: b[0],
            at_edge: b[1],
            on_ledge: b[2],
            spike_after_strong: b[3],
            forward_attack: b[4],
        }
    }

    /// An attack's record's bytes past its named flags: byte 0x11 (its low three bits count)
    /// and bytes 0x12 and 0x13, the fighter's `x2074`'s or the item's `xDA4` word's.
    fn attack_record_pad(&self, r: AttackRecord) -> [u8; 3] {
        match r {
            AttackRecord::Fighter(g) => {
                let x = self.game.fighter(g).attack_record.flags;
                [x[1], x[2], x[3]]
            }
            AttackRecord::Item(g) => {
                let w = super::super::it::item(self.game, g)
                    .attack_record_word
                    .to_be_bytes();
                [w[1], w[2], w[3]]
            }
        }
    }

    /// Marks the attack's record as one that landed (`x10_b7`).
    fn mark_record_landed(&mut self, r: AttackRecord) {
        match r {
            AttackRecord::Fighter(g) => self.game.fighter_mut(g).attack_record.flags[0] |= 0x01,
            AttackRecord::Item(g) => {
                super::super::it::item_mut(self.game, g).attack_record_word |= 0x0100_0000;
            }
        }
    }

    /// What a landed attack records, `pl_80038144`: from the attacker (a fighter, when there
    /// is one) its kind of attack against the victim (`pl_800410F4`) and whether it came from
    /// behind; for a new attack instance the victim keeps its flags and instance, and the
    /// attacker's statistics count the hit, its best hits, from behind or not, whom, and the
    /// bonus checks; then the victim's damage result takes the attack's record
    /// (`fn_80037F00`).
    #[doc(alias = "pl_800403FC")]
    #[doc(alias = "pl_80040D8C")]
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
        if instance == 0 || vf.dmg.instance_hit_by != instance {
            vf.dmg.attack_record = ev;
            vf.dmg.instance_hit_by = instance;
            let vteam = vf.team;
            if let Some((aslot, asub, ateam)) = a
                && id != 0
            {
                if self.game.vs_is_teams() && self.game.vs_friendly_fire() && ateam == vteam {
                    // `pl_80040D8C`.
                    let t = self.game.player_stale_moves(aslot);
                    t.teammate_hits = t.teammate_hits.wrapping_add(1);
                }
                if id < 100 {
                    let percent = self.game.fighter(victim).dmg.percent;
                    self.count_hit_stats(aslot, asub, vslot, vsub, percent, ev, &rec, behind);
                }
            }
        }
        self.take_attack_record(attacker, victim, record, rec, behind, prev_source);
    }

    /// A hit with no attacker, attack or record (`pl_80038144` with nulls): the hit's event
    /// and instance cleared, then the last hit's record (`fn_80037F00` with no record) as
    /// none, which leaves its top three flags.
    pub fn record_no_attack(&mut self, victim: GObjId, prev_source: i32) {
        let slot = i32::from(self.game.fighter(victim).player_idx);
        let edge = self.magnify_edge(slot);
        let fp = self.game.fighter_mut(victim);
        fp.dmg.attack_record = [0; 4];
        fp.dmg.instance_hit_by = 0;
        let prev_kb = if fp.in_hitstun {
            reg(fp.dmg.hit_records.kb_applied1)
        } else {
            0.0
        };
        let _ = prev_kb;
        let d = &mut fp.dmg.hit_records;
        d.attacker_percent = 0;
        d.victim_percent = 0;
        d.kb_applied1 = 0.0;
        d.item_thrower = 6;
        d.spike = false;
        d.spike_after_strong = false;
        d.metal = false;
        d.bunny_hood = false;
        d.starman = false;
        d.cloak = false;
        d.giant = false;
        d.tiny = false;
        d.landed = false;
        d.from_behind = false;
        d.at_edge = false;
        d.on_ledge = false;
        d.forward_attack = false;
        d.victim_percent = fp::fctiwz(reg(fp.dmg.percent));
        fp.dmg.hit_records.from_behind = false;
        if edge != 0 {
            fp.dmg.hit_records.at_edge = true;
        }
        if fp.motion_id == MS_X11_B2 {
            fp.dmg.hit_records.on_ledge = true;
        }
        // No strong-hit mark: the record's x10_b0 was just cleared.
        let _ = prev_source;
    }

    /// A hit on an enemy in the attacker's statistics, `pl_800384DC`: attack `id` (handed as an
    /// `int`, so the record's other bytes read 0) counts a hit, and a hit beating its best, as
    /// on a player; the fighter's record is marked landed (`x2084_b7`). `record` is the
    /// attack's record: the fighter's own, or its item's.
    #[doc(alias = "pl_800384DC")]
    pub fn record_enemy_hit(&mut self, gobj: GObjId, id: u8, record: AttackRecord) {
        let ev = [0, 0, 0, id];
        if id != 0 && id < 100 {
            let fp = self.game.fighter(gobj);
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            let rec = self.attack_record(record);
            let s = &mut self.game.player_stale_moves(slot).actions;
            count_stats(&mut s.hits, ev);
            if rec.spike {
                s.hits.spikes_count = s.hits.spikes_count.wrapping_add(1);
            }
            if id == 99 {
                // `pl_8003FE40`.
                self.count_bonus(slot, 75);
            }
            let s = &mut self.game.player_stale_moves(slot).actions;
            let best = s.attacks.by_attack_counts[usize::from(id)];
            if !rec.landed && best > word_at(&s.landed, 4 + 4 * usize::from(id)) {
                count_stats_bytes(&mut s.landed, ev, rec.spike);
                self.count_best_hit(slot, sub, i32::from(id));
            }
        }
        self.mark_record_landed(AttackRecord::Fighter(gobj));
    }

    /// The attacker's statistics for a hit with attack `ev` (`pl_80038144`'s body), with
    /// `pl_8003FE40`'s bonus for Luigi's taunt (attack 99, `ft_800895E0`'s rewrite of 0x71).
    #[doc(alias = "pl_8003FE40")]
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
        let s = &mut self.game.player_stale_moves(aslot).actions;
        count_stats(&mut s.hits, ev);
        if rec.spike {
            s.hits.spikes_count = s.hits.spikes_count.wrapping_add(1);
        }
        if id == 99 {
            // `pl_8003FE40`.
            self.count_bonus(aslot, 75);
        }
        let s = &mut self.game.player_stale_moves(aslot).actions;
        let best = s.attacks.by_attack_counts[usize::from(id)];
        if !rec.landed && best > word_at(&s.landed, 4 + 4 * usize::from(id)) {
            count_stats_bytes(&mut s.landed, ev, rec.spike);
            self.count_best_hit(aslot, asub, i32::from(id));
        }
        let s = &mut self.game.player_stale_moves(aslot).actions;
        if behind {
            s.back_hits = s.back_hits.wrapping_add(1);
        } else {
            s.front_hits = s.front_hits.wrapping_add(1);
        }
        if id == 8 || id == 0xB {
            s.down_hits = s.down_hits.wrapping_add(1);
        } else {
            s.other_hits = s.other_hits.wrapping_add(1);
        }
        assert!((0..8).contains(&vslot), "0 <= h_player && h_player < 8");
        let s = &mut self.game.player_stale_moves(aslot).actions;
        s.victims[usize::from(id)] |= 1 << (vslot as u8);
        self.first_hit_bonus(aslot);
        self.average_victim_percent(aslot, vslot, percent);
        self.self_hit_bonus(aslot, vslot, vsub, rec.item_thrower);
        if vsub == 0 && (84..=87).contains(&id) {
            // `pl_800403FC`.
            self.game.player_stale_moves(vslot).flowered_by = aslot as u32;
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
        let e = &mut self.game.player_combos(vslot).by_attacker[aslot as usize];
        if e.attack != 0 && e.attack == id {
            e.repeats = e.repeats.wrapping_add(1);
            if e.attack == 90 && i32::from(e.repeats) >= limit {
                self.set_flag(aslot, Flag::Bonus(195), 1);
            }
        } else {
            e.repeats = 1;
            e.attack = id;
        }
    }

    /// A hit's count on the victim's frame counters (`pl_80040FBC`): its hit while the victim's
    /// own taunt's window is open (`StatsExt::own_taunt_window`, `xD50`), and hits but by a
    /// few attacks, past `x4` a bonus unless teammates.
    #[doc(alias = "pl_80040FBC")]
    fn count_hits_on(&mut self, aslot: i32, vslot: i32, vsub: i32, id: i32) {
        if vsub == 1 {
            return;
        }
        let taunting = self.game.player_mut(vslot).ext.own_taunt_window != 0;
        let limit = self.bonus_param(4) as i32;
        let e = &mut self.game.player_combos(vslot).by_attacker[aslot as usize];
        if id == 1 && taunting {
            e.shield_jabbed = true;
        }
        if !matches!(id, 76 | 90 | 91 | 92 | 96) {
            e.air_hits = e.air_hits.wrapping_add(1);
        }
        if i32::from(e.air_hits) >= limit {
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
        if (0..6).any(|i| self.game.player_stale_moves(i).bonuses[26] != 0) {
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
        t.victim_percent_count = t.victim_percent_count.wrapping_add(1);
        let n = t.victim_percent_count;
        let sum = fp::fmadds(
            reg(t.victim_percent_sum),
            fp::frsp(f64::from(n.wrapping_sub(1))),
            reg(percent),
        );
        let total = fp::frsp(f64::from(n));
        assert!(
            total != 0.0,
            "the original's assertion failed: total != 0.0F"
        );
        t.victim_percent_sum = mem(fp::fdivs(sum, total));
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
    #[doc(alias = "pl_8003EAAC")]
    #[doc(alias = "ft_80087A8C")]
    #[doc(alias = "pl_8003FF44")]
    fn count_best_hit(&mut self, slot: i32, sub: i32, id: i32) {
        self.count_combo_order(slot, sub, id);
        // `pl_8003FF44`.
        if sub == 0 && (1..=16).contains(&id) {
            let g = self
                .game
                .player_entity_at(slot, sub)
                .expect("the player's fighter");
            let held = reg(self.game.fighter(g).smash_attrs.smash_since_hitbox);
            if held > reg(self.bonus_param_f32(0x38)) {
                let p = self.game.player_mut(slot);
                p.ext.ko_flags = p.ext.ko_flags | 0x08;
            }
        }
        // `pl_8003EAAC`.
        if sub == 0 && id == 0x59 {
            let t = self.game.player_stale_moves(slot);
            t.ray_gun_hits = t.ray_gun_hits.wrapping_add(1);
            let target = crate::game::it::kinds::lgun::shots_bonus(self.game, self.data);
            if self.game.player_stale_moves(slot).ray_gun_hits == target {
                self.set_point(slot, 194, 1);
            }
        }
    }

    /// `pl_8003FC88`: jab, jab, jab and a dash or tilt in order, bonus 0x56.
    #[doc(alias = "pl_8003FC88")]
    fn count_combo_order(&mut self, slot: i32, sub: i32, id: i32) {
        if sub == 1 {
            return;
        }
        let t = self.game.player_stale_moves(slot);
        let step = t.combo_step;
        let next = match step {
            0 => (1..=3).contains(&id),
            1 => (6..=8).contains(&id),
            2 => (9..=11).contains(&id),
            3 => (17..=48).contains(&id),
            _ => false,
        };
        if next {
            if step == 3 {
                t.combo_step = 0;
                self.count_bonus(slot, 0x56);
            } else {
                t.combo_step = step.wrapping_add(1);
            }
        } else {
            t.combo_step = 0;
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
        let prev_kb = if fp.in_hitstun {
            reg(fp.dmg.hit_records.kb_applied1)
        } else {
            0.0
        };
        let d = &mut fp.dmg.hit_records;
        *d = rec;
        d.victim_percent = fp::fctiwz(reg(fp.dmg.percent));
        // The copy is the record's whole 0x14 bytes, its padding too.
        let pad = self.attack_record_pad(record);
        let f = self.game.fighter_id(victim);
        self.game.ft.hit_record_pad.insert(f.0, pad);
        let fp = self.game.fighter_mut(victim);
        fp.dmg.hit_records.from_behind = behind;
        if edge != 0 {
            fp.dmg.hit_records.at_edge = true;
        }
        if fp.motion_id == MS_X11_B2 {
            fp.dmg.hit_records.on_ledge = true;
        }
        if fp.dmg.source_player != prev_source && fp.dmg.hit_records.spike && prev_kb >= strong {
            fp.dmg.hit_records.spike_after_strong = true;
        }
        if let Some(a) = attacker {
            let msid = i32::from(fp.dmg.attack_record[3]);
            let grabbed_by = fp.victim_gobj;
            if self.forward_attack(a, msid) && grabbed_by != Some(a) {
                self.game.fighter_mut(victim).dmg.hit_records.forward_attack = true;
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
        if (kind == FighterKind::Fox || kind == FighterKind::Falco) && low {
            return false;
        }
        if kind == FighterKind::Mewtwo && msid == MS_ATTACK_S3_S {
            return false;
        }
        true
    }
}
