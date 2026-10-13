//! Slippi replays as a melee-sim match's input: the `.slp` format ([`Replay`]), and what the
//! Slippi EXI device answers Slippi's playback codes from one ([`start`], [`Device`]).
//!
//! The player plays replays with this crate at runtime. `leaf-check`'s replay tests check its
//! parsing against ssbm-rs's `ssbm_slippi`.

mod denylist;
pub mod playback;
pub mod replay;

pub use playback::{Device, start};
pub use replay::Replay;
