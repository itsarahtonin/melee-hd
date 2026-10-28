//! Celebi (`It_PKind_Cerebi`), `melee/it/kinds/itcerebi.c`, the rarest draw (`it::pokemon`'s,
//! only when the save records every other Pokémon): out of the ball it sparkles, and at its
//! animation's end flies off up to one side, out of the top blast zone. It doesn't attack and
//! makes no cry, unlike Mew, whose code it otherwise repeats.

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, effect_hitlag, enter_first_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::Arg;
use crate::game::gr::stage;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_3EF8;
pub const EVT_UNK: u32 = 0x802D_3F4C;
pub const OUT_LANDED: u32 = 0x802D_3F6C;
pub const ANIM1: u32 = 0x802D_4024;
pub const PHYS1: u32 = 0x802D_4064;
pub const COLL1: u32 = 0x802D_4068;
pub const ANIM2: u32 = 0x802D_40FC;
pub const PHYS2: u32 = 0x802D_4140;
pub const COLL2: u32 = 0x802D_4160;
pub const ANIM0: u32 = 0x802D_41DC;
pub const PHYS0: u32 = 0x802D_4200;
pub const COLL0: u32 = 0x802D_429C;

/// Its sparkle (`efSync_Spawn`'s), at its model at size 1.
const SPARKLE: i32 = 0x472;

/// Runs one of Celebi's predicates, if `addr` is one.
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

/// Runs one of Celebi's events, if `addr` is one.
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

/// Celebi out of the ball, `itCerebi_Logic23_Spawned`: facing nowhere, in the air at its first
/// size.
#[doc(alias = "itCerebi_Logic23_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).facing_dir = 0.0;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
}

/// Forgets a GObj going away, `it_802D3F4C` (`it_8026B894`): only a fighter's unload at the
/// scene's end runs it (`it_8026B7F8`), which no match reaches.
// Ports `it_802D3F4C`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802D3F6C` (and `itCerebi_UnkMotion0_Phys`'s inline copy):
/// stopped and sparkling.
#[doc(alias = "it_802D3F6C")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    sparkle(r, gobj);
}

/// Sparkles, `it_802D3FA0`: into its second state, then its sparkle (`0x472`) on its model.
#[doc(alias = "it_802D3FA0")]
pub fn sparkle(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    crate::game::ef::spawn(r, SPARKLE, Some(gobj), &[Arg::JObj(j), Arg::Float(1.0)]);
}

/// `itCerebi_UnkMotion1_Anim`: sparkling; at its animation's end it flies off.
#[doc(alias = "itCerebi_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_off(r, gobj);
    }
    false
}

/// `itCerebi_UnkMotion1_Phys`: nothing.
#[doc(alias = "itCerebi_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itCerebi_UnkMotion1_Coll`: nothing.
#[doc(alias = "itCerebi_UnkMotion1_Coll")]
pub fn coll1(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Flies off, `it_802D4070`: its last state, to a random side at `x4` and up at `x8`.
#[doc(alias = "it_802D4070")]
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

/// `itCerebi_UnkMotion2_Anim`: gone above the top blast zone.
#[doc(alias = "itCerebi_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    reg(item(r.game, gobj).pos.y) > reg(stage::blast_top(r.game))
}

/// `itCerebi_UnkMotion2_Phys`: rising faster (`xC`).
#[doc(alias = "itCerebi_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_f32(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(a)));
}

/// `itCerebi_UnkMotion2_Coll`: nothing.
#[doc(alias = "itCerebi_UnkMotion2_Coll")]
pub fn coll2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Into the air, `it_802D4168` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D4168")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itCerebi_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itCerebi_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itCerebi_UnkMotion0_Phys`: falling out of the ball; at the end, as when it lands, stopped
/// and sparkling.
#[doc(alias = "itCerebi_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        out_landed(r, gobj);
    }
}

/// `itCerebi_UnkMotion0_Coll`: lands out of the ball.
#[doc(alias = "itCerebi_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, out_landed)
}
