//! Mr. Game & Watch's oil, `melee/it/kinds/itgamewatchpanic.c`: thrown from the full bucket
//! through Oil Panic's throw, on the ground or in the air.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, lib26b1};
use super::gamewatch;
use crate::game::ft::common::GA_GROUND;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_7E64;
pub const PICKED_UP: u32 = 0x802C_7F20;
pub const ANIM: u32 = 0x802C_7FAC;
pub const EVT_UNK: u32 = 0x802C_8018;

/// Runs one of the oil's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the oil's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The oil at `parent`'s part `part`, `it_802C7D60`.
#[doc(alias = "it_802C7D60")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(r, super::GAME_WATCH_PANIC, parent, pos, part, facing_dir)
}

/// Gone, `itGameWatchPanic_Logic78_Destroyed`: its owner forgets it.
#[doc(alias = "itGameWatchPanic_Logic78_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_panic_set_flag(owner);
    }
}

/// Taken away, `it_802C7E94`.
#[doc(alias = "it_802C7E94")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C7EE0`.
#[doc(alias = "it_802C7EE0")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C7F00`.
#[doc(alias = "it_802C7F00")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGameWatchPanic_Logic78_PickedUp`: on the ground or in the air as its
/// owner is, animating.
#[doc(alias = "itGameWatchPanic_Logic78_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let Some(owner) = ip.owner else { return };
    let ga = r.game.ft_ground_air(owner);
    item_mut(r.game, gobj).ground_or_air = ga;
    let state = if ga == GA_GROUND { 0 } else { 1 };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// `itGamewatchpanic_UnkMotion1_Anim`: gone once its owner leaves the throw.
#[doc(alias = "itGamewatchpanic_UnkMotion1_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_panic_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itGameWatchPanic_Logic78_EvtUnk`.
// Ports `itGameWatchPanic_Logic78_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
