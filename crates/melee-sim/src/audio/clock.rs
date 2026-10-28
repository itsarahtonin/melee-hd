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

/// The pad library's queue (`HSD_PadInit`'s `qnum` in `main`): a round of the scene's loop
/// ticks once per poll the queue holds (`lb_80019894`), however many more the alarm read while
/// the loop was held (the queue merges the overflow, `HSD_PadRenewRawStatus`).
const PAD_QUEUE: u32 = 5;
/// From the audio interrupt to its DSP mail, and to the end of its handler.
const AI_MAIL: u64 = 16;
const AI_HANDLER: u64 = 120;
/// From the render's wait to its sleep: the draw time's read (`HSD_PerfSetDrawTime`'s
/// `OSGetTime`), after which `HSD_VIGetXFBDrawEnable` turns interrupts on and takes what came
/// due; and from the wake to the pad check.
const BEFORE_SLEEP: u64 = 16;
/// From the copy to the pad check: `GXSetDrawDone`'s finish interrupt, taken at once with
/// interrupts off (`GXFinishInterruptHandler`), then the total time's read
/// (`HSD_PerfSetTotalTime`'s `OSGetTime`), each a point where interrupts that came due are
/// taken (ssbm-rs's probe of `trophy-sessions/coll-l`, its disc interrupt at time base 2,496,845,386 taken
/// as the finish handler returns).
const GX_FINISH: u64 = 8;
const TOTAL_TIME_READ: u64 = 16;
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
const ARAM_CALLBACK: u64 = 8;
/// A memory card segment's transfer-complete interrupt's handler (EXI0's TC, OS interrupt 10,
/// `TCIntrruptHandler`), from its delivery to the next segment's DMA: the deselect (24),
/// `BlockReadCallback`'s `__CARDReadSegment` selecting again (64 later), its three commands 16
/// apart and the 512-byte DMA 32 after them, as ssbm-rs's probe of the boot's card load logs
/// every segment of it (`local/research/card-chain`). The transfer completes as the DMA starts
/// and raises the next interrupt at once.
pub const CARD_SEGMENT: u64 = 160;
/// The OS interrupts of slot A's channel (EXI0): the card's own (`EXIIntrruptHandler`, which
/// a sector's erase or a page's program raises as it finishes) and a transfer's completion
/// (`TCIntrruptHandler`). Both come after ARAM's (6) and before the disc's (21).
pub const EXI0_EXI: u8 = 9;
pub const EXI0_TC: u8 = 10;
/// A memory card sector's erase (`__CARDEraseSector`), with interrupts off: `EXILock`'s mask
/// (16), the select (16), the 3-byte command waited for (16), the deselect with its unmask and
/// probe (24) and `EXIUnlock`'s mask (16), as ssbm-rs's probe of a save's writes logs every
/// erase (`local/research/card-chain/write-*.log`). ssbm-rs's card finishes the erase at once
/// and raises the card's interrupt.
pub const CARD_ERASE: u64 = 88;
/// The card's interrupt's handler up to its callback: `EXIIntrruptHandler`'s status read (8),
/// then `__CARDExiHandler`'s lock (16), `__CARDReadStatus` (80) and `__CARDClearStatus` (56)
/// and the unlock (16).
const CARD_STATUS: u64 = 176;
/// `__CARDWritePage` from a callback (the erase's, or the last page's `BlockWriteCallback`):
/// the lock (16), the select (16), its two commands waited for (32), and the 128-byte DMA's
/// interrupt clear and unmask (16). The program finishes as the DMA starts and raises the
/// transfer-complete interrupt.
const CARD_PAGE: u64 = 80;
/// A page's transfer-complete interrupt's handler: `TCIntrruptHandler`'s mask and read (16),
/// `__CARDTxHandler`'s deselect with its unmask and probe (24) and the unlock (16). The deselect
/// ends the program, which raises the card's interrupt.
const CARD_PAGE_DONE: u64 = 56;
/// A sector's pages of 128 bytes (`__CARDWrite` of 0x2000 bytes).
const CARD_PAGES: u32 = 64;
/// A file's directory entry stamped with the time (`OSGetTime`): `CARDWrite`'s `WriteCallback`
/// before the directory's update, and `CARDSetStatusAsync`.
pub const CARD_STAMP: u64 = 16;
/// How long before an audio frame starts on this timeline ssbm-rs runs the audio DMA event
/// that raises it: its frames start (and their handler's times count) from 8 ticks after the
/// event, so an [`EventKind::AudioDma`] here is due `AI_LEAD` before its time. ssbm-rs runs
/// each event due at a wait point or an interrupt check, then delivers one interrupt, then
/// looks again; an event that falls due while a handler runs (interrupts off) runs when the
/// handler returns, with everything else due by then, in time order. So a block's end due
/// before another event by its time on ssbm-rs's clock goes first even when its frame here
/// starts after that event (2026-10-10_12-54-40's tick 705: the frame due at 562,837,254
/// here, its event at 246, holds off a disc read's end at 248 until the audio handler's
/// return at 374); and one due by a handler's return starts `AI_LEAD` after it.
const AI_LEAD: u64 = 8;
/// Where the movie player's alarm's handler runs (`fn_8001F2A4`), and the decrementer
/// exception's time after it: none, as the sessions' traces have it (the handler reads no
/// register, and the OS takes it in the pad alarm's exception).
const MOVIE_ALARM_CALLBACK: u64 = 0;
const MOVIE_ALARM_TAIL: u64 = 0;
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
    /// The movie player's alarm fires (`fn_8001F2A4`, every 60th of a second).
    MovieAlarm,
    /// The preload heaps' copy manager's alarm fires (`fn_80015184`), copying a compacted
    /// heap's block in slices.
    CopyAlarm,
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
    /// A memory card transfer's interrupts still to deliver ([`Clock::card_chain`]), the next
    /// one last. The chain always ends before the call that starts it returns, so it's never
    /// state between the clock's calls.
    #[clone_from(keep)]
    pub exi_chain: Vec<ExiIrq>,
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
    /// Whether the DSP has answered the last frame's mail (`__AXOutDspReady` 1).
    pub dsp_ready: bool,
    /// Audio frames are placed ([`Clock::place_audio_frames`]) rather than scheduled; the last
    /// one's start.
    pub placed_frames: bool,
    pub last_frame: Option<u64>,
    /// Rounds of a scene's loop that start without waiting for a frame buffer: its first
    /// (no render before it) and its second (a buffer is still free).
    pub scene_start: u8,
    /// The movie player's alarm's period.
    pub movie_period: u64,
    /// The round's display pass has run after the render's wait ([`Clock::render_start`]).
    pub render_started: bool,
    /// A new tick length the next round's pad check takes up, setting the pad alarm again
    /// at that period (`lb_80019628`).
    pub new_alarm_period: Option<u64>,
    /// What the code's reads of the time base add to [`Clock::now`]: the console's clock at
    /// power-on, in time base ticks from 2000-01-01 (`session::Setup::power_on`, ssbm-rs's
    /// `Regs::tb_power_on`); 0 for a replay's console. The timeline itself counts from
    /// power-on, as the hardware does; only what the code computes from `OSGetTime` (the
    /// alarms' grid, the clock's seconds) sees it.
    pub tb_power_on: u64,
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
    /// The movie player's alarm.
    fn movie_alarm(&mut self, base: u64) -> HwLog {
        HwLog::at(base)
    }
    /// The preload heaps' copy manager's alarm.
    fn copy_alarm(&mut self, base: u64) -> HwLog {
        HwLog::at(base)
    }
    /// A handler starts at time base `now` (its log's `base` counts from there).
    fn handler_at(&mut self, _now: Option<u64>) {}
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
    /// The game's own wait (`lb_800195D0`) at `now` runs the save's writer (`lb_8001CC84`):
    /// its card work there; none when no write is under way.
    fn game_wait(&mut self, now: u64) -> Option<CardWork> {
        let _ = now;
        None
    }
    /// The save's list unmounted the card ([`CardWork::unmount`]), detaching slot A's channel.
    fn card_unmounted(&mut self) {}
}

/// The save's writer's card work at one of the game's waits ([`Frames::game_wait`]), in
/// order: a task step's wait point (`hsd_803AAA48`), time with interrupts off (a probe, the
/// unmount), a transfer's interrupt chain ([`Clock::card_chain`]), and for the list's last
/// step the unmount's detaching slot A's channel ([`Frames::card_unmounted`]), after the
/// step's wait point: a pad alarm that fires in that wait still finds the channel attached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CardWork {
    pub step: bool,
    pub ticks: u64,
    pub chain: Option<CardChain>,
    pub unmount: bool,
}

/// One interrupt of a memory card transfer's chain: its OS interrupt ([`EXI0_EXI`] or
/// [`EXI0_TC`]) and its handler's time from its delivery to its return.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct ExiIrq {
    pub irq: u8,
    pub ticks: u64,
}

/// A memory card transfer as the CARD library runs it on ssbm-rs's card, whose transfers,
/// erases and programs finish at once ([`Clock::card_chain`]): the code that asks for it with
/// interrupts off up to its first transfer, then the interrupts each transfer's end raises,
/// whose handlers start the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum CardChain {
    /// A read of `segments` 512-byte segments (`__CARDRead`): `first` ticks up to the first
    /// segment's DMA, then each segment's transfer-complete handler issues the next
    /// ([`CARD_SEGMENT`]), the last one's taking `tail` (the deselect and the callbacks).
    Read {
        first: u64,
        segments: u32,
        tail: u64,
    },
    /// A write of `sectors` sectors, each erased and then programmed a page at a time: `first`
    /// ticks up to and through the first sector's erase ([`CARD_ERASE`]); then for each sector
    /// the erase's interrupt (its status read and cleared, `EraseCallback` starting the first
    /// page) and 64 pages, each a transfer-complete interrupt that ends its program and the
    /// card's interrupt whose `BlockWriteCallback` starts the next. The last page's callback
    /// ends the sector: before the next one, `CARDWrite`'s `WriteCallback` stamps the file's
    /// directory entry ([`CARD_STAMP`]) and `__CARDUpdateDir` erases the directory's sector;
    /// after the last, the request's callback (the HSD layer's) returns at once. A file's
    /// sector written (`CARDWriteAsync` of 0x2000 bytes) is two sectors, the file's and the
    /// directory's; a status set (`CARDSetStatusAsync`) the directory's alone. ssbm-rs's
    /// probe of a save's writes (`local/research/card-chain/write-*.log`) logs every one of
    /// the 258 interrupts of each of a save's writes, each page 312 ticks.
    Write { first: u64, sectors: u32 },
}

impl CardChain {
    /// The time before the first interrupt, with interrupts off.
    pub fn first(self) -> u64 {
        match self {
            CardChain::Read { first, .. } | CardChain::Write { first, .. } => first,
        }
    }

    /// The chain's interrupts, in the order they come, into `out`.
    pub fn irqs(self, out: &mut Vec<ExiIrq>) {
        let tc = |ticks| ExiIrq {
            irq: EXI0_TC,
            ticks,
        };
        let exi = |ticks| ExiIrq {
            irq: EXI0_EXI,
            ticks,
        };
        match self {
            CardChain::Read { segments, tail, .. } => {
                assert!(segments > 0, "a card read of no segments");
                out.extend(std::iter::repeat_n(tc(CARD_SEGMENT), segments as usize - 1));
                out.push(tc(tail));
            }
            CardChain::Write { sectors, .. } => {
                assert!(sectors > 0, "a card write of no sectors");
                for s in 0..sectors {
                    out.push(exi(CARD_STATUS + CARD_PAGE));
                    for _ in 1..CARD_PAGES {
                        out.push(tc(CARD_PAGE_DONE));
                        out.push(exi(CARD_STATUS + CARD_PAGE));
                    }
                    out.push(tc(CARD_PAGE_DONE));
                    let next = if s + 1 < sectors {
                        CARD_STAMP + CARD_ERASE
                    } else {
                        0
                    };
                    out.push(exi(CARD_STATUS + next));
                }
            }
        }
    }
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
    /// The movie player has no read in flight (`lbMthp_8001F800`'s spin on `fn_8001F294`).
    MovieIdle,
    /// No sound group's move in ARAM pending (`HSD_SynthSFXBankDeflagSync`'s spin).
    BanksMoved,
}

/// Time a stretch of the audio code took at the hardware, and what it started: a disc read (by
/// the time into the stretch its command went out, and the time its bytes take to transfer
/// after the latency; a second one can't start before it ends), ARAM transfers, which
/// finish at once and interrupt, and the copy manager's alarm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct HwLog {
    pub spent: u64,
    pub read: Option<u64>,
    pub transfer: u64,
    pub aram: u32,
    /// The copy manager's alarm set (`OSSetAlarm` in `lbMemory_80015320`'s copies), by
    /// when it fires counted from the stretch's start.
    pub copy_alarm: Option<u64>,
    /// The log is an interrupt handler's.
    pub handler: bool,
}

/// A disc read's command, from `DVDReadAsyncPrio`'s entry to the drive's control register's
/// write, from the game's code as from an interrupt handler's callback (the device command
/// queue's next read): ssbm-rs's `DVD_TIMELINE` has every read of the sessions' 24 ticks
/// from its `read` line to its `di-start`. The handler's work after it, and the game's code
/// going on from a wait a read's interrupt ended, take no time.
const DISC_ISSUE: u64 = 24;
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
        self.spent += DISC_ISSUE;
        assert!(self.read.is_none(), "two disc reads in flight");
        self.read = Some(self.spent);
        self.transfer = speed.transfer(len);
    }

    /// A hardware register is read.
    pub fn reg_read(&mut self) {
        self.spent += REG_READ;
    }

    /// An ARAM transfer is posted (and done). `ARQPostRequest` starts it at once
    /// (`ARStartDMA`'s seven register reads) only when no transfer is under way; else it only
    /// queues it, and the ARAM interrupt's handler that pops it starts it
    /// ([`HwLog::arq_start`]).
    pub fn arq_post(&mut self, starts: bool) {
        if starts {
            self.spent += ARQ_POST;
        }
        self.aram += 1;
    }

    /// A queued ARAM transfer starts from the interrupt handler of the one before
    /// (`__ARQInterruptServiceRoutine`'s `ARStartDMA`, after the finished one's callback).
    pub fn arq_start(&mut self) {
        self.spent += ARQ_POST;
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

    /// The time base as the code reads it now (`OSGetTime`).
    pub fn code_time(&self) -> u64 {
        self.now.wrapping_add(self.tb_power_on)
    }

    /// When a periodic alarm whose start is `period` (as the pad alarm's, `lb_80019AAC` and
    /// `lb_80019894`) set at `at` first fires: `InsertAlarm`'s next multiple of the period on
    /// the code's time base, back on the timeline's. ssbm-rs's system time offset (its boot's
    /// `0x800030D8`) is a whole number of seconds, so it leaves the grid where it is.
    pub fn next_period(&self, at: u64, period: u64) -> u64 {
        let code = at.wrapping_add(self.tb_power_on);
        (code / period + 1)
            .wrapping_mul(period)
            .wrapping_sub(self.tb_power_on)
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

    /// Sets the movie player's alarm to fire at `first` and every `period` after, on the
    /// timeline after the pad alarm when both are due at once (`OSSetPeriodicAlarm`).
    pub fn set_movie_alarm(&mut self, first: u64, period: u64) {
        self.cancel_movie_alarm();
        self.movie_period = period;
        self.schedule(first, EventKind::MovieAlarm);
    }

    /// Cancels the movie player's alarm (`OSCancelAlarm`).
    pub fn cancel_movie_alarm(&mut self) {
        self.events.retain(|e| e.kind != EventKind::MovieAlarm);
    }

    /// Stops the audio DMA: the block playing raises no interrupt.
    pub fn stop_audio_dma(&mut self) {
        self.events.retain(|e| e.kind != EventKind::AudioDma);
    }

    /// When an event runs on ssbm-rs's clock: its time, but for an audio DMA block's end,
    /// [`AI_LEAD`] before.
    fn due_at(e: &Event) -> u64 {
        match e.kind {
            EventKind::AudioDma => e.at.saturating_sub(AI_LEAD),
            _ => e.at,
        }
    }

    /// The next event, by when it runs then scheduling order.
    fn next_event(&self) -> Option<usize> {
        (0..self.events.len()).min_by_key(|&i| (Self::due_at(&self.events[i]), self.events[i].seq))
    }

    fn next_at(&self) -> u64 {
        Self::due_at(&self.events[self.next_event().expect("the timeline runs dry")])
    }

    /// Runs what is due now: each event due, then the interrupts they raised, one at a time in
    /// the OS's order (the audio interrupt, ARAM's, a card transfer's, the disc's, the
    /// retrace), looking again after each, until nothing is. An event that fell due while a
    /// handler ran runs as it returns ([`AI_LEAD`]).
    fn service(&mut self, f: &mut dyn Frames) {
        loop {
            let now = self.now;
            if let Some(i) = self
                .next_event()
                .filter(|&i| Self::due_at(&self.events[i]) <= now)
            {
                let e = self.events.swap_remove(i);
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
                        // Its frame starts `AI_LEAD` after the event runs.
                        let start = self.now + AI_LEAD;
                        self.ai_pending = true;
                        if self.placed_frames {
                            self.last_frame = Some(start);
                        } else {
                            self.schedule(start + AUDIO_FRAME, EventKind::AudioDma);
                        }
                    }
                    EventKind::DspResume => self.dsp_ready = true,
                    EventKind::DiscDone => self.di_pending = true,
                    EventKind::DiscTimeout => panic!("a disc read timed out"),
                    EventKind::MovieAlarm => {
                        self.schedule(e.at + self.movie_period, EventKind::MovieAlarm);
                        let start = self.now;
                        f.handler_at(Some(start));
                        let log = f.movie_alarm(MOVIE_ALARM_CALLBACK);
                        f.handler_at(None);
                        self.apply(start, &log);
                        self.now = start + log.spent + MOVIE_ALARM_TAIL;
                    }
                    EventKind::CopyAlarm => {
                        let start = self.now;
                        f.handler_at(Some(start));
                        let log = f.copy_alarm(0);
                        f.handler_at(None);
                        self.apply(start, &log);
                        self.now = start + log.spent;
                    }
                }
            } else if self.ai_pending {
                self.ai_pending = false;
                assert!(
                    self.dsp_ready,
                    "an audio frame before the DSP finished the last one"
                );
                let start = self.now + AI_LEAD;
                self.dsp_ready = false;
                self.schedule(start + AI_MAIL + DSP_FRAME, EventKind::DspResume);
                f.handler_at(Some(start));
                let log = f.audio_frame(AI_CALLBACK);
                f.handler_at(None);
                let end = start + AI_HANDLER + (log.spent - AI_CALLBACK);
                self.apply(start, &log);
                // What fell due meanwhile (a disc read's end) runs as the handler returns.
                self.now = end;
            } else if self.aram_pending != 0 {
                self.aram_pending -= 1;
                let start = self.now;
                f.handler_at(Some(start));
                let log = f.aram_done(ARAM_CALLBACK);
                f.handler_at(None);
                self.apply(start, &log);
                self.now = start + log.spent;
            } else if let Some(irq) = self.exi_chain.pop() {
                // A card transfer's interrupt (EXI0's): its handler starts the next transfer,
                // which raises the next interrupt as it ends, or ends the chain.
                self.now += irq.ticks;
            } else if self.di_pending {
                self.di_pending = false;
                self.disc_interrupt(f);
            } else if self.vi_pending {
                self.vi_pending = false;
                self.now += VI_HANDLER;
                self.woken = true;
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
        if let Some(seq) = self.disc_timeout.take() {
            self.cancel(seq);
        }
        let start = self.now;
        f.handler_at(Some(start));
        let log = f.disc_done(DI_CALLBACK);
        f.handler_at(None);
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
        if let Some(at) = log.copy_alarm {
            self.events.retain(|e| e.kind != EventKind::CopyAlarm);
            self.schedule(start + at, EventKind::CopyAlarm);
        }
    }

    /// The game's code since the last handler: its hardware time passes, with interrupts
    /// enabled.
    fn game_code(&mut self, f: &mut dyn Frames) {
        let log = f.take_log();
        if log.spent != 0 || log.read.is_some() || log.aram != 0 || log.copy_alarm.is_some() {
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

    /// One of the game's own waits (`lb_800195D0`): time passes to the next event or a
    /// quarter millisecond, then the save's writer runs ([`Frames::game_wait`]).
    fn lb_wait(&mut self, f: &mut dyn Frames) {
        self.wait(WAIT_STEP, f);
        if let Some(work) = f.game_wait(self.now) {
            self.card_work(work, f);
        }
    }

    /// The save's writer's card work ([`CardWork`]).
    pub fn card_work(&mut self, work: CardWork, f: &mut dyn Frames) {
        if work.step {
            self.wait_point(f);
        }
        if work.ticks != 0 {
            self.pass(work.ticks, f);
        }
        if let Some(chain) = work.chain {
            self.card_chain(chain, f);
        }
        if work.unmount {
            f.card_unmounted();
        }
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
            // The render's wait, unless the display pass ran after it ([`Clock::render_start`]).
            if !std::mem::take(&mut self.render_started) && start < 2 {
                self.lb_wait(f);
            }
            if start < 2 && self.xfb_free() == 0 {
                // The sleep until a retrace frees a frame buffer (`HSD_VIWaitXFBDrawEnable`).
                self.read(BEFORE_SLEEP, f);
                while self.xfb_free() == 0 {
                    let next = self.next_at();
                    self.now = self.now.max(next);
                    self.service(f);
                }
                self.read(GX_FINISH, f);
                self.read(TOTAL_TIME_READ, f);
            } else if start < 2 {
                // A buffer is free: the draw time's read, then the frame's GX finish
                // interrupt and the total time's read before the pad check, as after a sleep's
                // wake.
                self.read(BEFORE_SLEEP, f);
                self.read(GX_FINISH, f);
                self.read(TOTAL_TIME_READ, f);
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
                self.set_pad_alarm(self.next_period(self.now, p), p);
            }
            // The scene loop's wait for a pad sample, and one more wait after it. The round
            // runs a tick for each sample the queue holds (`lb_80019894`'s count): no more than
            // its five, however often the alarm fired.
            while self.pads == 0 {
                self.lb_wait(f);
            }
            self.ticks_left = self.pads.min(PAD_QUEUE);
            self.lb_wait(f);
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
        self.lb_wait(f);
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
        if !std::mem::take(&mut self.render_started) {
            self.lb_wait(f);
        }
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

    /// A round's display pass is about to run (a session's, whose pass can read the clock
    /// and the disc): the render's wait (`lb_800195D0`) runs first, as `gm_801A4D34` has it,
    /// so the pass's time reads and disc reads fall where the original's do; the next tick's
    /// start takes it up from there.
    pub fn render_start(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        if self.scene_start < 2 && !self.render_started {
            self.lb_wait(f);
            self.render_started = true;
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

    /// A memory card transfer ([`CardChain`]): the game's code asks for it with interrupts off
    /// up to its first transfer, then each transfer's interrupt's handler starts the next.
    /// Interrupts stay off in every handler and never come back on between them: the chain is
    /// back-to-back deliveries, and an event that comes due meanwhile runs as a handler returns,
    /// its interrupt ahead of the chain's next when it comes first in the OS's order (the audio
    /// interrupt's, 5, and ARAM's, 6, before the card's, 9 and 10), behind it otherwise (the
    /// disc's, 21, and the retrace's, 24, after the whole chain). ssbm-rs completes each
    /// transfer as it starts, so these are its boundaries, not a console's.
    pub fn card_chain(&mut self, chain: CardChain, f: &mut dyn Frames) {
        self.game_code(f);
        assert!(self.exi_chain.is_empty(), "a card transfer in flight");
        self.now += chain.first();
        chain.irqs(&mut self.exi_chain);
        self.exi_chain.reverse();
        self.service(f);
        debug_assert!(self.exi_chain.is_empty());
    }

    /// Time a tick's ARAM load takes (`ARAM_CHUNK` per 4 KB), with interrupts taken between
    /// its chunks. In ssbm-rs the load's request and each chunk's ARAM interrupt handler run
    /// with interrupts off, and an event that came due meanwhile runs when a handler returns:
    /// the request takes 56 ticks, each chunk's handler 64 and the last one's 8, so on this
    /// timeline (whose audio frames start [`AI_LEAD`] after ssbm-rs runs their events) the
    /// chunks end every `ARAM_CHUNK` from the load's start. The last handler returns as the
    /// load ends, and an audio DMA event due by then runs there too, before the game's code
    /// goes on: its frame starts `AI_LEAD` after the load's end.
    pub fn aram_load(&mut self, len: u32, f: &mut dyn Frames) {
        self.game_code(f);
        for _ in 0..len.div_ceil(0x1000) {
            self.read(ARAM_CHUNK, f);
        }
    }

    /// The game waiting inside a tick for its file load: `lbFile`'s loop, a wait point each
    /// spin (`lb_800195D0`), until the load's callback.
    pub fn wait_for_load(&mut self, f: &mut dyn Frames) {
        self.game_code(f);
        loop {
            self.lb_wait(f);
            if f.done(Wait::FileLoaded) {
                break;
            }
        }
    }

    /// The game waiting for `w` in a loop that checks before each wait point, or a spin
    /// ssbm-rs's SDK hooks (`install`'s `spins`): a wait point each spin until `w`.
    pub fn wait_until(&mut self, f: &mut dyn Frames, w: Wait) {
        self.game_code(f);
        while !f.done(w) {
            self.wait(WAIT_STEP, f);
        }
    }

    /// The game spinning on its own wait (`lb_800195D0`, the callback the audio library's
    /// loads spin with) until `w`: each spin a wait point that also runs the save's writer
    /// ([`Frames::game_wait`]).
    pub fn spin_until(&mut self, f: &mut dyn Frames, w: Wait) {
        self.game_code(f);
        while !f.done(w) {
            self.lb_wait(f);
        }
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
        // The retrace's handler ends at FIELD + 32. A frame due by AI_LEAD after that (its
        // event due by the handler's end on ssbm-rs's clock) starts then; one due later waits.
        for (due, ran) in [
            (FIELD + VI_HANDLER + AI_LEAD, true),
            (FIELD + VI_HANDLER + AI_LEAD + 1, false),
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

    /// A block's read, as the save's list reads one.
    const BLOCK: CardChain = CardChain::Read {
        first: 96,
        segments: 16,
        tail: 64,
    };

    #[test]
    fn a_card_read_takes_events_between_its_segments() {
        // A block's read from 1,000: its first segment's DMA 96 later, then fifteen handlers
        // of 160 and the last one's 64: it ends at 1,000 + 96 + 2,400 + 64 = 3,560.
        let end = 1_000 + 96 + 15 * CARD_SEGMENT + 64;
        let mut c = Clock::new(1_000, 10 * AUDIO_FRAME, 675_000, 675_000);
        c.card_chain(BLOCK, &mut Stub::new(false));
        assert_eq!(c.now, end);
        // An audio frame due inside the third handler (its event at 1,500 on ssbm-rs's clock)
        // runs at that handler's return, 1,000 + 96 + 3 * 160 = 1,576, and its handler goes
        // ahead of the chain's next: the read ends the audio handler's 128 later.
        let mut c = Clock::new(1_000, 1_500 + AI_LEAD, 675_000, 675_000);
        c.card_chain(BLOCK, &mut Stub::new(false));
        assert!(kinds(&c).contains(&EventKind::DspResume));
        let ran = c
            .events
            .iter()
            .find(|e| e.kind == EventKind::AudioDma)
            .unwrap()
            .at;
        assert_eq!(ran, 1_576 + AI_LEAD + AUDIO_FRAME);
        assert_eq!(c.now, end + AI_LEAD + AI_HANDLER);
        // A retrace due inside the read waits for the whole chain: its handler after the last.
        let mut c = Clock::new(FIELD - 200, 10 * AUDIO_FRAME, 10 * FIELD, 675_000);
        c.card_chain(BLOCK, &mut Stub::new(false));
        assert_eq!(
            c.now,
            FIELD - 200 + 96 + 15 * CARD_SEGMENT + 64 + VI_HANDLER
        );
        assert!(c.woken);
    }

    #[test]
    fn a_card_write_takes_events_between_its_interrupts() {
        // `2026-10-06_16-58-24`'s second write of the save (ssbm-rs's probe,
        // `write-16-58.log`): `CARDWriteAsync` from 277,485,778, the file's sector and the
        // directory's, 40,480 ticks with nothing due, as `2026-10-06_17-44-10`'s two writes.
        let write = CardChain::Write {
            first: CARD_ERASE,
            sectors: 2,
        };
        let start = 277_485_778;
        let mut irqs = Vec::new();
        write.irqs(&mut irqs);
        assert_eq!(irqs.len(), 258);
        assert_eq!(irqs.iter().filter(|i| i.irq == EXI0_EXI).count(), 130);
        let mut c = Clock::new(start, start + 100_000, 10 * FIELD * 1_000, 675_000);
        c.card_chain(write, &mut Stub::new(false));
        assert_eq!(c.now, start + 40_480);
        // The audio event due at 277,514,677 on ssbm-rs's clock runs at 277,514,794, as the
        // directory's 28th page's card interrupt returns, and its handler goes ahead of the
        // next page's transfer-complete interrupt: the write ends 128 later.
        let mut c = Clock::new(start, 277_514_677 + AI_LEAD, 10 * FIELD * 1_000, 675_000);
        c.card_chain(write, &mut Stub::new(false));
        let ran = c
            .events
            .iter()
            .find(|e| e.kind == EventKind::AudioDma)
            .unwrap()
            .at;
        assert_eq!(ran, 277_514_794 + AI_LEAD + AUDIO_FRAME);
        // Its handler starts then, not after the chain: the DSP's answer is due from it.
        let dsp = c
            .events
            .iter()
            .find(|e| e.kind == EventKind::DspResume)
            .unwrap()
            .at;
        assert_eq!(dsp, 277_514_794 + AI_LEAD + AI_MAIL + DSP_FRAME);
        assert_eq!(c.now, 277_526_386);
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
