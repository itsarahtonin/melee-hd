//! PowerPC integer semantics C leaves undefined and MWCC's code defines, and the compiler
//! runtime's 64-bit and conversion helpers (`Runtime/runtime.c`), transcribed from their
//! assembly so edge cases (division by zero, out-of-range conversions) behave the same.

use gekko_fp as fp;

/// `slw`: shifts of 32 or more give 0.
pub fn slw(a: u32, n: u32) -> u32 {
    let n = n & 63;
    if n >= 32 { 0 } else { a << n }
}

/// `slw` on a signed value.
pub fn slw_i(a: i32, n: u32) -> i32 {
    slw(a as u32, n) as i32
}

/// `srw`.
fn srw(x: u32, n: u32) -> u32 {
    let n = n & 63;
    if n >= 32 { 0 } else { x >> n }
}

/// `sraw`.
fn sraw(x: u32, n: u32) -> u32 {
    let n = n & 63;
    (if n >= 32 {
        (x as i32) >> 31
    } else {
        (x as i32) >> n
    }) as u32
}

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

/// The long division the four division helpers share: (r3:r4) by (r5:r6), a bit at a time.
/// None where the dividend has fewer significant bits than the divisor, which the helpers
/// return early for; else the quotient (r3:r4) and remainder (r7:r8).
fn long_division(mut r3: u32, mut r4: u32, r5: u32, r6: u32) -> Option<(u64, u64)> {
    let mut r0 = clz64(r3, r4);
    let mut r9 = clz64(r5, r6);
    if (r0 as i32) > (r9 as i32) {
        return None;
    }
    let r10 = 64u32.wrapping_sub(r0);
    r9 = 64u32.wrapping_sub(r9.wrapping_add(1));
    r0 = r0.wrapping_add(r9);
    r9 = r10.wrapping_sub(r9);
    let mut ctr = r9;
    let (mut r7, mut r8);
    if (r9 as i32) < 32 {
        r8 = srw(r4, r9) | slw(r3, 32u32.wrapping_sub(r9));
        r7 = srw(r3, r9);
    } else {
        r8 = srw(r3, r9.wrapping_sub(32));
        r7 = 0;
    }
    if (r0 as i32) < 32 {
        let low = srw(r4, 32u32.wrapping_sub(r0));
        r3 = slw(r3, r0) | low;
        r4 = slw(r4, r0);
    } else {
        r3 = slw(r4, r0.wrapping_sub(32));
        r4 = 0;
    }
    // `addic r7, r7, 0` clears the carry.
    let mut ca = false;
    loop {
        (r4, ca) = adde(r4, r4, ca);
        (r3, ca) = adde(r3, r3, ca);
        (r8, ca) = adde(r8, r8, ca);
        (r7, _) = adde(r7, r7, ca);
        // subfc r0, r6, r8; subfe. r9, r5, r7
        let d0 = r8.wrapping_sub(r6);
        let (d9, c) = adde(!r5, r7, r8 >= r6);
        ca = c;
        if (d9 as i32) >= 0 {
            r8 = d0;
            r7 = d9;
            // `addic r0, r10, 1` with r10 = -1 sets the carry: this quotient bit is 1.
            ca = true;
        }
        ctr = ctr.wrapping_sub(1);
        if ctr == 0 {
            break;
        }
    }
    (r4, ca) = adde(r4, r4, ca);
    (r3, _) = adde(r3, r3, ca);
    Some((join(r3, r4), join(r7, r8)))
}

/// Unsigned 64-bit `a / b`.
#[doc(alias = "__div2u")]
pub fn div2u(a: u64, b: u64) -> u64 {
    long_division(hi(a), lo(a), hi(b), lo(b)).map_or(0, |(q, _)| q)
}

/// Signed 64-bit `a / b`: the magnitudes divided, negated if the signs differ.
#[doc(alias = "__div2i")]
pub fn div2i(a: i64, b: i64) -> i64 {
    let (ua, ub) = (a.unsigned_abs(), b.unsigned_abs());
    match long_division(hi(ua), lo(ua), hi(ub), lo(ub)) {
        None => 0,
        Some((q, _)) if (a < 0) != (b < 0) => (q as i64).wrapping_neg(),
        Some((q, _)) => q as i64,
    }
}

/// Unsigned 64-bit `a % b`.
#[doc(alias = "__mod2u")]
pub fn mod2u(a: u64, b: u64) -> u64 {
    long_division(hi(a), lo(a), hi(b), lo(b)).map_or(a, |(_, r)| r)
}

/// Signed 64-bit `a % b`, with the sign of `a`.
#[doc(alias = "__mod2i")]
pub fn mod2i(a: i64, b: i64) -> i64 {
    let (ua, ub) = (a.unsigned_abs(), b.unsigned_abs());
    let r = long_division(hi(ua), lo(ua), hi(ub), lo(ub)).map_or(ua, |(_, r)| r) as i64;
    if a < 0 { r.wrapping_neg() } else { r }
}

/// 64-bit `a << n`.
#[doc(alias = "__shl2i")]
pub fn shl2i(a: i64, n: i32) -> i64 {
    let (r3, r4, n) = (hi(a as u64), lo(a as u64), n as u32);
    let r8 = 32u32.wrapping_sub(n);
    let r9 = n.wrapping_sub(32);
    let h = slw(r3, n) | srw(r4, r8) | slw(r4, r9);
    join(h, slw(r4, n)) as i64
}

/// Unsigned 64-bit `a >> n`.
#[doc(alias = "__shr2u")]
pub fn shr2u(a: u64, n: i32) -> u64 {
    let (r3, r4, n) = (hi(a), lo(a), n as u32);
    let r8 = 32u32.wrapping_sub(n);
    let r9 = n.wrapping_sub(32);
    let l = srw(r4, n) | slw(r3, r8) | srw(r3, r9);
    join(srw(r3, n), l)
}

/// Signed 64-bit `a >> n`.
#[doc(alias = "__shr2i")]
pub fn shr2i(a: i64, n: i32) -> i64 {
    let (r3, r4, n) = (hi(a as u64), lo(a as u64), n as u32);
    let r8 = 32u32.wrapping_sub(n);
    let r9 = n.wrapping_sub(32);
    let mut l = srw(r4, n) | slw(r3, r8);
    if (r9 as i32) > 0 {
        l |= sraw(r3, r9);
    }
    join(sraw(r3, n), l) as i64
}

/// `(float) x` for a signed 64-bit x: the double it builds by hand, rounded to nearest even,
/// then rounded to single precision.
#[doc(alias = "__cvt_sll_flt")]
pub fn cvt_sll_flt(x: i64) -> f64 {
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

/// `(long long) d` and `(unsigned long long) d`: truncated, and saturated past 2^63.
#[doc(alias = "__cvt_dbl_usll")]
pub fn cvt_dbl_usll(d: f64) -> u64 {
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

/// `(unsigned long) d`: 0 below 0, all ones from 2^32 up (and for NaN), and past 2^31 through
/// a signed conversion of d - 2^31.
#[doc(alias = "__cvt_fp2unsigned")]
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn cvt_fp2unsigned(d: f64) -> u32 {
    const TWO_32: f64 = 4_294_967_296.0;
    const TWO_31: f64 = 2_147_483_648.0;
    if d < 0.0 {
        return 0;
    }
    // `bge cr6` also takes NaN, which compares unordered.
    if !(d < TWO_32) {
        return u32::MAX;
    }
    if d < TWO_31 {
        fp::fctiwz(d) as u32
    } else {
        (fp::fctiwz(fp::fsub(d, TWO_31)) as u32).wrapping_add(0x8000_0000)
    }
}
