//! The Koopas' green shell, `melee/it/kinds/itzgshell.c` (`It_Kind_ZGShell`).

use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// Runs one of the green shell's events, if `addr` is one.
pub fn run_event(_r: &mut Running, _gobj: GObjId, _addr: u32) -> bool {
    false
}

/// Runs one of the green shell's predicates, if `addr` is one.
pub fn run_pred(_r: &mut Running, _gobj: GObjId, _addr: u32) -> Option<bool> {
    None
}

/// Spawns a green shell (`it_802E0100`): not ported yet.
pub fn spawn(_r: &mut Running, _kind: i32, _pos: Vec3, _facing: i32) -> Option<GObjId> {
    crate::game::code::unported_call("it_802E0100 (a Koopa's green shell)")
}
