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

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_2600() {
    compare(&SHEIK_FOX_FOD, 2600, Group::Sweep, Expect::Identical);
}

/// The reflection's particles with app-SRTs, drawn in the reflection camera's view.
#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_6000() {
    compare(&SHEIK_FOX_FOD, 6000, Group::Sweep, Expect::Identical);
}

#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn sheik_fox_fod_9000() {
    compare(&SHEIK_FOX_FOD, 9000, Group::Sweep, Expect::Identical);
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
    compare(&FOX_SHEIK_DL, 6000, Group::Sweep, Expect::Identical);
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

/// Pokémon Stadium's screen shows a feed copied before the first logged frame, so the
/// capture ran ssbm-rs's renderer; the feed shows the screen showing the feed, so the frames
/// drawn before it matter (`native::WARM_UP`). The images are identical; the two draws of
/// the screen's feed differ only in its texture's hash, a copy kept on the GPU.
#[test]
#[ignore = "the sweep: needs ssbm-rs's references and the disc; run with --ignored"]
fn fox_marth_ps_9000() {
    compare_in(
        &FOX_MARTH_PS,
        9000,
        Group::Sweep,
        Expect::SameImages {
            same: [2025, 2025],
            why: why::SCREEN_COPY,
        },
        Capture::Rendered,
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
            "scaled 2x: PSNR 28.18 dB, 18.190% off by 8, 3.359% off by 32, worst block 48.52 levels at (48, 448): OUT OF TOLERANCE",
            "scaled 3x: PSNR 28.21 dB, 18.383% off by 8, 3.390% off by 32, worst block 31.39 levels at (128, 464): within tolerance",
            "scaled 3x 16:9: PSNR 34.64 dB, 3.220% off by 8, 0.676% off by 32, worst block 16.77 levels at (16, 448): within tolerance",
        ],
        why::REFLECTION_SCALE,
    );
}
