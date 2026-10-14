//! Placeholder while the port is written.

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

pub(super) fn run(_g: &mut Running, _gobj: GObjId, _a: u32) -> bool {
    false
}

impl Running<'_> {
    pub fn peach_toad_on_death2(&mut self, _gobj: GObjId) {
        crate::game::code::unported_call("ftPe_SpecialN_OnDeath2");
    }
}
