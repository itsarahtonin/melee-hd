//! Raikou (`It_PKind_Raikou`), `melee/it/kinds/itraikou.c`: Entei's code with its own sounds and
//! effect ([`entei::Beast`]): out of the ball it lands with a large quake and roars up its attack
//! (its script says when), roaring again every so many frames while its effects run, and goes
//! when its animation ends.

use super::entei::{self, Beast};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_F814;
pub const LANDED_NOTHING: u32 = 0x802C_F880;
pub const EVT_UNK: u32 = 0x802C_F884;
pub const OUT_LANDED: u32 = 0x802C_F8A4;
pub const ANIM0: u32 = 0x802C_F978;
pub const PHYS0: u32 = 0x802C_FA74;
pub const COLL0: u32 = 0x802C_FAB0;
pub const ACCESSORY: u32 = 0x802C_FAFC;
pub const ANIM1: u32 = 0x802C_FBDC;
pub const PHYS1: u32 = 0x802C_FC00;
pub const COLL1: u32 = 0x802C_FC20;

/// Raikou.
pub const RAIKOU: Beast = Beast {
    spawned: SPAWNED,
    landed_nothing: LANDED_NOTHING,
    out_landed: OUT_LANDED,
    anim0: ANIM0,
    phys0: PHYS0,
    coll0: COLL0,
    accessory: ACCESSORY,
    anim1: ANIM1,
    phys1: PHYS1,
    coll1: COLL1,
    effect: 0x46D,
    cry: 0x2745,
    roars: [0x2746, 0x2747, 0x2748],
    on_landed: landed_nothing,
    on_out_landed: out_landed,
};

/// Runs one of Raikou's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    entei::run_pred_of(&RAIKOU, r, gobj, addr)
}

/// Runs one of Raikou's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    entei::run_event_of(&RAIKOU, r, gobj, addr)
}

/// Raikou out of the ball, `it_802CF814`: facing its owner's nearest opponent, its script's
/// flags clear, at its first size, into the air out of the ball.
#[doc(alias = "it_802CF814")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    entei::spawned_of(r, gobj);
}

/// Landing or leaving the floor in its first state, `it_802CF880`: nothing.
#[doc(alias = "it_802CF880")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CF884` (`it_8026B894`).
#[doc(alias = "it_802CF884")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    entei::evt_unk(r, gobj, other);
}

/// Lands out of the ball, `it_802CF8A4`: a large quake, its landing sound, stopped, and roaring.
#[doc(alias = "it_802CF8A4")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    entei::out_landed_of(&RAIKOU, r, gobj);
}

/// Roars, `it_802CF908`: its effect on its script's word, the frames to its next roar.
#[doc(alias = "it_802CF908")]
pub fn roar(r: &mut Running, gobj: GObjId) {
    entei::roar_of(&RAIKOU, r, gobj);
}

/// `itRaikou_UnkMotion0_Anim`: when its script says, its effects go out and it stops roaring; gone
/// when its animation ends; while its effects run, one of three roars every so many frames.
#[doc(alias = "itRaikou_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    entei::anim0_of(&RAIKOU, r, gobj)
}

/// `itRaikou_UnkMotion0_Phys`: gravity in the air.
#[doc(alias = "itRaikou_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    entei::phys0_of(r, gobj);
}

/// `itRaikou_UnkMotion0_Coll`: lands in the air (its hits kept) or walks off the floor, doing nothing
/// either way.
#[doc(alias = "itRaikou_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    entei::coll0_of(&RAIKOU, r, gobj)
}

/// Its `on_accessory`, `it_802CFAFC`: when its script says, its effect (0x46D) on its model
/// and its sound, and it starts roaring.
#[doc(alias = "it_802CFAFC")]
pub fn accessory(r: &mut Running, gobj: GObjId) {
    entei::fire_when_told_of(&RAIKOU, r, gobj);
}

/// Into the air out of the ball, `it_802CFB78` (`Item_EnterAirStateWithHitlag`, state 1).
#[doc(alias = "it_802CFB78")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    entei::enter_air(r, gobj);
}

/// `itRaikou_UnkMotion1_Anim`: growing out of the ball.
#[doc(alias = "itRaikou_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    entei::anim1_of(r, gobj)
}

/// `itRaikou_UnkMotion1_Phys`: falling out of the ball.
#[doc(alias = "itRaikou_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    entei::phys1_of(r, gobj);
}

/// `itRaikou_UnkMotion1_Coll`: lands out of the ball.
#[doc(alias = "itRaikou_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    entei::coll1_of(&RAIKOU, r, gobj)
}
