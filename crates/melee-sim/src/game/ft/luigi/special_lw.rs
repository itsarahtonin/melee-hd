//! Luigi's Cyclone (`ftluigispeciallw.c`): a spin the stick steers, which tapping B lifts,
//! once per time airborne in the air; it tilts with a sloped floor. Built as Mario's Tornado
//! ([`plumber`](super::super::mario::plumber)).

use super::super::super::state::Running;
use super::super::mario::plumber::Tornado;
use super::{addr, attr, ms, u};
use crate::hsd::gobj::GObjId;

const CYCLONE: Tornado = Tornado {
    base: attr::CYCLONE_TAP_MOMENTUM,
    ms_lw: ms::SPECIAL_LW,
    ms_air_lw: ms::SPECIAL_AIR_LW,
    charged: u::CYCLONE_CHARGE,
    effect: 0x509,
    on_damage: addr::SPECIAL_LW_UPDATE_ROT,
};

impl Running<'_> {
    /// `ftLg_SpecialLw_UpdateRot` (the spin's damage hook): the model upright again.
    #[doc(alias = "ftLg_SpecialLw_UpdateRot")]
    pub fn lg_special_lw_update_rot(&mut self, gobj: GObjId) {
        self.plumber_lw_update_rot(gobj);
    }

    /// `ftLg_SpecialLw_Enter`: from the ground the spin starts in its aerial state too.
    #[doc(alias = "ftLg_SpecialLw_Enter")]
    pub fn lg_special_lw_enter(&mut self, gobj: GObjId) {
        self.plumber_lw_enter(gobj, &CYCLONE, true);
    }

    /// `ftLg_SpecialAirLw_Enter`.
    #[doc(alias = "ftLg_SpecialAirLw_Enter")]
    pub fn lg_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.plumber_lw_enter(gobj, &CYCLONE, false);
    }

    /// `ftLg_SpecialLw_Anim`.
    #[doc(alias = "ftLg_SpecialLw_Anim")]
    pub fn lg_special_lw_anim(&mut self, gobj: GObjId) {
        self.plumber_lw_anim(gobj);
    }

    /// `ftLg_SpecialAirLw_Anim`.
    #[doc(alias = "ftLg_SpecialAirLw_Anim")]
    pub fn lg_special_air_lw_anim(&mut self, gobj: GObjId) {
        self.plumber_air_lw_anim(gobj, &CYCLONE);
    }

    /// `ftLg_SpecialLw_Phys`.
    #[doc(alias = "ftLg_SpecialLw_Phys")]
    pub fn lg_special_lw_phys(&mut self, gobj: GObjId) {
        self.plumber_lw_phys(gobj, &CYCLONE);
    }

    /// `ftLg_SpecialAirLw_Phys`.
    #[doc(alias = "ftLg_SpecialAirLw_Phys")]
    pub fn lg_special_air_lw_phys(&mut self, gobj: GObjId) {
        self.plumber_air_lw_phys(gobj, &CYCLONE);
    }

    /// `ftLg_SpecialLw_Coll`.
    #[doc(alias = "ftLg_SpecialLw_Coll")]
    pub fn lg_special_lw_coll(&mut self, gobj: GObjId) {
        self.plumber_lw_coll(gobj, &CYCLONE);
    }

    /// `ftLg_SpecialAirLw_Coll`.
    #[doc(alias = "ftLg_SpecialAirLw_Coll")]
    pub fn lg_special_air_lw_coll(&mut self, gobj: GObjId) {
        self.plumber_air_lw_coll(gobj, &CYCLONE);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_UPDATE_ROT => g.lg_special_lw_update_rot(gobj),
        addr::SPECIAL_LW_ENTER => g.lg_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.lg_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.lg_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.lg_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA | addr::SPECIAL_AIR_LW_IASA => g.lg_nothing(gobj),
        addr::SPECIAL_LW_PHYS => g.lg_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.lg_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.lg_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.lg_special_air_lw_coll(gobj),
        _ => return false,
    }
    true
}
