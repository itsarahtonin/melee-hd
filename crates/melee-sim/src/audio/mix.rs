//! Presentation output: what the DSP mixes each audio frame and what reaches ARAM, recorded
//! for a mixer outside the simulation (`melee-audio`). Nothing here is simulation state, and
//! no simulated value reads it: the log is off (`Audio::mix` is `None`) unless the
//! presentation turns it on, and the presentation empties it as it mixes. Its vectors keep
//! their storage, so recording allocates nothing once warmed up.

use super::ax::{Ax, Pb, VOICES};

/// The frames and ARAM transfers since the presentation last emptied the log.
#[derive(Debug, Default)]
pub struct MixLog {
    pub frames: Vec<MixFrame>,
    /// Writes to ARAM in the order they completed, each placed before the frame it precedes.
    pub aram: Vec<AramWrite>,
    /// The command list `__AXNextFrame` built at the last frame's end, which the next frame's
    /// DSP runs; none before the first frame the log saw.
    next_list: Option<List>,
}

/// What `__AXNextFrame` put in a command list: the mode's main-bus setup and the aux mixes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct List {
    /// `__AXClMode`.
    pub mode: u32,
    /// Whether aux A's and B's callbacks are registered. The list mixes aux B when A's is
    /// (`__AXGetAuxBInput` checks A's callback), which the mixer follows.
    pub aux_a: bool,
    pub aux_b: bool,
}

impl List {
    fn of(ax: &Ax) -> List {
        List {
            mode: ax.mode,
            aux_a: ax.aux_a,
            aux_b: ax.aux_b,
        }
    }
}

/// One audio frame as the DSP mixes it.
#[derive(Clone, Copy, Debug)]
pub struct MixFrame {
    /// The DSP's parameter blocks after `__AXSyncPBs`, as the command list's DSP reads them.
    /// The words that depend on the samples' values (the decoder's predictor and history, the
    /// resampler's last samples, the buses' last samples) hold what the simulation kept,
    /// which is the CPU's copy where `sync` says it wrote them.
    pub pbs: [Pb; VOICES],
    /// Each voice's sync flags as `__AXSyncPBs` serviced it (`AX_SYNC_FLAG_*`).
    pub sync: [u32; VOICES],
    /// The voices `__AXDepopVoice` added to the studio's depop sums: those whose depop flag
    /// was set, and those dropped for the DSP's budget while playing (a voice can be both).
    pub depop: u64,
    pub dropped: u64,
    /// The command list the DSP runs.
    pub list: List,
    /// The mode and aux callbacks as the frame's `__AXProcessAux` finds them.
    pub now: List,
}

/// One transfer into ARAM, which completes at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AramWrite {
    /// How many of the log's frames came before it.
    pub frame: u32,
    pub dest: u32,
    pub size: u32,
    pub from: AramSource,
}

/// Where an ARAM write's bytes come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AramSource {
    /// A disc file, by entry number (`Audio::devcom`'s files), from this offset.
    Disc { entrynum: i32, offset: u32 },
    /// ARAM itself, from this address.
    Aram(u32),
    /// Zeros.
    Zero,
    /// Main memory at this address, which the log doesn't hold.
    Memory(u32),
}

/// What `__AXSyncPBs` resets that the log needs, read before it runs.
#[derive(Clone, Copy)]
pub struct PreSync {
    sync: [u32; VOICES],
    depop: u64,
    playing: u64,
}

impl PreSync {
    pub fn of(ax: &Ax) -> PreSync {
        let mut p = PreSync {
            sync: [0; VOICES],
            depop: 0,
            playing: 0,
        };
        for (i, v) in ax.voices.iter().enumerate() {
            p.sync[i] = v.sync;
            p.depop |= u64::from(v.depop != 0) << i;
            p.playing |= u64::from(ax.dsp[i].state == 1) << i;
        }
        p
    }
}

impl MixLog {
    /// Records the frame the DSP is about to mix, after `__AXSyncPBs` (`pre` read before it).
    pub fn push_frame(&mut self, ax: &Ax, pre: &PreSync) {
        let now = List::of(ax);
        let list = self.next_list.unwrap_or(now);
        // The voices dropped this frame are on the callback stack, serviced after the mix.
        let mut dropped = 0u64;
        let mut at = ax.callback_stack;
        while let Some(p) = at {
            dropped |= 1 << p;
            at = ax.voices[usize::from(p)].next1;
        }
        self.frames.push(MixFrame {
            pbs: ax.dsp,
            sync: pre.sync,
            depop: pre.depop,
            dropped: dropped & pre.playing,
            list,
            now,
        });
    }

    /// The command list `__AXNextFrame` builds for the next frame.
    pub fn next_list(&mut self, ax: &Ax) {
        self.next_list = Some(List::of(ax));
    }

    /// Records a write into ARAM, completed now.
    pub fn aram(&mut self, dest: u32, size: u32, from: AramSource) {
        self.aram.push(AramWrite {
            frame: self.frames.len() as u32,
            dest,
            size,
            from,
        });
    }

    /// Empties the log, keeping its storage.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.aram.clear();
    }
}
