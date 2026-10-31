//! Mixes the audio voices melee-sim keeps into samples and plays them. The voices, their
//! parameter blocks and command streams are simulation state; turning them into sound is
//! presentation, and nothing here feeds back into the simulation.
//!
//! [`Mixer`] turns on the simulation's mix log (`melee_sim::audio::mix`) and, from the frames
//! it records and the game's sound files, mixes each audio frame as the console's DSP does
//! (the AX microcode, `melee_ax`, ported from ssbm-rs's): 160 stereo samples at 32 kHz, as the audio interface
//! plays them. [`Output`] plays them on the default device.

pub mod fx;
pub mod mixer;
pub mod output;

pub use fx::BootMemory;
pub use mixer::{FRAME_SAMPLES, Mixer};
pub use output::{Output, OutputOptions, OutputStats, Simulated};
