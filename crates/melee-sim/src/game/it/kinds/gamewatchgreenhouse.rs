//! Mr. Game & Watch's sprayer, `melee/it/kinds/itgamewatchgreenhouse.c`: held through his
//! jabs, each playing its own animation, the rapid jab's loop repeating while it lasts.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, lib26b1, lib2725};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_62F8;
pub const PICKED_UP: u32 = 0x802C_63B4;
pub const MOTION3_ANIM: u32 = 0x802C_64D0;
pub const MOTION2_ANIM: u32 = 0x802C_653C;
pub const EVT_UNK: u32 = 0x802C_65C4;

/// Runs one of the sprayer's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION3_ANIM => motion3_anim(r, gobj),
        MOTION2_ANIM => motion2_anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the sprayer's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The sprayer in `parent`'s hand at `part`, `itGamewatchGreenhouse_Spawn`.
#[doc(alias = "itGamewatchGreenhouse_Spawn")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(
        r,
        super::GAME_WATCH_GREENHOUSE,
        parent,
        pos,
        part,
        facing_dir,
    )
}

/// Gone, `itGamewatchGreenhouse_Destroyed`: its owner forgets it.
#[doc(alias = "itGamewatchGreenhouse_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_greenhouse_set_flag(owner);
    }
}

/// Taken away, `itGamewatchGreenhouse_802C6328`.
#[doc(alias = "itGamewatchGreenhouse_802C6328")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `itGamewatchGreenhouse_802C6374`.
#[doc(alias = "itGamewatchGreenhouse_802C6374")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `itGamewatchGreenhouse_802C6394`.
#[doc(alias = "itGamewatchGreenhouse_802C6394")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGamewatchGreenhouse_PickedUp`: the jab's animation, or the rapid jab's.
#[doc(alias = "itGamewatchGreenhouse_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let Some(owner) = ip.owner else { return };
    let state = if r.gw_not_attack11(owner) { 1 } else { 0 };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// The jabs' animations: `itGamewatchGreenhouse_802C6430` (the jab), `_802C6458` (the
/// rapid jab's start), `_802C6480` (its loop) and `_802C64A8` (its end).
#[doc(alias = "itGamewatchGreenhouse_802C6430")]
#[doc(alias = "itGamewatchGreenhouse_802C6458")]
#[doc(alias = "itGamewatchGreenhouse_802C6480")]
#[doc(alias = "itGamewatchGreenhouse_802C64A8")]
pub fn set_state(r: &mut Running, gobj: GObjId, state: i32) {
    change_state(r, gobj, state, change::ANIM_UPDATE);
}

/// Whether it goes (`greenhouse_Check`), telling its owner.
fn check(r: &mut Running, gobj: GObjId) -> bool {
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_greenhouse_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// `itGamewatchGreenhouse_Motion3_Anim`.
#[doc(alias = "itGamewatchGreenhouse_Motion3_Anim")]
pub fn motion3_anim(r: &mut Running, gobj: GObjId) -> bool {
    check(r, gobj)
}

/// `itGamewatchGreenhouse_Motion2_Anim`: the loop's animation again once it ends.
#[doc(alias = "itGamewatchGreenhouse_Motion2_Anim")]
pub fn motion2_anim(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    }
    check(r, gobj)
}

/// Forgets a GObj, `itGamewatchGreenhouse_EvtUnk`.
// Ports `itGamewatchGreenhouse_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
