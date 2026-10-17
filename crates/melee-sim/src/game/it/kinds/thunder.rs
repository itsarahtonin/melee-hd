//! Zapdos (`It_PKind_Thunder`), `melee/it/kinds/itthunder.c`: out of the ball it lands and
//! calls down thunder (its script's hitboxes, an effect when the count runs out), then flies
//! up, rising faster once its script says, gone past the top of the blast zone.

use gekko_fp as fp;

use super::super::ground::fall;
use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, enter_first_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_CAB4;
pub const LANDED_NOTHING: u32 = 0x802C_CB10;
pub const EVT_UNK: u32 = 0x802C_CB14;
pub const ANIM1: u32 = 0x802C_CB34;
pub const PHYS1: u32 = 0x802C_CBAC;
pub const COLL1: u32 = 0x802C_CBCC;
pub const THUNDER_ACCESSORY: u32 = 0x802C_CBF8;
pub const ANIM2: u32 = 0x802C_CCC4;
pub const PHYS2: u32 = 0x802C_CD9C;
pub const COLL2: u32 = 0x802C_CE20;
pub const ANIM0: u32 = 0x802C_CE9C;
pub const PHYS0: u32 = 0x802C_CEC0;
pub const COLL0: u32 = 0x802C_CF74;

/// Its variables past the release's (`itThunder_ItemVars`): the frames to the thunder's effect
/// (`x60`), whether the effect is still to come (`x64`), and its rising speed (`x68`).
const EFFECT_FRAMES: usize = 0x60;
const EFFECT_PENDING: usize = 0x64;
const RISE: usize = 0x68;

/// Runs one of Zapdos's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => false,
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        _ => return None,
    })
}

/// Runs one of Zapdos's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => {}
        PHYS1 => phys1(r, gobj),
        THUNDER_ACCESSORY => thunder_effect(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS0 => phys0(r, gobj),
        _ => return false,
    }
    true
}

/// `itThunder_Logic7_Spawned`: facing nowhere, its script's flag clear, into the air out of
/// the ball, at its first size.
#[doc(alias = "itThunder_Logic7_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
}

/// Landing in its states, `it_802CCB10`: nothing.
#[doc(alias = "it_802CCB10")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CCB14` (`it_8026B894`).
#[doc(alias = "it_802CCB14")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Its root joint's grandchild's motion taken as its own (`itGetJObjGrandchild`).
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let jobjs = &r.game.objs.jobjs;
    let bone = jobj_of(r.game, gobj)
        .and_then(|j| jobjs[j].child)
        .and_then(|c| jobjs[c].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// `itThunder_UnkMotion1_Anim`: calling thunder; flies off when its animation ends.
#[doc(alias = "itThunder_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        fly_off(r, gobj);
    }
    false
}

/// `itThunder_UnkMotion1_Phys`: the bone's motion.
#[doc(alias = "itThunder_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itThunder_UnkMotion1_Coll`: lands doing nothing.
#[doc(alias = "itThunder_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed_nothing);
    false
}

/// Its `on_accessory` calling thunder, `it_802CCBF8`: once its frames run out, the thunder's
/// effect on its third bone.
#[doc(alias = "it_802CCBF8")]
pub fn thunder_effect(r: &mut Running, gobj: GObjId) {
    let v = pokemon::vars(r.game, gobj);
    if v.i32(EFFECT_PENDING) == 0 {
        return;
    }
    let left = v.i32(EFFECT_FRAMES);
    if left <= 0 {
        v.set_i32(EFFECT_PENDING, 0);
        let bone = pokemon::table_bone(r.game, gobj, 2);
        ef::spawn(r, 0x460, Some(gobj), &[Arg::JObj(bone), Arg::Float(1.0)]);
    } else {
        v.set_i32(EFFECT_FRAMES, left.wrapping_sub(1));
    }
}

/// Flies off, `it_802CCC68` (and `itThunder_UnkMotion2_Anim_inline2`): not rising yet.
#[doc(alias = "it_802CCC68")]
pub fn fly_off(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    pokemon::vars(r.game, gobj).set_f32(RISE, 0.0);
    item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
}

/// `itThunder_UnkMotion2_Anim`: flying off, its animation over and over; gone past the top of
/// the blast zone.
#[doc(alias = "itThunder_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        fly_off(r, gobj);
    }
    let top = stage::blast_top(r.game);
    reg(item(r.game, gobj).pos.y) > reg(top)
}

/// `itThunder_UnkMotion2_Phys`: the bone's motion; once its script says, it rises from its
/// attribute's speed, faster each frame.
#[doc(alias = "itThunder_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        let rise = attr_f32(r, gobj, 4);
        pokemon::vars(r.game, gobj).set_f32(RISE, rise);
        let ip = item_mut(r.game, gobj);
        ip.xDAC_itcmd_var0 = 0;
        ip.xDB0_itcmd_var1 = 1;
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let accel = attr_f32(r, gobj, 8);
        let v = pokemon::vars(r.game, gobj);
        let rise = v.f32(RISE);
        v.set_f32(RISE, mem(fp::fadds(reg(rise), reg(accel))));
    }
    let rise = pokemon::vars(r.game, gobj).f32(RISE);
    item_mut(r.game, gobj).x40_vel.y = rise;
}

/// `itThunder_UnkMotion2_Coll`: nothing.
#[doc(alias = "itThunder_UnkMotion2_Coll")]
pub fn coll2(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Into the air out of the ball, `it_802CCE28` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802CCE28")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itThunder_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itThunder_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itThunder_UnkMotion0_Phys`: out of the ball, once its frames are up it stops and calls
/// thunder, its effect's frames from its attribute (the inlines
/// `itThunder_UnkMotion0_Phys_inline` and `_inline2`).
#[doc(alias = "itThunder_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if !pokemon::fall(r, gobj) {
        return;
    }
    lib2725::clear_velocity(r.game, gobj);
    let frames = attr_i32(r, gobj, 0xC);
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(EFFECT_FRAMES, frames);
    v.set_i32(EFFECT_PENDING, 1);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(THUNDER_ACCESSORY));
    set_flag(&mut ip.xDD1_flag, 1, true);
}

/// `itThunder_UnkMotion0_Coll`: lands out of the ball, doing nothing.
#[doc(alias = "itThunder_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}
