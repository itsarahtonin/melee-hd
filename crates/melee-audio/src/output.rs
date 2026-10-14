//! Playing the mixed samples on the default output device. The game loop pushes each tick's
//! 32 kHz samples into a lock-free ring and never waits; the device's callback resamples them to
//! its rate, nudging the rate a little to keep the ring as short as it can without running dry.
//!
//! **Decision: the queue is as short as stays glitch-free.** A tick pushes about 16.7 ms of
//! samples at once, so the ring rises and falls by that much between ticks; what sets the
//! latency of a tick's first sample is how full the ring is at its lowest, just before a push.
//! The callback keeps that low point what one callback takes plus a small margin (2 ms), so a
//! tick that comes a callback later still finds samples, measured over each quarter second;
//! running dry anyway (an underrun, counted in [`OutputStats`]) adds a millisecond to the
//! margin, unless it was a stall that no margin would cover, and a minute without takes one
//! back. The device's own buffer is as small as the
//! backend allows: on Windows the player talks to WASAPI directly ([`wasapi`]), asking for the
//! audio engine's smallest period (`IAudioClient3`, often 2 to 3 ms where the driver allows it,
//! else the standard 10 ms), or with [`OutputOptions::exclusive`] the device itself in exclusive
//! mode; elsewhere, and if that fails, it goes through cpal with buffers of about 5 ms.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

#[cfg(windows)]
mod wasapi;

/// The mixer's rate.
pub const RATE: u32 = 32_000;
/// The ring's size in stereo samples (about a second).
const CAPACITY: usize = 1 << 15;
/// About what a tick pushes at once.
const TICK: usize = (RATE / 60) as usize;
/// The slack the ring keeps at its lowest to start with, and the most the underruns can raise
/// it to.
const MARGIN: usize = RATE as usize / 500;
const MARGIN_STEP: usize = RATE as usize / 1000;
const MARGIN_MAX: usize = RATE as usize / 50;
/// Past this much queued beyond what it takes to start playing, the backlog is dropped: the
/// catch-up after a stall (the game runs up to four late ticks at once), which the rate's
/// small nudge would take many seconds to play off. Playing, the ring stays a tick below it.
const TOO_LONG: usize = 2 * TICK;
/// The most the rate is nudged off nominal, either way.
const MAX_NUDGE: f64 = 0.005;

/// How to open the output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputOptions {
    /// Windows: take the device in WASAPI's exclusive mode, the lowest latency the device has,
    /// at the cost of every other program's sound while the match plays. Elsewhere, and if the
    /// device refuses, the output is shared.
    pub exclusive: bool,
}

/// What the output measures as it plays, for the latency log.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OutputStats {
    /// The samples queued in the ring when the last tick's samples were pushed: how long that
    /// tick's first sample waits before the device's callback takes it.
    pub ring: Duration,
    /// How long the device's last callback's first sample waits before it's heard: what the
    /// device and the system's mixer hold (the backend's measure).
    pub device: Duration,
    /// The device's last callback, in device samples.
    pub callback_frames: u32,
    /// Callbacks that ran out of samples while playing (each one a gap), and backlogs dropped.
    pub underruns: u32,
    pub skips: u32,
}

impl OutputStats {
    /// The whole queue a tick's first sample waits through: the ring and the device.
    pub fn total(&self) -> Duration {
        self.ring + self.device
    }
}

/// The measures, kept as atomics the callback and the game's thread share.
#[derive(Default)]
struct Meters {
    ring_at_push: AtomicU32,
    device_us: AtomicU32,
    callback_frames: AtomicU32,
    underruns: AtomicU32,
    skips: AtomicU32,
}

impl Meters {
    fn device(&self, d: Duration) {
        let us = u32::try_from(d.as_micros()).unwrap_or(u32::MAX);
        self.device_us.store(us, Ordering::Relaxed);
    }
}

/// A single-producer, single-consumer ring of stereo samples packed in words.
struct Ring {
    buf: Box<[AtomicU32]>,
    /// Samples written and read since the start.
    head: AtomicUsize,
    tail: AtomicUsize,
    meters: Meters,
}

impl Ring {
    fn new() -> Ring {
        Ring {
            buf: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            meters: Meters::default(),
        }
    }

    /// Appends what fits; the rest is dropped.
    fn push(&self, samples: &[[i16; 2]]) {
        let w = self.head.load(Ordering::Relaxed);
        let r = self.tail.load(Ordering::Acquire);
        self.meters
            .ring_at_push
            .store((w - r) as u32, Ordering::Relaxed);
        let n = samples.len().min(CAPACITY - (w - r));
        for (i, s) in samples[..n].iter().enumerate() {
            let word = u32::from(s[0] as u16) | (u32::from(s[1] as u16) << 16);
            self.buf[(w + i) % CAPACITY].store(word, Ordering::Relaxed);
        }
        self.head.store(w + n, Ordering::Release);
    }

    fn stats(&self) -> OutputStats {
        let m = &self.meters;
        OutputStats {
            ring: Duration::from_secs_f64(
                f64::from(m.ring_at_push.load(Ordering::Relaxed)) / f64::from(RATE),
            ),
            device: Duration::from_micros(m.device_us.load(Ordering::Relaxed).into()),
            callback_frames: m.callback_frames.load(Ordering::Relaxed),
            underruns: m.underruns.load(Ordering::Relaxed),
            skips: m.skips.load(Ordering::Relaxed),
        }
    }

    fn get(&self, at: usize) -> [f32; 2] {
        let word = self.buf[at % CAPACITY].load(Ordering::Relaxed);
        let s = |v: u32| f32::from(v as u16 as i16) / 32768.0;
        [s(word), s(word >> 16)]
    }
}

/// The callback's resampler, and the control that keeps the ring short.
struct Resampler {
    ring: Arc<Ring>,
    /// Source samples per device sample at the nominal rates.
    step: f64,
    /// Where between the next two source samples the next output falls.
    frac: f64,
    /// Whether the ring has filled enough to play since it last ran dry.
    primed: bool,
    /// The slack to keep at the ring's lowest, in source samples, and the device samples since
    /// it last changed (it comes back down a step after a minute without running dry).
    margin: usize,
    calm: usize,
    /// Device samples played silent since the last underrun, if one raised the margin: a
    /// dry spell of more than two ticks was a stall (a window being dragged, the first frame's
    /// pipelines), which no margin covers, so the margin goes back down.
    dry: Option<usize>,
    /// The least slack a callback found, over the current window, and what the last callback
    /// took.
    low: usize,
    need: usize,
    /// Device samples left in the window, and in each window (a quarter second).
    window_left: usize,
    window: usize,
    /// How far off nominal the rate is nudged.
    nudge: f64,
}

impl Resampler {
    fn new(ring: Arc<Ring>, device_rate: u32) -> Resampler {
        let window = device_rate as usize / 4;
        Resampler {
            ring,
            step: f64::from(RATE) / f64::from(device_rate),
            frac: 0.0,
            primed: false,
            margin: MARGIN,
            calm: 0,
            dry: None,
            low: usize::MAX,
            need: 0,
            window_left: window,
            window,
            nudge: 0.0,
        }
    }

    /// Makes `frames` device samples, handing each to `put`.
    fn fill(&mut self, frames: usize, mut put: impl FnMut([f32; 2])) {
        let ring = Arc::clone(&self.ring);
        let meters = &ring.meters;
        meters
            .callback_frames
            .store(frames as u32, Ordering::Relaxed);
        let w = ring.head.load(Ordering::Acquire);
        let mut r = ring.tail.load(Ordering::Relaxed);
        let step = self.step * (1.0 + self.nudge);
        // What this callback takes, and what the ring needs to start playing: that, the
        // margin, and a tick's worth, so it lasts until the next tick's samples.
        let need = (frames as f64 * step).ceil() as usize + 1;
        self.need = need;
        let start = need + self.margin + TICK;
        if w - r > start + TOO_LONG {
            r = w - start;
            meters.skips.fetch_add(1, Ordering::Relaxed);
        }
        if !self.primed && w - r >= start {
            // Two ticks' worth of device samples (a window is a quarter second).
            if self
                .dry
                .take()
                .is_some_and(|d| d > self.window * 4 * 2 / 60)
            {
                self.margin = (self.margin - MARGIN_STEP).max(MARGIN);
            }
            self.primed = true;
            self.low = usize::MAX;
            self.window_left = self.window;
        }
        if let (false, Some(d)) = (self.primed, &mut self.dry) {
            *d += frames;
        }
        let playing = self.primed;
        // How much more than this callback takes the ring holds: the next callback runs dry
        // unless that's at least what it takes, or a tick's samples come first.
        let slack = (w - r).saturating_sub(need);
        for _ in 0..frames {
            self.primed &= w - r >= 2;
            let s = if self.primed {
                let (a, b) = (ring.get(r), ring.get(r + 1));
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
            put(s);
        }
        if playing && !self.primed {
            meters.underruns.fetch_add(1, Ordering::Relaxed);
            self.margin = (self.margin + MARGIN_STEP).min(MARGIN_MAX);
            self.calm = 0;
            self.dry = Some(0);
        }
        if self.primed {
            self.calm += frames;
            if self.calm >= 60 * 4 * self.window {
                self.margin = (self.margin - MARGIN_STEP).max(MARGIN);
                self.calm = 0;
            }
            self.low = self.low.min(slack);
            self.window_left = self.window_left.saturating_sub(frames);
            if self.window_left == 0 {
                self.adjust();
            }
        }
        ring.tail.store(r, Ordering::Release);
    }

    /// At each window's end: nudges the rate so the least slack comes to what a callback
    /// takes and the margin (so the ring lasts a callback more when a tick's samples come a
    /// callback later), closing the difference over about half a second. The game's ticks
    /// bring a little more than 32 kHz (the console's 59.94 frames a second come 60 times),
    /// so the slack settles a fraction of a millisecond above that, on the safe side.
    fn adjust(&mut self) {
        self.window_left = self.window;
        let target = self.need + self.margin;
        let err = (self.low as f64 - target as f64) / f64::from(RATE);
        self.low = usize::MAX;
        self.nudge = (2.0 * err).clamp(-MAX_NUDGE, MAX_NUDGE);
    }
}

/// What plays the samples.
enum Backend {
    Cpal(#[allow(dead_code)] cpal::Stream),
    #[cfg(windows)]
    Wasapi(#[allow(dead_code)] wasapi::Stream),
}

/// The audio output: push each tick's samples.
pub struct Output {
    ring: Arc<Ring>,
    _backend: Backend,
    /// The device's rate, its callback's size in samples where fixed, and what plays them,
    /// for the log.
    pub rate: u32,
    pub buffer: Option<u32>,
    pub describe: String,
}

impl Output {
    /// Opens the default output device, shared.
    pub fn open() -> Result<Output, String> {
        Output::open_with(OutputOptions::default())
    }

    /// Opens the default output device.
    pub fn open_with(options: OutputOptions) -> Result<Output, String> {
        let ring = Arc::new(Ring::new());
        #[cfg(windows)]
        match wasapi::Stream::open(&ring, options.exclusive) {
            Ok((stream, info)) => {
                return Ok(Output {
                    ring,
                    _backend: Backend::Wasapi(stream),
                    rate: info.rate,
                    buffer: Some(info.period),
                    describe: info.describe,
                });
            }
            Err(e) => eprintln!("sound: WASAPI: {e}; trying cpal"),
        }
        #[cfg(not(windows))]
        if options.exclusive {
            eprintln!("sound: exclusive mode is Windows only; sharing the device");
        }
        let (stream, rate, buffer) = open_cpal(&ring)?;
        Ok(Output {
            ring,
            _backend: Backend::Cpal(stream),
            rate,
            buffer,
            describe: format!(
                "cpal, shared, {}",
                buffer.map_or("the device's buffer".into(), |n| format!(
                    "{n}-sample buffers"
                ))
            ),
        })
    }

    /// Queues 32 kHz stereo samples; never waits (what doesn't fit is dropped).
    pub fn push(&self, samples: &[[i16; 2]]) {
        self.ring.push(samples);
    }

    /// What the output has measured.
    pub fn stats(&self) -> OutputStats {
        self.ring.stats()
    }
}

/// The default device through cpal, shared, with buffers of about 5 ms where it allows that.
fn open_cpal(ring: &Arc<Ring>) -> Result<(cpal::Stream, u32, Option<u32>), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no audio output device")?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("the audio device's configuration: {e}"))?;
    let mut config = supported.config();
    let want = config.sample_rate / 200;
    let buffer = match supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => Some(want.clamp(*min, *max)),
        cpal::SupportedBufferSize::Unknown => None,
    };
    if let Some(n) = buffer {
        config.buffer_size = cpal::BufferSize::Fixed(n);
    }
    let format = supported.sample_format();
    let build = |config: &cpal::StreamConfig| match format {
        cpal::SampleFormat::I16 => stream::<i16>(&device, config, ring),
        cpal::SampleFormat::U16 => stream::<u16>(&device, config, ring),
        cpal::SampleFormat::I32 => stream::<i32>(&device, config, ring),
        cpal::SampleFormat::F64 => stream::<f64>(&device, config, ring),
        _ => stream::<f32>(&device, config, ring),
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
    Ok((stream, config.sample_rate, buffer))
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: &Arc<Ring>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let mut r = Resampler::new(ring.clone(), config.sample_rate);
    device
        .build_output_stream(
            *config,
            move |out: &mut [T], info: &cpal::OutputCallbackInfo| {
                let t = info.timestamp();
                r.ring.meters.device(t.playback.duration_since(t.callback));
                let n = out.len() / channels.max(1);
                let mut frames = out.chunks_mut(channels.max(1));
                r.fill(n, |s| {
                    if let Some(frame) = frames.next() {
                        put_frame(frame, s);
                    }
                });
            },
            |e| eprintln!("audio output: {e}"),
            None,
        )
        .map_err(|e| format!("opening the audio stream: {e}"))
}

/// Writes a stereo sample to a device frame of any channel count: both channels averaged for
/// mono, left and right to the first two, silence on the rest.
fn put_frame<T: cpal::Sample + FromSample<f32>>(frame: &mut [T], s: [f32; 2]) {
    let channels = frame.len();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, from: usize) -> Vec<[i16; 2]> {
        (from..from + n)
            .map(|i| {
                let v = (8000.0 * (i as f64 * 0.05).sin()) as i16;
                [v, -v]
            })
            .collect()
    }

    /// A tone through the ring and the resampler at 48 kHz: silence until the ring holds what
    /// a callback needs, the margin and a tick, then the tone at the device's rate, with no
    /// gaps while the ring has samples.
    #[test]
    fn resamples_after_priming() {
        let ring = Arc::new(Ring::new());
        let mut r = Resampler::new(ring.clone(), 48_000);
        let src = tone(2 * TICK, 0);
        ring.push(&src[..TICK / 2]);
        let mut out = Vec::new();
        r.fill(480, |s| out.push(s));
        assert!(
            out.iter().all(|s| *s == [0.0; 2]),
            "plays before it can last"
        );
        ring.push(&src[TICK / 2..]);
        out.clear();
        r.fill(480, |s| out.push(s));
        assert!(r.primed);
        // Left and right stay opposite, and consecutive outputs stay close (no jumps).
        for s in &out {
            assert!((s[0] + s[1]).abs() < 1e-4);
        }
        for w in out.windows(2) {
            assert!((w[0][0] - w[1][0]).abs() < 0.02);
        }
        let read = ring.tail.load(Ordering::Relaxed);
        assert!(
            (310..=330).contains(&read),
            "{read} source samples for 480 at 48 kHz"
        );
    }

    /// Plays `seconds` of the game's pushes (a tick's samples 60 times a second, a little more
    /// than 32 kHz, as the console's 59.94 Hz frames come at 60, each up to `jitter` µs late)
    /// against callbacks of `period` samples at 48 kHz (each up to a fifth of the jitter late):
    /// the stats, and the mean ring at a push over the last half.
    fn simulate(period: usize, jitter: u64, seconds: u64) -> (OutputStats, f64) {
        let ring = Arc::new(Ring::new());
        let mut r = Resampler::new(ring.clone(), 48_000);
        let per_tick = 32_000.0 / 59.94;
        let mut pushed = 0.0f64;
        let mut rng = 0x1234_5678u32;
        let mut late = |max: u64| {
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            u64::from(rng >> 8) % (max + 1)
        };
        // In microseconds: when the next tick and callback are due.
        let call_period = period as f64 * 1e6 / 48_000.0;
        let (mut tick, mut call) = (0u64, 0u64);
        let (mut tick_at, mut call_at) = (0u64, 0u64);
        let (mut sum, mut n) = (0.0, 0);
        while tick_at < seconds * 1_000_000 {
            if tick_at <= call_at {
                let k = ((pushed + per_tick) as usize) - pushed as usize;
                ring.push(&tone(k, pushed as usize));
                pushed += per_tick;
                if tick_at > seconds * 500_000 {
                    sum += ring.stats().ring.as_secs_f64();
                    n += 1;
                }
                tick += 1;
                tick_at = tick * 16_667 + late(jitter);
            } else {
                r.fill(period, |_| {});
                call += 1;
                call_at = (call as f64 * call_period) as u64 + late(jitter / 5);
            }
        }
        (ring.stats(), sum / f64::from(n) * 1000.0)
    }

    /// With the engine's small periods and with the standard 10 ms, the ring settles a few
    /// milliseconds deep at a push and never runs dry, through a millisecond and a half of
    /// timing jitter.
    #[test]
    fn keeps_the_ring_short_without_running_dry() {
        for (period, most) in [(128, 8.0), (480, 16.0)] {
            let (stats, ring_ms) = simulate(period, 1500, 30);
            assert_eq!(stats.underruns, 0, "{period}: ran dry");
            assert_eq!(stats.skips, 0, "{period}");
            assert!(
                ring_ms < most,
                "{period}-sample callbacks: {ring_ms:.2} ms queued at a push"
            );
        }
    }

    /// A stall (no ticks for 150 ms, then the late ticks at once) runs the ring dry, but
    /// leaves the margin as it was, and the backlog is dropped.
    #[test]
    fn a_stall_leaves_the_margin() {
        let ring = Arc::new(Ring::new());
        let mut r = Resampler::new(ring.clone(), 48_000);
        let mut pushed = 0;
        let mut push = |ticks: usize| {
            for _ in 0..ticks {
                ring.push(&tone(TICK, pushed));
                pushed += TICK;
            }
        };
        // A second of ticks against 128-sample callbacks (six or seven a tick), a stall of
        // nine ticks, the late ones at once, and another second.
        for t in 0..60 {
            push(1);
            for _ in 0..if t % 4 == 0 { 7 } else { 6 } {
                r.fill(128, |_| {});
            }
        }
        assert_eq!(ring.stats().underruns, 0, "before the stall");
        for _ in 0..56 {
            r.fill(128, |_| {});
        }
        push(8);
        for t in 0..60 {
            push(1);
            for _ in 0..if t % 4 == 0 { 7 } else { 6 } {
                r.fill(128, |_| {});
            }
        }
        let s = ring.stats();
        assert_eq!(s.underruns, 1);
        assert_eq!(s.skips, 1);
        assert_eq!(r.margin, MARGIN);
    }

    /// Ticks up to 6 ms late: the ring settles deeper, as deep as their lateness needs.
    #[test]
    fn absorbs_late_ticks() {
        let (stats, ring_ms) = simulate(128, 6000, 30);
        assert!(stats.underruns <= 1, "{} underruns", stats.underruns);
        assert!(ring_ms < 12.0, "{ring_ms:.2} ms queued at a push");
    }
}
