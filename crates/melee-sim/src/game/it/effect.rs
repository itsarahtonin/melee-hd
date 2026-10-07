//! Item effects, `melee/it/iteffect.c`: loading `ItCo.dat` and spawning effects on items. The
//! effects themselves aren't ported, so spawning one stops the simulation.

use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// Spawns effect `ef_id` at the item, `it_802787B4` (`it_80278800` with no offsets).
#[doc(alias = "it_802787B4")]
pub fn spawn_simple(_r: &mut Running, _gobj: GObjId, ef_id: i32) {
    code::unported_call(&format!("it_80278800 (item effect {ef_id:#x})"));
}

/// Destroys the item's effects (`efLib_DestroyAll`).
pub fn destroy_all(r: &mut Running, gobj: GObjId) {
    crate::game::ef::destroy_all(r, gobj);
}
