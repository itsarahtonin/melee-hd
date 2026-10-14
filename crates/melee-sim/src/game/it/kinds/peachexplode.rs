//! The Peach Bomber's explosion, `melee/it/kinds/itpeachexplode.c`: made at her hips when the
//! attack hits, hidden, its hitbox live for its life (a smash's larger, state 1).

use super::super::item::{change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item_mut, lib26b1, lib2725};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DMG_DEALT: u32 = 0x802B_D240;
pub const MOTION1_ANIM: u32 = 0x802B_D2EC;
pub const EVT_UNK: u32 = 0x802B_D30C;

/// `It_Kind_Peach_Explode`.
pub const KIND: i32 = 98;

/// Runs one of the explosion's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT => dmg_dealt(r, gobj),
        MOTION1_ANIM => animated(r, gobj),
        _ => return None,
    })
}

/// Runs one of the explosion's events, if `addr` is one.
pub fn run_event(_r: &mut Running, _gobj: GObjId, _addr: u32) -> bool {
    false
}

/// Makes the explosion for `parent` at `pos` (a smash's when `smash`), `it_802BD158`.
#[doc(alias = "it_802BD158")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    smash: bool,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, keeping its depth.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    setup(r, gobj, parent, smash);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// `itPeachExplode_Logic55_DmgDealt`: nothing.
#[doc(alias = "itPeachExplode_Logic55_DmgDealt")]
fn dmg_dealt(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// The explosion's setup, `it_802BD248`: its owner, 60 frames of life, hidden, its blast
/// effect, not to be picked up, its hitbox's flag, vanishing at its life's end, its state (1
/// for a smash), sized as its owner.
#[doc(alias = "it_802BD248")]
pub fn setup(r: &mut Running, gobj: GObjId, owner: GObjId, smash: bool) {
    let ip = item_mut(r.game, gobj);
    ip.owner = Some(owner);
    ip.xD44_lifeTimer = 60.0;
    ip.xDAC_itcmd_var0 = 0;
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    change_state(r, gobj, i32::from(smash), change::ANIM_UPDATE);
    // `it_80274574`.
    lib2725::scale_by_owner(r, gobj);
}

/// `itPeachexplode_UnkMotion1_Anim`: gone once its life is out.
#[doc(alias = "itPeachexplode_UnkMotion1_Anim")]
fn animated(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// `itPeachExplode_Logic55_EvtUnk`: forgets `other`.
#[doc(alias = "itPeachExplode_Logic55_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
