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
/// tick 819). Their matches run Slippi's DynamicsFix (`ftCo_8009DD94+0x2FC`), which Slippi
/// Dolphin runs in every match and ssbm-rs's ruleset run doesn't (docs/slippi-ruleset.md, "What
/// the ruleset makes moot"): the comparison names its differences, from the Classic intro's demo
/// fighters (tick 819) and the attract match (tick 2490), whose bone chains it moves (tick 2667).
#[test]
fn the_ruleset_sessions_compare_clean() {
    for (name, ticks, named) in [
        (
            "menu-tour",
            3055,
            &[
                "the joints' dirty marks: 203481 differences, from tick 2490 to tick 3054",
                "the joints' matrices: 865 differences, from tick 2517 to tick 3031",
                "the bone chains' state: 1101 differences, from tick 2667 to tick 3054",
                "the chains' joints' poses: 23 differences, from tick 2668 to tick 2814",
            ][..],
        ),
        (
            "classic-cstick",
            3291,
            &[
                "the joints' dirty marks: 419650 differences, from tick 961 to tick 3290",
                "the joints' matrices: 16994 differences, from tick 819 to tick 3071",
            ][..],
        ),
    ] {
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
        assert!(
            out.contains("named outside the live matches: Slippi's DynamicsFix"),
            "{name}: {out}"
        );
        for n in named {
            assert!(out.contains(&format!("  {n}\n")), "{name}: {out}");
        }
    }
}

/// DynamicsFix turned off (`--mutate no-dynamics-fix`): the matches set up their fighters'
/// matrices as ssbm-rs's ruleset run does, so the sessions compare clean with nothing named,
/// and DynamicsFix's names hide nothing else.
#[test]
fn without_dynamics_fix_nothing_is_named() {
    for (name, ticks) in [("menu-tour", 3055), ("classic-cstick", 3291)] {
        let Some(out) = session(name, &["--all", "--heap", "--mutate", "no-dynamics-fix"]) else {
            return;
        };
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference\n")),
            "{name}: {out}"
        );
        assert!(!out.contains("named "), "{name}: {out}");
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
/// match, built from its game start as playback builds a replay's, goes on from the console's
/// timeline and compares as a live match to the trace's end (the match unfinished): on Fountain
/// of Dreams too, since the live match no longer runs playback's first-frame settling
/// (`Fighter_procInput+0x3CC`), which Slippi Dolphin's live play doesn't (docs/slippi-ruleset.md,
/// "What still differs from the console's run").
#[test]
fn vs_menus_hands_off_its_match() {
    let Some(out) = session("vs-menus", &["--all", "--heap"]) else {
        return;
    };
    assert!(
        out.contains(
            "2052 ticks compared, no difference, into the live match that starts in tick 848"
        ),
        "{out}"
    );
    assert!(
        out.contains("live match 1: stage 0x2, port 1 character 0xe, port 2 character 0x2"),
        "{out}"
    );
    assert!(out.contains("with no stop: 1 live matches"), "{out}");
}

/// Where each live match's recording, played by ssbm-rs, lies under `$SSBM_RS_LOCAL`: those the
/// settling's removal changed (2026-11-16; their entrances only) were traced again into
/// `m6s-settle`, the rest are `m6s-stall`'s.
fn traced(name: &str) -> &'static str {
    match name {
        "matches-a-3" | "matches-b-1" | "matches-b-3" | "matches-b-4" => "m6s-settle",
        _ => "m6s-stall",
    }
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
/// ssbm-rs's ruleset runner into `$SSBM_RS_LOCAL/lockstep/m6s-match` and traced whole into
/// `traces/m6s-stall`: each compares clean to its first live match and on through it as a live
/// match to its handback (`matches-b`'s on Fountain of Dreams, as `vs-menus`' above), then runs
/// on through every match, handback, Sudden Death, results screen and character select to past
/// its last read with no stop.
#[test]
fn the_live_match_sessions_run_on() {
    for (name, compared, end, matches) in [
        (
            "matches-a",
            "2105 ticks compared, no difference, through the live match that starts in tick 1014 to its handback before tick 2105",
            24187,
            5,
        ),
        (
            "matches-b",
            "2821 ticks compared, no difference, through the live match that starts in tick 1230 to its handback before tick 2821",
            18925,
            5,
        ),
    ] {
        let trace = ssbm_rs_local(&format!("traces/m6s-stall/{name}.trc"));
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
        // Each recording is the one ssbm-rs played.
        for n in 1..=matches {
            let n = format!("{name}-{n}");
            let ours = std::fs::read(dir.join(format!("{n}.slp"))).expect("a recording");
            let played = ssbm_rs_local(&format!("lockstep/{}/traced/{n}.slp", traced(&n)));
            assert!(
                ours == std::fs::read(&played).expect("the recording ssbm-rs played"),
                "{n}: the live match no longer writes the recording ssbm-rs played"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.contains(compared), "{name}: {out}");
        assert!(
            out.contains(&format!(
                "ran on to tick {end} past the recording's reads with no stop: {matches} live matches, {matches} recordings written"
            )),
            "{name}: {out}"
        );
    }
}

/// Each live match's recording, as ssbm-rs plays it with Slippi's playback codes and float mode
/// (`traces/DIR/NAME.trc`, its recording in `lockstep/DIR/traced`, [`traced`]), compares clean
/// with melee-hd's playback, the allocator model included, and ssbm-rs's replay check finds
/// nothing but what a pause leaves (`matches-a-1`'s, below) and what playback's first-frame
/// settling leaves, which the live match, as Slippi Dolphin's live play, doesn't run
/// (docs/slippi-bugs.md, "Playback settles each fighter on the first frame, which live play
/// doesn't"): in the entrance alone, a fighter's height an ulp off on its first frame on the
/// stage (frame -118, -113 or -108 by port, and the next frame's pre-frame update) and the
/// off-screen mark on frames -122 to -119, as in Sarah's Slippi Dolphin replays.
#[test]
fn the_live_matches_play_back_as_ssbm_rs_does() {
    for (name, ticks, diverging) in [
        ("matches-a-1", 941, 312),
        ("matches-a-2", 2732, 0),
        ("matches-a-3", 1030, 5),
        ("matches-a-4", 1829, 0),
        ("matches-a-5", 7428, 0),
        ("matches-b-1", 1531, 6),
        ("matches-b-2", 2732, 0),
        ("matches-b-3", 1530, 2),
        ("matches-b-4", 1531, 4),
        ("matches-b-5", 1539, 0),
    ] {
        let dir = traced(name);
        let trace = ssbm_rs_local(&format!("traces/{dir}/{name}.trc"));
        let slp = ssbm_rs_local(&format!("lockstep/{dir}/traced/{name}.slp"));
        let log = ssbm_rs_local(&format!("traces/{dir}/{name}.check.log"));
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
        let log = std::fs::read_to_string(&log).expect("the replay check's log");
        assert!(
            log.contains(&format!(", {diverging} diverge (0 known)")),
            "{name}: {log}"
        );
        if name == "matches-a-1" {
            continue;
        }
        // The settling's: every divergence in the entrance, of the two kinds.
        for line in log.lines().filter(|l| l.starts_with("  frame ")) {
            let frame: i32 = line["  frame ".len()..]
                .split(':')
                .next()
                .and_then(|f| f.parse().ok())
                .unwrap_or_else(|| panic!("{name}: {line}"));
            let height = (line.contains(": post-frame port ")
                && [-118, -113, -108, -103].contains(&frame))
                || (line.contains(": pre-frame port ")
                    && [-117, -112, -107, -102].contains(&frame));
            let mark = line
                .contains("state flags (+0x29) original [C0, 00, 00, 00], ours [40, 00, 00, 00]")
                && (-122..=-119).contains(&frame);
            assert!(
                (height && line.contains(": position y (+0x")) || mark,
                "{name}: {line}"
            );
        }
    }
    // The pause's: the scene frame counts from it and a mark after it.
    let log = std::fs::read_to_string(ssbm_rs_local("traces/m6s-stall/matches-a-1.check.log"));
    if let Ok(log) = log {
        assert!(
            log.contains("1 x post-frame state flags, first on frame 506")
                && log.contains("311 x frame start scene frame, first on frame 506"),
            "{log}"
        );
    }
}

/// Paused live matches (docs/oracle.md, "The Slippi ruleset's live matches", and
/// docs/slippi-bugs.md, "A paused match's recording"): each session compares through its live
/// match on the console's timeline to its handback, and the match writes the recording ssbm-rs
/// played (`lockstep/m6s-stall/traced/NAME-1.slp`, its replay check in
/// `traces/m6s-stall/NAME-1.check.log`), whose off-screen marks are the original's in ssbm-rs's
/// trace of the session (`melee-oracle live-check`, to the recording's end), and whose replay
/// check differs from the recording only in what a pause leaves (the frame starts' scene frame
/// count and the marks of the frames after a pause); ssbm-rs's playback compares clean with
/// melee-hd's. `matches-a`'s first match is checked the same way on the whole trace of its
/// session (`traces/m6s-stall/matches-a.trc`).
#[test]
fn the_paused_live_matches_record_as_the_console_does() {
    // (session, its comparison, live match's check, replay check's divergences, playback's
    // ticks)
    let cases = [
        (
            "pause-fd",
            "1757 ticks compared, no difference, through the live match that starts in tick 1018 to its handback before tick 1757",
            "frame -118 is tick 1023; 427 frames checked to the recording's end (854 post-frame updates, 81 marked off screen), 0 differ, 0 past the trace or unplaced, 0 a tick from their scene frame count",
            [
                "252 diverge (0 known)",
                "12 x post-frame state flags, first on frame 70",
                "240 x frame start scene frame, first on frame 70",
            ],
            434,
        ),
        (
            "pause-far",
            "2353 ticks compared, no difference, through the live match that starts in tick 1018 to its handback before tick 2353",
            "frame -118 is tick 1023; 882 frames checked to the recording's end (1764 post-frame updates, 199 marked off screen), 0 differ, 0 past the trace or unplaced, 0 a tick from their scene frame count",
            [
                "461 diverge (0 known)",
                "11 x post-frame state flags, first on frame 465",
                "450 x frame start scene frame, first on frame 315",
            ],
            889,
        ),
    ];
    for (name, compared, check, diverging, ticks) in cases {
        let trace = ssbm_rs_local(&format!("traces/m6s-pause/{name}.trc"));
        let inputs = ssbm_rs_local(&format!("lockstep/m6s-pause/{name}.inputs"));
        let played = ssbm_rs_local(&format!("lockstep/m6s-stall/traced/{name}-1.slp"));
        let played_trace = ssbm_rs_local(&format!("traces/m6s-stall/{name}-1.trc"));
        let log = ssbm_rs_local(&format!("traces/m6s-stall/{name}-1.check.log"));
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
        assert!(out.contains(compared), "{name}: {out}");
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
    let trace = ssbm_rs_local("traces/m6s-stall/matches-a.trc");
    let played = ssbm_rs_local("lockstep/m6s-stall/traced/matches-a-1.slp");
    let out = oracle(&["live-check", &trace, &played]);
    assert!(
        out.contains("frame -118 is tick 1019; 934 frames checked to the recording's end (1868 post-frame updates, 111 marked off screen), 0 differ, 0 past the trace or unplaced, 0 a tick from their scene frame count"),
        "{out}"
    );
}

/// A session the player recorded under the Slippi ruleset (docs/player.md, "The Slippi ruleset in
/// the player"): `player-fd`, `melee-hd --power-on --ruleset slippi --bench 3200 --holds` with
/// `pause-fd`'s script at the player's drive pace (`lockstep/m6s-player`, its `.holds.txt`
/// beside it), traced by ssbm-rs's ruleset runner from the session file's flag
/// (`traces/m6s-player`). It compares from power-on through its live match to the handback and
/// runs on with no stop; the live match writes the replay the player saved
/// (`lockstep/m6s-player/traced/player-fd-1.slp`), whose marks are the original's, whose replay
/// check differs only in what its pause leaves, and whose playback by ssbm-rs compares clean
/// with melee-hd's.
#[test]
fn the_players_own_session_compares_clean() {
    let trace = ssbm_rs_local("traces/m6s-player/player-fd.trc");
    let inputs = ssbm_rs_local("lockstep/m6s-player/player-fd.inputs");
    let saved = ssbm_rs_local("lockstep/m6s-player/traced/player-fd-1.slp");
    if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
        eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
        return;
    }
    let dir = std::env::temp_dir().join(format!("melee-hd-player-fd-{}", std::process::id()));
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
        out.contains("disc Drive, DSP Samples") && out.contains("no memory card"),
        "{out}"
    );
    assert!(
        out.contains("1766 ticks compared, no difference, through the live match that starts in tick 1027 to its handback before tick 1766"),
        "{out}"
    );
    assert!(
        out.contains(
            "ran on to tick 3259 past the recording's reads with no stop: 1 live matches, 1 recordings written"
        ),
        "{out}"
    );
    // The oracle's recording is the player's but for the metadata's start time.
    let ours = std::fs::read(dir.join("player-fd-1.slp")).expect("the live match's recording");
    let _ = std::fs::remove_dir_all(&dir);
    let player = std::fs::read(&saved).expect("the player's replay");
    let events = |f: &[u8]| melee_slippi::replay::raw_events(f).unwrap().to_vec();
    assert!(
        events(&ours) == events(&player),
        "the live match no longer records what the player saved"
    );
    let out = oracle(&["live-check", &trace, &saved]);
    assert!(
        out.contains("frame -118 is tick 1032; 427 frames checked to the recording's end (854 post-frame updates, 81 marked off screen), 0 differ, 0 past the trace or unplaced, 0 a tick from their scene frame count"),
        "{out}"
    );
    let log = std::fs::read_to_string(ssbm_rs_local("traces/m6s-player/player-fd-1.check.log"))
        .expect("the replay check's log");
    for d in [
        "252 diverge (0 known)",
        "12 x post-frame state flags, first on frame 70",
        "240 x frame start scene frame, first on frame 70",
    ] {
        assert!(log.contains(d), "{log}");
    }
    let played_trace = ssbm_rs_local("traces/m6s-player/player-fd-1.trc");
    let out = oracle(&["diff", &played_trace, &saved, "--fp", "slippi", "--heap"]);
    assert!(out.contains("434 ticks compared, no difference"), "{out}");
}
