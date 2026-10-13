//! `melee-audio-check TRACE REPLAY.slp AUDIO.wav [--fp hardware|slippi] [--ticks N]
//! [--window SECONDS] [--out MIX.wav]`: plays a replay natively from the boot state in
//! ssbm-rs's state trace (version 5, with memory over the boot tick), mixes its audio with
//! melee-audio, and compares the samples with ssbm-rs's recording of the same run's audio
//! interface (`AUDIO_OUT=AUDIO.wav ssbm-run ...`), which starts at power-on: the two streams
//! are aligned first (an exact match of a stretch of samples, else cross-correlation), then
//! compared window by window: SNR, the largest difference and the share of stereo samples
//! that are equal.
//!
//! With `--port` the trace isn't read (pass `-` for it): the match starts as the player
//! starts one, from melee-sim's port of the boot's audio setup (`game::boot`), and the mix
//! covers the boot's frames too.
//!
//! The disc comes from `SSBM_DISC`. `--out` writes the native mix as a WAV file.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_audio::Mixer;
use melee_data::disc::Disc;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_slippi::Replay;

mod boot;

/// The mixer's rate.
const RATE: usize = 32_000;

type Sample = [i16; 2];

/// A 16-bit stereo WAV file's samples.
fn read_wav(path: &str) -> Result<Vec<Sample>, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(format!("{path}: not a WAV file"));
    }
    let mut at = 12;
    let mut format = None;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let len = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &b[at + 8..(at + 8 + len).min(b.len())];
        match id {
            b"fmt " => {
                let h = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
                let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                format = Some((h(0), h(2), rate, h(14)));
            }
            b"data" => {
                if format != Some((1, 2, RATE as u32, 16)) {
                    return Err(format!(
                        "{path}: expected 16-bit stereo PCM at 32 kHz, found {format:?}"
                    ));
                }
                return Ok(body
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| {
                        [
                            i16::from_le_bytes([c[0], c[1]]),
                            i16::from_le_bytes([c[2], c[3]]),
                        ]
                    })
                    .collect());
            }
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    Err(format!("{path}: no samples"))
}

fn write_wav(path: &str, s: &[Sample]) -> std::io::Result<()> {
    let data = (s.len() * 4) as u32;
    let mut b = Vec::with_capacity(44 + s.len() * 4);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&(RATE as u32).to_le_bytes());
    b.extend_from_slice(&(RATE as u32 * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for x in s {
        b.extend_from_slice(&x[0].to_le_bytes());
        b.extend_from_slice(&x[1].to_le_bytes());
    }
    std::fs::write(path, b)
}

/// Where `ours[0]` falls in `theirs`: the first stretch of ours that isn't silent, found
/// exactly if it can be, else by the best normalized cross-correlation of its left channel.
fn align(ours: &[Sample], theirs: &[Sample]) -> Option<(usize, &'static str)> {
    const N: usize = 1024;
    let start = ours.iter().position(|s| s[0].unsigned_abs() > 64)?;
    let w = ours.get(start..start + N)?;
    let last = theirs.len().checked_sub(N)?;
    if let Some(o) = (0..=last).find(|&o| theirs[o..o + N] == *w) {
        return o.checked_sub(start).map(|o| (o, "exactly"));
    }
    let mut best = (f64::MIN, 0usize);
    for o in 0..=last {
        let t = &theirs[o..o + N];
        let (mut dot, mut energy) = (0i64, 0i64);
        for (a, b) in w.iter().zip(t) {
            dot += i64::from(a[0]) * i64::from(b[0]);
            energy += i64::from(b[0]) * i64::from(b[0]);
        }
        let score = dot as f64 / (energy as f64).sqrt().max(1.0);
        if score > best.0 {
            best = (score, o);
        }
    }
    best.1
        .checked_sub(start)
        .map(|o| (o, "by cross-correlation"))
}

/// A stretch's comparison.
#[derive(Default)]
struct Window {
    signal: f64,
    noise: f64,
    max: u32,
    equal: usize,
    count: usize,
}

impl Window {
    fn add(&mut self, a: Sample, b: Sample) {
        for c in 0..2 {
            let d = i32::from(a[c]) - i32::from(b[c]);
            self.signal += f64::from(b[c]).powi(2);
            self.noise += f64::from(d).powi(2);
            self.max = self.max.max(d.unsigned_abs());
        }
        self.equal += usize::from(a == b);
        self.count += 1;
    }

    fn snr(&self) -> String {
        if self.noise == 0.0 {
            "exact".into()
        } else {
            format!("{:.1} dB", 10.0 * (self.signal / self.noise).log10())
        }
    }

    fn share(&self) -> f64 {
        100.0 * self.equal as f64 / self.count.max(1) as f64
    }
}

fn run(args: &[String]) -> Result<bool, String> {
    let mut pos = Vec::new();
    let (mut ticks, mut window, mut out, mut port) = (2000u32, 1.0f64, None, false);
    let mut fp_mode = FpMode::Slippi;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--ticks" => ticks = value()?.parse().map_err(|_| "--ticks N")?,
            "--window" => window = value()?.parse().map_err(|_| "--window SECONDS")?,
            "--out" => out = Some(value()?.clone()),
            "--port" => port = true,
            "--fp" => {
                fp_mode = match value()?.as_str() {
                    "hardware" => FpMode::Hardware,
                    "slippi" => FpMode::Slippi,
                    _ => return Err("--fp hardware|slippi".into()),
                }
            }
            _ => pos.push(a.as_str()),
        }
    }
    let [trace_path, replay_path, wav_path] = pos[..] else {
        return Err(
            "usage: melee-audio-check TRACE|- REPLAY.slp AUDIO.wav [--port] [--fp hardware|slippi] [--ticks N] [--window SECONDS] [--out MIX.wav]"
                .into(),
        );
    };
    set_fp_mode(fp_mode);
    let disc_path = std::env::var("SSBM_DISC").map_err(|_| "SSBM_DISC names the disc")?;
    let disc = Disc::open(&disc_path).map_err(|e| format!("{disc_path}: {e}"))?;
    let bytes = std::fs::read(replay_path).map_err(|e| format!("{replay_path}: {e}"))?;
    let replay = Replay::parse(&bytes).map_err(|e| format!("{replay_path}: {e}"))?;
    let mut settings = Settings::from_replay(&melee_slippi::start(&replay))
        .map_err(|e| format!("{replay_path}: {e}"))?;
    let theirs = read_wav(wav_path)?;
    let boot = if port {
        println!("the boot's audio setup: melee-sim's");
        None
    } else {
        let boot = boot::Boot::from_trace(trace_path)?;
        boot.apply(&mut settings);
        println!(
            "boot state: trace tick {}, the match's first tick {}",
            boot.first_tick - 1,
            boot.first_tick
        );
        Some(boot)
    };
    let mut load = |name: &str| DiscFile {
        entrynum: disc
            .entrynum(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
        bytes: disc
            .read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
    };
    let mut mixer = Mixer::new();
    let mut m = Match::start_from(&settings, &mut load, &mut |g, data| match &boot {
        Some(boot) => {
            boot.seed(g, data);
            mixer.start(&mut g.audio, Some(&boot.memory()));
        }
        None => mixer.start(&mut g.audio, None),
    });
    for (bus, e) in ["A", "B"].iter().zip(mixer.effects()) {
        let name = match e {
            None => "none".to_owned(),
            Some(melee_audio::fx::Effect::ReverbStd(_)) => "the standard reverb".to_owned(),
            Some(melee_audio::fx::Effect::Delay(_)) => "the delay".to_owned(),
            Some(melee_audio::fx::Effect::Unknown(cb)) => format!("callback {cb:#010x}, not mixed"),
        };
        println!("aux {bus}: {name}");
    }
    let mut ours = Vec::new();
    mixer.mix(&mut m.game.audio, &m.data, &mut ours);
    println!(
        "the match's start: {} samples ({} frames)",
        ours.len(),
        ours.len() / melee_audio::FRAME_SAMPLES
    );
    let (mut sim, mut mixing, mut ran) = (Duration::ZERO, Duration::ZERO, 0u32);
    for _ in 0..ticks {
        let t0 = Instant::now();
        let f = m.step_replay(&mut melee_slippi::Device(&replay));
        let ended = f.ended;
        m.recycle(f);
        let t1 = Instant::now();
        mixer.mix(&mut m.game.audio, &m.data, &mut ours);
        sim += t1 - t0;
        mixing += t1.elapsed();
        ran += 1;
        if ended {
            break;
        }
    }
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / f64::from(ran.max(1));
    println!(
        "{ran} ticks: simulation {:.3} ms a tick, mixing {:.3} ms a tick",
        ms(sim),
        ms(mixing)
    );
    let s = &mixer.stats;
    println!(
        "mixed {} frames ({:.2} s); drifted voice frames {}, ARAM writes from memory {}, from missing files {} {:?}",
        s.frames,
        ours.len() as f64 / RATE as f64,
        s.drift,
        s.memory_writes,
        s.missing_files,
        s.missing
            .iter()
            .map(|&e| disc.path_of(e).unwrap_or_else(|_| format!("entry {e}")))
            .collect::<Vec<_>>()
    );
    let loud = |x: &[Sample]| {
        let e: f64 = x
            .iter()
            .map(|s| f64::from(s[0]).powi(2) + f64::from(s[1]).powi(2))
            .sum();
        (e / (2 * x.len().max(1)) as f64).sqrt()
    };
    println!(
        "our mix's RMS level: {:.1}; frames the aux effects processed sound in: A {}, B {}",
        loud(&ours),
        s.aux_active[0],
        s.aux_active[1]
    );
    if let Some(path) = &out {
        write_wav(path, &ours).map_err(|e| format!("{path}: {e}"))?;
    }
    let Some((offset, how)) = align(&ours, &theirs) else {
        return Err("the two streams don't line up (ours is silent, or too short)".into());
    };
    println!(
        "aligned {how}: our first sample is the recording's {offset} ({:.3} s, frame {:.2})",
        offset as f64 / RATE as f64,
        offset as f64 / melee_audio::FRAME_SAMPLES as f64
    );
    let n = ours.len().min(theirs.len().saturating_sub(offset));
    let per = ((window * RATE as f64) as usize).max(1);
    let mut all = Window::default();
    let mut first_diff = None;
    for start in (0..n).step_by(per) {
        let mut w = Window::default();
        for i in start..(start + per).min(n) {
            let (a, b) = (ours[i], theirs[offset + i]);
            if a != b && first_diff.is_none() {
                first_diff = Some(i);
            }
            w.add(a, b);
            all.add(a, b);
        }
        println!(
            "{:8.2} s: SNR {:>9}, max error {:5}, equal {:6.2}%",
            start as f64 / RATE as f64,
            w.snr(),
            w.max,
            w.share()
        );
    }
    println!(
        "all {:.2} s: SNR {}, max error {}, equal {:.2}%",
        n as f64 / RATE as f64,
        all.snr(),
        all.max,
        all.share()
    );
    match first_diff {
        Some(i) => println!(
            "first difference at our sample {i} (frame {}, +{}): ours {:?}, theirs {:?}",
            i / melee_audio::FRAME_SAMPLES,
            i % melee_audio::FRAME_SAMPLES,
            ours[i],
            theirs[offset + i]
        ),
        None => println!("sample-exact"),
    }
    Ok(first_diff.is_none())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The simulation recurses as the original does: a deep stack.
    let child = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(&args))
        .expect("a thread");
    match child.join() {
        Ok(Ok(true)) => ExitCode::SUCCESS,
        Ok(Ok(false)) => ExitCode::FAILURE,
        Ok(Err(e)) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
        Err(_) => ExitCode::FAILURE,
    }
}
