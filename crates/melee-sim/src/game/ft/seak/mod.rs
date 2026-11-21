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

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

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
    #[doc(alias("S_8011097C"))]
    pub const SPECIAL_S_NOTHING: u32 = 0x8011_097C;
    pub const S_UPDATE_HITBOXES: u32 = 0x8011_0980;
    pub const S_ZERO_HITBOX_POSITIONS: u32 = 0x8011_0A80;
    #[doc(alias("S_80110E4C"))]
    pub const SPECIAL_S_CHAIN_GONE: u32 = 0x8011_0E4C;
    pub const S_CHECK_AND_DESTROY_CHAIN: u32 = 0x8011_0E88;
    #[doc(alias("S_80110EE8"))]
    pub const SPECIAL_S_CHAIN_PAUSE: u32 = 0x8011_0EE8;
    pub const S_CHAIN_SOMETHING: u32 = 0x8011_0F18;
    #[doc(alias("S_80110F58"))]
    pub const STICK_X: u32 = 0x8011_0F58;
    #[doc(alias("S_80110F64"))]
    pub const STICK_Y: u32 = 0x8011_0F64;
    #[doc(alias("S_80110F70"))]
    pub const SPECIAL_S_INIT: u32 = 0x8011_0F70;
    pub const SPECIAL_S_ENTER: u32 = 0x8011_1038;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8011_108C;
    pub const S_CHECK_INIT_CHAIN: u32 = 0x8011_10E8;
    #[doc(alias("S_80111F70"))]
    pub const LETS_GO: u32 = 0x8011_1F70;
    #[doc(alias("S_80111FA0"))]
    pub const HELD_COUNT: u32 = 0x8011_1FA0;
    // The neutral special.
    #[doc(alias("N_80111FBC"))]
    pub const DROP_NEEDLES: u32 = 0x8011_1FBC;
    #[doc(alias("N_801120D4"))]
    pub const CLEAR_DAMAGE_CB: u32 = 0x8011_20D4;
    pub const SPECIAL_N_ENTER: u32 = 0x8011_20E8;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8011_2198;
    pub const SHOOT_NEEDLES: u32 = 0x8011_2D44;
    // The up special.
    #[doc(alias("HI_FN_80112ED8"))]
    pub const SPECIAL_HI_VANISH: u32 = 0x8011_2ED8;
    #[doc(alias("HI_FN_80113038"))]
    pub const SPECIAL_HI_APPEAR_GFX: u32 = 0x8011_3038;
    pub const SPECIAL_HI_ENTER: u32 = 0x8011_30D0;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8011_312C;
    // The down special.
    #[doc(alias("LW_FN_80114034"))]
    pub const SPECIAL_LW_START_GFX: u32 = 0x8011_4034;
    #[doc(alias("LW_FN_801140B0"))]
    pub const SPECIAL_LW_END_GFX: u32 = 0x8011_40B0;
    #[doc(alias("LW_FN_8011412C"))]
    pub const SPECIAL_LW_SWAP: u32 = 0x8011_412C;
    pub const SPECIAL_LW_ENTER: u32 = 0x8011_4160;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8011_4224;
    #[doc(alias("LW_80114758"))]
    pub const SPECIAL_LW_FINISH: u32 = 0x8011_4758;
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
}

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

    /// The held needles' item (`u.sk.x4`).
    pub fn seak_needles(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(u::NEEDLES)
    }

    /// The chain (`u.sk.x8`).
    pub fn seak_chain(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(u::CHAIN)
    }

    fn set_seak_chain(&mut self, gobj: GObjId, g: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(u::CHAIN, g);
    }
}

/// Runs one of Sheik's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, accessories, damage and hitlag hooks), if `addr` is one.
#[melee_callbacks::callbacks(Fighter)]
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Sheik's functions other code calls by address (`code::call`): the specials' entries, the
/// item hooks, the queries the items ask of Sheik, and every callback [`run`] takes.
#[melee_callbacks::callbacks(Call)]
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args).or_else(|| special_s::call(r, f, args)) {
        return Some(v);
    }
    None
}
