//! Wayland's presentation-time protocol (`wp_presentation`): before each present the player
//! asks the compositor for a feedback object on the window's surface, which applies to the
//! surface's next commit (the one the Vulkan driver makes for the present) and comes back with
//! when the frame was shown (`presented`, on the compositor's clock) and the output's refresh
//! period, or `discarded` when a newer frame replaced it.
//!
//! The requests go on winit's own connection (`Backend::from_foreign_display`, the display the
//! window and the Vulkan surface share), on a queue of the player's own; winit's event loop reads
//! the socket, and the events for this queue wait in it until [`Presentation::poll`] dispatches
//! them.

use std::time::{Duration, Instant};

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::presentation_time::client::wp_presentation::{self, WpPresentation};
use wayland_protocols::wp::presentation_time::client::wp_presentation_feedback::{
    self, WpPresentationFeedback,
};
use winit::raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use super::Displayed;
use crate::jit::Grid;

/// The window's surface's presentation feedback.
pub struct Presentation {
    conn: Connection,
    queue: EventQueue<State>,
    presentation: WpPresentation,
    surface: WlSurface,
    state: State,
}

/// What the feedback events left.
#[derive(Default)]
struct State {
    /// `wp_presentation` from the registry, and its version.
    global: Option<(u32, u32)>,
    /// The clock the compositor's times are on (a `clockid_t`).
    clock: Option<u32>,
    /// The frames shown since the last poll.
    shown: Vec<Displayed>,
    /// The latest refresh a frame was shown at (with the vsync flag), and the output's
    /// refresh period.
    refreshed: Option<Instant>,
    period: Option<Duration>,
}

impl Presentation {
    /// The feedback for `window`, when it's a Wayland window and the compositor has the
    /// protocol.
    pub fn new(window: &winit::window::Window) -> Option<Presentation> {
        let (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) = (
            window.display_handle().ok()?.as_raw(),
            window.window_handle().ok()?.as_raw(),
        ) else {
            return None;
        };
        // SAFETY: the display is winit's, which lives as long as the event loop, longer than
        // the window this is kept with.
        let backend = unsafe { Backend::from_foreign_display(d.display.as_ptr().cast()) };
        let conn = Connection::from_backend(backend);
        let mut queue = conn.new_event_queue::<State>();
        let qh = queue.handle();
        let mut state = State::default();
        let _registry = conn.display().get_registry(&qh, ());
        queue.roundtrip(&mut state).ok()?;
        let Some((name, version)) = state.global else {
            eprintln!("presentation feedback: none (the compositor has no wp_presentation)");
            return None;
        };
        let presentation: WpPresentation = _registry.bind(name, version.min(1), &qh, ());
        // Its clock_id event.
        queue.roundtrip(&mut state).ok()?;
        // SAFETY: the surface is the window's, which this is kept with (in `present::Shown`).
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), w.surface.as_ptr().cast()) }
            .ok()?;
        let surface = WlSurface::from_id(&conn, id).ok()?;
        Some(Presentation {
            conn,
            queue,
            presentation,
            surface,
            state,
        })
    }

    /// The window's present `n` comes next: its feedback is asked for.
    pub fn presenting(&mut self, n: u64) {
        let qh = self.queue.handle();
        let _ = self.presentation.feedback(&self.surface, &qh, n);
        let _ = self.conn.flush();
    }

    /// The frames shown since the last call.
    pub fn poll(&mut self) -> Vec<Displayed> {
        let _ = self.queue.dispatch_pending(&mut self.state);
        std::mem::take(&mut self.state.shown)
    }

    /// The output's refreshes, from the latest frame shown at one.
    pub fn grid(&self) -> Option<Grid> {
        let (phase, period) = (self.state.refreshed?, self.state.period?);
        (Instant::now().saturating_duration_since(phase) < Duration::from_secs(1))
            .then_some(Grid { phase, period })
    }
}

impl Drop for Presentation {
    fn drop(&mut self) {
        self.presentation.destroy();
        let _ = self.conn.flush();
    }
}

/// A time on the clock `clock` (a `clockid_t`), as an Instant.
fn instant_on(clock: u32, secs: u64, nanos: u32) -> Option<Instant> {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let at = Instant::now();
    // SAFETY: a plain output.
    if unsafe { libc::clock_gettime(clock as libc::clockid_t, &mut now) } != 0 {
        return None;
    }
    let then = secs as f64 + f64::from(nanos) / 1e9;
    let now = now.tv_sec as f64 + now.tv_nsec as f64 / 1e9;
    super::instant_of(then, now, at)
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut State,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == WpPresentation::interface().name
        {
            state.global = Some((name, version));
        }
    }
}

impl Dispatch<WpPresentation, ()> for State {
    fn event(
        state: &mut State,
        _: &WpPresentation,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            state.clock = Some(clk_id);
        }
    }
}

impl Dispatch<WpPresentationFeedback, u64> for State {
    fn event(
        state: &mut State,
        _: &WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        n: &u64,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let wp_presentation_feedback::Event::Presented {
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
            refresh,
            flags,
            ..
        } = event
        else {
            return;
        };
        let secs = (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo);
        let Some(at) = state.clock.and_then(|c| instant_on(c, secs, tv_nsec)) else {
            return;
        };
        let vsync = flags
            .into_result()
            .is_ok_and(|f| f.contains(wp_presentation_feedback::Kind::Vsync));
        if refresh > 0 {
            state.period = Some(Duration::from_nanos(u64::from(refresh)));
        }
        if vsync {
            state.refreshed = Some(at);
        }
        state.shown.push(Displayed {
            number: *n,
            at,
            exact: true,
        });
    }
}
