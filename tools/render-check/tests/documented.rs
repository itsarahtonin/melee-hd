//! The frames docs/player.md documents, each checked against ssbm-rs's reference as the
//! document records it: `melee-render-check compare` on each, its alignment (the log's frame N
//! is native frame N - 3) and its two images. Ignored by default, since it needs ssbm-rs's
//! references and the disc and takes minutes; run it with
//!
//! ```sh
//! SSBM_DISC=... cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! The references are ssbm-rs's FIFO logs and gxplay's frames of them, `REFS/player/ref-MATCH/fN.dff`
//! and `fN/` (`REFS` defaults to ssbm-rs's `local/`, `$SSBM_RS_LOCAL`), the benchmark replays
//! in the repository's `data/bench/`, and the Slippi replay of Fox vs Sheik `SLIPPI_FOX_SHEIK`
//! (default below, in `$SLIPPI_REPLAYS`; the author's, not public). Unlike the tests that skip
//! without their data, these fail without it: they are a check to run on purpose.

use std::path::PathBuf;
use std::process::Command;

use melee_local_data::{slippi_replays, ssbm_rs_local};

const SLIPPI_FOX_SHEIK: &str = "2026-01/Game_20260129T133652.slp";

fn refs() -> PathBuf {
    std::env::var_os("REFS").map_or_else(|| ssbm_rs_local("").into(), PathBuf::from)
}

/// What a documented frame's two images must be.
enum Expect {
    /// Identical to ssbm-rs's: PSNR infinite.
    Identical,
    /// The images' lines exactly as recorded (a frame with a known difference).
    Images([&'static str; 2]),
}

/// Runs `compare` on the log of `game`'s frame `frame` and checks the result.
fn check(game: &str, replay: PathBuf, slippi: bool, frame: u32, expect: Expect) {
    let disc = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let dir = refs().join("player").join(format!("ref-{game}"));
    let log = dir.join(format!("f{frame}.dff"));
    let images = dir.join(format!("f{frame}"));
    for p in [&replay, &log, &images] {
        assert!(
            p.exists(),
            "{} is missing (set SSBM_RS_LOCAL or REFS, and SLIPPI_REPLAYS)",
            p.display()
        );
    }
    let native = frame - 3;
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_melee-render-check"));
    cmd.arg("compare")
        .arg(&disc)
        .arg(&replay)
        .arg(&log)
        .arg(&images)
        .arg(native.to_string());
    if slippi {
        cmd.env("CHECK_FP", "slippi");
    } else {
        cmd.env_remove("CHECK_FP");
    }
    for v in ["CHECK_SEARCH", "CHECK_DRAW_LIMIT", "CHECK_THREADED"] {
        cmd.env_remove(v);
    }
    let out = cmd.output().expect("melee-render-check runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    let aligned = format!("the log's first frame is native frame {native} (");
    assert!(
        lines.iter().any(|l| l.starts_with(&aligned)),
        "{game} f{frame}: the log doesn't start at native frame {native}\n{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let notes: Vec<&str> = lines
        .iter()
        .filter(|l| l.trim_start().starts_with("not drawn exactly"))
        .copied()
        .collect();
    assert!(
        notes.is_empty(),
        "{game} f{frame}: melee-render left things undrawn: {notes:#?}\n{text}"
    );
    let found: Vec<&str> = lines
        .iter()
        .filter(|l| l.starts_with("  image:") || l.starts_with("  no "))
        .map(|l| l.trim())
        .collect();
    match expect {
        Expect::Identical => {
            assert!(
                found.len() == 2
                    && found
                        .iter()
                        .all(|l| l.starts_with("image: PSNR inf dB,")
                            && l.ends_with("within tolerance")),
                "{game} f{frame}: not identical: {found:#?}\n{text}"
            );
            assert!(out.status.success(), "{game} f{frame}: failed\n{text}");
        }
        Expect::Images(want) => {
            assert_eq!(found, want, "{game} f{frame}\n{text}");
        }
    }
}

fn bench(game: &str, frame: u32) {
    let replay = melee_local_data::bench_replay(game);
    check(game, replay, false, frame, Expect::Identical);
}

/// Fox vs Sheik's frames: identical but for the name plates Slippi's playback draws and
/// melee-sim doesn't (docs/player.md's open issues), as recorded here.
fn fox_sheik(frame: u32, images: [&'static str; 2]) {
    let replay = std::env::var_os("SLIPPI_FOX_SHEIK")
        .map_or_else(|| slippi_replays(SLIPPI_FOX_SHEIK).into(), PathBuf::from);
    check("fox-sheik-fd", replay, true, frame, Expect::Images(images));
}

macro_rules! bench_frames {
    ($game:literal: $($name:ident $frame:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                bench($game, $frame);
            }
        )*
    };
}

bench_frames!("fox-fox-fd":
    fox_fox_fd_140 140,
    fox_fox_fd_300 300,
    fox_fox_fd_600 600,
    fox_fox_fd_1000 1000,
    fox_fox_fd_2600 2600,
    fox_fox_fd_5000 5000,
    fox_fox_fd_8000 8000,
    fox_fox_fd_11000 11000,
    fox_fox_fd_14000 14000,
);
bench_frames!("fox-marth-bf":
    fox_marth_bf_600 600,
    fox_marth_bf_2600 2600,
    fox_marth_bf_6000 6000,
    fox_marth_bf_11000 11000,
);
bench_frames!("fox-marth-fd":
    fox_marth_fd_600 600,
    fox_marth_fd_6000 6000,
);

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_600() {
    fox_sheik(
        600,
        [
            "image: PSNR 31.65 dB, 0.535% off by 8, 0.366% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.22 dB, 0.732% off by 8, 0.557% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_1400() {
    fox_sheik(
        1400,
        [
            "image: PSNR 32.85 dB, 0.816% off by 8, 0.355% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.79 dB, 0.802% off by 8, 0.329% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_fd_6000() {
    fox_sheik(
        6000,
        [
            "image: PSNR 31.49 dB, 0.606% off by 8, 0.223% off by 32: OUT OF TOLERANCE",
            "image: PSNR 31.52 dB, 0.610% off by 8, 0.220% off by 32: OUT OF TOLERANCE",
        ],
    );
}
