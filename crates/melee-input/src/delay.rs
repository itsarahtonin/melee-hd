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

    /// The controllers the next tick will apply, which a delay has read already; none without
    /// one, or before the first read.
    pub fn next(&self) -> Option<[PadStatus; 4]> {
        self.queue.front().copied().filter(|_| self.frames > 0)
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

    #[test]
    fn the_next_tick_s_controllers_are_known_with_a_delay() {
        let mut d = Delay::new(0);
        d.apply(pads(1));
        assert_eq!(d.next(), None);
        let mut d = Delay::new(2);
        assert_eq!(d.next(), None);
        for k in 1..6 {
            let next = d.next();
            let applied = d.apply(pads(k));
            if let Some(n) = next {
                assert_eq!(n, applied);
            }
        }
        assert_eq!(d.next(), Some(pads(4)));
    }
}
