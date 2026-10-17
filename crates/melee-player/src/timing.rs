//! What the just-in-time tick needs from the OS: precise waits and the display's refresh
//! times. The scheduler ([`crate::jit`]) is portable; this is the per-OS layer under it.
//!
//! **Waiting.** The event loop sleeps until shortly before a tick or frame is due (winit's
//! wait, which wakes for window events too; on Windows it's a high-resolution waitable timer),
//! then [`sleep_until`] waits out the rest: on Windows a high-resolution waitable timer of its
//! own (`CreateWaitableTimerExW` with `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`) for all but the
//! last [`SPIN`], elsewhere the standard library's sleep (`nanosleep` on Linux and macOS), and a
//! spin for the last [`SPIN`]. [`Guard`] learns how late the event loop's wake-ups come, so the
//! precise wait starts before the worst of them.
//!
//! **Refresh times.** On Windows the desktop compositor's timing (`DwmGetCompositionTimingInfo`)
//! gives the time of a recent refresh and the refresh period, from which every refresh follows.
//! It describes the display the compositor runs on, so it's taken only when its period matches
//! the window's monitor's refresh rate. Other systems have no source yet (Wayland's
//! presentation-time protocol, macOS's `CAMetalDisplayLink`, `VK_GOOGLE_display_timing`), and
//! the scheduler runs ticks at their own cadence there.

use std::time::{Duration, Instant};

use crate::jit::Grid;

/// The last part of a precise wait, spun rather than slept: timers can wake this much late.
pub const SPIN: Duration = Duration::from_micros(300);

/// Waits until `deadline`, precisely: a timer for most of it and a spin for the end.
pub fn sleep_until(deadline: Instant) {
    let now = Instant::now();
    if deadline > now + SPIN {
        os::sleep(deadline - SPIN - now);
    }
    while Instant::now() < deadline {
        std::hint::spin_loop();
    }
}

/// How long before a deadline the event loop's wait should end so the precise wait can take
/// over: a little more than its wake-ups' recent 99th percentile lateness.
#[derive(Debug)]
pub struct Guard {
    late: std::collections::VecDeque<Duration>,
}

/// The guard's least and most.
const GUARD_MIN: Duration = Duration::from_micros(500);
const GUARD_MAX: Duration = Duration::from_millis(4);
/// Wake-ups kept for the percentile.
const GUARD_WINDOW: usize = 240;

impl Default for Guard {
    fn default() -> Guard {
        Guard {
            late: std::collections::VecDeque::with_capacity(GUARD_WINDOW),
        }
    }
}

impl Guard {
    /// The event loop woke at `woke`, having asked for `asked`.
    pub fn woke(&mut self, asked: Instant, woke: Instant) {
        if self.late.len() == GUARD_WINDOW {
            self.late.pop_front();
        }
        self.late.push_back(woke.saturating_duration_since(asked));
    }

    /// How long before a deadline to wake.
    pub fn get(&self) -> Duration {
        let mut v: Vec<Duration> = self.late.iter().copied().collect();
        if v.len() < 20 {
            return Duration::from_millis(2);
        }
        v.sort();
        let p99 = v[((v.len() as f64 * 0.99).ceil() as usize).clamp(1, v.len()) - 1];
        (p99 + Duration::from_micros(250)).clamp(GUARD_MIN, GUARD_MAX)
    }
}

/// The display's refreshes, where the system says when they come and the period matches
/// `expected` (the window's monitor's refresh period) within 1%.
pub fn refresh_grid(expected: Option<Duration>) -> Option<Grid> {
    let g = os::refresh_grid()?;
    let ok = expected.is_none_or(|e| {
        let r = g.period.as_secs_f64() / e.as_secs_f64();
        (0.99..=1.01).contains(&r)
    });
    ok.then_some(g)
}

#[cfg(windows)]
mod os {
    use std::cell::OnceCell;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
    use windows_sys::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE, SetWaitableTimer,
        TIMER_ALL_ACCESS, WaitForSingleObject,
    };

    use crate::jit::Grid;

    /// A thread's high-resolution waitable timer.
    struct Timer(HANDLE);

    impl Drop for Timer {
        fn drop(&mut self) {
            // SAFETY: the handle is the timer this thread made and nothing else closes it.
            unsafe { CloseHandle(self.0) };
        }
    }

    thread_local! {
        static TIMER: OnceCell<Option<Timer>> = const { OnceCell::new() };
    }

    pub fn sleep(d: Duration) {
        let slept = TIMER.with(|t| {
            let timer = t.get_or_init(|| {
                // SAFETY: no attributes or name; the handle is checked.
                let h = unsafe {
                    CreateWaitableTimerExW(
                        std::ptr::null(),
                        std::ptr::null(),
                        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                        TIMER_ALL_ACCESS,
                    )
                };
                (!h.is_null()).then_some(Timer(h))
            });
            let Some(timer) = timer else { return false };
            // Relative due times are negative, in 100 ns units.
            let due = -((d.as_nanos() / 100).max(1) as i64);
            // SAFETY: a valid timer handle and due time, no completion routine.
            unsafe {
                SetWaitableTimer(timer.0, &due, 0, None, std::ptr::null(), 0) != 0
                    && WaitForSingleObject(timer.0, INFINITE) == 0
            }
        });
        if !slept {
            std::thread::sleep(d);
        }
    }

    pub fn refresh_grid() -> Option<Grid> {
        let mut info = DWM_TIMING_INFO {
            cbSize: size_of::<DWM_TIMING_INFO>() as u32,
            ..Default::default()
        };
        let (mut freq, mut qpc) = (0i64, 0i64);
        // SAFETY: the struct is sized as the call wants; the counters are plain outputs.
        let (hr, now) = unsafe {
            let hr = DwmGetCompositionTimingInfo(std::ptr::null_mut(), &mut info);
            let now = Instant::now();
            QueryPerformanceCounter(&mut qpc);
            QueryPerformanceFrequency(&mut freq);
            (hr, now)
        };
        // The struct is packed: copy the fields out.
        let (period, vblank) = (info.qpcRefreshPeriod, info.qpcVBlank);
        if hr < 0 || freq <= 0 || period == 0 || vblank == 0 {
            return None;
        }
        let ticks = |n: u64| Duration::from_secs_f64(n as f64 / freq as f64);
        let qpc = qpc as u64;
        // A recent refresh, or the next one: one further than a second either way means the
        // clock is stale.
        let phase = if vblank <= qpc {
            now.checked_sub(ticks(qpc - vblank))?
        } else {
            now + ticks(vblank - qpc)
        };
        if phase.max(now) - phase.min(now) > Duration::from_secs(1) {
            return None;
        }
        Some(Grid {
            phase,
            period: ticks(period),
        })
    }
}

#[cfg(not(windows))]
mod os {
    use std::time::Duration;

    use crate::jit::Grid;

    pub fn sleep(d: Duration) {
        std::thread::sleep(d);
    }

    pub fn refresh_grid() -> Option<Grid> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precise_waits_end_at_their_deadline() {
        for us in [0u64, 200, 1500, 4000] {
            // The spin ends it within microseconds; a busy machine can preempt the spin, so
            // only a gross overshoot on every one of three tries fails.
            let late = (0..3)
                .map(|_| {
                    let deadline = Instant::now() + Duration::from_micros(us);
                    sleep_until(deadline);
                    Instant::now().saturating_duration_since(deadline)
                })
                .min()
                .unwrap();
            assert!(late < Duration::from_millis(5), "{us} us: {late:?} late");
        }
    }

    #[test]
    fn the_guard_follows_the_wake_ups() {
        let mut g = Guard::default();
        assert_eq!(g.get(), Duration::from_millis(2));
        let t = Instant::now();
        for i in 0..200u64 {
            g.woke(t, t + Duration::from_micros(100 + (i % 10) * 50));
        }
        // The 99th percentile is 550 us, plus 250.
        assert_eq!(g.get(), Duration::from_micros(800));
        for _ in 0..240 {
            g.woke(t, t);
        }
        assert_eq!(g.get(), GUARD_MIN);
    }

    #[test]
    fn a_refresh_grid_matches_the_monitor_or_is_left_out() {
        // Whatever the system says, a grid whose period is far from the monitor's isn't used.
        assert_eq!(refresh_grid(Some(Duration::from_secs(1))), None);
        if let Some(g) = refresh_grid(None) {
            assert!(g.period > Duration::from_millis(2) && g.period < Duration::from_millis(50));
            assert!(g.phase <= Instant::now());
        }
    }
}
