//! Mr. Game & Watch's Judgement hammer, `melee/it/kinds/itgamewatchjudge.c`: held through
//! Judgement, its sign posed at the number's frame and turned to face away from him.

use super::super::item::{change, change_state, destroy, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::gamewatch;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802C_7A54;
pub const PICKED_UP: u32 = 0x802C_7B10;
pub const ANIM: u32 = 0x802C_7B54;
pub const EVT_UNK: u32 = 0x802C_7D40;

/// Runs one of the hammer's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        _ => return None,
    })
}

/// Runs one of the hammer's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The hammer in `parent`'s hand at `part` showing Judgement's `number` (from 0),
/// `it_802C7774`.
#[doc(alias = "it_802C7774")]
pub fn spawn(
    r: &mut Running,
    facing_dir: f32,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    number: u32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::GAME_WATCH_JUDGE,
        ..Default::default()
    };
    gamewatch::spawn_position(&mut spawn, pos, false);
    gamewatch::spawn_common(&mut spawn, Some(parent), facing_dir, true);
    let gobj = spawn_air(r, &mut spawn)?;
    let attr = gamewatch::first_attr(r, gobj);
    super::super::item::pick_up(r, gobj, parent, part);
    let frame = gekko_fp::frsp(f64::from(number.wrapping_add(1) as i32));
    super::drmariopill::set_pose(r, gobj, 0, crate::fpu::mem(frame));
    face_away(r, gobj);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    gamewatch::colors(r, gobj, parent, attr);
    Some(gobj)
}

/// The sign turned away from its owner's facing, `it_802C78B8`.
#[doc(alias = "it_802C78B8")]
pub fn face_away(r: &mut Running, gobj: GObjId) {
    let Some(owner) = item(r.game, gobj).owner else {
        return;
    };
    let facing = r.game.ft_facing_dir(owner);
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    let child = r.game.objs.jobjs[j].child.expect("jobj.h: jobj");
    let sign = r.game.objs.jobjs[child].child.expect("jobj.h: jobj");
    let y = if facing == 1.0 {
        crate::fpu::mem(gekko_fp::frsp(std::f64::consts::PI))
    } else {
        0.0
    };
    lib2725::set_rotation_y(r.game, sign, y);
}

/// Gone, `itGameWatchJudge_Logic77_Destroyed`: its owner forgets it.
#[doc(alias = "itGameWatchJudge_Logic77_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.gw_judgement_set_flag(owner);
    }
}

/// Taken away, `it_802C7A84`.
#[doc(alias = "it_802C7A84")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// Paused, `it_802C7AD0`.
#[doc(alias = "it_802C7AD0")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C7AF0`.
#[doc(alias = "it_802C7AF0")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Taken in hand, `it_2725_Logic77_PickedUp`: its state, not yet animated.
#[doc(alias = "it_2725_Logic77_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    if item(r.game, gobj).owner.is_some() {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
}

/// `itGamewatchjudge_UnkMotion0_Anim`: the sign kept facing away; gone once its owner leaves
/// Judgement.
#[doc(alias = "itGamewatchjudge_UnkMotion0_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    face_away(r, gobj);
    let go = match item(r.game, gobj).owner {
        Some(owner) => r.gw_judgement_should_go(owner),
        None => true,
    };
    if go {
        destroyed(r, gobj);
    }
    go
}

/// Forgets a GObj, `itGameWatchJudge_Logic77_EvtUnk`.
// Ports `itGameWatchJudge_Logic77_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
