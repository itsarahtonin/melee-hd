//! The renderers' benchmark (docs/player.md, "The fast renderer"): a replay's frames drawn by
//! one renderer, headless, each frame's CPU time building and submitting it and the GPU's time
//! finishing it measured, at a scale. The simulation runs but isn't timed. Each frame waits for
//! the GPU before the next starts, so frames a second here are the renderer's alone with CPU
//! and GPU taken in turn: a floor under what the player, which overlaps them, makes of it
//! (`melee-bench player` measures that, in a window).

use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_sim::game::state::Cosmetic;

use super::exact::Exact;
use super::{Adapter, FrameRenderer, Which, device, renderer};
use crate::native::{self, View};

/// One frame's times.
#[derive(Clone, Copy, Default)]
pub struct Times {
    pub build: Duration,
    pub submit: Duration,
    /// Waiting for the GPU after the submit.
    pub gpu: Duration,
}

impl Times {
    pub fn cpu(&self) -> Duration {
        self.build + self.submit
    }

    pub fn total(&self) -> Duration {
        self.cpu() + self.gpu
    }
}

/// Mean and percentiles of a set of durations, in milliseconds.
pub fn stats(mut v: Vec<Duration>) -> (f64, f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    v.sort_unstable();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let at = |p: f64| ms(v[((v.len() - 1) as f64 * p).round() as usize]);
    let mean = v.iter().map(|&d| ms(d)).sum::<f64>() / v.len() as f64;
    (mean, at(0.5), at(0.95), at(0.99))
}

/// Draws the replay's native frames to `to` with `r` (frames before `from` drawn but not
/// measured: textures, pipelines and shaders made), each waited for; the times of `from` on.
pub fn measure(
    disc: &str,
    replay: &str,
    from: u32,
    to: u32,
    r: &mut dyn FrameRenderer,
) -> Vec<Times> {
    let mut out = Vec::with_capacity((to - from) as usize);
    native::drive(disc, replay, to, Cosmetic::default(), |t| {
        let s = r.frame(t, true, false);
        let start = Instant::now();
        r.wait_idle();
        let gpu = start.elapsed();
        if t.n >= from {
            out.push(Times {
                build: s.build,
                submit: s.submit,
                gpu,
            });
        }
        t.n < to
    });
    out
}

/// `melee-render-check bench DISC REPLAY [--renderer exact|f2|fast] [--scale N] [--from A] [--to B]
/// [--one-thread] [--adapter NAME|fallback]`
pub fn cli(args: &[String]) -> ExitCode {
    let (disc, replay) = (&args[1], &args[2]);
    let (mut scale, mut from, mut to, mut threaded) = (1u32, 600u32, 1800u32, true);
    let mut adapter = Adapter::Default;
    let mut which = Which::Exact;
    let mut rest = args[3..].iter();
    while let Some(a) = rest.next() {
        let mut num = |what: &str| -> u32 {
            rest.next()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| panic!("{what} N"))
        };
        match a.as_str() {
            "--scale" => scale = num("--scale"),
            "--from" => from = num("--from"),
            "--to" => to = num("--to"),
            "--one-thread" => threaded = false,
            "--renderer" => which = Which::parse(rest.next().expect("--renderer NAME")),
            "--adapter" => adapter = Adapter::parse(rest.next().expect("--adapter NAME")),
            other => panic!("bench: unknown argument {other}"),
        }
    }
    assert!(from < to, "--from before --to");
    let (dev, queue, name) = device(&adapter);
    // The frame stays on the GPU, as the player's does: no readback.
    let view = View {
        gpu_xfb: true,
        ..View::console(scale)
    };
    let mut r = renderer(&dev, &queue, view);
    r.xfb_readback = false;
    let mut exact = match which {
        Which::Exact => Exact::new(r, threaded),
        Which::F2 => Exact::f2(r, threaded),
        Which::Fast => Exact::fast(r, threaded),
    };
    let times = measure(disc, replay, from, to, &mut exact);
    if let Some((s, (n, bytes))) = exact.meshes() {
        println!(
            "meshes: {n} kept, {:.1} MB; display lists drawn from a kept mesh {}, decoded {}, as the stream has them {}",
            bytes as f64 / 1e6,
            s.hits,
            s.decoded,
            s.streamed
        );
    }
    let n = times.len();
    let (build, submit, cpu, gpu, total) = (
        stats(times.iter().map(|t| t.build).collect()),
        stats(times.iter().map(|t| t.submit).collect()),
        stats(times.iter().map(Times::cpu).collect()),
        stats(times.iter().map(|t| t.gpu).collect()),
        stats(times.iter().map(Times::total).collect()),
    );
    let workers = std::env::var("MELEE_GPU_WORKERS").unwrap_or_else(|_| "default".to_owned());
    let name_of = std::path::Path::new(replay)
        .file_stem()
        .map_or_else(|| replay.to_owned(), |s| s.to_string_lossy().into_owned());
    println!(
        "config: {} {name_of}, frames {from} to {to}, {scale}x, {}, transform workers {workers}, {name}",
        exact.name(),
        if threaded { "threaded" } else { "one thread" },
    );
    println!("frames measured: {n}");
    let line = |what: &str, s: (f64, f64, f64, f64)| {
        println!(
            "{what} ms: mean {:.3} p50 {:.3} p95 {:.3} p99 {:.3}",
            s.0, s.1, s.2, s.3
        );
    };
    line("build", build);
    line("submit", submit);
    line("cpu", cpu);
    line("gpu wait", gpu);
    line("frame", total);
    let fps = if total.0 > 0.0 { 1000.0 / total.0 } else { 0.0 };
    println!("frames/s: {fps:.1}");
    println!(
        "| {name_of} | {scale}x | {fps:.0} | {:.2} ({:.2}) | {:.2} | {:.2} | {:.2} |",
        cpu.0, cpu.2, build.0, submit.0, gpu.0
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_of_durations() {
        let v: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        let (mean, p50, p95, p99) = stats(v);
        assert!((mean - 50.5).abs() < 1e-9);
        assert_eq!((p50, p95, p99), (51.0, 95.0, 99.0));
        assert_eq!(stats(Vec::new()), (0.0, 0.0, 0.0, 0.0));
    }
}
