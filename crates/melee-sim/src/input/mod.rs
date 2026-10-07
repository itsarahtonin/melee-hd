//! Controller input, from what the hardware reports to what the game reads: the SDK's PAD
//! decoding ([`pad`]) and HAL's pad library ([`controller`]).
//!
//! Per poll, the input layer turns each port's report into a `PadStatus` with [`pad::read`]
//! and queues the four with [`controller::Controllers::renew_raw`]; each frame the game
//! renews its statuses from the queue.

pub mod controller;
pub mod pad;
