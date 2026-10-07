//! The Maxim Tomato, `melee/it/kinds/ittomato.c`: it falls and heals whoever picks it up.

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_42A4;
pub const DESTROYED: u32 = 0x8028_4324;
pub const FELL: u32 = 0x8028_4358;
pub const ANIM0: u32 = 0x8028_4380;
pub const PHYS0: u32 = 0x8028_4388;
pub const COLL0: u32 = 0x8028_43B8;
pub const LANDED: u32 = 0x8028_43E4;
pub const ANIM1: u32 = 0x8028_4420;
pub const PHYS1: u32 = 0x8028_4428;
pub const COLL1: u32 = 0x8028_442C;
pub const SLIPPED: u32 = 0x8028_4458;
pub const ANIM4: u32 = 0x8028_4480;
pub const PHYS4: u32 = 0x8028_4488;
pub const COLL4: u32 = 0x8028_44B8;
pub const PICKED_UP: u32 = 0x8028_44E4;
pub const ANIM3: u32 = 0x8028_4540;
pub const PHYS3: u32 = 0x8028_4548;
pub const DROPPED: u32 = 0x8028_454C;
pub const ENTERED_AIR: u32 = 0x8028_45BC;
pub const ANIM5: u32 = 0x8028_45E4;
pub const PHYS5: u32 = 0x8028_45EC;
pub const COLL5: u32 = 0x8028_45F0;

/// Runs one of the tomato's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM4 | ANIM3 | ANIM5 => still(r, gobj),
        COLL0 | COLL4 => falling(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL5 => coll5(r, gobj),
        _ => return None,
    })
}

/// Runs one of the tomato's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS4 => gravity(r, gobj),
        PHYS1 | PHYS3 | PHYS5 => idle(r, gobj),
        LANDED => landed(r, gobj),
        SLIPPED => slipped(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `MaximTomatoSpecialAttr`'s word at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a tomato without attributes")
        .at(off)
}

/// The heal from the attributes' second word, `it_8028428C`.
#[doc(alias = "it_8028428C")]
pub fn second_heal(r: &mut Running, gobj: GObjId) {
    let h = r.data.u32(attr(r, gobj, 4)) as i32;
    item_mut(r.game, gobj).xDD4_itemVar.tomato().heal_amount = h;
}

/// `itTomato_Logic9_Spawned`: popping up, falling.
#[doc(alias = "itTomato_Logic9_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    let vy = r.data.f32(attr(r, gobj, 0x14));
    let heal = r.data.u32(attr(r, gobj, 0)) as i32;
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, vy, 0.0);
    let v = ip.xDD4_itemVar.tomato();
    v.heal_amount = heal;
    v.x4_b0 = false;
    v.x8 = 0;
    fell(r, gobj);
}

/// `itTomato_Logic9_Destroyed`: a tomato from an event frees its slot.
#[doc(alias = "itTomato_Logic9_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).xDD4_itemVar.tomato().x4_b0 {
        code::unported_call("gm_80473A18 (an event's tomato slots)");
    }
}

/// Falls, `it_80284358`.
#[doc(alias = "it_80284358")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itTomato_UnkMotion0_Anim` (1, 3, 4, 5).
#[doc(alias = "itTomato_UnkMotion0_Anim")]
#[doc(alias = "itTomato_UnkMotion1_Anim")]
#[doc(alias = "itTomato_UnkMotion3_Anim")]
#[doc(alias = "itTomato_UnkMotion4_Anim")]
#[doc(alias = "itTomato_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itTomato_UnkMotion1_Phys` (3, 5).
#[doc(alias = "itTomato_UnkMotion1_Phys")]
#[doc(alias = "itTomato_UnkMotion3_Phys")]
#[doc(alias = "itTomato_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Gravity, `itTomato_UnkMotion0_Phys` and `itTomato_UnkMotion4_Phys`.
#[doc(alias = "itTomato_UnkMotion0_Phys")]
#[doc(alias = "itTomato_UnkMotion4_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Lands, `itTomato_UnkMotion0_Coll` and `itTomato_UnkMotion4_Coll`.
#[doc(alias = "itTomato_UnkMotion0_Coll")]
#[doc(alias = "itTomato_UnkMotion4_Coll")]
pub fn falling(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Lands, stopped, `it_802843E4`.
#[doc(alias = "it_802843E4")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itTomato_UnkMotion1_Coll`: falls off ledges.
#[doc(alias = "itTomato_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, slipped);
    false
}

/// Falls off a ledge, `it_80284458`.
#[doc(alias = "it_80284458")]
pub fn slipped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Shown and held, `itTomato_Logic9_PickedUp`.
#[doc(alias = "itTomato_Logic9_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    show(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

fn show(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
}

/// Let go of, it falls, `itTomato_Logic9_Dropped`.
#[doc(alias = "itTomato_Logic9_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Skidding off a slope, `itTomato_Logic9_EnteredAir`.
#[doc(alias = "itTomato_Logic9_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itTomato_UnkMotion5_Coll`: skidding, it stops or falls.
#[doc(alias = "itTomato_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, slipped);
    false
}
