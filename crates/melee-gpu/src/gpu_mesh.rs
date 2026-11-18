// SPDX-License-Identifier: GPL-3.0-or-later

//! The fast renderer's transforms on the GPU (docs/fast-renderer.md, F3): a mesh's vertices and
//! its elements' indices kept on the GPU from frame to frame, and each draw of it a record (its
//! mesh, its matrix indices, the normals the draws before left) and the draw's XF state, which
//! a compute pass (xform.wgsl) runs through the transform unit into the vertices the draw's
//! vertex shader reads (gx.wgsl's `vs_mesh`). Lines and points become quads there.
//!
//! The XF state goes up in four sections (the position, texture and normal matrices; the
//! post-transform matrices; the lights; the registers), each sent once a flush however many
//! draws in a row share it. The meshes' places in the GPU's pools stay while a frame is drawn:
//! a pool grows by copying itself on the GPU, and closes its gaps only between frames.

use std::ops::Range;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use melee_gx::vertex::{self, NormalCache};
use melee_gx::{State, xform};

use crate::bits;
use crate::mesh::{Kind, Mesh};

/// A draw's record, in words (xform.wgsl's `REC` and `R_`).
const REC: usize = 40;
const R_IN: usize = 0;
const R_TRI: usize = 1;
const R_STRIDE: usize = 2;
const R_FORMAT: usize = 3;
const R_VCD: usize = 4;
const R_DEFAULTS: usize = 5;
const R_OUT: usize = 8;
const R_TEXGENS: usize = 9;
const R_XF: usize = 10;
const R_OFF: usize = 14;
const R_NORMALS: usize = 16;
const R_SIZES: usize = 25;
const R_SCALES: usize = 26;
const R_LP: usize = 34;
const R_LP_COUNT: usize = 35;

/// Items (vertices, or lines' and points' quads) a workgroup transforms.
const GROUP: u32 = 64;

/// Frames a mesh no draw used is kept on the GPU (as the mesh cache keeps it).
const KEPT_FOR: u64 = 600;

/// The XF state's sections: the ranges of XF memory each holds, read from the record's offsets.
/// Position and texture matrices (to 0x110: matrix index 63's last row), with the normal
/// matrices (to 0x468: index 31's) after them; the post-transform matrices (to 0x610); the
/// lights; and the registers.
const MATRICES: [Range<usize>; 2] = [0x000..0x110, 0x400..0x468];
const POST: Range<usize> = 0x500..0x610;
const LIGHTS: Range<usize> = 0x600..0x680;
const REGS: Range<usize> = 0x1000..0x1058;

/// Words in one GPU buffer, each region kept at its place while the buffer grows.
pub(crate) struct Pool {
    pub(crate) buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
    label: &'static str,
    /// Words given out.
    used: u64,
}

impl Pool {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Pool {
        let usage = usage | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        Pool {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 1 << 20,
                usage,
                mapped_at_creation: false,
            }),
            usage,
            label,
            used: 0,
        }
    }

    /// Room for `words` more: their first word's place, the buffer grown (its words copied on
    /// the GPU, after the writes already queued) if they don't fit.
    fn alloc(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, words: u64) -> u64 {
        let at = self.used;
        let need = (at + words) * 4;
        if need > self.buffer.size() {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: need.next_power_of_two(),
                usage: self.usage,
                mapped_at_creation: false,
            });
            if at > 0 {
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(&self.buffer, 0, &buffer, 0, at * 4);
                queue.submit([encoder.finish()]);
            }
            self.buffer = buffer;
        }
        self.used += words;
        at
    }
}

/// A mesh on the GPU.
struct Resident {
    mesh: Arc<Mesh>,
    /// Its vertices' first word in the arena, `stride` words each, then its lines' and points'
    /// elements, two words each (a point's second is all ones).
    data: u32,
    stride: u32,
    /// Vertices transformed as vertices: all of them where it has triangles, else none.
    tri: u32,
    lp_count: u32,
    /// Each run's indices in the index pool: triangles name their vertices, lines' and points'
    /// quads the four vertices after them (`tri + 4e` on) each element makes.
    runs: Vec<(Kind, Range<u32>)>,
    /// Words it takes in the arena and the index pool.
    words: u64,
    indices: u64,
    used: u64,
}

impl Resident {
    /// Its vertices' words and its indices.
    #[allow(clippy::type_complexity)]
    fn encode(mesh: &Mesh) -> (u32, u32, u32, Vec<u32>, Vec<u32>, Vec<(Kind, Range<u32>)>) {
        let (nc, tc) = (mesh.normal_count, mesh.tex_count);
        let carries = !mesh.matrices.is_empty();
        let stride = 5 + 3 * nc + 2 * tc + if carries { 3 } else { 0 };
        let triangles = mesh.runs.iter().any(|r| r.0 == Kind::Triangles);
        let mut words = Vec::with_capacity(mesh.len() * stride);
        for v in 0..mesh.len() {
            words.extend(mesh.positions[v].map(f32::to_bits));
            for n in &mesh.normals[v * nc..(v + 1) * nc] {
                words.extend(n.map(f32::to_bits));
            }
            words.extend(mesh.colors[v].map(u32::from_le_bytes));
            for t in &mesh.tex[v * tc..(v + 1) * tc] {
                words.extend(t.map(f32::to_bits));
            }
            if let Some(m) = mesh.matrices.get(v) {
                words.extend([
                    u32::from_le_bytes([m[0], m[1], m[2], m[3]]),
                    u32::from_le_bytes([m[4], m[5], m[6], m[7]]),
                    u32::from(m[8]),
                ]);
            }
        }
        let tri = if triangles { mesh.len() as u32 } else { 0 };
        let mut indices = Vec::new();
        let mut runs = Vec::new();
        let mut e = 0u32;
        for (kind, run) in &mesh.runs {
            let start = indices.len() as u32;
            let els = &mesh.elements[run.start as usize..run.end as usize];
            match kind {
                Kind::Triangles => indices.extend_from_slice(els),
                Kind::Lines | Kind::Points => {
                    for el in els.chunks_exact(kind.per()) {
                        let (a, b) = match el {
                            [a, b] => (*a, *b),
                            _ => (el[0], u32::MAX),
                        };
                        words.extend([a, b]);
                        let base = tri + 4 * e;
                        // Lines' (b1, b0, a0, a1) and points' (ll, ul, lr, ur), as `line` and
                        // `point` order their quads' triangles.
                        let order = if *kind == Kind::Lines {
                            [0, 1, 2, 2, 3, 0]
                        } else {
                            [0, 1, 2, 3, 2, 1]
                        };
                        indices.extend(order.map(|k| base + k));
                        e += 1;
                    }
                }
            }
            runs.push((*kind, start..indices.len() as u32));
        }
        (stride as u32, tri, e, words, indices, runs)
    }
}

/// One flush's draws: their records, the XF sections they read, and the compute pass's work.
#[derive(Default)]
struct Batch {
    records: Vec<u32>,
    xf: Vec<u32>,
    /// Each workgroup's record and first item.
    work: Vec<[u32; 2]>,
    /// The last of each section sent, by its place in `xf`.
    last: [Option<(u32, u32)>; 4],
    /// The output's words so far.
    out_words: u64,
}

impl Batch {
    fn clear(&mut self) {
        self.records.clear();
        self.xf.clear();
        self.work.clear();
        self.last = [None; 4];
        self.out_words = 0;
    }

    /// Section `k` made of XF memory's `ranges`: where it is in `xf`, sent unless the last one
    /// sent is the same.
    fn section(&mut self, k: usize, xf: &[u32], ranges: &[Range<usize>]) -> u32 {
        let len: usize = ranges.iter().map(|r| r.len()).sum();
        if let Some((at, n)) = self.last[k]
            && n as usize == len
        {
            let mut from = at as usize;
            let same = ranges.iter().all(|r| {
                let s = self.xf[from..from + r.len()] == xf[r.clone()];
                from += r.len();
                s
            });
            if same {
                return at;
            }
        }
        let at = self.xf.len() as u32;
        for r in ranges {
            self.xf.extend_from_slice(&xf[r.clone()]);
        }
        self.last[k] = Some((at, len as u32));
        at
    }

    /// Adds a draw of `res` with `state`, the draws before having left `normals`, its words
    /// from vertex `out` of the output (or, `clips`, its vertices' clip-space positions there).
    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        state: &State,
        res: &Resident,
        normals: &NormalCache,
        off: (f32, f32),
        texgens: u32,
        out: u32,
        clips: bool,
    ) {
        let xf = &state.xf;
        let mesh = &res.mesh;
        let mut r = [0u32; REC];
        let (tri, lp) = if clips {
            (mesh.len() as u32, 0)
        } else {
            (res.tri, res.lp_count)
        };
        r[R_IN] = res.data;
        r[R_TRI] = tri;
        r[R_STRIDE] = res.stride;
        r[R_FORMAT] = mesh.normal_count as u32
            | (mesh.tex_count as u32) << 2
            | u32::from(!mesh.matrices.is_empty()) << 6
            | u32::from(clips) << 7;
        r[R_VCD] = mesh.vcd;
        let (p, t) = vertex::default_indices([xf[xform::MATINDEX_A], xf[xform::MATINDEX_A + 1]]);
        r[R_DEFAULTS] = u32::from_le_bytes([p, t[0], t[1], t[2]]);
        r[R_DEFAULTS + 1] = u32::from_le_bytes([t[3], t[4], t[5], t[6]]);
        r[R_DEFAULTS + 2] = u32::from(t[7]);
        r[R_OUT] = out;
        r[R_TEXGENS] = texgens;
        r[R_XF] = self.section(0, xf, &MATRICES);
        r[R_XF + 1] = if xf[xform::DUAL_TEX] & 1 != 0 {
            self.section(1, xf, std::slice::from_ref(&POST))
        } else {
            0
        };
        r[R_XF + 2] = self.section(2, xf, std::slice::from_ref(&LIGHTS));
        r[R_XF + 3] = self.section(3, xf, std::slice::from_ref(&REGS));
        r[R_OFF] = off.0.to_bits();
        r[R_OFF + 1] = off.1.to_bits();
        for (w, v) in r[R_NORMALS..R_NORMALS + 9]
            .iter_mut()
            .zip(normals.as_flattened())
        {
            *w = v.to_bits();
        }
        r[R_SIZES] = state.bp[0x22];
        for n in 0..8 {
            let s = state.bp[xform::SU_SSIZE + 2 * n] & 0xFFFF;
            let t = state.bp[xform::SU_SSIZE + 2 * n + 1] & 0xFFFF;
            r[R_SCALES + n] = s | t << 16;
        }
        r[R_LP] = res.data + res.stride * mesh.len() as u32;
        r[R_LP_COUNT] = lp;
        let index = (self.records.len() / REC) as u32;
        self.records.extend_from_slice(&r);
        for first in (0..tri + lp).step_by(GROUP as usize) {
            self.work.push([index, first]);
        }
    }
}

/// The meshes on the GPU, the transforms' pipeline and a flush's draws.
pub(crate) struct GpuMeshes {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    arena: Pool,
    /// The elements' indices, which the draws are drawn with.
    pub(crate) index: Pool,
    /// By the mesh's address, while it holds the mesh.
    resident: FxHashMap<usize, Resident>,
    /// Words of the pools no mesh holds any more.
    dead: (u64, u64),
    batch: Batch,
    records: Option<wgpu::Buffer>,
    xf: Option<wgpu::Buffer>,
    work: Option<wgpu::Buffer>,
    out: Option<wgpu::Buffer>,
    /// The output, as the draws' vertex shader reads it (group 2).
    pub(crate) out_group: Option<wgpu::BindGroup>,
    /// Meshes uploaded, and words of the XF state sent, since asked (for the profile).
    pub(crate) uploaded: u64,
}

/// A buffer of at least `size` bytes in `slot`, made again (twice as large) if it's smaller;
/// whether it was.
fn ensure(
    slot: &mut Option<wgpu::Buffer>,
    device: &wgpu::Device,
    label: &'static str,
    usage: wgpu::BufferUsages,
    size: u64,
) -> bool {
    let size = size.max(16);
    if slot.as_ref().is_some_and(|b| b.size() >= size) {
        return false;
    }
    *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.next_power_of_two(),
        usage,
        mapped_at_creation: false,
    }));
    true
}

fn storage(read_only: bool, binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl GpuMeshes {
    pub(crate) fn new(device: &wgpu::Device, checks: wgpu::ShaderRuntimeChecks) -> GpuMeshes {
        // SAFETY: every loop in xform.wgsl is bounded by a constant.
        let module = unsafe {
            device.create_shader_module_trusted(
                wgpu::ShaderModuleDescriptor {
                    label: Some("xform"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("xform.wgsl").into()),
                },
                checks,
            )
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("xform"),
            entries: &[
                storage(true, 0),
                storage(true, 1),
                storage(true, 2),
                storage(true, 3),
                storage(false, 4),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("xform"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("xform"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        GpuMeshes {
            pipeline,
            layout,
            arena: Pool::new(device, "mesh vertices", wgpu::BufferUsages::STORAGE),
            index: Pool::new(device, "mesh indices", wgpu::BufferUsages::INDEX),
            resident: FxHashMap::default(),
            dead: (0, 0),
            batch: Batch::default(),
            records: None,
            xf: None,
            work: None,
            out: None,
            out_group: None,
            uploaded: 0,
        }
    }

    /// The mesh on the GPU, uploaded if it isn't; marked used in `frame`.
    fn resident(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &Arc<Mesh>,
        frame: u64,
    ) -> &Resident {
        let key = Arc::as_ptr(mesh) as usize;
        let (arena, index) = (&mut self.arena, &mut self.index);
        let uploaded = &mut self.uploaded;
        let res = self.resident.entry(key).or_insert_with(|| {
            *uploaded += 1;
            Self::upload(device, queue, arena, index, mesh.clone())
        });
        res.used = frame;
        res
    }

    fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arena: &mut Pool,
        index: &mut Pool,
        mesh: Arc<Mesh>,
    ) -> Resident {
        let (stride, tri, lp_count, words, indices, mut runs) = Resident::encode(&mesh);
        let data = arena.alloc(device, queue, words.len() as u64);
        if !words.is_empty() {
            queue.write_buffer(&arena.buffer, data * 4, bytemuck::cast_slice(&words));
        }
        let first = index.alloc(device, queue, indices.len() as u64);
        if !indices.is_empty() {
            queue.write_buffer(&index.buffer, first * 4, bytemuck::cast_slice(&indices));
        }
        for (_, r) in &mut runs {
            *r = r.start + first as u32..r.end + first as u32;
        }
        Resident {
            mesh,
            data: data as u32,
            stride,
            tri,
            lp_count,
            runs,
            words: words.len() as u64,
            indices: indices.len() as u64,
            used: 0,
        }
    }

    /// Adds a draw of `mesh` with `state` (the draws before having left `normals`, its scissor's
    /// offset `off`, `texgens` texture coordinates) to the flush; puts its runs in the index
    /// pool in `runs`. Its vertices' words come out from the vertex this returns.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        state: &State,
        mesh: &Arc<Mesh>,
        normals: &NormalCache,
        off: (f32, f32),
        texgens: u32,
        frame: u64,
        runs: &mut Vec<(Kind, Range<u32>)>,
    ) -> u32 {
        let stride = u64::from(6 + 3 * texgens);
        let first = self.batch.out_words.div_ceil(stride);
        let res = self.resident(device, queue, mesh, frame);
        let count = u64::from(res.tri + 4 * res.lp_count);
        runs.clear();
        runs.extend(res.runs.iter().cloned());
        let res = &self.resident[&(Arc::as_ptr(mesh) as usize)];
        self.batch
            .record(state, res, normals, off, texgens, first as u32, false);
        self.batch.out_words = (first + count) * stride;
        first as u32
    }

    /// Runs the flush's transforms in `encoder`, before its draws: their records, XF state and
    /// work uploaded, the output made large enough.
    pub(crate) fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertex_layout: &wgpu::BindGroupLayout,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.batch.records.is_empty() {
            return;
        }
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let b = &mut self.batch;
        let n = b.work.len() as u32;
        b.work.insert(0, [n, 0]);
        ensure(
            &mut self.records,
            device,
            "mesh records",
            storage,
            (b.records.len() * 4) as u64,
        );
        ensure(
            &mut self.xf,
            device,
            "mesh xf",
            storage,
            (b.xf.len() * 4) as u64,
        );
        ensure(
            &mut self.work,
            device,
            "mesh work",
            storage,
            (b.work.len() * 8) as u64,
        );
        if ensure(
            &mut self.out,
            device,
            "mesh vertices out",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            b.out_words * 4,
        ) || self.out_group.is_none()
        {
            self.out_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mesh vertices out"),
                layout: vertex_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.out.as_ref().expect("the output").as_entire_binding(),
                }],
            }));
        }
        let (records, xf, work, out) = (
            self.records.as_ref().expect("records"),
            self.xf.as_ref().expect("xf"),
            self.work.as_ref().expect("work"),
            self.out.as_ref().expect("the output"),
        );
        queue.write_buffer(records, 0, bytemuck::cast_slice(&b.records));
        if !b.xf.is_empty() {
            queue.write_buffer(xf, 0, bytemuck::cast_slice(&b.xf));
        }
        queue.write_buffer(work, 0, bytemuck::cast_slice(&b.work));
        let group = self.bind_group(device, [records, xf, work, out]);
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(n.min(65535), n.div_ceil(65535), 1);
        }
        self.batch.clear();
    }

    /// Forgets a flush's draws without running them (one whose draws all drew nothing).
    pub(crate) fn drop_batch(&mut self) {
        self.batch.clear();
    }

    fn bind_group(&self, device: &wgpu::Device, buffers: [&wgpu::Buffer; 4]) -> wgpu::BindGroup {
        let [records, xf, work, out] = buffers;
        fn entry(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            }
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("xform"),
            layout: &self.layout,
            entries: &[
                entry(0, &self.arena.buffer),
                entry(1, records),
                entry(2, xf),
                entry(3, work),
                entry(4, out),
            ],
        })
    }

    /// Between frames: meshes no draw used for a while go, and the pools close their gaps once
    /// they're half gaps, every mesh kept uploaded again in order.
    pub(crate) fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: u64) {
        if !self.batch.records.is_empty() || !frame.is_multiple_of(60) {
            return;
        }
        let dead = &mut self.dead;
        self.resident.retain(|_, r| {
            let keep = frame.saturating_sub(r.used) < KEPT_FOR;
            if !keep {
                dead.0 += r.words;
                dead.1 += r.indices;
            }
            keep
        });
        if self.dead.0 * 2 <= self.arena.used && self.dead.1 * 2 <= self.index.used {
            return;
        }
        self.arena.used = 0;
        self.index.used = 0;
        self.dead = (0, 0);
        let old: Vec<(usize, Resident)> = self.resident.drain().collect();
        for (key, r) in old {
            let mut new = Self::upload(device, queue, &mut self.arena, &mut self.index, r.mesh);
            new.used = r.used;
            self.resident.insert(key, new);
        }
    }

    /// The mesh's vertices' clip-space positions as the transforms on the GPU make them, drawn
    /// with `state` and the draws before having left `normals`: for the harness's geometry.
    pub(crate) fn clips(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        state: &State,
        mesh: &Arc<Mesh>,
        normals: &NormalCache,
        frame: u64,
    ) -> Vec<[f32; 4]> {
        if mesh.is_empty() {
            return Vec::new();
        }
        self.resident(device, queue, mesh, frame);
        let res = &self.resident[&(Arc::as_ptr(mesh) as usize)];
        let mut b = Batch::default();
        b.record(state, res, normals, (0.0, 0.0), 0, 0, true);
        let n = b.work.len() as u32;
        b.work.insert(0, [n, 0]);
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let make = |label, usage, size: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage,
                mapped_at_creation: false,
            })
        };
        let records = make("clip records", storage, (b.records.len() * 4) as u64);
        let xf = make("clip xf", storage, (b.xf.len() * 4) as u64);
        let work = make("clip work", storage, (b.work.len() * 8) as u64);
        let size = mesh.len() as u64 * 16;
        let out = make(
            "clips",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            size,
        );
        let read = make(
            "clips readback",
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            size,
        );
        queue.write_buffer(&records, 0, bytemuck::cast_slice(&b.records));
        queue.write_buffer(&xf, 0, bytemuck::cast_slice(&b.xf));
        queue.write_buffer(&work, 0, bytemuck::cast_slice(&b.work));
        let group = self.bind_group(device, [&records, &xf, &work, &out]);
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(n.min(65535), n.div_ceil(65535), 1);
        }
        encoder.copy_buffer_to_buffer(&out, 0, &read, 0, size);
        queue.submit([encoder.finish()]);
        let slice = read.slice(..size);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU readback");
        let data = slice.get_mapped_range().expect("mapped readback");
        let words: &[u32] = bytemuck::cast_slice(&data);
        let clips = words
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| c.map(f32::from_bits))
            .collect();
        drop(data);
        read.unmap();
        clips
    }
}

#[cfg(test)]
impl GpuMeshes {
    /// A draw's output words as the transforms write them, read back: `(6 + 3 texgens)` a
    /// vertex, its triangles' vertices and then its lines' and points' quads.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn words(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertex_layout: &wgpu::BindGroupLayout,
        state: &State,
        mesh: &Arc<Mesh>,
        normals: &NormalCache,
        off: (f32, f32),
    ) -> Vec<u32> {
        let tg = crate::texgens(state) as u32;
        let mut runs = Vec::new();
        let first = self.draw(device, queue, state, mesh, normals, off, tg, 0, &mut runs);
        let start = u64::from(first) * u64::from(6 + 3 * tg) * 4;
        let size = self.batch.out_words * 4 - start;
        let mut encoder = device.create_command_encoder(&Default::default());
        self.flush(device, queue, vertex_layout, &mut encoder);
        if size == 0 {
            return Vec::new();
        }
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("words readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(
            self.out.as_ref().expect("the output"),
            start,
            &read,
            0,
            size,
        );
        queue.submit([encoder.finish()]);
        let slice = read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU readback");
        let words = bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped")).to_vec();
        read.unmap();
        words
    }
}

/// GX's cull mode for a draw's triangles (BP's gen mode): none, front, back or all.
pub(crate) fn cull_mode(state: &State) -> u32 {
    bits(state.bp[0x00], 14, 2)
}
