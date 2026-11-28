// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// `PhobGCC/common/stick.h`. See lib.rs for its authors. Distributed under the GNU General Public
// License version 3 only.

//! A stick's calibration: the linearization fit, the notch remap's affine regions, and the
//! cleaning and legalizing of the measured calibration points and notch angles.

use crate::c::{PI, abs_int, fmax, fmin};
use crate::curve_fit::fit_curve;
use crate::settings::{Settings, WhichStick};

pub const NOTCHES: usize = 16;
pub const CAL_POINTS: usize = NOTCHES * 2;
pub const ADJ_NOTCHES: usize = 12;
pub const FIT_ORDER: usize = 3;
/// The stick's largest angular deflection, 28 degrees (`_maxStickAngle`).
pub const MAX_STICK_ANGLE: f32 = 0.4886921906_f64 as f32;
/// The output's origin, for rounding (`_floatOrigin`) and as reported (`_intOrigin`).
pub const FLOAT_ORIGIN: f32 = 127.5;
pub const INT_ORIGIN: u8 = 127;

/// `_calOrder`: the calibration point each collection step measures (even: the centre before a
/// notch, odd: the notch).
pub const CAL_ORDER: [usize; CAL_POINTS] = [
    0, 1, 8, 9, 16, 17, 24, 25, 4, 5, 12, 13, 20, 21, 28, 29, 2, 3, 6, 7, 10, 11, 14, 15, 18, 19,
    22, 23, 26, 27, 30, 31,
];

/// A notch's kind (`NotchStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NotchStatus {
    /// A modder notch that wasn't measured: skipped in notch adjustment.
    #[default]
    TertInactive,
    TertActive,
    Secondary,
    Cardinal,
}

/// `_notchAngleDefaults`: every 22.5 degrees from the right, counterclockwise.
pub const NOTCH_ANGLE_DEFAULTS: [f32; NOTCHES] = {
    let mut a = [0f32; NOTCHES];
    let mut i = 1;
    while i < NOTCHES {
        a[i] = (PI * i as f64 / 8.0) as f32;
        i += 1;
    }
    a
};

/// `_notchStatusDefaults`.
pub const NOTCH_STATUS_DEFAULTS: [NotchStatus; NOTCHES] = {
    use NotchStatus::*;
    [
        Cardinal, TertActive, Secondary, TertActive, Cardinal, TertActive, Secondary, TertActive,
        Cardinal, TertActive, Secondary, TertActive, Cardinal, TertActive, Secondary, TertActive,
    ]
};

/// `_notchAdjOrder`: the notches adjusted after the points are collected, diagonals first.
pub const NOTCH_ADJ_ORDER: [usize; ADJ_NOTCHES] = [2, 6, 10, 14, 1, 3, 5, 7, 9, 11, 13, 15];

/// A stick's calibration as the processing uses it (`StickParams`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StickParams {
    /// The linearization's coefficients, highest power first.
    pub fit_coeffs_x: [f32; 4],
    pub fit_coeffs_y: [f32; 4],
    /// Each region's affine transform, and the angles where the regions begin.
    pub affine_coeffs: [[f32; 4]; NOTCHES],
    pub boundary_angles: [f32; NOTCHES],
}

/// `linearize`: the fit's cubic at `point`.
pub fn linearize(point: f32, c: &[f32; 4]) -> f32 {
    c[0] * (point * point * point) + c[1] * (point * point) + c[2] * point + c[3]
}

/// `calcStickValues`: the coordinates of a notch at `angle`, the stick moving on a sphere.
pub fn calc_stick_values(angle: f32) -> (f32, f32) {
    // Kept from being folded at compile time, so `sinf` and `cosf` are the platform's, as
    // upstream's calls are.
    let max = std::hint::black_box(MAX_STICK_ANGLE);
    let x = 100.0 * (max.sin() * angle.cos()).atan2(max.cos()) / max;
    let y = 100.0 * (max.sin() * angle.sin()).atan2(max.cos()) / max;
    (x, y)
}

/// `angleOnSphere`: the azimuth of the coordinates, the inverse of [`calc_stick_values`].
pub fn angle_on_sphere(x: f32, y: f32) -> f32 {
    let xx = (x * MAX_STICK_ANGLE / 100.0).sin() * (y * MAX_STICK_ANGLE / 100.0).cos();
    let yy = (x * MAX_STICK_ANGLE / 100.0).cos() * (y * MAX_STICK_ANGLE / 100.0).sin();
    let mut angle = yy.atan2(xx);
    if angle < 0.0 {
        angle = (angle as f64 + 2.0 * PI) as f32;
    }
    angle
}

/// `stripCalPoints`: the points without the modder notches (each fourth from index 3, set to
/// the first centre).
pub fn strip_cal_points(
    x: &[f32; CAL_POINTS],
    y: &[f32; CAL_POINTS],
) -> ([f32; CAL_POINTS], [f32; CAL_POINTS]) {
    let (mut sx, mut sy) = (*x, *y);
    for i in 0..CAL_POINTS {
        if (i + 1) % 4 == 0 {
            sx[i] = x[0];
            sy[i] = y[0];
        }
    }
    (sx, sy)
}

/// `computeStickAngles`: the notches' angles from transformed points (17 of them, the centre
/// first), the cardinals and diagonals at their defaults.
pub fn compute_stick_angles(x: &[f32; NOTCHES + 1], y: &[f32; NOTCHES + 1]) -> [f32; NOTCHES] {
    let mut angles = [0f32; NOTCHES];
    for i in 0..NOTCHES {
        angles[i] = if i % 2 == 0 {
            NOTCH_ANGLE_DEFAULTS[i]
        } else {
            angle_on_sphere(x[i + 1], y[i + 1])
        };
    }
    angles
}

/// `cleanNotches`: an unmeasured notch takes its measured angle.
pub fn clean_notches(
    notch_angles: &mut [f32; NOTCHES],
    measured: &[f32; NOTCHES],
    status: &[NotchStatus; NOTCHES],
) {
    for i in 0..NOTCHES {
        if status[i] == NotchStatus::TertInactive {
            notch_angles[i] = measured[i];
        }
    }
}

/// `notchRemap`: the stick's position through its region's affine transform and the analog
/// scaler, then, outside calibration (`cal_step == -1`), cardinal snapping and the small
/// deadzone. `abs` here is C's `int abs(int)` on the truncated float, as on the RP2040 build.
pub fn notch_remap(
    x_in: f32,
    y_in: f32,
    p: &StickParams,
    cal_step: i32,
    s: &Settings,
    stick: WhichStick,
) -> (f32, f32) {
    let regions = NOTCHES;
    let mut angle = y_in.atan2(x_in);
    if angle < p.boundary_angles[0] {
        angle = (angle as f64 + PI * 2.0) as f32;
    }
    let mut region = regions - 1;
    for i in 1..regions {
        if angle < p.boundary_angles[i] {
            region = i - 1;
            break;
        }
    }
    let a = &p.affine_coeffs[region];
    let mut x = a[0] * x_in + a[1] * y_in;
    let mut y = a[2] * x_in + a[3] * y_in;
    let scaler = match stick {
        WhichStick::A => s.a_analog_scaler,
        WhichStick::C => s.c_analog_scaler,
    };
    let stick_scale = scaler as f32 / 100.0;
    x *= stick_scale;
    y *= stick_scale;

    if cal_step == -1 {
        let snapping = match stick {
            WhichStick::A => s.a_cardinal_snapping,
            WhichStick::C => s.c_cardinal_snapping,
        };
        let rim = |v: f32| abs_int(v) as f64 >= 79.5;
        if snapping > 0 {
            let near = |v: f32| (abs_int(v) as f64) < snapping as f64 + 0.5;
            if near(x) && rim(y) {
                x = 0.0;
            }
            if near(y) && rim(x) {
                y = 0.0;
            }
        } else if snapping < 0 {
            let (limit, to) = if snapping == -2 {
                (6.5, 7.0)
            } else {
                (0.5, 1.0)
            };
            if (abs_int(x) as f64) < limit && rim(y) {
                x = if x >= 0.0 { to } else { -to };
            }
            if (abs_int(y) as f64) < limit && rim(x) {
                y = if y >= 0.0 { to } else { -to };
            }
        }
        if abs_int(x) < 3 && abs_int(y) < 3 {
            x = 0.0;
            y = 0.0;
        }
    }
    (x, y)
}

/// `transformCalPoints`: the cleaned points (17) linearized and remapped, without snapping.
pub fn transform_cal_points(
    x: &[f32; NOTCHES + 1],
    y: &[f32; NOTCHES + 1],
    p: &StickParams,
    s: &Settings,
    stick: WhichStick,
) -> ([f32; NOTCHES + 1], [f32; NOTCHES + 1]) {
    let mut ox = [0f32; NOTCHES + 1];
    let mut oy = [0f32; NOTCHES + 1];
    for i in 0..NOTCHES + 1 {
        let xv = linearize(x[i], &p.fit_coeffs_x);
        let yv = linearize(y[i], &p.fit_coeffs_y);
        (ox[i], oy[i]) = notch_remap(xv, yv, p, 0, s, stick);
    }
    (ox, oy)
}

/// What [`clean_cal_points`] makes: the centre and each notch's measured point (17), and the
/// coordinates each should map to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cleaned {
    pub x: [f32; NOTCHES + 1],
    pub y: [f32; NOTCHES + 1],
    pub notch_x: [f32; NOTCHES + 1],
    pub notch_y: [f32; NOTCHES + 1],
}

/// `cleanCalPoints`: the centre averaged from the 16 centre measurements less the two largest
/// and two smallest of each axis, each notch's measured point, the coordinates its angle asks
/// for, and each notch's status (an unmeasured modder notch averaged from its neighbours and
/// marked inactive).
pub fn clean_cal_points(
    cx: &[f32; CAL_POINTS],
    cy: &[f32; CAL_POINTS],
    notch_angles: &[f32; NOTCHES],
    status: &mut [NotchStatus; NOTCHES],
) -> Cleaned {
    let mut c = Cleaned::default();
    for i in 0..NOTCHES {
        c.x[0] += cx[i * 2];
        c.y[0] += cy[i * 2];
        c.x[i + 1] = cx[i * 2 + 1];
        c.y[i + 1] = cy[i * 2 + 1];
        let (x, y) = calc_stick_values(notch_angles[i]);
        c.notch_x[i + 1] = (x as f64).round() as f32;
        c.notch_y[i + 1] = (y as f64).round() as f32;
    }

    let (mut smallest_x, mut small_x, mut large_x, mut largest_x) = (0, 0, 0, 0);
    let (mut smallest_y, mut small_y, mut large_y, mut largest_y) = (0, 0, 0, 0);
    for i in 0..NOTCHES {
        let j = i * 2;
        if cx[j] < cx[smallest_x] {
            small_x = smallest_x;
            smallest_x = j;
        } else if cx[j] < cx[small_x] {
            small_x = j;
        }
        if cx[j] > cx[largest_x] {
            large_x = largest_x;
            largest_x = j;
        } else if cx[j] > cx[large_x] {
            large_x = j;
        }
        if cy[j] < cy[smallest_y] {
            small_y = smallest_y;
            smallest_y = j;
        } else if cy[j] < cy[small_y] {
            small_y = j;
        }
        if cy[j] > cy[largest_y] {
            large_y = largest_y;
            largest_y = j;
        } else if cy[j] > cy[large_y] {
            large_y = j;
        }
    }
    c.x[0] -= cx[smallest_x];
    c.x[0] -= cx[small_x];
    c.x[0] -= cx[large_x];
    c.x[0] -= cx[largest_x];
    c.y[0] -= cy[smallest_y];
    c.y[0] -= cy[small_y];
    c.y[0] -= cy[large_y];
    c.y[0] -= cy[largest_y];
    c.x[0] /= NOTCHES as f32 - 4.0;
    c.y[0] /= NOTCHES as f32 - 4.0;

    for i in 0..NOTCHES {
        let dx = c.x[i + 1] - c.x[0];
        let dy = c.y[i + 1] - c.y[0];
        let mag = ((dx * dx + dy * dy) as f64).sqrt() as f32;
        if (mag as f64) < 0.02 && i % 2 != 0 {
            let prev = ((i + NOTCHES - 1) % NOTCHES) + 1;
            let next = ((i + 1) % NOTCHES) + 1;
            c.x[i + 1] = ((c.x[prev] + c.x[next]) as f64 / 2.0) as f32;
            c.y[i + 1] = ((c.y[prev] + c.y[next]) as f64 / 2.0) as f32;
            c.notch_x[i + 1] = ((c.notch_x[prev] + c.notch_x[next]) as f64 / 2.0) as f32;
            c.notch_y[i + 1] = ((c.notch_y[prev] + c.notch_y[next]) as f64 / 2.0) as f32;
            status[i] = NotchStatus::TertInactive;
        } else {
            status[i] = NOTCH_STATUS_DEFAULTS[i];
        }
    }
    c
}

/// `legalizeNotch`: limits a notch's angle so the regions beside it stretch or compress by at
/// most a set factor (tighter for the diagonals), with leniency for modder notches in the
/// deadzone beside a cardinal.
pub fn legalize_notch(index: usize, measured: &[f32; NOTCHES], angles: &mut [f32; NOTCHES]) {
    let n = NOTCHES as i32;
    let i = index as i32;
    let mut is_diagonal = false;
    let mut prev = ((i - 1 + n) % n) as usize;
    let mut next = ((i + 1) % n) as usize;
    if (i - 2) % 4 == 0 {
        prev = ((i - 2 + n) % n) as usize;
        next = ((i + 2) % n) as usize;
        is_diagonal = true;
    }
    let prev_angle = angles[prev];
    let mut next_angle = angles[next];
    if next_angle < prev_angle {
        next_angle = (next_angle as f64 + 2.0 * PI) as f32;
    }
    let prev_meas = measured[prev];
    let this_meas = measured[index];
    let mut next_meas = measured[next];
    if next_meas < this_meas {
        next_meas = (next_meas as f64 + 2.0 * PI) as f32;
    }

    let (cmp_amt, str_amt): (f32, f32) = if is_diagonal {
        (0.769_f64 as f32, 1.3_f64 as f32)
    } else {
        (0.5, 2.0)
    };

    let lower_compress = prev_angle + cmp_amt * (this_meas - prev_meas);
    let mut lower_stretch = next_angle - str_amt * (next_meas - this_meas);
    let upper_compress = next_angle - cmp_amt * (next_meas - this_meas);
    let mut upper_stretch = prev_angle + str_amt * (this_meas - prev_meas);

    let min_threshold = (0.1500 / 0.9750) as f32;
    let deadzone_limit = (0.2875 / 0.9500) as f32;
    let deadzone_plus = (0.3250 / 0.9375) as f32;
    let before = this_meas - prev_meas;
    if prev % 4 == 0 && !is_diagonal && before > min_threshold && before < deadzone_limit {
        upper_stretch = (prev_angle as f64
            + fmax(
                (str_amt * (this_meas - prev_meas)) as f64,
                deadzone_plus as f64,
            )) as f32;
    }
    let after = next_meas - this_meas;
    if next % 4 == 0 && !is_diagonal && after > min_threshold && after < deadzone_limit {
        lower_stretch = (next_angle as f64
            - fmax(
                (str_amt * (next_meas - this_meas)) as f64,
                deadzone_plus as f64,
            )) as f32;
    }

    let lower_distort = fmax(lower_compress as f64, lower_stretch as f64) as f32;
    let mut upper_distort = fmin(upper_compress as f64, upper_stretch as f64) as f32;
    if upper_distort < lower_distort {
        upper_distort = (upper_distort as f64 + 2.0 * PI) as f32;
    }
    let lower = lower_distort;
    let mut upper = upper_distort;
    if upper < lower {
        upper = (upper as f64 + 2.0 * PI) as f32;
    }
    angles[index] = fmax(angles[index] as f64, lower as f64) as f32;
    angles[index] = fmin(angles[index] as f64, upper as f64) as f32;
}

/// `legalizeNotches`: the notches from calibration step `step` (32 or more) to the last.
pub fn legalize_notches(step: i32, measured: &[f32; NOTCHES], angles: &mut [f32; NOTCHES]) {
    for i in step..44 {
        let index = NOTCH_ADJ_ORDER[(i - CAL_POINTS as i32) as usize];
        legalize_notch(index, measured, angles);
    }
}

/// `adjustNotch`'s angle change: during notch adjustment (`step` 32 to 43), turns the notch
/// clockwise or counterclockwise by `loop_delta` (milliseconds) times 0.075 milliradians, or
/// back to its measured angle, then legalizes it and those after it. Whether it changed.
#[allow(clippy::too_many_arguments)]
pub fn adjust_notch(
    step: i32,
    loop_delta: f32,
    measured: &[f32; NOTCHES],
    angles: &mut [f32; NOTCHES],
    status: &[NotchStatus; NOTCHES],
    cw: bool,
    ccw: bool,
    reset: bool,
) -> bool {
    let index = NOTCH_ADJ_ORDER[(step - CAL_POINTS as i32) as usize];
    if status[index] == NotchStatus::TertInactive {
        return false;
    }
    let delta = loop_delta as f64 * 0.000075;
    if cw && !ccw {
        angles[index] = (angles[index] as f64 - delta) as f32;
    } else if ccw && !cw {
        angles[index] = (angles[index] as f64 + delta) as f32;
    } else if reset {
        angles[index] = measured[index];
    } else {
        return false;
    }
    legalize_notches(step, measured, angles);
    true
}

/// The notch a calibration step shows as the target: during collection the default angle of
/// the notch measured (none for a centre step), during notch adjustment the measured angle of
/// the notch adjusted (`displayNotch`, `adjustNotch`).
pub fn target_notch(step: i32, measured: &[f32; NOTCHES]) -> Option<(usize, f32)> {
    if (0..CAL_POINTS as i32).contains(&step) {
        let point = CAL_ORDER[step as usize];
        (point % 2 == 1).then(|| (point / 2, NOTCH_ANGLE_DEFAULTS[point / 2]))
    } else if (CAL_POINTS as i32..CAL_POINTS as i32 + ADJ_NOTCHES as i32).contains(&step) {
        let index = NOTCH_ADJ_ORDER[(step - CAL_POINTS as i32) as usize];
        Some((index, measured[index]))
    } else {
        None
    }
}

/// `linearizeCal`: fits the cubic that maps the centre, the cardinals and the diagonals of the
/// cleaned points to their ideal coordinates, offset so the centre is 0, into `p`'s fit; the
/// points linearized by it. Points with the centre at exactly 0 (nothing measured) leave `p`
/// as it was and zero some of the outputs, as upstream's does.
pub fn linearize_cal(
    in_x: &[f32; NOTCHES + 1],
    in_y: &[f32; NOTCHES + 1],
    p: &mut StickParams,
) -> ([f32; NOTCHES + 1], [f32; NOTCHES + 1]) {
    let (mut out_x, mut out_y) = (*in_x, *in_y);
    if in_x[0] == 0.0 && in_y[0] == 0.0 {
        for i in [0, 1, 3, 4, 5] {
            out_x[i] = 0.0;
            out_y[i] = 0.0;
        }
        return (out_x, out_y);
    }
    let avg = |a: f32, b: f32| (a + b) as f64 / 2.0;
    let fit_x = [
        in_x[9] as f64,
        avg(in_x[7], in_x[11]),
        in_x[0] as f64,
        avg(in_x[3], in_x[15]),
        in_x[1] as f64,
    ];
    let fit_y = [
        in_y[13] as f64,
        avg(in_y[11], in_y[15]),
        in_y[0] as f64,
        avg(in_y[7], in_y[3]),
        in_y[5] as f64,
    ];
    let output = [27.5, 53.2537879754, 127.5, 201.7462120246, 227.5];
    let mut cx = [0f64; FIT_ORDER + 1];
    let mut cy = [0f64; FIT_ORDER + 1];
    // Five points for a cubic: these never fail.
    let _ = fit_curve(FIT_ORDER, &fit_x, &output, FIT_ORDER + 1, &mut cx);
    let _ = fit_curve(FIT_ORDER, &fit_y, &output, FIT_ORDER + 1, &mut cy);
    for i in 0..FIT_ORDER + 1 {
        p.fit_coeffs_x[i] = cx[i] as f32;
        p.fit_coeffs_y[i] = cy[i] as f32;
    }
    let x_zero = linearize(fit_x[2] as f32, &p.fit_coeffs_x);
    let y_zero = linearize(fit_y[2] as f32, &p.fit_coeffs_y);
    p.fit_coeffs_x[3] -= x_zero;
    p.fit_coeffs_y[3] -= y_zero;
    for i in 0..=NOTCHES {
        out_x[i] = linearize(in_x[i], &p.fit_coeffs_x);
        out_y[i] = linearize(in_y[i], &p.fit_coeffs_y);
    }
    (out_x, out_y)
}

type M3 = [[f32; 3]; 3];

/// `inverse`.
fn inverse(m: &M3) -> M3 {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let invdet = 1.0 / det;
    [
        [
            (m[1][1] * m[2][2] - m[2][1] * m[1][2]) * invdet,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * invdet,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * invdet,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * invdet,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * invdet,
            (m[1][0] * m[0][2] - m[0][0] * m[1][2]) * invdet,
        ],
        [
            (m[1][0] * m[2][1] - m[2][0] * m[1][1]) * invdet,
            (m[2][0] * m[0][1] - m[0][0] * m[2][1]) * invdet,
            (m[0][0] * m[1][1] - m[1][0] * m[0][1]) * invdet,
        ],
    ]
}

/// `matrixMatrixMult`.
fn mul(l: &M3, r: &M3) -> M3 {
    let mut out = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                out[i][j] += l[i][k] * r[k][j];
            }
        }
    }
    out
}

/// `notchCalibrate`: each region's affine transform, from the centre and the region's two
/// notches to where they should map, and the regions' boundary angles.
pub fn notch_calibrate(
    x_in: &[f32; NOTCHES + 1],
    y_in: &[f32; NOTCHES + 1],
    x_out: &[f32; NOTCHES + 1],
    y_out: &[f32; NOTCHES + 1],
    p: &mut StickParams,
) {
    let regions = NOTCHES;
    for i in 1..=regions {
        let j = if i == regions { 1 } else { i + 1 };
        let points_in = [
            [x_in[0], x_in[i], x_in[j]],
            [y_in[0], y_in[i], y_in[j]],
            [1.0, 1.0, 1.0],
        ];
        let points_out = [
            [x_out[0], x_out[i], x_out[j]],
            [y_out[0], y_out[i], y_out[j]],
            [1.0, 1.0, 1.0],
        ];
        let a = mul(&points_out, &inverse(&points_in));
        for r in 0..2 {
            for k in 0..2 {
                p.affine_coeffs[i - 1][r * 2 + k] = a[r][k];
            }
        }
        p.boundary_angles[i - 1] = (y_in[i] - y_in[0]).atan2(x_in[i] - x_in[0]);
        if p.boundary_angles[i - 1] < p.boundary_angles[0] {
            p.boundary_angles[i - 1] = (p.boundary_angles[i - 1] as f64 + PI * 2.0) as f32;
        }
    }
}

/// A stick's parameters from stored calibration points and notch angles, as PhobGCC derives
/// them at power-on and after calibration (`cleanCalPoints`, `linearizeCal`, `notchCalibrate`).
pub fn params_from(
    points_x: &[f32; CAL_POINTS],
    points_y: &[f32; CAL_POINTS],
    notch_angles: &[f32; NOTCHES],
    p: &mut StickParams,
) -> [NotchStatus; NOTCHES] {
    let mut status = [NotchStatus::default(); NOTCHES];
    let c = clean_cal_points(points_x, points_y, notch_angles, &mut status);
    let (lx, ly) = linearize_cal(&c.x, &c.y, p);
    notch_calibrate(&lx, &ly, &c.notch_x, &c.notch_y, p);
    status
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A made-up stock stick: centre near 0.5 of full scale, the notches at `reach` raw units
    /// (of 1) along each default angle, with the modder notches unmeasured (at the centre).
    pub fn ideal_points(reach: f32) -> ([f32; CAL_POINTS], [f32; CAL_POINTS]) {
        let mut x = [0.5f32; CAL_POINTS];
        let mut y = [0.5f32; CAL_POINTS];
        for n in 0..NOTCHES {
            if n % 2 == 0 {
                let a = NOTCH_ANGLE_DEFAULTS[n];
                x[n * 2 + 1] = 0.5 + reach * a.cos();
                y[n * 2 + 1] = 0.5 + reach * a.sin();
            }
        }
        (x, y)
    }

    #[test]
    fn notch_angles_and_stick_values() {
        assert_eq!(NOTCH_ANGLE_DEFAULTS[4], (PI / 2.0) as f32);
        let (x, y) = calc_stick_values(0.0);
        assert_eq!(y, 0.0);
        assert!((x - 100.0).abs() < 1e-3);
        let a = angle_on_sphere(calc_stick_values(1.0).0, calc_stick_values(1.0).1);
        assert!((a - 1.0).abs() < 1e-4);
    }

    #[test]
    fn calibration_maps_notches_to_their_coordinates() {
        let (px, py) = ideal_points(0.3);
        let mut p = StickParams::default();
        let status = params_from(&px, &py, &NOTCH_ANGLE_DEFAULTS, &mut p);
        assert_eq!(status[1], NotchStatus::TertInactive);
        assert_eq!(status[0], NotchStatus::Cardinal);
        let s = Settings::default();
        // The right notch maps to (100, 0), snapped; the up-right one to its diagonal.
        let x = linearize(px[1], &p.fit_coeffs_x);
        let y = linearize(py[1], &p.fit_coeffs_y);
        let (ox, oy) = notch_remap(x, y, &p, -1, &s, WhichStick::A);
        assert!((ox - 100.0).abs() < 0.01 && oy == 0.0, "{ox} {oy}");
        let x = linearize(px[5], &p.fit_coeffs_x);
        let y = linearize(py[5], &p.fit_coeffs_y);
        let (ox, oy) = notch_remap(x, y, &p, -1, &s, WhichStick::A);
        assert!(
            (ox - 74.0).abs() < 0.05 && (oy - 74.0).abs() < 0.05,
            "{ox} {oy}"
        );
        // The centre maps to 0.
        let x = linearize(0.5, &p.fit_coeffs_x);
        let y = linearize(0.5, &p.fit_coeffs_y);
        assert_eq!(notch_remap(x, y, &p, -1, &s, WhichStick::A), (0.0, 0.0));
    }

    #[test]
    fn cardinal_snapping_truncates_as_the_rp2040_build() {
        // An identity calibration: one region covering everything.
        let mut p = StickParams::default();
        for a in &mut p.affine_coeffs {
            *a = [1.0, 0.0, 0.0, 1.0];
        }
        p.boundary_angles = [-4.0; NOTCHES];
        let s = Settings::default();
        let r = |x, y| notch_remap(x, y, &p, -1, &s, WhichStick::A);
        assert_eq!(r(6.9, 85.0), (0.0, 85.0));
        // |x| 7.2 truncates to 7: not under 6.5.
        assert_eq!(r(7.2, 85.0), (7.2, 85.0));
        // y 79.7 truncates to 79: not at the rim, so no snapping.
        assert_eq!(r(5.0, 79.7), (5.0, 79.7));
        assert_eq!(r(2.9, -2.9), (0.0, 0.0));
        // During calibration nothing snaps.
        assert_eq!(notch_remap(2.0, 2.0, &p, 3, &s, WhichStick::A), (2.0, 2.0));
    }
}
