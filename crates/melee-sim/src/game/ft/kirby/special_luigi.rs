//! Luigi's copy (`ftkirbyspecialluigi.c`): the Fireball thrown from his hand when the script
//! raises its flag, on the ground or in the air. Luigi's own code
//! ([`ft::luigi::special_n`](super::super::luigi::special_n)) over his states, his hand, the
//! copy's own item kind and its flash ([`KIRBY`]); only his copy's addresses are here.

use super::super::super::state::Running;
use super::super::luigi::special_n::Fireball;
use super::ms;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9614;
    pub const AIR_ENTER: u32 = 0x800F_9684;
    pub const N_ANIM: u32 = 0x800F_96F4;
    pub const AIR_N_ANIM: u32 = 0x800F_9730;
    pub const N_IASA: u32 = 0x800F_976C;
    pub const AIR_N_IASA: u32 = 0x800F_979C;
    pub const N_PHYS: u32 = 0x800F_97CC;
    pub const AIR_N_PHYS: u32 = 0x800F_97EC;
    pub const N_COLL: u32 = 0x800F_980C;
    pub const AIR_N_COLL: u32 = 0x800F_9880;
    pub const THROW: u32 = 0x800F_98F4;
}

/// Kirby's Fireball: his states, his left hand (`FtPart_LHandN`), the copy's item kind and
/// its flash.
pub const KIRBY: Fireball = Fireball {
    ground: ms::LG_SPECIAL_N,
    air: ms::LG_SPECIAL_AIR_N,
    throw: addr::THROW,
    hand: 21,
    item_kind: ItKind::KirbyLuigiFireball.id(),
    flash: 0x4B1,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let f = &KIRBY;
    match a {
        addr::ENTER => g.lg_special_n_enter(gobj, f),
        addr::AIR_ENTER => g.lg_special_air_n_enter(gobj, f),
        addr::N_ANIM => g.lg_special_n_anim(gobj),
        addr::AIR_N_ANIM => g.lg_special_air_n_anim(gobj),
        addr::N_IASA => g.lg_special_n_iasa(gobj),
        addr::AIR_N_IASA => g.lg_special_air_n_iasa(gobj),
        addr::N_PHYS => g.lg_ground_friction(gobj),
        addr::AIR_N_PHYS => g.lg_special_air_n_phys(gobj),
        addr::N_COLL => g.lg_special_n_coll(gobj, f),
        addr::AIR_N_COLL => g.lg_special_air_n_coll(gobj, f),
        addr::THROW => g.lg_special_n_fire_spawn(gobj, f),
        _ => return false,
    }
    true
}
