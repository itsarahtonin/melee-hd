//! The sound test's sessions' mix from power-on, checked against ssbm-rs's recordings of their
//! playback as docs/player.md records them: sample-exact, every song of `songs` and every
//! category's sounds of `sounds`. Ignored by default, since it needs ssbm-rs's recordings and
//! the disc; run it with the frame checks:
//!
//! ```sh
//! SSBM_DISC=... cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! The sessions are `REFS/lockstep/sound-sessions/NAME.inputs` and the recordings
//! `REFS/player/audio-sound-NAME.wav` (`REFS` defaults to ssbm-rs's `local/`,
//! `$SSBM_RS_LOCAL`). These fail without their data: they are a check to run on purpose.

use std::path::PathBuf;
use std::process::Command;

/// Runs the session's check and checks it's sample-exact over at least `seconds`.
fn check(name: &str, seconds: f64) {
    std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let refs = std::env::var_os("REFS")
        .map_or_else(|| melee_local_data::ssbm_rs_local("").into(), PathBuf::from);
    let inputs = refs.join(format!("lockstep/sound-sessions/{name}.inputs"));
    let wav = refs.join(format!("player/audio-sound-{name}.wav"));
    for p in [&inputs, &wav] {
        assert!(
            p.exists(),
            "{} is missing (set SSBM_RS_LOCAL or REFS)",
            p.display()
        );
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-audio-check"))
        .arg("--session")
        .arg(&inputs)
        .arg(&wav)
        .output()
        .expect("melee-audio-check runs");
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
fn every_song_is_sample_exact() {
    check("songs", 155.0);
}

#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn every_category_is_sample_exact() {
    check("sounds", 173.0);
}
