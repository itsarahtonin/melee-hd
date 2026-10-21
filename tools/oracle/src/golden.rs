//! Golden hashes for `melee-oracle diff` (docs/oracle.md, "Golden hashes"): once a replay's
//! full diff passes, its golden file keeps a 64-bit hash of melee-hd's state after every tick
//! it compared, and later runs (`--golden-check`) only simulate, hash and compare the hashes:
//! no trace past its boot state, no comparison.
//!
//! The hash is the whole state as the sync test compares it (`StateEq::state_hash`: every field
//! of `Game` but spare storage, floats by bit pattern, an unknown-until-written value as the
//! unknown it is), the allocator model (`hsd::heap`) only with `--heap`, as the comparer takes
//! it in only then. That is more than the comparer reads (freed pool slots, which nothing live
//! reaches, and the bookkeeping the original keeps nowhere), so a golden can miss on a change
//! the full diff would pass, but never pass a state the full diff would fail: equal hashes mean
//! the state the comparer compared is, field for field, the clean run's.
//!
//! The header names everything that makes a golden stale: the trace (its size and a hash of
//! its bytes; it also holds the boot state the match starts from), the replay, the options
//! that change what runs or what's compared, the comparer's coverage (a hash of its sources,
//! `build.rs`), the disc files the match loaded, and the format's version.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use melee_sim::game::vs_match::Match;
use melee_sim::state_eq::{StateEq, StateHasher};

/// The format's version: a golden of another version is stale.
pub const FORMAT: u32 = 1;

/// The comparer's coverage, from `build.rs`.
pub const COVERAGE: &str = env!("ORACLE_COVERAGE");

const MAGIC: &str = "melee-oracle golden";

/// What a golden was written for: equal identities, and the golden applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub trace_size: u64,
    pub trace_hash: u64,
    pub replay_size: u64,
    pub replay_hash: u64,
    /// The options that change what runs or what's compared, in one canonical line.
    pub options: String,
    pub coverage: String,
    /// The disc files the match loaded (names, entry numbers, bytes), once it has started.
    pub disc: u64,
    /// The trace tick that ran the match's first tick.
    pub first_tick: u32,
}

/// Why a run stopped where it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The trace ended.
    Trace,
    /// The match ended.
    Match,
    /// `--ticks` ticks were compared.
    Limit,
}

impl End {
    fn name(self) -> &'static str {
        match self {
            End::Trace => "trace",
            End::Match => "match",
            End::Limit => "limit",
        }
    }

    fn parse(s: &str) -> Option<End> {
        Some(match s {
            "trace" => End::Trace,
            "match" => End::Match,
            "limit" => End::Limit,
            _ => return None,
        })
    }
}

pub struct Golden {
    pub id: Identity,
    pub end: End,
    /// The full diff's stale-read report, a line each.
    pub stale: Vec<String>,
    /// The state's hash after each compared tick, from the match's first.
    pub hashes: Vec<u64>,
    /// Where the trace and the replay were when it was written: for people, not compared.
    pub trace_path: String,
    pub replay_path: String,
}

/// The golden's default path: `$SSBM_RS_LOCAL/golden/`, named by the trace and the options
/// that pick a verify job (the float mode, `--heap`).
pub fn default_path(trace: &str, fp: &str, heap: bool) -> Option<PathBuf> {
    let local = std::env::var("SSBM_RS_LOCAL").ok()?;
    let stem = Path::new(trace).file_stem()?.to_string_lossy().into_owned();
    let heap = if heap { ".heap" } else { "" };
    let local = local.trim_end_matches(['/', '\\']);
    Some(PathBuf::from(format!(
        "{local}/golden/{stem}.{fp}{heap}.golden"
    )))
}

/// A hash of a file's bytes, and its size.
pub fn file_hash(path: &str) -> std::io::Result<(u64, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = StateHasher::new();
    let mut buf = vec![0u8; 8 << 20];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        h.bytes(&buf[..n]);
    }
    Ok((size, h.finish()))
}

/// A hash of bytes in memory, and their size.
pub fn bytes_hash(bytes: &[u8]) -> (u64, u64) {
    let mut h = StateHasher::new();
    for c in bytes.chunks(8 << 20) {
        h.bytes(c);
    }
    (bytes.len() as u64, h.finish())
}

/// The state's hash after a tick: the whole `Game`, the allocator model only with `heap`, and
/// the archives the match's start loaded by name.
pub fn state_hash(m: &mut Match, heap: bool) -> u64 {
    let mut h = StateHasher::new();
    if heap {
        m.game.state_hash(&mut h);
    } else {
        // Left out as the comparer leaves it out; put back before the next tick.
        let model = std::mem::take(&mut m.game.objs.heap);
        m.game.state_hash(&mut h);
        m.game.objs.heap = model;
    }
    m.files.state_hash(&mut h);
    h.finish()
}

impl Golden {
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::new();
        let id = &self.id;
        text.push_str(&format!("{MAGIC} {FORMAT}\n"));
        text.push_str(&format!("trace {}\n", self.trace_path));
        text.push_str(&format!("trace-size {}\n", id.trace_size));
        text.push_str(&format!("trace-hash {:016x}\n", id.trace_hash));
        text.push_str(&format!("replay {}\n", self.replay_path));
        text.push_str(&format!("replay-size {}\n", id.replay_size));
        text.push_str(&format!("replay-hash {:016x}\n", id.replay_hash));
        text.push_str(&format!("options {}\n", id.options));
        text.push_str(&format!("coverage {}\n", id.coverage));
        text.push_str(&format!("disc {:016x}\n", id.disc));
        text.push_str(&format!("first-tick {}\n", id.first_tick));
        text.push_str(&format!("end {}\n", self.end.name()));
        for s in &self.stale {
            text.push_str(&format!("stale {s}\n"));
        }
        text.push_str(&format!("hashes {}\n", self.hashes.len()));
        let mut bytes = text.into_bytes();
        for h in &self.hashes {
            bytes.extend_from_slice(&h.to_le_bytes());
        }
        // Written whole, then renamed over the old one.
        let tmp = path.with_extension("golden.tmp");
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    }

    /// The golden at `path`, or why it can't be read.
    pub fn read(path: &Path) -> Result<Golden, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut at = 0;
        let mut line = || -> Result<&str, String> {
            let end = bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or("a header line ends early")?;
            let l = std::str::from_utf8(&bytes[at..at + end]).map_err(|e| e.to_string())?;
            at += end + 1;
            Ok(l)
        };
        let first = line()?;
        let version = first
            .strip_prefix(MAGIC)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .ok_or("not a golden file")?;
        if version != FORMAT {
            return Err(format!(
                "format version {version}, this oracle writes {FORMAT}"
            ));
        }
        let mut fields = std::collections::HashMap::new();
        let mut stale = Vec::new();
        let count = loop {
            let l = line()?;
            let (k, v) = l.split_once(' ').unwrap_or((l, ""));
            match k {
                "stale" => stale.push(v.to_owned()),
                "hashes" => break v.parse::<usize>().map_err(|e| e.to_string())?,
                _ => {
                    fields.insert(k.to_owned(), v.to_owned());
                }
            }
        };
        let get = |k: &str| fields.get(k).cloned().ok_or(format!("no {k} line"));
        let num = |k: &str| -> Result<u64, String> {
            get(k)?.parse::<u64>().map_err(|e| format!("{k}: {e}"))
        };
        let hex = |k: &str| -> Result<u64, String> {
            u64::from_str_radix(&get(k)?, 16).map_err(|e| format!("{k}: {e}"))
        };
        let rest = &bytes[at..];
        if rest.len() != count * 8 {
            return Err(format!(
                "{} bytes of hashes where {count} take {}",
                rest.len(),
                count * 8
            ));
        }
        Ok(Golden {
            id: Identity {
                trace_size: num("trace-size")?,
                trace_hash: hex("trace-hash")?,
                replay_size: num("replay-size")?,
                replay_hash: hex("replay-hash")?,
                options: get("options")?,
                coverage: get("coverage")?,
                disc: hex("disc")?,
                first_tick: num("first-tick")? as u32,
            },
            end: End::parse(&get("end")?).ok_or("an unknown end")?,
            stale,
            hashes: rest
                .as_chunks::<8>()
                .0
                .iter()
                .map(|&c| u64::from_le_bytes(c))
                .collect(),
            trace_path: get("trace")?,
            replay_path: get("replay")?,
        })
    }

    /// What makes the golden stale for `id`, if anything: each identity field that differs.
    /// `started` says whether `id` knows the disc files and first tick yet (the match has
    /// started).
    pub fn stale_for(&self, id: &Identity, started: bool) -> Vec<String> {
        let g = &self.id;
        let mut why = Vec::new();
        if (g.trace_size, g.trace_hash) != (id.trace_size, id.trace_hash) {
            why.push(format!(
                "the trace differs (size {} hash {:016x}, the golden's {} {:016x})",
                id.trace_size, id.trace_hash, g.trace_size, g.trace_hash
            ));
        }
        if (g.replay_size, g.replay_hash) != (id.replay_size, id.replay_hash) {
            why.push(format!(
                "the replay differs (size {} hash {:016x}, the golden's {} {:016x})",
                id.replay_size, id.replay_hash, g.replay_size, g.replay_hash
            ));
        }
        if g.options != id.options {
            why.push(format!(
                "the options differ ({}; the golden's {})",
                id.options, g.options
            ));
        }
        if g.coverage != id.coverage {
            why.push(format!(
                "the comparer's coverage differs ({}; the golden's {})",
                id.coverage, g.coverage
            ));
        }
        if started && g.disc != id.disc {
            why.push(format!(
                "the disc files the match loaded differ ({:016x}; the golden's {:016x})",
                id.disc, g.disc
            ));
        }
        if started && g.first_tick != id.first_tick {
            why.push(format!(
                "the match starts at trace tick {}, the golden's at {}",
                id.first_tick, g.first_tick
            ));
        }
        why
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden() -> Golden {
        Golden {
            id: Identity {
                trace_size: 10,
                trace_hash: 0x1234,
                replay_size: 20,
                replay_hash: 0x5678,
                options: "fp=hardware heap".into(),
                coverage: COVERAGE.into(),
                disc: 0x9abc,
                first_tick: 1,
            },
            end: End::Trace,
            stale: vec!["tick 5: HSD's objects".into()],
            hashes: vec![1, 2, u64::MAX],
            trace_path: "a.trc".into(),
            replay_path: "a.slp".into(),
        }
    }

    #[test]
    fn writes_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("melee-oracle-golden-{}", std::process::id()));
        let path = dir.join("a.golden");
        let g = golden();
        g.write(&path).unwrap();
        let back = Golden::read(&path).unwrap();
        assert_eq!(back.id, g.id);
        assert_eq!(back.end, g.end);
        assert_eq!(back.stale, g.stale);
        assert_eq!(back.hashes, g.hashes);
        assert!(back.stale_for(&g.id, true).is_empty());
        let mut other = g.id.clone();
        other.options = "fp=slippi heap".into();
        other.trace_hash ^= 1;
        assert_eq!(back.stale_for(&other, true).len(), 2);
        // A truncated file is refused, not read short.
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 3]).unwrap();
        assert!(Golden::read(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn one_value_changes_the_hash() {
        let (_, a) = bytes_hash(&[0u8; 4096]);
        let mut b = [0u8; 4096];
        b[1234] = 1;
        assert_ne!(a, bytes_hash(&b).1);
    }
}
