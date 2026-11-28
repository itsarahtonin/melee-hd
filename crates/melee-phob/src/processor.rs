// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea: the
// RP2040 main loop's order (`PhobGCC/rp2040/src/main.cpp`: the calibration's notch adjustment,
// `readSticks`, then `processButtons`' calibration controls and trigger output), run once an
// adapter report. See lib.rs for its authors. Distributed under the GNU General Public License
// version 3 only.

//! One controller's processing, report by report.

use std::collections::VecDeque;

use crate::calibrate::{Calibration, measure};
use crate::diag::{Diag, Sample};
use crate::filter::recompute_gains;
use crate::profile::{Profile, StickCal};
use crate::settings::{FilterGains, GAINS, Settings, WhichStick};
use crate::stick::{INT_ORIGIN, StickParams};
use crate::sticks::Sticks;
use crate::triggers;

// PADStatus buttons.
const B: u16 = 0x0200;
const X: u16 = 0x0400;
const Y: u16 = 0x0800;
const Z: u16 = 0x0010;
const A: u16 = 0x0100;
const L: u16 = 0x0040;
const R: u16 = 0x0020;
const START: u16 = 0x1000;

/// A report from the controller, as the adapter gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input {
    /// PADStatus buttons.
    pub buttons: u16,
    /// Control stick x and y, C-stick x and y: the controller's bytes.
    pub sticks: [u8; 4],
    /// L and R's sliders less their origin, where a PhobGCC board's sliders rest near 0.
    pub sliders: [u8; 2],
    /// The sticks' bytes when the controller was plugged in (its origin), for a stick that
    /// isn't calibrated.
    pub origin: [u8; 4],
}

/// What PhobGCC sends: the buttons, the sticks centred on 127 (the origin it reports), and the
/// triggers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub buttons: u16,
    pub sticks: [u8; 4],
    pub triggers: [u8; 2],
}

/// The origin PhobGCC reports: sticks at 127, triggers at 0.
pub const ORIGIN: [u8; 6] = [INT_ORIGIN, INT_ORIGIN, INT_ORIGIN, INT_ORIGIN, 0, 0];

/// Nothing pressed, the sticks at the origin: what the game gets while a stick is calibrated.
pub const NEUTRAL: Output = Output {
    buttons: 0,
    sticks: [INT_ORIGIN; 4],
    triggers: [0, 0],
};

/// How many recent reports a calibration measurement averages.
const MEASURE: usize = 8;

/// One controller's processing: its profile, the sticks' parameters, the filters' history, a
/// calibration in progress, and the recent samples for the diagnostic views.
pub struct Processor {
    profile: Profile,
    /// Bumped when the processor changes the profile itself (a calibration finished), so the
    /// one keeping it knows to save it.
    revision: u64,
    a: StickParams,
    c: StickParams,
    gains: FilterGains,
    gains_for: Option<(f32, Settings)>,
    sticks: Sticks,
    cal: Option<Calibration>,
    recent: VecDeque<[u8; 4]>,
    held: u16,
    diag: Diag,
}

fn params(cal: &Option<StickCal>) -> StickParams {
    cal.map(|c| c.params().0).unwrap_or_default()
}

impl Processor {
    pub fn new(profile: Profile) -> Processor {
        Processor {
            a: params(&profile.a),
            c: params(&profile.c),
            profile,
            revision: 0,
            gains: GAINS,
            gains_for: None,
            sticks: Sticks::default(),
            cal: None,
            recent: VecDeque::new(),
            held: 0,
            diag: Diag::default(),
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Takes another profile (or the same one changed): the filters keep their history; a
    /// calibration in progress is cancelled.
    pub fn set_profile(&mut self, profile: Profile) {
        self.cal = None;
        if profile.a != self.profile.a {
            self.a = params(&profile.a);
        }
        if profile.c != self.profile.c {
            self.c = params(&profile.c);
        }
        self.profile = profile;
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.profile.settings = settings;
    }

    /// Starts calibrating a stick; the game gets nothing from the controller until it's done.
    pub fn calibrate(&mut self, stick: WhichStick) {
        let stored = match stick {
            WhichStick::A => self.profile.a,
            WhichStick::C => self.profile.c,
        };
        self.cal = Some(Calibration::start(stick, stored.unwrap_or_default()));
    }

    pub fn calibration(&self) -> Option<&Calibration> {
        self.cal.as_ref()
    }

    /// Whether a stick has a calibration to skip measuring with.
    fn stored(&self, stick: WhichStick) -> bool {
        match stick {
            WhichStick::A => self.profile.a.is_some(),
            WhichStick::C => self.profile.c.is_some(),
        }
    }

    /// Stops calibrating, keeping the calibration the stick had.
    pub fn cancel_calibration(&mut self) {
        if let Some(cal) = self.cal.take() {
            match cal.stick {
                WhichStick::A => self.a = params(&self.profile.a),
                WhichStick::C => self.c = params(&self.profile.c),
            }
        }
    }

    /// Undo, as Z does.
    pub fn undo(&mut self) {
        if let Some(cal) = &mut self.cal
            && cal.undo_ok
        {
            cal.undo();
        }
    }

    /// Skip measuring, as Start does: only for a stick that has a calibration to take the
    /// points from (upstream's would take a hard reset's zeros).
    pub fn skip(&mut self) {
        let Some(stick) = self.cal.as_ref().map(|c| c.stick) else {
            return;
        };
        if !self.stored(stick) {
            return;
        }
        let s = self.profile.settings;
        let (cal, p) = self.cal_and_params();
        if let Some(cal) = cal
            && cal.measuring()
        {
            cal.skip(&s, p);
        }
    }

    /// Advance, as A does: measures the recent reports while measuring.
    pub fn advance(&mut self) {
        let Some(stick) = self.cal.as_ref().map(|c| c.stick) else {
            return;
        };
        let k = match stick {
            WhichStick::A => 0,
            WhichStick::C => 2,
        };
        let xs: Vec<u8> = self.recent.iter().map(|r| r[k]).collect();
        let ys: Vec<u8> = self.recent.iter().map(|r| r[k + 1]).collect();
        let point = (measure(&xs), measure(&ys));
        let s = self.profile.settings;
        let (cal, p) = self.cal_and_params();
        let Some(cal) = cal else { return };
        if let Some(done) = cal.advance(point, &s, p) {
            match stick {
                WhichStick::A => self.profile.a = Some(done),
                WhichStick::C => self.profile.c = Some(done),
            }
            self.cal = None;
            self.revision += 1;
        }
    }

    fn cal_and_params(&mut self) -> (Option<&mut Calibration>, &mut StickParams) {
        let p = match self.cal.as_ref().map(|c| c.stick) {
            Some(WhichStick::C) => &mut self.c,
            _ => &mut self.a,
        };
        (self.cal.as_mut(), p)
    }

    pub fn diag(&self) -> &Diag {
        &self.diag
    }

    /// The filters' gains for a step, recomputed only when the step or the settings change.
    fn gains(&mut self, step_ms: f32) -> FilterGains {
        let s = self.profile.settings;
        if self.gains_for.map(|(_, g)| g) != Some(s)
            || self.gains_for.map(|(t, _)| t) != Some(step_ms)
        {
            let mut base = GAINS;
            self.gains = recompute_gains(&s, &mut base, step_ms as f64);
            self.gains_for = Some((step_ms, s));
        }
        self.gains
    }

    /// Processes one report, `step_ms` milliseconds after the one before.
    pub fn process(&mut self, input: &Input, step_ms: f32) -> Output {
        let n = self.gains(step_ms);
        let s = self.profile.settings;
        if self.recent.len() == MEASURE {
            self.recent.pop_front();
        }
        self.recent.push_back(input.sticks);
        let raw = input.sticks.map(|v| v as f32 / 256.0);
        let hw = input.buttons;
        let pressed = hw & !self.held;
        self.held = hw;

        let out = if let Some(stick) = self.cal.as_ref().map(|c| c.stick) {
            let (cal, p) = self.cal_and_params();
            let step = cal.as_ref().map_or(-1, |c| c.step);
            if let Some(cal) = cal {
                cal.adjust(step_ms, hw & X != 0, hw & Y != 0, hw & B != 0, p);
            }
            self.sticks.read(
                raw,
                stick == WhichStick::A,
                stick == WhichStick::C,
                &s,
                &n,
                &self.a,
                &self.c,
                step,
                false,
            );
            if hw & START != 0 {
                self.skip();
            }
            if pressed & Z != 0 {
                self.undo();
            }
            if pressed & (A | L | R) != 0 {
                self.advance();
            }
            NEUTRAL
        } else {
            let raw_out = self
                .sticks
                .read(raw, true, true, &s, &n, &self.a, &self.c, -1, false);
            let mut sticks = self.sticks.out;
            // An uncalibrated stick sends the controller's own values, about its origin.
            let passthrough = |k: usize| {
                (input.sticks[k] as i32 - input.origin[k] as i32 + INT_ORIGIN as i32).clamp(0, 255)
                    as u8
            };
            for (cal, k) in [(self.profile.a.is_some(), 0), (self.profile.c.is_some(), 2)] {
                if !cal {
                    sticks[k] = passthrough(k);
                    sticks[k + 1] = passthrough(k + 1);
                }
            }
            let (buttons, triggers) =
                triggers::buttons(&s, hw, [input.sliders[0] as f32, input.sliders[1] as f32]);
            let u = raw_out.unfiltered;
            self.diag.push(Sample {
                step_ms,
                a: [sticks[0] as i32 - 127, sticks[1] as i32 - 127],
                c: [sticks[2] as i32 - 127, sticks[3] as i32 - 127],
                a_unfiltered: [u[0], u[1]],
                c_unfiltered: [u[2], u[3]],
                sliders: input.sliders,
                triggers,
                l: buttons & L != 0,
                r: buttons & R != 0,
            });
            Output {
                buttons,
                sticks,
                triggers,
            }
        };
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stick::CAL_ORDER;
    use crate::stick::tests::ideal_points;

    #[test]
    fn uncalibrated_sticks_pass_through_and_buttons_too() {
        let mut p = Processor::new(Profile::default());
        let input = Input {
            buttons: A | X,
            sticks: [200, 60, 130, 128],
            sliders: [40, 0],
            origin: [128, 127, 128, 130],
        };
        let out = p.process(&input, 8.0);
        assert_eq!(out.sticks, [199, 60, 129, 125]);
        assert_eq!(out.buttons, A | X);
        assert_eq!(out.triggers, [40, 0]);
    }

    /// The byte PhobGCC's raw reading `v` of full scale is.
    fn byte(v: f32) -> u8 {
        (v * 256.0).round() as u8
    }

    #[test]
    fn calibrating_with_the_controller() {
        let mut p = Processor::new(Profile::default());
        p.calibrate(WhichStick::A);
        let (px, py) = ideal_points(0.3);
        let report = |x: f32, y: f32, buttons: u16| Input {
            buttons,
            sticks: [byte(x), byte(y), 128, 128],
            ..Input::default()
        };
        let mut presses = 0;
        while p.calibration().is_some() {
            let cal = p.calibration().unwrap();
            let (x, y) = if cal.measuring() {
                let i = CAL_ORDER[cal.step as usize];
                (px[i], py[i])
            } else {
                (0.5, 0.5)
            };
            for _ in 0..MEASURE {
                assert_eq!(p.process(&report(x, y, 0), 8.0), NEUTRAL);
            }
            p.process(&report(x, y, A), 8.0);
            presses += 1;
            assert!(presses < 100);
        }
        assert_eq!(p.revision(), 1);
        assert!(p.profile().a.is_some());
        // Now calibrated: full right sends 100 from the origin, the centre the origin.
        let mut last = NEUTRAL;
        for _ in 0..50 {
            last = p.process(&report(0.8, 0.5, 0), 8.0);
        }
        assert_eq!((last.sticks[0], last.sticks[1]), (227, 127));
        for _ in 0..50 {
            last = p.process(&report(0.5, 0.5, 0), 8.0);
        }
        assert_eq!((last.sticks[0], last.sticks[1]), (127, 127));
        assert!(p.diag().samples().len() >= 100);
    }

    #[test]
    fn cancelling_keeps_the_old_calibration() {
        let mut p = Processor::new(Profile::default());
        p.calibrate(WhichStick::C);
        p.process(&Input::default(), 1.0);
        p.cancel_calibration();
        assert!(p.calibration().is_none());
        assert_eq!(p.profile().c, None);
        // Skip does nothing without a stored calibration.
        p.calibrate(WhichStick::C);
        p.skip();
        assert_eq!(p.calibration().unwrap().step, 0);
    }
}
