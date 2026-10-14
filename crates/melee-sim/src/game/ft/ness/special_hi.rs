//! PK Thunder (`ftnessspecialhi.c`): Ness steers the ball ([`super::super::super::it::kinds::nesspkthunder`])
//! while he holds it, and when it touches him he flies off it (PK Thunder 2, `SpecialHi` on the
//! ground and `SpecialAirHi` in the air), turning along walls or bouncing off them.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::nesspkthunder;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{PART_HIP_N, PART_L2ND_NA, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::structs::CollData;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::mp::coll::{CEILING_HUG, CEILING_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK};
use crate::vec::Vec3;

/// What PK Thunder's states keep between the ground and the air (`FTNESS_SPECIALHI_COLL_FLAG`).
const COLL_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// What PK Thunder 2 keeps (`FTNESS_JIBAKU_COLL_FLAG`).
const JIBAKU_FLAGS: u32 = COLL_FLAGS | KEEP_GFX | SKIP_HIT;

/// `M_PI` and `M_TAU` as the original's doubles; `M_PI_2` as its float.
const PI: f64 = std::f64::consts::PI;
const TAU: f64 = 2.0 * std::f64::consts::PI;
const HALF_PI: f64 = PI / 2.0;

/// The speeds under which PK Thunder 2's slowing stops (`vel_epsilon`, a float).
const EPSILON: f32 = 1.0e-4;

/// The effects: the ball's aura on Ness, PK Thunder 2's, and the rebound's dust.
const GFX_AURA: i32 = 1262;
const GFX_JIBAKU: i32 = 1263;
const GFX_REBOUND: i32 = 1030;

/// `M_PI_2` as a float register.
fn half_pi() -> f64 {
    fp::frsp(fp::fdiv(PI, 2.0))
}

/// The sign of `x` as the original's `sign` inline gives it.
fn sign(x: f64) -> f32 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

impl Running<'_> {
    /// `ftNs_SpecialHiStopGFX`: in a PK Thunder state, the fighter's effects go.
    #[doc(alias = "ftNs_SpecialHiStopGFX")]
    pub fn ness_special_hi_stop_gfx(&mut self, gobj: GObjId) {
        let id = self.game.fighter(gobj).motion_id;
        if (ms::SPECIAL_HI_START..=ms::SPECIAL_AIR_HI_REBOUND).contains(&id) {
            ef::eflib::destroy_all(self, gobj);
            self.game.fighter_mut(gobj).set_u_i32(u::PKTHUNDER_GFX, 0);
        }
    }

    /// The `startGFX` inline: the old effects go, and `id` follows the hip.
    fn ness_special_hi_start_gfx(&mut self, gobj: GObjId, id: i32) {
        self.ness_special_hi_stop_gfx(gobj);
        let j = self.bone_joint(gobj, PART_HIP_N);
        ef::spawn(self, id, Some(gobj), &[Arg::JObj(j)]);
        self.game.fighter_mut(gobj).set_u_i32(u::PKTHUNDER_GFX, 1);
    }

    /// Ness's middle (5 units up, scaled) and the ball's head.
    fn ness_thunder_offset(&mut self, gobj: GObjId) -> (Vec3, Vec3) {
        let thunder = self.game.fighter(gobj).u.gobj_at(u::PKTHUNDER);
        let fp = self.game.fighter(gobj);
        let mut pos = fp.cur_pos;
        pos.y = mem(fp::fmadds(5.0, reg(fp.x34_scale.y), reg(pos.y)));
        let pair = nesspkthunder::trail_pos(self, thunder.expect("no PK Thunder"), 0);
        (pos, pair)
    }

    /// Whether PK Thunder touches Ness, `ftNs_SpecialHi_ItemPKThunder_CheckNessCollide`: once
    /// it has left him (`thunderColl` 1 until then, 0 after), the ball within a box around his
    /// middle sets state 2 and keeps where its head and first trail are.
    #[doc(alias = "ftNs_SpecialHi_ItemPKThunder_CheckNessCollide")]
    pub fn ness_thunder_check_collide(&mut self, gobj: GObjId) -> bool {
        let Some(thunder) = self.game.fighter(gobj).u.gobj_at(u::PKTHUNDER) else {
            return false;
        };
        match self.game.fighter(gobj).mv_i32(mv::HI_THUNDER_COLL) {
            0 => {
                let (pos, pair) = self.ness_thunder_offset(gobj);
                if close(pos, pair) {
                    let p1 = nesspkthunder::trail_pos(self, thunder, 0);
                    let p2 = nesspkthunder::trail_pos(self, thunder, 1);
                    let fp = self.game.fighter_mut(gobj);
                    fp.set_mv_i32(mv::HI_THUNDER_COLL, 2);
                    set_mv_vec(fp, mv::HI_COLL_POS1, p1);
                    set_mv_vec(fp, mv::HI_COLL_POS2, p2);
                    return true;
                }
            }
            1 => {
                let (pos, pair) = self.ness_thunder_offset(gobj);
                if !close(pos, pair) {
                    self.game
                        .fighter_mut(gobj)
                        .set_mv_i32(mv::HI_THUNDER_COLL, 0);
                }
            }
            _ => {}
        }
        false
    }

    /// Whether Ness holds PK Thunder, `ftNs_SpecialHi_CheckSpecialHiHold`.
    #[doc(alias = "ftNs_SpecialHi_CheckSpecialHiHold")]
    pub fn ness_special_hi_holding(&self, gobj: GObjId) -> bool {
        let id = self.game.fighter(gobj).motion_id;
        id == ms::SPECIAL_HI_HOLD || id == ms::SPECIAL_AIR_HI_HOLD
    }

    /// The hooks and rotation PK Thunder leaves cleared, and the aura gone.
    fn ness_thunder_clear(&mut self, gobj: GObjId) {
        self.ness_special_hi_stop_gfx(gobj);
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// PK Thunder gone without Ness, `ftNs_SpecialHi_ItemPKThunderRemove`.
    #[doc(alias = "ftNs_SpecialHi_ItemPKThunderRemove")]
    pub fn ness_thunder_remove(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::PKTHUNDER).is_some() {
            fp.u.set_gobj_at(u::PKTHUNDER, None);
        }
        self.ness_thunder_clear(gobj);
    }

    /// Ness hit while PK Thunder is out, `ftNs_SpecialHi_TakeDamage`: the ball goes.
    #[doc(alias = "ftNs_SpecialHi_TakeDamage")]
    pub fn ness_special_hi_take_damage(&mut self, gobj: GObjId) {
        if let Some(t) = self.game.fighter(gobj).u.gobj_at(u::PKTHUNDER) {
            nesspkthunder::remove(self, t);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(u::PKTHUNDER, None);
        }
        self.ness_thunder_clear(gobj);
    }

    /// PK Thunder reflected, `ftNs_SpecialHi_ItemPKThunderCheckOwn`: Ness lets go of his own.
    #[doc(alias = "ftNs_SpecialHi_ItemPKThunderCheckOwn")]
    pub fn ness_thunder_check_own(&mut self, gobj: GObjId, thunder: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::PKTHUNDER) == Some(thunder) {
            fp.u.set_gobj_at(u::PKTHUNDER, None);
        }
    }

    /// Turning PK Thunder 2 along a wall, `ftNs_SpecialAirHi_CollisionModVel`: the flight's
    /// angle wrapped into [0, 2π], and the speed turned to run along the wall it met.
    #[doc(alias = "ftNs_SpecialAirHi_CollisionModVel")]
    pub fn ness_special_air_hi_turn(&mut self, gobj: GObjId, cd: &CollData) {
        let fp = self.game.fighter_mut(gobj);
        while reg(fp.mv_f32(mv::HI_AERIAL_VEL)) < 0.0 {
            let v = fp::frsp(fp::fadd(reg(fp.mv_f32(mv::HI_AERIAL_VEL)), TAU));
            fp.set_mv_f32(mv::HI_AERIAL_VEL, v);
        }
        while reg(fp.mv_f32(mv::HI_AERIAL_VEL)) > TAU {
            let v = fp::frsp(fp::fsub(reg(fp.mv_f32(mv::HI_AERIAL_VEL)), TAU));
            fp.set_mv_f32(mv::HI_AERIAL_VEL, v);
        }
        // The angle to turn to; the caller met a wall, so one branch sets it.
        let mut wall = None;
        if cd.env_flags & LEFT_WALL_MASK != 0 {
            let n = cd.left_facing_wall.normal;
            let mut f1 = reg(math::atan2(n.y, n.x));
            while f1 < 0.0 {
                f1 = fp::frsp(fp::fadd(f1, TAU));
            }
            while f1 > TAU {
                f1 = fp::frsp(fp::fsub(f1, TAU));
            }
            let mut f3 = fp::frsp(fp::fadd(PI, reg(fp.mv_f32(mv::HI_AERIAL_VEL))));
            while f3 < 0.0 {
                f3 = fp::frsp(fp::fadd(f3, TAU));
            }
            while f3 > TAU {
                f3 = fp::frsp(fp::fsub(f3, TAU));
            }
            f1 = if fp::fsubs(f3, f1) < 0.0 {
                fp::frsp(fp::fadd(f1, HALF_PI))
            } else {
                fp::frsp(fp::fsub(f1, HALF_PI))
            };
            wall = Some(f1);
        }
        if cd.env_flags & RIGHT_WALL_MASK != 0 {
            let n = cd.right_facing_wall.normal;
            let mut f1 = reg(math::atan2(n.y, n.x));
            let mut f3 = fp::frsp(fp::fadd(PI, f1));
            while f3 < 0.0 {
                f3 = fp::frsp(fp::fadd(f3, TAU));
            }
            while f3 > TAU {
                f3 = fp::frsp(fp::fsub(f3, TAU));
            }
            f1 = if fp::fsubs(reg(fp.mv_f32(mv::HI_AERIAL_VEL)), f3) < 0.0 {
                fp::frsp(fp::fadd(f1, HALF_PI))
            } else {
                fp::frsp(fp::fsub(f1, HALF_PI))
            };
            wall = Some(f1);
        }
        let f1 = wall.expect("ftNs_SpecialAirHi_CollisionModVel without a wall");
        let turn = mem(fp::fsubs(f1, reg(fp.mv_f32(mv::HI_AERIAL_VEL))));
        fp.self_vel.rotate_about(Vec3::new(0.0, 0.0, 1.0), turn);
        let a = math::atan2(fp.self_vel.y, fp.self_vel.x);
        fp.set_mv_f32(mv::HI_AERIAL_VEL, reg(a));
    }

    /// PK Thunder's start: the timers and its variables (shared by the ground's and the air's).
    fn ness_special_hi_start(&mut self, gobj: GObjId, msid: i32) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.ness_special_hi_reset(gobj);
        let delay = self.ness_attr_i32(f, attr::PK_THUNDER_GRAVITY_DELAY);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_THUNDER_COLL, 1);
        fp.set_mv_i32(mv::HI_GRAVITY_DELAY, delay);
        fp.set_mv_i32(mv::HI_JIBAKU_GFX, 0);
        fp.set_mv_f32(mv::HI_COLL_POS1, 0.0);
        fp.set_mv_f32(mv::HI_COLL_POS2, 0.0);
        let a = if fp.facing_dir == 1.0 { 0.0 } else { PI };
        fp.set_mv_f32(mv::HI_AERIAL_VEL, fp::frsp(a));
        fp.set_mv_f32(mv::HI_FACING_DIR, 1.0);
        fp.set_mv_f32(mv::HI_UNK_VAR, 0.0);
    }

    /// The timers, the fall and the unused variables reset, no hooks and no rotation.
    fn ness_special_hi_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let l1 = self.ness_attr_i32(f, attr::PK_THUNDER_LOOP1);
        let l2 = self.ness_attr_i32(f, attr::PK_THUNDER_LOOP2);
        let delay = self.ness_attr_i32(f, attr::PK_THUNDER_GRAVITY_DELAY);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HI_TIMER_LOOP1, l1);
        fp.set_mv_i32(mv::HI_TIMER_LOOP2, l2);
        fp.set_mv_i32(mv::HI_GRAVITY_DELAY, delay);
        fp.set_mv_f32(mv::HI_FALL_ACCEL, 0.0);
        set_mv_vec(fp, mv::HI_UNK_VECTOR1, Vec3::new(0.0, 0.0, 0.0));
        fp.set_mv_f32(mv::HI_UNK_VAR4, 0.0);
        fp.set_mv_f32(mv::HI_UNK_VAR3, 0.0);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
        self.game.part_set_rot_x(f, 0, 0.0);
    }

    /// `ftNs_SpecialHiStart_Enter`.
    #[doc(alias = "ftNs_SpecialHiStart_Enter")]
    pub fn ness_special_hi_start_enter(&mut self, gobj: GObjId) {
        self.ness_special_hi_start(gobj, ms::SPECIAL_HI_START);
        self.anim_update(gobj);
    }

    /// `ftNs_SpecialAirHiStart_Enter`: no rise kept.
    #[doc(alias = "ftNs_SpecialAirHiStart_Enter")]
    pub fn ness_special_air_hi_start_enter(&mut self, gobj: GObjId) {
        self.ness_special_hi_start(gobj, ms::SPECIAL_AIR_HI_START);
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.anim_update(gobj);
    }

    /// PK Thunder 2's rotation: the model turned along the flight.
    fn ness_jibaku_rotate(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let a = reg(math::atan2(fp.self_vel.x, fp.self_vel.y));
        let rot = mem(fp::fmsubs(reg(fp.facing_dir), a, half_pi()));
        self.game.part_set_rot_x(f, 0, rot);
    }

    /// PK Thunder 2's common entry: `unkVar`, the rotation, no hooks, no jumps left.
    fn ness_jibaku_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let v = self.ness_attr_f32(f, attr::PK_THUNDER_2_UNK1);
        self.game.fighters[f.0].set_mv_f32(mv::HI_UNK_VAR, reg(v));
        self.ness_jibaku_rotate(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// PK Thunder 2 on the ground, `ftNs_SpecialHi_Enter`: off a platform or when the ball
    /// comes from too far below the floor's normal, he flies off in the air; within the
    /// knockdown angle he slides off along the floor; past it he's knocked down.
    #[doc(alias = "ftNs_SpecialHi_Enter")]
    pub fn ness_special_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.coll_data.floor.flags & (1 << 8) == 0 {
            let p1 = mv_vec(fp, mv::HI_COLL_POS1);
            let d = Vec3::new(
                mem(fp::fsubs(reg(fp.cur_pos.x), reg(p1.x))),
                mem(fp::fsubs(
                    fp::fmadds(5.0, reg(fp.x34_scale.y), reg(fp.cur_pos.y)),
                    reg(p1.y),
                )),
                0.0,
            );
            let angle = reg(fp.coll_data.floor.normal.angle(d));
            if !(angle < half_pi()) {
                let knockdown = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_KNOCKDOWN_ANGLE));
                if !(angle > fp::fmuls(f64::from(0.017_453_292_f32), fp::fadds(90.0, knockdown))) {
                    let fp = &mut self.game.fighters[f.0];
                    fp.facing_dir = sign(reg(d.x));
                    fp.set_mv_f32(mv::HI_FACING_DIR, reg(sign(reg(d.y))));
                    let a = math::atan2(d.y, d.x);
                    fp.set_mv_f32(mv::HI_AERIAL_VEL, reg(a));
                    self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
                    let m = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_MOMENTUM));
                    let fp = &mut self.game.fighters[f.0];
                    fp.gr_vel = mem(fp::fmuls(m, reg(fp.facing_dir)));
                    self.ness_jibaku_start(gobj);
                    return;
                }
                // `block_stuff`: knocked down.
                self.ness_special_hi_stop_gfx(gobj);
                self.game.part_set_rot_x(f, 0, 0.0);
                self.enter_down_bound(gobj);
                return;
            }
        }
        let fp = &mut self.game.fighters[f.0];
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        self.go_airborne_jumped(f);
        self.ness_special_air_hi_enter(gobj);
    }

    /// The `NessFloatMath_PKThunder2` inline: facing away from where the ball touched him, at
    /// PK Thunder 2's speed.
    fn ness_jibaku_launch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let m = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_MOMENTUM));
        let fp = &mut self.game.fighters[f.0];
        let p1 = mv_vec(fp, mv::HI_COLL_POS1);
        let dx = fp::fsubs(reg(fp.cur_pos.x), reg(p1.x));
        let dy = fp::fsubs(
            fp::fmadds(5.0, reg(fp.x34_scale.y), reg(fp.cur_pos.y)),
            reg(p1.y),
        );
        fp.facing_dir = sign(dx);
        fp.set_mv_f32(mv::HI_FACING_DIR, reg(sign(dy)));
        let a = math::atan2(mem(dy), mem(dx));
        fp.set_mv_f32(mv::HI_AERIAL_VEL, reg(a));
        let c = reg(math::cos(fp.mv_f32(mv::HI_AERIAL_VEL)));
        fp.self_vel.x = mem(fp::fmuls(m, c));
        let s = reg(math::sin(fp.mv_f32(mv::HI_AERIAL_VEL)));
        fp.self_vel.y = mem(fp::fmuls(m, s));
    }

    /// PK Thunder 2 in the air, `ftNs_SpecialAirHi_Enter`.
    #[doc(alias = "ftNs_SpecialAirHi_Enter")]
    pub fn ness_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.ness_jibaku_launch(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.ness_jibaku_start(gobj);
    }

    /// The start's end (`ftNs_SpecialHiStart_Anim`, `ftNs_SpecialAirHiStart_Anim`): holding,
    /// the ball out from the left hand unless one is out, the jumps used and the aura.
    fn ness_special_hi_start_anim(&mut self, gobj: GObjId, hold: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.change_motion_state(gobj, hold, 0, 0.0, 1.0, 0.0, None);
        if self.game.fighter(gobj).u.gobj_at(u::PKTHUNDER).is_none() {
            let j = self.bone_joint(gobj, PART_L2ND_NA);
            let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            pos.z = 0.0;
            let facing = self.game.fighter(gobj).facing_dir;
            let t = nesspkthunder::spawn(self, gobj, pos, facing);
            let fp = self.game.fighter_mut(gobj);
            fp.u.set_gobj_at(u::PKTHUNDER, t);
            if t.is_some() {
                fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
                fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
            }
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        self.ness_special_hi_start_gfx(gobj, GFX_AURA);
    }

    /// `ftNs_SpecialHiStart_Anim`.
    #[doc(alias = "ftNs_SpecialHiStart_Anim")]
    pub fn ness_special_hi_start_anim_ground(&mut self, gobj: GObjId) {
        self.ness_special_hi_start_anim(gobj, ms::SPECIAL_HI_HOLD);
    }

    /// `ftNs_SpecialAirHiStart_Anim`.
    #[doc(alias = "ftNs_SpecialAirHiStart_Anim")]
    pub fn ness_special_air_hi_start_anim(&mut self, gobj: GObjId) {
        self.ness_special_hi_start_anim(gobj, ms::SPECIAL_AIR_HI_HOLD);
    }

    /// Holding the ball (`ftNs_SpecialHiHold_Anim`, `ftNs_SpecialAirHiHold_Anim`): the timers
    /// run down; without the ball, the end once both are out; with his own, PK Thunder 2 once
    /// it touches him; with one he no longer owns, the end. Returns whether it touched him.
    fn ness_special_hi_hold_tick(&mut self, gobj: GObjId, end: i32) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let t = fp.mv_i32(mv::HI_TIMER_LOOP1);
        if t > 0 {
            fp.set_mv_i32(mv::HI_TIMER_LOOP1, t - 1);
        }
        let thunder = fp.u.gobj_at(u::PKTHUNDER);
        if thunder.is_none() {
            let t = fp.mv_i32(mv::HI_TIMER_LOOP2);
            if t > 0 {
                fp.set_mv_i32(mv::HI_TIMER_LOOP2, t - 1);
            }
        }
        match thunder {
            None => {
                let fp = self.game.fighter(gobj);
                if fp.mv_i32(mv::HI_TIMER_LOOP1) <= 0 && fp.mv_i32(mv::HI_TIMER_LOOP2) <= 0 {
                    self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                    self.ness_special_hi_stop_gfx(gobj);
                }
                false
            }
            Some(t) if nesspkthunder::owner(self, t) == Some(gobj) => {
                self.ness_thunder_check_collide(gobj)
            }
            Some(_) => {
                self.game
                    .fighter_mut(gobj)
                    .u
                    .set_gobj_at(u::PKTHUNDER, None);
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                self.ness_special_hi_stop_gfx(gobj);
                false
            }
        }
    }

    /// `ftNs_SpecialHiHold_Anim`.
    #[doc(alias = "ftNs_SpecialHiHold_Anim")]
    pub fn ness_special_hi_hold_anim(&mut self, gobj: GObjId) {
        if self.ness_special_hi_hold_tick(gobj, ms::SPECIAL_HI_END) {
            self.ness_special_hi_enter(gobj);
        }
    }

    /// `ftNs_SpecialAirHiHold_Anim`.
    #[doc(alias = "ftNs_SpecialAirHiHold_Anim")]
    pub fn ness_special_air_hi_hold_anim(&mut self, gobj: GObjId) {
        if self.ness_special_hi_hold_tick(gobj, ms::SPECIAL_AIR_HI_END) {
            self.ness_special_air_hi_enter(gobj);
        }
    }

    /// `ftNs_SpecialHiEnd_Anim`.
    #[doc(alias = "ftNs_SpecialHiEnd_Anim")]
    pub fn ness_special_hi_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// PK Thunder 2's aura on its first frame.
    fn ness_jibaku_gfx(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::HI_JIBAKU_GFX).wrapping_add(1);
        fp.set_mv_i32(mv::HI_JIBAKU_GFX, n);
        if n == 1 {
            self.ness_special_hi_start_gfx(gobj, GFX_JIBAKU);
        }
    }

    /// `ftNs_SpecialHi_Anim`.
    #[doc(alias = "ftNs_SpecialHi_Anim")]
    pub fn ness_special_hi_anim(&mut self, gobj: GObjId) {
        self.ness_jibaku_gfx(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::SPECIAL_HI_END, 0, 0.0, 1.0, 0.0, None);
            self.ness_special_hi_stop_gfx(gobj);
        }
    }

    /// Falling helpless after PK Thunder, or just falling when its landing lag is 0.
    fn ness_special_hi_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let lag = self.ness_attr_f32(f, attr::PK_THUNDER_2_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        let blend = self.ness_attr_f32(f, attr::PK_THUNDER_2_FREEFALL_ANIM_BLEND);
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, blend, false);
    }

    /// `ftNs_SpecialAirHiEnd_Anim`.
    #[doc(alias = "ftNs_SpecialAirHiEnd_Anim")]
    pub fn ness_special_air_hi_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
            self.ness_special_hi_fall(gobj);
        }
    }

    /// `ftNs_SpecialAirHi_Anim`: at the end he falls as fast as the flight's fall had grown.
    #[doc(alias = "ftNs_SpecialAirHi_Anim")]
    pub fn ness_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.ness_jibaku_gfx(gobj);
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter_mut(gobj);
            let a = reg(fp.mv_f32(mv::HI_FALL_ACCEL));
            let abs = if a < 0.0 { fp::fneg(a) } else { a };
            fp.self_vel.y = mem(fp::fneg(abs));
            self.ness_special_hi_fall(gobj);
        }
    }

    /// `ftNs_SpecialAirHiRebound_Anim`.
    #[doc(alias = "ftNs_SpecialAirHiRebound_Anim")]
    pub fn ness_special_air_hi_rebound_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ness_special_hi_fall(gobj);
        }
    }

    /// `ftNs_SpecialHiStart_Phys`: the gravity delay runs, and friction.
    #[doc(alias = "ftNs_SpecialHiStart_Phys")]
    pub fn ness_special_hi_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let t = fp.mv_i32(mv::HI_GRAVITY_DELAY);
        if t != 0 {
            fp.set_mv_i32(mv::HI_GRAVITY_DELAY, t.wrapping_sub(1));
        }
        self.ground_friction(gobj);
    }

    /// PK Thunder 2 on the ground, `ftNs_SpecialHi_Phys`: the ground speed slows to a floor,
    /// the rise stops falling past zero, and the model turns along the motion.
    #[doc(alias = "ftNs_SpecialHi_Phys")]
    pub fn ness_special_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let decel = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_DECELERATION_RATE));
        let fp = &mut self.game.fighters[f.0];
        let ground_vel = fp.gr_vel;
        let vel_y = fp.self_vel.y;
        fp.gr_vel = mem(fp::fnmsubs(decel, reg(fp.facing_dir), reg(ground_vel)));
        let eps = reg(EPSILON);
        if fp.facing_dir == 1.0 {
            if reg(fp.gr_vel) <= eps {
                fp.gr_vel = ground_vel;
            }
        } else if reg(fp.gr_vel) >= fp::fneg(eps) {
            fp.gr_vel = ground_vel;
        }
        if fp.mv_f32(mv::HI_FACING_DIR) == 1.0 {
            if reg(fp.self_vel.y) <= eps {
                fp.self_vel.y = vel_y;
            }
        } else if reg(fp.self_vel.y) >= fp::fneg(eps) {
            fp.self_vel.y = vel_y;
        }
        let v = fp.self_vel;
        set_mv_vec(fp, mv::HI_UNK_VECTOR1, v);
        self.self_movement_from_ground(gobj);
        self.ness_jibaku_rotate(gobj);
    }

    /// The air's start, hold and end physics (`ftNs_SpecialAirHiStart_Phys`,
    /// `ftNs_SpecialAirHiHold_Phys`, `ftNs_SpecialAirHiEnd_Phys`): after the delay, PK
    /// Thunder's slow fall, and air friction.
    fn ness_special_air_hi_hold_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ness_attr_f32(f, attr::PK_THUNDER_FALL_ACCEL));
        let fp = &mut self.game.fighters[f.0];
        let t = fp.mv_i32(mv::HI_GRAVITY_DELAY);
        if t != 0 {
            fp.set_mv_i32(mv::HI_GRAVITY_DELAY, t.wrapping_sub(1));
        } else {
            let tv = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, tv);
        }
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// PK Thunder 2 in the air, `ftNs_SpecialAirHi_Phys`: the speed slows (down to a floor)
    /// along the flight's angle, the model turns along it, and once the script says so it
    /// falls ever faster.
    #[doc(alias = "ftNs_SpecialAirHi_Phys")]
    pub fn ness_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let decel = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_DECELERATION_RATE));
        let fall = reg(self.ness_attr_f32(f, attr::PK_THUNDER_FALL_ACCEL));
        let momentum = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_MOMENTUM));
        let fp = &mut self.game.fighters[f.0];
        let (x, y) = (reg(fp.self_vel.x), reg(fp.self_vel.y));
        let sq = fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y));
        // `ABS(lbVector_Len_xy)`: the inline square root only decides the sign; the length is
        // `sqrtf`'s, called again.
        let inline = if sq > 0.0 {
            reg(mem(math::sqrt_reg(sq)))
        } else {
            sq
        };
        let len = reg(math::sqrt(mem(sq)));
        let len = if inline >= 0.0 { len } else { fp::fneg(len) };
        let slowed = fp::fsubs(len, decel);
        let speed = if slowed <= reg(EPSILON) { len } else { slowed };
        let c = reg(math::cos(fp.mv_f32(mv::HI_AERIAL_VEL)));
        fp.self_vel.x = mem(fp::fmuls(speed, c));
        let s = reg(math::sin(fp.mv_f32(mv::HI_AERIAL_VEL)));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.ness_jibaku_rotate(gobj);
        let fp = &mut self.game.fighters[f.0];
        let v = fp.self_vel;
        set_mv_vec(fp, mv::HI_UNK_VECTOR1, v);
        if fp.cmd_vars[0] == 1 {
            let a = fp::fsubs(reg(fp.mv_f32(mv::HI_FALL_ACCEL)), fall);
            fp.set_mv_f32(mv::HI_FALL_ACCEL, a);
            let lim = fp::fneg(momentum);
            if reg(fp.mv_f32(mv::HI_FALL_ACCEL)) < lim {
                fp.set_mv_f32(mv::HI_FALL_ACCEL, lim);
            }
            fp.cur_pos.y = mem(fp::fadds(
                reg(fp.cur_pos.y),
                reg(fp.mv_f32(mv::HI_FALL_ACCEL)),
            ));
        }
    }

    /// `ftNs_SpecialAirHiRebound_Phys`: the fighter's own gravity and air friction.
    #[doc(alias = "ftNs_SpecialAirHiRebound_Phys")]
    pub fn ness_special_air_hi_rebound_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (g, tv) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, tv);
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The ground's start, hold and end off a ledge (`ftNs_SpecialHiStart_Coll`,
    /// `ftNs_SpecialHiHold_Coll`, `ftNs_SpecialHiEnd_Coll`).
    fn ness_special_hi_ground_coll(&mut self, gobj: GObjId, air: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne_jumped(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, air, COLL_FLAGS, frame, 1.0, 0.0, None);
        }
    }

    /// PK Thunder 2 on the ground's collision, `ftNs_SpecialHi_Coll`: off a ledge into a wall,
    /// the air's end; off a ledge otherwise, on in the air; into a wall or ceiling, knocked
    /// down; on the floor, the flight's angle follows the floor.
    #[doc(alias = "ftNs_SpecialHi_Coll")]
    pub fn ness_special_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.ground_coll_off_ledges(gobj) {
            let env = self.game.fighters[f.0].coll_data.env_flags;
            self.go_airborne_jumped(f);
            if env & LEFT_WALL_MASK != 0 || env & RIGHT_WALL_MASK != 0 {
                self.ness_special_hi_reset(gobj);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                self.change_motion_state(
                    gobj,
                    ms::SPECIAL_AIR_HI_END,
                    COLL_FLAGS,
                    frame,
                    1.0,
                    0.0,
                    None,
                );
                return;
            }
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_HI,
                JIBAKU_FLAGS,
                frame,
                1.0,
                0.0,
                None,
            );
            return;
        }
        let env = self.game.fighters[f.0].coll_data.env_flags;
        if env & (CEILING_PUSH | CEILING_HUG) != 0
            || env & LEFT_WALL_MASK != 0
            || env & RIGHT_WALL_MASK != 0
        {
            self.game.fighters[f.0].gr_vel = 0.0;
            self.ness_special_hi_stop_gfx(gobj);
            self.game.part_set_rot_x(f, 0, 0.0);
            self.enter_down_bound(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        let up = (fp.facing_dir == 1.0) == (reg(n.y) > 0.0);
        let angle = if up { fp::fneg(HALF_PI) } else { HALF_PI };
        let a = fp::frsp(fp::fadd(angle, reg(math::atan2(n.y, n.x))));
        fp.set_mv_f32(mv::HI_AERIAL_VEL, a);
    }

    /// The air's start, hold and end landing (`ftNs_SpecialAirHiStart_Coll`,
    /// `ftNs_SpecialAirHiHold_Coll`, `ftNs_SpecialAirHiEnd_Coll`).
    fn ness_special_air_hi_land(&mut self, gobj: GObjId, ground: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, COLL_FLAGS);
        }
    }

    /// Bouncing PK Thunder 2 off a surface (the `rebound` inline): the speed mirrored and
    /// halved, facing its way, and the rebound state with its dust.
    fn ness_jibaku_rebound(&mut self, gobj: GObjId, normal: Vec3) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.mirror_xy(normal);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), 0.5));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), 0.5));
        let (x, y) = (reg(fp.self_vel.x), reg(fp.self_vel.y));
        // Its result is dropped.
        let _ = math::sqrt_accurate(mem(fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y))));
        let max = reg(fp.co_attrs.air_drift_max);
        Self::clamp_self_vel_x(fp, max);
        fp.facing_dir = if reg(fp.self_vel.x) >= 0.0 { 1.0 } else { -1.0 };
        self.ness_special_hi_stop_gfx(gobj);
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI_REBOUND,
            KEEP_GFX,
            0.0,
            1.0,
            0.0,
            None,
        );
        self.anim_update(gobj);
        let rot = math::atan2(mem(fp::fneg(reg(normal.x))), normal.y);
        let pos = self.game.fighters[f.0].cur_pos;
        ef::spawn(
            self,
            GFX_REBOUND,
            Some(gobj),
            &[Arg::Vec(pos), Arg::Float(rot)],
        );
    }

    /// Whether PK Thunder 2 meets a surface head on: its angle to the flight past the attribute.
    fn ness_jibaku_steep(&self, f: super::super::super::handles::FighterId, normal: Vec3) -> bool {
        let fp = &self.game.fighters[f.0];
        let a = reg(normal.angle(fp.self_vel));
        let hug = reg(self.ness_attr_f32(f, attr::PK_THUNDER_2_WALLHUG_ANGLE));
        a > fp::fmuls(f64::from(0.017_453_292_f32), fp::fadds(90.0, hug))
    }

    /// PK Thunder 2 in the air's collision, `ftNs_SpecialAirHi_Coll`: landing head on knocks
    /// him down, otherwise he slides on along the floor; a ledge catches him; a ceiling or wall
    /// met head on bounces him off, a wall met otherwise turns him along it.
    #[doc(alias = "ftNs_SpecialAirHi_Coll")]
    pub fn ness_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = mv_vec(fp, mv::HI_UNK_VECTOR1);
        let dir = if fp.facing_dir == 1.0 { 1 } else { -1 };
        if self.air_coll_ground_and_ledge(gobj, dir) {
            let n = self.game.fighters[f.0].coll_data.floor.normal;
            if self.ness_jibaku_steep(f, n) {
                self.game.fighters[f.0].self_vel = Vec3::new(0.0, 0.0, 0.0);
                self.ness_special_hi_stop_gfx(gobj);
                self.game.part_set_rot_x(f, 0, 0.0);
                self.enter_down_bound(gobj);
                return;
            }
            self.fox_air_to_ground(gobj, ms::SPECIAL_HI, JIBAKU_FLAGS);
            return;
        }
        if self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
            return;
        }
        let cd = &self.game.fighters[f.0].coll_data;
        let env = cd.env_flags;
        if env & (CEILING_PUSH | CEILING_HUG) != 0 {
            let n = cd.ceiling.normal;
            if self.ness_jibaku_steep(f, n) {
                self.ness_jibaku_rebound(gobj, n);
            }
        } else if env & LEFT_WALL_MASK != 0 {
            let n = cd.left_facing_wall.normal;
            if self.ness_jibaku_steep(f, n) {
                self.ness_jibaku_rebound(gobj, n);
                return;
            }
            let cd = self.game.fighters[f.0].coll_data.clone();
            self.ness_special_air_hi_turn(gobj, &cd);
        } else if env & RIGHT_WALL_MASK != 0 {
            let n = cd.right_facing_wall.normal;
            if self.ness_jibaku_steep(f, n) {
                self.ness_jibaku_rebound(gobj, n);
                return;
            }
            let cd = self.game.fighters[f.0].coll_data.clone();
            self.ness_special_air_hi_turn(gobj, &cd);
        }
    }

    /// `ftNs_SpecialAirHiRebound_Coll`: landing knocks him down; a ledge catches him.
    #[doc(alias = "ftNs_SpecialAirHiRebound_Coll")]
    pub fn ness_special_air_hi_rebound_coll(&mut self, gobj: GObjId) {
        if self.air_coll_ground_and_ledge(gobj, 0) {
            let f = self.game.fighter_id(gobj);
            self.game.fighters[f.0].self_vel = Vec3::new(0.0, 0.0, 0.0);
            self.ness_special_hi_stop_gfx(gobj);
            self.game.part_set_rot_x(f, 0, 0.0);
            self.enter_down_bound(gobj);
            return;
        }
        if self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
        }
    }
}

/// The `check_distance` inline: within 8.33 across and 12.33 up or down.
fn close(pos: Vec3, pair: Vec3) -> bool {
    let abs = |d: f64| if d < 0.0 { fp::fneg(d) } else { d };
    abs(fp::fsubs(reg(pos.x), reg(pair.x))) < f64::from(8.333_333_f32)
        && abs(fp::fsubs(reg(pos.y), reg(pair.y))) < f64::from(12.333_333_f32)
}

fn mv_vec(fp: &crate::game::structs::Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

fn set_mv_vec(fp: &mut crate::game::structs::Fighter, off: usize, v: Vec3) {
    fp.set_mv_f32(off, reg(v.x));
    fp.set_mv_f32(off + 4, reg(v.y));
    fp.set_mv_f32(off + 8, reg(v.z));
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_START_ENTER => g.ness_special_hi_start_enter(gobj),
        addr::SPECIAL_AIR_HI_START_ENTER => g.ness_special_air_hi_start_enter(gobj),
        addr::SPECIAL_HI_START_ANIM => g.ness_special_hi_start_anim_ground(gobj),
        addr::SPECIAL_HI_HOLD_ANIM => g.ness_special_hi_hold_anim(gobj),
        addr::SPECIAL_HI_END_ANIM => g.ness_special_hi_end_anim(gobj),
        addr::SPECIAL_HI_ANIM => g.ness_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_START_ANIM => g.ness_special_air_hi_start_anim(gobj),
        addr::SPECIAL_AIR_HI_HOLD_ANIM => g.ness_special_air_hi_hold_anim(gobj),
        addr::SPECIAL_AIR_HI_END_ANIM => g.ness_special_air_hi_end_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.ness_special_air_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_REBOUND_ANIM => g.ness_special_air_hi_rebound_anim(gobj),
        addr::SPECIAL_HI_START_IASA..=addr::SPECIAL_AIR_HI_REBOUND_IASA
            if (a - addr::SPECIAL_HI_START_IASA).is_multiple_of(4) => {}
        addr::SPECIAL_HI_START_PHYS => g.ness_special_hi_start_phys(gobj),
        addr::SPECIAL_HI_HOLD_PHYS | addr::SPECIAL_HI_END_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_HI_PHYS => g.ness_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_START_PHYS
        | addr::SPECIAL_AIR_HI_HOLD_PHYS
        | addr::SPECIAL_AIR_HI_END_PHYS => g.ness_special_air_hi_hold_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.ness_special_air_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_REBOUND_PHYS => g.ness_special_air_hi_rebound_phys(gobj),
        addr::SPECIAL_HI_START_COLL => {
            g.ness_special_hi_ground_coll(gobj, ms::SPECIAL_AIR_HI_START)
        }
        addr::SPECIAL_HI_HOLD_COLL => g.ness_special_hi_ground_coll(gobj, ms::SPECIAL_AIR_HI_HOLD),
        addr::SPECIAL_HI_END_COLL => g.ness_special_hi_ground_coll(gobj, ms::SPECIAL_AIR_HI_END),
        addr::SPECIAL_HI_COLL => g.ness_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_START_COLL => g.ness_special_air_hi_land(gobj, ms::SPECIAL_HI_START),
        addr::SPECIAL_AIR_HI_HOLD_COLL => g.ness_special_air_hi_land(gobj, ms::SPECIAL_HI_HOLD),
        addr::SPECIAL_AIR_HI_END_COLL => g.ness_special_air_hi_land(gobj, ms::SPECIAL_HI_END),
        addr::SPECIAL_AIR_HI_COLL => g.ness_special_air_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_REBOUND_COLL => g.ness_special_air_hi_rebound_coll(gobj),
        addr::SPECIAL_HI_TAKE_DAMAGE => g.ness_special_hi_take_damage(gobj),
        addr::SPECIAL_HI_STOP_GFX => g.ness_special_hi_stop_gfx(gobj),
        addr::SPECIAL_HI_ITEM_PKTHUNDER_REMOVE => g.ness_thunder_remove(gobj),
        _ => return false,
    }
    true
}
