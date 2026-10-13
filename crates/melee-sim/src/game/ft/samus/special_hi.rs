//! The Screw Attack (`ftsamusspecialhi.c`): a spinning rise she can turn once, from the ground
//! (rising when the script says) or the air, ending in a helpless fall.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::{PART_Y_ROT_N, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The Screw Attack's spin.
const GFX_SCREW: i32 = 0x482;
/// `M_PI / 2`, in double precision.
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;

impl Running<'_> {
    /// The Screw Attack's start: the state, the charge's cleanup hooked, the effects paused in
    /// hitlag, airborne, the script's variables cleared and not turned, then `rise`, and the
    /// spin's effect on her Y rotation joint.
    fn samus_screw_start(&mut self, gobj: GObjId, msid: i32, rise: impl FnOnce(&mut Self)) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.samus_set_damage_cbs(gobj);
        self.set_effect_hitlag_cbs(gobj);
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::HI_TURNED, 0);
        rise(self);
        self.anim_update(gobj);
        let j = self.bone_joint(gobj, PART_Y_ROT_N);
        ef::spawn(self, GFX_SCREW, Some(gobj), &[Arg::JObj(j)]);
        self.game.fighter_mut(gobj).set_u_i32(u::SCREW_EFFECTS, 1);
    }

    /// `ftSs_SpecialHi_Enter`: on the ground (`ftCommon_8007D7FC`).
    #[doc(alias = "ftSs_SpecialHi_Enter")]
    pub fn samus_special_hi_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.samus_set_damage_cbs(gobj);
        self.set_effect_hitlag_cbs(gobj);
        self.land(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::HI_TURNED, 0);
        self.anim_update(gobj);
        let j = self.bone_joint(gobj, PART_Y_ROT_N);
        ef::spawn(self, GFX_SCREW, Some(gobj), &[Arg::JObj(j)]);
        self.game.fighter_mut(gobj).set_u_i32(u::SCREW_EFFECTS, 1);
    }

    /// `ftSs_SpecialAirHi_Enter`: airborne (`ftCommon_8007D60C`), rising at the attributes'
    /// `x44`, her drift held within `x40`.
    #[doc(alias = "ftSs_SpecialAirHi_Enter")]
    pub fn samus_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.samus_screw_start(gobj, ms::SPECIAL_AIR_HI, |g| {
            let f = g.game.fighter_id(gobj);
            let rise = g.samus_attr_f32(f, attr::HI_AIR_VEL_Y);
            let max = reg(g.samus_attr_f32(f, attr::HI_DRIFT_MAX));
            let fp = &mut g.game.fighters[f.0];
            fp.self_vel.y = rise;
            Self::clamp_self_vel_x(fp, max);
        });
    }

    /// `ftSs_SpecialHi_Anim` and `ftSs_SpecialAirHi_Anim`: at the end the effects go and she
    /// falls, helpless if the attributes give a landing lag.
    #[doc(alias = "ftSs_SpecialHi_Anim")]
    #[doc(alias = "ftSs_SpecialAirHi_Anim")]
    pub fn samus_special_hi_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.samus_screw_effects_off(gobj);
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let lag = self.samus_attr_f32(f, attr::HI_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        let mobility = self.samus_attr_f32(f, attr::HI_MOBILITY);
        self.enter_fall_special(gobj, 1, 1, 0, mobility, lag, 0.0, true);
    }

    /// `ftSs_SpecialHi_IASA` and `ftSs_SpecialAirHi_IASA`: the stick pushed back past the
    /// attributes' `x4C` turns her once.
    #[doc(alias = "ftSs_SpecialHi_IASA")]
    #[doc(alias = "ftSs_SpecialAirHi_IASA")]
    pub fn samus_special_hi_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 0 || fp.mv_i32(mv::HI_TURNED) != 0 {
            return;
        }
        let x = reg(fp.input.lstick[0].x);
        let mag = if x < 0.0 { fp::fneg(x) } else { x };
        if mag <= reg(self.samus_attr_f32(f, attr::HI_TURN_STICK)) {
            return;
        }
        let facing = reg(fp.facing_dir);
        if !((facing == 1.0 && x < 0.0) || (facing == -1.0 && x > 0.0)) {
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[1] = 1;
        fp.set_mv_i32(mv::HI_TURNED, 1);
        self.face_stick(gobj);
        let facing = reg(self.game.fighters[f.0].facing_dir);
        let yaw = mem(fp::frsp(fp::fmul(HALF_PI, facing)));
        self.game.part_set_rot_y(f, 0, yaw);
    }

    /// `ftSs_SpecialHi_Phys`: when the script says (`cmd_vars[0]`) she leaves the ground at
    /// the attributes' `x38`; in the air she rises with the animation and drifts.
    #[doc(alias = "ftSs_SpecialHi_Phys")]
    pub fn samus_special_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.go_airborne_jumped(f);
            let vx = reg(self.samus_attr_f32(f, attr::HI_VEL_X));
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[0] = 0;
            fp.self_vel.x = mem(fp::fmuls(vx, reg(fp.facing_dir)));
        }
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            self.air_anim_rise(gobj);
            let drift = reg(self.samus_attr_f32(f, attr::HI_DRIFT));
            let max = reg(self.samus_attr_f32(f, attr::HI_DRIFT_MAX));
            Self::drift_simple(&mut self.game.fighters[f.0], 0.0, drift, max);
            self.air_drift(gobj);
            return;
        }
        self.ground_friction(gobj);
    }

    /// `ftSs_SpecialAirHi_Phys`: falling with the air's physics, drifting.
    #[doc(alias = "ftSs_SpecialAirHi_Phys")]
    pub fn samus_special_air_hi_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
        let f = self.game.fighter_id(gobj);
        let drift = reg(self.samus_attr_f32(f, attr::HI_DRIFT));
        let max = reg(self.samus_attr_f32(f, attr::HI_DRIFT_MAX));
        Self::drift_simple(&mut self.game.fighters[f.0], 0.0, drift, max);
    }

    /// `ftSs_SpecialHi_Coll` and `ftSs_SpecialAirHi_Coll`: rising she only meets the stage;
    /// falling she lands helpless, or catches a ledge; on the ground, the ground's.
    #[doc(alias = "ftSs_SpecialHi_Coll")]
    #[doc(alias = "ftSs_SpecialAirHi_Coll")]
    pub fn samus_special_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.ground_or_air != GA_AIR {
            self.ground_coll(gobj);
            return;
        }
        if fp.self_vel.y >= 0.0 {
            self.air_coll_land(gobj);
            return;
        }
        let dir = if fp.facing_dir == 1.0 { 1 } else { -1 };
        if self.air_coll_ground_and_ledge(gobj, dir) {
            self.samus_screw_effects_off(gobj);
            let lag = self.samus_attr_f32(f, attr::HI_LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
            return;
        }
        if self.catch_ledge_check(gobj) {
            self.samus_screw_effects_off(gobj);
            self.enter_cliff_catch(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.samus_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.samus_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM | addr::SPECIAL_AIR_HI_ANIM => g.samus_special_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => g.samus_special_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS => g.samus_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.samus_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_COLL | addr::SPECIAL_AIR_HI_COLL => g.samus_special_hi_coll(gobj),
        _ => return false,
    }
    true
}
