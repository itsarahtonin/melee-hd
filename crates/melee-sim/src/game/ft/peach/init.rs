//! Peach's character callbacks (`ftpeach.c`): creation, spawn and attributes, her items'
//! cleanup when she's hit or KO'd, the item hooks, the knockback hooks and her side taunt's
//! model swap.

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::{ITEMS, addr, attr, u};
use crate::hsd::gobj::GObjId;

/// The float falls' animations (`ftPe_SM_FloatFallF` and `ftPe_SM_FloatFallB`, the
/// character's animations 18 and 19).
const FLOAT_FALL_ANIMS: [(i32, usize); 2] = [
    (18, attr::FLOAT_FALL_F_ANIM_START),
    (19, attr::FLOAT_FALL_B_ANIM_START),
];

impl Running<'_> {
    /// Peach's creation, `ftPe_Init_OnLoad`: the float falls' animation lengths, which the
    /// original writes into her loaded attributes (docs/engine.md), her attributes, and her
    /// five items registered.
    #[doc(alias = "ftPe_Init_OnLoad")]
    pub fn peach_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `lbAnim_8001E8F8(ftData_80085E50(fp, n))`: each animation goes through the second
        // buffer, which keeps the last.
        for (anim, _) in FLOAT_FALL_ANIMS {
            let tree = self.second_figatree(f, anim);
            let _ = super::super::data::figatree_frames(self.data, tree);
        }
        self.push_attrs(f, attr::SIZE);
        self.peach_patch_float_fall_starts(f);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// The float falls' animation lengths her creation writes over the first two words of
    /// her loaded attributes, put in the fighter's: the copies of those attributes (creation's
    /// and every later one) read them from there. The lengths follow from her animations, so
    /// they're worked out here rather than kept.
    fn peach_patch_float_fall_starts(&mut self, f: FighterId) {
        for (anim, off) in FLOAT_FALL_ANIMS {
            let frames = self.peach_anim_frames(f, anim);
            let a = self.game.fighters[f.0]
                .dat_attrs
                .expect("a fighter without attributes");
            self.game.fighter_attrs[a.0][off..off + 4]
                .copy_from_slice(&frames.to_bits().to_be_bytes());
        }
    }

    /// The frame count of the character's animation `anim`, as `ftData_80085E50` and
    /// `lbAnim_8001E8F8` find it, without loading it.
    fn peach_anim_frames(&self, f: FighterId, anim: i32) -> f32 {
        let fp = &self.game.fighters[f.0];
        if anim as u32 >= fp.x58C {
            return 0.0;
        }
        let entry = self.anim_entry(f, anim);
        if self.anim_aram(fp, entry) == 0 {
            return 0.0;
        }
        let tree = self.anim_tree(fp.kind, entry);
        super::super::data::figatree_frames(self.data, Some(tree))
    }

    /// Peach's spawn, `ftPe_Init_OnDeath`: her float back, no forward smash yet, no items, and
    /// her model's variants for her costume.
    #[doc(alias = "ftPe_Init_OnDeath")]
    pub fn peach_on_death(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::HAS_FLOAT, 1);
        fp.set_u_i32(u::ATTACK_S4_MSID, super::MS_NONE);
        fp.set_u_i32(u::SPECIAL_AIR_N_USED, 0);
        fp.u.set_gobj_at(u::PARASOL_1, None);
        fp.u.set_gobj_at(u::PARASOL_0, None);
        fp.u.set_gobj_at(u::TOAD, None);
        fp.u.set_gobj_at(u::VEG, None);
        let costume = fp.costume_id;
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 2, 0);
        self.game.set_part_vis(gobj, 3, -1);
        self.game.set_part_vis(gobj, 4, 0);
        if costume == 1 {
            self.game.set_part_vis(gobj, 1, -1);
            self.game.set_part_vis(gobj, 5, 0);
            self.game.set_part_vis(gobj, 6, -1);
        } else {
            self.game.set_part_vis(gobj, 1, 0);
            self.game.set_part_vis(gobj, 5, -1);
            self.game.set_part_vis(gobj, 6, 0);
        }
    }

    /// Her items' cleanup when she's hit or KO'd, `ftPe_Init_OnDeath2`: the parasol put away,
    /// Toad gone, the vegetable she's pulling dropped.
    #[doc(alias = "ftPe_Init_OnDeath2")]
    pub fn peach_on_death2(&mut self, gobj: GObjId) {
        self.peach_parasol_put_away(gobj);
        self.peach_toad_on_death2(gobj);
        self.peach_drop_veg(gobj);
    }

    /// `ftPe_Init_LoadSpecialAttrs`: her attributes copied again, the float falls' lengths
    /// with them.
    #[doc(alias = "ftPe_Init_LoadSpecialAttrs")]
    pub fn peach_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        self.peach_patch_float_fall_starts(f);
    }

    /// Picking up an item, `ftPe_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    #[doc(alias = "ftPe_Init_OnItemPickup")]
    pub fn peach_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_pickup(gobj, flag);
    }

    /// Hiding the held item, `ftPe_Init_OnItemInvisible` (as Fox's).
    #[doc(alias = "ftPe_Init_OnItemInvisible")]
    pub fn peach_on_item_invisible(&mut self, gobj: GObjId) {
        self.fox_on_item_invisible(gobj);
    }

    /// Showing the held item again, `ftPe_Init_OnItemVisible` (as Fox's).
    #[doc(alias = "ftPe_Init_OnItemVisible")]
    pub fn peach_on_item_visible(&mut self, gobj: GObjId) {
        self.fox_on_item_visible(gobj);
    }

    /// Dropping the item, `ftPe_Init_OnItemDrop` (as Fox's).
    #[doc(alias = "ftPe_Init_OnItemDrop")]
    pub fn peach_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.fox_on_item_drop(gobj, flag);
    }

    /// Her side taunt's swap, `ftPe_Init_8011B93C`: the third model's variant on if it was
    /// off, else off.
    #[doc(alias = "ftPe_Init_8011B93C")]
    pub fn peach_appeal_swap(&mut self, gobj: GObjId) {
        let v = if self.game.part_vis(gobj, 2) == -1 {
            0
        } else {
            -1
        };
        self.game.set_part_vis(gobj, 2, v);
    }

    /// Entering knockback, `ftPe_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftPe_Init_OnKnockbackEnter")]
    pub fn peach_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftPe_Init_OnKnockbackExit`.
    #[doc(alias = "ftPe_Init_OnKnockbackExit")]
    pub fn peach_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_DEATH2 => g.peach_on_death2(gobj),
        addr::ON_ITEM_INVISIBLE => g.peach_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.peach_on_item_visible(gobj),
        addr::APPEAL_SWAP => g.peach_appeal_swap(gobj),
        addr::ON_KNOCKBACK_ENTER => g.peach_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.peach_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.peach_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.peach_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
