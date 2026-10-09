//! Sheik's character callbacks (`ftseak.c`): the damage hook, the full charge's flash and the
//! item hooks. Her knockback hooks are in `ft::co::hit`, and her creation and spawn
//! (`ftSk_Init_OnLoad`, `ftSk_Init_OnDeath`, `ftSk_Init_LoadSpecialAttrs`) come with fighter
//! creation (`ft::kinds`).

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Sheik's damage and death hook, `ftSk_Init_80110198`: the held needles are thrown
    /// loose and the chain goes.
    #[doc(alias = "ftSk_Init_80110198")]
    pub fn seak_damage_cb(&mut self, gobj: GObjId) {
        self.seak_drop_needles(gobj);
        self.seak_check_and_destroy_chain(gobj);
    }

    /// `ftSk_Init_UnkMotionStates4`, by address: the full charge's flash, while all six needles
    /// are held (`ft::colanim` ports it, inline where the color animations reset).
    pub fn seak_full_charge_flash(&mut self, gobj: GObjId) {
        if self.seak_needle_count(gobj) == 6 {
            self.start_color_anim(gobj, 86, 0);
        }
    }

    /// Picking up an item, `ftSk_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    #[doc(alias = "ftSk_Init_OnItemPickup")]
    pub fn seak_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftSk_Init_OnItemInvisible` (as Fox's).
    #[doc(alias = "ftSk_Init_OnItemInvisible")]
    pub fn seak_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftSk_Init_OnItemVisible` (as Fox's).
    #[doc(alias = "ftSk_Init_OnItemVisible")]
    pub fn seak_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftSk_Init_OnItemDrop` (as Fox's).
    #[doc(alias = "ftSk_Init_OnItemDrop")]
    pub fn seak_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DAMAGE_CB => g.seak_damage_cb(gobj),
        addr::UNK_MOTION_STATES_4 => g.seak_full_charge_flash(gobj),
        addr::ON_ITEM_INVISIBLE => g.seak_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.seak_on_item_visible(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.seak_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.seak_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
