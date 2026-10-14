//! The bombs (`ftsamusspeciallw1.c`): the morph ball drops a bomb when the script says, rolls
//! on the ground or drifts in the air, intangible while the script asks. And the bomb jump
//! (`ftsamusspeciallw0.c`): her own bomb's blast throws her up and away from it in the jump
//! state (`ms::SPECIAL_LW` and its aerial twin).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::samusbomb;
use super::super::super::state::Running;
use super::super::super::structs::{Vec2, ftCollisionBox};
use super::super::common::GA_GROUND;
use super::super::motion::flags;
use super::{PART_TOP_N, PART_X_ROT_N, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule::{self, HURT_ENABLED, HURT_INTANGIBLE};
use crate::vec::Vec3;

/// `ftCommon_GroundAirColl_MF | Ft_MF_KeepColAnimHitStatus | Ft_MF_SkipHit | Ft_MF_SkipModel`:
/// what the bomb's switches between the ground and the air keep.
const KEEP: u32 =
    0x0C4C_5080 | flags::KEEP_COL_ANIM_HIT_STATUS | flags::SKIP_HIT | flags::SKIP_MODEL;
/// `JOBJ_HIDDEN`, the flags the bomb jump's switches pass.
const KEEP_JUMP: u32 = 1 << 4;
/// `HurtHeight_Mid`.
const HURT_HEIGHT_MID: i32 = 1;
/// `ftCo_MS_SquatWait`, a state the bomb starts from mid-animation.
const MS_SQUAT_WAIT: i32 = 40;
/// `M_PI / 2` as a float.
const HALF_PI: f64 = 1.570_796_370_506_286_6;

impl Running<'_> {
    /// The bomb jump's angle, `ftSs_Init_80128AC8`: straight up less the attributes' `x4`
    /// times how far she is from the blast's `x` across its reach, held within one.
    #[doc(alias = "ftSs_Init_80128AC8")]
    // The original's two compares, which NaN passes through (`clamp` would panic on it).
    #[allow(clippy::manual_clamp)]
    pub fn samus_bomb_jump_angle(&self, gobj: GObjId, x: f32, reach: f32) -> f64 {
        let f = self.game.fighter_id(gobj);
        let tilt = reg(self.samus_attr_f32(f, attr::BOMB_JUMP_ANGLE));
        let fp = &self.game.fighters[f.0];
        let mut value = fp::fdivs(fp::fsubs(reg(fp.cur_pos.x), reg(x)), reg(reach));
        if value >= 1.0 {
            value = 1.0;
        }
        if value <= -1.0 {
            value = -1.0;
        }
        fp::fmadds(fp::fneg(tilt), value, HALF_PI)
    }

    /// The bomb jump, `ftSs_Init_80128944`: unless she's intangible, in a state its blast
    /// can launch her from (not hit by a strong attack, or one with the move id 20), she
    /// jumps at the blast's angle; mid-roll (`x5F4_arr[0].idx` 2) from the attributes' `x0`.
    #[doc(alias = "ftSs_Init_80128944")]
    pub fn samus_bomb_jump(&mut self, gobj: GObjId, x: f32, reach: f32) {
        let angle = self.samus_bomb_jump_angle(gobj, x, reach);
        let f = self.game.fighter_id(gobj);
        if self.hit_status(f) != 0 {
            return;
        }
        let fp = &self.game.fighters[f.0];
        if !matches!(fp.x2070[1] >> 4, 0 | 2 | 3 | 4) {
            return;
        }
        if fp.x2070[3] != 0x14 && fp.x2070[1] & 0x04 != 0 {
            return;
        }
        let start = if fp.x5F4_arr[0].idx == 2 {
            self.samus_attr_f32(f, attr::BOMB_JUMP_FRAME)
        } else {
            0.0
        };
        self.samus_bomb_jump_start(gobj, angle, start, 1.0);
    }

    /// Whether the bomb's box touches any of her hurtboxes, `ftSs_Init_80128A1C`
    /// (`lbColl_80008248`, at the bomb's `scale`).
    #[doc(alias = "ftSs_Init_80128A1C")]
    pub fn samus_bomb_touches(&mut self, gobj: GObjId, bomb: GObjId, scale: f32) -> bool {
        let f = self.game.fighter_id(gobj);
        let mtx = self.own_matrix(f);
        let mut hit = super::super::super::it::item(self.game, bomb).x5D4_hitboxes[0]
            .hit
            .clone();
        let mut touch = false;
        for i in 0..self.game.fighters[f.0].hurt_capsules_len as usize {
            let fp = &mut self.game.fighters[f.0];
            let (owner_scale, z) = (fp.x34_scale.y, fp.cur_pos.z);
            if hitcapsule::touches_hurtbox(
                &mut self.game.objs,
                self.data,
                &mut hit,
                &mut fp.hurt_capsules[i].capsule,
                mtx.as_ref(),
                scale,
                owner_scale,
                z,
            ) {
                touch = true;
                break;
            }
        }
        let h = &mut super::super::super::it::item_mut(self.game, bomb).x5D4_hitboxes[0].hit;
        h.hurt_coll_pos = hit.hurt_coll_pos;
        h.coll_distance = hit.coll_distance;
        touch
    }

    /// The bomb jump's start, `ftSs_Init_80128B1C`: the damage hooks cleared, launched at
    /// `angle` by the attributes' `x8` (her drift held within the air's times `x10`), the
    /// script's flags and the intangibility off, airborne, and the jump state from `start` at
    /// `rate`.
    #[doc(alias = "ftSs_Init_80128B1C")]
    pub fn samus_bomb_jump_start(&mut self, gobj: GObjId, angle: f64, start: f32, rate: f32) {
        self.damage_hooks(gobj);
        let f = self.game.fighter_id(gobj);
        let speed = reg(self.samus_attr_f32(f, attr::BOMB_JUMP_SPEED));
        let drift = reg(self.samus_attr_f32(f, attr::BOMB_JUMP_DRIFT));
        let c = reg(crate::math::cos(mem(angle)));
        let s = reg(crate::math::sin(mem(angle)));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(speed, c));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        let max = fp::fmuls(reg(fp.co_attrs.air_drift_max), drift);
        Self::clamp_self_vel_x(fp, max);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.set_mv_i32(mv::INTANGIBLE, 0);
        if fp.ground_or_air == GA_GROUND {
            self.go_airborne(f);
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, start, rate, 0.0, None);
        self.anim_update(gobj);
    }

    /// The jump's intangible ball when the script turns it on (`cmd_vars[0]`) and off.
    fn samus_ball_intangibility(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (on, was) = (fp.cmd_vars[0] != 0, fp.mv_i32(mv::INTANGIBLE) != 0);
        if on && !was {
            self.samus_ball_intangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(mv::INTANGIBLE, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 && fp.mv_i32(mv::INTANGIBLE) != 0 {
            self.samus_ball_tangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(mv::INTANGIBLE, 0);
        }
    }

    /// `ftSs_SpecialLw_Anim`: the ball's intangibility, and waiting at the end.
    #[doc(alias = "ftSs_SpecialLw_Anim")]
    pub fn samus_special_lw_anim(&mut self, gobj: GObjId) {
        self.samus_ball_intangibility(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialAirLw_Anim`: the ball's intangibility, and falling at the end.
    #[doc(alias = "ftSs_SpecialAirLw_Anim")]
    pub fn samus_special_air_lw_anim(&mut self, gobj: GObjId) {
        self.samus_ball_intangibility(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSs_SpecialLw_IASA`: once the script allows it (`cmd_vars[1]`), the stick down past
    /// the attributes' `x14` crouches; then the ground's actions.
    #[doc(alias = "ftSs_SpecialLw_IASA")]
    pub fn samus_special_lw_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let threshold = self.samus_attr_f32(f, attr::BOMB_JUMP_STICK_Y);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 0 && fp.input.lstick[0].y < threshold {
            self.game.fighters[f.0].cmd_vars[1] = 0;
            self.enter_squat_wait_settled(gobj);
            return;
        }
        let _ = self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.catch_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.escape_n_check(gobj);
    }

    /// `ftSs_SpecialAirLw_IASA`: the fall's actions.
    #[doc(alias = "ftSs_SpecialAirLw_IASA")]
    pub fn samus_special_air_lw_iasa(&mut self, gobj: GObjId) {
        self.co_fall_iasa(gobj);
    }

    /// `ftSs_SpecialLw_Phys`: rolling with the stick at the attributes' `xC` of her walk
    /// while the ball is out, else friction.
    #[doc(alias = "ftSs_SpecialLw_Phys")]
    pub fn samus_special_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            self.ground_friction(gobj);
            return;
        }
        let mul = reg(self.samus_attr_f32(f, attr::BOMB_JUMP_WALK));
        let fp = &mut self.game.fighters[f.0];
        let accel = fp::fmuls(reg(fp.co_attrs.walk_accel_mul), mul);
        let max = fp::fmuls(reg(fp.co_attrs.walk_max_vel), mul);
        Self::ground_accel_to_stick(fp, 0.0, accel, max);
        self.self_movement_from_ground(gobj);
    }

    /// `ftSs_SpecialAirLw_Phys`: falling, drifting at the attributes' `x10` of her jump's.
    #[doc(alias = "ftSs_SpecialAirLw_Phys")]
    pub fn samus_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.samus_attr_f32(f, attr::BOMB_JUMP_DRIFT));
        let fp = &mut self.game.fighters[f.0];
        Self::fall_basic(fp);
        let a = &fp.co_attrs;
        let accel = fp::fmuls(reg(a.ground_to_air_jump_momentum_multiplier), mul);
        let max = fp::fmuls(reg(a.jump_h_max_velocity), mul);
        Self::drift_simple(fp, 0.0, accel, max);
    }

    /// The morph ball's box (the attributes' `x84`).
    pub(super) fn samus_ball_box(&self, gobj: GObjId) -> ftCollisionBox {
        let f = self.game.fighter_id(gobj);
        let v = |i: usize| self.samus_attr_f32(f, attr::LW_BOX + 4 * i);
        ftCollisionBox {
            top: v(0),
            bottom: v(1),
            left: Vec2 { x: v(2), y: v(3) },
            right: Vec2 { x: v(4), y: v(5) },
        }
    }

    /// `ftSs_SpecialLw_Coll`: off the ground, the jump goes on in the air.
    #[doc(alias = "ftSs_SpecialLw_Coll")]
    pub fn samus_special_lw_coll(&mut self, gobj: GObjId) {
        let on = if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.ground_coll_box(gobj, &b)
        } else {
            self.ground_coll_slide(gobj)
        };
        if !on {
            self.samus_bomb_jump_to_air(gobj);
        }
    }

    /// `ftSs_SpecialAirLw_Coll`: landing, the jump goes on on the ground.
    #[doc(alias = "ftSs_SpecialAirLw_Coll")]
    pub fn samus_special_air_lw_coll(&mut self, gobj: GObjId) {
        let landed = if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.air_coll_box(gobj, &b)
        } else {
            self.air_coll_land(gobj)
        };
        if landed {
            self.samus_bomb_jump_to_ground(gobj);
        }
    }

    /// The bomb jump off the ground, `ftSs_SpecialLw_80129048`.
    #[doc(alias = "ftSs_SpecialLw_80129048")]
    pub fn samus_bomb_jump_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let fp = &self.game.fighters[f.0];
        let (frame, rate) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, KEEP_JUMP, frame, rate, 0.0, None);
    }

    /// The bomb jump landing, `ftSs_SpecialLw_801290A4`.
    #[doc(alias = "ftSs_SpecialLw_801290A4")]
    pub fn samus_bomb_jump_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        let fp = self.game.fighter(gobj);
        let (frame, rate) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, ms::SPECIAL_LW, KEEP_JUMP, frame, rate, 0.0, None);
    }

    /// The bomb's drop (accessory 4), `ftSs_SpecialLw_8012ADF0`: once the script's throw flag
    /// is up, a bomb at her top joint plus the attributes' `x74` (ahead by her facing), and
    /// no more accessory.
    #[doc(alias = "ftSs_SpecialLw_8012ADF0")]
    pub fn samus_bomb_drop(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `ftCheckThrowB0`.
        if !self.game.fighters[f.0].throw_flag(0) {
            return;
        }
        self.game.fighters[f.0].set_throw_flag(0, false);
        let top = self.bone_joint(gobj, PART_TOP_N);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, top, None);
        let o = |i: usize| reg(self.samus_attr_f32(f, attr::LW_BOMB_OFFSET + 4 * i));
        let (ox, oy, oz) = (o(0), o(1), o(2));
        let facing = self.game.fighters[f.0].facing_dir;
        pos.x = mem(fp::fmadds(ox, reg(facing), reg(pos.x)));
        pos.y = mem(fp::fadds(reg(pos.y), oy));
        pos.z = mem(fp::fadds(reg(pos.z), oz));
        samusbomb::spawn(self, gobj, pos, facing);
        self.game.fighters[f.0].accessory4_cb = None;
    }

    /// The morph ball's intangible hurtbox, `ftSs_SpecialLw_8012AEBC`: every hurtbox
    /// intangible, then the first one a ball of radius 3 on her X rotation joint, enabled
    /// and not grabbable (`ftColl_HurtboxInit`).
    #[doc(alias = "ftSs_SpecialLw_8012AEBC")]
    #[doc(alias = "ftColl_HurtboxInit")]
    pub fn samus_ball_intangible(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_hurtbox_state(f, HURT_INTANGIBLE);
        let bone = self.bone_joint(gobj, PART_X_ROT_N);
        let fp = &mut self.game.fighters[f.0];
        let h = &mut fp.hurt_capsules[0];
        h.capsule.bone_idx = PART_X_ROT_N;
        h.height = HURT_HEIGHT_MID;
        h.is_grabbable = 0;
        h.capsule.state = HURT_ENABLED;
        h.capsule.bone = bone;
        h.capsule.a_offset = Vec3::default();
        h.capsule.b_offset = Vec3::default();
        h.capsule.scale = 3.0;
        fp.x221A_b6 = true;
    }

    /// The ball's hurtboxes back, `ftSs_SpecialLw_8012AF38`.
    #[doc(alias = "ftSs_SpecialLw_8012AF38")]
    pub fn samus_ball_tangible(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_hurtbox_state(f, HURT_ENABLED);
    }

    /// The bomb's start (`ftSamus_SpecialLw_StartAction_inner`): the script's variables and
    /// throw flag cleared, not intangible, bombing at once from frame 3, the drop hooked.
    fn samus_bomb_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_throw_flag(0, false);
        fp.set_mv_i32(mv::INTANGIBLE, 0);
        if fp.cur_anim_frame == 3.0 {
            fp.cmd_vars[1] = 1;
        }
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_8012ADF0));
    }

    /// `ftSs_SpecialLw_Enter`: her ground speed scaled by the attributes' `x6C`; from a
    /// crouch the ball starts at frame 3 and hops at once.
    #[doc(alias = "ftSs_SpecialLw_Enter")]
    pub fn samus_special_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.samus_attr_f32(f, attr::LW_GR_VEL_MUL));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), mul));
        if fp.motion_id == MS_SQUAT_WAIT {
            self.change_motion_state(gobj, ms::SPECIAL_LW_BOMB, 0, 3.0, 1.0, 0.0, None);
            self.samus_bomb_start(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 2;
            self.samus_bomb_hop(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::SPECIAL_LW_BOMB, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.samus_bomb_start(gobj);
    }

    /// `ftSs_SpecialAirLw_Enter`: her air speed scaled by the attributes' `x70`, rising at
    /// `x58`.
    #[doc(alias = "ftSs_SpecialAirLw_Enter")]
    pub fn samus_special_air_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.samus_attr_f32(f, attr::LW_AIR_VEL_MUL));
        let rise = self.samus_attr_f32(f, attr::LW_AIR_VEL_Y);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), mul));
        fp.self_vel.y = rise;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_BOMB, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.samus_bomb_start(gobj);
    }

    /// The ball's intangibility as the script turns it on and off (`checkStateVar1`).
    fn samus_bomb_intangibility(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.mv_i32(mv::INTANGIBLE) == 0 {
            self.samus_ball_intangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(mv::INTANGIBLE, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 && fp.mv_i32(mv::INTANGIBLE) != 0 {
            let f = self.game.fighter_id(gobj);
            self.set_hurtbox_state(f, HURT_ENABLED);
            self.game.fighter_mut(gobj).set_mv_i32(mv::INTANGIBLE, 0);
        }
    }

    /// `ftSs_SpecialLwBomb_Anim`: the hop when the script says (`cmd_vars[1]` 1); else the
    /// ball's intangibility, and waiting at the end.
    #[doc(alias = "ftSs_SpecialLwBomb_Anim")]
    pub fn samus_special_lw_bomb_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[1] == 1 {
            self.game.fighter_mut(gobj).cmd_vars[1] = 2;
            self.samus_bomb_hop(gobj);
            return;
        }
        self.samus_bomb_intangibility(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialAirLwBomb_Anim`: the ball's intangibility, and falling at the end.
    #[doc(alias = "ftSs_SpecialAirLwBomb_Anim")]
    pub fn samus_special_air_lw_bomb_anim(&mut self, gobj: GObjId) {
        self.samus_bomb_intangibility(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSs_SpecialLwBomb_IASA`: once the script allows it (`cmd_vars[2]`), the stick down
    /// past the attributes' `x80` crouches.
    #[doc(alias = "ftSs_SpecialLwBomb_IASA")]
    pub fn samus_special_lw_bomb_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let threshold = self.samus_attr_f32(f, attr::LW_STICK_Y);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[2] != 0 && fp.input.lstick[0].y < threshold {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            self.enter_squat_wait_settled(gobj);
        }
    }

    /// `ftSs_SpecialLwBomb_Phys`: rolling with the stick (the attributes' `x64` of her walk's
    /// acceleration, `x5C` of its speed) while the ball is out, else friction.
    #[doc(alias = "ftSs_SpecialLwBomb_Phys")]
    pub fn samus_special_lw_bomb_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            self.ground_friction(gobj);
            return;
        }
        let accel_mul = reg(self.samus_attr_f32(f, attr::LW_WALK_ACCEL));
        let max_mul = reg(self.samus_attr_f32(f, attr::LW_WALK_MAX));
        let fp = &mut self.game.fighters[f.0];
        let accel = fp::fmuls(reg(fp.co_attrs.walk_accel_mul), accel_mul);
        let max = fp::fmuls(reg(fp.co_attrs.walk_max_vel), max_mul);
        Self::ground_accel_to_stick(fp, 0.0, accel, max);
        self.self_movement_from_ground(gobj);
    }

    /// `ftSs_SpecialAirLwBomb_Phys`: falling, drifting at the attributes' `x68` and `x60` of
    /// the air's, never braking.
    #[doc(alias = "ftSs_SpecialAirLwBomb_Phys")]
    pub fn samus_special_air_lw_bomb_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let accel_mul = reg(self.samus_attr_f32(f, attr::LW_DRIFT_ACCEL));
        let max_mul = reg(self.samus_attr_f32(f, attr::LW_DRIFT_MAX));
        let fp = &mut self.game.fighters[f.0];
        Self::fall_basic(fp);
        let a = &fp.co_attrs;
        let accel = fp::fmuls(reg(a.air_drift_stick_mul), accel_mul);
        let max = fp::fmuls(reg(a.air_drift_max), max_mul);
        Self::drift_simple_no_friction(fp, 0.0, accel, max);
    }

    /// `ftSs_SpecialLwBomb_Coll`: off the ground, the bomb goes on in the air.
    #[doc(alias = "ftSs_SpecialLwBomb_Coll")]
    pub fn samus_special_lw_bomb_coll(&mut self, gobj: GObjId) {
        let on = if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.ground_coll_box(gobj, &b)
        } else {
            self.ground_coll_slide(gobj)
        };
        if !on {
            self.samus_bomb_to_air(gobj);
        }
    }

    /// `ftSs_SpecialAirLwBomb_Coll`: landing, the bomb goes on on the ground.
    #[doc(alias = "ftSs_SpecialAirLwBomb_Coll")]
    pub fn samus_special_air_lw_bomb_coll(&mut self, gobj: GObjId) {
        let landed = if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.air_coll_box(gobj, &b)
        } else {
            self.air_coll_land(gobj)
        };
        if landed {
            self.samus_bomb_to_ground(gobj);
        }
    }

    /// The script's hop done, not intangible, the drop hooked (`ftSamus_UnkSetStateAndCb`).
    fn samus_bomb_switched(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 2;
        fp.set_mv_i32(mv::INTANGIBLE, 0);
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_8012ADF0));
    }

    /// The bomb off the ground, `ftSs_SpecialLw_8012B570`.
    #[doc(alias = "ftSs_SpecialLw_8012B570")]
    pub fn samus_bomb_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_BOMB, KEEP);
        self.samus_bomb_switched(gobj);
    }

    /// The ball's hop, `ftSs_SpecialLw_8012B5F0`: rising at the attributes' `x54`, into the
    /// aerial bomb, the drop hooked.
    #[doc(alias = "ftSs_SpecialLw_8012B5F0")]
    pub fn samus_bomb_hop(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let rise = self.samus_attr_f32(f, attr::LW_VEL_Y);
        self.game.fighters[f.0].self_vel.y = rise;
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_BOMB, KEEP);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_8012ADF0));
    }

    /// The bomb landing, `ftSs_SpecialLw_8012B668`.
    #[doc(alias = "ftSs_SpecialLw_8012B668")]
    pub fn samus_bomb_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_BOMB, KEEP);
        self.samus_bomb_switched(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_ANIM => g.samus_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.samus_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA => g.samus_special_lw_iasa(gobj),
        addr::SPECIAL_AIR_LW_IASA => g.samus_special_air_lw_iasa(gobj),
        addr::SPECIAL_LW_PHYS => g.samus_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.samus_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.samus_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.samus_special_air_lw_coll(gobj),
        addr::SPECIAL_LW_80129048 => g.samus_bomb_jump_to_air(gobj),
        addr::SPECIAL_LW_801290A4 => g.samus_bomb_jump_to_ground(gobj),
        addr::SPECIAL_LW_8012ADF0 => g.samus_bomb_drop(gobj),
        addr::SPECIAL_LW_8012AEBC => g.samus_ball_intangible(gobj),
        addr::SPECIAL_LW_8012AF38 => g.samus_ball_tangible(gobj),
        addr::SPECIAL_LW_ENTER => g.samus_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.samus_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_BOMB_ANIM => g.samus_special_lw_bomb_anim(gobj),
        addr::SPECIAL_AIR_LW_BOMB_ANIM => g.samus_special_air_lw_bomb_anim(gobj),
        addr::SPECIAL_LW_BOMB_IASA => g.samus_special_lw_bomb_iasa(gobj),
        addr::SPECIAL_AIR_LW_BOMB_IASA => {}
        addr::SPECIAL_LW_BOMB_PHYS => g.samus_special_lw_bomb_phys(gobj),
        addr::SPECIAL_AIR_LW_BOMB_PHYS => g.samus_special_air_lw_bomb_phys(gobj),
        addr::SPECIAL_LW_BOMB_COLL => g.samus_special_lw_bomb_coll(gobj),
        addr::SPECIAL_AIR_LW_BOMB_COLL => g.samus_special_air_lw_bomb_coll(gobj),
        addr::SPECIAL_LW_8012B570 => g.samus_bomb_to_air(gobj),
        addr::SPECIAL_LW_8012B5F0 => g.samus_bomb_hop(gobj),
        addr::SPECIAL_LW_8012B668 => g.samus_bomb_to_ground(gobj),
        _ => return false,
    }
    true
}
