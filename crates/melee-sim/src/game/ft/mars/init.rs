//! Marth's character callbacks (`ftmars.c`): the item hooks and the knockback hooks. His
//! creation and spawn (`ftMs_Init_OnLoad`, `ftMs_Init_OnDeath`, `ftMs_Init_LoadSpecialAttrs`)
//! come with fighter creation (`ft::kinds`); the neutral special's accessories are in
//! [`super::special_n`].

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{addr, ext};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Picking up an item, `ftMs_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 0,
    /// 1)`): the hand's hold pose for the item, unless it is heavy.
    #[doc(alias = "ftMs_Init_OnItemPickup")]
    pub fn mars_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        let item = self
            .game
            .fighter(gobj)
            .item_gobj
            .expect("picking up no item");
        if super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            return;
        }
        let kind = self
            .ext(ext::IT_GET_HOLD_KIND, &[Val::GObj(Some(item))])
            .int();
        let pose = match kind {
            1 => Some(1),
            2 => Some(0),
            3 => Some(2),
            4 => Some(3),
            _ => None,
        };
        if let Some(p) = pose {
            self.ext(
                ext::FTANIM_80070FB4,
                &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(p)],
            );
        }
        if flag {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// Hiding the held item, `ftMs_Init_OnItemInvisible` (`Fighter_OnItemInvisible(gobj,
    /// 0)`).
    #[doc(alias = "ftMs_Init_OnItemInvisible")]
    pub fn mars_on_item_invisible(&mut self, gobj: GObjId) {
        let item = self.game.fighter(gobj).item_gobj.expect("hiding no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Showing the held item again, `ftMs_Init_OnItemVisible` (`Fighter_OnItemVisible(gobj,
    /// 0)`).
    #[doc(alias = "ftMs_Init_OnItemVisible")]
    pub fn mars_on_item_visible(&mut self, gobj: GObjId) {
        let item = self.game.fighter(gobj).item_gobj.expect("showing no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Dropping the item, `ftMs_Init_OnItemDrop` (`Fighter_OnItemDrop(gobj, flag, 0, 1)`).
    #[doc(alias = "ftMs_Init_OnItemDrop")]
    pub fn mars_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.ext(
            ext::FTANIM_80070FB4,
            &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(-1)],
        );
        if flag {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// Entering knockback, `ftMs_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftMs_Init_OnKnockbackEnter")]
    pub fn mars_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftMs_Init_OnKnockbackExit`.
    #[doc(alias = "ftMs_Init_OnKnockbackExit")]
    pub fn mars_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }
}

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
