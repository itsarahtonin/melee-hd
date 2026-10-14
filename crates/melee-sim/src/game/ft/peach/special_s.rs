//! Peach's side special, the Peach Bomber (`ftpeachspecials.c`): a hop forward (a smash's
//! farther), a flying hip attack, and an explosion at her hips when it hits or meets a wall,
//! which bounces her back.

use gekko_fp as fp;

use super::super::super::it::kinds::peachexplode;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::super::parts::HIP_N;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;

use super::super::phys::{
    SPEED_AWAY_FROM_FACING as FACE_BACK, SPEED_TOWARD_FACING as FACE_FORWARD,
};

/// `enterAirStart`, `enterStart`, `enterAirEnd` and `enterEnd`.
const ENTER_AIR_START: u32 = 0x8011_C7B0;
const ENTER_START: u32 = 0x8011_C818;
const ENTER_AIR_END: u32 = 0x8011_CBBC;
const ENTER_END: u32 = 0x8011_CC18;
/// `Collide_LeftWallMask`, `Collide_RightWallMask` and `Collide_CeilingMask`.
const LEFT_WALL: u32 = 0x3F;
const RIGHT_WALL: u32 = 0xFC0;
const CEILING: u32 = 0x6000;
/// `Ft_Kind_Mars`, `Ft_Kind_Emblem`.
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;

/// The flags the move keeps changing between the ground and the air (`start_mf`, `end_mf`).
const START_MF: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const END_MF: u32 = SKIP_COL_ANIM | UPDATE_CMD;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    match a {
        S_RESET => g.peach_special_s_reset(gobj),
        SPECIAL_S_ENTER => g.peach_special_s_enter(gobj),
        SPECIAL_AIR_S_ENTER => g.peach_special_air_s_enter(gobj),
        S_AIR_END0 => g.peach_special_s_air_end0(gobj),
        SPECIAL_S_START_ANIM => g.peach_special_s_start_anim(gobj),
        SPECIAL_AIR_S_START_ANIM => g.peach_special_air_s_start_anim(gobj),
        SPECIAL_S_START_IASA
        | SPECIAL_AIR_S_START_IASA
        | SPECIAL_AIR_S_JUMP_IASA
        | SPECIAL_S_END_IASA
        | SPECIAL_AIR_S_END_IASA => g.peach_special_s_iasa(gobj),
        SPECIAL_S_START_PHYS => g.peach_special_s_start_phys(gobj),
        SPECIAL_AIR_S_START_PHYS | SPECIAL_AIR_S_END_PHYS => g.peach_special_air_s_phys(gobj),
        SPECIAL_S_START_COLL => g.peach_special_s_start_coll(gobj),
        SPECIAL_AIR_S_START_COLL => g.peach_special_air_s_start_coll(gobj),
        ENTER_AIR_START => g.peach_special_s_to_air(gobj),
        ENTER_START => g.peach_special_s_to_ground(gobj),
        SPECIAL_AIR_S_JUMP_ANIM => g.peach_special_air_s_jump_anim(gobj),
        SPECIAL_AIR_S_JUMP_PHYS => g.peach_special_air_s_jump_phys(gobj),
        SPECIAL_AIR_S_JUMP_COLL => g.peach_special_air_s_jump_coll(gobj),
        SPECIAL_S_END_ANIM => g.peach_special_s_end_anim(gobj),
        SPECIAL_AIR_S_END_ANIM => g.peach_special_air_s_end_anim(gobj),
        SPECIAL_S_END_PHYS => g.peach_special_s_end_phys(gobj),
        SPECIAL_S_END_COLL => g.peach_special_s_end_coll(gobj),
        SPECIAL_AIR_S_END_COLL => g.peach_special_air_s_end_coll(gobj),
        ENTER_AIR_END => g.peach_special_s_end_to_air(gobj),
        ENTER_END => g.peach_special_s_end_to_ground(gobj),
        _ => return false,
    }
    true
}

/// Whether the fighter, facing `facing`, runs into a wall ahead (`env_flags`).
fn wall_ahead(env: u32, facing: f32) -> bool {
    (env & RIGHT_WALL != 0 && reg(facing) == -1.0) || (env & LEFT_WALL != 0 && reg(facing) == 1.0)
}

impl Running<'_> {
    /// The move's start over (`reset`, its `x21EC` hook): the script's variables cleared, no
    /// speed, and a smash (the stick flicked within the attributes' x30 frames) noted and
    /// counted as an item throw for the statistics.
    #[doc(alias = "reset")]
    pub fn peach_special_s_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let window = self.peach_attr_i32(f, attr::S_SMASH_FRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        fp.self_vel.x = 0.0;
        fp.x6A4_transNOffset.x = 0.0;
        if i32::from(fp.active_sticky.lstick.x) < window {
            fp.x2070[2] |= 0x08;
            fp.set_mv_i32(mv::S_SMASH, 1);
        } else {
            fp.set_mv_i32(mv::S_SMASH, 0);
        }
    }

    /// `ftPe_SpecialS_Enter`: on the ground, a start forward (the attributes' x34).
    #[doc(alias = "ftPe_SpecialS_Enter")]
    pub fn peach_special_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x34 = self.peach_attr_f32(f, attr::S_X34);
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::S_RESET));
        fp.self_vel.y = 0.0;
        fp.gr_vel = mem(fp::fmuls(reg(x34), reg(fp.facing_dir)));
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPe_SpecialAirS_Enter`: in the air, a hop (the attributes' x40).
    #[doc(alias = "ftPe_SpecialAirS_Enter")]
    pub fn peach_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x40 = self.peach_attr_f32(f, attr::S_X40);
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::S_RESET));
        fp.self_vel.y = x40;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The attack meets a hurtbox (`doAirEnd0`, the move's hurtbox hook): unless what it met
    /// isn't a Marth or Roy countering or a Peach using Toad, it stops, explodes and bounces
    /// back.
    #[doc(alias = "doAirEnd0")]
    pub fn peach_special_s_air_end0(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.x221C_b5 {
            let other = fp.unk_gobj.expect("a hit by no one");
            let ofp = self.game.fighter(other);
            match ofp.kind {
                KIND_MARS | KIND_EMBLEM => {
                    if ofp.motion_id != 369 && ofp.motion_id != 371 {
                        return;
                    }
                }
                super::KIND_PEACH => {
                    if ofp.motion_id != ms::SPECIAL_N && ofp.motion_id != ms::SPECIAL_AIR_N {
                        return;
                    }
                }
                _ => return,
            }
        }
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = 0.0;
        if reg(fp.self_vel.y) >= 0.0 {
            fp.self_vel.y = 0.0;
        }
        fp.cmd_vars[2] = 1;
        self.peach_special_air_s_end_smash(gobj);
    }

    /// `ftPe_SpecialSStart_Anim`: at the end, the explosion if the start met a wall, else she
    /// leaves the ground a little back and up and flies.
    #[doc(alias = "ftPe_SpecialSStart_Anim")]
    pub fn peach_special_s_start_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.peach_special_s_end_smash(gobj);
        } else {
            self.go_airborne(f);
            let fp = &mut self.game.fighters[f.0];
            let s = reg(fp.x34_scale.y);
            fp.cur_pos.x = mem(fp::fmadds(
                fp::fmuls(-4.0, reg(fp.facing_dir)),
                s,
                reg(fp.cur_pos.x),
            ));
            fp.cur_pos.y = mem(fp::fmadds(3.5, s, reg(fp.cur_pos.y)));
            self.peach_special_air_s_jump_enter(gobj);
        }
    }

    /// `ftPe_SpecialAirSStart_Anim`: at the end, the explosion if it met something, else the
    /// flight.
    #[doc(alias = "ftPe_SpecialAirSStart_Anim")]
    pub fn peach_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).cmd_vars[0] != 0 {
                self.peach_special_air_s_end_smash(gobj);
            } else {
                self.peach_special_air_s_jump_enter(gobj);
            }
        }
    }

    /// The move's states' interrupts (`ftPe_SpecialSStart_IASA` and the others): none.
    #[doc(alias = "ftPe_SpecialSStart_IASA")]
    #[doc(alias = "ftPe_SpecialAirSStart_IASA")]
    #[doc(alias = "ftPe_SpecialAirSJump_IASA")]
    #[doc(alias = "ftPe_SpecialSEnd_IASA")]
    #[doc(alias = "ftPe_SpecialAirSEnd_IASA")]
    pub fn peach_special_s_iasa(&mut self, _gobj: GObjId) {}

    /// `ftPe_SpecialSStart_Phys`: speeding up forward (the attributes' x38, up to x3C).
    #[doc(alias = "ftPe_SpecialSStart_Phys")]
    pub fn peach_special_s_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (accel, vel) = (
            self.peach_attr_f32(f, attr::S_START_ACCEL),
            self.peach_attr_f32(f, attr::S_START_VEL_X),
        );
        let fp = &mut self.game.fighters[f.0];
        let facing = reg(fp.facing_dir);
        Self::ground_accel_to_vel(
            fp,
            fp::fmuls(reg(accel), facing),
            fp::fmuls(reg(vel), facing),
        );
        self.self_movement_from_ground(gobj);
    }

    /// `ftPe_SpecialAirSStart_Phys` and `ftPe_SpecialAirSEnd_Phys` (`ft_80084EEC`).
    #[doc(alias = "ftPe_SpecialAirSStart_Phys")]
    #[doc(alias = "ftPe_SpecialAirSEnd_Phys")]
    pub fn peach_special_air_s_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftPe_SpecialSStart_Coll`: off the ground into the air's start; a wall ahead stops her
    /// and ends the move in the explosion.
    #[doc(alias = "ftPe_SpecialSStart_Coll")]
    pub fn peach_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.peach_special_s_to_air(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        if wall_ahead(fp.coll_data.env_flags as u32, fp.facing_dir) {
            fp.gr_vel = 0.0;
            fp.cmd_vars[0] = 1;
        }
    }

    /// `ftPe_SpecialAirSStart_Coll`: landing into the ground's start; a ceiling or a wall
    /// ahead stops her and ends the move in the explosion.
    #[doc(alias = "ftPe_SpecialAirSStart_Coll")]
    pub fn peach_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.peach_special_s_to_ground(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.coll_data.env_flags as u32 & CEILING != 0 {
            fp.self_vel.y = 0.0;
            fp.cmd_vars[0] = 1;
        }
        let fp = self.game.fighter_mut(gobj);
        if wall_ahead(fp.coll_data.env_flags as u32, fp.facing_dir) {
            fp.self_vel.x = 0.0;
            fp.cmd_vars[0] = 1;
        }
    }

    /// `enterAirStart`: the start goes on in the air, with no speed.
    #[doc(alias = "enterAirStart")]
    pub fn peach_special_s_to_air(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.x = 0.0;
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START, START_MF);
    }

    /// `enterStart`: the start goes on on the ground.
    #[doc(alias = "enterStart")]
    pub fn peach_special_s_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, START_MF);
    }

    /// `ftPe_SpecialAirSJump_Anim`: the flight ends in the explosion when the script says or
    /// the animation ends.
    #[doc(alias = "ftPe_SpecialAirSJump_Anim")]
    pub fn peach_special_air_s_jump_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[3] != 0 || !self.anim_frames_remaining(gobj) {
            self.peach_special_air_s_end_smash(gobj);
        }
    }

    /// `ftPe_SpecialAirSJump_Phys`: once the script says, slowing (the attributes' x54) and
    /// falling faster (x58 rather than x50), down to x5C.
    #[doc(alias = "ftPe_SpecialAirSJump_Phys")]
    pub fn peach_special_air_s_jump_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x54, x50, x58, x5c) = (
            reg(self.peach_attr_f32(f, attr::S_X54)),
            reg(self.peach_attr_f32(f, attr::S_X50_GRAVITY)),
            reg(self.peach_attr_f32(f, attr::S_X58_GRAVITY)),
            reg(self.peach_attr_f32(f, attr::S_X5C_TERMINAL)),
        );
        let fp = &mut self.game.fighters[f.0];
        let late = fp.cmd_vars[1] != 0;
        if late {
            Self::self_accel_deaccel(fp, x54);
        }
        Self::fall(fp, if late { x58 } else { x50 }, x5c);
    }

    /// `ftPe_SpecialAirSJump_Coll`: landing ends it in the explosion on the ground; a wall
    /// ahead in the air's.
    #[doc(alias = "ftPe_SpecialAirSJump_Coll")]
    pub fn peach_special_air_s_jump_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            self.peach_special_s_end_smash(gobj);
        }
        let fp = self.game.fighter(gobj);
        if wall_ahead(fp.coll_data.env_flags as u32, fp.facing_dir) {
            self.game.fighter_mut(gobj).cmd_vars[2] = 1;
            self.peach_special_air_s_end_smash(gobj);
        }
    }

    /// The flight (`enterAirJump`): forward at the attributes' x44 (x48 for a smash) and up
    /// at x4C, turned forward, and the hurtbox hook that ends it.
    #[doc(alias = "enterAirJump")]
    pub fn peach_special_air_s_jump_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let smash = self.game.fighters[f.0].mv_i32(mv::S_SMASH) != 0;
        let vx = self.peach_attr_f32(
            f,
            if smash {
                attr::S_SMASH_VEL_X
            } else {
                attr::S_VEL_X
            },
        );
        let vy = self.peach_attr_f32(f, attr::S_VEL_Y);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(vx), reg(fp.facing_dir)));
        fp.self_vel.y = vy;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_JUMP, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.x21F8 = Some(CodeRef(FACE_FORWARD));
        fp.hurtbox_detect_cb = Some(CodeRef(addr::S_AIR_END0));
    }

    /// `ftPe_SpecialSEnd_Anim`: at the end she waits.
    #[doc(alias = "ftPe_SpecialSEnd_Anim")]
    pub fn peach_special_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPe_SpecialAirSEnd_Anim`: at the end she falls.
    #[doc(alias = "ftPe_SpecialAirSEnd_Anim")]
    pub fn peach_special_air_s_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPe_SpecialSEnd_Phys`: friction.
    #[doc(alias = "ftPe_SpecialSEnd_Phys")]
    pub fn peach_special_s_end_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftPe_SpecialSEnd_Coll`: off a ledge, the air's end.
    #[doc(alias = "ftPe_SpecialSEnd_Coll")]
    pub fn peach_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.peach_special_s_end_to_air(gobj);
        }
    }

    /// `ftPe_SpecialAirSEnd_Coll`: landing, the ground's end.
    #[doc(alias = "ftPe_SpecialAirSEnd_Coll")]
    pub fn peach_special_air_s_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.peach_special_s_end_to_ground(gobj);
        }
    }

    /// `enterAirEnd`: the end goes on in the air.
    #[doc(alias = "enterAirEnd")]
    pub fn peach_special_s_end_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_END_0, END_MF);
    }

    /// `enterEnd`: the end goes on on the ground.
    #[doc(alias = "enterEnd")]
    pub fn peach_special_s_end_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_END, END_MF);
    }

    /// The explosion when it hit (`doPostEnd`): at her hips, with her bounce back (the
    /// attributes' x60 and x64); she faces back after either end.
    fn peach_special_s_post_end(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            let hip = self.bone_joint(gobj, HIP_N);
            let mut at =
                crate::lb::joint::world_position(&mut self.game.objs, self.data, hip, None);
            at.z = 0.0;
            let fp = &self.game.fighters[f.0];
            let (facing, smash) = (fp.facing_dir, fp.mv_i32(mv::S_SMASH) != 0);
            peachexplode::spawn(self, gobj, at, smash, facing);
            let (vx, vy) = (
                self.peach_attr_f32(f, attr::S_END_VEL_X),
                self.peach_attr_f32(f, attr::S_END_VEL_Y),
            );
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(vx), reg(fp.facing_dir)));
            fp.self_vel.y = vy;
        }
        self.game.fighters[f.0].x21F8 = Some(CodeRef(FACE_BACK));
    }

    /// The ground's end (`enterEndSmash`).
    #[doc(alias = "enterEndSmash")]
    pub fn peach_special_s_end_smash(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S_END, 0, 0.0, 1.0, 0.0, None);
        self.peach_special_s_post_end(gobj);
    }

    /// The air's end (`enterAirEndSmash`): her speed cut by the attributes' x68 and x6C, the
    /// end after a hit (or a wall) or the plain one.
    #[doc(alias = "enterAirEndSmash")]
    pub fn peach_special_air_s_end_smash(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x68, x6c) = (
            reg(self.peach_attr_f32(f, attr::S_X68)),
            reg(self.peach_attr_f32(f, attr::S_X6C)),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x68));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), x6c));
        let msid = if fp.cmd_vars[2] != 0 {
            ms::SPECIAL_AIR_S_END_1
        } else {
            ms::SPECIAL_AIR_S_END_0
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.peach_special_s_post_end(gobj);
    }
}
