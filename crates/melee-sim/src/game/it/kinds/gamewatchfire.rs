//! Mr. Game & Watch's torch, `melee/it/kinds/itgamewatchfire.c`: held through his forward
//! smash, shown from its animation's third frame, its animation stopped while he charges.

use super::super::item::{animate, change, change_state, destroy};
use super::super::{item, item_mut, jobj_of, lib26b1};
use super::gamewatch;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_69FC;
pub const PICKED_UP: u32 = 0x802C_6AB8;
pub const ANIM: u32 = 0x802C_6B20;
pub const EVT_UNK: u32 = 0x802C_6C18;

/// Runs one of the torch's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the torch's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The torch in `parent`'s hand at `part`, `itGamewatchFire_Spawn`.
#[doc(alias = "itGamewatchFire_Spawn")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    gamewatch::spawn_held(r, super::GAME_WATCH_FIRE, parent, pos, part, facing_dir)
}

/// Gone, `itGamewatchFire_Destroyed`: its owner forgets it.
#[doc(alias = "itGamewatchFire_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_torch_set_flag(owner);
    }
}

/// Taken away, `itGamewatchFire_802C6A2C`.
#[doc(alias = "itGamewatchFire_802C6A2C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `itGamewatchFire_802C6A78`.
#[doc(alias = "itGamewatchFire_802C6A78")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `itGamewatchFire_802C6A98`.
#[doc(alias = "itGamewatchFire_802C6A98")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `itGamewatchFire_PickedUp`: hidden, animating.
#[doc(alias = "itGamewatchFire_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    lib26b1::hide(r.game, gobj);
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        animate(r, gobj);
    }
}

/// `itGamewatchFire_Motion0_Anim`: shown on frame 3, gone once its owner leaves the forward
/// smash, its animation stopped while he charges and running at full speed otherwise.
#[doc(alias = "itGamewatchFire_Motion0_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let jobj = jobj_of(r.game, gobj);
    if item(r.game, gobj).x5CC_currentAnimFrame == 3.0 {
        lib26b1::show(r.game, gobj);
    }
    let owner = item(r.game, gobj).owner;
    let go = match owner {
        Some(o) => r.gw_torch_should_go(o),
        None => true,
    };
    if go {
        destroyed(r, gobj);
        return true;
    }
    if let Some(o) = owner {
        let speed = if r.game.ft_is_charging_smash(o) {
            0.0
        } else {
            1.0
        };
        if item(r.game, gobj).x5D0_animFrameSpeed != speed {
            item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
            crate::lb::anim::set_framerate(&mut r.game.objs, jobj, speed);
        }
    }
    false
}

/// Forgets a GObj, `itGamewatchFire_EvtUnk`.
// Ports `itGamewatchFire_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
