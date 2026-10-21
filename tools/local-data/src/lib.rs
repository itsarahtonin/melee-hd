//! Where the checks and tools find their data. The benchmark matches' replays are in the
//! repository ([`bench_dir`]); everything else is game memory or someone's own replays, kept
//! out of it and named by two environment variables, with no default:
//!
//! - `SSBM_RS_LOCAL`: ssbm-rs's `local/` folder, where ssbm-rs's runner writes the captures
//!   (`calls/`), state traces (`traces/`) and references (`player/`) the checks compare with.
//! - `SLIPPI_REPLAYS`: a folder of Slippi replays (as Slippi Launcher keeps them, a folder a
//!   month), for the checks that play replays beyond the benchmark matches.
//!
//! Unset, a path under one names the variable and never exists, so the checks that need it
//! skip (or, run on purpose with `--ignored`, fail) with a message naming it.

use std::path::{Path, PathBuf};
use std::sync::Once;

/// The variable naming ssbm-rs's `local/` folder.
pub const SSBM_RS_LOCAL: &str = "SSBM_RS_LOCAL";

/// The variable naming a folder of Slippi replays.
pub const SLIPPI_REPLAYS: &str = "SLIPPI_REPLAYS";

/// `rel` in ssbm-rs's `local/` folder (`$SSBM_RS_LOCAL/rel`).
pub fn ssbm_rs_local(rel: &str) -> String {
    static NOTE: Once = Once::new();
    under(
        SSBM_RS_LOCAL,
        rel,
        &NOTE,
        "ssbm-rs's captures, traces and references",
    )
}

/// `rel` in the folder of Slippi replays (`$SLIPPI_REPLAYS/rel`).
pub fn slippi_replays(rel: &str) -> String {
    static NOTE: Once = Once::new();
    under(SLIPPI_REPLAYS, rel, &NOTE, "Slippi replays")
}

/// The benchmark matches' replays, in the repository's `data/bench`.
pub fn bench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench")
}

/// A benchmark match's replay, `data/bench/NAME.slp`.
pub fn bench_replay(name: &str) -> PathBuf {
    bench_dir().join(format!("{name}.slp"))
}

/// Appends `VAR<TAB>rel` to the file `MELEE_LOCAL_DATA_LOG` names, once per process for each:
/// leaf-select's coverage pass learns from it which data a leaf-check job reads, and keys the
/// job on that data (docs/oracle.md, "Selecting leaf-check jobs").
fn log(var: &str, rel: &str) {
    use std::collections::BTreeSet;
    use std::io::Write;
    use std::sync::Mutex;
    static LOGGED: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());
    let Some(path) = std::env::var_os("MELEE_LOCAL_DATA_LOG") else {
        return;
    };
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    if logged.insert((var.to_owned(), rel.to_owned())) {
        let line = format!("{var}\t{rel}\n");
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(line.as_bytes()))
            .unwrap_or_else(|e| panic!("MELEE_LOCAL_DATA_LOG={}: {e}", path.display()));
    }
}

/// `$var/rel`, or a path naming the unset variable. An absolute `rel` is kept as it is.
fn under(var: &str, rel: &str, note: &Once, what: &str) -> String {
    if Path::new(rel).is_absolute() {
        // No variable's: logged under one that's never set, so the path stands alone.
        log("-", rel);
        return rel.to_owned();
    }
    log(var, rel);
    match std::env::var(var) {
        Ok(base) if !base.is_empty() => {
            let base = base.trim_end_matches(['/', '\\']);
            if rel.is_empty() {
                base.to_owned()
            } else {
                format!("{base}/{rel}")
            }
        }
        _ => {
            note.call_once(|| {
                eprintln!("{var} is unset: the checks that need {what} skip without them");
            });
            format!("<{var} unset>/{rel}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_paths_name_the_variable_and_dont_exist() {
        let p = under(
            "MELEE_LOCAL_DATA_TEST_UNSET",
            "calls/fox",
            &Once::new(),
            "data",
        );
        assert_eq!(p, "<MELEE_LOCAL_DATA_TEST_UNSET unset>/calls/fox");
        assert!(!Path::new(&p).exists());
    }

    #[test]
    fn bench_replays_are_in_the_repository() {
        assert!(bench_replay("fox-fox-fd").exists());
    }
}
