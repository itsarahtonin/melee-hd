//! Reads the player's controllers into the simulation's controller input: the keyboard,
//! standard gamepads (gilrs), and the GameCube controller adapter (raw over USB, `adapter`).
//! The player reads them right before each tick ([`Inputs::read`]) and hands the four ports to
//! the match; nothing here reads the simulation. [`Delay`] holds the reads back a set number of
//! ticks, for players who want latency added back.

pub mod adapter;
mod delay;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use melee_sim::input::pad::{ERR_NO_CONTROLLER, PadStatus};

pub use adapter::Adapter;
pub use delay::Delay;

// PADStatus buttons.
const LEFT: u16 = 0x0001;
const RIGHT: u16 = 0x0002;
const DOWN: u16 = 0x0004;
const UP: u16 = 0x0008;
const Z: u16 = 0x0010;
const R: u16 = 0x0020;
const L: u16 = 0x0040;
const A: u16 = 0x0100;
const B: u16 = 0x0200;
const X: u16 = 0x0400;
const Y: u16 = 0x0800;
const START: u16 = 0x1000;

/// The keys the keyboard controller uses; the window maps its key codes to these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    CLeft,
    CRight,
    CUp,
    CDown,
    A,
    B,
    X,
    Y,
    Z,
    L,
    R,
    Start,
    DLeft,
    DRight,
    DUp,
    DDown,
    /// Holds the stick at half its reach (walking, tilts).
    Modifier,
}

/// The keyboard as a controller: which keys are held, and which were pressed since the last
/// read (a tap shorter than a tick still counts as held for one read).
#[derive(Default)]
pub struct Keyboard {
    held: Vec<Key>,
    tapped: Vec<Key>,
    /// When the newest press or release since the last read arrived, for its age.
    changed: Option<Instant>,
}

impl Keyboard {
    pub fn set(&mut self, key: Key, pressed: bool) {
        if pressed {
            if !self.held.contains(&key) {
                self.held.push(key);
                self.tapped.push(key);
                self.changed = Some(Instant::now());
            }
        } else if self.held.contains(&key) {
            self.held.retain(|&k| k != key);
            self.changed = Some(Instant::now());
        }
    }

    fn down(&self, k: Key) -> bool {
        self.held.contains(&k) || self.tapped.contains(&k)
    }

    /// Whether any key has been used, so the keyboard counts as plugged in.
    pub fn used(&self) -> bool {
        !self.held.is_empty() || !self.tapped.is_empty()
    }

    /// Adds the keyboard to `pad`: its buttons, and its directions where it holds any.
    fn add(&mut self, pad: &mut PadStatus) {
        let bits = [
            (Key::A, A),
            (Key::B, B),
            (Key::X, X),
            (Key::Y, Y),
            (Key::Z, Z),
            (Key::L, L),
            (Key::R, R),
            (Key::Start, START),
            (Key::DLeft, LEFT),
            (Key::DRight, RIGHT),
            (Key::DDown, DOWN),
            (Key::DUp, UP),
        ];
        for (k, bit) in bits {
            if self.down(k) {
                pad.button |= bit;
            }
        }
        let reach: i8 = if self.down(Key::Modifier) { 64 } else { 127 };
        let dir = |neg: Key, pos: Key| -> i8 {
            match (self.down(neg), self.down(pos)) {
                (true, false) => -reach,
                (false, true) => reach,
                _ => 0,
            }
        };
        let set = |axis: &mut i8, v: i8| {
            if v != 0 {
                *axis = v;
            }
        };
        set(&mut pad.stick_x, dir(Key::Left, Key::Right));
        set(&mut pad.stick_y, dir(Key::Down, Key::Up));
        set(&mut pad.substick_x, dir(Key::CLeft, Key::CRight));
        set(&mut pad.substick_y, dir(Key::CDown, Key::CUp));
        if pad.button & L != 0 {
            pad.trigger_left = 255;
        }
        if pad.button & R != 0 {
            pad.trigger_right = 255;
        }
        self.tapped.clear();
    }
}

/// A standard gamepad (Xbox, PlayStation, Switch Pro layouts) as a GameCube controller.
fn gamepad(gp: &gilrs::Gamepad, tapped: &[gilrs::Button], pad: &mut PadStatus) {
    use gilrs::{Axis, Button};
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
    let axis = |a: Axis| (gp.value(a).clamp(-1.0, 1.0) * 127.0) as i8;
    pad.stick_x = axis(Axis::LeftStickX);
    pad.stick_y = axis(Axis::LeftStickY);
    pad.substick_x = axis(Axis::RightStickX);
    pad.substick_y = axis(Axis::RightStickY);
    let trigger = |b: Button| gp.button_data(b).map_or(0.0, |d| d.value());
    let (l, r) = (
        trigger(Button::LeftTrigger2),
        trigger(Button::RightTrigger2),
    );
    pad.trigger_left = (l * 255.0) as u8;
    pad.trigger_right = (r * 255.0) as u8;
    // A full press clicks the trigger's button, as a GameCube trigger's travel ends in one.
    if l > 0.95 {
        pad.button |= L;
    }
    if r > 0.95 {
        pad.button |= R;
    }
}

/// How old each source's newest input was when [`Inputs::read`] last read it: the time from
/// its arrival in the player (the adapter's report coming off USB, the window's key event, the
/// gamepad library's event) to the read. The keyboard and gamepads have an age only for a read
/// that took in a new event; the adapter's report is always the latest one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputAges {
    pub adapter: Option<Duration>,
    pub keyboard: Option<Duration>,
    pub gamepad: Option<Duration>,
}

/// Every controller the player reads, assigned to ports: GameCube controllers on an adapter
/// play their own ports; gamepads and the keyboard play the first ports left, the keyboard
/// with the first gamepad.
pub struct Inputs {
    pub keyboard: Keyboard,
    gilrs: Option<gilrs::Gilrs>,
    pub adapter: Option<Arc<Adapter>>,
    /// The inputs' ages at the last read.
    pub ages: InputAges,
}

impl Inputs {
    /// The controllers; `adapter` starts reading a GameCube adapter.
    pub fn new(adapter: bool) -> Inputs {
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("input: no gamepads ({e})");
                None
            }
        };
        Inputs {
            keyboard: Keyboard::default(),
            gilrs,
            adapter: adapter.then(Adapter::start),
            ages: InputAges::default(),
        }
    }

    /// The four ports as the PAD library would read them now: `err` is
    /// `PAD_ERR_NO_CONTROLLER` on a port nothing plays.
    pub fn read(&mut self) -> [PadStatus; 4] {
        let none = PadStatus {
            err: ERR_NO_CONTROLLER,
            ..PadStatus::default()
        };
        let mut out = [none; 4];
        let mut taken = [false; 4];
        let now = Instant::now();
        self.ages = InputAges {
            adapter: self
                .adapter
                .as_ref()
                .and_then(|a| a.report_time())
                .map(|t| now.saturating_duration_since(t)),
            keyboard: self
                .keyboard
                .changed
                .take()
                .map(|t| now.saturating_duration_since(t)),
            gamepad: None,
        };
        if let Some(a) = &self.adapter {
            for (i, p) in a.ports().into_iter().enumerate() {
                if let Some(p) = p {
                    out[i] = p;
                    taken[i] = true;
                }
            }
        }
        let mut free = (0..4).filter(|&i| !taken[i]);
        let mut first = true;
        if let Some(g) = &mut self.gilrs {
            let mut tapped = Vec::new();
            let mut newest: Option<SystemTime> = None;
            while let Some(e) = g.next_event() {
                if let gilrs::EventType::ButtonPressed(b, _) = e.event {
                    tapped.push((e.id, b));
                }
                newest = newest.max(Some(e.time));
            }
            self.ages.gamepad =
                newest.map(|t| SystemTime::now().duration_since(t).unwrap_or_default());
            let ids: Vec<_> = g.gamepads().map(|(id, _)| id).collect();
            for id in ids {
                let Some(port) = free.next() else { break };
                let mut pad = PadStatus::default();
                let taps: Vec<_> = tapped.iter().filter(|t| t.0 == id).map(|t| t.1).collect();
                gamepad(&g.gamepad(id), &taps, &mut pad);
                if first {
                    self.keyboard.add(&mut pad);
                    first = false;
                }
                out[port] = pad;
            }
        }
        if first && let Some(port) = free.next() {
            let mut pad = PadStatus::default();
            self.keyboard.add(&mut pad);
            out[port] = pad;
        }
        out
    }

    /// Rumble for each port, as the match asks for it.
    pub fn rumble(&self, on: [bool; 4]) {
        if let Some(a) = &self.adapter {
            a.rumble(on);
        }
    }

    /// Lets the controllers go: the adapter's motors off and its interface released.
    pub fn close(&mut self) {
        if let Some(a) = self.adapter.take() {
            a.close();
        }
    }
}

impl Drop for Inputs {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_directions_and_taps() {
        let mut k = Keyboard::default();
        k.set(Key::Left, true);
        k.set(Key::A, true);
        k.set(Key::A, false);
        let mut pad = PadStatus::default();
        k.add(&mut pad);
        assert_eq!(pad.stick_x, -127);
        // A tap released before the read still counts once.
        assert_eq!(pad.button & A, A);
        let mut pad = PadStatus::default();
        k.add(&mut pad);
        assert_eq!(pad.button & A, 0);
        k.set(Key::Modifier, true);
        let mut pad = PadStatus::default();
        k.add(&mut pad);
        assert_eq!(pad.stick_x, -64);
    }
}
