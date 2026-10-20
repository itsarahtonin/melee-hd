//! Porygon2 (`It_PKind_Porygon2`), `melee/it/kinds/itporygon2.c`: out of the ball it charges
//! along its animation, its root bone's motion taken as its own (its hitbox is its script's),
//! then a second run the same way, and is gone when that ends.

use super::super::ground::air;
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, effect_hitlag, table_bone, take_bone_motion};
use super::super::{item, lib26b1, lib2725};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_5ABC;
pub const CHARGE: u32 = 0x802D_5B14;
pub const ANIM0: u32 = 0x802D_5B60;
pub const PHYS0: u32 = 0x802D_5BBC;
pub const COLL0: u32 = 0x802D_5BDC;
pub const LEAVE: u32 = 0x802D_5C00;
pub const ANIM1: u32 = 0x802D_5C60;
pub const PHYS1: u32 = 0x802D_5CB0;
pub const COLL1: u32 = 0x802D_5CD0;
pub const EVT_UNK: u32 = 0x802D_5CD8;

/// Runs one of Porygon2's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        _ => return None,
    })
}

/// Runs one of Porygon2's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        CHARGE => charge(r, gobj),
        PHYS0 | PHYS1 => phys0(r, gobj),
        LEAVE => leave(r, gobj),
        _ => return false,
    }
    true
}

/// `itPorygon2_Logic26_Spawned`: at its item's scale (it doesn't grow out of the ball), its
/// sound, and it charges.
#[doc(alias = "itPorygon2_Logic26_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let scale = item(r.game, gobj).scl;
    pokemon::start(r, gobj, scale);
    play_sfx(r, gobj, 0x273E, 0x7F, 0x40);
    charge(r, gobj);
}

/// Charges, `it_802D5B14`.
#[doc(alias = "it_802D5B14")]
pub fn charge(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itPorygon2_UnkMotion0_Anim`: at its animation's end it runs again; its body's motion
/// taken as its own.
#[doc(alias = "itPorygon2_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        leave(r, gobj);
    }
    let body = table_bone(r.game, gobj, 1);
    take_bone_motion(r, body, gobj);
    false
}

/// `itPorygon2_UnkMotion0_Phys` (and `_1_Phys`): moved by its body.
#[doc(alias = "itPorygon2_UnkMotion0_Phys")]
#[doc(alias = "itPorygon2_UnkMotion1_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itPorygon2_UnkMotion0_Coll`: the airborne pass, its result unused.
#[doc(alias = "itPorygon2_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj);
    false
}

/// Runs again, `it_802D5C00`, with its sound.
#[doc(alias = "it_802D5C00")]
pub fn leave(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    play_sfx(r, gobj, 0x273F, 0x7F, 0x40);
}

/// `itPorygon2_UnkMotion1_Anim`: gone when its animation ends, else its body's motion taken
/// as its own.
#[doc(alias = "itPorygon2_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        return true;
    }
    let body = table_bone(r.game, gobj, 1);
    take_bone_motion(r, body, gobj);
    false
}

/// `itPorygon2_UnkMotion1_Coll`: nothing.
#[doc(alias = "itPorygon2_UnkMotion1_Coll")]
pub fn coll1(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Forgets a GObj going away, `it_802D5CD8` (`it_8026B894`).
#[doc(alias = "it_802D5CD8")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
