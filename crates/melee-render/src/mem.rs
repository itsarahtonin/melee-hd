//! The address space the GPU reads from. In the original, display lists, vertex arrays,
//! textures and palettes live in main memory and GX names them by address. Here they live in
//! loaded archives (named by [`DataRef`]) and in images the presentation makes (EFB copies),
//! so each archive gets a range of addresses the first time something of it is drawn, and the
//! GPU's reads find their bytes through those ranges.
//!
//! The archives' ranges are 32 MB in all, less than a long session draws (Adventure's, by its
//! Corneria intro), so once the space is full, the ranges of the archives the frame being
//! built hasn't drawn go back: such an archive gets a new range when it's drawn again, and
//! what the renderers kept of the old range by address goes ([`Space::reclaimed_since`],
//! `Gpu::run`). Until the space first fills, every archive keeps the range it was given.

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
    /// One of the results screen's images, by its index
    /// (`melee_sim::game::gm::results_fighters::image_addr`).
    Results(u8),
    /// Camera Mode's snapshot's copy of the frame (`cmsnap_data`'s image).
    Snapshot,
    /// The single-player clear screen's copy of the frame (`lbl_80472D28.x30`), which its
    /// blur draws back.
    ClearCopy,
    /// `HSD_EraseRect`'s depth image: 4 by 4 texels of 0xFF.
    EraseDepth,
    /// Data of the executable the GPU reads, by its address there (the particles' texture
    /// coordinates).
    Dol(u32),
    /// One of the album's images, by its place in the state's list
    /// (`melee_sim::lb::snap_read::SnapRead::images`).
    Album(u8),
    /// One of the trophy fall's images, by `melee_sim::game::gm::endings::toy_fall::IMAGES`'
    /// index.
    ToyFall(u8),
    /// Classic's horde intro's copies of the frame.
    IntroHorde(u8),
    /// The movie player's planes (`MoviePlayer.unk_50`, `unk_54`, `unk_58`): luma, then the
    /// two chroma planes, as the last frame decoded them.
    Movie(u8),
}

/// The archives' addresses.
pub const ARCHIVES: std::ops::Range<u32> = ARCHIVE_BASE..DYNAMIC_BASE;

/// How many of the latest ranges given back [`Space::reclaimed_since`] can name.
const RECLAIM_LOG: usize = 1024;

/// An archive's range, by the archive.
#[derive(Clone, Copy)]
struct Slot {
    start: u32,
    /// The last frame that drew something of it ([`Space::begin_frame`]).
    used: u32,
    /// Whether a copy from the EFB wrote into it: the copy lives at its addresses (in the
    /// renderers' memory, as in the console's), so the range stays.
    pinned: bool,
}

#[derive(Default)]
struct Ranges {
    /// Archives with a range: (start, end, archive), by start.
    archives: Vec<(u32, u32, ArchiveId)>,
    /// Each archive's range, by its id.
    slots: Vec<Option<Slot>>,
    next: u32,
    /// The frame being built.
    frame: u32,
    /// The latest ranges given back, (start, end), and how many went back in all.
    reclaimed: std::collections::VecDeque<(u32, u32)>,
    reclaims: u64,
    /// Images the presentation makes: (start, size, what), with their bytes.
    dynamic: Vec<(u32, u32, Dynamic, Vec<u8>)>,
    next_dynamic: u32,
    /// Images whose bytes start as what their block held (the results screen's): the
    /// leftovers last written into each, and whether a copy has written it since.
    seeds: Vec<(Dynamic, Seed, bool)>,
}

impl Ranges {
    /// Gives back the ranges of the archives the frame being built hasn't drawn, but those a
    /// copy went into, and moves the next range down to just past the highest kept.
    fn reclaim(&mut self) {
        let frame = self.frame;
        let Ranges {
            archives,
            slots,
            reclaimed,
            reclaims,
            ..
        } = self;
        archives.retain(|&(start, end, archive)| {
            let i = usize::from(archive.0);
            let Some(slot) = slots[i] else {
                return true;
            };
            if slot.used == frame || slot.pinned {
                return true;
            }
            slots[i] = None;
            if reclaimed.len() == RECLAIM_LOG {
                reclaimed.pop_front();
            }
            reclaimed.push_back((start, end));
            *reclaims += 1;
            false
        });
        self.next = self
            .archives
            .iter()
            .map(|a| (a.1 + 0x3F) & !0x1F)
            .max()
            .unwrap_or(ARCHIVE_BASE);
    }

    /// The lowest start of a gap between the ranges that holds `len` bytes, spaced from the
    /// range below as a new range is.
    fn first_fit(&self, len: u32) -> Option<u32> {
        let mut at = ARCHIVE_BASE;
        for &(start, end, _) in &self.archives {
            if u64::from(at) + u64::from(len) <= u64::from(start) {
                return Some(at);
            }
            at = at.max((end + 0x3F) & !0x1F);
        }
        (u64::from(at) + u64::from(len) <= u64::from(DYNAMIC_BASE)).then_some(at)
    }
}

/// What an image's block held when the game took it.
type Seed = std::sync::Arc<melee_sim::game::gm::results_fighters::ImageLeftovers>;

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

    /// Starts building a frame: the archives `keep` names (those whose addresses HSD's caches
    /// hold from the frames before, `prev_array`'s) count as drawn in it.
    pub fn begin_frame(&self, keep: &[Option<ArchiveId>]) {
        let mut ranges = self.ranges.borrow_mut();
        ranges.frame = ranges.frame.wrapping_add(1);
        let frame = ranges.frame;
        for a in keep.iter().flatten() {
            if let Some(Some(slot)) = ranges.slots.get_mut(usize::from(a.0)) {
                slot.used = frame;
            }
        }
    }

    /// The address of a place in an archive, giving the archive its range if it has none.
    pub fn addr(&self, data: &Archives, r: DataRef) -> u32 {
        let mut ranges = self.ranges.borrow_mut();
        let frame = ranges.frame;
        if let Some(Some(slot)) = ranges.slots.get_mut(usize::from(r.archive.0)) {
            slot.used = frame;
            return slot.start + r.offset;
        }
        let len = data.get(r.archive).data().len() as u32;
        let start = match ranges.next.checked_add(len) {
            Some(end) if end <= DYNAMIC_BASE => ranges.next,
            _ => {
                ranges.reclaim();
                ranges.first_fit(len).unwrap_or_else(|| {
                    panic!(
                        "the archives' address space is full: the frame draws more archives \
                         than its {} MB hold",
                        (DYNAMIC_BASE - ARCHIVE_BASE) >> 20
                    )
                })
            }
        };
        let end = start + len;
        ranges.next = ranges.next.max((end + 0x3F) & !0x1F);
        let at = ranges.archives.partition_point(|a| a.0 < start);
        ranges.archives.insert(at, (start, end, r.archive));
        let i = usize::from(r.archive.0);
        if ranges.slots.len() <= i {
            ranges.slots.resize(i + 1, None);
        }
        ranges.slots[i] = Some(Slot {
            start,
            used: frame,
            pinned: false,
        });
        start + r.offset
    }

    /// How many ranges have gone back so far.
    pub fn reclaims(&self) -> u64 {
        self.ranges.borrow().reclaims
    }

    /// The ranges (start, end) that went back since there had been `seen` ([`Space::reclaims`]),
    /// whose addresses may now hold another archive's bytes; `None` when that's more than the
    /// space remembers, so everything kept by an archive's address must go.
    pub fn reclaimed_since(&self, seen: u64) -> Option<Vec<(u32, u32)>> {
        let ranges = self.ranges.borrow();
        let n = usize::try_from(ranges.reclaims.checked_sub(seen)?).ok()?;
        let log = &ranges.reclaimed;
        (n <= log.len()).then(|| log.iter().skip(log.len() - n).copied().collect())
    }

    /// Keeps the range of the archive holding `phys`, if one does, for good: a copy from the
    /// EFB went there.
    pub fn pin_at(&self, phys: u32) {
        let phys = phys & 0x03FF_FFFF;
        let mut ranges = self.ranges.borrow_mut();
        let i = ranges.archives.partition_point(|a| a.0 <= phys);
        let Some(&(_, end, archive)) = i.checked_sub(1).and_then(|i| ranges.archives.get(i)) else {
            return;
        };
        if phys < end
            && let Some(Some(slot)) = ranges.slots.get_mut(usize::from(archive.0))
        {
            slot.pinned = true;
        }
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

    /// Starts image `what` (at `at`) as `seed`'s bytes, unless it already started as them: a
    /// new block the game took for it.
    pub fn seed(&self, what: Dynamic, at: u32, seed: &Seed) {
        {
            let mut ranges = self.ranges.borrow_mut();
            if ranges
                .seeds
                .iter()
                .any(|s| s.0 == what && std::sync::Arc::ptr_eq(&s.1, seed))
            {
                return;
            }
            ranges.seeds.retain(|s| s.0 != what);
            ranges.seeds.push((what, seed.clone(), false));
        }
        self.write(at, &seed.bytes);
    }

    /// Marks image `what` copied into since its seed.
    pub fn seed_copied(&self, what: Dynamic) {
        for s in &mut self.ranges.borrow_mut().seeds {
            if s.0 == what {
                s.2 = true;
            }
        }
    }

    /// Whether image `what` still shows its seed's bytes and the game didn't know them all.
    pub fn seed_unknown(&self, what: Dynamic) -> bool {
        self.ranges
            .borrow()
            .seeds
            .iter()
            .any(|s| s.0 == what && !s.2 && !s.1.known)
    }

    /// Writes bytes the renderer copied back: into an image the presentation made, or over
    /// loaded data, which reads then see.
    pub fn write(&self, addr: u32, bytes: &[u8]) {
        let addr = addr & 0x03FF_FFFF;
        if addr < DYNAMIC_BASE {
            self.pin_at(addr);
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
    use melee_data::archive::Archive;

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

    /// Archives of `n` MB each, every byte of the `i`th `i`.
    fn archives(count: usize, mb: usize) -> Archives {
        let mut data = Archives::default();
        for i in 0..count {
            data.add(Archive::raw(&vec![i as u8; mb << 20]));
        }
        data
    }

    fn at(archive: usize, offset: u32) -> DataRef {
        DataRef {
            archive: ArchiveId(archive as u16),
            offset,
        }
    }

    /// A session drawing 100 archives of 1 MB, two or three a frame, three times the 32 MB the
    /// archives' ranges hold: ranges go back once the space is full, every address stays in
    /// the space, each frame's archives keep theirs through the frame and read as their own
    /// bytes, and until the space first fills, ranges are the ones it always gave.
    #[test]
    fn ranges_go_back_once_the_space_is_full() {
        let data = archives(100, 1);
        let space = Space::default();
        let mut first = Vec::new();
        for frame in 0..400usize {
            space.begin_frame(&[]);
            let drawn: Vec<usize> = [frame % 100, (frame * 7 + 3) % 100, (frame / 3) % 100]
                .into_iter()
                .take(2 + frame % 2)
                .collect();
            let mut starts = Vec::new();
            for &a in &drawn {
                let start = space.addr(&data, at(a, 0));
                if space.reclaims() == 0 && !first.iter().any(|f: &(usize, u32)| f.0 == a) {
                    first.push((a, start));
                }
                starts.push(start);
            }
            for (&a, &start) in drawn.iter().zip(&starts) {
                assert!(ARCHIVES.contains(&start) && start + (1 << 20) <= ARCHIVES.end);
                // The same range all frame long, and the archive's bytes there.
                assert_eq!(space.addr(&data, at(a, 0x1234)), start + 0x1234);
                let mut b = [0xEE; 8];
                space.read(&data, start + 0x8_0000, &mut b);
                assert_eq!(b, [a as u8; 8]);
                assert_eq!(space.view(&data, start, 16), Some(&[a as u8; 16][..]));
            }
            for (i, &x) in starts.iter().enumerate() {
                for (j, &y) in starts.iter().enumerate() {
                    assert!(
                        drawn[i] == drawn[j] || x.abs_diff(y) >= 1 << 20,
                        "frame {frame}: archives {} and {} overlap",
                        drawn[i],
                        drawn[j]
                    );
                }
            }
        }
        assert!(
            space.reclaims() > 100,
            "{} ranges went back",
            space.reclaims()
        );
        // The ranges given before the space filled: one after another, 32 bytes apart.
        first.sort_by_key(|f| f.1);
        for (i, f) in first.iter().enumerate() {
            assert_eq!(f.1, ARCHIVE_BASE + i as u32 * ((1 << 20) + 0x20));
        }
        assert_eq!(first.len(), 31);
    }

    /// Ranges given back are named by [`Space::reclaimed_since`] in order, and a range a copy
    /// wrote into, or one HSD's caches hold (`begin_frame`'s), stays.
    #[test]
    fn copied_and_kept_ranges_stay() {
        let data = archives(40, 1);
        let space = Space::default();
        space.begin_frame(&[]);
        let copied = space.addr(&data, at(0, 0));
        space.write(copied + 0x100, &[0xAB; 4]);
        let kept = space.addr(&data, at(1, 0));
        let mut gone = Vec::new();
        for a in 2..40 {
            space.begin_frame(&[Some(ArchiveId(1))]);
            let seen = space.reclaims();
            let start = space.addr(&data, at(a, 0));
            for (s, e) in space.reclaimed_since(seen).expect("the latest ranges") {
                assert!(s >= ARCHIVE_BASE && e <= DYNAMIC_BASE && e - s == 1 << 20);
                assert!(s != copied && s != kept);
                gone.push(s);
            }
            assert!(start != copied && start != kept);
        }
        assert!(!gone.is_empty());
        assert_eq!(space.reclaimed_since(0).map(|r| r.len()), Some(gone.len()));
        assert_eq!(space.addr(&data, at(0, 0)), copied);
        assert_eq!(space.addr(&data, at(1, 0)), kept);
        let mut b = [0; 4];
        space.read(&data, copied + 0x100, &mut b);
        assert_eq!(b, [0xAB; 4]);
    }

    /// A frame that draws more than the space holds stops by name.
    #[test]
    #[should_panic(expected = "the archives' address space is full")]
    fn a_frame_drawing_more_than_the_space_stops() {
        let data = archives(40, 1);
        let space = Space::default();
        space.begin_frame(&[]);
        for a in 0..40 {
            space.addr(&data, at(a, 0));
        }
    }
}
