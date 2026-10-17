//! Kirby's own code (`ft/kinds/ftKirby`): his creation and copies of other characters' hats
//! and abilities (`ftkirby.c`), the dash attack, Final Cutter (up special), Stone (down
//! special), the hammer (side special), Inhale with its swallow and spit (neutral special),
//! and the copied neutral specials reached so far. His five double jumps are the common
//! multi-jump states (`ft::co::multijump`); his victims' states are in `ft::co::capture_kirby`
//! and his items in `it::kinds`.
//!
//! A state's variables live in the fighter's `mv` union (`ftKb_MotionVars`), the character's
//! in `u` (`ftKb_FighterVars`), and the character's attributes (`ftKb_DatAttrs`, the
//! multi-jump attributes first) in the fighter's attribute block, all read at the decomp's
//! offsets.
//!
//! Other code reaches Kirby's through [`call`] and [`run`] by the original's addresses.

pub mod attack_dash;
pub mod copy;
pub mod hat;
pub mod init;
pub mod special_captain;
pub mod special_fox;
pub mod special_gamewatch;
pub mod special_hi;
pub mod special_luigi;
pub mod special_lw;
pub mod special_mario;
pub mod special_n;
pub mod special_ness;
pub mod special_peach;
pub mod special_pikachu;
pub mod special_s;
pub mod special_samus;
pub mod special_seak;
pub mod special_zelda;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_KIRBY`.
pub const KIND_KIRBY: i32 = 4;

/// `It_Kind_Kirby_CBeam`, `_Hammer`, `It_Kind_Unk1` and `_Unk2`: his articles in his
/// `ftData`'s order.
pub const ITEMS: [i32; 4] = [50, 51, 52, 53];

/// Kirby's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x800E_E5C0;
    pub const ON_LOAD: u32 = 0x800E_E680;
    pub const DEATH3: u32 = 0x800E_E74C;
    pub const DEATH1: u32 = 0x800E_E7B8;
    pub const UNK_MOTION_STATES_4: u32 = 0x800E_E818;
    pub const ON_ITEM_PICKUP: u32 = 0x800E_E914;
    pub const ON_ITEM_INVISIBLE: u32 = 0x800E_E9DC;
    pub const ON_ITEM_VISIBLE: u32 = 0x800E_EA24;
    pub const ON_ITEM_DROP: u32 = 0x800E_EA6C;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x800E_EAC0;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x800E_EB38;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x800E_EB7C;
    pub const LOAD_COPY: u32 = 0x800E_ED50;
    pub const SPECIAL_N_ENTER: u32 = 0x800F_1630;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x800F_1680;
    pub const COPY: u32 = 0x800F_1BAC;
    pub const N_SPIT_VEL: u32 = 0x800F_58AC;
    pub const N_SWALLOW_VEL: u32 = 0x800F_58D8;
    pub const N_SKIP_FLOOR: u32 = 0x800F_5954;
    pub const N_ON_PLATFORM: u32 = 0x800F_597C;
    pub const N_VICTIM_HOP: u32 = 0x800F_598C;
    pub const N_VICTIM_PUSH: u32 = 0x800F_5A04;
    pub const N_VICTIM_CAN_MOVE: u32 = 0x800F_5A38;
    pub const N_VICTIM_FACING: u32 = 0x800F_5A60;
    pub const N_THROWN_SHRINK: u32 = 0x800F_5A70;
    pub const N_SPIT_STAR_TIME: u32 = 0x800F_5A88;
    pub const N_THROWN_SPIN: u32 = 0x800F_5A98;
    pub const N_SWALLOW_STAR_TIME: u32 = 0x800F_5AB0;
    pub const N_STAR_DIVISOR: u32 = 0x800F_5AC0;
    pub const N_STAR_DECEL: u32 = 0x800F_5AD8;
    pub const N_HOLD_TIME: u32 = 0x800F_5AF0;
    pub const N_HOLD_DIVISOR: u32 = 0x800F_5B00;
    pub const N_HOLD_RESISTANCE: u32 = 0x800F_5B10;
    pub const N_PULL_SPEEDS: u32 = 0x800F_5B20;
    pub const N_SHRINK: u32 = 0x800F_5B3C;
    pub const N_PULL_RANGE: u32 = 0x800F_5B4C;
    pub const N_MOUTH: u32 = 0x800F_5B5C;
    pub const MAYBE_LOSE_COPY: u32 = 0x800F_5BA4;
    pub const MAYBE_LOSE_COPY_NO_KB: u32 = 0x800F_5C34;
    pub const LOSE_COPY: u32 = 0x800F_5D04;
    pub const N_PULL_ITEM: u32 = 0x800F_5DE8;
    pub const N_PULL_FIGHTER: u32 = 0x800F_5EA8;
    pub const N_ENTER: u32 = 0x800F_5F68;
    pub const N_AIR_ENTER: u32 = 0x800F_6070;
    pub const N_CATCH_ITEM: u32 = 0x800F_6178;
    pub const N_CATCH: u32 = 0x800F_6210;
    pub const N_AIR_CATCH_ITEM: u32 = 0x800F_6280;
    pub const N_AIR_CATCH: u32 = 0x800F_6318;
    pub const N_EAT: u32 = 0x800F_6388;
    pub const N_EAT_AIR: u32 = 0x800F_63EC;
    pub const N_EAT_FALL: u32 = 0x800F_6450;
    pub const N_EAT_WALK: u32 = 0x800F_64C8;
    pub const N_EAT_LAND: u32 = 0x800F_6528;
    pub const N_TO_AIR: u32 = 0x800F_6588;
    pub const N_TO_GROUND: u32 = 0x800F_6638;
    pub const N_LOOP_TO_AIR: u32 = 0x800F_66E8;
    pub const N_LOOP_TO_GROUND: u32 = 0x800F_6798;
    pub const N_END_TO_AIR: u32 = 0x800F_6848;
    pub const N_END_TO_GROUND: u32 = 0x800F_68A8;
    pub const N_CAPTURE_ITEM_TO_AIR: u32 = 0x800F_6908;
    pub const N_CAPTURE_TO_AIR: u32 = 0x800F_697C;
    pub const N_CAPTURE_ITEM_TO_GROUND: u32 = 0x800F_69E8;
    pub const N_CAPTURE_TO_GROUND: u32 = 0x800F_6A5C;
    pub const N_EAT_TO_AIR: u32 = 0x800F_6AC8;
    pub const N_EAT_TO_GROUND: u32 = 0x800F_6B3C;
    pub const N_EAT_WAIT_TO_AIR: u32 = 0x800F_6BB0;
    pub const N_SPIT_ITEM_TO_AIR: u32 = 0x800F_6C24;
    pub const N_SPIT_TO_AIR: u32 = 0x800F_6C98;
    pub const N_SPIT_ITEM_TO_GROUND: u32 = 0x800F_6D0C;
    pub const N_SPIT_TO_GROUND: u32 = 0x800F_6D80;
    pub const N_DRINK_ITEM_TO_AIR: u32 = 0x800F_6DF4;
    pub const N_DRINK_TO_AIR: u32 = 0x800F_6E68;
    pub const N_DRINK_ITEM_TO_GROUND: u32 = 0x800F_6EDC;
    pub const N_DRINK_TO_GROUND: u32 = 0x800F_6F50;
    pub const N_EAT_TURN_TO_AIR: u32 = 0x800F_6FC4;
    pub const N_EAT_TURN_TO_GROUND: u32 = 0x800F_702C;
    pub const SPECIAL_N_ANIM: u32 = 0x800F_70A0;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x800F_71CC;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x800F_72F8;
    pub const SPECIAL_AIR_N_LOOP_ANIM: u32 = 0x800F_72FC;
    pub const SPECIAL_N_END_ANIM: u32 = 0x800F_7300;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x800F_733C;
    pub const SPECIAL_N_CAPTURE0_ANIM: u32 = 0x800F_7378;
    pub const SPECIAL_N_CAPTURE_ANIM: u32 = 0x800F_7498;
    pub const SPECIAL_N_CAPTURE1_ANIM: u32 = 0x800F_75B8;
    pub const SPECIAL_AIR_N_CAPTURE_ANIM: u32 = 0x800F_75D8;
    pub const EAT_ANIM: u32 = 0x800F_75F8;
    pub const SPECIAL_AIR_N_CAPTURED_ANIM: u32 = 0x800F_7674;
    pub const EAT_WAIT_ANIM: u32 = 0x800F_76F8;
    pub const SPECIAL_AIR_N_CAPTURE_WAIT_ANIM: u32 = 0x800F_76FC;
    pub const SPECIAL_N_SPIT0_ANIM: u32 = 0x800F_7700;
    pub const SPECIAL_N_SPIT_ANIM: u32 = 0x800F_7808;
    pub const SPECIAL_N_SPIT1_ANIM: u32 = 0x800F_78B8;
    pub const SPECIAL_AIR_N_SPIT_ANIM: u32 = 0x800F_79C0;
    pub const SPECIAL_N_DRINK0_ANIM: u32 = 0x800F_7A70;
    pub const SPECIAL_N_DRINK_ANIM: u32 = 0x800F_7B20;
    pub const SPECIAL_N_DRINK1_ANIM: u32 = 0x800F_7BE4;
    pub const SPECIAL_AIR_N_DRINK_ANIM: u32 = 0x800F_7C94;
    pub const EAT_TURN_ANIM: u32 = 0x800F_7D58;
    pub const SPECIAL_AIR_N_CAPTURE_TURN_ANIM: u32 = 0x800F_7DE4;
    pub const EAT_WALK_ANIM: u32 = 0x800F_7E78;
    pub const EAT_JUMP1_ANIM: u32 = 0x800F_7E98;
    pub const EAT_JUMP2_ANIM: u32 = 0x800F_7F20;
    pub const EAT_LANDING_ANIM: u32 = 0x800F_7F24;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x800F_7FA0;
    pub const SPECIAL_AIR_N_LOOP_IASA: u32 = 0x800F_8000;
    pub const EAT_WAIT_IASA: u32 = 0x800F_8060;
    pub const SPECIAL_AIR_N_CAPTURE_WAIT_IASA: u32 = 0x800F_844C;
    pub const EAT_WALK_IASA: u32 = 0x800F_8770;
    pub const EAT_JUMP1_IASA: u32 = 0x800F_8870;
    pub const SPECIAL_N_PHYS: u32 = 0x800F_8890;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x800F_88B0;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x800F_88D0;
    pub const SPECIAL_AIR_N_LOOP_PHYS: u32 = 0x800F_88F0;
    pub const SPECIAL_N_END_PHYS: u32 = 0x800F_8910;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x800F_8930;
    pub const SPECIAL_N_CAPTURE0_PHYS: u32 = 0x800F_8950;
    pub const SPECIAL_N_CAPTURE_PHYS: u32 = 0x800F_8970;
    pub const SPECIAL_N_CAPTURE1_PHYS: u32 = 0x800F_8990;
    pub const SPECIAL_AIR_N_CAPTURE_PHYS: u32 = 0x800F_89B0;
    pub const EAT_PHYS: u32 = 0x800F_89D0;
    pub const SPECIAL_AIR_N_CAPTURED_PHYS: u32 = 0x800F_89F0;
    pub const EAT_WAIT_PHYS: u32 = 0x800F_8A10;
    pub const SPECIAL_AIR_N_CAPTURE_WAIT_PHYS: u32 = 0x800F_8A30;
    pub const SPECIAL_N_SPIT0_PHYS: u32 = 0x800F_8A50;
    pub const SPECIAL_N_SPIT_PHYS: u32 = 0x800F_8A70;
    pub const SPECIAL_N_SPIT1_PHYS: u32 = 0x800F_8A90;
    pub const SPECIAL_AIR_N_SPIT_PHYS: u32 = 0x800F_8AB0;
    pub const SPECIAL_N_DRINK0_PHYS: u32 = 0x800F_8AD0;
    pub const SPECIAL_N_DRINK_PHYS: u32 = 0x800F_8AF0;
    pub const SPECIAL_N_DRINK1_PHYS: u32 = 0x800F_8B10;
    pub const SPECIAL_AIR_N_DRINK_PHYS: u32 = 0x800F_8B30;
    pub const EAT_TURN_PHYS: u32 = 0x800F_8B50;
    pub const SPECIAL_AIR_N_CAPTURE_TURN_PHYS: u32 = 0x800F_8B70;
    pub const EAT_WALK_PHYS: u32 = 0x800F_8B90;
    pub const EAT_JUMP1_PHYS: u32 = 0x800F_8BB0;
    pub const EAT_JUMP2_PHYS: u32 = 0x800F_8BD0;
    pub const EAT_LANDING_PHYS: u32 = 0x800F_8BF0;
    pub const SPECIAL_N_COLL: u32 = 0x800F_8C10;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x800F_8C38;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x800F_8C60;
    pub const SPECIAL_AIR_N_LOOP_COLL: u32 = 0x800F_8C88;
    pub const SPECIAL_N_END_COLL: u32 = 0x800F_8CB0;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x800F_8CD8;
    pub const SPECIAL_N_CAPTURE0_COLL: u32 = 0x800F_8D00;
    pub const SPECIAL_N_CAPTURE_COLL: u32 = 0x800F_8D28;
    pub const SPECIAL_N_CAPTURE1_COLL: u32 = 0x800F_8D50;
    pub const SPECIAL_AIR_N_CAPTURE_COLL: u32 = 0x800F_8D78;
    pub const EAT_COLL: u32 = 0x800F_8DA0;
    pub const SPECIAL_AIR_N_CAPTURED_COLL: u32 = 0x800F_8DC8;
    pub const EAT_WAIT_COLL: u32 = 0x800F_8DF0;
    pub const SPECIAL_AIR_N_CAPTURE_WAIT_COLL: u32 = 0x800F_8E18;
    pub const SPECIAL_N_SPIT0_COLL: u32 = 0x800F_8E40;
    pub const SPECIAL_N_SPIT_COLL: u32 = 0x800F_8E68;
    pub const SPECIAL_N_SPIT1_COLL: u32 = 0x800F_8E90;
    pub const SPECIAL_AIR_N_SPIT_COLL: u32 = 0x800F_8EB8;
    pub const SPECIAL_N_DRINK0_COLL: u32 = 0x800F_8EE0;
    pub const SPECIAL_N_DRINK_COLL: u32 = 0x800F_8F08;
    pub const SPECIAL_N_DRINK1_COLL: u32 = 0x800F_8F30;
    pub const SPECIAL_AIR_N_DRINK_COLL: u32 = 0x800F_8F58;
    pub const EAT_TURN_COLL: u32 = 0x800F_8F80;
    pub const SPECIAL_AIR_N_CAPTURE_TURN_COLL: u32 = 0x800F_8FA8;
    pub const EAT_WALK_COLL: u32 = 0x800F_8FD0;
    pub const EAT_JUMP1_COLL: u32 = 0x800F_8FF8;
    pub const EAT_JUMP2_COLL: u32 = 0x800F_9020;
    pub const EAT_LANDING_COLL: u32 = 0x800F_9048;
    pub const SET_DEATH_CBS: u32 = 0x800F_9070;
    pub const RELEASE_ITEM: u32 = 0x800F_9090;
    pub const LW_SHOW_STONE: u32 = 0x800F_331C;
    pub const LW_SCRIPT: u32 = 0x800F_346C;
    pub const LW_FLOOR: u32 = 0x800F_3570;
    pub const LW_CLEAR_SLOPES: u32 = 0x800F_36DC;
    pub const LW_SLOPES: u32 = 0x800F_37EC;
    pub const LW_SLIDE: u32 = 0x800F_3B28;
    pub const SPECIAL_LW_ENTER: u32 = 0x800F_3D50;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x800F_3E68;
    pub const SPECIAL_LW1_ANIM: u32 = 0x800F_3F80;
    pub const SPECIAL_LW_ANIM: u32 = 0x800F_402C;
    pub const SPECIAL_LW_END_ANIM: u32 = 0x800F_4074;
    pub const SPECIAL_AIR_LW_START_ANIM: u32 = 0x800F_412C;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x800F_41D8;
    pub const SPECIAL_AIR_LW_END_ANIM: u32 = 0x800F_4220;
    pub const SPECIAL_LW1_IASA: u32 = 0x800F_42D8;
    pub const SPECIAL_LW_IASA: u32 = 0x800F_42DC;
    pub const SPECIAL_LW_END_IASA: u32 = 0x800F_43FC;
    pub const SPECIAL_AIR_LW_START_IASA: u32 = 0x800F_4400;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x800F_4404;
    pub const SPECIAL_AIR_LW_END_IASA: u32 = 0x800F_4518;
    pub const SPECIAL_LW1_PHYS: u32 = 0x800F_451C;
    pub const SPECIAL_LW_PHYS: u32 = 0x800F_453C;
    pub const SPECIAL_LW_END_PHYS: u32 = 0x800F_4584;
    pub const SPECIAL_AIR_LW_START_PHYS: u32 = 0x800F_45A4;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x800F_45C4;
    pub const SPECIAL_AIR_LW_END_PHYS: u32 = 0x800F_4608;
    pub const SPECIAL_LW1_COLL: u32 = 0x800F_4628;
    pub const SPECIAL_LW_COLL: u32 = 0x800F_4800;
    pub const SPECIAL_LW_END_COLL: u32 = 0x800F_49E4;
    pub const SPECIAL_AIR_LW_START_COLL: u32 = 0x800F_4B3C;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x800F_4E78;
    pub const SPECIAL_AIR_LW_END_COLL: u32 = 0x800F_51C0;
    pub const STONE_CLEANUP: u32 = 0x800F_5318;
    pub const FORGET_HAMMER: u32 = 0x800F_539C;
    pub const S_HAMMER: u32 = 0x800F_53AC;
    pub const REMOVE_HAMMER: u32 = 0x800F_5524;
    pub const SPECIAL_S_ENTER: u32 = 0x800F_5564;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x800F_55D0;
    pub const SPECIAL_S_ANIM: u32 = 0x800F_5660;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x800F_569C;
    pub const SPECIAL_S_PHYS: u32 = 0x800F_56D8;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x800F_56F8;
    pub const SPECIAL_S_COLL: u32 = 0x800F_5718;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x800F_5778;
    pub const ATTACK_DASH_ENTER: u32 = 0x800F_1F68;
    pub const ATTACK_DASH_TO_GROUND: u32 = 0x800F_1FDC;
    pub const ATTACK_DASH_ANIM: u32 = 0x800F_203C;
    pub const ATTACK_DASH_IASA: u32 = 0x800F_205C;
    pub const ATTACK_DASH_PHYS: u32 = 0x800F_207C;
    pub const ATTACK_DASH_COLL: u32 = 0x800F_209C;
    pub const ATTACK_DASH_TO_AIR: u32 = 0x800F_20C4;
    pub const ATTACK_DASH_AIR_ANIM: u32 = 0x800F_2124;
    pub const ATTACK_DASH_AIR_IASA: u32 = 0x800F_2160;
    pub const ATTACK_DASH_AIR_PHYS: u32 = 0x800F_2164;
    pub const ATTACK_DASH_AIR_COLL: u32 = 0x800F_2198;
    pub const LEAN_ANGLE: u32 = 0x800F_21C0;
    pub const HI_SEND_WAVE: u32 = 0x800F_21E8;
    pub const CLEAR_LEAN: u32 = 0x800F_22D4;
    pub const SPECIAL_HI_ENTER: u32 = 0x800F_2300;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x800F_23B8;
    pub const SPECIAL_HI1_ANIM: u32 = 0x800F_2470;
    pub const SPECIAL_HI2_ANIM: u32 = 0x800F_24F8;
    pub const SPECIAL_HI3_ANIM: u32 = 0x800F_2578;
    pub const SPECIAL_HI4_ANIM: u32 = 0x800F_257C;
    pub const SPECIAL_AIR_HI1_ANIM: u32 = 0x800F_25EC;
    pub const SPECIAL_AIR_HI2_ANIM: u32 = 0x800F_266C;
    pub const SPECIAL_AIR_HI3_ANIM: u32 = 0x800F_26E4;
    pub const SPECIAL_AIR_HI_END_ANIM: u32 = 0x800F_26E8;
    pub const SPECIAL_HI1_IASA: u32 = 0x800F_2758;
    pub const SPECIAL_HI2_IASA: u32 = 0x800F_2828;
    pub const SPECIAL_HI3_IASA: u32 = 0x800F_282C;
    pub const SPECIAL_HI4_IASA: u32 = 0x800F_2830;
    pub const SPECIAL_AIR_HI1_IASA: u32 = 0x800F_2834;
    pub const SPECIAL_AIR_HI2_IASA: u32 = 0x800F_2904;
    pub const SPECIAL_AIR_HI3_IASA: u32 = 0x800F_2908;
    pub const SPECIAL_AIR_HI_END_IASA: u32 = 0x800F_290C;
    pub const SPECIAL_HI1_PHYS: u32 = 0x800F_2910;
    pub const SPECIAL_HI2_PHYS: u32 = 0x800F_2984;
    pub const SPECIAL_HI3_PHYS: u32 = 0x800F_29F8;
    pub const SPECIAL_HI4_PHYS: u32 = 0x800F_2A48;
    pub const SPECIAL_AIR_HI1_PHYS: u32 = 0x800F_2A68;
    pub const SPECIAL_AIR_HI2_PHYS: u32 = 0x800F_2AE4;
    pub const SPECIAL_AIR_HI3_PHYS: u32 = 0x800F_2B60;
    pub const SPECIAL_AIR_HI_END_PHYS: u32 = 0x800F_2B9C;
    pub const SPECIAL_HI1_COLL: u32 = 0x800F_2BBC;
    pub const SPECIAL_HI2_COLL: u32 = 0x800F_2C5C;
    pub const SPECIAL_HI3_COLL: u32 = 0x800F_2D98;
    pub const SPECIAL_HI4_COLL: u32 = 0x800F_2EA0;
    pub const SPECIAL_AIR_HI1_COLL: u32 = 0x800F_2F70;
    pub const SPECIAL_AIR_HI2_COLL: u32 = 0x800F_3004;
    pub const SPECIAL_AIR_HI3_COLL: u32 = 0x800F_3140;
    pub const SPECIAL_AIR_HI_END_COLL: u32 = 0x800F_3248;
}

/// Kirby's action states (`ftKb_MS_*`).
pub mod ms {
    pub const JUMP_AERIAL_F1: i32 = 341;
    pub const JUMP_AERIAL_F2: i32 = 342;
    pub const JUMP_AERIAL_F3: i32 = 343;
    pub const JUMP_AERIAL_F4: i32 = 344;
    pub const JUMP_AERIAL_F5: i32 = 345;
    pub const JUMP_AERIAL_F1_MET: i32 = 346;
    pub const JUMP_AERIAL_F2_MET: i32 = 347;
    pub const JUMP_AERIAL_F3_MET: i32 = 348;
    pub const JUMP_AERIAL_F4_MET: i32 = 349;
    pub const JUMP_AERIAL_F5_MET: i32 = 350;
    pub const ATTACK_DASH: i32 = 351;
    pub const ATTACK_DASH_AIR: i32 = 352;
    pub const SPECIAL_N: i32 = 353;
    pub const SPECIAL_N_LOOP: i32 = 354;
    pub const SPECIAL_N_END: i32 = 355;
    pub const SPECIAL_N_CAPTURE0: i32 = 356;
    pub const SPECIAL_N_CAPTURE1: i32 = 357;
    pub const EAT: i32 = 358;
    pub const EAT_WAIT: i32 = 359;
    pub const EAT_WALK_SLOW: i32 = 360;
    pub const EAT_WALK_MIDDLE: i32 = 361;
    pub const EAT_WALK_FAST: i32 = 362;
    pub const EAT_TURN: i32 = 363;
    pub const EAT_JUMP1: i32 = 364;
    pub const EAT_JUMP2: i32 = 365;
    pub const EAT_LANDING: i32 = 366;
    pub const SPECIAL_N_DRINK0: i32 = 367;
    pub const SPECIAL_N_DRINK1: i32 = 368;
    pub const SPECIAL_N_SPIT0: i32 = 369;
    pub const SPECIAL_N_SPIT1: i32 = 370;
    pub const SPECIAL_AIR_N: i32 = 371;
    pub const SPECIAL_AIR_N_LOOP: i32 = 372;
    pub const SPECIAL_AIR_N_END: i32 = 373;
    pub const SPECIAL_AIR_N_CAPTURE0: i32 = 374;
    pub const SPECIAL_AIR_N_CAPTURE1: i32 = 375;
    pub const EAT_AIR: i32 = 376;
    pub const EAT_FALL: i32 = 377;
    pub const SPECIAL_AIR_N_DRINK0: i32 = 378;
    pub const SPECIAL_AIR_N_DRINK1: i32 = 379;
    pub const SPECIAL_AIR_N_SPIT0: i32 = 380;
    pub const SPECIAL_AIR_N_SPIT1: i32 = 381;
    pub const EAT_TURN_AIR: i32 = 382;
    pub const SPECIAL_S: i32 = 383;
    pub const SPECIAL_AIR_S: i32 = 384;
    pub const SPECIAL_HI1: i32 = 385;
    pub const SPECIAL_HI2: i32 = 386;
    pub const SPECIAL_HI3: i32 = 387;
    pub const SPECIAL_HI4: i32 = 388;
    pub const SPECIAL_AIR_HI1: i32 = 389;
    pub const SPECIAL_AIR_HI2: i32 = 390;
    pub const SPECIAL_AIR_HI3: i32 = 391;
    pub const SPECIAL_AIR_HI4: i32 = 392;
    pub const SPECIAL_LW1: i32 = 393;
    pub const SPECIAL_LW: i32 = 394;
    pub const SPECIAL_LW_END: i32 = 395;
    pub const SPECIAL_AIR_LW_START: i32 = 396;
    pub const SPECIAL_AIR_LW: i32 = 397;
    pub const SPECIAL_AIR_LW_END: i32 = 398;
    pub const MR_SPECIAL_N: i32 = 399;
    pub const MR_SPECIAL_AIR_N: i32 = 400;
    pub const LK_SPECIAL_N_START: i32 = 401;
    pub const LK_SPECIAL_N_LOOP: i32 = 402;
    pub const LK_SPECIAL_N_END: i32 = 403;
    pub const LK_SPECIAL_AIR_N_START: i32 = 404;
    pub const LK_SPECIAL_AIR_N_LOOP: i32 = 405;
    pub const LK_SPECIAL_AIR_N_END: i32 = 406;
    pub const SS_SPECIAL_N_START: i32 = 407;
    pub const SS_SPECIAL_N_HOLD: i32 = 408;
    pub const SS_SPECIAL_N_CANCEL: i32 = 409;
    pub const SS_SPECIAL_N: i32 = 410;
    pub const SS_SPECIAL_AIR_N_START: i32 = 411;
    pub const SS_SPECIAL_AIR_N: i32 = 412;
    pub const YS_SPECIAL_N1: i32 = 413;
    pub const YS_SPECIAL_N_CAPTURE1_0: i32 = 414;
    pub const YS_SPECIAL_N_CAPTURE1_1: i32 = 415;
    pub const YS_SPECIAL_N_CAPTURE2_0: i32 = 416;
    pub const YS_SPECIAL_N_CAPTURE2_1: i32 = 417;
    pub const YS_SPECIAL_AIR_N_CAPTURE2: i32 = 418;
    pub const YS_SPECIAL_AIR_N_CAPTURE1_0: i32 = 419;
    pub const YS_SPECIAL_AIR_N_CAPTURE1_1: i32 = 420;
    pub const YS_SPECIAL_AIR_N2_0: i32 = 421;
    pub const YS_SPECIAL_AIR_N2_1: i32 = 422;
    pub const FX_SPECIAL_N_START: i32 = 423;
    pub const FX_SPECIAL_N_LOOP: i32 = 424;
    pub const FX_SPECIAL_N_END: i32 = 425;
    pub const FX_SPECIAL_AIR_N_START: i32 = 426;
    pub const FX_SPECIAL_AIR_N_LOOP: i32 = 427;
    pub const FX_SPECIAL_AIR_N_END: i32 = 428;
    pub const PK_SPECIAL_N: i32 = 429;
    pub const PK_SPECIAL_AIR_N: i32 = 430;
    pub const LG_SPECIAL_N: i32 = 431;
    pub const LG_SPECIAL_AIR_N: i32 = 432;
    pub const CA_SPECIAL_N: i32 = 433;
    pub const CA_SPECIAL_AIR_N: i32 = 434;
    pub const NS_SPECIAL_N_START: i32 = 435;
    pub const NS_SPECIAL_N_HOLD0: i32 = 436;
    pub const NS_SPECIAL_N_HOLD1: i32 = 437;
    pub const NS_SPECIAL_N_END: i32 = 438;
    pub const NS_SPECIAL_AIR_N_START: i32 = 439;
    pub const NS_SPECIAL_AIR_N_HOLD0: i32 = 440;
    pub const NS_SPECIAL_AIR_N_HOLD1: i32 = 441;
    pub const NS_SPECIAL_AIR_N_END: i32 = 442;
    pub const KP_SPECIAL_N_START: i32 = 443;
    pub const KP_SPECIAL_N: i32 = 444;
    pub const KP_SPECIAL_N_END: i32 = 445;
    pub const KP_SPECIAL_AIR_N_START: i32 = 446;
    pub const KP_SPECIAL_AIR_N: i32 = 447;
    pub const KP_SPECIAL_AIR_N_END: i32 = 448;
    pub const PE_SPECIAL_LW: i32 = 449;
    pub const PE_SPECIAL_LW_HIT: i32 = 450;
    pub const PE_SPECIAL_AIR_LW: i32 = 451;
    pub const PE_SPECIAL_AIR_LW_HIT: i32 = 452;
    pub const PP_SPECIAL_N: i32 = 453;
    pub const PP_SPECIAL_AIR_N: i32 = 454;
    pub const DK_SPECIAL_N_START: i32 = 455;
    pub const DK_SPECIAL_N_LOOP: i32 = 456;
    pub const DK_SPECIAL_N_CANCEL: i32 = 457;
    pub const DK_SPECIAL_N: i32 = 458;
    pub const DK_SPECIAL_N_FULL: i32 = 459;
    pub const DK_SPECIAL_AIR_N_START: i32 = 460;
    pub const DK_SPECIAL_AIR_N_LOOP: i32 = 461;
    pub const DK_SPECIAL_AIR_N_CANCEL: i32 = 462;
    pub const DK_SPECIAL_AIR_N: i32 = 463;
    pub const DK_SPECIAL_AIR_N_FULL: i32 = 464;
    pub const ZD_SPECIAL_N: i32 = 465;
    pub const ZD_SPECIAL_AIR_N: i32 = 466;
    pub const SK_SPECIAL_N_START: i32 = 467;
    pub const SK_SPECIAL_N_LOOP: i32 = 468;
    pub const SK_SPECIAL_N_CANCEL: i32 = 469;
    pub const SK_SPECIAL_N_END: i32 = 470;
    pub const SK_SPECIAL_AIR_N_START: i32 = 471;
    pub const SK_SPECIAL_AIR_N_LOOP: i32 = 472;
    pub const SK_SPECIAL_AIR_N_CANCEL: i32 = 473;
    pub const SK_SPECIAL_AIR_N_END: i32 = 474;
    pub const PR_SPECIAL_N_START_R: i32 = 475;
    pub const PR_SPECIAL_N_START_L: i32 = 476;
    pub const PR_SPECIAL_N_LOOP: i32 = 477;
    pub const PR_SPECIAL_N_FULL: i32 = 478;
    pub const PR_SPECIAL_N1: i32 = 479;
    pub const PR_SPECIAL_N_TURN: i32 = 480;
    pub const PR_SPECIAL_N_END_R: i32 = 481;
    pub const PR_SPECIAL_N_END_L: i32 = 482;
    pub const PR_SPECIAL_AIR_N_START_R: i32 = 483;
    pub const PR_SPECIAL_AIR_N_START_L: i32 = 484;
    pub const PR_SPECIAL_AIR_N_LOOP: i32 = 485;
    pub const PR_SPECIAL_AIR_N_FULL: i32 = 486;
    pub const PR_SPECIAL_AIR_N: i32 = 487;
    pub const PR_SPECIAL_N0: i32 = 488;
    pub const PR_SPECIAL_AIR_N_END_R0: i32 = 489;
    pub const PR_SPECIAL_AIR_N_END_R1: i32 = 490;
    pub const PR_SPECIAL_N_HIT: i32 = 491;
    pub const MS_SPECIAL_N_START: i32 = 492;
    pub const MS_SPECIAL_N_LOOP: i32 = 493;
    pub const MS_SPECIAL_N_END0: i32 = 494;
    pub const MS_SPECIAL_N_END1: i32 = 495;
    pub const MS_SPECIAL_AIR_N_START: i32 = 496;
    pub const MS_SPECIAL_AIR_N_LOOP: i32 = 497;
    pub const MS_SPECIAL_AIR_N_END0: i32 = 498;
    pub const MS_SPECIAL_AIR_N_END1: i32 = 499;
    pub const MT_SPECIAL_N_START: i32 = 500;
    pub const MT_SPECIAL_N_LOOP: i32 = 501;
    pub const MT_SPECIAL_N_LOOP_FULL: i32 = 502;
    pub const MT_SPECIAL_N_CANCEL: i32 = 503;
    pub const MT_SPECIAL_N_END: i32 = 504;
    pub const MT_SPECIAL_AIR_N_START: i32 = 505;
    pub const MT_SPECIAL_AIR_N_LOOP: i32 = 506;
    pub const MT_SPECIAL_AIR_N_LOOP_FULL: i32 = 507;
    pub const MT_SPECIAL_AIR_N_CANCEL: i32 = 508;
    pub const MT_SPECIAL_AIR_N_END: i32 = 509;
    pub const GW_SPECIAL_N: i32 = 510;
    pub const GW_SPECIAL_AIR_N: i32 = 511;
    pub const DR_SPECIAL_N: i32 = 512;
    pub const DR_SPECIAL_AIR_N: i32 = 513;
    pub const CL_SPECIAL_N_START: i32 = 514;
    pub const CL_SPECIAL_N_LOOP: i32 = 515;
    pub const CL_SPECIAL_N_END: i32 = 516;
    pub const CL_SPECIAL_AIR_N_START: i32 = 517;
    pub const CL_SPECIAL_AIR_N_LOOP: i32 = 518;
    pub const CL_SPECIAL_AIR_N_END: i32 = 519;
    pub const FC_SPECIAL_N_START: i32 = 520;
    pub const FC_SPECIAL_N_LOOP: i32 = 521;
    pub const FC_SPECIAL_N_END: i32 = 522;
    pub const FC_SPECIAL_AIR_N_START: i32 = 523;
    pub const FC_SPECIAL_AIR_N_LOOP: i32 = 524;
    pub const FC_SPECIAL_AIR_N_END: i32 = 525;
    pub const PC_SPECIAL_N: i32 = 526;
    pub const PC_SPECIAL_AIR_N: i32 = 527;
    pub const GN_SPECIAL_N: i32 = 528;
    pub const GN_SPECIAL_AIR_N: i32 = 529;
    pub const FE_SPECIAL_N_START: i32 = 530;
    pub const FE_SPECIAL_N_LOOP: i32 = 531;
    pub const FE_SPECIAL_N_END0: i32 = 532;
    pub const FE_SPECIAL_N_END1: i32 = 533;
    pub const FE_SPECIAL_AIR_N_START: i32 = 534;
    pub const FE_SPECIAL_AIR_N_LOOP: i32 = 535;
    pub const FE_SPECIAL_AIR_N_END0: i32 = 536;
    pub const FE_SPECIAL_AIR_N_END1: i32 = 537;
    pub const GK_SPECIAL_N_START: i32 = 538;
    pub const GK_SPECIAL_N: i32 = 539;
    pub const GK_SPECIAL_N_END: i32 = 540;
    pub const GK_SPECIAL_AIR_N_START: i32 = 541;
    pub const GK_SPECIAL_AIR_N: i32 = 542;
    pub const GK_SPECIAL_AIR_N_END: i32 = 543;
}

/// `ftKb_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// The copy's hat (`hat`): an item (`x0`), the copy-star's random number (`x4`), the
    /// player's flag (`x8_b0`), the copied kind (`kind`), its model (`jobj`), its DObj lists
    /// (`x14.data`, `x1C.data`) and its parts' visibility (`x24`).
    pub const HAT_ITEM: usize = 0x0;
    pub const HAT_X4: usize = 0x4;
    pub const HAT_FLAGS: usize = 0x8;
    pub const HAT_KIND: usize = 0xC;
    pub const HAT_JOBJ: usize = 0x10;
    pub const HAT_DOBJS_COUNT: usize = 0x14;
    pub const HAT_DOBJS: usize = 0x18;
    pub const HAT_DOBJS2_COUNT: usize = 0x1C;
    pub const HAT_DOBJS2: usize = 0x20;
    /// The frames left before his next wall or ceiling bump's effect (`x60`), and `x64`.
    pub const BUMP_TIMER: usize = 0x60;
    pub const X64: usize = 0x64;
    /// Mewtwo's copy: the ball's charge (`x9C`).
    pub const MT_CHARGE: usize = 0x9C;
    /// Samus's copy: the shot's charge (`xA8`).
    pub const SS_CHARGE: usize = 0xA8;
    /// Sheik's copy: the needles held (`xB4`).
    pub const SK_NEEDLES: usize = 0xB4;
    /// Donkey Kong's copy: the punch's swings (`xBC`).
    pub const DK_SWINGS: usize = 0xBC;
    /// Inhale: the kind he's about to copy (`xE0`), the loop's shortest frames left (`xE4`,
    /// an `s16`), his held walks' animation lengths (`xE8`, `xEC`, `xF0`, the hammer's fall,
    /// jump and landing), and whether he holds an item rather than a fighter (`xF4_b0`).
    pub const N_XE0: usize = 0xE0;
    pub const N_XE4: usize = 0xE4;
    pub const N_XE8: usize = 0xE8;
    pub const N_XEC: usize = 0xEC;
    pub const N_XF0: usize = 0xF0;
    pub const N_XF4: usize = 0xF4;
}

/// `ftKb_DatAttrs`' offsets (0x424 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0x424;
    /// The multi-jump attributes' `jumpaerial_unk` (an `s16`): the inhale loop's shortest
    /// frames.
    pub const JUMP_UNK: usize = 0x34;
    /// Inhale: his mouth's offset, the range a pulled victim shrinks within and its shrink,
    /// the pull's speeds, the swallow's reach, the hold's resistance, divisor and time, the
    /// stars' slowing, divisor and time, the swallowed star's time, the thrown victim's spin,
    /// the held moves' stick ranges, walk speed, jump height and the victim's push, the spat
    /// star's speed and slowing, the swallowed star's angle, speed and gravity, and the stars'
    /// collision box.
    pub const N_MOUTH_X: usize = 0x38;
    pub const N_MOUTH_Y: usize = 0x3C;
    pub const N_PULL_RANGE: usize = 0x40;
    pub const N_SHRINK: usize = 0x44;
    pub const N_PULL_OUTER: usize = 0x48;
    pub const N_PULL_INNER: usize = 0x4C;
    pub const N_INHALE_VEL: usize = 0x50;
    pub const N_HOLD_RESISTANCE: usize = 0x54;
    pub const N_HOLD_DIVISOR: usize = 0x58;
    pub const N_HOLD_TIME: usize = 0x5C;
    pub const N_STAR_DECEL: usize = 0x60;
    pub const N_STAR_DIVISOR: usize = 0x64;
    pub const N_STAR_TIME: usize = 0x68;
    pub const N_SWALLOW_STAR_TIME: usize = 0x6C;
    pub const N_SPIT_SPIN: usize = 0x70;
    pub const N_WALK_RANGE_X: usize = 0x74;
    pub const N_JUMP_RANGE_Y: usize = 0x78;
    pub const N_WALK_SPEED: usize = 0x7C;
    pub const N_JUMP_HEIGHT: usize = 0x80;
    pub const N_STOP_MOMENTUM: usize = 0x84;
    pub const N_SPIT_VEL: usize = 0x88;
    pub const N_SPIT_DECEL: usize = 0x8C;
    pub const N_SWALLOW_ANGLE: usize = 0x90;
    pub const N_SWALLOW_SPEED: usize = 0x94;
    pub const N_SWALLOW_GRAVITY: usize = 0x98;
    pub const N_BOX_TOP: usize = 0xB4;
    pub const N_BOX_BOTTOM: usize = 0xB8;
    pub const N_BOX_LEFT_X: usize = 0xBC;
    pub const N_BOX_LEFT_Y: usize = 0xC0;
    pub const N_BOX_RIGHT_X: usize = 0xC4;
    pub const N_BOX_RIGHT_Y: usize = 0xC8;
    /// Final Cutter: the rise's share kept each frame, the drift's share, the wave's offset
    /// and the stick's range that turns him around.
    pub const HI_RISE: usize = 0xD4;
    pub const HI_DRIFT: usize = 0xD8;
    pub const HI_WAVE_X: usize = 0xDC;
    pub const HI_WAVE_Y: usize = 0xE0;
    pub const HI_TURN_RANGE: usize = 0xE4;
    /// The hammer: its lift in the air and its landing lag.
    pub const S_AIR_LIFT: usize = 0xCC;
    pub const S_LANDING_LAG: usize = 0xD0;
    /// Stone: the longest and shortest times in it, the slopes he slides down from and the
    /// steepest he leans to, the slide's acceleration and top speed, his drop's speed, the
    /// hits' resistance, and the helpless fall's landing lag after it (0 for none).
    pub const LW_MAX_TIME: usize = 0xEC;
    pub const LW_MIN_TIME: usize = 0xF0;
    pub const LW_MIN_SLOPE: usize = 0xF4;
    pub const LW_MAX_SLOPE: usize = 0xF8;
    pub const LW_SLIDE_ACCEL: usize = 0xFC;
    pub const LW_SLIDE_MAX: usize = 0x100;
    pub const LW_GRAVITY: usize = 0x104;
    pub const LW_HP: usize = 0x108;
    pub const LW_FREEFALL: usize = 0x114;
    /// The star a hit knocks his copy out as: its height above him, its speed ahead and up.
    pub const LOSS_STAR_Y: usize = 0xA4;
    pub const LOSS_STAR_VEL_X: usize = 0xA8;
    pub const LOSS_STAR_VEL_Y: usize = 0xAC;
    /// The odds a hit knocks his copy out (one in this many).
    pub const LOSE_COPY_ODDS: usize = 0xB0;
    /// Bowser's copy: his full fuel and flame.
    pub const KP_MAX_FUEL: usize = 0x124;
    pub const KP_FLAME_SCALE: usize = 0x12C;
    /// Samus's copy: the shot's full charge (`specialn_ss_charge_time`).
    pub const SS_CHARGE_TIME: usize = 0x168;
    /// The aerial shot's recoil per level, the frames per level, and the helpless fall's
    /// landing lag after the aerial shot (none if 0).
    pub const SS_RECOIL: usize = 0x16C;
    pub const SS_FRAMES_PER_LEVEL: usize = 0x170;
    pub const SS_FREEFALL: usize = 0x174;
    /// Pikachu's and Pichu's copies: the jolt's offsets on the ground and in the air, the
    /// aerial throw's landing lag (none if 0), and the jolt's item kind.
    pub const PK_OFFSET_X: usize = 0x1C0;
    pub const PK_OFFSET_Y: usize = 0x1C4;
    pub const PK_AIR_OFFSET_X: usize = 0x1C8;
    pub const PK_AIR_OFFSET_Y: usize = 0x1CC;
    pub const PK_AIR_LANDING_LAG: usize = 0x1D0;
    pub const PK_ITKIND: usize = 0x1D4;
    pub const PC_OFFSET_X: usize = 0x1DC;
    pub const PC_OFFSET_Y: usize = 0x1E0;
    pub const PC_AIR_OFFSET_X: usize = 0x1E4;
    pub const PC_AIR_OFFSET_Y: usize = 0x1E8;
    pub const PC_AIR_LANDING_LAG: usize = 0x1EC;
    pub const PC_ITKIND: usize = 0x1F0;
    /// Captain Falcon's and Ganondorf's copies: the stick's range below which the aerial
    /// punch doesn't angle and its top, the angle at the top, the lunge's speed and its
    /// slowing.
    pub const CA_X_RANGE: usize = 0x1F8;
    pub const CA_Y_RANGE: usize = 0x1FC;
    pub const CA_ANGLE_DIFF: usize = 0x200;
    pub const CA_MOMENTUM: usize = 0x204;
    pub const CA_MOMENTUM_MUL: usize = 0x208;
    /// Donkey Kong's copy: the swings to a full punch.
    pub const DK_SWINGS_TO_FULL: usize = 0x190;
    /// Fox's and Falco's copies: the shot's angle and speed, the aerial end's landing lag (none
    /// if 0), and the shot's and the blaster's item kinds.
    pub const FX_ANGLE: usize = 0x230;
    pub const FX_SPEED: usize = 0x234;
    pub const FX_FREEFALL: usize = 0x238;
    pub const FX_SHOT_ITKIND: usize = 0x23C;
    pub const FX_GUN_ITKIND: usize = 0x240;
    pub const FC_ANGLE: usize = 0x254;
    pub const FC_SPEED: usize = 0x258;
    pub const FC_FREEFALL: usize = 0x25C;
    pub const FC_SHOT_ITKIND: usize = 0x260;
    pub const FC_GUN_ITKIND: usize = 0x264;
    /// Mewtwo's copy: the ball's full charge.
    pub const MT_CHARGE_TIME: usize = 0x384;
}

impl Running<'_> {
    /// A float of the fighter's `ftKb_DatAttrs`, `off` bytes in.
    pub(crate) fn kb_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `s16` of the fighter's `ftKb_DatAttrs`.
    pub(crate) fn kb_attr_i16(&self, f: FighterId, off: usize) -> i16 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        i16::from_be_bytes([b[off], b[off + 1]])
    }

    /// An `int` of the fighter's `ftKb_DatAttrs`.
    pub(crate) fn kb_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }
}

/// Runs one of Kirby's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || attack_dash::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_samus::run(g, gobj, addr)
        || special_fox::run(g, gobj, addr)
        || special_mario::run(g, gobj, addr)
        || special_luigi::run(g, gobj, addr)
        || special_captain::run(g, gobj, addr)
        || special_pikachu::run(g, gobj, addr)
        || special_ness::run(g, gobj, addr)
        || special_peach::run(g, gobj, addr)
        || special_zelda::run(g, gobj, addr)
        || special_seak::run(g, gobj, addr)
        || special_gamewatch::run(g, gobj, addr)
}

/// Kirby's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
