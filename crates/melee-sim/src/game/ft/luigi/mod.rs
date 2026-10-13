//! Luigi's own code (`ft/kinds/ftLuigi`): the Fireball (neutral special), the Green Missile
//! (side special), the Super Jump Punch (up special), the Luigi Cyclone (down special) and the
//! character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftLuigi_MotionVars`), the Cyclone's
//! air rise in `u` (`ftLuigi_FighterVars`), and the character's attributes
//! (`ftLuigiAttributes`) in the fighter's attribute block, all read at the decomp's offsets.

pub mod init;
pub mod shared;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Luigi`.
pub const KIND_LUIGI: i32 = 17;

/// Luigi's action states (`ftLg_MS_*`).
pub mod ms {
    pub const SPECIAL_N: i32 = 341;
    pub const SPECIAL_AIR_N: i32 = 342;
    pub const SPECIAL_S_START: i32 = 343;
    pub const SPECIAL_S_HOLD: i32 = 344;
    pub const SPECIAL_S2: i32 = 345;
    pub const SPECIAL_S_END: i32 = 346;
    pub const SPECIAL_S: i32 = 347;
    pub const SPECIAL_S_MISFIRE: i32 = 348;
    pub const SPECIAL_AIR_S_START: i32 = 349;
    pub const SPECIAL_AIR_S_HOLD: i32 = 350;
    pub const SPECIAL_AIR_S2: i32 = 351;
    pub const SPECIAL_AIR_S_END: i32 = 352;
    pub const SPECIAL_AIR_S: i32 = 353;
    pub const SPECIAL_AIR_S_MISFIRE: i32 = 354;
    pub const SPECIAL_HI: i32 = 355;
    pub const SPECIAL_AIR_HI: i32 = 356;
    pub const SPECIAL_LW: i32 = 357;
    pub const SPECIAL_AIR_LW: i32 = 358;
}

/// Luigi's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_22E8;
    pub const ON_LOAD: u32 = 0x8014_2324;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_2388;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_2468;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_24B0;
    pub const ON_ITEM_DROP: u32 = 0x8014_24F8;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_254C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_2584;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_25C8;
    // The neutral special.
    pub const SPECIAL_N_ENTER: u32 = 0x8014_267C;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8014_26EC;
    pub const SPECIAL_N_ANIM: u32 = 0x8014_275C;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8014_2798;
    pub const SPECIAL_N_IASA: u32 = 0x8014_27D4;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8014_2804;
    pub const SPECIAL_N_PHYS: u32 = 0x8014_2834;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8014_2854;
    pub const SPECIAL_N_COLL: u32 = 0x8014_2874;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8014_28E8;
    pub const SPECIAL_N_FIRE_SPAWN: u32 = 0x8014_295C;
    // The side special.
    pub const SPECIAL_S_SET_GFX: u32 = 0x8014_2A24;
    pub const SPECIAL_S_SET_VARS: u32 = 0x8014_2A5C;
    pub const SPECIAL_S_ENTER: u32 = 0x8014_2B14;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8014_2B88;
    pub const SPECIAL_S_ON_GIVE_DAMAGE: u32 = 0x8014_2C00;
    pub const SPECIAL_S_START_ANIM: u32 = 0x8014_2C40;
    pub const SPECIAL_AIR_S_START_ANIM: u32 = 0x8014_2C7C;
    pub const SPECIAL_S_START_IASA: u32 = 0x8014_2CB8;
    pub const SPECIAL_AIR_S_START_IASA: u32 = 0x8014_2CBC;
    pub const SPECIAL_S_START_PHYS: u32 = 0x8014_2CC0;
    pub const SPECIAL_AIR_S_START_PHYS: u32 = 0x8014_2D00;
    pub const SPECIAL_S_START_COLL: u32 = 0x8014_2D60;
    pub const SPECIAL_AIR_S_START_COLL: u32 = 0x8014_2D9C;
    pub const SPECIAL_S_HOLD_ANIM: u32 = 0x8014_2E98;
    pub const SPECIAL_AIR_S_HOLD_ANIM: u32 = 0x8014_2F38;
    pub const SPECIAL_S_HOLD_IASA: u32 = 0x8014_2FD8;
    pub const SPECIAL_AIR_S_HOLD_IASA: u32 = 0x8014_3008;
    pub const SPECIAL_S_HOLD_PHYS: u32 = 0x8014_3038;
    pub const SPECIAL_AIR_S_HOLD_PHYS: u32 = 0x8014_3058;
    pub const SPECIAL_S_HOLD_COLL: u32 = 0x8014_3078;
    pub const SPECIAL_AIR_S_HOLD_COLL: u32 = 0x8014_30B4;
    pub const SPECIAL_S_ANIM: u32 = 0x8014_3258;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8014_3304;
    pub const SPECIAL_S_IASA: u32 = 0x8014_33A8;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8014_33AC;
    pub const SPECIAL_S_PHYS: u32 = 0x8014_33B0;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8014_33D0;
    pub const SPECIAL_S_COLL: u32 = 0x8014_33F0;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8014_342C;
    pub const SPECIAL_S_MISFIRE_ANIM: u32 = 0x8014_36D8;
    pub const SPECIAL_AIR_S_MISFIRE_ANIM: u32 = 0x8014_3784;
    pub const SPECIAL_S_MISFIRE_IASA: u32 = 0x8014_3828;
    pub const SPECIAL_AIR_S_MISFIRE_IASA: u32 = 0x8014_382C;
    pub const SPECIAL_S_MISFIRE_PHYS: u32 = 0x8014_3830;
    pub const SPECIAL_AIR_S_MISFIRE_PHYS: u32 = 0x8014_3850;
    pub const SPECIAL_S_MISFIRE_COLL: u32 = 0x8014_3870;
    pub const SPECIAL_AIR_S_MISFIRE_COLL: u32 = 0x8014_38AC;
    pub const SPECIAL_S2_ANIM: u32 = 0x8014_3B18;
    pub const SPECIAL_AIR_S2_ANIM: u32 = 0x8014_3B1C;
    pub const SPECIAL_S2_IASA: u32 = 0x8014_3B58;
    pub const SPECIAL_AIR_S2_IASA: u32 = 0x8014_3B5C;
    pub const SPECIAL_S2_PHYS: u32 = 0x8014_3B60;
    pub const SPECIAL_AIR_S2_PHYS: u32 = 0x8014_3B64;
    pub const SPECIAL_S2_COLL: u32 = 0x8014_3BE0;
    pub const SPECIAL_AIR_S2_COLL: u32 = 0x8014_3BE4;
    pub const SPECIAL_S_END_ANIM: u32 = 0x8014_3D7C;
    pub const SPECIAL_AIR_S_END_ANIM: u32 = 0x8014_3DB8;
    pub const SPECIAL_S_END_IASA: u32 = 0x8014_3DF4;
    pub const SPECIAL_AIR_S_END_IASA: u32 = 0x8014_3DF8;
    pub const SPECIAL_S_END_PHYS: u32 = 0x8014_3DFC;
    pub const SPECIAL_AIR_S_END_PHYS: u32 = 0x8014_3E3C;
    pub const SPECIAL_S_END_COLL: u32 = 0x8014_3E8C;
    pub const SPECIAL_AIR_S_END_COLL: u32 = 0x8014_3EC8;
    pub const SPECIAL_S_START_GROUND_TO_AIR: u32 = 0x8014_2DD8;
    pub const SPECIAL_AIR_S_START_AIR_TO_GROUND: u32 = 0x8014_2E38;
    pub const SPECIAL_S_HOLD_GROUND_TO_AIR: u32 = 0x8014_30F0;
    pub const SPECIAL_AIR_S_HOLD_AIR_TO_GROUND: u32 = 0x8014_3150;
    pub const SPECIAL_S_HOLD_ENTER: u32 = 0x8014_31B0;
    pub const SPECIAL_AIR_S_HOLD_ENTER: u32 = 0x8014_3204;
    pub const SPECIAL_S_LAUNCH_GROUND_TO_AIR: u32 = 0x8014_3468;
    pub const SPECIAL_AIR_S_LAUNCH_AIR_TO_GROUND: u32 = 0x8014_34C8;
    pub const SPECIAL_S_LAUNCH_ENTER: u32 = 0x8014_3528;
    pub const SPECIAL_AIR_S_LAUNCH_ENTER: u32 = 0x8014_3600;
    pub const SPECIAL_S_MISFIRE_GROUND_TO_AIR: u32 = 0x8014_38E8;
    pub const SPECIAL_AIR_S_MISFIRE_AIR_TO_GROUND: u32 = 0x8014_3948;
    pub const SPECIAL_S_MISFIRE_ENTER: u32 = 0x8014_39A8;
    pub const SPECIAL_AIR_S_MISFIRE_ENTER: u32 = 0x8014_3A60;
    pub const SPECIAL_S_FLY_ENTER: u32 = 0x8014_3C60;
    pub const SPECIAL_S_END_ENTER: u32 = 0x8014_3F18;
    pub const SPECIAL_AIR_S_END_ENTER: u32 = 0x8014_3F70;
    // The up special.
    pub const SPECIAL_HI_ENTER: u32 = 0x8014_3FC8;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8014_4024;
    pub const SPECIAL_HI_ANIM: u32 = 0x8014_4098;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8014_40F8;
    pub const SPECIAL_HI_IASA: u32 = 0x8014_4158;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8014_42A0;
    pub const SPECIAL_HI_PHYS: u32 = 0x8014_43E8;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8014_4420;
    pub const SPECIAL_HI_CHECK_LANDING: u32 = 0x8014_44B4;
    pub const SPECIAL_HI_COLL: u32 = 0x8014_44E4;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8014_4554;
    // The down special.
    pub const SPECIAL_LW_UPDATE_ROT: u32 = 0x8014_45C4;
    pub const SPECIAL_LW_ENTER: u32 = 0x8014_45F0;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8014_4708;
    pub const SPECIAL_LW_ANIM: u32 = 0x8014_4830;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8014_487C;
    pub const SPECIAL_LW_IASA: u32 = 0x8014_4950;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8014_4954;
    pub const SPECIAL_LW_PHYS: u32 = 0x8014_4958;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8014_4A7C;
    pub const SPECIAL_LW_COLL: u32 = 0x8014_4B38;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8014_4CEC;
}

/// `ftLuigiAttributes`' offsets.
pub(crate) mod attr {
    pub const SIZE: usize = 0x98;
    pub const GREENMISSILE_SMASH: usize = 0x4;
    pub const GREENMISSILE_CHARGE_RATE: usize = 0x8;
    pub const GREENMISSILE_MAX_CHARGE_FRAMES: usize = 0xC;
    pub const GREENMISSILE_DAMAGE_TILT: usize = 0x10;
    pub const GREENMISSILE_DAMAGE_SLOPE: usize = 0x14;
    pub const GREENMISSILE_TRACTION: usize = 0x18;
    pub const GREENMISSILE_UNK2: usize = 0x1C;
    pub const GREENMISSILE_FALLING_SPEED: usize = 0x20;
    pub const GREENMISSILE_VEL_X: usize = 0x24;
    pub const GREENMISSILE_MUL_X: usize = 0x28;
    pub const GREENMISSILE_VEL_Y: usize = 0x2C;
    pub const GREENMISSILE_MUL_Y: usize = 0x30;
    pub const GREENMISSILE_GRAVITY_START: usize = 0x34;
    pub const GREENMISSILE_FRICTION_END: usize = 0x38;
    pub const GREENMISSILE_X_DECEL: usize = 0x3C;
    pub const GREENMISSILE_GRAVITY_MUL: usize = 0x40;
    pub const GREENMISSILE_MISFIRE_CHANCE: usize = 0x44;
    pub const GREENMISSILE_MISFIRE_VEL_X: usize = 0x48;
    pub const GREENMISSILE_MISFIRE_VEL_Y: usize = 0x4C;
    /// The Super Jump Punch's attributes, in Mario's layout (`plumber::SuperJump`).
    pub const SUPERJUMP_FREEFALL_MOBILITY: usize = 0x50;
    /// The Cyclone's attributes, in Mario's Tornado's layout (`plumber::Tornado`).
    pub const CYCLONE_TAP_MOMENTUM: usize = 0x70;
}

/// The `mv` offsets of `ftLuigi_MotionVars`' Green Missile members (the Cyclone's are
/// `plumber`'s).
pub(crate) mod mv {
    /// `SpecialS.chargeFrames` and `isMisfire`.
    pub const S_CHARGE_FRAMES: usize = 0;
    pub const S_IS_MISFIRE: usize = 4;
}

/// The `u` offsets of `ftLuigi_FighterVars`: whether the Cyclone has risen in the air this
/// time airborne (`x222C_cycloneCharge`), and `x2234`.
pub(crate) mod u {
    pub const CYCLONE_CHARGE: usize = 0;
    pub const X2234: usize = 8;
}

/// `FtPart_HipN` and `FtPart_L1stNb`.
pub(crate) mod part {
    pub const HIP_N: i32 = 4;
    pub const L_1ST_NB: i32 = 23;
}

impl Running<'_> {
    /// A float of the fighter's `ftLuigiAttributes`, `off` bytes in.
    pub(crate) fn lg_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.lg_attr_word(f, off))
    }

    fn lg_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Luigi's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Luigi's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
