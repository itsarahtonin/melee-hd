//! Packs of saved calls: one file per capture directory (`calls.pack` in it) holding every
//! distinct page of the calls' memory once, compressed in small blocks, and each call's name,
//! registers and the pages its memory is made of. The calls of one run share nearly all their
//! memory, so a pack is tens of times smaller than the `.call` files it replaces, and reading a
//! call from it is copying pages: each block is decompressed once per process and shared.
//!
//! [`Capture::read`](crate::Capture::read) and [`call_files`] read packs and directories
//! alike, the pack first: a call's path stays `DIR/NAME.call` whether the file or the pack
//! holds it. `pack-calls DIR...` makes a pack from a directory's `.call` files and checks that
//! every call reads back byte for byte.
//!
//! The file, in little-endian words: `MHDCALLS`, the version, the page size (0x1000), the pages
//! per block and the index's length; the index, compressed: the blocks' lengths, the number of
//! calls and each call's name, head (`Call::to_bytes` without its memory), memory and locked
//! cache lengths and page ids (0 for a page of zeros, else 1 + the page's place in the blocks),
//! each id xor the previous call's at the same place; then the blocks, each its pages
//! compressed together.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ssbm_rt::capture::Call;

/// A pack's name in the directory whose calls it holds.
pub const FILE_NAME: &str = "calls.pack";

const MAGIC: &[u8; 8] = b"MHDCALLS";
const VERSION: u32 = 1;
const PAGE: usize = 0x1000;

/// The page id of a page of zeros, which isn't stored.
const ZERO: u32 = 0;

/// A pack read back: its calls without their memory, and its pages, decompressed a block at a
/// time when first read.
pub struct Pack {
    path: PathBuf,
    calls: Vec<Packed>,
    by_name: HashMap<String, usize>,
    pages_per_block: usize,
    /// Each block's compressed bytes, as ranges of `data`.
    blocks: Vec<std::ops::Range<usize>>,
    data: Vec<u8>,
    decoded: Vec<OnceLock<Box<[u8]>>>,
}

/// A call in a pack: its file name, the call without its memory, and its memory's pages.
struct Packed {
    name: String,
    head: Call,
    mem1_len: usize,
    locked_len: usize,
    pages: Vec<u32>,
}

fn u32_at(b: &[u8], at: &mut usize) -> io::Result<u32> {
    let v = b
        .get(*at..*at + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "a pack cut short"))?;
    *at += 4;
    Ok(u32::from_le_bytes(v.try_into().unwrap()))
}

fn bytes_at<'b>(b: &'b [u8], at: &mut usize, n: usize) -> io::Result<&'b [u8]> {
    let v = b
        .get(*at..*at + n)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "a pack cut short"))?;
    *at += n;
    Ok(v)
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

impl Pack {
    pub fn open(path: &Path) -> io::Result<Pack> {
        let data = std::fs::read(path)?;
        let mut at = 0;
        if bytes_at(&data, &mut at, 8)? != MAGIC {
            return Err(bad("not a pack of calls"));
        }
        if u32_at(&data, &mut at)? != VERSION {
            return Err(bad("a pack of another version"));
        }
        if u32_at(&data, &mut at)? as usize != PAGE {
            return Err(bad("a pack of another page size"));
        }
        let pages_per_block = u32_at(&data, &mut at)? as usize;
        let index_len = u32_at(&data, &mut at)? as usize;
        let index = zstd::decode_all(bytes_at(&data, &mut at, index_len)?)?;
        let mut i = 0;
        let n_blocks = u32_at(&index, &mut i)? as usize;
        let mut blocks = Vec::with_capacity(n_blocks);
        for _ in 0..n_blocks {
            let len = u32_at(&index, &mut i)? as usize;
            blocks.push(at..at + len);
            at += len;
        }
        if at != data.len() {
            return Err(bad("a pack's blocks don't end where it does"));
        }
        let n_calls = u32_at(&index, &mut i)? as usize;
        let mut calls: Vec<Packed> = Vec::with_capacity(n_calls);
        for _ in 0..n_calls {
            let n = u32_at(&index, &mut i)? as usize;
            let name = String::from_utf8(bytes_at(&index, &mut i, n)?.to_vec())
                .map_err(|_| bad("a call's name isn't UTF-8"))?;
            let n = u32_at(&index, &mut i)? as usize;
            let head = Call::from_bytes(bytes_at(&index, &mut i, n)?)
                .ok_or_else(|| bad("a call's head"))?;
            let mem1_len = u32_at(&index, &mut i)? as usize;
            let locked_len = u32_at(&index, &mut i)? as usize;
            let n = mem1_len.div_ceil(PAGE) + locked_len.div_ceil(PAGE);
            // Each call's page ids are stored as their difference (xor) from the previous
            // call's at the same place, which are mostly the same.
            let prev = calls.last().map(|c| &c.pages[..]);
            let mut pages = Vec::with_capacity(n);
            for k in 0..n {
                let d = u32_at(&index, &mut i)?;
                let p = prev.and_then(|p| p.get(k)).copied().unwrap_or(0);
                pages.push(d ^ p);
            }
            if pages
                .iter()
                .any(|&p| p != ZERO && (p as usize - 1) / pages_per_block >= n_blocks)
            {
                return Err(bad("a page outside the pack"));
            }
            calls.push(Packed {
                name,
                head,
                mem1_len,
                locked_len,
                pages,
            });
        }
        let by_name = calls
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i))
            .collect();
        let decoded = (0..n_blocks).map(|_| OnceLock::new()).collect();
        Ok(Pack {
            path: path.to_owned(),
            calls,
            by_name,
            pages_per_block,
            blocks,
            data,
            decoded,
        })
    }

    /// The calls' file names (`NAME-callN.call`), in the order they were packed.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.calls.iter().map(|c| c.name.as_str())
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    fn page(&self, id: u32) -> &[u8] {
        let k = id as usize - 1;
        let (b, within) = (k / self.pages_per_block, k % self.pages_per_block);
        let block = self.decoded[b].get_or_init(|| {
            zstd::bulk::decompress(
                &self.data[self.blocks[b].clone()],
                self.pages_per_block * PAGE,
            )
            .unwrap_or_else(|e| panic!("{}: block {b}: {e}", self.path.display()))
            .into_boxed_slice()
        });
        &block[within * PAGE..(within + 1) * PAGE]
    }

    /// The call saved as `name`, with its memory.
    pub fn call(&self, name: &str) -> Option<Call> {
        let c = &self.calls[*self.by_name.get(name)?];
        let mut mem = vec![0u8; c.pages.len() * PAGE];
        for (dst, &id) in mem.as_chunks_mut::<PAGE>().0.iter_mut().zip(&c.pages) {
            if id != ZERO {
                dst.copy_from_slice(self.page(id));
            }
        }
        let mem1_pages = c.mem1_len.div_ceil(PAGE) * PAGE;
        let mut locked = mem.split_off(mem1_pages);
        locked.truncate(c.locked_len);
        mem.truncate(c.mem1_len);
        Some(Call {
            mem1: mem,
            locked,
            regs: c.head.regs.clone(),
            ..c.head
        })
    }
}

/// The pack in `dir`, opened once per process and shared, or `None` when there's none.
pub fn in_dir(dir: &Path) -> Option<Arc<Pack>> {
    type Open = Mutex<HashMap<PathBuf, Option<Arc<Pack>>>>;
    static OPEN: OnceLock<Open> = OnceLock::new();
    // One pack however the directory is named.
    let path = dir.canonicalize().ok()?.join(FILE_NAME);
    let mut open = OPEN.get_or_init(Default::default).lock().unwrap();
    open.entry(path)
        .or_insert_with_key(|path| {
            path.is_file().then(|| {
                Arc::new(Pack::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
            })
        })
        .clone()
}

/// The call saved at `path` (`DIR/NAME.call`) when `DIR`'s pack holds it.
pub fn read(path: &Path) -> Option<Call> {
    let pack = in_dir(path.parent()?)?;
    pack.call(path.file_name()?.to_str()?)
}

/// What `dir` holds, as paths: its entries, and its pack's calls as the `DIR/NAME.call` files
/// they were, sorted; none when the directory isn't there.
pub fn entries(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let dir = dir.as_ref();
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    if let Some(pack) = in_dir(dir) {
        v.extend(pack.names().map(|n| dir.join(n)));
    }
    v.sort();
    v.dedup();
    v
}

/// The saved calls in `dir` ([`entries`] that are `.call` files).
pub fn call_files(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut v = entries(dir);
    v.retain(|p| p.extension().is_some_and(|x| x == "call"));
    v
}

/// What `dir` holds for the checks, to say why one found no saved calls: whether it's there,
/// and its pack's and `.call` files' calls.
pub fn holdings(dir: impl AsRef<Path>) -> String {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return format!("{} isn't there", dir.display());
    }
    let files = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "call"))
        .count();
    match in_dir(dir) {
        Some(pack) => format!(
            "{} holds a {FILE_NAME} of {} calls and {files} .call files",
            dir.display(),
            pack.len()
        ),
        None => format!(
            "{} holds no {FILE_NAME} and {files} .call files",
            dir.display()
        ),
    }
}

/// Whether the call saved at `path` is there, as a file or in its directory's pack.
pub fn exists(path: &Path) -> bool {
    path.is_file()
        || path.parent().and_then(in_dir).is_some_and(|pack| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| pack.by_name.contains_key(n))
        })
}

/// Writes a pack of calls added one at a time; [`Writer::finish`] writes the file.
pub struct Writer {
    pages_per_block: usize,
    level: i32,
    /// Distinct pages so far, by a hash of their bytes.
    by_hash: HashMap<u64, Vec<u32>>,
    raw: Vec<u8>,
    /// Compressed blocks so far, and the pages not yet in one.
    blocks: Vec<Vec<u8>>,
    index: Vec<u8>,
    prev: Vec<u32>,
    calls: u32,
}

fn hash_page(p: &[u8]) -> u64 {
    // FNV-style over words: only to find candidates, which are compared byte for byte.
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for w in p.as_chunks::<8>().0 {
        h = (h ^ u64::from_le_bytes(*w)).wrapping_mul(0x0000_0100_0000_01B3);
        h ^= h >> 29;
    }
    h
}

impl Writer {
    /// A writer compressing `pages_per_block` pages together at zstd level `level`.
    pub fn new(pages_per_block: usize, level: i32) -> Writer {
        Writer {
            pages_per_block,
            level,
            by_hash: HashMap::new(),
            raw: Vec::new(),
            blocks: Vec::new(),
            index: Vec::new(),
            prev: Vec::new(),
            calls: 0,
        }
    }

    fn page_id(&mut self, page: &[u8]) -> u32 {
        let mut full = [0u8; PAGE];
        full[..page.len()].copy_from_slice(page);
        if full.iter().all(|&b| b == 0) {
            return ZERO;
        }
        let h = hash_page(&full);
        let ids = self.by_hash.entry(h).or_default();
        for &id in ids.iter() {
            let k = (id - 1) as usize;
            if self.raw[k * PAGE..(k + 1) * PAGE] == full {
                return id;
            }
        }
        let id = (self.raw.len() / PAGE) as u32 + 1;
        ids.push(id);
        self.raw.extend_from_slice(&full);
        let done = self.blocks.len() * self.pages_per_block;
        if self.raw.len() / PAGE - done == self.pages_per_block {
            self.seal();
        }
        id
    }

    /// Compresses the pages not yet in a block.
    fn seal(&mut self) {
        let start = self.blocks.len() * self.pages_per_block * PAGE;
        if start < self.raw.len() {
            let block = zstd::bulk::compress(&self.raw[start..], self.level).expect("zstd");
            self.blocks.push(block);
        }
    }

    /// Adds the call saved as `name`.
    pub fn add(&mut self, name: &str, call: &Call) {
        let ids: Vec<u32> = call
            .mem1
            .chunks(PAGE)
            .chain(call.locked.chunks(PAGE))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|p| self.page_id(p))
            .collect();
        let head = Call {
            mem1: Vec::new(),
            locked: Vec::new(),
            regs: call.regs.clone(),
            ..*call
        }
        .to_bytes();
        let ix = &mut self.index;
        ix.extend((name.len() as u32).to_le_bytes());
        ix.extend(name.as_bytes());
        ix.extend((head.len() as u32).to_le_bytes());
        ix.extend(&head);
        ix.extend((call.mem1.len() as u32).to_le_bytes());
        ix.extend((call.locked.len() as u32).to_le_bytes());
        for (k, &id) in ids.iter().enumerate() {
            let p = self.prev.get(k).copied().unwrap_or(0);
            ix.extend((id ^ p).to_le_bytes());
        }
        self.prev = ids;
        self.calls += 1;
    }

    /// Distinct pages so far.
    pub fn pages(&self) -> usize {
        self.raw.len() / PAGE
    }

    /// Writes the pack to `out`.
    pub fn finish(mut self, out: &mut impl Write) -> io::Result<()> {
        self.seal();
        let mut index = Vec::new();
        index.extend((self.blocks.len() as u32).to_le_bytes());
        for b in &self.blocks {
            index.extend((b.len() as u32).to_le_bytes());
        }
        index.extend(self.calls.to_le_bytes());
        index.extend(&self.index);
        let index = zstd::bulk::compress(&index, 19)?;
        out.write_all(MAGIC)?;
        for w in [
            VERSION,
            PAGE as u32,
            self.pages_per_block as u32,
            index.len() as u32,
        ] {
            out.write_all(&w.to_le_bytes())?;
        }
        out.write_all(&index)?;
        for b in &self.blocks {
            out.write_all(b)?;
        }
        out.flush()
    }
}

/// A `.call` file's bytes: the call as `Call::to_bytes` makes it.
pub fn call_file_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let mut packed = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut packed)?;
    zstd::decode_all(&packed[..])
}
