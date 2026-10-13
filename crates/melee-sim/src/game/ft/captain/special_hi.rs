//! Falcon Dive and Dark Dive (`ftcaptainspecialhi.c`): the rise, which grabs a fighter it
//! catches, the catch, and the throw that launches the caught fighter.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::ppc::cvt_fp2unsigned;

/// `specialhi.x2_b0` and `x2_b1` in the flags byte.
const HI_B0: u8 = 0x80;
const HI_B1: u8 = 0x40;

impl Running<'_> {
    fn ca_hi_flag(&self, gobj: GObjId, bit: u8) -> bool {
        self.game.fighter(gobj).mv[mv::HI_FLAGS..mv::HI_FLAGS + 1][0] & bit != 0
    }

    fn ca_set_hi_flag(&mut self, gobj: GObjId, bit: u8, on: bool) {
        let fp = self.game.fighter_mut(gobj);
        let b = fp.mv[mv::HI_FLAGS..mv::HI_FLAGS + 1][0];
        fp.mv[mv::HI_FLAGS..mv::HI_FLAGS + 1][0] = if on { b | bit } else { b & !bit };
    }

    /// The rise's variables as its state starts (`ftCa_SpecialLw_800E49FC`, the state's
    /// `x21EC`): the jumps used, `specialhi_air_var` and `specialhi_unk2` noted, no carried
    /// speed and its flags clear.
    #[doc(alias = "ftCa_SpecialLw_800E49FC")]
    pub fn ca_special_hi_on_change(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let air_var = self.ca_attr_i32(f, attr::SPECIALHI_AIR_VAR);
        let unk2 = reg(self.ca_attr_f32(f, attr::SPECIALHI_UNK2));
        let fp = &mut self.game.fighters[f.0];
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.mv[mv::HI_X0..mv::HI_X0 + 2].copy_from_slice(&(air_var as u16).to_be_bytes());
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = cvt_fp2unsigned(unk2);
        fp.set_mv_f32(mv::HI_VEL_X, 0.0);
        fp.set_mv_f32(mv::HI_VEL_Y, 0.0);
        self.ca_set_hi_flag(gobj, HI_B0, false);
        self.ca_set_hi_flag(gobj, HI_B1, false);
    }

    /// The rise's start: its state, and a grab (`ftCommon_8007E2D0`) whose catch takes the
    /// fighter it catches (`ftCa_SpecialLw_800E5128`, the victim's `ftCo_8009CA0C`).
    fn ca_special_hi_start(&mut self, gobj: GObjId, msid: i32) {
        self.game.fighter_mut(gobj).x21EC = Some(CodeRef(addr::HI_ON_CHANGE));
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 2;
        fp.grab_cb = Some(CodeRef(addr::HI_ON_GRAB));
        fp.grabbed_cb = Some(CodeRef(addr::CAPTURE_CAPTAIN));
        fp.x2194 = None;
        self.anim_update(gobj);
    }

    /// `ftCa_SpecialHi_Enter`.
    #[doc(alias = "ftCa_SpecialHi_Enter")]
    pub fn ca_special_hi_enter(&mut self, gobj: GObjId) {
        self.ca_special_hi_start(gobj, ms::SPECIAL_HI);
    }

    /// `ftCa_SpecialAirHi_Enter`.
    #[doc(alias = "ftCa_SpecialAirHi_Enter")]
    pub fn ca_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.ca_special_hi_start(gobj, ms::SPECIAL_AIR_HI);
    }

    /// `ftCa_SpecialHi_Anim` and `ftCa_SpecialAirHi_Anim`: the helpless fall when the rise
    /// ends.
    #[doc(alias = "ftCa_SpecialHi_Anim")]
    #[doc(alias = "ftCa_SpecialAirHi_Anim")]
    pub fn ca_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let drift = self.ca_attr_f32(f, attr::SPECIALHI_FREEFALL_AIR_SPD_MUL);
            let lag = self.ca_attr_f32(f, attr::SPECIALHI_LANDING_LAG);
            self.enter_fall_special(gobj, 1, 1, 0, drift, lag, 0.0, true);
        }
    }

    /// `ftCa_SpecialHi_IASA` and `ftCa_SpecialAirHi_IASA`: when the script says
    /// (`cmd_vars[0]`), the rise can catch the ledge (`x2_b1`), and the stick past
    /// `specialhi_input_var` turns it.
    #[doc(alias = "ftCa_SpecialHi_IASA")]
    #[doc(alias = "ftCa_SpecialAirHi_IASA")]
    pub fn ca_special_hi_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.ca_set_hi_flag(gobj, HI_B1, true);
        let f = self.game.fighter_id(gobj);
        let input = reg(self.ca_attr_f32(f, attr::SPECIALHI_INPUT_VAR));
        let x = reg(self.game.fighters[f.0].input.lstick[0].x);
        let x = if x < 0.0 { fp::fneg(x) } else { x };
        if x > input {
            self.face_stick(gobj);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, facing)));
            self.game.part_set_rot_y(f, 0, rot);
        }
    }

    /// The rise's motion (`ftCa_SpecialHi_Phys`'s body): the speed it carries (`mv` `vel`)
    /// drifts with the stick, slowed past `specialhi_horz_vel` of the most drift, and adds to
    /// the animation's motion.
    fn ca_special_hi_motion(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let horz = reg(self.ca_attr_f32(f, attr::SPECIALHI_HORZ_VEL));
        let friction = reg(self.ca_attr_f32(f, attr::SPECIALHI_AIR_FRICTION_MUL));
        let threshold = reg(self.common().f32(0x258));
        let oob = reg(self.common().f32(0x1FC));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = fp.mv_f32(mv::HI_VEL_X);
        fp.self_vel.y = fp.mv_f32(mv::HI_VEL_Y);
        fp.self_vel.z = 0.0;
        let max = reg(fp.co_attrs.air_drift_max);
        if !Self::deaccel_quick(fp, oob, fp::fmuls(horz, max)) {
            let stick_mul = reg(fp.co_attrs.air_drift_stick_mul);
            Self::drift_simple_no_friction(
                fp,
                threshold,
                fp::fmuls(stick_mul, friction),
                fp::fmuls(max, horz),
            );
        }
        let vx = fp::fadds(reg(fp.x74_self_accel.x), reg(fp.self_vel.x));
        fp.set_mv_f32(mv::HI_VEL_X, vx);
        let vy = fp::fadds(reg(fp.x74_self_accel.y), reg(fp.self_vel.y));
        fp.set_mv_f32(mv::HI_VEL_Y, vy);
        self.air_anim_motion(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
        fp.self_vel.x = mem(fp::fadds(reg(fp.self_vel.x), reg(fp.mv_f32(mv::HI_VEL_X))));
        fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), reg(fp.mv_f32(mv::HI_VEL_Y))));
    }

    /// `ftCa_SpecialHi_Phys` and `ftCa_SpecialAirHi_Phys`.
    #[doc(alias = "ftCa_SpecialHi_Phys")]
    #[doc(alias = "ftCa_SpecialAirHi_Phys")]
    pub fn ca_special_hi_phys(&mut self, gobj: GObjId) {
        self.ca_special_hi_motion(gobj);
    }

    /// The rise in the air (`doAirColl`): landing, helpless once it could catch the ledge
    /// (`x2_b1`), else staying in the air; or catching the ledge once it can.
    fn ca_special_hi_air_coll(&mut self, gobj: GObjId) {
        let can_catch = self.ca_hi_flag(gobj, HI_B1);
        if self.check_ground_and_ledge(gobj, 0) {
            if can_catch {
                let f = self.game.fighter_id(gobj);
                let lag = self.ca_attr_f32(f, attr::SPECIALHI_LANDING_LAG);
                self.enter_landing_fall_special(gobj, 0, lag);
            } else {
                self.air_coll_stay(gobj);
            }
        } else if can_catch && self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
        }
    }

    /// `ftCa_SpecialHi_Coll` and `ftCa_SpecialAirHi_Coll`: in the air, [`doAirColl`]; on the
    /// ground, airborne once off it.
    #[doc(alias = "ftCa_SpecialHi_Coll")]
    #[doc(alias = "ftCa_SpecialAirHi_Coll")]
    pub fn ca_special_hi_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == super::super::common::GA_AIR {
            self.ca_special_hi_air_coll(gobj);
        } else if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
    }

    /// The rise caught someone (`ftCa_SpecialLw_800E5128`, the grab's `grab_cb`): the catch,
    /// still, holding the victim at its throw bone if it's on the ground (`x221B_b7`), else
    /// letting the victim hold on (`ftCo_800DB368` on its side).
    #[doc(alias = "ftCa_SpecialLw_800E5128")]
    pub fn ca_special_hi_on_grab(&mut self, gobj: GObjId) {
        let victim = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("a catch without a victim");
        self.change_motion_state(gobj, ms::SPECIAL_HI_CATCH, 2, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.x1A6A = 0x1FF;
        self.stop_dead(gobj);
        if self.game.fighter(victim).ground_or_air == super::super::common::GA_AIR {
            self.game.fighter_mut(gobj).x221B.x221B_b7 = false;
        } else {
            // The catcher holds on to the grounded victim's throw bone.
            self.attach_to_thrower(victim, gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.x221B.x221B_b7 = true;
            fp.accessory4_cb = Some(CodeRef(addr::HI_FOLLOW_VICTIM));
        }
    }

    /// `ftCa_SpecialHiCatch_Anim`: the throw when the catch ends (`doCatchAnim`).
    #[doc(alias = "ftCa_SpecialHiCatch_Anim")]
    pub fn ca_special_hi_catch_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ca_special_hi_throw(gobj);
        }
    }

    /// The throw (`doCatchAnim`): its state, the carried speed cleared, the victim hit and
    /// launched.
    #[doc(alias = "doCatchAnim")]
    fn ca_special_hi_throw(&mut self, gobj: GObjId) {
        let victim = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("a throw without a victim");
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_f32(mv::HI_VEL_X, 0.0);
        fp.set_mv_f32(mv::HI_VEL_Y, 0.0);
        self.ca_set_hi_flag(gobj, HI_B0, false);
        self.change_motion_state(gobj, ms::SPECIAL_HI_THROW0, 0x8_0002, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x1A6A = 0;
        self.throw_hit(gobj, victim, true);
        self.thrown_launch(victim, None, false);
    }

    /// `ftCa_SpecialHiCatch_IASA`, `ftCa_SpecialHiCatch_Phys`, `ftCa_SpecialHiThrow0_IASA`:
    /// nothing.
    #[doc(alias = "ftCa_SpecialHiCatch_IASA")]
    #[doc(alias = "ftCa_SpecialHiCatch_Phys")]
    #[doc(alias = "ftCa_SpecialHiThrow0_IASA")]
    pub fn ca_special_hi_nothing(&mut self, _gobj: GObjId) {}

    /// `ftCa_SpecialHiCatch_Coll`: unless holding the victim on the ground, the air's
    /// collision.
    #[doc(alias = "ftCa_SpecialHiCatch_Coll")]
    pub fn ca_special_hi_catch_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x221B.x221B_b7 {
            self.air_coll_stay(gobj);
        }
    }

    /// `ftCa_SpecialHiThrow0_Anim`: a fall when the throw ends; airborne, and when the script
    /// says (`cmd_vars[0]`), the throw's drift (`x2_b0`).
    #[doc(alias = "ftCa_SpecialHiThrow0_Anim")]
    pub fn ca_special_hi_throw0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            self.ca_set_hi_flag(gobj, HI_B0, true);
        }
    }

    /// `ftCa_SpecialHiThrow0_Phys`: with the drift (`x2_b0`), the rise's motion and a fall
    /// at `specialhi_catch_grav` that the carried speed keeps; else the animation's motion.
    #[doc(alias = "ftCa_SpecialHiThrow0_Phys")]
    pub fn ca_special_hi_throw0_phys(&mut self, gobj: GObjId) {
        if !self.ca_hi_flag(gobj, HI_B0) {
            self.air_anim_motion(gobj);
            return;
        }
        self.ca_special_hi_motion(gobj);
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ca_attr_f32(f, attr::SPECIALHI_CATCH_GRAV));
        let fp = &mut self.game.fighters[f.0];
        let vel_y = fp::fsubs(reg(fp.self_vel.y), reg(fp.mv_f32(mv::HI_VEL_Y)));
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        let vy = fp::fsubs(reg(fp.self_vel.y), vel_y);
        fp.set_mv_f32(mv::HI_VEL_Y, vy);
    }

    /// `ftCa_SpecialHiThrow0_Coll`: landing helpless.
    #[doc(alias = "ftCa_SpecialHiThrow0_Coll")]
    pub fn ca_special_hi_throw0_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.ca_attr_f32(f, attr::SPECIALHI_LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// The fighter follows the victim it holds on the ground (`ftCa_SpecialLw_800E550C`,
    /// accessory 4).
    #[doc(alias = "ftCa_SpecialLw_800E550C")]
    pub fn ca_special_hi_follow_victim(&mut self, gobj: GObjId) {
        let victim = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("following no victim");
        let pos = self.game.fighter(victim).cur_pos;
        self.game.fighter_mut(gobj).cur_pos = pos;
    }

    /// The caught fighter, `ftCo_8009CA0C` (the grab's `grabbed_cb`): it lets go of what it
    /// held, faces its catcher, is held at the catcher's throw bone in the air
    /// (`x221B_b7` clear, following it as accessory 1), and is captured.
    #[doc(alias = "ftCo_8009CA0C")]
    pub fn enter_capture_captain(&mut self, gobj: GObjId, catcher: GObjId) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let cfacing = self.game.fighter(catcher).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(catcher);
        fp.victim_gobj = Some(catcher);
        fp.x221B.x221B_b5 = false;
        fp.facing_dir = -cfacing;
        if fp.ground_or_air == super::super::common::GA_AIR {
            self.attach_to_thrower(catcher, gobj);
            self.game.fighter_mut(gobj).x221B.x221B_b7 = false;
        } else {
            self.game.fighter_mut(gobj).x221B.x221B_b7 = true;
        }
        self.change_motion_state(
            gobj,
            super::super::co::ids::ms::CAPTURE_CAPTAIN,
            0,
            0.0,
            1.0,
            0.0,
            Some(catcher),
        );
        if !self.game.fighter(gobj).x221B.x221B_b7 {
            self.game.fighter_mut(gobj).accessory1_cb = Some(CodeRef(addr::HELD_FOLLOW));
        }
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// A held fighter follows its holder's throw bone, `ftCo_800DB464` (accessory 1): at its
    /// XRotN, offset by its throw offset (`x1A70`) at its scale, at depth 0.
    #[doc(alias = "ftCo_800DB464")]
    pub fn held_follow(&mut self, gobj: GObjId) {
        let j = self.part_joint(gobj, part::X_ROT_N);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let fp = self.game.fighter_mut(gobj);
        let scale = reg(fp.x34_scale.y);
        pos.x = mem(fp::fmadds(
            reg(fp.facing_dir),
            fp::fmuls(reg(fp.x1A70.z), scale),
            reg(pos.x),
        ));
        pos.y = mem(fp::fmadds(reg(fp.x1A70.y), scale, reg(pos.y)));
        pos.z = 0.0;
        fp.cur_pos = pos;
    }

    /// `ftCo_CaptureCaptain_Anim`, `ftCo_CaptureCaptain_IASA` and `ftCo_CaptureCaptain_Phys`:
    /// nothing.
    #[doc(alias = "ftCo_CaptureCaptain_Anim")]
    #[doc(alias = "ftCo_CaptureCaptain_IASA")]
    #[doc(alias = "ftCo_CaptureCaptain_Phys")]
    pub fn co_capture_captain_nothing(&mut self, _gobj: GObjId) {}

    /// `ftCo_CaptureCaptain_Coll`: unless held in place (`x2226_b2`), the air's collision.
    #[doc(alias = "ftCo_CaptureCaptain_Coll")]
    pub fn co_capture_captain_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.air_coll_stay(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_ON_CHANGE => g.ca_special_hi_on_change(gobj),
        addr::SPECIAL_HI_ENTER => g.ca_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.ca_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM | addr::SPECIAL_AIR_HI_ANIM => g.ca_special_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => g.ca_special_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS | addr::SPECIAL_AIR_HI_PHYS => g.ca_special_hi_phys(gobj),
        addr::SPECIAL_HI_COLL | addr::SPECIAL_AIR_HI_COLL => g.ca_special_hi_coll(gobj),
        addr::HI_ON_GRAB => g.ca_special_hi_on_grab(gobj),
        addr::SPECIAL_HI_CATCH_ANIM => g.ca_special_hi_catch_anim(gobj),
        addr::SPECIAL_HI_CATCH_IASA
        | addr::SPECIAL_HI_CATCH_PHYS
        | addr::SPECIAL_HI_THROW0_IASA => g.ca_special_hi_nothing(gobj),
        addr::SPECIAL_HI_CATCH_COLL => g.ca_special_hi_catch_coll(gobj),
        addr::SPECIAL_HI_THROW0_ANIM => g.ca_special_hi_throw0_anim(gobj),
        addr::SPECIAL_HI_THROW0_PHYS => g.ca_special_hi_throw0_phys(gobj),
        addr::SPECIAL_HI_THROW0_COLL => g.ca_special_hi_throw0_coll(gobj),
        addr::HI_FOLLOW_VICTIM => g.ca_special_hi_follow_victim(gobj),
        addr::HELD_FOLLOW => g.held_follow(gobj),
        addr::CAPTURE_CAPTAIN_ANIM | addr::CAPTURE_CAPTAIN_IASA | addr::CAPTURE_CAPTAIN_PHYS => {
            g.co_capture_captain_nothing(gobj)
        }
        addr::CAPTURE_CAPTAIN_COLL => g.co_capture_captain_coll(gobj),
        _ => return false,
    }
    true
}
