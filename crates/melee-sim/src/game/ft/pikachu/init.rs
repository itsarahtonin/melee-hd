//! Pikachu's character callbacks (`ftpikachu.c`): creation, spawn, the attribute copy, the
//! item hooks and the knockback hooks.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::{addr, attr};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Pichu's attributes, `ftPk_Init_OnLoadForPichu` (`PUSH_ATTRS` with Pikachu's layout).
    #[doc(alias = "ftPk_Init_OnLoadForPichu")]
    pub fn pk_on_load_for_pichu(&mut self, f: FighterId) {
        self.push_attrs(f, attr::SIZE);
    }

    /// Registers the character's articles with the items: the Thunder's bolt (`xDC`) and the
    /// Thunder Jolts on the ground and in the air.
    pub(crate) fn pk_set_articles(&mut self, f: FighterId) {
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        let kinds = [
            self.pk_attr_i32(f, attr::XDC),
            self.pk_attr_i32(f, attr::SPECIALN_ITKIND),
            self.pk_attr_i32(f, attr::SPECIALAIRN_ITKIND),
        ];
        for (i, kind) in kinds.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Pikachu's creation, `ftPk_Init_OnLoad`: his attributes, and his articles.
    #[doc(alias = "ftPk_Init_OnLoad")]
    pub fn pk_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, attr::SIZE);
        self.pk_set_articles(f);
    }

    /// Pikachu's spawn, `ftPk_Init_OnDeath`: his first model variants.
    #[doc(alias = "ftPk_Init_OnDeath")]
    pub fn pk_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
    }

    /// `ftPk_Init_LoadSpecialAttrs`: the attributes copied, and the Quick Attack's collision
    /// box scaled with the fighter.
    #[doc(alias = "ftPk_Init_LoadSpecialAttrs")]
    pub fn pk_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        let scale = self.game.fighters[f.0].x34_scale.y;
        if reg(scale) != 1.0 {
            let a = self.game.fighters[f.0]
                .dat_attrs
                .expect("a fighter without attributes");
            let b = &mut self.game.fighter_attrs[a.0];
            for i in 0..6 {
                let off = attr::HEIGHT_ATTRIBUTES + 4 * i;
                let v = f32::from_bits(u32::from_be_bytes(b[off..off + 4].try_into().unwrap()));
                let v = mem(fp::fmuls(reg(v), reg(scale)));
                b[off..off + 4].copy_from_slice(&v.to_bits().to_be_bytes());
            }
        }
    }

    /// Picking up an item, `ftPk_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 0,
    /// 0)`): the hold pose on hand 0.
    // Ports `ftPk_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn pk_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 0);
    }

    /// Hiding the held item, `ftPk_Init_OnItemInvisible`.
    // Ports `ftPk_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn pk_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 0);
    }

    /// Showing the held item again, `ftPk_Init_OnItemVisible`.
    // Ports `ftPk_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn pk_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 0);
    }

    /// Dropping the item, `ftPk_Init_OnItemDrop`.
    // Ports `ftPk_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn pk_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.pk_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.pk_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.pk_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.pk_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.pk_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.pk_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
