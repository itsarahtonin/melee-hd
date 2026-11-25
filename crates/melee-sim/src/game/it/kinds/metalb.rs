//! The Metal Box, `melee/it/kinds/itmetalb.c`: it falls and rests; the fighter that breaks it
//! (or picks it up, `ftpickupitem_8009447C`) turns metal (`ftLib_ApplyMetalBox`). Its callbacks
//! but the break are the power-ups' ([`powerup`]).

use super::super::item::is_fighter;
use super::super::{coll, flag, item, item_mut};
use super::powerup::{self, PowerUp};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_53C8;
pub const ANIM0: u32 = 0x8029_544C;
pub const PHYS0: u32 = 0x8029_5454;
pub const COLL0: u32 = 0x8029_5458;
pub const ANIM3: u32 = 0x8029_54C0;
pub const PHYS1: u32 = 0x8029_54C8;
pub const COLL1: u32 = 0x8029_54F8;
pub const PICKED_UP: u32 = 0x8029_5524;
pub const ANIM2: u32 = 0x8029_554C;
pub const DROPPED: u32 = 0x8029_5554;
pub const PHYS3: u32 = 0x8029_557C;
pub const COLL3: u32 = 0x8029_55AC;
pub const DMG_RECEIVED: u32 = 0x8029_55E0;
pub const ENTERED_AIR: u32 = 0x8029_5684;
pub const ANIM4: u32 = 0x8029_56AC;
pub const PHYS4: u32 = 0x8029_56B4;
pub const COLL4: u32 = 0x8029_56B8;

/// The Metal Box: its spawn clears its hurt flag; resting, it tilts with the floor.
pub const METAL_BOX: PowerUp = PowerUp {
    clears_hurt: true,
    on_spawn: None,
    tilts: true,
    on_pickup: None,
    on_drop: None,
};

/// Runs one of the Metal Box's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM2 | ANIM4 => powerup::still(r, gobj),
        COLL0 => powerup::rests(r, gobj, &METAL_BOX),
        COLL1 => powerup::lands(r, gobj),
        COLL3 => powerup::vanishes(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        COLL4 => powerup::skids(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Metal Box's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => powerup::spawned(r, gobj, &METAL_BOX),
        PHYS0 | PHYS4 => powerup::idle(r, gobj),
        PHYS1 | PHYS3 => powerup::gravity(r, gobj),
        PICKED_UP => powerup::picked_up(r, gobj, &METAL_BOX),
        DROPPED => powerup::dropped(r, gobj, &METAL_BOX),
        ENTERED_AIR => powerup::entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itMetalB_Logic32_DmgReceived`: broken, it turns its breaker metal when that's a fighter
/// whose hit could open it (`xDCF` bit 6), which counts the item for its player; gone either
/// way.
#[doc(alias = "ftLib_IsSubFighter")]
#[doc(alias = "itMetalB_Logic32_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let Some(by) = coll::take_attacker(r.game, gobj) else {
        return true;
    };
    if is_fighter(r.game, Some(by)) && flag(item(r.game, gobj).draw_flags, 6) {
        r.apply_metal_box(by, gobj);
        let fp = r.game.fighter(by);
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        r.item_pickup_stat(slot, sub, gobj);
        return true;
    }
    item_mut(r.game, gobj).attacker = None;
    true
}
