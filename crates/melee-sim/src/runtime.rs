//! MWCC's runtime helpers (`Runtime/runtime.c`): the 64-bit conversions the compiler calls
//! for casts the Gekko has no instruction for. Each follows its assembly instruction by
//! instruction, so results match everywhere, down to NaN and out-of-range values.

use gekko_fp as fp;

use crate::ppc::{slw, srw};

/// `adde`: a + b + carry, and the carry out.
fn adde(a: u32, b: u32, ca: bool) -> (u32, bool) {
    let sum = u64::from(a) + u64::from(b) + u64::from(ca);
    (sum as u32, sum >> 32 != 0)
}

fn hi(x: u64) -> u32 {
    (x >> 32) as u32
}

fn lo(x: u64) -> u32 {
    x as u32
}

fn join(h: u32, l: u32) -> u64 {
    (u64::from(h) << 32) | u64::from(l)
}

/// `cntlzw` of a 64-bit value in two words.
fn clz64(h: u32, l: u32) -> u32 {
    if h != 0 {
        h.leading_zeros()
    } else {
        l.leading_zeros() + 32
    }
}

/// `(float) x` for a signed 64-bit `x`: the double it builds by hand, rounded to nearest
/// even, then rounded to single precision. Returns the register.
#[doc(alias = "__cvt_sll_flt")]
pub fn i64_to_f32(x: i64) -> f64 {
    let (mut r3, mut r4) = (hi(x as u64), lo(x as u64));
    let r5 = r3 & 0x8000_0000;
    if r5 != 0 {
        let n = join(r3, r4).wrapping_neg();
        (r3, r4) = (hi(n), lo(n));
    }
    if r3 | r4 != 0 {
        let r7 = clz64(r3, r4);
        let r8 = 32u32.wrapping_sub(r7);
        let r9 = r7.wrapping_sub(32);
        let h = slw(r3, r7) | srw(r4, r8) | slw(r4, r9);
        r4 = slw(r4, r7);
        r3 = h;
        let mut r6 = 0u32.wrapping_sub(r7).wrapping_add(0x43E);
        let round = r4 & 0x7FF;
        if round > 0x400 || (round == 0x400 && r4 & 0x800 != 0) {
            let (l, c) = adde(r4, 0x800, false);
            let (h, c) = adde(r3, 0, c);
            r4 = l;
            r3 = h;
            r6 = r6.wrapping_add(u32::from(c));
        }
        r4 = r4.rotate_right(11);
        r4 = (r4 & 0x001F_FFFF) | (r3.rotate_left(21) & 0xFFE0_0000);
        r3 = r3.rotate_left(21) & 0x000F_FFFF;
        r3 |= r5 | (r6 << 20);
    }
    fp::frsp(f64::from_bits(join(r3, r4)))
}

/// `(long long) d` and `(unsigned long long) d` for a register `d`: truncated, 0 below 1 in
/// magnitude, and saturated (by sign) from 2^63 up, NaN included.
#[doc(alias = "__cvt_dbl_usll")]
pub fn f64_to_u64(d: f64) -> u64 {
    let bits = d.to_bits();
    let (mut r3, mut r4) = (hi(bits), lo(bits));
    let r5 = (r3 >> 20) & 0x7FF;
    if r5 < 0x3FF {
        return 0;
    }
    let r6 = r3;
    r3 = (r3 & 0x000F_FFFF) | 0x0010_0000;
    let r5 = r5.wrapping_sub(0x433);
    if (r5 as i32) < 0 {
        let n = r5.wrapping_neg();
        let r8 = 32u32.wrapping_sub(n);
        let r9 = n.wrapping_sub(32);
        r4 = srw(r4, n) | slw(r3, r8) | srw(r3, r9);
        r3 = srw(r3, n);
    } else if (r5 as i32) > 10 {
        return if r6 & 0x8000_0000 != 0 {
            0x8000_0000_0000_0000
        } else {
            0x7FFF_FFFF_FFFF_FFFF
        };
    } else {
        let r8 = 32u32.wrapping_sub(r5);
        let r9 = r5.wrapping_sub(32);
        let h = slw(r3, r5) | srw(r4, r8) | slw(r4, r9);
        r4 = slw(r4, r5);
        r3 = h;
    }
    let v = join(r3, r4);
    if r6 & 0x8000_0000 != 0 {
        v.wrapping_neg()
    } else {
        v
    }
}
