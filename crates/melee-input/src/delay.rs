//! Deliberate added input latency ("backfill"): the player reads the controllers as late as it
//! can, and a player who wants a CRT's or a console's feel back can ask for each read to reach
//! the simulation a set number of ticks later. Off (0) by default.

use std::collections::VecDeque;

use melee_sim::input::pad::PadStatus;

/// Hands each tick the controllers read `frames` ticks before.
#[derive(Clone, Debug, Default)]
pub struct Delay {
    frames: u32,
    queue: VecDeque<[PadStatus; 4]>,
}

impl Delay {
    pub fn new(frames: u32) -> Delay {
        Delay {
            frames,
            queue: VecDeque::with_capacity(frames as usize + 1),
        }
    }

    /// The ticks each read is held back.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// The controllers to apply this tick, given this tick's read: the read `frames` ticks
    /// ago, or the first read until there have been that many.
    pub fn apply(&mut self, pads: [PadStatus; 4]) -> [PadStatus; 4] {
        if self.frames == 0 {
            return pads;
        }
        if self.queue.is_empty() {
            self.queue.extend((0..self.frames).map(|_| pads));
        }
        self.queue.push_back(pads);
        self.queue.pop_front().unwrap_or(pads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pads(button: u16) -> [PadStatus; 4] {
        [PadStatus {
            button,
            ..PadStatus::default()
        }; 4]
    }

    #[test]
    fn reads_reach_the_tick_frames_later() {
        let mut d = Delay::new(0);
        assert_eq!(d.apply(pads(1)), pads(1));
        let mut d = Delay::new(2);
        // The first read stands in for the ticks before it.
        assert_eq!(d.apply(pads(1)), pads(1));
        assert_eq!(d.apply(pads(2)), pads(1));
        assert_eq!(d.apply(pads(3)), pads(1));
        assert_eq!(d.apply(pads(4)), pads(2));
        assert_eq!(d.apply(pads(5)), pads(3));
        assert_eq!(d.frames(), 2);
    }
}
