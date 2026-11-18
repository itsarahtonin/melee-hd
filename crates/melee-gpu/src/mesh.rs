// SPDX-License-Identifier: GPL-3.0-or-later

//! The fast renderer's meshes (docs/fast-renderer.md, F2): a display list of draws, its
//! vertices fetched and decoded once ([`Mesh::decode`]) rather than every frame. Stream
//! vertices whose attributes are the same bytes share one vertex, and the primitives are
//! expanded once into elements naming them, in the list's order and GX's (quads as (0, 1, 2)
//! and (0, 2, 3), strips alternating their winding, fans around their first vertex, line
//! strips as their segments).
//!
//! What a draw's state decides stays out of the mesh and comes at the draw ([`build`]): the
//! matrix indices a vertex doesn't carry (XF's matrix index registers), the normals and
//! binormals a format without them takes from the draws before (`vertex::NormalCache`), and
//! everything the transform unit does. Each vertex goes through the transform as the exact
//! renderer's do (`xform::Setup::transform` on a `vertex::Input` decoded from the same bytes),
//! so a mesh draws what its display list draws, bit for bit.
//!
//! The decoded attributes are plain arrays, one per attribute (positions, normals, colors,
//! texture coordinates, matrix indices), laid out as a GPU buffer takes them: the transforms
//! still run on the CPU here, and moving them to the GPU (F3) reads the same arrays.

use std::ops::Range;

use rustc_hash::FxHashMap;

use melee_gx::vertex::{self, Input, NormalCache};
use melee_gx::xform::{self, Output, Viewport};
use melee_gx::{Memory, State};

use crate::{BuiltDraw, Renderer, Scratch, bits, line, texgens};

/// What a run of a mesh's elements draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Three vertex numbers an element.
    Triangles,
    /// Two.
    Lines,
    /// One.
    Points,
}

impl Kind {
    pub fn per(self) -> usize {
        match self {
            Kind::Triangles => 3,
            Kind::Lines => 2,
            Kind::Points => 1,
        }
    }
}

/// A display list's draws with their vertices decoded.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// The vertex descriptor's low word (`VCD_LO`): which matrix indices the vertices carry.
    vcd: u32,
    /// Normals a vertex carries: 1, 3 (with binormal and tangent), or 0.
    normal_count: usize,
    /// Texture coordinates kept a vertex: up to the last the format carries.
    tex_count: usize,
    pub positions: Vec<[f32; 3]>,
    /// `normal_count` a vertex.
    pub normals: Vec<[f32; 3]>,
    /// Color channels 0 and 1, RGBA, as the decoder routes them (white where absent).
    pub colors: Vec<[[u8; 4]; 2]>,
    /// `tex_count` a vertex.
    pub tex: Vec<[f32; 2]>,
    /// The position matrix index and the eight texture matrix indices, where the format
    /// carries any (empty otherwise).
    pub matrices: Vec<[u8; 9]>,
    /// Vertex numbers, [`Kind::per`] an element, in the list's order.
    pub elements: Vec<u32>,
    /// The runs of `elements` each primitive (or several of a kind in a row) makes.
    pub runs: Vec<(Kind, Range<u32>)>,
    /// The normals the list's last vertex leaves (the first `normal_count`), if it sends any.
    last_normals: Option<NormalCache>,
    /// The vertices the list sends, before sharing.
    pub stream_vertices: u32,
    /// How far into each indexed attribute's array (position, normal, the two colors, the
    /// eight texture coordinates: `vertex::ARRAY_POS` on) the list's indices reach, in bytes.
    pub reach: [u32; 12],
}

/// The vertex format the display list's primitives name: only format 0, which HSD's shapes
/// use, so a mesh's key need hold no other.
const VAT: u8 = 0;

impl Mesh {
    /// Decodes display list `list` (as the GPU reads it) with the vertex descriptor and formats
    /// in `cp`, its indexed attributes from their arrays in `mem`. None for a list a mesh can't
    /// stand for: anything but draws in vertex format 0 and no-ops (state changes, another
    /// list).
    pub fn decode(cp: &[u32; 256], list: &[u8], mem: &dyn Memory) -> Option<Mesh> {
        let lo = cp[vertex::VCD_LO as usize];
        let a = cp[vertex::VAT_A as usize + usize::from(VAT)];
        let has_nrm = bits(lo, 11, 2) != 0;
        let normal_count = match (has_nrm, bits(a, 9, 1) != 0) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 3,
        };
        let hi = cp[vertex::VCD_HI as usize];
        let tex_count = (0..8)
            .rev()
            .find(|&i| bits(hi, 2 * i, 2) != 0)
            .map_or(0, |i| i as usize + 1);
        let carries_matrices = bits(lo, 0, 9) != 0;
        let mut mesh = Mesh {
            vcd: lo,
            normal_count,
            tex_count,
            ..Mesh::default()
        };
        let layout = vertex::layout(cp, usize::from(VAT));
        let stream_size = vertex::stream_size(&layout) as usize;
        let decoder = vertex::Decoder::new(cp, [0, 0], usize::from(VAT));
        let mut shared: FxHashMap<Vec<u8>, u32> = FxHashMap::default();
        let mut fetched = Vec::new();
        let mut numbers = Vec::new();
        let mut normals: NormalCache = [[0.0; 3]; 3];
        let mut at = 0;
        while at < list.len() {
            let op = list[at];
            match op {
                0x00 | 0x44 | 0x48 => {
                    at += 1;
                    continue;
                }
                0x80..=0xBF if op & 7 == VAT => {}
                _ => return None,
            }
            let count = usize::from(u16::from_be_bytes([*list.get(at + 1)?, *list.get(at + 2)?]));
            let len = 3 + count * stream_size;
            let stream = list.get(at + 3..at + len)?;
            vertex::reach(cp, &layout, count as u32, stream, |i, _, len| {
                let r = &mut mesh.reach[layout[i].array];
                *r = (*r).max(len);
            });
            fetched.clear();
            vertex::fetch(cp, &layout, count as u32, stream, mem, &mut fetched);
            let size = fetched.len().checked_div(count).unwrap_or(0);
            numbers.clear();
            let mut from = 0;
            for k in 0..count {
                let bytes = &fetched[k * size..(k + 1) * size];
                let mut v = Input::default();
                decoder.vertex(&fetched, &mut from, &mut normals, &mut v);
                if k == count - 1 && normal_count > 0 {
                    mesh.last_normals = Some(normals);
                }
                let n = match shared.get(bytes) {
                    Some(&n) => n,
                    None => {
                        let n = mesh.positions.len() as u32;
                        shared.insert(bytes.to_vec(), n);
                        mesh.push(&v, carries_matrices);
                        n
                    }
                };
                numbers.push(n);
            }
            mesh.stream_vertices += count as u32;
            let start = mesh.elements.len() as u32;
            let els = &mut mesh.elements;
            let kind = expand(op & 0xF8, numbers.len(), |e| {
                els.extend(e.iter().map(|&i| numbers[i]));
            });
            let end = mesh.elements.len() as u32;
            match mesh.runs.last_mut() {
                Some((k, r)) if *k == kind && r.end == start => r.end = end,
                _ if start == end => {}
                _ => mesh.runs.push((kind, start..end)),
            }
            at += len;
        }
        Some(mesh)
    }

    /// The mesh in parts of about `limit` vertices each, whole elements in the list's order, so
    /// the renderer's threads can share a large one out: drawn one after another, the parts
    /// draw what the mesh does. Every part leaves the normals the whole list does.
    pub fn split(self, limit: usize) -> Vec<Mesh> {
        if self.len() <= limit {
            return vec![self];
        }
        let empty = || Mesh {
            vcd: self.vcd,
            normal_count: self.normal_count,
            tex_count: self.tex_count,
            last_normals: self.last_normals,
            ..Mesh::default()
        };
        let mut parts = vec![empty()];
        // Each vertex's number in the part being made, or none.
        let mut local = vec![u32::MAX; self.len()];
        // The vertices of the whole mesh the part being made has.
        let mut taken: Vec<usize> = Vec::new();
        for (kind, run) in &self.runs {
            let per = kind.per();
            for e in self.elements[run.start as usize..run.end as usize].chunks_exact(per) {
                let mut part = parts.last_mut().expect("a part");
                if part.len() + per > limit && !part.elements.is_empty() {
                    for v in taken.drain(..) {
                        local[v] = u32::MAX;
                    }
                    parts.push(empty());
                    part = parts.last_mut().expect("a part");
                }
                let start = part.elements.len() as u32;
                for &v in e {
                    let v = v as usize;
                    if local[v] == u32::MAX {
                        local[v] = part.len() as u32;
                        taken.push(v);
                        part.copy_vertex(&self, v);
                    }
                    part.elements.push(local[v]);
                }
                let end = part.elements.len() as u32;
                match part.runs.last_mut() {
                    Some((k, r)) if k == kind && r.end == start => r.end = end,
                    _ => part.runs.push((*kind, start..end)),
                }
            }
        }
        // Each part's own vertices and reach: the stream's count and the arrays' reach stay
        // the whole list's, for the first part.
        let (stream, reach) = (self.stream_vertices, self.reach);
        parts[0].stream_vertices = stream;
        for p in &mut parts {
            p.reach = reach;
        }
        parts
    }

    /// Adds vertex `v` of `from` as this part's next.
    fn copy_vertex(&mut self, from: &Mesh, v: usize) {
        let (n, t) = (self.normal_count, self.tex_count);
        self.positions.push(from.positions[v]);
        self.normals
            .extend_from_slice(&from.normals[v * n..(v + 1) * n]);
        self.colors.push(from.colors[v]);
        self.tex.extend_from_slice(&from.tex[v * t..(v + 1) * t]);
        if let Some(m) = from.matrices.get(v) {
            self.matrices.push(*m);
        }
    }

    fn push(&mut self, v: &Input, carries_matrices: bool) {
        self.positions.push(v.position);
        self.normals
            .extend_from_slice(&v.normal[..self.normal_count]);
        self.colors.push(v.color);
        self.tex.extend_from_slice(&v.tex[..self.tex_count]);
        if carries_matrices {
            let mut m = [v.pos_mtx; 9];
            m[1..].copy_from_slice(&v.tex_mtx);
            self.matrices.push(m);
        }
    }

    /// Its vertices.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Whether any of its elements are triangles, which name its vertices' words.
    fn has_triangles(&self) -> bool {
        self.runs.iter().any(|r| r.0 == Kind::Triangles)
    }

    /// The bytes it holds, for the cache's size.
    pub fn bytes(&self) -> usize {
        self.positions.len() * 12
            + self.normals.len() * 12
            + self.colors.len() * 8
            + self.tex.len() * 8
            + self.matrices.len() * 9
            + self.elements.len() * 4
    }

    /// Leaves in `normals` what drawing it does: its last vertex's normals, if it carries them.
    pub fn leave_normals(&self, normals: &mut NormalCache) {
        if let Some(last) = &self.last_normals {
            normals[..self.normal_count].copy_from_slice(&last[..self.normal_count]);
        }
    }

    /// Vertex `i` as the transform unit takes it, as decoding its bytes with the draw's state
    /// gives it, into `v`, which holds the draw's: `defaults` the matrix indices a vertex that
    /// carries none takes, and the normals the draws before left and zeros past its texture
    /// coordinates ([`Mesh::transform`]).
    #[inline]
    fn input(&self, i: usize, defaults: &(u8, [u8; 8]), v: &mut Input) {
        v.pos_mtx = defaults.0;
        v.tex_mtx = defaults.1;
        if let Some(m) = self.matrices.get(i) {
            let lo = self.vcd;
            if bits(lo, 0, 1) != 0 {
                v.pos_mtx = m[0];
            }
            for k in 0..8 {
                if bits(lo, 1 + k as u32, 1) != 0 {
                    v.tex_mtx[k] = m[1 + k];
                }
            }
        }
        v.position = self.positions[i];
        let n = self.normal_count;
        v.normal[..n].copy_from_slice(&self.normals[i * n..(i + 1) * n]);
        v.color = self.colors[i];
        let t = self.tex_count;
        v.tex[..t].copy_from_slice(&self.tex[i * t..(i + 1) * t]);
    }

    /// Each vertex transformed with `state`, `normals` those the draws before left: calls
    /// `each` with its number and output.
    pub fn transform(
        &self,
        state: &State,
        normals: &NormalCache,
        mut each: impl FnMut(usize, &Output),
    ) {
        let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
        let defaults = vertex::default_indices(xf_index);
        let setup = xform::Setup::new(&state.xf, &state.bp);
        let mut v = Input {
            normal: *normals,
            ..Input::default()
        };
        // The transform writes every output a vertex's words read, and reads back only what it
        // wrote for the vertex, but for an emboss coordinate made from a later coordinate,
        // which reads a zero: then each vertex starts from zeros, as the exact renderer's do.
        let fresh = (0..(state.xf[xform::NUM_TEXGENS] & 0xF).min(8) as usize).any(|n| {
            let info = state.xf[xform::TEXMTX_INFO + n];
            bits(info, 4, 3) == 1 && bits(info, 12, 3) as usize >= n
        });
        let mut o = Output::default();
        for i in 0..self.len() {
            self.input(i, &defaults, &mut v);
            if fresh {
                o = Output::default();
            }
            setup.transform(&v, &mut o);
            each(i, &o);
        }
    }
}

/// Expands a primitive's `n` vertices into elements, as [`crate::build`] does: calls `out`
/// with each element's vertex indices; returns their kind.
fn expand(primitive: u8, n: usize, mut out: impl FnMut(&[usize])) -> Kind {
    match primitive {
        0x80 | 0x88 => {
            let mut i = 3;
            while i < n {
                out(&[i - 3, i - 2, i - 1]);
                out(&[i - 3, i - 1, i]);
                i += 4;
            }
            if i == n {
                out(&[n - 3, n - 2, n - 1]);
            }
        }
        0x90 => {
            let mut i = 2;
            while i < n {
                out(&[i - 2, i - 1, i]);
                i += 3;
            }
        }
        0x98 => {
            for i in 2..n {
                if i % 2 == 0 {
                    out(&[i - 2, i - 1, i]);
                } else {
                    out(&[i - 2, i, i - 1]);
                }
            }
        }
        0xA0 => {
            for i in 2..n {
                out(&[0, i - 1, i]);
            }
        }
        0xA8 => {
            for k in 0..n / 2 {
                out(&[2 * k, 2 * k + 1]);
            }
            return Kind::Lines;
        }
        0xB0 => {
            for k in 0..n.saturating_sub(1) {
                out(&[k, k + 1]);
            }
            return Kind::Lines;
        }
        _ => {
            for i in 0..n {
                out(&[i]);
            }
            return Kind::Points;
        }
    }
    Kind::Triangles
}

/// Transforms a mesh's vertices with `state` and, unless its scissor leaves nothing to draw,
/// builds it into `built` as [`crate::build`] builds the list's draws: the GPU's words for its
/// vertices and its triangles (back faces culled), and lines and points as quads, in the list's
/// order. Leaves its normals in `scratch`.
pub(crate) fn build(
    state: &State,
    mesh: &Mesh,
    scratch: &mut Scratch,
    built: &mut crate::Built,
) -> Option<BuiltDraw> {
    let vp = Viewport::of(&state.xf);
    let (scissor, off) = Renderer::scissor(state, &vp);
    if scissor[2] == 0 || scissor[3] == 0 {
        mesh.leave_normals(&mut scratch.normals);
        return None;
    }
    let tg = texgens(state);
    let words = 6 + 3 * tg as u32;
    let (w0, i0) = (built.words.len(), built.indices.len());
    let s = scratch;
    s.outs.clear();
    s.screens.clear();
    s.clips.clear();
    let triangles = mesh.has_triangles();
    // Lines' and points' quads are made from their vertices' outputs.
    let quads = mesh.runs.iter().any(|r| r.0 != Kind::Triangles);
    if triangles {
        built.words.reserve(mesh.len() * words as usize);
    }
    let mut first = None;
    mesh.transform(state, &s.normals, |_, o| {
        let screen = vp.screen(o.clip);
        if triangles {
            Renderer::gpu_vertex(o, screen, off, &vp).write(tg, &mut built.words);
            s.clips.push(o.clip);
        }
        if first.is_none() {
            first = Some((*o, screen));
        }
        if quads {
            s.outs.push(*o);
            s.screens.push(screen);
        }
    });
    mesh.leave_normals(&mut s.normals);
    let cull = bits(state.bp[0x00], 14, 2);
    let (outs, screens, clips) = (&s.outs, &s.screens, &s.clips);
    let mut note: Option<(&'static str, u64)> = None;
    for (kind, run) in &mesh.runs {
        let els = &mesh.elements[run.start as usize..run.end as usize];
        match kind {
            Kind::Triangles => {
                for t in els.as_chunks::<3>().0 {
                    let [a, b, c] = t.map(|i| i as usize);
                    let backface = xform::is_backface(&state.xf, clips[a], clips[b], clips[c]);
                    let culled = if backface {
                        cull == 2 || cull == 3
                    } else {
                        cull == 1 || cull == 3
                    };
                    if !culled {
                        built.indices.extend([a, b, c].map(|i| words * i as u32));
                    }
                }
            }
            Kind::Lines => {
                for l in els.as_chunks::<2>().0 {
                    let [a, b] = l.map(|i| i as usize);
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
                if bits(state.bp[0x22], 16, 3) != 0 && !els.is_empty() {
                    note = Some(("line texture offsets", (els.len() / 2) as u64));
                }
            }
            Kind::Points => {
                for &i in els {
                    let i = i as usize;
                    point(state, &vp, off, &outs[i], screens[i], w0, built);
                }
                if bits(state.bp[0x22], 19, 3) != 0 && !els.is_empty() {
                    note = Some(("point texture offsets", els.len() as u64));
                }
            }
        }
    }
    Some(BuiltDraw {
        words: w0..built.words.len(),
        indices: i0..built.indices.len(),
        vertices: mesh.stream_vertices,
        texgens: tg as u32,
        note,
        first,
    })
}

/// A point: a square its size wide, as `crate::point` makes it (its corners' words, from one
/// vertex's colors and texture coordinates).
fn point(
    state: &State,
    vp: &Viewport,
    off: (f32, f32),
    o: &Output,
    s: [f32; 3],
    start: usize,
    built: &mut crate::Built,
) {
    let r = bits(state.bp[0x22], 8, 8) as f32 / 12.0;
    let tg = texgens(state);
    let n = 6 + 3 * tg;
    let mut w = [0u32; 30];
    w[4] = u32::from_le_bytes(o.color[0]);
    w[5] = u32::from_le_bytes(o.color[1]);
    for (w, t) in w[6..].as_chunks_mut::<3>().0.iter_mut().zip(&o.tex[..tg]) {
        *w = t.map(f32::to_bits);
    }
    let base = (built.words.len() - start) as u32;
    // Corners (ll, ul, lr, ur), triangles (ll, ul, lr) and (ur, lr, ul).
    for (x, y) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let at = [s[0] + x * r, s[1] + y * r, s[2]];
        let p = Renderer::gpu_position(o.clip, at, off, vp);
        for (w, p) in w.iter_mut().zip(p) {
            *w = p.to_bits();
        }
        built.words.extend_from_slice(&w[..n]);
    }
    let words = n as u32;
    built
        .indices
        .extend([0, 1, 2, 3, 2, 1].map(|i| base + words * i));
}
