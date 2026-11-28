//! Reads the player's controllers into the simulation's controller input: the keyboard,
//! standard gamepads (gilrs), and the GameCube controller adapter (raw over USB, `adapter`).
//! The player reads them right before each tick ([`Inputs::read`]) and hands the four ports to
//! the match; nothing here reads the simulation. [`Delay`] holds the reads back a set number of
//! ticks, for players who want latency added back.
//!
//! **The device model** (milestone 7's task 1). Every controller has a stable identity
//! ([`DeviceId`]: the adapter's port n, a gamepad by gilrs's UUID and which of its kind it is,
//! the keyboard), plays one port ([`ports`]: the ports the player chose in the Controllers tab,
//! else the one it took on its own, kept across unplugging), and has bindings ([`bindings`]:
//! which of its inputs plays each GameCube input, the player's or its kind's default). Each
//! read, every device present is read through its bindings and the devices on a port are
//! combined: buttons pressed on any, each stick axis from the last device that moves it (the
//! keyboard after a gamepad), each trigger the further pressed. The game sees only the four
//! ports' `PADStatus`, so whatever this layer does is what the session recording and a `.slp`
//! record, and plays back the same.
//!
//! For a new kind of device (task 3's box controllers): give it a [`DeviceId`] variant, a
//! default in [`DeviceId::default_bindings`], a [`bindings::Reader`] for its state (with
//! [`Source`] variants for its inputs if gilrs's names don't fit), and list it in
//! [`Inputs::read`]'s present devices; [`Ports`] and the Controllers tab take it from there.

pub mod adapter;
pub mod bindings;
mod delay;
pub mod ports;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use melee_sim::input::pad::{ERR_NO_CONTROLLER, PadStatus};

pub use adapter::Adapter;
pub use bindings::{Bindings, Kind, Reader, Reading, Source, Target};
pub use delay::Delay;
/// gilrs, whose names gamepad bindings use.
pub use gilrs;
pub use ports::{Chosen, Config, DeviceId, DeviceInfo, Ports};

/// The keyboard as a controller: which keys are held, and which were pressed since the last
/// read (a tap shorter than a tick still counts as held for one read). Keys are named as the
/// window names their key codes (winit's `KeyCode`, as `KeyX`).
#[derive(Default)]
pub struct Keyboard {
    held: Vec<String>,
    tapped: Vec<String>,
    /// When the newest press or release since the last read arrived, for its age.
    changed: Option<Instant>,
}

impl Keyboard {
    pub fn set(&mut self, key: &str, pressed: bool) {
        if pressed {
            if !self.held.iter().any(|k| k == key) {
                self.held.push(key.to_owned());
                self.tapped.push(key.to_owned());
                self.changed = Some(Instant::now());
            }
        } else if self.held.iter().any(|k| k == key) {
            self.held.retain(|k| k != key);
            self.changed = Some(Instant::now());
        }
    }

    fn down(&self, k: &str) -> bool {
        self.held.iter().any(|h| h == k) || self.tapped.iter().any(|t| t == k)
    }

    /// Whether any key has been used, so the keyboard counts as plugged in.
    pub fn used(&self) -> bool {
        !self.held.is_empty() || !self.tapped.is_empty()
    }
}

impl Reader for Keyboard {
    fn read(&self, source: &Source, _click: bool, reach: f32) -> Reading {
        match source {
            Source::Key(k) => bindings::digital(self.down(k), reach),
            _ => Reading::default(),
        }
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

/// A connected gamepad: gilrs's id for it this run and its identity.
struct Pad {
    id: gilrs::GamepadId,
    device: DeviceId,
}

/// Every controller the player reads, each playing its port ([`ports`]).
pub struct Inputs {
    pub keyboard: Keyboard,
    gilrs: Option<gilrs::Gilrs>,
    pub adapter: Option<Arc<Adapter>>,
    /// The inputs' ages at the last read.
    pub ages: InputAges,
    /// What the player set: ports and bindings.
    config: Config,
    /// The ports devices took on their own.
    ports: Ports,
    /// The gamepads connected, in gilrs's order.
    pads: Vec<Pad>,
    /// Gamepad buttons pressed since the last read.
    taps: Vec<(gilrs::GamepadId, gilrs::Button)>,
    /// The devices present at the last read and the ports they played.
    playing: Vec<(DeviceId, Option<u8>)>,
    /// The name of every device seen, for while it's away.
    names: Vec<(DeviceId, String)>,
    /// Each kind's default bindings, for devices the player didn't rebind.
    defaults: [Bindings; 3],
    started: Instant,
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
            config: Config::default(),
            ports: Ports::default(),
            pads: Vec::new(),
            taps: Vec::new(),
            playing: Vec::new(),
            names: Vec::new(),
            defaults: [
                Bindings::gamecube(),
                Bindings::gamepad(),
                Bindings::keyboard(),
            ],
            started: Instant::now(),
        }
    }

    /// The player's ports and bindings, from the next read.
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// A key of the window's pressed or released (by winit's name for its key code): the
    /// keyboard controller takes the presses of the keys its bindings use, and every release,
    /// so no key stays held.
    pub fn key(&mut self, name: &str, pressed: bool) {
        if !pressed || self.bindings(&DeviceId::Keyboard).uses_key(name) {
            self.keyboard.set(name, pressed);
        }
    }

    /// Takes in gilrs's events: the gamepads connected and the buttons pressed since. The
    /// newest event's time, if any.
    fn pump(&mut self) -> Option<SystemTime> {
        let g = self.gilrs.as_mut()?;
        let mut newest: Option<SystemTime> = None;
        while let Some(e) = g.next_event() {
            if let gilrs::EventType::ButtonPressed(b, _) = e.event {
                self.taps.push((e.id, b));
            }
            newest = newest.max(Some(e.time));
        }
        let mut connected = Vec::new();
        for (id, gp) in g.gamepads() {
            connected.push(id);
            if !self.pads.iter().any(|p| p.id == id) {
                let uuid = gp.uuid();
                // The first number no gamepad of its kind connected now has: one unplugged and
                // plugged back in is the same gamepad again.
                let nth = (0..)
                    .find(|&n| {
                        !self
                            .pads
                            .iter()
                            .any(|p| p.device == DeviceId::Gamepad { uuid, nth: n })
                    })
                    .unwrap_or(0);
                let device = DeviceId::Gamepad { uuid, nth };
                let name = match nth {
                    0 => gp.name().to_owned(),
                    n => format!("{} ({})", gp.name(), n + 1),
                };
                remember(&mut self.names, &device, &name);
                self.pads.push(Pad { id, device });
            }
        }
        self.pads.retain(|p| connected.contains(&p.id));
        // gilrs's order.
        self.pads
            .sort_by_key(|p| connected.iter().position(|&c| c == p.id));
        self.taps.retain(|t| connected.contains(&t.0));
        newest
    }

    /// The devices present, in [`Ports::assign`]'s order: the adapter's controllers, the
    /// gamepads, the keyboard.
    fn present(&mut self, gc: &[Option<PadStatus>; 4]) -> Vec<DeviceId> {
        let mut present = Vec::with_capacity(4 + self.pads.len() + 1);
        for (n, p) in gc.iter().enumerate() {
            if p.is_some() {
                let d = DeviceId::Adapter(n as u8);
                remember(&mut self.names, &d, &ports::adapter_name(n as u8));
                present.push(d);
            }
        }
        present.extend(self.pads.iter().map(|p| p.device.clone()));
        remember(&mut self.names, &DeviceId::Keyboard, ports::KEYBOARD);
        present.push(DeviceId::Keyboard);
        present
    }

    /// Whether the adapter has had its chance to report its controllers, so where devices go
    /// can be remembered ([`ports::SETTLE`]).
    fn settled(&self) -> bool {
        self.adapter
            .as_ref()
            .is_none_or(|a| a.report_time().is_some())
            || self.started.elapsed() >= ports::SETTLE
    }

    /// `d`'s bindings: the player's, else its kind's default.
    fn bindings(&self, d: &DeviceId) -> &Bindings {
        match self.config.bindings.iter().find(|b| b.0 == *d) {
            Some(b) => &b.1,
            None => match d {
                DeviceId::Adapter(_) => &self.defaults[0],
                DeviceId::Gamepad { .. } => &self.defaults[1],
                DeviceId::Keyboard => &self.defaults[2],
            },
        }
    }

    /// The `PADStatus` device `d` plays now.
    fn pad_of(&self, d: &DeviceId, gc: &[Option<PadStatus>; 4]) -> Option<PadStatus> {
        let b = self.bindings(d);
        match d {
            DeviceId::Adapter(n) => {
                let p = gc[*n as usize]?;
                // Its own inputs as themselves: the report as `PADRead` gives it.
                Some(if *b == self.defaults[0] {
                    p
                } else {
                    b.pad(&bindings::GcReader(&p))
                })
            }
            DeviceId::Gamepad { .. } => {
                let g = self.gilrs.as_ref()?;
                let pad = self.pads.iter().find(|p| p.device == *d)?;
                let taps: Vec<_> = (self.taps.iter())
                    .filter(|t| t.0 == pad.id)
                    .map(|t| t.1)
                    .collect();
                Some(b.pad(&bindings::GamepadReader {
                    state: &g.gamepad(pad.id),
                    taps: &taps,
                }))
            }
            DeviceId::Keyboard => Some(b.pad(&self.keyboard)),
        }
    }

    /// The four ports as the PAD library would read them now: `err` is
    /// `PAD_ERR_NO_CONTROLLER` on a port nothing plays.
    pub fn read(&mut self) -> [PadStatus; 4] {
        let none = PadStatus {
            err: ERR_NO_CONTROLLER,
            ..PadStatus::default()
        };
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
        let gc = self.adapter.as_ref().map_or([None; 4], |a| a.ports());
        let newest = self.pump();
        self.ages.gamepad = newest.map(|t| SystemTime::now().duration_since(t).unwrap_or_default());
        let present = self.present(&gc);
        let ports = if self.settled() {
            self.ports.assign(&present, &self.config)
        } else {
            self.ports.clone().assign(&present, &self.config)
        };
        let mut out = [none; 4];
        let mut filled = [false; 4];
        for (d, port) in present.iter().zip(&ports) {
            let Some(port) = port.map(usize::from) else {
                continue;
            };
            if let Some(p) = self.pad_of(d, &gc) {
                combine(&mut out[port], filled[port], p);
                filled[port] = true;
            }
        }
        self.keyboard.tapped.clear();
        self.taps.clear();
        self.playing = present.into_iter().zip(ports).collect();
        out
    }

    /// The devices the Controllers tab shows, with the ports they'd play under `config`: those
    /// present, then those away that hold a port or that the player placed.
    pub fn look(&mut self, config: &Config) -> Vec<DeviceInfo> {
        let gc = self.adapter.as_ref().map_or([None; 4], |a| a.ports());
        self.pump();
        let present = self.present(&gc);
        let ports = self.ports.clone().assign(&present, config);
        let name = |names: &[(DeviceId, String)], d: &DeviceId| {
            names
                .iter()
                .find(|n| n.0 == *d)
                .map_or_else(|| d.key(), |n| n.1.clone())
        };
        let mut out: Vec<DeviceInfo> = present
            .iter()
            .zip(ports)
            .map(|(d, port)| DeviceInfo {
                id: d.clone(),
                name: name(&self.names, d),
                connected: true,
                port,
            })
            .collect();
        for c in &config.ports {
            if !present.contains(&c.device) {
                out.push(DeviceInfo {
                    id: c.device.clone(),
                    name: c.name.clone(),
                    connected: false,
                    port: c.port,
                });
            }
        }
        for (d, port) in self.ports.away(&present) {
            if config.chosen(d).is_none() && !config.ports.iter().any(|c| c.port == Some(port)) {
                out.push(DeviceInfo {
                    id: d.clone(),
                    name: name(&self.names, d),
                    connected: false,
                    port: Some(port),
                });
            }
        }
        out
    }

    /// What `device` holds pressed now (and, for a gamepad, any button it pressed since the
    /// last look), for Rebind… to take the input the player presses. The keyboard's come from
    /// the window's key events instead.
    pub fn active(&mut self, device: &DeviceId) -> Vec<Source> {
        let mut out = Vec::new();
        match device {
            DeviceId::Adapter(n) => {
                let gc = self.adapter.as_ref().map_or([None; 4], |a| a.ports());
                if let Some(p) = gc[*n as usize] {
                    let r = bindings::GcReader(&p);
                    for t in Target::ALL {
                        let s = Source::Gc(t);
                        if t != Target::Modifier && r.read(&s, false, 127.0).pressed {
                            out.push(s);
                        }
                    }
                }
            }
            DeviceId::Gamepad { .. } => {
                self.pump();
                let (Some(g), Some(pad)) = (
                    self.gilrs.as_ref(),
                    self.pads.iter().find(|p| p.device == *device),
                ) else {
                    return out;
                };
                let gp = g.gamepad(pad.id);
                for b in bindings::BUTTONS {
                    if gp.is_pressed(b) || self.taps.contains(&(pad.id, b)) {
                        out.push(Source::Button(b));
                    }
                }
                for a in bindings::AXES {
                    let v = gp.value(a);
                    if v.abs() >= 0.6 {
                        out.push(Source::Axis(a, v > 0.0));
                    }
                }
                let id = pad.id;
                self.taps.retain(|t| t.0 != id);
            }
            DeviceId::Keyboard => {
                out.extend(self.keyboard.held.iter().map(|k| Source::Key(k.clone())));
            }
        }
        out
    }

    /// Rumble for each port, as the match asks for it: each controller on the adapter rumbles
    /// with the port it plays.
    pub fn rumble(&self, on: [bool; 4]) {
        if let Some(a) = &self.adapter {
            a.rumble(motors(&self.playing, on));
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

/// Keeps `d`'s name for while it's away.
fn remember(names: &mut Vec<(DeviceId, String)>, d: &DeviceId, name: &str) {
    match names.iter_mut().find(|n| n.0 == *d) {
        Some(n) if n.1 != name => n.1 = name.to_owned(),
        Some(_) => {}
        None => names.push((d.clone(), name.to_owned())),
    }
}

/// Adds device `p` to a port's `into`: the first device on it gives everything; each one after
/// adds its buttons, moves an axis it holds off centre and presses a trigger further.
fn combine(into: &mut PadStatus, filled: bool, p: PadStatus) {
    if !filled {
        *into = p;
        return;
    }
    into.button |= p.button;
    for (a, b) in [
        (&mut into.stick_x, p.stick_x),
        (&mut into.stick_y, p.stick_y),
        (&mut into.substick_x, p.substick_x),
        (&mut into.substick_y, p.substick_y),
    ] {
        if b != 0 {
            *a = b;
        }
    }
    into.trigger_left = into.trigger_left.max(p.trigger_left);
    into.trigger_right = into.trigger_right.max(p.trigger_right);
}

/// The adapter's motors: each adapter port's as the game port its controller plays asks.
fn motors(playing: &[(DeviceId, Option<u8>)], on: [bool; 4]) -> [bool; 4] {
    let mut m = [false; 4];
    for (d, port) in playing {
        if let (DeviceId::Adapter(n), Some(p)) = (d, port) {
            m[*n as usize] = on[*p as usize];
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use bindings::{A, L};

    /// The keyboard's default bindings, as `Keyboard::add` read it before bindings (3846341f).
    #[test]
    fn keyboard_directions_and_taps() {
        let b = Bindings::keyboard();
        let mut k = Keyboard::default();
        k.set("ArrowLeft", true);
        k.set("KeyX", true);
        k.set("KeyX", false);
        let pad = b.pad(&k);
        k.tapped.clear();
        assert_eq!(pad.stick_x, -127);
        // A tap released before the read still counts once.
        assert_eq!(pad.button & A, A);
        let pad = b.pad(&k);
        assert_eq!(pad.button & A, 0);
        k.set("ShiftLeft", true);
        let pad = b.pad(&k);
        assert_eq!(pad.stick_x, -64);
        k.set("KeyQ", true);
        let pad = b.pad(&k);
        assert_eq!((pad.button & L, pad.trigger_left), (L, 255));
    }

    /// The keyboard read every way the old code could, against the old code.
    #[test]
    fn the_keyboards_default_is_the_old_mapping() {
        // The old `Keyboard::add` over an empty pad: buttons, directions with half reach on
        // the modifier, both directions giving neither, L and R's travel full with their button.
        fn old(held: &[&str]) -> PadStatus {
            let down = |k: &str| held.contains(&k);
            let mut pad = PadStatus::default();
            for (k, bit) in [
                ("KeyX", A),
                ("KeyZ", bindings::B),
                ("KeyC", bindings::X),
                ("KeyS", bindings::Y),
                ("KeyD", bindings::Z),
                ("KeyQ", L),
                ("KeyW", bindings::R),
                ("Enter", bindings::START),
                ("KeyF", bindings::LEFT),
                ("KeyH", bindings::RIGHT),
                ("KeyG", bindings::DOWN),
                ("KeyT", bindings::UP),
            ] {
                if down(k) {
                    pad.button |= bit;
                }
            }
            let reach: i8 = if down("ShiftLeft") { 64 } else { 127 };
            let dir = |n: &str, p: &str| match (down(n), down(p)) {
                (true, false) => -reach,
                (false, true) => reach,
                _ => 0,
            };
            pad.stick_x = dir("ArrowLeft", "ArrowRight");
            pad.stick_y = dir("ArrowDown", "ArrowUp");
            pad.substick_x = dir("KeyJ", "KeyL");
            pad.substick_y = dir("KeyK", "KeyI");
            if pad.button & L != 0 {
                pad.trigger_left = 255;
            }
            if pad.button & bindings::R != 0 {
                pad.trigger_right = 255;
            }
            pad
        }
        let keys = [
            "ArrowLeft",
            "ArrowRight",
            "ArrowUp",
            "ArrowDown",
            "KeyJ",
            "KeyL",
            "KeyI",
            "KeyK",
            "KeyX",
            "KeyZ",
            "KeyC",
            "KeyS",
            "KeyD",
            "KeyQ",
            "KeyW",
            "Enter",
            "KeyT",
            "KeyG",
            "KeyF",
            "KeyH",
            "ShiftLeft",
            "Space",
        ];
        let b = Bindings::keyboard();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..20000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let held: Vec<&str> = (keys.iter())
                .enumerate()
                .filter(|(i, _)| seed >> i & 1 == 1 && (seed >> (i + 30)) & 1 == 1)
                .map(|(_, k)| *k)
                .collect();
            let mut k = Keyboard::default();
            for h in &held {
                k.set(h, true);
            }
            assert_eq!(b.pad(&k), old(&held), "{held:?}");
        }
    }

    /// Devices on one port: the keyboard joins a gamepad as it did, its directions over the
    /// gamepad's stick and its buttons added.
    #[test]
    fn devices_on_one_port_combine() {
        let pad = PadStatus {
            button: A,
            stick_x: 40,
            stick_y: -20,
            trigger_left: 100,
            ..PadStatus::default()
        };
        let keys = PadStatus {
            button: bindings::B,
            stick_x: -127,
            trigger_left: 0,
            ..PadStatus::default()
        };
        let mut into = PadStatus::default();
        combine(&mut into, false, pad);
        assert_eq!(into, pad);
        combine(&mut into, true, keys);
        assert_eq!(
            (into.button, into.stick_x, into.stick_y, into.trigger_left),
            (A | bindings::B, -127, -20, 100)
        );
    }

    /// Rumble follows the port: a controller in the adapter's second port playing port 4
    /// rumbles when port 4 does.
    #[test]
    fn rumble_follows_the_port() {
        let playing = [
            (DeviceId::Adapter(1), Some(3)),
            (DeviceId::Adapter(0), Some(0)),
            (DeviceId::Adapter(2), None),
            (DeviceId::Keyboard, Some(1)),
        ];
        assert_eq!(
            motors(&playing, [true, true, false, true]),
            [true, true, false, false]
        );
        assert_eq!(
            motors(&playing, [false, false, true, false]),
            [false, false, false, false]
        );
        // With every controller on its own port, the motors are the ports'.
        let own: Vec<_> = (0..4).map(|n| (DeviceId::Adapter(n), Some(n))).collect();
        assert_eq!(
            motors(&own, [true, false, true, false]),
            [true, false, true, false]
        );
    }
}
