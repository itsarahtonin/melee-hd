//! Mew (`It_PKind_Mew`), `melee/it/kinds/itmew.c`, the rare draw (`it::pokemon`'s): out of
//! the ball it sparkles and cries, and at its animation's end flies off up to one side, out of
//! the top blast zone. It doesn't attack.

use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, effect_hitlag, enter_first_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_3B18;
pub const EVT_UNK: u32 = 0x802D_3B6C;
pub const OUT_LANDED: u32 = 0x802D_3B8C;
pub const ANIM1: u32 = 0x802D_3C50;
pub const PHYS1: u32 = 0x802D_3C90;
pub const COLL1: u32 = 0x802D_3C94;
pub const ANIM2: u32 = 0x802D_3D28;
pub const PHYS2: u32 = 0x802D_3D6C;
pub const COLL2: u32 = 0x802D_3D8C;
pub const ANIM0: u32 = 0x802D_3E08;
pub const PHYS0: u32 = 0x802D_3E2C;
pub const COLL0: u32 = 0x802D_3ED0;

/// Its sparkle's size (`it_8027ADEC`'s divisor), and its cry.
const SPARKLE_SIZE: f32 = 2.8;
const CRY: i32 = 0x2729;

/// Runs one of Mew's predicates, if `addr` is one.
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

/// Runs one of Mew's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        OUT_LANDED => out_landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// Mew out of the ball, `itMew_Logic22_Spawned`: facing nowhere, in the air at its first size.
#[doc(alias = "itMew_Logic22_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).facing_dir = 0.0;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
}

/// Forgets a GObj going away, `it_802D3B6C` (`it_8026B894`).
#[doc(alias = "it_802D3B6C")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802D3B8C`: stopped, sparkling, and its cry.
#[doc(alias = "it_802D3B8C")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    sparkle(r, gobj);
    play_sfx(r, gobj, CRY, 0x7F, 0x40);
}

/// Sparkles, `it_802D3BE0` (and `itMew_UnkMotion0_Phys`'s inline copy): its sparkle's effect
/// (`0x46B`) on its model, into its second state.
#[doc(alias = "it_802D3BE0")]
pub fn sparkle(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, 0x46B, gobj, j, SPARKLE_SIZE);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itMew_UnkMotion1_Anim`: sparkling; at its animation's end it flies off.
#[doc(alias = "itMew_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_off(r, gobj);
    }
    false
}

/// `itMew_UnkMotion1_Phys`: nothing.
#[doc(alias = "itMew_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itMew_UnkMotion1_Coll`: nothing.
#[doc(alias = "itMew_UnkMotion1_Coll")]
pub fn coll1(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Flies off, `it_802D3C9C`: its last state, to a random side at `x4` and up at `x8`.
#[doc(alias = "it_802D3C9C")]
pub fn fly_off(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    let side = attr_f32(r, gobj, 4);
    let x = if r.game.objs.rng.below(2) != 0 {
        side
    } else {
        mem(fp::fneg(reg(side)))
    };
    let up = attr_f32(r, gobj, 8);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = x;
    ip.x40_vel.y = up;
}

/// `itMew_UnkMotion2_Anim`: gone above the top blast zone.
#[doc(alias = "itMew_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    reg(item(r.game, gobj).pos.y) > reg(stage::blast_top(r.game))
}

/// `itMew_UnkMotion2_Phys`: rising faster (`xC`).
#[doc(alias = "itMew_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_f32(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(a)));
}

/// `itMew_UnkMotion2_Coll`: nothing.
#[doc(alias = "itMew_UnkMotion2_Coll")]
pub fn coll2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Into the air, `it_802D3D94` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D3D94")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itMew_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itMew_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itMew_UnkMotion0_Phys`: falling out of the ball; at the end, as when it lands, stopped,
/// sparkling, and its cry.
#[doc(alias = "itMew_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        out_landed(r, gobj);
    }
}

/// `itMew_UnkMotion0_Coll`: lands out of the ball.
#[doc(alias = "itMew_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, out_landed)
}
