// SPDX-License-Identifier: GPL-3.0-only
//
// Ported from PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea (v0.31).
// PhobGCC-SW is licensed under the GNU General Public License version 3. Its contributors
// (CONTRIBUTORS.md): Phobos132, FrostSSBM, CarVac, wav, Savestate, bjartskular1, NiceMitch,
// ArteSSBM, JonnyHaystack, Rienne, Kadano, Vurj, Altimor, venus23gg, mizuyoukanao.
// This port is part of melee-hd and is distributed under the same license, GPL-3.0-only.

//! PhobGCC's processing for a stock GameCube controller on the adapter: the sticks'
//! calibration (linearization and the notch remap), the snapback filter, waveshaping, the
//! smoothing, cardinal snapping, the analog scaler, and the triggers' modes, ported from
//! PhobGCC-SW's `PhobGCC/common` (`stick.h`, `filter.h`, `curveFitting.h`, and `readSticks`,
//! the calibration steps and the trigger output of `phobGCC.h`).
//!
//! This crate is GPL-3.0-only, as PhobGCC-SW is; the rest of melee-hd only calls into it.
//!
//! The firmware reads its sensors at 12 bits and runs once a millisecond; on the adapter the
//! controller's own 8-bit values arrive once a report. [`Processor::process`] runs once a report,
//! with that report's time step: a stick's byte `v` is PhobGCC's raw reading `v / 256` (what its
//! ADC's `n / 4096` is for `n = 16 v`), and the filters' gains are PhobGCC's for 1 ms scaled to the
//! step as its own `recomputeGains` scales them from 1.2 ms. At a 1 ms step every computation is
//! the firmware's, bit for bit on the same machine (`tools/phob-check`).
//!
//! The float semantics are those of the RP2040 build (PhobGCC 2): there `abs`, `pow`, `fmin` and
//! the rest name C's `int` and `double` functions, so `abs(x)` on a float truncates it to an
//! integer first ([`stick::notch_remap`]). Where upstream mixes `float` and `double`, so does
//! this port.
//!
//! Not ported: PhobGCC's button remap (melee-hd's bindings are the one remap layer), its
//! tournament toggle, rumble strength, the C-stick offset its firmware never applies, the
//! extras (ESS), and the raw-output mode, which an uncalibrated stick here replaces with the
//! controller's own values ([`Processor`]).

mod c;
pub mod calibrate;
pub mod curve_fit;
pub mod diag;
pub mod filter;
pub mod processor;
pub mod profile;
pub mod settings;
pub mod stick;
pub mod sticks;
pub mod triggers;

pub use calibrate::Calibration;
pub use processor::{Input, Output, Processor};
pub use profile::{Profile, StickCal};
pub use settings::{Settings, WhichStick};

/// The PhobGCC-SW commit this crate is ported from (v0.31).
pub const UPSTREAM_COMMIT: &str = "b4f175eaff2578193a82ad9706f92bf1cd733312";
