//! MWCC's runtime helpers for 64-bit conversions (`Runtime/runtime.c`), which the compiler
//! calls where C converts between floats and `long long`.

use gekko_fp as fp;

/// A double truncated toward zero to a 64-bit integer, `__cvt_dbl_usll`: 0 below 1 in
/// magnitude, and saturated from 2^63 up (infinities and NaN by their sign bit).
#[doc(alias = "__cvt_dbl_usll")]
pub fn trunc_to_i64(d: f64) -> i64 {
    let bits = d.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as i32;
    if exp < 0x3FF {
        return 0;
    }
    let negative = bits >> 63 != 0;
    let shift = exp - 0x433;
    if shift > 10 {
        return if negative { i64::MIN } else { i64::MAX };
    }
    let mantissa = (bits & 0x000F_FFFF_FFFF_FFFF) | 1 << 52;
    let v = if shift < 0 {
        mantissa >> -shift
    } else {
        mantissa << shift
    } as i64;
    if negative { v.wrapping_neg() } else { v }
}

/// A 64-bit integer converted to single precision, `__cvt_sll_flt`: rounded to a double's 53
/// bits (to nearest, ties to even, as Rust's conversion does), then by `frsp`.
#[doc(alias = "__cvt_sll_flt")]
pub fn i64_to_single(x: i64) -> f64 {
    fp::frsp(x as f64)
}
