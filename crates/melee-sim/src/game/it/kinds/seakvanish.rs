//! The flash Sheik's Vanish leaves where she was, `melee/it/kinds/itseakvanish.c`: an
//! explosion that lasts 60 frames.

use super::super::item::{change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item_mut, lib26b1, lib2725};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802B_1C60;
pub const DMG_DEALT: u32 = 0x802B_1D38;
pub const INIT: u32 = 0x802B_1D40;
pub const ANIM: u32 = 0x802B_1DAC;
pub const EVT_UNK: u32 = 0x802B_1DCC;

/// Runs one of the flash's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        _ => return None,
    })
}

/// Runs one of the flash's events, if `addr` is one.
pub fn run_event(_r: &mut Running, _gobj: GObjId, _addr: u32) -> bool {
    false
}

/// Makes the flash at `pos`, `it_802B1C60`.
#[doc(alias = "it_802B1C60")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::SEAK_VANISH,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, off the plane.
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
    init(r, gobj, parent);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// Starts the flash, `it_802B1D40`: 60 frames, hidden from pickup and vanishing.
#[doc(alias = "it_802B1D40")]
pub fn init(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.owner = Some(owner);
    ip.xD44_lifeTimer = 60.0;
    ip.xDAC_itcmd_var0 = 0;
    lib26b1::hide(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The flash's animation, `itSeakvanish_UnkMotion0_Anim`: gone once its life runs out.
#[doc(alias = "itSeakvanish_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Hitting something, `itSeakVanish_Logic42_DmgDealt`: the flash stays.
#[doc(alias = "itSeakVanish_Logic42_DmgDealt")]
pub fn dmg_dealt(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Forgets a GObj the flash points to, `it_802B1DCC` (`it_8026B894`).
#[doc(alias = "it_802B1DCC")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
