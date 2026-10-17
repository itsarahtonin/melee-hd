//! Young Link's own code (`ft/kinds/ftCLink`). His moves are Link's (`ft::link`, whose code and
//! items branch on the kind where the two differ: the hookshot's sounds, the fire arrow), and
//! so are most of his callbacks, which carry his aliases there. What is his: his creation
//! (`ft::link::init`), his shield's absorb with its own sound (`ftclink.c`), and his side taunt,
//! a drink of milk (`ftclinkappeals.c`, the milk an item, `it::kinds::clinkmilk`).
//!
//! The milk lives in Link's `x18` (`ftLk_FighterVars`).

use super::super::code::Val;
use super::super::handles::CodeRef;
use super::super::it::kinds::clinkmilk;
use super::super::state::Running;
use super::link::u;
use crate::hsd::gobj::GObjId;

pub use super::link::KIND_CLINK;

/// Young Link's functions other code reaches by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_8C64;
    pub const ON_LOAD: u32 = 0x8014_8CE4;
    pub const ON_ITEM_PICKUP_EXT: u32 = 0x8014_8DC0;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_8E34;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_8E7C;
    pub const ON_ITEM_DROP_EXT: u32 = 0x8014_8EC4;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_8F38;
    pub const ON_ITEM_DROP: u32 = 0x8014_9018;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_906C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_908C;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_90D0;
    pub const ABSORB_CB: u32 = 0x8014_9114;
    pub const SHIELD_ABSORB: u32 = 0x8014_919C;
    pub const MILK_DONE: u32 = 0x8014_920C;
    pub const REMOVE_MILK: u32 = 0x8014_9268;
    pub const FORGET_MILK: u32 = 0x8014_92C4;
    pub const APPEAL_CMD_VAR1: u32 = 0x8014_92F4;
    pub const APPEAL_ENTER: u32 = 0x8014_9318;
    pub const APPEAL_S_ANIM: u32 = 0x8014_9354;
    pub const APPEAL_S_IASA: u32 = 0x8014_943C;
    pub const APPEAL_S_PHYS: u32 = 0x8014_945C;
    pub const APPEAL_S_COLL: u32 = 0x8014_947C;
}

/// His side taunt's states (`ftLk_MS_AppealSR`, `ftLk_MS_AppealSL`).
pub mod ms {
    pub const APPEAL_SR: i32 = 342;
    pub const APPEAL_SL: i32 = 343;
}

/// `ft_PlaySFX`'s sound for his shield's absorb push.
const SFX_ABSORB: i32 = 0x111DA;

impl Running<'_> {
    /// His shield's absorb push, `ftCl_Init_80149114`: Link's (`ftLk_800EB334`) with his own
    /// sound.
    #[doc(alias = "ftCl_Init_80149114")]
    pub fn clink_absorb_push(&mut self, gobj: GObjId) {
        self.absorb_push_with(gobj, SFX_ABSORB);
    }

    /// His shield's absorb box while he waits or crouches, `ftCl_Init_8014919C`: Link's
    /// (`ftLk_AttackAir_800EB3BC`), whose hits push him back with his sound
    /// ([`addr::ABSORB_CB`]).
    #[doc(alias = "ftCl_Init_8014919C")]
    pub fn clink_shield_absorb(&mut self, gobj: GObjId) {
        self.shield_absorb_with(gobj, addr::ABSORB_CB);
    }

    /// Whether the taunt is over for the milk, `ftCl_Init_8014920C`: out of the side taunt, or
    /// without the milk.
    #[doc(alias = "ftCl_Init_8014920C")]
    pub fn clink_milk_done(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.motion_id != ms::APPEAL_SR && fp.motion_id != ms::APPEAL_SL {
            return true;
        }
        fp.u.gobj_at(u::X18).is_none()
    }

    /// `ftCl_Init_80149268` (`checkFighter2244`): the milk taken away (`it_802C8C34`), if
    /// out. Link's cleanup calls it for Link too, whose `x18` never holds one.
    #[doc(alias = "ftCl_Init_80149268")]
    pub fn clink_remove_milk(&mut self, gobj: GObjId) {
        if let Some(milk) = self.game.fighter(gobj).u.gobj_at(u::X18) {
            clinkmilk::remove(self, milk);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::X18, None);
        }
    }

    /// `ftCl_Init_801492C4`: he forgets the milk.
    #[doc(alias = "ftCl_Init_801492C4")]
    pub fn clink_forget_milk(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::X18).is_some() {
            fp.u.set_gobj_at(u::X18, None);
        }
    }

    /// `ftCl_Init_801492F4`: the taunt script's second variable (whether the milk shows).
    #[doc(alias = "ftCl_Init_801492F4")]
    pub fn clink_appeal_cmd_var1(&self, gobj: GObjId) -> u32 {
        self.game.fighter(gobj).cmd_vars[1]
    }

    /// His taunt, `ftCl_Init_80149318`: the side taunt (`ftCo_800DEAE8`), its milk to come.
    #[doc(alias = "ftCl_Init_80149318")]
    pub fn clink_appeal_enter(&mut self, gobj: GObjId) {
        self.enter_facing_state(gobj, ms::APPEAL_SR, ms::APPEAL_SL);
        self.game.fighter_mut(gobj).cmd_vars[1] = 0;
    }

    /// `ftCl_AppealS_Anim`: on the script's first word the milk in his left hand (with his
    /// cleanup as the hit hooks), on its second it goes; when the taunt ends, it goes and he
    /// waits.
    #[doc(alias = "ftCl_AppealS_Anim")]
    pub fn clink_appeal_s_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[1] == 1 && fp.u.gobj_at(u::X18).is_none() {
            let pos = fp.cur_pos;
            let facing = fp.facing_dir;
            let bone = self.link_left_thumb_bone(gobj);
            let milk = clinkmilk::spawn(self, gobj, pos, bone, facing);
            let fp = self.game.fighter_mut(gobj);
            fp.u.set_gobj_at(u::X18, milk);
            if milk.is_some() {
                let cb = Some(CodeRef(super::link::addr::DAMAGE_CB));
                fp.death2_cb = cb;
                fp.take_dmg_cb = cb;
            }
        } else if fp.cmd_vars[1] == 2 {
            self.clink_remove_milk(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.clink_remove_milk(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftCl_AppealS_IASA`: the side taunt's.
    #[doc(alias = "ftCl_AppealS_IASA")]
    pub fn clink_appeal_s_iasa(&mut self, gobj: GObjId) {
        self.co_appeal_s_iasa(gobj);
    }

    /// `ftCl_AppealS_Phys`: friction.
    #[doc(alias = "ftCl_AppealS_Phys")]
    pub fn clink_appeal_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCl_AppealS_Coll`: the ground's, stopping at ledges.
    #[doc(alias = "ftCl_AppealS_Coll")]
    pub fn clink_appeal_s_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter(gobj);
    }
}

/// Runs one of Young Link's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is
/// one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.link_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.link_on_item_visible(gobj),
        addr::LOAD_SPECIAL_ATTRS => g.link_load_special_attrs(gobj),
        addr::ON_KNOCKBACK_ENTER => g.link_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.link_on_knockback_exit(gobj),
        addr::ABSORB_CB => g.clink_absorb_push(gobj),
        addr::SHIELD_ABSORB => g.clink_shield_absorb(gobj),
        addr::REMOVE_MILK => g.clink_remove_milk(gobj),
        addr::FORGET_MILK => g.clink_forget_milk(gobj),
        addr::APPEAL_ENTER => g.clink_appeal_enter(gobj),
        addr::APPEAL_S_ANIM => g.clink_appeal_s_anim(gobj),
        addr::APPEAL_S_IASA => g.clink_appeal_s_iasa(gobj),
        addr::APPEAL_S_PHYS => g.clink_appeal_s_phys(gobj),
        addr::APPEAL_S_COLL => g.clink_appeal_s_coll(gobj),
        _ => return false,
    }
    true
}

/// Young Link's functions other code calls by address (`code::call`): the item hooks and every
/// callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    match f {
        addr::ON_ITEM_PICKUP_EXT => {
            r.link_on_item_pickup(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_DROP_EXT => {
            r.link_on_item_drop(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_PICKUP => {
            r.link_item_pickup(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::ON_ITEM_DROP => {
            r.link_item_drop(gobj(0), args[1].int() != 0);
            Some(Val::None)
        }
        addr::MILK_DONE => Some(Val::Int(i32::from(r.clink_milk_done(gobj(0))))),
        addr::APPEAL_CMD_VAR1 => Some(Val::Int(r.clink_appeal_cmd_var1(gobj(0)) as i32)),
        _ => match args.first() {
            Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
            _ => None,
        },
    }
}
