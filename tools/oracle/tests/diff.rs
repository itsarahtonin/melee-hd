//! `melee-oracle diff` on a real trace of a replay: the import of a tick compares back clean,
//! a later tick's memory differs, and the run stops where the match reaches unported code,
//! naming it.
//!
//! The trace is ssbm-rs's, with memory (branch `feat/state-trace-memory`), kept out of the
//! repository: `TRACE` (default below) of the Fox vs Sheik Final Destination replay `REPLAY`,
//! made as `docs/oracle.md` says. The tests skip without the trace, the replay or `SSBM_DISC`.

use std::process::Command;

const TRACE: &str = "C:/Projects/ssbm-rs-publication/local/traces/v4/Game_20260129T133652.trc";
const REPLAY: &str = "C:/Users/USER/Documents/Slippi/2026-01/Game_20260129T133652.slp";

/// The oracle's output for `args` after `diff TRACE REPLAY`, or none when what it needs isn't
/// here.
fn diff(args: &[&str]) -> Option<String> {
    let trace = std::env::var("TRACE").unwrap_or_else(|_| TRACE.into());
    let replay = std::env::var("REPLAY").unwrap_or_else(|_| REPLAY.into());
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&replay).exists()
    {
        eprintln!("no trace, replay or disc: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["diff", &trace, &replay])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some(text)
}

#[test]
fn import_compares_back_clean() {
    let Some(out) = diff(&["--from", "200"]) else {
        return;
    };
    assert!(out.contains("trace tick 200 imported and compared back: 0 differences"));
    // The match stops at the first process no port covers, named with the tick.
    assert!(
        out.contains("stopped in tick 201 (replay frame 77)"),
        "{out}"
    );
    assert!(out.contains("isn't ported"), "{out}");
}

#[test]
fn later_tick_differs() {
    let Some(out) = diff(&["--from", "200", "--against", "201"]) else {
        return;
    };
    let line = out
        .lines()
        .find(|l| l.contains("compared with tick 201"))
        .expect("the comparison's count");
    let n: usize = line
        .rsplit(": ")
        .next()
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .expect("a count");
    assert!(
        n > 0,
        "tick 200's state compared clean with tick 201's memory"
    );
}

#[test]
fn start_reports_its_seam() {
    let Some(out) = diff(&[]) else {
        return;
    };
    // Until the whole match is ported, it stops at a seam, before a difference.
    assert!(!out.contains(" differs in "), "{out}");
    assert!(out.contains("isn't ported"), "{out}");
}

#[test]
fn boot_state_seeds() {
    let Some(out) = diff(&["--boot-diff"]) else {
        return;
    };
    // What the seeding leaves different from the boot state is only what the scene's own
    // setup sets: the button maps and the rumble data pointer.
    let mut lines = out
        .lines()
        .skip_while(|l| !l.contains("against the boot state"));
    assert!(lines.next().is_some(), "{out}");
    let diffs: Vec<&str> = lines.take_while(|l| l.starts_with("  ")).collect();
    assert!(!diffs.is_empty(), "{out}");
    for d in diffs {
        assert!(
            d.starts_with("  controller_map") || d.starts_with("  lb_804D63C0"),
            "{d}"
        );
    }
}
