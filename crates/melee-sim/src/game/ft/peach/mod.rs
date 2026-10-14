//! Peach's own code (`ft/kinds/ftPeach`): her float and its aerials, the forward smash's three
//! weapons, Toad (neutral special), the Peach Bomber (side special), the parasol (up special),
//! the vegetable pull (down special), and the character's callbacks. Her items are in
//! `it::kinds` (`peachexplode`, `peachturnip`, `peachparasol`, `peachtoad`, `peachtoadspore`).
//!
//! A state's variables live in the fighter's `mv` union (`ftPe_MotionVars`), the character's
//! in `u` (`ftPeach_FighterVars`), and the character's attributes (`ftPe_DatAttrs`) in the
//! fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Peach's through [`call`] and [`run`] by the original's addresses.

pub mod attack_s4;
pub mod float;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_PEACH`.
pub const KIND_PEACH: i32 = 9;

/// `ftCo_MS_None`.
pub(crate) const MS_NONE: i32 = -1;

/// Peach's functions, by address: the callbacks her states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8011_B51C;
    pub const ON_LOAD: u32 = 0x8011_B628;
    pub const ON_DEATH2: u32 = 0x8011_B704;
    pub const ON_ITEM_PICKUP: u32 = 0x8011_B740;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8011_B820;
    pub const ON_ITEM_VISIBLE: u32 = 0x8011_B868;
    pub const ON_ITEM_DROP: u32 = 0x8011_B8B0;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8011_B904;
    pub const APPEAL_SWAP: u32 = 0x8011_B93C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8011_B998;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8011_B9DC;

    pub const FLOAT_ANIM: u32 = 0x8011_BC38;
    pub const FLOAT_IASA: u32 = 0x8011_BC8C;
    pub const FLOAT_PHYS: u32 = 0x8011_BD18;
    pub const FLOAT_COLL: u32 = 0x8011_BD3C;
    pub const FLOAT_FALL_ANIM: u32 = 0x8011_BDF0;
    pub const FLOAT_FALL_IASA: u32 = 0x8011_BE2C;
    pub const FLOAT_FALL_PHYS: u32 = 0x8011_BE30;
    pub const FLOAT_FALL_COLL: u32 = 0x8011_BE50;
    pub const FLOAT_ATTACK_AIR_ANIM: u32 = 0x8011_BF88;
    pub const FLOAT_ATTACK_AIR_IASA: u32 = 0x8011_C044;
    pub const FLOAT_ATTACK_AIR_PHYS: u32 = 0x8011_C160;
    pub const FLOAT_ATTACK_AIR_COLL: u32 = 0x8011_C198;

    pub const ATTACK_S4_ANIM: u32 = 0x8011_C248;
    pub const ATTACK_S4_IASA: u32 = 0x8011_C284;
    pub const ATTACK_S4_PHYS: u32 = 0x8011_C2B4;
    pub const ATTACK_S4_COLL: u32 = 0x8011_C2D4;

    pub const S_RESET: u32 = 0x8011_C2F4;
    pub const SPECIAL_S_ENTER: u32 = 0x8011_C34C;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8011_C3C4;
    pub const S_AIR_END0: u32 = 0x8011_C430;
    pub const SPECIAL_S_START_ANIM: u32 = 0x8011_C4F0;
    pub const SPECIAL_AIR_S_START_ANIM: u32 = 0x8011_C588;
    pub const SPECIAL_S_START_IASA: u32 = 0x8011_C5E8;
    pub const SPECIAL_AIR_S_START_IASA: u32 = 0x8011_C5EC;
    pub const SPECIAL_S_START_PHYS: u32 = 0x8011_C5F0;
    pub const SPECIAL_AIR_S_START_PHYS: u32 = 0x8011_C644;
    pub const SPECIAL_S_START_COLL: u32 = 0x8011_C664;
    pub const SPECIAL_AIR_S_START_COLL: u32 = 0x8011_C6FC;
    pub const SPECIAL_AIR_S_JUMP_ANIM: u32 = 0x8011_C878;
    pub const SPECIAL_AIR_S_JUMP_IASA: u32 = 0x8011_C8C8;
    pub const SPECIAL_AIR_S_JUMP_PHYS: u32 = 0x8011_C8CC;
    pub const SPECIAL_AIR_S_JUMP_COLL: u32 = 0x8011_C93C;
    pub const SPECIAL_S_END_ANIM: u32 = 0x8011_CA84;
    pub const SPECIAL_AIR_S_END_ANIM: u32 = 0x8011_CAC0;
    pub const SPECIAL_S_END_IASA: u32 = 0x8011_CAFC;
    pub const SPECIAL_AIR_S_END_IASA: u32 = 0x8011_CB00;
    pub const SPECIAL_S_END_PHYS: u32 = 0x8011_CB04;
    pub const SPECIAL_AIR_S_END_PHYS: u32 = 0x8011_CB24;
    pub const SPECIAL_S_END_COLL: u32 = 0x8011_CB44;
    pub const SPECIAL_AIR_S_END_COLL: u32 = 0x8011_CB80;

    pub const SPAWN_VEG: u32 = 0x8011_D018;
    pub const SPECIAL_LW_ENTER: u32 = 0x8011_D11C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8011_D1C4;
    pub const SPECIAL_LW_ANIM: u32 = 0x8011_D2EC;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8011_D340;
    pub const SPECIAL_LW_PHYS: u32 = 0x8011_D394;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8011_D3B4;
    pub const SPECIAL_LW_COLL: u32 = 0x8011_D3D4;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8011_D3FC;

    pub const SPECIAL_HI_8011D424: u32 = 0x8011_D424;
    pub const PARASOL_8011D518: u32 = 0x8011_D518;
    pub const PARASOL_8011D598: u32 = 0x8011_D598;
    pub const SPECIAL_HI_8011D620: u32 = 0x8011_D620;
    pub const SPECIAL_HI_8011D650: u32 = 0x8011_D650;
    pub const SPECIAL_HI_ENTER: u32 = 0x8011_D72C;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8011_D7F0;
    pub const SPECIAL_HI_START_ANIM: u32 = 0x8011_D8D0;
    pub const SPECIAL_AIR_HI_START_ANIM: u32 = 0x8011_D980;
    pub const SPECIAL_HI_START_IASA: u32 = 0x8011_DA30;
    pub const SPECIAL_AIR_HI_START_IASA: u32 = 0x8011_DB78;
    pub const SPECIAL_HI_START_PHYS: u32 = 0x8011_DCC0;
    pub const SPECIAL_AIR_HI_START_PHYS: u32 = 0x8011_DCF8;
    pub const SPECIAL_HI_8011DD8C: u32 = 0x8011_DD8C;
    pub const SPECIAL_HI_START_COLL: u32 = 0x8011_DDBC;
    pub const SPECIAL_AIR_HI_START_COLL: u32 = 0x8011_DE2C;
    pub const SPECIAL_HI_END_ANIM: u32 = 0x8011_DE9C;
    pub const SPECIAL_AIR_HI_END_ANIM: u32 = 0x8011_DEFC;
    pub const SPECIAL_HI_END_IASA: u32 = 0x8011_DF5C;
    pub const SPECIAL_AIR_HI_END_IASA: u32 = 0x8011_DF60;
    pub const SPECIAL_HI_END_PHYS: u32 = 0x8011_DF64;
    pub const SPECIAL_AIR_HI_END_PHYS: u32 = 0x8011_DFD0;
    pub const SPECIAL_HI_8011E064: u32 = 0x8011_E064;
    pub const SPECIAL_HI_END_COLL: u32 = 0x8011_E094;
    pub const SPECIAL_AIR_HI_END_COLL: u32 = 0x8011_E104;

    pub const N_ON_ACCESSORY4: u32 = 0x8011_E174;
    pub const N_ON_HIT_ACCESSORY4: u32 = 0x8011_E230;
    pub const SPECIAL_N_DO_DEATH2: u32 = 0x8011_E2AC;
    pub const SPECIAL_N_ON_DEATH2: u32 = 0x8011_E2E8;
    pub const N_ON_ENTER_HITLAG: u32 = 0x8011_E348;
    pub const N_ON_EXIT_HITLAG: u32 = 0x8011_E378;
    pub const SPECIAL_N_ENTER: u32 = 0x8011_E3D0;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8011_E45C;
    pub const SPECIAL_N_ANIM: u32 = 0x8011_E4F8;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8011_E5B4;
    pub const SPECIAL_N_IASA: u32 = 0x8011_E670;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8011_E674;
    pub const SPECIAL_N_PHYS: u32 = 0x8011_E678;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8011_E6AC;
    pub const SPECIAL_N_COLL: u32 = 0x8011_E75C;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8011_E798;
    pub const SPECIAL_N_HIT_ANIM: u32 = 0x8011_E9A0;
    pub const SPECIAL_AIR_N_HIT_ANIM: u32 = 0x8011_EA04;
    pub const SPECIAL_N_HIT_IASA: u32 = 0x8011_EA68;
    pub const SPECIAL_AIR_N_HIT_IASA: u32 = 0x8011_EA6C;
    pub const SPECIAL_N_HIT_PHYS: u32 = 0x8011_EA70;
    pub const SPECIAL_AIR_N_HIT_PHYS: u32 = 0x8011_EA90;
    pub const SPECIAL_N_HIT_COLL: u32 = 0x8011_EAE0;
    pub const SPECIAL_AIR_N_HIT_COLL: u32 = 0x8011_EB1C;
    pub const N_ON_UNK_HIT: u32 = 0x8011_EC6C;
}

/// Peach's action states (`ftPe_MS_*`).
pub mod ms {
    pub const FLOAT: i32 = 341;
    pub const FLOAT_FALL_F: i32 = 342;
    pub const FLOAT_FALL_B: i32 = 343;
    pub const FLOAT_ATTACK_AIR_N: i32 = 344;
    pub const ATTACK_S4_CLUB: i32 = 349;
    pub const ATTACK_S4_RACKET: i32 = 351;
    pub const SPECIAL_LW: i32 = 352;
    pub const SPECIAL_AIR_LW: i32 = 353;
    pub const SPECIAL_S_START: i32 = 354;
    pub const SPECIAL_S_END: i32 = 355;
    pub const SPECIAL_S_JUMP: i32 = 356;
    pub const SPECIAL_AIR_S_START: i32 = 357;
    pub const SPECIAL_AIR_S_END_0: i32 = 358;
    pub const SPECIAL_AIR_S_END_1: i32 = 359;
    pub const SPECIAL_AIR_S_JUMP: i32 = 360;
    pub const SPECIAL_HI_START: i32 = 361;
    pub const SPECIAL_HI_END: i32 = 362;
    pub const SPECIAL_AIR_HI_START: i32 = 363;
    pub const SPECIAL_AIR_HI_END: i32 = 364;
    pub const SPECIAL_N: i32 = 365;
    pub const SPECIAL_N_HIT: i32 = 366;
    pub const SPECIAL_AIR_N: i32 = 367;
    pub const SPECIAL_AIR_N_HIT: i32 = 368;
    pub const ITEM_PARASOL_OPEN: i32 = 369;
    pub const ITEM_PARASOL_FALL: i32 = 370;
}

/// `ftPeach_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// Whether she still has her float (`has_float`).
    pub const HAS_FLOAT: usize = 0x0;
    /// The float's frames left (`x4`), a float.
    pub const FLOAT_TIME: usize = 0x4;
    /// The last forward smash's state (`attacks4_motion_id`).
    pub const ATTACK_S4_MSID: usize = 0x8;
    /// Her parasol while it's out (`parasol_gobj_0`), and the one she opened (`parasol_gobj_1`).
    pub const PARASOL_0: usize = 0xC;
    pub const PARASOL_1: usize = 0x10;
    /// Whether her air Toad has slowed her this jump (`specialairn_used`).
    pub const SPECIAL_AIR_N_USED: usize = 0x14;
    /// Toad (`toad_gobj`) and the vegetable she's pulling (`veg_gobj`).
    pub const TOAD: usize = 0x18;
    pub const VEG: usize = 0x1C;
}

/// `ftPe_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `floatattack.x0`: she let go of the float during the aerial.
    pub const FLOAT_ATTACK_LET_GO: usize = 0x0;
    /// `specials.x0`: the Peach Bomber was smashed.
    pub const S_SMASH: usize = 0x0;
    /// `specialhi.kind`: the item kind the parasol makes.
    pub const HI_KIND: usize = 0x0;
    /// `specialn.facing_dir`.
    pub const N_FACING_DIR: usize = 0x0;
}

/// `ftPe_DatAttrs`' offsets (0xC0 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0xC0;
    pub const FLOAT_FALL_F_ANIM_START: usize = 0x0;
    pub const FLOAT_FALL_B_ANIM_START: usize = 0x4;
    pub const FLOAT_FALL_ANIM_START_OFFSET: usize = 0x8;
    pub const FLOAT_TIME: usize = 0xC;
    pub const LW_TABLE_COUNT: usize = 0x10;
    pub const LW_TABLE_ODDS: usize = 0x14;
    /// `speciallw_item_table`: (odds, item kind) pairs.
    pub const LW_TABLE: usize = 0x18;
    pub const S_SMASH_FRAMES: usize = 0x30;
    pub const S_X34: usize = 0x34;
    pub const S_START_ACCEL: usize = 0x38;
    pub const S_START_VEL_X: usize = 0x3C;
    pub const S_X40: usize = 0x40;
    pub const S_VEL_X: usize = 0x44;
    pub const S_SMASH_VEL_X: usize = 0x48;
    pub const S_VEL_Y: usize = 0x4C;
    pub const S_X50_GRAVITY: usize = 0x50;
    pub const S_X54: usize = 0x54;
    pub const S_X58_GRAVITY: usize = 0x58;
    pub const S_X5C_TERMINAL: usize = 0x5C;
    pub const S_END_VEL_X: usize = 0x60;
    pub const S_END_VEL_Y: usize = 0x64;
    pub const S_X68: usize = 0x68;
    pub const S_X6C: usize = 0x6C;
    pub const HI_X70: usize = 0x70;
    pub const HI_X74: usize = 0x74;
    pub const HI_X78: usize = 0x78;
    pub const HI_X7C: usize = 0x7C;
    pub const HI_X80: usize = 0x80;
    pub const HI_X84: usize = 0x84;
    pub const HI_X88: usize = 0x88;
    pub const HI_X8C: usize = 0x8C;
    pub const HI_X90: usize = 0x90;
    pub const N_AIR_VEL_X_DIV: usize = 0x94;
    pub const N_X98: usize = 0x98;
    pub const N_AIR_VEL_Y: usize = 0x9C;
    pub const N_XA0: usize = 0xA0;
    pub const N_XA4: usize = 0xA4;
    pub const N_XA8: usize = 0xA8;
    /// Toad's counter box (`xAC`, an `AbsorbDesc`).
    pub const N_XAC: usize = 0xAC;
}

/// `It_Kind_Peach_Explode`, `_Turnip`, `_Parasol`, `_Toad` and `_ToadSpore`.
pub const ITEMS: [i32; 5] = [98, 99, 103, 104, 111];

impl Running<'_> {
    /// A float of the fighter's `ftPe_DatAttrs`, `off` bytes in.
    pub(crate) fn peach_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.peach_attr_word(f, off))
    }

    pub(crate) fn peach_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.peach_attr_word(f, off) as i32
    }

    fn peach_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Peach's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    init::run(g, gobj, a)
        || float::run(g, gobj, a)
        || attack_s4::run(g, gobj, a)
        || special_s::run(g, gobj, a)
        || special_lw::run(g, gobj, a)
        || special_hi::run(g, gobj, a)
        || special_n::run(g, gobj, a)
}

/// Peach's functions other code calls by address (`code::call`): the item hooks and every
/// callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
