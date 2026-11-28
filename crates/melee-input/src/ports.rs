//! The device model: each controller the player reads has a stable identity ([`DeviceId`]),
//! and each plays a port. The ports a player chooses in the Controllers tab are kept
//! ([`Config::ports`], in `settings.txt`); a device nobody placed takes one on its own
//! ([`Ports::assign`]) and keeps it for the session. Either way a device keeps its port across
//! unplugging: its port stays empty while it's away, and nothing else moves.
//!
//! A device nobody placed takes the port melee-hd always gave it: a GameCube controller on
//! the adapter its own port number, a gamepad the first port free, the keyboard the first
//! gamepad's port (playing with it) or else the first port free. The keyboard alone never holds
//! a port: it's always there, so it follows. Until the adapter has sent its first report (or
//! for [`SETTLE`] after it started), nothing is remembered: the devices that turn up first
//! don't take the adapter's ports from it.

use crate::bindings::Bindings;
use crate::boxes::BoxKeys;

/// A controller, as the player knows it from one session to the next.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeviceId {
    /// The controller plugged into the adapter's port (0 to 3).
    Adapter(u8),
    /// A gamepad by gilrs's UUID (its vendor, product and version), and which of the gamepads
    /// with that UUID it is (0 for the first plugged in): identical gamepads have no serial
    /// gilrs reads.
    Gamepad {
        uuid: [u8; 16],
        nth: u8,
    },
    Keyboard,
    /// A box controller melee-hd recognizes ([`crate::boxes::MODELS`]) by its USB vendor and
    /// product ids, and which of the boxes with those ids it is (0 for the first plugged in).
    Box {
        vendor: u16,
        product: u16,
        nth: u8,
    },
}

impl DeviceId {
    /// Its name in `settings.txt`: `adapter:1` to `adapter:4`, `gamepad:UUID:N`, `keyboard`,
    /// `box:VENDOR:PRODUCT:N` (the ids in hex).
    pub fn key(&self) -> String {
        match self {
            DeviceId::Adapter(n) => format!("adapter:{}", n + 1),
            DeviceId::Gamepad { uuid, nth } => {
                let hex: String = uuid.iter().map(|b| format!("{b:02x}")).collect();
                format!("gamepad:{hex}:{nth}")
            }
            DeviceId::Keyboard => "keyboard".to_owned(),
            DeviceId::Box {
                vendor,
                product,
                nth,
            } => format!("box:{vendor:04x}:{product:04x}:{nth}"),
        }
    }

    /// [`DeviceId::key`]'s device.
    pub fn parse(v: &str) -> Option<DeviceId> {
        let mut parts = v.split(':');
        let d = match parts.next()? {
            "adapter" => {
                let n: u8 = parts.next()?.parse().ok()?;
                DeviceId::Adapter(n.checked_sub(1).filter(|n| *n < 4)?)
            }
            "gamepad" => {
                let hex = parts.next()?;
                if hex.len() != 32 || !hex.is_ascii() {
                    return None;
                }
                let mut uuid = [0; 16];
                for (i, b) in uuid.iter_mut().enumerate() {
                    *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
                }
                DeviceId::Gamepad {
                    uuid,
                    nth: parts.next()?.parse().ok()?,
                }
            }
            "keyboard" => DeviceId::Keyboard,
            "box" => {
                let mut id = || {
                    let h = parts.next()?;
                    (h.len() == 4).then_some(())?;
                    u16::from_str_radix(h, 16).ok()
                };
                let (vendor, product) = (id()?, id()?);
                DeviceId::Box {
                    vendor,
                    product,
                    nth: parts.next()?.parse().ok()?,
                }
            }
            _ => return None,
        };
        parts.next().is_none().then_some(d)
    }

    /// Its bindings when the player set none: a box's are a GameCube controller's, its report
    /// being one.
    pub fn default_bindings(&self) -> Bindings {
        match self {
            DeviceId::Adapter(_) | DeviceId::Box { .. } => Bindings::gamecube(),
            DeviceId::Gamepad { .. } => Bindings::gamepad(),
            DeviceId::Keyboard => Bindings::keyboard(),
        }
    }

    /// Whether the player can set "No deadzone, exact" for it: a gamepad (a box always reads
    /// so, the adapter's controllers and the keyboard have nothing to set).
    pub fn can_be_exact(&self) -> bool {
        matches!(self, DeviceId::Gamepad { .. })
    }

    /// Whether the player can give it a box layout: the keyboard.
    pub fn can_have_box_layout(&self) -> bool {
        *self == DeviceId::Keyboard
    }
}

/// The adapter's port's name in the Controllers tab ("GameCube adapter, port N").
pub fn adapter_name(n: u8) -> String {
    format!("GameCube adapter, port {}", n + 1)
}

/// The keyboard's name in the Controllers tab.
pub const KEYBOARD: &str = "Keyboard";

/// A port the player chose for a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub device: DeviceId,
    /// The port (0 to 3), or none: the device plays no port.
    pub port: Option<u8>,
    /// Its name, for while it's away.
    pub name: String,
}

/// What the player set for the controllers: the ports chosen and the bindings changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Each device the player placed, and where (empty: every device takes its own port).
    pub ports: Vec<Chosen>,
    /// The bindings that differ from their device's default.
    pub bindings: Vec<(DeviceId, Bindings)>,
    /// The gamepads set to "No deadzone, exact".
    pub exact: Vec<DeviceId>,
    /// The devices (the keyboard) played through a box layout, with its keys.
    pub box_layouts: Vec<(DeviceId, BoxKeys)>,
}

/// A device as the Controllers tab shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub connected: bool,
    /// The port it plays, or holds while it's away.
    pub port: Option<u8>,
}

impl Config {
    /// Where the player put `d`, if they did.
    pub fn chosen(&self, d: &DeviceId) -> Option<&Chosen> {
        self.ports.iter().find(|c| c.device == *d)
    }

    /// `d`'s bindings: the player's, else its default.
    pub fn bindings(&self, d: &DeviceId) -> Bindings {
        self.bindings
            .iter()
            .find(|b| b.0 == *d)
            .map_or_else(|| d.default_bindings(), |b| b.1.clone())
    }

    /// Sets `d`'s bindings, kept only where they differ from its default.
    pub fn set_bindings(&mut self, d: &DeviceId, b: Bindings) {
        self.bindings.retain(|x| x.0 != *d);
        if b != d.default_bindings() {
            self.bindings.push((d.clone(), b));
        }
    }

    /// Whether `d` reads "No deadzone, exact".
    pub fn is_exact(&self, d: &DeviceId) -> bool {
        self.exact.contains(d)
    }

    /// Sets "No deadzone, exact" for `d`, if it can take it.
    pub fn set_exact(&mut self, d: &DeviceId, on: bool) {
        self.exact.retain(|x| x != d);
        if on && d.can_be_exact() {
            self.exact.push(d.clone());
        }
    }

    /// `d`'s box layout, if it plays through one.
    pub fn box_layout(&self, d: &DeviceId) -> Option<&BoxKeys> {
        self.box_layouts.iter().find(|b| b.0 == *d).map(|b| &b.1)
    }

    /// Plays `d` through the box layout `keys`, or through its bindings with none.
    pub fn set_box_layout(&mut self, d: &DeviceId, keys: Option<BoxKeys>) {
        self.box_layouts.retain(|b| b.0 != *d);
        if let Some(k) = keys
            && d.can_have_box_layout()
        {
            self.box_layouts.push((d.clone(), k));
        }
    }

    /// The player picks `pick` (or nothing) for `port`, with `devices` where every device is
    /// now: every device stays where it is (kept as the player's from now on, so nothing moves
    /// later either), `pick` moves to `port`, and what played `port` takes `pick`'s old port:
    /// two devices swap. Nothing for a port leaves what played it on no port.
    pub fn choose(&mut self, devices: &[DeviceInfo], port: u8, pick: Option<&DeviceId>) {
        for d in devices {
            if self.chosen(&d.id).is_none() {
                self.ports.push(Chosen {
                    device: d.id.clone(),
                    port: d.port,
                    name: d.name.clone(),
                });
            }
        }
        let old = pick.and_then(|p| self.chosen(p)).and_then(|c| c.port);
        for c in &mut self.ports {
            if Some(&c.device) == pick {
                c.port = Some(port);
            } else if c.port == Some(port) {
                c.port = if pick.is_some() { old } else { None };
            }
        }
    }

    /// The `settings.txt` lines: `port = DEVICE PORT NAME` for each device placed (PORT 1 to 4
    /// or `none`), `bindings = DEVICE TARGET=SOURCE…` for each device rebound, `exact = DEVICE`
    /// for each gamepad set to "No deadzone, exact", `box-layout = DEVICE BUTTON=SOURCE…` for a
    /// device played through a box layout.
    pub fn save(&self) -> String {
        let mut out = String::new();
        for c in &self.ports {
            let port = c
                .port
                .map_or_else(|| "none".to_owned(), |p| (p + 1).to_string());
            out += &format!("port = {} {port} {}\n", c.device.key(), c.name);
        }
        for (d, b) in &self.bindings {
            out += &format!("bindings = {} {}\n", d.key(), b.save());
        }
        for d in &self.exact {
            out += &format!("exact = {}\n", d.key());
        }
        for (d, k) in &self.box_layouts {
            out += &format!("box-layout = {} {}\n", d.key(), k.save());
        }
        out
    }

    /// Takes one of [`Config::save`]'s lines (`key` and `value`); false for any other key. A
    /// line that can't be read is passed over.
    pub fn restore_line(&mut self, key: &str, value: &str) -> bool {
        match key {
            "port" => {
                let mut parts = value.splitn(3, ' ');
                let device = parts.next().and_then(DeviceId::parse);
                let port = match parts.next() {
                    Some("none") => Some(None),
                    Some(p) => p
                        .parse::<u8>()
                        .ok()
                        .filter(|p| (1..=4).contains(p))
                        .map(|p| Some(p - 1)),
                    None => None,
                };
                if let (Some(device), Some(port)) = (device, port) {
                    let name = parts.next().unwrap_or("").to_owned();
                    self.ports.retain(|c| c.device != device);
                    self.ports.push(Chosen { device, port, name });
                }
                true
            }
            "bindings" => {
                let (d, rest) = value.split_once(' ').unwrap_or((value, ""));
                if let Some(d) = DeviceId::parse(d)
                    && let Some(b) = d.default_bindings().restore(rest)
                {
                    self.set_bindings(&d, b);
                }
                true
            }
            "exact" => {
                if let Some(d) = DeviceId::parse(value) {
                    self.set_exact(&d, true);
                }
                true
            }
            "box-layout" => {
                let (d, rest) = value.split_once(' ').unwrap_or((value, ""));
                if let Some(d) = DeviceId::parse(d)
                    && let Some(k) = BoxKeys::restore(rest)
                {
                    self.set_box_layout(&d, Some(k));
                }
                true
            }
            _ => false,
        }
    }
}

/// How long after the adapter starts the ports wait for its first report before remembering
/// where devices went.
pub const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

/// The ports devices took on their own this session.
#[derive(Clone, Debug, Default)]
pub struct Ports {
    /// Each device that took a port on its own (the keyboard never does), and the port.
    auto: Vec<(DeviceId, u8)>,
}

impl Ports {
    /// The port each of `present` plays now, given the player's `config`: theirs for a device
    /// they placed, else the one it took, else a new one ([`Ports`]' rules). `present` lists
    /// the adapter's controllers, then the gamepads in gilrs's order, then the keyboard.
    pub fn assign(&mut self, present: &[DeviceId], config: &Config) -> Vec<Option<u8>> {
        let mut out = vec![None; present.len()];
        let mut decided = vec![false; present.len()];
        let mut held = [false; 4];
        for c in &config.ports {
            if let Some(p) = c.port {
                held[p as usize] = true;
            }
        }
        // A device the player has placed since is theirs; one whose port the player gave
        // another device takes a new one.
        self.auto
            .retain(|(d, p)| config.chosen(d).is_none() && !held[*p as usize]);
        for (_, p) in &self.auto {
            held[*p as usize] = true;
        }
        for (i, d) in present.iter().enumerate() {
            if let Some(c) = config.chosen(d) {
                out[i] = c.port;
                decided[i] = true;
            } else if let Some((_, p)) = self.auto.iter().find(|a| a.0 == *d) {
                out[i] = Some(*p);
                decided[i] = true;
            }
        }
        for (i, d) in present.iter().enumerate() {
            if decided[i] || *d == DeviceId::Keyboard {
                continue;
            }
            let own = match d {
                DeviceId::Adapter(n) if !held[*n as usize] => Some(*n),
                _ => (0..4).find(|&p| !held[p as usize]),
            };
            // No port free: the lowest one a device that took it on its own left empty.
            let port = own.or_else(|| {
                let away = self
                    .auto
                    .iter()
                    .filter(|a| !present.contains(&a.0))
                    .map(|a| a.1)
                    .min()?;
                self.auto.retain(|a| a.1 != away);
                Some(away)
            });
            if let Some(p) = port {
                held[p as usize] = true;
                self.auto.push((d.clone(), p));
                out[i] = Some(p);
            }
        }
        // The keyboard plays with the first gamepad (a box is one), or alone in the first port
        // free.
        if let Some(k) = present.iter().position(|d| *d == DeviceId::Keyboard)
            && !decided[k]
        {
            let pad = present.iter().zip(&out).find(|(d, p)| {
                matches!(d, DeviceId::Gamepad { .. } | DeviceId::Box { .. }) && p.is_some()
            });
            out[k] = match pad {
                Some((_, p)) => *p,
                None => (0..4).find(|&p| !held[p as usize]),
            };
        }
        out
    }

    /// The devices that took a port on their own and are away, with it.
    pub fn away<'a>(&'a self, present: &'a [DeviceId]) -> impl Iterator<Item = (&'a DeviceId, u8)> {
        self.auto
            .iter()
            .filter(|a| !present.contains(&a.0))
            .map(|a| (&a.0, a.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(n: u8) -> DeviceId {
        DeviceId::Gamepad {
            uuid: [n; 16],
            nth: 0,
        }
    }

    /// The ports `present` plays, as (device, port) pairs.
    fn run(p: &mut Ports, c: &Config, present: &[DeviceId]) -> Vec<(DeviceId, Option<u8>)> {
        present.iter().cloned().zip(p.assign(present, c)).collect()
    }

    fn port_of(r: &[(DeviceId, Option<u8>)], d: &DeviceId) -> Option<u8> {
        r.iter().find(|x| x.0 == *d).and_then(|x| x.1)
    }

    /// With nothing set, the devices take the ports melee-hd always gave them.
    #[test]
    fn the_old_order_with_nothing_set() {
        let c = Config::default();
        let k = DeviceId::Keyboard;
        let r = run(&mut Ports::default(), &c, &[pad(1), pad(2), k.clone()]);
        assert_eq!(
            r,
            [(pad(1), Some(0)), (pad(2), Some(1)), (k.clone(), Some(0))]
        );
        let r = run(&mut Ports::default(), &c, std::slice::from_ref(&k));
        assert_eq!(r, [(k.clone(), Some(0))]);
        let r = run(
            &mut Ports::default(),
            &c,
            &[
                DeviceId::Adapter(0),
                DeviceId::Adapter(2),
                pad(1),
                k.clone(),
            ],
        );
        assert_eq!(
            r,
            [
                (DeviceId::Adapter(0), Some(0)),
                (DeviceId::Adapter(2), Some(2)),
                (pad(1), Some(1)),
                (k.clone(), Some(1))
            ]
        );
        let r = run(
            &mut Ports::default(),
            &c,
            &[DeviceId::Adapter(0), k.clone()],
        );
        assert_eq!(port_of(&r, &k), Some(1));
        // Five gamepads: the fifth plays nowhere.
        let five: Vec<_> = (1..=5).map(pad).collect();
        let r = run(&mut Ports::default(), &c, &five);
        assert_eq!(
            r.iter().map(|x| x.1).collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2), Some(3), None]
        );
    }

    /// A scripted plug and unplug sequence: nothing that plays moves.
    #[test]
    fn nothing_moves_across_unplugging() {
        let c = Config::default();
        let mut p = Ports::default();
        let (a0, a1) = (DeviceId::Adapter(0), DeviceId::Adapter(1));
        let k = DeviceId::Keyboard;
        let r = run(&mut p, &c, &[a0.clone(), pad(1), pad(2), k.clone()]);
        assert_eq!(
            (port_of(&r, &pad(1)), port_of(&r, &pad(2))),
            (Some(1), Some(2))
        );
        // The first gamepad goes: the second stays on port 3 (not 2, as before), port 2 empty.
        let r = run(&mut p, &c, &[a0.clone(), pad(2), k.clone()]);
        assert_eq!(port_of(&r, &pad(2)), Some(2));
        assert_eq!(port_of(&r, &a0), Some(0));
        // A new gamepad takes the first port nobody holds: port 4, not the away one's.
        let r = run(&mut p, &c, &[a0.clone(), pad(2), pad(3), k.clone()]);
        assert_eq!(port_of(&r, &pad(3)), Some(3));
        // The first comes back to its port.
        let r = run(&mut p, &c, &[a0.clone(), pad(1), pad(2), pad(3), k.clone()]);
        assert_eq!(
            [&a0, &pad(1), &pad(2), &pad(3)].map(|d| port_of(&r, d)),
            [Some(0), Some(1), Some(2), Some(3)]
        );
        // The adapter's controller goes and comes back; the gamepads stay put.
        let r = run(&mut p, &c, &[pad(1), pad(2), pad(3), k.clone()]);
        assert_eq!(
            [&pad(1), &pad(2), &pad(3)].map(|d| port_of(&r, d)),
            [Some(1), Some(2), Some(3)]
        );
        let r = run(&mut p, &c, &[a0.clone(), pad(1), pad(2), pad(3), k.clone()]);
        assert_eq!(port_of(&r, &a0), Some(0));
        // Every port held: a controller in the adapter's second port plays nowhere until one
        // frees, then takes the one an away device left.
        let r = run(
            &mut p,
            &c,
            &[a0.clone(), a1.clone(), pad(1), pad(2), pad(3), k.clone()],
        );
        assert_eq!(port_of(&r, &a1), None);
        let r = run(
            &mut p,
            &c,
            &[a0.clone(), a1.clone(), pad(1), pad(3), k.clone()],
        );
        assert_eq!(port_of(&r, &a1), Some(2));
        assert_eq!(
            [&a0, &pad(1), &pad(3)].map(|d| port_of(&r, d)),
            [Some(0), Some(1), Some(3)]
        );
        // The second gamepad, back, has lost its port to it and takes none.
        let r = run(
            &mut p,
            &c,
            &[a0.clone(), a1.clone(), pad(1), pad(2), pad(3), k.clone()],
        );
        assert_eq!(port_of(&r, &pad(2)), None);
    }

    /// The player's ports: kept for devices away, a new device never takes one, and the
    /// devices nobody placed keep theirs as the player changes others.
    #[test]
    fn the_players_ports_hold() {
        let mut p = Ports::default();
        let mut c = Config::default();
        let k = DeviceId::Keyboard;
        let present = [pad(1), pad(2), k.clone()];
        let r = run(&mut p, &c, &present);
        let info = |r: &[(DeviceId, Option<u8>)]| -> Vec<DeviceInfo> {
            r.iter()
                .map(|(d, port)| DeviceInfo {
                    id: d.clone(),
                    name: format!("{d:?}"),
                    connected: true,
                    port: *port,
                })
                .collect()
        };
        // The keyboard to port 3: the rest stay where they were.
        c.choose(&info(&r), 2, Some(&k));
        let r = run(&mut p, &c, &present);
        assert_eq!(
            [&pad(1), &pad(2), &k].map(|d| port_of(&r, d)),
            [Some(0), Some(1), Some(2)]
        );
        // The two gamepads swap.
        c.choose(&info(&r), 0, Some(&pad(2)));
        let r = run(&mut p, &c, &present);
        assert_eq!(
            [&pad(1), &pad(2)].map(|d| port_of(&r, d)),
            [Some(1), Some(0)]
        );
        // A gamepad away keeps its port empty; a new one takes port 4.
        let r = run(&mut p, &c, &[pad(1), pad(3), k.clone()]);
        assert_eq!(
            [&pad(1), &pad(3)].map(|d| port_of(&r, d)),
            [Some(1), Some(3)]
        );
        // None for port 2: the first gamepad plays nowhere.
        c.choose(&info(&r), 1, None);
        let r = run(&mut p, &c, &[pad(1), pad(3), k.clone()]);
        assert_eq!(port_of(&r, &pad(1)), None);
        assert_eq!(port_of(&r, &pad(3)), Some(3));
        // And the settings keep all of it.
        let mut back = Config::default();
        for line in c.save().lines() {
            let (key, v) = line.split_once(" = ").unwrap();
            assert!(back.restore_line(key, v));
        }
        assert_eq!(back, c);
    }

    /// A device's identity and the settings' lines read back.
    #[test]
    fn config_round_trips() {
        let mut c = Config::default();
        let g = DeviceId::Gamepad {
            uuid: [
                0x03, 0, 0, 0, 0x5e, 0x04, 0, 0, 0x8e, 0x02, 0, 0, 0x14, 0x01, 0, 0,
            ],
            nth: 1,
        };
        c.ports.push(Chosen {
            device: g.clone(),
            port: Some(3),
            name: "Xbox Controller = 2".into(),
        });
        c.ports.push(Chosen {
            device: DeviceId::Adapter(2),
            port: None,
            name: adapter_name(2),
        });
        let mut b = Bindings::gamecube();
        b.set(crate::Target::A, Some(crate::Source::Gc(crate::Target::B)));
        c.set_bindings(&DeviceId::Adapter(2), b);
        c.set_bindings(&g, Bindings::gamepad());
        assert_eq!(c.bindings.len(), 1, "a default isn't kept");
        let haybox = DeviceId::Box {
            vendor: 0x2e8a,
            product: 0x000a,
            nth: 1,
        };
        c.ports.push(Chosen {
            device: haybox.clone(),
            port: Some(1),
            name: "Box controller (HayBox DInput) (2)".into(),
        });
        c.set_exact(&g, true);
        // Only a gamepad takes "No deadzone, exact", only the keyboard a box layout.
        c.set_exact(&haybox, true);
        c.set_exact(&DeviceId::Keyboard, true);
        assert_eq!(c.exact, std::slice::from_ref(&g));
        let mut keys = crate::boxes::BoxKeys::default();
        keys.set(melee_phob::haybox::Button::Rt1, Some("KeyX".into()));
        c.set_box_layout(&DeviceId::Keyboard, Some(keys.clone()));
        c.set_box_layout(&g, Some(keys.clone()));
        assert_eq!(c.box_layouts, [(DeviceId::Keyboard, keys.clone())]);
        let mut back = Config::default();
        for line in c.save().lines() {
            let (key, v) = line.split_once('=').unwrap();
            assert!(back.restore_line(key.trim(), v.trim()));
        }
        assert_eq!(back, c);
        assert!(back.is_exact(&g) && !back.is_exact(&haybox));
        assert_eq!(back.box_layout(&DeviceId::Keyboard), Some(&keys));
        back.set_box_layout(&DeviceId::Keyboard, None);
        back.set_exact(&g, false);
        assert!(back.box_layouts.is_empty() && back.exact.is_empty());
        for d in [
            g,
            DeviceId::Adapter(0),
            DeviceId::Adapter(3),
            DeviceId::Keyboard,
            haybox,
        ] {
            assert_eq!(DeviceId::parse(&d.key()), Some(d));
        }
        assert_eq!(
            DeviceId::parse("box:0738:4726:0"),
            Some(DeviceId::Box {
                vendor: 0x0738,
                product: 0x4726,
                nth: 0
            })
        );
        for bad in [
            "adapter:0",
            "adapter:5",
            "gamepad:00:1",
            "mouse",
            "keyboard:1",
            "box:2e8a:000a",
            "box:2e8a:0a:0",
            "box:2e8a:000a:0:1",
            "box:zzzz:000a:0",
        ] {
            assert_eq!(DeviceId::parse(bad), None, "{bad}");
        }
        assert!(!back.restore_line("volume", "50"));
    }
}
