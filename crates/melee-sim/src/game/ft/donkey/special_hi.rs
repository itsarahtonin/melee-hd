//! The Spinning Kong (`ftdonkeyspecialhi.c`): a spin along the ground, steered by the stick,
//! or a rising spin in the air with its own gravity until the script says, then a helpless
//! fall. It uses up his jumps.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, part};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// The flags the spin keeps between the ground and the air (`coll_mf`).
const COLL_FLAGS: u32 = 0x0C4C_5080;

/// The spin's effect.
const EFFECT: i32 = 1226;

impl Running<'_> {
    /// The spin's hooks (`setCallbacks`): when hit or KO'd, the Giant Punch's stored swings
    /// lost and the effects gone; the effects pausing through hitlag.
    fn dk_special_hi_set_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
        fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The spin's effect on the top joint.
    fn dk_special_hi_effect(&mut self, gobj: GObjId) {
        let top = self.bone_joint(gobj, part::TOP_N);
        spawn(self, EFFECT, Some(gobj), &[Arg::JObj(top)]);
    }

    /// `ftDk_SpecialHi_Enter`: the ground speed held to `GROUNDED_HORIZONTAL_VELOCITY`, the
    /// jumps used up.
    #[doc(alias = "ftDk_SpecialHi_Enter")]
    pub fn dk_special_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.dk_special_hi_set_cbs(gobj);
        let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        Self::clamp_ground_vel(fp, max);
        fp.self_vel.x = fp.gr_vel;
        fp.self_vel.y = 0.0;
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        self.anim_update(gobj);
        self.dk_special_hi_effect(gobj);
    }

    /// `ftDk_SpecialAirHi_Enter`: the air speed held to `GROUNDED_HORIZONTAL_VELOCITY`, the
    /// rise at `AERIAL_VERTICAL_VELOCITY`, the jumps used up.
    #[doc(alias = "ftDk_SpecialAirHi_Enter")]
    pub fn dk_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.dk_special_hi_set_cbs(gobj);
        let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY));
        let rise = self.dk_attr_f32(f, attr::SPECIAL_HI_AERIAL_VERTICAL_VELOCITY);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        Self::clamp_self_vel_x(fp, max);
        fp.self_vel.y = rise;
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        self.anim_update(gobj);
        self.dk_special_hi_effect(gobj);
    }

    /// `ftDk_SpecialHi_Anim`: waiting when it ends.
    #[doc(alias = "ftDk_SpecialHi_Anim")]
    pub fn dk_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftDk_SpecialAirHi_Anim`: at its end, airborne as after a jump, and a fall (helpless
    /// with `LANDING_LAG`).
    #[doc(alias = "ftDk_SpecialAirHi_Anim")]
    pub fn dk_special_air_hi_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let lag = self.dk_attr_f32(f, attr::SPECIAL_HI_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The spin's empty interrupts.
    #[doc(alias = "ftDk_SpecialHi_IASA")]
    #[doc(alias = "ftDk_SpecialAirHi_IASA")]
    pub fn dk_special_hi_iasa(&mut self, _gobj: GObjId) {}

    /// `ftDk_SpecialHi_Phys`: the ground speed toward the stick's, at `GROUNDED_MOBILITY`.
    #[doc(alias = "ftDk_SpecialHi_Phys")]
    pub fn dk_special_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mobility = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_GROUNDED_MOBILITY));
        let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY));
        Self::ground_accel_to_stick(&mut self.game.fighters[f.0], 0.0, mobility, max);
        self.self_movement_from_ground(gobj);
    }

    /// `ftDk_SpecialAirHi_Phys`: the fall at `AERIAL_GRAVITY` times the gravity until the
    /// script says (`cmd_vars[0]`), then at the gravity, and drift.
    #[doc(alias = "ftDk_SpecialAirHi_Phys")]
    pub fn dk_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = if self.game.fighters[f.0].cmd_vars[0] != 0 {
            1.0
        } else {
            reg(self.dk_attr_f32(f, attr::SPECIAL_HI_AERIAL_GRAVITY))
        };
        let mobility = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_AERIAL_MOBILITY));
        let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY));
        let fp = &mut self.game.fighters[f.0];
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, fp::fmuls(mul, g), t);
        Self::drift_simple(fp, 0.0, mobility, max);
    }

    /// `ftDk_SpecialHi_Coll`: off a ledge, airborne as after a jump and the aerial spin from
    /// the same frame, its air speed held to `AERIAL_HORIZONTAL_VELOCITY`.
    #[doc(alias = "ftDk_SpecialHi_Coll")]
    pub fn dk_special_hi_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, COLL_FLAGS, frame, 1.0, 0.0, None);
        self.dk_special_hi_set_cbs(gobj);
        let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY));
        Self::clamp_self_vel_x(&mut self.game.fighters[f.0], max);
    }

    /// `ftDk_SpecialAirHi_Coll`: rising, landing on a floor; falling, landing or catching a
    /// ledge; on landing, the grounded spin from the same frame with its ground speed held.
    #[doc(alias = "ftDk_SpecialAirHi_Coll")]
    pub fn dk_special_air_hi_coll(&mut self, gobj: GObjId) {
        let landed = if self.game.fighter(gobj).self_vel.y >= 0.0 {
            self.air_coll_land(gobj)
        } else if self.check_ground_and_ledge(gobj, 0) {
            true
        } else {
            if self.catch_ledge_check(gobj) {
                self.enter_cliff_catch(gobj);
            }
            false
        };
        if landed {
            self.fox_air_to_ground(gobj, ms::SPECIAL_HI, COLL_FLAGS);
            self.dk_special_hi_set_cbs(gobj);
            let f = self.game.fighter_id(gobj);
            let max = reg(self.dk_attr_f32(f, attr::SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY));
            Self::clamp_ground_vel(&mut self.game.fighters[f.0], max);
        }
    }

    /// The spin's effects gone (`ftDk_SpecialHi_DestroyAllEffects`).
    #[doc(alias = "ftDk_SpecialHi_DestroyAllEffects")]
    pub fn dk_special_hi_destroy_effects(&mut self, gobj: GObjId) {
        ef::destroy_all(self, gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        SPECIAL_HI_ENTER => g.dk_special_hi_enter(gobj),
        SPECIAL_AIR_HI_ENTER => g.dk_special_air_hi_enter(gobj),
        SPECIAL_HI_ANIM => g.dk_special_hi_anim(gobj),
        SPECIAL_AIR_HI_ANIM => g.dk_special_air_hi_anim(gobj),
        SPECIAL_HI_IASA | SPECIAL_AIR_HI_IASA => g.dk_special_hi_iasa(gobj),
        SPECIAL_HI_PHYS => g.dk_special_hi_phys(gobj),
        SPECIAL_AIR_HI_PHYS => g.dk_special_air_hi_phys(gobj),
        SPECIAL_HI_COLL => g.dk_special_hi_coll(gobj),
        SPECIAL_AIR_HI_COLL => g.dk_special_air_hi_coll(gobj),
        SPECIAL_HI_DESTROY_EFFECTS => g.dk_special_hi_destroy_effects(gobj),
        _ => return false,
    }
    true
}
