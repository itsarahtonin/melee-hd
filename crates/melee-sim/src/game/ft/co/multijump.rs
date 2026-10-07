//! Kirby's and Jigglypuff's several double jumps (`ftCo_JumpAerialF1.c`), the double jump's
//! turn around, and the screw attack's jump.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::checks::abs;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// `Ft_Kind_Kirby`.
const KIND_KIRBY: i32 = 4;
/// `ftCommon_MotionVars_jumpaerial`: the turn's frames left.
const JUMP_AERIAL_TURN: usize = 0x0;

/// `Fighter_x2D0_t`, the multi-jump attributes: the turn's frames, the stick's pull back
/// that turns, the horizontal and drift multipliers, each jump's rise, the states' count and
/// first state (with and without Kirby's hat).
const MJ_TURN_FRAMES: u32 = 0x0;
const MJ_TURN_STICK: u32 = 0x4;
const MJ_X_IMPULSE: u32 = 0x8;
const MJ_DRIFT_MUL: u32 = 0xC;
const MJ_SPEED_MUL: u32 = 0x10;
const MJ_Y_IMPULSE: u32 = 0x14;
const MJ_STATE_COUNT: u32 = 0x28;
const MJ_START_STATE: u32 = 0x2C;

impl Running<'_> {
    /// The multi-jump attributes' word at `off` (`Fighter::x2D0`, the character's attributes).
    fn mj_word(&self, gobj: GObjId, off: u32) -> u32 {
        let id = self
            .game
            .fighter(gobj)
            .x2D0
            .expect("a multi-jump without x2D0");
        let b = &self.game.fighter_attrs[id.0][off as usize..off as usize + 4];
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }

    fn mj_f32(&self, gobj: GObjId, off: u32) -> f64 {
        reg(f32::from_bits(self.mj_word(gobj, off)))
    }

    fn mj_i32(&self, gobj: GObjId, off: u32) -> i32 {
        self.mj_word(gobj, off) as i32
    }

    /// Which first state Kirby's jumps use, `ftCo_800D7268`: with a hat or not (the others
    /// have one).
    #[doc(alias = "ftCo_800D7268")]
    pub fn multi_jump_set(&mut self, gobj: GObjId) -> i32 {
        if self.game.fighter(gobj).kind == KIND_KIRBY {
            unported_call("ftKb_SpecialN_800F1CD8");
        }
        0
    }

    /// Whether the fighter's in one of its multi-jump states, `ftCo_800D72A0`.
    #[doc(alias = "ftCo_800D72A0")]
    pub fn in_multi_jump(&self, gobj: GObjId) -> bool {
        let count = self.mj_i32(gobj, MJ_STATE_COUNT);
        let msid = self.game.fighter(gobj).motion_id;
        (0..2).any(|i| {
            let start = self.mj_i32(gobj, MJ_START_STATE + 4 * i);
            start != -1 && start <= msid && msid < start.wrapping_add(count)
        })
    }

    /// A multi-jumper's double jump, `ftCo_800D730C`: out of a screw attack's jump by the
    /// usual input; the first by it too (with the screw attack held, its jump); the later
    /// ones by the stick held up or X/Y held, once the current jump's script allows.
    #[doc(alias = "ftCo_800D730C")]
    pub fn multi_jump_check(&mut self, gobj: GObjId, after_z: bool) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.motion_id == ms::ITEM_SCREW_AIR {
            if self.did_jump(gobj, after_z) {
                self.enter_multi_jump(gobj);
                return true;
            }
            return false;
        }
        if fp.x1968_jumpsUsed == 1 {
            if self.did_jump(gobj, after_z) {
                if self.holding_screw(gobj) {
                    let x = reg(self.game.fighter(gobj).input.lstick[0].x);
                    let vel = [
                        mem(fp::fmuls(x, self.mj_f32(gobj, MJ_X_IMPULSE))),
                        mem(self.mj_f32(gobj, MJ_Y_IMPULSE)),
                        0.0,
                    ];
                    self.screw_jump(gobj, vel);
                } else {
                    let f = self.game.fighter_id(gobj);
                    self.go_airborne(f);
                    self.enter_multi_jump(gobj);
                }
                return true;
            }
            return false;
        }
        let t = self.common().f32(0x70);
        let can = i32::from(fp.x1968_jumpsUsed) < fp.co_attrs.max_jumps;
        let held = fp.input.lstick[0].y >= t || fp.input.held_buttons[0] & 0xC00 != 0;
        let allowed = !(self.in_multi_jump(gobj) && self.game.fighter(gobj).cmd_vars[0] == 0);
        if can && allowed && held {
            self.enter_multi_jump(gobj);
            return true;
        }
        false
    }

    /// A multi-jumper's next double jump, `ftCo_800D74A4`: its own state and rise, turning
    /// around over the attributes' frames with the stick pulled back.
    #[doc(alias = "ftCo_800D74A4")]
    pub fn enter_multi_jump(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let set = self.multi_jump_set(gobj) as u32;
        let start = self.mj_i32(gobj, MJ_START_STATE.wrapping_add(set.wrapping_mul(4)));
        let jumps = i32::from(self.game.fighter(gobj).x1968_jumpsUsed);
        let msid = jumps.wrapping_add(start).wrapping_sub(1);
        let x = reg(self.game.fighter(gobj).input.lstick[0].x);
        let vx = mem(fp::fmuls(x, self.mj_f32(gobj, MJ_X_IMPULSE)));
        let set = self.multi_jump_set(gobj) as u32;
        let start = self.mj_i32(gobj, MJ_START_STATE.wrapping_add(set.wrapping_mul(4)));
        let i = msid.wrapping_sub(start) as u32;
        let vy = self.mj_f32(gobj, MJ_Y_IMPULSE.wrapping_add(i.wrapping_mul(4)));
        self.enter_jump_aerial(gobj, msid, [vx, mem(vy), 0.0], false);
        let (turn, stick) = (
            self.mj_i32(gobj, MJ_TURN_FRAMES),
            self.mj_f32(gobj, MJ_TURN_STICK),
        );
        let fp = self.game.fighter_mut(gobj);
        let pulled = fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) < fp::fneg(stick);
        fp.set_mv_i32(JUMP_AERIAL_TURN, if pulled { turn } else { 0 });
        self.jump_turn(gobj, turn);
    }

    /// The double jump's turn around, `ft_800CB6EC`: the model spins a half turn over
    /// `frames`, the fighter facing the other way halfway.
    #[doc(alias = "ft_800CB6EC")]
    pub fn jump_turn(&mut self, gobj: GObjId, frames: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let left = fp.mv_i32(JUMP_AERIAL_TURN);
        if left == 0 {
            return;
        }
        fp.set_mv_i32(JUMP_AERIAL_TURN, left.wrapping_sub(1));
        let j = self.bones(f)[0].joint.expect("jobj.h:1041: jobj");
        let step = fp::fdivs(180.0, fp::frsp(f64::from(frames)));
        let o = &self.game.objs.jobjs[j];
        let y = mem(fp::fnmsubs(reg(0.017453292f32), step, reg(o.rotate.y)));
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.rotate.y = y);
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(JUMP_AERIAL_TURN) == frames / 2 {
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
    }

    /// The screw attack's jump, `ft_800D2E7C`: `vel` scaled by `ftCommonData` x800.
    #[doc(alias = "ft_800D2E7C")]
    pub fn screw_jump(&mut self, gobj: GObjId, vel: [f32; 3]) {
        let s = reg(self.common().f32(0x800));
        let vel = [
            mem(fp::fmuls(reg(vel[0]), s)),
            mem(fp::fmuls(reg(vel[1]), s)),
            vel[2],
        ];
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.enter_jump_aerial(gobj, ms::ITEM_SCREW_AIR, vel, true);
    }

    /// `ftCo_ItemScrewAir_Anim`: falling helplessly when it plays out.
    #[doc(alias = "ftCo_ItemScrewAir_Anim")]
    pub fn co_item_screw_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall_aerial(gobj);
        }
    }

    /// `ftCo_JumpAerialF1_Anim`: the turn; when it plays out, falling (helplessly out of
    /// jumps).
    #[doc(alias = "ftCo_JumpAerialF1_Anim")]
    pub fn co_jump_aerial_f1_anim(&mut self, gobj: GObjId) {
        let turn = self.mj_i32(gobj, MJ_TURN_FRAMES);
        self.jump_turn(gobj, turn);
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter(gobj);
            if i32::from(fp.x1968_jumpsUsed) >= fp.co_attrs.max_jumps {
                self.enter_fall_aerial(gobj);
            } else {
                self.enter_fall(gobj);
            }
        }
    }

    /// `ftCo_JumpAerialF1_Phys`: gravity or a fast fall, and drift scaled by the attributes
    /// (`ft_80084E1C`).
    #[doc(alias = "ftCo_JumpAerialF1_Phys")]
    pub fn co_jump_aerial_f1_phys(&mut self, gobj: GObjId) {
        let threshold = reg(self.common().f32(0x258));
        let (dm, sm) = (
            self.mj_f32(gobj, MJ_DRIFT_MUL),
            self.mj_f32(gobj, MJ_SPEED_MUL),
        );
        let a = &self.game.fighter(gobj).co_attrs;
        let drift = fp::fmuls(reg(a.air_drift_stick_mul), dm);
        let target = fp::fmuls(reg(a.air_drift_max), sm);
        self.air_physics_scaled(gobj, threshold, drift, target);
    }

    /// Gravity or a fast fall, then drift toward the stick past `threshold`, `ft_80084E1C`.
    #[doc(alias = "ft_80084E1C")]
    pub fn air_physics_scaled(&mut self, gobj: GObjId, threshold: f64, drift: f64, target: f64) {
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.fall_fast {
            // `ftCommon_FallFast`.
            fp.self_vel.y = mem(fp::fneg(reg(fp.co_attrs.fast_fall_velocity)));
        } else {
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
        }
        let x = reg(fp.input.lstick[0].x);
        let (d, t) = if abs(x) >= threshold {
            (fp::fmuls(x, drift), fp::fmuls(x, target))
        } else {
            (0.0, 0.0)
        };
        let friction = reg(fp.co_attrs.aerial_friction);
        let vel = reg(fp.self_vel.x);
        Self::accel_to_vel_clamped(fp, vel, d, t, friction);
    }

    /// `ftCo_JumpAerialF1_Coll`: `ft_80082F28`.
    #[doc(alias = "ftCo_JumpAerialF1_Coll")]
    pub fn co_jump_aerial_f1_coll(&mut self, gobj: GObjId) {
        self.air_coll_full(gobj);
    }
}
