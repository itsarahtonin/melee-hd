//! Bowser's flame, `melee/it/kinds/itkoopaflame.c`: one of his fire breath's puffs, sized and
//! sped by the fuel left, flying at a random angle within its attributes' range and turning
//! along the surfaces it meets, its hitbox gone after `x4` frames and itself after `x0`.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use super::{KIRBY_KOOPA_FLAME, KOOPA_FLAME};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::mp::coll::EcbShape;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x802A_CF9C;
pub const PHYS: u32 = 0x802A_D160;
pub const COLL: u32 = 0x802A_D1E4;
pub const DMG_DEALT: u32 = 0x802A_D2E8;
pub const REFLECTED: u32 = 0x802A_D2F0;
pub const CLANKED: u32 = 0x802A_D384;
pub const ABSORBED: u32 = 0x802A_D38C;
pub const SHIELD_BOUNCED: u32 = 0x802A_D3B0;
pub const HIT_SHIELD: u32 = 0x802A_D450;
pub const EVT_UNK: u32 = 0x802A_D458;

/// `itKoopaFlame_Attributes`: its life, its hitbox's, its speed's and angle's ranges.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const HITBOX_LIFETIME: u32 = 0x4;
    pub const MIN_SPEED: u32 = 0x8;
    pub const MAX_SPEED: u32 = 0xC;
    pub const MIN_ANGLE: u32 = 0x10;
    pub const MAX_ANGLE: u32 = 0x14;
}

/// The surfaces it met (`itkpf_*`).
const FLOOR: i32 = 1;
const CEILING: i32 = 2;
const LEFT_WALL: i32 = 4;
const RIGHT_WALL: i32 = 8;

const PI: f64 = std::f64::consts::PI;
const TAU: f64 = std::f64::consts::TAU;

/// Runs one of the flame's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate_0(r, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        REFLECTED => reflected(r, gobj),
        ABSORBED => absorbed(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
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

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a flame without attributes");
    r.data.f32(attrs.at(off))
}

/// `Item_ClampAngle`: the angle brought within ±π.
fn clamp_angle(a: f32) -> f32 {
    let mut a = a;
    while reg(a) < fp::fneg(PI) {
        a = mem(fp::frsp(fp::fadd(reg(a), TAU)));
    }
    while reg(a) > PI {
        a = mem(fp::frsp(fp::fsub(reg(a), TAU)));
    }
    a
}

/// `Item_ClampAngleReverse`: the same, the top bound first.
fn clamp_angle_reverse(a: f32) -> f32 {
    let mut a = a;
    while reg(a) > PI {
        a = mem(fp::frsp(fp::fsub(reg(a), TAU)));
    }
    while reg(a) < fp::fneg(PI) {
        a = mem(fp::frsp(fp::fadd(reg(a), TAU)));
    }
    a
}

/// The flame's angle clamped in place.
fn clamp_own_angle(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.koopa_flame();
    v.x24_angle.update(clamp_angle);
}

/// A flame for `parent` from `pos`, `itKoopaFlame_Spawn`: its speed and size the fuel's share
/// of their full (`base_speed` and `scale` over the owner's full), a random speed and angle
/// in its attributes' ranges, mirrored when facing left, `gfx` its look.
#[doc(alias = "itKoopaFlame_Spawn")]
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    facing_dir: f32,
    attack_id: u32,
    gfx: i32,
    base_speed: i32,
    scale: i32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = attack_id as i32;
    let gobj = spawn_air(r, &mut spawn)?;
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDD4_itemVar.koopa_flame().x2C_lifetime.set(lifetime);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let ip = item_mut(r.game, gobj);
    let (kind, owner) = (ip.kind, ip.owner);
    let v = ip.xDD4_itemVar.koopa_flame();
    v.x0_pos.set(pos);
    v.x34_base_scale.set(0.0);
    match kind {
        KOOPA_FLAME => {
            let full_speed = r.kp_flame_max_speed(owner);
            let full_size = r.kp_flame_max_size(owner);
            let v = item_mut(r.game, gobj).xDD4_itemVar.koopa_flame();
            v.x38_base_speed.set(mem(fp::fdivs(
                fp::frsp(f64::from(base_speed)),
                fp::frsp(f64::from(full_speed)),
            )));
            v.x3C_scale.set(mem(fp::fdivs(
                fp::frsp(f64::from(scale)),
                fp::frsp(f64::from(full_size)),
            )));
        }
        KIRBY_KOOPA_FLAME => unported_call("ftKb_SpecialNKp_800FA83C (Kirby's flame)"),
        _ => {}
    }
    let (min_speed, max_speed) = (
        reg(attr_f32(r, gobj, attr::MIN_SPEED)),
        reg(attr_f32(r, gobj, attr::MAX_SPEED)),
    );
    let (min_angle, max_angle) = (
        reg(attr_f32(r, gobj, attr::MIN_ANGLE)),
        reg(attr_f32(r, gobj, attr::MAX_ANGLE)),
    );
    let rng = reg(r.game.objs.rng.draw_f32());
    let range = fp::fsubs(max_speed, min_speed);
    let v = item_mut(r.game, gobj).xDD4_itemVar.koopa_flame();
    let base = reg(v.x38_base_speed.get());
    v.x28_speed
        .set(mem(fp::fmuls(base, fp::fmadds(range, rng, min_speed))));
    let rng = reg(r.game.objs.rng.draw_f32());
    let range = fp::fsubs(max_angle, min_angle);
    let ip = item_mut(r.game, gobj);
    let facing = ip.facing_dir;
    let v = ip.xDD4_itemVar.koopa_flame();
    v.x24_angle.set(mem(fp::fmadds(range, rng, min_angle)));
    if facing != 1.0 {
        v.x24_angle.update(|a| mem(fp::fneg(reg(a))));
    }
    v.x24_angle.update(clamp_angle);
    v.x40_frame_counter.set(0);
    v.x30.set(0);
    v.x44_spawned.set(0);
    v.x48_gfx.set(gfx);
    v.xC_direction.set(Vec3::new(facing, 0.0, 0.0));
    ip.xDCC_flag.b3 = false;
    setup(r, gobj, parent);
    Some(gobj)
}

/// The flame's start, `itKoopaFlame_Setup`: its state, animated once, its velocity, and a
/// collision pass from a frame back, keeping where it is.
#[doc(alias = "itKoopaFlame_Setup")]
pub fn setup(r: &mut Running, gobj: GObjId, parent: GObjId) {
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
    debug_owner_bubbles(r.game, gobj, Some(parent));
}

/// `itKoopaFlame_UnkMotion0_Anim`: its hitbox and model at its size, the hitbox off after
/// `x4` frames, its effect on its first frame; gone when its life runs out.
#[doc(alias = "itKoopaFlame_UnkMotion0_Anim")]
fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    let hitbox_life = attr_f32(r, gobj, attr::HITBOX_LIFETIME);
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    if ip.x5D4_hitboxes[0].hit.state != hitcapsule::DISABLED {
        let first = ip.x5D4_hitboxes[0].hit.scale;
        let v = ip.xDD4_itemVar.koopa_flame();
        if v.x34_base_scale.get() == 0.0 {
            v.x34_base_scale.set(first);
        }
        let s = mem(fp::fmuls(
            reg(v.x34_base_scale.get()),
            reg(v.x3C_scale.get()),
        ));
        ip.x5D4_hitboxes[0].hit.scale = s;
    }
    let ip = item_mut(r.game, gobj);
    let s = ip.xDD4_itemVar.koopa_flame().x3C_scale.get();
    let jj = j.expect("a flame without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, jj, Vec3::new(s, s, s));
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.koopa_flame();
    v.x40_frame_counter.update(|n| n.wrapping_add(1));
    let n = v.x40_frame_counter.get();
    if fp::frsp(f64::from(n)) > reg(hitbox_life) {
        lib2725::clear_hitboxes(r.game, r.data, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let kind = ip.kind;
    let v = ip.xDD4_itemVar.koopa_flame();
    if v.x44_spawned.get() == 0 {
        v.x44_spawned.set(1);
        let gfx = v.x48_gfx.get();
        match kind {
            KOOPA_FLAME => {
                ef::spawn(r, 0x4DB_i32.wrapping_add(gfx), Some(gobj), &[Arg::JObj(j)]);
            }
            KIRBY_KOOPA_FLAME => {
                ef::spawn(r, 0x4A5_i32.wrapping_add(gfx), Some(gobj), &[Arg::JObj(j)]);
            }
            _ => {}
        }
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itKoopaFlame_UnkMotion0_Phys`: its velocity at its speed along its angle (from straight
/// up, clockwise), and its direction of travel.
#[doc(alias = "itKoopaFlame_UnkMotion0_Phys")]
fn physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.koopa_flame();
    v.x0_pos.set(pos);
    let (speed, angle) = (reg(v.x28_speed.get()), v.x24_angle.get());
    let x = mem(fp::fmuls(speed, reg(crate::math::sin(angle))));
    let y = mem(fp::fmuls(speed, reg(crate::math::cos(angle))));
    let mut vel = Vec3::new(x, y, 0.0);
    ip.x40_vel = vel;
    vel.normalize();
    ip.xDD4_itemVar.koopa_flame().x18_vel.set(vel);
}

/// `itKoopaFlame_Update_Direction`: its direction of travel turned by the normals of the
/// surfaces it met.
#[doc(alias = "itKoopaFlame_Update_Direction")]
pub fn update_direction(r: &mut Running, gobj: GObjId, flags: i32) {
    let ip = item_mut(r.game, gobj);
    let cd = &ip.x378_itemColl;
    let surfaces = [
        (FLOOR, cd.floor.normal),
        (CEILING, cd.ceiling.normal),
        (LEFT_WALL, cd.left_facing_wall.normal),
        (RIGHT_WALL, cd.right_facing_wall.normal),
    ];
    let v = ip.xDD4_itemVar.koopa_flame();
    let mut d = v.xC_direction.get();
    v.x24_angle.update(clamp_angle);
    for (bit, n) in surfaces {
        if flags & bit != 0 {
            d.x = mem(fp::fadds(reg(d.x), reg(n.x)));
            d.y = mem(fp::fadds(reg(d.y), reg(n.y)));
        }
    }
    d.normalize();
    v.xC_direction.set(d);
}

/// `itKoopaFlame_Update_Angle`: its angle turned toward its new direction, a little when they
/// differ by less than a right angle and more when by more.
#[doc(alias = "itKoopaFlame_Update_Angle")]
pub fn update_angle(r: &mut Running, gobj: GObjId, flags: i32) {
    if flags == 0 {
        return;
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.koopa_flame();
    let (vel, dir) = (v.x18_vel.get(), v.xC_direction.get());
    let a = crate::math::atan2(dir.x, dir.y);
    let b = crate::math::atan2(vel.x, vel.y);
    let diff = clamp_angle_reverse(mem(fp::fsubs(reg(b), reg(a))));
    let turn = if diff == 0.0 {
        0.0
    } else {
        let angle = reg(dir.angle(vel));
        let d = reg(diff);
        let abs = if d < 0.0 { fp::fneg(d) } else { d };
        let k = if abs < fp::fdiv(PI, 2.0) {
            fp::frsp(fp::fmul(f64::from(0.02_f32), fp::fdiv(abs, PI)))
        } else {
            fp::frsp(fp::fmul(0.5, fp::fdiv(abs, PI)))
        };
        let signed = if d < 0.0 { fp::fneg(k) } else { k };
        fp::fmuls(angle, signed)
    };
    v.x24_angle
        .update(|old| clamp_angle(mem(fp::fsubs(reg(old), turn))));
}

/// `itKoopaFlame_UnkMotion0_Coll`: a small fixed box, an airborne pass, and turning along
/// what it met.
#[doc(alias = "itKoopaFlame_UnkMotion0_Coll")]
fn collide(r: &mut Running, gobj: GObjId) -> bool {
    clamp_own_angle(r, gobj);
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
        update_direction(r, gobj, flags);
        update_angle(r, gobj, flags);
    }
    false
}

/// `itKoopaFlame_Logic111_DmgDealt`: it goes on.
#[doc(alias = "itKoopaFlame_Logic111_DmgDealt")]
fn dmg_dealt(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itKoopaFlame_Logic111_Clanked`: it goes on.
#[doc(alias = "itKoopaFlame_Logic111_Clanked")]
fn clanked(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itKoopaFlame_Logic111_HitShield`: it goes on.
#[doc(alias = "itKoopaFlame_Logic111_HitShield")]
fn hit_shield(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Sent back, `itKoopaFlame_Logic111_Reflected`: half a turn, facing and flying the other way.
#[doc(alias = "itKoopaFlame_Logic111_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.koopa_flame();
    v.x24_angle
        .update(|a| clamp_angle(mem(fp::frsp(fp::fadd(reg(a), PI)))));
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    false
}

/// Absorbed, `itKoopaFlame_Logic111_Absorbed`: its effects gone, and itself.
#[doc(alias = "itKoopaFlame_Logic111_Absorbed")]
fn absorbed(r: &mut Running, gobj: GObjId) -> bool {
    ef::eflib::destroy_all(r, gobj);
    true
}

/// Bounced off a shield, `itKoopaFlame_Logic111_ShieldBounced`: its velocity mirrored across
/// the shield's normal, its angle the velocity's.
#[doc(alias = "itKoopaFlame_Logic111_ShieldBounced")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let axis = ip.xC58;
    ip.x40_vel.mirror_xy(axis);
    ip.x40_vel.z = 0.0;
    let a = crate::math::atan2(ip.x40_vel.y, ip.x40_vel.x);
    ip.xDD4_itemVar.koopa_flame().x24_angle.set(clamp_angle(a));
    false
}

/// Forgets a GObj, `itKoopaFlame_Logic111_EvtUnk`.
#[doc(alias = "itKoopaFlame_Logic111_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
