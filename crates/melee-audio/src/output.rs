//! Playing the mixed samples on the default output device (cpal, shared mode): the game loop
//! pushes each tick's 32 kHz samples into a lock-free ring and never waits; the device's
//! callback resamples them to its rate, nudging its rate a little to hold the ring near a
//! short target (about 40 ms), skips ahead if the ring runs long, and after running dry waits
//! for the target again before it plays on.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

/// The mixer's rate.
pub const RATE: u32 = 32_000;
/// The ring's size in stereo samples (about a second).
const CAPACITY: usize = 1 << 15;
/// The fill the callback aims for, and past which it drops the backlog.
const TARGET: usize = RATE as usize / 25;
const TOO_LONG: usize = 4 * TARGET;

/// A single-producer, single-consumer ring of stereo samples packed in words.
struct Ring {
    buf: Box<[AtomicU32]>,
    /// Samples written and read since the start.
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl Ring {
    fn new() -> Ring {
        Ring {
            buf: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Appends what fits; the rest is dropped.
    fn push(&self, samples: &[[i16; 2]]) {
        let w = self.head.load(Ordering::Relaxed);
        let r = self.tail.load(Ordering::Acquire);
        let n = samples.len().min(CAPACITY - (w - r));
        for (i, s) in samples[..n].iter().enumerate() {
            let word = u32::from(s[0] as u16) | (u32::from(s[1] as u16) << 16);
            self.buf[(w + i) % CAPACITY].store(word, Ordering::Relaxed);
        }
        self.head.store(w + n, Ordering::Release);
    }

    fn get(&self, at: usize) -> [f32; 2] {
        let word = self.buf[at % CAPACITY].load(Ordering::Relaxed);
        let s = |v: u32| f32::from(v as u16 as i16) / 32768.0;
        [s(word), s(word >> 16)]
    }
}

/// The callback's resampler.
struct Resampler {
    ring: Arc<Ring>,
    /// Source samples per device sample at the nominal rates.
    step: f64,
    /// Where between the next two source samples the next output falls.
    frac: f64,
    /// Whether the ring has filled to the target since it last ran dry.
    primed: bool,
}

impl Resampler {
    fn fill<T: SizedSample + FromSample<f32>>(&mut self, out: &mut [T], channels: usize) {
        let w = self.ring.head.load(Ordering::Acquire);
        let mut r = self.ring.tail.load(Ordering::Relaxed);
        if w - r > TOO_LONG {
            r = w - TARGET;
        }
        self.primed |= w - r >= TARGET;
        // A rate a little off nominal, toward the target fill.
        let off = ((w - r) as f64 - TARGET as f64) / TARGET as f64;
        let step = self.step * (1.0 + 0.005 * off.clamp(-1.0, 1.0));
        for frame in out.chunks_mut(channels) {
            self.primed &= w - r >= 2;
            let s = if self.primed {
                let (a, b) = (self.ring.get(r), self.ring.get(r + 1));
                let t = self.frac as f32;
                self.frac += step;
                while self.frac >= 1.0 && w - r >= 2 {
                    self.frac -= 1.0;
                    r += 1;
                }
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            } else {
                [0.0; 2]
            };
            for (c, o) in frame.iter_mut().enumerate() {
                let v = match (channels, c) {
                    (1, _) => 0.5 * (s[0] + s[1]),
                    (_, 0) => s[0],
                    (_, 1) => s[1],
                    _ => 0.0,
                };
                *o = T::from_sample(v);
            }
        }
        self.ring.tail.store(r, Ordering::Release);
    }
}

/// The audio output: push each tick's samples.
pub struct Output {
    ring: Arc<Ring>,
    _stream: cpal::Stream,
    /// The device's rate and the callback's buffer, for the log.
    pub rate: u32,
    pub buffer: Option<u32>,
}

impl Output {
    /// Opens the default output device.
    pub fn open() -> Result<Output, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("the audio device's configuration: {e}"))?;
        let ring = Arc::new(Ring::new());
        let mut config = supported.config();
        // About 10 ms a callback where the device allows it.
        let want = config.sample_rate / 100;
        let buffer = match supported.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => Some(want.clamp(*min, *max)),
            cpal::SupportedBufferSize::Unknown => None,
        };
        if let Some(n) = buffer {
            config.buffer_size = cpal::BufferSize::Fixed(n);
        }
        let format = supported.sample_format();
        let build = |config: &cpal::StreamConfig| match format {
            cpal::SampleFormat::I16 => stream::<i16>(&device, config, &ring),
            cpal::SampleFormat::U16 => stream::<u16>(&device, config, &ring),
            cpal::SampleFormat::I32 => stream::<i32>(&device, config, &ring),
            cpal::SampleFormat::F64 => stream::<f64>(&device, config, &ring),
            _ => stream::<f32>(&device, config, &ring),
        };
        let (stream, buffer) = match build(&config) {
            Ok(s) => (s, buffer),
            Err(_) => {
                config.buffer_size = cpal::BufferSize::Default;
                (build(&config)?, None)
            }
        };
        stream
            .play()
            .map_err(|e| format!("starting the audio stream: {e}"))?;
        Ok(Output {
            ring,
            _stream: stream,
            rate: config.sample_rate,
            buffer,
        })
    }

    /// Queues 32 kHz stereo samples; never waits (what doesn't fit is dropped).
    pub fn push(&self, samples: &[[i16; 2]]) {
        self.ring.push(samples);
    }
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: &Arc<Ring>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let mut r = Resampler {
        ring: ring.clone(),
        step: f64::from(RATE) / f64::from(config.sample_rate),
        frac: 0.0,
        primed: false,
    };
    device
        .build_output_stream(
            *config,
            move |out: &mut [T], _: &cpal::OutputCallbackInfo| r.fill(out, channels),
            |e| eprintln!("audio output: {e}"),
            None,
        )
        .map_err(|e| format!("opening the audio stream: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tone through the ring and the resampler at 48 kHz: silence until the ring reaches its
    /// target, then the tone at the device's rate, with no gaps while the ring has samples.
    #[test]
    fn resamples_after_priming() {
        let ring = Arc::new(Ring::new());
        let mut r = Resampler {
            ring: ring.clone(),
            step: f64::from(RATE) / 48_000.0,
            frac: 0.0,
            primed: false,
        };
        let tone: Vec<[i16; 2]> = (0..TARGET + 3200)
            .map(|i| {
                let v = (8000.0 * (i as f64 * 0.05).sin()) as i16;
                [v, -v]
            })
            .collect();
        ring.push(&tone[..TARGET / 2]);
        let mut out = vec![0f32; 2 * 480];
        r.fill(&mut out, 2);
        assert!(out.iter().all(|&x| x == 0.0), "plays before the target");
        ring.push(&tone[TARGET / 2..]);
        r.fill(&mut out, 2);
        assert!(r.primed);
        // Left and right stay opposite, and consecutive outputs stay close (no jumps).
        for f in out.chunks(2) {
            assert!((f[0] + f[1]).abs() < 1e-4);
        }
        for w in out.chunks(2).collect::<Vec<_>>().windows(2) {
            assert!((w[0][0] - w[1][0]).abs() < 0.02);
        }
        let read = ring.tail.load(Ordering::Relaxed);
        assert!(
            (310..=330).contains(&read),
            "{read} source samples for 480 at 48 kHz"
        );
    }
}
