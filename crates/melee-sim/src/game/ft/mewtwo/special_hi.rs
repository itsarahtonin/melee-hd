//! Teleport (`ftmewtwospecialhi.c`): the wind-up, the invisible travel in the stick's
//! direction (along the floor on the ground), and the reappearance, as Zelda's Farore's Wind
//! with his own effect and sound.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::super::motion::flags::*;
use super::{PART_WAIST_N, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// The travel's flags between the ground and the air (`transition_flags0`), and with the
/// color animation's hit status kept (`transition_flags1`).
const TRANSITION_FLAGS_0: u32 = KEEP_GFX
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const TRANSITION_FLAGS_1: u32 = TRANSITION_FLAGS_0 | KEEP_COL_ANIM_HIT_STATUS;

/// `Collide_LeftWallMask`, `Collide_RightWallMask`, `Collide_CeilingMask`.
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;
const CEILING_MASK: i32 = 0x2000 | 0x4000;

/// The wind-up's effect, and the travel's sound.
const GFX_START: i32 = 0x4E8;
const SFX_TRAVEL: i32 = 0x30DA1;

/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

impl Running<'_> {
    /// The waist's position in the world (`lb_8000B1CC`), which also brings its matrix up to
    /// date.
    fn mt_waist_pos(&mut self, gobj: GObjId) -> crate::vec::Vec3 {
        let j = self.bone_joint(gobj, PART_WAIST_N);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// `ftMt_SpecialHi_CreateGFX` (an accessory): the wind-up's effect, then no more
    /// accessory.
    #[doc(alias = "ftMt_SpecialHi_CreateGFX")]
    pub fn mt_hi_start_gfx_once(&mut self, gobj: GObjId) {
        self.mt_hi_start_gfx(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The wind-up's effect at his waist, once (`x2219_b0`), `ftMt_SpecialHi_SetStartGFX`.
    #[doc(alias = "ftMt_SpecialHi_SetStartGFX")]
    pub fn mt_hi_start_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let pos = self.mt_waist_pos(gobj);
            spawn(self, GFX_START, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The reappearance's accessory, `ftMt_SpecialHi_SetEndGFX`: his waist's position taken
    /// once (and no effect made), then no more accessory.
    #[doc(alias = "ftMt_SpecialHi_SetEndGFX")]
    pub fn mt_hi_end_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let _ = self.mt_waist_pos(gobj);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The grounded wind-up, `ftMt_SpecialHiStart_Enter`: stopped.
    #[doc(alias = "ftMt_SpecialHiStart_Enter")]
    pub fn mt_special_hi_start_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_HI_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_AIR_FRAMES, 0);
        fp.accessory4_cb = Some(CodeRef(addr::HI_CREATE_GFX));
    }

    /// The aerial wind-up, `ftMt_SpecialAirHiStart_Enter`: the speeds divided down (`x40`,
    /// `x44`).
    #[doc(alias = "ftMt_SpecialAirHiStart_Enter")]
    pub fn mt_special_air_hi_start_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dx = reg(self.mt_attr_f32(f, attr::HI_VEL_DIV_X));
        let dy = reg(self.mt_attr_f32(f, attr::HI_VEL_DIV_Y));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), dx));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), dy));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_AIR_FRAMES, 0);
        fp.accessory4_cb = Some(CodeRef(addr::HI_CREATE_GFX));
    }

    /// `ftMt_SpecialHiStart_Anim`: the travel along the floor once the wind-up ends.
    #[doc(alias = "ftMt_SpecialHiStart_Anim")]
    pub fn mt_special_hi_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mt_hi_travel_ground(gobj);
        }
    }

    /// `ftMt_SpecialAirHiStart_Anim`: the travel through the air.
    #[doc(alias = "ftMt_SpecialAirHiStart_Anim")]
    pub fn mt_special_air_hi_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mt_hi_travel_air(gobj);
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftMt_SpecialHiStart_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiStart_IASA")]
    #[doc(alias = "ftMt_SpecialHiLost_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiLost_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiLost_Phys")]
    #[doc(alias = "ftMt_SpecialHi_IASA")]
    #[doc(alias = "ftMt_SpecialAirHi_IASA")]
    pub fn mt_special_hi_nothing(&mut self, _gobj: GObjId) {}

    /// `ftMt_SpecialAirHiStart_Phys`: the wind-up's own fall (`x48`, `x4C`), slowing.
    #[doc(alias = "ftMt_SpecialAirHiStart_Phys")]
    pub fn mt_special_air_hi_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.mt_attr_f32(f, attr::HI_GRAVITY));
        let t = reg(self.mt_attr_f32(f, attr::HI_TERMINAL_VEL));
        Self::fall(&mut self.game.fighters[f.0], g, t);
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftMt_SpecialHiStart_Coll`: off the ground, the aerial wind-up.
    #[doc(alias = "ftMt_SpecialHiStart_Coll")]
    pub fn mt_special_hi_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mt_special_hi_start_to_air(gobj);
        }
    }

    /// `ftMt_SpecialAirHiStart_Coll`: landing, the grounded wind-up; else a ledge.
    #[doc(alias = "ftMt_SpecialAirHiStart_Coll")]
    pub fn mt_special_air_hi_start_coll(&mut self, gobj: GObjId) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            self.mt_special_air_hi_start_to_ground(gobj);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// `ftMt_SpecialHiStart_GroundToAir`.
    #[doc(alias = "ftMt_SpecialHiStart_GroundToAir")]
    pub fn mt_special_hi_start_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START,
            TRANSITION_FLAGS_1,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_CREATE_GFX));
    }

    /// `ftMt_SpecialAirHiStart_AirToGround`.
    #[doc(alias = "ftMt_SpecialAirHiStart_AirToGround")]
    pub fn mt_special_air_hi_start_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI_START, TRANSITION_FLAGS_1);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_CREATE_GFX));
    }

    /// The travel's countdown (`travelFrames`); whether it ran out.
    fn mt_hi_count_down(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_FRAMES).wrapping_sub(1);
        fp.set_mv_i32(mv::HI_FRAMES, n);
        n <= 0
    }

    /// `ftMt_SpecialHiLost_Anim`: the reappearance on the ground once the travel ends.
    #[doc(alias = "ftMt_SpecialHiLost_Anim")]
    pub fn mt_special_hi_lost_anim(&mut self, gobj: GObjId) {
        if self.mt_hi_count_down(gobj) {
            self.mt_hi_appear_ground(gobj);
        }
    }

    /// `ftMt_SpecialAirHiLost_Anim`: the reappearance in the air.
    #[doc(alias = "ftMt_SpecialAirHiLost_Anim")]
    pub fn mt_special_air_hi_lost_anim(&mut self, gobj: GObjId) {
        if self.mt_hi_count_down(gobj) {
            self.mt_hi_appear_air(gobj);
        }
    }

    /// `ftMt_SpecialHiLost_Phys`: the travel along the floor.
    #[doc(alias = "ftMt_SpecialHiLost_Phys")]
    pub fn mt_special_hi_lost_phys(&mut self, gobj: GObjId) {
        self.self_movement_from_ground(gobj);
    }

    /// `ftMt_SpecialHiLost_Coll`: off the floor, the travel goes on in the air (or he
    /// reappears there against a wall); along it, he reappears against a wall.
    #[doc(alias = "ftMt_SpecialHiLost_Coll")]
    pub fn mt_special_hi_lost_coll(&mut self, gobj: GObjId) {
        let walls = |r: &Running| {
            r.game.fighter(gobj).coll_data.env_flags & (LEFT_WALL_MASK | RIGHT_WALL_MASK) != 0
        };
        if !self.ground_coll_off_ledges(gobj) {
            if walls(self) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne_jumped(f);
                self.mt_hi_appear_air(gobj);
                return;
            }
            self.mt_hi_travel_to_air(gobj);
            return;
        }
        if walls(self) {
            self.mt_hi_appear_ground(gobj);
        }
    }

    /// `ftMt_SpecialAirHiLost_Coll`: meeting the ground (after `x54` frames, or where he
    /// can't pass through), the travel goes on along it; else a ledge, or reappearing against
    /// a surface met head on (`ftCommon_HandleTeleportCollisions`).
    #[doc(alias = "ftMt_SpecialAirHiLost_Coll")]
    pub fn mt_special_air_hi_lost_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::HI_AIR_FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::HI_AIR_FRAMES, n);
        let dir = facing_dir_int(fp.facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let frames = self.game.fighters[f.0].mv_i32(mv::HI_AIR_FRAMES);
            let land =
                if fp::frsp(f64::from(frames)) >= reg(self.mt_attr_f32(f, attr::HI_LAND_FRAMES)) {
                    true
                } else {
                    !self.pass_skip_floor(gobj)
                };
            if land {
                self.mt_hi_travel_to_ground(gobj);
                return;
            }
        }
        if !self.catch_ledge_check(gobj) {
            self.mt_teleport_collisions(gobj);
        }
    }

    /// `ftCommon_HandleTeleportCollisions` with the attributes' angle (`x68`): he reappears
    /// for each of the ceiling and walls he meets more head on than that.
    fn mt_teleport_collisions(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let clamp = self.mt_attr_i32(f, attr::HI_ANGLE_CLAMP);
        let limit = fp::fmuls(DEG_TO_RAD, fp::fadds(90.0, fp::frsp(f64::from(clamp))));
        let checks = [(CEILING_MASK, 0), (LEFT_WALL_MASK, 1), (RIGHT_WALL_MASK, 2)];
        for (mask, which) in checks {
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & mask == 0 {
                continue;
            }
            let normal = match which {
                0 => c.ceiling.normal,
                1 => c.left_facing_wall.normal,
                _ => c.right_facing_wall.normal,
            };
            if reg(normal.angle_xy(fp.self_vel)) > limit {
                self.mt_hi_appear_air(gobj);
            }
        }
    }

    /// `ftMt_SpecialHi_GroundToAir`: the travel off the floor, on through the air.
    #[doc(alias = "ftMt_SpecialHi_GroundToAir")]
    pub fn mt_hi_travel_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_LOST,
            TRANSITION_FLAGS_1,
            frame,
            0.0,
            0.0,
            None,
        );
        let fp = &mut self.game.fighters[f.0];
        fp.x2223_b4 = true;
        fp.invisible = true;
    }

    /// `ftMt_SpecialAirHi_AirToGround`: the travel onto the floor, on along it.
    #[doc(alias = "ftMt_SpecialAirHi_AirToGround")]
    pub fn mt_hi_travel_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_LOST,
            TRANSITION_FLAGS_1,
            frame,
            0.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).invisible = true;
    }

    /// The stick's distance from center, at most 1.
    fn mt_stick_mag(&self, gobj: GObjId) -> f64 {
        let s = self.game.fighter(gobj).input.lstick[0].clone();
        let x = fp::fmuls(reg(s.x), reg(s.x));
        let y = fp::fmuls(reg(s.y), reg(s.y));
        let mag = reg(math::sqrt(mem(fp::fadds(x, y))));
        if mag > 1.0 { 1.0 } else { mag }
    }

    /// The travel's frames (`x50`), the jumps used up, intangible, invisible, and its sound
    /// (`ftMewtwo_SpecialHi_SetVars`).
    fn mt_hi_travel_common(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.mt_attr_i32(f, attr::HI_DURATION);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_FRAMES, frames);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.x2223_b4 = true;
        self.set_hit_status(gobj, 2);
        self.game.fighters[f.0].invisible = true;
        self.fighter_play_sfx(f, SFX_TRAVEL, 127, 64);
    }

    /// The travel along the floor, `ftMt_SpecialHi_Enter`: with the stick pushed far enough
    /// (`x58`), away from the floor's normal and not down through it, along the floor at its
    /// speed; else the travel through the air.
    #[doc(alias = "ftMt_SpecialHi_Enter")]
    pub fn mt_hi_travel_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mag = self.mt_stick_mag(gobj);
        if !(mag < reg(self.mt_attr_f32(f, attr::HI_STICK_MIN))) {
            let fp = &self.game.fighters[f.0];
            let normal = fp.coll_data.floor.normal;
            let s = fp.input.lstick[0].clone();
            let stick = crate::vec::Vec3::new(s.x, s.y, 0.0);
            let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            if !(reg(normal.angle_xy(stick)) < half_pi) && !self.pass_skip_floor(gobj) {
                self.face_stick(gobj);
                let fp = &self.game.fighters[f.0];
                let s = fp.input.lstick[0].clone();
                let a = math::atan2(s.y, mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))));
                let m = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM));
                let add = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM_ADD));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(mv::HI_STICK_X, reg(stick.x));
                fp.set_mv_f32(mv::HI_STICK_Y, reg(stick.y));
                let v = fp::fmuls(fp::fmadds(m, mag, add), reg(math::cos(a)));
                fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), v));
                self.change_motion_state(gobj, ms::SPECIAL_HI_LOST, 0, 35.0, 1.0, 0.0, None);
                self.anim_update(gobj);
                self.set_anim_rate(gobj, 0.0);
                self.mt_hi_travel_common(gobj);
                return;
            }
        }
        self.go_airborne_jumped(f);
        self.mt_hi_travel_air(gobj);
    }

    /// The travel through the air, `ftMt_SpecialAirHi_Enter`: in the stick's direction when
    /// pushed far enough, else straight up.
    #[doc(alias = "ftMt_SpecialAirHi_Enter")]
    pub fn mt_hi_travel_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.mt_stick_mag(gobj);
        let angle;
        if mag > reg(self.mt_attr_f32(f, attr::HI_STICK_MIN)) {
            let x = reg(self.game.fighters[f.0].input.lstick[0].x);
            let ax = if x < 0.0 { fp::fneg(x) } else { x };
            if ax > 0.001_000_000_047_497_451_3 {
                self.face_stick(gobj);
            }
            let fp = &mut self.game.fighters[f.0];
            let s = fp.input.lstick[0].clone();
            angle = reg(math::atan2(
                s.y,
                mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))),
            ));
            fp.set_mv_f32(mv::HI_STICK_X, reg(s.x));
            fp.set_mv_f32(mv::HI_STICK_Y, reg(s.y));
        } else {
            self.face_stick_if_pushed(gobj);
            angle = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::HI_STICK_X, 0.0);
            fp.set_mv_f32(mv::HI_STICK_Y, 1.0);
            mag = 1.0;
        }
        let m = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM));
        let add = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM_ADD));
        let speed = fp::fmadds(m, mag, add);
        let fp = &mut self.game.fighters[f.0];
        let c = reg(math::cos(mem(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(speed, c)));
        let s = reg(math::sin(mem(angle)));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_LOST, 0, 35.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        self.mt_hi_travel_common(gobj);
    }

    /// `ftMt_SpecialHi_Anim`: standing once the reappearance ends.
    #[doc(alias = "ftMt_SpecialHi_Anim")]
    pub fn mt_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMt_SpecialAirHi_Anim`: falling helplessly once it ends (`x70`, `x74`).
    #[doc(alias = "ftMt_SpecialAirHi_Anim")]
    pub fn mt_special_air_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let x = self.mt_attr_f32(f, attr::HI_FREEFALL_MOBILITY);
            let y = self.mt_attr_f32(f, attr::HI_LANDING_LAG);
            self.fall_special(gobj, x, y);
        }
    }

    /// `ftMt_SpecialAirHi_Phys`: once the script says (`cmd_vars[0]`), falling with the drift
    /// held to `x64` of the most; before, the rise slowing by a tenth each frame.
    #[doc(alias = "ftMt_SpecialAirHi_Phys")]
    pub fn mt_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            Self::fall_basic(&mut self.game.fighters[f.0]);
            let drift = reg(self.mt_attr_f32(f, attr::HI_DRIFT));
            let max = reg(self.game.fighters[f.0].co_attrs.air_drift_max);
            let limit = fp::fmuls(drift, max);
            Self::clamp_self_vel_x(&mut self.game.fighters[f.0], limit);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let vy = reg(fp.self_vel.y);
        fp.self_vel.y = mem(fp::fsubs(vy, fp::fdivs(vy, 10.0)));
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftMt_SpecialHi_Coll`: off the floor, the aerial reappearance.
    #[doc(alias = "ftMt_SpecialHi_Coll")]
    pub fn mt_special_hi_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.mt_special_hi_end_to_air(gobj);
        }
    }

    /// `ftMt_SpecialAirHi_Coll`: landing helplessly (`x74` frames); else a ledge.
    #[doc(alias = "ftMt_SpecialAirHi_Coll")]
    pub fn mt_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let lag = self.mt_attr_f32(f, attr::HI_LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// `ftMt_SpecialHiLost_GroundToAir`.
    #[doc(alias = "ftMt_SpecialHiLost_GroundToAir")]
    pub fn mt_special_hi_end_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI,
            TRANSITION_FLAGS_0,
            frame,
            1.0,
            0.0,
            None,
        );
    }

    /// The travel's speed kept and stopped, visible again, the reappearance's accessory to
    /// come (`ftMewtwo_SpecialHiLost_SetVars`).
    fn mt_hi_appear_common(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy, gv) = (fp.self_vel.x, fp.self_vel.y, fp.gr_vel);
        fp.set_mv_f32(mv::HI_VEL_X, reg(vx));
        fp.set_mv_f32(mv::HI_VEL_Y, reg(vy));
        fp.set_mv_f32(mv::HI_GR_VEL, reg(gv));
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
        fp.invisible = false;
        fp.accessory4_cb = Some(CodeRef(addr::HI_SET_END_GFX));
    }

    /// Reappearing on the ground, `ftMt_SpecialHiLost_Enter`: a share (`x6C`) of the travel's
    /// speed kept.
    #[doc(alias = "ftMt_SpecialHiLost_Enter")]
    pub fn mt_hi_appear_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.mt_hi_appear_common(gobj);
        let k = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM_END_MUL));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_GR_VEL)), k));
    }

    /// Reappearing in the air, `ftMt_SpecialAirHiLost_Enter`.
    #[doc(alias = "ftMt_SpecialAirHiLost_Enter")]
    pub fn mt_hi_appear_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.mt_hi_appear_common(gobj);
        let k = reg(self.mt_attr_f32(f, attr::HI_MOMENTUM_END_MUL));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_VEL_X)), k));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_VEL_Y)), k));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_CREATE_GFX => g.mt_hi_start_gfx_once(gobj),
        addr::HI_SET_START_GFX => g.mt_hi_start_gfx(gobj),
        addr::HI_SET_END_GFX => g.mt_hi_end_gfx(gobj),
        addr::SPECIAL_HI_START_ENTER => g.mt_special_hi_start_enter(gobj),
        addr::SPECIAL_AIR_HI_START_ENTER => g.mt_special_air_hi_start_enter(gobj),
        addr::SPECIAL_HI_START_ANIM => g.mt_special_hi_start_anim(gobj),
        addr::SPECIAL_AIR_HI_START_ANIM => g.mt_special_air_hi_start_anim(gobj),
        addr::SPECIAL_HI_START_IASA
        | addr::SPECIAL_AIR_HI_START_IASA
        | addr::SPECIAL_HI_LOST_IASA
        | addr::SPECIAL_AIR_HI_LOST_IASA
        | addr::SPECIAL_AIR_HI_LOST_PHYS
        | addr::SPECIAL_HI_IASA
        | addr::SPECIAL_AIR_HI_IASA => g.mt_special_hi_nothing(gobj),
        addr::SPECIAL_HI_START_PHYS | addr::SPECIAL_HI_PHYS => g.mt_ground_phys(gobj),
        addr::SPECIAL_AIR_HI_START_PHYS => g.mt_special_air_hi_start_phys(gobj),
        addr::SPECIAL_HI_START_COLL => g.mt_special_hi_start_coll(gobj),
        addr::SPECIAL_AIR_HI_START_COLL => g.mt_special_air_hi_start_coll(gobj),
        addr::SPECIAL_HI_START_GROUND_TO_AIR => g.mt_special_hi_start_to_air(gobj),
        addr::SPECIAL_AIR_HI_START_AIR_TO_GROUND => g.mt_special_air_hi_start_to_ground(gobj),
        addr::SPECIAL_HI_LOST_ANIM => g.mt_special_hi_lost_anim(gobj),
        addr::SPECIAL_AIR_HI_LOST_ANIM => g.mt_special_air_hi_lost_anim(gobj),
        addr::SPECIAL_HI_LOST_PHYS => g.mt_special_hi_lost_phys(gobj),
        addr::SPECIAL_HI_LOST_COLL => g.mt_special_hi_lost_coll(gobj),
        addr::SPECIAL_AIR_HI_LOST_COLL => g.mt_special_air_hi_lost_coll(gobj),
        addr::SPECIAL_HI_GROUND_TO_AIR => g.mt_hi_travel_to_air(gobj),
        addr::SPECIAL_AIR_HI_AIR_TO_GROUND => g.mt_hi_travel_to_ground(gobj),
        addr::SPECIAL_HI_ENTER => g.mt_hi_travel_ground(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.mt_hi_travel_air(gobj),
        addr::SPECIAL_HI_ANIM => g.mt_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.mt_special_air_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.mt_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_COLL => g.mt_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.mt_special_air_hi_coll(gobj),
        addr::SPECIAL_HI_LOST_GROUND_TO_AIR => g.mt_special_hi_end_to_air(gobj),
        addr::SPECIAL_HI_LOST_ENTER => g.mt_hi_appear_ground(gobj),
        addr::SPECIAL_AIR_HI_LOST_ENTER => g.mt_hi_appear_air(gobj),
        _ => return false,
    }
    true
}
