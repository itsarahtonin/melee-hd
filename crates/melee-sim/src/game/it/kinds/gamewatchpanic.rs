//! Mr. Game & Watch's oil, `melee/it/kinds/itgamewatchpanic.c`: thrown from the full bucket
//! through Oil Panic's throw, on the ground or in the air. Its hooks are his held items'
//! ([`gamewatch::Held`]).

use super::gamewatch::{self, Clears, Held};
use crate::game::ft::gamewatch::special_lw::PANIC;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const DESTROYED: u32 = 0x802C_7E64;
pub const PICKED_UP: u32 = 0x802C_7F20;
pub const ANIM: u32 = 0x802C_7FAC;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_8018;

/// The oil's settings: Oil Panic's throw's, both script variables cleared as it's taken, on the ground or in the air as he is.
pub const HELD: Held = Held {
    kind: ItKind::GameAndWatchOil,
    forget: |r, f| r.gw_held_set_flag(f, &PANIC),
    owner_done: |r, f| r.gw_held_should_go(f, &PANIC),
    clears: Clears::Both,
    owner_air: true,
    hides_on_land: false,
    gone_at: None,
};

/// Runs one of the oil's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => gamewatch::held_anim(r, gobj, &HELD),
        _ => return None,
    })
}

/// Runs one of the oil's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => gamewatch::held_destroyed(r, gobj, &HELD),
        PICKED_UP => gamewatch::held_picked_up(r, gobj, &HELD),
        _ => return false,
    }
    true
}
