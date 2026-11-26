//! `PSMTXConcat`'s paired-single sequence in 128-bit lanes (two doubles), on x86-64 with AVX2
//! and FMA3.
//!
//! Each output row of `a · b` is, per column `j` (the routine's `ps_muls0`, `ps_madds1`,
//! `ps_madds0` and, for the translation column pair, `ps_madds1` with `(0, 1)`):
//!
//! ```text
//! s1 = frsp(b[0][j] · a[i][0])
//! s2 = frsp(fma(b[1][j], a[i][1], s1))
//! s3 = frsp(fma(b[2][j], a[i][2], s2))
//! s4 = frsp(fma(u[j], a[i][3], s3))     columns 2 and 3 only, u = (0, 1)
//! ```
//!
//! the same operations, in the same order, as the scalar lanes do them: fused exactly where
//! they fuse, rounded to single exactly where they round. What the scalar ops do beyond the
//! plain IEEE arithmetic is left to them: any input not a normal float or zero, any fused
//! result exactly halfway between two singles whose double rounding lost something (the
//! Gekko's fix-up, [`crate::fmadds`]), or any output not a normal single or zero (NaN,
//! infinity, a subnormal, which the store and the modes treat apart) returns `None`, and the
//! caller runs the scalar routine. Otherwise every frC operand is already a single (its
//! rounding is itself), every product of two singles is exact, and the result is the scalar
//! routine's bit for bit, in both modes.
//!
//! So there are two paths, picked at run time: these lanes (any x86-64 with AVX2 and FMA3; the
//! same instructions on every such processor, no wider variant), and the scalar lanes
//! (anything else, a thread on [`crate::set_portable_fma`], `GEKKO_SCALAR_LANES` set in the
//! environment, or [`set_wide_lanes`] off). `tests` here and melee-sim's
//! `concat_paths_agree` hold them to the same bits.
//!
//! The lanes are 128 bits wide: 256-bit fused multiply-adds can lower the clock of a
//! processor with AVX-512 (the worker's Skylake-X), and a 256-bit version wasn't faster
//! (docs/performance.md).

#[cfg(target_arch = "x86_64")]
use std::sync::atomic::{AtomicU8, Ordering};

/// Whether the wide lanes run: not yet decided, on, or off.
#[cfg(target_arch = "x86_64")]
static WIDE: AtomicU8 = AtomicU8::new(UNDECIDED);
#[cfg(target_arch = "x86_64")]
const UNDECIDED: u8 = 0;
#[cfg(target_arch = "x86_64")]
const ON: u8 = 1;
#[cfg(target_arch = "x86_64")]
const OFF: u8 = 2;

/// Turns the wide lanes on (where the processor has them) or off for every thread, so tests
/// and checks can run the scalar path on a processor that would take the wide one.
pub fn set_wide_lanes(on: bool) {
    #[cfg(target_arch = "x86_64")]
    WIDE.store(if on && supported() { ON } else { OFF }, Ordering::Relaxed);
    #[cfg(not(target_arch = "x86_64"))]
    let _ = on;
}

/// Whether [`mtx_concat`] can take the wide lanes on this thread now.
pub fn wide_lanes() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        available()
    }
    #[cfg(not(target_arch = "x86_64"))]
    false
}

/// `a · b` as `PSMTXConcat` computes it, or `None` where the scalar lanes must run.
#[inline]
pub fn mtx_concat(a: &[[f32; 4]; 3], b: &[[f32; 4]; 3]) -> Option<[[f32; 4]; 3]> {
    #[cfg(target_arch = "x86_64")]
    {
        if !available() {
            return None;
        }
        // SAFETY: `available` checked AVX2 and FMA3.
        unsafe { concat_fma(a, b) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (a, b);
        None
    }
}

/// AVX2 and FMA3.
#[cfg(target_arch = "x86_64")]
fn supported() -> bool {
    std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma")
}

/// Whether the wide lanes may run: on (the processor has them and nothing turned them off),
/// and the thread's fused multiply-adds on the hardware path ([`crate::portable_fma`]).
#[cfg(target_arch = "x86_64")]
#[inline]
fn available() -> bool {
    let mut w = WIDE.load(Ordering::Relaxed);
    if w == UNDECIDED {
        w = if supported() && std::env::var_os("GEKKO_SCALAR_LANES").is_none() {
            ON
        } else {
            OFF
        };
        WIDE.store(w, Ordering::Relaxed);
    }
    w == ON && !crate::portable_fma()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn concat_fma(a: &[[f32; 4]; 3], b: &[[f32; 4]; 3]) -> Option<[[f32; 4]; 3]> {
    use std::arch::x86_64::*;
    // SAFETY (the whole body): the intrinsics need only AVX2 and FMA3, which the caller
    // checked; the loads and stores are of four-float rows.
    unsafe {
        let load = |r: &[f32; 4]| _mm_loadu_ps(r.as_ptr());
        // Lanes that aren't a normal float or a zero: magnitude bits at least the
        // infinity's, or nonzero and below the smallest normal's.
        let abs_mask = _mm_set1_epi32(0x7FFF_FFFF);
        let max_finite = _mm_set1_epi32(0x7F7F_FFFF);
        let min_normal = _mm_set1_epi32(0x0080_0000);
        let zero = _mm_setzero_si128();
        let odd = |v: __m128| {
            let m = _mm_and_si128(_mm_castps_si128(v), abs_mask);
            let special = _mm_cmpgt_epi32(m, max_finite);
            let subnormal = _mm_and_si128(_mm_cmpgt_epi32(m, zero), _mm_cmpgt_epi32(min_normal, m));
            _mm_or_si128(special, subnormal)
        };
        let (ar, br) = (a.map(|r| load(&r)), b.map(|r| load(&r)));
        let mut bad = zero;
        for v in ar.iter().chain(&br) {
            bad = _mm_or_si128(bad, odd(*v));
        }
        if _mm_testz_si128(bad, bad) == 0 {
            return None;
        }
        // Each row of `b` as two pairs of doubles: columns 0 and 1, columns 2 and 3.
        let lo = br.map(|v| _mm_cvtps_pd(v));
        let hi = br.map(|v| _mm_cvtps_pd(_mm_movehl_ps(v, v)));
        // `frsp`: to the nearest single and back (a NaN is caught by `rare` first).
        let frsp = |x: __m128d| _mm_cvtps_pd(_mm_cvtpd_ps(x));
        let low29 = _mm_set1_epi64x(0x1FFF_FFFF);
        let half = _mm_set1_epi64x(0x1000_0000);
        // A fused `x · c + s` whose result the scalar op's rare path may change: a NaN, or
        // exactly halfway between two singles with a nonzero error from the double rounding
        // (the error worked out as the scalar op does; a zero one leaves the result alone in
        // both modes). Products of short mantissas land halfway often, exactly.
        let rare = |x: __m128d, c: __m128d, s: __m128d, r: __m128d| {
            let bits = _mm_and_si128(_mm_castpd_si128(r), low29);
            let halfway = _mm_cmpeq_epi64(bits, half);
            let a_prime = _mm_sub_pd(s, r);
            let b_prime = _mm_add_pd(r, a_prime);
            let error = _mm_add_pd(_mm_fmadd_pd(x, c, a_prime), _mm_sub_pd(s, b_prime));
            let off = _mm_castpd_si128(_mm_cmpneq_pd(error, _mm_setzero_pd()));
            let nan = _mm_castpd_si128(_mm_cmpunord_pd(r, r));
            _mm_or_si128(_mm_and_si128(halfway, off), nan)
        };
        // Columns 2 and 3 take a fourth step: (0, 1) times the row's translation.
        let unit = _mm_set_pd(1.0, 0.0);
        let mut flags = zero;
        let mut out = [[0.0f32; 4]; 3];
        for (o, ai) in out.iter_mut().zip(a) {
            let c = |k: usize| _mm_set1_pd(f64::from(ai[k]));
            let mut half_row = |b: &[__m128d; 3]| {
                let p = _mm_mul_pd(b[0], c(0));
                flags = _mm_or_si128(flags, _mm_castpd_si128(_mm_cmpunord_pd(p, p)));
                let s = frsp(p);
                let r = _mm_fmadd_pd(b[1], c(1), s);
                flags = _mm_or_si128(flags, rare(b[1], c(1), s, r));
                let s = frsp(r);
                let r = _mm_fmadd_pd(b[2], c(2), s);
                flags = _mm_or_si128(flags, rare(b[2], c(2), s, r));
                frsp(r)
            };
            let left = half_row(&lo);
            let s3 = half_row(&hi);
            let r = _mm_fmadd_pd(unit, c(3), s3);
            flags = _mm_or_si128(flags, rare(unit, c(3), s3, r));
            let right = frsp(r);
            let row = _mm_movelh_ps(_mm_cvtpd_ps(left), _mm_cvtpd_ps(right));
            flags = _mm_or_si128(flags, odd(row));
            _mm_storeu_ps(o.as_mut_ptr(), row);
        }
        (_mm_testz_si128(flags, flags) != 0).then_some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FpMode, Ps, ps_madds0, ps_madds1, ps_muls0, set_fp_mode, stfs_ftz};

    /// The scalar sequence, as melee-sim's `Mtx34::concat` runs it.
    fn scalar(a: &[[f32; 4]; 3], b: &[[f32; 4]; 3]) -> [[f32; 4]; 3] {
        let l = |x: f32| crate::lfs(x.to_bits());
        let pair = |m: &[[f32; 4]; 3], i: usize| {
            let f = m.as_flattened();
            Ps::new(l(f[i]), l(f[i + 1]))
        };
        let unit01 = Ps::new(0.0, 1.0);
        let mut out = [[0.0f32; 4]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            let (a0, a2) = (pair(a, 4 * i), pair(a, 4 * i + 2));
            let r0 = ps_madds0(
                pair(b, 8),
                a2,
                ps_madds1(pair(b, 4), a0, ps_muls0(pair(b, 0), a0)),
            );
            let r2 = ps_madds1(
                unit01,
                a2,
                ps_madds0(
                    pair(b, 10),
                    a2,
                    ps_madds1(pair(b, 6), a0, ps_muls0(pair(b, 2), a0)),
                ),
            );
            for (o, v) in row.iter_mut().zip([r0.ps0, r0.ps1, r2.ps0, r2.ps1]) {
                *o = f32::from_bits(stfs_ftz(v));
            }
        }
        out
    }

    struct Bits(u64);

    impl Bits {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 32) as u32
        }

        /// Matrix-like floats mostly; zeros of both signs, ones, any bits, huge and tiny
        /// values, and short mantissas over a wide range of exponents (whose sums land
        /// exactly halfway between two singles) some of the time.
        fn float(&mut self) -> f32 {
            let b = self.next();
            match b % 16 {
                0 => f32::from_bits(b & 0x8000_0000),
                1 => f32::from_bits(self.next()),
                2 => f32::from_bits((b & 0x8000_0000) | 0x3F80_0000),
                3 => f32::from_bits((self.next() & 0x807F_FFFF) | (250 << 23)),
                4 => f32::from_bits((self.next() & 0x807F_FFFF) | (3 << 23)),
                5..=7 => f32::from_bits((self.next() & 0x807F_F000) | ((100 + b % 40) << 23)),
                _ => f32::from_bits((self.next() & 0x807F_FFFF) | ((118 + b % 16) << 23)),
            }
        }
    }

    fn check(a: &[[f32; 4]; 3], b: &[[f32; 4]; 3]) -> bool {
        let fast = mtx_concat(a, b);
        if let Some(f) = fast {
            let s = scalar(a, b);
            for (x, y) in f.iter().flatten().zip(s.iter().flatten()) {
                assert_eq!(
                    x.to_bits(),
                    y.to_bits(),
                    "{a:?} · {b:?}: {f:?} against {s:?}"
                );
            }
        }
        fast.is_some()
    }

    #[test]
    fn matches_the_scalar_lanes() {
        for mode in [FpMode::Hardware, FpMode::Slippi] {
            set_fp_mode(mode);
            let mut r = Bits(0x9E37_79B9_7F4A_7C15);
            let mut taken = 0;
            for _ in 0..200_000 {
                let mut m = [[[0.0f32; 4]; 3]; 2];
                for v in m.iter_mut().flatten().flatten() {
                    *v = r.float();
                }
                taken += usize::from(check(&m[0], &m[1]));
            }
            #[cfg(target_arch = "x86_64")]
            if std::arch::is_x86_feature_detected!("avx2")
                && std::arch::is_x86_feature_detected!("fma")
            {
                assert!(taken > 50_000, "the wide lanes ran {taken} times");
            }
            // Elsewhere the wide lanes never run; the count is only read above.
            #[cfg(not(target_arch = "x86_64"))]
            let _ = taken;
            set_fp_mode(FpMode::Hardware);
        }
    }

    /// A fused result exactly halfway after its double rounding, which the Gekko rounds the
    /// other way (`fmadds`'s fix-up): the wide lanes hand it back.
    #[test]
    fn hands_back_a_halfway_sum() {
        let odd = f32::from_bits(0x3F80_0001); // 1 + 2^-23
        let a = [
            [1.0, f32::from_bits(0x39800001), 0.0, 0.0],
            [0.0; 4],
            [0.0; 4],
        ];
        let b = [
            [odd, 0.0, 0.0, 0.0],
            [f32::from_bits(0x397F_FFFE), 0.0, 0.0, 0.0],
            [0.0; 4],
        ];
        // 2^-12 (1 + 2^-23) · 2^-12 (1 - 2^-23) = 2^-24 - 2^-70: (1 + 2^-23) plus it rounds
        // in double to 1 + 3 · 2^-24, exactly halfway; the exact sum is below, so the Gekko
        // keeps 1 + 2^-23 where rounding the double to even would give 1 + 2^-22.
        let s = scalar(&a, &b);
        assert_eq!(s[0][0].to_bits(), 0x3F80_0001, "the Gekko's fix-up");
        assert_eq!(mtx_concat(&a, &b), None);
        // 2^-12 · 2^-12 = 2^-24 exactly: halfway with nothing lost, which both the Gekko and
        // the wide lanes round to even (1 + 2^-22).
        let a = [
            [1.0, f32::from_bits(0x3980_0000), 0.0, 0.0],
            [0.0; 4],
            [0.0; 4],
        ];
        let b = [
            [odd, 0.0, 0.0, 0.0],
            [f32::from_bits(0x3980_0000), 0.0, 0.0, 0.0],
            [0.0; 4],
        ];
        let s = scalar(&a, &b);
        assert_eq!(s[0][0].to_bits(), 0x3F80_0002, "rounded to even");
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma")
        {
            assert_eq!(mtx_concat(&a, &b), Some(s));
        }
    }
}
