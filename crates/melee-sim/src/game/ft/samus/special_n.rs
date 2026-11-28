//! The Charge Shot (`ftsamusspecialn.c`): charging it on the ground level by level up to the
//! attributes' maximum (`u` `CHARGE`), keeping or firing the charge, and the shot in the air.
//! The shot is an item (`it::kinds::samuschargeshot`) she holds in her hand while charging.
//!
//! Kirby's copy of it (`ftkirbyspecialsamus.c`) is the same code over his own states,
//! attributes, `u` words, joints, shot kind, flash, glow and hooks, with two branches of its
//! own; a [`ChargeShot`] says which, Samus's [`SAMUS`] or Kirby's
//! (`ft::kirby::special_samus::KIRBY`), and each function here carries both originals' names.

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::samuschargeshot;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{PART_R_HAND_NB, PART_THROW_N, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What tells one Charge Shot from the other.
pub struct ChargeShot {
    pub(crate) ms: States,
    /// Added to `ftSamusAttributes`' offsets: 0, Kirby's `specialn_ss_*` 0x150 on.
    pub(crate) attrs: usize,
    /// Added to the `u` offsets of the shot held, the charge and the effects' flag: 0,
    /// Kirby's 0xA4 on.
    pub(crate) u: usize,
    /// The joint the shot appears on (`FtPart_RHandNb`, Kirby's `FtPart_R3rdNa`).
    pub(crate) hand: i32,
    /// The joint it fires from (`FtPart_ThrowN`, Kirby's the same hand), and the offset the
    /// fire's position is taken with: none for Samus; Kirby's copy passes a zero vector.
    pub(crate) fire_joint: i32,
    pub(crate) fire_offset: Option<Vec3>,
    pub(crate) shot_kind: i32,
    /// The muzzle flash and the full charge's color animation.
    pub(crate) flash: i32,
    pub(crate) full_glow: i32,
    /// The hooks while the shot is out: on a KO (`death2_cb`) and when hit (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// Kirby's: the charge's effects are destroyed whether or not any are out
    /// (`efLib_DestroyAll` unguarded), where Samus's check the effects' flag first.
    pub(crate) effects_off_always: bool,
}

/// The move's states, Samus's names.
pub(crate) struct States {
    pub special_n_start: i32,
    pub special_n_hold: i32,
    pub special_n_cancel: i32,
    pub special_n: i32,
    pub special_air_n_start: i32,
    pub special_air_n: i32,
}

/// Samus's own.
pub const SAMUS: ChargeShot = ChargeShot {
    ms: States {
        special_n_start: ms::SPECIAL_N_START,
        special_n_hold: ms::SPECIAL_N_HOLD,
        special_n_cancel: ms::SPECIAL_N_CANCEL,
        special_n: ms::SPECIAL_N,
        special_air_n_start: ms::SPECIAL_AIR_N_START,
        special_air_n: ms::SPECIAL_AIR_N,
    },
    attrs: 0,
    u: 0,
    hand: PART_R_HAND_NB,
    fire_joint: PART_THROW_N,
    fire_offset: None,
    shot_kind: ItKind::SamusChargeShot.id(),
    flash: 0x486,
    full_glow: 53,
    on_death: addr::DAMAGE_CB,
    on_damage: addr::DAMAGE_CB,
    effects_off_always: false,
};

/// `ftCommon_GroundAirColl_MF`: the flags the switches between the ground and air states keep.
const KEEP: u32 = 0x0C4C_5080;
/// `ftSs_Unk3_803CE6B8` (Kirby's `ftKb_Init_803CB6C0`, the same): the charge's sounds by fifth
/// of the charge.
const CHARGE_SFX: [i32; 5] = [0x3F7A6, 0x3F7A9, 0x3F7AC, 0x3F7AF, 0x3F7B2];
/// The sound of a charge kept when she leaves the ground.
const SFX_KEEP: i32 = 0x3F7B5;
/// `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;
/// `M_PI`.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// The user's hooks while the shot is out (Samus's `ftSamus_SetDamageCallbacks`, Kirby's
    /// generic cleanups).
    fn samus_special_n_set_cbs(&mut self, gobj: GObjId, p: &ChargeShot) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        fp.death2_cb = Some(CodeRef(p.on_death));
    }

    /// `ftSamus_destroyAllEF`: the charge's effects gone, if out.
    pub(crate) fn samus_charge_effects_off(&mut self, gobj: GObjId, p: &ChargeShot) {
        if p.effects_off_always || self.game.fighter(gobj).u_i32(p.u + u::CHARGE_EFFECTS) != 0 {
            ef::eflib::destroy_all(self, gobj);
            self.game
                .fighter_mut(gobj)
                .set_u_i32(p.u + u::CHARGE_EFFECTS, 0);
        }
    }

    /// `ftSamus_UnkAndDestroyAllEF`: the shot in her hand gone (`it_802B5974`), and the
    /// charge's effects.
    pub(crate) fn samus_shot_gone(&mut self, gobj: GObjId, p: &ChargeShot) {
        if let Some(shot) = self.game.fighter(gobj).u.gobj_at(p.u + u::CHARGE_SHOT) {
            samuschargeshot::remove(self, shot);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::CHARGE_SHOT, None);
        }
        self.samus_charge_effects_off(gobj, p);
    }

    /// The shot let go of, `ftSs_SpecialN_801291F0`: she no longer holds it, and the charge's
    /// effects are gone.
    #[doc(alias = "ftSs_SpecialN_801291F0")]
    #[doc(alias = "ftKb_SpecialNSs_800FCD04")]
    pub fn samus_shot_released(&mut self, gobj: GObjId, p: &ChargeShot) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(p.u + u::CHARGE_SHOT).is_some() {
            fp.u.set_gobj_at(p.u + u::CHARGE_SHOT, None);
        }
        self.samus_charge_effects_off(gobj, p);
    }

    /// The charge lost, `ftSs_SpecialN_80129258`: the shot in her hand gone and no charge.
    #[doc(alias = "ftSs_SpecialN_80129258")]
    #[doc(alias = "ftKb_SpecialNSs_800FCD60")]
    pub fn samus_drop_charge(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_gone(gobj, p);
        self.game.fighter_mut(gobj).set_u_i32(p.u + u::CHARGE, 0);
    }

    /// The shot appears in her hand when the script says (`cmd_vars[0]` 1) and she holds
    /// none, `ftSs_SpecialN_801292E4`; true if it couldn't be made.
    #[doc(alias = "ftSs_SpecialN_801292E4")]
    pub fn samus_shot_appear(&mut self, gobj: GObjId, p: &ChargeShot) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 1 || fp.u.gobj_at(p.u + u::CHARGE_SHOT).is_some() {
            return false;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let hand = self.bone_joint(gobj, p.hand);
        let offset = Vec3::new(fp::frsp(0.0) as f32, fp::frsp(0.0) as f32, 4.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, Some(offset));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        let shot = samuschargeshot::spawn(self, gobj, pos, p.hand, p.shot_kind, facing);
        self.game
            .fighter_mut(gobj)
            .u
            .set_gobj_at(p.u + u::CHARGE_SHOT, shot);
        if shot.is_some() {
            self.samus_special_n_set_cbs(gobj, p);
            false
        } else {
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::CHARGE_SHOT, None);
            true
        }
    }

    /// Fires the shot when the script says (`cmd_vars[1]` 1) and she holds one,
    /// `ftSs_SpecialN_801293BC`: from her throw joint, her way, as charged; in the air (or
    /// the aerial shot) she recoils by the charge; then no charge, the flash, and the item
    /// she held kept.
    #[doc(alias = "ftSs_SpecialN_801293BC")]
    #[doc(alias = "ftKb_SpecialNSs_800FCDE0")]
    pub fn samus_shot_fire(&mut self, gobj: GObjId, p: &ChargeShot) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let Some(shot) = fp.u.gobj_at(p.u + u::CHARGE_SHOT) else {
            return;
        };
        if fp.cmd_vars[1] != 1 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[1] = 2;
        let joint = self.bone_joint(gobj, p.fire_joint);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, joint, p.fire_offset);
        pos.z = 0.0;
        let fp = &self.game.fighters[f.0];
        let held = fp.item_gobj;
        let angle = if fp.facing_dir == fp::frsp(1.0) as f32 {
            0.0
        } else {
            PI
        };
        let charge = fp.u_i32(p.u + u::CHARGE) as u32;
        let max = self.samus_attr_f32(f, p.attrs + attr::CHARGE_MAX);
        samuschargeshot::fire(
            self,
            shot,
            pos,
            mem(fp::frsp(angle)),
            mem(fp::frsp(f64::from(charge))),
            max,
        );
        let fp = &self.game.fighters[f.0];
        if fp.motion_id == p.ms.special_air_n || fp.ground_or_air == GroundOrAir::Air {
            let recoil = reg(self.samus_attr_f32(f, p.attrs + attr::CHARGE_RECOIL));
            let fp = &mut self.game.fighters[f.0];
            let c = fp::frsp(f64::from(fp.u_i32(p.u + u::CHARGE)));
            fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(recoil, c)));
        }
        self.game.fighters[f.0].set_u_i32(p.u + u::CHARGE, 0);
        self.samus_shot_released(gobj, p);
        let facing = self.game.fighters[f.0].facing_dir;
        ef::spawn(
            self,
            p.flash,
            Some(gobj),
            &[Arg::Vec(pos), Arg::Float(facing)],
        );
        self.game.fighters[f.0].item_gobj = held;
    }

    /// The neutral special's state on the ground, `ftSs_SpecialN_Enter`.
    #[doc(alias = "ftSs_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNSs_800FCF74")]
    pub fn samus_special_n_enter(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.change_motion_state(gobj, p.ms.special_n_start, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.land(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.samus_special_n_set_cbs(gobj, p);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_AIR, 0);
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_mv_f32(mv::N_X8, 0.0);
        self.anim_update(gobj);
    }

    /// In the air, `ftSs_SpecialAirN_Enter`.
    #[doc(alias = "ftSs_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNSs_800FD020")]
    pub fn samus_special_air_n_enter(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.change_motion_state(gobj, p.ms.special_air_n_start, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.samus_special_n_set_cbs(gobj, p);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_AIR, 1);
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_mv_f32(mv::N_X8, 0.0);
        self.anim_update(gobj);
    }

    /// `ftSs_SpecialNStart_Anim`: the shot appears; at the start's end she fires if she
    /// started in the air or is fully charged, else she charges.
    #[doc(alias = "ftSs_SpecialNStart_Anim")]
    #[doc(alias = "ftKb_SsSpecialNStart_Anim")]
    pub fn samus_special_n_start_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_appear(gobj, p);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.samus_attr_f32(f, p.attrs + attr::CHARGE_MAX));
        let fp = &self.game.fighters[f.0];
        let full = fp::frsp(f64::from(fp.u_i32(p.u + u::CHARGE))) == max;
        if fp.mv_i32(mv::N_AIR) == 1 || full {
            self.change_motion_state(gobj, p.ms.special_n, 0, 0.0, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, p.ms.special_n_hold, 0, 0.0, 1.0, 0.0, None);
            self.game
                .fighter_mut(gobj)
                .set_u_i32(p.u + u::CHARGE_EFFECTS, 0);
        }
        self.samus_special_n_set_cbs(gobj, p);
    }

    /// `ftSs_SpecialNHold_Anim`: the charge's sound when the script asks (`cmd_vars[2]`), and
    /// every `x20` frames a level more; full, she glows and puts the charge away.
    #[doc(alias = "ftSs_SpecialNHold_Anim")]
    #[doc(alias = "ftKb_SsSpecialNHold_Anim")]
    pub fn samus_special_n_hold_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        let f = self.game.fighter_id(gobj);
        let max = reg(self.samus_attr_f32(f, p.attrs + attr::CHARGE_MAX));
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            let charge = self.game.fighters[f.0].u_i32(p.u + u::CHARGE);
            let part = if charge != 0 {
                fp::fdivs(fp::frsp(f64::from(charge)), max)
            } else {
                0.0
            };
            let index = fp::fctiwz(fp::fmuls(fp::frsp(5.0), part));
            self.fighter_play_track(f, Track::Effect2, CHARGE_SFX[index as usize], 127, 64);
        }
        let frames = self.samus_attr_i32(f, p.attrs + attr::CHARGE_FRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FRAMES, fp.mv_i32(mv::N_FRAMES).wrapping_add(1));
        if fp.mv_i32(mv::N_FRAMES) <= frames {
            return;
        }
        fp.set_mv_i32(mv::N_FRAMES, 0);
        fp.set_u_i32(p.u + u::CHARGE, fp.u_i32(p.u + u::CHARGE).wrapping_add(1));
        if fp::frsp(f64::from(fp.u_i32(p.u + u::CHARGE))) >= max {
            self.start_color_anim(gobj, p.full_glow, 0);
            self.game.fighters[f.0].set_u_i32(p.u + u::CHARGE, fp::fctiwz(max));
            self.change_motion_state(gobj, p.ms.special_n_cancel, 0, 0.0, 1.0, 0.0, None);
            self.samus_shot_gone(gobj, p);
            self.samus_special_n_set_cbs(gobj, p);
        }
    }

    /// `ftSs_SpecialNCancel_Anim`: the shot gone, and waiting at the end.
    #[doc(alias = "ftSs_SpecialNCancel_Anim")]
    #[doc(alias = "ftKb_SsSpecialNCancel_Anim")]
    pub fn samus_special_n_cancel_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_gone(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialN_Anim`: the shot fires, and waiting at the end.
    #[doc(alias = "ftSs_SpecialN_Anim")]
    #[doc(alias = "ftKb_SsSpecialN_Anim")]
    pub fn samus_special_n_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_fire(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialAirNStart_Anim`: the shot appears, and she fires at the start's end.
    #[doc(alias = "ftSs_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_SsSpecialAirNStart_Anim")]
    pub fn samus_special_air_n_start_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_appear(gobj, p);
        self.game.fighter_mut(gobj).set_mv_i32(mv::N_AIR, 1);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, p.ms.special_air_n, 0, 0.0, 1.0, 0.0, None);
            self.samus_special_n_set_cbs(gobj, p);
        }
    }

    /// `ftSs_SpecialAirN_Anim`: the shot fires; at the end she falls, helpless if the
    /// attributes give a landing lag.
    #[doc(alias = "ftSs_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_SsSpecialAirN_Anim")]
    pub fn samus_special_air_n_anim(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_shot_fire(gobj, p);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let lag = self.samus_attr_f32(f, p.attrs + attr::N_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// `ftSs_SpecialNHold_IASA`: rolling or spot dodging out keeps the charge (the shot
    /// gone); B fires, a shoulder puts the charge away.
    #[doc(alias = "ftSs_SpecialNHold_IASA")]
    #[doc(alias = "ftKb_SsSpecialNHold_IASA")]
    pub fn samus_special_n_hold_iasa(&mut self, gobj: GObjId, p: &ChargeShot) {
        if self.escape_check(gobj) {
            self.samus_shot_gone(gobj, p);
            return;
        }
        let pressed = self.game.fighter(gobj).input.pressed_buttons;
        if pressed & PAD_B != 0 {
            self.change_motion_state(gobj, p.ms.special_n, 0, 0.0, 1.0, 0.0, None);
            self.samus_special_n_set_cbs(gobj, p);
            return;
        }
        if pressed & PAD_LR != 0 {
            self.change_motion_state(gobj, p.ms.special_n_cancel, 0, 0.0, 1.0, 0.0, None);
            self.samus_shot_gone(gobj, p);
            self.samus_special_n_set_cbs(gobj, p);
        }
    }

    /// Off the ground: `ftSs_SpecialNStart_Coll` and the others, into the aerial state.
    fn samus_special_n_to_air(&mut self, gobj: GObjId, p: &ChargeShot, msid: i32) -> bool {
        if self.ground_coll_off_ledges(gobj) {
            return false;
        }
        self.fox_ground_to_air(gobj, msid, KEEP);
        self.samus_special_n_set_cbs(gobj, p);
        true
    }

    /// `ftSs_SpecialNStart_Coll`.
    #[doc(alias = "ftSs_SpecialNStart_Coll")]
    #[doc(alias = "ftKb_SsSpecialNStart_Coll")]
    pub fn samus_special_n_start_coll(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_special_n_to_air(gobj, p, p.ms.special_air_n_start);
    }

    /// `ftSs_SpecialNHold_Coll`: off the ground mid-charge, she fires (`cmd_vars[1]`).
    #[doc(alias = "ftSs_SpecialNHold_Coll")]
    #[doc(alias = "ftKb_SsSpecialNHold_Coll")]
    pub fn samus_special_n_hold_coll(&mut self, gobj: GObjId, p: &ChargeShot) {
        if self.samus_special_n_to_air(gobj, p, p.ms.special_air_n) {
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, SFX_KEEP, 127, 64);
            self.game.fighters[f.0].cmd_vars[1] = 1;
        }
    }

    /// `ftSs_SpecialNCancel_Coll` and `ftSs_SpecialN_Coll`.
    #[doc(alias = "ftSs_SpecialNCancel_Coll")]
    #[doc(alias = "ftKb_SsSpecialNCancel_Coll")]
    #[doc(alias = "ftSs_SpecialN_Coll")]
    #[doc(alias = "ftKb_SsSpecialN_Coll")]
    pub fn samus_special_n_coll(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_special_n_to_air(gobj, p, p.ms.special_air_n);
    }

    /// Landing: `ftSs_SpecialAirNStart_Coll` and `ftSs_SpecialAirN_Coll`.
    fn samus_special_air_n_land(&mut self, gobj: GObjId, p: &ChargeShot, msid: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid, KEEP);
            self.samus_special_n_set_cbs(gobj, p);
        }
    }

    /// `ftSs_SpecialAirNStart_Coll`.
    #[doc(alias = "ftSs_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_SsSpecialAirNStart_Coll")]
    pub fn samus_special_air_n_start_coll(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_special_air_n_land(gobj, p, p.ms.special_n_start);
    }

    /// `ftSs_SpecialAirN_Coll`.
    #[doc(alias = "ftSs_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_SsSpecialAirN_Coll")]
    pub fn samus_special_air_n_coll(&mut self, gobj: GObjId, p: &ChargeShot) {
        self.samus_special_air_n_land(gobj, p, p.ms.special_n);
    }

    /// Whether the shot she charges is still hers to hold, `ftSs_SpecialLw_80129158`: not
    /// outside the charge's states, nor in them once hit (`x2071_b6`).
    #[doc(alias = "ftSs_SpecialLw_80129158")]
    #[doc(alias = "ftKb_SpecialNSs_800FCC6C")]
    pub fn samus_shot_lost(&self, gobj: Option<GObjId>, p: &ChargeShot) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        match fp.motion_id {
            m if (p.ms.special_n_start..=p.ms.special_air_n).contains(&m) => {
                fp.attack_flags[1] & 0x02 != 0
            }
            _ => true,
        }
    }

    /// Whether she has left the charge's states, `ftSs_SpecialN_801291A8` (the shot fired
    /// from `SpecialN` and the aerial start count as left).
    #[doc(alias = "ftSs_SpecialN_801291A8")]
    #[doc(alias = "ftKb_SpecialNSs_800FCCBC")]
    pub fn samus_left_charge(&self, gobj: Option<GObjId>, p: &ChargeShot) -> bool {
        let Some(gobj) = gobj else { return true };
        let m = self.game.fighter(gobj).motion_id;
        ![
            p.ms.special_n_start,
            p.ms.special_n_hold,
            p.ms.special_n,
            p.ms.special_air_n_start,
            p.ms.special_air_n,
        ]
        .contains(&m)
    }

    /// The charge and its maximum while she holds a shot, `ftSs_SpecialLw_80129100`; None
    /// (-1) otherwise.
    #[doc(alias = "ftSs_SpecialLw_80129100")]
    #[doc(alias = "ftKb_SpecialNSs_800FCC14")]
    pub fn samus_charge_of(&self, gobj: Option<GObjId>, p: &ChargeShot) -> Option<(i32, i32)> {
        let gobj = gobj?;
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        fp.u.gobj_at(p.u + u::CHARGE_SHOT)?;
        let max = self.samus_attr_f32(f, p.attrs + attr::CHARGE_MAX);
        Some((fp.u_i32(p.u + u::CHARGE), fp::fctiwz(reg(max))))
    }
}

#[doc(alias = "ftSs_SpecialNCancel_Phys")]
#[doc(alias = "ftSs_SpecialNHold_Phys")]
#[doc(alias = "ftSs_SpecialNStart_Phys")]
#[doc(alias = "ftSs_SpecialNCancel_IASA")]
#[doc(alias = "ftSs_SpecialNStart_IASA")]
#[doc(alias = "ftSs_SpecialN_Phys")]
#[doc(alias = "ftSs_SpecialN_IASA")]
#[doc(alias = "ftSs_SpecialAirNStart_IASA")]
#[doc(alias = "ftSs_SpecialAirNStart_Phys")]
#[doc(alias = "ftSs_SpecialAirN_IASA")]
#[doc(alias = "ftSs_SpecialAirN_Phys")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &SAMUS;
    match a {
        addr::SHOT_RELEASED => g.samus_shot_released(gobj, p),
        addr::DROP_CHARGE => g.samus_drop_charge(gobj, p),
        addr::SPECIAL_N_ENTER => g.samus_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.samus_special_air_n_enter(gobj, p),
        addr::SPECIAL_N_START_ANIM => g.samus_special_n_start_anim(gobj, p),
        addr::SPECIAL_N_HOLD_ANIM => g.samus_special_n_hold_anim(gobj, p),
        addr::SPECIAL_N_CANCEL_ANIM => g.samus_special_n_cancel_anim(gobj, p),
        addr::SPECIAL_N_ANIM => g.samus_special_n_anim(gobj, p),
        addr::SPECIAL_AIR_N_START_ANIM => g.samus_special_air_n_start_anim(gobj, p),
        addr::SPECIAL_AIR_N_ANIM => g.samus_special_air_n_anim(gobj, p),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_CANCEL_IASA
        | addr::SPECIAL_N_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_IASA => {}
        addr::SPECIAL_N_HOLD_IASA => g.samus_special_n_hold_iasa(gobj, p),
        addr::SPECIAL_N_START_PHYS
        | addr::SPECIAL_N_HOLD_PHYS
        | addr::SPECIAL_N_CANCEL_PHYS
        | addr::SPECIAL_N_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N_START_PHYS | addr::SPECIAL_AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_N_START_COLL => g.samus_special_n_start_coll(gobj, p),
        addr::SPECIAL_N_HOLD_COLL => g.samus_special_n_hold_coll(gobj, p),
        addr::SPECIAL_N_CANCEL_COLL | addr::SPECIAL_N_COLL => g.samus_special_n_coll(gobj, p),
        addr::SPECIAL_AIR_N_START_COLL => g.samus_special_air_n_start_coll(gobj, p),
        addr::SPECIAL_AIR_N_COLL => g.samus_special_air_n_coll(gobj, p),
        _ => return false,
    }
    true
}
