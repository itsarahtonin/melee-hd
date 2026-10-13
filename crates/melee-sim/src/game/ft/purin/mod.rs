//! Jigglypuff's own code (`ft/kinds/ftPurin`): Rollout (neutral special), Pound (side
//! special), Sing (up special), Rest (down special), her five double jumps (the common
//! multi-jump states, `ftCo_JumpAerialF1`) and the character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftPurin_MotionVars`), the
//! character's in `u` (`ftPurin_FighterVars`), and the character's attributes
//! (`ftPurinAttributes`, the multi-jump attributes first) in the fighter's attribute block, all
//! read at the decomp's offsets.
//!
//! Other code reaches Jigglypuff's through [`call`] and [`run`] by the original's addresses.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_PURIN`.
pub const KIND_PURIN: i32 = 15;

/// Jigglypuff's action states (`ftPr_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START_R: i32 = 346;
    pub const SPECIAL_N_START_L: i32 = 347;
    pub const SPECIAL_N_LOOP: i32 = 348;
    pub const SPECIAL_N_FULL: i32 = 349;
    pub const SPECIAL_N_RELEASE: i32 = 350;
    pub const SPECIAL_N_TURN: i32 = 351;
    pub const SPECIAL_N_END_R: i32 = 352;
    pub const SPECIAL_N_END_L: i32 = 353;
    pub const SPECIAL_AIR_N_START_R: i32 = 354;
    pub const SPECIAL_AIR_N_START_L: i32 = 355;
    pub const SPECIAL_AIR_N_CHARGE_LOOP: i32 = 356;
    pub const SPECIAL_AIR_N_CHARGE_FULL: i32 = 357;
    pub const SPECIAL_AIR_N_CHARGE_RELEASE: i32 = 358;
    pub const SPECIAL_AIR_N_START_TURN: i32 = 359;
    pub const SPECIAL_AIR_N_END_R: i32 = 360;
    pub const SPECIAL_AIR_N_END_L: i32 = 361;
    pub const SPECIAL_N_HIT: i32 = 362;
    pub const SPECIAL_S: i32 = 363;
    pub const SPECIAL_AIR_S: i32 = 364;
    pub const SPECIAL_HI_L: i32 = 365;
    pub const SPECIAL_AIR_HI_L: i32 = 366;
    pub const SPECIAL_HI_R: i32 = 367;
    pub const SPECIAL_AIR_HI_R: i32 = 368;
    pub const SPECIAL_LW_L: i32 = 369;
    pub const SPECIAL_AIR_LW_L: i32 = 370;
    pub const SPECIAL_LW_R: i32 = 371;
    pub const SPECIAL_AIR_LW_R: i32 = 372;
}

/// Jigglypuff's functions, by address: the callbacks her states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8013_C318;
    pub const ON_USER_DATA_REMOVE: u32 = 0x8013_C340;
    pub const ON_LOAD: u32 = 0x8013_C67C;
    pub const ON_ITEM_PICKUP: u32 = 0x8013_C6EC;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8013_C798;
    pub const ON_ITEM_VISIBLE: u32 = 0x8013_C7E8;
    pub const ON_ITEM_DROP: u32 = 0x8013_C838;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8013_C88C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8013_C8C4;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8013_C908;
    pub const SING_GFX: u32 = 0x8013_C94C;
    pub const SPECIAL_HI_ENTER: u32 = 0x8013_C9C8;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8013_CA98;
    pub const SPECIAL_HI_ANIM: u32 = 0x8013_CB68;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8013_CBE4;
    pub const SPECIAL_HI_IASA: u32 = 0x8013_CC60;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8013_CC64;
    pub const SPECIAL_HI_PHYS: u32 = 0x8013_CC68;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8013_CC88;
    pub const SPECIAL_HI_COLL: u32 = 0x8013_CCA8;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8013_CCE4;
    pub const SPECIAL_HI_8013CD34: u32 = 0x8013_CD34;
    pub const SPECIAL_HI_8013CDD8: u32 = 0x8013_CDD8;
    pub const SPECIAL_HI_8013CE7C: u32 = 0x8013_CE7C;
    pub const SPECIAL_LW_ENTER: u32 = 0x8013_CE8C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8013_CF2C;
    pub const SPECIAL_LW_ANIM: u32 = 0x8013_CFCC;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8013_D008;
    pub const SPECIAL_LW_IASA: u32 = 0x8013_D044;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8013_D048;
    pub const SPECIAL_LW_PHYS: u32 = 0x8013_D04C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8013_D06C;
    pub const SPECIAL_LW_COLL: u32 = 0x8013_D08C;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8013_D0C8;
    pub const SPECIAL_LW_8013D104: u32 = 0x8013_D104;
    pub const SPECIAL_LW_8013D19C: u32 = 0x8013_D19C;
    pub const SPECIAL_S_ENTER: u32 = 0x8013_D234;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8013_D2A0;
    pub const SPECIAL_S_ANIM: u32 = 0x8013_D30C;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8013_D348;
    pub const SPECIAL_S_IASA: u32 = 0x8013_D384;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8013_D388;
    pub const SPECIAL_S_PHYS: u32 = 0x8013_D38C;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8013_D3AC;
    pub const SPECIAL_S_COLL: u32 = 0x8013_D518;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8013_D554;
    pub const SPECIAL_S_8013D590: u32 = 0x8013_D590;
    pub const SPECIAL_S_8013D5F0: u32 = 0x8013_D5F0;
    pub const SPECIAL_S_8013D658: u32 = 0x8013_D658;
    pub const SPECIAL_S_8013D764: u32 = 0x8013_D764;
    pub const SPECIAL_S_8013D8B0: u32 = 0x8013_D8B0;
    pub const SPECIAL_S_8013D8E4: u32 = 0x8013_D8E4;
    pub const SPECIAL_S_8013DA24: u32 = 0x8013_DA24;
    pub const SPECIAL_S_8013DC64: u32 = 0x8013_DC64;
    pub const SPECIAL_S_8013DD54: u32 = 0x8013_DD54;
    pub const SPECIAL_N_ENTER: u32 = 0x8013_DEA8;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8013_DF60;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8013_E014;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x8013_E0F0;
    pub const SPECIAL_N_FULL_ANIM: u32 = 0x8013_E2A0;
    pub const SPECIAL_N_RELEASE_ANIM: u32 = 0x8013_E410;
    pub const SPECIAL_N_TURN_ANIM: u32 = 0x8013_E7E0;
    pub const SPECIAL_N_END_ANIM: u32 = 0x8013_EAD8;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8013_EDB0;
    pub const SPECIAL_AIR_N_CHARGE_LOOP_ANIM: u32 = 0x8013_EE84;
    pub const SPECIAL_AIR_N_CHARGE_FULL_ANIM: u32 = 0x8013_F034;
    pub const SPECIAL_AIR_N_CHARGE_RELEASE_ANIM: u32 = 0x8013_F1A4;
    pub const SPECIAL_AIR_N_START_TURN_ANIM: u32 = 0x8013_F708;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x8013_F9C0;
    pub const SPECIAL_N_HIT_ANIM: u32 = 0x8013_FCAC;
    pub const SPECIAL_N_START_IASA: u32 = 0x8013_FF00;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x8013_FF04;
    pub const SPECIAL_N_FULL_IASA: u32 = 0x8014_0064;
    pub const SPECIAL_N_RELEASE_IASA: u32 = 0x8014_01C4;
    pub const SPECIAL_N_TURN_IASA: u32 = 0x8014_0344;
    pub const SPECIAL_N_END_IASA: u32 = 0x8014_0348;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8014_034C;
    pub const SPECIAL_AIR_N_CHARGE_LOOP_IASA: u32 = 0x8014_0350;
    pub const SPECIAL_AIR_N_CHARGE_FULL_IASA: u32 = 0x8014_04B0;
    pub const SPECIAL_AIR_N_CHARGE_RELEASE_IASA: u32 = 0x8014_0610;
    pub const SPECIAL_AIR_N_START_TURN_IASA: u32 = 0x8014_0614;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x8014_0618;
    pub const SPECIAL_N_HIT_IASA: u32 = 0x8014_061C;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8014_0620;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x8014_0650;
    pub const SPECIAL_N_FULL_PHYS: u32 = 0x8014_0680;
    pub const SPECIAL_N_RELEASE_PHYS: u32 = 0x8014_06B0;
    pub const SPECIAL_N_TURN_PHYS: u32 = 0x8014_08B8;
    pub const SPECIAL_N_END_PHYS: u32 = 0x8014_0BAC;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8014_0BE8;
    pub const SPECIAL_AIR_N_CHARGE_LOOP_PHYS: u32 = 0x8014_0C18;
    pub const SPECIAL_AIR_N_CHARGE_FULL_PHYS: u32 = 0x8014_0C48;
    pub const SPECIAL_AIR_N_CHARGE_RELEASE_PHYS: u32 = 0x8014_0C78;
    pub const SPECIAL_AIR_N_START_TURN_PHYS: u32 = 0x8014_0DF8;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x8014_0F10;
    pub const SPECIAL_N_HIT_PHYS: u32 = 0x8014_0F40;
    pub const SPECIAL_N_START_COLL: u32 = 0x8014_0FA4;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x8014_1054;
    pub const SPECIAL_N_FULL_COLL: u32 = 0x8014_1154;
    pub const SPECIAL_N_RELEASE_COLL: u32 = 0x8014_1254;
    pub const SPECIAL_N_TURN_COLL: u32 = 0x8014_15F4;
    pub const SPECIAL_N_END_COLL: u32 = 0x8014_16D0;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8014_1730;
    pub const SPECIAL_AIR_N_CHARGE_LOOP_COLL: u32 = 0x8014_17E0;
    pub const SPECIAL_AIR_N_CHARGE_FULL_COLL: u32 = 0x8014_18E0;
    pub const SPECIAL_AIR_N_CHARGE_RELEASE_COLL: u32 = 0x8014_19E0;
    pub const SPECIAL_AIR_N_START_TURN_COLL: u32 = 0x8014_1FB8;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x8014_2070;
    pub const SPECIAL_N_HIT_COLL: u32 = 0x8014_20D0;
    pub const SPECIAL_N_8014222C: u32 = 0x8014_222C;
}

/// `ftPurin_FighterVars`' offsets in `u`: Pound's uses in the air, the model's scale Rollout
/// keeps, and the hat's model, DObj list and part visibility.
pub(crate) mod u {
    pub const POUNDS: usize = 0x0;
    pub const SCALE: usize = 0x4;
    pub const HAT: usize = 0x10;
}

/// `ftPurin_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `specialhi.x0`: whether Sing puts to sleep on Pokémon Stadium's zoomed screen.
    pub const HI_SLEEP: usize = 0x0;
    // `specialn`.
    pub const N_FRAMES: usize = 0x0;
    pub const N_X4: usize = 0x4;
    pub const N_SCALE_FRAME: usize = 0x8;
    pub const N_HIT_TOGGLE: usize = 0xC;
    pub const N_TURN_VEL: usize = 0x10;
    pub const N_ROLL: usize = 0x14;
    pub const N_SPEED: usize = 0x18;
    pub const N_X1C: usize = 0x1C;
    pub const N_FACING_DIR: usize = 0x20;
    pub const N_TURN_FRAMES: usize = 0x24;
    pub const N_X28: usize = 0x28;
    pub const N_CHARGE: usize = 0x2C;
    pub const N_FULL: usize = 0x30;
    pub const N_DIR: usize = 0x34;
    pub const N_LAST_ROLL: usize = 0x38;
}

/// `ftPurinAttributes`' offsets (0x100 bytes, `Fighter_x2D0_t` first).
pub(crate) mod attr {
    pub const SIZE: usize = 0x100;
    pub const N_FRAMES: usize = 0x34;
    pub const N_HIT_FRAMES: usize = 0x38;
    pub const N_AIR_GRAVITY: usize = 0x3C;
    pub const N_AIR_TERMINAL: usize = 0x40;
    pub const N_X44: usize = 0x44;
    pub const N_GROUND_MAX: usize = 0x4C;
    pub const N_GROUND_MAX2: usize = 0x50;
    pub const N_X54: usize = 0x54;
    pub const N_AIR_DECEL: usize = 0x58;
    pub const N_AIR_MIN: usize = 0x5C;
    pub const N_TURN_STICK: usize = 0x68;
    pub const N_TURN_SPIN: usize = 0x6C;
    pub const N_TURN_GFX_EVERY: usize = 0x70;
    pub const N_TURN_SLOW: usize = 0x74;
    pub const N_BOUNCE: usize = 0x78;
    pub const N_BOUNCE_MIN: usize = 0x7C;
    pub const N_DAMAGE_BASE: usize = 0x80;
    pub const N_DAMAGE_MUL: usize = 0x84;
    pub const N_HIT_VEL_X: usize = 0x88;
    pub const N_HIT_VEL_Y: usize = 0x8C;
    pub const N_END_VEL_X: usize = 0x90;
    pub const N_END_VEL_Y: usize = 0x94;
    pub const N_SPIN: usize = 0x98;
    pub const N_HIT_TOGGLE: usize = 0x9C;
    pub const N_CHARGE_START: usize = 0xA0;
    pub const N_CHARGE_MAX: usize = 0xA4;
    pub const N_CHARGE_RATE: usize = 0xA8;
    pub const N_CHARGE_SPIN: usize = 0xAC;
    pub const N_DECAY: usize = 0xB4;
    pub const N_CHARGE_END: usize = 0xB8;
    pub const N_AIR_SPIN: usize = 0xBC;
    pub const N_SPEED_MUL: usize = 0xC0;
    pub const N_TURN_ACCEL: usize = 0xC4;
    pub const N_SLOPE: usize = 0xC8;
    pub const N_HIT_MIN_SPEED: usize = 0xCC;
    pub const N_TURN_DONE: usize = 0xD0;
    pub const N_WALL_BOUNCE: usize = 0xD4;
    pub const N_LANDING_LAG: usize = 0xD8;
    pub const S_STICK_MIN: usize = 0xDC;
    pub const S_STICK_MAX: usize = 0xE0;
    pub const S_ANGLE: usize = 0xE4;
    pub const S_SPEED: usize = 0xF0;
    pub const S_DECEL: usize = 0xF4;
}

/// `FtPart_TopN`, `FtPart_YRotN`, `FtPart_WaistN`.
pub(crate) const PART_TOP_N: i32 = 0;
pub(crate) const PART_Y_ROT_N: i32 = 3;
pub(crate) const PART_WAIST_N: i32 = 5;

impl Running<'_> {
    /// A float of the fighter's `ftPurinAttributes`, `off` bytes in.
    pub(crate) fn purin_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.purin_attr_word(f, off))
    }

    pub(crate) fn purin_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.purin_attr_word(f, off) as i32
    }

    fn purin_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Jigglypuff's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    init::run(g, gobj, a)
        || special_n::run(g, gobj, a)
        || special_s::run(g, gobj, a)
        || special_hi::run(g, gobj, a)
        || special_lw::run(g, gobj, a)
}

/// Jigglypuff's functions other code calls by address (`code::call`): the item hooks and every
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
