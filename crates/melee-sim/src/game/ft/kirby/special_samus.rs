//! Samus's copy (`ftkirbyspecialsamus.c`): her Charge Shot as Kirby charges it. Samus's own
//! code ([`ft::samus::special_n`](super::super::samus::special_n)) over his states, her
//! attributes 0x150 bytes on in his, the shot he holds, the charge and the effects' flag in his
//! `u` (`xA4`, `xA8`, `xAC`), his hand for the shot, the copy's own shot kind, flash and glow,
//! his generic cleanups as hooks, and two branches of his own ([`KIRBY`]); the charge's
//! variables are laid out as Samus's (`mv.kb.specialhi`). Only his copy's addresses are here.

use super::super::super::it::kinds as k;
use super::super::super::state::Running;
use super::super::samus::special_n::{ChargeShot, States};
use super::{attr, ms};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const SHOT_RELEASED: u32 = 0x800F_CD04;
    pub const DROP_CHARGE: u32 = 0x800F_CD60;
    pub const SHOT_FIRE: u32 = 0x800F_CDE0;
    pub const ENTER: u32 = 0x800F_CF74;
    pub const AIR_ENTER: u32 = 0x800F_D020;
    pub const N_START_ANIM: u32 = 0x800F_D0BC;
    pub const N_HOLD_ANIM: u32 = 0x800F_D250;
    pub const N_CANCEL_ANIM: u32 = 0x800F_D418;
    pub const N_ANIM: u32 = 0x800F_D49C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_D4E0;
    pub const AIR_N_ANIM: u32 = 0x800F_D608;
    pub const N_START_IASA: u32 = 0x800F_D688;
    pub const N_HOLD_IASA: u32 = 0x800F_D68C;
    pub const N_CANCEL_IASA: u32 = 0x800F_D7E0;
    pub const N_IASA: u32 = 0x800F_D7E4;
    pub const AIR_N_START_IASA: u32 = 0x800F_D7E8;
    pub const AIR_N_IASA: u32 = 0x800F_D7EC;
    pub const N_START_PHYS: u32 = 0x800F_D7F0;
    pub const N_HOLD_PHYS: u32 = 0x800F_D810;
    pub const N_CANCEL_PHYS: u32 = 0x800F_D830;
    pub const N_PHYS: u32 = 0x800F_D850;
    pub const AIR_N_START_PHYS: u32 = 0x800F_D870;
    pub const AIR_N_PHYS: u32 = 0x800F_D890;
    pub const N_START_COLL: u32 = 0x800F_D8B0;
    pub const N_HOLD_COLL: u32 = 0x800F_D938;
    pub const N_CANCEL_COLL: u32 = 0x800F_D9E0;
    pub const N_COLL: u32 = 0x800F_DA68;
    pub const AIR_N_START_COLL: u32 = 0x800F_DAF0;
    pub const AIR_N_COLL: u32 = 0x800F_DB78;
}

/// Kirby's Charge Shot. His hand (`FtPart_R3rdNa`) holds the shot and fires it, the fire's
/// position taken with a zero offset, and his charge's effects are destroyed whether or not
/// any are out.
pub const KIRBY: ChargeShot = ChargeShot {
    ms: States {
        special_n_start: ms::SS_SPECIAL_N_START,
        special_n_hold: ms::SS_SPECIAL_N_HOLD,
        special_n_cancel: ms::SS_SPECIAL_N_CANCEL,
        special_n: ms::SS_SPECIAL_N,
        special_air_n_start: ms::SS_SPECIAL_AIR_N_START,
        special_air_n: ms::SS_SPECIAL_AIR_N,
    },
    attrs: attr::SS_CHARGE_TIME - 0x18,
    u: 0xA4,
    hand: 44,
    fire_joint: 44,
    fire_offset: Some(Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }),
    shot_kind: k::KIRBY_SAMUS_CHARGE,
    flash: 0x4A4,
    full_glow: 0x36,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    effects_off_always: true,
};

/// The copy's callbacks; its empty interrupts are taken and do nothing, its grounded states
/// take friction (`ft_80084F3C`) and its aerial ones a fall without drift (`ft_80084EEC`).
#[doc(alias = "ftKb_SsSpecialNStart_IASA")]
#[doc(alias = "ftKb_SsSpecialNCancel_IASA")]
#[doc(alias = "ftKb_SsSpecialN_IASA")]
#[doc(alias = "ftKb_SsSpecialAirNStart_IASA")]
#[doc(alias = "ftKb_SsSpecialAirN_IASA")]
#[doc(alias = "ftKb_SsSpecialNStart_Phys")]
#[doc(alias = "ftKb_SsSpecialNHold_Phys")]
#[doc(alias = "ftKb_SsSpecialNCancel_Phys")]
#[doc(alias = "ftKb_SsSpecialN_Phys")]
#[doc(alias = "ftKb_SsSpecialAirNStart_Phys")]
#[doc(alias = "ftKb_SsSpecialAirN_Phys")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::SHOT_RELEASED => g.samus_shot_released(gobj, p),
        addr::DROP_CHARGE => g.samus_drop_charge(gobj, p),
        addr::SHOT_FIRE => g.samus_shot_fire(gobj, p),
        addr::ENTER => g.samus_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.samus_special_air_n_enter(gobj, p),
        addr::N_START_ANIM => g.samus_special_n_start_anim(gobj, p),
        addr::N_HOLD_ANIM => g.samus_special_n_hold_anim(gobj, p),
        addr::N_CANCEL_ANIM => g.samus_special_n_cancel_anim(gobj, p),
        addr::N_ANIM => g.samus_special_n_anim(gobj, p),
        addr::AIR_N_START_ANIM => g.samus_special_air_n_start_anim(gobj, p),
        addr::AIR_N_ANIM => g.samus_special_air_n_anim(gobj, p),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_IASA => {}
        addr::N_HOLD_IASA => g.samus_special_n_hold_iasa(gobj, p),
        addr::N_START_PHYS | addr::N_HOLD_PHYS | addr::N_CANCEL_PHYS | addr::N_PHYS => {
            g.ground_friction(gobj)
        }
        addr::AIR_N_START_PHYS | addr::AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::N_START_COLL => g.samus_special_n_start_coll(gobj, p),
        addr::N_HOLD_COLL => g.samus_special_n_hold_coll(gobj, p),
        addr::N_CANCEL_COLL | addr::N_COLL => g.samus_special_n_coll(gobj, p),
        addr::AIR_N_START_COLL => g.samus_special_air_n_start_coll(gobj, p),
        addr::AIR_N_COLL => g.samus_special_air_n_coll(gobj, p),
        _ => return false,
    }
    true
}
