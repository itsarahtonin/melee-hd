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
pub mod special_s;

use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

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
    #[doc(alias("INIT_801393AC"))]
    pub const SPECIAL_S_ON_DAMAGE: u32 = 0x8013_93AC;
    // The side special.
    pub const SPECIAL_S_ENTER: u32 = 0x8013_B638;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8013_B6D8;
    // The neutral special.
    #[doc(alias("N_8013A830"))]
    pub const SPECIAL_N_GFX: u32 = 0x8013_A830;
    #[doc(alias("N_8013A8AC"))]
    pub const SPECIAL_AIR_N_GFX: u32 = 0x8013_A8AC;
    pub const SPECIAL_N_ENTER: u32 = 0x8013_A928;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8013_A9A4;
    #[doc(alias("N_8013ADB0"))]
    pub const SPECIAL_N_REFLECTED: u32 = 0x8013_ADB0;
    // The up special.
    #[doc(alias("HI_801396AC"))]
    pub const SPECIAL_HI_START_GFX_ONCE: u32 = 0x8013_96AC;
    #[doc(alias("HI_8013979C"))]
    pub const SPECIAL_HI_APPEAR_GFX: u32 = 0x8013_979C;
    pub const SPECIAL_HI_ENTER: u32 = 0x8013_9834;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8013_98E8;
    // The down special.
    #[doc(alias("LW_8013ADB4"))]
    pub const SPECIAL_LW_START_GFX: u32 = 0x8013_ADB4;
    #[doc(alias("LW_8013AE30"))]
    pub const SPECIAL_LW_END_GFX: u32 = 0x8013_AE30;
    #[doc(alias("LW_8013AEAC"))]
    pub const SPECIAL_LW_SWAP: u32 = 0x8013_AEAC;
    pub const SPECIAL_LW_ENTER: u32 = 0x8013_AEE0;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8013_AFA4;
    #[doc(alias("LW_8013B4D8"))]
    pub const SPECIAL_LW_FINISH: u32 = 0x8013_B4D8;
}

/// The `mv` offsets of `ftZelda_MotionVars`' members.
pub(crate) mod mv {
    /// `specialn.x0`.
    pub const N_X0: usize = 0;
    /// `specials.x0`, `x4`, `x8`, `xC`.
    pub const S_X0: usize = 0;
    pub const S_X4: usize = 4;
    pub const S_X8: usize = 8;
    pub const S_XC: usize = 0xC;
}

/// `ftZelda_DatAttrs`' offsets.
pub(crate) mod attr {
    pub const X4: usize = 4;
    pub const X8: usize = 8;
    pub const XC: usize = 0xC;
    pub const X2C: usize = 0x2C;
    pub const X34: usize = 0x34;
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
#[melee_callbacks::callbacks(Fighter)]
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Zelda's functions other code calls by address (`code::call`).
#[melee_callbacks::callbacks(Call)]
pub fn call(
    r: &mut Running,
    f: u32,
    args: &[super::super::code::Val],
) -> Option<super::super::code::Val> {
    use super::super::code::Val;
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    None
}
