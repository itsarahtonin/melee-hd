//! Mr. Game & Watch's turtle, `melee/it/kinds/itgamewatchturtle.c`: out through his back aerial, gone once he leaves it. Its hooks are his held items' ([`gamewatch::Held`]).

use super::gamewatch::{self, Clears, Held};
use crate::game::ft::gamewatch::attack_air::TURTLE;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const DESTROYED: u32 = 0x802C_7044;
pub const PICKED_UP: u32 = 0x802C_7100;
pub const ANIM: u32 = 0x802C_7180;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_71EC;

/// The turtle's settings: his back aerial's, its first script variable cleared as it's taken.
pub const HELD: Held = Held {
    kind: ItKind::GameAndWatchTurtle,
    forget: |r, f| r.gw_held_set_flag(f, &TURTLE),
    owner_done: |r, f| r.gw_held_should_go(f, &TURTLE),
    clears: Clears::First,
    owner_air: false,
    hides_on_land: false,
    gone_at: None,
};

/// Runs one of the turtle's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => gamewatch::held_anim(r, gobj, &HELD),
        _ => return None,
    })
}

/// Runs one of the turtle's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => gamewatch::held_destroyed(r, gobj, &HELD),
        PICKED_UP => gamewatch::held_picked_up(r, gobj, &HELD),
        _ => return false,
    }
    true
}
