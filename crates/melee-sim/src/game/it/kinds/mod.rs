//! Item kinds (`ItemKind`) and the kinds' own code.

pub mod bat;
pub mod bombhei;
pub mod capsule;
pub mod climbersblizzard;
pub mod climbersice;
pub mod crate_box;
pub mod drmariopill;
pub mod foxblaster;
pub mod foxillusion;
pub mod foxlaser;
pub mod gshell;
pub mod hammer;
pub mod hammerhead;
pub mod heart;
pub mod heiho;
pub mod kusudama;
pub mod linkbomb;
pub mod linkboomerang;
pub mod luigifireball;
pub mod mariocape;
pub mod mariofireball;
pub mod nessbat;
pub mod nesspkfire;
pub mod nesspkfirepillar;
pub mod nesspkflash;
pub mod nesspkflashexplode;
pub mod nesspkthunder;
pub mod nesspkthundertrail;
pub mod nessyoyo;
pub mod peachexplode;
pub mod peachparasol;
pub mod peachturnip;
pub mod pikachuthunder;
pub mod pikachutjoltair;
pub mod pikachutjoltground;
pub mod rshell;
pub mod samusbomb;
pub mod samuschargeshot;
pub mod samusgrapple;
pub mod samusmissile;
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
        || climbersice::run_event(r, gobj, addr)
        || climbersblizzard::run_event(r, gobj, addr)
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
        || nessbat::run_event(r, gobj, addr)
        || nesspkfire::run_event(r, gobj, addr)
        || nesspkfirepillar::run_event(r, gobj, addr)
        || nesspkflash::run_event(r, gobj, addr)
        || nesspkflashexplode::run_event(r, gobj, addr)
        || nesspkthunder::run_event(r, gobj, addr)
        || nesspkthundertrail::run_event(r, gobj, addr)
        || nessyoyo::run_event(r, gobj, addr)
        || samusbomb::run_event(r, gobj, addr)
        || samuschargeshot::run_event(r, gobj, addr)
        || samusmissile::run_event(r, gobj, addr)
        || samusgrapple::run_event(r, gobj, addr)
        || luigifireball::run_event(r, gobj, addr)
        || mariofireball::run_event(r, gobj, addr)
        || mariocape::run_event(r, gobj, addr)
        || peachexplode::run_event(r, gobj, addr)
        || peachparasol::run_event(r, gobj, addr)
        || peachturnip::run_event(r, gobj, addr)
        || linkbomb::run_event(r, gobj, addr)
        || linkboomerang::run_event(r, gobj, addr)
        || drmariopill::run_event(r, gobj, addr)
        || pikachuthunder::run_event(r, gobj, addr)
        || pikachutjoltground::run_event(r, gobj, addr)
        || pikachutjoltair::run_event(r, gobj, addr)
        || heiho::run_event(r, gobj, addr)
        || super::zako::run_event(r, gobj, addr)
}

/// Runs a kind's predicate callback (`HSD_GObjPredicate`), if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    foxlaser::run_pred(r, gobj, addr)
        .or_else(|| climbersice::run_pred(r, gobj, addr))
        .or_else(|| climbersblizzard::run_pred(r, gobj, addr))
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
        .or_else(|| nessbat::run_pred(r, gobj, addr))
        .or_else(|| nesspkfire::run_pred(r, gobj, addr))
        .or_else(|| nesspkfirepillar::run_pred(r, gobj, addr))
        .or_else(|| nesspkflash::run_pred(r, gobj, addr))
        .or_else(|| nesspkflashexplode::run_pred(r, gobj, addr))
        .or_else(|| nesspkthunder::run_pred(r, gobj, addr))
        .or_else(|| nesspkthundertrail::run_pred(r, gobj, addr))
        .or_else(|| nessyoyo::run_pred(r, gobj, addr))
        .or_else(|| samusbomb::run_pred(r, gobj, addr))
        .or_else(|| samuschargeshot::run_pred(r, gobj, addr))
        .or_else(|| samusmissile::run_pred(r, gobj, addr))
        .or_else(|| luigifireball::run_pred(r, gobj, addr))
        .or_else(|| mariofireball::run_pred(r, gobj, addr))
        .or_else(|| mariocape::run_pred(r, gobj, addr))
        .or_else(|| peachexplode::run_pred(r, gobj, addr))
        .or_else(|| peachparasol::run_pred(r, gobj, addr))
        .or_else(|| peachturnip::run_pred(r, gobj, addr))
        .or_else(|| linkbomb::run_pred(r, gobj, addr))
        .or_else(|| linkboomerang::run_pred(r, gobj, addr))
        .or_else(|| drmariopill::run_pred(r, gobj, addr))
        .or_else(|| pikachuthunder::run_pred(r, gobj, addr))
        .or_else(|| pikachutjoltground::run_pred(r, gobj, addr))
        .or_else(|| pikachutjoltair::run_pred(r, gobj, addr))
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
/// `It_Kind_StarRod_Star`, `It_Kind_LipStick_Spore` and `It_Kind_S_Scope_Beam`.
pub const STARROD_STAR: i32 = 36;
pub const LIPSTICK_SPORE: i32 = 37;
pub const S_SCOPE_BEAM: i32 = 38;
/// `It_Kind_Kuriboh`: the first enemy, and the first kind of the second group of tables.
pub const KURIBOH: i32 = 43;
/// `It_Kind_Octarock_Stone`.
pub const OCTAROCK_STONE: i32 = 47;
/// `It_Kind_Mario_Fire`: the first character item.
pub const MARIO_FIRE: i32 = 48;
/// `It_Kind_DrMario_Vitamin`.
pub const DRMARIO_VITAMIN: i32 = 49;
/// `It_Kind_Fox_Laser`.
pub const FOX_LASER: i32 = 54;
/// `It_Kind_Falco_Laser`.
pub const FALCO_LASER: i32 = 55;
/// `It_Kind_Fox_Illusion` and `It_Kind_Falco_Phantasm`.
pub const FOX_ILLUSION: i32 = 56;
pub const FALCO_PHANTASM: i32 = 57;
/// `It_Kind_IceClimber_Ice` and `It_Kind_IceClimber_Blizzard`.
pub const ICE_CLIMBER_ICE: i32 = 106;
pub const ICE_CLIMBER_BLIZZARD: i32 = 107;
/// `It_Kind_Link_Bomb`.
pub const LINK_BOMB: i32 = 58;
/// `It_Kind_CLink_Bomb`.
pub const CLINK_BOMB: i32 = 59;
/// `It_Kind_Link_Boomerang`, `It_Kind_CLink_Boomerang`, `It_Kind_Link_HShot` and
/// `It_Kind_CLink_HShot`.
pub const LINK_BOOMERANG: i32 = 60;
pub const CLINK_BOOMERANG: i32 = 61;
pub const LINK_HSHOT: i32 = 62;
pub const CLINK_HSHOT: i32 = 63;
/// `It_Kind_Zelda_DinFire` and `_DinFire_Explode`.
pub const ZELDA_DIN_FIRE: i32 = 108;
pub const ZELDA_DIN_FIRE_EXPLODE: i32 = 109;
/// Ness's: `It_Kind_Ness_PKFire`, `_PKFire_Flame`, `_PKFlush`, `_PKThunder` and its four
/// trails, `_PKFlush_Explode`, `_Bat` and `_Yoyo`.
pub const NESS_PK_FIRE: i32 = 66;
pub const NESS_PK_FIRE_FLAME: i32 = 67;
pub const NESS_PK_FLUSH: i32 = 68;
pub const NESS_PK_THUNDER: i32 = 69;
pub const NESS_PK_THUNDER1: i32 = 70;
pub const NESS_PK_THUNDER2: i32 = 71;
pub const NESS_PK_THUNDER3: i32 = 72;
pub const NESS_PK_THUNDER4: i32 = 73;
pub const NESS_PK_FLUSH_EXPLODE: i32 = 78;
pub const NESS_BAT: i32 = 101;
pub const NESS_YOYO: i32 = 102;
/// Kirby's copies of PK Flash: `It_Kind_Kirby_NessPKFlush` and `_Explode`.
pub const KIRBY_NESS_PK_FLUSH: i32 = 145;
pub const KIRBY_NESS_PK_FLUSH_EXPLODE: i32 = 146;
/// `It_Kind_Fox_Blaster`.
pub const FOX_BLASTER: i32 = 74;
/// `It_Kind_Falco_Blaster`.
pub const FALCO_BLASTER: i32 = 75;
/// `It_Kind_Pikachu_Thunder` and `It_Kind_Pichu_Thunder`.
pub const PIKACHU_THUNDER: i32 = 81;
pub const PICHU_THUNDER: i32 = 82;
/// `It_Kind_Pikachu_TJolt_Ground`, `_Air`, and Pichu's.
pub const PIKACHU_TJOLT_GROUND: i32 = 89;
pub const PIKACHU_TJOLT_AIR: i32 = 90;
pub const PICHU_TJOLT_GROUND: i32 = 91;
pub const PICHU_TJOLT_AIR: i32 = 92;
/// `It_Kind_Seak_NeedleThrow`, `_NeedleHeld`, `_Vanish`, `_Chain`.
pub const SEAK_NEEDLE_THROW: i32 = 79;
pub const SEAK_NEEDLE_HELD: i32 = 80;
pub const SEAK_VANISH: i32 = 85;
pub const SEAK_CHAIN: i32 = 97;
/// `It_Kind_Samus_Bomb`, `_Charge`, `_Missile` and `_GBeam`.
pub const SAMUS_BOMB: i32 = 93;
pub const SAMUS_CHARGE: i32 = 94;
pub const SAMUS_MISSILE: i32 = 95;
pub const SAMUS_GBEAM: i32 = 96;
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
