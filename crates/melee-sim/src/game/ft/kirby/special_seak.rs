//! Sheik's copy (`ftkirbyspecialseak.c`): the needles as Kirby draws them, charges up to six
//! while B is held, cancels with a shield button and throws one at a time. Sheik's own code
//! ([`ft::seak::special_n`](super::super::seak::special_n)) over his states, Sheik's
//! attributes 0x290 bytes on in his, the count and the needles in his `u` (`xB4`, `xB8`), his
//! bone and glow, the copy's own item kinds, his generic cleanups as hooks, and one branch of
//! his own ([`KIRBY`]); the needles' variables are laid out as Sheik's (`mv.kb.specialhi`).
//! Only his copy's addresses are here.

use super::super::super::it::kinds as k;
use super::super::super::state::Running;
use super::super::seak::ms as sk;
use super::super::seak::special_n::Needles;
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const DROP: u32 = 0x8010_603C;
    pub const ENTER: u32 = 0x8010_612C;
    pub const AIR_ENTER: u32 = 0x8010_61E4;
    pub const N_START_ANIM: u32 = 0x8010_629C;
    pub const N_LOOP_ANIM: u32 = 0x8010_6334;
    pub const N_CANCEL_ANIM: u32 = 0x8010_63E0;
    pub const N_END_ANIM: u32 = 0x8010_6428;
    pub const AIR_N_START_ANIM: u32 = 0x8010_64AC;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_6544;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_65F0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_6670;
    pub const N_START_IASA: u32 = 0x8010_672C;
    pub const N_LOOP_IASA: u32 = 0x8010_6730;
    pub const N_CANCEL_IASA: u32 = 0x8010_680C;
    pub const N_END_IASA: u32 = 0x8010_6810;
    pub const AIR_N_START_IASA: u32 = 0x8010_6814;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_6818;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_68F4;
    pub const AIR_N_END_IASA: u32 = 0x8010_68F8;
    pub const N_START_PHYS: u32 = 0x8010_68FC;
    pub const N_LOOP_PHYS: u32 = 0x8010_691C;
    pub const N_CANCEL_PHYS: u32 = 0x8010_693C;
    pub const N_END_PHYS: u32 = 0x8010_695C;
    pub const AIR_N_START_PHYS: u32 = 0x8010_697C;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_699C;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_69BC;
    pub const AIR_N_END_PHYS: u32 = 0x8010_69DC;
    pub const N_START_COLL: u32 = 0x8010_69FC;
    pub const N_LOOP_COLL: u32 = 0x8010_6A84;
    pub const N_CANCEL_COLL: u32 = 0x8010_6B0C;
    pub const N_END_COLL: u32 = 0x8010_6B84;
    pub const AIR_N_START_COLL: u32 = 0x8010_6C10;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_6C98;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_6D20;
    pub const AIR_N_END_COLL: u32 = 0x8010_6D5C;
    pub const SHOOT: u32 = 0x8010_6DB0;
}

/// Kirby's needles: held on his bone 39, his own glow at full charge, and his damage hooks
/// staying through the cancels, the throws' leaving the ground or landing, and the needles
/// dropped.
pub const KIRBY: Needles = Needles {
    states: ms::SK_SPECIAL_N_START - sk::SPECIAL_N_START,
    attrs: 0x290,
    u: 0xB4,
    needle_part: 39,
    held_kind: k::KIRBY_SEAK_NEEDLE_HELD,
    thrown_kind: k::KIRBY_SEAK_NEEDLE_THROW,
    full_glow: 87,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    clears_hooks: false,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::DROP => g.seak_drop_needles(gobj, p),
        addr::ENTER => g.seak_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.seak_special_air_n_enter(gobj, p),
        addr::N_START_ANIM => g.seak_special_n_start_anim(gobj, p, false),
        addr::AIR_N_START_ANIM => g.seak_special_n_start_anim(gobj, p, true),
        addr::N_LOOP_ANIM | addr::AIR_N_LOOP_ANIM => g.seak_special_n_loop_anim(gobj, p),
        addr::N_CANCEL_ANIM => g.seak_special_n_cancel_anim(gobj, p, false),
        addr::AIR_N_CANCEL_ANIM => g.seak_special_n_cancel_anim(gobj, p, true),
        addr::N_END_ANIM => g.seak_special_n_end_anim(gobj, p, false),
        addr::AIR_N_END_ANIM => g.seak_special_n_end_anim(gobj, p, true),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_CANCEL_IASA
        | addr::AIR_N_END_IASA => g.seak_no_iasa(gobj),
        addr::N_LOOP_IASA => g.seak_special_n_loop_iasa(gobj, p, false, addr::SHOOT),
        addr::AIR_N_LOOP_IASA => g.seak_special_n_loop_iasa(gobj, p, true, addr::SHOOT),
        addr::N_START_PHYS | addr::N_LOOP_PHYS | addr::N_CANCEL_PHYS | addr::N_END_PHYS => {
            g.seak_ground_phys(gobj)
        }
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_CANCEL_PHYS
        | addr::AIR_N_END_PHYS => g.seak_air_phys(gobj),
        addr::N_START_COLL => g.seak_special_n_coll(gobj, p, sk::SPECIAL_N_START),
        addr::N_LOOP_COLL => g.seak_special_n_coll(gobj, p, sk::SPECIAL_N_LOOP),
        addr::N_CANCEL_COLL => g.seak_special_n_cancel_coll(gobj, p),
        addr::N_END_COLL => g.seak_special_n_end_coll(gobj, p),
        addr::AIR_N_START_COLL => g.seak_special_air_n_coll(gobj, p, sk::SPECIAL_N_START),
        addr::AIR_N_LOOP_COLL => g.seak_special_air_n_coll(gobj, p, sk::SPECIAL_N_LOOP),
        addr::AIR_N_CANCEL_COLL => g.seak_special_air_n_cancel_coll(gobj, p),
        addr::AIR_N_END_COLL => g.seak_special_air_n_end_coll(gobj, p),
        addr::SHOOT => g.seak_shoot_needles(gobj, p),
        _ => return false,
    }
    true
}
