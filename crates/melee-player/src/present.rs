//! Presenting frames: the GPU the renderer draws on, a window's surface, and the blit of the
//! XFB (the frame the renderer copied out) to it, letterboxed to 4:3. The swap chain is kept
//! one frame deep, presenting each frame as soon as it's drawn: mailbox where the display has
//! it, else immediate, else vsync.

use std::sync::{Arc, Mutex};

use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

static SIZE: Mutex<(u32, u32)> = Mutex::new((1280, 960));
/// The aspect ratio frames are shown at: 4:3, or wider in widescreen.
static ASPECT: Mutex<f32> = Mutex::new(4.0 / 3.0);

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
            required_features: wgpu::Features::DUAL_SOURCE_BLENDING,
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

    /// A renderer on this GPU, leaving its frames on the GPU.
    pub fn renderer(&self, scale: u32) -> melee_gpu::Renderer {
        let mut r =
            melee_gpu::Renderer::with_device_scaled(self.device.clone(), self.queue.clone(), scale);
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
}

impl Shown {
    pub fn new(gpu: &Gpu, event_loop: &ActiveEventLoop) -> Shown {
        let (w, h) = *SIZE.lock().unwrap();
        let attrs = Window::default_attributes()
            .with_title("melee-hd")
            .with_inner_size(winit::dpi::PhysicalSize::new(w, h));
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
