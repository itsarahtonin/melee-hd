//! Luigi's callbacks that are no more than a common helper or nothing (`ftluigi*.c`'s empty
//! callbacks and one-call physics), as the states hold them.

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The states' empty callbacks.
    #[doc(alias = "ftLg_SpecialSStart_IASA")]
    #[doc(alias = "ftLg_SpecialAirSStart_IASA")]
    #[doc(alias = "ftLg_SpecialS_IASA")]
    #[doc(alias = "ftLg_SpecialAirS_IASA")]
    #[doc(alias = "ftLg_SpecialSMisfire_IASA")]
    #[doc(alias = "ftLg_SpecialAirSMisfire_IASA")]
    #[doc(alias = "ftLg_SpecialS2_Anim")]
    #[doc(alias = "ftLg_SpecialS2_IASA")]
    #[doc(alias = "ftLg_SpecialAirS2_IASA")]
    #[doc(alias = "ftLg_SpecialS2_Phys")]
    #[doc(alias = "ftLg_SpecialS2_Coll")]
    #[doc(alias = "ftLg_SpecialSEnd_IASA")]
    #[doc(alias = "ftLg_SpecialAirSEnd_IASA")]
    #[doc(alias = "ftLg_SpecialLw_IASA")]
    #[doc(alias = "ftLg_SpecialAirLw_IASA")]
    pub fn lg_nothing(&mut self, _gobj: GObjId) {}

    /// Ground friction (`ft_80084F3C`), which the aerial misfire runs too.
    #[doc(alias = "ftLg_SpecialN_Phys")]
    #[doc(alias = "ftLg_SpecialSHold_Phys")]
    #[doc(alias = "ftLg_SpecialS_Phys")]
    #[doc(alias = "ftLg_SpecialSMisfire_Phys")]
    #[doc(alias = "ftLg_SpecialAirSMisfire_Phys")]
    pub fn lg_ground_friction(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Air physics without drift (`ft_80084EEC`).
    #[doc(alias = "ftLg_SpecialAirSHold_Phys")]
    #[doc(alias = "ftLg_SpecialAirS_Phys")]
    pub fn lg_air_physics_no_drift(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftLg_SpecialAirN_Phys`: air physics (`ft_80084DB0`).
    #[doc(alias = "ftLg_SpecialAirN_Phys")]
    pub fn lg_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftLg_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftLg_Init_OnKnockbackEnter")]
    pub fn lg_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftLg_Init_OnKnockbackExit`.
    #[doc(alias = "ftLg_Init_OnKnockbackExit")]
    pub fn lg_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}
