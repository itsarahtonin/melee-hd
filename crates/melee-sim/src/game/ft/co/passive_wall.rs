//! Wall jumps and wall techs (`ftwalljump.c`, `ftCo_PassiveWall.c`): the wall jump's inputs
//! against a wall the fighter hugs, the tech's, and the two states (`PassiveWall`,
//! `PassiveWallJump`) that cling to the wall for a moment and push off.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::ids::ms;
use super::jump::FT_800881D8;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::mp::coll::{LEFT_WALL_HUG, RIGHT_WALL_HUG};
use crate::vec::Vec3;

/// The wall jump's input timer at rest (`max_input_frames`).
const MAX_INPUT_FRAMES: u8 = 254;

/// `ftCommon_MotionVars_passivewall`: the cling's frames, `x4`, whether the jump was asked
/// for during them, and the jump's height exponent.
const PW_TIMER: usize = 0x0;
const PW_X4: usize = 0x4;
const PW_X8: usize = 0x8;
const PW_VEL_Y_EXPONENT: usize = 0xC;

/// `ftPassiveWall`'s rumble, color animation and sound.
const RUMBLE: u32 = 12;
const COLANIM: i32 = 120;
const SFX_CLING: i32 = 3;
const SFX_PUSH: i32 = 8;

impl Running<'_> {
    /// Wall jumping off the wall the fighter hugs, `ftWallJump_8008169C`: approaching the
    /// wall fast enough (relative to it) starts the input timer for that side, and the stick
    /// flicked away from it before the timer runs out jumps (`ftCo_800C1E64`). Not hugging
    /// a wall puts the timer at rest.
    #[doc(alias = "ftWallJump_8008169C")]
    pub fn wall_jump_check(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if !self.game.fighters[f.0].can_walljump {
            return false;
        }
        let flags = self.game.fighters[f.0].coll_data.env_flags;
        if flags & (RIGHT_WALL_HUG | LEFT_WALL_HUG) == 0 {
            self.game.fighters[f.0].wall_jump_input_timer = MAX_INPUT_FRAMES;
            return false;
        }
        let right = flags & RIGHT_WALL_HUG != 0;
        let wall_dir: f32 = if right { -1.0 } else { 1.0 };
        let fp = &self.game.fighters[f.0];
        if fp.wall_jump_input_timer < MAX_INPUT_FRAMES && wall_dir == fp.x2110_walljumpWallSide {
            let fp = &mut self.game.fighters[f.0];
            fp.wall_jump_input_timer += 1;
        } else {
            let c = &fp.coll_data;
            let (vertex, line) = if right {
                (c.ecb.left.clone(), c.right_facing_wall.index)
            } else {
                (c.ecb.right.clone(), c.left_facing_wall.index)
            };
            let ecb = Vec3::new(
                mem(fp::fadds(reg(vertex.x), reg(fp.cur_pos.x))),
                mem(fp::fadds(reg(vertex.y), reg(fp.cur_pos.y))),
                mem(fp::fadds(0.0, reg(fp.cur_pos.z))),
            );
            let geometry = self.game.geometry.as_ref().expect("no collision geometry");
            let wall_x = geometry.line_speed(line, ecb).map_or(0.0, |v| v.x);
            let fp = &mut self.game.fighters[f.0];
            let rel = fp::fsubs(reg(fp.pos_delta.x), reg(wall_x));
            let rel = if rel < 0.0 { fp::fneg(rel) } else { rel };
            if rel > reg(fp.co_attrs.wall_jump_min_approach_speed) {
                fp.x2110_walljumpWallSide = wall_dir;
                fp.wall_jump_input_timer = 0;
            }
        }
        let c = self.common();
        let (window, flick, tilt, timer) = (
            reg(c.f32(0x768)),
            reg(c.f32(0x76C)),
            reg(c.f32(0x770)),
            c.i32(0x774),
        );
        let fp = &self.game.fighters[f.0];
        let side = fp.x2110_walljumpWallSide;
        let x = reg(fp.input.lstick[0].x);
        let away = (side == -1.0 && x >= flick) || (side == 1.0 && x <= fp::fneg(flick));
        if f64::from(fp.wall_jump_input_timer) < window
            && away
            && f64::from(fp.active_timer.lstick.x) < tilt
        {
            let used = i32::from(fp.x1969_walljumpUsed);
            self.cling_to_wall(gobj, ms::PASSIVE_WALL_JUMP, timer, used, side);
            let fp = &mut self.game.fighters[f.0];
            fp.wall_jump_input_timer = MAX_INPUT_FRAMES;
            fp.x1969_walljumpUsed = fp.x1969_walljumpUsed.saturating_add(1);
            return true;
        }
        false
    }

    /// A wall tech, `ftCo_800C1D38`: hugging a wall with the tech's input
    /// (`ftCo_800986B0`), a cling (`ftCommonData` x760 frames) then the tech, or a wall jump
    /// with a jump asked for (`ftCo_800C1E0C`).
    // Ports `ftCo_800C1D38`; aliased once a check reaches it.
    pub fn passive_wall_check(&mut self, gobj: GObjId) -> bool {
        let flags = self.game.fighter(gobj).coll_data.env_flags;
        if flags & (RIGHT_WALL_HUG | LEFT_WALL_HUG) == 0 || !self.tech_input(gobj) {
            return false;
        }
        let msid = if self.wall_jump_asked(gobj) {
            ms::PASSIVE_WALL_JUMP
        } else {
            ms::PASSIVE_WALL
        };
        let timer = self.common().i32(0x760);
        let side = if flags & RIGHT_WALL_HUG != 0 {
            -1.0
        } else {
            1.0
        };
        self.cling_to_wall(gobj, msid, timer, 0, side);
        true
    }

    /// Whether a jump is asked for during a tech, `ftCo_800C1E0C`: the jump button within
    /// `ftCommonData` x250 frames (`x67E`), or the stick past the tap-jump threshold.
    // Ports `ftCo_800C1E0C`; aliased once a check reaches it.
    pub fn wall_jump_asked(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let fp = self.game.fighter(gobj);
        fp::frsp(f64::from(fp.x67E)) < reg(c.f32(0x250))
            || reg(fp.input.lstick[0].y) >= reg(c.f32(0x70))
    }

    /// Clinging to the wall, `ftCo_800C1E64`: facing away from it, stopped, an effect where
    /// the ECB meets it, the state still for `timer` frames (moving at once without one),
    /// the fighter set against the wall by its TransN, sounds, a rumble, a flash and
    /// intangibility (`ftCommonData` x764).
    // Ports `ftCo_800C1E64`; aliased once a check reaches it.
    pub fn cling_to_wall(
        &mut self,
        gobj: GObjId,
        msid: i32,
        timer: i32,
        vel_y_exponent: i32,
        facing_dir: f32,
    ) {
        let f = self.game.fighter_id(gobj);
        {
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[0] = 1;
            fp.facing_dir = mem(fp::fneg(reg(facing_dir)));
        }
        self.stop_dead(gobj);
        let fp = &self.game.fighters[f.0];
        let v = if fp.coll_data.env_flags & RIGHT_WALL_HUG != 0 {
            fp.coll_data.ecb.left.clone()
        } else {
            fp.coll_data.ecb.right.clone()
        };
        let offset = Vec3::new(v.x, v.y, 0.0);
        self.kirby_bump_effect(gobj, offset);
        let rate = if timer != 0 { 0.0 } else { 1.0 };
        self.change_motion_state(gobj, msid, 0, 0.0, rate, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.active_timer.lstick.x = 0xFE;
        fp.active_timer.lstick.y = 0xFE;
        fp.set_mv_i32(PW_TIMER, timer);
        fp.set_mv_i32(PW_X4, 0);
        fp.set_mv_i32(PW_X8, 0);
        fp.set_mv_i32(PW_VEL_Y_EXPONENT, vel_y_exponent);
        let x = mem(fp::fadds(reg(fp.cur_pos.x), reg(offset.x)));
        let pos = Vec3::new(
            x,
            mem(fp::fadds(reg(fp.cur_pos.y), reg(offset.y))),
            fp.cur_pos.z,
        );
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffset,
            0x41D,
            None,
            &[ef::Arg::Vec(pos)],
        );
        let fp = &mut self.game.fighters[f.0];
        let toward = fp::fmuls(reg(fp.x68C_transNPos.z), fp::fneg(reg(fp.facing_dir)));
        fp.cur_pos.x = mem(fp::fneg(fp::fsubs(toward, reg(x))));
        self.air_coll_ledges_snap_low(gobj);
        let sfx = self.character_sfx(gobj, 0x24);
        self.play_track(gobj, FT_800881D8, sfx);
        self.start_rumble(f, RUMBLE, 0);
        if timer == 0 {
            self.fighter_play_sfx(f, SFX_CLING, 0x7F, 0x40);
        }
        self.start_color_anim(gobj, COLANIM, 0);
        let frames = self.common().i32(0x764);
        self.set_intangible(gobj, frames);
    }

    /// The cling's end: pushing off, `inlineA0` when a jump was asked for during it.
    fn passive_wall_to_jump(&mut self, gobj: GObjId) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms::PASSIVE_WALL_JUMP, 0, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.active_timer.lstick.y = 0xFE;
        fp.set_mv_i32(PW_TIMER, 0);
    }

    /// `ftCo_PassiveWall_Anim`: when the cling runs out, a sound and the push off (a wall
    /// jump's speed, lower for each one since landing, or the tech's), then the fall once
    /// the animation ends.
    // Ports `ftCo_PassiveWall_Anim`; aliased once a check reaches it.
    pub fn co_passive_wall_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let timer = self.game.fighters[f.0].mv_i32(PW_TIMER);
        if timer != 0 {
            let timer = timer - 1;
            self.game.fighters[f.0].set_mv_i32(PW_TIMER, timer);
            if timer == 0 {
                self.fighter_play_sfx(f, SFX_PUSH, 127, 64);
                if self.game.fighters[f.0].mv_i32(PW_X8) != 0 {
                    self.passive_wall_to_jump(gobj);
                } else {
                    self.set_anim_rate(gobj, 1.0);
                }
                let base = self.common().f32(0x778);
                let fp = &mut self.game.fighters[f.0];
                if fp.motion_id == ms::PASSIVE_WALL {
                    fp.self_vel.x = mem(fp::fmuls(
                        reg(fp.facing_dir),
                        reg(fp.co_attrs.passivewall_vel_x),
                    ));
                } else {
                    fp.self_vel.x = mem(fp::fmuls(
                        reg(fp.facing_dir),
                        reg(fp.co_attrs.wall_jump_horizontal_velocity),
                    ));
                    fp.self_vel.y = fp.co_attrs.wall_jump_vertical_velocity;
                    if fp.x1969_walljumpUsed != 0 {
                        let e = fp.mv_i32(PW_VEL_Y_EXPONENT);
                        let k = math::pow(base, mem(fp::frsp(f64::from(e))));
                        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), reg(k)));
                    }
                }
            }
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_PassiveWall_IASA`: after the cling, the aerial actions; during it, a jump asked
    /// for is noted.
    // Ports `ftCo_PassiveWall_IASA`; aliased once a check reaches it.
    pub fn co_passive_wall_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(PW_TIMER) == 0 {
            let _ = self.special_air_check(gobj)
                || self.item_throw_air_check(gobj)
                || self.air_catch_item_check(gobj)
                || self.tether_check(gobj)
                || self.escape_air_check(gobj)
                || self.attack_air_check(gobj)
                || self.air_z_check(gobj)
                || self.jump_aerial_check(gobj)
                || self.game.fighter(gobj).cmd_vars[0] == 0
                || self.parasol_check(gobj);
        } else if self.wall_jump_asked(gobj) {
            self.game.fighter_mut(gobj).set_mv_i32(PW_X8, 1);
        }
    }

    /// `ftCo_PassiveWall_Phys`: after the cling, gravity or a fast fall, and air friction.
    // Ports `ftCo_PassiveWall_Phys`; aliased once a check reaches it.
    pub fn co_passive_wall_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(PW_TIMER) != 0 {
            return;
        }
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.fall_fast {
            // `ftCommon_FallFast`.
            fp.self_vel.y = mem(fp::fneg(reg(fp.co_attrs.fast_fall_velocity)));
        } else {
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
        }
        // `ftCommon_CalcSelfAccel_AccelToVelClamped(fp, 0, 0, aerial_friction)`: with no
        // target speed, plain friction.
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftCo_PassiveWall_Coll`: the filtered airborne pass (with the lower ECB during the
    /// cling), landing normally.
    // Ports `ftCo_PassiveWall_Coll`; aliased once a check reaches it.
    pub fn co_passive_wall_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(PW_TIMER) != 0 {
            self.air_coll_filtered_full_low(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
        } else {
            self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
        }
    }
}
