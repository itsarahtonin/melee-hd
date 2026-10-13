//! Pikachu's callbacks that are no more than a common helper or nothing (`ftpikachu*.c`'s
//! empty IASA callbacks and one-call physics), as the states hold them.

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The states' empty callbacks.
    #[doc(alias = "ftPk_SpecialN_IASA")]
    #[doc(alias = "ftPk_SpecialAirN_IASA")]
    #[doc(alias = "ftPk_SpecialSStart_IASA")]
    #[doc(alias = "ftPk_SpecialAirSStart_IASA")]
    #[doc(alias = "ftPk_SpecialS0_IASA")]
    #[doc(alias = "ftPk_SpecialAirS0_IASA")]
    #[doc(alias = "ftPk_SpecialS1_Anim")]
    #[doc(alias = "ftPk_SpecialS1_IASA")]
    #[doc(alias = "ftPk_SpecialAirS1_IASA")]
    #[doc(alias = "ftPk_SpecialS1_Phys")]
    #[doc(alias = "ftPk_SpecialS1_Coll")]
    #[doc(alias = "ftPk_SpecialSEnd_IASA")]
    #[doc(alias = "ftPk_SpecialAirSEnd_IASA")]
    #[doc(alias = "ftPk_SpecialHiStart0_IASA")]
    #[doc(alias = "ftPk_SpecialAirHiStart0_IASA")]
    #[doc(alias = "ftPk_SpecialHiStart1_IASA")]
    #[doc(alias = "ftPk_SpecialAirHiStart1_IASA")]
    #[doc(alias = "ftPk_SpecialHiEnd_IASA")]
    #[doc(alias = "ftPk_SpecialAirHiEnd_IASA")]
    pub fn pk_nothing(&mut self, _gobj: GObjId) {}

    /// Ground friction (`ft_80084F3C`).
    #[doc(alias = "ftPk_SpecialN_Phys")]
    #[doc(alias = "ftPk_SpecialSHold_Phys")]
    #[doc(alias = "ftPk_SpecialS0_Phys")]
    #[doc(alias = "ftPk_SpecialHiStart0_Phys")]
    #[doc(alias = "ftPk_SpecialLwStart_Phys")]
    #[doc(alias = "ftPk_SpecialLwLoop0_Phys")]
    #[doc(alias = "ftPk_SpecialLwLoop1_Phys")]
    #[doc(alias = "ftPk_SpecialLwEnd_Phys")]
    pub fn pk_ground_friction(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Air physics without drift (`ft_80084EEC`).
    #[doc(alias = "ftPk_SpecialAirSHold_Phys")]
    #[doc(alias = "ftPk_SpecialAirS0_Phys")]
    #[doc(alias = "ftPk_SpecialAirLwStart_Phys")]
    #[doc(alias = "ftPk_SpecialAirLwLoop0_Phys")]
    #[doc(alias = "ftPk_SpecialAirLwEnd_Phys")]
    pub fn pk_air_physics_no_drift(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftPk_SpecialAirN_Phys`: air physics (`ft_80084DB0`).
    #[doc(alias = "ftPk_SpecialAirN_Phys")]
    pub fn pk_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftPk_SpecialHiStart1_Phys`: the zip's speed along the ground.
    #[doc(alias = "ftPk_SpecialHiStart1_Phys")]
    pub fn pk_special_hi_start1_phys(&mut self, gobj: GObjId) {
        self.self_movement_from_ground(gobj);
    }

    /// `ftPk_SpecialAirHiStart1_Phys`: the aerial zip's pose and speeds
    /// (`ftPk_SpecialHi_8012642C`).
    #[doc(alias = "ftPk_SpecialAirHiStart1_Phys")]
    pub fn pk_special_air_hi_start1_phys(&mut self, gobj: GObjId) {
        self.pk_special_hi_air_zip_pose(gobj);
    }

    /// `ftPk_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftPk_Init_OnKnockbackEnter")]
    pub fn pk_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftPk_Init_OnKnockbackExit`.
    #[doc(alias = "ftPk_Init_OnKnockbackExit")]
    pub fn pk_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }
}
