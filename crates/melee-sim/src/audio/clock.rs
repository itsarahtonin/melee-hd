//! When audio frames run relative to engine ticks, as ssbm-rs's timeline places them.
//!
//! The original runs the driver's frame in the audio interrupt, every 5 ms of audio. ssbm-rs
//! keeps virtual time: it moves only at wait points (the scene loop's waits, a thread sleep)
//! and by a few time base ticks at each time base or hardware register read, and interrupts
//! are taken there. In play, every audio frame is taken at a wait point between ticks (the
//! only reads inside a tick are the perf timer's and fighters' ARAM animation loads, too few
//! to reach the next frame), so which ticks a frame falls between follows from a handful of
//! clocks this module runs:
//!
//! - video fields every 675,675 time base ticks (60000/1001 Hz), whose retrace interrupt wakes
//!   the scene loop from its wait for a free frame buffer;
//! - the pad sampling alarm every 675,000 (1/60 s), which queues the controllers the next ticks
//!   read, one tick per sample;
//! - the audio DMA every 202,500 (5 ms), whose interrupt runs the AX frame (and the driver's),
//!   and the DSP stand-in's answer 20,250 later;
//! - the scene loop's waits, which jump to the next event or a quarter millisecond (10,125),
//!   whichever is sooner, and the handlers' and loop's register reads in between.
//!
//! The constants are ssbm-rs's (`ssbm_sdk`'s `GAME_WAIT_STEP`, `DSP_FRAME_TICKS`, read steps)
//! and the read counts of the code between waits, measured from its runs; they reproduce its
//! tick starts and audio frames exactly over whole matches.
//!
//! An event can be taken back before its time, as on the console: a cancelled or re-set alarm
//! leaves the OS's queue, and a stopped audio DMA raises no more interrupts. Each disc read
//! sets the disc driver's timeout alarm, which the read's interrupt cancels; ssbm-rs before
//! its `ab3cd01` left such events queued, where they cut a wait short (see
//! `docs/engine.md`). One thing this doesn't model: an audio frame due inside a tick would run
//! there.

/// Time base ticks per video field.
pub const FIELD: u64 = 675_675;
/// Time base ticks per audio frame (a 0x280-byte DMA block at 32 kHz).
pub const AUDIO_FRAME: u64 = 202_500;
/// The scene loop's longest wait (`GAME_WAIT_STEP`).
const WAIT_STEP: u64 = 10_125;
/// The DSP stand-in's time to mix a frame (`DSP_FRAME_TICKS`).
const DSP_FRAME: u64 = 20_250;
/// Time the retrace interrupt's handler takes.
const VI_HANDLER: u64 = 32;
/// Time the pad alarm's handler takes.
const ALARM_HANDLER: u64 = 24;
/// From the audio interrupt to its DSP mail, and to the end of its handler.
const AI_MAIL: u64 = 16;
const AI_HANDLER: u64 = 120;
/// From the render's wait to its sleep, and from the wake to the pad check.
const BEFORE_SLEEP: u64 = 32;
const AFTER_WAKE: u64 = 24;
/// From the render's wait to the pad check when a frame buffer is free.
const NO_SLEEP: u64 = 40;
/// The perf timer's time base read at a tick's start and end.
const TICK_READ: u64 = 16;
/// Time an ARAM load takes per 4 KB chunk (eight register reads).
pub const ARAM_CHUNK: u64 = 64;
/// A disc read's time from its command to its interrupt, ssbm-rs's `DVD_LATENCY` (Slippi's
/// fast disc speed; a drive's transfer time isn't modeled).
pub const DISC_LATENCY: u64 = 20_250;
/// The disc driver's timeout for a read (`DVDLowRead`'s alarm: 10 s of the time base).
pub const DISC_TIMEOUT: u64 = 10 * 40_500_000;
/// Where the audio interrupt's handler runs the user callback, where the disc interrupt's
/// handler runs the disc driver's callback, and the ARAM interrupt's handler its own.
const AI_CALLBACK: u64 = 64;
const DI_CALLBACK: u64 = 32;
const ARAM_CALLBACK: u64 = 8;

/// What happens at a time on the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// A video field starts (its retrace interrupt).
    Field,
    /// The pad sampling alarm fires.
    PadAlarm,
    /// An audio DMA block ends (the audio interrupt).
    AudioDma,
    /// The DSP finishes a frame.
    DspResume,
    /// A disc read finishes (the disc interface interrupt).
    DiscDone,
    /// The disc driver's timeout alarm, which a read's interrupt cancels long before.
    DiscTimeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub at: u64,
    /// The order events due at the same time run in: the order they were scheduled.
    pub seq: u64,
    pub kind: EventKind,
}

/// The timeline's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    /// The time base.
    pub now: u64,
    pub events: Vec<Event>,
    pub seq: u64,
    /// Raised interrupts not yet taken.
    pub vi_pending: bool,
    pub ai_pending: bool,
    pub di_pending: bool,
    pub aram_pending: u32,
    /// The disc driver's timeout alarm, while a read is in flight, by its event's `seq`.
    pub disc_timeout: Option<u64>,
    /// Pad samples queued and not yet ticked.
    pub pads: u32,
    /// The pad alarm's period.
    pub alarm_period: u64,
    /// Ticks left in the scene loop's current round (it ticks once per sample it found
    /// queued).
    pub ticks_left: u32,
    /// The scene loop sleeps until a retrace wakes it.
    pub woken: bool,
    /// Whether the DSP has answered the last frame's mail (`__AXOutDspReady` 1).
    pub dsp_ready: bool,
    /// Audio frames are placed ([`Clock::place_audio_frames`]) rather than scheduled; the last
    /// one's start.
    pub placed_frames: bool,
    pub last_frame: Option<u64>,
    /// Rounds of a scene's loop that start without waiting for a frame buffer: its first
    /// (no render before it) and its second (a buffer is still free).
    pub scene_start: u8,
}

/// What the timeline asks of its owner. Each handler starts its time log at `base`, the time
/// into the handler where it runs, and returns the log.
pub trait Frames {
    /// The audio interrupt: an AX frame.
    fn audio_frame(&mut self, base: u64) -> HwLog;
    /// The disc interrupt: the read in flight is done.
    fn disc_done(&mut self, base: u64) -> HwLog;
    /// The ARAM interrupt: the next ARAM transfer is done.
    fn aram_done(&mut self, base: u64) -> HwLog;
    /// What the game's code between handlers did to the hardware.
    fn take_log(&mut self) -> HwLog;
    /// Whether what the game waits for has happened.
    fn done(&mut self, w: Wait) -> bool;
}

/// What the game waits for in a loop with a wait point each spin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// Its file load's callback (`lbFile`'s `cancel`).
    FileLoaded,
    /// Room in the sound bank load queue (`HSD_SynthSFXLoad`'s spin).
    SfxQueueRoom,
    /// No sound bank load queued (`HSD_SynthSFXWaitForLoadCompletion`).
    SfxLoadsDone,
}

/// Time a stretch of the audio code took at the hardware, and what it started: the disc reads
/// (each by the time into the stretch its command went out) and ARAM transfers, which finish
/// at once and interrupt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HwLog {
    pub spent: u64,
    pub reads: Vec<u64>,
    pub aram: u32,
}

/// A disc read's command, and an ARAM transfer's request.
const DISC_ISSUE: u64 = 24;
const ARQ_POST: u64 = 56;

impl HwLog {
    pub fn at(base: u64) -> HwLog {
        HwLog {
            spent: base,
            ..HwLog::default()
        }
    }

    /// A disc read's command goes out.
    pub fn disc_read(&mut self) {
        self.spent += DISC_ISSUE;
        self.reads.push(self.spent);
    }

    /// An ARAM transfer is posted (and done).
    pub fn arq_post(&mut self) {
        self.spent += ARQ_POST;
        self.aram += 1;
    }
}

impl Clock {
    /// A clock at `now` with its events due at those times.
    pub fn new(now: u64, next_audio: u64, next_alarm: u64, alarm_period: u64) -> Clock {
        let mut c = Clock {
            now,
            alarm_period,
            dsp_ready: true,
            ..Clock::default()
        };
        c.schedule((now / FIELD + 1) * FIELD, EventKind::Field);
        c.schedule(next_alarm, EventKind::PadAlarm);
        c.schedule(next_audio, EventKind::AudioDma);
        c
    }

    /// Schedules an event, returning its `seq` to cancel it by.
    pub fn schedule(&mut self, at: u64, kind: EventKind) -> u64 {
        let seq = self.seq;
        self.events.push(Event { at, seq, kind });
        self.seq += 1;
        seq
    }

    /// Takes back an event that hasn't happened: nothing happens at its time, and no wait
    /// stops for it.
    pub fn cancel(&mut self, seq: u64) {
        self.events.retain(|e| e.seq != seq);
    }

    /// Sets the pad alarm to fire at `first` and every `period` after (`OSSetPeriodicAlarm`),
    /// taking back its old schedule.
    pub fn set_pad_alarm(&mut self, first: u64, period: u64) {
        self.cancel_pad_alarm();
        self.alarm_period = period;
        self.schedule(first, EventKind::PadAlarm);
    }

    /// Cancels the pad alarm (`OSCancelAlarm`).
    pub fn cancel_pad_alarm(&mut self) {
        self.events.retain(|e| e.kind != EventKind::PadAlarm);
    }

    /// Stops the audio DMA: the block playing raises no interrupt.
    pub fn stop_audio_dma(&mut self) {
        self.events.retain(|e| e.kind != EventKind::AudioDma);
    }

    /// The next event, by time then scheduling order.
    fn next_event(&self) -> Option<usize> {
        (0..self.events.len()).min_by_key(|&i| (self.events[i].at, self.events[i].seq))
    }

    fn next_at(&self) -> u64 {
        self.events[self.next_event().expect("the timeline runs dry")].at
    }

    /// Runs what is due now: events, then the interrupts they raised (the audio interrupt
    /// before the retrace), until nothing is.
    fn service(&mut self, f: &mut dyn Frames) {
        loop {
            if let Some(i) = self.next_event().filter(|&i| self.events[i].at <= self.now) {
                let e = self.events.swap_remove(i);
                match e.kind {
                    EventKind::Field => {
                        self.vi_pending = true;
                        self.schedule(e.at + FIELD, EventKind::Field);
                    }
                    EventKind::PadAlarm => {
                        self.now += ALARM_HANDLER;
                        self.pads += 1;
                        self.schedule(e.at + self.alarm_period, EventKind::PadAlarm);
                    }
                    EventKind::AudioDma => {
                        self.ai_pending = true;
                        if self.placed_frames {
                            self.last_frame = Some(self.now);
                        } else {
                            self.schedule(self.now + AUDIO_FRAME, EventKind::AudioDma);
                        }
                    }
                    EventKind::DspResume => self.dsp_ready = true,
                    EventKind::DiscDone => self.di_pending = true,
                    EventKind::DiscTimeout => panic!("a disc read timed out"),
                }
            } else if self.ai_pending {
                self.ai_pending = false;
                assert!(
                    self.dsp_ready,
                    "an audio frame before the DSP finished the last one"
                );
                let start = self.now;
                self.dsp_ready = false;
                self.schedule(start + AI_MAIL + DSP_FRAME, EventKind::DspResume);
                let log = f.audio_frame(AI_CALLBACK);
                let end = start + AI_HANDLER + (log.spent - AI_CALLBACK);
                self.apply(start, &log);
                self.now = end;
            } else if self.aram_pending != 0 {
                self.aram_pending -= 1;
                let start = self.now;
                let log = f.aram_done(ARAM_CALLBACK);
                self.apply(start, &log);
                self.now = start + log.spent;
            } else if self.di_pending {
                self.di_pending = false;
                // The interrupt's handler cancels the read's timeout before its callback.
                if let Some(seq) = self.disc_timeout.take() {
                    self.cancel(seq);
                }
                let start = self.now;
                let log = f.disc_done(DI_CALLBACK);
                self.apply(start, &log);
                self.now = start + log.spent;
            } else if self.vi_pending {
                self.vi_pending = false;
                self.now += VI_HANDLER;
                self.woken = true;
            } else {
                break;
            }
        }
    }

    /// Schedules a handler's disc reads and raises its ARAM transfers' interrupts.
    fn apply(&mut self, start: u64, log: &HwLog) {
        for &r in &log.reads {
            assert!(self.disc_timeout.is_none(), "two disc reads in flight");
            self.schedule(start + r + DISC_LATENCY, EventKind::DiscDone);
            self.disc_timeout =
                Some(self.schedule(start + r + DISC_TIMEOUT, EventKind::DiscTimeout));
        }
        self.aram_pending += log.aram;
    }

    /// The game's code since the last handler: its hardware time passes, with interrupts
    /// enabled.
    fn game_code(&mut self, f: &mut dyn Frames) {
        let log = f.take_log();
        if log.spent != 0 || !log.reads.is_empty() || log.aram != 0 {
            let start = self.now;
            self.apply(start, &log);
            self.now = start + log.spent;
            self.service(f);
        }
    }

    /// A wait point that lets at most `max` ticks pass (`Sdk::wait`).
    fn wait(&mut self, max: u64, f: &mut dyn Frames) {
        let next = self.next_at();
        self.now = self.now.max(next.min(self.now + max));
        self.service(f);
    }

    /// Time passing at register reads, with interrupts taken where they come due.
    fn read(&mut self, ticks: u64, f: &mut dyn Frames) {
        self.now += ticks;
        self.service(f);
    }

    /// Runs the timeline from the end of the last tick to the start of the next one's game
    /// code, running the audio frames that fall in between.
    pub fn tick_start(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        if self.ticks_left == 0 {
            let start = self.scene_start;
            self.scene_start = start.saturating_sub(1);
            if start < 2 {
                // The render's wait.
                self.wait(WAIT_STEP, f);
            }
            if start == 0 && !self.woken {
                // The sleep until a retrace frees a frame buffer: the one shown before the
                // last frame was copied, two buffers back.
                self.now += BEFORE_SLEEP;
                while !self.woken {
                    let next = self.next_at();
                    self.now = self.now.max(next);
                    self.service(f);
                }
                self.read(AFTER_WAKE, f);
            } else if start < 2 {
                // A buffer is free: a read, the frame's GX finish interrupt and a read before
                // the pad check (the sleep's wake covers the last two otherwise,
                // `AFTER_WAKE`).
                self.read(NO_SLEEP, f);
            }
            // The frame's copy: the next one needs a retrace after it.
            self.woken = false;
            // The scene loop's wait for a pad sample, and one more wait after it.
            while self.pads == 0 {
                self.wait(WAIT_STEP, f);
            }
            self.ticks_left = self.pads;
            self.wait(WAIT_STEP, f);
        }
        self.ticks_left -= 1;
        self.pads -= 1;
        self.read(TICK_READ, f);
    }

    /// A scene's loop ends after a tick that asked for the next scene (`gm_801A4D34`'s last
    /// round): the frame's wait point (`lb_800195D0`), its render and copy, and the wait for
    /// the copy to show (`HSD_VIWaitXFBFlush`, until the next retrace). The scene's exit and
    /// the next one's preparation (`gm_801A4014`'s `on_exit`s, `preloadState`) read no clock
    /// before the next scene's first audio call.
    pub fn end_scene_loop(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        self.wait(WAIT_STEP, f);
        self.woken = false;
        while !self.woken {
            let next = self.next_at();
            self.now = self.now.max(next);
            self.service(f);
        }
    }

    /// A scene's loop starts (`gm_801A4D34`'s prologue): the pad queue keeps only its newest
    /// sample, and the first round waits for a sample without rendering first.
    pub fn start_scene_loop(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        if std::mem::take(&mut self.placed_frames) {
            self.events.retain(|e| e.kind != EventKind::AudioDma);
            let start = self.last_frame.take().expect("a placed audio frame");
            self.schedule(start + AUDIO_FRAME, EventKind::AudioDma);
        }
        self.pads = self.pads.min(1);
        self.ticks_left = 0;
        self.scene_start = 2;
    }

    /// Time a tick's ARAM loads take (`ARAM_CHUNK` per 4 KB).
    pub fn aram_load(&mut self, len: u32, f: &mut dyn Frames) {
        self.game_code(f);
        self.read(ARAM_CHUNK * u64::from(len.div_ceil(0x1000)), f);
    }

    /// The game waiting inside a tick for its file load: `lbFile`'s loop, a wait point each
    /// spin (`lb_800195D0`), until the load's callback.
    pub fn wait_for_load(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        loop {
            self.wait(WAIT_STEP, f);
            if f.done(Wait::FileLoaded) {
                break;
            }
        }
    }

    /// The game waiting for `w` in a loop that checks before each wait point.
    pub fn wait_until(&mut self, f: &mut dyn Frames, w: Wait) {
        self.game_code(f);
        while !f.done(w) {
            self.wait(WAIT_STEP, f);
        }
    }

    /// The audio frames of a stretch of code the model doesn't cover (a scene's entry), at
    /// the times their driver callbacks ran: their interrupts were taken `AI_CALLBACK` before.
    /// They replace the DMA's schedule until the scene's loop starts, which takes it up from
    /// the last.
    pub fn place_audio_frames(&mut self, callbacks: &[u64]) {
        self.events.retain(|e| e.kind != EventKind::AudioDma);
        for &tb in callbacks {
            self.schedule(tb - AI_CALLBACK, EventKind::AudioDma);
        }
        self.placed_frames = true;
    }

    /// Time passing to `tb` through code the model doesn't cover, its interrupts taken as
    /// their events come due.
    pub fn catch_up(&mut self, tb: u64, f: &mut dyn Frames) {
        self.game_code(f);
        while self.next_at() <= tb {
            self.now = self.now.max(self.next_at());
            self.service(f);
        }
        self.now = self.now.max(tb);
    }

    /// The perf timer's read at the end of a tick.
    pub fn tick_end(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        self.read(TICK_READ, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handlers that take no time, and game code that issues one disc read.
    struct Stub {
        read: bool,
    }

    impl Frames for Stub {
        fn audio_frame(&mut self, base: u64) -> HwLog {
            HwLog::at(base)
        }
        fn disc_done(&mut self, base: u64) -> HwLog {
            HwLog::at(base)
        }
        fn aram_done(&mut self, base: u64) -> HwLog {
            HwLog::at(base)
        }
        fn take_log(&mut self) -> HwLog {
            let mut log = HwLog::default();
            if std::mem::take(&mut self.read) {
                log.disc_read();
            }
            log
        }
        fn done(&mut self, _: Wait) -> bool {
            true
        }
    }

    fn kinds(c: &Clock) -> Vec<EventKind> {
        let mut e = c.events.clone();
        e.sort_by_key(|e| (e.at, e.seq));
        e.into_iter().map(|e| e.kind).collect()
    }

    #[test]
    fn disc_interrupt_takes_back_the_timeout() {
        let mut c = Clock::new(0, AUDIO_FRAME, 675_000, 675_000);
        let mut f = Stub { read: true };
        c.tick_end(&mut f);
        assert!(c.disc_timeout.is_some());
        assert!(kinds(&c).contains(&EventKind::DiscTimeout));
        for _ in 0..3 {
            c.tick_start(&mut f);
            c.tick_end(&mut f);
        }
        assert_eq!(c.disc_timeout, None);
        assert!(!kinds(&c).contains(&EventKind::DiscTimeout));
        assert!(!kinds(&c).contains(&EventKind::DiscDone));
    }

    #[test]
    fn cancelled_events_stop_no_wait() {
        let mut c = Clock::new(0, AUDIO_FRAME, 675_000, 675_000);
        let seq = c.schedule(5_000, EventKind::DspResume);
        c.cancel(seq);
        c.wait(WAIT_STEP, &mut Stub { read: false });
        assert_eq!(c.now, WAIT_STEP);
    }

    #[test]
    fn alarm_set_again_and_dma_stopped() {
        let mut c = Clock::new(0, AUDIO_FRAME, 675_000, 675_000);
        c.set_pad_alarm(1_000_000, 500_000);
        c.stop_audio_dma();
        assert_eq!(kinds(&c), [EventKind::Field, EventKind::PadAlarm]);
        assert_eq!(c.next_at(), FIELD);
        c.cancel_pad_alarm();
        assert_eq!(kinds(&c), [EventKind::Field]);
    }
}
