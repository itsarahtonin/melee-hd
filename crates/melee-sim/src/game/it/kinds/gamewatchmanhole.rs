//! Mr. Game & Watch's manhole cover, `melee/it/kinds/itgamewatchmanhole.c`: hidden until the
//! script shows it, it blows a gust (`lb_800119DC`) from its lid and goes once he leaves the
//! down tilt.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, jobj_of, lib26b1};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_66E8;
pub const PICKED_UP: u32 = 0x802C_67A4;
pub const ANIM: u32 = 0x802C_6808;
pub const EVT_UNK: u32 = 0x802C_68D8;

/// Runs one of the manhole's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the manhole's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The manhole in `parent`'s hand at `part`, `it_802C65E4`.
#[doc(alias = "it_802C65E4")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(r, super::GAME_WATCH_MANHOLE, parent, pos, part, facing_dir)
}

/// Gone, `itGameWatchManhole_Logic72_Destroyed`: its owner forgets it and takes back the
/// item it put away.
#[doc(alias = "itGameWatchManhole_Logic72_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_manhole_forget(owner);
    }
}

/// Taken away, `it_802C6718`.
#[doc(alias = "it_802C6718")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C6764`.
#[doc(alias = "it_802C6764")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C6784`.
#[doc(alias = "it_802C6784")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGameWatchManhole_Logic72_PickedUp`: hidden, animating.
#[doc(alias = "itGameWatchManhole_Logic72_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    lib26b1::hide(r.game, gobj);
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        animate(r, gobj);
    }
}

/// `itGamewatchmanhole_UnkMotion0_Anim`: at the script's mark shown, with its gust from the
/// lid; gone once its owner leaves the down tilt.
#[doc(alias = "itGamewatchmanhole_UnkMotion0_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        lib26b1::show(r.game, gobj);
        let lid = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, lid, None);
        crate::lb::wind::blast(
            &mut r.game.gr.winds,
            pos,
            120,
            3.0,
            0.1,
            std::f32::consts::FRAC_PI_3,
        );
    }
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_manhole_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itGameWatchManhole_Logic72_EvtUnk`.
// Ports `itGameWatchManhole_Logic72_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
