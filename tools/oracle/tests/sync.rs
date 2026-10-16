//! `melee-oracle sync-test` on the start of the Fox ditto benchmark match (`data/bench`, in the
//! repository): every tick rolled back 7 ticks and played again compares bit for bit with the
//! straight run, and a rollback whose restored state is changed by one bit fails, naming
//! where the states part. Skips without `SSBM_DISC`.

use std::process::Command;

use melee_local_data::bench_replay;

/// The oracle's exit status and output for `sync-test` on the Fox ditto with `args`, or none
/// without the disc.
fn sync_test(args: &[&str]) -> Option<(bool, String)> {
    if std::env::var("SSBM_DISC").is_err() {
        eprintln!("no SSBM_DISC: skipping");
        return None;
    }
    let replay = bench_replay("fox-fox-fd");
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .arg("sync-test")
        .arg(&replay)
        .args(["--fp", "hardware", "--ticks", "1800"])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some((out.status.success(), text))
}

#[test]
fn rollbacks_replay_the_straight_run() {
    let Some((ok, out)) = sync_test(&["--rollback", "7"]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(
        out.contains("1794 ticks checked, rolled back 7 each, no difference"),
        "{out}"
    );
}

#[test]
fn a_changed_restore_fails() {
    let Some((ok, out)) = sync_test(&["--rollback", "7", "--perturb", "900"]) else {
        return;
    };
    // The flipped bit shows in the first output that reads it (a drawn view matrix, tick 895)
    // or else in the state after tick 900; either way within the perturbed rollback.
    assert!(!ok, "{out}");
    assert!(out.contains("894 ticks checked, rolled back 7 each"), "{out}");
    assert!(out.contains("differs from the straight run's at "), "{out}");
}
