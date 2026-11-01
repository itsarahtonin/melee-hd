//! The Koopas' red shell, `melee/it/kinds/itzrshell.c` (`It_Kind_ZRShell`).

use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// Spawns a red shell (`it_802E0488`): not ported yet.
pub fn spawn(_r: &mut Running, _kind: i32, _pos: Vec3, _facing: i32) -> Option<GObjId> {
    crate::game::code::unported_call("it_802E0488 (a Koopa's red shell)")
}
