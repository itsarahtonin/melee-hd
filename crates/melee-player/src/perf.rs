//! The player's own benchmark (`--perf-report FILE`): the real player, in its window, presenting
//! as it always does, measured over a fixed run (a replay or a CPU match with a fixed seed, and
//! `--ticks`). When it quits it writes frames a second, the time between presents, each GPU
//! stage's time a frame (timestamp queries, [`melee_gpu::gpu_time`]) and the CPU's a frame, to
//! the file and the output. `melee-bench player` runs it on a hidden desktop over a matrix of
//! matches and settings and tabulates the reports (docs/performance.md).

use std::time::{Duration, Instant};

use melee_gpu::gpu_time::{FrameGpu, Stage};

use crate::latency::CpuTimes;

/// Ticks left out at the start: the match's first frames, while pipelines and textures load.
pub const SKIP_TICKS: u64 = 180;
/// GPU frames left out once measuring starts: those still read back from before it.
const SKIP_GPU: usize = 16;

/// The run's measures, from the first frame after [`SKIP_TICKS`].
pub struct Perf {
    path: String,
    presents: Vec<Instant>,
    cpu: Vec<CpuTimes>,
    gpu: Vec<FrameGpu>,
    gpu_seen: usize,
    sim: Vec<Duration>,
    ticks: u64,
}

impl Perf {
    pub fn new(path: &str) -> Perf {
        Perf {
            path: path.to_owned(),
            presents: Vec::with_capacity(1 << 16),
            cpu: Vec::with_capacity(1 << 16),
            gpu: Vec::with_capacity(1 << 16),
            gpu_seen: 0,
            sim: Vec::with_capacity(1 << 14),
            ticks: 0,
        }
    }

    fn measuring(&self) -> bool {
        self.ticks >= SKIP_TICKS
    }

    /// The player has run `ticks` ticks.
    pub fn ticks(&mut self, ticks: u64) {
        self.ticks = ticks;
    }

    /// A tick's simulation time, when its row is logged.
    pub fn sim(&mut self, sim: Duration) {
        if self.measuring() {
            self.sim.push(sim);
        }
    }

    /// A frame presented `at`, with its CPU times.
    pub fn frame(&mut self, at: Instant, cpu: CpuTimes) {
        if self.measuring() {
            self.presents.push(at);
            self.cpu.push(cpu);
        }
    }

    /// A frame's GPU times, read back.
    pub fn gpu(&mut self, f: FrameGpu) {
        if self.measuring() {
            self.gpu_seen += 1;
            if self.gpu_seen > SKIP_GPU {
                self.gpu.push(f);
            }
        }
    }

    /// The report: `config` describes the run, `untimed` counts frames the GPU timer missed,
    /// `specialized` says what the specialized shaders did.
    pub fn report(&self, config: &str, untimed: Option<u64>, specialized: Option<&str>) -> String {
        let mut s = format!("melee-hd perf report\nconfig: {config}\n");
        let n = self.presents.len();
        let secs = match (self.presents.first(), self.presents.last()) {
            (Some(a), Some(b)) => b.saturating_duration_since(*a).as_secs_f64(),
            _ => 0.0,
        };
        let fps = if secs > 0.0 {
            (n - 1) as f64 / secs
        } else {
            0.0
        };
        s += &format!("frames: {n} in {secs:.2} s, {fps:.2} frames/s\n");
        let gaps: Vec<Duration> = self
            .presents
            .windows(2)
            .map(|w| w[1].saturating_duration_since(w[0]))
            .collect();
        s += &format!("frame time ms: {}\n", Dist::of(gaps));
        let gpu = |f: &dyn Fn(&FrameGpu) -> Duration| -> Dist {
            Dist::of(self.gpu.iter().map(f).collect())
        };
        s += &format!(
            "gpu ms total: {} frames {} untimed {}\n",
            gpu(&FrameGpu::busy),
            self.gpu.len(),
            untimed.map_or("-".to_owned(), |u| u.to_string())
        );
        s += &format!("gpu ms span: {}\n", gpu(&|f| f.span));
        for stage in Stage::ALL {
            let passes = self
                .gpu
                .iter()
                .map(|f| f64::from(f.passes[stage as usize]))
                .sum::<f64>()
                / self.gpu.len().max(1) as f64;
            s += &format!(
                "gpu ms {}: {} passes {passes:.1}\n",
                stage.name(),
                gpu(&|f| f.stage(stage))
            );
        }
        let cpu = |f: fn(&CpuTimes) -> Duration| Dist::of(self.cpu.iter().map(f).collect());
        s += &format!("cpu ms build: {}\n", cpu(|c| c.build));
        s += &format!("cpu ms swap chain: {}\n", cpu(|c| c.acquire));
        s += &format!("cpu ms present: {}\n", cpu(|c| c.present));
        s += &format!("cpu ms tick: {}\n", Dist::of(self.sim.clone()));
        if let Some(t) = specialized {
            s += &format!("specialized shaders: {t}\n");
        }
        s
    }

    /// Writes the report to the file and prints it.
    pub fn finish(&self, config: &str, untimed: Option<u64>, specialized: Option<&str>) {
        let text = self.report(config, untimed, specialized);
        if let Err(e) = std::fs::write(&self.path, &text) {
            eprintln!("{}: {e}", self.path);
        }
        eprint!("{text}");
    }
}

/// A measure's mean and percentiles, in milliseconds.
struct Dist(Vec<Duration>);

impl Dist {
    fn of(mut v: Vec<Duration>) -> Dist {
        v.sort_unstable();
        Dist(v)
    }

    fn at(&self, p: f64) -> f64 {
        let v = &self.0;
        let i = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len().max(1)) - 1;
        v.get(i).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}

impl std::fmt::Display for Dist {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mean =
            self.0.iter().sum::<Duration>().as_secs_f64() * 1000.0 / self.0.len().max(1) as f64;
        write!(
            f,
            "mean {mean:.3} p50 {:.3} p95 {:.3} p99 {:.3} max {:.3}",
            self.at(0.5),
            self.at(0.95),
            self.at(0.99),
            self.at(1.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_measures_after_the_start() {
        let mut p = Perf::new("unused");
        let t0 = Instant::now();
        let us = Duration::from_micros;
        for i in 0..400u64 {
            p.ticks(i);
            p.sim(us(800));
            p.frame(
                t0 + us(7_000 * i),
                CpuTimes {
                    build: us(2000),
                    acquire: us(0),
                    present: us(300),
                },
            );
            let mut g = FrameGpu::default();
            g.stages[Stage::Scene as usize] = us(4000);
            g.passes[Stage::Scene as usize] = 3;
            g.span = us(5000);
            p.gpu(g);
        }
        let r = p.report("test", Some(2), None);
        assert!(
            r.contains("frames: 220 in 1.53 s, 142.86 frames/s\n"),
            "{r}"
        );
        assert!(r.contains("frame time ms: mean 7.000 p50 7.000"), "{r}");
        assert!(
            r.contains("gpu ms total: mean 4.000 p50 4.000 p95 4.000 p99 4.000 max 4.000 frames 204 untimed 2\n"),
            "{r}"
        );
        assert!(r.contains("gpu ms scene: mean 4.000"), "{r}");
        assert!(r.contains("passes 3.0\n"), "{r}");
        assert!(r.contains("cpu ms build: mean 2.000"), "{r}");
        assert!(r.contains("cpu ms tick: mean 0.800"), "{r}");
    }
}
