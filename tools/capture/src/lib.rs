//! Game states ssbm-rs captured (its lockstep captures: main memory and registers at a
//! function's entry, `CAPTURE=DIR CAPTURE_FUNCS=...`), made into melee-sim states, so a port
//! can run from a real state and be compared with ssbm-rs's run from the same one.

pub mod audio;
pub mod compare;
pub mod corpus;
pub mod display;
pub mod effects;
pub mod hud;
pub mod import;
pub mod items;
pub mod playback;
pub mod sis;
pub mod stage;
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
    pub fn read(path: &Path) -> Self {
        let packed = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let bytes =
            zstd::decode_all(&packed[..]).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let call = Call::from_bytes(&bytes)
            .unwrap_or_else(|| panic!("{}: not a saved call", path.display()));
        Capture { call }
    }

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
    /// The animation files' sub-archives by file name: each one's offset and size in the
    /// file.
    anim_files: HashMap<String, Vec<(u32, u32)>>,
    /// The animation files whole, by name.
    anim_bytes: HashMap<String, Vec<u8>>,
}

impl DiscIndex {
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
    let mut by_header: HashMap<[u32; 3], Vec<(String, Vec<u8>)>> = HashMap::new();
    let mut anim_files: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut anim_bytes = HashMap::new();
    // `.usd` files are the English versions of `.dat` ones (Pokémon Stadium's `GrPs.usd`).
    let archive = |f: &&String| f.ends_with(".dat") || f.ends_with(".usd");
    for name in disc.files().unwrap().iter().filter(archive) {
        let bytes = disc.read_file(name).unwrap();
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
    DiscIndex {
        by_header,
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
            let archive = Archive::parse(&file[off as usize..(off + size) as usize]).unwrap();
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
    let mut found = scan(index, mem1, archives, &|name, _| anims.get(name).copied());
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
    let known = |name: &str, file: u32| {
        before
            .iter()
            .find(|l| l.name == name && (l.file == file || l.file == 0))
            .map(|l| l.id)
    };
    let mut found = scan(index, mem1, archives, &known);
    let headerless = headerless_stage_files(index, mem1, archives, &found, &known);
    found.extend(headerless);
    found.extend(before.iter().filter(|l| l.file == 0).cloned());
    found
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
    let mut found = scan(index, mem1, archives, &|name, file| {
        before
            .iter()
            .find(|l| l.name == name && (l.file == file || l.file == 0))
            .map(|l| l.id)
            .or_else(|| named.iter().find(|(n, _)| n == name).map(|&(_, id)| id))
    });
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

/// The archives in main memory; `known` gives the id of one already added, by name and
/// address.
fn scan(
    index: &DiscIndex,
    mem1: &[u8],
    archives: &mut Archives,
    known: &dyn Fn(&str, u32) -> Option<ArchiveId>,
) -> Vec<Loaded> {
    let word = |i: usize| u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap());
    let by_header = &index.by_header;
    let mut found = Vec::new();
    let mut at = 0;
    while at + 0x20 <= mem1.len() {
        let header = [word(at), word(at + 4), word(at + 8)];
        let Some(candidates) = by_header.get(&header) else {
            at += 32;
            continue;
        };
        // The best-agreeing candidate (costume files share sizes), if it agrees on most words:
        // the game writes into some loaded data (a fighter's action table caches where each
        // animation was loaded).
        match best_match(candidates, mem1, at) {
            Some((name, bytes)) => {
                let file = MEM1 + at as u32;
                let id = known(name, file)
                    .unwrap_or_else(|| archives.add(Archive::parse(bytes).unwrap()));
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
                at += bytes.len().next_multiple_of(32);
            }
            None => at += 32,
        }
    }
    found
}

/// The best-agreeing candidate for a file at `at` (costume files share sizes), if it agrees on
/// most words: the game writes into some loaded data (a fighter's action table caches where
/// each animation was loaded).
fn best_match<'c>(
    candidates: &'c [(String, Vec<u8>)],
    mem1: &[u8],
    at: usize,
) -> Option<&'c (String, Vec<u8>)> {
    candidates
        .iter()
        .filter(|(_, bytes)| at + bytes.len() <= mem1.len())
        .map(|c| (agreement(&mem1[at..at + c.1.len()], &c.1), c))
        .filter(|((same, total), _)| same * 10 >= total * 7)
        .max_by_key(|((same, _), _)| *same)
        .map(|(_, c)| c)
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
        if let Some((name, bytes)) = best_match(candidates, mem1, at) {
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

/// How many of the file's words memory agrees with, relocated words counting as agreeing.
fn agreement(mem: &[u8], file: &[u8]) -> (usize, usize) {
    let w = |b: &[u8], i: usize| u32::from_be_bytes(b[i..i + 4].try_into().unwrap());
    let data_len = w(file, 4) as usize;
    let nb_reloc = w(file, 8) as usize;
    let mut relocated = vec![false; data_len / 4 + 1];
    for r in 0..nb_reloc {
        let at = w(file, 0x20 + data_len + 4 * r) as usize;
        if at / 4 < relocated.len() {
            relocated[at / 4] = true;
        }
    }
    // The header and the data block: the relocation and symbol tables after it may be reused
    // once the archive is parsed.
    let n = (0x20 + data_len) / 4;
    let same = (0..n)
        .filter(|&i| {
            let off = 4 * i;
            let in_data = (0x20..0x20 + data_len).contains(&off);
            (in_data && relocated[(off - 0x20) / 4]) || w(mem, off) == w(file, off)
        })
        .count();
    (same, n)
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

    /// The address of a `DataRef`.
    pub fn addr(&self, r: DataRef) -> u32 {
        let l = self
            .loaded
            .iter()
            .find(|l| l.id == r.archive)
            .expect("archive not loaded");
        l.data + r.offset
    }
}
