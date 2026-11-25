//! The power-ups' shared code: the Metal Box, the Bunny Hood, the Cloaking Device and the Warp
//! Star (`melee/it/kinds/itmetalb.c`, `itrabbitc.c`, `itspycloak.c`, `itwstar.c`, near-copies of
//! each other here). Each rests (state 0), falls (1), is held (2), thrown (3) and skids off a
//! slope (4); let go of, it falls. What they share is the spawn, the rest and the stop, the fall
//! and its landing, the pickup and the drop, the throw's vanishing, the skid and the forget, over
//! a [`PowerUp`] per item. What each gives its fighter (the Metal Box's metal, the Bunny Hood's
//! ears, the Cloaking Device's cloak) is the fighter's code; the Warp Star's ride (its pick of a
//! ride, its sizing, its descent and its crash) stays its own.

use super::super::ground::{Event, air, fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state};
use super::super::{item_mut, lib26b1, lib2725, maplib, set_flag};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// One power-up's settings the shared callbacks read.
pub struct PowerUp {
    /// Whether its spawn clears its hurt flag `xDCE` bit 7 (the Metal Box, the Warp Star).
    pub clears_hurt: bool,
    /// What its spawn sets up after that, before it falls (the Warp Star's ride's scales).
    pub on_spawn: Option<Event>,
    /// Whether resting, it tilts with the floor (the Metal Box, the Cloaking Device).
    pub tilts: bool,
    /// What its pickup does after it's held (the Bunny Hood's shine hidden, the Warp Star's
    /// glow).
    pub on_pickup: Option<Event>,
    /// What its drop does after it falls (the Bunny Hood's shine shown, the Warp Star's effects
    /// gone).
    pub on_drop: Option<Event>,
}

/// Its spawn: falling.
#[doc(alias = "itMetalB_Logic32_Spawned")]
#[doc(alias = "itRabbitC_Logic31_Spawned")]
#[doc(alias = "itSpyCloak_Logic33_Spawned")]
#[doc(alias = "itWStar_Logic29_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId, p: &PowerUp) {
    if p.clears_hurt {
        set_flag(&mut item_mut(r.game, gobj).hurt_flags, 7, false);
    }
    if let Some(f) = p.on_spawn {
        f(r, gobj);
    }
    fell(r, gobj);
}

/// Stops and rests, to be picked up: `it_802953FC` (the Metal Box), `it_80295138` (the Bunny
/// Hood), `it_80295C68` (the Cloaking Device), `it_8029455C` (the Warp Star).
#[doc(alias = "it_802953FC")]
#[doc(alias = "it_80295138")]
#[doc(alias = "it_80295C68")]
#[doc(alias = "it_8029455C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing.
#[doc(alias = "itMetalb_UnkMotion0_Anim")]
#[doc(alias = "itMetalb_UnkMotion2_Anim")]
#[doc(alias = "itMetalb_UnkMotion3_Anim")]
#[doc(alias = "itMetalb_UnkMotion4_Anim")]
#[doc(alias = "itRabbitc_UnkMotion0_Anim")]
#[doc(alias = "itRabbitc_UnkMotion2_Anim")]
#[doc(alias = "itRabbitc_UnkMotion3_Anim")]
#[doc(alias = "itRabbitc_UnkMotion4_Anim")]
#[doc(alias = "itSpycloak_UnkMotion0_Anim")]
#[doc(alias = "itSpycloak_UnkMotion2_Anim")]
#[doc(alias = "itSpycloak_UnkMotion3_Anim")]
#[doc(alias = "itSpycloak_UnkMotion4_Anim")]
#[doc(alias = "itWstar_UnkMotion0_Anim")]
#[doc(alias = "itWstar_UnkMotion5_Coll")]
#[doc(alias = "itWstar_UnkMotion1_Anim")]
#[doc(alias = "itWstar_UnkMotion3_Anim")]
#[doc(alias = "itWstar_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itMetalb_UnkMotion0_Phys")]
#[doc(alias = "itMetalb_UnkMotion4_Phys")]
#[doc(alias = "itRabbitc_UnkMotion0_Phys")]
#[doc(alias = "itRabbitc_UnkMotion4_Phys")]
#[doc(alias = "itSpycloak_UnkMotion0_Phys")]
#[doc(alias = "itSpycloak_UnkMotion4_Phys")]
#[doc(alias = "itWstar_UnkMotion4_Phys")]
#[doc(alias = "itWstar_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Resting, it falls off ledges (with [`PowerUp::tilts`], tilting with the floor).
#[doc(alias = "itMetalb_UnkMotion0_Coll")]
#[doc(alias = "itRabbitc_UnkMotion0_Coll")]
#[doc(alias = "itSpycloak_UnkMotion0_Coll")]
#[doc(alias = "itWstar_UnkMotion0_Coll")]
pub fn rests(r: &mut Running, gobj: GObjId, p: &PowerUp) -> bool {
    ground_or_fall(r, gobj, fell);
    if p.tilts {
        maplib::tilt_on_floor(r.game, r.data, gobj);
    }
    false
}

/// Falls (state 1): `it_80295498` (the Metal Box), `it_802951C0` (the Bunny Hood),
/// `it_80295D04` (the Cloaking Device), `it_80294624` (the Warp Star).
#[doc(alias = "it_80295498")]
#[doc(alias = "it_802951C0")]
#[doc(alias = "it_80295D04")]
#[doc(alias = "it_80294624")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, falling or thrown.
#[doc(alias = "itMetalb_UnkMotion1_Phys")]
#[doc(alias = "itMetalb_UnkMotion3_Phys")]
#[doc(alias = "itRabbitc_UnkMotion1_Phys")]
#[doc(alias = "itRabbitc_UnkMotion3_Phys")]
#[doc(alias = "itSpycloak_UnkMotion1_Phys")]
#[doc(alias = "itSpycloak_UnkMotion3_Phys")]
#[doc(alias = "itWstar_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// Falling, it lands and rests.
#[doc(alias = "itMetalb_UnkMotion1_Coll")]
#[doc(alias = "itRabbitc_UnkMotion1_Coll")]
#[doc(alias = "itSpycloak_UnkMotion1_Coll")]
#[doc(alias = "itWstar_UnkMotion1_Coll")]
pub fn lands(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Picked up, held (state 2), then its [`PowerUp::on_pickup`].
#[doc(alias = "itMetalB_Logic32_PickedUp")]
#[doc(alias = "itRabbitC_Logic31_PickedUp")]
#[doc(alias = "itSpyCloak_Logic33_PickedUp")]
#[doc(alias = "it_3F14_Logic29_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId, p: &PowerUp) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    if let Some(f) = p.on_pickup {
        f(r, gobj);
    }
}

/// Let go of, it falls (state 1), then its [`PowerUp::on_drop`].
#[doc(alias = "itMetalB_Logic32_Dropped")]
#[doc(alias = "itRabbitC_Logic31_Dropped")]
#[doc(alias = "itSpyCloak_Logic33_Dropped")]
#[doc(alias = "itWStar_Logic29_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId, p: &PowerUp) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    if let Some(f) = p.on_drop {
        f(r, gobj);
    }
}

/// Thrown, gone when it touches anything (the Warp Star's state 3 is its descent).
#[doc(alias = "itMetalb_UnkMotion3_Coll")]
#[doc(alias = "itRabbitc_UnkMotion3_Coll")]
#[doc(alias = "itSpycloak_UnkMotion3_Coll")]
pub fn vanishes(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// Skidding off a slope (state 4).
#[doc(alias = "itMetalB_Logic32_EnteredAir")]
#[doc(alias = "itRabbitC_Logic31_EnteredAir")]
#[doc(alias = "itSpyCloak_Logic33_EnteredAir")]
#[doc(alias = "itWStar_Logic29_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Skidding, it stops or falls.
#[doc(alias = "itMetalb_UnkMotion4_Coll")]
#[doc(alias = "itRabbitc_UnkMotion4_Coll")]
#[doc(alias = "itSpycloak_UnkMotion4_Coll")]
#[doc(alias = "itWstar_UnkMotion4_Coll")]
pub fn skids(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj it points to (`it_8026B894`).
#[doc(alias = "itMetalB_Logic32_EvtUnk")]
#[doc(alias = "itRabbitC_Logic31_EvtUnk")]
#[doc(alias = "itSpyCloak_Logic33_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
