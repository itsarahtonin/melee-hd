//! Ness's copy (`ftkirbyspecialness.c`, with its item's checks at the end of
//! `ftkirbyspecialfox.c`): PK Flash as Kirby charges it. Ness's own code
//! ([`ft::ness::special_n`](super::super::ness::special_n)) over his states, Ness's attributes
//! 0x1A0 bytes on in his, the flash kept in his `u` (`x7C`), his waist for the flash, the
//! copy's own flash kind, his generic cleanups as hooks, and one branch of his own
//! ([`KIRBY`]); the charge's variables are laid out as Ness's (`mv.ns.specialn`). Only his
//! copy's addresses are here.

use super::super::super::it::kinds as k;
use super::super::super::state::Running;
use super::super::ness::ms as ns;
use super::super::ness::special_n::PkFlash;
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const FORGET: u32 = 0x800F_ECBC;
    pub const LET_GO: u32 = 0x800F_ECE8;
    pub const ENTER: u32 = 0x800F_ED38;
    pub const AIR_ENTER: u32 = 0x800F_EDD0;
    pub const N_START_ANIM: u32 = 0x800F_EE70;
    pub const N_HOLD_ANIM: u32 = 0x800F_EF58;
    pub const N_END_ANIM: u32 = 0x800F_F08C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_F10C;
    pub const AIR_N_HOLD_ANIM: u32 = 0x800F_F1F4;
    pub const AIR_N_END_ANIM: u32 = 0x800F_F328;
    pub const N_START_IASA: u32 = 0x800F_F3DC;
    pub const N_HOLD_IASA: u32 = 0x800F_F3E0;
    pub const N_END_IASA: u32 = 0x800F_F450;
    pub const AIR_N_START_IASA: u32 = 0x800F_F454;
    pub const AIR_N_HOLD_IASA: u32 = 0x800F_F458;
    pub const AIR_N_END_IASA: u32 = 0x800F_F4C8;
    pub const N_START_PHYS: u32 = 0x800F_F4CC;
    pub const N_HOLD_PHYS: u32 = 0x800F_F504;
    pub const N_END_PHYS: u32 = 0x800F_F524;
    pub const AIR_N_START_PHYS: u32 = 0x800F_F544;
    pub const AIR_N_HOLD_PHYS: u32 = 0x800F_F5A4;
    pub const AIR_N_END_PHYS: u32 = 0x800F_F604;
    pub const N_START_COLL: u32 = 0x800F_F664;
    pub const N_HOLD_COLL: u32 = 0x800F_F6D0;
    pub const N_END_COLL: u32 = 0x800F_F73C;
    pub const AIR_N_START_COLL: u32 = 0x800F_F7A8;
    pub const AIR_N_HOLD_COLL: u32 = 0x800F_F814;
    pub const AIR_N_END_COLL: u32 = 0x800F_F880;
}

/// Kirby's PK Flash. The flash comes out of his waist (`FtPart_WaistN`, by bone index), his
/// hooks are his generic cleanups, and the flash let go of leaves them in place.
pub const KIRBY: PkFlash = PkFlash {
    states: ms::NS_SPECIAL_N_START - ns::SPECIAL_N_START,
    attrs: 0x1A0,
    u: 0x68,
    bone: 5,
    flash_kind: k::KIRBY_NESS_PK_FLUSH,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    clears_hooks: false,
};

/// The copy's callbacks; its empty interrupts do nothing, and the charge's and the end's
/// grounded physics are friction.
#[doc(alias = "ftKb_NsSpecialNStart_IASA")]
#[doc(alias = "ftKb_NsSpecialNEnd_IASA")]
#[doc(alias = "ftKb_NsSpecialAirNStart_IASA")]
#[doc(alias = "ftKb_NsSpecialAirNEnd_IASA")]
#[doc(alias = "ftKb_NsSpecialNHold_Phys")]
#[doc(alias = "ftKb_NsSpecialNEnd_Phys")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::FORGET => g.ness_special_n_set_null(gobj, p),
        addr::LET_GO => g.ness_pkflash_set_null(gobj, p),
        addr::ENTER => g.ness_special_n_start_enter(gobj, p),
        addr::AIR_ENTER => g.ness_special_air_n_start_enter(gobj, p),
        addr::N_START_ANIM => g.ness_special_n_start_anim(gobj, p, false),
        addr::AIR_N_START_ANIM => g.ness_special_n_start_anim(gobj, p, true),
        addr::N_HOLD_ANIM => g.ness_special_n_charge(gobj, p, false),
        addr::AIR_N_HOLD_ANIM => g.ness_special_n_charge(gobj, p, true),
        addr::N_END_ANIM => g.ness_special_n_end_anim(gobj, p),
        addr::AIR_N_END_ANIM => g.ness_special_air_n_end_anim(gobj, p),
        addr::N_START_IASA | addr::N_END_IASA | addr::AIR_N_START_IASA | addr::AIR_N_END_IASA => {}
        addr::N_HOLD_IASA | addr::AIR_N_HOLD_IASA => g.ness_special_n_charge_iasa(gobj, p),
        addr::N_START_PHYS => g.ness_special_n_start_phys(gobj),
        addr::N_HOLD_PHYS | addr::N_END_PHYS => g.ground_friction(gobj),
        addr::AIR_N_START_PHYS | addr::AIR_N_HOLD_PHYS | addr::AIR_N_END_PHYS => {
            g.ness_special_air_n_phys(gobj, p)
        }
        addr::N_START_COLL => g.ness_special_n_ground_coll(gobj, p, ns::SPECIAL_N_START),
        addr::N_HOLD_COLL => g.ness_special_n_ground_coll(gobj, p, ns::SPECIAL_N_HOLD),
        addr::N_END_COLL => g.ness_special_n_ground_coll(gobj, p, ns::SPECIAL_N_END),
        addr::AIR_N_START_COLL => g.ness_special_air_n_land(gobj, p, ns::SPECIAL_N_START),
        addr::AIR_N_HOLD_COLL => g.ness_special_air_n_land(gobj, p, ns::SPECIAL_N_HOLD),
        addr::AIR_N_END_COLL => g.ness_special_air_n_land(gobj, p, ns::SPECIAL_N_END),
        _ => return false,
    }
    true
}
