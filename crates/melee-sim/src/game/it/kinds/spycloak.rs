//! The Cloaking Device, `melee/it/kinds/itspycloak.c`: it falls and rests; the fighter that
//! picks it up turns invisible (`ftpickupitem_8009447C`, `ftCo_800C88D4`).

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state};
use super::super::{item_mut, lib26b1, maplib};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_5C48;
pub const ANIM0: u32 = 0x8029_5CB8;
pub const PHYS0: u32 = 0x8029_5CC0;
pub const COLL0: u32 = 0x8029_5CC4;
pub const ANIM3: u32 = 0x8029_5D2C;
pub const PHYS1: u32 = 0x8029_5D34;
pub const COLL1: u32 = 0x8029_5D64;
pub const PICKED_UP: u32 = 0x8029_5D90;
pub const ANIM2: u32 = 0x8029_5DB8;
pub const DROPPED: u32 = 0x8029_5DC0;
pub const PHYS3: u32 = 0x8029_5DE8;
pub const COLL3: u32 = 0x8029_5E18;
pub const ENTERED_AIR: u32 = 0x8029_5E4C;
pub const ANIM4: u32 = 0x8029_5E74;
pub const PHYS4: u32 = 0x8029_5E7C;
pub const COLL4: u32 = 0x8029_5E80;

/// Runs one of the Cloaking Device's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Cloaking Device's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 | PHYS4 => idle(r, gobj),
        PHYS1 | PHYS3 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itSpyCloak_Logic33_Spawned`: falling.
#[doc(alias = "itSpyCloak_Logic33_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    fell(r, gobj);
}

/// Stops and rests, `it_80295C68`.
#[doc(alias = "it_80295C68")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itSpycloak_UnkMotion0_Anim` (2, 3 for 1 and 3, 4).
#[doc(alias = "itSpycloak_UnkMotion0_Anim")]
#[doc(alias = "itSpycloak_UnkMotion2_Anim")]
#[doc(alias = "itSpycloak_UnkMotion3_Anim")]
#[doc(alias = "itSpycloak_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itSpycloak_UnkMotion0_Phys` and 4.
#[doc(alias = "itSpycloak_UnkMotion0_Phys")]
#[doc(alias = "itSpycloak_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itSpycloak_UnkMotion0_Coll`: resting, it falls off ledges and tilts with the floor.
#[doc(alias = "itSpycloak_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Falls, `it_80295D04`.
#[doc(alias = "it_80295D04")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itSpycloak_UnkMotion1_Phys` and 3.
#[doc(alias = "itSpycloak_UnkMotion1_Phys")]
#[doc(alias = "itSpycloak_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itSpycloak_UnkMotion1_Coll`: lands.
#[doc(alias = "itSpycloak_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itSpyCloak_Logic33_PickedUp`.
#[doc(alias = "itSpyCloak_Logic33_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itSpyCloak_Logic33_Dropped`.
#[doc(alias = "itSpyCloak_Logic33_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itSpycloak_UnkMotion3_Coll`: thrown, gone when it touches anything.
#[doc(alias = "itSpycloak_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// Skidding off a slope, `itSpyCloak_Logic33_EnteredAir`.
#[doc(alias = "itSpyCloak_Logic33_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itSpycloak_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itSpycloak_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj the device points to, `itSpyCloak_Logic33_EvtUnk` (`it_8026B894`).
#[doc(alias = "itSpyCloak_Logic33_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
