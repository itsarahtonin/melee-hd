//! Draws melee-sim's display pass. The simulation hands each frame's [`Draw`] commands over,
//! naming the objects they draw; this crate reads those objects' state as the pass left it and
//! draws them. Nothing here feeds back into the simulation.
//!
//! [`Draw`]: melee_sim::hsd::display::Draw
