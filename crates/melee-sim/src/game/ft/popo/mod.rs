//! The Ice Climbers' own code (`ft/kinds/ftPopo`, `ft/kinds/ftNana`): Popo, whom the player
//! controls, and Nana, the partner the computer plays as his second fighter. Nana runs Popo's
//! states with her own versions of the specials she does differently; where the two differ
//! the code branches on the kind. The character's callbacks are in [`init`].
//!
//! The character's variables live in the fighter's `u` union (`ftNana_FighterVars`, which
//! Popo's code uses too), and the character's attributes (`ftIceClimberAttributes`) in the
//! fighter's attribute block, read at the decomp's offsets.

pub mod init;
pub mod nana_special_hi;
pub mod nana_special_s;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Popo` and `Ft_Kind_Nana`.
pub const KIND_POPO: i32 = 10;
pub const KIND_NANA: i32 = 11;

/// The Ice Climbers' items: `It_Kind_IceClimber_Ice`, `_Blizzard` and `_GumStrings`.
pub const IT_KIND_ICE: i32 = 106;
pub const IT_KIND_BLIZZARD: i32 = 107;
pub const IT_KIND_GUM_STRINGS: i32 = 113;

/// Offsets into `ftIceClimberAttributes`.
pub mod attr {
    /// Popo's `Fighter::x40`.
    pub const POPO_X40: usize = 0x0;
    /// Nana's `Fighter::x40` and her armor at each spawn.
    pub const NANA_X40: usize = 0xC4;
    pub const NANA_ARMOR: usize = 0xC8;
    /// The size `PUSH_ATTRS` copies.
    pub const SIZE: usize = 0x15C;
}

/// Offsets into the character's variables (`ftPopo_FighterVars`, from `Fighter::u`).
pub mod var {
    /// `x222C`: an item a special holds.
    pub const ITEM: usize = 0x0;
    /// `x2230`: its first bit.
    pub const X2230: usize = 0x4;
    pub const X2234: usize = 0x8;
    /// `x2238`: another item.
    pub const ITEM2: usize = 0xC;
    /// `x223C`, and `x2240`: the point Nana's hand puts the string's end at while she's
    /// thrown.
    pub const X223C: usize = 0x10;
    pub const HAND_POINT: usize = 0x14;
    pub const X224C: usize = 0x20;
    pub const X2250: usize = 0x24;
}

/// Popo's and Nana's functions, by address: the callbacks their states and hooks hold and the
/// entries other code calls.
pub mod addr {
    pub const PP_ON_ITEM_PICKUP: u32 = 0x8011_ED38;
    pub const PP_ON_ITEM_INVISIBLE: u32 = 0x8011_EE18;
    pub const PP_ON_ITEM_VISIBLE: u32 = 0x8011_EE60;
    pub const PP_ON_ITEM_DROP: u32 = 0x8011_EEA8;
    pub const PP_ON_LOAD_FOR_NANA: u32 = 0x8011_EEFC;
    pub const PP_ON_LOAD: u32 = 0x8011_EF3C;
    pub const PP_ON_DEATH: u32 = 0x8011_EFE8;
    pub const PP_LOAD_SPECIAL_ATTRS: u32 = 0x8011_F0A4;
    pub const PP_ON_KNOCKBACK_ENTER: u32 = 0x8011_F0E4;
    pub const PP_ON_KNOCKBACK_EXIT: u32 = 0x8011_F128;
    pub const PP_REMOVE_ITEMS: u32 = 0x8011_F060;
    // Ice Shot.
    pub const PP_SPECIAL_N_ENTER: u32 = 0x8011_F1F8;
    pub const PP_SPECIAL_AIR_N_ENTER: u32 = 0x8011_F26C;
    pub const PP_SPECIAL_N_ANIM: u32 = 0x8011_F318;
    pub const PP_SPECIAL_AIR_N_ANIM: u32 = 0x8011_F354;
    pub const PP_SPECIAL_N_IASA: u32 = 0x8011_F390;
    pub const PP_SPECIAL_AIR_N_IASA: u32 = 0x8011_F394;
    pub const PP_SPECIAL_N_PHYS: u32 = 0x8011_F398;
    pub const PP_SPECIAL_AIR_N_PHYS: u32 = 0x8011_F3B8;
    pub const PP_SPECIAL_N_COLL: u32 = 0x8011_F3D8;
    pub const PP_SPECIAL_AIR_N_COLL: u32 = 0x8011_F454;
    pub const PP_SPECIAL_N_ICE: u32 = 0x8011_F500;
    // Squall Hammer, Popo's.
    pub const PP_SPECIAL_S_ENTER: u32 = 0x8011_F99C;
    pub const PP_SPECIAL_AIR_S_ENTER: u32 = 0x8011_FB08;
    pub const PP_SPECIAL_S1_ANIM: u32 = 0x8011_FC78;
    pub const PP_SPECIAL_S2_ANIM: u32 = 0x8011_FCD0;
    pub const PP_SPECIAL_AIR_S1_ANIM: u32 = 0x8011_FD9C;
    pub const PP_SPECIAL_AIR_S2_ANIM: u32 = 0x8011_FE48;
    pub const PP_SPECIAL_S1_IASA: u32 = 0x8011_FF40;
    pub const PP_SPECIAL_S2_IASA: u32 = 0x8011_FF90;
    pub const PP_SPECIAL_AIR_S1_IASA: u32 = 0x8011_FFE0;
    pub const PP_SPECIAL_AIR_S2_IASA: u32 = 0x8012_0030;
    pub const PP_SPECIAL_S1_PHYS: u32 = 0x8012_0080;
    pub const PP_SPECIAL_S2_PHYS: u32 = 0x8012_0230;
    pub const PP_SPECIAL_AIR_S1_PHYS: u32 = 0x8012_03E0;
    pub const PP_SPECIAL_AIR_S2_PHYS: u32 = 0x8012_0520;
    pub const PP_SPECIAL_S1_COLL: u32 = 0x8012_0660;
    pub const PP_SPECIAL_S2_COLL: u32 = 0x8012_0854;
    pub const PP_SPECIAL_AIR_S1_COLL: u32 = 0x8012_0A48;
    pub const PP_SPECIAL_AIR_S2_COLL: u32 = 0x8012_0C58;
    // Squall Hammer, Nana's.
    pub const NN_SPECIAL_S_RELEASE: u32 = 0x8012_2FAC;
    pub const NN_SPECIAL_S_0_ANIM: u32 = 0x8012_3CA4;
    pub const NN_SPECIAL_S_1_ANIM: u32 = 0x8012_3D68;
    pub const NN_SPECIAL_S_0_IASA: u32 = 0x8012_3E58;
    pub const NN_SPECIAL_S_1_IASA: u32 = 0x8012_3E5C;
    pub const NN_SPECIAL_S_0_PHYS: u32 = 0x8012_3E60;
    pub const NN_SPECIAL_S_1_PHYS: u32 = 0x8012_3EFC;
    pub const NN_SPECIAL_S_0_COLL: u32 = 0x8012_3F98;
    pub const NN_SPECIAL_S_1_COLL: u32 = 0x8012_41A0;
    // Blizzard.
    pub const PP_SPECIAL_LW_ENTER: u32 = 0x8012_2904;
    pub const PP_SPECIAL_AIR_LW_ENTER: u32 = 0x8012_2988;
    pub const PP_SPECIAL_LW_ANIM: u32 = 0x8012_2A0C;
    pub const PP_SPECIAL_AIR_LW_ANIM: u32 = 0x8012_2A8C;
    pub const PP_SPECIAL_LW_IASA: u32 = 0x8012_2B0C;
    pub const PP_SPECIAL_AIR_LW_IASA: u32 = 0x8012_2B10;
    pub const PP_SPECIAL_LW_PHYS: u32 = 0x8012_2B14;
    pub const PP_SPECIAL_AIR_LW_PHYS: u32 = 0x8012_2B34;
    pub const PP_SPECIAL_LW_COLL: u32 = 0x8012_2C18;
    pub const PP_SPECIAL_AIR_LW_COLL: u32 = 0x8012_2D04;
    pub const PP_SPECIAL_LW_BLOW: u32 = 0x8012_2D2C;
    // Belay, Popo's.
    pub const PP_SPECIAL_HI_ENTER: u32 = 0x8012_11B4;
    pub const PP_SPECIAL_AIR_HI_ENTER: u32 = 0x8012_122C;
    pub const PP_SPECIAL_HI_START_0_ANIM: u32 = 0x8012_12C4;
    pub const PP_SPECIAL_AIR_HI_START_0_ANIM: u32 = 0x8012_13CC;
    pub const PP_SPECIAL_HI_START_0_IASA: u32 = 0x8012_14D4;
    pub const PP_SPECIAL_AIR_HI_START_0_IASA: u32 = 0x8012_1558;
    pub const PP_SPECIAL_HI_START_0_PHYS: u32 = 0x8012_15DC;
    pub const PP_SPECIAL_AIR_HI_START_0_PHYS: u32 = 0x8012_1680;
    pub const PP_SPECIAL_HI_START_0_COLL: u32 = 0x8012_1740;
    pub const PP_SPECIAL_AIR_HI_START_0_COLL: u32 = 0x8012_177C;
    pub const PP_SPECIAL_HI_THROW_0_ANIM: u32 = 0x8012_1944;
    pub const PP_SPECIAL_AIR_HI_THROW_0_ANIM: u32 = 0x8012_19F4;
    pub const PP_SPECIAL_HI_THROW_0_IASA: u32 = 0x8012_1AC8;
    pub const PP_SPECIAL_AIR_HI_THROW_0_IASA: u32 = 0x8012_1ACC;
    pub const PP_SPECIAL_HI_THROW_0_PHYS: u32 = 0x8012_1AD0;
    pub const PP_SPECIAL_AIR_HI_THROW_0_PHYS: u32 = 0x8012_1B74;
    pub const PP_SPECIAL_HI_THROW_0_COLL: u32 = 0x8012_1C34;
    pub const PP_SPECIAL_AIR_HI_THROW_0_COLL: u32 = 0x8012_1C70;
    pub const PP_SPECIAL_HI_START_1_ANIM: u32 = 0x8012_1E10;
    pub const PP_SPECIAL_AIR_HI_START_1_ANIM: u32 = 0x8012_1E4C;
    pub const PP_SPECIAL_HI_START_1_IASA: u32 = 0x8012_1EB0;
    pub const PP_SPECIAL_AIR_HI_START_1_IASA: u32 = 0x8012_1EB4;
    pub const PP_SPECIAL_HI_START_1_PHYS: u32 = 0x8012_1EB8;
    pub const PP_SPECIAL_AIR_HI_START_1_PHYS: u32 = 0x8012_1ED8;
    pub const PP_SPECIAL_HI_START_1_COLL: u32 = 0x8012_1F2C;
    pub const PP_SPECIAL_AIR_HI_START_1_COLL: u32 = 0x8012_1F68;
    pub const PP_SPECIAL_HI_THROW_1_ANIM: u32 = 0x8012_2110;
    pub const PP_SPECIAL_AIR_HI_THROW_1_ANIM: u32 = 0x8012_214C;
    pub const PP_SPECIAL_HI_THROW_1_IASA: u32 = 0x8012_21AC;
    pub const PP_SPECIAL_AIR_HI_THROW_1_IASA: u32 = 0x8012_21B0;
    pub const PP_SPECIAL_HI_THROW_1_PHYS: u32 = 0x8012_21B4;
    pub const PP_SPECIAL_AIR_HI_THROW_1_PHYS: u32 = 0x8012_21D4;
    pub const PP_SPECIAL_HI_THROW_1_COLL: u32 = 0x8012_2228;
    pub const PP_SPECIAL_AIR_HI_THROW_1_COLL: u32 = 0x8012_2264;
    pub const PP_SPECIAL_AIR_HI_THROW2_ANIM: u32 = 0x8012_2410;
    pub const PP_SPECIAL_AIR_HI_THROW2_IASA: u32 = 0x8012_2490;
    pub const PP_SPECIAL_AIR_HI_THROW2_PHYS: u32 = 0x8012_2538;
    pub const PP_SPECIAL_AIR_HI_THROW2_COLL: u32 = 0x8012_26A0;
    // Belay, Nana's.
    pub const NN_SPECIAL_HI_HAND: u32 = 0x8012_3218;
    pub const NN_SPECIAL_HI_0_ANIM: u32 = 0x8012_3348;
    pub const NN_SPECIAL_HI_0_IASA: u32 = 0x8012_3388;
    pub const NN_SPECIAL_HI_0_PHYS: u32 = 0x8012_3390;
    pub const NN_SPECIAL_HI_0_COLL: u32 = 0x8012_33F0;
    pub const NN_SPECIAL_HI_1_ANIM: u32 = 0x8012_3448;
    pub const NN_SPECIAL_HI_4_ANIM: u32 = 0x8012_3484;
    pub const NN_SPECIAL_HI_1_IASA: u32 = 0x8012_34E4;
    pub const NN_SPECIAL_HI_4_IASA: u32 = 0x8012_34E8;
    pub const NN_SPECIAL_HI_1_PHYS: u32 = 0x8012_34EC;
    pub const NN_SPECIAL_HI_4_PHYS: u32 = 0x8012_350C;
    pub const NN_SPECIAL_HI_1_COLL: u32 = 0x8012_35BC;
    pub const NN_SPECIAL_HI_4_COLL: u32 = 0x8012_35F8;
    pub const NN_ON_LOAD: u32 = 0x8012_2EDC;
    pub const NN_ON_DEATH: u32 = 0x8012_2F28;
    pub const NN_LOAD_SPECIAL_ATTRS: u32 = 0x8012_2FCC;
}

impl Running<'_> {
    /// A float of the fighter's `ftIceClimberAttributes`, `off` bytes in.
    pub(crate) fn ic_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        f32::from_bits(u32::from_be_bytes(b[off..off + 4].try_into().unwrap()))
    }

    /// An `int` of the fighter's `ftIceClimberAttributes`, `off` bytes in.
    pub(crate) fn ic_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.ic_attr_f32(f, off).to_bits() as i32
    }
}

/// Runs one of the Ice Climbers' callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr`
/// is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || nana_special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || nana_special_s::run(g, gobj, addr)
}

/// The Ice Climbers' functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
