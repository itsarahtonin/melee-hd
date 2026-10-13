//! Mario's character callbacks (`ftmario.c`): creation (his own and Dr. Mario's attributes),
//! spawn, the attribute copy, the hit hook that drops the cape, and the item and knockback
//! hooks.

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{addr, attr, u};
use crate::game::ft::vars::{MARIO_CAPE, MARIO_X2240};
use crate::game::handles::FighterId;
use crate::hsd::gobj::GObjId;

/// `It_Kind_Mario_Fire`.
pub const IT_KIND_MARIO_FIRE: i32 = 48;

impl Running<'_> {
    /// Mario's creation, `ftMr_Init_OnLoad`: he can wall jump, has his attributes, and
    /// registers his fireball and his cape with the items.
    #[doc(alias = "ftMr_Init_OnLoad")]
    pub fn mr_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].can_walljump = true;
        self.push_attrs(f, attr::SIZE);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let cape = self.char_attr_word(f, attr::SPECIALS_CAPE_KIND) as i32;
        let fire = self.data.ptr(items.at(0));
        super::super::super::it::item::set_char_article(self.game, fire, IT_KIND_MARIO_FIRE);
        let cape_article = self.data.ptr(items.at(8));
        super::super::super::it::item::set_char_article(self.game, cape_article, cape);
    }

    /// Dr. Mario's attributes, `ftMr_Init_OnLoadForDrMario` (`PUSH_ATTRS` with Mario's
    /// layout).
    #[doc(alias = "ftMr_Init_OnLoadForDrMario")]
    pub fn mr_on_load_for_drmario(&mut self, f: FighterId) {
        self.push_attrs(f, attr::SIZE);
    }

    /// Mario's spawn, `ftMr_Init_OnDeath`: his first model variant, no pill colors yet (9),
    /// the Tornado's rise and the cape's boost ready, no cape.
    #[doc(alias = "ftMr_Init_OnDeath")]
    pub fn mr_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::VITAMIN_CURR, 9);
        fp.set_u_i32(u::VITAMIN_PREV, 9);
        fp.set_u_i32(u::TORNADO_CHARGE, 0);
        fp.set_u_i32(u::IS_CAPE_BOOST, 0);
        fp.u.set_gobj_at(MARIO_CAPE, None);
        fp.u.set_gobj_at(MARIO_X2240, None);
    }

    /// `ftMr_Init_OnTakeDamage`: a hit takes the cape away.
    #[doc(alias = "ftMr_Init_OnTakeDamage")]
    pub fn mr_on_take_damage(&mut self, gobj: GObjId) {
        self.mr_special_s_remove_cape(gobj);
    }

    /// `ftMr_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftMr_Init_LoadSpecialAttrs")]
    pub fn mr_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// `ftMr_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftMr_Init_OnKnockbackEnter")]
    pub fn mr_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftMr_Init_OnKnockbackExit`: the hurt textures, as Sheik's.
    #[doc(alias = "ftMr_Init_OnKnockbackExit")]
    pub fn mr_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_TAKE_DAMAGE => g.mr_on_take_damage(gobj),
        addr::ON_ITEM_INVISIBLE => g.fox_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.fox_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.mr_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.mr_on_knockback_exit(gobj),
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
