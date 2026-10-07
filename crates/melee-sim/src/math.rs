//! The game's own math routines, bit for bit: MSL's `sinf`, `cosf`, `tanf` and the inline
//! `sqrtf`, and `lbtrigf`'s `atanf`, `atan2f`, `acosf` and `asinf`.
//!
//! Each follows the instructions MWCC emitted, not just the C: which multiply-adds fused,
//! which operations rounded to single precision, and the `frsqrte` estimate. `gekko-fp` does
//! the arithmetic, so the console's (or Slippi's) rounding follows its mode.

use gekko_fp as fp;

/// `__sincos_on_quadrant` (MSL/math_data.c).
const QUADRANT: [u32; 8] = [
    0x00000000, 0x3F800000, 0x3F800000, 0x00000000, 0x00000000, 0xBF800000, 0xBF800000, 0x00000000,
];
/// `__sincos_poly` (MSL/math_data.c).
const SINCOS_POLY: [u32; 10] = [
    0x366CCFAA, 0x34A5E129, 0xB9AAE275, 0xB8196543, 0x3C81E0ED, 0x3B2335DD, 0xBE9DE9E6, 0xBDA55DE7,
    0x3F800000, 0x3F490FDB,
];
/// `__four_over_pi_m1` as `__sinit_trigf_c` sets it (MSL/trigf.c).
const FOUR_OVER_PI_M1: [u32; 4] = [0x3E800000, 0x3CBE6080, 0x34372200, 0x2DA44152];
/// `2.0f / (f32) M_PI`.
const TWO_OVER_PI: u32 = 0x3F22F983;
/// `__epsilon` in trigf.c.
const SINCOS_EPSILON: u32 = 0x39B504F3;
/// `atanf_lookup` (melee/lb/lbtrigf.c).
const ATAN: [u32; 46] = [
    0x3F800000, 0xBEAAAAAA, 0x3E4CCC81, 0xBE123E7D, 0x3DE21F95, 0xBDAD417C, 0x3D41186D, 0x40DA826B,
    0x404F5958, 0x40000000, 0x3FB925AB, 0x3F95F61A, 0x3F851081, 0x36EF692F, 0x355C1DF9, 0x00000000,
    0x35291D45, 0x00000000, 0x00000000, 0x00000000, 0x3EC90EAA, 0x3F16CBE4, 0x3F490FDA, 0x3F7B53C5,
    0x3F96CBE2, 0x3FAFEDD9, 0x00000000, 0x37185D99, 0x32C59189, 0x33874A9E, 0x353CFA83, 0x348637BD,
    0x35541063, 0x401A8277, 0x3FBF90C7, 0x3F800000, 0x3F2B0DC1, 0x3ED413CD, 0x3E4BAFAF, 0x3516DC59,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
];
/// atanf's split points: 1 + sqrt(2), sqrt(2) - 1.
const SILVER_RATIO: u32 = 0x401A827A;
const SILVER_RATIO_CONJUGATE: u32 = 0x3ED413CD;
/// Mantissa thresholds that pick atanf's interval within an octave.
const ATAN_THRESHOLDS: [i32; 4] = [0x3F08D5B9, 0x3F521801, 0x3F9BF7EC, 0x3FEF789E];
/// `(float) M_PI` and `(float) M_PI_2`.
const PI: u32 = 0x40490FDB;
const PI_2: u32 = 0x3FC90FDB;
/// What lbtrigf returns for a negative square root and for zero (MSL/float.c).
const NAN: u32 = 0x7FFFFFFF;
const INF: u32 = 0x7F800000;

/// A float as a register holds it after `lfs`.
fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

fn reg_bits(bits: u32) -> f64 {
    fp::lfs(bits)
}

/// A register stored with `stfs`.
fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

/// A store to the stack and a load back, as the original does between steps.
fn spill(r: f64) -> f64 {
    fp::lfs(fp::stfs(r))
}

fn sign(x: f32) -> u32 {
    x.to_bits() & 0x8000_0000
}

/// Reduces `x` by quarter turns: the quadrant (0 to 3) and the remainder.
fn reduce(x: f32) -> (usize, f64) {
    let xr = reg(x);
    let z = fp::fmuls(reg_bits(TWO_OVER_PI), xr);
    let n = if sign(x) != 0 {
        fp::fctiwz(fp::fsubs(z, 0.5))
    } else {
        fp::fctiwz(fp::fadds(0.5, z))
    };
    let mut y = fp::fsubs(xr, fp::frsp(n.wrapping_mul(2) as f64));
    for k in FOUR_OVER_PI_M1 {
        y = fp::fmadds(reg_bits(k), xr, y);
    }
    ((n & 3) as usize, y)
}

fn q(i: usize) -> f64 {
    reg_bits(QUADRANT[i])
}

fn p(i: usize) -> f64 {
    reg_bits(SINCOS_POLY[i])
}

/// The even polynomial in `ysq`.
fn cos_poly(ysq: f64) -> f64 {
    let z = fp::fmadds(p(0), ysq, p(2));
    let z = fp::fmadds(ysq, z, p(4));
    let z = fp::fmadds(ysq, z, p(6));
    fp::fmadds(ysq, z, p(8))
}

/// The odd polynomial's factor, before multiplying by `y`, with its last step.
fn sin_poly(ysq: f64, last: fn(f64, f64, f64) -> f64) -> f64 {
    let z = fp::fmadds(p(1), ysq, p(3));
    let z = fp::fmadds(ysq, z, p(5));
    let z = fp::fmadds(ysq, z, p(7));
    last(ysq, z, p(9))
}

/// `sinf`.
#[doc(alias = "sinf")]
#[doc(alias = "sin__Ff")]
pub fn sin(x: f32) -> f32 {
    let (n, y) = reduce(x);
    let n = n << 1;
    if fp::fabs(y) < reg_bits(SINCOS_EPSILON) {
        return mem(fp::fmadds(p(9), fp::fmuls(y, q(n + 1)), q(n)));
    }
    let ysq = fp::fmuls(y, y);
    if n & 2 != 0 {
        mem(fp::fmuls(cos_poly(ysq), q(n)))
    } else {
        mem(fp::fmuls(fp::fmuls(y, sin_poly(ysq, fp::fmadds)), q(n + 1)))
    }
}

/// `cosf`.
#[doc(alias = "cosf")]
#[doc(alias = "cos__Ff")]
pub fn cos(x: f32) -> f32 {
    let (n, y) = reduce(x);
    let n = n << 1;
    if fp::fabs(y) < reg_bits(SINCOS_EPSILON) {
        return mem(fp::fnmsubs(y, q(n), q(n + 1)));
    }
    let ysq = fp::fmuls(y, y);
    if n & 2 != 0 {
        mem(fp::fmuls(fp::fmuls(y, sin_poly(ysq, fp::fnmadds)), q(n)))
    } else {
        mem(fp::fmuls(cos_poly(ysq), q(n + 1)))
    }
}

/// `tanf`.
#[doc(alias = "tanf")]
pub fn tan(x: f32) -> f32 {
    mem(fp::fdivs(reg(sin(x)), reg(cos(x))))
}

/// The inline `sqrtf` of MSL's math_ppc.h: three Newton steps on `frsqrte` in double
/// precision. Zero, negatives and NaN come back as they are.
// NaN takes the original's branches, which the negated comparisons below spell out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "sqrtf__Ff")]
pub fn sqrt(x: f32) -> f32 {
    let xr = reg(x);
    if !(xr > 0.0) {
        return x;
    }
    let mut g = fp::frsqrte(xr);
    for _ in 0..3 {
        let gg = fp::fmul(g, g);
        let half = fp::fmul(0.5, g);
        g = fp::fmul(half, fp::fnmsub(xr, gg, 3.0));
    }
    mem(fp::frsp(fp::fmul(xr, g)))
}

/// `lbVector_sqrtf_accurate`: the inline `sqrtf` with a fourth Newton step.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "lbVector_sqrtf_accurate")]
pub fn sqrt_accurate(x: f32) -> f32 {
    let xr = reg(x);
    if !(xr > 0.0) {
        return x;
    }
    let mut g = fp::frsqrte(xr);
    for _ in 0..4 {
        let gg = fp::fmul(g, g);
        let half = fp::fmul(0.5, g);
        g = fp::fmul(half, fp::fnmsub(xr, gg, 3.0));
    }
    mem(fp::frsp(fp::fmul(xr, g)))
}

/// `1 / sqrt(x)` in single precision, lbtrigf's static `lb_sqrtf`; a NaN for negatives and
/// NaN, infinity for zero.
#[doc(alias = "lb_sqrtf")]
fn rsqrt(x: f64) -> f64 {
    if x > 0.0 {
        let mut g = fp::frsp(fp::frsqrte(x));
        for _ in 0..3 {
            g = fp::fmuls(fp::fmuls(0.5, g), fp::fnmsubs(x, fp::fmuls(g, g), 3.0));
        }
        g
    } else if x != 0.0 {
        reg_bits(NAN)
    } else {
        reg_bits(INF)
    }
}

/// `atanf`.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "atanf")]
pub fn atan(x: f32) -> f32 {
    let negative = sign(x);
    let bits = x.to_bits() & 0x7FFF_FFFF;
    let xa = reg_bits(bits);
    let t = |i: i32| reg_bits(ATAN[i as usize]);
    let mut index = -1;
    let mut reciprocal = false;
    let mut r = if xa >= reg_bits(SILVER_RATIO) {
        reciprocal = true;
        fp::fdivs(1.0, xa)
    } else if !(reg_bits(SILVER_RATIO_CONJUGATE) < xa) {
        xa
    } else {
        let b = bits as i32;
        index = match bits & 0x7F80_0000 {
            0x3F00_0000 => i32::from(b >= ATAN_THRESHOLDS[0]) + i32::from(b >= ATAN_THRESHOLDS[1]),
            0x3F80_0000 => {
                2 + i32::from(b >= ATAN_THRESHOLDS[2]) + i32::from(b >= ATAN_THRESHOLDS[3])
            }
            0x4000_0000 => 4,
            _ => 0,
        };
        let (a, c) = (t(index + 39), t(index + 33));
        let r = spill(fp::fdivs(1.0, fp::fadds(c, fp::fadds(xa, a))));
        fp::fadds(
            fp::fnmsubs(r, t(index + 7), c),
            fp::fnmsubs(r, t(index + 13), a),
        )
    };
    let rsq = fp::fmuls(r, r);
    let cube = fp::fmuls(r, rsq);
    let mut poly = fp::fmadds(rsq, t(6), t(5));
    for k in (1..=4).rev() {
        poly = fp::fmadds(rsq, poly, t(k));
    }
    r = spill(fp::fmadds(cube, poly, r));
    r = spill(fp::fadds(r, t(index + 27)));
    r = spill(fp::fadds(r, t(index + 20)));
    if reciprocal {
        let r = spill(fp::fsubs(r, reg_bits(PI_2)));
        return mem(if negative != 0 { r } else { fp::fneg(r) });
    }
    f32::from_bits(fp::stfs(r) | negative)
}

/// `atan2f`.
#[doc(alias = "atan2f")]
pub fn atan2(y: f32, x: f32) -> f32 {
    let (yr, xr) = (reg(y), reg(x));
    if sign(x) == sign(y) {
        if sign(x) != 0 {
            return if xr == 0.0 {
                mem(fp::frsp(fp::fneg(fp::fdiv(std::f64::consts::PI, 2.0))))
            } else {
                mem(fp::fsubs(reg(atan(mem(fp::fdivs(yr, xr)))), reg_bits(PI)))
            };
        }
        return if xr != 0.0 {
            atan(mem(fp::fdivs(yr, xr)))
        } else {
            f32::from_bits(PI_2)
        };
    }
    if xr < 0.0 {
        return mem(fp::fadds(reg_bits(PI), reg(atan(mem(fp::fdivs(yr, xr))))));
    }
    if xr != 0.0 {
        return atan(mem(fp::fdivs(yr, xr)));
    }
    f32::from_bits(sign(y).wrapping_add(PI_2))
}

/// `acosf`.
#[doc(alias = "acosf")]
pub fn acos(x: f32) -> f32 {
    let xr = reg(x);
    let r = rsqrt(fp::fnmsubs(xr, xr, 1.0));
    mem(fp::fsubs(reg_bits(PI_2), reg(atan(mem(fp::fmuls(xr, r))))))
}

/// `asinf`.
#[doc(alias = "asinf")]
pub fn asin(x: f32) -> f32 {
    let xr = reg(x);
    atan(mem(fp::fmuls(xr, rsqrt(fp::fnmsubs(xr, xr, 1.0)))))
}
