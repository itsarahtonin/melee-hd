//! Game states ssbm-rs captured (its lockstep captures: main memory and registers at a
//! function's entry, `CAPTURE=DIR CAPTURE_FUNCS=...`), made into melee-sim states, so a port
//! can run from a real state and be compared with ssbm-rs's run from the same one.

pub mod compare;
pub mod import;

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
}

/// The disc's archives loaded in `mem1`, added to `archives` in address order, found by their
/// headers and confirmed by their contents (every word relocation leaves alone).
pub fn find_archives(mem1: &[u8], disc: &Disc, archives: &mut Archives) -> Vec<Loaded> {
    let word = |i: usize| u32::from_be_bytes(mem1[i..i + 4].try_into().unwrap());
    let mut by_header: HashMap<[u32; 3], Vec<(String, Vec<u8>)>> = HashMap::new();
    for name in disc.files().unwrap().iter().filter(|f| f.ends_with(".dat")) {
        let bytes = disc.read_file(name).unwrap();
        if bytes.len() < 0x20 {
            continue;
        }
        let w = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
        if name.ends_with("AJ.dat") {
            // A fighter's animations: one archive per animation, each loaded on its own.
            let mut off = 0;
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
                off = (off + size).next_multiple_of(32);
            }
            continue;
        }
        by_header
            .entry([w(0), w(4), w(8)])
            .or_default()
            .push((name.clone(), bytes));
    }
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
        let matched = candidates
            .iter()
            .filter(|(_, bytes)| at + bytes.len() <= mem1.len())
            .map(|c| (agreement(&mem1[at..at + c.1.len()], &c.1), c))
            .filter(|((same, total), _)| same * 10 >= total * 7)
            .max_by_key(|((same, _), _)| *same)
            .map(|(_, c)| c);
        match matched {
            Some((name, bytes)) => {
                let archive = Archive::parse(bytes).unwrap();
                let data_len = archive.data().len() as u32;
                let id = archives.add(archive);
                found.push(Loaded {
                    name: name.clone(),
                    file: MEM1 + at as u32,
                    data: MEM1 + at as u32 + 0x20,
                    data_len,
                    id,
                });
                at += bytes.len().next_multiple_of(32);
            }
            None => at += 32,
        }
    }
    found
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
