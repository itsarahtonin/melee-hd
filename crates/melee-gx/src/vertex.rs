// SPDX-License-Identifier: GPL-3.0-or-later
// Vertex formats follow Dolphin's VertexLoader (GPL-2.0-or-later).

//! Vertex layouts: which attributes a vertex has (the vertex descriptor, VCD) and their formats
//! (the vertex attribute table, VAT), and fetching a draw's vertices, indexed attributes from
//! their arrays in memory.

use crate::{Memory, bits};

/// CP registers.
pub const MATINDEX_A: u8 = 0x30;
pub const MATINDEX_B: u8 = 0x40;
pub const VCD_LO: u8 = 0x50;
pub const VCD_HI: u8 = 0x60;
pub const VAT_A: u8 = 0x70;
pub const VAT_B: u8 = 0x80;
pub const VAT_C: u8 = 0x90;
pub const ARRAY_BASE: u8 = 0xA0;
pub const ARRAY_STRIDE: u8 = 0xB0;

/// Arrays, by attribute: position, normal, two colors, eight texture coordinates; then the four
/// the XF's indexed loads read.
pub const ARRAY_POS: usize = 0;
pub const ARRAY_NRM: usize = 1;
pub const ARRAY_COL0: usize = 2;
pub const ARRAY_TEX0: usize = 4;
pub const ARRAY_XF_A: usize = 12;

/// How an attribute comes: absent, in the stream, or as an 8- or 16-bit index into its array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    None,
    Direct,
    Index8,
    Index16,
}

impl Source {
    fn from(kind: u32) -> Self {
        match kind {
            0 => Source::None,
            1 => Source::Direct,
            2 => Source::Index8,
            _ => Source::Index16,
        }
    }

    /// Bytes it takes in the stream, given the attribute's own size.
    fn stream_size(self, direct: u32, indices: u32) -> u32 {
        match self {
            Source::None => 0,
            Source::Direct => direct,
            Source::Index8 => indices,
            Source::Index16 => 2 * indices,
        }
    }
}

/// Bytes per component of a position, normal or texture coordinate format.
pub fn comp_size(format: u32) -> u32 {
    match format {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 4,
    }
}

/// Bytes per color of a color format.
pub fn color_size(format: u32) -> u32 {
    match format {
        0 | 3 => 2,
        1 | 4 => 3,
        _ => 4,
    }
}

/// One attribute of a vertex layout.
#[derive(Clone, Copy, Debug)]
pub struct Attr {
    /// Its array, for an indexed one.
    pub array: usize,
    pub source: Source,
    /// Bytes of the attribute itself.
    pub size: u32,
    /// For normals with separate indices for normal, binormal and tangent: 3.
    pub indices: u32,
}

/// The vertex formats' attributes, kept by the registers that make them: a frame's draws use a
/// few formats over and over, alternating between them.
#[derive(Clone, Default)]
pub struct LayoutCache {
    /// Each format's registers and attributes, the last used first.
    entries: Vec<([u32; 6], Vec<Attr>)>,
}

/// The formats a [`LayoutCache`] keeps.
const LAYOUTS_KEPT: usize = 16;

impl LayoutCache {
    /// [`layout`]`(cp, vat)`, made again only when its registers are new.
    pub fn get(&mut self, cp: &[u32; 256], vat: usize) -> &[Attr] {
        let key = [
            cp[VCD_LO as usize],
            cp[VCD_HI as usize],
            cp[VAT_A as usize + vat],
            cp[VAT_B as usize + vat],
            cp[VAT_C as usize + vat],
            vat as u32,
        ];
        match self.entries.iter().position(|e| e.0 == key) {
            Some(0) => {}
            Some(i) => self.entries[..=i].rotate_right(1),
            None => {
                if self.entries.len() == LAYOUTS_KEPT {
                    self.entries.pop();
                }
                self.entries.insert(0, (key, layout(cp, vat)));
            }
        }
        &self.entries[0].1
    }
}

/// The attributes of a vertex in vertex format `vat`, in stream order.
pub fn layout(cp: &[u32; 256], vat: usize) -> Vec<Attr> {
    let lo = cp[VCD_LO as usize];
    let hi = cp[VCD_HI as usize];
    let a = cp[VAT_A as usize + vat];
    let b = cp[VAT_B as usize + vat];
    let c = cp[VAT_C as usize + vat];
    let mut out = Vec::new();
    let direct = |size| Attr {
        array: 0,
        source: Source::Direct,
        size,
        indices: 1,
    };
    // Matrix indices: position/normal, then eight texture matrices, a byte each.
    for i in 0..9 {
        if bits(lo, i, 1) != 0 {
            out.push(direct(1));
        }
    }
    let mut push = |array, kind, size, indices| {
        let source = Source::from(kind);
        if source != Source::None {
            out.push(Attr {
                array,
                source,
                size,
                indices,
            });
        }
    };
    push(
        ARRAY_POS,
        bits(lo, 9, 2),
        (2 + bits(a, 0, 1)) * comp_size(bits(a, 1, 3)),
        1,
    );
    let nbt = bits(a, 9, 1) != 0;
    let index3 = nbt && bits(a, 31, 1) != 0;
    let nrm_kind = bits(lo, 11, 2);
    let nrm_size = (if nbt { 9 } else { 3 }) * comp_size(bits(a, 10, 3));
    push(
        ARRAY_NRM,
        nrm_kind,
        nrm_size,
        if index3 && nrm_kind >= 2 { 3 } else { 1 },
    );
    push(ARRAY_COL0, bits(lo, 13, 2), color_size(bits(a, 14, 3)), 1);
    push(
        ARRAY_COL0 + 1,
        bits(lo, 15, 2),
        color_size(bits(a, 18, 3)),
        1,
    );
    // Texture coordinates: (elements bit, format) per coordinate.
    let tex = [
        (bits(a, 21, 1), bits(a, 22, 3)),
        (bits(b, 0, 1), bits(b, 1, 3)),
        (bits(b, 9, 1), bits(b, 10, 3)),
        (bits(b, 18, 1), bits(b, 19, 3)),
        (bits(b, 27, 1), bits(b, 28, 3)),
        (bits(c, 5, 1), bits(c, 6, 3)),
        (bits(c, 14, 1), bits(c, 15, 3)),
        (bits(c, 23, 1), bits(c, 24, 3)),
    ];
    for (i, (elems, format)) in tex.iter().enumerate() {
        push(
            ARRAY_TEX0 + i,
            bits(hi, 2 * i as u32, 2),
            (1 + elems) * comp_size(*format),
            1,
        );
    }
    out
}

/// Bytes a vertex takes in the stream.
pub fn stream_size(layout: &[Attr]) -> u32 {
    layout
        .iter()
        .map(|a| a.source.stream_size(a.size, a.indices))
        .sum()
}

/// Reads `count` vertices from `data` (the stream after the draw's header) into `out`, each
/// attribute as its bytes: an indexed one's from its array. Returns the stream bytes used.
///
/// Each indexed attribute's array is looked up once a draw: the stretch of it the draw's
/// indices reach, where the host holds it in one piece ([`Memory::view`]), else each element
/// is read on its own.
pub fn fetch(
    cp: &[u32; 256],
    layout: &[Attr],
    count: u32,
    data: &[u8],
    mem: &dyn Memory,
    out: &mut Vec<u8>,
) -> usize {
    let vertex = stream_size(layout) as usize;
    let index_at = |src: Source, at: usize| -> u32 {
        if src == Source::Index8 {
            u32::from(data[at])
        } else {
            u32::from(u16::from_be_bytes([data[at], data[at + 1]]))
        }
    };
    // Each attribute's array as far as the draw's indices reach it.
    let mut arrays: [Option<&[u8]>; MAX_ATTRS] = [None; MAX_ATTRS];
    let mut offset = 0usize;
    let mut size = 0usize;
    for (i, attr) in layout.iter().enumerate() {
        let width = attr.source.stream_size(attr.size, attr.indices) as usize;
        size += attr.size as usize;
        if matches!(attr.source, Source::Index8 | Source::Index16) && i < MAX_ATTRS {
            // The highest index, read straight from each vertex's bytes.
            let stream = &data[..count as usize * vertex];
            let mut top = 0u32;
            if vertex > 0 {
                for v in stream.chunks_exact(vertex) {
                    let ix = &v[offset..offset + width];
                    top = top.max(if attr.source == Source::Index8 {
                        ix.iter().copied().max().map_or(0, u32::from)
                    } else {
                        ix.as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u32::from(u16::from_be_bytes(*b)))
                            .max()
                            .unwrap_or(0)
                    });
                }
            }
            let base = cp[ARRAY_BASE as usize + attr.array] & 0x03FF_FFFF;
            let stride = cp[ARRAY_STRIDE as usize + attr.array] & 0xFF;
            let len = top * stride + attr.size;
            arrays[i] = mem.view(base, len as usize);
        }
        offset += width;
    }
    // Each attribute's registers, read once for the draw.
    let mut plans = [Plan::NONE; MAX_ATTRS];
    let mut n = 0;
    for (i, a) in layout.iter().enumerate() {
        if a.source != Source::None && n < MAX_ATTRS {
            plans[n] = Plan {
                source: a.source,
                size: a.size,
                indices: a.indices,
                part: a.size / a.indices.max(1),
                base: cp[ARRAY_BASE as usize + a.array] & 0x03FF_FFFF,
                stride: cp[ARRAY_STRIDE as usize + a.array] & 0xFF,
                array: arrays.get(i).copied().flatten(),
            };
            n += 1;
        }
    }
    let plan = &plans[..n];
    // Written into place: each vertex takes `size` bytes.
    let start = out.len();
    out.resize(start + count as usize * size, 0);
    let dst = &mut out[start..];
    let (mut at, mut to) = (0, 0);
    for _ in 0..count {
        for p in plan {
            if p.source == Source::Direct {
                let n = p.size as usize;
                put(&mut dst[to..to + n], &data[at..at + n]);
                at += n;
                to += n;
                continue;
            }
            let n = p.part as usize;
            for k in 0..p.indices {
                let index = index_at(p.source, at);
                at += if p.source == Source::Index8 { 1 } else { 2 };
                let off = index * p.stride + k * p.part;
                let into = &mut dst[to..to + n];
                if let Some(array) = p.array {
                    put(into, &array[off as usize..off as usize + n]);
                } else if let Some(bytes) = mem.view(p.base + off, n) {
                    put(into, bytes);
                } else {
                    mem.read(p.base + off, into);
                }
                to += n;
            }
        }
    }
    at
}

/// An attribute as [`fetch`] copies it: its registers, and its array as far as the draw reaches.
#[derive(Clone, Copy)]
struct Plan<'a> {
    source: Source,
    size: u32,
    indices: u32,
    part: u32,
    base: u32,
    stride: u32,
    array: Option<&'a [u8]>,
}

impl Plan<'_> {
    const NONE: Self = Plan {
        source: Source::None,
        size: 0,
        indices: 0,
        part: 0,
        base: 0,
        stride: 0,
        array: None,
    };
}

/// The most attributes a vertex has: the matrix indices, position, normal, two colors and
/// eight texture coordinates.
const MAX_ATTRS: usize = 32;

/// Copies an attribute's bytes, the common sizes as moves rather than a copy call.
fn put(dst: &mut [u8], b: &[u8]) {
    match b.len() {
        1 => dst[0] = b[0],
        2 => dst[..2].copy_from_slice(&b[..2]),
        3 => dst[..3].copy_from_slice(&b[..3]),
        4 => dst[..4].copy_from_slice(&b[..4]),
        6 => dst[..6].copy_from_slice(&b[..6]),
        8 => dst[..8].copy_from_slice(&b[..8]),
        12 => dst[..12].copy_from_slice(&b[..12]),
        _ => dst.copy_from_slice(b),
    }
}

/// A vertex as the transform unit takes it (Dolphin's software renderer's InputVertexData):
/// its matrix indices, position, normal with binormals, colors as RGBA and texture coordinates.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub pos_mtx: u8,
    pub tex_mtx: [u8; 8],
    pub position: [f32; 3],
    pub normal: [[f32; 3]; 3],
    pub color: [[u8; 4]; 2],
    pub tex: [[f32; 2]; 8],
}

/// What a vertex without normals takes: the normal and binormals of the last vertex of the
/// last draw that had them.
pub type NormalCache = [[f32; 3]; 3];

fn component(data: &[u8], at: &mut usize, format: u32, scale: f32) -> f32 {
    let b = &data[*at..];
    let (v, n) = match format {
        0 => (f32::from(b[0]) * scale, 1),
        1 => (f32::from(b[0] as i8) * scale, 1),
        2 => (f32::from(u16::from_be_bytes([b[0], b[1]])) * scale, 2),
        3 => (f32::from(i16::from_be_bytes([b[0], b[1]])) * scale, 2),
        _ => (
            f32::from_bits(u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            4,
        ),
    };
    *at += n;
    v
}

/// The fixed scale of a normal's integer format.
fn normal_scale(format: u32) -> f32 {
    match format {
        0 => 1.0 / 128.0,
        1 => 1.0 / 64.0,
        2 => 1.0 / 32768.0,
        3 => 1.0 / 16384.0,
        _ => 1.0,
    }
}

fn expand(v: u32, bits: u32) -> u8 {
    match bits {
        4 => (v << 4 | v) as u8,
        5 => (v << 3 | v >> 2) as u8,
        _ => (v << 2 | v >> 4) as u8,
    }
}

fn color(data: &[u8], at: &mut usize, format: u32) -> [u8; 4] {
    let b = &data[*at..];
    *at += color_size(format) as usize;
    match format {
        0 => {
            let v = u32::from(u16::from_be_bytes([b[0], b[1]]));
            [
                expand(v >> 11, 5),
                expand(v >> 5 & 63, 6),
                expand(v & 31, 5),
                255,
            ]
        }
        1 | 2 => [b[0], b[1], b[2], 255],
        3 => [
            expand(u32::from(b[0] >> 4), 4),
            expand(u32::from(b[0] & 15), 4),
            expand(u32::from(b[1] >> 4), 4),
            expand(u32::from(b[1] & 15), 4),
        ],
        4 => {
            let v = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
            [
                expand(v >> 18 & 63, 6),
                expand(v >> 12 & 63, 6),
                expand(v >> 6 & 63, 6),
                expand(v & 63, 6),
            ]
        }
        _ => [b[0], b[1], b[2], b[3]],
    }
}

/// Decodes `count` vertices of format `vat` from `data`, a draw's vertices as `fetch` gives
/// them. Matrix indices a vertex doesn't carry come from the XF's matrix index registers
/// (`xf_index`: MATINDEX_A and B), missing colors are white, and missing normals come from
/// `normals`, which the last vertex with them updates.
pub fn decode(
    cp: &[u32; 256],
    xf_index: [u32; 2],
    vat: usize,
    data: &[u8],
    count: usize,
    normals: &mut NormalCache,
    out: &mut Vec<Input>,
) {
    let lo = cp[VCD_LO as usize];
    let hi = cp[VCD_HI as usize];
    let a = cp[VAT_A as usize + vat];
    let b = cp[VAT_B as usize + vat];
    let c = cp[VAT_C as usize + vat];
    // (elements bit, format, frac) per texture coordinate.
    let tex = [
        (bits(a, 21, 1), bits(a, 22, 3), bits(a, 25, 5)),
        (bits(b, 0, 1), bits(b, 1, 3), bits(b, 4, 5)),
        (bits(b, 9, 1), bits(b, 10, 3), bits(b, 13, 5)),
        (bits(b, 18, 1), bits(b, 19, 3), bits(b, 22, 5)),
        (bits(b, 27, 1), bits(b, 28, 3), bits(c, 0, 5)),
        (bits(c, 5, 1), bits(c, 6, 3), bits(c, 9, 5)),
        (bits(c, 14, 1), bits(c, 15, 3), bits(c, 18, 5)),
        (bits(c, 23, 1), bits(c, 24, 3), bits(c, 27, 5)),
    ];
    let pos_scale = 1.0 / (1u32 << bits(a, 4, 5)) as f32;
    let nbt = bits(a, 9, 1) != 0;
    let has_nrm = bits(lo, 11, 2) != 0;
    let (has_col0, has_col1) = (bits(lo, 13, 2) != 0, bits(lo, 15, 2) != 0);
    // What every vertex starts from: the registers' matrix indices, white, and the normals.
    let mut base = Input {
        pos_mtx: (xf_index[0] & 0x3F) as u8,
        normal: *normals,
        color: [[255; 4]; 2],
        ..Input::default()
    };
    for i in 0..8 {
        let reg = xf_index[usize::from(i >= 4)];
        base.tex_mtx[i] = (reg >> (6 * ((i % 4) + usize::from(i < 4))) & 0x3F) as u8;
    }
    let tex_scales = tex.map(|(_, _, frac)| 1.0 / (1u32 << frac) as f32);
    out.reserve(count);
    let mut at = 0;
    for _ in 0..count {
        // Built in place: the normals a vertex without them takes don't change in a draw.
        out.push(base);
        let v = out.last_mut().expect("a vertex");
        if bits(lo, 0, 1) != 0 {
            v.pos_mtx = data[at] & 0x3F;
            at += 1;
        }
        for i in 0..8 {
            if bits(lo, 1 + i, 1) != 0 {
                v.tex_mtx[i as usize] = data[at] & 0x3F;
                at += 1;
            }
        }
        if bits(lo, 9, 2) != 0 {
            let n = 2 + bits(a, 0, 1);
            let format = bits(a, 1, 3);
            for k in 0..n as usize {
                v.position[k] = component(data, &mut at, format, pos_scale);
            }
        }
        if has_nrm {
            let format = bits(a, 10, 3);
            let scale = normal_scale(format);
            for k in 0..if nbt { 3 } else { 1 } {
                for j in 0..3 {
                    v.normal[k][j] = component(data, &mut at, format, scale);
                }
            }
        }
        // Only one color goes to channel 0.
        let col0 = has_col0.then(|| color(data, &mut at, bits(a, 14, 3)));
        let col1 = has_col1.then(|| color(data, &mut at, bits(a, 18, 3)));
        match (col0, col1) {
            (Some(c0), c1) => {
                v.color[0] = c0;
                v.color[1] = c1.unwrap_or([255; 4]);
            }
            (None, Some(c1)) => v.color[0] = c1,
            (None, None) => {}
        }
        for (i, (elems, format, _)) in tex.iter().enumerate() {
            if bits(hi, 2 * i as u32, 2) != 0 {
                let scale = tex_scales[i];
                for k in 0..=*elems as usize {
                    v.tex[i][k] = component(data, &mut at, *format, scale);
                }
            }
        }
        if has_nrm {
            let n = if nbt { 3 } else { 1 };
            normals[..n].copy_from_slice(&v.normal[..n]);
        }
    }
}

/// What [`decode`] leaves in `normals` for a draw, decoding only its last vertex: the normals
/// of a draw whose vertices carry them are its last vertex's, and a draw without them keeps
/// them. `data` holds `count` vertices of one size, as [`fetch`] gives them.
pub fn skip(
    cp: &[u32; 256],
    xf_index: [u32; 2],
    vat: usize,
    data: &[u8],
    count: usize,
    normals: &mut NormalCache,
    scratch: &mut Vec<Input>,
) {
    if count == 0 || bits(cp[VCD_LO as usize], 11, 2) == 0 {
        return;
    }
    let size = data.len() / count;
    scratch.clear();
    decode(
        cp,
        xf_index,
        vat,
        &data[(count - 1) * size..],
        1,
        normals,
        scratch,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator for the checks' formats and bytes.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 16) as u32
        }
    }

    /// Memory of one piece from address 0, which views where asked to.
    struct Flat(Vec<u8>, bool);

    impl Memory for Flat {
        fn read(&self, phys: u32, out: &mut [u8]) {
            for (i, b) in out.iter_mut().enumerate() {
                *b = self.0.get(phys as usize + i).copied().unwrap_or(0);
            }
        }

        fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
            self.1
                .then(|| self.0.get(phys as usize..phys as usize + len))
                .flatten()
        }
    }

    /// A random vertex format: every attribute absent, direct or indexed, in any format.
    fn random_cp(r: &mut Rng) -> [u32; 256] {
        let mut cp = [0u32; 256];
        cp[VCD_LO as usize] = r.next() & 0x1_FFFF;
        cp[VCD_HI as usize] = r.next() & 0xFFFF;
        for vat in 0..8 {
            cp[VAT_A as usize + vat] = r.next();
            cp[VAT_B as usize + vat] = r.next();
            cp[VAT_C as usize + vat] = r.next();
        }
        for a in 0..16 {
            cp[ARRAY_BASE as usize + a] = r.next() % 4096;
            cp[ARRAY_STRIDE as usize + a] = r.next() % 64;
        }
        cp
    }

    /// `fetch` as it was, an element at a time, to check the faster one against.
    fn fetch_slowly(
        cp: &[u32; 256],
        layout: &[Attr],
        count: u32,
        data: &[u8],
        mem: &dyn Memory,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut at = 0;
        for _ in 0..count {
            for attr in layout {
                match attr.source {
                    Source::None => {}
                    Source::Direct => {
                        out.extend_from_slice(&data[at..at + attr.size as usize]);
                        at += attr.size as usize;
                    }
                    Source::Index8 | Source::Index16 => {
                        let base = cp[ARRAY_BASE as usize + attr.array] & 0x03FF_FFFF;
                        let stride = cp[ARRAY_STRIDE as usize + attr.array] & 0xFF;
                        let part = attr.size / attr.indices;
                        for k in 0..attr.indices {
                            let index = if attr.source == Source::Index8 {
                                u32::from(data[at])
                            } else {
                                u32::from(u16::from_be_bytes([data[at], data[at + 1]]))
                            };
                            at += if attr.source == Source::Index8 { 1 } else { 2 };
                            let mut bytes = vec![0; part as usize];
                            mem.read(base + index * stride + k * part, &mut bytes);
                            out.extend_from_slice(&bytes);
                        }
                    }
                }
            }
        }
        out
    }

    /// Fetching copies each vertex's attributes as an element-at-a-time copy does, from
    /// memory held in one piece or read, and the layout cache gives each format's layout as
    /// formats come and go.
    #[test]
    fn fetch_and_layouts_are_as_made_one_at_a_time() {
        let mut r = Rng(0x2545_F491_4F6C_DD1D);
        let memory: Vec<u8> = (0..1 << 20).map(|_| r.next() as u8).collect();
        let mut cache = LayoutCache::default();
        let cps: Vec<[u32; 256]> = (0..40).map(|_| random_cp(&mut r)).collect();
        for round in 0..2000 {
            let cp = &cps[r.next() as usize % cps.len()];
            let vat = r.next() as usize % 8;
            let want = layout(cp, vat);
            assert_eq!(format!("{:?}", cache.get(cp, vat)), format!("{want:?}"));
            let count = r.next() % 40;
            let data: Vec<u8> = (0..count * stream_size(&want))
                .map(|_| r.next() as u8)
                .collect();
            for viewed in [true, false] {
                let mem = Flat(memory.clone(), viewed);
                let mut got = vec![7];
                let used = fetch(cp, &want, count, &data, &mem, &mut got);
                assert_eq!(used, data.len(), "round {round}");
                assert_eq!(
                    got[1..],
                    fetch_slowly(cp, &want, count, &data, &mem)[..],
                    "round {round}"
                );
            }
        }
    }

    /// Skipping a draw leaves the normals decoding all of it does, with and without normals,
    /// binormals and tangents, in every normal format.
    #[test]
    fn skipping_leaves_the_normals_decoding_does() {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..4000 {
            let cp = random_cp(&mut r);
            let vat = r.next() as usize % 8;
            let attrs = layout(&cp, vat);
            // The vertices as fetching gives them: each attribute's bytes.
            let size: u32 = attrs.iter().map(|a| a.size).sum();
            let count = r.next() as usize % 6;
            let data: Vec<u8> = (0..count * size as usize).map(|_| r.next() as u8).collect();
            let xf_index = [r.next(), r.next()];
            let start: NormalCache =
                std::array::from_fn(|_| std::array::from_fn(|_| r.next() as f32));
            let (mut decoded, mut skipped) = (start, start);
            let mut scratch = Vec::new();
            decode(&cp, xf_index, vat, &data, count, &mut decoded, &mut scratch);
            skip(&cp, xf_index, vat, &data, count, &mut skipped, &mut scratch);
            let bits = |n: &NormalCache| n.map(|v| v.map(f32::to_bits));
            assert_eq!(bits(&skipped), bits(&decoded));
        }
    }
}
