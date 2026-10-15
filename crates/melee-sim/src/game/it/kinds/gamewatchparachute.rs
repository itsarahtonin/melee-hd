//! Mr. Game & Watch's parachute, `melee/it/kinds/itgamewatchparachute.c`: out through his neutral aerial, gone after its landing animation's 30th frame or once he leaves the aerial.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, lib26b1};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_6D3C;
pub const PICKED_UP: u32 = 0x802C_6DF8;
pub const ANIM: u32 = 0x802C_6E8C;
pub const EVT_UNK: u32 = 0x802C_6F20;

/// Runs one of the parachute's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the parachute's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The parachute at `parent`'s part `part`, `it_802C6C38`.
#[doc(alias = "it_802C6C38")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(
        r,
        super::GAME_WATCH_PARACHUTE,
        parent,
        pos,
        part,
        facing_dir,
    )
}

/// Gone, `itGameWatchParachute_Logic74_Destroyed`: its owner forgets it.
#[doc(alias = "itGameWatchParachute_Logic74_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_parachute_set_flag(owner);
    }
}

/// Taken away, `it_802C6D6C`.
#[doc(alias = "it_802C6D6C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C6DB8`.
#[doc(alias = "it_802C6DB8")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C6DD8`.
#[doc(alias = "it_802C6DD8")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGameWatchParachute_Logic74_PickedUp`: animating.
#[doc(alias = "itGameWatchParachute_Logic74_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        animate(r, gobj);
    }
}

/// Its landing animation, `it_802C6E50`, hidden.
#[doc(alias = "it_802C6E50")]
pub fn land(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itGamewatchparachute_UnkMotion1_Anim`: gone on its animation's frame 30, or once its owner leaves his neutral aerial and its landing.
#[doc(alias = "itGamewatchparachute_UnkMotion1_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).x5CC_currentAnimFrame == 30.0 {
        destroyed(r, gobj);
        return true;
    }
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_parachute_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itGameWatchParachute_Logic74_EvtUnk`.
// Ports `itGameWatchParachute_Logic74_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
