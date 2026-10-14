//! Link's own code (`ft/kinds/ftLink`): his down aerial's bounce and his shield's absorb, the
//! bow (neutral special), the boomerang (side special), the Spin Attack (up special), the bombs
//! (down special) and the character's callbacks. The hookshot his grabs and air grab shoot is an
//! item (`it::kinds::linkhookshot`), as are his arrows, bow, boomerang and bombs. Young Link
//! (`ftCl_*`) shares most of this code; his own parts aren't ported.
//!
//! A state's variables live in the fighter's `mv` union (`ftLk_MotionVars`), the character's
//! in `u` (`ftLk_FighterVars`), and the character's attributes (`ftLk_DatAttrs`) in the
//! fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Link's through [`call`] and [`run`] by the original's addresses.

pub mod attack_air;
pub mod attack_s42;
pub mod catch;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_LINK` and `FTKIND_CLINK`.
pub const KIND_LINK: i32 = 6;
pub const KIND_CLINK: i32 = 20;

/// Link's functions, by address: the callbacks his states and hooks hold and the entries other
/// code calls.
pub mod addr {
    pub const BOOMERANG_EXISTS: u32 = 0x800E_AD64;
    pub const ON_DEATH: u32 = 0x800E_AD84;
    pub const ON_LOAD_FOR_CLINK: u32 = 0x800E_AE04;
    pub const ON_LOAD: u32 = 0x800E_AE44;
    pub const REMOVE_BOOMERANG_CB: u32 = 0x800E_AF38;
    pub const DAMAGE_CB: u32 = 0x800E_AF58;
    pub const ON_ITEM_PICKUP_EXT: u32 = 0x800E_AFA4;
    pub const ON_ITEM_INVISIBLE: u32 = 0x800E_B018;
    pub const ON_ITEM_VISIBLE: u32 = 0x800E_B060;
    pub const ON_ITEM_DROP_EXT: u32 = 0x800E_B0A8;
    pub const ON_ITEM_PICKUP: u32 = 0x800E_B11C;
    pub const ON_ITEM_DROP: u32 = 0x800E_B1FC;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x800E_B250;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x800E_B2AC;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x800E_B2F0;
    pub const ABSORB_CB: u32 = 0x800E_B334;

    pub const ATTACK_S42_ANIM: u32 = 0x800C_EDE0;
    pub const ATTACK_S42_IASA: u32 = 0x800C_EE00;
    pub const ATTACK_S42_PHYS: u32 = 0x800C_EE30;
    pub const ATTACK_S42_COLL: u32 = 0x800C_EE50;

    pub const ATTACK_AIR_SHIELD: u32 = 0x800E_B3BC;
    pub const ATTACK_AIR_ENTER: u32 = 0x800E_B42C;
    pub const ATTACK_AIR_LW_ON_HIT: u32 = 0x800E_B484;
    pub const ATTACK_AIR_LW_ON_ANIM: u32 = 0x800E_B528;
    pub const ATTACK_AIR_SETUP_PARTS: u32 = 0x800E_B618;

    pub const SPECIAL_N_ENTER: u32 = 0x800E_CDAC;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x800E_CF78;
    pub const SPECIAL_N_START_ANIM: u32 = 0x800E_D15C;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x800E_D480;
    pub const SPECIAL_N_END_ANIM: u32 = 0x800E_D5B4;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x800E_D72C;
    pub const SPECIAL_AIR_N_LOOP_ANIM: u32 = 0x800E_DA58;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x800E_DB8C;
    pub const SPECIAL_N_START_IASA: u32 = 0x800E_DD88;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x800E_DE30;
    pub const SPECIAL_N_END_IASA: u32 = 0x800E_DEAC;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x800E_DEB0;
    pub const SPECIAL_AIR_N_LOOP_IASA: u32 = 0x800E_DF58;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x800E_DFD4;
    pub const SPECIAL_N_START_PHYS: u32 = 0x800E_DFD8;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x800E_DFF8;
    pub const SPECIAL_N_END_PHYS: u32 = 0x800E_E018;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x800E_E038;
    pub const SPECIAL_AIR_N_LOOP_PHYS: u32 = 0x800E_E058;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x800E_E078;
    pub const SPECIAL_N_START_COLL: u32 = 0x800E_E098;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x800E_E120;
    pub const SPECIAL_N_END_COLL: u32 = 0x800E_E1A8;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x800E_E2E0;
    pub const SPECIAL_AIR_N_LOOP_COLL: u32 = 0x800E_E368;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x800E_E3F0;

    pub const SPECIAL_S_ON_CHANGE: u32 = 0x800E_BF2C;
    pub const SPECIAL_S_HOOK: u32 = 0x800E_C210;
    pub const SPECIAL_S_ENTER: u32 = 0x800E_C354;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x800E_C404;
    pub const SPECIAL_S2_ENTER: u32 = 0x800E_C4B4;
    pub const SPECIAL_S1_ANIM: u32 = 0x800E_C54C;
    pub const SPECIAL_S2_ANIM: u32 = 0x800E_C588;
    pub const SPECIAL_S1_EMPTY_ANIM: u32 = 0x800E_C5F8;
    pub const SPECIAL_AIR_S1_ANIM: u32 = 0x800E_C634;
    pub const SPECIAL_AIR_S2_ANIM: u32 = 0x800E_C670;
    pub const SPECIAL_AIR_S1_EMPTY_ANIM: u32 = 0x800E_C6E0;
    pub const SPECIAL_S2_IASA: u32 = 0x800E_C71C;
    pub const SPECIAL_AIR_S2_IASA: u32 = 0x800E_C810;
    pub const SPECIAL_S1_PHYS: u32 = 0x800E_C88C;
    pub const SPECIAL_S2_PHYS: u32 = 0x800E_C8AC;
    pub const SPECIAL_S1_EMPTY_PHYS: u32 = 0x800E_C8CC;
    pub const SPECIAL_AIR_S1_PHYS: u32 = 0x800E_C8EC;
    pub const SPECIAL_AIR_S2_PHYS: u32 = 0x800E_C90C;
    pub const SPECIAL_AIR_S1_EMPTY_PHYS: u32 = 0x800E_C92C;
    pub const SPECIAL_S1_COLL: u32 = 0x800E_C94C;
    pub const SPECIAL_S2_COLL: u32 = 0x800E_C9C4;
    pub const SPECIAL_S1_EMPTY_COLL: u32 = 0x800E_CA1C;
    pub const SPECIAL_AIR_S1_COLL: u32 = 0x800E_CA88;
    pub const SPECIAL_AIR_S2_COLL: u32 = 0x800E_CB00;
    pub const SPECIAL_AIR_S1_EMPTY_COLL: u32 = 0x800E_CB58;

    pub const SPECIAL_LW_ENTER: u32 = 0x800E_B65C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x800E_B6DC;
    pub const SPAWN_BOMB: u32 = 0x800E_B7C8;
    pub const SPECIAL_LW_ANIM: u32 = 0x800E_B8A4;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x800E_B8E0;
    pub const SPECIAL_LW_PHYS: u32 = 0x800E_B91C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x800E_B93C;
    pub const SPECIAL_LW_COLL: u32 = 0x800E_B95C;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x800E_B9D4;

    pub const SPECIAL_HI_ON_ACCESSORY4: u32 = 0x800E_BA4C;
    pub const SPECIAL_HI_ENTER: u32 = 0x800E_BB1C;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x800E_BB80;
    pub const SPECIAL_HI_ANIM: u32 = 0x800E_BC10;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x800E_BC4C;
    pub const SPECIAL_HI_IASA: u32 = 0x800E_BCAC;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x800E_BCB0;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x800E_BCB4;
    pub const SPECIAL_HI_PHYS: u32 = 0x800E_BD30;
    pub const SPECIAL_HI_COLL: u32 = 0x800E_BDC0;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x800E_BDFC;
    pub const SPECIAL_HI_DO_COLL: u32 = 0x800E_BE64;
}

/// Link's action states (`ftLk_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 344;
    pub const SPECIAL_N_LOOP: i32 = 345;
    pub const SPECIAL_N_END: i32 = 346;
    pub const SPECIAL_AIR_N_START: i32 = 347;
    pub const SPECIAL_AIR_N_LOOP: i32 = 348;
    pub const SPECIAL_AIR_N_END: i32 = 349;
    pub const ATTACK_S42: i32 = 341;
    pub const SPECIAL_S1: i32 = 350;
    pub const SPECIAL_S2: i32 = 351;
    pub const SPECIAL_S1_EMPTY: i32 = 352;
    pub const SPECIAL_AIR_S1: i32 = 353;
    pub const SPECIAL_AIR_S2: i32 = 354;
    pub const SPECIAL_AIR_S1_EMPTY: i32 = 355;
    pub const SPECIAL_LW: i32 = 358;
    pub const SPECIAL_AIR_LW: i32 = 359;
    pub const SPECIAL_HI: i32 = 356;
    pub const SPECIAL_AIR_HI: i32 = 357;
}

/// `ftLk_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// The boomerang thrown this jump (`used_boomerang`).
    pub const USED_BOOMERANG: usize = 0x0;
    /// `x4`.
    pub const X4: usize = 0x4;
    /// The boomerang (`boomerang_gobj`), the hookshot (`xC`), the arrow (`arrow_gobj`), the
    /// bow (`x14`) and `x18`.
    pub const BOOMERANG: usize = 0x8;
    pub const HOOKSHOT: usize = 0xC;
    pub const ARROW: usize = 0x10;
    pub const BOW: usize = 0x14;
    pub const X18: usize = 0x18;
}

/// `ftLk_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `attackair.lw_frame_start`: the down aerial's bounce frames left, a float.
    pub const ATTACK_AIR_LW_FRAME_START: usize = 0x0;
}

/// `ftLk_DatAttrs`' offsets (0xDC bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0xDC;
    pub const X0: usize = 0x0;
    pub const SPECIAL_N_ANIM_RATE: usize = 0x4;
    pub const X8: usize = 0x8;
    pub const XC: usize = 0xC;
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const SPECIAL_HI_POS_Y_OFFSET: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X34: usize = 0x34;
    pub const SPECIAL_AIR_HI_DRIFT_STICK_MUL: usize = 0x38;
    pub const SPECIAL_AIR_HI_DRIFT_MAX_MUL: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const SPECIAL_HI_GRAV_MUL: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const ATTACK_AIR_LW_HIT_VEL_Y: usize = 0x4C;
    pub const ATTACK_AIR_LW_HIT_FRAME_START: usize = 0x50;
    pub const ATTACK_AIR_LW_HIT_FRAME_END: usize = 0x54;
    pub const ATTACK_AIR_LW_ANIM_FLAGS: usize = 0x58;
    pub const XBC: usize = 0xBC;
    /// The shield's absorb box (`AbsorbDesc`).
    pub const XC4: usize = 0xC4;
    pub const XD8: usize = 0xD8;
}

impl Running<'_> {
    /// A float of the fighter's `ftLk_DatAttrs`, `off` bytes in.
    pub(crate) fn link_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.link_attr_word(f, off))
    }

    pub(crate) fn link_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.link_attr_word(f, off) as i32
    }

    pub(crate) fn link_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }
}

/// Runs one of Link's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    init::run(g, gobj, a)
        || attack_air::run(g, gobj, a)
        || attack_s42::run(g, gobj, a)
        || special_hi::run(g, gobj, a)
        || special_lw::run(g, gobj, a)
        || special_s::run(g, gobj, a)
        || special_n::run(g, gobj, a)
}

/// Link's functions other code calls by address (`code::call`): the item hooks and every
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
