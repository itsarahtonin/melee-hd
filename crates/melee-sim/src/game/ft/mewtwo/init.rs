//! Mewtwo's character callbacks (`ftmewtwo.c`): creation, spawn and attributes, his items'
//! cleanup when he's hit or KO'd, the full charge's glow, and the item and knockback hooks.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::state::Running;
use super::{ITEMS, PART_TRANS_N, addr, attr, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCo_800BFFD0`'s color animation for a full charge.
pub(crate) const COLANIM_CHARGED: i32 = 92;

impl Running<'_> {
    /// Mewtwo's spawn, `ftMt_Init_OnDeath`: his first model variant, no Disable or ball out,
    /// no charge, and Confusion's rise his again.
    #[doc(alias = "ftMt_Init_OnDeath")]
    pub fn mt_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::DISABLE, None);
        fp.u.set_gobj_at(u::HELD_BALL, None);
        fp.set_u_i32(u::CHARGE, 0);
        fp.set_u_i32(u::CHARGE_EFFECTS, 0);
        fp.set_u_i32(u::CONFUSION_BOOST, 0);
    }

    /// Mewtwo's creation, `ftMt_Init_OnLoad`: his attributes, TransN's `flags_b4`,
    /// `x2221_b2`, and his two items registered.
    #[doc(alias = "ftMt_Init_OnLoad")]
    pub fn mt_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, attr::SIZE);
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts");
        self.game.fighter_parts[parts.0][PART_TRANS_N as usize].x8[0] |= 0x08;
        self.game.fighters[f.0].x2221_b2 = true;
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// His Disable and ball gone when KO'd, `ftMt_Init_OnDeath2`, and his charge with them
    /// (`ftMt_SpecialN_OnTakeDamage`).
    #[doc(alias = "ftMt_Init_OnDeath2")]
    pub fn mt_on_death2(&mut self, gobj: GObjId) {
        self.mt_remove_disable(gobj);
        self.mt_n_lose_ball(gobj);
    }

    /// His Disable and ball gone when hit, `ftMt_Init_OnTakeDamage` (the charge kept if full,
    /// `ftMt_SpecialN_OnDeath`).
    #[doc(alias = "ftMt_Init_OnTakeDamage")]
    pub fn mt_on_take_damage(&mut self, gobj: GObjId) {
        self.mt_remove_disable(gobj);
        self.mt_n_drop_ball(gobj);
    }

    /// The full charge's glow (the color animations' hook), `ftMt_Init_UnkMotionStates4`.
    #[doc(alias = "ftMt_Init_UnkMotionStates4")]
    pub fn mt_charged_glow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cycles = reg(self.mt_attr_f32(f, attr::N_CHARGE_CYCLES));
        let charge = self.game.fighters[f.0].u_i32(u::CHARGE);
        if fp::frsp(f64::from(charge)) == cycles {
            self.start_color_anim(gobj, COLANIM_CHARGED, 0);
        }
    }

    /// `ftMt_Init_LoadSpecialAttrs`: his attributes copied again, Disable's offset scaled by
    /// his height.
    #[doc(alias = "ftMt_Init_LoadSpecialAttrs")]
    pub fn mt_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        let scale = self.game.fighters[f.0].x34_scale.y;
        if scale != 1.0 {
            for off in [attr::LW_OFFSET_X, attr::LW_OFFSET_Y] {
                let v = mem(fp::fmuls(reg(self.mt_attr_f32(f, off)), reg(scale)));
                let a = self.game.fighters[f.0]
                    .dat_attrs
                    .expect("a fighter without attributes");
                self.game.fighter_attrs[a.0][off..off + 4]
                    .copy_from_slice(&v.to_bits().to_be_bytes());
            }
        }
    }

    /// Entering knockback, `ftMt_Init_OnKnockbackEnter`: the hurt textures.
    #[doc(alias = "ftMt_Init_OnKnockbackEnter")]
    pub fn mt_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 2, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftMt_Init_OnKnockbackExit`.
    #[doc(alias = "ftMt_Init_OnKnockbackExit")]
    pub fn mt_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 2, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }

    /// The item hooks, all empty (`ftMt_Init_OnItemPickup`, `OnItemDrop`, `OnItemInvisible`,
    /// `OnItemVisible`).
    // Ports `ftMt_Init_OnItemPickup`, `ftMt_Init_OnItemDrop`, `ftMt_Init_OnItemInvisible` and
    // `ftMt_Init_OnItemVisible`; aliased once a check reaches them.
    pub fn mt_item_hook(&mut self, _gobj: GObjId) {}
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_DEATH2 => g.mt_on_death2(gobj),
        addr::ON_TAKE_DAMAGE => g.mt_on_take_damage(gobj),
        addr::ON_ITEM_INVISIBLE | addr::ON_ITEM_VISIBLE => g.mt_item_hook(gobj),
        addr::UNK_MOTION_STATES_4 => g.mt_charged_glow(gobj),
        addr::ON_KNOCKBACK_ENTER => g.mt_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.mt_on_knockback_exit(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP | addr::ON_ITEM_DROP => {
            r.mt_item_hook(gobj(0));
            Val::None
        }
        _ => return None,
    })
}
