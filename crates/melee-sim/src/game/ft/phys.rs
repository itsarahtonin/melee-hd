//! Fighter physics (`ftcommon.c`, `ft_081B.c`, `ft_084E.c`): gravity, air drift, ground
//! friction and the velocities they leave for `Fighter_procUpdate` to apply.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::state::Running;
use super::super::structs::Fighter;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `abs` as the original's inline spells it: compare with 0, negate.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

/// `Ft_Kind_Popo` and `Ft_Kind_Nana`: the Ice Climbers, whom no floor makes slippery.
const ICE_CLIMBERS: [i32; 2] = [10, 11];

impl Running<'_> {
    /// The floor's friction multiplier, `ft_GetGroundFrictionMultiplier`: the terrain's
    /// (`mpColl_8004CA6C`, inline here), 1 off the ground or for the Ice Climbers.
    #[doc(alias = "ft_GetGroundFrictionMultiplier")]
    pub fn ground_friction_mul(&self, fp: &Fighter) -> f64 {
        if ICE_CLIMBERS.contains(&fp.kind) {
            return 1.0;
        }
        if fp.coll_data.floor.index == -1 {
            return 1.0;
        }
        let grkind = self.game.stage_info.grkind;
        reg(crate::mp::friction(grkind, fp.coll_data.floor.flags))
    }

    /// Ground friction, `ft_80084F3C`: more above walking speed.
    #[doc(alias = "ft_80084F3C")]
    pub fn ground_friction(&mut self, gobj: GObjId) {
        let above = reg(self.common().f32(0x6C));
        let fp = self.game.fighter_mut(gobj);
        let mut friction = reg(fp.co_attrs.ground_friction);
        if abs(reg(fp.gr_vel)) > reg(fp.co_attrs.walk_max_vel) {
            friction = fp::fmuls(friction, above);
        }
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftCommon_CalcGroundAccel_Deaccel`: friction against the ground speed, stopping at 0.
    #[doc(alias = "ftCommon_CalcGroundAccel_Deaccel")]
    pub fn ground_accel_deaccel(fp: &mut Fighter, friction: f64) {
        let gr = reg(fp.gr_vel);
        let f = if abs(friction) > abs(gr) {
            fp::fneg(gr)
        } else if gr > 0.0 {
            fp::fneg(friction)
        } else {
            friction
        };
        fp.xE4_ground_accel_1 = mem(f);
    }

    /// The ground speed and its acceleration as velocities along the floor,
    /// `ftCommon_SetSelfMovementFromGroundedMovement`.
    #[doc(alias = "ftCommon_SetSelfMovementFromGroundedMovement")]
    pub fn self_movement_from_ground(&mut self, gobj: GObjId) {
        let mul = self.ground_friction_mul(self.game.fighter(gobj));
        let fp = self.game.fighter_mut(gobj);
        let n = fp.coll_data.floor.normal;
        if mul < 1.0 {
            fp.xE4_ground_accel_1 = mem(fp::fmuls(reg(fp.xE4_ground_accel_1), mul));
        }
        let a = reg(fp.xE4_ground_accel_1);
        fp.x74_self_accel.x = mem(fp::fmuls(reg(n.y), a));
        fp.x74_self_accel.y = mem(fp::fmuls(fp::fneg(reg(n.x)), a));
        fp.x74_self_accel.z = 0.0;
        let g = reg(fp.gr_vel);
        fp.self_vel.x = mem(fp::fmuls(reg(n.y), g));
        fp.self_vel.y = mem(fp::fmuls(fp::fneg(reg(n.x)), g));
        fp.self_vel.z = 0.0;
    }

    /// Gravity or a fast fall, then air drift, `ft_80084DB0`.
    #[doc(alias = "ft_80084DB0")]
    pub fn air_physics(&mut self, gobj: GObjId) {
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.fall_fast {
            fp.self_vel.y = mem(fp::fneg(reg(fp.co_attrs.fast_fall_velocity)));
        } else {
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
        }
        self.air_drift(gobj);
    }

    /// Gravity and air friction without drift, `ft_80084EEC`.
    #[doc(alias = "ft_80084EEC")]
    pub fn air_physics_no_drift(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// Starts a fast fall when the stick was flicked down while falling,
    /// `ftCommon_CheckFallFast`.
    #[doc(alias = "ftCommon_CheckFallFast")]
    pub fn check_fall_fast(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (stick, window) = (reg(c.f32(0x88)), c.i32(0x8C));
        let fp = self.game.fighter_mut(gobj);
        if !fp.fall_fast
            && fp.self_vel.y < 0.0
            && reg(fp.input.lstick[0].y) <= fp::fneg(stick)
            && i32::from(fp.active_timer.lstick.y) < window
        {
            fp.fall_fast = true;
            fp.active_timer.lstick.y = 254;
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, 0x96, 0x7F, 0x40);
            return true;
        }
        false
    }

    /// `ftCommon_Fall`: gravity, down to the terminal velocity.
    #[doc(alias = "ftCommon_Fall")]
    pub fn fall(fp: &mut Fighter, gravity: f64, terminal: f64) {
        fp.self_vel.y = mem(fp::fsubs(reg(fp.self_vel.y), gravity));
        if reg(fp.self_vel.y) < fp::fneg(terminal) {
            fp.self_vel.y = mem(fp::fneg(terminal));
        }
    }

    /// Air drift from the stick, `ftCommon_CalcSelfAccel_Drift` and
    /// `ftCommon_CalcSelfAccel_DriftFrom`.
    #[doc(alias = "ftCommon_CalcSelfAccel_Drift")]
    #[doc(alias = "ftCommon_CalcSelfAccel_DriftFrom")]
    pub fn air_drift(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let vel = reg(fp.self_vel.x);
        let lsx = reg(fp.input.lstick[0].x);
        let a = &fp.co_attrs;
        let scaling = fp::fmuls(lsx, reg(a.air_drift_stick_mul));
        let flat = if lsx > 0.0 {
            reg(a.aerial_drift_base)
        } else {
            fp::fneg(reg(a.aerial_drift_base))
        };
        let target = fp::fmuls(lsx, reg(a.air_drift_max));
        let friction = reg(a.aerial_friction);
        Self::accel_to_vel_clamped(fp, vel, fp::fadds(scaling, flat), target, friction);
    }

    /// Accelerates toward a target velocity, braking by `friction` past it and never past
    /// the character's top air speed, `ftCommon_CalcSelfAccel_AccelToVelClampedFrom`.
    #[doc(alias = "ftCommon_CalcSelfAccel_AccelToVelClampedFrom")]
    pub fn accel_to_vel_clamped(
        fp: &mut Fighter,
        vel: f64,
        mut accel: f64,
        target: f64,
        friction: f64,
    ) {
        if target == 0.0 {
            Self::self_accel_deaccel(fp, friction);
            return;
        }
        let max = reg(fp.co_attrs.air_max_horizontal_velocity);
        if !(fp::fmuls(vel, accel) < 0.0) {
            if accel > 0.0 {
                if fp::fadds(vel, accel) > target {
                    accel = fp::fneg(friction);
                    if fp::fadds(vel, accel) < target {
                        accel = fp::fsubs(target, vel);
                    }
                    if fp::fadds(vel, accel) > max {
                        accel = fp::fsubs(max, vel);
                    }
                }
            } else if fp::fadds(vel, accel) < target {
                accel = friction;
                if fp::fadds(vel, accel) > target {
                    accel = fp::fsubs(target, vel);
                }
                if fp::fadds(vel, accel) < fp::fneg(max) {
                    accel = fp::fsubs(fp::fneg(max), vel);
                }
            }
        }
        fp.x74_self_accel.x = mem(accel);
    }

    /// Friction against the horizontal velocity, stopping at 0,
    /// `ftCommon_CalcSelfAccel_Deaccel`.
    #[doc(alias = "ftCommon_CalcSelfAccel_Deaccel")]
    pub fn self_accel_deaccel(fp: &mut Fighter, friction: f64) {
        let v = reg(fp.self_vel.x);
        let f = if abs(friction) >= abs(v) {
            fp::fneg(v)
        } else if v > 0.0 {
            fp::fneg(friction)
        } else {
            friction
        };
        fp.x74_self_accel.x = mem(f);
    }

    /// Ground friction on knockback, `ftCommon_ApplyGroundedKnockbackFriction`.
    #[doc(alias = "ftCommon_ApplyGroundedKnockbackFriction")]
    pub fn grounded_knockback_friction(fp: &mut Fighter, friction: f64) {
        fp.xF0_ground_kb_vel = toward_zero(fp.xF0_ground_kb_vel, friction);
    }

    /// Ground friction on shield pushback, `ftCommon_ApplyShieldKnockbackFriction`.
    #[doc(alias = "ftCommon_ApplyShieldKnockbackFriction")]
    pub fn shield_knockback_friction(fp: &mut Fighter, friction: f64) {
        fp.xF4_ground_attacker_shield_kb_vel =
            toward_zero(fp.xF4_ground_attacker_shield_kb_vel, friction);
    }

    /// Ground friction, or the animation's own motion when TransN drives it, `ft_80084FA8`.
    #[doc(alias = "ft_80084FA8")]
    pub fn ground_friction_or_anim(&mut self, gobj: GObjId) {
        let above = reg(self.common().f32(0x6C));
        let fp = self.game.fighter(gobj);
        let mut friction = reg(fp.co_attrs.ground_friction);
        if abs(reg(fp.gr_vel)) > reg(fp.co_attrs.walk_max_vel) {
            friction = fp::fmuls(friction, above);
        }
        let facing = reg(fp.facing_dir);
        self.ground_anim_motion(gobj, friction, facing);
    }

    /// The animation's TransN motion as ground speed when `x594_b0` says it drives the
    /// fighter, else friction, `ft_80085030`.
    #[doc(alias = "ft_80085030")]
    pub fn ground_anim_motion(&mut self, gobj: GObjId, friction: f64, facing: f64) {
        let fp = self.game.fighter_mut(gobj);
        if super::anim::x594_bit(fp, 0) {
            fp.xE4_ground_accel_1 = mem(fp::fmsubs(
                reg(fp.x6A4_transNOffset.z),
                facing,
                reg(fp.gr_vel),
            ));
        } else {
            Self::ground_accel_deaccel(fp, friction);
        }
        self.self_movement_from_ground(gobj);
    }

    /// `ft_80085004`: the animation's motion along the throw's facing (`facing_dir1`).
    #[doc(alias = "ft_80085004")]
    pub fn ground_anim_motion_thrown(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (friction, facing) = (reg(fp.co_attrs.ground_friction), reg(fp.facing_dir1));
        self.ground_anim_motion(gobj, friction, facing);
    }

    /// The animation's TransN motion as the ground speed itself when `x594_b0` says it
    /// drives the fighter, else friction, `ft_800850E0`.
    #[doc(alias = "ft_800850E0")]
    pub fn ground_anim_speed(&mut self, gobj: GObjId, friction: f64, facing: f64) {
        let fp = self.game.fighter_mut(gobj);
        if super::anim::x594_bit(fp, 0) {
            fp.gr_vel = mem(fp::fmuls(reg(fp.x6A4_transNOffset.z), facing));
        } else {
            Self::ground_accel_deaccel(fp, friction);
        }
        self.self_movement_from_ground(gobj);
    }

    /// `ft_80085088`: [`Self::ground_anim_speed`] with the ground friction, facing the
    /// fighter's way.
    #[doc(alias = "ft_80085088")]
    pub fn ground_anim_speed_facing(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (friction, facing) = (reg(fp.co_attrs.ground_friction), reg(fp.facing_dir));
        self.ground_anim_speed(gobj, friction, facing);
    }

    /// `ft_800851C0`: the animation's TransN rise as the vertical velocity.
    // Ports `ft_800851C0`; aliased once a check reaches it.
    pub fn air_anim_rise(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.y = fp.x6A4_transNOffset.y;
    }

    /// `ftCommon_8007F76C`: the ground speed and horizontal velocity turned the way the
    /// fighter faces.
    // Ports `ftCommon_8007F76C`; aliased once a check reaches it.
    pub fn speed_toward_facing(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let facing = reg(fp.facing_dir);
        fp.gr_vel = mem(fp::fmuls(facing, abs(reg(fp.gr_vel))));
        fp.self_vel.x = mem(fp::fmuls(facing, abs(reg(fp.self_vel.x))));
    }

    /// `ft_80085134`: the animation's TransN motion as the air velocity.
    #[doc(alias = "ft_80085134")]
    pub fn air_anim_motion(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.x6A4_transNOffset.z), reg(fp.facing_dir)));
        fp.self_vel.y = fp.x6A4_transNOffset.y;
    }
}

/// `ftCommon_8007F76C`, which states keep as a callback.
pub const SPEED_TOWARD_FACING: u32 = 0x8007_F76C;

/// A speed slowed by `friction`, stopping at 0.
fn toward_zero(v: f32, friction: f64) -> f32 {
    if v < 0.0 {
        let n = mem(fp::fadds(reg(v), friction));
        if n > 0.0 { 0.0 } else { n }
    } else {
        let n = mem(fp::fsubs(reg(v), friction));
        if n < 0.0 { 0.0 } else { n }
    }
}
