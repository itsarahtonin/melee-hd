//! Togepi (`It_PKind_Togepy`), `melee/it/kinds/ittogepy.c`: out of the ball it waits a few
//! loops of its idle animation, then does one of five things by its weights (its scripts do
//! the rest), and leaves when its script says.

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx, set_state_anims};
use super::super::pokemon::{self, attr_f32, attr_i32};
use super::super::{item, item_mut, lib26b1, lib2725, maplib};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_36B0;
pub const LANDED: u32 = 0x802D_3728;
pub const EVT_UNK: u32 = 0x802D_372C;
pub const ANIM1: u32 = 0x802D_374C;
pub const PHYS1: u32 = 0x802D_37C0;
pub const COLL1: u32 = 0x802D_37FC;
pub const ACT: u32 = 0x802D_3848;
pub const ANIM6: u32 = 0x802D_390C;
pub const PHYS6: u32 = 0x802D_3970;
pub const COLL6: u32 = 0x802D_39AC;
pub const ENTER_AIR: u32 = 0x802D_39F8;
pub const ANIM0: u32 = 0x802D_3A68;
pub const PHYS0: u32 = 0x802D_3A8C;
pub const COLL0: u32 = 0x802D_3AF0;

/// Its variables past the release's (`itPokemon_ItemVars`): the idle loops left (`timer`).
const TIMER: usize = 0x60;

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// Runs one of Togepi's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 | COLL6 => coll1(r, gobj),
        ANIM6 => anim6(r, gobj),
        _ => return None,
    })
}

/// Runs one of Togepi's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 | PHYS6 => phys1(r, gobj),
        ACT => act(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itTogepy_Logic21_Spawned`: facing nowhere, its script's variables clear, at its first
/// size in the air, and its sound.
#[doc(alias = "itTogepy_Logic21_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    play_sfx(r, gobj, 0x272C, 0x7F, 0x40);
}

/// Lands (or leaves the floor) and nothing happens, `it_802D3728`.
#[doc(alias = "it_802D3728")]
pub fn landed(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `itTogepy_Logic21_EvtUnk` (`it_8026B894`).
#[doc(alias = "itTogepy_Logic21_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itTogepy_UnkMotion1_Anim`: each idle loop's end counts one down, and the last acts.
#[doc(alias = "itTogepy_UnkMotion1_Anim")]
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
        .expect("Togepi without its state's description");
    set_state_anims(r, gobj, desc);
}

/// `itTogepy_UnkMotion1_Phys` (and `_6_Phys`): in the air, its gravity.
#[doc(alias = "itTogepy_UnkMotion1_Phys")]
#[doc(alias = "itTogepy_UnkMotion6_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        super::gravity(r, gobj);
    }
}

/// `itTogepy_UnkMotion1_Coll` (and `_6_Coll`): lands in the air, stays on the floor on it.
#[doc(alias = "itTogepy_UnkMotion1_Coll")]
#[doc(alias = "itTogepy_UnkMotion6_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        fall(r, gobj, landed);
    } else {
        ground_or_fall(r, gobj, landed);
    }
    false
}

/// Acts, `it_802D3848`: one of states 2 to 6 by the weights in its attributes, its effects
/// not paused in hitlag.
#[doc(alias = "it_802D3848")]
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
        } else {
            val = attr_i32(r, gobj, 0x14).wrapping_add(val);
            if roll < val {
                4
            } else if roll < attr_i32(r, gobj, 0x18).wrapping_add(val) {
                5
            } else {
                6
            }
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

/// `itTogepy_UnkMotion6_Anim` (states 2 to 6): its animation looping, gone when its script
/// says (`xDAC_itcmd_var0`).
#[doc(alias = "itTogepy_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        replay(r, gobj);
    }
    item(r.game, gobj).xDAC_itcmd_var0 != 0
}

/// Into the air at its first state, `it_802D39F8`: its idle loops from its attributes.
#[doc(alias = "it_802D39F8")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    no_effect_hitlag(r, gobj);
    let timer = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(TIMER, timer);
}

/// `itTogepy_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itTogepy_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itTogepy_UnkMotion0_Phys`: falling out of the ball, then idle.
#[doc(alias = "itTogepy_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        no_effect_hitlag(r, gobj);
    }
}

/// `itTogepy_UnkMotion0_Coll`: lands out of the ball.
#[doc(alias = "itTogepy_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
