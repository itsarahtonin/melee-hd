//! The opt-in sweep: more frames of each stage's match against ssbm-rs's, and more frames
//! drawn at higher scales and in widescreen, beyond the standard set (`documented.rs`), which
//! the pre-push step runs. It takes a few minutes, so it builds only with the `sweep` feature:
//!
//! ```sh
//! SSBM_DISC=... SSBM_RS_LOCAL=... SLIPPI_REPLAYS=... \
//!     cargo test --release -p melee-render-check --features sweep --test sweep -- --ignored
//! ```
//!
//! See `common` for where the references come from.

mod common;

use common::{
    Capture, Expect, FOUR_FD, FOX_FOX_FD, FOX_MARTH_FD, FOX_MARTH_PS, FOX_SHEIK_DL, FOX_SHEIK_YS,
    Game, Group, SHEIK_FOX_FOD, compare, compare_in, scaled, scaled_known, why,
};

/// A frame with a known difference, as recorded.
fn known(game: &Game, frame: u32, images: [&'static str; 2], why: &'static str) {
    compare(
        game,
        frame,
        Group::Sweep,
        Expect::Known {
            images,
            notes: &[],
            why,
        },
    );
}

/// A Fountain of Dreams frame, whose reflection draws some particles elsewhere.
fn fod(frame: u32, images: [&'static str; 2]) {
    known(&SHEIK_FOX_FOD, frame, images, why::REFLECTION_PARTICLES);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_2600() {
    fod(
        2600,
        [
            "image: PSNR 59.43 dB, 0.027% off by 8, 0.001% off by 32: within tolerance",
            "image: PSNR 65.13 dB, 0.011% off by 8, 0.000% off by 32: within tolerance",
        ],
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_6000() {
    fod(
        6000,
        [
            "image: PSNR 40.63 dB, 1.603% off by 8, 0.248% off by 32: within tolerance",
            "image: PSNR 55.10 dB, 0.033% off by 8, 0.009% off by 32: within tolerance",
        ],
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_9000() {
    fod(
        9000,
        [
            "image: PSNR 50.01 dB, 0.047% off by 8, 0.022% off by 32: within tolerance",
            "image: PSNR 49.31 dB, 0.089% off by 8, 0.024% off by 32: within tolerance",
        ],
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_ys_3000() {
    compare(&FOX_SHEIK_YS, 3000, Group::Sweep, Expect::Identical);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_ys_6000() {
    compare(&FOX_SHEIK_YS, 6000, Group::Sweep, Expect::Identical);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_dl_6000() {
    known(
        &FOX_SHEIK_DL,
        6000,
        [
            "image: PSNR 94.99 dB, 0.000% off by 8, 0.000% off by 32: within tolerance",
            "image: PSNR inf dB, 0.000% off by 8, 0.000% off by 32: within tolerance",
        ],
        why::VERTEX_COLORS,
    );
}

/// A magnifier bubble from the first logged frame, so the capture ran ssbm-rs's renderer.
#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_dl_12000() {
    compare_in(
        &FOX_SHEIK_DL,
        12000,
        Group::Sweep,
        Expect::Identical,
        Capture::Rendered,
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_2600() {
    compare(&FOX_MARTH_PS, 2600, Group::Sweep, Expect::Identical);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_9000() {
    known(
        &FOX_MARTH_PS,
        9000,
        [
            "image: PSNR 25.16 dB, 8.878% off by 8, 4.244% off by 32: OUT OF TOLERANCE",
            "image: PSNR 39.80 dB, 0.852% off by 8, 0.306% off by 32: OUT OF TOLERANCE",
        ],
        why::SCREEN,
    );
}

/// Pokémon Stadium transformed: the water stage.
#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_11000_water() {
    compare(&FOX_MARTH_PS, 11000, Group::Sweep, Expect::Identical);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_13000() {
    compare(&FOX_MARTH_PS, 13000, Group::Sweep, Expect::Identical);
}

fn scaled_sweep(game: &Game, frame: u32) {
    scaled(game, frame, Group::Sweep);
}

macro_rules! scaled_frames {
    ($game:expr; $($name:ident $frame:literal),* $(,)?) => {
        $(
            #[test]
            #[ignore = "the sweep: needs the replay and the disc; run with --ignored"]
            fn $name() {
                scaled_sweep(&$game, $frame);
            }
        )*
    };
}

scaled_frames!(FOX_FOX_FD;
    scaled_fox_fox_fd_600 600,
    scaled_fox_fox_fd_11000 11000,
);
scaled_frames!(FOX_MARTH_FD;
    scaled_fox_marth_fd_6000 6000,
);
scaled_frames!(FOX_SHEIK_YS;
    scaled_fox_sheik_ys_600 600,
);
scaled_frames!(FOX_SHEIK_DL;
    scaled_fox_sheik_dl_600 600,
);
scaled_frames!(FOX_MARTH_PS;
    scaled_fox_marth_ps_6000_fire 6000,
    scaled_fox_marth_ps_11000_water 11000,
);
scaled_frames!(FOUR_FD;
    scaled_four_fd_600 600,
);

#[test]
#[ignore = "the sweep: needs the replay and the disc; run with --ignored"]
fn scaled_sheik_fox_fod_6000() {
    scaled_known(
        &SHEIK_FOX_FOD,
        6000,
        Group::Sweep,
        [
            "scaled 1x GPU XFB: PSNR inf dB, 0.000% off by 8, 0.000% off by 32, worst block 0.00 levels at (0, 0): within tolerance",
            "scaled 2x: PSNR 30.05 dB, 17.002% off by 8, 2.401% off by 32, worst block 49.23 levels at (48, 448): OUT OF TOLERANCE",
            "scaled 3x: PSNR 30.69 dB, 17.230% off by 8, 2.062% off by 32, worst block 32.35 levels at (0, 448): OUT OF TOLERANCE",
            "scaled 3x 16:9: PSNR 37.06 dB, 3.297% off by 8, 0.260% off by 32, worst block 13.93 levels at (288, 416): within tolerance",
        ],
        why::REFLECTION_SCALE,
    );
}
