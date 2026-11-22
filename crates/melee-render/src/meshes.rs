//! The fast renderer's mesh cache (docs/fast-renderer.md, F2): each shape's display list and
//! vertices decoded once (`melee_gpu::mesh::Mesh`) and kept from frame to frame, rather than
//! fetched and decoded from the GX stream every frame.
//!
//! A mesh is keyed by what the GPU reads to draw the list, named by where it is in the loaded
//! archives rather than by address: the display list (`PObj::display`) and its length, how the
//! game rewrote it if it did (refraction's matrix indices, `PObjs::refract_patches`), and the
//! vertex descriptor, format and arrays the shape's vertex descriptions (`PObj::verts`) set.
//! So a rewritten list's key changes with its patch; a freed shape's place reused by another
//! shape, or an archive's range of addresses given to another archive (`mem::Space`), finds
//! another key; and nothing is kept by a shape's handle or an address. Archives are immutable,
//! so a key's bytes never change while the key stands, unless a copy from the EFB lands on
//! them, which no mesh is drawn over ([`Meshes::get`]).

use std::sync::Arc;

use melee_data::archive::{Archives, DataRef};
use melee_gpu::mesh::Mesh;
use melee_gx::{Memory, State, vertex};
use rustc_hash::FxHashMap;

use crate::mem::Space;

/// A mesh in parts, drawn one after another ([`Mesh::split`]).
pub type Parts = Arc<[Arc<Mesh>]>;

/// The vertices a part of a mesh holds, at most about, where the CPU transforms them (F2): a
/// large list (Fountain of Dreams' stars, thousands of points) is shared out among the
/// renderer's threads.
const PART: usize = 1024;

/// Bytes of meshes kept while nobody draws them: past this, the least recently drawn go first
/// ([`Meshes::begin_frame`]). A match's meshes take a few MB, so a match keeps every mesh it has
/// drawn, and one that comes back (a joint shown again, an effect used again) isn't decoded
/// again.
const BUDGET: usize = 64 << 20;

/// Frames between checks of the budget.
const CHECK_EVERY: u32 = 60;

/// What the GPU reads to draw a display list, by place in the archives.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    list: DataRef,
    size: u32,
    /// The rewrite of the list: the vertex stride and the two indices' offsets.
    patch: Option<(u8, u8, u8)>,
    /// The vertex descriptor (`VCD_LO`, `VCD_HI`) and format 0 (`VAT_A`, `B`, `C`), as far as
    /// the descriptor reads it ([`format_key`]).
    format: [u32; 5],
    /// Each indexed attribute's array and stride, by `vertex::ARRAY_POS` on.
    arrays: [Option<(DataRef, u8)>; 12],
}

struct Entry {
    /// None for a list a mesh can't stand for, which draws as the stream has it.
    mesh: Option<Parts>,
    used: u32,
    /// Its parts' bytes.
    bytes: usize,
}

/// Counts of what the cache did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Lists drawn from a mesh kept, and from one decoded.
    pub hits: u64,
    pub decoded: u64,
    /// Lists drawn as the stream has them: outside the archives, a copy over them, or not a
    /// mesh's (state changes among the draws, another vertex format).
    pub streamed: u64,
}

/// The meshes kept, by key.
#[derive(Default)]
pub struct Meshes {
    entries: FxHashMap<Key, Entry>,
    /// The entries' bytes.
    bytes: usize,
    /// The budget they're kept to ([`BUDGET`]).
    budget: usize,
    frame: u32,
    /// Whether a large list is drawn in parts ([`PART`]).
    split: bool,
    pub stats: Stats,
}

/// A prepared shape's arrays as the GPU reads them ([`Meshes::prepare`]): each array at an
/// address of its own, `SPAN` bytes apart, read from its archive.
struct Arrays<'a> {
    data: &'a Archives,
    arrays: &'a [Option<(DataRef, u8)>; 12],
}

impl Arrays<'_> {
    const SPAN: u32 = 4 << 20;

    fn base(a: usize) -> u32 {
        (a as u32 + 1) * Self::SPAN
    }
}

impl Memory for Arrays<'_> {
    fn read(&self, phys: u32, out: &mut [u8]) {
        out.fill(0);
        let a = (phys / Self::SPAN) as usize;
        let Some(Some((at, _))) = a.checked_sub(1).and_then(|a| self.arrays.get(a)) else {
            return;
        };
        let bytes = self.data.get(at.archive).data();
        let from = at.offset as usize + (phys % Self::SPAN) as usize;
        if let Some(src) = bytes.get(from..) {
            let n = src.len().min(out.len());
            out[..n].copy_from_slice(&src[..n]);
        }
    }
}

/// Whether an attribute's VCD kind is an index into its array.
fn indexed(kind: u32) -> bool {
    kind >= 2
}

/// The VCD kind of each attribute with an array, by `vertex::ARRAY_POS` on.
fn kinds(cp: &[u32; 256]) -> [u32; 12] {
    let lo = cp[vertex::VCD_LO as usize];
    let hi = cp[vertex::VCD_HI as usize];
    std::array::from_fn(|a| match a {
        0..4 => (lo >> (9 + 2 * a)) & 3,
        _ => (hi >> (2 * (a - 4))) & 3,
    })
}

/// The vertex descriptor and format 0 as a key holds them: the format's fields for the
/// attributes the descriptor sends (`NORMALINDEX3` with the normals), with every other field
/// cleared (`BYTEDEQUANT` and `VCACHE_ENHANCE` too, which decoding doesn't read). The game leaves an attribute's format as the last shape that sent it set
/// it, and the GPU reads only those it sends ([`Mesh::decode`] likewise), so a list drawn after
/// shapes that left different formats for attributes it doesn't send is the same mesh.
fn format_key(cp: &[u32; 256]) -> [u32; 5] {
    let lo = cp[vertex::VCD_LO as usize];
    let hi = cp[vertex::VCD_HI as usize];
    let sent = |on: bool, mask: u32| if on { mask } else { 0 };
    let tex = |n: u32| hi >> (2 * n) & 3 != 0;
    let a = sent(lo >> 9 & 3 != 0, 0x0000_01FF)
        | sent(lo >> 11 & 3 != 0, 0x8000_1E00)
        | sent(lo >> 13 & 3 != 0, 0x0001_E000)
        | sent(lo >> 15 & 3 != 0, 0x001E_0000)
        | sent(tex(0), 0x3FE0_0000);
    let b = sent(tex(1), 0x0000_01FF)
        | sent(tex(2), 0x0003_FE00)
        | sent(tex(3), 0x07FC_0000)
        | sent(tex(4), 0x7800_0000);
    let c = sent(tex(4), 0x0000_001F)
        | sent(tex(5), 0x0000_3FE0)
        | sent(tex(6), 0x007F_C000)
        | sent(tex(7), 0xFF80_0000);
    [
        lo,
        hi,
        cp[vertex::VAT_A as usize] & a,
        cp[vertex::VAT_B as usize] & b,
        cp[vertex::VAT_C as usize] & c,
    ]
}

impl Meshes {
    /// A cache whose large lists are drawn in parts, with `split`.
    pub fn new(split: bool) -> Meshes {
        Meshes {
            split,
            budget: BUDGET,
            ..Meshes::default()
        }
    }

    /// Starts a frame: while the meshes kept hold more than the budget, those drawn longest
    /// ago go.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        if !self.frame.is_multiple_of(CHECK_EVERY) || self.bytes <= self.budget {
            return;
        }
        let frame = self.frame;
        let mut by_age: Vec<(u32, usize)> = self
            .entries
            .values()
            .map(|e| (frame.wrapping_sub(e.used), e.bytes))
            .collect();
        by_age.sort_unstable_by_key(|e| std::cmp::Reverse(e.0));
        // Drop the oldest until what's left fits: those at least `age` frames old.
        let mut held = self.bytes;
        let mut age = u32::MAX;
        for &(a, b) in &by_age {
            if held <= self.budget || a == 0 {
                break;
            }
            held -= b;
            age = a;
        }
        self.entries.retain(|_, e| frame.wrapping_sub(e.used) < age);
        self.bytes = self.entries.values().map(|e| e.bytes).sum();
    }

    /// Decodes the mesh of every shape the game holds (`game.objs.pobjs`: the stage's, the
    /// fighters', the interface's, shown or not yet) before anything draws it, keyed as its
    /// draw will key it: its list and arrays, and the formats its vertex descriptions set
    /// (`setupVtxDesc`'s), on several threads. A shape whose draw keys it otherwise (a
    /// rewritten list, a shape animation's vertices) is decoded as before when drawn. Returns
    /// how many were decoded.
    pub fn prepare(
        &mut self,
        space: &Space,
        data: &Archives,
        pobjs: &melee_sim::hsd::pobj::PObjs,
    ) -> usize {
        use crate::gx::enums::{
            GX_DIRECT, GX_VA_NBT, GX_VA_NRM, GX_VA_NULL, GX_VA_PNMTXIDX, GX_VA_POS,
            GX_VA_TEX7MTXIDX, GX_VTXFMT0,
        };
        type Todo = (Key, Box<[u32; 256]>, Vec<u8>, [Option<(DataRef, u8)>; 12]);
        let pool = &pobjs.pool;
        let mut todo: Vec<Todo> = Vec::new();
        for h in pool.handles() {
            let Some(p) = pool.get(h).filter(|_| !pool.is_free(h)) else {
                continue;
            };
            let (Some(verts), Some(display)) = (p.verts, p.display) else {
                continue;
            };
            if p.ty() == melee_sim::hsd::pobj::SHAPEANIM || space.patch(display).is_some() {
                continue;
            }
            let mut gx = crate::gx::Gx::new();
            gx.clear_vtx_desc();
            let mut arrays = [None; 12];
            let mut d = verts;
            loop {
                let attr = data.u32(d);
                if attr == GX_VA_NULL {
                    break;
                }
                gx.set_vtx_desc(attr, data.u32(d.at(4)));
                if !(GX_VA_PNMTXIDX..=GX_VA_TEX7MTXIDX).contains(&attr) {
                    gx.set_vtx_attr_fmt(
                        GX_VTXFMT0,
                        attr,
                        data.u32(d.at(8)),
                        data.u32(d.at(0xC)),
                        data.u8(d.at(0x10)),
                    );
                }
                if data.u32(d.at(4)) != GX_DIRECT {
                    let a = if attr == GX_VA_NBT { GX_VA_NRM } else { attr };
                    let slot = a.wrapping_sub(GX_VA_POS) as usize;
                    if let (Some(s), Some(base)) = (arrays.get_mut(slot), data.ptr(d.at(0x14))) {
                        *s = Some((base, data.u16(d.at(0x12)) as u8));
                    }
                }
                d = d.at(0x18);
            }
            let mut cp = Box::new([0u32; 256]);
            let g = &gx.data;
            cp[vertex::VCD_LO as usize] = g.vcd_lo;
            cp[vertex::VCD_HI as usize] = g.vcd_hi;
            cp[vertex::VAT_A as usize] = g.vat_a[0];
            cp[vertex::VAT_B as usize] = g.vat_b[0];
            cp[vertex::VAT_C as usize] = g.vat_c[0];
            let kinds = kinds(&cp);
            if (0..12).any(|a| indexed(kinds[a]) && arrays[a].is_none()) {
                continue;
            }
            // The arrays the descriptor indexes, each at an address of its own (`Arrays`).
            for (a, slot) in arrays.iter().enumerate() {
                if let Some((_, stride)) = slot {
                    cp[vertex::ARRAY_BASE as usize + a] = Arrays::base(a);
                    cp[vertex::ARRAY_STRIDE as usize + a] = u32::from(*stride);
                }
            }
            let size = u32::from(p.n_display) * 32;
            let key = Key {
                list: display,
                size,
                patch: None,
                format: format_key(&cp),
                arrays: std::array::from_fn(|a| arrays[a].filter(|_| indexed(kinds[a]))),
            };
            if self.entries.contains_key(&key) || todo.iter().any(|t| t.0 == key) {
                continue;
            }
            let at = display.offset as usize;
            let Some(list) = data.get(display.archive).data().get(at..at + size as usize) else {
                continue;
            };
            todo.push((key, cp, list.to_vec(), arrays));
        }
        let part = if self.split { PART } else { usize::MAX };
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(8);
        let chunk = todo.len().div_ceil(threads).max(1);
        let decoded: Vec<Option<Parts>> = std::thread::scope(|scope| {
            let jobs: Vec<_> = todo
                .chunks(chunk)
                .map(|some| {
                    scope.spawn(move || {
                        some.iter()
                            .map(|(_, cp, list, arrays)| {
                                let mem = Arrays { data, arrays };
                                Mesh::decode(cp, list, &mem).map(|m| {
                                    m.split(part).into_iter().map(Arc::new).collect::<Parts>()
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            jobs.into_iter()
                .flat_map(|j| j.join().expect("a mesh decoded"))
                .collect()
        });
        let n = todo.len();
        for ((key, _, list, _), mesh) in todo.into_iter().zip(decoded) {
            let bytes = mesh
                .as_ref()
                .map_or(0, |p| p.iter().map(|m| m.bytes()).sum())
                + list.len();
            self.bytes += bytes;
            self.entries.insert(
                key,
                Entry {
                    mesh,
                    used: self.frame,
                    bytes,
                },
            );
        }
        n
    }

    /// Meshes kept, and their bytes.
    pub fn size(&self) -> (usize, usize) {
        let parts = self.entries.values().filter_map(|e| e.mesh.as_ref());
        let bytes = parts.flat_map(|p| p.iter()).map(|m| m.bytes()).sum();
        (self.entries.len(), bytes)
    }

    /// The mesh for the display list of `size` bytes at `phys`, drawn with `state`'s vertex
    /// descriptor, formats and arrays: kept, or decoded through `mem` (the GPU's memory) and
    /// kept. None when it draws as the stream has it: a list or an array outside the loaded
    /// archives, a copy from the EFB over any of them, or a list a mesh can't stand for.
    pub fn get(
        &mut self,
        space: &Space,
        data: &Archives,
        state: &State,
        phys: u32,
        size: u32,
        mem: &dyn Memory,
    ) -> Option<Parts> {
        let Some(key) = Self::key(space, state, phys, size) else {
            self.stats.streamed += 1;
            return None;
        };
        let frame = self.frame;
        let (mesh, decoded) = match self.entries.get_mut(&key) {
            Some(e) => {
                e.used = frame;
                (e.mesh.clone(), false)
            }
            None => {
                let made = std::time::Instant::now();
                let mut list = vec![0; size as usize];
                match space.view(data, phys, size as usize) {
                    Some(bytes) => list.copy_from_slice(bytes),
                    // Rewritten: read with the game's rewrite.
                    None => space.read(data, phys, &mut list),
                }
                let part = if self.split { PART } else { usize::MAX };
                let mesh = Mesh::decode(&state.cp, &list, mem)
                    .map(|m| m.split(part).into_iter().map(Arc::new).collect::<Parts>());
                melee_gpu::made::add(melee_gpu::made::What::Mesh, made);
                let bytes = mesh
                    .as_ref()
                    .map_or(0, |p| p.iter().map(|m| m.bytes()).sum())
                    + size as usize;
                self.bytes += bytes;
                self.entries.insert(
                    key,
                    Entry {
                        mesh: mesh.clone(),
                        used: frame,
                        bytes,
                    },
                );
                (mesh, true)
            }
        };
        match mesh {
            Some(m) if Self::clear(space, &state.cp, &m[0]) => {
                if decoded {
                    self.stats.decoded += 1;
                } else {
                    self.stats.hits += 1;
                }
                Some(m)
            }
            _ => {
                self.stats.streamed += 1;
                None
            }
        }
    }

    /// The key of the list at `phys`, if it and its arrays are in the archives with no copy
    /// from the EFB over the list.
    fn key(space: &Space, state: &State, phys: u32, size: u32) -> Option<Key> {
        let (list, over) = space.locate(phys, size)?;
        if over {
            return None;
        }
        let cp = &state.cp;
        let kinds = kinds(cp);
        let mut arrays = [None; 12];
        for (a, slot) in arrays.iter_mut().enumerate() {
            if indexed(kinds[a]) {
                let base = cp[vertex::ARRAY_BASE as usize + a] & 0x03FF_FFFF;
                let stride = (cp[vertex::ARRAY_STRIDE as usize + a] & 0xFF) as u8;
                *slot = Some((space.locate(base, 0)?.0, stride));
            }
        }
        Some(Key {
            list,
            size,
            patch: space.patch(list).map(|p| (p.stride, p.pos, p.tex)),
            format: format_key(cp),
            arrays,
        })
    }

    /// Whether no copy from the EFB lies over what the mesh's indices reach of its arrays.
    fn clear(space: &Space, cp: &[u32; 256], mesh: &Mesh) -> bool {
        let kinds = kinds(cp);
        (0..12).all(|a| {
            !indexed(kinds[a]) || {
                let base = cp[vertex::ARRAY_BASE as usize + a] & 0x03FF_FFFF;
                matches!(space.locate(base, mesh.reach[a]), Some((_, false)))
            }
        })
    }
}

/// The cache's invalidation: a list's key changes with everything that changes what it draws.
/// Each check fails with that part of the key left out.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{ARCHIVES, Patch};
    use melee_data::archive::{Archive, ArchiveId};

    /// The GPU's memory as the renderer reads it.
    struct Host<'a> {
        space: &'a Space,
        data: &'a Archives,
    }

    impl Memory for Host<'_> {
        fn read(&self, phys: u32, out: &mut [u8]) {
            self.space.read(self.data, phys, out);
        }

        fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
            self.space.view(self.data, phys, len)
        }
    }

    /// An archive holding a display list of one triangle at 0 and an array of positions at
    /// 0x20, `scale` times (1, 0, 0), (0, 1, 0), (0, 0, 1). Each vertex sends a position matrix
    /// index (its number times 3), a texture matrix index (0) and its position's index.
    fn shape(scale: f32) -> Archive {
        let mut bytes = vec![0u8; 0x20 + 36];
        bytes[..3].copy_from_slice(&[0x90, 0, 3]);
        for v in 0..3u8 {
            let at = 3 + 3 * usize::from(v);
            bytes[at..at + 3].copy_from_slice(&[3 * v, 0, v]);
            let p = 0x20 + 12 * usize::from(v) + 4 * usize::from(v);
            bytes[p..p + 4].copy_from_slice(&scale.to_be_bytes());
        }
        Archive::raw(&bytes)
    }

    fn at(archive: ArchiveId, offset: u32) -> DataRef {
        DataRef { archive, offset }
    }

    /// The state drawing the shape in `archive`: matrix indices direct, positions indexed by
    /// a byte, three floats each.
    fn state(space: &Space, data: &Archives, archive: ArchiveId) -> State {
        let mut s = State::new();
        s.cp[vertex::VCD_LO as usize] = 1 | 1 << 1 | 2 << 9;
        s.cp[vertex::VAT_A as usize] = 1 | 4 << 1;
        s.cp[vertex::ARRAY_BASE as usize] = space.addr(data, at(archive, 0x20));
        s.cp[vertex::ARRAY_STRIDE as usize] = 12;
        s
    }

    fn mesh(meshes: &mut Meshes, space: &Space, data: &Archives, archive: ArchiveId) -> Arc<Mesh> {
        let s = state(space, data, archive);
        let list = space.addr(data, at(archive, 0));
        let mem = Host { space, data };
        meshes.get(space, data, &s, list, 32, &mem).expect("a mesh")[0].clone()
    }

    /// A list the game rewrites is a new mesh once rewritten: refraction copies each vertex's
    /// position matrix index over its texture matrix index (`lbRefract_PObjLoad`).
    #[test]
    fn a_rewritten_list_is_a_new_mesh() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let mut space = Space::default();
        let mut meshes = Meshes::default();
        let before = mesh(&mut meshes, &space, &data, a);
        assert_eq!(before.matrices[2][..2], [6, 0]);
        space.set_patches(vec![Patch {
            list: at(a, 0),
            len: 32,
            stride: 3,
            pos: 0,
            tex: 1,
        }]);
        let after = mesh(&mut meshes, &space, &data, a);
        assert_eq!(after.matrices[2][..2], [6, 6], "the rewritten list's mesh");
        assert_eq!(meshes.stats.decoded, 2);
    }

    /// An archive's range given back and taken by another archive (`mem::Space`), as a freed
    /// shape's list is replaced by another's at the same addresses, draws the new archive's
    /// shape.
    #[test]
    fn a_range_taken_by_another_archive_is_a_new_mesh() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let filler = data.add(Archive::raw(&vec![
            0;
            (ARCHIVES.end - ARCHIVES.start) as usize
                - 0x80
        ]));
        let b = data.add(shape(2.0));
        let space = Space::default();
        let mut meshes = Meshes::default();
        space.begin_frame(&[]);
        let first = mesh(&mut meshes, &space, &data, a);
        space.addr(&data, at(filler, 0));
        space.begin_frame(&[]);
        let second = mesh(&mut meshes, &space, &data, b);
        assert_eq!(
            space.addr(&data, at(b, 0)),
            ARCHIVES.start,
            "B took A's range"
        );
        assert_eq!(first.positions[0], [1.0, 0.0, 0.0]);
        assert_eq!(second.positions[0], [2.0, 0.0, 0.0], "B's shape");
    }

    /// The same list drawn with other arrays (another shape's vertex descriptions, as a shape
    /// sharing the list or a freed shape's place reused) is a new mesh.
    #[test]
    fn the_same_list_with_other_arrays_is_a_new_mesh() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let b = data.add(shape(3.0));
        let space = Space::default();
        let mut meshes = Meshes::default();
        let first = mesh(&mut meshes, &space, &data, a);
        let mut s = state(&space, &data, b);
        let list = space.addr(&data, at(a, 0));
        let mem = Host {
            space: &space,
            data: &data,
        };
        let second = meshes
            .get(&space, &data, &s, list, 32, &mem)
            .expect("a mesh")[0]
            .clone();
        assert_eq!(first.positions[1], [0.0, 1.0, 0.0]);
        assert_eq!(second.positions[1], [0.0, 3.0, 0.0], "B's positions");
        // And with the same arrays, the mesh kept.
        s.cp[vertex::ARRAY_BASE as usize] = space.addr(&data, at(a, 0x20));
        let third = meshes
            .get(&space, &data, &s, list, 32, &mem)
            .expect("a mesh")[0]
            .clone();
        assert!(Arc::ptr_eq(&first, &third));
    }

    /// A mesh nobody draws for a long while is kept while the cache is within its budget, so a
    /// shape shown again isn't decoded again; past the budget, the least recently drawn goes.
    #[test]
    fn meshes_are_kept_within_the_budget_and_the_oldest_go_past_it() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let b = data.add(shape(2.0));
        let space = Space::default();
        let mut meshes = Meshes::new(false);
        let first = mesh(&mut meshes, &space, &data, a);
        for _ in 0..10_000 {
            meshes.begin_frame();
        }
        assert!(Arc::ptr_eq(&first, &mesh(&mut meshes, &space, &data, a)));
        assert_eq!(meshes.stats.decoded, 1, "kept, not decoded again");
        // Room for one: B drawn every frame and A not, A goes and B stays.
        let second = mesh(&mut meshes, &space, &data, b);
        meshes.budget = meshes.bytes - 1;
        for _ in 0..CHECK_EVERY {
            meshes.begin_frame();
            mesh(&mut meshes, &space, &data, b);
        }
        assert_eq!(meshes.entries.len(), 1);
        assert!(Arc::ptr_eq(&second, &mesh(&mut meshes, &space, &data, b)));
        assert!(
            !Arc::ptr_eq(&first, &mesh(&mut meshes, &space, &data, a)),
            "A decoded again"
        );
    }

    /// A list drawn after shapes that left other formats for attributes it doesn't send is the
    /// same mesh; another format for one it sends is a new one.
    #[test]
    fn formats_of_attributes_not_sent_leave_the_mesh_as_it_is() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let space = Space::default();
        let mut meshes = Meshes::default();
        let first = mesh(&mut meshes, &space, &data, a);
        let list = space.addr(&data, at(a, 0));
        let mem = Host {
            space: &space,
            data: &data,
        };
        let mut s = state(&space, &data, a);
        // Color 0's format, texture coordinate 1's and 5's: none of them sent.
        s.cp[vertex::VAT_A as usize] |= 5 << 14;
        s.cp[vertex::VAT_B as usize] |= 0x1F0;
        s.cp[vertex::VAT_C as usize] |= 0x3E0;
        let again = meshes
            .get(&space, &data, &s, list, 32, &mem)
            .expect("a mesh");
        assert!(Arc::ptr_eq(&first, &again[0]), "the same mesh");
        // The positions' fraction: sent.
        s.cp[vertex::VAT_A as usize] |= 1 << 4;
        let other = meshes
            .get(&space, &data, &s, list, 32, &mem)
            .expect("a mesh");
        assert!(!Arc::ptr_eq(&first, &other[0]), "a new mesh");
    }

    /// A copy from the EFB written over a mesh's array: the list draws as the stream has it.
    #[test]
    fn a_copy_over_an_array_draws_from_the_stream() {
        let mut data = Archives::default();
        let a = data.add(shape(1.0));
        let space = Space::default();
        let mut meshes = Meshes::default();
        mesh(&mut meshes, &space, &data, a);
        let s = state(&space, &data, a);
        space.write(space.addr(&data, at(a, 0x24)), &[0; 4]);
        let list = space.addr(&data, at(a, 0));
        let mem = Host {
            space: &space,
            data: &data,
        };
        assert!(meshes.get(&space, &data, &s, list, 32, &mem).is_none());
    }
}
