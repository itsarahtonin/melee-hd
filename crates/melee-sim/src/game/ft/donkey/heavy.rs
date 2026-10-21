//! Donkey Kong carrying a heavy item (`ftdonkeyheavy*.c`, `ftdonkeyms3450.c`): a box, barrel or
//! Party Ball on his shoulders in his own states, 341 to 350, counted from the first one his
//! cargo attributes name (`x2CC`'s `motion_state`; the cargo states after his forward throw
//! count from its `x4_motion_state`): wait, three walks, turn, knee bend, fall, jump, landing
//! and the hurt wait. Unlike
//! the common carry (`ft::co::lift`) he walks, turns, jumps and falls with it, and only a hit
//! hard enough (or a throw) makes him let go.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::it::release;
use super::super::super::state::Running;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The states past the first: walks, turn, knee bend, fall, jump, landing, the hurt wait.
const WALK: i32 = 1;
const TURN: i32 = 4;
const KNEE_BEND: i32 = 5;
const FALL: i32 = 6;
const JUMP: i32 = 7;
const LANDING: i32 = 8;
const WAIT2: i32 = 9;

/// `ftDonkeyAttributes`: the walks' animation frames (`x8` to `x10`) and speed scales (`x14` to
/// `x1C`), and `cargo_hold`'s turn frames, jump startup and landing lag.
const WALK_FRAMES: usize = 0x8;
const WALK_RATES: usize = 0x14;
const TURN_SPEED: usize = 0x20;
const JUMP_STARTUP_LAG: usize = 0x24;
const LANDING_LAG: usize = 0x28;

/// `mv.dk`: the turn's `x0` (has turned, `ftCo_Turn_Enter`'s), the knee bend's jump input
/// (`unk7.x4`), its `x0` and its lag left (`unk7.x8`), and the landing's lag left (`unk8.x4`).
const TURN_HAS_TURNED: usize = 0x0;
const KNEE_BEND_X0: usize = 0x0;
const KNEE_BEND_INPUT: usize = 0x4;
const KNEE_BEND_LAG: usize = 0x8;
const LANDING_LAG_LEFT: usize = 0x4;

/// `Ft_MF_KeepFastFall`.
const MF_KEEP_FAST_FALL: u32 = 1;

/// The states' callbacks.
pub mod addr {
    pub const WAIT0_IASA: u32 = 0x800D_F9F0;
    pub const WAIT0_PHYS: u32 = 0x800D_FA50;
    pub const WAIT0_COLL: u32 = 0x800D_FA70;
    pub const WALK_ANIM: u32 = 0x800D_FB34;
    pub const WALK_IASA: u32 = 0x800D_FB54;
    pub const WALK_PHYS: u32 = 0x800D_FBB8;
    pub const WALK_COLL: u32 = 0x800D_FBD8;
    pub const TURN_ANIM: u32 = 0x800E_01BC;
    pub const TURN_IASA: u32 = 0x800E_0200;
    pub const TURN_PHYS: u32 = 0x800E_0254;
    pub const TURN_COLL: u32 = 0x800E_0274;
    pub const FALL_IASA: u32 = 0x800E_030C;
    pub const FALL_PHYS: u32 = 0x800E_0330;
    pub const FALL_COLL: u32 = 0x800E_0350;
    pub const JUMP_IASA: u32 = 0x800E_0440;
    pub const JUMP_PHYS: u32 = 0x800E_0464;
    pub const JUMP_COLL: u32 = 0x800E_0484;
    pub const WAIT1_ANIM: u32 = 0x800E_0518;
    pub const WAIT1_IASA: u32 = 0x800E_0568;
    pub const WAIT1_PHYS: u32 = 0x800E_05A4;
    pub const WAIT1_COLL: u32 = 0x800E_05C4;
    pub const LANDING_ANIM: u32 = 0x800E_0648;
    pub const LANDING_PHYS: u32 = 0x800E_0698;
    pub const LANDING_COLL: u32 = 0x800E_06B8;
    pub const WAIT2_ANIM: u32 = 0x800E_07E4;
    pub const WAIT2_PHYS: u32 = 0x800E_0848;
    pub const WAIT2_COLL: u32 = 0x800E_0868;
}

/// Runs one of the heavy states' callbacks, if `a` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::WAIT0_IASA => g.dk_heavy_wait0_iasa(gobj),
        addr::WAIT0_PHYS => g.dk_heavy_wait0_phys(gobj),
        addr::WAIT0_COLL => g.dk_heavy_wait0_coll(gobj),
        addr::WALK_ANIM => g.dk_heavy_walk_anim(gobj),
        addr::WALK_IASA => g.dk_heavy_walk_iasa(gobj),
        addr::WALK_PHYS => g.dk_heavy_walk_phys(gobj),
        addr::WALK_COLL => g.dk_heavy_walk_coll(gobj),
        addr::TURN_ANIM => g.dk_heavy_turn_anim(gobj),
        addr::TURN_IASA => g.dk_heavy_turn_iasa(gobj),
        addr::TURN_PHYS => g.dk_heavy_turn_phys(gobj),
        addr::TURN_COLL => g.dk_heavy_turn_coll(gobj),
        addr::FALL_IASA => g.dk_heavy_fall_iasa(gobj),
        addr::FALL_PHYS => g.dk_heavy_fall_phys(gobj),
        addr::FALL_COLL => g.dk_heavy_fall_coll(gobj),
        addr::JUMP_IASA => g.dk_heavy_jump_iasa(gobj),
        addr::JUMP_PHYS => g.dk_heavy_jump_phys(gobj),
        addr::JUMP_COLL => g.dk_heavy_jump_coll(gobj),
        addr::WAIT1_ANIM => g.dk_heavy_wait1_anim(gobj),
        addr::WAIT1_IASA => g.dk_heavy_wait1_iasa(gobj),
        addr::WAIT1_PHYS => g.dk_heavy_wait1_phys(gobj),
        addr::WAIT1_COLL => g.dk_heavy_wait1_coll(gobj),
        addr::LANDING_ANIM => g.dk_heavy_landing_anim(gobj),
        addr::LANDING_PHYS => g.dk_heavy_landing_phys(gobj),
        addr::LANDING_COLL => g.dk_heavy_landing_coll(gobj),
        addr::WAIT2_ANIM => g.dk_heavy_wait2_anim(gobj),
        addr::WAIT2_PHYS => g.dk_heavy_wait2_phys(gobj),
        addr::WAIT2_COLL => g.dk_heavy_wait2_coll(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// One of the heavy states, `step` past the first (`x2CC`'s `motion_state`).
    fn dk_heavy_state(&self, gobj: GObjId, step: i32) -> i32 {
        let a = self
            .game
            .fighter(gobj)
            .x2CC
            .expect("a heavy carrier without cargo attributes");
        let b = &self.game.fighter_attrs[a.0];
        i32::from_be_bytes(b[0..4].try_into().unwrap()).wrapping_add(step)
    }

    /// Back to the heavy wait when the stick lets go, `ftDk_MS_341_800DF938`.
    #[doc(alias = "ftDk_MS_341_800DF938")]
    pub fn dk_heavy_wait_check(&mut self, gobj: GObjId) -> bool {
        if self.stick_not_forward(gobj) {
            self.enter_dk_heavy_wait(gobj);
            return true;
        }
        false
    }

    /// Carrying the item, waiting, `ftDk_MS_341_800DF980`: landed first if in the air.
    #[doc(alias = "ftDk_MS_341_800DF980")]
    pub fn enter_dk_heavy_wait(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.land(gobj);
        }
        let msid = self.dk_heavy_state(gobj, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftDk_HeavyWait0_IASA`: a throw, a jump, a turn or a walk.
    #[doc(alias = "ftDk_HeavyWait0_IASA")]
    pub fn dk_heavy_wait0_iasa(&mut self, gobj: GObjId) {
        let _ = self.heavy_throw_check(gobj)
            || self.dk_heavy_jump_check(gobj)
            || self.dk_heavy_turn_check(gobj)
            || self.dk_heavy_walk_check(gobj);
    }

    /// `ftDk_HeavyWait0_Phys`: friction.
    #[doc(alias = "ftDk_HeavyWait0_Phys")]
    pub fn dk_heavy_wait0_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDk_HeavyWait0_Coll`: off a ledge, the heavy fall, the item kept.
    #[doc(alias = "ftDk_HeavyWait0_Coll")]
    pub fn dk_heavy_wait0_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.enter_dk_heavy_fall(gobj));
    }

    /// A heavy walk when the stick leans forward, `ftDk_MS_341_800DFA98`.
    #[doc(alias = "ftDk_MS_341_800DFA98")]
    pub fn dk_heavy_walk_check(&mut self, gobj: GObjId) -> bool {
        if self.walk_input(gobj) {
            self.enter_dk_heavy_walk(gobj, 0.0);
            return true;
        }
        false
    }

    /// A heavy walk at the speed the ground speed calls for, `ftDk_MS_341_800DFAE4`.
    #[doc(alias = "ftDk_MS_341_800DFAE4")]
    pub fn enter_dk_heavy_walk(&mut self, gobj: GObjId, anim_start: f32) {
        let f = self.game.fighter_id(gobj);
        let msid = self.dk_heavy_state(gobj, WALK);
        let at = |g: &Self, base: usize| [0, 1, 2].map(|i| g.cargo_attr_f32(f, base + 4 * i));
        let (frames, rates) = (at(self, WALK_FRAMES), at(self, WALK_RATES));
        self.enter_walk_common(gobj, msid, 0, anim_start, frames, rates, 1.0);
    }

    /// `ftDk_HeavyWalk_Anim`: the walk's animation speed follows the ground speed.
    #[doc(alias = "ftDk_HeavyWalk_Anim")]
    pub fn dk_heavy_walk_anim(&mut self, gobj: GObjId) {
        self.co_walk_anim(gobj);
    }

    /// `ftDk_HeavyWalk_IASA`: a throw, a jump, the wait when the stick lets go, else the walk's
    /// speed switch.
    #[doc(alias = "ftDk_HeavyWalk_IASA")]
    pub fn dk_heavy_walk_iasa(&mut self, gobj: GObjId) {
        if self.heavy_throw_check(gobj)
            || self.dk_heavy_jump_check(gobj)
            || self.dk_heavy_wait_check(gobj)
        {
            return;
        }
        self.walk_speed_switch(gobj, |g, gobj, frame| g.enter_dk_heavy_walk(gobj, frame));
    }

    /// `ftDk_HeavyWalk_Phys`: the walk's acceleration.
    #[doc(alias = "ftDk_HeavyWalk_Phys")]
    pub fn dk_heavy_walk_phys(&mut self, gobj: GObjId) {
        self.co_walk_phys(gobj);
    }

    /// `ftDk_HeavyWalk_Coll`: the wait's.
    #[doc(alias = "ftDk_HeavyWalk_Coll")]
    pub fn dk_heavy_walk_coll(&mut self, gobj: GObjId) {
        self.dk_heavy_wait0_coll(gobj);
    }

    /// A heavy turn when the stick points back, `ftDk_MS_345_800E0134`.
    #[doc(alias = "ftDk_MS_345_800E0134")]
    pub fn dk_heavy_turn_check(&mut self, gobj: GObjId) -> bool {
        if self.turn_input(gobj) {
            self.enter_dk_heavy_turn(gobj);
            return true;
        }
        false
    }

    /// The heavy turn over `cargo_hold.x20_TURN_SPEED` frames, `ftDk_MS_345_800E0134`'s
    /// `ifSomething`.
    #[doc(alias = "ifSomething")]
    pub fn enter_dk_heavy_turn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let msid = self.dk_heavy_state(gobj, TURN);
        let frames = self.cargo_attr_f32(f, TURN_SPEED);
        self.enter_turn(gobj, msid, 0, 0.0, frames, 0.0);
    }

    /// `ftDk_HeavyTurn_Anim`: the turn, then the wait when it ends.
    #[doc(alias = "ftDk_HeavyTurn_Anim")]
    pub fn dk_heavy_turn_anim(&mut self, gobj: GObjId) {
        self.turn_anim_inner(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_dk_heavy_wait(gobj);
        }
    }

    /// `ftDk_HeavyTurn_IASA`: once turned, a throw or a jump.
    #[doc(alias = "ftDk_HeavyTurn_IASA")]
    pub fn dk_heavy_turn_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(TURN_HAS_TURNED) != 0 && !self.heavy_throw_check(gobj) {
            self.dk_heavy_jump_check(gobj);
        }
    }

    /// `ftDk_HeavyTurn_Phys`: friction.
    #[doc(alias = "ftDk_HeavyTurn_Phys")]
    pub fn dk_heavy_turn_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDk_HeavyTurn_Coll`: the wait's.
    #[doc(alias = "ftDk_HeavyTurn_Coll")]
    pub fn dk_heavy_turn_coll(&mut self, gobj: GObjId) {
        self.dk_heavy_wait0_coll(gobj);
    }

    /// The heavy fall, `ftDk_MS_345_800E0294`, keeping a fast fall, its animation stopped;
    /// airborne if it starts on the ground.
    #[doc(alias = "ftDk_MS_345_800E0294")]
    pub fn enter_dk_heavy_fall(&mut self, gobj: GObjId) {
        let msid = self.dk_heavy_state(gobj, FALL);
        self.change_motion_state(gobj, msid, MF_KEEP_FAST_FALL, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
        if !self.is_airborne(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
    }

    /// `ftDk_HeavyFall_IASA`: a throw.
    #[doc(alias = "ftDk_HeavyFall_IASA")]
    pub fn dk_heavy_fall_iasa(&mut self, gobj: GObjId) {
        self.heavy_throw_check(gobj);
    }

    /// `ftDk_HeavyFall_Phys`: the air's physics.
    #[doc(alias = "ftDk_HeavyFall_Phys")]
    pub fn dk_heavy_fall_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftDk_HeavyFall_Coll`: landing, the heavy landing.
    #[doc(alias = "ftDk_HeavyFall_Coll")]
    pub fn dk_heavy_fall_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_dk_heavy_landing(gobj));
    }

    /// The heavy knee bend on a jump's input, `ftDk_MS_347_800E0378`.
    #[doc(alias = "ftDk_MS_347_800E0378")]
    pub fn dk_heavy_jump_check(&mut self, gobj: GObjId) -> bool {
        let input = self.jump_input(gobj);
        if input != 0 {
            self.enter_dk_heavy_knee_bend(gobj, input);
            return true;
        }
        false
    }

    /// The heavy jump, `ftDk_MS_347_800E03C0`: airborne, its animation stopped, the jump's
    /// takeoff.
    #[doc(alias = "ftDk_MS_347_800E03C0")]
    pub fn enter_dk_heavy_jump(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = self.dk_heavy_state(gobj, JUMP);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
        self.jump_take_off(gobj, true, 1.0);
    }

    /// `ftDk_HeavyJump_IASA`: a throw.
    #[doc(alias = "ftDk_HeavyJump_IASA")]
    pub fn dk_heavy_jump_iasa(&mut self, gobj: GObjId) {
        self.heavy_throw_check(gobj);
    }

    /// `ftDk_HeavyJump_Phys`: the jump's physics (`ftCo_Jump_Phys_Inner`).
    #[doc(alias = "ftDk_HeavyJump_Phys")]
    pub fn dk_heavy_jump_phys(&mut self, gobj: GObjId) {
        self.co_jump_phys(gobj);
    }

    /// `ftDk_HeavyJump_Coll`: the fall's.
    #[doc(alias = "ftDk_HeavyJump_Coll")]
    pub fn dk_heavy_jump_coll(&mut self, gobj: GObjId) {
        self.dk_heavy_fall_coll(gobj);
    }

    /// The heavy knee bend, `ftDk_MS_348_800E04A4`: `input` the jump's input, held for
    /// `cargo_hold.x24_JUMP_STARTUP_LAG` frames with the animation stopped.
    #[doc(alias = "ftDk_MS_348_800E04A4")]
    pub fn enter_dk_heavy_knee_bend(&mut self, gobj: GObjId, input: i32) {
        let f = self.game.fighter_id(gobj);
        let lag = self.cargo_attr_f32(f, JUMP_STARTUP_LAG);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(KNEE_BEND_INPUT, input);
        fp.set_mv_i32(KNEE_BEND_X0, 0);
        fp.set_mv_f32(KNEE_BEND_LAG, reg(lag));
        let msid = self.dk_heavy_state(gobj, KNEE_BEND);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_anim_rate(gobj, 0.0);
    }

    /// `ftDk_HeavyWait1_Anim`: the jump once the knee bend's lag runs out.
    #[doc(alias = "ftDk_HeavyWait1_Anim")]
    pub fn dk_heavy_wait1_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_f32(KNEE_BEND_LAG) <= 0.0 {
            self.enter_dk_heavy_jump(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let lag = reg(fp.mv_f32(KNEE_BEND_LAG));
        fp.set_mv_f32(KNEE_BEND_LAG, fp::fsubs(lag, 1.0));
    }

    /// `ftDk_HeavyWait1_IASA`: a throw, else a short hop's input.
    #[doc(alias = "ftDk_HeavyWait1_IASA")]
    pub fn dk_heavy_wait1_iasa(&mut self, gobj: GObjId) {
        if !self.heavy_throw_check(gobj) {
            self.knee_bend_short_hop_check(gobj);
        }
    }

    /// `ftDk_HeavyWait1_Phys`: friction.
    #[doc(alias = "ftDk_HeavyWait1_Phys")]
    pub fn dk_heavy_wait1_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDk_HeavyWait1_Coll`: the wait's.
    #[doc(alias = "ftDk_HeavyWait1_Coll")]
    pub fn dk_heavy_wait1_coll(&mut self, gobj: GObjId) {
        self.dk_heavy_wait0_coll(gobj);
    }

    /// The heavy landing, `ftDk_MS_346_800E05E4`: `cargo_hold.x28_LANDING_LAG` frames with its
    /// animation stopped.
    #[doc(alias = "ftDk_MS_346_800E05E4")]
    pub fn enter_dk_heavy_landing(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.cargo_attr_f32(f, LANDING_LAG);
        self.game.fighters[f.0].set_mv_f32(LANDING_LAG_LEFT, reg(lag));
        let msid = self.dk_heavy_state(gobj, LANDING);
        self.enter_landing(gobj, msid, 1, 0, 0.0, 1.0);
        self.set_anim_rate(gobj, 0.0);
    }

    /// `ftDk_HeavyLanding_Anim`: the wait once the lag runs out.
    #[doc(alias = "ftDk_HeavyLanding_Anim")]
    pub fn dk_heavy_landing_anim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_f32(LANDING_LAG_LEFT) <= 0.0 {
            self.enter_dk_heavy_wait(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let lag = reg(fp.mv_f32(LANDING_LAG_LEFT));
        fp.set_mv_f32(LANDING_LAG_LEFT, fp::fsubs(lag, 1.0));
    }

    /// `ftDk_HeavyLanding_Phys`: friction.
    #[doc(alias = "ftDk_HeavyLanding_Phys")]
    pub fn dk_heavy_landing_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDk_HeavyLanding_Coll`: the wait's.
    #[doc(alias = "ftDk_HeavyLanding_Coll")]
    pub fn dk_heavy_landing_coll(&mut self, gobj: GObjId) {
        self.dk_heavy_wait0_coll(gobj);
    }

    /// A hit while carrying the item, `ftDk_MS_349_800E06D8`: a light one (`checkSomething`)
    /// leaves him hurt with it in the hurt wait (`doSomething`); a harder one makes him drop
    /// it and react as anyone does.
    #[doc(alias = "ftDk_MS_349_800E06D8")]
    pub fn dk_heavy_hit(&mut self, gobj: GObjId) {
        if self.dk_heavy_hit_keeps(gobj) {
            self.dk_heavy_hit_keep(gobj);
        } else {
            let item = self.game.fighter(gobj).item_gobj.expect("carrying nothing");
            release::drop(self, item, Vec3::new(0.0, 0.0, 0.0), 1.0);
            self.damage_reaction(gobj, 0.0);
        }
    }

    /// Whether a hit leaves him carrying, `checkSomething`: too light to react to, or
    /// knockback below the third level (`ftCo_8008D8E8` of `ftCo_ScaleBy154`).
    #[doc(alias = "checkSomething")]
    pub fn dk_heavy_hit_keeps(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.hit_too_light(f) {
            return true;
        }
        let c = self.common();
        let scaled = fp::fmuls(
            reg(self.game.fighters[f.0].dmg.kb_applied),
            reg(c.f32(0x154)),
        );
        // `ftCo_8008D8E8` < 3: below `ftCommonData` x160 (and so below x158 and x15C).
        let level = if scaled < reg(c.f32(0x158)) {
            0
        } else if scaled < reg(c.f32(0x15C)) {
            1
        } else if scaled < reg(c.f32(0x160)) {
            2
        } else {
            3
        };
        level < 3
    }

    /// Hurt carrying the item, `doSomething`: hitstun in the hurt wait.
    #[doc(alias = "doSomething")]
    pub fn dk_heavy_hit_keep(&mut self, gobj: GObjId) {
        let msid = self.dk_heavy_state(gobj, WAIT2);
        self.enter_damage(gobj, msid, 0.0);
    }

    /// `ftDk_HeavyWait2_Anim`: the hurt wait's hitstun, then the heavy fall or wait.
    #[doc(alias = "ftDk_HeavyWait2_Anim")]
    pub fn dk_heavy_wait2_anim(&mut self, gobj: GObjId) {
        self.damage_tick(gobj);
        if !self.game.fighter(gobj).x221C_b6 {
            if self.is_airborne(gobj) {
                self.enter_dk_heavy_fall(gobj);
            } else {
                self.enter_dk_heavy_wait(gobj);
            }
        }
    }

    /// `ftDk_HeavyWait2_Phys`: the damage state's physics.
    #[doc(alias = "ftDk_HeavyWait2_Phys")]
    pub fn dk_heavy_wait2_phys(&mut self, gobj: GObjId) {
        self.co_damage_phys(gobj);
    }

    /// `ftDk_HeavyWait2_Coll`: off the ground, airborne; in the air, landing.
    #[doc(alias = "ftDk_HeavyWait2_Coll")]
    pub fn dk_heavy_wait2_coll(&mut self, gobj: GObjId) {
        if !self.is_airborne(gobj) {
            if !self.ground_coll_off_ledges(gobj) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne(f);
            }
        } else if self.air_coll_land(gobj) {
            self.land(gobj);
        }
    }
}
