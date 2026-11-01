//! The stage-wide sweep and the slap (`ftmasterhandsweep.c`, `ftmasterhandsweepwait.c`,
//! `ftmasterhandslap.c`), the walk with its finger shots, and the drill
//! (`ftmasterhandwalk.c`, `ftmasterhanddrill.c`); `ftmasterhandsweep.c` also holds the beaten
//! hand's physics.

use gekko_fp as fp;

use super::super::super::state::Running;
use super::wait::set_mv_vec;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// Enters one of his states from its first frame (`Fighter_ChangeMotionState` and
    /// `ftAnim_8006EBA4`): `ftMh_MS_344_80151828` (the sweep's wind-up),
    /// `ftMh_MS_347_80151AC8` (its end), `ftMh_MS_348_80151BB8` (the slap),
    /// `ftMh_MS_353_80151DC4` and `ftMh_MS_350_80151EB4` (the walk), `ftMh_MS_354_80152370` and
    /// `ftMh_MS_355_8015247C` (the punch).
    #[doc(alias = "ftMh_MS_344_80151828")]
    #[doc(alias = "ftMh_MS_347_80151AC8")]
    #[doc(alias = "ftMh_MS_348_80151BB8")]
    #[doc(alias = "ftMh_MS_353_80151DC4")]
    #[doc(alias = "ftMh_MS_350_80151EB4")]
    #[doc(alias = "ftMh_MS_354_80152370")]
    #[doc(alias = "ftMh_MS_355_8015247C")]
    #[doc(alias = "ftMh_MS_366_80153820")]
    pub fn mh_enter(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftMh_Damage_Phys`: the beaten hand falls away (`x150`, `x158` each frame).
    #[doc(alias = "ftMh_Damage_Phys")]
    pub fn mh_damage_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (dy, dz) = (self.mh_da_f32(f, attr::X150), self.mh_da_f32(f, attr::X158));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), reg(dy)));
        fp.self_vel.z = mem(fp::fadds(reg(fp.self_vel.z), reg(dz)));
    }

    /// `ftMh_WaitSweep_Anim`: the sweep once the wind-up ends.
    #[doc(alias = "ftMh_WaitSweep_Anim")]
    pub fn mh_wait_sweep_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_sweep_enter(gobj);
        }
    }

    /// A stretch of the sweep, `ftMh_MS_346_80151918`: `x3C` to the left, at `x38`'s height.
    #[doc(alias = "ftMh_MS_346_80151918")]
    pub fn mh_sweep_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (step, y) = (self.mh_da_f32(f, attr::X3C), self.mh_da_f32(f, attr::X38));
        let fp = &mut self.game.fighters[f.0];
        let x = mem(fp::fsubs(reg(fp.cur_pos.x), reg(step)));
        set_mv_vec(fp, mv::TARGET, Vec3::new(x, y, 0.0));
        self.mh_enter(gobj, ms::SWEEP_LOOP);
    }

    /// `ftMh_SweepLoop_Anim`: another stretch, or its end past the stage's left end.
    #[doc(alias = "ftMh_SweepLoop_Anim")]
    pub fn mh_sweep_loop_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let left = self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .floor_left(0);
        if self.game.fighter(gobj).cur_pos.x < left.x {
            self.mh_enter(gobj, ms::SWEEP_WAIT);
            return;
        }
        self.mh_sweep_enter(gobj);
    }

    /// The glide alone (`ftBossLib_8015BE40` toward `mv.xC`): `ftMh_SweepLoop_Phys`.
    #[doc(alias = "ftMh_SweepLoop_Phys")]
    pub fn mh_glide_only(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let target = super::wait::mv_vec(&self.game.fighters[f.0], mv::TARGET);
        let (near, rate) = (self.mh_da_f32(f, attr::X2C), self.mh_da_f32(f, attr::X28));
        let d = self.boss_glide(gobj, target, near, rate);
        self.game.fighters[f.0].set_mv_f32(mv::DISTANCE, reg(d));
    }

    /// The walk's shot, `ftMh_MS_349_80151CA8`: at `x50.x`'s height, stopped.
    #[doc(alias = "ftMh_MS_349_80151CA8")]
    pub fn mh_walk_shoot_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::WALK_SHOOT);
        let y = self.mh_da_f32(f, attr::X50);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.y = y;
        fp.self_vel.x = 0.0;
    }

    /// `ftMh_Walk2_Anim`: walking once the step down ends.
    #[doc(alias = "ftMh_Walk2_Anim")]
    pub fn mh_walk2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_enter(gobj, ms::WALK_LOOP);
        }
    }

    /// `ftMh_WalkLoop_Anim`: the shot once the nearest opponent is within `x4C`, a stop past
    /// the stage's left end, the walk's animation again.
    #[doc(alias = "ftMh_WalkLoop_Anim")]
    pub fn mh_walk_loop_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let cur = self.game.fighters[f.0].cur_pos;
        let other = self.boss_nearest(gobj, cur);
        let other = other.expect("ftLib_GetCameraBonePos of no fighter reads address 0");
        let of = self.game.fighter_id(other);
        let pos = self.camera_bone_pos(of);
        let d = pos - self.game.fighters[f.0].cur_pos;
        let (x, y, z) = (reg(d.x), reg(d.y), reg(d.z));
        let len = crate::math::sqrt_reg(fp::fadds(
            fp::fmuls(z, z),
            fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
        ));
        if len < reg(self.mh_da_f32(f, attr::X4C)) {
            self.mh_walk_shoot_enter(gobj);
        }
        let left = self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .floor_left(0);
        if self.game.fighters[f.0].cur_pos.x < left.x {
            self.mh_walk_wait_enter(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.mh_enter(gobj, ms::WALK_LOOP);
        }
    }

    /// `ftMh_WalkLoop_Phys`: the animation's motion, walking at `x40.z`.
    #[doc(alias = "ftMh_WalkLoop_Phys")]
    pub fn mh_walk_loop_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let v = self.mh_da_f32(f, attr::X40 + 8);
        self.game.fighters[f.0].self_vel.x = v;
    }

    /// The walk's end, `ftMh_MS_351_801520D8`.
    #[doc(alias = "ftMh_MS_351_801520D8")]
    pub fn mh_walk_wait_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::WALK_WAIT);
        self.game.fighter_mut(gobj).self_vel.x = 0.0;
    }

    /// The drill, `ftMh_MS_352_801521DC`: 107 frames (`mv.x0`).
    #[doc(alias = "ftMh_MS_352_801521DC")]
    pub fn mh_drill_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::DRILL);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 107.0);
    }

    /// `ftMh_Drill_Phys`: following the nearest opponent (at most `x80` a frame) while the
    /// count is within `x84`, over the stage.
    #[doc(alias = "ftMh_Drill_Phys")]
    pub fn mh_drill_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let (lim, max) = (self.mh_da_i32(f, attr::X84), self.mh_da_f32(f, attr::X80));
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(lim)) || n < 0.0 {
            fp.self_vel.x = 0.0;
        } else {
            self.boss_follow_x(gobj, max);
        }
        self.boss_clamp_to_floor(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DAMAGE_PHYS => g.mh_damage_phys(gobj),
        addr::MS_344_80151828 => g.mh_enter(gobj, ms::WAIT_SWEEP),
        addr::WAIT_SWEEP_ANIM => g.mh_wait_sweep_anim(gobj),
        addr::MS_346_80151918 => g.mh_sweep_enter(gobj),
        addr::SWEEP_LOOP_ANIM => g.mh_sweep_loop_anim(gobj),
        addr::SWEEP_LOOP_PHYS => g.mh_glide_only(gobj),
        addr::MS_347_80151AC8 => g.mh_enter(gobj, ms::SWEEP_WAIT),
        addr::MS_348_80151BB8 => g.mh_enter(gobj, ms::SLAP),
        addr::MS_349_80151CA8 => g.mh_walk_shoot_enter(gobj),
        addr::MS_353_80151DC4 => g.mh_enter(gobj, ms::WALK2),
        addr::WALK2_ANIM => g.mh_walk2_anim(gobj),
        addr::MS_350_80151EB4 => g.mh_enter(gobj, ms::WALK_LOOP),
        addr::WALK_LOOP_ANIM => g.mh_walk_loop_anim(gobj),
        addr::WALK_LOOP_PHYS => g.mh_walk_loop_phys(gobj),
        addr::MS_351_801520D8 => g.mh_walk_wait_enter(gobj),
        addr::MS_352_801521DC => g.mh_drill_enter(gobj),
        addr::DRILL_PHYS => g.mh_drill_phys(gobj),
        _ => return false,
    }
    true
}
