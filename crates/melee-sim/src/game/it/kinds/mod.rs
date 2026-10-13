//! Item kinds (`ItemKind`) and the kinds' own code.

pub mod bat;
pub mod bombhei;
pub mod capsule;
pub mod crate_box;
pub mod foxblaster;
pub mod foxillusion;
pub mod foxlaser;
pub mod gshell;
pub mod hammer;
pub mod hammerhead;
pub mod heart;
pub mod heiho;
pub mod kusudama;
pub mod rshell;
pub mod seakchain;
pub mod seakneedleheld;
pub mod seakneedlethrown;
pub mod seakvanish;
pub mod sword;
pub mod taru;
pub mod tomato;
pub mod zeldadinfire;
pub mod zeldadinfireexplode;

use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// Runs a kind's callback of the event kind (`HSD_GObjEvent`), if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    foxlaser::run_event(r, gobj, addr)
        || foxblaster::run_event(r, gobj, addr)
        || foxillusion::run_event(r, gobj, addr)
        || capsule::run_event(r, gobj, addr)
        || heart::run_event(r, gobj, addr)
        || tomato::run_event(r, gobj, addr)
        || kusudama::run_event(r, gobj, addr)
        || taru::run_event(r, gobj, addr)
        || crate_box::run_event(r, gobj, addr)
        || bombhei::run_event(r, gobj, addr)
        || rshell::run_event(r, gobj, addr)
        || gshell::run_event(r, gobj, addr)
        || hammerhead::run_event(r, gobj, addr)
        || hammer::run_event(r, gobj, addr)
        || sword::run_event(r, gobj, addr)
        || bat::run_event(r, gobj, addr)
        || seakneedleheld::run_event(r, gobj, addr)
        || seakneedlethrown::run_event(r, gobj, addr)
        || seakvanish::run_event(r, gobj, addr)
        || seakchain::run_event(r, gobj, addr)
        || zeldadinfire::run_event(r, gobj, addr)
        || zeldadinfireexplode::run_event(r, gobj, addr)
        || heiho::run_event(r, gobj, addr)
        || super::zako::run_event(r, gobj, addr)
}

/// Runs a kind's predicate callback (`HSD_GObjPredicate`), if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    foxlaser::run_pred(r, gobj, addr)
        .or_else(|| foxblaster::run_pred(r, gobj, addr))
        .or_else(|| foxillusion::run_pred(r, gobj, addr))
        .or_else(|| capsule::run_pred(r, gobj, addr))
        .or_else(|| heart::run_pred(r, gobj, addr))
        .or_else(|| tomato::run_pred(r, gobj, addr))
        .or_else(|| kusudama::run_pred(r, gobj, addr))
        .or_else(|| taru::run_pred(r, gobj, addr))
        .or_else(|| crate_box::run_pred(r, gobj, addr))
        .or_else(|| bombhei::run_pred(r, gobj, addr))
        .or_else(|| rshell::run_pred(r, gobj, addr))
        .or_else(|| gshell::run_pred(r, gobj, addr))
        .or_else(|| hammerhead::run_pred(r, gobj, addr))
        .or_else(|| hammer::run_pred(r, gobj, addr))
        .or_else(|| sword::run_pred(r, gobj, addr))
        .or_else(|| bat::run_pred(r, gobj, addr))
        .or_else(|| seakneedleheld::run_pred(r, gobj, addr))
        .or_else(|| seakneedlethrown::run_pred(r, gobj, addr))
        .or_else(|| seakvanish::run_pred(r, gobj, addr))
        .or_else(|| seakchain::run_pred(r, gobj, addr))
        .or_else(|| zeldadinfire::run_pred(r, gobj, addr))
        .or_else(|| zeldadinfireexplode::run_pred(r, gobj, addr))
        .or_else(|| heiho::run_pred(r, gobj, addr))
}

/// The common items' kinds (`It_Kind_*`).
pub const CAPSULE: i32 = 0;
pub const BOX: i32 = 1;
pub const TARU: i32 = 2;
pub const EGG: i32 = 3;
pub const KUSUDAMA: i32 = 4;
pub const TARU_CANN: i32 = 5;
pub const BOMBHEI: i32 = 6;
pub const HEART: i32 = 8;
pub const TOMATO: i32 = 9;
pub const BAT: i32 = 11;
pub const SWORD: i32 = 12;
pub const G_SHELL: i32 = 14;
pub const R_SHELL: i32 = 15;
pub const HAMMER: i32 = 28;
/// `It_Kind_Foods`.
pub const FOODS: i32 = 18;
/// `It_Kind_M_Ball`, the Poké Ball.
pub const M_BALL: i32 = 34;
/// `It_Kind_L_Gun_Ray`: the first kind past the common items.
pub const L_GUN_RAY: i32 = 35;
/// `It_Kind_Kuriboh`: the first enemy, and the first kind of the second group of tables.
pub const KURIBOH: i32 = 43;
/// `It_Kind_Octarock_Stone`.
pub const OCTAROCK_STONE: i32 = 47;
/// `It_Kind_Mario_Fire`: the first character item.
pub const MARIO_FIRE: i32 = 48;
/// `It_Kind_Fox_Laser`.
pub const FOX_LASER: i32 = 54;
/// `It_Kind_Falco_Laser`.
pub const FALCO_LASER: i32 = 55;
/// `It_Kind_Fox_Illusion` and `It_Kind_Falco_Phantasm`.
pub const FOX_ILLUSION: i32 = 56;
pub const FALCO_PHANTASM: i32 = 57;
/// `It_Kind_Link_Bomb`.
pub const LINK_BOMB: i32 = 58;
/// `It_Kind_CLink_Bomb`.
pub const CLINK_BOMB: i32 = 59;
/// `It_Kind_Zelda_DinFire` and `_DinFire_Explode`.
pub const ZELDA_DIN_FIRE: i32 = 108;
pub const ZELDA_DIN_FIRE_EXPLODE: i32 = 109;
/// `It_Kind_Fox_Blaster`.
pub const FOX_BLASTER: i32 = 74;
/// `It_Kind_Falco_Blaster`.
pub const FALCO_BLASTER: i32 = 75;
/// `It_Kind_Seak_NeedleThrow`, `_NeedleHeld`, `_Vanish`, `_Chain`.
pub const SEAK_NEEDLE_THROW: i32 = 79;
pub const SEAK_NEEDLE_HELD: i32 = 80;
pub const SEAK_VANISH: i32 = 85;
pub const SEAK_CHAIN: i32 = 97;
/// `It_Kind_Peach_Turnip`.
pub const PEACH_TURNIP: i32 = 99;
/// `It_Kind_Peach_Parasol`.
pub const PEACH_PARASOL: i32 = 103;
/// `It_Kind_Unk4`.
pub const UNK4: i32 = 158;
/// `It_PKind_Random`.
pub const PKIND_RANDOM: i32 = 160;
/// `It_PKind_Start`: the first Pokémon, and the third group of tables.
pub const PKIND_START: i32 = 161;
/// `It_PKind_Terminate`.
pub const PKIND_TERMINATE: i32 = 191;
/// `It_Kind_Old_Kuri`: the first stage item, and the last group of tables.
pub const OLD_KURI: i32 = 208;
/// `It_Kind_Heiho`, Yoshi's Story's Shy Guys.
pub const HEIHO: i32 = 210;
/// `It_Kind_Arwing_Laser`.
pub const ARWING_LASER: i32 = 234;
