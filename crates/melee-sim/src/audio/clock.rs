//! When audio frames run relative to engine ticks, as ssbm-rs's timeline places them.
//!
//! The original runs the driver's frame in the audio interrupt, every 5 ms of audio. ssbm-rs
//! keeps virtual time: it moves only at wait points (the scene loop's waits, a thread sleep)
//! and by a few time base ticks at each time base or hardware register read, and interrupts
//! are taken there. In play, audio frames are taken at wait points between ticks, or inside a
//! tick at the few reads there (the perf timer's, and fighters' ARAM animation loads, chunk by
//! chunk), so where a frame falls follows from a handful of clocks this module runs:
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
//! `docs/engine.md`).

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
/// Time the pad alarm's handler takes besides its memory card probes ([`Frames::pad_alarm`]):
/// all of it on ssbm-rs's console without a card at power-on (a replay's, Slippi's playback
/// boot's), whose CARD library is stood in for.
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
/// fast disc speed), before the transfer time a [`DiscSpeed`] adds.
pub const DISC_LATENCY: u64 = 20_250;
/// Time base ticks a second.
pub const TB_HZ: u64 = 40_500_000;
/// A GameCube drive's average read rate, in bytes a second: the pace ssbm-rs's window reads
/// at.
pub const DRIVE_RATE: u32 = 3_000_000;
/// The disc driver's timeout for a read (`DVDLowRead`'s alarm: 10 s of the time base).
pub const DISC_TIMEOUT: u64 = 10 * TB_HZ;
/// Where the audio interrupt's handler runs the user callback, where the disc interrupt's
/// handler runs the disc driver's callback, and the ARAM interrupt's handler its own.
const AI_CALLBACK: u64 = 64;
const DI_CALLBACK: u64 = 32;
/// From the disc interrupt's handler's end to an event that came due while it ran (an audio
/// DMA block's end, measured at a drive's pace, where the boot's bank loads' interrupts meet
/// one).
const DI_TAIL: u64 = 8;
const ARAM_CALLBACK: u64 = 8;
/// How long before the audio interrupt's handler returns the OS's dispatcher takes a disc
/// interrupt raised meanwhile (measured from a console's boot, where a bank read's interrupt
/// falls in an audio frame's handler).
const AI_CHAIN: u64 = 16;

/// How fast the disc reads: a match setting, since when reads finish decides when the music
/// stream's commands reach the DSP and so which voices are free (see `docs/engine.md`).
///
/// HSD starts a song with `AXSetVoiceAddr`, which carries its loop flag, and sets the stream's
/// addresses five reads later (0x20 bytes, then four of 0x4000). At the fast disc's latency
/// alone those often fall within one 5 ms audio frame, so the SDK's `__AXSyncPBs` copies only
/// the addresses to the DSP, the stale loop flag stops the voice at the end of its first
/// buffer, and the song goes silent about 1.8 s in. A drive's pace keeps them frames apart: any
/// rate under about 13 MB/s does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum DiscSpeed {
    /// Each read finishes [`DISC_LATENCY`] after its command, as ssbm-rs's runs read unless told
    /// otherwise (Slippi's fast disc speed): what the oracle's traces and the replay checks
    /// were made with.
    #[default]
    Fast,
    /// A drive's pace, as ssbm-rs's window reads (`Hw::set_disc_rate`, and `DISC_RATE=3000000`
    /// in any run): reads also move their bytes at [`DRIVE_RATE`], after the latency. What the
    /// player reads at.
    Drive,
}

impl DiscSpeed {
    /// Time base ticks a read of `len` bytes takes to transfer, on top of its latency.
    pub fn transfer(self, len: u32) -> u64 {
        match self {
            DiscSpeed::Fast => 0,
            DiscSpeed::Drive => u64::from(len) * TB_HZ / u64::from(DRIVE_RATE),
        }
    }

    /// The speed a command line names: `fast` or `drive`.
    pub fn parse(name: &str) -> Option<DiscSpeed> {
        match name {
            "fast" => Some(DiscSpeed::Fast),
            "drive" => Some(DiscSpeed::Drive),
            _ => None,
        }
    }
}

/// What happens at a time on the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Event {
    pub at: u64,
    /// The order events due at the same time run in: the order they were scheduled.
    pub seq: u64,
    pub kind: EventKind,
}

/// The timeline's state.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
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
    /// HSD's two frame buffers (`HSD_VIStatus`'s `xfb`): how many copied frames wait to show
    /// (`HSD_VI_XFB_NEXT` and `DRAWDONE`), and whether one shows (`DISPLAY`); the rest are
    /// free. A copy needs a free one. At power-on neither has been drawn.
    pub xfb_pending: u8,
    pub xfb_shown: bool,
    /// The pad alarm's firings since the game last looked, with what their card probes found
    /// (a session's; none elsewhere).
    pub alarm_times: Option<Vec<AlarmProbe>>,
    /// A disc read's interrupt was taken since the game's code last waited.
    pub disc_woke: bool,
    /// Whether the DSP has answered the last frame's mail (`__AXOutDspReady` 1).
    pub dsp_ready: bool,
    /// Audio frames are placed ([`Clock::place_audio_frames`]) rather than scheduled; the last
    /// one's start.
    pub placed_frames: bool,
    pub last_frame: Option<u64>,
    /// Rounds of a scene's loop that start without waiting for a frame buffer: its first
    /// (no render before it) and its second (a buffer is still free).
    pub scene_start: u8,
    /// A new tick length the next round's pad check takes up, setting the pad alarm again
    /// at that period (`lb_80019628`).
    pub new_alarm_period: Option<u64>,
}

/// A firing of the pad alarm: when, and whether its handler's probe of slot A found a card
/// that answers (`CARDProbe(0)`, [`Frames::pad_alarm`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct AlarmProbe {
    pub at: u64,
    pub card: bool,
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
    /// The pad alarm's handler (`fn_800195FC`) probing the memory card slots at `now`, after
    /// its controllers' poll: the time its probes take, and whether slot A's card answered.
    /// On ssbm-rs's console without a card at power-on the CARD library is stood in for, and
    /// they take none.
    fn pad_alarm(&mut self, now: u64) -> (u64, bool) {
        let _ = now;
        (0, false)
    }
    /// Whether what the game waits for has happened.
    fn done(&mut self, w: Wait) -> bool;
}

/// What the game waits for in a loop with a wait point each spin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Wait {
    /// Its file load's callback (`lbFile`'s `cancel`).
    FileLoaded,
    /// Room in the sound bank load queue (`HSD_SynthSFXLoad`'s spin).
    SfxQueueRoom,
    /// No sound bank load queued (`HSD_SynthSFXWaitForLoadCompletion`).
    SfxLoadsDone,
    /// The boot's read of the sound bank (`AXDriver_8038DA70`'s spin).
    BankRead,
    /// No music box read in flight (`HSD_SynthPStreamStart`'s spin).
    StreamIdle,
    /// This many of the first scene's preloaded files are in (their callbacks have run).
    Preloaded(u32),
    /// The preload cache has the file of this disc entry ready, or doesn't hold it
    /// (`lbDvd_800189EC`'s spin on `lbDvd_800187F4`).
    PreloadFile(i32),
    /// The preload cache let go of every entry of this heap, none being read
    /// (`lbDvd_80017598`'s spin).
    HeapReleased(i32),
    /// The preload cache let go of this heap's unwanted entries already set up
    /// (`lbDvd_80017700`'s spin on `lbDvd_80017644`).
    StaleReleased(i32),
}

/// Time a stretch of the audio code took at the hardware, and what it started: a disc read (by
/// the time into the stretch its command went out, and the time its bytes take to transfer
/// after the latency; a second one can't start before it ends) and ARAM transfers, which
/// finish at once and interrupt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct HwLog {
    pub spent: u64,
    pub read: Option<u64>,
    pub transfer: u64,
    pub aram: u32,
    /// The log is an interrupt handler's.
    pub handler: bool,
}

/// A disc read's command from the game's code, and from an interrupt handler's callback (the
/// device command queue's next read), and an ARAM transfer's request.
const DISC_ISSUE: u64 = 8;
const DISC_ISSUE_HANDLER: u64 = 24;
/// After a read's command from a handler's callback, the rest of the handler's work.
const DISC_ISSUED_HANDLER: u64 = 16;
/// The game's code going on from a wait a disc read's interrupt ended.
const DISC_WAKE: u64 = 16;
/// A hardware register's read (ssbm-rs's read step).
const REG_READ: u64 = 8;
const ARQ_POST: u64 = 56;

impl HwLog {
    /// An interrupt handler's log, its callback `base` into it.
    pub fn at(base: u64) -> HwLog {
        HwLog {
            spent: base,
            handler: true,
            ..HwLog::default()
        }
    }

    /// A disc read's command goes out, for `len` bytes at `speed`.
    pub fn disc_read(&mut self, len: u32, speed: DiscSpeed) {
        self.spent += if self.handler {
            DISC_ISSUE_HANDLER
        } else {
            DISC_ISSUE
        };
        assert!(self.read.is_none(), "two disc reads in flight");
        self.read = Some(self.spent);
        if self.handler {
            self.spent += DISC_ISSUED_HANDLER;
        }
        self.transfer = speed.transfer(len);
    }

    /// A hardware register is read.
    pub fn reg_read(&mut self) {
        self.spent += REG_READ;
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
            // As a match's tick leaves them: one frame showing, the last copied waiting.
            xfb_pending: 1,
            xfb_shown: true,
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
        // An event that came due while the disc or retrace interrupt's handler ran happens
        // `DI_TAIL` after the handler. That holds for an audio DMA block that ends by the
        // handler's end on ssbm-rs's clock, whose frame starts `DI_TAIL` past its event here.
        let mut after_disc = false;
        loop {
            let tail = std::mem::take(&mut after_disc);
            let due = |e: &Event| {
                e.at <= self.now
                    || (tail && e.kind == EventKind::AudioDma && e.at <= self.now + DI_TAIL)
            };
            if let Some(i) = self.next_event().filter(|&i| due(&self.events[i])) {
                let e = self.events.swap_remove(i);
                if tail {
                    self.now += DI_TAIL;
                }
                match e.kind {
                    EventKind::Field => {
                        self.vi_pending = true;
                        self.schedule(e.at + FIELD, EventKind::Field);
                    }
                    EventKind::PadAlarm => {
                        self.now += ALARM_HANDLER;
                        let (probes, card) = f.pad_alarm(self.now);
                        self.now += probes;
                        if let Some(t) = &mut self.alarm_times {
                            t.push(AlarmProbe { at: e.at, card });
                        }
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
                // A disc read that finished while the handler ran: the OS's dispatcher takes
                // its interrupt before the handler's return (`AI_CHAIN` before its end), and
                // the return's time after.
                let chained = self
                    .events
                    .iter()
                    .filter(|e| e.kind == EventKind::DiscDone && e.at + AI_CHAIN <= end)
                    .min_by_key(|e| (e.at, e.seq))
                    .map(|e| e.seq);
                match chained {
                    Some(seq) => {
                        self.cancel(seq);
                        self.now = end - AI_CHAIN;
                        self.disc_interrupt(f);
                        self.now += AI_CHAIN;
                        after_disc = true;
                    }
                    None => self.now = end,
                }
            } else if self.aram_pending != 0 {
                self.aram_pending -= 1;
                let start = self.now;
                let log = f.aram_done(ARAM_CALLBACK);
                self.apply(start, &log);
                self.now = start + log.spent;
            } else if self.di_pending {
                self.di_pending = false;
                self.disc_interrupt(f);
                after_disc = true;
            } else if self.vi_pending {
                self.vi_pending = false;
                self.now += VI_HANDLER;
                self.woken = true;
                after_disc = true;
                // `HSD_VIPostRetraceCB`: the next copied frame shows, freeing the one shown.
                if self.xfb_pending > 0 {
                    self.xfb_pending -= 1;
                    self.xfb_shown = true;
                }
            } else {
                break;
            }
        }
    }

    /// The disc interrupt's handler: the read's timeout cancelled, then its callback.
    fn disc_interrupt(&mut self, f: &mut dyn Frames) {
        self.disc_woke = true;
        if let Some(seq) = self.disc_timeout.take() {
            self.cancel(seq);
        }
        let start = self.now;
        let log = f.disc_done(DI_CALLBACK);
        self.apply(start, &log);
        self.now = start + log.spent;
    }

    /// Schedules a handler's disc reads and raises its ARAM transfers' interrupts.
    fn apply(&mut self, start: u64, log: &HwLog) {
        if let Some(r) = log.read {
            assert!(self.disc_timeout.is_none(), "two disc reads in flight");
            self.schedule(start + r + DISC_LATENCY + log.transfer, EventKind::DiscDone);
            self.disc_timeout =
                Some(self.schedule(start + r + DISC_TIMEOUT, EventKind::DiscTimeout));
        }
        self.aram_pending += log.aram;
    }

    /// The game's code since the last handler: its hardware time passes, with interrupts
    /// enabled.
    fn game_code(&mut self, f: &mut dyn Frames) {
        let log = f.take_log();
        if log.spent != 0 || log.read.is_some() || log.aram != 0 {
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

    /// A wait point of the game's (`lb_800195D0`, the card layer's step `hsd_803AAA48`):
    /// time passes to the next event or a quarter millisecond, its interrupts taken.
    pub fn wait_point(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        self.wait(WAIT_STEP, f);
    }

    /// The game's code reading the hardware's registers `reads` times, interrupts taken at
    /// each read where they come due.
    pub fn reg_reads(&mut self, reads: u64, f: &mut dyn Frames) {
        self.game_code(f);
        for _ in 0..reads {
            self.read(REG_READ, f);
        }
    }

    /// The game's code spending `ticks` at the hardware: its log so far, then the time, with
    /// interrupts taken where they come due.
    pub fn pass(&mut self, ticks: u64, f: &mut dyn Frames) {
        self.game_code(f);
        self.read(ticks, f);
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
            if start < 2 && self.xfb_free() == 0 {
                // The sleep until a retrace frees a frame buffer (`HSD_VIWaitXFBDrawEnable`).
                self.now += BEFORE_SLEEP;
                while self.xfb_free() == 0 {
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
            // The frame's copy.
            self.woken = false;
            if start < 2 {
                self.xfb_pending += 1;
            }
            // The pad check (`lb_80019894`) takes up a new tick length first: the alarm is
            // cancelled and set again to fire every period (`OSSetPeriodicAlarm` with the
            // period as its start), at the time base's next multiple of it (`InsertAlarm`).
            if let Some(p) = self.new_alarm_period.take() {
                self.set_pad_alarm((self.now / p + 1) * p, p);
            }
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
        // The copy shows; nothing waits.
        self.xfb_pending = 0;
        self.xfb_shown = true;
    }

    /// [`Clock::end_scene_loop`], `copied` when the loop's last round copied a frame: the
    /// flush (`HSD_VIWaitXFBFlush`) waits until every copied frame has shown.
    pub fn end_scene_loop_after(&mut self, copied: bool, f: &mut dyn Frames) {
        self.game_code(f);
        self.wait(WAIT_STEP, f);
        if copied {
            self.xfb_pending += 1;
        }
        self.woken = false;
        while self.xfb_pending > 0 {
            let next = self.next_at();
            self.now = self.now.max(next);
            self.service(f);
        }
    }

    /// Frame buffers neither showing nor waiting to.
    fn xfb_free(&self) -> u8 {
        2 - self.xfb_pending - u8::from(self.xfb_shown)
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

    /// Time a tick's ARAM load takes (`ARAM_CHUNK` per 4 KB), with interrupts taken between
    /// its chunks. In ssbm-rs the load's request and each chunk's ARAM interrupt handler run
    /// with interrupts off, and an event that came due meanwhile runs when a handler returns:
    /// the request takes 56 ticks, each chunk's handler 64 and the last one's 8, so on this
    /// timeline (whose audio frames start 8 ticks after ssbm-rs runs their events) the
    /// chunks end every `ARAM_CHUNK` from the load's start. The last handler returns as the
    /// load ends, and an audio DMA event due by then runs there too, its frame starting after
    /// the audio handler's register read: here a frame due up to `REG_READ` past the load's
    /// end starts there, before the game's code goes on.
    pub fn aram_load(&mut self, len: u32, f: &mut dyn Frames) {
        self.game_code(f);
        for _ in 0..len.div_ceil(0x1000) {
            self.read(ARAM_CHUNK, f);
        }
        let end = self.now + REG_READ;
        if self
            .events
            .iter()
            .any(|e| e.kind == EventKind::AudioDma && e.at <= end)
        {
            self.read(REG_READ, f);
        }
    }

    /// The game waiting inside a tick for its file load: `lbFile`'s loop, a wait point each
    /// spin (`lb_800195D0`), until the load's callback.
    pub fn wait_for_load(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        self.disc_woke = false;
        loop {
            self.wait(WAIT_STEP, f);
            if f.done(Wait::FileLoaded) {
                break;
            }
        }
        self.after_disc_wait(f);
    }

    /// A wait a disc read's interrupt ended: the game's code takes `DISC_WAKE` to go on.
    fn after_disc_wait(&mut self, f: &mut dyn Frames) {
        if std::mem::take(&mut self.disc_woke) {
            self.read(DISC_WAKE, f);
        }
    }

    /// The game waiting for `w` in a loop that checks before each wait point.
    pub fn wait_until(&mut self, f: &mut dyn Frames, w: Wait) {
        self.game_code(f);
        self.disc_woke = false;
        let mut waited = false;
        while !f.done(w) {
            self.wait(WAIT_STEP, f);
            waited = true;
        }
        if waited {
            self.after_disc_wait(f);
        }
        self.disc_woke = false;
    }

    /// `VIWaitForRetrace`: time passes, its interrupts taken, until the next retrace's handler
    /// has run.
    pub fn wait_for_retrace(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        self.woken = false;
        while !self.woken {
            let next = self.next_at();
            self.now = self.now.max(next);
            self.service(f);
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

    /// The boot's first engine tick starts at `at`, where ssbm-rs's clock has the boot's first
    /// scene start its loop and take the tick: the pad samples queued by then are flushed to
    /// the newest (the loop's prologue), and the tick takes it.
    pub fn first_tick(&mut self, at: u64, f: &mut dyn Frames) {
        self.catch_up(at - TICK_READ, f);
        self.pads = self.pads.min(1);
        assert!(self.pads == 1, "the first tick before a pad sample");
        self.pads = 0;
        self.ticks_left = 0;
        self.scene_start = 0;
        // Slippi's boot has run its own scenes: a frame shows, the last copied waiting.
        self.xfb_pending = 1;
        self.xfb_shown = true;
        self.read(TICK_READ, f);
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

    /// Handlers that take no time, and game code that issues one disc read of 0x4000 bytes at
    /// `speed`.
    struct Stub {
        read: bool,
        speed: DiscSpeed,
    }

    impl Stub {
        fn new(read: bool) -> Stub {
            Stub {
                read,
                speed: DiscSpeed::Fast,
            }
        }
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
                log.disc_read(0x4000, self.speed);
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
        let mut f = Stub::new(true);
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
    fn a_drive_reads_at_its_pace() {
        // 0x4000 bytes at 3 MB/s: 5.46 ms, longer than an audio frame.
        for (speed, transfer) in [(DiscSpeed::Fast, 0), (DiscSpeed::Drive, 221_184)] {
            let mut c = Clock::new(0, AUDIO_FRAME, 675_000, 675_000);
            c.tick_end(&mut Stub { read: true, speed });
            let done = c.events.iter().find(|e| e.kind == EventKind::DiscDone);
            assert_eq!(
                done.map(|e| e.at),
                Some(DISC_ISSUE + DISC_LATENCY + transfer)
            );
        }
        assert!(DiscSpeed::Drive.transfer(0x4000) > AUDIO_FRAME);
    }

    #[test]
    fn a_frame_due_as_an_aram_load_ends_runs_before_the_game_goes_on() {
        // A load of two chunks ends at 128. A frame due within the audio handler's register
        // read after that (136 on this timeline) starts at 136, as ssbm-rs takes its event
        // when the last chunk's handler returns; one due later waits for the game's code.
        for (due, ran) in [(129, true), (136, true), (137, false)] {
            let mut c = Clock::new(0, due, 675_000, 675_000);
            c.aram_load(0x2000, &mut Stub::new(false));
            assert_eq!(
                kinds(&c).contains(&EventKind::DspResume),
                ran,
                "due at {due}"
            );
            let end = if ran { 136 + AI_HANDLER } else { 128 };
            assert_eq!(c.now, end, "due at {due}");
        }
    }

    #[test]
    fn a_frame_due_as_the_retrace_handler_ends_runs_then() {
        // The retrace's handler ends at FIELD + 32. A frame due by DI_TAIL after that (its
        // event due by the handler's end on ssbm-rs's clock) starts then; one due later waits.
        for (due, ran) in [
            (FIELD + VI_HANDLER + DI_TAIL, true),
            (FIELD + VI_HANDLER + DI_TAIL + 1, false),
        ] {
            let mut c = Clock::new(FIELD - 1, due, 10 * FIELD, 675_000);
            c.wait(WAIT_STEP, &mut Stub::new(false));
            assert_eq!(
                kinds(&c).contains(&EventKind::DspResume),
                ran,
                "due at {due}"
            );
        }
    }

    /// Handlers that take no time, and a pad alarm whose card probes take 56 ticks.
    struct Probing;

    impl Frames for Probing {
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
            HwLog::default()
        }
        fn done(&mut self, _: Wait) -> bool {
            true
        }
        fn pad_alarm(&mut self, _: u64) -> (u64, bool) {
            (56, true)
        }
    }

    #[test]
    fn the_alarm_handler_takes_its_card_probes() {
        let mut c = Clock::new(0, 10 * AUDIO_FRAME, 1_000, 675_000);
        c.alarm_times = Some(Vec::new());
        c.wait(WAIT_STEP, &mut Probing);
        assert_eq!(c.now, 1_000 + ALARM_HANDLER + 56);
        assert_eq!(
            c.alarm_times,
            Some(vec![AlarmProbe {
                at: 1_000,
                card: true
            }])
        );
    }

    #[test]
    fn cancelled_events_stop_no_wait() {
        let mut c = Clock::new(0, AUDIO_FRAME, 675_000, 675_000);
        let seq = c.schedule(5_000, EventKind::DspResume);
        c.cancel(seq);
        c.wait(WAIT_STEP, &mut Stub::new(false));
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
