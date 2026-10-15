//! Mr. Game & Watch's own code (`ft/kinds/ftGameWatch`): his jab and rapid jab with the
//! sprayer (`ftgamewatchattack11.c`, `ftgamewatchattack100.c`), the down tilt's manhole, the
//! forward smash's torch, the aerials' parachute, turtle and Sparky, Chef (neutral special),
//! Judgement (side special), Fire (up special), Oil Panic (down special) and the character's
//! callbacks. Nearly every move holds an item of his own (`it::kinds::gamewatch*`), which the
//! move spawns from its accessory callback and which goes when he leaves the move.
//!
//! The character's variables live in the fighter's `u` union (`ftGameWatch_FighterVars`), a
//! state's in `mv` (`ftGameWatch_MotionVars`), and his attributes
//! (`ftGameWatchAttributes`) in the fighter's attribute block, all read at the decomp's
//! offsets.
//!
//! Other code reaches his through [`call`] and [`run`] by the original's addresses.

pub mod attack11;
pub mod attack_air;
pub mod attack_lw3;
pub mod attack_s4;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::{CodeRef, FighterId};
use super::super::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FTKIND_GAMEWATCH`.
pub const KIND_GAME_WATCH: i32 = 24;

/// His items, in his `ftData`'s order: `It_Kind_GameWatch_Greenhouse`, `_Manhole`, `_Fire`,
/// `_Parachute`, `_Turtle`, `_Breath`, `_Judge`, `_Panic`, `_Chef` and `_Rescue`.
pub const ITEMS: [i32; 10] = [114, 115, 116, 117, 118, 119, 120, 121, 122, 124];

/// The common parts his code reads straight from his parts table (`FtPart_*`).
pub(crate) mod part {
    pub const TOP_N: i32 = 0;
    pub const TRANS_N: i32 = 1;
    pub const L_SHOULDER_N: i32 = 17;
    pub const L_HAND_N: i32 = 21;
    pub const L_THUMB_NB: i32 = 31;
    pub const L_HAND_NB: i32 = 32;
    pub const R_THUMB_NB: i32 = 49;
}

/// His functions, by address: the callbacks his states and hooks hold and the entries other
/// code calls.
pub mod addr {
    pub const INIT_ON_DEATH: u32 = 0x8014_A250;
    pub const INIT_ON_LOAD: u32 = 0x8014_A37C;
    pub const INIT_ON_DAMAGE: u32 = 0x8014_A4CC;
    pub const INIT_8014A538: u32 = 0x8014_A538;
    pub const INIT_ON_ITEM_PICKUP: u32 = 0x8014_A5B8;
    pub const INIT_ON_ITEM_INVISIBLE: u32 = 0x8014_A698;
    pub const INIT_ON_ITEM_VISIBLE: u32 = 0x8014_A6E0;
    pub const INIT_ON_ITEM_DROP: u32 = 0x8014_A728;
    pub const INIT_UNK_MOTION_STATES4: u32 = 0x8014_A77C;
    pub const INIT_LOAD_SPECIAL_ATTRS: u32 = 0x8014_A7B4;
    pub const INIT_8014A7F4: u32 = 0x8014_A7F4;
    pub const INIT_8014A814: u32 = 0x8014_A814;
    pub const INIT_ON_ABSORB: u32 = 0x8014_A828;
    pub const ITEM_TORCH_SETUP: u32 = 0x8014_A848;
    pub const ATTACK_S4_ITEM_TORCH_SET_FLAG: u32 = 0x8014_A904;
    pub const ATTACK_S4_ITEM_TORCH_ON_DAMAGE: u32 = 0x8014_A938;
    pub const ATTACK_S4_ITEM_TORCH_ENTER_HITLAG: u32 = 0x8014_A990;
    pub const ATTACK_S4_ITEM_TORCH_EXIT_HITLAG: u32 = 0x8014_A9C0;
    pub const ATTACK_S4_ITEM_CHECK_TORCH_REMOVE: u32 = 0x8014_A9F0;
    pub const ATTACK_S4_ENTER: u32 = 0x8014_AA10;
    pub const ATTACK_S4_ANIM: u32 = 0x8014_AA88;
    pub const ATTACK_S4_IASA: u32 = 0x8014_AAC4;
    pub const ATTACK_S4_PHYS: u32 = 0x8014_AAF4;
    pub const ATTACK_S4_COLL: u32 = 0x8014_AB14;
    pub const ATTACK_LW3_ITEM_MANHOLE_SETUP: u32 = 0x8014_AB48;
    pub const ATTACK_LW3_ITEM_MANHOLE_REMOVE: u32 = 0x8014_AC40;
    pub const ATTACK_LW3_ITEM_MANHOLE_ON_DAMAGE: u32 = 0x8014_ACB0;
    pub const ATTACK_LW3_ITEM_MANHOLE_ENTER_HITLAG: u32 = 0x8014_AD38;
    pub const ATTACK_LW3_ITEM_MANHOLE_EXIT_HITLAG: u32 = 0x8014_AD68;
    pub const ATTACK_LW3_ITEM_CHECK_MANHOLE_REMOVE: u32 = 0x8014_AD98;
    pub const ATTACK_LW3_ENTER: u32 = 0x8014_ADB8;
    pub const ATTACK_LW3_ANIM: u32 = 0x8014_AE3C;
    pub const ATTACK_LW3_IASA: u32 = 0x8014_AE78;
    pub const ATTACK_LW3_PHYS: u32 = 0x8014_AF6C;
    pub const ATTACK_LW3_COLL: u32 = 0x8014_AF8C;
    pub const ATTACK_AIR_N_ITEM_PARACHUTE_SETUP: u32 = 0x8014_AFC0;
    pub const ATTACK_AIR_N_ITEM_PARACHUTE_ON_LAND: u32 = 0x8014_B074;
    pub const ATTACK_AIR_N_ITEM_PARACHUTE_SET_FLAG: u32 = 0x8014_B0F0;
    pub const ATTACK_AIR_N_ITEM_PARACHUTE_REMOVE: u32 = 0x8014_B12C;
    pub const ATTACK_AIR_N_ITEM_CHECK_PARACHUTE_REMOVE: u32 = 0x8014_B18C;
    pub const ATTACK_AIR_N_ITEM_TURTLE_SETUP: u32 = 0x8014_B1B4;
    pub const ATTACK_AIR_N_ITEM_TURTLE_ON_LAND: u32 = 0x8014_B268;
    pub const ATTACK_AIR_N_ITEM_TURTLE_SET_FLAG: u32 = 0x8014_B2E4;
    pub const ATTACK_AIR_N_ITEM_TURTLE_REMOVE: u32 = 0x8014_B320;
    pub const ATTACK_AIR_N_ITEM_CHECK_TURTLE_REMOVE: u32 = 0x8014_B380;
    pub const ATTACK_AIR_N_ITEM_SPARKY_SETUP: u32 = 0x8014_B3A8;
    pub const ATTACK_AIR_N_ITEM_SPARKY_ON_LAND: u32 = 0x8014_B45C;
    pub const ATTACK_AIR_N_ITEM_SPARKY_SET_FLAG: u32 = 0x8014_B4D8;
    pub const ATTACK_AIR_N_ITEM_SPARKY_REMOVE: u32 = 0x8014_B514;
    pub const ATTACK_AIR_N_ENTER_ITEM_HITLAG: u32 = 0x8014_B574;
    pub const ATTACK_AIR_N_EXIT_ITEM_HITLAG: u32 = 0x8014_B5CC;
    pub const ATTACK_AIR_N_ITEM_CHECK_SPARKY_REMOVE: u32 = 0x8014_B624;
    pub const ATTACK_AIR_N_DECIDE_ACTION: u32 = 0x8014_B64C;
    pub const ATTACK_AIR_N_ENTER: u32 = 0x8014_B6E4;
    pub const ATTACK_AIR_N_ANIM: u32 = 0x8014_B720;
    pub const ATTACK_AIR_N_IASA: u32 = 0x8014_B740;
    pub const ATTACK_AIR_N_PHYS: u32 = 0x8014_B760;
    pub const ATTACK_AIR_N_COLL: u32 = 0x8014_B780;
    pub const LANDING_AIR_N_ENTER: u32 = 0x8014_B7A8;
    pub const ATTACK_AIR_B_ENTER: u32 = 0x8014_B840;
    pub const ATTACK_AIR_B_ANIM: u32 = 0x8014_B87C;
    pub const ATTACK_AIR_B_IASA: u32 = 0x8014_B89C;
    pub const ATTACK_AIR_B_PHYS: u32 = 0x8014_B8BC;
    pub const ATTACK_AIR_B_COLL: u32 = 0x8014_B8DC;
    pub const LANDING_AIR_B_ENTER: u32 = 0x8014_B904;
    pub const ATTACK_AIR_HI_ENTER: u32 = 0x8014_B99C;
    pub const ATTACK_AIR_HI_ANIM: u32 = 0x8014_B9D8;
    pub const ATTACK_AIR_HI_IASA: u32 = 0x8014_B9F8;
    pub const ATTACK_AIR_HI_PHYS: u32 = 0x8014_BA18;
    pub const ATTACK_AIR_HI_COLL: u32 = 0x8014_BA38;
    pub const LANDING_AIR_HI_ENTER: u32 = 0x8014_BA60;
    pub const LANDING_AIR_N_INIT: u32 = 0x8014_BAF8;
    pub const LANDING_AIR_N_ANIM: u32 = 0x8014_BB24;
    pub const LANDING_AIR_N_IASA: u32 = 0x8014_BB70;
    pub const LANDING_AIR_N_PHYS: u32 = 0x8014_BB74;
    pub const LANDING_AIR_N_COLL: u32 = 0x8014_BB94;
    pub const LANDING_AIR_B_INIT: u32 = 0x8014_BBE0;
    pub const LANDING_AIR_B_ANIM: u32 = 0x8014_BC0C;
    pub const LANDING_AIR_B_IASA: u32 = 0x8014_BC58;
    pub const LANDING_AIR_B_PHYS: u32 = 0x8014_BC5C;
    pub const LANDING_AIR_B_COLL: u32 = 0x8014_BC7C;
    pub const LANDING_AIR_HI_INIT: u32 = 0x8014_BCC8;
    pub const LANDING_AIR_HI_ANIM: u32 = 0x8014_BCF4;
    pub const LANDING_AIR_HI_IASA: u32 = 0x8014_BD40;
    pub const LANDING_AIR_HI_PHYS: u32 = 0x8014_BD44;
    pub const LANDING_AIR_HI_COLL: u32 = 0x8014_BD64;
    pub const ATTACK11_ITEM_GREENHOUSE_SETUP: u32 = 0x8014_BDB0;
    pub const ATTACK11_DECIDE_ACTION: u32 = 0x8014_BE84;
    pub const ATTACK11_ITEM_GREENHOUSE_SET_FLAG: u32 = 0x8014_BF48;
    pub const ATTACK11_ITEM_GREENHOUSE_REMOVE: u32 = 0x8014_BF7C;
    pub const ATTACK11_ITEM_GREENHOUSE_ENTER_HITLAG: u32 = 0x8014_BFD4;
    pub const ATTACK11_ITEM_GREENHOUSE_EXIT_HITLAG: u32 = 0x8014_C004;
    pub const ATTACK11_ITEM_GREENHOUSE_CHECK_ALL: u32 = 0x8014_C034;
    pub const ATTACK11_ITEM_GREENHOUSE_CHECK_ATTACK11: u32 = 0x8014_C05C;
    pub const ATTACK11_ENTER: u32 = 0x8014_C07C;
    pub const ATTACK11_ANIM: u32 = 0x8014_C138;
    pub const ATTACK11_IASA: u32 = 0x8014_C174;
    pub const ATTACK11_PHYS: u32 = 0x8014_C194;
    pub const ATTACK11_COLL: u32 = 0x8014_C1B4;
    pub const ATTACK100_START_ENTER: u32 = 0x8014_C1E8;
    pub const ATTACK100_START_ANIM: u32 = 0x8014_C224;
    pub const ATTACK100_START_IASA: u32 = 0x8014_C260;
    pub const ATTACK100_START_PHYS: u32 = 0x8014_C264;
    pub const ATTACK100_START_COLL: u32 = 0x8014_C284;
    pub const ATTACK100_LOOP_ENTER: u32 = 0x8014_C2B8;
    pub const ATTACK100_LOOP_ANIM: u32 = 0x8014_C308;
    pub const ATTACK100_LOOP_IASA: u32 = 0x8014_C330;
    pub const ATTACK100_LOOP_PHYS: u32 = 0x8014_C350;
    pub const ATTACK100_LOOP_COLL: u32 = 0x8014_C370;
    pub const ATTACK100_END_ENTER: u32 = 0x8014_C3A4;
    pub const ATTACK100_END_ANIM: u32 = 0x8014_C3F4;
    pub const ATTACK100_END_IASA: u32 = 0x8014_C414;
    pub const ATTACK100_END_PHYS: u32 = 0x8014_C418;
    pub const ATTACK100_END_COLL: u32 = 0x8014_C438;
    pub const SPECIAL_S_ITEM_JUDGEMENT_SETUP: u32 = 0x8014_C46C;
    pub const SPECIAL_S_ITEM_JUDGEMENT_SET_FLAG: u32 = 0x8014_C590;
    pub const SPECIAL_S_ITEM_JUDGEMENT_REMOVE: u32 = 0x8014_C5CC;
    pub const SPECIAL_S_ITEM_JUDGEMENT_ENTER_HITLAG: u32 = 0x8014_C62C;
    pub const SPECIAL_S_ITEM_JUDGEMENT_EXIT_HITLAG: u32 = 0x8014_C65C;
    pub const SPECIAL_S_ITEM_CHECK_JUDGEMENT_REMOVE: u32 = 0x8014_C68C;
    pub const SPECIAL_S_GET_RANDOM_INT: u32 = 0x8014_C6B4;
    pub const SPECIAL_S_ENTER: u32 = 0x8014_C7A0;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8014_C828;
    pub const SPECIAL_S_ANIM: u32 = 0x8014_C8BC;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8014_C8F8;
    pub const SPECIAL_S_IASA: u32 = 0x8014_C934;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8014_C938;
    pub const SPECIAL_S_PHYS: u32 = 0x8014_C93C;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8014_C974;
    pub const SPECIAL_S_COLL: u32 = 0x8014_CA10;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8014_CA4C;
    pub const SPECIAL_S_GROUND_TO_AIR: u32 = 0x8014_CA88;
    pub const SPECIAL_AIR_S_AIR_TO_GROUND: u32 = 0x8014_CB44;
    pub const SPECIAL_LW_ITEM_PANIC_SETUP: u32 = 0x8014_CBF4;
    pub const SPECIAL_LW_ITEM_PANIC_SET_FLAG: u32 = 0x8014_CC9C;
    pub const SPECIAL_LW_ITEM_PANIC_REMOVE: u32 = 0x8014_CCD8;
    pub const SPECIAL_LW_ITEM_PANIC_ENTER_HITLAG: u32 = 0x8014_CD38;
    pub const SPECIAL_LW_ITEM_PANIC_EXIT_HITLAG: u32 = 0x8014_CD68;
    pub const SPECIAL_LW_ITEM_CHECK_PANIC_REMOVE: u32 = 0x8014_CD98;
    pub const SPECIAL_LW_UPDATE_BUCKET_MODEL: u32 = 0x8014_CDC0;
    pub const SPECIAL_LW_ENTER: u32 = 0x8014_CEF0;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8014_CF78;
    pub const SPECIAL_LW_ANIM: u32 = 0x8014_D014;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8014_D0E0;
    pub const SPECIAL_LW_IASA: u32 = 0x8014_D1AC;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8014_D264;
    pub const SPECIAL_LW_PHYS: u32 = 0x8014_D31C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8014_D350;
    pub const SPECIAL_LW_COLL: u32 = 0x8014_D3B4;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8014_D3F0;
    pub const SPECIAL_LW_GROUND_TO_AIR: u32 = 0x8014_D42C;
    pub const SPECIAL_AIR_LW_AIR_TO_GROUND: u32 = 0x8014_D4DC;
    pub const SPECIAL_LW_UPDATE_ACTION: u32 = 0x8014_D58C;
    pub const SPECIAL_AIR_LW_UPDATE_ACTION: u32 = 0x8014_D620;
    pub const SPECIAL_LW_CATCH_ANIM: u32 = 0x8014_D6B4;
    pub const SPECIAL_AIR_LW_CATCH_ANIM: u32 = 0x8014_D774;
    pub const SPECIAL_LW_CATCH_IASA: u32 = 0x8014_D834;
    pub const SPECIAL_AIR_LW_CATCH_IASA: u32 = 0x8014_D838;
    pub const SPECIAL_LW_CATCH_PHYS: u32 = 0x8014_D83C;
    pub const SPECIAL_AIR_LW_CATCH_PHYS: u32 = 0x8014_D85C;
    pub const SPECIAL_LW_CATCH_COLL: u32 = 0x8014_D87C;
    pub const SPECIAL_AIR_LW_CATCH_COLL: u32 = 0x8014_D8B8;
    pub const SPECIAL_LW_CATCH_GROUND_TO_AIR: u32 = 0x8014_D8F4;
    pub const SPECIAL_AIR_LW_CATCH_AIR_TO_GROUND: u32 = 0x8014_D954;
    pub const SPECIAL_LW_ABSORB_THINK_DECIDE_ACTION: u32 = 0x8014_D9B4;
    pub const SPECIAL_LW_SHOOT_ANIM: u32 = 0x8014_DA60;
    pub const SPECIAL_AIR_LW_SHOOT_ANIM: u32 = 0x8014_DAF0;
    pub const SPECIAL_LW_SHOOT_IASA: u32 = 0x8014_DB80;
    pub const SPECIAL_AIR_LW_SHOOT_IASA: u32 = 0x8014_DB84;
    pub const SPECIAL_LW_SHOOT_PHYS: u32 = 0x8014_DB88;
    pub const SPECIAL_AIR_LW_SHOOT_PHYS: u32 = 0x8014_DBA8;
    pub const SPECIAL_LW_SHOOT_COLL: u32 = 0x8014_DBC8;
    pub const SPECIAL_AIR_LW_SHOOT_COLL: u32 = 0x8014_DC04;
    pub const SPECIAL_LW_SHOOT_GROUND_TO_AIR: u32 = 0x8014_DC40;
    pub const SPECIAL_AIR_LW_SHOOT_AIR_TO_GROUND: u32 = 0x8014_DCB4;
    pub const SPECIAL_LW_SHOOT_RELEASE_OIL: u32 = 0x8014_DD28;
    pub const SPECIAL_AIR_LW_SHOOT_RELEASE_OIL: u32 = 0x8014_DE0C;
    pub const SPECIAL_HI_ITEM_RESCUE_SETUP: u32 = 0x8014_DEF0;
    pub const SPECIAL_HI_ITEM_CHECK_RESCUE_REMOVE: u32 = 0x8014_DFB8;
    pub const SPECIAL_HI_ITEM_RESCUE_SET_NULL: u32 = 0x8014_DFE4;
    pub const SPECIAL_HI_ITEM_RESCUE_REMOVE: u32 = 0x8014_DFFC;
    pub const SPECIAL_HI_ITEM_RESCUE_ENTER_HITLAG: u32 = 0x8014_E04C;
    pub const SPECIAL_HI_ITEM_RESCUE_EXIT_HITLAG: u32 = 0x8014_E07C;
    pub const SPECIAL_HI_ENTER: u32 = 0x8014_E0AC;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8014_E158;
    pub const SPECIAL_HI_ANIM: u32 = 0x8014_E1F8;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8014_E218;
    pub const SPECIAL_HI_IASA: u32 = 0x8014_E290;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8014_E2B0;
    pub const SPECIAL_HI_PHYS: u32 = 0x8014_E374;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8014_E394;
    pub const SPECIAL_HI_COLL: u32 = 0x8014_E3B4;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8014_E3D4;
    pub const SPECIAL_N_CREATE_SAUSAGE: u32 = 0x8014_E4F0;
    pub const SPECIAL_N_ENTER: u32 = 0x8014_E644;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8014_E6C0;
    pub const SPECIAL_N_ANIM: u32 = 0x8014_E73C;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8014_E7DC;
    pub const SPECIAL_N_IASA: u32 = 0x8014_E87C;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8014_E900;
    pub const SPECIAL_N_PHYS: u32 = 0x8014_E984;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8014_E9A4;
    pub const SPECIAL_N_COLL: u32 = 0x8014_E9C4;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8014_EA00;
    pub const SPECIAL_N_GROUND_TO_AIR: u32 = 0x8014_EA3C;
    pub const SPECIAL_AIR_N_AIR_TO_GROUND: u32 = 0x8014_EAAC;
    pub const SPECIAL_N_LOOP: u32 = 0x8014_EB1C;
    pub const SPECIAL_AIR_N_LOOP: u32 = 0x8014_EB8C;
}

/// His action states (`ftGw_MS_*`).
pub mod ms {
    pub const ATTACK11: i32 = 341;
    pub const ATTACK100_START: i32 = 342;
    pub const ATTACK100_LOOP: i32 = 343;
    pub const ATTACK100_END: i32 = 344;
    pub const ATTACK_LW3: i32 = 345;
    pub const ATTACK_S4: i32 = 346;
    pub const ATTACK_AIR_N: i32 = 347;
    pub const ATTACK_AIR_B: i32 = 348;
    pub const ATTACK_AIR_HI: i32 = 349;
    pub const LANDING_AIR_N: i32 = 350;
    pub const LANDING_AIR_B: i32 = 351;
    pub const LANDING_AIR_HI: i32 = 352;
    pub const SPECIAL_N: i32 = 353;
    pub const SPECIAL_AIR_N: i32 = 354;
    /// Judgement's nine numbers on the ground, then in the air.
    pub const SPECIAL_S1: i32 = 355;
    pub const SPECIAL_AIR_S1: i32 = 364;
    pub const SPECIAL_AIR_S9: i32 = 372;
    pub const SPECIAL_HI: i32 = 373;
    pub const SPECIAL_AIR_HI: i32 = 374;
    pub const SPECIAL_LW: i32 = 375;
    pub const SPECIAL_LW_CATCH: i32 = 376;
    pub const SPECIAL_LW_SHOOT: i32 = 377;
    pub const SPECIAL_AIR_LW: i32 = 378;
    pub const SPECIAL_AIR_LW_CATCH: i32 = 379;
    pub const SPECIAL_AIR_LW_SHOOT: i32 = 380;
}

/// `ftGameWatch_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// Judgement's last two numbers (`x222C_judgeVar1`, `x2230_judgeVar2`), from 0.
    pub const JUDGE_VAR1: usize = 0x0;
    pub const JUDGE_VAR2: usize = 0x4;
    /// Whether Judgement in the air has lifted him since he last landed (`x2234`).
    pub const X2234: usize = 0x8;
    /// Oil Panic's bucket: the hits it holds (`x2238_panicCharge`) and their damage
    /// (`x223C_panicDamage`).
    pub const PANIC_CHARGE: usize = 0xC;
    pub const PANIC_DAMAGE: usize = 0x10;
    /// Chef's last two sausages (`x2240_chefVar1`, `x2244_chefVar2`).
    pub const CHEF_VAR1: usize = 0x14;
    pub const CHEF_VAR2: usize = 0x18;
    /// The item he held when the down tilt took his hand (`x2248_manholeGObj`), and his
    /// items: the sprayer, the manhole, the torch, the parachute, the turtle, Sparky, the
    /// Judgement hammer, the bucket and the trampoline (`x224C` to `x226C`).
    pub const MANHOLE_HELD: usize = 0x1C;
    pub const GREENHOUSE: usize = 0x20;
    pub const MANHOLE: usize = 0x24;
    pub const FIRE: usize = 0x28;
    pub const PARACHUTE: usize = 0x2C;
    pub const TURTLE: usize = 0x30;
    pub const SPARKY: usize = 0x34;
    pub const JUDGEMENT: usize = 0x38;
    pub const PANIC: usize = 0x3C;
    pub const RESCUE: usize = 0x40;
    /// Every word of `u` that holds a GObj.
    pub const ITEMS: [usize; 10] = [
        MANHOLE_HELD,
        GREENHOUSE,
        MANHOLE,
        FIRE,
        PARACHUTE,
        TURTLE,
        SPARKY,
        JUDGEMENT,
        PANIC,
        RESCUE,
    ];
}

/// `ftGameWatch_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `Attack11.unk`.
    pub const ATTACK11_UNK: usize = 0x0;
    /// Chef: B let go of (`isChefLoopDisable`) and the sausages thrown (`maxSausage`).
    pub const N_LOOP_DISABLE: usize = 0x0;
    pub const N_SAUSAGES: usize = 0x4;
    /// Oil Panic: B let go of (`isRelease`) and the frames before he can turn again
    /// (`turnFrames`).
    pub const LW_RELEASE: usize = 0x0;
    pub const LW_TURN_FRAMES: usize = 0x4;
}

/// `ftGameWatchAttributes`' offsets (0x94 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0x94;
    /// His model's depth (`x0_GAMEWATCH_WIDTH`), his four costumes' colors (`x4`) and his
    /// outline's (`x14`).
    pub const WIDTH: usize = 0x0;
    pub const COLOR: usize = 0x4;
    pub const OUTLINE: usize = 0x14;
    /// Chef: the frame its loop goes back to, and the most sausages one throws.
    pub const CHEF_LOOPFRAME: usize = 0x18;
    pub const CHEF_MAX: usize = 0x1C;
    /// Judgement: the air speed it keeps and its slowing, its lift, its fall, and the nine
    /// numbers' weights.
    pub const JUDGE_MOMENTUM_PRESERVE: usize = 0x20;
    pub const JUDGE_MOMENTUM_MUL: usize = 0x24;
    pub const JUDGE_VEL_Y: usize = 0x28;
    pub const JUDGE_FRICTION1: usize = 0x2C;
    pub const JUDGE_FRICTION2: usize = 0x30;
    pub const JUDGE_ROLL: usize = 0x34;
    /// Fire: the stick past which it angles, how far, and its landing lag.
    pub const RESCUE_STICK_RANGE: usize = 0x58;
    pub const RESCUE_ANGLE: usize = 0x5C;
    pub const RESCUE_LANDING: usize = 0x60;
    /// Oil Panic: the air speed it keeps and its slowing, its fall, the oil's damage, the
    /// turn's frames and the bucket's absorber (`AbsorbDesc`).
    pub const PANIC_MOMENTUM_PRESERVE: usize = 0x64;
    pub const PANIC_MOMENTUM_MUL: usize = 0x68;
    pub const PANIC_FALL_ACCEL: usize = 0x6C;
    pub const PANIC_VEL_Y_MAX: usize = 0x70;
    pub const PANIC_DAMAGE_ADD: usize = 0x74;
    pub const PANIC_DAMAGE_MUL: usize = 0x78;
    pub const PANIC_TURN_FRAMES: usize = 0x7C;
    pub const PANIC_ABSORPTION: usize = 0x80;
}

/// `ftGw_Panic_*`: the bucket's fill.
pub(crate) mod panic {
    pub const EMPTY: i32 = 0;
    pub const LOW: i32 = 1;
    pub const MID: i32 = 2;
    pub const FULL: i32 = 3;
}

impl Running<'_> {
    /// A float of the fighter's `ftGameWatchAttributes`, `off` bytes in.
    pub(crate) fn gw_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftGameWatchAttributes`.
    pub(crate) fn gw_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }

    /// One of his items' GObjs in `u`.
    pub(crate) fn gw_item(&self, gobj: GObjId, off: usize) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(off)
    }

    pub(crate) fn gw_set_item(&mut self, gobj: GObjId, off: usize, item: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(off, item);
    }

    /// Where `offset` (or the joint's origin) on the joint of his parts table's entry `i` is
    /// (`lb_8000B1CC(fp->parts[i].joint, offset, &pos)`).
    pub(crate) fn gw_part_pos(&mut self, gobj: GObjId, i: i32, offset: Option<Vec3>) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let j = self.bones(f)[i as usize].joint;
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, offset)
    }

    /// His damage hooks (`death2_cb`, `take_dmg_cb`) set to `ftGw_Init_OnDamage`, each only if
    /// none is set.
    pub(crate) fn gw_damage_cbs_if_unset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.death2_cb.is_none() {
            fp.death2_cb = Some(CodeRef(addr::INIT_ON_DAMAGE));
        }
        if fp.take_dmg_cb.is_none() {
            fp.take_dmg_cb = Some(CodeRef(addr::INIT_ON_DAMAGE));
        }
    }

    /// His damage hooks set to `ftGw_Init_OnDamage`.
    pub(crate) fn gw_damage_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(addr::INIT_ON_DAMAGE));
        fp.take_dmg_cb = Some(CodeRef(addr::INIT_ON_DAMAGE));
    }

    /// His damage hooks cleared.
    pub(crate) fn gw_clear_damage_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// His hitlag hooks.
    pub(crate) fn gw_hitlag_cbs(&mut self, gobj: GObjId, enter: u32, exit: u32) {
        let fp = self.game.fighter_mut(gobj);
        fp.pre_hitlag_cb = Some(CodeRef(enter));
        fp.post_hitlag_cb = Some(CodeRef(exit));
    }

    /// His accessory callback (`accessory4_cb`).
    pub(crate) fn gw_set_accessory(&mut self, gobj: GObjId, cb: Option<u32>) {
        self.game.fighter_mut(gobj).accessory4_cb = cb.map(CodeRef);
    }
}

/// Runs one of his callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || attack11::run(g, gobj, addr)
        || attack_lw3::run(g, gobj, addr)
        || attack_s4::run(g, gobj, addr)
        || attack_air::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// His functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
