//! The paced bench's frames one by one (`--bench` with High refresh): each frame's CPU parts and
//! its GPU passes' times (timestamp queries, [`melee_gpu::gpu_time`]), so the frames that miss a
//! display's interval show what they held: the GPU's own work, or waiting with the GPU idle.
//! `MELEE_BENCH_FRAMES=FILE` also writes every frame as a CSV row.

use std::fmt::Write as _;
use std::time::Duration;

use melee_gpu::gpu_time::{FrameGpu, Stage};
use melee_gpu::made::{Made, What};

/// 144 Hz's interval, Sarah's target (2026-11-20).
pub const INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 144);

/// One frame: its place (the tick and the frame between ticks), the CPU's parts and the GPU's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Row {
    pub tick: u32,
    /// The frame's place between ticks (0 the first after the tick).
    pub k: u32,
    /// Before the frame's drawing could start: waiting for Extrapolate's prediction, made beside
    /// the tick's first frame, to finish (on the second frame, `k` 1).
    pub ahead: Duration,
    /// Building and running the frame on the CPU, until the renderer hands it over.
    pub draw: Duration,
    /// Recording and submitting the present.
    pub submit: Duration,
    /// Waiting for the GPU to finish everything submitted.
    pub wait: Duration,
    /// The frame's passes' times on the GPU, when timed.
    pub gpu: Option<FrameGpu>,
    /// What the renderer made over the frame ([`melee_gpu::made`]).
    pub made: Made,
    /// The fast renderer's game thread building the frame until its last piece is handed on
    /// (`Frames::handed_in`), and its stream thread running the pieces (`Gpu::fed`).
    pub build: Duration,
    pub stream: Duration,
    /// On a tick's first frame (`k` 0): the tick before it (the simulation and its display
    /// pass), and keeping its transforms or predicting the next.
    pub tick_sim: Duration,
    pub tick_keep: Duration,
}

impl Row {
    /// The frame with its present and the GPU's wait, and the wait for the prediction before it.
    pub fn presented(&self) -> Duration {
        self.ahead + self.draw + self.submit + self.wait
    }
}

#[derive(Default)]
pub struct Frames {
    pub rows: Vec<Row>,
}

impl Frames {
    pub fn with_capacity(n: usize) -> Frames {
        Frames {
            rows: Vec::with_capacity(n),
        }
    }

    pub fn push(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// The GPU times read back after the last frame's present, which are the frame's `back`
    /// frames before the last: the last's (0) when the bench waits for each frame, the one
    /// before (1) when it leaves one in flight. One time is that frame's; anything else (a
    /// frame not timed, or times from before) is left off rather than put on the wrong frame.
    pub fn gpu(&mut self, times: &[FrameGpu], back: usize) {
        let at = self.rows.len().checked_sub(back + 1);
        if let ([t], Some(row)) = (times, at.and_then(|i| self.rows.get_mut(i))) {
            row.gpu = Some(*t);
        }
    }

    /// The report's lines: the GPU's time a frame by stage, the frames over `interval` with
    /// presenting and where they fall in the tick, and the slowest frames' parts.
    pub fn report(&self, interval: Duration) -> String {
        let mut out = String::new();
        let rows = &self.rows;
        let timed: Vec<&FrameGpu> = rows.iter().filter_map(|r| r.gpu.as_ref()).collect();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        if !timed.is_empty() {
            let line = |name: &str, f: &dyn Fn(&FrameGpu) -> Duration| {
                let mut v: Vec<Duration> = timed.iter().map(|g| f(g)).collect();
                format!(
                    "{name} p50 {:.3}, p99 {:.3}, max {:.3}",
                    ms(pct(&mut v, 0.5)),
                    ms(pct(&mut v, 0.99)),
                    ms(pct(&mut v, 1.0))
                )
            };
            let mut parts = vec![line("busy", &FrameGpu::busy), line("span", &|g| g.span)];
            for s in Stage::ALL {
                if timed.iter().any(|g| g.passes[s as usize] > 0) {
                    let passes: u64 = timed.iter().map(|g| u64::from(g.passes[s as usize])).sum();
                    parts.push(format!(
                        "{} ({:.1} passes) {}",
                        s.name(),
                        passes as f64 / timed.len() as f64,
                        line("", &|g| g.stage(s)).trim_start()
                    ));
                }
            }
            let _ = writeln!(
                out,
                "the GPU a frame ({} of {} timed), ms: {}",
                timed.len(),
                rows.len(),
                parts.join("; ")
            );
        }
        let slow: Vec<&Row> = rows.iter().filter(|r| r.presented() > interval).collect();
        let places = rows.iter().map(|r| r.k + 1).max().unwrap_or(0);
        let by_k: Vec<String> = (0..places)
            .map(|k| {
                let all = rows.iter().filter(|r| r.k == k).count();
                let over = slow.iter().filter(|r| r.k == k).count();
                format!("frame {k} {over} of {all}")
            })
            .collect();
        let mean = |f: &dyn Fn(&Row) -> Duration, of: &[&Row]| {
            ms(of.iter().map(|r| f(r)).sum::<Duration>()) / of.len().max(1) as f64
        };
        let gpu_mean = |f: &dyn Fn(&FrameGpu) -> Duration, of: &[&Row]| {
            let g: Vec<Duration> = of.iter().filter_map(|r| r.gpu.as_ref().map(f)).collect();
            ms(g.iter().sum::<Duration>()) / g.len().max(1) as f64
        };
        let fast: Vec<&Row> = rows.iter().filter(|r| r.presented() <= interval).collect();
        let _ = writeln!(
            out,
            "frames over {:.2} ms with presenting: {} of {} ({:.2}%); by place in the tick: {}",
            ms(interval),
            slow.len(),
            rows.len(),
            100.0 * slow.len() as f64 / rows.len().max(1) as f64,
            by_k.join(", ")
        );
        for (name, of) in [("over", &slow), ("within", &fast)] {
            let _ = writeln!(
                out,
                "  the frames {name} it, mean ms: drawing {:.3}, submitting the present {:.3}, the GPU's wait {:.3}; the GPU busy {:.3}, span {:.3}",
                mean(&|r| r.draw, of),
                mean(&|r| r.submit, of),
                mean(&|r| r.wait, of),
                gpu_mean(&FrameGpu::busy, of),
                gpu_mean(&|g| g.span, of)
            );
        }
        let mut worst: Vec<&Row> = rows.iter().collect();
        worst.sort_by_key(|r| std::cmp::Reverse(r.presented()));
        let _ = writeln!(out, "the slowest frames, ms:");
        for r in worst.iter().take(12) {
            let gpu = r.gpu.map_or("not timed".to_owned(), |g| {
                let stages: Vec<String> = Stage::ALL
                    .iter()
                    .filter(|&&s| g.passes[s as usize] > 0)
                    .map(|&s| {
                        format!(
                            "{} {:.3} ({})",
                            s.name(),
                            ms(g.stage(s)),
                            g.passes[s as usize]
                        )
                    })
                    .collect();
                format!(
                    "GPU busy {:.3}, span {:.3}: {}",
                    ms(g.busy()),
                    ms(g.span),
                    stages.join(", ")
                )
            });
            let _ = writeln!(
                out,
                "  tick {} frame {}: {:.3} = {}drawing {:.3} + submitting {:.3} + waiting {:.3}; {gpu}{}",
                r.tick,
                r.k,
                ms(r.presented()),
                if r.ahead.is_zero() {
                    String::new()
                } else {
                    format!("the prediction {:.3} + ", ms(r.ahead))
                },
                ms(r.draw),
                ms(r.submit),
                ms(r.wait),
                if r.made.any() {
                    format!("; made {}", r.made.summary())
                } else {
                    String::new()
                }
            );
        }
        out
    }

    /// Every frame as a CSV row, times in microseconds.
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "tick,k,draw,submit,wait,build,stream,tick_sim,tick_keep,ahead,gpu_busy,gpu_span",
        );
        for s in Stage::ALL {
            let _ = write!(out, ",{0},{0}_passes", s.name().replace(' ', "_"));
        }
        for w in What::ALL {
            let _ = write!(out, ",{0},{0}_us", w.name());
        }
        out.push('\n');
        let us = |d: Duration| d.as_micros();
        for r in &self.rows {
            let _ = write!(
                out,
                "{},{},{},{},{},{},{},{},{},{}",
                r.tick,
                r.k,
                us(r.draw),
                us(r.submit),
                us(r.wait),
                us(r.build),
                us(r.stream),
                us(r.tick_sim),
                us(r.tick_keep),
                us(r.ahead)
            );
            match &r.gpu {
                Some(g) => {
                    let _ = write!(out, ",{},{}", us(g.busy()), us(g.span));
                    for s in Stage::ALL {
                        let _ = write!(out, ",{},{}", us(g.stage(s)), g.passes[s as usize]);
                    }
                }
                None => out.push_str(&",".repeat(2 + 2 * Stage::ALL.len())),
            }
            for w in What::ALL {
                let _ = write!(out, ",{},{}", r.made.count(w), us(r.made.time(w)));
            }
            out.push('\n');
        }
        out
    }
}

/// The `p` quantile of `v` (sorting it), nearest rank.
fn pct(v: &mut [Duration], p: f64) -> Duration {
    v.sort();
    let i = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len().max(1)) - 1;
    v.get(i).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(scene: u64) -> FrameGpu {
        let mut g = FrameGpu::default();
        g.stages[Stage::Scene as usize] = Duration::from_micros(scene);
        g.passes[Stage::Scene as usize] = 1;
        g.span = Duration::from_micros(scene);
        g
    }

    #[test]
    fn gpu_times_go_on_the_frame_just_presented_or_on_none() {
        let mut f = Frames::default();
        f.gpu(&[gpu(1)], 0);
        assert!(f.rows.is_empty());
        f.push(Row::default());
        f.push(Row::default());
        f.gpu(&[gpu(5)], 0);
        assert!(f.rows[0].gpu.is_none());
        assert_eq!(f.rows[1].gpu.unwrap().busy(), Duration::from_micros(5));
        // Two at once (one left from before), or none: the frame stays untimed.
        f.push(Row::default());
        f.gpu(&[gpu(2), gpu(3)], 0);
        f.gpu(&[], 0);
        assert!(f.rows[2].gpu.is_none());
        // One frame in flight: the times are the frame before's, and the first frame has none
        // before it.
        f.push(Row::default());
        f.gpu(&[gpu(7)], 1);
        assert_eq!(f.rows[2].gpu.unwrap().busy(), Duration::from_micros(7));
        assert!(f.rows[3].gpu.is_none());
        let mut g = Frames::default();
        g.push(Row::default());
        g.gpu(&[gpu(7)], 1);
        assert!(g.rows[0].gpu.is_none());
    }

    #[test]
    fn the_report_counts_the_slow_frames_by_their_place_in_the_tick() {
        let mut f = Frames::default();
        let ms = Duration::from_millis;
        for tick in 0..4 {
            for k in 0..2 {
                let wait = if k == 0 && tick % 2 == 1 {
                    ms(20)
                } else {
                    ms(1)
                };
                f.push(Row {
                    tick,
                    k,
                    draw: ms(2),
                    submit: Duration::ZERO,
                    wait,
                    gpu: Some(gpu(1000)),
                    made: Made::default(),
                    ..Row::default()
                });
            }
        }
        let r = f.report(INTERVAL);
        assert!(r.contains("2 of 8 (25.00%)"), "{r}");
        assert!(r.contains("frame 0 2 of 4, frame 1 0 of 4"), "{r}");
        assert!(r.contains("tick 1 frame 0: 22.000"), "{r}");
        assert!(r.contains("busy p50 1.000"), "{r}");
        assert_eq!(f.csv().lines().count(), 9);
    }
}
