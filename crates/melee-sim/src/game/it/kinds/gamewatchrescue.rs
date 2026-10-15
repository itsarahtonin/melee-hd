//! Mr. Game & Watch's trampoline, `melee/it/kinds/itgamewatchrescue.c`: the firemen under him
//! through Fire, on the ground (state 0) or in the air (state 1), gone once he leaves Fire.

use super::super::item::{animate, change, change_state, destroy, spawn_air};
use super::super::{item, item_mut, lib26b1};
use super::gamewatch::{self, GameWatchRescueVars};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x802C_8240;
pub const PHYS: u32 = 0x802C_8350;
pub const COLL: u32 = 0x802C_8354;
pub const EVT_UNK: u32 = 0x802C_835C;

/// Runs one of the trampoline's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => collide(r, gobj),
        _ => return None,
    })
}

/// Runs one of the trampoline's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => physics(r, gobj),
        _ => return false,
    }
    true
}

fn vars<'r>(r: &'r mut Running<'_>, gobj: GObjId) -> &'r mut GameWatchRescueVars {
    item_mut(r.game, gobj).xDD4_itemVar.game_watch_rescue()
}

/// The trampoline under `parent` in state `state`, `it_802C8038`: flattened onto the stage's
/// plane, without a first collision, not held but colored as he is. (`part` and `farg1` go
/// unread.)
#[doc(alias = "it_802C8038")]
pub fn spawn(
    r: &mut Running,
    parent: Option<GObjId>,
    pos: Vec3,
    _part: i32,
    state: i32,
    facing_dir: f32,
    _farg1: f32,
) -> Option<GObjId> {
    let parent = parent?;
    let mut spawn = SpawnItem {
        kind: super::GAME_WATCH_RESCUE,
        ..Default::default()
    };
    gamewatch::spawn_position(&mut spawn, pos, true);
    gamewatch::spawn_common(&mut spawn, Some(parent), facing_dir, false);
    let gobj = spawn_air(r, &mut spawn)?;
    let attr = gamewatch::first_attr(r, gobj);
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDCC_flag.b3 = false;
    vars(r, gobj).parent.set(Some(parent));
    gamewatch::colors(r, gobj, parent, attr);
    set_state(r, gobj, state);
    Some(gobj)
}

/// Taken away, `it_802C8158`: its fighter, if still its owner, forgets it.
#[doc(alias = "it_802C8158")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    let parent = vars(r, gobj).parent.get();
    if let Some(p) = parent
        && item(r.game, gobj).owner == Some(p)
    {
        r.gw_rescue_set_null(p);
    }
    vars(r, gobj).parent.set(None);
    item_mut(r.game, gobj).owner = None;
    destroy(r, gobj);
}

/// Paused, `it_802C81C8`.
#[doc(alias = "it_802C81C8")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_start(r, gobj);
}

/// Running again, `it_802C81E8`.
#[doc(alias = "it_802C81E8")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    gamewatch::hitlag_end(r, gobj);
}

/// Its state, animating, `it_802C8208`.
#[doc(alias = "it_802C8208")]
pub fn set_state(r: &mut Running, gobj: GObjId, state: i32) {
    change_state(r, gobj, state, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// `itGamewatchrescue_UnkMotion1_Anim`: once its fighter leaves Fire (or it has none), it is
/// taken away there (`it_802C8158` inlined), its fighter and owner cleared once more after,
/// and the destroy asked for again.
#[doc(alias = "itGamewatchrescue_UnkMotion1_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    if let Some(p) = vars(r, gobj).parent.get()
        && !r.gw_rescue_should_go(p)
    {
        return false;
    }
    remove(r, gobj);
    vars(r, gobj).parent.set(None);
    item_mut(r.game, gobj).owner = None;
    true
}

/// `itGamewatchrescue_UnkMotion1_Phys`: nothing.
#[doc(alias = "itGamewatchrescue_UnkMotion1_Phys")]
pub fn physics(_r: &mut Running, _gobj: GObjId) {}

/// `itGamewatchrescue_UnkMotion1_Coll`: nothing.
#[doc(alias = "itGamewatchrescue_UnkMotion1_Coll")]
pub fn collide(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Forgets a GObj, `itGameWatchRescue_Logic81_EvtUnk`.
// Ports `itGameWatchRescue_Logic81_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
