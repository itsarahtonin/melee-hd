//! Ness's character callbacks (`ftness.c`): creation, spawn and attributes, the damage and
//! absorb hooks, the item hooks and the knockback hooks.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{ITEMS, addr, attr, u};
use crate::hsd::gobj::GObjId;

/// `ftNessAttributes`' size, which creation pushes and the attribute changes copy.
pub(crate) const ATTRS_SIZE: usize = 0xDC;

impl Running<'_> {
    /// Ness's creation, `ftNs_Init_OnLoad`: his attributes, and his items registered: PK Fire
    /// and its pillar, PK Flash and its blast, PK Thunder's ball and its four trails, the bat
    /// and the yo-yo.
    #[doc(alias = "ftNs_Init_OnLoad")]
    pub fn ness_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, ATTRS_SIZE);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, kind) in ITEMS {
            let article = self.data.ptr(items.at(4 * i));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Ness's spawn, `ftNs_Init_OnDeath`: his first model variant, and no items out.
    #[doc(alias = "ftNs_Init_OnDeath")]
    pub fn ness_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::PKFLASH, None);
        fp.u.set_gobj_at(u::PKTHUNDER, None);
        fp.u.set_gobj_at(u::BAT, None);
        fp.u.set_gobj_at(u::YOYO, None);
        fp.set_u_i32(u::PKTHUNDER_GFX, 0);
    }

    /// `ftNs_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftNs_Init_LoadSpecialAttrs")]
    pub fn ness_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, ATTRS_SIZE);
    }

    /// The hook his states set for taking damage and dying, `ftNs_Init_OnDamage`: the yo-yo,
    /// PK Flash, PK Thunder and the bat let go of.
    #[doc(alias = "ftNs_Init_OnDamage")]
    pub fn ness_on_damage(&mut self, gobj: GObjId) {
        self.ness_yoyo_item_despawn(gobj);
        self.ness_pkflash_set_null(gobj);
        self.ness_special_hi_take_damage(gobj);
        self.ness_bat_remove(gobj);
    }

    /// Absorbing an item, `ftNs_Init_OnAbsorb`: PSI Magnet's response.
    #[doc(alias = "ftNs_Init_OnAbsorb")]
    pub fn ness_on_absorb(&mut self, gobj: GObjId) {
        self.ness_absorb_decide_action(gobj);
    }

    /// How much of an absorbed item's damage heals him, `ftNs_Init_GetAbsorbHeal`.
    #[doc(alias = "ftNs_Init_GetAbsorbHeal")]
    pub fn ness_absorb_heal(&self, gobj: GObjId) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.ness_attr_f32(f, attr::PSI_MAGNET_HEAL_MUL)
    }

    /// Picking up an item, `ftNs_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    // Ports `ftNs_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn ness_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftNs_Init_OnItemInvisible` (as Fox's).
    // Ports `ftNs_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn ness_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftNs_Init_OnItemVisible` (as Fox's).
    // Ports `ftNs_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn ness_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftNs_Init_OnItemDrop` (as Fox's).
    // Ports `ftNs_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn ness_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// Entering knockback, `ftNs_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftNs_Init_OnKnockbackEnter")]
    pub fn ness_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftNs_Init_OnKnockbackExit`.
    #[doc(alias = "ftNs_Init_OnKnockbackExit")]
    pub fn ness_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_DAMAGE => g.ness_on_damage(gobj),
        addr::ON_ABSORB => g.ness_on_absorb(gobj),
        addr::ON_ITEM_INVISIBLE => g.ness_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.ness_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.ness_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.ness_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.ness_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.ness_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
