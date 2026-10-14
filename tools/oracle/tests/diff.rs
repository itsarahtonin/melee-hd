//! `melee-oracle diff` on a real trace of a replay: the import of a tick compares back clean,
//! a later tick's memory differs, and the run stops where the match reaches unported code,
//! naming it.
//!
//! The trace is ssbm-rs's, with memory (branch `feat/state-trace-memory`), kept out of the
//! repository: `TRACE` (default below) of the Fox vs Sheik Final Destination replay `REPLAY`,
//! made as `docs/oracle.md` says, and a trace made at a drive's pace (`docs/player.md`). The
//! tests skip without the trace, the replay or `SSBM_DISC`.

use std::process::Command;

const TRACE: &str =
    "C:/Projects/ssbm-rs-publication/local/traces/v5-mixed/Game_20260129T133652.trc";
const REPLAY: &str = "C:/Users/USER/Documents/Slippi/2026-01/Game_20260129T133652.slp";

/// A trace ssbm-rs made reading the disc at a drive's pace (`DISC_RATE=3000000`), of the Fox
/// vs Marth benchmark match on Final Destination, whose song stops at the fast disc speed
/// (`docs/player.md`), with memory over its first 4,500 fields, and the replay.
const DRIVE_TRACE: &str =
    "C:/Projects/ssbm-rs-publication/local/player/audio-drive-fox-marth-fd.trc";
const DRIVE_REPLAY: &str = "C:/Projects/ssbm-rs-publication/local/bench/fox-marth-fd.slp";

/// The oracle's output for `args` after `diff TRACE REPLAY`, or none when what it needs isn't
/// here.
fn diff(args: &[&str]) -> Option<String> {
    let trace = std::env::var("TRACE").unwrap_or_else(|_| TRACE.into());
    let replay = std::env::var("REPLAY").unwrap_or_else(|_| REPLAY.into());
    diff_of(&trace, &replay, args)
}

/// The oracle's output for `diff trace replay args`, or none when what it needs isn't here.
fn diff_of(trace: &str, replay: &str, args: &[&str]) -> Option<String> {
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(trace).exists()
        || !std::path::Path::new(replay).exists()
    {
        eprintln!("no trace, replay or disc: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["diff", trace, replay])
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
    // Taken up from an import, play runs to the trace's end: a new item's unused hitboxes'
    // joints, which it keeps from the item its block held, import as the live joints they
    // point to (docs/oracle.md).
    assert!(out.contains("11728 ticks compared, no difference"), "{out}");
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
fn start_reaches_the_end() {
    let Some(out) = diff(&[]) else {
        return;
    };
    // From its start the whole match compares to the trace's end with no difference, Sheik
    // turning into Zelda and back and items hitting items among it, and nothing read a stale
    // field.
    assert!(!out.contains(" differs in "), "{out}");
    assert!(!out.contains("isn't ported"), "{out}");
    assert!(out.contains("the trace ends after tick 11928"), "{out}");
    assert!(out.contains("11928 ticks compared, no difference"), "{out}");
    assert!(out.contains("no stale reads"), "{out}");
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

#[test]
fn drive_pace_from_the_boot() {
    let Some(out) = diff_of(
        DRIVE_TRACE,
        DRIVE_REPLAY,
        &["--fp", "hardware", "--disc-speed", "drive", "--boot-audio"],
    ) else {
        return;
    };
    // At a drive's pace melee-sim's boot leaves what ssbm-rs's does, its timeline included,
    // and the match, its music playing on, compares to the trace's end.
    assert!(
        out.contains("the boot's audio setup against the boot state: 0 differences"),
        "{out}"
    );
    assert!(out.contains("the trace ends after tick 4159"), "{out}");
    assert!(out.contains("4159 ticks compared, no difference"), "{out}");
    assert!(out.contains("no stale reads"), "{out}");
}
