//! Zelda's copy (`ftkirbyspecialzelda.c`, its reflection's hook at the start of
//! `ftkirbyspecialseak.c`): Nayru's Love as Kirby spins it. Zelda's own code
//! ([`ft::zelda::special_n`](super::super::zelda::special_n)) over his states, her attributes
//! 0x2A4 bytes on in his (`specialn_zd_reflectdesc` 0x37C on), his effects and hooks; changing
//! between the ground and the air keeps the hitlag hooks but not the effect's accessory
//! ([`KIRBY`]). Only his copy's addresses are here.

use super::super::super::state::Running;
use super::super::zelda::special_n::NayrusLove;
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const GFX_GROUND: u32 = 0x8010_5A34;
    pub const GFX_AIR: u32 = 0x8010_5AB0;
    pub const ENTER: u32 = 0x8010_5B2C;
    pub const AIR_ENTER: u32 = 0x8010_5BA8;
    pub const N_ANIM: u32 = 0x8010_5C3C;
    pub const AIR_N_ANIM: u32 = 0x8010_5CD0;
    pub const N_IASA: u32 = 0x8010_5D64;
    pub const AIR_N_IASA: u32 = 0x8010_5D68;
    pub const N_PHYS: u32 = 0x8010_5D6C;
    pub const AIR_N_PHYS: u32 = 0x8010_5DA0;
    pub const N_COLL: u32 = 0x8010_5E14;
    pub const AIR_N_COLL: u32 = 0x8010_5E50;
    pub const TO_AIR: u32 = 0x8010_5E8C;
    pub const TO_GROUND: u32 = 0x8010_5F3C;
    pub const REFLECTED: u32 = 0x8010_5FEC;
}

/// Kirby's Nayru's Love.
pub const KIRBY: NayrusLove = NayrusLove {
    ground: ms::ZD_SPECIAL_N,
    air: ms::ZD_SPECIAL_AIR_N,
    attrs: 0x2A4,
    reflect: 0x37C,
    gfx: (0x4B6, 0x4B7),
    gfx_cb: (addr::GFX_GROUND, addr::GFX_AIR),
    reflected: addr::REFLECTED,
    hooks_on_change: true,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let z = &KIRBY;
    match a {
        addr::GFX_GROUND => g.zelda_special_n_gfx_ground(gobj, z),
        addr::GFX_AIR => g.zelda_special_n_gfx_air(gobj, z),
        addr::ENTER => g.zelda_special_n_enter(gobj, z),
        addr::AIR_ENTER => g.zelda_special_air_n_enter(gobj, z),
        addr::N_ANIM => g.zelda_special_n_anim(gobj, z),
        addr::AIR_N_ANIM => g.zelda_special_air_n_anim(gobj, z),
        addr::N_IASA | addr::AIR_N_IASA | addr::REFLECTED => g.zelda_special_n_nothing(gobj),
        addr::N_PHYS => g.zelda_special_n_phys(gobj),
        addr::AIR_N_PHYS => g.zelda_special_air_n_phys(gobj, z),
        addr::N_COLL => g.zelda_special_n_coll(gobj, z),
        addr::AIR_N_COLL => g.zelda_special_air_n_coll(gobj, z),
        addr::TO_AIR => g.zelda_special_n_to_air(gobj, z),
        addr::TO_GROUND => g.zelda_special_n_to_ground(gobj, z),
        _ => return false,
    }
    true
}
