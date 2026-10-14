//! Ness's own code (`ft/kinds/ftNess`): the yo-yo's up and down smashes, the bat's forward
//! smash, PK Flash (neutral special), PK Fire (side special), PK Thunder (up special), PSI
//! Magnet (down special), his double jump and the character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftNess_MotionVars`), the items he
//! keeps in `u` (`ftNess_FighterVars`), and the character's attributes (`ftNessAttributes`)
//! in the fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Ness's through [`call`] and [`run`] by the original's addresses.

pub mod attack_hi4;
pub mod attack_lw4;
pub mod attack_s4;
pub mod init;
pub mod jump;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

pub(crate) use super::common::GA_AIR;

/// `FTKIND_NESS`.
pub const KIND_NESS: i32 = 8;

/// The items Ness registers at his creation, by their slot in his data's item list.
pub(crate) const ITEMS: [(u32, i32); 11] = [
    (0, super::super::it::kinds::NESS_PK_FIRE),
    (1, super::super::it::kinds::NESS_PK_FIRE_FLAME),
    (2, super::super::it::kinds::NESS_PK_FLUSH),
    (8, super::super::it::kinds::NESS_PK_FLUSH_EXPLODE),
    (3, super::super::it::kinds::NESS_PK_THUNDER),
    (4, super::super::it::kinds::NESS_PK_THUNDER1),
    (5, super::super::it::kinds::NESS_PK_THUNDER2),
    (6, super::super::it::kinds::NESS_PK_THUNDER3),
    (7, super::super::it::kinds::NESS_PK_THUNDER4),
    (9, super::super::it::kinds::NESS_BAT),
    (10, super::super::it::kinds::NESS_YOYO),
];

/// Ness's action states (`ftNs_MS_*`).
pub mod ms {
    pub const ATTACK_S4: i32 = 341;
    pub const ATTACK_HI4: i32 = 342;
    pub const ATTACK_HI4_CHARGE: i32 = 343;
    pub const ATTACK_HI4_RELEASE: i32 = 344;
    pub const ATTACK_LW4: i32 = 345;
    pub const ATTACK_LW4_CHARGE: i32 = 346;
    pub const ATTACK_LW4_RELEASE: i32 = 347;
    pub const SPECIAL_N_START: i32 = 348;
    pub const SPECIAL_N_HOLD: i32 = 349;
    pub const SPECIAL_N_RELEASE: i32 = 350;
    pub const SPECIAL_N_END: i32 = 351;
    pub const SPECIAL_AIR_N_START: i32 = 352;
    pub const SPECIAL_AIR_N_HOLD: i32 = 353;
    pub const SPECIAL_AIR_N_RELEASE: i32 = 354;
    pub const SPECIAL_AIR_N_END: i32 = 355;
    pub const SPECIAL_S: i32 = 356;
    pub const SPECIAL_AIR_S: i32 = 357;
    pub const SPECIAL_HI_START: i32 = 358;
    pub const SPECIAL_HI_HOLD: i32 = 359;
    pub const SPECIAL_HI_END: i32 = 360;
    pub const SPECIAL_HI: i32 = 361;
    pub const SPECIAL_AIR_HI_START: i32 = 362;
    pub const SPECIAL_AIR_HI_HOLD: i32 = 363;
    pub const SPECIAL_AIR_HI_END: i32 = 364;
    pub const SPECIAL_AIR_HI: i32 = 365;
    pub const SPECIAL_AIR_HI_REBOUND: i32 = 366;
    pub const SPECIAL_LW_START: i32 = 367;
    pub const SPECIAL_LW_HOLD: i32 = 368;
    pub const SPECIAL_LW_HIT: i32 = 369;
    pub const SPECIAL_LW_END: i32 = 370;
    pub const SPECIAL_LW_TURN: i32 = 371;
    pub const SPECIAL_AIR_LW_START: i32 = 372;
    pub const SPECIAL_AIR_LW_HOLD: i32 = 373;
    pub const SPECIAL_AIR_LW_HIT: i32 = 374;
    pub const SPECIAL_AIR_LW_END: i32 = 375;
    pub const SPECIAL_AIR_LW_TURN: i32 = 376;
}

/// Ness's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const JUMP_AERIAL_ENTER: u32 = 0x800C_BD18;
    pub const JUMP_AERIAL_PHYS_CB: u32 = 0x800C_C654;
    pub const ON_DEATH: u32 = 0x8011_47C0;
    pub const ON_LOAD: u32 = 0x8011_480C;
    pub const ON_DAMAGE: u32 = 0x8011_48F8;
    pub const ON_ABSORB: u32 = 0x8011_493C;
    pub const GET_ABSORB_HEAL: u32 = 0x8011_495C;
    pub const ON_ITEM_PICKUP: u32 = 0x8011_4968;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8011_4A48;
    pub const ON_ITEM_VISIBLE: u32 = 0x8011_4A90;
    pub const ON_ITEM_DROP: u32 = 0x8011_4AD8;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8011_4B2C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8011_4B6C;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8011_4BB0;
    pub const ATTACK_S4_ON_REFLECT: u32 = 0x8011_4BF4;
    pub const ATTACK_S4_ENTER: u32 = 0x8011_4C24;
    pub const ATTACK_S4_CHECK_NESS_BAT_REMOVE: u32 = 0x8011_4CC0;
    pub const ATTACK_S4_ITEM_NESS_BAT_REMOVE: u32 = 0x8011_4CF4;
    pub const ATTACK_S4_ITEM_NESS_BAT_SET_NULL: u32 = 0x8011_4D34;
    pub const ATTACK_S4_ANIM: u32 = 0x8011_4D50;
    pub const ATTACK_S4_IASA: u32 = 0x8011_4E04;
    pub const ATTACK_S4_PHYS: u32 = 0x8011_4E64;
    pub const ATTACK_S4_COLL: u32 = 0x8011_4E98;
    pub const ATTACK_HI4_YOYO_UPDATE_HIT_POS: u32 = 0x8011_4EB8;
    pub const ATTACK_HI4_YOYO_CHECK_TIMED_REHIT: u32 = 0x8011_4F0C;
    pub const ATTACK_HI4_YOYO_APPLY_DAMAGE: u32 = 0x8011_4F70;
    pub const ATTACK_HI4_YOYO_CHECK_ENV_COLL: u32 = 0x8011_4FF8;
    pub const ATTACK_HI4_YOYO_SET_UNK_POS: u32 = 0x8011_5114;
    pub const ATTACK_HI4_YOYO_SET_HIT_POS: u32 = 0x8011_51EC;
    pub const ATTACK_HI4_YOYO_SET_HIT_POS_UNK: u32 = 0x8011_52D0;
    pub const ATTACK_HI4_YOYO_CHECK_NO_OBSTRUCT: u32 = 0x8011_5404;
    pub const ATTACK_HI4_YOYO_SET_VAR_ALL: u32 = 0x8011_5534;
    pub const ATTACK_HI4_YOYO_APPLY_SMASH: u32 = 0x8011_556C;
    pub const ATTACK_HI4_YOYO_SET_CHARGE_DAMAGE: u32 = 0x8011_5680;
    pub const ATTACK_HI4_YOYO_THINK_IS_REMOVE: u32 = 0x8011_5784;
    pub const ATTACK_HI4_YOYO_SET_UNK_RATE: u32 = 0x8011_599C;
    pub const ATTACK_HI4_YOYO_CREATE_ITEM: u32 = 0x8011_5A08;
    pub const ATTACK_HI4_YOYO_ITEM_SET_FLAG: u32 = 0x8011_5AC4;
    pub const ATTACK_HI4_YOYO_ITEM_DESPAWN: u32 = 0x8011_5AF8;
    pub const ATTACK_HI4_YOYO_ITEM_SET_UNK: u32 = 0x8011_5B50;
    pub const ATTACK_HI4_YOYO_ITEM_SET_UNK2: u32 = 0x8011_5B80;
    pub const ATTACK_HI4_ENTER: u32 = 0x8011_5BB0;
    pub const ATTACK_HI4_YOYO_START_TIMED_REHIT: u32 = 0x8011_5C74;
    pub const ATTACK_HI4_ANIM: u32 = 0x8011_5C9C;
    pub const ATTACK_HI4_IASA: u32 = 0x8011_5E74;
    pub const ATTACK_HI4_PHYS: u32 = 0x8011_5EB8;
    pub const ATTACK_HI4_COLL: u32 = 0x8011_5F14;
    pub const ATTACK_HI4_CHARGE_ANIM: u32 = 0x8011_5F88;
    pub const ATTACK_HI4_CHARGE_IASA: u32 = 0x8011_60B4;
    pub const ATTACK_HI4_CHARGE_PHYS: u32 = 0x8011_60E4;
    pub const ATTACK_HI4_CHARGE_COLL: u32 = 0x8011_6104;
    pub const ATTACK_HI4_CHARGE_ENTER: u32 = 0x8011_6178;
    pub const ATTACK_HI4_RELEASE_ANIM: u32 = 0x8011_620C;
    pub const ATTACK_HI4_RELEASE_IASA: u32 = 0x8011_62B0;
    pub const ATTACK_HI4_RELEASE_PHYS: u32 = 0x8011_62E0;
    pub const ATTACK_HI4_RELEASE_COLL: u32 = 0x8011_6420;
    pub const ATTACK_HI4_RELEASE_ENTER: u32 = 0x8011_6494;
    pub const ATTACK_LW4_ENTER: u32 = 0x8011_659C;
    pub const ATTACK_LW4_ANIM: u32 = 0x8011_6638;
    pub const ATTACK_LW4_IASA: u32 = 0x8011_66D4;
    pub const ATTACK_LW4_PHYS: u32 = 0x8011_6718;
    pub const ATTACK_LW4_COLL: u32 = 0x8011_674C;
    pub const ATTACK_LW4_CHARGE_ANIM: u32 = 0x8011_6798;
    pub const ATTACK_LW4_CHARGE_IASA: u32 = 0x8011_6828;
    pub const ATTACK_LW4_CHARGE_PHYS: u32 = 0x8011_6858;
    pub const ATTACK_LW4_CHARGE_COLL: u32 = 0x8011_6878;
    pub const ATTACK_LW4_CHARGE_ENTER: u32 = 0x8011_68C4;
    pub const ATTACK_LW4_RELEASE_ANIM: u32 = 0x8011_6958;
    pub const ATTACK_LW4_RELEASE_IASA: u32 = 0x8011_69BC;
    pub const ATTACK_LW4_RELEASE_PHYS: u32 = 0x8011_69EC;
    pub const ATTACK_LW4_RELEASE_COLL: u32 = 0x8011_6A9C;
    pub const ATTACK_LW4_RELEASE_ENTER: u32 = 0x8011_6AE8;
    pub const SPECIAL_S_ITEM_PKFIRE_SPAWN: u32 = 0x8011_6B70;
    pub const SPECIAL_S_ENTER: u32 = 0x8011_6C94;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8011_6D04;
    pub const SPECIAL_S_ANIM: u32 = 0x8011_6D74;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8011_6DB0;
    pub const SPECIAL_S_PHYS: u32 = 0x8011_6DEC;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8011_6E0C;
    pub const SPECIAL_S_COLL: u32 = 0x8011_6E2C;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8011_6E68;
    pub const SPECIAL_N_CHECK_SPECIAL_N_HOLD: u32 = 0x8011_6EBC;
    pub const SPECIAL_N_SET_NULL: u32 = 0x8011_6F00;
    pub const SPECIAL_N_ITEM_PKFLUSH_SET_NULL: u32 = 0x8011_6F38;
    pub const SPECIAL_N_START_ENTER: u32 = 0x8011_6F94;
    pub const SPECIAL_AIR_N_START_ENTER: u32 = 0x8011_7034;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8011_70DC;
    pub const SPECIAL_N_RELEASE_ANIM: u32 = 0x8011_71BC;
    pub const SPECIAL_N_END_ANIM: u32 = 0x8011_72F0;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8011_7378;
    pub const SPECIAL_AIR_N_RELEASE_ANIM: u32 = 0x8011_7458;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x8011_758C;
    pub const SPECIAL_N_START_IASA: u32 = 0x8011_7648;
    pub const SPECIAL_N_RELEASE_IASA: u32 = 0x8011_764C;
    pub const SPECIAL_N_END_IASA: u32 = 0x8011_76C8;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8011_76CC;
    pub const SPECIAL_AIR_N_RELEASE_IASA: u32 = 0x8011_76D0;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x8011_774C;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8011_7750;
    pub const SPECIAL_N_RELEASE_PHYS: u32 = 0x8011_7788;
    pub const SPECIAL_N_END_PHYS: u32 = 0x8011_77A8;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8011_77C8;
    pub const SPECIAL_AIR_N_RELEASE_PHYS: u32 = 0x8011_7828;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x8011_7888;
    pub const SPECIAL_N_START_COLL: u32 = 0x8011_78E8;
    pub const SPECIAL_N_RELEASE_COLL: u32 = 0x8011_7954;
    pub const SPECIAL_N_END_COLL: u32 = 0x8011_79C0;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8011_7A2C;
    pub const SPECIAL_AIR_N_RELEASE_COLL: u32 = 0x8011_7A98;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x8011_7B04;
    pub const SPECIAL_HI_STOP_GFX: u32 = 0x8011_7B70;
    pub const SPECIAL_HI_ITEM_PKTHUNDER_CHECK_NESS_COLLIDE: u32 = 0x8011_7BBC;
    pub const SPECIAL_HI_CHECK_SPECIAL_HI_HOLD: u32 = 0x8011_7D9C;
    pub const SPECIAL_HI_ITEM_PKTHUNDER_REMOVE: u32 = 0x8011_7DD4;
    pub const SPECIAL_HI_TAKE_DAMAGE: u32 = 0x8011_7E60;
    pub const SPECIAL_HI_ITEM_PKTHUNDER_CHECK_OWN: u32 = 0x8011_7F00;
    pub const SPECIAL_AIR_HI_COLLISION_MOD_VEL: u32 = 0x8011_7F24;
    pub const SPECIAL_HI_START_ENTER: u32 = 0x8011_8120;
    pub const SPECIAL_AIR_HI_START_ENTER: u32 = 0x8011_8250;
    pub const SPECIAL_HI_ENTER: u32 = 0x8011_8384;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8011_8570;
    pub const SPECIAL_HI_START_ANIM: u32 = 0x8011_86B0;
    pub const SPECIAL_HI_HOLD_ANIM: u32 = 0x8011_87A4;
    pub const SPECIAL_HI_END_ANIM: u32 = 0x8011_8900;
    pub const SPECIAL_HI_ANIM: u32 = 0x8011_893C;
    pub const SPECIAL_AIR_HI_START_ANIM: u32 = 0x8011_8A10;
    pub const SPECIAL_AIR_HI_HOLD_ANIM: u32 = 0x8011_8B04;
    pub const SPECIAL_AIR_HI_END_ANIM: u32 = 0x8011_8D60;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8011_8DF8;
    pub const SPECIAL_AIR_HI_REBOUND_ANIM: u32 = 0x8011_8EF0;
    pub const SPECIAL_HI_START_IASA: u32 = 0x8011_8F80;
    pub const SPECIAL_HI_HOLD_IASA: u32 = 0x8011_8F84;
    pub const SPECIAL_HI_END_IASA: u32 = 0x8011_8F88;
    pub const SPECIAL_HI_IASA: u32 = 0x8011_8F8C;
    pub const SPECIAL_AIR_HI_START_IASA: u32 = 0x8011_8F90;
    pub const SPECIAL_AIR_HI_HOLD_IASA: u32 = 0x8011_8F94;
    pub const SPECIAL_AIR_HI_END_IASA: u32 = 0x8011_8F98;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8011_8F9C;
    pub const SPECIAL_AIR_HI_REBOUND_IASA: u32 = 0x8011_8FA0;
    pub const SPECIAL_HI_START_PHYS: u32 = 0x8011_8FA4;
    pub const SPECIAL_HI_HOLD_PHYS: u32 = 0x8011_8FDC;
    pub const SPECIAL_HI_END_PHYS: u32 = 0x8011_8FFC;
    pub const SPECIAL_HI_PHYS: u32 = 0x8011_901C;
    pub const SPECIAL_AIR_HI_START_PHYS: u32 = 0x8011_9134;
    pub const SPECIAL_AIR_HI_HOLD_PHYS: u32 = 0x8011_9194;
    pub const SPECIAL_AIR_HI_END_PHYS: u32 = 0x8011_91F4;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8011_9254;
    pub const SPECIAL_AIR_HI_REBOUND_PHYS: u32 = 0x8011_9410;
    pub const SPECIAL_HI_START_COLL: u32 = 0x8011_9460;
    pub const SPECIAL_HI_HOLD_COLL: u32 = 0x8011_94CC;
    pub const SPECIAL_HI_END_COLL: u32 = 0x8011_9538;
    pub const SPECIAL_HI_COLL: u32 = 0x8011_95A4;
    pub const SPECIAL_AIR_HI_START_COLL: u32 = 0x8011_9798;
    pub const SPECIAL_AIR_HI_HOLD_COLL: u32 = 0x8011_9804;
    pub const SPECIAL_AIR_HI_END_COLL: u32 = 0x8011_9870;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8011_98DC;
    pub const SPECIAL_AIR_HI_REBOUND_COLL: u32 = 0x8011_9D58;
    pub const SPECIAL_LW_START_ENTER: u32 = 0x8011_9E14;
    pub const SPECIAL_AIR_LW_START_ENTER: u32 = 0x8011_9E90;
    pub const SPECIAL_LW_START_ANIM: u32 = 0x8011_9F20;
    pub const SPECIAL_AIR_LW_START_ANIM: u32 = 0x8011_A000;
    pub const SPECIAL_LW_START_IASA: u32 = 0x8011_A0E0;
    pub const SPECIAL_AIR_LW_START_IASA: u32 = 0x8011_A0E4;
    pub const SPECIAL_LW_START_PHYS: u32 = 0x8011_A0E8;
    pub const SPECIAL_AIR_LW_START_PHYS: u32 = 0x8011_A108;
    pub const SPECIAL_LW_START_COLL: u32 = 0x8011_A168;
    pub const SPECIAL_AIR_LW_START_COLL: u32 = 0x8011_A1A4;
    pub const SPECIAL_LW_START_GROUND_TO_AIR: u32 = 0x8011_A1E0;
    pub const SPECIAL_AIR_LW_START_AIR_TO_GROUND: u32 = 0x8011_A240;
    pub const SPECIAL_LW_HOLD_ANIM: u32 = 0x8011_A2A8;
    pub const SPECIAL_AIR_LW_HOLD_ANIM: u32 = 0x8011_A370;
    pub const SPECIAL_LW_HOLD_IASA: u32 = 0x8011_A438;
    pub const SPECIAL_AIR_LW_HOLD_IASA: u32 = 0x8011_A43C;
    pub const SPECIAL_LW_HOLD_PHYS: u32 = 0x8011_A440;
    pub const SPECIAL_AIR_LW_HOLD_PHYS: u32 = 0x8011_A474;
    pub const SPECIAL_LW_HOLD_COLL: u32 = 0x8011_A4E8;
    pub const SPECIAL_AIR_LW_HOLD_COLL: u32 = 0x8011_A524;
    pub const SPECIAL_LW_HOLD_GROUND_TO_AIR: u32 = 0x8011_A560;
    pub const SPECIAL_AIR_LW_HOLD_AIR_TO_GROUND: u32 = 0x8011_A5D4;
    pub const SPECIAL_LW_HOLD_ENTER: u32 = 0x8011_A650;
    pub const SPECIAL_AIR_LW_HOLD_ENTER: u32 = 0x8011_A6A8;
    pub const SPECIAL_LW_TURN_ANIM: u32 = 0x8011_A700;
    pub const SPECIAL_AIR_LW_TURN_ANIM: u32 = 0x8011_A810;
    pub const SPECIAL_LW_TURN_IASA: u32 = 0x8011_A920;
    pub const SPECIAL_AIR_LW_TURN_IASA: u32 = 0x8011_A924;
    pub const SPECIAL_LW_TURN_PHYS: u32 = 0x8011_A928;
    pub const SPECIAL_AIR_LW_TURN_PHYS: u32 = 0x8011_A95C;
    pub const SPECIAL_LW_TURN_COLL: u32 = 0x8011_A9D0;
    pub const SPECIAL_AIR_LW_TURN_COLL: u32 = 0x8011_AA0C;
    pub const SPECIAL_LW_TURN_GROUND_TO_AIR: u32 = 0x8011_AA48;
    pub const SPECIAL_AIR_LW_TURN_AIR_TO_GROUND: u32 = 0x8011_AAA8;
    pub const SPECIAL_LW_HOLD_GROUND_OR_AIR: u32 = 0x8011_AB10;
    pub const SPECIAL_LW_HIT_ANIM: u32 = 0x8011_ABF8;
    pub const SPECIAL_AIR_LW_HIT_ANIM: u32 = 0x8011_ADC8;
    pub const SPECIAL_LW_HIT_IASA: u32 = 0x8011_AEE0;
    pub const SPECIAL_AIR_LW_HIT_IASA: u32 = 0x8011_AEE4;
    pub const SPECIAL_LW_HIT_PHYS: u32 = 0x8011_AEE8;
    pub const SPECIAL_AIR_LW_HIT_PHYS: u32 = 0x8011_AF1C;
    pub const SPECIAL_LW_HIT_COLL: u32 = 0x8011_AF90;
    pub const SPECIAL_AIR_LW_HIT_COLL: u32 = 0x8011_AFCC;
    pub const SPECIAL_LW_HIT_GROUND_TO_AIR: u32 = 0x8011_B008;
    pub const SPECIAL_AIR_LW_HIT_AIR_TO_GROUND: u32 = 0x8011_B07C;
    pub const ABSORB_THINK_DECIDE_ACTION: u32 = 0x8011_B0F8;
    pub const SPECIAL_LW_END_ANIM: u32 = 0x8011_B25C;
    pub const SPECIAL_AIR_LW_END_ANIM: u32 = 0x8011_B2A0;
    pub const SPECIAL_LW_END_IASA: u32 = 0x8011_B2E4;
    pub const SPECIAL_AIR_LW_END_IASA: u32 = 0x8011_B2E8;
    pub const SPECIAL_LW_END_PHYS: u32 = 0x8011_B2EC;
    pub const SPECIAL_AIR_LW_END_PHYS: u32 = 0x8011_B30C;
    pub const SPECIAL_LW_END_COLL: u32 = 0x8011_B36C;
    pub const SPECIAL_AIR_LW_END_COLL: u32 = 0x8011_B3A8;
    pub const SPECIAL_LW_END_GROUND_TO_AIR: u32 = 0x8011_B3E4;
    pub const SPECIAL_AIR_LW_END_AIR_TO_GROUND: u32 = 0x8011_B444;
    pub const SPECIAL_LW_END_ENTER: u32 = 0x8011_B4AC;
    pub const SPECIAL_AIR_LW_END_ENTER: u32 = 0x8011_B4E4;
}

/// `ftNess_FighterVars`' offsets in `u`.
pub(crate) mod u {
    pub const YOYO: usize = 0x0;
    pub const YOYO_HITBOX_POS: usize = 0x4;
    pub const X223C: usize = 0x10;
    pub const PKFLASH: usize = 0x14;
    pub const PKTHUNDER: usize = 0x18;
    pub const BAT: usize = 0x1C;
    pub const PKTHUNDER_GFX: usize = 0x20;
}

/// `ftNess_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    // `attackhi4` and `attacklw4` (`ftNess_YoyoVars`).
    pub const YOYO_CURRENT_FRAME: usize = 0x0;
    pub const YOYO_REHIT_TIMER: usize = 0x4;
    pub const YOYO_CHARGE_DISABLE: usize = 0x8;
    pub const YOYO_POS_UPDATE_MOD: usize = 0xC;
    // `specialn`.
    pub const N_FRAMES_TO_LOOP_CHARGE_GROUND: usize = 0x0;
    pub const N_FRAMES_TO_LOOP_CHARGE_AIR: usize = 0x4;
    pub const N_FALLING_ACCELERATION_DELAY: usize = 0x8;
    pub const N_CHARGE_RELEASE_DELAY: usize = 0xC;
    // `specialhi`.
    pub const HI_THUNDER_COLL: usize = 0x0;
    pub const HI_TIMER_LOOP1: usize = 0x4;
    pub const HI_TIMER_LOOP2: usize = 0x8;
    pub const HI_GRAVITY_DELAY: usize = 0xC;
    pub const HI_COLL_POS1: usize = 0x10;
    pub const HI_COLL_POS2: usize = 0x1C;
    pub const HI_AERIAL_VEL: usize = 0x28;
    pub const HI_UNK_VAR: usize = 0x2C;
    pub const HI_FACING_DIR: usize = 0x30;
    pub const HI_UNK_VECTOR1: usize = 0x34;
    pub const HI_JIBAKU_GFX: usize = 0x40;
    pub const HI_FALL_ACCEL: usize = 0x44;
    pub const HI_UNK_VAR3: usize = 0x48;
    pub const HI_UNK_VAR4: usize = 0x4C;
    // `speciallw`.
    pub const LW_RELEASE_LAG: usize = 0x0;
    pub const LW_TURN_FRAMES: usize = 0x4;
    pub const LW_IS_RELEASE: usize = 0x8;
    pub const LW_GRAVITY_DELAY: usize = 0xC;
    pub const LW_X10: usize = 0x10;
}

/// `ftNessAttributes`' offsets.
pub(crate) mod attr {
    pub const PKFLASH_TIMER1_LOOPFRAMES: usize = 0x0;
    pub const PKFLASH_TIMER2_LOOPFRAMES: usize = 0x4;
    pub const PKFLASH_GRAVITY_DELAY: usize = 0x8;
    pub const PKFLASH_MINCHARGEFRAMES: usize = 0xC;
    pub const PKFLASH_FALL_ACCEL: usize = 0x14;
    pub const PKFLASH_LANDING_LAG: usize = 0x1C;
    pub const PKFIRE_AERIAL_LAUNCH_TRAJECTORY: usize = 0x20;
    pub const PKFIRE_AERIAL_VELOCITY: usize = 0x24;
    pub const PKFIRE_GROUNDED_LAUNCH_TRAJECTORY: usize = 0x28;
    pub const PKFIRE_GROUNDED_VELOCITY: usize = 0x2C;
    pub const PKFIRE_SPAWN_X: usize = 0x30;
    pub const PKFIRE_SPAWN_Y: usize = 0x34;
    pub const PKFIRE_LANDING_LAG: usize = 0x38;
    pub const PK_THUNDER_LOOP1: usize = 0x40;
    pub const PK_THUNDER_LOOP2: usize = 0x44;
    pub const PK_THUNDER_GRAVITY_DELAY: usize = 0x48;
    pub const PK_THUNDER_FALL_ACCEL: usize = 0x50;
    pub const PK_THUNDER_2_MOMENTUM: usize = 0x54;
    pub const PK_THUNDER_2_UNK1: usize = 0x58;
    pub const PK_THUNDER_2_DECELERATION_RATE: usize = 0x5C;
    pub const PK_THUNDER_2_KNOCKDOWN_ANGLE: usize = 0x60;
    pub const PK_THUNDER_2_WALLHUG_ANGLE: usize = 0x64;
    pub const PK_THUNDER_2_FREEFALL_ANIM_BLEND: usize = 0x6C;
    pub const PK_THUNDER_2_LANDING_LAG: usize = 0x70;
    pub const PSI_MAGNET_RELEASE_LAG: usize = 0x74;
    pub const PSI_MAGNET_UNK1: usize = 0x78;
    pub const PSI_MAGNET_UNK2: usize = 0x7C;
    pub const PSI_MAGNET_FRAMES_BEFORE_GRAVITY: usize = 0x84;
    pub const PSI_MAGNET_MOMENTUM_PRESERVATION: usize = 0x88;
    pub const PSI_MAGNET_FALL_ACCEL: usize = 0x8C;
    pub const PSI_MAGNET_HEAL_MUL: usize = 0x94;
    pub const PSI_MAGNET_ABSORPTION: usize = 0x98;
    pub const YOYO_CHARGE_DURATION: usize = 0xAC;
    pub const YOYO_DAMAGE_MUL: usize = 0xB0;
    pub const YOYO_REHIT_RATE: usize = 0xB4;
    pub const BASEBALL_BAT: usize = 0xB8;
}

/// `FtPart_HipN`, `FtPart_L2ndNa`, `FtPart_R2ndNa`, the parts Ness's code reads straight from
/// his parts table.
pub(crate) const PART_HIP_N: i32 = 4;
pub(crate) const PART_L2ND_NA: i32 = 24;
pub(crate) const PART_R2ND_NA: i32 = 42;

impl Running<'_> {
    /// A float of the fighter's `ftNessAttributes`, `off` bytes in.
    pub(crate) fn ness_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.ness_attr_word(f, off))
    }

    pub(crate) fn ness_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.ness_attr_word(f, off) as i32
    }

    fn ness_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Ness's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    if a == addr::JUMP_AERIAL_PHYS_CB {
        g.ness_jump_aerial_phys_cb(gobj);
        return true;
    }
    init::run(g, gobj, a)
        || attack_hi4::run(g, gobj, a)
        || attack_lw4::run(g, gobj, a)
        || attack_s4::run(g, gobj, a)
        || special_n::run(g, gobj, a)
        || special_s::run(g, gobj, a)
        || special_hi::run(g, gobj, a)
        || special_lw::run(g, gobj, a)
}

/// Ness's functions other code calls by address (`code::call`): the item hooks, the yo-yo's
/// stage check and every callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    if f == addr::ATTACK_HI4_YOYO_CHECK_ENV_COLL {
        return Some(Val::Int(r.ness_yoyo_check_env_coll(
            args[0].gobj().expect("a null fighter"),
            args[1].vec(),
            args[2].vec(),
            args[3].float(),
        )));
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
