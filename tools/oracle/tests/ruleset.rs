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

/// Each session runs under the ruleset from its flag and compares clean to its trace's end:
/// the memory card scene skipped, the menus' getters on Slippi's general codes, and the
/// matches on its gameplay codes (Init Stage Data and Init Player Data zeroing only the
/// blocks their hooks see: `Ground_GetStageGObj`'s and `Fighter_Create`'s, not
/// `Ground_801C1A20`'s on Fountain of Dreams, tick 848, nor the Classic intro's demo fighters',
/// tick 819).
#[test]
fn the_ruleset_sessions_compare_clean() {
    for (name, ticks) in [
        ("vs-menus", 2052),
        ("menu-tour", 3055),
        ("classic-cstick", 3291),
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
