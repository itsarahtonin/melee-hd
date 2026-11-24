//! Marth's character callbacks (`ftmars.c`): the item hooks and the knockback hooks. His
//! creation and spawn (`ftMs_Init_OnLoad`, `ftMs_Init_OnDeath`, `ftMs_Init_LoadSpecialAttrs`)
//! come with fighter creation (`ft::kinds`); the neutral special's accessories are in
//! [`super::special_n`].

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::super::hooks::{HOLD_POSES, NoItem};
use super::addr;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Picking up an item, `ftMs_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 0,
    /// 1)`): the hand's hold pose for the item, unless it is heavy.
    #[doc(alias = "ftMs_Init_OnItemPickup")]
    pub fn mars_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup(gobj, flag, &HOLD_POSES, 0, 1);
    }

    /// Hiding the held item, `ftMs_Init_OnItemInvisible` (`Fighter_OnItemInvisible(gobj,
    /// 0)`).
    #[doc(alias = "ftMs_Init_OnItemInvisible")]
    pub fn mars_on_item_invisible(&mut self, gobj: GObjId) {
        self.hide_held_item(gobj, 0, NoItem::Stops);
    }

    /// Showing the held item again, `ftMs_Init_OnItemVisible` (`Fighter_OnItemVisible(gobj,
    /// 0)`).
    #[doc(alias = "ftMs_Init_OnItemVisible")]
    pub fn mars_on_item_visible(&mut self, gobj: GObjId) {
        self.show_held_item(gobj, 0, NoItem::Stops);
    }

    /// Dropping the item, `ftMs_Init_OnItemDrop` (`Fighter_OnItemDrop(gobj, flag, 0, 1)`).
    #[doc(alias = "ftMs_Init_OnItemDrop")]
    pub fn mars_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop(gobj, flag, 0, 1);
    }

    /// Entering knockback, `ftMs_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftMs_Init_OnKnockbackEnter")]
    pub fn mars_on_knockback_enter(&mut self, gobj: GObjId) {
        self.knockback_textures(gobj, 1, 3.0);
    }

    /// Leaving knockback, `ftMs_Init_OnKnockbackExit`.
    #[doc(alias = "ftMs_Init_OnKnockbackExit")]
    pub fn mars_on_knockback_exit(&mut self, gobj: GObjId) {
        self.knockback_textures(gobj, 1, 0.0);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.mars_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.mars_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.mars_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.mars_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

#[melee_callbacks::callbacks(Call)]
pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.mars_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.mars_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
