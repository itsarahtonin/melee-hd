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
    Expect, FOUR_FD, FOX_FOX_FD, FOX_MARTH_BF, FOX_MARTH_FD, FOX_MARTH_PS, FOX_SHEIK_DL,
    FOX_SHEIK_FD, FOX_SHEIK_YS, Game, Group, SHEIK_FOX_FOD, compare, scaled, why,
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
    compare(
        &SHEIK_FOX_FOD,
        600,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR 40.87 dB, 1.314% off by 8, 0.093% off by 32: within tolerance",
                "image: PSNR 41.01 dB, 1.250% off by 8, 0.090% off by 32: within tolerance",
            ],
            notes: &[
                "not drawn exactly: draw PointSize (12)",
                "not drawn exactly: draw PointSize (12)",
            ],
            why: why::STARS,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_ys_600() {
    compare(
        &FOX_SHEIK_YS,
        600,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR 33.38 dB, 0.268% off by 8, 0.267% off by 32: OUT OF TOLERANCE",
                "image: PSNR 33.76 dB, 0.257% off by 8, 0.256% off by 32: OUT OF TOLERANCE",
            ],
            notes: &[],
            why: why::YS_PLATFORM,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn fox_sheik_dl_600() {
    compare(
        &FOX_SHEIK_DL,
        600,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR 90.53 dB, 0.000% off by 8, 0.000% off by 32: within tolerance",
                "image: PSNR 64.60 dB, 0.005% off by 8, 0.002% off by 32: within tolerance",
            ],
            notes: &[],
            why: why::SHADOWS,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references, the replay and the disc; run with --ignored"]
fn four_fd_600() {
    compare(
        &FOUR_FD,
        600,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR inf dB, 0.000% off by 8, 0.000% off by 32: within tolerance",
                "image: PSNR 59.49 dB, 0.001% off by 8, 0.001% off by 32: within tolerance",
            ],
            notes: &[],
            why: why::PALETTE,
        },
    );
}

#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_600() {
    compare(
        &FOX_MARTH_PS,
        600,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR 15.21 dB, 15.388% off by 8, 12.116% off by 32: OUT OF TOLERANCE",
                "image: PSNR 15.28 dB, 15.705% off by 8, 12.210% off by 32: OUT OF TOLERANCE",
            ],
            notes: &[],
            why: why::STADIUM_FLOOR,
        },
    );
}

/// Pokémon Stadium transformed: the fire stage.
#[test]
#[ignore = "needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_6000_fire() {
    compare(
        &FOX_MARTH_PS,
        6000,
        Group::Standard,
        Expect::Known {
            images: [
                "image: PSNR 22.11 dB, 0.872% off by 8, 0.867% off by 32: OUT OF TOLERANCE",
                "image: PSNR 39.33 dB, 0.663% off by 8, 0.510% off by 32: OUT OF TOLERANCE",
            ],
            notes: &[],
            why: why::BUBBLE,
        },
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
