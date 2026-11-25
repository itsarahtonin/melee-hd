//! Mr. Game & Watch's parachute, `melee/it/kinds/itgamewatchparachute.c`: out through his neutral aerial, gone after its landing animation's 30th frame or once he leaves the aerial. Its hooks are his held items' ([`gamewatch::Held`]).

use super::gamewatch::{self, Clears, Held};
use crate::game::ft::gamewatch::attack_air::PARACHUTE;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const DESTROYED: u32 = 0x802C_6D3C;
pub const PICKED_UP: u32 = 0x802C_6DF8;
pub const ANIM: u32 = 0x802C_6E8C;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_6F20;

/// The parachute's settings: his neutral aerial's, its first script variable cleared as it's taken, hidden as its landing animation starts, gone on its 30th frame.
pub const HELD: Held = Held {
    kind: ItKind::GameAndWatchParachute,
    forget: |r, f| r.gw_held_set_flag(f, &PARACHUTE),
    owner_done: |r, f| r.gw_held_should_go(f, &PARACHUTE),
    clears: Clears::First,
    owner_air: false,
    hides_on_land: true,
    gone_at: Some(30.0),
};

/// Runs one of the parachute's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => gamewatch::held_anim(r, gobj, &HELD),
        _ => return None,
    })
}

/// Runs one of the parachute's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => gamewatch::held_destroyed(r, gobj, &HELD),
        PICKED_UP => gamewatch::held_picked_up(r, gobj, &HELD),
        _ => return false,
    }
    true
}
