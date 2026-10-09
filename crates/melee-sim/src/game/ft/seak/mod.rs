//! Sheik's own code (`ft/kinds/ftSeak`): the needles (neutral special), the chain (side
//! special), Vanish (up special), the transformation into Zelda (down special) and the
//! character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftSeak_MotionVars`), the needles and
//! chain in `u` (`ftSeak_FighterVars`), and the character's attributes (`ftSeakAttributes`) in
//! the fighter's attribute block, all read at the decomp's offsets.
//!
//! Where Sheik's code reaches code another part of the port owns and hasn't ported yet (the
//! yo-yo's stage check), it calls it through `code::call` by the original's address
//! ([`ext`]), which stops until that part adds its arms. Other code reaches Sheik's
//! through [`call`] and [`run`] the same way.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::{self, Val};
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Seak`.
pub const KIND_SEAK: i32 = 7;

/// Sheik's action states (`ftSk_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 341;
    pub const SPECIAL_N_LOOP: i32 = 342;
    pub const SPECIAL_N_CANCEL: i32 = 343;
    pub const SPECIAL_N_END: i32 = 344;
    pub const SPECIAL_AIR_N_START: i32 = 345;
    pub const SPECIAL_AIR_N_LOOP: i32 = 346;
    pub const SPECIAL_AIR_N_CANCEL: i32 = 347;
    pub const SPECIAL_AIR_N_END: i32 = 348;
    pub const SPECIAL_S_START: i32 = 349;
    pub const SPECIAL_S: i32 = 350;
    pub const SPECIAL_S_END: i32 = 351;
    pub const SPECIAL_AIR_S_START: i32 = 352;
    pub const SPECIAL_AIR_S: i32 = 353;
    pub const SPECIAL_AIR_S_END: i32 = 354;
    pub const SPECIAL_HI_START_0: i32 = 355;
    pub const SPECIAL_HI_START_1: i32 = 356;
    pub const SPECIAL_HI: i32 = 357;
    pub const SPECIAL_AIR_HI_START_0: i32 = 358;
    pub const SPECIAL_AIR_HI_START_1: i32 = 359;
    pub const SPECIAL_AIR_HI: i32 = 360;
    pub const SPECIAL_LW: i32 = 361;
    pub const SPECIAL_LW2: i32 = 362;
    pub const SPECIAL_AIR_LW: i32 = 363;
    pub const SPECIAL_AIR_LW2: i32 = 364;
}

/// The functions other parts of the port own that Sheik's code calls, by address: each
/// answers `code::call` once its part is ported.
pub mod ext {
    /// `ftNs_AttackHi4_YoyoCheckEnvColl`: whether the stage lies between two points.
    pub const FTNS_YOYO_CHECK_ENV_COLL: u32 = 0x8011_4FF8;
}

/// Sheik's functions other code reaches by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8011_0094;
    pub const ON_LOAD: u32 = 0x8011_00EC;
    pub const DAMAGE_CB: u32 = 0x8011_0198;
    pub const UNK_MOTION_STATES_4: u32 = 0x8011_01CC;
    pub const ON_ITEM_PICKUP: u32 = 0x8011_0204;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8011_02E4;
    pub const ON_ITEM_VISIBLE: u32 = 0x8011_032C;
    pub const ON_ITEM_DROP: u32 = 0x8011_0374;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8011_03C8;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8011_0408;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8011_044C;
    // The side special.
    pub const S_8011097C: u32 = 0x8011_097C;
    pub const S_UPDATE_HITBOXES: u32 = 0x8011_0980;
    pub const S_ZERO_HITBOX_POSITIONS: u32 = 0x8011_0A80;
    pub const S_80110E4C: u32 = 0x8011_0E4C;
    pub const S_CHECK_AND_DESTROY_CHAIN: u32 = 0x8011_0E88;
    pub const S_80110EE8: u32 = 0x8011_0EE8;
    pub const S_CHAIN_SOMETHING: u32 = 0x8011_0F18;
    pub const S_80110F58: u32 = 0x8011_0F58;
    pub const S_80110F64: u32 = 0x8011_0F64;
    pub const S_80110F70: u32 = 0x8011_0F70;
    pub const SPECIAL_S_ENTER: u32 = 0x8011_1038;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8011_108C;
    pub const S_CHECK_INIT_CHAIN: u32 = 0x8011_10E8;
    pub const S_80111F70: u32 = 0x8011_1F70;
    pub const S_80111FA0: u32 = 0x8011_1FA0;
    // The neutral special.
    pub const N_80111FBC: u32 = 0x8011_1FBC;
    pub const N_801120D4: u32 = 0x8011_20D4;
    pub const SPECIAL_N_ENTER: u32 = 0x8011_20E8;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8011_2198;
    pub const SHOOT_NEEDLES: u32 = 0x8011_2D44;
    // The up special.
    pub const HI_FN_80112ED8: u32 = 0x8011_2ED8;
    pub const HI_FN_80113038: u32 = 0x8011_3038;
    pub const SPECIAL_HI_ENTER: u32 = 0x8011_30D0;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8011_312C;
    // The down special.
    pub const LW_FN_80114034: u32 = 0x8011_4034;
    pub const LW_FN_801140B0: u32 = 0x8011_40B0;
    pub const LW_FN_8011412C: u32 = 0x8011_412C;
    pub const SPECIAL_LW_ENTER: u32 = 0x8011_4160;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8011_4224;
    pub const LW_80114758: u32 = 0x8011_4758;
}

/// The `u` offsets of `ftSeak_FighterVars`' members.
pub(crate) mod u {
    /// `x0`: how many needles are held.
    pub const NEEDLE_COUNT: usize = 0;
    /// `x4`: the held needles' item.
    pub const NEEDLES: usize = super::super::vars::SEAK_NEEDLES;
    /// `x8`: the chain.
    pub const CHAIN: usize = super::super::vars::SEAK_CHAIN;
    /// `xC[4]` and `x3C[4]`: the chain's hitbox positions, this frame's and last.
    pub const HIT_POS: usize = 0xC;
    pub const HIT_POS_PREV: usize = 0x3C;
    /// `lstick_delta`.
    pub const LSTICK_DELTA: usize = 0x6C;
}

/// The `mv` offsets of `ftSeak_MotionVars`' members.
pub(crate) mod mv {
    /// `specialn.x0`, `x4`, `x8`.
    pub const N_X0: usize = 0;
    pub const N_X4: usize = 4;
    pub const N_X8: usize = 8;
    /// `specials.x0` .. `x20`.
    pub const S_X0: usize = 0;
    pub const S_X4: usize = 4;
    pub const S_X8: usize = 8;
    pub const S_XC: usize = 0xC;
    pub const S_X10: usize = 0x10;
    pub const S_X14: usize = 0x14;
    pub const S_X18: usize = 0x18;
    pub const S_X1C: usize = 0x1C;
    pub const S_X20: usize = 0x20;
    /// `specialhi.x0`, `vel`, `xC`.
    pub const HI_X0: usize = 0;
    pub const HI_VEL_X: usize = 4;
    pub const HI_VEL_Y: usize = 8;
    pub const HI_XC: usize = 0xC;
    /// `mv.co.walk.fast_anim_frame`, `mv.co.common.x14`, `x18`.
    pub const CO_X10: usize = 0x10;
    pub const CO_X14: usize = 0x14;
    pub const CO_X18: usize = 0x18;
}

/// `ftSeakAttributes`' offsets.
pub(crate) mod attr {
    pub const X0: usize = 0;
    pub const X4: usize = 4;
    pub const X8: usize = 8;
    pub const XC: usize = 0xC;
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const SELF_VEL_Y: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X34: usize = 0x34;
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
}

/// `It_Kind_Seak_NeedleThrow`, `_NeedleHeld`, `_Vanish`, `_Chain`.
pub const IT_NEEDLE_THROW: i32 = 79;
pub const IT_NEEDLE_HELD: i32 = 80;
pub const IT_VANISH: i32 = 85;
pub const IT_CHAIN: i32 = 97;

/// `HSD_PAD_B`, `HSD_PAD_LR`.
pub(crate) const PAD_B: u32 = 1 << 9;
pub(crate) const PAD_LR: u32 = 1 << 31;

pub(crate) use super::common::{GA_AIR, GA_GROUND};

impl Running<'_> {
    /// A float of the fighter's `ftSeakAttributes`, `off` bytes in.
    pub(crate) fn seak_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.seak_attr_word(f, off))
    }

    pub(crate) fn seak_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.seak_attr_word(f, off) as i32
    }

    fn seak_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// How many needles Sheik holds (`u.sk.x0`).
    pub fn seak_needle_count(&self, gobj: GObjId) -> i32 {
        self.game.fighter(gobj).u_i32(u::NEEDLE_COUNT)
    }

    fn set_seak_needle_count(&mut self, gobj: GObjId, n: i32) {
        self.game.fighter_mut(gobj).set_u_i32(u::NEEDLE_COUNT, n);
    }

    /// The held needles' item (`u.sk.x4`).
    pub fn seak_needles(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(u::NEEDLES)
    }

    fn set_seak_needles(&mut self, gobj: GObjId, g: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(u::NEEDLES, g);
    }

    /// The chain (`u.sk.x8`).
    pub fn seak_chain(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(u::CHAIN)
    }

    fn set_seak_chain(&mut self, gobj: GObjId, g: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(u::CHAIN, g);
    }

    /// Calls a function another part of the port owns ([`ext`]).
    pub(crate) fn seak_ext(&mut self, f: u32, args: &[Val]) -> Val {
        code::call(self, f, args)
    }
}

/// Runs one of Sheik's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, accessories, damage and hitlag hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Sheik's functions other code calls by address (`code::call`): the specials' entries, the
/// item hooks, the queries the items ask of Sheik, and every callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args).or_else(|| special_s::call(r, f, args)) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
