//! Donkey Kong's copy (`ftkirbyspecialdonkey.c`): the Giant Punch as Kirby swings it. Donkey
//! Kong's own code ([`ft::donkey::special_n`](super::super::donkey::special_n)) over his
//! states, his attributes, the swings stored in his `u` (`xBC`), his effects and glow and his
//! own hooks ([`KIRBY`]); the punch's variables are Donkey Kong's (`mv.dk`). Only his copy's
//! addresses are here.

use super::super::super::state::Running;
use super::super::donkey::attr as dk;
use super::super::donkey::special_n::{GiantPunch, States};
use super::{attr, ms, u};
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_F8EC;
    pub const AIR_ENTER: u32 = 0x800F_FA10;
    pub const N_START_ANIM: u32 = 0x800F_FB24;
    pub const N_LOOP_ANIM: u32 = 0x800F_FBAC;
    pub const N_CANCEL_ANIM: u32 = 0x800F_FC48;
    pub const N_ANIM: u32 = 0x800F_FC94;
    pub const N_FULL_ANIM: u32 = 0x800F_FE6C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_FFB8;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_0040;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_00DC;
    pub const AIR_N_ANIM: u32 = 0x8010_0128;
    pub const AIR_N_FULL_ANIM: u32 = 0x8010_02C8;
    pub const N_START_IASA: u32 = 0x8010_03DC;
    pub const N_LOOP_IASA: u32 = 0x8010_03E0;
    pub const N_CANCEL_IASA: u32 = 0x8010_0518;
    pub const N_IASA: u32 = 0x8010_051C;
    pub const N_FULL_IASA: u32 = 0x8010_0520;
    pub const AIR_N_START_IASA: u32 = 0x8010_0524;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_0528;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_0654;
    pub const AIR_N_IASA: u32 = 0x8010_0658;
    pub const AIR_N_FULL_IASA: u32 = 0x8010_065C;
    pub const N_START_PHYS: u32 = 0x8010_0660;
    pub const N_LOOP_PHYS: u32 = 0x8010_0680;
    pub const N_CANCEL_PHYS: u32 = 0x8010_06A0;
    pub const N_PHYS: u32 = 0x8010_06C0;
    pub const N_FULL_PHYS: u32 = 0x8010_06E0;
    pub const AIR_N_START_PHYS: u32 = 0x8010_0700;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_0720;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_0740;
    pub const AIR_N_PHYS: u32 = 0x8010_0760;
    pub const AIR_N_FULL_PHYS: u32 = 0x8010_0780;
    pub const N_START_COLL: u32 = 0x8010_07A0;
    pub const N_LOOP_COLL: u32 = 0x8010_0840;
    pub const N_CANCEL_COLL: u32 = 0x8010_08E0;
    pub const N_COLL: u32 = 0x8010_0980;
    pub const N_FULL_COLL: u32 = 0x8010_0A20;
    pub const AIR_N_START_COLL: u32 = 0x8010_0AC0;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_0B60;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_0C00;
    pub const AIR_N_COLL: u32 = 0x8010_0CA0;
    pub const AIR_N_FULL_COLL: u32 = 0x8010_0D40;
    pub const CLEANUP: u32 = 0x8010_0DE0;
}

/// Kirby's Giant Punch: his states, Donkey Kong's attributes 0x164 bytes on in his
/// (`specialn_dk_*` from `ftKb_DatAttrs` x190), the swings in his `u` (`xBC`), his effects and
/// glow, and his generic cleanups (`ftKb_Init_800EE74C` on a KO, `_800EE7B8` when hit) with
/// no any-damage hook.
pub const KIRBY: GiantPunch = GiantPunch {
    ms: States {
        special_n_start: ms::DK_SPECIAL_N_START,
        special_n_loop: ms::DK_SPECIAL_N_LOOP,
        special_n_cancel: ms::DK_SPECIAL_N_CANCEL,
        special_n: ms::DK_SPECIAL_N,
        special_n_full: ms::DK_SPECIAL_N_FULL,
        special_air_n_start: ms::DK_SPECIAL_AIR_N_START,
        special_air_n_loop: ms::DK_SPECIAL_AIR_N_LOOP,
        special_air_n_cancel: ms::DK_SPECIAL_AIR_N_CANCEL,
        special_air_n: ms::DK_SPECIAL_AIR_N,
        special_air_n_full: ms::DK_SPECIAL_AIR_N_FULL,
    },
    attrs: attr::DK_SWINGS_TO_FULL - dk::MAX_ARM_SWINGS,
    swings: u::DK_SWINGS,
    effect: (1193, 1194),
    full_glow: 58,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    on_any_damage: None,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    let p = &KIRBY;
    match a {
        ENTER => g.dk_special_n_enter(gobj, p),
        AIR_ENTER => g.dk_special_air_n_enter(gobj, p),
        N_START_ANIM => g.dk_special_n_start_anim_cb(gobj, p),
        N_LOOP_ANIM => g.dk_special_n_loop_anim_cb(gobj, p),
        N_CANCEL_ANIM => g.dk_special_n_cancel_anim(gobj, p),
        N_ANIM => g.dk_special_n_anim(gobj, p),
        N_FULL_ANIM => g.dk_special_n_full_anim(gobj, p),
        AIR_N_START_ANIM => g.dk_special_air_n_start_anim(gobj, p),
        AIR_N_LOOP_ANIM => g.dk_special_air_n_loop_anim(gobj, p),
        AIR_N_CANCEL_ANIM => g.dk_special_air_n_cancel_anim(gobj, p),
        AIR_N_ANIM => g.dk_special_air_n_anim(gobj, p),
        AIR_N_FULL_ANIM => g.dk_special_air_n_full_anim(gobj, p),
        N_START_IASA | N_CANCEL_IASA | N_IASA | N_FULL_IASA | AIR_N_START_IASA
        | AIR_N_CANCEL_IASA | AIR_N_IASA | AIR_N_FULL_IASA => g.dk_special_n_nothing(gobj),
        N_LOOP_IASA => g.dk_special_n_loop_iasa_cb(gobj, p),
        AIR_N_LOOP_IASA => g.dk_special_air_n_loop_iasa(gobj, p),
        N_START_PHYS | N_LOOP_PHYS | N_CANCEL_PHYS | N_PHYS | N_FULL_PHYS => {
            g.dk_special_n_phys(gobj)
        }
        AIR_N_START_PHYS | AIR_N_LOOP_PHYS | AIR_N_CANCEL_PHYS | AIR_N_PHYS | AIR_N_FULL_PHYS => {
            g.dk_special_air_n_phys(gobj)
        }
        N_START_COLL => g.dk_special_n_start_coll(gobj, p),
        N_LOOP_COLL => g.dk_special_n_loop_coll(gobj, p),
        N_CANCEL_COLL => g.dk_special_n_cancel_coll(gobj, p),
        N_COLL => g.dk_special_n_punch_coll(gobj, p),
        N_FULL_COLL => g.dk_special_n_full_coll(gobj, p),
        AIR_N_START_COLL => g.dk_special_air_n_start_coll(gobj, p),
        AIR_N_LOOP_COLL => g.dk_special_air_n_loop_coll(gobj, p),
        AIR_N_CANCEL_COLL => g.dk_special_air_n_cancel_coll(gobj, p),
        AIR_N_COLL => g.dk_special_air_n_punch_coll(gobj, p),
        AIR_N_FULL_COLL => g.dk_special_air_n_full_coll(gobj, p),
        CLEANUP => g.dk_special_n_destroy_effects_plus(gobj, p),
        _ => return false,
    }
    true
}
