//! The common action states' physics callbacks, run by `Fighter_procUpdate` before it applies
//! the fighter's velocities.

use gekko_fp as fp;

use super::super::super::state::Running;
use super::super::common::GA_AIR;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math::{self, sqrt_reg};

/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;

/// `ftCo_MS_DamageFlyRoll`.
const MS_DAMAGE_FLY_ROLL: i32 = 0x5B;

impl Running<'_> {
    /// Ground friction alone: `ftCo_Landing_Phys`, `ftCo_KneeBend_Phys` and the other states
    /// that slide to a stop.
    #[doc(alias = "ftCo_Landing_Phys")]
    #[doc(alias = "ftCo_LandingAir_Phys")]
    #[doc(alias = "ftCo_KneeBend_Phys")]
    #[doc(alias = "ftCo_AttackS3_Phys")]
    #[doc(alias = "ftCo_AttackHi3_Phys")]
    #[doc(alias = "ftCo_AttackLw3_Phys")]
    #[doc(alias = "ftCo_AttackLw4_Phys")]
    #[doc(alias = "ftCo_Turn_Phys")]
    #[doc(alias = "ftCo_Squat_Phys")]
    #[doc(alias = "ftCo_SquatRv_Phys")]
    #[doc(alias = "ftCo_AttackHi4_Phys")]
    #[doc(alias = "ftCo_EscapeN_Phys")]
    #[doc(alias = "ftFx_SpecialLwStart_Phys")]
    #[doc(alias = "ftCo_DownBound_Phys")]
    #[doc(alias = "ftCo_DownWait_Phys")]
    #[doc(alias = "ftCo_DownStand_Phys")]
    #[doc(alias = "ftCo_DownAttack_Phys")]
    #[doc(alias = "ftCo_DownSpot_Phys")]
    #[doc(alias = "ftCo_Passive_Phys")]
    #[doc(alias = "ftCo_GuardOff_Phys")]
    pub fn co_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCo_Wait_Phys`: friction, and the shield's hit position follows again.
    #[doc(alias = "ftCo_Wait_Phys")]
    #[doc(alias = "ftCo_GuardSetOff_Phys")]
    #[doc(alias = "ftCo_SquatWait_Phys")]
    pub fn co_wait_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        // `ftColl_8007AEE0`.
        self.game.fighter_mut(gobj).shield_hit.skip_update_pos = false;
    }

    /// Gravity and drift: `ftCo_Fall_Phys`, `ftCo_AttackAir_Phys`, `ftCo_JumpAerial_Phys`.
    #[doc(alias = "ftCo_Fall_Phys")]
    #[doc(alias = "ftCo_ItemScrewAir_Phys")]
    #[doc(alias = "ftCo_DamageFall_Phys")]
    #[doc(alias = "ftCo_AttackAir_Phys")]
    #[doc(alias = "ftCo_JumpAerial_Phys")]
    #[doc(alias = "ftCo_FallAerial_Phys")]
    #[doc(alias = "ftCo_Pass_Phys")]
    #[doc(alias = "ftCo_MissFoot_Phys")]
    pub fn co_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftCo_Jump_Phys`: the first frame keeps the jump's velocity whole.
    #[doc(alias = "ftCo_Jump_Phys")]
    pub fn co_jump_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(4) == 0 {
            fp.set_mv_i32(4, 1);
            return;
        }
        self.air_physics(gobj);
    }

    /// `ftCo_Damage_Phys`: in hitstun the fighter can't drift.
    #[doc(alias = "ftCo_Damage_Phys")]
    #[doc(alias = "ftCo_DownDamage_Phys")]
    pub fn co_damage_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if !fp.x221C_b6 {
                self.air_physics(gobj);
            } else {
                self.air_physics_no_drift(gobj);
            }
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftCo_DamageFly_Phys`: as `ftCo_Damage_Phys`; a tumble's roll follows the trajectory,
    /// and a thrown fighter's hitbox goes once it slows.
    #[doc(alias = "ftCo_DamageFly_Phys")]
    pub fn co_damage_fly_phys(&mut self, gobj: GObjId) {
        self.co_damage_phys(gobj);
        if self.game.fighter(gobj).motion_id == MS_DAMAGE_FLY_ROLL {
            // `doFlyRoll`.
            let f = self.game.fighter_id(gobj);
            let fp = &self.game.fighters[f.0];
            let x = mem(fp::fadds(reg(fp.self_vel.x), reg(fp.x8c_kb_vel.x)));
            let y = mem(fp::fadds(reg(fp.self_vel.y), reg(fp.x8c_kb_vel.y)));
            let rot = mem(fp::fmuls(reg(fp.facing_dir), reg(math::atan2(x, y))));
            let part = self.bone_index(fp, X_ROT_N);
            self.game.part_set_rot_x(f, part, rot);
        }
        let slow = reg(self.common().f32(0x1C8));
        let fp = self.game.fighter(gobj);
        if u32::from_be_bytes(fp.x1064_thrownHitbox.x134) != 0 {
            let k = fp.x8c_kb_vel;
            let sq = |v: f32| fp::fmuls(reg(v), reg(v));
            let speed = sqrt_reg(fp::fadds(sq(k.z), fp::fadds(sq(k.x), sq(k.y))));
            if speed < slow {
                let f = self.game.fighter_id(gobj);
                self.clear_hitboxes(f);
            }
        }
    }
}

impl Running<'_> {
    /// `ftCo_Damage_OnEveryHitlag`: smash directional influence, the stick nudging the
    /// fighter during hitlag.
    #[doc(alias = "ftCo_Damage_OnEveryHitlag")]
    pub fn co_damage_hitlag(&mut self, gobj: GObjId) {
        let c = self.common();
        let (min, window, scale) = (reg(c.f32(0x4B0)), c.i32(0x4B4), reg(c.f32(0x4B8)));
        let fp = self.game.fighter(gobj);
        let (sx, sy) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        if fp.allow_sdi
            && fp::fadds(fp::fmuls(sx, sx), fp::fmuls(sy, sy)) >= fp::fmuls(min, min)
            && (i32::from(fp.active_timer.lstick.x) < window
                || i32::from(fp.active_timer.lstick.y) < window
                || self.game.rules.ucf && self.ucf_sdi(gobj, min))
        {
            let fp = self.game.fighter_mut(gobj);
            let (dx, dy) = (fp::fmuls(sx, scale), fp::fmuls(sy, scale));
            fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), dx));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), dy));
            fp.active_timer.lstick.x = 254;
            fp.active_timer.lstick.y = 254;
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            self.game.add_sdi_distance(slot, sub, dx, dy);
        }
    }
}

impl Running<'_> {
    /// Ground friction or the animation's motion: `ftCo_Attack11_Phys`, `ftCo_AttackS4_Phys`.
    #[doc(alias = "ftCo_Attack11_Phys")]
    #[doc(alias = "ftCo_AttackS4_Phys")]
    #[doc(alias = "ftCo_Down_Phys")]
    #[doc(alias = "ftCo_PassiveStand_Phys")]
    pub fn co_attack_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// `ftCo_AttackDash_Phys`: the dash attack slides with its own friction.
    #[doc(alias = "ftCo_AttackDash_Phys")]
    pub fn co_attack_dash_phys(&mut self, gobj: GObjId) {
        let mul = reg(self.common().f32(0x50));
        let fp = self.game.fighter(gobj);
        let friction = fp::fmuls(mul, reg(fp.co_attrs.ground_friction));
        let facing = reg(fp.facing_dir);
        self.ground_anim_motion(gobj, friction, facing);
    }

    /// `ftCo_Guard_Phys`, `ftCo_GuardOn_Phys`, `ftCo_GuardReflect_Phys`: friction, and the
    /// shield's and reflector's hit positions follow again.
    #[doc(alias = "ftCo_Guard_Phys")]
    #[doc(alias = "ftCo_GuardOn_Phys")]
    #[doc(alias = "ftCo_GuardReflect_Phys")]
    pub fn co_guard_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        let fp = self.game.fighter_mut(gobj);
        // `ftColl_8007AEE0` and `ftColl_8007AEF8`.
        fp.shield_hit.skip_update_pos = false;
        fp.reflect_hit.skip_update_pos = false;
    }

    /// The reflector's bubble follows the fighter again, `ftColl_8007AEF8`.
    #[doc(alias = "ftColl_8007AEF8")]
    pub fn reflect_follow(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).reflect_hit.skip_update_pos = false;
    }

    /// `ftCo_EscapeAir_Phys`: an air dodge's momentum decays until it ends.
    #[doc(alias = "ftCo_EscapeAir_Phys")]
    pub fn co_escape_air_phys(&mut self, gobj: GObjId) {
        let decay = reg(self.common().f32(0x33C));
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 0 {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), decay));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), decay));
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_CatchWait_Phys`: holding a grab slides with the grab's friction.
    #[doc(alias = "ftCo_CatchWait_Phys")]
    pub fn co_catch_wait_phys(&mut self, gobj: GObjId) {
        let mul = reg(self.common().f32(0x64));
        let fp = self.game.fighter_mut(gobj);
        let friction = fp::fmuls(mul, reg(fp.co_attrs.ground_friction));
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftCo_ThrowB_Phys`, `ftCo_ThrowHi_Phys`, `ftCo_ThrowLw_Phys`: a throw in the air keeps
    /// its set velocity, or the animation's.
    #[doc(alias = "ftCo_ThrowB_Phys")]
    #[doc(alias = "ftCo_ThrowHi_Phys")]
    #[doc(alias = "ftCo_ThrowLw_Phys")]
    pub fn co_throw_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.mv_i32(4) != 0 {
                fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(0x14)), reg(fp.facing_dir)));
                fp.self_vel.y = fp.mv_f32(0x10);
            } else {
                self.air_anim_motion(gobj);
            }
        } else {
            self.ground_anim_motion_thrown(gobj);
        }
    }

    /// `ftCo_ThrownLw_Phys`: nothing.
    #[doc(alias = "ftCo_ThrownLw_Phys")]
    #[doc(alias = "ftCo_Ottotto_Phys")]
    #[doc(alias = "ftCo_OttottoWait_Phys")]
    #[doc(alias = "ftCo_StopWall_Phys")]
    #[doc(alias = "ftCo_StopCeil_Phys")]
    pub fn co_no_phys(&mut self, _gobj: GObjId) {}
}
