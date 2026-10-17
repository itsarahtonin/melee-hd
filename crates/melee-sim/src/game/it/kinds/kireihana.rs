//! Bellossom (`It_PKind_Kireihana`), `melee/it/kinds/itkireihana.c`: out of the ball it
//! dances where it lands, its petals making fighters fall asleep (its script's hitboxes), and
//! leaves; walking off a ledge mid-dance, it falls dancing and dances on where it lands.

use gekko_fp as fp;

use super::super::ground::{air, fall_keep_hits};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{
    self, attr_f32, attr_i32, effect_hitlag, enter_first_state, table_bone, take_bone_motion,
};
use super::super::{item_mut, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::it::data;
use crate::game::it::item::attr_of;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_007C;
pub const EVT_UNK: u32 = 0x802D_0118;
pub const ANIM1: u32 = 0x802D_0204;
pub const PHYS1: u32 = 0x802D_02BC;
pub const COLL1: u32 = 0x802D_0390;
pub const ANIM2: u32 = 0x802D_0454;
pub const PHYS2: u32 = 0x802D_04F8;
pub const COLL2: u32 = 0x802D_05AC;
pub const ANIM3: u32 = 0x802D_0690;
pub const PHYS3: u32 = 0x802D_06EC;
pub const COLL3: u32 = 0x802D_070C;
pub const ANIM4: u32 = 0x802D_080C;
pub const PHYS4: u32 = 0x802D_0860;
pub const COLL4: u32 = 0x802D_08C4;
pub const ANIM0: u32 = 0x802D_0964;
pub const PHYS0: u32 = 0x802D_0988;
pub const COLL0: u32 = 0x802D_09A8;

/// Its variables past the release's (`itkireihana_ItemVars`): its fall speed, the dance's
/// frames left, the frames to the next petal effect and the petal effects to the next sound,
/// and the bone's last y motion (`x70`). Its `x8` is the release's bone motion's y.
const FALL_SPEED: usize = 0x60;
const DANCES: usize = 0x64;
const PETAL_FRAMES: usize = 0x68;
const PETALS: usize = 0x6C;
const LAST_Y: usize = 0x70;
const MOTION_Y: usize = 0x8;

/// Runs one of Bellossom's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of Bellossom's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        _ => return false,
    }
    true
}

/// `itKireihana_Logic15_Spawned`: facing nowhere, in the air at its first size, its sound,
/// its dances and petal counts from its attributes.
#[doc(alias = "itKireihana_Logic15_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    let dances = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(DANCES, dances);
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x2725, 0x7F, 0x40);
    let (frames, petals) = (attr_i32(r, gobj, 8), attr_i32(r, gobj, 0xC));
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(PETAL_FRAMES, frames);
    v.set_i32(PETALS, petals);
    v.set_f32(FALL_SPEED, 0.0);
    v.set_f32(LAST_Y, 0.0);
}

/// Forgets a GObj going away, `itKireihana_Logic15_EvtUnk` (`it_8026B894`).
#[doc(alias = "itKireihana_Logic15_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball and dances, `it_802D0138`.
#[doc(alias = "it_802D0138")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
}

/// Lands from a fall and dances on, `it_802D01A0`: its animation and hits kept.
#[doc(alias = "it_802D01A0")]
pub fn landed_dancing(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 1 | change::HIT_PRESERVE);
    effect_hitlag(r.game, gobj);
    bone_motion(r, gobj);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
}

fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = table_bone(r.game, gobj, 1);
    take_bone_motion(r, bone, gobj);
}

/// The bone's motion's y kept for the ledge test.
fn keep_last_y(r: &mut Running, gobj: GObjId) {
    let v = pokemon::vars(r.game, gobj);
    let y = v.f32(MOTION_Y);
    v.set_f32(LAST_Y, y);
}

/// `itKireihana_UnkMotion1_Anim`: dancing, each dance's end counts one down, and the last
/// leaves.
#[doc(alias = "itKireihana_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    keep_last_y(r, gobj);
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let n = v.i32(DANCES).wrapping_sub(1);
        v.set_i32(DANCES, n);
        if n <= 0 {
            leave(r, gobj);
        } else {
            landed(r, gobj);
        }
    }
    false
}

/// A petal effect every few frames, `itKireihana_UnkMotion1_Phys`'s and `_2_Phys`'s count.
fn petals(r: &mut Running, gobj: GObjId, sounds: bool) {
    let v = pokemon::vars(r.game, gobj);
    let n = v.i32(PETAL_FRAMES).wrapping_sub(1);
    v.set_i32(PETAL_FRAMES, n);
    if n != 0 {
        return;
    }
    let bone = table_bone(r.game, gobj, 2);
    pokemon::effect_scaled(r, 0x470, gobj, bone, 2.6);
    if sounds {
        let v = pokemon::vars(r.game, gobj);
        let p = v.i32(PETALS).wrapping_sub(1);
        v.set_i32(PETALS, p);
        if p == 0 {
            let sfx = if r.game.objs.rng.below(2) != 0 {
                0x2726
            } else {
                0x2727
            };
            play_sfx(r, gobj, sfx, 0x7F, 0x40);
            let petals = attr_i32(r, gobj, 0xC);
            pokemon::vars(r.game, gobj).set_i32(PETALS, petals);
        }
    }
    let frames = attr_i32(r, gobj, 8);
    pokemon::vars(r.game, gobj).set_i32(PETAL_FRAMES, frames);
}

/// `itKireihana_UnkMotion1_Phys`: petals, a sound every few, and the bone's motion.
#[doc(alias = "itKireihana_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    petals(r, gobj, true);
    pokemon::move_by_bone(r.game, gobj);
}

/// Off a ledge as the dance rises: not on the floor, the bone going up after coming down.
fn off_ledge(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        return false;
    }
    let v = pokemon::vars(r.game, gobj);
    reg(v.f32(LAST_Y)) < 0.0 && reg(v.f32(MOTION_Y)) > 0.0
}

/// `itKireihana_UnkMotion1_Coll`.
#[doc(alias = "itKireihana_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if off_ledge(r, gobj) {
        fall_dancing(r, gobj);
    }
    false
}

/// Falls dancing, `it_802D03F8`.
#[doc(alias = "it_802D03F8")]
pub fn fall_dancing(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, 1 | change::HIT_PRESERVE);
    effect_hitlag(r.game, gobj);
    bone_motion(r, gobj);
}

/// `itKireihana_UnkMotion2_Anim`: falling, each dance's end counts one down (after the
/// test), and the last falls leaving.
#[doc(alias = "itKireihana_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let n = v.i32(DANCES);
        v.set_i32(DANCES, n.wrapping_sub(1));
        if n <= 0 {
            fall_leaving(r, gobj);
        } else {
            change_state(r, gobj, 1, change::ANIM_UPDATE);
            effect_hitlag(r.game, gobj);
        }
    }
    false
}

/// Falls by its own gravity (`x10_fall_speed`, `x14_fall_speed_max`).
fn fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(super::super::item(r.game, gobj));
    let (g, max) = (
        a.f32(r.data, data::attr::FALL_SPEED),
        a.f32(r.data, data::attr::FALL_SPEED_MAX),
    );
    let v = pokemon::vars(r.game, gobj);
    let mut s = mem(fp::fadds(reg(v.f32(FALL_SPEED)), reg(g)));
    v.set_f32(FALL_SPEED, s);
    if reg(s) > reg(max) {
        s = max;
        v.set_f32(FALL_SPEED, s);
    }
    item_mut(r.game, gobj).x40_vel.y = mem(fp::fneg(reg(s)));
}

/// `itKireihana_UnkMotion2_Phys`: petals without sounds, the bone's motion and its fall.
#[doc(alias = "itKireihana_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    petals(r, gobj, false);
    pokemon::move_by_bone(r.game, gobj);
    fall(r, gobj);
}

/// `itKireihana_UnkMotion2_Coll`: lands dancing on.
#[doc(alias = "itKireihana_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall_keep_hits(r, gobj, landed_dancing);
    false
}

/// Leaves, `it_802D05D8`.
#[doc(alias = "it_802D05D8")]
pub fn leave(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
}

/// Lands leaving, `it_802D062C`: its animation and hits kept.
#[doc(alias = "it_802D062C")]
pub fn landed_leaving(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, 1 | change::HIT_PRESERVE);
    effect_hitlag(r.game, gobj);
    bone_motion(r, gobj);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
}

/// `itKireihana_UnkMotion3_Anim`: gone when its animation ends.
#[doc(alias = "itKireihana_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    keep_last_y(r, gobj);
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// `itKireihana_UnkMotion3_Phys` (and `_4_Phys`'s first part): the bone's motion.
#[doc(alias = "itKireihana_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itKireihana_UnkMotion3_Coll`.
#[doc(alias = "itKireihana_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if off_ledge(r, gobj) {
        fall_leaving_kept(r, gobj);
    }
    false
}

/// Falls leaving, `it_802D0774`.
#[doc(alias = "it_802D0774")]
pub fn fall_leaving(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// Falls leaving off a ledge, `it_802D07C0`: its animation and hits kept.
#[doc(alias = "it_802D07C0")]
pub fn fall_leaving_kept(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, 1 | change::HIT_PRESERVE);
    effect_hitlag(r.game, gobj);
}

/// `itKireihana_UnkMotion4_Anim`: gone when its animation ends.
#[doc(alias = "itKireihana_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// `itKireihana_UnkMotion4_Phys`: the bone's motion and its fall.
#[doc(alias = "itKireihana_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    fall(r, gobj);
}

/// `itKireihana_UnkMotion4_Coll`: lands leaving.
#[doc(alias = "itKireihana_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    fall_keep_hits(r, gobj, landed_leaving);
    false
}

/// Into the air, `it_802D08F0` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D08F0")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itKireihana_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itKireihana_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itKireihana_UnkMotion0_Phys`: falling out of the ball.
#[doc(alias = "itKireihana_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itKireihana_UnkMotion0_Coll`: lands and dances.
#[doc(alias = "itKireihana_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
