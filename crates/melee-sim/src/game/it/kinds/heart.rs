//! The heart container, `melee/it/kinds/itheart.c`: it falls, spinning, and heals whoever
//! picks it up.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_3BEC;
pub const DESTROYED: u32 = 0x8028_3C48;
pub const LANDED: u32 = 0x8028_3C7C;
pub const ANIM0: u32 = 0x8028_3CD4;
pub const PHYS0: u32 = 0x8028_3DA4;
pub const COLL0: u32 = 0x8028_3DA8;
pub const FELL: u32 = 0x8028_3DD4;
pub const ANIM3: u32 = 0x8028_3DFC;
pub const PHYS3: u32 = 0x8028_3ECC;
pub const COLL3: u32 = 0x8028_3EFC;
pub const PICKED_UP: u32 = 0x8028_3F28;
pub const ANIM2: u32 = 0x8028_4020;
pub const PHYS2: u32 = 0x8028_4028;
pub const DROPPED: u32 = 0x8028_402C;
pub const ENTERED_AIR: u32 = 0x8028_412C;
pub const ANIM4: u32 = 0x8028_4154;
pub const PHYS4: u32 = 0x8028_415C;
pub const COLL4: u32 = 0x8028_4160;

/// Runs one of the heart's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 => spin(r, gobj),
        ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the heart's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS2 | PHYS4 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS3 => phys3(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `HeartContainerAttr`'s field at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a heart without attributes")
        .at(off)
}

/// The heal from the attributes' second word, `it_80283BD4`.
#[doc(alias = "it_80283BD4")]
pub fn second_heal(r: &mut Running, gobj: GObjId) {
    let h = r.data.u32(attr(r, gobj, 4)) as i32;
    item_mut(r.game, gobj).xDD4_itemVar.heart().xDD4_heal.set(h);
}

/// `itHeart_Logic8_Spawned`: popping up, falling.
#[doc(alias = "itHeart_Logic8_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let vy = r.data.f32(attr(r, gobj, 0x14));
    let heal = r.data.u32(attr(r, gobj, 0)) as i32;
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, vy, 0.0);
    let v = ip.xDD4_itemVar.heart();
    v.xDD4_heal.set(heal);
    v.xDD8.get()[0] &= !0x80;
    v.xDDC.set(0);
    fell(r, gobj);
}

/// `itHeart_Logic8_Destroyed`: a heart from an event frees its slot.
#[doc(alias = "itHeart_Logic8_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).xDD4_itemVar.heart().xDD8.get()[0] & 0x80 != 0 {
        code::unported_call("gm_80473A18 (an event's heart slots)");
    }
}

/// Lands, stopped, `it_80283C7C`.
#[doc(alias = "it_80283C7C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, 2);
}

/// Spins its model about y, `itHeart_UnkMotion0_Anim` and `itHeart_UnkMotion3_Anim`.
#[doc(alias = "itHeart_UnkMotion0_Anim")]
#[doc(alias = "itHeart_UnkMotion3_Anim")]
pub fn spin(r: &mut Running, gobj: GObjId) -> bool {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let speed = r.data.f32(attr(r, gobj, 0x18));
    add_rotation_y(r, child, speed);
    false
}

/// `HSD_JObjAddRotationY` (inline).
fn add_rotation_y(r: &mut Running, j: Option<crate::hsd::jobj::JObjId>, v: f32) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), reg(v)));
    });
}

/// The states that animate nothing, `itHeart_UnkMotion2_Anim` and 4.
#[doc(alias = "itHeart_UnkMotion2_Anim")]
#[doc(alias = "itHeart_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itHeart_UnkMotion0_Phys` (2, 4).
#[doc(alias = "itHeart_UnkMotion0_Phys")]
#[doc(alias = "itHeart_UnkMotion2_Phys")]
#[doc(alias = "itHeart_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itHeart_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itHeart_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80283DD4`.
#[doc(alias = "it_80283DD4")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 2);
}

/// `itHeart_UnkMotion3_Phys`: gravity.
#[doc(alias = "itHeart_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itHeart_UnkMotion3_Coll`: lands.
#[doc(alias = "itHeart_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Shown, square, held, `itHeart_Logic8_PickedUp`.
#[doc(alias = "itHeart_Logic8_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    show_square(r, gobj);
    change_state(r, gobj, 2, 2);
}

/// Shows the model and squares it.
fn show_square(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    jobj::set_rotation_y(&mut r.game.objs.jobjs, child.expect("jobj"), 0.0);
}

/// Let go of, it falls, `itHeart_Logic8_Dropped`.
#[doc(alias = "itHeart_Logic8_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    show_square(r, gobj);
    change_state(r, gobj, 3, 6);
}

/// Skidding off a slope, `itHeart_Logic8_EnteredAir`.
#[doc(alias = "itHeart_Logic8_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, 2);
}

/// `itHeart_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itHeart_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}
