//! Crazy Hand (`ft/kinds/ftCrazyHand`, fighter kind 28): the single-player modes' second boss,
//! Master Hand's left hand. He floats over the stage and attacks from his own table at random
//! (or a human's pad's button pairs on port 4), drops bombs (`it::kinds::crazyhandbomb`), and
//! joins Master Hand in three tag attacks: he picks one and waits (`TagRockPaper`) until Master
//! Hand's attack timer runs out, and Master Hand starts it for both (`ft::masterhand::tag`).
//! He's beaten only as a stamina fighter (the single-player setups); a fighter he grabs runs
//! the capture states here too (`ftcrazyhandcapture*.c`, `ftcrazyhandthrowncrazyhand.c`).
//!
//! His variables are Master Hand's unions (`ftMasterhand_FighterVars` in `u`,
//! `ftMasterHand_MotionVars` in `mv`: `fp->u.ch` and `fp->mv.ch` in the decomp), read at the
//! decomp's offsets; his code reads his attributes from his data (`ftData::ext_attr`,
//! `ftCrazyHand_DatAttrs`) rather than the fighter's copy.
//!
//! His states take Master Hand's numbers and the decomp's `ftMh_MS_*` names, which name Master
//! Hand's moves, not his: 0x157 and 0x158 are attacks, 0x179 to 0x17B his halves of the tag
//! attacks, 0x180 the wait for Master Hand, 0x181 and 0x182 his defeat, 0x183 his entrance,
//! 0x184 and 0x185 his glides. Nothing enters 0x17C, 0x17D or 0x17F (`ledger/dead.txt`).

pub mod attack;
pub mod common;
pub mod entry;
pub mod grab;
pub mod init;
pub mod tag;
pub mod wait;

use super::super::state::Running;
use super::FighterKind;
use super::co::ids::ms as co_ms;
use super::masterhand::Hand;
use super::masterhand::airplane::{Pass, Ram};
use super::masterhand::crush::{Chases, Countdown, Hold, Window};
use super::masterhand::damage::Beaten;
use super::masterhand::finger::Beams;
use super::masterhand::grab::Grab;
use super::masterhand::tag::TagCry;
use crate::hsd::gobj::GObjId;

/// `It_Kind_CrazyHand_Laser`, `_Bullet` and `_Bomb`, his articles in his `ftData`'s order.
pub const ITEMS: [i32; 3] = [127, 128, 129];

/// Crazy Hand's functions, by address.
pub mod addr {
    pub const INIT_ON_DEATH: u32 = 0x8015_5E18;
    pub const INIT_ON_LOAD: u32 = 0x8015_5E1C;
    pub const INIT_LOAD_SPECIAL_ATTRS: u32 = 0x8015_5F8C;
    #[doc(alias("INIT_80155FCC"))]
    pub const START: u32 = 0x8015_5FCC;
    #[doc(alias("INIT_80156014"))]
    pub const NOTHING: u32 = 0x8015_6014;
    #[doc(alias("INIT_80156018"))]
    pub const WAIT1_ENTER: u32 = 0x8015_6018;
    #[doc(alias("INIT_801560D8"))]
    pub const WAIT2_ENTER: u32 = 0x8015_60D8;
    #[doc(alias("INIT_80156198"))]
    pub const WAIT_ENTER: u32 = 0x8015_6198;
    #[doc(alias("INIT_80156310"))]
    pub const START_IN_PLACE: u32 = 0x8015_6310;
    pub const WAIT1_0_ANIM: u32 = 0x8015_6404;
    pub const WAIT1_0_IASA: u32 = 0x8015_6A70;
    pub const WAIT1_0_PHYS: u32 = 0x8015_6AB4;
    pub const WAIT1_0_COLL: u32 = 0x8015_6AD4;
    #[doc(alias("INIT_80156AD8"))]
    pub const HUMAN_ATTACK: u32 = 0x8015_6AD8;
    #[doc(alias("FN_80156F6C"))]
    pub const RISE_ENTER: u32 = 0x8015_6F6C;
    pub const ENTRY_ANIM: u32 = 0x8015_6FDC;
    pub const ENTRY_IASA: u32 = 0x8015_7018;
    pub const ENTRY_PHYS: u32 = 0x8015_705C;
    pub const ENTRY_COLL: u32 = 0x8015_707C;
    #[doc(alias("FN_80157080"))]
    pub const SLIDE_WINDUP_ENTER: u32 = 0x8015_7080;
    pub const DAMAGE_ANIM: u32 = 0x8015_70CC;
    pub const DAMAGE_IASA: u32 = 0x8015_7108;
    pub const DAMAGE_PHYS: u32 = 0x8015_714C;
    pub const DAMAGE_COLL: u32 = 0x8015_716C;
    #[doc(alias("INIT_80157170"))]
    pub const SLIDE_ENTER: u32 = 0x8015_7170;
    pub const DAMAGE2_ANIM: u32 = 0x8015_7204;
    pub const DAMAGE2_IASA: u32 = 0x8015_72D4;
    pub const DAMAGE2_PHYS: u32 = 0x8015_7318;
    pub const DAMAGE2_COLL: u32 = 0x8015_7378;
    #[doc(alias("INIT_8015737C"))]
    pub const WAIT_SWEEP_ENTER: u32 = 0x8015_737C;
    pub const WAIT_SWEEP_ANIM: u32 = 0x8015_73C8;
    pub const WAIT_SWEEP_PHYS: u32 = 0x8015_7404;
    pub const WAIT_SWEEP_IASA: u32 = 0x8015_7424;
    pub const WAIT_SWEEP_COLL: u32 = 0x8015_7468;
    #[doc(alias("FN_8015746C"))]
    pub const SWEEP_LOOP_ENTER: u32 = 0x8015_746C;
    pub const SWEEP_LOOP_ANIM: u32 = 0x8015_74B8;
    pub const SWEEP_LOOP_IASA: u32 = 0x8015_74F4;
    pub const SWEEP_LOOP_PHYS: u32 = 0x8015_7538;
    pub const SWEEP_LOOP_COLL: u32 = 0x8015_7558;
    #[doc(alias("FN_8015755C"))]
    pub const SLAP_WINDUP_ENTER: u32 = 0x8015_755C;
    pub const SWEEP_WAIT_ANIM: u32 = 0x8015_75A8;
    pub const SWEEP_WAIT_IASA: u32 = 0x8015_75E4;
    pub const SWEEP_WAIT_PHYS: u32 = 0x8015_7628;
    pub const SWEEP_WAIT_COLL: u32 = 0x8015_7648;
    #[doc(alias("INIT_8015764C"))]
    pub const SLAP_ENTER: u32 = 0x8015_764C;
    pub const SLAP_ANIM: u32 = 0x8015_76B0;
    pub const SLAP_IASA: u32 = 0x8015_7724;
    pub const SLAP_PHYS: u32 = 0x8015_7768;
    #[doc(alias("FN_801577B4"))]
    pub const SLAP_WATCH: u32 = 0x8015_77B4;
    #[doc(alias("INIT_801577F8"))]
    pub const WALK2_ENTER: u32 = 0x8015_77F8;
    pub const WALK2_ANIM: u32 = 0x8015_7844;
    pub const WALK2_IASA: u32 = 0x8015_7880;
    pub const WALK2_PHYS: u32 = 0x8015_78C4;
    pub const SLAP_COLL: u32 = 0x8015_78E4;
    #[doc(alias("FN_801578E8"))]
    pub const HOLD_START_ENTER: u32 = 0x8015_78E8;
    pub const WALK_LOOP_ANIM: u32 = 0x8015_7934;
    pub const WALK_LOOP_IASA: u32 = 0x8015_798C;
    pub const WALK_LOOP_PHYS: u32 = 0x8015_79D0;
    pub const WALK_LOOP_COLL: u32 = 0x8015_79F0;
    #[doc(alias("INIT_801579F4"))]
    pub const HOLD_ENTER: u32 = 0x8015_79F4;
    pub const WALK_WAIT_ANIM: u32 = 0x8015_7A40;
    pub const WALK_WAIT_IASA: u32 = 0x8015_7ABC;
    pub const WALK_WAIT_PHYS: u32 = 0x8015_7B00;
    pub const WALK_WAIT_COLL: u32 = 0x8015_7B54;
    #[doc(alias("INIT_80157B58"))]
    pub const HOLD_END_ENTER: u32 = 0x8015_7B58;
    pub const WALK_SHOOT_ANIM: u32 = 0x8015_7BAC;
    pub const WALK_SHOOT_IASA: u32 = 0x8015_7BE8;
    pub const WALK_SHOOT_PHYS: u32 = 0x8015_7C2C;
    pub const WALK_SHOOT_COLL: u32 = 0x8015_7C4C;
    #[doc(alias("FN_80157C50"))]
    pub const WINDOW_ENTER: u32 = 0x8015_7C50;
    pub const DRILL_ANIM: u32 = 0x8015_7CB0;
    pub const DRILL_IASA: u32 = 0x8015_7CEC;
    pub const DRILL_PHYS: u32 = 0x8015_7D30;
    pub const DRILL_COLL: u32 = 0x8015_7DF4;
    #[doc(alias("INIT_80157DF8"))]
    pub const CHASES_ENTER: u32 = 0x8015_7DF8;
    pub const ROCK_CRUSH_UP_ANIM: u32 = 0x8015_7EA0;
    pub const ROCK_CRUSH_WAIT_ANIM: u32 = 0x8015_7F48;
    pub const ROCK_CRUSH_UP_IASA: u32 = 0x8015_7F84;
    pub const ROCK_CRUSH_UP_PHYS: u32 = 0x8015_7FC8;
    pub const ROCK_CRUSH_UP_COLL: u32 = 0x8015_8140;
    #[doc(alias("FN_80158144"))]
    pub const COUNTDOWN_ENTER: u32 = 0x8015_8144;
    pub const ROCK_CRUSH_DOWN_ANIM: u32 = 0x8015_81A4;
    pub const ROCK_CRUSH_DOWN_IASA: u32 = 0x8015_81E0;
    pub const ROCK_CRUSH_DOWN_PHYS: u32 = 0x8015_8224;
    pub const ROCK_CRUSH_DOWN_COLL: u32 = 0x8015_82D4;
    #[doc(alias("FN_801582D8"))]
    pub const BEAMS_WINDUP_ENTER: u32 = 0x8015_82D8;
    pub const PAPER_CRUSH_ANIM: u32 = 0x8015_8340;
    pub const PAPER_CRUSH_IASA: u32 = 0x8015_837C;
    pub const PAPER_CRUSH_PHYS: u32 = 0x8015_83C0;
    pub const PAPER_CRUSH_COLL: u32 = 0x8015_83E0;
    #[doc(alias("INIT_801583E4"))]
    pub const BEAMS_ENTER: u32 = 0x8015_83E4;
    pub const POKE1_ANIM: u32 = 0x8015_8450;
    pub const POKE1_IASA: u32 = 0x8015_84CC;
    pub const POKE1_PHYS: u32 = 0x8015_8510;
    pub const POKE1_COLL: u32 = 0x8015_8530;
    #[doc(alias("FN_80158534"))]
    pub const BEAMS_LASERS: u32 = 0x8015_8534;
    #[doc(alias("INIT_8015868C"))]
    pub const BEAMS_END_ENTER: u32 = 0x8015_868C;
    pub const POKE2_ANIM: u32 = 0x8015_870C;
    pub const POKE2_IASA: u32 = 0x8015_8748;
    pub const POKE2_PHYS: u32 = 0x8015_878C;
    pub const POKE2_COLL: u32 = 0x8015_87AC;
    #[doc(alias("FN_801587B0"))]
    pub const BOMBS_ENTER: u32 = 0x8015_87B0;
    pub const FINGER_BEAM_START_ANIM: u32 = 0x8015_8814;
    pub const FINGER_BEAM_START_IASA: u32 = 0x8015_8850;
    pub const FINGER_BEAM_START_PHYS: u32 = 0x8015_8894;
    pub const FINGER_BEAM_START_COLL: u32 = 0x8015_88B4;
    #[doc(alias("FN_801588B8"))]
    pub const BOMBS: u32 = 0x8015_88B8;
    #[doc(alias("INIT_80158B3C"))]
    pub const REACH_ENTER: u32 = 0x8015_8B3C;
    pub const FINGER_BEAM_LOOP_ANIM: u32 = 0x8015_8BBC;
    pub const FINGER_BEAM_LOOP_IASA: u32 = 0x8015_8C44;
    pub const FINGER_BEAM_LOOP_PHYS: u32 = 0x8015_8C88;
    pub const FINGER_BEAM_LOOP_COLL: u32 = 0x8015_8DF8;
    #[doc(alias("INIT_80158DFC"))]
    pub const GRAB_ENTER: u32 = 0x8015_8DFC;
    pub const FINGER_BEAM_END_ANIM: u32 = 0x8015_8E7C;
    pub const FINGER_BEAM_END_IASA: u32 = 0x8015_8ECC;
    pub const FINGER_BEAM_END_PHYS: u32 = 0x8015_8F10;
    pub const FINGER_BEAM_END_COLL: u32 = 0x8015_8F30;
    #[doc(alias("INIT_80158F34"))]
    pub const MISS_ENTER: u32 = 0x8015_8F34;
    pub const BACK_PUNCH_ANIM: u32 = 0x8015_8FB4;
    pub const BACK_PUNCH_IASA: u32 = 0x8015_8FF0;
    pub const BACK_PUNCH_PHYS: u32 = 0x8015_9034;
    pub const BACK_PUNCH_COLL: u32 = 0x8015_9094;
    #[doc(alias("INIT_80159098"))]
    pub const LET_GO_ENTER: u32 = 0x8015_9098;
    pub const FINGER_GUN1_ANIM: u32 = 0x8015_9134;
    pub const FINGER_GUN1_IASA: u32 = 0x8015_91C0;
    pub const FINGER_GUN1_PHYS: u32 = 0x8015_9204;
    pub const FINGER_GUN1_COLL: u32 = 0x8015_9264;
    #[doc(alias("FN_80159288"))]
    pub const CAUGHT: u32 = 0x8015_9288;
    #[doc(alias("INIT_801592D4"))]
    pub const SQUEEZE_ENTER: u32 = 0x8015_92D4;
    pub const BACK_AIRPLANE1_ANIM: u32 = 0x8015_9388;
    pub const BACK_AIRPLANE1_IASA: u32 = 0x8015_940C;
    pub const BACK_AIRPLANE1_PHYS: u32 = 0x8015_9450;
    pub const BACK_AIRPLANE1_COLL: u32 = 0x8015_94B0;
    #[doc(alias("INIT_801594D4"))]
    pub const THROW1_ENTER: u32 = 0x8015_94D4;
    pub const BACK_AIRPLANE2_ANIM: u32 = 0x8015_9534;
    pub const BACK_AIRPLANE2_IASA: u32 = 0x8015_9608;
    pub const BACK_AIRPLANE2_PHYS: u32 = 0x8015_964C;
    pub const BACK_AIRPLANE2_COLL: u32 = 0x8015_966C;
    #[doc(alias("INIT_80159670"))]
    pub const THROW2_ENTER: u32 = 0x8015_9670;
    pub const BACK_AIRPLANE3_ANIM: u32 = 0x8015_96D0;
    pub const BACK_AIRPLANE3_IASA: u32 = 0x8015_9788;
    pub const BACK_AIRPLANE3_PHYS: u32 = 0x8015_97CC;
    pub const BACK_AIRPLANE3_COLL: u32 = 0x8015_97EC;
    #[doc(alias("INIT_801597F0"))]
    pub const VANISH_ENTER: u32 = 0x8015_97F0;
    pub const BACK_CRUSH_ANIM: u32 = 0x8015_9858;
    pub const BACK_CRUSH_IASA: u32 = 0x8015_98A0;
    pub const BACK_CRUSH_PHYS: u32 = 0x8015_98E4;
    pub const BACK_CRUSH_COLL: u32 = 0x8015_9904;
    #[doc(alias("FN_80159908"))]
    pub const RAM_ENTER: u32 = 0x8015_9908;
    pub const BACK_DISAPPEAR_ANIM: u32 = 0x8015_99C0;
    pub const BACK_DISAPPEAR_IASA: u32 = 0x8015_9A08;
    pub const BACK_DISAPPEAR_PHYS: u32 = 0x8015_9A4C;
    pub const BACK_DISAPPEAR_COLL: u32 = 0x8015_9AA0;
    #[doc(alias("FN_80159AA4"))]
    pub const SHRINK_ENTER: u32 = 0x8015_9AA4;
    pub const WAIT1_1_ANIM: u32 = 0x8015_9D04;
    pub const WAIT1_1_IASA: u32 = 0x8015_9E8C;
    pub const WAIT1_1_PHYS: u32 = 0x8015_9ED0;
    pub const WAIT1_1_COLL: u32 = 0x8015_9F3C;
    #[doc(alias("INIT_80159F40"))]
    pub const PASS_RISE_ENTER: u32 = 0x8015_9F40;
    pub const GRAB_ANIM: u32 = 0x8015_9F8C;
    pub const GRAB_IASA: u32 = 0x8015_9FC8;
    pub const GRAB_PHYS: u32 = 0x8015_A00C;
    pub const GRAB_COLL: u32 = 0x8015_A02C;
    #[doc(alias("INIT_8015A030"))]
    pub const PASS_ENTER: u32 = 0x8015_A030;
    pub const CANCEL_ANIM: u32 = 0x8015_A0C4;
    pub const CANCEL_IASA: u32 = 0x8015_A10C;
    pub const CANCEL_PHYS: u32 = 0x8015_A150;
    pub const CANCEL_COLL: u32 = 0x8015_A180;
    #[doc(alias("INIT_8015A184"))]
    pub const PASS_BACK_ENTER: u32 = 0x8015_A184;
    pub const SQUEEZING0_ANIM: u32 = 0x8015_A20C;
    pub const SQUEEZING0_IASA: u32 = 0x8015_A248;
    pub const SQUEEZING0_PHYS: u32 = 0x8015_A28C;
    pub const SQUEEZING0_COLL: u32 = 0x8015_A2AC;
    #[doc(alias("INIT_8015A2B0"))]
    pub const TAG_HALF_CRUSH_ENTER: u32 = 0x8015_A2B0;
    pub const SQUEEZING1_ANIM: u32 = 0x8015_A318;
    pub const SQUEEZING1_IASA: u32 = 0x8015_A38C;
    pub const SQUEEZING1_PHYS: u32 = 0x8015_A3D0;
    pub const SQUEEZING1_COLL: u32 = 0x8015_A3F0;
    #[doc(alias("INIT_8015A3F4"))]
    pub const TAG_HALF_APPLAUD_ENTER: u32 = 0x8015_A3F4;
    pub const SQUEEZE_ANIM: u32 = 0x8015_A464;
    pub const SQUEEZE_IASA: u32 = 0x8015_A4F8;
    pub const SQUEEZE_PHYS: u32 = 0x8015_A53C;
    pub const SQUEEZE_COLL: u32 = 0x8015_A55C;
    #[doc(alias("INIT_8015A560"))]
    pub const TAG_HALF_ROCK_PAPER_ENTER: u32 = 0x8015_A560;
    pub const THROW_ANIM: u32 = 0x8015_A5D0;
    pub const THROW_IASA: u32 = 0x8015_A664;
    pub const THROW_PHYS: u32 = 0x8015_A6A8;
    pub const THROW_COLL: u32 = 0x8015_A6C8;
    pub const SLAM_ANIM: u32 = 0x8015_A6CC;
    pub const SLAM_IASA: u32 = 0x8015_A750;
    pub const SLAM_PHYS: u32 = 0x8015_A794;
    pub const SLAM_COLL: u32 = 0x8015_A7B4;
    pub const FAIL_ANIM: u32 = 0x8015_A7B8;
    pub const FAIL_IASA: u32 = 0x8015_A820;
    pub const FAIL_PHYS: u32 = 0x8015_A864;
    pub const FAIL_COLL: u32 = 0x8015_A884;
    #[doc(alias("GRAB_UNK1_8015A888"))]
    pub const TAG_CRUSH_ENTER: u32 = 0x8015_A888;
    pub const TAG_CRUSH_ANIM: u32 = 0x8015_A8D4;
    pub const TAG_CRUSH_IASA: u32 = 0x8015_A910;
    pub const TAG_CRUSH_PHYS: u32 = 0x8015_A954;
    pub const TAG_CRUSH_COLL: u32 = 0x8015_A974;
    pub const TAG_APPLAUD_ANIM: u32 = 0x8015_A978;
    pub const TAG_APPLAUD_IASA: u32 = 0x8015_AA00;
    pub const TAG_APPLAUD_PHYS: u32 = 0x8015_AA44;
    pub const TAG_APPLAUD_COLL: u32 = 0x8015_AAA4;
    #[doc(alias("FN_8015AAC8"))]
    pub const TAG_WAIT_ENTER: u32 = 0x8015_AAC8;
    pub const TAG_ROCK_PAPER_ANIM: u32 = 0x8015_AB04;
    pub const TAG_ROCK_PAPER_IASA: u32 = 0x8015_AB84;
    pub const TAG_ROCK_PAPER_PHYS: u32 = 0x8015_ABC8;
    pub const TAG_ROCK_PAPER_COLL: u32 = 0x8015_ABCC;
    #[doc(alias("GRAB_UNK1_8015ABD0"))]
    pub const CLOSE_CAUGHT_ENTER: u32 = 0x8015_ABD0;
    #[doc(alias("GRAB_UNK1_8015AC50"))]
    pub const CLOSE_EMPTY_ENTER: u32 = 0x8015_AC50;
    pub const FINGER_GUN2_ANIM: u32 = 0x8015_ACD0;
    pub const FINGER_GUN2_IASA: u32 = 0x8015_AD28;
    pub const FINGER_GUN2_PHYS: u32 = 0x8015_AD6C;
    pub const FINGER_GUN2_COLL: u32 = 0x8015_ADCC;
    #[doc(alias("GRAB_UNK1_8015ADD0"))]
    pub const BEATEN_ENTER: u32 = 0x8015_ADD0;
    pub const TAG_GRAB_ANIM: u32 = 0x8015_AF04;
    pub const TAG_SQUEEZE_ANIM: u32 = 0x8015_B000;
    pub const TAG_GRAB_IASA: u32 = 0x8015_B0FC;
    pub const TAG_GRAB_PHYS: u32 = 0x8015_B140;
    pub const TAG_GRAB_COLL: u32 = 0x8015_B170;
    #[doc(alias("GRAB_UNK1_8015B174"))]
    pub const ENTRY_ENTER: u32 = 0x8015_B174;
    pub const TAG_FAIL_ANIM: u32 = 0x8015_B22C;
    pub const TAG_FAIL_IASA: u32 = 0x8015_B274;
    pub const TAG_FAIL_PHYS: u32 = 0x8015_B2B8;
    pub const TAG_FAIL_COLL: u32 = 0x8015_B2BC;
    #[doc(alias("FN_8015B2C0"))]
    pub const ENTRY_CAMERA: u32 = 0x8015_B2C0;
    #[doc(alias("FN_8015B4EC"))]
    pub const WAIST_POINT: u32 = 0x8015_B4EC;
    #[doc(alias("FN_8015B548"))]
    pub const CAPTURE_ENTER: u32 = 0x8015_B548;
    pub const CAPTURE_CRAZY_HAND_ANIM: u32 = 0x8015_B660;
    pub const CAPTURE_CRAZY_HAND_IASA: u32 = 0x8015_B664;
    pub const CAPTURE_CRAZY_HAND_PHYS: u32 = 0x8015_B668;
    pub const CAPTURE_CRAZY_HAND_COLL: u32 = 0x8015_B66C;
    #[doc(alias("GRAB_UNK1_8015B670"))]
    pub const CAPTURE_DAMAGE_ENTER: u32 = 0x8015_B670;
    pub const CAPTURE_DAMAGE_CRAZY_HAND_ANIM: u32 = 0x8015_B704;
    pub const CAPTURE_DAMAGE_CRAZY_HAND_IASA: u32 = 0x8015_B76C;
    pub const CAPTURE_DAMAGE_CRAZY_HAND_PHYS: u32 = 0x8015_B770;
    pub const CAPTURE_DAMAGE_CRAZY_HAND_COLL: u32 = 0x8015_B774;
    #[doc(alias("GRAB_UNK1_8015B778"))]
    pub const CAPTURE_WAIT_ENTER: u32 = 0x8015_B778;
    #[doc(alias("GRAB_UNK1_8015B800"))]
    pub const CAPTURE_CUT: u32 = 0x8015_B800;
    pub const CAPTURE_WAIT_CRAZY_HAND_ANIM: u32 = 0x8015_B840;
    pub const CAPTURE_WAIT_CRAZY_HAND_IASA: u32 = 0x8015_B844;
    pub const CAPTURE_WAIT_CRAZY_HAND_PHYS: u32 = 0x8015_B848;
    pub const CAPTURE_WAIT_CRAZY_HAND_COLL: u32 = 0x8015_B84C;
    #[doc(alias("GRAB_UNK1_8015B850"))]
    pub const THROWN_ENTER: u32 = 0x8015_B850;
    pub const THROWN_CRAZY_HAND_ANIM: u32 = 0x8015_B8EC;
    pub const THROWN_CRAZY_HAND_IASA: u32 = 0x8015_B8F0;
    pub const THROWN_CRAZY_HAND_PHYS: u32 = 0x8015_B8F4;
    pub const THROWN_CRAZY_HAND_COLL: u32 = 0x8015_B8F8;
    #[doc(alias("GRAB_UNK1_8015B8FC"))]
    pub const GLIDE1_ENTER: u32 = 0x8015_B8FC;
    #[doc(alias("GRAB_UNK1_8015B998"))]
    pub const GLIDE2_ENTER: u32 = 0x8015_B998;
    #[doc(alias("GRAB_UNK1_8015BA34"))]
    pub const GLIDE_TO: u32 = 0x8015_BA34;
    pub const TAG_CANCEL_ANIM: u32 = 0x8015_BB70;
    pub const TAG_CANCEL_PHYS: u32 = 0x8015_BBD8;
    pub const TAG_CANCEL_COLL: u32 = 0x8015_BC38;
    #[doc(alias("GRAB_UNK1_8015BC88"))]
    pub const RETURN: u32 = 0x8015_BC88;
}

/// His action states (Master Hand's numbers and the decomp's names).
pub mod ms {
    pub const WAIT1_0: i32 = 0x155;
    pub const WAIT2_0: i32 = 0x156;
    /// An attack that rises at the stage's left end (`fn_80156F6C`).
    pub const ENTRY: i32 = 0x157;
    /// An attack from the stage's left end (`fn_80157080`), then the slide (`Damage2`).
    pub const DAMAGE: i32 = 0x158;
    pub const DAMAGE2: i32 = 0x159;
    pub const WAIT_SWEEP: i32 = 0x15A;
    pub const SWEEP_LOOP: i32 = 0x15B;
    pub const SWEEP_WAIT: i32 = 0x15C;
    pub const SLAP: i32 = 0x15D;
    pub const WALK2: i32 = 0x15E;
    pub const WALK_LOOP: i32 = 0x15F;
    pub const WALK_WAIT: i32 = 0x160;
    pub const WALK_SHOOT: i32 = 0x161;
    pub const DRILL: i32 = 0x162;
    pub const ROCK_CRUSH_UP: i32 = 0x163;
    pub const ROCK_CRUSH_WAIT: i32 = 0x164;
    pub const ROCK_CRUSH_DOWN: i32 = 0x165;
    pub const PAPER_CRUSH: i32 = 0x166;
    pub const POKE1: i32 = 0x167;
    pub const POKE2: i32 = 0x168;
    pub const FINGER_BEAM_START: i32 = 0x169;
    pub const FINGER_BEAM_LOOP: i32 = 0x16A;
    pub const FINGER_BEAM_END: i32 = 0x16B;
    pub const FINGER_GUN1: i32 = 0x16C;
    pub const FINGER_GUN2: i32 = 0x16D;
    pub const FINGER_GUN3: i32 = 0x16E;
    pub const BACK_AIRPLANE1: i32 = 0x16F;
    pub const BACK_AIRPLANE2: i32 = 0x170;
    pub const BACK_AIRPLANE3: i32 = 0x171;
    pub const BACK_PUNCH: i32 = 0x172;
    pub const BACK_CRUSH: i32 = 0x173;
    pub const BACK_DISAPPEAR: i32 = 0x174;
    pub const WAIT1_1: i32 = 0x175;
    pub const GRAB: i32 = 0x176;
    pub const CANCEL: i32 = 0x177;
    pub const SQUEEZING0: i32 = 0x178;
    pub const SQUEEZING1: i32 = 0x179;
    pub const SQUEEZE: i32 = 0x17A;
    pub const THROW: i32 = 0x17B;
    pub const SLAM: i32 = 0x17C;
    pub const TAG_CRUSH: i32 = 0x17E;
    pub const TAG_ROCK_PAPER: i32 = 0x180;
    pub const TAG_GRAB: i32 = 0x181;
    pub const TAG_SQUEEZE: i32 = 0x182;
    pub const TAG_FAIL: i32 = 0x183;
    pub const TAG_CANCEL: i32 = 0x184;
    pub const WAIT2_1: i32 = 0x185;
}

/// `ftMasterHand_MotionVars`' offsets in `mv` as he uses them (`fp->mv.ch`) where they aren't
/// the hands' shared words (`ft::masterhand::mv`).
pub(crate) mod mv {
    /// Where he's gliding (`unk0.xC`).
    pub const TARGET: usize = 0xC;
    /// The poke's lasers (`grab.x28` to `x34`).
    pub const LASERS: [usize; 4] = [0x28, 0x2C, 0x30, 0x34];
    /// The poke's hums (`unk0.x38` to `x40`).
    pub const HUMS: [usize; 3] = [0x38, 0x3C, 0x40];
    /// The shrink's growth (`ch_backcrush.x44`: its step, then his full scale at `x50` and
    /// its count at `x5C`).
    pub const SCALE_STEP: usize = 0x44;
    /// The defeat's cheers so far (`ch_dmg.x60`, `x64`).
    pub const CHEERS: usize = 0x60;
    pub const CHEERS2: usize = 0x64;
}

/// `ftCrazyHand_DatAttrs`' offsets (0x144 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0x144;
    /// The attack timer's base and spread (`x0`, `x4` low, `x8` high).
    pub const X0: usize = 0x0;
    pub const X4: usize = 0x4;
    pub const X8: usize = 0x8;
    /// The attacks between forced walks (`xC`).
    pub const XC: usize = 0xC;
    /// The glide's rate and its near distance (`x10`, `x14`).
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    /// His place (`x18`, `x1C`).
    pub const X18: usize = 0x18;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X34: usize = 0x34;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X54: usize = 0x54;
    pub const X5C: usize = 0x5C;
    pub const X64: usize = 0x64;
    pub const X68: usize = 0x68;
    pub const X6C: usize = 0x6C;
    pub const X74: usize = 0x74;
    pub const X78: usize = 0x78;
    pub const X7C: usize = 0x7C;
    pub const X80: usize = 0x80;
    pub const X88: usize = 0x88;
    pub const X90: usize = 0x90;
    pub const X98: usize = 0x98;
    pub const XA0: usize = 0xA0;
    pub const XA4: usize = 0xA4;
    pub const XA8: usize = 0xA8;
    pub const XAC: usize = 0xAC;
    pub const XB0: usize = 0xB0;
    pub const XB4: usize = 0xB4;
    pub const XB8: usize = 0xB8;
    pub const XBC: usize = 0xBC;
    pub const XC0: usize = 0xC0;
    pub const XC8: usize = 0xC8;
    pub const XD0: usize = 0xD0;
    pub const XD8: usize = 0xD8;
    pub const XDC: usize = 0xDC;
    pub const XE0: usize = 0xE0;
    pub const XE4: usize = 0xE4;
    pub const XEC: usize = 0xEC;
    pub const XF0: usize = 0xF0;
    pub const XF4: usize = 0xF4;
    pub const XF8: usize = 0xF8;
    pub const XFC: usize = 0xFC;
    pub const X100: usize = 0x100;
    pub const X104: usize = 0x104;
    pub const X10C: usize = 0x10C;
    pub const X114: usize = 0x114;
    pub const X11C: usize = 0x11C;
    pub const X124: usize = 0x124;
    pub const X128: usize = 0x128;
    pub const X12C: usize = 0x12C;
    pub const X130: usize = 0x130;
    pub const X134: usize = 0x134;
    pub const X138: usize = 0x138;
    pub const X13C: usize = 0x13C;
}

/// Crazy Hand's settings of the hands' shared code (`ft::masterhand::Hand`).
pub const HAND: Hand = Hand {
    kind: FighterKind::CrazyHand,
    attrs_size: attr::SIZE,
    home: attr::X18,
    glide_near: attr::X14,
    glide_rate: attr::X10,
    timer: [attr::X0, attr::X8, attr::X4],
    walk_every: attr::XC,
    wait1: ms::WAIT1_0,
    wait2: ms::WAIT2_0,
    glide1: ms::TAG_CANCEL,
    glide2: ms::WAIT2_1,
    back_to_wait: addr::WAIT_ENTER,
    return_mark: ms::TAG_CANCEL,
    attacks: &wait::ATTACKS,
    rows: &wait::ROWS,
    next_rows: &wait::NEXT_ROWS,
    start_attack: |r, g, a| r.ch_attack(g, a),
    human_attack: |r, g| r.ch_human_attack(g),
    entry: ms::TAG_FAIL,
    entry_camera: addr::ENTRY_CAMERA,
    waist: addr::WAIST_POINT,
    entry_sfx: 320002,
    entry_facing: 1.0,
    items: &ITEMS,
    first_attack: ms::DAMAGE2,
    lasers: mv::LASERS,
    hums: mv::HUMS,
    partner: FighterKind::MasterHand,
    grab: Grab {
        reach: ms::FINGER_BEAM_LOOP,
        grab: ms::FINGER_BEAM_END,
        close_caught: ms::FINGER_GUN2,
        close_empty: ms::FINGER_GUN3,
        miss: ms::BACK_PUNCH,
        let_go: ms::FINGER_GUN1,
        squeeze: ms::BACK_AIRPLANE1,
        throws: [ms::BACK_AIRPLANE2, ms::BACK_AIRPLANE3],
        reach_frames: attr::XC8,
        reach_chase: attr::XC0,
        squeeze_at: attr::XD0,
        let_go_frames: attr::XD8,
        box_size: 0x100,
        caught: addr::CAUGHT,
        victim: addr::CAPTURE_ENTER,
        capture: co_ms::CAPTURE_CRAZY_HAND,
        capture_damage: co_ms::CAPTURE_DAMAGE_CRAZY_HAND,
        capture_wait: co_ms::CAPTURE_WAIT_CRAZY_HAND,
        thrown: co_ms::THROWN_CRAZY_HAND,
    },
    beaten: Beaten {
        states: [ms::TAG_GRAB, ms::TAG_SQUEEZE],
        fall: [attr::X12C, attr::X134, attr::X13C],
        accel: [attr::X130, attr::X138],
        cheer_counts: [attr::X124, attr::X128],
        cheers: [mv::CHEERS, mv::CHEERS2],
        sfx: 0x4E219,
    },
    tag_cry: TagCry::AllBut(ms::SQUEEZING1),
    hold: Hold {
        states: [ms::WALK_WAIT, ms::WALK_SHOOT],
        frames: attr::X64,
        follow: attr::X68,
    },
    window: Window {
        state: ms::DRILL,
        frames: [attr::X74, attr::X78],
        follow: attr::X7C,
    },
    countdown: Countdown {
        state: ms::ROCK_CRUSH_DOWN,
        late: attr::X48,
        follow: attr::X44,
    },
    chases: Chases {
        states: [ms::ROCK_CRUSH_UP, ms::ROCK_CRUSH_WAIT],
        count: attr::X54,
        point: attr::X5C,
    },
    pass: Pass {
        states: [ms::CANCEL, ms::SQUEEZING0],
        over: attr::XE0,
        follow: attr::XDC,
        back: attr::XE4,
    },
    ram: Ram {
        states: [ms::BACK_DISAPPEAR, ms::WAIT1_1],
        over: [attr::XEC, attr::XF4],
        frames: [attr::XF0, attr::XF8],
        follow: attr::XDC,
        small: attr::XFC,
        grow: attr::X100,
        growth: mv::SCALE_STEP,
    },
    beams: Beams {
        states: [ms::PAPER_CRUSH, ms::POKE1, ms::POKE2],
        lasers: addr::BEAMS_LASERS,
        partners_lasers: true,
    },
    vanish: ms::BACK_CRUSH,
    cries_waiting: true,
    tags_from_wait: false,
    fewer_rows_alone: true,
};

/// Runs one of Crazy Hand's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is
/// one.
#[melee_callbacks::callbacks(Fighter)]
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    common::run(g, gobj, addr)
        || init::run(g, gobj, addr)
        || wait::run(g, gobj, addr)
        || entry::run(g, gobj, addr)
        || attack::run(g, gobj, addr)
        || grab::run(g, gobj, addr)
        || tag::run(g, gobj, addr)
}

/// Runs one of his functions that takes his GObj and a callback (`ftCh_Init_801597F0`), if
/// `addr` is one.
pub fn run_with(g: &mut Running, gobj: GObjId, addr: u32, cb: u32) -> bool {
    match addr {
        addr::VANISH_ENTER => g.hand_vanish_enter(gobj, &HAND, cb),
        _ => return false,
    }
    true
}
