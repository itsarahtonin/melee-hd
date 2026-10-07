//! Melee's game simulation.
//!
//! A tick is a pure function of the state and the controllers: no rendering, audio, clocks or
//! disc timing reach it. Float math reproduces the console's bit for bit through `gekko-fp`
//! where the original fused, estimated or used paired singles. State is plain data, so a
//! snapshot for rollback is a clone.

mod fpu;
pub mod hsd;
pub mod lb;
pub mod math;
pub mod mtx;
mod pairs;
mod ppc;
pub mod rng;
pub mod vec;
