//! The Ray Gun, `melee/it/kinds/itlgun.c`: a shooting item that falls, lands and is held
//! (its shots, `it_8028E79C`, come with the fighter's shooting states, not ported).

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725};
use super::falling_phys;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8028_E738;
pub const ANIM0: u32 = 0x8028_E828;
pub const PHYS0: u32 = 0x8028_E830;
pub const COLL0: u32 = 0x8028_E834;
pub const FELL: u32 = 0x8028_E860;
pub const ANIM1: u32 = 0x8028_E888;
pub const PHYS1: u32 = 0x8028_E890;
pub const COLL1: u32 = 0x8028_E8C0;
pub const PICKED_UP: u32 = 0x8028_E904;
pub const ANIM2: u32 = 0x8028_E92C;
pub const PHYS2: u32 = 0x8028_E934;
pub const ANIM3: u32 = 0x8028_E96C;
pub const PHYS3: u32 = 0x8028_E9B4;
pub const DROPPED: u32 = 0x8028_E9B8;
pub const THROWN: u32 = 0x8028_E9E0;
pub const PHYS4: u32 = 0x8028_EA08;
pub const DMG_DEALT: u32 = 0x8028_EA54;
pub const CLANKED: u32 = 0x8028_EA78;
pub const HIT_SHIELD: u32 = 0x8028_EA9C;
pub const REFLECTED: u32 = 0x8028_EAC0;
pub const SHIELD_BOUNCED: u32 = 0x8028_EAE0;
pub const ENTERED_AIR: u32 = 0x8028_EB00;
pub const ANIM5: u32 = 0x8028_EB28;
pub const PHYS5: u32 = 0x8028_EB30;
pub const COLL5: u32 = 0x8028_EB34;

/// Runs one of the Ray Gun's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM5 => still(r, gobj),
        ANIM3 => recoil(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL5 => coll5(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => bounce(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Ray Gun's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS3 | PHYS5 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => dropped(r, gobj),
        PHYS4 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itLGun_Logic16_Spawned`: loaded with its attributes' shots, falling.
#[doc(alias = "itLGun_Logic16_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Ray Gun without attributes");
    let ammo = r.data.u32(a) as i32;
    let ip = item_mut(r.game, gobj);
    ip.xD4C = ammo;
    ip.xDD4_itemVar.lgun().timer.set(0);
    fell(r, gobj);
}

/// Stops and stands, `it_8028E7D8`.
#[doc(alias = "it_8028E7D8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itLgun_UnkMotion0_Anim` (2, 4, 5, and 1's).
#[doc(alias = "itLgun_UnkMotion0_Anim")]
#[doc(alias = "itLgun_UnkMotion2_Anim")]
#[doc(alias = "itLgun_UnkMotion4_Anim")]
#[doc(alias = "itLgun_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itLgun_UnkMotion0_Phys` (2, 3, 5).
#[doc(alias = "itLgun_UnkMotion0_Phys")]
#[doc(alias = "itLgun_UnkMotion2_Phys")]
#[doc(alias = "itLgun_UnkMotion3_Phys")]
#[doc(alias = "itLgun_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itLgun_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itLgun_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8028E860`.
#[doc(alias = "it_8028E860")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itLgun_UnkMotion1_Phys`.
#[doc(alias = "itLgun_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itLgun_UnkMotion4_Coll` (state 1's too): with shots left it lands; empty, it falls
/// through, gone once it's past the stage.
#[doc(alias = "itLgun_UnkMotion4_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Held, `itLGun_Logic16_PickedUp`.
#[doc(alias = "itLGun_Logic16_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// A shot's recoil, `it_8028E938`.
#[doc(alias = "it_8028E938")]
pub fn shot(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDD4_itemVar.lgun().timer.set(40);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// `itLgun_UnkMotion3_Anim`: the recoil counts down, then back to held.
#[doc(alias = "itLgun_UnkMotion3_Anim")]
pub fn recoil(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.lgun();
    let t = v.timer.get().wrapping_sub(1);
    v.timer.set(t);
    if t <= 0 {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    }
    false
}

/// Let go of or thrown, it falls, `itLGun_Logic16_Dropped` and `itLGun_Logic16_Thrown`.
#[doc(alias = "itLGun_Logic16_Dropped")]
#[doc(alias = "itLGun_Logic16_Thrown")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Bounces off what it hit, a clank or a shield, `itLGun_Logic16_DmgDealt` (and
/// `Clanked`, `HitShield`).
#[doc(alias = "itLGun_Logic16_DmgDealt")]
#[doc(alias = "itLGun_Logic16_Clanked")]
#[doc(alias = "itLGun_Logic16_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Sent back by a reflection, `itLGun_Logic16_Reflected`.
#[doc(alias = "itLGun_Logic16_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itLGun_Logic16_ShieldBounced`.
#[doc(alias = "itLGun_Logic16_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, `itLGun_Logic16_EnteredAir`.
#[doc(alias = "itLGun_Logic16_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itLgun_UnkMotion5_Coll`: skidding, it stops or falls.
#[doc(alias = "itLgun_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`), `itLgun_UnkMotion4_Phys`.
#[doc(alias = "itLgun_UnkMotion4_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}
