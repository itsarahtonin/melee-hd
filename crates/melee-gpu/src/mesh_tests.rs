//! A mesh draws what its display list draws: over random display lists (every primitive, in
//! random vertex formats, indexed attributes repeating), random transform states and the
//! normals the draws before left, the words of each element a mesh's draw builds, in order,
//! and the normals it leaves are those the list's draws build one by one ([`crate::build`]),
//! bit for bit (any NaN as any other), whole or split in parts.

use melee_gx::vertex::{self, NormalCache};
use melee_gx::{Draw, Memory, State, xform};

use crate::mesh::{self, Mesh};
use crate::{Built, BuiltDraw, Scratch, build};

pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    pub(crate) fn float(&mut self) -> f32 {
        (self.next() % 2000) as f32 / 500.0 - 2.0
    }
}

/// Memory of one piece from address 0.
pub(crate) struct Flat(pub(crate) Vec<u8>);

impl Memory for Flat {
    fn read(&self, phys: u32, out: &mut [u8]) {
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.0.get(phys as usize + i).copied().unwrap_or(0);
        }
    }

    fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
        self.0.get(phys as usize..phys as usize + len)
    }
}

/// A random vertex format 0: matrix indices or not, each attribute absent, direct or indexed,
/// in any format; arrays in the first 4 KB.
pub(crate) fn format(r: &mut Rng) -> [u32; 256] {
    let mut cp = [0u32; 256];
    // Each attribute's kind: positions, normals and the two colors, then the coordinates.
    let lo = r.next() & 0x1_FFFF;
    let hi = r.next() & 0xFFFF;
    cp[vertex::VCD_LO as usize] = lo;
    cp[vertex::VCD_HI as usize] = hi;
    cp[vertex::VAT_A as usize] = r.next();
    cp[vertex::VAT_B as usize] = r.next();
    cp[vertex::VAT_C as usize] = r.next();
    for a in 0..12 {
        cp[vertex::ARRAY_BASE as usize + a] = r.next() % 4096;
        cp[vertex::ARRAY_STRIDE as usize + a] = r.next() % 64;
    }
    cp
}

/// A display list of random primitives in format 0, a few vertices each.
pub(crate) fn list(r: &mut Rng, cp: &[u32; 256]) -> Vec<u8> {
    let size = vertex::stream_size(&vertex::layout(cp, 0)) as usize;
    // A few vertices the stream's take, so vertices are shared; small bytes, so indices repeat.
    let pool: Vec<Vec<u8>> = (0..1 + r.next() % 8)
        .map(|_| (0..size).map(|_| (r.next() % 4) as u8).collect())
        .collect();
    let mut out = Vec::new();
    for _ in 0..1 + r.next() % 6 {
        let op = [0x80, 0x88, 0x90, 0x98, 0xA0, 0xA8, 0xB0, 0xB8][r.next() as usize % 8];
        let n = r.next() as usize % 13;
        out.push(op);
        out.extend((n as u16).to_be_bytes());
        for _ in 0..n {
            out.extend(&pool[r.next() as usize % pool.len()]);
        }
    }
    out.resize(out.len().next_multiple_of(32), 0);
    out
}

/// A random transform state with a viewport and scissor that draw.
pub(crate) fn state(r: &mut Rng, cp: [u32; 256]) -> State {
    let mut s = State::new();
    s.cp = cp;
    for w in s.xf[..0x1000].iter_mut() {
        *w = r.float().to_bits();
    }
    for l in 0..8 {
        s.xf[xform::LIGHTS + l * 16 + 3] = r.next();
    }
    for w in s.xf[0x1000..].iter_mut() {
        *w = r.next();
    }
    s.xf[xform::PROJECTION + 6] = r.next() & 1;
    for i in 0..6 {
        s.xf[xform::PROJECTION + i] = r.float().to_bits();
    }
    s.xf[xform::NUM_TEXGENS] = r.next() % 9;
    let vp = [320.0f32, -240.0, 16_777_215.0, 662.0, 582.0, 16_777_215.0];
    for (i, v) in vp.into_iter().enumerate() {
        s.xf[xform::VIEWPORT + i] = v.to_bits();
    }
    for w in s.bp.iter_mut() {
        *w = r.next() & 0xFF_FFFF;
    }
    s.bp[0x20] = 342 << 12 | 342;
    s.bp[0x21] = (342 + 639) << 12 | (342 + 527);
    s.bp[0x59] = 171 << 10 | 171;
    s
}

/// Each element's vertices' words, in order, from a draw built into `built`.
fn elements(built: &Built, d: Option<&BuiltDraw>, texgens: usize, out: &mut Vec<Vec<u32>>) {
    let Some(d) = d else { return };
    let words = 6 + 3 * texgens;
    for &i in &built.indices[d.indices.clone()] {
        let at = d.words.start + i as usize;
        // A NaN's sign and payload are the compiler's to choose (Rust's float semantics), and
        // the GPU drops a NaN vertex's triangle either way: the float words' NaNs as one.
        let mut v = built.words[at..at + words].to_vec();
        for (k, w) in v.iter_mut().enumerate() {
            if !(4..6).contains(&k) && f32::from_bits(*w).is_nan() {
                *w = f32::NAN.to_bits();
            }
        }
        out.push(v);
    }
}

/// The list's draws built one by one, as the exact renderer builds them.
fn exact(s: &State, list: &[u8], mem: &Flat, normals: NormalCache) -> (Vec<Vec<u32>>, NormalCache) {
    let layout = vertex::layout(&s.cp, 0);
    let size = vertex::stream_size(&layout) as usize;
    let tg = crate::texgens(s);
    let mut scratch = Scratch {
        normals,
        ..Scratch::default()
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at < list.len() && list[at] != 0 {
        let n = usize::from(u16::from_be_bytes([list[at + 1], list[at + 2]]));
        let stream = &list[at + 3..at + 3 + n * size];
        let mut fetched = Vec::new();
        vertex::fetch(&s.cp, &layout, n as u32, stream, mem, &mut fetched);
        let draw = Draw {
            primitive: list[at] & 0xF8,
            vat: 0,
            count: n as u16,
            layout: &layout,
            data: &fetched,
            fetched: true,
        };
        let mut built = Built::default();
        let d = build(s, &draw, &mut scratch, &mut built);
        elements(&built, d.as_ref(), tg, &mut out);
        at += 3 + n * size;
    }
    (out, scratch.normals)
}

/// The mesh's parts built one after another.
fn meshes(s: &State, parts: &[Mesh], normals: NormalCache) -> (Vec<Vec<u32>>, NormalCache) {
    let tg = crate::texgens(s);
    let mut scratch = Scratch {
        normals,
        ..Scratch::default()
    };
    let mut out = Vec::new();
    for m in parts {
        let mut built = Built::default();
        let d = mesh::build(s, m, &mut scratch, &mut built);
        elements(&built, d.as_ref(), tg, &mut out);
    }
    (out, scratch.normals)
}

#[test]
fn a_mesh_draws_what_its_list_draws() {
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let mem = Flat((0..8192).map(|_| r.next() as u8).collect());
    let (mut shared, mut lists) = (0, 0);
    for round in 0..3000 {
        let cp = format(&mut r);
        let list = list(&mut r, &cp);
        let s = state(&mut r, cp);
        let normals: NormalCache = std::array::from_fn(|_| std::array::from_fn(|_| r.float()));
        let m = Mesh::decode(&s.cp, &list, &mem).expect("a mesh");
        shared += usize::from((m.len() as u32) < m.stream_vertices);
        lists += 1;
        let want = exact(&s, &list, &mem, normals);
        let bits = |n: &NormalCache| n.map(|v| v.map(f32::to_bits));
        let limit = 1 + r.next() as usize % 8;
        for parts in [vec![m.clone()], m.clone().split(limit)] {
            let got = meshes(&s, &parts, normals);
            assert_eq!(got.0, want.0, "round {round}: the elements' words");
            assert_eq!(
                bits(&got.1),
                bits(&want.1),
                "round {round}: the normals left"
            );
        }
    }
    // The check reaches meshes with shared vertices.
    assert!(
        shared > lists / 4,
        "{shared} of {lists} lists shared vertices"
    );
}

/// A list with anything but draws in format 0 is no mesh.
#[test]
fn state_changes_and_other_formats_are_no_mesh() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mem = Flat(vec![0; 64]);
    let cp = format(&mut r);
    for op in [0x08, 0x10, 0x20, 0x40, 0x61, 0x91] {
        let list = [0x90, 0, 0, op, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(Mesh::decode(&cp, &list, &mem).is_none(), "{op:#04x}");
    }
    assert!(Mesh::decode(&cp, &[0x90, 0, 0, 0, 0x48, 0x44], &mem).is_some());
}
