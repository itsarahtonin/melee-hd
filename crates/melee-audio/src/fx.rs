//! The aux effects HAL's driver registers (`AXDriverSetupAux`): the AXFX library's standard
//! reverb (`dolphin/axfx/reverb_std.c`) and delay (`delay.c`), which process a frame's aux
//! buses on the CPU and hand them back to the DSP. Melee sets up the reverb on aux A and the
//! delay on aux B at boot (`lbAudioAx_8002838C`); their state, delay lines included, starts
//! from the boot state's memory.
//!
//! The reverb's float math follows the original's instructions through `gekko_fp`, in the
//! simulation's float mode, as ssbm-rs runs them.

use melee_sim::gekko_fp as fp;

/// Samples per bus in a frame.
const SAMPLES: usize = 160;

/// Main memory as the boot state holds it, and the game's symbols' addresses.
pub struct BootMemory<'a> {
    pub mem1: &'a [u8],
    pub sym: &'a dyn Fn(&str) -> u32,
}

impl BootMemory<'_> {
    pub fn u32(&self, addr: u32) -> u32 {
        let i = (addr & 0x01FF_FFFF) as usize;
        self.mem1
            .get(i..i + 4)
            .map_or(0, |b| u32::from_be_bytes(b.try_into().unwrap()))
    }

    pub fn words(&self, addr: u32, n: usize) -> Vec<u32> {
        (0..n).map(|i| self.u32(addr + 4 * i as u32)).collect()
    }
}

/// `AXFX_REVSTD_DELAYLINE`: positions and length in bytes, inputs as float bits.
#[derive(Clone, Debug, Default)]
struct DelayLine {
    in_point: usize,
    out_point: usize,
    length: usize,
    inputs: Vec<u32>,
    last_output: f64,
}

impl DelayLine {
    fn from_memory(m: &BootMemory, at: u32) -> DelayLine {
        let length = m.u32(at + 8) as usize;
        DelayLine {
            in_point: m.u32(at) as usize,
            out_point: m.u32(at + 4) as usize,
            length,
            inputs: m.words(m.u32(at + 0xC), length / 4),
            last_output: fp::lfs(m.u32(at + 0x10)),
        }
    }

    fn store(&mut self, v: f64) {
        self.inputs[self.in_point / 4] = fp::stfs(v);
        self.in_point += 4;
        if self.in_point == self.length {
            self.in_point = 0;
        }
    }

    fn load(&mut self) -> f64 {
        let v = fp::lfs(self.inputs[self.out_point / 4]);
        self.out_point += 4;
        if self.out_point == self.length {
            self.out_point = 0;
        }
        v
    }
}

/// `AXFX_REVERBSTD`.
#[derive(Clone, Debug)]
pub struct ReverbStd {
    ap: [DelayLine; 6],
    c: [DelayLine; 6],
    all_pass_coeff: f64,
    comb_coef: [f64; 6],
    lp_lastout: [f64; 3],
    level: f64,
    damping: f64,
    pre_delay_time: usize,
    pre_delay_line: [Vec<u32>; 3],
    pre_delay_ptr: [usize; 3],
    temp_disable_fx: bool,
}

impl ReverbStd {
    /// The reverb at `at` (an `AXFX_REVERBSTD`) in the boot state's memory.
    pub fn from_memory(m: &BootMemory, at: u32) -> ReverbStd {
        let line = |i: u32| DelayLine::from_memory(m, at + 0x14 * i);
        let f = |o: u32| fp::lfs(m.u32(at + o));
        let pre_delay_time = m.u32(at + 0x120) as usize;
        let pre_line = |k: u32| m.u32(at + 0x124 + 4 * k);
        ReverbStd {
            ap: std::array::from_fn(|i| line(i as u32)),
            c: std::array::from_fn(|i| line(6 + i as u32)),
            all_pass_coeff: f(0xF0),
            comb_coef: std::array::from_fn(|i| f(0xF4 + 4 * i as u32)),
            lp_lastout: std::array::from_fn(|i| f(0x10C + 4 * i as u32)),
            level: f(0x118),
            damping: f(0x11C),
            pre_delay_time,
            pre_delay_line: std::array::from_fn(|k| {
                if pre_delay_time == 0 {
                    Vec::new()
                } else {
                    m.words(pre_line(k as u32), pre_delay_time)
                }
            }),
            pre_delay_ptr: std::array::from_fn(|k| {
                let ptr = m.u32(at + 0x130 + 4 * k as u32);
                (ptr.wrapping_sub(pre_line(k as u32)) / 4) as usize
            }),
            temp_disable_fx: m.mem1[((at + 0x13C) & 0x01FF_FFFF) as usize] != 0,
        }
    }

    /// `AXFXReverbStdCallback`: the three buses, in place.
    #[doc(alias = "AXFXReverbStdCallback")]
    pub fn process(&mut self, bufs: &mut [i32; 3 * SAMPLES]) {
        if !self.temp_disable_fx {
            self.handle_reverb(bufs);
        }
    }

    /// `HandleReverb`: each bus through the pre-delay, two comb filters, two all-pass
    /// filters and a low-pass, mixed with the dry signal.
    #[doc(alias = "HandleReverb")]
    fn handle_reverb(&mut self, bufs: &mut [i32; 3 * SAMPLES]) {
        // The int-to-float conversion's magic, 2^52 + 2^31.
        const MAGIC: f64 = 4_503_601_774_854_144.0;
        let to_float = |x: i32| {
            let bits = 0x4330_0000_0000_0000 | u64::from(x as u32 ^ 0x8000_0000);
            fp::fsubs(f64::from_bits(bits), MAGIC)
        };
        let c03 = fp::lfs(0.3f32.to_bits());
        let c06 = fp::lfs(0.6f32.to_bits());
        let f2 = self.all_pass_coeff;
        let f11 = self.damping;
        let f3 = fp::fmuls(self.level, c06);
        let f4 = fp::fsubs(c06, f3);
        for k in 0..3 {
            let (f19, f20) = (self.comb_coef[2 * k], self.comb_coef[2 * k + 1]);
            let mut f7 = self.lp_lastout[k];
            let mut ptr = self.pre_delay_ptr[k];
            let pre_end = self.pre_delay_time.wrapping_sub(1);
            let [c0, c1] = &mut self.c[2 * k..2 * k + 2] else {
                unreachable!()
            };
            let [ap0, ap1] = &mut self.ap[2 * k..2 * k + 2] else {
                unreachable!()
            };
            let (mut f15, mut f16) = (c0.last_output, c1.last_output);
            let (mut f17, mut f18) = (ap0.last_output, ap1.last_output);
            let bus = &mut bufs[k * SAMPLES..(k + 1) * SAMPLES];
            let mut f12 = to_float(bus[0]);
            for i in 0..SAMPLES {
                let mut f13 = f12;
                if self.pre_delay_time != 0 {
                    let line = &mut self.pre_delay_line[k];
                    f13 = fp::lfs(line[ptr]);
                    line[ptr] = fp::stfs(f12);
                    ptr += 1;
                    if ptr == pre_end {
                        ptr = 0;
                    }
                }
                let f8 = fp::fmadds(f19, f15, f13);
                let f9 = fp::fmadds(f20, f16, f13);
                c0.store(f8);
                c1.store(f9);
                let mut f14 = c0.load();
                f16 = c1.load();
                f15 = f14;
                f14 = fp::fadds(f14, f16);
                let f9 = fp::fmadds(f2, f17, f14);
                ap0.store(f9);
                f14 = fp::fnmsubs(f2, f9, f17);
                f17 = ap0.load();
                f14 = fp::fmuls(f14, c03);
                f14 = fp::fmadds(f11, f7, f14);
                let f9 = fp::fmadds(f2, f18, f14);
                f7 = f14;
                ap1.store(f9);
                f14 = fp::fnmsubs(f2, f9, f18);
                let f8 = fp::fmuls(f4, f12);
                f18 = ap1.load();
                f14 = fp::fmadds(f3, f14, f8);
                bus[i] = fp::fctiwz(f14);
                if i + 1 < SAMPLES {
                    f12 = to_float(bus[i + 1]);
                }
            }
            c0.last_output = f15;
            c1.last_output = f16;
            ap0.last_output = f17;
            ap1.last_output = f18;
            self.lp_lastout[k] = f7;
            self.pre_delay_ptr[k] = ptr;
        }
    }
}

/// `AXFX_DELAY`.
#[derive(Clone, Debug)]
pub struct Delay {
    size: [u32; 3],
    pos: [u32; 3],
    feedback: [u32; 3],
    output: [u32; 3],
    lines: [Vec<i32>; 3],
}

impl Delay {
    /// The delay at `at` (an `AXFX_DELAY`) in the boot state's memory.
    pub fn from_memory(m: &BootMemory, at: u32) -> Delay {
        let w = |i: u32| m.u32(at + 4 * i);
        let size = [w(0), w(1), w(2)];
        Delay {
            size,
            pos: [w(3), w(4), w(5)],
            feedback: [w(6), w(7), w(8)],
            output: [w(9), w(10), w(11)],
            lines: std::array::from_fn(|k| {
                m.words(w(12 + k as u32), size[k] as usize * SAMPLES)
                    .into_iter()
                    .map(|v| v as i32)
                    .collect()
            }),
        }
    }

    /// `AXFXDelayCallback`: each bus delayed by its line, with feedback.
    #[doc(alias = "AXFXDelayCallback")]
    pub fn process(&mut self, bufs: &mut [i32; 3 * SAMPLES]) {
        for k in 0..3 {
            let at = self.pos[k] as usize * SAMPLES;
            let line = &mut self.lines[k][at..at + SAMPLES];
            let bus = &mut bufs[k * SAMPLES..(k + 1) * SAMPLES];
            for (d, b) in line.iter_mut().zip(bus.iter_mut()) {
                let v = *d as u32;
                *d = b.wrapping_add((v.wrapping_mul(self.feedback[k]) as i32) >> 7);
                *b = (v.wrapping_mul(self.output[k]) as i32) >> 7;
            }
            self.pos[k] = (self.pos[k] + 1) % self.size[k];
        }
    }
}

/// An aux bus's effect.
#[derive(Clone, Debug)]
pub enum Effect {
    ReverbStd(Box<ReverbStd>),
    Delay(Box<Delay>),
    /// A callback the mixer doesn't have (`AXFXReverbHiCallback`, `AXFXChorusCallback`), by
    /// address: its bus comes back unprocessed.
    Unknown(u32),
}

impl Effect {
    /// The effect `callback` (with `context`) is, from the boot state's memory.
    pub fn from_memory(m: &BootMemory, callback: u32, context: u32) -> Effect {
        if callback == (m.sym)("AXFXReverbStdCallback") {
            Effect::ReverbStd(Box::new(ReverbStd::from_memory(m, context)))
        } else if callback == (m.sym)("AXFXDelayCallback") {
            Effect::Delay(Box::new(Delay::from_memory(m, context)))
        } else {
            Effect::Unknown(callback)
        }
    }

    pub fn process(&mut self, bufs: &mut [i32; 3 * SAMPLES]) {
        match self {
            Effect::ReverbStd(r) => r.process(bufs),
            Effect::Delay(d) => d.process(bufs),
            Effect::Unknown(_) => {}
        }
    }
}
