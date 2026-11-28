// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea: what
// PhobGCC keeps in its storage (the settings, and each stick's calibration points and notch
// angles), as a melee-hd profile. See lib.rs for its authors. Distributed under the GNU General
// Public License version 3 only.

//! A named profile: the settings and each stick's calibration, kept as `key = value` lines.

use crate::settings::Settings;
use crate::stick::{
    CAL_POINTS, NOTCH_ANGLE_DEFAULTS, NOTCHES, NotchStatus, StickParams, params_from,
};

/// A stick's calibration as PhobGCC stores it: the 32 measured points and the notch angles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickCal {
    pub points_x: [f32; CAL_POINTS],
    pub points_y: [f32; CAL_POINTS],
    pub notch_angles: [f32; NOTCHES],
}

impl Default for StickCal {
    /// What a hard reset stores (`_defaultCalPoints`, the default notch angles).
    fn default() -> StickCal {
        StickCal {
            points_x: [0.0; CAL_POINTS],
            points_y: [0.0; CAL_POINTS],
            notch_angles: NOTCH_ANGLE_DEFAULTS,
        }
    }
}

impl StickCal {
    /// The stick's parameters, as PhobGCC derives them at power-on (`readEEPROM`).
    pub fn params(&self) -> (StickParams, [NotchStatus; NOTCHES]) {
        let mut p = StickParams::default();
        let status = params_from(&self.points_x, &self.points_y, &self.notch_angles, &mut p);
        (p, status)
    }
}

/// A profile: its settings and the sticks' calibrations (none for a stick never calibrated).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub settings: Settings,
    pub a: Option<StickCal>,
    pub c: Option<StickCal>,
}

const HEADER: &str = "# melee-hd PhobGCC profile";

fn floats(v: &[f32]) -> String {
    v.iter()
        .map(|f| format!("{f:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_floats<const N: usize>(v: &str) -> Option<[f32; N]> {
    let mut out = [0f32; N];
    let mut it = v.split_whitespace();
    for o in &mut out {
        *o = it.next()?.parse().ok()?;
    }
    it.next().is_none().then_some(out)
}

impl Profile {
    /// The profile as text. Floats are written so they read back exactly.
    pub fn to_text(&self) -> String {
        let mut out = format!("{HEADER}\n");
        let mut s = self.settings;
        for (key, v, _) in s.fields() {
            out += &format!("{key} = {v}\n");
        }
        for (name, cal) in [("a", &self.a), ("c", &self.c)] {
            if let Some(cal) = cal {
                out += &format!("{name}-points-x = {}\n", floats(&cal.points_x));
                out += &format!("{name}-points-y = {}\n", floats(&cal.points_y));
                out += &format!("{name}-notch-angles = {}\n", floats(&cal.notch_angles));
            }
        }
        out
    }

    /// A profile from text: unknown or unreadable lines are skipped, settings out of range go
    /// back to their defaults, and a stick's calibration counts only when all three of its
    /// lines read.
    pub fn from_text(text: &str) -> Profile {
        let mut p = Profile::default();
        let mut cal = [[None, None, None], [None, None, None]];
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            for (k, v, _) in p.settings.fields() {
                if k == key
                    && let Ok(n) = value.parse()
                {
                    *v = n;
                }
            }
            for (i, stick) in ["a", "c"].into_iter().enumerate() {
                let Some(rest) = key.strip_prefix(stick).and_then(|r| r.strip_prefix('-')) else {
                    continue;
                };
                match rest {
                    "points-x" => cal[i][0] = parse_floats::<CAL_POINTS>(value).map(|a| a.to_vec()),
                    "points-y" => cal[i][1] = parse_floats::<CAL_POINTS>(value).map(|a| a.to_vec()),
                    "notch-angles" => {
                        cal[i][2] = parse_floats::<NOTCHES>(value).map(|a| a.to_vec())
                    }
                    _ => {}
                }
            }
        }
        p.settings = p.settings.legalized();
        let [a, c] = cal.map(|[x, y, n]| {
            Some(StickCal {
                points_x: x?.try_into().ok()?,
                points_y: y?.try_into().ok()?,
                notch_angles: n?.try_into().ok()?,
            })
        });
        p.a = a;
        p.c = c;
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_exactly() {
        let mut cal = StickCal::default();
        for (i, v) in cal.points_x.iter_mut().enumerate() {
            *v = 0.1 + i as f32 / 7.0;
        }
        cal.notch_angles[3] = 1.234_567_9;
        let p = Profile {
            settings: Settings {
                x_snapback: -3,
                l_mode: 6,
                ..Settings::default()
            },
            a: Some(cal),
            c: None,
        };
        let back = Profile::from_text(&p.to_text());
        assert_eq!(back, p);
        // A stick missing one of its lines is uncalibrated.
        let text = p.to_text().replace("a-notch-angles", "a-notch-angels");
        assert_eq!(Profile::from_text(&text).a, None);
    }
}
