//! Pikachu's Skull Bash (`ftpikachuspecials.c`): a wind-up, a charge held with B (a smash
//! input starts it charged), then the launch and the flight, which ends on a hit, a wall or
//! the ground.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The flags each phase keeps going between the ground and the air.
const START_FLAGS: u32 = 0x0C4C_5284;
const HOLD_FLAGS: u32 = 0x0C4C_5286;
const LAUNCH_FLAGS: u32 = 0x0C4C_5086;
/// The flight's flags (`ftPk_MF_SpecialAirS1`).
const FLY_FLAGS: u32 = 10;

/// B held.
const PAD_B: u32 = 1 << 9;

impl Running<'_> {
    /// `ftPk_SpecialS_Enter`: the ground speed cut by `x30`, the wind-up.
    #[doc(alias = "ftPk_SpecialS_Enter")]
    pub fn pk_special_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.pk_attr_f32(f, attr::X30));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::SPECIAL_N_SET_CHARGE));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), cut));
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPk_SpecialAirS_Enter`: the air speed cut, no vertical speed, the wind-up.
    #[doc(alias = "ftPk_SpecialAirS_Enter")]
    pub fn pk_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.pk_attr_f32(f, attr::X30));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::SPECIAL_N_SET_CHARGE));
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), cut));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPk_SpecialS_ZeroVelocity` (the flight's `deal_dmg_cb`): the flight stops (falling
    /// speed kept) and ends.
    #[doc(alias = "ftPk_SpecialS_ZeroVelocity")]
    pub fn pk_special_s_zero_velocity(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = 0.0;
        if reg(fp.self_vel.y) >= 0.0 {
            fp.self_vel.y = 0.0;
        }
        self.pk_special_air_s_end_enter(gobj);
    }

    /// `ftPk_SpecialSStart_Anim`.
    #[doc(alias = "ftPk_SpecialSStart_Anim")]
    pub fn pk_special_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.pk_special_s_hold_ground_enter(gobj);
        }
    }

    /// `ftPk_SpecialAirSStart_Anim`.
    #[doc(alias = "ftPk_SpecialAirSStart_Anim")]
    pub fn pk_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.pk_special_s_hold_air_enter(gobj);
        }
    }

    /// `ftPk_SpecialSStart_Phys`: friction `specials_start_friction`.
    #[doc(alias = "ftPk_SpecialSStart_Phys")]
    pub fn pk_special_s_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.pk_attr_f32(f, attr::SPECIALS_START_FRICTION));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftPk_SpecialAirSStart_Phys`: falling at `specials_start_gravity` once the script
    /// says, and air friction `specials_start_friction`.
    #[doc(alias = "ftPk_SpecialAirSStart_Phys")]
    pub fn pk_special_air_s_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.pk_attr_f32(f, attr::SPECIALS_START_GRAVITY));
        let friction = reg(self.pk_attr_f32(f, attr::SPECIALS_START_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftPk_SpecialSStart_Coll`: off a ledge, the aerial wind-up
    ///.
    #[doc(alias = "ftPk_SpecialSStart_Coll")]
    pub fn pk_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.pk_special_s_start_ground_to_air(gobj);
        }
    }

    /// `ftPk_SpecialAirSStart_Coll`: landing, the grounded wind-up
    ///.
    #[doc(alias = "ftPk_SpecialAirSStart_Coll")]
    pub fn pk_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.pk_special_air_s_start_air_to_ground(gobj);
        }
    }

    /// The charge, with its spark callback.
    fn pk_special_s_hold_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0x200, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_SPAWN_EFFECT0));
    }

    /// The charge's frame: the effects go and the spark callback comes back when its loop
    /// ends, one more frame of charge, and the launch past `x24`.
    fn pk_special_s_hold_tick(&mut self, gobj: GObjId) -> bool {
        if !self.anim_frames_remaining(gobj) {
            self.destroy_effects(gobj);
            self.game.fighter_mut(gobj).accessory4_cb =
                Some(CodeRef(addr::SPECIAL_N_SPAWN_EFFECT0));
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.pk_attr_f32(f, attr::X24));
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::S_CHARGE).wrapping_add(1);
        fp.set_mv_i32(mv::S_CHARGE, n);
        fp::frsp(f64::from(n)) > max
    }

    /// `ftPk_SpecialSHold_Anim`.
    #[doc(alias = "ftPk_SpecialSHold_Anim")]
    pub fn pk_special_s_hold_anim(&mut self, gobj: GObjId) {
        if self.pk_special_s_hold_tick(gobj) {
            self.pk_special_s_launch_ground_enter(gobj);
        }
    }

    /// `ftPk_SpecialAirSHold_Anim`.
    #[doc(alias = "ftPk_SpecialAirSHold_Anim")]
    pub fn pk_special_air_s_hold_anim(&mut self, gobj: GObjId) {
        if self.pk_special_s_hold_tick(gobj) {
            self.pk_special_s_launch_air_enter(gobj);
        }
    }

    /// `ftPk_SpecialSHold_IASA`: B let go, the launch.
    #[doc(alias = "ftPk_SpecialSHold_IASA")]
    pub fn pk_special_s_hold_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.pk_special_s_launch_ground_enter(gobj);
        }
    }

    /// `ftPk_SpecialAirSHold_IASA`.
    #[doc(alias = "ftPk_SpecialAirSHold_IASA")]
    pub fn pk_special_air_s_hold_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.pk_special_s_launch_air_enter(gobj);
        }
    }

    /// `ftPk_SpecialSHold_Coll`.
    #[doc(alias = "ftPk_SpecialSHold_Coll")]
    pub fn pk_special_s_hold_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.pk_special_s_hold_ground_to_air(gobj);
        }
    }

    /// `ftPk_SpecialAirSHold_Coll`.
    #[doc(alias = "ftPk_SpecialAirSHold_Coll")]
    pub fn pk_special_air_s_hold_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.pk_special_air_s_hold_air_to_ground(gobj);
        }
    }

    /// The launch: the charge's effects go, with the launch's spark callback.
    fn pk_special_s_launch_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.destroy_effects(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_SPAWN_EFFECT1));
    }

    /// The launch's frame: the hitbox's damage grows with the charge (`x28` plus `x2C` per
    /// frame); when the script says, the flight.
    fn pk_special_s_launch_tick(&mut self, gobj: GObjId, grounded: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x914[0].state == hitcapsule::ENABLED {
            let charge = fp::frsp(f64::from(self.game.fighters[f.0].mv_i32(mv::S_CHARGE)));
            let slope = reg(self.pk_attr_f32(f, attr::X2C));
            let base = reg(self.pk_attr_f32(f, attr::X28));
            let dmg = crate::ppc::cvt_fp2unsigned(fp::fmadds(charge, slope, base));
            self.hitbox_damage(f, 0, false, dmg);
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            if grounded {
                self.go_airborne(f);
            }
            self.pk_special_s_fly_enter(gobj);
        }
    }

    /// `ftPk_SpecialS0_Anim`: the launch's damage, then airborne into the flight.
    #[doc(alias = "ftPk_SpecialS0_Anim")]
    pub fn pk_special_s0_anim(&mut self, gobj: GObjId) {
        self.pk_special_s_launch_tick(gobj, true);
    }

    /// `ftPk_SpecialAirS0_Anim`.
    #[doc(alias = "ftPk_SpecialAirS0_Anim")]
    pub fn pk_special_air_s0_anim(&mut self, gobj: GObjId) {
        self.pk_special_s_launch_tick(gobj, false);
    }

    /// `ftPk_SpecialS0_Coll`.
    #[doc(alias = "ftPk_SpecialS0_Coll")]
    pub fn pk_special_s0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.pk_special_s0_ground_to_air(gobj);
        }
    }

    /// `ftPk_SpecialAirS0_Coll`.
    #[doc(alias = "ftPk_SpecialAirS0_Coll")]
    pub fn pk_special_air_s0_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.pk_special_air_s0_air_to_ground(gobj);
        }
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk00`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk00")]
    pub fn pk_special_s_start_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START, START_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk01`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk01")]
    pub fn pk_special_air_s_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, START_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk02`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk02")]
    pub fn pk_special_s_hold_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_HOLD, HOLD_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk03`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk03")]
    pub fn pk_special_air_s_hold_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_HOLD, HOLD_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk04`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk04")]
    pub fn pk_special_s_hold_ground_enter(&mut self, gobj: GObjId) {
        self.pk_special_s_hold_enter(gobj, ms::SPECIAL_S_HOLD);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk05`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk05")]
    pub fn pk_special_s_hold_air_enter(&mut self, gobj: GObjId) {
        self.pk_special_s_hold_enter(gobj, ms::SPECIAL_AIR_S_HOLD);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk06`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk06")]
    pub fn pk_special_s0_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S0, LAUNCH_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk07`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk07")]
    pub fn pk_special_air_s0_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S0, LAUNCH_FLAGS);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk08`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk08")]
    pub fn pk_special_s_launch_ground_enter(&mut self, gobj: GObjId) {
        self.pk_special_s_launch_enter(gobj, ms::SPECIAL_S0);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk09`.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk09")]
    pub fn pk_special_s_launch_air_enter(&mut self, gobj: GObjId) {
        self.pk_special_s_launch_enter(gobj, ms::SPECIAL_AIR_S0);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk10`: the flight's speed, ahead at `x3C` plus `x40` per
    /// frame of charge and up by half of `x44` and more with the charge; it ends on a hit.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk10")]
    pub fn pk_special_s_fly_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let base = reg(self.pk_attr_f32(f, attr::X3C));
        let slope = reg(self.pk_attr_f32(f, attr::X40));
        let up = reg(self.pk_attr_f32(f, attr::X44));
        let max = reg(self.pk_attr_f32(f, attr::X24));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let charge = fp::frsp(f64::from(fp.mv_i32(mv::S_CHARGE)));
        fp.self_vel.x = mem(fp::fmadds(slope, charge, base));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), reg(fp.facing_dir)));
        let t = fp::fdivs(fp::fmuls(0.5, charge), max);
        fp.self_vel.y = mem(fp::fmadds(0.5, up, fp::fmuls(up, t)));
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S1, FLY_FLAGS, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x21F8 = Some(CodeRef(super::super::phys::SPEED_TOWARD_FACING));
        fp.deal_dmg_cb = Some(CodeRef(addr::SPECIAL_S_ZERO_VELOCITY));
    }

    /// `ftPk_SpecialAirS1_Anim`: the end when the flight's animation ends.
    #[doc(alias = "ftPk_SpecialAirS1_Anim")]
    pub fn pk_special_air_s1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.pk_special_air_s_end_enter(gobj);
        }
    }

    /// `ftPk_SpecialAirS1_Phys`: falling at `x48` (`x58` once the script says) up to `x4C`,
    /// and air friction `x54` once the script says.
    #[doc(alias = "ftPk_SpecialAirS1_Phys")]
    pub fn pk_special_air_s1_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g0 = reg(self.pk_attr_f32(f, attr::X48));
        let g1 = reg(self.pk_attr_f32(f, attr::X58));
        let t = reg(self.pk_attr_f32(f, attr::X4C));
        let friction = reg(self.pk_attr_f32(f, attr::X54));
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

    /// `ftPk_SpecialAirS1_Coll`: landing, the grounded end; touching any wall
    /// (`env_flags`), the aerial end.
    #[doc(alias = "ftPk_SpecialAirS1_Coll")]
    pub fn pk_special_air_s1_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.pk_special_s_end_enter(gobj);
        }
        let env = self.game.fighter(gobj).coll_data.env_flags;
        if env & 0x3F != 0 || env & 0xFC0 != 0 {
            self.pk_special_air_s_end_enter(gobj);
        }
    }

    /// `ftPk_SpecialSEnd_Anim`.
    #[doc(alias = "ftPk_SpecialSEnd_Anim")]
    pub fn pk_special_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPk_SpecialAirSEnd_Anim`.
    #[doc(alias = "ftPk_SpecialAirSEnd_Anim")]
    pub fn pk_special_air_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPk_SpecialSEnd_Phys`: friction `x54`.
    #[doc(alias = "ftPk_SpecialSEnd_Phys")]
    pub fn pk_special_s_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.pk_attr_f32(f, attr::X54));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftPk_SpecialAirSEnd_Phys`: falling at `x58`, air friction `x54`.
    #[doc(alias = "ftPk_SpecialAirSEnd_Phys")]
    pub fn pk_special_air_s_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.pk_attr_f32(f, attr::X58));
        let friction = reg(self.pk_attr_f32(f, attr::X54));
        let fp = &mut self.game.fighters[f.0];
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftPk_SpecialSEnd_Coll`: off a ledge, a fall.
    #[doc(alias = "ftPk_SpecialSEnd_Coll")]
    pub fn pk_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPk_SpecialAirSEnd_Coll`: landing, the grounded end.
    #[doc(alias = "ftPk_SpecialAirSEnd_Coll")]
    pub fn pk_special_air_s_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.pk_special_s_end_enter(gobj);
        }
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk11`: the ground speed cut by `x50`, the grounded end.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk11")]
    pub fn pk_special_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.pk_attr_f32(f, attr::X50));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), cut));
        self.change_motion_state(gobj, ms::SPECIAL_S_END, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftPk_SpecialS_ChangeMotion_Unk12`: the air speed cut by `x50`, the aerial end.
    #[doc(alias = "ftPk_SpecialS_ChangeMotion_Unk12")]
    pub fn pk_special_air_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cut = reg(self.pk_attr_f32(f, attr::X50));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), cut));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_END, 0, 0.0, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ENTER => g.pk_special_s_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK00 => g.pk_special_s_start_ground_to_air(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK01 => g.pk_special_air_s_start_air_to_ground(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK02 => g.pk_special_s_hold_ground_to_air(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK03 => g.pk_special_air_s_hold_air_to_ground(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK04 => g.pk_special_s_hold_ground_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK05 => g.pk_special_s_hold_air_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK06 => g.pk_special_s0_ground_to_air(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK07 => g.pk_special_air_s0_air_to_ground(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK08 => g.pk_special_s_launch_ground_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK09 => g.pk_special_s_launch_air_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK10 => g.pk_special_s_fly_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK11 => g.pk_special_s_end_enter(gobj),
        addr::SPECIAL_S_CHANGE_MOTION_UNK12 => g.pk_special_air_s_end_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.pk_special_air_s_enter(gobj),
        addr::SPECIAL_S_ZERO_VELOCITY => g.pk_special_s_zero_velocity(gobj),
        addr::SPECIAL_S_START_ANIM => g.pk_special_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_START_ANIM => g.pk_special_air_s_start_anim(gobj),
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
        addr::SPECIAL_S_START_PHYS => g.pk_special_s_start_phys(gobj),
        addr::SPECIAL_AIR_S_START_PHYS => g.pk_special_air_s_start_phys(gobj),
        addr::SPECIAL_S_START_COLL => g.pk_special_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_START_COLL => g.pk_special_air_s_start_coll(gobj),
        addr::SPECIAL_S_HOLD_ANIM => g.pk_special_s_hold_anim(gobj),
        addr::SPECIAL_AIR_S_HOLD_ANIM => g.pk_special_air_s_hold_anim(gobj),
        addr::SPECIAL_S_HOLD_IASA => g.pk_special_s_hold_iasa(gobj),
        addr::SPECIAL_AIR_S_HOLD_IASA => g.pk_special_air_s_hold_iasa(gobj),
        addr::SPECIAL_S_HOLD_PHYS | addr::SPECIAL_S0_PHYS => g.pk_ground_friction(gobj),
        addr::SPECIAL_AIR_S_HOLD_PHYS | addr::SPECIAL_AIR_S0_PHYS => {
            g.pk_air_physics_no_drift(gobj)
        }
        addr::SPECIAL_S_HOLD_COLL => g.pk_special_s_hold_coll(gobj),
        addr::SPECIAL_AIR_S_HOLD_COLL => g.pk_special_air_s_hold_coll(gobj),
        addr::SPECIAL_S0_ANIM => g.pk_special_s0_anim(gobj),
        addr::SPECIAL_AIR_S0_ANIM => g.pk_special_air_s0_anim(gobj),
        addr::SPECIAL_S0_COLL => g.pk_special_s0_coll(gobj),
        addr::SPECIAL_AIR_S0_COLL => g.pk_special_air_s0_coll(gobj),
        addr::SPECIAL_AIR_S1_ANIM => g.pk_special_air_s1_anim(gobj),
        addr::SPECIAL_AIR_S1_PHYS => g.pk_special_air_s1_phys(gobj),
        addr::SPECIAL_AIR_S1_COLL => g.pk_special_air_s1_coll(gobj),
        addr::SPECIAL_S_END_ANIM => g.pk_special_s_end_anim(gobj),
        addr::SPECIAL_AIR_S_END_ANIM => g.pk_special_air_s_end_anim(gobj),
        addr::SPECIAL_S_END_PHYS => g.pk_special_s_end_phys(gobj),
        addr::SPECIAL_AIR_S_END_PHYS => g.pk_special_air_s_end_phys(gobj),
        addr::SPECIAL_S_END_COLL => g.pk_special_s_end_coll(gobj),
        addr::SPECIAL_AIR_S_END_COLL => g.pk_special_air_s_end_coll(gobj),
        _ => return false,
    }
    true
}
