//! The Super Scope's shots, `melee/it/kinds/itsscopebeam.c`: ten sizes by charge, each flying
//! straight until its life runs out or it hits a wall or ceiling, skidding along gentle
//! slopes and glancing off steeper ones at a random angle.

use gekko_fp as fp;

use super::super::ground::{air_stay, air_touches};
use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x8029_90CC;
pub const PHYS: u32 = 0x8029_9148;
pub const COLL: u32 = 0x8029_914C;
pub const DMG_DEALT: u32 = 0x8029_9360;
pub const CLANKED: u32 = 0x8029_9368;
pub const HIT_SHIELD: u32 = 0x8029_9370;
pub const ABSORBED: u32 = 0x8029_9378;
pub const SHIELD_BOUNCED: u32 = 0x8029_9380;
pub const REFLECTED: u32 = 0x8029_93A0;

/// `ScopeBeamAttrs`: per charge, a speed, scale and life, 12 bytes each; the steepest slope a
/// shot glances off (`x78`, radians) and the speed it keeps (`x7C`).
const X78: u32 = 0x78;
const X7C: u32 = 0x7C;

/// Runs one of the shot's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => gone(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shot's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

/// The charge's speed, scale and life.
fn floats(r: &Running, gobj: GObjId, charge: i32) -> (f32, f32, f32) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Super Scope shot without attributes")
        .at(12 * charge as u32);
    (r.data.f32(a), r.data.f32(a.at(4)), r.data.f32(a.at(8)))
}

fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Super Scope shot without attributes");
    r.data.f32(a.at(off))
}

/// Fires a shot of charge `charge` from `owner`'s `pos`, facing `facing`, `it_80298DEC`.
#[doc(alias = "it_80298DEC")]
pub fn fire(r: &mut Running, owner: GObjId, pos: Vec3, charge: i32, facing: f32) {
    let mut spawn = SpawnItem {
        kind: super::S_SCOPE_BEAM,
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
        .scope_beam()
        .x0
        .set(charge);
    launch(r, gobj, owner);
    debug_owner_bubbles(r.game, gobj, Some(owner));
}

/// Sends the shot off, `it_80298ED0`: its charge's state, speed, scale and life.
#[doc(alias = "it_80298ED0")]
pub fn launch(r: &mut Running, gobj: GObjId, owner: GObjId) {
    item_mut(r.game, gobj).owner = Some(owner);
    let charge = item_mut(r.game, gobj).xDD4_itemVar.scope_beam().x0.get();
    let (speed, scale, life) = floats(r, gobj, charge);
    lib26b1::disable_pickup(r.game, gobj);
    if (0..=9).contains(&charge) {
        change_state(r, gobj, charge, change::ANIM_UPDATE);
    }
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    let j = jobj_of(r.game, gobj).expect("a shot without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(scale, scale, scale));
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// `itSscopebeam_UnkMotion9_Anim`: on its first tick its hitbox grows to its scale; gone once
/// its life runs out.
#[doc(alias = "itSscopebeam_UnkMotion9_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let charge = item_mut(r.game, gobj).xDD4_itemVar.scope_beam().x0.get();
    let (_, scale, life) = floats(r, gobj, charge);
    let ip = item_mut(r.game, gobj);
    if ip.xD44_lifeTimer == life {
        let hit = &mut ip.x5D4_hitboxes[0].hit;
        if hit.state >= hitcapsule::ENABLED {
            hit.scale = mem(fp::fmuls(reg(hit.scale), reg(scale)));
        }
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        return true;
    }
    false
}

/// Moves nothing, `itSscopebeam_UnkMotion9_Phys`.
#[doc(alias = "itSscopebeam_UnkMotion9_Phys")]
pub fn phys(_: &mut Running, _: GObjId) {}

/// `fabsf` as the branches compile it.
fn abs(v: f64) -> f64 {
    if v >= 0.0 { v } else { fp::fneg(v) }
}

/// `itSscopebeam_UnkMotion9_Coll`: gone on a wall or ceiling; on a floor within a degree of
/// flat it flies on as if it hadn't touched it; on a slope up to `x78` it glances off at twice
/// the slope plus a random part of `x78`, keeping `x7C` of its speed (or skids on if that
/// sends it down); steeper, it's gone.
#[doc(alias = "itSscopebeam_UnkMotion9_Coll")]
pub fn coll(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item(r.game, gobj);
    let (saved_pos, saved_coll, saved_vx) = (ip.pos, ip.x378_itemColl.clone(), ip.x40_vel.x);
    let t = air_touches(r, gobj);
    if t & 0xE != 0 {
        return true;
    }
    let ip = item(r.game, gobj);
    if t & 1 == 0 || ip.x378_itemColl.env_flags & 0x18000 == 0 {
        return false;
    }
    let n = ip.x378_itemColl.floor.normal;
    let angle = fp::fneg(reg(math::atan2(n.x, n.y)));
    let one_degree = f64::from(0.017_453_292_f32);
    if abs(angle) <= one_degree {
        let ip = item_mut(r.game, gobj);
        ip.pos = saved_pos;
        ip.x378_itemColl = saved_coll;
        air_stay(r, gobj);
        return false;
    }
    let limit = attr(r, gobj, X78);
    if abs(angle) > reg(limit) {
        return true;
    }
    let keep = attr(r, gobj, X7C);
    let ip = item(r.game, gobj);
    let mut vel = Vec3::new(mem(fp::fmuls(reg(ip.x40_vel.x), reg(keep))), 0.0, 0.0);
    let facing = ip.facing_dir;
    let rand = r.game.objs.rng.draw_f32();
    let spread = fp::fmuls(reg(facing), fp::fmuls(reg(limit), reg(rand)));
    let turn = mem(fp::fmadds(2.0, angle, spread));
    vel.rotate_about(Vec3::new(0.0, 0.0, 1.0), turn);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = vel.x;
    ip.x40_vel.y = vel.y;
    if reg(ip.x40_vel.y) <= 0.0 {
        ip.x40_vel.x = saved_vx;
        ip.x40_vel.y = 0.0;
    }
    false
}

/// Gone on a hit, a clank, a shield or an absorption, `itSScopeBeam_Logic38_DmgDealt` (and
/// `Clanked`, `HitShield`, `Absorbed`).
#[doc(alias = "itSScopeBeam_Logic38_DmgDealt")]
#[doc(alias = "itSScopeBeam_Logic38_Clanked")]
#[doc(alias = "itSScopeBeam_Logic38_HitShield")]
#[doc(alias = "itSScopeBeam_Logic38_Absorbed")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Bounces off a shield, `itSScopeBeam_Logic38_ShieldBounced`.
#[doc(alias = "itSScopeBeam_Logic38_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Sent back by a reflection, `itSScopeBeam_Logic38_Reflected`.
#[doc(alias = "itSScopeBeam_Logic38_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}
