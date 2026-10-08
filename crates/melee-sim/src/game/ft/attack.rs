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

/// `It_Kind_BombHei`, and `It_Kind_Unk4`, which hits no fighter.
const IT_KIND_BOMBHEI: i32 = 6;
const IT_KIND_UNK4: i32 = 158;

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
#[derive(Clone, Copy)]
enum Bubble {
    Shield,
    Reflect,
    Absorb,
}

/// How many entries each damage log holds.
const LOG_LEN: usize = 20;

/// The hitbox a logged hit came from (`DmgLogEntry::xC`): one of a fighter's (`x0` 1) or of
/// an item's (2), by its GObj.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoggedHit {
    Fighter { fighter: FighterId, index: usize },
    Item { item: GObjId, index: usize },
}

/// A logged hit (`DmgLogEntry`): a fighter's or an item's. The stage's stop at their seams
/// before they would log.
#[derive(Clone, Copy, Debug, PartialEq)]
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitLogs {
    pub hits: Vec<LogEntry>,
    pub tips: Vec<LogEntry>,
    pub clank: [bool; 4],
}

/// A hit's damage as an integer, at least 1 for any damage, `getEnvDmg`.
fn whole_damage(dmg: f64) -> i32 {
    if dmg != 0.0 {
        let i = fp::fctiwz(dmg);
        if i != 0 { i } else { 1 }
    } else {
        0
    }
}

/// Which fighter a hitbox's GObj carries, for the victim lists.
fn victim(f: FighterId) -> Victim {
    Victim::Fighter(f)
}

/// `Fighter_procAttackColl`: unless the fighter sleeps or is out (`x2219_b1`), the frame's
/// hits on it from fighters, coins, items, stage devices and other thrown fighters, then what
/// they do, and the lipstick flower's damage over time.
#[doc(alias = "Fighter_procAttackColl")]
pub fn proc_attack_coll(r: &mut Running, gobj: GObjId) {
    let fp = r.game.fighter(gobj);
    if fp.is_sleeping || fp.x2219_b1 {
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
        unported_call("ftCommon_8007FC7C (the lipstick flower's damage)");
    }
}

impl Running<'_> {
    /// Starts both damage logs over, `ftColl_800765E0`.
    #[doc(alias = "ftColl_800765E0")]
    pub fn clear_hit_logs(&mut self) {
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
        (reg(fp.x34_scale.z) != 1.0).then_some(Mtx34 { m: fp.x44_mtx })
    }

    /// Whether a hitbox can't touch the fighter because something else holds it: unless it
    /// is still 0, the thrown fighter's thrower (`x1064_thrownHitbox.x134`, a GObj pointer the
    /// mirror keeps as raw bytes) isn't modeled.
    fn thrower(&self, f: FighterId) -> Option<GObjId> {
        if self.game.fighters[f.0].x1064_thrownHitbox.x134 != [0; 4] {
            unported_call("a thrown fighter's thrower (Fighter::x1198)");
        }
        None
    }

    /// The highest of the fighter's invincibility states, `ftColl_8007B868`: 2 makes it
    /// intangible, 1 invincible.
    #[doc(alias = "ftColl_8007B868")]
    pub fn hit_status(&self, f: FighterId) -> i32 {
        let fp = &self.game.fighters[f.0];
        i32::from(fp.x221D_b6).max(fp.x1988).max(fp.x198C)
    }

    /// The other fighters' hitboxes against this one, `ftColl_80078C70`: each other fighter
    /// not on its side (team rules aside), whose boxes touch its boxes (a clank, between two
    /// grounded fighters, tested only against fighters after it in the list), its shield, or
    /// a hurtbox.
    #[doc(alias = "ftColl_80078C70")]
    pub fn hits_from_fighters(&mut self, gobj: GObjId) {
        if self.game.vs_x5_2() {
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
            if mf.player_idx == vf.grabber_unk1 {
                return;
            }
        } else if mf.player_idx == vf.player_idx {
            return;
        }
        let other_team = if thrown { vf.x119C_teamUnk } else { vf.team };
        let my_team = mf.team;
        let other_b4 = vf.x2225_b4;
        if self.game.vs_is_teams()
            && !self.game.vs_friendly_fire()
            && !other_b4
            && my_team == other_team
        {
            return;
        }
        // The thrower can't hit the fighter it throws (`x1198 == this_gobj`): no thrower here.
        let mut count = 0;
        if passed && !self.game.fighters[me.0].x221B.x221B_b5 {
            let other_ga = self.game.fighters[v.0].ground_or_air;
            for i in 0..4 {
                let h = &self.game.fighters[me.0].x914[i];
                let can = h.state != hitcapsule::DISABLED
                    && !h.x43_b2
                    && h.element != ELEMENT_CATCH
                    && h.element != ELEMENT_INERT
                    && h.x40_b0
                    && h.x42_b5
                    && ((h.x40_b2 && other_ga == GA_AIR) || (h.x40_b3 && other_ga == GA_GROUND))
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
        let h0 = &vf.x914[j];
        if h0.state == hitcapsule::DISABLED || h0.element == ELEMENT_CATCH || !h0.x42_b5 {
            return;
        }
        if !((h0.x40_b2 && mf.ground_or_air == GA_AIR)
            || (h0.x40_b3 && mf.ground_or_air == GA_GROUND))
        {
            return;
        }
        // A box that only hits its owner's grab victim.
        if h0.x134[0] & 0x80 != 0
            && vf.victim_gobj.is_some()
            && vf.x221B.x221B_b5
            && vf.victim_gobj != Some(gobj)
        {
            return;
        }
        if hitcapsule::has_victim(h0, victim(me)) {
            return;
        }
        let phantom = h0.x43_b2;
        let element = h0.element;
        if passed
            && !phantom
            && vf.victim_gobj != Some(gobj)
            && mf.ground_or_air == GA_GROUND
            && vf.ground_or_air == GA_GROUND
            && !mf.x221B.x221B_b5
            && element != ELEMENT_INERT
            && h0.x40_b0
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
        if mf.x221B.x221B_b0 {
            let vx = self.game.fighters[v.0].cur_pos.x;
            let mut facing = true;
            if mf.x221B.x221B_b3 {
                if -1.0 == reg(mf.facing_dir) {
                    if reg(mf.cur_pos.x) < reg(vx) {
                        facing = false;
                    }
                } else if reg(mf.cur_pos.x) > reg(vx) {
                    facing = false;
                }
            }
            if mf.x221B.x221B_b4 && !self.game.fighters[v.0].x914[j].x42_b4 {
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
            vf.x221C_b5 = true;
            vf.unk_gobj = Some(gobj);
        }
        let mf = &self.game.fighters[me.0];
        if mf.x1988 == 2 || mf.x198C == 2 {
            return;
        }
        let mut n = 0;
        while n < usize::from(self.game.fighters[me.0].hurt_capsules_len) {
            if self.box_hits_hurtbox(v, j, me, n, phantom) {
                if element == ELEMENT_INERT {
                    self.game.fighters[v.0].unk_gobj = Some(gobj);
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
        let mut a = hitcapsule::shape(&self.game.fighters[v.0].x914[j]);
        let mut b = hitcapsule::shape(&self.game.fighters[me.0].x914[m]);
        let (sa, sb) = (
            self.game.fighters[v.0].x34_scale.y,
            self.game.fighters[me.0].x34_scale.y,
        );
        let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
        self.game.fighters[v.0].x914[j].hurt_coll_pos = a.contact;
        self.game.fighters[me.0].x914[m].hurt_coll_pos = b.contact;
        touch
    }

    /// Whether the other fighter's box `j` touches this fighter's shield (`lbColl_80007BCC`).
    fn box_hits_shield(&mut self, v: FighterId, j: usize, me: FighterId, phantom: bool) -> bool {
        let mtx = self.own_matrix(me);
        let mut hit = self.game.fighters[v.0].x914[j].clone();
        let hit_scale = self.game.fighters[v.0].x34_scale.y;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.x34_scale.y, fp.cur_pos.z);
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
        let h = &mut self.game.fighters[v.0].x914[j];
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
        let mut hit = self.game.fighters[v.0].x914[j].clone();
        let hit_scale = self.game.fighters[v.0].x34_scale.y;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.x34_scale.y, fp.cur_pos.z);
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
        let h = &mut self.game.fighters[v.0].x914[j];
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// Remembers `victim` in each live box of `f`'s group of box `i` (`ftColl_80076808`);
    /// with `clear`, a box that newly hits it can't clank this frame.
    #[doc(alias = "ftColl_80076808")]
    pub(crate) fn hit_group(&mut self, f: FighterId, i: usize, kind: i32, v: Victim, clear: bool) {
        let group = self.game.fighters[f.0].x914[i].x4;
        for k in 0..4 {
            let cur = &mut self.game.fighters[f.0].x914[k];
            if cur.state != hitcapsule::DISABLED
                && cur.x4 == group
                && hitcapsule::add_victim(cur, kind, v)
                && clear
            {
                self.game.ft.hit_logs.clank[k] = false;
            }
        }
    }

    /// Remembers a phantom hit's victim in each live box of the group.
    fn tip_group(&mut self, f: FighterId, i: usize, kind: i32, v: Victim) {
        let group = self.game.fighters[f.0].x914[i].x4;
        for k in 0..4 {
            let cur = &mut self.game.fighters[f.0].x914[k];
            if cur.state != hitcapsule::DISABLED && cur.x4 == group {
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
            if fp.x914[i].x40_b1 && fp.ground_or_air == GA_GROUND {
                fp.dmg.x191C = mem(fp::fmadds(fp::frsp(f64::from(dmg)), reg(mul), reg(add)));
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
            &self.game.fighters[v.0].x914[j],
            &self.game.fighters[me.0].x914[m],
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

    /// The clank's sound, `ftColl_800784B4`: a random sword clash between two slashes.
    #[doc(alias = "ftColl_800784B4")]
    fn clank_sound(&mut self, me: FighterId, v: FighterId, j: usize, m: usize) {
        let e0 = self.game.fighters[v.0].x914[j].element;
        let e1 = self.game.fighters[me.0].x914[m].element;
        let id = if e0 == ELEMENT_SLASH && e1 == ELEMENT_SLASH {
            SLASH_CLANK_SFX[self.game.objs.rng.below(3) as usize]
        } else {
            0x6A
        };
        self.fighter_play_sfx(me, id, 0x7F, 0x40);
    }

    /// The other fighter's box `j` hits this fighter's shield, `ftColl_80076CBC`: the
    /// attacker's shield-hit push and the shield's damage, recoil and spark; a broken shield
    /// isn't ported.
    #[doc(alias = "ftColl_80076CBC")]
    fn shield_hit(&mut self, v: FighterId, j: usize, me: FighterId) {
        self.hit_group(v, j, 1, victim(me), false);
        let h0 = &self.game.fighters[v.0].x914[j];
        let (dmg, element, x34, pos) = (
            whole_damage(reg(h0.damage)),
            h0.element,
            h0.x34,
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
        if dmg > vf.dmg.x1924 {
            vf.dmg.x1924 = dmg;
            if vf.ground_or_air == GA_GROUND {
                vf.dmg.x1928 = mem(fp::fmuls(reg(light), fp::frsp(f64::from(dmg))));
                vf.dmg.x192c = if reg(my_x) > reg(vf.cur_pos.x) {
                    1.0
                } else {
                    -1.0
                };
            }
        }
        let vx = vf.cur_pos.x;
        let mf = &mut self.game.fighters[me.0];
        if dmg > mf.x19A4 {
            mf.x19A4 = dmg;
            mf.specialn_facing_dir = if reg(mf.cur_pos.x) > reg(vx) {
                -1.0
            } else {
                1.0
            };
            mf.x19B0 = element as i32;
            mf.x19A8 = other_gobj;
        }
        if mf.x221C_b2 {
            unported_call("ftCo_80094138 (a shield broken by a hit)");
        }
        let total = dmg.wrapping_add(x34);
        mf.x19A0_shieldDamageTaken = mf.x19A0_shieldDamageTaken.wrapping_add(total.max(0));
        mf.x19BC_shieldDamageTaken3 = i32::from(other_player);
        mf.x221F_b6 = other_sub;
        self.spawn_effect(1052, pos, None);
        let push = self.game.fighters[me.0].shield_unk0;
        if push != 0.0 {
            self.game.fighters[v.0].x1964 = push;
            self.game.fighters[me.0].x1964 = push;
        }
    }

    /// A hit's damage to this fighter (`inlineB3`): less for one who holds someone else, more
    /// when frozen, times its damage multiplier.
    fn scaled_damage(&self, v: FighterId, j: usize, me: FighterId) -> f64 {
        let c = self.common();
        let (vf, mf) = (&self.game.fighters[v.0], &self.game.fighters[me.0]);
        let mut d = reg(vf.x914[j].damage);
        if mf.victim_gobj.is_some() && !mf.x221B.x221B_b5 && mf.victim_gobj != vf.gobj {
            d = fp::fmuls(d, reg(c.f32(0x128)));
        }
        if mf.motion_id == MS_DAMAGE_ICE {
            d = fp::fmuls(d, reg(c.f32(0x714)));
        }
        fp::fmuls(d, reg(mf.dmg.x182c_behavior))
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
    #[doc(alias = "ftColl_80076ED8")]
    fn take_hit(&mut self, v: FighterId, j: usize, me: FighterId, n: usize) -> bool {
        let mut dmg = self.scaled_damage(v, j, me);
        let tip = self.common().f32(0x7A8);
        let h0 = &self.game.fighters[v.0].x914[j];
        if reg(h0.coll_distance) < reg(tip) {
            let mf = &self.game.fighters[me.0];
            if !self.game.ft.hit_logs.hits.is_empty() || mf.dmg.x189C_unk_num_frames != 0.0 {
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
            if mf.x1988 == 0
                && mf.x198C == 0
                && !mf.x221D_b6
                && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
            {
                let h0 = &self.game.fighters[v.0].x914[j];
                let count = h0.unk_count;
                let mut len = count >> 1;
                if len == 0 && count != 0 {
                    len = 1;
                }
                let whole = fp::fctiwz(half);
                let pos = h0.hurt_coll_pos;
                let mf = &mut self.game.fighters[me.0];
                if whole > mf.dmg.x1840 {
                    mf.dmg.x1840 = whole;
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
        let group = self.game.fighters[v.0].x914[j].x4;
        for k in 0..4 {
            let cur = &mut self.game.fighters[v.0].x914[k];
            if cur.state != hitcapsule::DISABLED && cur.x4 == group {
                hitcapsule::add_victim(cur, 0, victim(me));
            }
        }
        let vf = &mut self.game.fighters[v.0];
        if whole > vf.dmg.x1914 {
            vf.dmg.x1914 = whole;
        }
        let mf = &self.game.fighters[me.0];
        if mf.x1988 == 0
            && mf.x198C == 0
            && !mf.x221D_b6
            && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
        {
            let count = whole_damage(dmg);
            let mf = &mut self.game.fighters[me.0];
            if mf.x221C_b4 {
                mf.dmg.x1834 = mem(fp::fsubs(reg(mf.dmg.x1834), dmg));
                if mf.dmg.x1834 < 0.0 {
                    dmg = fp::fneg(reg(mf.dmg.x1834));
                    mf.x221C_b4 = false;
                }
            }
            if !mf.x221C_b4 {
                assert!(!(dmg > 500.0), "attack power over 500!! {dmg}");
                mf.dmg.x1838_percentTemp = mem(fp::fadds(reg(mf.dmg.x1838_percentTemp), dmg));
                if count > mf.dmg.x183C_applied {
                    mf.dmg.x183C_applied = count;
                }
                let a = self.attacker(v);
                let af = &self.game.fighters[a.0];
                let h0 = &self.game.fighters[v.0].x914[j];
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
                    count: h0.unk_count,
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
            let pos = self.game.fighters[v.0].x914[j].hurt_coll_pos;
            self.spawn_effect(1052, pos, None);
        }
        true
    }

    /// The hit's sound: the plain hit sound by severity when the hurtbox is invincible
    /// (`ftColl_80078384`'s check), else the hitbox's own by kind and severity.
    fn hit_sound(&mut self, v: FighterId, j: usize, me: FighterId, n: usize) {
        let h0 = &self.game.fighters[v.0].x914[j];
        let (severity, kind) = (h0.sfx_severity, h0.sfx_kind);
        self.hit_sound_of(me, n, kind, severity);
    }

    /// A hit's sound on hurtbox `n` for a hitbox's sound kind and severity.
    fn hit_sound_of(&mut self, me: FighterId, n: usize, kind: i32, severity: i32) {
        let mf = &self.game.fighters[me.0];
        if mf.x1988 != 0
            || mf.x198C != 0
            || mf.x221D_b6
            || mf.hurt_capsules[n].capsule.state != HURT_ENABLED
        {
            self.fighter_play_sfx(me, HIT_SFX[severity as usize], 0x7F, 0x40);
            return;
        }
        let slot = i32::from(mf.player_idx) * 2 + i32::from(mf.is_sub_fighter);
        if kind == 13 && severity == 2 {
            let h = self.hit_kind_sound(kind, severity, 0x72 + slot);
            self.game.fighters[me.0].x215C = h;
        } else {
            let h = self.hit_kind_sound(kind, severity, 0x7E + slot);
            self.game.fighters[me.0].x2160 = h;
        }
    }

    /// Plays a hitbox's sound by its kind and severity, `lbColl_80005BB0`: on `track` for the
    /// heaviest of kind 13, else on none.
    #[doc(alias = "lbColl_80005BB0")]
    fn hit_kind_sound(&mut self, kind: i32, severity: i32, track: i32) -> i32 {
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
    fn record_fighter_hit(&mut self, attacker: GObjId, victim_gobj: GObjId, dmg: f64) {
        self.stale_move_from_fighter(attacker, victim_gobj);
        let a = self.game.fighter_id(attacker);
        let attack_id = self.game.fighters[a.0].x2068_attackID;
        self.count_combo(attacker, victim_gobj, attack_id);
        let v = self.game.fighter_id(victim_gobj);
        let (af, vf) = (&self.game.fighters[a.0], &self.game.fighters[v.0]);
        let kind = i32::from(af.x2070[3]);
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
        let (instance, attack_id, slot) = (
            fp.x206C_attack_instance,
            fp.x2068_attackID,
            i32::from(fp.player_idx),
        );
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
        if fp.x2094.is_none() {
            fp.x208C = attack_id;
            fp.x2090 = 1;
            fp.x2094 = Some(victim_gobj);
        } else if fp.x2094 == Some(victim_gobj) {
            if attack_id != 1 && fp.x208C == attack_id {
                fp.x2090 = fp.x2090.wrapping_add(1);
                if i32::from(fp.x2090) >= limit {
                    fp.x2092 = timer as u16;
                }
            } else {
                fp.x2090 = 0;
                fp.x208C = attack_id;
            }
        }
    }

    /// The damage a hit counts in the players' statistics, `pl_8003EB30`.
    #[doc(alias = "pl_8003EB30")]
    fn count_damage(&mut self, dmg: f64, slot: i32, sub: i32, vslot: i32, vsub: i32, kind: i32) {
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
        self.game.player_mut(slot).xDB0[0x14..0x18].copy_from_slice(&[0; 4]);
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

    /// The coin battles' coin pickups, `ft_8007C77C`: nothing outside coin battles.
    #[doc(alias = "ft_8007C77C")]
    pub fn coin_pickups(&mut self, _gobj: GObjId) {
        if self.game.vs_is_coins() {
            unported_call("ft_8007C77C (coin pickups)");
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
            if ip.kind == IT_KIND_UNK4 {
                continue;
            }
            let (owner, b5, b6, team) = (
                ip.owner,
                ip.xDCD_flag.x0.b5,
                ip.xDCD_flag.x0.b6,
                ip.x20_team_id,
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
            if !self.game.fighters[me.0].x221B.x221B_b5 && self.thrower(me).is_none() {
                let item_ga = it::item(self.game, g).ground_or_air;
                for i in 0..4 {
                    let h = &self.game.fighters[me.0].x914[i];
                    let can = h.state != hitcapsule::DISABLED
                        && !h.x43_b2
                        && h.element != ELEMENT_CATCH
                        && h.x42_b5
                        && ((h.x40_b2 && item_ga == GA_AIR) || (h.x40_b3 && item_ga == GA_GROUND))
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
        let ib = &ip.x5D4_hitboxes[j];
        let h = &ib.hit;
        let mf = &self.game.fighters[me.0];
        if h.state == hitcapsule::DISABLED || !h.x42_b5 {
            return;
        }
        if !((h.x40_b2 && mf.ground_or_air == GA_AIR)
            || (h.x40_b3 && mf.ground_or_air == GA_GROUND))
        {
            return;
        }
        if h.x42_b2 && reg(mf.facing_dir) == reg(ip.facing_dir) {
            return;
        }
        if self.game.vs_x5_2() && !ib.x138 {
            return;
        }
        if hitcapsule::has_victim(h, victim(me)) {
            return;
        }
        let phantom = h.x43_b2;
        let element = h.element;
        if element != ELEMENT_INERT && !phantom && h.x42_b4 {
            let mf = &self.game.fighters[me.0];
            if mf.reflecting
                && it::item(self.game, g).x5D4_hitboxes[j].hit.x41_b7
                && self.item_box_hits_bubble(g, j, me, Bubble::Reflect, false)
            {
                unported_call("ftColl_80077464 (a fighter reflecting an item)");
            }
            let mf = &self.game.fighters[me.0];
            let h = &it::item(self.game, g).x5D4_hitboxes[j].hit;
            if mf.x2218_b6
                && (h.x42_b0 || (mf.x2218_b7 && h.x41_b7))
                && self.item_box_hits_bubble(g, j, me, Bubble::Absorb, false)
            {
                unported_call("it_8026FAC4 from ftColl_8007925C (a fighter absorbing an item)");
            }
        }
        if !self.game.fighters[me.0].x221B.x221B_b5 && !phantom && clanks != 0 {
            for m in 0..4 {
                if !self.game.ft.hit_logs.clank[m] {
                    continue;
                }
                let theirs = &self.game.fighters[me.0].x914[m];
                let h = &it::item(self.game, g).x5D4_hitboxes[j].hit;
                if element == ELEMENT_INERT || theirs.element == ELEMENT_INERT {
                    if element == theirs.element {
                        continue;
                    }
                    if self.item_box_touches(g, j, me, m) {
                        unported_call("ftColl_8007925C (an inert box touching another)");
                    }
                } else {
                    if !h.x40_b0 || !theirs.x40_b0 {
                        continue;
                    }
                    if self.item_box_touches(g, j, me, m) {
                        unported_call("ftColl_80077970 (an item's box clanking with a fighter's)");
                    }
                }
            }
        }
        let mf = &self.game.fighters[me.0];
        let h = &it::item(self.game, g).x5D4_hitboxes[j].hit;
        if element != ELEMENT_INERT && mf.x221B.x221B_b0 && h.x42_b1 {
            let ix = it::item(self.game, g).pos.x;
            let mut facing = true;
            if mf.x221B.x221B_b3 {
                if -1.0 == reg(mf.facing_dir) {
                    if reg(mf.cur_pos.x) < reg(ix) {
                        facing = false;
                    }
                } else if reg(mf.cur_pos.x) > reg(ix) {
                    facing = false;
                }
            }
            if mf.x221B.x221B_b4 && !h.x42_b4 {
                facing = false;
            }
            if facing && self.item_box_hits_bubble(g, j, me, Bubble::Shield, phantom) {
                unported_call("ftColl_80077688 (an item hitting a shield)");
            }
        }
        if element == ELEMENT_INERT {
            unported_call("lbColl_80008248 (an inert item box against hurtboxes)");
        }
        let mf = &self.game.fighters[me.0];
        if mf.x1988 == 2 || mf.x198C == 2 {
            return;
        }
        let grab_only = it::item(self.game, g).x5D4_hitboxes[j].hit.x42_b6;
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
            LoggedHit::Fighter { fighter, index } => &self.game.fighters[fighter.0].x914[index],
            LoggedHit::Item { item, index } => {
                &super::super::it::item(self.game, item).x5D4_hitboxes[index].hit
            }
        }
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
        let mut a = hitcapsule::shape(&ip.x5D4_hitboxes[j].hit);
        let mut b = hitcapsule::shape(&self.game.fighters[me.0].x914[m]);
        let (sa, sb) = (ip.scl, self.game.fighters[me.0].x34_scale.y);
        let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
        super::super::it::item_mut(self.game, g).x5D4_hitboxes[j]
            .hit
            .hurt_coll_pos = a.contact;
        self.game.fighters[me.0].x914[m].hurt_coll_pos = b.contact;
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
        let mut hit = ip.x5D4_hitboxes[j].hit.clone();
        let hit_scale = ip.scl;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.x34_scale.y, fp.cur_pos.z);
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
        let h = &mut super::super::it::item_mut(self.game, g).x5D4_hitboxes[j].hit;
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
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
        let mut hit = ip.x5D4_hitboxes[j].hit.clone();
        let hit_scale = ip.scl;
        let fp = &mut self.game.fighters[me.0];
        let (scale, z) = (fp.x34_scale.y, fp.cur_pos.z);
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
        let h = &mut super::super::it::item_mut(self.game, g).x5D4_hitboxes[j].hit;
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// An item's hitbox damage, `it_8026B1D4`: its damage, plus for an item whose speed
    /// counts (`xDC8_word` bit 0x14) the speed's share, at least 1.
    #[doc(alias = "it_8026B1D4")]
    fn item_damage(&self, g: GObjId, j: usize) -> f64 {
        let ip = super::super::it::item(self.game, g);
        let mut dmg = reg(ip.x5D4_hitboxes[j].hit.damage);
        if ip.xDC8_word.flags.x14 {
            let v = ip.x40_vel;
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
        let (id, group) = (ip.xAC4_ignoreItemID, ip.x5D4_hitboxes[j].hit.x4);
        let add = |game: &mut super::super::state::Game, item: GObjId| {
            for k in 0..4 {
                let cur = &mut it::item_mut(game, item).x5D4_hitboxes[k].hit;
                if cur.state != hitcapsule::DISABLED
                    && cur.x4 == group
                    && hitcapsule::add_victim(cur, kind, v)
                    && clear
                {
                    game.it.clank_boxes[k] = 0;
                }
            }
        };
        if id == 0 {
            add(self.game, g);
            return;
        }
        let mut cur = self.game.world.first_in(P_LINK_ITEM);
        while let Some(o) = cur {
            cur = self.game.world.gobjs[o.0].next;
            if it::item(self.game, o).xAC4_ignoreItemID == id {
                add(self.game, o);
            }
        }
    }

    /// The fighter takes item `g`'s box `j` on hurtbox `n`, `ftColl_80077C60`: the damage
    /// scaled by a grab's and the frozen state's factors and the fighter's own; a hit (not a
    /// tip) is remembered by the item, which keeps the most damage it dealt and which way;
    /// then, unless the fighter is invincible there, logged and recorded. An item taken on
    /// touch (`x0_78` 4) and a tip stop at their seams. Returns whether the box hit.
    #[doc(alias = "ftColl_80077C60")]
    fn take_item_hit(&mut self, g: GObjId, j: usize, me: FighterId, n: usize) -> bool {
        use super::super::it;
        let data = self.data;
        let ip = it::item(self.game, g);
        if it::item::attr_of(ip).x0_78(data) == 4 {
            unported_call("ftColl_80077C60 (an item a fighter takes on touch)");
        }
        let raw = self.item_damage(g, j);
        let mut f3 = raw;
        let mf = &self.game.fighters[me.0];
        if mf.victim_gobj.is_some() && !mf.x221B.x221B_b5 {
            f3 = fp::fmuls(f3, reg(self.common().f32(0x128)));
        }
        if mf.motion_id == MS_DAMAGE_ICE {
            f3 = fp::fmuls(f3, reg(self.common().f32(0x714)));
        }
        let mut dmg = fp::fmuls(f3, reg(self.game.fighters[me.0].dmg.x182c_behavior));
        let tip = self.common().f32(0x7A8);
        let h = &it::item(self.game, g).x5D4_hitboxes[j].hit;
        if reg(h.coll_distance) < reg(tip) {
            let mf = &self.game.fighters[me.0];
            if self.game.ft.hit_logs.hits.is_empty()
                && mf.dmg.x189C_unk_num_frames == 0.0
                && !hitcapsule::has_victim2(h, victim(me))
            {
                unported_call("ftColl_80077C60 (an item's tip, it_8026FC00)");
            }
            return false;
        }
        let mode = if h.x41_b5 { 5 } else { 0 };
        self.game.ft.hit_logs.tips.clear();
        self.item_add_victim(g, j, mode, victim(me), false);
        let ip = it::item(self.game, g);
        if dmg > fp::frsp(f64::from(ip.xC34_damageDealt)) {
            let slow = reg(it::item::common_data(self.game).f32(data, IT_SLOW_FOR_ITEM));
            let me_gobj = self.game.fighters[me.0].gobj;
            let mx = self.game.fighters[me.0].cur_pos.x;
            let ip = it::item_mut(self.game, g);
            ip.xC34_damageDealt = fp::fctiwz(dmg);
            ip.xCF4_fighterGObjUnk = me_gobj;
            let vx = reg(ip.x40_vel.x);
            let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
            ip.xCB8_outDamageDirection = if speed < slow {
                if reg(ip.pos.x) > reg(mx) { -1.0 } else { 1.0 }
            } else if vx < 0.0 {
                -1.0
            } else {
                1.0
            };
        }
        let mut logged = false;
        let mf = &self.game.fighters[me.0];
        if mf.x1988 == 0
            && mf.x198C == 0
            && !mf.x221D_b6
            && mf.hurt_capsules[n].capsule.state == HURT_ENABLED
        {
            let count = whole_damage(dmg);
            let mf = &mut self.game.fighters[me.0];
            if mf.x221C_b4 {
                mf.dmg.x1834 = mem(fp::fsubs(reg(mf.dmg.x1834), dmg));
                if mf.dmg.x1834 < 0.0 {
                    dmg = fp::fneg(reg(mf.dmg.x1834));
                    mf.x221C_b4 = false;
                }
            }
            if !mf.x221C_b4 {
                assert!(!(dmg > 500.0), "attack power over 500!! {dmg}");
                mf.dmg.x1838_percentTemp = mem(fp::fadds(reg(mf.dmg.x1838_percentTemp), dmg));
                if count > mf.dmg.x183C_applied {
                    mf.dmg.x183C_applied = count;
                }
                let ip = it::item(self.game, g);
                let h = &ip.x5D4_hitboxes[j].hit;
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
            let pos = it::item(self.game, g).x5D4_hitboxes[j].hit.hurt_coll_pos;
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
            ip.xD88_attackID,
            ip.xD8C_attack_instance,
            i32::from(ip.xD90[3]),
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
        let h = &super::super::it::item(self.game, g).x5D4_hitboxes[j].hit;
        let (severity, kind) = (h.sfx_severity, h.sfx_kind);
        self.hit_sound_of(me, n, kind, severity);
    }

    /// Stage devices that hurt or bury fighters, `ftColl_8007BAC0`: none of the registered
    /// devices (`ft_80459A8C`) is ported; then the stage lines the fighter touches.
    #[doc(alias = "ftColl_8007BAC0")]
    pub fn touch_devices(&mut self, gobj: GObjId) {
        if self.game.vs_x5_2() {
            return;
        }
        let me = self.game.fighter_id(gobj);
        let max = self.hit_status(me);
        if self.game.gr.devices.x6570 > 0 {
            unported_call("ftColl_8007BAC0 (stage devices, ft_80459A8C)");
        }
        if max == 0 {
            self.touch_lines(me);
        }
    }

    /// Damage from the stage lines the fighter touches, `ftCo_800C0B20`: the floor, ceiling
    /// and walls it touches, each asking the stage (`Ground_801C5700`) for a hazard; the
    /// last one asked wins. Neither ported stage has hazard lines.
    #[doc(alias = "ftCo_800C0B20")]
    fn touch_lines(&mut self, me: FighterId) {
        let fp = &self.game.fighters[me.0];
        if fp.bury_timer_1 != 0 {
            return;
        }
        let coll = &fp.coll_data;
        let flags = coll.env_flags;
        let mut lines = Vec::new();
        if flags & (0x8000 | 0x10000) != 0 {
            lines.push(coll.floor.index);
        }
        if flags & (0x2000 | 0x4000) != 0 {
            lines.push(coll.ceiling.index);
        }
        if flags & 0xFC0 != 0 {
            lines.push(coll.right_facing_wall.index);
        }
        if flags & 0x3F != 0 {
            lines.push(coll.left_facing_wall.index);
        }
        let mut hazard = false;
        for line in lines {
            hazard = self.line_hazard(line);
        }
        if hazard {
            unported_call("ftCo_800C0B20 (a stage line's damage)");
        }
    }

    /// Whether the stage gives the line a hazard (`Ground_801C5700`, through the stage's
    /// `on_touch_line`): Final Destination's and Battlefield's never do.
    #[doc(alias = "Ground_801C5700")]
    fn line_hazard(&mut self, _line: i32) -> bool {
        match self.game.stage_info.on_touch_line {
            None => false,
            // grLast_OnTouchLine, grBattle_OnTouchLine: `return NULL`.
            Some(f) if f.0 == 0x8021_C740 || f.0 == 0x8021_A610 => false,
            Some(f) => unported_call(&format!("a stage's on_touch_line {:#x}", f.0)),
        }
    }

    /// Thrown fighters' hitboxes against each other, `ft_8007C4BC`: when this fighter's
    /// thrown hitbox is live, each fighter after it in the list with one too, unless one
    /// holds the other; touching boxes push their owners apart in the air, others forget
    /// each other.
    #[doc(alias = "ft_8007C4BC")]
    pub fn thrown_hits(&mut self, gobj: GObjId) {
        let me = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[me.0];
        if !fp.x2227_b2 || self.hit_status(me) == 2 || fp.is_sub_fighter {
            return;
        }
        let holder = if fp.victim_gobj.is_some() {
            fp.victim_gobj
        } else if fp.x221C_b6 {
            fp.dmg.x1868_source
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
            if of.x2219_b1
                || !of.x2227_b2
                || self.hit_status(o) == 2
                || of.is_sub_fighter
                || (of.x221C_b6 && of.dmg.x1868_source == Some(gobj))
            {
                continue;
            }
            if of.x1064_thrownHitbox.state == hitcapsule::DISABLED || !seen {
                continue;
            }
            let mut a = hitcapsule::shape(&self.game.fighters[o.0].x1064_thrownHitbox);
            let mut b = hitcapsule::shape(&self.game.fighters[me.0].x1064_thrownHitbox);
            let (sa, sb) = (
                self.game.fighters[o.0].x34_scale.y,
                self.game.fighters[me.0].x34_scale.y,
            );
            let touch = hitboxes_touch(&mut a, &mut b, sa, sb);
            self.game.fighters[o.0].x1064_thrownHitbox.hurt_coll_pos = a.contact;
            self.game.fighters[me.0].x1064_thrownHitbox.hurt_coll_pos = b.contact;
            if touch {
                self.thrown_clash(o, me);
            } else {
                hitcapsule::remove_victim(
                    &mut self.game.fighters[me.0].x1064_thrownHitbox,
                    victim(o),
                );
                hitcapsule::remove_victim(
                    &mut self.game.fighters[o.0].x1064_thrownHitbox,
                    victim(me),
                );
            }
        }
    }

    /// Two thrown fighters' boxes meet, `ft_8007C2E0`: each remembers the other and, in the
    /// air, takes some of the other's motion, by their weights.
    #[doc(alias = "ft_8007C2E0")]
    fn thrown_clash(&mut self, f0: FighterId, f1: FighterId) {
        if hitcapsule::has_victim(&self.game.fighters[f0.0].x1064_thrownHitbox, victim(f1)) {
            return;
        }
        let mul = reg(self.common().f32(0x7D4));
        let push = |r: &mut Running, from: FighterId, to: FighterId| {
            hitcapsule::add_victim(
                &mut r.game.fighters[to.0].x1064_thrownHitbox,
                3,
                victim(from),
            );
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
            t.x98_atk_shield_kb.x = axis(same_x, df.x, dt.x, t.x98_atk_shield_kb.x);
            t.x98_atk_shield_kb.y = axis(same_y, df.y, dt.y, t.x98_atk_shield_kb.y);
        };
        push(self, f0, f1);
        push(self, f1, f0);
    }

    /// The frame's strongest hit becomes the fighter's damage result (`ftColl_8007AB48`, with
    /// `ftColl_8007A06C` on the hit log): its effects spawn as each hit is weighed.
    #[doc(alias = "ftColl_8007AB48")]
    pub fn take_hits(&mut self, gobj: GObjId) {
        let log = self.game.ft.hit_logs.hits.clone();
        self.weigh_hits(gobj, &log, false);
    }

    /// The same for the tips (`ftColl_8007AB80`), with no effects; `x18a0` keeps the result's
    /// knockback, 0 without tips.
    #[doc(alias = "ftColl_8007AB80")]
    pub fn take_tip_hits(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).dmg.x187c = 0.0;
        let log = self.game.ft.hit_logs.tips.clone();
        self.weigh_hits(gobj, &log, true);
        let fp = self.game.fighter_mut(gobj);
        fp.dmg.x18a0 = fp.dmg.x187c;
    }

    /// The knockback a fighter's hitbox deals this fighter, `ftColl_80079C70`.
    #[doc(alias = "ftColl_80079C70")]
    fn fighter_knockback(
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
    fn knockback(
        &self,
        me: FighterId,
        hit: LoggedHit,
        count: u32,
        ratio: f32,
        attack: f32,
        defense: f32,
        weight: f32,
    ) -> f64 {
        let c = self.common();
        let fp = &self.game.fighters[me.0];
        let h = self.logged_capsule(hit);
        let f = |o: u32| reg(c.f32(o));
        let w = fp::fmuls(reg(weight), f(0xF4));
        let weight_term = fp::fsubs(f(0xF8), fp::fdivs(fp::fmuls(w, f(0xF8)), fp::fadds(1.0, w)));
        let growth = if h.x28 != 0 {
            let x118 = f(0x118);
            fp::fmadds(
                x118,
                f(0x110),
                fp::fmuls(f(0x114), fp::fmuls(x118, fp::frsp(f64::from(h.x28)))),
            )
        } else {
            let damage = if fp.x2225_b7 {
                if fp.stamina_dead {
                    c.i32(0x6D8)
                } else {
                    c.i32(0x6D4)
                }
            } else {
                fp::fctiwz(reg(fp.dmg.x1830_percent))
            };
            let total = fp::fadds(fp::frsp(f64::from(damage)), reg(fp.dmg.x1838_percentTemp));
            fp::fmadds(
                f(0x110),
                total,
                fp::fmuls(f(0x114), fp::fmuls(fp::frsp(f64::from(count)), total)),
            )
        };
        let inner = fp::fmadds(f(0x11C), fp::fmuls(weight_term, growth), f(0x120));
        let kb = fp::fmadds(
            fp::fmuls(0.009_999_999_776_482_582, fp::frsp(f64::from(h.x24))),
            inner,
            fp::frsp(f64::from(h.x2C)),
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
            };
            if !tips {
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
                    && !fp.x221B.x221B_b5
                    && fp.victim_gobj == e.gobj;
                self.game.fighters[me.0].x221C_b0 = held;
                best = kb;
                best_at = Some(i);
            }
        }
        let e = log[best_at
            .expect("no logged hit with knockback above -1 (the original reads a stale index)")];
        let h = self.logged_capsule(e.hit);
        let (kb_angle, element, severity) = (h.kb_angle as u32, h.element, h.sfx_severity);
        let fp = &self.game.fighters[me.0];
        let mut dir: f32 = match e.hit {
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
                let vx = reg(ip.x40_vel.x);
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
        if kb_angle == 0x16A {
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
            d.x1870 = super::super::handles::RawPtr(dir.to_bits());
            d.x1874 = angle;
            d.x1878 = height;
            d.x187c = mem(best);
            d.x1880 = e.pos;
            d.x188c = element as i32;
            d.x1890 = severity;
            d.x1894 = e.gobj;
            d.x1898 = e.damage;
        } else {
            d.facing_dir_1 = dir;
            d.x1848_kb_angle = angle;
            d.x184c_damaged_hurtbox = height;
            d.kb_applied = mem(best);
            d.x1854_collpos = e.pos;
            d.x1860_element = element;
            d.x1864 = severity;
            d.x1868_source = e.gobj;
            // The hit result's damage: a float where the layout has an integer.
            d.x186c = e.damage.to_bits() as i32;
        }
        let grounded = height == 0;
        match e.hit {
            LoggedHit::Fighter { .. } => {
                let a = self
                    .game
                    .fighter_id(e.gobj.expect("a logged hit without its GObj"));
                let af = &self.game.fighters[a.0];
                let (a_gobj, kind, x2070, instance) = (
                    af.gobj.expect("a fighter without its GObj"),
                    af.kind,
                    u32::from_be_bytes(af.x2070),
                    af.x2074.x2088,
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
            LoggedHit::Item { item, .. } => {
                let ip = super::super::it::item(self.game, item);
                let (kind, owner, x2070, instance) = (
                    ip.kind,
                    ip.owner,
                    u32::from_be_bytes(ip.xD90),
                    ip.xDA8_short,
                );
                if kind == IT_KIND_BOMBHEI {
                    unported_call("pl_80041B08 (a Bob-omb's hit counted)");
                }
                if super::super::it::item::is_fighter(self.game, owner) {
                    self.attack_landed(
                        owner,
                        gobj,
                        2,
                        kind,
                        x2070,
                        AttackRecord::Item(item),
                        instance,
                        false,
                        grounded,
                    );
                } else {
                    unported_call("pl_8003D60C (an ownerless item's hit)");
                }
            }
        }
        if element == ELEMENT_ELECTRIC {
            let v = self.common().f32(0x1A4);
            self.game.fighters[me.0].x1960_vibrateMult = v;
        }
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
        match effect {
            1000 => self.hit_spark(me, pos, severity, mem(fp::frsp(f64::from(dmg))), kb),
            1001 | 1002 | 1004 | 1046 | 1145 | 1255 => self.spawn_effect(effect, pos, None),
            1005 => {
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
        let prev = self.game.fighters[me.0].dmg.x18c4_source_ply;
        match a {
            Some(a) => {
                let af = &self.game.fighters[a.0];
                let (spawn, player, sub) = (af.x8_spawnNum, af.player_idx, af.is_sub_fighter);
                let fp = &mut self.game.fighters[me.0];
                fp.dmg.x18C0 = if a == me { 0 } else { spawn };
                fp.dmg.x18c4_source_ply = i32::from(player);
                fp.dmg.x18C8 = -1;
                fp.x221F_b5 = sub;
            }
            None => {
                let fp = &mut self.game.fighters[me.0];
                fp.dmg.x18C0 = 0;
                fp.dmg.x18C8 = -1;
                if !keep_source {
                    fp.dmg.x18c4_source_ply = 6;
                }
            }
        }
        let fp = &mut self.game.fighters[me.0];
        fp.dmg.x18CC = kind;
        fp.dmg.x18D0 = akind;
        let _ = grounded;
        self.record_attack(attacker, gobj, x2070, record, instance, prev);
    }

    /// The lipstick flower's damage this frame, `ftColl_8007BBCC`: the logged hits of the
    /// lipstick element.
    #[doc(alias = "ftColl_8007BBCC")]
    pub fn lipstick_damage(&self) -> f64 {
        let mut dmg = 0.0;
        for e in &self.game.ft.hit_logs.hits {
            let h = self.logged_capsule(e.hit);
            if h.element == ELEMENT_LIPSTICK {
                dmg = fp::fadds(dmg, reg(h.damage));
            }
        }
        dmg
    }
}
