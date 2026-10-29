//! Zelda's character callbacks (`ftzelda.c`): the item hooks. Her knockback hooks are in
//! `ft::co::hit`.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Her side taunt's swap at the debug ROM's level, `ftZd_Init_801395C8`: the second
    /// model's variant on if it was off, else off.
    // Ports `ftZd_Init_801395C8`; aliased once a check reaches it.
    pub fn zelda_appeal_swap(&mut self, gobj: GObjId) {
        let v = if self.game.part_vis(gobj, 1) == -1 {
            0
        } else {
            -1
        };
        self.game.set_part_vis(gobj, 1, v);
    }

    /// Picking up an item, `ftZd_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    // Ports `ftZd_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn zelda_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftZd_Init_OnItemInvisible` (as Fox's).
    // Ports `ftZd_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn zelda_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftZd_Init_OnItemVisible` (as Fox's).
    // Ports `ftZd_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn zelda_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftZd_Init_OnItemDrop` (as Fox's).
    #[doc(alias = "ftZd_Init_OnItemDrop")]
    pub fn zelda_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.zelda_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.zelda_on_item_visible(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.zelda_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.zelda_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
