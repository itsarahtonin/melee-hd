//! The PhobGCC section of each adapter port in the Controllers tab: whether the port's
//! controller runs through PhobGCC's processing (`melee_phob`, on the adapter's thread), the
//! named profile it uses, calibration, the settings, and the diagnostic views (a stick map, the
//! snapback graph and the triggers, after PhobVision's).
//!
//! Profiles are files in the settings folder's `phob` folder, `NAME.txt`, and which profile
//! each adapter port uses, and whether it's on, is `phob/ports.txt`; neither is in
//! `settings.txt`. This file is melee-hd's own (GPL-3.0-or-later): it only calls `melee_phob`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use melee_input::Adapter;
use melee_phob::diag::Sample;
use melee_phob::settings::{
    ANALOG_SCALER, CARDINAL_SNAPPING, SMOOTHING, SNAPBACK, TRIGGER_MODE, TRIGGER_MODES,
    TRIGGER_OFFSET, WAVESHAPING,
};
use melee_phob::{Profile, Settings, WhichStick};

const SECTION: &str = "PhobGCC";
const ON: &str = "Process this port with PhobGCC";
const ON_HELP: &str = "PhobGCC's calibration, snapback filter and trigger modes for a stock controller, done in melee-hd. Off for a box or a controller that has its own.";
const PROFILE: &str = "Profile";
const NEW_PROFILE: &str = "New profile";
const RENAME: &str = "Rename";
const NOT_PLUGGED: &str = "No controller in this port.";
const NO_ADAPTER: &str = "No GameCube adapter is connected.";
const CALIBRATE_A: &str = "Calibrate\u{2026} (control stick)";
const CALIBRATE_C: &str = "Calibrate\u{2026} (C-stick)";
const UNCALIBRATED: &str = "Not calibrated: the stick sends the controller's own values.";
const UNDO: &str = "Undo";
const SKIP: &str = "Skip";
const SKIP_HELP: &str = "Skip measuring and adjust the notches of the last calibration.";
const CANCEL: &str = "Cancel";
const CENTRE_PROMPT: &str = "Let go of the stick and press A";
const ADJUST_HELP: &str = "Hold X or Y to turn the notch, B to put it back, then press A.";
const CALIBRATING_NOTE: &str = "The game gets nothing from this controller until calibration ends.";
const SNAPBACK_NAME: &str = "Snapback filter";
const WAVESHAPING_NAME: &str = "Waveshaping";
const SMOOTHING_NAME: &str = "Smoothing";
const CARDINAL_NAME: &str = "Cardinal snapping";
const SCALER_NAME: &str = "Analog scaler";
const TRIGGER_MODE_NAME: &str = "Trigger mode";
const TRIGGER_OFFSET_NAME: &str = "Trigger offset";
const STICK_MAP: &str = "Stick map";
const SNAPBACK_VIEW: &str = "Snapback";
const TRIGGERS_VIEW: &str = "Triggers";

/// The notches' names, counterclockwise from the right.
const NOTCH_NAMES: [&str; 16] = [
    "right",
    "right, toward up-right",
    "up-right",
    "up, toward up-right",
    "up",
    "up, toward up-left",
    "up-left",
    "left, toward up-left",
    "left",
    "left, toward down-left",
    "down-left",
    "down, toward down-left",
    "down",
    "down, toward down-right",
    "down-right",
    "right, toward down-right",
];

/// The prompt for a calibration step.
pub fn prompt(cal: &melee_phob::Calibration) -> String {
    match cal.notch() {
        None => CENTRE_PROMPT.to_owned(),
        Some(n) if cal.measuring() && n % 2 == 1 => format!(
            "Hold the stick at the {} notch and press A (no notch there: let go of the stick and press A)",
            NOTCH_NAMES[n]
        ),
        Some(n) if cal.measuring() => {
            format!("Hold the stick at the {} notch and press A", NOTCH_NAMES[n])
        }
        Some(n) => format!("Adjust the {} notch", NOTCH_NAMES[n]),
    }
}

/// Which diagnostic view a port shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum View {
    #[default]
    StickMap,
    Snapback,
    Triggers,
}

/// A port's choice: on or off, and the profile (kept while off).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Port {
    on: bool,
    profile: String,
}

/// The PhobGCC sections' state: the profiles, each port's choice, and what each section shows.
pub struct PhobPanel {
    folder: PathBuf,
    ports: [Port; 4],
    profiles: BTreeMap<String, Profile>,
    /// The processor's revision each port's profile was last saved at.
    saved: [u64; 4],
    renaming: Option<(usize, String)>,
    views: [View; 4],
    /// The adapter, for the section's place in the Settings tab until the Controllers tab
    /// (task 1) draws it with its own.
    pub adapter: Option<std::sync::Arc<Adapter>>,
}

impl Default for PhobPanel {
    fn default() -> PhobPanel {
        PhobPanel::load(crate::settings::folder().join("phob"))
    }
}

/// A profile's name as a file name: letters, digits, spaces, `-` and `_` kept, the rest `_`.
fn file_name(name: &str) -> String {
    let n: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.txt", n.trim())
}

impl PhobPanel {
    /// The profiles and ports kept in `folder`.
    pub fn load(folder: PathBuf) -> PhobPanel {
        let mut profiles = BTreeMap::new();
        if let Ok(dir) = std::fs::read_dir(&folder) {
            for e in dir.flatten() {
                let path = e.path();
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if name == "ports" || path.extension().is_none_or(|x| x != "txt") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    profiles.insert(name.to_owned(), Profile::from_text(&text));
                }
            }
        }
        let mut ports: [Port; 4] = Default::default();
        if let Ok(text) = std::fs::read_to_string(folder.join("ports.txt")) {
            for line in text.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let Some(n) = key
                    .trim()
                    .strip_prefix("adapter-port-")
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| (1..=4).contains(n))
                else {
                    continue;
                };
                let value = value.trim();
                let (on, profile) = match value.split_once(' ') {
                    Some(("on", p)) => (true, p),
                    Some(("off", p)) => (false, p),
                    _ => (false, ""),
                };
                ports[n - 1] = Port {
                    on: on && profiles.contains_key(profile),
                    profile: profile.to_owned(),
                };
            }
        }
        PhobPanel {
            folder,
            ports,
            profiles,
            saved: [0; 4],
            renaming: None,
            views: Default::default(),
            adapter: None,
        }
    }

    /// Turns the processing on in the adapter for the ports it's on for.
    pub fn apply(&self, adapter: Option<&Adapter>) {
        let Some(adapter) = adapter else { return };
        for i in 0..self.ports.len() {
            adapter.set_phob(i, self.profile_on(i).cloned());
        }
    }

    fn profile_on(&self, port: usize) -> Option<&Profile> {
        let p = &self.ports[port];
        p.on.then(|| self.profiles.get(&p.profile)).flatten()
    }

    fn save_ports(&self) {
        let mut text = String::from("# melee-hd: PhobGCC on the adapter's ports\n");
        for (i, p) in self.ports.iter().enumerate() {
            if !p.profile.is_empty() {
                let state = if p.on { "on" } else { "off" };
                text += &format!("adapter-port-{} = {state} {}\n", i + 1, p.profile);
            }
        }
        self.write("ports.txt", &text);
    }

    fn save_profile(&self, name: &str) {
        if let Some(p) = self.profiles.get(name) {
            self.write(&file_name(name), &p.to_text());
        }
    }

    fn write(&self, file: &str, text: &str) {
        if let Err(e) = std::fs::create_dir_all(&self.folder)
            .and_then(|()| std::fs::write(self.folder.join(file), text))
        {
            eprintln!(
                "phob: couldn't keep {file} in {}: {e}",
                self.folder.display()
            );
        }
    }

    /// A new profile's name: "Controller N", the first N free.
    fn new_name(&self) -> String {
        (1..)
            .map(|n| format!("Controller {n}"))
            .find(|n| !self.profiles.contains_key(n))
            .unwrap()
    }

    fn create(&mut self, name: String) {
        self.profiles.insert(name.clone(), Profile::default());
        self.save_profile(&name);
    }

    /// After a profile changed: every port on with it gets it (the processor's own included).
    fn push(&self, name: &str, adapter: Option<&Adapter>, settings_only: bool) {
        let (Some(adapter), Some(profile)) = (adapter, self.profiles.get(name)) else {
            return;
        };
        for (i, p) in self.ports.iter().enumerate() {
            if p.on && p.profile == name {
                if settings_only {
                    adapter.phob(i, |proc| proc.set_settings(profile.settings));
                } else {
                    adapter.set_phob(i, Some(profile.clone()));
                }
            }
        }
    }

    /// Picks up a calibration the processor finished: the profile is saved and the other ports
    /// using it get it.
    fn take_calibration(&mut self, port: usize, adapter: Option<&Adapter>) {
        let Some(adapter) = adapter else { return };
        let Some((rev, profile)) = adapter.phob(port, |p| (p.revision(), p.profile().clone()))
        else {
            return;
        };
        if rev == self.saved[port] {
            return;
        }
        self.saved[port] = rev;
        let name = self.ports[port].profile.clone();
        if let Some(stored) = self.profiles.get_mut(&name) {
            stored.a = profile.a;
            stored.c = profile.c;
            let stored = stored.clone();
            self.save_profile(&name);
            for (i, p) in self.ports.iter().enumerate() {
                if i != port && p.on && p.profile == name {
                    adapter.set_phob(i, Some(stored.clone()));
                }
            }
        }
    }

    /// The section for adapter port `port` (0 to 3).
    pub fn show(&mut self, ui: &mut egui::Ui, port: usize, adapter: Option<&Adapter>) {
        self.take_calibration(port, adapter);
        ui.add_space(8.0);
        ui.strong(SECTION);
        let mut on = self.ports[port].on;
        if ui.checkbox(&mut on, ON).changed() {
            if on && !self.profiles.contains_key(&self.ports[port].profile) {
                let name = self
                    .profiles
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| self.new_name());
                if !self.profiles.contains_key(&name) {
                    self.create(name.clone());
                }
                self.ports[port].profile = name;
            }
            self.ports[port].on = on;
            // A processor made anew counts its revisions from 0.
            self.saved[port] = 0;
            self.save_ports();
            if let Some(a) = adapter {
                a.set_phob(port, self.profile_on(port).cloned());
            }
        }
        ui.weak(ON_HELP);
        if !self.ports[port].on {
            return;
        }
        self.show_profile(ui, port, adapter);
        let Some(adapter) = adapter else {
            ui.weak(NO_ADAPTER);
            self.show_settings(ui, port, None);
            return;
        };
        if !adapter.plugged(port) {
            ui.weak(NOT_PLUGGED);
        }
        self.show_calibration(ui, port, adapter);
        self.show_settings(ui, port, Some(adapter));
        self.show_views(ui, port, adapter);
        // The views and the calibration's prompts follow the controller.
        ui.ctx().request_repaint();
    }

    fn show_profile(&mut self, ui: &mut egui::Ui, port: usize, adapter: Option<&Adapter>) {
        let id = format!("phob-profile-{port}");
        ui.horizontal(|ui| {
            ui.label(PROFILE);
            let mut chosen = self.ports[port].profile.clone();
            egui::ComboBox::from_id_salt(&id)
                .selected_text(chosen.as_str())
                .width(160.0)
                .show_ui(ui, |ui| {
                    for name in self.profiles.keys() {
                        ui.selectable_value(&mut chosen, name.clone(), name);
                    }
                });
            if ui.button(NEW_PROFILE).clicked() {
                chosen = self.new_name();
                self.create(chosen.clone());
            }
            if ui.button(RENAME).clicked() {
                self.renaming = Some((port, chosen.clone()));
            }
            if chosen != self.ports[port].profile {
                self.ports[port].profile = chosen;
                self.save_ports();
                if let Some(a) = adapter {
                    a.set_phob(port, self.profile_on(port).cloned());
                }
            }
        });
        if let Some((p, name)) = &mut self.renaming
            && *p == port
        {
            let mut done = false;
            ui.horizontal(|ui| {
                let r = ui.text_edit_singleline(name);
                done = r.lost_focus() || ui.button("OK").clicked();
            });
            if done {
                let new = name.trim().to_owned();
                let old = self.ports[port].profile.clone();
                self.renaming = None;
                if !new.is_empty() && new != old && !self.profiles.contains_key(&new) {
                    if let Some(p) = self.profiles.remove(&old) {
                        self.profiles.insert(new.clone(), p);
                        let _ = std::fs::remove_file(self.folder.join(file_name(&old)));
                        self.save_profile(&new);
                    }
                    for p in &mut self.ports {
                        if p.profile == old {
                            p.profile = new.clone();
                        }
                    }
                    self.save_ports();
                }
            }
        }
    }

    fn show_calibration(&mut self, ui: &mut egui::Ui, port: usize, adapter: &Adapter) {
        let state = adapter.phob(port, |p| {
            (
                p.calibration()
                    .map(|c| (prompt(c), c.step, c.stick, c.measuring(), c.undo_ok)),
                p.profile().a.is_some(),
                p.profile().c.is_some(),
            )
        });
        let Some((cal, a_done, c_done)) = state else {
            return;
        };
        match cal {
            None => {
                ui.horizontal(|ui| {
                    for (label, stick) in
                        [(CALIBRATE_A, WhichStick::A), (CALIBRATE_C, WhichStick::C)]
                    {
                        if ui.button(label).clicked() {
                            adapter.phob(port, |p| p.calibrate(stick));
                        }
                    }
                });
                if !a_done || !c_done {
                    ui.weak(UNCALIBRATED);
                }
            }
            Some((prompt, step, stick, measuring, undo_ok)) => {
                let stored = match stick {
                    WhichStick::A => a_done,
                    WhichStick::C => c_done,
                };
                let of = if measuring {
                    format!("Step {} of 32", step + 1)
                } else {
                    format!("Notch {} of 12", step - 31)
                };
                ui.label(egui::RichText::new(prompt).strong());
                if !measuring {
                    ui.label(ADJUST_HELP);
                }
                ui.weak(of);
                ui.horizontal(|ui| {
                    if ui.add_enabled(undo_ok, egui::Button::new(UNDO)).clicked() {
                        adapter.phob(port, |p| p.undo());
                    }
                    if measuring
                        && ui
                            .add_enabled(stored, egui::Button::new(SKIP))
                            .on_hover_text(SKIP_HELP)
                            .clicked()
                    {
                        adapter.phob(port, |p| p.skip());
                    }
                    if ui.button(CANCEL).clicked() {
                        adapter.phob(port, |p| p.cancel_calibration());
                    }
                });
                ui.weak(CALIBRATING_NOTE);
            }
        }
    }

    fn show_settings(&mut self, ui: &mut egui::Ui, port: usize, adapter: Option<&Adapter>) {
        let name = self.ports[port].profile.clone();
        let Some(profile) = self.profiles.get_mut(&name) else {
            return;
        };
        let before = profile.settings;
        let s = &mut profile.settings;
        egui::CollapsingHeader::new("Settings")
            .id_salt(format!("phob-settings-{port}"))
            .default_open(true)
            .show(ui, |ui| {
                // Two sliders a row in the panel's width.
                ui.spacing_mut().slider_width = 64.0;
                egui::Grid::new(format!("phob-grid-{port}"))
                    .num_columns(3)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("");
                        ui.weak("x");
                        ui.weak("y");
                        ui.end_row();
                        pair(
                            ui,
                            SNAPBACK_NAME,
                            &mut s.x_snapback,
                            &mut s.y_snapback,
                            SNAPBACK,
                        );
                        pair(
                            ui,
                            SMOOTHING_NAME,
                            &mut s.ax_smoothing,
                            &mut s.ay_smoothing,
                            SMOOTHING,
                        );
                        pair(
                            ui,
                            WAVESHAPING_NAME,
                            &mut s.ax_waveshaping,
                            &mut s.ay_waveshaping,
                            WAVESHAPING,
                        );
                        pair(
                            ui,
                            &format!("C-stick {}", SMOOTHING_NAME.to_lowercase()),
                            &mut s.cx_smoothing,
                            &mut s.cy_smoothing,
                            SMOOTHING,
                        );
                        pair(
                            ui,
                            &format!("C-stick {}", WAVESHAPING_NAME.to_lowercase()),
                            &mut s.cx_waveshaping,
                            &mut s.cy_waveshaping,
                            WAVESHAPING,
                        );
                        ui.label("");
                        ui.weak("Control stick");
                        ui.weak("C-stick");
                        ui.end_row();
                        pair(
                            ui,
                            CARDINAL_NAME,
                            &mut s.a_cardinal_snapping,
                            &mut s.c_cardinal_snapping,
                            CARDINAL_SNAPPING,
                        );
                        pair(
                            ui,
                            SCALER_NAME,
                            &mut s.a_analog_scaler,
                            &mut s.c_analog_scaler,
                            ANALOG_SCALER,
                        );
                        ui.label("");
                        ui.weak("L");
                        ui.weak("R");
                        ui.end_row();
                        pair(
                            ui,
                            TRIGGER_OFFSET_NAME,
                            &mut s.l_offset,
                            &mut s.r_offset,
                            TRIGGER_OFFSET,
                        );
                    });
                // The modes' names are long: a row each.
                egui::Grid::new(format!("phob-modes-{port}"))
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for (mode, side) in [(&mut s.l_mode, "L"), (&mut s.r_mode, "R")] {
                            ui.label(format!("{side} {}", TRIGGER_MODE_NAME.to_lowercase()));
                            egui::ComboBox::from_id_salt(format!("phob-mode-{port}-{side}"))
                                .selected_text(mode_name(*mode))
                                .width(260.0)
                                .show_ui(ui, |ui| {
                                    for m in TRIGGER_MODE {
                                        ui.selectable_value(mode, m, mode_name(m));
                                    }
                                });
                            ui.end_row();
                        }
                    });
                if ui.button("Reset to default").clicked() {
                    *s = Settings::default();
                }
            });
        if profile.settings != before {
            self.save_profile(&name);
            self.push(&name, adapter, true);
        }
    }

    fn show_views(&mut self, ui: &mut egui::Ui, port: usize, adapter: &Adapter) {
        ui.horizontal(|ui| {
            let view = &mut self.views[port];
            ui.selectable_value(view, View::StickMap, STICK_MAP);
            ui.selectable_value(view, View::Snapback, SNAPBACK_VIEW);
            ui.selectable_value(view, View::Triggers, TRIGGERS_VIEW);
        });
        let samples: Vec<Sample> = adapter
            .phob(port, |p| p.diag().samples().copied().collect())
            .unwrap_or_default();
        match self.views[port] {
            View::StickMap => stick_map(ui, &samples),
            View::Snapback => snapback(ui, &samples),
            View::Triggers => triggers(ui, &samples),
        }
    }
}

fn mode_name(mode: i32) -> String {
    format!("{}: {}", mode + 1, TRIGGER_MODES[mode as usize])
}

/// A row of two sliders over a setting's range.
fn pair(
    ui: &mut egui::Ui,
    name: &str,
    a: &mut i32,
    b: &mut i32,
    range: std::ops::RangeInclusive<i32>,
) {
    ui.label(name);
    ui.add(egui::Slider::new(a, range.clone()));
    ui.add(egui::Slider::new(b, range));
    ui.end_row();
}

/// Melee's stick units: 80 is the rim (1.0).
const RIM: f32 = 80.0;
/// Melee's deadzone on each axis, below 0.2875 (23 units).
const DEADZONE: f32 = 23.0;

/// The stick map: the samples' control stick (and the C-stick, fainter) as sent, over Melee's
/// rim, the deadzone on each axis and the eight directions.
fn stick_map(ui: &mut egui::Ui, samples: &[Sample]) {
    let size = 240.0;
    let (rect, painter) = {
        let (r, p) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());
        (r.rect, p)
    };
    let v = ui.visuals();
    let (bg, line, weak, strong, c_color) = (
        v.extreme_bg_color,
        v.widgets.noninteractive.bg_stroke.color,
        v.weak_text_color(),
        v.strong_text_color(),
        v.selection.bg_fill,
    );
    painter.rect_filled(rect, 4.0, bg);
    let c = rect.center();
    let scale = size / 2.0 / 110.0;
    let at = |x: f32, y: f32| egui::pos2(c.x + x * scale, c.y - y * scale);
    let thin = egui::Stroke::new(1.0, line);
    painter.circle_stroke(c, RIM * scale, thin);
    for k in [-DEADZONE, DEADZONE] {
        painter.line_segment([at(k, -110.0), at(k, 110.0)], thin);
        painter.line_segment([at(-110.0, k), at(110.0, k)], thin);
    }
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        painter.line_segment(
            [
                at(DEADZONE * a.cos(), DEADZONE * a.sin()),
                at(RIM * a.cos(), RIM * a.sin()),
            ],
            egui::Stroke::new(0.5, weak),
        );
    }
    let recent = samples.len().saturating_sub(300);
    for s in &samples[recent..] {
        painter.circle_filled(at(s.c[0] as f32, s.c[1] as f32), 1.5, c_color);
        painter.circle_filled(at(s.a[0] as f32, s.a[1] as f32), 1.5, weak);
    }
    if let Some(s) = samples.last() {
        painter.circle_filled(at(s.a[0] as f32, s.a[1] as f32), 4.0, strong);
        painter.text(
            rect.left_bottom() + egui::vec2(6.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{:.4}, {:.4}", s.a[0] as f32 / RIM, s.a[1] as f32 / RIM),
            egui::FontId::monospace(11.0),
            strong,
        );
    }
}

/// The snapback graph: the control stick's x over the last samples as sent (bright) and as
/// remapped without the filters (faint), so a release's overshoot shows against what's sent.
fn snapback(ui: &mut egui::Ui, samples: &[Sample]) {
    let (w, h) = (ui.available_width().min(440.0), 160.0);
    let (r, painter) = ui.allocate_painter(egui::vec2(w, h), egui::Sense::hover());
    let rect = r.rect;
    let v = ui.visuals();
    painter.rect_filled(rect, 4.0, v.extreme_bg_color);
    let mid = rect.center().y;
    let y = |u: f32| mid - u.clamp(-110.0, 110.0) / 110.0 * (h / 2.0 - 4.0);
    let thin = egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color);
    for k in [-RIM, -DEADZONE, 0.0, DEADZONE, RIM] {
        painter.line_segment(
            [
                egui::pos2(rect.left(), y(k)),
                egui::pos2(rect.right(), y(k)),
            ],
            thin,
        );
    }
    let total: f32 = samples.iter().map(|s| s.step_ms).sum::<f32>().max(1.0);
    let span = total.min(1000.0);
    let mut t = total;
    let mut sent = Vec::new();
    let mut raw = Vec::new();
    for s in samples.iter().rev() {
        if total - t > span {
            break;
        }
        let x = rect.right() - (total - t) / span * w;
        sent.push(egui::pos2(x, y(s.a[0] as f32)));
        raw.push(egui::pos2(x, y(s.a_unfiltered[0])));
        t -= s.step_ms;
    }
    painter.add(egui::Shape::line(
        raw,
        egui::Stroke::new(1.0, v.weak_text_color()),
    ));
    painter.add(egui::Shape::line(
        sent,
        egui::Stroke::new(1.5, v.strong_text_color()),
    ));
    painter.text(
        rect.left_top() + egui::vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "x, last second",
        egui::FontId::proportional(11.0),
        v.weak_text_color(),
    );
}

/// The triggers: each slider as read (faint) and the analog value sent (bright), and the
/// digital button.
fn triggers(ui: &mut egui::Ui, samples: &[Sample]) {
    let Some(s) = samples.last() else {
        ui.weak(NOT_PLUGGED);
        return;
    };
    let (w, h) = (ui.available_width().min(440.0), 70.0);
    let (r, painter) = ui.allocate_painter(egui::vec2(w, h), egui::Sense::hover());
    let rect = r.rect;
    let v = ui.visuals();
    painter.rect_filled(rect, 4.0, v.extreme_bg_color);
    for (i, (name, slider, sent, digital)) in [
        ("L", s.sliders[0], s.triggers[0], s.l),
        ("R", s.sliders[1], s.triggers[1], s.r),
    ]
    .into_iter()
    .enumerate()
    {
        let top = rect.top() + 8.0 + i as f32 * 30.0;
        let left = rect.left() + 28.0;
        let full = w - 40.0;
        let bar = |value: u8, height: f32, color| {
            let r = egui::Rect::from_min_size(
                egui::pos2(left, top + (20.0 - height) / 2.0),
                egui::vec2(full * value as f32 / 255.0, height),
            );
            painter.rect_filled(r, 2.0, color);
        };
        bar(slider, 20.0, v.widgets.inactive.bg_fill);
        bar(sent, 10.0, v.strong_text_color());
        let color = if digital {
            v.selection.bg_fill
        } else {
            v.weak_text_color()
        };
        painter.text(
            egui::pos2(rect.left() + 8.0, top + 10.0),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(14.0),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let f = std::env::temp_dir().join(format!("melee-hd-phob-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&f);
        f
    }

    #[test]
    fn profiles_and_ports_are_kept() {
        let f = folder("keep");
        let mut p = PhobPanel::load(f.clone());
        let name = p.new_name();
        assert_eq!(name, "Controller 1");
        p.create(name.clone());
        p.profiles.get_mut(&name).unwrap().settings.l_mode = 3;
        p.save_profile(&name);
        p.ports[2] = Port {
            on: true,
            profile: name.clone(),
        };
        p.ports[0] = Port {
            on: false,
            profile: name.clone(),
        };
        p.save_ports();
        let back = PhobPanel::load(f.clone());
        assert_eq!(back.ports, p.ports);
        assert_eq!(back.profiles, p.profiles);
        assert_eq!(back.profile_on(2).unwrap().settings.l_mode, 3);
        assert!(back.profile_on(0).is_none());
        let _ = std::fs::remove_dir_all(&f);
    }

    #[test]
    fn a_port_naming_a_missing_profile_is_off() {
        let f = folder("missing");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(
            f.join("ports.txt"),
            "adapter-port-1 = on Gone\nadapter-port-9 = on X\n",
        )
        .unwrap();
        let p = PhobPanel::load(f.clone());
        assert!(!p.ports[0].on);
        assert_eq!(p.ports[0].profile, "Gone");
        let _ = std::fs::remove_dir_all(&f);
    }

    #[test]
    fn the_views_draw() {
        let samples: Vec<Sample> = (0..1500)
            .map(|i| {
                let t = i as f32 / 40.0;
                Sample {
                    step_ms: 8.0,
                    a: [(t.cos() * 80.0) as i32, (t.sin() * 80.0) as i32],
                    c: [0, (t.sin() * 60.0) as i32],
                    a_unfiltered: [t.cos() * 95.0, t.sin() * 95.0],
                    c_unfiltered: [0.0, 0.0],
                    sliders: [(i % 256) as u8, 30],
                    triggers: [(i % 200) as u8, 0],
                    l: i % 3 == 0,
                    r: false,
                }
            })
            .collect();
        let ctx = egui::Context::default();
        for empty in [false, true] {
            let s: &[Sample] = if empty { &[] } else { &samples };
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                stick_map(ui, s);
                snapback(ui, s);
                triggers(ui, s);
            });
        }
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Sarah's pad 2"), "Sarah_s pad 2.txt");
        assert_eq!(file_name("../x"), "___x.txt");
    }

    #[test]
    fn prompts() {
        let mut cal = melee_phob::Calibration::start(WhichStick::A, Default::default());
        assert_eq!(prompt(&cal), CENTRE_PROMPT);
        cal.step = 1;
        assert_eq!(
            prompt(&cal),
            "Hold the stick at the right notch and press A"
        );
        cal.step = 17;
        assert!(prompt(&cal).contains("right, toward up-right"));
        cal.step = 32;
        assert_eq!(prompt(&cal), "Adjust the up-right notch");
    }
}
