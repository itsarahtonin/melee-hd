//! Lip's Stick, `melee/it/kinds/itlipstick.c`: a swung item with charges (`xD4C`) that falls,
//! lands and is held; thrown, it bounces off what it touches (its spores, `it_8029A218`, and
//! its swing's, `it_80295770`, aren't ported).

use super::super::ground::{air, air_touches, fall, ground_or_fall, ground_skid, try_land};
use super::super::item::{article, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725, maplib};
use super::falling_phys;
use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_570C;
pub const LANDED: u32 = 0x8029_57AC;
pub const ANIM0: u32 = 0x8029_5804;
pub const PHYS0: u32 = 0x8029_580C;
pub const COLL0: u32 = 0x8029_5810;
pub const FELL: u32 = 0x8029_583C;
pub const ANIM1: u32 = 0x8029_5864;
pub const PHYS1: u32 = 0x8029_586C;
pub const COLL1: u32 = 0x8029_589C;
pub const PICKED_UP: u32 = 0x8029_58C8;
pub const ANIM2: u32 = 0x8029_58F0;
pub const PHYS2: u32 = 0x8029_5904;
pub const DROPPED: u32 = 0x8029_5908;
pub const COLL4: u32 = 0x8029_5930;
pub const THROWN: u32 = 0x8029_5974;
pub const PHYS4: u32 = 0x8029_599C;
pub const COLL3: u32 = 0x8029_59E8;
pub const DMG_DEALT: u32 = 0x8029_5ACC;
pub const ENTERED_AIR: u32 = 0x8029_5B08;
pub const ANIM5: u32 = 0x8029_5B30;
pub const PHYS5: u32 = 0x8029_5B38;
pub const COLL5: u32 = 0x8029_5B3C;
pub const CLANKED: u32 = 0x8029_5B70;
pub const REFLECTED: u32 = 0x8029_5BAC;
pub const HIT_SHIELD: u32 = 0x8029_5BCC;
pub const SHIELD_BOUNCED: u32 = 0x8029_5C08;

/// Runs one of Lip's Stick's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM5 => still(r, gobj),
        ANIM2 => held(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of Lip's Stick's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
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

/// `itLipstick_Logic23_Spawned`: its attributes' charges, falling.
#[doc(alias = "itLipstick_Logic23_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Lip's Stick without attributes");
    let charges = r.data.u32(a) as i32;
    let ip = item_mut(r.game, gobj);
    ip.xD4C = charges;
    ip.xDAC_itcmd_var0 = 0;
    fell(r, gobj);
}

/// Lands and lies still, `it_802957AC`.
#[doc(alias = "it_802957AC")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itLipstick_UnkMotion0_Anim` (4 for 1, 3, 4; and 5).
#[doc(alias = "itLipstick_UnkMotion0_Anim")]
#[doc(alias = "itLipstick_UnkMotion4_Anim")]
#[doc(alias = "itLipstick_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itLipstick_UnkMotion0_Phys` (2, 5).
#[doc(alias = "itLipstick_UnkMotion0_Phys")]
#[doc(alias = "itLipstick_UnkMotion2_Phys")]
#[doc(alias = "itLipstick_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itLipstick_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itLipstick_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029583C`.
#[doc(alias = "it_8029583C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itLipstick_UnkMotion1_Phys`.
#[doc(alias = "itLipstick_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itLipstick_UnkMotion1_Coll`: lands.
#[doc(alias = "itLipstick_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itLipstick_Logic23_PickedUp`.
#[doc(alias = "itLipstick_Logic23_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itLipstick_UnkMotion2_Anim`: held, it will spread spores if thrown.
#[doc(alias = "itLipstick_UnkMotion2_Anim")]
pub fn held(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
    false
}

/// Let go of, it falls, `itLipstick_Logic23_Dropped`.
#[doc(alias = "itLipstick_Logic23_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itLipstick_UnkMotion4_Coll`: dropped, with charges left it lands; empty, it falls
/// through, gone once it's past the stage.
#[doc(alias = "itLipstick_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Thrown, `itLipstick_Logic23_Thrown`.
#[doc(alias = "itLipstick_Logic23_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itLipstick_UnkMotion3_Coll`: thrown with charges left, the first thing it touches puffs
/// its spores, it bounces, and on the floor it lands once it settles; empty, it falls
/// through.
#[doc(alias = "itLipstick_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C == 0 {
        return air(r, gobj);
    }
    let t = air_touches(r, gobj);
    if t & 0xF != 0 {
        if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
            code::unported_call("it_8029A218 (Lip's Stick's spores)");
        }
        maplib::bounce(r, gobj, t);
        if t & 1 != 0 && try_land(r, gobj) {
            landed(r, gobj);
        }
    }
    false
}

/// Bounces off what it hit when thrown or dropped, a clank or a shield,
/// `itLipstick_Logic23_DmgDealt` (and `Clanked`, `HitShield`).
#[doc(alias = "itLipstick_Logic23_DmgDealt")]
#[doc(alias = "itLipstick_Logic23_Clanked")]
#[doc(alias = "itLipstick_Logic23_HitShield")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    }
    false
}

/// Sent back by a reflection, `itLipstick_Logic23_Reflected`.
#[doc(alias = "itLipstick_Logic23_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itLipstick_Logic23_ShieldBounced`.
#[doc(alias = "itLipstick_Logic23_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, `itLipstick_Logic23_EnteredAir`.
#[doc(alias = "itLipstick_Logic23_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itLipstick_UnkMotion5_Coll`: skidding, it stops or falls.
#[doc(alias = "itLipstick_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`), `itLipstick_UnkMotion4_Phys`.
#[doc(alias = "itLipstick_UnkMotion4_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}
