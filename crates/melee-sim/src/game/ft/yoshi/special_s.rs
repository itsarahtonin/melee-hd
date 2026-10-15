//! Placeholder.

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

pub(super) fn run(_g: &mut Running, _gobj: GObjId, _a: u32) -> bool {
    false
}
