//! Pikachu's (and Pichu's) Skull Bash (`ftpikachuspecials.c`): Luigi's Green Missile's code
//! ([`super::super::luigi::special_s`]) over Pikachu's [`SKULL_BASH`], without the misfire.
//! Pikachu's addresses, his descriptor and his callback table are here.

use super::super::super::state::Running;
use super::super::luigi::special_s::{LaunchGfx, Missile};
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

/// Pikachu's Skull Bash: its attributes from `ftPikachuAttributes`' x1C, its own states, the
/// flight's flags `ftPk_MF_SpecialAirS1`, the charge's and launch's sparks as effect
/// callbacks, no misfire.
pub const SKULL_BASH: Missile = Missile {
    attrs: 0x1C,
    start: [ms::SPECIAL_S_START, ms::SPECIAL_AIR_S_START],
    hold: [ms::SPECIAL_S_HOLD, ms::SPECIAL_AIR_S_HOLD],
    launch: [ms::SPECIAL_S0, ms::SPECIAL_AIR_S0],
    fly: ms::SPECIAL_AIR_S1,
    end: [ms::SPECIAL_S_END, ms::SPECIAL_AIR_S_END],
    fly_flags: 10,
    set_vars: addr::SPECIAL_N_SET_CHARGE,
    hold_gfx: addr::SPECIAL_N_SPAWN_EFFECT0,
    on_give_damage: addr::SPECIAL_S_ZERO_VELOCITY,
    launch_gfx: LaunchGfx::Callback(addr::SPECIAL_N_SPAWN_EFFECT1),
    misfire: None,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let m = &SKULL_BASH;
    match a {
        addr::SPECIAL_S_ENTER => g.missile_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK00 => g.missile_start_ground_to_air(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK01 => g.missile_air_start_air_to_ground(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK02 => g.missile_hold_ground_to_air(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK03 => g.missile_air_hold_air_to_ground(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK04 => g.missile_hold_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK05 => g.missile_air_hold_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK06 => g.missile_launch_ground_to_air(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK07 => g.missile_air_launch_air_to_ground(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK08 => g.missile_launch_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK09 => g.missile_air_launch_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK10 => g.missile_fly_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK11 => g.missile_end_enter(gobj, m),
        addr::SPECIAL_S_CHANGE_MOTION_UNK12 => g.missile_air_end_enter(gobj, m),
        addr::SPECIAL_AIR_S_ENTER => g.missile_air_enter(gobj, m),
        addr::SPECIAL_S_ZERO_VELOCITY => g.missile_on_give_damage(gobj, m),
        addr::SPECIAL_S_START_ANIM => g.missile_start_anim(gobj, m),
        addr::SPECIAL_AIR_S_START_ANIM => g.missile_air_start_anim(gobj, m),
        addr::SPECIAL_S_START_IASA
        | addr::SPECIAL_AIR_S_START_IASA
        | addr::SPECIAL_S0_IASA
        | addr::SPECIAL_AIR_S0_IASA
        | addr::SPECIAL_S1_ANIM
        | addr::SPECIAL_S1_IASA
        | addr::SPECIAL_AIR_S1_IASA
        | addr::SPECIAL_S1_PHYS
        | addr::SPECIAL_S1_COLL
        | addr::SPECIAL_S_END_IASA
        | addr::SPECIAL_AIR_S_END_IASA => g.pk_nothing(gobj),
        addr::SPECIAL_S_START_PHYS => g.missile_start_phys(gobj, m),
        addr::SPECIAL_AIR_S_START_PHYS => g.missile_air_start_phys(gobj, m),
        addr::SPECIAL_S_START_COLL => g.missile_start_coll(gobj, m),
        addr::SPECIAL_AIR_S_START_COLL => g.missile_air_start_coll(gobj, m),
        addr::SPECIAL_S_HOLD_ANIM => g.missile_hold_anim(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_ANIM => g.missile_air_hold_anim(gobj, m),
        addr::SPECIAL_S_HOLD_IASA => g.missile_hold_iasa(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_IASA => g.missile_air_hold_iasa(gobj, m),
        addr::SPECIAL_S_HOLD_PHYS | addr::SPECIAL_S0_PHYS => g.pk_ground_friction(gobj),
        addr::SPECIAL_AIR_S_HOLD_PHYS | addr::SPECIAL_AIR_S0_PHYS => {
            g.pk_air_physics_no_drift(gobj)
        }
        addr::SPECIAL_S_HOLD_COLL => g.missile_hold_coll(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_COLL => g.missile_air_hold_coll(gobj, m),
        addr::SPECIAL_S0_ANIM => g.missile_launch_anim(gobj, m),
        addr::SPECIAL_AIR_S0_ANIM => g.missile_air_launch_anim(gobj, m),
        addr::SPECIAL_S0_COLL => g.missile_launch_coll(gobj, m),
        addr::SPECIAL_AIR_S0_COLL => g.missile_air_launch_coll(gobj, m),
        addr::SPECIAL_AIR_S1_ANIM => g.missile_fly_anim(gobj, m),
        addr::SPECIAL_AIR_S1_PHYS => g.missile_fly_phys(gobj, m),
        addr::SPECIAL_AIR_S1_COLL => g.missile_fly_coll(gobj, m),
        addr::SPECIAL_S_END_ANIM => g.missile_end_anim(gobj),
        addr::SPECIAL_AIR_S_END_ANIM => g.missile_air_end_anim(gobj),
        addr::SPECIAL_S_END_PHYS => g.missile_end_phys(gobj, m),
        addr::SPECIAL_AIR_S_END_PHYS => g.missile_air_end_phys(gobj, m),
        addr::SPECIAL_S_END_COLL => g.missile_end_coll(gobj),
        addr::SPECIAL_AIR_S_END_COLL => g.missile_air_end_coll(gobj, m),
        _ => return false,
    }
    true
}
