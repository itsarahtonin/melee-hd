//! Zelda's own code (`ft/kinds/ftZelda`): Nayru's Love (neutral special), Din's Fire (side
//! special), Farore's Wind (up special), the transformation into Sheik (down special) and the
//! character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftZelda_MotionVars`), Din's Fire in
//! `u` (`ftZelda_FighterVars`), and the character's attributes (`ftZelda_DatAttrs`) in the
//! fighter's attribute block, all read at the decomp's offsets. Her creation and spawn
//! (`ftZd_Init_OnLoad`, `ftZd_Init_OnDeath`, `ftZd_Init_LoadSpecialAttrs`) come with fighter
//! creation (`ft::kinds`).

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;

use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Zelda`.
pub const KIND_ZELDA: i32 = 19;

/// Zelda's action states (`ftZd_MS_*`).
pub mod ms {
    pub const SPECIAL_N: i32 = 341;
    pub const SPECIAL_AIR_N: i32 = 342;
    pub const SPECIAL_S_START: i32 = 343;
    pub const SPECIAL_S_LOOP: i32 = 344;
    pub const SPECIAL_S_END: i32 = 345;
    pub const SPECIAL_AIR_S_START: i32 = 346;
    pub const SPECIAL_AIR_S_LOOP: i32 = 347;
    pub const SPECIAL_AIR_S_END: i32 = 348;
    pub const SPECIAL_HI_START_0: i32 = 349;
    pub const SPECIAL_HI_START_1: i32 = 350;
    pub const SPECIAL_HI: i32 = 351;
    pub const SPECIAL_AIR_HI_START_0: i32 = 352;
    pub const SPECIAL_AIR_HI_START_1: i32 = 353;
    pub const SPECIAL_AIR_HI: i32 = 354;
    pub const SPECIAL_LW: i32 = 355;
    pub const SPECIAL_LW2: i32 = 356;
    pub const SPECIAL_AIR_LW: i32 = 357;
    pub const SPECIAL_AIR_LW2: i32 = 358;
}

/// Zelda's functions other code reaches by address.
pub mod addr {
    pub const ON_ITEM_PICKUP: u32 = 0x8013_93CC;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8013_94AC;
    pub const ON_ITEM_VISIBLE: u32 = 0x8013_94F4;
    pub const ON_ITEM_DROP: u32 = 0x8013_953C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8013_9624;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8013_9668;
    // The neutral special.
    pub const N_8013A830: u32 = 0x8013_A830;
    pub const N_8013A8AC: u32 = 0x8013_A8AC;
    pub const SPECIAL_N_ENTER: u32 = 0x8013_A928;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8013_A9A4;
    pub const N_8013ADB0: u32 = 0x8013_ADB0;
    // The up special.
    pub const HI_801396AC: u32 = 0x8013_96AC;
    pub const HI_8013979C: u32 = 0x8013_979C;
    pub const SPECIAL_HI_ENTER: u32 = 0x8013_9834;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8013_98E8;
    // The down special.
    pub const LW_8013ADB4: u32 = 0x8013_ADB4;
    pub const LW_8013AE30: u32 = 0x8013_AE30;
    pub const LW_8013AEAC: u32 = 0x8013_AEAC;
    pub const SPECIAL_LW_ENTER: u32 = 0x8013_AEE0;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8013_AFA4;
    pub const LW_8013B4D8: u32 = 0x8013_B4D8;
}

/// The `mv` offsets of `ftZelda_MotionVars`' members.
pub(crate) mod mv {
    /// `specialhi.x0`, `x4`, `xC`, `x10`, `x18`.
    pub const HI_X0: usize = 0;
    pub const HI_X4_X: usize = 4;
    pub const HI_X4_Y: usize = 8;
    pub const HI_XC: usize = 0xC;
    pub const HI_X10_X: usize = 0x10;
    pub const HI_X10_Y: usize = 0x14;
    pub const HI_X18: usize = 0x18;
    /// `specialn.x0`.
    pub const N_X0: usize = 0;
}

/// `ftZelda_DatAttrs`' offsets.
pub(crate) mod attr {
    pub const X4: usize = 4;
    pub const X8: usize = 8;
    pub const XC: usize = 0xC;
    pub const X38: usize = 0x38;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X54: usize = 0x54;
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
}

pub(crate) use super::common::GA_GROUND;

impl Running<'_> {
    /// A float of the fighter's `ftZelda_DatAttrs`, `off` bytes in.
    pub(crate) fn zelda_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.zelda_attr_word(f, off))
    }

    pub(crate) fn zelda_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.zelda_attr_word(f, off) as i32
    }

    fn zelda_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Zelda's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Zelda's functions other code calls by address (`code::call`).
pub fn call(
    r: &mut Running,
    f: u32,
    args: &[super::super::code::Val],
) -> Option<super::super::code::Val> {
    use super::super::code::Val;
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
