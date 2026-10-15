//! The scene loop (`melee/gm/gmscene.c`): each engine tick takes a controller poll from the
//! queue, runs the scene's frame function and the button maps when the game clock comes due,
//! sets the pause mask, and runs every GObj process.
//!
//! After each batch of ticks the original renders a frame: the display pass
//! (`HSD_GObj_80390FC0`, which changes game state as it draws; [`crate::game::render`]) and the
//! video copy. melee-sim runs the display pass there; the audio mix is left out (the audio
//! driver's frames run on their own timeline, `crate::audio::clock`). The original also takes
//! as many ticks per frame as polls queued; melee-sim takes one poll per tick ([`step`]).

use super::super::state::{Game, Running};
use crate::input::controller::{Flush, PadData, Requests};

/// The pause mask's halves (`gm_803DA8C8`), by `unk_34`: the processes the game clock and the
/// 60 Hz clock each gate.
const CLOCK_MASKS: [u64; 2] = [u64::MAX, u64::MAX];

/// Process links each debug pause bit stops (`gm_803DA888`).
const PAUSE_LINKS: [u64; 8] = [0, 0x82FFFA, 0, 0x8EFFFA, 0x800FFA, 0x808FFA, 0x800FFA, 0];

/// What a tick hands back besides the state.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Tick {
    /// What the pad library asked of the hardware (resets, rumble motors).
    pub pads: Requests,
    /// Whether the scene asked to end (`gm_801A4B60`, `gm_801A4B74`) or the console's reset
    /// button was pressed.
    pub ended: bool,
}

impl Game {
    /// Whether a debug pause bit is set, `gm_GetDbPauseFlag`.
    #[doc(alias = "gm_GetDbPauseFlag")]
    pub fn db_pause_flag(&self, bit: u32) -> bool {
        u64::from(self.scene.unk_10.x0) & (1u64 << bit) != 0
    }

    /// The debug pause bits, `gm_801A4624`.
    #[doc(alias = "gm_801A4624")]
    pub fn db_pause_flags(&self) -> u8 {
        self.scene.unk_10.x0
    }

    #[doc(alias = "gm_SetDbPauseFlag")]
    pub fn set_db_pause_flag(&mut self, bit: u32) {
        self.scene.unk_10.x0 = (u64::from(self.scene.unk_10.x0) | 1u64 << bit) as u8;
    }

    #[doc(alias = "gm_ClearDbPauseFlag")]
    pub fn clear_db_pause_flag(&mut self, bit: u32) {
        self.scene.unk_10.x0 = (u64::from(self.scene.unk_10.x0) & !(1u64 << bit)) as u8;
    }

    /// Whether a debug frame step is pending, `gm_801A46B8`.
    #[doc(alias = "gm_801A46B8")]
    pub fn db_frame_step(&self, bit: u32) -> bool {
        u64::from(self.scene.unk_10.x2) & (1u64 << bit) != 0
    }

    /// The process links the debug pause bits stop, `gm_801A48A4`.
    #[doc(alias = "gm_801A48A4")]
    pub fn pause_links(bits: u8) -> u64 {
        let mut bits = bits;
        let mut out = 0;
        for m in PAUSE_LINKS {
            if bits & 1 != 0 {
                out |= m;
            }
            bits >>= 1;
        }
        out
    }

    /// `gm_SetDbPauseInputHandlers`.
    #[doc(alias = "gm_SetDbPauseInputHandlers")]
    pub fn set_db_pause_handlers(&mut self, pause: u32, step: u32) {
        let h = &mut self.scene.unk_10.db_input;
        h.check_pause = super::super::mirror::code(pause);
        h.check_framestep = super::super::mirror::code(step);
    }

    /// The debug pause buttons every scene starts with, `gm_801A4B1C`.
    #[doc(alias = "gm_801A4B1C")]
    pub fn default_db_pause_handlers(&mut self) {
        self.set_db_pause_handlers(0x801A_46F4, 0x801A_47E4);
    }

    #[doc(alias = "gm_SetPreGObjProcCallback")]
    pub fn set_pre_gobj_proc(&mut self, f: u32) {
        self.scene.unk_10.pre_gobj_proc = super::super::mirror::code(f);
    }

    /// Which half of the pause mask the clocks gate, `gm_801A4B50`.
    #[doc(alias = "gm_801A4B50")]
    pub fn set_clock_mask(&mut self, which: i32) {
        self.scene.unk_10.unk_34 = which;
    }

    /// Ends the scene after this frame, `gm_801A4B60`.
    #[doc(alias = "gm_801A4B60")]
    pub fn end_scene(&mut self) {
        self.scene.unk_C = 1;
    }

    /// Ends the scene at once, without rendering this frame, `gm_801A4B74`.
    #[doc(alias = "gm_801A4B74")]
    pub fn end_scene_now(&mut self) {
        self.scene.unk_C = 2;
    }

    /// Engine ticks since the scene started, `gm_801A4BA8`.
    #[doc(alias = "gm_801A4BA8")]
    pub fn scene_ticks(&self) -> u32 {
        self.scene.unk_0
    }

    /// Ticks the game clock was due since the scene started, `gm_801A4BB8`.
    #[doc(alias = "gm_801A4BB8")]
    pub fn scene_game_ticks(&self) -> u32 {
        self.scene.unk_8
    }

    /// Clears the debug pause state, `gm_801677C0`.
    #[doc(alias = "gm_801677C0")]
    pub fn clear_db_pause(&mut self) {
        let s = &mut self.scene.unk_10;
        s.x0 = 0;
        s.x1 = 0;
        s.x2 = 0;
        s.x3 = 0;
        s.unk_20 = 0;
        s.unk_28 = 0;
    }

    /// The scene loop's setup before its first tick, `gm_801A4D34`'s prologue: the debug
    /// pause state and counters cleared and all but the newest queued poll dropped.
    pub fn start_scene_loop(&mut self) {
        self.clear_db_pause();
        self.scene.unk_0 = 0;
        self.scene.unk_4 = 0;
        self.scene.unk_8 = 0;
        self.scene.unk_C = 0;
        self.pads.flush_queue(Flush::Leave1);
    }
}

/// One engine tick of the scene loop, the body of `gm_801A4D34`'s inner loop, taking the
/// oldest queued poll. Returns whether the scene asked to end. Not yet checked against
/// ssbm-rs as a whole: a tick runs every process, the fighters' among them (its parts are:
/// the clocks, the button maps, the frame function and the pause mask's halves).
///
/// The audio frames that fall before the tick on ssbm-rs's timeline run first, and its
/// perf timer's read at the end lets time pass (`crate::audio::clock`).
pub fn tick(r: &mut Running) -> bool {
    r.game.audio.tick_start(r.data);
    crate::game::gr::stadium::take_load_end(r.game);
    queue_alarm_polls(r, 1);
    begin_tick(r);
    let ended = finish_tick(r, Resume::ButtonMaps);
    r.game.audio.tick_end(r.data);
    crate::game::gr::stadium::take_load_end(r.game);
    queue_alarm_polls(r, 0);
    ended
}

/// The pad alarm's polls on the timeline that the pad library hasn't queued yet
/// (`HSD_PadRenewRawStatus` from the alarm): the alarm runs a little faster than video
/// fields, so now and then a round finds two queued and ticks twice. `taking` counts the
/// tick about to read its poll. Only replay runs keep a timeline, and their controllers are
/// at rest.
fn queue_alarm_polls(r: &mut Running, taking: u32) {
    let Some(c) = r.game.audio.clock.as_ref() else {
        return;
    };
    let want = c.pads + taking;
    while u32::from(r.game.pads.lib.qcount) < want {
        r.game.pads.renew_raw(
            r.data,
            [crate::input::pad::PadStatus::default(); 4],
            false,
            false,
        );
    }
}

/// The tick's first part: the oldest poll into the master statuses and, when the game
/// clock comes due, the game and copy statuses.
pub fn begin_tick(r: &mut Running) {
    let g = &mut *r.game;
    g.pads.renew_master();
    g.assert_retail();
    let run = g.db_frame_step(0) || !g.db_pause_flag(0);
    g.scene.unk_10.unk_38_0 = run;
    if run {
        g.pacing.advance();
        if g.pacing.due(0) {
            g.pads.renew_game();
            g.pads.renew_copy();
        }
    }
}

/// Where [`finish_tick`] takes a tick up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Resume {
    /// Before the button maps (`gm_EvaluateAllControllerInputs`).
    ButtonMaps,
    /// Before the scene's frame function.
    FrameFunction,
}

/// The rest of a tick after [`begin_tick`], from `from`: the button maps and the scene's
/// frame function when the game clock comes due, the pause mask, the audio library's tick
/// (`lbAudioAx_80027DF8`), the hook before the processes, every process, and the tick
/// counts. Returns whether the scene asked to end.
pub fn finish_tick(r: &mut Running, from: Resume) -> bool {
    let g = &mut *r.game;
    if g.scene.unk_10.unk_38_0 {
        if from == Resume::ButtonMaps && g.pacing.due(0) {
            g.evaluate_controller_inputs();
        }
        if g.pacing.due(0)
            && let Some(f) = g.gm.on_frame
        {
            super::super::code::run_event(r, f.0);
        }
    }
    let g = &mut *r.game;
    let s = &mut g.scene.unk_10;
    if s.x0 != s.x1 || s.x2 != s.x3 {
        s.unk_20 = if s.unk_38_0 {
            Game::pause_links(s.x0)
        } else {
            u64::MAX
        };
        s.x1 = s.x0;
        s.x3 = s.x2;
        s.x2 = 0;
    }
    s.unk_28 = s.unk_20;
    let mask = CLOCK_MASKS[s.unk_34 as usize];
    if !g.pacing.due(0) {
        s.unk_28 |= mask;
    }
    if !g.pacing.due(1) {
        s.unk_28 |= !mask;
    }
    r.lb_audio_tick();
    if let Some(f) = r.game.scene.unk_10.pre_gobj_proc {
        super::super::code::run_event(r, f.0);
    }
    let g = &mut *r.game;
    // The scheduler reads the mask through `HSD_GObjLibInitData.unk_2`.
    g.world.skip_links = g.scene.unk_10.unk_28;
    crate::hsd::gobj::run_procs(r);
    let g = &mut *r.game;
    let sc = &mut g.scene;
    if sc.unk_0 != u32::MAX - 1 {
        sc.unk_0 = sc.unk_0.wrapping_add(1);
    }
    if sc.unk_10.unk_38_0 && g.pacing.due(0) && sc.unk_8 != u32::MAX - 1 {
        sc.unk_8 = sc.unk_8.wrapping_add(1);
    }
    sc.unk_C != 0
}

/// Runs a frame of the scene loop with one controller poll: the pad library queues the poll
/// (`HSD_PadRenewRawStatus`, which the original's polling alarm runs), the clocks take up a
/// new game speed (`lb_80019894`), one engine tick runs, and unless the scene ended at once
/// the display pass runs on the screen and the frame counts as rendered.
pub fn step(r: &mut Running, poll: PadData, reset_pressed: bool) -> Tick {
    let (t, render) = step_tick(r, poll, reset_pressed);
    if render {
        render_screen(r);
    }
    t
}

/// [`step`] up to the frame's display pass, where ssbm-rs's state traces sample the
/// original: whether the pass is due ([`render_screen`]).
pub fn step_tick(r: &mut Running, poll: PadData, reset_pressed: bool) -> (Tick, bool) {
    // The loop waits for a poll only when none is queued (`lb_80019894`): the scene's first
    // round runs on the one its setup left.
    let pads = if r.game.pads.lib.qcount == 0 {
        r.game.pads.renew_raw(r.data, poll, false, reset_pressed)
    } else {
        Requests::default()
    };
    r.game.pacing.update();
    if r.game.pads.reset_switch() {
        return (Tick { pads, ended: true }, false);
    }
    let ended = tick(r);
    // A round ticks once per poll it found queued (on the timeline, which counts the
    // round's ticks), or until the scene asks to end, then renders once.
    let round_over = r.game.scene.unk_C != 0
        || r.game
            .audio
            .clock
            .as_ref()
            .is_none_or(|c| c.ticks_left == 0);
    (Tick { pads, ended }, r.game.scene.unk_C != 2 && round_over)
}

/// The frame's display pass on the screen (`HSD_StartRender(HSD_RP_SCREEN)`), which counts
/// the frame as rendered.
pub fn render_screen(r: &mut Running) {
    crate::hsd::display::start_frame(&mut r.game.objs);
    crate::hsd::display::set_offscreen(&mut r.game.objs, false);
    r.render_frame();
    let sc = &mut r.game.scene;
    if sc.unk_4 != u32::MAX - 1 {
        sc.unk_4 = sc.unk_4.wrapping_add(1);
    }
}
