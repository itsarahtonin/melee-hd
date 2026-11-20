//! Mario's and Dr. Mario's copies (`ftkirbyspecialmario.c`): the Fireball or the Megavitamin
//! thrown from his hand when the script raises its flag, on the ground or in the air. Mario's
//! own code ([`ft::mario::special_n`](super::super::mario::special_n)) over his states for
//! each copy, his hand, the copies' own item kinds and flash, the pill's colors in his `u`
//! (`x68`, `x6C`) and the kind his copy's ([`KIRBY`]); only his copies' addresses are here.

use super::super::super::state::Running;
use super::super::mario::special_n::Fireball;
use super::{KindFrom, ms};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9110;
    pub const N_ANIM: u32 = 0x800F_9198;
    pub const N_IASA: u32 = 0x800F_91D4;
    pub const N_PHYS: u32 = 0x800F_9204;
    pub const N_COLL: u32 = 0x800F_9224;
    pub const THROW: u32 = 0x800F_9260;
    pub const AIR_ENTER: u32 = 0x800F_93CC;
    pub const AIR_N_ANIM: u32 = 0x800F_9454;
    pub const AIR_N_IASA: u32 = 0x800F_9490;
    pub const AIR_N_PHYS: u32 = 0x800F_94C0;
    pub const AIR_N_COLL: u32 = 0x800F_94E0;
    pub const TO_AIR: u32 = 0x800F_951C;
    pub const TO_GROUND: u32 = 0x800F_9598;
}

/// Kirby's copies: Mario's states for Mario's copy, Dr. Mario's for any other, his left hand
/// (`FtPart_LHandN`), the copies' items and Mario's copy's flash.
pub const KIRBY: Fireball = Fireball {
    kind: KindFrom::Copy,
    mario: (ms::MR_SPECIAL_N, ms::MR_SPECIAL_AIR_N),
    doctor: (ms::DR_SPECIAL_N, ms::DR_SPECIAL_AIR_N),
    throw: addr::THROW,
    hand: 21,
    fire_kind: ItKind::KirbyMarioFireball.id(),
    pill_kind: ItKind::KirbyDrMarioMegavitamin.id(),
    flash: 0x49F,
    vitamin_curr: 0x68,
    vitamin_prev: 0x6C,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let f = &KIRBY;
    match a {
        addr::ENTER => g.mr_special_n_enter(gobj, f),
        addr::AIR_ENTER => g.mr_special_air_n_enter(gobj, f),
        addr::N_ANIM => g.mr_special_n_anim(gobj),
        addr::AIR_N_ANIM => g.mr_special_air_n_anim(gobj),
        addr::N_IASA => g.mr_special_n_iasa(gobj),
        addr::AIR_N_IASA => g.mr_special_air_n_iasa(gobj),
        addr::N_PHYS => g.mr_special_n_phys(gobj),
        addr::AIR_N_PHYS => g.mr_special_air_n_phys(gobj),
        addr::N_COLL => g.mr_special_n_coll(gobj, f),
        addr::AIR_N_COLL => g.mr_special_air_n_coll(gobj, f),
        addr::TO_AIR => g.mr_special_n_ground_to_air(gobj, f),
        addr::TO_GROUND => g.mr_special_air_n_air_to_ground(gobj, f),
        addr::THROW => g.mr_special_n_item_fire_spawn(gobj, f),
        _ => return false,
    }
    true
}
