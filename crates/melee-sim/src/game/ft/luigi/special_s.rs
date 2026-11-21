//! Luigi's Green Missile (`ftluigispecials.c`) and, over the same code, Pikachu's and Pichu's
//! Skull Bash (`ftpikachuspecials.c`, [`super::super::pikachu::special_s`]): a wind-up, a
//! charge held with B (a smash input starts it charged), then the launch, which may misfire
//! (Luigi's), and the flight, which ends on a hit, a wall or the ground.
//!
//! The two moves are one copy in the original with its attributes at another place: each
//! shared function takes the user's [`Missile`] and carries both originals' names. Luigi's
//! states, addresses and [`GREEN_MISSILE`] are here, Pikachu's [`SKULL_BASH`] in his file
//! (Pichu runs Pikachu's).
//!
//! [`SKULL_BASH`]: super::super::pikachu::special_s::SKULL_BASH

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::{addr, ms, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The flags each phase keeps going between the ground and the air (both moves').
const START_FLAGS: u32 = 0x0C4C_5284;
const HOLD_FLAGS: u32 = 0x0C4C_5286;
const LAUNCH_FLAGS: u32 = 0x0C4C_5086;

/// B held.
const PAD_B: u32 = 1 << 9;

/// The move's attributes, from its first (Luigi's `greenmissile_smash` at x4 of
/// `ftLuigiAttributes`, Pikachu's x1C of `ftPikachuAttributes`): one layout in both.
mod at {
    /// The stick's tilt timer below which the input is a smash (it starts charged), and
    /// the charge a smash starts with.
    pub const SMASH: usize = 0x0;
    pub const SMASH_CHARGE: usize = 0x4;
    /// The longest charge, in frames.
    pub const MAX_CHARGE: usize = 0x8;
    /// The launch's damage, base and per frame of charge.
    pub const DAMAGE: usize = 0xC;
    pub const DAMAGE_SLOPE: usize = 0x10;
    /// The wind-up's speed is divided by this (`greenmissile_traction`).
    pub const SPEED_CUT: usize = 0x14;
    /// The wind-up's friction and its gravity in the air.
    pub const START_FRICTION: usize = 0x18;
    pub const START_GRAVITY: usize = 0x1C;
    /// The flight's speed ahead, base and per frame of charge, and up.
    pub const VEL_X: usize = 0x20;
    pub const VEL_X_SLOPE: usize = 0x24;
    pub const VEL_Y: usize = 0x28;
    /// The flight's gravity before the script's flag, and its terminal speed.
    pub const FLY_GRAVITY: usize = 0x2C;
    pub const FLY_TERMINAL: usize = 0x30;
    /// The end's speed cut, the friction from the flight's flag on, the gravity after it.
    pub const END_CUT: usize = 0x34;
    pub const FRICTION: usize = 0x38;
    pub const GRAVITY: usize = 0x3C;
    /// Luigi's misfire: its odds (one in this many) and the flight's speeds.
    pub const MISFIRE_CHANCE: usize = 0x40;
    pub const MISFIRE_VEL_X: usize = 0x44;
    pub const MISFIRE_VEL_Y: usize = 0x48;
}

/// What a user of the move has its own: where its attributes start, its states, the
/// callbacks it sets, the flight's flags, how the launch's effect comes and whether it
/// misfires.
pub struct Missile {
    /// The move's first attribute in the character's attributes ([`at`]'s offsets on).
    pub attrs: usize,
    /// The states, on the ground and in the air: the wind-up, the charge, the launch, and
    /// the end; the flight is in the air only.
    pub start: [i32; 2],
    pub hold: [i32; 2],
    pub launch: [i32; 2],
    pub fly: i32,
    pub end: [i32; 2],
    /// The flight's state-change flags (Luigi's `0x0C4C508A`, Pikachu's `ftPk_MF_SpecialAirS1`).
    pub fly_flags: u32,
    /// The wind-up's `x21EC`, which sets the charge up (`ftLg_SpecialS_SetVars`,
    /// `ftPk_SpecialN_80124DC8`, both [`Running::missile_set_vars`]).
    pub set_vars: u32,
    /// The charge's effect callback (`accessory4_cb`), set again each loop.
    pub hold_gfx: u32,
    /// The flight's hit callback (`deal_dmg_cb`, [`Running::missile_on_give_damage`]).
    pub on_give_damage: u32,
    /// The launch's effect.
    pub launch_gfx: LaunchGfx,
    /// Luigi's misfire: a draw as the charge starts, and its own states.
    pub misfire: Option<Misfire>,
}

/// How the launch's effect comes.
pub enum LaunchGfx {
    /// Luigi's: made at once on bone `bone` (`ftLuigi_SpecialS_Setup`), its effects paused in
    /// hitlag, no effect callback.
    Spawn { effect: i32, bone: i32 },
    /// Pikachu's: an effect callback that makes it (`ftPk_SpecialN_SpawnEffect1`).
    Callback(u32),
}

/// Luigi's misfire: one in `greenmissile_misfire_chance` launches (the draw as the charge
/// starts, `mv` x4) goes into these states instead, with the misfire's own flight speed and
/// no damage growing with the charge.
pub struct Misfire {
    pub states: [i32; 2],
}

/// The `mv` words: the charge (both moves'), and Luigi's misfire flag.
mod mv {
    pub const CHARGE: usize = 0;
    pub const MISFIRE: usize = 4;
}

/// Luigi's Green Missile.
pub const GREEN_MISSILE: Missile = Missile {
    attrs: 0x4,
    start: [ms::SPECIAL_S_START, ms::SPECIAL_AIR_S_START],
    hold: [ms::SPECIAL_S_HOLD, ms::SPECIAL_AIR_S_HOLD],
    launch: [ms::SPECIAL_S, ms::SPECIAL_AIR_S],
    fly: ms::SPECIAL_AIR_S2,
    end: [ms::SPECIAL_S_END, ms::SPECIAL_AIR_S_END],
    fly_flags: 0x0C4C_508A,
    set_vars: addr::SPECIAL_S_SET_VARS,
    hold_gfx: addr::SPECIAL_S_SET_GFX,
    on_give_damage: addr::SPECIAL_S_ON_GIVE_DAMAGE,
    launch_gfx: LaunchGfx::Spawn {
        effect: 0x50A,
        bone: part::HIP_N,
    },
    misfire: Some(Misfire {
        states: [ms::SPECIAL_S_MISFIRE, ms::SPECIAL_AIR_S_MISFIRE],
    }),
};

impl Running<'_> {
    /// One of the move's attributes.
    fn missile_attr(&self, f: FighterId, m: &Missile, off: usize) -> f32 {
        self.char_attr_f32(f, m.attrs + off)
    }

    /// `ftLg_SpecialS_SetGFX`: the effect flag up, and the effects paused in hitlag.
    #[doc(alias = "ftLg_SpecialS_SetGFX")]
    pub fn lg_special_s_set_gfx(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.effect_once {
            fp.effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The wind-up's `x21EC`: charged from the start by a smash input (the stick's tilt
    /// timer below the window), and for Luigi whether it misfires (`HSD_Randi`).
    #[doc(alias = "ftLg_SpecialS_SetVars")]
    #[doc(alias = "ftPk_SpecialN_80124DC8")]
    #[doc(alias("lg_special_s_set_vars", "pk_special_n_set_charge"))]
    pub fn missile_set_vars(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let smash = reg(self.missile_attr(f, m, at::SMASH));
        let charge = reg(self.missile_attr(f, m, at::SMASH_CHARGE));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let timer = fp::frsp(f64::from(i32::from(fp.active_sticky.lstick.x)));
        if timer < smash {
            fp.set_mv_i32(mv::CHARGE, fp::fctiwz(charge));
            fp.attack_flags[2] |= 0x08;
        } else {
            fp.set_mv_i32(mv::CHARGE, 0);
        }
        if m.misfire.is_some() {
            let chance = reg(self.missile_attr(f, m, at::MISFIRE_CHANCE));
            let misfire = self.game.objs.rng.below(fp::fctiwz(chance)) == 0;
            self.game.fighters[f.0].set_mv_i32(mv::MISFIRE, i32::from(misfire));
        }
    }

    /// The ground's entry: the ground speed cut, the wind-up.
    #[doc(alias = "ftLg_SpecialS_Enter")]
    #[doc(alias = "ftPk_SpecialS_Enter")]
    #[doc(alias("lg_special_s_enter", "pk_special_s_enter"))]
    pub fn missile_enter(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.missile_attr(f, m, at::SPEED_CUT));
        let fp = &mut self.game.fighters[f.0];
        fp.motion_change_cb = Some(CodeRef(m.set_vars));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), cut));
        self.change_motion_state(gobj, m.start[0], 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The air's entry: the air speed cut, no vertical speed, the wind-up.
    #[doc(alias = "ftLg_SpecialAirS_Enter")]
    #[doc(alias = "ftPk_SpecialAirS_Enter")]
    #[doc(alias("lg_special_air_s_enter", "pk_special_air_s_enter"))]
    pub fn missile_air_enter(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.missile_attr(f, m, at::SPEED_CUT));
        let fp = &mut self.game.fighters[f.0];
        fp.motion_change_cb = Some(CodeRef(m.set_vars));
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), cut));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, m.start[1], 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The flight's `deal_dmg_cb`: the flight stops (falling speed kept) and ends.
    #[doc(alias = "ftLg_SpecialS_OnGiveDamage")]
    #[doc(alias = "ftPk_SpecialS_ZeroVelocity")]
    #[doc(alias("lg_special_s_on_give_damage", "pk_special_s_zero_velocity"))]
    pub fn missile_on_give_damage(&mut self, gobj: GObjId, m: &Missile) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = 0.0;
        if reg(fp.self_vel.y) >= 0.0 {
            fp.self_vel.y = 0.0;
        }
        self.missile_air_end_enter(gobj, m);
    }

    /// The ground's wind-up's end: the charge.
    #[doc(alias = "ftLg_SpecialSStart_Anim")]
    #[doc(alias = "ftPk_SpecialSStart_Anim")]
    #[doc(alias("lg_special_s_start_anim", "pk_special_s_start_anim"))]
    pub fn missile_start_anim(&mut self, gobj: GObjId, m: &Missile) {
        if !self.anim_frames_remaining(gobj) {
            self.missile_hold_enter(gobj, m);
        }
    }

    /// The air's wind-up's end.
    #[doc(alias = "ftLg_SpecialAirSStart_Anim")]
    #[doc(alias = "ftPk_SpecialAirSStart_Anim")]
    #[doc(alias("lg_special_air_s_start_anim", "pk_special_air_s_start_anim"))]
    pub fn missile_air_start_anim(&mut self, gobj: GObjId, m: &Missile) {
        if !self.anim_frames_remaining(gobj) {
            self.missile_air_hold_enter(gobj, m);
        }
    }

    /// The wind-up's friction on the ground.
    #[doc(alias = "ftLg_SpecialSStart_Phys")]
    #[doc(alias = "ftPk_SpecialSStart_Phys")]
    #[doc(alias("lg_special_s_start_phys", "pk_special_s_start_phys"))]
    pub fn missile_start_phys(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.missile_attr(f, m, at::START_FRICTION));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// The wind-up in the air: falling once the script says, and air friction.
    #[doc(alias = "ftLg_SpecialAirSStart_Phys")]
    #[doc(alias = "ftPk_SpecialAirSStart_Phys")]
    #[doc(alias("lg_special_air_s_start_phys", "pk_special_air_s_start_phys"))]
    pub fn missile_air_start_phys(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.missile_attr(f, m, at::START_GRAVITY));
        let friction = reg(self.missile_attr(f, m, at::START_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        Self::self_accel_deaccel(fp, friction);
    }

    /// Off a ledge, the aerial wind-up.
    #[doc(alias = "ftLg_SpecialSStart_Coll")]
    #[doc(alias = "ftPk_SpecialSStart_Coll")]
    #[doc(alias("lg_special_s_start_coll", "pk_special_s_start_coll"))]
    pub fn missile_start_coll(&mut self, gobj: GObjId, m: &Missile) {
        if !self.ground_coll_off_ledges(gobj) {
            self.missile_start_ground_to_air(gobj, m);
        }
    }

    /// The wind-up into the air.
    #[doc(alias = "ftLg_SpecialSStart_GroundToAir")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk00")]
    #[doc(alias("lg_special_s_start_ground_to_air", "pk_special_s_start_ground_to_air"))]
    pub fn missile_start_ground_to_air(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_ground_to_air(gobj, m.start[1], START_FLAGS);
    }

    /// Landing, the grounded wind-up.
    #[doc(alias = "ftLg_SpecialAirSStart_Coll")]
    #[doc(alias = "ftPk_SpecialAirSStart_Coll")]
    #[doc(alias("lg_special_air_s_start_coll", "pk_special_air_s_start_coll"))]
    pub fn missile_air_start_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.missile_air_start_air_to_ground(gobj, m);
        }
    }

    /// The wind-up onto the ground.
    #[doc(alias = "ftLg_SpecialAirSStart_AirToGround")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk01")]
    #[doc(alias(
        "lg_special_air_s_start_air_to_ground",
        "pk_special_air_s_start_air_to_ground"
    ))]
    pub fn missile_air_start_air_to_ground(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_air_to_ground(gobj, m.start[0], START_FLAGS);
    }

    /// The charge's frame: the effects go and the effect callback comes back when its loop
    /// ends, one more frame of charge, and the launch past the longest charge.
    fn missile_hold_tick(&mut self, gobj: GObjId, m: &Missile) -> bool {
        if !self.anim_frames_remaining(gobj) {
            self.destroy_effects(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(m.hold_gfx));
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.missile_attr(f, m, at::MAX_CHARGE));
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::CHARGE).wrapping_add(1);
        fp.set_mv_i32(mv::CHARGE, n);
        fp::frsp(f64::from(n)) > max
    }

    /// The charge on the ground: past the longest, the launch.
    #[doc(alias = "ftLg_SpecialSHold_Anim")]
    #[doc(alias = "ftPk_SpecialSHold_Anim")]
    #[doc(alias("lg_special_s_hold_anim", "pk_special_s_hold_anim"))]
    pub fn missile_hold_anim(&mut self, gobj: GObjId, m: &Missile) {
        if self.missile_hold_tick(gobj, m) {
            self.missile_launch_enter(gobj, m);
        }
    }

    /// The charge in the air.
    #[doc(alias = "ftLg_SpecialAirSHold_Anim")]
    #[doc(alias = "ftPk_SpecialAirSHold_Anim")]
    #[doc(alias("lg_special_air_s_hold_anim", "pk_special_air_s_hold_anim"))]
    pub fn missile_air_hold_anim(&mut self, gobj: GObjId, m: &Missile) {
        if self.missile_hold_tick(gobj, m) {
            self.missile_air_launch_enter(gobj, m);
        }
    }

    /// B let go, the launch.
    #[doc(alias = "ftLg_SpecialSHold_IASA")]
    #[doc(alias = "ftPk_SpecialSHold_IASA")]
    #[doc(alias("lg_special_s_hold_iasa", "pk_special_s_hold_iasa"))]
    pub fn missile_hold_iasa(&mut self, gobj: GObjId, m: &Missile) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.missile_launch_enter(gobj, m);
        }
    }

    /// B let go in the air.
    #[doc(alias = "ftLg_SpecialAirSHold_IASA")]
    #[doc(alias = "ftPk_SpecialAirSHold_IASA")]
    #[doc(alias("lg_special_air_s_hold_iasa", "pk_special_air_s_hold_iasa"))]
    pub fn missile_air_hold_iasa(&mut self, gobj: GObjId, m: &Missile) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.missile_air_launch_enter(gobj, m);
        }
    }

    /// Off a ledge, the aerial charge.
    #[doc(alias = "ftLg_SpecialSHold_Coll")]
    #[doc(alias = "ftPk_SpecialSHold_Coll")]
    #[doc(alias("lg_special_s_hold_coll", "pk_special_s_hold_coll"))]
    pub fn missile_hold_coll(&mut self, gobj: GObjId, m: &Missile) {
        if !self.ground_coll_off_ledges(gobj) {
            self.missile_hold_ground_to_air(gobj, m);
        }
    }

    /// The charge into the air.
    #[doc(alias = "ftLg_SpecialSHold_GroundToAir")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk02")]
    #[doc(alias("lg_special_s_hold_ground_to_air", "pk_special_s_hold_ground_to_air"))]
    pub fn missile_hold_ground_to_air(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_ground_to_air(gobj, m.hold[1], HOLD_FLAGS);
    }

    /// Landing, the grounded charge.
    #[doc(alias = "ftLg_SpecialAirSHold_Coll")]
    #[doc(alias = "ftPk_SpecialAirSHold_Coll")]
    #[doc(alias("lg_special_air_s_hold_coll", "pk_special_air_s_hold_coll"))]
    pub fn missile_air_hold_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.missile_air_hold_air_to_ground(gobj, m);
        }
    }

    /// The charge onto the ground.
    #[doc(alias = "ftLg_SpecialAirSHold_AirToGround")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk03")]
    #[doc(alias(
        "lg_special_air_s_hold_air_to_ground",
        "pk_special_air_s_hold_air_to_ground"
    ))]
    pub fn missile_air_hold_air_to_ground(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_air_to_ground(gobj, m.hold[0], HOLD_FLAGS);
    }

    /// The charge on the ground, with its effect callback.
    #[doc(alias = "ftLg_SpecialSHold_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk04")]
    #[doc(alias("lg_special_s_hold_enter", "pk_special_s_hold_ground_enter"))]
    pub fn missile_hold_enter(&mut self, gobj: GObjId, m: &Missile) {
        self.change_motion_state(gobj, m.hold[0], 0x200, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(m.hold_gfx));
    }

    /// The charge in the air.
    #[doc(alias = "ftLg_SpecialAirSHold_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk05")]
    #[doc(alias("lg_special_air_s_hold_enter", "pk_special_s_hold_air_enter"))]
    pub fn missile_air_hold_enter(&mut self, gobj: GObjId, m: &Missile) {
        self.change_motion_state(gobj, m.hold[1], 0x200, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(m.hold_gfx));
    }

    /// Whether the launch is Luigi's misfire.
    fn missile_misfires(&self, gobj: GObjId, m: &Missile) -> bool {
        m.misfire.is_some() && self.game.fighter(gobj).mv_i32(mv::MISFIRE) != 0
    }

    /// The launch's (and the misfire's) frame: unless it misfires, the hitbox's damage grows
    /// with the charge; when the script says, the flight (airborne first from the ground).
    fn missile_launch_tick(&mut self, gobj: GObjId, m: &Missile, grounded: bool) {
        let f = self.game.fighter_id(gobj);
        if !self.missile_misfires(gobj, m)
            && self.game.fighters[f.0].hitboxes[0].state == hitcapsule::ENABLED
        {
            let charge = fp::frsp(f64::from(self.game.fighters[f.0].mv_i32(mv::CHARGE)));
            let slope = reg(self.missile_attr(f, m, at::DAMAGE_SLOPE));
            let base = reg(self.missile_attr(f, m, at::DAMAGE));
            let dmg = crate::ppc::cvt_fp2unsigned(fp::fmadds(charge, slope, base));
            self.hitbox_damage(f, 0, false, dmg);
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            if grounded {
                self.go_airborne(f);
            }
            self.missile_fly_enter(gobj, m);
        }
    }

    /// The launch on the ground (Luigi's misfire's too): its damage, then airborne into the
    /// flight.
    #[doc(alias = "ftLg_SpecialS_Anim")]
    #[doc(alias = "ftLg_SpecialSMisfire_Anim")]
    #[doc(alias = "ftPk_SpecialS0_Anim")]
    #[doc(alias("lg_special_s_anim", "lg_special_s_misfire_anim", "pk_special_s0_anim"))]
    pub fn missile_launch_anim(&mut self, gobj: GObjId, m: &Missile) {
        self.missile_launch_tick(gobj, m, true);
    }

    /// The launch in the air (Luigi's misfire's too).
    #[doc(alias = "ftLg_SpecialAirS_Anim")]
    #[doc(alias = "ftLg_SpecialAirSMisfire_Anim")]
    #[doc(alias = "ftPk_SpecialAirS0_Anim")]
    #[doc(alias(
        "lg_special_air_s_anim",
        "lg_special_air_s_misfire_anim",
        "pk_special_air_s0_anim"
    ))]
    pub fn missile_air_launch_anim(&mut self, gobj: GObjId, m: &Missile) {
        self.missile_launch_tick(gobj, m, false);
    }

    /// Off a ledge, the aerial launch.
    #[doc(alias = "ftLg_SpecialS_Coll")]
    #[doc(alias = "ftPk_SpecialS0_Coll")]
    #[doc(alias("lg_special_s_coll", "pk_special_s0_coll"))]
    pub fn missile_launch_coll(&mut self, gobj: GObjId, m: &Missile) {
        if !self.ground_coll_off_ledges(gobj) {
            self.missile_launch_ground_to_air(gobj, m);
        }
    }

    /// The launch into the air.
    #[doc(alias = "ftLg_SpecialSLaunch_GroundToAir")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk06")]
    #[doc(alias("lg_special_s_launch_ground_to_air", "pk_special_s0_ground_to_air"))]
    pub fn missile_launch_ground_to_air(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_ground_to_air(gobj, m.launch[1], LAUNCH_FLAGS);
    }

    /// Landing, the grounded launch.
    #[doc(alias = "ftLg_SpecialAirS_Coll")]
    #[doc(alias = "ftPk_SpecialAirS0_Coll")]
    #[doc(alias("lg_special_air_s_coll", "pk_special_air_s0_coll"))]
    pub fn missile_air_launch_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.missile_air_launch_air_to_ground(gobj, m);
        }
    }

    /// The launch onto the ground.
    #[doc(alias = "ftLg_SpecialAirSLaunch_AirToGround")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk07")]
    #[doc(alias(
        "lg_special_air_s_launch_air_to_ground",
        "pk_special_air_s0_air_to_ground"
    ))]
    pub fn missile_air_launch_air_to_ground(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_air_to_ground(gobj, m.launch[0], LAUNCH_FLAGS);
    }

    /// The launch's or misfire's start in state `msid`: the charge's effects go, and the
    /// launch's effect comes (`ftLuigi_SpecialS_RemoveGFX`, `_Setup`; Pikachu's callback).
    fn missile_launch_start(&mut self, gobj: GObjId, m: &Missile, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.destroy_effects(gobj);
        match m.launch_gfx {
            LaunchGfx::Spawn { effect, bone } => {
                if !self.game.fighter(gobj).effect_once {
                    let j = self.bone_joint(gobj, bone);
                    spawn(self, effect, Some(gobj), &[Arg::JObj(j)]);
                    self.game.fighter_mut(gobj).effect_once = true;
                }
                self.set_effect_hitlag_cbs(gobj);
                self.game.fighter_mut(gobj).accessory4_cb = None;
            }
            LaunchGfx::Callback(cb) => {
                self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(cb));
            }
        }
    }

    /// The launch on the ground, Luigi's misfire if it's one.
    #[doc(alias = "ftLg_SpecialSLaunch_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk08")]
    #[doc(alias("lg_special_s_launch_enter", "pk_special_s_launch_ground_enter"))]
    pub fn missile_launch_enter(&mut self, gobj: GObjId, m: &Missile) {
        if self.missile_misfires(gobj, m) {
            self.missile_misfire_enter(gobj, m);
            return;
        }
        self.missile_launch_start(gobj, m, m.launch[0]);
    }

    /// The launch in the air, Luigi's misfire if it's one.
    #[doc(alias = "ftLg_SpecialAirSLaunch_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk09")]
    #[doc(alias("lg_special_air_s_launch_enter", "pk_special_s_launch_air_enter"))]
    pub fn missile_air_launch_enter(&mut self, gobj: GObjId, m: &Missile) {
        if self.missile_misfires(gobj, m) {
            self.missile_air_misfire_enter(gobj, m);
            return;
        }
        self.missile_launch_start(gobj, m, m.launch[1]);
    }

    /// Luigi's misfire's states.
    fn misfire_states(m: &Missile) -> [i32; 2] {
        m.misfire.as_ref().expect("a move without a misfire").states
    }

    /// `ftLg_SpecialSMisfire_Coll`: off a ledge, the aerial misfire.
    #[doc(alias = "ftLg_SpecialSMisfire_Coll")]
    #[doc(alias("lg_special_s_misfire_coll"))]
    pub fn missile_misfire_coll(&mut self, gobj: GObjId, m: &Missile) {
        if !self.ground_coll_off_ledges(gobj) {
            self.missile_misfire_ground_to_air(gobj, m);
        }
    }

    /// `ftLg_SpecialSMisfire_GroundToAir`.
    #[doc(alias = "ftLg_SpecialSMisfire_GroundToAir")]
    #[doc(alias("lg_special_s_misfire_ground_to_air"))]
    pub fn missile_misfire_ground_to_air(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_ground_to_air(gobj, Self::misfire_states(m)[1], LAUNCH_FLAGS);
    }

    /// `ftLg_SpecialAirSMisfire_Coll`: landing, the grounded misfire.
    #[doc(alias = "ftLg_SpecialAirSMisfire_Coll")]
    #[doc(alias("lg_special_air_s_misfire_coll"))]
    pub fn missile_air_misfire_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.missile_air_misfire_air_to_ground(gobj, m);
        }
    }

    /// `ftLg_SpecialAirSMisfire_AirToGround`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_AirToGround")]
    #[doc(alias("lg_special_air_s_misfire_air_to_ground"))]
    pub fn missile_air_misfire_air_to_ground(&mut self, gobj: GObjId, m: &Missile) {
        self.fox_air_to_ground(gobj, Self::misfire_states(m)[0], LAUNCH_FLAGS);
    }

    /// `ftLg_SpecialSMisfire_Enter`.
    #[doc(alias = "ftLg_SpecialSMisfire_Enter")]
    #[doc(alias("lg_special_s_misfire_enter"))]
    pub fn missile_misfire_enter(&mut self, gobj: GObjId, m: &Missile) {
        self.missile_launch_start(gobj, m, Self::misfire_states(m)[0]);
    }

    /// `ftLg_SpecialAirSMisfire_Enter`.
    #[doc(alias = "ftLg_SpecialAirSMisfire_Enter")]
    #[doc(alias("lg_special_air_s_misfire_enter"))]
    pub fn missile_air_misfire_enter(&mut self, gobj: GObjId, m: &Missile) {
        self.missile_launch_start(gobj, m, Self::misfire_states(m)[1]);
    }

    /// The flight's end when its animation ends.
    #[doc(alias = "ftLg_SpecialAirS2_Anim")]
    #[doc(alias = "ftPk_SpecialAirS1_Anim")]
    #[doc(alias("lg_special_air_s2_anim", "pk_special_air_s1_anim"))]
    pub fn missile_fly_anim(&mut self, gobj: GObjId, m: &Missile) {
        if !self.anim_frames_remaining(gobj) {
            self.missile_air_end_enter(gobj, m);
        }
    }

    /// The flight: falling at its gravity up to its terminal speed (at the end's gravity
    /// once the script says, which also starts the friction).
    #[doc(alias = "ftLg_SpecialAirS2_Phys")]
    #[doc(alias = "ftPk_SpecialAirS1_Phys")]
    #[doc(alias("lg_special_air_s2_phys", "pk_special_air_s1_phys"))]
    pub fn missile_fly_phys(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let t = reg(self.missile_attr(f, m, at::FLY_TERMINAL));
        let g1 = reg(self.missile_attr(f, m, at::GRAVITY));
        let g0 = reg(self.missile_attr(f, m, at::FLY_GRAVITY));
        let friction = reg(self.missile_attr(f, m, at::FRICTION));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            Self::fall(fp, g1, t);
        } else {
            Self::fall(fp, g0, t);
        }
        if fp.cmd_vars[0] != 0 {
            Self::self_accel_deaccel(fp, friction);
        }
    }

    /// Landing, the grounded end; touching any wall or ceiling (`env_flags`), the aerial
    /// end.
    #[doc(alias = "ftLg_SpecialAirS2_Coll")]
    #[doc(alias = "ftPk_SpecialAirS1_Coll")]
    #[doc(alias("lg_special_air_s2_coll", "pk_special_air_s1_coll"))]
    pub fn missile_fly_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.missile_end_enter(gobj, m);
        }
        let env = self.game.fighter(gobj).coll_data.env_flags;
        if env & 0x3F != 0 || env & 0xFC0 != 0 {
            self.missile_air_end_enter(gobj, m);
        }
    }

    /// The flight's speed, ahead at its base plus its slope per frame of charge and up by
    /// half its upward speed and more with the charge, or Luigi's misfire's own speeds; it
    /// ends on a hit.
    #[doc(alias = "ftLg_SpecialSFly_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk10")]
    #[doc(alias("lg_special_s_fly_enter", "pk_special_s_fly_enter"))]
    pub fn missile_fly_enter(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let misfire = self.missile_misfires(gobj, m);
        let (misfire_x, misfire_y) = if misfire {
            (
                self.missile_attr(f, m, at::MISFIRE_VEL_X),
                self.missile_attr(f, m, at::MISFIRE_VEL_Y),
            )
        } else {
            (0.0, 0.0)
        };
        let slope = reg(self.missile_attr(f, m, at::VEL_X_SLOPE));
        let base = reg(self.missile_attr(f, m, at::VEL_X));
        let up = reg(self.missile_attr(f, m, at::VEL_Y));
        let max = reg(self.missile_attr(f, m, at::MAX_CHARGE));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let charge = fp::frsp(f64::from(fp.mv_i32(mv::CHARGE)));
        fp.self_vel.x = if misfire {
            misfire_x
        } else {
            mem(fp::fmadds(slope, charge, base))
        };
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), reg(fp.facing_dir)));
        fp.self_vel.y = if misfire {
            misfire_y
        } else {
            let t = fp::fdivs(fp::fmuls(0.5, charge), max);
            mem(fp::fmadds(0.5, up, fp::fmuls(up, t)))
        };
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, m.fly, m.fly_flags, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.turn_cb = Some(CodeRef(super::super::phys::SPEED_TOWARD_FACING));
        fp.deal_dmg_cb = Some(CodeRef(m.on_give_damage));
    }

    /// The end on the ground: a wait when its animation ends.
    #[doc(alias = "ftLg_SpecialSEnd_Anim")]
    #[doc(alias = "ftPk_SpecialSEnd_Anim")]
    #[doc(alias("lg_special_s_end_anim", "pk_special_s_end_anim"))]
    pub fn missile_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The end in the air: a fall when its animation ends.
    #[doc(alias = "ftLg_SpecialAirSEnd_Anim")]
    #[doc(alias = "ftPk_SpecialAirSEnd_Anim")]
    #[doc(alias("lg_special_air_s_end_anim", "pk_special_air_s_end_anim"))]
    pub fn missile_air_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The end's friction on the ground.
    #[doc(alias = "ftLg_SpecialSEnd_Phys")]
    #[doc(alias = "ftPk_SpecialSEnd_Phys")]
    #[doc(alias("lg_special_s_end_phys", "pk_special_s_end_phys"))]
    pub fn missile_end_phys(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.missile_attr(f, m, at::FRICTION));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// The end in the air: falling at the end's gravity, and air friction.
    #[doc(alias = "ftLg_SpecialAirSEnd_Phys")]
    #[doc(alias = "ftPk_SpecialAirSEnd_Phys")]
    #[doc(alias("lg_special_air_s_end_phys", "pk_special_air_s_end_phys"))]
    pub fn missile_air_end_phys(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.missile_attr(f, m, at::GRAVITY));
        let friction = reg(self.missile_attr(f, m, at::FRICTION));
        let fp = &mut self.game.fighters[f.0];
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// Off a ledge, a fall.
    #[doc(alias = "ftLg_SpecialSEnd_Coll")]
    #[doc(alias = "ftPk_SpecialSEnd_Coll")]
    #[doc(alias("lg_special_s_end_coll", "pk_special_s_end_coll"))]
    pub fn missile_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// Landing, the grounded end.
    #[doc(alias = "ftLg_SpecialAirSEnd_Coll")]
    #[doc(alias = "ftPk_SpecialAirSEnd_Coll")]
    #[doc(alias("lg_special_air_s_end_coll", "pk_special_air_s_end_coll"))]
    pub fn missile_air_end_coll(&mut self, gobj: GObjId, m: &Missile) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.missile_end_enter(gobj, m);
        }
    }

    /// The end on the ground: the ground speed cut.
    #[doc(alias = "ftLg_SpecialSEnd_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk11")]
    #[doc(alias("lg_special_s_end_enter", "pk_special_s_end_enter"))]
    pub fn missile_end_enter(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.missile_attr(f, m, at::END_CUT));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), cut));
        self.change_motion_state(gobj, m.end[0], 0, 0.0, 1.0, 0.0, None);
    }

    /// The end in the air: the air speed cut.
    #[doc(alias = "ftLg_SpecialAirSEnd_Enter")]
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk12")]
    #[doc(alias("lg_special_air_s_end_enter", "pk_special_air_s_end_enter"))]
    pub fn missile_air_end_enter(&mut self, gobj: GObjId, m: &Missile) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.missile_attr(f, m, at::END_CUT));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), cut));
        self.change_motion_state(gobj, m.end[1], 0, 0.0, 1.0, 0.0, None);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let m = &GREEN_MISSILE;
    match a {
        addr::SPECIAL_S_SET_GFX => g.lg_special_s_set_gfx(gobj),
        addr::SPECIAL_S_START_GROUND_TO_AIR => g.missile_start_ground_to_air(gobj, m),
        addr::SPECIAL_AIR_S_START_AIR_TO_GROUND => g.missile_air_start_air_to_ground(gobj, m),
        addr::SPECIAL_S_HOLD_GROUND_TO_AIR => g.missile_hold_ground_to_air(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_AIR_TO_GROUND => g.missile_air_hold_air_to_ground(gobj, m),
        addr::SPECIAL_S_HOLD_ENTER => g.missile_hold_enter(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_ENTER => g.missile_air_hold_enter(gobj, m),
        addr::SPECIAL_S_LAUNCH_GROUND_TO_AIR => g.missile_launch_ground_to_air(gobj, m),
        addr::SPECIAL_AIR_S_LAUNCH_AIR_TO_GROUND => g.missile_air_launch_air_to_ground(gobj, m),
        addr::SPECIAL_S_LAUNCH_ENTER => g.missile_launch_enter(gobj, m),
        addr::SPECIAL_AIR_S_LAUNCH_ENTER => g.missile_air_launch_enter(gobj, m),
        addr::SPECIAL_S_MISFIRE_GROUND_TO_AIR => g.missile_misfire_ground_to_air(gobj, m),
        addr::SPECIAL_AIR_S_MISFIRE_AIR_TO_GROUND => g.missile_air_misfire_air_to_ground(gobj, m),
        addr::SPECIAL_S_MISFIRE_ENTER => g.missile_misfire_enter(gobj, m),
        addr::SPECIAL_AIR_S_MISFIRE_ENTER => g.missile_air_misfire_enter(gobj, m),
        addr::SPECIAL_S_FLY_ENTER => g.missile_fly_enter(gobj, m),
        addr::SPECIAL_S_END_ENTER => g.missile_end_enter(gobj, m),
        addr::SPECIAL_AIR_S_END_ENTER => g.missile_air_end_enter(gobj, m),
        addr::SPECIAL_S_SET_VARS => g.missile_set_vars(gobj, m),
        addr::SPECIAL_S_ENTER => g.missile_enter(gobj, m),
        addr::SPECIAL_AIR_S_ENTER => g.missile_air_enter(gobj, m),
        addr::SPECIAL_S_ON_GIVE_DAMAGE => g.missile_on_give_damage(gobj, m),
        addr::SPECIAL_S_START_ANIM => g.missile_start_anim(gobj, m),
        addr::SPECIAL_AIR_S_START_ANIM => g.missile_air_start_anim(gobj, m),
        addr::SPECIAL_S_START_IASA
        | addr::SPECIAL_AIR_S_START_IASA
        | addr::SPECIAL_S_IASA
        | addr::SPECIAL_AIR_S_IASA
        | addr::SPECIAL_S_MISFIRE_IASA
        | addr::SPECIAL_AIR_S_MISFIRE_IASA
        | addr::SPECIAL_S2_ANIM
        | addr::SPECIAL_S2_IASA
        | addr::SPECIAL_AIR_S2_IASA
        | addr::SPECIAL_S2_PHYS
        | addr::SPECIAL_S2_COLL
        | addr::SPECIAL_S_END_IASA
        | addr::SPECIAL_AIR_S_END_IASA => g.lg_nothing(gobj),
        addr::SPECIAL_S_START_PHYS => g.missile_start_phys(gobj, m),
        addr::SPECIAL_AIR_S_START_PHYS => g.missile_air_start_phys(gobj, m),
        addr::SPECIAL_S_START_COLL => g.missile_start_coll(gobj, m),
        addr::SPECIAL_AIR_S_START_COLL => g.missile_air_start_coll(gobj, m),
        addr::SPECIAL_S_HOLD_ANIM => g.missile_hold_anim(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_ANIM => g.missile_air_hold_anim(gobj, m),
        addr::SPECIAL_S_HOLD_IASA => g.missile_hold_iasa(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_IASA => g.missile_air_hold_iasa(gobj, m),
        addr::SPECIAL_S_HOLD_PHYS | addr::SPECIAL_S_PHYS => g.lg_ground_friction(gobj),
        addr::SPECIAL_AIR_S_HOLD_PHYS | addr::SPECIAL_AIR_S_PHYS => g.lg_air_physics_no_drift(gobj),
        addr::SPECIAL_S_HOLD_COLL => g.missile_hold_coll(gobj, m),
        addr::SPECIAL_AIR_S_HOLD_COLL => g.missile_air_hold_coll(gobj, m),
        addr::SPECIAL_S_ANIM | addr::SPECIAL_S_MISFIRE_ANIM => g.missile_launch_anim(gobj, m),
        addr::SPECIAL_AIR_S_ANIM | addr::SPECIAL_AIR_S_MISFIRE_ANIM => {
            g.missile_air_launch_anim(gobj, m)
        }
        addr::SPECIAL_S_COLL => g.missile_launch_coll(gobj, m),
        addr::SPECIAL_AIR_S_COLL => g.missile_air_launch_coll(gobj, m),
        addr::SPECIAL_S_MISFIRE_PHYS | addr::SPECIAL_AIR_S_MISFIRE_PHYS => {
            g.lg_ground_friction(gobj)
        }
        addr::SPECIAL_S_MISFIRE_COLL => g.missile_misfire_coll(gobj, m),
        addr::SPECIAL_AIR_S_MISFIRE_COLL => g.missile_air_misfire_coll(gobj, m),
        addr::SPECIAL_AIR_S2_ANIM => g.missile_fly_anim(gobj, m),
        addr::SPECIAL_AIR_S2_PHYS => g.missile_fly_phys(gobj, m),
        addr::SPECIAL_AIR_S2_COLL => g.missile_fly_coll(gobj, m),
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
