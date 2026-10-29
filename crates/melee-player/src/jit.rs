//! The just-in-time tick (docs/player.md, "Just-in-time ticks"): each tick starts as late as
//! it can and still have its frame ready for the refresh that will show it, so the controllers
//! it reads are as fresh as they can be when the frame reaches the screen.
//!
//! **Melee's clock.** Tick n is due at `anchor + n * tick`, 60 a second, as the player has
//! always run them. The scheduler never moves a tick before its due time nor past the next
//! tick's, so ticks stay one for one with that clock however the display runs; it only chooses
//! where in its slot a tick starts. A stall of more than four ticks puts the clock back on now
//! (as the fixed timer did), and a tick whose successor is due already runs at once.
//!
//! **Aligned** (a fixed-refresh display presenting FIFO or mailbox): the tick's frame is meant
//! for the first refresh D at or after `due + lead` that no earlier frame took, and the tick
//! starts at `D - lead`, where lead is the measured cost from a tick's start to the GPU having
//! finished its frame, at its 99th percentile over the last few seconds, plus a margin. Input
//! read to refresh is then `lead` every tick, where a timer that knows nothing of the display
//! adds anything from nothing to a refresh. When the refresh D would put the tick's start at or
//! past the next tick's due time (the display runs slower than the ticks, as 59.94 Hz against
//! 60 does once in 1000 ticks, or a late frame holds the refresh this tick would take), the
//! tick runs at its due time without a frame of its own: the display can't show every tick
//! then, and the tick's state shows in the next frame. A frame the GPU finishes after its
//! refresh shows a refresh late; the next tick's frame takes a later refresh, never the same.
//!
//! **Cadence** (a variable-refresh display, which waits for frames, or presenting immediately,
//! or no refresh times known): ticks start at their due times, precisely, and frames are
//! presented as soon as they're ready.
//!
//! **The margin** adapts: a missed refresh widens it by a step (up to [`MAX_MARGIN`]; at most
//! once in [`WIDEN_EVERY`] frames, as one stall misses several in a row, and not before the
//! costs' percentile is measured), and every [`NARROW_AFTER`] frames on time narrow it by a
//! smaller step, back down to the margin asked for (`--jit-margin`, the trade between latency
//! and the odd late frame).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How ticks are timed (`--tick-timing`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Timing {
    /// Just in time, aligned to the refreshes when the display's rate is a multiple of the
    /// ticks' (60, 120, 240 Hz: even at that rate whether or not the display waits for frames),
    /// else at the ticks' cadence (a faster display that isn't a multiple is taken to be
    /// variable-refresh). Experimental.
    Auto,
    /// Just in time, aligned to the refreshes: a fixed-refresh display.
    Refresh,
    /// Just in time at the ticks' cadence, frames presented as soon as they're ready: a
    /// variable-refresh display (G-SYNC, FreeSync), which shows a frame when it comes.
    Vrr,
    /// The fixed 60 Hz timer: the event loop's wake-up, nothing aligned. The default while the
    /// just-in-time ones are experimental.
    #[default]
    Timer,
}

impl Timing {
    /// The settings panel's choices, with their names.
    pub const CHOICES: [(Timing, &str); 4] = [
        (Timing::Timer, "Fixed timer"),
        (Timing::Auto, "Just in time, auto (experimental)"),
        (
            Timing::Refresh,
            "Just in time, fixed refresh (experimental)",
        ),
        (Timing::Vrr, "Just in time, variable refresh (experimental)"),
    ];

    pub fn parse(v: &str) -> Result<Timing, String> {
        match v {
            "auto" | "jit" => Ok(Timing::Auto),
            "refresh" => Ok(Timing::Refresh),
            "vrr" => Ok(Timing::Vrr),
            "timer" | "off" => Ok(Timing::Timer),
            _ => Err(format!("--tick-timing auto|refresh|vrr|timer, not {v}")),
        }
    }

    /// The name the command line and the settings file use.
    pub fn key(self) -> &'static str {
        match self {
            Timing::Auto => "auto",
            Timing::Refresh => "refresh",
            Timing::Vrr => "vrr",
            Timing::Timer => "timer",
        }
    }

    /// The name the settings panel shows.
    pub fn name(self) -> &'static str {
        Timing::CHOICES
            .iter()
            .find(|(t, _)| *t == self)
            .map_or("?", |(_, n)| n)
    }

    /// How ticks are placed for a display refreshing at `grid` (when its refresh times are
    /// known), presenting immediately (tearing) when `immediate`, with ticks of `tick`.
    pub fn policy(self, grid: Option<Grid>, immediate: bool, tick: Duration) -> Policy {
        match (self, grid) {
            (Timing::Timer, _) => Policy::Timer,
            (_, None) | (Timing::Vrr, _) => Policy::Cadence,
            _ if immediate => Policy::Cadence,
            (Timing::Refresh, Some(_)) => Policy::Aligned,
            (Timing::Auto, Some(g)) if multiple(g.period, tick) => Policy::Aligned,
            (Timing::Auto, Some(_)) => Policy::Cadence,
        }
    }
}

/// Whether `period` divides `tick` a whole number of times, within half a percent (59.94 Hz
/// and 119.88 Hz against 60 ticks a second included).
pub fn multiple(period: Duration, tick: Duration) -> bool {
    if period.is_zero() {
        return false;
    }
    let r = tick.as_secs_f64() / period.as_secs_f64();
    r >= 0.995 && (r - r.round()).abs() <= 0.005 * r
}

/// Where ticks start.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Policy {
    /// At their due times, as the event loop wakes (the old fixed timer).
    Timer,
    /// At their due times, waited for precisely; frames presented when ready.
    Cadence,
    /// Just before the refresh their frame is meant for.
    Aligned,
}

impl Policy {
    pub fn name(self) -> &'static str {
        match self {
            Policy::Timer => "fixed timer",
            Policy::Cadence => "just in time at the ticks' cadence",
            Policy::Aligned => "just in time, aligned to the refreshes",
        }
    }
}

/// The display's refreshes: one at `phase`, then every `period`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid {
    pub phase: Instant,
    pub period: Duration,
}

impl Grid {
    /// The first refresh at or after `t`.
    pub fn at_or_after(&self, t: Instant) -> Instant {
        let p = self.period.as_nanos().max(1) as i128;
        let d = nanos_between(self.phase, t);
        let k = d.div_euclid(p) + i128::from(d.rem_euclid(p) != 0);
        offset(self.phase, k * p)
    }
}

/// `to - from` in nanoseconds, negative when `to` comes first.
pub fn nanos_between(from: Instant, to: Instant) -> i128 {
    match to.checked_duration_since(from) {
        Some(d) => d.as_nanos() as i128,
        None => -(from.duration_since(to).as_nanos() as i128),
    }
}

/// `base` moved by `nanos`, either way.
fn offset(base: Instant, nanos: i128) -> Instant {
    if nanos >= 0 {
        base + Duration::from_nanos(nanos as u64)
    } else {
        base.checked_sub(Duration::from_nanos((-nanos) as u64))
            .unwrap_or(base)
    }
}

/// The refresh a frame meant for `target` and ready at `ready` shows at: `target` if it was ready
/// in time, else the first refresh after. The refresh times are read afresh as frames go, so
/// `target` may lie a little off the grid's refreshes now.
pub fn shown_at(grid: Grid, target: Instant, ready: Instant) -> Instant {
    let at = grid.at_or_after(ready + LATCH);
    if at <= target + grid.period / 2 {
        target
    } else {
        at
    }
}

/// The margin asked for when none is (`--jit-margin`).
pub const DEFAULT_MARGIN: Duration = Duration::from_millis(1);
/// The widest the margin grows after missed refreshes: past the costs' 99th percentile, a wider
/// one buys little but latency.
pub const MAX_MARGIN: Duration = Duration::from_millis(4);
/// What a missed refresh adds to the margin.
pub const MISS_STEP: Duration = Duration::from_millis(1);
/// Frames between two widenings: a second of ticks.
pub const WIDEN_EVERY: u32 = 60;
/// What a run of frames on time takes back.
pub const NARROW_STEP: Duration = Duration::from_micros(250);
/// Frames on time in a row before the margin narrows (ten seconds of ticks).
pub const NARROW_AFTER: u32 = 600;
/// From the GPU finishing a frame to its flip being ready for a refresh, which the miss check
/// allows for (PresentMon measured about 0.3 ms here).
pub const LATCH: Duration = Duration::from_micros(300);
/// Costs kept for the percentile: five seconds of ticks.
const WINDOW: usize = 300;
/// Costs measured before the percentile is trusted; until then the lead is half a tick.
const WARM_UP: usize = 20;

/// The recent costs of something (a tick and its frame, a frame, a tick alone) and the margin
/// added to their 99th percentile, which misses widen and runs on time narrow.
#[derive(Clone, Debug)]
pub struct Lead {
    costs: VecDeque<Duration>,
    sorted: Vec<Duration>,
    /// The margin asked for, and the margin now.
    base: Duration,
    margin: Duration,
    /// Frames on time since the margin last changed, and frames since it last widened.
    on_time: u32,
    since_widen: u32,
    /// Until enough costs are measured.
    default: Duration,
}

impl Lead {
    /// A lead with `margin` over the costs' percentile, `default` until costs are measured.
    pub fn new(margin: Duration, default: Duration) -> Lead {
        Lead {
            costs: VecDeque::with_capacity(WINDOW),
            sorted: Vec::with_capacity(WINDOW),
            base: margin,
            margin,
            on_time: 0,
            since_widen: WIDEN_EVERY,
            default,
        }
    }

    pub fn add(&mut self, cost: Duration) {
        if self.costs.len() == WINDOW
            && let Some(old) = self.costs.pop_front()
            && let Ok(i) = self.sorted.binary_search(&old)
        {
            self.sorted.remove(i);
        }
        self.costs.push_back(cost);
        let i = self.sorted.partition_point(|&c| c <= cost);
        self.sorted.insert(i, cost);
    }

    /// The costs' 99th percentile, once enough are measured.
    pub fn percentile(&self) -> Option<Duration> {
        let n = self.sorted.len();
        (n >= WARM_UP).then(|| self.sorted[((n as f64 * 0.99).ceil() as usize).clamp(1, n) - 1])
    }

    pub fn margin(&self) -> Duration {
        self.margin
    }

    /// The costs' median, once enough are measured.
    pub fn median(&self) -> Option<Duration> {
        let n = self.sorted.len();
        (n >= WARM_UP).then(|| self.sorted[n / 2])
    }

    /// Whether starting just before a refresh (input to refresh: this lead) beats starting at
    /// the due time and waiting for the next refresh (on average the median cost and half a
    /// refresh), with half a millisecond of hysteresis either way from `aligned`, the choice
    /// now. Until the costs are measured, it's taken to.
    pub fn pays(&self, period: Duration, aligned: bool) -> bool {
        let Some(median) = self.median() else {
            return true;
        };
        let waiting = median + period / 2;
        let hysteresis = Duration::from_micros(500);
        if aligned {
            self.get() < waiting + hysteresis
        } else {
            self.get() + hysteresis < waiting
        }
    }

    /// The percentile and the margin.
    pub fn get(&self) -> Duration {
        self.percentile().unwrap_or(self.default) + self.margin
    }

    /// A deadline was missed: the margin widens, unless it widened within the last second or
    /// the costs aren't measured yet.
    pub fn missed(&mut self) {
        self.on_time = 0;
        if self.since_widen >= WIDEN_EVERY && self.percentile().is_some() {
            self.margin = (self.margin + MISS_STEP).min(MAX_MARGIN.max(self.base));
            self.since_widen = 0;
        } else {
            self.since_widen += 1;
        }
    }

    /// A deadline was met: after a run of them, the margin narrows toward the one asked for.
    pub fn met(&mut self) {
        self.since_widen = self.since_widen.saturating_add(1);
        self.on_time += 1;
        if self.on_time >= NARROW_AFTER {
            self.on_time = 0;
            self.margin = self.margin.saturating_sub(NARROW_STEP).max(self.base);
        }
    }
}

/// When the next tick starts, and what it's for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// When it starts.
    pub start: Instant,
    /// When Melee's clock has it due.
    pub due: Instant,
    /// The refresh its frame is meant for (aligned).
    pub target: Option<Instant>,
    /// Whether its frame is drawn: not when the next tick is due already, or (aligned) when the
    /// display has no refresh for it before the next tick's.
    pub render: bool,
}

/// The counts the latency log and the statistics line report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Frames meant for a refresh, and those the GPU finished too late for it.
    pub targeted: u64,
    pub missed: u64,
    /// Ticks run without a frame of their own: the next tick was due already, or the display
    /// had no refresh for it.
    pub unshown: u64,
    /// Times Melee's clock was put back on now after a stall.
    pub stalls: u64,
}

/// A stall longer than this many ticks puts Melee's clock back on now.
const STALL_TICKS: u32 = 4;

/// Places ticks on Melee's clock, just in time for their refreshes.
#[derive(Clone, Debug)]
pub struct Scheduler {
    tick: Duration,
    /// Tick `n0` was due at `anchor`.
    anchor: Instant,
    n0: u64,
    n: u64,
    /// From a tick's start to the GPU having finished its frame.
    pub lead: Lead,
    /// The refresh the last frame shows at (or is meant to), so the next takes a later one.
    last_slot: Option<Instant>,
    pub counts: Counts,
}

impl Scheduler {
    /// Ticks of `tick`, the first due at `now`, with `margin` over the costs' percentile.
    pub fn new(tick: Duration, margin: Duration, now: Instant) -> Scheduler {
        Scheduler {
            tick,
            anchor: now,
            n0: 0,
            n: 0,
            lead: Lead::new(margin, tick / 2),
            last_slot: None,
            counts: Counts::default(),
        }
    }

    pub fn tick(&self) -> Duration {
        self.tick
    }

    /// Ticks of `tick` from the next one due on (a game sped up shortens Melee's tick).
    pub fn set_tick(&mut self, tick: Duration) {
        self.anchor = self.due();
        self.n0 = self.n;
        self.tick = tick;
    }

    /// When Melee's clock has tick `n` due.
    fn due_of(&self, n: u64) -> Instant {
        self.anchor + self.tick * (n - self.n0) as u32
    }

    /// When the next tick is due.
    pub fn due(&self) -> Instant {
        self.due_of(self.n)
    }

    /// Puts Melee's clock back on `now` for the next tick, if it's more than a stall behind.
    fn catch_up(&mut self, now: Instant) {
        if now > self.due() + self.tick * STALL_TICKS {
            // Time is dropped from the clock, not ticks from the game: the next is due now.
            self.anchor = now;
            self.n0 = self.n;
            self.last_slot = None;
            self.counts.stalls += 1;
        }
    }

    /// Plans the next tick at `now` under `policy`, the display refreshing at `grid`.
    pub fn plan(&mut self, now: Instant, policy: Policy, grid: Option<Grid>) -> Plan {
        self.catch_up(now);
        let due = self.due();
        let next_due = self.due_of(self.n + 1);
        let at_due = |render| Plan {
            start: due,
            due,
            target: None,
            render,
        };
        // Behind Melee's clock: this tick runs at once, and the latest due draws the frame (the
        // fixed timer drew every one).
        if now >= next_due {
            return at_due(policy == Policy::Timer);
        }
        let grid = match (policy, grid) {
            (Policy::Aligned, Some(g)) if !g.period.is_zero() => g,
            _ => return at_due(true),
        };
        let lead = self.lead.get().min(self.tick);
        let mut earliest = due + lead;
        if let Some(s) = self.last_slot {
            earliest = earliest.max(s + grid.period / 2);
        }
        let target = grid.at_or_after(earliest);
        let start = target.checked_sub(lead).unwrap_or(due).max(due);
        if start >= next_due {
            return at_due(false);
        }
        Plan {
            start,
            due,
            target: Some(target),
            render: true,
        }
    }

    /// Plans the next tick at `now` when frames are drawn on their own schedule (Interpolate,
    /// Extrapolate): it starts just before the first frame at least its lead after its due
    /// time, frames starting at `next_frame` and every `frame_period` after.
    pub fn plan_before_frames(
        &mut self,
        now: Instant,
        policy: Policy,
        next_frame: Instant,
        frame_period: Duration,
    ) -> Plan {
        self.catch_up(now);
        let due = self.due();
        let next_due = self.due_of(self.n + 1);
        let at_due = Plan {
            start: due,
            due,
            target: None,
            render: true,
        };
        if policy == Policy::Timer || now >= next_due || frame_period.is_zero() {
            return at_due;
        }
        let lead = self.lead.get().min(self.tick);
        let frames = Grid {
            phase: next_frame,
            period: frame_period,
        };
        let frame = frames.at_or_after(due + lead);
        let start = frame.checked_sub(lead).unwrap_or(due).max(due);
        if start >= next_due {
            return at_due;
        }
        Plan {
            start,
            due,
            target: Some(frame),
            render: true,
        }
    }

    /// The planned tick ran.
    pub fn ran(&mut self, plan: &Plan) {
        self.n += 1;
        if !plan.render {
            self.counts.unshown += 1;
        }
        if let Some(t) = plan.target
            && plan.render
        {
            self.last_slot = Some(self.last_slot.map_or(t, |s| s.max(t)));
        }
    }

    /// A tick that started at `start`, its frame meant for `target`, had the frame finished by
    /// the GPU at `ready`, the display refreshing at `grid`: the cost is kept, and a refresh
    /// missed widens the margin. True when it missed.
    pub fn ready(
        &mut self,
        start: Instant,
        target: Option<Instant>,
        ready: Instant,
        grid: Option<Grid>,
    ) -> Option<bool> {
        self.lead.add(ready.saturating_duration_since(start));
        let (target, grid) = (target?, grid?);
        self.counts.targeted += 1;
        let shown = shown_at(grid, target, ready);
        if shown > target {
            self.counts.missed += 1;
            self.lead.missed();
            // The refresh it shows at is taken too.
            self.last_slot = Some(self.last_slot.map_or(shown, |s| s.max(shown)));
            Some(true)
        } else {
            self.lead.met();
            Some(false)
        }
    }

    /// A tick alone (Interpolate, Extrapolate) that started at `start` was done at `done`, its
    /// frame starting at `frame`: the cost is kept, and a frame it held up widens the margin.
    pub fn tick_done(&mut self, start: Instant, done: Instant, frame: Option<Instant>) {
        self.lead.add(done.saturating_duration_since(start));
        if let Some(f) = frame {
            self.counts.targeted += 1;
            if done > f {
                self.counts.missed += 1;
                self.lead.missed();
            } else {
                self.lead.met();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_nanos(16_666_667);

    fn ms(x: f64) -> Duration {
        Duration::from_secs_f64(x / 1000.0)
    }

    fn secs(from: Instant, to: Instant) -> f64 {
        nanos_between(from, to) as f64 / 1e9
    }

    /// A deterministic stream of costs: mostly `base` with jitter, and a spike now and then.
    struct Costs {
        state: u64,
        base: f64,
        jitter: f64,
        spike_every: u64,
        spike: f64,
        n: u64,
    }

    impl Costs {
        fn new(base: f64, jitter: f64, spike_every: u64, spike: f64) -> Costs {
            Costs {
                state: 0x9E37_79B9_7F4A_7C15,
                base,
                jitter,
                spike_every,
                spike,
                n: 0,
            }
        }

        fn next(&mut self) -> Duration {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            self.n += 1;
            let u = (self.state >> 11) as f64 / (1u64 << 53) as f64;
            let mut c = self.base + self.jitter * u;
            if self.spike_every > 0 && self.n.is_multiple_of(self.spike_every) {
                c += self.spike;
            }
            ms(c)
        }
    }

    /// What a simulated run gave.
    struct Run {
        /// Each tick's (due, start, rendered).
        ticks: Vec<(Instant, Instant, bool)>,
        /// Each shown frame's (input read, refresh it shows at), and the frames a newer one
        /// replaced before they showed.
        shown: Vec<(Instant, Instant)>,
        replaced: u32,
        counts: Counts,
        t0: Instant,
        margin: Duration,
    }

    /// Runs `seconds` of ticks against a display refreshing every `refresh` (phase `phase` ms),
    /// under `policy`, each tick's frame costing what `costs` gives; mailbox: a frame shows at
    /// the first refresh after it's ready, unless a newer frame is ready for that refresh.
    fn simulate(
        policy: Policy,
        refresh: Duration,
        phase: f64,
        seconds: f64,
        mut costs: Costs,
    ) -> Run {
        let t0 = Instant::now();
        let grid = Grid {
            phase: t0 + ms(phase),
            period: refresh,
        };
        let mut s = Scheduler::new(TICK, DEFAULT_MARGIN, t0);
        let mut now = t0;
        let mut ticks = Vec::new();
        let mut shown: Vec<(Instant, Instant)> = Vec::new();
        let mut replaced = 0;
        while secs(t0, now) < seconds {
            let p = s.plan(now, policy, Some(grid));
            // The loop wakes at the start (a fake clock: precisely).
            now = now.max(p.start);
            let start = now;
            s.ran(&p);
            let ready = start + costs.next();
            if p.render {
                s.ready(start, p.target, ready, Some(grid));
                let at = grid.at_or_after(ready + LATCH);
                if shown.last().is_some_and(|l| l.1 == at) {
                    shown.pop();
                    replaced += 1;
                }
                shown.push((start, at));
            }
            ticks.push((p.due, start, p.render));
            // The next tick can't start before this one's frame is presented.
            now = now.max(ready);
        }
        Run {
            ticks,
            shown,
            replaced,
            counts: s.counts,
            t0,
            margin: s.lead.margin(),
        }
    }

    fn mean_latency(r: &Run) -> f64 {
        let v: Vec<f64> = r.shown.iter().map(|(i, d)| secs(*i, *d) * 1000.0).collect();
        v.iter().sum::<f64>() / v.len() as f64
    }

    #[test]
    fn timings_parse_and_name_themselves() {
        for (t, _) in Timing::CHOICES {
            assert_eq!(Timing::parse(t.key()), Ok(t));
        }
        assert_eq!(Timing::parse("off"), Ok(Timing::Timer));
        assert!(Timing::parse("sometimes").is_err());
        assert_eq!(Timing::default(), Timing::Timer);
    }

    #[test]
    fn policies_follow_the_display() {
        let g = |hz: f64| {
            Some(Grid {
                phase: Instant::now(),
                period: Duration::from_secs_f64(1.0 / hz),
            })
        };
        // A multiple of the ticks' rate aligns even when the display might wait for frames.
        for hz in [60.0, 59.94, 60.05, 120.0, 119.88, 240.0] {
            assert_eq!(
                Timing::Auto.policy(g(hz), false, TICK),
                Policy::Aligned,
                "{hz}"
            );
        }
        for hz in [144.0, 143.998, 165.0, 75.0, 50.0] {
            assert_eq!(
                Timing::Auto.policy(g(hz), false, TICK),
                Policy::Cadence,
                "{hz}"
            );
            assert_eq!(Timing::Refresh.policy(g(hz), false, TICK), Policy::Aligned);
        }
        assert_eq!(Timing::Refresh.policy(None, false, TICK), Policy::Cadence);
        assert_eq!(Timing::Refresh.policy(g(60.0), true, TICK), Policy::Cadence);
        assert_eq!(Timing::Vrr.policy(g(60.0), false, TICK), Policy::Cadence);
        assert_eq!(Timing::Timer.policy(g(60.0), false, TICK), Policy::Timer);
    }

    #[test]
    fn the_grid_finds_the_next_refresh_either_side_of_its_phase() {
        let t0 = Instant::now() + Duration::from_secs(10);
        let g = Grid {
            phase: t0,
            period: ms(10.0),
        };
        assert_eq!(g.at_or_after(t0), t0);
        assert_eq!(g.at_or_after(t0 + ms(0.001)), t0 + ms(10.0));
        assert_eq!(g.at_or_after(t0 + ms(25.0)), t0 + ms(30.0));
        assert_eq!(g.at_or_after(t0 - ms(25.0)), t0 - ms(20.0));
        assert_eq!(g.at_or_after(t0 - ms(30.0)), t0 - ms(30.0));
    }

    #[test]
    fn a_frame_shows_at_its_refresh_if_ready_in_time() {
        let t0 = Instant::now();
        let g = Grid {
            phase: t0,
            period: ms(10.0),
        };
        // A target read off an earlier grid, a few microseconds from this one's refresh.
        let target = t0 + ms(20.0) - Duration::from_micros(5);
        assert_eq!(shown_at(g, target, t0 + ms(15.0)), target);
        assert_eq!(shown_at(g, target, t0 + ms(19.8)), t0 + ms(30.0));
        assert_eq!(shown_at(g, target, t0 + ms(31.0)), t0 + ms(40.0));
    }

    #[test]
    fn the_lead_is_the_percentile_and_the_margin() {
        let mut l = Lead::new(ms(1.0), ms(8.0));
        assert_eq!(l.get(), ms(9.0));
        for i in 0..200 {
            l.add(ms(if i % 60 == 59 { 9.0 } else { 3.0 }));
        }
        // Three in 200 at 9 ms: the 99th percentile is the 198th, 9 ms.
        assert_eq!(l.percentile(), Some(ms(9.0)));
        for _ in 0..WINDOW {
            l.add(ms(9.0));
        }
        assert_eq!(l.percentile(), Some(ms(9.0)));
        for _ in 0..WINDOW {
            l.add(ms(3.0));
        }
        // The spikes left the window.
        assert_eq!(l.get(), ms(4.0));
        assert!(l.percentile().is_some());
    }

    #[test]
    fn aligning_pays_when_the_lead_is_under_the_average_wait() {
        let mut l = Lead::new(ms(1.0), ms(8.0));
        assert!(l.pays(ms(1000.0 / 240.0), false));
        for i in 0..100 {
            l.add(ms(if i % 10 == 0 { 9.0 } else { 6.0 }));
        }
        // Lead 9 + 1 = 10 ms against 6 ms and half a refresh.
        assert!(l.pays(ms(1000.0 / 60.0), false));
        assert!(!l.pays(ms(1000.0 / 144.0), false));
        assert!(!l.pays(ms(1000.0 / 240.0), true));
        // 120 Hz: 10 against 10.17, within the hysteresis: it keeps the choice it has.
        assert!(l.pays(ms(1000.0 / 120.0), true));
        assert!(!l.pays(ms(1000.0 / 120.0), false));
    }

    #[test]
    fn misses_widen_the_margin_and_runs_on_time_narrow_it_slowly() {
        let mut l = Lead::new(ms(1.0), ms(8.0));
        // Not before the costs are measured.
        l.missed();
        assert_eq!(l.margin(), ms(1.0));
        for _ in 0..WARM_UP {
            l.add(ms(3.0));
        }
        for _ in 0..WIDEN_EVERY {
            l.met();
        }
        // A run of misses widens once.
        for _ in 0..10 {
            l.missed();
        }
        assert_eq!(l.margin(), ms(2.0));
        for _ in 0..WIDEN_EVERY {
            l.missed();
        }
        assert_eq!(l.margin(), ms(3.0));
        for _ in 0..WIDEN_EVERY * 20 {
            l.missed();
        }
        assert_eq!(l.margin(), MAX_MARGIN);
        for _ in 0..NARROW_AFTER - 1 {
            l.met();
        }
        assert_eq!(l.margin(), MAX_MARGIN);
        l.met();
        assert_eq!(l.margin(), MAX_MARGIN - NARROW_STEP);
        for _ in 0..NARROW_AFTER * 100 {
            l.met();
        }
        assert_eq!(l.margin(), ms(1.0));
    }

    /// Aligned at 60 Hz: each tick starts its lead before a refresh, and its frame shows at
    /// that refresh, so input read to refresh is the lead, every tick.
    #[test]
    fn aligned_ticks_finish_just_before_their_refresh() {
        let r = simulate(
            Policy::Aligned,
            TICK,
            5.3,
            10.0,
            Costs::new(4.0, 1.0, 0, 0.0),
        );
        let settled = &r.shown[r.shown.len() / 2..];
        for (read, shown) in settled {
            let l = secs(*read, *shown) * 1000.0;
            // The costs' 99th percentile (under 5 ms) plus the 1 ms margin.
            assert!((4.9..=6.1).contains(&l), "input to refresh {l} ms");
        }
        assert_eq!(r.counts.missed, 0);
        assert_eq!(r.margin, DEFAULT_MARGIN);
    }

    /// Against the fixed timer on the same simulated displays: the latency drops by about half
    /// a refresh on average, and never a skipped refresh.
    #[test]
    fn aligned_ticks_beat_the_timer() {
        for (hz, phase) in [(60.0, 3.1), (59.94, 11.0), (144.0, 2.0), (120.0, 7.5)] {
            let refresh = Duration::from_secs_f64(1.0 / hz);
            let costs = || Costs::new(4.0, 1.5, 0, 0.0);
            let timer = simulate(Policy::Timer, refresh, phase, 20.0, costs());
            let jit = simulate(Policy::Aligned, refresh, phase, 20.0, costs());
            let (t, j) = (mean_latency(&timer), mean_latency(&jit));
            assert!(j < t, "{hz} Hz: just in time {j:.2} ms, timer {t:.2} ms");
            assert!(j < 7.0, "{hz} Hz: {j:.2} ms");
        }
    }

    /// Input read to refresh, simulated against displays that wait for refreshes (FIFO or
    /// mailbox at a fixed rate), the fixed timer at eight phases against the refreshes and
    /// aligned ticks, for a quiet machine's costs (4 to 5.5 ms) and a busy one's (as measured
    /// here beside other agents' builds: 7.5 to 11.5 ms, a 6 ms spike every 50th): mean, 99th
    /// percentile, refreshes missed and ticks unshown, printed with `--nocapture`. Aligned
    /// beats the timer at 60 Hz on both; at 144 Hz only where the costs spread less than half
    /// a refresh, which is why the automatic choice checks ([`Lead::pays`]).
    #[test]
    fn simulated_latencies() {
        let quiet = || Costs::new(4.0, 1.5, 0, 0.0);
        let busy = || Costs::new(7.5, 4.0, 50, 6.0);
        for (hz, name, costs) in [
            (60.0, "quiet", &quiet as &dyn Fn() -> Costs),
            (60.0, "busy", &busy),
            (59.94, "busy", &busy),
            (120.0, "quiet", &quiet),
            (144.0, "quiet", &quiet),
            (144.0, "busy", &busy),
        ] {
            let refresh = Duration::from_secs_f64(1.0 / hz);
            let mut row = Vec::new();
            for policy in [Policy::Timer, Policy::Aligned] {
                let mut lat = Vec::new();
                let (mut missed, mut unshown) = (0, 0);
                for k in 0..8 {
                    let phase = refresh.as_secs_f64() * 1000.0 * f64::from(k) / 8.0;
                    let r = simulate(policy, refresh, phase, 30.0, costs());
                    lat.extend(
                        r.shown[r.shown.len() / 10..]
                            .iter()
                            .map(|(i, d)| secs(*i, *d) * 1000.0),
                    );
                    missed += r.counts.missed;
                    unshown += r.counts.unshown;
                }
                lat.sort_by(f64::total_cmp);
                let mean = lat.iter().sum::<f64>() / lat.len() as f64;
                let p99 = lat[(lat.len() as f64 * 0.99) as usize];
                row.push((mean, p99, missed, unshown));
            }
            let ((tm, tp, _, _), (jm, jp, jmiss, jun)) = (row[0], row[1]);
            eprintln!(
                "{hz} Hz, {name}: timer mean {tm:.2} ms, p99 {tp:.2}; aligned mean {jm:.2} ms, p99 {jp:.2}, {jmiss} refreshes missed, {jun} ticks unshown (8 x 30 s)"
            );
            if hz < 61.0 {
                assert!(
                    jm < tm,
                    "{hz} Hz {name}: aligned {jm:.2} ms, timer {tm:.2} ms"
                );
            }
        }
    }

    /// Ticks stay one for one with Melee's clock over minutes, whatever the display's rate,
    /// with spikes that miss refreshes: none skipped or doubled, each in its slot.
    #[test]
    fn ticks_keep_melee_s_clock() {
        for hz in [60.0, 59.94, 60.05, 144.0, 143.998, 50.0, 240.0] {
            for policy in [Policy::Aligned, Policy::Cadence, Policy::Timer] {
                let refresh = Duration::from_secs_f64(1.0 / hz);
                let seconds = 300.0;
                let r = simulate(policy, refresh, 1.7, seconds, Costs::new(3.0, 2.0, 97, 9.0));
                let n = r.ticks.len() as f64;
                assert!(
                    (n - seconds * 60.0).abs() <= 2.0,
                    "{hz} Hz {policy:?}: {n} ticks in {seconds} s"
                );
                for (k, &(due, start, _)) in r.ticks.iter().enumerate() {
                    // Tick k is due at k ticks of Melee's clock, starts no earlier, and before
                    // the next is due.
                    let want = r.t0 + TICK * k as u32;
                    assert!(nanos_between(want, due).abs() <= 1, "{hz} Hz tick {k}");
                    assert!(start >= due && start < due + TICK, "{hz} Hz tick {k}");
                }
                assert_eq!(r.counts.stalls, 0);
            }
        }
    }

    /// A late frame shows a refresh late, and the next tick's frame takes a later refresh, never
    /// the same one: frames reach the screen in order, one a refresh at most.
    #[test]
    fn a_missed_refresh_makes_one_late_frame() {
        for hz in [60.0, 59.94, 144.0] {
            let refresh = Duration::from_secs_f64(1.0 / hz);
            let r = simulate(
                Policy::Aligned,
                refresh,
                0.4,
                60.0,
                Costs::new(3.0, 1.0, 211, 12.0),
            );
            assert!(r.counts.missed > 0, "{hz} Hz");
            // Each spike misses at most once: the margin and the percentile take it in.
            assert!(r.counts.missed <= 3600 / 211 + 1, "{hz} Hz: {:?}", r.counts);
            // The late frame shows; no frame is meant for its refresh too.
            assert_eq!(r.replaced, 0, "{hz} Hz: two frames for one refresh");
        }
    }

    /// 59.94 Hz against 60 ticks a second: once in about 1000 ticks the display has no refresh
    /// for a tick, which runs at its due time without a frame; the frames that do show keep
    /// their lead.
    #[test]
    fn a_slower_display_leaves_a_tick_unshown_now_and_then() {
        let r = simulate(
            Policy::Aligned,
            Duration::from_secs_f64(1.0 / 59.94),
            8.0,
            100.0,
            Costs::new(4.0, 1.0, 0, 0.0),
        );
        // 6000 ticks against 5994 refreshes.
        assert!((4..=8).contains(&r.counts.unshown), "{:?}", r.counts);
        assert_eq!(r.counts.missed, 0);
        let settled = &r.shown[100..];
        assert!(settled.iter().all(|(i, d)| secs(*i, *d) < 0.0065));
    }

    #[test]
    fn a_stall_puts_melee_s_clock_back_on_now() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(TICK, DEFAULT_MARGIN, t0);
        let p = s.plan(t0, Policy::Cadence, None);
        assert_eq!(p.start, t0);
        s.ran(&p);
        let late = t0 + ms(200.0);
        let p = s.plan(late, Policy::Cadence, None);
        assert_eq!((p.due, p.render), (late, true));
        assert_eq!(s.counts.stalls, 1);
        // Two ticks behind but not a stall: they run at once, the first without its frame.
        s.ran(&p);
        let p = s.plan(late + TICK * 3, Policy::Cadence, None);
        assert_eq!((p.due, p.render), (late + TICK, false));
    }

    #[test]
    fn ticks_go_just_before_the_frames_that_show_them() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(TICK, DEFAULT_MARGIN, t0);
        for _ in 0..WARM_UP {
            s.lead.add(ms(1.0));
        }
        // Frames every 6.94 ms from 3 ms: the first at least 2 ms (1 + 1) after the due time is
        // at 3 ms, so the tick starts at 1 ms.
        let frame = ms(1000.0 / 144.0);
        let p = s.plan_before_frames(t0, Policy::Cadence, t0 + ms(3.0), frame);
        assert_eq!((p.start, p.target), (t0 + ms(1.0), Some(t0 + ms(3.0))));
        s.ran(&p);
        // The next is due at 16.67: its frame is the one at 3 + 3 * 6.94 = 23.8 ms.
        let p = s.plan_before_frames(t0 + ms(2.0), Policy::Cadence, t0 + ms(3.0) + frame, frame);
        let f = t0 + ms(3.0) + frame * 3;
        assert!(nanos_between(f, p.target.unwrap()).abs() < 1000);
        assert!(nanos_between(f - ms(2.0), p.start).abs() < 1000);
        // The old timer runs it at its due time.
        let p = s.plan_before_frames(t0 + ms(2.0), Policy::Timer, t0 + ms(3.0) + frame, frame);
        assert_eq!(p.start, p.due);
    }
}
