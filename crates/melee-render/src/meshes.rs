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

/// Frames a mesh nobody draws is kept.
const KEPT_FOR: u32 = 600;

/// What the GPU reads to draw a display list, by place in the archives.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    list: DataRef,
    size: u32,
    /// The rewrite of the list: the vertex stride and the two indices' offsets.
    patch: Option<(u8, u8, u8)>,
    /// The vertex descriptor (`VCD_LO`, `VCD_HI`) and format 0 (`VAT_A`, `B`, `C`).
    format: [u32; 5],
    /// Each indexed attribute's array and stride, by `vertex::ARRAY_POS` on.
    arrays: [Option<(DataRef, u8)>; 12],
}

struct Entry {
    /// None for a list a mesh can't stand for, which draws as the stream has it.
    mesh: Option<Parts>,
    used: u32,
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
    frame: u32,
    /// Whether a large list is drawn in parts ([`PART`]).
    split: bool,
    pub stats: Stats,
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

impl Meshes {
    /// A cache whose large lists are drawn in parts, with `split`.
    pub fn new(split: bool) -> Meshes {
        Meshes {
            split,
            ..Meshes::default()
        }
    }

    /// Starts a frame: meshes no frame drew for a while go.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        if self.frame.is_multiple_of(KEPT_FOR) {
            let frame = self.frame;
            self.entries
                .retain(|_, e| frame.wrapping_sub(e.used) < KEPT_FOR);
        }
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
                let mut list = vec![0; size as usize];
                match space.view(data, phys, size as usize) {
                    Some(bytes) => list.copy_from_slice(bytes),
                    // Rewritten: read with the game's rewrite.
                    None => space.read(data, phys, &mut list),
                }
                let part = if self.split { PART } else { usize::MAX };
                let mesh = Mesh::decode(&state.cp, &list, mem)
                    .map(|m| m.split(part).into_iter().map(Arc::new).collect::<Parts>());
                self.entries.insert(
                    key,
                    Entry {
                        mesh: mesh.clone(),
                        used: frame,
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
            format: [
                cp[vertex::VCD_LO as usize],
                cp[vertex::VCD_HI as usize],
                cp[vertex::VAT_A as usize],
                cp[vertex::VAT_B as usize],
                cp[vertex::VAT_C as usize],
            ],
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
