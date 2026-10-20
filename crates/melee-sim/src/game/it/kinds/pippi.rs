//! Clefairy (`It_PKind_Pippi`), `melee/it/kinds/itpippi.c`: out of the ball it waits a few
//! loops of its idle animation, then does one of four things by its weights (its scripts do
//! the rest: Metronome), and leaves when its script says.

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx, set_state_anims};
use super::super::pokemon::{self, attr_f32, attr_i32};
use super::super::{item, item_mut, lib26b1, lib2725, maplib};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_3260;
pub const LANDED: u32 = 0x802D_32D8;
pub const EVT_UNK: u32 = 0x802D_32DC;
pub const ANIM1: u32 = 0x802D_32FC;
pub const PHYS1: u32 = 0x802D_3370;
pub const COLL1: u32 = 0x802D_33AC;
pub const ACT: u32 = 0x802D_33F8;
pub const ANIM5: u32 = 0x802D_34A4;
pub const PHYS5: u32 = 0x802D_3508;
pub const COLL5: u32 = 0x802D_3544;
pub const ENTER_AIR: u32 = 0x802D_3590;
pub const ANIM0: u32 = 0x802D_3600;
pub const PHYS0: u32 = 0x802D_3624;
pub const COLL0: u32 = 0x802D_3688;

/// Its variables past the release's (`itPokemon_ItemVars`): the idle loops left (`timer`).
const TIMER: usize = 0x60;

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// Runs one of Clefairy's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 | COLL5 => coll1(r, gobj),
        ANIM5 => anim5(r, gobj),
        _ => return None,
    })
}

/// Runs one of Clefairy's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 | PHYS5 => phys1(r, gobj),
        ACT => act(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itPippi_Logic20_Spawned`: facing nowhere, its script's variables clear, at its first
/// size in the air, and its sound.
#[doc(alias = "itPippi_Logic20_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    play_sfx(r, gobj, 0x272A, 0x7F, 0x40);
}

/// Lands (or leaves the floor) and nothing happens, `it_802D32D8`.
#[doc(alias = "it_802D32D8")]
pub fn landed(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802D32DC` (`it_8026B894`).
#[doc(alias = "it_802D32DC")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itPippi_UnkMotion1_Anim`: each idle loop's end counts one down, and the last acts.
#[doc(alias = "itPippi_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let n = v.i32(TIMER).wrapping_sub(1);
        v.set_i32(TIMER, n);
        if v.i32(TIMER) <= 0 {
            act(r, gobj);
        } else {
            replay(r, gobj);
        }
    }
    false
}

/// Its state's animations from the start again (`Item_80268D34` with `xD0_itemStateDesc`).
fn replay(r: &mut Running, gobj: GObjId) {
    let desc = item(r.game, gobj)
        .xD0_itemStateDesc
        .expect("Clefairy without its state's description");
    set_state_anims(r, gobj, desc);
}

/// `itPippi_UnkMotion1_Phys` (and `_5_Phys`): in the air, its gravity.
#[doc(alias = "itPippi_UnkMotion1_Phys")]
#[doc(alias = "itPippi_UnkMotion5_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        super::gravity(r, gobj);
    }
}

/// `itPippi_UnkMotion1_Coll` (and `_5_Coll`): lands in the air, stays on the floor on it.
#[doc(alias = "itPippi_UnkMotion1_Coll")]
#[doc(alias = "itPippi_UnkMotion5_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        fall(r, gobj, landed);
    } else {
        ground_or_fall(r, gobj, landed);
    }
    false
}

/// Acts, `it_802D33F8`: one of states 2 to 5 by the weights in its attributes, its effects
/// not paused in hitlag.
#[doc(alias = "it_802D33F8")]
pub fn act(r: &mut Running, gobj: GObjId) {
    let max = attr_i32(r, gobj, 8);
    let roll = r.game.objs.rng.below(max);
    let mut val = attr_i32(r, gobj, 0xC);
    let state = if roll < val {
        2
    } else {
        val = val.wrapping_add(attr_i32(r, gobj, 0x10));
        if roll < val {
            3
        } else if roll < attr_i32(r, gobj, 0x14).wrapping_add(val) {
            4
        } else {
            5
        }
    };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    no_effect_hitlag(r, gobj);
}

/// No effect hitlag callbacks.
fn no_effect_hitlag(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.entered_hitlag = None;
    ip.exited_hitlag = None;
}

/// `itPippi_UnkMotion5_Anim` (states 2 to 5): its animation looping, gone when its script
/// says (`xDAC_itcmd_var0`).
#[doc(alias = "itPippi_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        replay(r, gobj);
    }
    item(r.game, gobj).xDAC_itcmd_var0 != 0
}

/// Into the air at its first state, `it_802D3590`: its idle loops from its attributes.
#[doc(alias = "it_802D3590")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    no_effect_hitlag(r, gobj);
    let timer = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(TIMER, timer);
}

/// `itPippi_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itPippi_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itPippi_UnkMotion0_Phys`: falling out of the ball, then idle.
#[doc(alias = "itPippi_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        no_effect_hitlag(r, gobj);
    }
}

/// `itPippi_UnkMotion0_Coll`: lands out of the ball.
#[doc(alias = "itPippi_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
