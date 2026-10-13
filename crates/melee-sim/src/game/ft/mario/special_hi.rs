//! Mario's Super Jump Punch (`ftmariospecialhi.c`), Dr. Mario's too: a rising punch the stick
//! angles and turns, ending in a helpless fall ([`plumber`](super::plumber)).

use super::super::super::state::Running;
use super::plumber::SuperJump;
use super::{addr, attr, ms};
use crate::hsd::gobj::GObjId;

const SUPER_JUMP: SuperJump = SuperJump {
    base: attr::SPECIALHI,
    ms_hi: ms::SPECIAL_HI,
    ms_air_hi: ms::SPECIAL_AIR_HI,
    landing: |g, gobj| g.mr_special_hi_check_landing(gobj),
};

impl Running<'_> {
    /// `ftMr_SpecialHi_Enter`.
    // Ports `ftMr_SpecialHi_Enter`; aliased once a check reaches it.
    pub fn mr_special_hi_enter(&mut self, gobj: GObjId) {
        self.plumber_hi_enter(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialAirHi_Enter`.
    // Ports `ftMr_SpecialAirHi_Enter`; aliased once a check reaches it.
    pub fn mr_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.plumber_air_hi_enter(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialHi_Anim`.
    // Ports `ftMr_SpecialHi_Anim`; aliased once a check reaches it.
    pub fn mr_special_hi_anim(&mut self, gobj: GObjId) {
        self.plumber_hi_anim(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialAirHi_Anim`.
    // Ports `ftMr_SpecialAirHi_Anim`; aliased once a check reaches it.
    pub fn mr_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.plumber_hi_anim(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialHi_IASA`.
    // Ports `ftMr_SpecialHi_IASA`; aliased once a check reaches it.
    pub fn mr_special_hi_iasa(&mut self, gobj: GObjId) {
        self.plumber_hi_iasa(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialAirHi_IASA`.
    // Ports `ftMr_SpecialAirHi_IASA`; aliased once a check reaches it.
    pub fn mr_special_air_hi_iasa(&mut self, gobj: GObjId) {
        self.plumber_hi_iasa(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialHi_Phys`.
    // Ports `ftMr_SpecialHi_Phys`; aliased once a check reaches it.
    pub fn mr_special_hi_phys(&mut self, gobj: GObjId) {
        self.plumber_hi_phys(gobj);
    }

    /// `ftMr_SpecialAirHi_Phys`.
    // Ports `ftMr_SpecialAirHi_Phys`; aliased once a check reaches it.
    pub fn mr_special_air_hi_phys(&mut self, gobj: GObjId) {
        self.plumber_air_hi_phys(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialHi_CheckLanding`.
    // Ports `ftMr_SpecialHi_CheckLanding`; aliased once a check reaches it.
    pub fn mr_special_hi_check_landing(&mut self, gobj: GObjId) {
        self.plumber_hi_check_landing(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialHi_Coll`.
    // Ports `ftMr_SpecialHi_Coll`; aliased once a check reaches it.
    pub fn mr_special_hi_coll(&mut self, gobj: GObjId) {
        self.plumber_hi_coll(gobj, &SUPER_JUMP);
    }

    /// `ftMr_SpecialAirHi_Coll`.
    // Ports `ftMr_SpecialAirHi_Coll`; aliased once a check reaches it.
    pub fn mr_special_air_hi_coll(&mut self, gobj: GObjId) {
        self.plumber_hi_coll(gobj, &SUPER_JUMP);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.mr_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.mr_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM => g.mr_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.mr_special_air_hi_anim(gobj),
        addr::SPECIAL_HI_IASA => g.mr_special_hi_iasa(gobj),
        addr::SPECIAL_AIR_HI_IASA => g.mr_special_air_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS => g.mr_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.mr_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_CHECK_LANDING => g.mr_special_hi_check_landing(gobj),
        addr::SPECIAL_HI_COLL => g.mr_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.mr_special_air_hi_coll(gobj),
        _ => return false,
    }
    true
}
