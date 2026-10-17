//! The Metal Box, `melee/it/kinds/itmetalb.c`: it falls and rests; the fighter that breaks it
//! (or picks it up, `ftpickupitem_8009447C`) turns metal (`ftLib_ApplyMetalBox`).

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state, is_fighter};
use super::super::{coll, flag, item, item_mut, lib26b1, lib2725, maplib, set_flag};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_53C8;
pub const ANIM0: u32 = 0x8029_544C;
pub const PHYS0: u32 = 0x8029_5454;
pub const COLL0: u32 = 0x8029_5458;
pub const ANIM3: u32 = 0x8029_54C0;
pub const PHYS1: u32 = 0x8029_54C8;
pub const COLL1: u32 = 0x8029_54F8;
pub const PICKED_UP: u32 = 0x8029_5524;
pub const ANIM2: u32 = 0x8029_554C;
pub const DROPPED: u32 = 0x8029_5554;
pub const PHYS3: u32 = 0x8029_557C;
pub const COLL3: u32 = 0x8029_55AC;
pub const DMG_RECEIVED: u32 = 0x8029_55E0;
pub const ENTERED_AIR: u32 = 0x8029_5684;
pub const ANIM4: u32 = 0x8029_56AC;
pub const PHYS4: u32 = 0x8029_56B4;
pub const COLL4: u32 = 0x8029_56B8;

/// Runs one of the Metal Box's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Metal Box's events, if `addr` is one.
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

/// `itMetalB_Logic32_Spawned`: falling.
#[doc(alias = "itMetalB_Logic32_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
    fell(r, gobj);
}

/// Stops and rests, `it_802953FC`.
#[doc(alias = "it_802953FC")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itMetalb_UnkMotion0_Anim` (2, 3 for 1 and 3, 4).
#[doc(alias = "itMetalb_UnkMotion0_Anim")]
#[doc(alias = "itMetalb_UnkMotion2_Anim")]
#[doc(alias = "itMetalb_UnkMotion3_Anim")]
#[doc(alias = "itMetalb_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itMetalb_UnkMotion0_Phys` and 4.
#[doc(alias = "itMetalb_UnkMotion0_Phys")]
#[doc(alias = "itMetalb_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itMetalb_UnkMotion0_Coll`: resting, it falls off ledges and tilts with the floor.
#[doc(alias = "itMetalb_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Falls, `it_80295498`.
#[doc(alias = "it_80295498")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itMetalb_UnkMotion1_Phys` and 3.
#[doc(alias = "itMetalb_UnkMotion1_Phys")]
#[doc(alias = "itMetalb_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itMetalb_UnkMotion1_Coll`: lands.
#[doc(alias = "itMetalb_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itMetalB_Logic32_PickedUp`.
#[doc(alias = "itMetalB_Logic32_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itMetalB_Logic32_Dropped`.
#[doc(alias = "itMetalB_Logic32_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itMetalb_UnkMotion3_Coll`: thrown, gone when it touches anything.
#[doc(alias = "itMetalb_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// `itMetalB_Logic32_DmgReceived`: broken, it turns its breaker metal when that's a fighter
/// whose hit could open it (`xDCF` bit 6), which counts the item for its player; gone either
/// way.
#[doc(alias = "itMetalB_Logic32_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let Some(by) = coll::take_attacker(r.game, gobj) else {
        return true;
    };
    if is_fighter(r.game, Some(by)) && flag(item(r.game, gobj).xDCF_flag, 6) {
        r.apply_metal_box(by, gobj);
        let fp = r.game.fighter(by);
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        r.item_pickup_stat(slot, sub, gobj);
        return true;
    }
    item_mut(r.game, gobj).xCEC_fighterGObj = None;
    true
}

/// Skidding off a slope, `itMetalB_Logic32_EnteredAir`.
#[doc(alias = "itMetalB_Logic32_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itMetalb_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itMetalb_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj the box points to, `itMetalB_Logic32_EvtUnk` (`it_8026B894`).
#[doc(alias = "itMetalB_Logic32_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
