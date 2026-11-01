//! Master Hand (`ft/kinds/ftMasterHand`, fighter kind 27): the first single-player boss, a
//! hand that floats over the stage and attacks from a table at random (or a human's pad's
//! button pairs on port 3), and is beaten only as a stamina fighter (the single-player
//! setups). His items, the finger beams' lasers and the finger gun's bullets, are in
//! `it::kinds`; the hands' shared code is `ft::bosslib`; a fighter he grabs runs the capture
//! states here too (`ftmasterhandcapture*.c`, `ftmasterhandthrownmasterhand.c`).
//!
//! His variables are `ftMasterhand_FighterVars` in `u` and `ftMasterHand_MotionVars` in `mv`,
//! read at the decomp's offsets; his code reads his attributes from his data
//! (`ftData::ext_attr`, `ftMasterHand_SpecialAttrs`) rather than the fighter's copy.
//!
//! Crazy Hand's tag attacks with him (`ftmasterhandtag*.c`) wait for task 12.

pub mod airplane;
pub mod common;
pub mod crush;
pub mod damage;
pub mod entry;
pub mod finger;
pub mod grab;
pub mod init;
pub mod sweep;
pub mod wait;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_MASTERH` and `FTKIND_CREZYH`.
pub const KIND_MASTER_HAND: i32 = 0x1B;
pub const KIND_CRAZY_HAND: i32 = 0x1C;

/// `It_Kind_MasterHand_Laser` and `_Bullet`, his articles in his `ftData`'s order.
pub const ITEMS: [i32; 2] = [125, 126];

/// `FtPart_WaistN`.
pub(crate) const PART_WAIST_N: usize = 5;

/// Master Hand's functions, by address.
pub mod addr {
    pub const INIT_ON_DEATH: u32 = 0x8014_FC68;
    pub const INIT_ON_LOAD: u32 = 0x8014_FC6C;
    pub const INIT_LOAD_SPECIAL_ATTRS: u32 = 0x8014_FDD0;
    pub const MS_341_8014FE10: u32 = 0x8014_FE10;
    pub const MS_341_8014FE58: u32 = 0x8014_FE58;
    pub const MS_341_8014FE5C: u32 = 0x8014_FE5C;
    pub const MS_341_8014FF1C: u32 = 0x8014_FF1C;
    pub const MS_341_8014FFDC: u32 = 0x8014_FFDC;
    pub const IF_STAGE251: u32 = 0x8015_0144;
    pub const WAIT1_0_ANIM: u32 = 0x8015_0230;
    pub const WAIT1_0_IASA: u32 = 0x8015_082C;
    pub const WAIT1_0_PHYS: u32 = 0x8015_0870;
    pub const WAIT1_0_COLL: u32 = 0x8015_0890;
    pub const MS_341_80150894: u32 = 0x8015_0894;
    pub const MS_389_80150C8C: u32 = 0x8015_0C8C;
    pub const MS_389_80150D28: u32 = 0x8015_0D28;
    pub const MS_389_80150DC4: u32 = 0x8015_0DC4;
    pub const WAIT1_2_ANIM: u32 = 0x8015_0F00;
    pub const WAIT1_2_PHYS: u32 = 0x8015_0F68;
    pub const WAIT1_2_COLL: u32 = 0x8015_0FC8;
    pub const MS_389_80151018: u32 = 0x8015_1018;
    pub const MS_343_801510B0: u32 = 0x8015_10B0;
    pub const ENTRY_ANIM: u32 = 0x8015_1168;
    pub const ENTRY_IASA: u32 = 0x8015_11B0;
    pub const ENTRY_PHYS: u32 = 0x8015_11F4;
    pub const ENTRY_COLL: u32 = 0x8015_11F8;
    pub const MS_343_801511FC: u32 = 0x8015_11FC;
    pub const MS_343_80151428: u32 = 0x8015_1428;
    pub const MS_343_80151484: u32 = 0x8015_1484;
    pub const DAMAGE_ANIM: u32 = 0x8015_15B8;
    pub const MS_345_ANIM: u32 = 0x8015_16B4;
    pub const DAMAGE_IASA: u32 = 0x8015_17B0;
    pub const DAMAGE_PHYS: u32 = 0x8015_17F4;
    pub const DAMAGE_COLL: u32 = 0x8015_1824;
    pub const MS_344_80151828: u32 = 0x8015_1828;
    pub const WAIT_SWEEP_ANIM: u32 = 0x8015_1874;
    pub const WAIT_SWEEP_IASA: u32 = 0x8015_18B0;
    pub const WAIT_SWEEP_PHYS: u32 = 0x8015_18F4;
    pub const WAIT_SWEEP_COLL: u32 = 0x8015_1914;
    pub const MS_346_80151918: u32 = 0x8015_1918;
    pub const SWEEP_LOOP_ANIM: u32 = 0x8015_198C;
    pub const SWEEP_LOOP_IASA: u32 = 0x8015_1A44;
    pub const SWEEP_LOOP_PHYS: u32 = 0x8015_1A88;
    pub const SWEEP_LOOP_COLL: u32 = 0x8015_1AC4;
    pub const MS_347_80151AC8: u32 = 0x8015_1AC8;
    pub const SWEEP_WAIT_ANIM: u32 = 0x8015_1B14;
    pub const SWEEP_WAIT_PHYS: u32 = 0x8015_1B50;
    pub const SWEEP_WAIT_IASA: u32 = 0x8015_1B70;
    pub const SWEEP_WAIT_COLL: u32 = 0x8015_1BB4;
    pub const MS_348_80151BB8: u32 = 0x8015_1BB8;
    pub const SLAP_ANIM: u32 = 0x8015_1C04;
    pub const SLAP_IASA: u32 = 0x8015_1C40;
    pub const SLAP_PHYS: u32 = 0x8015_1C84;
    pub const SLAP_COLL: u32 = 0x8015_1CA4;
    pub const MS_349_80151CA8: u32 = 0x8015_1CA8;
    pub const WALK_SHOOT_ANIM: u32 = 0x8015_1D20;
    pub const WALK_SHOOT_IASA: u32 = 0x8015_1D5C;
    pub const WALK_SHOOT_PHYS: u32 = 0x8015_1DA0;
    pub const WALK_SHOOT_COLL: u32 = 0x8015_1DC0;
    pub const MS_353_80151DC4: u32 = 0x8015_1DC4;
    pub const WALK2_ANIM: u32 = 0x8015_1E10;
    pub const WALK2_IASA: u32 = 0x8015_1E4C;
    pub const WALK2_PHYS: u32 = 0x8015_1E90;
    pub const WALK2_COLL: u32 = 0x8015_1EB0;
    pub const MS_350_80151EB4: u32 = 0x8015_1EB4;
    pub const WALK_LOOP_ANIM: u32 = 0x8015_1F00;
    pub const WALK_LOOP_IASA: u32 = 0x8015_204C;
    pub const WALK_LOOP_PHYS: u32 = 0x8015_2090;
    pub const WALK_LOOP_COLL: u32 = 0x8015_20D4;
    pub const MS_351_801520D8: u32 = 0x8015_20D8;
    pub const WALK_WAIT_ANIM: u32 = 0x8015_2138;
    pub const WALK_WAIT_IASA: u32 = 0x8015_2174;
    pub const WALK_WAIT_PHYS: u32 = 0x8015_21B8;
    pub const WALK_WAIT_COLL: u32 = 0x8015_21D8;
    pub const MS_352_801521DC: u32 = 0x8015_21DC;
    pub const DRILL_ANIM: u32 = 0x8015_223C;
    pub const DRILL_IASA: u32 = 0x8015_2278;
    pub const DRILL_PHYS: u32 = 0x8015_22BC;
    pub const DRILL_COLL: u32 = 0x8015_236C;
    pub const MS_354_80152370: u32 = 0x8015_2370;
    pub const ROCK_CRUSH_UP_ANIM: u32 = 0x8015_23BC;
    pub const ROCK_CRUSH_UP_IASA: u32 = 0x8015_2414;
    pub const ROCK_CRUSH_UP_PHYS: u32 = 0x8015_2458;
    pub const ROCK_CRUSH_UP_COLL: u32 = 0x8015_2478;
    pub const MS_355_8015247C: u32 = 0x8015_247C;
    pub const ROCK_CRUSH_WAIT_ANIM: u32 = 0x8015_24C8;
    pub const ROCK_CRUSH_WAIT_IASA: u32 = 0x8015_2544;
    pub const ROCK_CRUSH_WAIT_PHYS: u32 = 0x8015_2588;
    pub const ROCK_CRUSH_WAIT_COLL: u32 = 0x8015_25DC;
    pub const MS_356_801525E0: u32 = 0x8015_25E0;
    pub const ROCK_CRUSH_DOWN_ANIM: u32 = 0x8015_2634;
    pub const ROCK_CRUSH_DOWN_IASA: u32 = 0x8015_2670;
    pub const ROCK_CRUSH_DOWN_PHYS: u32 = 0x8015_26B4;
    pub const ROCK_CRUSH_DOWN_COLL: u32 = 0x8015_26D4;
    pub const MS_357_801526D8: u32 = 0x8015_26D8;
    pub const PAPER_CRUSH_ANIM: u32 = 0x8015_2738;
    pub const PAPER_CRUSH_IASA: u32 = 0x8015_2774;
    pub const PAPER_CRUSH_PHYS: u32 = 0x8015_27B8;
    pub const PAPER_CRUSH_COLL: u32 = 0x8015_287C;
    pub const MS_358_80152880: u32 = 0x8015_2880;
    pub const POKE1_ANIM: u32 = 0x8015_2928;
    pub const POKE2_ANIM: u32 = 0x8015_29D0;
    pub const POKE1_IASA: u32 = 0x8015_2A0C;
    pub const POKE1_PHYS: u32 = 0x8015_2A50;
    pub const POKE1_COLL: u32 = 0x8015_2BC8;
    pub const MS_359_80152BCC: u32 = 0x8015_2BCC;
    pub const FINGER_BEAM_START_ANIM: u32 = 0x8015_2C34;
    pub const FINGER_BEAM_START_IASA: u32 = 0x8015_2C70;
    pub const FINGER_BEAM_START_PHYS: u32 = 0x8015_2CB4;
    pub const FINGER_BEAM_START_COLL: u32 = 0x8015_2CD4;
    pub const MS_361_80152CD8: u32 = 0x8015_2CD8;
    pub const FINGER_BEAM_LOOP_ANIM: u32 = 0x8015_2D44;
    pub const FINGER_BEAM_LOOP_IASA: u32 = 0x8015_2DC0;
    pub const FINGER_BEAM_LOOP_PHYS: u32 = 0x8015_2E04;
    pub const FINGER_BEAM_LOOP_COLL: u32 = 0x8015_2E24;
    pub const MS_362_80152E28: u32 = 0x8015_2E28;
    pub const MS_362_80152F80: u32 = 0x8015_2F80;
    pub const FINGER_BEAM_END_ANIM: u32 = 0x8015_3000;
    pub const FINGER_BEAM_END_IASA: u32 = 0x8015_303C;
    pub const FINGER_BEAM_END_PHYS: u32 = 0x8015_3080;
    pub const FINGER_BEAM_END_COLL: u32 = 0x8015_30A0;
    pub const MS_363_801530A4: u32 = 0x8015_30A4;
    pub const FINGER_GUN1_ANIM: u32 = 0x8015_3160;
    pub const FINGER_GUN1_IASA: u32 = 0x8015_3210;
    pub const FINGER_GUN1_PHYS: u32 = 0x8015_3254;
    pub const FINGER_GUN1_COLL: u32 = 0x8015_33C8;
    pub const MS_364_801533CC: u32 = 0x8015_33CC;
    pub const FINGER_GUN2_ANIM: u32 = 0x8015_346C;
    pub const FINGER_GUN2_IASA: u32 = 0x8015_3548;
    pub const FINGER_GUN2_PHYS: u32 = 0x8015_358C;
    pub const FINGER_GUN2_COLL: u32 = 0x8015_35AC;
    pub const MS_365_801535B0: u32 = 0x8015_35B0;
    pub const MS_365_8015364C: u32 = 0x8015_364C;
    pub const MS_365_80153730: u32 = 0x8015_3730;
    pub const FINGER_GUN3_ANIM: u32 = 0x8015_377C;
    pub const FINGER_GUN3_IASA: u32 = 0x8015_37B8;
    pub const FINGER_GUN3_PHYS: u32 = 0x8015_37FC;
    pub const FINGER_GUN3_COLL: u32 = 0x8015_381C;
    pub const MS_366_80153820: u32 = 0x8015_3820;
    pub const BACK_AIRPLANE1_ANIM: u32 = 0x8015_386C;
    pub const BACK_AIRPLANE1_IASA: u32 = 0x8015_38A8;
    pub const BACK_AIRPLANE1_PHYS: u32 = 0x8015_38EC;
    pub const BACK_AIRPLANE1_COLL: u32 = 0x8015_390C;
    pub const DO_ANIM: u32 = 0x8015_3910;
    pub const BACK_AIRPLANE2_ANIM: u32 = 0x8015_39A4;
    pub const BACK_AIRPLANE2_IASA: u32 = 0x8015_39EC;
    pub const BACK_AIRPLANE2_PHYS: u32 = 0x8015_3A30;
    pub const BACK_AIRPLANE2_COLL: u32 = 0x8015_3A60;
    pub const MS_368_80153A64: u32 = 0x8015_3A64;
    pub const BACK_AIRPLANE3_ANIM: u32 = 0x8015_3AEC;
    pub const BACK_AIRPLANE3_IASA: u32 = 0x8015_3B28;
    pub const BACK_AIRPLANE3_PHYS: u32 = 0x8015_3B6C;
    pub const BACK_AIRPLANE3_COLL: u32 = 0x8015_3B8C;
    pub const MS_369_80153B90: u32 = 0x8015_3B90;
    pub const BACK_PUNCH_ANIM: u32 = 0x8015_3C48;
    pub const BACK_PUNCH_IASA: u32 = 0x8015_3C90;
    pub const BACK_PUNCH_PHYS: u32 = 0x8015_3CD4;
    pub const BACK_PUNCH_COLL: u32 = 0x8015_3D28;
    pub const MS_370_80153D2C: u32 = 0x8015_3D2C;
    pub const BACK_CRUSH_ANIM: u32 = 0x8015_3F8C;
    pub const BACK_CRUSH_IASA: u32 = 0x8015_4114;
    pub const BACK_CRUSH_PHYS: u32 = 0x8015_4158;
    pub const BACK_CRUSH_COLL: u32 = 0x8015_41C4;
    pub const MS_371_801541C8: u32 = 0x8015_41C8;
    pub const BACK_DISAPPEAR_ANIM: u32 = 0x8015_4230;
    pub const BACK_DISAPPEAR_IASA: u32 = 0x8015_4278;
    pub const BACK_DISAPPEAR_PHYS: u32 = 0x8015_42BC;
    pub const BACK_DISAPPEAR_COLL: u32 = 0x8015_42DC;
    pub const MS_372_801542E0: u32 = 0x8015_42E0;
    pub const WAIT1_1_ANIM: u32 = 0x8015_4360;
    pub const WAIT1_1_IASA: u32 = 0x8015_43E8;
    pub const WAIT1_1_PHYS: u32 = 0x8015_442C;
    pub const WAIT1_1_COLL: u32 = 0x8015_459C;
    pub const MS_373_801545A0: u32 = 0x8015_45A0;
    pub const GRAB_ANIM: u32 = 0x8015_4620;
    pub const GRAB_IASA: u32 = 0x8015_4670;
    pub const GRAB_PHYS: u32 = 0x8015_46B4;
    pub const GRAB_COLL: u32 = 0x8015_46D4;
    pub const MS_374_801546D8: u32 = 0x8015_46D8;
    pub const FAIL_ANIM: u32 = 0x8015_4758;
    pub const FAIL_IASA: u32 = 0x8015_4794;
    pub const FAIL_PHYS: u32 = 0x8015_47D8;
    pub const FAIL_COLL: u32 = 0x8015_4838;
    pub const MS_381_8015483C: u32 = 0x8015_483C;
    pub const CANCEL_ANIM: u32 = 0x8015_48D8;
    pub const CANCEL_IASA: u32 = 0x8015_4964;
    pub const CANCEL_PHYS: u32 = 0x8015_49A8;
    pub const CANCEL_COLL: u32 = 0x8015_4A08;
    pub const MS_375_80154A2C: u32 = 0x8015_4A2C;
    pub const MS_378_80154A78: u32 = 0x8015_4A78;
    pub const SQUEEZE_ANIM: u32 = 0x8015_4B2C;
    pub const SQUEEZE_IASA: u32 = 0x8015_4BB0;
    pub const SQUEEZE_PHYS: u32 = 0x8015_4BF4;
    pub const SQUEEZE_COLL: u32 = 0x8015_4C54;
    pub const MS_375_80154C78: u32 = 0x8015_4C78;
    pub const MS_378_80154CF8: u32 = 0x8015_4CF8;
    pub const SQUEEZING_ANIM: u32 = 0x8015_4D78;
    pub const SQUEEZING_IASA: u32 = 0x8015_4DD0;
    pub const SQUEEZING_PHYS: u32 = 0x8015_4E14;
    pub const SQUEEZING_COLL: u32 = 0x8015_4E74;
    pub const MS_376_80154E78: u32 = 0x8015_4E78;
    pub const THROW_ANIM: u32 = 0x8015_4ED8;
    pub const THROW_IASA: u32 = 0x8015_4FAC;
    pub const THROW_PHYS: u32 = 0x8015_4FF0;
    pub const THROW_COLL: u32 = 0x8015_5010;
    pub const MS_379_80155014: u32 = 0x8015_5014;
    pub const SLAM_ANIM: u32 = 0x8015_5074;
    pub const SLAM_IASA: u32 = 0x8015_512C;
    pub const SLAM_PHYS: u32 = 0x8015_5170;
    pub const SLAM_COLL: u32 = 0x8015_5190;
    pub const MS_380_80155194: u32 = 0x8015_5194;
    pub const TAG_CRUSH_ANIM: u32 = 0x8015_521C;
    pub const TAG_CRUSH_IASA: u32 = 0x8015_5290;
    pub const TAG_CRUSH_PHYS: u32 = 0x8015_52D4;
    pub const TAG_CRUSH_COLL: u32 = 0x8015_52F4;
    pub const MS_382_801552F8: u32 = 0x8015_52F8;
    pub const TAG_APPLAUD_ANIM: u32 = 0x8015_5388;
    pub const TAG_APPLAUD_IASA: u32 = 0x8015_541C;
    pub const TAG_APPLAUD_PHYS: u32 = 0x8015_5460;
    pub const TAG_APPLAUD_COLL: u32 = 0x8015_5480;
    pub const MS_383_80155484: u32 = 0x8015_5484;
    pub const TAG_ROCK_PAPER_ANIM: u32 = 0x8015_550C;
    pub const TAG_ROCK_PAPER_IASA: u32 = 0x8015_5580;
    pub const TAG_ROCK_PAPER_PHYS: u32 = 0x8015_55C4;
    pub const TAG_ROCK_PAPER_COLL: u32 = 0x8015_55E4;
    pub const TAG_GRAB_ANIM: u32 = 0x8015_55E8;
    pub const TAG_GRAB_IASA: u32 = 0x8015_565C;
    pub const TAG_GRAB_PHYS: u32 = 0x8015_56A0;
    pub const TAG_GRAB_COLL: u32 = 0x8015_56C0;
    pub const TAG_SQUEEZE_ANIM: u32 = 0x8015_56C4;
    pub const TAG_SQUEEZE_IASA: u32 = 0x8015_57B0;
    pub const TAG_SQUEEZE_PHYS: u32 = 0x8015_57F4;
    pub const TAG_SQUEEZE_COLL: u32 = 0x8015_5814;
    pub const MS_386_80155818: u32 = 0x8015_5818;
    pub const TAG_FAIL_ANIM: u32 = 0x8015_5864;
    pub const TAG_FAIL_IASA: u32 = 0x8015_58A0;
    pub const TAG_FAIL_PHYS: u32 = 0x8015_58E4;
    pub const TAG_FAIL_COLL: u32 = 0x8015_5904;
    pub const TAG_CANCEL_ANIM: u32 = 0x8015_5908;
    pub const TAG_CANCEL_IASA: u32 = 0x8015_5990;
    pub const TAG_CANCEL_PHYS: u32 = 0x8015_59D4;
    pub const TAG_CANCEL_COLL: u32 = 0x8015_5A34;
    pub const MS_388_80155A58: u32 = 0x8015_5A58;
    pub const CAPTURE_MASTER_HAND_ANIM: u32 = 0x8015_5B70;
    pub const CAPTURE_MASTER_HAND_IASA: u32 = 0x8015_5B74;
    pub const CAPTURE_MASTER_HAND_PHYS: u32 = 0x8015_5B78;
    pub const CAPTURE_MASTER_HAND_COLL: u32 = 0x8015_5B7C;
    pub const CAPTURE_MASTER_HAND_80155B80: u32 = 0x8015_5B80;
    pub const CAPTURE_DAMAGE_MASTER_HAND_ANIM: u32 = 0x8015_5C20;
    pub const CAPTURE_DAMAGE_MASTER_HAND_IASA: u32 = 0x8015_5C88;
    pub const CAPTURE_DAMAGE_MASTER_HAND_PHYS: u32 = 0x8015_5C8C;
    pub const CAPTURE_DAMAGE_MASTER_HAND_COLL: u32 = 0x8015_5C90;
    pub const CAPTURE_DAMAGE_MASTER_HAND_80155C94: u32 = 0x8015_5C94;
    pub const CAPTURE_WAIT_MASTER_HAND_80155D1C: u32 = 0x8015_5D1C;
    pub const CAPTURE_WAIT_MASTER_HAND_ANIM: u32 = 0x8015_5D5C;
    pub const CAPTURE_WAIT_MASTER_HAND_IASA: u32 = 0x8015_5D60;
    pub const CAPTURE_WAIT_MASTER_HAND_PHYS: u32 = 0x8015_5D64;
    pub const CAPTURE_WAIT_MASTER_HAND_COLL: u32 = 0x8015_5D68;
    pub const CAPTURE_WAIT_MASTER_HAND_80155D6C: u32 = 0x8015_5D6C;
    pub const THROWN_MASTER_HAND_ANIM: u32 = 0x8015_5E08;
    pub const THROWN_MASTER_HAND_IASA: u32 = 0x8015_5E0C;
    pub const THROWN_MASTER_HAND_PHYS: u32 = 0x8015_5E10;
    pub const THROWN_MASTER_HAND_COLL: u32 = 0x8015_5E14;
}

/// His action states (`ftMh_MS_*`).
pub mod ms {
    pub const WAIT1_0: i32 = 341;
    pub const WAIT2_0: i32 = 342;
    pub const ENTRY: i32 = 343;
    pub const DAMAGE: i32 = 344;
    pub const DAMAGE2: i32 = 345;
    pub const WAIT_SWEEP: i32 = 346;
    pub const SWEEP_LOOP: i32 = 347;
    pub const SWEEP_WAIT: i32 = 348;
    pub const SLAP: i32 = 349;
    pub const WALK2: i32 = 350;
    pub const WALK_LOOP: i32 = 351;
    pub const WALK_WAIT: i32 = 352;
    pub const WALK_SHOOT: i32 = 353;
    pub const DRILL: i32 = 354;
    pub const ROCK_CRUSH_UP: i32 = 355;
    pub const ROCK_CRUSH_WAIT: i32 = 356;
    pub const ROCK_CRUSH_DOWN: i32 = 357;
    pub const PAPER_CRUSH: i32 = 358;
    pub const POKE1: i32 = 359;
    pub const POKE2: i32 = 360;
    pub const FINGER_BEAM_START: i32 = 361;
    pub const FINGER_BEAM_LOOP: i32 = 362;
    pub const FINGER_BEAM_END: i32 = 363;
    pub const FINGER_GUN1: i32 = 364;
    pub const FINGER_GUN2: i32 = 365;
    pub const FINGER_GUN3: i32 = 366;
    pub const BACK_AIRPLANE1: i32 = 367;
    pub const BACK_AIRPLANE2: i32 = 368;
    pub const BACK_AIRPLANE3: i32 = 369;
    pub const BACK_PUNCH: i32 = 370;
    pub const BACK_CRUSH: i32 = 371;
    pub const BACK_DISAPPEAR: i32 = 372;
    pub const WAIT1_1: i32 = 373;
    pub const GRAB: i32 = 374;
    pub const CANCEL: i32 = 375;
    pub const SQUEEZING0: i32 = 376;
    pub const SQUEEZING1: i32 = 377;
    pub const SQUEEZE: i32 = 378;
    pub const THROW: i32 = 379;
    pub const SLAM: i32 = 380;
    pub const FAIL: i32 = 381;
    pub const TAG_CRUSH: i32 = 382;
    pub const TAG_APPLAUD: i32 = 383;
    pub const TAG_ROCK_PAPER: i32 = 384;
    pub const TAG_GRAB: i32 = 385;
    pub const TAG_SQUEEZE: i32 = 386;
    pub const TAG_FAIL: i32 = 387;
    pub const TAG_CANCEL: i32 = 388;
    pub const WAIT1_2: i32 = 389;
    pub const WAIT2_1: i32 = 390;
}

/// `ftMasterhand_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// The nearest opponent when created (`x222C`).
    pub const TARGET: usize = 0x0;
    pub const X2230: usize = 0x4;
    pub const X2234: usize = 0x8;
    /// The attack timer's speed (`x2238`) and the frames to the next attack (`x223C`).
    pub const TIMER_SPEED: usize = 0xC;
    pub const TIMER: usize = 0x10;
    /// Where he waits (`x2240_pos`).
    pub const WAIT_POS: usize = 0x14;
    /// The attack table's row (`x224C`), the next attack (`x2250`) and the attacks since the
    /// last walk (`x2254`).
    pub const ROW: usize = 0x20;
    pub const NEXT_ATTACK: usize = 0x24;
    pub const SINCE_WALK: usize = 0x28;
    /// Which wait he's in (`x2258`).
    pub const WAIT: usize = 0x2C;
}

/// `ftMasterHand_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    pub const X0: usize = 0x0;
    /// The callback once he's reached `TARGET` (`unk0.x4`), or the entrance's camera step
    /// (`unk4.x0`).
    pub const X4: usize = 0x4;
    pub const X8: usize = 0x8;
    /// Where he's gliding (`unk0.xC`) and how far it is (`x18`).
    pub const TARGET: usize = 0xC;
    pub const DISTANCE: usize = 0x18;
    pub const X1C: usize = 0x1C;
    /// Whether he holds a fighter (`unk0.x20`).
    pub const HOLDING: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X34: usize = 0x34;
    pub const X38: usize = 0x38;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X54: usize = 0x54;
    pub const X58: usize = 0x58;
    pub const X64: usize = 0x64;
    pub const X70: usize = 0x70;
    pub const X74: usize = 0x74;
    pub const X78: usize = 0x78;
}

/// `ftMasterHand_SpecialAttrs`' offsets (0x17C bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0x17C;
    pub const X0: usize = 0x0;
    pub const X4: usize = 0x4;
    pub const X8: usize = 0x8;
    pub const XC: usize = 0xC;
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X38: usize = 0x38;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X58: usize = 0x58;
    pub const X5C: usize = 0x5C;
    pub const X60: usize = 0x60;
    pub const X64: usize = 0x64;
    pub const X68: usize = 0x68;
    pub const X6C: usize = 0x6C;
    pub const X70: usize = 0x70;
    pub const X74: usize = 0x74;
    pub const X78: usize = 0x78;
    pub const X7C: usize = 0x7C;
    pub const X80: usize = 0x80;
    pub const X84: usize = 0x84;
    pub const X88: usize = 0x88;
    pub const X90: usize = 0x90;
    pub const X94: usize = 0x94;
    pub const X98: usize = 0x98;
    pub const X9C: usize = 0x9C;
    pub const XA0: usize = 0xA0;
    pub const XA4: usize = 0xA4;
    pub const XA8: usize = 0xA8;
    pub const XB0: usize = 0xB0;
    pub const XB4: usize = 0xB4;
    pub const XB8: usize = 0xB8;
    pub const XBC: usize = 0xBC;
    pub const XC4: usize = 0xC4;
    pub const XCC: usize = 0xCC;
    pub const XD4: usize = 0xD4;
    pub const XD8: usize = 0xD8;
    pub const XDC: usize = 0xDC;
    pub const XE0: usize = 0xE0;
    pub const XE4: usize = 0xE4;
    pub const XE8: usize = 0xE8;
    pub const XEC: usize = 0xEC;
    pub const XF0: usize = 0xF0;
    pub const XF4: usize = 0xF4;
    pub const XF8: usize = 0xF8;
    pub const XFC: usize = 0xFC;
    pub const X100: usize = 0x100;
    pub const X104: usize = 0x104;
    pub const X108: usize = 0x108;
    pub const X10C: usize = 0x10C;
    pub const X110: usize = 0x110;
    pub const X118: usize = 0x118;
    pub const X120: usize = 0x120;
    pub const X124: usize = 0x124;
    pub const X12C: usize = 0x12C;
    pub const X134: usize = 0x134;
    pub const X13C: usize = 0x13C;
    pub const X144: usize = 0x144;
    pub const X148: usize = 0x148;
    pub const X14C: usize = 0x14C;
    pub const X150: usize = 0x150;
    pub const X154: usize = 0x154;
    pub const X158: usize = 0x158;
    pub const X15C: usize = 0x15C;
    pub const X160: usize = 0x160;
    pub const X164: usize = 0x164;
    pub const X168: usize = 0x168;
    pub const X16C: usize = 0x16C;
    pub const X170: usize = 0x170;
    pub const X174: usize = 0x174;
    pub const X178: usize = 0x178;
    /// The camera's distance in front of a player (`x178`).
    pub const CAMERA_Z: usize = 0x178;
}

impl Running<'_> {
    /// A float of his attributes (`ftData::ext_attr`), `off` bytes in.
    pub(crate) fn mh_da_f32(&self, f: FighterId, off: usize) -> f32 {
        let b = self.ext_attrs(f, attr::SIZE);
        f32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// A word of his attributes.
    pub(crate) fn mh_da_i32(&self, f: FighterId, off: usize) -> i32 {
        let b = self.ext_attrs(f, attr::SIZE);
        i32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// A point of his attributes (`Vec2`, z 0).
    pub(crate) fn mh_da_vec2(&self, f: FighterId, off: usize) -> crate::vec::Vec3 {
        crate::vec::Vec3::new(self.mh_da_f32(f, off), self.mh_da_f32(f, off + 4), 0.0)
    }
}

/// Runs one of Master Hand's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr`
/// is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    common::run(g, gobj, addr)
        || init::run(g, gobj, addr)
        || wait::run(g, gobj, addr)
        || entry::run(g, gobj, addr)
        || sweep::run(g, gobj, addr)
        || crush::run(g, gobj, addr)
        || airplane::run(g, gobj, addr)
        || grab::run(g, gobj, addr)
        || finger::run(g, gobj, addr)
        || damage::run(g, gobj, addr)
}

/// Runs one of his functions that takes his GObj and a callback (`ftMh_MS_371_801541C8`),
/// if `addr` is one.
pub fn run_with(g: &mut Running, gobj: GObjId, addr: u32, cb: u32) -> bool {
    match addr {
        addr::MS_371_801541C8 => g.mh_back_disappear_enter(gobj, cb),
        _ => return false,
    }
    true
}

/// Master Hand's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
