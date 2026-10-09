//! Vanish (`ftseakspecialhi.c`): the wind-up, the explosion that leaves a flash behind
//! (`it::kinds::seakvanish`) as Sheik vanishes, the invisible travel in the stick's direction
//! (along the floor on the ground), and the reappearance.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::seakvanish;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::super::motion::flags::*;
use super::{addr, attr, ext, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `FtPart_HipN`: Sheik's code indexes `fp->parts` by the part directly.
const PART_HIP_N: i32 = 4;

/// `ftSk_MF_SpecialHi_Coll`, and the travel's flags (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;
/// What the travel keeps when it changes between the ground and the air: the model's
/// visibility, the items', the effects, the color animation and the script.
const TRAVEL_FLAGS: u32 = (SKIP_MODEL_PART_VIS
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_FLAGS
    | UNK_27)
    + (KEEP_GFX | KEEP_COL_ANIM_HIT_STATUS | SKIP_HIT | SKIP_MAT_ANIM | SKIP_COL_ANIM | UPDATE_CMD);
/// The reappearance's flags off the ground (`0xC4C508A`).
const END_FALL_FLAGS: u32 = 0x0C4C_508A;

/// `Collide_LeftWallMask`, `Collide_RightWallMask`, `Collide_CeilingMask`.
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;
const CEILING_MASK: i32 = 0x2000 | 0x4000;

/// The explosion's effect, the reappearance's, and the explosion's sound.
const GFX_VANISH: i32 = 1284;
const GFX_APPEAR: i32 = 1285;
const SFX_VANISH: i32 = 115;

/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

impl Running<'_> {
    /// The hip's position in the world.
    fn seak_hip_pos(&mut self, gobj: GObjId) -> crate::vec::Vec3 {
        let j = self.bone_joint(gobj, PART_HIP_N);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// The explosion, once (`x2219_b0`), `fn_80112ED8` (an accessory): the flash left
    /// behind, its effect and sound.
    #[doc(alias = "fn_80112ED8")]
    pub fn seak_special_hi_vanish(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            self.seak_special_hi_spawn_flash(gobj);
            self.seak_special_hi_vanish_gfx(gobj);
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, SFX_VANISH, 0x7F, 0x40);
        }
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The flash at the hip, `ftSk_SpecialHi_80112F48`.
    #[doc(alias = "ftSk_SpecialHi_80112F48")]
    pub fn seak_special_hi_spawn_flash(&mut self, gobj: GObjId) {
        let mut pos = self.seak_hip_pos(gobj);
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        seakvanish::spawn(self, gobj, pos, facing);
    }

    /// The explosion's effect at the hip, once, `ftSk_SpecialHi_80112FA8`.
    #[doc(alias = "ftSk_SpecialHi_80112FA8")]
    pub fn seak_special_hi_vanish_gfx(&mut self, gobj: GObjId) {
        let pos = self.seak_hip_pos(gobj);
        if !self.game.fighter(gobj).x2219_b0 {
            spawn(self, GFX_VANISH, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The reappearance's effect at the hip, once, `fn_80113038` (an accessory).
    #[doc(alias = "fn_80113038")]
    pub fn seak_special_hi_appear_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let pos = self.seak_hip_pos(gobj);
            spawn(self, GFX_APPEAR, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The grounded wind-up, `ftSk_SpecialHi_Enter`.
    #[doc(alias = "ftSk_SpecialHi_Enter")]
    pub fn seak_special_hi_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_XC, 0);
        self.change_motion_state(gobj, ms::SPECIAL_HI_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The aerial wind-up, `ftSk_SpecialAirHi_Enter`: a hop up.
    #[doc(alias = "ftSk_SpecialAirHi_Enter")]
    pub fn seak_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let vy = self.seak_attr_f32(f, attr::SELF_VEL_Y);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_XC, 0);
        fp.self_vel.y = vy;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftSk_SpecialHiStart_0_Anim`: the travel along the floor once the wind-up ends.
    #[doc(alias = "ftSk_SpecialHiStart_0_Anim")]
    pub fn seak_special_hi_start_0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.seak_special_hi_travel_ground(gobj);
        }
    }

    /// `ftSk_SpecialAirHiStart_0_Anim`: the travel through the air.
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Anim")]
    pub fn seak_special_air_hi_start_0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.seak_special_hi_travel_air(gobj);
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftSk_SpecialHiStart_0_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_0_IASA")]
    #[doc(alias = "ftSk_SpecialHiStart_1_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_IASA")]
    #[doc(alias = "ftSk_SpecialHi_IASA")]
    #[doc(alias = "ftSk_SpecialAirHi_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Phys")]
    pub fn seak_special_hi_nothing(&mut self, _gobj: GObjId) {}

    /// `ftSk_SpecialAirHiStart_0_Phys`: the wind-up's own fall, and drift.
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Phys")]
    pub fn seak_special_air_hi_start_0_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.seak_attr_f32(f, attr::X30));
        let t = reg(self.seak_attr_f32(f, attr::X34));
        Self::fall(&mut self.game.fighters[f.0], g, t);
        self.air_drift(gobj);
    }

    /// `ftSk_SpecialHiStart_0_Coll`: off the ground, the aerial wind-up.
    #[doc(alias = "ftSk_SpecialHiStart_0_Coll")]
    pub fn seak_special_hi_start_0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.seak_special_hi_start_0_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirHiStart_0_Coll`: landing, the grounded wind-up; else a ledge.
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Coll")]
    pub fn seak_special_air_hi_start_0_coll(&mut self, gobj: GObjId) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            self.seak_special_hi_start_0_to_ground(gobj);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// `ftSk_SpecialHi_80113324`.
    #[doc(alias = "ftSk_SpecialHi_80113324")]
    pub fn seak_special_hi_start_0_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START_0,
            COLL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_FN_80112ED8));
    }

    /// `ftSk_SpecialHi_80113390`.
    #[doc(alias = "ftSk_SpecialHi_80113390")]
    pub fn seak_special_hi_start_0_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI_START_0, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_FN_80112ED8));
    }

    /// The travel's countdown (`specialhi.x0`); whether it ran out.
    fn seak_special_hi_count_down(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_X0).wrapping_sub(1);
        fp.set_mv_i32(mv::HI_X0, n);
        n <= 0
    }

    /// `ftSk_SpecialHiStart_1_Anim`: the reappearance on the ground once the travel ends.
    #[doc(alias = "ftSk_SpecialHiStart_1_Anim")]
    pub fn seak_special_hi_start_1_anim(&mut self, gobj: GObjId) {
        if self.seak_special_hi_count_down(gobj) {
            self.seak_special_hi_appear_ground(gobj);
        }
    }

    /// `ftSk_SpecialAirHiStart_1_Anim`: the reappearance in the air.
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Anim")]
    pub fn seak_special_air_hi_start_1_anim(&mut self, gobj: GObjId) {
        if self.seak_special_hi_count_down(gobj) {
            self.seak_special_hi_appear_air(gobj);
        }
    }

    /// `ftSk_SpecialHiStart_1_Phys`: the travel along the floor.
    #[doc(alias = "ftSk_SpecialHiStart_1_Phys")]
    pub fn seak_special_hi_start_1_phys(&mut self, gobj: GObjId) {
        self.self_movement_from_ground(gobj);
    }

    /// `ftSk_SpecialHiStart_1_Coll`: off the floor, the travel goes on in the air (or Sheik
    /// reappears there against a wall); along it, she reappears against a wall.
    #[doc(alias = "ftSk_SpecialHiStart_1_Coll")]
    pub fn seak_special_hi_start_1_coll(&mut self, gobj: GObjId) {
        let walls = |r: &Running| {
            r.game.fighter(gobj).coll_data.env_flags & (LEFT_WALL_MASK | RIGHT_WALL_MASK) != 0
        };
        if !self.ground_coll_off_ledges(gobj) {
            if walls(self) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne_jumped(f);
                self.seak_special_hi_appear_air(gobj);
                return;
            }
            self.seak_special_hi_travel_to_air(gobj);
            return;
        }
        if walls(self) {
            self.seak_special_hi_appear_ground(gobj);
        }
    }

    /// `ftSk_SpecialAirHiStart_1_Coll`: meeting the ground (after `x3C` frames, or where she
    /// can't pass through), the travel goes on along it; else a ledge, or reappearing against
    /// a surface met head on (`ftCommon_HandleTeleportCollisions`).
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Coll")]
    pub fn seak_special_air_hi_start_1_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::HI_XC).wrapping_add(1);
        fp.set_mv_i32(mv::HI_XC, n);
        let dir = facing_dir_int(fp.facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let frames = self.game.fighters[f.0].mv_i32(mv::HI_XC);
            let land = if fp::frsp(f64::from(frames)) >= reg(self.seak_attr_f32(f, attr::X3C)) {
                true
            } else {
                !self.pass_skip_floor(gobj)
            };
            if land {
                self.seak_special_hi_travel_to_ground(gobj);
                return;
            }
        }
        if !self.catch_ledge_check(gobj) {
            self.seak_teleport_collisions(gobj);
        }
    }

    /// `ftCommon_HandleTeleportCollisions` with the attributes' angle (`x50`): Sheik reappears
    /// for each of the ceiling and walls she meets more head on than that.
    fn seak_teleport_collisions(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let clamp = self.seak_attr_i32(f, attr::X50);
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
                self.seak_special_hi_appear_air(gobj);
            }
        }
    }

    /// `ftSk_SpecialHi_8011374C`: the travel off the floor, on through the air.
    #[doc(alias = "ftSk_SpecialHi_8011374C")]
    pub fn seak_special_hi_travel_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START_1,
            TRAVEL_FLAGS,
            frame,
            0.0,
            0.0,
            None,
        );
        let fp = &mut self.game.fighters[f.0];
        fp.x2223_b4 = true;
        fp.invisible = true;
    }

    /// `ftSk_SpecialHi_801137C8`: the travel onto the floor, on along it.
    #[doc(alias = "ftSk_SpecialHi_801137C8")]
    pub fn seak_special_hi_travel_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_START_1,
            TRAVEL_FLAGS,
            frame,
            0.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).invisible = true;
    }

    /// The stick's distance from center, at most 1 (`my_sqrtf`).
    fn seak_stick_mag(&self, gobj: GObjId) -> f64 {
        let s = self.game.fighter(gobj).input.lstick[0].clone();
        let x = fp::fmuls(reg(s.x), reg(s.x));
        let y = fp::fmuls(reg(s.y), reg(s.y));
        let mag = reg(math::sqrt(mem(fp::fadds(x, y))));
        if mag > 1.0 { 1.0 } else { mag }
    }

    /// `inlineA0`: the travel's frames (`x38`), the jumps used up, intangible and invisible,
    /// the explosion to come.
    fn seak_special_hi_travel_common(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.seak_attr_i32(f, attr::X38);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_X0, frames);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.x2223_b4 = true;
        self.set_hit_status(gobj, 2);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.accessory4_cb = Some(CodeRef(addr::HI_FN_80112ED8));
    }

    /// The travel along the floor, `ftSk_SpecialHi_80113838`: with the stick pushed far
    /// enough, away from the floor's normal and not down through it, along the floor at its
    /// speed; else the travel through the air.
    #[doc(alias = "ftSk_SpecialHi_80113838")]
    pub fn seak_special_hi_travel_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mag = self.seak_stick_mag(gobj);
        if !(mag < reg(self.seak_attr_f32(f, attr::X40))) {
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
                let x44 = reg(self.seak_attr_f32(f, attr::X44));
                let x48 = reg(self.seak_attr_f32(f, attr::X48));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(mv::HI_VEL_X, reg(stick.x));
                fp.set_mv_f32(mv::HI_VEL_Y, reg(stick.y));
                let v = fp::fmuls(fp::fmadds(x44, mag, x48), reg(math::cos(a)));
                fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), v));
                self.change_motion_state(gobj, ms::SPECIAL_HI_START_1, 0, 35.0, 1.0, 0.0, None);
                self.anim_update(gobj);
                self.set_anim_rate(gobj, 0.0);
                self.seak_special_hi_travel_common(gobj);
                return;
            }
        }
        self.go_airborne_jumped(f);
        self.seak_special_hi_travel_air(gobj);
    }

    /// The travel through the air, `ftSk_SpecialHi_80113A30`: in the stick's direction when
    /// pushed far enough, else straight up.
    #[doc(alias = "ftSk_SpecialHi_80113A30")]
    pub fn seak_special_hi_travel_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.seak_stick_mag(gobj);
        let angle;
        if mag > reg(self.seak_attr_f32(f, attr::X40)) {
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
            fp.set_mv_f32(mv::HI_VEL_X, reg(s.x));
            fp.set_mv_f32(mv::HI_VEL_Y, reg(s.y));
        } else {
            self.seak_ext(ext::FTCOMMON_8007DA24, &[Val::Fighter(f)]);
            angle = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::HI_VEL_X, 0.0);
            fp.set_mv_f32(mv::HI_VEL_Y, 1.0);
            mag = 1.0;
        }
        let x44 = reg(self.seak_attr_f32(f, attr::X44));
        let x48 = reg(self.seak_attr_f32(f, attr::X48));
        let speed = fp::fmadds(x44, mag, x48);
        let fp = &mut self.game.fighters[f.0];
        let c = reg(math::cos(mem(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(speed, c)));
        let s = reg(math::sin(mem(angle)));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START_1, 0, 35.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        self.seak_special_hi_travel_common(gobj);
    }

    /// `ftSk_SpecialHi_Anim`: standing once the reappearance ends.
    #[doc(alias = "ftSk_SpecialHi_Anim")]
    pub fn seak_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSk_SpecialAirHi_Anim`: falling helplessly once it ends.
    #[doc(alias = "ftSk_SpecialAirHi_Anim")]
    pub fn seak_special_air_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let x = self.seak_attr_f32(f, attr::X58);
            let y = self.seak_attr_f32(f, attr::X5C);
            self.fall_special(gobj, x, y);
        }
    }

    /// `ftSk_SpecialAirHi_Phys`: once the script says (`cmd_vars[0]`), falling with the drift
    /// held to `x4C` of the most; before, the rise slowing by a tenth each frame.
    #[doc(alias = "ftSk_SpecialAirHi_Phys")]
    pub fn seak_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.seak_ext(ext::FTCOMMON_FALL_BASIC, &[Val::Fighter(f)]);
            let x4c = reg(self.seak_attr_f32(f, attr::X4C));
            let max = reg(self.game.fighters[f.0].co_attrs.air_drift_max);
            let limit = mem(fp::fmuls(x4c, max));
            self.seak_ext(
                ext::FTCOMMON_CLAMP_SELF_VEL_X,
                &[Val::Fighter(f), Val::Float(reg(limit))],
            );
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let vy = reg(fp.self_vel.y);
        fp.self_vel.y = mem(fp::fsubs(vy, fp::fdivs(vy, 10.0)));
        self.seak_ext(
            ext::FTCOMMON_CALC_SELF_ACCEL_DEACCEL_AIR,
            &[Val::Fighter(f)],
        );
    }

    /// `ftSk_SpecialHi_Coll`: off the floor, the aerial reappearance.
    #[doc(alias = "ftSk_SpecialHi_Coll")]
    pub fn seak_special_hi_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.seak_special_hi_end_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirHi_Coll`: landing helplessly (`x5C` frames); else a ledge.
    #[doc(alias = "ftSk_SpecialAirHi_Coll")]
    pub fn seak_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let lag = self.seak_attr_f32(f, attr::X5C);
            self.enter_landing_fall_special(gobj, 0, lag);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// `ftSk_SpecialHi_80113E40`.
    #[doc(alias = "ftSk_SpecialHi_80113E40")]
    pub fn seak_special_hi_end_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI,
            END_FALL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_FN_80113038));
    }

    /// The travel's speed kept (`mv.co` `x10`, `x14`, `x18`) and stopped, visible again, the
    /// reappearance's effect to come.
    fn seak_special_hi_appear_common(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy, gv) = (fp.self_vel.x, fp.self_vel.y, fp.gr_vel);
        fp.set_mv_f32(mv::CO_X10, reg(vx));
        fp.set_mv_f32(mv::CO_X14, reg(vy));
        fp.set_mv_f32(mv::CO_X18, reg(gv));
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
        fp.invisible = false;
        fp.accessory4_cb = Some(CodeRef(addr::HI_FN_80113038));
    }

    /// Reappearing on the ground, `ftSk_SpecialHi_80113EAC`: a share (`x54`) of the travel's
    /// speed kept.
    #[doc(alias = "ftSk_SpecialHi_80113EAC")]
    pub fn seak_special_hi_appear_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.seak_special_hi_appear_common(gobj);
        let x54 = reg(self.seak_attr_f32(f, attr::X54));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(reg(fp.mv_f32(mv::CO_X18)), x54));
    }

    /// Reappearing in the air, `ftSk_SpecialHi_80113F68`.
    #[doc(alias = "ftSk_SpecialHi_80113F68")]
    pub fn seak_special_hi_appear_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.seak_special_hi_appear_common(gobj);
        let x54 = reg(self.seak_attr_f32(f, attr::X54));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(mv::CO_X10)), x54));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.mv_f32(mv::CO_X14)), x54));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_FN_80112ED8 => g.seak_special_hi_vanish(gobj),
        addr::HI_FN_80113038 => g.seak_special_hi_appear_gfx(gobj),
        0x8011_2F48 => g.seak_special_hi_spawn_flash(gobj),
        0x8011_2FA8 => g.seak_special_hi_vanish_gfx(gobj),
        addr::SPECIAL_HI_ENTER => g.seak_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.seak_special_air_hi_enter(gobj),
        0x8011_3194 => g.seak_special_hi_start_0_anim(gobj),
        0x8011_31D0 => g.seak_special_air_hi_start_0_anim(gobj),
        0x8011_320C | 0x8011_3210 | 0x8011_3474 | 0x8011_3478 | 0x8011_349C | 0x8011_3CDC
        | 0x8011_3CE0 => g.seak_special_hi_nothing(gobj),
        0x8011_3214 | 0x8011_3CE4 => g.seak_ground_phys(gobj),
        0x8011_3234 => g.seak_special_air_hi_start_0_phys(gobj),
        0x8011_3278 => g.seak_special_hi_start_0_coll(gobj),
        0x8011_32B4 => g.seak_special_air_hi_start_0_coll(gobj),
        0x8011_3324 => g.seak_special_hi_start_0_to_air(gobj),
        0x8011_3390 => g.seak_special_hi_start_0_to_ground(gobj),
        0x8011_33FC => g.seak_special_hi_start_1_anim(gobj),
        0x8011_3438 => g.seak_special_air_hi_start_1_anim(gobj),
        0x8011_347C => g.seak_special_hi_start_1_phys(gobj),
        0x8011_34A0 => g.seak_special_hi_start_1_coll(gobj),
        0x8011_3540 => g.seak_special_air_hi_start_1_coll(gobj),
        0x8011_374C => g.seak_special_hi_travel_to_air(gobj),
        0x8011_37C8 => g.seak_special_hi_travel_to_ground(gobj),
        0x8011_3838 => g.seak_special_hi_travel_ground(gobj),
        0x8011_3A30 => g.seak_special_hi_travel_air(gobj),
        0x8011_3C40 => g.seak_special_hi_anim(gobj),
        0x8011_3C7C => g.seak_special_air_hi_anim(gobj),
        0x8011_3D04 => g.seak_special_air_hi_phys(gobj),
        0x8011_3D80 => g.seak_special_hi_coll(gobj),
        0x8011_3DBC => g.seak_special_air_hi_coll(gobj),
        0x8011_3E40 => g.seak_special_hi_end_to_air(gobj),
        0x8011_3EAC => g.seak_special_hi_appear_ground(gobj),
        0x8011_3F68 => g.seak_special_hi_appear_air(gobj),
        _ => return false,
    }
    true
}
