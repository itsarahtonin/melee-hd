// SPDX-License-Identifier: GPL-3.0-or-later
// Semantics ported from Dolphin's interpreter (GPL-2.0-or-later), which is verified against hardware.

//! Bit-exact floating-point operations of the GameCube's Gekko CPU.
//!
//! Values are floating-point register contents (`f64`), as on PowerPC. Operand order follows the
//! assembly (`fmadds frD, frA, frC, frB` is `fmadds(a, c, b)`). FPSCR is modelled as 0: round to
//! nearest, non-IEEE mode off.
//!
//! [`FpMode::Slippi`] instead reproduces how Slippi's netplay Dolphin (the Ishiiruka fork, x86-64
//! JIT with FMA3) computes, since that is what recorded Slippi replays.
//!
//! This is melee-hd's copy of ssbm-rs's crate (at `melee-hd-base`), changed only for speed: the
//! fused multiply-add takes x86-64's FMA3 where the processor has it, the NaN and halfway
//! results are computed out of line, a single frC skips its rounding, and the ops inline
//! across crates. `tests/against_upstream.rs` checks every op against the unchanged upstream
//! in both modes (`docs/performance.md`).

use std::cell::Cell;
#[cfg(target_arch = "x86_64")]
use std::sync::atomic::AtomicU8;
use std::sync::atomic::{AtomicUsize, Ordering};

mod estimate;
mod paired;

pub use estimate::{fres, frsqrte};
pub use paired::*;

const SIGN: u64 = 0x8000_0000_0000_0000;
const EXP: u64 = 0x7FF0_0000_0000_0000;
const FRAC: u64 = 0x000F_FFFF_FFFF_FFFF;
const QUIET: u64 = 0x0008_0000_0000_0000;

/// The NaN invalid operations produce. Unlike x86's default NaN, it is positive.
pub const PPC_NAN: f64 = f64::from_bits(0x7FF8_0000_0000_0000);

/// x86's default NaN, which Slippi's Dolphin produces instead of [`PPC_NAN`].
pub const X86_NAN: f64 = f64::from_bits(0xFFF8_0000_0000_0000);

/// Whose floating point to reproduce.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FpMode {
    /// The Gekko itself, as current Dolphin reproduces it.
    #[default]
    Hardware,
    /// Slippi's netplay Dolphin (the Ishiiruka fork), whose x86-64 JIT recorded Slippi replays.
    /// It differs from hardware: fused ops round to double and then to single;
    /// `fnmadd`/`fnmsub` use x86's `-(a*c) - b` and `-(a*c) + b`, which give the other signed
    /// zero when the result cancels; stores round to nearest instead of truncating; invalid
    /// operations give x86's negative NaN; frC keeps subnormals as they are when rounded to 25
    /// bits; and `fctiw` does not mark a negative zero.
    Slippi,
}

thread_local! {
    static FP_MODE: Cell<FpMode> = const { Cell::new(FpMode::Hardware) };
}

/// How many threads have set [`FpMode::Slippi`] and not set it back (one that ends in it
/// stays counted). While none has, every thread is in [`FpMode::Hardware`] and the mode
/// needs no thread-local read, which isn't inlined across crates on Windows. Each thread
/// counts only itself, so the count it reads includes its own change.
static SLIPPI_THREADS: AtomicUsize = AtomicUsize::new(0);

/// The floating-point mode of the current thread.
#[inline]
pub fn fp_mode() -> FpMode {
    if SLIPPI_THREADS.load(Ordering::Relaxed) == 0 {
        return FpMode::Hardware;
    }
    FP_MODE.with(Cell::get)
}

/// Sets the floating-point mode for the current thread.
#[inline]
pub fn set_fp_mode(mode: FpMode) {
    let old = FP_MODE.with(|m| m.replace(mode));
    match (old, mode) {
        (FpMode::Hardware, FpMode::Slippi) => SLIPPI_THREADS.fetch_add(1, Ordering::Relaxed),
        (FpMode::Slippi, FpMode::Hardware) => SLIPPI_THREADS.fetch_sub(1, Ordering::Relaxed),
        _ => 0,
    };
}

#[inline]
pub(crate) fn slippi() -> bool {
    fp_mode() == FpMode::Slippi
}

/// The NaN an invalid operation produces in the current mode.
#[inline]
fn default_nan() -> f64 {
    if slippi() { X86_NAN } else { PPC_NAN }
}

#[inline]
pub(crate) fn make_quiet(x: f64) -> f64 {
    f64::from_bits(x.to_bits() | QUIET)
}

#[cold]
#[inline(never)]
fn nan_result(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        make_quiet(a)
    } else if b.is_nan() {
        make_quiet(b)
    } else {
        default_nan()
    }
}

#[inline]
fn negate_unless_nan(x: f64) -> f64 {
    if x.is_nan() { x } else { -x }
}

#[inline]
pub(crate) fn add(a: f64, b: f64) -> f64 {
    let r = a + b;
    if r.is_nan() { nan_result(a, b) } else { r }
}

#[inline]
pub(crate) fn sub(a: f64, b: f64) -> f64 {
    let r = a - b;
    if r.is_nan() { nan_result(a, b) } else { r }
}

#[inline]
pub(crate) fn mul(a: f64, c: f64) -> f64 {
    let r = a * c;
    if r.is_nan() { nan_result(a, c) } else { r }
}

#[inline]
pub(crate) fn div(a: f64, b: f64) -> f64 {
    let r = a / b;
    if r.is_nan() { nan_result(a, b) } else { r }
}

/// Rounds the frC operand of a single-precision multiply to 25 mantissa bits.
#[inline]
pub(crate) fn round_c(c: f64) -> f64 {
    let bits = c.to_bits();
    if bits & 0x0FFF_FFFF == 0 {
        // Nothing below the rounding bit, wherever normalizing puts it: a single's value
        // (frC usually is one) is its own rounding.
        return c;
    }
    let (keep, round) = if bits & EXP == 0 && bits & FRAC != 0 && !slippi() {
        // Subnormals are normalized first, which moves the rounding bit.
        let shift = (bits & FRAC).leading_zeros() - 11;
        (
            (0xFFFF_FFFF_F800_0000_u64 as i64 >> shift) as u64,
            0x800_0000_u64 >> shift,
        )
    } else {
        (0xFFFF_FFFF_F800_0000, 0x800_0000)
    };
    f64::from_bits((bits & keep) + (bits & round))
}

/// Which fused multiply-add [`mul_add`] takes: [`UNDETECTED`] before the first, then
/// [`HARDWARE`] or [`PORTABLE`], for every thread but those [`set_portable_fma`] sets.
#[cfg(target_arch = "x86_64")]
const UNDETECTED: u8 = 0;
#[cfg(target_arch = "x86_64")]
const HARDWARE: u8 = 1;
#[cfg(target_arch = "x86_64")]
const PORTABLE: u8 = 2;
#[cfg(target_arch = "x86_64")]
static FMA_PATH: AtomicU8 = AtomicU8::new(UNDETECTED);

thread_local! {
    static PORTABLE_HERE: Cell<bool> = const { Cell::new(false) };
}

/// How many threads have [`set_portable_fma`] on (counted as [`SLIPPI_THREADS`] is), so the
/// others need no thread-local read.
static PORTABLE_THREADS: AtomicUsize = AtomicUsize::new(0);

/// Makes this thread's fused multiply-adds take the portable path (`f64::mul_add`) even where
/// the hardware one is there, for checking the two against each other; `false` goes back to
/// the hardware one where the processor has it.
#[inline]
pub fn set_portable_fma(portable: bool) {
    let old = PORTABLE_HERE.with(|p| p.replace(portable));
    match (old, portable) {
        (false, true) => PORTABLE_THREADS.fetch_add(1, Ordering::Relaxed),
        (true, false) => PORTABLE_THREADS.fetch_sub(1, Ordering::Relaxed),
        _ => 0,
    };
}

/// `a * c + b` rounded once to double: IEEE 754's fusedMultiplyAdd, the same bits either way.
/// x86-64's FMA3 does it in one instruction where the processor has it (detected at run time);
/// elsewhere `f64::mul_add`, which is the hardware instruction on aarch64 and the C library's
/// `fma` on x86-64.
#[inline]
pub(crate) fn mul_add(a: f64, c: f64, b: f64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    if !portable_fma() {
        // SAFETY: the processor has FMA3 (`detect_fma`).
        return unsafe { mul_add_fma3(a, c, b) };
    }
    a.mul_add(c, b)
}

/// Whether this thread's fused multiply-adds take the portable path: where the processor
/// has no FMA3, or after [`set_portable_fma`].
#[inline]
pub fn portable_fma() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let mut path = FMA_PATH.load(Ordering::Relaxed);
        if path == UNDETECTED {
            path = detect_fma();
        }
        path != HARDWARE
            || (PORTABLE_THREADS.load(Ordering::Relaxed) != 0 && PORTABLE_HERE.with(Cell::get))
    }
    #[cfg(not(target_arch = "x86_64"))]
    true
}

#[cfg(target_arch = "x86_64")]
#[cold]
#[inline(never)]
fn detect_fma() -> u8 {
    let path = if std::arch::is_x86_feature_detected!("fma") {
        HARDWARE
    } else {
        PORTABLE
    };
    FMA_PATH.store(path, Ordering::Relaxed);
    path
}

/// FMA3's `vfmadd213sd`, written out so it inlines into code built without the feature.
///
/// # Safety
///
/// The processor must have FMA3.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn mul_add_fma3(a: f64, c: f64, b: f64) -> f64 {
    let mut r = a;
    // SAFETY: the caller checked FMA3; the instruction touches nothing but its registers.
    unsafe {
        std::arch::asm!(
            "vfmadd213sd {r}, {c}, {b}",
            r = inout(xmm_reg) r,
            c = in(xmm_reg) c,
            b = in(xmm_reg) b,
            options(pure, nomem, nostack, preserves_flags),
        );
    }
    r
}

#[inline]
fn madd_double(a: f64, c: f64, b: f64, negate_b: bool) -> f64 {
    let r = mul_add(a, c, if negate_b { -b } else { b });
    if r.is_nan() {
        fused_nan_result(a, c, b)
    } else {
        r
    }
}

/// `frA * frC ± frB` for single-precision fused ops, before the final rounding to single.
#[inline]
pub(crate) fn madd_single(a: f64, c: f64, b: f64, negate_b: bool) -> f64 {
    let c_round = round_c(c);
    let b_signed = if negate_b { -b } else { b };
    let r = mul_add(a, c_round, b_signed);
    if r.to_bits() & 0x1FFF_FFFF == 0x1000_0000 || r.is_nan() {
        return madd_single_rare(a, c, b, c_round, b_signed, r);
    }
    r
}

/// [`madd_single`]'s halfway and NaN results, out of line so the common path branches past
/// them instead of computing them.
#[cold]
#[inline(never)]
fn madd_single_rare(a: f64, c: f64, b: f64, c_round: f64, b_signed: f64, mut r: f64) -> f64 {
    // A double result exactly halfway between two singles may hide which way the exact
    // result lies; recover the double rounding error and nudge toward it.
    let bits = r.to_bits();
    if bits & 0x1FFF_FFFF == 0x1000_0000 && !slippi() {
        let a_prime = b_signed - r;
        let b_prime = r + a_prime;
        let error = mul_add(a, c_round, a_prime) + (b_signed - b_prime);
        if error != 0.0 {
            r = f64::from_bits(if (error > 0.0) == (r > 0.0) {
                bits + 1
            } else {
                bits - 1
            });
        }
    }

    if r.is_nan() {
        // Slippi's Dolphin picks the NaN after rounding frC, so it rounds a NaN's payload too.
        let c = if slippi() { c_round } else { c };
        fused_nan_result(a, c, b)
    } else {
        r
    }
}

/// Slippi's `fnmadd`/`fnmsub`: x86's `-(a*c) - b` and `-(a*c) + b`, one rounding to double. The
/// negation is inside the fused op, so an exact cancellation gives +0 where PowerPC gives -0.
#[inline]
pub(crate) fn nmadd_x86(a: f64, c: f64, b: f64, subtract: bool) -> f64 {
    let addend = if subtract { b } else { -b };
    let r = mul_add(-a, c, addend);
    if r.is_nan() {
        fused_nan_result(a, c, b)
    } else {
        r
    }
}

// Fused ops check NaN operands in the order frA, frB, frC.
#[cold]
#[inline(never)]
fn fused_nan_result(a: f64, c: f64, b: f64) -> f64 {
    if a.is_nan() {
        make_quiet(a)
    } else if b.is_nan() {
        make_quiet(b)
    } else if c.is_nan() {
        make_quiet(c)
    } else {
        default_nan()
    }
}

/// `frsp`: rounds a register value to single precision.
#[inline]
pub fn frsp(x: f64) -> f64 {
    if x.is_nan() {
        return frsp_nan(x);
    }
    f64::from(x as f32)
}

/// [`frsp`] of a NaN: keeps the sign and top payload bits, and sets the quiet bit.
#[cold]
#[inline(never)]
fn frsp_nan(x: f64) -> f64 {
    let bits = x.to_bits();
    f64::from_bits((bits & SIGN) | EXP | QUIET | (bits & 0x0007_FFFF_E000_0000))
}

/// `fadd`
#[inline]
pub fn fadd(a: f64, b: f64) -> f64 {
    add(a, b)
}

/// `fsub`
#[inline]
pub fn fsub(a: f64, b: f64) -> f64 {
    sub(a, b)
}

/// `fmul`
#[inline]
pub fn fmul(a: f64, c: f64) -> f64 {
    mul(a, c)
}

/// `fdiv`
#[inline]
pub fn fdiv(a: f64, b: f64) -> f64 {
    div(a, b)
}

/// `fmadd`: `a * c + b`, rounded once.
#[inline]
pub fn fmadd(a: f64, c: f64, b: f64) -> f64 {
    madd_double(a, c, b, false)
}

/// `fmsub`: `a * c - b`, rounded once.
#[inline]
pub fn fmsub(a: f64, c: f64, b: f64) -> f64 {
    madd_double(a, c, b, true)
}

/// `fnmadd`: `-(a * c + b)`.
#[inline]
pub fn fnmadd(a: f64, c: f64, b: f64) -> f64 {
    if slippi() {
        return nmadd_x86(a, c, b, false);
    }
    negate_unless_nan(madd_double(a, c, b, false))
}

/// `fnmsub`: `-(a * c - b)`.
#[inline]
pub fn fnmsub(a: f64, c: f64, b: f64) -> f64 {
    if slippi() {
        return nmadd_x86(a, c, b, true);
    }
    negate_unless_nan(madd_double(a, c, b, true))
}

/// `fadds`
#[inline]
pub fn fadds(a: f64, b: f64) -> f64 {
    frsp(add(a, b))
}

/// `fsubs`
#[inline]
pub fn fsubs(a: f64, b: f64) -> f64 {
    frsp(sub(a, b))
}

/// `fmuls`: frC is first rounded to 25 mantissa bits, as on hardware.
#[inline]
pub fn fmuls(a: f64, c: f64) -> f64 {
    frsp(mul(a, round_c(c)))
}

/// `fdivs`
#[inline]
pub fn fdivs(a: f64, b: f64) -> f64 {
    frsp(div(a, b))
}

/// `fmadds`: `a * c + b`, rounded once to single (see [`FpMode`]).
#[inline]
pub fn fmadds(a: f64, c: f64, b: f64) -> f64 {
    frsp(madd_single(a, c, b, false))
}

/// `fmsubs`: `a * c - b`, rounded once to single.
#[inline]
pub fn fmsubs(a: f64, c: f64, b: f64) -> f64 {
    frsp(madd_single(a, c, b, true))
}

/// `fnmadds`: `-(a * c + b)`, rounded once to single.
#[inline]
pub fn fnmadds(a: f64, c: f64, b: f64) -> f64 {
    if slippi() {
        return frsp(nmadd_x86(a, round_c(c), b, false));
    }
    negate_unless_nan(frsp(madd_single(a, c, b, false)))
}

/// `fnmsubs`: `-(a * c - b)`, rounded once to single.
#[inline]
pub fn fnmsubs(a: f64, c: f64, b: f64) -> f64 {
    if slippi() {
        return frsp(nmadd_x86(a, round_c(c), b, true));
    }
    negate_unless_nan(frsp(madd_single(a, c, b, true)))
}

/// `fsel`: `c` if `a >= -0.0`, else `b` (NaN picks `b`).
#[inline]
pub fn fsel(a: f64, c: f64, b: f64) -> f64 {
    if a >= -0.0 { c } else { b }
}

/// `fabs`
#[inline]
pub fn fabs(b: f64) -> f64 {
    f64::from_bits(b.to_bits() & !SIGN)
}

/// `fneg`
#[inline]
pub fn fneg(b: f64) -> f64 {
    f64::from_bits(b.to_bits() ^ SIGN)
}

/// `fnabs`
#[inline]
pub fn fnabs(b: f64) -> f64 {
    f64::from_bits(b.to_bits() | SIGN)
}

/// Result of `fcmpu` and `fcmpo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpCompare {
    Less,
    Greater,
    Equal,
    Unordered,
}

/// `fcmpu`/`fcmpo`
#[inline]
pub fn fcmp(a: f64, b: f64) -> FpCompare {
    if a.is_nan() || b.is_nan() {
        FpCompare::Unordered
    } else if a < b {
        FpCompare::Less
    } else if a > b {
        FpCompare::Greater
    } else {
        FpCompare::Equal
    }
}

#[inline]
fn convert_to_integer(b: f64, rounded: f64) -> i32 {
    if b.is_nan() || rounded < -2_147_483_648.0 {
        i32::MIN
    } else if rounded >= 2_147_483_648.0 {
        i32::MAX
    } else {
        rounded as i32
    }
}

/// `fctiwz`: converts to an integer, rounding toward zero and saturating (NaN gives `i32::MIN`).
#[inline]
pub fn fctiwz(b: f64) -> i32 {
    convert_to_integer(b, b.trunc())
}

/// `fctiw`: converts to an integer, rounding to nearest even and saturating.
#[inline]
pub fn fctiw(b: f64) -> i32 {
    let int_precision = 4_503_599_627_370_496.0_f64.copysign(b);
    convert_to_integer(b, (b + int_precision) - int_precision)
}

/// The full register image `fctiw`/`fctiwz` leave for a following `stfd`, given its result.
#[inline]
pub fn fcti_bits(b: f64, value: i32) -> u64 {
    let mut bits = 0xFFF8_0000_0000_0000 | u64::from(value as u32);
    // Hardware marks a negative-zero input; Slippi's Dolphin does not.
    if value == 0 && b.is_sign_negative() && !slippi() {
        bits |= 0x1_0000_0000;
    }
    bits
}

/// `lfs`: widens a single from memory into a register value.
#[inline]
pub fn lfs(bits: u32) -> f64 {
    if bits & 0x7F80_0000 != 0x7F80_0000 {
        // A number, zero or subnormal included: IEEE widening gives the same exact value.
        return f64::from(f32::from_bits(bits));
    }
    lfs_special(bits)
}

/// [`lfs`] of an infinity or NaN, whose payload moves over unquieted.
#[cold]
#[inline(never)]
fn lfs_special(bits: u32) -> f64 {
    let x = u64::from(bits);
    let exp = (x >> 23) & 0xFF;
    let mut frac = x & 0x007F_FFFF;
    let bits = if exp > 0 && exp < 255 {
        let y = u64::from(exp >> 7 == 0);
        ((x & 0xC000_0000) << 32) | (y << 61) | (y << 60) | (y << 59) | ((x & 0x3FFF_FFFF) << 29)
    } else if exp == 0 && frac != 0 {
        let mut exp = 1023 - 126;
        loop {
            frac <<= 1;
            exp -= 1;
            if frac & 0x0080_0000 != 0 {
                break;
            }
        }
        ((x & 0x8000_0000) << 32) | (exp << 52) | ((frac & 0x007F_FFFF) << 29)
    } else {
        let y = exp >> 7;
        ((x & 0xC000_0000) << 32) | (y << 61) | (y << 60) | (y << 59) | ((x & 0x3FFF_FFFF) << 29)
    };
    f64::from_bits(bits)
}

/// Slippi's float stores: x86's conversion, which rounds to nearest. `keep_snan` restores a
/// signaling NaN's quiet bit, as its `stfs` does and its `psq_st` does not.
#[inline]
fn x86_to_single(x: f64, keep_snan: bool) -> u32 {
    if x.is_nan() {
        let bits = x.to_bits();
        let s =
            ((bits >> 32) as u32 & 0x8000_0000) | 0x7FC0_0000 | ((bits >> 29) as u32 & 0x003F_FFFF);
        if keep_snan && bits & QUIET == 0 {
            s & !0x0040_0000
        } else {
            s
        }
    } else {
        (x as f32).to_bits()
    }
}

/// `stfs`: narrows a register value for memory. Hardware truncates (round first with
/// [`frsp`]); Slippi's Dolphin rounds to nearest.
#[inline]
pub fn stfs(x: f64) -> u32 {
    if slippi() {
        return x86_to_single(x, true);
    }
    let x = x.to_bits();
    let exp = (x >> 52) & 0x7FF;
    if (874..=896).contains(&exp) && x & !SIGN != 0 {
        let t = (0x8000_0000 | ((x & FRAC) >> 21) as u32) >> (905 - exp);
        t | ((x >> 32) & 0x8000_0000) as u32
    } else {
        // Exponents below 874 are undefined; this matches hardware tests.
        (((x >> 32) & 0xC000_0000) | ((x >> 29) & 0x3FFF_FFFF)) as u32
    }
}

/// The float conversion `psq_st` uses: like [`stfs`], but single subnormals become zero
/// (Slippi's Dolphin keeps them and rounds).
#[inline]
pub fn stfs_ftz(x: f64) -> u32 {
    if slippi() {
        return x86_to_single(x, false);
    }
    let x = x.to_bits();
    let exp = (x >> 52) & 0x7FF;
    if exp > 896 || x & !SIGN == 0 {
        (((x >> 32) & 0xC000_0000) | ((x >> 29) & 0x3FFF_FFFF)) as u32
    } else {
        ((x >> 32) & 0x8000_0000) as u32
    }
}
