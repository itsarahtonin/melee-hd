//! Dolphin Slash (`ftmarsspecialhi.c`): the rising slash, angled by the stick early on and
//! turned by it once, then the helpless fall.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::coll::PLATFORM_FILTER;
use super::{GA_AIR, addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// `abs` as the original's macro spells it.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

impl Running<'_> {
    /// `ftMs_SpecialHi_Enter`.
    #[doc(alias = "ftMs_SpecialHi_Enter")]
    pub fn mars_special_hi_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftMs_SpecialAirHi_Enter`: no rise kept, the horizontal speed scaled.
    #[doc(alias = "ftMs_SpecialAirHi_Enter")]
    pub fn mars_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x3c = reg(self.mars_attr_f32(f, attr::X3C));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), x3c));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The slash's end, `ftMs_SpecialHi_Anim` and `ftMs_SpecialAirHi_Anim`: the helpless
    /// fall.
    #[doc(alias = "ftMs_SpecialHi_Anim")]
    #[doc(alias = "ftMs_SpecialAirHi_Anim")]
    pub fn mars_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let drift = self.mars_attr_f32(f, attr::X28);
            let lag = self.mars_attr_f32(f, attr::X2C);
            self.enter_fall_special(gobj, 0, 1, 0, drift, lag, 0.0, true);
        }
    }

    /// The slash's input, `ftMs_SpecialHi_IASA` and `ftMs_SpecialAirHi_IASA`: until the
    /// script says (`cmd_vars[0]`), the stick past `x34` tilts the slash up to `x38` degrees,
    /// keeping the widest tilt; where the script allows (`ftCheckThrowB3`), the stick past
    /// `x30` turns Marth.
    #[doc(alias = "ftMs_SpecialHi_IASA")]
    #[doc(alias = "ftMs_SpecialAirHi_IASA")]
    pub fn mars_special_hi_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x30 = reg(self.mars_attr_f32(f, attr::X30));
        let x34 = reg(self.mars_attr_f32(f, attr::X34));
        let x38 = reg(self.mars_attr_f32(f, attr::X38));
        let fp = &mut self.game.fighters[f.0];
        let x = reg(fp.input.lstick[0].x);
        let abs_x = abs(x);
        if fp.cmd_vars[0] == 0 && abs_x > x34 {
            let t = fp::frsp(fp::fmul(
                x38,
                fp::fdiv(fp::fsubs(abs_x, x34), fp::fsub(1.0, x34)),
            ));
            let deg = fp::fdivs(reg(std::f32::consts::PI), 180.0);
            let t = if x > 0.0 {
                fp::fneg(fp::fmuls(deg, t))
            } else {
                fp::fmuls(deg, t)
            };
            if abs(t) > abs(reg(fp.lstick_angle)) {
                fp.lstick_angle = mem(t);
            }
        }
        if self.take_throw_flag3(gobj) {
            let x = reg(self.game.fighters[f.0].input.lstick[0].x);
            if abs(x) > x30 {
                self.face_stick(gobj);
                let facing = reg(self.game.fighters[f.0].facing_dir);
                let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, facing)));
                self.game.part_set_rot_y(f, 0, rot);
            }
        }
    }

    /// The slash's flight once the script has it rise (`cmd_vars[2]` clear): the
    /// animation's motion turned by the tilt, facing the way it moves, until the animation
    /// starts down.
    fn mars_special_hi_rise(&mut self, gobj: GObjId, scale: Option<f64>) {
        self.air_anim_motion_angled(gobj);
        let fp = self.game.fighter_mut(gobj);
        if let Some(s) = scale {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), s));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), s));
            fp.self_vel.z = mem(fp::fmuls(reg(fp.self_vel.z), s));
        }
        let num: i32 = if fp.self_vel.x < 0.0 { -1 } else { 1 };
        if reg(fp.facing_dir) != fp::frsp(f64::from(num)) {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), fp::fneg(1.0)));
        }
        if fp.x6A4_transNOffset.y < 0.0 {
            fp.cmd_vars[2] = 1;
        }
    }

    /// The slash's descent: its own gravity, and drift scaled by `x28`.
    fn mars_special_hi_descent(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.mars_attr_f32(f, attr::X44)),
            reg(self.mars_attr_f32(f, attr::X48)),
        );
        let x28 = reg(self.mars_attr_f32(f, attr::X28));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        let accel = fp::fmuls(reg(fp.co_attrs.air_drift_stick_mul), x28);
        let target = fp::fmuls(reg(fp.co_attrs.air_drift_max), x28);
        Self::drift_simple(fp, 0.0, accel, target);
    }

    /// `ftMs_SpecialHi_Phys`: on the ground, the animation's motion or friction.
    #[doc(alias = "ftMs_SpecialHi_Phys")]
    pub fn mars_special_hi_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.cmd_vars[2] == 0 {
                self.mars_special_hi_rise(gobj, None);
            } else {
                self.mars_special_hi_descent(gobj);
            }
        } else {
            self.ground_friction_or_anim(gobj);
        }
    }

    /// `ftMs_SpecialAirHi_Phys`: before the script starts the rise (`cmd_vars[0]`),
    /// ordinary gravity and air friction; the rise scaled by `x40`.
    #[doc(alias = "ftMs_SpecialAirHi_Phys")]
    pub fn mars_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            if fp.cmd_vars[2] == 0 {
                let x40 = reg(self.mars_attr_f32(f, attr::X40));
                self.mars_special_hi_rise(gobj, Some(x40));
            } else {
                self.mars_special_hi_descent(gobj);
            }
        } else {
            let fp = &mut self.game.fighters[f.0];
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
            self.self_accel_deaccel_quick_air(gobj);
        }
    }

    /// Landing from the slash, `ftMs_SpecialHi_80138884`.
    #[doc(alias = "ftMs_SpecialHi_80138884")]
    pub fn mars_special_hi_land(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.mars_attr_f32(f, attr::X2C);
        self.enter_landing_fall_special(gobj, 0, lag);
    }

    /// The slash's collision, `ftMs_SpecialHi_Coll` and `ftMs_SpecialAirHi_Coll`: no
    /// landing while rising or before the script allows it, and not on the first frame
    /// falling either.
    #[doc(alias = "ftMs_SpecialHi_Coll")]
    #[doc(alias = "ftMs_SpecialAirHi_Coll")]
    pub fn mars_special_hi_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air != GA_AIR {
            self.ground_coll(gobj);
            return;
        }
        if fp.cmd_vars[0] == 0 || fp.self_vel.y >= 0.0 {
            self.air_coll_stay(gobj);
        } else if fp.cmd_vars[1] == 0 {
            fp.cmd_vars[1] = 1;
            self.air_coll_stay(gobj);
        } else {
            self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |g, gobj| {
                g.mars_special_hi_land(gobj)
            });
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.mars_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.mars_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM | addr::SPECIAL_AIR_HI_ANIM => g.mars_special_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => g.mars_special_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS => g.mars_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.mars_special_air_hi_phys(gobj),
        addr::HI_80138884 => g.mars_special_hi_land(gobj),
        addr::SPECIAL_HI_COLL | addr::SPECIAL_AIR_HI_COLL => g.mars_special_hi_coll(gobj),
        _ => return false,
    }
    true
}
