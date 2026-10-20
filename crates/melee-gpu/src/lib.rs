// SPDX-License-Identifier: GPL-3.0-or-later
// Follows Dolphin's software renderer (GPL-2.0-or-later) in what it draws.
// Ported from ssbm-rs's ssbm-render at the melee-hd-base tag.

//! Draws the GameCube GPU's command stream with wgpu. Each draw's vertices go through the
//! transform unit on the CPU (`melee_gx`), as Dolphin's software renderer does; the GPU rasterizes
//! them into an EFB-sized target and runs the TEV per pixel in integers (gx.wgsl), sampling
//! textures decoded on the CPU by hand. Copies to the XFB come back as frames, through the
//! XFB's YUV encoding as Dolphin's software renderer makes it.

#[cfg(test)]
mod clear_tests;
mod copy;
#[cfg(test)]
mod copy_tests;
#[cfg(test)]
mod depth_tests;
mod encode;
pub mod gpu_time;
mod thread;
#[cfg(test)]
mod vertex_tests;
mod xfb;

pub use gpu_time::GpuTimer;
pub use thread::Threaded;

use std::collections::HashMap;

use rustc_hash::{FxHashMap, FxHashSet};

use bytemuck::{Pod, Zeroable};
use melee_gx::texture::{self, Image};
use melee_gx::xform::{self, Output, Viewport};
use melee_gx::{Draw, Memory, Sink, State, vertex};

pub const EFB_WIDTH: u32 = 640;
pub const EFB_HEIGHT: u32 = 528;

/// The physical addresses the GPU's memory spans (as melee-gx masks them): the console's 24
/// MB, and above it the images melee-render's address space keeps from 0x0380_0000
/// (`melee_render::mem`). Masked to 32 MB, as ssbm-render's copies were, a copy into one of
/// those images landed on the loaded archives at 0x0180_0000 and changed their textures.
pub(crate) const PHYS_MASK: u32 = 0x03FF_FFFF;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn bits(v: u32, at: u32, width: u32) -> u32 {
    (v >> at) & ((1 << width) - 1)
}

/// The EFB's 24-bit depth `z` as the depth buffer holds it, as gx.wgsl's `efb_depth`.
fn efb_depth(z: u32) -> f32 {
    (z as f32 + if z < 0x7F_FFFF { 1.5 } else { 1.0 }) / 16_777_216.0
}

fn sext(v: u32, width: u32) -> i32 {
    ((v << (32 - width)) as i32) >> (32 - width)
}

/// A vertex as the GPU gets it: its position, its colors (RGBA) and its first `texgens`
/// texture coordinates, written as words to the vertex buffer (gx.wgsl's `vdata`).
#[derive(Clone, Copy)]
struct GpuVertex {
    pos: [f32; 4],
    c0: [u8; 4],
    c1: [u8; 4],
    tex: [[f32; 3]; 8],
}

impl GpuVertex {
    fn zeroed() -> GpuVertex {
        GpuVertex {
            pos: [0.0; 4],
            c0: [0; 4],
            c1: [0; 4],
            tex: [[0.0; 3]; 8],
        }
    }

    /// Its words, with `texgens` texture coordinates.
    fn write(&self, texgens: usize, out: &mut Vec<u32>) {
        let mut w = [0u32; 30];
        for (w, p) in w.iter_mut().zip(self.pos) {
            *w = p.to_bits();
        }
        w[4] = u32::from_le_bytes(self.c0);
        w[5] = u32::from_le_bytes(self.c1);
        for (w, t) in w[6..]
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(&self.tex[..texgens])
        {
            *w = t.map(f32::to_bits);
        }
        out.extend_from_slice(&w[..6 + 3 * texgens]);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    bp: [u32; 256],
    colors: [[i32; 4]; 4],
    konsts: [[i32; 4]; 4],
    depth: [f32; 4],
    fog: [f32; 4],
    levels: [[u32; 4]; 2],
    mode: [u32; 4],
}

/// Uniforms are bound at offsets aligned to 256 bytes.
const UNIFORM_STRIDE: usize = std::mem::size_of::<Uniforms>().next_multiple_of(256);

const MODE_DRAW: u32 = 0;
const MODE_CLEAR: u32 = 1;
const MODE_DEPTH_ONLY: u32 = 2;

/// What a pipeline is made for: blending, which channels it writes, and depth.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct PipelineKey {
    /// None, or (src factor, dst factor, subtract) as BP 0x41 has them, and whether the written
    /// alpha is the constant (destination alpha).
    blend: Option<(u32, u32, bool)>,
    const_alpha: bool,
    write_color: bool,
    write_alpha: bool,
    depth_compare: Option<u32>,
    depth_write: bool,
    /// The fragment shader promises a depth no nearer than the rasterized one
    /// (`early_depth_test(greater_equal)`), so the GPU can drop fragments its depth test would
    /// fail before shading them.
    early_z: bool,
}

impl PipelineKey {
    fn pack(&self) -> [u32; 8] {
        let (has_blend, src, dst, sub) = match self.blend {
            Some((s, d, sub)) => (1, s, d, u32::from(sub)),
            None => (0, 0, 0, 0),
        };
        [
            has_blend | sub << 1 | u32::from(self.const_alpha) << 2 | u32::from(self.early_z) << 3,
            src,
            dst,
            u32::from(self.write_color),
            u32::from(self.write_alpha),
            self.depth_compare.map_or(8, |c| c & 7),
            u32::from(self.depth_write),
            0x4D47_5055, // a mark, so a stray list isn't taken for keys
        ]
    }

    fn unpack(k: [u32; 8]) -> Option<PipelineKey> {
        if k[7] != 0x4D47_5055 || k[1] > 7 || k[2] > 7 || k[5] > 8 {
            return None;
        }
        Some(PipelineKey {
            blend: (k[0] & 1 != 0).then_some((k[1], k[2], k[0] & 2 != 0)),
            const_alpha: k[0] & 4 != 0,
            write_color: k[3] != 0,
            write_alpha: k[4] != 0,
            depth_compare: (k[5] < 8).then_some(k[5]),
            depth_write: k[6] != 0,
            early_z: k[0] & 8 != 0,
        })
    }
}

struct Command {
    pipeline: PipelineKey,
    uniform: u32,
    textures: [u32; 8],
    /// The vertices' texture coordinates (the draw call's instance).
    texgens: u32,
    first: u32,
    count: u32,
    scissor: [u32; 4],
}

/// One value the EFB holds everywhere outside a rectangle drawn in since it was cleared whole.
#[derive(Clone, Copy)]
struct Outside<T> {
    value: T,
    /// The bounds (left, top, right, bottom) of every draw since, if any.
    drawn: Option<[u32; 4]>,
}

impl<T: Copy + PartialEq> Outside<T> {
    fn new(value: T) -> Self {
        Outside { value, drawn: None }
    }

    fn draw(&mut self, [x, y, w, h]: [u32; 4]) {
        if w == 0 || h == 0 {
            return;
        }
        let [l, t, r, b] = self.drawn.unwrap_or([x, y, x + w, y + h]);
        self.drawn = Some([l.min(x), t.min(y), r.max(x + w), b.max(y + h)]);
    }

    /// Whether clearing `rect` to `value` leaves what clearing all of the EFB to it would:
    /// `rect` is all of it, or everything outside it already holds the value.
    fn cleared_whole(&self, [x, y, w, h]: [u32; 4], value: T) -> bool {
        if [x, y, w, h] == [0, 0, EFB_WIDTH, EFB_HEIGHT] {
            return true;
        }
        self.value == value
            && self
                .drawn
                .is_none_or(|[l, t, r, b]| x <= l && y <= t && r <= x + w && b <= y + h)
    }
}

/// What the EFB is known to hold where nothing has drawn since its last clear of all of it, so
/// a clear of only part of it can still be made by clearing all of it. New targets hold zeros.
#[derive(Clone, Copy)]
struct Known {
    /// RGBA.
    color: Outside<[u8; 4]>,
    /// All of the EFB's alpha, while nothing writes alpha: in a format without alpha a clear
    /// keeps it, as the target's alpha channel still holds it.
    alpha: Option<u8>,
    depth: Outside<u32>,
}

impl Known {
    fn new() -> Known {
        Known {
            color: Outside::new([0; 4]),
            alpha: Some(0),
            depth: Outside::new(0),
        }
    }
}

/// A clear of all of the EFB made by the next render pass's load operations: RGBA, and the
/// depth as the EFB holds it (24 bits).
#[derive(Clone, Copy, Default)]
struct PendingClear {
    color: Option<[u8; 4]>,
    depth: Option<u32>,
}

struct TexEntry {
    view: wgpu::TextureView,
    _texture: wgpu::Texture,
    levels: u32,
    last_used: u64,
}

/// A texture as the cache keys it: its image, its palette, and a hash of their bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TexKey {
    image: (u32, u32, u32, u32, u32),
    tlut_format: u32,
    hash: u64,
}

/// A frame: a copy to the XFB, as shown.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// The pixels, unless the frame stayed on the GPU (`xfb_on_gpu` without `xfb_readback`).
    pub rgba: Vec<u8>,
    /// The frame on the GPU, with `xfb_on_gpu`.
    pub texture: Option<wgpu::Texture>,
}

impl Frame {
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let rgb: Vec<u8> = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        writer.write_image_data(&rgb)?;
        Ok(())
    }
}

/// Time spent by part, over the frames since the last report.
#[derive(Default)]
struct Profile {
    draw: std::time::Duration,
    xform: std::time::Duration,
    texture: std::time::Duration,
    flush: std::time::Duration,
    readback: std::time::Duration,
    xfb: std::time::Duration,
    /// Vertices drawn and the texture coordinates they carry, and the words, indices and
    /// draw calls flushed.
    vertices: u64,
    words: u64,
    indices: u64,
    calls: u64,
    texgens: u64,
    frames: u32,
}

impl Profile {
    fn report(&mut self) {
        self.frames += 1;
        if self.frames < 300 {
            return;
        }
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0 / f64::from(self.frames);
        let n = u64::from(self.frames);
        eprintln!(
            "render per frame: draws {:.2} ms (transform {:.2}, textures {:.2}), flush {:.2}, readback {:.2}, xfb {:.2}; {} vertices ({:.1} texture coordinates each) in {} words, {} indices, {} draw calls",
            ms(self.draw),
            ms(self.xform),
            ms(self.texture),
            ms(self.flush),
            ms(self.readback),
            ms(self.xfb),
            self.vertices / n,
            self.texgens as f64 / self.vertices.max(1) as f64,
            self.words / n,
            self.indices / n,
            self.calls / n
        );
        *self = Profile::default();
    }
}

/// What each frame draws, digested to compare two renderers' outputs exactly (GX_DIGEST): each
/// vertex drawn in order, with the state it's drawn with when that changes, and the clears and
/// copies, whatever the batching and buffer layout.
struct Digest {
    out: std::io::BufWriter<std::fs::File>,
    hash: u64,
    /// The last state digested.
    state: u64,
}

impl Digest {
    fn add(&mut self, v: u64) {
        self.hash = (self.hash ^ v)
            .wrapping_mul(0xFF51_AFD7_ED55_8CCD)
            .rotate_left(31);
    }

    /// The commands a flush draws, from its buffers.
    fn flush(&mut self, clear: PendingClear, r: &Renderer) {
        if clear.color.is_some() || clear.depth.is_some() {
            self.add(hash_bytes(
                &format!("{:?} {:?}", clear.color, clear.depth).into_bytes(),
            ));
        }
        for c in &r.commands {
            let at = c.uniform as usize;
            let mut state = hash_bytes(&r.uniforms[at..at + std::mem::size_of::<Uniforms>()]);
            for w in c
                .pipeline
                .pack()
                .into_iter()
                .chain(c.textures)
                .chain(c.scissor)
            {
                state = (state ^ u64::from(w)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
            }
            if state != self.state {
                self.state = state;
                self.add(state);
            }
            let words = 6 + 3 * c.texgens as usize;
            for &i in &r.indices[c.first as usize..(c.first + c.count) as usize] {
                let v = &r.vertices[i as usize..i as usize + words];
                self.add(hash_bytes(bytemuck::cast_slice(v)));
            }
        }
    }

    fn frame(&mut self, n: u64) {
        use std::io::Write;
        let _ = writeln!(self.out, "{n} {:016x}", self.hash);
        let _ = self.out.flush();
        self.hash = 0;
        self.state = 0;
    }
}

/// Counts of what the renderer met and doesn't draw yet (or draws approximately), by name.
pub type Unsupported = HashMap<&'static str, u64>;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    efb: wgpu::Texture,
    efb_view: wgpu::TextureView,
    /// The multisampled EFB the draws go to, with `samples` above 1, resolved into `efb`.
    efb_msaa: Option<wgpu::TextureView>,
    samples: u32,
    depth_view: wgpu::TextureView,
    module: wgpu::ShaderModule,
    /// gx.wgsl with its fragment shader promising depth_greater, for the early z pipelines:
    /// where the device has `SHADER_EARLY_DEPTH_TEST` (Vulkan), the EFB has one sample a pixel
    /// and `MELEE_EARLY_Z` isn't 0.
    early_module: Option<wgpu::ShaderModule>,
    layout: wgpu::PipelineLayout,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    pipelines: FxHashMap<PipelineKey, wgpu::RenderPipeline>,
    /// The vertices' words (`GpuVertex::write`).
    vertices: Vec<u32>,
    /// The vertices' order, three to a triangle, each its first word: a draw's vertices are
    /// written once and its triangles name them.
    indices: Vec<u32>,
    uniforms: Vec<u8>,
    /// Where each state's uniforms are in `uniforms`, by their hash.
    uniform_offsets: FxHashMap<u64, u32>,
    commands: Vec<Command>,
    /// A clear the next render pass makes before its commands, and what the EFB holds.
    pending_clear: PendingClear,
    known: Known,
    /// Draws every clear as a rectangle, to check the load operations against.
    #[cfg(test)]
    rect_clears: bool,
    vertex_buffer: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    vertex_layout: wgpu::BindGroupLayout,
    index_buffer: Option<wgpu::Buffer>,
    uniform_buffer: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    textures: Vec<Option<TexEntry>>,
    texture_ids: FxHashMap<TexKey, u32>,
    bind_groups: FxHashMap<[u32; 8], wgpu::BindGroup>,
    /// Hashes of memory ranges this frame, so a texture's bytes hash once a frame.
    hashes: FxHashMap<(u32, u32), u64>,
    /// Hashes of ranges whose bytes never change (the host's own, in one piece, with no copy
    /// laid over them), kept from frame to frame.
    stable_hashes: FxHashMap<(u32, u32), u64>,
    /// Hashes of the ranges a threaded renderer's game thread sent, of the bytes it sent.
    pub(crate) held_hashes: FxHashMap<(u32, u32), u64>,
    /// What copies from the EFB to textures wrote, by address: memory as the GPU then reads it.
    copies: Vec<(u32, Vec<u8>)>,
    /// What copies wrote since the host last took it (`Sink::take_written`).
    written: Vec<(u32, Vec<u8>)>,
    /// Copies from the EFB kept on the GPU instead, by the address they copied to.
    copier: copy::Copier,
    gpu_copies: FxHashMap<u32, copy::Copied>,
    /// Where copies went, to forget those the game writes over (copy::Footprints), when this
    /// renderer reads the game's memory itself (`track_copies`); a threaded one is told.
    footprints: copy::Footprints,
    pub(crate) track_copies: bool,
    /// How many draw calls are queued before a draw flushes them, on the render thread, so
    /// encoding them overlaps the stream.
    pub(crate) flush_at: Option<usize>,
    /// Copy addresses sampled another way than copied, whose copies are encoded into memory.
    on_cpu: FxHashSet<u32>,
    scratch: Scratch,
    built: Built,
    /// The last alpha test register and whether it can fail.
    alpha_test: Option<(u32, bool)>,
    frame: u64,
    frames: Vec<Frame>,
    on_frame: Option<Box<dyn FnMut(Frame) + Send>>,
    pub unsupported: Unsupported,
    /// Prints each draw of the first frame, for debugging.
    pub trace: bool,
    /// Draws everything flat white, unblended and without depth, for debugging.
    pub silhouettes: bool,
    /// Draws without blending, for debugging.
    pub opaque: bool,
    /// Draws only this many draws of each frame, for debugging.
    pub draw_limit: Option<u64>,
    /// Where the time goes, when profiling (GX_PROFILE).
    profile: Option<Profile>,
    /// A digest of what each frame draws, written a line a frame (GX_DIGEST=FILE).
    digest: Option<Digest>,
    /// Copies to the XFB on the GPU, leaving each frame there, rather than reading the EFB back
    /// and copying on the CPU as Dolphin's software renderer does; `xfb_readback` reads each
    /// frame back too.
    pub xfb_on_gpu: bool,
    pub xfb_readback: bool,
    gpu_xfb: Option<xfb::GpuXfb>,
    draws_this_frame: u64,
    /// How many pixels a side each EFB pixel renders as: the EFB is this many times the
    /// console's in each dimension, as are the frames copied out on the GPU. Copies to
    /// textures and readbacks come out at the console's size.
    scale: u32,
    /// The EFB's width in pixels for the console's 640: `640 * scale`, as square as the
    /// console's, or another width, which draws the same picture into narrower or wider pixels
    /// for showing at another aspect ratio with square pixels.
    width: u32,
    /// The driver's pipeline cache, where the device keeps one (`Features::PIPELINE_CACHE`).
    cache: Option<wgpu::PipelineCache>,
    /// Times each render pass on the GPU, when set ([`Renderer::set_timer`]).
    timer: Option<GpuTimer>,
}

/// The EFB's targets at a scale and width: what a resize makes again.
struct EfbTargets {
    efb: wgpu::Texture,
    efb_view: wgpu::TextureView,
    efb_msaa: Option<wgpu::TextureView>,
    depth_view: wgpu::TextureView,
}

impl EfbTargets {
    fn new(device: &wgpu::Device, scale: u32, width: u32, samples: u32) -> EfbTargets {
        let size = wgpu::Extent3d {
            width,
            height: EFB_HEIGHT * scale,
            depth_or_array_layers: 1,
        };
        let efb = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let efb_msaa = (samples > 1).then(|| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("efb, multisampled"),
                    size,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb depth"),
            size,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        EfbTargets {
            efb_view: efb.create_view(&Default::default()),
            efb,
            efb_msaa,
            depth_view: depth.create_view(&Default::default()),
        }
    }
}

/// The texture coordinates the transform unit generates for a draw.
fn texgens(state: &State) -> usize {
    (state.xf[xform::NUM_TEXGENS] & 0xF).min(8) as usize
}

/// The texture maps a draw's TEV and indirect stages read.
fn texture_maps(state: &State) -> impl Iterator<Item = usize> + '_ {
    let genmode = state.bp[0x00];
    let tev = (0..=bits(genmode, 10, 4)).filter_map(move |n| {
        let order = state.bp[0x28 + (n as usize >> 1)];
        let shift = (n & 1) * 12;
        (bits(order, shift + 6, 1) != 0).then(|| bits(order, shift, 3) as usize)
    });
    let indirect = (0..bits(genmode, 16, 3)).map(move |i| bits(state.bp[0x27], 6 * i, 3) as usize);
    tev.chain(indirect)
}

/// The image texture map `map` samples, with no more levels than its size has.
fn sampled_image(state: &State, map: usize) -> Image {
    let image = Image::of(&state.bp, map);
    let max_levels = 32 - image.width.max(image.height).leading_zeros();
    Image {
        levels: image.levels.clamp(1, max_levels),
        ..image
    }
}

/// A fast hash of bytes, for telling textures apart.
fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15 ^ bytes.len() as u64;
    let (chunks, rest) = bytes.as_chunks::<8>();
    for c in chunks {
        let v = u64::from_le_bytes(*c);
        h = (h ^ v).wrapping_mul(0xFF51_AFD7_ED55_8CCD).rotate_left(29);
    }
    for &b in rest {
        h = (h ^ u64::from(b)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    }
    h ^ (h >> 33)
}

/// Dolphin's rounding of a float to an integer: truncation, then up from a half or more. In
/// floats, as an `i32` saturates at 2^31 sixteenths of a pixel (134 million pixels), and a
/// vertex just behind the eye lies farther out: saturated, it moved in clip space.
fn iround(x: f32) -> f32 {
    let t = x.trunc();
    if x - t >= 0.5 { t + 1.0 } else { t }
}

/// A position in the EFB, snapped to the rasterizer's sixteenths and moved so the GPU's pixel
/// centers sample where the GameCube's rasterizer does (9/16 into a pixel, as Dolphin's has it).
fn snap(v: f32) -> f32 {
    (iround(16.0 * v) - 1.0) / 16.0
}

fn compare_function(func: u32) -> wgpu::CompareFunction {
    use wgpu::CompareFunction as C;
    match func {
        0 => C::Never,
        1 => C::Less,
        2 => C::Equal,
        3 => C::LessEqual,
        4 => C::Greater,
        5 => C::NotEqual,
        6 => C::GreaterEqual,
        _ => C::Always,
    }
}

/// A blend factor of BP 0x41 for the color or alpha component; src1 is the TEV's alpha.
fn blend_factor(factor: u32, is_src: bool) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as F;
    match factor {
        0 => F::Zero,
        1 => F::One,
        2 => {
            if is_src {
                F::Dst
            } else {
                F::Src
            }
        }
        3 => {
            if is_src {
                F::OneMinusDst
            } else {
                F::OneMinusSrc
            }
        }
        4 => F::Src1Alpha,
        5 => F::OneMinusSrc1Alpha,
        6 => F::DstAlpha,
        _ => F::OneMinusDstAlpha,
    }
}

impl Renderer {
    /// A renderer on a headless device.
    pub fn new() -> Result<Self, String> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        if std::env::var_os("GX_ADAPTER").is_some() {
            let info = adapter.get_info();
            eprintln!("GPU: {} ({:?}, {})", info.name, info.backend, info.driver);
        }
        Self::with_adapter(&adapter)
    }

    pub fn with_adapter(adapter: &wgpu::Adapter) -> Result<Self, String> {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("melee-gpu"),
            required_features: wgpu::Features::DUAL_SOURCE_BLENDING
                | (adapter.features() & wgpu::Features::SHADER_EARLY_DEPTH_TEST),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        Ok(Self::with_device(device, queue))
    }

    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self::with_device_scaled(device, queue, 1)
    }

    /// A renderer whose EFB is `scale` times the console's in each dimension.
    pub fn with_device_scaled(device: wgpu::Device, queue: wgpu::Queue, scale: u32) -> Self {
        Self::with_device_msaa(device, queue, scale, 1)
    }

    /// A renderer whose EFB is `scale` times the console's in each dimension, drawn with
    /// `samples` samples a pixel (1 or 4): the draws go to a multisampled EFB, resolved into
    /// the EFB that copies, readbacks and frames read at the end of each pass.
    pub fn with_device_msaa(
        device: wgpu::Device,
        queue: wgpu::Queue,
        scale: u32,
        samples: u32,
    ) -> Self {
        let scale = scale.clamp(1, 8);
        Self::with_device_efb(device, queue, scale, EFB_WIDTH * scale, samples)
    }

    /// A renderer whose EFB is `scale` times the console's tall and `width` pixels wide for
    /// the console's 640, drawn with `samples` samples a pixel (see
    /// [`Renderer::with_device_msaa`]). The frames copied out on the GPU are `width` wide;
    /// copies to textures and readbacks still come out at the console's size.
    pub fn with_device_efb(
        device: wgpu::Device,
        queue: wgpu::Queue,
        scale: u32,
        width: u32,
        samples: u32,
    ) -> Self {
        let scale = scale.clamp(1, 8);
        let width = width.max(1);
        let samples = if samples >= 4 { 4 } else { 1 };
        let targets = EfbTargets::new(&device, scale, width, samples);
        // Without wgpu's guards against endless loops, which keep FXC (D3D12) from unrolling the
        // loops that write arrays by index.
        let mut checks = wgpu::ShaderRuntimeChecks::checked();
        checks.force_loop_bounding = false;
        // SAFETY: every loop in gx.wgsl ends: each is bounded by a constant or by a register's
        // count of at most 16.
        let module = unsafe {
            device.create_shader_module_trusted(
                wgpu::ShaderModuleDescriptor {
                    label: Some("gx"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("gx.wgsl").into()),
                },
                checks,
            )
        };
        // With samples, each sample is tested at its own depth, which the one the shader writes
        // for the pixel isn't bound to.
        let early_z = std::env::var("MELEE_EARLY_Z").map_or(true, |v| v != "0" && v != "off");
        let early_module = (early_z
            && samples == 1
            && device
                .features()
                .contains(wgpu::Features::SHADER_EARLY_DEPTH_TEST))
        .then(|| {
            let source = include_str!("gx.wgsl");
            assert_eq!(source.matches("\nfn fs_main(").count(), 1);
            let source = source.replace(
                "\nfn fs_main(",
                "\n@early_depth_test(greater_equal)\nfn fs_main(",
            );
            // SAFETY: as `module`'s.
            unsafe {
                device.create_shader_module_trusted(
                    wgpu::ShaderModuleDescriptor {
                        label: Some("gx early z"),
                        source: wgpu::ShaderSource::Wgsl(source.into()),
                    },
                    checks,
                )
            }
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("state"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                },
                count: None,
            }],
        });
        let texture_entries: Vec<_> = (0..8)
            .map(|i| wgpu::BindGroupLayoutEntry {
                binding: i,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("textures"),
            entries: &texture_entries,
        });
        let vertex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vertices"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gx"),
            bind_group_layouts: &[
                Some(&uniform_layout),
                Some(&texture_layout),
                Some(&vertex_layout),
            ],
            immediate_size: 0,
        });
        let copier = copy::Copier::new(&device, &targets.efb_view);
        let mut r = Renderer {
            efb_view: targets.efb_view,
            efb_msaa: targets.efb_msaa,
            samples,
            depth_view: targets.depth_view,
            efb: targets.efb,
            device,
            queue,
            module,
            early_module,
            layout,
            uniform_layout,
            texture_layout,
            pipelines: FxHashMap::default(),
            vertices: Vec::new(),
            indices: Vec::new(),
            uniforms: Vec::new(),
            uniform_offsets: FxHashMap::default(),
            commands: Vec::new(),
            pending_clear: PendingClear::default(),
            known: Known::new(),
            #[cfg(test)]
            rect_clears: false,
            vertex_buffer: None,
            vertex_layout,
            index_buffer: None,
            uniform_buffer: None,
            textures: Vec::new(),
            texture_ids: FxHashMap::default(),
            bind_groups: FxHashMap::default(),
            hashes: FxHashMap::default(),
            stable_hashes: FxHashMap::default(),
            held_hashes: FxHashMap::default(),
            copies: Vec::new(),
            written: Vec::new(),
            copier,
            gpu_copies: FxHashMap::default(),
            footprints: copy::Footprints::default(),
            track_copies: true,
            flush_at: None,
            on_cpu: Default::default(),
            scratch: Scratch::default(),
            built: Built::default(),
            alpha_test: None,
            frame: 0,
            frames: Vec::new(),
            on_frame: None,
            trace: false,
            silhouettes: false,
            opaque: false,
            draw_limit: None,
            profile: std::env::var_os("GX_PROFILE").map(|_| Profile::default()),
            digest: std::env::var_os("GX_DIGEST").map(|path| Digest {
                out: std::io::BufWriter::new(
                    std::fs::File::create(path).expect("GX_DIGEST's file"),
                ),
                hash: 0,
                state: 0,
            }),
            xfb_on_gpu: false,
            xfb_readback: false,
            gpu_xfb: None,
            draws_this_frame: 0,
            scale,
            width,
            cache: None,
            timer: None,
            unsupported: HashMap::new(),
        };
        // Texture 0: a 1x1 black texture for maps nothing samples.
        r.add_texture(1, 1, &[vec![0; 4]]);
        r
    }

    /// Uses the driver's pipeline cache from an earlier run (`data`, from
    /// [`Renderer::pipeline_cache_data`]), where the device keeps one: pipelines it holds
    /// compile without the driver's work. Call before drawing.
    pub fn use_pipeline_cache(&mut self, data: Option<&[u8]>) {
        if !self
            .device
            .features()
            .contains(wgpu::Features::PIPELINE_CACHE)
        {
            return;
        }
        // Safety: the data comes from `get_data` of a cache made on this kind of device;
        // `fallback` has wgpu start afresh when it doesn't match the adapter.
        self.cache = Some(unsafe {
            self.device
                .create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("melee-gpu"),
                    data,
                    fallback: true,
                })
        });
    }

    /// The driver's pipeline cache's contents, to keep for the next run.
    pub fn pipeline_cache_data(&self) -> Option<Vec<u8>> {
        self.cache.as_ref().and_then(|c| c.get_data())
    }

    /// The pipelines made so far, by what they're made for, to make again at the next run's
    /// start ([`Renderer::warm`]): a frame never waits for a pipeline it has needed before.
    pub fn pipeline_keys(&self) -> Vec<[u32; 8]> {
        let mut keys: Vec<[u32; 8]> = self.pipelines.keys().map(|k| k.pack()).collect();
        keys.sort_unstable();
        keys
    }

    /// Makes the pipelines `keys` name, as [`Renderer::pipeline_keys`] gave them.
    pub fn warm(&mut self, keys: &[[u32; 8]]) {
        for k in keys {
            if let Some(mut key) = PipelineKey::unpack(*k) {
                key.early_z &= self.early_module.is_some();
                self.pipeline(key);
            }
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Times the renderer's passes on the GPU by stage with `timer` (made on this device), or
    /// stops timing them.
    pub fn set_timer(&mut self, timer: Option<GpuTimer>) {
        if let Some(x) = &mut self.gpu_xfb {
            x.timer = timer.clone();
        }
        self.timer = timer;
    }

    /// Queries for a pass of `stage`, while the passes are timed.
    fn pass_time(&self, stage: gpu_time::Stage) -> Option<gpu_time::PassTime> {
        self.timer.as_ref().and_then(|t| t.pass(stage))
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Hands each frame to `f` as it's finished, instead of keeping it for `take_frames`.
    pub fn on_frame(&mut self, f: impl FnMut(Frame) + Send + 'static) {
        self.on_frame = Some(Box::new(f));
    }

    /// The frames finished since last asked.
    pub fn take_frames(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }

    fn note(&mut self, what: &'static str) {
        *self.unsupported.entry(what).or_default() += 1;
    }

    fn add_texture(&mut self, width: u32, height: u32, levels: &[Vec<u8>]) -> u32 {
        let mip_count = levels.len() as u32;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, rgba) in levels.iter().enumerate() {
            // Our level's size, as Dolphin's software renderer has it, against wgpu's.
            let (ow, oh) = (((width - 1) >> level) + 1, ((height - 1) >> level) + 1);
            let (gw, gh) = ((width >> level).max(1), (height >> level).max(1));
            let (w, h) = (ow.min(gw), oh.min(gh));
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for row in 0..h {
                let start = (row * ow * 4) as usize;
                data.extend_from_slice(&rgba[start..start + (w * 4) as usize]);
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.insert_texture(texture, mip_count)
    }

    /// Gives `texture` an id, reusing a free one.
    fn insert_texture(&mut self, texture: wgpu::Texture, levels: u32) -> u32 {
        let entry = TexEntry {
            view: texture.create_view(&Default::default()),
            _texture: texture,
            levels,
            last_used: self.frame,
        };
        let free = self
            .textures
            .iter()
            .skip(1)
            .position(Option::is_none)
            .map(|i| i + 1);
        match free {
            Some(i) => {
                self.textures[i] = Some(entry);
                i as u32
            }
            None => {
                self.textures.push(Some(entry));
                (self.textures.len() - 1) as u32
            }
        }
    }

    /// Memory as the GPU reads it: with what copies from the EFB wrote over it.
    fn read_memory(&self, mem: &dyn Memory, address: u32, len: u32) -> Vec<u8> {
        let mut buf = vec![0; len as usize];
        mem.read(address, &mut buf);
        let (start, end) = (address as u64, address as u64 + len as u64);
        for (at, bytes) in &self.copies {
            let (cs, ce) = (*at as u64, *at as u64 + bytes.len() as u64);
            let (from, to) = (start.max(cs), end.min(ce));
            if from < to {
                buf[(from - start) as usize..(to - start) as usize]
                    .copy_from_slice(&bytes[(from - cs) as usize..(to - cs) as usize]);
            }
        }
        buf
    }

    /// Whether a copy from the EFB this renderer encoded into memory lies over any of the
    /// range.
    fn copied_over(&self, address: u32, len: u32) -> bool {
        let (start, end) = (u64::from(address), u64::from(address) + u64::from(len));
        self.copies.iter().any(|(at, bytes)| {
            let (cs, ce) = (u64::from(*at), u64::from(*at) + bytes.len() as u64);
            start.max(cs) < end.min(ce)
        })
    }

    fn hash_memory(&mut self, mem: &dyn Memory, address: u32, len: u32) -> u64 {
        if let Some(&h) = self.hashes.get(&(address, len)) {
            return h;
        }
        let buf = self.read_memory(mem, address, len);
        let h = hash_bytes(&buf);
        self.hashes.insert((address, len), h);
        h
    }

    /// The texture texture map `map` samples, made if new.
    fn texture(&mut self, state: &State, map: usize, mem: &dyn Memory) -> (u32, u32) {
        let t = self.profile.is_some().then(std::time::Instant::now);
        let r = self.texture_uncached(state, map, mem);
        if let (Some(p), Some(t)) = (&mut self.profile, t) {
            p.texture += t.elapsed();
        }
        r
    }

    fn texture_uncached(&mut self, state: &State, map: usize, mem: &dyn Memory) -> (u32, u32) {
        let image = sampled_image(state, map);
        let tlut_reg = state.bp[texture::reg(texture::SETTLUT, map)];
        let tlut_offset = ((tlut_reg & 0x3FF) << 9) as usize;
        let tlut_format = bits(tlut_reg, 10, 2);
        let size = image.size();
        // A copy the game has written over since is gone, even within the frame.
        if self.track_copies
            && let Some((start, end)) = self.footprints.take_if_overwritten(image.address, mem)
        {
            self.forget(start, end);
        }
        if let Some(c) = self.gpu_copies.get(&image.address) {
            if (c.format, c.width, c.height, image.levels)
                == (image.format, image.width, image.height, 1)
                && let Some(Some(entry)) = self.textures.get_mut(c.id as usize)
            {
                entry.last_used = self.frame;
                return (c.id, 1);
            }
            // Sampled some other way: from now on, its bytes as the encoder would have
            // written them.
            self.note("EFB copy read back to sample it another way");
            self.materialize(image.address);
        }
        // Bytes the host holds as they are and no copy lies over never change: their hash
        // is kept.
        let stable = mem
            .view(image.address, size as usize)
            .filter(|_| !self.copied_over(image.address, size));
        let mut hash = match stable {
            Some(bytes) => *self
                .stable_hashes
                .entry((image.address, size))
                .or_insert_with(|| hash_bytes(bytes)),
            // The hash the game thread sent with the bytes, unless a copy lies over them.
            None => match self.held_hashes.get(&(image.address, size)) {
                Some(&h) if !self.copied_over(image.address, size) => h,
                _ => self.hash_memory(mem, image.address, size),
            },
        };
        let tlut = state.tmem.get(tlut_offset..).unwrap_or(&[]);
        if let Some(entries) = texture::palette_entries(image.format) {
            let len = (entries as usize * 2).min(tlut.len());
            hash ^= hash_bytes(&tlut[..len]).rotate_left(17);
        }
        let key = TexKey {
            image: (
                image.address,
                image.width,
                image.height,
                image.format,
                image.levels,
            ),
            tlut_format,
            hash,
        };
        if let Some(&id) = self.texture_ids.get(&key)
            && let Some(Some(entry)) = self.textures.get_mut(id as usize)
        {
            entry.last_used = self.frame;
            return (id, entry.levels);
        }
        let read;
        let data = match stable {
            Some(bytes) => bytes,
            None => {
                read = self.read_memory(mem, image.address, size);
                &read
            }
        };
        let levels = texture::decode(
            data,
            image.width,
            image.height,
            image.levels,
            image.format,
            tlut,
            tlut_format,
        );
        let id = self.add_texture(image.width, image.height, &levels);
        self.texture_ids.insert(key, id);
        (id, image.levels)
    }

    fn bind_group(&mut self, ids: [u32; 8]) {
        if self.bind_groups.contains_key(&ids) {
            return;
        }
        let views: Vec<_> = ids
            .iter()
            .map(|&id| {
                &self.textures[id as usize]
                    .as_ref()
                    .expect("bound texture")
                    .view
            })
            .collect();
        let entries: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(i, v)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(v),
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_layout,
            entries: &entries,
        });
        self.bind_groups.insert(ids, group);
    }

    fn uniforms_of(state: &State, levels: [u32; 8], mode: [u32; 4]) -> Uniforms {
        let color = |ra: u32, bg: u32| {
            [
                sext(ra & 0x7FF, 11),
                sext(bg >> 12 & 0x7FF, 11),
                sext(bg & 0x7FF, 11),
                sext(ra >> 12 & 0x7FF, 11),
            ]
        };
        let mut u = Uniforms::zeroed();
        u.bp = state.bp;
        for i in 0..4 {
            u.colors[i] = color(state.color_ra[i], state.color_bg[i]);
            u.konsts[i] = color(state.konst_ra[i], state.konst_bg[i]);
        }
        let vp = Viewport::of(&state.xf);
        u.depth = [vp.z_range, vp.far_z, 0.0, 0.0];
        let float = |v: u32| {
            f32::from_bits(bits(v, 19, 1) << 31 | bits(v, 11, 8) << 23 | bits(v, 0, 11) << 12)
        };
        let (a, c) = (state.bp[0xEE], state.bp[0xF1]);
        let nan_case =
            bits(a, 11, 8) == 255 && bits(c, 11, 8) == 255 && bits(a, 19, 1) == bits(c, 19, 1);
        let (fa, fc) = if nan_case {
            (
                0.0,
                if bits(a, 19, 1) == 0 {
                    f32::NEG_INFINITY
                } else {
                    f32::INFINITY
                },
            )
        } else {
            (float(a), float(c))
        };
        u.fog = [fa, fc, vp.wd, 0.0];
        u.levels = [
            [levels[0], levels[1], levels[2], levels[3]],
            [levels[4], levels[5], levels[6], levels[7]],
        ];
        u.mode = mode;
        u
    }

    /// Queues a draw call; one that follows the last with the same state and the vertices
    /// right after its own joins it, as one call draws the same.
    fn push_command(&mut self, c: Command) {
        // Nothing draws outside its scissor.
        let key = c.pipeline;
        if key.write_color || key.write_alpha {
            self.known.color.draw(c.scissor);
        }
        if key.write_alpha {
            self.known.alpha = None;
        }
        if key.depth_write {
            self.known.depth.draw(c.scissor);
        }
        if let Some(last) = self.commands.last_mut()
            && last.pipeline == c.pipeline
            && last.uniform == c.uniform
            && last.textures == c.textures
            && last.texgens == c.texgens
            && last.scissor == c.scissor
            && last.first + last.count == c.first
        {
            last.count += c.count;
            return;
        }
        self.commands.push(c);
    }

    fn push_uniforms(&mut self, u: &Uniforms) -> u32 {
        let mut u = *u;
        // The shader's pixel coordinates are in scaled pixels: the vertical scale, and the
        // EFB's width for the console's 640.
        u.mode[3] = self.scale | (self.width << 4);
        let bytes = bytemuck::bytes_of(&u);
        // The same state as the last draw's shares its uniforms, and so does the same state
        // as any draw's since the last flush.
        if self.uniforms.len() >= UNIFORM_STRIDE {
            let last = self.uniforms.len() - UNIFORM_STRIDE;
            if &self.uniforms[last..last + bytes.len()] == bytes {
                return last as u32;
            }
        }
        let hash = hash_bytes(bytes);
        if let Some(&at) = self.uniform_offsets.get(&hash)
            && &self.uniforms[at as usize..at as usize + bytes.len()] == bytes
        {
            return at;
        }
        let at = self.uniforms.len();
        self.uniforms.extend_from_slice(bytes);
        self.uniforms.resize(at + UNIFORM_STRIDE, 0);
        self.uniform_offsets.insert(hash, at as u32);
        at as u32
    }

    /// The scissor rectangle in the EFB, cut to the viewport (which the GameCube clips to), and
    /// the scissor's offset.
    pub(crate) fn scissor(state: &State, vp: &Viewport) -> ([u32; 4], (f32, f32)) {
        let tl = state.bp[0x20];
        let br = state.bp[0x21];
        let off = state.bp[0x59];
        let (xo, yo) = ((bits(off, 0, 9) * 2) as i32, (bits(off, 10, 9) * 2) as i32);
        let mut left = bits(tl, 12, 11) as i32 - xo;
        let mut top = bits(tl, 0, 11) as i32 - yo;
        let mut right = bits(br, 12, 11) as i32 - xo + 1;
        let mut bottom = bits(br, 0, 11) as i32 - yo + 1;
        let (vl, vr) = (
            vp.x_orig - vp.wd.abs() - xo as f32,
            vp.x_orig + vp.wd.abs() - xo as f32,
        );
        let (vt, vb) = (
            vp.y_orig - vp.ht.abs() - yo as f32,
            vp.y_orig + vp.ht.abs() - yo as f32,
        );
        left = left.max(vl.round() as i32);
        right = right.min(vr.round() as i32);
        top = top.max(vt.round() as i32);
        bottom = bottom.min(vb.round() as i32);
        let left = left.clamp(0, EFB_WIDTH as i32) as u32;
        let right = right.clamp(0, EFB_WIDTH as i32) as u32;
        let top = top.clamp(0, EFB_HEIGHT as i32) as u32;
        let bottom = bottom.clamp(0, EFB_HEIGHT as i32) as u32;
        (
            [
                left,
                top,
                right.saturating_sub(left),
                bottom.saturating_sub(top),
            ],
            (xo as f32, yo as f32),
        )
    }

    /// A vertex at `screen` in the EFB (the screen position of `o`, or a line's or point's
    /// corner beside it), back in clip space for the GPU to clip as the GameCube does.
    fn gpu_vertex(o: &Output, screen: [f32; 3], off: (f32, f32), vp: &Viewport) -> GpuVertex {
        let w = o.clip[3];
        let (hw, hh) = (EFB_WIDTH as f32 / 2.0, EFB_HEIGHT as f32 / 2.0);
        let x = snap(screen[0] - off.0) / hw - 1.0;
        let y = 1.0 - snap(screen[1] - off.1) / hh;
        // In the eye's plane (w = 0) it has no screen position, only a direction.
        let (x, y) = if x.is_finite() && y.is_finite() {
            (x * w, y * w)
        } else {
            (o.clip[0] * vp.wd / hw, -o.clip[1] * vp.ht / hh)
        };
        GpuVertex {
            pos: [x, y, o.clip[2] + w, w],
            c0: o.color[0],
            c1: o.color[1],
            tex: o.tex,
        }
    }

    /// The pipeline and uniforms' mode for the state's blending and depth.
    fn pipeline_key(state: &State) -> PipelineKey {
        let zmode = state.bp[0x40];
        let blend = state.bp[0x41];
        let format = bits(state.bp[0x43], 0, 3);
        let has_alpha = format == 1;
        let test = bits(zmode, 0, 1) != 0;
        PipelineKey {
            blend: (bits(blend, 0, 1) != 0).then(|| {
                (
                    bits(blend, 8, 3),
                    bits(blend, 5, 3),
                    bits(blend, 11, 1) != 0,
                )
            }),
            const_alpha: bits(state.bp[0x42], 8, 1) != 0,
            write_color: bits(blend, 3, 1) != 0,
            write_alpha: bits(blend, 4, 1) != 0 && has_alpha,
            depth_compare: test.then(|| bits(zmode, 1, 3)),
            depth_write: test && bits(zmode, 4, 1) != 0,
            early_z: false,
        }
    }

    /// Whether a draw's depth test can run before its shader: it tests depth, and writes the
    /// depth it rasterized (no z texture replaces or adds to it, or the z test comes before
    /// texturing) through GXSetViewport's z range 0 to 1, for which `efb_depth` never comes
    /// out nearer than the rasterized depth (see gx.wgsl).
    fn depth_follows_raster(state: &State) -> bool {
        let vp = Viewport::of(&state.xf);
        let test = bits(state.bp[0x40], 0, 1) != 0;
        let ztex = bits(state.bp[0xF5], 2, 2) != 0;
        let early = bits(state.bp[0x43], 6, 1) != 0;
        test && (!ztex || early) && vp.z_range == 16_777_215.0 && vp.far_z == 16_777_215.0
    }

    fn pipeline(&mut self, key: PipelineKey) {
        if self.pipelines.contains_key(&key) {
            return;
        }
        let blend = key.blend.map(|(src, dst, subtract)| {
            let component = |is_alpha: bool| {
                if subtract {
                    wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::ReverseSubtract,
                    }
                } else if is_alpha && key.const_alpha {
                    wgpu::BlendComponent::REPLACE
                } else {
                    wgpu::BlendComponent {
                        src_factor: blend_factor(src, true),
                        dst_factor: blend_factor(dst, false),
                        operation: wgpu::BlendOperation::Add,
                    }
                }
            };
            wgpu::BlendState {
                color: component(false),
                alpha: component(true),
            }
        });
        let mut mask = wgpu::ColorWrites::empty();
        if key.write_color {
            mask |= wgpu::ColorWrites::COLOR;
        }
        if key.write_alpha {
            mask |= wgpu::ColorWrites::ALPHA;
        }
        let module = match &self.early_module {
            Some(m) if key.early_z => m,
            _ => &self.module,
        };
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(key.depth_write),
                    depth_compare: Some(
                        key.depth_compare
                            .map_or(wgpu::CompareFunction::Always, compare_function),
                    ),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: self.samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend,
                        write_mask: mask,
                    })],
                }),
                multiview_mask: None,
                cache: self.cache.as_ref(),
            });
        self.pipelines.insert(key, pipeline);
    }

    /// Clears `rect` of the EFB as a copy with the clear bit does: color, alpha and depth as
    /// their updates are enabled. A target the clear leaves as clearing all of it would is
    /// cleared by the next render pass's load operation instead (as melee-unlocked's D3D12
    /// backend clears natively): in a match every clear, as each covers what's been drawn and
    /// the rest of the EFB holds its values. What's left is drawn as a rectangle.
    fn clear(&mut self, state: &State, rect: [u32; 4]) {
        let blend = state.bp[0x41];
        let format = bits(state.bp[0x43], 0, 3);
        let mut color = state.bp[0x4F] << 16 | state.bp[0x50] & 0xFFFF;
        // As ARGB from the registers; to RGBA, dropping the precision the format lacks.
        let (a, r, g, b) = (
            color >> 24 & 0xFF,
            color >> 16 & 0xFF,
            color >> 8 & 0xFF,
            color & 0xFF,
        );
        let six = |v: u32| (v & 0xFC) | (v >> 6);
        color = match format {
            1 => six(r) << 24 | six(g) << 16 | six(b) << 8 | six(a),
            _ => r << 24 | g << 16 | b << 8 | a,
        };
        let mut key = PipelineKey {
            blend: None,
            const_alpha: false,
            write_color: bits(blend, 3, 1) != 0,
            write_alpha: bits(blend, 4, 1) != 0 && format == 1,
            depth_compare: Some(7),
            depth_write: bits(state.bp[0x40], 4, 1) != 0,
            early_z: false,
        };
        let z = state.bp[0x51] & 0xFF_FFFF;
        let [r, g, b, a] = color.to_be_bytes();
        // A load operation clears all four channels: alpha too, so one that keeps alpha
        // clears it to what all of it holds.
        let whole_color = match (key.write_color, key.write_alpha) {
            (true, true) => Some([r, g, b, a]),
            (true, false) => self.known.alpha.map(|a| [r, g, b, a]),
            _ => None,
        }
        .filter(|&c| self.known.color.cleared_whole(rect, c));
        let whole_depth = key
            .depth_write
            .then_some(z)
            .filter(|&z| self.known.depth.cleared_whole(rect, z));
        #[cfg(test)]
        let (whole_color, whole_depth) = if self.rect_clears {
            (None, None)
        } else {
            (whole_color, whole_depth)
        };
        if whole_color.is_some() || whole_depth.is_some() {
            // What's drawn before the clear is drawn first; clears in a row make one.
            if !self.commands.is_empty() {
                self.flush();
            }
            if let Some(c) = whole_color {
                self.pending_clear.color = Some(c);
                self.known.color = Outside::new(c);
                self.known.alpha = Some(c[3]);
                key.write_color = false;
                key.write_alpha = false;
            }
            if let Some(z) = whole_depth {
                self.pending_clear.depth = Some(z);
                self.known.depth = Outside::new(z);
                key.depth_write = false;
            }
        }
        if key.write_color || key.write_alpha || key.depth_write {
            self.push_rect(state, key, rect, [MODE_CLEAR, color, z, 0]);
        }
    }

    fn push_rect(&mut self, state: &State, key: PipelineKey, rect: [u32; 4], mode: [u32; 4]) {
        let [x, y, w, h] = rect;
        if w == 0 || h == 0 {
            return;
        }
        let first = self.indices.len() as u32;
        // At the depth the shader writes for it, the clear's own. On NVIDIA (Vulkan and D3D12)
        // with the GPU frames behind, the clear sometimes landed at its rasterized depth instead
        // of the one its shader wrote; at 0.5 that left the depth buffer nearer than the whole
        // scene, and the next frame drew only what doesn't test depth (its background color and
        // the HUD).
        let z = efb_depth(mode[2]);
        let corner = |px: u32, py: u32| {
            let nx = px as f32 / (EFB_WIDTH as f32 / 2.0) - 1.0;
            let ny = 1.0 - py as f32 / (EFB_HEIGHT as f32 / 2.0);
            GpuVertex {
                pos: [nx, ny, z, 1.0],
                ..GpuVertex::zeroed()
            }
        };
        let (a, b, c, d) = (
            corner(x, y),
            corner(x + w, y),
            corner(x + w, y + h),
            corner(x, y + h),
        );
        self.push_in_order(&[a, b, c, a, c, d], 0);
        let u = Self::uniforms_of(state, [1; 8], mode);
        let uniform = self.push_uniforms(&u);
        self.pipeline(key);
        self.bind_group([0; 8]);
        self.push_command(Command {
            pipeline: key,
            texgens: 0,
            uniform,
            textures: [0; 8],
            first,
            count: 6,
            scissor: rect,
        });
    }

    /// The render pass that draws into the EFB, making the pending clear first.
    fn efb_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        clear: PendingClear,
    ) -> wgpu::RenderPass<'e> {
        let color = clear.color.map_or(wgpu::LoadOp::Load, |c| {
            let unorm = |v: u8| f64::from(v) / 255.0;
            wgpu::LoadOp::Clear(wgpu::Color {
                r: unorm(c[0]),
                g: unorm(c[1]),
                b: unorm(c[2]),
                a: unorm(c[3]),
            })
        });
        // As gx.wgsl's clear writes it.
        let depth = clear
            .depth
            .map_or(wgpu::LoadOp::Load, |z| wgpu::LoadOp::Clear(efb_depth(z)));
        let time = self.pass_time(gpu_time::Stage::Scene);
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("efb"),
            // With samples, every sample is cleared and the pass resolves them into `efb`.
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.efb_msaa.as_ref().unwrap_or(&self.efb_view),
                depth_slice: None,
                resolve_target: self.efb_msaa.as_ref().map(|_| &self.efb_view),
                ops: wgpu::Operations {
                    load: color,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: depth,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: gpu_time::writes(&time),
            ..Default::default()
        })
    }

    /// Draws what's batched into the EFB, after the pending clear: everything that reads the
    /// EFB (copies, readbacks) flushes first, so a clear with nothing drawn after it still
    /// happens, in a pass of its own.
    fn flush(&mut self) {
        let clear = std::mem::take(&mut self.pending_clear);
        if let Some(mut d) = self.digest.take() {
            d.flush(clear, self);
            self.digest = Some(d);
        }
        if self.commands.is_empty() {
            if clear.color.is_some() || clear.depth.is_some() {
                let mut encoder = self.device.create_command_encoder(&Default::default());
                drop(self.efb_pass(&mut encoder, clear));
                self.queue.submit([encoder.finish()]);
            }
            return;
        }
        if let Some(p) = &mut self.profile {
            p.words += self.vertices.len() as u64;
            p.indices += self.indices.len() as u64;
            p.calls += self.commands.len() as u64;
        }
        let vbytes: &[u8] = bytemuck::cast_slice(&self.vertices);
        if self
            .vertex_buffer
            .as_ref()
            .is_none_or(|(b, _)| b.size() < vbytes.len() as u64)
        {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertices"),
                size: (vbytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.vertex_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            self.vertex_buffer = Some((buffer, group));
        }
        if self
            .uniform_buffer
            .as_ref()
            .is_none_or(|(b, _)| b.size() < self.uniforms.len() as u64)
        {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uniforms"),
                size: (self.uniforms.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                    }),
                }],
            });
            self.uniform_buffer = Some((buffer, group));
        }
        let ibytes: &[u8] = bytemuck::cast_slice(&self.indices);
        if self
            .index_buffer
            .as_ref()
            .is_none_or(|b| b.size() < ibytes.len() as u64)
        {
            self.index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indices"),
                size: (ibytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let (vbuf, vgroup) = self.vertex_buffer.as_ref().unwrap();
        let ibuf = self.index_buffer.as_ref().unwrap();
        let (ubuf, ugroup) = self.uniform_buffer.as_ref().unwrap();
        self.queue.write_buffer(vbuf, 0, vbytes);
        self.queue.write_buffer(ibuf, 0, ibytes);
        self.queue.write_buffer(ubuf, 0, &self.uniforms);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = self.efb_pass(&mut encoder, clear);
            pass.set_bind_group(2, vgroup, &[]);
            pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
            // Each state set only when it changes.
            let mut last: Option<PipelineKey> = None;
            let mut uniform = None;
            let mut textures = None;
            let mut scissor = None;
            for c in &self.commands {
                if last != Some(c.pipeline) {
                    pass.set_pipeline(&self.pipelines[&c.pipeline]);
                    last = Some(c.pipeline);
                }
                if uniform != Some(c.uniform) {
                    pass.set_bind_group(0, ugroup, &[c.uniform]);
                    uniform = Some(c.uniform);
                }
                if textures != Some(c.textures) {
                    pass.set_bind_group(1, &self.bind_groups[&c.textures], &[]);
                    textures = Some(c.textures);
                }
                if scissor != Some(c.scissor) {
                    let [x, y, w, h] = c.scissor;
                    let s = self.scale;
                    let (left, right) = (self.px_x(x), self.px_x(x + w));
                    pass.set_scissor_rect(left, y * s, right - left, h * s);
                    scissor = Some(c.scissor);
                }
                pass.draw_indexed(c.first..c.first + c.count, 0, c.texgens..c.texgens + 1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.vertices.clear();
        self.indices.clear();
        self.uniforms.clear();
        self.uniform_offsets.clear();
        self.commands.clear();
    }

    /// The EFB pixel column where the console's column `x` starts.
    fn px_x(&self, x: u32) -> u32 {
        (u64::from(x) * u64::from(self.width) / u64::from(EFB_WIDTH)) as u32
    }

    /// The EFB's color, read back: RGBA rows of 640.
    fn read_efb(&mut self) -> Vec<u8> {
        let (width, height) = (self.width, EFB_HEIGHT * self.scale);
        // Rows padded to what a copy to a buffer needs (an EFB as wide as 73:60's 584 isn't).
        let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("efb readback"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.efb,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU readback");
        let data = slice.get_mapped_range().expect("mapped readback").to_vec();
        buffer.unmap();
        if self.scale == 1 && self.width == EFB_WIDTH && row == width * 4 {
            return data;
        }
        // At the console's size: each pixel the scaled block's middle sample.
        let s = self.scale as usize;
        let (w, h) = (EFB_WIDTH as usize, EFB_HEIGHT as usize);
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let px = (self.px_x(x as u32) + self.px_x(x as u32 + 1)) as usize / 2;
                let at = (y * s + s / 2) * row as usize + px * 4;
                out[(y * w + x) * 4..][..4].copy_from_slice(&data[at..at + 4]);
            }
        }
        out
    }

    /// Makes a copy from the EFB into a texture of its own, on the GPU.
    fn gpu_copy(&mut self, r: &copy::Request) {
        let address = r.footprint.address;
        let reuse = self.gpu_copies.get(&address).and_then(|c| {
            ((c.format, c.width, c.height) == (r.format, r.width, r.height)
                && matches!(self.textures.get(c.id as usize), Some(Some(_))))
            .then_some(c.id)
        });
        let id = match reuse {
            Some(id) => {
                if let Some(c) = self.gpu_copies.get_mut(&address) {
                    c.footprint = r.footprint;
                }
                id
            }
            None => {
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("efb copy"),
                    size: wgpu::Extent3d {
                        width: r.width,
                        height: r.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Uint,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let id = self.insert_texture(texture, 1);
                self.gpu_copies.insert(
                    address,
                    copy::Copied {
                        id,
                        format: r.format,
                        width: r.width,
                        height: r.height,
                        footprint: r.footprint,
                    },
                );
                id
            }
        };
        let entry = self.textures[id as usize].as_mut().expect("copy texture");
        entry.last_used = self.frame;
        let time = self
            .timer
            .as_ref()
            .and_then(|t| t.pass(gpu_time::Stage::EfbCopy));
        self.copier.copy(
            &self.device,
            &self.queue,
            &entry.view,
            r,
            (self.scale, self.width),
            time,
        );
        // What earlier copies read back left in memory there is gone.
        let f = r.footprint;
        let before = self.copies.len();
        for row in 0..f.rows {
            let at = (f.address + row * f.stride) & PHYS_MASK;
            let end = u64::from(at) + u64::from(f.row_bytes);
            self.copies
                .retain(|(a, b)| !(*a >= at && *a as u64 + b.len() as u64 <= end));
        }
        if self.copies.len() != before {
            self.hashes.clear();
        }
    }

    /// Reads a copy kept on the GPU back and keeps its bytes as the encoder would have written
    /// them to memory, for a texture that samples it in another format or size.
    fn materialize(&mut self, address: u32) {
        let Some(c) = self.gpu_copies.remove(&address) else {
            return;
        };
        // Its later copies go to memory too, rather than back and forth.
        self.on_cpu.insert(address);
        let Some(Some(entry)) = self.textures.get(c.id as usize) else {
            return;
        };
        let texels = self.read_texture(&entry._texture, c.width, c.height);
        for (at, row) in copy::encode_texels(&c, &texels) {
            let end = u64::from(at) + row.len() as u64;
            self.copies
                .retain(|(a, b)| !(*a >= at && *a as u64 + b.len() as u64 <= end));
            self.copies.push((at, row));
        }
        self.hashes.clear();
    }

    /// Forgets what copies left in `start..end`, which the game has written over since.
    pub(crate) fn forget(&mut self, start: u32, end: u32) {
        self.gpu_copies.retain(|&a, _| !(start..end).contains(&a));
        self.on_cpu.retain(|a| !(start..end).contains(a));
        let (start, end) = (start & PHYS_MASK, end & PHYS_MASK);
        self.copies
            .retain(|(a, b)| *a as usize + b.len() <= start as usize || *a >= end);
        self.hashes.clear();
    }

    /// An RGBA8 texture's pixels, read back.
    fn read_texture(&self, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
        let row = width * 4;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame readback"),
            size: u64::from(padded * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
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

    /// The FIFO player's clear before a log's first frame: all of the EFB.
    pub fn clear_efb(&mut self, state: &State) {
        self.clear(state, [0, 0, EFB_WIDTH, EFB_HEIGHT]);
    }

    /// Makes the EFB `scale` times the console's tall and `width` wide for its 640, between
    /// frames, as [`Renderer::with_device_efb`] would make it: a resized window's. Only what
    /// the EFB's size decides is made again: the EFB, its depth and multisampled targets, and
    /// the bind groups of the copies that read it (to textures and to the XFB, whose frames
    /// are made at each copy's size). The copies made from the old EFB are dropped, as their
    /// texels are its pixels averaged, with what they wrote to memory and the bind groups
    /// naming them, so the next copies are made from the new EFB as a new renderer's would be.
    /// Textures, pipelines (the sample count stays), samplers and buffers are kept. Clear the
    /// new EFB ([`Renderer::clear_efb`]) before drawing to it.
    pub fn resize_efb(&mut self, scale: u32, width: u32) {
        let (scale, width) = (scale.clamp(1, 8), width.max(1));
        if (scale, width) == (self.scale, self.width) {
            return;
        }
        self.flush();
        let t = EfbTargets::new(&self.device, scale, width, self.samples);
        self.copier.set_efb(&self.device, &t.efb_view);
        if let Some(x) = &mut self.gpu_xfb {
            x.set_efb(&self.device, &t.efb_view);
        }
        (self.efb, self.efb_view, self.efb_msaa, self.depth_view) =
            (t.efb, t.efb_view, t.efb_msaa, t.depth_view);
        self.known = Known::new();
        (self.scale, self.width) = (scale, width);
        for (_, c) in self.gpu_copies.drain() {
            self.textures[c.id as usize] = None;
        }
        let textures = &self.textures;
        self.texture_ids
            .retain(|_, id| textures[*id as usize].is_some());
        self.bind_groups
            .retain(|ids, _| ids.iter().all(|&id| textures[id as usize].is_some()));
        self.copies.clear();
        self.on_cpu.clear();
        self.footprints = copy::Footprints::default();
        self.hashes.clear();
    }

    /// Drops textures unused for a while.
    fn evict(&mut self) {
        let frame = self.frame;
        let mut dropped = false;
        for slot in self.textures.iter_mut().skip(1) {
            if slot.as_ref().is_some_and(|e| e.last_used + 120 < frame) {
                *slot = None;
                dropped = true;
            }
        }
        if dropped {
            let textures = &self.textures;
            self.gpu_copies
                .retain(|_, c| textures[c.id as usize].is_some());
            self.texture_ids
                .retain(|_, id| textures[*id as usize].is_some());
            self.bind_groups
                .retain(|ids, _| ids.iter().all(|&id| textures[id as usize].is_some()));
        }
    }
}

impl Sink for Renderer {
    fn draw(&mut self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        let t = self.profile.is_some().then(std::time::Instant::now);
        if draw.fetched {
            self.draw_timed(state, draw, mem);
        } else {
            let mut bytes = Vec::new();
            let count = u32::from(draw.count);
            vertex::fetch(&state.cp, draw.layout, count, draw.data, mem, &mut bytes);
            let fetched = Draw {
                data: &bytes,
                fetched: true,
                ..*draw
            };
            self.draw_timed(state, &fetched, mem);
        }
        if let (Some(p), Some(t)) = (&mut self.profile, t) {
            p.draw += t.elapsed();
        }
    }

    fn copy(&mut self, state: &State, value: u32, mem: &dyn Memory) {
        self.copy_timed(state, value, mem);
    }

    fn take_written(&mut self) -> Vec<melee_gx::Written> {
        let written = std::mem::take(&mut self.written);
        written
            .into_iter()
            .map(|(address, bytes)| melee_gx::Written {
                address,
                bytes,
                before: None,
            })
            .collect()
    }
}

/// Draws' vertices as the GPU takes them: their words (`GpuVertex::write`), and the order the
/// GPU draws them in, each the offset of a vertex's first word from its draw's first. A
/// transform thread builds a batch's draws into one, which the renderer appends to its buffers.
#[derive(Default)]
pub(crate) struct Built {
    words: Vec<u32>,
    indices: Vec<u32>,
    /// Each draw built, or none for one whose scissor leaves nothing to draw.
    pub(crate) draws: Vec<Option<BuiltDraw>>,
}

impl Built {
    pub(crate) fn clear(&mut self) {
        self.words.clear();
        self.indices.clear();
        self.draws.clear();
    }
}

/// One draw in a [`Built`].
pub(crate) struct BuiltDraw {
    /// Its words and indices in the `Built`'s.
    words: std::ops::Range<usize>,
    indices: std::ops::Range<usize>,
    /// The vertices it was given, and their texture coordinates.
    vertices: u32,
    texgens: u32,
    /// What it met that the renderer doesn't draw (exactly), and how many times.
    note: Option<(&'static str, u64)>,
    /// Its first vertex and that vertex's screen position, for tracing.
    first: Option<(Output, [f32; 3])>,
}

/// What building draws keeps from draw to draw: the normals a draw without them takes, and
/// buffers.
#[derive(Default)]
pub(crate) struct Scratch {
    pub(crate) normals: vertex::NormalCache,
    pub(crate) inputs: Vec<vertex::Input>,
    outs: Vec<Output>,
    screens: Vec<[f32; 3]>,
    clips: Vec<[f32; 4]>,
}

/// Decodes a draw's vertices and, unless its scissor leaves nothing to draw, transforms them
/// and builds them into `built`: the GPU's words for each, and its triangles (lines and points
/// as quads), back faces culled. `scratch`'s normals carry over between draws, so every draw
/// builds in order, or decodes its normals (`vertex::skip`).
pub(crate) fn build(
    state: &State,
    draw: &Draw<'_>,
    scratch: &mut Scratch,
    built: &mut Built,
) -> Option<BuiltDraw> {
    let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
    let s = scratch;
    let (vat, n) = (usize::from(draw.vat), usize::from(draw.count));
    let vp = Viewport::of(&state.xf);
    let (scissor, off) = Renderer::scissor(state, &vp);
    if scissor[2] == 0 || scissor[3] == 0 {
        // Decoded only for the normals it leaves.
        vertex::skip(
            &state.cp,
            xf_index,
            vat,
            draw.data,
            n,
            &mut s.normals,
            &mut s.inputs,
        );
        return None;
    }
    // Each vertex decoded as it's transformed.
    let decoder = vertex::Decoder::new(&state.cp, xf_index, vat);
    let mut at = 0;
    let mut v = vertex::Input::default();
    // Every vertex through the draw's transform setup, from zeroed outputs.
    let setup = xform::Setup::new(&state.xf, &state.bp);
    let tg = texgens(state);
    let words = 6 + 3 * tg as u32;
    let (w0, i0) = (built.words.len(), built.indices.len());
    let mut first = None;
    let mut note = None;
    if draw.primitive < 0xA8 {
        // A triangle primitive's vertices, written once for its triangles to name.
        s.clips.clear();
        built.words.reserve(n * words as usize);
        for _ in 0..n {
            decoder.vertex(draw.data, &mut at, &mut s.normals, &mut v);
            let mut o = Output::default();
            setup.transform(&v, &mut o);
            let screen = vp.screen(o.clip);
            Renderer::gpu_vertex(&o, screen, off, &vp).write(tg, &mut built.words);
            s.clips.push(o.clip);
            if first.is_none() {
                first = Some((o, screen));
            }
        }
        let cull = bits(state.bp[0x00], 14, 2);
        let clips = &s.clips;
        let indices = &mut built.indices;
        let mut tri = |a: usize, b: usize, c: usize| {
            let backface = xform::is_backface(&state.xf, clips[a], clips[b], clips[c]);
            let culled = if backface {
                cull == 2 || cull == 3
            } else {
                cull == 1 || cull == 3
            };
            if !culled {
                indices.extend([a, b, c].map(|i| words * i as u32));
            }
        };
        match draw.primitive {
            0x80 | 0x88 => {
                let mut i = 3;
                while i < n {
                    tri(i - 3, i - 2, i - 1);
                    tri(i - 3, i - 1, i);
                    i += 4;
                }
                if i == n {
                    tri(n - 3, n - 2, n - 1);
                }
            }
            0x90 => {
                let mut i = 2;
                while i < n {
                    tri(i - 2, i - 1, i);
                    i += 3;
                }
            }
            0x98 => {
                for i in 2..n {
                    if i % 2 == 0 {
                        tri(i - 2, i - 1, i);
                    } else {
                        tri(i - 2, i, i - 1);
                    }
                }
            }
            _ => {
                for i in 2..n {
                    tri(0, i - 1, i);
                }
            }
        }
    } else {
        s.outs.clear();
        s.screens.clear();
        for _ in 0..n {
            decoder.vertex(draw.data, &mut at, &mut s.normals, &mut v);
            let mut o = Output::default();
            setup.transform(&v, &mut o);
            s.screens.push(vp.screen(o.clip));
            s.outs.push(o);
        }
        if n > 0 {
            first = Some((s.outs[0], s.screens[0]));
        }
        let (outs, screens) = (&s.outs, &s.screens);
        if draw.primitive == 0xA8 || draw.primitive == 0xB0 {
            let pairs = if draw.primitive == 0xA8 {
                n / 2
            } else {
                n.saturating_sub(1)
            };
            for k in 0..pairs {
                let (a, b) = if draw.primitive == 0xA8 {
                    (2 * k, 2 * k + 1)
                } else {
                    (k, k + 1)
                };
                line(
                    state,
                    &vp,
                    off,
                    (&outs[a], screens[a]),
                    (&outs[b], screens[b]),
                    w0,
                    built,
                );
            }
            if bits(state.bp[0x22], 16, 3) != 0 && pairs > 0 {
                note = Some(("line texture offsets", pairs as u64));
            }
        } else {
            for i in 0..n {
                point(state, &vp, off, &outs[i], screens[i], w0, built);
            }
            if bits(state.bp[0x22], 19, 3) != 0 && n > 0 {
                note = Some(("point texture offsets", n as u64));
            }
        }
    }
    Some(BuiltDraw {
        words: w0..built.words.len(),
        indices: i0..built.indices.len(),
        vertices: n as u32,
        texgens: tg as u32,
        note,
        first,
    })
}

/// A line: a quad its width wide, its caps vertical or horizontal by its slope.
fn line(
    state: &State,
    vp: &Viewport,
    off: (f32, f32),
    (a, sa): (&Output, [f32; 3]),
    (b, sb): (&Output, [f32; 3]),
    start: usize,
    built: &mut Built,
) {
    let half = bits(state.bp[0x22], 0, 8) as f32 / 12.0;
    let (dx, dy) = (sb[0] - sa[0], sb[1] - sa[1]);
    let (px, py) = if dx.abs() > dy.abs() {
        (0.0, if dx > 0.0 { -1.0 } else { 1.0 })
    } else {
        (if dy > 0.0 { 1.0 } else { -1.0 }, 0.0)
    };
    let at = |o: &Output, s: [f32; 3], sign: f32| {
        Renderer::gpu_vertex(
            o,
            [s[0] + sign * px * half, s[1] + sign * py * half, s[2]],
            off,
            vp,
        )
    };
    let (a0, b0, b1, a1) = (
        at(a, sa, 1.0),
        at(b, sb, 1.0),
        at(b, sb, -1.0),
        at(a, sa, -1.0),
    );
    // Triangles (b1, b0, a0) and (a0, a1, b1).
    quad(
        [b1, b0, a0, a1],
        [0, 1, 2, 2, 3, 0],
        texgens(state),
        start,
        built,
    );
}

/// A point: a square its size wide.
fn point(
    state: &State,
    vp: &Viewport,
    off: (f32, f32),
    o: &Output,
    s: [f32; 3],
    start: usize,
    built: &mut Built,
) {
    let r = bits(state.bp[0x22], 8, 8) as f32 / 12.0;
    let at = |x: f32, y: f32| Renderer::gpu_vertex(o, [s[0] + x * r, s[1] + y * r, s[2]], off, vp);
    let (ll, lr, ur, ul) = (at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0));
    // Triangles (ll, ul, lr) and (ur, lr, ul).
    quad(
        [ll, ul, lr, ur],
        [0, 1, 2, 3, 2, 1],
        texgens(state),
        start,
        built,
    );
}

/// Four vertices with `texgens` texture coordinates, and two triangles naming them.
fn quad(vs: [GpuVertex; 4], order: [u32; 6], texgens: usize, start: usize, built: &mut Built) {
    let words = (6 + 3 * texgens) as u32;
    let base = (built.words.len() - start) as u32;
    for v in &vs {
        v.write(texgens, &mut built.words);
    }
    built.indices.extend(order.map(|i| base + words * i));
}

impl Renderer {
    /// Draws a draw [`build`] built into `built`, or nothing for one whose scissor leaves
    /// nothing (`None`).
    pub(crate) fn draw_built(
        &mut self,
        state: &State,
        primitive: u8,
        built: &Built,
        d: Option<&BuiltDraw>,
        mem: &dyn Memory,
    ) {
        let t = self.profile.is_some().then(std::time::Instant::now);
        self.draws_this_frame += 1;
        if let Some(d) = d {
            self.draw_outs(state, primitive, built, d, mem);
        }
        if let (Some(p), Some(t)) = (&mut self.profile, t) {
            p.draw += t.elapsed();
        }
    }

    fn draw_timed(&mut self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        self.draws_this_frame += 1;
        if self.draw_limit.is_some_and(|n| self.draws_this_frame > n) {
            return;
        }
        // The draw's vertices, decoded, transformed and built, in buffers kept from draw to draw.
        let t = self.profile.is_some().then(std::time::Instant::now);
        let mut built = std::mem::take(&mut self.built);
        built.clear();
        let d = build(state, draw, &mut self.scratch, &mut built);
        if let (Some(p), Some(t)) = (&mut self.profile, t) {
            p.xform += t.elapsed();
        }
        if let Some(d) = &d {
            self.draw_outs(state, draw.primitive, &built, d, mem);
        }
        self.built = built;
    }

    /// The rest of a draw, from its built vertices.
    fn draw_outs(
        &mut self,
        state: &State,
        primitive: u8,
        built: &Built,
        d: &BuiltDraw,
        mem: &dyn Memory,
    ) {
        let vp = Viewport::of(&state.xf);
        let (scissor, _) = Self::scissor(state, &vp);
        let tg = d.texgens as usize;
        if let Some(p) = &mut self.profile {
            p.vertices += u64::from(d.vertices);
            p.texgens += tg as u64 * u64::from(d.vertices);
        }
        if let Some((what, n)) = d.note {
            *self.unsupported.entry(what).or_default() += n;
        }
        // Draw calls queued so far go to the GPU while more come, a pass at a time.
        if self.flush_at.is_some_and(|n| self.commands.len() >= n) {
            self.flush();
        }
        let first = self.indices.len() as u32;
        let count = d.indices.len() as u32;
        if self.trace
            && self.frame == 0
            && let Some((o, s)) = &d.first
        {
            trace_draw(state, primitive, d.vertices, count, scissor, o, *s, mem);
        }
        if count == 0 {
            return;
        }
        let base = self.vertices.len() as u32;
        self.vertices
            .extend_from_slice(&built.words[d.words.clone()]);
        self.indices
            .extend(built.indices[d.indices.clone()].iter().map(|&i| base + i));

        let mut ids = [0u32; 8];
        let mut levels = [1u32; 8];
        if bits(state.bp[0x00], 16, 3) != 0 {
            self.note("indirect texturing");
        }
        for map in texture_maps(state) {
            if ids[map] == 0 {
                let (id, l) = self.texture(state, map, mem);
                ids[map] = id;
                levels[map] = l;
            }
        }
        self.bind_group(ids);

        let mut key = Self::pipeline_key(state);
        key.early_z = self.early_module.is_some() && Self::depth_follows_raster(state);
        if self.opaque {
            key.blend = None;
        }
        if self.silhouettes {
            key = PipelineKey {
                blend: Some((1, 1, false)),
                const_alpha: false,
                write_color: true,
                write_alpha: true,
                depth_compare: None,
                depth_write: false,
                early_z: false,
            };
            let u = Self::uniforms_of(state, levels, [3, 0, 0, 0]);
            let uniform = self.push_uniforms(&u);
            self.pipeline(key);
            self.push_command(Command {
                pipeline: key,
                texgens: tg as u32,
                uniform,
                textures: ids,
                first,
                count,
                scissor,
            });
            return;
        }
        let blend = state.bp[0x41];
        if bits(blend, 0, 1) == 0 && bits(blend, 1, 1) != 0 && bits(blend, 12, 4) != 3 {
            self.note("logic op");
        }
        if bits(state.bp[0x43], 0, 3) > 1 {
            self.note("EFB format other than RGB8 and RGBA6");
        }
        // A z test before texturing still updates depth where the alpha test then fails: a
        // depth pass, then color where depth now matches.
        let early = bits(state.bp[0x43], 6, 1) != 0;
        let at = state.bp[0xF3];
        let alpha_can_fail = match self.alpha_test {
            Some((last, fails)) if last == at => fails,
            _ => {
                let fails = !(0..=255).all(|a| alpha_passes(at, a));
                self.alpha_test = Some((at, fails));
                fails
            }
        };
        if early && key.depth_write && alpha_can_fail {
            let depth_key = PipelineKey {
                blend: None,
                write_color: false,
                write_alpha: false,
                ..key
            };
            let u = Self::uniforms_of(state, levels, [MODE_DEPTH_ONLY, 0, 0, 0]);
            let uniform = self.push_uniforms(&u);
            self.pipeline(depth_key);
            self.push_command(Command {
                pipeline: depth_key,
                texgens: tg as u32,
                uniform,
                textures: ids,
                first,
                count,
                scissor,
            });
            let color_key = PipelineKey {
                depth_compare: Some(2),
                depth_write: false,
                ..key
            };
            let u = Self::uniforms_of(state, levels, [MODE_DRAW, 0, 0, 0]);
            let uniform = self.push_uniforms(&u);
            self.pipeline(color_key);
            self.push_command(Command {
                pipeline: color_key,
                texgens: tg as u32,
                uniform,
                textures: ids,
                first,
                count,
                scissor,
            });
        } else {
            let u = Self::uniforms_of(state, levels, [MODE_DRAW, 0, 0, 0]);
            let uniform = self.push_uniforms(&u);
            self.pipeline(key);
            self.push_command(Command {
                pipeline: key,
                texgens: tg as u32,
                uniform,
                textures: ids,
                first,
                count,
                scissor,
            });
        }
    }

    fn copy_timed(&mut self, state: &State, value: u32, mem: &dyn Memory) {
        let tl = state.bp[0x49];
        let wh = state.bp[0x4A];
        let left = bits(tl, 0, 10);
        let top = bits(tl, 10, 10);
        let right = (left + bits(wh, 0, 10) + 1).min(EFB_WIDTH);
        let bottom = (top + bits(wh, 10, 10) + 1).min(EFB_HEIGHT);
        let t = std::time::Instant::now();
        self.flush();
        if let Some(d) = &mut self.digest {
            d.add(u64::from(value) << 32 ^ hash_bytes(bytemuck::cast_slice(&state.bp[0x49..0x55])));
        }
        let t1 = std::time::Instant::now();
        if bits(value, 14, 1) != 0 {
            // A frame's end: copies the game has since written over are gone.
            if self.track_copies {
                for (start, end) in self.footprints.take_overwritten(mem) {
                    self.forget(start, end);
                }
                self.footprints.new_frame();
            }
            let rect = [left, top, right, bottom];
            let (frame, t2) = if self.xfb_on_gpu {
                let gpu = self.gpu_xfb.get_or_insert_with(|| {
                    let mut x = xfb::GpuXfb::new(&self.device, &self.queue, &self.efb_view);
                    x.timer = self.timer.clone();
                    x
                });
                let (texture, width, height) = gpu.copy(
                    &self.device,
                    &self.queue,
                    state,
                    value,
                    rect,
                    (self.scale, self.width),
                );
                let t2 = std::time::Instant::now();
                let rgba = if self.xfb_readback {
                    self.read_texture(&texture, width, height)
                } else {
                    Vec::new()
                };
                let frame = Frame {
                    width,
                    height,
                    rgba,
                    texture: Some(texture),
                };
                (frame, t2)
            } else {
                let efb = self.read_efb();
                let t2 = std::time::Instant::now();
                (xfb::copy(state, value, &efb, rect), t2)
            };
            if let Some(p) = &mut self.profile {
                p.flush += t1 - t;
                p.readback += t2 - t1;
                p.xfb += t2.elapsed();
                p.report();
            }
            match &mut self.on_frame {
                Some(f) => f(frame),
                None => self.frames.push(frame),
            }
            if let Some(d) = &mut self.digest {
                d.frame(self.frame);
            }
            self.frame += 1;
            self.draws_this_frame = 0;
            self.hashes.clear();
            self.evict();
        } else if let Some(request) = copy::request(state, value)
            && !self.on_cpu.contains(&request.footprint.address)
        {
            self.gpu_copy(&request);
            if self.track_copies {
                self.footprints.record(request.footprint, mem);
            }
        } else {
            let efb = self.read_efb();
            match encode::encode(state, value, &efb) {
                Some(e) => {
                    let end = e.address + e.rows.len() as u32 * e.stride;
                    self.gpu_copies
                        .retain(|&a, _| !(e.address..end).contains(&a));
                    for (i, row) in e.rows.into_iter().enumerate() {
                        let at = (e.address + i as u32 * e.stride) & PHYS_MASK;
                        let end = at as u64 + row.len() as u64;
                        self.copies.retain(|(a, b)| {
                            !(*a as u64 >= at as u64 && *a as u64 + b.len() as u64 <= end)
                        });
                        self.written.push((at, row.clone()));
                        self.copies.push((at, row));
                    }
                    self.hashes.clear();
                }
                None => self.note("EFB copy to a texture in a format not encoded"),
            }
            if self.track_copies
                && let Some(f) = copy::footprint(state, value)
            {
                self.footprints.record(f, mem);
            }
        }
        if bits(value, 11, 1) != 0 {
            self.clear(
                state,
                [
                    left,
                    top,
                    right.saturating_sub(left),
                    bottom.saturating_sub(top),
                ],
            );
        }
    }
}

fn alpha_passes(at: u32, a: u32) -> bool {
    let cmp = |r: u32, f: u32| match f {
        0 => false,
        1 => a < r,
        2 => a == r,
        3 => a <= r,
        4 => a > r,
        5 => a != r,
        6 => a >= r,
        _ => true,
    };
    let c0 = cmp(bits(at, 0, 8), bits(at, 16, 3));
    let c1 = cmp(bits(at, 8, 8), bits(at, 19, 3));
    match bits(at, 22, 2) {
        0 => c0 && c1,
        1 => c0 || c1,
        2 => c0 != c1,
        _ => c0 == c1,
    }
}

impl Renderer {
    /// Vertices drawn in the order given, with `texgens` texture coordinates.
    fn push_in_order(&mut self, vs: &[GpuVertex], texgens: usize) {
        for v in vs {
            self.indices.push(self.vertices.len() as u32);
            v.write(texgens, &mut self.vertices);
        }
    }
}

/// Prints a draw and the state it's drawn with, for debugging (`trace`).
#[allow(clippy::too_many_arguments)]
fn trace_draw(
    state: &State,
    primitive: u8,
    vertices: u32,
    emitted: u32,
    scissor: [u32; 4],
    first: &Output,
    screen: [f32; 3],
    mem: &dyn Memory,
) {
    let bp = &state.bp;
    let stages = bits(bp[0], 10, 4) as usize + 1;
    eprintln!(
        "draw {:02X} vertices {} emitted {emitted} scissor {scissor:?} clip {:?} screen {screen:?} color {:?}",
        primitive, vertices, first.clip, first.color
    );
    eprintln!(
        "  genmode {:06x} zmode {:06x} blend {:06x} pe {:06x} alpha {:06x} ztex {:06x} {:06x} fog {:06x?}",
        bp[0x00],
        bp[0x40],
        bp[0x41],
        bp[0x43],
        bp[0xF3],
        bp[0xF4],
        bp[0xF5],
        &bp[0xEE..0xF3]
    );
    eprintln!(
        "  env {:06x?} orders {:06x?} ksel {:06x?} ind {:06x?}",
        &bp[0xC0..0xC0 + 2 * stages],
        &bp[0x28..0x28 + stages.div_ceil(2)],
        &bp[0xF6..0xFE],
        &bp[0x10..0x10 + stages]
    );
    eprintln!(
        "  colors {:06x?} {:06x?} konsts {:06x?} {:06x?}",
        state.color_ra, state.color_bg, state.konst_ra, state.konst_bg
    );
    let img = Image::of(bp, 0);
    let mut head = [0u8; 16];
    mem.read(img.address, &mut head);
    eprintln!(
        "  map 0: format {} {}x{} levels {} at {:08x} {:02x?}, tex 0 {:?}",
        img.format, img.width, img.height, img.levels, img.address, head, first.tex[0]
    );
}
