//! The hammer, `melee/it/kinds/ithammer.c`: swung by its holder (the fighter's hammer
//! states); a rare one loses its head (`xD4C` counts the heads left), which flies off as its own
//! item and leaves the head joint hidden.

use gekko_fp as fp;

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725};
use super::hammerhead;
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

pub const HITLAG: u32 = 0x8029_3D94;
pub const SPAWNED: u32 = 0x8029_3F58;
pub const LANDED: u32 = 0x8029_3F84;
pub const ANIM0: u32 = 0x8029_3FF4;
pub const PHYS0: u32 = 0x8029_3FFC;
pub const COLL0: u32 = 0x8029_4000;
pub const FELL: u32 = 0x8029_402C;
pub const ANIM4: u32 = 0x8029_4080;
pub const PHYS1: u32 = 0x8029_4088;
pub const COLL1: u32 = 0x8029_40B8;
pub const PICKED_UP: u32 = 0x8029_40FC;
pub const ANIM3: u32 = 0x8029_4170;
pub const HEADLESS: u32 = 0x8029_41A4;
pub const DROPPED: u32 = 0x8029_41F8;
pub const PHYS4: u32 = 0x8029_424C;
pub const COLL4: u32 = 0x8029_427C;
pub const ENTERED_AIR: u32 = 0x8029_42B0;
pub const ANIM5: u32 = 0x8029_4304;
pub const PHYS5: u32 = 0x8029_430C;
pub const COLL5: u32 = 0x8029_4310;

/// Runs one of the hammer's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM4 | ANIM5 => still(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        _ => return None,
    })
}

/// Runs one of the hammer's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        HITLAG => hitlag(r, gobj),
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS5 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS4 => phys_fall(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        HEADLESS => headless(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// Hides the head joint once no head is left (the file's inline `thing`).
fn hide_lost_head(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xD4C <= 0 {
        let head = lib2725::bone(r.game, r.data, gobj, 3);
        lib2725::hide_tree(r.game, head);
    }
}

/// Puts the holder in the item's hitlag too, `it_80293D94` (`it_8026BC14`).
#[doc(alias = "it_80293D94")]
#[doc(alias = "it_8026BC14")]
pub fn hitlag(r: &mut Running, gobj: GObjId) {
    let f = &item(r.game, gobj).xDC8_word.flags;
    if f.xB || !f.xA {
        return;
    }
    let ip = item(r.game, gobj);
    let (owner, frames) = (ip.owner, ip.xCBC_hitlagFrames);
    if let Some(o) = owner
        && super::super::item::is_fighter(r.game, owner)
    {
        r.game.ft_80086a4c(o, mem(fp::fsubs(reg(frames), 1.0)));
    }
}

/// The number of swings until the head flies off, `it_80293DCC`: 0 unless a draw under the
/// attributes' chance picks one in `[x0, x4)`.
#[doc(alias = "it_80293DCC")]
pub fn swings_to_lose_head(r: &mut Running, gobj: GObjId) -> i32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a hammer without attributes");
    let (lo, hi) = (r.data.u32(a), r.data.u32(a.at(4)));
    let chance = r.data.f32(a.at(8));
    if f64::from(r.game.objs.rng.draw_f32()) < fp::fdivs(reg(chance), 100.0) {
        return r.game.objs.rng.below(hi.wrapping_sub(lo) as i32) + lo as i32;
    }
    0
}

/// Throws the head off along the handle when one is left, `it_80293E34`.
#[doc(alias = "it_80293E34")]
pub fn lose_head(r: &mut Running, gobj: GObjId) {
    let left = item(r.game, gobj).xD4C;
    if left > 0 {
        item_mut(r.game, gobj).xD4C = left - 1;
        let head = lib2725::bone(r.game, r.data, gobj, 3);
        let head = world_position(&mut r.game.objs, r.data, head, None);
        let handle = lib2725::bone(r.game, r.data, gobj, 1);
        let handle = world_position(&mut r.game.objs, r.data, handle, None);
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(head.x), reg(handle.x))),
            mem(fp::fsubs(reg(head.y), reg(handle.y))),
            0.0,
        );
        dir.normalize();
        let facing = if reg(dir.x) < 0.0 { -1.0 } else { 1.0 };
        if reg(dir.x) < 0.0 {
            dir.x = -dir.x;
        }
        let owner = item(r.game, gobj)
            .owner
            .expect("a hammer losing its head unheld");
        hammerhead::spawn(r, owner, head, dir, facing);
    }
    hide_lost_head(r, gobj);
}

/// `itHammer_Logic28_Spawned`: one head, falling.
#[doc(alias = "itHammer_Logic28_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xD4C = 1;
    fell(r, gobj);
}

/// Lands, stopped, `it_80293F84`.
#[doc(alias = "it_80293F84")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    hide_lost_head(r, gobj);
}

/// The states that animate nothing, `itHammer_UnkMotion0_Anim` (4, 5).
#[doc(alias = "itHammer_UnkMotion0_Anim")]
#[doc(alias = "itHammer_UnkMotion4_Anim")]
#[doc(alias = "itHammer_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itHammer_UnkMotion0_Phys` and 5.
#[doc(alias = "itHammer_UnkMotion0_Phys")]
#[doc(alias = "itHammer_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itHammer_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itHammer_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029402C`.
#[doc(alias = "it_8029402C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    hide_lost_head(r, gobj);
}

/// Gravity from the attributes, `itHammer_UnkMotion1_Phys` and 4.
#[doc(alias = "itHammer_UnkMotion1_Phys")]
#[doc(alias = "itHammer_UnkMotion4_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itHammer_UnkMotion1_Coll`: a hammer with its head lands; one without it is gone on
/// touching anything.
#[doc(alias = "itHammer_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// `itHammer_Logic28_PickedUp`.
#[doc(alias = "itHammer_Logic28_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    item_mut(r.game, gobj).entered_hitlag = Some(CodeRef(HITLAG));
    hide_lost_head(r, gobj);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
}

/// `itHammer_UnkMotion3_Anim`: held, it turns headless once the head is gone.
#[doc(alias = "itHammer_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C == 0 {
        headless(r, gobj);
    }
    false
}

/// The held headless state, `it_802941A4`.
#[doc(alias = "it_802941A4")]
pub fn headless(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    hide_lost_head(r, gobj);
}

/// `itHammer_Logic28_Dropped`.
#[doc(alias = "itHammer_Logic28_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    hide_lost_head(r, gobj);
}

/// `itHammer_UnkMotion4_Coll`.
#[doc(alias = "itHammer_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// `itHammer_Logic28_EnteredAir`.
#[doc(alias = "itHammer_Logic28_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    hide_lost_head(r, gobj);
}

/// `itHammer_UnkMotion5_Coll`.
#[doc(alias = "itHammer_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Forgets a GObj the hammer points to, `itHammer_Logic28_EvtUnk` (`it_8026B894`).
#[doc(alias = "itHammer_Logic28_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
