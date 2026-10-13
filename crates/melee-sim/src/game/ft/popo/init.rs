//! Popo's and Nana's character callbacks (`ftpopo.c`, `ftnana.c`): their creation, spawn and
//! attribute copy, and the item and knockback hooks.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{IT_KIND_BLIZZARD, IT_KIND_GUM_STRINGS, IT_KIND_ICE, addr, attr, var};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Popo's creation, `ftPp_Init_OnLoad`: `x2222_b5`, his attributes, `x40` from them, and
    /// his ice, Blizzard and strings registered with the items.
    #[doc(alias = "ftPp_Init_OnLoad")]
    pub fn popo_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].x2222_b5 = true;
        self.push_attrs(f, attr::SIZE);
        self.game.fighters[f.0].x40 = self.ic_attr_f32(f, attr::POPO_X40);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, kind) in [IT_KIND_ICE, IT_KIND_BLIZZARD, IT_KIND_GUM_STRINGS]
            .into_iter()
            .enumerate()
        {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Nana's creation, `ftNn_Init_OnLoad`: `x2222_b4`, Popo's attribute layout
    /// (`ftPp_Init_OnLoadForNana`) and `x40` from it.
    #[doc(alias = "ftNn_Init_OnLoad")]
    pub fn nana_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].x2222_b4 = true;
        self.popo_on_load_for_nana(f);
        self.game.fighters[f.0].x40 = self.ic_attr_f32(f, attr::NANA_X40);
    }

    /// Nana's attributes, `ftPp_Init_OnLoadForNana` (`PUSH_ATTRS` with Popo's layout).
    #[doc(alias = "ftPp_Init_OnLoadForNana")]
    pub fn popo_on_load_for_nana(&mut self, f: super::super::super::handles::FighterId) {
        self.push_attrs(f, attr::SIZE);
    }

    /// Popo's spawn, `ftPp_Init_OnDeath`: his first model variants, and his variables cleared.
    #[doc(alias = "ftPp_Init_OnDeath")]
    pub fn popo_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(var::X2234, 0);
        fp.u.set_gobj_at(var::ITEM, None);
        fp.u[var::X2230] &= 0x7F;
        fp.u.set_gobj_at(var::ITEM2, None);
        fp.set_u_i32(var::X224C, 0);
        fp.set_u_i32(var::X2250, 0);
    }

    /// Nana's spawn, `ftNn_Init_OnDeath`: her armor from her attributes, then as Popo's.
    #[doc(alias = "ftNn_Init_OnDeath")]
    pub fn nana_on_death(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].dmg.armor0 = self.ic_attr_f32(f, attr::NANA_ARMOR);
        self.popo_on_death(gobj);
    }

    /// `ftPp_Init_LoadSpecialAttrs` and `ftNn_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftPp_Init_LoadSpecialAttrs")]
    #[doc(alias = "ftNn_Init_LoadSpecialAttrs")]
    pub fn popo_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// Entering knockback, `ftPp_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    // Ports `ftPp_Init_OnKnockbackEnter`; aliased once a check reaches it.
    pub fn popo_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftPp_Init_OnKnockbackExit`.
    // Ports `ftPp_Init_OnKnockbackExit`; aliased once a check reaches it.
    pub fn popo_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }

    /// Picking up an item, `ftPp_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    // Ports `ftPp_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn popo_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftPp_Init_OnItemInvisible` (as Fox's).
    // Ports `ftPp_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn popo_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftPp_Init_OnItemVisible` (as Fox's).
    // Ports `ftPp_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn popo_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftPp_Init_OnItemDrop` (as Fox's).
    // Ports `ftPp_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn popo_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::PP_ON_ITEM_INVISIBLE => g.popo_on_item_invisible(gobj),
        addr::PP_ON_ITEM_VISIBLE => g.popo_on_item_visible(gobj),
        addr::PP_ON_KNOCKBACK_ENTER => g.popo_on_knockback_enter(gobj),
        addr::PP_ON_KNOCKBACK_EXIT => g.popo_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::PP_ON_ITEM_PICKUP => {
            r.popo_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::PP_ON_ITEM_DROP => {
            r.popo_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
