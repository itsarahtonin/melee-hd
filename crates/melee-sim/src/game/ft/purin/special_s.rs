//! Pound (`ftpurinspecials.c`): a punch, in the air a lunge whose angle the stick tilts, which
//! counts her lunges (`u.x222C`).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::{addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `ftPr_MF_SpecialS_Coll`: the flags the switches between the ground and air states keep.
const KEEP: u32 = 0x0C4C_508A;
/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

/// `stickGetDir`: the stick's distance from `x2`.
fn stick_dir(x1: f64, x2: f64) -> f64 {
    if x1 < x2 { fp::fneg(x1) } else { x1 }
}

impl Running<'_> {
    /// Pound's entry from the ground or the air: the state, and the script's variables cleared.
    fn purin_pound_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
    }

    /// `ftPr_SpecialS_Enter`.
    // Ports `ftPr_SpecialS_Enter`; aliased once a check reaches it.
    pub fn purin_special_s_enter(&mut self, gobj: GObjId) {
        self.purin_pound_start(gobj, ms::SPECIAL_S);
    }

    /// `ftPr_SpecialAirS_Enter`.
    #[doc(alias = "ftPr_SpecialAirS_Enter")]
    pub fn purin_special_air_s_enter(&mut self, gobj: GObjId) {
        self.purin_pound_start(gobj, ms::SPECIAL_AIR_S);
    }

    /// `ftPr_SpecialS_Anim`.
    // Ports `ftPr_SpecialS_Anim`; aliased once a check reaches it.
    pub fn purin_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPr_SpecialAirS_Anim`.
    #[doc(alias = "ftPr_SpecialAirS_Anim")]
    pub fn purin_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The lunge's angle from the stick's height (`calcAngleRadians`): the stick past the dead
    /// zone, up to the attributes' top, scaled to the angle's range, in radians.
    fn purin_pound_angle(&self, gobj: GObjId, stick_y: f64) -> f64 {
        let f = self.game.fighter_id(gobj);
        let a = |off| reg(self.purin_attr_f32(f, off));
        let (lo, hi, range) = (a(attr::S_STICK_MIN), a(attr::S_STICK_MAX), a(attr::S_ANGLE));
        let mut y = stick_dir(stick_y, fp::frsp(0.0));
        if y > hi {
            y = hi;
        }
        y = fp::fsubs(y, lo);
        if y < 0.0 {
            y = 0.0;
        }
        if stick_y < 0.0 {
            y = fp::fneg(y);
        }
        fp::fmuls(
            DEG_TO_RAD,
            fp::fdivs(fp::fmuls(y, range), fp::fsubs(hi, lo)),
        )
    }

    /// `ftPr_SpecialAirS_Phys`: once the script says (`cmd_vars[0]`), a lunge at the stick's
    /// angle, counted; then as the script's second variable says, falling, slowing, or drifting.
    #[doc(alias = "ftPr_SpecialAirS_Phys")]
    pub fn purin_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let speed = reg(self.purin_attr_f32(f, attr::S_SPEED));
        let decel = reg(self.purin_attr_f32(f, attr::S_DECEL));
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[0] = 0;
            let n = fp.u_i32(u::POUNDS);
            fp.set_u_i32(u::POUNDS, n.wrapping_add(1));
            let stick_y = reg(fp.input.lstick[0].y);
            let angle = mem(self.purin_pound_angle(gobj, stick_y));
            let s = reg(math::sin(angle));
            let c = reg(math::cos(angle));
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.y = mem(fp::fmuls(speed, s));
            fp.self_vel.x = mem(fp::fmuls(speed, fp::fmuls(reg(fp.facing_dir), c)));
        }
        match self.game.fighters[f.0].cmd_vars[1] {
            0 => self.air_physics_no_drift(gobj),
            1 => {
                let fp = &mut self.game.fighters[f.0];
                fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), decel));
                fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), decel));
            }
            2 => self.air_physics(gobj),
            _ => {}
        }
    }

    /// `ftPr_SpecialS_Coll`.
    // Ports `ftPr_SpecialS_Coll`; aliased once a check reaches it.
    pub fn purin_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.purin_special_s_to_air(gobj);
        }
    }

    /// `ftPr_SpecialAirS_Coll`.
    #[doc(alias = "ftPr_SpecialAirS_Coll")]
    pub fn purin_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.purin_special_air_s_to_ground(gobj);
        }
    }

    /// Off the ground mid-punch, `ftPr_SpecialS_8013D590`.
    // Ports `ftPr_SpecialS_8013D590`; aliased once a check reaches it.
    pub fn purin_special_s_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S, KEEP);
    }

    /// Landing mid-lunge, `ftPr_SpecialS_8013D5F0`: the drift kept within the air's.
    // Ports `ftPr_SpecialS_8013D5F0`; aliased once a check reaches it.
    pub fn purin_special_air_s_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S, KEEP);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ENTER => g.purin_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.purin_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.purin_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.purin_special_air_s_anim(gobj),
        addr::SPECIAL_S_IASA | addr::SPECIAL_AIR_S_IASA => {}
        addr::SPECIAL_S_PHYS => g.ground_friction_or_anim(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.purin_special_air_s_phys(gobj),
        addr::SPECIAL_S_COLL => g.purin_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.purin_special_air_s_coll(gobj),
        addr::SPECIAL_S_8013D590 => g.purin_special_s_to_air(gobj),
        addr::SPECIAL_S_8013D5F0 => g.purin_special_air_s_to_ground(gobj),
        _ => return false,
    }
    true
}
