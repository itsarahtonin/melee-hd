//! Mr. Game & Watch's Sparky, `melee/it/kinds/itgamewatchbreath.c`: out through his up aerial, gone once he leaves it.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, lib26b1};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_7310;
pub const PICKED_UP: u32 = 0x802C_73CC;
pub const ANIM: u32 = 0x802C_744C;
pub const EVT_UNK: u32 = 0x802C_74B8;

/// Runs one of the Sparky's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Sparky's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The Sparky at `parent`'s part `part`, `it_802C720C`.
#[doc(alias = "it_802C720C")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(r, super::GAME_WATCH_BREATH, parent, pos, part, facing_dir)
}

/// Gone, `itGameWatchBreath_Logic76_Destroyed`: its owner forgets it.
#[doc(alias = "itGameWatchBreath_Logic76_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_sparky_set_flag(owner);
    }
}

/// Taken away, `it_802C7340`.
#[doc(alias = "it_802C7340")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C738C`.
#[doc(alias = "it_802C738C")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C73AC`.
#[doc(alias = "it_802C73AC")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGameWatchBreath_Logic76_PickedUp`: animating.
#[doc(alias = "itGameWatchBreath_Logic76_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        animate(r, gobj);
    }
}

/// Its landing animation, `it_802C7424`.
#[doc(alias = "it_802C7424")]
pub fn land(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itGamewatchbreath_UnkMotion1_Anim`: gone once its owner leaves his up aerial and its landing.
#[doc(alias = "itGamewatchbreath_UnkMotion1_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_sparky_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itGameWatchBreath_Logic76_EvtUnk`.
// Ports `itGameWatchBreath_Logic76_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
