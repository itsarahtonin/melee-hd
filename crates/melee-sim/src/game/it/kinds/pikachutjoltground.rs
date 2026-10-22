//! Pikachu's and Pichu's Thunder Jolt, `melee/it/kinds/itpikachutjoltground.c`: thrown down
//! at an angle, it runs along the first surface it meets as a linked jolt
//! ([`pikachutjoltair`](super::pikachutjoltair)) that hugs the stage, while this one follows
//! it and carries the effect.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use super::pikachutjoltair;
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, spawn as ef_spawn};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const ANIM0: u32 = 0x802B_3680;
pub const ANIM1: u32 = 0x802B_36BC;
pub const PHYS0: u32 = 0x802B_3790;
pub const PHYS1: u32 = 0x802B_37F0;
pub const COLL0: u32 = 0x802B_3808;
pub const COLL1: u32 = 0x802B_397C;
pub const DESTROYED: u32 = 0x802B_34E4;
pub const DMG_DEALT: u32 = 0x802B_3A94;
pub const REFLECTED: u32 = 0x802B_3AF8;
pub const CLANKED: u32 = 0x802B_3C44;
pub const ABSORBED: u32 = 0x802B_3CA8;
pub const HIT_SHIELD: u32 = 0x802B_3D0C;
pub const SHIELD_BOUNCED: u32 = 0x802B_3D70;
pub const EVT_UNK: u32 = 0x802B_3EDC;

/// `itPikachutJoltGroundAttributes`: the lifetime, the throw's angle, the speed, and the
/// turn between surfaces past which the running jolt ends.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const ANGLE: u32 = 0x4;
    pub const SPEED: u32 = 0x8;
    pub const TURN_LIMIT: u32 = 0xC;
}

/// Runs one of the jolt's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => end(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the jolt's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a jolt without attributes");
    r.data.f32(attrs.at(off))
}

/// Brings an angle into [0, 2π], rounding to single precision each step.
fn normalize(mut a: f64) -> f32 {
    let tau = std::f64::consts::TAU;
    while a < 0.0 {
        a = fp::frsp(fp::fadd(a, tau));
    }
    while a > tau {
        a = fp::frsp(fp::fsub(a, tau));
    }
    mem(a)
}

/// The model turned to the facing.
fn face(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a jolt without a model");
    let facing = reg(item(r.game, gobj).facing_dir);
    let y = fp::frsp(fp::fmul(fp::fdiv(std::f64::consts::PI, 2.0), facing));
    lib2725::set_rotation_y(r.game, j, mem(y));
}

/// The running jolt this one follows (`it_802B3368`).
#[doc(alias = "it_802B3368")]
pub fn linked(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDDC
        .get()
}

/// Forgets the running jolt (`it_802B3544`).
#[doc(alias = "it_802B3544")]
pub fn unlink(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDDC
        .set(None);
}

/// Throws a jolt for `owner` from `pos`, `itPikachuThunderJolt_Spawn`: started at the owner's
/// middle when the way from there to `pos` crosses the stage.
#[doc(alias = "itPikachuThunderJolt_Spawn")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    facing_dir: f32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, owner);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    if ground::stage_crossing_normal(r.game, spawn.pos, spawn.prev_pos).is_some() {
        item_mut(r.game, gobj).pos = spawn.pos;
    }
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_ground();
    v.xDDC.set(None);
    v.xDE4.set(0);
    v.xDE8.set(spawn.pos);
    v.xDF4.set(Vec3::new(0.0, 0.0, 0.0));
    setup(r, gobj, owner);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDD8
        .set(Some(owner));
    Some(gobj)
}

/// The jolt's start, `it_802B3554`: its first state, its heading (`attr::ANGLE`, mirrored
/// when facing left) and its effect.
#[doc(alias = "it_802B3554")]
fn setup(r: &mut Running, gobj: GObjId, owner: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let a = reg(attr_f32(r, gobj, attr::ANGLE));
    let angle = if reg(item(r.game, gobj).facing_dir) == 1.0 {
        a
    } else {
        fp::fadd(std::f64::consts::PI, if a < 0.0 { fp::fneg(a) } else { a })
    };
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDD4
        .set(mem(angle));
    let id = match item(r.game, gobj).kind {
        89 | 91 => Some(0x4BD),
        147 | 149 => Some(0x4AD),
        _ => None,
    };
    if let Some(id) = id {
        let j = jobj_of(r.game, gobj);
        ef_spawn(r, id, Some(gobj), &[Arg::JObj(j)]);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .pikachu_jolt_ground()
            .xDE0
            .set(1);
    }
    debug_owner_bubbles(r.game, gobj, Some(owner));
}

/// `itPikachutjoltground_UnkMotion0_Anim`: remembers where it was; its lifetime.
#[doc(alias = "itPikachutjoltground_UnkMotion0_Anim")]
fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.xDD4_itemVar.pikachu_jolt_ground().xDE8.set(pos);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itPikachutjoltground_UnkMotion1_Anim`: ends when its running jolt is gone or follows
/// another; else its lifetime, and it moves with the running jolt.
#[doc(alias = "itPikachutjoltground_UnkMotion1_Anim")]
fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let Some(air) = linked(r, gobj) else {
        return true;
    };
    if pikachutjoltair::linked(r, air) != Some(gobj) {
        return true;
    }
    if anim0(r, gobj) {
        return true;
    }
    let pos = pikachutjoltair::tip(r, air);
    item_mut(r.game, gobj).pos = pos;
    false
}

/// `itPikachutjoltground_UnkMotion0_Phys`: flying along its heading at `attr::SPEED`.
#[doc(alias = "itPikachutjoltground_UnkMotion0_Phys")]
fn phys0(r: &mut Running, gobj: GObjId) {
    let speed = reg(attr_f32(r, gobj, attr::SPEED));
    let ip = item_mut(r.game, gobj);
    let a = ip.xDD4_itemVar.pikachu_jolt_ground().xDD4.get();
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(math::cos(a))));
    let ip = item_mut(r.game, gobj);
    let a = ip.xDD4_itemVar.pikachu_jolt_ground().xDD4.get();
    ip.x40_vel.y = mem(fp::fmuls(speed, reg(math::sin(a))));
}

/// `itPikachutjoltground_UnkMotion1_Phys`: still (the running jolt moves it).
#[doc(alias = "itPikachutjoltground_UnkMotion1_Phys")]
fn phys1(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
}

/// Whether the move since last frame crosses the stage, stopping it there and keeping the
/// surface's normal (`it_8026E9A4` from `xDE8`).
fn crosses(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let from = ip.xDD4_itemVar.pikachu_jolt_ground().xDE8.get();
    let to = ip.pos;
    match ground::stage_crossing_normal(r.game, from, to) {
        Some((p, n)) => {
            let ip = item_mut(r.game, gobj);
            ip.pos = p;
            ip.xDD4_itemVar.pikachu_jolt_ground().xDF4.set(n);
            true
        }
        None => false,
    }
}

/// Keeps the surface's normal as the last (`xE00 = xDF4`).
fn keep_normal(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_ground();
    let n = v.xDF4.get();
    v.xE00.set(n);
}

/// Hands the running jolt where this one is and the surface it runs along.
fn place_linked(r: &mut Running, gobj: GObjId, air: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (pos, coll) = (ip.pos, ip.x378_itemColl.clone());
    let n = ip.xDD4_itemVar.pikachu_jolt_ground().xDF4.get();
    pikachutjoltair::place(r, air, pos, coll, n);
}

/// `itPikachutjoltground_UnkMotion0_Coll`: meeting the stage, its effect goes, the running
/// state, and a running jolt along the surface, facing the way the jolt turns onto it.
#[doc(alias = "itPikachutjoltground_UnkMotion0_Coll")]
fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    keep_normal(r, gobj);
    if !crosses(r, gobj) {
        return false;
    }
    crate::game::ef::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDE0
        .set(0);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    let n = ip.xDD4_itemVar.pikachu_jolt_ground().xDF4.get();
    let ground_angle = math::atan2(n.y, n.x);
    let v = ip.x40_vel;
    let diff = fp::fsubs(reg(math::atan2(v.y, v.x)), reg(ground_angle));
    let diff = normalize(diff);
    let facing = if reg(diff) < std::f64::consts::PI {
        -1.0
    } else {
        1.0
    };
    let ip = item(r.game, gobj);
    let (owner, pos, kind) = (ip.owner, ip.pos, ip.kind);
    let air = pikachutjoltair::spawn(r, owner, gobj, pos, kind + 1, facing);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDDC
        .set(air);
    keep_normal(r, gobj);
    let Some(air) = air else {
        return true;
    };
    place_linked(r, gobj, air);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDE4
        .set(0);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    false
}

/// `itPikachutjoltground_UnkMotion1_Coll`: meeting the stage again, it ends if the new
/// surface turns more than `attr::TURN_LIMIT` from the last, else the running jolt follows
/// it.
#[doc(alias = "itPikachutjoltground_UnkMotion1_Coll")]
fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDE4
        .get()
        >= 0
    {
        keep_normal(r, gobj);
        if crosses(r, gobj) {
            let limit = reg(attr_f32(r, gobj, attr::TURN_LIMIT));
            let ends = if 0.0 == limit {
                false
            } else {
                let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_ground();
                let angle = reg(v.xDF4.get().angle(v.xE00.get()));
                angle > if limit < 0.0 { fp::fneg(limit) } else { limit }
            };
            if ends {
                return true;
            }
            let Some(air) = linked(r, gobj) else {
                return true;
            };
            place_linked(r, gobj, air);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .pikachu_jolt_ground()
                .xDE4
                .set(0);
        }
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_ground();
    v.xDE4.update(|n| n.wrapping_add(1));
    false
}

/// The jolt lets go of its running jolt and its effect goes.
fn release(r: &mut Running, gobj: GObjId) {
    if let Some(air) = linked(r, gobj) {
        pikachutjoltair::unlink(r, air);
        unlink(r, gobj);
    }
    crate::game::ef::destroy_all(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_ground()
        .xDE0
        .set(0);
}

/// `it_2725_Logic106_Destroyed`.
#[doc(alias = "it_2725_Logic106_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    release(r, gobj);
}

/// `it_2725_Logic106_DmgDealt` (and `_Clanked`, `_Absorbed`, `_HitShield`): it ends.
#[doc(alias = "it_2725_Logic106_DmgDealt")]
#[doc(alias = "it_2725_Logic106_Clanked")]
#[doc(alias = "it_2725_Logic106_Absorbed")]
#[doc(alias = "it_2725_Logic106_HitShield")]
fn end(r: &mut Running, gobj: GObjId) -> bool {
    release(r, gobj);
    true
}

/// `it_2725_Logic106_Reflected`: turned around.
#[doc(alias = "it_2725_Logic106_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    face(r, gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_ground();
    let a = fp::frsp(fp::fadd(reg(v.xDD4.get()), std::f64::consts::PI));
    v.xDD4.set(normalize(a));
    false
}

/// `it_2725_Logic106_ShieldBounced`: mirrored off the shield.
#[doc(alias = "it_2725_Logic106_ShieldBounced")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let (vx, vy) = (ip.x40_vel.x, ip.x40_vel.y);
    let a = reg(math::atan2(vy, vx));
    ip.xDD4_itemVar.pikachu_jolt_ground().xDD4.set(normalize(a));
    ip.facing_dir = if reg(vx) >= 0.0 { 1.0 } else { -1.0 };
    face(r, gobj);
    false
}

/// Forgets a GObj the jolt points to, `itPikachuTJoltGround_Logic106_EvtUnk` (`it_8026B894`).
// Ports `itPikachuTJoltGround_Logic106_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
