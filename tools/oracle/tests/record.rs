//! `melee-oracle record` on the Fox against Marth benchmark match on Battlefield (`data/bench`, in
//! the repository): melee-hd's recording of the match compares with the replay through
//! `ssbm_slippi::compare` with no divergence and nothing missing, and its game start differs
//! only in the fields docs/oracle.md ("Recordings") explains; then the recording, recorded
//! again, is the same byte for byte. Skips without `SSBM_DISC`.

use std::path::Path;
use std::process::Command;

use melee_local_data::bench_replay;

/// The oracle's exit status and output for `record replay out --fp hardware`, or none without
/// the disc.
fn record(replay: &Path, out: &Path) -> Option<(bool, String)> {
    if std::env::var("SSBM_DISC").is_err() || !replay.exists() {
        eprintln!("no SSBM_DISC or no {}: skipping", replay.display());
        return None;
    }
    let run = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .arg("record")
        .arg(replay)
        .arg(out)
        .args(["--fp", "hardware"])
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    eprintln!("{text}");
    Some((run.status.success(), text))
}

#[test]
fn a_benchmark_match_records_as_its_replay() {
    let dir = std::env::temp_dir().join(format!("melee-oracle-record-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let (first, second) = (dir.join("first.slp"), dir.join("second.slp"));
    let Some((_, out)) = record(&bench_replay("fox-marth-bf"), &first) else {
        return;
    };
    assert!(
        out.contains("91239 events compared through frame Some(14986), 0 not reached, 0 diverge"),
        "{out}"
    );
    assert!(out.contains("game end sent"), "{out}");
    // The version written (3.19.1 against the recording's 3.19.0) and the UCF toggles (3.19's
    // codes write 1 whatever runs; the match's code list runs no UCF).
    assert!(
        out.contains("game start: version at 0x2: original 00, ours 01"),
        "{out}"
    );
    assert!(out.contains("game start: UCF toggles"), "{out}");
    assert!(
        out.contains("game start: 2 fields differ; code list equal"),
        "{out}"
    );
    let Some((ok, again)) = record(&first, &second) else {
        return;
    };
    assert!(ok, "{again}");
    assert!(
        again.contains("every event equals the replay's, byte for byte"),
        "{again}"
    );
    std::fs::remove_dir_all(&dir).expect("the scratch folder goes");
}
