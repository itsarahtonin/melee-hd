//! Ditto (`It_PKind_Metamon`), `melee/it/kinds/itmetamon.c`: out of the ball it cries, falls
//! and grows to its size, and once it lands it plays its animation and is gone. It doesn't
//! attack. Its weight in the Poké Ball's table is 0, so only the debug menu's Pokémon (in a
//! match with fewer than four fighters) brings it out.

use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32};
use super::super::{ground, item_mut, lib26b1, lib2725, maplib};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_3008;
pub const LANDED_NOTHING: u32 = 0x802D_306C;
pub const EVT_UNK: u32 = 0x802D_3070;
pub const LANDED: u32 = 0x802D_3090;
pub const ANIM0: u32 = 0x802D_30EC;
pub const PHYS0: u32 = 0x802D_3120;
pub const COLL0: u32 = 0x802D_3124;
pub const ANIM1: u32 = 0x802D_3150;
pub const PHYS1: u32 = 0x802D_3184;
pub const COLL1: u32 = 0x802D_3188;
pub const ANIM2: u32 = 0x802D_31F4;
pub const PHYS2: u32 = 0x802D_3218;
pub const COLL2: u32 = 0x802D_3238;

/// Its cry.
const CRY: i32 = 0x2728;

/// Runs one of Ditto's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Ditto's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => landed_nothing(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// Ditto out of the ball, `it_802D3008`: at its first size, in the air growing, its cry, and
/// kept as the last Ditto (`it_804D6D50`).
#[doc(alias = "it_802D3008")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    play_sfx(r, gobj, CRY, 0x7F, 0x40);
    r.game.it.last_metamon = Some(gobj);
}

/// `it_802D306C`: nothing (its landing in its first two states).
// Ports `it_802D306C`; aliased once a check reaches it (Ditto landing again).
pub fn landed_nothing(_r: &mut Running, _gobj: GObjId) {}

/// Forgets a GObj going away, `itMetamon_Logic19_EvtUnk` (`it_8026B894`).
// Ports `itMetamon_Logic19_EvtUnk`; aliased once a check reaches it (a fighter's unload).
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802D3090`: stopped, into its animation (`it_802D30C4`).
#[doc(alias = "it_802D3090")]
#[doc(alias = "it_802D30C4")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itMetamon_UnkMotion0_Anim`: gone once its animation ends.
#[doc(alias = "itMetamon_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// `itMetamon_UnkMotion0_Phys`: nothing.
#[doc(alias = "itMetamon_UnkMotion0_Phys")]
pub fn phys0(_r: &mut Running, _gobj: GObjId) {}

/// `itMetamon_UnkMotion0_Coll`: falls, nothing on landing.
#[doc(alias = "itMetamon_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall(r, gobj, landed_nothing);
    false
}

/// `itMetamon_UnkMotion1_Anim`: as its first state's.
// Ports `itMetamon_UnkMotion1_Anim`; aliased once a check reaches it (nothing enters state 1).
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// `itMetamon_UnkMotion1_Phys`: nothing.
// Ports `itMetamon_UnkMotion1_Phys`; aliased once a check reaches it (nothing enters state 1).
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itMetamon_UnkMotion1_Coll`: as its first state's.
// Ports `itMetamon_UnkMotion1_Coll`; aliased once a check reaches it (nothing enters state 1).
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall(r, gobj, landed_nothing);
    false
}

/// Into the air out of the ball, `it_802D31B4`: its third state.
#[doc(alias = "it_802D31B4")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itMetamon_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itMetamon_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itMetamon_UnkMotion2_Phys`: falling out of the ball.
#[doc(alias = "itMetamon_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itMetamon_UnkMotion2_Coll`: lands out of the ball.
#[doc(alias = "itMetamon_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
