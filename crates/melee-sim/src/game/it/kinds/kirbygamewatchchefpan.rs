//! The pan of Kirby's copy of Chef, `melee/it/kinds/itkirbygamewatchchefpan.c`: held at his
//! thumb and colored as his copy, gone once he leaves the move.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, lib26b1};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_75DC;
pub const PICKED_UP: u32 = 0x802C_7698;
pub const ANIM: u32 = 0x802C_76E8;
pub const EVT_UNK: u32 = 0x802C_7754;

/// Runs one of the pan's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the pan's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The pan at `parent`'s part `part`, `it_802C74D8`.
#[doc(alias = "it_802C74D8")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(
        r,
        super::KIRBY_GAME_WATCH_CHEF_PAN,
        parent,
        pos,
        part,
        facing_dir,
    )
}

/// Gone, `itKirbyGameWatchChefPan_Logic113_Destroyed`: its owner forgets it.
#[doc(alias = "itKirbyGameWatchChefPan_Logic113_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.kb_gw_pan_gone(owner);
    }
}

/// Taken away, `it_802C760C`.
#[doc(alias = "it_802C760C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C7658`.
#[doc(alias = "it_802C7658")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C7678`.
#[doc(alias = "it_802C7678")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itKirbyGameWatchChefPan_Logic113_PickedUp`: animating.
#[doc(alias = "itKirbyGameWatchChefPan_Logic113_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        animate(r, gobj);
    }
}

/// `itKirbygamewatchchefpan_UnkMotion0_Anim`: gone once its owner has left the move (or it
/// has none).
#[doc(alias = "itKirbygamewatchchefpan_UnkMotion0_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.kb_gw_done(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itKirbyGameWatchChefPan_Logic113_EvtUnk`.
// Ports `itKirbyGameWatchChefPan_Logic113_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
