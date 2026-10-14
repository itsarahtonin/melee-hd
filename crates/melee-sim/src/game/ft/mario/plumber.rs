//! The moves Mario, Dr. Mario and Luigi build the same way, each from its own copy of the
//! code (`ftmariospecialhi.c` and `ftmariospeciallw.c`, `ftluigispecialhi.c` and
//! `ftluigispeciallw.c`): the Super Jump Punch and the Mario Tornado / Luigi Cyclone. Their
//! attributes share a layout at different places in each character's attribute block; a
//! [`SuperJump`] or [`Tornado`] says where, and which states, effect and callbacks are the
//! character's.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::super::coll::{Cb, PLATFORM_FILTER};
use super::super::common::{GA_AIR, GA_GROUND};
use crate::fpu::{mem, reg};
use crate::game::handles::FighterId;
use crate::game::structs::{Vec2, ftCollisionBox};
use crate::hsd::gobj::GObjId;

/// A character's Super Jump Punch: where its attributes start (`freefall_mobility`, then
/// `landing_lag`, `reverse_stick_range`, `momentum_stick_range`, `angle_diff`, `vel_x`, `grav`,
/// `vel_mul`), its two states, and its landing callback (`*_SpecialHi_CheckLanding`).
pub(crate) struct SuperJump {
    pub base: usize,
    pub ms_hi: i32,
    pub ms_air_hi: i32,
    pub landing: Cb,
}

const HI_FREEFALL_MOBILITY: usize = 0x0;
const HI_LANDING_LAG: usize = 0x4;
const HI_REVERSE_STICK_RANGE: usize = 0x8;
const HI_MOMENTUM_STICK_RANGE: usize = 0xC;
const HI_ANGLE_DIFF: usize = 0x10;
const HI_VEL_X: usize = 0x14;
const HI_GRAV: usize = 0x18;
const HI_VEL_MUL: usize = 0x1C;

/// A character's Tornado or Cyclone: where its attributes start (`vel_y`, then `momentum_x`,
/// `air_momentum_x`, `momentum_x_mul`, `air_momentum_x_mul`, `friction_end`, `unk0`,
/// `tap_y_vel_max`, `tap_grav`, `landing_lag`), its two states, the `u` word that says it has
/// risen this time airborne, its effect, and its damage callback (the model upright again).
pub(crate) struct Tornado {
    pub base: usize,
    pub ms_lw: i32,
    pub ms_air_lw: i32,
    pub charged: usize,
    pub effect: i32,
    pub on_damage: u32,
}

const LW_VEL_Y: usize = 0x0;
const LW_MOMENTUM_X: usize = 0x4;
const LW_AIR_MOMENTUM_X: usize = 0x8;
const LW_MOMENTUM_X_MUL: usize = 0xC;
const LW_AIR_MOMENTUM_X_MUL: usize = 0x10;
const LW_FRICTION_END: usize = 0x14;
const LW_UNK0: usize = 0x18;
const LW_TAP_Y_VEL_MAX: usize = 0x1C;
const LW_TAP_GRAV: usize = 0x20;
const LW_LANDING_LAG: usize = 0x24;

/// The spin's state variables (`ftMarioSpecialLw`, `ftLuigiSpecialLw`): the speed it keeps,
/// a word set from `unk0` and whether it's on a floor.
const MV_GROUND_VEL_X: usize = 0x0;
const MV_UNK: usize = 0x4;
const MV_IS_UNK_COLL: usize = 0xC;

/// The flags the spin keeps between the ground and the air.
const LW_FLAGS: u32 = 0x0C4C_508A;

/// B pressed.
const PAD_B: u32 = 1 << 9;

/// The spin's ECB, Mario's (`coll_box`) and Luigi's (`ftLg_SpecialLw_CollisionBox`) alike.
fn spin_box() -> ftCollisionBox {
    ftCollisionBox {
        top: 12.0,
        bottom: 0.0,
        left: Vec2 { x: -6.0, y: 6.0 },
        right: Vec2 { x: 6.0, y: 6.0 },
    }
}

impl Running<'_> {
    /// A word of the fighter's character attributes, `off` bytes in.
    pub(crate) fn char_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    pub(crate) fn char_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.char_attr_word(f, off))
    }

    /// `*_SpecialHi_Enter`.
    pub(crate) fn plumber_hi_enter(&mut self, gobj: GObjId, p: &SuperJump) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, p.ms_hi, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `*_SpecialAirHi_Enter`: no vertical speed, the horizontal cut by `vel_x`.
    pub(crate) fn plumber_air_hi_enter(&mut self, gobj: GObjId, p: &SuperJump) {
        let f = self.game.fighter_id(gobj);
        let vel_x = reg(self.char_attr_f32(f, p.base + HI_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vel_x));
        self.change_motion_state(gobj, p.ms_air_hi, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `*_SpecialHi_Anim`: falling helpless with `freefall_mobility` and `landing_lag` when
    /// the punch ends.
    pub(crate) fn plumber_hi_anim(&mut self, gobj: GObjId, p: &SuperJump) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let mobility = self.char_attr_f32(f, p.base + HI_FREEFALL_MOBILITY);
            let lag = self.char_attr_f32(f, p.base + HI_LANDING_LAG);
            self.enter_fall_special(gobj, 0, 1, 0, mobility, lag, 0.0, true);
        }
    }

    /// `*_SpecialHi_IASA`: until the script says (`cmd_vars[0]`), a stick past
    /// `momentum_stick_range` tilts the rise up to `angle_diff` degrees (kept if steeper than
    /// the tilt so far); when the script raises its flag, a stick past `reverse_stick_range`
    /// turns the fighter.
    pub(crate) fn plumber_hi_iasa(&mut self, gobj: GObjId, p: &SuperJump) {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.char_attr_f32(f, p.base + HI_MOMENTUM_STICK_RANGE));
        let diff = reg(self.char_attr_f32(f, p.base + HI_ANGLE_DIFF));
        let reverse = reg(self.char_attr_f32(f, p.base + HI_REVERSE_STICK_RANGE));
        let fp = &mut self.game.fighters[f.0];
        let stick_x = reg(fp.input.lstick[0].x);
        let stick = if stick_x < 0.0 {
            fp::fneg(stick_x)
        } else {
            stick_x
        };
        if fp.cmd_vars[0] == 0 && stick > range {
            let t = fp::frsp(fp::fmul(
                diff,
                fp::fdiv(fp::fsubs(stick, range), fp::fsub(1.0, range)),
            ));
            let rad = if stick_x > 0.0 {
                fp::fneg(fp::fmuls(0.017_453_292_384_743_69, t))
            } else {
                fp::fmuls(0.017_453_292_384_743_69, t)
            };
            let mag = if rad < 0.0 { fp::fneg(rad) } else { rad };
            let cur = reg(fp.lstick_angle);
            let cur = if cur < 0.0 { fp::fneg(cur) } else { cur };
            if mag > cur {
                fp.lstick_angle = mem(rad);
            }
        }
        if fp.throw_flag(3) {
            fp.set_throw_flag(3, false);
            let x = reg(fp.input.lstick[0].x);
            let x = if x < 0.0 { fp::fneg(x) } else { x };
            if x > reverse {
                self.face_stick(gobj);
                let facing = reg(self.game.fighters[f.0].facing_dir);
                let rot = mem(fp::frsp(fp::fmul(
                    fp::fdiv(std::f64::consts::PI, 2.0),
                    facing,
                )));
                self.game.part_set_rot_y(f, 0, rot);
            }
        }
    }

    /// `*_SpecialHi_Phys`: in the air the animation's motion turned by the stick's angle, on
    /// the ground friction or the animation's motion.
    pub(crate) fn plumber_hi_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.air_anim_motion_angled(gobj);
        } else {
            self.ground_friction_or_anim(gobj);
        }
    }

    /// `*_SpecialAirHi_Phys`: once the script says, the angled motion slowed by `vel_mul`;
    /// until then falling at `grav` with quick air friction.
    pub(crate) fn plumber_air_hi_phys(&mut self, gobj: GObjId, p: &SuperJump) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.air_anim_motion_angled(gobj);
            let mul = reg(self.char_attr_f32(f, p.base + HI_VEL_MUL));
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), mul));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), mul));
            fp.self_vel.z = mem(fp::fmuls(reg(fp.self_vel.z), mul));
            return;
        }
        let g = reg(self.char_attr_f32(f, p.base + HI_GRAV));
        let fp = &mut self.game.fighters[f.0];
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        self.self_accel_deaccel_quick_air(gobj);
    }

    /// `*_SpecialHi_CheckLanding`: landing helpless over `landing_lag`.
    pub(crate) fn plumber_hi_check_landing(&mut self, gobj: GObjId, p: &SuperJump) {
        let f = self.game.fighter_id(gobj);
        let lag = self.char_attr_f32(f, p.base + HI_LANDING_LAG);
        self.enter_landing_fall_special(gobj, 0, lag);
    }

    /// `*_SpecialHi_Coll`: in the air, staying airborne while rising or before the script
    /// says, then landing helpless through platforms; on the ground, the ground's.
    pub(crate) fn plumber_hi_coll(&mut self, gobj: GObjId, p: &SuperJump) {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.cmd_vars[0] == 0 || reg(fp.self_vel.y) >= 0.0 {
                self.air_coll_stay(gobj);
            } else {
                self.air_coll_filtered_full(gobj, PLATFORM_FILTER, p.landing);
            }
        } else {
            self.ground_coll(gobj);
        }
    }

    /// The model upright again (Mario's `updateRot`, `ftLg_SpecialLw_UpdateRot`).
    pub(crate) fn plumber_lw_update_rot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// `*_SpecialLw_Enter` and `*_SpecialAirLw_Enter`: the spin starts in its aerial state,
    /// lifted by `vel_y` less `tap_y_vel_max` (in the air, without the cut once it has risen
    /// this time airborne), its drift capped, its effect on the model.
    pub(crate) fn plumber_lw_enter(&mut self, gobj: GObjId, p: &Tornado, grounded: bool) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[2] = 0;
        self.change_motion_state(gobj, p.ms_air_lw, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let vel_y = reg(self.char_attr_f32(f, p.base + LW_VEL_Y));
        let max = self.char_attr_f32(f, p.base + LW_TAP_Y_VEL_MAX);
        let air_x = reg(self.char_attr_f32(f, p.base + LW_AIR_MOMENTUM_X));
        let unk = self.char_attr_word(f, p.base + LW_UNK0) as i32;
        let fp = &mut self.game.fighters[f.0];
        let cut = if grounded || fp.u_i32(p.charged) == 0 {
            reg(max)
        } else {
            0.0
        };
        fp.self_vel.y = mem(fp::fsubs(vel_y, cut));
        Self::clamp_self_vel_x(fp, air_x);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.set_mv_f32(MV_GROUND_VEL_X, 0.0);
        fp.set_mv_i32(MV_UNK, unk.wrapping_add(1));
        fp.set_mv_i32(MV_IS_UNK_COLL, 0);
        self.set_damage_cb(gobj, Some(p.on_damage));
        let root = Some(self.root_jobj(gobj));
        spawn(self, p.effect, Some(gobj), &[Arg::JObj(root)]);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `*_SpecialLw_Anim`: waiting when the spin ends.
    pub(crate) fn plumber_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.set_damage_cb(gobj, None);
            self.enter_wait(gobj);
        }
    }

    /// `*_SpecialAirLw_Anim`: the rise is spent when the script says; when the spin ends, a
    /// fall, or a helpless one with `landing_lag`.
    pub(crate) fn plumber_air_lw_anim(&mut self, gobj: GObjId, p: &Tornado) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[1] != 0 {
            fp.cmd_vars[1] = 0;
            fp.set_u_i32(p.charged, 1);
        }
        if !self.anim_frames_remaining(gobj) {
            self.set_damage_cb(gobj, None);
            let lag = self.char_attr_word(f, p.base + LW_LANDING_LAG) as i32;
            if f64::from(lag) == 0.0 {
                self.enter_fall(gobj);
                return;
            }
            let lag = mem(fp::frsp(f64::from(lag)));
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
        }
    }

    /// The spin's top speed: `base`, plus the speed it keeps (shrinking by `friction_end` each
    /// frame) once the script says, never below 0.
    fn plumber_lw_top_speed(&mut self, gobj: GObjId, p: &Tornado, base: f64) -> f64 {
        let f = self.game.fighter_id(gobj);
        let end = reg(self.char_attr_f32(f, p.base + LW_FRICTION_END));
        let fp = &mut self.game.fighters[f.0];
        let mut top = base;
        if fp.cmd_vars[0] != 0 {
            let v = fp::fsubs(reg(fp.mv_f32(MV_GROUND_VEL_X)), end);
            fp.set_mv_f32(MV_GROUND_VEL_X, v);
            top = fp::fadds(top, reg(fp.mv_f32(MV_GROUND_VEL_X)));
            if top < 0.0 {
                top = 0.0;
            }
        }
        top
    }

    /// `*_SpecialLw_Phys`: steered along the ground by the stick up to the top speed; B tapped
    /// while the script allows (`cmd_vars[2]`) lifts it into the air.
    pub(crate) fn plumber_lw_phys(&mut self, gobj: GObjId, p: &Tornado) {
        let f = self.game.fighter_id(gobj);
        let base = reg(self.char_attr_f32(f, p.base + LW_MOMENTUM_X));
        let top = self.plumber_lw_top_speed(gobj, p, base);
        let mul = reg(self.char_attr_f32(f, p.base + LW_MOMENTUM_X_MUL));
        Self::ground_accel_to_stick(&mut self.game.fighters[f.0], 0.0, mul, top);
        self.self_movement_from_ground(gobj);
        let tap = reg(self.char_attr_f32(f, p.base + LW_TAP_Y_VEL_MAX));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[2] != 0 && fp.input.pressed_buttons & PAD_B != 0 {
            fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), tap));
            self.plumber_lw_ground_to_air(gobj, p);
        }
    }

    /// `*_SpecialAirLw_Phys`: B tapped while the script allows lifts it, once per time
    /// airborne; the fall; steered in the air by the stick up to the top speed.
    pub(crate) fn plumber_air_lw_phys(&mut self, gobj: GObjId, p: &Tornado) {
        let f = self.game.fighter_id(gobj);
        let tap = reg(self.char_attr_f32(f, p.base + LW_TAP_Y_VEL_MAX));
        let g = reg(self.char_attr_f32(f, p.base + LW_TAP_GRAV));
        let base = reg(self.char_attr_f32(f, p.base + LW_AIR_MOMENTUM_X));
        let mul = reg(self.char_attr_f32(f, p.base + LW_AIR_MOMENTUM_X_MUL));
        let fp = &mut self.game.fighters[f.0];
        if fp.u_i32(p.charged) == 0 && fp.cmd_vars[2] != 0 && fp.input.pressed_buttons & PAD_B != 0
        {
            Self::ascend(fp, tap, g);
        }
        Self::fall_basic(fp);
        let top = self.plumber_lw_top_speed(gobj, p, base);
        Self::drift_simple_no_friction(&mut self.game.fighters[f.0], 0.0, mul, top);
    }

    /// Airborne in the aerial spin, its fall and drift capped.
    fn plumber_lw_ground_to_air(&mut self, gobj: GObjId, p: &Tornado) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[2] = 0;
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, p.ms_air_lw, LW_FLAGS, frame, 1.0, 0.0, None);
        let g = reg(self.char_attr_f32(f, p.base + LW_TAP_GRAV));
        let air_x = reg(self.char_attr_f32(f, p.base + LW_AIR_MOMENTUM_X));
        let fp = &mut self.game.fighters[f.0];
        Self::clamp_fall_speed(fp, g);
        Self::clamp_self_vel_x(fp, air_x);
        self.set_effect_hitlag_cbs(gobj);
    }

    /// Landed: the grounded spin, the rise ready again.
    fn plumber_air_lw_air_to_ground(&mut self, gobj: GObjId, p: &Tornado) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[2] = 0;
        self.land(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.set_u_i32(p.charged, 0);
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, p.ms_lw, LW_FLAGS, frame, 1.0, 0.0, None);
        let x = reg(self.char_attr_f32(f, p.base + LW_MOMENTUM_X));
        Self::clamp_ground_vel(&mut self.game.fighters[f.0], x);
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The spin's tilt: with the floor's slope while the script says (`cmd_vars[3]`) and the
    /// spin is on a floor, else upright.
    fn plumber_lw_tilt(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[3] != 0 && fp.mv_i32(MV_IS_UNK_COLL) != 0 {
            let n = fp.coll_data.floor.normal;
            let a = crate::math::atan2(n.x, n.y);
            let facing = reg(fp.facing_dir);
            self.game
                .part_set_rot_x(f, 0, mem(fp::fmuls(facing, reg(a))));
            return;
        }
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// `*_SpecialLw_Coll`: with the spin's ECB, on the ground (or the airborne pass when it
    /// isn't), off a ledge into the air.
    pub(crate) fn plumber_lw_coll(&mut self, gobj: GObjId, p: &Tornado) {
        let b = spin_box();
        let on = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll_box(gobj, &b)
        } else {
            self.air_coll_box(gobj, &b)
        };
        if !on {
            self.plumber_lw_ground_to_air(gobj, p);
        }
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(MV_IS_UNK_COLL, i32::from(on));
        self.plumber_lw_tilt(gobj);
    }

    /// `*_SpecialAirLw_Coll`: landing, the grounded spin.
    pub(crate) fn plumber_air_lw_coll(&mut self, gobj: GObjId, p: &Tornado) {
        let b = spin_box();
        let landed = self.air_coll_box(gobj, &b);
        if landed {
            self.plumber_air_lw_air_to_ground(gobj, p);
        }
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(MV_IS_UNK_COLL, i32::from(landed));
        self.plumber_lw_tilt(gobj);
    }
}
