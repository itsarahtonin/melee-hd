//! Captain Falcon's and Ganondorf's copies (`ftkirbyspecialcaptain.c`): the Falcon and Warlock
//! Punch, wind-up and punch, on the ground and in the air, where the stick angles the lunge.
//! Captain Falcon's own code ([`ft::captain::special_n`](super::super::captain::special_n))
//! over his states and attributes, his flames from his top and hand joints, the kind his
//! copy's and no wind ([`KIRBY`]); only his copies' addresses are here.

use super::super::super::state::Running;
use super::super::captain::special_n::FalconPunch;
use super::{KindFrom, attr, ms};
use crate::hsd::gobj::GObjId;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_99BC;
    pub const AIR_ENTER: u32 = 0x800F_9A54;
    pub const N_ANIM: u32 = 0x800F_9AEC;
    pub const AIR_N_ANIM: u32 = 0x800F_9B28;
    pub const N_IASA: u32 = 0x800F_9B64;
    pub const AIR_N_IASA: u32 = 0x800F_9B68;
    pub const N_PHYS: u32 = 0x800F_9C58;
    pub const AIR_N_PHYS: u32 = 0x800F_9D40;
    pub const N_COLL: u32 = 0x800F_9E8C;
    pub const AIR_N_COLL: u32 = 0x800F_9F34;
}

/// Kirby's punches: his states for each copy, his attributes (`specialn_ca_*`), his copies'
/// flames, his hand (`fp->parts[44]`) for either, and no wind.
pub const KIRBY: FalconPunch = FalconPunch {
    kind: KindFrom::Copy,
    captain: (ms::CA_SPECIAL_N, ms::CA_SPECIAL_AIR_N),
    ganon: (ms::GN_SPECIAL_N, ms::GN_SPECIAL_AIR_N),
    attrs: attr::CA_PUNCH,
    flame: (0x4AB, 0x4B3),
    fist: (44, 44),
    wind: false,
};

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::ENTER => g.ca_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.ca_special_air_n_enter(gobj, p),
        addr::N_ANIM => g.ca_special_n_anim(gobj, p),
        addr::AIR_N_ANIM => g.ca_special_air_n_anim(gobj, p),
        addr::N_IASA => g.ca_special_n_iasa(gobj),
        addr::AIR_N_IASA => g.ca_special_air_n_iasa(gobj, p),
        addr::N_PHYS => g.ca_special_n_phys(gobj, p),
        addr::AIR_N_PHYS => g.ca_special_air_n_phys(gobj, p),
        addr::N_COLL => g.ca_special_n_coll(gobj, p),
        addr::AIR_N_COLL => g.ca_special_air_n_coll(gobj, p),
        _ => return false,
    }
    true
}
