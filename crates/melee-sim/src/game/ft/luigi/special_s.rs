//! Luigi's Green Missile (`ftluigispecials.c`): a wind-up, a charge held with B (a smash
//! input starts it charged), then the launch, which may misfire, and the flight, which ends
//! on a hit, a wall or the ground.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, mv, part};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The flags each phase keeps going between the ground and the air.
const START_FLAGS: u32 = 0x0C4C_5284;
const HOLD_FLAGS: u32 = 0x0C4C_5286;
const LAUNCH_FLAGS: u32 = 0x0C4C_5086;
const FLY_FLAGS: u32 = 0x0C4C_508A;

/// B held.
const PAD_B: u32 = 1 << 9;

impl Running<'_> {
    /// `ftLg_SpecialS_SetGFX`: the effect flag up, and the effects paused in hitlag.
    #[doc(alias = "ftLg_SpecialS_SetGFX")]
    pub fn lg_special_s_set_gfx(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.x2219_b0 {
            fp.x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `ftLg_SpecialS_SetVars` (the wind-up's `x21EC`): charged from the start by a smash
    /// input (the stick's tilt timer below `greenmissile_smash`), and whether it misfires
    /// (`HSD_Randi`).
    #[doc(alias = "ftLg_SpecialS_SetVars")]
    pub fn lg_special_s_set_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let smash = reg(self.lg_attr_f32(f, attr::GREENMISSILE_SMASH));
        let rate = reg(self.lg_attr_f32(f, attr::GREENMISSILE_CHARGE_RATE));
        let chance = reg(self.lg_attr_f32(f, attr::GREENMISSILE_MISFIRE_CHANCE));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let timer = fp::frsp(f64::from(i32::from(fp.active_sticky.lstick.x)));
        if timer < smash {
            fp.set_mv_i32(mv::S_CHARGE_FRAMES, fp::fctiwz(rate));
            fp.x2070[2] |= 0x08;
        } else {
            fp.set_mv_i32(mv::S_CHARGE_FRAMES, 0);
        }
        let misfire = self.game.objs.rng.below(fp::fctiwz(chance)) == 0;
        self.game.fighters[f.0].set_mv_i32(mv::S_IS_MISFIRE, i32::from(misfire));
    }

    /// `ftLg_SpecialS_Enter`: the ground speed cut by `greenmissile_traction`, the wind-up.
    #[doc(alias = "ftLg_SpecialS_Enter")]
    pub fn lg_special_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t = reg(self.lg_attr_f32(f, attr::GREENMISSILE_TRACTION));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::SPECIAL_S_SET_VARS));
        fp.gr_vel = crate::fpu::mem(fp::fdivs(reg(fp.gr_vel), t));
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftLg_SpecialAirS_Enter`: the air speed cut, no vertical speed, the wind-up.
    #[doc(alias = "ftLg_SpecialAirS_Enter")]
    pub fn lg_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t = reg(self.lg_attr_f32(f, attr::GREENMISSILE_TRACTION));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::SPECIAL_S_SET_VARS));
        fp.self_vel.x = crate::fpu::mem(fp::fdivs(reg(fp.self_vel.x), t));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftLg_SpecialS_OnGiveDamage`: the flight stops (falling speed kept) and ends.
    #[doc(alias = "ftLg_SpecialS_OnGiveDamage")]
    pub fn lg_special_s_on_give_damage(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = 0.0;
        if reg(fp.self_vel.y) >= 0.0 {
            fp.self_vel.y = 0.0;
        }
        self.lg_special_air_s_end_enter(gobj);
    }

    /// `ftLg_SpecialSStart_Anim`.
    #[doc(alias = "ftLg_SpecialSStart_Anim")]
    pub fn lg_special_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.lg_special_s_hold_enter(gobj);
        }
    }

    /// `ftLg_SpecialAirSStart_Anim`.
    #[doc(alias = "ftLg_SpecialAirSStart_Anim")]
    pub fn lg_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.lg_special_air_s_hold_enter(gobj);
        }
    }

    /// `ftLg_SpecialSStart_Phys`: friction `greenmissile_unk2`.
    #[doc(alias = "ftLg_SpecialSStart_Phys")]
    pub fn lg_special_s_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.lg_attr_f32(f, attr::GREENMISSILE_UNK2));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftLg_SpecialAirSStart_Phys`: falling at `greenmissile_falling_speed` once the script
    /// says, and air friction `greenmissile_unk2`.
    #[doc(alias = "ftLg_SpecialAirSStart_Phys")]
    pub fn lg_special_air_s_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.lg_attr_f32(f, attr::GREENMISSILE_FALLING_SPEED));
        let friction = reg(self.lg_attr_f32(f, attr::GREENMISSILE_UNK2));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftLg_SpecialSStart_Coll`.
    #[doc(alias = "ftLg_SpecialSStart_Coll")]
    pub fn lg_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.lg_special_s_start_ground_to_air(gobj);
        }
    }

    /// `ftLg_SpecialSStart_GroundToAir`.
    #[doc(alias = "ftLg_SpecialSStart_GroundToAir")]
    pub fn lg_special_s_start_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START, START_FLAGS);
    }

    /// `ftLg_SpecialAirSStart_Coll`.
    #[doc(alias = "ftLg_SpecialAirSStart_Coll")]
    pub fn lg_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.lg_special_air_s_start_air_to_ground(gobj);
        }
    }

    /// `ftLg_SpecialAirSStart_AirToGround`.
    #[doc(alias = "ftLg_SpecialAirSStart_AirToGround")]
    pub fn lg_special_air_s_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, START_FLAGS);
    }

    /// The charge's frame: the effects go and the effect callback comes back when its loop
    /// ends, one more frame of charge, and the launch past `greenmissile_max_charge_frames`.
    fn lg_special_s_hold_tick(&mut self, gobj: GObjId) -> bool {
        if !self.anim_frames_remaining(gobj) {
            self.destroy_effects(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_SET_GFX));
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.lg_attr_f32(f, attr::GREENMISSILE_MAX_CHARGE_FRAMES));
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::S_CHARGE_FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::S_CHARGE_FRAMES, n);
        fp::frsp(f64::from(n)) > max
    }

    /// `ftLg_SpecialSHold_Anim`.
    #[doc(alias = "ftLg_SpecialSHold_Anim")]
    pub fn lg_special_s_hold_anim(&mut self, gobj: GObjId) {
        if self.lg_special_s_hold_tick(gobj) {
            self.lg_special_s_launch_enter(gobj);
        }
    }

    /// `ftLg_SpecialAirSHold_Anim`.
    #[doc(alias = "ftLg_SpecialAirSHold_Anim")]
    pub fn lg_special_air_s_hold_anim(&mut self, gobj: GObjId) {
        if self.lg_special_s_hold_tick(gobj) {
            self.lg_special_air_s_launch_enter(gobj);
        }
    }

    /// `ftLg_SpecialSHold_IASA`: B let go, the launch.
    #[doc(alias = "ftLg_SpecialSHold_IASA")]
    pub fn lg_special_s_hold_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.lg_special_s_launch_enter(gobj);
        }
    }

    /// `ftLg_SpecialAirSHold_IASA`.
    #[doc(alias = "ftLg_SpecialAirSHold_IASA")]
    pub fn lg_special_air_s_hold_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.lg_special_air_s_launch_enter(gobj);
        }
    }

    /// `ftLg_SpecialSHold_Coll`.
    #[doc(alias = "ftLg_SpecialSHold_Coll")]
    pub fn lg_special_s_hold_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.lg_special_s_hold_ground_to_air(gobj);
        }
    }

    /// `ftLg_SpecialSHold_GroundToAir`.
    #[doc(alias = "ftLg_SpecialSHold_GroundToAir")]
    pub fn lg_special_s_hold_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_HOLD, HOLD_FLAGS);
    }

    /// `ftLg_SpecialAirSHold_Coll`.
    #[doc(alias = "ftLg_SpecialAirSHold_Coll")]
    pub fn lg_special_air_s_hold_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.lg_special_air_s_hold_air_to_ground(gobj);
        }
    }

    /// `ftLg_SpecialAirSHold_AirToGround`.
    #[doc(alias = "ftLg_SpecialAirSHold_AirToGround")]
    pub fn lg_special_air_s_hold_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_HOLD, HOLD_FLAGS);
    }

    /// `ftLg_SpecialSHold_Enter`: the charge, with its effect callback.
    #[doc(alias = "ftLg_SpecialSHold_Enter")]
    pub fn lg_special_s_hold_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S_HOLD, 0x200, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_SET_GFX));
    }

    /// `ftLg_SpecialAirSHold_Enter`.
    #[doc(alias = "ftLg_SpecialAirSHold_Enter")]
    pub fn lg_special_air_s_hold_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_HOLD, 0x200, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_SET_GFX));
    }

    /// The launch's frame: unless it misfires, the hitbox's damage grows with the charge
    /// (`greenmissile_damage_tilt` plus `_slope` per frame); when the script says, the flight.
    fn lg_special_s_launch_tick(&mut self, gobj: GObjId, airborne_first: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(mv::S_IS_MISFIRE) == 0 && fp.x914[0].state == hitcapsule::ENABLED {
            let charge = fp::frsp(f64::from(fp.mv_i32(mv::S_CHARGE_FRAMES)));
            let slope = reg(self.lg_attr_f32(f, attr::GREENMISSILE_DAMAGE_SLOPE));
            let tilt = reg(self.lg_attr_f32(f, attr::GREENMISSILE_DAMAGE_TILT));
            let dmg = crate::ppc::cvt_fp2unsigned(fp::fmadds(charge, slope, tilt));
            self.hitbox_damage(f, 0, false, dmg);
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            if airborne_first {
                self.go_airborne(f);
            }
            self.lg_special_s_fly_enter(gobj);
        }
    }

    /// `ftLg_SpecialS_Anim`: the launch's damage, then airborne into the flight.
    #[doc(alias = "ftLg_SpecialS_Anim")]
    pub fn lg_special_s_anim(&mut self, gobj: GObjId) {
        self.lg_special_s_launch_tick(gobj, true);
    }

    /// `ftLg_SpecialAirS_Anim`.
    #[doc(alias = "ftLg_SpecialAirS_Anim")]
    pub fn lg_special_air_s_anim(&mut self, gobj: GObjId) {
        self.lg_special_s_launch_tick(gobj, false);
    }

    /// `ftLg_SpecialS_Coll`.
    #[doc(alias = "ftLg_SpecialS_Coll")]
    pub fn lg_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.lg_special_s_launch_ground_to_air(gobj);
        }
    }

    /// `ftLg_SpecialSLaunch_GroundToAir`.
    #[doc(alias = "ftLg_SpecialSLaunch_GroundToAir")]
    pub fn lg_special_s_launch_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S, LAUNCH_FLAGS);
    }

    /// `ftLg_SpecialAirS_Coll`.
    #[doc(alias = "ftLg_SpecialAirS_Coll")]
    pub fn lg_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.lg_special_air_s_launch_air_to_ground(gobj);
        }
    }

    /// `ftLg_SpecialAirSLaunch_AirToGround`.
    #[doc(alias = "ftLg_SpecialAirSLaunch_AirToGround")]
    pub fn lg_special_air_s_launch_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S, LAUNCH_FLAGS);
    }

    /// The launch's or misfire's start (`ftLuigi_SpecialS_RemoveGFX`, `_Setup`): the charge's
    /// effects go, the launch's effect from the hip, the effects paused in hitlag, no effect
    /// callback.
    fn lg_special_s_launch_setup(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.destroy_effects(gobj);
        if !self.game.fighter(gobj).x2219_b0 {
            let hip = self.bone_joint(gobj, part::HIP_N);
            spawn(self, 0x50A, Some(gobj), &[Arg::JObj(hip)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftLg_SpecialSLaunch_Enter`: the misfire if it's one, else the launch.
    #[doc(alias = "ftLg_SpecialSLaunch_Enter")]
    pub fn lg_special_s_launch_enter(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(mv::S_IS_MISFIRE) != 0 {
            self.lg_special_s_misfire_enter(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::SPECIAL_S, 0, 0.0, 1.0, 0.0, None);
        self.lg_special_s_launch_setup(gobj);
    }

    /// `ftLg_SpecialAirSLaunch_Enter`.
    #[doc(alias = "ftLg_SpecialAirSLaunch_Enter")]
    pub fn lg_special_air_s_launch_enter(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(mv::S_IS_MISFIRE) != 0 {
            self.lg_special_air_s_misfire_enter(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, 0, 0.0, 1.0, 0.0, None);
        self.lg_special_s_launch_setup(gobj);
    }

    /// `ftLg_SpecialSMisfire_Anim`: as the launch's.
    #[doc(alias = "ftLg_SpecialSMisfire_Anim")]
    pub fn lg_special_s_misfire_anim(&mut self, gobj: GObjId) {
        self.lg_special_s_launch_tick(gobj, true);
    }

    /// `ftLg_SpecialAirSMisfire_Anim`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_Anim")]
    pub fn lg_special_air_s_misfire_anim(&mut self, gobj: GObjId) {
        self.lg_special_s_launch_tick(gobj, false);
    }

    /// `ftLg_SpecialSMisfire_Coll`.
    #[doc(alias = "ftLg_SpecialSMisfire_Coll")]
    pub fn lg_special_s_misfire_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.lg_special_s_misfire_ground_to_air(gobj);
        }
    }

    /// `ftLg_SpecialSMisfire_GroundToAir`.
    #[doc(alias = "ftLg_SpecialSMisfire_GroundToAir")]
    pub fn lg_special_s_misfire_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_MISFIRE, LAUNCH_FLAGS);
    }

    /// `ftLg_SpecialAirSMisfire_Coll`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_Coll")]
    pub fn lg_special_air_s_misfire_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.lg_special_air_s_misfire_air_to_ground(gobj);
        }
    }

    /// `ftLg_SpecialAirSMisfire_AirToGround`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_AirToGround")]
    pub fn lg_special_air_s_misfire_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_MISFIRE, LAUNCH_FLAGS);
    }

    /// `ftLg_SpecialSMisfire_Enter`.
    #[doc(alias = "ftLg_SpecialSMisfire_Enter")]
    pub fn lg_special_s_misfire_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S_MISFIRE, 0, 0.0, 1.0, 0.0, None);
        self.lg_special_s_launch_setup(gobj);
    }

    /// `ftLg_SpecialAirSMisfire_Enter`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_Enter")]
    pub fn lg_special_air_s_misfire_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_MISFIRE, 0, 0.0, 1.0, 0.0, None);
        self.lg_special_s_launch_setup(gobj);
    }

    /// `ftLg_SpecialAirS2_Anim`: the end when the flight's animation ends.
    #[doc(alias = "ftLg_SpecialAirS2_Anim")]
    pub fn lg_special_air_s2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.lg_special_air_s_end_enter(gobj);
        }
    }

    /// `ftLg_SpecialAirS2_Phys`: falling at `greenmissile_mul_y` up to
    /// `greenmissile_gravity_start`, at `_gravity_mul` once the script says, which also
    /// starts air friction `greenmissile_x_decel`.
    #[doc(alias = "ftLg_SpecialAirS2_Phys")]
    pub fn lg_special_air_s2_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let start = reg(self.lg_attr_f32(f, attr::GREENMISSILE_GRAVITY_START));
        let mul = reg(self.lg_attr_f32(f, attr::GREENMISSILE_GRAVITY_MUL));
        let mul_y = reg(self.lg_attr_f32(f, attr::GREENMISSILE_MUL_Y));
        let decel = reg(self.lg_attr_f32(f, attr::GREENMISSILE_X_DECEL));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            Self::fall(fp, mul, start);
        } else {
            Self::fall(fp, mul_y, start);
        }
        if fp.cmd_vars[0] != 0 {
            Self::self_accel_deaccel(fp, decel);
        }
    }

    /// `ftLg_SpecialAirS2_Coll`: landing, the grounded end; touching any wall or ceiling
    /// (`env_flags`), the aerial end.
    #[doc(alias = "ftLg_SpecialAirS2_Coll")]
    pub fn lg_special_air_s2_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.lg_special_s_end_enter(gobj);
        }
        let env = self.game.fighter(gobj).coll_data.env_flags;
        if env & 0x3F != 0 || env & 0xFC0 != 0 {
            self.lg_special_air_s_end_enter(gobj);
        }
    }

    /// `ftLg_SpecialSFly_Enter`: the flight's speed, ahead at `greenmissile_vel_x` plus
    /// `_mul_x` per frame of charge and up by half of `_vel_y` and more with the charge, or a
    /// misfire's own speeds; it ends on a hit (`ftLg_SpecialS_OnGiveDamage`).
    #[doc(alias = "ftLg_SpecialSFly_Enter")]
    pub fn lg_special_s_fly_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let misfire_x = self.lg_attr_f32(f, attr::GREENMISSILE_MISFIRE_VEL_X);
        let misfire_y = self.lg_attr_f32(f, attr::GREENMISSILE_MISFIRE_VEL_Y);
        let mul_x = reg(self.lg_attr_f32(f, attr::GREENMISSILE_MUL_X));
        let vel_x = reg(self.lg_attr_f32(f, attr::GREENMISSILE_VEL_X));
        let vel_y = reg(self.lg_attr_f32(f, attr::GREENMISSILE_VEL_Y));
        let max = reg(self.lg_attr_f32(f, attr::GREENMISSILE_MAX_CHARGE_FRAMES));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let misfire = fp.mv_i32(mv::S_IS_MISFIRE) != 0;
        let charge = fp::frsp(f64::from(fp.mv_i32(mv::S_CHARGE_FRAMES)));
        if misfire {
            fp.self_vel.x = misfire_x;
        } else {
            fp.self_vel.x = crate::fpu::mem(fp::fmadds(mul_x, charge, vel_x));
        }
        fp.self_vel.x = crate::fpu::mem(fp::fmuls(reg(fp.self_vel.x), reg(fp.facing_dir)));
        if misfire {
            fp.self_vel.y = misfire_y;
        } else {
            let t = fp::fdivs(fp::fmuls(0.5, charge), max);
            fp.self_vel.y = crate::fpu::mem(fp::fmadds(0.5, vel_y, fp::fmuls(vel_y, t)));
        }
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S2, FLY_FLAGS, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x21F8 = Some(CodeRef(super::super::phys::SPEED_TOWARD_FACING));
        fp.deal_dmg_cb = Some(CodeRef(addr::SPECIAL_S_ON_GIVE_DAMAGE));
    }

    /// `ftLg_SpecialSEnd_Anim`.
    #[doc(alias = "ftLg_SpecialSEnd_Anim")]
    pub fn lg_special_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftLg_SpecialAirSEnd_Anim`.
    #[doc(alias = "ftLg_SpecialAirSEnd_Anim")]
    pub fn lg_special_air_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftLg_SpecialSEnd_Phys`: friction `greenmissile_x_decel`.
    #[doc(alias = "ftLg_SpecialSEnd_Phys")]
    pub fn lg_special_s_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let decel = reg(self.lg_attr_f32(f, attr::GREENMISSILE_X_DECEL));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], decel);
        self.self_movement_from_ground(gobj);
    }

    /// `ftLg_SpecialAirSEnd_Phys`: falling at `greenmissile_gravity_mul`, air friction
    /// `greenmissile_x_decel`.
    #[doc(alias = "ftLg_SpecialAirSEnd_Phys")]
    pub fn lg_special_air_s_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.lg_attr_f32(f, attr::GREENMISSILE_GRAVITY_MUL));
        let decel = reg(self.lg_attr_f32(f, attr::GREENMISSILE_X_DECEL));
        let fp = &mut self.game.fighters[f.0];
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, decel);
    }

    /// `ftLg_SpecialSEnd_Coll`: off a ledge, a fall.
    #[doc(alias = "ftLg_SpecialSEnd_Coll")]
    pub fn lg_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftLg_SpecialAirSEnd_Coll`: landing, the grounded end.
    #[doc(alias = "ftLg_SpecialAirSEnd_Coll")]
    pub fn lg_special_air_s_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.lg_special_s_end_enter(gobj);
        }
    }

    /// `ftLg_SpecialSEnd_Enter`: the ground speed cut by `greenmissile_friction_end`.
    #[doc(alias = "ftLg_SpecialSEnd_Enter")]
    pub fn lg_special_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let end = reg(self.lg_attr_f32(f, attr::GREENMISSILE_FRICTION_END));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.gr_vel = crate::fpu::mem(fp::fdivs(reg(fp.gr_vel), end));
        self.change_motion_state(gobj, ms::SPECIAL_S_END, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftLg_SpecialAirSEnd_Enter`: the air speed cut by `greenmissile_friction_end`.
    #[doc(alias = "ftLg_SpecialAirSEnd_Enter")]
    pub fn lg_special_air_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let end = reg(self.lg_attr_f32(f, attr::GREENMISSILE_FRICTION_END));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = crate::fpu::mem(fp::fdivs(reg(fp.self_vel.x), end));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_END, 0, 0.0, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_SET_GFX => g.lg_special_s_set_gfx(gobj),
        addr::SPECIAL_S_START_GROUND_TO_AIR => g.lg_special_s_start_ground_to_air(gobj),
        addr::SPECIAL_AIR_S_START_AIR_TO_GROUND => g.lg_special_air_s_start_air_to_ground(gobj),
        addr::SPECIAL_S_HOLD_GROUND_TO_AIR => g.lg_special_s_hold_ground_to_air(gobj),
        addr::SPECIAL_AIR_S_HOLD_AIR_TO_GROUND => g.lg_special_air_s_hold_air_to_ground(gobj),
        addr::SPECIAL_S_HOLD_ENTER => g.lg_special_s_hold_enter(gobj),
        addr::SPECIAL_AIR_S_HOLD_ENTER => g.lg_special_air_s_hold_enter(gobj),
        addr::SPECIAL_S_LAUNCH_GROUND_TO_AIR => g.lg_special_s_launch_ground_to_air(gobj),
        addr::SPECIAL_AIR_S_LAUNCH_AIR_TO_GROUND => g.lg_special_air_s_launch_air_to_ground(gobj),
        addr::SPECIAL_S_LAUNCH_ENTER => g.lg_special_s_launch_enter(gobj),
        addr::SPECIAL_AIR_S_LAUNCH_ENTER => g.lg_special_air_s_launch_enter(gobj),
        addr::SPECIAL_S_MISFIRE_GROUND_TO_AIR => g.lg_special_s_misfire_ground_to_air(gobj),
        addr::SPECIAL_AIR_S_MISFIRE_AIR_TO_GROUND => g.lg_special_air_s_misfire_air_to_ground(gobj),
        addr::SPECIAL_S_MISFIRE_ENTER => g.lg_special_s_misfire_enter(gobj),
        addr::SPECIAL_AIR_S_MISFIRE_ENTER => g.lg_special_air_s_misfire_enter(gobj),
        addr::SPECIAL_S_FLY_ENTER => g.lg_special_s_fly_enter(gobj),
        addr::SPECIAL_S_END_ENTER => g.lg_special_s_end_enter(gobj),
        addr::SPECIAL_AIR_S_END_ENTER => g.lg_special_air_s_end_enter(gobj),
        addr::SPECIAL_S_SET_VARS => g.lg_special_s_set_vars(gobj),
        addr::SPECIAL_S_ENTER => g.lg_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.lg_special_air_s_enter(gobj),
        addr::SPECIAL_S_ON_GIVE_DAMAGE => g.lg_special_s_on_give_damage(gobj),
        addr::SPECIAL_S_START_ANIM => g.lg_special_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_START_ANIM => g.lg_special_air_s_start_anim(gobj),
        addr::SPECIAL_S_START_IASA
        | addr::SPECIAL_AIR_S_START_IASA
        | addr::SPECIAL_S_IASA
        | addr::SPECIAL_AIR_S_IASA
        | addr::SPECIAL_S_MISFIRE_IASA
        | addr::SPECIAL_AIR_S_MISFIRE_IASA
        | addr::SPECIAL_S2_ANIM
        | addr::SPECIAL_S2_IASA
        | addr::SPECIAL_AIR_S2_IASA
        | addr::SPECIAL_S2_PHYS
        | addr::SPECIAL_S2_COLL
        | addr::SPECIAL_S_END_IASA
        | addr::SPECIAL_AIR_S_END_IASA => g.lg_nothing(gobj),
        addr::SPECIAL_S_START_PHYS => g.lg_special_s_start_phys(gobj),
        addr::SPECIAL_AIR_S_START_PHYS => g.lg_special_air_s_start_phys(gobj),
        addr::SPECIAL_S_START_COLL => g.lg_special_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_START_COLL => g.lg_special_air_s_start_coll(gobj),
        addr::SPECIAL_S_HOLD_ANIM => g.lg_special_s_hold_anim(gobj),
        addr::SPECIAL_AIR_S_HOLD_ANIM => g.lg_special_air_s_hold_anim(gobj),
        addr::SPECIAL_S_HOLD_IASA => g.lg_special_s_hold_iasa(gobj),
        addr::SPECIAL_AIR_S_HOLD_IASA => g.lg_special_air_s_hold_iasa(gobj),
        addr::SPECIAL_S_HOLD_PHYS | addr::SPECIAL_S_PHYS => g.lg_ground_friction(gobj),
        addr::SPECIAL_AIR_S_HOLD_PHYS | addr::SPECIAL_AIR_S_PHYS => g.lg_air_physics_no_drift(gobj),
        addr::SPECIAL_S_HOLD_COLL => g.lg_special_s_hold_coll(gobj),
        addr::SPECIAL_AIR_S_HOLD_COLL => g.lg_special_air_s_hold_coll(gobj),
        addr::SPECIAL_S_ANIM => g.lg_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.lg_special_air_s_anim(gobj),
        addr::SPECIAL_S_COLL => g.lg_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.lg_special_air_s_coll(gobj),
        addr::SPECIAL_S_MISFIRE_ANIM => g.lg_special_s_misfire_anim(gobj),
        addr::SPECIAL_AIR_S_MISFIRE_ANIM => g.lg_special_air_s_misfire_anim(gobj),
        addr::SPECIAL_S_MISFIRE_PHYS | addr::SPECIAL_AIR_S_MISFIRE_PHYS => {
            g.lg_ground_friction(gobj)
        }
        addr::SPECIAL_S_MISFIRE_COLL => g.lg_special_s_misfire_coll(gobj),
        addr::SPECIAL_AIR_S_MISFIRE_COLL => g.lg_special_air_s_misfire_coll(gobj),
        addr::SPECIAL_AIR_S2_ANIM => g.lg_special_air_s2_anim(gobj),
        addr::SPECIAL_AIR_S2_PHYS => g.lg_special_air_s2_phys(gobj),
        addr::SPECIAL_AIR_S2_COLL => g.lg_special_air_s2_coll(gobj),
        addr::SPECIAL_S_END_ANIM => g.lg_special_s_end_anim(gobj),
        addr::SPECIAL_AIR_S_END_ANIM => g.lg_special_air_s_end_anim(gobj),
        addr::SPECIAL_S_END_PHYS => g.lg_special_s_end_phys(gobj),
        addr::SPECIAL_AIR_S_END_PHYS => g.lg_special_air_s_end_phys(gobj),
        addr::SPECIAL_S_END_COLL => g.lg_special_s_end_coll(gobj),
        addr::SPECIAL_AIR_S_END_COLL => g.lg_special_air_s_end_coll(gobj),
        _ => return false,
    }
    true
}
