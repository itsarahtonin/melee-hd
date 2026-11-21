//! Crazy Hand's character callbacks (`ftcrazyhand.c`; the hands' shared ones,
//! `ft::masterhand::init`) and his empty hit hook (`ftcrazyhandwait10.c`'s
//! `ftCh_Init_80156014`).

use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_ON_DEATH | addr::NOTHING => g.hand_nothing(gobj),
        _ => return false,
    }
    true
}
