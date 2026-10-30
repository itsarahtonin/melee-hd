//! The player's front end (milestone 5's task 18, Sarah's call of 2026-10-28): melee-hd boots
//! straight into Melee, and the window's only screen of its own is the settings panel, which
//! Escape opens over the game, paused, and closes again. It holds what belongs to this
//! machine and the player rather than the game: the display, the latency, the save's import,
//! export and unlock-all, and recording; matches start from the game's own menus. The panel
//! is egui on the player's own device and window ([`crate::ui`]).
//!
//! The settings are kept in the settings folder's `settings.txt` (`key = value` lines; an
//! older `last-match.txt`, the launcher's, is read when there's none yet), except those the
//! command line gives.

use crate::jit::Timing;
use crate::pacing::Mode;
use crate::present::{Aspect, Present};

/// The file that keeps the settings.
pub const FILE: &str = "settings.txt";
/// The launcher's file, read once for a settings file that isn't there yet.
const OLD_FILE: &str = "last-match.txt";

/// The widest timing margin, in milliseconds.
pub const MAX_MARGIN_MS: f32 = 8.0;

/// A just-in-time margin in milliseconds, 0 to [`MAX_MARGIN_MS`].
pub fn parse_margin(v: &str) -> Option<f32> {
    v.parse()
        .ok()
        .filter(|m: &f32| (0.0..=MAX_MARGIN_MS).contains(m))
}

/// The settings the command line gave, which win over the kept ones.
#[derive(Clone, Copy, Default)]
pub struct Given {
    pub aspect: bool,
    pub fullscreen: bool,
    pub present: bool,
    pub added_delay: bool,
    pub high_refresh: bool,
    pub tick_timing: bool,
    pub jit_margin: bool,
    pub scale: bool,
    pub msaa: bool,
    pub record: bool,
    pub replays: bool,
    pub record_sessions: bool,
    pub debug_level: bool,
}

/// The player's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The aspect ratio frames are shown at.
    pub aspect: Aspect,
    pub fullscreen: bool,
    /// The internal resolution's scale: 0 follows the window, else that many times the
    /// console's.
    pub scale: u32,
    /// Four samples a pixel.
    pub msaa: bool,
    pub high_refresh: Mode,
    pub present: Present,
    /// Ticks each controller read is held back.
    pub added_delay: u32,
    pub tick_timing: Timing,
    /// The just-in-time tick's margin, in milliseconds.
    pub jit_margin: f32,
    /// Saves each VS match as a Slippi replay, to `replays` (empty: beside the program).
    pub record: bool,
    pub replays: String,
    /// Records each session from power-on (`session_rec`).
    pub record_sessions: bool,
    /// The developer setting: the debug level Melee's next power-on raises `DbLevel` to
    /// ([`DEBUG_LEVELS`]), 0 for retail's.
    pub debug_level: i32,
}

/// The developer setting's levels (Sarah, 2026-10-29: Off, Debug ROM and Develop): each
/// `DbLKind`, its name and what it turns on. Levels 1 and 2 only add the title's build date and
/// the disc cache's assertions, which can only stop the game.
pub const DEBUG_LEVELS: [(i32, &str, &str); 3] = [
    (0, "Off", "Melee as sold."),
    (
        3,
        "Debug ROM",
        "HAL's debug menu (Y at the title), debug VS (A) and the sound test's debug mode (X); in matches, the develop tools, the debug pause and the frame step.",
    ),
    (
        4,
        "Develop",
        "Debug ROM's tools and the develop level's extras: the effect-pool limits and the item counts' reset.",
    ),
];

/// The developer setting's help line.
pub const DEBUG_HELP: &str = "Raises Melee's debug level, as an Action Replay code does. Applies from Melee's next start (Restart Melee\u{2026}), and never to a replay.";
/// The warning while the debug level isn't Off.
pub const DEBUG_WARNING: &str = "The debug menu switches your save's language to Japanese as it opens (Melee's own bug), and its entries can change unlocks and records, which Melee saves to your card. Some debug tools aren't in melee-hd yet and stop the game.";
/// The "Save replays" setting's help line (Sarah, 2026-10-29).
pub const REPLAYS_HELP: &str =
    "Replays of matches started from Melee's menus may not play back exactly in Slippi Dolphin.";

/// A debug level's line of [`DEBUG_LEVELS`] (Off's for one not offered).
fn debug_level(level: i32) -> (i32, &'static str, &'static str) {
    DEBUG_LEVELS
        .into_iter()
        .find(|l| l.0 == level)
        .unwrap_or(DEBUG_LEVELS[0])
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            aspect: Aspect::GAME,
            fullscreen: false,
            scale: 0,
            msaa: false,
            high_refresh: Mode::Off,
            present: Present::Mailbox,
            added_delay: 0,
            tick_timing: Timing::Timer,
            jit_margin: (crate::jit::DEFAULT_MARGIN.as_secs_f64() * 1000.0) as f32,
            record: true,
            replays: String::new(),
            record_sessions: true,
            debug_level: 0,
        }
    }
}

impl Settings {
    /// The settings as the settings folder keeps them, one `key = value` a line.
    pub fn save(&self) -> String {
        let aspect = match self.aspect {
            a if a.widen => format!("{}:1", a.shown),
            Aspect::STRETCHED => "4:3".to_owned(),
            _ => "73:60".to_owned(),
        };
        let scale = match self.scale {
            0 => "auto".to_owned(),
            n => n.to_string(),
        };
        let present = match self.present {
            Present::Mailbox => "mailbox",
            Present::Fifo => "fifo",
            Present::Immediate => "immediate",
        };
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "aspect = {aspect}\nfullscreen = {}\nscale = {scale}\nmsaa = {}\nhigh-refresh = {}\npresent = {present}\nadded-delay = {}\ntick-timing = {}\njit-margin = {}\nrecord = {}\nreplays = {}\nrecord-sessions = {}\ndebug-level = {}\n",
            self.fullscreen,
            if self.msaa { "4" } else { "off" },
            self.high_refresh.key(),
            self.added_delay,
            self.tick_timing.key(),
            self.jit_margin,
            on(self.record),
            self.replays,
            on(self.record_sessions),
            match self.debug_level {
                3 => "debug-rom",
                4 => "develop",
                _ => "off",
            },
        )
    }

    /// Takes back what [`Settings::save`] kept, except what the command line gave; a line
    /// missing or unreadable leaves its setting as it is.
    pub fn restore(&mut self, text: &str, given: &Given) {
        let on = |v: &str| match v {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        };
        for line in text.lines() {
            let Some((key, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match key.trim() {
                "aspect" if !given.aspect => {
                    if let Ok(a) = Aspect::parse(v) {
                        self.aspect = a;
                    }
                }
                "fullscreen" if !given.fullscreen => {
                    if let Ok(f) = v.parse() {
                        self.fullscreen = f;
                    }
                }
                "scale" if !given.scale => {
                    if v == "auto" {
                        self.scale = 0;
                    } else if let Some(n) = v.parse().ok().filter(|n| (1..=8).contains(n)) {
                        self.scale = n;
                    }
                }
                "msaa" if !given.msaa => match v {
                    "4" => self.msaa = true,
                    "off" | "1" => self.msaa = false,
                    _ => {}
                },
                "high-refresh" if !given.high_refresh => {
                    if let Ok(m) = Mode::parse(v) {
                        self.high_refresh = m;
                    }
                }
                "present" if !given.present => {
                    if let Ok(p) = Present::parse(v) {
                        self.present = p;
                    }
                }
                "added-delay" if !given.added_delay => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 15) {
                        self.added_delay = n;
                    }
                }
                "tick-timing" if !given.tick_timing => {
                    if let Ok(t) = Timing::parse(v) {
                        self.tick_timing = t;
                    }
                }
                "jit-margin" if !given.jit_margin => {
                    if let Some(m) = parse_margin(v) {
                        self.jit_margin = m;
                    }
                }
                "record" if !given.record => {
                    if let Some(b) = on(v) {
                        self.record = b;
                    }
                }
                "replays" if !given.replays => self.replays = v.to_owned(),
                "record-sessions" if !given.record_sessions => {
                    if let Some(b) = on(v) {
                        self.record_sessions = b;
                    }
                }
                "debug-level" if !given.debug_level => match v {
                    "off" => self.debug_level = 0,
                    "debug-rom" => self.debug_level = 3,
                    "develop" => self.debug_level = 4,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    /// The kept settings over `self`, but what the command line gave.
    pub fn load(&mut self, given: &Given) {
        if let Some(text) = crate::settings::read(FILE).or_else(|| crate::settings::read(OLD_FILE))
        {
            self.restore(&text, given);
        }
    }

    /// Keeps the settings in the settings folder.
    pub fn keep(&self) {
        crate::settings::write(FILE, &self.save());
    }
}

/// What the panel asks the player to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Resume,
    Restart,
    Quit,
    ChangeDisc,
    Import,
    Export,
    UnlockAll,
}

/// The panel: the settings and what it shows of the save and the disc.
pub struct Panel {
    pub settings: Settings,
    /// Whether it shows (the console paused behind it).
    pub open: bool,
    /// Why Melee stopped, or what the last action did.
    pub message: Option<String>,
    /// Melee stopped: it can't be resumed, only restarted.
    pub halted: bool,
    /// The save's line, and whether everything is unlocked in it (none: no save).
    pub save_line: String,
    pub unlocked: Option<bool>,
    /// The disc's path.
    pub disc: String,
}

impl Panel {
    pub fn new(settings: Settings, disc: String) -> Panel {
        let mut p = Panel {
            settings,
            open: false,
            message: None,
            halted: false,
            save_line: String::new(),
            unlocked: None,
            disc,
        };
        p.look_at_save();
        p
    }

    /// Reads what the saves folder holds, for the panel's save line.
    pub fn look_at_save(&mut self) {
        let dir = crate::saves::dir();
        let path = dir.join(crate::saves::SAVE_FILE);
        self.unlocked = crate::saves::unlocked(&dir);
        self.save_line = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => format!(
                "Your save: {}, last written {}",
                crate::saves::SAVE_FILE,
                written_at(t)
            ),
            Err(_) => "Your save: none yet".to_owned(),
        };
    }

    /// Lays the panel out; what was pressed, if anything.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        egui::ScrollArea::vertical().show(ui, |ui| action = self.show_panel(ui));
        action
    }

    fn show_panel(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.heading("Settings");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 440.0) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_max_width(440.0);
                    action = self.show_sections(ui);
                });
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 420.0) / 2.0).max(0.0));
                let size = egui::vec2(130.0, 30.0);
                if ui
                    .add_enabled(!self.halted, egui::Button::new("Resume").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Resume);
                }
                if ui
                    .add(egui::Button::new("Restart Melee\u{2026}").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Restart);
                }
                if ui
                    .add(egui::Button::new("Quit melee-hd").min_size(size))
                    .clicked()
                {
                    action = Some(Action::Quit);
                }
            });
            if let Some(m) = &self.message {
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, m);
            }
            ui.add_space(16.0);
            ui.weak("Esc: settings. F11 or Alt+Enter: fullscreen. F3: latency overlay. F12: bug capture.");
            ui.add_space(16.0);
        });
        action
    }

    fn show_sections(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let s = &mut self.settings;
        section(ui, "Display");
        egui::Grid::new("display")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Resolution");
                let (scale, aspect) = (&mut s.scale, s.aspect);
                let name = |n: u32| match n {
                    0 => "Auto (the window's)".to_owned(),
                    n => format!("{n}x ({}x{})", aspect.efb_width(n), 480 * n),
                };
                *scale = (*scale).min(aspect.max_scale());
                egui::ComboBox::from_id_salt("scale")
                    .selected_text(name(*scale))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for n in 0..=aspect.max_scale() {
                            ui.selectable_value(scale, n, name(n));
                        }
                    });
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut s.msaa, "Antialiasing (MSAA 4x)");
                ui.end_row();
                ui.label("Aspect ratio");
                let aspect = &mut s.aspect;
                egui::ComboBox::from_id_salt("aspect")
                    .selected_text(aspect.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        let mut choices: Vec<Aspect> =
                            Aspect::CHOICES.iter().map(|(a, _)| *a).collect();
                        if !choices.contains(aspect) {
                            choices.push(*aspect);
                        }
                        for a in choices {
                            ui.selectable_value(aspect, a, a.name());
                        }
                    });
                ui.end_row();
                help(
                    ui,
                    "Wider ratios show more of the stage at the sides; gameplay is unchanged.",
                );
                ui.label("");
                ui.checkbox(&mut s.fullscreen, "Fullscreen");
                ui.end_row();
                ui.label("High refresh");
                let mode = &mut s.high_refresh;
                egui::ComboBox::from_id_salt("high-refresh")
                    .selected_text(mode.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (m, name) in Mode::CHOICES {
                            ui.selectable_value(mode, m, name);
                        }
                    });
                ui.end_row();
                help(
                    ui,
                    "Draws frames between the game's 60 a second for faster displays. Extrapolate adds no latency.",
                );
            });
        section(ui, "Latency");
        egui::Grid::new("latency")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Present");
                let present = &mut s.present;
                egui::ComboBox::from_id_salt("present")
                    .selected_text(present.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (p, name) in Present::CHOICES {
                            ui.selectable_value(present, p, name);
                        }
                    });
                ui.end_row();
                ui.label("Added delay");
                ui.add(
                    egui::DragValue::new(&mut s.added_delay)
                        .range(0..=15)
                        .suffix(" frames"),
                );
                ui.end_row();
                help(
                    ui,
                    "Holds the controllers back on purpose, for a CRT's feel.",
                );
                ui.label("Tick timing");
                let timing = &mut s.tick_timing;
                egui::ComboBox::from_id_salt("tick-timing")
                    .selected_text(timing.name())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (t, name) in Timing::CHOICES {
                            ui.selectable_value(timing, t, name);
                        }
                    });
                ui.end_row();
                ui.label("Timing margin");
                ui.add_enabled(
                    s.tick_timing != Timing::Timer,
                    egui::DragValue::new(&mut s.jit_margin)
                        .range(0.0..=MAX_MARGIN_MS)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .suffix(" ms"),
                );
                ui.end_row();
            });
        section(ui, "Save");
        ui.label(&self.save_line);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import a save\u{2026}").clicked() {
                action = Some(Action::Import);
            }
            if ui
                .add_enabled(
                    self.unlocked.is_some(),
                    egui::Button::new("Export the save\u{2026}"),
                )
                .clicked()
            {
                action = Some(Action::Export);
            }
            let b = ui.add_enabled(
                self.unlocked == Some(false),
                egui::Button::new("Unlock everything\u{2026}"),
            );
            let b = if self.unlocked == Some(true) {
                b.on_disabled_hover_text("Everything is unlocked.")
            } else {
                b
            };
            if b.clicked() {
                action = Some(Action::UnlockAll);
            }
        });
        if self.unlocked == Some(true) {
            ui.weak("Everything is unlocked.");
        }
        let s = &mut self.settings;
        section(ui, "Recording");
        egui::Grid::new("recording")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("");
                ui.checkbox(&mut s.record, "Save replays");
                ui.end_row();
                help(ui, REPLAYS_HELP);
                ui.label("Replays folder");
                ui.add_enabled(
                    s.record,
                    egui::TextEdit::singleline(&mut s.replays)
                        .hint_text("replays beside the program")
                        .desired_width(220.0),
                );
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut s.record_sessions, "Record sessions");
                ui.end_row();
                help(
                    ui,
                    "Keeps a recording of each session from power-on, to report a bug.",
                );
            });
        section(ui, "Developer");
        egui::Grid::new("developer")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Debug level");
                let level = &mut s.debug_level;
                egui::ComboBox::from_id_salt("debug-level")
                    .selected_text(debug_level(*level).1)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (l, name, _) in DEBUG_LEVELS {
                            ui.selectable_value(level, l, name);
                        }
                    });
                ui.end_row();
                help(ui, debug_level(*level).2);
                help(ui, DEBUG_HELP);
                if *level != 0 {
                    ui.label("");
                    let warn = ui.visuals().warn_fg_color;
                    ui.colored_label(warn, DEBUG_WARNING);
                    ui.end_row();
                }
            });
        section(ui, "Disc");
        ui.label(&self.disc);
        if ui.button("Change disc\u{2026}").clicked() {
            action = Some(Action::ChangeDisc);
        }
        action
    }
}

/// A section's heading.
fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(12.0);
    ui.strong(title);
    ui.separator();
}

/// A help line under a grid's row.
fn help(ui: &mut egui::Ui, text: &str) {
    ui.label("");
    ui.weak(text);
    ui.end_row();
}

/// A file's time as the panel says it: the local date and time to the minute (UTC off
/// Windows).
fn written_at(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let offset = local_offset();
    let local = (secs as i64 + offset).max(0) as u64;
    let (y, mo, d) = civil(local / 86_400);
    let (h, mi) = (local % 86_400 / 3600, local % 3600 / 60);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

/// The local time's offset from UTC now, in seconds.
fn local_offset() -> i64 {
    let [y, mo, d, h, mi, s] = crate::recording::local_now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let days = crate::power::days_since_2000(y, mo, d) as i64 + 10_957;
    let local = days * 86_400 + i64::from(h * 3600 + mi * 60 + s);
    // Rounded to the quarter hour, which every time zone's offset is a multiple of.
    ((local - now) as f64 / 900.0).round() as i64 * 900
}

/// A date from days since 1970-01-01.
fn civil(days: u64) -> (u64, u64, u64) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u64, m as u64, d as u64)
}

/// The note over the game when a gated menu entry is refused (`gates`): a dark box with
/// white text near the bottom of the frame.
pub fn note(
    painter: &mut crate::ui::Painter,
    gpu: &crate::present::Gpu,
    text: &str,
    view: &wgpu::TextureView,
    w: u32,
    h: u32,
) {
    let zoom = painter.ctx.zoom_factor();
    let size = egui::vec2(w as f32, h as f32) / zoom;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let out = painter.ctx.run_ui(input, |ui| {
        egui::Area::new(egui::Id::new("gate-note"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -size.y * 0.08])
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(210))
                    .inner_margin(12.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_max_width(size.x * 0.8);
                        ui.label(egui::RichText::new(text).color(egui::Color32::WHITE));
                    });
            });
    });
    painter.paint(gpu, out, view, w, h);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked() -> Settings {
        Settings {
            aspect: Aspect::wide(16.0 / 9.0),
            fullscreen: true,
            scale: 3,
            msaa: true,
            high_refresh: Mode::Interpolate,
            present: Present::Immediate,
            added_delay: 2,
            tick_timing: Timing::Vrr,
            jit_margin: 2.5,
            record: false,
            replays: "D:/Slippi/melee-hd".to_owned(),
            record_sessions: false,
            debug_level: 4,
        }
    }

    #[test]
    fn the_settings_come_back() {
        for aspect in Aspect::CHOICES.map(|(a, _)| a) {
            for (mode, _) in Mode::CHOICES {
                let was = Settings {
                    aspect,
                    high_refresh: mode,
                    ..picked()
                };
                let mut s = Settings::default();
                s.restore(&was.save(), &Given::default());
                assert_eq!(s, was, "{}", was.save());
            }
        }
    }

    #[test]
    fn the_command_line_wins() {
        let mut s = Settings {
            scale: 2,
            aspect: Aspect::STRETCHED,
            ..Settings::default()
        };
        let given = Given {
            scale: true,
            aspect: true,
            ..Given::default()
        };
        s.restore(&picked().save(), &given);
        assert_eq!((s.scale, s.aspect), (2, Aspect::STRETCHED));
        assert!(s.msaa && !s.record_sessions);
    }

    #[test]
    fn an_unreadable_file_changes_nothing() {
        let mut s = Settings::default();
        s.restore(
            "aspect = 1:2\nfullscreen = maybe\nscale = 9\nmsaa = 2\nadded-delay = -1\nhigh-refresh = sometimes\ntick-timing = soon\njit-margin = 99\nrecord = perhaps\n\u{0}garbage\n",
            &Given::default(),
        );
        assert_eq!(s, Settings::default());
    }

    /// The launcher's last match file still gives its display and latency settings.
    #[test]
    fn the_launchers_file_is_read() {
        let mut s = Settings::default();
        s.restore(
            "p1 = fox\nstage = fd\nscale = 4\nmsaa = 4\npresent = fifo\nrecord = off\n",
            &Given::default(),
        );
        assert_eq!(
            (s.scale, s.msaa, s.present, s.record),
            (4, true, Present::Fifo, false)
        );
    }

    /// The developer setting keeps its level, off by default, and nothing else reads as one.
    #[test]
    fn the_debug_level_is_kept() {
        assert_eq!(Settings::default().debug_level, 0);
        for level in [0, 3, 4] {
            let s = Settings {
                debug_level: level,
                ..Settings::default()
            };
            let mut back = Settings::default();
            back.restore(&s.save(), &Given::default());
            assert_eq!(back, s);
        }
        let mut s = Settings::default();
        s.restore("debug-level = 2\n", &Given::default());
        assert_eq!(s.debug_level, 0);
    }

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_754), (2026, 10, 28));
    }
}
