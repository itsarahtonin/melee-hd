//! The launcher: started with no match on its command line, the player's window shows a small
//! form (a character or none for each port, each human or a CPU at a level, a stage, stocks and minutes, the items, the aspect
//! ratio and fullscreen) drawn with egui on the
//! player's own GPU device and window, and Start plays that match in the same window. Escape in
//! the match comes back here; Escape here quits. It opens with the choices last started from it,
//! which the settings folder keeps (`settings`), except those the command line gives. A tool for
//! manual testing, and the seed of the menus to come.

use std::path::Path;
use std::time::{Duration, Instant};

use melee_sim::game::roster;

use crate::jit::Timing;
use crate::pacing::Mode;
use crate::present::{Aspect, Gpu, Present, Shown};
use crate::{cpu, items};

/// A match the launcher picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// Each port's `CharacterKind`, or none.
    pub players: [Option<u8>; 4],
    /// Each port's CPU level (1 to 9), or none for a human player.
    pub cpus: [Option<u8>; 4],
    /// `StKind`.
    pub stage: u16,
    pub stocks: u8,
    pub minutes: u8,
    /// The item frequency and switch, of which the match gets the kinds offered for its stage
    /// and characters (`items`).
    pub items: items::Switch,
}

impl Choice {
    pub fn player_count(&self) -> usize {
        self.players.iter().flatten().count()
    }

    /// The players' `CharacterKind`s.
    pub fn characters(&self) -> Vec<u8> {
        self.players.iter().flatten().copied().collect()
    }

    /// What the match gets of the item setting.
    pub fn items(&self) -> items::ForMatch {
        self.items.for_match(&self.characters(), self.stage)
    }

    /// Why the match can't start: fewer than two players, a character whose own items could
    /// stop the match (`roster::characters_refusal`), a stage that isn't offered or could
    /// stop with these characters (`Stage::refusal`), or computer players, Nana among them,
    /// that could stop it (`cpu::refusal`).
    pub fn refusal(&self) -> Option<String> {
        if self.player_count() < 2 {
            return Some("Pick at least two players.".into());
        }
        let characters = self.characters();
        if let Some(why) = roster::characters_refusal(&characters) {
            return Some(why);
        }
        let stage = match roster::stage_by_kind(self.stage) {
            Some(s) => s.refusal(&characters),
            None => Some("Pick a stage.".into()),
        };
        stage.or_else(|| self.cpu_refusal(&self.items().kinds))
    }

    /// Why the CPU audit refuses the match with the item kinds it gets.
    pub fn cpu_refusal(&self, kinds: &[u8]) -> Option<String> {
        cpu::refusal(&self.players, &self.cpus, self.stage, kinds)
    }

    /// Each port's character and costume as the command line keeps them (0xFF: none). A
    /// character a port before has already picked takes the next costume, so the two look
    /// apart.
    pub fn ports(&self) -> Vec<(u8, u8)> {
        let p = &self.players;
        (0..4)
            .map(|i| match p[i] {
                Some(c) => (c, p[..i].iter().filter(|&&q| q == Some(c)).count() as u8),
                None => (0xFF, 0),
            })
            .collect()
    }
}

/// The widest margin the launcher and the command line take, in milliseconds.
pub const MAX_MARGIN_MS: f32 = 8.0;

/// A just-in-time margin in milliseconds, 0 to [`MAX_MARGIN_MS`].
pub fn parse_margin(v: &str) -> Option<f32> {
    v.parse()
        .ok()
        .filter(|m: &f32| (0.0..=MAX_MARGIN_MS).contains(m))
}

/// The form's choices the command line gave, which win over the last match's.
#[derive(Clone, Copy, Default)]
pub struct Given {
    pub stocks: bool,
    pub minutes: bool,
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
}

/// The form and what it last had to say.
pub struct Form {
    pub choice: Choice,
    /// Why the last match didn't start or stopped.
    pub message: Option<String>,
    /// Whether the window is to be fullscreen (the player follows the box, and the box F11).
    pub fullscreen: bool,
    /// The aspect ratio the match is shown at.
    pub aspect: Aspect,
    /// The internal resolution's scale: 0 follows the window (the smallest scale whose 480
    /// lines cover its height), else that many times the console's.
    pub scale: u32,
    /// Four samples a pixel.
    pub msaa: bool,
    /// How frames reach the display, and the input latency added back on purpose (ticks).
    pub present: Present,
    pub added_delay: u32,
    /// How frames are drawn between ticks.
    pub high_refresh: Mode,
    /// How ticks are timed, and the just-in-time tick's margin in milliseconds.
    pub tick_timing: Timing,
    pub jit_margin: f32,
    /// Saves each match as a Slippi replay, to `replays` (empty: the replays folder beside the
    /// program).
    pub record: bool,
    pub replays: String,
}

impl Form {
    /// Fox against Fox on Final Destination, as the command line's defaults are.
    pub fn new(stocks: u8, minutes: u8, fullscreen: bool, aspect: Aspect) -> Form {
        let fox = roster::character("fox").map(|c| c.kind);
        Form {
            choice: Choice {
                players: [fox, fox, None, None],
                cpus: [None; 4],
                stage: roster::stage("fd").map_or(32, |s| s.kind),
                stocks,
                minutes,
                items: items::Switch::default(),
            },
            message: None,
            fullscreen,
            aspect,
            scale: 0,
            msaa: false,
            present: Present::Mailbox,
            added_delay: 0,
            high_refresh: Mode::Off,
            tick_timing: Timing::Timer,
            jit_margin: (crate::jit::DEFAULT_MARGIN.as_secs_f64() * 1000.0) as f32,
            record: true,
            replays: String::new(),
        }
    }

    /// The choices as the settings folder keeps them, one `key = value` a line.
    pub fn save(&self) -> String {
        let c = &self.choice;
        let mut text = String::new();
        for (i, p) in c.players.iter().enumerate() {
            let name = p
                .and_then(roster::character_by_kind)
                .map_or("none", |c| c.keys[0]);
            text += &format!("p{} = {name}\n", i + 1);
        }
        for (i, l) in c.cpus.iter().enumerate() {
            text += &format!("p{}-cpu = {}\n", i + 1, cpu::save(*l));
        }
        if let Some(s) = roster::stage_by_kind(c.stage) {
            text += &format!("stage = {}\n", s.keys[0]);
        }
        let aspect = match self.aspect {
            a if a.widen => format!("{}:1", a.shown),
            Aspect::STRETCHED => "4:3".to_owned(),
            _ => "73:60".to_owned(),
        };
        let scale = match self.scale {
            0 => "auto".to_owned(),
            n => n.to_string(),
        };
        let msaa = if self.msaa { "4" } else { "off" };
        let present = match self.present {
            Present::Mailbox => "mailbox",
            Present::Fifo => "fifo",
            Present::Immediate => "immediate",
        };
        text += &format!(
            "stocks = {}\nminutes = {}\naspect = {aspect}\nfullscreen = {}\nscale = {scale}\nmsaa = {msaa}\npresent = {present}\nadded-delay = {}\nhigh-refresh = {}\ntick-timing = {}\njit-margin = {}\n",
            c.stocks,
            c.minutes,
            self.fullscreen,
            self.added_delay,
            self.high_refresh.key(),
            self.tick_timing.key(),
            self.jit_margin
        );
        text += &format!(
            "record = {}\nreplays = {}\n",
            if self.record { "on" } else { "off" },
            self.replays
        );
        text += &c.items.save();
        text
    }

    /// Takes back the choices [`Form::save`] kept, except those the command line gave; a line
    /// missing or unreadable leaves its choice as it is.
    pub fn restore(&mut self, text: &str, given: &Given) {
        let c = &mut self.choice;
        for line in text.lines() {
            let Some((key, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match key.trim() {
                k @ ("p1" | "p2" | "p3" | "p4") => {
                    let port = usize::from(k.as_bytes()[1] - b'1');
                    if v == "none" {
                        c.players[port] = None;
                    } else if let Some(ch) = roster::character(v) {
                        c.players[port] = Some(ch.kind);
                    }
                }
                k @ ("p1-cpu" | "p2-cpu" | "p3-cpu" | "p4-cpu") => {
                    if let Some(l) = cpu::restore(v) {
                        c.cpus[usize::from(k.as_bytes()[1] - b'1')] = l;
                    }
                }
                "stage" => {
                    if let Some(s) = roster::stage(v).filter(|s| s.offered()) {
                        c.stage = s.kind;
                    }
                }
                "stocks" if !given.stocks => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 99) {
                        c.stocks = n;
                    }
                }
                "minutes" if !given.minutes => {
                    if let Some(n) = v.parse().ok().filter(|n| *n <= 99) {
                        c.minutes = n;
                    }
                }
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
                "high-refresh" if !given.high_refresh => {
                    if let Ok(m) = Mode::parse(v) {
                        self.high_refresh = m;
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
                "record" if !given.record => match v {
                    "on" => self.record = true,
                    "off" => self.record = false,
                    _ => {}
                },
                "replays" if !given.replays => self.replays = v.to_owned(),
                k => {
                    c.items.restore(k, v);
                }
            }
        }
    }

    /// Lays the form out; true when Start is pressed (or Enter with nothing focused).
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut start = false;
        egui::ScrollArea::vertical().show(ui, |ui| start = self.show_form(ui));
        start
    }

    fn show_form(&mut self, ui: &mut egui::Ui) -> bool {
        let mut start = false;
        ui.vertical_centered(|ui| {
            ui.add_space(32.0);
            ui.heading("melee-hd");
            ui.add_space(16.0);
            let c = &mut self.choice;
            // The item kinds the match gets, which the CPU audit's gate asks with.
            let kinds = c.items().kinds;
            // The grid, centered: its width is about the labels' and the boxes'.
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 390.0) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    egui::Grid::new("match")
                        .num_columns(2)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            for i in 0..4 {
                                ui.label(format!("Port {}", i + 1));
                                ui.horizontal(|ui| {
                                    let p = &mut c.players[i];
                                    let name = p
                                        .and_then(roster::character_by_kind)
                                        .map_or("None", |c| c.name);
                                    egui::ComboBox::from_id_salt(("port", i))
                                        .selected_text(name)
                                        .width(200.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(p, None, "None");
                                            for ch in roster::offered_characters() {
                                                ui.selectable_value(p, Some(ch.kind), ch.name);
                                            }
                                        });
                                    show_control(ui, c, &kinds, i);
                                });
                                ui.end_row();
                            }
                            ui.label("Stage");
                            let name = roster::stage_by_kind(c.stage).map_or("?", |s| s.name);
                            let with_cpus = c
                                .players
                                .iter()
                                .zip(&c.cpus)
                                .any(|(p, l)| p.is_some() && l.is_some());
                            egui::ComboBox::from_id_salt("stage")
                                .selected_text(name)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for s in roster::offered_stages() {
                                        let status = match with_cpus {
                                            true => cpu::stage_status(
                                                &c.players, &c.cpus, s.kind, &kinds,
                                            ),
                                            false => cpu::Status::Offered,
                                        };
                                        offer(ui, &mut c.stage, s.kind, s.name, &status);
                                    }
                                });
                            ui.end_row();
                            ui.label("Stocks");
                            ui.add(egui::DragValue::new(&mut c.stocks).range(0..=99));
                            ui.end_row();
                            ui.label("Minutes");
                            ui.add(egui::DragValue::new(&mut c.minutes).range(0..=99));
                            ui.end_row();
                            ui.label("Items");
                            let freq = &mut c.items.freq;
                            let name = (items::FREQUENCIES.iter())
                                .find(|f| f.0 == *freq)
                                .map_or("?", |f| f.1);
                            egui::ComboBox::from_id_salt("items")
                                .selected_text(name)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (f, name, _) in items::FREQUENCIES {
                                        ui.selectable_value(freq, f, name);
                                    }
                                });
                            ui.end_row();
                            ui.label("Aspect");
                            let aspect = &mut self.aspect;
                            egui::ComboBox::from_id_salt("aspect")
                                .selected_text(aspect.name())
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    // The command line's, when it isn't one of the choices.
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
                            ui.label("");
                            ui.checkbox(&mut self.fullscreen, "Fullscreen");
                            ui.end_row();
                            ui.label("Resolution");
                            let (scale, aspect) = (&mut self.scale, self.aspect);
                            let name = |s: u32| match s {
                                0 => "Auto (the window's)".to_owned(),
                                s => format!("{s}x ({}x{})", aspect.efb_width(s), 480 * s),
                            };
                            // A scale past what this aspect's EFB allows is shown at the most.
                            *scale = (*scale).min(aspect.max_scale());
                            egui::ComboBox::from_id_salt("scale")
                                .selected_text(name(*scale))
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for s in 0..=aspect.max_scale() {
                                        ui.selectable_value(scale, s, name(s));
                                    }
                                });
                            ui.end_row();
                            ui.label("");
                            ui.checkbox(&mut self.msaa, "Antialiasing (MSAA 4x)");
                            ui.end_row();
                            ui.label("Present");
                            let present = &mut self.present;
                            egui::ComboBox::from_id_salt("present")
                                .selected_text(present.name())
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (p, name) in Present::CHOICES {
                                        ui.selectable_value(present, p, name);
                                    }
                                });
                            ui.end_row();
                            ui.label("Added delay");
                            ui.add(
                                egui::DragValue::new(&mut self.added_delay)
                                    .range(0..=15)
                                    .suffix(" frames"),
                            );
                            ui.end_row();
                            ui.label("High refresh");
                            let mode = &mut self.high_refresh;
                            egui::ComboBox::from_id_salt("high-refresh")
                                .selected_text(mode.name())
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (m, name) in Mode::CHOICES {
                                        ui.selectable_value(mode, m, name);
                                    }
                                });
                            ui.end_row();
                            ui.label("Tick timing");
                            let timing = &mut self.tick_timing;
                            egui::ComboBox::from_id_salt("tick-timing")
                                .selected_text(timing.name())
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (t, name) in Timing::CHOICES {
                                        ui.selectable_value(timing, t, name);
                                    }
                                });
                            ui.end_row();
                            ui.label("Timing margin");
                            ui.add_enabled(
                                self.tick_timing != Timing::Timer,
                                egui::DragValue::new(&mut self.jit_margin)
                                    .range(0.0..=MAX_MARGIN_MS)
                                    .speed(0.05)
                                    .fixed_decimals(2)
                                    .suffix(" ms"),
                            );
                            ui.end_row();
                            ui.label("");
                            ui.checkbox(&mut self.record, "Save replays");
                            ui.end_row();
                            ui.label("Replays folder");
                            ui.add_enabled(
                                self.record,
                                egui::TextEdit::singleline(&mut self.replays)
                                    .hint_text("replays beside the program")
                                    .desired_width(200.0),
                            );
                            ui.end_row();
                        });
                    let characters = c.characters();
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new("Item switch")
                        .id_salt("item-switch")
                        .show(ui, |ui| c.items.show_switch(ui, &characters, c.stage));
                });
            });
            ui.add_space(16.0);
            let refusal = c.refusal();
            let ready = refusal.is_none();
            let button = ui.add_enabled(
                ready,
                egui::Button::new("Start").min_size(egui::vec2(120.0, 32.0)),
            );
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter))
                && ui.memory(|m| m.focused().is_none())
                && !ui.ctx().any_popup_open();
            start = ready && (button.clicked() || enter);
            ui.add_space(8.0);
            if let Some(why) = &refusal {
                ui.label(why);
            }
            if let Some(note) = c.items.note(&c.characters(), c.stage) {
                ui.label(note);
            }
            if let Some(m) = &self.message {
                ui.colored_label(ui.visuals().error_fg_color, m);
            }
            ui.add_space(16.0);
            ui.weak("0 stocks: a timed match. 0 minutes: no time limit.");
            ui.weak("Items: the switch offers only items ported for the stage and characters.");
            ui.weak("CPU players: a level is offered only where nothing it reaches is unported.");
            ui.weak("Enter starts. Escape in a match comes back here; Escape here quits.");
            ui.weak("F11 or Alt+Enter: fullscreen. F3: latency overlay. F12: bug capture.");
            ui.weak("Added delay holds the controllers back on purpose, for a CRT's feel.");
        });
        start
    }
}

/// A drop-down's entry: selectable when offered, else shown disabled with the reason in a few
/// words and what isn't ported on hover, as the item switch shows a kind.
fn offer<T: PartialEq>(ui: &mut egui::Ui, value: &mut T, it: T, name: &str, status: &cpu::Status) {
    if *status == cpu::Status::Offered {
        ui.selectable_value(value, it, name);
    } else {
        let label = format!("{name}: {}", status.short());
        ui.add_enabled(false, egui::Button::selectable(*value == it, label))
            .on_disabled_hover_text(status.detail());
    }
}

/// Port `port`'s control: Human, or a CPU at a level, each level offered by the CPU audit
/// (`cpu::control_status`). Off with the port.
fn show_control(ui: &mut egui::Ui, c: &mut Choice, kinds: &[u8], port: usize) {
    let (players, cpus, stage) = (c.players, c.cpus, c.stage);
    ui.add_enabled_ui(players[port].is_some(), |ui| {
        egui::ComboBox::from_id_salt(("control", port))
            .selected_text(cpu::control_name(cpus[port]))
            .width(90.0)
            .show_ui(ui, |ui| {
                for level in std::iter::once(None).chain(cpu::LEVELS.map(Some)) {
                    let status = cpu::control_status(&players, &cpus, stage, kinds, port, level);
                    let name = cpu::control_name(level);
                    offer(ui, &mut c.cpus[port], level, &name, &status);
                }
            });
    });
}

/// How much larger than egui's own sizes the form is drawn.
const ZOOM: f32 = 1.5;

/// egui on the player's GPU: a context and a renderer for one target format.
pub struct Painter {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    /// Clears the target first (the launcher), or draws over it (the latency overlay).
    pub clear: bool,
}

impl Painter {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Painter {
        let ctx = egui::Context::default();
        ctx.set_zoom_factor(ZOOM);
        let renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: true,
                predictable_texture_filtering: false,
            },
        );
        Painter {
            ctx,
            renderer,
            clear: true,
        }
    }

    /// Runs the form over a frame's input: whether Start was pressed, and what to paint.
    pub fn run(&mut self, form: &mut Form, input: egui::RawInput) -> (bool, egui::FullOutput) {
        let mut start = false;
        let out = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default_margins().show(ui, |ui| start |= form.show(ui));
        });
        (start, out)
    }

    /// Paints a frame's shapes into `view`, `w` by `h` pixels.
    pub fn paint(
        &mut self,
        gpu: &Gpu,
        out: egui::FullOutput,
        view: &wgpu::TextureView,
        w: u32,
        h: u32,
    ) {
        let (device, queue) = (&gpu.device, &gpu.queue);
        let jobs = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.renderer.update_texture(device, queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: out.pixels_per_point,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut buffers = self
            .renderer
            .update_buffers(device, queue, &mut encoder, &jobs, &screen);
        // The overlay's pass is timed with the frame it's drawn over; the launcher's aren't.
        let time = if self.clear {
            None
        } else {
            gpu.timer
                .as_ref()
                .and_then(|t| t.pass(melee_gpu::gpu_time::Stage::Overlay))
        };
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("launcher"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: if self.clear {
                                wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    timestamp_writes: melee_gpu::gpu_time::writes(&time),
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        buffers.push(encoder.finish());
        queue.submit(buffers);
        for id in &out.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}

/// The launcher in the player's window: egui's painter and its window input.
pub struct Window {
    painter: Painter,
    state: egui_winit::State,
    /// When egui wants the next frame, if it asked for one later.
    pub repaint_at: Option<Instant>,
}

impl Window {
    pub fn new(gpu: &Gpu, shown: &Shown) -> Window {
        let painter = Painter::new(gpu, shown.format());
        let state = egui_winit::State::new(
            painter.ctx.clone(),
            egui::ViewportId::ROOT,
            &*shown.window,
            Some(shown.window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );
        shown.window.request_redraw();
        Window {
            painter,
            state,
            repaint_at: None,
        }
    }

    /// Hands a window event to egui, asking for a frame if egui wants one.
    pub fn event(&mut self, shown: &Shown, event: &winit::event::WindowEvent) {
        if self.state.on_window_event(&shown.window, event).repaint {
            shown.window.request_redraw();
        }
    }

    /// Whether a drop-down is open (Escape closes it rather than quitting).
    pub fn popup_open(&self) -> bool {
        self.painter.ctx.any_popup_open()
    }

    /// Draws the form in the window; true when Start was pressed.
    pub fn redraw(&mut self, gpu: &Gpu, shown: &mut Shown, form: &mut Form) -> bool {
        let input = self.state.take_egui_input(&shown.window);
        let (start, mut out) = self.painter.run(form, input);
        let platform = std::mem::take(&mut out.platform_output);
        self.state.handle_platform_output(&shown.window, platform);
        let delay = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        self.repaint_at = if delay.is_zero() {
            shown.window.request_redraw();
            None
        } else {
            Instant::now().checked_add(delay)
        };
        let painter = &mut self.painter;
        shown.paint(gpu, |view, w, h| painter.paint(gpu, out, view, w, h));
        start
    }
}

/// Draws the launcher offscreen at `size` and saves it as a PNG, without a window.
pub fn shot(gpu: &Gpu, form: &mut Form, size: (u32, u32), path: &Path) -> std::io::Result<()> {
    let (w, h) = size;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("launcher shot"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut painter = Painter::new(gpu, format);
    // The screen in points, as egui-winit gives it in a window.
    let zoom = ZOOM;
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(w as f32, h as f32) / zoom,
        )),
        ..Default::default()
    };
    // egui lays a new form out over a couple of frames; the textures each frame adds are kept.
    for _ in 0..2 {
        let (_, out) = painter.run(form, input());
        painter.paint(gpu, out, &view, w, h);
    }
    let (_, out) = painter.run(form, input());
    painter.paint(gpu, out, &view, w, h);
    let row = (4 * w).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("launcher shot"),
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let bytes = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    let rgba: Vec<u8> = bytes
        .chunks(row as usize)
        .flat_map(|r| &r[..4 * w as usize])
        .copied()
        .collect();
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, form: &mut Form, events: Vec<egui::Event>) -> bool {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 960.0),
            )),
            events,
            ..Default::default()
        };
        let mut start = false;
        let _ = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default_margins().show(ui, |ui| start |= form.show(ui));
        });
        start
    }

    fn enter() -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]
    }

    #[test]
    fn enter_starts_a_match_of_two_or_more() {
        let ctx = egui::Context::default();
        let mut form = Form::new(4, 8, false, Aspect::GAME);
        assert!(!frame(&ctx, &mut form, vec![]));
        assert!(frame(&ctx, &mut form, enter()));
        form.choice.players = [None, roster::character("marth").map(|c| c.kind), None, None];
        assert!(!frame(&ctx, &mut form, enter()), "one player can't start");
    }

    /// The stage list offers only stages whose audit found nothing missing, and a stage whose
    /// gap one character reaches refuses the match with that character.
    #[test]
    fn only_complete_stages_start() {
        let ctx = egui::Context::default();
        let mut form = Form::new(4, 8, false, Aspect::GAME);
        assert!(roster::offered_stages().all(|s| s.missing_with(&[]).is_empty()));
        assert!(roster::offered_stages().any(|s| s.keys[0] == "fd"));
        let ics = roster::character("ics").unwrap().kind;
        form.choice.stage = roster::stage("ht").unwrap().kind;
        assert!(form.choice.refusal().is_none());
        form.choice.players[1] = Some(ics);
        let why = form
            .choice
            .refusal()
            .expect("Hyrule Temple with the Ice Climbers");
        assert!(why.contains("Ice Climbers"), "{why}");
        assert!(!frame(&ctx, &mut form, enter()), "refused");
        // A stage the list doesn't offer, kept from an older settings file, isn't taken back.
        let unoffered = roster::STAGES.iter().find(|s| !s.offered());
        if let Some(s) = unoffered {
            form.choice.players[1] = form.choice.players[0];
            form.restore(&format!("stage = {}\n", s.keys[0]), &Given::default());
            assert_eq!(form.choice.stage, roster::stage("ht").unwrap().kind);
            form.choice.stage = s.kind;
            assert!(form.choice.refusal().is_some(), "{}", s.name);
        }
        // The CPU audit's Nana: the Ice Climbers with Peach, whose pulled Beam Sword Nana can
        // pick up, items off and Popo human.
        form.choice.stage = roster::stage("fd").unwrap().kind;
        form.choice.players = [
            Some(ics),
            roster::character("peach").map(|c| c.kind),
            None,
            None,
        ];
        let why = form.choice.refusal().expect("Nana and Peach's sword");
        assert!(why.contains("Nana"), "{why}");
        assert!(!frame(&ctx, &mut form, enter()), "refused");
    }

    /// The launcher's gate, held to the audits as the item switch's is
    /// (`items::offered_items_reach_nothing_unported`): every match the form would start, built
    /// as the player builds it (`cpu::players`, `Settings::vs_items`), reaches nothing the stage,
    /// items or CPU audits name, read back from its start data (the computer players' slots and
    /// levels, the characters, the stage, the item kinds on). Every offered stage, every pair of
    /// characters (a gap is reached through one computer player and one other character at
    /// most, so larger sets reach only what their pairs do), human or CPU at every level, items
    /// off and on.
    #[test]
    fn the_launcher_starts_nothing_the_audits_reject() {
        use melee_sim::game::cpu_audit::{self, Computer};
        use melee_sim::game::items_audit::{self, RANDOM_KINDS};
        use melee_sim::game::vs_match::Settings;
        let off = items::Switch::default();
        let on = items::Switch {
            freq: 3,
            ..items::Switch::default()
        };
        let mut controls: Vec<[Option<u8>; 2]> = vec![[None, None]];
        for l in cpu::LEVELS {
            controls.extend([[None, Some(l)], [Some(l), None], [Some(l), Some(10 - l)]]);
        }
        let few = [
            [None, None],
            [None, Some(1)],
            [Some(9), None],
            [Some(5), Some(5)],
        ];
        // The port lists hold only the offered characters (`only_complete_characters_start`).
        let offered: Vec<_> = roster::offered_characters().collect();
        let (mut started, mut with_cpus, mut refused) = (0usize, 0usize, 0usize);
        for (items, controls) in [(off, &controls[..]), (on, &few[..])] {
            for s in roster::offered_stages() {
                for a in &offered {
                    for b in &offered {
                        let base = Choice {
                            players: [Some(a.kind), Some(b.kind), None, None],
                            cpus: [None; 4],
                            stage: s.kind,
                            stocks: 4,
                            minutes: 8,
                            items,
                        };
                        let m = base.items();
                        let stage_refusal = s.refusal(&base.characters());
                        for ctl in controls {
                            let c = Choice {
                                cpus: [ctl[0], ctl[1], None, None],
                                ..base.clone()
                            };
                            let why = stage_refusal.clone().or_else(|| c.cpu_refusal(&m.kinds));
                            if items == off || ctl == &few[1] {
                                assert_eq!(why, c.refusal(), "{c:?}");
                            }
                            if why.is_some() {
                                refused += 1;
                                continue;
                            }
                            let players = cpu::players(&c.ports(), &c.cpus);
                            let settings =
                                Settings::vs_items(c.stage, &players, 4, 8, &m.prefs).start;
                            let slots = &settings.players[..4];
                            let cpus: Vec<Computer> = (slots.iter())
                                .filter(|p| p.slot_type == cpu::COMPUTER)
                                .map(|p| Computer {
                                    character: p.ckind as u8,
                                    level: p.cpu_level,
                                })
                                .collect();
                            let chars: Vec<u8> = (slots.iter())
                                .filter(|p| p.slot_type != 3)
                                .map(|p| p.ckind as u8)
                                .collect();
                            let r = &settings.rules;
                            let kinds: Vec<i32> = match r.item_freq {
                                -1 => Vec::new(),
                                _ => (0..RANDOM_KINDS).filter(|k| r.x20 >> k & 1 != 0).collect(),
                            };
                            let stage = r.stkind;
                            assert_eq!(cpus.len(), ctl.iter().flatten().count(), "{c:?}");
                            let gaps = cpu_audit::missing_with(&cpus, &chars, stage, &kinds);
                            assert!(gaps.is_empty(), "{c:?} reaches {gaps:?}");
                            assert!(items_audit::missing_with(&kinds, &chars, stage).is_empty());
                            let st = roster::stage_by_kind(stage).unwrap();
                            assert!(st.offered() && st.missing_with(&chars).is_empty());
                            started += 1;
                            with_cpus += usize::from(!cpus.is_empty());
                        }
                    }
                }
            }
        }
        eprintln!(
            "the launcher starts {started} of {} choices, {with_cpus} with CPU players",
            started + refused
        );
        assert!(started > 0);
    }

    /// The port lists offer only characters whose own items the fighter audit finds complete,
    /// and a match refuses a character it doesn't (`roster::characters_refusal`). Every roster
    /// character is offered now, so each starts a match on Final Destination against Marth.
    #[test]
    fn only_complete_characters_start() {
        let mut form = Form::new(4, 8, false, Aspect::GAME);
        form.choice.stage = roster::stage("fd").unwrap().kind;
        let marth = roster::character("marth").unwrap().kind;
        for c in roster::CHARACTERS {
            assert!(c.offered(), "{}", c.name);
            form.choice.players = [Some(c.kind), Some(marth), None, None];
            assert_eq!(form.choice.refusal(), None, "{}", c.name);
        }
        assert_eq!(
            roster::offered_characters().count(),
            roster::CHARACTERS.len()
        );
    }

    #[test]
    fn a_repeated_character_takes_the_next_costume() {
        let fox = roster::character("fox").unwrap().kind;
        let marth = roster::character("marth").unwrap().kind;
        let c = Choice {
            players: [Some(fox), None, Some(marth), Some(fox)],
            cpus: [None; 4],
            stage: 32,
            stocks: 4,
            minutes: 8,
            items: items::Switch::default(),
        };
        assert_eq!(c.player_count(), 3);
        assert_eq!(c.ports(), vec![(fox, 0), (0xFF, 0), (marth, 0), (fox, 1)]);
    }

    fn picked() -> Form {
        let mut form = Form::new(3, 0, true, Aspect::wide(16.0 / 9.0));
        form.choice.players = [
            None,
            roster::character("marth").map(|c| c.kind),
            None,
            roster::character("fox").map(|c| c.kind),
        ];
        form.choice.stage = roster::offered_stages().last().unwrap().kind;
        form.present = Present::Immediate;
        form.added_delay = 2;
        form.high_refresh = Mode::Interpolate;
        form.tick_timing = Timing::Vrr;
        form.jit_margin = 2.5;
        form.scale = 3;
        form.msaa = true;
        form.record = false;
        form.replays = "D:/Slippi/melee-hd".to_owned();
        form.choice.items.freq = 3;
        form.choice.items.set(0x06, false);
        form.choice.items.set(0x1C, false);
        form.choice.cpus = [Some(9), Some(7), None, Some(1)];
        form
    }

    fn same(a: &Form, b: &Form) -> bool {
        (
            a.choice.clone(),
            a.aspect,
            a.fullscreen,
            a.present,
            a.added_delay,
            a.high_refresh,
            a.tick_timing,
            a.jit_margin,
            a.scale,
            a.msaa,
            a.record,
            a.replays.clone(),
        ) == (
            b.choice.clone(),
            b.aspect,
            b.fullscreen,
            b.present,
            b.added_delay,
            b.high_refresh,
            b.tick_timing,
            b.jit_margin,
            b.scale,
            b.msaa,
            b.record,
            b.replays.clone(),
        )
    }

    #[test]
    fn the_last_match_comes_back() {
        for aspect in Aspect::CHOICES.map(|(a, _)| a) {
            for (mode, timing) in Mode::CHOICES
                .map(|(m, _)| m)
                .into_iter()
                .zip(Timing::CHOICES.map(|(t, _)| t).into_iter().cycle())
            {
                let mut was = picked();
                was.aspect = aspect;
                was.high_refresh = mode;
                was.tick_timing = timing;
                let mut form = Form::new(4, 8, false, Aspect::GAME);
                form.restore(&was.save(), &Given::default());
                assert!(same(&form, &was), "{}", was.save());
            }
        }
        // The window's scale and no MSAA come back too.
        let mut was = picked();
        (was.scale, was.msaa) = (0, false);
        let mut form = picked();
        form.restore(&was.save(), &Given::default());
        assert!(same(&form, &was), "{}", was.save());
    }

    #[test]
    fn the_command_line_wins_over_the_last_match() {
        let mut form = Form::new(5, 7, false, Aspect::STRETCHED);
        form.scale = 2;
        let given = Given {
            stocks: true,
            aspect: true,
            scale: true,
            ..Given::default()
        };
        form.restore(&picked().save(), &given);
        assert_eq!((form.choice.stocks, form.choice.minutes), (5, 0));
        assert_eq!(form.aspect, Aspect::STRETCHED);
        assert_eq!((form.scale, form.msaa), (2, true));
        assert_eq!(form.choice.players, picked().choice.players);
    }

    #[test]
    fn an_unreadable_last_match_changes_nothing() {
        let mut form = Form::new(4, 8, false, Aspect::GAME);
        let text = "p1 = nobody\nstage = nowhere\nstocks = 300\naspect = 1:2\n\u{0}garbage\nfullscreen = maybe\npresent\nadded-delay = -1\nhigh-refresh = sometimes\ntick-timing = soon\njit-margin = 99\nscale = 9\nmsaa = 2\nitems = lots\nitem-switch-off = everything\np2-cpu = 10\np3-cpu = human\n";
        form.restore(text, &Given::default());
        assert!(same(&form, &Form::new(4, 8, false, Aspect::GAME)));
    }
}
