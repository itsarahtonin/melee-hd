//! Crazy Hand's entrance (`ftcrazyhandgrabunk1b174.c`): the hands' (`ft::masterhand::entry`),
//! in his state 0x183, facing right, Master Hand's camera script with its interest on his
//! waist.

use super::super::super::state::Running;
use super::{HAND, addr};
use crate::hsd::gobj::GObjId;

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTRY_ENTER => g.hand_entry_enter(gobj, &HAND),
        addr::TAG_FAIL_ANIM => g.hand_entry_anim(gobj, &HAND),
        addr::ENTRY_CAMERA => g.hand_entry_camera(gobj, &HAND),
        _ => return false,
    }
    true
}
