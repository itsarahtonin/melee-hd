//! The Star Rod's stars, `melee/it/kinds/itstarrodstar.c`: a small one from a tilt or jab and
//! a big one from a smash, flying straight and spinning until their life runs out or they
//! touch the stage; a hit, clank, shield or absorber ends them.

use gekko_fp as fp;

use super::super::ground::air_touched_any;
use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{coll, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x8029_8ACC;
pub const PHYS: u32 = 0x8029_8BAC;
pub const COLL: u32 = 0x8029_8BB0;
pub const DMG_DEALT: u32 = 0x8029_8BE4;
pub const CLANKED: u32 = 0x8029_8BEC;
pub const HIT_SHIELD: u32 = 0x8029_8BF4;
pub const ABSORBED: u32 = 0x8029_8BFC;
pub const REFLECTED: u32 = 0x8029_8C04;
pub const SHIELD_BOUNCED: u32 = 0x8029_8CE8;

/// Runs one of the star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => gone(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

/// `StarRodStarAttrs` at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a star without attributes")
        .at(off)
}

/// Fires a star (`big` for a smash's) from `owner`'s `pos`, facing `facing`, `it_802988E4`.
#[doc(alias = "it_802988E4")]
pub fn fire(r: &mut Running, owner: GObjId, pos: Vec3, big: i32, facing: f32) {
    let mut spawn = SpawnItem {
        kind: super::STARROD_STAR,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .star_rod_star()
        .x0
        .set(big);
    launch(r, gobj);
    debug_owner_bubbles(r.game, gobj, Some(owner));
}

/// Sends the star off, `it_802989C8`: the big or small star's life, speed and damage, and its
/// spin.
#[doc(alias = "it_802989C8")]
pub fn launch(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let big = item_mut(r.game, gobj).xDD4_itemVar.star_rod_star().x0.get() != 0;
    let speed = reg(r.data.f32(attr(r, gobj, 0)));
    let (life, mul, damage) = if big {
        (attr(r, gobj, 0x10), attr(r, gobj, 8), attr(r, gobj, 0x18))
    } else {
        (attr(r, gobj, 0xC), attr(r, gobj, 4), attr(r, gobj, 0x14))
    };
    let (life, mul, damage) = (r.data.f32(life), r.data.f32(mul), r.data.u32(damage));
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), fp::fmuls(speed, reg(mul))));
    coll::set_hit_damage(r.game, r.data, gobj, 0, damage);
    let spin = reg(r.data.f32(attr(r, gobj, 0x1C)));
    let ip = item_mut(r.game, gobj);
    let facing = reg(ip.facing_dir);
    ip.xDD4_itemVar
        .star_rod_star()
        .x4
        .set(mem(fp::fmuls(spin, facing)));
    super::super::set_flag(&mut ip.xDCE_flag, 7, false);
}

/// `itStarrodstar_UnkMotion0_Anim`: gone once its life runs out, else it spins.
#[doc(alias = "itStarrodstar_UnkMotion0_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    let spin = ip.xDD4_itemVar.star_rod_star().x4.get();
    let j = jobj_of(r.game, gobj).expect("a star without a model");
    // `HSD_JObjAddRotationZ`.
    let z = r.game.objs.jobjs[j].rotate.z;
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, mem(fp::fadds(reg(z), reg(spin))));
    false
}

/// Moves nothing, `itStarrodstar_UnkMotion0_Phys`.
#[doc(alias = "itStarrodstar_UnkMotion0_Phys")]
pub fn phys(_: &mut Running, _: GObjId) {}

/// `itStarrodstar_UnkMotion0_Coll`: gone once it touches the stage.
#[doc(alias = "itStarrodstar_UnkMotion0_Coll")]
pub fn coll(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}

/// Gone on a hit, a clank, a shield or an absorber, `itStarRodStar_Logic36_DmgDealt` (and
/// `Clanked`, `HitShield`, `Absorbed`).
#[doc(alias = "itStarRodStar_Logic36_DmgDealt")]
#[doc(alias = "itStarRodStar_Logic36_Clanked")]
#[doc(alias = "itStarRodStar_Logic36_HitShield")]
#[doc(alias = "itStarRodStar_Logic36_Absorbed")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Points the model the way the star now flies.
fn face_velocity(r: &mut Running, gobj: GObjId) {
    let v = item(r.game, gobj).x40_vel;
    let j = jobj_of(r.game, gobj).expect("a star without a model");
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, math::atan2(v.y, v.x));
}

/// Sent back by a reflection, pointing its new way, `itStarRodStar_Logic36_Reflected`.
#[doc(alias = "itStarRodStar_Logic36_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    face_velocity(r, gobj);
    false
}

/// Bounced off a shield, pointing its new way, `itStarRodStar_Logic36_ShieldBounced`.
#[doc(alias = "itStarRodStar_Logic36_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj);
    face_velocity(r, gobj);
    false
}
