//! Target Test's targets, `melee/it/kinds/itmato.c` (`It_Kind_Mato`): an enemy item the target
//! stages put at their target points (`Ground_801C4210`, `gr::target`), which follows its
//! point's joint and breaks at the first hit, counting the stage's targets down
//! (`Ground_801C4338`; the last ends the match).
//!
//! Its variables are the enemies' common words alone (`itZako_ItemVars`, the union's bytes as
//! the containers' monsters keep them): `x0` is the joint it follows.

use super::super::item::{change, change_state};
use super::super::{item_mut, zako};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_84F8;
pub const DMG_RECEIVED: u32 = 0x802D_85F4;
pub const ANIM0: u32 = 0x802D_858C;
pub const PHYS0: u32 = 0x802D_8594;
pub const COLL0: u32 = 0x802D_85EC;

/// Runs one of the target's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | COLL0 => still(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of the target's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => follow(r, gobj),
        _ => return false,
    }
    true
}

/// A new target, `it_802D84F8`: the enemies' common words reset (`it_8027B730`), facing 0, its
/// `xD5C` cleared and `xDCC_flag.b3` off, then still in its one state.
#[doc(alias = "it_802D84F8")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xD5C = 0;
    ip.xDCC_flag.b3 = false;
    enter_still(r, gobj);
}

/// The target's one state, `it_802D8554`: no velocity, state 0 with its animation.
#[doc(alias = "it_802D8554")]
pub fn enter_still(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The state's animation and collision callbacks, `itMato_UnkMotion0_Anim` and
/// `itMato_UnkMotion0_Coll`: nothing, never ending it.
#[doc(alias = "itMato_UnkMotion0_Anim")]
#[doc(alias = "itMato_UnkMotion0_Coll")]
pub fn still(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// The state's physics, `itMato_UnkMotion0_Phys`: where its joint is, if it has one
/// (`lb_8000B1CC`).
#[doc(alias = "itMato_UnkMotion0_Phys")]
pub fn follow(r: &mut Running, gobj: GObjId) {
    let Some(j) = zako::bytes(r.game, gobj).jobj(zako::off::JOBJ) else {
        return;
    };
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
    item_mut(r.game, gobj).pos = pos;
}

/// A hit on the target, `it_802D85F4`: the stage's targets counted down (`Ground_801C4338`),
/// and the target destroyed.
#[doc(alias = "it_802D85F4")]
pub fn damaged(r: &mut Running, _gobj: GObjId) -> bool {
    crate::game::gr::ground::target_broken(r.game);
    true
}
