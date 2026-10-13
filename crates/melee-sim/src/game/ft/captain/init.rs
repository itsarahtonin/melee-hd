//! Captain Falcon's and Ganondorf's character callbacks (`ftcaptain.c`, `ftganon.c`): the side
//! special's effect hook and the item and knockback hooks. Their creation and spawn
//! (`ftCa_Init_OnLoad`, `ftCa_Init_OnDeath`, `ftCa_Init_LoadSpecialAttrs` and Ganondorf's)
//! come with fighter creation (`ft::kinds`).

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The side special's damage and death hook, `ftCa_Init_800E28C8`: its effects go.
    #[doc(alias = "ftCa_Init_800E28C8")]
    pub fn ca_remove_gfx_cb(&mut self, gobj: GObjId) {
        self.ca_special_s_remove_gfx(gobj);
    }

    /// Picking up an item, `ftCa_Init_OnItemPickup` and `ftGn_Init_OnItemPickup`
    /// (`Fighter_OnItemPickup(gobj, flag, 1, 1)`, as Fox's).
    // Ports `ftCa_Init_OnItemPickup` and `ftGn_Init_OnItemPickup`; aliased once a check reaches them.
    pub fn ca_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftCa_Init_OnItemInvisible` and `ftGn_Init_OnItemInvisible` (as
    /// Fox's).
    // Ports `ftCa_Init_OnItemInvisible` and `ftGn_Init_OnItemInvisible`; aliased once a check reaches them.
    pub fn ca_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftCa_Init_OnItemVisible` and `ftGn_Init_OnItemVisible`
    /// (as Fox's).
    // Ports `ftCa_Init_OnItemVisible` and `ftGn_Init_OnItemVisible`; aliased once a check reaches them.
    pub fn ca_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftCa_Init_OnItemDrop` and `ftGn_Init_OnItemDrop` (as Fox's).
    // Ports `ftCa_Init_OnItemDrop` and `ftGn_Init_OnItemDrop`; aliased once a check reaches them.
    pub fn ca_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// Ganondorf entering knockback, `ftGn_Init_OnKnockbackEnter`: his hurt model parts.
    // Ports `ftGn_Init_OnKnockbackEnter`; aliased once a check reaches it.
    pub fn gn_on_knockback_enter(&mut self, gobj: GObjId) {
        self.apply_part_anim(gobj, 3, 3, 0.0);
        self.apply_part_anim(gobj, 4, 3, 0.0);
    }

    /// Ganondorf leaving knockback, `ftGn_Init_OnKnockbackExit`.
    // Ports `ftGn_Init_OnKnockbackExit`; aliased once a check reaches it.
    pub fn gn_on_knockback_exit(&mut self, gobj: GObjId) {
        self.apply_part_anim(gobj, 3, 2, 0.0);
        self.apply_part_anim(gobj, 4, 2, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::REMOVE_GFX_CB => g.ca_remove_gfx_cb(gobj),
        addr::ON_ITEM_INVISIBLE | addr::GN_ON_ITEM_INVISIBLE => g.ca_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE | addr::GN_ON_ITEM_VISIBLE => g.ca_on_item_visible(gobj),
        addr::GN_ON_KNOCKBACK_ENTER => g.gn_on_knockback_enter(gobj),
        addr::GN_ON_KNOCKBACK_EXIT => g.gn_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP | addr::GN_ON_ITEM_PICKUP => {
            r.ca_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP | addr::GN_ON_ITEM_DROP => {
            r.ca_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
