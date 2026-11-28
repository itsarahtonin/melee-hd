//! DXGI's frame statistics (`IDXGISwapChain::GetFrameStatistics`) on wgpu's DX12 surface.
//!
//! After each present the swap chain's present count (`GetLastPresentCount`) is kept with the
//! window's present number. The statistics name the last present that reached the screen
//! (`PresentCount`), the refresh it showed at (`PresentRefreshCount`), and the latest refresh
//! with its time on the performance counter (`SyncRefreshCount`, `SyncQPCTime`): when the two
//! refreshes are one, that time is when the frame showed, exactly; when the statistics were read
//! a refresh or more later, the time is worked out back by the refresh period, which only a
//! fixed-refresh display allows (variable refresh spaces its refreshes as the frames come).
//!
//! The swap chain is fetched for each call and let go at once: DXGI allows one swap chain a
//! window, and wgpu makes a new one on some configures, which a reference kept here would stop.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use windows::Win32::Graphics::Dxgi::{DXGI_FRAME_STATISTICS, IDXGISwapChain3};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use super::Displayed;

/// Presents kept for matching against the statistics.
const KEPT: usize = 32;
/// The most refreshes a time is worked back by.
const MOST_BEHIND: u32 = 8;

/// The swap chain's frame statistics.
pub struct Stats {
    /// The swap chain's present counts, each with the window's present number and when the
    /// present call returned, oldest first.
    presents: VecDeque<(u32, u64, Instant)>,
    /// The last present count the statistics named.
    last: Option<u32>,
    /// The performance counter's ticks a second.
    freq: i64,
}

/// The swap chain wgpu's DX12 surface presents to, if that's the surface's backend.
fn swap_chain(surface: &wgpu::Surface<'static>) -> Option<IDXGISwapChain3> {
    // SAFETY: the surface is wgpu's and outlives the call; the swap chain is used and dropped
    // by the caller, never kept.
    unsafe { surface.as_hal::<wgpu::hal::api::Dx12>()?.swap_chain() }
}

/// The performance counter now, with the Instant read beside it.
fn counter_now() -> (i64, Instant) {
    let mut qpc = 0i64;
    // SAFETY: a plain output.
    unsafe { QueryPerformanceCounter(&mut qpc) };
    (qpc, Instant::now())
}

impl Stats {
    /// The statistics, when the adapter presents through DX12.
    pub fn new(adapter: &wgpu::Adapter, surface: &wgpu::Surface<'static>) -> Option<Stats> {
        let backend = adapter.get_info().backend;
        if backend != wgpu::Backend::Dx12 {
            eprintln!(
                "presentation feedback: none on {backend:?} (on DX12, WGPU_BACKEND=dx12, the swap chain's statistics measure it)"
            );
            return None;
        }
        let mut freq = 0i64;
        // SAFETY: a plain output.
        unsafe { QueryPerformanceFrequency(&mut freq) };
        (freq > 0 && swap_chain(surface).is_some()).then(|| Stats {
            presents: VecDeque::with_capacity(KEPT),
            last: None,
            freq,
        })
    }

    /// The window's present `n` was presented.
    pub fn presented(&mut self, surface: &wgpu::Surface<'static>, n: u64) {
        let at = Instant::now();
        let Some(sc) = swap_chain(surface) else {
            return;
        };
        // SAFETY: a live swap chain.
        let Ok(count) = (unsafe { sc.GetLastPresentCount() }) else {
            return;
        };
        // A new swap chain counts from the start again.
        if self.presents.back().is_some_and(|&(c, _, _)| c >= count) {
            self.presents.clear();
            self.last = None;
        }
        if self.presents.len() == KEPT {
            self.presents.pop_front();
        }
        self.presents.push_back((count, n, at));
    }

    /// The present that last reached the screen, if it's new since the last call. `period` is
    /// the display's refresh period when it's known to be fixed.
    pub fn poll(
        &mut self,
        surface: &wgpu::Surface<'static>,
        period: Option<Duration>,
    ) -> Vec<Displayed> {
        let Some(sc) = swap_chain(surface) else {
            return Vec::new();
        };
        let mut s = DXGI_FRAME_STATISTICS::default();
        // SAFETY: a live swap chain and a plain output.
        // A composed window's swap chain (not independent flip) reads all zeros: nothing shown
        // is named, and its frames keep the estimate.
        if unsafe { sc.GetFrameStatistics(&mut s) }.is_err() {
            return Vec::new();
        }
        let (qpc, now) = counter_now();
        if s.PresentCount == 0 || self.last == Some(s.PresentCount) {
            return Vec::new();
        }
        self.last = Some(s.PresentCount);
        let Some(&(_, number, presented)) = self.presents.iter().find(|p| p.0 == s.PresentCount)
        else {
            return Vec::new();
        };
        let behind = s.SyncRefreshCount.wrapping_sub(s.PresentRefreshCount);
        let back = match (behind, period) {
            (0, _) => Duration::ZERO,
            (1..=MOST_BEHIND, Some(p)) => p * behind,
            _ => return Vec::new(),
        };
        let f = self.freq as f64;
        let Some(at) = super::instant_of(s.SyncQPCTime as f64 / f, qpc as f64 / f, now)
            .and_then(|t| t.checked_sub(back))
        else {
            return Vec::new();
        };
        // A frame can't show before its present call returned (less the flip's few
        // microseconds): a count that doesn't match the statistics' is left out.
        if at + Duration::from_millis(1) < presented {
            return Vec::new();
        }
        vec![Displayed {
            number,
            at,
            exact: behind == 0,
        }]
    }
}
