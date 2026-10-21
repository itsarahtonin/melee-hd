//! The Flipper, `melee/it/kinds/itflipper.c`: thrown, it flies a few frames and settles in the
//! air, where its hits knock fighters away; a hit on it spins its paddle and pushes it back.
//! A stage can fix one to a joint (`itFlipper_Spawn`).

use super::super::ground::{air_stay, air_touches, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, common_data, is_fighter, spawn_air};
use super::super::{data, hitbox, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::lb::hitcapsule::clear_victims;
use crate::lb::joint::world_position;
use crate::math;
use crate::vec::Vec3;
use gekko_fp as fp;
use melee_data::archive::DataRef;

pub const SPAWNED: u32 = 0x8029_0A20;
pub const ENTER_RESTING: u32 = 0x8029_0E78;
pub const ANIM0: u32 = 0x8029_0EC8;
pub const PHYS0: u32 = 0x8029_0ED0;
pub const COLL0: u32 = 0x8029_0ED4;
pub const ENTER_FALLING: u32 = 0x8029_0F00;
pub const ANIM1: u32 = 0x8029_0F28;
pub const PHYS1: u32 = 0x8029_0F30;
pub const COLL1: u32 = 0x8029_0F60;
pub const PICKED_UP: u32 = 0x8029_0F8C;
pub const ANIM2: u32 = 0x8029_0FB4;
pub const PHYS2: u32 = 0x8029_0FBC;
pub const DROPPED: u32 = 0x8029_0FC0;
pub const THROWN: u32 = 0x8029_0FE0;
pub const ANIM3: u32 = 0x8029_1068;
pub const PHYS3: u32 = 0x8029_10A8;
pub const COLL3: u32 = 0x8029_1188;
pub const ANIM5: u32 = 0x8029_14C8;
pub const ANIM6: u32 = 0x8029_151C;
pub const PHYS5: u32 = 0x8029_1570;
pub const COLL5: u32 = 0x8029_15B4;
pub const DMG_DEALT: u32 = 0x8029_15E8;
pub const CLANKED: u32 = 0x8029_17C8;
pub const HIT_SHIELD: u32 = 0x8029_18A0;
pub const REFLECTED: u32 = 0x8029_1978;
pub const SHIELD_BOUNCED: u32 = 0x8029_1998;
pub const DMG_RECEIVED: u32 = 0x8029_19B8;
pub const ENTERED_AIR: u32 = 0x8029_1B58;
pub const ANIM4: u32 = 0x8029_1B80;
pub const PHYS4: u32 = 0x8029_1B88;
pub const COLL4: u32 = 0x8029_1B8C;

/// The states (`FLIPPER_MS_*`).
const MS_RESTING: i32 = 0;
const MS_FALLING: i32 = 1;
const MS_HELD: i32 = 2;
const MS_INFLIGHT: i32 = 3;
const MS_AIRBORNE: i32 = 4;
const MS_ACTIVE: i32 = 5;
const MS_SPINNING: i32 = 6;

/// `itFlipper_DatAttrs`' offsets.
const A_THROW: u32 = 0x0;
const A_SMASH_THROW: u32 = 0x4;
const A_LIFETIME: u32 = 0x8;
const A_WALL_BOUNCE: u32 = 0xC;
const A_FLOOR_BOUNCE: u32 = 0x10;
const A_HITBOX_INTERVAL: u32 = 0x14;
const A_SPIN_MULT: u32 = 0x18;
const A_BASE_SPIN: u32 = 0x1C;
const A_SPIN_DECAY: u32 = 0x20;
const A_MAX_SPIN: u32 = 0x24;

/// π and 2π as single-precision constants, and a degree in radians.
const PI: f64 = 3.1415927410125732;
const TAU: f64 = 6.2831854820251465;
const DEG: f64 = 0.01745329238474369;

/// Runs one of the Flipper's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => resting_coll(r, gobj),
        COLL1 => falling_coll(r, gobj),
        ANIM3 => inflight_anim(r, gobj),
        COLL3 => inflight_coll(r, gobj),
        ANIM5 => active_anim(r, gobj),
        ANIM6 => spinning_anim(r, gobj),
        COLL5 => spinning_coll(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        CLANKED | HIT_SHIELD => bounce_or_spin(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        COLL4 => airborne_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Flipper's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        ENTER_RESTING => enter_resting(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => idle(r, gobj),
        ENTER_FALLING => enter_falling(r, gobj),
        PHYS1 => falling_phys1(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS3 => inflight_phys(r, gobj),
        PHYS5 => spinning_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Flipper without attributes")
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f64 {
    reg(r.data.f32(attrs(r, gobj).at(off)))
}

fn attr_i(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attrs(r, gobj).at(off)) as i32
}

/// A Flipper fixed to a stage's joint, `itFlipper_Spawn`.
// Ports `itFlipper_Spawn`; aliased once a check reaches it.
pub fn spawn(r: &mut Running, jobj: Option<JObjId>) -> Option<GObjId> {
    let j = jobj?;
    let pos = world_position(&mut r.game.objs, r.data, Some(j), None);
    let mut spawn = SpawnItem {
        kind: super::FLIPPER,
        prev_pos: pos,
        pos,
        ..Default::default()
    };
    spawn.facing_dir = 1.0;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    v.xDE8_isStageFixed.set(1);
    v.xDEC_stageAnchor.set(Some(j));
    settle(r, gobj);
    Some(gobj)
}

/// `itFlipper_Spawned`: nothing thrown, settled or spun yet, falling.
#[doc(alias = "itFlipper_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    v.xDD4_flightTimer.set(0);
    v.xDD8_isSettled.set(0);
    v.xDDC_hitboxTimer.set(0);
    v.xDE0_spinAngle.set(0.0);
    v.xDE4_spinVel.set(0.0);
    v.xDE8_isStageFixed.set(0);
    v.xDEC_stageAnchor.set(None);
    hitbox::start_blink(r.game, gobj);
    enter_falling(r, gobj);
}

/// Turns the paddle by its spin, which slows by the attributes' decay, `itFlipper_UpdateSpin`.
/// (The spin, not the angle, is kept within ±π.)
#[doc(alias = "itFlipper_UpdateSpin")]
pub fn update_spin(r: &mut Running, gobj: GObjId) {
    let decay = attr_f(r, gobj, A_SPIN_DECAY);
    let j = jobj_of(r.game, gobj)
        .and_then(|j| r.game.objs.jobjs[j].child)
        .and_then(|c| r.game.objs.jobjs[c].child);
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    if reg(v.xDE4_spinVel.get()) > PI {
        v.xDE4_spinVel.update(|s| mem(fp::fsubs(reg(s), TAU)));
    }
    if reg(v.xDE4_spinVel.get()) < fp::fneg(PI) {
        v.xDE4_spinVel.update(|s| mem(fp::fadds(reg(s), TAU)));
    }
    let abs = |s: f64| if s < 0.0 { fp::fneg(s) } else { s };
    let s = reg(v.xDE4_spinVel.get());
    if abs(s) > 0.0 {
        if abs(s) < decay {
            v.xDE4_spinVel.set(0.0);
        } else if s > 0.0 {
            v.xDE4_spinVel.set(mem(fp::fsubs(s, decay)));
        } else {
            v.xDE4_spinVel.set(mem(fp::fadds(s, decay)));
        }
    }
    let s = reg(v.xDE4_spinVel.get());
    v.xDE0_spinAngle.update(|a| mem(fp::fadds(reg(a), s)));
    let angle = v.xDE0_spinAngle.get();
    lib2725::set_rotation_x(r.game, j.expect("a Flipper without its paddle"), angle);
}

/// Spins the paddle by `vel` plus the attributes' base spin, the way that turns it from a hit
/// at `pos`, up to the attributes' limit, `itFlipper_AddSpinImpulse`.
#[doc(alias = "itFlipper_AddSpinImpulse")]
pub fn add_spin_impulse(r: &mut Running, gobj: GObjId, pos: Vec3, vel: f64) {
    let speed = fp::fadds(vel, attr_f(r, gobj, A_BASE_SPIN));
    let max = attr_f(r, gobj, A_MAX_SPIN);
    let me = item(r.game, gobj).pos;
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    let right = reg(pos.x) > reg(me.x);
    let above = reg(pos.y) > reg(me.y);
    let add = right == above;
    if add {
        v.xDE4_spinVel.update(|s| mem(fp::fadds(reg(s), speed)));
    } else {
        v.xDE4_spinVel.update(|s| mem(fp::fsubs(reg(s), speed)));
    }
    if reg(v.xDE4_spinVel.get()) > max {
        v.xDE4_spinVel.set(mem(max));
    }
    if reg(v.xDE4_spinVel.get()) < fp::fneg(max) {
        v.xDE4_spinVel.set(mem(fp::fneg(max)));
    }
}

/// The spin a fighter's hit gives: its speed times the attributes' multiplier, from the
/// fighter's camera bone (which becomes `pos`), or 10 without a fighter (the inline
/// `spinSpeed`).
fn spin_speed(r: &mut Running, gobj: GObjId, other: Option<GObjId>, pos: &mut Vec3) -> f64 {
    let mut speed = 10.0;
    if let Some(f) = other.filter(|&g| is_fighter(r.game, Some(g))) {
        let fid = r.game.fighter_id(f);
        *pos = r.camera_bone_pos(fid);
        let d = r.game.fighters[fid.0].pos_delta;
        let len2 = mem(fp::fadds(
            fp::fmuls(reg(d.x), reg(d.x)),
            fp::fmuls(reg(d.y), reg(d.y)),
        ));
        speed = fp::fmuls(attr_f(r, gobj, A_SPIN_MULT), reg(math::sqrt(len2)));
    }
    speed
}

/// Spins from the fighter its hitbox last met (`xCF4`), `itFlipper_SpinFromFighter`.
#[doc(alias = "itFlipper_SpinFromFighter")]
pub fn spin_from_fighter(r: &mut Running, gobj: GObjId) {
    let mut pos = item(r.game, gobj).pos;
    let other = item(r.game, gobj).xCF4_fighterGObjUnk;
    let mut speed = 10.0;
    if other.is_some() {
        speed = spin_speed(r, gobj, other, &mut pos);
        item_mut(r.game, gobj).xCF4_fighterGObjUnk = None;
    }
    add_spin_impulse(r, gobj, pos, fp::fmuls(DEG, speed));
}

/// Spins from the fighter that hit it (`xCEC`), the damage hook's inline.
fn spin_from_attacker(r: &mut Running, gobj: GObjId) {
    let mut pos = item(r.game, gobj).pos;
    let other = item(r.game, gobj).xCEC_fighterGObj;
    let mut speed = 10.0;
    if other.is_some() {
        speed = spin_speed(r, gobj, other, &mut pos);
        item_mut(r.game, gobj).xCEC_fighterGObj = None;
    }
    add_spin_impulse(r, gobj, pos, fp::fmuls(DEG, speed));
}

/// Pushed away from another Flipper at `pos` (another kind pushes nothing),
/// `itFlipper_Repel`.
#[doc(alias = "itFlipper_Repel")]
pub fn repel(r: &mut Running, gobj: GObjId, kind: i32, pos: Vec3) {
    if kind != super::FLIPPER {
        return;
    }
    let me = item(r.game, gobj).pos;
    let mut v = Vec3::new(
        mem(fp::fsubs(reg(me.x), reg(pos.x))),
        mem(fp::fsubs(reg(me.y), reg(pos.y))),
        0.0,
    );
    v.normalize_xy();
    v.x = mem(fp::fmuls(reg(v.x), 0.20000000298023224));
    v.y = mem(fp::fmuls(reg(v.y), 0.20000000298023224));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), reg(v.x)));
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(v.y)));
}

/// Stops and rests on the ground, `itFlipper_EnterResting`.
#[doc(alias = "itFlipper_EnterResting")]
pub fn enter_resting(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, MS_RESTING, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itFlipper_Resting_Anim` (falling, held, airborne).
#[doc(alias = "itFlipper_Resting_Anim")]
#[doc(alias = "itFlipper_Falling_Anim")]
#[doc(alias = "itFlipper_Held_Anim")]
#[doc(alias = "itFlipper_Airborne_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itFlipper_Resting_Phys` (held, airborne).
#[doc(alias = "itFlipper_Resting_Phys")]
#[doc(alias = "itFlipper_Held_Phys")]
#[doc(alias = "itFlipper_Airborne_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itFlipper_Resting_Coll`: falls off ledges.
#[doc(alias = "itFlipper_Resting_Coll")]
pub fn resting_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, enter_falling);
    false
}

/// Falls, `itFlipper_EnterFalling`.
#[doc(alias = "itFlipper_EnterFalling")]
pub fn enter_falling(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, MS_FALLING, change::ANIM_UPDATE);
}

/// Gravity, `itFlipper_Falling_Phys`.
#[doc(alias = "itFlipper_Falling_Phys")]
pub fn falling_phys1(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itFlipper_Falling_Coll`: lands.
#[doc(alias = "itFlipper_Falling_Coll")]
pub fn falling_coll(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, enter_resting);
    false
}

/// Held, `itFlipper_PickedUp`.
#[doc(alias = "itFlipper_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, MS_HELD, change::ANIM_UPDATE);
}

/// Thrown (or let go of), it flies for the attributes' frames, longer for a smash throw,
/// `itFlipper_Thrown` (and `itFlipper_Dropped`).
#[doc(alias = "itFlipper_Thrown")]
#[doc(alias = "itFlipper_Dropped")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        // `ftLib_80087284`: whether the owner's state is a smash throw.
        let smash = r.game.fighter(owner).motion_id >= crate::game::ft::co::ids::ms::LIGHT_THROW_F4;
        let t = attr_i(r, gobj, if smash { A_SMASH_THROW } else { A_THROW });
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .flipper()
            .xDD4_flightTimer
            .set(t);
    }
    lib26b1::disable_pickup(r.game, gobj);
    change_state(
        r,
        gobj,
        MS_INFLIGHT,
        change::ANIM_UPDATE | change::DROP_UPDATE,
    );
}

/// `itFlipper_Inflight_Anim`: settles when its flight runs out.
#[doc(alias = "itFlipper_Inflight_Anim")]
pub fn inflight_anim(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    v.xDD4_flightTimer.update(|t| t.wrapping_sub(1));
    if v.xDD4_flightTimer.get() <= 0 {
        settle(r, gobj);
    }
    false
}

/// `itFlipper_Inflight_Phys`: gravity, slowing over its last ten frames, and the spin.
#[doc(alias = "itFlipper_Inflight_Phys")]
pub fn inflight_phys(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
    let t = item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDD4_flightTimer
        .get();
    if t <= 10 {
        let k = || {
            let tf = fp::frsp(f64::from(t));
            fp::fdivs(tf, fp::fadds(1.0, tf))
        };
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k()));
        ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), k()));
    }
    let x68 = common_data(r.game).f32(r.data, data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itFlipper_Inflight_Coll`: bounces off walls, ceilings and floors.
#[doc(alias = "itFlipper_Inflight_Coll")]
pub fn inflight_coll(r: &mut Running, gobj: GObjId) -> bool {
    let wall = attr_f(r, gobj, A_WALL_BOUNCE);
    let floor = attr_f(r, gobj, A_FLOOR_BOUNCE);
    let coll = air_touches(r, gobj);
    if coll != 0 {
        let ip = item_mut(r.game, gobj);
        let v = &mut ip.x40_vel;
        if coll & 0xC != 0 {
            v.x = mem(fp::fmuls(reg(v.x), fp::fneg(wall)));
        }
        v.y = mem(fp::fmuls(reg(v.y), wall));
        if coll & 2 != 0 {
            v.x = mem(fp::fmuls(reg(v.x), wall));
        }
        v.y = mem(fp::fmuls(reg(v.y), fp::fneg(wall)));
        if coll & 1 != 0 {
            v.x = mem(fp::fmuls(reg(v.x), floor));
        }
        v.y = mem(fp::fmuls(reg(v.y), fp::fneg(floor)));
    }
    false
}

/// Stops in the air, armed, for the attributes' lifetime, `itFlipper_Settle`.
#[doc(alias = "itFlipper_Settle")]
pub fn settle(r: &mut Running, gobj: GObjId) {
    let life = attr_i(r, gobj, A_LIFETIME);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDD1_flag, 1, true);
    ip.xDD4_itemVar.flipper().xDD8_isSettled.set(1);
    hitbox::set_hit_ids(r.game, gobj);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE8_isStageFixed
        .get()
        != 0
    {
        hitbox::clear_hit_ids_b7(r.game, gobj);
    }
    item_mut(r.game, gobj).xD44_lifeTimer = mem(fp::frsp(f64::from(life)));
    lib26b1::disable_pickup(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    enter_active(r, gobj);
}

/// `itFlipper_EnterActive`.
#[doc(alias = "itFlipper_EnterActive")]
pub fn enter_active(r: &mut Running, gobj: GObjId) {
    change_state(
        r,
        gobj,
        MS_ACTIVE,
        change::ANIM_UPDATE | change::HIT_PRESERVE,
    );
}

/// What both armed states do each frame, `itFlipper_UpdateActive`: the spin, its drift
/// slowing, its hitboxes back after a hit's interval, and its life (whether it's gone).
#[doc(alias = "itFlipper_UpdateActive")]
pub fn update_active(r: &mut Running, gobj: GObjId) -> bool {
    update_spin(r, gobj);
    let ip = item_mut(r.game, gobj);
    let abs = |s: f64| if s < 0.0 { fp::fneg(s) } else { s };
    for v in [&mut ip.x40_vel.x, &mut ip.x40_vel.y] {
        if abs(reg(*v)) >= 0.009999999776482582 {
            *v = mem(fp::fmuls(reg(*v), 0.8999999761581421));
        } else {
            *v = 0.0;
        }
    }
    let timer = ip.xDD4_itemVar.flipper().xDDC_hitboxTimer.get();
    if timer > 0 {
        let v = ip.xDD4_itemVar.flipper();
        v.xDDC_hitboxTimer.set(timer.wrapping_sub(1));
        if v.xDDC_hitboxTimer.get() == 0 {
            // `itFlipper_RefreshHitboxes`.
            lib2725::start_hitbox(r.game, r.data, gobj, 0);
            lib2725::start_hitbox(r.game, r.data, gobj, 1);
            let ip = item_mut(r.game, gobj);
            clear_victims(&mut ip.x5D4_hitboxes[0].hit);
            clear_victims(&mut ip.x5D4_hitboxes[1].hit);
            hitbox::end_blink(r.game, gobj);
        }
    }
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE8_isStageFixed
        .get()
        == 0
    {
        let flicker = common_data(r.game).i32(r.data, data::common::X34);
        let ip = item_mut(r.game, gobj);
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
        if reg(ip.xD44_lifeTimer) <= 0.0 {
            return true;
        }
        if reg(ip.xD44_lifeTimer) <= fp::frsp(f64::from(flicker)) {
            lib2725::flicker(r.game, r.data, gobj);
        }
    }
    false
}

/// `itFlipper_Active_Anim`: spinning once its paddle turns.
#[doc(alias = "itFlipper_Active_Anim")]
pub fn active_anim(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE4_spinVel
        .get()
        != 0.0
    {
        change_state(
            r,
            gobj,
            MS_SPINNING,
            change::ANIM_UPDATE | change::HIT_PRESERVE,
        );
    }
    update_active(r, gobj)
}

/// `itFlipper_Spinning_Anim`: still again once its paddle stops.
#[doc(alias = "itFlipper_Spinning_Anim")]
pub fn spinning_anim(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE4_spinVel
        .get()
        == 0.0
    {
        change_state(
            r,
            gobj,
            MS_ACTIVE,
            change::ANIM_UPDATE | change::HIT_PRESERVE,
        );
    }
    update_active(r, gobj)
}

/// `itFlipper_Spinning_Phys` (both armed states): a fixed Flipper follows its joint.
#[doc(alias = "itFlipper_Spinning_Phys")]
pub fn spinning_phys(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.flipper();
    if v.xDE8_isStageFixed.get() != 0
        && let Some(j) = v.xDEC_stageAnchor.get()
    {
        let p = world_position(&mut r.game.objs, r.data, Some(j), None);
        item_mut(r.game, gobj).pos = p;
    }
}

/// `itFlipper_Spinning_Coll` (both armed states): a free Flipper keeps to the air.
#[doc(alias = "itFlipper_Spinning_Coll")]
pub fn spinning_coll(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE8_isStageFixed
        .get()
        == 0
    {
        air_stay(r, gobj);
    }
    false
}

/// Bounces off a victim while flying (the inline `bounce`): off it as items do after six
/// frames, straight back before.
fn bounce(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDD4_flightTimer
        .get()
        >= 6
    {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    } else {
        let x60 = reg(common_data(r.game).f32(r.data, data::common::X60));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(fp::fneg(1.0), reg(ip.x40_vel.x)));
        ip.x40_vel.y = mem(fp::fmadds(fp::fneg(1.0), reg(ip.x40_vel.y), x60));
    }
}

/// A hit's spin, armed: its hitboxes out for the attributes' interval, spun by `spin`, and
/// pushed from another Flipper of `kind` at `pos` unless fixed.
fn hit_spin(r: &mut Running, gobj: GObjId, spin: fn(&mut Running, GObjId), kind: i32, pos: Vec3) {
    let interval = attr_i(r, gobj, A_HITBOX_INTERVAL);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDDC_hitboxTimer
        .set(interval);
    spin(r, gobj);
    lib2725::clear_hitbox(r.game, r.data, gobj, 0);
    lib2725::clear_hitbox(r.game, r.data, gobj, 1);
    hitbox::start_blink(r.game, gobj);
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDE8_isStageFixed
        .get()
        == 0
    {
        repel(r, gobj, kind, pos);
    }
}

fn settled(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .flipper()
        .xDD8_isSettled
        .get()
        != 0
}

/// Its hitbox hit something, `itFlipper_DmgDealt`: flying, it bounces; armed, it spins from
/// the fighter it hit and is pushed from what it hit (`xC38`, `xCD4`).
#[doc(alias = "itFlipper_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if !settled(r, gobj) {
        bounce(r, gobj);
    } else {
        let (kind, pos) = (item(r.game, gobj).xC38, item(r.game, gobj).xCD4);
        hit_spin(r, gobj, spin_from_fighter, kind, pos);
    }
    false
}

/// A clank or a shield, `itFlipper_Clanked` and `itFlipper_HitShield`: as a hit dealt.
#[doc(alias = "itFlipper_Clanked")]
#[doc(alias = "itFlipper_HitShield")]
pub fn bounce_or_spin(r: &mut Running, gobj: GObjId) -> bool {
    dmg_dealt(r, gobj)
}

/// Reflected, it flies back the way it came, `itFlipper_Reflected`.
#[doc(alias = "itFlipper_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounced off a shield, `itFlipper_ShieldBounced`.
#[doc(alias = "itFlipper_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Hit, armed, it spins from its attacker and is pushed from what hit it (`xCB4`, `xCE0`),
/// `itFlipper_DmgReceived`.
#[doc(alias = "itFlipper_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    if settled(r, gobj) {
        let (kind, pos) = (item(r.game, gobj).xCB4, item(r.game, gobj).xCE0);
        hit_spin(r, gobj, spin_from_attacker, kind, pos);
    }
    false
}

/// Skidding off a slope, `itFlipper_EnteredAir`.
#[doc(alias = "itFlipper_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, MS_AIRBORNE, change::ANIM_UPDATE);
}

/// `itFlipper_Airborne_Coll`: skidding, it stops or falls.
#[doc(alias = "itFlipper_Airborne_Coll")]
pub fn airborne_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, enter_resting, enter_falling);
    false
}

/// Forgets a GObj the Flipper points to, `itFlipper_EvtUnk` (`it_8026B894`).
// Ports `itFlipper_EvtUnk`; aliased once a check reaches it (nothing dispatches the items'
// `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
