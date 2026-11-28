// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea: the
// trigger output of `processButtons` in `PhobGCC/common/phobGCC.h` and the RP2040 board's
// `readLa`/`readRa` (`PhobGCC/rp2040/include/readHardware.h`). See lib.rs for its authors.
// Distributed under the GNU General Public License version 3 only.

//! The triggers' modes and the button output.

use crate::c::{fmax, fmin};
use crate::settings::Settings;

// PADStatus buttons.
pub const L: u16 = 0x0040;
pub const R: u16 = 0x0020;
pub const A: u16 = 0x0100;
pub const START: u16 = 0x1000;

/// `readLa` and `readRa`: the slider's reading (0 to 255 here, the 12-bit reading over 16 on
/// the board) with its small deadzone, scaled and capped at 255.
pub fn read_trigger(value: f32, scale: f32) -> i32 {
    let temp = if value < 3.0 { 0.0 } else { value };
    let initial = 0;
    fmin(
        255.0,
        fmax(0.0, (temp - initial as f32) as f64) * scale as f64,
    ) as i32
}

/// One trigger's mode (`lConfig` or `rConfig`) on its digital button and slider: the digital
/// button and the analog value sent.
fn trigger(mode: i32, offset: i32, digital: bool, value: f32) -> (bool, u8) {
    let read = |scale: f32| read_trigger(value, scale) as u8;
    let capped = |gain: f64| fmin(offset as f64, gain * read_trigger(value, 1.0) as f64) as u8;
    match mode {
        1 => (digital, 0),
        2 => (false, read(1.0)),
        3 => (digital, if digital { 255 } else { capped(1.0) }),
        4 => (false, if digital { offset.min(255) as u8 } else { 0 }),
        5 => (digital, if digital { offset.min(255) as u8 } else { 0 }),
        6 => {
            let scale = 0.0112f32 * offset as f32 + 0.4494f32;
            (digital, read(scale))
        }
        7 => (false, capped(1.0)),
        8 => (digital, if digital { 255 } else { capped(2.5) }),
        _ => (digital, read(1.0)),
    }
}

/// The buttons and triggers sent for the controller's `buttons` (PADStatus bits) and its
/// sliders' readings: each trigger's mode, then L, R, A and Start held together sent as they
/// are with both sliders full, so the console's reset always works.
pub fn buttons(s: &Settings, buttons: u16, sliders: [f32; 2]) -> (u16, [u8; 2]) {
    let (l, la) = trigger(s.l_mode, s.l_offset, buttons & L != 0, sliders[0]);
    let (r, ra) = trigger(s.r_mode, s.r_offset, buttons & R != 0, sliders[1]);
    let mut out = buttons & !(L | R);
    if l {
        out |= L;
    }
    if r {
        out |= R;
    }
    let mut analog = [la, ra];
    if buttons & (L | R | A | START) == L | R | A | START {
        out |= L | R | A | START;
        analog = [255, 255];
    }
    (out, analog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(mode: i32, offset: i32) -> Settings {
        Settings {
            l_mode: mode,
            l_offset: offset,
            ..Settings::default()
        }
    }

    /// The L trigger's output in `mode` for a press: (digital, analog in).
    fn l(mode: i32, offset: i32, digital: bool, value: f32) -> (bool, u8) {
        let (b, a) = buttons(&s(mode, offset), if digital { L } else { 0 }, [value, 0.0]);
        (b & L != 0, a[0])
    }

    #[test]
    fn mode_0_default() {
        assert_eq!(l(0, 49, false, 100.0), (false, 100));
        assert_eq!(l(0, 49, true, 200.0), (true, 200));
        // The deadzone below 3.
        assert_eq!(l(0, 49, false, 2.0), (false, 0));
        assert_eq!(l(0, 49, false, 3.0), (false, 3));
    }

    #[test]
    fn mode_1_digital_only() {
        assert_eq!(l(1, 49, true, 200.0), (true, 0));
        assert_eq!(l(1, 49, false, 120.0), (false, 0));
    }

    #[test]
    fn mode_2_analog_only() {
        assert_eq!(l(2, 49, true, 200.0), (false, 200));
    }

    #[test]
    fn mode_3_trigger_plug() {
        assert_eq!(l(3, 80, false, 200.0), (false, 80));
        assert_eq!(l(3, 80, false, 50.0), (false, 50));
        assert_eq!(l(3, 80, true, 200.0), (true, 255));
    }

    #[test]
    fn mode_4_digital_to_analog() {
        assert_eq!(l(4, 120, true, 10.0), (false, 120));
        assert_eq!(l(4, 120, false, 200.0), (false, 0));
    }

    #[test]
    fn mode_5_digital_to_analog_and_digital() {
        assert_eq!(l(5, 120, true, 10.0), (true, 120));
        assert_eq!(l(5, 120, false, 200.0), (false, 0));
    }

    #[test]
    fn mode_6_scaled() {
        // Offset 49: 0.0112 * 49 + 0.4494 = 0.9982.
        assert_eq!(l(6, 49, false, 100.0), (false, 99));
        // Offset 227: about 2.99, capped at 255.
        assert_eq!(l(6, 227, false, 100.0), (false, 255));
        assert_eq!(l(6, 227, false, 50.0), (false, 149));
    }

    #[test]
    fn mode_7_plug_without_digital() {
        assert_eq!(l(7, 80, true, 200.0), (false, 80));
    }

    #[test]
    fn mode_8_plug_high_gain() {
        assert_eq!(l(8, 80, false, 20.0), (false, 50));
        assert_eq!(l(8, 80, false, 40.0), (false, 80));
        assert_eq!(l(8, 80, true, 40.0), (true, 255));
    }

    #[test]
    fn lras_always_resets() {
        let (b, a) = buttons(&s(4, 100), L | R | A | START, [0.0, 0.0]);
        assert_eq!(b, L | R | A | START);
        assert_eq!(a, [255, 255]);
    }
}
