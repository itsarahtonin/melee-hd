//! The game's own math routines, bit for bit: MSL's `sinf`, `cosf`, `tanf` and the inline
//! `sqrtf`, `lbtrigf`'s `atanf`, `atan2f`, `acosf` and `asinf`, and the `fmodf` HSD's AObj
//! carries.
//!
//! Each follows the instructions MWCC emitted, not just the C: which multiply-adds fused,
//! which operations rounded to single precision, and the `frsqrte` estimate. `gekko-fp` does
//! the arithmetic, so the console's (or Slippi's) rounding follows its mode.

use std::cell::RefCell;

use gekko_fp as fp;

use crate::memo::{self, Memo};

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

/// `sinf`. Its result depends only on the angle's bits (and the float mode), so it is kept
/// for angles that come again (`memo`).
#[doc(alias = "sinf")]
#[doc(alias = "sin__Ff")]
pub fn sin(x: f32) -> f32 {
    thread_local! {
        static SIN: RefCell<Memo<[u32; 2], f32, 12>> = const { RefCell::new(Memo::new()) };
    }
    let key = [x.to_bits(), memo::mode()];
    Memo::get(&SIN, key, memo::hash(&key), || sin_uncached(x))
}

/// [`sin`] without its memo.
pub(crate) fn sin_uncached(x: f32) -> f32 {
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

/// `cosf`, kept like [`sin`].
#[doc(alias = "cosf")]
#[doc(alias = "cos__Ff")]
pub fn cos(x: f32) -> f32 {
    thread_local! {
        static COS: RefCell<Memo<[u32; 2], f32, 12>> = const { RefCell::new(Memo::new()) };
    }
    let key = [x.to_bits(), memo::mode()];
    Memo::get(&COS, key, memo::hash(&key), || cos_uncached(x))
}

/// [`cos`] without its memo.
pub(crate) fn cos_uncached(x: f32) -> f32 {
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
    mem(sqrt_reg(xr))
}

/// The inline `sqrtf` on a register, its result left in a register.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn sqrt_reg(xr: f64) -> f64 {
    if !(xr > 0.0) {
        return xr;
    }
    let mut g = fp::frsqrte(xr);
    for _ in 0..3 {
        let gg = fp::fmul(g, g);
        let half = fp::fmul(0.5, g);
        g = fp::fmul(half, fp::fnmsub(xr, gg, 3.0));
    }
    fp::frsp(fp::fmul(xr, g))
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

/// `fmodf` on register values, the copy compiled into `sysdolphin/baselib/aobj.c`: `x` when
/// `|y| > |x|`, else `x - y * trunc(x / y)` with the quotient through a 64-bit integer
/// (saturating, so huge or NaN quotients give the runtime's results) and one fused
/// multiply-subtract.
#[doc(alias = "fmodf")]
pub fn fmod(x: f64, y: f64) -> f64 {
    if fp::fabs(y) > fp::fabs(x) {
        return x;
    }
    let q = crate::ppc::cvt_dbl_usll(fp::fdivs(x, y)) as i64;
    fp::fnmsubs(y, crate::ppc::cvt_sll_flt(q), x)
}

/// `__ln_F` (MSL/math_data.c): ln(1 + n/128).
pub const LN_F: [u32; 129] = [
    0x00000000, 0x3BFF0153, 0x3C7E0545, 0x3CBDC8D8, 0x3CFC14D8, 0x3D1CF43E, 0x3D3BA2C8, 0x3D5A16EC,
    0x3D785186, 0x3D8B29B7, 0x3D9A0EBD, 0x3DA8D83A, 0x3DB78694, 0x3DC61A2F, 0x3DD4936A, 0x3DE2F2A4,
    0x3DF1383B, 0x3DFF648A, 0x3E06BBF4, 0x3E0DB957, 0x3E14AA98, 0x3E1B8FE1, 0x3E22695B, 0x3E29372F,
    0x3E2FF984, 0x3E36B07F, 0x3E3D5C48, 0x3E43FD03, 0x3E4A92D5, 0x3E511DE1, 0x3E579E4A, 0x3E5E1434,
    0x3E647FBE, 0x3E6AE10B, 0x3E71383B, 0x3E77856E, 0x3E7DC8C3, 0x3E82012D, 0x3E851927, 0x3E882C60,
    0x3E8B3AE5, 0x3E8E44C6, 0x3E914A10, 0x3E944AD1, 0x3E974716, 0x3E9A3EED, 0x3E9D3263, 0x3EA02184,
    0x3EA30C5E, 0x3EA5F2FD, 0x3EA8D56C, 0x3EABB3B9, 0x3EAE8DEE, 0x3EB16418, 0x3EB43641, 0x3EB70475,
    0x3EB9CEC0, 0x3EBC952B, 0x3EBF57C2, 0x3EC2168F, 0x3EC4D19C, 0x3EC788F4, 0x3ECA3CA1, 0x3ECCECAC,
    0x3ECF991F, 0x3ED24205, 0x3ED4E765, 0x3ED7894A, 0x3EDA27BC, 0x3EDCC2C5, 0x3EDF5A6D, 0x3EE1EEBD,
    0x3EE47FBE, 0x3EE70D78, 0x3EE997F4, 0x3EEC1F39, 0x3EEEA350, 0x3EF12441, 0x3EF3A213, 0x3EF61CCF,
    0x3EF8947B, 0x3EFB0920, 0x3EFD7AC4, 0x3EFFE970, 0x3F012A95, 0x3F025EFD, 0x3F0391F3, 0x3F04C37A,
    0x3F05F397, 0x3F07224C, 0x3F084F9D, 0x3F097B8D, 0x3F0AA61F, 0x3F0BCF56, 0x3F0CF736, 0x3F0E1DC1,
    0x3F0F42FB, 0x3F1066E7, 0x3F118987, 0x3F12AADE, 0x3F13CAF1, 0x3F14E9C0, 0x3F16074F, 0x3F1723A2,
    0x3F183EBA, 0x3F19589A, 0x3F1A7145, 0x3F1B88BE, 0x3F1C9F07, 0x3F1DB422, 0x3F1EC813, 0x3F1FDADC,
    0x3F20EC7F, 0x3F21FCFF, 0x3F230C5E, 0x3F241A9F, 0x3F2527C3, 0x3F2633CE, 0x3F273EC1, 0x3F28489E,
    0x3F295169, 0x3F2A5923, 0x3F2B5FCF, 0x3F2C656E, 0x3F2D6A02, 0x3F2E6D8F, 0x3F2F7015, 0x3F307198,
    0x3F317218,
];
/// `__one_over_F` (MSL/math_data.c): 1 / (1 + n/128).
pub const ONE_OVER_F: [u32; 129] = [
    0x3F800000, 0x3F7E03F8, 0x3F7C0FC1, 0x3F7A232D, 0x3F783E10, 0x3F76603E, 0x3F74898D, 0x3F72B9D6,
    0x3F70F0F1, 0x3F6F2EB7, 0x3F6D7304, 0x3F6BBDB3, 0x3F6A0EA1, 0x3F6865AC, 0x3F66C2B4, 0x3F652598,
    0x3F638E39, 0x3F61FC78, 0x3F607038, 0x3F5EE95C, 0x3F5D67C9, 0x3F5BEB62, 0x3F5A740E, 0x3F5901B2,
    0x3F579436, 0x3F562B81, 0x3F54C77B, 0x3F53680D, 0x3F520D21, 0x3F50B6A0, 0x3F4F6475, 0x3F4E168A,
    0x3F4CCCCD, 0x3F4B8728, 0x3F4A4588, 0x3F4907DA, 0x3F47CE0C, 0x3F46980C, 0x3F4565C8, 0x3F443730,
    0x3F430C31, 0x3F41E4BC, 0x3F40C0C1, 0x3F3FA030, 0x3F3E82FA, 0x3F3D6910, 0x3F3C5264, 0x3F3B3EE7,
    0x3F3A2E8C, 0x3F392144, 0x3F381703, 0x3F370FBB, 0x3F360B61, 0x3F3509E7, 0x3F340B41, 0x3F330F63,
    0x3F321643, 0x3F311FD4, 0x3F302C0B, 0x3F2F3ADE, 0x3F2E4C41, 0x3F2D602B, 0x3F2C7692, 0x3F2B8F6A,
    0x3F2AAAAB, 0x3F29C84A, 0x3F28E83F, 0x3F280A81, 0x3F272F05, 0x3F2655C4, 0x3F257EB5, 0x3F24A9CF,
    0x3F23D70A, 0x3F23065E, 0x3F2237C3, 0x3F216B31, 0x3F20A0A1, 0x3F1FD80A, 0x3F1F1166, 0x3F1E4CAD,
    0x3F1D89D9, 0x3F1CC8E1, 0x3F1C09C1, 0x3F1B4C70, 0x3F1A90E8, 0x3F19D723, 0x3F191F1A, 0x3F1868C8,
    0x3F17B426, 0x3F17012E, 0x3F164FDA, 0x3F15A025, 0x3F14F209, 0x3F144581, 0x3F139A86, 0x3F12F114,
    0x3F124925, 0x3F11A2B4, 0x3F10FDBC, 0x3F105A38, 0x3F0FB824, 0x3F0F177A, 0x3F0E7835, 0x3F0DDA52,
    0x3F0D3DCB, 0x3F0CA29C, 0x3F0C08C1, 0x3F0B7034, 0x3F0AD8F3, 0x3F0A42F8, 0x3F09AE41, 0x3F091AC7,
    0x3F088889, 0x3F07F781, 0x3F0767AB, 0x3F06D905, 0x3F064B8A, 0x3F05BF37, 0x3F053408, 0x3F04A9FA,
    0x3F042108, 0x3F039930, 0x3F03126F, 0x3F028CC0, 0x3F020821, 0x3F01848E, 0x3F010204, 0x3F008080,
    0x3F000000,
];
/// logf's polynomial coefficients (`MSL_Math_804DE1B0`, `MSL_Math_804DE1B4`).
const LOG_COEF: [u32; 2] = [0xBF000030, 0x3EAAAA36];
/// `LN2`: ln 2 as a float.
const LN2: u32 = 0x3F317218;

/// `logf`: ln 2 times the exponent plus a table and a short polynomial in the mantissa. The
/// exponent keeps the sign bit, as the original's does, so negative inputs give large values.
#[doc(alias = "logf")]
pub fn log(x: f32) -> f32 {
    let raw = x.to_bits();
    match raw & 0x7F80_0000 {
        0x7F80_0000 => {
            if raw & 0x007F_FFFF != 0 {
                x
            } else if raw & 0x8000_0000 != 0 {
                f32::from_bits(NAN)
            } else {
                f32::from_bits(INF)
            }
        }
        0 => mem(fp::fneg(reg_bits(INF))),
        _ => {
            let e = ((raw & 0xFF80_0000) >> 23).wrapping_sub(127) as i32;
            let mut m = ((raw >> 16) & 0x7F) as usize;
            let base =
                |m: usize| fp::fmadds(reg_bits(LN2), fp::frsp(f64::from(e)), reg_bits(LN_F[m]));
            if raw & 0xFFFF == 0 {
                return mem(base(m));
            }
            let mut fm = (raw & 0x007F_0000) | 0x3F80_0000;
            let fbig = (raw & 0x007F_FFFF) | 0x3F80_0000;
            if raw & 0x8000 != 0 {
                fm = fm.wrapping_add(0x1_0000);
                m += 1;
            }
            let h = fp::fmuls(
                fp::fsubs(reg_bits(fbig), reg_bits(fm)),
                reg_bits(ONE_OVER_F[m]),
            );
            let poly = fp::fmuls(
                fp::fmuls(h, h),
                fp::fmadds(h, reg_bits(LOG_COEF[1]), reg_bits(LOG_COEF[0])),
            );
            mem(fp::fadds(base(m), fp::fadds(h, poly)))
        }
    }
}

/// `expf` (melee/lb/lb_00CE.c): the Taylor series summed until the sum stops changing, and
/// inverted for negative inputs. Like the original it never returns for NaN or inputs large
/// enough to overflow the terms; the game doesn't pass those.
#[doc(alias = "expf")]
pub fn exp(x: f32) -> f32 {
    let mut x = reg(x);
    let negative = x < 0.0;
    if negative {
        x = fp::fneg(x);
    }
    let (mut n, mut fact) = (2i32, 1.0);
    let mut term = x;
    let mut sum = fp::fadds(1.0, x);
    loop {
        let k = n;
        term = fp::fmuls(term, x);
        let before = sum;
        n = n.wrapping_add(1);
        fact = fp::fmuls(fact, fp::frsp(f64::from(k)));
        sum = fp::fadds(sum, fp::fdivs(term, fact));
        if sum == before {
            break;
        }
    }
    if negative {
        sum = fp::fdivs(1.0, sum);
    }
    mem(sum)
}

/// `powf` (melee/lb/lb_00CE.c): `exp(y * 2 atanh((x - 1) / (x + 1)))`, the series summed
/// until it stops changing, and 0 for a base of 0.
#[doc(alias = "powf")]
pub fn pow(x: f32, y: f32) -> f32 {
    let (x, y) = (reg(x), reg(y));
    if x == 0.0 {
        return 0.0;
    }
    let mut n = 1i32;
    let mut term = fp::fdivs(fp::fsubs(x, 1.0), fp::fadds(1.0, x));
    let mut sum = term;
    let sq = fp::fmuls(term, term);
    loop {
        n = n.wrapping_add(2);
        term = fp::fmuls(term, sq);
        let before = sum;
        sum = fp::fadds(sum, fp::fdivs(term, fp::frsp(f64::from(n))));
        if sum == before {
            break;
        }
    }
    exp(mem(fp::fmuls(y, fp::fmuls(2.0, sum))))
}
