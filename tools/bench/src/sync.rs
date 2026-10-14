//! What a run of a benchmark match is checked against, Dolphin's (`dolphin.rs`) and melee-hd's
//! (`native.rs`) alike: the replay's fighters at each frame's end, and the fighters' addresses
//! in ssbm-rs's trace of the match's start.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ssbm_slippi::replay::{Replay, event};

/// A benchmark match's replay: `DIR/NAME.slp` when `dir` has it, else the repository's
/// `data/bench/NAME.slp`.
pub fn replay_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(format!("{name}.slp"));
    if path.exists() {
        path
    } else {
        melee_local_data::bench_replay(name)
    }
}

/// ssbm-rs's state trace of a benchmark match's start (`make-matches.sh` writes it):
/// `DIR/NAME.start.trace` when `dir` has it, else `$SSBM_RS_LOCAL/bench/NAME.start.trace`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn start_trace_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(format!("{name}.start.trace"));
    if path.exists() {
        path
    } else {
        melee_local_data::ssbm_rs_local(&format!("bench/{name}.start.trace")).into()
    }
}

/// The match's first frame.
pub const FIRST_FRAME: i32 = -123;

/// One fighter at the end of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fighter {
    pub action: u16,
    pub x: u32,
    pub y: u32,
    pub facing: u32,
    pub percent: u32,
    pub stocks: u8,
}

/// Whether the fighters a run shows are the replay's.
pub fn same(got: &[Option<Fighter>; 8], want: &[Option<Fighter>; 8]) -> bool {
    got.iter().zip(want).all(|(g, w)| w.is_none() || g == w)
}

/// The last frame the replay has a post-frame update for.
pub fn replay_last_frame(replay: &Replay) -> i32 {
    replay
        .frames
        .iter()
        .rev()
        .find(|(_, f)| f.complete)
        .map_or(FIRST_FRAME, |(&k, _)| k)
}

/// Each frame's fighters at the end of the frame, from the post-frame updates: leaders in 0
/// to 3, followers (Nana) in 4 to 7.
pub fn post_frames(replay: &Replay) -> BTreeMap<i32, [Option<Fighter>; 8]> {
    let mut out: BTreeMap<i32, [Option<Fighter>; 8]> = BTreeMap::new();
    for e in &replay.events {
        let p = &e.payload;
        if e.command != event::POST_FRAME || p.len() < 0x21 {
            continue;
        }
        let frame = i32::from_be_bytes(p[0..4].try_into().unwrap());
        let word = |at: usize| u32::from_be_bytes(p[at..at + 4].try_into().unwrap());
        let index = usize::from(p[4] & 3) + if p[5] != 0 { 4 } else { 0 };
        out.entry(frame).or_default()[index] = Some(Fighter {
            action: u16::from_be_bytes([p[7], p[8]]),
            x: word(9),
            y: word(0xD),
            facing: word(0x11),
            percent: word(0x15),
            stocks: p[0x20],
        });
    }
    out
}

/// A fighter in an ssbm-rs state trace: its address and whether it's a follower.
#[cfg(windows)]
pub struct TracedFighter {
    pub address: u32,
    pub follower: bool,
}

/// The fighters of the first tick that has any, in an ssbm-rs state trace (`STATE_TRACE`):
/// magic, version, then zstd frames of ticks, each a number, a region count and regions (kind,
/// address, owning GObj, length, bytes; header integers little-endian). Kind 4 is a `Fighter`.
#[cfg(windows)]
pub fn traced_fighters(path: &Path) -> Option<Vec<TracedFighter>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut file = std::io::BufReader::new(file);
    let mut head = [0u8; 12];
    file.read_exact(&mut head).ok()?;
    if &head[..8] != b"SSBMTRC\0" {
        return None;
    }
    let mut z = zstd::stream::read::Decoder::with_buffer(file).ok()?;
    let u32le = |z: &mut dyn Read| -> Option<u32> {
        let mut b = [0u8; 4];
        z.read_exact(&mut b).ok()?;
        Some(u32::from_le_bytes(b))
    };
    loop {
        let _tick = u32le(&mut z)?;
        let count = u32le(&mut z)?;
        let mut out = Vec::new();
        for _ in 0..count {
            let mut kind = [0u8; 1];
            z.read_exact(&mut kind).ok()?;
            let address = u32le(&mut z)?;
            let _gobj = u32le(&mut z)?;
            let len = u32le(&mut z)? as usize;
            let mut bytes = vec![0u8; len];
            z.read_exact(&mut bytes).ok()?;
            // is_sub_fighter, fp+0x221F bit 4.
            if kind[0] == 4 && bytes.len() > 0x221F {
                out.push(TracedFighter {
                    address,
                    follower: bytes[0x221F] & 0x08 != 0,
                });
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
}
