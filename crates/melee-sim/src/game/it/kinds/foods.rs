//! Foods, `melee/it/kinds/itfoods.c`: the spawn Mr. Game & Watch's Judgement 7 calls. With
//! items off, as in every replay at hand, no food spawns; the food itself isn't ported.

use crate::game::code;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// A food at `pos` from `parent`, if foods are on, `it_8028FAF4`: with foods off, none.
#[doc(alias = "it_8028FAF4")]
pub fn spawn(r: &mut Running, _parent: Option<GObjId>, _pos: Vec3) -> Option<GObjId> {
    if !super::super::spawn::kind_on(r.game, super::FOODS) {
        return None;
    }
    code::unported_call("it_8028FAF4 with foods on (a food)")
}
