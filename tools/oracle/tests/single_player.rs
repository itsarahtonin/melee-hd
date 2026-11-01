//! `melee-oracle session` on milestone 6's sessions (docs/oracle.md, "Single-player sessions"):
//! one into each single-player mode from the main menu on the unlock-all card, and the debug
//! menu's routes into the same content at a raised debug level (its Versus page with each boss
//! and on each single-player stage, the Otoguro test's adventure routes, the debug cutscene and
//! the debug mode's milestone 6 states). Each compares clean from power-on with `--all --heap`
//! to its first stop, which the test names: a port that moves a stop has to move the test, and
//! the mode's entry in `game::single_player_audit` with it.
//!
//! The sessions are in `$SSBM_RS_LOCAL/lockstep/m6-oracle` (each `NAME.inputs` with its
//! `NAME.card` and `NAME.holds.txt`), traced into `traces/m6-oracle` (the card after each as
//! `NAME-play.card`). The tests skip without them or `SSBM_DISC`.

use std::process::Command;

use melee_local_data::ssbm_rs_local;

const DIR: &str = "m6-oracle";

/// The oracle's output for `session TRACE INPUTS args` on the session `name`, or `None`
/// without its files.
fn session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/{DIR}/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/{DIR}/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["session", &trace, &inputs])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some(text)
}

/// Each session compares clean from power-on, `--all --heap`, to its first stop in tick
/// `tick`, the mode's first tick or its loading: (name, tick, the stop).
fn first_stops(list: &[(&str, u32, &str)]) {
    for (name, tick, stop) in list {
        let Some(out) = session(name, &["--all", "--heap"]) else {
            return;
        };
        let want = format!("{tick} ticks compared, no difference; stopped in tick {tick}: {stop}");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
    }
}

/// Each mode from the main menu's 1P list on the unlock-all card: its entry, its select (or
/// list), a character, and its first match.
#[test]
fn each_mode_runs_to_its_first_stop() {
    first_stops(&[
        ("classic", 623, "game mode 0x3's loading isn't ported"),
        ("adventure2", 653, "game mode 0x4's loading isn't ported"),
        ("allstar", 683, "game mode 0x5's loading isn't ported"),
        ("event", 592, "mnEvent_8024E838 (the Event list) isn't ported"),
        ("target", 683, "game mode 0xf's loading isn't ported"),
        ("homerun", 713, "game mode 0x20's loading isn't ported"),
        ("multiman", 742, "mnHyaku_8024CD64 (the Multi-Man list) isn't ported"),
        ("training", 656, "game mode 0x1c's loading isn't ported"),
    ]);
}

/// The debug menu's Versus page with Master Hand (a computer player) against Link on Final
/// Destination, and the Otoguro test's Go to Kinoko (Fox alone on the Mushroom Kingdom
/// adventure route, stage 59).
#[test]
fn the_debug_routes_run_to_their_first_stops() {
    first_stops(&[
        (
            "mh-fd",
            1441,
            "index out of bounds: the len is 31 but the index is 255",
        ),
        (
            "kinoko",
            1272,
            "not implemented: stage kind 31 isn't ported",
        ),
    ]);
}

/// A press the sessions didn't make is caught where it lands: A added to read 700 of
/// `classic` (the main menu, before the 1P list's A) moves the menu on a list early, and to
/// read 1000 of `kinoko` (the Otoguro page) presses its cursor's entry.
#[test]
fn the_sessions_catch_an_added_press() {
    for (name, read, want) in [
        ("classic", "read=700", "501 ticks compared; tick 500 differs"),
        ("kinoko", "read=1000", "879 ticks compared; tick 878 differs"),
    ] {
        let Some(out) = session(name, &["--all", "--heap", "--mutate", read]) else {
            return;
        };
        assert!(out.contains(want), "{name}: {out}");
    }
}
