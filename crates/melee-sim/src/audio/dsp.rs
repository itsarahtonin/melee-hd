//! What the DSP does to the voices' parameter blocks each frame, as ssbm-rs's DSP models it.
//!
//! ssbm-rs's DSP stand-in either leaves the blocks alone (its default: a voice plays until it
//! is keyed off, stolen or dropped) or runs the AX microcode (`AX=1`, `ssbm_ax`, after
//! Dolphin's HLE), which walks each playing voice through its samples and stops it at their
//! end, as the console does. The sample-accurate model here does that walk: the resampler's
//! position, the accelerator's address through ADPCM frames or PCM, looping and stopping, the
//! volume envelope and the bus volume ramps. What depends on the samples' values (the ADPCM
//! decoder's predictor and history, the resampler's last samples and the buses' last mixed
//! samples) is the mix's, which is presentation: those words keep what they held, and the
//! comparison leaves them out in this model.

use super::ax::{Ax, Pb};

/// Which of ssbm-rs's DSP models the simulation follows; a recording carries it, as ssbm-rs's
/// recordings carry their sound flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DspModel {
    /// Voices end only when keyed off, stolen or dropped (ssbm-rs's default).
    #[default]
    Silent,
    /// Voices also end when their samples end (ssbm-rs's `AX=1`, the console's behavior).
    Samples,
}

/// Samples per millisecond, and milliseconds per frame.
const SAMPLES: usize = 32;
const MS: usize = 5;

const START_END_MASK: u32 = 0x3FFF_FFFF;
const CURRENT_MASK: u32 = 0xBFFF_FFFF;

/// The bus volume ramps the early microcode's mixer control enables: every bus ramps with
/// bit 3. Main left and right always mix; aux A with bit 0, aux B with bit 1, and the
/// surround buses with bit 2 (Dolby Pro Logic II, bit 4, mixes a fixed set).
fn mixed_buses(ctrl: u16) -> [bool; 9] {
    // In the PB's mix order: L, R, aux A L, aux A R, aux B L, aux B R, aux B S, S, aux A S.
    let c = u32::from(ctrl);
    if c & 0x10 != 0 {
        let b = c & 0x6 == 0;
        let a = c & 0x7 == 1;
        return [true, true, a, a, b, b, false, false, a];
    }
    let (a, b, s) = (c & 1 != 0, c & 2 != 0, c & 4 != 0);
    [true, true, a, a, b, b, b && s, s, a && s]
}

/// The accelerator walking one voice's samples for a millisecond.
struct Accelerator {
    start: u32,
    end: u32,
    current: u32,
    format: u16,
    stopped: bool,
}

impl Accelerator {
    fn new(pb: &Pb) -> Self {
        Accelerator {
            start: pb.addr.loop_address & START_END_MASK,
            end: pb.addr.end_address & START_END_MASK,
            current: pb.addr.current_address & CURRENT_MASK,
            format: pb.addr.format,
            stopped: false,
        }
    }

    /// One sample's step: the address past it, past an ADPCM frame's header, round a loop,
    /// or stopping the voice at the end of its samples.
    fn step(&mut self, pb: &mut Pb) {
        if self.stopped {
            return;
        }
        let decode = (self.format >> 2) & 3;
        let mut step: u32 = 2;
        if decode == 0 {
            self.current = self.current.wrapping_add(1);
            if self.end & 0xF == 0 && self.current == self.end {
                self.current = self.start.wrapping_add(1);
            } else if self.end & 0xF == 1 && self.current == self.end.wrapping_sub(1) {
                self.current = self.start;
            } else if self.current & 15 == 0 {
                // The frame's header byte, which sets the predictor (the samples').
                self.current = self.current.wrapping_add(2);
                step += 2;
            }
        } else if decode != 1 {
            self.current = self.current.wrapping_add(1);
        }
        if self.current == self.end.wrapping_add(step).wrapping_sub(1) {
            self.current = self.start;
            self.stopped = true;
            if pb.addr.loop_flag != 0 {
                // The loop's predictor and history come back (a stream counts the loop in
                // the block's padding, which isn't modeled); reads go on.
                self.stopped = false;
            } else {
                pb.state = 0;
            }
        }
        self.current &= CURRENT_MASK;
    }
}

/// One millisecond of a voice: the resampler's input steps, the volume envelope and the
/// buses' ramps.
fn voice_ms(pb: &mut Pb) {
    if pb.state != 1 {
        return;
    }
    let mut acc = Accelerator::new(pb);
    let ratio = pb.src.ratio;
    let mut pos = u32::from(pb.src.current_address_frac);
    if pb.src_select <= 1 {
        for _ in 0..SAMPLES {
            pos = pos.wrapping_add(ratio);
            while pos >= 0x10000 {
                acc.step(pb);
                pos -= 0x10000;
            }
        }
    } else {
        for _ in 0..SAMPLES {
            acc.step(pb);
        }
    }
    pb.src.current_address_frac = (pos & 0xFFFF) as u16;
    pb.addr.current_address = acc.current;
    let ve = &mut pb.ve;
    for _ in 0..SAMPLES {
        ve.current_volume = ve.current_volume.wrapping_add(ve.current_delta as u16);
    }
    let ctrl = pb.mixer_ctrl;
    if ctrl & 8 != 0 {
        let m = &mut pb.mix;
        let vols = [
            (&mut m.l, m.delta_l),
            (&mut m.r, m.delta_r),
            (&mut m.aux_a_l, m.delta_aux_a_l),
            (&mut m.aux_a_r, m.delta_aux_a_r),
            (&mut m.aux_b_l, m.delta_aux_b_l),
            (&mut m.aux_b_r, m.delta_aux_b_r),
            (&mut m.aux_b_s, m.delta_aux_b_s),
            (&mut m.s, m.delta_s),
            (&mut m.aux_a_s, m.delta_aux_a_s),
        ];
        for ((v, d), on) in vols.into_iter().zip(mixed_buses(ctrl)) {
            if on {
                *v = v.wrapping_add(d.wrapping_mul(SAMPLES as u16));
            }
        }
    }
}

impl Ax {
    /// The DSP's frame: every voice on the block list, in list order, through five
    /// milliseconds (the command list's PB processing, as `ssbm_ax::Ax::run` does it).
    pub fn dsp_frame(&mut self) {
        let base = self.dsp[0].curr;
        let mut at = Some(0usize);
        let mut seen = 0;
        while let Some(i) = at {
            for _ in 0..MS {
                voice_ms(&mut self.dsp[i]);
            }
            let next = self.dsp[i].next;
            at = (next != 0).then(|| (next.wrapping_sub(base) / 0xC0) as usize);
            seen += 1;
            assert!(seen <= super::ax::VOICES, "the DSP's block list loops");
        }
    }
}
