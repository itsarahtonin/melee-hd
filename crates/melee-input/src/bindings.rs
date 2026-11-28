//! Bindings: which of a device's own inputs plays each GameCube input ([`Target`]), for every
//! kind of device: a key, a gamepad's button or one direction of its axis, or (for a GameCube
//! controller on the adapter) the controller's own input. Milestone 7's design call 1: this is
//! the one place buttons are remapped, the adapter's controllers included.
//!
//! [`Bindings::pad`] turns a device's inputs into the `PADStatus` it plays, through a
//! [`Reader`] for the device. Each device kind's default bindings give exactly what melee-hd
//! read before bindings existed, value for value (the tests check each against the old code).

use melee_sim::input::pad::{ERR_NONE, PadStatus};

// PADStatus buttons.
pub(crate) const LEFT: u16 = 0x0001;
pub(crate) const RIGHT: u16 = 0x0002;
pub(crate) const DOWN: u16 = 0x0004;
pub(crate) const UP: u16 = 0x0008;
pub(crate) const Z: u16 = 0x0010;
pub(crate) const R: u16 = 0x0020;
pub(crate) const L: u16 = 0x0040;
pub(crate) const A: u16 = 0x0100;
pub(crate) const B: u16 = 0x0200;
pub(crate) const X: u16 = 0x0400;
pub(crate) const Y: u16 = 0x0800;
pub(crate) const START: u16 = 0x1000;

/// A GameCube controller's input, as a binding's target: each button, each direction of the
/// two sticks, each trigger's analog travel and its click, and the modifier that holds digital
/// directions at half reach (the keyboard's walk key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    A,
    B,
    X,
    Y,
    Z,
    LAnalog,
    LClick,
    RAnalog,
    RClick,
    Start,
    StickUp,
    StickDown,
    StickLeft,
    StickRight,
    CUp,
    CDown,
    CLeft,
    CRight,
    DUp,
    DDown,
    DLeft,
    DRight,
    Modifier,
}

/// How many targets there are.
pub const TARGETS: usize = 23;

impl Target {
    /// Every target, in the order Rebind… asks for them.
    pub const ALL: [Target; TARGETS] = [
        Target::A,
        Target::B,
        Target::X,
        Target::Y,
        Target::Z,
        Target::LAnalog,
        Target::LClick,
        Target::RAnalog,
        Target::RClick,
        Target::Start,
        Target::StickUp,
        Target::StickDown,
        Target::StickLeft,
        Target::StickRight,
        Target::CUp,
        Target::CDown,
        Target::CLeft,
        Target::CRight,
        Target::DUp,
        Target::DDown,
        Target::DLeft,
        Target::DRight,
        Target::Modifier,
    ];

    fn index(self) -> usize {
        self as usize
    }

    /// Its name in the Controllers tab.
    pub fn name(self) -> &'static str {
        match self {
            Target::A => "A",
            Target::B => "B",
            Target::X => "X",
            Target::Y => "Y",
            Target::Z => "Z",
            Target::LAnalog => "L (analog)",
            Target::LClick => "L (click)",
            Target::RAnalog => "R (analog)",
            Target::RClick => "R (click)",
            Target::Start => "Start",
            Target::StickUp => "Stick up",
            Target::StickDown => "Stick down",
            Target::StickLeft => "Stick left",
            Target::StickRight => "Stick right",
            Target::CUp => "C-stick up",
            Target::CDown => "C-stick down",
            Target::CLeft => "C-stick left",
            Target::CRight => "C-stick right",
            Target::DUp => "D-pad up",
            Target::DDown => "D-pad down",
            Target::DLeft => "D-pad left",
            Target::DRight => "D-pad right",
            Target::Modifier => "Half stick (modifier)",
        }
    }

    /// Its name in `settings.txt`.
    fn key(self) -> &'static str {
        match self {
            Target::A => "a",
            Target::B => "b",
            Target::X => "x",
            Target::Y => "y",
            Target::Z => "z",
            Target::LAnalog => "l-analog",
            Target::LClick => "l",
            Target::RAnalog => "r-analog",
            Target::RClick => "r",
            Target::Start => "start",
            Target::StickUp => "up",
            Target::StickDown => "down",
            Target::StickLeft => "left",
            Target::StickRight => "right",
            Target::CUp => "c-up",
            Target::CDown => "c-down",
            Target::CLeft => "c-left",
            Target::CRight => "c-right",
            Target::DUp => "d-up",
            Target::DDown => "d-down",
            Target::DLeft => "d-left",
            Target::DRight => "d-right",
            Target::Modifier => "modifier",
        }
    }

    fn parse(v: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.key() == v)
    }

    /// The `PADStatus` button it sets, for a digital target.
    pub fn bit(self) -> Option<u16> {
        Some(match self {
            Target::A => A,
            Target::B => B,
            Target::X => X,
            Target::Y => Y,
            Target::Z => Z,
            Target::LClick => L,
            Target::RClick => R,
            Target::Start => START,
            Target::DUp => UP,
            Target::DDown => DOWN,
            Target::DLeft => LEFT,
            Target::DRight => RIGHT,
            _ => return None,
        })
    }

    /// What kind of value it takes.
    pub fn kind(self) -> Kind {
        match self {
            Target::StickUp
            | Target::StickDown
            | Target::StickLeft
            | Target::StickRight
            | Target::CUp
            | Target::CDown
            | Target::CLeft
            | Target::CRight => Kind::Axis,
            Target::LAnalog | Target::RAnalog => Kind::Trigger,
            _ => Kind::Digital,
        }
    }

    /// A trigger's click: a GameCube trigger clicks at the end of its travel, so an analog
    /// source presses it only near its end.
    fn click(self) -> bool {
        matches!(self, Target::LClick | Target::RClick)
    }
}

/// What kind of value an input carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Pressed or not.
    Digital,
    /// One direction of a stick, 0 to 127.
    Axis,
    /// A trigger's travel, 0 to 255.
    Trigger,
}

/// One of a device's own inputs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// A key, by the window's name for its key code (winit's `KeyCode`, as `KeyX`).
    Key(String),
    /// A gamepad's button (gilrs's names; triggers carry their travel).
    Button(gilrs::Button),
    /// One direction of a gamepad's axis: positive with `true`.
    Axis(gilrs::Axis, bool),
    /// A GameCube controller's own input, on the adapter ([`Target::Modifier`] isn't one).
    Gc(Target),
}

/// The gamepad buttons a binding can name.
pub const BUTTONS: [gilrs::Button; 19] = {
    use gilrs::Button::*;
    [
        South,
        East,
        North,
        West,
        C,
        Z,
        LeftTrigger,
        LeftTrigger2,
        RightTrigger,
        RightTrigger2,
        Select,
        Start,
        Mode,
        LeftThumb,
        RightThumb,
        DPadUp,
        DPadDown,
        DPadLeft,
        DPadRight,
    ]
};

/// The gamepad axes a binding can name.
pub const AXES: [gilrs::Axis; 8] = {
    use gilrs::Axis::*;
    [
        LeftStickX,
        LeftStickY,
        LeftZ,
        RightStickX,
        RightStickY,
        RightZ,
        DPadX,
        DPadY,
    ]
};

impl Source {
    /// What kind of value it carries: a gamepad's trigger buttons carry their travel.
    pub fn kind(&self) -> Kind {
        match self {
            Source::Key(_) => Kind::Digital,
            Source::Button(b) => {
                if matches!(
                    b,
                    gilrs::Button::LeftTrigger2 | gilrs::Button::RightTrigger2
                ) {
                    Kind::Trigger
                } else {
                    Kind::Digital
                }
            }
            Source::Axis(..) => Kind::Axis,
            Source::Gc(t) => t.kind(),
        }
    }

    /// Its name in `settings.txt`: `key:KeyX`, `button:South`, `axis:LeftStickX+`, `gc:a`.
    pub fn key(&self) -> String {
        match self {
            Source::Key(k) => format!("key:{k}"),
            Source::Button(b) => format!("button:{b:?}"),
            Source::Axis(a, pos) => format!("axis:{a:?}{}", if *pos { '+' } else { '-' }),
            Source::Gc(t) => format!("gc:{}", t.key()),
        }
    }

    /// [`Source::key`]'s source.
    pub fn parse(v: &str) -> Option<Source> {
        let (kind, name) = v.split_once(':')?;
        match kind {
            "key" if !name.is_empty() && !name.contains(char::is_whitespace) => {
                Some(Source::Key(name.to_owned()))
            }
            "button" => BUTTONS
                .into_iter()
                .find(|b| format!("{b:?}") == name)
                .map(Source::Button),
            "axis" => {
                let (axis, sign) = name.split_at(name.len().checked_sub(1)?);
                let pos = match sign {
                    "+" => true,
                    "-" => false,
                    _ => return None,
                };
                AXES.into_iter()
                    .find(|a| format!("{a:?}") == axis)
                    .map(|a| Source::Axis(a, pos))
            }
            "gc" => Target::parse(name)
                .filter(|t| *t != Target::Modifier)
                .map(Source::Gc),
            _ => None,
        }
    }

    /// Its name in the Controllers tab.
    pub fn name(&self) -> String {
        match self {
            Source::Key(k) => key_name(k),
            Source::Button(b) => button_name(*b).to_owned(),
            Source::Axis(a, pos) => axis_name(*a, *pos).to_owned(),
            Source::Gc(t) => t.name().to_owned(),
        }
    }
}

/// A key's name for the player, from winit's: `KeyX` is X, `Digit1` is 1, `ArrowLeft` is Left
/// arrow, `ShiftLeft` is Left Shift.
pub fn key_name(k: &str) -> String {
    if let Some(c) = k.strip_prefix("Key").filter(|c| c.len() == 1) {
        return c.to_owned();
    }
    if let Some(d) = k.strip_prefix("Digit").filter(|d| d.len() == 1) {
        return d.to_owned();
    }
    if let Some(d) = k.strip_prefix("Arrow") {
        return format!("{d} arrow");
    }
    if let Some(d) = k.strip_prefix("Numpad") {
        return format!("Numpad {d}");
    }
    for side in ["Left", "Right"] {
        if let Some(m) = k.strip_suffix(side) {
            return format!("{side} {m}");
        }
    }
    k.to_owned()
}

fn button_name(b: gilrs::Button) -> &'static str {
    use gilrs::Button::*;
    match b {
        South => "Bottom face button",
        East => "Right face button",
        North => "Top face button",
        West => "Left face button",
        C => "C button",
        Z => "Z button",
        LeftTrigger => "Left bumper",
        LeftTrigger2 => "Left trigger",
        RightTrigger => "Right bumper",
        RightTrigger2 => "Right trigger",
        Select => "Back",
        Start => "Start",
        Mode => "Guide",
        LeftThumb => "Left stick press",
        RightThumb => "Right stick press",
        DPadUp => "D-pad up",
        DPadDown => "D-pad down",
        DPadLeft => "D-pad left",
        DPadRight => "D-pad right",
        Unknown => "Unknown button",
    }
}

fn axis_name(a: gilrs::Axis, pos: bool) -> &'static str {
    use gilrs::Axis::*;
    match (a, pos) {
        (LeftStickX, true) => "Left stick right",
        (LeftStickX, false) => "Left stick left",
        (LeftStickY, true) => "Left stick up",
        (LeftStickY, false) => "Left stick down",
        (RightStickX, true) => "Right stick right",
        (RightStickX, false) => "Right stick left",
        (RightStickY, true) => "Right stick up",
        (RightStickY, false) => "Right stick down",
        (LeftZ, true) => "Left trigger axis",
        (LeftZ, false) => "Left trigger axis (reversed)",
        (RightZ, true) => "Right trigger axis",
        (RightZ, false) => "Right trigger axis (reversed)",
        (DPadX, true) => "D-pad axis right",
        (DPadX, false) => "D-pad axis left",
        (DPadY, true) => "D-pad axis up",
        (DPadY, false) => "D-pad axis down",
        (Unknown, _) => "Unknown axis",
    }
}

/// A source's value as each kind of target takes it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reading {
    /// As a stick's direction: 0 to 127 (128 for an adapter stick's full -128).
    pub axis: f32,
    /// As a trigger's travel: 0 to 255.
    pub trigger: f32,
    /// As a button.
    pub pressed: bool,
}

/// Reads a device's sources.
pub trait Reader {
    /// `source` now: `click` for a trigger's click (an analog source presses it near its end
    /// only), `reach` a digital source's stick direction (127, or 64 with the modifier held).
    fn read(&self, source: &Source, click: bool, reach: f32) -> Reading;
}

/// A digital source's reading.
pub(crate) fn digital(pressed: bool, reach: f32) -> Reading {
    if pressed {
        Reading {
            axis: reach,
            trigger: 255.0,
            pressed,
        }
    } else {
        Reading::default()
    }
}

/// What a gamepad shows a binding (gilrs's gamepad, or a test's).
pub trait GamepadState {
    fn is_pressed(&self, b: gilrs::Button) -> bool;
    /// A button's travel, 0 to 1 (its press for a digital one).
    fn button_value(&self, b: gilrs::Button) -> f32;
    fn axis_value(&self, a: gilrs::Axis) -> f32;
}

impl GamepadState for gilrs::Gamepad<'_> {
    fn is_pressed(&self, b: gilrs::Button) -> bool {
        gilrs::Gamepad::is_pressed(self, b)
    }

    fn button_value(&self, b: gilrs::Button) -> f32 {
        self.button_data(b).map_or(0.0, |d| d.value())
    }

    fn axis_value(&self, a: gilrs::Axis) -> f32 {
        self.value(a)
    }
}

/// A gamepad as a [`Reader`]: its state, and the buttons pressed since the last read (a tap
/// shorter than a tick still counts once).
pub struct GamepadReader<'a, S> {
    pub state: &'a S,
    pub taps: &'a [gilrs::Button],
    /// "No deadzone, exact" (its state then unfiltered too, `Inputs::pump`): each axis the
    /// nearest of the 256 bytes across its range, less the centre, and each trigger's travel the
    /// nearest of 256, so a device sending the GameCube's bytes plays them as they are.
    pub exact: bool,
}

impl<S: GamepadState> Reader for GamepadReader<'_, S> {
    fn read(&self, source: &Source, click: bool, reach: f32) -> Reading {
        let travel = |v: f32| {
            if self.exact {
                f32::from(crate::boxes::byte_of_fraction(f64::from(v)))
            } else {
                v * 255.0
            }
        };
        match *source {
            Source::Button(b) => {
                let value = self.state.button_value(b);
                let held = self.state.is_pressed(b) || self.taps.contains(&b);
                Reading {
                    axis: if held { reach } else { value * 127.0 },
                    trigger: travel(value),
                    // A trigger clicks at the end of its travel, as a GameCube trigger's does.
                    pressed: if click { value > 0.95 } else { held },
                }
            }
            Source::Axis(a, pos) => {
                let v = self.state.axis_value(a).clamp(-1.0, 1.0);
                let half = if pos { v.max(0.0) } else { (-v).max(0.0) };
                let axis = if self.exact {
                    let s = crate::boxes::exact_stick(v);
                    (if pos { s.max(0) } else { (-s).max(0) }) as f32
                } else {
                    half * 127.0
                };
                Reading {
                    axis,
                    trigger: travel(half),
                    pressed: half >= 0.5,
                }
            }
            Source::Key(_) | Source::Gc(_) => Reading::default(),
        }
    }
}

/// A GameCube controller's report on the adapter (as `PADRead` gives it) as a [`Reader`].
pub struct GcReader<'a>(pub &'a PadStatus);

impl Reader for GcReader<'_> {
    fn read(&self, source: &Source, click: bool, reach: f32) -> Reading {
        let Source::Gc(t) = *source else {
            return Reading::default();
        };
        let p = self.0;
        let half = |v: i8, pos: bool| {
            let v = i16::from(v);
            f32::from(if pos { v.max(0) } else { (-v).max(0) })
        };
        let stick = |h: f32| Reading {
            axis: h,
            trigger: (h * 255.0 / 127.0).min(255.0),
            pressed: h >= 64.0,
        };
        let trigger = |t: u8| {
            let t = f32::from(t);
            Reading {
                axis: t * 127.0 / 255.0,
                trigger: t,
                pressed: t > 0.95 * 255.0 || (!click && t >= 128.0),
            }
        };
        match t {
            Target::StickUp => stick(half(p.stick_y, true)),
            Target::StickDown => stick(half(p.stick_y, false)),
            Target::StickLeft => stick(half(p.stick_x, false)),
            Target::StickRight => stick(half(p.stick_x, true)),
            Target::CUp => stick(half(p.substick_y, true)),
            Target::CDown => stick(half(p.substick_y, false)),
            Target::CLeft => stick(half(p.substick_x, false)),
            Target::CRight => stick(half(p.substick_x, true)),
            Target::LAnalog => trigger(p.trigger_left),
            Target::RAnalog => trigger(p.trigger_right),
            Target::Modifier => Reading::default(),
            t => digital(t.bit().is_some_and(|b| p.button & b != 0), reach),
        }
    }
}

/// A device's bindings: each target's source, if any.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Bindings {
    sources: [Option<Source>; TARGETS],
}

impl Bindings {
    /// Nothing bound.
    pub fn none() -> Bindings {
        Bindings {
            sources: std::array::from_fn(|_| None),
        }
    }

    fn of(pairs: impl IntoIterator<Item = (Target, Source)>) -> Bindings {
        let mut b = Bindings::none();
        for (t, s) in pairs {
            b.set(t, Some(s));
        }
        b
    }

    /// The keyboard's: arrows, IJKL for the C-stick, X Z C S for A B X Y, D for Z, Q W for L
    /// and R, Enter for Start, TFGH for the D-pad, left Shift for half the stick.
    pub fn keyboard() -> Bindings {
        let k = |name: &str| Source::Key(name.to_owned());
        Bindings::of([
            (Target::A, k("KeyX")),
            (Target::B, k("KeyZ")),
            (Target::X, k("KeyC")),
            (Target::Y, k("KeyS")),
            (Target::Z, k("KeyD")),
            (Target::LAnalog, k("KeyQ")),
            (Target::LClick, k("KeyQ")),
            (Target::RAnalog, k("KeyW")),
            (Target::RClick, k("KeyW")),
            (Target::Start, k("Enter")),
            (Target::StickUp, k("ArrowUp")),
            (Target::StickDown, k("ArrowDown")),
            (Target::StickLeft, k("ArrowLeft")),
            (Target::StickRight, k("ArrowRight")),
            (Target::CUp, k("KeyI")),
            (Target::CDown, k("KeyK")),
            (Target::CLeft, k("KeyJ")),
            (Target::CRight, k("KeyL")),
            (Target::DUp, k("KeyT")),
            (Target::DDown, k("KeyG")),
            (Target::DLeft, k("KeyF")),
            (Target::DRight, k("KeyH")),
            (Target::Modifier, k("ShiftLeft")),
        ])
    }

    /// A standard gamepad's (Xbox, PlayStation and Switch Pro layouts): the face buttons by
    /// position (bottom A, left B, right X, top Y), the right bumper Z, the triggers L and R
    /// with their click at the end of their travel, the sticks and the D-pad as themselves.
    pub fn gamepad() -> Bindings {
        use gilrs::{Axis, Button};
        Bindings::of([
            (Target::A, Source::Button(Button::South)),
            (Target::B, Source::Button(Button::West)),
            (Target::X, Source::Button(Button::East)),
            (Target::Y, Source::Button(Button::North)),
            (Target::Z, Source::Button(Button::RightTrigger)),
            (Target::LAnalog, Source::Button(Button::LeftTrigger2)),
            (Target::LClick, Source::Button(Button::LeftTrigger2)),
            (Target::RAnalog, Source::Button(Button::RightTrigger2)),
            (Target::RClick, Source::Button(Button::RightTrigger2)),
            (Target::Start, Source::Button(Button::Start)),
            (Target::StickUp, Source::Axis(Axis::LeftStickY, true)),
            (Target::StickDown, Source::Axis(Axis::LeftStickY, false)),
            (Target::StickLeft, Source::Axis(Axis::LeftStickX, false)),
            (Target::StickRight, Source::Axis(Axis::LeftStickX, true)),
            (Target::CUp, Source::Axis(Axis::RightStickY, true)),
            (Target::CDown, Source::Axis(Axis::RightStickY, false)),
            (Target::CLeft, Source::Axis(Axis::RightStickX, false)),
            (Target::CRight, Source::Axis(Axis::RightStickX, true)),
            (Target::DUp, Source::Button(Button::DPadUp)),
            (Target::DDown, Source::Button(Button::DPadDown)),
            (Target::DLeft, Source::Button(Button::DPadLeft)),
            (Target::DRight, Source::Button(Button::DPadRight)),
        ])
    }

    /// A GameCube controller's: each input as itself.
    pub fn gamecube() -> Bindings {
        Bindings::of(
            Target::ALL
                .into_iter()
                .filter(|t| *t != Target::Modifier)
                .map(|t| (t, Source::Gc(t))),
        )
    }

    pub fn get(&self, t: Target) -> Option<&Source> {
        self.sources[t.index()].as_ref()
    }

    pub fn set(&mut self, t: Target, s: Option<Source>) {
        self.sources[t.index()] = s;
    }

    /// Whether a target is bound to the key `name`.
    pub fn uses_key(&self, name: &str) -> bool {
        (self.sources.iter()).any(|s| matches!(s, Some(Source::Key(k)) if k == name))
    }

    /// The `PADStatus` the device plays, read through `r`.
    pub fn pad(&self, r: &impl Reader) -> PadStatus {
        let read = |t: Target, reach: f32| {
            self.get(t)
                .map_or_else(Reading::default, |s| r.read(s, t.click(), reach))
        };
        let reach = if read(Target::Modifier, 127.0).pressed {
            64.0
        } else {
            127.0
        };
        let mut pad = PadStatus {
            err: ERR_NONE,
            ..PadStatus::default()
        };
        for t in Target::ALL {
            if let Some(bit) = t.bit()
                && read(t, reach).pressed
            {
                pad.button |= bit;
            }
        }
        // Each axis is its positive direction less its negative one: both held give neither.
        let axis = |neg: Target, pos: Target| (read(pos, reach).axis - read(neg, reach).axis) as i8;
        pad.stick_x = axis(Target::StickLeft, Target::StickRight);
        pad.stick_y = axis(Target::StickDown, Target::StickUp);
        pad.substick_x = axis(Target::CLeft, Target::CRight);
        pad.substick_y = axis(Target::CDown, Target::CUp);
        pad.trigger_left = read(Target::LAnalog, reach).trigger as u8;
        pad.trigger_right = read(Target::RAnalog, reach).trigger as u8;
        pad
    }

    /// The bindings as `settings.txt` keeps them after the device: `TARGET=SOURCE` for each
    /// target, `none` where nothing is bound.
    pub fn save(&self) -> String {
        Target::ALL
            .into_iter()
            .map(|t| {
                let s = self.get(t).map_or_else(|| "none".to_owned(), Source::key);
                format!("{}={s}", t.key())
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// [`Bindings::save`]'s bindings over `self`; none if a pair can't be read.
    pub fn restore(&self, text: &str) -> Option<Bindings> {
        let mut b = self.clone();
        for pair in text.split_whitespace() {
            let (t, s) = pair.split_once('=')?;
            let t = Target::parse(t)?;
            let s = if s == "none" {
                None
            } else {
                Some(Source::parse(s)?)
            };
            b.set(t, s);
        }
        Some(b)
    }
}

/// Of the sources just pressed, the one Rebind… takes for `target`: one of the target's own
/// kind first (a trigger's travel for L (analog), a stick's direction for a stick), then a
/// button.
pub fn pick(target: Target, pressed: &[Source]) -> Option<Source> {
    let want = target.kind();
    let rank = |s: &Source| {
        let k = s.kind();
        if k == want {
            0
        } else if k == Kind::Digital {
            1
        } else {
            2
        }
    };
    pressed.iter().min_by_key(|s| rank(s)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gamepad's state for the tests.
    #[derive(Default)]
    pub(crate) struct Fake {
        pub pressed: Vec<gilrs::Button>,
        pub values: Vec<(gilrs::Button, f32)>,
        pub axes: Vec<(gilrs::Axis, f32)>,
    }

    impl GamepadState for Fake {
        fn is_pressed(&self, b: gilrs::Button) -> bool {
            self.pressed.contains(&b)
        }
        fn button_value(&self, b: gilrs::Button) -> f32 {
            self.values
                .iter()
                .find(|v| v.0 == b)
                .map_or(if self.is_pressed(b) { 1.0 } else { 0.0 }, |v| v.1)
        }
        fn axis_value(&self, a: gilrs::Axis) -> f32 {
            self.axes.iter().find(|v| v.0 == a).map_or(0.0, |v| v.1)
        }
    }

    /// melee-hd's gamepad mapping before bindings (`gamepad` in `lib.rs` at 3846341f).
    fn old_gamepad(gp: &Fake, tapped: &[gilrs::Button]) -> PadStatus {
        use gilrs::{Axis, Button};
        let mut pad = PadStatus::default();
        let bits = [
            (Button::South, A),
            (Button::West, B),
            (Button::East, X),
            (Button::North, Y),
            (Button::RightTrigger, Z),
            (Button::Start, START),
            (Button::DPadLeft, LEFT),
            (Button::DPadRight, RIGHT),
            (Button::DPadDown, DOWN),
            (Button::DPadUp, UP),
        ];
        for (b, bit) in bits {
            if gp.is_pressed(b) || tapped.contains(&b) {
                pad.button |= bit;
            }
        }
        let axis = |a: Axis| (gp.axis_value(a).clamp(-1.0, 1.0) * 127.0) as i8;
        pad.stick_x = axis(Axis::LeftStickX);
        pad.stick_y = axis(Axis::LeftStickY);
        pad.substick_x = axis(Axis::RightStickX);
        pad.substick_y = axis(Axis::RightStickY);
        let (l, r) = (
            gp.button_value(Button::LeftTrigger2),
            gp.button_value(Button::RightTrigger2),
        );
        pad.trigger_left = (l * 255.0) as u8;
        pad.trigger_right = (r * 255.0) as u8;
        if l > 0.95 {
            pad.button |= L;
        }
        if r > 0.95 {
            pad.button |= R;
        }
        pad
    }

    /// A small deterministic generator for the tests.
    pub(crate) struct Rng(pub u64);
    impl Rng {
        pub fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        pub fn unit(&mut self) -> f32 {
            // Edges and exact values often, as the sticks' rests and ends are.
            match self.next() % 8 {
                0 => 0.0,
                1 => 1.0,
                2 => -1.0,
                3 => 0.95,
                _ => (self.next() % 20001) as f32 / 10000.0 - 1.0,
            }
        }
    }

    #[test]
    fn the_gamepads_default_is_the_old_mapping() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let defaults = Bindings::gamepad();
        for _ in 0..20000 {
            let mut gp = Fake::default();
            for b in BUTTONS {
                if rng.next().is_multiple_of(3) {
                    gp.pressed.push(b);
                }
            }
            for b in [gilrs::Button::LeftTrigger2, gilrs::Button::RightTrigger2] {
                gp.values.push((b, rng.unit().abs()));
            }
            for a in AXES {
                gp.axes.push((a, rng.unit() * 1.1));
            }
            let taps: Vec<_> = BUTTONS
                .into_iter()
                .filter(|_| rng.next().is_multiple_of(5))
                .collect();
            let got = defaults.pad(&GamepadReader {
                state: &gp,
                taps: &taps,
                exact: false,
            });
            let mut want = old_gamepad(&gp, &taps);
            want.err = ERR_NONE;
            assert_eq!(got, want);
        }
    }

    /// "No deadzone, exact": a gamepad sending each GameCube byte as its axis's `v × 257` over
    /// 0 to 65535 (gilrs's `axis_value` of it) and each trigger's as `t / 255` plays those
    /// bytes, −128 and every step off centre included; without it the old scaling truncates.
    #[test]
    fn exact_gamepads_play_their_bytes() {
        use gilrs::{Axis, Button};
        let value = |v: u8| (f32::from(v) * 257.0 + 1.0) / 65536.0 * 2.0 - 1.0;
        // A trigger's byte as Windows Gaming Input and gilrs scale it (`t / 255` times
        // `i32::MAX`, then over 0 to `i32::MAX`), which lands just short of `t / 255`.
        let travel = |t: u8| {
            let val = (f64::from(t) / 255.0 * f64::from(i32::MAX)) as i32;
            (val as f32 / i32::MAX as f32).clamp(0.0, 1.0)
        };
        let b = Bindings::gamepad();
        let mut moved = 0;
        for v in 0..=255u8 {
            let gp = Fake {
                values: vec![
                    (Button::LeftTrigger2, travel(v)),
                    (Button::RightTrigger2, travel(255 - v)),
                ],
                axes: vec![
                    (Axis::LeftStickX, value(v)),
                    (Axis::LeftStickY, value(255 - v)),
                    (Axis::RightStickX, value(v)),
                    (Axis::RightStickY, value(v)),
                ],
                ..Fake::default()
            };
            let read = |exact| {
                b.pad(&GamepadReader {
                    state: &gp,
                    taps: &[],
                    exact,
                })
            };
            let p = read(true);
            let s = (i16::from(v) - 128) as i8;
            assert_eq!(
                (p.stick_x, p.stick_y, p.substick_x, p.substick_y),
                (s, (127 - i16::from(v)) as i8, s, s),
                "{v}"
            );
            assert_eq!((p.trigger_left, p.trigger_right), (v, 255 - v), "{v}");
            moved += usize::from(read(false) != p);
        }
        assert!(moved > 200, "the old reading differs");
    }

    fn random_pad(rng: &mut Rng) -> PadStatus {
        let s = |rng: &mut Rng| rng.next() as u8 as i8;
        PadStatus {
            button: rng.next() as u16 & 0x1f7f,
            stick_x: s(rng),
            stick_y: s(rng),
            substick_x: s(rng),
            substick_y: s(rng),
            trigger_left: rng.next() as u8,
            trigger_right: rng.next() as u8,
            analog_a: 0,
            analog_b: 0,
            err: ERR_NONE,
        }
    }

    /// The adapter's default plays the controller's report as it is, every value (−128
    /// included).
    #[test]
    fn the_adapters_default_is_the_report_itself() {
        let mut rng = Rng(7);
        let b = Bindings::gamecube();
        for _ in 0..20000 {
            let p = random_pad(&mut rng);
            assert_eq!(b.pad(&GcReader(&p)), p);
        }
        for v in [i8::MIN, -127, -1, 0, 1, 127] {
            let p = PadStatus {
                stick_x: v,
                substick_y: v,
                err: ERR_NONE,
                ..PadStatus::default()
            };
            assert_eq!(b.pad(&GcReader(&p)), p);
        }
    }

    /// Every binding makes its `PADStatus`: each target bound alone to a key, pressed, sets
    /// exactly its own field.
    #[test]
    fn every_target_makes_its_input() {
        struct Keys(Vec<&'static str>);
        impl Reader for Keys {
            fn read(&self, s: &Source, _: bool, reach: f32) -> Reading {
                let Source::Key(k) = s else {
                    return Reading::default();
                };
                digital(self.0.contains(&k.as_str()), reach)
            }
        }
        let none = PadStatus {
            err: ERR_NONE,
            ..PadStatus::default()
        };
        for t in Target::ALL {
            let mut b = Bindings::none();
            b.set(t, Some(Source::Key("KeyP".into())));
            let p = b.pad(&Keys(vec!["KeyP"]));
            let mut want = none;
            match t {
                Target::StickUp => want.stick_y = 127,
                Target::StickDown => want.stick_y = -127,
                Target::StickLeft => want.stick_x = -127,
                Target::StickRight => want.stick_x = 127,
                Target::CUp => want.substick_y = 127,
                Target::CDown => want.substick_y = -127,
                Target::CLeft => want.substick_x = -127,
                Target::CRight => want.substick_x = 127,
                Target::LAnalog => want.trigger_left = 255,
                Target::RAnalog => want.trigger_right = 255,
                Target::Modifier => {}
                t => want.button = t.bit().unwrap(),
            }
            assert_eq!(p, want, "{t:?}");
            // Released, nothing.
            assert_eq!(b.pad(&Keys(vec![])), none, "{t:?}");
        }
        // The modifier halves a digital direction, and opposite directions cancel.
        let mut b = Bindings::keyboard();
        b.set(Target::StickLeft, Some(Source::Key("KeyP".into())));
        let p = b.pad(&Keys(vec!["KeyP", "ShiftLeft", "KeyI"]));
        assert_eq!((p.stick_x, p.substick_y), (-64, 64));
        let p = b.pad(&Keys(vec!["KeyP", "ArrowRight"]));
        assert_eq!(p.stick_x, 0);
    }

    /// Rebound across kinds: a GameCube controller with its stick and C-stick swapped, A and
    /// B swapped, and the D-pad up from the stick: each value lands where it's bound, exactly.
    #[test]
    fn a_rebound_gamecube_controller() {
        let mut b = Bindings::gamecube();
        for (t, s) in [
            (Target::StickUp, Target::CUp),
            (Target::StickDown, Target::CDown),
            (Target::StickLeft, Target::CLeft),
            (Target::StickRight, Target::CRight),
            (Target::CUp, Target::StickUp),
            (Target::CDown, Target::StickDown),
            (Target::CLeft, Target::StickLeft),
            (Target::CRight, Target::StickRight),
            (Target::A, Target::B),
            (Target::B, Target::A),
            (Target::DUp, Target::StickUp),
        ] {
            b.set(t, Some(Source::Gc(s)));
        }
        let mut rng = Rng(99);
        for _ in 0..5000 {
            let p = random_pad(&mut rng);
            let got = b.pad(&GcReader(&p));
            assert_eq!(
                (got.stick_x, got.stick_y, got.substick_x, got.substick_y),
                (p.substick_x, p.substick_y, p.stick_x, p.stick_y)
            );
            assert_eq!(got.button & A != 0, p.button & B != 0);
            assert_eq!(got.button & B != 0, p.button & A != 0);
            assert_eq!(got.button & UP != 0, p.stick_y >= 64);
            assert_eq!(got.button & !(A | B | UP), p.button & !(A | B | UP));
            assert_eq!(
                (got.trigger_left, got.trigger_right),
                (p.trigger_left, p.trigger_right)
            );
        }
    }

    #[test]
    fn bindings_round_trip_as_text() {
        let mut b = Bindings::gamepad();
        b.set(Target::A, Some(Source::Key("Space".into())));
        b.set(Target::B, None);
        b.set(Target::Z, Some(Source::Axis(gilrs::Axis::RightZ, false)));
        b.set(Target::Y, Some(Source::Gc(Target::CLeft)));
        for start in [Bindings::none(), Bindings::keyboard()] {
            assert_eq!(start.restore(&b.save()), Some(b.clone()));
        }
        for d in [Bindings::keyboard(), Bindings::gamecube(), Bindings::none()] {
            assert_eq!(Bindings::none().restore(&d.save()), Some(d.clone()));
        }
        assert_eq!(Bindings::none().restore("a=button:Nope"), None);
        assert_eq!(Bindings::none().restore("q=key:KeyX"), None);
        assert_eq!(Bindings::none().restore("a=gc:modifier"), None);
        assert_eq!(key_name("KeyX"), "X");
        assert_eq!(key_name("ArrowLeft"), "Left arrow");
        assert_eq!(key_name("ShiftLeft"), "Left Shift");
        assert_eq!(key_name("Enter"), "Enter");
    }

    #[test]
    fn rebind_picks_the_targets_kind() {
        let travel = Source::Gc(Target::LAnalog);
        let click = Source::Gc(Target::LClick);
        let both = [click.clone(), travel.clone()];
        assert_eq!(pick(Target::LAnalog, &both), Some(travel.clone()));
        assert_eq!(pick(Target::LClick, &both), Some(click));
        assert_eq!(pick(Target::A, std::slice::from_ref(&travel)), Some(travel));
        assert_eq!(pick(Target::A, &[]), None);
    }
}
