//! Fire Fox and Fire Bird (`ftfoxspecialhi.c`): the charge, the launch in the stick's
//! direction (along the floor on the ground), the bounce off walls and floors, and the end.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{GA_AIR, addr, attr, ext, facing_dir_int, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FTFOX_SPECIALHI_COLL_FLAG`: what the charge keeps between the ground and the air.
const COLL_FLAGS: u32 = KEEP_GFX
    | SKIP_MAT_ANIM
    | UPDATE_CMD
    | SKIP_COL_ANIM
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// `FtPart_TransN`, `FtPart_XRotN`, `FtPart_HipN`.
const PART_TRANS_N: i32 = 1;
const PART_X_ROT_N: i32 = 2;
const PART_HIP_N: i32 = 4;

/// `CLIFFCATCH_BOTH`.
const CLIFF_CATCH_BOTH: i32 = 0;

/// `Collide_FloorMask`, `Collide_CeilingMask`, `Collide_LeftWallMask`,
/// `Collide_RightWallMask`.
const FLOOR_MASK: i32 = 0x8000 | 0x1_0000;
const CEILING_MASK: i32 = 0x2000 | 0x4000;
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;

/// `HALF_PI32`.
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

/// `stickGetDir`: the stick's distance from `x2`.
fn stick_dir(x1: f64, x2: f64) -> f64 {
    if x1 < x2 { fp::fneg(x1) } else { x1 }
}

impl Running<'_> {
    /// An effect on a part's joint, once (`x2219_b0`), then the hitlag hooks and no more
    /// accessory: the charge's and launch's effects.
    fn fox_part_gfx(&mut self, gobj: GObjId, gfx: i32, part: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.part_joint(gobj, part);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The launch's flames, `ftFx_SpecialHi_CreateLaunchGFX`.
    #[doc(alias = "ftFx_SpecialHi_CreateLaunchGFX")]
    pub fn fox_special_hi_launch_gfx(&mut self, gobj: GObjId) {
        self.fox_part_gfx(gobj, 1164, PART_HIP_N);
    }

    /// The charge's flames, `ftFx_SpecialHi_CreateChargeGFX`.
    #[doc(alias = "ftFx_SpecialHi_CreateChargeGFX")]
    pub fn fox_special_hi_charge_gfx(&mut self, gobj: GObjId) {
        self.fox_part_gfx(gobj, 1163, PART_TRANS_N);
    }

    /// The grounded charge, `ftFx_SpecialHi_Enter`.
    #[doc(alias = "ftFx_SpecialHi_Enter")]
    pub fn fox_special_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let delay = fp::fctiwz(reg(self.fox_attr_f32(f, attr::FIREFOX_GRAVITY_DELAY)));
        let div = reg(self.fox_attr_f32(f, attr::FIREFOX_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_GRAVITY_DELAY, delay);
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), div));
        self.change_motion_state(gobj, ms::SPECIAL_HI_HOLD, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_CREATE_CHARGE_GFX));
    }

    /// The aerial charge, `ftFx_SpecialAirHiStart_Enter`.
    #[doc(alias = "ftFx_SpecialAirHiStart_Enter")]
    pub fn fox_special_air_hi_start_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let delay = fp::fctiwz(reg(self.fox_attr_f32(f, attr::FIREFOX_GRAVITY_DELAY)));
        let div = reg(self.fox_attr_f32(f, attr::FIREFOX_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_GRAVITY_DELAY, delay);
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_HI_HOLD_AIR, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_CREATE_CHARGE_GFX));
    }

    /// `ftFox_SpecialHi_RotateModel`: the body turned along the flight.
    fn fox_special_hi_rotate_model(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let part = self.bone_index(&self.game.fighters[f.0], PART_X_ROT_N);
        let rot = reg(self.game.fighters[f.0].mv_f32(mv::HI_ROTATE));
        let v = fp::fsubs(
            fp::fmuls(fp::frsp(2.0), fp::frsp(std::f64::consts::PI)),
            rot,
        );
        self.game.part_set_rot_x(f, part, mem(v));
    }

    /// The charges' end, `ftFx_SpecialHiHold_Anim` and `ftFx_SpecialHiHoldAir_Anim`: the
    /// launch, in the air or along the floor.
    #[doc(alias = "ftFx_SpecialHiHold_Anim")]
    #[doc(alias = "ftFx_SpecialHiHoldAir_Anim")]
    pub fn fox_special_hi_hold_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).ground_or_air == GA_AIR {
                self.fox_special_air_hi_enter(gobj);
                return;
            }
            self.fox_special_air_hi_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialHiHoldAir_Phys`: falling once gravity's delay is over.
    #[doc(alias = "ftFx_SpecialHiHoldAir_Phys")]
    pub fn fox_special_hi_hold_air_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.fox_count_down(gobj, mv::HI_GRAVITY_DELAY) {
            let g = reg(self.fox_attr_f32(f, attr::FIREFOX_FALL_ACCEL));
            let fp = &mut self.game.fighters[f.0];
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        let fr = reg(self.fox_attr_f32(f, attr::FIREFOX_AIR_MOMENTUM_PRESERVE_X));
        Self::self_accel_deaccel(&mut self.game.fighters[f.0], fr);
    }

    /// `ftFx_SpecialHiHold_Coll`.
    #[doc(alias = "ftFx_SpecialHiHold_Coll")]
    pub fn fox_special_hi_hold_coll(&mut self, gobj: GObjId) {
        if !self.ext_gobj_bool(ext::FT_80082708, gobj) {
            self.fox_special_hi_hold_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialHiHoldAir_Coll`.
    #[doc(alias = "ftFx_SpecialHiHoldAir_Coll")]
    pub fn fox_special_hi_hold_air_coll(&mut self, gobj: GObjId) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            self.fox_special_hi_hold_air_air_to_ground(gobj);
            return;
        }
        self.ext_gobj_bool(ext::FTCLIFFCOMMON_80081298, gobj);
    }

    /// `ftFx_SpecialHiHold_GroundToAir`.
    #[doc(alias = "ftFx_SpecialHiHold_GroundToAir")]
    pub fn fox_special_hi_hold_ground_to_air(&mut self, gobj: GObjId) {
        self.ext_fighter(ext::FTCOMMON_8007D60C, gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_HOLD_AIR,
            COLL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).accessory4_cb =
            Some(CodeRef(addr::SPECIAL_HI_CREATE_CHARGE_GFX));
    }

    /// `ftFx_SpecialHiHoldAir_AirToGround`.
    #[doc(alias = "ftFx_SpecialHiHoldAir_AirToGround")]
    pub fn fox_special_hi_hold_air_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI_HOLD, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb =
            Some(CodeRef(addr::SPECIAL_HI_CREATE_CHARGE_GFX));
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// The flights, `ftFx_SpecialHi_Anim` and `ftFx_SpecialAirHi_Anim`: the end when their
    /// frames run out.
    #[doc(alias = "ftFx_SpecialHi_Anim")]
    #[doc(alias = "ftFx_SpecialAirHi_Anim")]
    pub fn fox_special_hi_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_TRAVEL_FRAMES).wrapping_sub(1);
        fp.set_mv_i32(mv::HI_TRAVEL_FRAMES, n);
        if n <= 0 {
            if fp.ground_or_air == GA_AIR {
                self.fox_special_hi_landing_ground_to_air(gobj);
                return;
            }
            self.fox_special_hi_fall_air_to_ground(gobj);
        }
    }

    /// Counts the flight's frames (`unk`); whether its slowdown has started.
    fn fox_special_hi_slowing(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let end = self.fox_attr_f32(f, attr::FIREFOX_DURATION_END);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::HI_UNK).wrapping_add(1);
        fp.set_mv_i32(mv::HI_UNK, n);
        fp::frsp(f64::from(n)) >= reg(end)
    }

    /// `ftFx_SpecialHi_Phys`: the flight slows along the floor once it nears its end.
    #[doc(alias = "ftFx_SpecialHi_Phys")]
    pub fn fox_special_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.fox_special_hi_slowing(gobj) {
            let a = reg(self.fox_attr_f32(f, attr::FIREFOX_REVERSE_ACCEL));
            Self::ground_accel_deaccel(&mut self.game.fighters[f.0], a);
        }
        self.self_movement_from_ground(gobj);
    }

    /// `ftFx_SpecialAirHi_Phys`: the flight slows along its angle once it nears its end.
    #[doc(alias = "ftFx_SpecialAirHi_Phys")]
    pub fn fox_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.fox_special_hi_slowing(gobj) {
            let a = reg(self.fox_attr_f32(f, attr::FIREFOX_REVERSE_ACCEL));
            let fp = &mut self.game.fighters[f.0];
            let rot = fp.mv_f32(mv::HI_ROTATE);
            let c = reg(crate::math::cos(rot));
            fp.self_vel.x = mem(fp::fnmsubs(
                reg(fp.facing_dir),
                fp::fmuls(a, c),
                reg(fp.self_vel.x),
            ));
            let s = reg(crate::math::sin(rot));
            fp.self_vel.y = mem(fp::fnmsubs(a, s, reg(fp.self_vel.y)));
        }
    }

    /// `ftFx_SpecialHi_Coll`: off the ground into the air; on it, the body turned to the
    /// floor's slope.
    #[doc(alias = "ftFx_SpecialHi_Coll")]
    pub fn fox_special_hi_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_UNK2).wrapping_add(1);
        fp.set_mv_i32(mv::HI_UNK2, n);
        if !self.ext_gobj_bool(ext::FT_80082708, gobj) {
            self.fox_special_hi_ground_to_air(gobj);
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.coll_data.env_flags & FLOOR_MASK != 0 {
            let n = fp.coll_data.floor.normal;
            let r = crate::math::atan2(mem(fp::fmuls(fp::fneg(reg(n.x)), reg(fp.facing_dir))), n.y);
            fp.set_mv_f32(mv::HI_ROTATE, reg(r));
            self.fox_special_hi_rotate_model(gobj);
        }
    }

    /// `ftFox_SpecialHi_IsBound`.
    fn fox_special_hi_is_bound(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::HI_UNK2)
            >= self.fox_attr_i32(f, attr::FIREFOX_BOUNCE_VAR)
        {
            return true;
        }
        !self.ext_gobj_bool(ext::FTCO_8009A134, gobj)
    }

    /// `ftFx_SpecialAirHi_Coll`: a bounce off a floor hit head on, or a turn along a
    /// surface grazed (a floor, ceiling or wall within the bound angle).
    #[doc(alias = "ftFx_SpecialAirHi_Coll")]
    pub fn fox_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let bound = reg(self.fox_attr_f32(f, attr::FIREFOX_BOUND_ANGLE));
        let limit = || fp::fmuls(DEG_TO_RAD, fp::fadds(90.0, bound));
        let mut turn = false;
        if self.check_ground_and_ledge(gobj, CLIFF_CATCH_BOTH) && self.fox_special_hi_is_bound(gobj)
        {
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & FLOOR_MASK == 0
                || !(reg(c.floor.normal.angle_xy(fp.self_vel)) < limit())
            {
                self.fox_special_hi_bound_enter(gobj);
                return;
            }
            turn = true;
        }
        if !turn {
            if self.ext_gobj_bool(ext::FTCLIFFCOMMON_80081298, gobj) {
                return;
            }
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            let env = c.env_flags;
            let normal = if env & CEILING_MASK != 0 {
                c.ceiling.normal
            } else if env & LEFT_WALL_MASK != 0 {
                c.left_facing_wall.normal
            } else if env & RIGHT_WALL_MASK != 0 {
                c.right_facing_wall.normal
            } else {
                return;
            };
            if !(reg(normal.angle_xy(fp.self_vel)) < limit()) {
                return;
            }
        }
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = if fp.self_vel.x >= 0.0 { 1.0 } else { -1.0 };
        let r = crate::math::atan2(
            fp.self_vel.y,
            mem(fp::fmuls(reg(fp.self_vel.x), reg(fp.facing_dir))),
        );
        fp.set_mv_f32(mv::HI_ROTATE, reg(r));
        self.fox_special_hi_rotate_model(gobj);
    }

    /// `ftFx_SpecialHi_GroundToAir`.
    #[doc(alias = "ftFx_SpecialHi_GroundToAir")]
    pub fn fox_special_hi_ground_to_air(&mut self, gobj: GObjId) {
        self.ext_fighter(ext::FTCOMMON_8007D60C, gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI,
            SKIP_HIT | COLL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
        let fp = self.game.fighter_mut(gobj);
        fp.x2223_b4 = true;
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_CREATE_LAUNCH_GFX));
    }

    /// `ftFox_SpecialHi` launch's shared start: the flight's frames, counters cleared.
    fn fox_special_hi_launch_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = fp::fctiwz(reg(self.fox_attr_f32(f, attr::FIREFOX_DURATION)));
        let fp = &mut self.game.fighters[f.0];
        fp.x2223_b4 = true;
        fp.set_mv_i32(mv::HI_TRAVEL_FRAMES, frames);
        fp.set_mv_i32(mv::HI_UNK, 0);
        fp.set_mv_i32(mv::HI_UNK2, 0);
    }

    /// The charge ending on the ground, `ftFx_SpecialAirHi_AirToGround`: launching along
    /// the floor when the stick points away from it, else into the air.
    #[doc(alias = "ftFx_SpecialAirHi_AirToGround")]
    pub fn fox_special_air_hi_air_to_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let mut stick_y = reg(fp.input.lstick[0].y);
        if stick_y < 0.0 {
            stick_y = fp::fneg(stick_y);
        }
        let temp_stick = fp.input.lstick[0].x;
        let stick_x = stick_dir(reg(fp.input.lstick[0].x), 0.0);
        let min = reg(self.fox_attr_f32(f, attr::FIREFOX_DIRECTION_STICK_RANGE_MIN));
        if !(fp::fadds(stick_x, stick_y) < min) {
            let fp = &self.game.fighters[f.0];
            let dir = Vec3::new(temp_stick, fp.input.lstick[0].y, 0.0);
            if !(reg(fp.coll_data.floor.normal.angle_xy(dir)) < reg(HALF_PI))
                && !self.ext_gobj_bool(ext::FTCO_8009A134, gobj)
            {
                self.ext_fighter(ext::FTCOMMON_UPDATE_FACING, gobj);
                self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
                self.fox_special_hi_launch_vars(gobj);
                let speed = reg(self.fox_attr_f32(f, attr::FIREFOX_SPEED));
                let fp = &mut self.game.fighters[f.0];
                fp.gr_vel = mem(fp::fmuls(speed, reg(fp.facing_dir)));
                let n = fp.coll_data.floor.normal;
                let r =
                    crate::math::atan2(mem(fp::fmuls(fp::fneg(reg(n.x)), reg(fp.facing_dir))), n.y);
                fp.set_mv_f32(mv::HI_ROTATE, reg(r));
                self.fox_special_hi_rotate_model(gobj);
                let fp = &mut self.game.fighters[f.0];
                fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_CREATE_LAUNCH_GFX));
                fp.x21F8 = Some(CodeRef(ext::FTCOMMON_8007F76C));
                return;
            }
        }
        self.ext_fighter(ext::FTCOMMON_8007D60C, gobj);
        self.fox_special_air_hi_enter(gobj);
    }

    /// The aerial launch, `ftFx_SpecialAirHi_Enter`: toward the stick when it's held far
    /// enough, else straight up.
    #[doc(alias = "ftFx_SpecialAirHi_Enter")]
    pub fn fox_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let stick_y = stick_dir(reg(fp.input.lstick[0].y), 0.0);
        let mut temp_stick = reg(fp.input.lstick[0].x);
        let stick_x = stick_dir(temp_stick, 0.0);
        let min = reg(self.fox_attr_f32(f, attr::FIREFOX_DIRECTION_STICK_RANGE_MIN));
        if fp::fadds(stick_x, stick_y) >= min {
            if temp_stick < 0.0 {
                temp_stick = fp::fneg(temp_stick);
            }
            if temp_stick > reg(self.fox_attr_f32(f, attr::FIREFOX_FACING_STICK_RANGE_MIN)) {
                self.ext_fighter(ext::FTCOMMON_UPDATE_FACING, gobj);
            }
            let fp = &mut self.game.fighters[f.0];
            let s = &fp.input.lstick[0];
            let (sx, sy) = (s.x, s.y);
            let r = crate::math::atan2(sy, mem(fp::fmuls(reg(sx), reg(fp.facing_dir))));
            fp.set_mv_f32(mv::HI_ROTATE, reg(r));
        } else {
            self.game.fighters[f.0].set_mv_f32(mv::HI_ROTATE, reg(HALF_PI));
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.fox_special_hi_launch_vars(gobj);
        let speed = reg(self.fox_attr_f32(f, attr::FIREFOX_SPEED));
        let fp = &mut self.game.fighters[f.0];
        let rot = fp.mv_f32(mv::HI_ROTATE);
        let c = reg(crate::math::cos(rot));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(speed, c)));
        let s = reg(crate::math::sin(rot));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.fox_special_hi_rotate_model(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x21F8 = Some(CodeRef(ext::FTCOMMON_8007F76C));
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_CREATE_LAUNCH_GFX));
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// `ftFx_SpecialHiFall_Anim`: falling helplessly when it ends.
    #[doc(alias = "ftFx_SpecialHiFall_Anim")]
    pub fn fox_special_hi_fall_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (m, l) = self.fox_special_hi_fall_attrs(f);
        if !self.anim_frames_remaining(gobj) {
            self.fall_special(gobj, m, l);
        }
    }

    fn fox_special_hi_fall_attrs(&self, f: super::super::super::handles::FighterId) -> (f32, f32) {
        (
            self.fox_attr_f32(f, attr::FIREFOX_FREEFALL_MOBILITY),
            self.fox_attr_f32(f, attr::FIREFOX_LANDING_LAG),
        )
    }

    /// `ftFx_SpecialHiLanding_Phys`.
    #[doc(alias = "ftFx_SpecialHiLanding_Phys")]
    pub fn fox_special_hi_landing_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = reg(self.fox_attr_f32(f, attr::FIREFOX_GROUND_MOMENTUM_END));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], a);
        self.self_movement_from_ground(gobj);
    }

    /// `ftFx_SpecialHiLanding_Coll`: falling helplessly off the edge.
    #[doc(alias = "ftFx_SpecialHiLanding_Coll")]
    pub fn fox_special_hi_landing_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (m, l) = self.fox_special_hi_fall_attrs(f);
        if !self.ext_gobj_bool(ext::FT_80082708, gobj) {
            self.fall_special(gobj, m, l);
        }
    }

    /// `ftFx_SpecialHiFall_Coll`.
    #[doc(alias = "ftFx_SpecialHiFall_Coll")]
    pub fn fox_special_hi_fall_coll(&mut self, gobj: GObjId) {
        if self.check_ground_and_ledge(gobj, CLIFF_CATCH_BOTH) {
            self.fox_special_hi_fall_enter(gobj);
            return;
        }
        self.ext_gobj_bool(ext::FTCLIFFCOMMON_80081298, gobj);
    }

    /// Landing from the end's fall, `ftFx_SpecialHiFall_Enter`.
    #[doc(alias = "ftFx_SpecialHiFall_Enter")]
    pub fn fox_special_hi_fall_enter(&mut self, gobj: GObjId) {
        self.ext_fighter(ext::FTCOMMON_8007D7FC, gobj);
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_LANDING,
            SKIP_COL_ANIM | UPDATE_CMD,
            13.0,
            1.0,
            0.0,
            None,
        );
        self.anim_update(gobj);
    }

    /// The grounded flight's end, `ftFx_SpecialHiFall_AirToGround`.
    #[doc(alias = "ftFx_SpecialHiFall_AirToGround")]
    pub fn fox_special_hi_fall_air_to_ground(&mut self, gobj: GObjId) {
        self.ext_gobj(ext::FTCOMMON_8007DB24, gobj);
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.ext_fighter(ext::FTCOMMON_8007D7FC, gobj);
        }
        self.change_motion_state(gobj, ms::SPECIAL_HI_LANDING, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x21F8 = Some(CodeRef(ext::FTCOMMON_8007F76C));
    }

    /// The aerial flight's end, `ftFx_SpecialHiLanding_GroundToAir`.
    #[doc(alias = "ftFx_SpecialHiLanding_GroundToAir")]
    pub fn fox_special_hi_landing_ground_to_air(&mut self, gobj: GObjId) {
        self.ext_gobj(ext::FTCOMMON_8007DB24, gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI_FALL, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x21F8 = Some(CodeRef(ext::FTCOMMON_8007F76C));
    }

    /// The bounce, `ftFx_SpecialHiBound_Anim`: falling helplessly once the script says or
    /// the animation ends in the air, else waiting.
    #[doc(alias = "ftFx_SpecialHiBound_Anim")]
    pub fn fox_special_hi_bound_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (m, l) = self.fox_special_hi_fall_attrs(f);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 && fp.ground_or_air == GA_AIR {
            self.fall_special(gobj, m, l);
            let fp = &mut self.game.fighters[f.0];
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighters[f.0].ground_or_air == GA_AIR {
                self.fall_special(gobj, m, l);
                let fp = &mut self.game.fighters[f.0];
                fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
                return;
            }
            self.enter_wait(gobj);
        }
    }

    /// `ftFx_SpecialHiBound_Phys`.
    #[doc(alias = "ftFx_SpecialHiBound_Phys")]
    pub fn fox_special_hi_bound_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.ext_gobj(ext::FT_800851C0, gobj);
            self.ext_fighter(ext::FTCOMMON_CALC_SELF_ACCEL_DEACCEL_QUICK_AIR, gobj);
            return;
        }
        self.ground_friction(gobj);
    }

    /// `ftFx_SpecialHiBound_Coll`.
    #[doc(alias = "ftFx_SpecialHiBound_Coll")]
    pub fn fox_special_hi_bound_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
            if self.check_ground_and_ledge(gobj, dir) {
                self.ext_fighter(ext::FTCOMMON_8007D7FC, gobj);
                return;
            }
            self.ext_gobj_bool(ext::FTCLIFFCOMMON_80081298, gobj);
        } else {
            self.ground_coll(gobj);
        }
    }

    /// The bounce, `ftFx_SpecialHiBound_Enter`: slowed, with its dust facing the floor.
    #[doc(alias = "ftFx_SpecialHiBound_Enter")]
    pub fn fox_special_hi_bound_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI_BOUND, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let v = reg(self.fox_attr_f32(f, attr::FIREFOX_BOUND_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.x21F8 = Some(CodeRef(ext::FTCOMMON_8007F76C));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), v));
        fp.cmd_vars[0] = 0;
        // `ftFox_SpecialHiBound_SetVars`.
        let angle = if fp.coll_data.env_flags & FLOOR_MASK != 0 {
            let n = fp.coll_data.floor.normal;
            mem(fp::fneg(reg(crate::math::atan2(n.x, n.y))))
        } else {
            0.0
        };
        let pos = fp.cur_pos;
        spawn(self, 1030, Some(gobj), &[Arg::Vec(pos), Arg::Float(angle)]);
        self.game.fighters[f.0].x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_CREATE_LAUNCH_GFX => g.fox_special_hi_launch_gfx(gobj),
        addr::SPECIAL_HI_CREATE_CHARGE_GFX => g.fox_special_hi_charge_gfx(gobj),
        addr::SPECIAL_HI_ENTER => g.fox_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_START_ENTER => g.fox_special_air_hi_start_enter(gobj),
        0x800E_7354 | 0x800E_73B4 => g.fox_special_hi_hold_anim(gobj),
        0x800E_7414 | 0x800E_7418 | 0x800E_76D4 | 0x800E_76D8 | 0x800E_7ED8 | 0x800E_7EDC
        | 0x800E_81FC => g.fox_no_iasa(gobj),
        0x800E_741C => g.fox_ground_phys(gobj),
        0x800E_743C => g.fox_special_hi_hold_air_phys(gobj),
        0x800E_74A8 => g.fox_special_hi_hold_coll(gobj),
        0x800E_74E4 => g.fox_special_hi_hold_air_coll(gobj),
        0x800E_7554 => g.fox_special_hi_hold_ground_to_air(gobj),
        0x800E_75C0 => g.fox_special_hi_hold_air_air_to_ground(gobj),
        0x800E_7634 | 0x800E_7684 => g.fox_special_hi_anim(gobj),
        0x800E_76DC => g.fox_special_hi_phys(gobj),
        0x800E_7758 => g.fox_special_air_hi_phys(gobj),
        0x800E_7800 => g.fox_special_hi_coll(gobj),
        0x800E_78B4 => g.fox_special_air_hi_coll(gobj),
        0x800E_7A78 => g.fox_special_hi_ground_to_air(gobj),
        0x800E_7AF4 => g.fox_special_air_hi_air_to_ground(gobj),
        0x800E_7C98 => g.fox_special_air_hi_enter(gobj),
        0x800E_7E3C => g.fox_wait_when_done_anim(gobj),
        0x800E_7E78 => g.fox_special_hi_fall_anim(gobj),
        0x800E_7EE0 => g.fox_special_hi_landing_phys(gobj),
        0x800E_7F20 => g.fox_air_phys(gobj),
        0x800E_7F40 => g.fox_special_hi_landing_coll(gobj),
        0x800E_7FA0 => g.fox_special_hi_fall_coll(gobj),
        0x800E_7FF0 => g.fox_special_hi_fall_enter(gobj),
        0x800E_8048 => g.fox_special_hi_fall_air_to_ground(gobj),
        0x800E_80C0 => g.fox_special_hi_landing_ground_to_air(gobj),
        0x800E_8124 => g.fox_special_hi_bound_anim(gobj),
        0x800E_8200 => g.fox_special_hi_bound_phys(gobj),
        0x800E_824C => g.fox_special_hi_bound_coll(gobj),
        0x800E_82E4 => g.fox_special_hi_bound_enter(gobj),
        _ => return false,
    }
    true
}
