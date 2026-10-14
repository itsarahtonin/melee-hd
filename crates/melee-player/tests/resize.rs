//! A renderer whose EFB is made again mid-match at another scale, as a window's resize or a
//! fullscreen toggle makes it, draws what a renderer made at that scale from the match's start
//! draws. Plays a benchmark replay headless (`--bench` with `--resize-at`) and compares the last
//! frames pixel for pixel. Skips without `SSBM_DISC`.

use std::path::{Path, PathBuf};
use std::process::Command;

const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/fox-fox-fd.slp"
);

/// The last frame of `ticks` ticks of the replay, drawn with `args`: its size and pixels.
fn shot(name: &str, ticks: u32, args: &[&str]) -> (u32, u32, Vec<u8>) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("resize-{name}.png"));
    let _ = std::fs::remove_file(&path);
    let out = Command::new(env!("CARGO_BIN_EXE_melee-hd"))
        .args(["--replay", REPLAY, "--bench", &ticks.to_string()])
        .args(args)
        .arg("--shot")
        .arg(&path)
        .output()
        .expect("melee-hd runs");
    assert!(
        out.status.success() && path.exists(),
        "{name}: melee-hd failed\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    decode(&path)
}

fn decode(path: &Path) -> (u32, u32, Vec<u8>) {
    let file = std::fs::File::open(path).expect("the shot");
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("its size")];
    let info = reader.next_frame(&mut buf).expect("its pixels");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// Draws `ticks` ticks resized as `resizes` asks, then from the start at the last size, and
/// checks the last frames are the same.
fn same_as_from_the_start(name: &str, ticks: u32, start: &str, resizes: &[&str], extra: &[&str]) {
    if std::env::var_os("SSBM_DISC").is_none() {
        eprintln!("skipped: SSBM_DISC names no disc");
        return;
    }
    let mut args = vec!["--size", start];
    for r in resizes {
        args.extend(["--resize-at", r]);
    }
    args.extend(extra);
    let resized = shot(&format!("{name}-resized"), ticks, &args);
    let last = resizes.last().and_then(|r| r.split_once(':')).unwrap().1;
    let mut args = vec!["--size", last];
    args.extend(extra);
    let fresh = shot(&format!("{name}-fresh"), ticks, &args);
    assert_eq!(
        (resized.0, resized.1),
        (fresh.0, fresh.1),
        "{name}: the frames' sizes differ"
    );
    let off = resized
        .2
        .iter()
        .zip(&fresh.2)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        off == 0,
        "{name}: {off} of {} bytes differ from a renderer made at {last} from the start",
        fresh.2.len()
    );
}

/// 1460x1200 (3x) to 2880x2160 (5x) at tick 300, on the renderer's threads, as the player draws:
/// the frame after the remake.
#[test]
fn a_renderer_made_larger_mid_match_draws_as_one_made_at_the_start() {
    same_as_from_the_start("up", 302, "1460x1200", &["300:2880x2160"], &[]);
}

/// A 4K fullscreen toggle and back (3x, 5x, then 2x), with the renderer on the game's thread.
#[test]
fn a_renderer_made_again_and_smaller_draws_as_one_made_at_the_start() {
    same_as_from_the_start(
        "down",
        330,
        "1460x1200",
        &["250:3840x2160", "320:1280x960"],
        &["--one-thread"],
    );
}

/// With MSAA, whose multisampled EFB is made again beside the EFB: 2x to 5x at tick 250.
#[test]
fn a_multisampled_efb_made_larger_mid_match_draws_as_one_made_at_the_start() {
    same_as_from_the_start(
        "msaa",
        252,
        "1280x960",
        &["250:2880x2160"],
        &["--msaa", "4"],
    );
}
