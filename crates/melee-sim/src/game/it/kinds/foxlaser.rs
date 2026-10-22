//! Fox's and Falco's blaster shots, `melee/it/kinds/itfoxlaser.c`: a ray that flies straight,
//! stretching to its full length, until its lifetime runs out or it meets the stage.

use gekko_fp as fp;

use super::super::item::{article, change_state, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x8029_C6F4;
pub const PHYS: u32 = 0x8029_C9CC;
pub const COLL: u32 = 0x8029_C9EC;
#[doc(alias = "itFoxLaser_Logic94_Clanked")]
pub const CLANKED: u32 = 0x8029_CA78;
pub const REFLECTED: u32 = 0x8029_CA80;
#[doc(alias = "itFoxLaser_Logic94_Absorbed")]
pub const ABSORBED: u32 = 0x8029_CC4C;
pub const SHIELD_BOUNCED: u32 = 0x8029_CC54;
#[doc(alias = "itFoxLaser_Logic94_HitShield")]
pub const HIT_SHIELD: u32 = 0x8029_CCF0;
pub const EVT_UNK: u32 = 0x8029_CCF8;

/// `FoxLaserAttr` offsets: the lifetime and the full length.
const LIFETIME: u32 = 0;
const SCALE: u32 = 4;

/// Runs one of the shot's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        CLANKED | ABSORBED | HIT_SHIELD => true,
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

/// Brings an angle into [0, 2π] (`Item_NormalizeAngle`), in double precision rounded to single
/// each step.
pub fn normalize(angle: f32) -> f32 {
    let tau = std::f64::consts::TAU;
    let mut a = reg(angle);
    while a < 0.0 {
        a = fp::frsp(fp::fadd(a, tau));
    }
    while a > tau {
        a = fp::frsp(fp::fsub(a, tau));
    }
    mem(a)
}

/// Fires a shot from `parent` at `pos`, `it_8029C504`: in state `msid`, flying at `angle`
/// (radians) and `speed`.
#[doc(alias = "it_8029C504")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    msid: i32,
    kind: i32,
    angle: f32,
    speed: f32,
) {
    let angle = normalize(angle);
    let pi = std::f64::consts::PI;
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    let a = reg(angle);
    let right = a < fp::fdiv(pi, 2.0) || a > fp::fdiv(fp::fmul(pi, 3.0), 2.0);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = if right { 1.0 } else { -1.0 };
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    change_state(r, gobj, msid, super::super::item::change::ANIM_UPDATE);
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a shot without attributes");
    let lifetime = r.data.f32(attrs.at(LIFETIME));
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    v.scale.set(0.0);
    v.angle.set(angle);
    v.speed.set(speed);
    v.pos.set(spawn.pos);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
}

/// Fires a shot in state 0, `it_8029C6A4`.
#[doc(alias = "it_8029C6A4")]
pub fn spawn_0(r: &mut Running, angle: f32, speed: f32, parent: GObjId, pos: Vec3, kind: i32) {
    spawn(r, parent, pos, 0, kind, angle, speed);
}

/// Fires a shot in state 1, `it_8029C6CC`.
#[doc(alias = "it_8029C6CC")]
pub fn spawn_1(r: &mut Running, angle: f32, speed: f32, parent: GObjId, pos: Vec3, kind: i32) {
    spawn(r, parent, pos, 1, kind, angle, speed);
}

/// The shot's animation callback, `itFoxlaser_UnkMotion1_Anim` (`Item_UpdateRayAnimation`):
/// its velocity from its angle, its model turned along it and stretched to its length, and its
/// lifetime counted down.
#[doc(alias = "itFoxlaser_UnkMotion1_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a shot without attributes");
    let max = r.data.f32(attrs.at(SCALE));
    let j = jobj_of(r.game, gobj).expect("a shot without a model");
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
        .update(|old| mem(fp::fadds(reg(old), fp::fdivs(mag, 11.25))));
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

/// The shot's physics callback, `itFoxlaser_UnkMotion1_Phys`: remembers where it was.
#[doc(alias = "itFoxlaser_UnkMotion1_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.xDD4_itemVar.fox_laser().pos.set(pos);
}

/// Whether the shot's move since last frame crosses the stage, stopping it there, `it_8029C4D4`
/// (`it_8026E9A4`).
#[doc(alias = "it_8029C4D4")]
pub fn hits_stage(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let from = ip.xDD4_itemVar.fox_laser().pos.get();
    let to = ip.pos;
    match super::super::ground::stage_crossing(r.game, from, to) {
        Some(p) => {
            item_mut(r.game, gobj).pos = p;
            true
        }
        None => false,
    }
}

/// The shot's collision callback, `itFoxlaser_UnkMotion1_Coll`: one more frame once it meets
/// the stage.
#[doc(alias = "itFoxlaser_UnkMotion1_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    if hits_stage(r, gobj) {
        lib2725::set_lifetime(r.game, r.data, gobj, 1.0);
        item_mut(r.game, gobj).pos = pos;
    }
    false
}

/// Turns an angle by `by` and brings it into [0, 2π].
pub fn turned(angle: f32, by: f64) -> f32 {
    let tau = std::f64::consts::TAU;
    let mut a = fp::frsp(fp::fadd(reg(angle), by));
    while a < 0.0 {
        a = fp::frsp(fp::fadd(a, tau));
    }
    while a > tau {
        a = fp::frsp(fp::fsub(a, tau));
    }
    mem(a)
}

/// The shot bounces back reflected, `itFoxLaser_Logic94_Reflected`
/// (`Item_ResetRayAfterReflection`).
#[doc(alias = "itFoxLaser_Logic94_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a shot without a model");
    let ip = item_mut(r.game, gobj);
    if reg(ip.facing_dir) != reg(ip.xC68) {
        ip.facing_dir = ip.xC68;
        let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
        let y = mem(fp::frsp(fp::fmul(half_pi, reg(ip.facing_dir))));
        lib2725::set_rotation_y(r.game, j, y);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    v.scale.set(1e-3);
    lib2725::set_scale_z(r.game, j, 1e-3);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_laser();
    v.angle.update(|old| turned(old, std::f64::consts::PI));
    false
}

/// The shot bounces off a shield, `itFoxLaser_Logic94_ShieldBounced`
/// (`Item_BounceRayOffShield`).
#[doc(alias = "itFoxLaser_Logic94_ShieldBounced")]
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

/// Forgets a GObj the shot points to, `itFoxLaser_Logic94_EvtUnk` (`it_8026B894`).
#[doc(alias = "itFoxLaser_Logic94_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
