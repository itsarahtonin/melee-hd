//! The home-run bat, `melee/it/kinds/itbat.c`: a battering item that falls, is swung, and
//! bounces off what it hits when thrown.

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_4910;
pub const LANDED: u32 = 0x8028_4974;
pub const ANIM0: u32 = 0x8028_49B8;
pub const PHYS0: u32 = 0x8028_49C0;
pub const COLL0: u32 = 0x8028_49C4;
pub const FELL: u32 = 0x8028_49F0;
pub const ANIM3: u32 = 0x8028_4A2C;
pub const PHYS3: u32 = 0x8028_4A34;
pub const COLL3: u32 = 0x8028_4A80;
pub const PICKED_UP: u32 = 0x8028_4AAC;
pub const ANIM2: u32 = 0x8028_4B1C;
pub const PHYS2: u32 = 0x8028_4B24;
pub const COLL2: u32 = 0x8028_4B28;
pub const DROPPED: u32 = 0x8028_4B30;
pub const THROWN: u32 = 0x8028_4BA0;
pub const ENTERED_AIR: u32 = 0x8028_4C10;
pub const ANIM4: u32 = 0x8028_4C38;
pub const PHYS4: u32 = 0x8028_4C40;
pub const COLL4: u32 = 0x8028_4C44;
pub const DMG_DEALT: u32 = 0x8028_4C78;
pub const REFLECTED: u32 = 0x8028_4CAC;
pub const CLANKED: u32 = 0x8028_4CCC;
pub const HIT_SHIELD: u32 = 0x8028_4CF0;
pub const SHIELD_BOUNCED: u32 = 0x8028_4D14;

/// Runs one of the bat's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM2 | ANIM4 | COLL2 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED | HIT_SHIELD => bounce(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bat's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS3 => phys3(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itBat_Logic11_Spawned`: popping up, falling.
#[doc(alias = "itBat_Logic11_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a bat without attributes");
    let vy = r.data.f32(a.at(0xC));
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, vy, 0.0);
    fell(r, gobj);
}

/// Lands, stopped, `it_80284974`.
#[doc(alias = "it_80284974")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.xD50_landNum = 0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that do nothing, `itBat_UnkMotion0_Anim` (and 2, 3, 4, and 2's collision).
#[doc(alias = "itBat_UnkMotion0_Anim")]
#[doc(alias = "itBat_UnkMotion2_Anim")]
#[doc(alias = "itBat_UnkMotion3_Anim")]
#[doc(alias = "itBat_UnkMotion4_Anim")]
#[doc(alias = "itBat_UnkMotion2_Coll")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itBat_UnkMotion0_Phys` (2, 4).
#[doc(alias = "itBat_UnkMotion0_Phys")]
#[doc(alias = "itBat_UnkMotion2_Phys")]
#[doc(alias = "itBat_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itBat_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itBat_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_802849F0`.
#[doc(alias = "it_802849F0")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).xD50_landNum = 0;
}

/// `itBat_UnkMotion3_Phys`: falling and spinning.
#[doc(alias = "itBat_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itBat_UnkMotion3_Coll`: lands.
#[doc(alias = "itBat_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

fn show(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
}

/// Shown and held, `itBat_Logic11_PickedUp`.
#[doc(alias = "itBat_Logic11_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    show(r, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    item_mut(r.game, gobj).xD50_landNum = 0;
}

/// Let go of or thrown, it falls, `itBat_Logic11_Dropped` and `itBat_Logic11_Thrown`.
#[doc(alias = "itBat_Logic11_Dropped")]
#[doc(alias = "itBat_Logic11_Thrown")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Skidding off a slope, `itBat_Logic11_EnteredAir`.
#[doc(alias = "itBat_Logic11_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itBat_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itBat_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Bounces off what it hit when thrown, `itBat_Logic11_DmgDealt`.
#[doc(alias = "itBat_Logic11_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    }
    false
}

/// Sent back by a reflection, `itBat_Logic11_Reflected`.
#[doc(alias = "itBat_Logic11_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a clank or a shield, `itBat_Logic11_Clanked` and `itBat_Logic11_HitShield`.
#[doc(alias = "itBat_Logic11_Clanked")]
#[doc(alias = "itBat_Logic11_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Bounces off a shield, `itBat_Logic11_ShieldBounced`.
#[doc(alias = "itBat_Logic11_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}
