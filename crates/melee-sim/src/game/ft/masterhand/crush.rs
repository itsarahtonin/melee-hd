//! The hands' attacks that follow the nearest opponent across the stage: a hold that follows
//! for a count (Master Hand's punch, `ftmasterhandrockcrush.c`; Crazy Hand's walk,
//! `ftcrazyhandwalk.c`), a window of frames that follows (Master Hand's flat slap,
//! `ftmasterhandpapercrush.c`; Crazy Hand's drill, `ftcrazyhanddrill.c`), a countdown that
//! follows until late (Master Hand's drill, `ftmasterhanddrill.c`; Crazy Hand's falling crush,
//! `ftcrazyhandrockcrush1.c`), and repeated chases at a random count (Master Hand's poke,
//! `ftmasterhandpoke.c`; Crazy Hand's rising crush, `ftcrazyhandrockcrush0.c`). Each hand runs
//! them in his own states, over his own attributes.

use gekko_fp as fp;

use super::super::super::state::Running;
use super::{HAND, Hand, addr, ms, mv};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// The repeated chases' end's thud (`ft_PlaySFX`), the same for both.
const SFX_CHASES_END: i32 = 320007;

/// A hold that follows the nearest opponent for a count: its state and its end's, the count's
/// attribute and the most it follows a frame.
pub struct Hold {
    pub states: [i32; 2],
    pub frames: usize,
    pub follow: usize,
}

/// A window of frames in which the attack follows the nearest opponent: its state, the
/// window's first and last frames' attributes and the most it follows a frame.
pub struct Window {
    pub state: i32,
    pub frames: [usize; 2],
    pub follow: usize,
}

/// A countdown from 107 that follows the nearest opponent from the frame in its attribute
/// `late` on: its state and the most it follows a frame.
pub struct Countdown {
    pub state: i32,
    pub late: usize,
    pub follow: usize,
}

/// Repeated chases toward a point off the nearest opponent: the state and its end's, the
/// count's attributes (the high word, then the low), and the point.
pub struct Chases {
    pub states: [i32; 2],
    pub count: usize,
    pub point: usize,
}

impl Running<'_> {
    /// `ftMh_RockCrushUp_Anim` and `ftCh_WalkLoop_Anim`: the hold for its count once the
    /// start ends.
    #[doc(alias = "ftMh_RockCrushUp_Anim")]
    #[doc(alias = "ftCh_WalkLoop_Anim")]
    pub fn hand_hold_start_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.hand_enter(gobj, h.hold.states[0]);
            let n = h.i32(self, f, h.hold.frames);
            self.game.fighters[f.0].set_mv_i32(mv::X8, n);
        }
    }

    /// `ftMh_RockCrushWait_Anim` and `ftCh_WalkWait_Anim`: the end once the count runs out;
    /// the hold's animation again.
    #[doc(alias = "ftMh_RockCrushWait_Anim")]
    #[doc(alias = "ftCh_WalkWait_Anim")]
    pub fn hand_hold_anim(&mut self, gobj: GObjId, h: &Hand) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.hand_hold_end_enter(gobj, h);
        }
        if !self.anim_frames_remaining(gobj) {
            self.hand_enter(gobj, h.hold.states[0]);
        }
    }

    /// `ftMh_RockCrushWait_Phys` and `ftCh_WalkWait_Phys`: the air motion, following the
    /// nearest opponent at most the hold's most a frame, over the stage.
    #[doc(alias = "ftMh_RockCrushWait_Phys")]
    #[doc(alias = "ftCh_WalkWait_Phys")]
    pub fn hand_hold_phys(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let max = h.f32(self, f, h.hold.follow);
        self.boss_follow_x(gobj, max);
        self.boss_clamp_to_floor(gobj);
    }

    /// The hold's end, still, `ftMh_MS_356_801525E0` (the fist down) and `ftCh_Init_80157B58`.
    #[doc(alias = "ftMh_MS_356_801525E0")]
    #[doc(alias = "ftCh_Init_80157B58")]
    pub fn hand_hold_end_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.game.fighter_mut(gobj).self_vel.x = 0.0;
        self.hand_enter(gobj, h.hold.states[1]);
    }

    /// The window's attack, its frame count from 0 (`mv.x0`), `ftMh_MS_357_801526D8` (the flat
    /// slap) and `fn_80157C50` (the drill).
    #[doc(alias = "ftMh_MS_357_801526D8")]
    #[doc(alias = "fn_80157C50")]
    pub fn hand_window_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.window.state);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 0.0);
    }

    /// `ftMh_PaperCrush_Phys` and `ftCh_Drill_Phys`: the air motion; between the window's
    /// frames following the nearest opponent at most its most a frame, else still; over the
    /// stage.
    #[doc(alias = "ftMh_PaperCrush_Phys")]
    #[doc(alias = "ftCh_Drill_Phys")]
    pub fn hand_window_phys(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let [lo, hi] = h.window.frames.map(|off| h.i32(self, f, off));
        let max = h.f32(self, f, h.window.follow);
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fadds(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(lo)) && n < fp::frsp(f64::from(hi)) {
            self.boss_follow_x(gobj, max);
        } else {
            self.game.fighters[f.0].self_vel.x = 0.0;
        }
        self.boss_clamp_to_floor(gobj);
    }

    /// The countdown's attack, 107 frames (`mv.x0`), `ftMh_MS_352_801521DC` (the drill) and
    /// `fn_80158144` (the falling crush).
    #[doc(alias = "ftMh_MS_352_801521DC")]
    #[doc(alias = "fn_80158144")]
    pub fn hand_countdown_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.countdown.state);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 107.0);
    }

    /// `ftMh_Drill_Phys` and `ftCh_RockCrushDown_Phys`: the air motion; from the late frame
    /// on until the count runs out following the nearest opponent at most its most a frame,
    /// else still; over the stage.
    #[doc(alias = "ftMh_Drill_Phys")]
    #[doc(alias = "ftCh_RockCrushDown_Phys")]
    pub fn hand_countdown_phys(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let (late, max) = (
            h.i32(self, f, h.countdown.late),
            h.f32(self, f, h.countdown.follow),
        );
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(late)) || n < 0.0 {
            fp.self_vel.x = 0.0;
        } else {
            self.boss_follow_x(gobj, max);
        }
        self.boss_clamp_to_floor(gobj);
    }

    /// The chases, `ftMh_MS_358_80152880` (the poke) and `ftCh_Init_80157DF8` (the rising
    /// crush): the low word and up to the high word more at random (`mv.x0`), not chasing yet
    /// (`mv.x4`).
    #[doc(alias = "ftMh_MS_358_80152880")]
    #[doc(alias = "ftCh_Init_80157DF8")]
    pub fn hand_chases_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.chases.states[0]);
        let (hi, lo) = (
            h.i32(self, f, h.chases.count),
            h.i32(self, f, h.chases.count + 4),
        );
        let r = self.game.objs.rng.below(hi.wrapping_sub(lo));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(lo.wrapping_add(r))));
        fp.set_mv_f32(mv::X4, 0.0);
    }

    /// `ftMh_Poke1_Anim` and `ftCh_RockCrushUp_Anim`: once a round ends he chases; past the
    /// last, still, the end and its thud.
    #[doc(alias = "ftMh_Poke1_Anim")]
    #[doc(alias = "ftCh_RockCrushUp_Anim")]
    pub fn hand_chases_anim(&mut self, gobj: GObjId, h: &Hand) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X4, 1.0);
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        if reg(fp.mv_f32(mv::X0)) < 0.0 {
            fp.set_mv_f32(mv::X4, 0.0);
            fp.self_vel.x = 0.0;
            fp.self_vel.y = 0.0;
            self.hand_enter(gobj, h.chases.states[1]);
            self.fighter_play_sfx(f, SFX_CHASES_END, 127, 64);
        }
    }

    /// `ftMh_Poke1_Phys` and `ftCh_RockCrushUp_Phys`: the air motion; while chasing, toward
    /// the chases' point off the nearest opponent.
    #[doc(alias = "ftMh_Poke1_Phys")]
    #[doc(alias = "ftCh_RockCrushUp_Phys")]
    pub fn hand_chases_phys(&mut self, gobj: GObjId, h: &Hand) {
        self.air_anim_motion(gobj);
        if reg(self.game.fighter(gobj).mv_f32(mv::X4)) != 0.0 {
            self.hand_chase(gobj, h, h.chases.point);
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_354_80152370 => g.hand_enter(gobj, ms::ROCK_CRUSH_UP),
        addr::ROCK_CRUSH_UP_ANIM => g.hand_hold_start_anim(gobj, &HAND),
        addr::MS_355_8015247C => g.hand_enter(gobj, ms::ROCK_CRUSH_WAIT),
        addr::ROCK_CRUSH_WAIT_ANIM => g.hand_hold_anim(gobj, &HAND),
        addr::ROCK_CRUSH_WAIT_PHYS => g.hand_hold_phys(gobj, &HAND),
        addr::MS_356_801525E0 => g.hand_hold_end_enter(gobj, &HAND),
        addr::MS_357_801526D8 => g.hand_window_enter(gobj, &HAND),
        addr::PAPER_CRUSH_PHYS => g.hand_window_phys(gobj, &HAND),
        addr::MS_352_801521DC => g.hand_countdown_enter(gobj, &HAND),
        addr::DRILL_PHYS => g.hand_countdown_phys(gobj, &HAND),
        addr::MS_358_80152880 => g.hand_chases_enter(gobj, &HAND),
        addr::POKE1_ANIM => g.hand_chases_anim(gobj, &HAND),
        addr::POKE2_ANIM => g.hand_anim_return(gobj, &HAND),
        addr::POKE1_IASA => g.hand_human_nop(gobj),
        addr::POKE1_PHYS => g.hand_chases_phys(gobj, &HAND),
        _ => return false,
    }
    true
}
