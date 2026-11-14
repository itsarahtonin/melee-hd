//! The settings panel's Quick match tab (Sarah, 2026-11-14: "a tiny lil quickmatch section that
//! has char icons, 1 for each port, with human/cpu toggles for each, plus a start button"):
//! four ports, each a character or none, human or a computer player at a level, a stage and
//! Start. Laid out only: Start waits for the Slippi ruleset's task 4, which builds each match
//! from its game start the way playback builds a replay's, so a quick match is exact by
//! construction (docs/player.md, "Quick match: the plan"). Nothing here starts a match yet.

use melee_sim::game::roster;

/// The ports' colors, as Melee's character select draws them.
const COLORS: [egui::Color32; 4] = [
    egui::Color32::from_rgb(0xE8, 0x40, 0x40),
    egui::Color32::from_rgb(0x48, 0x78, 0xF0),
    egui::Color32::from_rgb(0xF0, 0xC8, 0x30),
    egui::Color32::from_rgb(0x40, 0xB8, 0x50),
];

/// The note under Start while it waits.
pub const WAITING: &str = "Quick match comes with the Slippi ruleset, which starts each match the way a replay plays back, so it plays exactly.";

/// A port: its character (`CharacterKind`, none: empty), and human or a computer player at a
/// level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Port {
    character: Option<u8>,
    cpu: bool,
    level: u8,
}

/// The tab's choices, kept while the player runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickMatch {
    ports: [Port; 4],
    /// The stage, by `StKind`.
    stage: u16,
}

impl Default for QuickMatch {
    fn default() -> Self {
        let none = Port {
            character: None,
            cpu: false,
            level: 9,
        };
        QuickMatch {
            ports: [
                Port {
                    character: Some(2), // Fox
                    ..none
                },
                Port {
                    character: Some(9), // Marth
                    cpu: true,
                    ..none
                },
                none,
                none,
            ],
            stage: 31, // Battlefield
        }
    }
}

impl QuickMatch {
    /// Lays the tab out: the four ports side by side, the stage and Start.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut characters: Vec<_> = roster::offered_characters().collect();
        characters.sort_by_key(|c| c.name);
        let mut stages: Vec<_> = roster::offered_stages().collect();
        stages.sort_by_key(|s| s.name);
        let name = |kind: Option<u8>| {
            kind.and_then(roster::character_by_kind)
                .map_or("None", |c| c.name)
        };
        ui.columns(4, |cols| {
            for (i, (ui, port)) in cols.iter_mut().zip(&mut self.ports).enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!("P{}", i + 1))
                                .strong()
                                .color(COLORS[i]),
                        );
                        // The character's icon goes here (text until the disc's icons are
                        // drawn: docs/player.md).
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(name(port.character)).heading());
                        ui.add_space(4.0);
                        egui::ComboBox::from_id_salt(("quick-character", i))
                            .selected_text(name(port.character))
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut port.character, None, "None");
                                for c in &characters {
                                    ui.selectable_value(&mut port.character, Some(c.kind), c.name);
                                }
                            });
                        ui.add_enabled_ui(port.character.is_some(), |ui| {
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut port.cpu, false, "Human");
                                ui.selectable_value(&mut port.cpu, true, "CPU");
                            });
                            ui.add_enabled(
                                port.cpu,
                                egui::DragValue::new(&mut port.level)
                                    .range(1..=9)
                                    .prefix("Level "),
                            );
                        });
                    });
                });
            }
        });
        ui.add_space(12.0);
        egui::Grid::new("quick")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Stage");
                let stage = &mut self.stage;
                egui::ComboBox::from_id_salt("quick-stage")
                    .selected_text(roster::stage_by_kind(*stage).map_or("", |s| s.name))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for s in &stages {
                            ui.selectable_value(stage, s.kind, s.name);
                        }
                    });
                ui.end_row();
                ui.label("");
                ui.weak("4 stocks, 8 minutes, items off.");
                ui.end_row();
            });
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.add_enabled(
                false,
                egui::Button::new("Start").min_size(egui::vec2(130.0, 30.0)),
            );
            ui.add_space(4.0);
            ui.weak(WAITING);
        });
    }
}
