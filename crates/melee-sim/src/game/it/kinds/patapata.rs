//! The Koopa Paratroopa, `melee/it/kinds/itpatapata.c` (`It_Kind_Patapata`).

use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// Spawns a Koopa Paratroopa (`it_802E16F8`): not ported yet.
pub fn spawn(_r: &mut Running, _x2: i32, _pos: Vec3, _slot: i32) -> Option<GObjId> {
    crate::game::code::unported_call("it_802E16F8 (a Koopa Paratroopa)")
}
