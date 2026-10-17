//! The Fire Flower's flame, `melee/it/kinds/itlgunbeam.c` (`It_Kind_L_Gun_Beam`, a name from
//! the decomp's guesses): a puff flying at a random speed and angle in its attributes' ranges,
//! turning along the surfaces it meets, until its lifetime runs out.

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use super::koopaflame::{clamp_angle, clamp_angle_reverse};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg, eflib};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::mp::coll::EcbShape;
use crate::vec::Vec3;
use gekko_fp as fp;

pub const ANIM: u32 = 0x8029_999C;
pub const PHYS: u32 = 0x8029_99E4;
pub const COLL: u32 = 0x8029_9A68;
pub const DMG_DEALT: u32 = 0x8029_9B6C;
pub const REFLECTED: u32 = 0x8029_9B74;
pub const CLANKED: u32 = 0x8029_9C08;
pub const ABSORBED: u32 = 0x8029_9C10;
pub const SHIELD_BOUNCED: u32 = 0x8029_9C18;
pub const HIT_SHIELD: u32 = 0x8029_9C20;

/// `ItLGunBeamAttr`: the lifetime, the speed's range and the angle's.
const LIFETIME: u32 = 0x0;
const MIN_SPEED: u32 = 0x4;
const MAX_SPEED: u32 = 0x8;
const MIN_ANGLE: u32 = 0xC;
const MAX_ANGLE: u32 = 0x10;

/// The surfaces it met.
const FLOOR: i32 = 1;
const CEILING: i32 = 2;
const LEFT_WALL: i32 = 4;
const RIGHT_WALL: i32 = 8;

const PI: f64 = std::f64::consts::PI;
const TAU: f64 = std::f64::consts::TAU;

/// The flame's look: one of four effects from this one.
const GFX_FLAME: i32 = 0x44E;

/// Runs one of the flame's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT | ABSORBED => goes_on(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED | SHIELD_BOUNCED | HIT_SHIELD => gone(r, gobj),
        _ => return None,
    })
}

/// Runs one of the flame's events, if `addr` is one.
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
        .expect("a flame without attributes");
    r.data.f32(a.at(off))
}

/// The direction of the surfaces it met (`flags`), their normals summed, `it_802993E0`.
#[doc(alias = "it_802993E0")]
pub fn surface_direction(r: &mut Running, gobj: GObjId, flags: i32) {
    let ip = item_mut(r.game, gobj);
    let cd = &ip.x378_itemColl;
    let surfaces = [
        (FLOOR, cd.floor.normal),
        (CEILING, cd.ceiling.normal),
        (LEFT_WALL, cd.left_facing_wall.normal),
        (RIGHT_WALL, cd.right_facing_wall.normal),
    ];
    let v = ip.xDD4_itemVar.lgunbeam();
    v.angle0.update(clamp_angle);
    let mut d = Vec3::new(0.0, 0.0, 0.0);
    for (bit, n) in surfaces {
        if flags & bit != 0 {
            d.x = mem(fp::fadds(reg(d.x), reg(n.x)));
            d.y = mem(fp::fadds(reg(d.y), reg(n.y)));
        }
    }
    d.normalize();
    v.position1.set(d);
}

/// Its angle turned toward the surfaces' direction, a little when they differ by less than a
/// right angle and more when by more, `it_80299528`.
#[doc(alias = "it_80299528")]
pub fn turn(r: &mut Running, gobj: GObjId, flags: i32) {
    if flags == 0 {
        return;
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.lgunbeam();
    let (dir, vel) = (v.position1.get(), v.velocity.get());
    let a = math::atan2(dir.x, dir.y);
    let diff = clamp_angle_reverse(mem(fp::fsubs(reg(math::atan2(vel.x, vel.y)), reg(a))));
    let by = if diff == 0.0 {
        0.0
    } else {
        let angle = reg(dir.angle(vel));
        let d = reg(diff);
        let abs = if d < fp::frsp(0.0) { fp::fneg(d) } else { d };
        let k = if abs < fp::fdiv(PI, 2.0) {
            fp::frsp(fp::fmul(f64::from(0.02_f32), fp::fdiv(abs, PI)))
        } else {
            fp::frsp(fp::fmul(0.5, fp::fdiv(abs, PI)))
        };
        let signed = if d < 0.0 { fp::fneg(k) } else { k };
        fp::fmuls(angle, signed)
    };
    v.angle0
        .update(|old| clamp_angle(mem(fp::fsubs(reg(old), by))));
}

/// A flame from `owner` at `pos` facing `dir`, with the flower's attack id `ignore`,
/// `it_802996D0`.
#[doc(alias = "it_802996D0")]
pub fn spawn(
    r: &mut Running,
    owner: Option<GObjId>,
    pos: Vec3,
    ignore: u32,
    dir: f32,
) -> Option<GObjId> {
    let parent = owner.expect("a flame without its shooter");
    let mut spawn = SpawnItem {
        kind: super::L_GUN_BEAM,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = ignore as i32;
    let gobj = spawn_air(r, &mut spawn)?;
    let lifetime = attr(r, gobj, LIFETIME);
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDD4_itemVar.lgunbeam().lifetime.set(lifetime);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .lgunbeam()
        .position0
        .set(pos);
    let (lo, hi) = (reg(attr(r, gobj, MIN_SPEED)), reg(attr(r, gobj, MAX_SPEED)));
    let rng = reg(r.game.objs.rng.draw_f32());
    let speed = mem(fp::fmadds(fp::fsubs(hi, lo), rng, lo));
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .lgunbeam()
        .angle1
        .set(speed);
    let (lo, hi) = (reg(attr(r, gobj, MIN_ANGLE)), reg(attr(r, gobj, MAX_ANGLE)));
    let rng = reg(r.game.objs.rng.draw_f32());
    let angle = mem(fp::fmadds(fp::fsubs(hi, lo), rng, lo));
    let ip = item_mut(r.game, gobj);
    let right = reg(ip.facing_dir) == fp::frsp(1.0);
    let v = ip.xDD4_itemVar.lgunbeam();
    v.angle0.set(if right {
        angle
    } else {
        mem(fp::fneg(reg(angle)))
    });
    v.angle0.update(|a| {
        let mut a = reg(a);
        while a < fp::fneg(PI) {
            a = fp::frsp(fp::fadd(a, TAU));
        }
        while a > PI {
            a = fp::frsp(fp::fsub(a, TAU));
        }
        mem(a)
    });
    v.xE04.set(0);
    ip.xDCC_flag.b3 = false;
    let look = r.game.objs.rng.below(4);
    setup(r, gobj, parent, look);
    Some(gobj)
}

/// The flame's start, `it_802998A0`: its state, animated once, its velocity, a collision pass
/// from a frame back (keeping where it is), and its effect (`look` of four).
#[doc(alias = "it_802998A0")]
pub fn setup(r: &mut Running, gobj: GObjId, parent: GObjId, look: i32) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let at = item(r.game, gobj).pos;
    super::super::item::animate(r, gobj);
    physics(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fsubs(reg(ip.pos.x), reg(ip.x40_vel.x)));
    ip.pos.y = mem(fp::fsubs(reg(ip.pos.y), reg(ip.x40_vel.y)));
    ground::air_stay(r, gobj);
    item_mut(r.game, gobj).pos = at;
    let j = jobj_of(r.game, gobj);
    let facing = item(r.game, gobj).facing_dir;
    ef::spawn(
        r,
        look.wrapping_add(GFX_FLAME),
        Some(gobj),
        &[Arg::JObj(j), Arg::Float(facing), Arg::Float(1.0)],
    );
    debug_owner_bubbles(r.game, gobj, Some(parent));
}

/// `itLgunbeam_UnkMotion0_Anim`: gone, its effects with it, when its life runs out.
#[doc(alias = "itLgunbeam_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    if lib2725::tick_lifetime(r.game, gobj) {
        eflib::destroy_all(r, gobj);
        return true;
    }
    false
}

/// `itLgunbeam_UnkMotion0_Phys`: where it was, and its velocity from its speed and angle.
#[doc(alias = "itLgunbeam_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.lgunbeam();
    v.position0.set(pos);
    let (speed, angle) = (reg(v.angle1.get()), v.angle0.get());
    let x = mem(fp::fmuls(speed, reg(math::sin(angle))));
    let y = mem(fp::fmuls(speed, reg(math::cos(angle))));
    let mut vel = Vec3::new(x, y, 0.0);
    ip.x40_vel = vel;
    vel.normalize();
    ip.xDD4_itemVar.lgunbeam().velocity.set(vel);
}

/// `itLgunbeam_UnkMotion0_Coll`: a small fixed box, an airborne pass, and turning along what
/// it met.
#[doc(alias = "itLgunbeam_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .lgunbeam()
        .angle0
        .update(clamp_angle);
    match &mut item_mut(r.game, gobj).x378_itemColl.ecb_source.x4 {
        EcbShape::Box {
            up,
            down,
            front,
            back,
            ..
        } => {
            *up = 3.0;
            *down = 3.0;
            *front = 3.0;
            *back = 3.0;
        }
        other => panic!("a flame's ECB source holds {other:?}, not a box"),
    }
    ground::air_stay(r, gobj);
    let env = item(r.game, gobj).x378_itemColl.env_flags;
    let mut flags = 0;
    if env & (0x8000 | 0x10000) != 0 {
        flags |= FLOOR;
    }
    if env & (0x2000 | 0x4000) != 0 {
        flags |= CEILING;
    }
    if env & 0x3F != 0 {
        flags |= LEFT_WALL;
    }
    if env & 0xFC0 != 0 {
        flags |= RIGHT_WALL;
    }
    if flags != 0 {
        surface_direction(r, gobj, flags);
        turn(r, gobj, flags);
    }
    false
}

/// It goes on after a hit or an absorb, `itLGunBeam_Logic39_DmgDealt` and
/// `itLGunBeam_Logic39_Absorbed`.
#[doc(alias = "itLGunBeam_Logic39_DmgDealt")]
#[doc(alias = "itLGunBeam_Logic39_Absorbed")]
pub fn goes_on(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Gone on a clank or a shield, `itLGunBeam_Logic39_Clanked` (and `ShieldBounced`,
/// `HitShield`).
#[doc(alias = "itLGunBeam_Logic39_Clanked")]
#[doc(alias = "itLGunBeam_Logic39_ShieldBounced")]
#[doc(alias = "itLGunBeam_Logic39_HitShield")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Sent back, `itLGunBeam_Logic39_Reflected`: half a turn, facing and flying the other way.
#[doc(alias = "itLGunBeam_Logic39_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.lgunbeam();
    v.angle0
        .update(|a| clamp_angle(mem(fp::frsp(fp::fadd(reg(a), PI)))));
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    false
}

/// Forgets a GObj the flame points to, `itLGunBeam_Logic39_EvtUnk` (`it_8026B894`).
// Ports `itLGunBeam_Logic39_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
