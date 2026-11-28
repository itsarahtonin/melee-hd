// SPDX-License-Identifier: GPL-3.0-or-later
// Follows NKit's GameCube image format as its reader decodes it, at v3.2.2
// (https://github.com/Nanook/NKit, `NKit/Containers/NKitAsIso.cs`, Copyright (c) 2018 Nanook,
// MIT): the header's fields, the gaps' encodings and their leading zeros, so the disc restored
// here is the one NKit's own reader restores.

//! NKit's GameCube images (`.nkit.iso`, and `.nkit.gcz`, the same compressed with GCZ, which nod
//! opens): the retail disc they were made from, restored.
//!
//! An NKit image isn't the retail disc: NKit keeps the disc's header, executable and file system
//! table, then the files packed one after another, the junk between them (filler the mastering
//! tool generated from the disc's ID) left out and the gaps described by a few words each. Its
//! file system table points at the packed files, so nod opens one as if it were a disc, and Melee
//! would read its files at other offsets than the retail disc's, where the fast disc speed times
//! a read by its offset (`melee_sim::audio::clock::dolphin_read_cycles`) and a read's tail past a
//! file's end is the disc's next bytes ([`crate::disc::Disc::read_after`]): another game.
//! [`crate::disc::Disc::open`] refuses one.
//!
//! [`Restored`] rebuilds the retail layout from the image: each file at its retail offset, the
//! junk between them regenerated from the disc ID with nod's GameCube junk generator
//! (`nod::util::lfg`), the zeros and preserved bytes NKit recorded, the header's NKit fields
//! cleared and the file system table pointing at the retail offsets again. NKit records the
//! retail disc's CRC32 in its header, and [`restore`] checks the RVZ nod converts the restored
//! disc to against it, read back whole once written. The player converts an image
//! once, with the player's yes, to an RVZ in its cache folder, which is then the disc it plays.
//!
//! The header, at 0x200 in the disc header (big-endian, 0x20 bytes, cleared on the retail disc):
//!
//! | Bytes | What |
//! | --- | --- |
//! | 0x200..0x208 | `NKIT v01` |
//! | 0x208..0x20C | the retail disc's CRC32 |
//! | 0x20C..0x210 | the word that makes the NKit image's own CRC32 the retail disc's |
//! | 0x210..0x214 | the retail disc's size |
//! | 0x214..0x218 | the ID the junk is generated from, when not the disc's own (else 0) |
//! | 0x218..0x21C | Wii only: the removed update partition's CRC32 |
//!
//! After the file system table and after each file (in the order of their offsets, then sizes)
//! comes its gap, when the next file doesn't start at once: a word whose low two bits say what
//! the retail disc has there and the rest its length.
//!
//! | Kind | What follows the word | The retail disc's bytes |
//! | --- | --- | --- |
//! | 0, junk | nothing | leading zeros (below), then junk |
//! | 1, zeros | nothing | zeros |
//! | 2, mixed | words of runs of 0x100 bytes: two bits of kind, 30 of count | per run: junk, the preserved bytes themselves, a byte repeated (its value in the count's low 8 bits), or the run before's kind again |
//! | 3, a junk file | the file's size | the file (its count of leading zeros in the word's bits 2 to 7, then junk), then a gap word of its own |
//!
//! Junk follows the end of a file's data after 0x1C zeros, which the retail disc has where the
//! mastering tool didn't fill (NKit's `DataNullsCount`); NKit's rules for how many of them a gap
//! starts with are kept exactly.

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use nod::common::{Compression, Format};
use nod::read::{DiscOptions, DiscReader, DiscStream};
use nod::util::lfg::LaggedFibonacci;
use nod::write::{DiscWriter, FormatOptions, ProcessOptions};

/// The header's magic and the one version NKit writes for GameCube images.
pub const MAGIC: &[u8; 8] = b"NKIT v01";
/// Where the header is: in the disc header, cleared on the retail disc.
const FIELDS: std::ops::Range<usize> = 0x200..0x220;
/// The disc header's and boot header's size, to the file system table's pointers.
const BOOT: usize = 0x440;
/// The zeros after a file's data before junk starts (NKit's `DataNullsCount`).
const DATA_NULLS: u64 = 0x1C;
/// A mixed gap's runs are counted in these.
const RUN: u64 = 0x100;
/// The junk generator restarts every sector.
const SECTOR: u64 = 0x8000;

/// Why an NKit image doesn't restore to the disc it was made from.
#[derive(Debug, thiserror::Error)]
pub enum NKitError {
    #[error(transparent)]
    Nod(#[from] nod::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("it's cut short")]
    Truncated,
    #[error("its {0} is damaged")]
    Damaged(&'static str),
    #[error("it's NKit's {0:?} format, which melee-hd doesn't read")]
    Version(String),
    #[error(
        "the restored disc's CRC32 is {found:08X}, not the {recorded:08X} NKit recorded for it"
    )]
    Mismatch { found: u32, recorded: u32 },
}

/// An NKit image's header: what it says of the retail disc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The retail disc's CRC32.
    pub crc32: u32,
    /// The retail disc's size.
    pub size: u64,
    /// The ID the junk is generated from.
    junk_id: [u8; 4],
    /// The disc number, the junk's other seed.
    disc_num: u8,
}

impl Header {
    /// The header of the disc image `disc` reads, if it's an NKit image: none when the disc header
    /// has no NKit magic, an error when it's an NKit version melee-hd doesn't read.
    pub fn read(disc: &DiscReader) -> Result<Option<Header>, NKitError> {
        let mut boot = [0u8; BOOT];
        let mut r = disc.clone();
        r.seek(SeekFrom::Start(0))?;
        r.read_exact(&mut boot)?;
        Header::parse(&boot)
    }

    /// The header of the disc image at `path`, as [`Header::read`].
    pub fn of(path: impl AsRef<Path>) -> Result<Option<Header>, NKitError> {
        Header::read(&DiscReader::new(path, &DiscOptions::default())?)
    }

    fn parse(boot: &[u8; BOOT]) -> Result<Option<Header>, NKitError> {
        if &boot[0x200..0x204] != b"NKIT" {
            return Ok(None);
        }
        if &boot[0x200..0x208] != MAGIC {
            return Err(NKitError::Version(
                String::from_utf8_lossy(&boot[0x200..0x208]).into_owned(),
            ));
        }
        let junk_id = match be32(boot, 0x214) {
            0 => [boot[0], boot[1], boot[2], boot[3]],
            id => id.to_be_bytes(),
        };
        Ok(Some(Header {
            crc32: be32(boot, 0x208),
            size: u64::from(be32(boot, 0x210)),
            junk_id,
            disc_num: boot[6],
        }))
    }
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn align4(n: u64) -> u64 {
    n.next_multiple_of(4)
}

/// What a stretch of the restored disc is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// The restored header, executable and file system table, from this offset in them.
    Head,
    /// The image's bytes, from this offset in it.
    Image(u64),
    Zero,
    Fill(u8),
    Junk,
}

/// A stretch of the restored disc, from `start` to the next one's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Piece {
    start: u64,
    kind: Kind,
}

/// The restored disc's layout.
struct Layout {
    /// The disc header, everything to the file system table and the table, restored.
    head: Vec<u8>,
    pieces: Vec<Piece>,
    size: u64,
    header: Header,
}

/// A file of the image's file system table, as NKit's reader lists them.
struct File {
    /// Its entry's offset in the table.
    entry: usize,
    offset: u64,
    size: u64,
}

/// The image's files in NKit's order: each directory's own files in table order, then its
/// subdirectories' the same way, sorted (stably) by offset and then size.
fn files(fst: &[u8]) -> Result<Vec<File>, NKitError> {
    const DAMAGED: NKitError = NKitError::Damaged("file system table");
    if fst.len() < 12 {
        return Err(DAMAGED);
    }
    let count = be32(fst, 8) as usize;
    if count == 0 || count.checked_mul(12).is_none_or(|n| n > fst.len()) {
        return Err(DAMAGED);
    }
    // A directory's entry holds the index past its last child.
    fn walk(fst: &[u8], dir: usize, end: usize, out: &mut Vec<File>) -> Result<(), NKitError> {
        let mut subdirs = Vec::new();
        let mut i = dir + 1;
        while i < end {
            let e = i * 12;
            let next = be32(fst, e + 8) as usize;
            if fst[e] & 1 == 1 {
                if next <= i || next > end {
                    return Err(DAMAGED);
                }
                subdirs.push((i, next));
                i = next;
            } else {
                let offset = u64::from(be32(fst, e + 4));
                if offset == 0 {
                    return Err(DAMAGED);
                }
                out.push(File {
                    entry: e,
                    offset,
                    size: next as u64,
                });
                i += 1;
            }
        }
        for (d, end) in subdirs {
            walk(fst, d, end, out)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(fst, 0, count, &mut out)?;
    if out.is_empty() {
        return Err(DAMAGED);
    }
    out.sort_by_key(|f| (f.offset, f.size));
    Ok(out)
}

/// Reads the image's bytes at `at`, refusing a read past its end.
struct Image<'a> {
    disc: &'a mut DiscReader,
    len: u64,
}

impl Image<'_> {
    fn read(&mut self, at: u64, buf: &mut [u8]) -> Result<(), NKitError> {
        if at + buf.len() as u64 > self.len {
            return Err(NKitError::Truncated);
        }
        self.disc.seek(SeekFrom::Start(at))?;
        self.disc.read_exact(buf).map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => NKitError::Truncated,
            _ => e.into(),
        })
    }

    fn word(&mut self, at: &mut u64) -> Result<u64, NKitError> {
        let mut w = [0u8; 4];
        self.read(*at, &mut w)?;
        *at += 4;
        Ok(u64::from(u32::from_be_bytes(w)))
    }
}

/// The walk through the image: where it is in the image and in the restored disc.
struct Walk<'a> {
    image: Image<'a>,
    src: u64,
    dst: u64,
    /// Where the leading zeros after the last file's data end.
    nulls: u64,
    pieces: Vec<Piece>,
}

impl Walk<'_> {
    fn push(&mut self, kind: Kind, len: u64) {
        if len == 0 {
            return;
        }
        self.pieces.push(Piece {
            start: self.dst,
            kind,
        });
        self.dst += len;
    }

    /// The gap after `file` (none: after the table or the last file), `len` bytes of the image
    /// to the next file; `edge` when it's the first or the last file's. NKit's `parseGcGap`.
    fn gap(
        &mut self,
        head: &mut [u8],
        fst_at: usize,
        file: Option<&File>,
        edge: bool,
        len: u64,
    ) -> Result<(), NKitError> {
        let mut size = self.image.word(&mut self.src)?;
        let mut kind = size & 3;
        size &= !3;
        if kind == 3 {
            // A file whose data was junk: zero-sized in the image, its size here.
            self.nulls = self.nulls.min(self.dst);
            let nulls = (size & 0xFC) >> 2;
            let file_len = self.image.word(&mut self.src)?;
            if let Some(f) = file {
                let at = fst_at + f.entry + 8;
                head[at..at + 4].copy_from_slice(&(file_len as u32).to_be_bytes());
            }
            let aligned = align4(file_len);
            if aligned > 0 {
                self.push(Kind::Zero, nulls);
                self.push(Kind::Junk, aligned.saturating_sub(nulls));
            }
            if len <= 8 {
                return Ok(());
            }
            size = self.image.word(&mut self.src)?;
            kind = size & 3;
            size &= !3;
        } else if file.is_some_and(|f| f.size == 0) {
            self.nulls = self.dst + DATA_NULLS;
        }
        if size == 0 {
            return Ok(());
        }
        let leading = |nulls: u64, dst: u64, size: u64, bytes: u64| {
            let max = nulls.saturating_sub(dst);
            if size < max {
                bytes
            } else if bytes >= 0x40000 && !edge {
                0
            } else {
                max.min(bytes)
            }
        };
        match kind {
            0 => {
                let n = leading(self.nulls, self.dst, size, size);
                self.push(Kind::Zero, n);
                self.push(Kind::Junk, size - n);
            }
            1 => self.push(Kind::Zero, size),
            _ => {
                let mut left = size;
                let mut run = 0;
                let mut byte = 0u8;
                while left > 0 {
                    let w = self.image.word(&mut self.src)?;
                    let mut count = w & 0x3FFF_FFFF;
                    let repeat = w >> 30 == 3;
                    if !repeat {
                        run = w >> 30;
                    }
                    if run == 2 && !repeat {
                        byte = count as u8;
                        count >>= 8;
                    }
                    let bytes = (count * RUN).min(left);
                    match run {
                        1 => {
                            self.push(Kind::Image(self.src), bytes);
                            self.src += bytes;
                            if self.src > self.image.len {
                                return Err(NKitError::Truncated);
                            }
                        }
                        2 if byte == 0 => self.push(Kind::Zero, bytes),
                        2 => self.push(Kind::Fill(byte), bytes),
                        _ => {
                            let n = leading(self.nulls, self.dst, left, bytes);
                            self.push(Kind::Zero, n);
                            self.push(Kind::Junk, bytes - n);
                        }
                    }
                    left -= bytes;
                }
            }
        }
        Ok(())
    }
}

impl Layout {
    /// The retail disc's layout from an NKit image (NKit's `ParseGameCube`).
    fn read(disc: &mut DiscReader, header: Header) -> Result<Layout, NKitError> {
        let len = disc.disc_size();
        let mut image = Image { disc, len };
        let mut boot = [0u8; BOOT];
        image.read(0, &mut boot)?;
        let dol = u64::from(be32(&boot, 0x420));
        let fst_at = be32(&boot, 0x424) as usize;
        let fst_size = align4(u64::from(be32(&boot, 0x428))) as usize;
        if fst_at < BOOT || dol >= fst_at as u64 || fst_size == 0 {
            // Melee's executable is before its file system table, as NKit leaves it.
            return Err(NKitError::Damaged("disc header"));
        }
        let mut head = vec![0u8; fst_at + fst_size];
        image.read(0, &mut head)?;
        head[FIELDS].fill(0);
        let files = files(&head[fst_at..])?;

        let mut w = Walk {
            image,
            src: head.len() as u64,
            dst: head.len() as u64,
            nulls: head.len() as u64 + DATA_NULLS,
            pieces: vec![Piece {
                start: 0,
                kind: Kind::Head,
            }],
        };
        let first = files[0].offset;
        if first > w.src && w.dst < header.size {
            let gap = first - w.src;
            w.gap(&mut head, fst_at, Some(&files[0]), true, gap)?;
        }
        for (i, f) in files.iter().enumerate() {
            let last = i == files.len() - 1;
            w.src = w.src.max(f.offset);
            let at = fst_at + f.entry + 4;
            head[at..at + 4].copy_from_slice(&(w.dst as u32).to_be_bytes());
            if f.size > 0 {
                let aligned = align4(f.size).min(header.size.saturating_sub(w.dst));
                if w.src + aligned > len {
                    return Err(NKitError::Truncated);
                }
                w.push(Kind::Image(w.src), aligned);
                w.src += aligned;
                w.nulls = w.dst + DATA_NULLS;
            }
            let next = if last { len } else { files[i + 1].offset };
            let gap = next.saturating_sub(w.src);
            if gap > 0 && w.dst < header.size {
                w.gap(&mut head, fst_at, Some(f), last, gap)?;
            } else if f.size == 0 {
                w.nulls = w.dst + DATA_NULLS;
            }
        }
        let gap = len.saturating_sub(w.src);
        if gap > 0 && w.dst < header.size {
            w.gap(&mut head, fst_at, None, true, gap)?;
        }
        if w.dst > header.size {
            return Err(NKitError::Damaged("gap encoding"));
        }
        let size = header.size;
        let end = w.dst;
        let mut pieces = w.pieces;
        if end < size {
            pieces.push(Piece {
                start: end,
                kind: Kind::Zero,
            });
        }
        // A piece that's empty after a later one at the same start: keep the later.
        pieces.dedup_by(|b, a| {
            if a.start == b.start {
                *a = *b;
                true
            } else {
                false
            }
        });
        Ok(Layout {
            head,
            pieces,
            size,
            header,
        })
    }

    /// The restored disc's bytes at `offset` into `buf`, through `image`.
    fn read_at(&self, image: &mut DiscReader, buf: &mut [u8], offset: u64) -> io::Result<()> {
        if offset + buf.len() as u64 > self.size {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        let mut at = offset;
        let mut out = buf;
        let mut i = self.pieces.partition_point(|p| p.start <= at) - 1;
        while !out.is_empty() {
            let p = self.pieces[i];
            let end = self.pieces.get(i + 1).map_or(self.size, |n| n.start);
            let n = ((end - at) as usize).min(out.len());
            let (chunk, rest) = out.split_at_mut(n);
            let within = at - p.start;
            match p.kind {
                Kind::Head => {
                    let from = within as usize;
                    chunk.copy_from_slice(&self.head[from..from + n]);
                }
                Kind::Image(src) => {
                    image.seek(SeekFrom::Start(src + within))?;
                    image.read_exact(chunk)?;
                }
                Kind::Zero => chunk.fill(0),
                Kind::Fill(b) => chunk.fill(b),
                Kind::Junk => self.junk(chunk, at),
            }
            at += n as u64;
            out = rest;
            i += 1;
        }
        Ok(())
    }

    /// The disc's junk at `offset`: nod's generator, from the junk ID, the disc number and the
    /// sector, zeros past the last whole sector (NKit's `NJunk.Fill`).
    fn junk(&self, buf: &mut [u8], offset: u64) {
        let whole = self.size - self.size % SECTOR;
        let n = (whole.saturating_sub(offset) as usize).min(buf.len());
        let (junk, past) = buf.split_at_mut(n);
        if !junk.is_empty() {
            let mut lfg = LaggedFibonacci::default();
            lfg.fill_sector_chunked(junk, self.header.junk_id, self.header.disc_num, offset);
        }
        past.fill(0);
    }
}

/// The retail disc an NKit image was made from, rebuilt as it's read: a [`DiscStream`] nod opens
/// as the disc.
#[derive(Clone)]
pub struct Restored {
    image: DiscReader,
    layout: Arc<Layout>,
}

impl Restored {
    /// The NKit image at `path` (`.nkit.iso`, or one inside a format nod reads, such as GCZ),
    /// restored.
    pub fn open(path: impl AsRef<Path>) -> Result<Restored, NKitError> {
        Restored::new(DiscReader::new(path, &DiscOptions::default())?)
    }

    /// The NKit image `image` reads, restored. Its layout is read now, the gaps' words alone;
    /// the files and junk as the restored disc is read.
    pub fn new(mut image: DiscReader) -> Result<Restored, NKitError> {
        let header = Header::read(&image)?.ok_or(NKitError::Damaged("NKit header"))?;
        let layout = Layout::read(&mut image, header)?;
        Ok(Restored {
            image,
            layout: Arc::new(layout),
        })
    }

    /// What the image's header says of the retail disc.
    pub fn header(&self) -> Header {
        self.layout.header
    }

    /// The restored disc's size.
    pub fn size(&self) -> u64 {
        self.layout.size
    }

    /// The restored disc's bytes at `offset`.
    pub fn read_at(&mut self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.layout.read_at(&mut self.image, buf, offset)
    }

    /// The restored disc's CRC32, read whole.
    pub fn crc32(&mut self) -> io::Result<u32> {
        crc32_of(self.size(), |buf, at| self.read_at(buf, at))
    }

    /// Checks the restored disc against the CRC32 NKit recorded for the retail disc.
    pub fn verify(&mut self) -> Result<(), NKitError> {
        check(self.crc32()?, self.header().crc32)
    }

    /// The restored disc, as nod opens a disc.
    pub fn into_disc(self) -> Result<DiscReader, NKitError> {
        Ok(DiscReader::new_stream(
            Box::new(self),
            &DiscOptions::default(),
        )?)
    }
}

impl DiscStream for Restored {
    fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.read_at(buf, offset)
    }

    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(self.size())
    }
}

/// The CRC32 of `size` bytes, read in chunks by `read`.
fn crc32_of(size: u64, mut read: impl FnMut(&mut [u8], u64) -> io::Result<()>) -> io::Result<u32> {
    let mut crc = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut at = 0;
    while at < size {
        let n = ((size - at) as usize).min(buf.len());
        read(&mut buf[..n], at)?;
        crc.update(&buf[..n]);
        at += n as u64;
    }
    Ok(crc.finalize())
}

fn check(found: u32, recorded: u32) -> Result<(), NKitError> {
    if found == recorded {
        Ok(())
    } else {
        Err(NKitError::Mismatch { found, recorded })
    }
}

/// The RVZ's compression: Dolphin's default for RVZ, zstd at level 5 in 128 KiB chunks.
const RVZ_LEVEL: i8 = 5;

/// Restores the NKit image at `nkit` to an RVZ at `out`, checked ([`Restored::write_rvz`]);
/// nothing is written beside `nkit`. `progress` hears the bytes converted so far and of all.
pub fn restore(nkit: &Path, out: &Path, progress: impl FnMut(u64, u64)) -> Result<(), NKitError> {
    Restored::open(nkit)?.write_rvz(out, progress)
}

/// Checks the disc image at `path` against the CRC32 `recorded`, read whole through nod.
pub fn verify_file(path: &Path, recorded: u32) -> Result<(), NKitError> {
    let mut disc = DiscReader::new(path, &DiscOptions::default())?;
    let size = disc.disc_size();
    let crc = crc32_of(size, |buf, at| {
        disc.seek(SeekFrom::Start(at))?;
        disc.read_exact(buf)
    })?;
    check(crc, recorded)
}

impl Restored {
    /// Writes the restored disc as an RVZ at `out`, checked: the RVZ as written, read back whole,
    /// against the CRC32 NKit recorded, which checks the restoring and the writing at once.
    /// Written to `out` with `.part` added and renamed to `out` once checked, so `out` is only
    /// ever a checked disc; on a failure the part is deleted. `progress` hears the bytes converted
    /// so far and of all.
    pub fn write_rvz(self, out: &Path, progress: impl FnMut(u64, u64)) -> Result<(), NKitError> {
        self.write_rvz_then(out, progress, |_| {})
    }

    /// [`Restored::write_rvz`], `between` given the part written before it's checked.
    fn write_rvz_then(
        self,
        out: &Path,
        mut progress: impl FnMut(u64, u64),
        between: impl FnOnce(&Path),
    ) -> Result<(), NKitError> {
        let recorded = self.header().crc32;
        let disc = self.into_disc()?;
        let options = FormatOptions {
            format: Format::Rvz,
            compression: Compression::Zstandard(RVZ_LEVEL),
            block_size: Format::Rvz.default_block_size(),
        };
        let writer = DiscWriter::new(disc, &options)?;
        let part = out.with_extension(match out.extension() {
            Some(e) => format!("{}.part", e.to_string_lossy()),
            None => "part".to_owned(),
        });
        let result = (|| {
            let mut file = std::fs::File::create(&part)?;
            let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
            let done = writer.process(
                |data, at, of| {
                    file.write_all(&data)?;
                    progress(at, of);
                    Ok(())
                },
                &ProcessOptions {
                    processor_threads: threads.div_ceil(2),
                    ..Default::default()
                },
            )?;
            if !done.header.is_empty() {
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&done.header)?;
            }
            file.sync_all()?;
            drop(file);
            between(&part);
            verify_file(&part, recorded)?;
            std::fs::rename(&part, out)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A disc header with the GameCube magic, `id` and an NKit header for a disc of `size` bytes
    /// whose CRC32 is `crc`, the file system table at `fst_at` (`fst_len` long), the executable
    /// at 0x440.
    fn boot(size: u32, crc: u32, fst_at: u32, fst_len: u32) -> Vec<u8> {
        let mut b = vec![0u8; BOOT];
        b[..6].copy_from_slice(b"GALE01");
        b[7] = 2;
        b[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        b[0x20..0x24].copy_from_slice(b"Test");
        b[0x200..0x208].copy_from_slice(MAGIC);
        b[0x208..0x20C].copy_from_slice(&crc.to_be_bytes());
        b[0x210..0x214].copy_from_slice(&size.to_be_bytes());
        b[0x420..0x424].copy_from_slice(&0x440u32.to_be_bytes());
        b[0x424..0x428].copy_from_slice(&fst_at.to_be_bytes());
        b[0x428..0x42C].copy_from_slice(&fst_len.to_be_bytes());
        b
    }

    /// A file system table of files with these (offset, size), all in the root, named "a", "b"…
    fn fst(files: &[(u32, u32)]) -> Vec<u8> {
        let n = files.len() as u32 + 1;
        let mut t = Vec::new();
        t.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
        t.extend_from_slice(&n.to_be_bytes());
        for (i, (off, size)) in files.iter().enumerate() {
            t.extend_from_slice(&((i as u32) * 2).to_be_bytes());
            t.extend_from_slice(&off.to_be_bytes());
            t.extend_from_slice(&size.to_be_bytes());
        }
        for i in 0..files.len() {
            t.push(b'a' + i as u8);
            t.push(0);
        }
        while t.len() % 4 != 0 {
            t.push(0);
        }
        t
    }

    fn be(w: u32) -> [u8; 4] {
        w.to_be_bytes()
    }

    /// The disc's junk at `offset`, `len` bytes, as the retail disc has it.
    fn junk(offset: u64, len: usize) -> Vec<u8> {
        let mut b = vec![0u8; len];
        LaggedFibonacci::default().fill_sector_chunked(&mut b, *b"GALE", 0, offset);
        b
    }

    /// A small retail disc and the NKit image of it, each byte of both built by hand: after the
    /// table a junk gap with its leading zeros, file a, a mixed gap (junk, preserved bytes, a
    /// byte repeated, the kind before again), file b, a gap of zeros, a junk file c and the
    /// junk to the disc's end.
    fn pair() -> (Vec<u8>, Vec<u8>) {
        let size: usize = 0x20000;
        let fst_at = 0x2440u32;
        // The retail layout.
        let a = (0x3000u32, 0x105u32);
        let b = (0x5000u32, 0x40u32);
        let c = (0x6000u32, 0x900u32);
        let retail_fst = fst(&[a, b, c]);
        let head_len = fst_at as usize + retail_fst.len();
        let mut retail = vec![0u8; size];
        retail[..BOOT].copy_from_slice(&boot(0, 0, fst_at, retail_fst.len() as u32));
        retail[0x200..0x220].fill(0);
        retail[0x440..0x460].copy_from_slice(&[0x11; 0x20]);
        retail[fst_at as usize..head_len].copy_from_slice(&retail_fst);
        // After the table: 0x1C zeros, then junk to file a.
        let j0 = head_len + 0x1C;
        retail[j0..a.0 as usize].copy_from_slice(&junk(j0 as u64, a.0 as usize - j0));
        let a_data: Vec<u8> = (0..a.1).map(|i| (i * 7 + 1) as u8).collect();
        retail[a.0 as usize..(a.0 + a.1) as usize].copy_from_slice(&a_data);
        // a's end, aligned: 0x3108. Mixed to 0x5000: 0x300 junk runs (0x1C zeros first), 0x100
        // preserved, 0x200 of 0xAB, 0x100 more of 0xAB, junk to 0x5000.
        let a_end = 0x3108usize;
        let mut at = a_end;
        retail[at + 0x1C..at + 0x300].copy_from_slice(&junk((at + 0x1C) as u64, 0x300 - 0x1C));
        at += 0x300;
        let kept: Vec<u8> = (0..0x100).map(|i| (i ^ 0x5A) as u8).collect();
        retail[at..at + 0x100].copy_from_slice(&kept);
        at += 0x100;
        retail[at..at + 0x300].fill(0xAB);
        at += 0x300;
        let rest = b.0 as usize - at;
        retail[at..b.0 as usize].copy_from_slice(&junk(at as u64, rest));
        let b_data = vec![0x77u8; b.1 as usize];
        retail[b.0 as usize..(b.0 + b.1) as usize].copy_from_slice(&b_data);
        // b's end 0x5040: zeros to c at 0x6000; c is junk, 0x900 bytes (no leading zeros: its
        // count is 0); a junk file ends the leading zeros at its start, so the gap after it has
        // none: junk to the end.
        let c_end = (c.0 + c.1) as usize;
        retail[c.0 as usize..c_end].copy_from_slice(&junk(u64::from(c.0), c.1 as usize));
        retail[c_end..].copy_from_slice(&junk(c_end as u64, size - c_end));
        let crc = crc32fast::hash(&retail);

        // The NKit image: the same head (the table pointing at the packed files, c zero-sized),
        // then a's gap words, a, its gap, b, its gap, c's.
        let mut nkit = Vec::new();
        let nk_a = head_len as u32 + 4;
        let mixed: Vec<u32> = vec![
            3, // junk, 3 runs
            (1 << 30) | 1,
            (2 << 30) | (2 << 8) | 0xAB,
            (3 << 30) | 1,
            (0x1000 - 0x700) / 0x100 + 0x100, // junk to b's start: 0x1EF8 bytes, clipped
        ];
        let mixed_len = 4 + 4 * mixed.len() + 0x100;
        let nk_b = nk_a + 0x108 + mixed_len as u32;
        let nk_c = nk_b + 0x40 + 4;
        let nkit_fst = fst(&[(nk_a, a.1), (nk_b, b.1), (nk_c, 0)]);
        nkit.extend_from_slice(&boot(size as u32, crc, fst_at, nkit_fst.len() as u32));
        nkit.resize(fst_at as usize, 0);
        nkit[0x440..0x460].copy_from_slice(&[0x11; 0x20]);
        nkit.extend_from_slice(&nkit_fst);
        assert_eq!(nkit.len(), head_len);
        nkit.extend_from_slice(&be((a.0 - head_len as u32) & !3));
        nkit.extend_from_slice(&a_data);
        nkit.extend_from_slice(&[0, 0, 0]);
        nkit.extend_from_slice(&be(((b.0 - 0x3108) & !3) | 2));
        for (k, w) in mixed.iter().enumerate() {
            nkit.extend_from_slice(&be(*w));
            if k == 1 {
                nkit.extend_from_slice(&kept);
            }
        }
        assert_eq!(nkit.len() as u32, nk_b);
        nkit.extend_from_slice(&b_data);
        nkit.extend_from_slice(&be((0x6000 - 0x5040) | 1));
        assert_eq!(nkit.len() as u32, nk_c);
        nkit.extend_from_slice(&be(3));
        nkit.extend_from_slice(&be(c.1));
        nkit.extend_from_slice(&be((size as u32 - c_end as u32) & !3));
        (retail, nkit)
    }

    fn restored(nkit: Vec<u8>) -> Result<Restored, NKitError> {
        Restored::new(DiscReader::new_stream(
            Box::new(nkit),
            &DiscOptions::default(),
        )?)
    }

    fn read_all(r: &mut Restored) -> Vec<u8> {
        let mut out = vec![0u8; r.size() as usize];
        r.read_at(&mut out, 0).unwrap();
        out
    }

    #[test]
    fn an_image_restores_to_its_retail_disc() {
        let (retail, nkit) = pair();
        let mut r = restored(nkit).unwrap();
        let got = read_all(&mut r);
        let first = got.iter().zip(&retail).position(|(a, b)| a != b);
        assert_eq!(first, None, "the restored disc differs first at {first:x?}");
        r.verify().unwrap();
        // Read in odd pieces, the same.
        let mut piece = vec![0u8; 0x1235];
        for at in (0..retail.len() - piece.len()).step_by(0xF00) {
            r.read_at(&mut piece, at as u64).unwrap();
            assert!(piece[..] == retail[at..at + piece.len()], "at {at:x}");
        }
    }

    #[test]
    fn a_restored_image_opens_as_the_disc() {
        let (retail, nkit) = pair();
        let disc = restored(nkit).unwrap().into_disc().unwrap();
        let mut r = disc.clone();
        let mut got = Vec::new();
        r.read_to_end(&mut got).unwrap();
        assert!(got == retail);
        assert_eq!(Header::read(&disc).unwrap(), None);
    }

    #[test]
    fn a_modified_image_is_refused() {
        let (_, mut nkit) = pair();
        // A byte of b's data.
        let b = nkit.iter().rposition(|&x| x == 0x77).unwrap();
        nkit[b] ^= 1;
        let mut r = restored(nkit).unwrap();
        assert!(matches!(r.verify(), Err(NKitError::Mismatch { .. })));
    }

    /// Cut inside a file's data or a gap's words, an image is refused as cut short; cut in the
    /// last gap's last word, which it can do without, its restored disc doesn't match.
    #[test]
    fn a_truncated_image_is_refused() {
        let (_, nkit) = pair();
        for cut in [nkit.len() - 0x48, nkit.len() - 0x200, nkit.len() - 0x10] {
            let short = nkit[..cut].to_vec();
            assert!(
                matches!(restored(short), Err(NKitError::Truncated)),
                "cut at {cut:x}"
            );
        }
        let short = nkit[..nkit.len() - 4].to_vec();
        assert!(matches!(
            restored(short).unwrap().verify(),
            Err(NKitError::Mismatch { .. })
        ));
    }

    /// One file running to the disc's end, nothing after it: the image restores; cut inside
    /// that file's data, it's refused as cut short.
    #[test]
    fn a_last_file_cut_short_is_refused() {
        let size = 0x8000usize;
        let fst_at = 0x2440u32;
        let t = fst(&[(0x7000, 0x1000)]);
        let head_len = fst_at as usize + t.len();
        let data: Vec<u8> = (0..0x1000).map(|i| (i * 13) as u8).collect();
        let mut retail = vec![0u8; size];
        retail[..BOOT].copy_from_slice(&boot(0, 0, fst_at, t.len() as u32));
        retail[0x200..0x220].fill(0);
        retail[fst_at as usize..head_len].copy_from_slice(&t);
        let j = head_len + 0x1C;
        retail[j..0x7000].copy_from_slice(&junk(j as u64, 0x7000 - j));
        retail[0x7000..].copy_from_slice(&data);
        let mut nkit = boot(
            size as u32,
            crc32fast::hash(&retail),
            fst_at,
            t.len() as u32,
        );
        nkit.resize(fst_at as usize, 0);
        nkit.extend_from_slice(&fst(&[(head_len as u32 + 4, 0x1000)]));
        nkit.extend_from_slice(&be(0x7000 - head_len as u32));
        nkit.extend_from_slice(&data);
        let mut r = restored(nkit.clone()).unwrap();
        let got = read_all(&mut r);
        let first = got.iter().zip(&retail).position(|(a, b)| a != b);
        assert_eq!(first, None, "the restored disc differs first at {first:x?}");
        r.verify().unwrap();
        nkit.truncate(nkit.len() - 0x100);
        assert!(matches!(restored(nkit), Err(NKitError::Truncated)));
    }

    /// The RVZ written is checked read back: one changed after it was written is refused, and
    /// neither it nor its part is left.
    #[test]
    fn an_rvz_written_wrong_is_refused() {
        let (retail, nkit) = pair();
        let dir = std::env::temp_dir().join(format!("melee-data-rvz-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("disc.rvz");
        restored(nkit.clone())
            .unwrap()
            .write_rvz(&out, |_, _| {})
            .unwrap();
        let mut got = Vec::new();
        DiscReader::new(&out, &DiscOptions::default())
            .unwrap()
            .read_to_end(&mut got)
            .unwrap();
        std::fs::remove_file(&out).unwrap();
        let tampered = restored(nkit).unwrap().write_rvz_then(
            &out,
            |_, _| {},
            |part| {
                let mut b = std::fs::read(part).unwrap();
                let at = b.len() - 0x10;
                b[at] ^= 0xFF;
                std::fs::write(part, b).unwrap();
            },
        );
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(got == retail, "the RVZ isn't the disc");
        assert!(tampered.is_err());
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    #[test]
    fn other_versions_are_refused() {
        let (_, mut nkit) = pair();
        nkit[0x207] = b'2';
        assert!(matches!(restored(nkit), Err(NKitError::Version(v)) if v == "NKIT v02"));
    }

    #[test]
    fn files_are_in_nkits_order() {
        // root: file x (0x500), dir d { file y (0x100) }, file z (0x500, size 0): NKit lists the
        // root's files first, then d's, then sorts by offset and size: y, z, x.
        let mut t = Vec::new();
        t.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]);
        t.extend_from_slice(&[0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 8]);
        t.extend_from_slice(&[1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 4]);
        t.extend_from_slice(&[0, 0, 0, 4, 0, 0, 1, 0, 0, 0, 0, 8]);
        t.extend_from_slice(&[0, 0, 0, 6, 0, 0, 5, 0, 0, 0, 0, 0]);
        t.extend_from_slice(b"x\0d\0y\0z\0");
        let order: Vec<usize> = files(&t).unwrap().iter().map(|f| f.entry / 12).collect();
        assert_eq!(order, [3, 4, 1]);
    }
}
