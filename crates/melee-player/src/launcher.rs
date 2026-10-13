//! The launcher: started with no match on its command line, the player's window shows a small
//! form (a character or none for each port, a stage, stocks and minutes, the aspect ratio and
//! fullscreen) drawn with egui on the
//! player's own GPU device and window, and Start plays that match in the same window. Escape in
//! the match comes back here; Escape here quits. A tool for manual testing, and the seed of the
//! menus to come.

use std::path::Path;
use std::time::{Duration, Instant};

use melee_sim::game::roster::{self, CHARACTERS, STAGES};

use crate::present::{Aspect, Gpu, Shown};

/// A match the launcher picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// Each port's `CharacterKind`, or none.
    pub players: [Option<u8>; 4],
    /// `StKind`.
    pub stage: u16,
    pub stocks: i8,
    pub minutes: u32,
}

impl Choice {
    pub fn player_count(&self) -> usize {
        self.players.iter().flatten().count()
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

/// The form and what it last had to say.
pub struct Form {
    pub choice: Choice,
    /// Why the last match didn't start or stopped.
    pub message: Option<String>,
    /// Whether the window is to be fullscreen (the player follows the box, and the box F11).
    pub fullscreen: bool,
    /// The aspect ratio the match is shown at.
    pub aspect: Aspect,
}

impl Form {
    /// Fox against Fox on Final Destination, as the command line's defaults are.
    pub fn new(stocks: i8, minutes: u32, fullscreen: bool, aspect: Aspect) -> Form {
        let fox = roster::character("fox").map(|c| c.kind);
        Form {
            choice: Choice {
                players: [fox, fox, None, None],
                stage: roster::stage("fd").map_or(STAGES[0].kind, |s| s.kind),
                stocks,
                minutes,
            },
            message: None,
            fullscreen,
            aspect,
        }
    }

    /// Lays the form out; true when Start is pressed (or Enter with nothing focused).
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut start = false;
        ui.vertical_centered(|ui| {
            ui.add_space(32.0);
            ui.heading("melee-hd");
            ui.add_space(16.0);
            let c = &mut self.choice;
            // The grid, centered: its width is about the labels' and the boxes'.
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 290.0) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    egui::Grid::new("match")
                        .num_columns(2)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            for (i, p) in c.players.iter_mut().enumerate() {
                                ui.label(format!("Port {}", i + 1));
                                let name = p
                                    .and_then(roster::character_by_kind)
                                    .map_or("None", |c| c.name);
                                egui::ComboBox::from_id_salt(("port", i))
                                    .selected_text(name)
                                    .width(200.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(p, None, "None");
                                        for ch in CHARACTERS {
                                            ui.selectable_value(p, Some(ch.kind), ch.name);
                                        }
                                    });
                                ui.end_row();
                            }
                            ui.label("Stage");
                            let name = roster::stage_by_kind(c.stage).map_or("?", |s| s.name);
                            egui::ComboBox::from_id_salt("stage")
                                .selected_text(name)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for s in STAGES {
                                        ui.selectable_value(&mut c.stage, s.kind, s.name);
                                    }
                                });
                            ui.end_row();
                            ui.label("Stocks");
                            ui.add(egui::DragValue::new(&mut c.stocks).range(0..=99));
                            ui.end_row();
                            ui.label("Minutes");
                            ui.add(egui::DragValue::new(&mut c.minutes).range(0..=99));
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
                        });
                });
            });
            ui.add_space(16.0);
            let ready = c.player_count() >= 2;
            let button = ui.add_enabled(
                ready,
                egui::Button::new("Start").min_size(egui::vec2(120.0, 32.0)),
            );
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter))
                && ui.memory(|m| m.focused().is_none())
                && !ui.ctx().any_popup_open();
            start = ready && (button.clicked() || enter);
            ui.add_space(8.0);
            if !ready {
                ui.label("Pick at least two players.");
            }
            if let Some(m) = &self.message {
                ui.colored_label(ui.visuals().error_fg_color, m);
            }
            ui.add_space(16.0);
            ui.weak("0 stocks: a timed match. 0 minutes: no time limit.");
            ui.weak("Enter starts. Escape in a match comes back here; Escape here quits.");
            ui.weak("F11 or Alt+Enter: fullscreen.");
        });
        start
    }
}

/// How much larger than egui's own sizes the form is drawn.
const ZOOM: f32 = 1.5;

/// egui on the player's GPU: a context and a renderer for one target format.
pub struct Painter {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
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
        Painter { ctx, renderer }
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
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("launcher"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
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

    #[test]
    fn a_repeated_character_takes_the_next_costume() {
        let fox = roster::character("fox").unwrap().kind;
        let marth = roster::character("marth").unwrap().kind;
        let c = Choice {
            players: [Some(fox), None, Some(marth), Some(fox)],
            stage: 32,
            stocks: 4,
            minutes: 8,
        };
        assert_eq!(c.player_count(), 3);
        assert_eq!(c.ports(), vec![(fox, 0), (0xFF, 0), (marth, 0), (fox, 1)]);
    }
}
