//! The pan of Kirby's copy of Chef, `melee/it/kinds/itkirbygamewatchchefpan.c`: held at his
//! thumb and colored as his copy, gone once he leaves the move. Its hooks are Mr. Game &
//! Watch's held items' ([`gamewatch::Held`]).

use super::gamewatch::{self, Clears, Held};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const DESTROYED: u32 = 0x802C_75DC;
pub const PICKED_UP: u32 = 0x802C_7698;
pub const ANIM: u32 = 0x802C_76E8;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_7754;

/// The pan's settings: Kirby's copy's, its script variables left as they are.
pub const HELD: Held = Held {
    kind: ItKind::KirbyGameAndWatchPan,
    forget: |r, f| r.kb_gw_pan_gone(f),
    owner_done: |r, f| r.kb_gw_done(f),
    clears: Clears::None,
    owner_air: false,
    hides_on_land: false,
    gone_at: None,
};

/// Runs one of the pan's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => gamewatch::held_anim(r, gobj, &HELD),
        _ => return None,
    })
}

/// Runs one of the pan's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => gamewatch::held_destroyed(r, gobj, &HELD),
        PICKED_UP => gamewatch::held_picked_up(r, gobj, &HELD),
        _ => return false,
    }
    true
}
