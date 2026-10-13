//! Mario's Tornado (`ftmariospeciallw.c`), Dr. Mario's too: a spin the stick steers, which
//! tapping B lifts, once per time airborne in the air; it tilts with a sloped floor
//! ([`plumber`](super::plumber)).

use super::super::super::state::Running;
use super::plumber::Tornado;
use super::{addr, attr, ms, u};
use crate::hsd::gobj::GObjId;

const TORNADO: Tornado = Tornado {
    base: attr::SPECIALLW,
    ms_lw: ms::SPECIAL_LW,
    ms_air_lw: ms::SPECIAL_AIR_LW,
    charged: u::TORNADO_CHARGE,
    effect: 0x47C,
    on_damage: addr::SPECIAL_LW_UPDATE_ROT,
};

impl Running<'_> {
    /// `updateRot` (the spin's damage hook): the model upright again.
    // Ports `updateRot`; aliased once a check reaches it.
    pub fn mr_special_lw_update_rot(&mut self, gobj: GObjId) {
        self.plumber_lw_update_rot(gobj);
    }

    /// `ftMr_SpecialLw_Enter`: from the ground the spin starts in its aerial state too.
    // Ports `ftMr_SpecialLw_Enter`; aliased once a check reaches it.
    pub fn mr_special_lw_enter(&mut self, gobj: GObjId) {
        self.plumber_lw_enter(gobj, &TORNADO, true);
    }

    /// `ftMr_SpecialAirLw_Enter`.
    // Ports `ftMr_SpecialAirLw_Enter`; aliased once a check reaches it.
    pub fn mr_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.plumber_lw_enter(gobj, &TORNADO, false);
    }

    /// `ftMr_SpecialLw_Anim`.
    // Ports `ftMr_SpecialLw_Anim`; aliased once a check reaches it.
    pub fn mr_special_lw_anim(&mut self, gobj: GObjId) {
        self.plumber_lw_anim(gobj);
    }

    /// `ftMr_SpecialAirLw_Anim`.
    // Ports `ftMr_SpecialAirLw_Anim`; aliased once a check reaches it.
    pub fn mr_special_air_lw_anim(&mut self, gobj: GObjId) {
        self.plumber_air_lw_anim(gobj, &TORNADO);
    }

    /// `ftMr_SpecialLw_Phys`.
    // Ports `ftMr_SpecialLw_Phys`; aliased once a check reaches it.
    pub fn mr_special_lw_phys(&mut self, gobj: GObjId) {
        self.plumber_lw_phys(gobj, &TORNADO);
    }

    /// `ftMr_SpecialAirLw_Phys`.
    // Ports `ftMr_SpecialAirLw_Phys`; aliased once a check reaches it.
    pub fn mr_special_air_lw_phys(&mut self, gobj: GObjId) {
        self.plumber_air_lw_phys(gobj, &TORNADO);
    }

    /// `ftMr_SpecialLw_Coll`.
    // Ports `ftMr_SpecialLw_Coll`; aliased once a check reaches it.
    pub fn mr_special_lw_coll(&mut self, gobj: GObjId) {
        self.plumber_lw_coll(gobj, &TORNADO);
    }

    /// `ftMr_SpecialAirLw_Coll`.
    // Ports `ftMr_SpecialAirLw_Coll`; aliased once a check reaches it.
    pub fn mr_special_air_lw_coll(&mut self, gobj: GObjId) {
        self.plumber_air_lw_coll(gobj, &TORNADO);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_UPDATE_ROT => g.mr_special_lw_update_rot(gobj),
        addr::SPECIAL_LW_ENTER => g.mr_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.mr_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.mr_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.mr_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA | addr::SPECIAL_AIR_LW_IASA => {}
        addr::SPECIAL_LW_PHYS => g.mr_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.mr_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.mr_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.mr_special_air_lw_coll(gobj),
        _ => return false,
    }
    true
}
