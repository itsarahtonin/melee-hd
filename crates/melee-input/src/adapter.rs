//! The Wii U GameCube controller adapter (WUP-028, and adapters that copy it, such as
//! Mayflash's in Wii U mode), read raw over USB as Dolphin's GCAdapter reads it, not through
//! SDL, which rescales the axes. On Windows it needs the WinUSB driver (Zadig installs it),
//! which HIDUSBF can overclock to 1000 Hz; on Linux, a udev rule giving the user its device
//! (`crates/melee-player/linux/51-melee-hd-gcadapter.rules`), and the kernel's HID driver is
//! detached from it as it's opened. On macOS the system's HID driver holds it and nothing short
//! of root can detach that, so it's read through the HID driver ([`hid`]), the same reports,
//! unless a driver such as GCAdapterDriver has taken it for raw reading. A thread keeps the
//! latest report and sends rumble; either way each report goes the same way from there
//! (`Shared::take`, PhobGCC's processing on the ports it's on for, then `decode`), so the
//! adapter's port `n` is `DeviceId::Adapter(n)` whichever path read it. [`Adapter::ports`] turns the report into what `PADRead` would return, with the origin
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
    /// PhobGCC's processing, for the ports it's on for.
    phob: [Option<Phob>; 4],
}

/// A port's PhobGCC processing: it runs on this thread for every report, so its filters see
/// every sample the adapter gives, and [`Adapter::ports`] hands its latest output to `status`.
struct Phob {
    processor: melee_phob::Processor,
    /// When the port's last report came, and the time between its reports, averaged, in
    /// milliseconds.
    last: Option<Instant>,
    interval: Option<f64>,
    /// The latest output, while a controller is plugged in.
    out: Option<melee_phob::Output>,
}

impl Phob {
    fn new(profile: melee_phob::Profile) -> Phob {
        Phob {
            processor: melee_phob::Processor::new(profile),
            last: None,
            interval: None,
            out: None,
        }
    }

    /// The time step the filters run with: the reports' average interval in whole
    /// milliseconds (USB polls in 1 ms frames), so a late or bunched report doesn't jolt them.
    fn step(&mut self, now: Instant) -> f32 {
        if let Some(last) = self.last {
            let ms = now.saturating_duration_since(last).as_secs_f64() * 1000.0;
            self.interval = Some(match self.interval {
                Some(avg) => avg + (ms - avg) / 32.0,
                None => ms,
            });
        }
        self.last = Some(now);
        self.interval.unwrap_or(8.0).round().clamp(1.0, 16.0) as f32
    }
}

/// A port's nine bytes of a report: the controller's kind (0 none, 1 wired, 2 wireless), its
/// buttons, and its sticks and triggers; `None` with no controller.
fn decode(d: &[u8]) -> Option<(u8, u16, [u8; 6])> {
    let kind = d[0] >> 4;
    let raw = [d[3], d[4], d[5], d[6], d[7], d[8]];
    // 1 wired, 2 wireless. A WaveBird that isn't on yet reports its sticks at 0.
    (kind != 0 && raw[..4] != [0; 4]).then(|| (kind, buttons(d[1], d[2]), raw))
}

impl Shared {
    /// Takes a report: kept for [`Adapter::ports`], counted, and run through PhobGCC's
    /// processing on the ports it's on for.
    fn take(&mut self, data: [u8; REPORT_LEN], now: Instant) {
        self.report = Some((data, now));
        self.reports += 1;
        for (i, phob) in self.phob.iter_mut().enumerate() {
            let Some(phob) = phob else { continue };
            let Some((_, button, raw)) = decode(&data[1 + 9 * i..][..9]) else {
                phob.out = None;
                phob.last = None;
                continue;
            };
            let port = &self.ports[i];
            // The origin `ports` took, once it has seen the controller.
            let origin = if port.connected { port.origin } else { raw };
            let input = melee_phob::Input {
                buttons: button,
                sticks: [raw[0], raw[1], raw[2], raw[3]],
                sliders: [
                    raw[4].saturating_sub(origin[4]),
                    raw[5].saturating_sub(origin[5]),
                ],
                origin: [origin[0], origin[1], origin[2], origin[3]],
            };
            let step = phob.step(now);
            phob.out = Some(phob.processor.process(&input, step));
        }
    }
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
            let Some((len, at)) = link.next(&mut data)? else {
                continue;
            };
            let data = &data[..len];
            let rumble = {
                let mut s = self.shared.lock().unwrap();
                if data.len() == REPORT_LEN && data[0] == REPORT {
                    s.take(data.try_into().unwrap(), at);
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
        let Shared { ports, phob, .. } = &mut *s;
        for (i, port) in ports.iter_mut().enumerate() {
            let Some((kind, button, raw)) = decode(&report[1 + 9 * i..][..9]) else {
                port.connected = false;
                continue;
            };
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
            // With PhobGCC's processing, its output, with the origin it reports.
            if let Some(o) = phob[i].as_ref().and_then(|p| p.out) {
                let [ax, ay, cx, cy] = o.sticks;
                let [l, r] = o.triggers;
                out[i] = Some(status(
                    o.buttons,
                    [ax, ay, cx, cy, l, r],
                    melee_phob::processor::ORIGIN,
                ));
            }
        }
        out
    }

    /// Turns PhobGCC's processing on for an adapter port with `profile`, or off with `None`.
    /// On a port it's already on for, the new profile replaces the old (a calibration in
    /// progress is cancelled) and the filters keep their history.
    pub fn set_phob(&self, port: usize, profile: Option<melee_phob::Profile>) {
        let mut s = self.shared.lock().unwrap();
        match (&mut s.phob[port], profile) {
            (slot, None) => *slot = None,
            (Some(p), Some(profile)) => p.processor.set_profile(profile),
            (slot, Some(profile)) => *slot = Some(Phob::new(profile)),
        }
    }

    /// Runs `f` on an adapter port's PhobGCC processing, while it's on: for its settings, its
    /// calibration and the diagnostic views.
    pub fn phob<R>(
        &self,
        port: usize,
        f: impl FnOnce(&mut melee_phob::Processor) -> R,
    ) -> Option<R> {
        let mut s = self.shared.lock().unwrap();
        s.phob[port].as_mut().map(|p| f(&mut p.processor))
    }

    /// Whether a controller is plugged into an adapter port, by the latest report.
    pub fn plugged(&self, port: usize) -> bool {
        let s = self.shared.lock().unwrap();
        s.report
            .is_some_and(|(r, _)| decode(&r[1 + 9 * port..][..9]).is_some())
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

    /// The next report into `out`, its length and when it arrived (PhobGCC's filters step
    /// by the time between reports); none when none came within about 100 ms. An error when
    /// the adapter has gone.
    fn next(&mut self, out: &mut [u8; MAX_REPORT]) -> Result<Option<(usize, Instant)>, String>;
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

    fn next(&mut self, out: &mut [u8; MAX_REPORT]) -> Result<Option<(usize, Instant)>, String> {
        let Some(c) = self.input.wait_next_complete(Duration::from_millis(100)) else {
            return Ok(None);
        };
        c.status.map_err(|e| e.to_string())?;
        let len = c.actual_len.min(MAX_REPORT);
        out[..len].copy_from_slice(&c.buffer[..len]);
        self.input.submit(c.buffer);
        Ok(Some((len, Instant::now())))
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
pub(crate) fn status(button: u16, raw: [u8; 6], origin: [u8; 6]) -> PadStatus {
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

    fn adapter() -> Adapter {
        Adapter {
            shared: Mutex::default(),
            stop: AtomicBool::new(false),
        }
    }

    /// A made-up report sequence: controllers plugged and unplugged, WaveBirds waking, sticks
    /// and triggers anywhere, every button.
    fn reports(n: usize) -> Vec<[u8; REPORT_LEN]> {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut kinds = [1u8, 2, 0, 1];
        (0..n)
            .map(|_| {
                let mut r = [0u8; REPORT_LEN];
                r[0] = REPORT;
                for (i, kind) in kinds.iter_mut().enumerate() {
                    if next() % 200 == 0 {
                        *kind = (next() % 3) as u8;
                    }
                    let d = &mut r[1 + 9 * i..][..9];
                    d[0] = *kind << 4;
                    for b in &mut d[1..] {
                        *b = next() as u8;
                    }
                    match next() % 50 {
                        0 => d[3..7].fill(0),
                        // Some sticks' bytes at 0 but not all: still plugged in.
                        1 => d[3..6].fill(0),
                        _ => {}
                    }
                }
                r
            })
            .collect()
    }

    /// What `ports` gave before PhobGCC's processing came in (the console's path alone),
    /// kept as it was for the check below.
    fn ports_before(report: &[u8; REPORT_LEN], ports: &mut [Port; 4]) -> [Option<PadStatus>; 4] {
        let mut out = [None; 4];
        for (i, port) in ports.iter_mut().enumerate() {
            let d = &report[1 + 9 * i..][..9];
            let kind = d[0] >> 4;
            let raw = [d[3], d[4], d[5], d[6], d[7], d[8]];
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
            out[i] = Some(status(button, raw, port.origin));
        }
        out
    }

    #[test]
    fn without_phob_the_ports_are_as_before() {
        let a = adapter();
        let mut before: [Port; 4] = Default::default();
        for r in reports(20_000) {
            a.shared.lock().unwrap().take(r, Instant::now());
            assert_eq!(a.ports(), ports_before(&r, &mut before));
        }
        // Turned on and off again, a port is as before too.
        a.set_phob(1, Some(melee_phob::Profile::default()));
        a.set_phob(1, None);
        for r in reports(1_000) {
            a.shared.lock().unwrap().take(r, Instant::now());
            assert_eq!(a.ports(), ports_before(&r, &mut before));
        }
    }

    /// A report with one wired controller in port 0.
    fn one(button: u16, raw: [u8; 6]) -> [u8; REPORT_LEN] {
        let mut r = [0u8; REPORT_LEN];
        r[0] = REPORT;
        r[1] = 0x10;
        let mut b = [0u8; 2];
        for (byte, bit, pad) in [(0, 0x01, 0x0100u16), (1, 0x08, 0x0040)] {
            if button & pad != 0 {
                b[byte] |= bit;
            }
        }
        r[2] = b[0];
        r[3] = b[1];
        r[4..10].copy_from_slice(&raw);
        r
    }

    #[test]
    fn phob_runs_every_report() {
        let a = adapter();
        // The L trigger in PhobGCC's digital-only mode.
        let mut profile = melee_phob::Profile::default();
        profile.settings.l_mode = 1;
        a.set_phob(0, Some(profile));
        let origin = [128, 126, 129, 127, 30, 25];
        let t = Instant::now();
        let mut s = a.shared.lock().unwrap();
        s.take(one(0, origin), t);
        drop(s);
        a.ports();
        // Uncalibrated, the sticks pass through about the controller's origin; the triggers are
        // PhobGCC's: L digital only, R's slider less its origin.
        for k in 1..=40u32 {
            a.shared.lock().unwrap().take(
                one(0x0040, [200, 60, 129, 127, 130, 25]),
                t + Duration::from_millis(8 * k as u64),
            );
        }
        let p = a.ports()[0].unwrap();
        assert_eq!((p.stick_x, p.stick_y), (72, -66));
        assert_eq!((p.substick_x, p.substick_y), (0, 0));
        assert_eq!((p.trigger_left, p.trigger_right), (0, 0));
        assert_eq!(p.button, 0x0040);
        assert_eq!(a.phob(0, |p| p.diag().samples().len()), Some(41));
        // Each report's step: the 8 ms between them.
        let steps: Vec<f32> = a
            .phob(0, |p| p.diag().samples().map(|s| s.step_ms).collect())
            .unwrap();
        assert_eq!(steps[40], 8.0);
        assert_eq!(a.phob(1, |_| ()), None);
    }
}
