//! Moltres (`It_PKind_Fire`), `melee/it/kinds/itfire.c`: out of the ball it bursts into flame
//! and rises, spinning fire (its script's hitboxes), then flies up out of the top blast zone.

use gekko_fp as fp;

use super::super::ground::fall_unstick;
use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, effect_hitlag, enter_first_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_C6E4;
pub const LANDED: u32 = 0x802C_C740;
pub const EVT_UNK: u32 = 0x802C_C744;
pub const ANIM1: u32 = 0x802C_C764;
pub const PHYS1: u32 = 0x802C_C7A4;
pub const COLL1: u32 = 0x802C_C7A8;
pub const RISING_LANDED: u32 = 0x802C_C7D4;
pub const ANIM2: u32 = 0x802C_C860;
pub const PHYS2: u32 = 0x802C_C91C;
pub const COLL2: u32 = 0x802C_C93C;
pub const ANIM0: u32 = 0x802C_C9B8;
pub const PHYS0: u32 = 0x802C_C9DC;
pub const COLL0: u32 = 0x802C_CA8C;

/// Runs one of Moltres's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Moltres's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        RISING_LANDED => rising_landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// `itFire_Logic6_Spawned`: facing nowhere, in the air at its first size.
#[doc(alias = "itFire_Logic6_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
}

/// Lands out of the ball: nothing, `it_802CC740`.
#[doc(alias = "it_802CC740")]
pub fn landed(_r: &mut Running, _gobj: GObjId) {}

/// Forgets a GObj going away, `itFire_Logic6_EvtUnk` (`it_8026B894`).
#[doc(alias = "itFire_Logic6_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itFire_UnkMotion1_Anim`: bursting into flame; at the end it flies up.
#[doc(alias = "itFire_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_up(r, gobj);
    }
    false
}

/// `itFire_UnkMotion1_Phys`: nothing.
#[doc(alias = "itFire_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itFire_UnkMotion1_Coll`.
#[doc(alias = "itFire_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_unstick(r, gobj, rising_landed);
    false
}

/// Lands bursting: nothing, `it_802CC7D4`.
#[doc(alias = "it_802CC7D4")]
pub fn rising_landed(_r: &mut Running, _gobj: GObjId) {}

/// Its fire effect (`0x463`) and its rise's start (`x8`), flying up, `it_802CC7D8` (and
/// `itFire_UnkMotion2_Anim`'s inline copy).
#[doc(alias = "it_802CC7D8")]
pub fn fly_up(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, 0x463, gobj, j, 1.1);
    let up = attr_f32(r, gobj, 8);
    item_mut(r.game, gobj).x40_vel.y = up;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itFire_UnkMotion2_Anim`: flying up, its animation looping with its effect; gone above the
/// top blast zone.
#[doc(alias = "itFire_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_up(r, gobj);
    }
    reg(item(r.game, gobj).pos.y) > reg(stage::blast_top(r.game))
}

/// `itFire_UnkMotion2_Phys`: rising faster (`xC`).
#[doc(alias = "itFire_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_f32(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(a)));
}

/// `itFire_UnkMotion2_Coll`: nothing.
#[doc(alias = "itFire_UnkMotion2_Coll")]
pub fn coll2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Into the air, `it_802CC944` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802CC944")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itFire_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itFire_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itFire_UnkMotion0_Phys`: falling out of the ball; at the end its burst effect (`0x464`),
/// stopped and rising (`x4`), bursting into flame.
#[doc(alias = "itFire_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        let j = jobj_of(r.game, gobj);
        pokemon::effect_scaled(r, 0x464, gobj, j, 1.1);
        lib2725::clear_velocity(r.game, gobj);
        let up = attr_f32(r, gobj, 4);
        item_mut(r.game, gobj).x40_vel.y = up;
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
        set_flag(&mut item_mut(r.game, gobj).xDD1_flag, 1, true);
    }
}

/// `itFire_UnkMotion0_Coll`: lands doing nothing.
#[doc(alias = "itFire_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
