//! The Star Rod, `melee/it/kinds/itstarrod.c`: a swung item with charges (`xD4C`) that falls,
//! lands and is held; each swing's script flag fires a star (`itstarrodstar.c`) while charges
//! last. Thrown, it bounces off what it hits; empty, it falls through the stage.

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725};
use super::{falling_phys, starrodstar};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_2360;
pub const ANIM0: u32 = 0x8029_2450;
pub const PHYS0: u32 = 0x8029_2458;
pub const COLL0: u32 = 0x8029_245C;
pub const ANIM4: u32 = 0x8029_24B0;
pub const PHYS1: u32 = 0x8029_24B8;
pub const COLL1: u32 = 0x8029_24E8;
pub const PICKED_UP: u32 = 0x8029_2514;
pub const ANIM2: u32 = 0x8029_253C;
pub const PHYS2: u32 = 0x8029_2544;
pub const DROPPED: u32 = 0x8029_2548;
pub const COLL4: u32 = 0x8029_2570;
pub const THROWN: u32 = 0x8029_25B4;
pub const PHYS4: u32 = 0x8029_25DC;
pub const COLL3: u32 = 0x8029_2628;
pub const DMG_DEALT: u32 = 0x8029_266C;
pub const ENTERED_AIR: u32 = 0x8029_26A8;
pub const ANIM5: u32 = 0x8029_26D0;
pub const PHYS5: u32 = 0x8029_26D8;
pub const COLL5: u32 = 0x8029_26DC;
pub const CLANKED: u32 = 0x8029_2710;
pub const REFLECTED: u32 = 0x8029_274C;
pub const HIT_SHIELD: u32 = 0x8029_276C;
pub const SHIELD_BOUNCED: u32 = 0x8029_27A8;

/// Runs one of the Star Rod's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM4 | ANIM2 | ANIM5 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 | COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Star Rod's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 | PHYS2 | PHYS5 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS4 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itStarRod_Logic22_Spawned`: its attributes' charges, falling.
#[doc(alias = "itStarRod_Logic22_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Star Rod without attributes");
    let charges = r.data.u32(a) as i32;
    item_mut(r.game, gobj).xD4C = charges;
    fell(r, gobj);
}

/// Where a star leaves from, in the hand's space (the attributes' `x4`), `it_80292394`.
#[doc(alias = "it_80292394")]
pub fn star_offset(r: &Running, gobj: GObjId) -> Vec3 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Star Rod without attributes");
    Vec3::new(
        r.data.f32(a.at(4)),
        r.data.f32(a.at(8)),
        r.data.f32(a.at(0xC)),
    )
}

/// Fires a star from `pos`, facing `facing` (`big` for a smash swing's), a charge spent,
/// `it_802923BC`.
#[doc(alias = "it_802923BC")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, facing: f32, big: i32) {
    let ip = item_mut(r.game, gobj);
    if ip.xD4C > 0 {
        ip.xD4C -= 1;
    }
    let owner = ip.owner.expect("a Star Rod fired with no owner");
    starrodstar::fire(r, owner, pos, big, facing);
}

/// Lands and lies still, `it_802923F8`.
#[doc(alias = "it_802923F8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itStarrod_UnkMotion0_Anim` (and 2, 4, 5).
#[doc(alias = "itStarrod_UnkMotion0_Anim")]
#[doc(alias = "itStarrod_UnkMotion2_Anim")]
#[doc(alias = "itStarrod_UnkMotion4_Anim")]
#[doc(alias = "itStarrod_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itStarrod_UnkMotion0_Phys` (and 2, 5).
#[doc(alias = "itStarrod_UnkMotion0_Phys")]
#[doc(alias = "itStarrod_UnkMotion2_Phys")]
#[doc(alias = "itStarrod_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itStarrod_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itStarrod_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80292488`.
#[doc(alias = "it_80292488")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itStarrod_UnkMotion1_Phys`: gravity.
#[doc(alias = "itStarrod_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itStarrod_UnkMotion1_Coll`: lands.
#[doc(alias = "itStarrod_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itStarRod_Logic22_PickedUp`.
#[doc(alias = "itStarRod_Logic22_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itStarRod_Logic22_Dropped`.
#[doc(alias = "itStarRod_Logic22_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itStarrod_UnkMotion4_Coll` (and 3's): dropped or thrown, with charges left it lands;
/// empty, it falls through, gone once it's past the stage.
#[doc(alias = "itStarrod_UnkMotion4_Coll")]
#[doc(alias = "itStarrod_UnkMotion3_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Thrown, `itStarRod_Logic22_Thrown`.
#[doc(alias = "itStarRod_Logic22_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown or dropped, gravity and the spin it slows, `itStarrod_UnkMotion4_Phys`.
#[doc(alias = "itStarrod_UnkMotion4_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// Bounces off what it hit when thrown or dropped, a clank or a shield,
/// `itStarRod_Logic22_DmgDealt` (and `Clanked`, `HitShield`).
#[doc(alias = "itStarRod_Logic22_DmgDealt")]
#[doc(alias = "itStarRod_Logic22_Clanked")]
#[doc(alias = "itStarRod_Logic22_HitShield")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    }
    false
}

/// Skidding off a slope, `itStarRod_Logic22_EnteredAir`.
#[doc(alias = "itStarRod_Logic22_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itStarrod_UnkMotion5_Coll`: skidding, it stops or falls.
#[doc(alias = "itStarrod_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Sent back by a reflection, `itStarRod_Logic22_Reflected`.
#[doc(alias = "itStarRod_Logic22_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itStarRod_Logic22_ShieldBounced`.
#[doc(alias = "itStarRod_Logic22_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}
