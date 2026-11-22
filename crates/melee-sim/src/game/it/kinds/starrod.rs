//! The Star Rod, `melee/it/kinds/itstarrod.c`: a swung item with charges (`xD4C`) that falls,
//! lands and is held; each swing's script flag fires a star (`itstarrodstar.c`) while charges
//! last. Thrown, it bounces off what it hits; empty, it falls through the stage. Its
//! callbacks are the held items' ([`super::held`]).

use super::held::{self, Held};
use super::starrodstar;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_2360;
pub const ANIM0: u32 = 0x8029_2450;
pub const PHYS0: u32 = 0x8029_2458;
pub const COLL0: u32 = 0x8029_245C;
pub const ANIM4: u32 = 0x8029_24B0;
pub const PHYS1: u32 = 0x8029_24B8;
pub const COLL1: u32 = 0x8029_24E8;
pub const PICKED_UP: u32 = 0x8029_2514;
pub const ANIM2: u32 = 0x8029_253C;
pub const PHYS2: u32 = 0x8029_2544;
pub const DROPPED: u32 = 0x8029_2548;
pub const COLL4: u32 = 0x8029_2570;
pub const THROWN: u32 = 0x8029_25B4;
pub const PHYS4: u32 = 0x8029_25DC;
pub const COLL3: u32 = 0x8029_2628;
pub const DMG_DEALT: u32 = 0x8029_266C;
pub const ENTERED_AIR: u32 = 0x8029_26A8;
pub const ANIM5: u32 = 0x8029_26D0;
pub const PHYS5: u32 = 0x8029_26D8;
pub const COLL5: u32 = 0x8029_26DC;
pub const CLANKED: u32 = 0x8029_2710;
pub const REFLECTED: u32 = 0x8029_274C;
pub const HIT_SHIELD: u32 = 0x8029_276C;
pub const SHIELD_BOUNCED: u32 = 0x8029_27A8;

/// The Star Rod: its charges, thrown into state 3 and dropped into 4, skidding in 5, its hits
/// bouncing it only once let go of.
pub const STAR_ROD: Held = Held {
    name: "Star Rod",
    charges: true,
    on_spawn: None,
    fell: held::fell,
    dropped: 4,
    thrown: 3,
    drop_update: true,
    skid: 5,
    bounces_only_let_go: true,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of the Star Rod's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM4 | ANIM2 | ANIM5 => held::still(r, gobj),
        COLL0 => held::grounded(r, gobj, &STAR_ROD),
        COLL1 => held::lands(r, gobj),
        COLL3 | COLL4 => held::lands_loaded(r, gobj),
        COLL5 => held::skid(r, gobj, &STAR_ROD),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &STAR_ROD),
        REFLECTED => held::reflected(r, gobj, &STAR_ROD),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &STAR_ROD),
        _ => return None,
    })
}

/// Runs one of the Star Rod's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &STAR_ROD),
        PHYS0 | PHYS2 | PHYS5 => held::idle(r, gobj),
        PHYS1 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED => held::dropped(r, gobj, &STAR_ROD),
        THROWN => held::thrown(r, gobj, &STAR_ROD),
        PHYS4 => held::thrown_phys(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &STAR_ROD),
        _ => return false,
    }
    true
}

/// Where a star leaves from, in the hand's space ([`held::hand_offset`]).
pub fn star_offset(r: &Running, gobj: GObjId) -> Vec3 {
    held::hand_offset(r, gobj, &STAR_ROD)
}

/// Fires a star from `pos`, facing `facing` (`big` for a smash swing's), a charge spent,
/// `it_802923BC`.
#[doc(alias = "it_802923BC")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, facing: f32, big: i32) {
    let owner = held::spend_charge(r, gobj).expect("a Star Rod fired with no owner");
    starrodstar::fire(r, owner, pos, big, facing);
}
