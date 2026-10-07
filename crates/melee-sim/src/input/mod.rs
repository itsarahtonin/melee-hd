//! Controller input, from what the hardware reports to what the game reads: the SDK's PAD
//! decoding ([`pad`]), HAL's pad library ([`controller`]) and its rumble ([`rumble`]).
//!
//! Per poll, the input layer turns each port's report into a `PadStatus` with [`pad::read`]
//! and queues the four with [`controller::Controllers::renew_raw`], which also runs a frame
//! of rumble and returns the motor changes to send; each frame the game renews its statuses
//! from the queue.

pub mod controller;
pub mod pad;
pub mod rumble;
