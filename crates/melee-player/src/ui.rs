//! egui on the player's own GPU device and window: the settings panel ([`crate::front`]),
//! drawn over the paused game, and the overlays drawn over a frame (the latency overlay, the
//! menus' gate note). The panel takes a controller's moves ([`crate::pad_nav`]) as egui's keys:
//! down and up as Tab and Shift+Tab, through every widget in order (egui's arrows skip some in
//! the panel's layout, and a number's or text's box keeps them), left and right as the arrows
//! (a slider's steps, the tabs' row; in a number's box, its steps), A as Enter, and B closing a
//! drop-down onto its button; the widget with the focus is ringed and scrolled into view.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::front::{Action, Panel};
use crate::pad_nav::Nav;
use crate::present::{Gpu, Shown};

/// How much larger than egui's own sizes the panel is drawn.
const ZOOM: f32 = 1.5;

/// egui on the player's GPU: a context and a renderer for one target format.
pub struct Painter {
    pub ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    /// Clears the target first, or draws over it (the panel over the game, the overlays).
    pub clear: bool,
    /// Rings the widget with the focus (the panel worked from a controller or the keys).
    pub focus_ring: bool,
}

/// egui's temporary data key for whether the focus is kept in view ([`keep_focus_shown`]).
fn follow_id() -> egui::Id {
    egui::Id::new("melee-hd focus follow")
}

/// Inside the panel's scroll area: scrolls the widget with the focus into view, while the panel
/// is worked from a controller or the keys.
pub fn keep_focus_shown(ui: &egui::Ui) {
    let ctx = ui.ctx();
    if !ctx.data(|d| d.get_temp::<bool>(follow_id()).unwrap_or(false)) {
        return;
    }
    if let Some(r) = ctx
        .memory(egui::Memory::focused)
        .and_then(|id| ctx.read_response(id))
    {
        r.scroll_to_me(None);
    }
}

/// The ring around the widget with the focus.
fn ring(ctx: &egui::Context) {
    let Some(r) = ctx
        .memory(egui::Memory::focused)
        .and_then(|id| ctx.read_response(id))
    else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("melee-hd focus ring"),
    ));
    painter.rect_stroke(
        r.rect.expand(3.0),
        4.0,
        egui::Stroke::new(2.0, FOCUS_RING),
        egui::StrokeKind::Outside,
    );
}

/// One frame of the panel in `ctx` over `input`, over the game dimmed (or over black, `clear`),
/// with the focus ringed and kept in view (`ring_it`): what was pressed, and what to paint.
fn run_panel(
    ctx: &egui::Context,
    panel: &mut Panel,
    input: egui::RawInput,
    clear: bool,
    ring_it: bool,
) -> (Option<Action>, egui::FullOutput) {
    let mut action = None;
    ctx.data_mut(|d| d.insert_temp(follow_id(), ring_it));
    let out = ctx.run_ui(input, |ui| {
        let frame = egui::Frame::central_panel(ui.style()).fill(egui::Color32::from_black_alpha(
            if clear { 255 } else { 200 },
        ));
        egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| action = panel.show(ui));
        if ring_it {
            ring(ui.ctx());
        }
    });
    (action, out)
}

/// The focus ring's color: Melee's menus' yellow.
pub const FOCUS_RING: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xD0, 0x40);

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
            focus_ring: false,
        }
    }

    /// Runs the panel over a frame's input, over the game dimmed: what was pressed, and what
    /// to paint.
    pub fn run(
        &mut self,
        panel: &mut Panel,
        input: egui::RawInput,
    ) -> (Option<Action>, egui::FullOutput) {
        run_panel(&self.ctx, panel, input, self.clear, self.focus_ring)
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
    /// A controller's moves, for the next frame.
    keys: PadKeys,
}

/// A controller's moves as egui's key events, and what they do to the drop-downs.
#[derive(Default)]
struct PadKeys {
    /// The events for the next frame.
    pending: Vec<egui::Event>,
    /// What A last pressed with no drop-down open: a drop-down's button, once it opens, which
    /// the focus goes back to as the drop-down closes.
    opener: Option<egui::Id>,
    /// After the next frame: the drop-down closes (B, or A on one of its entries).
    close_popup: bool,
}

impl PadKeys {
    /// The moves queued for `ctx`'s next frame; what the panel then does (B with no drop-down
    /// open, and Start, resume unless Melee stopped, `halted`).
    fn queue(&mut self, ctx: &egui::Context, navs: &[Nav], halted: bool) -> Option<Action> {
        let focused = ctx.memory(egui::Memory::focused);
        let popup = egui::Popup::is_any_open(ctx);
        // A number's box or a text box keeps the arrows to itself: there left and right step the
        // number (egui's up and down arrows).
        let editing = ctx.text_edit_focused();
        let mut action = None;
        let mut tabbed = false;
        for &n in navs {
            let (key, modifiers) = match n {
                // Up and down go through the widgets in their order (egui's Tab), so every one is
                // reached and none keeps the focus to itself.
                Nav::Up => (egui::Key::Tab, egui::Modifiers::SHIFT),
                Nav::Down => (egui::Key::Tab, egui::Modifiers::NONE),
                Nav::Left if editing => (egui::Key::ArrowDown, egui::Modifiers::NONE),
                Nav::Right if editing => (egui::Key::ArrowUp, egui::Modifiers::NONE),
                Nav::Left => (egui::Key::ArrowLeft, egui::Modifiers::NONE),
                Nav::Right => (egui::Key::ArrowRight, egui::Modifiers::NONE),
                Nav::Activate => {
                    if !popup {
                        self.opener = focused;
                    } else if focused != self.opener {
                        // An entry: picked, then the drop-down closes.
                        self.close_popup = true;
                    }
                    (egui::Key::Enter, egui::Modifiers::NONE)
                }
                Nav::Back if popup => {
                    self.close_popup = true;
                    continue;
                }
                Nav::Back | Nav::Close => {
                    if !halted {
                        action = Some(Action::Resume);
                    }
                    continue;
                }
            };
            // With nothing focused, a direction focuses the first widget (egui moves the focus by
            // the arrows only from a widget that has it).
            let (key, modifiers) = if focused.is_none() && key != egui::Key::Enter {
                if tabbed {
                    continue;
                }
                tabbed = true;
                (egui::Key::Tab, egui::Modifiers::NONE)
            } else {
                (key, modifiers)
            };
            for pressed in [true, false] {
                self.pending.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
        }
        action
    }

    /// Whether the next frame has moves to take.
    fn any(&self) -> bool {
        !self.pending.is_empty() || self.close_popup
    }

    /// After a frame with a controller's B, or A on a drop-down's entry: the drop-down closes and
    /// its button has the focus again.
    fn after(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.close_popup) {
            egui::Popup::close_all(ctx);
            if let Some(id) = self.opener {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
    }
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
            keys: PadKeys::default(),
        }
    }

    /// Hands a window event to egui, asking for a frame if egui wants one. The keys that move
    /// the focus ring it; the mouse's buttons and wheel take the ring away.
    pub fn event(&mut self, shown: &Shown, event: &winit::event::WindowEvent) {
        use winit::event::WindowEvent as E;
        use winit::keyboard::{KeyCode, PhysicalKey};
        match event {
            E::KeyboardInput { event: k, .. }
                if matches!(
                    k.physical_key,
                    PhysicalKey::Code(
                        KeyCode::Tab
                            | KeyCode::ArrowUp
                            | KeyCode::ArrowDown
                            | KeyCode::ArrowLeft
                            | KeyCode::ArrowRight
                    )
                ) =>
            {
                self.painter.focus_ring = true;
            }
            E::MouseInput { .. } | E::MouseWheel { .. } => self.painter.focus_ring = false,
            _ => {}
        }
        if self.state.on_window_event(&shown.window, event).repaint {
            shown.window.request_redraw();
        }
    }

    /// Whether a drop-down is open (Escape closes it rather than quitting).
    pub fn popup_open(&self) -> bool {
        self.painter.ctx.any_popup_open()
    }

    /// A controller's moves in the panel, for its next frame; what the panel then does (B with
    /// no drop-down open, and Start, resume unless Melee stopped, `halted`, which only a
    /// restart leaves).
    pub fn pad(&mut self, shown: &Shown, navs: &[Nav], halted: bool) -> Option<Action> {
        if navs.is_empty() {
            return None;
        }
        let action = self.keys.queue(&self.painter.ctx, navs, halted);
        self.painter.focus_ring = true;
        shown.window.request_redraw();
        action
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
        let mut input = self.state.take_egui_input(&shown.window);
        let padded = self.keys.any();
        input.events.append(&mut self.keys.pending);
        let (action, mut out) = self.painter.run(panel, input);
        self.keys.after(&self.painter.ctx);
        if padded {
            // The frame after a move scrolls the newly focused widget into view.
            shown.window.request_redraw();
        }
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
    // The Recordings tab's rows are read meanwhile, for up to a few seconds.
    let until = Instant::now() + Duration::from_secs(5);
    for i in 0.. {
        let (_, out) = painter.run(panel, input());
        painter.paint(gpu, out, &view, w, h);
        if i >= 1 && (!panel.recordings.reading() || Instant::now() > until) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::{Settings, Tab};
    use egui::accesskit;
    use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

    /// The panel in an egui context of its own, without a GPU, worked from a controller as the
    /// window works it.
    struct Harness {
        ctx: egui::Context,
        panel: Panel,
        keys: PadKeys,
        /// The last frame's widgets as accesskit gives them (all of them while accesskit is on).
        nodes: BTreeMap<u64, accesskit::Node>,
    }

    impl Harness {
        fn new(tab: Tab) -> Harness {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(ZOOM);
            ctx.enable_accesskit();
            let mut settings = Settings::default();
            // Empty folders of its own for the Recordings tab and the PhobGCC profiles, rather
            // than the player's.
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("melee-hd-pad-ui-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            settings.recordings = dir.display().to_string();
            let mut panel = Panel::new(settings, "Melee.iso".into());
            panel.tab = tab;
            panel.phob = crate::phob_ui::PhobPanel::load(dir.join("phob"));
            // The Controllers tab's devices: the keyboard and a controller on the adapter's
            // first port (its PhobGCC section), as `Inputs::look` gives them.
            panel.controllers.devices = vec![
                melee_input::DeviceInfo {
                    id: melee_input::DeviceId::Keyboard,
                    name: "Keyboard".into(),
                    connected: true,
                    port: Some(1),
                },
                melee_input::DeviceInfo {
                    id: melee_input::DeviceId::Adapter(0),
                    name: "GameCube adapter, port 1".into(),
                    connected: true,
                    port: Some(0),
                },
            ];
            let mut h = Harness {
                ctx,
                panel,
                keys: PadKeys::default(),
                nodes: BTreeMap::new(),
            };
            for _ in 0..3 {
                h.frame();
            }
            h
        }

        fn frame(&mut self) -> Option<Action> {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0) / ZOOM,
                )),
                ..Default::default()
            };
            input.events.append(&mut self.keys.pending);
            let (action, out) = run_panel(&self.ctx, &mut self.panel, input, true, true);
            self.keys.after(&self.ctx);
            if let Some(u) = out.platform_output.accesskit_update {
                self.nodes = u.nodes.into_iter().map(|(id, n)| (id.0, n)).collect();
            }
            action
        }

        /// The focusable widgets the last frame laid out: accesskit's nodes with a focus
        /// action, not disabled.
        fn focusable(&self) -> BTreeSet<u64> {
            (self.nodes.iter())
                .filter(|(_, n)| n.supports_action(accesskit::Action::Focus) && !n.is_disabled())
                .map(|(id, _)| *id)
                .collect()
        }

        /// What a widget is, for a failure's message.
        fn describe(&self, id: u64) -> String {
            match self.nodes.get(&id) {
                Some(n) => format!(
                    "{:?} {:?}",
                    n.role(),
                    n.label().or(n.value()).unwrap_or_default()
                ),
                None => format!("{id:x}"),
            }
        }

        /// The controller's moves, then two frames (the second scrolls the focus into view).
        fn press(&mut self, navs: &[Nav]) -> Option<Action> {
            let mut action = self.keys.queue(&self.ctx, navs, false);
            for _ in 0..2 {
                action = self.frame().or(action);
            }
            action
        }

        fn focused(&self) -> Option<egui::Id> {
            self.ctx.memory(egui::Memory::focused)
        }

        fn focus(&mut self, id: egui::Id) {
            self.ctx.memory_mut(|m| m.request_focus(id));
            self.frame();
        }

        /// Whether the focus is on a widget of `role` labelled or valued `text`.
        fn on(&self, role: accesskit::Role, text: &str) -> bool {
            self.focused()
                .and_then(|id| self.nodes.get(&id.accesskit_id().0))
                .is_some_and(|n| {
                    n.role() == role && (n.value() == Some(text) || n.label() == Some(text))
                })
        }

        /// Down until the focus is on a widget of `role` labelled or valued `text`.
        fn down_to(&mut self, role: accesskit::Role, text: &str) {
            for _ in 0..60 {
                if self.on(role, text) {
                    return;
                }
                self.press(&[Nav::Down]);
            }
            panic!("no {role:?} {text:?} reached going down");
        }

        /// Down until the focus is on a widget of `role`.
        fn down_to_role(&mut self, role: accesskit::Role) {
            for _ in 0..60 {
                let n = self
                    .focused()
                    .and_then(|id| self.nodes.get(&id.accesskit_id().0));
                if n.is_some_and(|n| n.role() == role) {
                    return;
                }
                self.press(&[Nav::Down]);
            }
            panic!("no {role:?} reached going down");
        }
    }

    /// From nothing focused, the controller's directions reach every widget that can take the
    /// focus, on each tab: the D-pad and stick alone get anywhere in the panel.
    #[test]
    fn every_widget_is_reached_by_the_controller() {
        for (tab, name) in Tab::ALL {
            let mut h = Harness::new(tab);
            h.press(&[Nav::Down]);
            if tab == Tab::Controllers {
                // The adapter port's PhobGCC section shows its settings once it's on: turned
                // on from the controller, then the walk starts again from the first widget.
                h.down_to(accesskit::Role::CheckBox, "Process this port with PhobGCC");
                h.press(&[Nav::Activate]);
                assert!(
                    h.nodes
                        .values()
                        .any(|n| n.role() == accesskit::Role::Slider),
                    "the PhobGCC section's settings show"
                );
                let on = h.focused().expect("the checkbox focused");
                h.ctx.memory_mut(|m| m.surrender_focus(on));
                h.frame();
                h.press(&[Nav::Down]);
            }
            let first = h.focused().expect("a direction focuses the first widget");
            let mut seen = HashSet::from([first]);
            let mut queue = VecDeque::from([first]);
            while let Some(id) = queue.pop_front() {
                assert!(
                    seen.len() <= 4 * h.focusable().len() + 50,
                    "{name}: the walk keeps finding new widgets: {:?}",
                    (seen.iter())
                        .map(|id| h.describe(id.accesskit_id().0))
                        .collect::<Vec<_>>()
                );
                for d in [Nav::Up, Nav::Down, Nav::Left, Nav::Right] {
                    h.focus(id);
                    h.press(&[d]);
                    if let Some(to) = h.focused()
                        && seen.insert(to)
                    {
                        queue.push_back(to);
                    }
                }
            }
            assert_eq!(h.panel.tab, tab, "moving the focus never changes the tab");
            let reached: BTreeSet<u64> = seen.iter().map(|id| id.accesskit_id().0).collect();
            let focusable = h.focusable();
            let missed: Vec<String> = (focusable.iter())
                .filter(|id| !reached.contains(id))
                .map(|&id| h.describe(id))
                .collect();
            assert!(
                focusable.len() > 3,
                "{name}: only {} focusable widgets",
                focusable.len()
            );
            assert!(
                missed.is_empty(),
                "{name}: {} of {} widgets never reached: {missed:?}",
                missed.len(),
                focusable.len()
            );
            // Down alone walks them all, in order, back round to the first.
            h.focus(first);
            let mut walked = BTreeSet::from([first.accesskit_id().0]);
            for _ in 0..2 * focusable.len() {
                h.press(&[Nav::Down]);
                match h.focused() {
                    Some(id) if id == first => break,
                    Some(id) => walked.insert(id.accesskit_id().0),
                    None => panic!("{name}: down lost the focus"),
                };
            }
            assert_eq!(h.focused(), Some(first), "{name}: down comes back round");
            assert!(
                focusable.is_subset(&walked),
                "{name}: down alone missed {:?}",
                (focusable.difference(&walked))
                    .map(|&id| h.describe(id))
                    .collect::<Vec<_>>()
            );
            eprintln!("{name}: all {} widgets reached", focusable.len());
        }
    }

    /// A on a drop-down opens it, a direction goes into it, A on an entry picks it, the
    /// drop-down closes and its button has the focus again; B closes it without picking.
    #[test]
    fn a_drop_down_works_from_a_controller() {
        let mut h = Harness::new(Tab::Settings);
        assert!(h.panel.settings.fast_renderer);
        h.press(&[Nav::Down]);
        h.down_to(accesskit::Role::ComboBox, "Fast");
        let button = h.focused();
        h.press(&[Nav::Activate]);
        assert!(egui::Popup::is_any_open(&h.ctx), "A opens the drop-down");
        assert_eq!(h.focused(), button);
        // Its entries, Fast then Exact, come next going down.
        h.down_to(accesskit::Role::Button, "Exact");
        assert!(egui::Popup::is_any_open(&h.ctx));
        h.press(&[Nav::Activate]);
        assert!(!h.panel.settings.fast_renderer, "A on an entry picks it");
        assert!(
            !egui::Popup::is_any_open(&h.ctx),
            "picking closes the drop-down"
        );
        assert_eq!(h.focused(), button, "the focus is back on its button");
        // B closes it without picking.
        h.press(&[Nav::Activate]);
        assert!(egui::Popup::is_any_open(&h.ctx));
        h.press(&[Nav::Down]);
        assert_eq!(
            h.press(&[Nav::Back]),
            None,
            "B in a drop-down doesn't resume"
        );
        assert!(!egui::Popup::is_any_open(&h.ctx));
        assert_eq!(h.focused(), button);
        assert!(!h.panel.settings.fast_renderer);
    }

    /// Left and right step a slider, and a number's box, which keeps the focus while up and
    /// down still leave it.
    #[test]
    fn left_and_right_step_sliders_and_numbers() {
        let mut h = Harness::new(Tab::Settings);
        h.press(&[Nav::Down]);
        h.down_to_role(accesskit::Role::SpinButton);
        // The first number is Added delay.
        let delay = h.panel.settings.added_delay;
        let boxed = h.focused();
        h.press(&[Nav::Right]);
        h.press(&[Nav::Right]);
        h.press(&[Nav::Left]);
        assert_eq!(h.panel.settings.added_delay, delay + 1);
        assert_eq!(h.focused(), boxed);
        h.press(&[Nav::Down]);
        assert_ne!(h.focused(), boxed, "down leaves a number's box");
        h.press(&[Nav::Up]);
        assert_eq!(h.focused(), boxed, "up comes back to it");
        assert_eq!(
            h.panel.settings.added_delay,
            delay + 1,
            "and leaving it keeps the value"
        );
        // The Volume slider.
        h.down_to_role(accesskit::Role::Slider);
        let volume = h.panel.settings.volume;
        h.press(&[Nav::Left]);
        h.press(&[Nav::Left]);
        assert!(h.panel.settings.volume < volume);
        // Its box, next going down.
        h.press(&[Nav::Down]);
        assert!(!h.on(accesskit::Role::Slider, ""), "down leaves the slider");
        let volume = h.panel.settings.volume;
        h.press(&[Nav::Right]);
        assert_eq!(h.panel.settings.volume, volume + 1, "right steps its box");
    }

    /// B and Start resume Melee, unless it stopped; A on a tab's name shows that tab.
    #[test]
    fn b_and_start_close_the_panel_unless_melee_stopped() {
        let mut h = Harness::new(Tab::Settings);
        assert_eq!(h.press(&[Nav::Back]), Some(Action::Resume));
        assert_eq!(h.press(&[Nav::Close]), Some(Action::Resume));
        let mut k = PadKeys::default();
        assert_eq!(k.queue(&h.ctx, &[Nav::Back, Nav::Close], true), None);
        h.press(&[Nav::Down]);
        assert!(h.focused().is_some());
        h.press(&[Nav::Right]);
        h.press(&[Nav::Activate]);
        assert_eq!(h.panel.tab, Tab::Controllers, "A on a tab's name shows it");
    }
}
