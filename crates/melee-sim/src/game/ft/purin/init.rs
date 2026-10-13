//! Jigglypuff's character callbacks (`ftpurin.c`): creation, spawn and attributes, the item
//! hooks, the knockback hooks and Sing's effect.

use super::super::super::code::Val;
use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::super::fox::ext;
use super::{PART_WAIST_N, addr, attr};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Jigglypuff's creation, `ftPr_Init_OnLoad`: her attributes, which are also her
    /// multi-jump attributes (`x2D0`), and her costume's hat (`ftPr_Init_8013C360`).
    #[doc(alias = "ftPr_Init_OnLoad")]
    pub fn purin_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.push_attrs(f, attr::SIZE);
        let fp = &mut self.game.fighters[f.0];
        fp.can_multijump = true;
        fp.x2D0 = fp.dat_attrs;
        self.purin_load_hat(gobj);
    }

    /// Jigglypuff's spawn, `ftPr_Init_OnDeath`: her first model variant.
    #[doc(alias = "ftPr_Init_OnDeath")]
    pub fn purin_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
    }

    /// `ftPr_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftPr_Init_LoadSpecialAttrs")]
    pub fn purin_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// Picking up an item, `ftPr_Init_OnItemPickup`: the left hand takes the hold pose of the
    /// one-handed and the two-handed swing items, unless the item is heavy.
    // Ports `ftPr_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn purin_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
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
            3 => Some(0),
            4 => Some(1),
            _ => None,
        };
        if let Some(p) = pose {
            self.ext(
                ext::FTANIM_80070FB4,
                &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(p)],
            );
        }
        if flag {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Hiding the held item, `ftPr_Init_OnItemInvisible`: the hand lets go of its pose,
    /// unless the item is heavy.
    #[doc(alias = "ftPr_Init_OnItemInvisible")]
    pub fn purin_on_item_invisible(&mut self, gobj: GObjId) {
        let Some(item) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Showing the held item again, `ftPr_Init_OnItemVisible`.
    #[doc(alias = "ftPr_Init_OnItemVisible")]
    pub fn purin_on_item_visible(&mut self, gobj: GObjId) {
        let Some(item) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Dropping the item, `ftPr_Init_OnItemDrop`: no hold pose, and the hand lets go of it
    /// when `flag` says so.
    // Ports `ftPr_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn purin_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.ext(
            ext::FTANIM_80070FB4,
            &[Val::GObj(Some(gobj)), Val::Int(0), Val::Int(-1)],
        );
        if flag {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(0)]);
        }
    }

    /// Entering knockback, `ftPr_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftPr_Init_OnKnockbackEnter")]
    pub fn purin_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftPr_Init_OnKnockbackExit` (`Fighter_OnKnockbackExit(gobj, 1)`).
    #[doc(alias = "ftPr_Init_OnKnockbackExit")]
    pub fn purin_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }

    /// Sing's notes, `ftPr_Init_8013C94C` (accessory 4): effect 1238 on the waist once
    /// (`x2219_b0`), the hitlag hooks, and no more accessory.
    #[doc(alias = "ftPr_Init_8013C94C")]
    pub fn purin_sing_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_WAIST_N);
            spawn(self, 1238, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.purin_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.purin_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.purin_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.purin_on_knockback_exit(gobj),
        addr::SING_GFX => g.purin_sing_gfx(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.purin_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.purin_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
