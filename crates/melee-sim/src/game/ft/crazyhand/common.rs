//! The callbacks his states share: a human's input, the air motion, the glide toward his
//! target, the empty ones, and the return once an animation ends.

use super::super::super::state::Running;
use super::super::masterhand::wait::mv_vec;
use super::{addr, attr, mv};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// A human-played hand has nothing to do (`ftBossLib_8015BD20`), a computer's nothing
    /// either: the states' input callbacks.
    #[doc(alias = "ftCh_Wait1_0_IASA")]
    #[doc(alias = "ftCh_Entry_IASA")]
    #[doc(alias = "ftCh_Damage_IASA")]
    #[doc(alias = "ftCh_Damage2_IASA")]
    #[doc(alias = "ftCh_WaitSweep_IASA")]
    #[doc(alias = "ftCh_SweepLoop_IASA")]
    #[doc(alias = "ftCh_SweepWait_IASA")]
    #[doc(alias = "ftCh_Slap_IASA")]
    #[doc(alias = "ftCh_Walk2_IASA")]
    #[doc(alias = "ftCh_WalkLoop_IASA")]
    #[doc(alias = "ftCh_WalkWait_IASA")]
    #[doc(alias = "ftCh_WalkShoot_IASA")]
    #[doc(alias = "ftCh_Drill_IASA")]
    #[doc(alias = "ftCh_RockCrushUp_IASA")]
    #[doc(alias = "ftCh_RockCrushDown_IASA")]
    #[doc(alias = "ftCh_PaperCrush_IASA")]
    #[doc(alias = "ftCh_Poke1_IASA")]
    #[doc(alias = "ftCh_Poke2_IASA")]
    #[doc(alias = "ftCh_FingerBeamStart_IASA")]
    #[doc(alias = "ftCh_FingerBeamLoop_IASA")]
    #[doc(alias = "ftCh_FingerBeamEnd_IASA")]
    #[doc(alias = "ftCh_BackPunch_IASA")]
    // Ports `ftCh_FingerGun1_IASA`; aliased once a check reaches it.
    #[doc(alias = "ftCh_BackAirplane1_IASA")]
    #[doc(alias = "ftCh_BackAirplane2_IASA")]
    #[doc(alias = "ftCh_BackAirplane3_IASA")]
    #[doc(alias = "ftCh_BackCrush_IASA")]
    #[doc(alias = "ftCh_BackDisappear_IASA")]
    #[doc(alias = "ftCh_Wait1_1_IASA")]
    #[doc(alias = "ftCh_Grab_IASA")]
    #[doc(alias = "ftCh_Cancel_IASA")]
    #[doc(alias = "ftCh_Squeezing0_IASA")]
    #[doc(alias = "ftCh_Squeezing1_IASA")]
    #[doc(alias = "ftCh_Squeeze_IASA")]
    #[doc(alias = "ftCh_Throw_IASA")]
    #[doc(alias = "ftCh_TagRockPaper_IASA")]
    #[doc(alias = "ftCh_FingerGun2_IASA")]
    #[doc(alias = "ftCh_TagGrab_IASA")]
    #[doc(alias = "ftCh_TagFail_IASA")]
    pub fn ch_human_nop(&mut self, gobj: GObjId) {
        self.mh_human_nop(gobj);
    }

    /// The animation's motion in the air (`ft_80085134`): the states' physics.
    #[doc(alias = "ftCh_Wait1_0_Phys")]
    #[doc(alias = "ftCh_Entry_Phys")]
    #[doc(alias = "ftCh_Damage_Phys")]
    #[doc(alias = "ftCh_WaitSweep_Phys")]
    #[doc(alias = "ftCh_SweepLoop_Phys")]
    #[doc(alias = "ftCh_SweepWait_Phys")]
    #[doc(alias = "ftCh_Walk2_Phys")]
    #[doc(alias = "ftCh_WalkLoop_Phys")]
    #[doc(alias = "ftCh_WalkShoot_Phys")]
    #[doc(alias = "ftCh_PaperCrush_Phys")]
    #[doc(alias = "ftCh_Poke1_Phys")]
    #[doc(alias = "ftCh_Poke2_Phys")]
    #[doc(alias = "ftCh_FingerBeamStart_Phys")]
    #[doc(alias = "ftCh_FingerBeamEnd_Phys")]
    #[doc(alias = "ftCh_BackAirplane2_Phys")]
    #[doc(alias = "ftCh_BackAirplane3_Phys")]
    #[doc(alias = "ftCh_BackCrush_Phys")]
    #[doc(alias = "ftCh_Grab_Phys")]
    #[doc(alias = "ftCh_Squeezing0_Phys")]
    #[doc(alias = "ftCh_Squeezing1_Phys")]
    #[doc(alias = "ftCh_Squeeze_Phys")]
    #[doc(alias = "ftCh_Throw_Phys")]
    pub fn ch_air_motion(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// The air motion and the glide toward `mv.xC` (`ftCh_UpdateBossMotion`: `ft_80085134` and
    /// `ftBossLib_8015BE40` with `x14` and `x10`).
    #[doc(alias = "ftCh_BackPunch_Phys")]
    // Ports `ftCh_FingerGun1_Phys`; aliased once a check reaches it.
    #[doc(alias = "ftCh_BackAirplane1_Phys")]
    #[doc(alias = "ftCh_FingerGun2_Phys")]
    #[doc(alias = "ftCh_TagCancel_Phys")]
    #[doc(alias = "ftCh_Damage2_Phys")]
    pub fn ch_update_motion(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.ch_glide_step(gobj);
    }

    /// `ftBossLib_8015BE40` toward `mv.xC` at `x10` of the distance a frame, none within
    /// `x14`; the distance left into `mv.x18`.
    pub(crate) fn ch_glide_step(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let target = mv_vec(&self.game.fighters[f.0], mv::TARGET);
        let (near, rate) = (self.ch_da_f32(f, attr::X14), self.ch_da_f32(f, attr::X10));
        let d = self.boss_glide(gobj, target, near, rate);
        self.game.fighters[f.0].set_mv_f32(mv::DISTANCE, reg(d));
    }

    /// Stopped where the glide ends: `ftCh_FingerGun1_Coll` and `ftCh_BackAirplane1_Coll`.
    // Ports `ftCh_FingerGun1_Coll`; aliased once a check reaches it.
    #[doc(alias = "ftCh_BackAirplane1_Coll")]
    pub fn ch_stop_at_target(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        }
    }

    /// Nothing: the states' empty callbacks.
    #[doc(alias = "ftCh_Wait1_0_Coll")]
    #[doc(alias = "ftCh_Entry_Coll")]
    #[doc(alias = "ftCh_Damage_Coll")]
    #[doc(alias = "ftCh_Damage2_Coll")]
    #[doc(alias = "ftCh_WaitSweep_Coll")]
    #[doc(alias = "ftCh_SweepLoop_Coll")]
    #[doc(alias = "ftCh_SweepWait_Coll")]
    #[doc(alias = "ftCh_Slap_Coll")]
    #[doc(alias = "ftCh_WalkLoop_Coll")]
    #[doc(alias = "ftCh_WalkWait_Coll")]
    #[doc(alias = "ftCh_WalkShoot_Coll")]
    #[doc(alias = "ftCh_Drill_Coll")]
    #[doc(alias = "ftCh_RockCrushUp_Coll")]
    #[doc(alias = "ftCh_RockCrushDown_Coll")]
    #[doc(alias = "ftCh_PaperCrush_Coll")]
    #[doc(alias = "ftCh_Poke1_Coll")]
    #[doc(alias = "ftCh_Poke2_Coll")]
    #[doc(alias = "ftCh_FingerBeamStart_Coll")]
    #[doc(alias = "ftCh_FingerBeamLoop_Coll")]
    #[doc(alias = "ftCh_FingerBeamEnd_Coll")]
    #[doc(alias = "ftCh_BackPunch_Coll")]
    #[doc(alias = "ftCh_BackAirplane2_Coll")]
    #[doc(alias = "ftCh_BackAirplane3_Coll")]
    #[doc(alias = "ftCh_BackCrush_Coll")]
    #[doc(alias = "ftCh_BackDisappear_Coll")]
    #[doc(alias = "ftCh_Wait1_1_Coll")]
    #[doc(alias = "ftCh_Grab_Coll")]
    #[doc(alias = "ftCh_Cancel_Coll")]
    #[doc(alias = "ftCh_Squeezing0_Coll")]
    #[doc(alias = "ftCh_Squeezing1_Coll")]
    #[doc(alias = "ftCh_Squeeze_Coll")]
    #[doc(alias = "ftCh_Throw_Coll")]
    #[doc(alias = "ftCh_TagRockPaper_Phys")]
    #[doc(alias = "ftCh_TagRockPaper_Coll")]
    #[doc(alias = "ftCh_FingerGun2_Coll")]
    #[doc(alias = "ftCh_TagGrab_Coll")]
    #[doc(alias = "ftCh_TagFail_Phys")]
    #[doc(alias = "ftCh_TagFail_Coll")]
    #[doc(alias = "ftCo_CaptureCrazyHand_Anim")]
    #[doc(alias = "ftCo_CaptureCrazyHand_IASA")]
    #[doc(alias = "ftCo_CaptureCrazyHand_Phys")]
    #[doc(alias = "ftCo_CaptureCrazyHand_Coll")]
    #[doc(alias = "ftCo_CaptureDamageCrazyHand_IASA")]
    #[doc(alias = "ftCo_CaptureDamageCrazyHand_Phys")]
    #[doc(alias = "ftCo_CaptureDamageCrazyHand_Coll")]
    // Ports `ftCo_CaptureWaitCrazyHand_Anim`; aliased once a check reaches it.
    // Ports `ftCo_CaptureWaitCrazyHand_IASA`; aliased once a check reaches it.
    // Ports `ftCo_CaptureWaitCrazyHand_Phys`; aliased once a check reaches it.
    // Ports `ftCo_CaptureWaitCrazyHand_Coll`; aliased once a check reaches it.
    // Ports `ftCo_ThrownCrazyHand_Anim`; aliased once a check reaches it.
    // Ports `ftCo_ThrownCrazyHand_IASA`; aliased once a check reaches it.
    // Ports `ftCo_ThrownCrazyHand_Phys`; aliased once a check reaches it.
    // Ports `ftCo_ThrownCrazyHand_Coll`; aliased once a check reaches it.
    pub fn ch_nothing(&mut self, _gobj: GObjId) {}

    /// Back to his place and the wait once the animation ends (`ftCh_GrabUnk1_8015BC88`).
    #[doc(alias = "ftCh_Entry_Anim")]
    #[doc(alias = "ftCh_WaitSweep_Anim")]
    #[doc(alias = "ftCh_SweepLoop_Anim")]
    #[doc(alias = "ftCh_Walk2_Anim")]
    #[doc(alias = "ftCh_WalkShoot_Anim")]
    #[doc(alias = "ftCh_Drill_Anim")]
    #[doc(alias = "ftCh_RockCrushWait_Anim")]
    #[doc(alias = "ftCh_RockCrushDown_Anim")]
    #[doc(alias = "ftCh_Poke2_Anim")]
    #[doc(alias = "ftCh_FingerBeamStart_Anim")]
    #[doc(alias = "ftCh_BackPunch_Anim")]
    #[doc(alias = "ftCh_Squeezing0_Anim")]
    pub fn ch_anim_return(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_return(gobj);
        }
    }

    /// Enters one of his states from its first frame (`Fighter_ChangeMotionState` and
    /// `ftAnim_8006EBA4`).
    pub(crate) fn ch_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        WAIT1_0_IASA
        | ENTRY_IASA
        | DAMAGE_IASA
        | DAMAGE2_IASA
        | WAIT_SWEEP_IASA
        | SWEEP_LOOP_IASA
        | SWEEP_WAIT_IASA
        | SLAP_IASA
        | WALK2_IASA
        | WALK_LOOP_IASA
        | WALK_WAIT_IASA
        | WALK_SHOOT_IASA
        | DRILL_IASA
        | ROCK_CRUSH_UP_IASA
        | ROCK_CRUSH_DOWN_IASA
        | PAPER_CRUSH_IASA
        | POKE1_IASA
        | POKE2_IASA
        | FINGER_BEAM_START_IASA
        | FINGER_BEAM_LOOP_IASA
        | FINGER_BEAM_END_IASA
        | BACK_PUNCH_IASA
        | FINGER_GUN1_IASA
        | BACK_AIRPLANE1_IASA
        | BACK_AIRPLANE2_IASA
        | BACK_AIRPLANE3_IASA
        | BACK_CRUSH_IASA
        | BACK_DISAPPEAR_IASA
        | WAIT1_1_IASA
        | GRAB_IASA
        | CANCEL_IASA
        | SQUEEZING0_IASA
        | SQUEEZING1_IASA
        | SQUEEZE_IASA
        | THROW_IASA
        | TAG_ROCK_PAPER_IASA
        | FINGER_GUN2_IASA
        | TAG_GRAB_IASA
        | TAG_FAIL_IASA => g.ch_human_nop(gobj),
        WAIT1_0_PHYS
        | ENTRY_PHYS
        | DAMAGE_PHYS
        | WAIT_SWEEP_PHYS
        | SWEEP_LOOP_PHYS
        | SWEEP_WAIT_PHYS
        | WALK2_PHYS
        | WALK_LOOP_PHYS
        | WALK_SHOOT_PHYS
        | PAPER_CRUSH_PHYS
        | POKE1_PHYS
        | POKE2_PHYS
        | FINGER_BEAM_START_PHYS
        | FINGER_BEAM_END_PHYS
        | BACK_AIRPLANE2_PHYS
        | BACK_AIRPLANE3_PHYS
        | BACK_CRUSH_PHYS
        | GRAB_PHYS
        | SQUEEZING0_PHYS
        | SQUEEZING1_PHYS
        | SQUEEZE_PHYS
        | THROW_PHYS => g.ch_air_motion(gobj),
        BACK_PUNCH_PHYS | FINGER_GUN1_PHYS | BACK_AIRPLANE1_PHYS | FINGER_GUN2_PHYS
        | TAG_CANCEL_PHYS => g.ch_update_motion(gobj),
        FINGER_GUN1_COLL | BACK_AIRPLANE1_COLL => g.ch_stop_at_target(gobj),
        WAIT1_0_COLL
        | ENTRY_COLL
        | DAMAGE_COLL
        | DAMAGE2_COLL
        | WAIT_SWEEP_COLL
        | SWEEP_LOOP_COLL
        | SWEEP_WAIT_COLL
        | SLAP_COLL
        | WALK_LOOP_COLL
        | WALK_WAIT_COLL
        | WALK_SHOOT_COLL
        | DRILL_COLL
        | ROCK_CRUSH_UP_COLL
        | ROCK_CRUSH_DOWN_COLL
        | PAPER_CRUSH_COLL
        | POKE1_COLL
        | POKE2_COLL
        | FINGER_BEAM_START_COLL
        | FINGER_BEAM_LOOP_COLL
        | FINGER_BEAM_END_COLL
        | BACK_PUNCH_COLL
        | BACK_AIRPLANE2_COLL
        | BACK_AIRPLANE3_COLL
        | BACK_CRUSH_COLL
        | BACK_DISAPPEAR_COLL
        | WAIT1_1_COLL
        | GRAB_COLL
        | CANCEL_COLL
        | SQUEEZING0_COLL
        | SQUEEZING1_COLL
        | SQUEEZE_COLL
        | THROW_COLL
        | TAG_ROCK_PAPER_PHYS
        | TAG_ROCK_PAPER_COLL
        | FINGER_GUN2_COLL
        | TAG_GRAB_COLL
        | TAG_FAIL_PHYS
        | TAG_FAIL_COLL
        | CAPTURE_CRAZY_HAND_ANIM
        | CAPTURE_CRAZY_HAND_IASA
        | CAPTURE_CRAZY_HAND_PHYS
        | CAPTURE_CRAZY_HAND_COLL
        | CAPTURE_DAMAGE_CRAZY_HAND_IASA
        | CAPTURE_DAMAGE_CRAZY_HAND_PHYS
        | CAPTURE_DAMAGE_CRAZY_HAND_COLL
        | CAPTURE_WAIT_CRAZY_HAND_ANIM
        | CAPTURE_WAIT_CRAZY_HAND_IASA
        | CAPTURE_WAIT_CRAZY_HAND_PHYS
        | CAPTURE_WAIT_CRAZY_HAND_COLL
        | THROWN_CRAZY_HAND_ANIM
        | THROWN_CRAZY_HAND_IASA
        | THROWN_CRAZY_HAND_PHYS
        | THROWN_CRAZY_HAND_COLL => g.ch_nothing(gobj),
        ENTRY_ANIM
        | WAIT_SWEEP_ANIM
        | SWEEP_LOOP_ANIM
        | WALK2_ANIM
        | WALK_SHOOT_ANIM
        | DRILL_ANIM
        | ROCK_CRUSH_WAIT_ANIM
        | ROCK_CRUSH_DOWN_ANIM
        | POKE2_ANIM
        | FINGER_BEAM_START_ANIM
        | BACK_PUNCH_ANIM
        | SQUEEZING0_ANIM => g.ch_anim_return(gobj),
        _ => return false,
    }
    true
}
