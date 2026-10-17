//! The Bunny Hood, `melee/it/kinds/itrabbitc.c`: it falls and rests; the fighter that picks it
//! up wears it (`ftCommon_8007FA58`), its ears and parts fitted to the head
//! (`it_80294E78`, `it_80294EB0`), its shine shown or hidden (`it_802950D4`).

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{change, change_state};
use super::super::{item_mut, lib26b1, lib2725};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SHIELD_BOUNCED: u32 = 0x8029_4DA0;
pub const SPAWNED: u32 = 0x8029_5118;
pub const ANIM0: u32 = 0x8029_5188;
pub const PHYS0: u32 = 0x8029_5190;
pub const COLL0: u32 = 0x8029_5194;
pub const ANIM3: u32 = 0x8029_51E8;
pub const PHYS1: u32 = 0x8029_51F0;
pub const COLL1: u32 = 0x8029_5220;
pub const PICKED_UP: u32 = 0x8029_524C;
pub const ANIM2: u32 = 0x8029_5290;
pub const DROPPED: u32 = 0x8029_5298;
pub const PHYS3: u32 = 0x8029_52DC;
pub const COLL3: u32 = 0x8029_530C;
pub const ENTERED_AIR: u32 = 0x8029_5340;
pub const ANIM4: u32 = 0x8029_5368;
pub const PHYS4: u32 = 0x8029_5370;
pub const COLL4: u32 = 0x8029_5374;

/// Runs one of the Bunny Hood's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Bunny Hood's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 | PHYS4 => idle(r, gobj),
        PHYS1 | PHYS3 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itRabbitC_Logic30_ShieldBounced`: bounces off a shield.
#[doc(alias = "itRabbitC_Logic30_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Scales the ears (bone 1) to `scale`, `it_80294E78`.
#[doc(alias = "it_80294E78")]
pub fn scale_ears(r: &mut Running, gobj: GObjId, scale: f32) {
    let j = lib2725::bone(r.game, r.data, gobj, 1).expect("a Bunny Hood without its bones");
    lib2725::set_uniform_scale(r.game, j, scale);
}

/// Moves bone 8 by `a` and bone 4 by `b`, `it_80294EB0`.
#[doc(alias = "it_80294EB0")]
pub fn shift_parts(r: &mut Running, gobj: GObjId, a: Vec3, b: Vec3) {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    for (bone, d) in [(8, a), (4, b)] {
        let j = lib2725::bone(r.game, r.data, gobj, bone).expect("a Bunny Hood without its bones");
        let t = r.game.objs.jobjs[j].translate;
        let t = Vec3::new(
            mem(fp::fadds(reg(t.x), reg(d.x))),
            mem(fp::fadds(reg(t.y), reg(d.y))),
            mem(fp::fadds(reg(t.z), reg(d.z))),
        );
        super::super::item::set_translate(r.game, j, t);
    }
}

/// Shows (`on`) or hides its shine, bone 3, `it_802950D4`.
#[doc(alias = "it_802950D4")]
pub fn show_shine(r: &mut Running, gobj: GObjId, on: bool) {
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    if on {
        lib2725::show_tree(r.game, j);
    } else {
        lib2725::hide_tree(r.game, j);
    }
}

/// `itRabbitC_Logic31_Spawned`: falling.
#[doc(alias = "itRabbitC_Logic31_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    fell(r, gobj);
}

/// Stops and rests, `it_80295138`.
#[doc(alias = "it_80295138")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itRabbitc_UnkMotion0_Anim` (2, 3 for 1 and 3, 4).
#[doc(alias = "itRabbitc_UnkMotion0_Anim")]
#[doc(alias = "itRabbitc_UnkMotion2_Anim")]
#[doc(alias = "itRabbitc_UnkMotion3_Anim")]
#[doc(alias = "itRabbitc_UnkMotion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itRabbitc_UnkMotion0_Phys` and 4.
#[doc(alias = "itRabbitc_UnkMotion0_Phys")]
#[doc(alias = "itRabbitc_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itRabbitc_UnkMotion0_Coll`: resting, it falls off ledges.
#[doc(alias = "itRabbitc_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_802951C0`.
#[doc(alias = "it_802951C0")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itRabbitc_UnkMotion1_Phys` and 3.
#[doc(alias = "itRabbitc_UnkMotion1_Phys")]
#[doc(alias = "itRabbitc_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itRabbitc_UnkMotion1_Coll`: lands.
#[doc(alias = "itRabbitc_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, its shine hidden, `itRabbitC_Logic31_PickedUp`.
#[doc(alias = "itRabbitC_Logic31_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    lib2725::hide_tree(r.game, j);
}

/// Let go of, it falls with its shine shown, `itRabbitC_Logic31_Dropped`.
#[doc(alias = "itRabbitC_Logic31_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    let j = lib2725::bone(r.game, r.data, gobj, 3);
    lib2725::show_tree(r.game, j);
}

/// `itRabbitc_UnkMotion3_Coll`: thrown, gone when it touches anything.
#[doc(alias = "itRabbitc_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// Skidding off a slope, `itRabbitC_Logic31_EnteredAir`.
#[doc(alias = "itRabbitC_Logic31_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itRabbitc_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itRabbitc_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj the hood points to, `itRabbitC_Logic31_EvtUnk` (`it_8026B894`).
#[doc(alias = "itRabbitC_Logic31_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
