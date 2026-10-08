//! Sheik's character callbacks (`ftseak.c`): the damage hook, the full charge's flash, the
//! item hooks and the knockback hooks. Her creation and spawn (`ftSk_Init_OnLoad`,
//! `ftSk_Init_OnDeath`, `ftSk_Init_LoadSpecialAttrs`) come with fighter creation (`ft::kinds`).

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

    /// `ftSk_Init_UnkMotionStates4`: the full charge's flash, while all six needles are held.
    #[doc(alias = "ftSk_Init_UnkMotionStates4")]
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

    /// Entering knockback, `ftSk_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj, 1)`):
    /// the hurt textures.
    #[doc(alias = "ftSk_Init_OnKnockbackEnter")]
    pub fn seak_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftSk_Init_OnKnockbackExit`.
    #[doc(alias = "ftSk_Init_OnKnockbackExit")]
    pub fn seak_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DAMAGE_CB => g.seak_damage_cb(gobj),
        addr::UNK_MOTION_STATES_4 => g.seak_full_charge_flash(gobj),
        addr::ON_ITEM_INVISIBLE => g.seak_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.seak_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.seak_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.seak_on_knockback_exit(gobj),
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
