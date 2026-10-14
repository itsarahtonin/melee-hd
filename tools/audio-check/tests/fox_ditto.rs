//! The Fox ditto's mix, checked against ssbm-rs's recording as docs/player.md records it:
//! sample-exact from the trace's boot state and from melee-sim's own boot. Ignored by default,
//! since it needs ssbm-rs's recording and the disc; run it with the frame checks:
//!
//! ```sh
//! SSBM_DISC=... cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! The recording is `REFS/player/audio-fox-fox-fd.wav` with its trace `.trc` (`REFS` defaults
//! to ssbm-rs's `local/`, `$SSBM_RS_LOCAL`), and the replay the repository's
//! `data/bench/fox-fox-fd.slp`. These fail without their data: they are a check to run on
//! purpose.

use std::path::PathBuf;
use std::process::Command;

/// Runs the check (from the trace's boot state, or with `port` from melee-sim's boot) and
/// checks it's sample-exact over at least `seconds`.
fn check(port: bool, seconds: f64) {
    std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let refs = std::env::var_os("REFS")
        .map_or_else(|| melee_local_data::ssbm_rs_local("").into(), PathBuf::from);
    let trace = refs.join("player/audio-fox-fox-fd.trc");
    let wav = refs.join("player/audio-fox-fox-fd.wav");
    let replay = melee_local_data::bench_replay("fox-fox-fd");
    for p in [&trace, &wav, &replay] {
        assert!(
            p.exists(),
            "{} is missing (set SSBM_RS_LOCAL or REFS)",
            p.display()
        );
    }
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_melee-audio-check"));
    if port {
        cmd.arg("-").arg("--port");
    } else {
        cmd.arg(&trace);
    }
    cmd.arg(&replay)
        .arg(&wav)
        .args(["--fp", "hardware", "--ticks", "2500"]);
    let out = cmd.output().expect("melee-audio-check runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && text.lines().any(|l| l == "sample-exact"),
        "not sample-exact\n{text}{stderr}"
    );
    let compared: f64 = text
        .lines()
        .find_map(|l| l.strip_prefix("all ")?.split(" s:").next()?.parse().ok())
        .expect("the length compared");
    assert!(
        compared >= seconds,
        "only {compared} s compared, under {seconds} s\n{text}"
    );
}

#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn fox_fox_fd_from_trace() {
    check(false, 39.8);
}

#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn fox_fox_fd_from_boot() {
    check(true, 39.9);
}
