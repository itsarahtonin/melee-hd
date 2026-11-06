//! The callbacks his states share: empty ones, a human's input, the air motion, and the
//! return once an animation ends.

use super::super::super::state::Running;
use super::addr;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// A human-played hand has nothing to do (`ftBossLib_8015BD20`), a computer's nothing either: the states' input callbacks.
    #[doc(alias = "ftMh_Poke1_IASA")]
    #[doc(alias = "ftMh_BackDisappear_IASA")]
    #[doc(alias = "ftMh_Grab_IASA")]
    #[doc(alias = "ftMh_BackAirplane1_IASA")]
    #[doc(alias = "ftMh_BackAirplane2_IASA")]
    #[doc(alias = "ftMh_BackAirplane3_IASA")]
    #[doc(alias = "ftMh_BackPunch_IASA")]
    #[doc(alias = "ftMh_BackCrush_IASA")]
    #[doc(alias = "ftMh_Wait1_1_IASA")]
    #[doc(alias = "ftMh_Fail_IASA")]
    // Ports `ftMh_Cancel_IASA`; aliased once a check reaches it.
    #[doc(alias = "ftMh_WalkWait_IASA")]
    #[doc(alias = "ftMh_Drill_IASA")]
    #[doc(alias = "ftMh_Entry_IASA")]
    #[doc(alias = "ftMh_FingerBeamStart_IASA")]
    #[doc(alias = "ftMh_FingerBeamLoop_IASA")]
    #[doc(alias = "ftMh_FingerBeamEnd_IASA")]
    #[doc(alias = "ftMh_FingerGun1_IASA")]
    #[doc(alias = "ftMh_FingerGun2_IASA")]
    #[doc(alias = "ftMh_FingerGun3_IASA")]
    #[doc(alias = "ftMh_RockCrushDown_IASA")]
    #[doc(alias = "ftMh_PaperCrush_IASA")]
    #[doc(alias = "ftMh_RockCrushUp_IASA")]
    #[doc(alias = "ftMh_RockCrushWait_IASA")]
    #[doc(alias = "ftMh_Slam_IASA")]
    #[doc(alias = "ftMh_Slap_IASA")]
    #[doc(alias = "ftMh_Squeeze_IASA")]
    #[doc(alias = "ftMh_Squeezing_IASA")]
    #[doc(alias = "ftMh_Damage_IASA")]
    #[doc(alias = "ftMh_WaitSweep_IASA")]
    #[doc(alias = "ftMh_SweepLoop_IASA")]
    #[doc(alias = "ftMh_SweepWait_IASA")]
    #[doc(alias = "ftMh_TagCrush_IASA")]
    #[doc(alias = "ftMh_TagApplaud_IASA")]
    #[doc(alias = "ftMh_TagRockPaper_IASA")]
    #[doc(alias = "ftMh_Throw_IASA")]
    #[doc(alias = "ftMh_Wait1_0_IASA")]
    #[doc(alias = "ftMh_WalkShoot_IASA")]
    #[doc(alias = "ftMh_Walk2_IASA")]
    #[doc(alias = "ftMh_WalkLoop_IASA")]
    pub fn mh_human_nop(&mut self, gobj: GObjId) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        if self.game.player_pkind(slot) == 0 {
            self.boss_nop(gobj);
        }
    }

    /// The animation's motion in the air (`ft_80085134`): the states' physics.
    #[doc(alias = "ftMh_BackAirplane1_Phys")]
    #[doc(alias = "ftMh_BackAirplane3_Phys")]
    #[doc(alias = "ftMh_BackDisappear_Phys")]
    #[doc(alias = "ftMh_Grab_Phys")]
    #[doc(alias = "ftMh_WalkWait_Phys")]
    #[doc(alias = "ftMh_FingerBeamStart_Phys")]
    #[doc(alias = "ftMh_FingerBeamLoop_Phys")]
    #[doc(alias = "ftMh_FingerBeamEnd_Phys")]
    #[doc(alias = "ftMh_FingerGun2_Phys")]
    #[doc(alias = "ftMh_FingerGun3_Phys")]
    #[doc(alias = "ftMh_RockCrushDown_Phys")]
    #[doc(alias = "ftMh_RockCrushUp_Phys")]
    #[doc(alias = "ftMh_Slam_Phys")]
    #[doc(alias = "ftMh_Slap_Phys")]
    #[doc(alias = "ftMh_WaitSweep_Phys")]
    #[doc(alias = "ftMh_SweepWait_Phys")]
    #[doc(alias = "ftMh_TagApplaud_Phys")]
    #[doc(alias = "ftMh_TagCrush_Phys")]
    #[doc(alias = "ftMh_TagRockPaper_Phys")]
    #[doc(alias = "ftMh_Throw_Phys")]
    #[doc(alias = "ftMh_Wait1_0_Phys")]
    #[doc(alias = "ftMh_WalkShoot_Phys")]
    #[doc(alias = "ftMh_Walk2_Phys")]
    pub fn mh_air_motion(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// Nothing: the states' empty callbacks.
    #[doc(alias = "ftMh_BackAirplane1_Coll")]
    #[doc(alias = "ftMh_BackAirplane2_Coll")]
    #[doc(alias = "ftMh_BackAirplane3_Coll")]
    #[doc(alias = "ftMh_BackPunch_Coll")]
    #[doc(alias = "ftMh_BackCrush_Coll")]
    #[doc(alias = "ftMh_BackDisappear_Coll")]
    #[doc(alias = "ftMh_Wait1_1_Coll")]
    #[doc(alias = "ftMh_Grab_Coll")]
    #[doc(alias = "ftMh_Fail_Coll")]
    #[doc(alias = "ftMh_CaptureDamageMasterHand_IASA")]
    #[doc(alias = "ftMh_CaptureDamageMasterHand_Phys")]
    #[doc(alias = "ftMh_CaptureDamageMasterHand_Coll")]
    #[doc(alias = "ftMh_CaptureMasterHand_Anim")]
    #[doc(alias = "ftMh_CaptureMasterHand_IASA")]
    #[doc(alias = "ftMh_CaptureMasterHand_Phys")]
    #[doc(alias = "ftMh_CaptureMasterHand_Coll")]
    #[doc(alias = "ftMh_WalkWait_Coll")]
    #[doc(alias = "ftMh_Entry_Phys")]
    #[doc(alias = "ftMh_Entry_Coll")]
    #[doc(alias = "ftMh_Poke1_Coll")]
    #[doc(alias = "ftMh_FingerBeamStart_Coll")]
    #[doc(alias = "ftMh_FingerBeamLoop_Coll")]
    #[doc(alias = "ftMh_FingerBeamEnd_Coll")]
    #[doc(alias = "ftMh_FingerGun1_Coll")]
    #[doc(alias = "ftMh_FingerGun2_Coll")]
    #[doc(alias = "ftMh_FingerGun3_Coll")]
    #[doc(alias = "ftMh_RockCrushDown_Coll")]
    #[doc(alias = "ftMh_PaperCrush_Coll")]
    #[doc(alias = "ftMh_Drill_Coll")]
    #[doc(alias = "ftMh_RockCrushUp_Coll")]
    #[doc(alias = "ftMh_RockCrushWait_Coll")]
    #[doc(alias = "ftMh_Slam_Coll")]
    #[doc(alias = "ftMh_Slap_Coll")]
    #[doc(alias = "ftMh_Squeezing_Coll")]
    #[doc(alias = "ftMh_Damage_Coll")]
    #[doc(alias = "ftMh_WaitSweep_Coll")]
    #[doc(alias = "ftMh_SweepLoop_Coll")]
    #[doc(alias = "ftMh_SweepWait_Coll")]
    #[doc(alias = "ftMh_TagApplaud_Coll")]
    #[doc(alias = "ftMh_TagCrush_Coll")]
    #[doc(alias = "ftMh_TagRockPaper_Coll")]
    #[doc(alias = "ftMh_Throw_Coll")]
    // Ports `ftMh_CaptureWaitMasterHand_Anim`; aliased once a check reaches it.
    // Ports `ftMh_CaptureWaitMasterHand_IASA`; aliased once a check reaches it.
    // Ports `ftMh_CaptureWaitMasterHand_Phys`; aliased once a check reaches it.
    // Ports `ftMh_CaptureWaitMasterHand_Coll`; aliased once a check reaches it.
    // Ports `ftMh_ThrownMasterHand_Anim`; aliased once a check reaches it.
    // Ports `ftMh_ThrownMasterHand_IASA`; aliased once a check reaches it.
    // Ports `ftMh_ThrownMasterHand_Phys`; aliased once a check reaches it.
    // Ports `ftMh_ThrownMasterHand_Coll`; aliased once a check reaches it.
    #[doc(alias = "ftMh_Wait1_0_Coll")]
    #[doc(alias = "ftMh_WalkShoot_Coll")]
    #[doc(alias = "ftMh_Walk2_Coll")]
    #[doc(alias = "ftMh_WalkLoop_Coll")]
    pub fn mh_nothing(&mut self, _gobj: GObjId) {}

    /// Back to his place and the wait once the animation ends (`ftMh_MS_389_80151018`).
    #[doc(alias = "ftMh_BackAirplane3_Anim")]
    #[doc(alias = "ftMh_Fail_Anim")]
    #[doc(alias = "ftMh_WalkWait_Anim")]
    #[doc(alias = "ftMh_Drill_Anim")]
    #[doc(alias = "ftMh_FingerBeamEnd_Anim")]
    #[doc(alias = "ftMh_FingerGun3_Anim")]
    #[doc(alias = "ftMh_RockCrushDown_Anim")]
    #[doc(alias = "ftMh_PaperCrush_Anim")]
    #[doc(alias = "ftMh_Slap_Anim")]
    #[doc(alias = "ftMh_SweepWait_Anim")]
    #[doc(alias = "ftMh_WalkShoot_Anim")]
    pub fn mh_anim_return(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_return(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::BACK_AIRPLANE1_IASA
        | addr::BACK_AIRPLANE2_IASA
        | addr::BACK_AIRPLANE3_IASA
        | addr::BACK_PUNCH_IASA
        | addr::BACK_CRUSH_IASA
        | addr::WAIT1_1_IASA
        | addr::FAIL_IASA
        | addr::CANCEL_IASA
        | addr::WALK_WAIT_IASA
        | addr::DRILL_IASA
        | addr::ENTRY_IASA
        | addr::FINGER_BEAM_START_IASA
        | addr::FINGER_BEAM_LOOP_IASA
        | addr::FINGER_BEAM_END_IASA
        | addr::FINGER_GUN1_IASA
        | addr::FINGER_GUN2_IASA
        | addr::FINGER_GUN3_IASA
        | addr::ROCK_CRUSH_DOWN_IASA
        | addr::PAPER_CRUSH_IASA
        | addr::ROCK_CRUSH_UP_IASA
        | addr::ROCK_CRUSH_WAIT_IASA
        | addr::SLAM_IASA
        | addr::SLAP_IASA
        | addr::SQUEEZE_IASA
        | addr::SQUEEZING_IASA
        | addr::DAMAGE_IASA
        | addr::WAIT_SWEEP_IASA
        | addr::SWEEP_LOOP_IASA
        | addr::SWEEP_WAIT_IASA
        | addr::TAG_CRUSH_IASA
        | addr::TAG_APPLAUD_IASA
        | addr::TAG_ROCK_PAPER_IASA
        | addr::THROW_IASA
        | addr::WAIT1_0_IASA
        | addr::WALK_SHOOT_IASA
        | addr::WALK2_IASA
        | addr::WALK_LOOP_IASA => g.mh_human_nop(gobj),
        addr::BACK_AIRPLANE1_PHYS
        | addr::BACK_AIRPLANE3_PHYS
        | addr::BACK_DISAPPEAR_PHYS
        | addr::GRAB_PHYS
        | addr::WALK_WAIT_PHYS
        | addr::FINGER_BEAM_START_PHYS
        | addr::FINGER_BEAM_LOOP_PHYS
        | addr::FINGER_BEAM_END_PHYS
        | addr::FINGER_GUN2_PHYS
        | addr::FINGER_GUN3_PHYS
        | addr::ROCK_CRUSH_DOWN_PHYS
        | addr::ROCK_CRUSH_UP_PHYS
        | addr::SLAM_PHYS
        | addr::SLAP_PHYS
        | addr::WAIT_SWEEP_PHYS
        | addr::SWEEP_WAIT_PHYS
        | addr::TAG_APPLAUD_PHYS
        | addr::TAG_CRUSH_PHYS
        | addr::TAG_ROCK_PAPER_PHYS
        | addr::THROW_PHYS
        | addr::WAIT1_0_PHYS
        | addr::WALK_SHOOT_PHYS
        | addr::WALK2_PHYS => g.mh_air_motion(gobj),
        addr::BACK_AIRPLANE1_COLL
        | addr::BACK_AIRPLANE2_COLL
        | addr::BACK_AIRPLANE3_COLL
        | addr::BACK_PUNCH_COLL
        | addr::BACK_CRUSH_COLL
        | addr::BACK_DISAPPEAR_COLL
        | addr::WAIT1_1_COLL
        | addr::GRAB_COLL
        | addr::FAIL_COLL
        | addr::CAPTURE_DAMAGE_MASTER_HAND_IASA
        | addr::CAPTURE_DAMAGE_MASTER_HAND_PHYS
        | addr::CAPTURE_DAMAGE_MASTER_HAND_COLL
        | addr::CAPTURE_MASTER_HAND_ANIM
        | addr::CAPTURE_MASTER_HAND_IASA
        | addr::CAPTURE_MASTER_HAND_PHYS
        | addr::CAPTURE_MASTER_HAND_COLL
        | addr::WALK_WAIT_COLL
        | addr::ENTRY_PHYS
        | addr::ENTRY_COLL
        | addr::POKE1_COLL
        | addr::FINGER_BEAM_START_COLL
        | addr::FINGER_BEAM_LOOP_COLL
        | addr::FINGER_BEAM_END_COLL
        | addr::FINGER_GUN1_COLL
        | addr::FINGER_GUN2_COLL
        | addr::FINGER_GUN3_COLL
        | addr::ROCK_CRUSH_DOWN_COLL
        | addr::PAPER_CRUSH_COLL
        | addr::DRILL_COLL
        | addr::ROCK_CRUSH_UP_COLL
        | addr::ROCK_CRUSH_WAIT_COLL
        | addr::SLAM_COLL
        | addr::SLAP_COLL
        | addr::SQUEEZING_COLL
        | addr::DAMAGE_COLL
        | addr::WAIT_SWEEP_COLL
        | addr::SWEEP_LOOP_COLL
        | addr::SWEEP_WAIT_COLL
        | addr::TAG_APPLAUD_COLL
        | addr::TAG_CRUSH_COLL
        | addr::TAG_ROCK_PAPER_COLL
        | addr::THROW_COLL
        | addr::CAPTURE_WAIT_MASTER_HAND_ANIM
        | addr::CAPTURE_WAIT_MASTER_HAND_IASA
        | addr::CAPTURE_WAIT_MASTER_HAND_PHYS
        | addr::CAPTURE_WAIT_MASTER_HAND_COLL
        | addr::THROWN_MASTER_HAND_ANIM
        | addr::THROWN_MASTER_HAND_IASA
        | addr::THROWN_MASTER_HAND_PHYS
        | addr::THROWN_MASTER_HAND_COLL
        | addr::WAIT1_0_COLL
        | addr::WALK_SHOOT_COLL
        | addr::WALK2_COLL
        | addr::WALK_LOOP_COLL => g.mh_nothing(gobj),
        addr::BACK_AIRPLANE3_ANIM
        | addr::FAIL_ANIM
        | addr::WALK_WAIT_ANIM
        | addr::DRILL_ANIM
        | addr::FINGER_BEAM_END_ANIM
        | addr::FINGER_GUN3_ANIM
        | addr::ROCK_CRUSH_DOWN_ANIM
        | addr::PAPER_CRUSH_ANIM
        | addr::SLAP_ANIM
        | addr::SWEEP_WAIT_ANIM
        | addr::WALK_SHOOT_ANIM => g.mh_anim_return(gobj),
        _ => return false,
    }
    true
}
