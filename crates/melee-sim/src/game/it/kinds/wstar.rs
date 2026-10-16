//! The Warp Star, `melee/it/kinds/itwstar.c`: it falls, then rests spinning; picked up, it
//! carries its rider off (the ride and the crash, `it_802946B0` to `it_802947CC`, come with
//! the fighter's Warp Star states, not ported).

use super::super::ground::{fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{effect, item, item_mut, lib26b1, lib2725, set_flag};
use crate::game::ef::SpawnKind;
use crate::game::ef::eflib;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_451C;
pub const LANDED: u32 = 0x8029_455C;
pub const ANIM0: u32 = 0x8029_45AC;
pub const PHYS0: u32 = 0x8029_45B4;
pub const COLL0: u32 = 0x8029_45F8;
pub const FELL: u32 = 0x8029_4624;
pub const ANIM1: u32 = 0x8029_464C;
pub const PHYS1: u32 = 0x8029_4654;
pub const COLL1: u32 = 0x8029_4684;
pub const PICKED_UP: u32 = 0x8029_4728;
pub const ANIM3: u32 = 0x8029_4788;
pub const DROPPED: u32 = 0x8029_4790;
pub const ENTERED_AIR: u32 = 0x8029_4A28;
pub const ANIM4: u32 = 0x8029_4A50;
pub const PHYS4: u32 = 0x8029_4A58;
pub const COLL4: u32 = 0x8029_4A5C;

/// Runs one of the Warp Star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM3 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Warp Star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => resting(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        PHYS4 => idle(r, gobj),
        _ => return false,
    }
    true
}

/// `itWStar_Logic29_Spawned`: its ride's scales at 1, falling.
#[doc(alias = "itWStar_Logic29_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    set_flag(&mut ip.xDCE_flag, 7, false);
    let v = ip.xDD4_itemVar.wstar();
    v.xDD8.set(1.0);
    v.xDD4.set(1.0);
    fell(r, gobj);
}

/// Stops and stands, `it_8029455C`.
#[doc(alias = "it_8029455C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itWstar_UnkMotion0_Anim` (1, 3 for 2 and 3, 4).
#[doc(alias = "itWstar_UnkMotion0_Anim")]
#[doc(alias = "itWstar_UnkMotion1_Anim")]
#[doc(alias = "itWstar_UnkMotion3_Anim")]
#[doc(alias = "itWstar_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Moves nothing, `itWstar_UnkMotion4_Phys`.
#[doc(alias = "itWstar_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itWstar_UnkMotion0_Phys`: resting, it spins at its attributes' rate.
#[doc(alias = "itWstar_UnkMotion0_Phys")]
pub fn resting(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Warp Star without attributes");
    let rate = r.data.f32(a);
    lib2725::clamp_fall(r.game, gobj, rate);
    lib2725::spin(r.game, r.data, gobj);
}

/// `itWstar_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itWstar_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80294624`.
#[doc(alias = "it_80294624")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itWstar_UnkMotion1_Phys`.
#[doc(alias = "itWstar_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itWstar_UnkMotion1_Coll`: lands.
#[doc(alias = "itWstar_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Picked up, with its glow on bone 3, `it_3F14_Logic29_PickedUp`.
#[doc(alias = "it_3F14_Logic29_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    effect::queue(r, gobj, SpawnKind::Attach, 0x43D, j, &[]);
}

/// Let go of, it falls, its effects gone, `itWStar_Logic29_Dropped`.
#[doc(alias = "itWStar_Logic29_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    eflib::destroy_all(r, gobj);
}

/// Skidding off a slope, `itWStar_Logic29_EnteredAir`.
#[doc(alias = "itWStar_Logic29_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itWstar_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itWstar_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}
