//! Presenting frames: the GPU the renderer draws on, a window's surface, and the blit of the
//! XFB (the frame the renderer copied out) to it, letterboxed to the aspect ratio shown (the
//! game's 73:60 unless asked otherwise, see [`Aspect`]). The swap chain is kept one frame
//! deep, presenting each frame as soon as it's drawn: mailbox where the display has it, else
//! immediate, else vsync.

use std::sync::{Arc, Mutex};

use melee_render::frame::GAME_ASPECT;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window};

static SIZE: Mutex<(u32, u32)> = Mutex::new((1280, 960));
/// The aspect ratio frames are shown at: the game's 73:60, wider in widescreen, or 4:3.
static ASPECT: Mutex<f32> = Mutex::new(GAME_ASPECT);

/// The largest EFB side the renderer makes (wgpu's default limit on a texture's side).
const MAX_EFB_SIDE: u32 = 8192;

/// How frames are shown: the aspect ratio, and whether the drawn cameras widen to it.
///
/// The game's 640 by 480 picture is made for 73:60 ([`GAME_ASPECT`]), and that's how it's shown
/// by default, pillarboxed, with the proportions the game's cameras give it. Wider aspects
/// widen each full-screen camera to the shown aspect (`melee_render::frame::widen`), so the
/// frame keeps those proportions and sees more at the sides. 4:3 shows the 73:60 picture
/// stretched to 4:3, as the player did before and as Dolphin's "Force 4:3" does.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Aspect {
    /// Width over height of the frame shown.
    pub shown: f32,
    /// The drawn cameras widen to `shown`; otherwise the 73:60 picture is shown at it.
    pub widen: bool,
}

impl Aspect {
    /// The game's own 73:60.
    pub const GAME: Aspect = Aspect {
        shown: GAME_ASPECT,
        widen: false,
    };
    /// The 73:60 picture stretched to 4:3.
    pub const STRETCHED: Aspect = Aspect {
        shown: 4.0 / 3.0,
        widen: false,
    };

    /// The cameras widened to `shown`.
    pub const fn wide(shown: f32) -> Aspect {
        Aspect { shown, widen: true }
    }

    /// The launcher's choices, with their names.
    pub const CHOICES: [(Aspect, &str); 5] = [
        (Aspect::GAME, "73:60 (the game's)"),
        (Aspect::wide(16.0 / 9.0), "16:9 (widescreen)"),
        (Aspect::wide(16.0 / 10.0), "16:10 (widescreen)"),
        (Aspect::wide(21.0 / 9.0), "21:9 (widescreen)"),
        (Aspect::STRETCHED, "4:3 (stretched)"),
    ];

    /// `--aspect W:H`: 4:3 is the stretched picture, 73:60 the game's, anything wider widens
    /// the cameras to it.
    pub fn parse(v: &str) -> Result<Aspect, String> {
        let bad = || format!("--aspect W:H (73:60, 4:3 or wider), not {v}");
        let (w, h) = v.split_once(':').ok_or_else(bad)?;
        let w: f32 = w.parse().map_err(|_| bad())?;
        let h: f32 = h.parse().map_err(|_| bad())?;
        let r = w / h;
        if !(r.is_finite() && r > 0.0) {
            return Err(bad());
        }
        Ok(if (r - 4.0 / 3.0).abs() < 0.001 {
            Aspect::STRETCHED
        } else if (r - GAME_ASPECT).abs() < 0.001 {
            Aspect::GAME
        } else if r > GAME_ASPECT {
            Aspect::wide(r)
        } else {
            return Err(format!("--aspect {v} is narrower than the game's 73:60"));
        })
    }

    /// What the renderer's `Hsd::widescreen` takes: the aspect the cameras widen to.
    pub fn widescreen(self) -> Option<f32> {
        self.widen.then_some(self.shown)
    }

    /// The name the launcher shows.
    pub fn name(self) -> String {
        Aspect::CHOICES
            .iter()
            .find(|(a, _)| *a == self)
            .map_or_else(|| format!("{:.3}:1", self.shown), |(_, n)| (*n).to_owned())
    }

    /// The EFB's width at `scale` (480 times `scale` lines for the picture): as many pixels
    /// as the frame is shown wide, so they're square, and even, for the XFB's pairs.
    pub fn efb_width(self, scale: u32) -> u32 {
        let w = (480.0 * scale as f32 * self.shown / 2.0).round() as u32 * 2;
        w.clamp(2, MAX_EFB_SIDE)
    }

    /// The largest scale whose EFB fits the renderer's limit (8 at most).
    pub fn max_scale(self) -> u32 {
        (1..=8u32)
            .rev()
            .find(|&s| (480.0 * s as f32 * self.shown).round() as u32 <= MAX_EFB_SIDE)
            .unwrap_or(1)
    }
}

/// The aspect ratio to show frames at.
pub fn set_aspect(aspect: f32) {
    *ASPECT.lock().unwrap() = aspect;
}

/// The window's first size.
pub fn set_size(size: (u32, u32)) {
    *SIZE.lock().unwrap() = size;
}

/// The adapter's name and backend, for messages.
pub fn describe(adapter: &wgpu::Adapter) -> String {
    let info = adapter.get_info();
    format!("{}, {:?}", info.name, info.backend)
}

/// The GPU: the renderer and the window share its device.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Gpu {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("a GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("melee-hd"),
            required_features: wgpu::Features::DUAL_SOURCE_BLENDING
                | (adapter.features() & wgpu::Features::PIPELINE_CACHE),
            ..Default::default()
        }))
        .expect("a GPU device");
        let gpu = describe(&adapter);
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("the GPU ({gpu}) was lost: {reason:?}: {message}");
        });
        Gpu {
            instance,
            adapter,
            device,
            queue,
        }
    }

    /// A renderer on this GPU, leaving its frames on the GPU, with `samples` samples a pixel
    /// (1, or 4 for MSAA), its EFB `scale` times the console's tall and as wide as `aspect`
    /// shows it with square pixels.
    pub fn renderer(&self, scale: u32, samples: u32, aspect: Aspect) -> melee_gpu::Renderer {
        let mut r = melee_gpu::Renderer::with_device_efb(
            self.device.clone(),
            self.queue.clone(),
            scale,
            aspect.efb_width(scale),
            samples,
        );
        r.xfb_on_gpu = true;
        r.xfb_readback = false;
        r
    }
}

const BLIT: &str = r#"
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return textureSample(frame, smp, i.uv);
}
"#;

/// The blit's pipeline and sampler for one target format.
pub struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Blit {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Blit {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Blit {
            pipeline,
            layout,
            sampler,
        }
    }

    /// Draws `frame` into `view` (`w` by `h`), letterboxed to the aspect ratio shown.
    fn draw(&self, gpu: &Gpu, frame: &wgpu::Texture, view: &wgpu::TextureView, w: u32, h: u32) {
        let device = &gpu.device;
        let fview = frame.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fview),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
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
            });
            let (sw, sh) = (w as f32, h as f32);
            let aspect = *ASPECT.lock().unwrap();
            let scale = (sw / aspect).min(sh);
            let (vw, vh) = (aspect * scale, scale);
            pass.set_viewport((sw - vw) / 2.0, (sh - vh) / 2.0, vw, vh, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
    }
}

/// The window and its swap chain.
pub struct Shown {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit: Blit,
    fullscreen: bool,
}

impl Shown {
    /// The window, borderless fullscreen on its monitor when `fullscreen`.
    pub fn new(gpu: &Gpu, event_loop: &ActiveEventLoop, fullscreen: bool) -> Shown {
        let (w, h) = *SIZE.lock().unwrap();
        let attrs = Window::default_attributes()
            .with_title("melee-hd")
            .with_inner_size(winit::dpi::PhysicalSize::new(w, h))
            .with_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attrs).expect("a window"));
        let surface = gpu
            .instance
            .create_surface(window.clone())
            .expect("a surface");
        let size = window.inner_size();
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
            .into_iter()
            .find(|m| caps.present_modes.contains(m))
            .unwrap_or(wgpu::PresentMode::AutoVsync);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let blit = Blit::new(gpu, format);
        eprintln!(
            "window: {}x{}, {present_mode:?}, {}",
            config.width,
            config.height,
            describe(&gpu.adapter)
        );
        Shown {
            window,
            surface,
            config,
            blit,
            fullscreen,
        }
    }

    pub fn resize(&mut self, gpu: &Gpu, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&gpu.device, &self.config);
    }

    /// The swap chain's format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Whether the window is fullscreen.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Makes the window borderless fullscreen on the monitor it's on, or a window again. The
    /// surface follows the resize the window then gets.
    pub fn set_fullscreen(&mut self, on: bool) {
        if on != self.fullscreen {
            self.fullscreen = on;
            self.window
                .set_fullscreen(on.then(|| Fullscreen::Borderless(self.window.current_monitor())));
        }
    }

    /// Draws into the window's next image with `draw` (given its view, width and height) and
    /// shows it.
    pub fn paint(&mut self, gpu: &Gpu, draw: impl FnOnce(&wgpu::TextureView, u32, u32)) {
        let out = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&gpu.device, &self.config);
                return;
            }
        };
        let view = out.texture.create_view(&Default::default());
        draw(&view, self.config.width, self.config.height);
        self.window.pre_present_notify();
        gpu.queue.present(out);
    }

    /// Shows a frame now.
    pub fn present(&mut self, gpu: &Gpu, frame: &melee_gpu::Frame) {
        let Some(texture) = &frame.texture else {
            return;
        };
        let out = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&gpu.device, &self.config);
                return;
            }
        };
        let view = out.texture.create_view(&Default::default());
        self.blit
            .draw(gpu, texture, &view, self.config.width, self.config.height);
        self.window.pre_present_notify();
        gpu.queue.present(out);
    }
}

/// An offscreen target of a display's size, for timing the presentation without a window.
pub struct Offscreen {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    blit: Blit,
}

impl Offscreen {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Offscreen {
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Offscreen {
            texture,
            width,
            height,
            blit: Blit::new(gpu, format),
        }
    }

    /// Draws a frame into the target and waits for the GPU to finish it.
    pub fn present(&mut self, gpu: &Gpu, frame: &melee_gpu::Frame) {
        let Some(texture) = &frame.texture else {
            return;
        };
        let view = self.texture.create_view(&Default::default());
        self.blit.draw(gpu, texture, &view, self.width, self.height);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspects_parse_to_the_game_s_stretched_or_widened() {
        assert_eq!(Aspect::parse("73:60"), Ok(Aspect::GAME));
        assert_eq!(Aspect::parse("4:3"), Ok(Aspect::STRETCHED));
        assert_eq!(Aspect::parse("16:9"), Ok(Aspect::wide(16.0 / 9.0)));
        assert_eq!(Aspect::parse("21:9"), Ok(Aspect::wide(21.0 / 9.0)));
        assert_eq!(Aspect::GAME.widescreen(), None);
        assert_eq!(Aspect::STRETCHED.widescreen(), None);
        assert_eq!(Aspect::wide(16.0 / 9.0).widescreen(), Some(16.0 / 9.0));
        assert!(Aspect::parse("1:1").is_err());
        assert!(Aspect::parse("16x9").is_err());
        assert!(Aspect::parse("16:0").is_err());
    }

    #[test]
    fn the_efb_is_as_wide_as_the_frame_is_shown() {
        assert_eq!(Aspect::STRETCHED.efb_width(1), 640);
        assert_eq!(Aspect::STRETCHED.efb_width(3), 1920);
        assert_eq!(Aspect::GAME.efb_width(1), 584);
        assert_eq!(Aspect::GAME.efb_width(3), 1752);
        assert_eq!(Aspect::wide(16.0 / 9.0).efb_width(3), 2560);
        assert_eq!(Aspect::wide(16.0 / 9.0).efb_width(1), 854);
        assert_eq!(Aspect::wide(21.0 / 9.0).max_scale(), 7);
        assert_eq!(Aspect::GAME.max_scale(), 8);
    }
}
