//! Fighters' attack collision (`ftcoll.c`, `Fighter_procAttackColl`): each fighter, once a
//! frame, tests the other fighters' hitboxes against its own hitboxes (clanks), shield and
//! hurtboxes, logs the hits it takes, and then turns the strongest logged hit into the
//! knockback its damage state applies later.
//!
//! The two damage logs (`dmg_log0`, the hits; `dmg_log1`, the phantom "tip" hits) and the
//! clank flags (`ftColl_804D6560`) are scratch: the process starts them over each time and
//! nothing reads them after it, so they live in [`HitLogs`] and stay out of the mirror.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::pl::trick::AttackRecord;
use super::super::state::Running;
use super::common::{GA_AIR, GA_GROUND};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::lb::collision::hitboxes_touch;
use crate::lb::hitcapsule::{self, Victim};
use crate::mtx::Mtx34;
use crate::ppc::cvt_fp2unsigned;
use crate::vec::Vec3;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

/// `HitElement`s the collision treats apart.
const ELEMENT_ELECTRIC: u32 = 2;
const ELEMENT_SLASH: u32 = 3;
const ELEMENT_CATCH: u32 = 8;
const ELEMENT_INERT: u32 = 11;
const ELEMENT_LIPSTICK: u32 = 15;

/// `HurtCapsule_Enabled`: a hurtbox that takes hits.
const HURT_ENABLED: i32 = 0;

/// `ftCo_MS_DamageIce`.
const MS_DAMAGE_ICE: i32 = 325;

/// `ItCo.dat`'s common item data (`it_804D6D28`): the speed below which an item's hit
/// pushes away from it rather than along its flight, for the damage result and for its
/// own record; and the speed damage's slope and offset (`x80_float[5]`, `[6]`).
const IT_SLOW_FOR_RESULT: u32 = 0x78;
const IT_SLOW_FOR_ITEM: u32 = 0xD4;
const IT_SPEED_DAMAGE: u32 = 0x94;
const IT_SPEED_DAMAGE_BASE: u32 = 0x98;

/// The hit sounds by severity (`ftColl_803C0C40`) and the slash-on-slash clank sounds
/// (`ftColl_803C0C4C`).
const HIT_SFX: [i32; 3] = [141, 142, 143];
const SLASH_CLANK_SFX: [i32; 3] = [107, 108, 109];

/// The effect a hit of each element spawns (`hit_effect_ids`), -1 for none.
const HIT_EFFECTS: [i32; 17] = [
    1000, 1002, 1001, 1004, 1145, 1005, -1, -1, -1, 1000, 1000, -1, -1, 1046, -1, -1, 0,
];

/// The sounds `lbColl_80005BB0` plays for a hit, by kind and severity (`lbColl_803B9880`).
const HIT_KIND_SFX: [i32; 42] = [
    540000, 540000, 540000, 91, 90, 89, 88, 87, 86, 111, 112, 113, 84, 84, 84, 90, 89, 223, 225,
    225, 225, 98, 99, 100, 101, 102, 103, 280091, 280091, 280091, 241, 241, 241, 94, 93, 92,
    220079, 220082, 220085, 540000, 540000, 525,
];

/// The fighter's bubbles an item's hitbox can meet.
#[derive(Clone, Copy, melee_derive::StateEq)]
enum Bubble {
    Shield,
    Reflect,
    Absorb,
}

/// How many entries each damage log holds.
const LOG_LEN: usize = 20;

/// The hitbox a logged hit came from (`DmgLogEntry::xC`): one of a fighter's (`x0` 1) or of
/// an item's (2), by its GObj.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum LoggedHit {
    Fighter {
        fighter: FighterId,
        index: usize,
    },
    Item {
        item: GObjId,
        index: usize,
    },
    /// A throw's hitbox (`Fighter::xDF4`), which deals its hit directly.
    Throw {
        fighter: FighterId,
        index: usize,
    },
    /// A stage device's hit (`x0` 3, `ftCo_800C08A0`): its description in the stage's data
    /// (`lbColl_80008D30_arg1`), which the log's hitbox word points at.
    Device {
        desc: melee_data::archive::DataRef,
    },
}

/// A logged hit (`DmgLogEntry`): a fighter's or an item's. The stage's stop at their seams
/// before they would log.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct LogEntry {
    /// The attacker's character (`kind`) and GObj, the thrown fighter's thrower for a thrown
    /// fighter's hitbox.
    pub kind: i32,
    pub gobj: Option<GObjId>,
    pub hit: LoggedHit,
    /// The hit hurtbox of the fighter the log belongs to (`x10`).
    pub hurt: usize,
    pub pos: Vec3,
    /// The damage (`x20`): the hitbox's for a hit, the halved for a tip.
    pub damage: f32,
    /// The hitbox's `unk_count` (halved for a tip), its knockback's damage count.
    pub count: u32,
}

/// The attack collision's scratch: the damage logs and the clank flags.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct HitLogs {
    pub hits: Vec<LogEntry>,
    pub tips: Vec<LogEntry>,
    pub clank: [bool; 4],
    /// The hit log's damage words (`dmg_log0[i].x20`) as their last entries left them: a
    /// device's entry writes none and keeps its slot's.
    pub stale_damage: [f32; LOG_LEN],
}

/// A hit's damage as an integer, at least 1 for any damage, `getEnvDmg`.
pub(crate) fn whole_damage(dmg: f64) -> i32 {
    if dmg != 0.0 {
        let i = fp::fctiwz(dmg);
        if i != 0 { i } else { 1 }
    } else {
        0
    }
}

/// Which fighter a hitbox's GObj carries, for the victim lists.
pub(super) fn victim(f: FighterId) -> Victim {
    Victim::Fighter(f)
}

/// `Fighter_procAttackColl`: unless the fighter sleeps or is out (`x2219_b1`), the frame's
/// hits on it from fighters, coins, items, stage devices and other thrown fighters, then what
/// they do, and the lipstick flower's damage over time.
#[doc(alias = "Fighter_procAttackColl")]
pub fn proc_attack_coll(r: &mut Running, gobj: GObjId) {
    let fp = r.game.fighter(gobj);
    if fp.is_sleeping || fp.out_of_play {
        return;
    }
    r.clear_hit_logs();
    r.hits_from_fighters(gobj);
    r.coin_pickups(gobj);
    r.hits_from_items(gobj);
    r.touch_devices(gobj);
    r.thrown_hits(gobj);
    r.take_hits(gobj);
    r.take_tip_hits(gobj);
    let lipstick = r.lipstick_damage();
    if lipstick > 0.0 {
        r.flower_hit(gobj, lipstick);
    }
}

impl Running<'_> {
    /// Starts both damage logs over, `ftColl_800765E0`.
    #[doc(alias = "ftColl_800765E0")]
    pub fn clear_hit_logs(&mut self) {
        let logs = &mut self.game.ft.hit_logs;
        for (i, e) in logs.hits.iter().enumerate() {
            logs.stale_damage[i] = e.damage;
        }
        self.game.ft.hit_logs.hits.clear();
        self.game.ft.hit_logs.tips.clear();
    }

    /// Spawns an effect at `pos` with no owner, `efSync_Spawn(id, NULL, pos[, &value])`.
    fn spawn_effect(&mut self, id: i32, pos: Vec3, value: Option<f32>) {
        use crate::game::ef::Arg;
        match value {
            Some(v) => crate::game::ef::spawn(self, id, None, &[Arg::Vec(pos), Arg::Float(v)]),
            None => crate::game::ef::spawn(self, id, None, &[Arg::Vec(pos)]),
        };
    }

    /// The fighter's own matrix when its scale's depth isn't 1 (Flat Zone's flat fighters),
    /// `ftCommon_8007F804`.
    #[doc(alias = "ftCommon_8007F804")]
    pub fn own_matrix(&self, f: FighterId) -> Option<Mtx34> {
        let fp = &self.game.fighters[f.0];
        (reg(fp.scale.z) != 1.0).then_some(Mtx34 {
            m: fp.depth_scale_mtx,
        })
    }

    /// A thrown fighter's thrower (`x1064_thrownHitbox.x134.owner`, `Fighter::x1198`).
    fn thrower(&self, f: FighterId) -> Option<GObjId> {
        let x134 = self.game.fighters[f.0].thrown_hitbox.owner;
        if x134.is_set() && x134.thrower().is_none() {
            unported_call("a thrown fighter's hitbox word that isn't a thrower");
        }
        x134.thrower()
    }

    /// The highest of the fighter's invincibility states, `ftColl_8007B868`: 2 makes it
    /// intangible, 1 invincible.
    #[doc(alias = "ftColl_8007B868")]
    pub fn hit_status(&self, f: FighterId) -> i32 {
        let fp = &self.game.fighters[f.0];
        i32::from(fp.starman)
            .max(fp.hit_status)
            .max(fp.timed_hit_status)
    }

    /// The other fighters' hitboxes against this one, `ftColl_80078C70`: each other fighter
    /// not on its side (team rules aside), whose boxes touch its boxes (a clank, between two
    /// grounded fighters, tested only against fighters after it in the list), its shield, or
    /// a hurtbox.
    #[doc(alias = "ftColl_80078C70")]
    pub fn hits_from_fighters(&mut self, gobj: GObjId) {
        if self.game.vs_no_fighter_hits() {
            return;
        }
        let me = self.game.fighter_id(gobj);
        let mut passed = false;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(other) = cur {
            if other == gobj {
                passed = true;
            } else {
                self.hits_from_fighter(gobj, me, other, passed);
            }
            cur = self.game.world.gobjs[other.0].next;
        }
    }

    fn hits_from_fighter(&mut self, gobj: GObjId, me: FighterId, other: GObjId, passed: bool) {
        let v = self.game.fighter_id(other);
        let thrown = self.thrower(v).is_some();
        let (mf, vf) = (&self.game.fighters[me.0], &self.game.fighters[v.0]);
        if thrown {
            if mf.player_idx == vf.thrower_slot {
                return;
            }
        } else if mf.player_idx == vf.player_idx {
            return;
        }
        let other_team = if thrown { vf.thrown_team } else { vf.team };
        let my_team = mf.team;
        let other_b4 = vf.hits_others_team;
        if self.game.vs_is_teams()
            && !self.game.vs_friendly_fire()
            && !other_b4
            && my_team == other_team
        {
            return;
        }
        // The thrower can't hit the fighter it throws (`x1198 == this_gobj`).
        if self.thrower(v) == Some(gobj) {
            return;
        }
        let mut count = 0;
        if passed && !self.game.fighters[me.0].state_flags.x221B_b5 {
            let other_ga = self.game.fighters[v.0].ground_or_air;
            for i in 0..4 {
                let h = &self.game.fighters[me.0].hitboxes[i];
                let can = h.state != hitcapsule::DISABLED
                    && !h.phantom
                    && h.element != ELEMENT_CATCH
                    && h.element != ELEMENT_INERT
                    && h.clanks
                    && h.hits_fighters
                    && ((h.hits_air && other_ga == GA_AIR)
                        || (h.hits_ground && other_ga == GA_GROUND))
                    && !hitcapsule::has_victim(h, victim(v));
                self.game.ft.hit_logs.clank[i] = can;
                if can {
                    count += 1;
                }
            }
        }
        for j in 0..4 {
            self.hit_from_box(gobj, me, other, v, j, passed, count);
        }
    }

    /// One of the other fighter's hitboxes against this fighter.
    #[allow(clippy::too_many_arguments)]
    fn hit_from_box(
        &mut self,
        gobj: GObjId,
        me: FighterId,
        other: GObjId,
        v: FighterId,
        j: usize,
        passed: bool,
        count: i32,
    ) {
        let (mf, vf) = (&self.game.fighters[me.0], &self.game.fighters[v.0]);
        let h0 = &vf.hitboxes[j];
        if h0.state == hitcapsule::DISABLED || h0.element == ELEMENT_CATCH || !h0.hits_fighters {
            return;
        }
        if !((h0.hits_air && mf.ground_or_air == GA_AIR)
            || (h0.hits_ground && mf.ground_or_air == GA_GROUND))
        {
            return;
        }
        // A box that only hits its owner's grab victim.
        if h0.owner.grabbed_victim_only()
            && vf.victim_gobj.is_some()
            && vf.state_flags.x221B_b5
            && vf.victim_gobj != Some(gobj)
        {
            return;
        }
        if hitcapsule::has_victim(h0, victim(me)) {
            return;
        }
        let phantom = h0.phantom;
        let element = h0.element;
        if passed
            && !phantom
            && vf.victim_gobj != Some(gobj)
            && mf.ground_or_air == GA_GROUND
            && vf.ground_or_air == GA_GROUND
            && !mf.state_flags.x221B_b5
            && element != ELEMENT_INERT
            && h0.clanks
            && count != 0
        {
            for m in 0..4 {
                if self.game.ft.hit_logs.clank[m]
                    && self.boxes_touch(v, j, me, m)
                    && self.clank(v, j, me, m)
                {
                    return;
                }
            }
        }
        let _ = other;
        let mf = &self.game.fighters[me.0];
        let mut bubble_hit = false;
        if mf.state_flags.x221B_b0 {
            let vx = self.game.fighters[v.0].cur_pos.x;
            let mut facing = true;
            if mf.state_flags.x221B_b3 {
                if -1.0 == reg(mf.facing_dir) {
                    if reg(mf.cur_pos.x) < reg(vx) {
                        facing = false;
                    }
                } else if reg(mf.cur_pos.x) > reg(vx) {
                    facing = false;
                }
            }
            if mf.state_flags.x221B_b4 && !self.game.fighters[v.0].hitboxes[j].meets_reflector {
                facing = false;
            }
            bubble_hit = facing && self.box_hits_shield(v, j, me, phantom);
        }
        if bubble_hit {
            if element != ELEMENT_INERT {
                self.shield_hit(v, j, me);
                return;
            }
            let vf = &mut self.game.fighters[v.0];
            vf.hit_this_frame = true;
            vf.hit_by = Some(gobj);
        }
        let mf = &self.game.fighters[me.0];
        if mf.hit_status == 2 || mf.timed_hit_status == 2 {
            return;
        }
        let mut n = 0;
        while n < usize::from(self.game.fighters[me.0].hurt_capsules_len) {
            if self.box_hits_hurtbox(v, j, me, n, phantom) {
                if element == ELEMENT_INERT {
                    self.game.fighters[v.0].hit_by = Some(gobj);
                } else if self.take_hit(v, j, me, n) {
                    self.hit_sound(v, j, me, n);
                }
                return;
            }
            n += 1;
        }
    }

    /// Whether the other fighter's box `j` and this fighter's box `m` touch
    /// (`lbColl_80007AFC`), each keeping where it met the other.
    fn boxes_touch(&mut self, v: FighterId, j: usize, me: FighterId, m: usize) -> bool {
        let mut a = hitcapsule::shape(&self.game.fighters[v.0].hitboxes[j]);
        let mut b = hitcapsule::shape(&self.game.fighters[me.0].hitboxes[m]);
        let (sa, sb) = (
            self.game.fighters[v.0].scale.y,
            self.game.fighters[me.0].scale.y,
        );
        let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
        self.game.fighters[v.0].hitboxes[j].hurt_coll_pos = a.contact;
        self.game.fighters[me.0].hitboxes[m].hurt_coll_pos = b.contact;
        touch
    }

    /// Whether the other fighter's box `j` touches this fighter's shield (`lbColl_80007BCC`).
    fn box_hits_shield(&mut self, v: FighterId, j: usize, me: FighterId, phantom: bool) -> bool {
        let mtx = self.own_matrix(me);
        let mut hit = self.game.fighters[v.0].hitboxes[j].clone();
        let hit_scale = self.game.fighters[v.0].scale.y;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.scale.y, fp.cur_pos.z);
        let touch = hitcapsule::hits_bubble(
            &mut self.game.objs,
            self.data,
            &mut hit,
            &mut fp.shield_hit,
            mtx.as_ref(),
            phantom,
            hit_scale,
            scale,
            z,
        );
        let h = &mut self.game.fighters[v.0].hitboxes[j];
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// Whether the other fighter's box `j` touches this fighter's hurtbox `n`
    /// (`lbColl_8000805C`).
    fn box_hits_hurtbox(
        &mut self,
        v: FighterId,
        j: usize,
        me: FighterId,
        n: usize,
        phantom: bool,
    ) -> bool {
        let mtx = self.own_matrix(me);
        let mut hit = self.game.fighters[v.0].hitboxes[j].clone();
        let hit_scale = self.game.fighters[v.0].scale.y;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.scale.y, fp.cur_pos.z);
        let touch = hitcapsule::hits_hurtbox(
            &mut self.game.objs,
            self.data,
            &mut hit,
            &mut fp.hurt_capsules[n].capsule,
            mtx.as_ref(),
            phantom,
            hit_scale,
            scale,
            z,
        );
        let h = &mut self.game.fighters[v.0].hitboxes[j];
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// Remembers `victim` in each live box of `f`'s group of box `i` (`ftColl_80076808`);
    /// with `clear`, a box that newly hits it can't clank this frame.
    #[doc(alias = "ftColl_80076808")]
    pub(crate) fn hit_group(&mut self, f: FighterId, i: usize, kind: i32, v: Victim, clear: bool) {
        let group = self.game.fighters[f.0].hitboxes[i].group;
        for k in 0..4 {
            let cur = &mut self.game.fighters[f.0].hitboxes[k];
            if cur.state != hitcapsule::DISABLED
                && cur.group == group
                && hitcapsule::add_victim(cur, kind, v)
                && clear
            {
                self.game.ft.hit_logs.clank[k] = false;
            }
        }
    }

    /// Remembers a phantom hit's victim in each live box of the group.
    fn tip_group(&mut self, f: FighterId, i: usize, kind: i32, v: Victim) {
        let group = self.game.fighters[f.0].hitboxes[i].group;
        for k in 0..4 {
            let cur = &mut self.game.fighters[f.0].hitboxes[k];
            if cur.state != hitcapsule::DISABLED && cur.group == group {
                hitcapsule::add_victim2(cur, kind, v);
            }
        }
    }

    /// A clank's recoil for the fighter whose box `i` hit `other_x`: the strongest clank's
    /// damage, and on the ground its push and which way (`updateClankDamage`).
    fn clank_recoil(&mut self, f: FighterId, i: usize, dmg: i32, other_x: f32) {
        let c = self.common();
        let (mul, add) = (c.f32(0x3D0), c.f32(0x3D4));
        let fp = &mut self.game.fighters[f.0];
        if dmg > fp.dmg.int_value {
            fp.dmg.int_value = dmg;
            if fp.hitboxes[i].rebounds && fp.ground_or_air == GA_GROUND {
                fp.dmg.clank_damage = mem(fp::fmadds(fp::frsp(f64::from(dmg)), reg(mul), reg(add)));
                // Toward the other (the original compares either way round, which agrees
                // for NaN too).
                let toward = reg(fp.cur_pos.x) < reg(other_x);
                fp.dmg.facing_dir = if toward { 1.0 } else { -1.0 };
            }
        }
    }

    /// Two boxes clank, `ftColl_8007699C`: each box that isn't outdamaged by more than the
    /// clank range (`x3CC`) recoils, with a spark between them; true when this fighter's box
    /// `m` outdamages the other's `j` enough to stop it.
    #[doc(alias = "ftColl_8007699C")]
    fn clank(&mut self, v: FighterId, j: usize, me: FighterId, m: usize) -> bool {
        let range = self.common().i32(0x3CC);
        let (h0, h1) = (
            &self.game.fighters[v.0].hitboxes[j],
            &self.game.fighters[me.0].hitboxes[m],
        );
        let mid = |a: f32, b: f32| mem(fp::fmuls(0.5, fp::fadds(reg(a), reg(b))));
        let midpoint = Vec3::new(
            mid(h0.hurt_coll_pos.x, h1.hurt_coll_pos.x),
            mid(h0.hurt_coll_pos.y, h1.hurt_coll_pos.y),
            mid(h0.hurt_coll_pos.z, h1.hurt_coll_pos.z),
        );
        let (d0, d1) = (reg(h0.damage), reg(h1.damage));
        if fp::fctiwz(d1).wrapping_sub(range) < fp::fctiwz(d0) {
            let dmg = whole_damage(d1);
            self.hit_group(me, m, 3, victim(v), true);
            let x = self.game.fighters[v.0].cur_pos.x;
            self.clank_recoil(me, m, dmg, x);
            self.spawn_effect(1052, midpoint, None);
        }
        if fp::fctiwz(d0).wrapping_sub(range) < fp::fctiwz(d1) {
            let dmg = whole_damage(d0);
            self.hit_group(v, j, 3, victim(me), false);
            let x = self.game.fighters[me.0].cur_pos.x;
            self.clank_recoil(v, j, dmg, x);
            self.spawn_effect(1052, midpoint, None);
            self.clank_sound(me, v, j, m);
            return true;
        }
        false
    }

    /// An item's box `j` clanks with the fighter's box `m`, `ftColl_80077970`: each box the
    /// other doesn't outdamage by more than the clank range (`x3CC`) recoils, the fighter's as
    /// in a fighter's clank, the item's remembering the fighter and which way it was pushed
    /// (`recordItemClank`), each with a spark between them.
    #[doc(alias = "ftColl_80077970")]
    fn item_clank(&mut self, g: GObjId, j: usize, me: FighterId, m: usize) {
        use super::super::it;
        let range = self.common().i32(0x3CC);
        let (h1, h2) = (
            &it::item(self.game, g).hitboxes[j].hit,
            &self.game.fighters[me.0].hitboxes[m],
        );
        let mid = |a: f32, b: f32| mem(fp::fmuls(fp::fadds(reg(a), reg(b)), 0.5));
        let midpoint = Vec3::new(
            mid(h1.hurt_coll_pos.x, h2.hurt_coll_pos.x),
            mid(h1.hurt_coll_pos.y, h2.hurt_coll_pos.y),
            mid(h1.hurt_coll_pos.z, h2.hurt_coll_pos.z),
        );
        let (d1, d2) = (reg(h1.damage), reg(h2.damage));
        let item_victim = Victim::Item(it::item_of(self.game, g));
        if fp::fctiwz(d2).wrapping_sub(range) < fp::fctiwz(d1) {
            let dmg = whole_damage(d2);
            self.hit_group(me, m, 3, item_victim, true);
            let x = it::item(self.game, g).pos.x;
            self.clank_recoil(me, m, dmg, x);
            self.spawn_effect(1052, midpoint, None);
        }
        if fp::fctiwz(d1).wrapping_sub(range) < fp::fctiwz(d2) {
            let dmg = whole_damage(d1);
            let kind = if it::item(self.game, g).hitboxes[j].hit.fighter_hit_alt {
                4
            } else {
                3
            };
            self.item_add_victim(g, j, kind, victim(me), false);
            if dmg > it::item(self.game, g).clank_damage {
                let slow =
                    super::super::it::item::common_data(self.game).f32(self.data, IT_SLOW_FOR_ITEM);
                let (fg, fx) = (
                    self.game.fighters[me.0].gobj,
                    self.game.fighters[me.0].cur_pos.x,
                );
                let ip = it::item_mut(self.game, g);
                ip.met_fighter = fg;
                ip.clank_damage = dmg;
                let vx = reg(ip.vel.x);
                let mag = if vx < 0.0 { fp::fneg(vx) } else { vx };
                ip.out_damage_dir = if mag < reg(slow) {
                    if reg(ip.pos.x) > reg(fx) { -1.0 } else { 1.0 }
                } else if vx < 0.0 {
                    -1.0
                } else {
                    1.0
                };
            }
            self.spawn_effect(1052, midpoint, None);
        }
    }

    /// The fighter reflects item `g`'s box `j`, `ftColl_80077464`: the item remembers it;
    /// damage past the reflector's limit overpowers it (the item keeps the damage and which
    /// way, the reflector its excess); else the item turns the fighter's, taking its attack's
    /// record and the reflector's damage and speed factors. Either way the reflector notes
    /// which way the hit came from.
    #[doc(alias = "ftColl_80077464")]
    /// The fighter's absorb bubble takes item box `j`, `ftColl_8007925C`'s absorb branch: the
    /// item remembers the fighter, learns it was absorbed and which way, and an absorbable box
    /// adds its damage to the fighter's absorbed total.
    fn absorb_item(&mut self, g: GObjId, j: usize, me: FighterId) {
        use super::super::it;
        self.item_add_victim(g, j, 6, victim(me), false);
        let h = &it::item(self.game, g).hitboxes[j].hit;
        let (damage, absorbable) = (whole_damage(reg(h.damage)), h.absorbable);
        let fp = &self.game.fighters[me.0];
        let (fx, fg) = (fp.cur_pos.x, fp.gobj);
        let ip = it::item_mut(self.game, g);
        ip.absorber = fg;
        let dir = if reg(fx) > reg(ip.pos.x) { -1.0 } else { 1.0 };
        let fp = &mut self.game.fighters[me.0];
        fp.absorb.x1A40_absorbHitDirection = dir;
        if absorbable {
            fp.absorb.x1A44_damageTaken = fp.absorb.x1A44_damageTaken.wrapping_add(damage);
            fp.absorb.x1A48_hitsTaken = fp.absorb.x1A48_hitsTaken.wrapping_add(1);
        }
    }

    fn reflect_item(&mut self, g: GObjId, j: usize, me: FighterId) {
        use super::super::it;
        self.item_add_victim(g, j, 7, victim(me), false);
        let h = &it::item(self.game, g).hitboxes[j].hit;
        let damage = whole_damage(reg(h.damage));
        let second = h.fighter_hit_alt;
        let fp = &self.game.fighters[me.0];
        let (fx, fg) = (fp.cur_pos.x, fp.gobj);
        let over = damage > fp.reflect.x1A30_maxDamage;
        let (x2070, rec, dmg_mul, speed_mul) = (
            i32::from_be_bytes(fp.attack_flags),
            fp.attack_record.clone(),
            fp.reflect.x1A34_damageMul,
            fp.reflect.x1A38_speedMul,
        );
        let (b5, b4) = (fp.reflect_behavior, fp.reflect_kept);
        let ip = it::item_mut(self.game, g);
        if over {
            if second {
                if damage > ip.reflected_damage {
                    ip.reflected_damage = damage;
                }
            } else if damage > ip.damage_dealt {
                ip.damage_dealt = damage;
            }
        } else {
            ip.reflector = fg;
            ip.reflect_attack_flags = x2070;
            ip.reflect_record_a = rec.x2074_vec;
            ip.reflect_record_b = rec.x207C;
            ip.reflect_record_word = i32::from_be_bytes(rec.x10);
            ip.reflect_record_short = rec.x2088;
        }
        if !over {
            // The record's flags keep what the fighter's block held below the read bits.
            let i = it::item_of(self.game, g);
            crate::game::leftovers::carry_render(
                self.game,
                crate::game::leftovers::RenderOf::Fighter(me),
                0x2084,
                crate::game::leftovers::RenderOf::Item(i),
                0xC88,
                4,
            );
        }
        let ip = it::item_mut(self.game, g);
        ip.reflect_dir = if reg(fx) > reg(ip.pos.x) { -1.0 } else { 1.0 };
        if !over {
            ip.reflect_damage_mul = dmg_mul;
            ip.reflect_speed_mul = speed_mul;
            ip.state_flags.keeps_pokeball_reflected = b5;
            if b4 {
                ip.state_flags.keeps_owner_reflected = true;
            }
        }
        let (vx, ix) = (reg(ip.vel.x), ip.pos.x);
        let dir = if vx != 0.0 {
            if vx > 0.0 { -1.0 } else { 1.0 }
        } else if reg(ix) > reg(fx) {
            -1.0
        } else {
            1.0
        };
        let fp = &mut self.game.fighters[me.0];
        if over {
            fp.reflect.x1A3C_damageOver = damage;
        }
        fp.reflect.x1A2C_reflectHitDirection = dir;
    }

    /// The clank's sound, `ftColl_800784B4`: a random sword clash between two slashes.
    #[doc(alias = "ftColl_800784B4")]
    fn clank_sound(&mut self, me: FighterId, v: FighterId, j: usize, m: usize) {
        let e0 = self.game.fighters[v.0].hitboxes[j].element;
        let e1 = self.game.fighters[me.0].hitboxes[m].element;
        let id = if e0 == ELEMENT_SLASH && e1 == ELEMENT_SLASH {
            SLASH_CLANK_SFX[self.game.objs.rng.below(3) as usize]
        } else {
            0x6A
        };
        self.fighter_play_sfx(me, id, 0x7F, 0x40);
    }

    /// The other fighter's box `j` hits this fighter's shield, `ftColl_80076CBC`: the
    /// attacker's shield-hit push and the shield's damage, recoil and spark, or a
    /// powershield's flash.
    #[doc(alias = "ftColl_80076CBC")]
    fn shield_hit(&mut self, v: FighterId, j: usize, me: FighterId) {
        self.hit_group(v, j, 1, victim(me), false);
        let h0 = &self.game.fighters[v.0].hitboxes[j];
        let (dmg, element, x34, pos) = (
            whole_damage(reg(h0.damage)),
            h0.element,
            h0.shield_damage,
            h0.hurt_coll_pos,
        );
        let (my_x, light) = (
            self.game.fighters[me.0].cur_pos.x,
            self.game.fighters[me.0].lightshield_amount,
        );
        let (other_gobj, other_player, other_sub) = {
            let vf = &self.game.fighters[v.0];
            (vf.gobj, vf.player_idx, vf.is_sub_fighter)
        };
        let vf = &mut self.game.fighters[v.0];
        if dmg > vf.dmg.max_shield_damage {
            vf.dmg.max_shield_damage = dmg;
            if vf.ground_or_air == GA_GROUND {
                vf.dmg.shield_push = mem(fp::fmuls(reg(light), fp::frsp(f64::from(dmg))));
                vf.dmg.shield_push_dir = if reg(my_x) > reg(vf.cur_pos.x) {
                    1.0
                } else {
                    -1.0
                };
            }
        }
        let vx = vf.cur_pos.x;
        let mf = &mut self.game.fighters[me.0];
        if dmg > mf.shield_hit_damage {
            mf.shield_hit_damage = dmg;
            mf.specialn_facing_dir = if reg(mf.cur_pos.x) > reg(vx) {
                -1.0
            } else {
                1.0
            };
            mf.shield_hit_element = element as i32;
            mf.shield_hit_by = other_gobj;
        }
        if mf.powershield {
            // A powershield: no damage, the release window, a flash and its sound.
            let (gobj, slot) = (
                mf.gobj.expect("a fighter without a GObj"),
                i32::from(mf.player_idx),
            );
            self.guard_release_window(gobj);
            self.start_color_anim(gobj, 118, 0);
            self.fighter_play_sfx(me, 104, 0x7F, 0x40);
            self.record_powershield(slot);
            self.spawn_effect(27, pos, None);
        } else {
            let total = dmg.wrapping_add(x34);
            mf.shield_damage_taken = mf.shield_damage_taken.wrapping_add(total.max(0));
            mf.shield_hit_slot = i32::from(other_player);
            mf.shield_hit_by_sub = other_sub;
            self.spawn_effect(1052, pos, None);
        }
        let push = self.game.fighters[me.0].counter_min_hitlag;
        if push != 0.0 {
            self.game.fighters[v.0].min_hitlag = push;
            self.game.fighters[me.0].min_hitlag = push;
        }
    }

    /// A hit's damage to this fighter (`inlineB3`): less for one who holds someone else, more
    /// when frozen, times its damage multiplier.
    fn scaled_damage(&self, v: FighterId, j: usize, me: FighterId) -> f64 {
        let c = self.common();
        let (vf, mf) = (&self.game.fighters[v.0], &self.game.fighters[me.0]);
        let mut d = reg(vf.hitboxes[j].damage);
        if mf.victim_gobj.is_some() && !mf.state_flags.x221B_b5 && mf.victim_gobj != vf.gobj {
            d = fp::fmuls(d, reg(c.f32(0x128)));
        }
        if mf.motion_id == MS_DAMAGE_ICE {
            d = fp::fmuls(d, reg(c.f32(0x714)));
        }
        fp::fmuls(d, reg(mf.dmg.damage_scale))
    }

    /// The fighter whose attack a fighter's box is: its thrower's for a thrown fighter.
    fn attacker(&self, f: FighterId) -> FighterId {
        match self.thrower(f) {
            Some(g) => self.game.fighter_id(g),
            None => f,
        }
    }

    /// The other fighter's box `j` hits this fighter's hurtbox `n`, `ftColl_80076ED8`. A box
    /// that barely reaches (closer than `x7A8` overlap) lands a tip, a phantom hit for half,
    /// unless the fighter already took a hit this frame or is in hitlag; any other logs the
    /// hit, unless the fighter is invincible there, and records it. Returns whether the box
    /// counts as having hit.
    #[doc(alias = "ftColl_80078488")]
    #[doc(alias = "ftColl_80076ED8")]
    fn take_hit(&mut self, v: FighterId, j: usize, me: FighterId, n: usize) -> bool {
        let mut dmg = self.scaled_damage(v, j, me);
        let tip = self.common().f32(0x7A8);
        let h0 = &self.game.fighters[v.0].hitboxes[j];
        if reg(h0.coll_distance) < reg(tip) {
            let mf = &self.game.fighters[me.0];
            if !self.game.ft.hit_logs.hits.is_empty() || mf.dmg.hit_frames != 0.0 {
                return false;
            }
            if hitcapsule::has_victim2(h0, victim(me)) {
                return false;
            }
            let mut half = fp::fmuls(0.5, dmg);
            if fp::fctiwz(half) == 0 && dmg != 0.0 {
                half = 1.0;
            }
            self.tip_group(v, j, 0, victim(me));
            let mf = &self.game.fighters[me.0];
            if mf.hit_status == 0
                && mf.timed_hit_status == 0
                && !mf.starman
                && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
            {
                let h0 = &self.game.fighters[v.0].hitboxes[j];
                let count = h0.whole_damage;
                let mut len = count >> 1;
                if len == 0 && count != 0 {
                    len = 1;
                }
                let whole = fp::fctiwz(half);
                let pos = h0.hurt_coll_pos;
                let mf = &mut self.game.fighters[me.0];
                if whole > mf.dmg.max_tip_damage {
                    mf.dmg.max_tip_damage = whole;
                }
                let a = self.attacker(v);
                let af = &self.game.fighters[a.0];
                let entry = LogEntry {
                    kind: af.kind,
                    gobj: af.gobj,
                    hit: LoggedHit::Fighter {
                        fighter: v,
                        index: j,
                    },
                    hurt: n,
                    pos,
                    damage: mem(half),
                    count: len,
                };
                let tips = &mut self.game.ft.hit_logs.tips;
                assert!(tips.len() < LOG_LEN, "tip log over {LOG_LEN}!!");
                tips.push(entry);
            }
            self.fighter_play_sfx(me, 85, 0x7F, 0x40);
            return true;
        }
        let whole = whole_damage(dmg);
        self.game.ft.hit_logs.tips.clear();
        let mut logged = false;
        let group = self.game.fighters[v.0].hitboxes[j].group;
        for k in 0..4 {
            let cur = &mut self.game.fighters[v.0].hitboxes[k];
            if cur.state != hitcapsule::DISABLED && cur.group == group {
                hitcapsule::add_victim(cur, 0, victim(me));
            }
        }
        let vf = &mut self.game.fighters[v.0];
        if whole > vf.dmg.item_hit_damage {
            vf.dmg.item_hit_damage = whole;
        }
        let mf = &self.game.fighters[me.0];
        if mf.hit_status == 0
            && mf.timed_hit_status == 0
            && !mf.starman
            && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
        {
            let count = whole_damage(dmg);
            let mf = &mut self.game.fighters[me.0];
            if mf.armor {
                mf.dmg.armor_hp = mem(fp::fsubs(reg(mf.dmg.armor_hp), dmg));
                if mf.dmg.armor_hp < 0.0 {
                    dmg = fp::fneg(reg(mf.dmg.armor_hp));
                    mf.armor = false;
                }
            }
            if !mf.armor {
                assert!(!(dmg > 500.0), "attack power over 500!! {dmg}");
                mf.dmg.pending_damage = mem(fp::fadds(reg(mf.dmg.pending_damage), dmg));
                if count > mf.dmg.damage_applied {
                    mf.dmg.damage_applied = count;
                }
                let a = self.attacker(v);
                let af = &self.game.fighters[a.0];
                let h0 = &self.game.fighters[v.0].hitboxes[j];
                let entry = LogEntry {
                    kind: af.kind,
                    gobj: af.gobj,
                    hit: LoggedHit::Fighter {
                        fighter: v,
                        index: j,
                    },
                    hurt: n,
                    pos: h0.hurt_coll_pos,
                    damage: h0.damage,
                    count: h0.whole_damage,
                };
                let hits = &mut self.game.ft.hit_logs.hits;
                assert!(hits.len() < LOG_LEN, "damage log over {LOG_LEN}!!");
                hits.push(entry);
                logged = true;
            }
            let (ag, vg) = (
                self.game.fighters[v.0]
                    .gobj
                    .expect("a fighter without its GObj"),
                self.game.fighters[me.0]
                    .gobj
                    .expect("a fighter without its GObj"),
            );
            self.record_fighter_hit(ag, vg, dmg);
        }
        if !logged {
            let pos = self.game.fighters[v.0].hitboxes[j].hurt_coll_pos;
            self.spawn_effect(1052, pos, None);
        }
        true
    }

    /// The hit's sound: the plain hit sound by severity when the hurtbox is invincible
    /// (`ftColl_80078384`'s check), else the hitbox's own by kind and severity.
    fn hit_sound(&mut self, v: FighterId, j: usize, me: FighterId, n: usize) {
        let h0 = &self.game.fighters[v.0].hitboxes[j];
        let (severity, kind) = (h0.sfx_severity, h0.sfx_kind);
        self.hit_sound_of(me, n, kind, severity);
    }

    /// A hit's sound on hurtbox `n` for a hitbox's sound kind and severity, `ftColl_80078384`.
    #[doc(alias = "ftColl_80078384")]
    fn hit_sound_of(&mut self, me: FighterId, n: usize, kind: i32, severity: i32) {
        let mf = &self.game.fighters[me.0];
        if mf.hit_status != 0
            || mf.timed_hit_status != 0
            || mf.starman
            || mf.hurt_capsules[n].capsule.state != HURT_ENABLED
        {
            self.fighter_play_sfx(me, HIT_SFX[severity as usize], 0x7F, 0x40);
            return;
        }
        let slot = i32::from(mf.player_idx) * 2 + i32::from(mf.is_sub_fighter);
        if kind == 13 && severity == 2 {
            let h = self.hit_kind_sound(kind, severity, 0x72 + slot);
            self.game.fighters[me.0].track_handle = h;
        } else {
            let h = self.hit_kind_sound(kind, severity, 0x7E + slot);
            self.game.fighters[me.0].sfx_handle = h;
        }
    }

    /// Plays a hitbox's sound by its kind and severity, `lbColl_80005BB0`: on `track` for the
    /// heaviest of kind 13, else on none.
    #[doc(alias = "lbColl_80005BB0")]
    pub(crate) fn hit_kind_sound(&mut self, kind: i32, severity: i32, track: i32) -> i32 {
        let id = HIT_KIND_SFX[(kind * 3 + severity) as usize];
        let track = if kind == 13 && severity == 2 {
            track
        } else {
            -1
        };
        self.sfx_play_tracked(id, 127, 64, track)
    }

    /// What a fighter's hit on another records, `ftColl_8007891C`: the attacker's stale-move
    /// queue, its combo against the victim, and the damage both players' statistics count.
    #[doc(alias = "ftColl_8007891C")]
    pub(crate) fn record_fighter_hit(&mut self, attacker: GObjId, victim_gobj: GObjId, dmg: f64) {
        self.stale_move_from_fighter(attacker, victim_gobj);
        let a = self.game.fighter_id(attacker);
        let attack_id = self.game.fighters[a.0].attack_id;
        self.count_combo(attacker, victim_gobj, attack_id);
        let v = self.game.fighter_id(victim_gobj);
        let (af, vf) = (&self.game.fighters[a.0], &self.game.fighters[v.0]);
        let kind = i32::from(af.attack_flags[3]);
        let (ap, asub, vp, vsub) = (
            i32::from(af.player_idx),
            i32::from(af.is_sub_fighter),
            i32::from(vf.player_idx),
            i32::from(vf.is_sub_fighter),
        );
        self.count_damage(dmg, ap, asub, vp, vsub, kind);
    }

    /// Queues the attacker's move as stale in its player's table unless this instance of it
    /// is already there, `plStale_UpdateStaleMovesFromFighter`.
    #[doc(alias = "plStale_UpdateStaleMovesFromFighter")]
    fn stale_move_from_fighter(&mut self, attacker: GObjId, victim_gobj: GObjId) {
        if attacker == victim_gobj {
            return;
        }
        let fp = self.game.fighter(attacker);
        let (instance, attack_id, slot) =
            (fp.attack_instance, fp.attack_id, i32::from(fp.player_idx));
        if attack_id == 1 {
            return;
        }
        let table = &mut self.game.player_mut(slot).stale_moves;
        if table
            .StaleMoves
            .iter()
            .any(|m| attack_id == i32::from(m.move_id) && instance == m.attack_instance)
        {
            return;
        }
        let at = table.current_index as usize;
        table.StaleMoves[at].move_id = attack_id as u16;
        table.StaleMoves[at].attack_instance = instance;
        table.current_index = if table.current_index == 9 {
            0
        } else {
            table.current_index + 1
        };
    }

    /// Counts a repeated move on the same victim, `ftColl_800763C0` (through
    /// `ftColl_80076444`): the same move again past `x4C4` hits sets the stale timer
    /// (`x2092`).
    #[doc(alias = "ftColl_800763C0")]
    #[doc(alias = "ftColl_80076444")]
    fn count_combo(&mut self, attacker: GObjId, victim_gobj: GObjId, attack_id: i32) {
        if attacker == victim_gobj {
            return;
        }
        let c = self.common();
        let (limit, timer) = (c.i32(0x4C4), c.u32(0x4D8));
        let fp = self.game.fighter_mut(attacker);
        if fp.combo_victim.is_none() {
            fp.combo_attack_id = attack_id;
            fp.combo_count = 1;
            fp.combo_victim = Some(victim_gobj);
        } else if fp.combo_victim == Some(victim_gobj) {
            if attack_id != 1 && fp.combo_attack_id == attack_id {
                fp.combo_count = fp.combo_count.wrapping_add(1);
                if i32::from(fp.combo_count) >= limit {
                    fp.combo_push_frames = timer as u16;
                }
            } else {
                fp.combo_count = 0;
                fp.combo_attack_id = attack_id;
            }
        }
    }

    /// The damage a hit counts in the players' statistics, `pl_8003EB30`.
    #[doc(alias = "pl_8003EB30")]
    pub(crate) fn count_damage(
        &mut self,
        dmg: f64,
        slot: i32,
        sub: i32,
        vslot: i32,
        vsub: i32,
        kind: i32,
    ) {
        if slot == 6 || slot == vslot {
            return;
        }
        let t = &mut self.game.player_mut(slot).stale_moves;
        t.xC6C = mem(fp::fadds(reg(t.xC6C), dmg));
        if kind == 52 {
            t.xC70 = mem(fp::fadds(reg(t.xC70), dmg));
        }
        if (kind.wrapping_sub(92) as u32) <= 1 {
            t.xC74 = mem(fp::fadds(reg(t.xC74), dmg));
        }
        if vslot != 6 {
            if vsub == 0 {
                let v = &mut self.game.player_mut(vslot).stale_moves.xC78[slot as usize];
                *v = mem(fp::fadds(reg(*v), dmg));
            }
            self.count_damage_dealt(slot, sub, vslot, vsub, kind, dmg);
        }
        // `pl_StaleMoveTableExt_t::xD08`, past the stale-move table.
        self.game.player_mut(slot).stats_ext[0x14..0x18].copy_from_slice(&[0; 4]);
    }

    /// Counts a hit between two players, `pl_80040ED4`: unless the victim is a sub-fighter,
    /// the attacker's hits (but for a few kinds), the players it has hit, and the damage each
    /// has dealt the other.
    #[doc(alias = "pl_80040ED4")]
    fn count_damage_dealt(
        &mut self,
        slot: i32,
        _sub: i32,
        vslot: i32,
        vsub: i32,
        kind: i32,
        dmg: f64,
    ) {
        if vsub == 1 {
            return;
        }
        let counts = !matches!(kind, 76 | 90 | 91 | 92 | 96);
        if counts {
            let first = self.game.player(vslot).stale_moves.x5EC.x10[slot as usize].x4 == 0;
            let t = &mut self.game.player_mut(slot).stale_moves.x5EC;
            t.x8 = t.x8.wrapping_add(1);
            if first {
                t.xC = t.xC.wrapping_add(1);
            }
            let e = &mut self.game.player_mut(vslot).stale_moves.x5EC.x10[slot as usize];
            e.x4 = e.x4.wrapping_add(1);
        }
        let t = &mut self.game.player_mut(slot).stale_moves.x5EC;
        t.x0 = vslot as u8;
        t.x4 = mem(dmg);
        let e = &mut self.game.player_mut(vslot).stale_moves.x5EC.x10[slot as usize];
        e.x0 = mem(fp::fadds(reg(e.x0), dmg));
    }

    /// A coin match's coin pickups, `ft_8007C77C`: on alternate frames by its player's port,
    /// unless it's intangible, the fighter picks up each coin it may whose pickup circle meets
    /// one of its own two (`x1614`), adding its worth to its player's coins with a sound and
    /// sparkle.
    #[doc(alias = "ft_8007C77C")]
    pub fn coin_pickups(&mut self, gobj: GObjId) {
        use super::super::it::{self, kinds};
        use crate::lb::collision::{SweptCircle, swept_circles_touch};
        if !self.game.vs_is_coins() {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let odd = self.game.scene_game_ticks() & 1;
        let fp = &mut self.game.fighters[f.0];
        fp.coin_odd_frame = odd != u32::from(fp.player_idx) & 1;
        if fp.coin_odd_frame || self.hit_status(f) == 2 {
            return;
        }
        let mut cur = self.game.world.first_in(it::P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if it::item(self.game, g).kind != ItKind::VsCoin {
                continue;
            }
            let owner = it::item(self.game, g).owner;
            let flags = kinds::coin::vars(self.game, g).flags.get();
            let may = if self.is_same_player(Some(gobj), owner) {
                flags.owner
            } else {
                flags.others
            };
            if !may {
                continue;
            }
            let scl = it::item(self.game, g).scl;
            for i in 0..2 {
                let sub = &mut kinds::coin::vars(self.game, g).sub;
                let mut coin = SweptCircle {
                    radius: sub.radius.get(),
                    pos: sub.pos.get(),
                    prev_pos: sub.prev_pos.get(),
                    contact: Default::default(),
                };
                let fp = &self.game.fighters[f.0];
                let e = &fp.stamina_markers[i];
                let mut mine = SweptCircle {
                    radius: e.x0,
                    pos: e.x8,
                    prev_pos: e.x14,
                    contact: e.x20,
                };
                let scale = fp.scale.y;
                let touch = swept_circles_touch(&mut coin, &mut mine, scl, scale);
                self.game.fighters[f.0].stamina_markers[i].x20 = mine.contact;
                if touch.is_some() {
                    // Past the coin's variables (`coin::CoinVars`).
                    let v = it::item_mut(self.game, g).vars.coin();
                    v.contact.set(coin.contact);
                }
                if touch == Some(true) {
                    let worth = kinds::coin::worth(self, g);
                    let slot = i32::from(self.game.fighters[f.0].player_idx);
                    let n = worth.wrapping_add(self.game.player_coins(slot));
                    self.game.set_player_coins(slot, n);
                    let n = worth.wrapping_add(self.game.player_total_coins(slot));
                    self.game.set_player_total_coins(slot, n);
                    self.sfx_play_track(147, 127, 64, 26);
                    // The sparkle at the coin's contact point, which the test just wrote.
                    crate::game::ef::spawn(
                        self,
                        0x432,
                        None,
                        &[
                            crate::game::ef::Arg::Vec(coin.contact),
                            crate::game::ef::Arg::Float(1.0),
                        ],
                    );
                    it::item::destroy(self, g);
                    break;
                }
            }
        }
    }

    /// The items' hitboxes against this fighter, `ftColl_8007925C`: each item not on its side
    /// (team rules aside) whose hitbox can reach it and hasn't already hit it, against its
    /// reflector and absorber, its own hitboxes (a clank), its shield, then its hurtboxes,
    /// where the first one hit takes the hit and its sound. Reflecting, absorbing, clanking,
    /// shields and inert boxes stop at their seams.
    #[doc(alias = "ftColl_8007925C")]
    pub fn hits_from_items(&mut self, gobj: GObjId) {
        use super::super::it::{self, P_LINK_ITEM};
        let me = self.game.fighter_id(gobj);
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            let ip = it::item(self.game, g);
            if ip.kind == ItKind::VsCoin {
                continue;
            }
            let (owner, b5, b6, team) = (
                ip.owner,
                ip.hit_flags.bits.hits_own_player,
                ip.hit_flags.bits.hits_own_team,
                ip.team,
            );
            if self.is_same_player(Some(gobj), owner) && !b5 {
                continue;
            }
            let teams = self.game.vs_is_teams() && !self.game.vs_friendly_fire() && !b6;
            if teams && team == self.game.fighters[me.0].team {
                continue;
            }
            // The fighter's own boxes that could clank with the item's (`ftColl_804D6560`),
            // unless a thrower holds the fighter.
            let mut clanks = 0;
            if !self.game.fighters[me.0].state_flags.x221B_b5 && self.thrower(me).is_none() {
                let item_ga = it::item(self.game, g).ground_or_air;
                for i in 0..4 {
                    let h = &self.game.fighters[me.0].hitboxes[i];
                    let can = h.state != hitcapsule::DISABLED
                        && !h.phantom
                        && h.element != ELEMENT_CATCH
                        && h.hits_fighters
                        && ((h.hits_air && item_ga == GA_AIR)
                            || (h.hits_ground && item_ga == GA_GROUND))
                        && !hitcapsule::has_victim(h, Victim::Item(it::item_of(self.game, g)));
                    self.game.ft.hit_logs.clank[i] = can;
                    clanks += i32::from(can);
                }
            }
            for j in 0..4 {
                self.item_box_against(gobj, me, g, j, clanks);
            }
        }
    }

    /// One of an item's hitboxes against the fighter, `ftColl_8007925C`'s inner loop.
    fn item_box_against(&mut self, gobj: GObjId, me: FighterId, g: GObjId, j: usize, clanks: i32) {
        use super::super::it;
        let ip = it::item(self.game, g);
        let ib = &ip.hitboxes[j];
        let h = &ib.hit;
        let mf = &self.game.fighters[me.0];
        if h.state == hitcapsule::DISABLED || !h.hits_fighters {
            return;
        }
        if !((h.hits_air && mf.ground_or_air == GA_AIR)
            || (h.hits_ground && mf.ground_or_air == GA_GROUND))
        {
            return;
        }
        if h.misses_same_facing && reg(mf.facing_dir) == reg(ip.facing_dir) {
            return;
        }
        if self.game.vs_no_fighter_hits() && !ib.always_hits {
            return;
        }
        if hitcapsule::has_victim(h, victim(me)) {
            return;
        }
        let phantom = h.phantom;
        let element = h.element;
        if element != ELEMENT_INERT && !phantom && h.meets_reflector {
            let mf = &self.game.fighters[me.0];
            if mf.reflecting
                && it::item(self.game, g).hitboxes[j].hit.reflectable
                && self.item_box_hits_bubble(g, j, me, Bubble::Reflect, false)
            {
                self.reflect_item(g, j, me);
                return;
            }
            let mf = &self.game.fighters[me.0];
            let h = &it::item(self.game, g).hitboxes[j].hit;
            if mf.absorbing
                && (h.absorbable || (mf.absorb_any && h.reflectable))
                && self.item_box_hits_bubble(g, j, me, Bubble::Absorb, false)
            {
                self.absorb_item(g, j, me);
                return;
            }
        }
        if !self.game.fighters[me.0].state_flags.x221B_b5 && !phantom && clanks != 0 {
            for m in 0..4 {
                if !self.game.ft.hit_logs.clank[m] {
                    continue;
                }
                let theirs = &self.game.fighters[me.0].hitboxes[m];
                let h = &it::item(self.game, g).hitboxes[j].hit;
                if element == ELEMENT_INERT || theirs.element == ELEMENT_INERT {
                    if element == theirs.element {
                        continue;
                    }
                    if self.item_box_touches(g, j, me, m) {
                        let fg = self.game.fighters[me.0].gobj;
                        let ip = it::item_mut(self.game, g);
                        it::set_flag(&mut ip.hurt_flags, 6, true);
                        ip.toucher = fg;
                        return;
                    }
                } else {
                    if !h.clanks || !theirs.clanks {
                        continue;
                    }
                    if self.item_box_touches(g, j, me, m) {
                        self.item_clank(g, j, me, m);
                        return;
                    }
                }
            }
        }
        let mf = &self.game.fighters[me.0];
        let h = &it::item(self.game, g).hitboxes[j].hit;
        if element != ELEMENT_INERT && mf.state_flags.x221B_b0 && h.meets_bubble {
            let ix = it::item(self.game, g).pos.x;
            let mut facing = true;
            if mf.state_flags.x221B_b3 {
                if -1.0 == reg(mf.facing_dir) {
                    if reg(mf.cur_pos.x) < reg(ix) {
                        facing = false;
                    }
                } else if reg(mf.cur_pos.x) > reg(ix) {
                    facing = false;
                }
            }
            if mf.state_flags.x221B_b4 && !h.meets_reflector {
                facing = false;
            }
            if facing && self.item_box_hits_bubble(g, j, me, Bubble::Shield, phantom) {
                // Where the box entered the shield, wanted only by a box that bounces off it
                // or a shield that deflects; the original leaves its stack slots unset
                // otherwise, and `item_shield_hit` reads them only in those cases.
                let mf = &self.game.fighters[me.0];
                let bounce = it::item(self.game, g).hitboxes[j].hit.bounces_off_shield;
                let entry = if !mf.state_flags.x221B_b2 && (bounce || mf.state_flags.x221B_b1) {
                    Some(self.item_box_shield_entry(g, j, me))
                } else {
                    None
                };
                self.item_shield_hit(g, j, me, entry);
                return;
            }
        }
        if element == ELEMENT_INERT {
            // An inert box only notes that it touched the fighter (`xDCE` bit 6, `toucher`).
            let mtx = self.own_matrix(me);
            for m in 0..usize::from(self.game.fighters[me.0].hurt_capsules_len) {
                let ip = it::item(self.game, g);
                let mut hit = ip.hitboxes[j].hit.clone();
                let hit_scale = ip.scl;
                let fp = &mut self.game.fighters[me.0];
                let (scale, z) = (fp.scale.y, fp.cur_pos.z);
                let touch = hitcapsule::touches_hurtbox(
                    &mut self.game.objs,
                    self.data,
                    &mut hit,
                    &mut fp.hurt_capsules[m].capsule,
                    mtx.as_ref(),
                    hit_scale,
                    scale,
                    z,
                );
                let ip = it::item_mut(self.game, g);
                let h = &mut ip.hitboxes[j].hit;
                h.hurt_coll_pos = hit.hurt_coll_pos;
                h.coll_distance = hit.coll_distance;
                if touch {
                    it::set_flag(&mut ip.hurt_flags, 6, true);
                    ip.toucher = Some(gobj);
                    break;
                }
            }
            return;
        }
        let mf = &self.game.fighters[me.0];
        if mf.hit_status == 2 || mf.timed_hit_status == 2 {
            return;
        }
        let grab_only = it::item(self.game, g).hitboxes[j]
            .hit
            .grabbable_hurtboxes_only;
        let mut n = 0;
        while n < usize::from(self.game.fighters[me.0].hurt_capsules_len) {
            if grab_only && self.game.fighters[me.0].hurt_capsules[n].is_grabbable == 0 {
                n += 1;
                continue;
            }
            if self.item_box_hits_hurtbox(g, j, me, n, phantom) {
                if self.take_item_hit(g, j, me, n) {
                    self.item_hit_sound(g, j, me, n);
                }
                return;
            }
            n += 1;
        }
        let _ = gobj;
    }

    /// The hitbox a logged hit names.
    fn logged_capsule(&self, hit: LoggedHit) -> &super::super::structs::HitCapsule {
        match hit {
            LoggedHit::Fighter { fighter, index } => &self.game.fighters[fighter.0].hitboxes[index],
            LoggedHit::Throw { fighter, index } => {
                &self.game.fighters[fighter.0].throw_hitboxes[index]
            }
            LoggedHit::Item { item, index } => {
                &super::super::it::item(self.game, item).hitboxes[index].hit
            }
            LoggedHit::Device { .. } => unreachable!("a device's hit has no hitbox"),
        }
    }

    /// The knockback a stage device's hit deals this fighter (`ftColl_8007A06C`'s device
    /// case): its description copied to a hitbox (`lbColl_80008D30`), at an attack ratio of 1.
    fn device_knockback(&self, me: FighterId, desc: melee_data::archive::DataRef) -> f64 {
        let h = device_capsule(self.data, desc);
        let fp = &self.game.fighters[me.0];
        let defense = self.game.player_defense_ratio(i32::from(fp.player_idx));
        let ratio = self.game.vs_damage_ratio();
        self.knockback_of(
            me,
            &h,
            h.whole_damage,
            ratio,
            1.0,
            defense,
            fp.co_attrs.weight,
        )
    }

    /// The knockback an item's hitbox deals this fighter (`ftColl_8007A06C`'s item case): the
    /// owner's attack ratio when a fighter owns it.
    fn item_knockback(&self, me: FighterId, item: GObjId, hit: LoggedHit, count: u32) -> f64 {
        let owner = super::super::it::item(self.game, item).owner;
        let attack = if super::super::it::item::is_fighter(self.game, owner) {
            let o = self.game.fighter(owner.expect("an owner"));
            self.game.player_attack_ratio(i32::from(o.player_idx))
        } else {
            1.0
        };
        let fp = &self.game.fighters[me.0];
        let defense = self.game.player_defense_ratio(i32::from(fp.player_idx));
        let ratio = self.game.vs_damage_ratio();
        self.knockback(me, hit, count, ratio, attack, defense, fp.co_attrs.weight)
    }

    /// Whether the item's box `j` touches the fighter's box `m` (`lbColl_80007AFC`), each
    /// keeping where it met the other.
    fn item_box_touches(&mut self, g: GObjId, j: usize, me: FighterId, m: usize) -> bool {
        let ip = super::super::it::item(self.game, g);
        let mut a = hitcapsule::shape(&ip.hitboxes[j].hit);
        let mut b = hitcapsule::shape(&self.game.fighters[me.0].hitboxes[m]);
        let (sa, sb) = (ip.scl, self.game.fighters[me.0].scale.y);
        let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
        super::super::it::item_mut(self.game, g).hitboxes[j]
            .hit
            .hurt_coll_pos = a.contact;
        self.game.fighters[me.0].hitboxes[m].hurt_coll_pos = b.contact;
        touch
    }

    /// Whether the item's box `j` touches one of the fighter's bubbles (`lbColl_80007BCC`).
    fn item_box_hits_bubble(
        &mut self,
        g: GObjId,
        j: usize,
        me: FighterId,
        which: Bubble,
        phantom: bool,
    ) -> bool {
        let mtx = self.own_matrix(me);
        let ip = super::super::it::item(self.game, g);
        let mut hit = ip.hitboxes[j].hit.clone();
        let hit_scale = ip.scl;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.scale.y, fp.cur_pos.z);
        let bubble = match which {
            Bubble::Shield => &mut fp.shield_hit,
            Bubble::Reflect => &mut fp.reflect_hit,
            Bubble::Absorb => &mut fp.absorb_hit,
        };
        let touch = hitcapsule::hits_bubble(
            &mut self.game.objs,
            self.data,
            &mut hit,
            bubble,
            mtx.as_ref(),
            phantom,
            hit_scale,
            scale,
            z,
        );
        let h = &mut super::super::it::item_mut(self.game, g).hitboxes[j].hit;
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// Where the item's box `j` entered the fighter's shield (`lbColl_80007DD8`).
    fn item_box_shield_entry(
        &mut self,
        g: GObjId,
        j: usize,
        me: FighterId,
    ) -> crate::lb::collision::SphereEntry {
        let mtx = self.own_matrix(me);
        let ip = super::super::it::item(self.game, g);
        let hit = ip.hitboxes[j].hit.clone();
        let scale = ip.scl;
        let bubble = self.game.fighters[me.0].shield_hit.clone();
        hitcapsule::bubble_entry(
            &mut self.game.objs,
            self.data,
            &hit,
            &bubble,
            mtx.as_ref(),
            scale,
        )
    }

    /// Item `g`'s box `j` hits this fighter's shield, `ftColl_80077688`: the item remembers
    /// the fighter (as a reflected or blocked hit), and a hit past the most damage it dealt
    /// becomes that, with which way it came, whether it bounces off and, if so, its rebound:
    /// along the shield's normal where it entered, or the common data's angle when the shield
    /// deflects. The shield takes the damage, notes who dealt it and the strongest hit's
    /// direction, element and source, and the hit's spark plays where it touched. `entry` is
    /// where the box entered the shield, which the caller works out only when the rebound
    /// needs it.
    #[doc(alias = "ftColl_80077688")]
    fn item_shield_hit(
        &mut self,
        g: GObjId,
        j: usize,
        me: FighterId,
        entry: Option<crate::lb::collision::SphereEntry>,
    ) {
        use super::super::it;
        let mode = if it::item(self.game, g).hitboxes[j].hit.shield_hit_alt {
            2
        } else {
            1
        };
        self.item_add_victim(g, j, mode, victim(me), false);
        let h = &it::item(self.game, g).hitboxes[j].hit;
        let (dmg, x34, element, x42_b3, x42_b4, pos) = (
            whole_damage(reg(h.damage)),
            h.shield_damage,
            h.element,
            h.bounces_off_shield,
            h.meets_reflector,
            h.hurt_coll_pos,
        );
        if dmg > it::item(self.game, g).damage_dealt {
            let slow = reg(it::item::common_data(self.game).f32(self.data, IT_SLOW_FOR_ITEM));
            let angle = self.common().f32(0x2D0);
            let mf = &self.game.fighters[me.0];
            let (deflects, deflect_off, mx, mg, unk1) = (
                mf.state_flags.x221B_b1,
                mf.state_flags.x221B_b2,
                mf.cur_pos.x,
                mf.gobj,
                mf.counter_item_min_hitlag,
            );
            let ip = it::item_mut(self.game, g);
            ip.shield_damage = dmg;
            it::set_flag(&mut ip.hurt_flags, 5, !deflect_off && x42_b3);
            ip.met_fighter = mg;
            let vx = reg(ip.vel.x);
            let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
            ip.out_damage_dir = if speed < slow {
                if reg(ip.pos.x) > reg(mx) { -1.0 } else { 1.0 }
            } else if vx < 0.0 {
                -1.0
            } else {
                1.0
            };
            if it::flag(ip.hurt_flags, 5) {
                let entry = entry.expect("a bouncing item without where it entered the shield");
                if deflects {
                    let a = mem(fp::fmuls(0.017_453_292_384_743_69, reg(angle)));
                    ip.shield_angle = 0.0;
                    let c = reg(crate::math::cos(a));
                    let s = crate::math::sin(a);
                    let x = if entry.normal.x >= 0.0 {
                        c
                    } else {
                        fp::fneg(c)
                    };
                    ip.shield_normal = Vec3::new(mem(x), s, 0.0);
                    it::set_flag(&mut ip.hurt_flags, 4, true);
                } else {
                    ip.shield_angle = entry.angle;
                    ip.shield_normal = entry.normal;
                }
            }
            if unk1 != 0.0 {
                ip.min_hitlag = unk1;
            }
        }
        let owner = it::item(self.game, g).owner;
        let owner_fighter = if it::item::is_fighter(self.game, owner) {
            let of = self.game.fighter(owner.expect("a fighter owner"));
            Some((of.player_idx, of.is_sub_fighter))
        } else {
            None
        };
        let (ix, entity) = {
            let ip = it::item(self.game, g);
            (ip.pos.x, ip.entity)
        };
        let mf = &mut self.game.fighters[me.0];
        mf.shield_damage_taken = mf
            .shield_damage_taken
            .wrapping_add(dmg.wrapping_add(x34).max(0));
        match owner_fighter {
            Some((player, sub)) => {
                mf.shield_hit_slot = i32::from(player);
                mf.shield_hit_by_sub = sub;
            }
            None => mf.shield_hit_slot = 6,
        }
        if dmg > mf.shield_hit_damage {
            mf.shield_hit_damage = dmg;
            mf.specialn_facing_dir = if reg(mf.cur_pos.x) > reg(ix) {
                -1.0
            } else {
                1.0
            };
            mf.shield_hit_element = element as i32;
            mf.shield_hit_by = entity;
        }
        if mf.counter_item_min_hitlag != 0.0 && x42_b4 {
            mf.min_hitlag = mf.counter_item_min_hitlag;
        }
        self.spawn_effect(0x41C, pos, None);
    }

    /// Whether the item's box `j` touches the fighter's hurtbox `n` (`lbColl_8000805C`).
    fn item_box_hits_hurtbox(
        &mut self,
        g: GObjId,
        j: usize,
        me: FighterId,
        n: usize,
        phantom: bool,
    ) -> bool {
        let mtx = self.own_matrix(me);
        let ip = super::super::it::item(self.game, g);
        let mut hit = ip.hitboxes[j].hit.clone();
        let hit_scale = ip.scl;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.scale.y, fp.cur_pos.z);
        let touch = hitcapsule::hits_hurtbox(
            &mut self.game.objs,
            self.data,
            &mut hit,
            &mut fp.hurt_capsules[n].capsule,
            mtx.as_ref(),
            phantom,
            hit_scale,
            scale,
            z,
        );
        let h = &mut super::super::it::item_mut(self.game, g).hitboxes[j].hit;
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// An item's hitbox damage, `it_8026B1D4`: its damage, plus for an item whose speed
    /// counts (`xDC8_word` bit 0x14) the speed's share, at least 1.
    #[doc(alias = "it_8026B1D4")]
    fn item_damage(&self, g: GObjId, j: usize) -> f64 {
        let ip = super::super::it::item(self.game, g);
        let mut dmg = reg(ip.hitboxes[j].hit.damage);
        if ip.flags.flags.thrown {
            let v = ip.vel;
            let (x, y, z) = (reg(v.x), reg(v.y), reg(v.z));
            let speed = crate::math::sqrt_reg(fp::fadds(
                fp::fmuls(z, z),
                fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
            ));
            let c = super::super::it::item::common_data(self.game);
            dmg = fp::fmadds(speed, reg(c.f32(self.data, IT_SPEED_DAMAGE)), dmg);
            dmg = fp::fadds(dmg, reg(c.f32(self.data, IT_SPEED_DAMAGE_BASE)));
            if dmg <= 1.0 {
                return 1.0;
            }
        }
        dmg
    }

    /// Remembers the fighter in the item's boxes of box `j`'s group as kind `kind`, and in
    /// every item sharing its id's (`it_8026FAC4`, `it_8026FA2C`); with `clear`, a box that
    /// newly hit it can't clank this frame.
    #[doc(alias = "it_8026FAC4")]
    #[doc(alias = "it_8026FA2C")]
    pub(crate) fn item_add_victim(
        &mut self,
        g: GObjId,
        j: usize,
        kind: i32,
        v: Victim,
        clear: bool,
    ) {
        use super::super::it::{self, P_LINK_ITEM};
        let ip = it::item(self.game, g);
        let (id, group) = (ip.ignore_id, ip.hitboxes[j].hit.group);
        let add = |game: &mut super::super::state::Game, item: GObjId| {
            for k in 0..4 {
                let cur = &mut it::item_mut(game, item).hitboxes[k].hit;
                if cur.state != hitcapsule::DISABLED
                    && cur.group == group
                    && hitcapsule::add_victim(cur, kind, v)
                    && clear
                {
                    game.it.clank_boxes[k] = 0;
                }
            }
        };
        if !id.is_set() {
            add(self.game, g);
            return;
        }
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(o) = cur {
            cur = self.game.world.gobjs[o.0].next;
            if self.same_ignore_id(o, id, o == g) {
                add(self.game, o);
            }
        }
    }

    /// Remembers the fighter as a tip's victim in the item's boxes of box `j`'s group, and in
    /// every item sharing its id's, `it_8026FC00`.
    #[doc(alias = "it_8026FC00")]
    fn item_tip_group(&mut self, g: GObjId, j: usize, kind: i32, v: Victim) {
        use super::super::it::{self, P_LINK_ITEM};
        let ip = it::item(self.game, g);
        let (id, group) = (ip.ignore_id, ip.hitboxes[j].hit.group);
        let add = |game: &mut super::super::state::Game, item: GObjId| {
            for k in 0..4 {
                let cur = &mut it::item_mut(game, item).hitboxes[k].hit;
                if cur.state != hitcapsule::DISABLED && cur.group == group {
                    hitcapsule::add_victim2(cur, kind, v);
                }
            }
        };
        if !id.is_set() {
            add(self.game, g);
            return;
        }
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(o) = cur {
            cur = self.game.world.gobjs[o.0].next;
            if self.same_ignore_id(o, id, o == g) {
                add(self.game, o);
            }
        }
    }

    /// Whether item `o` shares the ignore id `id` (`itself`: it's the item `id` is from), as
    /// [`IgnoreId::same`](super::super::it::IgnoreId::same) takes a grapple beam's stack word.
    #[track_caller]
    fn same_ignore_id(&mut self, o: GObjId, id: super::super::it::IgnoreId, itself: bool) -> bool {
        id.same(super::super::it::item(self.game, o).ignore_id, itself)
    }

    /// The fighter takes item `g`'s box `j` on hurtbox `n`, `ftColl_80077C60`: the damage
    /// scaled by a grab's and the frozen state's factors and the fighter's own; a hit (not a
    /// tip) is remembered by the item, which keeps the most damage it dealt and which way;
    /// then, unless the fighter is invincible there, logged and recorded; a tip is logged as
    /// one, at half damage, with its sound. An item taken on touch (`x0_78` 4) stops at its
    /// seam. Returns whether the box hit.
    #[doc(alias = "ftColl_80077C60")]
    fn take_item_hit(&mut self, g: GObjId, j: usize, me: FighterId, n: usize) -> bool {
        use super::super::it;
        let data = self.data;
        let ip = it::item(self.game, g);
        if it::item::attr_of(ip).use_kind(data) == 4 {
            self.take_touch_item(g, j, me);
            return false;
        }
        let raw = self.item_damage(g, j);
        let mut f3 = raw;
        let mf = &self.game.fighters[me.0];
        if mf.victim_gobj.is_some() && !mf.state_flags.x221B_b5 {
            f3 = fp::fmuls(f3, reg(self.common().f32(0x128)));
        }
        if mf.motion_id == MS_DAMAGE_ICE {
            f3 = fp::fmuls(f3, reg(self.common().f32(0x714)));
        }
        let mut dmg = fp::fmuls(f3, reg(self.game.fighters[me.0].dmg.damage_scale));
        let tip = self.common().f32(0x7A8);
        let h = &it::item(self.game, g).hitboxes[j].hit;
        if reg(h.coll_distance) < reg(tip) {
            let mf = &self.game.fighters[me.0];
            if !self.game.ft.hit_logs.hits.is_empty()
                || mf.dmg.hit_frames != 0.0
                || hitcapsule::has_victim2(h, victim(me))
            {
                return false;
            }
            let mode = if h.fighter_hit_alt { 5 } else { 0 };
            self.item_tip_group(g, j, mode, victim(me));
            let mf = &self.game.fighters[me.0];
            if mf.hit_status == 0
                && mf.timed_hit_status == 0
                && !mf.starman
                && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
            {
                // Half the damage, from the damage taken whole (at least 1), and half the raw
                // damage's knockback count.
                let (scaled, whole_raw) = (fp::fctiwz(dmg), fp::fctiwz(raw));
                let half = fp::fmuls(0.5, fp::frsp(f64::from(scaled)));
                let half = if fp::fctiwz(half) == 0 && scaled != 0 {
                    1.0
                } else {
                    half
                };
                let half_raw = fp::frsp(f64::from(whole_raw / 2));
                let half_raw = if half_raw == 0.0 && whole_raw != 0 {
                    1.0
                } else {
                    half_raw
                };
                let mf = &mut self.game.fighters[me.0];
                if fp::fctiwz(half) > mf.dmg.max_tip_damage {
                    mf.dmg.max_tip_damage = fp::fctiwz(half);
                }
                let ip = it::item(self.game, g);
                let entry = LogEntry {
                    kind: ip.kind,
                    gobj: ip.entity,
                    hit: LoggedHit::Item { item: g, index: j },
                    hurt: n,
                    pos: ip.hitboxes[j].hit.hurt_coll_pos,
                    damage: mem(half),
                    count: cvt_fp2unsigned(half_raw),
                };
                let tips = &mut self.game.ft.hit_logs.tips;
                assert!(tips.len() < LOG_LEN, "tip log over {LOG_LEN}!!");
                tips.push(entry);
            }
            // `ftColl_80078488`.
            self.fighter_play_sfx(me, 85, 0x7F, 0x40);
            return true;
        }
        let mode = if h.fighter_hit_alt { 5 } else { 0 };
        self.game.ft.hit_logs.tips.clear();
        self.item_add_victim(g, j, mode, victim(me), false);
        let ip = it::item(self.game, g);
        if dmg > fp::frsp(f64::from(ip.damage_dealt)) {
            let slow = reg(it::item::common_data(self.game).f32(data, IT_SLOW_FOR_ITEM));
            let me_gobj = self.game.fighters[me.0].gobj;
            let mx = self.game.fighters[me.0].cur_pos.x;
            let ip = it::item_mut(self.game, g);
            ip.damage_dealt = fp::fctiwz(dmg);
            ip.met_fighter = me_gobj;
            let vx = reg(ip.vel.x);
            let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
            ip.out_damage_dir = if speed < slow {
                if reg(ip.pos.x) > reg(mx) { -1.0 } else { 1.0 }
            } else if vx < 0.0 {
                -1.0
            } else {
                1.0
            };
        }
        let mut logged = false;
        let mf = &self.game.fighters[me.0];
        if mf.hit_status == 0
            && mf.timed_hit_status == 0
            && !mf.starman
            && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
        {
            let count = whole_damage(dmg);
            let mf = &mut self.game.fighters[me.0];
            if mf.armor {
                mf.dmg.armor_hp = mem(fp::fsubs(reg(mf.dmg.armor_hp), dmg));
                if mf.dmg.armor_hp < 0.0 {
                    dmg = fp::fneg(reg(mf.dmg.armor_hp));
                    mf.armor = false;
                }
            }
            if !mf.armor {
                assert!(!(dmg > 500.0), "attack power over 500!! {dmg}");
                mf.dmg.pending_damage = mem(fp::fadds(reg(mf.dmg.pending_damage), dmg));
                if count > mf.dmg.damage_applied {
                    mf.dmg.damage_applied = count;
                }
                let ip = it::item(self.game, g);
                let h = &ip.hitboxes[j].hit;
                let entry = LogEntry {
                    kind: ip.kind,
                    gobj: ip.entity,
                    hit: LoggedHit::Item { item: g, index: j },
                    hurt: n,
                    pos: h.hurt_coll_pos,
                    damage: h.damage,
                    count: cvt_fp2unsigned(raw),
                };
                let hits = &mut self.game.ft.hit_logs.hits;
                assert!(hits.len() < LOG_LEN, "damage log over {LOG_LEN}!!");
                hits.push(entry);
                logged = true;
            }
            let vg = self.game.fighters[me.0]
                .gobj
                .expect("a fighter without its GObj");
            self.record_item_hit(g, vg, dmg);
        }
        if !logged {
            let pos = it::item(self.game, g).hitboxes[j].hit.hurt_coll_pos;
            self.spawn_effect(1052, pos, None);
        }
        true
    }

    /// What an item's hit on a fighter records, `ftColl_80078998`: the owner's stale-move
    /// queue (`plStale_UpdateStaleMovesFromItem`), its combo against the victim
    /// (`ftColl_8007646C`), and the damage both players' statistics count, when a fighter
    /// owns the item.
    #[doc(alias = "ftColl_80078998")]
    #[doc(alias = "plStale_UpdateStaleMovesFromItem")]
    #[doc(alias = "ftColl_8007646C")]
    fn record_item_hit(&mut self, g: GObjId, victim_gobj: GObjId, dmg: f64) {
        use super::super::it;
        let ip = it::item(self.game, g);
        let (owner, attack_id, instance, kind) = (
            ip.owner,
            ip.attack_id,
            ip.attack_instance,
            i32::from(ip.attack_flags[3]),
        );
        let owned = it::item::is_fighter(self.game, owner);
        if owned && owner != Some(victim_gobj) {
            let slot = i32::from(self.game.fighter(owner.expect("an owner")).player_idx);
            self.game.record_stale_move(slot, attack_id, instance);
        }
        if owned {
            let msid = it::lib26b1::attack_id(self.game, g);
            self.count_combo(owner.expect("an owner"), victim_gobj, msid);
        }
        if owned {
            let (of, vf) = (
                self.game.fighter(owner.expect("an owner")),
                self.game.fighter(victim_gobj),
            );
            let (ap, asub, vp, vsub) = (
                i32::from(of.player_idx),
                i32::from(of.is_sub_fighter),
                i32::from(vf.player_idx),
                i32::from(vf.is_sub_fighter),
            );
            self.count_damage(dmg, ap, asub, vp, vsub, kind);
        }
    }

    /// An item's hit's sound (`ftColl_8007925C`'s tail): the plain hit sound by severity when
    /// the hurtbox is invincible, else the hitbox's own by kind and severity.
    fn item_hit_sound(&mut self, g: GObjId, j: usize, me: FighterId, n: usize) {
        let h = &super::super::it::item(self.game, g).hitboxes[j].hit;
        let (severity, kind) = (h.sfx_severity, h.sfx_kind);
        self.hit_sound_of(me, n, kind, severity);
    }

    /// Stage devices that hurt or bury fighters, `ftColl_8007BAC0`: the registered collision
    /// device (`ft_80459A8C`, one slot) asked about the fighter where its type acts on it
    /// (Great Bay's sea, which never acts, and Brinstar's lava), acting on it with the hit it
    /// gives, then the stage lines the fighter touches.
    #[doc(alias = "ftColl_8007BAC0")]
    pub fn touch_devices(&mut self, gobj: GObjId) {
        if self.game.vs_no_fighter_hits() {
            return;
        }
        let me = self.game.fighter_id(gobj);
        let max = self.hit_status(me);
        for i in 0..self.game.gr.devices.coll_count {
            assert!(i == 0, "ftColl_8007BAC0: a device past ft_80459A8C's one");
            let d = &self.game.gr.devices;
            let Some(ground) = d.device2 else { continue };
            let (kind, cb) = (d.device2_type, d.device2_cb);
            if !self.device_applies(gobj, kind) {
                continue;
            }
            let desc = match cb.map(|c| c.0) {
                Some(super::super::gr::great_bay::SEA_DEVICE) => {
                    let _ = super::super::gr::great_bay::sea_device(self, gobj);
                    None
                }
                Some(super::super::gr::brinstar::LAVA_DEVICE) => {
                    super::super::gr::brinstar::lava_device(self, gobj)
                }
                Some(super::super::gr::mushroom_kingdom_route::WATER_DEVICE) => {
                    super::super::gr::mushroom_kingdom_route::water_device(self, gobj);
                    None
                }
                Some(super::super::gr::underground_maze::PIT_DEVICE) => {
                    super::super::gr::underground_maze::pit_device(self, gobj)
                }
                Some(super::super::gr::race_to_the_finish::SPIKES_DEVICE) => {
                    super::super::gr::race_to_the_finish::spikes_device(self, gobj)
                }
                Some(super::super::gr::target_test::MARIO_WATER) => {
                    super::super::gr::target_test::mario_water(self, gobj);
                    None
                }
                Some(other) => {
                    unported_call(&format!("a collision device's callback {other:#010x}"))
                }
                None => panic!("a collision device without a callback"),
            };
            if let Some(desc) = desc
                && max == 0
            {
                self.device_hit(gobj, Some(ground), desc, kind);
            }
        }
        if max == 0 {
            self.touch_lines(gobj);
        }
    }

    /// A stage device acting on the fighter, `ftCo_800C08A0`: its hit's damage
    /// (`ftColl_800765F0`: less while holding someone, more when frozen, times the damage
    /// multiplier); a burying type sets its timer; the damage taken (or the armor's) logs the
    /// hit (`ftColl_80076764`, its damage word left as the log's slot last held it) with its
    /// sound on the first hurtbox (`ftColl_80078384`); and the player's records count it
    /// (`pl_8003EC30`).
    #[doc(alias = "ftCo_800C08A0")]
    #[doc(alias = "ftColl_800765F0")]
    #[doc(alias = "ftColl_80076764")]
    #[doc(alias = "pl_8003EC30")]
    fn device_hit(
        &mut self,
        gobj: GObjId,
        ground: Option<GObjId>,
        desc: melee_data::archive::DataRef,
        kind: u32,
    ) {
        let me = self.game.fighter_id(gobj);
        let count = self.data.u32(desc.at(4));
        let c = self.common();
        let fp = &self.game.fighters[me.0];
        let mut d = fp::frsp(f64::from(count));
        if fp.victim_gobj.is_some() && !fp.state_flags.x221B_b5 {
            d = fp::fmuls(d, reg(c.f32(0x128)));
        }
        if fp.motion_id == MS_DAMAGE_ICE {
            d = fp::fmuls(d, reg(c.f32(0x714)));
        }
        let mut f = mem(fp::fmuls(d, reg(fp.dmg.damage_scale)));
        match kind {
            1 => self.game.fighters[me.0].bury_timer_1 = c.u32(0x5DC),
            3 => self.game.fighters[me.0].bury_timer_2 = c.u32(0x5E4),
            _ => {}
        }
        if self.take_damage_or_armor(me, &mut f) {
            let pos = self.game.fighters[me.0].cur_pos;
            let logs = &mut self.game.ft.hit_logs;
            let slot = logs.hits.len();
            assert!(slot < LOG_LEN, "damage log over {LOG_LEN}!!");
            let damage = logs.stale_damage[slot];
            logs.hits.push(LogEntry {
                kind: kind as i32,
                gobj: ground,
                hit: LoggedHit::Device { desc },
                hurt: 0,
                pos,
                damage,
                count,
            });
            let severity = self.data.u32(desc.at(0x1C)) as i32;
            let sfx = self.data.u32(desc.at(0x20)) as i32;
            self.hit_sound_of(me, 0, sfx, severity);
        }
        let fp = &self.game.fighters[me.0];
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        if !sub && matches!(kind, 1 | 3) {
            let t = &mut self.game.player_mut(slot).stale_moves;
            t.xC98 = mem(fp::fadds(reg(t.xC98), reg(f)));
        }
    }

    /// Damage from the stage lines the fighter touches, `ftCo_800C0B20`: the floor, ceiling
    /// and walls it touches, each asking the stage (`Ground_801C5700`) for a hazard; the
    /// last one asked wins, and hits as a burying device's would with no object
    /// ([`Self::device_hit`] of kind 1).
    #[doc(alias = "ftCo_800C0B20")]
    pub fn touch_lines(&mut self, gobj: GObjId) {
        let me = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[me.0];
        if fp.bury_timer_1 != 0 {
            return;
        }
        let coll = &fp.coll_data;
        let flags = coll.env_flags;
        let touched = [
            (flags & (0x8000 | 0x10000) != 0, coll.floor.index),
            (flags & (0x2000 | 0x4000) != 0, coll.ceiling.index),
            (flags & 0xFC0 != 0, coll.right_facing_wall.index),
            (flags & 0x3F != 0, coll.left_facing_wall.index),
        ];
        let mut hazard = None;
        for (touching, line) in touched {
            if touching {
                hazard = self.line_hazard_desc(line);
            }
        }
        if let Some(desc) = hazard {
            self.device_hit(gobj, None, desc, 1);
        }
    }

    /// Whether the stage gives the line a hazard (`Ground_801C5700`, through the stage's
    /// `on_touch_line`): only Mute City's track does, while it hurts.
    #[doc(alias = "Ground_801C5700")]
    #[doc(alias = "grLast_OnTouchLine")]
    #[doc(alias = "grBattle_OnTouchLine")]
    #[doc(alias = "grStadium_OnTouchLine")]
    #[doc(alias = "grStory_801E36D0")]
    #[doc(alias = "grOldPupupu_80211CA0")]
    #[doc(alias = "grIzumi_801CD278")]
    #[doc(alias = "grOldYoshi_8020F404")]
    #[doc(alias = "grKraid_OnTouchLine")]
    #[doc(alias = "grFourside_801F41E0")]
    #[doc(alias = "grGreatBay_801F680C")]
    #[doc(alias = "grOldKongo_80210780")]
    #[doc(alias = "grShrine_80201F44")]
    #[doc(alias = "grYorster_80202B5C")]
    #[doc(alias = "grGarden_802035C8")]
    #[doc(alias = "grKongo_801D8444")]
    #[doc(alias = "grGreens_80216E64")]
    #[doc(alias = "grZebes_801DCCB8")]
    #[doc(alias = "grOnett_801E5760")]
    #[doc(alias = "grPura_802130C0")]
    #[doc(alias = "grRCruise_80201C50")]
    #[doc(alias = "grFlatzone_802182B4")]
    #[doc(alias = "grInishie1_801FCBB0")]
    #[doc(alias = "grInishie2_801FDFE8")]
    #[doc(alias = "grIceMt_801FA8F8")]
    #[doc(alias = "grCorneria_801E2EE4")]
    #[doc(alias = "grCastle_801D0B04")]
    #[doc(alias = "grBigBlue_801EFC0C")]
    pub(crate) fn line_hazard(&mut self, line: i32) -> bool {
        self.line_hazard_desc(line).is_some()
    }

    /// The hit the stage gives a line ([`Self::line_hazard`]'s `DynamicsDesc`).
    pub(crate) fn line_hazard_desc(&mut self, line: i32) -> Option<melee_data::archive::DataRef> {
        let f = self.game.stage_info.on_touch_line?;
        let touch = super::super::gr::registry::by_address(self.game.stage_info.grkind, f.0, |d| {
            d.on_touch_line
        })
        .and_then(|s| s.touch_line)
        .unwrap_or_else(|| unported_call(&format!("a stage's on_touch_line {:#x}", f.0)));
        touch(self, line)
    }

    /// Thrown fighters' hitboxes against each other, `ft_8007C4BC`: when this fighter's
    /// thrown hitbox is live, each fighter after it in the list with one too, unless one
    /// holds the other; touching boxes push their owners apart in the air, others forget
    /// each other.
    #[doc(alias = "ft_8007C4BC")]
    pub fn thrown_hits(&mut self, gobj: GObjId) {
        let me = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[me.0];
        if !fp.thrown_hitbox_live || self.hit_status(me) == 2 || fp.is_sub_fighter {
            return;
        }
        let holder = if fp.victim_gobj.is_some() {
            fp.victim_gobj
        } else if fp.in_hitstun {
            fp.dmg.source
        } else {
            None
        };
        let mut seen = false;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if g == gobj {
                seen = true;
                continue;
            }
            if Some(g) == holder {
                continue;
            }
            let o = self.game.fighter_id(g);
            let of = &self.game.fighters[o.0];
            if of.out_of_play
                || !of.thrown_hitbox_live
                || self.hit_status(o) == 2
                || of.is_sub_fighter
                || (of.in_hitstun && of.dmg.source == Some(gobj))
            {
                continue;
            }
            if of.thrown_hitbox.state == hitcapsule::DISABLED || !seen {
                continue;
            }
            let mut a = hitcapsule::shape(&self.game.fighters[o.0].thrown_hitbox);
            let mut b = hitcapsule::shape(&self.game.fighters[me.0].thrown_hitbox);
            let (sa, sb) = (
                self.game.fighters[o.0].scale.y,
                self.game.fighters[me.0].scale.y,
            );
            let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
            self.game.fighters[o.0].thrown_hitbox.hurt_coll_pos = a.contact;
            self.game.fighters[me.0].thrown_hitbox.hurt_coll_pos = b.contact;
            if touch {
                self.thrown_clash(o, me);
            } else {
                hitcapsule::remove_victim(&mut self.game.fighters[me.0].thrown_hitbox, victim(o));
                hitcapsule::remove_victim(&mut self.game.fighters[o.0].thrown_hitbox, victim(me));
            }
        }
    }

    /// Two thrown fighters' boxes meet, `ft_8007C2E0`: each remembers the other and, in the
    /// air, takes some of the other's motion, by their weights.
    #[doc(alias = "ft_8007C2E0")]
    fn thrown_clash(&mut self, f0: FighterId, f1: FighterId) {
        if hitcapsule::has_victim(&self.game.fighters[f0.0].thrown_hitbox, victim(f1)) {
            return;
        }
        let mul = reg(self.common().f32(0x7D4));
        let push = |r: &mut Running, from: FighterId, to: FighterId| {
            hitcapsule::add_victim(&mut r.game.fighters[to.0].thrown_hitbox, 3, victim(from));
            let (pf, pt) = (
                r.game.fighters[f0.0].pos_delta,
                r.game.fighters[f1.0].pos_delta,
            );
            let (wf, wt) = (
                r.game.fighters[from.0].co_attrs.weight,
                r.game.fighters[to.0].co_attrs.weight,
            );
            let (df, dt) = (
                r.game.fighters[from.0].pos_delta,
                r.game.fighters[to.0].pos_delta,
            );
            let t = &mut r.game.fighters[to.0];
            if t.ground_or_air != GA_AIR {
                return;
            }
            let mut w = fp::fdivs(reg(wf), reg(wt));
            if w > 1.0 {
                w = 1.0;
            }
            w = fp::fmuls(w, mul);
            let axis = |same: bool, d_from: f32, d_to: f32, kb: f32| {
                mem(if same {
                    fp::fmadds(w, fp::fsubs(reg(d_from), reg(d_to)), reg(kb))
                } else {
                    fp::fmadds(reg(d_from), w, reg(kb))
                })
            };
            let same_x = fp::fmuls(reg(pf.x), reg(pt.x)) >= 0.0;
            let same_y = fp::fmuls(reg(pf.y), reg(pt.y)) >= 0.0;
            t.atk_shield_kb_vel.x = axis(same_x, df.x, dt.x, t.atk_shield_kb_vel.x);
            t.atk_shield_kb_vel.y = axis(same_y, df.y, dt.y, t.atk_shield_kb_vel.y);
        };
        push(self, f0, f1);
        push(self, f1, f0);
    }

    /// The frame's strongest hit becomes the fighter's damage result (`ftColl_8007AB48`, with
    /// `ftColl_8007A06C` on the hit log): its effects spawn as each hit is weighed.
    #[doc(alias = "ftColl_8007AB48")]
    pub fn take_hits(&mut self, gobj: GObjId) {
        // Lent out while the hits are weighed, which logs none.
        let log = std::mem::take(&mut self.game.ft.hit_logs.hits);
        self.weigh_hits(gobj, &log, false);
        assert!(
            self.game.ft.hit_logs.hits.is_empty(),
            "a hit logged while weighing"
        );
        self.game.ft.hit_logs.hits = log;
    }

    /// The same for the tips (`ftColl_8007AB80`), with no effects; `x18a0` keeps the result's
    /// knockback, 0 without tips.
    #[doc(alias = "ftColl_8007AB80")]
    pub fn take_tip_hits(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).dmg.tip_kb = 0.0;
        let log = std::mem::take(&mut self.game.ft.hit_logs.tips);
        self.weigh_hits(gobj, &log, true);
        assert!(
            self.game.ft.hit_logs.tips.is_empty(),
            "a tip logged while weighing"
        );
        self.game.ft.hit_logs.tips = log;
        let fp = self.game.fighter_mut(gobj);
        fp.dmg.tip_kb_applied = fp.dmg.tip_kb;
    }

    /// The knockback a fighter's hitbox deals this fighter, `ftColl_80079C70`.
    #[doc(alias = "ftColl_80079C70")]
    pub(crate) fn fighter_knockback(
        &self,
        me: FighterId,
        attacker: FighterId,
        hit: LoggedHit,
        count: u32,
    ) -> f64 {
        let fp = &self.game.fighters[me.0];
        let defense = self.game.player_defense_ratio(i32::from(fp.player_idx));
        let attack = self
            .game
            .player_attack_ratio(i32::from(self.game.fighters[attacker.0].player_idx));
        let ratio = self.game.vs_damage_ratio();
        self.knockback(me, hit, count, ratio, attack, defense, fp.co_attrs.weight)
    }

    /// Knockback, `ftColl_80079AB0`: a fixed amount (`x28`) or from the victim's damage after
    /// the hit, scaled by growth (`x24`) and the weight, plus the base (`x2C`), times the
    /// match's and players' ratios, capped at `x108`.
    #[doc(alias = "ftColl_80079AB0")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn knockback(
        &self,
        me: FighterId,
        hit: LoggedHit,
        count: u32,
        ratio: f32,
        attack: f32,
        defense: f32,
        weight: f32,
    ) -> f64 {
        let h = self.logged_capsule(hit).clone();
        self.knockback_of(me, &h, count, ratio, attack, defense, weight)
    }

    /// [`Running::knockback`] of a hit capsule held anywhere (one on the stack too).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn knockback_of(
        &self,
        me: FighterId,
        h: &super::super::structs::HitCapsule,
        count: u32,
        ratio: f32,
        attack: f32,
        defense: f32,
        weight: f32,
    ) -> f64 {
        let c = self.common();
        let fp = &self.game.fighters[me.0];
        let f = |o: u32| reg(c.f32(o));
        let w = fp::fmuls(reg(weight), f(0xF4));
        let weight_term = fp::fsubs(f(0xF8), fp::fdivs(fp::fmuls(w, f(0xF8)), fp::fadds(1.0, w)));
        let growth = if h.set_kb != 0 {
            let x118 = f(0x118);
            fp::fmadds(
                x118,
                f(0x110),
                fp::fmuls(f(0x114), fp::fmuls(x118, fp::frsp(f64::from(h.set_kb)))),
            )
        } else {
            let damage = if fp.is_stamina {
                if fp.stamina_dead {
                    c.i32(0x6D8)
                } else {
                    c.i32(0x6D4)
                }
            } else {
                fp::fctiwz(reg(fp.dmg.percent))
            };
            let total = fp::fadds(fp::frsp(f64::from(damage)), reg(fp.dmg.pending_damage));
            fp::fmadds(
                f(0x110),
                total,
                fp::fmuls(f(0x114), fp::fmuls(fp::frsp(f64::from(count)), total)),
            )
        };
        let inner = fp::fmadds(f(0x11C), fp::fmuls(weight_term, growth), f(0x120));
        let kb = fp::fmadds(
            fp::fmuls(0.009_999_999_776_482_582, fp::frsp(f64::from(h.kb_growth))),
            inner,
            fp::frsp(f64::from(h.base_kb)),
        );
        let result = fp::fmuls(
            reg(defense),
            fp::fmuls(reg(attack), fp::fmuls(reg(ratio), kb)),
        );
        let cap = f(0x108);
        if result >= cap { cap } else { result }
    }

    /// Picks the logged hit with the most knockback, `ftColl_8007A06C`, and writes the
    /// fighter's damage result for it (`dmg.facing_dir_1` on for hits, `dmg.x1870` on for
    /// tips): direction, angle (the autolink angle 362 aims from the hit toward the hurtbox's
    /// middle), hurtbox height, knockback, where, element, sound, source and damage; the
    /// attack then counts in the statistics (`ftColl_8007861C`). For hits, each one's effect
    /// spawns as it's weighed.
    #[doc(alias = "ftColl_8007A06C")]
    fn weigh_hits(&mut self, gobj: GObjId, log: &[LogEntry], tips: bool) {
        if log.is_empty() {
            return;
        }
        let me = self.game.fighter_id(gobj);
        let mut best = -1.0;
        let mut best_at = None;
        for (i, e) in log.iter().enumerate() {
            let kb = match e.hit {
                LoggedHit::Fighter { .. } => {
                    let attacker = self
                        .game
                        .fighter_id(e.gobj.expect("a logged hit without its GObj"));
                    self.fighter_knockback(me, attacker, e.hit, e.count)
                }
                LoggedHit::Item { item, .. } => self.item_knockback(me, item, e.hit, e.count),
                LoggedHit::Throw { .. } => unreachable!("a throw's hitbox in the damage log"),
                LoggedHit::Device { desc } => self.device_knockback(me, desc),
            };
            if !tips && !matches!(e.hit, LoggedHit::Device { .. }) {
                let h = self.logged_capsule(e.hit);
                let (element, severity) = (h.element, h.sfx_severity);
                let dmg = cvt_fp2unsigned(reg(e.damage));
                let effect = HIT_EFFECTS[element as usize];
                self.hit_effect(me, effect, e.pos, severity as u32, dmg, kb);
            }
            if kb > best {
                let fp = &self.game.fighters[me.0];
                let held = matches!(e.hit, LoggedHit::Fighter { .. })
                    && fp.victim_gobj.is_some()
                    && !fp.state_flags.x221B_b5
                    && fp.victim_gobj == e.gobj;
                self.game.fighters[me.0].held_hit_light = held;
                best = kb;
                best_at = Some(i);
            }
        }
        let e = log[best_at
            .expect("no logged hit with knockback above -1 (the original reads a stale index)")];
        let (kb_angle, element, severity, angle_word) = match e.hit {
            LoggedHit::Device { desc } => {
                let h = device_capsule(self.data, desc);
                // The angle test reads the hitbox's angle word through the description,
                // where it holds the sound kind.
                (h.kb_angle as u32, h.element, 0, h.sfx_kind as u32)
            }
            hit => {
                let h = self.logged_capsule(hit);
                (
                    h.kb_angle as u32,
                    h.element,
                    h.sfx_severity,
                    h.kb_angle as u32,
                )
            }
        };
        let fp = &self.game.fighters[me.0];
        let mut dir: f32 = match e.hit {
            LoggedHit::Throw { .. } => unreachable!("a throw's hitbox in the damage log"),
            LoggedHit::Device { .. } => fp.facing_dir,
            LoggedHit::Fighter { .. } => {
                let attacker = self
                    .game
                    .fighter_id(e.gobj.expect("a logged hit without its GObj"));
                let ax = self.game.fighters[attacker.0].cur_pos.x;
                if reg(fp.cur_pos.x) > reg(ax) {
                    -1.0
                } else {
                    1.0
                }
            }
            LoggedHit::Item { item, .. } => {
                let ip = super::super::it::item(self.game, item);
                let slow = reg(super::super::it::item::common_data(self.game)
                    .f32(self.data, IT_SLOW_FOR_RESULT));
                let vx = reg(ip.vel.x);
                let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
                if speed < slow {
                    if reg(fp.cur_pos.x) > reg(ip.pos.x) {
                        -1.0
                    } else {
                        1.0
                    }
                } else if vx < 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        };
        let mut angle = fp::frsp(f64::from(kb_angle));
        let hurt = &fp.hurt_capsules[e.hurt];
        if angle_word == 0x16A {
            let c = &hurt.capsule;
            let dx = fp::fmsubs(0.5, fp::fadds(reg(c.a_pos.x), reg(c.b_pos.x)), reg(e.pos.x));
            let dy = fp::fmsubs(0.5, fp::fadds(reg(c.a_pos.y), reg(c.b_pos.y)), reg(e.pos.y));
            dir = if dx < 0.0 { 1.0 } else { -1.0 };
            let abs = if dx < 0.0 { fp::fneg(dx) } else { dx };
            let deg = if abs < reg(1e-5) {
                0
            } else {
                let a = crate::math::atan(fp::fdivs(dy, abs) as f32);
                fp::fctiwz(fp::fmuls(reg(57.295_78), reg(a)))
            };
            angle = fp::frsp(f64::from(deg));
        }
        let height = hurt.height;
        let fp = &mut self.game.fighters[me.0];
        let d = &mut fp.dmg;
        let angle = fp::fctiwz(angle);
        if tips {
            // The tip result's direction: a float where the layout has a pointer.
            d.unread_1870 = super::super::handles::RawPtr(dir.to_bits());
            d.unread_1874 = angle;
            d.unread_1878 = height;
            d.tip_kb = mem(best);
            d.tip_pos = e.pos;
            d.tip_element = element as i32;
            d.tip_severity = severity;
            d.tip_source = e.gobj;
            d.tip_damage = e.damage;
        } else {
            d.facing_dir_1 = dir;
            d.kb_angle = angle;
            d.damaged_hurtbox = height;
            d.kb_applied = mem(best);
            d.coll_pos = e.pos;
            d.element = element;
            d.unread_1864 = severity;
            d.source = e.gobj;
            // The hit result's damage: a float where the layout has an integer.
            d.unread_186c = e.damage.to_bits() as i32;
        }
        let grounded = height == 0;
        match e.hit {
            LoggedHit::Throw { .. } => unreachable!("a throw's hitbox in the damage log"),
            LoggedHit::Fighter { .. } => {
                let a = self
                    .game
                    .fighter_id(e.gobj.expect("a logged hit without its GObj"));
                let af = &self.game.fighters[a.0];
                let (a_gobj, kind, x2070, instance) = (
                    af.gobj.expect("a fighter without its GObj"),
                    af.kind,
                    u32::from_be_bytes(af.attack_flags),
                    af.attack_record.x2088,
                );
                self.attack_landed(
                    Some(a_gobj),
                    gobj,
                    1,
                    kind,
                    x2070,
                    AttackRecord::Fighter(a_gobj),
                    instance,
                    false,
                    grounded,
                );
            }
            LoggedHit::Item { item, .. } => self.item_attack_landed(item, gobj, grounded),
            LoggedHit::Device { .. } => {
                let fp = &mut self.game.fighters[me.0];
                let prev = fp.dmg.source_player;
                fp.dmg.source_spawn = 0;
                fp.dmg.source_attack = -1;
                fp.dmg.source_class = 3;
                fp.dmg.source_kind = e.kind;
                self.record_no_attack(gobj, prev);
            }
        }
        if element == ELEMENT_ELECTRIC {
            let v = self.common().f32(0x1A4);
            self.game.fighters[me.0].hitlag_vibrate_mul = v;
        }
    }

    /// A tip's damage, held back while its hitlag lasted, lands, `ftColl_8007BE3C`: taken
    /// (or the armor's), recorded for whoever's hitbox it was, a fighter's or an item's, and
    /// its effect spawned where it hit.
    #[doc(alias = "ftColl_8007BE3C")]
    pub(crate) fn delayed_tip_damage(&mut self, gobj: GObjId) {
        let me = self.game.fighter_id(gobj);
        let mut dmg = self.game.fighters[me.0].dmg.tip_damage;
        let taken = self.take_damage_or_armor(me, &mut dmg);
        self.game.fighters[me.0].dmg.tip_damage = dmg;
        if !taken {
            return;
        }
        let source = self.game.fighters[me.0]
            .dmg
            .tip_source
            .expect("a tip without its source");
        // A source freed since the hit (a pill that vanished as it hit) keeps the free list's
        // link in its first word, so its classifier reads as the link's high half (0x80xx, or 0
        // at the list's end): neither a fighter's nor an item's (`HSD_ObjFree`).
        let classifier = if self.game.world.gobjs.is_free(source.0) {
            0
        } else {
            self.game.world.gobjs[source.0].classifier
        };
        match classifier {
            4 => self.record_fighter_hit(source, gobj, reg(dmg)),
            6 => self.record_item_hit(source, gobj, reg(dmg)),
            _ => {}
        }
        let d = &self.game.fighters[me.0].dmg;
        let (element, pos, severity, kb) = (d.tip_element, d.tip_pos, d.tip_severity, d.tip_kb);
        let effect = HIT_EFFECTS[element as usize];
        let whole = cvt_fp2unsigned(reg(dmg));
        self.hit_effect(me, effect, pos, severity as u32, whole, reg(kb));
    }

    /// The damage a hit deals the fighter, `ftColl_80076640` (inlined in the throw's hit):
    /// armor (`x221C_b4`) takes it first, and what passes it is the damage; whether any did,
    /// counted into the damage taken this frame.
    fn take_damage_or_armor(&mut self, f: FighterId, dmg: &mut f32) -> bool {
        let count = whole_damage(reg(*dmg));
        let fp = &mut self.game.fighters[f.0];
        if fp.armor {
            fp.dmg.armor_hp = mem(fp::fsubs(reg(fp.dmg.armor_hp), reg(*dmg)));
            if fp.dmg.armor_hp < 0.0 {
                *dmg = mem(fp::fneg(reg(fp.dmg.armor_hp)));
                fp.armor = false;
            }
        }
        if fp.armor {
            return false;
        }
        // NaN passes, as the original's `!(dmg > 500.0)` does.
        assert!(
            reg(*dmg).partial_cmp(&500.0) != Some(std::cmp::Ordering::Greater),
            "attack power over 500!! {dmg}"
        );
        fp.dmg.pending_damage = mem(fp::fadds(reg(fp.dmg.pending_damage), reg(*dmg)));
        if count > fp.dmg.damage_applied {
            fp.dmg.damage_applied = count;
        }
        true
    }

    /// A hit's effect by its element (`spawnHitEffect`): the normal hit's spark by knockback
    /// (`ftColl_80078538`), others' own.
    fn hit_effect(
        &mut self,
        me: FighterId,
        effect: i32,
        pos: Vec3,
        severity: u32,
        dmg: u32,
        kb: f64,
    ) {
        // The effects it treats apart: the normal hit's spark, sized by the knockback, and
        // the one that faces the attacker's way.
        const HIT_SPARK: i32 = 1000;
        const HIT_FACING: i32 = 1005;
        match effect {
            HIT_SPARK => self.hit_spark(me, pos, severity, mem(fp::frsp(f64::from(dmg))), kb),
            1001 | 1002 | 1004 | 1046 | 1145 | 1255 => self.spawn_effect(effect, pos, None),
            HIT_FACING => {
                let dir = self.game.fighters[me.0].facing_dir;
                self.spawn_effect(effect, pos, Some(dir));
            }
            _ => {}
        }
    }

    /// The normal hit's spark, `ftColl_80078538`: small or big by knockback, and for a hit
    /// with any severity a chance of the character's extra spark.
    #[doc(alias = "ftColl_80078538")]
    fn hit_spark(&mut self, me: FighterId, pos: Vec3, severity: u32, dmg: f32, kb: f64) {
        let c = self.common();
        let (big, chance0, chance1) = (c.f32(0x3F0), c.i32(0x3F4), c.i32(0x3F8));
        if kb < reg(big) {
            self.spawn_effect(1000, pos, Some(dmg));
        } else {
            self.spawn_effect(0x3F3, pos, None);
        }
        if severity >= 1 {
            match self.game.fighters[me.0].co_attrs.hit_spark_variant {
                0 => {
                    if self.game.objs.rng.below(chance0) == 0 {
                        let dir = self.game.fighters[me.0].facing_dir;
                        self.spawn_effect(0x3EF, pos, Some(dir));
                    }
                }
                1 => {
                    self.game.objs.rng.below(chance1);
                }
                _ => {}
            }
        }
    }

    /// Who hit the fighter and with what, `ftColl_8007861C`: the attacking fighter's spawn
    /// number and player (an item's owner's, `kind` 2), or none, which forgets the last
    /// attacker's player unless `keep_source`; then the attack statistics (`pl_80038144`, the
    /// seam).
    #[doc(alias = "ftColl_8007861C")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attack_landed(
        &mut self,
        attacker: Option<GObjId>,
        gobj: GObjId,
        kind: i32,
        akind: i32,
        x2070: u32,
        record: AttackRecord,
        instance: u16,
        keep_source: bool,
        grounded: bool,
    ) {
        let a = attacker.map(|g| self.game.fighter_id(g));
        let me = self.game.fighter_id(gobj);
        let prev = self.game.fighters[me.0].dmg.source_player;
        match a {
            Some(a) => {
                let af = &self.game.fighters[a.0];
                let (spawn, player, sub) = (af.spawn_num, af.player_idx, af.is_sub_fighter);
                let fp = &mut self.game.fighters[me.0];
                fp.dmg.source_spawn = if a == me { 0 } else { spawn };
                fp.dmg.source_player = i32::from(player);
                fp.dmg.source_attack = -1;
                fp.last_hit_by_sub = sub;
            }
            None => {
                let fp = &mut self.game.fighters[me.0];
                fp.dmg.source_spawn = 0;
                fp.dmg.source_attack = -1;
                if !keep_source {
                    fp.dmg.source_player = 6;
                }
            }
        }
        let fp = &mut self.game.fighters[me.0];
        fp.dmg.source_class = kind;
        fp.dmg.source_kind = akind;
        let _ = grounded;
        self.record_attack(attacker, gobj, x2070, record, instance, prev);
    }

    /// An item's attack on the fighter, `ftColl_800787B4`: a Bob-omb's counts for the
    /// fighter's player, and the attacker is the item's fighter owner, or none (an
    /// explosion's keeps the last attacker's player, `pl_8003D60C`).
    #[doc(alias = "ftColl_800787B4")]
    pub fn item_attack_landed(&mut self, item: GObjId, gobj: GObjId, grounded: bool) {
        let me = self.game.fighter_id(gobj);
        let ip = super::super::it::item(self.game, item);
        let (kind, owner, x2070, instance) = (
            ip.kind,
            ip.owner,
            u32::from_be_bytes(ip.attack_flags),
            ip.attack_record_short,
        );
        if kind == ItKind::BobOmb {
            let spawn = ip.spawn_num as u16;
            let slot = i32::from(self.game.fighters[me.0].player_idx);
            self.game.count_bombhei_hit(slot, spawn);
        }
        let (attacker, keep) = if super::super::it::item::is_fighter(self.game, owner) {
            (owner, false)
        } else {
            (None, crate::game::pl::death::explosive_kind(kind))
        };
        self.attack_landed(
            attacker,
            gobj,
            2,
            kind,
            x2070,
            AttackRecord::Item(item),
            instance,
            keep,
            grounded,
        );
    }

    /// No attacker on the fighter's last hit, `ftColl_800788D4` (`ftColl_8007861C` with no
    /// attacker, attack 0 and kind -10, and no record).
    #[doc(alias = "ftColl_800788D4")]
    pub(crate) fn clear_attacker(&mut self, gobj: GObjId) {
        let me = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[me.0];
        let prev = fp.dmg.source_player;
        fp.dmg.source_spawn = 0;
        fp.dmg.source_attack = -1;
        fp.dmg.source_player = 6;
        fp.dmg.source_class = 0;
        fp.dmg.source_kind = -10;
        self.record_no_attack(gobj, prev);
    }

    /// The lipstick flower's damage this frame, `ftColl_8007BBCC`: the logged fighters' and
    /// items' hits of the lipstick element.
    #[doc(alias = "ftColl_8007BBCC")]
    pub fn lipstick_damage(&self) -> f64 {
        let mut dmg = 0.0;
        for e in &self.game.ft.hit_logs.hits {
            if matches!(e.hit, LoggedHit::Device { .. }) {
                continue;
            }
            let h = self.logged_capsule(e.hit);
            if h.element == ELEMENT_LIPSTICK {
                // An item's hit deals its damage with its speed's (`it_8026B1D4`).
                let d = match e.hit {
                    LoggedHit::Item { item, index } => self.item_damage(item, index),
                    _ => reg(h.damage),
                };
                dmg = fp::fadds(dmg, d);
            }
        }
        dmg
    }
}

/// A stage device's hit as `lbColl_80008D30` copies its description onto the stack.
#[doc(alias = "lbColl_80008D30")]
pub(crate) fn device_capsule(
    data: &melee_data::archive::Archives,
    desc: melee_data::archive::DataRef,
) -> super::super::structs::HitCapsule {
    let w = |o: u32| data.u32(desc.at(o));
    super::super::structs::HitCapsule {
        state: w(0) as i32,
        damage: mem(fp::frsp(f64::from(w(4)))),
        kb_angle: w(8) as i32,
        kb_growth: w(0xC),
        set_kb: w(0x10),
        base_kb: w(0x14),
        element: w(0x18),
        sfx_severity: w(0x1C) as i32,
        sfx_kind: w(0x20) as i32,
        whole_damage: w(4),
        ..Default::default()
    }
}
