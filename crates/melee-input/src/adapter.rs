//! The Wii U GameCube controller adapter (WUP-028, and adapters that copy it, such as
//! Mayflash's in Wii U mode), read raw over USB as Dolphin's GCAdapter reads it, not through
//! SDL, which rescales the axes. On Windows it needs the WinUSB driver (Zadig installs it),
//! which HIDUSBF can overclock to 1000 Hz; on Linux, a udev rule giving the user its device
//! (`crates/melee-player/linux/51-melee-hd-gcadapter.rules`), and the kernel's HID driver is
//! detached from it as it's opened. On macOS the system's HID driver holds it and nothing short
//! of root can detach that, so it's read through the HID driver ([`hid`]), the same reports,
//! unless a driver such as GCAdapterDriver has taken it for raw reading. A thread keeps the
//! latest report and sends rumble;
//! [`Adapter::ports`] turns the report into what `PADRead` would return, with the origin
//! calibration the console's SI and PAD library do. Ported from ssbm-rs's player.

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use melee_sim::input::pad::{ERR_NONE, PadStatus};
use nusb::MaybeFuture;
use nusb::transfer::{Buffer, In, Interrupt, Out};

#[cfg(target_os = "macos")]
mod hid;

const VENDOR: u16 = 0x057e;
const PRODUCT: u16 = 0x0337;
/// A report: this byte, then nine per port.
const REPORT: u8 = 0x21;
const REPORT_LEN: usize = 37;
/// Room for any report the adapter sends.
const MAX_REPORT: usize = 64;
/// Starts the reports.
const START: u8 = 0x13;
/// Sets the four motors.
const RUMBLE: u8 = 0x11;

/// What the system needs before the adapter can be read, for the message when it can't.
const NEEDS: &str = if cfg!(windows) {
    "on Windows it needs the WinUSB driver (Zadig)"
} else if cfg!(target_os = "linux") {
    "on Linux it needs a udev rule that lets you open it (README.md's Controllers)"
} else if cfg!(target_os = "macos") {
    "on macOS it's read through the system's HID driver, or directly where a driver such as      GCAdapterDriver has taken it"
} else {
    "it may need a driver or permission to open it"
};

// PADStatus buttons.
const X: u16 = 0x0400;
const Y: u16 = 0x0800;
const START_BUTTON: u16 = 0x1000;

/// The adapter, while the thread reading it runs.
pub struct Adapter {
    shared: Mutex<Shared>,
    stop: AtomicBool,
}

#[derive(Default)]
struct Shared {
    /// The latest report, while the adapter is open, and when it came.
    report: Option<([u8; REPORT_LEN], Instant)>,
    ports: [Port; 4],
    /// The motors the game wants on, and those the adapter was last told.
    rumble: [bool; 4],
    sent: [bool; 4],
    /// Reports read, for the polling rate.
    reports: u64,
}

#[derive(Clone, Copy, Default)]
struct Port {
    connected: bool,
    wireless: bool,
    /// Stick x and y, C-stick x and y, L and R, as the controller reported them when plugged in.
    origin: [u8; 6],
    /// Since when X, Y and Start alone have been held.
    combo: Option<Instant>,
}

impl Adapter {
    /// Starts reading the adapter, now and whenever it's plugged in later. The thread holds
    /// the adapter's interface only while it reads it; [`Adapter::close`] lets it go.
    pub fn start() -> Arc<Adapter> {
        let adapter = Arc::new(Adapter {
            shared: Mutex::default(),
            stop: AtomicBool::new(false),
        });
        let a = adapter.clone();
        std::thread::Builder::new()
            .name("gc-adapter".into())
            .spawn(move || a.run())
            .expect("the adapter thread");
        adapter
    }

    fn run(&self) {
        let mut said = String::new();
        while !self.stop.load(Ordering::Relaxed) {
            let found = nusb::list_devices()
                .wait()
                .ok()
                .and_then(|mut d| d.find(|d| d.vendor_id() == VENDOR && d.product_id() == PRODUCT));
            if let Some(info) = found {
                let Err(e) = self.read(&info);
                let was_open = {
                    let mut s = self.shared.lock().unwrap();
                    s.sent = [false; 4];
                    s.report.take().is_some()
                };
                if self.stop.load(Ordering::Relaxed) {
                    return;
                }
                if was_open {
                    eprintln!("input: GameCube adapter disconnected ({e})");
                    said.clear();
                } else {
                    let msg = format!(
                        "input: found a GameCube adapter but can't read it ({e}); {NEEDS}, and \
                         no other program (Dolphin) can have it open"
                    );
                    if msg != said {
                        eprintln!("{msg}");
                        said = msg;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// Reads reports until the adapter goes away or the player closes: raw over USB, or on
    /// macOS, where the system's HID driver holds the adapter and only a driver of its own
    /// (GCAdapterDriver) frees it for that, through the HID driver.
    fn read(&self, info: &nusb::DeviceInfo) -> Result<Infallible, String> {
        let mut link: Box<dyn Link> = match Usb::open(info) {
            Ok(usb) => Box::new(usb),
            #[cfg(target_os = "macos")]
            Err(usb) => Box::new(
                hid::Hid::open().map_err(|e| format!("{usb}; through macOS's HID driver: {e}"))?,
            ),
            #[cfg(not(target_os = "macos"))]
            Err(e) => return Err(e),
        };
        let mut first = true;
        let mut data = [0; MAX_REPORT];
        loop {
            if self.stop.load(Ordering::Relaxed) {
                let _ = link.send(&[RUMBLE, 0, 0, 0, 0]);
                return Err("closed".into());
            }
            let Some(len) = link.next(&mut data)? else {
                continue;
            };
            let data = &data[..len];
            let rumble = {
                let mut s = self.shared.lock().unwrap();
                if data.len() == REPORT_LEN && data[0] == REPORT {
                    s.report = Some((data.try_into().unwrap(), Instant::now()));
                    s.reports += 1;
                    if first {
                        eprintln!("input: GameCube adapter connected");
                        first = false;
                    }
                }
                // WaveBirds have no motor.
                let mut want = s.rumble;
                for (w, p) in want.iter_mut().zip(&s.ports) {
                    *w &= p.connected && !p.wireless;
                }
                (want != s.sent).then(|| {
                    s.sent = want;
                    want
                })
            };
            if let Some(m) = rumble {
                link.send(&[RUMBLE, m[0] as u8, m[1] as u8, m[2] as u8, m[3] as u8])?;
            }
        }
    }

    /// The four ports as `PADRead` would report them; `None` where no controller is plugged in.
    pub fn ports(&self) -> [Option<PadStatus>; 4] {
        let mut s = self.shared.lock().unwrap();
        let Some((report, _)) = s.report else {
            s.ports = Default::default();
            return [None; 4];
        };
        let mut out = [None; 4];
        for (i, port) in s.ports.iter_mut().enumerate() {
            let d = &report[1 + 9 * i..][..9];
            let kind = d[0] >> 4;
            let raw = [d[3], d[4], d[5], d[6], d[7], d[8]];
            // 1 wired, 2 wireless. A WaveBird that isn't on yet reports its sticks at 0.
            if kind == 0 || raw[..4] == [0; 4] {
                port.connected = false;
                continue;
            }
            let button = buttons(d[1], d[2]);
            if !port.connected {
                *port = Port {
                    connected: true,
                    wireless: kind == 2,
                    origin: raw,
                    combo: None,
                };
            }
            // X, Y and Start held alone for three seconds take the origin again, as on the
            // console.
            if button == X | Y | START_BUTTON {
                match port.combo {
                    None => port.combo = Some(Instant::now()),
                    Some(t) if t.elapsed() >= Duration::from_secs(3) => {
                        port.origin = raw;
                        port.combo = None;
                    }
                    Some(_) => {}
                }
            } else {
                port.combo = None;
            }
            out[i] = Some(status(button, raw, port.origin));
        }
        out
    }

    /// Reports read so far, for measuring the polling rate.
    pub fn reports(&self) -> u64 {
        self.shared.lock().unwrap().reports
    }

    /// When the latest report arrived, while the adapter is open: [`Adapter::ports`] reads it.
    pub fn report_time(&self) -> Option<Instant> {
        self.shared.lock().unwrap().report.map(|(_, t)| t)
    }

    /// Turns the motors on or off, for ports with a controller that has one.
    pub fn rumble(&self, on: [bool; 4]) {
        self.shared.lock().unwrap().rumble = on;
    }

    /// Stops the motors and lets the adapter go, waiting briefly for the thread to hear it.
    pub fn close(&self) {
        self.rumble([false; 4]);
        self.stop.store(true, Ordering::Relaxed);
        let until = Instant::now() + Duration::from_millis(200);
        while Instant::now() < until {
            if self.shared.lock().unwrap().report.is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Whether a USB or HID device is the adapter, by its vendor and product, so that the gamepad
/// layer can leave alone what this module reads (macOS's HID driver shows it to gilrs).
pub fn is_adapter(vendor: u16, product: u16) -> bool {
    (vendor, product) == (VENDOR, PRODUCT)
}

/// A way to the adapter: raw over USB ([`Usb`]), or macOS's HID driver ([`hid::Hid`]).
trait Link {
    /// Sends a command: its first byte says which (`START`, `RUMBLE`), as the adapter's
    /// numbered reports do.
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;

    /// The next report into `out`, its length; none when none came within about 100 ms. An
    /// error when the adapter has gone.
    fn next(&mut self, out: &mut [u8; MAX_REPORT]) -> Result<Option<usize>, String>;
}

/// The adapter's interface claimed over USB with nusb, four reads kept in flight.
struct Usb {
    out: nusb::Endpoint<Interrupt, Out>,
    input: nusb::Endpoint<Interrupt, In>,
}

impl Usb {
    /// Claims the adapter's interface and starts its reports.
    fn open(info: &nusb::DeviceInfo) -> Result<Usb, String> {
        let device = info.open().wait().map_err(|e| e.to_string())?;
        // On Linux the kernel's HID driver takes the adapter's interface as it's plugged in, so
        // it's let go of first, as Dolphin does; elsewhere this is a plain claim.
        let intf = device
            .detach_and_claim_interface(0)
            .wait()
            .map_err(|e| e.to_string())?;
        let out = intf
            .endpoint::<Interrupt, Out>(0x02)
            .map_err(|e| e.to_string())?;
        let input = intf
            .endpoint::<Interrupt, In>(0x81)
            .map_err(|e| e.to_string())?;
        let mut usb = Usb { out, input };
        usb.send(&[START])?;
        let len = usb.input.max_packet_size();
        for _ in 0..4 {
            usb.input.submit(Buffer::new(len));
        }
        Ok(usb)
    }
}

impl Link for Usb {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut b = Buffer::new(bytes.len());
        b.extend_from_slice(bytes);
        self.out
            .transfer_blocking(b, Duration::from_millis(100))
            .status
            .map_err(|e| e.to_string())
    }

    fn next(&mut self, out: &mut [u8; MAX_REPORT]) -> Result<Option<usize>, String> {
        let Some(c) = self.input.wait_next_complete(Duration::from_millis(100)) else {
            return Ok(None);
        };
        c.status.map_err(|e| e.to_string())?;
        let len = c.actual_len.min(MAX_REPORT);
        out[..len].copy_from_slice(&c.buffer[..len]);
        self.input.submit(c.buffer);
        Ok(Some(len))
    }
}

/// A report from macOS's HID driver as the adapter sent it over USB: the HID driver passes a
/// numbered report with its number first, but should it pass one without, the number goes back
/// in front.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn as_sent(id: u32, report: &[u8], out: &mut [u8; MAX_REPORT]) -> usize {
    if report.len() == REPORT_LEN - 1 && id == u32::from(REPORT) {
        out[0] = REPORT;
        out[1..REPORT_LEN].copy_from_slice(report);
        REPORT_LEN
    } else {
        let len = report.len().min(MAX_REPORT);
        out[..len].copy_from_slice(&report[..len]);
        len
    }
}

/// The adapter's two button bytes as `PADStatus` buttons.
fn buttons(b1: u8, b2: u8) -> u16 {
    const BITS: [(u8, u8, u16); 12] = [
        (0, 0x01, 0x0100), // A
        (0, 0x02, 0x0200), // B
        (0, 0x04, X),
        (0, 0x08, Y),
        (0, 0x10, 0x0001), // left
        (0, 0x20, 0x0002), // right
        (0, 0x40, 0x0004), // down
        (0, 0x80, 0x0008), // up
        (1, 0x01, START_BUTTON),
        (1, 0x02, 0x0010), // Z
        (1, 0x04, 0x0020), // R
        (1, 0x08, 0x0040), // L
    ];
    let mut out = 0;
    for (byte, bit, b) in BITS {
        if [b1, b2][byte as usize] & bit != 0 {
            out |= b;
        }
    }
    out
}

/// What the PAD library makes of a controller's raw values (`SPEC2_MakeStatus` in analog mode
/// 3, the default), given its origin (`UpdateOrigin`).
fn status(button: u16, raw: [u8; 6], origin: [u8; 6]) -> PadStatus {
    let centered = |v: u8| v.wrapping_sub(128) as i8;
    let mut org = origin.map(centered);
    // The X patch, for controllers that report a bad stick x origin.
    if org[0] > 0x40 {
        org[0] = 0;
    }
    PadStatus {
        button,
        stick_x: clamp_s8(centered(raw[0]), org[0]),
        stick_y: clamp_s8(centered(raw[1]), org[1]),
        substick_x: clamp_s8(centered(raw[2]), org[2]),
        substick_y: clamp_s8(centered(raw[3]), org[3]),
        trigger_left: raw[4].max(origin[4]) - origin[4],
        trigger_right: raw[5].max(origin[5]) - origin[5],
        analog_a: 0,
        analog_b: 0,
        err: ERR_NONE,
    }
}

/// `ClampS8`: the value less the origin, kept in range.
fn clamp_s8(v: i8, org: i8) -> i8 {
    let v = if org > 0 {
        v.max(i8::MIN + org)
    } else if org < 0 {
        v.min(i8::MAX + org)
    } else {
        v
    };
    v.wrapping_sub(org)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_calibration() {
        // At rest at the origin: centered.
        let origin = [0x7f, 0x80, 0x82, 0x7e, 0x20, 0x00];
        let p = status(0, origin, origin);
        assert_eq!(
            (p.stick_x, p.stick_y, p.substick_x, p.substick_y),
            (0, 0, 0, 0)
        );
        assert_eq!((p.trigger_left, p.trigger_right), (0, 0));
        // Full left from an origin of -1: clamped to -127 rather than wrapping.
        let p = status(0, [0x00, 0xff, 0x82, 0x7e, 0x10, 0xff], origin);
        assert_eq!((p.stick_x, p.stick_y), (-127, 127));
        // Triggers below their origin read 0.
        assert_eq!((p.trigger_left, p.trigger_right), (0, 0xff));
        // The X patch: a stick x origin past 0x40 counts as 0.
        let p = status(
            0,
            [0xd0, 0x80, 0x80, 0x80, 0, 0],
            [0xd0, 0x80, 0x80, 0x80, 0, 0],
        );
        assert_eq!(p.stick_x, 0x50);
    }

    #[test]
    fn adapter_buttons() {
        assert_eq!(buttons(0x01, 0x00), 0x0100);
        assert_eq!(buttons(0x0c, 0x01), X | Y | START_BUTTON);
        assert_eq!(buttons(0x80, 0x0e), 0x0008 | 0x0010 | 0x0020 | 0x0040);
    }

    #[test]
    fn a_report_reads_as_sent() {
        let mut usb = [0; REPORT_LEN];
        usb[0] = REPORT;
        usb[1] = 0x10;
        usb[36] = 0x7f;
        let mut out = [0; MAX_REPORT];
        // With its number, as IOKit passes a numbered report.
        assert_eq!(as_sent(u32::from(REPORT), &usb, &mut out), REPORT_LEN);
        assert_eq!(out[..REPORT_LEN], usb);
        // Without it, should IOKit pass it so.
        out = [0; MAX_REPORT];
        assert_eq!(as_sent(u32::from(REPORT), &usb[1..], &mut out), REPORT_LEN);
        assert_eq!(out[..REPORT_LEN], usb);
        // Any other report as it came.
        assert_eq!(as_sent(0x22, &usb[1..], &mut out), REPORT_LEN - 1);
        assert_eq!(out[..REPORT_LEN - 1], usb[1..]);
    }
}
