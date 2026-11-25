//! Mr. Game & Watch's Sparky, `melee/it/kinds/itgamewatchbreath.c`: out through his up aerial, gone once he leaves it. Its hooks are his held items' ([`gamewatch::Held`]).

use super::gamewatch::{self, Clears, Held};
use crate::game::ft::gamewatch::attack_air::SPARKY;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const DESTROYED: u32 = 0x802C_7310;
pub const PICKED_UP: u32 = 0x802C_73CC;
pub const ANIM: u32 = 0x802C_744C;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_74B8;

/// The Sparky's settings: his up aerial's, its first script variable cleared as it's taken.
pub const HELD: Held = Held {
    kind: ItKind::GameAndWatchSparky,
    forget: |r, f| r.gw_held_set_flag(f, &SPARKY),
    owner_done: |r, f| r.gw_held_should_go(f, &SPARKY),
    clears: Clears::First,
    owner_air: false,
    hides_on_land: false,
    gone_at: None,
};

/// Runs one of the Sparky's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => gamewatch::held_anim(r, gobj, &HELD),
        _ => return None,
    })
}

/// Runs one of the Sparky's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => gamewatch::held_destroyed(r, gobj, &HELD),
        PICKED_UP => gamewatch::held_picked_up(r, gobj, &HELD),
        _ => return false,
    }
    true
}
