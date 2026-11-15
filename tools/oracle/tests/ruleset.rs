//! `melee-oracle session` on the Slippi ruleset's sessions (docs/oracle.md, "Sessions under
//! the Slippi ruleset"): recorded with ssbm-rs's `--ruleset slippi` from power-on with no card
//! (`SSBMINP1`'s flag 0x04), every field scripted, each compared from power-on with
//! `--all --heap`. `vs-menus`: the menus to a VS match on the codes' unlocks, two players and
//! C-stick smashes in it; `menu-tour`: the main menu's lists, back to the title and its attract
//! match under the ruleset's rules; `classic-cstick`: Classic's select, intro and first match,
//! C-stick smashes cardinal and diagonal and the C-stick held up.
//!
//! The sessions are in `$SSBM_RS_LOCAL/lockstep/m6s-ruleset` (each `NAME.inputs` with its
//! `NAME.holds.txt`), traced into `traces/m6s-ruleset`. The tests skip without them or
//! `SSBM_DISC`.

use std::process::Command;

use melee_local_data::ssbm_rs_local;

/// The oracle's output for `session TRACE INPUTS args` on the session `name`, or `None`
/// without its files.
fn session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/m6s-ruleset/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/m6s-ruleset/{name}.inputs"));
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

/// Each session runs under the ruleset from its flag and compares clean to its trace's end
/// (`vs-menus`, whose match is a live one, to the match's start: below):
/// the memory card scene skipped, the menus' getters on Slippi's general codes, and the
/// matches on its gameplay codes (Init Stage Data and Init Player Data zeroing only the
/// blocks their hooks see: `Ground_GetStageGObj`'s and `Fighter_Create`'s, not
/// `Ground_801C1A20`'s on Fountain of Dreams, tick 848, nor the Classic intro's demo fighters',
/// tick 819).
#[test]
fn the_ruleset_sessions_compare_clean() {
    for (name, ticks) in [("menu-tour", 3055), ("classic-cstick", 3291)] {
        let Some(out) = session(name, &["--all", "--heap"]) else {
            return;
        };
        assert!(
            out.contains("the Slippi ruleset, in Slippi's floating point"),
            "{name}: {out}"
        );
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference\n")),
            "{name}: {out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
    }
}

/// The C-stick rule turned off (`--mutate c-stick-1p`): Classic's camera zooms on the C-stick
/// held up where the trace's doesn't (tick 1229, the zoom's hold count), so the rule's zoom site
/// is what ssbm-rs's code runs. (Its fighter sites: a diagonal C-stick in tick 1279 is a down
/// smash in the trace, which UCF's cardinals can't make; a scratch build with them answering
/// as Vanilla's differed there, 2026-11-14.)
#[test]
fn the_c_stick_rule_off_zooms() {
    let Some(out) = session(
        "classic-cstick",
        &["--all", "--mutate", "c-stick-1p", "--ignore", "c_stick_1p"],
    ) else {
        return;
    };
    assert!(
        out.contains("1230 ticks compared; tick 1229 differs") && out.contains("camera.x2BA"),
        "{out}"
    );
}

/// `vs-menus`' VS match is a live match (`game::live`, task 4 of the plan): it compares clean
/// from power-on to the match's start (tick 848), where the menus' session goes aside and the
/// match, built from its game start as playback builds a replay's, plays on with the
/// recording's reads, uncompared: ssbm-rs's run has no handoff.
#[test]
fn vs_menus_hands_off_its_match() {
    let Some(out) = session("vs-menus", &["--all", "--heap"]) else {
        return;
    };
    assert!(
        out.contains(
            "848 ticks compared, no difference, to the live match that starts in tick 848"
        ),
        "{out}"
    );
    assert!(
        out.contains("live match 1: stage 0x2, port 1 character 0xe, port 2 character 0x2"),
        "{out}"
    );
    assert!(out.contains("with no stop: 1 live matches"), "{out}");
}

/// The output of `melee-oracle ARGS`.
fn oracle(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    text
}

/// Task 4's sessions (docs/oracle.md, "The Slippi ruleset's live matches"), recorded with
/// ssbm-rs's ruleset runner into `$SSBM_RS_LOCAL/lockstep/m6s-match`, their menus traced to
/// past the first match's start into `traces/m6s-match`: each compares clean to its first live
/// match, then runs on through every match, handback, Sudden Death, results screen and
/// character select to past its last read with no stop.
#[test]
fn the_live_match_sessions_run_on() {
    for (name, start, end, matches) in
        [("matches-a", 1014, 24194, 5), ("matches-b", 1230, 18933, 5)]
    {
        let trace = ssbm_rs_local(&format!("traces/m6s-match/{name}.trc"));
        let inputs = ssbm_rs_local(&format!("lockstep/m6s-match/{name}.inputs"));
        if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
            eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("melee-hd-live-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let out = oracle(&[
            "session",
            &trace,
            &inputs,
            "--all",
            "--heap",
            "--slp",
            dir.to_str().unwrap(),
        ]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            out.contains(&format!(
                "{start} ticks compared, no difference, to the live match that starts in tick {start}"
            )),
            "{name}: {out}"
        );
        assert!(
            out.contains(&format!(
                "ran on to tick {end} past the recording's reads with no stop: {matches} live matches, {matches} recordings written"
            )),
            "{name}: {out}"
        );
    }
}

/// Each live match's recording, as ssbm-rs plays it with Slippi's playback codes and float mode
/// (`traces/m6s-match/NAME.trc`, its recording in `lockstep/m6s-match/traced`), compares clean
/// with melee-hd's playback, the allocator model included.
#[test]
fn the_live_matches_play_back_as_ssbm_rs_does() {
    for (name, ticks) in [
        ("matches-a-1", 942),
        ("matches-a-2", 2734),
        ("matches-a-3", 1032),
        ("matches-a-4", 1832),
        ("matches-a-5", 7428),
        ("matches-b-1", 1532),
        ("matches-b-2", 2734),
        ("matches-b-3", 1531),
        ("matches-b-4", 1533),
        ("matches-b-5", 1541),
    ] {
        let trace = ssbm_rs_local(&format!("traces/m6s-match/{name}.trc"));
        let slp = ssbm_rs_local(&format!("lockstep/m6s-match/traced/{name}.slp"));
        if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
            eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
            return;
        }
        let out = oracle(&["diff", &trace, &slp, "--fp", "slippi", "--heap"]);
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
    }
}

/// Paused live matches (docs/oracle.md, "The Slippi ruleset's live matches", and
/// docs/slippi-bugs.md, "A paused match's recording"): each session's live match writes the
/// recording ssbm-rs played (`lockstep/m6s-pause/traced/NAME-1.slp`, its replay check in
/// `traces/m6s-pause/NAME-1.check.log`), whose off-screen marks are the original's in ssbm-rs's
/// trace of the session (`melee-oracle live-check`, to where the two runs' items part), and
/// whose replay check differs from the recording only in what a pause leaves (the frame starts'
/// scene frame count and the marks of the frames after a pause); ssbm-rs's playback compares
/// clean with melee-hd's. `matches-a`'s first match is checked the same way on a longer trace of
/// its session (`traces/m6s-pause/matches-a-long.trc`, to past its pause).
#[test]
fn the_paused_live_matches_record_as_the_console_does() {
    // (session, live match's check, replay check's divergences, playback's ticks)
    let cases = [
        (
            "pause-fd",
            "frame -118 is tick 1023; 429 frames checked to the recording's end (858 post-frame updates, 81 marked off screen), 0 differ",
            [
                "252 diverge (0 known)",
                "12 x post-frame state flags, first on frame 72",
                "240 x frame start scene frame, first on frame 72",
            ],
            436,
        ),
        (
            "pause-far",
            "frame -118 is tick 1023; 595 frames checked to frame 478 (tick 1852), where the items part (1190 post-frame updates, 195 marked off screen), 0 differ",
            [
                "463 diverge (0 known)",
                "13 x post-frame state flags, first on frame 467",
                "450 x frame start scene frame, first on frame 317",
            ],
            891,
        ),
    ];
    for (name, check, diverging, ticks) in cases {
        let trace = ssbm_rs_local(&format!("traces/m6s-pause/{name}.trc"));
        let inputs = ssbm_rs_local(&format!("lockstep/m6s-pause/{name}.inputs"));
        let played = ssbm_rs_local(&format!("lockstep/m6s-pause/traced/{name}-1.slp"));
        let played_trace = ssbm_rs_local(&format!("traces/m6s-pause/{name}-1.trc"));
        let log = ssbm_rs_local(&format!("traces/m6s-pause/{name}-1.check.log"));
        if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
            eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("melee-hd-pause-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let out = oracle(&[
            "session",
            &trace,
            &inputs,
            "--all",
            "--heap",
            "--slp",
            dir.to_str().unwrap(),
        ]);
        assert!(
            out.contains(
                "1018 ticks compared, no difference, to the live match that starts in tick 1018"
            ),
            "{name}: {out}"
        );
        let slp = dir.join(format!("{name}-1.slp"));
        let ours = std::fs::read(&slp).expect("the live match's recording");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            ours == std::fs::read(&played).expect("the recording ssbm-rs played"),
            "{name}: the live match no longer writes the recording ssbm-rs played"
        );
        let out = oracle(&["live-check", &trace, &played]);
        assert!(out.contains(check), "{name}: {out}");
        let log = std::fs::read_to_string(&log).expect("the replay check's log");
        for d in diverging {
            assert!(log.contains(d), "{name}: {log}");
        }
        // Every divergence is a scene frame count or a mark: the post-frame updates' only
        // differing byte is the flags' `+0x29` (the payload's offset), set in the recording.
        for line in log.lines().filter(|l| l.contains(": post-frame port")) {
            assert!(
                line.contains(
                    "state flags (+0x29) original [80, 00, 00, 00], ours [00, 00, 00, 00]"
                ),
                "{name}: {line}"
            );
        }
        let out = oracle(&["diff", &played_trace, &played, "--fp", "slippi", "--heap"]);
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
    }
    let trace = ssbm_rs_local("traces/m6s-pause/matches-a-long.trc");
    let played = ssbm_rs_local("lockstep/m6s-match/traced/matches-a-1.slp");
    let out = oracle(&["live-check", &trace, &played]);
    assert!(
        out.contains("frame -118 is tick 1019; 595 frames checked to frame 478 (tick 1614), where the items part (1190 post-frame updates, 100 marked off screen), 0 differ"),
        "{out}"
    );
}
