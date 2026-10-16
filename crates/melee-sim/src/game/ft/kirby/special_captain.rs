//! Captain Falcon's and Ganondorf's copies (`ftkirbyspecialcaptain.c`): the Falcon and Warlock
//! Punch, wind-up and punch, on the ground and in the air, where the stick angles the lunge.
//! Captain Falcon's own (`ft::captain::special_n`) with Kirby's states and attributes, and his
//! fist's flame from his top and hand joints.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::{attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_99BC;
    pub const AIR_ENTER: u32 = 0x800F_9A54;
    pub const N_ANIM: u32 = 0x800F_9AEC;
    pub const AIR_N_ANIM: u32 = 0x800F_9B28;
    pub const N_IASA: u32 = 0x800F_9B64;
    pub const AIR_N_IASA: u32 = 0x800F_9B68;
    pub const N_PHYS: u32 = 0x800F_9C58;
    pub const AIR_N_PHYS: u32 = 0x800F_9D40;
    pub const N_COLL: u32 = 0x800F_9E8C;
    pub const AIR_N_COLL: u32 = 0x800F_9F34;
}

/// The copied kinds.
const KIND_CAPTAIN: i32 = 2;
const KIND_GANON: i32 = 25;
/// The flags the punch keeps between the ground and the air (`ftKb_Ca_transition_flags`).
const COLL_FLAGS: u32 = 0x0C4C_5882;
/// The fist's flame, Captain Falcon's copy's and Ganondorf's.
const GFX_CA: i32 = 0x4AB;
const GFX_GN: i32 = 0x4B3;
/// The flame's bones (`fp->parts[0]` and `[44]`).
const BONE_TOP: i32 = 0;
const BONE_FIST: i32 = 44;

impl Running<'_> {
    /// The punch's state for his copy: Captain Falcon's, or Ganondorf's for any other.
    fn kb_ca_state(&self, gobj: GObjId, captain: i32, ganon: i32) -> i32 {
        if self.game.fighter(gobj).u_i32(u::HAT_KIND) != KIND_CAPTAIN {
            ganon
        } else {
            captain
        }
    }

    fn kb_ca_start(&mut self, gobj: GObjId, captain: i32, ganon: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        let msid = self.kb_ca_state(gobj, captain, ganon);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_effect_hitlag_cbs(gobj);
        self.anim_update(gobj);
    }

    /// The punch on the ground, `ftKb_SpecialNCa_800F99BC`.
    #[doc(alias = "ftKb_SpecialNCa_800F99BC")]
    pub fn kb_ca_enter(&mut self, gobj: GObjId) {
        self.kb_ca_start(gobj, ms::CA_SPECIAL_N, ms::GN_SPECIAL_N);
    }

    /// In the air, `ftKb_SpecialNCa_800F9A54`.
    #[doc(alias = "ftKb_SpecialNCa_800F9A54")]
    pub fn kb_ca_air_enter(&mut self, gobj: GObjId) {
        self.kb_ca_start(gobj, ms::CA_SPECIAL_AIR_N, ms::GN_SPECIAL_AIR_N);
    }

    /// `ftKb_CaSpecialN_Anim`: waiting when the punch ends.
    #[doc(alias = "ftKb_CaSpecialN_Anim")]
    pub fn kb_ca_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_CaSpecialAirN_Anim`: a fall when the punch ends.
    #[doc(alias = "ftKb_CaSpecialAirN_Anim")]
    pub fn kb_ca_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_CaSpecialN_IASA`: nothing.
    #[doc(alias = "ftKb_CaSpecialN_IASA")]
    pub fn kb_ca_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_CaSpecialAirN_IASA`: when the script says (`cmd_vars[0]`), the lunge at
    /// `specialn_ca_forward_momentum` along the stick's tilt: its y beyond the attributes'
    /// x range, up to their y range, scaled to their angle in degrees, signed as the stick.
    #[doc(alias = "ftKb_CaSpecialAirN_IASA")]
    pub fn kb_ca_air_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let f = self.game.fighter_id(gobj);
        let y = reg(self.game.fighters[f.0].input.lstick[0].y);
        let mut stick = if y < 0.0 { fp::fneg(y) } else { y };
        let max = reg(self.kb_attr_f32(f, attr::CA_Y_RANGE));
        if stick > max {
            stick = max;
        }
        let min = reg(self.kb_attr_f32(f, attr::CA_X_RANGE));
        let mut range = fp::fsubs(stick, min);
        if range < 0.0 {
            range = 0.0;
        }
        if reg(self.game.fighters[f.0].input.lstick[0].y) < 0.0 {
            range = fp::fneg(range);
        }
        let diff = reg(self.kb_attr_f32(f, attr::CA_ANGLE_DIFF));
        let angle = fp::fmuls(
            0.017_453_292_384_743_69,
            fp::fdivs(fp::fmuls(range, diff), fp::fsubs(max, min)),
        );
        let a = mem(angle);
        let vel = reg(self.kb_attr_f32(f, attr::CA_MOMENTUM));
        let s = reg(crate::math::sin(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fmuls(vel, s));
        let c = reg(crate::math::cos(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(vel, fp::fmuls(reg(fp.facing_dir), c)));
    }

    /// The fist's flame when the script raises its flag: the first time, his copy's flame
    /// from his top and hand joints; again, his effects end.
    fn kb_ca_flame(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(1) {
            return;
        }
        fp.set_throw_flag(1, false);
        if fp.x2219_b0 {
            self.destroy_effects(gobj);
            return;
        }
        let id = match fp.u_i32(u::HAT_KIND) {
            KIND_CAPTAIN => Some(GFX_CA),
            KIND_GANON => Some(GFX_GN),
            _ => None,
        };
        if let Some(id) = id {
            let top = self.bone_joint(gobj, BONE_TOP);
            let fist = self.bone_joint(gobj, BONE_FIST);
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::JObj(fist)]);
        }
        self.game.fighter_mut(gobj).x2219_b0 = true;
    }

    /// `ftKb_CaSpecialN_Phys`: the flame, then friction or the animation's motion.
    #[doc(alias = "ftKb_CaSpecialN_Phys")]
    pub fn kb_ca_phys(&mut self, gobj: GObjId) {
        self.kb_ca_flame(gobj);
        self.ground_friction_or_anim(gobj);
    }

    /// `ftKb_CaSpecialAirN_Phys`: the flame, then by the script's phase (`cmd_vars[1]`) the
    /// air's physics without drift, the lunge slowing by
    /// `specialn_ca_additional_vertical_momentum`, or the air's physics.
    #[doc(alias = "ftKb_CaSpecialAirN_Phys")]
    pub fn kb_ca_air_phys(&mut self, gobj: GObjId) {
        self.kb_ca_flame(gobj);
        match self.game.fighter(gobj).cmd_vars[1] {
            0 => self.air_physics_no_drift(gobj),
            1 => {
                let f = self.game.fighter_id(gobj);
                let mul = reg(self.kb_attr_f32(f, attr::CA_MOMENTUM_MUL));
                let fp = &mut self.game.fighters[f.0];
                fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), mul));
                fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), mul));
            }
            2 => self.air_physics(gobj),
            _ => {}
        }
    }

    /// `ftKb_CaSpecialN_Coll`: off the ground, the aerial punch from the same frame.
    #[doc(alias = "ftKb_CaSpecialN_Coll")]
    pub fn kb_ca_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            let msid = self.kb_ca_state(gobj, ms::CA_SPECIAL_AIR_N, ms::GN_SPECIAL_AIR_N);
            self.fox_ground_to_air(gobj, msid, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
            let f = self.game.fighter_id(gobj);
            self.clamp_air_drift(f);
        }
    }

    /// `ftKb_CaSpecialAirN_Coll`: landing, the grounded punch from the same frame.
    #[doc(alias = "ftKb_CaSpecialAirN_Coll")]
    pub fn kb_ca_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let msid = self.kb_ca_state(gobj, ms::CA_SPECIAL_N, ms::GN_SPECIAL_N);
            self.fox_air_to_ground(gobj, msid, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTER => g.kb_ca_enter(gobj),
        addr::AIR_ENTER => g.kb_ca_air_enter(gobj),
        addr::N_ANIM => g.kb_ca_anim(gobj),
        addr::AIR_N_ANIM => g.kb_ca_air_anim(gobj),
        addr::N_IASA => g.kb_ca_iasa(gobj),
        addr::AIR_N_IASA => g.kb_ca_air_iasa(gobj),
        addr::N_PHYS => g.kb_ca_phys(gobj),
        addr::AIR_N_PHYS => g.kb_ca_air_phys(gobj),
        addr::N_COLL => g.kb_ca_coll(gobj),
        addr::AIR_N_COLL => g.kb_ca_air_coll(gobj),
        _ => return false,
    }
    true
}
