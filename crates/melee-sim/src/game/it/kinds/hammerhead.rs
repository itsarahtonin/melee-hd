//! The hammer's head once it flies off, `melee/it/kinds/ithammerhead.c`: an item that falls,
//! hurting what it hits, lies a while and vanishes.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{article, attr_of, change, change_state, common_data};
use super::super::{item, item_mut, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `It_Kind_Hammer_Head`.
pub const KIND: i32 = 40;

pub const SPAWNED: u32 = 0x8029_9D4C;
pub const PICKED_UP: u32 = 0x8029_9DD0;
pub const ANIM1: u32 = 0x8029_9E1C;
pub const PHYS1: u32 = 0x8029_9E24;
pub const DROPPED: u32 = 0x8029_9E28;
pub const THROWN: u32 = 0x8029_9E48;
pub const ANIM2: u32 = 0x8029_9E9C;
pub const PHYS2: u32 = 0x8029_9F1C;
pub const COLL2: u32 = 0x8029_9F68;
pub const LANDED: u32 = 0x8029_9F94;
pub const ANIM3: u32 = 0x8029_A000;
pub const PHYS3: u32 = 0x8029_A080;
pub const COLL3: u32 = 0x8029_A084;
pub const DMG_DEALT: u32 = 0x8029_A0B0;
pub const CLANKED: u32 = 0x8029_A0B8;
pub const HIT_SHIELD: u32 = 0x8029_A0C0;
pub const REFLECTED: u32 = 0x8029_A0C8;
pub const DMG_RECEIVED: u32 = 0x8029_A0EC;

/// Runs one of the head's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 | DMG_DEALT | CLANKED | HIT_SHIELD | DMG_RECEIVED => no(r, gobj),
        ANIM2 | ANIM3 => tick(r, gobj),
        COLL2 => coll2(r, gobj),
        COLL3 => coll3(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the head's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS1 | PHYS3 => idle(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS2 => phys2(r, gobj),
        LANDED => landed(r, gobj),
        _ => return false,
    }
    true
}

/// `itHammerheadAttributes`' field at `off`: the launch speed (0) and the lifetime (4).
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a hammer head without attributes");
    r.data.f32(a.at(off))
}

/// Throws a head from `pos` along `dir`, `it_80299C48`.
#[doc(alias = "it_80299C48")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, dir: Vec3, facing: f32) {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = owner::ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = super::super::item::spawn_air(r, &mut spawn) else {
        return;
    };
    let speed = attr(r, gobj, 0);
    fly(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = facing;
    let vx = fp::fmuls(reg(dir.x), reg(speed));
    ip.x40_vel.x = mem(fp::fmuls(vx, reg(ip.facing_dir)));
    ip.x40_vel.y = mem(fp::fmadds(reg(dir.y), reg(speed), 0.5));
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
}

/// `itHammerHead_Logic40_Spawned`.
#[doc(alias = "itHammerHead_Logic40_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let life = attr(r, gobj, 4);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// Flies in state 0, `it_80299D7C`.
#[doc(alias = "it_80299D7C")]
pub fn fly(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air != 1 {
        maplib::enter_air(ip);
    }
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, 1 | change::DROP_UPDATE);
}

/// `itHammerHead_Logic40_PickedUp`.
#[doc(alias = "itHammerHead_Logic40_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let life = attr(r, gobj, 4);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// The callbacks that do nothing and answer false: `itHammerhead_UnkMotion1_Anim` and the
/// head's hit callbacks.
#[doc(alias = "itHammerhead_UnkMotion1_Anim")]
#[doc(alias = "itHammerHead_Logic40_DmgDealt")]
#[doc(alias = "itHammerHead_Logic40_Clanked")]
#[doc(alias = "itHammerHead_Logic40_HitShield")]
#[doc(alias = "itHammerHead_Logic40_DmgReceived")]
pub fn no(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itHammerhead_UnkMotion1_Phys` and 3.
#[doc(alias = "itHammerhead_UnkMotion1_Phys")]
#[doc(alias = "itHammerhead_UnkMotion3_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Falls in state 2, `itHammerHead_Logic40_Thrown` (and `_Dropped`).
#[doc(alias = "itHammerHead_Logic40_Thrown")]
#[doc(alias = "itHammerHead_Logic40_Dropped")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air != 1 {
        maplib::enter_air(ip);
    }
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Counts the lifetime down, blinking near its end: whether it ran out,
/// `itHammerhead_UnkMotion2_Anim` and 3.
#[doc(alias = "itHammerhead_UnkMotion2_Anim")]
#[doc(alias = "itHammerhead_UnkMotion3_Anim")]
pub fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let x34 = common_data(r.game).u32(r.data, super::super::data::common::X34);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    if reg(ip.xD44_lifeTimer) <= fp::frsp(f64::from(x34)) {
        lib2725::flicker(r.game, r.data, gobj);
    }
    false
}

/// Gravity and spin, `itHammerhead_UnkMotion2_Phys`.
#[doc(alias = "itHammerhead_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itHammerhead_UnkMotion2_Coll`.
#[doc(alias = "itHammerhead_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Lands, stopped, in state 3, `it_80299F94` (`it_80299FB4`).
#[doc(alias = "it_80299F94")]
#[doc(alias = "it_80299FB4")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 3, 1);
}

/// `itHammerhead_UnkMotion3_Coll`: falls off ledges.
#[doc(alias = "itHammerhead_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, thrown);
    false
}

/// `itHammerHead_Logic40_Reflected`.
#[doc(alias = "itHammerHead_Logic40_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    false
}

/// Forgets a GObj the head points to, `itHammerHead_Logic40_EvtUnk` (`it_8026B894`).
#[doc(alias = "itHammerHead_Logic40_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
