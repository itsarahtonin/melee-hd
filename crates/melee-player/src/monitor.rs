//! The host's times for Melee's debug performance monitor (X and D-pad right at a debug level,
//! `melee_sim::game::perf`): on a console it draws the CPU's, the drawing's and the whole
//! frame's time and the frames shown each second; here the player's own, measured as it runs
//! and handed to the renderer each frame (`melee_render::hsd::PerfHost`). They reach the drawing
//! alone: never the simulation, a recording or a replay.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use melee_render::hsd::PerfHost;

/// What the monitor shows: the last frame's simulation, its drawing (the frame built and its
/// GPU work submitted) and the whole frame to its present, and the presents of the last second.
#[derive(Default)]
pub struct HostTimes {
    sim: Duration,
    build: Duration,
    total: Duration,
    presents: VecDeque<Instant>,
    stats: melee_sim::game::perf::Stats,
}

/// A duration in 60ths of a second, as the console's perf timer counts.
fn sixtieths(d: Duration) -> f32 {
    (d.as_secs_f64() * 60.0) as f32
}

impl HostTimes {
    /// The tick's simulation took `d`.
    pub fn sim(&mut self, d: Duration) {
        self.sim = d;
    }

    /// A frame was built in `build` and presented `total` after its tick began, at `at`.
    pub fn frame(&mut self, build: Duration, total: Duration, at: Instant) {
        self.build = build;
        self.total = total;
        self.presents.push_back(at);
        while self
            .presents
            .front()
            .is_some_and(|&p| at.saturating_duration_since(p) >= Duration::from_secs(1))
        {
            self.presents.pop_front();
        }
    }

    /// The renderer's times for the next frame: the last frame's, as the console's monitor
    /// draws the last frame's (`HSD_PerfLastStat`). The drawn times' statics the renderer
    /// updated last frame come back in with them.
    pub fn host(&mut self, last: Option<PerfHost>) -> PerfHost {
        if let Some(l) = last {
            self.stats = l.stats;
        }
        PerfHost {
            times: [
                sixtieths(self.sim),
                sixtieths(self.build),
                sixtieths(self.total),
            ],
            frame_renew: self.presents.len() as i32,
            stats: self.stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host's times, in 60ths of a second, and its presents over the last second replace
    /// the timeline's in what the renderer draws; the drawn statics carry over frames.
    #[test]
    fn host_times_replace_the_timeline() {
        let mut h = HostTimes::default();
        let t0 = Instant::now();
        h.sim(Duration::from_millis(5));
        for i in 0..144u32 {
            h.frame(
                Duration::from_millis(2),
                Duration::from_micros(8_333),
                t0 + Duration::from_micros(6_944) * i,
            );
        }
        let p = h.host(None);
        assert_eq!(
            p.times,
            [0.3, 0.12, sixtieths(Duration::from_micros(8_333))]
        );
        assert_eq!(p.frame_renew, 144);
        let timeline = melee_sim::game::perf::PerfDraw {
            items: vec![],
            stats: melee_sim::game::perf::Stats::default(),
            frame_renew: 60,
            game_ticks: 0,
            movie: [0; 3],
        };
        let mut host = Some(p);
        let shown = melee_render::hsd::devtext::perf_shown(host.as_mut(), &timeline);
        assert_eq!(shown.stats.now, p.times);
        assert_eq!(shown.frame_renew, 144);
        assert_eq!(
            melee_render::hsd::devtext::perf_shown(None, &timeline),
            timeline.timeline()
        );
        // The statics the renderer updated come back next frame.
        let next = h.host(host);
        assert_eq!(next.stats.peak, p.times);
    }
}
