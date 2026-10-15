//! Kirby's dash attack (`ftkirbyattackdash.c`): the common dash attack's callbacks in a state of
//! his own, which carries on in the air when he runs off a ledge.

use super::super::super::state::Running;
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

/// What the switches between the ground and the air keep (`0x0C4C5082`).
const COLL_FLAGS: u32 = 0x0C4C_5082;

impl Running<'_> {
    /// How far Kirby's body leans, `ftKb_AttackDashAir_800F21C0`: his first part's X rotation.
    #[doc(alias = "ftKb_AttackDashAir_800F21C0")]
    pub fn kb_lean_angle(&self, gobj: GObjId) -> f32 {
        self.game.part_rot_x(self.game.fighter_id(gobj), 0)
    }

    /// Kirby's dash attack, `ftKb_SpecialN_800F1F68` (the common dash attack's entry for him).
    #[doc(alias = "ftKb_SpecialN_800F1F68")]
    pub fn kb_attack_dash_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, ms::ATTACK_DASH, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(0, 0);
    }

    /// Landing from the aerial dash attack into the grounded one, `fn_800F1FDC`.
    #[doc(alias = "fn_800F1FDC")]
    pub fn kb_attack_dash_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let fp = self.game.fighter(gobj);
        let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, ms::ATTACK_DASH, COLL_FLAGS, frame, speed, 0.0, None);
    }

    /// `ftKb_AttackDash_Anim` (`ftCo_AttackDash_Anim`).
    #[doc(alias = "ftKb_AttackDash_Anim")]
    pub fn kb_attack_dash_anim(&mut self, gobj: GObjId) {
        self.co_wait_when_done_anim(gobj);
    }

    /// `ftKb_AttackDash_IASA` (`ftCo_AttackDash_IASA`).
    #[doc(alias = "ftKb_AttackDash_IASA")]
    pub fn kb_attack_dash_iasa(&mut self, gobj: GObjId) {
        self.co_attack_dash_iasa(gobj);
    }

    /// `ftKb_AttackDash_Phys` (`ft_80084FA8`).
    #[doc(alias = "ftKb_AttackDash_Phys")]
    pub fn kb_attack_dash_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// `ftKb_AttackDash_Coll`: off a ledge into the aerial one.
    #[doc(alias = "ftKb_AttackDash_Coll")]
    pub fn kb_attack_dash_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.kb_attack_dash_to_air(gobj));
    }

    /// Running off a ledge in the dash attack, `fn_800F20C4`.
    #[doc(alias = "fn_800F20C4")]
    pub fn kb_attack_dash_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let fp = &self.game.fighters[f.0];
        let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, ms::ATTACK_DASH_AIR, COLL_FLAGS, frame, speed, 0.0, None);
    }

    /// `ftKb_AttackDashAir_Anim`: falling once it ends.
    #[doc(alias = "ftKb_AttackDashAir_Anim")]
    pub fn kb_attack_dash_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_AttackDashAir_IASA`: nothing.
    #[doc(alias = "ftKb_AttackDashAir_IASA")]
    pub fn kb_attack_dash_air_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_AttackDashAir_Phys`: the animation's motion along his facing, and falling.
    #[doc(alias = "ftKb_AttackDashAir_Phys")]
    pub fn kb_attack_dash_air_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion_x(gobj);
        Self::fall_basic(self.game.fighter_mut(gobj));
    }

    /// `ftKb_AttackDashAir_Coll`: landing into the grounded one.
    #[doc(alias = "ftKb_AttackDashAir_Coll")]
    pub fn kb_attack_dash_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.kb_attack_dash_to_ground(gobj));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_DASH_ANIM => g.kb_attack_dash_anim(gobj),
        addr::ATTACK_DASH_IASA => g.kb_attack_dash_iasa(gobj),
        addr::ATTACK_DASH_PHYS => g.kb_attack_dash_phys(gobj),
        addr::ATTACK_DASH_COLL => g.kb_attack_dash_coll(gobj),
        addr::ATTACK_DASH_AIR_ANIM => g.kb_attack_dash_air_anim(gobj),
        addr::ATTACK_DASH_AIR_IASA => g.kb_attack_dash_air_iasa(gobj),
        addr::ATTACK_DASH_AIR_PHYS => g.kb_attack_dash_air_phys(gobj),
        addr::ATTACK_DASH_AIR_COLL => g.kb_attack_dash_air_coll(gobj),
        _ => return false,
    }
    true
}
