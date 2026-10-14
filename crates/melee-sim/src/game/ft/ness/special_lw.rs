//! PSI Magnet (`ftnessspeciallw.c`): a bubble that absorbs energy projectiles while B is held
//! (the attributes' `AbsorbDesc`), healing Ness by what it takes. Its turnaround states have
//! callbacks, but nothing enters them.

use gekko_fp as fp;

use super::super::super::ef::{self, SpawnKind};
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::motion::flags::*;
use super::super::sound::Track;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What PSI Magnet's states keep between the ground and the air (`FTNESS_SPECIALLW_COLL_FLAG`)
/// and its end's (`FTNESS_SPECIALLW_END_FLAG`).
const END_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const COLL_FLAGS: u32 = END_FLAGS | KEEP_GFX;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// `FtPart_L1stNb`, the hand the bubble's effect follows.
const PART_L1ST_NB: i32 = 23;

/// The bubble's effect and its hum.
const GFX_BUBBLE: i32 = 1264;
const SFX_HUM: i32 = 210_081;

impl Running<'_> {
    /// PSI Magnet's start: its release lag, gravity delay and hum timer.
    fn ness_special_lw_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = fp::fctiwz(reg(self.ness_attr_f32(f, attr::PSI_MAGNET_RELEASE_LAG)));
        let delay = self.ness_attr_i32(f, attr::PSI_MAGNET_FRAMES_BEFORE_GRAVITY);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::LW_RELEASE_LAG, lag);
        fp.set_mv_i32(mv::LW_IS_RELEASE, 0);
        fp.set_mv_i32(mv::LW_GRAVITY_DELAY, delay);
        fp.set_mv_i32(mv::LW_X10, 0);
    }

    /// `ftNs_SpecialLwStart_Enter`.
    #[doc(alias = "ftNs_SpecialLwStart_Enter")]
    pub fn ness_special_lw_start_enter(&mut self, gobj: GObjId) {
        self.ness_special_lw_start(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_LW_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftNs_SpecialAirLwStart_Enter`: no rise, and the sideways speed cut.
    #[doc(alias = "ftNs_SpecialAirLwStart_Enter")]
    pub fn ness_special_air_lw_start_enter(&mut self, gobj: GObjId) {
        self.ness_special_lw_start(gobj);
        let f = self.game.fighter_id(gobj);
        let keep = reg(self.ness_attr_f32(f, attr::PSI_MAGNET_MOMENTUM_PRESERVATION));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), keep));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// Letting go of B marks the release.
    fn ness_special_lw_check_release(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::LW_IS_RELEASE, 1);
        }
    }

    /// The bubble's effect on the hand, once (`x2219_b0`), and the effects paused in hitlag.
    fn ness_special_lw_bubble_gfx(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.game.fighters[f.0].x2219_b0 {
            let j = self.bone_joint(gobj, PART_L1ST_NB);
            let mut head = self.game.fighters[f.0].x60C;
            ef::queue(
                self,
                Some(gobj),
                &mut head,
                SpawnKind::Attach,
                GFX_BUBBLE,
                j,
                &[],
            );
            let fp = &mut self.game.fighters[f.0];
            fp.x60C = head;
            fp.x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The start's end (`ftNs_SpecialLwStart_Anim`, `ftNs_SpecialAirLwStart_Anim`): the
    /// bubble's effect and holding.
    fn ness_special_lw_start_anim(&mut self, gobj: GObjId) {
        self.ness_special_lw_check_release(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.ness_special_lw_bubble_gfx(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::LW_X10, 0);
        if fp.ground_or_air == GA_GROUND {
            self.ness_special_lw_hold_enter(gobj);
        } else {
            self.ness_special_air_lw_hold_enter(gobj);
        }
    }

    /// The air's physics: after the delay, PSI Magnet's slow fall, and the quick air
    /// friction.
    fn ness_special_air_lw_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ness_attr_f32(f, attr::PSI_MAGNET_FALL_ACCEL));
        let fp = &mut self.game.fighters[f.0];
        let d = fp.mv_i32(mv::LW_GRAVITY_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::LW_GRAVITY_DELAY, d.wrapping_sub(1));
        } else {
            let tv = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, tv);
        }
        self.self_accel_deaccel_quick_air(gobj);
    }

    /// The bubble, from the attributes' `AbsorbDesc`.
    fn ness_psi_magnet_bubble(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.ness_attr_f32(f, attr::PSI_MAGNET_ABSORPTION + o);
        let bone = self.ness_attr_i32(f, attr::PSI_MAGNET_ABSORPTION);
        let offset = Vec3::new(a(4), a(8), a(0xC));
        let size = a(0x10);
        self.create_absorb_hit(gobj, bone, offset, size);
    }

    /// `ftNs_SpecialLwStart_GroundToAir`.
    #[doc(alias = "ftNs_SpecialLwStart_GroundToAir")]
    pub fn ness_special_lw_start_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_START, COLL_FLAGS);
    }

    /// Landing, then the air drift clamped.
    fn ness_special_lw_land(&mut self, gobj: GObjId, msid: i32, flags: u32) {
        self.fox_air_to_ground(gobj, msid, flags);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftNs_SpecialAirLwStart_AirToGround`.
    #[doc(alias = "ftNs_SpecialAirLwStart_AirToGround")]
    pub fn ness_special_air_lw_start_to_ground(&mut self, gobj: GObjId) {
        self.ness_special_lw_land(gobj, ms::SPECIAL_LW_START, COLL_FLAGS);
    }

    /// The release lag and the release: the end once both are due.
    fn ness_special_lw_maybe_end(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::LW_RELEASE_LAG) <= 0 && fp.mv_i32(mv::LW_IS_RELEASE) != 0 {
            if fp.ground_or_air == GA_GROUND {
                self.ness_special_lw_end_enter(gobj);
            } else {
                self.ness_special_air_lw_end_enter(gobj);
            }
        }
    }

    /// The release lag counting down.
    fn ness_special_lw_count_lag(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let l = fp.mv_i32(mv::LW_RELEASE_LAG);
        if l > 0 {
            fp.set_mv_i32(mv::LW_RELEASE_LAG, l - 1);
        }
    }

    /// The bubble's hum every 40 frames (`MagnetStateVarCalc`).
    fn ness_special_lw_hum(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::LW_X10).wrapping_sub(1);
        fp.set_mv_i32(mv::LW_X10, n);
        if n <= 0 {
            self.fighter_play_track(f, Track::Effect, SFX_HUM, 127, 64);
            self.game.fighters[f.0].set_mv_i32(mv::LW_X10, 40);
        }
    }

    /// Holding (`ftNs_SpecialLwHold_Anim`, `ftNs_SpecialAirLwHold_Anim`): the end once B is let
    /// go of and the lag is out, and the hum.
    fn ness_special_lw_hold_anim(&mut self, gobj: GObjId) {
        self.ness_special_lw_check_release(gobj);
        self.ness_special_lw_count_lag(gobj);
        self.ness_special_lw_maybe_end(gobj);
        self.ness_special_lw_hum(gobj);
    }

    /// `ftNs_SpecialLwHold_GroundToAir`.
    #[doc(alias = "ftNs_SpecialLwHold_GroundToAir")]
    pub fn ness_special_lw_hold_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_HOLD, COLL_FLAGS);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// `ftNs_SpecialAirLwHold_AirToGround`.
    #[doc(alias = "ftNs_SpecialAirLwHold_AirToGround")]
    pub fn ness_special_air_lw_hold_to_ground(&mut self, gobj: GObjId) {
        self.ness_special_lw_land(gobj, ms::SPECIAL_LW_HOLD, COLL_FLAGS);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// `ftNs_SpecialLwHold_Enter`.
    #[doc(alias = "ftNs_SpecialLwHold_Enter")]
    pub fn ness_special_lw_hold_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_LW_HOLD, KEEP_GFX, 0.0, 1.0, 0.0, None);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// `ftNs_SpecialAirLwHold_Enter`.
    #[doc(alias = "ftNs_SpecialAirLwHold_Enter")]
    pub fn ness_special_air_lw_hold_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_HOLD, KEEP_GFX, 0.0, 1.0, 0.0, None);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// The turnaround (`ftNs_SpecialLwTurn_Anim`, `ftNs_SpecialAirLwTurn_Anim`, the
    /// `getAttrStuff` inline): turned halfway through, the model spinning, then holding again.
    fn ness_special_lw_turn_anim(&mut self, gobj: GObjId) {
        self.ness_special_lw_check_release(gobj);
        self.ness_special_lw_count_lag(gobj);
        let f = self.game.fighter_id(gobj);
        let unk1 = reg(self.ness_attr_f32(f, attr::PSI_MAGNET_UNK1));
        let fp = &mut self.game.fighters[f.0];
        let t = fp.mv_i32(mv::LW_TURN_FRAMES).wrapping_add(-1);
        fp.set_mv_i32(mv::LW_TURN_FRAMES, t);
        if fp.cmd_vars[0] == 0 && fp::frsp(f64::from(t)) <= unk1 {
            fp.cmd_vars[0] = 1;
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        let z = reg(self.game.part_rot_y(f, 0));
        let rot = fp::fnmsubs(f64::from(0.017_453_292_f32), fp::fdivs(180.0, unk1), z);
        self.game.part_set_rot_y(f, 0, mem(rot));
        if self.game.fighters[f.0].mv_i32(mv::LW_TURN_FRAMES) <= 0 {
            self.ness_special_lw_hold_ground_or_air(gobj);
        }
    }

    /// `ftNs_SpecialLwTurn_GroundToAir`.
    #[doc(alias = "ftNs_SpecialLwTurn_GroundToAir")]
    pub fn ness_special_lw_turn_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_TURN, COLL_FLAGS);
    }

    /// `ftNs_SpecialAirLwTurn_AirToGround`.
    #[doc(alias = "ftNs_SpecialAirLwTurn_AirToGround")]
    pub fn ness_special_air_lw_turn_to_ground(&mut self, gobj: GObjId) {
        self.ness_special_lw_land(gobj, ms::SPECIAL_LW_TURN, COLL_FLAGS);
    }

    /// Holding again, or the end, `ftNs_SpecialLwHold_GroundOrAir`: whether he holds on.
    #[doc(alias = "ftNs_SpecialLwHold_GroundOrAir")]
    pub fn ness_special_lw_hold_ground_or_air(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let ground = fp.ground_or_air == GA_GROUND;
        if fp.mv_i32(mv::LW_RELEASE_LAG) <= 0 && fp.mv_i32(mv::LW_IS_RELEASE) != 0 {
            if ground {
                self.ness_special_lw_end_enter(gobj);
            } else {
                self.ness_special_air_lw_end_enter(gobj);
            }
            return false;
        }
        if ground {
            self.ness_special_lw_hold_enter(gobj);
        } else {
            self.ness_special_air_lw_hold_enter(gobj);
        }
        true
    }

    /// After a hold again: the effects cleared, and the bubble's effect back.
    fn ness_special_lw_rehold(&mut self, gobj: GObjId) {
        self.destroy_effects(gobj);
        self.ness_special_lw_bubble_gfx(gobj);
    }

    /// `ftNs_SpecialLwHit_Anim`: at the absorption's end, holding again (or the end).
    #[doc(alias = "ftNs_SpecialLwHit_Anim")]
    pub fn ness_special_lw_hit_anim(&mut self, gobj: GObjId) {
        self.ness_special_lw_check_release(gobj);
        self.ness_special_lw_count_lag(gobj);
        if !self.anim_frames_remaining(gobj) && self.ness_special_lw_hold_ground_or_air(gobj) {
            self.ness_special_lw_rehold(gobj);
        }
        self.ness_special_lw_hum(gobj);
    }

    /// `ftNs_SpecialAirLwHit_Anim`.
    #[doc(alias = "ftNs_SpecialAirLwHit_Anim")]
    pub fn ness_special_air_lw_hit_anim(&mut self, gobj: GObjId) {
        self.ness_special_lw_hit_anim(gobj);
    }

    /// `ftNs_SpecialLwHit_GroundToAir`.
    #[doc(alias = "ftNs_SpecialLwHit_GroundToAir")]
    pub fn ness_special_lw_hit_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_HIT, COLL_FLAGS);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// `ftNs_SpecialAirLwHit_AirToGround`.
    #[doc(alias = "ftNs_SpecialAirLwHit_AirToGround")]
    pub fn ness_special_air_lw_hit_to_ground(&mut self, gobj: GObjId) {
        self.ness_special_lw_land(gobj, ms::SPECIAL_LW_HIT, COLL_FLAGS);
        self.ness_psi_magnet_bubble(gobj);
    }

    /// Absorbing, `ftNs_AbsorbThink_DecideAction` (`ftNs_Init_OnAbsorb`): healed by the damage
    /// taken times the attribute (down to 0%, the healing counted), facing the absorbed
    /// projectile, and the absorption state unless early in one.
    #[doc(alias = "ftNs_AbsorbThink_DecideAction")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn ness_absorb_decide_action(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let heal_mul = reg(self.ness_absorb_heal(gobj));
        let unk2 = self.ness_attr_f32(f, attr::PSI_MAGNET_UNK2);
        let fp = &mut self.game.fighters[f.0];
        let taken = fp::frsp(f64::from(fp.AbsorbAttr.x1A44_damageTaken));
        let mut heal = fp::frsp(f64::from(fp::fctiwz(fp::fmuls(taken, heal_mul))));
        fp.dmg.x1830_percent = mem(fp::fsubs(reg(fp.dmg.x1830_percent), heal));
        let cur = reg(fp.dmg.x1830_percent);
        if cur < 0.0 {
            heal = fp::fadds(cur, heal);
            fp.dmg.x1830_percent = 0.0;
        }
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        let percent = fp::fctiwz(reg(fp.dmg.x1830_percent));
        self.game.set_player_damage_at(slot, sub, percent);
        self.game.add_healed(slot, sub, fp::fctiwz(heal));
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = fp.AbsorbAttr.x1A40_absorbHitDirection;
        let id = fp.motion_id;
        if (id != ms::SPECIAL_LW_HIT && id != ms::SPECIAL_AIR_LW_HIT)
            || !(fp.cur_anim_frame <= unk2)
        {
            let msid = if fp.ground_or_air == GA_GROUND {
                ms::SPECIAL_LW_HIT
            } else {
                ms::SPECIAL_AIR_LW_HIT
            };
            self.change_motion_state(gobj, msid, KEEP_GFX, 0.0, 1.0, 0.0, None);
            self.ness_psi_magnet_bubble(gobj);
        }
    }

    /// The end's end (`ftNs_SpecialLwEnd_Anim`, `ftNs_SpecialAirLwEnd_Anim`): the effects
    /// cleared, then falling or waiting.
    fn ness_special_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.destroy_effects(gobj);
            self.fall_or_wait(gobj);
        }
    }

    /// `ftNs_SpecialLwEnd_GroundToAir`.
    #[doc(alias = "ftNs_SpecialLwEnd_GroundToAir")]
    pub fn ness_special_lw_end_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_END, END_FLAGS);
    }

    /// `ftNs_SpecialAirLwEnd_AirToGround`.
    #[doc(alias = "ftNs_SpecialAirLwEnd_AirToGround")]
    pub fn ness_special_air_lw_end_to_ground(&mut self, gobj: GObjId) {
        self.ness_special_lw_land(gobj, ms::SPECIAL_LW_END, END_FLAGS);
    }

    /// `ftNs_SpecialLwEnd_Enter`.
    #[doc(alias = "ftNs_SpecialLwEnd_Enter")]
    pub fn ness_special_lw_end_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_LW_END, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftNs_SpecialAirLwEnd_Enter`.
    #[doc(alias = "ftNs_SpecialAirLwEnd_Enter")]
    pub fn ness_special_air_lw_end_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_END, 0, 0.0, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_START_ENTER => g.ness_special_lw_start_enter(gobj),
        addr::SPECIAL_AIR_LW_START_ENTER => g.ness_special_air_lw_start_enter(gobj),
        addr::SPECIAL_LW_START_ANIM | addr::SPECIAL_AIR_LW_START_ANIM => {
            g.ness_special_lw_start_anim(gobj)
        }
        addr::SPECIAL_LW_START_IASA
        | addr::SPECIAL_AIR_LW_START_IASA
        | addr::SPECIAL_LW_HOLD_IASA
        | addr::SPECIAL_AIR_LW_HOLD_IASA
        | addr::SPECIAL_LW_TURN_IASA
        | addr::SPECIAL_AIR_LW_TURN_IASA
        | addr::SPECIAL_LW_HIT_IASA
        | addr::SPECIAL_AIR_LW_HIT_IASA
        | addr::SPECIAL_LW_END_IASA
        | addr::SPECIAL_AIR_LW_END_IASA => {}
        addr::SPECIAL_LW_START_PHYS | addr::SPECIAL_LW_END_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_LW_START_PHYS | addr::SPECIAL_AIR_LW_END_PHYS => {
            g.ness_special_air_lw_fall(gobj)
        }
        addr::SPECIAL_LW_START_COLL => {
            if !g.ground_coll_off_ledges(gobj) {
                g.ness_special_lw_start_to_air(gobj);
            }
        }
        addr::SPECIAL_AIR_LW_START_COLL => {
            if g.air_coll_land(gobj) {
                g.ness_special_air_lw_start_to_ground(gobj);
            }
        }
        addr::SPECIAL_LW_START_GROUND_TO_AIR => g.ness_special_lw_start_to_air(gobj),
        addr::SPECIAL_AIR_LW_START_AIR_TO_GROUND => g.ness_special_air_lw_start_to_ground(gobj),
        addr::SPECIAL_LW_HOLD_ANIM | addr::SPECIAL_AIR_LW_HOLD_ANIM => {
            g.ness_special_lw_hold_anim(gobj)
        }
        addr::SPECIAL_LW_HOLD_PHYS | addr::SPECIAL_LW_TURN_PHYS | addr::SPECIAL_LW_HIT_PHYS => {
            g.ground_friction(gobj);
            g.absorb_follow(gobj);
        }
        addr::SPECIAL_AIR_LW_HOLD_PHYS
        | addr::SPECIAL_AIR_LW_TURN_PHYS
        | addr::SPECIAL_AIR_LW_HIT_PHYS => {
            g.ness_special_air_lw_fall(gobj);
            g.absorb_follow(gobj);
        }
        addr::SPECIAL_LW_HOLD_COLL => {
            if !g.ground_coll_off_ledges(gobj) {
                g.ness_special_lw_hold_to_air(gobj);
            }
        }
        addr::SPECIAL_AIR_LW_HOLD_COLL => {
            if g.air_coll_land(gobj) {
                g.ness_special_air_lw_hold_to_ground(gobj);
            }
        }
        addr::SPECIAL_LW_HOLD_GROUND_TO_AIR => g.ness_special_lw_hold_to_air(gobj),
        addr::SPECIAL_AIR_LW_HOLD_AIR_TO_GROUND => g.ness_special_air_lw_hold_to_ground(gobj),
        addr::SPECIAL_LW_HOLD_ENTER => g.ness_special_lw_hold_enter(gobj),
        addr::SPECIAL_AIR_LW_HOLD_ENTER => g.ness_special_air_lw_hold_enter(gobj),
        addr::SPECIAL_LW_TURN_ANIM | addr::SPECIAL_AIR_LW_TURN_ANIM => {
            g.ness_special_lw_turn_anim(gobj)
        }
        addr::SPECIAL_LW_TURN_COLL => {
            if !g.ground_coll_off_ledges(gobj) {
                g.ness_special_lw_turn_to_air(gobj);
            }
        }
        addr::SPECIAL_AIR_LW_TURN_COLL => {
            if g.air_coll_land(gobj) {
                g.ness_special_air_lw_turn_to_ground(gobj);
            }
        }
        addr::SPECIAL_LW_TURN_GROUND_TO_AIR => g.ness_special_lw_turn_to_air(gobj),
        addr::SPECIAL_AIR_LW_TURN_AIR_TO_GROUND => g.ness_special_air_lw_turn_to_ground(gobj),
        addr::SPECIAL_LW_HIT_ANIM => g.ness_special_lw_hit_anim(gobj),
        addr::SPECIAL_AIR_LW_HIT_ANIM => g.ness_special_air_lw_hit_anim(gobj),
        addr::SPECIAL_LW_HIT_COLL => {
            if !g.ground_coll_off_ledges(gobj) {
                g.ness_special_lw_hit_to_air(gobj);
            }
        }
        addr::SPECIAL_AIR_LW_HIT_COLL => {
            if g.air_coll_land(gobj) {
                g.ness_special_air_lw_hit_to_ground(gobj);
            }
        }
        addr::SPECIAL_LW_HIT_GROUND_TO_AIR => g.ness_special_lw_hit_to_air(gobj),
        addr::SPECIAL_AIR_LW_HIT_AIR_TO_GROUND => g.ness_special_air_lw_hit_to_ground(gobj),
        addr::ABSORB_THINK_DECIDE_ACTION => g.ness_absorb_decide_action(gobj),
        addr::SPECIAL_LW_END_ANIM | addr::SPECIAL_AIR_LW_END_ANIM => {
            g.ness_special_lw_end_anim(gobj)
        }
        addr::SPECIAL_LW_END_COLL => {
            if !g.ground_coll_off_ledges(gobj) {
                g.ness_special_lw_end_to_air(gobj);
            }
        }
        addr::SPECIAL_AIR_LW_END_COLL => {
            if g.air_coll_land(gobj) {
                g.ness_special_air_lw_end_to_ground(gobj);
            }
        }
        addr::SPECIAL_LW_END_GROUND_TO_AIR => g.ness_special_lw_end_to_air(gobj),
        addr::SPECIAL_AIR_LW_END_AIR_TO_GROUND => g.ness_special_air_lw_end_to_ground(gobj),
        addr::SPECIAL_LW_END_ENTER => g.ness_special_lw_end_enter(gobj),
        addr::SPECIAL_AIR_LW_END_ENTER => g.ness_special_air_lw_end_enter(gobj),
        _ => return false,
    }
    true
}
