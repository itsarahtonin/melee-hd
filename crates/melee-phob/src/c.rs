// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea;
// see lib.rs for its authors. Distributed under the GNU General Public License version 3 only.

//! What upstream's unqualified math calls mean on the RP2040 build. Its translation units include
//! `<cmath>` and `<algorithm>` (libstdc++) before PhobGCC's headers and no `<stdlib.h>` or
//! `<math.h>` wrapper, so the global namespace holds only C's functions: `abs` is
//! `int abs(int)`, and `pow`, `sqrt`, `fmin`, `fmax` and `round` take and return `double`.

/// `M_PI`.
pub const PI: f64 = std::f64::consts::PI;

/// `abs(x)` with `x` a float: converted to `int` (truncated toward zero) first.
pub fn abs_int(v: f32) -> i32 {
    (v as i32).wrapping_abs()
}

/// `fmin(a, b)` in double, as C's.
pub fn fmin(a: f64, b: f64) -> f64 {
    a.min(b)
}

/// `fmax(a, b)` in double, as C's.
pub fn fmax(a: f64, b: f64) -> f64 {
    a.max(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abs_truncates() {
        assert_eq!(abs_int(79.9), 79);
        assert_eq!(abs_int(-79.9), 79);
        assert_eq!(abs_int(-0.4), 0);
        assert_eq!(abs_int(3.0), 3);
    }
}
