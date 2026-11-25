//! The Falcon and Warlock Punch (`ftcaptainspecialn.c`): the wind-up and the punch, on the
//! ground and in the air, where the stick angles the lunge.
//!
//! Kirby's copies of them (`ftkirbyspecialcaptain.c`) are the same code over his own states,
//! attributes, flames and fist, the kind they run as being his copy's, without the Warlock
//! Punch's wind; a [`FalconPunch`] says which, Captain Falcon's [`CAPTAIN`] (Ganondorf's too)
//! or Kirby's (`ft::kirby::special_captain::KIRBY`), and each function here carries both
//! originals' names.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::super::kirby::KindFrom;
use super::{addr, attr, ms, part};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::hsd::gobj::GObjId;

/// What tells one punch from another: whose kind picks Captain Falcon's or Ganondorf's, the
/// user's states for each, its attributes, the fist's flame and bone, and the wind.
pub struct FalconPunch {
    /// The fighter's own kind (Captain Falcon's or Ganondorf's), or Kirby's copy.
    pub(crate) kind: KindFrom,
    /// The states on the ground and in the air as Captain Falcon, then as Ganondorf (Captain
    /// Falcon's own are the same: Ganondorf's state table has them at the same numbers).
    pub(crate) captain: (i32, i32),
    pub(crate) ganon: (i32, i32),
    /// Added to `ftCaptainAttributes`' offsets: 0, Kirby's `specialn_ca_*` 0x1F8.
    pub(crate) attrs: usize,
    /// The fist's flame as Captain Falcon, then as Ganondorf.
    pub(crate) flame: (i32, i32),
    /// The flame's second bone, as Captain Falcon then Ganondorf (`fp->parts[57]` and `[78]`,
    /// Kirby's `[44]` for both).
    pub(crate) fist: (i32, i32),
    /// Ganondorf's Warlock Punch blows wind; Kirby's copy of it doesn't.
    pub(crate) wind: bool,
}

/// Captain Falcon's and Ganondorf's own.
pub const CAPTAIN: FalconPunch = FalconPunch {
    kind: KindFrom::Fighter,
    captain: (ms::SPECIAL_N, ms::SPECIAL_AIR_N),
    ganon: (ms::SPECIAL_N, ms::SPECIAL_AIR_N),
    attrs: 0,
    flame: (0x48F, 0x50B),
    fist: (57, 78),
    wind: true,
};

/// The flags the punch keeps going between the ground and the air (`transition_flags`).
const COLL_FLAGS: u32 = 0x0C4C_5882;

impl Running<'_> {
    /// The state of the punch for the kind the user runs it as: Captain Falcon's, or
    /// Ganondorf's for any other.
    fn ca_special_n_state(&self, gobj: GObjId, p: &FalconPunch, air: bool) -> i32 {
        let (ground, aerial) = if self.special_kind(gobj, p.kind) != FighterKind::Captain {
            p.ganon
        } else {
            p.captain
        };
        if air { aerial } else { ground }
    }

    /// The punch's start, on the ground or in the air.
    fn ca_special_n_start(&mut self, gobj: GObjId, p: &FalconPunch, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.throw_flags = [0; 4];
        let msid = self.ca_special_n_state(gobj, p, air);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_effect_hitlag_cbs(gobj);
        self.anim_update(gobj);
    }

    /// `ftCa_SpecialN_Enter` (Kirby's `ftKb_SpecialNCa_800F99BC`).
    #[doc(alias = "ftCa_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNCa_800F99BC")]
    pub fn ca_special_n_enter(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_start(gobj, p, false);
    }

    /// `ftCa_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNCa_800F9A54`).
    #[doc(alias = "ftCa_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNCa_800F9A54")]
    pub fn ca_special_air_n_enter(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_start(gobj, p, true);
    }

    /// The Warlock Punch's wind (`ftCaptain_SpecialN_CreateWindEffect`): on Ganondorf's odd
    /// frames, a small blast through its wind-up and a stronger one as it lands.
    fn ca_special_n_wind(&mut self, gobj: GObjId, p: &FalconPunch) {
        if !p.wind {
            return;
        }
        let fp = self.game.fighter(gobj);
        let frame = fp::fctiwz(reg(fp.cur_anim_frame));
        if fp.kind != FighterKind::Ganon || frame & 1 == 0 {
            return;
        }
        let pos = fp.cur_pos;
        let scale = if (16..=50).contains(&frame) {
            2.0
        } else if (51..=68).contains(&frame) {
            4.0
        } else {
            return;
        };
        crate::lb::wind::blast(self.game.gr.winds_mut(), pos, 2, scale, scale, 0.0);
    }

    /// `ftCa_SpecialN_Anim`: the wind, then waiting when the punch ends.
    #[doc(alias = "ftCa_SpecialN_Anim")]
    #[doc(alias = "ftKb_CaSpecialN_Anim")]
    pub fn ca_special_n_anim(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_wind(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCa_SpecialAirN_Anim`: the wind, then a fall when the punch ends.
    #[doc(alias = "ftCa_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_CaSpecialAirN_Anim")]
    pub fn ca_special_air_n_anim(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_wind(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The stick's tilt of the aerial punch, in radians
    /// (`ftCaptain_SpecialN_GetAngleVel`): its y beyond `specialn_stick_range_y_neg`, up to
    /// `_pos`, scaled to `specialn_angle_diff` degrees, signed as the stick.
    fn ca_special_n_angle(&self, gobj: GObjId, p: &FalconPunch) -> f64 {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let y = reg(fp.input.lstick[0].y);
        let mut stick = if y < 0.0 { fp::fneg(y) } else { y };
        let max = reg(self.ca_attr_f32(f, p.attrs + attr::SPECIALN_STICK_RANGE_Y_POS));
        if stick > max {
            stick = max;
        }
        let min = reg(self.ca_attr_f32(f, p.attrs + attr::SPECIALN_STICK_RANGE_Y_NEG));
        stick = fp::fsubs(stick, min);
        if stick < 0.0 {
            stick = 0.0;
        }
        if y < 0.0 {
            stick = fp::fneg(stick);
        }
        let diff = reg(self.ca_attr_f32(f, p.attrs + attr::SPECIALN_ANGLE_DIFF));
        fp::fmuls(
            0.017_453_292_384_743_69,
            fp::fdivs(fp::fmuls(stick, diff), fp::fsubs(max, min)),
        )
    }

    /// `ftCa_SpecialN_IASA`: nothing.
    #[doc(alias = "ftCa_SpecialN_IASA")]
    #[doc(alias = "ftKb_CaSpecialN_IASA")]
    pub fn ca_special_n_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCa_SpecialAirN_IASA`: when the script says (`cmd_vars[0]`), the lunge at
    /// `specialn_vel_x` along the stick's tilt.
    #[doc(alias = "ftCa_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_CaSpecialAirN_IASA")]
    pub fn ca_special_air_n_iasa(&mut self, gobj: GObjId, p: &FalconPunch) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let f = self.game.fighter_id(gobj);
        let a = mem(self.ca_special_n_angle(gobj, p));
        let vel = reg(self.ca_attr_f32(f, p.attrs + attr::SPECIALN_VEL_X));
        let s = reg(crate::math::sin(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fmuls(vel, s));
        let c = reg(crate::math::cos(a));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(vel, fp::fmuls(reg(fp.facing_dir), c)));
    }

    /// The punch's effect when the script raises its flag (`doPhys`): the first time, the
    /// fist's flame of the kind the user runs the punch as (none for another kind); again,
    /// its effects end.
    fn ca_special_n_effect(&mut self, gobj: GObjId, p: &FalconPunch) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(1) {
            return;
        }
        fp.set_throw_flag(1, false);
        if fp.effect_once {
            self.destroy_effects(gobj);
            return;
        }
        let kind = self.special_kind(gobj, p.kind);
        let bone = if kind == FighterKind::Ganon {
            p.fist.1
        } else {
            p.fist.0
        };
        let id = match FighterKind::from_id(kind) {
            Some(FighterKind::Captain) => Some(p.flame.0),
            Some(FighterKind::Ganon) => Some(p.flame.1),
            _ => None,
        };
        if let Some(id) = id {
            let top = self.bone_joint(gobj, part::TOP_N);
            let fist = self.bone_joint(gobj, bone);
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::JObj(fist)]);
        }
        self.game.fighter_mut(gobj).effect_once = true;
    }

    /// `ftCa_SpecialN_Phys`: the effect, then friction or the animation's motion.
    #[doc(alias = "ftCa_SpecialN_Phys")]
    #[doc(alias = "ftKb_CaSpecialN_Phys")]
    pub fn ca_special_n_phys(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_effect(gobj, p);
        self.ground_friction_or_anim(gobj);
    }

    /// `ftCa_SpecialAirN_Phys`: the effect, then by the script's phase (`cmd_vars[1]`) the
    /// air's physics without drift, the lunge slowing by `specialn_vel_mul`, or the air's
    /// physics.
    #[doc(alias = "ftCa_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_CaSpecialAirN_Phys")]
    pub fn ca_special_air_n_phys(&mut self, gobj: GObjId, p: &FalconPunch) {
        self.ca_special_n_effect(gobj, p);
        match self.game.fighter(gobj).cmd_vars[1] {
            0 => self.air_physics_no_drift(gobj),
            1 => {
                let f = self.game.fighter_id(gobj);
                let mul = reg(self.ca_attr_f32(f, p.attrs + attr::SPECIALN_VEL_MUL));
                let fp = &mut self.game.fighters[f.0];
                fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), mul));
                fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), mul));
            }
            2 => self.air_physics(gobj),
            _ => {}
        }
    }

    /// `ftCa_SpecialN_Coll`: off the ground, the aerial punch from the same frame.
    #[doc(alias = "ftCa_SpecialN_Coll")]
    #[doc(alias = "ftKb_CaSpecialN_Coll")]
    pub fn ca_special_n_coll(&mut self, gobj: GObjId, p: &FalconPunch) {
        if !self.ground_coll_slide(gobj) {
            let msid = self.ca_special_n_state(gobj, p, true);
            self.fox_ground_to_air(gobj, msid, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
            let f = self.game.fighter_id(gobj);
            self.clamp_air_drift(f);
        }
    }

    /// `ftCa_SpecialAirN_Coll`: landing, the grounded punch from the same frame.
    #[doc(alias = "ftCa_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_CaSpecialAirN_Coll")]
    pub fn ca_special_air_n_coll(&mut self, gobj: GObjId, p: &FalconPunch) {
        if self.air_coll_land(gobj) {
            let msid = self.ca_special_n_state(gobj, p, false);
            self.fox_air_to_ground(gobj, msid, COLL_FLAGS);
            self.set_effect_hitlag_cbs(gobj);
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &CAPTAIN;
    match a {
        addr::SPECIAL_N_ENTER => g.ca_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.ca_special_air_n_enter(gobj, p),
        addr::SPECIAL_N_ANIM => g.ca_special_n_anim(gobj, p),
        addr::SPECIAL_AIR_N_ANIM => g.ca_special_air_n_anim(gobj, p),
        addr::SPECIAL_N_IASA => g.ca_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.ca_special_air_n_iasa(gobj, p),
        addr::SPECIAL_N_PHYS => g.ca_special_n_phys(gobj, p),
        addr::SPECIAL_AIR_N_PHYS => g.ca_special_air_n_phys(gobj, p),
        addr::SPECIAL_N_COLL => g.ca_special_n_coll(gobj, p),
        addr::SPECIAL_AIR_N_COLL => g.ca_special_air_n_coll(gobj, p),
        _ => return false,
    }
    true
}
