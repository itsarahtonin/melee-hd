//! Falco's character callbacks (`ftfalco.c`): his creation, spawn and attribute copy, and the
//! item and knockback hooks, which are copies of Fox's.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

/// `It_Kind_Falco_Phantasm`.
const IT_KIND_FALCO_PHANTASM: i32 = 57;

impl Running<'_> {
    /// Falco's creation, `ftFc_Init_OnLoad`: he can wall jump, has Fox's attribute layout
    /// (`ftFx_Init_OnLoadForFalco`), and registers his blaster's shot, his blaster and his
    /// Phantasm with the items. Phantasm is his fourth article, not his third as Fox's
    /// Illusion is.
    #[doc(alias = "ftFc_Init_OnLoad")]
    pub fn falco_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].can_walljump = true;
        self.fox_on_load_for_falco(f);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let kinds = [
            (
                0,
                self.fox_attr_i32(f, super::super::fox::attr::BLASTER_SHOT_ITKIND),
            ),
            (
                1,
                self.fox_attr_i32(f, super::super::fox::attr::BLASTER_GUN_ITKIND),
            ),
            (3, IT_KIND_FALCO_PHANTASM),
        ];
        for (i, kind) in kinds {
            let article = self.data.ptr(items.at(4 * i));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Falco's spawn, `ftFc_Init_OnDeath`: no blaster out, and his first model variant.
    #[doc(alias = "ftFc_Init_OnDeath")]
    pub fn falco_on_death(&mut self, gobj: GObjId) {
        self.fox_on_death(gobj);
    }

    /// `ftFc_Init_LoadSpecialAttrs`: Fox's (`ftFx_Init_LoadSpecialAttrs`).
    #[doc(alias = "ftFc_Init_LoadSpecialAttrs")]
    pub fn falco_load_special_attrs(&mut self, gobj: GObjId) {
        self.fox_load_special_attrs(gobj);
    }

    /// Picking up an item, `ftFc_Init_OnItemPickup` (as Fox's).
    #[doc(alias = "ftFc_Init_OnItemPickup")]
    pub fn falco_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftFc_Init_OnItemInvisible` (as Fox's).
    #[doc(alias = "ftFc_Init_OnItemInvisible")]
    pub fn falco_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftFc_Init_OnItemVisible` (as Fox's).
    #[doc(alias = "ftFc_Init_OnItemVisible")]
    pub fn falco_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftFc_Init_OnItemDrop` (as Fox's).
    #[doc(alias = "ftFc_Init_OnItemDrop")]
    pub fn falco_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// Entering knockback, `ftFc_Init_OnKnockbackEnter` (as Fox's).
    #[doc(alias = "ftFc_Init_OnKnockbackEnter")]
    pub fn falco_on_knockback_enter(&mut self, gobj: GObjId) {
        self.fox_on_knockback_enter(gobj);
    }

    /// Leaving knockback, `ftFc_Init_OnKnockbackExit` (as Fox's).
    #[doc(alias = "ftFc_Init_OnKnockbackExit")]
    pub fn falco_on_knockback_exit(&mut self, gobj: GObjId) {
        self.fox_on_knockback_exit(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.falco_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.falco_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.falco_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.falco_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.falco_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.falco_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
