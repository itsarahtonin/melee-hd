//! Game states ssbm-rs captured (its lockstep captures: main memory and registers at a
//! function's entry, `CAPTURE=DIR CAPTURE_FUNCS=...`), made into melee-sim states, so a port
//! can run from a real state and be compared with ssbm-rs's run from the same one.

pub mod audio;
pub mod compare;
pub mod corpus;
pub mod display;
pub mod effects;
pub mod heap;
pub mod hud;
pub mod import;
pub mod items;
pub mod izumi;
pub mod links;
pub mod menus;
pub mod pack;
pub mod playback;
pub mod sis;
pub mod sound;
pub mod stadium;
pub mod stage;
pub mod tev;
pub mod title;
pub mod vs;

use std::collections::HashMap;
use std::path::Path;

use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use ssbm_rt::capture::Call;

/// Main memory's address.
pub const MEM1: u32 = 0x8000_0000;

/// A saved call.
pub struct Capture {
    pub call: Call,
}

impl Capture {
    /// The call saved at `path` (`DIR/NAME.call`): from `DIR`'s pack when it holds it
    /// ([`pack`]), from the file otherwise.
    pub fn read(path: &Path) -> Self {
        if let Some(call) = pack::read(path) {
            return Capture { call };
        }
        let packed = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let bytes =
            zstd::decode_all(&packed[..]).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let call = Call::from_bytes(&bytes)
            .unwrap_or_else(|| panic!("{}: not a saved call", path.display()));
        Capture { call }
    }

    #[track_caller]
    pub fn u32(&self, addr: u32) -> u32 {
        let i = addr.wrapping_sub(MEM1) as usize;
        u32::from_be_bytes(self.call.mem1[i..i + 4].try_into().unwrap())
    }
}

/// A disc archive found loaded in memory.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub name: String,
    /// Where its file starts, and its data block.
    pub file: u32,
    pub data: u32,
    pub data_len: u32,
    pub id: ArchiveId,
    /// Its public symbols and their offsets in the data block.
    pub publics: Vec<(String, u32)>,
}

/// The disc's archives by their headers' first three words, for [`find_archives_in`], and
/// the fighters' animation files (`*AJ.dat`, one archive per animation, each loaded on its
/// own).
pub struct DiscIndex {
    by_header: HashMap<[u32; 3], Vec<(String, Vec<u8>)>>,
    /// A bit for each of `by_header`'s headers by [`header_bit`], to pass over the places in
    /// memory no file starts at without a lookup.
    header_bits: Box<[u64]>,
    /// The animation files' sub-archives by file name: each one's offset and size in the
    /// file.
    anim_files: HashMap<String, Vec<(u32, u32)>>,
    /// The animation files whole, by name.
    anim_bytes: HashMap<String, Vec<u8>>,
}

/// The bits of [`DiscIndex::header_bits`].
const HEADER_BITS: u32 = 20;

/// A header's bit in [`DiscIndex::header_bits`].
fn header_bit(h: [u32; 3]) -> usize {
    let x = u64::from(h[0]) ^ u64::from(h[1]).rotate_left(21) ^ u64::from(h[2]).rotate_left(42);
    (x.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - HEADER_BITS)) as usize
}

impl DiscIndex {
    /// Whether some file on the disc may start with `header`.
    fn may_start(&self, header: [u32; 3]) -> bool {
        let b = header_bit(header);
        self.header_bits[b / 64] & (1 << (b % 64)) != 0
    }

    /// The animation file named `name` (`PlFxAJ.dat`), whole.
    pub fn anim_file(&self, name: &str) -> Option<&[u8]> {
        self.anim_bytes.get(name).map(Vec::as_slice)
    }
}

/// `ftData_Table_Unk0`: each character's animation file, `{ARAM address, count}`, once the
/// game has copied it to ARAM.
pub const FT_ANIM_FILES: u32 = 0x803C_0FC8;
/// `ftData_803C23E4`: each character's animation file name.
pub const FT_ANIM_FILE_NAMES: u32 = 0x803C_23E4;
/// How many characters those tables cover.
pub const FT_KINDS: u32 = 33;

/// The NUL-terminated string at `addr` in `mem1`.
pub fn c_string(mem1: &[u8], addr: u32) -> String {
    let at = (addr - MEM1) as usize;
    let end = mem1[at..].iter().position(|&b| b == 0).unwrap_or(0);
    String::from_utf8_lossy(&mem1[at..at + end]).into_owned()
}

/// The disc's archives loaded in `mem1`, added to `archives` in address order, found by their
/// headers and confirmed by their contents (every word relocation leaves alone). After them
/// come the animations of each fighter whose animation file the game has copied to ARAM
/// (`ftData_Table_Unk0`): every sub-archive, named `FILE@OFFSET`, with no address in main
/// memory. Fighters load their current animation from there, and one found loaded in main
/// memory is the same archive.
pub fn find_archives(mem1: &[u8], disc: &Disc, archives: &mut Archives) -> Vec<Loaded> {
    find_archives_in(&disc_index(disc), mem1, archives)
}

/// Indexes the disc's archives, once for many captures.
pub fn disc_index(disc: &Disc) -> DiscIndex {
    // `.usd` files are the English versions of `.dat` ones (Pokémon Stadium's `GrPs.usd`).
    let archive = |f: &&String| f.ends_with(".dat") || f.ends_with(".usd");
    let files = disc.files().unwrap();
    index_files(
        files
            .iter()
            .filter(archive)
            .map(|name| (name.clone(), disc.read_file(name).unwrap())),
    )
}

/// Indexes archive files by name.
fn index_files(files: impl Iterator<Item = (String, Vec<u8>)>) -> DiscIndex {
    let mut by_header: HashMap<[u32; 3], Vec<(String, Vec<u8>)>> = HashMap::new();
    let mut anim_files: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut anim_bytes = HashMap::new();
    for (name, bytes) in files {
        let name = &name;
        if bytes.len() < 0x20 {
            continue;
        }
        let w = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
        if name.ends_with("AJ.dat") {
            // A fighter's animations: one archive per animation, each loaded on its own.
            let mut off = 0;
            let subs = anim_files.entry(name.clone()).or_default();
            while off + 0x20 <= bytes.len() {
                let size = w(off) as usize;
                if size < 0x20 || off + size > bytes.len() {
                    break;
                }
                let sub = bytes[off..off + size].to_vec();
                by_header
                    .entry([w(off), w(off + 4), w(off + 8)])
                    .or_default()
                    .push((format!("{name}@{off:#x}"), sub));
                subs.push((off as u32, size as u32));
                off = (off + size).next_multiple_of(32);
            }
            anim_bytes.insert(name.clone(), bytes);
            continue;
        }
        by_header
            .entry([w(0), w(4), w(8)])
            .or_default()
            .push((name.clone(), bytes));
    }
    let mut header_bits = vec![0u64; (1 << HEADER_BITS) / 64].into_boxed_slice();
    for &h in by_header.keys() {
        let b = header_bit(h);
        header_bits[b / 64] |= 1 << (b % 64);
    }
    DiscIndex {
        by_header,
        header_bits,
        anim_files,
        anim_bytes,
    }
}

/// [`find_archives`] with an index made beforehand.
pub fn find_archives_in(index: &DiscIndex, mem1: &[u8], archives: &mut Archives) -> Vec<Loaded> {
    let word = |a: u32| {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap())
    };
    let mut aram = Vec::new();
    let mut anims = HashMap::new();
    for kind in 0..FT_KINDS {
        if word(FT_ANIM_FILES + 8 * kind) == 0 {
            continue;
        }
        let name = c_string(mem1, word(FT_ANIM_FILE_NAMES + 4 * kind));
        let (Some(subs), Some(file)) = (index.anim_files.get(&name), index.anim_file(&name)) else {
            continue;
        };
        for &(off, size) in subs {
            let (off, end) = (off as usize, (off + size) as usize);
            // What a fighter's buffer takes along past it (`lbArq_80014BD0` rounds up).
            let after = &file[end..end.next_multiple_of(32).min(file.len())];
            let archive = Archive::parse_in(&file[off..end], after).unwrap();
            let publics = archive.publics().map(|(n, o)| (n.to_owned(), o)).collect();
            let id = archives.add(archive);
            let sub = format!("{name}@{off:#x}");
            anims.insert(sub.clone(), id);
            aram.push(Loaded {
                name: sub,
                file: 0,
                data: 0,
                data_len: 0,
                id,
                publics,
            });
        }
    }
    let mut found = scan(
        index,
        mem1,
        archives,
        &|name, _| anims.get(name).copied(),
        None,
        None,
    );
    let headerless = headerless_stage_files(index, mem1, archives, &found, &|_, _| None);
    found.extend(headerless);
    found.extend(aram);
    found
}

/// [`find_archives_in`] again, on memory after something ran from the state `before` was
/// found in: an archive found where it was keeps its id, as does any animation (it may have
/// been loaded where another was).
pub fn find_archives_after(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    before: &[Loaded],
) -> Vec<Loaded> {
    find_after(index, mem1, archives, before, &[], None, None)
}

/// [`find_archives_after`], with archives added beforehand by name (files loaded for the
/// port's side, which the original loaded where it chose): a copy of one found in memory where
/// no archive was before is that archive.
pub fn find_archives_named(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    before: &[Loaded],
    named: &[(String, ArchiveId)],
) -> Vec<Loaded> {
    find_after(index, mem1, archives, before, named, None, None)
}

/// [`find_archives_named`] on memory that changed only in the pages `changed` (their indices,
/// in increasing order, [`PAGE`] bytes each) since `rescan`'s last scan, which finds the same
/// archives as a full scan without going over the unchanged pages again ([`Rescan`]). A file
/// the OS heaps freed claims nothing ([`Freed`]).
#[allow(clippy::too_many_arguments)]
pub fn find_archives_rescan(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    before: &[Loaded],
    named: &[(String, ArchiveId)],
    rescan: &mut Rescan,
    changed: &[usize],
    freed: &Freed,
) -> Vec<Loaded> {
    find_after(
        index,
        mem1,
        archives,
        before,
        named,
        Some((rescan, changed)),
        Some(freed),
    )
}

fn find_after(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    before: &[Loaded],
    named: &[(String, ArchiveId)],
    rescan: Option<(&mut Rescan, &[usize])>,
    freed: Option<&Freed>,
) -> Vec<Loaded> {
    let known = |name: &str, file: u32| {
        before
            .iter()
            .find(|l| l.name == name && (l.file == file || l.file == 0))
            .map(|l| l.id)
            .or_else(|| named.iter().find(|(n, _)| n == name).map(|&(_, id)| id))
    };
    let mut found = scan(index, mem1, archives, &known, rescan, freed);
    let headerless = headerless_stage_files(index, mem1, archives, &found, &known);
    found.extend(headerless);
    found.extend(before.iter().filter(|l| l.file == 0).cloned());
    found
}

/// The disc's file named `name`, whole, if the index has it as an archive.
pub fn disc_file<'a>(index: &'a DiscIndex, name: &str) -> Option<&'a [u8]> {
    index
        .by_header
        .values()
        .flatten()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b.as_slice())
}

/// An animation file's sub-archives (`NAME@OFFSET`, one per animation) by offset.
pub fn anim_subarchives(index: &DiscIndex, name: &str) -> Vec<(u32, Vec<u8>)> {
    let (Some(subs), Some(file)) = (index.anim_files.get(name), index.anim_file(name)) else {
        return Vec::new();
    };
    subs.iter()
        .map(|&(off, size)| (off, file[off as usize..(off + size) as usize].to_vec()))
        .collect()
}

/// The OS heaps' arenas and allocated cells as memory holds them (`HeapArray`, `NumHeaps`): a
/// file that lies in a heap's arena but in none of its allocated cells was freed, and its stale
/// header and data claim nothing, so the files loaded since over its range are found. A
/// scene's heap made anew (`HSD_CreateMainHeap`) leaves the last scene's files there until
/// they're written over: the progressive scan prompt's `EfCoData.dat` under the memory card
/// scene's files.
#[derive(Clone, Debug, Default)]
pub struct Freed {
    heaps: Vec<HeapCells>,
}

/// A live OS heap's arena, and its allocated cells by address (each with its header), with
/// their sizes.
#[derive(Clone, Debug, Default)]
struct HeapCells {
    lo: u32,
    hi: u32,
    allocated: Vec<(u32, u32)>,
}

impl Freed {
    pub fn read(mem1: &[u8]) -> Freed {
        let in_mem = |a: u32| a >= MEM1 && ((a - MEM1) as usize) + 4 <= mem1.len();
        let word = |a: u32| {
            let i = (a - MEM1) as usize;
            u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap())
        };
        // A cell list's cells (address, size), stopping at anything that isn't one.
        let cells = |mut c: u32| {
            let mut v = Vec::new();
            while c != 0 && in_mem(c + 8) && v.len() < 1 << 16 {
                v.push((c, word(c + 8)));
                c = word(c + 4);
            }
            v
        };
        let array = word(crate::import::sym("HeapArray"));
        let n = word(crate::import::sym("NumHeaps")) as i32;
        let mut heaps = Vec::new();
        for i in 0..n.clamp(0, 64) as u32 {
            let desc = array.wrapping_add(12 * i);
            if !in_mem(desc + 8) || (word(desc) as i32) < 0 {
                continue;
            }
            let free = cells(word(desc + 4));
            let mut allocated = cells(word(desc + 8));
            let all = || free.iter().chain(&allocated);
            let (Some(lo), Some(hi)) = (
                all().map(|c| c.0).min(),
                all().map(|c| c.0.wrapping_add(c.1)).max(),
            ) else {
                continue;
            };
            allocated.sort_unstable();
            heaps.push(HeapCells { lo, hi, allocated });
        }
        Freed { heaps }
    }

    /// Whether a file of `len` bytes at `file` can be live: outside every heap's arena, or in
    /// one of a heap's allocated cells.
    pub fn claims(&self, file: u32, len: usize) -> bool {
        let end = u64::from(file) + len as u64;
        self.heaps.iter().all(|h| {
            if end <= u64::from(h.lo) || file >= h.hi {
                return true;
            }
            let i = h.allocated.partition_point(|c| c.0 <= file);
            i > 0 && {
                let (at, size) = h.allocated[i - 1];
                end <= u64::from(at) + u64::from(size)
            }
        })
    }
}

/// A disc file the scan placed in memory: where it starts, and the index's entry.
type Placed<'i> = (usize, &'i (String, Vec<u8>));

/// The archives in main memory; `known` gives the id of one already added, by name and
/// address. With `rescan`, only the pages it's told changed are gone over again.
fn scan(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    known: &dyn Fn(&str, u32) -> Option<ArchiveId>,
    rescan: Option<(&mut Rescan, &[usize])>,
    freed: Option<&Freed>,
) -> Vec<Loaded> {
    let prefer = |name: &str, file: u32| known(name, file).is_some();
    let live = |file: u32, len: usize| freed.is_none_or(|f| f.claims(file, len));
    let placed = match rescan {
        None => placements(index, mem1, &prefer, &live),
        Some((r, changed)) => {
            let placed = r.placements(index, mem1, &prefer, &live, changed);
            if r.check {
                r.checks += 1;
                let full = placements(index, mem1, &prefer, &live);
                let same = |a: &Placed, b: &Placed| a.0 == b.0 && std::ptr::eq(a.1, b.1);
                if full.len() != placed.len() || !full.iter().zip(&placed).all(|(a, b)| same(a, b))
                {
                    let names = |p: &[Placed]| {
                        p.iter()
                            .map(|(at, (name, _))| format!("{name} at {:#x}", MEM1 + *at as u32))
                            .collect::<Vec<_>>()
                    };
                    let (full, placed) = (names(&full), names(&placed));
                    let only = |a: &[String], b: &[String]| {
                        a.iter()
                            .filter(|x| !b.contains(x))
                            .cloned()
                            .collect::<Vec<_>>()
                    };
                    r.disagreements.push(format!(
                        "a full scan alone finds {:?}, the rescan alone {:?}",
                        only(&full, &placed),
                        only(&placed, &full),
                    ));
                }
            }
            placed
        }
    };
    let mut found = Vec::new();
    for (at, (name, bytes)) in placed {
        let file = MEM1 + at as u32;
        let id = known(name, file).unwrap_or_else(|| archives.add(Archive::parse(bytes).unwrap()));
        let archive = archives.get(id);
        let data_len = archive.data().len() as u32;
        let publics = archive.publics().map(|(n, o)| (n.to_owned(), o)).collect();
        found.push(Loaded {
            name: name.clone(),
            file,
            data: file + 0x20,
            data_len,
            id,
            publics,
        });
    }
    found
}

/// The three header words at `at` in memory.
fn header_at(mem1: &[u8], at: usize) -> [u32; 3] {
    let word = |i: usize| u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap());
    [word(at), word(at + 4), word(at + 8)]
}

/// Where in memory the disc's files are, by a walk over all of it: at each 32-byte step, the
/// file starting there if one does ([`best_match`]), and the walk goes on past its end.
fn placements<'i>(
    index: &'i DiscIndex,
    mem1: &[u8],
    prefer: &dyn Fn(&str, u32) -> bool,
    live: &dyn Fn(u32, usize) -> bool,
) -> Vec<Placed<'i>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 0x20 <= mem1.len() {
        let header = header_at(mem1, at);
        let Some(candidates) = index
            .may_start(header)
            .then(|| index.by_header.get(&header))
            .flatten()
        else {
            at += 32;
            continue;
        };
        let file = MEM1 + at as u32;
        match best_match(index, candidates, mem1, at, &|n| prefer(n, file), &|len| {
            live(file, len)
        }) {
            Some(c) => {
                out.push((at, c));
                at += c.1.len().next_multiple_of(32);
            }
            None => at += 32,
        }
    }
    out
}

/// Bytes per page of memory, the unit [`find_archives_rescan`] is told changed (a trace's).
pub const PAGE: usize = 0x1000;

/// The archive scan's work kept from one call to the next, for memory that changes a few pages
/// at a time (a trace's ticks): which 32-byte steps hold a disc file's header, and at each
/// header the walk met, how many words memory agrees on with each candidate file
/// ([`Agreement`]), page by page. A changed page is counted again only where it lies in a
/// candidate's span, and a site whose header changed is made again. The walk is
/// [`placements`]' walk, the steps with no header passed over at once, and each site's pick is
/// [`best_match`]'s from the same counts ([`choose`]), so the result is a full scan's.
#[derive(Default)]
pub struct Rescan {
    /// The memory's size the steps are for; 0 before the first scan, which goes over it all.
    len: usize,
    /// A bit for each 32-byte step: whether a disc file's header is there.
    headers: Vec<u64>,
    /// The headers the last walk met, by address.
    sites: HashMap<usize, Site>,
    /// Checks each scan against a full one (`diff --check-rescan`), noting what differs.
    pub check: bool,
    /// The scans checked, and how the checked ones differed.
    pub checks: u32,
    pub disagreements: Vec<String>,
}

/// A header's candidates' agreements, each by page; none for one past memory's end.
struct Site {
    header: [u32; 3],
    candidates: Vec<Option<Counted>>,
}

/// A candidate file's [`Agreement`] with memory, its memory-dependent counts by page.
struct Counted {
    relocated: Vec<bool>,
    /// The first page its compared words lie in, and each page's `(same, own_same)`.
    first: usize,
    pages: Vec<(usize, usize)>,
    sum: Agreement,
}

impl Counted {
    fn new(mem1: &[u8], at: usize, file: &[u8]) -> Self {
        let (relocated, total, own_total) = relocations(file);
        let first = at / PAGE;
        let last = (at + 4 * total - 1) / PAGE;
        let mut c = Counted {
            relocated,
            first,
            pages: vec![(0, 0); last - first + 1],
            sum: Agreement {
                same: 0,
                total,
                own_same: 0,
                own_total,
            },
        };
        for p in first..=last {
            c.count(mem1, at, file, p);
        }
        c
    }

    /// Counts page `p` again.
    fn count(&mut self, mem1: &[u8], at: usize, file: &[u8], p: usize) {
        let words =
            (p * PAGE).saturating_sub(at) / 4..(((p + 1) * PAGE - at) / 4).min(self.sum.total);
        let mem = &mem1[at..at + file.len()];
        let (same, own_same) = agree_words(mem, file, &self.relocated, words);
        let old = std::mem::replace(&mut self.pages[p - self.first], (same, own_same));
        self.sum.same = self.sum.same - old.0 + same;
        self.sum.own_same = self.sum.own_same - old.1 + own_same;
    }
}

impl Rescan {
    /// Whether a disc file's header is at `at`.
    fn header(index: &DiscIndex, mem1: &[u8], at: usize) -> bool {
        let h = header_at(mem1, at);
        index.may_start(h) && index.by_header.contains_key(&h)
    }

    fn set(&mut self, step: usize, on: bool) {
        let (w, b) = (step / 64, step % 64);
        self.headers[w] = self.headers[w] & !(1 << b) | u64::from(on) << b;
    }

    /// Brings the headers and the sites up to date with memory, which changed in the pages
    /// `changed` since the last scan.
    fn update(&mut self, index: &DiscIndex, mem1: &[u8], changed: &[usize]) {
        let steps = if mem1.len() < 0x20 {
            0
        } else {
            (mem1.len() - 0x20) / 32 + 1
        };
        if self.len != mem1.len() {
            self.len = mem1.len();
            self.headers = vec![0; steps.div_ceil(64)];
            self.sites.clear();
            for s in 0..steps {
                self.set(s, Self::header(index, mem1, 32 * s));
            }
            return;
        }
        debug_assert!(changed.is_sorted());
        if changed.is_empty() {
            return;
        }
        for &p in changed {
            for s in (p * PAGE / 32..(p + 1) * PAGE / 32).take_while(|&s| s < steps) {
                self.set(s, Self::header(index, mem1, 32 * s));
            }
        }
        // A site whose header changed has other candidates, or none.
        self.sites
            .retain(|&at, site| header_at(mem1, at) == site.header);
        for (&at, site) in &mut self.sites {
            let candidates = &index.by_header[&site.header];
            for (c, (_, file)) in site.candidates.iter_mut().zip(candidates) {
                let Some(c) = c else { continue };
                let last = c.first + c.pages.len() - 1;
                let from = changed.partition_point(|&p| p < c.first);
                for &p in changed[from..].iter().take_while(|&&p| p <= last) {
                    c.count(mem1, at, file, p);
                }
            }
        }
    }

    /// The first step at or after `at` holding a disc file's header.
    fn next_header(&self, at: usize) -> Option<usize> {
        let s = at / 32;
        let mut w = s / 64;
        let mut bits = *self.headers.get(w)? & (!0u64 << (s % 64));
        loop {
            if bits != 0 {
                return Some(32 * (64 * w + bits.trailing_zeros() as usize));
            }
            w += 1;
            bits = *self.headers.get(w)?;
        }
    }

    /// [`placements`], going over only the pages `changed` again.
    fn placements<'i>(
        &mut self,
        index: &'i DiscIndex,
        mem1: &[u8],
        prefer: &dyn Fn(&str, u32) -> bool,
        live: &dyn Fn(u32, usize) -> bool,
        changed: &[usize],
    ) -> Vec<Placed<'i>> {
        self.update(index, mem1, changed);
        let mut out = Vec::new();
        let mut met = Vec::new();
        let mut at = 0;
        while let Some(h) = self.next_header(at) {
            at = h;
            met.push(at);
            let header = header_at(mem1, at);
            let candidates = &index.by_header[&header];
            let site = self.sites.entry(at).or_insert_with(|| Site {
                header,
                candidates: candidates
                    .iter()
                    .map(|(_, file)| {
                        (at + file.len() <= mem1.len()).then(|| Counted::new(mem1, at, file))
                    })
                    .collect(),
            });
            let agreements: Vec<Option<Agreement>> = site
                .candidates
                .iter()
                .map(|c| c.as_ref().map(|c| c.sum))
                .collect();
            let file = MEM1 + at as u32;
            match choose(
                index,
                candidates,
                &agreements,
                mem1,
                at,
                &|n| prefer(n, file),
                &|len| live(file, len),
            ) {
                Some(c) => {
                    out.push((at, c));
                    at += c.1.len().next_multiple_of(32);
                }
                None => at += 32,
            }
        }
        // Only the sites the walk met are kept up to date.
        self.sites.retain(|at, _| met.binary_search(at).is_ok());
        out
    }
}

/// The best-agreeing candidate for a file at `at` (costume files share sizes), if it agrees on
/// most words: the game writes into some loaded data (a fighter's action table caches where
/// each animation was loaded). Between candidates that agree as well, one `prefer` names wins:
/// files can hold the same bytes (Luigi's animation file repeats some of Mario's), and the
/// copy is the one the game loaded.
fn best_match<'c>(
    index: &DiscIndex,
    candidates: &'c [(String, Vec<u8>)],
    mem1: &[u8],
    at: usize,
    prefer: &dyn Fn(&str) -> bool,
    live: &dyn Fn(usize) -> bool,
) -> Option<&'c (String, Vec<u8>)> {
    let agreements: Vec<Option<Agreement>> = candidates
        .iter()
        .map(|(_, file)| {
            (at + file.len() <= mem1.len()).then(|| agreement(&mem1[at..at + file.len()], file))
        })
        .collect();
    choose(index, candidates, &agreements, mem1, at, prefer, live)
}

/// [`best_match`]'s pick from the candidates' agreements with memory (none for one past its
/// end), of those `live` (by their length) says weren't freed.
#[allow(clippy::too_many_arguments)]
fn choose<'c>(
    index: &DiscIndex,
    candidates: &'c [(String, Vec<u8>)],
    agreements: &[Option<Agreement>],
    mem1: &[u8],
    at: usize,
    prefer: &dyn Fn(&str) -> bool,
    live: &dyn Fn(usize) -> bool,
) -> Option<&'c (String, Vec<u8>)> {
    candidates
        .iter()
        .zip(agreements)
        .filter_map(|(c, a)| a.map(|a| (a, c)))
        .filter(|(_, c)| live(c.1.len()))
        // One already found here, or the only file with its header, needs only most of the data
        // words relocation leaves alone, if no other file was loaded over it: loading a copy
        // costume onto Kirby's bones rewrites the matrix indices of its display lists in place,
        // 31% of Jigglypuff's copy's file. A freed file's header and relocated words alone make 9
        // of `LbRf.dat`'s 16, and what's left of one can be half (`GmRstMFx.dat` under `PlSk.dat`).
        .filter(|(a, c)| {
            a.same * 10 >= a.total * 7
                || ((prefer(&c.0) || candidates.len() == 1)
                    && a.own_same * 2 > a.own_total
                    && !file_starts_in(index, mem1, at + 0x20..at + c.1.len()))
        })
        .max_by_key(|(a, c)| (a.same, prefer(&c.0)))
        .map(|(_, c)| c)
}

/// Whether a disc file starts in `range` of memory (at a 32-byte step) and agrees on most
/// words.
fn file_starts_in(index: &DiscIndex, mem1: &[u8], range: std::ops::Range<usize>) -> bool {
    range
        .step_by(32)
        .take_while(|at| at + 0x20 <= mem1.len())
        .any(|at| {
            let header = header_at(mem1, at);
            index.may_start(header)
                && index.by_header.get(&header).is_some_and(|files| {
                    files.iter().any(|(_, bytes)| {
                        let a = mem1
                            .get(at..at + bytes.len())
                            .map(|mem| agreement(mem, bytes));
                        a.is_some_and(|a| a.same * 10 >= a.total * 7)
                    })
                })
        })
}

/// The stage archives whose files [`scan`] can't find by their headers: Pokémon Stadium's
/// return to its default stage hands `grAnime_801C65B0` its transformation file's buffer
/// instead of the file's archive, which zeroes the header's first three words and leaves the
/// file registered. The stage's `HSD_Archive` keeps a copy of the header and where the data
/// is, which finds the file.
fn headerless_stage_files(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    found: &[Loaded],
    known: &dyn Fn(&str, u32) -> Option<ArchiveId>,
) -> Vec<Loaded> {
    let word = |a: u32| {
        let i = a.wrapping_sub(MEM1) as usize;
        mem1.get(i..i + 4)
            .map_or(0, |b| u32::from_be_bytes(b.try_into().unwrap()))
    };
    let table = import::sym("grDatFiles_8049EE10");
    let mut out = Vec::new();
    for i in 0..4 {
        let arc = word(table + 0xC * i);
        let data = word(arc + 0x20);
        let in_mem = |a: u32| (MEM1 + 0x20..MEM1 + mem1.len() as u32).contains(&a);
        if !in_mem(arc)
            || !in_mem(data)
            || found
                .iter()
                .chain(&out)
                .any(|l: &Loaded| (l.data..l.data + l.data_len).contains(&data))
            || word(data - 0x20) != 0
        {
            continue;
        }
        let header = [word(arc), word(arc + 4), word(arc + 8)];
        let Some(candidates) = index.by_header.get(&header) else {
            continue;
        };
        let at = (data - 0x20 - MEM1) as usize;
        if let Some((name, bytes)) = best_match(index, candidates, mem1, at, &|_| false, &|_| true)
        {
            let id = known(name, data - 0x20)
                .unwrap_or_else(|| archives.add(Archive::parse(bytes).unwrap()));
            let archive = archives.get(id);
            out.push(Loaded {
                name: name.clone(),
                file: data - 0x20,
                data,
                data_len: archive.data().len() as u32,
                id,
                publics: archive.publics().map(|(n, o)| (n.to_owned(), o)).collect(),
            });
        }
    }
    out
}

/// How many of a file's words memory agrees with.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Agreement {
    /// Of the header and the data block, relocated words counting as agreeing.
    same: usize,
    total: usize,
    /// Of the data block's words relocation leaves alone.
    own_same: usize,
    own_total: usize,
}

/// [`Agreement`] of memory with `file`.
fn agreement(mem: &[u8], file: &[u8]) -> Agreement {
    let (relocated, total, own_total) = relocations(file);
    let (same, own_same) = agree_words(mem, file, &relocated, 0..total);
    Agreement {
        same,
        total,
        own_same,
        own_total,
    }
}

/// Which of `file`'s data words relocation sets, how many words [`Agreement`] compares (the
/// header and the data block: the relocation and symbol tables after it may be reused once the
/// archive is parsed), and how many of them relocation leaves alone in the data block.
fn relocations(file: &[u8]) -> (Vec<bool>, usize, usize) {
    let w = |i: usize| u32::from_be_bytes(file[i..i + 4].try_into().unwrap());
    let data_len = w(4) as usize;
    let nb_reloc = w(8) as usize;
    let mut relocated = vec![false; data_len / 4 + 1];
    for r in 0..nb_reloc {
        let at = w(0x20 + data_len + 4 * r) as usize;
        if at / 4 < relocated.len() {
            relocated[at / 4] = true;
        }
    }
    let total = (0x20 + data_len) / 4;
    let own_total = (8..total).filter(|&i| !relocated[i - 8]).count();
    (relocated, total, own_total)
}

/// Of `file`'s words numbered `words`, how many memory agrees with (relocated data words
/// counting as agreeing), and how many of those are data words relocation leaves alone.
fn agree_words(
    mem: &[u8],
    file: &[u8],
    relocated: &[bool],
    words: std::ops::Range<usize>,
) -> (usize, usize) {
    let w = |b: &[u8], i: usize| u32::from_be_bytes(b[i..i + 4].try_into().unwrap());
    let (mut same, mut own_same) = (0, 0);
    for off in words.map(|i| 4 * i) {
        let agrees = w(mem, off) == w(file, off);
        if off < 0x20 {
            same += usize::from(agrees);
        } else if relocated[(off - 0x20) / 4] {
            same += 1;
        } else {
            same += usize::from(agrees);
            own_same += usize::from(agrees);
        }
    }
    (same, own_same)
}

/// mpcoll's statics in memory (`mpColl_80458810` and the `mpColl_804D64xx` words), the floor
/// filter's fighter as its address (for the caller to map).
pub fn read_coll_statics(mem1: &[u8]) -> (melee_sim::mp::coll::CollStatics, u32) {
    use melee_sim::game::handles::CodeRef;
    use melee_sim::vec::Vec3;
    let at = |a: u32| {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap())
    };
    let r = |n: &str| at(import::sym(n));
    let walls = import::sym("mpColl_80458810");
    let f = |a: u32| f32::from_bits(at(a));
    let code = |a: u32| (a != 0).then_some(CodeRef(a));
    let s = melee_sim::mp::coll::CollStatics {
        right_walls: std::array::from_fn(|i| at(walls + 4 * i as u32) as i32),
        right_count: r("mpColl_804D6488") as i32,
        left_walls: std::array::from_fn(|i| at(walls + 0x24 + 4 * i as u32) as i32),
        left_count: r("mpColl_804D648C") as i32,
        max_x: f(import::sym("mpColl_804D6490_max_x")),
        line_id: r("mpColl_804D6494_line_id") as i32,
        flags: r("mpColl_804D6498_flags"),
        normal: Vec3::new(f(walls + 0x48), f(walls + 0x4C), f(walls + 0x50)),
        ecb_tiny: r("mpColl_IsEcbTiny") != 0,
        floor_filter: code(r("mpColl_804D64A0")),
        floor_filter_gobj: None,
        x64a8: code(r("mpColl_804D64A8")),
        stale_reads: 0,
        watchdog: None,
    };
    (s, r("mpColl_804D64A4"))
}

/// Maps addresses in loaded archives to `DataRef`s.
pub struct DataMap {
    pub loaded: Vec<Loaded>,
}

impl DataMap {
    pub fn get(&self, addr: u32) -> Option<DataRef> {
        let l = self
            .loaded
            .iter()
            .find(|l| (l.data..l.data + l.data_len).contains(&addr))?;
        Some(DataRef {
            archive: l.id,
            offset: addr - l.data,
        })
    }

    /// The public symbol `name` of the loaded file named `file`.
    pub fn public(&self, file: &str, name: &str) -> Option<DataRef> {
        let l = self.loaded.iter().find(|l| l.name == file)?;
        let &(_, offset) = l.publics.iter().find(|(n, _)| n == name)?;
        Some(DataRef {
            archive: l.id,
            offset,
        })
    }

    /// The loaded file named `file`'s archive.
    pub fn archive_named(&self, file: &str) -> Option<melee_data::archive::ArchiveId> {
        self.loaded.iter().find(|l| l.name == file).map(|l| l.id)
    }

    /// The address of a `DataRef`.
    pub fn addr(&self, r: DataRef) -> u32 {
        let l = self
            .loaded
            .iter()
            .find(|l| l.id == r.archive)
            .unwrap_or_else(|| panic!("archive {:?} not loaded (at {:#x})", r.archive, r.offset));
        l.data + r.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator for the test's memory.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// An archive file's bytes as far as the scan reads them: the header's size words, a data
    /// block and its relocations.
    fn file(rng: &mut Rng, data_len: usize, relocs: usize) -> Vec<u8> {
        let size = 0x20 + data_len + 4 * relocs;
        let mut f = vec![0; size];
        f[0..4].copy_from_slice(&(size as u32).to_be_bytes());
        f[4..8].copy_from_slice(&(data_len as u32).to_be_bytes());
        f[8..12].copy_from_slice(&(relocs as u32).to_be_bytes());
        for b in &mut f[0x20..0x20 + data_len] {
            *b = rng.next() as u8;
        }
        for r in 0..relocs {
            let at = 4 * rng.below(data_len / 4);
            f[0x20 + data_len + 4 * r..][..4].copy_from_slice(&(at as u32).to_be_bytes());
        }
        f
    }

    /// The rescan finds what a full scan finds after each of many changes to memory: files
    /// loaded, loaded over, worn by writes into them, their headers cleared, and `prefer`
    /// changing its mind.
    #[test]
    fn rescan_agrees_with_a_full_scan() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        // Some files share their header (costume files share sizes).
        let mut files = Vec::new();
        for i in 0..12 {
            let (data_len, relocs) = [(0x200, 4), (0x200, 4), (0x1800, 20), (0x60, 0)][i % 4];
            files.push((format!("F{i}.dat"), file(&mut rng, data_len, relocs)));
        }
        files.push(("Big.dat".into(), file(&mut rng, 0x8000, 40)));
        let index = index_files(files.clone().into_iter());
        let mut mem = vec![0u8; 64 * PAGE];
        let mut rescan = Rescan::default();
        let (mut placed, mut epoch) = (0, 0u64);
        for step in 0..3000 {
            let mut changed = std::collections::BTreeSet::new();
            let mut write = |mem: &mut Vec<u8>, at: usize, bytes: &[u8]| {
                let end = (at + bytes.len()).min(mem.len());
                mem[at..end].copy_from_slice(&bytes[..end - at]);
                changed.extend(at / PAGE..end.div_ceil(PAGE));
            };
            for _ in 0..1 + rng.below(3) {
                let at = 32 * rng.below(mem.len() / 32);
                match rng.below(7) {
                    0 | 1 => {
                        let f = &files[rng.below(files.len())].1;
                        write(&mut mem, at, f);
                    }
                    2 => {
                        let bytes: Vec<u8> = (0..rng.below(64)).map(|_| rng.next() as u8).collect();
                        write(&mut mem, at + rng.below(32), &bytes);
                    }
                    3 => {
                        // Words of a file worn, up to most of them.
                        let (_, f) = &files[rng.below(files.len())];
                        let mut f = f.clone();
                        let worn = rng.below(90);
                        for w in 0..f.len() / 4 {
                            if rng.below(100) < worn {
                                f[4 * w] ^= 1 + rng.below(255) as u8;
                            }
                        }
                        write(&mut mem, at, &f);
                    }
                    4 => write(&mut mem, at, &[0; 12]),
                    5 => {
                        // A page filled with new bytes.
                        let bytes: Vec<u8> = (0..PAGE).map(|_| rng.next() as u8).collect();
                        write(&mut mem, at / PAGE * PAGE, &bytes);
                    }
                    _ => {
                        // A single word changed.
                        let w = rng.next().to_be_bytes();
                        write(&mut mem, at + 4 * rng.below(8), &w[..4]);
                    }
                }
            }
            if rng.below(50) == 0 {
                epoch += 1;
            }
            let prefer = |name: &str, file: u32| {
                let h = name.bytes().fold(u64::from(file) ^ epoch, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3)
                });
                h % 3 == 0
            };
            let changed: Vec<usize> = changed.into_iter().collect();
            let ours = rescan.placements(&index, &mem, &prefer, &|_, _| true, &changed);
            let full = placements(&index, &mem, &prefer, &|_, _| true);
            let key = |p: &[Placed]| {
                p.iter()
                    .map(|(at, (n, _))| (*at, n.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(key(&ours), key(&full), "step {step}");
            // Every site's counts are what counting memory afresh gives.
            for (&at, site) in &rescan.sites {
                let candidates = &index.by_header[&header_at(&mem, at)];
                for (c, (name, file)) in site.candidates.iter().zip(candidates) {
                    if let Some(c) = c {
                        let fresh = agreement(&mem[at..at + file.len()], file);
                        assert_eq!(c.sum, fresh, "step {step}: {name} at {at:#x}");
                    }
                }
            }
            placed += full.len();
        }
        assert!(placed > 3000, "few files placed: {placed}");
    }
}
