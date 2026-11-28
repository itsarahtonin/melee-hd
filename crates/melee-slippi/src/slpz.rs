// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from slpz 1.3.0 (https://github.com/AlexanderHarrison/slpz, Copyright (c) 2018 Alex
// Harrison, MIT OR Apache-2.0; used here under MIT): the format, the event reordering and the
// compression's parameters, so a file this writes equals the slpz tool's byte for byte.

//! `.slpz`, the compressed form of a `.slp` that Slippi's community tools write (slpz, rwing):
//! the payload sizes, the game start and the metadata uncompressed, so a browser lists a
//! replay without decompressing it, then the rest of the event stream reordered into byte
//! columns and compressed with zstd at the slpz tool's default level.
//!
//! Ported rather than taken as a dependency: the `slpz` crate turns on zstd's `thin` feature,
//! which Cargo unifies into the one zstd every build links (`melee-data`'s, for RVZ discs and
//! the oracle's traces), building it for size with its fast decoders forced off.
//!
//! The layout (all big-endian), a 24-byte header and then its sections, contiguous:
//!
//! | Bytes | What |
//! | --- | --- |
//! | 0..4 | the version, 0 |
//! | 4..8 | the payload sizes' offset (24) |
//! | 8..12 | the game start's offset |
//! | 12..16 | the metadata's offset |
//! | 16..20 | the compressed events' offset, to the file's end |
//! | 20..24 | the reordered events' size, uncompressed |

use std::borrow::Cow;
use std::ops::Range;

/// The file extension, without its dot.
pub const EXTENSION: &str = "slpz";

/// The format's version: files of a later one aren't read.
pub const VERSION: u32 = 0;

/// The zstd level the slpz tool compresses at by default (its `--fast`).
pub const LEVEL: i32 = 3;

/// The header's size.
pub const HEADER: usize = 24;

const EVENT_PAYLOADS: u8 = 0x35;
const GAME_START: u8 = 0x36;
/// A `.slp`'s start: `{U\x03raw[$U#l`, the raw element's length following.
const RAW_HEADER: [u8; 11] = *b"{U\x03raw[$U#l";

/// Why a file couldn't be compressed or decompressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The `.slp` isn't one the format can hold: no raw element's length (a game still being
    /// written), or an event stream cut short or with an unknown command.
    #[error("not a whole Slippi replay")]
    InvalidSlp,
    #[error("not a .slpz file")]
    InvalidSlpz,
    #[error("its compressed events are damaged")]
    Decompression,
    #[error("zstd couldn't compress it")]
    Compression,
}

/// Where a `.slpz`'s sections are, from its header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sections {
    pub payload_sizes: Range<usize>,
    pub game_start: Range<usize>,
    pub metadata: Range<usize>,
    /// The compressed events' offset: they run to the file's end.
    pub events: usize,
    /// The reordered events' size, uncompressed.
    pub events_size: usize,
}

/// The sections a `.slpz` header (its first [`HEADER`] bytes or more) gives, or none when it
/// isn't one: a later version, or offsets out of order. A `.slp` starts with `{` or the
/// payload sizes' command, never the version's zero byte, so the two can't be confused.
pub fn sections(head: &[u8]) -> Option<Sections> {
    let word = |i: usize| -> Option<usize> {
        let b = head.get(i * 4..i * 4 + 4)?;
        Some(u32::from_be_bytes(b.try_into().ok()?) as usize)
    };
    if word(0)? as u32 > VERSION {
        return None;
    }
    let (sizes, start, meta, events) = (word(1)?, word(2)?, word(3)?, word(4)?);
    if sizes < HEADER || sizes > start || start > meta || meta > events {
        return None;
    }
    Some(Sections {
        payload_sizes: sizes..start,
        game_start: start..meta,
        metadata: meta..events,
        events,
        events_size: word(5)?,
    })
}

/// Whether `file` is a `.slpz`, by its header and length (not by its name).
pub fn is_slpz(file: &[u8]) -> bool {
    sections(file).is_some_and(|s| file.len() >= s.events)
}

/// A replay file's `.slp` bytes: a `.slp` as it is, a `.slpz` decompressed.
pub fn slp(file: &[u8]) -> Result<Cow<'_, [u8]>, Error> {
    if is_slpz(file) {
        decompress(file).map(Cow::Owned)
    } else {
        Ok(Cow::Borrowed(file))
    }
}

/// The payload sizes event's table: each command's payload size, and how many commands it
/// lists. None when it's cut short.
fn payload_sizes(events: &[u8]) -> Option<([u16; 256], usize)> {
    let info = usize::from(*events.get(1)?);
    let count = info.checked_sub(1)? / 3;
    let mut sizes = [0; 256];
    for i in 0..count {
        let e = events.get(2 + 3 * i..5 + 3 * i)?;
        sizes[usize::from(e[0])] = u16::from_be_bytes([e[1], e[2]]);
    }
    Some((sizes, count))
}

/// Compresses a `.slp` to a `.slpz`, as the slpz tool does at its default level.
pub fn compress(slp: &[u8]) -> Result<Vec<u8>, Error> {
    if slp.len() < 16 || slp[..11] != RAW_HEADER {
        return Err(Error::InvalidSlp);
    }
    let raw_len = u32::from_be_bytes(slp[11..15].try_into().expect("four bytes")) as usize;
    let meta_at = 15 + raw_len;
    let metadata = slp.get(meta_at..).ok_or(Error::InvalidSlp)?;
    if slp[15] != EVENT_PAYLOADS {
        return Err(Error::InvalidSlp);
    }
    let (sizes, count) = payload_sizes(&slp[15..]).ok_or(Error::InvalidSlp)?;
    let sizes_len = 2 + count * 3;
    let sizes_payload = slp.get(15..15 + sizes_len).ok_or(Error::InvalidSlp)?;
    let start_at = 15 + sizes_len;
    let start_len = usize::from(sizes[usize::from(GAME_START)]) + 1;
    let start_payload = slp
        .get(start_at..start_at + start_len)
        .ok_or(Error::InvalidSlp)?;
    if start_payload[0] != GAME_START {
        return Err(Error::InvalidSlp);
    }
    let others_at = start_at + start_len;
    if meta_at < others_at {
        return Err(Error::InvalidSlp);
    }

    // The tool's buffer, the `.slp`'s size, which zstd compresses into after the sections.
    let mut out = Vec::with_capacity(slp.len());
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&[0; 20]);
    let section = |out: &mut Vec<u8>, word: usize, bytes: &[u8]| {
        let at = out.len() as u32;
        out[word * 4..word * 4 + 4].copy_from_slice(&at.to_be_bytes());
        out.extend_from_slice(bytes);
    };
    section(&mut out, 1, sizes_payload);
    section(&mut out, 2, start_payload);
    section(&mut out, 3, metadata);
    section(&mut out, 4, &[]);
    let events_at = out.len();

    let reordered = reorder(&slp[others_at..meta_at], &sizes)?;
    out[20..24].copy_from_slice(&(reordered.len() as u32).to_be_bytes());
    let mut cursor = std::io::Cursor::new(out);
    cursor.set_position(events_at as u64);
    zstd::bulk::Compressor::new(LEVEL)
        .and_then(|mut z| z.compress_to_buffer(&reordered, &mut cursor))
        .map_err(|_| Error::Compression)?;
    Ok(cursor.into_inner())
}

/// Decompresses a `.slpz` to the `.slp` it was made from.
pub fn decompress(slpz: &[u8]) -> Result<Vec<u8>, Error> {
    let s = sections(slpz).ok_or(Error::InvalidSlpz)?;
    if slpz.len() < s.events {
        return Err(Error::InvalidSlpz);
    }
    let sizes_bytes = &slpz[s.payload_sizes.clone()];
    let (sizes, _) = payload_sizes(sizes_bytes).ok_or(Error::InvalidSlpz)?;
    let mut slp = Vec::with_capacity(slpz.len() * 32);
    slp.extend_from_slice(&RAW_HEADER);
    slp.extend_from_slice(&[0; 4]);
    slp.extend_from_slice(sizes_bytes);
    slp.extend_from_slice(&slpz[s.game_start.clone()]);
    let reordered = zstd::bulk::Decompressor::new()
        .and_then(|mut z| z.decompress(&slpz[s.events..], s.events_size))
        .map_err(|_| Error::Decompression)?;
    unorder(&reordered, &sizes, &mut slp)?;
    let raw_len = slp.len() - 15;
    slp.extend_from_slice(&slpz[s.metadata.clone()]);
    slp[11..15].copy_from_slice(&(raw_len as u32).to_be_bytes());
    Ok(slp)
}

/// Each command's count in an event stream; an unknown command (no payload size) is refused.
fn counts(events: &[u8], sizes: &[u16; 256]) -> Result<[u32; 256], Error> {
    let mut counts = [0u32; 256];
    let mut i = 0;
    while i < events.len() {
        let e = usize::from(events[i]);
        if sizes[e] == 0 {
            return Err(Error::InvalidSlp);
        }
        counts[e] += 1;
        i += 1 + usize::from(sizes[e]);
    }
    Ok(counts)
}

/// Each command's reordered bytes' start, from its payload size and count, and their end.
fn columns(sizes: &[u16; 256], counts: &[u32; 256]) -> ([usize; 256], usize) {
    let mut at = [0; 256];
    let mut end = 0;
    for e in 0..256 {
        at[e] = end;
        end += usize::from(sizes[e]) * counts[e] as usize;
    }
    (at, end)
}

/// The events reordered: their count, their commands in order, then each command's payloads'
/// first bytes, second bytes and so on, command by command.
fn reorder(events: &[u8], sizes: &[u16; 256]) -> Result<Vec<u8>, Error> {
    let counts = counts(events, sizes)?;
    let total = counts.iter().map(|&c| c as usize).sum::<usize>();
    let (at, size) = columns(sizes, &counts);
    // A last event cut short.
    if size != events.len() - total {
        return Err(Error::InvalidSlp);
    }
    let mut data = vec![0; 4 + total + size];
    data[..4].copy_from_slice(&(total as u32).to_be_bytes());
    let (order, columns) = data[4..].split_at_mut(total);
    let mut written = [0usize; 256];
    let mut i = 0;
    for slot in order.iter_mut() {
        let e = usize::from(events[i]);
        *slot = events[i];
        let (n, stride) = (usize::from(sizes[e]), counts[e] as usize);
        let start = at[e] + written[e];
        for j in 0..n {
            columns[start + j * stride] = events[i + 1 + j];
        }
        written[e] += 1;
        i += 1 + n;
    }
    Ok(data)
}

/// Undoes [`reorder`], appending the event stream to `out`.
fn unorder(b: &[u8], sizes: &[u16; 256], out: &mut Vec<u8>) -> Result<(), Error> {
    let total = u32::from_be_bytes(
        b.get(..4)
            .ok_or(Error::Decompression)?
            .try_into()
            .expect("four bytes"),
    ) as usize;
    let order = b.get(4..4 + total).ok_or(Error::Decompression)?;
    let columns_bytes = &b[4 + total..];
    let mut counts = [0u32; 256];
    for &e in order {
        counts[usize::from(e)] += 1;
    }
    let (at, size) = columns(sizes, &counts);
    if size != columns_bytes.len() {
        return Err(Error::Decompression);
    }
    let base = out.len();
    out.resize(base + size + total, 0);
    let data = &mut out[base..];
    let mut written = [0usize; 256];
    let mut i = 0;
    for &cmd in order {
        let e = usize::from(cmd);
        data[i] = cmd;
        let (n, stride) = (usize::from(sizes[e]), counts[e] as usize);
        let start = at[e] + written[e];
        for j in 0..n {
            data[i + 1 + j] = columns_bytes[start + j * stride];
        }
        written[e] += 1;
        i += 1 + n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn bench() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench")
    }

    /// The tool's own reordering test: events of three commands, sizes 3, 1 and 5.
    #[test]
    fn reordering_round_trips() {
        #[rustfmt::skip]
        let events = [
            3, 1, 2, 3, 4, 5,
            1, 0, 1, 2,
            1, 10, 11, 12,
            2, 1,
            2, 2,
            3, 1, 2, 3, 4, 5,
            1, 20, 21, 22,
        ];
        let mut sizes = [0u16; 256];
        sizes[..4].copy_from_slice(&[0, 3, 1, 5]);
        let r = reorder(&events, &sizes).unwrap();
        #[rustfmt::skip]
        assert_eq!(r, [
            0, 0, 0, 7, 3, 1, 1, 2, 2, 3, 1,
            0, 10, 20, 1, 11, 21, 2, 12, 22,
            1, 2,
            1, 1, 2, 2, 3, 3, 4, 4, 5, 5,
        ]);
        let mut back = Vec::new();
        unorder(&r, &sizes, &mut back).unwrap();
        assert_eq!(back, events);
        // An unknown command, and a last event cut short.
        assert_eq!(reorder(&[4, 0], &sizes), Err(Error::InvalidSlp));
        assert_eq!(
            reorder(&events[..events.len() - 1], &sizes),
            Err(Error::InvalidSlp)
        );
    }

    /// Every benchmark replay compresses and decompresses back to its bytes, its sections
    /// uncompressed where the header says, and reads as a replay either way.
    #[test]
    fn the_benchmark_replays_round_trip() {
        let mut n = 0;
        for e in std::fs::read_dir(bench()).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_none_or(|x| x != "slp") {
                continue;
            }
            let slp = std::fs::read(&path).unwrap();
            let slpz = compress(&slp).unwrap();
            assert!(is_slpz(&slpz) && !is_slpz(&slp), "{}", path.display());
            assert!(
                slpz.len() * 4 < slp.len(),
                "{}: {}",
                path.display(),
                slpz.len()
            );
            assert_eq!(decompress(&slpz).unwrap(), slp, "{}", path.display());
            assert!(matches!(self::slp(&slp), Ok(Cow::Borrowed(_))));
            let s = sections(&slpz).unwrap();
            let raw = crate::replay::raw_events(&slp).unwrap();
            let start = s.game_start.len() + s.payload_sizes.len();
            assert_eq!(slpz[s.payload_sizes.start..s.game_start.end], raw[..start]);
            let meta_at = 15 + raw.len();
            assert_eq!(slpz[s.metadata.clone()], slp[meta_at..]);
            let a = crate::Replay::parse(&slp).unwrap();
            let b = crate::Replay::parse(&slpz).unwrap();
            assert_eq!((a.events, a.header), (b.events, b.header));
            n += 1;
        }
        assert!(n >= 6, "{n} benchmark replays");
    }

    /// The `.slp` files in `dir` and its subfolders, by their path in it.
    fn replays_in(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                replays_in(root, &path, out);
            } else if path.extension().is_some_and(|x| x == "slp") {
                out.push(path.strip_prefix(root).unwrap().to_owned());
            }
        }
    }

    /// Every replay under `SLIPPI_REPLAYS` and every benchmark replay compresses and
    /// decompresses back to its bytes. With `SLPZ_REFERENCE` naming a copy of both that the slpz
    /// tool compressed (built from its source; `slippi/` holding `SLIPPI_REPLAYS`, `bench/` the
    /// benchmark replays, after `slpz -r -x` each `.slpz` beside its `.slp`), each `.slpz` of
    /// ours equals the tool's byte for byte, and the tool's decompresses to the replay. The
    /// bytes are equal only when the tool links the zstd melee-hd does (1.5.7): zstd's output
    /// changes between its versions (1.5.6's differs for 211 of 214), as any version's reads.
    #[test]
    fn every_replay_round_trips_and_equals_the_tools() {
        let Some(dir) = std::env::var_os("SLIPPI_REPLAYS") else {
            melee_local_data::skip!("skipped: SLIPPI_REPLAYS isn't set");
            return;
        };
        let reference = std::env::var_os("SLPZ_REFERENCE").map(PathBuf::from);
        let mut n = (0, 0);
        for (root, key) in [(PathBuf::from(dir), "slippi"), (bench(), "bench")] {
            let mut found = Vec::new();
            replays_in(&root, &root, &mut found);
            for rel in found {
                let slp = std::fs::read(root.join(&rel)).unwrap();
                let slpz = compress(&slp).unwrap_or_else(|e| panic!("{}: {e}", rel.display()));
                assert_eq!(decompress(&slpz).unwrap(), slp, "{}", rel.display());
                n.0 += 1;
                let Some(r) = &reference else { continue };
                let theirs = std::fs::read(r.join(key).join(&rel).with_extension(EXTENSION))
                    .unwrap_or_else(|e| panic!("{}'s reference: {e}", rel.display()));
                assert!(slpz == theirs, "{}: not the tool's bytes", rel.display());
                assert_eq!(decompress(&theirs).unwrap(), slp, "{}", rel.display());
                n.1 += 1;
            }
        }
        eprintln!("{} replays round trip, {} equal the tool's", n.0, n.1);
        assert!(n.0 > 6);
    }

    /// What isn't a whole `.slp` isn't compressed, what isn't a `.slpz` isn't decompressed, and
    /// a damaged one is refused rather than read.
    #[test]
    fn damaged_files_are_refused() {
        let slp = std::fs::read(bench().join("fox-fox-fd.slp")).unwrap();
        assert_eq!(compress(b"hello"), Err(Error::InvalidSlp));
        // A game still being written: Slippi fills in the raw element's length at its end.
        let mut open = slp.clone();
        open[11..15].fill(0);
        assert_eq!(compress(&open), Err(Error::InvalidSlp));
        assert_eq!(decompress(&slp), Err(Error::InvalidSlpz));
        let slpz = compress(&slp).unwrap();
        // Cut short, a later version, offsets out of order.
        assert_eq!(decompress(&slpz[..100]), Err(Error::InvalidSlpz));
        assert_eq!(
            decompress(&slpz[..slpz.len() - 10]),
            Err(Error::Decompression)
        );
        let mut later = slpz.clone();
        later[3] = 1;
        assert_eq!(decompress(&later), Err(Error::InvalidSlpz));
        let mut order = slpz.clone();
        order[8..12].copy_from_slice(&(slpz.len() as u32 + 1).to_be_bytes());
        assert_eq!(decompress(&order), Err(Error::InvalidSlpz));
        // The events' count past the reordered bytes.
        let s = sections(&slpz).unwrap();
        let mut bytes = zstd::bulk::decompress(&slpz[s.events..], s.events_size).unwrap();
        bytes[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut sizes = [0; 256];
        sizes[0x36] = 1;
        assert_eq!(
            unorder(&bytes, &sizes, &mut Vec::new()),
            Err(Error::Decompression)
        );
        assert_eq!(
            unorder(&[0, 0], &sizes, &mut Vec::new()),
            Err(Error::Decompression)
        );
    }
}
