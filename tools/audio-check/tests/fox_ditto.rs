//! The Fox ditto's mix, checked against ssbm-rs's recording as docs/player.md records it:
//! sample-exact from the trace's boot state and from melee-sim's own boot; and the benchmark's
//! Fox ditto and Fox against Marth (whose song the flat model cut) at Slippi Dolphin's fast disc
//! speed, against ssbm-rs's recordings at it. Ignored by default, since they need ssbm-rs's
//! recordings and the disc; run them with the frame checks:
//!
//! ```sh
//! SSBM_DISC=... cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! The recordings are `REFS/player/audio-NAME.wav` with their traces `.trc` (`REFS` defaults
//! to ssbm-rs's `local/`, `$SSBM_RS_LOCAL`), and the replays the repository's `data/bench`.
//! These fail without their data: they are a check to run on purpose. `audio-fox-fox-fd` was
//! recorded at ssbm-rs's flat model of the fast disc speed (before its `05d550a`), so its replay
//! is read at it (`--disc-speed fast-flat`); `audio-dolphin-fox-fox-fd` and
//! `audio-dolphin-fox-marth-fd` at Dolphin's (ssbm-rs's `05d550a`, `--fields 4500`, names
//! off), the replay's default (`fast`).

use std::path::PathBuf;
use std::process::Command;

/// Runs the check of recording `audio-NAME` of benchmark match `bench` read at `disc` (from the
/// trace's boot state, or with `port` from melee-sim's boot) over `ticks`, and checks it's
/// sample-exact over at least `seconds`; returns the check's output.
fn check_of(name: &str, bench: &str, disc: &str, ticks: u32, port: bool, seconds: f64) -> String {
    std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let refs = std::env::var_os("REFS")
        .map_or_else(|| melee_local_data::ssbm_rs_local("").into(), PathBuf::from);
    let trace = refs.join(format!("player/audio-{name}.trc"));
    let wav = refs.join(format!("player/audio-{name}.wav"));
    let replay = melee_local_data::bench_replay(bench);
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
        .args(["--fp", "hardware", "--disc-speed", disc])
        .args(["--ticks", &ticks.to_string()]);
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
    text.into_owned()
}

/// The Fox ditto at the flat model, as docs/player.md records it.
fn check(port: bool, seconds: f64) {
    check_of("fox-fox-fd", "fox-fox-fd", "fast-flat", 2500, port, seconds);
}

/// When the music stream last moved to its next box, from the check's report.
fn last_box_move(text: &str) -> u32 {
    text.lines()
        .find_map(|l| {
            l.split("last in tick ")
                .nth(1)?
                .split([' ', ',', ')'])
                .next()?
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("no box move in the report\n{text}"))
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

#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn fox_fox_fd_at_dolphins_speed() {
    check_of(
        "dolphin-fox-fox-fd",
        "fox-fox-fd",
        "fast",
        4200,
        false,
        70.0,
    );
    check_of("dolphin-fox-fox-fd", "fox-fox-fd", "fast", 4200, true, 70.0);
}

/// The match whose song the flat model cut 1.8 s in (the stream's last box move in tick 109):
/// at Dolphin's speed its mix is the recording's and the song plays on.
#[test]
#[ignore = "needs ssbm-rs's recording and the disc; run with --ignored"]
fn fox_marth_fd_keeps_its_song_at_dolphins_speed() {
    for port in [false, true] {
        let text = check_of(
            "dolphin-fox-marth-fd",
            "fox-marth-fd",
            "fast",
            4200,
            port,
            70.0,
        );
        assert!(last_box_move(&text) > 4000, "{text}");
    }
}
