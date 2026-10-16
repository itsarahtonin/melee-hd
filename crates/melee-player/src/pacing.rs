//! Frames between ticks, for displays faster than the game's 60 ticks a second
//! (docs/architecture.md, "Frame pacing and high refresh rates").
//!
//! [`Mode::Off`] draws each tick's frame as the tick renders it, one frame a tick.
//! [`Mode::Interpolate`] draws frames at the display's own rate, each between the last two
//! ticks (N-1 and N, `melee_render::blend`), so a frame shows at most a tick later than Off
//! would show it. [`Mode::Extrapolate`] draws them between the latest tick N and a prediction
//! of N+1 (`melee_render::predict`), adding no latency. Ticks still run at the game's rate on
//! their own schedule; frames fill the time between.
//!
//! **The blend's t comes from when the frame reaches the screen.** A frame drawn now shows
//! at its display time D: the next refresh after it's presented (FIFO), or when it's presented
//! (mailbox, immediate and variable refresh, which show a frame as soon as it's ready). The
//! pacer estimates D from the frame's measured start-to-present time and, presenting FIFO, the
//! refreshes it sees: with a swap chain one frame deep, the wait for the next image ends at a
//! refresh, so those waits' ends time the display's refreshes, and D snaps to the refresh grid
//! they set. Then t = (D - lead - s_N) / tick, s_N being when tick N was due and lead the
//! average of D less the frame's start, which keeps t between 0 and 1 while ticks are on time
//! and makes successive frames' t follow their display times, so motion on screen is even at
//! any refresh rate, 144 Hz's uneven frames a tick and variable refresh included. wgpu doesn't
//! expose the swap chain's presentation clock (DXGI's frame statistics, Vulkan's present
//! timing); reading it, per OS, is the follow-up the latency notes name.

use std::time::{Duration, Instant};

/// How frames are drawn for displays faster than the game's ticks (`--high-refresh`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// One frame a tick, as the tick renders it.
    #[default]
    Off,
    /// Frames at the display's rate between the last two ticks: up to a tick of added latency.
    Interpolate,
    /// Frames at the display's rate between the latest tick and the next, predicted: no added
    /// latency, a tick of correction when an input changes.
    Extrapolate,
}

impl Mode {
    /// The launcher's choices, with their names.
    pub const CHOICES: [(Mode, &str); 3] = [
        (Mode::Off, "Off (a frame a tick)"),
        (Mode::Interpolate, "Interpolate (up to a tick later)"),
        (Mode::Extrapolate, "Extrapolate (predicts a tick)"),
    ];

    pub fn parse(v: &str) -> Result<Mode, String> {
        match v {
            "off" => Ok(Mode::Off),
            "interpolate" => Ok(Mode::Interpolate),
            "extrapolate" => Ok(Mode::Extrapolate),
            _ => Err(format!(
                "--high-refresh off|interpolate|extrapolate, not {v}"
            )),
        }
    }

    /// The name the command line and the settings file use.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Interpolate => "interpolate",
            Mode::Extrapolate => "extrapolate",
        }
    }

    /// Whether frames are drawn at the display's rate, between two states.
    pub fn paced(self) -> bool {
        self != Mode::Off
    }

    /// The name the launcher shows.
    pub fn name(self) -> &'static str {
        Mode::CHOICES
            .iter()
            .find(|(m, _)| *m == self)
            .map_or("?", |(_, n)| n)
    }
}

/// A moving average: the mean of its first ten samples, then each new one counting a tenth.
#[derive(Clone, Copy, Debug, Default)]
struct Average {
    mean: f64,
    n: u32,
}

impl Average {
    fn add(&mut self, x: f64) {
        self.n = (self.n + 1).min(10);
        self.mean += (x - self.mean) / f64::from(self.n);
    }

    fn get(self) -> f64 {
        self.mean
    }
}

/// A wait for the swap chain's next image longer than this ended at a refresh.
const BLOCKED: Duration = Duration::from_micros(500);

/// When frames are drawn and what t each takes.
#[derive(Debug)]
pub struct Pacer {
    /// The ticks' period.
    tick: Duration,
    /// The display's refresh period: the frames' cadence.
    refresh: Duration,
    /// Frames wait for a refresh to show (FIFO); else they show when presented.
    fifo: bool,
    /// From a frame's start to its present call returning, in seconds.
    latency: Average,
    /// From a frame's start to its estimated display time, in seconds.
    lead: Average,
    /// A refresh the waits for the swap chain's images saw.
    refreshed: Option<Instant>,
    /// When the next frame is due.
    next: Instant,
}

impl Pacer {
    /// A pacer for ticks of `tick` and a display refreshing every `refresh`, presenting FIFO
    /// when `fifo`.
    pub fn new(tick: Duration, refresh: Duration, fifo: bool, now: Instant) -> Pacer {
        Pacer {
            tick,
            refresh,
            fifo,
            latency: Average::default(),
            lead: Average::default(),
            refreshed: None,
            next: now,
        }
    }

    /// The display's refresh period, which it may change (another monitor, another mode).
    pub fn set_refresh(&mut self, refresh: Duration, fifo: bool) {
        if (refresh, fifo) != (self.refresh, self.fifo) {
            self.refresh = refresh;
            self.fifo = fifo;
            self.refreshed = None;
        }
    }

    pub fn refresh(&self) -> Duration {
        self.refresh
    }

    /// When the next frame is due.
    pub fn due(&self) -> Instant {
        self.next
    }

    /// Starts a frame at `now`: schedules the next a refresh later, and returns t for this one,
    /// whose latest tick was due at `tick_due` (clamped to 0 to 1).
    pub fn start(&mut self, now: Instant, tick_due: Instant) -> f32 {
        // A frame late by more than a refresh (a stall) puts the cadence back on now.
        self.next = (self.next + self.refresh).max(now);
        let shown = self.display_time(now);
        let lead = shown.saturating_duration_since(now).as_secs_f64();
        self.lead.add(lead);
        let since_tick = seconds_between(tick_due, shown);
        let t = (since_tick - self.lead.get()) / self.tick.as_secs_f64();
        t.clamp(0.0, 1.0) as f32
    }

    /// When a frame started at `now` should reach the screen.
    fn display_time(&self, now: Instant) -> Instant {
        let ready = now + Duration::from_secs_f64(self.latency.get());
        match self.refreshed {
            Some(r) if self.fifo && !self.refresh.is_zero() => {
                // The first refresh of the grid the waits saw at or after `ready`.
                let since = ready.saturating_duration_since(r).as_secs_f64();
                let n = (since / self.refresh.as_secs_f64()).ceil();
                r + self.refresh.mul_f64(n)
            }
            _ => ready,
        }
    }

    /// A frame started at `start` waited `acquire` for the swap chain's image (the wait ending
    /// at `acquired`) and was presented at `presented`.
    pub fn presented(
        &mut self,
        start: Instant,
        acquire: Duration,
        acquired: Instant,
        presented: Instant,
    ) {
        self.latency
            .add(presented.saturating_duration_since(start).as_secs_f64());
        if self.fifo && acquire > BLOCKED {
            self.refreshed = Some(acquired);
        }
    }
}

/// `to - from` in seconds, negative when `to` comes first.
fn seconds_between(from: Instant, to: Instant) -> f64 {
    match to.checked_duration_since(from) {
        Some(d) => d.as_secs_f64(),
        None => -from.duration_since(to).as_secs_f64(),
    }
}

/// The refresh period of a display refreshing `millihertz` times a thousand seconds; 60 Hz when
/// the system doesn't say.
pub fn refresh_period(millihertz: Option<u32>) -> Duration {
    let hz = millihertz
        .filter(|&m| m >= 1000)
        .map_or(60.0, |m| f64::from(m) / 1000.0);
    Duration::from_secs_f64(1.0 / hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_nanos(16_666_667);

    fn ms(x: f64) -> Duration {
        Duration::from_secs_f64(x / 1000.0)
    }

    #[test]
    fn modes_parse_and_name_themselves() {
        assert_eq!(Mode::parse("off"), Ok(Mode::Off));
        assert_eq!(Mode::parse("interpolate"), Ok(Mode::Interpolate));
        assert_eq!(Mode::parse("extrapolate"), Ok(Mode::Extrapolate));
        assert!(Mode::parse("predict").is_err());
        for (m, _) in Mode::CHOICES {
            assert_eq!(Mode::parse(m.key()), Ok(m));
        }
        assert_eq!(Mode::default(), Mode::Off);
    }

    #[test]
    fn refresh_rates_from_the_system() {
        assert_eq!(refresh_period(None), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(refresh_period(Some(0)), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(
            refresh_period(Some(143_998)),
            Duration::from_secs_f64(1.0 / 143.998)
        );
    }

    /// Frames every refresh at `hz` for two seconds of ticks every 16.7 ms, presented `took` ms
    /// after they start, with the swap chain's wait ending at each refresh when `fifo`: each
    /// frame's (display time, t, tick due).
    fn run(hz: f64, took: f64, fifo: bool) -> Vec<(f64, f32, f64)> {
        let t0 = Instant::now();
        let refresh = Duration::from_secs_f64(1.0 / hz);
        let mut p = Pacer::new(TICK, refresh, fifo, t0);
        let mut out = Vec::new();
        let mut now = t0;
        while now < t0 + Duration::from_secs(2) {
            // The ticks run when due; the latest due by now is the one drawn.
            let n = (now - t0).as_nanos() / TICK.as_nanos();
            let tick_due = t0 + TICK * n as u32;
            let t = p.start(now, tick_due);
            let presented = now + ms(took);
            // FIFO: shown at the first refresh after it's presented; the next image comes then.
            let shown = if fifo {
                let k = ((presented - t0).as_secs_f64() / refresh.as_secs_f64()).ceil();
                t0 + refresh.mul_f64(k)
            } else {
                presented
            };
            let (acquire, acquired) = if fifo {
                (ms(1.0), shown)
            } else {
                (Duration::ZERO, now)
            };
            p.presented(now, acquire, acquired, presented);
            out.push(((shown - t0).as_secs_f64(), t, (tick_due - t0).as_secs_f64()));
            now = if fifo { shown } else { p.due() };
        }
        out
    }

    /// Where each frame shows the game, in seconds of game time: tick N-1's time plus t ticks.
    fn game_time(f: &(f64, f32, f64)) -> f64 {
        f.2 - TICK.as_secs_f64() + f64::from(f.1) * TICK.as_secs_f64()
    }

    /// After the averages settle, the game time each frame shows moves on as its display time
    /// does: even motion, at 240 Hz, 144 Hz and 50 Hz, FIFO or not.
    #[test]
    fn motion_follows_the_display_time() {
        for (hz, took, fifo) in [
            (240.0, 2.0, false),
            (144.0, 3.0, false),
            (144.0, 3.0, true),
            (50.0, 5.0, true),
            (165.0, 1.0, true),
        ] {
            let frames = run(hz, took, fifo);
            let settled = &frames[frames.len() / 4..];
            for w in settled.windows(2) {
                let shown = w[1].0 - w[0].0;
                let moved = game_time(&w[1]) - game_time(&w[0]);
                assert!(
                    (moved - shown).abs() < 0.0005,
                    "{hz} Hz, fifo {fifo}: shown {shown} apart but moved {moved}"
                );
            }
            assert!(settled.iter().all(|f| (0.0..=1.0).contains(&f.1)));
        }
    }

    #[test]
    fn a_stall_puts_the_cadence_back_on_now() {
        let t0 = Instant::now();
        let mut p = Pacer::new(TICK, ms(4.0), false, t0);
        p.start(t0, t0);
        assert_eq!(p.due(), t0 + ms(4.0));
        let late = t0 + ms(100.0);
        p.start(late, late);
        assert_eq!(p.due(), late);
    }
}
