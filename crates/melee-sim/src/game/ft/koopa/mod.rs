//! Bowser's own code (`ft/kinds/ftKoopa`): his fire breath (neutral special) with its fuel and
//! flame size, Koopa Klaw (side special) with its bite and throws, Whirling Fortress (up
//! special) and Bowser Bomb (down special), and the character's callbacks. His super armor is
//! the common damage code's (`dmg.armor0`, which his spawn sets), the Klaw's victim's states
//! are in `ft::co::capture_koopa`, and the flame is an item (`it::kinds::koopaflame`).
//!
//! A state's variables live in the fighter's `mv` union (`ftKoopa_MotionVars`), the
//! character's in `u` (`ftKoopa_FighterVars`), and the character's attributes
//! (`ftKoopaAttributes`) in the fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Bowser's through [`call`] and [`run`] by the original's addresses.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Koopa`.
pub const KIND_KOOPA: i32 = 5;
/// `Ft_Kind_GKoopa`, Giga Bowser, who runs the same code.
pub const KIND_GKOOPA: i32 = 31;

/// Bowser's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8013_2A0C;
    pub const UNK_MOTION_STATES3: u32 = 0x8013_2A64;
    pub const ON_LOAD: u32 = 0x8013_2ABC;
    pub const DAMAGE_CB: u32 = 0x8013_2B38;
    pub const ON_ITEM_PICKUP: u32 = 0x8013_2B3C;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8013_2C1C;
    pub const ON_ITEM_VISIBLE: u32 = 0x8013_2C64;
    pub const ON_ITEM_DROP: u32 = 0x8013_2CAC;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8013_2D00;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8013_2D38;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8013_2D7C;

    // The Klaw's attributes the victim reads, and the throw's release.
    pub const S_GRAB_TIMER: u32 = 0x8013_2DC0;
    pub const S_MASH_DEC: u32 = 0x8013_2DD0;
    pub const S_MASH: u32 = 0x8013_2DE0;
    pub const S_SHAKE_FRAMES: u32 = 0x8013_2DF0;
    pub const S_SHAKE_RATE: u32 = 0x8013_2E00;
    pub const S_SHAKE: u32 = 0x8013_2E10;
    pub const S_SHAKE_MAX: u32 = 0x8013_2E20;
    pub const S_THROW_B: u32 = 0x8013_2E30;

    // Koopa Klaw.
    pub const SPECIAL_S_ENTER: u32 = 0x8013_2EFC;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8013_2F94;
    pub const S_ON_GRAB: u32 = 0x8013_302C;
    pub const AIR_S_ON_GRAB: u32 = 0x8013_30E4;
    pub const S_START_TO_AIR: u32 = 0x8013_319C;
    pub const AIR_S_START_TO_GROUND: u32 = 0x8013_322C;
    pub const S_HIT_OFF_LEDGE: u32 = 0x8013_32C4;
    pub const AIR_S_HIT_TO_GROUND: u32 = 0x8013_3324;
    pub const S_WAIT_OFF_LEDGE: u32 = 0x8013_3398;
    pub const AIR_S_WAIT_TO_GROUND: u32 = 0x8013_33F8;
    pub const S_END_OFF_LEDGE: u32 = 0x8013_3484;
    pub const AIR_S_END_F_TO_GROUND: u32 = 0x8013_34E4;
    pub const AIR_S_END_B_TO_GROUND: u32 = 0x8013_359C;
    pub const SPECIAL_S_START_ANIM: u32 = 0x8013_3654;
    pub const SPECIAL_AIR_S_START_ANIM: u32 = 0x8013_3690;
    pub const SPECIAL_S_HIT_ANIM: u32 = 0x8013_36CC;
    pub const SPECIAL_AIR_S_HIT_ANIM: u32 = 0x8013_383C;
    pub const SPECIAL_S_WAIT_ANIM: u32 = 0x8013_39AC;
    pub const SPECIAL_AIR_S_WAIT_ANIM: u32 = 0x8013_39B0;
    pub const SPECIAL_S_END_F_ANIM: u32 = 0x8013_39B4;
    pub const SPECIAL_S_END_B_ANIM: u32 = 0x8013_3A90;
    pub const SPECIAL_AIR_S_END_F_ANIM: u32 = 0x8013_3AD4;
    pub const SPECIAL_AIR_S_END_B_ANIM: u32 = 0x8013_3BB0;
    pub const SPECIAL_S_HIT_IASA: u32 = 0x8013_3BF4;
    pub const SPECIAL_AIR_S_HIT_IASA: u32 = 0x8013_3D20;
    pub const SPECIAL_S_WAIT_IASA: u32 = 0x8013_3E4C;
    pub const SPECIAL_AIR_S_WAIT_IASA: u32 = 0x8013_4038;
    pub const SPECIAL_S_START_PHYS: u32 = 0x8013_4224;
    pub const SPECIAL_AIR_S_START_PHYS: u32 = 0x8013_4244;
    pub const SPECIAL_S_HIT_PHYS: u32 = 0x8013_4288;
    pub const SPECIAL_AIR_S_HIT_PHYS: u32 = 0x8013_42A8;
    pub const SPECIAL_S_WAIT_PHYS: u32 = 0x8013_42C8;
    pub const SPECIAL_AIR_S_WAIT_PHYS: u32 = 0x8013_42E8;
    pub const SPECIAL_S_END_F_PHYS: u32 = 0x8013_4308;
    pub const SPECIAL_S_END_B_PHYS: u32 = 0x8013_4328;
    pub const SPECIAL_AIR_S_END_F_PHYS: u32 = 0x8013_4348;
    pub const SPECIAL_AIR_S_END_B_PHYS: u32 = 0x8013_4368;
    pub const SPECIAL_S_START_COLL: u32 = 0x8013_4388;
    pub const SPECIAL_AIR_S_START_COLL: u32 = 0x8013_43B0;
    pub const SPECIAL_S_HIT_COLL: u32 = 0x8013_43D8;
    pub const SPECIAL_AIR_S_HIT_COLL: u32 = 0x8013_4400;
    pub const SPECIAL_S_WAIT_COLL: u32 = 0x8013_4428;
    pub const SPECIAL_AIR_S_WAIT_COLL: u32 = 0x8013_4450;
    pub const SPECIAL_S_END_F_COLL: u32 = 0x8013_4478;
    pub const SPECIAL_S_END_B_COLL: u32 = 0x8013_44A0;
    pub const SPECIAL_AIR_S_END_F_COLL: u32 = 0x8013_44C8;
    pub const SPECIAL_AIR_S_END_B_COLL: u32 = 0x8013_44F0;

    // Bowser Bomb.
    pub const LW_LANDING_EFFECT: u32 = 0x8013_4518;
    pub const LW_FALL_EFFECT: u32 = 0x8013_4590;
    pub const SPECIAL_LW_ENTER: u32 = 0x8013_460C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8013_46A4;
    pub const SPECIAL_LW_ANIM: u32 = 0x8013_474C;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8013_4788;
    pub const SPECIAL_LW_PHYS: u32 = 0x8013_47C4;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8013_4804;
    pub const SPECIAL_LW_COLL: u32 = 0x8013_48A0;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8013_48C0;
    pub const LW_FALL: u32 = 0x8013_4988;
    pub const SPECIAL_LW_LANDING_ANIM: u32 = 0x8013_49C4;
    pub const SPECIAL_LW_LANDING_PHYS: u32 = 0x8013_4A00;
    pub const SPECIAL_LW_LANDING_COLL: u32 = 0x8013_4A20;
    pub const LW_LANDING: u32 = 0x8013_4A5C;

    // Fire breath (named for the down special's file in the decomp's symbols).
    pub const N_FLAME: u32 = 0x8013_4ACC;
    pub const N_REFUEL: u32 = 0x8013_4D78;
    pub const N_MAX_SPEED: u32 = 0x8013_4DE0;
    pub const N_MAX_SIZE: u32 = 0x8013_4E1C;
    pub const SPECIAL_N_ENTER: u32 = 0x8013_4E58;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8013_4ED0;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8013_4F48;
    pub const SPECIAL_N_ANIM: u32 = 0x8013_4F9C;
    pub const SPECIAL_N_END_ANIM: u32 = 0x8013_5040;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8013_507C;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8013_50D0;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x8013_5174;
    pub const SPECIAL_N_START_IASA: u32 = 0x8013_51B0;
    pub const SPECIAL_N_IASA: u32 = 0x8013_51B4;
    pub const SPECIAL_N_END_IASA: u32 = 0x8013_52F0;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8013_52F4;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8013_52F8;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x8013_5434;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8013_5438;
    pub const SPECIAL_N_PHYS: u32 = 0x8013_5458;
    pub const SPECIAL_N_END_PHYS: u32 = 0x8013_5478;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8013_5498;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8013_54B8;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x8013_54D8;
    pub const SPECIAL_N_START_COLL: u32 = 0x8013_54F8;
    pub const SPECIAL_N_COLL: u32 = 0x8013_5564;
    pub const SPECIAL_N_END_COLL: u32 = 0x8013_55D0;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8013_563C;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8013_56A8;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x8013_5714;
    /// Whirling Fortress's shell: intangible or not by the script (in the neutral special's
    /// file).
    pub const HI_SHELL: u32 = 0x8013_5780;

    // Whirling Fortress.
    pub const SPECIAL_HI_ENTER: u32 = 0x8013_5A2C;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8013_5B04;
    pub const SPECIAL_HI_ANIM: u32 = 0x8013_5C08;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8013_5C60;
    pub const SPECIAL_HI_IASA: u32 = 0x8013_5D78;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8013_5D7C;
    pub const SPECIAL_HI_PHYS: u32 = 0x8013_5D80;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8013_5DE0;
    pub const SPECIAL_HI_COLL: u32 = 0x8013_5E4C;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8013_5F6C;
}

/// Bowser's action states (`ftKp_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 341;
    pub const SPECIAL_N: i32 = 342;
    pub const SPECIAL_N_END: i32 = 343;
    pub const SPECIAL_AIR_N_START: i32 = 344;
    pub const SPECIAL_AIR_N: i32 = 345;
    pub const SPECIAL_AIR_N_END: i32 = 346;
    pub const SPECIAL_S_START: i32 = 347;
    pub const SPECIAL_S_HIT0_0: i32 = 348;
    pub const SPECIAL_S_HIT1: i32 = 349;
    pub const SPECIAL_S_HIT0_1: i32 = 350;
    pub const SPECIAL_S_END_F: i32 = 351;
    pub const SPECIAL_S_END_B: i32 = 352;
    pub const SPECIAL_AIR_S_START: i32 = 353;
    pub const SPECIAL_AIR_S_HIT0_0: i32 = 354;
    pub const SPECIAL_AIR_S_HIT1: i32 = 355;
    pub const SPECIAL_AIR_S_HIT0_1: i32 = 356;
    pub const SPECIAL_AIR_S_END_F: i32 = 357;
    pub const SPECIAL_AIR_S_END_B: i32 = 358;
    pub const SPECIAL_HI: i32 = 359;
    pub const SPECIAL_AIR_HI: i32 = 360;
    pub const SPECIAL_LW: i32 = 361;
    pub const SPECIAL_AIR_LW: i32 = 362;
    pub const SPECIAL_LW_LANDING: i32 = 363;
}

/// `ftKoopa_FighterVars`' offsets in `u`: the flame's speed (`x222C`) and size (`x2230`), which
/// the breath uses up and time refills.
pub(crate) mod u {
    pub const FLAME_SPEED: usize = 0x0;
    pub const FLAME_SIZE: usize = 0x4;
}

/// `ftKoopa_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// The neutral special's (`specialn`): the frame within each three (`x0`), the flames'
    /// attack id (`x4`), the last flame's angle group (`facing_dir`), the frames breathed
    /// (`flame_timer`), the loops it still runs B up or not (`x10`), the frame within twelve
    /// (`x14`) and within the quake's period (`x18`).
    pub const N_X0: usize = 0x0;
    pub const N_ATTACK_ID: usize = 0x4;
    pub const N_DIR: usize = 0x8;
    pub const N_TIMER: usize = 0xC;
    pub const N_X10: usize = 0x10;
    pub const N_X14: usize = 0x14;
    pub const N_X18: usize = 0x18;
    /// The side special's (`unk1`, `specials`): another bite asked for (`x0`, `b_held`),
    /// biting (`x4`), the stick's throw (`facing_dir`, `x8`) and the back throw's turn (`xC`).
    pub const S_X0: usize = 0x0;
    pub const S_X4: usize = 0x4;
    pub const S_X8: usize = 0x8;
    pub const S_XC: usize = 0xC;
    /// The up special's: frames spun (`mv.co.walk.middle_anim_frame`, `xC`) and on the ground
    /// (`specials.x10`, which the effect's tilt reads).
    pub const HI_FRAMES: usize = 0xC;
    pub const HI_GROUNDED: usize = 0x10;
}

/// `ftKoopaAttributes`' offsets (0xA0 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0xA0;
    /// His super armor (`dmg.armor0` at each spawn).
    pub const ARMOR: usize = 0x0;
    /// The fire breath: the frames it breathes at least (`flame_limit`), the speed's and
    /// size's refill each frame (`x8`, `xC`), their full (`x10`, `x18`) and least (`x14`,
    /// `x1C`), the quake's period (`x20`, an `int`) and the flame's offset ahead and up
    /// (`x24`, `x28`).
    pub const FLAME_LIMIT: usize = 0x4;
    pub const X8: usize = 0x8;
    pub const XC: usize = 0xC;
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    /// Koopa Klaw: the bite's damage (`x2C`, an `int`), the stick's throw threshold (`x30`),
    /// the victim's shake (`x34`) and its range (`x38`), the shake's rate and frames (`x3C`,
    /// `x40`), the mash's (`x44`), the timer's drain (`x48`) and the grab's timer (`x4C`).
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X34: usize = 0x34;
    pub const X38: usize = 0x38;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    /// Whirling Fortress: the air start's rise (`x54`), its gravity and fall (`x58`, `x5C`),
    /// the ground and air speeds (`x60`, `x64`) and accelerations (`x68`, `x6C`), the landing's
    /// window (`x70`) and frame shift (`x78`), and the helpless fall's landing lag (`x7C`).
    pub const X54: usize = 0x54;
    pub const X58: usize = 0x58;
    pub const X5C: usize = 0x5C;
    pub const X60: usize = 0x60;
    pub const X64: usize = 0x64;
    pub const X68: usize = 0x68;
    pub const X6C: usize = 0x6C;
    pub const X70: usize = 0x70;
    pub const X78: usize = 0x78;
    pub const X7C: usize = 0x7C;
    /// Bowser Bomb: the air start's speed kept (`x80`, `x84`), the drift's friction (`x88`),
    /// the fall's gravity, top speed (`x8C`, `x90`) and its least speed (`x94`).
    pub const X80: usize = 0x80;
    pub const X84: usize = 0x84;
    pub const X88: usize = 0x88;
    pub const X8C: usize = 0x8C;
    pub const X90: usize = 0x90;
    pub const X94: usize = 0x94;
}

impl Running<'_> {
    /// A float of the fighter's `ftKoopaAttributes`, `off` bytes in.
    pub(crate) fn kp_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftKoopaAttributes`.
    pub(crate) fn kp_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }
}

/// Runs one of Bowser's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Bowser's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    if let Some(v) = special_s::call(r, f, args) {
        return Some(v);
    }
    if let Some(v) = special_n::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
