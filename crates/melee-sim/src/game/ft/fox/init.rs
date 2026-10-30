//! Fox's and Falco's character callbacks (`ftfox.c`): the blaster's damage hook, the item
//! hooks, Falco's attribute copy and the knockback hooks. Fox's creation and spawn
//! (`ftFx_Init_OnLoad`, `ftFx_Init_OnDeath`, `ftFx_Init_LoadSpecialAttrs`) come with fighter
//! creation (`ft::kinds` on its branch).

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{BLASTER, addr, attr, ext};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Whether Fox has his blaster out, `ftFx_Init_800E5534`.
    #[doc(alias = "ftFx_Init_800E5534")]
    pub fn fox_has_blaster(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).u.gobj_at(BLASTER).is_some()
    }

    /// The blaster's damage and death hook, `ftFx_Init_800E5588`: the blaster goes.
    #[doc(alias = "ftFx_Init_800E5588")]
    pub fn fox_blaster_hit(&mut self, gobj: GObjId) {
        self.fox_remove_blaster(gobj);
    }

    /// Picking up an item, `ftFx_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`): the right hand takes the item's hold pose, unless it is heavy.
    #[doc(alias = "ftFx_Init_OnItemPickup")]
    pub fn fox_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 1);
    }

    /// `Fighter_OnItemPickup(gobj, flag, hand, hand)`: hand `hand` takes the item's hold
    /// pose, unless it is heavy, and closes on it when `flag` says so.
    pub(crate) fn item_pickup_hand(&mut self, gobj: GObjId, flag: bool, hand: i32) {
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
                &[Val::GObj(Some(gobj)), Val::Int(hand), Val::Int(p)],
            );
        }
        if flag {
            self.ext(
                ext::FTANIM_80070C48,
                &[Val::GObj(Some(gobj)), Val::Int(hand)],
            );
        }
    }

    /// Hiding the held item, `ftFx_Init_OnItemInvisible`: the hand lets go of its pose,
    /// unless the item is heavy.
    #[doc(alias = "ftFx_Init_OnItemInvisible")]
    pub fn fox_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 1);
    }

    /// `Fighter_OnItemInvisible(gobj, hand)`.
    pub(crate) fn item_invisible_hand(&mut self, gobj: GObjId, hand: i32) {
        let item = self.game.fighter(gobj).item_gobj.expect("hiding no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(
                ext::FTANIM_80070CC4,
                &[Val::GObj(Some(gobj)), Val::Int(hand)],
            );
        }
    }

    /// Showing the held item again, `ftFx_Init_OnItemVisible`.
    #[doc(alias = "ftFx_Init_OnItemVisible")]
    pub fn fox_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 1);
    }

    /// `Fighter_OnItemVisible(gobj, hand)`.
    pub(crate) fn item_visible_hand(&mut self, gobj: GObjId, hand: i32) {
        let item = self.game.fighter(gobj).item_gobj.expect("showing no item");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(
                ext::FTANIM_80070C48,
                &[Val::GObj(Some(gobj)), Val::Int(hand)],
            );
        }
    }

    /// Dropping the item, `ftFx_Init_OnItemDrop`: no hold pose, and the hand lets go of it
    /// when `flag` says so.
    #[doc(alias = "ftFx_Init_OnItemDrop")]
    pub fn fox_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 1);
    }

    /// `Fighter_OnItemDrop(gobj, flag, hand, hand)`.
    pub(crate) fn item_drop_hand(&mut self, gobj: GObjId, flag: bool, hand: i32) {
        self.ext(
            ext::FTANIM_80070FB4,
            &[Val::GObj(Some(gobj)), Val::Int(hand), Val::Int(-1)],
        );
        if flag {
            self.ext(
                ext::FTANIM_80070CC4,
                &[Val::GObj(Some(gobj)), Val::Int(hand)],
            );
        }
    }

    /// Falco's attributes, `ftFx_Init_OnLoadForFalco` (`PUSH_ATTRS` with Fox's layout): the
    /// character's copied into the fighter's backup block, which becomes its attributes.
    #[doc(alias = "ftFx_Init_OnLoadForFalco")]
    pub fn fox_on_load_for_falco(&mut self, f: super::super::super::handles::FighterId) {
        // As `PUSH_ATTRS` everywhere: the block's bytes past the attributes are the heap's,
        // never read (`push_attrs`).
        self.push_attrs(f, attr::SIZE);
    }

    /// Entering knockback, `ftFx_Init_OnKnockbackEnter`: the eyes' and mouth's hurt textures
    /// and model parts.
    #[doc(alias = "ftFx_Init_OnKnockbackEnter")]
    pub fn fox_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
        self.apply_part_anim(gobj, 3, 3, 0.0);
        self.apply_part_anim(gobj, 4, 3, 0.0);
    }

    /// Leaving knockback, `ftFx_Init_OnKnockbackExit`.
    #[doc(alias = "ftFx_Init_OnKnockbackExit")]
    pub fn fox_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
        self.apply_part_anim(gobj, 3, 2, 0.0);
        self.apply_part_anim(gobj, 4, 2, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::REMOVE_BLASTER_CB => g.fox_blaster_hit(gobj),
        addr::ON_ITEM_INVISIBLE => g.fox_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.fox_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.fox_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.fox_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::HAS_BLASTER => Val::Int(i32::from(r.fox_has_blaster(gobj(0)))),
        addr::ON_ITEM_PICKUP => {
            r.fox_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.fox_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_LOAD_FOR_FALCO => {
            let Val::Fighter(fid) = args[0] else {
                panic!(
                    "ftFx_Init_OnLoadForFalco takes a fighter, not {:?}",
                    args[0]
                );
            };
            r.fox_on_load_for_falco(fid);
            Val::None
        }
        _ => return None,
    })
}
