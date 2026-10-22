//! The running half of Pikachu's and Pichu's Thunder Jolt, `melee/it/kinds/itpikachutjoltair.c`:
//! spawned where the thrown jolt ([`pikachutjoltground`](super::pikachutjoltground)) meets
//! the stage, it runs along the surface (its animation does the moving), with the thrown
//! jolt following its tip.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state, debug_owner_bubbles, play_sfx1, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::pikachutjoltground;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::{CollData, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x802B_4450;
#[doc(alias = "itPikachutjoltair_UnkMotion0_Phys")]
pub const PHYS: u32 = 0x802B_45D0;
#[doc(alias = "itPikachutjoltair_UnkMotion0_Coll")]
pub const COLL: u32 = 0x802B_45E8;
pub const DMG_DEALT: u32 = 0x802B_45F0;
pub const CLANKED: u32 = 0x802B_4638;
pub const ABSORBED: u32 = 0x802B_4680;
pub const REFLECTED: u32 = 0x802B_46C8;
pub const HIT_SHIELD: u32 = 0x802B_4A18;
pub const SHIELD_BOUNCED: u32 = 0x802B_4A60;
pub const EVT_UNK: u32 = 0x802B_4AA8;

/// The frames the running jolt lasts past its last surface.
const MAX_FRAMES: i32 = 24;

/// The bones of the model: the tip the thrown jolt follows, and the one the reflection turns
/// by.
const TIP_BONE: usize = 6;
const TURN_BONE: usize = 4;

/// Runs one of the running jolt's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => false,
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD | SHIELD_BOUNCED => end(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the running jolt's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => {
            // `itPikachutjoltair_UnkMotion0_Phys`: still (its animation moves it).
            item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
        }
        _ => return false,
    }
    true
}

/// A bone of the model.
fn bone(r: &Running, gobj: GObjId, i: usize) -> Option<JObjId> {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a jolt without bones");
    r.game.it.bone_tables[t.0].bones[i]
}

/// The model turned to the facing.
fn face(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a jolt without a model");
    let facing = reg(item(r.game, gobj).facing_dir);
    let y = fp::frsp(fp::fmul(fp::fdiv(std::f64::consts::PI, 2.0), facing));
    lib2725::set_rotation_y(r.game, j, mem(y));
}

/// The model tilted along the surface it runs on (`xDE8`), mirrored when facing left.
fn tilt(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a jolt without a model");
    let ip = item_mut(r.game, gobj);
    let n = ip.xDD4_itemVar.pikachu_jolt_air().xDE8.get();
    let angle = math::atan2(n.x, n.y);
    let x = if reg(ip.facing_dir) == 1.0 {
        angle
    } else {
        mem(fp::fneg(reg(angle)))
    };
    lib2725::set_rotation_x(r.game, j, x);
}

/// The thrown jolt this one runs for (`it_802B3EFC`).
#[doc(alias = "it_802B3EFC")]
pub fn linked(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_air()
        .xDD8
        .get()
}

/// Forgets the thrown jolt (`it_802B43B0`).
#[doc(alias = "it_802B43B0")]
pub fn unlink(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_air()
        .xDD8
        .set(None);
}

/// Where the thrown jolt goes: the tip's place while linked, else the origin
/// (`it_802B3F20`).
#[doc(alias = "it_802B3F20")]
pub fn tip(r: &mut Running, gobj: GObjId) -> Vec3 {
    if linked(r, gobj).is_none() {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    let j = bone(r, gobj, TIP_BONE);
    let mut p = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    p.z = 0.0;
    p
}

/// Puts the running jolt on a surface, `it_802B3F88`: at `pos` with the thrown jolt's
/// collision data, along the surface's normal `n`, its frames restarted, and its sound.
#[doc(alias = "it_802B3F88")]
pub fn place(r: &mut Running, gobj: GObjId, pos: Vec3, coll: CollData, n: Vec3) {
    let ip = item_mut(r.game, gobj);
    ip.pos = pos;
    ip.x378_itemColl = coll;
    let v = ip.xDD4_itemVar.pikachu_jolt_air();
    v.xDE8.set(n);
    v.xDF4.set(0);
    let j = jobj_of(r.game, gobj).expect("a jolt without a model");
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    tilt(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE | change::HIT_PRESERVE);
    super::super::item::animate(r, gobj);
    match item(r.game, gobj).kind {
        148 | 90 => play_sfx1(r, gobj, 0x3A9CF, 127, 64),
        150 | 92 => play_sfx1(r, gobj, 0x382B6, 127, 64),
        _ => {}
    }
}

/// Spawns the running jolt for `ground` at `pos`, `it_802B4224`.
#[doc(alias = "it_802B4224")]
pub fn spawn(
    r: &mut Running,
    owner: Option<GObjId>,
    ground: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = owner;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let v = ip.xDD4_itemVar.pikachu_jolt_air();
    v.xDD8.set(Some(ground));
    v.xDF4.set(0);
    face(r, gobj);
    setup(r, gobj, owner);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pikachu_jolt_air()
        .xDD4
        .set(owner);
    Some(gobj)
}

/// The running jolt's start, `it_802B43D0`.
#[doc(alias = "it_802B43D0")]
fn setup(r: &mut Running, gobj: GObjId, owner: Option<GObjId>) {
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    super::super::item::animate(r, gobj);
    debug_owner_bubbles(r.game, gobj, owner);
}

/// The running jolt lets go of the thrown jolt.
fn release(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    if let Some(ground) = linked(r, gobj) {
        pikachutjoltground::unlink(r, ground);
        unlink(r, gobj);
    }
}

/// `itPikachutjoltair_UnkMotion0_Anim`: tilted along its surface; it ends past
/// [`MAX_FRAMES`] or once the thrown jolt no longer follows it.
#[doc(alias = "itPikachutjoltair_UnkMotion0_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    tilt(r, gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_air();
    v.xDF4.update(|n| n.wrapping_add(1));
    let frames = v.xDF4.get();
    let followed = match linked(r, gobj) {
        Some(ground) => pikachutjoltground::linked(r, ground) == Some(gobj),
        None => false,
    };
    if !(frames <= MAX_FRAMES && followed) {
        release(r, gobj);
        return true;
    }
    false
}

/// `it_2725_Logic107_DmgDealt` (and `_Clanked`, `_Absorbed`, `_HitShield`,
/// `_ShieldBounced`): it ends.
#[doc(alias = "it_2725_Logic107_DmgDealt")]
#[doc(alias = "it_2725_Logic107_Clanked")]
// Ports `it_2725_Logic107_Absorbed`; aliased once a check reaches it.
#[doc(alias = "it_2725_Logic107_HitShield")]
#[doc(alias = "it_2725_Logic107_ShieldBounced")]
fn end(r: &mut Running, gobj: GObjId) -> bool {
    release(r, gobj);
    true
}

/// `it_2725_Logic107_Reflected`: turned around at its tip, heading back along the way it
/// came.
#[doc(alias = "it_2725_Logic107_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a jolt without a model");
    let t = bone(r, gobj, TIP_BONE);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, t, None);
    let ip = item_mut(r.game, gobj);
    ip.pos = pos;
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let facing = reg(ip.facing_dir);
    let rot = reg(r.game.objs.jobjs[j].rotate.x);
    let rot = if facing == 1.0 { rot } else { fp::fneg(rot) };
    let turn = bone(r, gobj, TURN_BONE).expect("a jolt without its bone");
    let by = reg(r.game.objs.jobjs[turn].rotate.x);
    let mut angle = fp::frsp(fp::fadd(rot, fp::fsub(std::f64::consts::PI, by)));
    let tau = std::f64::consts::TAU;
    while angle < 0.0 {
        angle = fp::frsp(fp::fadd(angle, tau));
    }
    while angle > tau {
        angle = fp::frsp(fp::fsub(angle, tau));
    }
    lib2725::set_rotation_x(r.game, j, mem(angle));
    face(r, gobj);
    let pos = item(r.game, gobj).pos;
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    let a = if facing == 1.0 {
        mem(angle)
    } else {
        mem(fp::fneg(angle))
    };
    let n = Vec3::new(math::sin(a), math::cos(a), 0.0);
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_jolt_air();
    v.xDE8.set(n);
    v.xDF4.set(0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    false
}

/// Forgets a GObj the jolt points to, `itPikachuTJoltAir_Logic107_EvtUnk` (`it_8026B894`).
// Ports `itPikachuTJoltAir_Logic107_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
