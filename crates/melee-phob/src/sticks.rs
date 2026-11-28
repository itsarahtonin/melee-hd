// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// `readSticks` of `PhobGCC/common/phobGCC.h`. See lib.rs for its authors. Distributed under the
// GNU General Public License version 3 only.

//! The sticks' pipeline, once a sample (`readSticks`): linearize, the snapback filter,
//! waveshaping, the smoothing, the notch remap, the clamp, and the output's hysteresis.

use crate::c::{fmax, fmin};
use crate::filter::{Kalman, Waveshaping};
use crate::settings::{FilterGains, Settings, WhichStick};
use crate::stick::{FLOAT_ORIGIN, INT_ORIGIN, StickParams, linearize, notch_remap};

/// What one run of the pipeline computed besides the output, for the diagnostic views
/// (`RawStick`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Raw {
    /// The raw readings, 0 to 1 of full scale: control stick x and y, C-stick x and y.
    pub raw: [f32; 4],
    /// Linearized.
    pub linearized: [f32; 4],
    /// Linearized and remapped without the filters or snapping, clamped.
    pub unfiltered: [f32; 4],
}

/// The pipeline's history (`readSticks`' statics and the filters') and its output bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sticks {
    pub kalman: Kalman,
    pub a_wave: Waveshaping,
    pub c_wave: Waveshaping,
    /// The control stick's smoothing (`oldPosAx`, `oldPosAy`) and the C-stick's (`cXPos`,
    /// `cYPos`).
    pub old_pos_a: [f32; 2],
    pub c_pos: [f32; 2],
    /// The output: control stick x and y, C-stick x and y, centred on 127 (`btn.Ax` and the
    /// rest), which the hysteresis compares with.
    pub out: [u8; 4],
}

impl Default for Sticks {
    fn default() -> Sticks {
        Sticks {
            kalman: Kalman::default(),
            a_wave: Waveshaping::default(),
            c_wave: Waveshaping::default(),
            old_pos_a: [0.0; 2],
            c_pos: [0.0; 2],
            // `initializeButtons`.
            out: [INT_ORIGIN; 4],
        }
    }
}

/// One output byte: hysteresis of 0.3 around the rounding, so a value on the edge doesn't
/// flicker.
fn hysteresis(out: &mut u8, v: f32) {
    let hyst: f32 = 0.3_f64 as f32;
    let diff = (v + FLOAT_ORIGIN) - *out as f32;
    if diff as f64 > 1.0 + hyst as f64 || diff < -hyst {
        *out = (v + FLOAT_ORIGIN) as u8;
    }
}

impl Sticks {
    /// `readSticks` after the reading: `raw` is each axis 0 to 1 of full scale. `read_a` and
    /// `read_c` say which sticks' outputs change; `cal_step` is -1 outside calibration (it
    /// turns off snapping); `currently_raw` asks for the raw output (an uncalibrated stick gets
    /// it too).
    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &mut self,
        raw: [f32; 4],
        read_a: bool,
        read_c: bool,
        s: &Settings,
        n: &FilterGains,
        a: &StickParams,
        c: &StickParams,
        cal_step: i32,
        currently_raw: bool,
    ) -> Raw {
        let [a_x, a_y, c_x, c_y] = raw;
        let xz = linearize(a_x, &a.fit_coeffs_x);
        let yz = linearize(a_y, &a.fit_coeffs_y);
        let mut pos_cx = linearize(c_x, &c.fit_coeffs_x);
        let mut pos_cy = linearize(c_y, &c.fit_coeffs_y);
        let linearized = [xz, yz, pos_cx, pos_cy];

        self.kalman.run(xz, yz, s, n);
        let (shaped_ax, shaped_ay) = self.a_wave.run(
            self.kalman.x_pos_filt,
            self.kalman.y_pos_filt,
            s.ax_waveshaping,
            s.ay_waveshaping,
            true,
            n,
        );
        let pos_ax = n.x_smoothing * shaped_ax + (1.0 - n.x_smoothing) * self.old_pos_a[0];
        let pos_ay = n.y_smoothing * shaped_ay + (1.0 - n.y_smoothing) * self.old_pos_a[1];
        self.old_pos_a = [pos_ax, pos_ay];

        (pos_cx, pos_cy) =
            self.c_wave
                .run(pos_cx, pos_cy, s.cx_waveshaping, s.cy_waveshaping, false, n);
        let [old_cx, old_cy] = self.c_pos;
        let x_weight1 = n.cx_smoothing;
        let x_weight2 = 1.0 - x_weight1;
        let y_weight1 = n.cy_smoothing;
        let y_weight2 = 1.0 - y_weight1;
        self.c_pos = [
            x_weight1 * pos_cx + x_weight2 * old_cx,
            y_weight1 * pos_cy + y_weight2 * old_cy,
        ];
        let [pos_cx, pos_cy] = self.c_pos;

        let (ax, ay) = notch_remap(pos_ax, pos_ay, a, cal_step, s, WhichStick::A);
        let (cx, cy) = notch_remap(pos_cx, pos_cy, c, cal_step, s, WhichStick::C);
        let (ax_u, ay_u) = notch_remap(xz, yz, a, 1, s, WhichStick::A);
        let (cx_u, cy_u) = notch_remap(linearized[2], linearized[3], c, 1, s, WhichStick::C);
        let clamp = |v: f32| fmin(125.0, fmax(-125.0, v as f64)) as f32;
        let (ax, ay, cx, cy) = (clamp(ax), clamp(ay), clamp(cx), clamp(cy));
        let unfiltered = [clamp(ax_u), clamp(ay_u), clamp(cx_u), clamp(cy_u)];

        let a_raw = currently_raw || (a.fit_coeffs_x[0] == 0.0 && a.fit_coeffs_y[0] == 0.0);
        let c_raw = currently_raw || (c.fit_coeffs_x[0] == 0.0 && c.fit_coeffs_y[0] == 0.0);
        let raw_out = |v: f32| (FLOAT_ORIGIN + v * 100.0) as u8;
        if read_a {
            if !a_raw {
                hysteresis(&mut self.out[0], ax);
                hysteresis(&mut self.out[1], ay);
            } else {
                self.out[0] = raw_out(a_x);
                self.out[1] = raw_out(a_y);
            }
        }
        if read_c {
            if !c_raw {
                hysteresis(&mut self.out[2], cx);
                hysteresis(&mut self.out[3], cy);
            } else {
                self.out[2] = raw_out(c_x);
                self.out[3] = raw_out(c_y);
            }
        }
        Raw {
            raw,
            linearized,
            unfiltered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_holds_near_the_edge() {
        let mut out = 127;
        // 127.5 + 0.2 = 127.7: within 1.3 above 127, held.
        hysteresis(&mut out, 0.2);
        assert_eq!(out, 127);
        hysteresis(&mut out, 0.9);
        assert_eq!(out, 128);
        // Falling: 127.5 - 0.1 = 127.4, 0.6 below 128: past 0.3, so it takes 127.
        hysteresis(&mut out, -0.1);
        assert_eq!(out, 127);
    }
}
