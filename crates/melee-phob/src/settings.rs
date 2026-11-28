// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// the settings of `PhobGCC/common/structsAndEnums.h` (`ControlConfig`, `FilterGains`) with the
// ranges and defaults `phobGCC.h` gives them. See lib.rs for its authors. Distributed under the
// GNU General Public License version 3 only.

//! A controller's settings, as PhobGCC keeps them in its `ControlConfig`, with their ranges.

use std::ops::RangeInclusive;

/// Which stick (`WhichStick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WhichStick {
    A,
    C,
}

/// The settings PhobGCC's processing reads (`ControlConfig`'s that melee-hd ports).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The control stick's snapback filter per axis (`xSnapback`, `ySnapback`): 0 off, above 0
    /// the filter, below 0 a low-pass instead.
    pub x_snapback: i32,
    pub y_snapback: i32,
    /// Smoothing per axis of each stick (`axSmoothing` and the rest).
    pub ax_smoothing: i32,
    pub ay_smoothing: i32,
    pub cx_smoothing: i32,
    pub cy_smoothing: i32,
    /// Waveshaping per axis of each stick; below 0 it also reacts to acceleration.
    pub ax_waveshaping: i32,
    pub ay_waveshaping: i32,
    pub cx_waveshaping: i32,
    pub cy_waveshaping: i32,
    /// Cardinal snapping (`astickCardinalSnapping`, `cstickCardinalSnapping`).
    pub a_cardinal_snapping: i32,
    pub c_cardinal_snapping: i32,
    /// The analog scaler in percent (`astickAnalogScaler`, `cstickAnalogScaler`).
    pub a_analog_scaler: i32,
    pub c_analog_scaler: i32,
    /// The triggers' modes (`lConfig`, `rConfig`, [`TRIGGER_MODES`]) and offsets.
    pub l_mode: i32,
    pub r_mode: i32,
    pub l_offset: i32,
    pub r_offset: i32,
}

pub const SNAPBACK: RangeInclusive<i32> = -10..=10;
pub const SMOOTHING: RangeInclusive<i32> = 0..=18;
pub const WAVESHAPING: RangeInclusive<i32> = -24..=24;
pub const CARDINAL_SNAPPING: RangeInclusive<i32> = -2..=6;
pub const ANALOG_SCALER: RangeInclusive<i32> = 90..=110;
pub const TRIGGER_MODE: RangeInclusive<i32> = 0..=8;
pub const TRIGGER_OFFSET: RangeInclusive<i32> = 49..=227;

/// The trigger modes' names, by mode, in melee-hd's words.
pub const TRIGGER_MODES: [&str; 9] = [
    "Analog and digital",
    "Digital only",
    "Analog only",
    "Analog up to the offset, digital at full",
    "Digital press gives the offset, no digital",
    "Digital press gives the offset and digital",
    "Analog scaled by the offset",
    "Analog up to the offset, no digital",
    "Analog up to the offset at 2.5x, digital at full",
];

impl Default for Settings {
    /// What PhobGCC's reset sets (`resetDefaults`).
    fn default() -> Settings {
        Settings {
            x_snapback: 4,
            y_snapback: 4,
            ax_smoothing: 0,
            ay_smoothing: 0,
            cx_smoothing: 5,
            cy_smoothing: 5,
            ax_waveshaping: 0,
            ay_waveshaping: 0,
            cx_waveshaping: 0,
            cy_waveshaping: 0,
            a_cardinal_snapping: 6,
            c_cardinal_snapping: 6,
            a_analog_scaler: 100,
            c_analog_scaler: 100,
            l_mode: 0,
            r_mode: 0,
            l_offset: 49,
            r_offset: 49,
        }
    }
}

impl Settings {
    /// Each setting with its key in a profile, a mutable reference to it, and its range.
    pub fn fields(&mut self) -> [(&'static str, &mut i32, RangeInclusive<i32>); 18] {
        [
            ("x-snapback", &mut self.x_snapback, SNAPBACK),
            ("y-snapback", &mut self.y_snapback, SNAPBACK),
            ("ax-smoothing", &mut self.ax_smoothing, SMOOTHING),
            ("ay-smoothing", &mut self.ay_smoothing, SMOOTHING),
            ("cx-smoothing", &mut self.cx_smoothing, SMOOTHING),
            ("cy-smoothing", &mut self.cy_smoothing, SMOOTHING),
            ("ax-waveshaping", &mut self.ax_waveshaping, WAVESHAPING),
            ("ay-waveshaping", &mut self.ay_waveshaping, WAVESHAPING),
            ("cx-waveshaping", &mut self.cx_waveshaping, WAVESHAPING),
            ("cy-waveshaping", &mut self.cy_waveshaping, WAVESHAPING),
            (
                "a-cardinal-snapping",
                &mut self.a_cardinal_snapping,
                CARDINAL_SNAPPING,
            ),
            (
                "c-cardinal-snapping",
                &mut self.c_cardinal_snapping,
                CARDINAL_SNAPPING,
            ),
            ("a-analog-scaler", &mut self.a_analog_scaler, ANALOG_SCALER),
            ("c-analog-scaler", &mut self.c_analog_scaler, ANALOG_SCALER),
            ("l-mode", &mut self.l_mode, TRIGGER_MODE),
            ("r-mode", &mut self.r_mode, TRIGGER_MODE),
            ("l-offset", &mut self.l_offset, TRIGGER_OFFSET),
            ("r-offset", &mut self.r_offset, TRIGGER_OFFSET),
        ]
    }

    /// Every setting kept in its range: one out of range goes back to its default, as
    /// PhobGCC's `readEEPROM` treats a bad stored value (the trigger offsets clamp instead).
    pub fn legalized(mut self) -> Settings {
        let mut d = Settings::default();
        for ((key, v, range), (_, dv, _)) in self.fields().into_iter().zip(d.fields()) {
            if !range.contains(v) {
                *v = if key.ends_with("-offset") {
                    (*v).clamp(*range.start(), *range.end())
                } else {
                    *dv
                };
            }
        }
        self
    }
}

/// The filters' gains (`FilterGains`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FilterGains {
    pub max_stick: f32,
    pub x_vel_decay: f32,
    pub y_vel_decay: f32,
    pub x_vel_pos_factor: f32,
    pub y_vel_pos_factor: f32,
    pub x_vel_damp: f32,
    pub y_vel_damp: f32,
    pub vel_thresh: f32,
    pub accel_thresh: f32,
    pub x_smoothing: f32,
    pub y_smoothing: f32,
    pub cx_smoothing: f32,
    pub cy_smoothing: f32,
}

/// `_gains` as `phobGCC.h` starts them, before `recomputeGains` (its double literals rounded to
/// float).
pub const GAINS: FilterGains = FilterGains {
    max_stick: 100.0,
    x_vel_decay: 0.1_f64 as f32,
    y_vel_decay: 0.1_f64 as f32,
    x_vel_pos_factor: 0.01_f64 as f32,
    y_vel_pos_factor: 0.01_f64 as f32,
    x_vel_damp: 0.125,
    y_vel_damp: 0.125,
    vel_thresh: 1.00,
    accel_thresh: 3.00,
    x_smoothing: 0.0,
    y_smoothing: 0.0,
    cx_smoothing: 0.0,
    cy_smoothing: 0.0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legalized_restores_defaults_and_clamps_offsets() {
        let s = Settings {
            x_snapback: 11,
            ax_smoothing: -1,
            l_offset: 300,
            r_offset: 10,
            a_analog_scaler: 95,
            ..Settings::default()
        }
        .legalized();
        assert_eq!(s.x_snapback, 4);
        assert_eq!(s.ax_smoothing, 0);
        assert_eq!((s.l_offset, s.r_offset), (227, 49));
        assert_eq!(s.a_analog_scaler, 95);
    }
}
