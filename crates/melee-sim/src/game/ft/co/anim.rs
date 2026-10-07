//! The common action states' animation callbacks: each frame after the animation steps, a
//! state checks whether it has played out and what follows.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::super::parts::TRANS_N;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// `ftCo_SM_Fall`, `ftCo_SM_FallF`, `ftCo_SM_FallB`: the fall animations by drift.
const SM_FALL: [i32; 3] = [20, 21, 22];

impl Running<'_> {
    /// The state ends when its animation does, back to waiting (`ft_8008A2BC`).
    fn wait_when_done(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Wait_Anim`: an idle animation plays out and another starts.
    #[doc(alias = "ftCo_Wait_Anim")]
    pub fn co_wait_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).stamina_dead {
            self.enter_down_spot(gobj);
        } else {
            self.wait_anim_tick(gobj);
        }
    }

    /// `ftCo_8008A7A8`: when the idle animation ends, the next is picked (not ported yet:
    /// only the check that it hasn't ended).
    fn wait_anim_tick(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            unported_call("ftCo_8008A7A8");
        }
    }

    /// `ftCo_Landing_Anim`, also `ftCo_LandingAir_Anim`'s.
    #[doc(alias = "ftCo_Landing_Anim")]
    #[doc(alias = "ftCo_LandingAir_Anim")]
    pub fn co_landing_anim(&mut self, gobj: GObjId) {
        self.wait_when_done(gobj);
    }

    /// The ground attacks, rolls and spot dodges that end in waiting: `ftCo_AttackHi3_Anim`,
    /// `ftCo_AttackS3_Anim`, `ftCo_EscapeN_Anim`.
    #[doc(alias = "ftCo_AttackHi3_Anim")]
    #[doc(alias = "ftCo_AttackS3_Anim")]
    #[doc(alias = "ftCo_AttackS4_Anim")]
    #[doc(alias = "ftCo_EscapeN_Anim")]
    pub fn co_ground_move_anim(&mut self, gobj: GObjId) {
        self.wait_when_done(gobj);
    }

    /// `ftCo_AttackAir_Anim`: an aerial that turned the fighter around, then falling.
    #[doc(alias = "ftCo_AttackAir_Anim")]
    pub fn co_attack_air_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.throw_flag(3) {
            fp.set_throw_flag(3, false);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_Jump_Anim`.
    #[doc(alias = "ftCo_Jump_Anim")]
    pub fn co_jump_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_JumpAerial_Anim`.
    #[doc(alias = "ftCo_JumpAerial_Anim")]
    pub fn co_jump_aerial_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall_aerial(gobj);
        }
    }

    /// `ftCo_KneeBend_Anim`: the jump leaves the ground after the character's jump squat.
    #[doc(alias = "ftCo_KneeBend_Anim")]
    pub fn co_knee_bend_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cur_anim_frame >= fp.co_attrs.jump_startup_time || !self.anim_frames_remaining(gobj) {
            unported_call("ftCo_Jump_Enter");
        }
    }

    /// `ftCo_EscapeAir_Anim`.
    #[doc(alias = "ftCo_EscapeAir_Anim")]
    pub fn co_escape_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            unported_call("ftCo_80096900");
        }
    }

    /// `ftCo_CliffWait_Anim`: the ledge's invincibility counts down.
    #[doc(alias = "ftCo_CliffWait_Anim")]
    pub fn co_cliff_wait_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(4) > 0.0 {
            fp.set_mv_f32(4, fp::fsubs(reg(fp.mv_f32(4)), 1.0));
        }
    }

    /// `ftCo_Turn_Anim`: the fighter faces the other way once its turn frames pass.
    #[doc(alias = "ftCo_Turn_Anim")]
    pub fn co_turn_anim(&mut self, gobj: GObjId) {
        // `ftCo_Turn_Anim_Inner`.
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(0x10) > 0.0 {
            fp.set_mv_f32(0x10, fp::fsubs(reg(fp.mv_f32(0x10)), 1.0));
        } else if fp.mv_i32(0) == 0 {
            fp.set_mv_i32(0, 1);
            fp.set_mv_i32(0x18, 1);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        self.wait_when_done(gobj);
    }

    /// `ftCo_RunBrake_Anim`: the brake's animation holds while the fighter is still fast.
    #[doc(alias = "ftCo_RunBrake_Anim")]
    pub fn co_run_brake_anim(&mut self, gobj: GObjId) {
        let max = reg(self.common().f32(0x42C));
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[1] != 0 {
            let abs = |v: f32| if v < 0.0 { fp::fneg(reg(v)) } else { reg(v) };
            if fp.mv_i32(0) == 0 {
                if abs(fp.gr_vel) >= max {
                    self.set_anim_rate(gobj, 0.0);
                    self.game.fighter_mut(gobj).set_mv_i32(0, 1);
                }
            } else if abs(fp.gr_vel) <= max {
                self.set_anim_rate(gobj, 1.0);
                self.game.fighter_mut(gobj).cmd_vars[1] = 0;
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(4) != 0.0 {
            fp.set_mv_f32(4, fp::fsubs(reg(fp.mv_f32(4)), 1.0));
            if fp.mv_f32(4) < 0.0 {
                fp.set_mv_f32(4, 0.0);
            }
        }
        if !(self.anim_frames_remaining(gobj) && self.game.fighter(gobj).mv_f32(4) != 0.0) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Fall_Anim`: the fall animation leans with the fighter's drift.
    #[doc(alias = "ftCo_Fall_Anim")]
    pub fn co_fall_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).is_sandbag {
            return;
        }
        self.fall_lean(gobj, SM_FALL);
        // `ftCo_800CC988`.
        let lean = self.game.fighter(gobj).mv_f32(4);
        if lean != 0.0 {
            let f = self.game.fighter_id(gobj);
            let skel = self.game.fighters[f.0].x8AC_animSkeleton;
            jobj::anim_all(&mut self.game.objs, self.data, skel);
            if lean != 1.0 {
                self.blend_parts(f, TRANS_N, reg(lean), fp::fsubs(1.0, reg(lean)));
            } else {
                self.copy_parts(f, TRANS_N);
            }
        }
    }

    /// `ftCo_Fall_Anim_Inner`: eases the lean (`mv` x4) toward the fighter's drift and
    /// switches to the forward or backward fall animation when it leans.
    #[doc(alias = "ftCo_Fall_Anim_Inner")]
    pub fn fall_lean(&mut self, gobj: GObjId, smids: [i32; 3]) {
        let c = self.common();
        let (dead, ease) = (reg(c.f32(0x444)), reg(c.f32(0x448)));
        let fp = self.game.fighter_mut(gobj);
        let mut frac = fp::fdivs(reg(fp.self_vel.x), reg(fp.co_attrs.air_drift_max));
        if frac > 1.0 {
            frac = 1.0;
        } else if frac < fp::fneg(1.0) {
            frac = fp::fneg(1.0);
        }
        let abs = if frac < 0.0 { fp::fneg(frac) } else { frac };
        let (smid, target) = if abs > dead {
            let smid = if fp::fmuls(frac, reg(fp.facing_dir)) > 0.0 {
                smids[1]
            } else {
                smids[2]
            };
            (smid, fp::fdivs(fp::fsubs(abs, dead), fp::fsubs(1.0, dead)))
        } else {
            (smids[0], 0.0)
        };
        let x4 = reg(fp.mv_f32(4));
        fp.set_mv_f32(4, fp::fmadds(ease, fp::fsubs(target, x4), x4));
        if fp.mv_f32(4) != 0.0 && smid != fp.mv_i32(0) {
            unported_call("ftAnim_8006EDD0");
        }
    }

    /// `ftCo_Damage_Anim`: hitstun ends with the animation.
    #[doc(alias = "ftCo_Damage_Anim")]
    pub fn co_damage_anim(&mut self, gobj: GObjId) {
        self.damage_tick(gobj);
        let fp = self.game.fighter(gobj);
        if !self.anim_frames_remaining(gobj) && !fp.x221C_b6 {
            if fp.ground_or_air == GA_AIR {
                if fp.stamina_dead {
                    unported_call("ftCo_80090780");
                } else if !self.damage_tech(gobj) {
                    self.enter_fall(gobj);
                }
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftCo_DamageFly_Anim`: the tumble trail's timer, hitstun, then tumbling.
    #[doc(alias = "ftCo_DamageFly_Anim")]
    pub fn co_damage_fly_anim(&mut self, gobj: GObjId) {
        // `inlineD0`: the knockback smoke's timer.
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(8) != 0 {
            fp.set_mv_i32(8, fp.mv_i32(8).wrapping_sub(1));
            if fp.mv_i32(8) == 0 {
                unported_call("efAsync_Spawn");
            }
        }
        self.damage_tick(gobj);
        let fp = self.game.fighter(gobj);
        if !self.anim_frames_remaining(gobj) && !fp.x221C_b6 && !self.damage_tech(gobj) {
            unported_call("ftCo_80090780");
        }
    }

    /// `inlineC0` of `ftCo_Damage.c`: a tech input buffered in time (`mv` x14) turns the end
    /// of hitstun into the jump or attack it asks for.
    fn damage_tech(&mut self, gobj: GObjId) -> bool {
        let window = self.common().f32(0x1D0);
        let fp = self.game.fighter_mut(gobj);
        let x14 = fp.mv_f32(0x14);
        if x14 != 0.0 && x14 <= window {
            fp.input.pressed_buttons |= (1 << 10) | (1 << 11);
            unported_call("ftCo_800CB870");
        }
        false
    }

    /// `ftCo_8008F744`: hitstun (`mv` x0) counts down; at its end the fighter can act and its
    /// combo window opens.
    #[doc(alias = "ftCo_8008F744")]
    fn damage_tick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(0) > 0.0 {
            fp.set_mv_f32(0, fp::fsubs(reg(fp.mv_f32(0)), 1.0));
        }
        if fp.x221C_b6 && fp.mv_f32(0) <= 0.0 {
            let combo = self.common().i32(0x4CC);
            let fp = self.game.fighter_mut(gobj);
            fp.x221C_b6 = false;
            fp.x2098 = combo as u16;
            self.knockback_exit(gobj);
            if self.game.fighter(gobj).item_gobj.is_some() {
                // `ftCo_800C5240`, whether it holds the hammer.
                unported_call("itGetKind");
            }
        }
    }

    /// `ftCommon_8007F86C`: the character's own reaction to leaving hitstun
    /// (`ftData_OnKnockbackExit`). Unchecked: every character the saved calls have reacts,
    /// and no reaction is ported.
    pub fn knockback_exit(&mut self, gobj: GObjId) {
        // Captain Falcon, Samus, Game & Watch, the hands and the wireframes have none.
        if !matches!(self.game.fighter(gobj).kind, 2 | 13 | 24 | 27..=30) {
            unported_call("ftData_OnKnockbackExit");
        }
    }

    /// `ftCo_DeadDown_Anim`: the bottom blast zone's countdown to respawning.
    #[doc(alias = "ftCo_DeadDown_Anim")]
    pub fn co_dead_down_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, fp.mv_i32(0).wrapping_sub(1));
        if fp.mv_i32(0) == 0 {
            unported_call("ftCo_800BFD9C");
        }
    }

    /// `ftCo_Sleep_Anim`: a sleeping fighter (Zelda while Sheik plays) does nothing.
    #[doc(alias = "ftCo_Sleep_Anim")]
    pub fn co_sleep_anim(&mut self, _gobj: GObjId) {}
}
