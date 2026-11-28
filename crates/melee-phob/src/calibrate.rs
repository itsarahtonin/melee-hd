// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// `applyCalFromPoints`, `calibrationSkipMeasurement`, `calibrationUndo` and `calibrationAdvance`
// of `PhobGCC/common/phobGCC.h`, and the notch adjustment of the RP2040 main loop
// (`PhobGCC/rp2040/src/main.cpp`). See lib.rs for its authors. Distributed under the GNU General
// Public License version 3 only.

//! A stick's calibration, step by step as PhobGCC takes it: 32 measurements (the centre, then
//! a notch, sixteen times: the cardinals, the diagonals, then the modder notches between), then
//! the twelve notches' angles adjusted (diagonals first), each step advanced with A.

use crate::profile::StickCal;
use crate::settings::{Settings, WhichStick};
use crate::stick::{
    ADJ_NOTCHES, CAL_ORDER, CAL_POINTS, NOTCH_ADJ_ORDER, NOTCH_ANGLE_DEFAULTS,
    NOTCH_STATUS_DEFAULTS, NOTCHES, NotchStatus, StickParams, adjust_notch, clean_cal_points,
    clean_notches, compute_stick_angles, legalize_notches, linearize_cal, notch_calibrate,
    strip_cal_points, transform_cal_points,
};

const POINTS: i32 = CAL_POINTS as i32;
const LAST: i32 = POINTS + ADJ_NOTCHES as i32;

/// One stick's calibration in progress (the RP2040 main loop's calibration statics).
#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    pub stick: WhichStick,
    /// 0 to 31 measuring, 32 to 43 adjusting notches, -1 finished (`currentCalStep`).
    pub step: i32,
    pub points_x: [f32; CAL_POINTS],
    pub points_y: [f32; CAL_POINTS],
    pub notch_angles: [f32; NOTCHES],
    pub measured: [f32; NOTCHES],
    pub status: [NotchStatus; NOTCHES],
    /// Whether Undo goes back a step (`undoCal`).
    pub undo_ok: bool,
    /// The stick's calibration before this one, which PhobGCC reads back from its storage.
    pub stored: StickCal,
}

/// A measurement from samples of one axis, each a byte (0 to 255): their mean as PhobGCC
/// averages its 128 readings, `v / 256` of full scale each.
pub fn measure(samples: &[u8]) -> f32 {
    let mut x = 0f32;
    for &v in samples {
        x = (x as f64 + (v as i32 * 16) as f64 / 4096.0 * 1.0f32 as f64) as f32;
    }
    (x as f64 / samples.len().max(1) as f64) as f32
}

impl Calibration {
    /// Starts calibrating `stick`, whose calibration so far is `stored` (the default one when it
    /// has none).
    pub fn start(stick: WhichStick, stored: StickCal) -> Calibration {
        Calibration {
            stick,
            step: 0,
            points_x: [0.0; CAL_POINTS],
            points_y: [0.0; CAL_POINTS],
            notch_angles: stored.notch_angles,
            measured: [0.0; NOTCHES],
            status: NOTCH_STATUS_DEFAULTS,
            undo_ok: false,
            stored,
        }
    }

    pub fn finished(&self) -> bool {
        self.step < 0
    }

    /// Whether the step measures (rather than adjusts notches).
    pub fn measuring(&self) -> bool {
        (0..POINTS).contains(&self.step)
    }

    /// The notch the step measures, none for a centre step; during notch adjustment, the notch
    /// adjusted.
    pub fn notch(&self) -> Option<usize> {
        if self.measuring() {
            let point = CAL_ORDER[self.step as usize];
            (point % 2 == 1).then_some(point / 2)
        } else if (POINTS..LAST).contains(&self.step) {
            Some(NOTCH_ADJ_ORDER[(self.step - POINTS) as usize])
        } else {
            None
        }
    }

    /// `applyCalFromPoints`: the stick's parameters from the points measured, the modder
    /// notches' angles measured from where they map, and the notch angles cleaned.
    pub fn apply_from_points(&mut self, s: &Settings, p: &mut StickParams) {
        self.notch_angles = self.stored.notch_angles;
        let (sx, sy) = strip_cal_points(&self.points_x, &self.points_y);
        let c = clean_cal_points(&sx, &sy, &NOTCH_ANGLE_DEFAULTS, &mut self.status);
        let (lx, ly) = linearize_cal(&c.x, &c.y, p);
        notch_calibrate(&lx, &ly, &c.notch_x, &c.notch_y, p);
        let c = clean_cal_points(
            &self.points_x,
            &self.points_y,
            &NOTCH_ANGLE_DEFAULTS,
            &mut self.status,
        );
        let (tx, ty) = transform_cal_points(&c.x, &c.y, p, s, self.stick);
        self.measured = compute_stick_angles(&tx, &ty);
        clean_cal_points(
            &self.points_x,
            &self.points_y,
            &self.measured,
            &mut self.status,
        );
        clean_notches(&mut self.notch_angles, &self.measured, &self.status);
        self.recompute(p);
    }

    /// Cleans the points with the notch angles as they are and derives `p` from them.
    fn recompute(&mut self, p: &mut StickParams) {
        let c = clean_cal_points(
            &self.points_x,
            &self.points_y,
            &self.notch_angles,
            &mut self.status,
        );
        let (lx, ly) = linearize_cal(&c.x, &c.y, p);
        notch_calibrate(&lx, &ly, &c.notch_x, &c.notch_y, p);
    }

    /// `calibrationSkipMeasurement`: skips to notch adjustment with the stored points.
    pub fn skip(&mut self, s: &Settings, p: &mut StickParams) {
        self.step = POINTS;
        self.points_x = self.stored.points_x;
        self.points_y = self.stored.points_y;
        self.apply_from_points(s, p);
    }

    fn inactive(&self, step: i32) -> bool {
        let index = NOTCH_ADJ_ORDER[(step - POINTS).min(ADJ_NOTCHES as i32 - 1) as usize];
        self.status[index] == NotchStatus::TertInactive
    }

    /// `calibrationUndo`: back to the centre before the previous notch while measuring, or the
    /// previous notch while adjusting, skipping notches that weren't measured.
    pub fn undo(&mut self) {
        let step = self.step;
        if step % 2 == 0 && step < POINTS && step != 0 {
            self.step -= 2;
        } else if step % 2 == 1 && step < POINTS && step != 0 {
            self.step = (self.step - 3).max(0);
        } else if step > POINTS {
            self.step -= 1;
        }
        while self.step >= POINTS && self.step < LAST && self.inactive(self.step) {
            self.step -= 1;
        }
    }

    /// `calibrationAdvance`: records the measurement `point` (x and y, 0 to 1) while measuring,
    /// and goes on: after the last measurement the parameters are computed into `p`; after the
    /// last notch the calibration is finished and returned, with `p` its parameters.
    pub fn advance(
        &mut self,
        point: (f32, f32),
        s: &Settings,
        p: &mut StickParams,
    ) -> Option<StickCal> {
        if self.step < POINTS {
            let i = CAL_ORDER[self.step as usize];
            self.points_x[i] = point.0;
            self.points_y[i] = point.1;
        }
        self.step += 1;
        self.undo_ok = self.step >= 2 && self.step != POINTS;
        if self.step == POINTS {
            legalize_notches(self.step, &self.measured, &mut self.notch_angles);
            self.apply_from_points(s, p);
        }
        while self.step >= POINTS && self.step < LAST && self.inactive(self.step) {
            legalize_notches(self.step, &self.measured, &mut self.notch_angles);
            self.step += 1;
        }
        if self.step >= LAST {
            let cal = StickCal {
                points_x: self.points_x,
                points_y: self.points_y,
                notch_angles: self.notch_angles,
            };
            self.recompute(p);
            self.step = -1;
            return Some(cal);
        }
        None
    }

    /// The notch adjustment of each sample (`adjustNotch` and the main loop after it): turns the
    /// notch clockwise (`cw`) or counterclockwise for `loop_delta` milliseconds, or resets it,
    /// and with any of them held derives `p` again.
    pub fn adjust(
        &mut self,
        loop_delta: f32,
        cw: bool,
        ccw: bool,
        reset: bool,
        p: &mut StickParams,
    ) {
        if !(POINTS..LAST).contains(&self.step) {
            return;
        }
        adjust_notch(
            self.step,
            loop_delta,
            &self.measured,
            &mut self.notch_angles,
            &self.status,
            cw,
            ccw,
            reset,
        );
        if cw || ccw || reset {
            self.recompute(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stick::tests::ideal_points;
    use crate::stick::{linearize, notch_remap, params_from};

    #[test]
    fn measure_averages() {
        assert_eq!(measure(&[128; 128]), 0.5);
        assert_eq!(measure(&[0, 128]), 0.25);
    }

    /// Calibrates with the made-up stick's points, A pressed at each step.
    fn calibrate(cal: &mut Calibration, p: &mut StickParams) -> StickCal {
        let (px, py) = ideal_points(0.3);
        let s = Settings::default();
        loop {
            let point = if cal.measuring() {
                let i = CAL_ORDER[cal.step as usize];
                (px[i], py[i])
            } else {
                (0.0, 0.0)
            };
            if let Some(done) = cal.advance(point, &s, p) {
                return done;
            }
        }
    }

    #[test]
    fn a_full_calibration() {
        let mut cal = Calibration::start(WhichStick::A, StickCal::default());
        let mut p = StickParams::default();
        let done = calibrate(&mut cal, &mut p);
        assert!(cal.finished());
        // The parameters are those derived from what was stored, as at power-on.
        let mut q = StickParams::default();
        params_from(&done.points_x, &done.points_y, &done.notch_angles, &mut q);
        assert_eq!(p, q);
        let s = Settings::default();
        let x = linearize(done.points_x[17], &p.fit_coeffs_x);
        let y = linearize(done.points_y[17], &p.fit_coeffs_y);
        let (ox, oy) = notch_remap(x, y, &p, -1, &s, WhichStick::A);
        assert!((ox + 100.0).abs() < 0.01 && oy == 0.0, "left: {ox} {oy}");
    }

    #[test]
    fn unmeasured_modder_notches_are_skipped() {
        let mut cal = Calibration::start(WhichStick::A, StickCal::default());
        let mut p = StickParams::default();
        let (px, py) = ideal_points(0.3);
        let s = Settings::default();
        for step in 0..POINTS {
            let i = CAL_ORDER[step as usize];
            cal.advance((px[i], py[i]), &s, &mut p);
        }
        // The four diagonals are adjusted, then the modder notches, none measured, skipped.
        assert_eq!(cal.step, POINTS);
        for _ in 0..3 {
            cal.advance((0.0, 0.0), &s, &mut p);
        }
        assert_eq!(cal.notch(), Some(14));
        assert!(cal.advance((0.0, 0.0), &s, &mut p).is_some());
    }

    #[test]
    fn undo_steps_back() {
        let mut cal = Calibration::start(WhichStick::C, StickCal::default());
        let mut p = StickParams::default();
        let s = Settings::default();
        for _ in 0..5 {
            cal.advance((0.5, 0.5), &s, &mut p);
        }
        assert!(cal.undo_ok);
        // Measuring a notch (step 5): back to the centre before the previous notch.
        cal.undo();
        assert_eq!(cal.step, 2);
        cal.undo();
        assert_eq!(cal.step, 0);
    }

    #[test]
    fn notch_adjustment_turns_and_resets() {
        let mut cal = Calibration::start(WhichStick::A, StickCal::default());
        let mut p = StickParams::default();
        let (px, py) = ideal_points(0.3);
        let s = Settings::default();
        for step in 0..POINTS {
            let i = CAL_ORDER[step as usize];
            cal.advance((px[i], py[i]), &s, &mut p);
        }
        let before = cal.notch_angles[2];
        cal.adjust(8.0, false, true, false, &mut p);
        assert!(cal.notch_angles[2] > before);
        cal.adjust(8.0, false, false, true, &mut p);
        assert_eq!(cal.notch_angles[2], cal.measured[2]);
    }
}
