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
    Expect, FOUR_FD, FOX_FOX_FD, FOX_MARTH_FD, FOX_MARTH_PS, FOX_SHEIK_DL, FOX_SHEIK_YS, Game,
    Group, SHEIK_FOX_FOD, compare, scaled, scaled_known, why,
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

/// A Fountain of Dreams frame, whose stars melee-render notes it doesn't draw exactly.
fn fod(frame: u32, images: [&'static str; 2]) {
    compare(
        &SHEIK_FOX_FOD,
        frame,
        Group::Sweep,
        Expect::Known {
            images,
            notes: &[
                "not drawn exactly: draw PointSize (12)",
                "not drawn exactly: draw PointSize (12)",
            ],
            why: why::WATER,
        },
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_2600() {
    fod(
        2600,
        [
            "image: PSNR 32.22 dB, 22.338% off by 8, 0.029% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.30 dB, 22.247% off by 8, 0.035% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_6000() {
    fod(
        6000,
        [
            "image: PSNR 33.28 dB, 13.622% off by 8, 0.212% off by 32: OUT OF TOLERANCE",
            "image: PSNR 33.81 dB, 13.430% off by 8, 0.164% off by 32: OUT OF TOLERANCE",
        ],
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_9000() {
    fod(
        9000,
        [
            "image: PSNR 31.95 dB, 27.059% off by 8, 0.042% off by 32: OUT OF TOLERANCE",
            "image: PSNR 32.06 dB, 26.828% off by 8, 0.044% off by 32: OUT OF TOLERANCE",
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
    known(
        &FOX_SHEIK_YS,
        6000,
        [
            "image: PSNR 32.03 dB, 2.015% off by 8, 0.520% off by 32: OUT OF TOLERANCE",
            "image: PSNR 36.51 dB, 1.761% off by 8, 0.352% off by 32: OUT OF TOLERANCE",
        ],
        why::YS_PLATFORM,
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_dl_6000() {
    known(
        &FOX_SHEIK_DL,
        6000,
        [
            "image: PSNR 38.62 dB, 1.368% off by 8, 0.513% off by 32: OUT OF TOLERANCE",
            "image: PSNR 38.84 dB, 1.348% off by 8, 0.496% off by 32: OUT OF TOLERANCE",
        ],
        why::SHADOWS,
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_sheik_dl_12000() {
    known(
        &FOX_SHEIK_DL,
        12000,
        [
            "image: PSNR 29.58 dB, 0.847% off by 8, 0.793% off by 32: OUT OF TOLERANCE",
            "image: PSNR inf dB, 0.000% off by 8, 0.000% off by 32: within tolerance",
        ],
        why::BUBBLE,
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_2600() {
    known(
        &FOX_MARTH_PS,
        2600,
        [
            "image: PSNR 32.97 dB, 0.176% off by 8, 0.156% off by 32: OUT OF TOLERANCE",
            "image: PSNR 33.28 dB, 0.165% off by 8, 0.142% off by 32: OUT OF TOLERANCE",
        ],
        why::SHADOWS,
    );
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_9000() {
    known(
        &FOX_MARTH_PS,
        9000,
        [
            "image: PSNR 17.89 dB, 13.987% off by 8, 8.697% off by 32: OUT OF TOLERANCE",
            "image: PSNR 18.99 dB, 6.631% off by 8, 5.247% off by 32: OUT OF TOLERANCE",
        ],
        why::SCREEN,
    );
}

/// Pokémon Stadium transformed: the water stage.
#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_11000_water() {
    known(
        &FOX_MARTH_PS,
        11000,
        [
            "image: PSNR 35.58 dB, 0.852% off by 8, 0.600% off by 32: OUT OF TOLERANCE",
            "image: PSNR 35.78 dB, 0.874% off by 8, 0.613% off by 32: OUT OF TOLERANCE",
        ],
        why::SHADOWS,
    );
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
            "scaled 2x: PSNR 30.65 dB, 16.307% off by 8, 2.160% off by 32, worst block 40.29 levels at (48, 448): OUT OF TOLERANCE",
            "scaled 3x: PSNR 31.16 dB, 16.535% off by 8, 1.824% off by 32, worst block 28.57 levels at (128, 464): within tolerance",
            "scaled 3x 16:9: PSNR 39.39 dB, 2.709% off by 8, 0.098% off by 32, worst block 12.14 levels at (80, 464): within tolerance",
        ],
        why::REFLECTION_SCALE,
    );
}
