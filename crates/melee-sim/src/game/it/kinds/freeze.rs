//! The Freezie, `melee/it/kinds/itfreeze.c`: it falls, then slides along the ground, faster
//! downhill, turning at walls; thrown, it breaks on what it touches. Topis push it (states 4
//! and 5, through `itFreeze_ItemVars::unk_1C`): Icicle Mountain's, and with the Freezie on, the
//! containers' ([`spawn_pushed`]).

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::ground::{air_touches, fall_to_floor, ground_or_fall};
use super::super::item::{article, change, change_state, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::{FLOOR_HUG, FLOOR_PUSH};
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x8028_ED10;
pub const SPAWNED: u32 = 0x8028_ED50;
pub const ANIM0: u32 = 0x8028_EF88;
pub const PHYS0: u32 = 0x8028_EF90;
pub const COLL0: u32 = 0x8028_EFB0;
pub const FELL: u32 = 0x8028_F1D8;
pub const ANIM1: u32 = 0x8028_F200;
pub const PHYS1: u32 = 0x8028_F208;
pub const COLL1: u32 = 0x8028_F238;
pub const PICKED_UP: u32 = 0x8028_F2B4;
pub const ANIM2: u32 = 0x8028_F310;
pub const DROPPED: u32 = 0x8028_F318;
pub const THROWN: u32 = 0x8028_F340;
pub const PHYS3: u32 = 0x8028_F368;
pub const COLL3: u32 = 0x8028_F398;
pub const DMG_DEALT: u32 = 0x8028_F3CC;
pub const CLANKED: u32 = 0x8028_F3D4;
pub const HIT_SHIELD: u32 = 0x8028_F3DC;
pub const ABSORBED: u32 = 0x8028_F3E4;
pub const REFLECTED: u32 = 0x8028_F3EC;
pub const SHIELD_BOUNCED: u32 = 0x8028_F40C;
pub const DMG_RECEIVED: u32 = 0x8028_F42C;
pub const ANIM4: u32 = 0x8028_F4AC;
pub const PHYS4: u32 = 0x8028_F4B4;
pub const COLL4: u32 = 0x8028_F560;
pub const ANIM5: u32 = 0x8028_F828;
pub const PHYS5: u32 = 0x8028_F830;
pub const COLL5: u32 = 0x8028_F834;

/// Runs one of the Freezie's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 | ANIM5 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED | DMG_RECEIVED => gone(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Freezie's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS4 => pushed(r, gobj),
        PHYS5 => idle(r, gobj),
        _ => return false,
    }
    true
}

/// `itUnkAttributes`' float at `off`: the slide's start speed (4), top speed (8), slope
/// acceleration (0xC) and deceleration (0x10).
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Freezie without attributes");
    r.data.f32(a.at(off))
}

/// Lets go of the Topi pushing it, if one is (the inline `itFreeze_ClearLinkedItem`): the Topi
/// forgets it (`it_802E37A4`), and it the Topi.
fn release_pusher(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.freeze();
    if let Some(topi) = v.pusher.get() {
        super::topi::forget_freezie(r, topi);
        item_mut(r.game, gobj).vars.freeze().pusher.set(None);
    }
}

/// A Freezie a Topi pushes, `it_8028EB88`: made in the air from `pos` toward the Topi's
/// position, facing `facing`, no parent; set up as any Freezie (again, after its own
/// setup), then pushed (`decel` its deceleration, [`start_pushed`]).
#[doc(alias = "it_8028EB88")]
pub fn spawn_pushed(
    r: &mut Running,
    topi: GObjId,
    pos: Vec3,
    facing: f32,
    decel: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: ItKind::Freezie.id(),
        ..SpawnItem::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = item(r.game, topi).pos;
    // `Item_InitSpawnCommonFields(&spawn, NULL, facing_dir, true)`.
    spawn.facing_dir = facing;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = None;
    spawn.maker = spawn.owner;
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    spawned(r, gobj);
    start_pushed(r, gobj, decel, Some(topi));
    Some(gobj)
}

/// Pushed by `topi`, `it_8028F434`: stopped, slowing by `decel`'s size, pickable, on the
/// ground, state 4.
#[doc(alias = "it_8028F434")]
pub fn start_pushed(r: &mut Running, gobj: GObjId, decel: f32, topi: Option<GObjId>) {
    let ip = item_mut(r.game, gobj);
    ip.vel.z = 0.0;
    ip.vel.y = 0.0;
    ip.vel.x = 0.0;
    let decel = abs(reg(decel));
    let v = ip.vars.freeze();
    v.push_decel.set(mem(decel));
    v.pusher.set(topi);
    lib26b1::enable_pickup(r.game, gobj);
    maplib::enter_ground(item_mut(r.game, gobj));
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// The Topi hands the Freezie a speed, `it_8028EC98`, which its push takes on its next frame,
/// and its life starts over at the items' common lifetime.
#[doc(alias = "it_8028EC98")]
pub fn hand_speed(r: &mut Running, gobj: GObjId, vel: f32) {
    item_mut(r.game, gobj).vars.freeze().handed_speed.set(vel);
    let life =
        super::super::item::common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life as i32))));
}

/// The Topi lets go, `it_8028ECE0`: the Freezie forgets it.
#[doc(alias = "it_8028ECE0")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).vars.freeze().pusher.set(None);
}

/// The Freezie's position, `it_8028ECF0`.
#[doc(alias = "it_8028ECF0")]
pub fn position(r: &Running, gobj: GObjId) -> Vec3 {
    item(r.game, gobj).pos
}

/// `itFreeze_Logic17_Destroyed`: lets go of the Topi pushing it.
#[doc(alias = "itFreeze_Logic17_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    release_pusher(r, gobj);
}

/// `it_3F14_Logic17_Spawned`: facing the stage's middle, at rest, falling.
#[doc(alias = "it_3F14_Logic17_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = if reg(ip.pos.x) > 0.0 { -1.0 } else { 1.0 };
    ip.skid = 0;
    let v = ip.vars.freeze();
    v.speed.set(0.0);
    v.normal.set_x(0.0);
    v.normal.set_y(1.0);
    v.normal.set_z(0.0);
    v.pusher.set(None);
    fell(r, gobj);
}

/// `fabsf` as the branches compile it.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

/// The slide, `it_8028EDBC`: on a slope it speeds up downhill (starting at its start speed
/// from rest), on the flat it slows to its start speed, up to its top speed, facing the way
/// it goes, the ground's normal's y scaling its velocity.
#[doc(alias = "it_8028EDBC")]
pub fn slide(r: &mut Running, gobj: GObjId) {
    let (start, top, accel, decel) = (
        attr(r, gobj, 4),
        attr(r, gobj, 8),
        attr(r, gobj, 0xC),
        attr(r, gobj, 0x10),
    );
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air == GroundOrAir::Air {
        return;
    }
    let v = ip.vars.freeze();
    let nx = v.normal.x();
    let mut speed = v.speed.get();
    if abs(reg(nx)) > f64::from(0.1_f32) {
        if reg(speed) == 0.0 {
            ip.facing_dir = if reg(nx) > 0.0 { 1.0 } else { -1.0 };
            speed = mem(fp::fmuls(reg(start), reg(ip.facing_dir)));
        } else {
            speed = mem(fp::fmadds(reg(nx), reg(accel), reg(speed)));
        }
    } else if abs(reg(speed)) < reg(start) {
        speed = mem(fp::fmuls(reg(start), reg(ip.facing_dir)));
    } else if reg(speed) > 0.0 {
        speed = mem(fp::fsubs(reg(speed), reg(decel)));
    } else {
        speed = mem(fp::fadds(reg(speed), reg(decel)));
    }
    if reg(speed) > reg(top) {
        speed = top;
    } else if reg(speed) < fp::fneg(reg(top)) {
        speed = mem(fp::fneg(reg(top)));
    }
    if (reg(speed) > 0.0 && reg(ip.facing_dir) < 0.0)
        || (reg(speed) < 0.0 && reg(ip.facing_dir) > 0.0)
    {
        ip.facing_dir = mem(fp::fmuls(reg(ip.facing_dir), -1.0));
    }
    let v = ip.vars.freeze();
    v.speed.set(speed);
    let ny = v.normal.y();
    ip.vel.x = mem(fp::fmuls(reg(speed), reg(ny)));
}

/// Lands and slides on at its speed, `it_8028EF34`.
#[doc(alias = "it_8028EF34")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let speed = ip.vars.freeze().speed.get();
    ip.vel.x = speed;
    ip.vel.z = 0.0;
    ip.vel.y = 0.0;
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itFreeze_UnkMotion0_Anim` (2, 3 for 1 and 3, 4, 5).
#[doc(alias = "itFreeze_UnkMotion0_Anim")]
#[doc(alias = "itFreeze_UnkMotion2_Anim")]
#[doc(alias = "itFreeze_UnkMotion3_Anim")]
#[doc(alias = "itFreeze_UnkMotion4_Anim")]
#[doc(alias = "itFreeze_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Moves nothing, `itFreeze_UnkMotion5_Phys`.
#[doc(alias = "itFreeze_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Turns the model along the ground's slope, `HSD_JObjSetRotationZ` with `it_8027649C`.
fn lie_along(r: &mut Running, gobj: GObjId) {
    let angle = maplib::slope_angle(r.game, gobj);
    let angle = if item(r.game, gobj).facing_dir == -1.0 {
        mem(angle)
    } else {
        mem(fp::fmuls(-1.0, angle))
    };
    let j = jobj_of(r.game, gobj).expect("a Freezie without a model");
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, angle);
}

/// Keeps the floor's normal (`it_80276408`), when it touched a floor.
fn keep_normal(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if let Some(n) = maplib::surface_normal(&ip.coll) {
        let v = ip.vars.freeze();
        v.normal.set_x(n.x);
        v.normal.set_y(n.y);
        v.normal.set_z(n.z);
    }
}

/// Whether the collision touched a wall (`0x3F`, `0xFC0`).
fn hit_wall(r: &Running, gobj: GObjId) -> bool {
    let env = item(r.game, gobj).coll.env_flags;
    env & 0x3F != 0 || env & 0xFC0 != 0
}

/// `itFreeze_UnkMotion0_Coll`: sliding, it falls off ledges, keeps the floor's normal (or
/// none on the flat), turns back at walls, and lies along the slope.
#[doc(alias = "itFreeze_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    if item(r.game, gobj).ground_or_air == GroundOrAir::Air {
        return false;
    }
    if item(r.game, gobj).coll.env_flags & 0x18000 != 0 {
        keep_normal(r, gobj);
    } else {
        item_mut(r.game, gobj).vars.freeze().normal.set_x(0.0);
    }
    if hit_wall(r, gobj) {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        let v = ip.vars.freeze();
        let speed = mem(fp::fneg(reg(v.speed.get())));
        v.speed.set(speed);
        let ny = v.normal.y();
        ip.vel.x = mem(fp::fmuls(reg(speed), reg(ny)));
    }
    lie_along(r, gobj);
    false
}

/// Falls, `it_8028F1D8`.
#[doc(alias = "it_8028F1D8")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itFreeze_UnkMotion1_Phys` and `itFreeze_UnkMotion3_Phys`.
#[doc(alias = "itFreeze_UnkMotion1_Phys")]
#[doc(alias = "itFreeze_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itFreeze_UnkMotion1_Coll`: lands on a floor, keeping its normal.
#[doc(alias = "itFreeze_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    let ip = item(r.game, gobj);
    if ip.ground_or_air != GroundOrAir::Air && ip.coll.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        keep_normal(r, gobj);
    }
    false
}

/// Held, `itFreeze_Logic17_PickedUp`.
#[doc(alias = "itFreeze_Logic17_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    release_pusher(r, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itFreeze_Logic17_Dropped`.
#[doc(alias = "itFreeze_Logic17_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown, `itFreeze_Logic17_Thrown`.
#[doc(alias = "itFreeze_Logic17_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itFreeze_UnkMotion3_Coll`: thrown, it's gone once it touches anything.
#[doc(alias = "itFreeze_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air_touches(r, gobj) != 0
}

/// Gone after a hit, a clank, a shield, an absorption or damage, `itFreeze_Logic17_DmgDealt`
/// (and `Clanked`, `HitShield`, `Absorbed`, `DmgReceived`).
#[doc(alias = "itFreeze_Logic17_DmgDealt")]
#[doc(alias = "itFreeze_Logic17_Clanked")]
#[doc(alias = "itFreeze_Logic17_HitShield")]
#[doc(alias = "itFreeze_Logic17_Absorbed")]
#[doc(alias = "itFreeze_Logic17_DmgReceived")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Sent back by a reflection, `itFreeze_Logic17_Reflected`.
#[doc(alias = "itFreeze_Logic17_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itFreeze_Logic17_ShieldBounced`.
#[doc(alias = "itFreeze_Logic17_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// `itFreeze_UnkMotion4_Phys`: pushed, a speed handed over takes effect, then it slows by
/// its deceleration (`x14`) to a stop.
#[doc(alias = "itFreeze_UnkMotion4_Phys")]
pub fn pushed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let v = ip.vars.freeze();
    let handed = v.handed_speed.get();
    if reg(handed) != 0.0 {
        v.handed_speed.set(mem(fp::fmuls(0.0, 0.0)));
        ip.vel.x = handed;
    }
    let mut decel = reg(ip.vars.freeze().push_decel.get());
    let vel = reg(ip.vel.x);
    if abs(vel) < abs(decel) {
        ip.vel.x = 0.0;
        return;
    }
    if vel > 0.0 {
        if reg(ip.vars.freeze().push_decel.get()) < 0.0 {
            decel = fp::fneg(decel);
        }
        ip.vel.x = mem(fp::fsubs(vel, decel));
    } else {
        if decel < 0.0 {
            decel = fp::fneg(decel);
        }
        ip.vel.x = mem(fp::fadds(vel, decel));
    }
}

/// Slides on at its speed, `itFreeze_ResetToMotion0`.
fn slide_on(r: &mut Running, gobj: GObjId) {
    landed(r, gobj);
}

/// `itFreeze_UnkMotion4_Coll`: pushed, it falls off ledges (letting go of the Topi), stops
/// against a wall, slides on once the Topi lets go, and lies along the slope.
#[doc(alias = "itFreeze_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    if item(r.game, gobj).ground_or_air == GroundOrAir::Air {
        release_pusher(r, gobj);
        return false;
    }
    if hit_wall(r, gobj) {
        stopped(r, gobj);
        release_pusher(r, gobj);
        return false;
    }
    if item_mut(r.game, gobj).vars.freeze().pusher.get().is_none() {
        slide_on(r, gobj);
        return false;
    }
    lie_along(r, gobj);
    false
}

/// Stopped against a wall for 90 ticks, `it_8028F7C8`.
#[doc(alias = "it_8028F7C8")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).vars.freeze().stop_frames.set(90);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itFreeze_UnkMotion5_Coll`: stopped, it falls off ledges, and slides on once its count
/// runs out.
#[doc(alias = "itFreeze_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    let v = item_mut(r.game, gobj).vars.freeze();
    let left = v.stop_frames.get();
    if left <= 0 {
        slide_on(r, gobj);
    } else {
        v.stop_frames.set(left - 1);
    }
    false
}

/// `itFreeze_UnkMotion0_Phys`: the slide.
#[doc(alias = "itFreeze_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    slide(r, gobj);
}
