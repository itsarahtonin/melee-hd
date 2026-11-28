// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's PhobGCC module (see lib.rs for PhobGCC-SW's authors), distributed under the
// GNU General Public License version 3 only.

//! The recent samples the diagnostic views draw (PhobVision's stick map, snapback graph and
//! trigger view).

use std::collections::VecDeque;

/// One report's processing, as the diagnostic views show it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// The time step it ran with, in milliseconds.
    pub step_ms: f32,
    /// The control stick and the C-stick as sent, from the origin (x, y).
    pub a: [i32; 2],
    pub c: [i32; 2],
    /// The control stick and the C-stick remapped without the filters (the snapback graph's
    /// other line), in the same units.
    pub a_unfiltered: [f32; 2],
    pub c_unfiltered: [f32; 2],
    /// The sliders as read and the analog values sent, and the digital buttons sent.
    pub sliders: [u8; 2],
    pub triggers: [u8; 2],
    pub l: bool,
    pub r: bool,
}

/// The last [`Diag::CAPACITY`] samples, oldest first.
#[derive(Clone, Debug, Default)]
pub struct Diag {
    samples: VecDeque<Sample>,
}

impl Diag {
    /// Eight seconds at the stock adapter's 125 reports a second, one at 1000.
    pub const CAPACITY: usize = 1000;

    pub fn push(&mut self, s: Sample) {
        if self.samples.len() == Self::CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }

    pub fn samples(&self) -> impl ExactSizeIterator<Item = &Sample> + DoubleEndedIterator {
        self.samples.iter()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}
