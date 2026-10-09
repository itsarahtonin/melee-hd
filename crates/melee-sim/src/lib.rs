//! Melee's game simulation.
//!
//! A tick is a pure function of the state and the controllers: no rendering, audio, clocks or
//! disc timing reach it. Float math reproduces the console's bit for bit through `gekko-fp`
//! where the original fused, estimated or used paired singles. State is plain data, so a
//! snapshot for rollback is a clone.

/// The float crate the simulation computes with, melee-hd's copy of ssbm-rs's `gekko-fp`: set
/// its mode (`gekko_fp::set_fp_mode`) to run the simulation in the console's or Slippi's.
pub use gekko_fp;

pub mod audio;
mod fpu;
pub mod game;
pub mod hsd;
pub mod input;
pub mod lb;
pub mod math;
mod memo;
pub mod mp;
pub mod mtx;
mod pairs;
pub mod ppc;
pub mod quat;
pub mod rng;
pub mod vec;
