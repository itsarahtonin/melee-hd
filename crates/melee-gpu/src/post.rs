// SPDX-License-Identifier: GPL-3.0-or-later

//! The present's filters, the player's Supersampling and Sharpening (both off by default: they
//! change the picture from the console's). Supersampling draws the EFB at the internal scale
//! times its factor (the player's choice of scale) and filters the frame down here, averaging
//! the texels under each pixel, where the plain blit's bilinear sample would skip them.
//! Sharpening draws the picture at the target's size first (filtered down, or scaled as the
//! blit scales it), then sharpens it into the target, adapting to its local contrast
//! (post.wgsl). With both off the player blits as it always did and none of this runs.

use crate::gpu_time::{self, GpuTimer, Stage};

/// The present's filters.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Filters {
    /// Supersampling's factor per side: 1 (off), 2 or 4. The EFB is drawn this many times
    /// larger by the player; here it only decides the filter down.
    pub supersampling: u32,
    /// Sharpening's strength in percent, when on.
    pub sharpening: Option<u32>,
}

impl Filters {
    pub const OFF: Filters = Filters {
        supersampling: 1,
        sharpening: None,
    };

    /// Supersampling's choices.
    pub const SUPERSAMPLING: [u32; 3] = [1, 2, 4];

    /// Whether the present is the plain blit.
    pub fn is_off(&self) -> bool {
        self.supersampling <= 1 && self.sharpening.is_none()
    }
}

/// The intermediate picture's format, which sharpening reads.
const MID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The filters' pipelines for one target format, and the picture sharpening reads.
pub struct Post {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    /// Filtered down into the target, and into the intermediate picture.
    down: wgpu::RenderPipeline,
    down_mid: wgpu::RenderPipeline,
    /// Scaled as the blit scales into the intermediate picture.
    plain_mid: wgpu::RenderPipeline,
    sharpen: wgpu::RenderPipeline,
    mid: Option<(wgpu::Texture, u32, u32)>,
}

impl Post {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Post {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(
                    1,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
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
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Post {
            down: pipeline("fs_down", format),
            down_mid: pipeline("fs_down", MID_FORMAT),
            plain_mid: pipeline("fs_plain", MID_FORMAT),
            sharpen: pipeline("fs_sharpen", format),
            layout,
            sampler,
            params,
            mid: None,
        }
    }

    /// Encodes `frame` drawn into `view` (`w` by `h`, cleared to black around it) at `rect`
    /// (left, top, width and height) through `filters`, each pass timed as the present's.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::Texture,
        view: &wgpu::TextureView,
        rect: [f32; 4],
        filters: Filters,
        timer: Option<&GpuTimer>,
    ) {
        let [x, y, vw, vh] = rect;
        let (fw, fh) = (frame.width() as f32, frame.height() as f32);
        let mid_size = (vw.round().max(1.0) as u32, vh.round().max(1.0) as u32);
        let span = if filters.sharpening.is_some() {
            [fw / mid_size.0 as f32, fh / mid_size.1 as f32]
        } else {
            [fw / vw.max(1.0), fh / vh.max(1.0)]
        };
        let sharpness = filters.sharpening.unwrap_or(0).min(100) as f32 / 100.0;
        let params: [f32; 8] = [span[0], span[1], x, y, sharpness, 0.0, 0.0, 0.0];
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        let frame_group = self.bind_group(device, frame);
        let pass_time = || timer.and_then(|t| t.pass(Stage::Present));
        let (pipeline, source) = if filters.sharpening.is_some() {
            if self
                .mid
                .as_ref()
                .is_none_or(|(_, w, h)| (*w, *h) != mid_size)
            {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("post picture"),
                    size: wgpu::Extent3d {
                        width: mid_size.0,
                        height: mid_size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: MID_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                self.mid = Some((texture, mid_size.0, mid_size.1));
            }
            let mid = &self.mid.as_ref().expect("the picture").0;
            let mid_view = mid.create_view(&Default::default());
            let time = pass_time();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post picture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &mid_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                timestamp_writes: gpu_time::writes(&time),
                ..Default::default()
            });
            pass.set_pipeline(if filters.supersampling > 1 {
                &self.down_mid
            } else {
                &self.plain_mid
            });
            pass.set_bind_group(0, &frame_group, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            (&self.sharpen, self.bind_group(device, mid))
        } else {
            (&self.down, frame_group)
        };
        let time = pass_time();
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
            timestamp_writes: gpu_time::writes(&time),
            ..Default::default()
        });
        pass.set_viewport(x, y, vw, vh, 0.0, 1.0);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &source, &[]);
        pass.draw(0..3, 0..1);
    }

    fn bind_group(&self, device: &wgpu::Device, texture: &wgpu::Texture) -> wgpu::BindGroup {
        let tview = texture.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tview),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.as_entire_binding(),
                },
            ],
        })
    }

    /// `frame` through `filters` into a new RGBA8 image `w` by `h` (the picture filling it),
    /// read back: for looking at a filter's frames (`melee-render-check`) and tests.
    pub fn image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &wgpu::Texture,
        w: u32,
        h: u32,
        filters: Filters,
    ) -> Vec<u8> {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("post image"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MID_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        let rect = [0.0, 0.0, w as f32, h as f32];
        self.encode(
            device,
            queue,
            &mut encoder,
            frame,
            &view,
            rect,
            filters,
            None,
        );
        queue.submit([encoder.finish()]);
        read_rgba8(device, queue, &target)
    }
}

/// An RGBA8 texture's pixels, read back.
pub fn read_rgba8(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let (width, height) = (texture.width(), texture.height());
    let row = width * 4;
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("post readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback");
    let data = slice.get_mapped_range().expect("mapped readback");
    let mut rgba = Vec::with_capacity((row * height) as usize);
    for r in data.chunks(padded as usize) {
        rgba.extend_from_slice(&r[..row as usize]);
    }
    drop(data);
    buffer.unmap();
    rgba
}
