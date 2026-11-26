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
//! skip (or, run on purpose with `--ignored`, fail) with a message naming it ([`skip!`]).
//!
//! `MELEE_CHECKS_REQUIRE_DATA=1` turns each such skip into a failure: a merge check or a verify
//! that means to run a check on its data sets it, so a machine missing the data (the disc,
//! ssbm-rs's captures or traces, the replays) fails the check instead of passing it by running
//! nothing (the refactor's task 26).

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

/// The variable that turns a check's skip for missing data into a failure.
pub const REQUIRE_DATA: &str = "MELEE_CHECKS_REQUIRE_DATA";

/// Whether `MELEE_CHECKS_REQUIRE_DATA` is set (to anything but empty or `0`).
pub fn data_required() -> bool {
    std::env::var(REQUIRE_DATA).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// A check skipping for want of its data: `why` on stderr, or, with
/// `MELEE_CHECKS_REQUIRE_DATA` set, a panic that fails the check ([`skip!`]).
#[track_caller]
pub fn skipped(why: std::fmt::Arguments) {
    skip_or_fail(data_required(), why);
}

#[track_caller]
fn skip_or_fail(required: bool, why: std::fmt::Arguments) {
    if required {
        panic!("{why}: {REQUIRE_DATA} is set, so a check without its data fails");
    }
    eprintln!("{why}");
}

/// `eprintln!`'s form for a check that skips without its data (the disc, `SSBM_RS_LOCAL`'s
/// captures and traces, `SLIPPI_REPLAYS`): the message, or under `MELEE_CHECKS_REQUIRE_DATA` a
/// failure ([`skipped`]). The caller returns as before.
#[macro_export]
macro_rules! skip {
    ($($t:tt)*) => {
        $crate::skipped(format_args!($($t)*))
    };
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
    fn a_skip_fails_when_data_is_required() {
        let why = format_args!("no data here: skipping");
        let fail = std::panic::AssertUnwindSafe(|| skip_or_fail(true, why));
        assert!(std::panic::catch_unwind(fail).is_err());
        skip_or_fail(false, why);
    }

    #[test]
    fn bench_replays_are_in_the_repository() {
        assert!(bench_replay("fox-fox-fd").exists());
    }
}
