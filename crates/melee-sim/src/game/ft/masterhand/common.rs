//! The callbacks both hands' states share: empty ones, a human's input, the air motion, the
//! return once an animation ends, and entering a state from its first frame.

use super::super::super::state::Running;
use super::{HAND, Hand, addr};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// A human-played hand has nothing to do (`ftBossLib_8015BD20`), a computer's nothing
    /// either: the states' input callbacks.
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
    #[doc(alias = "ftMh_Cancel_IASA")]
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
    #[doc(alias = "ftCh_FingerGun1_IASA")]
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
    pub fn hand_human_nop(&mut self, gobj: GObjId) {
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
    pub fn hand_air_motion(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// Nothing: the states' empty callbacks, the spawn hooks (`OnDeath`), Master Hand's
    /// `ftMh_MS_341_8014FE58` and Crazy Hand's hit hook.
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
    #[doc(alias = "ftMh_CaptureWaitMasterHand_Anim")]
    #[doc(alias = "ftMh_CaptureWaitMasterHand_IASA")]
    #[doc(alias = "ftMh_CaptureWaitMasterHand_Phys")]
    #[doc(alias = "ftMh_CaptureWaitMasterHand_Coll")]
    // `ftMh_ThrownMasterHand_*` run here too, but never: the victim leaves the state in the
    // call that enters it (ledger/dead.txt).
    #[doc(alias = "ftMh_Wait1_0_Coll")]
    #[doc(alias = "ftMh_WalkShoot_Coll")]
    #[doc(alias = "ftMh_Walk2_Coll")]
    #[doc(alias = "ftMh_WalkLoop_Coll")]
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
    #[doc(alias = "ftCo_CaptureWaitCrazyHand_Anim")]
    #[doc(alias = "ftCo_CaptureWaitCrazyHand_IASA")]
    #[doc(alias = "ftCo_CaptureWaitCrazyHand_Phys")]
    #[doc(alias = "ftCo_CaptureWaitCrazyHand_Coll")]
    // `ftCo_ThrownCrazyHand_*` run here too, but never: the victim leaves the state in the
    // call that enters it (ledger/dead.txt).
    #[doc(alias = "ftMh_MS_341_8014FE58")]
    #[doc(alias = "ftCh_Init_80156014")]
    #[doc(alias = "ftMh_Init_OnDeath")]
    #[doc(alias = "ftCh_Init_OnDeath")]
    pub fn hand_nothing(&mut self, _gobj: GObjId) {}

    /// Back to his place and the wait once the animation ends (`ftMh_MS_389_80151018`,
    /// `ftCh_GrabUnk1_8015BC88`).
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
    pub fn hand_anim_return(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.hand_return(gobj, h);
        }
    }

    /// Enters one of his states from its first frame (`Fighter_ChangeMotionState` and
    /// `ftAnim_8006EBA4`): `ftMh_MS_344_80151828` (the sweep's wind-up),
    /// `ftMh_MS_347_80151AC8` (its end), `ftMh_MS_348_80151BB8` (the slap),
    /// `ftMh_MS_353_80151DC4` and `ftMh_MS_350_80151EB4` (the walk), `ftMh_MS_354_80152370` and
    /// `ftMh_MS_355_8015247C` (the punch).
    #[doc(alias = "ftMh_MS_344_80151828")]
    #[doc(alias = "ftMh_MS_347_80151AC8")]
    #[doc(alias = "ftMh_MS_348_80151BB8")]
    #[doc(alias = "ftMh_MS_353_80151DC4")]
    #[doc(alias = "ftMh_MS_350_80151EB4")]
    #[doc(alias = "ftMh_MS_354_80152370")]
    #[doc(alias = "ftMh_MS_355_8015247C")]
    #[doc(alias = "ftMh_MS_366_80153820")]
    #[doc(alias = "ftMh_MS_365_80153730")]
    pub fn hand_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }
}

#[melee_callbacks::callbacks(Fighter)]
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
        | addr::WALK_LOOP_IASA => g.hand_human_nop(gobj),
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
        | addr::WALK2_PHYS => g.hand_air_motion(gobj),
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
        | addr::WALK_LOOP_COLL => g.hand_nothing(gobj),
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
        | addr::WALK_SHOOT_ANIM => g.hand_anim_return(gobj, &HAND),
        _ => return false,
    }
    true
}
