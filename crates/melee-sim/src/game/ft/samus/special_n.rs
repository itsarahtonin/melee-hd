//! The Charge Shot (`ftsamusspecialn.c`): charging it on the ground level by level up to the
//! attributes' maximum (`u` `CHARGE`), keeping or firing the charge, and the shot in the air.
//! The shot is an item (`it::kinds::samuschargeshot`) she holds in her hand while charging.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::it::kinds::samuschargeshot;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{PART_R_HAND_NB, PART_THROW_N, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCommon_GroundAirColl_MF`: the flags the switches between the ground and air states keep.
const KEEP: u32 = 0x0C4C_5080;
/// `ftSs_Unk3_803CE6B8`: the charge's sounds by fifth of the charge.
const CHARGE_SFX: [i32; 5] = [0x3F7A6, 0x3F7A9, 0x3F7AC, 0x3F7AF, 0x3F7B2];
/// The sound of a charge kept when she leaves the ground.
const SFX_KEEP: i32 = 0x3F7B5;
/// The shot's muzzle flash.
const GFX_SHOT: i32 = 0x486;
/// `ftCo_800BFFD0`'s color animation for a full charge.
const COLANIM_CHARGED: i32 = 53;
/// `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;
/// `M_PI`.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// `ftSamus_destroyAllEF`: the charge's effects gone, if out.
    pub(crate) fn samus_charge_effects_off(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u_i32(u::CHARGE_EFFECTS) != 0 {
            ef::eflib::destroy_all(self, gobj);
            self.game.fighter_mut(gobj).set_u_i32(u::CHARGE_EFFECTS, 0);
        }
    }

    /// `ftSamus_UnkAndDestroyAllEF`: the shot in her hand gone (`it_802B5974`), and the
    /// charge's effects.
    pub(crate) fn samus_shot_gone(&mut self, gobj: GObjId) {
        if let Some(shot) = self.game.fighter(gobj).u.gobj_at(u::CHARGE_SHOT) {
            samuschargeshot::remove(self, shot);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(u::CHARGE_SHOT, None);
        }
        self.samus_charge_effects_off(gobj);
    }

    /// The shot let go of, `ftSs_SpecialN_801291F0`: she no longer holds it, and the charge's
    /// effects are gone.
    #[doc(alias = "ftSs_SpecialN_801291F0")]
    pub fn samus_shot_released(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::CHARGE_SHOT).is_some() {
            fp.u.set_gobj_at(u::CHARGE_SHOT, None);
        }
        self.samus_charge_effects_off(gobj);
    }

    /// The charge lost, `ftSs_SpecialN_80129258`: the shot in her hand gone and no charge.
    #[doc(alias = "ftSs_SpecialN_80129258")]
    pub fn samus_drop_charge(&mut self, gobj: GObjId) {
        self.samus_shot_gone(gobj);
        self.game.fighter_mut(gobj).set_u_i32(u::CHARGE, 0);
    }

    /// The shot appears in her hand when the script says (`cmd_vars[0]` 1) and she holds
    /// none, `ftSs_SpecialN_801292E4`; true if it couldn't be made.
    #[doc(alias = "ftSs_SpecialN_801292E4")]
    pub fn samus_shot_appear(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 1 || fp.u.gobj_at(u::CHARGE_SHOT).is_some() {
            return false;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let hand = self.bone_joint(gobj, PART_R_HAND_NB);
        let offset = Vec3::new(fp::frsp(0.0) as f32, fp::frsp(0.0) as f32, 4.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, Some(offset));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        let shot = samuschargeshot::spawn(
            self,
            gobj,
            pos,
            PART_R_HAND_NB,
            samuschargeshot::KIND,
            facing,
        );
        self.game
            .fighter_mut(gobj)
            .u
            .set_gobj_at(u::CHARGE_SHOT, shot);
        if shot.is_some() {
            self.samus_set_damage_cbs(gobj);
            false
        } else {
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(u::CHARGE_SHOT, None);
            true
        }
    }

    /// Fires the shot when the script says (`cmd_vars[1]` 1) and she holds one,
    /// `ftSs_SpecialN_801293BC`: from her throw joint, her way, as charged; in the air (or
    /// the aerial shot) she recoils by the charge; then no charge, the flash, and the item
    /// she held kept.
    #[doc(alias = "ftSs_SpecialN_801293BC")]
    pub fn samus_shot_fire(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let Some(shot) = fp.u.gobj_at(u::CHARGE_SHOT) else {
            return;
        };
        if fp.cmd_vars[1] != 1 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[1] = 2;
        let throw_n = self.bone_joint(gobj, PART_THROW_N);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, throw_n, None);
        pos.z = 0.0;
        let fp = &self.game.fighters[f.0];
        let held = fp.item_gobj;
        let angle = if fp.facing_dir == fp::frsp(1.0) as f32 {
            0.0
        } else {
            PI
        };
        let charge = fp.u_i32(u::CHARGE) as u32;
        let max = self.samus_attr_f32(f, attr::CHARGE_MAX);
        samuschargeshot::fire(
            self,
            shot,
            pos,
            mem(fp::frsp(angle)),
            mem(fp::frsp(f64::from(charge))),
            max,
        );
        let fp = &self.game.fighters[f.0];
        if fp.motion_id == ms::SPECIAL_AIR_N || fp.ground_or_air == super::super::common::GA_AIR {
            let recoil = reg(self.samus_attr_f32(f, attr::CHARGE_RECOIL));
            let fp = &mut self.game.fighters[f.0];
            let c = fp::frsp(f64::from(fp.u_i32(u::CHARGE)));
            fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(recoil, c)));
        }
        self.game.fighters[f.0].set_u_i32(u::CHARGE, 0);
        self.samus_shot_released(gobj);
        let facing = self.game.fighters[f.0].facing_dir;
        ef::spawn(
            self,
            GFX_SHOT,
            Some(gobj),
            &[Arg::Vec(pos), Arg::Float(facing)],
        );
        self.game.fighters[f.0].item_gobj = held;
    }

    /// The neutral special's state on the ground, `ftSs_SpecialN_Enter`.
    #[doc(alias = "ftSs_SpecialN_Enter")]
    pub fn samus_special_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.land(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.samus_set_damage_cbs(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_AIR, 0);
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_mv_f32(mv::N_X8, 0.0);
        self.anim_update(gobj);
    }

    /// In the air, `ftSs_SpecialAirN_Enter`.
    #[doc(alias = "ftSs_SpecialAirN_Enter")]
    pub fn samus_special_air_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.samus_set_damage_cbs(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_AIR, 1);
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_mv_f32(mv::N_X8, 0.0);
        self.anim_update(gobj);
    }

    /// `ftSs_SpecialNStart_Anim`: the shot appears; at the start's end she fires if she
    /// started in the air or is fully charged, else she charges.
    #[doc(alias = "ftSs_SpecialNStart_Anim")]
    pub fn samus_special_n_start_anim(&mut self, gobj: GObjId) {
        self.samus_shot_appear(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.samus_attr_f32(f, attr::CHARGE_MAX));
        let fp = &self.game.fighters[f.0];
        let full = fp::frsp(f64::from(fp.u_i32(u::CHARGE))) == max;
        if fp.mv_i32(mv::N_AIR) == 1 || full {
            self.change_motion_state(gobj, ms::SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, ms::SPECIAL_N_HOLD, 0, 0.0, 1.0, 0.0, None);
            self.game.fighter_mut(gobj).set_u_i32(u::CHARGE_EFFECTS, 0);
        }
        self.samus_set_damage_cbs(gobj);
    }

    /// `ftSs_SpecialNHold_Anim`: the charge's sound when the script asks (`cmd_vars[2]`), and
    /// every `x20` frames a level more; full, she glows and puts the charge away.
    #[doc(alias = "ftSs_SpecialNHold_Anim")]
    pub fn samus_special_n_hold_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let max = reg(self.samus_attr_f32(f, attr::CHARGE_MAX));
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            let charge = self.game.fighters[f.0].u_i32(u::CHARGE);
            let part = if charge != 0 {
                fp::fdivs(fp::frsp(f64::from(charge)), max)
            } else {
                0.0
            };
            let index = fp::fctiwz(fp::fmuls(fp::frsp(5.0), part));
            self.fighter_play_track(f, Track::Effect2, CHARGE_SFX[index as usize], 127, 64);
        }
        let frames = self.samus_attr_i32(f, attr::CHARGE_FRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FRAMES, fp.mv_i32(mv::N_FRAMES).wrapping_add(1));
        if fp.mv_i32(mv::N_FRAMES) <= frames {
            return;
        }
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_u_i32(u::CHARGE, fp.u_i32(u::CHARGE).wrapping_add(1));
        if fp::frsp(f64::from(fp.u_i32(u::CHARGE))) >= max {
            self.start_color_anim(gobj, COLANIM_CHARGED, 0);
            self.game.fighters[f.0].set_u_i32(u::CHARGE, fp::fctiwz(max));
            self.change_motion_state(gobj, ms::SPECIAL_N_CANCEL, 0, 0.0, 1.0, 0.0, None);
            self.samus_shot_gone(gobj);
            self.samus_set_damage_cbs(gobj);
        }
    }

    /// `ftSs_SpecialNCancel_Anim`: the shot gone, and waiting at the end.
    #[doc(alias = "ftSs_SpecialNCancel_Anim")]
    pub fn samus_special_n_cancel_anim(&mut self, gobj: GObjId) {
        self.samus_shot_gone(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialN_Anim`: the shot fires, and waiting at the end.
    #[doc(alias = "ftSs_SpecialN_Anim")]
    pub fn samus_special_n_anim(&mut self, gobj: GObjId) {
        self.samus_shot_fire(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialAirNStart_Anim`: the shot appears, and she fires at the start's end.
    #[doc(alias = "ftSs_SpecialAirNStart_Anim")]
    pub fn samus_special_air_n_start_anim(&mut self, gobj: GObjId) {
        self.samus_shot_appear(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(mv::N_AIR, 1);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
            self.samus_set_damage_cbs(gobj);
        }
    }

    /// `ftSs_SpecialAirN_Anim`: the shot fires; at the end she falls, helpless if the
    /// attributes give a landing lag.
    #[doc(alias = "ftSs_SpecialAirN_Anim")]
    pub fn samus_special_air_n_anim(&mut self, gobj: GObjId) {
        self.samus_shot_fire(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let lag = self.samus_attr_f32(f, attr::N_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// `ftSs_SpecialNHold_IASA`: rolling or spot dodging out keeps the charge (the shot
    /// gone); B fires, a shoulder puts the charge away.
    #[doc(alias = "ftSs_SpecialNHold_IASA")]
    pub fn samus_special_n_hold_iasa(&mut self, gobj: GObjId) {
        if self.escape_check(gobj) {
            self.samus_shot_gone(gobj);
            return;
        }
        let pressed = self.game.fighter(gobj).input.pressed_buttons;
        if pressed & PAD_B != 0 {
            self.change_motion_state(gobj, ms::SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
            self.samus_set_damage_cbs(gobj);
            return;
        }
        if pressed & PAD_LR != 0 {
            self.change_motion_state(gobj, ms::SPECIAL_N_CANCEL, 0, 0.0, 1.0, 0.0, None);
            self.samus_shot_gone(gobj);
            self.samus_set_damage_cbs(gobj);
        }
    }

    /// Off the ground: `ftSs_SpecialNStart_Coll` and the others, into the aerial state.
    fn samus_special_n_to_air(&mut self, gobj: GObjId, msid: i32) -> bool {
        if self.ground_coll_off_ledges(gobj) {
            return false;
        }
        self.fox_ground_to_air(gobj, msid, KEEP);
        self.samus_set_damage_cbs(gobj);
        true
    }

    /// `ftSs_SpecialNStart_Coll`.
    #[doc(alias = "ftSs_SpecialNStart_Coll")]
    pub fn samus_special_n_start_coll(&mut self, gobj: GObjId) {
        self.samus_special_n_to_air(gobj, ms::SPECIAL_AIR_N_START);
    }

    /// `ftSs_SpecialNHold_Coll`: off the ground mid-charge, she fires (`cmd_vars[1]`).
    #[doc(alias = "ftSs_SpecialNHold_Coll")]
    pub fn samus_special_n_hold_coll(&mut self, gobj: GObjId) {
        if self.samus_special_n_to_air(gobj, ms::SPECIAL_AIR_N) {
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, SFX_KEEP, 127, 64);
            self.game.fighters[f.0].cmd_vars[1] = 1;
        }
    }

    /// `ftSs_SpecialNCancel_Coll` and `ftSs_SpecialN_Coll`.
    #[doc(alias = "ftSs_SpecialNCancel_Coll")]
    #[doc(alias = "ftSs_SpecialN_Coll")]
    pub fn samus_special_n_coll(&mut self, gobj: GObjId) {
        self.samus_special_n_to_air(gobj, ms::SPECIAL_AIR_N);
    }

    /// Landing: `ftSs_SpecialAirNStart_Coll` and `ftSs_SpecialAirN_Coll`.
    fn samus_special_air_n_land(&mut self, gobj: GObjId, msid: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid, KEEP);
            self.samus_set_damage_cbs(gobj);
        }
    }

    /// `ftSs_SpecialAirNStart_Coll`.
    #[doc(alias = "ftSs_SpecialAirNStart_Coll")]
    pub fn samus_special_air_n_start_coll(&mut self, gobj: GObjId) {
        self.samus_special_air_n_land(gobj, ms::SPECIAL_N_START);
    }

    /// `ftSs_SpecialAirN_Coll`.
    #[doc(alias = "ftSs_SpecialAirN_Coll")]
    pub fn samus_special_air_n_coll(&mut self, gobj: GObjId) {
        self.samus_special_air_n_land(gobj, ms::SPECIAL_N);
    }

    /// Whether the shot she charges is still hers to hold, `ftSs_SpecialLw_80129158`: not
    /// outside the charge's states, nor in them once hit (`x2071_b6`).
    #[doc(alias = "ftSs_SpecialLw_80129158")]
    pub fn samus_shot_lost(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        match fp.motion_id {
            0x157..=0x15C => fp.x2070[1] & 0x02 != 0,
            _ => true,
        }
    }

    /// Whether she has left the charge's states, `ftSs_SpecialN_801291A8` (the shot fired
    /// from `SpecialN` and the aerial start count as left).
    #[doc(alias = "ftSs_SpecialN_801291A8")]
    pub fn samus_left_charge(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        !matches!(
            self.game.fighter(gobj).motion_id,
            0x157 | 0x158 | 0x15A | 0x15B | 0x15C
        )
    }

    /// The charge and its maximum while she holds a shot, `ftSs_SpecialLw_80129100`; None
    /// (-1) otherwise.
    #[doc(alias = "ftSs_SpecialLw_80129100")]
    pub fn samus_charge_of(&self, gobj: Option<GObjId>) -> Option<(i32, i32)> {
        let gobj = gobj?;
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        fp.u.gobj_at(u::CHARGE_SHOT)?;
        let max = self.samus_attr_f32(f, attr::CHARGE_MAX);
        Some((fp.u_i32(u::CHARGE), fp::fctiwz(reg(max))))
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_801291F0 => g.samus_shot_released(gobj),
        addr::SPECIAL_N_80129258 => g.samus_drop_charge(gobj),
        addr::SPECIAL_N_ENTER => g.samus_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.samus_special_air_n_enter(gobj),
        addr::SPECIAL_N_START_ANIM => g.samus_special_n_start_anim(gobj),
        addr::SPECIAL_N_HOLD_ANIM => g.samus_special_n_hold_anim(gobj),
        addr::SPECIAL_N_CANCEL_ANIM => g.samus_special_n_cancel_anim(gobj),
        addr::SPECIAL_N_ANIM => g.samus_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_START_ANIM => g.samus_special_air_n_start_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.samus_special_air_n_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_CANCEL_IASA
        | addr::SPECIAL_N_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_IASA => {}
        addr::SPECIAL_N_HOLD_IASA => g.samus_special_n_hold_iasa(gobj),
        addr::SPECIAL_N_START_PHYS
        | addr::SPECIAL_N_HOLD_PHYS
        | addr::SPECIAL_N_CANCEL_PHYS
        | addr::SPECIAL_N_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N_START_PHYS | addr::SPECIAL_AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_N_START_COLL => g.samus_special_n_start_coll(gobj),
        addr::SPECIAL_N_HOLD_COLL => g.samus_special_n_hold_coll(gobj),
        addr::SPECIAL_N_CANCEL_COLL | addr::SPECIAL_N_COLL => g.samus_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_START_COLL => g.samus_special_air_n_start_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.samus_special_air_n_coll(gobj),
        _ => return false,
    }
    true
}
