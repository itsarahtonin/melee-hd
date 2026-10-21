//! The comparer's coverage, for the golden hashes (`src/golden.rs`): a hash of the sources that
//! decide what `melee-oracle diff` compares and how a state is hashed. A golden written by an
//! oracle whose coverage differs is stale.

use std::path::{Path, PathBuf};

/// What the full diff checks and how the golden hashes the state: the comparer and importer
/// (`melee-capture`), the mirrored structs' comparisons, the diff itself, the state hash.
const SOURCES: &[&str] = &[
    "../capture/src",
    "src/diff.rs",
    "src/golden.rs",
    "../../crates/melee-sim/src/game/mirror.rs",
    "../../crates/melee-sim/src/game/mirror_gen.rs",
    "../../crates/melee-sim/src/state_eq.rs",
    "../../crates/melee-derive/src/lib.rs",
];

fn files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for e in entries {
            files(&e, out);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.push(path.to_owned());
    }
}

fn main() {
    let mut paths = Vec::new();
    for s in SOURCES {
        println!("cargo:rerun-if-changed={s}");
        files(Path::new(s), &mut paths);
    }
    // FNV-1a over each file's path and contents, line endings normalized.
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    for p in &paths {
        let name = p.to_string_lossy().replace('\\', "/");
        feed(name.as_bytes());
        feed(&[0]);
        let text = std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let text: Vec<u8> = text.into_iter().filter(|&b| b != b'\r').collect();
        feed(&(text.len() as u64).to_le_bytes());
        feed(&text);
    }
    println!("cargo:rustc-env=ORACLE_COVERAGE={h:016x}");
}
