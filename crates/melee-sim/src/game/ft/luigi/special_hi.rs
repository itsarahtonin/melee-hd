//! Luigi's Super Jump Punch (`ftluigispecialhi.c`): a rising punch the stick angles and
//! turns, ending in a helpless fall; built as Mario's ([`plumber`](super::super::mario::plumber)).

use super::super::super::state::Running;
use super::super::mario::plumber::SuperJump;
use super::{addr, attr, ms};
use crate::hsd::gobj::GObjId;

const SUPER_JUMP: SuperJump = SuperJump {
    base: attr::SUPERJUMP_FREEFALL_MOBILITY,
    ms_hi: ms::SPECIAL_HI,
    ms_air_hi: ms::SPECIAL_AIR_HI,
    landing: |g, gobj| g.lg_special_hi_check_landing(gobj),
};

impl Running<'_> {
    /// `ftLg_SpecialHi_Enter`.
    #[doc(alias = "ftLg_SpecialHi_Enter")]
    pub fn lg_special_hi_enter(&mut self, gobj: GObjId) {
        self.plumber_hi_enter(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialAirHi_Enter`.
    #[doc(alias = "ftLg_SpecialAirHi_Enter")]
    pub fn lg_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.plumber_air_hi_enter(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialHi_Anim`.
    #[doc(alias = "ftLg_SpecialHi_Anim")]
    pub fn lg_special_hi_anim(&mut self, gobj: GObjId) {
        self.plumber_hi_anim(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialAirHi_Anim`.
    #[doc(alias = "ftLg_SpecialAirHi_Anim")]
    pub fn lg_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.plumber_hi_anim(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialHi_IASA`.
    #[doc(alias = "ftLg_SpecialHi_IASA")]
    pub fn lg_special_hi_iasa(&mut self, gobj: GObjId) {
        self.plumber_hi_iasa(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialAirHi_IASA`.
    #[doc(alias = "ftLg_SpecialAirHi_IASA")]
    pub fn lg_special_air_hi_iasa(&mut self, gobj: GObjId) {
        self.plumber_hi_iasa(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialHi_Phys`.
    #[doc(alias = "ftLg_SpecialHi_Phys")]
    pub fn lg_special_hi_phys(&mut self, gobj: GObjId) {
        self.plumber_hi_phys(gobj);
    }

    /// `ftLg_SpecialAirHi_Phys`.
    #[doc(alias = "ftLg_SpecialAirHi_Phys")]
    pub fn lg_special_air_hi_phys(&mut self, gobj: GObjId) {
        self.plumber_air_hi_phys(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialHi_CheckLanding`.
    #[doc(alias = "ftLg_SpecialHi_CheckLanding")]
    pub fn lg_special_hi_check_landing(&mut self, gobj: GObjId) {
        self.plumber_hi_check_landing(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialHi_Coll`.
    #[doc(alias = "ftLg_SpecialHi_Coll")]
    pub fn lg_special_hi_coll(&mut self, gobj: GObjId) {
        self.plumber_hi_coll(gobj, &SUPER_JUMP);
    }

    /// `ftLg_SpecialAirHi_Coll`.
    #[doc(alias = "ftLg_SpecialAirHi_Coll")]
    pub fn lg_special_air_hi_coll(&mut self, gobj: GObjId) {
        self.plumber_hi_coll(gobj, &SUPER_JUMP);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.lg_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.lg_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM => g.lg_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.lg_special_air_hi_anim(gobj),
        addr::SPECIAL_HI_IASA => g.lg_special_hi_iasa(gobj),
        addr::SPECIAL_AIR_HI_IASA => g.lg_special_air_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS => g.lg_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.lg_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_CHECK_LANDING => g.lg_special_hi_check_landing(gobj),
        addr::SPECIAL_HI_COLL => g.lg_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.lg_special_air_hi_coll(gobj),
        _ => return false,
    }
    true
}
