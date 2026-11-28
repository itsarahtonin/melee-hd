//! Box controllers (milestone 7's task 3): the boxes melee-hd recognizes over USB by their
//! vendor and product ids ([`MODELS`], each line with its source), read without gilrs's
//! deadzone and decoded to the GameCube report the box sends through a GameCube cable for the
//! same press, so the `PADStatus` the game reads is the one it would read on a console; the
//! "No deadzone, exact" reading any other gamepad can be set to; and the keyboard's box layout,
//! HayBox's Melee mode (`melee_phob::haybox`, GPL-3.0-only) over the keys the player chose.
//!
//! **Over USB.** A box's firmware works out the GameCube report (its sticks' bytes, 128 the
//! centre) and then sends it in its USB report's format. HayBox's DInput mode sends each stick
//! byte `v` as a 16-bit axis `v × 257` over 0 to 65535 (the vertical axes flipped, `255 − v`),
//! each trigger's byte plus one, its buttons by number and the D-pad as a hat; its XInput mode
//! sends each stick as `(v − 128) × 257 + 128`, a signed 16-bit value, and each trigger as its
//! byte, or 255 for the trigger's click. However the system and gilrs scale those values, they
//! stay hundreds of steps apart in 65536, so each is read back to its byte exactly by rounding
//! ([`byte_of_fraction`], [`stick_of_xinput`]). gilrs's deadzone and jitter filter would move
//! them, so a box's events skip gilrs's filters (`Inputs::pump`).
//!
//! **Not recognized:** the B0XX's and the Frame1's own firmware, whose USB ids neither maker
//! publishes (a B0XX running HayBox is HayBox's). Either can still play with "No deadzone,
//! exact" set in its bindings.

use melee_phob::haybox;
use melee_sim::input::pad::PadStatus;

use crate::adapter;
use crate::bindings::{A, B, DOWN, L, LEFT, R, RIGHT, START, UP, X, Y, Z};

/// The report format a box sends over USB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// HayBox's DInput backend: a HID gamepad report (HayBox's `TUGamepad` on the Pico, its
    /// Joystick library on AVR): axes X, Y, Z, Rz, Rx, Ry, the D-pad as a hat, buttons by number.
    HayBoxDInput,
    /// HayBox's XInput backend (an Xbox 360 controller's report).
    HayBoxXInput,
}

/// A box melee-hd recognizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub vendor: u16,
    pub product: u16,
    /// Its name in the Controllers tab.
    pub name: &'static str,
    pub report: Report,
    /// Where the ids come from.
    pub source: &'static str,
}

/// The boxes recognized by their USB ids. HayBox's ids are its builds' (HayBox at 2cffcd23,
/// the firmware's 3.1.0): its XInput backend sets them itself; its DInput backend keeps the
/// board's, from the build platforms its `platformio.ini` names.
pub const MODELS: [Model; 3] = [
    Model {
        vendor: 0x2e8a,
        product: 0x000a,
        name: "Box controller (HayBox DInput)",
        report: Report::HayBoxDInput,
        source: "HayBox 2cffcd23, Pico/RP2040 builds (B0XX R4 and others): \
                 HAL/pico/src/comms/DInputBackend.cpp sets no ids, so the board's apply; \
                 platformio.ini builds them with maxgerhardt/platform-raspberrypi at 5e87ae34, \
                 whose boards/pico.json gives usb_vid 0x2e8a, usb_pid 0x000a",
    },
    Model {
        vendor: 0x2341,
        product: 0x8036,
        name: "Box controller (HayBox DInput)",
        report: Report::HayBoxDInput,
        source: "HayBox 2cffcd23, Arduino Leonardo builds (B0XX R1 and R2, LBX, GCCPCB, GCCMX, \
                 htangl: config/*/env.ini board = leonardo): dolphin/HayBox_DInput.ini names the \
                 device \"Arduino Leonardo\"; PlatformIO's atmelavr boards/leonardo.json gives \
                 the Leonardo's ids, hwids 0x2341 0x8036 first",
    },
    Model {
        vendor: 0x0738,
        product: 0x4726,
        name: "Box controller (HayBox XInput)",
        report: Report::HayBoxXInput,
        source: "HayBox 2cffcd23, HAL/pico/src/comms/XInputBackend.cpp: \
                 TinyUSBDevice.setID(0x0738, 0x4726)",
    },
];

/// The box with these USB ids, if melee-hd recognizes one.
pub fn model(vendor: u16, product: u16) -> Option<&'static Model> {
    MODELS
        .iter()
        .find(|m| m.vendor == vendor && m.product == product)
}

/// The GameCube report as a `PADStatus`: what the PAD library reads from a controller whose
/// origin is the report's rest (the sticks at 128, the triggers at 0), as a box's is through a
/// GameCube cable (`adapter::status`, the same arithmetic as the adapter's controllers).
pub fn status(button: u16, sticks: [u8; 4], triggers: [u8; 2]) -> PadStatus {
    let raw = [
        sticks[0],
        sticks[1],
        sticks[2],
        sticks[3],
        triggers[0],
        triggers[1],
    ];
    adapter::status(button, raw, [128, 128, 128, 128, 0, 0])
}

/// The `PADStatus` HayBox's Melee mode's report gives through a GameCube cable: its buttons as
/// HayBox's GameCube backend sends them (`buttonR` as Z), the sticks and triggers as bytes.
pub fn haybox_status(o: &haybox::Output) -> PadStatus {
    let mut button = 0;
    for (on, bit) in [
        (o.a, A),
        (o.b, B),
        (o.x, X),
        (o.y, Y),
        (o.z, Z),
        (o.l, L),
        (o.r, R),
        (o.start, START),
        (o.dpad_up, UP),
        (o.dpad_down, DOWN),
        (o.dpad_left, LEFT),
        (o.dpad_right, RIGHT),
    ] {
        if on {
            button |= bit;
        }
    }
    status(
        button,
        [o.stick_x, o.stick_y, o.c_x, o.c_y],
        [o.l_analog, o.r_analog],
    )
}

/// A value read as a fraction of its range (0 to 1) back to the byte the box sent: the nearest
/// of 256 steps.
pub fn byte_of_fraction(f: f64) -> u8 {
    (f * 255.0).round().clamp(0.0, 255.0) as u8
}

/// An XInput stick axis, as gilrs gives it (−1 to 1, up positive), back to the GameCube byte
/// HayBox sent as `(v − 128) × 257 + 128`.
pub fn stick_of_xinput(f: f32) -> u8 {
    let v = f64::from(f) * 32768.0;
    (((v - 128.0) / 257.0).round().clamp(-128.0, 127.0) + 128.0) as u8
}

/// A gamepad's axis in "No deadzone, exact" (−1 to 1 as gilrs gives it) as a stick's signed
/// value: the nearest of the 256 bytes across its range, less the centre.
pub fn exact_stick(f: f32) -> i32 {
    i32::from(byte_of_fraction((f64::from(f) + 1.0) / 2.0)) - 128
}

// ---- HayBox's DInput report ----

/// A DInput box's HID inputs as its report carries them: the six axes' values as bytes (each
/// the nearest of 256 steps over the axis's logical range), the buttons by number (bit n for
/// HID button n + 1), the hat's horizontal and vertical (−1, 0 or 1, up −1 as HID has it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hid {
    /// X, Y, Z, Rx, Ry, Rz.
    pub axes: [u8; 6],
    pub buttons: u32,
    pub hat: (i8, i8),
}

/// The axes' order in [`Hid::axes`].
pub const X_AXIS: usize = 0;
pub const Y_AXIS: usize = 1;
pub const Z_AXIS: usize = 2;
pub const RX_AXIS: usize = 3;
pub const RY_AXIS: usize = 4;
pub const RZ_AXIS: usize = 5;

impl Hid {
    /// HayBox's DInput report with nothing pressed: the sticks centred (the vertical axes
    /// flipped, 255 − 128), the triggers at 0 + 1. An axis gilrs has had no event for since
    /// the box connected still holds this.
    pub const REST: Hid = Hid {
        axes: [128, 127, 1, 128, 127, 1],
        buttons: 0,
        hat: (0, 0),
    };

    fn button(&self, n: u32) -> bool {
        self.buttons >> n & 1 == 1
    }

    /// The `PADStatus` this report means: the GameCube report HayBox worked it out from (its
    /// DInput backend's buttons 0 B, 1 A, 2 Y, 3 X, 4 Z, 5 R, 7 L, 8 Select as D-pad left as
    /// its GameCube backend sends it, 9 Start, 12 Home as D-pad right; the hat as the D-pad;
    /// each vertical axis flipped back; each trigger less the one it adds).
    ///
    /// The hat holds one direction of each axis, so a D-pad left and right held together (its
    /// D-pad layer's C-left and C-right) arrive as neither; over a GameCube cable both would be.
    pub fn haybox_status(&self) -> PadStatus {
        let mut button = 0;
        for (n, bit) in [
            (0, B),
            (1, A),
            (2, Y),
            (3, X),
            (4, Z),
            (5, R),
            (7, L),
            (8, LEFT),
            (9, START),
            (12, RIGHT),
        ] {
            if self.button(n) {
                button |= bit;
            }
        }
        let (hx, hy) = self.hat;
        for (on, bit) in [
            (hx < 0, LEFT),
            (hx > 0, RIGHT),
            (hy < 0, UP),
            (hy > 0, DOWN),
        ] {
            if on {
                button |= bit;
            }
        }
        let a = self.axes;
        status(
            button,
            [a[X_AXIS], 255 - a[Y_AXIS], a[RX_AXIS], 255 - a[RY_AXIS]],
            [a[Z_AXIS].saturating_sub(1), a[RZ_AXIS].saturating_sub(1)],
        )
    }
}

/// Which of a DInput box's HID inputs a native code is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidInput {
    Axis(usize),
    Button(u32),
    HatX,
    HatY,
}

/// The system gilrs reads devices through, for its native codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Windows.Gaming.Input's raw game controller (gilrs's default Windows backend): a code is
    /// its kind (0 button, 1 axis, 2 switch) shifted 16 left, then its index.
    Wgi,
    /// Linux's evdev: the event type (1 a key, 3 an absolute axis) shifted 16 left, then its
    /// code.
    Evdev,
    Other,
}

/// The system this build reads.
pub const PLATFORM: Platform = if cfg!(windows) {
    Platform::Wgi
} else if cfg!(target_os = "linux") {
    Platform::Evdev
} else {
    Platform::Other
};

/// Whether gilrs turns the vertical axes it names (the sticks' and the D-pad's) over on this
/// system (gilrs-core's `IS_Y_AXIS_REVERSED`, true on Windows Gaming Input, Linux and macOS),
/// which [`hid`] turns back.
pub const Y_REVERSED: bool = cfg!(any(windows, target_os = "linux", target_os = "macos"));

impl HidInput {
    /// The HID input a native code is on `platform`. Windows Gaming Input lists a raw
    /// controller's axes in DirectInput's order (X, Y, Z, Rx, Ry, Rz, as the SDL mappings gilrs
    /// applies to these devices number them) and its hat as switch 0, which gilrs splits into
    /// two codes; evdev names each axis (ABS_X to ABS_RZ, ABS_HAT0X and ABS_HAT0Y) and numbers a
    /// gamepad's buttons from BTN_GAMEPAD (a joystick's from BTN_JOYSTICK).
    pub fn of(platform: Platform, code: u32) -> Option<HidInput> {
        let (kind, n) = (code >> 16, code & 0xffff);
        match (platform, kind) {
            (Platform::Wgi, 0) => Some(HidInput::Button(n)),
            (Platform::Wgi, 1) if n < 6 => Some(HidInput::Axis(n as usize)),
            (Platform::Wgi, 2) => match n {
                0 => Some(HidInput::HatX),
                1 => Some(HidInput::HatY),
                _ => None,
            },
            (Platform::Evdev, 3) => match n {
                0..=5 => Some(HidInput::Axis(n as usize)),
                0x10 => Some(HidInput::HatX),
                0x11 => Some(HidInput::HatY),
                _ => None,
            },
            (Platform::Evdev, 1) => match n {
                0x130..=0x13f => Some(HidInput::Button(n - 0x130)),
                0x120..=0x12f => Some(HidInput::Button(n - 0x120)),
                0x2c0..=0x2e7 => Some(HidInput::Button(n - 0x2c0 + 16)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A value gilrs holds for one of a device's native codes, and how gilrs worked it out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Raw {
    /// An axis's value, −1 to 1 over the axis's range (`axis_value`), turned over if gilrs
    /// named it a vertical axis on a system that turns those.
    Axis { value: f32, turned: bool },
    /// An axis gilrs names a button (a trigger): 0 to 1 over its range (`btn_value`).
    Travel(f32),
    /// A button.
    Pressed(bool),
}

/// The DInput box's HID inputs from gilrs's values for its codes, over the report's rest for
/// any gilrs has seen no change in.
pub fn hid(platform: Platform, values: impl IntoIterator<Item = (u32, Raw)>) -> Hid {
    let mut h = Hid::REST;
    for (code, raw) in values {
        let Some(input) = HidInput::of(platform, code) else {
            continue;
        };
        // The value as a fraction of its range, and as a press.
        let (fraction, pressed) = match raw {
            Raw::Axis { value, turned } => {
                let v = if turned { -value } else { value };
                ((f64::from(v) + 1.0) / 2.0, v > 0.0)
            }
            Raw::Travel(v) => (f64::from(v), v > 0.5),
            Raw::Pressed(p) => (if p { 1.0 } else { 0.0 }, p),
        };
        match input {
            HidInput::Axis(n) => h.axes[n] = byte_of_fraction(fraction),
            HidInput::Button(n) if n < 32 => {
                if pressed {
                    h.buttons |= 1 << n;
                } else {
                    h.buttons &= !(1 << n);
                }
            }
            HidInput::Button(_) => {}
            HidInput::HatX | HidInput::HatY => {
                let v = if fraction < 0.25 {
                    -1
                } else if fraction > 0.75 {
                    1
                } else {
                    0
                };
                if input == HidInput::HatX {
                    h.hat.0 = v;
                } else {
                    h.hat.1 = v;
                }
            }
        }
    }
    h
}

/// gilrs's values for a gamepad's codes, as [`hid`] takes them.
pub fn gilrs_values(gp: &gilrs::Gamepad<'_>) -> Vec<(u32, Raw)> {
    use gilrs::Axis;
    use gilrs::ev::AxisOrBtn;
    let vertical = |code| {
        matches!(
            gp.axis_or_btn_name(code),
            Some(AxisOrBtn::Axis(
                Axis::LeftStickY | Axis::RightStickY | Axis::DPadY
            ))
        )
    };
    let state = gp.state();
    let mut out = Vec::new();
    for (code, data) in state.axes() {
        let value = data.value();
        out.push((
            code.into_u32(),
            Raw::Axis {
                value,
                turned: Y_REVERSED && vertical(code) && value != 0.0,
            },
        ));
    }
    for (code, data) in state.buttons() {
        let n = code.into_u32();
        let axis_kind = match PLATFORM {
            Platform::Wgi => n >> 16 == 1,
            Platform::Evdev => n >> 16 == 3,
            Platform::Other => false,
        };
        out.push((
            n,
            if axis_kind {
                Raw::Travel(data.value())
            } else {
                Raw::Pressed(data.is_pressed())
            },
        ));
    }
    out
}

// ---- HayBox's XInput report ----

/// An XInput box's values as gilrs names them: the sticks (−1 to 1, up positive), the
/// triggers' travel (0 to 1) and the buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct XInput {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub triggers: (f32, f32),
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    /// The right bumper, HayBox's `buttonR`: Z.
    pub rb: bool,
    pub start: bool,
    /// Back, HayBox's Select: D-pad left, as its GameCube backend sends it.
    pub back: bool,
    /// The guide button, HayBox's Home: D-pad right.
    pub guide: bool,
    pub dpad: [bool; 4],
}

impl XInput {
    /// What gilrs holds for the box.
    pub fn of(gp: &gilrs::Gamepad<'_>) -> XInput {
        use gilrs::{Axis, Button};
        let travel = |b| gp.button_data(b).map_or(0.0, |d| d.value());
        XInput {
            left: (gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY)),
            right: (gp.value(Axis::RightStickX), gp.value(Axis::RightStickY)),
            triggers: (travel(Button::LeftTrigger2), travel(Button::RightTrigger2)),
            a: gp.is_pressed(Button::South),
            b: gp.is_pressed(Button::East),
            x: gp.is_pressed(Button::West),
            y: gp.is_pressed(Button::North),
            rb: gp.is_pressed(Button::RightTrigger),
            start: gp.is_pressed(Button::Start),
            back: gp.is_pressed(Button::Select),
            guide: gp.is_pressed(Button::Mode),
            dpad: [
                gp.is_pressed(Button::DPadUp),
                gp.is_pressed(Button::DPadDown),
                gp.is_pressed(Button::DPadLeft),
                gp.is_pressed(Button::DPadRight),
            ],
        }
    }

    /// The `PADStatus` this report means. A trigger at 255 is HayBox's click, which over a
    /// GameCube cable is the click with the analog value 140 (the Melee mode's); any other
    /// value is the analog value alone.
    pub fn haybox_status(&self) -> PadStatus {
        let mut button = 0;
        let trigger = |v: f32, bit: u16, button: &mut u16| {
            let t = byte_of_fraction(f64::from(v));
            if t == 255 {
                *button |= bit;
                140
            } else {
                t
            }
        };
        let l = trigger(self.triggers.0, L, &mut button);
        let r = trigger(self.triggers.1, R, &mut button);
        let [up, down, left, right] = self.dpad;
        for (on, bit) in [
            (self.a, A),
            (self.b, B),
            (self.x, X),
            (self.y, Y),
            (self.rb, Z),
            (self.start, START),
            (up, UP),
            (down, DOWN),
            (left || self.back, LEFT),
            (right || self.guide, RIGHT),
        ] {
            if on {
                button |= bit;
            }
        }
        status(
            button,
            [
                stick_of_xinput(self.left.0),
                stick_of_xinput(self.left.1),
                stick_of_xinput(self.right.0),
                stick_of_xinput(self.right.1),
            ],
            [l, r],
        )
    }
}

// ---- The keyboard's box layout ----

/// Each box button's name in `settings.txt`: HayBox's.
pub fn button_key(b: haybox::Button) -> &'static str {
    use haybox::Button::*;
    match b {
        Lf1 => "lf1",
        Lf2 => "lf2",
        Lf3 => "lf3",
        Lf4 => "lf4",
        Lt1 => "lt1",
        Lt2 => "lt2",
        Mb1 => "mb1",
        Mb2 => "mb2",
        Mb3 => "mb3",
        Rt1 => "rt1",
        Rt2 => "rt2",
        Rt3 => "rt3",
        Rt4 => "rt4",
        Rt5 => "rt5",
        Rf1 => "rf1",
        Rf2 => "rf2",
        Rf3 => "rf3",
        Rf4 => "rf4",
        Rf5 => "rf5",
        Rf6 => "rf6",
        Rf7 => "rf7",
        Rf8 => "rf8",
    }
}

/// The keyboard layout's name in the Controllers tab.
pub const LAYOUT_NAME: &str = "Box layout (B0XX)";

/// A box layout: the key for each of the box's buttons.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoxKeys {
    keys: [Option<String>; haybox::Button::ALL.len()],
}

fn index(b: haybox::Button) -> usize {
    haybox::Button::ALL
        .iter()
        .position(|x| *x == b)
        .expect("every button is listed")
}

impl Default for BoxKeys {
    /// The B0XX's layout on a keyboard: the left hand's four fingers on A S D F (L, left,
    /// down, right), Mod X and Mod Y on V and C under the left thumb; the right hand's fingers
    /// on J K L ; (B, X, Z, up) with U I O P above them (R, Y, light shield, mid shield); A
    /// on Space and the C-buttons around N (H up, B left, N down, M right); Start on Enter.
    fn default() -> BoxKeys {
        use haybox::Button::*;
        let mut k = BoxKeys {
            keys: std::array::from_fn(|_| None),
        };
        for (b, key) in [
            (Lf4, "KeyA"),
            (Lf3, "KeyS"),
            (Lf2, "KeyD"),
            (Lf1, "KeyF"),
            (Lt1, "KeyV"),
            (Lt2, "KeyC"),
            (Mb1, "Enter"),
            (Rt1, "Space"),
            (Rt2, "KeyN"),
            (Rt3, "KeyB"),
            (Rt4, "KeyH"),
            (Rt5, "KeyM"),
            (Rf1, "KeyJ"),
            (Rf2, "KeyK"),
            (Rf3, "KeyL"),
            (Rf4, "Semicolon"),
            (Rf5, "KeyU"),
            (Rf6, "KeyI"),
            (Rf7, "KeyO"),
            (Rf8, "KeyP"),
        ] {
            k.set(b, Some(key.to_owned()));
        }
        k
    }
}

impl BoxKeys {
    pub fn get(&self, b: haybox::Button) -> Option<&str> {
        self.keys[index(b)].as_deref()
    }

    pub fn set(&mut self, b: haybox::Button, key: Option<String>) {
        self.keys[index(b)] = key;
    }

    /// Whether a button is on the key `name`.
    pub fn uses_key(&self, name: &str) -> bool {
        self.keys.iter().any(|k| k.as_deref() == Some(name))
    }

    /// The buttons held, given which keys are down.
    pub fn inputs(&self, down: impl Fn(&str) -> bool) -> haybox::Inputs {
        let mut i = haybox::Inputs::default();
        for b in haybox::Button::ALL {
            if let Some(k) = self.get(b) {
                i.set(b, down(k));
            }
        }
        i
    }

    /// As `settings.txt` keeps it: `BUTTON=key:KEY` for each button, `none` for a button on no
    /// key.
    pub fn save(&self) -> String {
        haybox::Button::ALL
            .into_iter()
            .map(|b| match self.get(b) {
                Some(k) => format!("{}=key:{k}", button_key(b)),
                None => format!("{}=none", button_key(b)),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// [`BoxKeys::save`]'s layout over the default; none if a pair can't be read.
    pub fn restore(text: &str) -> Option<BoxKeys> {
        let mut k = BoxKeys::default();
        for pair in text.split_whitespace() {
            let (b, v) = pair.split_once('=')?;
            let b = haybox::Button::ALL
                .into_iter()
                .find(|x| button_key(*x) == b)?;
            let key = match v {
                "none" => None,
                v => Some(v.strip_prefix("key:").filter(|k| !k.is_empty())?.to_owned()),
            };
            k.set(b, key);
        }
        Some(k)
    }
}

#[cfg(test)]
mod tests;
