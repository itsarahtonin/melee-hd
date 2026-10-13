//! The audio driver's bookkeeping, which is simulation (`docs/architecture.md`): playing a
//! sound returns a handle game code keeps, and which voices are free, and so the handles new
//! sounds get, depends on the driver's frames ending voices. So melee-sim runs HAL's driver
//! ([`driver`]), the synth under it ([`synth`]) and the AX library's voice allocation
//! ([`ax`]) as the original does, on the schedule ssbm-rs's audio interrupts keep relative to
//! engine ticks ([`clock`]). Mixing and output are the presentation layer's: it reads the
//! voices' parameter blocks, which describe exactly what the DSP would play.

pub mod ax;
pub mod bank;
pub mod boot;
pub mod clock;
pub mod devcom;
pub mod driver;
pub mod dsp;
pub mod heap;
pub mod mix;
pub mod stream;
pub mod synth;

use melee_data::archive::Archives;

pub use clock::HwLog;

use crate::game::handles::CodeRef;

/// `HSD_SynthCallback`, the AX user callback the synth registers.
pub const SYNTH_CALLBACK: CodeRef = CodeRef(0x8038_ABD4);

/// Everything the audio driver keeps.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Audio {
    pub ax: ax::Ax,
    pub synth: synth::Synth,
    pub driver: driver::Driver,
    /// The device command queue the music stream and sound bank loads read through.
    pub devcom: devcom::DevCom,
    /// The audio heap the queue's nodes and the banks' effect groups live in.
    pub heap: heap::Heap,
    /// Time the audio code's hardware accesses take, for the timeline (empty between its
    /// steps).
    pub hw: HwLog,
    /// Which of ssbm-rs's DSP models the voices follow.
    pub dsp: dsp::DspModel,
    /// The game's audio library's state (`melee/lb/lbaudio_ax.c`).
    pub lb: crate::lb::audio::LbAudio,
    /// When audio frames run; none where the state came without a timeline (a single
    /// captured call), and no frame runs then.
    pub clock: Option<clock::Clock>,
    /// Presentation output, not state: when the presentation turns it on, each frame's mix
    /// and each ARAM write is recorded here for it to mix (`mix::MixLog`). A clone starts
    /// without it and a restore leaves it alone.
    #[clone_from(keep)]
    pub mix: Option<mix::MixLog>,
}

impl Audio {
    /// `__AXOutNewFrame`, an AX frame as far as it decides voices: the parameter blocks'
    /// sync (which drops voices past the DSP's budget), the dropped voices' callbacks, the
    /// user callback (the synth's frame, and the driver's in it) and the next command list's
    /// cost. The aux effects' processing and the DSP's mix are presentation.
    #[doc(alias = "__AXOutNewFrame")]
    pub fn ax_frame(&mut self, data: &Archives) {
        let pre = self.mix.is_some().then(|| mix::PreSync::of(&self.ax));
        self.ax.sync_pbs(0);
        // `__AXGetCommandListAddress`: the list the DSP is handed, and the next one.
        self.ax.list_position ^= 1;
        if let (Some(log), Some(pre)) = (&mut self.mix, &pre) {
            log.push_frame(&self.ax, pre);
        }
        // The DSP runs the command list as the mail hands it over.
        if self.dsp == dsp::DspModel::Samples {
            self.ax.dsp_frame();
        }
        self.service_callback_stack();
        match self.ax.user_callback {
            None => {}
            Some(SYNTH_CALLBACK) => self.synth_frame(data),
            Some(o) => panic!("AX user callback {:#010x} isn't ported", o.0),
        }
        self.ax.next_frame();
        if let Some(log) = &mut self.mix {
            log.next_list(&self.ax);
        }
        self.ax.out_frame = (self.ax.out_frame + 1) & 1;
    }

    /// `__AXServiceCallbackStack`, with the dropped voices' callbacks reaching the synth.
    pub fn service_callback_stack(&mut self) {
        let mut dropped = Dropped::default();
        self.ax
            .service_callback_stack(&mut |_, cb, v| dropped.push(cb, v));
        // Each callback ran before its voice was freed; dropcallback doesn't touch the AX
        // library's stacks, so running them after in the same order is the same.
        for &(cb, v) in dropped.list() {
            match cb {
                ax::DROP_CALLBACK => self.synth_drop(v),
                other => panic!("AX voice callback {:#010x} isn't ported", other.0),
            }
        }
    }

    /// Runs the timeline up to the next tick's game code, with the audio frames before it.
    pub fn tick_start(&mut self, data: &Archives) {
        self.with_clock(data, |c, f| c.tick_start(f));
    }

    /// The end of a tick's game code.
    pub fn tick_end(&mut self, data: &Archives) {
        self.with_clock(data, |c, f| c.tick_end(f));
    }

    /// Game code outside the timeline's model (a scene's end and the next one's entry) ran
    /// until time `tb`: what came due by then runs.
    pub fn clock_catch_up(&mut self, data: &Archives, tb: u64) {
        self.with_clock(data, |c, f| c.catch_up(tb, f));
    }

    /// The audio frames of a stretch of code outside the timeline's model, by when their
    /// driver callbacks ran.
    pub fn clock_place_audio_frames(&mut self, data: &Archives, callbacks: &[u64]) {
        self.with_clock(data, |c, _| c.place_audio_frames(callbacks));
    }

    /// A scene's loop ends, and the next scene's preparation runs.
    pub fn end_scene_loop(&mut self, data: &Archives) {
        self.with_clock(data, |c, f| c.end_scene_loop(f));
    }

    /// A scene's loop starts.
    pub fn start_scene_loop(&mut self, data: &Archives) {
        self.with_clock(data, |c, f| c.start_scene_loop(f));
    }

    /// An ARAM load of `len` bytes inside a tick.
    pub fn aram_load(&mut self, data: &Archives, len: u32) {
        self.with_clock(data, |c, f| c.aram_load(len, f));
    }

    pub(crate) fn with_clock(
        &mut self,
        data: &Archives,
        run: impl FnOnce(&mut clock::Clock, &mut dyn clock::Frames),
    ) {
        let Some(mut c) = self.clock.take() else {
            return;
        };
        let mut f = Running { audio: self, data };
        run(&mut c, &mut f);
        self.clock = Some(c);
    }
}

/// Voices the AX library dropped, with their callbacks, run once it's done: at most every
/// voice, kept without allocating.
pub(crate) struct Dropped {
    list: [(CodeRef, ax::VoiceId); ax::VOICES],
    len: usize,
}

impl Default for Dropped {
    fn default() -> Self {
        Dropped {
            list: [(CodeRef(0), 0); ax::VOICES],
            len: 0,
        }
    }
}

impl Dropped {
    pub(crate) fn push(&mut self, cb: CodeRef, v: ax::VoiceId) {
        self.list[self.len] = (cb, v);
        self.len += 1;
    }

    pub(crate) fn list(&self) -> &[(CodeRef, ax::VoiceId)] {
        &self.list[..self.len]
    }
}

/// The audio state and the data its frames read, as the timeline's owner.
struct Running<'a> {
    audio: &'a mut Audio,
    data: &'a Archives,
}

impl clock::Frames for Running<'_> {
    fn audio_frame(&mut self, base: u64) -> HwLog {
        self.audio.hw = HwLog::at(base);
        self.audio.ax_frame(self.data);
        std::mem::take(&mut self.audio.hw)
    }

    fn disc_done(&mut self, base: u64) -> HwLog {
        self.audio.hw = HwLog::at(base);
        if self.audio.driver.reading_bank {
            // `fn_8038DA5C`, the sound bank read's callback.
            self.audio.driver.reading_bank = false;
        } else {
            self.audio.devcom_dvd_done(self.data);
        }
        std::mem::take(&mut self.audio.hw)
    }

    fn aram_done(&mut self, base: u64) -> HwLog {
        self.audio.hw = HwLog::at(base);
        self.audio.devcom_aram_done(self.data);
        std::mem::take(&mut self.audio.hw)
    }

    fn take_log(&mut self) -> HwLog {
        std::mem::take(&mut self.audio.hw)
    }

    fn done(&mut self, w: clock::Wait) -> bool {
        let b = &self.audio.synth.banks;
        match w {
            clock::Wait::FileLoaded => self.audio.devcom.file_loaded,
            clock::Wait::SfxQueueRoom => b.load_count < 6,
            clock::Wait::SfxLoadsDone => b.load_count == 0,
            clock::Wait::BankRead => !self.audio.driver.reading_bank,
        }
    }
}
