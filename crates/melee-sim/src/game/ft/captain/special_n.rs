//! The Falcon and Warlock Punch (`ftcaptainspecialn.c`): the wind-up and the punch, on the
//! ground and in the air, where the stick angles the lunge.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::{KIND_GANON, addr, attr, ms, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The flags the punch keeps going between the ground and the air (`transition_flags`).
const COLL_FLAGS: u32 = 0x0C4C_5882;

/// The punch's second effect's bone, Captain Falcon's and Ganondorf's (`fp->parts[57]` and
/// `[78]`).
const CA_FIST_BONE: i32 = 57;
const GN_FIST_BONE: i32 = 78;

impl Running<'_> {
    /// The punch's start, on the ground or in the air.
    fn ca_special_n_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_effect_hitlag_cbs(gobj);
        self.anim_update(gobj);
    }

    /// `ftCa_SpecialN_Enter`.
    #[doc(alias = "ftCa_SpecialN_Enter")]
    pub fn ca_special_n_enter(&mut self, gobj: GObjId) {
        self.ca_special_n_start(gobj, ms::SPECIAL_N);
    }

    /// `ftCa_SpecialAirN_Enter`.
    #[doc(alias = "ftCa_SpecialAirN_Enter")]
    pub fn ca_special_air_n_enter(&mut self, gobj: GObjId) {
        self.ca_special_n_start(gobj, ms::SPECIAL_AIR_N);
    }

    /// The Warlock Punch's wind (`ftCaptain_SpecialN_CreateWindEffect`): on Ganondorf's odd
    /// frames, a small blast through its wind-up and a stronger one as it lands.
    fn ca_special_n_wind(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let frame = fp::fctiwz(reg(fp.cur_anim_frame));
        if fp.kind != KIND_GANON || frame & 1 == 0 {
            return;
        }
        let pos = fp.cur_pos;
        let scale = if (16..=50).contains(&frame) {
            2.0
        } else if (51..=68).contains(&frame) {
            4.0
        } else {
            return;
        };
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, 2, scale, scale, 0.0);
    }

    /// `ftCa_SpecialN_Anim`: the wind, then waiting when the punch ends.
    #[doc(alias = "ftCa_SpecialN_Anim")]
    pub fn ca_special_n_anim(&mut self, gobj: GObjId) {
        self.ca_special_n_wind(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCa_SpecialAirN_Anim`: the wind, then a fall when the punch ends.
    #[doc(alias = "ftCa_SpecialAirN_Anim")]
    pub fn ca_special_air_n_anim(&mut self, gobj: GObjId) {
        self.ca_special_n_wind(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The stick's tilt of the aerial punch, in radians
    /// (`ftCaptain_SpecialN_GetAngleVel`): its y beyond `specialn_stick_range_y_neg`, up to
    /// `_pos`, scaled to `specialn_angle_diff` degrees, signed as the stick.
    fn ca_special_n_angle(&self, gobj: GObjId) -> f64 {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let y = reg(fp.input.lstick[0].y);
        let mut stick = if y < 0.0 { fp::fneg(y) } else { y };
        let max = reg(self.ca_attr_f32(f, attr::SPECIALN_STICK_RANGE_Y_POS));
        if stick > max {
            stick = max;
        }
        let min = reg(self.ca_attr_f32(f, attr::SPECIALN_STICK_RANGE_Y_NEG));
        stick = fp::fsubs(stick, min);
        if stick < 0.0 {
            stick = 0.0;
        }
        if y < 0.0 {
            stick = fp::fneg(stick);
        }
        let diff = reg(self.ca_attr_f32(f, attr::SPECIALN_ANGLE_DIFF));
        fp::fmuls(
            0.017_453_292_384_743_69,
            fp::fdivs(fp::fmuls(stick, diff), fp::fsubs(max, min)),
        )
    }

    /// `ftCa_SpecialN_IASA`: nothing.
    #[doc(alias = "ftCa_SpecialN_IASA")]
    pub fn ca_special_n_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCa_SpecialAirN_IASA`: when the script says (`cmd_vars[0]`), the lunge at
    /// `specialn_vel_x` along the stick's tilt.
    #[doc(alias = "ftCa_SpecialAirN_IASA")]
    pub fn ca_special_air_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let f = self.game.fighter_id(gobj);
        let a = mem(self.ca_special_n_angle(gobj));
        let vel = reg(self.ca_attr_f32(f, attr::SPECIALN_VEL_X));
        let s = reg(crate::math::sin(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fmuls(vel, s));
        let c = reg(crate::math::cos(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(vel, fp::fmuls(reg(fp.facing_dir), c)));
    }

    /// The punch's effect when the script raises its flag (`doPhys`): the first time, the
    /// fist's flame; again, its effects end.
    fn ca_special_n_effect(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(1) {
            return;
        }
        fp.set_throw_flag(1, false);
        if fp.x2219_b0 {
            self.destroy_effects(gobj);
            return;
        }
        let bone = if fp.kind == KIND_GANON {
            GN_FIST_BONE
        } else {
            CA_FIST_BONE
        };
        if let Some(id) = self.ca_effect_id(gobj, 0x48F, 0x50B) {
            let top = self.bone_joint(gobj, part::TOP_N);
            let fist = self.bone_joint(gobj, bone);
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::JObj(fist)]);
        }
        self.game.fighter_mut(gobj).x2219_b0 = true;
    }

    /// `ftCa_SpecialN_Phys`: the effect, then friction or the animation's motion.
    #[doc(alias = "ftCa_SpecialN_Phys")]
    pub fn ca_special_n_phys(&mut self, gobj: GObjId) {
        self.ca_special_n_effect(gobj);
        self.ground_friction_or_anim(gobj);
    }

    /// `ftCa_SpecialAirN_Phys`: the effect, then by the script's phase (`cmd_vars[1]`) the
    /// air's physics without drift, the lunge slowing by `specialn_vel_mul`, or the air's
    /// physics.
    #[doc(alias = "ftCa_SpecialAirN_Phys")]
    pub fn ca_special_air_n_phys(&mut self, gobj: GObjId) {
        self.ca_special_n_effect(gobj);
        match self.game.fighter(gobj).cmd_vars[1] {
            0 => self.air_physics_no_drift(gobj),
            1 => {
                let f = self.game.fighter_id(gobj);
                let mul = reg(self.ca_attr_f32(f, attr::SPECIALN_VEL_MUL));
                let fp = &mut self.game.fighters[f.0];
                fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), mul));
                fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), mul));
            }
            2 => self.air_physics(gobj),
            _ => {}
        }
    }

    /// `ftCa_SpecialN_Coll`: off the ground, the aerial punch from the same frame.
    #[doc(alias = "ftCa_SpecialN_Coll")]
    pub fn ca_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
            let f = self.game.fighter_id(gobj);
            self.clamp_air_drift(f);
        }
    }

    /// `ftCa_SpecialAirN_Coll`: landing, the grounded punch from the same frame.
    #[doc(alias = "ftCa_SpecialAirN_Coll")]
    pub fn ca_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_N, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.ca_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.ca_special_air_n_enter(gobj),
        addr::SPECIAL_N_ANIM => g.ca_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.ca_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.ca_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.ca_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.ca_special_n_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.ca_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.ca_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.ca_special_air_n_coll(gobj),
        _ => return false,
    }
    true
}
