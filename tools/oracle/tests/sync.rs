//! `melee-oracle sync-test` on the start of the Fox ditto benchmark match (`data/bench`, in the
//! repository): every tick rolled back 7 ticks and played again compares bit for bit with the
//! straight run, and a rollback whose restored state is changed by one bit fails, naming
//! where the states part. Skips without `SSBM_DISC`.
//!
//! Then the same on the first 1,800 ticks of one replay per ported character and tournament
//! stage (`covers_*`): the other benchmark matches, and ranked replays the oracle traces, in
//! `$SSBM_RS_LOCAL/replays/hf` (docs/oracle.md, "Sync test"), which skip without it. The whole
//! matches run with the diffs before each merge (docs/oracle.md).

use std::process::Command;

use melee_local_data::{bench_replay, ssbm_rs_local};

/// The ticks each run plays: the rollbacks check every tick from the 7th on.
const TICKS: &str = "1800";
const CLEAN: &str = "1794 ticks checked, rolled back 7 each, no difference";

/// The oracle's exit status and output for `sync-test` on `replay` with `args`, or none
/// without the disc or the replay.
fn sync_test_of(replay: &str, fp: &str, args: &[&str]) -> Option<(bool, String)> {
    if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(replay).exists() {
        eprintln!("no SSBM_DISC or no {replay}: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .arg("sync-test")
        .arg(replay)
        .args(["--fp", fp, "--ticks", TICKS])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some((out.status.success(), text))
}

/// The same on the Fox ditto.
fn sync_test(args: &[&str]) -> Option<(bool, String)> {
    let replay = bench_replay("fox-fox-fd");
    sync_test_of(&replay.to_string_lossy(), "hardware", args)
}

#[test]
fn rollbacks_replay_the_straight_run() {
    let Some((ok, out)) = sync_test(&["--rollback", "7"]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(out.contains(CLEAN), "{out}");
}

#[test]
fn a_changed_restore_fails() {
    let Some((ok, out)) = sync_test(&["--rollback", "7", "--perturb", "900"]) else {
        return;
    };
    // The flipped bit shows in the first output that reads it (a drawn view matrix, tick 895)
    // or else in the state after tick 900; either way within the perturbed rollback.
    assert!(!ok, "{out}");
    assert!(
        out.contains("894 ticks checked, rolled back 7 each"),
        "{out}"
    );
    assert!(out.contains("differs from the straight run's at "), "{out}");
}

/// `sync-test --rollback 7` on a replay's first ticks compares clean.
fn covers(replay: &str, fp: &str) {
    let Some((ok, out)) = sync_test_of(replay, fp, &["--rollback", "7"]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(out.contains(CLEAN), "{out}");
}

/// A benchmark match, `--fp hardware`.
fn covers_bench(name: &str) {
    covers(&bench_replay(name).to_string_lossy(), "hardware");
}

/// A ranked replay in `$SSBM_RS_LOCAL/replays/hf`, `--fp slippi`.
fn covers_hf(rel: &str) {
    covers(&ssbm_rs_local(&format!("replays/hf/{rel}")), "slippi");
}

/// Marth, Battlefield.
#[test]
fn covers_fox_marth_bf() {
    covers_bench("fox-marth-bf");
}

/// Pokémon Stadium.
#[test]
fn covers_fox_marth_ps() {
    covers_bench("fox-marth-ps");
}

/// Ganondorf, Captain Falcon.
#[test]
fn covers_ganon_captain_fd() {
    covers_bench("ganon-captain-fd");
}

/// The Ice Climbers, Peach.
#[test]
fn covers_ics_peach_ps() {
    covers_bench("ics-peach-ps");
}

/// Mewtwo, Pikachu, Fountain of Dreams.
#[test]
fn covers_mewtwo_pikachu_fod() {
    covers_hf("mewtwo-mm/slp/master-master-aaf066d135e47d4ebaade34f.slp");
}

/// Mr. Game & Watch, Zelda, Yoshi's Story.
#[test]
fn covers_gnw_zelda_ys() {
    covers_hf("gnw-dd-a6/slp/diamond-diamond-4e9d13cb15c1fd34cefb360b.slp");
}

/// Bowser, Dream Land.
#[test]
fn covers_bowser_captain_dl() {
    covers_hf("bowser-dd-a6/slp/diamond-diamond-63df86130317e9f4d5655cff.slp");
}

/// Donkey Kong, Luigi.
#[test]
fn covers_dk_luigi_ys() {
    covers_hf("dk-dd-a6/slp/diamond-diamond-3d2fd655aa7a8ce5dcf0ad6c.slp");
}

/// Yoshi, Falco.
#[test]
fn covers_yoshi_falco_fd() {
    covers_hf("yoshi-dd-a6/slp/diamond-diamond-1394844d94a8a1101750cf0e.slp");
}

/// Young Link, Sheik.
#[test]
fn covers_ylink_sheik_fd() {
    covers_hf("ylink-dd-a6/slp/diamond-diamond-bd2c97c0b832bc1b309fc7da.slp");
}

/// Link.
#[test]
fn covers_link_marth_fd() {
    covers_hf("link-dd-a6/slp/diamond-diamond-51a2bdce5920a268c02b6d88.slp");
}

/// Roy, Jigglypuff.
#[test]
fn covers_roy_puff_fod() {
    covers_hf("roy-master-master-a6/slp/master-master-9636cc0947ea4c8483f4c1a6.slp");
}

/// Mario.
#[test]
fn covers_mario_falco_fd() {
    covers_hf("mario-master-master-a5/slp/master-master-8b2f8e2060170fac6cec4cec.slp");
}

/// Dr. Mario, Ness.
#[test]
fn covers_doc_ness_ys() {
    covers_hf("doc-diamond-diamond-a6/slp/diamond-diamond-ecdc4d64a42f9673c7d56ce8.slp");
}

/// Pichu.
#[test]
fn covers_pichu_captain_bf() {
    covers_hf("pichu-diamond-diamond-a5/slp/diamond-diamond-328faec79dc7639b5ffeefa6.slp");
}

/// Samus.
#[test]
fn covers_samus_doc_fod() {
    covers_hf("doc-diamond-diamond-a6/slp/diamond-diamond-3937907a366cbdb91dc87ca1.slp");
}
