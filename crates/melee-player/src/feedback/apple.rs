//! macOS's refresh times: a `CADisplayLink` from the window's view (macOS 14 and later,
//! `-[NSView displayLinkWithTarget:selector:]`) on the main run loop, which the player's thread
//! runs (winit's rule there). Each callback leaves the refresh that just came and the next one
//! (`timestamp`, `targetTimestamp`, on `CACurrentMediaTime`'s clock), from which every refresh
//! follows. It says when refreshes come, not when a frame showed: wgpu's present keeps the
//! drawable, whose `presentedTime` would say that, to itself.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::jit::Grid;

/// The latest callback's refresh and the next one's, in seconds on `CACurrentMediaTime`'s
/// clock.
type Last = Arc<Mutex<Option<(f64, f64)>>>;

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the class implements no Drop.
    #[unsafe(super(NSObject))]
    #[name = "MeleeHdRefreshTimes"]
    #[ivars = Last]
    struct Target;

    impl Target {
        /// The display link's callback.
        #[unsafe(method(step:))]
        fn step(&self, link: &CADisplayLink) {
            if let Ok(mut last) = self.ivars().lock() {
                *last = Some((link.timestamp(), link.targetTimestamp()));
            }
        }
    }
);

/// The window's display link.
pub struct Link {
    link: Retained<CADisplayLink>,
    _target: Retained<Target>,
    last: Last,
}

impl Link {
    /// The display link for `window`'s view, where the system has one (macOS 14 and later).
    pub fn new(window: &winit::window::Window) -> Option<Link> {
        let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: winit's view, alive as long as the window this is kept with.
        let view: &AnyObject = unsafe { h.ns_view.cast::<AnyObject>().as_ref() };
        let selector = sel!(displayLinkWithTarget:selector:);
        // SAFETY: an NSObject method on a view.
        let has: bool = unsafe { msg_send![view, respondsToSelector: selector] };
        if !has {
            eprintln!("presentation feedback: none (CADisplayLink needs macOS 14)");
            return None;
        }
        let last = Last::default();
        let target = Target::alloc().set_ivars(last.clone());
        // SAFETY: NSObject's init on a freshly allocated object.
        let target: Retained<Target> = unsafe { msg_send![super(target), init] };
        // SAFETY: the view answers the selector (checked above); the target answers `step:`
        // with the display link, as the method expects, and lives as long as the link.
        let link: Retained<CADisplayLink> =
            unsafe { msg_send![view, displayLinkWithTarget: &*target, selector: sel!(step:)] };
        // SAFETY: the main run loop and its common modes, a valid pair; the player's thread is
        // the main thread on macOS.
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        Some(Link {
            link,
            _target: target,
            last,
        })
    }

    /// The refreshes, from the latest callback, if it came within the last second.
    pub fn grid(&self) -> Option<Grid> {
        let (shown, next) = (*self.last.lock().ok()?)?;
        let (now, at) = (CACurrentMediaTime(), Instant::now());
        let period = next - shown;
        if !(0.001..0.1).contains(&period) || now - shown > 1.0 {
            return None;
        }
        Some(Grid {
            phase: super::instant_of(next, now, at)?,
            period: Duration::from_secs_f64(period),
        })
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}
