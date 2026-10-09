//! Farore's Wind (`ftzeldaspecialhi.c`): the wind-up, the invisible travel in the stick's
//! direction (along the floor on the ground), and the reappearance, as Sheik's Vanish without
//! its flash.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::super::motion::flags::*;
use super::{GA_GROUND, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `FtPart_TopN`, `FtPart_HipN`: Zelda's code indexes `fp->parts` by the part directly, but
/// for the wind, which takes part 4 through `ftParts_GetBoneIndex`.
const PART_TOP_N: i32 = 0;
const PART_HIP_N: i32 = 4;

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

/// The wind-up's effect on the ground and in the air, and the reappearance's.
const GFX_START_GROUND: i32 = 1270;
const GFX_START_AIR: i32 = 1271;
const GFX_APPEAR: i32 = 1285;

/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

impl Running<'_> {
    /// The hip's position in the world (`lb_8000B1CC`), which also brings its matrix up to
    /// date.
    fn zelda_hip_pos(&mut self, gobj: GObjId) -> crate::vec::Vec3 {
        let j = self.bone_joint(gobj, PART_HIP_N);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// `ftZd_SpecialHi_801396AC` (an accessory): the wind-up's effect, then no more
    /// accessory.
    // Ports `ftZd_SpecialHi_801396AC`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_gfx_once(&mut self, gobj: GObjId) {
        self.zelda_special_hi_start_gfx(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The wind-up's effect on the top joint, once (`x2219_b0`), `ftZd_SpecialHi_801396E0`:
    /// the hip's position is taken first (and not used).
    // Ports `ftZd_SpecialHi_801396E0`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let _ = self.zelda_hip_pos(gobj);
            let gfx = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
                GFX_START_GROUND
            } else {
                GFX_START_AIR
            };
            let j = self.bone_joint(gobj, PART_TOP_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The reappearance's effect at the hip, once, `ftZd_SpecialHi_8013979C` (an accessory).
    // Ports `ftZd_SpecialHi_8013979C`; aliased once a check reaches it.
    pub fn zelda_special_hi_appear_gfx(&mut self, gobj: GObjId) {
        let pos = self.zelda_hip_pos(gobj);
        if !self.game.fighter(gobj).x2219_b0 {
            spawn(self, GFX_APPEAR, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The wind at the fighter's part 4 (`ftParts_GetBoneIndex`), `lb_800119DC(pos, 120,
    /// 1.5, 0.02, 60°)`.
    fn zelda_special_hi_wind(&mut self, gobj: GObjId) {
        let b = self.bone_index(self.game.fighter(gobj), 4);
        let j = self.bone_joint(gobj, b);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        // `60 * (float) M_PI / 180`, folded in single precision.
        let a = 60.0_f32 * std::f32::consts::PI / 180.0;
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, 120, 1.5, 0.02, a);
    }

    /// The grounded wind-up, `ftZd_SpecialHi_Enter`: stopped.
    // Ports `ftZd_SpecialHi_Enter`; aliased once a check reaches it.
    pub fn zelda_special_hi_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_HI_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_XC, 0);
        self.zelda_special_hi_wind(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_801396AC));
    }

    /// The aerial wind-up, `ftZd_SpecialAirHi_Enter`: the speeds divided down (`x38`, `x3C`).
    // Ports `ftZd_SpecialAirHi_Enter`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x38 = reg(self.zelda_attr_f32(f, attr::X38));
        let x3c = reg(self.zelda_attr_f32(f, attr::X3C));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x38));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), x3c));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::HI_XC, 0);
        self.zelda_special_hi_wind(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_801396AC));
    }

    /// `ftZd_SpecialHiStart_0_Anim`: the travel along the floor once the wind-up ends.
    // Ports `ftZd_SpecialHiStart_0_Anim`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.zelda_special_hi_travel_ground(gobj);
        }
    }

    /// `ftZd_SpecialAirHiStart_0_Anim`: the travel through the air.
    // Ports `ftZd_SpecialAirHiStart_0_Anim`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_start_0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.zelda_special_hi_travel_air(gobj);
        }
    }

    /// The states' empty callbacks.
    // Ports `ftZd_SpecialHiStart_0_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirHiStart_0_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialHiStart_1_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirHiStart_1_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialHi_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirHi_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirHiStart_1_Phys`; aliased once a check reaches it.
    pub fn zelda_special_hi_nothing(&mut self, _gobj: GObjId) {}

    /// `ftZd_SpecialAirHiStart_0_Phys`: the wind-up's own fall (`x40`, `x44`), slowing.
    // Ports `ftZd_SpecialAirHiStart_0_Phys`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_start_0_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.zelda_attr_f32(f, attr::X40));
        let t = reg(self.zelda_attr_f32(f, attr::X44));
        Self::fall(&mut self.game.fighters[f.0], g, t);
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftZd_SpecialHiStart_0_Coll`: off the ground, the aerial wind-up.
    // Ports `ftZd_SpecialHiStart_0_Coll`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.zelda_special_hi_start_0_to_air(gobj);
        }
    }

    /// `ftZd_SpecialAirHiStart_0_Coll`: landing, the grounded wind-up; else a ledge.
    // Ports `ftZd_SpecialAirHiStart_0_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_start_0_coll(&mut self, gobj: GObjId) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            self.zelda_special_hi_start_0_to_ground(gobj);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// `ftZd_SpecialHi_80139B44`.
    // Ports `ftZd_SpecialHi_80139B44`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_0_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START_0,
            TRANSITION_FLAGS_1,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_801396AC));
    }

    /// `ftZd_SpecialHi_80139BB0`.
    // Ports `ftZd_SpecialHi_80139BB0`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_0_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI_START_0, TRANSITION_FLAGS_1);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_801396AC));
    }

    /// The travel's countdown (`specialhi.x0`); whether it ran out.
    fn zelda_special_hi_count_down(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_X0).wrapping_sub(1);
        fp.set_mv_i32(mv::HI_X0, n);
        n <= 0
    }

    /// `ftZd_SpecialHiStart_1_Anim`: the reappearance on the ground once the travel ends.
    // Ports `ftZd_SpecialHiStart_1_Anim`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_1_anim(&mut self, gobj: GObjId) {
        if self.zelda_special_hi_count_down(gobj) {
            self.zelda_special_hi_appear_ground(gobj);
        }
    }

    /// `ftZd_SpecialAirHiStart_1_Anim`: the reappearance in the air.
    // Ports `ftZd_SpecialAirHiStart_1_Anim`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_start_1_anim(&mut self, gobj: GObjId) {
        if self.zelda_special_hi_count_down(gobj) {
            self.zelda_special_hi_appear_air(gobj);
        }
    }

    /// `ftZd_SpecialHiStart_1_Phys`: the travel along the floor.
    // Ports `ftZd_SpecialHiStart_1_Phys`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_1_phys(&mut self, gobj: GObjId) {
        self.self_movement_from_ground(gobj);
    }

    /// `ftZd_SpecialHiStart_1_Coll`: off the floor, the travel goes on in the air (or Zelda
    /// reappears there against a wall); along it, she reappears against a wall.
    // Ports `ftZd_SpecialHiStart_1_Coll`; aliased once a check reaches it.
    pub fn zelda_special_hi_start_1_coll(&mut self, gobj: GObjId) {
        let walls = |r: &Running| {
            r.game.fighter(gobj).coll_data.env_flags & (LEFT_WALL_MASK | RIGHT_WALL_MASK) != 0
        };
        if !self.ground_coll_off_ledges(gobj) {
            if walls(self) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne_jumped(f);
                self.zelda_special_hi_appear_air(gobj);
                return;
            }
            self.zelda_special_hi_travel_to_air(gobj);
            return;
        }
        if walls(self) {
            self.zelda_special_hi_appear_ground(gobj);
        }
    }

    /// `ftZd_SpecialAirHiStart_1_Coll`: meeting the ground (after `x4C` frames, or where she
    /// can't pass through), the travel goes on along it; else a ledge, or reappearing against
    /// a surface met head on (`ftCommon_HandleTeleportCollisions`).
    // Ports `ftZd_SpecialAirHiStart_1_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_start_1_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::HI_XC).wrapping_add(1);
        fp.set_mv_i32(mv::HI_XC, n);
        let dir = facing_dir_int(fp.facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let frames = self.game.fighters[f.0].mv_i32(mv::HI_XC);
            let land = if fp::frsp(f64::from(frames)) >= reg(self.zelda_attr_f32(f, attr::X4C)) {
                true
            } else {
                !self.pass_skip_floor(gobj)
            };
            if land {
                self.zelda_special_hi_travel_to_ground(gobj);
                return;
            }
        }
        if !self.catch_ledge_check(gobj) {
            self.zelda_teleport_collisions(gobj);
        }
    }

    /// `ftCommon_HandleTeleportCollisions` with the attributes' angle (`x60`): Zelda
    /// reappears for each of the ceiling and walls she meets more head on than that.
    fn zelda_teleport_collisions(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let clamp = self.zelda_attr_i32(f, attr::X60);
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
                self.zelda_special_hi_appear_air(gobj);
            }
        }
    }

    /// `ftZd_SpecialHi_80139F6C`: the travel off the floor, on through the air.
    // Ports `ftZd_SpecialHi_80139F6C`; aliased once a check reaches it.
    pub fn zelda_special_hi_travel_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_START_1,
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

    /// `ftZd_SpecialHi_80139FE8`: the travel onto the floor, on along it.
    // Ports `ftZd_SpecialHi_80139FE8`; aliased once a check reaches it.
    pub fn zelda_special_hi_travel_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_HI_START_1,
            TRANSITION_FLAGS_1,
            frame,
            0.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).invisible = true;
    }

    /// The stick's distance from center, at most 1.
    fn zelda_stick_mag(&self, gobj: GObjId) -> f64 {
        let s = self.game.fighter(gobj).input.lstick[0].clone();
        let x = fp::fmuls(reg(s.x), reg(s.x));
        let y = fp::fmuls(reg(s.y), reg(s.y));
        let mag = reg(math::sqrt(mem(fp::fadds(x, y))));
        if mag > 1.0 { 1.0 } else { mag }
    }

    /// The travel's frames (`x48`), the jumps used up, intangible and invisible.
    fn zelda_special_hi_travel_common(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.zelda_attr_i32(f, attr::X48);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_X0, frames);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.x2223_b4 = true;
        self.set_hit_status(gobj, 2);
        self.game.fighters[f.0].invisible = true;
    }

    /// The travel along the floor, `ftZd_SpecialHi_8013A058`: with the stick pushed far
    /// enough (`x50`), away from the floor's normal and not down through it, along the floor
    /// at its speed; else the travel through the air.
    // Ports `ftZd_SpecialHi_8013A058`; aliased once a check reaches it.
    pub fn zelda_special_hi_travel_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mag = self.zelda_stick_mag(gobj);
        if !(mag < reg(self.zelda_attr_f32(f, attr::X50))) {
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
                let x54 = reg(self.zelda_attr_f32(f, attr::X54));
                let x58 = reg(self.zelda_attr_f32(f, attr::X58));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(mv::HI_X4_X, reg(stick.x));
                fp.set_mv_f32(mv::HI_X4_Y, reg(stick.y));
                let v = fp::fmuls(fp::fmadds(x54, mag, x58), reg(math::cos(a)));
                fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), v));
                self.change_motion_state(gobj, ms::SPECIAL_HI_START_1, 0, 35.0, 1.0, 0.0, None);
                self.anim_update(gobj);
                self.set_anim_rate(gobj, 0.0);
                self.zelda_special_hi_travel_common(gobj);
                return;
            }
        }
        self.go_airborne_jumped(f);
        self.zelda_special_hi_travel_air(gobj);
    }

    /// The travel through the air, `ftZd_SpecialHi_8013A244`: in the stick's direction when
    /// pushed far enough, else straight up.
    // Ports `ftZd_SpecialHi_8013A244`; aliased once a check reaches it.
    pub fn zelda_special_hi_travel_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.zelda_stick_mag(gobj);
        let angle;
        if mag > reg(self.zelda_attr_f32(f, attr::X50)) {
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
            fp.set_mv_f32(mv::HI_X4_X, reg(s.x));
            fp.set_mv_f32(mv::HI_X4_Y, reg(s.y));
        } else {
            self.face_stick_if_pushed(gobj);
            angle = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::HI_X4_X, 0.0);
            fp.set_mv_f32(mv::HI_X4_Y, 1.0);
            mag = 1.0;
        }
        let x54 = reg(self.zelda_attr_f32(f, attr::X54));
        let x58 = reg(self.zelda_attr_f32(f, attr::X58));
        let speed = fp::fmadds(x54, mag, x58);
        let fp = &mut self.game.fighters[f.0];
        let c = reg(math::cos(mem(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(speed, c)));
        let s = reg(math::sin(mem(angle)));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START_1, 0, 35.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        self.zelda_special_hi_travel_common(gobj);
    }

    /// `ftZd_SpecialHi_Anim`: standing once the reappearance ends.
    // Ports `ftZd_SpecialHi_Anim`; aliased once a check reaches it.
    pub fn zelda_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftZd_SpecialAirHi_Anim`: falling helplessly once it ends (`x68`, `x6C`).
    // Ports `ftZd_SpecialAirHi_Anim`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let x = self.zelda_attr_f32(f, attr::X68);
            let y = self.zelda_attr_f32(f, attr::X6C);
            self.fall_special(gobj, x, y);
        }
    }

    /// `ftZd_SpecialAirHi_Phys`: once the script says (`cmd_vars[0]`), falling with the drift
    /// held to `x5C` of the most; before, the rise slowing by a tenth each frame.
    // Ports `ftZd_SpecialAirHi_Phys`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            Self::fall_basic(&mut self.game.fighters[f.0]);
            let x5c = reg(self.zelda_attr_f32(f, attr::X5C));
            let max = reg(self.game.fighters[f.0].co_attrs.air_drift_max);
            let limit = fp::fmuls(x5c, max);
            Self::clamp_self_vel_x(&mut self.game.fighters[f.0], limit);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let vy = reg(fp.self_vel.y);
        fp.self_vel.y = mem(fp::fsubs(vy, fp::fdivs(vy, 10.0)));
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftZd_SpecialHi_Coll`: off the floor, the aerial reappearance.
    // Ports `ftZd_SpecialHi_Coll`; aliased once a check reaches it.
    pub fn zelda_special_hi_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.zelda_special_hi_end_to_air(gobj);
        }
    }

    /// `ftZd_SpecialAirHi_Coll`: landing helplessly (`x6C` frames); else a ledge.
    // Ports `ftZd_SpecialAirHi_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let lag = self.zelda_attr_f32(f, attr::X6C);
            self.enter_landing_fall_special(gobj, 0, lag);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// `ftZd_SpecialHi_8013A648`.
    // Ports `ftZd_SpecialHi_8013A648`; aliased once a check reaches it.
    pub fn zelda_special_hi_end_to_air(&mut self, gobj: GObjId) {
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

    /// The travel's speed kept (`specialhi.x10`, `x18`) and stopped, visible again, the
    /// reappearance's effect to come.
    fn zelda_special_hi_appear_common(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy, gv) = (fp.self_vel.x, fp.self_vel.y, fp.gr_vel);
        fp.set_mv_f32(mv::HI_X10_X, reg(vx));
        fp.set_mv_f32(mv::HI_X10_Y, reg(vy));
        fp.set_mv_f32(mv::HI_X18, reg(gv));
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
        fp.invisible = false;
        fp.accessory4_cb = Some(CodeRef(addr::HI_8013979C));
    }

    /// Reappearing on the ground, `ftZd_SpecialHi_8013A6A8`: a share (`x64`) of the travel's
    /// speed kept.
    // Ports `ftZd_SpecialHi_8013A6A8`; aliased once a check reaches it.
    pub fn zelda_special_hi_appear_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_hi_appear_common(gobj);
        let x64 = reg(self.zelda_attr_f32(f, attr::X64));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_X18)), x64));
    }

    /// Reappearing in the air, `ftZd_SpecialHi_8013A764`.
    // Ports `ftZd_SpecialHi_8013A764`; aliased once a check reaches it.
    pub fn zelda_special_hi_appear_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_hi_appear_common(gobj);
        let x64 = reg(self.zelda_attr_f32(f, attr::X64));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_X10_X)), x64));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.mv_f32(mv::HI_X10_Y)), x64));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_801396AC => g.zelda_special_hi_start_gfx_once(gobj),
        0x8013_96E0 => g.zelda_special_hi_start_gfx(gobj),
        addr::HI_8013979C => g.zelda_special_hi_appear_gfx(gobj),
        addr::SPECIAL_HI_ENTER => g.zelda_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.zelda_special_air_hi_enter(gobj),
        0x8013_99B4 => g.zelda_special_hi_start_0_anim(gobj),
        0x8013_99F0 => g.zelda_special_air_hi_start_0_anim(gobj),
        0x8013_9A2C | 0x8013_9A30 | 0x8013_9C94 | 0x8013_9C98 | 0x8013_9CBC | 0x8013_A4E4
        | 0x8013_A4E8 => g.zelda_special_hi_nothing(gobj),
        0x8013_9A34 | 0x8013_A4EC => g.seak_ground_phys(gobj),
        0x8013_9A54 => g.zelda_special_air_hi_start_0_phys(gobj),
        0x8013_9A98 => g.zelda_special_hi_start_0_coll(gobj),
        0x8013_9AD4 => g.zelda_special_air_hi_start_0_coll(gobj),
        0x8013_9B44 => g.zelda_special_hi_start_0_to_air(gobj),
        0x8013_9BB0 => g.zelda_special_hi_start_0_to_ground(gobj),
        0x8013_9C1C => g.zelda_special_hi_start_1_anim(gobj),
        0x8013_9C58 => g.zelda_special_air_hi_start_1_anim(gobj),
        0x8013_9C9C => g.zelda_special_hi_start_1_phys(gobj),
        0x8013_9CC0 => g.zelda_special_hi_start_1_coll(gobj),
        0x8013_9D60 => g.zelda_special_air_hi_start_1_coll(gobj),
        0x8013_9F6C => g.zelda_special_hi_travel_to_air(gobj),
        0x8013_9FE8 => g.zelda_special_hi_travel_to_ground(gobj),
        0x8013_A058 => g.zelda_special_hi_travel_ground(gobj),
        0x8013_A244 => g.zelda_special_hi_travel_air(gobj),
        0x8013_A448 => g.zelda_special_hi_anim(gobj),
        0x8013_A484 => g.zelda_special_air_hi_anim(gobj),
        0x8013_A50C => g.zelda_special_air_hi_phys(gobj),
        0x8013_A588 => g.zelda_special_hi_coll(gobj),
        0x8013_A5C4 => g.zelda_special_air_hi_coll(gobj),
        0x8013_A648 => g.zelda_special_hi_end_to_air(gobj),
        0x8013_A6A8 => g.zelda_special_hi_appear_ground(gobj),
        0x8013_A764 => g.zelda_special_hi_appear_air(gobj),
        _ => return false,
    }
    true
}
