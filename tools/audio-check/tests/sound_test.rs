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

/// Music at 0% (`--music 0`): the songs session's stream voices are found (two a frame while a
/// song plays) and the mix loses the music, keeping the menu's sounds: on 2026-11-27, 56940
/// voice frames scaled and the level went from the recording's -20.9 dB to -41.6 dB, 139 of
/// the 154 seconds exactly silent.
#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn no_music_leaves_the_songs_silent() {
    std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let refs = std::env::var_os("REFS")
        .map_or_else(|| melee_local_data::ssbm_rs_local("").into(), PathBuf::from);
    let out = Command::new(env!("CARGO_BIN_EXE_melee-audio-check"))
        .arg("--session")
        .arg(refs.join("lockstep/sound-sessions/songs.inputs"))
        .arg(refs.join("player/audio-sound-songs.wav"))
        .args(["--music", "0"])
        .output()
        .expect("melee-audio-check runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("music at 0%: "))
        .unwrap_or_else(|| panic!("no music line\n{text}"));
    // "N voice frames scaled; level A dB, the recording's B dB"
    let numbers: Vec<f64> = line
        .split([' ', ',', ';'])
        .filter_map(|w| w.parse().ok())
        .collect();
    let [scaled, ours, theirs] = numbers[..] else {
        panic!("unexpected music line: {line}");
    };
    assert!(scaled > 50_000.0, "{scaled} voice frames scaled\n{text}");
    assert!(
        ours < theirs - 18.0,
        "{ours} dB against {theirs} dB\n{text}"
    );
}
