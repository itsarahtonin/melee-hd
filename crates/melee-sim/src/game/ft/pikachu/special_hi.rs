//! Pikachu's Quick Attack (`ftpikachuspecialhi.c`): a short wind-up, then a zip toward the
//! stick, which a second stick input may turn once; the zip stops at a surface it meets too
//! squarely.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::{KIND_PICHU, addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

/// The flags the wind-up keeps going between the ground and the air, the zip's, the end's.
const START0_FLAGS: u32 = 0x0C4C_5084;
const ZIP_FLAGS: u32 = 0x0C4C_508A;

/// The zip's flags, and the flags of the first state of a second zip.
const ZIP_ENTER_FLAGS: u32 = 10;
const ZIP_TURN_FLAGS: u32 = 2;

/// `0.999F`, the stick's longest reach.
const STICK_MAX: f64 = 0.999_000_012_874_603_3;
/// `M_PI / 2`, rounded to single precision.
const HALF_PI: f64 = 1.570_796_370_506_286_6;
/// `M_PI / 180` in single precision.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;
/// `0.001F`.
const STICK_DEAD: f64 = 0.001_000_000_047_497_451_3;

/// `SpecialHi.x8`: whether this is the second zip.
const SECOND: i32 = 1;

impl Running<'_> {
    /// The stick's tilt (the inline `sqrtf`).
    fn pk_stick_mag(&self, gobj: GObjId) -> f64 {
        let s = self.game.fighter(gobj).input.lstick[0].clone();
        let sq = fp::fadds(fp::fmuls(reg(s.x), reg(s.x)), fp::fmuls(reg(s.y), reg(s.y)));
        math::sqrt_reg(sq)
    }

    /// Whether the angle between `n` and `v` is more than `xA0` past square (90 + `xA0`
    /// degrees).
    fn pk_hi_past_square(&self, gobj: GObjId, n: Vec3, v: Vec3) -> bool {
        let f = self.game.fighter_id(gobj);
        let clamp = self.pk_attr_i32(f, attr::XA0);
        let limit = fp::fmuls(DEG_TO_RAD, fp::fadds(90.0, fp::frsp(f64::from(clamp))));
        reg(n.angle_xy(v)) > limit
    }

    /// `ftPk_SpecialHi_UpdateVel` (the zip's `x21F8`): the speeds and the zip's stick turn
    /// toward the facing.
    #[doc(alias = "ftPk_SpecialHi_UpdateVel")]
    pub fn pk_special_hi_update_vel(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let facing = reg(fp.facing_dir);
        fp.gr_vel = mem(fp::fmuls(facing, reg(fp.gr_vel).abs()));
        fp.self_vel.x = mem(fp::fmuls(facing, reg(fp.self_vel.x).abs()));
        let x = reg(fp.mv_f32(mv::HI_STICK_X));
        fp.set_mv_f32(mv::HI_STICK_X, fp::fmuls(facing, x.abs()));
    }

    fn pk_special_hi_start(&mut self, gobj: GObjId, msid: i32) {
        let f = self.game.fighter_id(gobj);
        let delay = self.pk_attr_i32(f, attr::X5C);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_FALL_DELAY, delay);
        fp.set_mv_i32(mv::HI_SECOND, 0);
        fp.set_mv_i32(mv::HI_AIR_FRAMES, 0);
        fp.gr_vel = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPk_SpecialHi_Enter`.
    #[doc(alias = "ftPk_SpecialHi_Enter")]
    pub fn pk_special_hi_enter(&mut self, gobj: GObjId) {
        self.pk_special_hi_start(gobj, ms::SPECIAL_HI_START0);
    }

    /// `ftPk_SpecialAirHi_Enter`.
    #[doc(alias = "ftPk_SpecialAirHi_Enter")]
    pub fn pk_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.pk_special_hi_start(gobj, ms::SPECIAL_AIR_HI_START0);
    }

    /// `ftPk_SpecialHiStart0_Anim`: the zip when the wind-up ends.
    #[doc(alias = "ftPk_SpecialHiStart0_Anim")]
    pub fn pk_special_hi_start0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.pk_special_hi_zip(gobj);
        }
    }

    /// `ftPk_SpecialAirHiStart0_Anim`.
    #[doc(alias = "ftPk_SpecialAirHiStart0_Anim")]
    pub fn pk_special_air_hi_start0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.pk_special_air_hi_zip(gobj);
        }
    }

    /// `ftPk_SpecialAirHiStart0_Phys`: floating for `x5C` frames, then falling at `x64`;
    /// quick air friction.
    #[doc(alias = "ftPk_SpecialAirHiStart0_Phys")]
    pub fn pk_special_air_hi_start0_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.pk_attr_f32(f, attr::X64));
        let fp = &mut self.game.fighters[f.0];
        let delay = fp.mv_i32(mv::HI_FALL_DELAY);
        if delay != 0 {
            fp.set_mv_i32(mv::HI_FALL_DELAY, delay.wrapping_sub(1));
        } else {
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        self.self_accel_deaccel_quick_air(gobj);
    }

    /// `ftPk_SpecialHiStart0_Coll`: off a ledge, the aerial wind-up
    ///.
    #[doc(alias = "ftPk_SpecialHiStart0_Coll")]
    pub fn pk_special_hi_start0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.pk_special_hi_start0_ground_to_air(gobj);
        }
    }

    /// `ftPk_SpecialHi_ChangeMotion_Unk00`: the aerial wind-up from the same frame.
    #[doc(alias = "ftPk_SpecialHi_ChangeMotion_Unk00")]
    pub fn pk_special_hi_start0_ground_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START0,
            START0_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftPk_SpecialHi_ChangeMotion_Unk01`: the grounded wind-up from the same frame.
    #[doc(alias = "ftPk_SpecialHi_ChangeMotion_Unk01")]
    pub fn pk_special_air_hi_start0_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI_START0, START0_FLAGS);
    }

    /// `ftPk_SpecialAirHiStart0_Coll`: landing, the grounded wind-up Else a ledge to catch.
    #[doc(alias = "ftPk_SpecialAirHiStart0_Coll")]
    pub fn pk_special_air_hi_start0_coll(&mut self, gobj: GObjId) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            self.pk_special_air_hi_start0_air_to_ground(gobj);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// The zip's sparks (not Pichu's): on the last frame at the rotation joint, else
    /// scattered by `spread` around it.
    fn pk_special_hi_spark(&mut self, gobj: GObjId, spread: Option<(f64, f64)>) {
        if self.game.fighter(gobj).kind == KIND_PICHU {
            return;
        }
        let j = self.part_joint(gobj, part::X_ROT_N);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        if let Some((width, half)) = spread {
            let r = self.game.objs.rng.draw_f32();
            pos.x = mem(fp::fadds(reg(pos.x), fp::fmsubs(width, reg(r), half)));
            let r = self.game.objs.rng.draw_f32();
            pos.y = mem(fp::fadds(reg(pos.y), fp::fmsubs(width, reg(r), half)));
        }
        spawn(self, 0x3F4, Some(gobj), &[Arg::Vec(pos)]);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The zip's frame: one frame fewer, and its end after the last.
    fn pk_special_hi_zip_tick(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_FRAMES).wrapping_sub(1);
        fp.set_mv_i32(mv::HI_FRAMES, n);
        if n <= 0 {
            if air {
                self.pk_special_hi_air_end_enter(gobj);
            } else {
                self.pk_special_hi_end_enter(gobj);
            }
            self.pk_special_hi_spark(gobj, None);
        } else {
            let spread = if air { (10.0, 5.0) } else { (6.0, 3.0) };
            self.pk_special_hi_spark(gobj, Some(spread));
        }
    }

    /// `ftPk_SpecialHiStart1_Anim`.
    #[doc(alias = "ftPk_SpecialHiStart1_Anim")]
    pub fn pk_special_hi_start1_anim(&mut self, gobj: GObjId) {
        self.pk_special_hi_zip_tick(gobj, false);
    }

    /// `ftPk_SpecialAirHiStart1_Anim`.
    #[doc(alias = "ftPk_SpecialAirHiStart1_Anim")]
    pub fn pk_special_air_hi_start1_anim(&mut self, gobj: GObjId) {
        self.pk_special_hi_zip_tick(gobj, true);
    }

    /// Sets the rotation joint's scale to the `Vec3` at attribute `off`.
    fn pk_special_hi_scale(&mut self, gobj: GObjId, off: usize) {
        let f = self.game.fighter_id(gobj);
        let s = Vec3::new(
            self.pk_attr_f32(f, off),
            self.pk_attr_f32(f, off + 4),
            self.pk_attr_f32(f, off + 8),
        );
        let j = self
            .part_joint(gobj, part::X_ROT_N)
            .expect("a part without a joint");
        jobj::set_scale(&mut self.game.objs.jobjs, j, s);
    }

    /// Tilts the rotation joint to `angle`.
    fn pk_special_hi_tilt(&mut self, gobj: GObjId, angle: f64) {
        let f = self.game.fighter_id(gobj);
        let i = self.bone_index(&self.game.fighters[f.0], part::X_ROT_N);
        self.game.part_set_rot_x(f, i, mem(angle));
    }

    /// On a sloped floor, tilts the zip along it (plus `x68`); then its ground scale.
    fn pk_special_hi_ground_pose(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.coll_data.env_flags & (0x8000 | 0x10000) != 0 {
            let n = fp.coll_data.floor.normal;
            let facing = reg(fp.facing_dir);
            let offset = reg(self.pk_attr_f32(f, attr::X68));
            let angle = fp::fmadds(facing, reg(math::atan2(n.x, n.y)), offset);
            self.pk_special_hi_tilt(gobj, angle);
        }
        self.pk_special_hi_scale(gobj, attr::X6C_SCALE);
    }

    /// `ftPk_SpecialHi_8012642C` (the aerial zip's physics): the zip tilted along its speed
    /// (plus `x78`) with its air scale, and every speed but its own cleared.
    #[doc(alias = "ftPk_SpecialHi_8012642C")]
    pub fn pk_special_hi_air_zip_pose(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let offset = reg(self.pk_attr_f32(f, attr::X78));
        let fp = &self.game.fighters[f.0];
        let angle = math::atan2(fp.self_vel.x, fp.self_vel.y);
        let tilt = fp::fmadds(reg(fp.facing_dir), reg(angle), fp::fsubs(offset, HALF_PI));
        self.pk_special_hi_tilt(gobj, tilt);
        self.pk_special_hi_scale(gobj, attr::X7C_SCALE);
        let vel = self.game.fighters[f.0].self_vel;
        self.stop_dead(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = vel;
        fp.x68C_transNPos = Vec3::new(0.0, 0.0, 0.0);
        fp.x6A4_transNOffset = Vec3::new(0.0, 0.0, 0.0);
        fp.lstick_angle = 0.0;
        fp.x6C0 = Vec3::new(0.0, 0.0, 0.0);
        fp.x6D8 = Vec3::new(0.0, 0.0, 0.0);
    }

    /// `ftPk_SpecialHiStart1_Coll`: off the floor, the aerial end if a wall stopped it, else
    /// the aerial zip; on it, its pose, and the end at
    /// a wall.
    #[doc(alias = "ftPk_SpecialHiStart1_Coll")]
    pub fn pk_special_hi_start1_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.pk_height_box(f);
        if !self.ground_coll_box(gobj, &b) {
            let env = self.game.fighters[f.0].coll_data.env_flags;
            if env & 0x3F != 0 || env & 0xFC0 != 0 {
                self.go_airborne_jumped(f);
                self.pk_special_hi_air_end_enter(gobj);
                return;
            }
            self.pk_special_hi_start1_ground_to_air(gobj);
            return;
        }
        self.pk_special_hi_ground_pose(gobj);
        let env = self.game.fighters[f.0].coll_data.env_flags;
        if env & 0x3F != 0 || env & 0xFC0 != 0 {
            self.pk_special_hi_end_enter(gobj);
        }
    }

    /// `ftPk_SpecialHi_ChangeMotion_Unk02`: the zip goes on in the air.
    #[doc(alias = "ftPk_SpecialHi_ChangeMotion_Unk02")]
    pub fn pk_special_hi_start1_ground_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START1,
            ZIP_FLAGS,
            frame,
            0.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].x2223_b4 = true;
        self.pk_special_hi_air_zip_pose(gobj);
    }

    /// `ftPk_SpecialAirHiStart1_Coll`: landing (once `x88` frames airborne, or not through a
    /// platform), the grounded zip, or its end if it meets the floor too squarely; else a
    /// ledge to catch, or the aerial end at a ceiling or wall met too squarely.
    #[doc(alias = "ftPk_SpecialAirHiStart1_Coll")]
    pub fn pk_special_air_hi_start1_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let frames = fp.mv_i32(mv::HI_AIR_FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::HI_AIR_FRAMES, frames);
        let dir = facing_dir_int(fp.facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            let wait = reg(self.pk_attr_f32(f, attr::X88));
            let lands = fp::frsp(f64::from(frames)) >= wait || !self.pass_skip_floor(gobj);
            if lands {
                let fp = &self.game.fighters[f.0];
                let (n, v) = (fp.coll_data.floor.normal, fp.self_vel);
                if self.pk_hi_past_square(gobj, n, v) {
                    self.land(gobj);
                    self.pk_special_hi_end_enter(gobj);
                    return;
                }
                self.pk_special_hi_land(gobj);
                return;
            }
        }
        if !self.catch_ledge_check(gobj) {
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & (0x2000 | 0x4000) != 0 {
                let (n, v) = (c.ceiling.normal, fp.self_vel);
                if self.pk_hi_past_square(gobj, n, v) {
                    self.pk_special_hi_air_end_enter(gobj);
                }
            }
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & 0x3F != 0 {
                let (n, v) = (c.left_facing_wall.normal, fp.self_vel);
                if self.pk_hi_past_square(gobj, n, v) {
                    self.pk_special_hi_air_end_enter(gobj);
                }
            }
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & 0xFC0 != 0 {
                let (n, v) = (c.right_facing_wall.normal, fp.self_vel);
                if self.pk_hi_past_square(gobj, n, v) {
                    self.pk_special_hi_air_end_enter(gobj);
                }
            }
        }
    }

    /// `ftPk_SpecialHi_ChangeMotion_Unk03`: the zip lands and goes on along the floor.
    #[doc(alias = "ftPk_SpecialHi_ChangeMotion_Unk03")]
    pub fn pk_special_hi_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_START1,
            ZIP_FLAGS,
            frame,
            0.0,
            0.0,
            None,
        );
        self.pk_special_hi_ground_pose(gobj);
    }

    /// Starts a zip at `msid`: the frames it lasts (`x60`), the jumps used up; a second zip
    /// first enters the zip at frame 12.
    fn pk_special_hi_zip_enter(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::HI_SECOND) != 0 {
            self.change_motion_state(gobj, msid, ZIP_TURN_FLAGS, 12.0, 1.0, 0.0, None);
            self.anim_update(gobj);
        }
        self.change_motion_state(gobj, msid, ZIP_ENTER_FLAGS, 13.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        let fp = self.game.fighter_mut(gobj);
        fp.x2223_b4 = true;
        fp.x21F8 = Some(CodeRef(addr::SPECIAL_HI_UPDATE_VEL));
    }

    /// The zip's frames and the jumps used up.
    fn pk_special_hi_zip_frames(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.pk_attr_i32(f, attr::X60);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_FRAMES, frames);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// `ftPk_SpecialHi_80126C0C`: a grounded zip toward the stick when it's tilted past `x8C`
    /// away from the floor (and not down through a platform); else the aerial zip.
    #[doc(alias = "ftPk_SpecialHi_80126C0C")]
    pub fn pk_special_hi_zip(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.pk_stick_mag(gobj);
        if mag > STICK_MAX {
            mag = STICK_MAX;
        }
        let dead = reg(self.pk_attr_f32(f, attr::X8C));
        if !(mag < dead) {
            let fp = &self.game.fighters[f.0];
            let s = fp.input.lstick[0].clone();
            let dir = Vec3::new(s.x, s.y, 0.0);
            let floor = fp.coll_data.floor.normal;
            if !(reg(floor.angle_xy(dir)) < HALF_PI) && !self.pass_skip_floor(gobj) {
                self.face_stick(gobj);
                let x90 = reg(self.pk_attr_f32(f, attr::X90));
                let x94 = reg(self.pk_attr_f32(f, attr::X94));
                let x98 = reg(self.pk_attr_f32(f, attr::X98));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(mv::HI_STICK_X, reg(s.x));
                fp.set_mv_f32(mv::HI_STICK_Y, reg(s.y));
                self.pk_special_hi_zip_frames(gobj);
                let fp = &mut self.game.fighters[f.0];
                fp.gr_vel = mem(fp::fmadds(x90, mag, x94));
                fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), reg(fp.facing_dir)));
                if fp.mv_i32(mv::HI_SECOND) != 0 {
                    fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), x98));
                }
                self.pk_special_hi_zip_enter(gobj, ms::SPECIAL_HI_START1);
                return;
            }
        }
        self.go_airborne_jumped(f);
        self.pk_special_air_hi_zip(gobj);
    }

    /// `ftPk_SpecialHi_80126E1C`: an aerial zip toward the stick when it's tilted past `x8C`,
    /// else straight up at the longest reach.
    #[doc(alias = "ftPk_SpecialHi_80126E1C")]
    pub fn pk_special_air_hi_zip(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.pk_stick_mag(gobj);
        if mag > STICK_MAX {
            mag = STICK_MAX;
        }
        let dead = reg(self.pk_attr_f32(f, attr::X8C));
        let angle;
        if mag > dead {
            let s = self.game.fighters[f.0].input.lstick[0].clone();
            if reg(s.x).abs() > STICK_DEAD {
                self.face_stick(gobj);
            }
            let fp = &mut self.game.fighters[f.0];
            angle = math::atan2(s.y, mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))));
            fp.set_mv_f32(mv::HI_STICK_X, reg(s.x));
            fp.set_mv_f32(mv::HI_STICK_Y, reg(s.y));
        } else {
            self.face_stick_if_pushed(gobj);
            mag = STICK_MAX;
            angle = mem(HALF_PI);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::HI_STICK_X, 0.0);
            fp.set_mv_f32(mv::HI_STICK_Y, STICK_MAX);
        }
        self.pk_special_hi_zip_frames(gobj);
        let x90 = reg(self.pk_attr_f32(f, attr::X90));
        let x94 = reg(self.pk_attr_f32(f, attr::X94));
        let x98 = reg(self.pk_attr_f32(f, attr::X98));
        let fp = &mut self.game.fighters[f.0];
        let speed = fp::fmadds(x90, mag, x94);
        let t = fp::fmuls(speed, reg(math::cos(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), t));
        let speed = fp::fmadds(x90, mag, x94);
        fp.self_vel.y = mem(fp::fmuls(speed, reg(math::sin(angle))));
        if fp.mv_i32(mv::HI_SECOND) != 0 {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), x98));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), x98));
        }
        self.pk_special_hi_zip_enter(gobj, ms::SPECIAL_AIR_HI_START1);
    }

    /// `ftPk_SpecialHi_80127064`: whether the stick asks for a second zip: tilted past `x8C`,
    /// on the first zip, and turned more than `xA8` degrees from the first.
    #[doc(alias = "ftPk_SpecialHi_80127064")]
    pub fn pk_special_hi_turns(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let mag = self.pk_stick_mag(gobj);
        if mag < reg(self.pk_attr_f32(f, attr::X8C)) {
            return false;
        }
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(mv::HI_SECOND) != 0 {
            return false;
        }
        let s = fp.input.lstick[0].clone();
        let now = Vec3::new(s.x, s.y, 0.0);
        let first = Vec3::new(fp.mv_f32(mv::HI_STICK_X), fp.mv_f32(mv::HI_STICK_Y), 0.0);
        let turn = self.pk_attr_i32(f, attr::XA8);
        reg(first.angle_xy(now)) > fp::fmuls(DEG_TO_RAD, fp::frsp(f64::from(turn)))
    }

    /// The end's script window: a second zip if the stick asks for one, else no more.
    fn pk_special_hi_end_window(&mut self, gobj: GObjId) -> Option<bool> {
        if self.game.fighter(gobj).cmd_vars[0] != 1 {
            return None;
        }
        if self.pk_special_hi_turns(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.cmd_vars[0] = 0;
            fp.set_mv_i32(mv::HI_SECOND, SECOND);
            return Some(true);
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 2;
        Some(false)
    }

    /// `ftPk_SpecialHiEnd_Anim`.
    #[doc(alias = "ftPk_SpecialHiEnd_Anim")]
    pub fn pk_special_hi_end_anim(&mut self, gobj: GObjId) {
        match self.pk_special_hi_end_window(gobj) {
            Some(true) => self.pk_special_hi_zip(gobj),
            Some(false) => {}
            None => {
                if !self.anim_frames_remaining(gobj) {
                    self.enter_wait(gobj);
                }
            }
        }
    }

    /// `ftPk_SpecialAirHiEnd_Anim`: a helpless fall when it ends.
    #[doc(alias = "ftPk_SpecialAirHiEnd_Anim")]
    pub fn pk_special_air_hi_end_anim(&mut self, gobj: GObjId) {
        match self.pk_special_hi_end_window(gobj) {
            Some(true) => self.pk_special_air_hi_zip(gobj),
            Some(false) => {}
            None => {
                if !self.anim_frames_remaining(gobj) {
                    let f = self.game.fighter_id(gobj);
                    let drift = self.pk_attr_f32(f, attr::XAC);
                    let lag = self.pk_attr_f32(f, attr::XB0);
                    self.fall_special(gobj, drift, lag);
                }
            }
        }
    }

    /// `ftPk_SpecialHiEnd_Phys`: ground friction once the script says.
    #[doc(alias = "ftPk_SpecialHiEnd_Phys")]
    pub fn pk_special_hi_end_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.ground_friction(gobj);
        }
    }

    /// `ftPk_SpecialAirHiEnd_Phys`: once the script says, a plain fall with the drift held
    /// to `x9C` of the most; before, the rise slowing by a ninth a frame, with air friction.
    #[doc(alias = "ftPk_SpecialAirHiEnd_Phys")]
    pub fn pk_special_air_hi_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x9c = reg(self.pk_attr_f32(f, attr::X9C));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            Self::fall_basic(fp);
            let max = fp::fmuls(x9c, reg(fp.co_attrs.air_drift_max));
            Self::clamp_self_vel_x(fp, max);
        } else {
            let vy = reg(fp.self_vel.y);
            fp.self_vel.y = mem(fp::fsubs(vy, fp::fdivs(vy, 9.0)));
            Self::self_accel_deaccel_air(fp);
        }
    }

    /// `ftPk_SpecialHiEnd_Coll`: off the floor, the aerial end.
    #[doc(alias = "ftPk_SpecialHiEnd_Coll")]
    pub fn pk_special_hi_end_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.pk_height_box(f);
        if !self.ground_coll_box(gobj, &b) {
            self.pk_special_hi_end_ground_to_air(gobj);
        }
    }

    /// `ftPk_SpecialHi_ChangeMotion_Unk04`: the aerial end from the same frame.
    #[doc(alias = "ftPk_SpecialHi_ChangeMotion_Unk04")]
    pub fn pk_special_hi_end_ground_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_END,
            ZIP_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftPk_SpecialAirHiEnd_Coll`: landing, helpless over `xB0` frames; else a ledge to
    /// catch.
    #[doc(alias = "ftPk_SpecialAirHiEnd_Coll")]
    pub fn pk_special_air_hi_end_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.pk_height_box(f);
        let dir = fp::fctiwz(reg(self.game.fighters[f.0].facing_dir));
        if self.air_coll_box_ledges(gobj, dir, &b) {
            let lag = self.pk_attr_f32(f, attr::XB0);
            self.enter_landing_fall_special(gobj, 0, lag);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// The end's speeds: the zip's kept (`x1C`, `x24`) and cleared.
    fn pk_special_hi_keep_vel(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy, gv) = (fp.self_vel.x, fp.self_vel.y, fp.gr_vel);
        fp.set_mv_f32(mv::HI_END_VEL_X, reg(vx));
        fp.set_mv_f32(mv::HI_END_VEL_Y, reg(vy));
        fp.set_mv_f32(mv::HI_END_GR_VEL, reg(gv));
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
    }

    /// `ftPk_SpecialHi_MotionChangeUpdateVel_Unk0`: the grounded end, at `xA4` of the zip's
    /// speed.
    #[doc(alias = "ftPk_SpecialHi_MotionChangeUpdateVel_Unk0")]
    pub fn pk_special_hi_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xa4 = reg(self.pk_attr_f32(f, attr::XA4));
        self.pk_special_hi_keep_vel(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_END_GR_VEL)), xa4));
        self.change_motion_state(gobj, ms::SPECIAL_HI_END, 2, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x21F8 = Some(CodeRef(addr::SPECIAL_HI_UPDATE_VEL));
    }

    /// `ftPk_SpecialHi_MotionChangeUpdateVel_Unk1`: the aerial end, at `xA4` of the zip's
    /// speed.
    #[doc(alias = "ftPk_SpecialHi_MotionChangeUpdateVel_Unk1")]
    pub fn pk_special_hi_air_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xa4 = reg(self.pk_attr_f32(f, attr::XA4));
        self.pk_special_hi_keep_vel(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_END_VEL_X)), xa4));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_END_VEL_Y)), xa4));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_END, 2, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x21F8 = Some(CodeRef(addr::SPECIAL_HI_UPDATE_VEL));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_UPDATE_VEL => g.pk_special_hi_update_vel(gobj),
        addr::SPECIAL_HI_ENTER => g.pk_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.pk_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_START0_ANIM => g.pk_special_hi_start0_anim(gobj),
        addr::SPECIAL_AIR_HI_START0_ANIM => g.pk_special_air_hi_start0_anim(gobj),
        addr::SPECIAL_HI_START0_IASA
        | addr::SPECIAL_AIR_HI_START0_IASA
        | addr::SPECIAL_HI_START1_IASA
        | addr::SPECIAL_AIR_HI_START1_IASA
        | addr::SPECIAL_HI_END_IASA
        | addr::SPECIAL_AIR_HI_END_IASA => g.pk_nothing(gobj),
        addr::SPECIAL_HI_START0_PHYS => g.pk_ground_friction(gobj),
        addr::SPECIAL_AIR_HI_START0_PHYS => g.pk_special_air_hi_start0_phys(gobj),
        addr::SPECIAL_HI_START0_COLL => g.pk_special_hi_start0_coll(gobj),
        addr::SPECIAL_AIR_HI_START0_COLL => g.pk_special_air_hi_start0_coll(gobj),
        addr::SPECIAL_HI_START1_ANIM => g.pk_special_hi_start1_anim(gobj),
        addr::SPECIAL_AIR_HI_START1_ANIM => g.pk_special_air_hi_start1_anim(gobj),
        addr::SPECIAL_HI_START1_PHYS => g.pk_special_hi_start1_phys(gobj),
        addr::SPECIAL_AIR_HI_START1_PHYS => g.pk_special_air_hi_start1_phys(gobj),
        addr::SPECIAL_HI_START1_COLL => g.pk_special_hi_start1_coll(gobj),
        addr::SPECIAL_AIR_HI_START1_COLL => g.pk_special_air_hi_start1_coll(gobj),
        addr::SPECIAL_HI_END_ANIM => g.pk_special_hi_end_anim(gobj),
        addr::SPECIAL_AIR_HI_END_ANIM => g.pk_special_air_hi_end_anim(gobj),
        addr::SPECIAL_HI_END_PHYS => g.pk_special_hi_end_phys(gobj),
        addr::SPECIAL_AIR_HI_END_PHYS => g.pk_special_air_hi_end_phys(gobj),
        addr::SPECIAL_HI_END_COLL => g.pk_special_hi_end_coll(gobj),
        addr::SPECIAL_AIR_HI_END_COLL => g.pk_special_air_hi_end_coll(gobj),
        addr::SPECIAL_HI_AIR_END_ENTER => g.pk_special_hi_air_end_enter(gobj),
        addr::SPECIAL_HI_END_ENTER => g.pk_special_hi_end_enter(gobj),
        addr::SPECIAL_HI_CHANGE_MOTION_UNK00 => g.pk_special_hi_start0_ground_to_air(gobj),
        addr::SPECIAL_HI_CHANGE_MOTION_UNK01 => g.pk_special_air_hi_start0_air_to_ground(gobj),
        addr::SPECIAL_HI_CHANGE_MOTION_UNK02 => g.pk_special_hi_start1_ground_to_air(gobj),
        addr::SPECIAL_HI_CHANGE_MOTION_UNK03 => g.pk_special_hi_land(gobj),
        addr::SPECIAL_HI_CHANGE_MOTION_UNK04 => g.pk_special_hi_end_ground_to_air(gobj),
        addr::SPECIAL_HI_AIR_ZIP_POSE => g.pk_special_hi_air_zip_pose(gobj),
        addr::SPECIAL_HI_ZIP => g.pk_special_hi_zip(gobj),
        addr::SPECIAL_AIR_HI_ZIP => g.pk_special_air_hi_zip(gobj),
        _ => return false,
    }
    true
}
