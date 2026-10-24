// SPDX-License-Identifier: GPL-3.0-or-later
// The idea and the neighbour test follow melee-unlocked's --flicker-scan
// (port/runtime/gx/flicker_scan.cpp, GPL-2.0-or-later); the flatness test is this player's own.

//! `--flicker-scan`: catches a frame that drops out for a single presented frame, which F12 is
//! too slow for. Each presented image is copied into a small ring on the GPU and reduced there
//! to a 64 by 64 grid of luminance, read back a few frames later without waiting for the GPU.
//! A frame that pops out from both neighbours while they match each other, and is much flatter
//! than both ([`Judged::hit`]), is a hit: logged, and saved to `captures/<ms>-flicker` beside
//! the executable (the three grids, the three presented images, `flicker.txt`, and in `after/`
//! the F12 capture of the next frame drawn), at most one every [`COOLDOWN`] and [`MAX_CAPTURES`]
//! a run.
//!
//! `MELEE_HD_FLICKER_TEST=N` (with `--flicker-scan`) blanks every Nth presented frame, to test it.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use crate::{capture, present};

/// The grid's side.
pub const GRID: usize = 64;
const CELLS: usize = GRID * GRID;
/// Presented images kept whole: the three around a hit must outlast their grids' readback.
const RING: usize = 6;
/// Grids reading back at once; a frame finding none free goes unscanned.
const READBACKS: usize = 8;
/// The least time between two captures (the F12 capture in `after/` stalls the game for a few
/// seconds at 5x), and the most a run saves.
pub const COOLDOWN: Duration = Duration::from_secs(30);
pub const MAX_CAPTURES: u32 = 10;

/// A hit differs from both neighbours by at least this much (mean over the cells, luminance
/// 0 to 255)...
const POP_MIN: f32 = 8.0;
/// ...and by this many times the difference between them (normal motion sits between them).
const POP_RATIO: f32 = 2.0;
/// Its detail (adjacent cells' difference) is at most this fraction of the flatter neighbour's:
/// the world gone, leaving the clear color and the HUD.
const FLAT_RATIO: f32 = 0.5;
/// It isn't brighter than both by more than this: a flash isn't a dropout.
const BRIGHTER: f32 = 24.0;

/// A frame's luminance grid, row by row from the top.
pub type Grid = Box<[f32; CELLS]>;

/// What drew a presented frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meta {
    pub tick: u64,
    /// The blend's t, between ticks.
    pub t: Option<f32>,
    pub draws: usize,
}

/// A frame measured against the frames before and after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Judged {
    pub to_prev: f32,
    pub to_next: f32,
    /// The frames before and after, against each other.
    pub across: f32,
    /// Detail and mean brightness: before, the frame, after.
    pub detail: [f32; 3],
    pub mean: [f32; 3],
}

impl Judged {
    pub fn of(a: &[f32; CELLS], b: &[f32; CELLS], c: &[f32; CELLS]) -> Judged {
        Judged {
            to_prev: distance(a, b),
            to_next: distance(b, c),
            across: distance(a, c),
            detail: [detail(a), detail(b), detail(c)],
            mean: [mean(a), mean(b), mean(c)],
        }
    }

    /// The frame dropped out: it pops out from both neighbours, which match, and is much flatter
    /// than both and not much brighter.
    pub fn hit(&self) -> bool {
        let out = self.to_prev.min(self.to_next);
        let [da, db, dc] = self.detail;
        let [ma, mb, mc] = self.mean;
        out >= POP_MIN
            && out >= POP_RATIO * self.across
            && db <= FLAT_RATIO * da.min(dc)
            && mb <= ma.max(mc) + BRIGHTER
    }
}

/// The mean difference of two grids' cells.
fn distance(a: &[f32; CELLS], b: &[f32; CELLS]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / CELLS as f32
}

/// The mean difference between adjacent cells, across and down: how much the frame shows.
fn detail(g: &[f32; CELLS]) -> f32 {
    let mut sum = 0.0;
    for y in 0..GRID {
        for x in 0..GRID {
            let v = g[y * GRID + x];
            if x + 1 < GRID {
                sum += (g[y * GRID + x + 1] - v).abs();
            }
            if y + 1 < GRID {
                sum += (g[(y + 1) * GRID + x] - v).abs();
            }
        }
    }
    sum / (2 * GRID * (GRID - 1)) as f32
}

fn mean(g: &[f32; CELLS]) -> f32 {
    g.iter().sum::<f32>() / CELLS as f32
}

/// The detector: grids in presentation order, a verdict for each frame with both neighbours.
#[derive(Default)]
pub struct Detector {
    last: VecDeque<(u64, Grid, Meta)>,
}

/// A hit: the three frames' numbers, grids and draws, and the measures.
pub struct Hit {
    pub frames: [u64; 3],
    pub grids: [Grid; 3],
    pub meta: [Meta; 3],
    pub judged: Judged,
}

impl Detector {
    /// Takes frame `frame`'s grid; a gap since the last frame starts over.
    pub fn push(&mut self, frame: u64, grid: Grid, meta: Meta) -> Option<Hit> {
        if self.last.back().is_some_and(|(f, ..)| *f + 1 != frame) {
            self.last.clear();
        }
        self.last.push_back((frame, grid, meta));
        if self.last.len() > 3 {
            self.last.pop_front();
        }
        if self.last.len() < 3 {
            return None;
        }
        let [(fa, a, ma), (fb, b, mb), (fc, c, mc)] = [0, 1, 2].map(|i| &self.last[i]);
        let judged = Judged::of(a, b, c);
        judged.hit().then(|| Hit {
            frames: [*fa, *fb, *fc],
            grids: [a.clone(), b.clone(), c.clone()],
            meta: [*ma, *mb, *mc],
            judged,
        })
    }
}

/// The scan's reduction pass: the picture's rectangle cut into the grid's cells, each the mean
/// luminance of up to 16 by 16 of its pixels.
const REDUCE: &str = r#"
struct Params {
    origin: vec2<u32>,
    size: vec2<u32>,
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var<uniform> p: Params;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let cell = vec2<u32>(pos.xy);
    let lo = p.origin + cell * p.size / 64u;
    let hi = max(p.origin + (cell + 1u) * p.size / 64u, lo + 1u);
    let step = max((hi - lo) / 16u, vec2<u32>(1u));
    var sum = 0.0;
    var n = 0.0;
    for (var y = lo.y; y < hi.y; y += step.y) {
        for (var x = lo.x; x < hi.x; x += step.x) {
            let c = textureLoad(image, vec2<u32>(x, y), 0).rgb;
            sum += dot(c, vec3<f32>(0.299, 0.587, 0.114));
            n += 1.0;
        }
    }
    return vec4<f32>(sum / n * 255.0, 0.0, 0.0, 1.0);
}
"#;

const FREE: u8 = 0;
const WAITING: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;

/// A buffer mapping for reading, and where it is.
struct Mapped {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
}

impl Mapped {
    fn new(device: &wgpu::Device, label: &str, size: u64) -> Mapped {
        Mapped {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(FREE)),
        }
    }

    /// Maps it once the GPU has done what's been submitted; whoever polls the device next (the
    /// GPU timing thread, or a submit) notes it done.
    fn map(&self) {
        self.state.store(WAITING, Ordering::Release);
        let state = self.state.clone();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                state.store(if r.is_ok() { READY } else { FAILED }, Ordering::Release);
            });
    }

    fn state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }
}

/// The presented images kept whole, at the swap chain's size and format.
struct Ring {
    textures: Vec<wgpu::Texture>,
    views: Vec<wgpu::TextureView>,
    groups: Vec<wgpu::BindGroup>,
    /// The frame each holds.
    frames: Vec<Option<u64>>,
}

/// A hit's presented images, reading back.
struct Save {
    dir: PathBuf,
    images: Vec<(&'static str, Mapped, u32, u32)>,
    bgra: bool,
}

pub struct Scan {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    grid_view: wgpu::TextureView,
    grid: wgpu::Texture,
    ring: Option<Ring>,
    readbacks: Vec<(Mapped, u64, Meta)>,
    /// Readbacks waiting, in frame order.
    order: VecDeque<usize>,
    detector: Detector,
    /// The next presented frame's number.
    frame: u64,
    /// `MELEE_HD_FLICKER_TEST`: blanks every Nth presented frame.
    blank_every: Option<u64>,
    hits: u64,
    captures: u32,
    last_capture: Option<Instant>,
    /// Hits logged at most one a second; the rest are counted into the next line.
    last_log: Option<Instant>,
    unlogged: u64,
    saves: Vec<Save>,
}

impl Scan {
    pub fn new(gpu: &present::Gpu) -> Scan {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flicker scan"),
            source: wgpu::ShaderSource::Wgsl(REDUCE.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("flicker scan"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
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
            label: Some("flicker scan"),
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
                targets: &[Some(wgpu::TextureFormat::R32Float.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("flicker scan params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grid = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flicker scan grid"),
            size: wgpu::Extent3d {
                width: GRID as u32,
                height: GRID as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readbacks = (0..READBACKS)
            .map(|_| {
                let m = Mapped::new(device, "flicker scan readback", (CELLS * 4) as u64);
                (m, 0, Meta::default())
            })
            .collect();
        let blank_every = std::env::var("MELEE_HD_FLICKER_TEST")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n: &u64| n >= 2);
        if let Some(n) = blank_every {
            eprintln!("flicker scan: blanking every {n}th presented frame (MELEE_HD_FLICKER_TEST)");
        }
        Scan {
            pipeline,
            layout,
            params,
            grid_view: grid.create_view(&Default::default()),
            grid,
            ring: None,
            readbacks,
            order: VecDeque::new(),
            detector: Detector::default(),
            frame: 0,
            blank_every,
            hits: 0,
            captures: 0,
            last_capture: None,
            last_log: None,
            unlogged: 0,
            saves: Vec::new(),
        }
    }

    /// The next presented frame is to be blanked (`MELEE_HD_FLICKER_TEST`).
    pub fn blank_next(&self) -> bool {
        self.blank_every.is_some_and(|n| self.frame % n == n - 1)
    }

    /// Makes the ring at `texture`'s size and format, again if it changed.
    fn fit_ring(&mut self, device: &wgpu::Device, texture: &wgpu::Texture) {
        let fits = self.ring.as_ref().is_some_and(|r| {
            let t = &r.textures[0];
            t.size() == texture.size() && t.format() == texture.format()
        });
        if !fits {
            let textures: Vec<_> = (0..RING)
                .map(|_| {
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("flicker scan ring"),
                        size: texture.size(),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: texture.format(),
                        usage: wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                })
                .collect();
            let views: Vec<_> = textures
                .iter()
                .map(|t| t.create_view(&Default::default()))
                .collect();
            let groups = views
                .iter()
                .map(|v| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("flicker scan"),
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(v),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: self.params.as_entire_binding(),
                            },
                        ],
                    })
                })
                .collect();
            self.ring = Some(Ring {
                textures,
                views,
                groups,
                frames: vec![None; RING],
            });
        }
    }

    /// Scans the image about to be presented: `surface`, the swap chain's image, copied when
    /// `copyable`, else `fill` draws the same into the ring's view; `picture` is the frame's
    /// rectangle in it (left, top, width, height).
    pub fn record(
        &mut self,
        gpu: &present::Gpu,
        surface: &wgpu::Texture,
        copyable: bool,
        fill: impl FnOnce(&wgpu::TextureView),
        picture: [u32; 4],
        meta: Meta,
    ) {
        let frame = self.frame;
        self.frame += 1;
        let Some(k) = (0..READBACKS).find(|&k| self.readbacks[k].0.state() == FREE) else {
            return;
        };
        let device = &gpu.device;
        let slot = frame as usize % RING;
        let size = surface.size();
        self.fit_ring(device, surface);
        let ring = self.ring.as_ref().expect("the ring");
        let mut encoder = device.create_command_encoder(&Default::default());
        if copyable {
            encoder.copy_texture_to_texture(
                surface.as_image_copy(),
                ring.textures[slot].as_image_copy(),
                size,
            );
        } else {
            fill(&ring.views[slot]);
        }
        let [left, top, width, height] = picture;
        let left = left.min(size.width - 1);
        let top = top.min(size.height - 1);
        let params = [
            left,
            top,
            width.clamp(1, size.width - left),
            height.clamp(1, size.height - top),
        ];
        let bytes: Vec<u8> = params.iter().flat_map(|v| v.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.params, 0, &bytes);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flicker scan"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.grid_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &ring.groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        let (readback, at, about) = &mut self.readbacks[k];
        encoder.copy_texture_to_buffer(
            self.grid.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((GRID * 4) as u32),
                    rows_per_image: Some(GRID as u32),
                },
            },
            self.grid.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        readback.map();
        *at = frame;
        *about = meta;
        self.ring.as_mut().expect("the ring").frames[slot] = Some(frame);
        self.order.push_back(k);
    }

    /// Takes in the grids read back since the last call and judges them; finishes saving hits'
    /// images. Returns the folder of each new capture, for the F12 capture to go in `after/`.
    pub fn collect(&mut self, gpu: &present::Gpu, mode: &str) -> Vec<PathBuf> {
        self.finish_saves();
        let mut captured = Vec::new();
        while let Some(&k) = self.order.front() {
            let (readback, frame, meta) = &self.readbacks[k];
            match readback.state() {
                READY => {
                    let mut grid: Grid = Box::new([0.0; CELLS]);
                    if let Ok(data) = readback.buffer.slice(..).get_mapped_range() {
                        for (g, b) in grid.iter_mut().zip(data.as_chunks::<4>().0) {
                            *g = f32::from_le_bytes(*b);
                        }
                    }
                    readback.buffer.unmap();
                    readback.state.store(FREE, Ordering::Release);
                    let (frame, meta) = (*frame, *meta);
                    self.order.pop_front();
                    if let Some(hit) = self.detector.push(frame, grid, meta)
                        && let Some(dir) = self.hit(gpu, hit, mode)
                    {
                        captured.push(dir);
                    }
                }
                FAILED => {
                    readback.state.store(FREE, Ordering::Release);
                    self.order.pop_front();
                }
                _ => break,
            }
        }
        captured
    }

    /// Logs a hit and, unless captures are rate-limited, saves it.
    fn hit(&mut self, gpu: &present::Gpu, hit: Hit, mode: &str) -> Option<PathBuf> {
        self.hits += 1;
        let now = Instant::now();
        let capture =
            self.captures < MAX_CAPTURES && self.last_capture.is_none_or(|t| now - t >= COOLDOWN);
        let j = hit.judged;
        let [_, bad, _] = hit.frames;
        let mut line = format!(
            "flicker scan: presented frame {bad} (tick {}) dropped out: {:.1} from the frame before, {:.1} from the one after, {:.1} between them; detail {:.1} against {:.1} and {:.1}; brightness {:.0} against {:.0} and {:.0}; {} so far",
            hit.meta[1].tick,
            j.to_prev,
            j.to_next,
            j.across,
            j.detail[1],
            j.detail[0],
            j.detail[2],
            j.mean[1],
            j.mean[0],
            j.mean[2],
            self.hits,
        );
        let dir = capture.then(|| capture::folder("-flicker"));
        match &dir {
            Some(d) => {
                let _ = write!(line, "; captured to {}", d.display());
            }
            None => line.push_str("; not captured (rate limit)"),
        }
        if self.unlogged > 0 {
            let _ = write!(line, " ({} more not logged)", self.unlogged);
        }
        if capture
            || self
                .last_log
                .is_none_or(|t| now - t >= Duration::from_secs(1))
        {
            eprintln!("{line}");
            self.last_log = Some(now);
            self.unlogged = 0;
        } else {
            self.unlogged += 1;
        }
        let dir = dir?;
        self.captures += 1;
        self.last_capture = Some(now);
        if let Err(e) = self.save(gpu, &dir, &hit, mode) {
            eprintln!("flicker scan: the capture failed: {e}");
        }
        Some(dir)
    }

    /// Writes the hit's grids and measures, and starts reading back its presented images.
    fn save(
        &mut self,
        gpu: &present::Gpu,
        dir: &Path,
        hit: &Hit,
        mode: &str,
    ) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        const NAMES: [&str; 3] = ["1-before", "2-bad", "3-after"];
        for (name, g) in NAMES.iter().zip(&hit.grids) {
            let path = dir.join(format!("grid-{name}.png"));
            grid_image(g)
                .save_png(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
        let j = hit.judged;
        let mut text = String::new();
        let _ = writeln!(text, "{}", crate::channel::stamp());
        let _ = writeln!(text, "mode {mode}");
        let _ = writeln!(
            text,
            "difference (mean over {GRID}x{GRID} cells, luminance 0-255): to the frame before {:.2}, to the one after {:.2}, between them {:.2}",
            j.to_prev, j.to_next, j.across
        );
        let _ = writeln!(
            text,
            "detail (adjacent cells' difference): {:.2?}",
            j.detail
        );
        let _ = writeln!(text, "brightness: {:.1?}", j.mean);
        let _ = writeln!(
            text,
            "rule: both differences >= {POP_MIN} and >= {POP_RATIO} x between them; detail <= {FLAT_RATIO} x the neighbours' lesser; brightness <= the neighbours' greater + {BRIGHTER}"
        );
        for ((name, f), m) in NAMES.iter().zip(hit.frames).zip(hit.meta) {
            let _ = writeln!(
                text,
                "{name}: presented frame {f}, tick {}, blend t {:?}, draws {}",
                m.tick, m.t, m.draws
            );
        }
        let _ = writeln!(
            text,
            "after/: the F12 capture of the next frame drawn (it finds the player's state a few frames after the bad one)"
        );
        // The presented images, from the ring while it still holds them.
        let ring = self.ring.as_ref().ok_or("no ring")?;
        let format = ring.textures[0].format();
        let bgra = match format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => Some(true),
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Some(false),
            _ => None,
        };
        let mut images = Vec::new();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        for (name, f) in NAMES.iter().zip(hit.frames) {
            let slot = f as usize % RING;
            if bgra.is_none() || ring.frames[slot] != Some(f) {
                let _ = writeln!(
                    text,
                    "{name}.png: not saved ({})",
                    if bgra.is_none() {
                        format!("the swap chain's format is {format:?}")
                    } else {
                        "the ring no longer held it".into()
                    }
                );
                continue;
            }
            let t = &ring.textures[slot];
            let (w, h) = (t.width(), t.height());
            let padded = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            let m = Mapped::new(&gpu.device, "flicker scan image", u64::from(padded * h));
            encoder.copy_texture_to_buffer(
                t.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &m.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded),
                        rows_per_image: Some(h),
                    },
                },
                t.size(),
            );
            images.push((*name, m, w, h));
        }
        gpu.queue.submit([encoder.finish()]);
        for (_, m, ..) in &images {
            m.map();
        }
        std::fs::write(dir.join("flicker.txt"), text).map_err(|e| format!("flicker.txt: {e}"))?;
        self.saves.push(Save {
            dir: dir.to_owned(),
            images,
            bgra: bgra == Some(true),
        });
        Ok(())
    }

    /// Writes the images of saves whose readback finished, on a thread of their own.
    fn finish_saves(&mut self) {
        let mut i = 0;
        while i < self.saves.len() {
            let states: Vec<u8> = self.saves[i]
                .images
                .iter()
                .map(|(_, m, ..)| m.state())
                .collect();
            if states.contains(&WAITING) {
                i += 1;
                continue;
            }
            let save = self.saves.swap_remove(i);
            let mut frames = Vec::new();
            for (name, m, w, h) in save.images {
                if m.state() != READY {
                    continue;
                }
                let row = (w * 4) as usize;
                let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
                let mut rgba = Vec::with_capacity(row * h as usize);
                if let Ok(data) = m.buffer.slice(..).get_mapped_range() {
                    for r in data.chunks(padded) {
                        rgba.extend_from_slice(&r[..row]);
                    }
                }
                m.buffer.unmap();
                frames.push((name, w, h, rgba));
            }
            let (dir, bgra) = (save.dir, save.bgra);
            let _ = std::thread::Builder::new()
                .name("flicker capture".into())
                .spawn(move || {
                    for (name, width, height, mut rgba) in frames {
                        if bgra {
                            for p in rgba.as_chunks_mut::<4>().0 {
                                p.swap(0, 2);
                            }
                        }
                        let f = melee_gpu::Frame {
                            width,
                            height,
                            rgba,
                            texture: None,
                        };
                        let path = dir.join(format!("{name}.png"));
                        if let Err(e) = f.save_png(&path) {
                            eprintln!("flicker scan: {}: {e}", path.display());
                        }
                    }
                });
        }
    }
}

/// A grid as a gray image, each cell 4 pixels square.
fn grid_image(g: &[f32; CELLS]) -> melee_gpu::Frame {
    const K: usize = 4;
    let side = GRID * K;
    let mut rgba = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let v = g[(y / K) * GRID + x / K].clamp(0.0, 255.0) as u8;
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
    }
    melee_gpu::Frame {
        width: side as u32,
        height: side as u32,
        rgba,
        texture: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame with detail: a gradient with a pattern, shifted by `dx` cells and brightened by
    /// `lift`.
    fn scene(dx: usize, lift: f32) -> Grid {
        let mut g: Grid = Box::new([0.0; CELLS]);
        for y in 0..GRID {
            for x in 0..GRID {
                let u = x + dx;
                g[y * GRID + x] = (40.0
                    + y as f32
                    + if (u / 4 + y / 4).is_multiple_of(2) {
                        60.0
                    } else {
                        0.0
                    }
                    + lift)
                    .min(255.0);
            }
        }
        g
    }

    /// The clear color with a HUD strip at the bottom.
    fn dropout(color: f32) -> Grid {
        let mut g: Grid = Box::new([color; CELLS]);
        for x in 8..56 {
            g[60 * GRID + x] = 230.0;
        }
        g
    }

    fn verdicts(frames: Vec<Grid>) -> Vec<u64> {
        let mut d = Detector::default();
        frames
            .into_iter()
            .enumerate()
            .filter_map(|(i, g)| d.push(i as u64, g, Meta::default()))
            .map(|h| h.frames[1])
            .collect()
    }

    #[test]
    fn a_frame_dropping_out_is_a_hit() {
        let frames = vec![
            scene(0, 0.0),
            scene(1, 0.0),
            dropout(20.0),
            scene(2, 0.0),
            scene(3, 0.0),
        ];
        assert_eq!(verdicts(frames), [2]);
        // A bright clear color too, as long as it's no flash.
        let frames = vec![scene(0, 0.0), dropout(110.0), scene(1, 0.0)];
        assert_eq!(verdicts(frames), [1]);
    }

    #[test]
    fn motion_cuts_and_flashes_are_not() {
        // Steady motion.
        assert!(verdicts((0..20).map(|i| scene(i, 0.0)).collect()).is_empty());
        // A cut that stays.
        let mut frames: Vec<Grid> = (0..5).map(|i| scene(i, 0.0)).collect();
        frames.extend((0..5).map(|_| dropout(20.0)));
        frames.extend((0..5).map(|i| scene(i + 30, 0.0)));
        assert!(verdicts(frames).is_empty());
        // A one-frame white flash: flat, but bright.
        let frames = vec![scene(0, 0.0), dropout(250.0), scene(1, 0.0)];
        assert!(verdicts(frames).is_empty());
        // A one-frame brightening that keeps the detail.
        let frames = vec![scene(0, 0.0), scene(0, 60.0), scene(1, 0.0)];
        assert!(verdicts(frames).is_empty());
        // A still screen with a little noise.
        let mut noisy = scene(0, 0.0);
        noisy[100] += 3.0;
        assert!(verdicts(vec![scene(0, 0.0), noisy, scene(0, 0.0)]).is_empty());
    }

    #[test]
    fn a_gap_starts_over() {
        let mut d = Detector::default();
        assert!(d.push(0, scene(0, 0.0), Meta::default()).is_none());
        assert!(d.push(1, dropout(20.0), Meta::default()).is_none());
        // Frame 2 went unscanned: frame 1 has no next to judge it by.
        assert!(d.push(3, scene(1, 0.0), Meta::default()).is_none());
    }
}
