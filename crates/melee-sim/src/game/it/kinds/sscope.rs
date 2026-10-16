//! The Super Scope, `melee/it/kinds/itsscope.c`: a shooting item that falls, lands and is
//! held, with its ammunition (`xD4C`), the cost of each charge, and its shots
//! ([`sscopebeam`](super::sscopebeam)), which the fighter's scope states fire.

use gekko_fp as fp;

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725};
use super::{falling_phys, sscopebeam};
use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_1C98;
pub const ANIM0: u32 = 0x8029_2080;
pub const PHYS0: u32 = 0x8029_2088;
pub const COLL0: u32 = 0x8029_208C;
pub const FELL: u32 = 0x8029_20B8;
pub const ANIM1: u32 = 0x8029_20E0;
pub const PHYS1: u32 = 0x8029_20E8;
pub const COLL1: u32 = 0x8029_2118;
pub const PICKED_UP: u32 = 0x8029_215C;
pub const ANIM2: u32 = 0x8029_2184;
pub const PHYS2: u32 = 0x8029_218C;
pub const DROPPED: u32 = 0x8029_2190;
pub const THROWN: u32 = 0x8029_21B8;
pub const PHYS3: u32 = 0x8029_21E0;
pub const DMG_DEALT: u32 = 0x8029_222C;
pub const CLANKED: u32 = 0x8029_2250;
pub const HIT_SHIELD: u32 = 0x8029_2274;
pub const REFLECTED: u32 = 0x8029_2298;
pub const SHIELD_BOUNCED: u32 = 0x8029_22B8;
pub const ENTERED_AIR: u32 = 0x8029_22D8;
pub const ANIM4: u32 = 0x8029_2300;
pub const PHYS4: u32 = 0x8029_2308;
pub const COLL4: u32 = 0x8029_230C;

/// Runs one of the Super Scope's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => bounce(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Super Scope's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => dropped(r, gobj),
        PHYS3 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itSScopeAttributes`: the ammunition (0), the charge below which a shot is the smallest
/// (4), the charges a level takes, by eighths (8), each level's cost (0xC, ten floats) and
/// the muzzle's offset (0x34).
fn attrs(r: &Running, gobj: GObjId) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Super Scope without attributes")
}

/// `itSScope_Logic21_Spawned`: its attributes' ammunition, falling.
#[doc(alias = "itSScope_Logic21_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ammo = r.data.u32(attrs(r, gobj)) as i32;
    item_mut(r.game, gobj).xD4C = ammo;
    fell(r, gobj);
}

/// Where the shots leave the muzzle, in the hand's space, `it_80291CCC`.
#[doc(alias = "it_80291CCC")]
pub fn muzzle(r: &Running, gobj: GObjId) -> Vec3 {
    crate::game::mirror::read_vec3(r.data, attrs(r, gobj).at(0x34))
}

/// The level a charge of `charge` ticks makes, 0 to 9, `it_80291CF4`.
#[doc(alias = "it_80291CF4")]
pub fn charge_level(r: &Running, gobj: GObjId, charge: i32) -> i32 {
    let a = attrs(r, gobj);
    let (low, step) = (r.data.u32(a.at(4)) as i32, r.data.u32(a.at(8)) as i32);
    if charge <= low {
        0
    } else if charge < step.wrapping_mul(8) {
        charge / 8
    } else {
        9
    }
}

/// What a shot of `level` costs, `it_80291D38`: its float, truncated.
#[doc(alias = "it_80291D38")]
pub fn cost(r: &Running, gobj: GObjId, level: i32) -> i32 {
    assert!(
        (0..=9).contains(&level),
        "it_80291D38 returns a register it never set for level {level}"
    );
    fp::fctiwz(f64::from(
        r.data.f32(attrs(r, gobj).at(0xC + 4 * level as u32)),
    ))
}

/// The level a shot of `charge` ticks fires with what's left, `it_80291DAC`: none (-1) when
/// empty; the charge's level when it can afford it; else, going down from that level, the
/// first level it can afford exactly, or the one above the first it can afford with some
/// left. When even level 1 is too dear, the original returns its GObj's address, which
/// stops here.
#[doc(alias = "it_80291DAC")]
pub fn affordable(r: &Running, gobj: GObjId, charge: i32) -> i32 {
    let ammo = item(r.game, gobj).xD4C;
    if ammo <= 0 {
        return -1;
    }
    let mut level = charge_level(r, gobj, charge);
    if ammo.wrapping_sub(cost(r, gobj, level)) >= 0 {
        return level;
    }
    while level > 0 {
        let left = ammo.wrapping_sub(cost(r, gobj, level));
        if left == 0 {
            return level;
        }
        if left > 0 {
            return level + 1;
        }
        level -= 1;
    }
    code::unported_call("it_80291DAC with no level affordable (it returns its GObj)")
}

/// Spends a shot of `level`, to no less than none, `it_80291F14`.
#[doc(alias = "it_80291F14")]
pub fn spend(r: &mut Running, gobj: GObjId, level: i32) {
    let c = cost(r, gobj, level);
    let ip = item_mut(r.game, gobj);
    ip.xD4C = ip.xD4C.wrapping_sub(c);
    if ip.xD4C < 0 {
        ip.xD4C = 0;
    }
}

/// Spends and fires a shot of `level` from `pos`, facing `facing`, `it_80291FA8`.
#[doc(alias = "it_80291FA8")]
pub fn shoot(r: &mut Running, gobj: GObjId, pos: Vec3, level: i32, facing: f32) {
    spend(r, gobj, level);
    let owner = item(r.game, gobj)
        .owner
        .expect("a Super Scope fired without an owner");
    sscopebeam::fire(r, owner, pos, level, facing);
}

/// Stops and stands, `it_80292030`.
#[doc(alias = "it_80292030")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itSscope_UnkMotion0_Anim` (2, 3 for 1 and 3, 4).
#[doc(alias = "itSscope_UnkMotion0_Anim")]
#[doc(alias = "itSscope_UnkMotion2_Anim")]
#[doc(alias = "itSscope_UnkMotion3_Anim")]
#[doc(alias = "itSscope_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itSscope_UnkMotion0_Phys` (2, 4).
#[doc(alias = "itSscope_UnkMotion0_Phys")]
#[doc(alias = "itSscope_UnkMotion2_Phys")]
#[doc(alias = "itSscope_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itSscope_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itSscope_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_802920B8`.
#[doc(alias = "it_802920B8")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itSscope_UnkMotion1_Phys`.
#[doc(alias = "itSscope_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itSscope_UnkMotion3_Coll` (states 1 and 3): with ammunition left it lands; empty, it
/// falls through, gone once it's past the stage.
#[doc(alias = "itSscope_UnkMotion3_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Held, `itSScope_Logic21_PickedUp`.
#[doc(alias = "itSScope_Logic21_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, it falls, `itSScope_Logic21_Dropped` and `itSScope_Logic21_Thrown`.
#[doc(alias = "itSScope_Logic21_Dropped")]
#[doc(alias = "itSScope_Logic21_Thrown")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Bounces off what it hit, a clank or a shield, `itSScope_Logic21_DmgDealt` (and
/// `Clanked`, `HitShield`).
#[doc(alias = "itSScope_Logic21_DmgDealt")]
#[doc(alias = "itSScope_Logic21_Clanked")]
#[doc(alias = "itSScope_Logic21_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Sent back by a reflection, `itSScope_Logic21_Reflected`.
#[doc(alias = "itSScope_Logic21_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itSScope_Logic21_ShieldBounced`.
#[doc(alias = "itSScope_Logic21_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, `itSScope_Logic21_EnteredAir`.
#[doc(alias = "itSScope_Logic21_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itSscope_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itSscope_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`), `itSscope_UnkMotion3_Phys`.
#[doc(alias = "itSscope_UnkMotion3_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}
