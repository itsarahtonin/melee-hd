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
/// `tick`: (name, tick, the stop), the stop empty for a session clean to its trace's end
/// (`tick` its ticks), or "tick N: ..." for one that stops in the tick it compares last.
fn first_stops(list: &[(&str, u32, &str)]) {
    for (name, tick, stop) in list {
        let Some(out) = session(name, &["--all", "--heap"]) else {
            return;
        };
        let want = if stop.is_empty() {
            format!(
                "{tick} ticks compared, no difference
"
            )
        } else if stop.starts_with("tick ") {
            format!("{tick} ticks compared, no difference; stopped in {stop}")
        } else {
            format!("{tick} ticks compared, no difference; stopped in tick {tick}: {stop}")
        };
        assert!(
            out.contains(&want),
            "{name}: wants {want:?}
{out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
    }
}

/// Each mode from the main menu's 1P list on the unlock-all card: its entry, its select (or
/// list), Fox, and its first match: Classic's and Adventure's intros, the matches of All-Star,
/// Target Test, the Home-Run Contest and Training (Fountain of Dreams, on
/// `2026-10-06_17-44-10`'s card), the Event list's first event and Multi-Man Melee's list.
#[test]
fn each_mode_runs_to_its_first_stop() {
    first_stops(&[
        ("classic", 796, "gmClassic_801B3500 isn't ported"),
        ("adventure2", 826, "gm_801B3F40 isn't ported"),
        ("allstar", 856, "gm_801B5624 isn't ported"),
        (
            "event",
            592,
            "mnEvent_8024E838 (the Event list) isn't ported",
        ),
        ("target", 3496, ""),
        ("homerun", 3399, ""),
        (
            "multiman",
            742,
            "mnHyaku_8024CD64 (the Multi-Man list) isn't ported",
        ),
        ("training", 3748, ""),
    ]);
}

/// The debug menu's routes at the debug ROM's level: the Versus page with Master Hand against
/// Link on Final Destination (a computer player, `mh-fd`, and a human, `mh-human`), the
/// Otoguro test's Go to Kinoko and Go to Meikyu (the Mushroom Kingdom and Underground Maze
/// adventure routes), the debug mode's milestone 6 states (Intro Easy, AllStar Enemy, EasyEnd's
/// game over, the staff roll), the debug cutscene (mode 0x19) and the regular ending test (mode
/// 0x1A: the trophy fall and congratulations).
#[test]
fn the_debug_routes_run_to_their_first_stops() {
    first_stops(&[
        ("mh-fd", 7178, ""),
        ("mh-human", 7188, ""),
        ("kinoko", 6493, ""),
        (
            "meikyu",
            1242,
            "not implemented: stage kind 32 isn't ported",
        ),
        ("intro-easy", 1362, "game scene 0x20 isn't ported"),
        ("intro-allstar", 1332, "game scene 0x21 isn't ported"),
        ("gameover", 3157, ""),
        ("staffroll", 1232, "game scene 0x2b isn't ported"),
        ("cutscene", 3282, ""),
        ("ending-test", 1232, "game scene 0xf isn't ported"),
    ]);
}

/// The Versus page's tours: the other bosses on Final Destination (Crazy Hand, Giga Bowser,
/// the Wire Frames, Sandbag), the 26 target stages (33 to 58) and the single-player stages
/// (59 to 77, where the original halts; 78 to 83, where it fails an assert; 84 and 85), each
/// a short match, its no-contest and its results.
#[test]
fn the_versus_page_tours_run_to_their_first_stops() {
    first_stops(&[
        ("bosses", 13377, ""),
        ("targets", 47425, ""),
        (
            "stages-1p",
            3409,
            "tick 3408: unknown leftover bytes of a freed Item under Fighter: its layout isn't ported",
        ),
        (
            "stages-1p-b",
            10941,
            "not implemented: stage kind 38 isn't ported",
        ),
        (
            "stages-1p-c",
            3925,
            "not implemented: stage kind 66 isn't ported",
        ),
    ]);
}

/// A press the sessions didn't make is caught where it lands: A added to read 700 of
/// `classic` (the main menu, before the 1P list's A) moves the menu on a list early, and to
/// read 1000 of `kinoko` (the Otoguro page) presses its cursor's entry.
#[test]
fn the_sessions_catch_an_added_press() {
    for (name, read, want) in [
        (
            "classic",
            "read=700",
            "501 ticks compared; tick 500 differs",
        ),
        (
            "kinoko",
            "read=1000",
            "879 ticks compared; tick 878 differs",
        ),
    ] {
        let Some(out) = session(name, &["--all", "--heap", "--mutate", read]) else {
            return;
        };
        assert!(out.contains(want), "{name}: {out}");
    }
}
