//! The settings panel's Controllers tab (milestone 7's task 1): which controller plays each
//! port, and each controller's bindings, from `melee_input`'s device model. The player reads
//! the devices into [`Controllers::devices`] before each frame of the tab
//! (`melee_input::Inputs::look`), and while Rebind… runs, hands it what the device holds
//! ([`Controllers::offer`]) or the keys pressed ([`Controllers::key`]). What the tab changes is
//! the panel's settings (`front::Settings::controllers`), which the inputs take when the panel
//! closes, and `settings.txt` keeps.
//!
//! **Extension point:** each GameCube adapter port listed gets a call to the closure
//! [`Controllers::show`] takes, under its bindings, with the port (0 to 3) and the adapter:
//! task 2 (PhobGCC) draws its section there (`front::Panel::show_panel` passes it).

use std::sync::Arc;
use std::time::{Duration, Instant};

use melee_input::{Adapter, Config, DeviceId, DeviceInfo, Source, Target, bindings};

/// The tab's name.
pub const TAB: &str = "Controllers";
/// A port nothing plays, and a target nothing is bound to.
pub const NONE: &str = "None";
pub const REBIND: &str = "Rebind\u{2026}";
pub const RESET: &str = "Reset to default";
pub const PORTS: &str = "Ports";
pub const DEVICES: &str = "Devices";
pub const BINDINGS: &str = "Bindings";

/// How long Rebind… waits for each input before skipping it.
pub const WAIT: Duration = Duration::from_secs(5);

/// A port's name: "Port 1" to "Port 4".
pub fn port_name(p: u8) -> String {
    format!("Port {}", p + 1)
}

/// Rebind…'s prompt for `t`.
pub fn press(t: Target) -> String {
    format!("Press the key or button for {} (Esc skips it)", t.name())
}

/// A device's name, saying so while it's away.
pub fn shown_name(d: &DeviceInfo) -> String {
    if d.connected {
        d.name.clone()
    } else {
        format!("{} (not connected)", d.name)
    }
}

/// The tab's own state.
#[derive(Default)]
pub struct Controllers {
    /// The devices, as the player last looked (`Inputs::look`).
    pub devices: Vec<DeviceInfo>,
    /// The adapter, for the adapter ports' sections.
    pub adapter: Option<Arc<Adapter>>,
    capture: Option<Capture>,
}

/// Rebind… running.
struct Capture {
    device: DeviceId,
    /// The target asked for now, of [`Target::ALL`].
    at: usize,
    since: Instant,
    /// What the device held at the last offer (none before the first): only a new press binds.
    held: Option<Vec<Source>>,
    /// The input pressed for the target, bound at the next frame.
    pressed: Option<Source>,
}

impl Controllers {
    /// The device Rebind… is reading, while it runs.
    pub fn capturing(&self) -> Option<&DeviceId> {
        self.capture.as_ref().map(|c| &c.device)
    }

    /// What the device being rebound holds now: an input it didn't hold at the last offer binds
    /// the target asked for. What it held when Rebind… began (the button that pressed Rebind…)
    /// binds nothing.
    pub fn offer(&mut self, held: Vec<Source>) {
        let Some(c) = &mut self.capture else { return };
        if let Some(before) = &c.held {
            let new: Vec<Source> = held
                .iter()
                .filter(|s| !before.contains(s))
                .cloned()
                .collect();
            if c.pressed.is_none() {
                c.pressed = bindings::pick(Target::ALL[c.at], &new);
            }
        }
        c.held = Some(held);
    }

    /// A key pressed in the window, while the panel is open: it binds the target asked for when
    /// the keyboard is being rebound. True when it was taken.
    pub fn key(&mut self, name: &str) -> bool {
        match &mut self.capture {
            Some(c) if c.device == DeviceId::Keyboard => {
                c.pressed
                    .get_or_insert_with(|| Source::Key(name.to_owned()));
                true
            }
            _ => false,
        }
    }

    /// Esc: skips the target asked for, leaving its binding as it was.
    pub fn skip(&mut self) {
        self.next();
    }

    /// Rebind… stops (the panel closed).
    pub fn stop(&mut self) {
        self.capture = None;
    }

    fn next(&mut self) {
        let Some(c) = &mut self.capture else { return };
        c.at += 1;
        c.since = Instant::now();
        c.pressed = None;
        if c.at == Target::ALL.len() {
            self.capture = None;
        }
    }

    /// Binds what Rebind… took, or skips a target it waited on too long.
    fn take(&mut self, config: &mut Config) {
        let Some(c) = &mut self.capture else { return };
        if !self.devices.iter().any(|d| d.id == c.device && d.connected) {
            self.capture = None;
            return;
        }
        if let Some(s) = c.pressed.take() {
            let mut b = config.bindings(&c.device);
            b.set(Target::ALL[c.at], Some(s));
            config.set_bindings(&c.device, b);
            self.next();
        } else if c.since.elapsed() >= WAIT {
            self.next();
        }
    }

    /// Lays the tab out over the player's `config`; `adapter_port` draws what an adapter
    /// port's controller shows under its bindings (task 2's PhobGCC section).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut Config,
        adapter_port: &mut dyn FnMut(&mut egui::Ui, u8, Option<&Adapter>),
    ) {
        self.take(config);
        // The devices come and go while the tab shows; Rebind… waits on presses.
        let every = if self.capture.is_some() { 30 } else { 250 };
        ui.ctx().request_repaint_after(Duration::from_millis(every));
        section(ui, PORTS);
        egui::Grid::new("ports")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for p in 0..4u8 {
                    ui.label(port_name(p));
                    let on: Vec<String> = (self.devices.iter())
                        .filter(|d| d.port == Some(p))
                        .map(shown_name)
                        .collect();
                    let shown = if on.is_empty() {
                        NONE.to_owned()
                    } else {
                        on.join(" + ")
                    };
                    let mut pick: Option<Option<DeviceId>> = None;
                    egui::ComboBox::from_id_salt(("port", p))
                        .selected_text(shown)
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(on.is_empty(), NONE).clicked() {
                                pick = Some(None);
                            }
                            for d in self.devices.iter().filter(|d| d.connected) {
                                if ui.selectable_label(d.port == Some(p), &d.name).clicked() {
                                    pick = Some(Some(d.id.clone()));
                                }
                            }
                        });
                    if let Some(pick) = pick {
                        config.choose(&self.devices, p, pick.as_ref());
                    }
                    ui.end_row();
                }
            });
        section(ui, DEVICES);
        for d in self.devices.clone() {
            self.show_device(ui, config, &d, adapter_port);
            ui.add_space(8.0);
        }
    }

    fn show_device(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut Config,
        d: &DeviceInfo,
        adapter_port: &mut dyn FnMut(&mut egui::Ui, u8, Option<&Adapter>),
    ) {
        let rebound = config.bindings.iter().any(|b| b.0 == d.id);
        ui.horizontal(|ui| {
            ui.strong(shown_name(d));
            if let Some(p) = d.port {
                ui.weak(port_name(p));
            }
        });
        ui.horizontal(|ui| {
            let idle = self.capture.is_none();
            if ui
                .add_enabled(idle && d.connected, egui::Button::new(REBIND))
                .clicked()
            {
                self.capture = Some(Capture {
                    device: d.id.clone(),
                    at: 0,
                    since: Instant::now(),
                    held: None,
                    pressed: None,
                });
            }
            if ui
                .add_enabled(idle && rebound, egui::Button::new(RESET))
                .clicked()
            {
                config.set_bindings(&d.id, d.id.default_bindings());
            }
        });
        if let Some(c) = &self.capture
            && c.device == d.id
        {
            let t = Target::ALL[c.at];
            egui::Frame::group(ui.style())
                .stroke(egui::Stroke::new(1.0, ui.visuals().selection.stroke.color))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(press(t));
                    ui.weak(format!("{} / {}", c.at + 1, Target::ALL.len()));
                });
        }
        let b = config.bindings(&d.id);
        egui::CollapsingHeader::new(BINDINGS)
            .id_salt(("bindings", d.id.key()))
            .show(ui, |ui| {
                egui::Grid::new(("binding-grid", d.id.key()))
                    .num_columns(2)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        for t in Target::ALL {
                            ui.label(t.name());
                            ui.label(b.get(t).map_or_else(|| NONE.to_owned(), Source::name));
                            ui.end_row();
                        }
                    });
            });
        if let DeviceId::Adapter(n) = d.id {
            adapter_port(ui, n, self.adapter.as_deref());
        }
    }
}

/// A section's heading, as the Settings tab's.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(12.0);
    ui.strong(title);
    ui.separator();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId::Keyboard,
            name: melee_input::ports::KEYBOARD.into(),
            connected: true,
            port: Some(0),
        }
    }

    /// Rebind… takes one input after another: a new press binds the target asked for, what
    /// was held already binds nothing, Esc skips, and the end stops it.
    #[test]
    fn rebind_takes_each_input_in_turn() {
        let mut c = Controllers {
            devices: vec![keyboard()],
            ..Controllers::default()
        };
        let mut config = Config::default();
        c.capture = Some(Capture {
            device: DeviceId::Keyboard,
            at: 0,
            since: Instant::now(),
            held: None,
            pressed: None,
        });
        assert!(c.key("Space"));
        c.take(&mut config);
        c.skip();
        assert!(c.key("KeyM"));
        c.take(&mut config);
        let b = config.bindings(&DeviceId::Keyboard);
        assert_eq!(b.get(Target::A), Some(&Source::Key("Space".into())));
        // B skipped: its default stays.
        assert_eq!(b.get(Target::B), Some(&Source::Key("KeyZ".into())));
        assert_eq!(b.get(Target::X), Some(&Source::Key("KeyM".into())));
        for _ in 3..Target::ALL.len() {
            c.skip();
        }
        assert!(c.capturing().is_none());
        assert!(!c.key("KeyN"), "nothing takes a key once it's done");
    }

    /// What a gamepad held as Rebind… began binds nothing; its next press does.
    #[test]
    fn rebind_waits_for_a_new_press() {
        let pad = DeviceInfo {
            id: DeviceId::Gamepad {
                uuid: [1; 16],
                nth: 0,
            },
            name: "Pad".into(),
            connected: true,
            port: Some(0),
        };
        let mut c = Controllers {
            devices: vec![pad.clone()],
            ..Controllers::default()
        };
        let mut config = Config::default();
        c.capture = Some(Capture {
            device: pad.id.clone(),
            at: 0,
            since: Instant::now(),
            held: None,
            pressed: None,
        });
        let south = Source::Button(melee_input::gilrs::Button::South);
        c.offer(vec![south.clone()]);
        c.take(&mut config);
        assert_eq!(c.capture.as_ref().unwrap().at, 0);
        c.offer(vec![]);
        c.offer(vec![Source::Button(melee_input::gilrs::Button::North)]);
        c.take(&mut config);
        assert_eq!(
            config.bindings(&pad.id).get(Target::A),
            Some(&Source::Button(melee_input::gilrs::Button::North))
        );
        // Still held: the next target waits.
        c.offer(vec![Source::Button(melee_input::gilrs::Button::North)]);
        c.take(&mut config);
        assert_eq!(c.capture.as_ref().unwrap().at, 1);
        // The device unplugged: Rebind… stops.
        c.devices[0].connected = false;
        c.take(&mut config);
        assert!(c.capturing().is_none());
    }
}
