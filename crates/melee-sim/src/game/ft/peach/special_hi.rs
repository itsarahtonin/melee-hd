//! Placeholder while the port is written.

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

pub(super) fn run(_g: &mut Running, _gobj: GObjId, _a: u32) -> bool {
    false
}

impl Running<'_> {
    pub fn peach_parasol_put_away(&mut self, _gobj: GObjId) {
        crate::game::code::unported_call("ftPe_8011D598");
    }
}
