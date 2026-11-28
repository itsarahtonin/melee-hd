//! Presentation feedback (docs/player.md, "Presentation feedback"): when each presented frame
//! actually reached the screen, and from those times whether the display is variable-refresh.
//!
//! **Sources.** Each OS has its own, reached through wgpu's surface where wgpu lets it:
//!
//! - **Windows, DX12** (`dxgi`): the swap chain's frame statistics
//!   (`IDXGISwapChain::GetFrameStatistics`), which name the last present that reached the screen
//!   and the refresh it showed at, with that refresh's time on the performance counter. The
//!   swap chain comes from wgpu's DX12 surface (`Surface::as_hal`). wgpu's Vulkan backend (the
//!   default here) has no way to it: `VK_KHR_present_wait` needs present IDs passed with each
//!   present, which wgpu doesn't pass, and `VK_GOOGLE_display_timing`, which it does pass, isn't
//!   offered by Windows drivers. There the frames' times stay the estimate.
//! - **Linux, Wayland** (`wayland`): the compositor's presentation-time protocol
//!   (`wp_presentation`): a feedback object asked for before each present comes back with the
//!   time the frame was shown and the output's refresh period, which is also the refresh grid
//!   the just-in-time tick lacked there. X11 has nothing comparable, and the NVIDIA driver on the
//!   worker offers no `VK_GOOGLE_display_timing`.
//! - **macOS** (`apple`): `CADisplayLink` on the window's view gives the refresh times (the
//!   grid); it says nothing of when a frame showed. `CAMetalDisplayLink` would also give the
//!   refreshes, but it hands out the layer's drawables, which wgpu's own present takes from the
//!   same small pool, so it would hold frames back.
//!
//! **What it changes.** The latency log's `shown_ms` is the measured time where there is one,
//! the estimate where not, and the log and F3's overlay say which. The display's refreshes
//! feed [`Vrr`], which answers whether frames wait for a refresh (fixed refresh: every frame
//! shows on the same grid) or show when they come (variable refresh: their times spread over
//! the refresh period), and `auto` takes that answer rather than guessing from the rate
//! ([`crate::jit::Timing::policy`]). Nothing here changes a tick's inputs or state, and the
//! margin's misses stay on the GPU's finishing time, so a measured time never adds latency to
//! the default path.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::jit::Grid;

#[cfg(target_os = "macos")]
mod apple;
#[cfg(windows)]
mod dxgi;
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
mod wayland;

/// A window's presentation feedback, from whichever source its system has.
pub struct Feedback {
    os: Option<Os>,
}

enum Os {
    #[cfg(windows)]
    Dxgi(dxgi::Stats),
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    Wayland(Box<wayland::Presentation>),
    #[cfg(target_os = "macos")]
    Link(apple::Link),
}

impl Feedback {
    /// The source for `surface`, presented on `adapter`, in `window`: none where the system
    /// has none (or `MELEE_PRESENT_FEEDBACK=0`).
    pub fn new(
        adapter: &wgpu::Adapter,
        surface: &wgpu::Surface<'static>,
        window: &winit::window::Window,
    ) -> Feedback {
        let off = std::env::var("MELEE_PRESENT_FEEDBACK").is_ok_and(|v| v == "0" || v == "off");
        let os = if off {
            None
        } else {
            Os::new(adapter, surface, window)
        };
        if let Some(o) = &os {
            eprintln!("presentation feedback: {}", o.name());
        }
        Feedback { os }
    }

    /// The source's name, for the log: None when frames' times are estimated.
    pub fn name(&self) -> Option<&'static str> {
        self.os.as_ref().map(Os::name)
    }

    /// Whether the source says when frames reached the screen (not only when refreshes come).
    pub fn measures_frames(&self) -> bool {
        self.os.as_ref().is_some_and(Os::measures_frames)
    }

    /// The window's present number `n` is about to be presented.
    pub fn presenting(&mut self, n: u64) {
        match &mut self.os {
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Some(Os::Wayland(w)) => w.presenting(n),
            _ => {
                let _ = n;
            }
        }
    }

    /// The window's present number `n` was presented on `surface`.
    pub fn presented(&mut self, surface: &wgpu::Surface<'static>, n: u64) {
        match &mut self.os {
            #[cfg(windows)]
            Some(Os::Dxgi(s)) => s.presented(surface, n),
            _ => {
                let _ = (surface, n);
            }
        }
    }

    /// The presents that reached the screen since the last call: each one's number, when it
    /// showed, and whether that time is the refresh it showed at exactly (else one worked out
    /// from a later refresh, which only a fixed-refresh display allows: `period` is the
    /// refreshes' when they're known to be fixed).
    pub fn poll(
        &mut self,
        surface: &wgpu::Surface<'static>,
        period: Option<Duration>,
    ) -> Vec<Displayed> {
        match &mut self.os {
            #[cfg(windows)]
            Some(Os::Dxgi(s)) => s.poll(surface, period),
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Some(Os::Wayland(w)) => {
                let _ = (surface, period);
                w.poll()
            }
            _ => {
                let _ = (surface, period);
                Vec::new()
            }
        }
    }

    /// The display's refreshes, where the source knows them (Wayland's feedback, macOS's
    /// display link; on Windows the compositor's timing gives them, [`crate::timing`]).
    pub fn grid(&self) -> Option<Grid> {
        match &self.os {
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Some(Os::Wayland(w)) => w.grid(),
            #[cfg(target_os = "macos")]
            Some(Os::Link(l)) => l.grid(),
            _ => None,
        }
    }
}

impl Os {
    #[allow(unused_variables)]
    fn new(
        adapter: &wgpu::Adapter,
        surface: &wgpu::Surface<'static>,
        window: &winit::window::Window,
    ) -> Option<Os> {
        #[cfg(windows)]
        {
            dxgi::Stats::new(adapter, surface).map(Os::Dxgi)
        }
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        {
            wayland::Presentation::new(window).map(|w| Os::Wayland(Box::new(w)))
        }
        #[cfg(target_os = "macos")]
        {
            apple::Link::new(window).map(Os::Link)
        }
        #[cfg(not(any(windows, unix)))]
        {
            None
        }
    }

    fn name(&self) -> &'static str {
        match *self {
            #[cfg(windows)]
            Os::Dxgi(_) => "DXGI frame statistics",
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Os::Wayland(_) => "Wayland presentation time",
            #[cfg(target_os = "macos")]
            Os::Link(_) => "CADisplayLink (refreshes only)",
        }
    }

    fn measures_frames(&self) -> bool {
        match *self {
            #[cfg(windows)]
            Os::Dxgi(_) => true,
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Os::Wayland(_) => true,
            #[cfg(target_os = "macos")]
            Os::Link(_) => false,
        }
    }
}

/// A present that reached the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Displayed {
    /// The window's present number ([`crate::present::Presented::number`]).
    pub number: u64,
    /// When it showed.
    pub at: Instant,
    /// `at` is the refresh it showed at, read as it came, not worked out from a later one: the
    /// only times [`Vrr`] takes.
    pub exact: bool,
}

/// Displayed times kept for the answer: a few seconds of frames.
const VRR_WINDOW: usize = 240;
/// Displayed times before there's an answer.
const VRR_MIN: usize = 120;
/// How far from the refreshes' common phase a displayed time may be and still be on the grid:
/// refresh times are the hardware's (microseconds); a frame shown when it came lands anywhere.
const ON_GRID: Duration = Duration::from_micros(500);
/// Off the grid at most this fraction: fixed refresh. At least [`VRR_SPREAD`]: variable.
const VRR_FIXED: f64 = 0.05;
const VRR_SPREAD: f64 = 0.3;
/// The answer is worked out again every this many times.
const VRR_EVERY: usize = 30;

/// Whether the display is variable-refresh, from when frames reached the screen.
///
/// A fixed-refresh display shows every frame at one of its refreshes, so the displayed times
/// all share one phase against the refresh period. A variable-refresh display (G-SYNC,
/// FreeSync) shows a frame when it comes, so frames that come at the ticks' 60 a second, or
/// whenever the GPU finishes them, land all over the period. The answer is the fraction of the
/// recent displayed times further than [`ON_GRID`] from their common phase (a circular mean):
/// at most [`VRR_FIXED`] is fixed, at least [`VRR_SPREAD`] variable, and between keeps the
/// answer it had. Frames that come once a refresh exactly show on a grid on either display,
/// which is what they then are.
#[derive(Debug, Default)]
pub struct Vrr {
    period: Option<Duration>,
    shown: VecDeque<Instant>,
    added: usize,
    answer: Option<bool>,
}

impl Vrr {
    /// The refresh period the displayed times are compared against (the display's nominal
    /// one, its highest on a variable-refresh display); a new one starts again.
    pub fn set_period(&mut self, period: Option<Duration>) {
        let same = match (self.period, period) {
            (Some(a), Some(b)) => (a.as_secs_f64() / b.as_secs_f64() - 1.0).abs() < 0.001 || a == b,
            (None, None) => true,
            _ => false,
        };
        if !same {
            *self = Vrr {
                period,
                ..Vrr::default()
            };
        }
    }

    /// A frame reached the screen at `at` (a time read as it came, never worked out from a
    /// period, which would put it on a grid by construction).
    pub fn add(&mut self, at: Instant) {
        if self.shown.len() == VRR_WINDOW {
            self.shown.pop_front();
        }
        self.shown.push_back(at);
        self.added += 1;
        if self.added.is_multiple_of(VRR_EVERY) {
            self.decide();
        }
    }

    /// True for a variable-refresh display, false for a fixed one, None until enough frames
    /// have shown.
    pub fn answer(&self) -> Option<bool> {
        self.answer
    }

    /// The fraction of the recent displayed times off the refreshes' common phase.
    pub fn spread(&self) -> Option<f64> {
        let p = self.period?.as_secs_f64();
        if self.shown.len() < VRR_MIN || p <= 0.0 {
            return None;
        }
        let r = *self.shown.front()?;
        let angle = |t: Instant| {
            let s = crate::jit::nanos_between(r, t) as f64 / 1e9;
            (s / p).rem_euclid(1.0) * std::f64::consts::TAU
        };
        let (mut x, mut y) = (0.0, 0.0);
        for &t in &self.shown {
            let a = angle(t);
            x += a.cos();
            y += a.sin();
        }
        let mean = y.atan2(x);
        let on = ON_GRID.as_secs_f64();
        let off = self
            .shown
            .iter()
            .filter(|&&t| {
                let d = (angle(t) - mean + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                (d / std::f64::consts::TAU * p).abs() > on
            })
            .count();
        Some(off as f64 / self.shown.len() as f64)
    }

    fn decide(&mut self) {
        match self.spread() {
            Some(s) if s <= VRR_FIXED => self.answer = Some(false),
            Some(s) if s >= VRR_SPREAD => self.answer = Some(true),
            _ => {}
        }
    }

    /// What the log and the overlay say about the display.
    pub fn describe(&self) -> &'static str {
        match self.answer {
            Some(true) => "variable refresh (measured)",
            Some(false) => "fixed refresh (measured)",
            None => "refresh not measured",
        }
    }
}

/// The time a clock's reading `then` stands for, on the clock `Instant` reads, given the same
/// clock's reading `now` taken with `at` (seconds on any clock that counts like Instant's).
pub fn instant_of(then: f64, now: f64, at: Instant) -> Option<Instant> {
    let d = now - then;
    if !d.is_finite() || d.abs() > 10.0 {
        return None;
    }
    if d >= 0.0 {
        at.checked_sub(Duration::from_secs_f64(d))
    } else {
        Some(at + Duration::from_secs_f64(-d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(x: f64) -> Duration {
        Duration::from_secs_f64(x / 1000.0)
    }

    /// Feeds `n` displayed times from `times` and returns the answer.
    fn answer(period: f64, times: impl Iterator<Item = Instant>) -> (Option<bool>, f64) {
        let mut v = Vrr::default();
        v.set_period(Some(ms(period)));
        for t in times.take(VRR_WINDOW) {
            v.add(t);
        }
        (v.answer(), v.spread().unwrap())
    }

    /// A fixed 144 Hz display shows frames at its refreshes, whichever refreshes they take
    /// (ticks at 60 a second take every second or third); a variable-refresh one shows each
    /// when it comes, here at the ticks' 60 a second with the GPU's finishing time spread.
    #[test]
    fn displayed_times_tell_fixed_from_variable_refresh() {
        let t0 = Instant::now();
        let p = 1000.0 / 144.0;
        let tick = 1000.0 / 60.0;
        // Fixed: the first refresh after each tick's frame is ready, with timestamps a few
        // microseconds off.
        let fixed = (0..).map(|i| {
            let ready = tick * f64::from(i) + 4.0 + 0.7 * f64::from(i % 5) / 5.0;
            let k = (ready / p).ceil();
            t0 + ms(k * p + 0.002 * f64::from(i % 3))
        });
        let (a, s) = answer(p, fixed);
        assert_eq!(a, Some(false), "spread {s}");
        // Variable: shown when ready.
        let vrr = (0..).map(|i| t0 + ms(tick * f64::from(i) + 4.0 + 0.7 * f64::from(i % 5) / 5.0));
        let (a, s) = answer(p, vrr);
        assert_eq!(a, Some(true), "spread {s}");
        // At 60 Hz too, where the ticks' rate is the refreshes': frames shown when ready spread
        // by the GPU's time (2 ms here) are off the grid, a fixed display's aren't.
        let p = 1000.0 / 60.0;
        let vrr = (0..).map(|i| t0 + ms(tick * f64::from(i) + 4.0 + 2.0 * f64::from(i % 7) / 7.0));
        assert_eq!(answer(p, vrr).0, Some(true));
        let fixed = (0..).map(|i| t0 + ms(p * f64::from(i) + 0.001 * f64::from(i % 2)));
        assert_eq!(answer(p, fixed).0, Some(false));
    }

    /// Too few frames give no answer; between the two bounds the answer stays; a new period
    /// starts again.
    #[test]
    fn the_answer_waits_for_frames_and_holds_between_its_bounds() {
        let t0 = Instant::now();
        let p = ms(10.0);
        let mut v = Vrr::default();
        v.set_period(Some(p));
        for i in 0..VRR_MIN as u32 - 1 {
            v.add(t0 + p * i);
        }
        assert_eq!(v.answer(), None);
        for i in 0..VRR_WINDOW as u32 {
            v.add(t0 + p * (i + 200));
        }
        assert_eq!(v.answer(), Some(false));
        // One in six frames off the grid (between 5% and 30%): still fixed.
        for i in 0..VRR_WINDOW as u32 {
            let off = if i % 6 == 0 { ms(3.0) } else { Duration::ZERO };
            v.add(t0 + p * (i + 500) + off);
        }
        assert_eq!(v.answer(), Some(false));
        let s = v.spread().unwrap();
        assert!((0.1..0.25).contains(&s), "{s}");
        v.set_period(Some(ms(10.0001)));
        assert_eq!(v.answer(), Some(false), "within 0.1%: the same period");
        v.set_period(Some(ms(8.0)));
        assert_eq!(v.answer(), None);
        v.set_period(None);
        assert_eq!(v.spread(), None);
    }

    /// The phase is circular: a grid whose refreshes sit at the period's wrap (half of them
    /// just before it, half just after) is one phase, not two.
    #[test]
    fn a_grid_at_the_period_s_wrap_is_one_phase() {
        let t0 = Instant::now();
        let p = 1000.0 / 144.0;
        let times = (0..).map(|i| {
            let jitter = if i % 2 == 0 { -0.1 } else { 0.1 };
            t0 + ms(p * (f64::from(i) + 1.0) + jitter)
        });
        assert_eq!(answer(p, times).0, Some(false));
    }

    #[test]
    fn clock_readings_become_instants() {
        let at = Instant::now() + Duration::from_secs(100);
        assert_eq!(instant_of(5.0, 7.5, at), Some(at - ms(2500.0)));
        assert_eq!(instant_of(8.0, 7.5, at), Some(at + ms(500.0)));
        assert_eq!(instant_of(0.0, 50.0, at), None, "a stale reading");
        assert_eq!(instant_of(f64::NAN, 1.0, at), None);
    }
}
