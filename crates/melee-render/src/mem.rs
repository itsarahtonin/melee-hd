//! The address space the GPU reads from. In the original, display lists, vertex arrays,
//! textures and palettes live in main memory and GX names them by address. Here they live in
//! loaded archives (named by [`DataRef`]) and in images the presentation makes (EFB copies),
//! so each archive gets a range of addresses the first time something of it is drawn, and the
//! GPU's reads find their bytes through those ranges.

use std::cell::RefCell;

use melee_data::archive::{ArchiveId, Archives, DataRef};

/// Where archives' ranges start: above main memory's 24 MB, below the 26-bit physical limit
/// the GPU's address registers keep.
const ARCHIVE_BASE: u32 = 0x0180_0000;
/// The end of the archives' ranges, where images the presentation makes start.
const DYNAMIC_BASE: u32 = 0x0380_0000;
const DYNAMIC_END: u32 = 0x0400_0000;

/// An image the presentation makes, by what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dynamic {
    /// A shadow's texture, by the shadow's slot.
    Shadow(u32),
    /// Refraction's indirect map `n`.
    Refract(u8),
    /// Refraction's copy of the screen.
    RefractFrame,
    /// A magnifier bubble's copy of its fighter, by player.
    Magnifier(u8),
    /// Fountain of Dreams' reflection, the render its water shows.
    Reflection,
    /// One of Pokémon Stadium's screens' images, by its index (`ImageRef::Screen`).
    Screen(u8),
    /// `HSD_EraseRect`'s depth image: 4 by 4 texels of 0xFF.
    EraseDepth,
    /// Data of the executable the GPU reads, by its address there (the particles' texture
    /// coordinates).
    Dol(u32),
    /// The movie player's planes (`MoviePlayer.unk_50`, `unk_54`, `unk_58`): luma, then the
    /// two chroma planes, as the last frame decoded them.
    Movie(u8),
}

#[derive(Default)]
struct Ranges {
    /// Archives with a range: (start, end, archive), by start.
    archives: Vec<(u32, u32, ArchiveId)>,
    next: u32,
    /// Images the presentation makes: (start, size, what), with their bytes.
    dynamic: Vec<(u32, u32, Dynamic, Vec<u8>)>,
    next_dynamic: u32,
}

/// A display list `lbRefract_PObjLoad` rewrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patch {
    pub list: DataRef,
    pub len: u32,
    pub stride: u8,
    pub pos: u8,
    pub tex: u8,
}

/// The GPU's view of memory.
pub struct Space {
    ranges: RefCell<Ranges>,
    /// Display lists the game rewrote in place (refraction's matrix indices): each list, its
    /// length, and the vertex stride and the offsets of the two indices, as
    /// `PObjs::refract_patches` holds them, in order ([`Space::set_patches`]).
    patches: Vec<Patch>,
    /// The longest of them, and the archives they're in.
    patch_len: u32,
    patched_archives: Vec<ArchiveId>,
    /// The executable's sections at their physical addresses (`main.dol`, its static data
    /// such as the default font's glyphs).
    dol: Vec<(u32, Vec<u8>)>,
    /// Bytes copies from the EFB wrote over loaded data, by address.
    overlay: RefCell<Vec<(u32, Vec<u8>)>>,
}

impl Default for Space {
    fn default() -> Self {
        Space {
            ranges: RefCell::new(Ranges {
                next: ARCHIVE_BASE,
                next_dynamic: DYNAMIC_BASE,
                ..Ranges::default()
            }),
            patches: Default::default(),
            patch_len: 0,
            patched_archives: Vec::new(),
            dol: Vec::new(),
            overlay: RefCell::new(Vec::new()),
        }
    }
}

impl Space {
    /// The display lists the game rewrote, which reads see rewritten and views leave out.
    pub fn set_patches(&mut self, mut patches: Vec<Patch>) {
        patches.sort_by_key(|p| (p.list, p.len));
        patches.dedup();
        self.patch_len = patches.iter().map(|p| p.len).max().unwrap_or(0);
        self.patched_archives = patches.iter().map(|p| p.list.archive).collect();
        self.patched_archives.dedup();
        self.patches = patches;
    }

    /// Whether a rewritten display list lies over any of the bytes from `at` to `end` in
    /// `archive` (by offset): found among the lists in order, from the first that could reach
    /// `at`.
    fn patched(&self, archive: ArchiveId, at: u32, end: u64) -> bool {
        if !self.patched_archives.contains(&archive) {
            return false;
        }
        let ps = &self.patches;
        let from = at.saturating_sub(self.patch_len);
        let first = ps.partition_point(|p| (p.list.archive, p.list.offset) < (archive, from));
        ps[first..]
            .iter()
            .take_while(|p| p.list.archive == archive && u64::from(p.list.offset) < end)
            .any(|p| u64::from(p.list.offset) + u64::from(p.len) > u64::from(at))
    }

    /// The address of a place in an archive, giving the archive its range if it has none.
    pub fn addr(&self, data: &Archives, r: DataRef) -> u32 {
        let mut ranges = self.ranges.borrow_mut();
        if let Some(&(start, _, _)) = ranges.archives.iter().find(|a| a.2 == r.archive) {
            return start + r.offset;
        }
        let len = data.get(r.archive).data().len() as u32;
        let start = ranges.next;
        let end = start + len;
        assert!(end <= DYNAMIC_BASE, "the archives' address space is full");
        ranges.next = (end + 0x3F) & !0x1F;
        let at = ranges.archives.partition_point(|a| a.0 < start);
        ranges.archives.insert(at, (start, end, r.archive));
        start + r.offset
    }

    /// The address of an image the presentation makes, `size` bytes, making it the first
    /// time or when its size changes.
    pub fn dynamic(&self, what: Dynamic, size: u32) -> u32 {
        let mut ranges = self.ranges.borrow_mut();
        if let Some(d) = ranges.dynamic.iter().find(|d| d.2 == what)
            && d.1 >= size
        {
            return d.0;
        }
        let start = ranges.next_dynamic;
        assert!(
            start + size <= DYNAMIC_END,
            "the images' address space is full"
        );
        ranges.next_dynamic = (start + size + 0x3F) & !0x1F;
        ranges.dynamic.retain(|d| d.2 != what);
        ranges
            .dynamic
            .push((start, size, what, vec![0; size as usize]));
        start
    }

    /// Writes bytes the renderer copied back: into an image the presentation made, or over
    /// loaded data, which reads then see.
    pub fn write(&self, addr: u32, bytes: &[u8]) {
        let addr = addr & 0x03FF_FFFF;
        if addr < DYNAMIC_BASE {
            let mut o = self.overlay.borrow_mut();
            o.retain(|(a, b)| {
                !(*a >= addr && *a as usize + b.len() <= addr as usize + bytes.len())
            });
            o.push((addr, bytes.to_vec()));
            return;
        }
        let mut ranges = self.ranges.borrow_mut();
        for d in &mut ranges.dynamic {
            if addr >= d.0 && addr < d.0 + d.1 {
                let at = (addr - d.0) as usize;
                let n = bytes.len().min(d.3.len() - at);
                d.3[at..at + n].copy_from_slice(&bytes[..n]);
            }
        }
    }

    /// Maps the executable's text and data sections (`main.dol`'s header: 18 file offsets,
    /// load addresses and sizes) where the console loads them.
    pub fn load_dol(&mut self, dol: &[u8]) {
        let word = |at: usize| u32::from_be_bytes(dol[at..at + 4].try_into().unwrap());
        self.dol.clear();
        for i in 0..18 {
            let (off, addr, size) = (word(4 * i) as usize, word(0x48 + 4 * i), word(0x90 + 4 * i));
            if size == 0 || off + size as usize > dol.len() {
                continue;
            }
            self.dol
                .push((addr & 0x01FF_FFFF, dol[off..off + size as usize].to_vec()));
        }
    }

    /// The `len` bytes at an address in the executable's loaded sections, if they lie in one.
    pub fn dol_bytes(&self, addr: u32, len: usize) -> Option<&[u8]> {
        let phys = addr & 0x01FF_FFFF;
        self.dol.iter().find_map(|(start, bytes)| {
            let at = usize::try_from(phys.checked_sub(*start)?).ok()?;
            bytes.get(at..at.checked_add(len)?)
        })
    }

    /// The `len` bytes at a physical address in a loaded archive, where nothing lies over
    /// them (no copy from the EFB, no rewritten display list): what [`Space::read`] would
    /// copy out, without copying.
    pub fn view<'a>(&self, data: &'a Archives, phys: u32, len: usize) -> Option<&'a [u8]> {
        let phys = phys & 0x03FF_FFFF;
        if !(ARCHIVE_BASE..DYNAMIC_BASE).contains(&phys) {
            return None;
        }
        let (start, end, archive) = {
            let ranges = self.ranges.borrow();
            let i = ranges.archives.partition_point(|a| a.0 <= phys);
            *ranges.archives.get(i.checked_sub(1)?)?
        };
        let hi = phys as u64 + len as u64;
        if hi > u64::from(end) {
            return None;
        }
        let bytes = data.get(archive).data();
        let at = (phys - start) as usize;
        let out = bytes.get(at..at + len)?;
        let over =
            |a: u32, n: usize| u64::from(a) < hi && u64::from(a) + n as u64 > u64::from(phys);
        if self.overlay.borrow().iter().any(|(a, b)| over(*a, b.len())) {
            return None;
        }
        if self.patched(archive, at as u32, hi - u64::from(start)) {
            return None;
        }
        Some(out)
    }

    /// Reads what the GPU reads at a physical address: an archive's bytes (with the game's
    /// rewrites applied) or an image the presentation made; zeros elsewhere.
    pub fn read(&self, data: &Archives, phys: u32, out: &mut [u8]) {
        out.fill(0);
        let ranges = self.ranges.borrow();
        let phys = phys & 0x03FF_FFFF;
        if phys >= DYNAMIC_BASE {
            for d in &ranges.dynamic {
                if phys >= d.0 && phys < d.0 + d.1 {
                    let at = (phys - d.0) as usize;
                    let n = out.len().min(d.3.len() - at);
                    out[..n].copy_from_slice(&d.3[at..at + n]);
                }
            }
            return;
        }
        if phys < ARCHIVE_BASE {
            for (start, bytes) in &self.dol {
                if phys >= *start && ((phys - start) as usize) < bytes.len() {
                    let at = (phys - start) as usize;
                    let n = out.len().min(bytes.len() - at);
                    out[..n].copy_from_slice(&bytes[at..at + n]);
                }
            }
            return;
        }
        let i = ranges.archives.partition_point(|a| a.0 <= phys);
        if i == 0 {
            return;
        }
        let (start, end, archive) = ranges.archives[i - 1];
        if phys >= end {
            return;
        }
        let bytes = data.get(archive).data();
        let at = (phys - start) as usize;
        let n = out.len().min(bytes.len() - at);
        out[..n].copy_from_slice(&bytes[at..at + n]);
        for (a, b) in self.overlay.borrow().iter() {
            let (lo, hi) = (*a.max(&phys), (*a + b.len() as u32).min(phys + n as u32));
            if lo < hi {
                out[(lo - phys) as usize..(hi - phys) as usize]
                    .copy_from_slice(&b[(lo - a) as usize..(hi - a) as usize]);
            }
        }
        // The game's rewrites of refracting shapes' display lists.
        for p in &self.patches {
            if p.list.archive != archive {
                continue;
            }
            patch_display_list(bytes, p, at, &mut out[..n]);
        }
    }
}

/// Applies `lbRefract_PObjLoad`'s rewrite of the display list at `list` in `bytes` to `out`,
/// which holds `bytes[at..]`: in each primitive's vertices, the position-matrix index copied
/// over the texture-matrix index.
fn patch_display_list(bytes: &[u8], p: &Patch, at: usize, out: &mut [u8]) {
    let (stride, pos, tex) = (
        usize::from(p.stride),
        usize::from(p.pos),
        usize::from(p.tex),
    );
    let list = p.list.offset as usize;
    let end = (list + p.len as usize).min(bytes.len());
    if stride == 0 || at + out.len() <= list || end <= at {
        return;
    }
    // The original rewrites the list as it walks it, reading each vertex's position index
    // after the earlier vertices' rewrites; a vertex's two indices don't overlap another's.
    let mut q = list;
    while q < end {
        let op = bytes[q];
        q += 1;
        if op & 0xF8 == 0 || q + 2 > bytes.len() {
            break;
        }
        let count = usize::from(u16::from_be_bytes([bytes[q], bytes[q + 1]]));
        q += 2;
        for _ in 0..count {
            let (src, dst) = (q + pos, q + tex);
            if dst >= at && dst < at + out.len() && src < bytes.len() {
                out[dst - at] = bytes[src];
            }
            q += stride;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finding a rewritten list over a range finds what testing every list does.
    #[test]
    fn patched_finds_what_testing_every_list_does() {
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        let mut next = |n: u32| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 16) as u32 % n
        };
        for _ in 0..200 {
            let patches: Vec<Patch> = (0..next(40))
                .map(|_| Patch {
                    list: DataRef {
                        archive: ArchiveId(next(4) as u16),
                        offset: next(4000),
                    },
                    len: next(300),
                    stride: 1,
                    pos: 0,
                    tex: 0,
                })
                .collect();
            let mut space = Space::default();
            space.set_patches(patches.clone());
            for _ in 0..500 {
                let archive = ArchiveId(next(5) as u16);
                let at = next(4500);
                let end = u64::from(at) + 1 + u64::from(next(400));
                let want = patches.iter().any(|p| {
                    p.list.archive == archive
                        && u64::from(p.list.offset) < end
                        && u64::from(p.list.offset) + u64::from(p.len) > u64::from(at)
                });
                assert_eq!(space.patched(archive, at, end), want);
            }
        }
    }
}
