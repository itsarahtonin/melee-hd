//! The Cloaking Device, `melee/it/kinds/itspycloak.c`: it falls and rests; the fighter that
//! picks it up turns invisible (`ftpickupitem_8009447C`, `ftCo_800C88D4`). Its callbacks are the
//! power-ups' ([`powerup`]).

use super::powerup::{self, PowerUp};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_5C48;
pub const ANIM0: u32 = 0x8029_5CB8;
pub const PHYS0: u32 = 0x8029_5CC0;
pub const COLL0: u32 = 0x8029_5CC4;
pub const ANIM3: u32 = 0x8029_5D2C;
pub const PHYS1: u32 = 0x8029_5D34;
pub const COLL1: u32 = 0x8029_5D64;
pub const PICKED_UP: u32 = 0x8029_5D90;
pub const ANIM2: u32 = 0x8029_5DB8;
pub const DROPPED: u32 = 0x8029_5DC0;
pub const PHYS3: u32 = 0x8029_5DE8;
pub const COLL3: u32 = 0x8029_5E18;
pub const ENTERED_AIR: u32 = 0x8029_5E4C;
pub const ANIM4: u32 = 0x8029_5E74;
pub const PHYS4: u32 = 0x8029_5E7C;
pub const COLL4: u32 = 0x8029_5E80;

/// The Cloaking Device: resting, it tilts with the floor.
pub const CLOAKING_DEVICE: PowerUp = PowerUp {
    clears_hurt: false,
    on_spawn: None,
    tilts: true,
    on_pickup: None,
    on_drop: None,
};

/// Runs one of the Cloaking Device's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => powerup::still(r, gobj),
        COLL0 => powerup::rests(r, gobj, &CLOAKING_DEVICE),
        COLL1 => powerup::lands(r, gobj),
        COLL3 => powerup::vanishes(r, gobj),
        COLL4 => powerup::skids(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Cloaking Device's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => powerup::spawned(r, gobj, &CLOAKING_DEVICE),
        PHYS0 | PHYS4 => powerup::idle(r, gobj),
        PHYS1 | PHYS3 => powerup::gravity(r, gobj),
        PICKED_UP => powerup::picked_up(r, gobj, &CLOAKING_DEVICE),
        DROPPED => powerup::dropped(r, gobj, &CLOAKING_DEVICE),
        ENTERED_AIR => powerup::entered_air(r, gobj),
        _ => return false,
    }
    true
}
