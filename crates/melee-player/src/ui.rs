//! egui on the player's own GPU device and window: the settings panel ([`crate::front`]),
//! drawn over the paused game, and the overlays drawn over a frame (the latency overlay, the
//! menus' gate note).

use std::path::Path;
use std::time::{Duration, Instant};

use crate::front::{Action, Panel};
use crate::present::{Gpu, Shown};

/// How much larger than egui's own sizes the panel is drawn.
const ZOOM: f32 = 1.5;

/// egui on the player's GPU: a context and a renderer for one target format.
pub struct Painter {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    /// Clears the target first, or draws over it (the panel over the game, the overlays).
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

    /// Runs the panel over a frame's input, over the game dimmed: what was pressed, and what
    /// to paint.
    pub fn run(
        &mut self,
        panel: &mut Panel,
        input: egui::RawInput,
    ) -> (Option<Action>, egui::FullOutput) {
        let mut action = None;
        let out = self.ctx.run_ui(input, |ui| {
            let frame = egui::Frame::central_panel(ui.style()).fill(
                egui::Color32::from_black_alpha(if self.clear { 255 } else { 200 }),
            );
            egui::CentralPanel::default()
                .frame(frame)
                .show(ui, |ui| action = panel.show(ui));
        });
        (action, out)
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
        // The overlay's pass is timed with the frame it's drawn over; the panel's alone aren't.
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
                    label: Some("panel"),
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

/// The panel in the player's window: egui's painter and its window input.
pub struct Window {
    painter: Painter,
    state: egui_winit::State,
    /// When egui wants the next frame, if it asked for one later.
    pub repaint_at: Option<Instant>,
}

impl Window {
    pub fn new(gpu: &Gpu, shown: &Shown) -> Window {
        let mut painter = Painter::new(gpu, shown.format());
        painter.clear = false;
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

    /// Clears the window to black, as a console's screen is while it powers on.
    pub fn black(&mut self, gpu: &Gpu, shown: &mut Shown) {
        let out = self.painter.ctx.run_ui(Default::default(), |_| {});
        let painter = &mut self.painter;
        painter.clear = true;
        shown.paint(gpu, |view, w, h| painter.paint(gpu, out, view, w, h));
        painter.clear = false;
    }

    /// Draws the panel in the window over `under` (the last frame shown, else black); what
    /// was pressed.
    pub fn redraw(
        &mut self,
        gpu: &Gpu,
        shown: &mut Shown,
        panel: &mut Panel,
        under: Option<&melee_gpu::Frame>,
    ) -> Option<Action> {
        let input = self.state.take_egui_input(&shown.window);
        let (action, mut out) = self.painter.run(panel, input);
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
        match under {
            Some(frame) => {
                let _ = shown.present(gpu, frame, None, |view, w, h| {
                    painter.paint(gpu, out, view, w, h)
                });
            }
            None => {
                painter.clear = true;
                shown.paint(gpu, |view, w, h| painter.paint(gpu, out, view, w, h));
                painter.clear = false;
            }
        }
        action
    }
}

/// Draws the panel offscreen at `size` and saves it as a PNG, without a window.
pub fn shot(gpu: &Gpu, panel: &mut Panel, size: (u32, u32), path: &Path) -> std::io::Result<()> {
    let (w, h) = size;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("panel shot"),
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
    // egui lays a new panel out over a couple of frames; the textures each frame adds are kept.
    for _ in 0..2 {
        let (_, out) = painter.run(panel, input());
        painter.paint(gpu, out, &view, w, h);
    }
    let (_, out) = painter.run(panel, input());
    painter.paint(gpu, out, &view, w, h);
    let row = (4 * w).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("panel shot"),
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
