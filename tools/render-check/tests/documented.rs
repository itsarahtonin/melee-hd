//! The frames docs/player.md documents, the standard set: each checked against ssbm-rs's
//! reference as the document records it (`melee-render-check compare` on each, its alignment
//! and its two images), and a few drawn at higher scales and in widescreen against their own
//! 1x frame (`scaled`). Ignored by default, since it needs ssbm-rs's references and the disc
//! and takes minutes; run it, with the Fox ditto's mix, as the pre-push frames+audio step:
//!
//! ```sh
//! SSBM_DISC=... SSBM_RS_LOCAL=... SLIPPI_REPLAYS=... \
//!     cargo test --release -p melee-render-check -p melee-audio-check -- --ignored
//! ```
//!
//! More frames of the same matches are in the opt-in sweep (`tests/sweep.rs`). See
//! `common` for where the references come from.

mod common;

use common::{
    Capture, Expect, FLAT_ZONE, FOUR_FD, FOX_FOX_FD, FOX_MARTH_BF, FOX_MARTH_FD, FOX_MARTH_PS,
    FOX_SHEIK_DL, FOX_SHEIK_FD, FOX_SHEIK_YS, Game, Group, IDLE_MOVIE, IDLE_TITLE, POKE_FLOATS,
    SHEIK_FOX_FOD, compare, compare_in, scaled,
};

fn identical(game: &Game, frame: u32) {
    compare(game, frame, Group::Standard, Expect::Identical);
}

macro_rules! frames {
    ($game:expr; $($name:ident $frame:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
            fn $name() {
                identical(&$game, $frame);
            }
        )*
    };
}

frames!(FOX_FOX_FD;
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
frames!(FOX_MARTH_BF;
    fox_marth_bf_600 600,
    fox_marth_bf_2600 2600,
    fox_marth_bf_6000 6000,
    fox_marth_bf_11000 11000,
);
frames!(FOX_MARTH_FD;
    fox_marth_fd_600 600,
    fox_marth_fd_6000 6000,
);
// The opening movie (its frames as `lbMthp` decodes them, the title's mark over them at 470,
// the logo's flash at 960) and the title screen, from power-on in a recorded session.
frames!(IDLE_MOVIE;
    movie_300 300,
    movie_470 470,
    movie_960 960,
    movie_3000 3000,
    movie_6000 6000,
);
frames!(IDLE_TITLE;
    title_300 300,
    title_700 700,
);
frames!(FLAT_ZONE;
    flat_zone_600 600,
    flat_zone_2800 2800,
    flat_zone_6000 6000,
    flat_zone_6900 6900,
    flat_zone_9000 9000,
    flat_zone_9300 9300,
);
frames!(POKE_FLOATS;
    poke_floats_900 900,
    poke_floats_6000 6000,
);

/// Slippi's name plates, which its playback draws and melee-sim doesn't (docs/player.md's
/// open issues).
const NAME_PLATES: &str = "Slippi's name plates, which melee-sim doesn't draw";

/// Fox vs Sheik's frames: identical but for the name plates, as recorded here.
fn fox_sheik(frame: u32, images: [&'static str; 2]) {
    compare(
        &FOX_SHEIK_FD,
        frame,
        Group::Standard,
        Expect::Known {
            images,
            notes: &[],
            why: NAME_PLATES,
        },
    );
}

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

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn sheik_fox_fod_600() {
    compare(&SHEIK_FOX_FOD, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_ys_600() {
    compare(&FOX_SHEIK_YS, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_dl_600() {
    compare(&FOX_SHEIK_DL, 600, Group::Standard, Expect::Identical);
}

/// Four players: the stock icons' palettes (indexed entries only), Sheik's needle's
/// highlight, and the background's state cache reset pinned.
#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn four_fd_600() {
    compare(&FOUR_FD, 600, Group::Standard, Expect::Identical);
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_600() {
    compare(&FOX_MARTH_PS, 600, Group::Standard, Expect::Identical);
}

/// Pokémon Stadium transformed: the fire stage, with Fox's magnifier bubble from the first
/// logged frame, whose copy comes from the frame before (so the capture ran ssbm-rs's renderer).
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_6000_fire() {
    compare_in(
        &FOX_MARTH_PS,
        6000,
        Group::Standard,
        Expect::Identical,
        Capture::Rendered,
    );
}

#[test]
#[ignore = "needs the replay and the disc; run with --ignored"]
fn scaled_sheik_fox_fod_600() {
    scaled(&SHEIK_FOX_FOD, 600, Group::Standard);
}

#[test]
#[ignore = "needs the disc; run with --ignored"]
fn scaled_fox_marth_ps_600() {
    scaled(&FOX_MARTH_PS, 600, Group::Standard);
}

#[test]
#[ignore = "needs the disc; run with --ignored"]
fn scaled_fox_marth_bf_600() {
    scaled(&FOX_MARTH_BF, 600, Group::Standard);
}
