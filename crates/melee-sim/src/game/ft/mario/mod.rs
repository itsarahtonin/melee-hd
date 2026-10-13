//! Mario's own code (`ft/kinds/ftMario`), which Dr. Mario shares: the Fireball and the
//! Megavitamin (neutral special), the Cape and the Super Sheet (side special), the Super Jump
//! Punch (up special), the Mario and Dr. Tornado (down special) and the character's callbacks.
//! Where the two differ the code branches on the kind (the fireball or the pill); Dr. Mario's
//! own callbacks and taunt are in [`super::drmario`]. Luigi's up and down specials are built as
//! Mario's ([`plumber`]).
//!
//! A state's variables live in the fighter's `mv` union (`ftMario_MotionVars`), the pills'
//! colors, the Tornado's rise and the cape in `u` (`ftMario_FighterVars`), and the character's
//! attributes (`ftMario_DatAttrs`) in the fighter's attribute block, all read at the decomp's
//! offsets.

pub mod init;
pub mod plumber;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Mario` and `Ft_Kind_DrMario`.
pub const KIND_MARIO: i32 = 0;
pub const KIND_DRMARIO: i32 = 21;

/// Mario's action states (`ftMr_MS_*`).
pub mod ms {
    pub const APPEAL_SR: i32 = 341;
    pub const APPEAL_SL: i32 = 342;
    pub const SPECIAL_N: i32 = 343;
    pub const SPECIAL_AIR_N: i32 = 344;
    pub const SPECIAL_S: i32 = 345;
    pub const SPECIAL_AIR_S: i32 = 346;
    pub const SPECIAL_HI: i32 = 347;
    pub const SPECIAL_AIR_HI: i32 = 348;
    pub const SPECIAL_LW: i32 = 349;
    pub const SPECIAL_AIR_LW: i32 = 350;
}

/// Mario's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x800E_08CC;
    pub const ON_LOAD_FOR_DR_MARIO: u32 = 0x800E_0920;
    pub const ON_LOAD: u32 = 0x800E_0960;
    pub const ON_TAKE_DAMAGE: u32 = 0x800E_0A00;
    pub const ON_ITEM_PICKUP: u32 = 0x800E_0A20;
    pub const ON_ITEM_INVISIBLE: u32 = 0x800E_0B00;
    pub const ON_ITEM_VISIBLE: u32 = 0x800E_0B48;
    pub const ON_ITEM_DROP: u32 = 0x800E_0B90;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x800E_0BE4;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x800E_0C24;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x800E_0C68;
    // The neutral special.
    pub const SPECIAL_N_ENTER: u32 = 0x800E_0DA8;
    pub const SPECIAL_N_ANIM: u32 = 0x800E_0E18;
    pub const SPECIAL_N_IASA: u32 = 0x800E_0E54;
    pub const SPECIAL_N_PHYS: u32 = 0x800E_0E84;
    pub const SPECIAL_N_COLL: u32 = 0x800E_0EA4;
    pub const SPECIAL_N_ITEM_FIRE_SPAWN: u32 = 0x800E_0EE0;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x800E_1040;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x800E_10B0;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x800E_10EC;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x800E_111C;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x800E_113C;
    // The side special.
    pub const SPECIAL_S_CREATE_CAPE: u32 = 0x800E_1248;
    pub const SPECIAL_S_RESET: u32 = 0x800E_132C;
    pub const SPECIAL_S_REMOVE_CAPE: u32 = 0x800E_1368;
    pub const SPECIAL_S_ENTER_HITLAG: u32 = 0x800E_13C8;
    pub const SPECIAL_S_EXIT_HITLAG: u32 = 0x800E_13F8;
    pub const SPECIAL_S_CHECK_ITEM_CAPE_REMOVE: u32 = 0x800E_1428;
    pub const SPECIAL_S_ENTER: u32 = 0x800E_1450;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x800E_14C8;
    pub const SPECIAL_S_ANIM: u32 = 0x800E_1550;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x800E_158C;
    pub const SPECIAL_S_IASA: u32 = 0x800E_15C8;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x800E_15CC;
    pub const SPECIAL_S_PHYS: u32 = 0x800E_15D0;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x800E_16E0;
    pub const SPECIAL_S_COLL: u32 = 0x800E_1840;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x800E_187C;
    // The up special.
    pub const SPECIAL_HI_ENTER: u32 = 0x800E_1A54;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x800E_1AB0;
    pub const SPECIAL_HI_ANIM: u32 = 0x800E_1B24;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x800E_1B84;
    pub const SPECIAL_HI_IASA: u32 = 0x800E_1BE4;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x800E_1D2C;
    pub const SPECIAL_HI_PHYS: u32 = 0x800E_1E74;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x800E_1EAC;
    pub const SPECIAL_HI_CHECK_LANDING: u32 = 0x800E_1F40;
    pub const SPECIAL_HI_COLL: u32 = 0x800E_1F70;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x800E_1FE0;
    // The down special.
    pub const SPECIAL_LW_UPDATE_ROT: u32 = 0x800E_2050;
    pub const SPECIAL_LW_ENTER: u32 = 0x800E_207C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x800E_2194;
    pub const SPECIAL_LW_ANIM: u32 = 0x800E_22BC;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x800E_2308;
    pub const SPECIAL_LW_IASA: u32 = 0x800E_23DC;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x800E_23E0;
    pub const SPECIAL_LW_PHYS: u32 = 0x800E_23E4;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x800E_2508;
    pub const SPECIAL_LW_COLL: u32 = 0x800E_25C4;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x800E_2778;
}

/// `ftMario_DatAttrs`' offsets.
pub(crate) mod attr {
    pub const SIZE: usize = 0x84;
    pub const SPECIALS_VEL_X_DECAY: usize = 0x0;
    pub const SPECIALS_VEL_X: usize = 0x4;
    pub const SPECIALS_VEL_Y: usize = 0x8;
    pub const SPECIALS_GRAV: usize = 0xC;
    pub const SPECIALS_TERMINAL_VEL: usize = 0x10;
    pub const SPECIALS_CAPE_KIND: usize = 0x14;
    pub const SPECIALHI: usize = 0x18;
    pub const SPECIALLW: usize = 0x38;
    pub const CAPE_REFLECTION: usize = 0x60;
}

/// The `u` offsets of `ftMario_FighterVars` (the cape's GObj words are `ft::vars::MARIO_CAPE`
/// and `MARIO_X2240`).
pub(crate) mod u {
    pub const VITAMIN_CURR: usize = 0x0;
    pub const VITAMIN_PREV: usize = 0x4;
    pub const TORNADO_CHARGE: usize = 0x8;
    pub const IS_CAPE_BOOST: usize = 0xC;
}

/// The `mv` offset of `ftMarioSpecialS::reflecting`.
pub(crate) mod mv {
    pub const S_REFLECTING: usize = 0;
}

/// `FtPart_HipN`, `FtPart_L1stNb` and `FtPart_RThumbNb`.
pub(crate) mod part {
    pub const HIP_N: i32 = 4;
    pub const L_1ST_NB: i32 = 23;
    pub const R_THUMB_NB: i32 = 49;
}

/// Runs one of Mario's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Mario's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    if let Some(v) = special_s::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
