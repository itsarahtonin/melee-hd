//! Standing and moving on the ground (`ftCo_Wait.c`, `ftCo_Walk.c`, `ftwalkcommon.c`,
//! `ftCo_Dash.c`, `ftCo_Run.c`, `ftCo_RunBrake.c`, `ftCo_Turn.c`, `ftCo_TurnRun.c`,
//! `ftwaitanim.c`).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::super::coll::scaled_by_height;
use super::checks::{KIND_FOX, KIND_MEWTWO, abs};
use super::ids::ms;
use super::{PAD_A, PAD_B};
use crate::fpu::{mem, reg};
use crate::game::structs::Fighter;
use crate::hsd::gobj::GObjId;
use crate::mp::coll::{LEFT_EDGE, RIGHT_EDGE};

/// `ftCommon_MotionVars_walk`.
const WALK_X0: usize = 0x0;
const WALK_MSID: usize = 0x4;
const WALK_FRAMES: usize = 0x8;
const WALK_RATES: usize = 0x14;
const WALK_ACCEL_MUL: usize = 0x20;
/// `ftCommon_MotionVars_dash`.
const DASH_X0: usize = 0x0;
const DASH_X4: usize = 0x4;
/// `ftCommon_MotionVars_run`.
const RUN_X0: usize = 0x0;
const RUN_X4: usize = 0x4;
/// `ftCommon_MotionVars_runbrake`.
const RUN_BRAKE_X0: usize = 0x0;
const RUN_BRAKE_FRAMES: usize = 0x4;
/// `ftCommon_MotionVars_turn`.
const TURN_HAS_TURNED: usize = 0x0;
const TURN_FACING_AFTER: usize = 0x4;
const TURN_X8: usize = 0x8;
const TURN_FRAMES: usize = 0x10;
const TURN_JUST_TURNED: usize = 0x18;
const TURN_X1C: usize = 0x1C;
/// `ftCommon_MotionVars_turnrun`.
const TURN_RUN_ACCEL_MUL: usize = 0xC;
const TURN_RUN_X14: usize = 0x14;

impl Running<'_> {
    /// Plays a fighter sound, `ft_PlaySFX`.
    pub(crate) fn play_sfx(&mut self, gobj: GObjId, id: i32, vol: i32, pan: i32) {
        let f = self.game.fighter_id(gobj);
        self.fighter_play_sfx(f, id, vol as u8, pan as u8);
    }

    /// `ftCo_Wait_IASA`.
    #[doc(alias = "ftCo_Wait_IASA")]
    pub fn co_wait_iasa(&mut self, gobj: GObjId) {
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
            || self.escape_n_check(gobj)
            || self.guard_check(gobj)
            || self.fox_appeal_s_check_input(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj)
            || self.squat_check(gobj)
            || self.turn_check(gobj)
            || self.walk_check(gobj);
    }

    /// The idle animations' next pick, `ftCo_8008A7A8`: when one ends, another drawn by weight
    /// from `table` (`WaitStruct`, from `ftData`), not the same one again unless it's one of
    /// the two that may repeat; holding an item (but as Fox or Mewtwo) replays it.
    #[doc(alias = "ftCo_8008A7A8")]
    pub fn wait_anim_tick(&mut self, gobj: GObjId, table_off: u32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        let ft_data = fp.ft_data.expect("a fighter without ftData");
        let table = self.data.ptr(ft_data.at(table_off));
        let replay = match table {
            None => true,
            Some(_) => fp.item_gobj.is_some() && fp.kind != KIND_MEWTWO && fp.kind != KIND_FOX,
        };
        if replay {
            let anim = fp.anim_id;
            self.play_anim(gobj, anim);
            return;
        }
        let table = table.unwrap();
        let anim = loop {
            // `getAnimID`.
            let max = self.game.objs.rng.below(100) + 1;
            let mut count = 0i32;
            let mut at = table;
            let pick = loop {
                let id = self.data.u32(at) as i32;
                assert!(
                    id != -1,
                    "ftwaitanim.c:86: wait anim data illegal!! ({max})"
                );
                count = count.wrapping_add(self.data.u32(at.at(4)) as i32);
                if max <= count {
                    break id;
                }
                at = at.at(8);
            };
            let cur = self.game.fighter(gobj).anim_id;
            // `inlineA0`: the two idles that may follow themselves.
            if cur == 2 || cur == 31 || cur != pick {
                break pick;
            }
        };
        self.play_anim(gobj, anim);
    }

    /// `ftCo_Wait_Coll` and the other grounded states that can teeter: `ft_80084280`.
    #[doc(alias = "ftCo_Wait_Coll")]
    #[doc(alias = "ftCo_Walk_Coll")]
    #[doc(alias = "ftCo_RunBrake_Coll")]
    #[doc(alias = "ftCo_Landing_Coll")]
    #[doc(alias = "ftCo_LandingAir_Coll")]
    pub fn co_wait_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter(gobj);
    }

    /// Walking when the stick leans forward, `ftCo_Walk_CheckInput`.
    #[doc(alias = "ftCo_Walk_CheckInput")]
    pub fn walk_check(&mut self, gobj: GObjId) -> bool {
        if self.walk_input(gobj) {
            self.enter_walk(gobj, 0.0);
            return true;
        }
        false
    }

    /// The stick leaning forward past the walk threshold, `ftWalkCommon_800DFC70`.
    #[doc(alias = "ftWalkCommon_800DFC70")]
    pub fn walk_input(&self, gobj: GObjId) -> bool {
        self.stick_forward(gobj, 0x24)
    }

    /// Walking off a teeter, which takes the stick further forward,
    /// `ftCo_Walk_CheckInput_Ottotto`.
    #[doc(alias = "ftCo_Walk_CheckInput_Ottotto")]
    pub fn walk_check_teeter(&mut self, gobj: GObjId) -> bool {
        if self.stick_forward(gobj, 0x474) && self.walk_input(gobj) {
            self.enter_walk(gobj, 0.0);
            return true;
        }
        false
    }

    /// Walking, `ftCo_Walk_Enter`: the walk's speed scaled for metal, size and a bunny hood
    /// (`Fighter_804D651C`, `Fighter_804D6524` and `Fighter_804D6520`).
    #[doc(alias = "ftCo_Walk_Enter")]
    pub fn enter_walk(&mut self, gobj: GObjId, anim_start: f32) {
        let fp = self.game.fighter(gobj);
        let mut mul = 1.0;
        if fp.is_metal {
            mul = reg(self.data.f32(self.common_ptr(0x38)));
        }
        if fp.x34_scale.y != 1.0 {
            let k = reg(self.data.f32(self.common_ptr(0x30).at(0xC)));
            mul = scaled_by_height(mul, reg(fp.x34_scale.y), k);
        }
        if fp.x197C.is_some() {
            mul = fp::fmuls(mul, reg(self.data.f32(self.common_ptr(0x34))));
        }
        let a = &fp.co_attrs;
        let frames = [fp.x2DC, fp.x2E0, fp.x2E4];
        let rates = [a.slow_walk_max, a.mid_walk_point, a.fast_walk_min];
        self.enter_walk_common(gobj, ms::WALK_SLOW, 0, anim_start, frames, rates, mem(mul));
    }

    /// Which of a walk's three speeds the fighter's ground speed is,
    /// `ftWalkCommon_GetWalkType`: 2 fast, 1 middle, 0 slow, scaled by the walk's `accel_mul`.
    #[doc(alias = "ftWalkCommon_GetWalkType")]
    pub fn walk_type(&self, gobj: GObjId) -> i32 {
        let c = self.common();
        let (fast, middle) = (reg(c.f32(0x2C)), reg(c.f32(0x28)));
        let fp = self.game.fighter(gobj);
        let v = abs(reg(fp.gr_vel));
        let mul = reg(fp.mv_f32(WALK_ACCEL_MUL));
        let max = reg(fp.co_attrs.walk_max_vel);
        if v >= fp::fmuls(mul, fp::fmuls(fast, max)) {
            2
        } else if v >= fp::fmuls(mul, fp::fmuls(middle, max)) {
            1
        } else {
            0
        }
    }

    /// A walk at the speed the ground speed calls for, `ftWalkCommon_800DFCA4`: `msid` is
    /// the slow walk's state, the middle and fast ones follow it; each speed's animation frame
    /// to switch at and its animation's speed scale are kept for the walk.
    #[doc(alias = "ftWalkCommon_800DFCA4")]
    #[allow(clippy::too_many_arguments)]
    pub fn enter_walk_common(
        &mut self,
        gobj: GObjId,
        msid: i32,
        flags: u32,
        anim_start: f32,
        frames: [f32; 3],
        rates: [f32; 3],
        accel_mul: f32,
    ) {
        self.game
            .fighter_mut(gobj)
            .set_mv_f32(WALK_ACCEL_MUL, reg(accel_mul));
        let walk = self.walk_type(gobj);
        self.change_motion_state(
            gobj,
            msid.wrapping_add(walk),
            flags,
            anim_start,
            1.0,
            0.0,
            None,
        );
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(WALK_X0, reg(fp.gr_vel));
        fp.set_mv_i32(WALK_MSID, msid);
        for i in 0..3 {
            fp.set_mv_f32(WALK_FRAMES + 4 * i, reg(frames[i]));
            fp.set_mv_f32(WALK_RATES + 4 * i, reg(rates[i]));
        }
    }

    /// A walk's animation speed follows the ground speed (or, on a slippery floor, the speed
    /// the stick asks for), `ftWalkCommon_800DFDDC` (`ftCo_Walk_Anim`).
    ///
    /// The original leaves the rate in a register its caller set when the state isn't one of
    /// the walk's three (`f31`); the walk's own states are always one, and a call outside them
    /// stops here.
    #[doc(alias = "ftWalkCommon_800DFDDC")]
    #[doc(alias = "ftCo_Walk_Anim")]
    pub fn co_walk_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let mut v = if self.ground_friction_mul(fp) < 1.0 {
            reg(fp.mv_f32(WALK_X0))
        } else {
            reg(fp.gr_vel)
        };
        let rate = if fp::fmuls(v, reg(fp.facing_dir)) <= 0.0 {
            0.0
        } else {
            v = abs(v);
            let walk = fp.motion_id.wrapping_sub(fp.mv_i32(WALK_MSID));
            assert!(
                (0..3).contains(&walk),
                "ftWalkCommon_800DFDDC: state {} isn't the walk's ({}); the original reads a \
                 register its caller left",
                fp.motion_id,
                fp.mv_i32(WALK_MSID)
            );
            fp::fdivs(v, reg(fp.mv_f32(WALK_RATES + 4 * walk as usize)))
        };
        self.set_anim_rate(gobj, mem(rate));
    }

    /// Switches a walk to the speed the ground speed now calls for, at the same point of its
    /// animation's cycle, through `enter` (`ftWalkCommon_800DFEC8`).
    #[doc(alias = "ftWalkCommon_800DFEC8")]
    pub fn walk_speed_switch(&mut self, gobj: GObjId, enter: impl FnOnce(&mut Self, GObjId, f32)) {
        let walk = self.walk_type(gobj);
        let fp = self.game.fighter(gobj);
        let base = fp.mv_i32(WALK_MSID);
        let sum = base.wrapping_add(walk);
        if sum == fp.motion_id {
            return;
        }
        let frame = match sum.wrapping_sub(base) {
            k @ 0..=2 => reg(fp.mv_f32(WALK_FRAMES + 4 * k as usize)),
            _ => panic!("ftwalkcommon.c:71: couldn't get walk frame"),
        };
        let end = reg(self.anim_end_frame(gobj));
        let cur = reg(self.game.fighter(gobj).cur_anim_frame);
        let q = fp::fctiwz(fp::fdivs(cur, end));
        let adjusted = fp::fnmsubs(end, fp::frsp(f64::from(q)), cur);
        let at = fp::fctiwz(fp::fmuls(frame, fp::fdivs(adjusted, end)));
        enter(self, gobj, mem(fp::frsp(f64::from(at))));
    }

    /// The fighter's animation's last frame, `ftAnim_8006F484`: the blend skeleton's while
    /// blending.
    #[doc(alias = "ftAnim_8006F484")]
    pub fn anim_end_frame(&self, gobj: GObjId) -> f32 {
        let fp = self.game.fighter(gobj);
        let j = if fp.x8A4_animBlendFrames == 0.0 {
            Some(self.root_jobj(gobj))
        } else {
            fp.x8AC_animSkeleton
        };
        crate::lb::anim::end_frame(&self.game.objs, j)
    }

    /// `ftCo_Walk_IASA`.
    #[doc(alias = "ftCo_Walk_IASA")]
    pub fn co_walk_iasa(&mut self, gobj: GObjId) {
        if self.catch_check(gobj)
            || self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj)
            || self.squat_check(gobj)
            || self.walk_stop_check(gobj)
        {
            return;
        }
        self.walk_speed_switch(gobj, |g, gobj, frame| g.enter_walk(gobj, frame));
    }

    /// A walk's acceleration toward the speed the stick asks for, easing off near it,
    /// `ftWalkCommon_800E0060` (`ftCo_Walk_Phys`).
    #[doc(alias = "ftWalkCommon_800E0060")]
    #[doc(alias = "ftCo_Walk_Phys")]
    pub fn co_walk_phys(&mut self, gobj: GObjId) {
        let c = self.common();
        let (taper, x440) = (reg(c.f32(0x30)), reg(c.f32(0x440)));
        let fp = self.game.fighter_mut(gobj);
        let mul = reg(fp.mv_f32(WALK_ACCEL_MUL));
        let x = reg(fp.input.lstick[0].x);
        let a = &fp.co_attrs;
        let mut accel = fp::fmuls(mul, fp::fmuls(x, reg(a.walk_accel_mul)));
        // `getWalkAccel`.
        let base = if fp.input.lstick[0].x > 0.0 {
            fp::fmuls(mul, reg(a.walk_accel_base))
        } else {
            fp::fmuls(mul, fp::fneg(reg(a.walk_accel_base)))
        };
        accel = fp::fadds(accel, base);
        let target = fp::fmuls(mul, fp::fmuls(x, reg(a.walk_max_vel)));
        if target != 0.0 {
            let frac = fp::fdivs(reg(fp.gr_vel), target);
            if frac > 0.0 && frac < 1.0 {
                accel = fp::fmuls(accel, fp::fmuls(fp::fsubs(1.0, frac), taper));
            }
        }
        fp.set_mv_f32(WALK_X0, fp::fmuls(target, x440));
        let friction = reg(fp.co_attrs.ground_friction);
        Self::ground_accel_dash_run(fp, mem(accel), mem(target), friction);
        self.self_movement_from_ground(gobj);
    }

    /// Accelerates along the ground toward a target speed, braking by `friction` past it and
    /// never past the character's top ground speed, `ftCommon_CalcGroundAccel_DashRun`.
    #[doc(alias = "ftCommon_CalcGroundAccel_DashRun")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn ground_accel_dash_run(fp: &mut Fighter, accel: f32, target: f32, friction: f64) {
        let target = reg(target);
        if target == 0.0 {
            Self::ground_accel_deaccel(fp, friction);
            return;
        }
        let mut accel = reg(accel);
        let v = reg(fp.gr_vel);
        let max = reg(fp.co_attrs.ground_max_horizontal_velocity);
        if !(fp::fmuls(v, accel) < 0.0) {
            if accel > 0.0 {
                if fp::fadds(v, accel) > target {
                    accel = fp::fneg(friction);
                    if fp::fadds(v, accel) < target {
                        accel = fp::fsubs(target, v);
                    }
                    if fp::fadds(v, accel) > max {
                        accel = fp::fsubs(max, v);
                    }
                }
            } else if fp::fadds(v, accel) < target {
                accel = friction;
                if fp::fadds(v, accel) > target {
                    accel = fp::fsubs(target, v);
                }
                if fp::fadds(v, accel) < fp::fneg(max) {
                    accel = fp::fsubs(fp::fneg(max), v);
                }
            }
        }
        fp.xE4_ground_accel_1 = mem(accel);
    }

    /// The dash's and run's acceleration and target speed from the stick (`getAccelAndTarget`),
    /// as the original keeps them on its stack.
    fn dash_accel_and_target(fp: &Fighter) -> (f32, f32) {
        let x = reg(fp.input.lstick[0].x);
        let a = &fp.co_attrs;
        let base = if fp.input.lstick[0].x > 0.0 {
            reg(a.dash_accel_base)
        } else {
            fp::fneg(reg(a.dash_accel_base))
        };
        let accel = fp::fadds(fp::fmuls(x, reg(a.dash_accel_mul)), base);
        let target = fp::fmuls(x, reg(a.dash_max_velocity));
        (mem(accel), mem(target))
    }

    /// The run's, dash's and turn's friction: the character's, times `ftCommonData` x60.
    fn run_friction(&self, fp: &Fighter) -> f64 {
        fp::fmuls(
            reg(fp.co_attrs.ground_friction),
            reg(self.common().f32(0x60)),
        )
    }

    /// Dashing when the stick is smashed to the side, `ftCo_Dash_CheckInput`: turning first if
    /// it's behind.
    #[doc(alias = "ftCo_Dash_CheckInput")]
    pub fn dash_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (reg(c.f32(0x3C)), c.i32(0x40));
        let fp = self.game.fighter(gobj);
        let x = reg(fp.input.lstick[0].x);
        if abs(x) >= t && i32::from(fp.active_timer.lstick.x) < window {
            if fp::fmuls(x, reg(fp.facing_dir)) < 0.0 {
                self.enter_turn_smash(gobj);
            } else {
                self.enter_dash(gobj, 1);
            }
            return true;
        }
        false
    }

    /// Dashing, `ftCo_Dash_Enter`: the dash's first push (`ftCommon_800804A0`); `smashed`
    /// says the stick was smashed (`mv` x4).
    #[doc(alias = "ftCo_Dash_Enter")]
    pub fn enter_dash(&mut self, gobj: GObjId, smashed: i32) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::DASH, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.active_timer.lstick.x = 0xFE;
        let init = fp::fmuls(reg(fp.facing_dir), reg(fp.co_attrs.dash_initial_velocity));
        if fp::fmuls(reg(fp.gr_vel), reg(fp.facing_dir)) < 0.0 {
            fp.set_mv_f32(DASH_X0, init);
        } else {
            fp.set_mv_f32(DASH_X0, fp::fsubs(init, reg(fp.gr_vel)));
        }
        let push = fp.mv_f32(DASH_X0);
        self.ground_push(gobj, push);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(DASH_X4, smashed);
        if fp.x197C.is_some() {
            self.play_sfx(gobj, 0x118, 0x7F, 0x40);
        }
    }

    /// A push along the ground, slowed on slippery floors, `ftCommon_800804A0`.
    #[doc(alias = "ftCommon_800804A0")]
    pub fn ground_push(&mut self, gobj: GObjId, v: f32) {
        let fp = self.game.fighter(gobj);
        let friction = self.ground_friction_mul(fp);
        let mut v = reg(v);
        if friction < 1.0 {
            v = fp::fmuls(v, friction);
        }
        self.game.fighter_mut(gobj).xE8_ground_accel_2 = mem(v);
    }

    /// `ftCo_Dash_Anim`, `ftCo_RunBrake_Anim`'s end and the other states that end in waiting.
    #[doc(alias = "ftCo_Dash_Anim")]
    #[doc(alias = "ftCo_SquatRv_Anim")]
    #[doc(alias = "ftCo_Attack11_Anim")]
    #[doc(alias = "ftCo_AttackHi4_Anim")]
    #[doc(alias = "ftCo_AttackLw4_Anim")]
    #[doc(alias = "ftCo_AttackDash_Anim")]
    #[doc(alias = "ftCo_Attack12_Anim")]
    #[doc(alias = "ftCo_Attack13_Anim")]
    #[doc(alias = "ftCo_StopWall_Anim")]
    #[doc(alias = "ftCo_Down_Anim")]
    #[doc(alias = "ftCo_DownStand_Anim")]
    #[doc(alias = "ftCo_DownAttack_Anim")]
    #[doc(alias = "ftCo_Passive_Anim")]
    #[doc(alias = "ftCo_PassiveStand_Anim")]
    pub fn co_wait_when_done_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Dash_IASA`: early in a smashed dash, a forward smash, item throw, grab or roll;
    /// a little later a dash attack, a dash back or a shield; past the dash's window a dash
    /// again or a shield; otherwise a taunt, a jump or running. Friction slows the fighter
    /// on the ways out that don't change state themselves.
    #[doc(alias = "ftCo_Dash_IASA")]
    pub fn co_dash_iasa(&mut self, gobj: GObjId) {
        let c = self.common();
        let (x44, x48, x4c, x54) = (c.f32(0x44), c.f32(0x48), c.f32(0x4C), reg(c.f32(0x54)));
        let fp = self.game.fighter(gobj);
        let frame = fp.cur_anim_frame;
        let smashed = fp.mv_i32(DASH_X4) != 0;
        let slow = 'slow: {
            if smashed && frame <= x44 {
                if self.special_s_check(gobj) {
                    break 'slow true;
                }
                if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
                    self.enter_item_throw(gobj, ms::LIGHT_THROW_F4);
                    break 'slow true;
                }
                if self.catch_dash_check(gobj) {
                    return;
                }
                if self.attack_s4_forward_check(gobj) {
                    break 'slow true;
                }
                let frame = self.game.fighter(gobj).cur_anim_frame;
                if frame <= x48 && self.escape_forward_check(gobj) {
                    break 'slow true;
                }
            } else if frame <= x4c {
                if self.special_s_check(gobj) {
                    break 'slow true;
                }
                if self.catch_dash_check(gobj) {
                    return;
                }
                if self.attack_dash_check(gobj) {
                    self.attack_dash_set_mv0(gobj);
                    return;
                }
                let fp = self.game.fighter(gobj);
                if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) < 0.0
                    && self.dash_check(gobj)
                {
                    break 'slow true;
                }
                let frame = self.game.fighter(gobj).cur_anim_frame;
                let left = fp::fctiwz(fp::fsubs(reg(x4c), reg(frame)));
                if self.guard_check_keep(gobj, left) {
                    break 'slow true;
                }
            } else {
                if self.catch_dash_check(gobj) {
                    return;
                }
                if self.dash_check(gobj) {
                    break 'slow true;
                }
                if self.guard_check(gobj) {
                    self.guard_set_x24(gobj);
                    break 'slow true;
                }
            }
            // `block_42`.
            if self.appeal_check(gobj) {
                break 'slow true;
            }
            if self.jump_check_relaxed(gobj) {
                return;
            }
            if self.game.fighter(gobj).cmd_vars[0] == 0 {
                return;
            }
            self.run_check(gobj);
            return;
        };
        if slow {
            let fp = self.game.fighter(gobj);
            let friction = self.ground_friction_mul(fp);
            let fp = self.game.fighter_mut(gobj);
            let v = reg(fp.gr_vel);
            fp.gr_vel = mem(fp::fmadds(fp::fneg(fp::fmuls(v, x54)), friction, v));
        }
    }

    /// `ftCo_Dash_Phys`: the first frame keeps the dash's push, then the dash accelerates.
    #[doc(alias = "ftCo_Dash_Phys")]
    pub fn co_dash_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_f32(DASH_X0) != 0.0 {
            self.game.fighters[f.0].set_mv_f32(DASH_X0, 0.0);
        } else {
            let friction = self.run_friction(&self.game.fighters[f.0]);
            let fp = &mut self.game.fighters[f.0];
            let (accel, target) = Self::dash_accel_and_target(fp);
            Self::ground_accel_dash_run(fp, accel, target, friction);
        }
        self.self_movement_from_ground(gobj);
    }

    /// `ftCo_Dash_Coll`, `ftCo_Run_Coll`: `ft_800844EC`.
    #[doc(alias = "ftCo_Dash_Coll")]
    #[doc(alias = "ftCo_Run_Coll")]
    pub fn co_run_coll(&mut self, gobj: GObjId) {
        self.ground_coll_run(gobj);
    }

    /// Running when the stick leans far enough forward, `fn_800CA5F0`.
    #[doc(alias = "fn_800CA5F0")]
    pub fn run_check(&mut self, gobj: GObjId) -> bool {
        if self.stick_forward(gobj, 0x58) {
            self.enter_run(gobj, 0.0, 0.0, 1.0);
            return true;
        }
        false
    }

    /// Running out of a turn-around, with `ftCommonData` x430 frames before braking,
    /// `fn_800CA644`.
    #[doc(alias = "fn_800CA644")]
    pub fn run_check_after_turn(&mut self, gobj: GObjId) -> bool {
        if self.stick_forward(gobj, 0x58) {
            let x430 = self.common().f32(0x430);
            self.enter_run(gobj, x430, 0.0, 1.0);
            return true;
        }
        false
    }

    /// Running on from the animation's frame and speed, `fn_800CA698`.
    #[doc(alias = "fn_800CA698")]
    pub fn run_check_keep_anim(&mut self, gobj: GObjId) -> bool {
        if self.stick_forward(gobj, 0x58) {
            let fp = self.game.fighter(gobj);
            let (frame, rate) = (fp.cur_anim_frame, fp.frame_speed_mul);
            self.enter_run(gobj, 0.0, frame, rate);
            return true;
        }
        false
    }

    /// Running, `ftCo_Run_Enter_Full` (`ftCo_Run_Enter` from the start): `brake_after`
    /// frames before the run can brake (`mv` x0).
    #[doc(alias = "ftCo_Run_Enter")]
    #[doc(alias = "ftCo_Run_Enter_Full")]
    pub fn enter_run(&mut self, gobj: GObjId, brake_after: f32, anim_start: f32, rate: f32) {
        self.change_motion_state(gobj, ms::RUN, 0, anim_start, rate, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(RUN_X0, reg(brake_after));
        fp.set_mv_f32(RUN_X4, reg(fp.gr_vel));
    }

    /// `ftCo_Run_Anim`: the run's animation speed follows the ground speed (or, on a slippery
    /// floor, the speed the stick asks for).
    #[doc(alias = "ftCo_Run_Anim")]
    pub fn co_run_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let v = if self.ground_friction_mul(fp) < 1.0 {
            reg(fp.mv_f32(RUN_X4))
        } else {
            reg(fp.gr_vel)
        };
        let rate = if fp::fmuls(v, reg(fp.facing_dir)) <= 0.0 {
            0.0
        } else {
            fp::fdivs(abs(v), reg(fp.co_attrs.run_animation_scaling))
        };
        self.set_anim_rate(gobj, mem(rate));
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(RUN_X0) > 0.0 {
            fp.set_mv_f32(RUN_X0, fp::fsubs(reg(fp.mv_f32(RUN_X0)), 1.0));
        }
    }

    /// `ftCo_Run_IASA`.
    #[doc(alias = "ftCo_Run_IASA")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn co_run_iasa(&mut self, gobj: GObjId) {
        if self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.catch_dash_check(gobj)
        {
            return;
        }
        if self.attack_dash_check(gobj) {
            self.attack_dash_set_mv0(gobj);
            return;
        }
        if self.guard_check(gobj) {
            let x410 = self.common().i32(0x410);
            self.guard_set_x20(gobj, x410);
            self.guard_set_x24(gobj);
            return;
        }
        if self.appeal_check(gobj) || self.jump_check_relaxed(gobj) {
            return;
        }
        let can_brake = self.game.fighter(gobj).mv_f32(RUN_X0) <= 0.0;
        if can_brake && self.turn_run_check(gobj) {
            return;
        }
        if !(self.game.fighter(gobj).mv_f32(RUN_X0) <= 0.0) {
            return;
        }
        self.run_brake_check(gobj);
    }

    /// `ftCo_Run_Phys`: the run accelerates toward the speed the stick asks for, easing off
    /// near it.
    #[doc(alias = "ftCo_Run_Phys")]
    pub fn co_run_phys(&mut self, gobj: GObjId) {
        let c = self.common();
        let (taper, x440) = (reg(c.f32(0x5C)), reg(c.f32(0x440)));
        let f = self.game.fighter_id(gobj);
        let friction = self.run_friction(&self.game.fighters[f.0]);
        let fp = &mut self.game.fighters[f.0];
        let (mut accel, target) = Self::dash_accel_and_target(fp);
        if target != 0.0 {
            let frac = fp::fdivs(reg(fp.gr_vel), reg(target));
            if frac > 0.0 && frac < 1.0 {
                accel = mem(fp::fmuls(
                    reg(accel),
                    fp::fmuls(fp::fsubs(1.0, frac), taper),
                ));
            }
        }
        fp.set_mv_f32(RUN_X4, fp::fmuls(reg(target), x440));
        Self::ground_accel_dash_run(fp, accel, target, friction);
        self.self_movement_from_ground(gobj);
    }

    /// Braking out of a run when the stick lets go, `ftCo_RunBrake_CheckInput`.
    #[doc(alias = "ftCo_RunBrake_CheckInput")]
    pub fn run_brake_check(&mut self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x58));
        if abs(reg(self.game.fighter(gobj).input.lstick[0].x)) < t {
            self.enter_run_brake(gobj);
            return true;
        }
        false
    }

    /// Braking, `ftCo_RunBrake_Enter`.
    #[doc(alias = "ftCo_RunBrake_Enter")]
    pub fn enter_run_brake(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        self.change_motion_state(gobj, ms::RUN_BRAKE, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(RUN_BRAKE_X0, 0);
        fp.set_mv_f32(RUN_BRAKE_FRAMES, reg(fp.co_attrs.max_run_brake_frames));
        if fp.x197C.is_some() {
            self.play_sfx(gobj, 0x119, 0x7F, 0x40);
        }
    }

    /// `ftCo_RunBrake_IASA`: a jump, turning around once the brake allows, or crouching.
    #[doc(alias = "ftCo_RunBrake_IASA")]
    pub fn co_run_brake_iasa(&mut self, gobj: GObjId) {
        if self.jump_check_relaxed(gobj) {
            return;
        }
        if self.game.fighter(gobj).cmd_vars[0] != 0 && self.turn_run_check_keep_frame(gobj) {
            return;
        }
        self.squat_check(gobj);
    }

    /// `ftCo_RunBrake_Phys`: braking with the run's friction.
    #[doc(alias = "ftCo_RunBrake_Phys")]
    pub fn co_run_brake_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.common().f32(0x60));
        let fp = &mut self.game.fighters[f.0];
        let friction = fp::fmuls(mul, reg(fp.co_attrs.ground_friction));
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// Whether the stick points back past `ftCommonData` x34, `ftCo_800C97A8`.
    #[doc(alias = "ftCo_800C97A8")]
    pub fn turn_input(&self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x34));
        let fp = self.game.fighter(gobj);
        fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) <= t
    }

    /// Turning around when the stick points back, `ftCo_Turn_CheckInput`.
    #[doc(alias = "ftCo_Turn_CheckInput")]
    pub fn turn_check(&mut self, gobj: GObjId) -> bool {
        if self.turn_input(gobj) {
            self.enter_turn_basic(gobj);
            return true;
        }
        false
    }

    /// Turning around, `ftCo_Turn_Enter`: facing the other way after `frames` frames;
    /// `dash_dir` a smashed turn's dash direction (`mv` x8).
    #[doc(alias = "ftCo_Turn_Enter")]
    pub fn enter_turn(
        &mut self,
        gobj: GObjId,
        msid: i32,
        flags: u32,
        dash_dir: f32,
        frames: f32,
        anim_start: f32,
    ) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TURN_HAS_TURNED, 0);
        fp.set_mv_i32(TURN_JUST_TURNED, 0);
        fp.set_mv_f32(TURN_FACING_AFTER, fp::fneg(reg(fp.facing_dir)));
        fp.set_mv_f32(TURN_FRAMES, reg(frames));
        fp.set_mv_f32(TURN_X8, reg(dash_dir));
        fp.set_mv_i32(TURN_X1C, 0);
        self.change_motion_state(gobj, msid, flags, anim_start, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// A standing turn, `ftCo_Turn_Enter_Basic`.
    #[doc(alias = "ftCo_Turn_Enter_Basic")]
    pub fn enter_turn_basic(&mut self, gobj: GObjId) {
        let frames = self.game.fighter(gobj).co_attrs.standing_turn_frames;
        self.enter_turn(gobj, ms::TURN, 0, 0.0, frames, 0.0);
    }

    /// A turn from a smashed stick, which dashes once it turns, `ftCo_Turn_Enter_Smash`.
    #[doc(alias = "ftCo_Turn_Enter_Smash")]
    pub fn enter_turn_smash(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let facing = fp.facing_dir;
        fp.set_mv_i32(TURN_HAS_TURNED, 0);
        fp.set_mv_i32(TURN_JUST_TURNED, 0);
        fp.set_mv_f32(TURN_FACING_AFTER, fp::fneg(reg(fp.facing_dir)));
        fp.set_mv_f32(TURN_FRAMES, 0.0);
        fp.set_mv_f32(TURN_X8, reg(facing));
        fp.set_mv_i32(TURN_X1C, 0);
        self.change_motion_state(gobj, ms::TURN, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftCo_Turn_IASA`: the checks run as if already facing the new way; A and B pressed
    /// before the turn carry over to the frame it turns; a smash during the turn dashes.
    #[doc(alias = "ftCo_Turn_IASA")]
    pub fn co_turn_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(TURN_JUST_TURNED) != 0 {
            fp.input.pressed_buttons |= fp.mv_word(TURN_X1C);
        }
        let flip = |g: &mut Running<'_>| {
            let fp = g.game.fighter_mut(gobj);
            if fp.mv_i32(TURN_HAS_TURNED) == 0 {
                fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            }
        };
        flip(self);
        if self.special_s_check(gobj)
            || self.special_lw_check(gobj)
            || self.special_hi_check(gobj)
            || self.catch_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
        {
            return;
        }
        flip(self);
        if self.guard_check(gobj) || self.appeal_check(gobj) || self.jump_check(gobj) {
            return;
        }
        self.turn_dash_input(gobj);
        let dash = reg(self.common().f32(0x3C));
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(TURN_JUST_TURNED) != 0
            && fp.mv_f32(TURN_X8) != 0.0
            && fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.mv_f32(TURN_FACING_AFTER))) >= dash
        {
            self.enter_dash(gobj, 0);
        }
        let fp = self.game.fighter_mut(gobj);
        let pressed = fp.input.pressed_buttons;
        let mut x1c = fp.mv_word(TURN_X1C);
        if pressed & PAD_A != 0 {
            x1c |= PAD_A;
            fp.set_mv_i32(TURN_X1C, x1c as i32);
        }
        if pressed & PAD_B != 0 {
            x1c |= PAD_B;
            fp.set_mv_i32(TURN_X1C, x1c as i32);
        }
        if fp.mv_i32(TURN_JUST_TURNED) != 0 {
            fp.set_mv_i32(TURN_JUST_TURNED, 0);
        }
    }

    /// A smash toward the way the turn faces, which makes the turn dash, `fn_800C9C2C`.
    #[doc(alias = "fn_800C9C2C")]
    pub fn turn_dash_input(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (reg(c.f32(0x3C)), c.i32(0x40));
        let fp = self.game.fighter_mut(gobj);
        let after = fp.mv_f32(TURN_FACING_AFTER);
        if fp::fmuls(reg(fp.input.lstick[0].x), reg(after)) >= t
            && i32::from(fp.active_timer.lstick.x) < window
        {
            fp.set_mv_f32(TURN_X8, reg(after));
            return true;
        }
        false
    }

    /// `ftCo_Turn_Coll`, `ftCo_KneeBend_Coll` and the other grounded states that fall off
    /// ledges: `ft_80083F88`.
    #[doc(alias = "ftCo_Turn_Coll")]
    #[doc(alias = "ftCo_KneeBend_Coll")]
    #[doc(alias = "ftCo_Squat_Coll")]
    #[doc(alias = "ftCo_SquatWait_Coll")]
    #[doc(alias = "ftCo_SquatRv_Coll")]
    #[doc(alias = "ftCo_DownWait_Coll")]
    #[doc(alias = "ftCo_DownStand_Coll")]
    #[doc(alias = "ftCo_DownSpot_Coll")]
    #[doc(alias = "ftCo_Passive_Coll")]
    pub fn co_fall_off_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }

    /// Turning out of a run when the stick points back, on from the brake's frame,
    /// `fn_800C9CEC`.
    #[doc(alias = "fn_800C9CEC")]
    pub fn turn_run_check_keep_frame(&mut self, gobj: GObjId) -> bool {
        if self.turn_run_input(gobj) {
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.enter_turn_run(gobj, frame);
            return true;
        }
        false
    }

    /// Turning out of a run when the stick points back, `fn_800C9D40`.
    #[doc(alias = "fn_800C9D40")]
    pub fn turn_run_check(&mut self, gobj: GObjId) -> bool {
        if self.turn_run_input(gobj) {
            self.enter_turn_run(gobj, 0.0);
            return true;
        }
        false
    }

    fn turn_run_input(&self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x38));
        let fp = self.game.fighter(gobj);
        fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) <= t
    }

    /// Turning out of a run, `ftCo_TurnRun_Enter`.
    #[doc(alias = "ftCo_TurnRun_Enter")]
    pub fn enter_turn_run(&mut self, gobj: GObjId, anim_start: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.set_mv_f32(TURN_RUN_ACCEL_MUL, reg(fp.facing_dir));
        // `Ft_MF_SkipAnimVel`.
        self.change_motion_state(gobj, ms::TURN_RUN, 1 << 5, anim_start, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(TURN_RUN_X14, 0);
        if fp.x197C.is_some() {
            self.play_sfx(gobj, 0x119, 0x7F, 0x40);
        }
    }

    /// `ftCo_TurnRun_Anim`: the animation holds while the fighter slides, and turns it once
    /// it stops; at its end the fighter runs the new way or waits.
    #[doc(alias = "ftCo_TurnRun_Anim")]
    pub fn co_turn_run_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[1] != 0 {
            if fp.mv_i32(TURN_RUN_X14) == 0 {
                self.set_anim_rate(gobj, 0.0);
                self.game.fighter_mut(gobj).set_mv_i32(TURN_RUN_X14, 1);
            } else if fp::fmuls(reg(fp.mv_f32(0xC)), reg(fp.gr_vel)) <= reg(0.01f32) {
                self.set_anim_rate(gobj, 1.0);
                let fp = self.game.fighter_mut(gobj);
                fp.cmd_vars[1] = 0;
                fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            }
        }
        if !self.anim_frames_remaining(gobj) && !self.run_check_after_turn(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_TurnRun_IASA`: a jump.
    #[doc(alias = "ftCo_TurnRun_IASA")]
    pub fn co_turn_run_iasa(&mut self, gobj: GObjId) {
        self.jump_check_relaxed(gobj);
    }

    /// `ftCo_TurnRun_Phys`: accelerating the new way only once the stick points it, braking
    /// otherwise.
    #[doc(alias = "ftCo_TurnRun_Phys")]
    pub fn co_turn_run_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.common().f32(0x60));
        let fp = &mut self.game.fighters[f.0];
        let (accel, target) = Self::dash_accel_and_target(fp);
        let friction = reg(fp.co_attrs.ground_friction);
        if !(target != 0.0) {
            Self::ground_accel_deaccel(fp, fp::fmuls(friction, mul));
        } else if fp::fmuls(reg(fp.mv_f32(TURN_RUN_ACCEL_MUL)), reg(accel)) < 0.0 {
            let (v, target) = (reg(fp.gr_vel), reg(target));
            let mut accel = reg(accel);
            if accel > 0.0 {
                if fp::fadds(v, accel) > target {
                    accel = mem_r(fp::fnmsubs(friction, mul, accel));
                    if fp::fadds(v, accel) < target {
                        accel = mem_r(fp::fsubs(target, v));
                    }
                }
            } else if fp::fadds(v, accel) < target {
                accel = mem_r(fp::fmadds(friction, mul, accel));
                if fp::fadds(v, accel) > target {
                    accel = mem_r(fp::fsubs(target, v));
                }
            }
            fp.xE4_ground_accel_1 = mem(accel);
        } else {
            Self::ground_accel_deaccel(fp, fp::fmuls(friction, mul));
        }
        self.self_movement_from_ground(gobj);
    }

    /// `ftCo_TurnRun_Coll`: off a ledge the fighter falls; at an edge it stops dead
    /// (`ftCommon_8007E2FC`).
    #[doc(alias = "ftCo_TurnRun_Coll")]
    pub fn co_turn_run_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
            return;
        }
        if self.game.fighter(gobj).coll_data.env_flags & (LEFT_EDGE | RIGHT_EDGE) != 0 {
            self.stop_dead(gobj);
        }
    }

    /// Clears every velocity and acceleration, `ftCommon_8007E2FC`.
    #[doc(alias = "ftCommon_8007E2FC")]
    pub fn stop_dead(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.xE4_ground_accel_1 = 0.0;
        fp.xE8_ground_accel_2 = 0.0;
        fp.x74_self_accel.z = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
        fp.gr_vel = 0.0;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.xF0_ground_kb_vel = 0.0;
        fp.x8c_kb_vel.z = 0.0;
        fp.x8c_kb_vel.y = 0.0;
        fp.x8c_kb_vel.x = 0.0;
        fp.xF4_ground_attacker_shield_kb_vel = 0.0;
        fp.x98_atk_shield_kb.z = 0.0;
        fp.x98_atk_shield_kb.y = 0.0;
        fp.x98_atk_shield_kb.x = 0.0;
        fp.xD4_unk_vel.z = 0.0;
        fp.xD4_unk_vel.y = 0.0;
        fp.xD4_unk_vel.x = 0.0;
    }
}

/// A single-precision value back in a register, as the original's stack slot holds it.
fn mem_r(r: f64) -> f64 {
    reg(mem(r))
}
