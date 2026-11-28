// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// `PhobGCC/common/filter.h`. See lib.rs for its authors. Distributed under the GNU General Public
// License version 3 only.

//! The snapback filter (`runKalman`), waveshaping and the gains they run with.
//!
//! Upstream keeps each filter's history in function statics; here it's the structs' fields.

use crate::c::{fmax, fmin};
use crate::settings::{FilterGains, Settings};

/// `velDampFromSnapback`.
pub fn vel_damp_from_snapback(snapback: i32) -> f32 {
    if snapback >= 0 {
        (0.125 * 2f64.powf((snapback - 4) as f64 / 3.0)) as f32
    } else {
        (1.0 - 0.25 * 2f64.powf((snapback + 4) as f64 / 3.0)) as f32
    }
}

/// `recomputeGains`: sets `gains`' damping and smoothing from the settings and returns the
/// gains the filters use for a time step of `step_ms`. Upstream's is for its 1 ms loop, scaling
/// gains meant for 1.2 ms by `1.0 / 1.2` and `1.2 / 1.0`; here the step is the report's, and at
/// 1 ms the factors (and so every gain) are upstream's exactly.
pub fn recompute_gains(s: &Settings, gains: &mut FilterGains, step_ms: f64) -> FilterGains {
    gains.x_vel_damp = vel_damp_from_snapback(s.x_snapback);
    gains.y_vel_damp = vel_damp_from_snapback(s.y_snapback);
    let smoothing = |v: i32| {
        if v <= 8 {
            v as f32 / 10.0
        } else {
            v as f32 / 100.0 + 0.72
        }
    };
    gains.x_smoothing = smoothing(s.ax_smoothing);
    gains.y_smoothing = smoothing(s.ay_smoothing);
    gains.cx_smoothing = smoothing(s.cx_smoothing);
    gains.cy_smoothing = smoothing(s.cy_smoothing);

    let time_factor = (step_ms / 1.2) as f32;
    let time_divisor = (1.2 / step_ms) as f32;
    let g = *gains;
    let mut n = FilterGains {
        max_stick: g.max_stick * g.max_stick,
        x_vel_decay: g.x_vel_decay * time_factor,
        y_vel_decay: g.y_vel_decay * time_factor,
        x_vel_pos_factor: g.x_vel_pos_factor * time_factor,
        y_vel_pos_factor: g.y_vel_pos_factor * time_factor,
        x_vel_damp: g.x_vel_damp,
        y_vel_damp: g.y_vel_damp,
        vel_thresh: 1.0 / (g.vel_thresh * time_factor),
        accel_thresh: 1.0 / (g.accel_thresh * time_factor),
        ..FilterGains::default()
    };
    if s.x_snapback >= 0 {
        n.x_vel_damp *= time_divisor;
    }
    if s.y_snapback >= 0 {
        n.y_vel_damp *= time_divisor;
    }
    n.vel_thresh *= n.vel_thresh;
    n.accel_thresh *= n.accel_thresh;
    let pow = |v: f32| ((1.0 - v) as f64).powf(time_divisor as f64) as f32;
    n.x_smoothing = pow(g.x_smoothing);
    n.y_smoothing = pow(g.y_smoothing);
    n.cx_smoothing = pow(g.cx_smoothing);
    n.cy_smoothing = pow(g.cy_smoothing);
    n
}

/// `0.5*(a + b)`, computed in double as upstream's is.
fn half(a: f32, b: f32) -> f32 {
    (0.5 * (a + b) as f64) as f32
}

/// The snapback filter's history (`runKalman`'s statics) and its output (`readSticks`'
/// `xPosFilt`, `yPosFilt`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kalman {
    pub x_pos: f32,
    pub y_pos: f32,
    pub x_vel: f32,
    pub y_vel: f32,
    pub x_vel_filt: f32,
    pub y_vel_filt: f32,
    pub x_pos_filt: f32,
    pub y_pos_filt: f32,
}

impl Kalman {
    /// `runKalman`: one step with the measurement `(xz, yz)`; the result is in `x_pos_filt` and
    /// `y_pos_filt`.
    pub fn run(&mut self, xz: f32, yz: f32, s: &Settings, n: &FilterGains) {
        let old_x_pos = self.x_pos;
        let old_y_pos = self.y_pos;
        let old_x_pos_filt = self.x_pos_filt;
        let old_y_pos_filt = self.y_pos_filt;
        let old_x_vel = self.x_vel;
        let old_y_vel = self.y_vel;
        let old_x_vel_filt = self.x_vel_filt;
        let old_y_vel_filt = self.y_vel_filt;

        self.x_pos = xz;
        self.y_pos = yz;
        self.x_vel = self.x_pos - old_x_pos;
        self.y_vel = self.y_pos - old_y_pos;
        let x_vel_smooth = half(self.x_vel, old_x_vel);
        let y_vel_smooth = half(self.y_vel, old_y_vel);
        let x_accel = self.x_vel - old_x_vel;
        let y_accel = self.y_vel - old_y_vel;
        let old_x_pos_diff = old_x_pos - old_x_pos_filt;
        let old_y_pos_diff = old_y_pos - old_y_pos_filt;

        let (x_pos, y_pos) = (self.x_pos, self.y_pos);
        let stick_distance2 = (fmin(n.max_stick as f64, (x_pos * x_pos + y_pos * y_pos) as f64)
            / n.max_stick as f64) as f32;
        let stick_distance6 = stick_distance2 * stick_distance2 * stick_distance2;
        let vel_weight1 = stick_distance2;
        let vel_weight2 = 1.0 - vel_weight1;

        let pos_weight = |vel_smooth: f32, accel: f32| -> f32 {
            let vel_acc = (1.0
                - fmin(
                    1.0,
                    (vel_smooth * vel_smooth * n.vel_thresh + accel * accel * n.accel_thresh)
                        as f64,
                )) as f32;
            fmax(vel_acc as f64, stick_distance6 as f64) as f32
        };

        if s.x_snapback > 0 {
            self.x_vel_filt = vel_weight1 * self.x_vel
                + (1.0 - n.x_vel_decay) * vel_weight2 * old_x_vel_filt
                + n.x_vel_pos_factor * old_x_pos_diff;
            let w1 = pos_weight(x_vel_smooth, x_accel);
            let w2 = 1.0 - w1;
            self.x_pos_filt =
                w1 * x_pos + w2 * (old_x_pos_filt + (1.0 - n.x_vel_damp) * self.x_vel_filt);
        } else if s.x_snapback < 0 {
            let lpf = old_x_pos_filt * n.x_vel_damp + x_pos * (1.0 - n.x_vel_damp);
            let w1 = pos_weight(x_vel_smooth, x_accel);
            let w2 = 1.0 - w1;
            self.x_pos_filt = w1 * x_pos + w2 * lpf;
        } else {
            self.x_pos_filt = x_pos;
        }

        if s.y_snapback > 0 {
            self.y_vel_filt = vel_weight1 * self.y_vel
                + (1.0 - n.y_vel_decay) * vel_weight2 * old_y_vel_filt
                + n.y_vel_pos_factor * old_y_pos_diff;
            let w1 = pos_weight(y_vel_smooth, y_accel);
            let w2 = 1.0 - w1;
            self.y_pos_filt =
                w1 * y_pos + w2 * (old_y_pos_filt + (1.0 - n.y_vel_damp) * self.y_vel_filt);
        } else if s.y_snapback < 0 {
            let lpf = old_y_pos_filt * n.y_vel_damp + y_pos * (1.0 - n.y_vel_damp);
            let w1 = pos_weight(y_vel_smooth, y_accel);
            let w2 = 1.0 - w1;
            self.y_pos_filt = w1 * y_pos + w2 * lpf;
        } else {
            self.y_pos_filt = y_pos;
        }
    }
}

/// `calcWaveshapeMult`: the setting's strength, 0 for 0.
pub fn calc_waveshape_mult(setting: i32) -> f32 {
    let d = if setting > 0 && setting <= 5 {
        440 - 40 * setting
    } else if setting > 5 && setting <= 15 {
        340 - 20 * setting
    } else if setting > 15 && setting <= 24 {
        100 - 4 * setting
    } else {
        return 0.0;
    };
    (1.0 / d as f64) as f32
}

/// Waveshaping's history for one stick (`aRunWaveShaping`'s or `cRunWaveShaping`'s statics).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Waveshaping {
    pub old_x_pos: f32,
    pub old_y_pos: f32,
    pub old_x_vel: f32,
    pub old_y_vel: f32,
    pub old_x_out: f32,
    pub old_y_out: f32,
}

impl Waveshaping {
    /// One step with the stick's settings. The control stick's (`aRunWaveShaping`) scales its
    /// y axis' acceleration term by the x axis' strength, as upstream's does: `a_stick` keeps
    /// that, and the C-stick's (`cRunWaveShaping`) uses each axis' own.
    pub fn run(
        &mut self,
        x_pos: f32,
        y_pos: f32,
        x_setting: i32,
        y_setting: i32,
        a_stick: bool,
        n: &FilterGains,
    ) -> (f32, f32) {
        let x_vel = x_pos - self.old_x_pos;
        let y_vel = y_pos - self.old_y_pos;
        let x_vel_smooth = half(x_vel, self.old_x_vel);
        let y_vel_smooth = half(y_vel, self.old_y_vel);
        let x_accel = x_vel - self.old_x_vel;
        let y_accel = y_vel - self.old_y_vel;

        let x_factor = calc_waveshape_mult(x_setting.abs());
        let y_factor = calc_waveshape_mult(y_setting.abs());
        let high_pode_x = (x_setting < 0) as i32 as f32;
        let high_pode_y = (y_setting < 0) as i32 as f32;
        let accel_factor: f32 = 3.0;
        let y_accel_factor = if a_stick { x_factor } else { y_factor };

        let old_x_weight = fmin(
            1.0,
            (x_vel_smooth * x_vel_smooth * n.vel_thresh * x_factor
                + high_pode_x * x_accel * x_accel * n.accel_thresh * x_factor * accel_factor)
                as f64,
        ) as f32;
        let new_x_weight = 1.0 - old_x_weight;
        let old_y_weight = fmin(
            1.0,
            (y_vel_smooth * y_vel_smooth * n.vel_thresh * y_factor
                + high_pode_y * y_accel * y_accel * n.accel_thresh * y_accel_factor * accel_factor)
                as f64,
        ) as f32;
        let new_y_weight = 1.0 - old_y_weight;

        let x_out = self.old_x_out * old_x_weight + x_pos * new_x_weight;
        let y_out = self.old_y_out * old_y_weight + y_pos * new_y_weight;

        self.old_x_pos = x_pos;
        self.old_y_pos = y_pos;
        self.old_x_vel = x_vel;
        self.old_y_vel = y_vel;
        self.old_x_out = x_out;
        self.old_y_out = y_out;
        (x_out, y_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::GAINS;

    #[test]
    fn gains_at_one_millisecond() {
        let mut g = GAINS;
        let n = recompute_gains(&Settings::default(), &mut g, 1.0);
        assert_eq!(g.x_vel_damp, 0.125);
        assert_eq!(n.max_stick, 10000.0);
        assert_eq!(n.x_vel_damp, 0.125 * 1.2f32);
        // The C-stick's default smoothing of 5 is 0.5, raised to 1.2.
        assert_eq!(n.cx_smoothing, 0.5f64.powf(1.2f32 as f64) as f32);
        // A longer step weighs the new sample more.
        let n8 = recompute_gains(&Settings::default(), &mut g, 8.0);
        assert!(n8.cx_smoothing > n.cx_smoothing);
        assert!(n8.x_vel_decay > n.x_vel_decay);
    }

    #[test]
    fn snapback_off_passes_through() {
        let s = Settings {
            x_snapback: 0,
            y_snapback: 0,
            ..Settings::default()
        };
        let mut g = GAINS;
        let n = recompute_gains(&s, &mut g, 1.0);
        let mut k = Kalman::default();
        for v in [0.0, 50.0, -80.0, 3.5] {
            k.run(v, -v, &s, &n);
            assert_eq!((k.x_pos_filt, k.y_pos_filt), (v, -v));
        }
    }

    #[test]
    fn waveshape_strengths() {
        assert_eq!(calc_waveshape_mult(0), 0.0);
        assert_eq!(calc_waveshape_mult(1), (1.0f64 / 400.0) as f32);
        assert_eq!(calc_waveshape_mult(24), (1.0f64 / 4.0) as f32);
        assert_eq!(calc_waveshape_mult(25), 0.0);
    }
}
