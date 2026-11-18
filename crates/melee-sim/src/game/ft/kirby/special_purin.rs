//! Jigglypuff's copy (`ftkirbyspecialpurin.c`): Rollout as Kirby rolls it, Jigglypuff's own
//! code ([`ft::purin::special_n`](super::super::purin::special_n)) over his states, the
//! attributes Jigglypuff's block 0x280 bytes on in his, his model's scale kept in `u` (`x8C`),
//! his own cleanups on a hit or KO, which leave the roll's look as it was, and two branches of
//! his own ([`KIRBY`]). Only the addresses of his copy's functions are his here.

use super::super::super::state::Running;
use super::super::purin::special_n::{Rollout, States};
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ROLL_HIT: u32 = 0x8010_0E0C;
    pub const NEW_ATTACK: u32 = 0x8010_0F60;
    pub const HITBOX: u32 = 0x8010_0F94;
    pub const INIT: u32 = 0x8010_131C;
    pub const ENTER: u32 = 0x8010_1560;
    pub const AIR_ENTER: u32 = 0x8010_1618;
    pub const N_START_ANIM: u32 = 0x8010_16CC;
    pub const N_LOOP_ANIM: u32 = 0x8010_17B0;
    pub const N_FULL_ANIM: u32 = 0x8010_1960;
    pub const N1_ANIM: u32 = 0x8010_1AD0;
    pub const N_TURN_ANIM: u32 = 0x8010_1EA0;
    pub const N_END_ANIM: u32 = 0x8010_2198;
    pub const AIR_N_START_ANIM: u32 = 0x8010_2470;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_254C;
    pub const AIR_N_FULL_ANIM: u32 = 0x8010_26FC;
    pub const AIR_N_ANIM: u32 = 0x8010_286C;
    pub const N_ANIM: u32 = 0x8010_2DD0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_3088;
    pub const N_HIT_ANIM: u32 = 0x8010_3374;
    pub const N_START_IASA: u32 = 0x8010_35C8;
    pub const N_LOOP_IASA: u32 = 0x8010_35CC;
    pub const N_FULL_IASA: u32 = 0x8010_3734;
    pub const N1_IASA: u32 = 0x8010_389C;
    pub const N_TURN_IASA: u32 = 0x8010_3A24;
    pub const N_END_IASA: u32 = 0x8010_3A28;
    pub const AIR_N_START_IASA: u32 = 0x8010_3A2C;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_3A30;
    pub const AIR_N_FULL_IASA: u32 = 0x8010_3B98;
    pub const AIR_N_IASA: u32 = 0x8010_3D00;
    pub const N_IASA: u32 = 0x8010_3D04;
    pub const AIR_N_END_IASA: u32 = 0x8010_3D08;
    pub const N_HIT_IASA: u32 = 0x8010_3D0C;
    pub const N_START_PHYS: u32 = 0x8010_3D10;
    pub const N_LOOP_PHYS: u32 = 0x8010_3D40;
    pub const N_FULL_PHYS: u32 = 0x8010_3D70;
    pub const N1_PHYS: u32 = 0x8010_3DA0;
    pub const N_TURN_PHYS: u32 = 0x8010_3FA8;
    pub const N_END_PHYS: u32 = 0x8010_42AC;
    pub const AIR_N_START_PHYS: u32 = 0x8010_42E8;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_4318;
    pub const AIR_N_FULL_PHYS: u32 = 0x8010_4348;
    pub const AIR_N_PHYS: u32 = 0x8010_4378;
    pub const N_PHYS: u32 = 0x8010_44F8;
    pub const AIR_N_END_PHYS: u32 = 0x8010_4610;
    pub const N_HIT_PHYS: u32 = 0x8010_4640;
    pub const N_START_COLL: u32 = 0x8010_46A4;
    pub const N_LOOP_COLL: u32 = 0x8010_475C;
    pub const N_FULL_COLL: u32 = 0x8010_4864;
    pub const N1_COLL: u32 = 0x8010_496C;
    pub const N_TURN_COLL: u32 = 0x8010_4D14;
    pub const N_END_COLL: u32 = 0x8010_4DF8;
    pub const AIR_N_START_COLL: u32 = 0x8010_4E58;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_4F10;
    pub const AIR_N_FULL_COLL: u32 = 0x8010_5018;
    pub const AIR_N_COLL: u32 = 0x8010_5120;
    pub const N_COLL: u32 = 0x8010_56FC;
    pub const AIR_N_END_COLL: u32 = 0x8010_57BC;
    pub const N_HIT_COLL: u32 = 0x8010_581C;
    pub const MIRROR: u32 = 0x8010_5978;
}

/// Kirby's Rollout: his states, Jigglypuff's attributes 0x280 bytes on (`specialn_pr_*` from
/// `ftKb_DatAttrs` x2B4, her `ftPurinAttributes` x34), and his hooks: `ftKb_PrSetCallbacks`
/// hooks his generic cleanups (`ftKb_Init_800EE74C` on death, `_800EE7B8` on damage) where
/// Jigglypuff's hooks the roll's undo (docs/melee-bugs.md).
pub const KIRBY: Rollout = Rollout {
    attrs: 0x280,
    scale: 0x8C,
    ms: States {
        start_r: ms::PR_SPECIAL_N_START_R,
        start_l: ms::PR_SPECIAL_N_START_L,
        charge: ms::PR_SPECIAL_N_LOOP,
        full: ms::PR_SPECIAL_N_FULL,
        release: ms::PR_SPECIAL_N1,
        turn: ms::PR_SPECIAL_N_TURN,
        end_r: ms::PR_SPECIAL_N_END_R,
        end_l: ms::PR_SPECIAL_N_END_L,
        air_start_r: ms::PR_SPECIAL_AIR_N_START_R,
        air_start_l: ms::PR_SPECIAL_AIR_N_START_L,
        air_charge: ms::PR_SPECIAL_AIR_N_LOOP,
        air_full: ms::PR_SPECIAL_AIR_N_FULL,
        air_release: ms::PR_SPECIAL_AIR_N,
        air_turn: ms::PR_SPECIAL_N0,
        air_end_r: ms::PR_SPECIAL_AIR_N_END_R0,
        air_end_l: ms::PR_SPECIAL_AIR_N_END_R1,
        hit: ms::PR_SPECIAL_N_HIT,
    },
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    on_hit: addr::ROLL_HIT,
    on_turned: addr::NEW_ATTACK,
    on_mirror: addr::MIRROR,
    roll_before_top: true,
    dust_from_roll: true,
};

/// The copy's callbacks; its empty interrupts (lone `blr`s) are taken and do nothing.
#[doc(alias = "ftKb_PrSpecialNStart_IASA")]
#[doc(alias = "ftKb_PrSpecialNTurn_IASA")]
#[doc(alias = "ftKb_PrSpecialNEnd_IASA")]
#[doc(alias = "ftKb_PrSpecialAirNStart_IASA")]
#[doc(alias = "ftKb_PrSpecialAirN_IASA")]
#[doc(alias = "ftKb_PrSpecialN_IASA")]
#[doc(alias = "ftKb_PrSpecialAirNEndR_IASA")]
#[doc(alias = "ftKb_PrSpecialNHit_IASA")]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let r = &KIRBY;
    match a {
        addr::ROLL_HIT => g.rollout_hit(gobj, r),
        addr::NEW_ATTACK => g.rollout_new_attack(gobj),
        addr::HITBOX => g.rollout_hitbox(gobj, r),
        addr::INIT => g.rollout_init(gobj, r),
        addr::ENTER => g.rollout_enter(gobj, r),
        addr::AIR_ENTER => g.rollout_air_enter(gobj, r),
        addr::N_START_ANIM => g.rollout_start_anim(gobj, r, false),
        addr::N_LOOP_ANIM => g.rollout_charge_anim(gobj, r, false, true),
        addr::N_FULL_ANIM => g.rollout_charge_anim(gobj, r, false, false),
        addr::N1_ANIM => g.rollout_release_anim(gobj, r, false),
        addr::N_TURN_ANIM => g.rollout_turn_anim(gobj, r),
        addr::N_END_ANIM => g.rollout_end_anim(gobj, r, false),
        addr::AIR_N_START_ANIM => g.rollout_start_anim(gobj, r, true),
        addr::AIR_N_LOOP_ANIM => g.rollout_charge_anim(gobj, r, true, true),
        addr::AIR_N_FULL_ANIM => g.rollout_charge_anim(gobj, r, true, false),
        addr::AIR_N_ANIM => g.rollout_release_anim(gobj, r, true),
        addr::N_ANIM => g.rollout_air_turn_anim(gobj, r),
        addr::AIR_N_END_ANIM => g.rollout_end_anim(gobj, r, true),
        addr::N_HIT_ANIM => g.rollout_hit_anim(gobj, r),
        addr::N_START_IASA
        | addr::N_TURN_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_IASA
        | addr::N_IASA
        | addr::AIR_N_END_IASA
        | addr::N_HIT_IASA => {}
        addr::N_LOOP_IASA | addr::N_FULL_IASA => g.rollout_charge_iasa(gobj, r, false),
        addr::N1_IASA => g.rollout_release_iasa(gobj, r),
        addr::AIR_N_LOOP_IASA | addr::AIR_N_FULL_IASA => g.rollout_charge_iasa(gobj, r, true),
        addr::N_START_PHYS | addr::N_LOOP_PHYS | addr::N_FULL_PHYS => g.rollout_start_phys(gobj),
        addr::N1_PHYS => g.rollout_release_phys(gobj, r),
        addr::N_TURN_PHYS => g.rollout_turn_phys(gobj, r),
        addr::N_END_PHYS => g.rollout_end_phys(gobj),
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_FULL_PHYS
        | addr::AIR_N_END_PHYS => g.rollout_fall(gobj, r),
        addr::AIR_N_PHYS => g.rollout_air_release_phys(gobj, r),
        addr::N_PHYS => g.rollout_air_turn_phys(gobj, r),
        addr::N_HIT_PHYS => g.rollout_hit_phys(gobj, r),
        addr::N_START_COLL => g.rollout_start_coll(gobj, r),
        addr::N_LOOP_COLL => g.rollout_charge_coll(gobj, r, false),
        addr::N_FULL_COLL => g.rollout_charge_coll(gobj, r, true),
        addr::N1_COLL => g.rollout_release_coll(gobj, r),
        addr::N_TURN_COLL => g.rollout_turn_coll(gobj, r),
        addr::N_END_COLL => g.rollout_end_coll(gobj, r),
        addr::AIR_N_START_COLL => g.rollout_air_start_coll(gobj, r),
        addr::AIR_N_LOOP_COLL => g.rollout_air_charge_coll(gobj, r, false),
        addr::AIR_N_FULL_COLL => g.rollout_air_charge_coll(gobj, r, true),
        addr::AIR_N_COLL => g.rollout_air_release_coll(gobj, r),
        addr::N_COLL => g.rollout_air_turn_coll(gobj, r),
        addr::AIR_N_END_COLL => g.rollout_air_end_coll(gobj, r),
        addr::N_HIT_COLL => g.rollout_hit_coll(gobj, r),
        addr::MIRROR => g.rollout_mirror(gobj),
        _ => return false,
    }
    true
}
