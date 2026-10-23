//! How engine ticks fall on frames (`melee/lb/lb_0195.c`): the game keeps two clocks in
//! timer ticks, one at the game's speed and one at 60 Hz, and each engine tick advances both
//! by the shorter period. A clock that comes due runs that tick's game logic (slot 0) or lifts
//! the pause mask's second half (slot 1), so a slowed game skips logic on some ticks.
//!
//! The original also re-arms the controller polling alarm at that period and sets the pads'
//! sampling rate; those belong to the platform and are left out, as is the clock's source:
//! `OSSecondsToTicks` reads the bus clock the boot ROM stores, 162 MHz on every console.

use gekko_fp as fp;

/// `OS_TIMER_CLOCK`: the time base's rate, a quarter of the bus clock.
const TIMER_CLOCK: u32 = 162_000_000 / 4;

/// One clock (`UnkArrElem`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Clock {
    /// Its period in timer ticks (`x0`).
    pub period: i64,
    /// Time since it last came due (`x8`).
    pub elapsed: i64,
    /// Whether it came due this tick (`x10`).
    pub due: bool,
}

/// `lb_804329F0`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Pacing {
    pub clocks: [Clock; 2],
    /// The pads' sampling rate in milliseconds (`x4`).
    pub sampling_rate: u32,
    /// The game clock's period as last asked for (`x38`).
    pub requested: u64,
    /// The tick length both clocks advance by (`x40`).
    pub tick: i64,
    /// Whether the polling alarm runs (`x48`).
    pub alarm: bool,
}

/// `OSSecondsToTicks` of a single-precision number of seconds, as the original computes it:
/// the seconds times the clock rate in single precision, truncated (`__cvt_dbl_usll`).
pub fn seconds_to_ticks(seconds: f64) -> u64 {
    let ticks = fp::fmuls(seconds, fp::frsp(f64::from(TIMER_CLOCK)));
    ticks as u64
}

/// A sixtieth of a second in timer ticks, as `OSSecondsToTicks(1.0F / 60)`.
fn sixtieth() -> i64 {
    seconds_to_ticks(fp::fdivs(1.0, fp::frsp(60.0))) as i64
}

/// The game clock's period at a game speed: `OSSecondsToTicks(1.0F / 60 / speed)`.
pub fn ticks_at_speed(speed: f32) -> u64 {
    let s = fp::fdivs(fp::fdivs(1.0, 60.0), crate::fpu::reg(speed));
    seconds_to_ticks(s)
}

impl Pacing {
    /// Both clocks at 60 Hz and nothing due, `lb_80019AAC` (after its argument, the pads'
    /// setup, has run).
    #[doc(alias = "lb_80019AAC")]
    pub fn new() -> Pacing {
        let mut p = Pacing {
            clocks: [Clock {
                period: sixtieth(),
                elapsed: 0,
                due: false,
            }; 2],
            sampling_rate: 0,
            requested: 0,
            tick: 0,
            alarm: false,
        };
        p.clocks[0].period = 0;
        p.requested = sixtieth() as u64;
        p.update();
        p
    }

    /// Asks for the game clock's period, `lb_80019880`; [`Pacing::update`] takes it up.
    #[doc(alias = "lb_80019880")]
    pub fn request(&mut self, ticks: u64) {
        self.requested = ticks;
    }

    /// Takes up a new game clock period and the tick length, `lb_80019628` (which the scene
    /// loop runs each round through `lb_80019894`). Returns the new tick length when it
    /// changed, which sets the polling alarm again at that period.
    #[doc(alias = "lb_80019628")]
    pub fn update(&mut self) -> Option<i64> {
        let new = self.requested as i64;
        let c = &mut self.clocks[0];
        if new == c.period {
            return None;
        }
        c.period = new;
        if c.elapsed >= c.period {
            c.elapsed = 0;
        }
        let mut period = seconds_to_ticks(fp::frsp(f64::from(TIMER_CLOCK))) as i64;
        for c in &self.clocks {
            if c.period < period {
                period = c.period;
            }
        }
        if period >= sixtieth() {
            period = sixtieth();
        }
        if self.tick == period {
            return None;
        }
        self.tick = period;
        let rate = (self.tick / i64::from(TIMER_CLOCK / 1000)) as u32;
        self.sampling_rate = rate.min(11);
        self.alarm = true;
        Some(period)
    }

    /// Advances both clocks by a tick, `lb_80019900`'s clocks. Its caller renews the game and
    /// copy pad statuses when the game clock comes due.
    pub fn advance(&mut self) {
        let tick = self.tick;
        for c in &mut self.clocks {
            c.elapsed += tick;
            if c.elapsed >= c.period {
                c.elapsed -= c.period;
                c.due = true;
            } else {
                c.due = false;
            }
        }
    }

    /// The engine tick's length in nanoseconds when it is shorter than a 60th of a second (a
    /// game sped up, Lightning Melee's): the frontend runs ticks that often. None at 60 a
    /// second, a slowed game's too (whose game clock skips ticks instead).
    pub fn short_tick_nanos(&self) -> Option<u64> {
        (self.tick > 0 && self.tick < sixtieth())
            .then(|| (self.tick as u128 * 1_000_000_000 / u128::from(TIMER_CLOCK)) as u64)
    }

    /// Whether clock `i` came due this tick, `lb_80019A30`.
    #[doc(alias = "lb_80019A30")]
    pub fn due(&self, i: usize) -> bool {
        self.clocks[i].due
    }
}
