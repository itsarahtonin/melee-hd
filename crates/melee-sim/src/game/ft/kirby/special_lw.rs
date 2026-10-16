//! Stone (`ftkirbyspeciallw.c`): Kirby turns into one of his five stones (drawn from his
//! previous one), hard to knock back for the attributes' hits; dropping in the air he lands with
//! a quake, then slides down slopes steep enough by the floor's averaged slope over the last
//! four frames, until his time in stone runs out or he lets go (B after the shortest time).
//!
//! The state's variables (`ftKb_MotionVars`) are read through the decomp's two views of the
//! same union, `specialhi` and `speciallw`, and two words through the common view.

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::ef::{self, Arg, eflib};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::game::structs::Fighter;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// `QuakeKind_Large`.
const QUAKE_LARGE: i32 = 4;

/// `FtPart_YRotN`.
const PART_Y_ROT_N: i32 = 3;

/// `ftKb_Init_803CB490`: for each frame of the change into and out of stone, whether he shows
/// as a stone.
const SHOW_STONE: [bool; 23] = [
    false, true, false, false, false, false, true, true, false, false, false, true, true, true,
    false, false, true, true, true, true, false, true, true,
];

/// `ftKb_Init_803CB4EC`: each stone's model variant, by its number.
const STONE_VARIANTS: [i32; 6] = [0, 2, 3, 4, 5, 6];

/// What his stone's state changes keep (`0x04440012`, `0x0C4C5092`, `0x0C4C1092`).
const STONE_FLAGS: u32 = 0x0444_0012;
const LAND_FLAGS: u32 = 0x0C4C_5092;
const LAND_HOLD_FLAGS: u32 = 0x0C4C_1092;

/// The state's variables: the frames left in stone (`speciallw.x0`, an `s16`) and the
/// shortest time (`x2`); the change's direction (`specialhi.x8`, 1 into stone, 2 out of it),
/// whether he turned into one (`xC`), the stone's number (`x10`) and the change's frame
/// (`x14`); the floor's normal (`x18`) and slope (`x84`, `specialhi.xC4`), the last four
/// normals (`speciallw.x24`) and their running averages (`x54`), the last four slopes and their
/// averages (`x88[0..8]`), and the slide's share (`x88[8]`).
mod mv {
    pub const FRAMES: usize = 0x0;
    pub const MIN_FRAMES: usize = 0x2;
    pub const X4: usize = 0x4;
    pub const X6: usize = 0x6;
    pub const CHANGE: usize = 0x8;
    pub const STONE: usize = 0xC;
    pub const NUMBER: usize = 0x10;
    pub const CHANGE_FRAME: usize = 0x14;
    pub const NORMAL: usize = 0x18;
    pub const NORMALS: usize = 0x24;
    pub const AVERAGES: usize = 0x54;
    pub const SLOPE: usize = 0x84;
    pub const SLOPES: usize = 0x88;
    /// `mv.co.common.x18`, `x4C.z` and `x58.x`: the floor normal's x, and the averaged
    /// normal's x and y.
    pub const NORMAL_X: usize = 0x18;
    pub const AVERAGE_X: usize = 0x54;
    pub const AVERAGE_Y: usize = 0x58;
}

/// `(0, 1, 0)`: the upright normal (`ftKb_Init_803CB4EC.vec`).
const UP: Vec3 = Vec3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

fn mv_vec(fp: &Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

fn set_mv_vec(fp: &mut Fighter, off: usize, v: Vec3) {
    fp.set_mv_f32(off, reg(v.x));
    fp.set_mv_f32(off + 4, reg(v.y));
    fp.set_mv_f32(off + 8, reg(v.z));
}

fn mv_i16(fp: &Fighter, off: usize) -> i16 {
    i16::from_be_bytes([fp.mv[off], fp.mv[off + 1]])
}

fn set_mv_i16(fp: &mut Fighter, off: usize, v: i16) {
    fp.mv[off..off + 2].copy_from_slice(&v.to_be_bytes());
}

fn slope(fp: &Fighter, i: usize) -> f64 {
    reg(fp.mv_f32(mv::SLOPES + 4 * i))
}

fn set_slope(fp: &mut Fighter, i: usize, v: f64) {
    fp.set_mv_f32(mv::SLOPES + 4 * i, v);
}

/// C's `ABS` on a float register.
fn abs(x: f64) -> f64 {
    if x < fp::frsp(0.0) { fp::fneg(x) } else { x }
}

impl Running<'_> {
    /// His cleanups when hit or KO'd (`ftKb_SpecialN_set_cbs2`, `Fighter_SetDamageCallbacks`).
    fn kb_lw_set_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::DEATH1));
        fp.death2_cb = Some(CodeRef(addr::DEATH3));
    }

    /// The change into or out of stone a frame on, `ftKb_SpecialHi_800F331C`: on its frames
    /// that show the stone, the stone's variant, his parts and item hidden; else Kirby.
    #[doc(alias = "ftKb_SpecialHi_800F331C")]
    pub fn kb_lw_show_stone(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let frame = fp.mv_i32(mv::CHANGE_FRAME);
        match fp.mv_i32(mv::CHANGE) {
            1 => fp.set_mv_i32(mv::CHANGE_FRAME, frame.wrapping_add(1)),
            2 => fp.set_mv_i32(mv::CHANGE_FRAME, frame.wrapping_sub(1)),
            _ => {}
        }
        let frame = fp.mv_i32(mv::CHANGE_FRAME);
        if !(0..0x16).contains(&frame) {
            fp.set_mv_i32(mv::CHANGE, 0);
            return;
        }
        if !SHOW_STONE[frame as usize] {
            self.set_model_variant(f, 0, 0);
            self.set_model_variant(f, 1, 0);
            self.set_item_visible(gobj, true);
            let fp = &mut self.game.fighters[f.0];
            fp.x221E_b4 = true;
            fp.x2225_b2 = true;
        } else {
            let n = self.game.fighters[f.0].mv_i32(mv::NUMBER);
            self.set_model_variant(f, 0, STONE_VARIANTS[n as usize]);
            self.set_model_variant(f, 1, -1);
            self.set_item_visible(gobj, false);
            let fp = &mut self.game.fighters[f.0];
            fp.x221E_b4 = false;
            fp.x2225_b2 = false;
        }
    }

    /// The script's change (`cmd_vars[0]`), `ftKb_SpecialHi_800F346C`: 1 starts the change
    /// into stone once (his hits' resistance, the next stone after his last), 2 the change
    /// out of it.
    #[doc(alias = "ftKb_SpecialHi_800F346C")]
    pub fn kb_lw_script(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let hp = self.kb_attr_i32(f, attr::LW_HP);
        let fp = &self.game.fighters[f.0];
        match fp.cmd_vars[0] {
            1 => {
                if fp.mv_i32(mv::CHANGE) == 0 && fp.mv_i32(mv::STONE) == 0 {
                    let c = fp.cmd_vars[0] as i32;
                    let draw = self.game.objs.rng.below(4);
                    let fp = &mut self.game.fighters[f.0];
                    fp.set_mv_i32(mv::CHANGE, c);
                    fp.dmg.x1834 = mem(fp::frsp(f64::from(hp)));
                    let n = (fp.u_i32(u::HAT_X4).wrapping_add(draw)) % 5 + 1;
                    fp.set_mv_i32(mv::NUMBER, n);
                    fp.set_u_i32(u::HAT_X4, n);
                    fp.set_mv_i32(mv::CHANGE_FRAME, 0);
                    fp.set_mv_i32(mv::STONE, 1);
                }
            }
            2 if fp.mv_i32(mv::CHANGE) == 0 && fp.mv_i32(mv::STONE) == 1 => {
                let c = fp.cmd_vars[0] as i32;
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_i32(mv::CHANGE, c);
                fp.set_mv_i32(mv::CHANGE_FRAME, 0x16);
            }
            _ => {}
        }
        self.game.fighters[f.0].cmd_vars[0] = 0;
    }

    /// The floor under him, `ftKb_SpecialHi_800F3570`: its normal and slope his way; on a
    /// slope, a check down the slope's way for the floor continuing, and none or one sloping
    /// the other way stops him flat.
    #[doc(alias = "ftKb_SpecialHi_800F3570")]
    pub fn kb_lw_floor(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let speed = reg(self.kb_attr_f32(f, attr::LW_SLIDE_MAX));
        let fp = &mut self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        set_mv_vec(fp, mv::NORMAL, n);
        let a = fp::fmuls(reg(fp.facing_dir), reg(math::atan2(n.x, n.y)));
        fp.set_mv_f32(mv::SLOPE, a);
        if reg(n.x) == 0.0 {
            return;
        }
        let y = reg(fp.cur_pos.y);
        let x = reg(fp.cur_pos.x);
        let top = fp::fadds(y, speed);
        let sign = if reg(n.x) > 0.0 { 1.0 } else { -1.0 };
        let bottom = fp::fsubs(y, speed);
        let ny = fp::fmuls(reg(n.y), speed);
        let x1 = fp::fmadds(ny, sign, x);
        let g = self.game.geometry.as_mut().expect("no collision geometry");
        let hit = g.check_all_remap(-1, -1, mem(x), mem(top), mem(x1), mem(bottom));
        let found = hit.map_or(UP, |h| h.normal);
        let fp = &mut self.game.fighters[f.0];
        let stop = if reg(n.x) > 0.0 {
            reg(found.x) < 0.0
        } else {
            reg(found.x) > 0.0
        };
        if stop {
            set_mv_vec(fp, mv::NORMAL, UP);
            fp.set_mv_f32(mv::SLOPE, 0.0);
            fp.xE4_ground_accel_1 = 0.0;
            fp.gr_vel = 0.0;
        }
    }

    /// The last four floors forgotten, `ftKb_SpecialHi_800F36DC`.
    #[doc(alias = "ftKb_SpecialHi_800F36DC")]
    pub fn kb_lw_clear_slopes(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        for i in 0..4 {
            set_mv_vec(fp, mv::NORMALS + 12 * i, UP);
            set_mv_vec(fp, mv::AVERAGES + 12 * i, UP);
            set_slope(fp, i, 0.0);
            set_slope(fp, i + 4, 0.0);
        }
        set_mv_vec(fp, mv::NORMAL, UP);
        fp.set_mv_f32(mv::SLOPE, 0.0);
    }

    /// The last four floors and slopes on a frame, `ftKb_SpecialHi_800F37EC`: their averages,
    /// and the slide's share, cut where the slopes change direction or flatten.
    #[doc(alias = "ftKb_SpecialHi_800F37EC")]
    pub fn kb_lw_slopes(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let min = reg(self.kb_attr_f32(f, attr::LW_MIN_SLOPE));
        let fp = &mut self.game.fighters[f.0];
        for i in (1..4).rev() {
            let v = mv_vec(fp, mv::NORMALS + 12 * (i - 1));
            set_mv_vec(fp, mv::NORMALS + 12 * i, v);
            let v = mv_vec(fp, mv::AVERAGES + 12 * (i - 1));
            set_mv_vec(fp, mv::AVERAGES + 12 * i, v);
            let s = slope(fp, i - 1);
            set_slope(fp, i, s);
            let s = slope(fp, i + 3);
            set_slope(fp, i + 4, s);
        }
        let n = mv_vec(fp, mv::NORMAL);
        set_mv_vec(fp, mv::NORMALS, n);
        let s = reg(fp.mv_f32(mv::SLOPE));
        set_slope(fp, 0, s);
        let mut avg = (0.0f64, 0.0f64, 0.0f64);
        let mut sum = 0.0f64;
        set_mv_vec(fp, mv::AVERAGES, Vec3::new(0.0, 0.0, 0.0));
        set_slope(fp, 4, 0.0);
        for i in 0..4 {
            let v = mv_vec(fp, mv::NORMALS + 12 * i);
            avg.0 = fp::fadds(avg.0, reg(v.x));
            avg.1 = fp::fadds(avg.1, reg(v.y));
            avg.2 = fp::fadds(avg.2, reg(v.z));
            set_mv_vec(
                fp,
                mv::AVERAGES,
                Vec3::new(mem(avg.0), mem(avg.1), mem(avg.2)),
            );
            sum = fp::fadds(sum, slope(fp, i));
            set_slope(fp, 4, sum);
        }
        let mut v = mv_vec(fp, mv::AVERAGES);
        v.normalize();
        set_mv_vec(fp, mv::AVERAGES, v);
        let s = fp::fmuls(slope(fp, 4), 0.25);
        set_slope(fp, 4, s);
        set_slope(fp, 8, 1.0);
        let mut i: i32 = 3;
        while i > 0 {
            if abs(slope(fp, i as usize)) > min {
                let mut j = i;
                while j >= 0 {
                    let a = slope(fp, j as usize);
                    let mag = if a < 0.0 { fp::fneg(a) } else { a };
                    let cut = |fp: &mut Fighter| {
                        let s = fp::fsubs(slope(fp, 8), 0.25);
                        set_slope(fp, 8, s);
                        if slope(fp, 8) < 0.0 {
                            set_slope(fp, 8, 0.0);
                        }
                    };
                    if mag <= min {
                        set_slope(fp, 8, f64::from(0.9_f32));
                        j = -1;
                        i = -1;
                    } else if slope(fp, i as usize) > 0.0 {
                        if a < 0.0 {
                            cut(fp);
                        }
                    } else if a > 0.0 {
                        cut(fp);
                    }
                    j = j.wrapping_sub(1);
                }
            }
            i = i.wrapping_sub(1);
        }
    }

    /// The slide, `ftKb_SpecialHi_800F3B28`: leaning by the average slope; on one steep
    /// enough (up to the steepest the attributes allow), accelerating down it, else slowing by
    /// the floor's friction; the slide's share of both, his speed held to the attributes', as
    /// self movement along the averaged floor.
    #[doc(alias = "ftKb_SpecialHi_800F3B28")]
    pub fn kb_lw_slide(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let min = reg(self.kb_attr_f32(f, attr::LW_MIN_SLOPE));
        let max_slope = reg(self.kb_attr_f32(f, attr::LW_MAX_SLOPE));
        let accel = reg(self.kb_attr_f32(f, attr::LW_SLIDE_ACCEL));
        let max = reg(self.kb_attr_f32(f, attr::LW_SLIDE_MAX));
        let mut s = slope(&self.game.fighters[f.0], 4);
        self.game.part_set_rot_x(f, 0, mem(s));
        if abs(s) < min {
            s = 0.0;
        }
        if s > max_slope {
            s = max_slope;
        }
        if s < fp::fneg(max_slope) {
            s = fp::fneg(max_slope);
        }
        let mul = self.ground_friction_mul(&self.game.fighters[f.0]);
        let fp = &mut self.game.fighters[f.0];
        let ny = reg(fp.mv_f32(mv::AVERAGE_Y));
        let nx = reg(fp.mv_f32(mv::AVERAGE_X));
        let (add, brake);
        if abs(s) < f64::from(0.017_453_292_f32) {
            add = 0.0;
            let v = reg(fp.gr_vel);
            let friction = fp::fmuls(mul, fp::fmuls(ny, reg(fp.co_attrs.ground_friction)));
            let speed = abs(v);
            brake = if abs(friction) > speed {
                fp::fneg(v)
            } else if v > 0.0 {
                fp::fneg(friction)
            } else {
                friction
            };
        } else {
            brake = 0.0;
            add = fp::fmuls(fp::fadds(1.0, fp::fsubs(1.0, mul)), fp::fmuls(nx, accel));
        }
        let share = slope(fp, 8);
        fp.xE4_ground_accel_1 = mem(fp::fmuls(share, fp::fadds(add, brake)));
        fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), share));
        if reg(fp.gr_vel) < fp::fneg(max) {
            fp.gr_vel = mem(fp::fneg(max));
        } else if reg(fp.gr_vel) > max {
            fp.gr_vel = mem(max);
        }
        let sum = fp::fadds(reg(fp.gr_vel), reg(fp.xE4_ground_accel_1));
        if sum < fp::fneg(max) {
            fp.xE4_ground_accel_1 = mem(fp::fsubs(fp::fneg(max), reg(fp.gr_vel)));
        } else if fp::fadds(reg(fp.gr_vel), reg(fp.xE4_ground_accel_1)) > max {
            fp.xE4_ground_accel_1 = mem(fp::fsubs(max, reg(fp.gr_vel)));
        }
        let a = reg(fp.xE4_ground_accel_1);
        fp.x74_self_accel.x = mem(fp::fmuls(ny, a));
        fp.x74_self_accel.y = mem(fp::fmuls(fp::fneg(nx), a));
        fp.x74_self_accel.z = 0.0;
        let v = reg(fp.gr_vel);
        fp.self_vel.x = mem(fp::fmuls(ny, v));
        fp.self_vel.y = mem(fp::fmuls(fp::fneg(nx), v));
        fp.self_vel.z = 0.0;
    }

    fn kb_lw_enter(&mut self, gobj: GObjId, msid: i32) {
        let f = self.game.fighter_id(gobj);
        let j = self.bone_joint(gobj, PART_Y_ROT_N);
        let max_t = self.kb_attr_i32(f, attr::LW_MAX_TIME);
        let min_t = self.kb_attr_i32(f, attr::LW_MIN_TIME);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        set_mv_i16(fp, mv::FRAMES, max_t as i16);
        set_mv_i16(fp, mv::MIN_FRAMES, min_t as i16);
        set_mv_i16(fp, mv::X4, 0);
        set_mv_i16(fp, mv::X6, 0);
        fp.set_mv_i32(mv::CHANGE, 0);
        fp.set_mv_i32(mv::STONE, 0);
        fp.set_mv_i32(mv::NUMBER, -1);
        fp.set_mv_i32(mv::CHANGE_FRAME, -1);
        self.kb_lw_clear_slopes(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.x1834 = 0.0;
        fp.x221C_b4 = false;
        let keep = fp.x221C_b4;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x221C_b4 = keep;
        self.kb_lw_set_cbs(gobj);
        self.anim_update(gobj);
        ef::spawn(self, 0x49B, Some(gobj), &[Arg::JObj(j)]);
    }

    /// Stone from the ground, `ftKb_SpecialLw_Enter`.
    #[doc(alias = "ftKb_SpecialLw_Enter")]
    pub fn kb_special_lw_enter(&mut self, gobj: GObjId) {
        self.kb_lw_enter(gobj, ms::SPECIAL_LW1);
    }

    /// In the air, `ftKb_SpecialAirLw_Enter`.
    #[doc(alias = "ftKb_SpecialAirLw_Enter")]
    pub fn kb_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.kb_lw_enter(gobj, ms::SPECIAL_AIR_LW_START);
    }

    /// The script's change a frame on, the flag `b4` set to `val` first (`ftKbUnkInline`).
    fn kb_lw_step(&mut self, gobj: GObjId, val: bool) {
        self.game.fighter_mut(gobj).x221C_b4 = val;
        self.kb_lw_script(gobj);
        self.kb_lw_show_stone(gobj);
    }

    /// The change into stone, then the stone held still: `msid` once it ends.
    fn kb_lw_start_anim(&mut self, gobj: GObjId, msid: i32) {
        self.kb_lw_step(gobj, false);
        if !self.anim_frames_remaining(gobj) {
            let keep = self.game.fighter(gobj).x221C_b4;
            self.change_motion_state(gobj, msid, STONE_FLAGS, 0.0, 0.0, 0.0, None);
            self.game.fighter_mut(gobj).x221C_b4 = keep;
            self.set_anim_rate(gobj, 0.0);
        }
    }

    /// `ftKb_SpecialLw1_Anim`.
    #[doc(alias = "ftKb_SpecialLw1_Anim")]
    pub fn kb_special_lw1_anim(&mut self, gobj: GObjId) {
        self.kb_lw_start_anim(gobj, ms::SPECIAL_LW);
    }

    /// `ftKb_SpecialAirLwStart_Anim`.
    #[doc(alias = "ftKb_SpecialAirLwStart_Anim")]
    pub fn kb_special_air_lw_start_anim(&mut self, gobj: GObjId) {
        self.kb_lw_start_anim(gobj, ms::SPECIAL_AIR_LW);
    }

    /// The stone held, `ftKb_SpecialLw_Anim` and `ftKb_SpecialAirLw_Anim`.
    #[doc(alias = "ftKb_SpecialLw_Anim")]
    #[doc(alias = "ftKb_SpecialAirLw_Anim")]
    pub fn kb_special_lw_anim(&mut self, gobj: GObjId) {
        self.kb_lw_step(gobj, true);
    }

    /// The change back, `ftKb_SpecialLwEnd_Anim` and `ftKb_SpecialAirLwEnd_Anim`: at its end
    /// the resistance gone, falling, helplessly if the attributes say.
    #[doc(alias = "ftKb_SpecialLwEnd_Anim")]
    #[doc(alias = "ftKb_SpecialAirLwEnd_Anim")]
    pub fn kb_special_lw_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let toggle = self.kb_attr_f32(f, attr::LW_FREEFALL);
        self.kb_lw_step(gobj, false);
        if !self.anim_frames_remaining(gobj) {
            let fp = &mut self.game.fighters[f.0];
            fp.dmg.x1834 = 0.0;
            fp.x221C_b4 = false;
            if toggle == 0.0 {
                self.enter_fall(gobj);
                return;
            }
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, toggle, 0.0, true);
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftKb_SpecialLw1_IASA")]
    #[doc(alias = "ftKb_SpecialLwEnd_IASA")]
    #[doc(alias = "ftKb_SpecialAirLwStart_IASA")]
    #[doc(alias = "ftKb_SpecialAirLwEnd_IASA")]
    pub fn kb_special_lw_nothing(&mut self, _gobj: GObjId) {}

    /// Whether he leaves the stone (`fbKb_SpecialLw_IASA_Inline`): his time ran out, or B once
    /// past the shortest time; else a frame less.
    fn kb_lw_should_end(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let max_t = self.kb_attr_i32(f, attr::LW_MAX_TIME) as u32;
        let min_t = self.kb_attr_i32(f, attr::LW_MIN_TIME) as u32;
        let fp = &mut self.game.fighters[f.0];
        let frames = i32::from(mv_i16(fp, mv::FRAMES));
        if frames <= 0 {
            return true;
        }
        if frames <= max_t.wrapping_sub(min_t) as i32 && fp.input.pressed_buttons & 0x200 != 0 {
            return true;
        }
        if frames > 0 {
            set_mv_i16(fp, mv::FRAMES, (frames - 1) as i16);
        }
        false
    }

    /// Leaving the stone into `msid`.
    fn kb_lw_leave(&mut self, gobj: GObjId, msid: i32) {
        let j = self.bone_joint(gobj, PART_Y_ROT_N);
        let keep = self.game.fighter(gobj).x221C_b4;
        self.change_motion_state(gobj, msid, STONE_FLAGS, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x221C_b4 = keep;
        self.kb_lw_set_cbs(gobj);
        ef::spawn(self, 0x49C, Some(gobj), &[Arg::JObj(j)]);
    }

    /// `ftKb_SpecialLw_IASA`: leaving the stone on the ground goes airborne first.
    #[doc(alias = "ftKb_SpecialLw_IASA")]
    pub fn kb_special_lw_iasa(&mut self, gobj: GObjId) {
        if self.kb_lw_should_end(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            self.kb_lw_leave(gobj, ms::SPECIAL_LW_END);
        }
    }

    /// `ftKb_SpecialAirLw_IASA`.
    #[doc(alias = "ftKb_SpecialAirLw_IASA")]
    pub fn kb_special_air_lw_iasa(&mut self, gobj: GObjId) {
        if self.kb_lw_should_end(gobj) {
            self.kb_lw_leave(gobj, ms::SPECIAL_AIR_LW_END);
        }
    }

    /// `ftKb_SpecialLw1_Phys` (`ft_80084FA8`).
    #[doc(alias = "ftKb_SpecialLw1_Phys")]
    pub fn kb_special_lw1_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// `ftKb_SpecialLw_Phys`: friction, no rise, and the slide.
    #[doc(alias = "ftKb_SpecialLw_Phys")]
    pub fn kb_special_lw_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.kb_lw_slide(gobj);
    }

    /// The animation's motion (`ft_80085134`): `ftKb_SpecialLwEnd_Phys`,
    /// `ftKb_SpecialAirLwStart_Phys` and `ftKb_SpecialAirLwEnd_Phys`.
    #[doc(alias = "ftKb_SpecialLwEnd_Phys")]
    #[doc(alias = "ftKb_SpecialAirLwStart_Phys")]
    #[doc(alias = "ftKb_SpecialAirLwEnd_Phys")]
    pub fn kb_special_lw_anim_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// `ftKb_SpecialAirLw_Phys`: the animation's motion, dropping at the attributes' speed.
    #[doc(alias = "ftKb_SpecialAirLw_Phys")]
    pub fn kb_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = self.kb_attr_f32(f, attr::LW_GRAVITY);
        self.air_anim_motion(gobj);
        self.game.fighters[f.0].self_vel.y = mem(fp::fneg(reg(g)));
    }

    /// Off a ledge from the stone on the ground into `msid`, its floors forgotten
    /// (`frame` the animation frame it starts from, `hold` a held animation).
    fn kb_lw_off_ledge(&mut self, gobj: GObjId, msid: i32, flags: u32, from_frame: bool) {
        self.kb_lw_clear_slopes(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].self_vel.x = 0.0;
        self.go_airborne(f);
        let fp = &self.game.fighters[f.0];
        let keep = fp.x221C_b4;
        if from_frame {
            let frame = fp.cur_anim_frame;
            self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, None);
            self.game.fighters[f.0].x221C_b4 = keep;
        } else {
            self.change_motion_state(gobj, msid, flags, 0.0, 0.0, 0.0, None);
            self.game.fighters[f.0].x221C_b4 = keep;
            self.set_anim_rate(gobj, 0.0);
        }
        self.kb_lw_set_cbs(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// `ftKb_SpecialLw1_Coll`: off a ledge into the aerial change; else the floor's slopes.
    #[doc(alias = "ftKb_SpecialLw1_Coll")]
    pub fn kb_special_lw1_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_lw_off_ledge(gobj, ms::SPECIAL_AIR_LW_START, LAND_FLAGS, true);
        } else {
            self.kb_lw_floor(gobj);
            self.kb_lw_slopes(gobj);
        }
    }

    /// `ftKb_SpecialLw_Coll`: off a ledge into the aerial stone; else the floor's slopes.
    #[doc(alias = "ftKb_SpecialLw_Coll")]
    pub fn kb_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_lw_off_ledge(gobj, ms::SPECIAL_AIR_LW, LAND_HOLD_FLAGS, false);
        } else {
            self.kb_lw_floor(gobj);
            self.kb_lw_slopes(gobj);
        }
    }

    /// The change back's collision, `ftKb_SpecialLwEnd_Coll` and `ftKb_SpecialAirLwEnd_Coll`:
    /// the air's, its floors forgotten, upright.
    #[doc(alias = "ftKb_SpecialLwEnd_Coll")]
    #[doc(alias = "ftKb_SpecialAirLwEnd_Coll")]
    pub fn kb_special_lw_end_coll(&mut self, gobj: GObjId) {
        self.air_coll_land(gobj);
        self.kb_lw_clear_slopes(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// The floor's normal and slope as the last four (`ftKb_SpecialLw_SetStoneVecs`): and on a
    /// slope steep enough, sliding at once at the attributes' speed.
    fn kb_lw_set_stone_vecs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let min = reg(self.kb_attr_f32(f, attr::LW_MIN_SLOPE));
        let max = reg(self.kb_attr_f32(f, attr::LW_SLIDE_MAX));
        let fp = &mut self.game.fighters[f.0];
        let angle = reg(fp.mv_f32(mv::SLOPE));
        let n = mv_vec(fp, mv::NORMAL);
        for i in 0..4 {
            set_mv_vec(fp, mv::NORMALS + 12 * i, n);
            set_mv_vec(fp, mv::AVERAGES + 12 * i, n);
            set_slope(fp, i, angle);
            set_slope(fp, i + 4, angle);
        }
        set_mv_vec(fp, mv::NORMAL, n);
        fp.set_mv_f32(mv::SLOPE, angle);
        let a = abs(reg(fp.mv_f32(mv::SLOPE)));
        if a >= min {
            let nx = reg(fp.mv_f32(mv::NORMAL_X));
            fp.gr_vel = mem(fp::fmuls(nx, max));
        }
    }

    /// Landing as a stone into `msid` with a quake and rumble (`hold` a held animation).
    fn kb_lw_land(&mut self, gobj: GObjId, msid: i32, hold: bool) {
        self.kb_lw_floor(gobj);
        self.kb_lw_slopes(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.land(gobj);
        let fp = &self.game.fighters[f.0];
        let keep = fp.x221C_b4;
        if hold {
            self.change_motion_state(gobj, msid, LAND_FLAGS, 0.0, 0.0, 0.0, None);
            self.game.fighters[f.0].x221C_b4 = keep;
            self.set_anim_rate(gobj, 0.0);
        } else {
            let frame = fp.cur_anim_frame;
            self.change_motion_state(gobj, msid, LAND_FLAGS, frame, 1.0, 0.0, None);
            self.game.fighters[f.0].x221C_b4 = keep;
        }
        self.kb_lw_set_cbs(gobj);
        self.fighter_play_sfx(f, 0x222E7, 0x7F, 0x40);
        let pos = self.game.fighters[f.0].cur_pos;
        camera::request_quake(self, QUAKE_LARGE, Some(pos));
        self.start_rumble(f, 0xE, 0x14);
        self.kb_lw_set_stone_vecs(gobj);
    }

    /// `ftKb_SpecialAirLwStart_Coll`: landing into the grounded change; else its floors
    /// forgotten, upright.
    #[doc(alias = "ftKb_SpecialAirLwStart_Coll")]
    pub fn kb_special_air_lw_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_lw_land(gobj, ms::SPECIAL_LW1, false);
        } else {
            self.kb_lw_clear_slopes(gobj);
            let f = self.game.fighter_id(gobj);
            self.game.part_set_rot_x(f, 0, 0.0);
        }
    }

    /// `ftKb_SpecialAirLw_Coll`: landing into the grounded stone; else its floors forgotten,
    /// upright.
    #[doc(alias = "ftKb_SpecialAirLw_Coll")]
    pub fn kb_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_lw_land(gobj, ms::SPECIAL_LW, true);
        } else {
            self.kb_lw_clear_slopes(gobj);
            let f = self.game.fighter_id(gobj);
            self.game.part_set_rot_x(f, 0, 0.0);
        }
    }

    /// The stone's cleanup, `ftKb_SpecialAirLw_800F5318`: Kirby shown again, no resistance,
    /// upright, his effects gone.
    #[doc(alias = "ftKb_SpecialAirLw_800F5318")]
    pub fn kb_stone_cleanup(&mut self, gobj: GObjId) {
        self.set_item_visible(gobj, true);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b4 = true;
        fp.x2225_b2 = true;
        fp.dmg.x1834 = 0.0;
        fp.x221C_b4 = false;
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        eflib::destroy_all(self, gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_SHOW_STONE => g.kb_lw_show_stone(gobj),
        addr::LW_SCRIPT => g.kb_lw_script(gobj),
        addr::LW_FLOOR => g.kb_lw_floor(gobj),
        addr::LW_CLEAR_SLOPES => g.kb_lw_clear_slopes(gobj),
        addr::LW_SLOPES => g.kb_lw_slopes(gobj),
        addr::LW_SLIDE => g.kb_lw_slide(gobj),
        addr::SPECIAL_LW_ENTER => g.kb_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.kb_special_air_lw_enter(gobj),
        addr::SPECIAL_LW1_ANIM => g.kb_special_lw1_anim(gobj),
        addr::SPECIAL_AIR_LW_START_ANIM => g.kb_special_air_lw_start_anim(gobj),
        addr::SPECIAL_LW_ANIM | addr::SPECIAL_AIR_LW_ANIM => g.kb_special_lw_anim(gobj),
        addr::SPECIAL_LW_END_ANIM | addr::SPECIAL_AIR_LW_END_ANIM => g.kb_special_lw_end_anim(gobj),
        addr::SPECIAL_LW1_IASA
        | addr::SPECIAL_LW_END_IASA
        | addr::SPECIAL_AIR_LW_START_IASA
        | addr::SPECIAL_AIR_LW_END_IASA => g.kb_special_lw_nothing(gobj),
        addr::SPECIAL_LW_IASA => g.kb_special_lw_iasa(gobj),
        addr::SPECIAL_AIR_LW_IASA => g.kb_special_air_lw_iasa(gobj),
        addr::SPECIAL_LW1_PHYS => g.kb_special_lw1_phys(gobj),
        addr::SPECIAL_LW_PHYS => g.kb_special_lw_phys(gobj),
        addr::SPECIAL_LW_END_PHYS
        | addr::SPECIAL_AIR_LW_START_PHYS
        | addr::SPECIAL_AIR_LW_END_PHYS => g.kb_special_lw_anim_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.kb_special_air_lw_phys(gobj),
        addr::SPECIAL_LW1_COLL => g.kb_special_lw1_coll(gobj),
        addr::SPECIAL_LW_COLL => g.kb_special_lw_coll(gobj),
        addr::SPECIAL_LW_END_COLL | addr::SPECIAL_AIR_LW_END_COLL => g.kb_special_lw_end_coll(gobj),
        addr::SPECIAL_AIR_LW_START_COLL => g.kb_special_air_lw_start_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.kb_special_air_lw_coll(gobj),
        addr::STONE_CLEANUP => g.kb_stone_cleanup(gobj),
        _ => return false,
    }
    true
}
