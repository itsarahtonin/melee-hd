//! The adapter through macOS's HID driver. macOS's HID driver takes every HID device as it's
//! plugged in, the adapter among them, so claiming its interface over USB fails (exclusive
//! access) unless a driver of its own such as GCAdapterDriver has taken it from the HID driver;
//! nusb can't detach the HID driver on macOS, and IOKit lets only root or an entitled program
//! capture a device. The HID driver hands over the same reports: the adapter's input report
//! (number 0x21, 37 bytes with its number) and its output reports (0x13 starts the reports,
//! 0x11 sets the motors), which IOKit's report calls pass as they are, so [`super::Adapter`]
//! reads them as it reads them over USB. Opened shared, not seized: gilrs sees the adapter too
//! and `melee_input`'s gamepads skip it ([`super::is_adapter`]).
//!
//! Everything runs on the adapter's thread: the device is scheduled on that thread's run loop,
//! which [`Hid::next`] runs, and IOKit calls [`report`] and [`removed`] from inside it.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_foundation::{
    CFDictionary, CFIndex, CFNumber, CFRetained, CFRunLoop, CFRunLoopMode, CFString,
    kCFRunLoopDefaultMode,
};
use objc2_io_kit::{
    IOHIDDevice, IOHIDManager, IOHIDReportType, IOReturn, kIOHIDOptionsTypeNone,
    kIOHIDProductIDKey, kIOHIDVendorIDKey, kIOReturnSuccess,
};

use super::{Link, MAX_REPORT, PRODUCT, START, VENDOR, as_sent};

/// Reports kept between reads: the newest matter, and the reader keeps up.
const KEPT: usize = 32;

/// What IOKit's calls write, at an address that stays put while the device is scheduled.
struct State {
    /// IOKit copies each input report here before calling [`report`].
    buffer: [u8; MAX_REPORT],
    reports: VecDeque<([u8; MAX_REPORT], usize)>,
    removed: bool,
}

/// The adapter opened through the HID driver.
pub struct Hid {
    device: CFRetained<IOHIDDevice>,
    /// Kept while the device is used: it found the device.
    _manager: CFRetained<IOHIDManager>,
    run_loop: CFRetained<CFRunLoop>,
    mode: &'static CFRunLoopMode,
    /// Owned (from `Box::into_raw`), freed in `drop` once IOKit no longer calls into it.
    state: NonNull<State>,
}

impl Hid {
    /// Finds the adapter among the HID driver's devices, opens it and starts its reports.
    pub fn open() -> Result<Hid, String> {
        let manager = IOHIDManager::new(None, kIOHIDOptionsTypeNone);
        let key = |k: &'static std::ffi::CStr| CFString::from_static_str(k.to_str().unwrap());
        let (vendor, product) = (key(kIOHIDVendorIDKey), key(kIOHIDProductIDKey));
        let ids = [
            CFNumber::new_i32(i32::from(VENDOR)),
            CFNumber::new_i32(i32::from(PRODUCT)),
        ];
        let matching = CFDictionary::from_slices(&[&*vendor, &*product], &[&*ids[0], &*ids[1]]);
        // SAFETY: a dictionary of the matching keys IOKit documents, with numbers for values.
        unsafe { manager.set_device_matching(Some(matching.as_opaque())) };
        let devices = manager.devices().ok_or("it has no such device")?;
        let count = devices.count();
        if count < 1 {
            return Err("it has no such device".into());
        }
        let mut all = vec![std::ptr::null::<c_void>(); count as usize];
        // SAFETY: room for every value; the set holds IOHIDDevices, retained while it lives.
        unsafe { devices.values(all.as_mut_ptr()) };
        let first = NonNull::new(all[0].cast_mut().cast::<IOHIDDevice>()).ok_or("no device")?;
        // SAFETY: an IOHIDDevice from the set, retained here to outlive it.
        let device = unsafe { CFRetained::retain(first) };
        let r = device.open(kIOHIDOptionsTypeNone);
        if r != kIOReturnSuccess {
            return Err(format!("it wouldn't open the adapter ({})", code(r)));
        }
        let run_loop = CFRunLoop::current().ok_or("the thread has no run loop")?;
        // SAFETY: CoreFoundation's constant, set before any program code runs.
        let mode = unsafe { kCFRunLoopDefaultMode }.ok_or("no default run loop mode")?;
        let state = NonNull::from(Box::leak(Box::new(State {
            buffer: [0; MAX_REPORT],
            reports: VecDeque::with_capacity(KEPT),
            removed: false,
        })));
        // SAFETY: the buffer and the context are `state`'s, which lives until `drop` has
        // unscheduled the device and closed it; the calls match IOKit's signatures.
        unsafe {
            let buffer = NonNull::new_unchecked((*state.as_ptr()).buffer.as_mut_ptr());
            device.register_input_report_callback(
                buffer,
                MAX_REPORT as CFIndex,
                Some(report),
                state.as_ptr().cast(),
            );
            device.register_removal_callback(Some(removed), state.as_ptr().cast());
            device.schedule_with_run_loop(&run_loop, mode);
        }
        let mut hid = Hid {
            device,
            _manager: manager,
            run_loop,
            mode,
            state,
        };
        hid.send(&[START])?;
        Ok(hid)
    }
}

impl Link for Hid {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut b = [0; MAX_REPORT];
        b[..bytes.len()].copy_from_slice(bytes);
        // SAFETY: `b` holds the report, its first byte its number, as IOKit asks of numbered
        // reports; the call is synchronous.
        let r = unsafe {
            self.device.set_report(
                IOHIDReportType::Output,
                CFIndex::from(bytes[0]),
                NonNull::from(&mut b).cast(),
                bytes.len() as CFIndex,
            )
        };
        if r == kIOReturnSuccess {
            Ok(())
        } else {
            Err(format!(
                "sending report {:#04x} failed ({})",
                bytes[0],
                code(r)
            ))
        }
    }

    fn next(&mut self, out: &mut [u8; MAX_REPORT]) -> Result<Option<usize>, String> {
        let state = self.state.as_ptr();
        // SAFETY: IOKit writes `state` only from inside the run loop's run, while no
        // reference to it is held here.
        if unsafe { (*state).reports.is_empty() } {
            CFRunLoop::run_in_mode(Some(self.mode), 0.1, true);
        }
        // SAFETY: as above; the run is over.
        unsafe {
            if (*state).removed {
                return Err("unplugged".into());
            }
            Ok((*state).reports.pop_front().map(|(r, len)| {
                out[..len].copy_from_slice(&r[..len]);
                len
            }))
        }
    }
}

impl Drop for Hid {
    fn drop(&mut self) {
        // SAFETY: scheduled on this run loop in `open`, on this thread; once unscheduled and
        // closed, IOKit calls neither callback again, so `state` can go.
        unsafe {
            self.device
                .unschedule_from_run_loop(&self.run_loop, self.mode);
            self.device.close(kIOHIDOptionsTypeNone);
            drop(Box::from_raw(self.state.as_ptr()));
        }
    }
}

/// An IOReturn as IOKit's headers write it.
fn code(r: IOReturn) -> String {
    format!("{:#010x}", r as u32)
}

/// IOKit's input report call: the report is kept for [`Hid::next`].
unsafe extern "C-unwind" fn report(
    context: *mut c_void,
    result: IOReturn,
    _sender: *mut c_void,
    _kind: IOHIDReportType,
    id: u32,
    report: NonNull<u8>,
    len: CFIndex,
) {
    let state = context.cast::<State>();
    if result != kIOReturnSuccess || len < 1 {
        return;
    }
    // SAFETY: IOKit passes the buffer registered in `open`, `len` bytes of it filled, and
    // the context `open` gave, on the thread running the run loop.
    unsafe {
        let len = (len as usize).min(MAX_REPORT);
        let bytes = std::slice::from_raw_parts(report.as_ptr(), len);
        let mut r = [0; MAX_REPORT];
        let n = as_sent(id, bytes, &mut r);
        let reports = &mut (*state).reports;
        if reports.len() == KEPT {
            reports.pop_front();
        }
        reports.push_back((r, n));
    }
}

/// IOKit's removal call: the adapter was unplugged.
unsafe extern "C-unwind" fn removed(context: *mut c_void, _result: IOReturn, _sender: *mut c_void) {
    // SAFETY: the context `open` gave, on the thread running the run loop.
    unsafe { (*context.cast::<State>()).removed = true };
}
