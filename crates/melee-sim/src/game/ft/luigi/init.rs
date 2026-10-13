//! Luigi's character callbacks (`ftluigi.c`): creation, spawn, the attribute copy, the item
//! hooks and the knockback hooks.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{addr, attr, u};
use crate::hsd::gobj::GObjId;

/// `It_Kind_Luigi_Fire`.
pub const IT_KIND_LUIGI_FIRE: i32 = 105;

impl Running<'_> {
    /// Luigi's creation, `ftLg_Init_OnLoad`: his attributes, and his fireball registered with
    /// the items.
    #[doc(alias = "ftLg_Init_OnLoad")]
    pub fn lg_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, attr::SIZE);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let article = self.data.ptr(items.at(0));
        super::super::super::it::item::set_char_article(self.game, article, IT_KIND_LUIGI_FIRE);
    }

    /// Luigi's spawn, `ftLg_Init_OnDeath`: his first model variant, `x2234` cleared.
    #[doc(alias = "ftLg_Init_OnDeath")]
    pub fn lg_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.fighter_mut(gobj).set_u_i32(u::X2234, 0);
    }

    /// `ftLg_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftLg_Init_LoadSpecialAttrs")]
    pub fn lg_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.fox_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.fox_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.lg_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.lg_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.fox_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.fox_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
