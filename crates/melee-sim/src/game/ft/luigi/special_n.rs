//! Luigi's Fireball (`ftluigispecialn.c`): a fireball thrown from his hand when the script
//! raises its flag, on the ground or in the air.
//!
//! Kirby's copy of it (`ftkirbyspecialluigi.c`) is the same code over his own states, hand,
//! item kind and flash; a [`Fireball`] says which, Luigi's [`LUIGI`] or Kirby's
//! (`ft::kirby::special_luigi::KIRBY`), and each function here carries both originals' names.

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::luigifireball;
use super::super::super::state::Running;
use super::init::IT_KIND_LUIGI_FIRE;
use super::{addr, ms, part};
use crate::hsd::gobj::GObjId;

/// What tells one Fireball from the other: the user's states, the throw's hook, the hand the
/// fireball leaves, its item kind and its flash.
pub struct Fireball {
    pub(crate) ground: i32,
    pub(crate) air: i32,
    /// The throw (`accessory4_cb`): `ftLg_SpecialN_FireSpawn`, Kirby's `fn_800F98F4`.
    pub(crate) throw: u32,
    /// `FtPart_L1stNb`, Kirby's `FtPart_LHandN`.
    pub(crate) hand: i32,
    pub(crate) item_kind: i32,
    /// The flash at the hand.
    pub(crate) flash: i32,
}

/// Luigi's own.
pub const LUIGI: Fireball = Fireball {
    ground: ms::SPECIAL_N,
    air: ms::SPECIAL_AIR_N,
    throw: addr::SPECIAL_N_FIRE_SPAWN,
    hand: part::L_1ST_NB,
    item_kind: IT_KIND_LUIGI_FIRE,
    flash: 0x507,
};

/// The flags the throw keeps going between the ground and the air (`ftLg_MF_SpecialN_Coll`).
const COLL_FLAGS: u32 = 0x5000;

impl Running<'_> {
    fn lg_special_n_start(&mut self, gobj: GObjId, f: &Fireball, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
    }

    /// `ftLg_SpecialN_Enter` (Kirby's `ftKb_SpecialNLg_800F9614`).
    #[doc(alias = "ftLg_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNLg_800F9614")]
    pub fn lg_special_n_enter(&mut self, gobj: GObjId, f: &Fireball) {
        self.lg_special_n_start(gobj, f, f.ground);
    }

    /// `ftLg_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNLg_800F9684`).
    #[doc(alias = "ftLg_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNLg_800F9684")]
    pub fn lg_special_air_n_enter(&mut self, gobj: GObjId, f: &Fireball) {
        self.lg_special_n_start(gobj, f, f.air);
    }

    /// `ftLg_SpecialN_Anim`: waiting when the throw ends.
    #[doc(alias = "ftLg_SpecialN_Anim")]
    #[doc(alias = "ftKb_LgSpecialN_Anim")]
    pub fn lg_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftLg_SpecialAirN_Anim`: a fall when the throw ends.
    #[doc(alias = "ftLg_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_LgSpecialAirN_Anim")]
    pub fn lg_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftLg_SpecialN_IASA`: once the script allows (`cmd_vars[0]`), what waiting allows.
    #[doc(alias = "ftLg_SpecialN_IASA")]
    #[doc(alias = "ftKb_LgSpecialN_IASA")]
    pub fn lg_special_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftLg_SpecialAirN_IASA`: once the script allows, what falling allows.
    #[doc(alias = "ftLg_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_LgSpecialAirN_IASA")]
    pub fn lg_special_air_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftLg_SpecialN_Coll`: off a ledge, the aerial throw from the same frame.
    #[doc(alias = "ftLg_SpecialN_Coll")]
    #[doc(alias = "ftKb_LgSpecialN_Coll")]
    pub fn lg_special_n_coll(&mut self, gobj: GObjId, f: &Fireball) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, f.air, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
        }
    }

    /// `ftLg_SpecialAirN_Coll`: landing, the grounded throw from the same frame.
    #[doc(alias = "ftLg_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_LgSpecialAirN_Coll")]
    pub fn lg_special_air_n_coll(&mut self, gobj: GObjId, f: &Fireball) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, f.ground, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
        }
    }

    /// `ftLg_SpecialN_FireSpawn` (Kirby's `fn_800F98F4`): when the script raises the throw
    /// flag, the fireball from the user's hand and its flash.
    #[doc(alias = "ftLg_SpecialN_FireSpawn")]
    #[doc(alias = "fn_800F98F4")]
    pub fn lg_special_n_fire_spawn(&mut self, gobj: GObjId, f: &Fireball) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, f.hand);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        luigifireball::spawn(self, gobj, pos, f.item_kind, facing);
        let hand = self.part_joint(gobj, f.hand);
        let facing = self.game.fighter(gobj).facing_dir;
        spawn(
            self,
            f.flash,
            Some(gobj),
            &[Arg::JObj(hand), Arg::Float(facing)],
        );
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let f = &LUIGI;
    match a {
        addr::SPECIAL_N_ENTER => g.lg_special_n_enter(gobj, f),
        addr::SPECIAL_AIR_N_ENTER => g.lg_special_air_n_enter(gobj, f),
        addr::SPECIAL_N_ANIM => g.lg_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.lg_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.lg_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.lg_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.lg_ground_friction(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.lg_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.lg_special_n_coll(gobj, f),
        addr::SPECIAL_AIR_N_COLL => g.lg_special_air_n_coll(gobj, f),
        addr::SPECIAL_N_FIRE_SPAWN => g.lg_special_n_fire_spawn(gobj, f),
        _ => return false,
    }
    true
}
