//! Slippi replays as a melee-sim match's input: the `.slp` format ([`Replay`]), what the
//! Slippi EXI device answers Slippi's playback codes from one ([`start`], [`Device`]), a
//! match's own recording written as one ([`write`]) and `.slpz`, its compressed form
//! ([`slpz`]).
//!
//! The player plays replays with this crate at runtime. `leaf-check`'s replay tests check its
//! parsing against ssbm-rs's `ssbm_slippi`.

mod denylist;
pub mod metadata;
pub mod playback;
pub mod replay;
pub mod slpz;
pub mod write;

pub use playback::{Device, start};
pub use replay::Replay;
