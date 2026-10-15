//! Final Cutter (`ftkirbyspecialhi.c`): the rise (turned around once by the stick), the drop
//! once past its first frames, the blade slamming down where it meets the ground with the wave
//! it sends (`it::kinds::kirbycutterbeam`), his body leaning along the floor's slope.

use gekko_fp as fp;

use super::super::super::ef::{self, eflib};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::kirbycutterbeam;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `ftKb_MF_SpecialHi_Coll`: what the switches between the ground and the air keep.
const COLL_FLAGS: u32 = KEEP_GFX | SKIP_MAT_ANIM | SKIP_COL_ANIM | SKIP_ITEM_VIS | UNK_19;

/// What the drop's landings keep (`0xC1082`).
const LAND_FLAGS: u32 = 0xC_1082;

/// What the rise's next parts keep (`0xA`).
const NEXT_FLAGS: u32 = 0xA;

/// `ftKb_MotionVars::specialhi`: whether the wave went out (`xC`) and whether the stick turned
/// him around (`x4`); the drop's frames counted (`specialn_pe.facing_dir`, `x0`).
const MV_FRAMES: usize = 0x0;
const MV_TURNED: usize = 0x4;
const MV_X8: usize = 0x8;
const MV_WAVE: usize = 0xC;

impl Running<'_> {
    /// His damage and death cleanups (`ftKb_SpecialN_set_cbs`).
    pub(crate) fn kb_set_cleanup_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(addr::DEATH3));
        fp.take_dmg_cb = Some(CodeRef(addr::DEATH1));
    }

    /// The wave sent when the script says (`cmd_vars[2]`), once, `fn_800F21E8` (an
    /// accessory): ahead of his top joint and a little below.
    #[doc(alias = "fn_800F21E8")]
    pub fn kb_hi_send_wave(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(MV_WAVE) != 0 || fp.cmd_vars[2] == 0 {
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(MV_WAVE, 1);
        let j = self.bone_joint(gobj, 0);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let x = reg(self.kb_attr_f32(f, attr::HI_WAVE_X));
        let y = reg(self.kb_attr_f32(f, attr::HI_WAVE_Y));
        let facing = self.game.fighters[f.0].facing_dir;
        let dx = if facing == 1.0 { x } else { fp::fneg(x) };
        pos.x = mem(fp::fadds(reg(pos.x), dx));
        pos.y = mem(fp::fadds(reg(pos.y), y));
        pos.z = 0.0;
        eflib::set_flags(self.game, Some(gobj), 1);
        pos.y = mem(fp::fsubs(reg(pos.y), 1.0));
        kirbycutterbeam::spawn(self, gobj, pos, facing);
    }

    /// His body upright again, `ftKb_AttackDashAir_800F22D4`.
    #[doc(alias = "ftKb_AttackDashAir_800F22D4")]
    pub fn kb_clear_lean(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    fn kb_hi_enter(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        for off in [MV_FRAMES, MV_TURNED, MV_X8, MV_WAVE] {
            fp.set_mv_i32(off, 0);
        }
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        ef::spawn(self, 0x494, Some(gobj), &[]);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
    }

    /// Final Cutter from the ground, `ftKb_SpecialHi_Enter`.
    #[doc(alias = "ftKb_SpecialHi_Enter")]
    pub fn kb_special_hi_enter(&mut self, gobj: GObjId) {
        self.kb_hi_enter(gobj, ms::SPECIAL_HI1);
    }

    /// In the air, `ftKb_SpecialAirHi_Enter`.
    #[doc(alias = "ftKb_SpecialAirHi_Enter")]
    pub fn kb_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.kb_hi_enter(gobj, ms::SPECIAL_AIR_HI1);
    }

    /// `ftKb_SpecialHi1_Anim`: the rise in the air once the start ends, its frames counted
    /// anew.
    #[doc(alias = "ftKb_SpecialHi1_Anim")]
    pub fn kb_special_hi1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::SPECIAL_AIR_HI2, NEXT_FLAGS, 0.0, 1.0, 0.0, None);
            self.set_effect_hitlag_cbs(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_FRAMES, 0);
        }
    }

    /// `ftKb_SpecialHi2_Anim`: the drop in the air once the rise ends.
    #[doc(alias = "ftKb_SpecialHi2_Anim")]
    pub fn kb_special_hi2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::SPECIAL_AIR_HI3, NEXT_FLAGS, 0.0, 1.0, 0.0, None);
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// `ftKb_SpecialAirHi1_Anim`: the rise once the start ends, its frames counted anew.
    #[doc(alias = "ftKb_SpecialAirHi1_Anim")]
    pub fn kb_special_air_hi1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::SPECIAL_AIR_HI2, NEXT_FLAGS, 0.0, 1.0, 0.0, None);
            self.set_effect_hitlag_cbs(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_FRAMES, 0);
        }
    }

    /// `ftKb_SpecialAirHi2_Anim`: the drop once the rise ends.
    #[doc(alias = "ftKb_SpecialAirHi2_Anim")]
    pub fn kb_special_air_hi2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::SPECIAL_AIR_HI3, NEXT_FLAGS, 0.0, 1.0, 0.0, None);
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The landing's end, `ftKb_SpecialHi4_Anim` and `ftKb_SpecialAirHiEnd_Anim`: upright,
    /// waiting or falling.
    #[doc(alias = "ftKb_SpecialHi4_Anim")]
    #[doc(alias = "ftKb_SpecialAirHiEnd_Anim")]
    pub fn kb_special_hi_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_clear_lean(gobj);
            if self.game.fighter(gobj).ground_or_air == 0 {
                self.enter_wait(gobj);
            } else {
                self.enter_fall(gobj);
            }
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftKb_SpecialHi3_Anim")]
    #[doc(alias = "ftKb_SpecialAirHi3_Anim")]
    #[doc(alias = "ftKb_SpecialHi2_IASA")]
    #[doc(alias = "ftKb_SpecialHi3_IASA")]
    #[doc(alias = "ftKb_SpecialHi4_IASA")]
    #[doc(alias = "ftKb_SpecialAirHi2_IASA")]
    #[doc(alias = "ftKb_SpecialAirHi3_IASA")]
    #[doc(alias = "ftKb_SpecialAirHiEnd_IASA")]
    pub fn kb_special_hi_nothing(&mut self, _gobj: GObjId) {}

    /// The start's turn, `ftKb_SpecialHi1_IASA` and `ftKb_SpecialAirHi1_IASA`: once, while the
    /// script allows (`cmd_vars[3]`), the stick pushed back past the attributes' range turns
    /// him around.
    #[doc(alias = "ftKb_SpecialHi1_IASA")]
    #[doc(alias = "ftKb_SpecialAirHi1_IASA")]
    pub fn kb_special_hi1_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.kb_attr_f32(f, attr::HI_TURN_RANGE));
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[3] != 0 || fp.mv_i32(MV_TURNED) != 0 {
            return;
        }
        let x = reg(fp.input.lstick[0].x);
        let mag = if x < 0.0 { fp::fneg(x) } else { x };
        if !(mag > range) {
            return;
        }
        let facing = reg(fp.facing_dir);
        if (facing == 1.0 && x < 0.0) || (facing == -1.0 && x > 0.0) {
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[3] = 1;
            fp.set_mv_i32(MV_TURNED, 1);
            self.face_stick(gobj);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
            let y = mem(fp::frsp(fp::fmul(half_pi, facing)));
            self.game.part_set_rot_y(f, 0, y);
        }
    }

    /// Drift on the ground at the attributes' share of the air's, as self movement.
    fn kb_hi_ground_drift(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let h = reg(self.kb_attr_f32(f, attr::HI_DRIFT));
        let fp = &mut self.game.fighters[f.0];
        let accel = fp::fmuls(reg(fp.co_attrs.air_drift_stick_mul), h);
        let max = reg(fp.co_attrs.air_drift_max);
        Self::ground_accel_to_stick(fp, 0.0, accel, max);
        self.self_movement_from_ground(gobj);
    }

    /// `ftKb_SpecialHi1_Phys`: friction (`ft_80084FA8`) and the drift.
    #[doc(alias = "ftKb_SpecialHi1_Phys")]
    pub fn kb_special_hi1_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
        self.kb_hi_ground_drift(gobj);
    }

    /// `ftKb_SpecialHi2_Phys`: the animation's motion (`ft_80085134`) and the drift.
    #[doc(alias = "ftKb_SpecialHi2_Phys")]
    pub fn kb_special_hi2_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.kb_hi_ground_drift(gobj);
    }

    /// `ftKb_SpecialHi3_Phys`: the drift.
    #[doc(alias = "ftKb_SpecialHi3_Phys")]
    pub fn kb_special_hi3_phys(&mut self, gobj: GObjId) {
        self.kb_hi_ground_drift(gobj);
    }

    /// `ftKb_SpecialHi4_Phys` (`ft_80084FA8`).
    #[doc(alias = "ftKb_SpecialHi4_Phys")]
    pub fn kb_special_hi4_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// Drift in the air at the attributes' share of the air's.
    fn kb_hi_air_drift(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let h = reg(self.kb_attr_f32(f, attr::HI_DRIFT));
        let fp = &mut self.game.fighters[f.0];
        let accel = fp::fmuls(reg(fp.co_attrs.air_drift_stick_mul), h);
        let max = reg(fp.co_attrs.air_drift_max);
        Self::drift_simple_no_friction(fp, 0.0, accel, max);
    }

    /// `ftKb_SpecialAirHi1_Phys` and `ftKb_SpecialAirHi2_Phys`: the animation's motion, a
    /// rise scaled by the attributes' share, and the drift.
    #[doc(alias = "ftKb_SpecialAirHi1_Phys")]
    #[doc(alias = "ftKb_SpecialAirHi2_Phys")]
    pub fn kb_special_air_hi_rise_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        let f = self.game.fighter_id(gobj);
        let v = reg(self.kb_attr_f32(f, attr::HI_RISE));
        let fp = &mut self.game.fighters[f.0];
        if reg(fp.self_vel.y) > 0.0 {
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), v));
        }
        self.kb_hi_air_drift(gobj);
    }

    /// `ftKb_SpecialAirHi3_Phys`: the drift.
    #[doc(alias = "ftKb_SpecialAirHi3_Phys")]
    pub fn kb_special_air_hi3_phys(&mut self, gobj: GObjId) {
        self.kb_hi_air_drift(gobj);
    }

    /// `ftKb_SpecialAirHiEnd_Phys` (`ft_80085134`).
    #[doc(alias = "ftKb_SpecialAirHiEnd_Phys")]
    pub fn kb_special_air_hi_end_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// His body leaning along the floor's slope, his way.
    fn kb_lean_to_floor(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        let a = mem(fp::fmuls(reg(fp.facing_dir), reg(math::atan2(n.x, n.y))));
        self.game.part_set_rot_x(f, 0, a);
    }

    /// The blade landing: `msid` from the same frame's start, still, leaning on the floor,
    /// the wave to come.
    fn kb_hi_land(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, LAND_FLAGS, 0.0, 1.0, 0.0, None);
        self.kb_set_cleanup_cbs(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(addr::HI_SEND_WAVE));
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
        self.kb_lean_to_floor(gobj);
    }

    /// `ftKb_SpecialHi1_Coll`: off a ledge into the aerial start, upright.
    #[doc(alias = "ftKb_SpecialHi1_Coll")]
    pub fn kb_special_hi1_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_HI1, COLL_FLAGS);
            self.kb_set_cleanup_cbs(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.z = 0.0;
            fp.self_vel.y = 0.0;
            self.kb_clear_lean(gobj);
        }
    }

    /// The drop meeting the ground or a ledge, after the rise's first twenty frames: the
    /// landing `msid` once it lands (`airborne` first going airborne, else landing), else a
    /// ledge caught. Before, it counts the frame and only touches.
    fn kb_hi_drop_coll(&mut self, gobj: GObjId, msid: i32, airborne: bool) {
        let f = self.game.fighter_id(gobj);
        let frames = self.game.fighters[f.0].mv_i32(MV_FRAMES);
        if frames > 20 {
            let dir = if self.game.fighters[f.0].facing_dir == 1.0 {
                1
            } else {
                -1
            };
            if self.air_coll_ground_and_ledge(gobj, dir) {
                self.game.fighters[f.0].set_mv_i32(MV_FRAMES, frames.wrapping_add(1));
                if airborne {
                    self.go_airborne(f);
                } else {
                    self.land(gobj);
                }
                self.kb_hi_land(gobj, msid);
                return;
            }
            if self.catch_ledge_check(gobj) {
                self.enter_cliff_catch(gobj);
            }
        } else {
            self.game.fighters[f.0].set_mv_i32(MV_FRAMES, frames.wrapping_add(1));
            self.go_airborne_jumped(f);
            self.air_coll_touch(gobj);
        }
    }

    /// `ftKb_SpecialHi2_Coll`.
    #[doc(alias = "ftKb_SpecialHi2_Coll")]
    pub fn kb_special_hi2_coll(&mut self, gobj: GObjId) {
        self.kb_hi_drop_coll(gobj, ms::SPECIAL_AIR_HI4, true);
    }

    /// `ftKb_SpecialAirHi2_Coll`.
    #[doc(alias = "ftKb_SpecialAirHi2_Coll")]
    pub fn kb_special_air_hi2_coll(&mut self, gobj: GObjId) {
        self.kb_hi_drop_coll(gobj, ms::SPECIAL_HI4, false);
    }

    /// The fall meeting the ground or a ledge: the landing `msid` (`airborne` first going
    /// airborne, else landing), else a ledge caught.
    fn kb_hi_fall_coll(&mut self, gobj: GObjId, msid: i32, airborne: bool) {
        let f = self.game.fighter_id(gobj);
        let dir = if self.game.fighters[f.0].facing_dir == 1.0 {
            1
        } else {
            -1
        };
        if self.air_coll_ground_and_ledge(gobj, dir) {
            if airborne {
                self.go_airborne(f);
            } else {
                self.land(gobj);
            }
            self.kb_hi_land(gobj, msid);
            return;
        }
        if self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
        }
    }

    /// `ftKb_SpecialHi3_Coll`.
    #[doc(alias = "ftKb_SpecialHi3_Coll")]
    pub fn kb_special_hi3_coll(&mut self, gobj: GObjId) {
        self.kb_hi_fall_coll(gobj, ms::SPECIAL_AIR_HI4, true);
    }

    /// `ftKb_SpecialAirHi3_Coll`.
    #[doc(alias = "ftKb_SpecialAirHi3_Coll")]
    pub fn kb_special_air_hi3_coll(&mut self, gobj: GObjId) {
        self.kb_hi_fall_coll(gobj, ms::SPECIAL_HI4, false);
    }

    /// `ftKb_SpecialHi4_Coll`: off a ledge into the aerial landing, upright; else leaning
    /// along the floor.
    #[doc(alias = "ftKb_SpecialHi4_Coll")]
    pub fn kb_special_hi4_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_HI4, COLL_FLAGS);
            self.kb_set_cleanup_cbs(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.accessory4_cb = Some(CodeRef(addr::HI_SEND_WAVE));
            fp.self_vel.z = 0.0;
            fp.self_vel.y = 0.0;
            self.kb_clear_lean(gobj);
            return;
        }
        self.kb_lean_to_floor(gobj);
    }

    /// `ftKb_SpecialAirHi1_Coll`: landing into the grounded start.
    #[doc(alias = "ftKb_SpecialAirHi1_Coll")]
    pub fn kb_special_air_hi1_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_HI1, COLL_FLAGS);
            self.kb_set_cleanup_cbs(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.z = 0.0;
            fp.self_vel.y = 0.0;
        }
    }

    /// `ftKb_SpecialAirHiEnd_Coll`: landing into the grounded landing, leaning along the
    /// floor; else upright.
    #[doc(alias = "ftKb_SpecialAirHiEnd_Coll")]
    pub fn kb_special_air_hi_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_HI4, COLL_FLAGS);
            self.kb_set_cleanup_cbs(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.accessory4_cb = Some(CodeRef(addr::HI_SEND_WAVE));
            fp.self_vel.z = 0.0;
            fp.self_vel.y = 0.0;
            self.kb_lean_to_floor(gobj);
            return;
        }
        self.kb_clear_lean(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_SEND_WAVE => g.kb_hi_send_wave(gobj),
        addr::CLEAR_LEAN => g.kb_clear_lean(gobj),
        addr::SPECIAL_HI_ENTER => g.kb_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.kb_special_air_hi_enter(gobj),
        addr::SPECIAL_HI1_ANIM => g.kb_special_hi1_anim(gobj),
        addr::SPECIAL_HI2_ANIM => g.kb_special_hi2_anim(gobj),
        addr::SPECIAL_HI4_ANIM | addr::SPECIAL_AIR_HI_END_ANIM => g.kb_special_hi_end_anim(gobj),
        addr::SPECIAL_AIR_HI1_ANIM => g.kb_special_air_hi1_anim(gobj),
        addr::SPECIAL_AIR_HI2_ANIM => g.kb_special_air_hi2_anim(gobj),
        addr::SPECIAL_HI3_ANIM
        | addr::SPECIAL_AIR_HI3_ANIM
        | addr::SPECIAL_HI2_IASA
        | addr::SPECIAL_HI3_IASA
        | addr::SPECIAL_HI4_IASA
        | addr::SPECIAL_AIR_HI2_IASA
        | addr::SPECIAL_AIR_HI3_IASA
        | addr::SPECIAL_AIR_HI_END_IASA => g.kb_special_hi_nothing(gobj),
        addr::SPECIAL_HI1_IASA | addr::SPECIAL_AIR_HI1_IASA => g.kb_special_hi1_iasa(gobj),
        addr::SPECIAL_HI1_PHYS => g.kb_special_hi1_phys(gobj),
        addr::SPECIAL_HI2_PHYS => g.kb_special_hi2_phys(gobj),
        addr::SPECIAL_HI3_PHYS => g.kb_special_hi3_phys(gobj),
        addr::SPECIAL_HI4_PHYS => g.kb_special_hi4_phys(gobj),
        addr::SPECIAL_AIR_HI1_PHYS | addr::SPECIAL_AIR_HI2_PHYS => {
            g.kb_special_air_hi_rise_phys(gobj)
        }
        addr::SPECIAL_AIR_HI3_PHYS => g.kb_special_air_hi3_phys(gobj),
        addr::SPECIAL_AIR_HI_END_PHYS => g.kb_special_air_hi_end_phys(gobj),
        addr::SPECIAL_HI1_COLL => g.kb_special_hi1_coll(gobj),
        addr::SPECIAL_HI2_COLL => g.kb_special_hi2_coll(gobj),
        addr::SPECIAL_HI3_COLL => g.kb_special_hi3_coll(gobj),
        addr::SPECIAL_HI4_COLL => g.kb_special_hi4_coll(gobj),
        addr::SPECIAL_AIR_HI1_COLL => g.kb_special_air_hi1_coll(gobj),
        addr::SPECIAL_AIR_HI2_COLL => g.kb_special_air_hi2_coll(gobj),
        addr::SPECIAL_AIR_HI3_COLL => g.kb_special_air_hi3_coll(gobj),
        addr::SPECIAL_AIR_HI_END_COLL => g.kb_special_air_hi_end_coll(gobj),
        _ => return false,
    }
    true
}
