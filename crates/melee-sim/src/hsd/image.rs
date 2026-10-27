//! What main memory holds where the scene heaps are, as melee-sim knows it: a session's match
//! takes blocks the menus' scenes used, and every field its creation doesn't write holds those
//! scenes' bytes (`docs/engine.md`, "A session's heap leftovers"). melee-sim's objects are
//! typed, so the image holds runs of where the bytes come from rather than the bytes: a file's
//! (an archive as `HSD_ArchiveParse` relocated it where it was loaded, a movie's frames as
//! they were read), an object's as the game lays it out ([`super::export`]), the allocators'
//! own words (cells' headers, free lists' links), or unknown.
//!
//! An allocation writes nothing but the allocator's words; its owner then writes its part of
//! the block, which the image takes as unknown (a claim) until the owner says what it wrote:
//! a file loaded into it, or the object as it goes (`HSD_ObjFree`, `hsdFreeMemPiece`,
//! `OSFreeToHeap`) or as the scene's heap is made anew with the object still in it. A byte no
//! owner claimed keeps what it held. A new object takes the bytes the image has at its
//! address for the fields its creation doesn't write ([`crate::game::leftovers`]), an unknown
//! byte reading zero; `melee-oracle session --image` checks the image against memory.
//!
//! It's state, built from the state alone: the boot clears the arena, so memory nobody wrote
//! is zero. It's kept only in a session from power-on (`on`): a replay's heap is fresh and
//! Slippi's playback zeroes the fighters' and stage objects' blocks anyway, and an image that
//! isn't kept costs nothing and adds nothing to a state's hash. Its runs are kept by page of
//! main memory, each page shared between snapshots until something writes it, so a snapshot
//! copies lists of pages; what the scenes before left is one layer, the scene's own changes
//! two beside it, which the scene's end folds in.

use std::collections::BTreeMap;
use std::sync::Arc;

use melee_data::archive::{ArchiveId, Archives};

use crate::state_eq::{Diff, StateEq, StateHasher};

/// The owner of a free cell's header, which the heap's lists write ([`Image::unknown`]).
pub const CELL_HEADER: &str = "a heap cell's header";
/// The owner of an `HSD_MemAlloc` block that names no type of its own.
pub const HEAP_BLOCK: &str = "a main-heap block";
/// The owner of a memory piece that names no class or pool.
pub const PIECE: &str = "a memory piece";

/// An object made in a block where the image didn't know some of the bytes its creation
/// leaves as they were: which, by their owners. A new object reads those as zero; where the
/// original's differ, the oracle stops there, naming the owner whose layout it lacks, rather
/// than report a difference (docs/engine.md, a session's heap leftovers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unlaid {
    /// The object's address.
    pub at: u32,
    /// Its type.
    pub ty: &'static str,
    /// The unknown runs, as offsets into the object, each with its owner.
    pub runs: Vec<(u32, u32, &'static str)>,
}

/// Where a run of the image's bytes comes from, its first byte's place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Src {
    /// Bytes an owner wrote that melee-sim doesn't lay out: the owner's type (an object
    /// pool's, a class's, a named block's), which says whose layout a new object over them
    /// needs ([`Image::unlaid`]).
    Unknown(&'static str),
    /// A file read into memory, from byte `offset` of `archive`'s file as read (header first,
    /// `Archive::file_byte`); with a `base`, the words its relocation table names hold the
    /// data block's address `base` added, as `HSD_ArchiveParse` leaves them.
    File {
        archive: ArchiveId,
        offset: u32,
        base: u32,
    },
    /// A file that isn't an archive (the movies'), from byte `offset` of its data.
    Raw { archive: ArchiveId, offset: u32 },
    /// Bytes an object laid out as the game keeps it, from `offset` of `bytes`.
    Bytes { bytes: Arc<[u8]>, offset: u32 },
    /// Pixels the GPU copied into a buffer (`GXCopyTex`: the magnifier's bubbles): the
    /// renderer's output, which isn't simulation state and never reaches it. Unknown as bytes;
    /// a new object over them takes the image's fill there ([`Image::render_fill`]), and
    /// the comparer leaves such a byte out while it still holds the fill (docs/engine.md,
    /// "Uninitialized values").
    RenderTarget,
}

impl Src {
    fn advance(&self, n: u32) -> Src {
        match self {
            &Src::Unknown(o) => Src::Unknown(o),
            Src::RenderTarget => Src::RenderTarget,
            &Src::File {
                archive,
                offset,
                base,
            } => Src::File {
                archive,
                offset: offset + n,
                base,
            },
            &Src::Raw { archive, offset } => Src::Raw {
                archive,
                offset: offset + n,
            },
            Src::Bytes { bytes, offset } => Src::Bytes {
                bytes: bytes.clone(),
                offset: offset + n,
            },
        }
    }

    /// Whether the bytes are unknown.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Src::Unknown(_))
    }

    /// The `i`th byte from the source's place, if known.
    fn byte(&self, data: &Archives, i: u32) -> Option<u8> {
        match self {
            Src::Unknown(_) | Src::RenderTarget => None,
            &Src::File {
                archive,
                offset,
                base,
            } => file_byte(data, archive, offset + i, base),
            &Src::Raw { archive, offset } => {
                data.get(archive).data().get((offset + i) as usize).copied()
            }
            Src::Bytes { bytes, offset } => bytes.get((offset + i) as usize).copied(),
        }
    }
}

/// Byte `at` of an archive's file in memory: as read, or with its relocated words holding the
/// data block's address (`base`) added.
fn file_byte(data: &Archives, archive: ArchiveId, at: u32, base: u32) -> Option<u8> {
    let a = data.get(archive);
    let raw = a.file_byte(at)?;
    if base == 0 || at < 0x20 {
        return Some(raw);
    }
    let d = at - 0x20;
    if d as usize >= a.data().len() {
        return Some(raw);
    }
    // A relocated word covering the byte: the four bytes there hold the word plus `base`.
    let relocs = a.relocs();
    let from = relocs.partition_point(|&r| r + 3 < d);
    match relocs.get(from) {
        Some(&r) if r <= d => {
            let w = (0..4).try_fold(0u32, |w, k| {
                a.file_byte(0x20 + r + k).map(|b| w << 8 | u32::from(b))
            })?;
            Some(w.wrapping_add(base).to_be_bytes()[(d - r) as usize])
        }
        _ => Some(raw),
    }
}

/// A run of the image: its end and where its bytes come from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Span {
    end: u32,
    src: Src,
}

/// Runs by start within one page of main memory, none overlapping.
type Page = BTreeMap<u32, Span>;

/// A page's size, as a shift: no run crosses a page's end, so a snapshot shares every page
/// nothing has written since.
const PAGE_SHIFT: u32 = 16;

/// Runs by start, none overlapping, kept by 64 KiB page of main memory (none crossing a
/// page's end), each page shared between snapshots until something writes it; memory no run
/// covers is zero.
#[derive(Debug, Default)]
struct Spans {
    pages: Vec<Option<Arc<Page>>>,
}

impl Clone for Spans {
    fn clone(&self) -> Self {
        Spans {
            pages: self.pages.clone(),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.pages.clone_from(&other.pages);
    }
}

impl PartialEq for Spans {
    fn eq(&self, other: &Self) -> bool {
        let n = self.pages.len().max(other.pages.len());
        (0..n).all(|i| match (self.page(i), other.page(i)) {
            (Some(a), Some(b)) => std::ptr::eq(a, b) || a == b,
            (Some(p), None) | (None, Some(p)) => p.is_empty(),
            (None, None) => true,
        })
    }
}

impl Eq for Spans {}

/// The page `a` is in.
fn page_of(a: u32) -> usize {
    (a.wrapping_sub(MEM1.start) >> PAGE_SHIFT) as usize
}

/// The end of the page `a` is in.
fn page_end(a: u32) -> u32 {
    (a | ((1 << PAGE_SHIFT) - 1)).wrapping_add(1)
}

impl Spans {
    fn page(&self, i: usize) -> Option<&Page> {
        self.pages.get(i).and_then(|p| p.as_deref())
    }

    /// Page `i` to write, copied first if a snapshot shares it.
    fn page_mut(&mut self, i: usize) -> &mut Page {
        if self.pages.len() <= i {
            self.pages.resize(i + 1, None);
        }
        Arc::make_mut(self.pages[i].get_or_insert_with(Default::default))
    }

    /// The runs overlapping `[lo, hi)`, by start.
    fn range(&self, lo: u32, hi: u32) -> impl Iterator<Item = (u32, &Span)> + '_ {
        let last = if hi > lo { page_of(hi - 1) } else { 0 };
        let pages = if hi > lo && MEM1.contains(&lo) {
            page_of(lo)..last + 1
        } else {
            0..0
        };
        pages
            .filter_map(move |i| self.page(i))
            .flat_map(move |p| p.range(first(p, lo)..hi).map(|(&k, s)| (k, s)))
    }

    /// Every run, by start.
    fn iter(&self) -> impl Iterator<Item = (u32, &Span)> + '_ {
        self.pages
            .iter()
            .flatten()
            .flat_map(|p| p.iter().map(|(&k, s)| (k, s)))
    }

    fn len(&self) -> usize {
        self.pages.iter().flatten().map(|p| p.len()).sum()
    }

    fn is_empty(&self) -> bool {
        self.pages.iter().flatten().all(|p| p.is_empty())
    }
}

/// The start of the run `lo` falls in, else `lo`.
fn first(spans: &Page, lo: u32) -> u32 {
    spans
        .range(..lo)
        .next_back()
        .filter(|(_, s)| s.end > lo)
        .map(|(&k, _)| k)
        .unwrap_or(lo)
}

/// Cuts `[lo, hi)` out of `spans`, keeping what the runs it overlaps hold either side.
fn cut(spans: &mut Page, lo: u32, hi: u32) {
    let hit: Vec<u32> = spans.range(first(spans, lo)..hi).map(|(&k, _)| k).collect();
    for k in hit {
        let s = spans.remove(&k).unwrap();
        if k < lo {
            spans.insert(
                k,
                Span {
                    end: lo,
                    src: s.src.clone(),
                },
            );
        }
        if s.end > hi {
            spans.insert(
                hi,
                Span {
                    end: s.end,
                    src: s.src.advance(hi - k),
                },
            );
        }
    }
}

/// Writes `src` over `[lo, hi)` of one page.
fn write_page(spans: &mut Page, lo: u32, hi: u32, src: Src) {
    cut(spans, lo, hi);
    // An unknown run joins an unknown neighbour of the same owner.
    let (mut lo, mut hi) = (lo, hi);
    if src.is_unknown() {
        if let Some((&k, s)) = spans.range(..lo).next_back()
            && s.end == lo
            && s.src == src
        {
            spans.remove(&k);
            lo = k;
        }
        if spans.get(&hi).is_some_and(|s| s.src == src) {
            hi = spans.remove(&hi).unwrap().end;
        }
    }
    spans.insert(lo, Span { end: hi, src });
}

/// Writes `src` over `[lo, hi)` of `spans`, page by page (main memory only).
fn write(spans: &mut Spans, lo: u32, hi: u32, src: Src) {
    let hi = hi.min(MEM1.end);
    if lo >= hi || !MEM1.contains(&lo) {
        return;
    }
    let mut a = lo;
    while a < hi {
        let b = page_end(a).min(hi);
        write_page(spans.page_mut(page_of(a)), a, b, src.advance(a - lo));
        a = b;
    }
}

/// The runs of `spans` within `[lo, hi)`, cut to it, which `spans` then no longer holds.
fn take(spans: &mut Spans, lo: u32, hi: u32) -> Vec<(u32, u32, Src)> {
    let hi = hi.min(MEM1.end);
    let out: Vec<(u32, u32, Src)> = spans
        .range(lo, hi)
        .map(|(k, s)| {
            let from = k.max(lo);
            (from, s.end.min(hi), s.src.advance(from - k))
        })
        .collect();
    let mut a = lo;
    while a < hi && MEM1.contains(&a) {
        let b = page_end(a).min(hi);
        if spans.page(page_of(a)).is_some_and(|p| !p.is_empty()) {
            cut(spans.page_mut(page_of(a)), a, b);
        }
        a = b;
    }
    out
}

/// The bytes of `[lo, lo + out.len())` in `runs` over what `out` and `known` hold.
fn read_runs<'a>(
    runs: impl Iterator<Item = (u32, &'a Span)>,
    data: &Archives,
    lo: u32,
    out: &mut [u8],
    known: &mut [bool],
) {
    let hi = lo + out.len() as u32;
    for (k, s) in runs {
        for a in k.max(lo)..s.end.min(hi) {
            let i = (a - lo) as usize;
            match s.src.byte(data, a - k) {
                Some(b) => {
                    out[i] = b;
                    known[i] = true;
                }
                None => {
                    out[i] = 0;
                    known[i] = false;
                }
            }
        }
    }
}

/// The bytes of `[lo, lo + out.len())` in `spans` over what `out` and `known` hold.
fn read(spans: &Spans, data: &Archives, lo: u32, out: &mut [u8], known: &mut [bool]) {
    let hi = lo + out.len() as u32;
    read_runs(spans.range(lo, hi), data, lo, out, known);
}

/// The heap's memory as melee-sim knows it (see the module's notes).
#[derive(Debug, Default)]
pub struct Image {
    /// Whether it's kept: a session from power-on.
    pub on: bool,
    /// What the scenes before this one left, its pages shared between snapshots (an image not
    /// kept has none and allocates nothing, so a step's loan of the heap stays free).
    base: Spans,
    /// What the blocks this scene freed held, over the base.
    overlay: Spans,
    /// What the blocks allocated now hold, as far as the image knows: their owners' claims,
    /// the files loaded into them, the objects laid out as they went, the allocators' words.
    /// A block freed takes its runs to the overlay, and the scene's end all of them.
    live: Spans,
    /// Where archives were relocated to (their data blocks' addresses and lengths), newest
    /// last: what a pointer the image holds into one names.
    places: Option<Arc<Vec<(u32, u32, ArchiveId)>>>,
    /// Pointer words outside main memory that a leftover handle stands for
    /// ([`Image::unknown_handle`]).
    words: Vec<u32>,
    /// The object a pool or a size class of pieces last handed out and the free list's link
    /// its first word still holds, which its owner's claim hides from the image.
    taken: Option<(u32, u32)>,
    /// HSD's ID table's entries now, oldest first: each goes on the front of its hash chain
    /// (`HSD_IDInsertToTable`), so the order gives the chains' links.
    id_order: Vec<u32>,
    /// The parts of the next piece its owner writes, if not all ([`Image::claim_next`]), as
    /// offsets.
    claim_parts: Option<Vec<(u32, u32)>>,
    /// Whether the last object an `HSD_ObjAlloc` pool handed out came new from the heap (the
    /// pool grew), where the image knows its block, or again from the pool's free list.
    pub pool_grew: bool,
    /// The pools' slots this scene allocated, by the pool's object type: the objects laid out
    /// as the scene's heap is made anew (an older scene's live objects are its heap's).
    /// Sorted, so a snapshot copies it whole.
    scene_slots: Vec<(&'static str, u32)>,
    /// The scene heap's allocated cells, oldest first: the order of `OSAllocFromHeap`'s
    /// allocated list, which the cells' headers link.
    alloc_order: Vec<u32>,
    /// What a new object takes where its block held render-target bytes ([`Src::RenderTarget`]):
    /// 0, or another value for the oracle's mutation run (`--mutate render-target=ff`), which
    /// shows any read of them before a write.
    pub render_fill: u8,
    /// The objects this scene made over bytes the image doesn't know ([`Unlaid`]), until
    /// their blocks go back.
    unlaid: Vec<Unlaid>,
    /// Copies of an archive's file a buffer holds besides its places (a fighter's animation
    /// in its animation buffer), newest last: where the objects laid out as they go point
    /// into ([`Image::lay_addr`]), which the comparison's own places don't take.
    copies: Option<Arc<Vec<(u32, u32, ArchiveId)>>>,
    /// [`Image::set_copy_hint`]'s copy, set only while a figatree binds.
    copy_hint: Option<u32>,
}

impl Clone for Image {
    fn clone(&self) -> Self {
        let mut c = Image::default();
        c.clone_from(self);
        c
    }

    /// A restore: each part copied into what the target already holds.
    fn clone_from(&mut self, o: &Self) {
        self.on = o.on;
        self.base.clone_from(&o.base);
        self.overlay.clone_from(&o.overlay);
        self.live.clone_from(&o.live);
        self.places.clone_from(&o.places);
        self.words.clone_from(&o.words);
        self.taken = o.taken;
        self.id_order.clone_from(&o.id_order);
        self.claim_parts.clone_from(&o.claim_parts);
        self.pool_grew = o.pool_grew;
        self.scene_slots.clone_from(&o.scene_slots);
        self.alloc_order.clone_from(&o.alloc_order);
        self.render_fill = o.render_fill;
        self.unlaid.clone_from(&o.unlaid);
        self.copies.clone_from(&o.copies);
    }
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.on == other.on
            && (!self.on
                || (self.base() == other.base()
                    && self.overlay == other.overlay
                    && self.live == other.live
                    && self.places() == other.places()
                    && self.words == other.words
                    && self.taken == other.taken
                    && self.id_order == other.id_order
                    && self.claim_parts == other.claim_parts
                    && self.pool_grew == other.pool_grew
                    && self.scene_slots == other.scene_slots
                    && self.alloc_order == other.alloc_order
                    && self.render_fill == other.render_fill
                    && self.unlaid == other.unlaid
                    && self.copies() == other.copies()))
    }
}

impl Eq for Image {}

fn hash_spans(spans: &Spans, h: &mut StateHasher) {
    h.word(spans.len() as u64);
    for (k, s) in spans.iter() {
        h.word(u64::from(k) << 32 | u64::from(s.end));
        match &s.src {
            Src::Unknown(o) => {
                h.word(0);
                h.bytes(o.as_bytes());
            }
            Src::File {
                archive,
                offset,
                base,
            } => {
                h.word(1 << 48 | u64::from(archive.0) << 32 | u64::from(*offset));
                h.word(u64::from(*base));
            }
            Src::Raw { archive, offset } => {
                h.word(2 << 48 | u64::from(archive.0) << 32 | u64::from(*offset))
            }
            Src::Bytes { bytes, offset } => {
                h.word(3 << 48 | u64::from(*offset));
                h.bytes(bytes);
            }
            Src::RenderTarget => h.word(4 << 48),
        }
    }
}

impl StateEq for Image {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (self != other).then(|| {
            Diff::new(format!(
                "the heap's images differ (on {} and {}, {} and {} runs)",
                self.on,
                other.on,
                self.base().len() + self.overlay.len() + self.live.len(),
                other.base().len() + other.overlay.len() + other.live.len()
            ))
        })
    }

    fn state_hash(&self, h: &mut StateHasher) {
        // Not kept: nothing, so states without one hash as before it existed.
        if !self.on {
            return;
        }
        hash_spans(self.base(), h);
        hash_spans(&self.overlay, h);
        hash_spans(&self.live, h);
        h.word(self.places().len() as u64);
        for &(b, len, a) in self.places() {
            h.word(u64::from(b) << 32 | u64::from(len));
            h.word(u64::from(a.0));
        }
        h.word(self.words.len() as u64);
        for &w in &self.words {
            h.word(u64::from(w));
        }
        let (a, w) = self.taken.unwrap_or((0, 0));
        h.word(u64::from(a) << 32 | u64::from(w));
        for &(a, b) in self.claim_parts.iter().flatten() {
            h.word(u64::from(a) << 32 | u64::from(b));
        }
        h.word(self.id_order.len() as u64);
        for &e in &self.id_order {
            h.word(u64::from(e));
        }
        h.word(u64::from(self.pool_grew));
        h.word(self.scene_slots.len() as u64);
        for &(k, s) in &self.scene_slots {
            h.bytes(k.as_bytes());
            h.word(u64::from(s));
        }
        h.word(self.alloc_order.len() as u64);
        for &c in &self.alloc_order {
            h.word(u64::from(c));
        }
        h.word(u64::from(self.render_fill));
        h.word(self.copies().len() as u64);
        for &(b, len, a) in self.copies() {
            h.word(u64::from(b) << 32 | u64::from(len));
            h.word(u64::from(a.0));
        }
        h.word(self.unlaid.len() as u64);
        for u in &self.unlaid {
            h.word(u64::from(u.at));
            h.bytes(u.ty.as_bytes());
            for &(a, b, o) in &u.runs {
                h.word(u64::from(a) << 32 | u64::from(b));
                h.bytes(o.as_bytes());
            }
        }
    }
}

/// The handle a leftover pointer word into main memory stands as ([`Image::unknown_handle`]),
/// none for another word.
pub fn leftover_handle(word: u32) -> Option<u32> {
    MEM1.contains(&word)
        .then(|| super::pool::UNKNOWN | (word - MEM1.start))
}

/// The handle a leftover pointer word of any value stands as, from the heap's image while a
/// step has it lent ([`Image::unknown_handle`]): none outside a session's step, where a word
/// into main memory still has its handle.
pub fn leftover_word(word: u32) -> Option<u32> {
    super::heap::with(|h| h.image.on.then(|| h.image.unknown_handle(word)))
        .flatten()
        .or_else(|| leftover_handle(word))
}

/// Main memory's range.
const MEM1: std::ops::Range<u32> = 0x8000_0000..0x8180_0000;
/// The handles past every pool that stand for a leftover word ([`super::pool::UNKNOWN`]): one
/// in main memory carries its offset there, another indexes [`Image::words`].
const WORD_TABLE: u32 = 0x0800_0000;

/// No places: what an image starts from.
static NO_PLACES: Vec<(u32, u32, ArchiveId)> = Vec::new();

impl Image {
    fn base(&self) -> &Spans {
        &self.base
    }

    fn places(&self) -> &[(u32, u32, ArchiveId)] {
        self.places.as_deref().map_or(&NO_PLACES, |p| p)
    }

    fn copies(&self) -> &[(u32, u32, ArchiveId)] {
        self.copies.as_deref().map_or(&NO_PLACES, |p| p)
    }

    /// A copy of an archive's file's data (`len` bytes at `base`) a buffer holds now: where
    /// the objects laid out as they go point into it ([`Image::lay_addr`]).
    pub fn place_copy(&mut self, base: u32, len: u32, archive: ArchiveId) {
        if self.on {
            let c = Arc::make_mut(self.copies.get_or_insert_with(Default::default));
            // An object pointing into an earlier copy at the same place may still go after
            // the next is loaded there (an animation's FObjs, as the next animation starts),
            // so every copy is kept, and one copied into two places is ambiguous.
            c.retain(|&(b, _, a)| b != base || a != archive);
            c.push((base, len, archive));
        }
    }

    /// The copy the objects made now point into, where it holds their archive (a fighter's
    /// own animation buffer while its figatree binds: another fighter's buffer can hold a copy
    /// of the same archive), or none for the newest copy.
    pub fn set_copy_hint(&mut self, base: Option<u32>) {
        self.copy_hint = base;
    }

    /// Where the newest copy of an archive's file's data is ([`Image::place_copy`]).
    pub fn copy_at(&self, archive: ArchiveId) -> Option<u32> {
        if !self.on {
            return None;
        }
        if let Some(b) = self.copy_hint
            && self
                .copies()
                .iter()
                .any(|&(c, _, a)| c == b && a == archive)
        {
            return Some(b);
        }
        self.copies()
            .iter()
            .rev()
            .find(|&&(_, _, a)| a == archive)
            .map(|&(b, _, _)| b)
    }

    /// The address a layout gives a place in an archive: its copy ([`Image::place_copy`]),
    /// unknown where two places hold one, else its newest place.
    pub fn lay_addr(&self, d: melee_data::archive::DataRef) -> Option<u32> {
        // Copies in two places (a fighter's two animation buffers holding the same
        // animation) leave the object's place unknown.
        let mut bases = self
            .copies()
            .iter()
            .filter(|&&(_, len, a)| a == d.archive && d.offset < len)
            .map(|&(b, _, _)| b);
        match bases.next() {
            Some(b) => bases.all(|o| o == b).then_some(b + d.offset),
            None => self.data_addr(d),
        }
    }

    /// A session's image from power-on: the boot cleared the arena.
    pub fn power_on() -> Image {
        Image {
            on: true,
            ..Image::default()
        }
    }

    /// Writes `src` over `[lo, hi)` of memory no block holds now.
    pub fn write(&mut self, lo: u32, hi: u32, src: Src) {
        if self.on {
            write(&mut self.overlay, lo, hi, src);
        }
    }

    /// Marks `[lo, hi)` of memory no block holds now unknown (a free cell's header).
    pub fn unknown(&mut self, lo: u32, hi: u32) {
        self.write(lo, hi, Src::Unknown(CELL_HEADER));
    }

    /// Bytes `[lo, lo + len)` and which of them are known (unknown ones read zero).
    pub fn bytes(&self, data: &Archives, lo: u32, len: u32) -> (Vec<u8>, Vec<bool>) {
        let mut out = vec![0; len as usize];
        let mut known = vec![true; len as usize];
        if self.on {
            read(self.base(), data, lo, &mut out, &mut known);
            read(&self.overlay, data, lo, &mut out, &mut known);
            read(&self.live, data, lo, &mut out, &mut known);
        }
        (out, known)
    }

    /// The byte at `at` as the image has it now, if known.
    pub fn byte_at(&self, data: &Archives, at: u32) -> Option<u8> {
        let (b, k) = self.bytes(data, at, 1);
        k[0].then_some(b[0])
    }

    /// Bytes `[lo, lo + len)` as a new object's block held them before its owner wrote it:
    /// what the scenes left, with the allocators' writes since (its free list's link), not its
    /// owner's claim on it.
    pub fn leftover_bytes(&self, data: &Archives, lo: u32, len: u32) -> (Vec<u8>, Vec<bool>) {
        let mut out = vec![0; len as usize];
        let mut known = vec![true; len as usize];
        if self.on {
            read(self.base(), data, lo, &mut out, &mut known);
            read(&self.overlay, data, lo, &mut out, &mut known);
            let written = self
                .live
                .range(lo, lo + len)
                .filter(|(_, s)| !s.src.is_unknown());
            read_runs(written, data, lo, &mut out, &mut known);
            if let Some((a, w)) = self.taken
                && a == lo
                && len >= 4
            {
                out[..4].copy_from_slice(&w.to_be_bytes());
                known[..4].fill(true);
            }
        }
        (out, known)
    }

    /// The runs of `[lo, lo + len)` a new object's block holds unknown, as
    /// [`Image::leftover_bytes`] reads the block: offsets into it, each with its owner.
    pub fn leftover_unknown(&self, lo: u32, len: u32) -> Vec<(u32, u32, &'static str)> {
        let mut owner: Vec<Option<&'static str>> = vec![None; len as usize];
        if !self.on {
            return Vec::new();
        }
        let hi = lo + len;
        let mut mark = |runs: &mut dyn Iterator<Item = (u32, &Span)>| {
            for (k, s) in runs {
                for a in k.max(lo)..s.end.min(hi) {
                    owner[(a - lo) as usize] = match s.src {
                        Src::Unknown(o) => Some(o),
                        _ => None,
                    };
                }
            }
        };
        mark(&mut self.base().range(lo, hi));
        mark(&mut self.overlay.range(lo, hi));
        mark(&mut self.live.range(lo, hi).filter(|(_, s)| !s.src.is_unknown()));
        if let Some((a, _)) = self.taken
            && a == lo
        {
            owner.iter_mut().take(4).for_each(|o| *o = None);
        }
        let mut out: Vec<(u32, u32, &'static str)> = Vec::new();
        for (i, o) in owner.into_iter().enumerate() {
            let Some(o) = o else { continue };
            match out.last_mut() {
                Some((_, end, last)) if *end == i as u32 && *last == o => *end += 1,
                _ => out.push((i as u32, i as u32 + 1, o)),
            }
        }
        out
    }

    /// An object of type `ty` made at `at` over `len` bytes of its block: where the image
    /// doesn't know what the block held, it notes which bytes and whose they were
    /// ([`Unlaid`]).
    pub fn note_unlaid(&mut self, at: u32, len: u32, ty: &'static str) {
        if !self.on {
            return;
        }
        self.unlaid.retain(|u| u.at != at);
        let runs = self.leftover_unknown(at, len);
        if !runs.is_empty() {
            self.unlaid.push(Unlaid { at, ty, runs });
        }
    }

    /// The bytes the object at `at` was made over that the image didn't know
    /// ([`Image::note_unlaid`]).
    pub fn unlaid(&self, at: u32) -> Option<&Unlaid> {
        self.unlaid.iter().find(|u| u.at == at)
    }

    /// Which of the bytes `[lo, lo + len)` a new object's block holds from a render target
    /// ([`Src::RenderTarget`]), as [`Image::leftover_bytes`] reads the block.
    pub fn render_target(&self, lo: u32, len: u32) -> Vec<bool> {
        let mut out = vec![false; len as usize];
        if !self.on {
            return out;
        }
        let hi = lo + len;
        let mut mark = |runs: &mut dyn Iterator<Item = (u32, &Span)>| {
            for (k, s) in runs {
                for a in k.max(lo)..s.end.min(hi) {
                    out[(a - lo) as usize] = s.src == Src::RenderTarget;
                }
            }
        };
        mark(&mut self.base().range(lo, hi));
        mark(&mut self.overlay.range(lo, hi));
        mark(&mut self.live.range(lo, hi).filter(|(_, s)| !s.src.is_unknown()));
        out
    }

    /// A pool or a size class handed out the object at `at`, whose first word holds the free
    /// list's link `link` until its owner writes it.
    pub fn taken(&mut self, at: u32, link: u32) {
        if self.on {
            self.taken = Some((at, link));
        }
    }

    /// Memory a block now allocated holds that melee-sim knows (a file loaded into it, a
    /// movie's frame read into its ring).
    pub fn hold(&mut self, lo: u32, hi: u32, src: Src) {
        if !self.on {
            return;
        }
        if let Src::File {
            archive,
            offset: 0,
            base: b @ 1..,
        } = src
        {
            Arc::make_mut(self.places.get_or_insert_with(Default::default)).push((
                b,
                (hi - lo).saturating_sub(0x20),
                archive,
            ));
        }
        write(&mut self.live, lo, hi, src);
    }

    /// An archive whose data block is at `base` (`len` bytes) outside the heap the image
    /// keeps (the preload cache's): where a pointer into it points, which the image doesn't
    /// hold the bytes of.
    pub fn place(&mut self, base: u32, len: u32, archive: ArchiveId) {
        if self.on {
            Arc::make_mut(self.places.get_or_insert_with(Default::default))
                .push((base, len, archive));
        }
    }

    /// The owner of `[lo, hi)` of a block now allocated, an `owner` (its type), writes it:
    /// unknown until the owner says what it wrote (its layout as it goes, [`super::export`]).
    pub fn claim(&mut self, lo: u32, hi: u32, owner: &'static str) {
        if self.on {
            write(&mut self.live, lo, hi, Src::Unknown(owner));
        }
    }

    /// The owner of `[lo, hi)` of a block now allocated doesn't write it after all: it keeps
    /// what it held.
    pub fn unclaim(&mut self, lo: u32, hi: u32) {
        if self.on {
            take(&mut self.live, lo, hi);
        }
    }

    /// The next piece's owner writes only `parts` of it (offsets).
    pub fn claim_next(&mut self, parts: &[(u32, u32)]) {
        if self.on {
            self.claim_parts = Some(parts.to_vec());
        }
    }

    /// A piece of `size` bytes handed out at `p` to an `owner`: its claim on it.
    pub fn claim_piece(&mut self, p: u32, size: u32, owner: &'static str) {
        match self.claim_parts.take() {
            Some(parts) => {
                for (a, b) in parts {
                    self.claim(p + a, p + b.min(size), owner);
                }
            }
            None => self.claim(p, p + size, owner),
        }
    }

    /// The allocators write a word into memory no owner holds now (a free list's link).
    pub fn free_word(&mut self, at: u32, w: u32) {
        if self.on {
            let bytes = w.to_be_bytes().into();
            write(
                &mut self.overlay,
                at,
                at + 4,
                Src::Bytes { bytes, offset: 0 },
            );
        }
    }

    /// The allocators write a word into a block (a free list's link).
    pub fn word(&mut self, at: u32, w: u32) {
        self.put(at, &w.to_be_bytes());
    }

    /// An object's bytes laid out as the game keeps them, at `at` of a block now allocated.
    pub fn put(&mut self, at: u32, bytes: &[u8]) {
        if self.on {
            write(
                &mut self.live,
                at,
                at + bytes.len() as u32,
                Src::Bytes {
                    bytes: bytes.into(),
                    offset: 0,
                },
            );
        }
    }

    /// The scene's heap gave out the block whose cell is at `cell`.
    pub fn alloc_cell(&mut self, cell: u32) {
        if self.on {
            self.alloc_order.push(cell);
        }
    }

    /// The block whose cell is at `cell` went back to the scene's heap.
    pub fn free_cell(&mut self, cell: u32) {
        if let Some(i) = self.alloc_order.iter().rposition(|&c| c == cell) {
            self.alloc_order.remove(i);
        }
    }

    /// The allocated cells, oldest first.
    pub fn alloc_order(&self) -> &[u32] {
        &self.alloc_order
    }

    /// A cell's header (`Cell`'s first 0xC bytes), as the scene's heap is made anew.
    pub fn header(&mut self, cell: u32, prev: u32, next: u32, size: u32) {
        let mut b = [0; 12];
        b[..4].copy_from_slice(&prev.to_be_bytes());
        b[4..8].copy_from_slice(&next.to_be_bytes());
        b[8..].copy_from_slice(&size.to_be_bytes());
        self.put(cell, &b);
    }

    /// A free cell's header (`Cell`'s first 0xC bytes) as `OSAllocFromHeap` or
    /// `OSFreeToHeap` writes it, in memory no block holds now.
    pub fn free_header(&mut self, cell: u32, prev: u32, next: u32, size: u32) {
        self.free_word(cell, prev);
        self.free_word(cell + 4, next);
        self.free_word(cell + 8, size);
    }

    /// HSD's ID table made an entry at `addr`.
    pub fn id_insert(&mut self, addr: u32) {
        if self.on {
            self.id_order.push(addr);
        }
    }

    /// HSD's ID table let its entry at `addr` go.
    pub fn id_remove(&mut self, addr: u32) {
        if let Some(i) = self.id_order.iter().rposition(|&a| a == addr) {
            self.id_order.remove(i);
        }
    }

    /// The entries in the order they were made ([`Image::id_insert`]).
    pub fn id_order(&self) -> &[u32] {
        &self.id_order
    }

    /// A pool of objects of type `kind` gave out `slot` in this scene.
    pub fn note_slot(&mut self, kind: &'static str, slot: u32) {
        if self.on
            && let Err(i) = self.scene_slots.binary_search(&(kind, slot))
        {
            self.scene_slots.insert(i, (kind, slot));
        }
    }

    /// Whether a pool of objects of type `kind` gave out `slot` in this scene.
    pub fn in_scene(&self, kind: &str, slot: u32) -> bool {
        self.scene_slots
            .binary_search_by(|&(k, s)| (k, s).cmp(&(kind, slot)))
            .is_ok()
    }

    /// The slots the scene's pools gave out ([`Image::note_slot`]).
    pub fn scene_slots(&self) -> Vec<(&'static str, u32)> {
        self.scene_slots.clone()
    }

    /// The bytes at a place in an archive the image has placed, as a source: its file's bytes
    /// there, relocated as its newest place has them.
    pub fn file_src(&self, d: melee_data::archive::DataRef) -> Option<Src> {
        self.places()
            .iter()
            .rev()
            .find(|&&(_, len, a)| a == d.archive && d.offset < len)
            .map(|&(b, _, archive)| Src::File {
                archive,
                offset: 0x20 + d.offset,
                base: b,
            })
    }

    /// Bytes from `src` at `[lo, hi)` of a block now allocated (an object's copy of loaded
    /// data).
    pub fn put_src(&mut self, lo: u32, hi: u32, src: Src) {
        if self.on {
            write(&mut self.live, lo, hi, src);
        }
    }

    /// The address of a place in an archive the image has placed, at its newest place.
    pub fn data_addr(&self, d: melee_data::archive::DataRef) -> Option<u32> {
        self.places()
            .iter()
            .rev()
            .find(|&&(_, len, a)| a == d.archive && d.offset < len)
            .map(|&(b, _, _)| b + d.offset)
    }

    /// The block `[lo, hi)` goes back to its heap (`OSFreeToHeap`): it holds what its owner
    /// left, as far as the image knows it, and what it held before past that.
    pub fn release(&mut self, lo: u32, hi: u32) {
        if !self.on {
            return;
        }
        for (a, b, src) in take(&mut self.live, lo, hi) {
            write(&mut self.overlay, a, b, src);
        }
        self.unlaid.retain(|u| u.at < lo || u.at >= hi);
    }

    /// The scene's heap is made anew: what its allocated blocks hold stays in memory
    /// ([`super::heap::Heap::end_scene_image`]).
    pub fn flush_live(&mut self) {
        let live = std::mem::take(&mut self.live);
        for (k, s) in live.iter() {
            write(&mut self.overlay, k, s.end, s.src.clone());
        }
    }

    /// The scene's end: what this scene wrote joins what the scenes before left.
    pub fn end_scene(&mut self) {
        if !self.on {
            return;
        }
        self.flush_live();
        self.scene_slots.clear();
        self.alloc_order.clear();
        self.unlaid.clear();
        // The scene's ID table is forgotten with its heap (`_HSD_IDForgetMemory`).
        self.id_order.clear();
        if self.overlay.is_empty() {
            return;
        }
        let overlay = std::mem::take(&mut self.overlay);
        for (k, s) in overlay.iter() {
            write(&mut self.base, k, s.end, s.src.clone());
        }
    }

    /// The archive data a leftover pointer points into, at its newest place there: none where
    /// the archive has been placed elsewhere since (a file loaded again, the pointer naming the
    /// old copy's freed block), which a place in the data can't stand for, so the word stays a
    /// word.
    pub fn data_at(&self, addr: u32) -> Option<melee_data::archive::DataRef> {
        let places = self.places();
        let (i, &(b, _, archive)) = places
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &(b, len, _))| addr >= b && addr < b + len)?;
        if places[i + 1..].iter().any(|&(_, _, a)| a == archive) {
            return None;
        }
        Some(melee_data::archive::DataRef {
            archive,
            offset: addr - b,
        })
    }

    /// The handle a leftover pointer word stands as: past every pool, carrying the word.
    pub fn unknown_handle(&mut self, word: u32) -> u32 {
        if let Some(h) = leftover_handle(word) {
            return h;
        }
        let i = self
            .words
            .iter()
            .position(|&w| w == word)
            .unwrap_or_else(|| {
                self.words.push(word);
                self.words.len() - 1
            });
        super::pool::UNKNOWN | WORD_TABLE | i as u32
    }

    /// The word a leftover handle stands for ([`Image::unknown_handle`]), in a session that
    /// keeps the image (an imported state's unknown words are the importer's).
    pub fn unknown_word(&self, handle: u32) -> Option<u32> {
        if !self.on {
            return None;
        }
        let h = handle.checked_sub(super::pool::UNKNOWN)?;
        if h & WORD_TABLE == 0 {
            Some(MEM1.start + h)
        } else {
            self.words.get((h & !WORD_TABLE) as usize).copied()
        }
    }

    /// The files and objects melee-sim knows the allocated blocks hold now, for the oracle's
    /// check against memory.
    pub fn live_runs(&self) -> Vec<(u32, u32)> {
        self.live
            .iter()
            .filter(|(_, s)| !s.src.is_unknown())
            .map(|(k, s)| (k, s.end))
            .collect()
    }

    /// The runs the image holds, for the oracle's check against memory: each run's range and
    /// whether its bytes are known.
    pub fn runs(&self) -> Vec<(u32, u32, &'static str)> {
        let mut spans = self.base().clone();
        for (k, s) in self.overlay.iter().chain(self.live.iter()) {
            write(&mut spans, k, s.end, s.src.clone());
        }
        spans
            .iter()
            .map(|(k, s)| {
                (
                    k,
                    s.end,
                    match s.src {
                        Src::Unknown(_) => "unknown",
                        Src::File { .. } => "file",
                        Src::Raw { .. } => "raw file",
                        Src::Bytes { .. } => "object",
                        Src::RenderTarget => "render target",
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(b: &[u8]) -> Src {
        Src::Bytes {
            bytes: b.into(),
            offset: 0,
        }
    }

    #[test]
    fn writes_cut_older_runs_and_unknown_runs_join() {
        let data = Archives::default();
        let mut img = Image::power_on();
        img.write(0x8040_0100, 0x8040_0108, bytes(&[1, 2, 3, 4, 5, 6, 7, 8]));
        img.write(0x8040_0102, 0x8040_0104, bytes(&[9, 9]));
        let (b, k) = img.bytes(&data, 0x8040_00FE, 12);
        assert_eq!(b, [0, 0, 1, 2, 9, 9, 5, 6, 7, 8, 0, 0]);
        assert!(k.iter().all(|&k| k));
        img.unknown(0x8040_0105, 0x8040_0106);
        img.unknown(0x8040_0106, 0x8040_010A);
        img.end_scene();
        let (b, k) = img.bytes(&data, 0x8040_0100, 10);
        assert_eq!(b, [1, 2, 9, 9, 5, 0, 0, 0, 0, 0]);
        assert_eq!(
            k,
            [
                true, true, true, true, true, false, false, false, false, false
            ]
        );
        assert_eq!(img.runs().len(), 4);
    }

    #[test]
    fn an_owner_hides_what_it_claims_until_it_says_what_it_wrote() {
        let data = Archives::default();
        let mut img = Image::power_on();
        img.write(0x8040_0200, 0x8040_0210, bytes(&[0xAA; 16]));
        img.end_scene();
        img.claim(0x8040_0200, 0x8040_0208, "a test");
        // A new object reads what the block held before its owner wrote it.
        let (b, k) = img.leftover_bytes(&data, 0x8040_0200, 16);
        assert_eq!(b, [0xAA; 16]);
        assert!(k.iter().all(|&k| k));
        let (_, k) = img.bytes(&data, 0x8040_0200, 16);
        assert_eq!(k, [[false; 8], [true; 8]].concat());
        img.put(0x8040_0200, &[1, 2, 3, 4]);
        img.release(0x8040_0200, 0x8040_0220);
        img.end_scene();
        let (b, k) = img.bytes(&data, 0x8040_0200, 16);
        assert_eq!(b[..4], [1, 2, 3, 4]);
        assert_eq!(b[8..], [0xAA; 8]);
        assert_eq!(k, [&[true; 4][..], &[false; 4], &[true; 8]].concat());
    }

    #[test]
    fn a_pool_object_taken_keeps_its_link_for_its_leftovers() {
        let data = Archives::default();
        let mut img = Image::power_on();
        img.word(0x8040_0300, 0x8012_3450);
        img.claim(0x8040_0300, 0x8040_0310, "a test");
        img.taken(0x8040_0300, 0x8012_3450);
        let (b, k) = img.leftover_bytes(&data, 0x8040_0300, 8);
        assert_eq!(b, [0x80, 0x12, 0x34, 0x50, 0, 0, 0, 0]);
        assert!(k.iter().all(|&k| k));
    }

    #[test]
    fn claims_can_leave_parts_of_a_piece() {
        let data = Archives::default();
        let mut img = Image::power_on();
        img.claim_next(&[(0, 4), (8, 0x10)]);
        img.claim_piece(0x8040_0400, 0xC, "a test");
        let (_, k) = img.bytes(&data, 0x8040_0400, 0x10);
        assert_eq!(k, [[false; 4], [true; 4], [false; 4], [true; 4]].concat());
    }

    #[test]
    fn runs_split_at_pages_and_snapshots_share_the_pages_not_written() {
        let data = Archives::default();
        let mut img = Image::power_on();
        img.write(0x8040_FFFC, 0x8041_0004, bytes(&[1, 2, 3, 4, 5, 6, 7, 8]));
        img.write(0x8050_0000, 0x8050_0004, bytes(&[9; 4]));
        assert_eq!(img.overlay.len(), 3);
        let (b, _) = img.bytes(&data, 0x8040_FFFC, 8);
        assert_eq!(b, [1, 2, 3, 4, 5, 6, 7, 8]);
        let snap = img.clone();
        img.unknown(0x8041_0000, 0x8041_0002);
        let (_, k) = img.bytes(&data, 0x8040_FFFC, 8);
        assert_eq!(k, [true, true, true, true, false, false, true, true]);
        let (b, k) = snap.bytes(&data, 0x8040_FFFC, 8);
        assert_eq!(b, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(k.iter().all(|&k| k));
        // The pages the write didn't touch are the snapshot's.
        let shared =
            |i: usize| std::ptr::eq(img.overlay.page(i).unwrap(), snap.overlay.page(i).unwrap());
        assert!(shared(page_of(0x8040_FFFC)) && shared(page_of(0x8050_0000)));
        assert!(!shared(page_of(0x8041_0000)));
        // Outside main memory nothing is kept.
        img.write(0x100, 0x108, bytes(&[1; 8]));
        assert_eq!(img.bytes(&data, 0x100, 8).0, [0; 8]);
    }

    #[test]
    fn leftover_words_round_trip_through_their_handles() {
        let mut img = Image::power_on();
        for w in [
            0x8000_0000,
            0x80C6_51B8,
            0x817F_FFFC,
            0x3F80_0000,
            0xFFFF_FFFF,
        ] {
            let h = img.unknown_handle(w);
            assert!(h >= super::super::pool::UNKNOWN);
            assert_eq!(img.unknown_word(h), Some(w));
        }
    }

    #[test]
    fn an_image_not_kept_hashes_as_nothing() {
        let mut h = StateHasher::new();
        Image::default().state_hash(&mut h);
        assert_eq!(h.finish(), StateHasher::new().finish());
    }
}
