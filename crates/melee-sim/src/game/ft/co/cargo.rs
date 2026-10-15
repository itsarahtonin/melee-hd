//! Carrying a fighter on the shoulders after a forward throw (`ftCo_Cargo*.c`, `ftCo_09C4.c`,
//! `ftCo_CargoThrow.c`) and being carried (`ftCo_Shouldered.c`). The carrier's states are its
//! character's own (Donkey Kong's `ThrowF*`, 351 to 368), counted from the first one its cargo
//! attributes name (`x2CC`'s `x4_motion_state`): wait, three walks, turn, knee bend, fall,
//! jump, landing, the hurt wait, then the four throws on the ground and in the air. The
//! carried fighter follows in the shouldered states, mashing to break free.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::parts::TRANS_N2;
use super::checks::abs;
use super::ids::ms;
use super::{PAD_A, PAD_B};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;

/// The cargo attributes' offsets (`ftDonkeyAttributes`, which `x2CC` points to): the first
/// state, the walks' animation frames and speed scales, the turn's frames, the knee bend's and
/// the landing's lag.
const X4_MOTION_STATE: usize = 0x4;
const WALK_FRAMES: usize = 0x8;
const WALK_RATES: usize = 0x14;
const TURN_SPEED: usize = 0x20;
const JUMP_STARTUP_LAG: usize = 0x24;
const LANDING_LAG: usize = 0x28;

/// The states past the first: walks, turn, knee bend, fall, jump, landing, the throws.
const WALK: i32 = 1;
const TURN: i32 = 4;
const KNEE_BEND: i32 = 5;
const FALL: i32 = 6;
const JUMP: i32 = 7;
const LANDING: i32 = 8;
const THROW_F: i32 = 10;
const THROW_B: i32 = 11;
const THROW_HI: i32 = 12;
const THROW_LW: i32 = 13;
/// An aerial throw's state is its grounded one's plus this.
const THROW_AIR: i32 = 4;

/// `ftCommon_MotionVars_cargoturn::x0` (has turned) and the turn's frames (the turn's `x10`).
const TURN_HAS_TURNED: usize = 0x0;
const TURN_FRAMES: usize = 0x10;
/// The knee bend's lag left (`cargokneebend.x8`) and the landing's (`mv` x4).
const KNEE_BEND_INPUT: usize = 0x4;
const KNEE_BEND_LAG: usize = 0x8;
const KNEE_BEND_X0: usize = 0x0;
const LANDING_LAG_LEFT: usize = 0x4;
/// `ftCommon_MotionVars_shouldered`: the frames the mash's animation speed lasts, and whether
/// the last frame mashed.
const SHOULDERED_TIMER: usize = 0x0;
const SHOULDERED_MASHED: usize = 0x4;

/// The flags a cargo throw keeps between the ground and the air.
const THROW_COLL_FLAGS: u32 = 0x0C4C_5080;
/// `Ft_MF_FreezeState`.
const MF_FREEZE_STATE: u32 = 0x20_0000;
/// The held fighter's follow callback (`ftCo_800DB464`).
const HELD_FOLLOW: u32 = 0x800D_B464;

impl Running<'_> {
    /// A float of the fighter's cargo attributes (`x2CC`).
    fn cargo_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        let a = self.game.fighters[f.0]
            .x2CC
            .expect("a cargo fighter without cargo attributes");
        let b = &self.game.fighter_attrs[a.0];
        f32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// One of the carrier's states, `step` past the first.
    fn cargo_state(&self, gobj: GObjId, step: i32) -> i32 {
        let f = self.game.fighter_id(gobj);
        let a = self.game.fighters[f.0]
            .x2CC
            .expect("a cargo fighter without cargo attributes");
        let b = &self.game.fighter_attrs[a.0];
        i32::from_be_bytes(b[X4_MOTION_STATE..X4_MOTION_STATE + 4].try_into().unwrap())
            .wrapping_add(step)
    }

    /// The carried fighter.
    fn cargo_victim(&self, gobj: GObjId) -> GObjId {
        self.game
            .fighter(gobj)
            .victim_gobj
            .expect("carrying no one")
    }

    /// The stick pulled back or within the walk's threshold, `ft_8008A1FC`.
    #[doc(alias = "ft_8008A1FC")]
    pub fn stick_not_forward(&self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x24));
        let fp = self.game.fighter(gobj);
        let x = reg(fp.input.lstick[0].x);
        fp::fmuls(x, reg(fp.facing_dir)) < 0.0 || abs(x) < t
    }

    /// Back to the cargo wait when the stick lets go, `ftCo_8009B4D0`.
    #[doc(alias = "ftCo_8009B4D0")]
    pub fn cargo_wait_check(&mut self, gobj: GObjId) -> bool {
        if self.stick_not_forward(gobj) {
            self.enter_cargo_wait(gobj);
            return true;
        }
        false
    }

    /// The cargo wait, `ftCo_8009B518`, the carried fighter waiting too.
    #[doc(alias = "ftCo_8009B518")]
    pub fn enter_cargo_wait(&mut self, gobj: GObjId) {
        let msid = self.cargo_state(gobj, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// The cargo wait after the forward throw, `ftCo_8009B56C`: the carried fighter's mash
    /// timer starts.
    #[doc(alias = "ftCo_8009B56C")]
    pub fn enter_cargo_start(&mut self, gobj: GObjId) {
        let msid = self.cargo_state(gobj, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered_start(v, ms::SHOULDERED_WAIT);
    }

    /// `ftCo_CargoWait_Anim`, `ftCo_CargoJump_Anim` and `ftCo_CargoFall_Anim`: nothing.
    #[doc(alias = "ftCo_CargoWait_Anim")]
    #[doc(alias = "ftCo_CargoJump_Anim")]
    #[doc(alias = "ftCo_CargoFall_Anim")]
    #[doc(alias = "ftCo_CargoThrowAir_IASA")]
    #[doc(alias = "ftCo_Shouldered_IASA")]
    #[doc(alias = "ftCo_Shouldered_Phys")]
    #[doc(alias = "ftCo_Shouldered_Coll")]
    pub fn co_cargo_nothing(&mut self, _gobj: GObjId) {}

    /// `ftCo_CargoWait_IASA`: a throw, a jump, a drop through a platform, a turn or a walk.
    #[doc(alias = "ftCo_CargoWait_IASA")]
    pub fn co_cargo_wait_iasa(&mut self, gobj: GObjId) {
        let _ = self.cargo_throw_check(gobj)
            || self.cargo_jump_check(gobj)
            || self.cargo_pass_check(gobj)
            || self.cargo_turn_check(gobj)
            || self.cargo_walk_check(gobj);
    }

    /// The grounded cargo states' physics: friction or the animation's motion.
    #[doc(alias = "ftCo_CargoWait_Phys")]
    #[doc(alias = "ftCo_CargoTurn_Phys")]
    #[doc(alias = "ftCo_CargoKneebend_Phys")]
    #[doc(alias = "ftCo_CargoLanding_Phys")]
    pub fn co_cargo_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The grounded cargo states' collision: off a ledge, the cargo fall.
    #[doc(alias = "ftCo_CargoWait_Coll")]
    #[doc(alias = "ftCo_CargoWalk_Coll")]
    #[doc(alias = "ftCo_CargoTurn_Coll")]
    #[doc(alias = "ftCo_CargoKneebend_Coll")]
    #[doc(alias = "ftCo_CargoLanding_Coll")]
    pub fn co_cargo_wait_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.enter_cargo_fall(gobj));
    }

    /// Walking when the stick leans forward, `ftCo_8009B67C`.
    #[doc(alias = "ftCo_8009B67C")]
    pub fn cargo_walk_check(&mut self, gobj: GObjId) -> bool {
        if self.walk_input(gobj) {
            self.enter_cargo_walk(gobj, 0.0);
            return true;
        }
        false
    }

    /// A cargo walk at the speed the ground speed calls for, `ftCo_8009B6C8`, the carried
    /// fighter walking with it.
    #[doc(alias = "ftCo_8009B6C8")]
    pub fn enter_cargo_walk(&mut self, gobj: GObjId, anim_start: f32) {
        let f = self.game.fighter_id(gobj);
        let msid = self.cargo_state(gobj, WALK);
        let at = |g: &Self, base: usize| [0, 1, 2].map(|i| g.cargo_attr_f32(f, base + 4 * i));
        let (frames, rates) = (at(self, WALK_FRAMES), at(self, WALK_RATES));
        self.enter_walk_common(gobj, msid, 0, anim_start, frames, rates, 1.0);
        let walk = self.walk_type(gobj);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, walk.wrapping_add(ms::SHOULDERED_WAIT));
    }

    /// `ftCo_CargoWalk_Anim`: the walk's animation speed follows the ground speed.
    #[doc(alias = "ftCo_CargoWalk_Anim")]
    pub fn co_cargo_walk_anim(&mut self, gobj: GObjId) {
        self.co_walk_anim(gobj);
    }

    /// `ftCo_CargoWalk_IASA`: a throw, a jump, a drop through, the wait when the stick lets
    /// go, else the walk's speed switch.
    #[doc(alias = "ftCo_CargoWalk_IASA")]
    pub fn co_cargo_walk_iasa(&mut self, gobj: GObjId) {
        if self.cargo_throw_check(gobj)
            || self.cargo_jump_check(gobj)
            || self.cargo_pass_check(gobj)
            || self.cargo_wait_check(gobj)
        {
            return;
        }
        self.walk_speed_switch(gobj, |g, gobj, frame| g.enter_cargo_walk(gobj, frame));
    }

    /// `ftCo_CargoWalk_Phys`: the walk's acceleration.
    #[doc(alias = "ftCo_CargoWalk_Phys")]
    pub fn co_cargo_walk_phys(&mut self, gobj: GObjId) {
        self.co_walk_phys(gobj);
    }

    /// Turning when the stick points back, `ftCo_8009B818`.
    #[doc(alias = "ftCo_8009B818")]
    pub fn cargo_turn_check(&mut self, gobj: GObjId) -> bool {
        if self.turn_input(gobj) {
            self.enter_cargo_turn(gobj);
            return true;
        }
        false
    }

    /// The cargo turn, `ftCo_8009B860`, over `TURN_SPEED` frames, the carried fighter
    /// turning too.
    #[doc(alias = "ftCo_8009B860")]
    pub fn enter_cargo_turn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let msid = self.cargo_state(gobj, TURN);
        let frames = self.cargo_attr_f32(f, TURN_SPEED);
        self.enter_turn(gobj, msid, 0, 0.0, frames, 0.0);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_TURN);
    }

    /// `ftCo_CargoTurn_Anim`: after the turn's frames, both face the other way once; the wait
    /// when it ends.
    #[doc(alias = "ftCo_CargoTurn_Anim")]
    pub fn co_cargo_turn_anim(&mut self, gobj: GObjId) {
        let v = self.cargo_victim(gobj);
        let fp = self.game.fighter_mut(gobj);
        let frames = reg(fp.mv_f32(TURN_FRAMES));
        if frames > 0.0 {
            fp.set_mv_f32(TURN_FRAMES, fp::fsubs(frames, 1.0));
        } else if fp.mv_i32(TURN_HAS_TURNED) == 0 {
            fp.set_mv_i32(TURN_HAS_TURNED, 1);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            let facing = fp.facing_dir;
            self.game.fighter_mut(v).facing_dir = facing;
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_cargo_wait(gobj);
        }
    }

    /// `ftCo_CargoTurn_IASA`: a throw or a jump.
    #[doc(alias = "ftCo_CargoTurn_IASA")]
    pub fn co_cargo_turn_iasa(&mut self, gobj: GObjId) {
        let _ = self.cargo_throw_check(gobj) || self.cargo_jump_check(gobj);
    }

    /// The cargo knee bend, `ftCo_8009B9C8`: `input` the jump's input, held for
    /// `JUMP_STARTUP_LAG` frames with the animation stopped.
    #[doc(alias = "ftCo_8009B9C8")]
    pub fn enter_cargo_knee_bend(&mut self, gobj: GObjId, input: i32) {
        let f = self.game.fighter_id(gobj);
        let lag = self.cargo_attr_f32(f, JUMP_STARTUP_LAG);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(KNEE_BEND_INPUT, input);
        fp.set_mv_i32(KNEE_BEND_X0, 0);
        fp.set_mv_f32(KNEE_BEND_LAG, reg(lag));
        let msid = self.cargo_state(gobj, KNEE_BEND);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// `ftCo_CargoKneebend_Anim`: the jump once the lag runs out.
    #[doc(alias = "ftCo_CargoKneebend_Anim")]
    pub fn co_cargo_knee_bend_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_f32(KNEE_BEND_LAG) <= 0.0 {
            self.enter_cargo_jump(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let lag = reg(fp.mv_f32(KNEE_BEND_LAG));
        fp.set_mv_f32(KNEE_BEND_LAG, fp::fsubs(lag, 1.0));
    }

    /// `ftCo_CargoKneebend_IASA`: a throw, else a short hop's input.
    #[doc(alias = "ftCo_CargoKneebend_IASA")]
    pub fn co_cargo_knee_bend_iasa(&mut self, gobj: GObjId) {
        if !self.cargo_throw_check(gobj) {
            self.knee_bend_short_hop_check(gobj);
        }
    }

    /// The cargo knee bend on a jump's input, `ftCo_8009BB1C`.
    #[doc(alias = "ftCo_8009BB1C")]
    pub fn cargo_jump_check(&mut self, gobj: GObjId) -> bool {
        let input = self.jump_input(gobj);
        if input != 0 {
            self.enter_cargo_knee_bend(gobj, input);
            return true;
        }
        false
    }

    /// The cargo jump, `ftCo_8009BB64`: airborne, its animation stopped, the jump's takeoff.
    #[doc(alias = "ftCo_8009BB64")]
    pub fn enter_cargo_jump(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.cargo_state(gobj, JUMP);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
        self.jump_take_off(gobj, true, 1.0);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// The airborne cargo states' interrupt: a throw.
    #[doc(alias = "ftCo_CargoJump_IASA")]
    #[doc(alias = "ftCo_CargoFall_IASA")]
    pub fn co_cargo_air_iasa(&mut self, gobj: GObjId) {
        let _ = self.cargo_throw_check(gobj);
    }

    /// `ftCo_CargoJump_Phys`: the jump's physics (`ftCo_Jump_Phys_Inner`).
    #[doc(alias = "ftCo_CargoJump_Phys")]
    #[doc(alias = "ftCo_Jump_Phys_Inner")]
    pub fn co_cargo_jump_phys(&mut self, gobj: GObjId) {
        self.co_jump_phys(gobj);
    }

    /// The airborne cargo states' collision: landing, the cargo landing.
    #[doc(alias = "ftCo_CargoJump_Coll")]
    #[doc(alias = "ftCo_CargoFall_Coll")]
    pub fn co_cargo_fall_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_cargo_landing(gobj));
    }

    /// The cargo fall, `ftCo_8009BC58`, keeping a fast fall, its animation stopped; airborne
    /// if it starts on the ground.
    #[doc(alias = "ftCo_8009BC58")]
    pub fn enter_cargo_fall(&mut self, gobj: GObjId) {
        let msid = self.cargo_state(gobj, FALL);
        self.change_motion_state(gobj, msid, 1, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
        if !self.is_airborne(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// `ftCo_CargoFall_Phys`: the air's physics.
    #[doc(alias = "ftCo_CargoFall_Phys")]
    #[doc(alias = "ftCo_CargoThrowAir_Phys")]
    pub fn co_cargo_fall_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// The cargo landing, `ftCo_8009BD4C`: `LANDING_LAG` frames with its animation stopped.
    #[doc(alias = "ftCo_8009BD4C")]
    pub fn enter_cargo_landing(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.cargo_attr_f32(f, LANDING_LAG);
        self.game.fighters[f.0].set_mv_f32(LANDING_LAG_LEFT, reg(lag));
        let msid = self.cargo_state(gobj, LANDING);
        self.enter_landing(gobj, msid, 1, 0, 0.0, 1.0);
        self.set_anim_rate(gobj, 0.0);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// `ftCo_CargoLanding_Anim`: the wait once the lag runs out.
    #[doc(alias = "ftCo_CargoLanding_Anim")]
    pub fn co_cargo_landing_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_f32(LANDING_LAG_LEFT) <= 0.0 {
            self.enter_cargo_wait(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let lag = reg(fp.mv_f32(LANDING_LAG_LEFT));
        fp.set_mv_f32(LANDING_LAG_LEFT, fp::fsubs(lag, 1.0));
    }

    /// Dropping through a platform, `ftCo_8009C4F8`.
    #[doc(alias = "ftCo_8009C4F8")]
    pub fn cargo_pass_check(&mut self, gobj: GObjId) -> bool {
        if self.pass_input(gobj) {
            self.enter_cargo_pass(gobj);
            return true;
        }
        false
    }

    /// The drop through a platform as the cargo fall, `ftCo_8009C540`.
    #[doc(alias = "ftCo_8009C540")]
    pub fn enter_cargo_pass(&mut self, gobj: GObjId) {
        let msid = self.cargo_state(gobj, FALL);
        self.enter_pass_as(gobj, msid, 0, 0.0);
        self.set_anim_rate(gobj, 0.0);
        let v = self.cargo_victim(gobj);
        self.enter_shouldered(v, ms::SHOULDERED_WAIT);
    }

    /// `ftCo_CargoWait2_Anim`: the hurt wait's hitstun, then the cargo fall or wait.
    #[doc(alias = "ftCo_CargoWait2_Anim")]
    pub fn co_cargo_wait2_anim(&mut self, gobj: GObjId) {
        self.damage_tick(gobj);
        if !self.game.fighter(gobj).x221C_b6 {
            if self.is_airborne(gobj) {
                self.enter_cargo_fall(gobj);
            } else {
                self.enter_cargo_wait(gobj);
            }
        }
    }

    /// `ftCo_CargoWait2_Phys`: the damage state's physics.
    #[doc(alias = "ftCo_CargoWait2_Phys")]
    pub fn co_cargo_wait2_phys(&mut self, gobj: GObjId) {
        self.co_damage_phys(gobj);
    }

    /// `ftCo_CargoWait2_Coll`: off the ground, airborne; in the air, landing.
    #[doc(alias = "ftCo_CargoWait2_Coll")]
    pub fn co_cargo_wait2_coll(&mut self, gobj: GObjId) {
        if !self.is_airborne(gobj) {
            if !self.ground_coll_off_ledges(gobj) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne(f);
            }
        } else if self.air_coll_land(gobj) {
            self.land(gobj);
        }
    }

    /// The throw A or B asks for (`inlineA0`): forward or back by the stick's side past
    /// `ftCommonData` x98, else up or down past its thresholds; the state the fighter is in
    /// if none.
    fn cargo_throw_input(&self, gobj: GObjId) -> i32 {
        let c = self.common();
        let (side, up, down) = (reg(c.f32(0x98)), reg(c.f32(0xAC)), reg(c.f32(0xB0)));
        let fp = self.game.fighter(gobj);
        let mut msid = fp.motion_id;
        if fp.input.pressed_buttons & (PAD_A | PAD_B) != 0 {
            let (x, y) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
            let step = if abs(x) >= side {
                if fp::fmuls(x, reg(fp.facing_dir)) > 0.0 {
                    Some(THROW_F)
                } else {
                    Some(THROW_B)
                }
            } else if y >= up {
                Some(THROW_HI)
            } else if y <= down {
                Some(THROW_LW)
            } else {
                None
            };
            if let Some(s) = step {
                msid = self.cargo_state(gobj, s);
            }
        }
        msid
    }

    /// A cargo throw on A or B, `ftCo_8009BF3C`.
    #[doc(alias = "ftCo_8009BF3C")]
    pub fn cargo_throw_check(&mut self, gobj: GObjId) -> bool {
        let msid = self.cargo_throw_input(gobj);
        if msid != self.game.fighter(gobj).motion_id {
            self.enter_cargo_throw(gobj, msid);
            return true;
        }
        false
    }

    /// A cargo throw, `ftCo_8009C02C`: its aerial state in the air, the carried fighter into
    /// the matching thrown state at full speed.
    #[doc(alias = "ftCo_8009C02C")]
    pub fn enter_cargo_throw(&mut self, gobj: GObjId, msid: i32) {
        let air = self.is_airborne(gobj);
        let (state, flags) = if air {
            (msid.wrapping_add(THROW_AIR), 1)
        } else {
            (msid, 0)
        };
        self.change_motion_state(gobj, state, flags, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        let first = self.cargo_state(gobj, THROW_F);
        let thrown = msid.wrapping_sub(first).wrapping_add(ms::THROWN_FF);
        let v = self.cargo_victim(gobj);
        self.enter_thrown(v, thrown, 1.0);
    }

    /// `ftCo_CargoThrowF_Anim` and `ftCo_CargoThrowAir_Anim`: the throw's progress, then
    /// falling or waiting.
    #[doc(alias = "ftCo_CargoThrowF_Anim")]
    #[doc(alias = "ftCo_CargoThrowAir_Anim")]
    pub fn co_cargo_throw_f_anim(&mut self, gobj: GObjId) {
        self.throw_progress(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// A grounded cargo throw from the same frame in the air (`ftCo_8009C170`) or an aerial
    /// one on the ground (`ftCo_8009C45C`), facing as at the throw's start for the change.
    fn cargo_throw_switch(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if air {
            self.go_airborne(f);
        } else {
            self.land(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        let msid = fp
            .motion_id
            .wrapping_add(if air { THROW_AIR } else { -THROW_AIR });
        let facing = fp.facing_dir;
        if facing != fp.facing_dir1 {
            fp.facing_dir = fp.facing_dir1;
        }
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, msid, THROW_COLL_FLAGS, frame, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = facing;
        fp.x1A6A = 0x1FF;
    }

    /// `ftCo_8009C170`.
    #[doc(alias = "ftCo_8009C170")]
    pub fn cargo_throw_to_air(&mut self, gobj: GObjId) {
        self.cargo_throw_switch(gobj, true);
    }

    /// `ftCo_8009C45C`.
    #[doc(alias = "ftCo_8009C45C")]
    pub fn cargo_throw_to_ground(&mut self, gobj: GObjId) {
        self.cargo_throw_switch(gobj, false);
    }

    /// The grounded cargo throws' collision: off a ledge, the aerial throw.
    #[doc(alias = "ftCo_CargoThrowF_Coll")]
    #[doc(alias = "ftCo_CargoThrowB_Coll")]
    #[doc(alias = "ftCo_CargoThrowHi_Coll")]
    #[doc(alias = "ftCo_CargoThrowLw_Coll")]
    pub fn co_cargo_throw_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.cargo_throw_to_air(gobj));
    }

    /// `ftCo_CargoThrowAir_Coll`: landing, the grounded throw.
    #[doc(alias = "ftCo_CargoThrowAir_Coll")]
    pub fn co_cargo_throw_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.cargo_throw_to_ground(gobj));
    }

    /// The carried fighter into `msid` (`ftCo_8009C5A4`) unless it waits on and on: frozen
    /// with its carrier when `x2222_b6` says, still, and following the carrier's throw bone.
    #[doc(alias = "ftCo_8009C5A4")]
    pub fn enter_shouldered(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter(gobj);
        if fp.motion_id == msid && msid == ms::SHOULDERED_WAIT {
            return;
        }
        let flags = if fp.x2222_b6 { MF_FREEZE_STATE } else { 0 };
        let holder = fp.victim_gobj;
        self.change_motion_state(gobj, msid, flags, 0.0, 1.0, 0.0, holder);
        self.stop_dead(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x1A6A = 0x1FF;
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
    }

    /// Shouldered after the forward throw, `ftCo_8009C640`: its mash timer by its damage
    /// (`ftCommonData` x4A0 per percent plus x4A4, in whole frames), then
    /// [`Self::enter_shouldered`].
    #[doc(alias = "ftCo_8009C640")]
    pub fn enter_shouldered_start(&mut self, gobj: GObjId, msid: i32) {
        let c = self.common();
        let (per, base) = (reg(c.f32(0x4A0)), reg(c.f32(0x4A4)));
        let fp = self.game.fighter_mut(gobj);
        let timer = fp::fctiwz(fp::fmadds(reg(fp.dmg.x1830_percent), per, base));
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(fp::frsp(f64::from(timer)));
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        fp.set_mv_f32(SHOULDERED_TIMER, 0.0);
        fp.set_mv_i32(SHOULDERED_MASHED, 0);
        self.enter_shouldered(gobj, msid);
    }

    /// The knockback a fighter's throw hitbox `hit` deals `me` (`ftColl_80079C70` for a hit
    /// capsule held in the attacker's throw hitboxes).
    fn shouldered_knockback(
        &self,
        me: FighterId,
        attacker: FighterId,
        hit: &super::super::super::structs::HitCapsule,
    ) -> f64 {
        let fp = &self.game.fighters[me.0];
        let defense = self.game.player_defense_ratio(i32::from(fp.player_idx));
        let attack = self
            .game
            .player_attack_ratio(i32::from(self.game.fighters[attacker.0].player_idx));
        let ratio = self.game.vs_damage_ratio();
        self.knockback_of(
            me,
            hit,
            hit.unk_count,
            ratio,
            attack,
            defense,
            fp.co_attrs.weight,
        )
    }

    /// `me` hit by `attacker`'s second throw hitbox at `joint`'s place (the carried fighter's
    /// escape): knockback, angle, facing `facing`, the hit recorded as the carrier's on the
    /// carried fighter (`ftColl_80078710(holder, held)`, both ways round), its damage, hitstun.
    fn shouldered_escape_hit(
        &mut self,
        me: GObjId,
        attacker: GObjId,
        joint: i32,
        facing: f32,
        (holder, held): (GObjId, GObjId),
    ) {
        let (m, a) = (self.game.fighter_id(me), self.game.fighter_id(attacker));
        let hit = self.game.fighters[a.0].xDF4[1].clone();
        let j = self.part_joint(me, joint);
        let pos = world_position(&mut self.game.objs, self.data, j, None);
        let kb = self.shouldered_knockback(m, a, &hit);
        let fp = &mut self.game.fighters[m.0];
        fp.dmg.kb_applied = mem(kb);
        fp.dmg.x1848_kb_angle = hit.kb_angle;
        fp.dmg.facing_dir_1 = facing;
        fp.dmg.x184c_damaged_hurtbox = 1;
        fp.dmg.x1854_collpos = pos;
        fp.dmg.x1860_element = hit.element;
        // `ftColl_80078710(holder, held, &facing_dir_1)`.
        let af = self.game.fighter(holder);
        let (akind, x2070, instance) = (af.kind, u32::from_be_bytes(af.x2070), af.x2074.x2088);
        self.attack_landed(
            Some(holder),
            held,
            1,
            akind,
            x2070,
            super::super::super::pl::trick::AttackRecord::Fighter(holder),
            instance,
            true,
            false,
        );
        self.take_damage_coins(m, reg(hit.damage));
        self.calc_knockback(m);
        self.damage_reaction(me, 0.0);
    }

    /// The carried fighter breaking free, `ftCo_8009C744`: let go of, and knocked back by
    /// its carrier's second throw hitbox from its XRotN, facing away from it.
    #[doc(alias = "ftCo_8009C744")]
    pub fn shouldered_break_free(&mut self, gobj: GObjId) {
        let holder = self.cargo_victim(gobj);
        self.grab_release(holder, gobj);
        let facing = mem(fp::fneg(reg(self.game.fighter(holder).facing_dir)));
        self.shouldered_escape_hit(
            gobj,
            holder,
            super::super::parts::XROT_N,
            facing,
            (holder, gobj),
        );
    }

    /// The shouldered states' animation, `ftCo_Shouldered_Anim`: a mash by A, B, the stick,
    /// worth `ftCommonData` x4A8 (times x4AC with the carrier in the air); at the timer's end
    /// the carrier is knocked back by its victim's second throw hitbox and the victim breaks
    /// free; while mashing, its animation sped up for `x3B0` frames.
    #[doc(alias = "ftCo_Shouldered_Anim")]
    pub fn co_shouldered_anim(&mut self, gobj: GObjId) {
        let c = self.common();
        let (x4a8, x4ac) = (reg(c.f32(0x4A8)), reg(c.f32(0x4AC)));
        let holder = self.cargo_victim(gobj);
        let amount = if self.is_airborne(holder) {
            fp::fmuls(x4a8, x4ac)
        } else {
            x4a8
        };
        let mashed = self.grab_mash(gobj, mem(amount));
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(SHOULDERED_MASHED, i32::from(mashed));
        if self.game.fighter(gobj).grab_timer <= 0.0 {
            let facing = self.game.fighter(holder).facing_dir;
            self.shouldered_escape_hit(holder, gobj, TRANS_N2, facing, (holder, gobj));
            self.shouldered_break_free(gobj);
            return;
        }
        let (x3b0, rate) = (c.f32(0x3B0), c.f32(0x3B4));
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(SHOULDERED_TIMER) != 0.0 {
            let t = fp::fsubs(reg(fp.mv_f32(SHOULDERED_TIMER)), 1.0);
            fp.set_mv_f32(SHOULDERED_TIMER, t);
            if fp.mv_f32(SHOULDERED_TIMER) <= 0.0 && fp.mv_i32(SHOULDERED_MASHED) == 0 {
                self.set_anim_rate(gobj, 1.0);
                self.game
                    .fighter_mut(gobj)
                    .set_mv_f32(SHOULDERED_TIMER, 0.0);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(SHOULDERED_TIMER) <= 0.0
            && fp.mv_i32(SHOULDERED_MASHED) != 0
            && fp.motion_id == ms::SHOULDERED_WAIT
        {
            fp.set_mv_f32(SHOULDERED_TIMER, reg(x3b0));
            self.set_anim_rate(gobj, rate);
        }
    }
}

/// Runs a cargo or shouldered state's callback, if `cb` is one.
pub fn run(g: &mut Running<'_>, gobj: GObjId, cb: u32) -> bool {
    match cb {
        // Nothing: ftCo_CargoWait_Anim, ftCo_CargoJump_Anim, ftCo_CargoFall_Anim,
        // ftCo_CargoThrowAir_IASA, ftCo_Shouldered_IASA, _Phys and _Coll.
        0x8009_B5C0 | 0x8009_BBF0 | 0x8009_BCDC | 0x8009_C410 | 0x8009_CA00 | 0x8009_CA04
        | 0x8009_CA08 => g.co_cargo_nothing(gobj),
        0x8009_B5C4 => g.co_cargo_wait_iasa(gobj),
        0x8009_B634 | 0x8009_B988 | 0x8009_BADC | 0x8009_BE14 => g.co_cargo_ground_phys(gobj),
        0x8009_B654 | 0x8009_B7F8 | 0x8009_B9A8 | 0x8009_BAFC | 0x8009_BE34 => {
            g.co_cargo_wait_coll(gobj)
        }
        0x8009_B744 => g.co_cargo_walk_anim(gobj),
        0x8009_B764 => g.co_cargo_walk_iasa(gobj),
        0x8009_B7D8 => g.co_cargo_walk_phys(gobj),
        0x8009_B8B4 => g.co_cargo_turn_anim(gobj),
        0x8009_B948 => g.co_cargo_turn_iasa(gobj),
        0x8009_BA50 => g.co_cargo_knee_bend_anim(gobj),
        0x8009_BAA0 => g.co_cargo_knee_bend_iasa(gobj),
        0x8009_BBF4 | 0x8009_BCE0 => g.co_cargo_air_iasa(gobj),
        0x8009_BC18 => g.co_cargo_jump_phys(gobj),
        0x8009_BC38 | 0x8009_BD24 => g.co_cargo_fall_coll(gobj),
        0x8009_BD04 | 0x8009_C414 => g.co_cargo_fall_phys(gobj),
        0x8009_BDC4 => g.co_cargo_landing_anim(gobj),
        0x8009_BE54 => g.co_cargo_wait2_anim(gobj),
        0x8009_BEB8 => g.co_cargo_wait2_phys(gobj),
        0x8009_BED8 => g.co_cargo_wait2_coll(gobj),
        0x8009_C0EC | 0x8009_C3CC => g.co_cargo_throw_f_anim(gobj),
        // ftCo_CargoThrowF_IASA, ftCo_CargoThrowB_IASA, ftCo_CargoThrowHi_IASA,
        // ftCo_CargoThrowLw_IASA: the throws' (empty).
        0x8009_C130 | 0x8009_C254 | 0x8009_C2DC | 0x8009_C364 => g.co_throw_nothing(gobj),
        // ftCo_CargoThrowF_Phys, _B_Phys, _Hi_Phys, _Lw_Phys: the throws'.
        0x8009_C150 | 0x8009_C274 | 0x8009_C2FC | 0x8009_C384 => g.co_throw_phys(gobj),
        // ftCo_CargoThrowB_Anim, _Hi_Anim, _Lw_Anim: the throws'.
        0x8009_C234 | 0x8009_C2BC | 0x8009_C344 => g.co_throw_anim(gobj),
        0x8009_C20C | 0x8009_C294 | 0x8009_C31C | 0x8009_C3A4 => g.co_cargo_throw_coll(gobj),
        0x8009_C434 => g.co_cargo_throw_air_coll(gobj),
        0x8009_C830 => g.co_shouldered_anim(gobj),
        _ => return false,
    }
    true
}
