//! The Screw Attack, `melee/it/kinds/itscball.c`: an item that falls, lands and is held; held,
//! the fighter's jumps spin as Samus's up special does (`ftCo_ItemScrew_*`, and
//! `ft_800D2E7C` for the aerial jump). Thrown, it breaks on what it hits.

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state};
use super::super::{lib26b1, lib2725};
use super::falling_phys;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_4AB0;
pub const ANIM0: u32 = 0x8029_4B20;
pub const PHYS0: u32 = 0x8029_4B28;
pub const COLL0: u32 = 0x8029_4B2C;
pub const ANIM1: u32 = 0x8029_4B80;
pub const PHYS1: u32 = 0x8029_4B88;
pub const COLL1: u32 = 0x8029_4BB8;
pub const PICKED_UP: u32 = 0x8029_4BE4;
pub const ANIM2: u32 = 0x8029_4C0C;
pub const DROPPED: u32 = 0x8029_4C14;
pub const THROWN: u32 = 0x8029_4C3C;
pub const ANIM3: u32 = 0x8029_4C64;
pub const PHYS3: u32 = 0x8029_4C6C;
pub const COLL3: u32 = 0x8029_4CB8;
pub const ENTERED_AIR: u32 = 0x8029_4CE4;
pub const ANIM4: u32 = 0x8029_4D0C;
pub const PHYS4: u32 = 0x8029_4D14;
pub const COLL4: u32 = 0x8029_4D18;
pub const DMG_DEALT: u32 = 0x8029_4D4C;
pub const CLANKED: u32 = 0x8029_4D54;
pub const REFLECTED: u32 = 0x8029_4D5C;
pub const HIT_SHIELD: u32 = 0x8029_4D7C;

/// Runs one of the Screw Attack's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM3 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 | COLL3 => coll1(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT | CLANKED => broken(r, gobj),
        REFLECTED => reflected(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Screw Attack's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => fell(r, gobj),
        PHYS0 | PHYS4 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS3 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// Lands and lies still, `it_80294AD0`.
#[doc(alias = "it_80294AD0")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itScball_UnkMotion0_Anim` (and 1 to 4).
#[doc(alias = "itScball_UnkMotion0_Anim")]
#[doc(alias = "itScball_UnkMotion1_Anim")]
#[doc(alias = "itScball_UnkMotion2_Anim")]
#[doc(alias = "itScball_UnkMotion3_Anim")]
#[doc(alias = "itScball_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itScball_UnkMotion0_Phys` (and 4).
#[doc(alias = "itScball_UnkMotion0_Phys")]
#[doc(alias = "itScball_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itScball_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itScball_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80294B58` (and `itScball_Logic30_Spawned`).
#[doc(alias = "it_80294B58")]
#[doc(alias = "itScball_Logic30_Spawned")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itScball_UnkMotion1_Phys`: gravity.
#[doc(alias = "itScball_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itScball_UnkMotion1_Coll` (and 3's): lands.
#[doc(alias = "itScball_UnkMotion1_Coll")]
#[doc(alias = "itScball_UnkMotion3_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itScball_Logic30_PickedUp`.
#[doc(alias = "itScball_Logic30_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itScball_Logic30_Dropped`.
#[doc(alias = "itScball_Logic30_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown, `itScball_Logic30_Thrown`.
#[doc(alias = "itScball_Logic30_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown, gravity and the spin it slows, `itScball_UnkMotion3_Phys`.
#[doc(alias = "itScball_UnkMotion3_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// Skidding off a slope, `itScball_Logic30_EnteredAir`.
#[doc(alias = "itScball_Logic30_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itScball_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itScball_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Breaks on what it hits or clanks with, `itScball_Logic30_DmgDealt` (and `Clanked`).
#[doc(alias = "itScball_Logic30_DmgDealt")]
#[doc(alias = "itScball_Logic30_Clanked")]
pub fn broken(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Sent back by a reflection, `itScball_Logic30_Reflected`.
#[doc(alias = "itScball_Logic30_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield it hits, `itScball_Logic30_HitShield`.
#[doc(alias = "itScball_Logic30_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}
