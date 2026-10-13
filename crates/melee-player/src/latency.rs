//! The latency log (`--latency-log FILE`): for each tick, how old the controllers' input was
//! when the tick read it, how long the tick took to simulate, render and hand its frame to the
//! display, and how much sound was queued ahead of the tick's samples. It writes one CSV row a
//! tick and prints a summary when the player quits; docs/player.md says what each column means.
//! What happens after the present call (the GPU finishing, the compositor, scan-out) the player
//! can't see portably: on Windows PresentMon measures it. F3 in a match shows the last second's
//! means over the frame ([`Live`], [`overlay`]).

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

use melee_audio::OutputStats;
use melee_input::InputAges;

use crate::launcher::Painter;
use crate::present::Gpu;

/// When a frame's stages finished, from its tick's start.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTimes {
    /// The renderer has drawn the frame (its GPU work submitted).
    pub rendered: Duration,
    /// The time spent waiting for the swap chain's next image.
    pub acquire: Duration,
    /// The present call returned: the frame is the display's to show.
    pub presented: Duration,
}

/// One tick's measures.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    /// When the tick started, and how long after it was due.
    pub at: Option<Instant>,
    pub late: Duration,
    pub ages: InputAges,
    /// The adapter's reports so far, for its rate.
    pub adapter_reports: Option<u64>,
    /// From the tick's start to the simulation's step (with the sound's mix) done.
    pub sim: Duration,
    pub frame: Option<FrameTimes>,
    pub audio: Option<OutputStats>,
}

/// The log: the CSV file and every tick's row, for the summary.
pub struct Log {
    path: String,
    out: BufWriter<File>,
    start: Option<Instant>,
    rows: Vec<Tick>,
    /// The present mode and the added delay, for the summary.
    pub present_mode: String,
    pub added_delay: u32,
}

const HEADER: &str = "tick,time_ms,late_ms,adapter_age_ms,keyboard_age_ms,gamepad_age_ms,adapter_reports,sim_ms,frame,rendered_ms,acquire_ms,presented_ms,audio_ring_ms,audio_device_ms,audio_total_ms,audio_callback_frames,audio_underruns,audio_skips";

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn opt_ms(d: Option<Duration>) -> String {
    d.map_or(String::new(), |d| format!("{:.3}", ms(d)))
}

impl Log {
    pub fn create(path: &str) -> Result<Log, String> {
        let file = File::create(path).map_err(|e| format!("{path}: {e}"))?;
        let mut out = BufWriter::new(file);
        writeln!(out, "{HEADER}").map_err(|e| format!("{path}: {e}"))?;
        Ok(Log {
            path: path.to_owned(),
            out,
            start: None,
            rows: Vec::with_capacity(60 * 600),
            present_mode: String::new(),
            added_delay: 0,
        })
    }

    /// Records a tick and writes its row.
    pub fn tick(&mut self, t: Tick) {
        let at = t.at.unwrap_or_else(Instant::now);
        let start = *self.start.get_or_insert(at);
        let f = t.frame;
        let a = t.audio;
        let row = format!(
            "{},{:.3},{:.3},{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{}",
            self.rows.len(),
            ms(at.saturating_duration_since(start)),
            ms(t.late),
            opt_ms(t.ages.adapter),
            opt_ms(t.ages.keyboard),
            opt_ms(t.ages.gamepad),
            t.adapter_reports.map_or(String::new(), |n| n.to_string()),
            ms(t.sim),
            u8::from(f.is_some()),
            opt_ms(f.map(|f| f.rendered)),
            opt_ms(f.map(|f| f.acquire)),
            opt_ms(f.map(|f| f.presented)),
            opt_ms(a.map(|a| a.ring)),
            opt_ms(a.map(|a| a.device)),
            opt_ms(a.map(|a| a.total())),
            a.map_or(String::new(), |a| a.callback_frames.to_string()),
            a.map_or(String::new(), |a| a.underruns.to_string()),
            a.map_or(String::new(), |a| a.skips.to_string()),
        );
        if let Err(e) = writeln!(self.out, "{row}") {
            eprintln!("{}: {e}", self.path);
        }
        self.rows.push(t);
    }

    /// Flushes the file and prints the summary.
    pub fn finish(mut self) {
        if let Err(e) = self.out.flush() {
            eprintln!("{}: {e}", self.path);
        }
        eprintln!("{}", self.summary());
    }

    /// The summary: medians, 99th percentiles and worsts of each measure.
    pub fn summary(&self) -> String {
        let rows = &self.rows;
        let frames: Vec<FrameTimes> = rows.iter().filter_map(|t| t.frame).collect();
        let mut s = format!(
            "latency log {}: {} ticks, {} frames, present mode {}, added delay {} frames\n",
            self.path,
            rows.len(),
            frames.len(),
            self.present_mode,
            self.added_delay
        );
        let mut line = |what: &str, v: Vec<Duration>| {
            if let Some(d) = Dist::of(v) {
                s += &format!("  {what}: {d}\n");
            }
        };
        line(
            "input age, adapter report",
            rows.iter().filter_map(|t| t.ages.adapter).collect(),
        );
        line(
            "input age, key event",
            rows.iter().filter_map(|t| t.ages.keyboard).collect(),
        );
        line(
            "input age, gamepad event",
            rows.iter().filter_map(|t| t.ages.gamepad).collect(),
        );
        line(
            "tick start after its due time",
            rows.iter().map(|t| t.late).collect(),
        );
        line(
            "tick start to simulated",
            rows.iter().map(|t| t.sim).collect(),
        );
        line(
            "tick start to rendered",
            frames.iter().map(|f| f.rendered).collect(),
        );
        line(
            "waiting for the swap chain",
            frames.iter().map(|f| f.acquire).collect(),
        );
        line(
            "tick start to presented (present call returned)",
            frames.iter().map(|f| f.presented).collect(),
        );
        line(
            "adapter report arrived to presented",
            rows.iter()
                .filter_map(|t| Some(t.ages.adapter? + t.frame?.presented))
                .collect(),
        );
        let audio: Vec<OutputStats> = rows.iter().filter_map(|t| t.audio).collect();
        line(
            "audio queued in the ring at the tick's push",
            audio.iter().map(|a| a.ring).collect(),
        );
        line(
            "audio queued in the device",
            audio.iter().map(|a| a.device).collect(),
        );
        line(
            "audio latency (ring and device)",
            audio.iter().map(|a| a.total()).collect(),
        );
        if let Some(a) = audio.last() {
            s += &format!(
                "  audio: {}-sample device callbacks, {} underruns, {} backlogs dropped\n",
                a.callback_frames, a.underruns, a.skips
            );
        }
        if let Some(rate) = self.adapter_rate() {
            s += &format!("  adapter: {rate:.0} reports a second\n");
        }
        s.trim_end().to_owned()
    }

    /// The adapter's reports a second over the ticks it was open.
    fn adapter_rate(&self) -> Option<f64> {
        let mut with = self
            .rows
            .iter()
            .filter_map(|t| Some((t.at?, t.adapter_reports?)));
        let (t0, n0) = with.next()?;
        let (t1, n1) = with.next_back()?;
        let secs = t1.saturating_duration_since(t0).as_secs_f64();
        (secs > 0.5 && n1 > n0).then(|| (n1 - n0) as f64 / secs)
    }
}

/// The last second's ticks, for the overlay.
#[derive(Default)]
pub struct Live {
    recent: VecDeque<Tick>,
}

impl Live {
    pub fn tick(&mut self, t: Tick) {
        if self.recent.len() == 60 {
            self.recent.pop_front();
        }
        self.recent.push_back(t);
    }

    /// The overlay's lines: the last second's means.
    pub fn text(&self, present_mode: &str, added_delay: u32) -> String {
        let mean = |v: Vec<Duration>| -> Option<f64> {
            (!v.is_empty()).then(|| ms(v.iter().sum::<Duration>()) / v.len() as f64)
        };
        let show = |v: Option<f64>| v.map_or("-".to_owned(), |v| format!("{v:.1} ms"));
        let r = &self.recent;
        let input = mean(r.iter().filter_map(|t| t.ages.adapter).collect()).or_else(|| {
            mean(
                r.iter()
                    .filter_map(|t| t.ages.keyboard.or(t.ages.gamepad))
                    .collect(),
            )
        });
        let presented = mean(r.iter().filter_map(|t| Some(t.frame?.presented)).collect());
        let audio = mean(r.iter().filter_map(|t| Some(t.audio?.total())).collect());
        let underruns = r.back().and_then(|t| t.audio).map_or(0, |a| a.underruns);
        let rate = match (r.front(), r.back()) {
            (Some(a), Some(b)) => match (a.at, b.at, a.adapter_reports, b.adapter_reports) {
                (Some(t0), Some(t1), Some(n0), Some(n1)) if t1 > t0 => {
                    format!(", {:.0} Hz", (n1 - n0) as f64 / (t1 - t0).as_secs_f64())
                }
                _ => String::new(),
            },
            _ => String::new(),
        };
        format!(
            "input age {}{rate}\ntick to present {}\naudio queue {} ({underruns} underruns)\n{present_mode}, added delay {added_delay}",
            show(input),
            show(presented),
            show(audio),
        )
    }
}

/// Draws `text` in the frame's top left corner, over it.
pub fn overlay(
    painter: &mut Painter,
    gpu: &Gpu,
    text: &str,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
) {
    let zoom = painter.ctx.zoom_factor();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(w as f32, h as f32) / zoom,
        )),
        ..Default::default()
    };
    let out = painter.ctx.run_ui(input, |ui| {
        egui::Area::new(egui::Id::new("latency"))
            .fixed_pos([8.0, 8.0])
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(190))
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(text)
                                .monospace()
                                .color(egui::Color32::WHITE),
                        )
                    });
            });
    });
    painter.paint(gpu, out, view, w, h);
}

/// A measure's median, 99th percentile and worst.
struct Dist {
    n: usize,
    p50: Duration,
    p99: Duration,
    max: Duration,
    mean: Duration,
}

impl Dist {
    fn of(mut v: Vec<Duration>) -> Option<Dist> {
        if v.is_empty() {
            return None;
        }
        v.sort();
        let at = |p: f64| v[((v.len() as f64 * p).ceil() as usize).clamp(1, v.len()) - 1];
        Some(Dist {
            n: v.len(),
            p50: at(0.5),
            p99: at(0.99),
            max: v[v.len() - 1],
            mean: v.iter().sum::<Duration>() / v.len() as u32,
        })
    }
}

impl std::fmt::Display for Dist {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "mean {:.2} ms, p50 {:.2}, p99 {:.2}, max {:.2} ({} samples)",
            ms(self.mean),
            ms(self.p50),
            ms(self.p99),
            ms(self.max),
            self.n
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_summary() {
        let dir = std::env::temp_dir().join(format!("melee-hd-latency-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("log.csv");
        let mut log = Log::create(path.to_str().unwrap()).unwrap();
        log.present_mode = "Mailbox".into();
        let t0 = Instant::now();
        for i in 0..120u32 {
            log.tick(Tick {
                at: Some(t0 + Duration::from_micros(16_667 * u64::from(i))),
                late: Duration::from_micros(100),
                ages: InputAges {
                    adapter: Some(Duration::from_micros(500 + u64::from(i % 8) * 100)),
                    ..InputAges::default()
                },
                adapter_reports: Some(u64::from(i) * 1000 / 60),
                sim: Duration::from_millis(1),
                frame: (i % 2 == 0).then_some(FrameTimes {
                    rendered: Duration::from_millis(4),
                    acquire: Duration::ZERO,
                    presented: Duration::from_millis(5),
                }),
                audio: None,
            });
        }
        let mut live = Live::default();
        for t in &log.rows {
            live.tick(*t);
        }
        let text = live.text("Mailbox", 0);
        assert!(text.contains("input age 0.9 ms, 1000 Hz"), "{text}");
        assert!(text.contains("tick to present 5.0 ms"), "{text}");
        let summary = log.summary();
        assert!(summary.contains("120 ticks, 60 frames"), "{summary}");
        assert!(
            summary.contains("adapter: 1000 reports a second"),
            "{summary}"
        );
        assert!(summary.contains("tick start to presented"), "{summary}");
        log.finish();
        let csv = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 121);
        assert_eq!(
            lines[0].split(',').count(),
            lines[1].split(',').count(),
            "{}",
            lines[1]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
