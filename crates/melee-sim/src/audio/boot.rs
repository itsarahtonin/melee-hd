//! The audio library's setup at boot: `main`'s `lbAudioAx_8002838C` brings up the AX library,
//! the synth, the driver and the device command queue (`HSD_AudioInitMultiPStream`), sets up
//! the aux effects and the three sound banks' room in ARAM; the save data's reset then loads the
//! sound bank (`smash2.sem`) and the boot's banks (`lbAudioAx_80028690`, in `lb::audio`).
//!
//! Everything here runs on ssbm-rs's timeline ([`super::clock`]), which starts at the audio
//! setup: the boot's code before it (the OS, video, disc and pad libraries, HSD's setup) and
//! between the audio library's calls isn't simulated, so where it takes time the timeline is
//! moved to where ssbm-rs's clock has that code end ([`BootTimes`]).

use melee_data::archive::{ArchiveId, Archives};

use super::Audio;
use super::ax::{Ax, PB_WORDS, VOICES as AX_VOICES, Voice};
use super::clock::{AUDIO_FRAME, Clock, EventKind, FIELD};
use super::heap::{Cell, Heap};

/// `HSD_OSInit`'s audio heap: the arena's start after the boot's allocations (the frame
/// buffers, the GX FIFO, the OS heap table), and `iparam_audio_heap_size`.
pub const HEAP_START: u32 = 0x8065_FB20;
/// The same on a console's own boot: the arena past the OS's four heaps' descriptors, which
/// Slippi's bootloader's code list and its two more heaps don't move.
pub const HEAP_START_CONSOLE: u32 = crate::hsd::heap::arena_lo(crate::hsd::heap::CONSOLE_HEAPS);
pub const HEAP_SIZE: u32 = 0x8_0000;

/// ARAM below this is the DSP's, reserved by `ARInit`; allocations start there.
const ARAM_USER_BASE: u32 = 0x4000;

/// `__AXPB`, `__AXITD`, `__AXUpdates`: the DSP's parameter blocks, the ITD buffers and the
/// update buffers, whose addresses each voice's block carries.
const AX_PB: u32 = 0x804B_09E0;
const AX_ITD: u32 = 0x804B_39E0;
const AX_UPDATES: u32 = 0x804B_49E0;
/// The blocks' sizes in those arrays.
const PB_BYTES: u32 = 2 * PB_WORDS as u32;
const ITD_BYTES: u32 = 0x40;
const UPDATES_BYTES: u32 = 0x100;

/// `OS_BUS_CLOCK / 400`: the DSP cycles in an audio frame.
const MAX_DSP_CYCLES: u32 = 162_000_000 / 400;

/// The pad sampling alarm's period (`HSD_PadInit`'s, a 60th of a second).
pub const PAD_ALARM_PERIOD: u64 = 675_000;

/// When the boot's unsimulated code ends on ssbm-rs's clock, measured from its runs (its
/// `AUDIO_TIMELINE` log of a replay's boot, at Slippi's fast disc speed and at a drive's pace;
/// the boot doesn't depend on the replay). Up to the save data's reset no disc read has
/// ended, so those times hold at any disc speed; after it, the boot's times follow its reads.
pub struct BootTimes;

impl BootTimes {
    /// `HSD_AudioInitMultiPStream`'s start, after the OS, video, disc, pad and HSD setup.
    pub const INIT: u64 = 1_360_670;
    /// `__AXOutInit` starts the audio DMA: the AX library's setup, the DSP's included, took
    /// this long.
    pub const DMA_START: u64 = 1_360_710;
    /// The aux effects' setup, after `HSD_AudioInitMultiPStream`'s return.
    pub const AUX: u64 = 1_360_790;
    /// `lbAudioAx_80028690`'s start, after the rest of `main`'s setup up to the save data's
    /// reset.
    pub const RESET: u64 = 1_360_814;
    /// The first scene's preparation (`preloadState`) starts its preload after the boot's
    /// bank loads end (its first read 32 after the last relay read's ARAM request:
    /// `game::boot`), and the scene waits for its first file: its loop's first engine tick
    /// starts this long after that read's interrupt is handled. At Slippi's fast disc speed
    /// the preload's first read starts at 5,412,316 and the tick at 5,442,771; at a drive's
    /// pace the read at 48,470,620 and the tick 48,515,331 into the boot, the same time after
    /// the first file's read.
    pub const FIRST_TICK_AFTER_PRELOAD: u64 = 10_149;
    /// `main`'s `OSGetTick` for the random seed, after HSD's setup and before the audio
    /// setup (a console's boot: the seed is the time base's low word then).
    pub const SEED: u64 = Self::INIT - 9_288;
}

/// How much later a console's own boot with a card in slot A reaches the audio setup in
/// ssbm-rs than Slippi's playback boot: 17 video fields, the OS's probes of the card at boot
/// (`CARDProbeEx` from field 2 to 19), measured from the recorded sessions' boots
/// (`HSD_AudioInitMultiPStream` at 12,847,145, against [`BootTimes::INIT`]); a boot with no
/// card has none (the no-card session's boot mode starts 17 fields earlier). Every later
/// boot time moves with it.
pub const CONSOLE_BOOT_DELAY: u64 = 17 * super::clock::FIELD;

/// [`CONSOLE_BOOT_DELAY`] at a clock `power_on` (time base ticks from 2000-01-01): `OSInit`'s
/// `EXIInit` probes the card first, at the boot's start, which starts `__EXIProbe`'s count of
/// tenths of a second (on the clock); the boot then probes it at each field from field 2
/// (`EXIProbeEx`) until it answers, three tenths after that first probe, and goes on from the
/// field before the one it answers in. At 2000-01-01's clock, ssbm-rs's for version 1
/// sessions, it answers in field 18: 17 fields; a clock whose tenths turn sooner answers
/// sooner. The first probe's time within the boot is taken as its start: ssbm-rs logs it in
/// the boot's first field, and the sessions recorded put it before 769,000 ticks.
pub fn console_boot_delay(power_on: u64) -> u64 {
    use crate::game::session::field_start;
    use crate::lb::save::probe_tenths;
    let first = probe_tenths(0, power_on);
    let mut n = 2;
    while probe_tenths(field_start(n), power_on).wrapping_sub(first) < 3 {
        n += 1;
    }
    (n - 1) * super::clock::FIELD
}

/// How much longer than Slippi's playback boot a console's own boot with a card in slot A
/// takes from the audio setup to the save data's audio reset: measured from the recorded
/// sessions' first bank read (`LastReadIssued` 12,847,369 against the replay boot's). It is
/// the boot's own probes of the card slots, `lbCardGame_Init`'s `CARDProbe(0)` and
/// `lbSnap_8001E290`'s of slots A and B, as the pad alarm's handler makes them with the card
/// in and settled (`lb::save::alarm_card_probes`). One with no card takes as long as the
/// playback boot (the no-card session's reads): ssbm-rs stands in for the CARD library there.
pub const CONSOLE_RESET_EXTRA: u64 =
    2 * (crate::lb::save::EXI_READ + crate::lb::save::TIME_READ) + crate::lb::save::EXI_READ;

/// ARAM's allocator (`ARInit`'s stack): each allocation goes after the last.
pub struct Aram {
    next: u32,
}

impl Aram {
    /// `ARInit`.
    #[doc(alias = "ARInit")]
    pub fn new() -> Aram {
        Aram {
            next: ARAM_USER_BASE,
        }
    }

    /// Where the next allocation goes.
    pub fn next(&self) -> u32 {
        self.next
    }

    /// `ARAlloc`.
    #[doc(alias = "ARAlloc")]
    pub fn alloc(&mut self, len: u32) -> u32 {
        let at = self.next;
        self.next += len;
        at
    }
}

impl Default for Aram {
    fn default() -> Self {
        Aram::new()
    }
}

impl Ax {
    /// `AXInit`'s state: the voice stacks emptied (`__AXAllocInit`, `__AXInitVoiceStacks`), every voice set to its
    /// defaults with its block's addresses and pushed on the free stack (`__AXVPBInit`), no
    /// aux callbacks (`__AXAuxInit`), the command list's mode and position (`__AXClInit`), and
    /// the first command list built with no user callback (`__AXOutInit`). The DSP's setup and
    /// the audio DMA's start are the timeline's.
    #[doc(alias = "AXInit")]
    #[doc(alias = "__AXAllocInit")]
    #[doc(alias = "__AXVPBInit")]
    #[doc(alias = "__AXAuxInit")]
    #[doc(alias = "__AXClInit")]
    #[doc(alias = "__AXOutInit")]
    pub fn init(&mut self) {
        self.callback_stack = None;
        self.stack_head = [None; super::ax::PRIORITY_STACKS];
        self.stack_tail = [None; super::ax::PRIORITY_STACKS];
        self.max_dsp_cycles = MAX_DSP_CYCLES;
        self.rec_dsp_cycles = 0;
        for i in 0..AX_VOICES {
            let pb = AX_PB + PB_BYTES * i as u32;
            let next = if i == AX_VOICES - 1 { 0 } else { pb + PB_BYTES };
            // `__AXSetPBDefault`.
            let mut v = Voice {
                sync: 0xA4,
                ..Voice::default()
            };
            for block in [&mut v.pb, &mut self.dsp[i]] {
                *block = super::ax::Pb::default();
                block.next = next;
                block.curr = pb;
                block.itd.buffer = AX_ITD + ITD_BYTES * i as u32;
                block.update.data = AX_UPDATES + UPDATES_BYTES * i as u32;
            }
            self.voices[i] = v;
            // `__AXPushFreeStack`, which sets the priority the loop gave back to 0.
            self.voices[i].next = self.stack_head[0];
            self.voices[i].priority = 0;
            self.stack_head[0] = Some(i as u8);
        }
        self.aux_a = false;
        self.aux_b = false;
        self.mode = 0;
        self.list_position = 0;
        self.out_frame = 0;
        self.next_frame();
        self.user_callback = None;
    }
}

impl Audio {
    /// `HSD_AudioInitMultiPStream`, on a timeline started here: the driver's voices on its
    /// free list, the synth (`HSD_SynthInit`, with the AX library) and its callbacks into the
    /// driver, and both aux buses off. `bank_size` is the ARAM the sound banks get.
    #[doc(alias = "HSD_AudioInitMultiPStream")]
    pub fn init_multi_pstream(&mut self, data: &Archives, aram: &mut Aram, bank_size: u32) {
        // What the game's data holds before any code runs: the driver's flags leave the
        // streams' second send alone, the synth's sample rate conversion block has a ratio
        // of 1, and the device command queue's ids start at 4.
        self.driver.aux_flags = 2;
        self.synth.src_ratio = 0x1_0000;
        self.devcom.next_id = 4;
        self.heap = Heap {
            size: HEAP_SIZE as i32,
            free: vec![Cell {
                at: if self.console {
                    HEAP_START_CONSOLE
                } else {
                    HEAP_START
                },
                size: HEAP_SIZE,
            }],
            allocated: Vec::new(),
        };
        let now = BootTimes::INIT + self.boot_delay;
        let mut clock = Clock {
            now,
            alarm_period: PAD_ALARM_PERIOD,
            dsp_ready: true,
            tb_power_on: self.power_on,
            // `VIInit` comes after the boot's card probes: one retrace's handler has run
            // since, with or without a card (every session's tick 0).
            retraces: 1,
            // HSD's frame rate count took that retrace, with a new frame (the boot's first
            // copy): `vr_count` and `renew_count` are 1 (ssbm-rs's perf-monitor session).
            vr_count: 1,
            renew_count: 1,
            ..Clock::default()
        };
        clock.schedule((now / FIELD + 1) * FIELD, EventKind::Field);
        clock.schedule(
            clock.next_period(now, PAD_ALARM_PERIOD),
            EventKind::PadAlarm,
        );
        self.clock = Some(clock);
        for v in 0..super::driver::VOICES as u8 {
            self.driver.voices[usize::from(v)].flags &= !super::driver::STATE_MASK;
            self.driver_link(v, true);
        }
        self.synth_init(data, aram, bank_size);
        self.synth.master_clock = Some(super::driver::MASTER_CLOCK);
        self.synth.inactivated = Some(super::driver::KILL_CALLBACK);
        self.synth.pause = Some(super::driver::PAUSE_CALLBACK);
        self.driver_setup_aux(0, 0);
        self.driver_setup_aux(1, 0);
    }

    /// `HSD_SynthInit`: the AX library, the silence the music stream's last box loops to
    /// (cleared in ARAM), the sound banks' and the stream's room in ARAM, the synth's frame
    /// callback, its channels at full volume, and the console's sound mode.
    #[doc(alias = "HSD_SynthInit")]
    fn synth_init(&mut self, data: &Archives, aram: &mut Aram, bank_size: u32) {
        self.ax.init();
        if let Some(log) = &mut self.mix {
            log.next_list(&self.ax);
        }
        let delay = self.boot_delay;
        self.with_clock(data, |c, f| {
            c.catch_up(BootTimes::DMA_START + delay, f);
            c.schedule(c.now + AUDIO_FRAME, EventKind::AudioDma);
        });
        let silence = aram.alloc(0x500);
        self.devcom_request(data, 0, 0, silence, 0x500, 3, 0, None, 0);
        self.synth.stream.silence = silence * 2;
        let banks = &mut self.synth.banks;
        banks.head[0] = aram.alloc(bank_size) as i32;
        self.ax.user_callback = Some(super::SYNTH_CALLBACK);
        self.synth.master = 1.0;
        self.synth.channels.fill(super::synth::Channel {
            volume: 1.0,
            target: 1.0,
            frames: 0,
        });
        let banks = &mut self.synth.banks;
        banks.aram_end = banks.head[0] as u32 + bank_size;
        self.synth.stream.base = aram.alloc(0x3_0000);
        // `OSGetSoundMode`: the SRAM's setting, stereo on ssbm-rs's console.
        self.synth.sound_mode = u32::from(!self.mono);
    }

    /// `AXDriverSetupAux`'s simulated part: the aux effect type the driver records for the
    /// channel's bus, and whether the AX library has a callback for it, which the command
    /// list's cost counts. The effect itself is the presentation's (`melee-audio`).
    #[doc(alias = "AXDriverSetupAux")]
    #[doc(alias = "HSD_AudioSFXSetupAux")]
    pub fn driver_setup_aux(&mut self, channel: u32, kind: u32) {
        let shift = if channel == 0 { 8 } else { 12 };
        let flags = &mut self.driver.aux_flags;
        let old = *flags >> shift & 0xF;
        *flags &= !(old << shift);
        *flags |= kind << shift;
        let on = kind != 0;
        if channel == 0 {
            self.ax.aux_a = on;
        } else {
            self.ax.aux_b = on;
        }
    }

    /// `AXDriver_8038DA70`: the sound bank (`smash2.sem`, `len` bytes, its archive `bank`)
    /// read whole into a block of the audio heap, the game spinning on `lb_800195D0` until
    /// the read's callback; its tables then point into it.
    #[doc(alias = "AXDriver_8038DA70")]
    #[doc(alias = "fn_8038DA5C")]
    pub fn driver_load_bank(&mut self, data: &Archives, bank: ArchiveId, len: u32) {
        let base = self.audio_malloc(len.next_multiple_of(32));
        self.driver.bank_addr = base;
        // `DVDReadAsyncPrio`: one read straight into the block, done after the disc's latency
        // (and its transfer); its callback (`fn_8038DA5C`) ends the spin.
        self.hw.disc_read(len.next_multiple_of(32), self.disc);
        self.driver.reading_bank = true;
        self.with_clock(data, |c, f| c.spin_until(f, super::clock::Wait::BankRead));
        let file = data.get(bank).data();
        let word = |at: usize| u32::from_be_bytes(file[at..at + 4].try_into().unwrap());
        let d = &mut self.driver;
        d.bank = Some(bank);
        // The file's tables in order: the effects' (not kept), the sounds' command streams
        // (relocated), the banks' and the streams'.
        let mut at = 4 + 4 * word(0) as usize;
        let n = word(at) as usize;
        at += 4 + 4 * n;
        d.banks = word(at) as i32;
        d.bank_table = (at + 4) as u32;
        at += 4 + 4 * d.banks as usize;
        d.streams = word(at) as i32;
        d.stream_table = (at + 4) as u32;
    }

    /// `AXDriver_8038DCFC`: the sound bank's block freed.
    #[doc(alias = "AXDriver_8038DCFC")]
    pub fn driver_free_bank(&mut self) {
        if self.driver.bank.is_some() {
            self.audio_free(self.driver.bank_addr);
        }
        self.driver.bank = None;
        self.driver.bank_addr = 0;
    }
}
