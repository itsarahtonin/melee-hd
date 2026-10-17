//! The Ray Gun's shot, `melee/it/kinds/itlgunray.c`: a ray like Fox's blaster shot
//! (`itRay_ItemVars`, the same variables), flying straight and stretching to its full length
//! until its lifetime runs out or it meets the stage.

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::foxlaser::{hits_stage, normalize, turned};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;
use gekko_fp as fp;

pub const ANIM: u32 = 0x8029_8300;
pub const PHYS: u32 = 0x8029_85D8;
pub const COLL: u32 = 0x8029_85F8;
pub const DMG_DEALT: u32 = 0x8029_8634;
pub const CLANKED: u32 = 0x8029_863C;
pub const HIT_SHIELD: u32 = 0x8029_8644;
pub const ABSORBED: u32 = 0x8029_864C;
pub const REFLECTED: u32 = 0x8029_8654;
pub const SHIELD_BOUNCED: u32 = 0x8029_8828;

/// `ItLGunRayAttr`: the speed, lifetime and full length.
const SPEED: u32 = 0;
const LIFETIME: u32 = 4;
const MAX_SCALE: u32 = 8;

/// Runs one of the shot's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => gone(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shot's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => physics(r, gobj),
        _ => return false,
    }
    true
}

fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a ray without attributes");
    r.data.f32(a.at(off))
}

/// Fires a ray from `owner` at `pos` the way it faces, `it_80298168`.
#[doc(alias = "it_80298168")]
pub fn spawn(r: &mut Running, owner: Option<GObjId>, pos: Vec3, facing: f32) {
    let parent = owner.expect("a ray without its shooter");
    let mut spawn = SpawnItem {
        kind: super::L_GUN_RAY,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    start(r, gobj);
    let speed = attr(r, gobj, SPEED);
    let ip = item_mut(r.game, gobj);
    let right = reg(ip.facing_dir) == fp::frsp(1.0);
    let v = ip.xDD4_itemVar.fox_laser();
    v.scale.set(0.0);
    v.angle.set(mem(if right {
        0.0
    } else {
        fp::frsp(std::f64::consts::PI)
    }));
    v.speed.set(speed);
    v.pos.set(spawn.pos);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    debug_owner_bubbles(r.game, gobj, Some(parent));
}

/// Its lifetime, not to be picked up, flying, `it_802982AC`.
#[doc(alias = "it_802982AC")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let lifetime = attr(r, gobj, LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itLgunray_UnkMotion0_Anim` (`Item_UpdateRayAnimation`): its velocity from its angle, its
/// model turned along it and stretched toward its full length (a seventh of its speed a frame),
/// and its lifetime counted down.
#[doc(alias = "itLgunray_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let max = attr(r, gobj, MAX_SCALE);
    let j = jobj_of(r.game, gobj).expect("a ray without a model");
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.fox_laser();
    let (speed, angle) = (v.speed.get(), v.angle.get());
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(angle))));
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(math::sin(angle))));
    ip.x40_vel.z = 0.0;
    ip.facing_dir = if reg(ip.x40_vel.x) > 0.0 { 1.0 } else { -1.0 };
    let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
    let y = mem(fp::frsp(fp::fmul(half_pi, reg(ip.facing_dir))));
    let vel_x = if reg(ip.facing_dir) == 1.0 {
        mem(fp::fneg(reg(ip.x40_vel.x)))
    } else {
        ip.x40_vel.x
    };
    let vel_y = ip.x40_vel.y;
    lib2725::set_rotation_y(r.game, j, y);
    let x = mem(fp::frsp(fp::fadd(
        std::f64::consts::PI,
        reg(math::atan2(vel_y, vel_x)),
    )));
    lib2725::set_rotation_x(r.game, j, x);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    let speed = reg(v.speed.get());
    let mag = if speed < fp::frsp(0.0) {
        fp::fneg(speed)
    } else {
        speed
    };
    v.scale
        .update(|old| mem(fp::fadds(reg(old), fp::fdivs(mag, 7.0))));
    if reg(v.scale.get()) > reg(max) {
        v.scale.set(max);
    }
    if reg(v.scale.get()) < f64::from(1e-5_f32) {
        v.scale.set(1e-3);
    }
    let s = v.scale.get();
    lib2725::set_scale_z(r.game, j, s);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itLgunray_UnkMotion0_Phys`: remembers where it was.
#[doc(alias = "itLgunray_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.xDD4_itemVar.fox_laser().pos.set(pos);
}

/// `itLgunray_UnkMotion0_Coll`: gone once its move crosses the stage (`it_8029C4D4`).
#[doc(alias = "itLgunray_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    hits_stage(r, gobj)
}

/// Gone on a hit, a clank, a shield or an absorb, `itLGunRay_Logic35_DmgDealt` (and
/// `Clanked`, `HitShield`, `Absorbed`).
#[doc(alias = "itLGunRay_Logic35_DmgDealt")]
#[doc(alias = "itLGunRay_Logic35_Clanked")]
#[doc(alias = "itLGunRay_Logic35_HitShield")]
#[doc(alias = "itLGunRay_Logic35_Absorbed")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Sent back by a reflection, faster by the reflector's factor (`xC70`),
/// `itLGunRay_Logic35_Reflected` (`Item_ResetRayAfterReflection`).
#[doc(alias = "itLGunRay_Logic35_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a ray without a model");
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
    let y = mem(fp::frsp(fp::fmul(half_pi, reg(ip.facing_dir))));
    let k = reg(ip.xC70);
    ip.xDD4_itemVar
        .fox_laser()
        .speed
        .update(|s| mem(fp::fmuls(reg(s), k)));
    lib2725::set_rotation_y(r.game, j, y);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    v.scale.set(1e-3);
    lib2725::set_scale_z(r.game, j, 1e-3);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    v.angle.update(|old| turned(old, std::f64::consts::PI));
    false
}

/// Bounces off a shield, `itLGunRay_Logic35_ShieldBounced` (`Item_BounceRayOffShield`).
#[doc(alias = "itLGunRay_Logic35_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let (vy, vx) = (ip.x40_vel.y, ip.x40_vel.x);
    let v = ip.xDD4_itemVar.fox_laser();
    v.scale.set(1e-3);
    v.angle.set(normalize(math::atan2(vy, vx)));
    false
}

/// Forgets a GObj the ray points to, `itLGunRay_Logic35_EvtUnk` (`it_8026B894`).
// Ports `itLGunRay_Logic35_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
