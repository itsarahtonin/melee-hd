//! The ledge's options (`ftCo_CliffClimb.c`, `ftCo_CliffAttack.c`, `ftCo_CliffEscape.c`,
//! `ftCo_CliffJump.c`).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::checks::abs;
use super::ids::ms;
use super::ledge::{CLIFF_LEDGE_ID, CLIFF_X8};
use super::{PAD_A, PAD_B, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCommon_MotionVars_cliffjump`.
const CLIFF_JUMP_X0: usize = 0x0;

impl Running<'_> {
    /// A ledge option's state (`ftCo_Cliff_EnterState`), quick under `ftCommonData` x488
    /// percent, else slow.
    fn enter_cliff_state(&mut self, gobj: GObjId, quick: i32, slow: i32) {
        let pct = f64::from(self.common().i32(0x488));
        let msid = if reg(self.game.fighter(gobj).dmg.x1830_percent) < pct {
            quick
        } else {
            slow
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x1A6A = 32;
        fp.x221D_b7 = true;
    }

    /// A ledge attack with A or B, or the C-stick up, `ftCo_8009AE38`.
    #[doc(alias = "ftCo_8009AE38")]
    #[doc(alias = "ftCo_8009AEA4")]
    #[doc(alias = "ftCo_800DF6F8")]
    pub fn cliff_attack_check(&mut self, gobj: GObjId) -> bool {
        let t = self.common().f32(0x7F8);
        let fp = self.game.fighter(gobj);
        let c = &fp.input.cstick;
        if fp.input.pressed_buttons & (PAD_A | PAD_B) != 0 || (c[1].y < t && c[0].y >= t) {
            self.enter_cliff_state(gobj, ms::CLIFF_ATTACK_QUICK, ms::CLIFF_ATTACK_SLOW);
            self.game.fighter_mut(gobj).x221D_b5 = true;
            self.co_cliff_catch_phys(gobj);
            return true;
        }
        false
    }

    /// A ledge roll with a shoulder, or the C-stick forward, `ftCo_8009AFD4`.
    #[doc(alias = "ftCo_8009AFD4")]
    #[doc(alias = "ftCo_8009B040")]
    #[doc(alias = "ftCo_800DF72C")]
    pub fn cliff_escape_check(&mut self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x7FC));
        let single = self.game.vs_single_button();
        let fp = self.game.fighter(gobj);
        let facing = reg(fp.facing_dir);
        let c = &fp.input.cstick;
        let cstick =
            !single && fp::fmuls(facing, reg(c[1].x)) < t && fp::fmuls(facing, reg(c[0].x)) >= t;
        if fp.input.pressed_buttons & PAD_LR != 0 || cstick {
            self.enter_cliff_state(gobj, ms::CLIFF_ESCAPE_QUICK, ms::CLIFF_ESCAPE_SLOW);
            self.game.fighter_mut(gobj).x221D_b5 = true;
            self.co_cliff_catch_phys(gobj);
            return true;
        }
        false
    }

    /// A ledge jump, `ftCo_8009B170`.
    #[doc(alias = "ftCo_8009B170")]
    #[doc(alias = "ftCo_8009B1B8")]
    pub fn cliff_jump_check(&mut self, gobj: GObjId) -> bool {
        if self.jump_input(gobj) != 0 {
            self.enter_cliff_state(gobj, ms::CLIFF_JUMP_QUICK1, ms::CLIFF_JUMP_SLOW1);
            self.co_cliff_catch_phys(gobj);
            return true;
        }
        false
    }

    /// A ledge climb with the stick up or forward, or letting go with it down or back
    /// (`ftCo_8009AA0C`); the stick must have come back first (`mv` x8).
    #[doc(alias = "ftCo_8009AA0C")]
    #[doc(alias = "ftCo_800DF79C")]
    pub fn cliff_climb_check(&mut self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x494));
        let fp = self.game.fighter(gobj);
        let (s, c) = (&fp.input.lstick[0], &fp.input.cstick[0]);
        if abs(reg(s.x)) >= t || abs(reg(s.y)) >= t {
            let (x, angle) = (s.x, self.lstick_angle(gobj));
            return self.cliff_stick(gobj, true, x, angle);
        }
        if abs(reg(c.x)) >= t || abs(reg(c.y)) >= t {
            let (x, angle) = (c.x, self.cstick_angle(gobj));
            return self.cliff_stick(gobj, false, x, angle);
        }
        self.game.fighter_mut(gobj).set_mv_i32(CLIFF_X8, 1);
        false
    }

    /// `ftCo_8009AAFC`: up or forward climbs (only with the control stick); down or back lets
    /// go, starting the ledge cooldown.
    #[doc(alias = "ftCo_8009AAFC")]
    #[doc(alias = "ftCo_8009AB9C")]
    fn cliff_stick(&mut self, gobj: GObjId, lstick: bool, x: f32, angle: f64) -> bool {
        let up = reg(self.common().f32(0x20));
        let fp = self.game.fighter(gobj);
        let ready = fp.mv_i32(CLIFF_X8) != 0;
        if angle > up || (angle > fp::fneg(up) && fp::fmuls(reg(x), reg(fp.facing_dir)) >= 0.0) {
            if lstick && ready {
                // `ftCo_8009AB9C`.
                self.enter_cliff_state(gobj, ms::CLIFF_CLIMB_QUICK, ms::CLIFF_CLIMB_SLOW);
                self.game.fighter_mut(gobj).x221D_b5 = true;
                self.co_cliff_catch_phys(gobj);
                return true;
            }
            return false;
        }
        if ready {
            let cooldown = self.common().i32(0x498);
            self.game.fighter_mut(gobj).x2064_ledgeCooldown = cooldown;
            self.enter_fall(gobj);
            return true;
        }
        false
    }

    /// `ftCo_CliffClimb_Anim` (and the attack's and roll's): standing or falling when it
    /// plays out (`ftCommon_8007D92C`).
    #[doc(alias = "ftCo_CliffClimb_Anim")]
    #[doc(alias = "ftCo_CliffAttack_Anim")]
    #[doc(alias = "ftCo_CliffEscape_Anim")]
    pub fn co_cliff_climb_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// Falling in the air, else waiting, `ftCommon_8007D92C`.
    #[doc(alias = "ftCommon_8007D92C")]
    pub fn fall_or_wait(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.enter_fall(gobj);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_CliffClimb_Phys` (and the attack's and roll's): hanging from the ledge by the
    /// animation's offset until it rises above it and touches down; then the animation's
    /// motion.
    #[doc(alias = "ftCo_CliffClimb_Phys")]
    #[doc(alias = "ftCo_CliffAttack_Phys")]
    #[doc(alias = "ftCo_CliffEscape_Phys")]
    pub fn co_cliff_climb_phys(&mut self, gobj: GObjId) {
        if !self.is_airborne(gobj) {
            self.ground_friction_or_anim(gobj);
            return;
        }
        let fp = self.game.fighter(gobj);
        let ledge = fp.mv_i32(CLIFF_LEDGE_ID);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        if !g.line_live(ledge) {
            self.enter_fall(gobj);
            return;
        }
        let v = if fp.facing_dir > 0.0 {
            g.data_floor_left(ledge)
        } else {
            g.data_floor_right(ledge)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.cur_pos.x = mem(fp::fmadds(
            reg(fp.x68C_transNPos.z),
            reg(fp.facing_dir),
            reg(v.x),
        ));
        fp.cur_pos.y = mem(fp::fadds(reg(v.y), reg(fp.x68C_transNPos.y)));
        if fp.ground_or_air == GA_AIR && fp.x68C_transNPos.z >= 0.0 && fp.x68C_transNPos.y >= 0.0 {
            fp.coll_data.floor.index = ledge;
            self.land(gobj);
        }
    }

    /// `ftCo_CliffClimb_Coll` (and the attack's, roll's and first jump's): touching down in
    /// the air (`ftCo_8009AE14`) or stopping at a ceiling; on the ground, `ft_80084104`.
    #[doc(alias = "ftCo_CliffClimb_Coll")]
    #[doc(alias = "ftCo_CliffAttack_Coll")]
    #[doc(alias = "ftCo_CliffEscape_Coll")]
    #[doc(alias = "ftCo_CliffJump1_Coll")]
    #[doc(alias = "ftCo_8009AE14")]
    pub fn co_cliff_climb_coll(&mut self, gobj: GObjId) {
        if !self.is_airborne(gobj) {
            self.ground_coll(gobj);
            return;
        }
        if self.air_coll_land_low(gobj) {
            self.land(gobj);
        } else {
            self.stop_at_ceiling_check(gobj);
        }
    }

    /// `ftCo_CliffJump1_Anim`: the jump's rise once the climb plays out.
    #[doc(alias = "ftCo_CliffJump1_Anim")]
    pub fn co_cliff_jump1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_cliff_jump2(gobj);
        }
    }

    /// The ledge jump's rise, `ftCo_8009B2F8`.
    #[doc(alias = "ftCo_8009B2F8")]
    pub fn enter_cliff_jump2(&mut self, gobj: GObjId) {
        let quick = self.game.fighter(gobj).motion_id == ms::CLIFF_JUMP_QUICK1;
        let msid = if quick {
            ms::CLIFF_JUMP_QUICK2
        } else {
            ms::CLIFF_JUMP_SLOW2
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(CLIFF_JUMP_X0, 0);
        let (h, v) = (
            fp.co_attrs.ledge_jump_horizontal_velocity,
            fp.co_attrs.ledge_jump_vertical_velocity,
        );
        fp.self_vel.x = mem(fp::fmadds(reg(fp.facing_dir), reg(h), reg(fp.self_vel.x)));
        fp.self_vel.y = v;
    }

    /// `ftCo_CliffJump2_Anim`: falling once the rise plays out.
    #[doc(alias = "ftCo_CliffJump2_Anim")]
    pub fn co_cliff_jump2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_CliffJump2_Phys`: the first frame keeps the jump's speed whole.
    #[doc(alias = "ftCo_CliffJump2_Phys")]
    pub fn co_cliff_jump2_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(CLIFF_JUMP_X0) == 0 {
            fp.set_mv_i32(CLIFF_JUMP_X0, 1);
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_CliffJump2_Coll`: `ft_800835B0`, landing normally.
    #[doc(alias = "ftCo_CliffJump2_Coll")]
    pub fn co_cliff_jump2_coll(&mut self, gobj: GObjId) {
        self.co_jump_coll(gobj);
    }
}
