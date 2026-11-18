//! The fast renderer's comparison harness (`melee-render-check fast`, docs/fast-renderer.md)
//! over the frames tests' matches: the frame of every stage the documented frames check, drawn
//! by a renderer under test and by the exact renderer and compared. The fast renderer as far as
//! it goes (`--renderer fast`, F2's mesh cache) draws every frame within the tolerance; the
//! exact renderer with a change made on purpose stands in for a renderer under test, which
//! proves the harness: unchanged, every frame is identical; each wrong frame (a material's
//! color off, a draw missing, a draw out of order, a joint's matrices off, the depth range's
//! edge one ulp off) fails with its message; float noise (one ulp in the position matrices, a
//! sixteenth of a pixel) passes within the tolerance.
//!
//! Builds only with the `fast` feature, so the pre-push frames step doesn't run it; it needs
//! the disc, ssbm-rs's local data (the oracle matches) and the Slippi replays:
//!
//! ```sh
//! SSBM_DISC=... SSBM_RS_LOCAL=... SLIPPI_REPLAYS=... \
//!     cargo test --release -p melee-render-check --features fast --test fast -- --ignored --test-threads=4
//! ```

mod common;

use std::process::Command;

use common::{
    BIG_BLUE, BRINSTAR, BRINSTAR_DEPTHS, CORNERIA, FLAT_ZONE, FOUR_FD, FOURSIDE, FOX_FOX_FD,
    FOX_MARTH_BF, FOX_MARTH_FD, FOX_MARTH_PS, FOX_SHEIK_DL, FOX_SHEIK_FD, FOX_SHEIK_YS, GREAT_BAY,
    GREEN_GREENS, Game, HYRULE_TEMPLE, ICICLE_MOUNTAIN, ITEMS_POWERUPS_BF, ITEMS_POWERUPS_FD,
    ITEMS_POWERUPS_PS, JUNGLE_JAPES, KONGO_JUNGLE, KONGO_JUNGLE_64, METAL_CLOAK_RC,
    MUSHROOM_KINGDOM, MUSHROOM_KINGDOM_2, MUTE_CITY, ONETT, PC_IZUMI_SIZE, PEACHS_CASTLE,
    POKE_FLOATS, RAINBOW_CRUISE, SHEIK_FOX_FOD, SPECIAL_INVISIBLE_YS, VENOM, YOSHIS_ISLAND,
    YOSHIS_ISLAND_64,
};

/// Runs the harness on the game's log frames `frames` (as the documented frames name them)
/// with `args` (the renderer under test: `--mutate NAME`, or `--renderer fast`); returns whether
/// it passed and what it printed.
fn fast(game: &Game, frames: &[u32], args: &[&str]) -> (bool, String) {
    let replay = game.replay.path();
    assert!(
        replay.exists(),
        "{} is missing (set SSBM_RS_LOCAL and SLIPPI_REPLAYS)",
        replay.display()
    );
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_melee-render-check"));
    if game.replay.slippi() {
        cmd.env("CHECK_FP", "slippi");
    } else {
        cmd.env_remove("CHECK_FP");
    }
    let out = cmd
        .arg("fast")
        .arg(std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc"))
        .arg(&replay)
        .args(args)
        .args(
            frames
                .iter()
                .map(|f| (f - game.replay.log_offset()).to_string()),
        )
        .output()
        .expect("melee-render-check runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

/// Each frame's lines of the harness's output, from its `== frame` line.
fn frames_of(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        if l.starts_with("== frame") {
            out.push(String::new());
        }
        if let Some(f) = out.last_mut()
            && !l.ends_with("frames within tolerance")
        {
            f.push_str(l);
            f.push('\n');
        }
    }
    out
}

/// Unchanged, the renderer under test draws every frame as the reference does, bit for bit.
fn identical(game: &Game, frames: &[u32]) {
    let (ok, text) = fast(game, frames, &["--mutate", "none"]);
    let each = frames_of(&text);
    assert!(ok && each.len() == frames.len(), "{}: {text}", game.name);
    for f in &each {
        assert!(
            f.contains(": identical\n") && f.contains("x480: identical"),
            "{}: not identical:\n{f}",
            game.name
        );
    }
}

/// The fast renderer draws every frame within the tolerance; prints each frame's report.
fn within(game: &Game, frames: &[u32]) {
    let (ok, text) = fast(game, frames, &["--renderer", "fast"]);
    let each = frames_of(&text);
    println!("{text}");
    assert!(ok && each.len() == frames.len(), "{}: {text}", game.name);
    for f in &each {
        assert!(
            f.contains(": PASS\n"),
            "{}: out of tolerance:\n{f}",
            game.name
        );
    }
}

macro_rules! identical {
    ($($name:ident: $game:expr, [$($frame:literal),*];)*) => {
        $(
            #[test]
            #[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
            fn $name() {
                identical(&$game, &[$($frame),*]);
            }
        )*

        /// The fast renderer (`--renderer fast`) on the same frames, each within tolerance.
        mod the_fast_renderer {
            use super::*;

            $(
                #[test]
                #[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
                fn $name() {
                    within(&$game, &[$($frame),*]);
                }
            )*
        }
    };
}

// The documented frames of the frames tests' matches: every stage the player offers.
identical! {
    identical_fox_fox_fd: FOX_FOX_FD, [140, 300, 600, 1000, 2600, 5000, 8000, 11000, 14000];
    identical_fox_marth_bf: FOX_MARTH_BF, [600, 2600, 6000, 11000];
    identical_fox_marth_fd: FOX_MARTH_FD, [600, 6000];
    identical_fox_sheik_fd: FOX_SHEIK_FD, [600, 1400, 6000];
    identical_four_fd: FOUR_FD, [600];
    identical_sheik_fox_fod: SHEIK_FOX_FOD, [600];
    identical_pc_izumi_size: PC_IZUMI_SIZE, [600, 6000];
    identical_fox_sheik_ys: FOX_SHEIK_YS, [600];
    identical_fox_sheik_dl: FOX_SHEIK_DL, [600];
    identical_fox_marth_ps: FOX_MARTH_PS, [600, 6000];
    identical_flat_zone: FLAT_ZONE, [600, 2800, 6000, 6900, 9000, 9300];
    identical_poke_floats: POKE_FLOATS, [900, 6000];
    identical_big_blue: BIG_BLUE, [900, 4500, 11400, 13500, 21600];
    identical_brinstar: BRINSTAR, [900, 2700, 4200];
    identical_brinstar_depths: BRINSTAR_DEPTHS, [900, 3743, 9900, 13500];
    identical_corneria: CORNERIA, [1200, 6300];
    identical_fourside: FOURSIDE, [2700, 6000];
    identical_great_bay: GREAT_BAY, [2100, 6000, 9900];
    identical_green_greens: GREEN_GREENS, [900, 5100, 6900, 13500];
    identical_hyrule_temple: HYRULE_TEMPLE, [900, 3900, 6300];
    identical_icicle_mountain: ICICLE_MOUNTAIN, [600, 3000, 9900, 16200, 26100];
    identical_jungle_japes: JUNGLE_JAPES, [900, 2400];
    identical_kongo_jungle: KONGO_JUNGLE, [900, 4200];
    identical_kongo_jungle_64: KONGO_JUNGLE_64, [600, 3300];
    identical_mushroom_kingdom: MUSHROOM_KINGDOM, [900, 4800];
    identical_mushroom_kingdom_2: MUSHROOM_KINGDOM_2, [900, 8100];
    identical_mute_city: MUTE_CITY, [900, 2100, 2700, 17400];
    identical_onett: ONETT, [150, 2400, 9600];
    identical_peachs_castle: PEACHS_CASTLE, [4200, 7800];
    identical_rainbow_cruise: RAINBOW_CRUISE, [600, 3000, 4200, 5400, 6300];
    identical_venom: VENOM, [777, 2700, 5400];
    identical_yoshis_island: YOSHIS_ISLAND, [1200, 3000];
    identical_yoshis_island_64: YOSHIS_ISLAND_64, [600, 3300];
    identical_items_powerups_ps: ITEMS_POWERUPS_PS, [2100, 7300];
    identical_items_powerups_bf: ITEMS_POWERUPS_BF, [2803, 5300];
    identical_items_powerups_fd: ITEMS_POWERUPS_FD, [7000];
    identical_metal_cloak_rc: METAL_CLOAK_RC, [603, 703, 733, 1003];
    identical_special_invisible_ys: SPECIAL_INVISIBLE_YS, [603];
}

/// The frames the changes made on purpose are tried on: the tournament stages, a four-player
/// match, and stages with their own drawing (Onett's cars and Mr. Game & Watch, Big Blue's
/// race, Great Bay's sea).
const SAMPLE: &[(Game, &[u32])] = &[
    (FOX_FOX_FD, &[600, 11000]),
    (FOX_MARTH_BF, &[6000]),
    (FOX_MARTH_PS, &[6000]),
    (SHEIK_FOX_FOD, &[600]),
    (FOX_SHEIK_YS, &[600]),
    (FOX_SHEIK_DL, &[600]),
    (FOUR_FD, &[600]),
    (ONETT, &[150]),
    (BIG_BLUE, &[900]),
    (GREAT_BAY, &[2100]),
];

/// The frames the material color change is tried on: those of [`SAMPLE`] where the draw it
/// picks shows (Battlefield's 6000, the four-player match's 600 and Big Blue's 900 show it
/// too little or not at all: docs/fast-renderer.md).
const COLOR_SAMPLE: &[(Game, &[u32])] = &[
    (FOX_FOX_FD, &[600, 11000]),
    (FOX_MARTH_PS, &[6000]),
    (SHEIK_FOX_FOD, &[600]),
    (FOX_SHEIK_YS, &[600]),
    (FOX_SHEIK_DL, &[600]),
    (ONETT, &[150]),
    (GREAT_BAY, &[2100]),
];

/// A change that makes a wrong frame fails every frame of `sample`, each with `says` in its
/// report.
fn wrong(sample: &[(Game, &[u32])], mutation: &str, says: &[&str]) {
    for (game, frames) in sample {
        let (ok, text) = fast(game, frames, &["--mutate", mutation]);
        let each = frames_of(&text);
        assert!(
            !ok && each.len() == frames.len(),
            "{} with {mutation}: passed\n{text}",
            game.name
        );
        for f in &each {
            assert!(
                f.contains(": FAIL\n") && says.iter().all(|s| f.contains(s)),
                "{} with {mutation}: not failed as expected ({says:?}):\n{f}",
                game.name
            );
        }
    }
}

/// Float noise passes every frame of the sample, though the frames aren't identical.
fn noise(mutation: &str) {
    for (game, frames) in SAMPLE {
        let (ok, text) = fast(game, frames, &["--mutate", mutation]);
        let each = frames_of(&text);
        assert!(
            ok && each.len() == frames.len(),
            "{} with {mutation}: {text}",
            game.name
        );
        for f in &each {
            assert!(
                f.contains("geometry")
                    && f.contains("within tolerance")
                    && !f.contains(": identical\n"),
                "{} with {mutation}: unchanged or out of tolerance:\n{f}",
                game.name
            );
        }
    }
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn a_color_off_fails() {
    wrong(
        COLOR_SAMPLE,
        "color",
        &["vertices: identical", "OUT OF TOLERANCE"],
    );
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn a_draw_missing_fails() {
    wrong(SAMPLE, "drop", &["  missing: display draw"]);
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn a_draw_out_of_order_fails() {
    wrong(SAMPLE, "order", &["  out of order: display draw"]);
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn a_joint_off_fails() {
    wrong(
        SAMPLE,
        "joint",
        &["  moved: display draw", "vertices out of tolerance"],
    );
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn an_ulp_at_the_depth_range_s_edge_fails() {
    wrong(
        SAMPLE,
        "edge",
        &[
            "on the other side of the depth range's edge",
            "OUT OF TOLERANCE",
        ],
    );
}

/// The frames whose shadow cameras erase their textures with a quad exactly on the near plane
/// (docs/fast-renderer.md, "The depth range's edge"): the tournament stages', where `edge`
/// fails.
const EDGE_SAMPLE: &[(Game, &[u32])] = &[
    (FOX_FOX_FD, &[600, 11000]),
    (FOX_MARTH_BF, &[6000]),
    (FOX_MARTH_PS, &[6000]),
    (SHEIK_FOX_FOD, &[600]),
    (FOX_SHEIK_YS, &[600]),
    (FOX_SHEIK_DL, &[600]),
];

/// Whether the renderer `args` name puts every vertex of the edge sample's frames on the same
/// side of the depth range's edge as the exact renderer, each frame within tolerance; the
/// harness's report of the first that doesn't, if any.
fn keeps_the_edge(args: &[&str]) -> Result<(), String> {
    for (game, frames) in EDGE_SAMPLE {
        let (ok, text) = fast(game, frames, args);
        let each = frames_of(&text);
        if each.len() != frames.len() {
            return Err(format!("{}: {text}", game.name));
        }
        if !ok || each.iter().any(|f| f.contains("depth range's edge")) {
            return Err(format!("{} with {args:?}:\n{text}", game.name));
        }
    }
    Ok(())
}

/// The fast renderer keeps what lies on the depth range's edge exactly there: the shadow
/// cameras' erases stay on the near plane, and nothing crosses it. The check fails a renderer
/// that puts them one ulp outside (the exact renderer with `edge`).
#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn the_fast_renderer_keeps_the_depth_range_s_edge() {
    if let Err(e) = keeps_the_edge(&["--renderer", "fast"]) {
        panic!("the fast renderer crossed the depth range's edge: {e}");
    }
    assert!(
        keeps_the_edge(&["--mutate", "edge"]).is_err(),
        "the check passed a renderer one ulp off the edge"
    );
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn an_ulp_in_the_matrices_passes() {
    noise("ulp");
}

#[test]
#[ignore = "needs the disc, ssbm-rs's local data and the Slippi replays"]
fn a_sixteenth_of_a_pixel_passes() {
    noise("snap");
}
