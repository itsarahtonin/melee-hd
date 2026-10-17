//! The Ice Climbers' ice, `melee/it/kinds/itclimbersice.c`: a block of ice that forms by the
//! hand (state 0), drops if let go before it's thrown (state 1), and once thrown slides along
//! the ground (state 2, falling in state 3), speeding down slopes and slowing up them, until
//! it slows to a stop, bounces off too often or its lifetime runs out. Its damage grows with
//! its speed. Kirby's copy makes it too (`It_Kind_Kirby_IceClimber_Ice`), with its own effects.

use gekko_fp as fp;

use super::super::data::attr as item_attr;
use super::super::item::{
    article, attr_of, change, change_state, common_data, debug_owner_bubbles, spawn_air,
};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, SpawnKind};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_17FC;
pub const ANIM_0: u32 = 0x802C_198C;
pub const PHYS_0: u32 = 0x802C_1A14;
pub const COLL_0: u32 = 0x802C_1A18;
pub const ANIM_1: u32 = 0x802C_1A80;
pub const PHYS_1: u32 = 0x802C_1A88;
pub const COLL_1: u32 = 0x802C_1AB8;
pub const ANIM_2: u32 = 0x802C_1B80;
pub const PHYS_2: u32 = 0x802C_1BB4;
pub const COLL_2: u32 = 0x802C_1C7C;
pub const ANIM_3: u32 = 0x802C_1D80;
pub const PHYS_3: u32 = 0x802C_1DB4;
pub const COLL_3: u32 = 0x802C_1E94;
pub const DMG_DEALT: u32 = 0x802C_1FC8;
pub const REFLECTED: u32 = 0x802C_1FD0;
pub const CLANKED: u32 = 0x802C_1FF0;
pub const HIT_SHIELD: u32 = 0x802C_1FF8;
pub const ABSORBED: u32 = 0x802C_20E4;
pub const SHIELD_BOUNCED: u32 = 0x802C_20EC;
pub const EVT_UNK: u32 = 0x802C_210C;

/// `itClimbersIceAttributes` offsets.
mod attr {
    /// The thrown ice's lifetime.
    pub const LIFETIME: u32 = 0x0;
    /// What a bounce takes off its lifetime, and the least it may have left.
    pub const BOUNCE_COST: u32 = 0x4;
    pub const BOUNCE_MIN_LIFE: u32 = 0x8;
    /// The fastest it still bounces off a wall or shield at.
    pub const BOUNCE_SPEED: u32 = 0xC;
    /// Its throw speed.
    pub const SPEED: u32 = 0x10;
    /// The slope's pull, and its scale down and up the slope.
    pub const SLOPE: u32 = 0x14;
    pub const SLOPE_DOWN: u32 = 0x18;
    pub const SLOPE_UP: u32 = 0x1C;
    /// The speed it stops below.
    pub const STOP_SPEED: u32 = 0x24;
    /// How fast its model shrinks once thrown away.
    pub const SHRINK: u32 = 0x28;
    /// Its damage at rest, and per unit of speed.
    pub const DAMAGE: u32 = 0x2C;
    pub const DAMAGE_SCALE: u32 = 0x30;
}

/// `Collide_FloorMask`.
const FLOOR_MASK: i32 = 0x18000;

/// Runs one of the ice's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => shrink(r, gobj),
        COLL_0 => {
            ground::ground_or_fall(r, gobj, drop);
            maplib::tilt_on_floor(r.game, r.data, gobj);
            false
        }
        ANIM_1 => false,
        COLL_1 => {
            ground::fall(r, gobj, stop);
            false
        }
        ANIM_2 | ANIM_3 => tick_lifetime(r, gobj),
        COLL_2 => {
            ground::ground_or_fall(r, gobj, slide_fall);
            maplib::tilt_on_floor(r.game, r.data, gobj);
            wall_bounce(r, gobj)
        }
        COLL_3 => {
            if ground::air_touches(r, gobj) & 1 != 0 {
                slide_effect(r, gobj);
                change_state(r, gobj, 2, change::ANIM_UPDATE);
            }
            wall_bounce(r, gobj)
        }
        DMG_DEALT | CLANKED | ABSORBED => true,
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => lib2725::bounce_off_shield(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of the ice's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS_0 => {}
        PHYS_1 => fall(r, gobj),
        PHYS_2 => {
            if slide(r, gobj) {
                set_damage(r, gobj);
            }
        }
        PHYS_3 => {
            // `Item_ApplyFallingPhysics`.
            fall(r, gobj);
            let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
            lib2725::clamp_fall(r.game, gobj, x68);
            set_damage(r, gobj);
        }
        _ => return false,
    }
    true
}

/// Falls at its kind's fall speeds (`it_80272860`).
fn fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, item_attr::FALL_SPEED);
    let max = a.f32(r.data, item_attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("ice without attributes");
    r.data.f32(attrs.at(off))
}

fn attr_u32(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("ice without attributes");
    r.data.u32(attrs.at(off))
}

/// The model's first child, which the ice's effects and scale go on.
fn child(r: &Running, gobj: GObjId) -> Option<crate::hsd::jobj::JObjId> {
    jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child)
}

/// Makes the ice at `pos` by its parent, `it_802C1590`: forming (state 0) at its full size,
/// its owner kept, with the forming effect, not to be picked up.
// Ports `it_802C1590`; aliased once a check reaches it.
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let scale = attr_of(item(r.game, gobj)).f32(r.data, item_attr::SCALE);
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.climbers_ice();
    v.x0.set(owner);
    v.x8_b0.set(false);
    v.x4.set(scale);
    let id = if kind == super::ICE_CLIMBER_ICE {
        0x4E9
    } else {
        0x4AE
    };
    let j = child(r, gobj);
    super::super::effect::queue(r, gobj, SpawnKind::Attach, id, j, &[]);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    lib26b1::disable_pickup(r.game, gobj);
    super::super::lib2725::first_frame(r, gobj);
    Some(gobj)
}

/// Throws the ice, `it_802C16F8`: forward at its speed, its lifetime set, its model turned
/// along its motion, with the throw's effect, sliding (state 2).
// Ports `it_802C16F8`; aliased once a check reaches it.
pub fn throw(r: &mut Running, gobj: GObjId) {
    let speed = attr_f32(r, gobj, attr::SPEED);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_ice()
        .x8_b0
        .set(true);
    lib2725::face_motion(r.game, gobj);
    effect(r, gobj, 0x4EA, 0x4AF);
    slide_effect(r, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Takes the ice away, `it_802C17DC` (`Item_8026A8EC`).
// Ports `it_802C17DC`; aliased once a check reaches it.
pub fn remove(r: &mut Running, gobj: GObjId) {
    super::super::item::destroy(r, gobj);
}

/// The ice's end, `it_2725_Logic90_Destroyed`: its owner forgets it (`ftPp_Init_8011F16C`, or
/// Kirby's copy's `ftKb_SpecialNIc_80108CE8`).
// Ports `it_2725_Logic90_Destroyed`; aliased once a check reaches it.
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let kind = ip.kind;
    if let Some(owner) = ip.xDD4_itemVar.climbers_ice().x0.get() {
        if kind == super::ICE_CLIMBER_ICE {
            r.popo_forget_item(owner, gobj);
        } else {
            r.kb_ic_forget(owner, gobj);
        }
    }
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_ice()
        .x0
        .set(None);
}

/// One of the ice's effects by its kind (`itClimbersice_Spawn`): on its model, facing its
/// way.
fn effect(r: &mut Running, gobj: GObjId, ice: i32, kirby: i32) {
    let ip = item(r.game, gobj);
    let id = if ip.kind == super::ICE_CLIMBER_ICE {
        ice
    } else {
        kirby
    };
    let facing = ip.facing_dir;
    let j = child(r, gobj);
    super::super::effect::queue(
        r,
        gobj,
        SpawnKind::AttachParam,
        id,
        j,
        &[Arg::Float(facing)],
    );
}

/// The sliding effect, then state 2, `it_802C1AE4`.
// Ports `it_802C1AE4`; aliased once a check reaches it.
fn slide_effect(r: &mut Running, gobj: GObjId) {
    effect(r, gobj, 0x4EB, 0x4B0);
}

/// Shrinks the ice once thrown away, `itClimbersice_UnkMotion0_Anim`: done once small.
// Ports `itClimbersice_UnkMotion0_Anim`; aliased once a check reaches it.
fn shrink(r: &mut Running, gobj: GObjId) -> bool {
    let shrink = attr_f32(r, gobj, attr::SHRINK);
    let j = child(r, gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_ice();
    if !v.x8_b0.get() {
        return false;
    }
    let s = mem(fp::fmuls(reg(v.x4.get()), reg(shrink)));
    v.x4.set(s);
    if let Some(j) = j {
        lib2725::set_uniform_scale(r.game, j, s);
    }
    reg(s) < 0.009_999_999_776_482_582
}

/// Lets go of the ice before its throw, `it_802C1A58`: it falls (state 1).
// Ports `it_802C1A58`; aliased once a check reaches it.
fn drop(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Stops the ice, `it_802C1950`: no velocity, state 0.
// Ports `it_802C1950`; aliased once a check reaches it.
fn stop(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The slide falling off a floor, `fn_802C1D44`: state 3, its effects gone.
// Ports `fn_802C1D44`; aliased once a check reaches it.
fn slide_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    crate::game::ef::eflib::destroy_all(r, gobj);
}

/// `Item_TickLifetime`: done once it reaches 0, else one less.
fn tick_lifetime(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// The slide on a floor, `it_802C1854`: the floor's slope speeds it down and slows it up;
/// slow enough, it stops (pickup back on, `xDCD_b3`, state 0, its effects gone). True once
/// it stops.
// Ports `it_802C1854`; aliased once a check reaches it.
fn slide(r: &mut Running, gobj: GObjId) -> bool {
    let slope = reg(attr_f32(r, gobj, attr::SLOPE));
    let down = reg(attr_f32(r, gobj, attr::SLOPE_DOWN));
    let up = reg(attr_f32(r, gobj, attr::SLOPE_UP));
    let stop_speed = reg(attr_f32(r, gobj, attr::STOP_SPEED));
    let ip = item_mut(r.game, gobj);
    if ip.x378_itemColl.env_flags & FLOOR_MASK == 0 {
        return false;
    }
    let nx = reg(ip.x378_itemColl.floor.normal.x);
    let normal_sign = if nx < 0.0 { -1 } else { 1 };
    let vel_sign = if ip.x40_vel.x < 0.0 { -1 } else { 1 };
    let delta = if vel_sign != normal_sign {
        fp::fmuls(up, fp::fmuls(nx, slope))
    } else {
        fp::fmuls(down, fp::fmuls(nx, slope))
    };
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), delta));
    let vx = reg(ip.x40_vel.x);
    let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if speed < stop_speed {
        // `it_8026BD84`.
        item_mut(r.game, gobj).xDCD_flag.x0.b4 = true;
        lib26b1::set_xdcd_b3(r.game, gobj);
        stop(r, gobj);
        crate::game::ef::eflib::destroy_all(r, gobj);
        return true;
    }
    false
}

/// The ice's damage by its speed (`itClimbersice_Phys_inline`).
fn set_damage(r: &mut Running, gobj: GObjId) {
    let scale = attr_u32(r, gobj, attr::DAMAGE_SCALE);
    let base = attr_u32(r, gobj, attr::DAMAGE);
    let vx = reg(item(r.game, gobj).x40_vel.x);
    let v = fp::fmuls(vx, fp::frsp(f64::from(scale as i32)));
    let v = if v < 0.0 { fp::fneg(v) } else { v };
    let damage = crate::ppc::cvt_fp2unsigned(v).wrapping_add(base);
    super::super::coll::set_hit_damage(r.game, r.data, gobj, 0, damage);
}

/// What a bounce takes off the lifetime (`itClimbersIce_sub_x4`).
fn bounce_cost(r: &mut Running, gobj: GObjId) {
    let cost = attr_f32(r, gobj, attr::BOUNCE_COST);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), reg(cost)));
}

/// A wall met (`itClimbersice_Coll`): slow enough, it bounces back at a cost to its
/// lifetime; else it breaks.
fn wall_bounce(r: &mut Running, gobj: GObjId) -> bool {
    let max = reg(attr_f32(r, gobj, attr::BOUNCE_SPEED));
    if maplib::touched_wall(r.game, gobj) == 0 {
        return false;
    }
    let vx = reg(item(r.game, gobj).x40_vel.x);
    let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if speed <= max {
        maplib::bounce_off_wall(r.game, r.data, gobj);
        lib2725::face_motion(r.game, gobj);
        bounce_cost(r, gobj);
        return false;
    }
    true
}

/// A shield hit, `it_2725_Logic90_HitShield`: slow enough, it bounces back slowed at a cost
/// to its lifetime, breaking once too little is left; faster, it breaks.
// Ports `it_2725_Logic90_HitShield`; aliased once a check reaches it.
fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    let max = reg(attr_f32(r, gobj, attr::BOUNCE_SPEED));
    let min_life = reg(attr_f32(r, gobj, attr::BOUNCE_MIN_LIFE));
    let vx = reg(item(r.game, gobj).x40_vel.x);
    let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if speed > max {
        return true;
    }
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    lib2725::face_motion(r.game, gobj);
    let s = reg(attr_of(item(r.game, gobj)).f32(r.data, item_attr::X58));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), s));
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), s));
    ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), s));
    bounce_cost(r, gobj);
    reg(item(r.game, gobj).xD44_lifeTimer) < min_life
}

/// Forgets a GObj the ice points to, `itClimbersIce_Logic90_EvtUnk`: its owner, then
/// `it_8026B894`'s.
// Ports `itClimbersIce_Logic90_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_ice();
    if v.x0.get() == other {
        v.x0.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}
