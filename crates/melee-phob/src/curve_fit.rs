// SPDX-License-Identifier: GPL-3.0-only
//
// Part of melee-hd's port of PhobGCC-SW (https://github.com/PhobGCC/PhobGCC-SW) at b4f175ea:
// `PhobGCC/common/curveFitting.h`, itself "created by Rowan Easter-Robinson, August 23, 2018.
// Released into the public domain." See lib.rs for PhobGCC's authors. Distributed under the GNU
// General Public License version 3 only.

//! A least-squares polynomial fit, solved by Cramer's rule (`fitCurve`), in double precision.

/// The largest number of coefficients (`MAX_ORDER`).
pub const MAX_ORDER: usize = 20;

/// Why a fit wasn't made (`curveFitERROR`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitError {
    OrderAndNCoeffsDoNotMatch = -1,
    OrderIncorrect = -2,
    NPointsIncorrect = -3,
}

/// `trianglize`: Gaussian elimination over the rows `m` points at, swapping the pointers.
/// Upstream's pivot search starts at row 0 rather than row `i`, and swaps (flipping the sign)
/// whenever the row it finds isn't row 0, itself included: both kept.
fn trianglize(a: &mut [f64], rows: &mut [usize], n: usize) -> i32 {
    let mut sign = 1;
    for i in 0..n {
        let mut max = 0;
        for row in i..n {
            if a[rows[row] * n + i].abs() > a[rows[max] * n + i].abs() {
                max = row;
            }
        }
        if max != 0 {
            sign = -sign;
            rows.swap(i, max);
        }
        if a[rows[i] * n + i] == 0.0 {
            return 0;
        }
        for row in i + 1..n {
            let r = a[rows[row] * n + i] / a[rows[i] * n + i];
            if r == 0.0 {
                continue;
            }
            for col in i..n {
                a[rows[row] * n + col] -= a[rows[i] * n + col] * r;
            }
        }
    }
    sign
}

/// `det`: the determinant of the `n` by `n` matrix `a` (row-major), which it alters.
fn det(a: &mut [f64], n: usize) -> f64 {
    let mut rows = [0usize; MAX_ORDER];
    for (i, r) in rows.iter_mut().enumerate().take(n) {
        *r = i;
    }
    let sign = trianglize(a, &mut rows[..n], n);
    if sign == 0 {
        return 0.0;
    }
    let mut p = 1.0;
    for i in 0..n {
        p *= a[rows[i] * n + i];
    }
    p * sign as f64
}

/// `curveFitPower`.
fn power(base: f64, exponent: usize) -> f64 {
    if exponent == 0 {
        1.0
    } else {
        let mut val = base;
        for _ in 1..exponent {
            val *= base;
        }
        val
    }
}

/// `fitCurve` (the overload with x values): the coefficients of the polynomial of `order`
/// through the points, highest power first.
pub fn fit_curve(
    order: usize,
    px: &[f64],
    py: &[f64],
    n_coeffs: usize,
    coeffs: &mut [f64],
) -> Result<(), FitError> {
    if n_coeffs != order + 1 {
        return Err(FitError::OrderAndNCoeffsDoNotMatch);
    }
    if n_coeffs > MAX_ORDER || n_coeffs < 2 {
        return Err(FitError::OrderIncorrect);
    }
    if px.is_empty() {
        return Err(FitError::NPointsIncorrect);
    }
    let mut t = [0f64; MAX_ORDER];
    let mut s = [0f64; MAX_ORDER * 2 + 1];
    for (&x, &y) in px.iter().zip(py) {
        for (j, sj) in s.iter_mut().enumerate().take(n_coeffs * 2 - 1) {
            *sj += power(x, j);
        }
        for (j, tj) in t.iter_mut().enumerate().take(n_coeffs) {
            *tj += y * power(x, j);
        }
    }
    let n = n_coeffs;
    let mut master = [0f64; MAX_ORDER * MAX_ORDER];
    for i in 0..n {
        for j in 0..n {
            master[i * n + j] = s[i + j];
        }
    }
    let mut mat = master;
    let denom = det(&mut mat[..n * n], n);
    mat = master;
    for i in 0..n {
        // `subCol`: column i becomes the right-hand side.
        for (r, &tr) in t.iter().enumerate().take(n) {
            mat[r * n + i] = tr;
        }
        coeffs[n - i - 1] = det(&mut mat[..n * n], n) / denom;
        mat = master;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_cubic() {
        let px = [0.0, 1.0, 2.0, 3.0, 4.0];
        let py: Vec<f64> = px.iter().map(|x| 2.0 * x * x * x - x + 5.0).collect();
        let mut c = [0.0; 4];
        fit_curve(3, &px, &py, 4, &mut c).unwrap();
        for (got, want) in c.iter().zip([2.0, 0.0, -1.0, 5.0]) {
            assert!((got - want).abs() < 1e-9, "{c:?}");
        }
        assert_eq!(
            fit_curve(3, &px, &py, 3, &mut c),
            Err(FitError::OrderAndNCoeffsDoNotMatch)
        );
    }
}
