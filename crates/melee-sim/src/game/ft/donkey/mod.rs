//! Donkey Kong's own code (`ft/kinds/ftDonkey`): the Giant Punch (neutral special), the
//! Headbutt (side special), the Spinning Kong (up special), the Hand Slap (down special) and
//! the character's callbacks. Carrying a fighter after his forward throw runs the common cargo
//! states (`ft::co::cargo`) from his own state ids, which his attributes name.
//!
//! A state's variables live in the fighter's `mv` union (`ftDonkey_MotionVars`), the Giant
//! Punch's swings in `u` (`ftDonkey_FighterVars`), and the character's attributes
//! (`ftDonkeyAttributes`) in the fighter's attribute block, all read at the decomp's offsets.
//! Carrying a heavy item runs his own heavy states (`heavy`, 341 to 350), counted from the same
//! first state.

pub mod heavy;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Donkey`.
pub const KIND_DONKEY: i32 = 3;

/// Donkey Kong's action states (`ftDk_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 369;
    pub const SPECIAL_N_LOOP: i32 = 370;
    pub const SPECIAL_N_CANCEL: i32 = 371;
    pub const SPECIAL_N: i32 = 372;
    pub const SPECIAL_N_FULL: i32 = 373;
    pub const SPECIAL_AIR_N_START: i32 = 374;
    pub const SPECIAL_AIR_N_LOOP: i32 = 375;
    pub const SPECIAL_AIR_N_CANCEL: i32 = 376;
    pub const SPECIAL_AIR_N: i32 = 377;
    pub const SPECIAL_AIR_N_FULL: i32 = 378;
    pub const SPECIAL_S: i32 = 379;
    pub const SPECIAL_AIR_S: i32 = 380;
    pub const SPECIAL_HI: i32 = 381;
    pub const SPECIAL_AIR_HI: i32 = 382;
    pub const SPECIAL_LW_START: i32 = 383;
    pub const SPECIAL_LW_LOOP: i32 = 384;
    pub const SPECIAL_LW_END0: i32 = 385;
}

/// Donkey Kong's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8010_D740;
    pub const ON_DAMAGE: u32 = 0x8010_D774;
    pub const ON_ITEM_PICKUP: u32 = 0x8010_D7A8;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8010_D888;
    pub const ON_ITEM_VISIBLE: u32 = 0x8010_D8D0;
    pub const ON_ITEM_DROP: u32 = 0x8010_D918;
    pub const UNK_MOTION_STATES4: u32 = 0x8010_D96C;
    pub const ON_LOAD: u32 = 0x8010_D9AC;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8010_DA74;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8010_DAB4;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8010_DAF8;
    pub const HAND_SLAP_HITS: u32 = 0x8010_DB3C;
    // The down special.
    pub const SPECIAL_LW_ENTER: u32 = 0x8010_DC80;
    pub const SPECIAL_LW_START_ANIM: u32 = 0x8010_DCD8;
    pub const SPECIAL_LW_START_IASA: u32 = 0x8010_DD14;
    pub const SPECIAL_LW_START_PHYS: u32 = 0x8010_DD18;
    pub const SPECIAL_LW_START_COLL: u32 = 0x8010_DD38;
    pub const SPECIAL_LW_LOOP_ANIM: u32 = 0x8010_DD74;
    pub const SPECIAL_LW_LOOP_IASA: u32 = 0x8010_DDDC;
    pub const SPECIAL_LW_LOOP_PHYS: u32 = 0x8010_DDF8;
    pub const SPECIAL_LW_LOOP_COLL: u32 = 0x8010_DE18;
    pub const SPECIAL_LW_ON_CHANGE: u32 = 0x8010_DE54;
    pub const SPECIAL_LW_END0_ANIM: u32 = 0x8010_DF5C;
    pub const SPECIAL_LW_END0_IASA: u32 = 0x8010_DF98;
    pub const SPECIAL_LW_END0_PHYS: u32 = 0x8010_DF9C;
    pub const SPECIAL_LW_END0_COLL: u32 = 0x8010_DFBC;
    pub const SPECIAL_LW_END1_ANIM: u32 = 0x8010_E030;
    pub const SPECIAL_LW_END1_IASA: u32 = 0x8010_E06C;
    pub const SPECIAL_LW_END1_PHYS: u32 = 0x8010_E070;
    pub const SPECIAL_LW_END1_COLL: u32 = 0x8010_E090;
    pub const SPECIAL_S_EFFECT: u32 = 0x8010_E0CC;
    pub const SPECIAL_AIR_S_EFFECT: u32 = 0x8010_E148;
    // The side special.
    pub const SPECIAL_S_ENTER: u32 = 0x8010_E1C4;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8010_E234;
    pub const SPECIAL_S_ANIM: u32 = 0x8010_E2BC;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8010_E2F8;
    pub const SPECIAL_S_IASA: u32 = 0x8010_E334;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8010_E338;
    pub const SPECIAL_S_PHYS: u32 = 0x8010_E33C;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8010_E35C;
    pub const SPECIAL_S_COLL: u32 = 0x8010_E3BC;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8010_E428;
    // The neutral special.
    pub const SPECIAL_N_ENTER: u32 = 0x8010_E574;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8010_E69C;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8010_E7B4;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x8010_E840;
    pub const SPECIAL_N_CANCEL_ANIM: u32 = 0x8010_E8E0;
    pub const SPECIAL_N_ANIM: u32 = 0x8010_E930;
    pub const SPECIAL_N_FULL_ANIM: u32 = 0x8010_EB0C;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8010_EC5C;
    pub const SPECIAL_AIR_N_LOOP_ANIM: u32 = 0x8010_ECE8;
    pub const SPECIAL_AIR_N_CANCEL_ANIM: u32 = 0x8010_ED88;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8010_EDD8;
    pub const SPECIAL_AIR_N_FULL_ANIM: u32 = 0x8010_EF7C;
    pub const SPECIAL_N_START_IASA: u32 = 0x8010_F094;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x8010_F098;
    pub const SPECIAL_N_CANCEL_IASA: u32 = 0x8010_F1D8;
    pub const SPECIAL_N_IASA: u32 = 0x8010_F1DC;
    pub const SPECIAL_N_FULL_IASA: u32 = 0x8010_F1E0;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8010_F1E4;
    pub const SPECIAL_AIR_N_LOOP_IASA: u32 = 0x8010_F1E8;
    pub const SPECIAL_AIR_N_CANCEL_IASA: u32 = 0x8010_F31C;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8010_F320;
    pub const SPECIAL_AIR_N_FULL_IASA: u32 = 0x8010_F324;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8010_F328;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x8010_F348;
    pub const SPECIAL_N_CANCEL_PHYS: u32 = 0x8010_F368;
    pub const SPECIAL_N_PHYS: u32 = 0x8010_F388;
    pub const SPECIAL_N_FULL_PHYS: u32 = 0x8010_F3A8;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8010_F3C8;
    pub const SPECIAL_AIR_N_LOOP_PHYS: u32 = 0x8010_F3E8;
    pub const SPECIAL_AIR_N_CANCEL_PHYS: u32 = 0x8010_F408;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8010_F428;
    pub const SPECIAL_AIR_N_FULL_PHYS: u32 = 0x8010_F448;
    pub const SPECIAL_N_START_COLL: u32 = 0x8010_F468;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x8010_F50C;
    pub const SPECIAL_N_CANCEL_COLL: u32 = 0x8010_F5B0;
    pub const SPECIAL_N_COLL: u32 = 0x8010_F654;
    pub const SPECIAL_N_FULL_COLL: u32 = 0x8010_F6F8;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8010_F79C;
    pub const SPECIAL_AIR_N_LOOP_COLL: u32 = 0x8010_F840;
    pub const SPECIAL_AIR_N_CANCEL_COLL: u32 = 0x8010_F8E4;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8010_F988;
    pub const SPECIAL_AIR_N_FULL_COLL: u32 = 0x8010_FA2C;
    pub const SPECIAL_N_DESTROY_EFFECTS: u32 = 0x8010_FAD0;
    pub const SPECIAL_N_DESTROY_EFFECTS_PLUS: u32 = 0x8010_FAF0;
    // The up special.
    pub const SPECIAL_HI_ENTER: u32 = 0x8010_FB1C;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8010_FBFC;
    pub const SPECIAL_HI_ANIM: u32 = 0x8010_FCD4;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8010_FD10;
    pub const SPECIAL_HI_IASA: u32 = 0x8010_FD9C;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8010_FDA0;
    pub const SPECIAL_HI_PHYS: u32 = 0x8010_FDA4;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8010_FDEC;
    pub const SPECIAL_HI_COLL: u32 = 0x8010_FE60;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8010_FF14;
    pub const SPECIAL_HI_DESTROY_EFFECTS: u32 = 0x8011_0074;
}

/// `ftDonkeyAttributes`' offsets.
pub(crate) mod attr {
    pub const SIZE: usize = 0x74;
    /// The cargo walks' animation lengths (`x8` to `x10`, written at creation) and speed
    /// scales (`x14` to `x1C`); `ft::co::cargo` reads the rest of the cargo attributes.
    pub const CARGO_WALK_FRAMES: usize = 0x8;
    pub const CARGO_WALK_RATES: usize = 0x14;
    /// The Giant Punch.
    pub const MAX_ARM_SWINGS: usize = 0x2C;
    pub const DAMAGE_PER_SWING: usize = 0x30;
    pub const PUNCH_HORIZONTAL_VEL: usize = 0x34;
    pub const SPECIAL_N_LANDING_LAG: usize = 0x38;
    /// The Headbutt.
    pub const MIN_STICK_X_MOMENTUM: usize = 0x3C;
    pub const MOMENTUM_TRANSITION_MODIFIER: usize = 0x40;
    pub const SPECIAL_S_AERIAL_GRAVITY: usize = 0x44;
    /// The Spinning Kong.
    pub const SPECIAL_HI_AERIAL_VERTICAL_VELOCITY: usize = 0x4C;
    pub const SPECIAL_HI_AERIAL_GRAVITY: usize = 0x50;
    pub const SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY: usize = 0x54;
    pub const SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY: usize = 0x58;
    pub const SPECIAL_HI_GROUNDED_MOBILITY: usize = 0x5C;
    pub const SPECIAL_HI_AERIAL_MOBILITY: usize = 0x60;
    pub const SPECIAL_HI_LANDING_LAG: usize = 0x64;
    /// The Hand Slap's hitboxes: their spacing, their reach ahead and the walls a walk along
    /// the floor may climb.
    pub const SPECIAL_LW_SPACING: usize = 0x68;
    pub const SPECIAL_LW_REACH: usize = 0x6C;
    pub const SPECIAL_LW_MAX_WALL: usize = 0x70;
}

/// The `mv` offsets of `ftDonkey_MotionVars`' members.
pub(crate) mod mv {
    /// `specialn`: a cancel asked for (`x0`), the lunge's phase (`x4`), the effect's phase
    /// (`x8`), the swings let go with (`xC`), and the two hitboxes' base damage (`x10`,
    /// `x14`, -1 until read).
    pub const N_CANCEL: usize = 0x0;
    pub const N_LUNGE: usize = 0x4;
    pub const N_EFFECT: usize = 0x8;
    pub const N_SWINGS: usize = 0xC;
    pub const N_DAMAGE0: usize = 0x10;
    pub const N_DAMAGE1: usize = 0x14;
    /// `speciallw`: another slap asked for.
    pub const LW_AGAIN: usize = 0x0;
}

/// `ftDonkey_FighterVars::x222C`: the Giant Punch's swings stored.
pub(crate) const U_SWINGS: usize = 0x0;

/// `FtPart_TopN` and `FtPart_TransN`.
pub(crate) mod part {
    pub const TOP_N: i32 = 0;
    pub const TRANS_N: i32 = 1;
}

impl Running<'_> {
    /// A float of the fighter's `ftDonkeyAttributes`, `off` bytes in.
    pub(crate) fn dk_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftDonkeyAttributes`.
    pub(crate) fn dk_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }
}

/// Runs one of Donkey Kong's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is
/// one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    heavy::run(g, gobj, addr)
        || init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Donkey Kong's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
