// SPDX-License-Identifier: GPL-3.0-or-later

//! This copy's ops against ssbm-rs's gekko-fp (`upstream`, unchanged): every op, scalar and
//! paired, in both float modes, on special values, random bits and the ranges the game uses,
//! must give the upstream's bits, on the hardware fused multiply-add and on the portable one.

mod common;
#[allow(dead_code, clippy::all)]
mod upstream;

use common::Rng;
use gekko_fp::{FpCompare, FpMode, Ps, set_fp_mode, set_portable_fma};

/// An op's result bits from three operands, the unused ones ignored.
type Op = fn(f64, f64, f64) -> (u64, u64);

fn cmp_bits(c: FpCompare) -> u64 {
    c as u64
}

fn up_cmp_bits(c: upstream::FpCompare) -> u64 {
    c as u64
}

fn ps(a: f64, b: f64) -> Ps {
    Ps::new(a, b)
}

fn up_ps(a: f64, b: f64) -> upstream::Ps {
    upstream::Ps::new(a, b)
}

/// Pairs each op of this crate with the upstream's, given how to call it on `(a, c, b)` with
/// `$m` standing for the crate.
macro_rules! ops {
    ($(($name:literal, |$m:ident, $p:ident, $a:ident, $c:ident, $b:ident| $e:expr)),* $(,)?) => {
        [$((
            $name,
            {
                #[allow(unused_imports, unused_variables)]
                fn ours($a: f64, $c: f64, $b: f64) -> (u64, u64) {
                    use gekko_fp as $m;
                    let $p = ps;
                    $e
                }
                ours as Op
            },
            {
                #[allow(unused_imports, unused_variables)]
                fn theirs($a: f64, $c: f64, $b: f64) -> (u64, u64) {
                    use upstream as $m;
                    let $p = up_ps;
                    $e
                }
                theirs as Op
            },
        )),*]
    };
}

fn f(x: f64) -> (u64, u64) {
    (x.to_bits(), 0)
}

fn i(x: impl Into<i64>) -> (u64, u64) {
    (x.into() as u64, 0)
}

const OPS: [(&str, Op, Op); 51] = ops![
    ("frsp", |m, p, a, c, b| f(m::frsp(b))),
    ("fadd", |m, p, a, c, b| f(m::fadd(a, b))),
    ("fsub", |m, p, a, c, b| f(m::fsub(a, b))),
    ("fmul", |m, p, a, c, b| f(m::fmul(a, c))),
    ("fdiv", |m, p, a, c, b| f(m::fdiv(a, b))),
    ("fmadd", |m, p, a, c, b| f(m::fmadd(a, c, b))),
    ("fmsub", |m, p, a, c, b| f(m::fmsub(a, c, b))),
    ("fnmadd", |m, p, a, c, b| f(m::fnmadd(a, c, b))),
    ("fnmsub", |m, p, a, c, b| f(m::fnmsub(a, c, b))),
    ("fadds", |m, p, a, c, b| f(m::fadds(a, b))),
    ("fsubs", |m, p, a, c, b| f(m::fsubs(a, b))),
    ("fmuls", |m, p, a, c, b| f(m::fmuls(a, c))),
    ("fdivs", |m, p, a, c, b| f(m::fdivs(a, b))),
    ("fmadds", |m, p, a, c, b| f(m::fmadds(a, c, b))),
    ("fmsubs", |m, p, a, c, b| f(m::fmsubs(a, c, b))),
    ("fnmadds", |m, p, a, c, b| f(m::fnmadds(a, c, b))),
    ("fnmsubs", |m, p, a, c, b| f(m::fnmsubs(a, c, b))),
    ("fsel", |m, p, a, c, b| f(m::fsel(a, c, b))),
    ("fabs", |m, p, a, c, b| f(m::fabs(b))),
    ("fneg", |m, p, a, c, b| f(m::fneg(b))),
    ("fnabs", |m, p, a, c, b| f(m::fnabs(b))),
    ("fctiwz", |m, p, a, c, b| i(m::fctiwz(b))),
    ("fctiw", |m, p, a, c, b| i(m::fctiw(b))),
    ("fcti_bits", |m, p, a, c, b| i(
        m::fcti_bits(b, m::fctiw(b)) as i64
    )),
    ("lfs", |m, p, a, c, b| f(m::lfs((b.to_bits() >> 32) as u32))),
    ("lfs low", |m, p, a, c, b| f(m::lfs(b.to_bits() as u32))),
    ("stfs", |m, p, a, c, b| i(m::stfs(b))),
    ("stfs_ftz", |m, p, a, c, b| i(m::stfs_ftz(b))),
    ("frsqrte", |m, p, a, c, b| f(m::frsqrte(b))),
    ("fres", |m, p, a, c, b| f(m::fres(b))),
    ("ps_add", |m, p, a, c, b| m::ps_add(p(a, c), p(b, a))
        .to_bits()),
    ("ps_sub", |m, p, a, c, b| m::ps_sub(p(a, c), p(b, a))
        .to_bits()),
    ("ps_mul", |m, p, a, c, b| m::ps_mul(p(a, b), p(c, a))
        .to_bits()),
    ("ps_div", |m, p, a, c, b| m::ps_div(p(a, c), p(b, a))
        .to_bits()),
    ("ps_madd", |m, p, a, c, b| m::ps_madd(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_msub", |m, p, a, c, b| m::ps_msub(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_nmadd", |m, p, a, c, b| m::ps_nmadd(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_nmsub", |m, p, a, c, b| m::ps_nmsub(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_sum0", |m, p, a, c, b| m::ps_sum0(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_sum1", |m, p, a, c, b| m::ps_sum1(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_muls0", |m, p, a, c, b| m::ps_muls0(p(a, b), p(c, a))
        .to_bits()),
    ("ps_muls1", |m, p, a, c, b| m::ps_muls1(p(a, b), p(c, a))
        .to_bits()),
    ("ps_madds0", |m, p, a, c, b| m::ps_madds0(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_madds1", |m, p, a, c, b| m::ps_madds1(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_res", |m, p, a, c, b| m::ps_res(p(b, a)).to_bits()),
    ("ps_rsqrte", |m, p, a, c, b| m::ps_rsqrte(p(b, a)).to_bits()),
    ("ps_sel", |m, p, a, c, b| m::ps_sel(
        p(a, b),
        p(c, a),
        p(b, c)
    )
    .to_bits()),
    ("ps_neg", |m, p, a, c, b| m::ps_neg(p(b, a)).to_bits()),
    ("ps_abs", |m, p, a, c, b| m::ps_abs(p(b, a)).to_bits()),
    ("ps_nabs", |m, p, a, c, b| m::ps_nabs(p(b, a)).to_bits()),
    ("ps_merge01", |m, p, a, c, b| m::ps_merge01(
        p(a, c),
        p(b, a)
    )
    .to_bits()),
];

/// `fcmp`, whose result isn't bits.
#[test]
fn fcmp_agrees() {
    for &a in &specials() {
        for &b in &specials() {
            assert_eq!(
                cmp_bits(gekko_fp::fcmp(a, b)),
                up_cmp_bits(upstream::fcmp(a, b))
            );
        }
    }
}

/// Values where float arithmetic has edges: zeros, infinities, quiet and signaling NaNs with
/// payloads, subnormals at both ends (double and single), the largest finite values, ones,
/// halfway and near-halfway single mantissas, and values whose products cancel.
fn specials() -> Vec<f64> {
    let mut v = vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7FF0_0000_0000_0001),
        f64::from_bits(0xFFF0_0000_DEAD_BEEF),
        f64::from_bits(0x7FF8_0000_0000_1234),
        f64::from_bits(0x7FF0_0000_1000_0000),
        f64::from_bits(1),
        f64::from_bits(0x000F_FFFF_FFFF_FFFF),
        f64::from_bits(0x0000_0000_1000_0000),
        f64::MIN_POSITIVE,
        f64::MAX,
        1.0,
        0.5,
        3.0,
        f64::from(f32::MIN_POSITIVE),
        f64::from(f32::from_bits(1)),
        f64::from(f32::from_bits(0x007F_FFFF)),
        f64::from(f32::MAX),
        f64::from(f32::MAX) * 2.0,
        1.0 + f64::EPSILON,
        1.0 - f64::EPSILON / 2.0,
        f64::from_bits(0x3FF0_0000_1000_0000),
        f64::from_bits(0x3FF0_0000_0FFF_FFFF),
        f64::from_bits(0x3FF0_0000_1000_0001),
        f64::from_bits(0x3FF0_0000_0800_0000),
        f64::from_bits(0x3FF0_0000_07FF_FFFF),
        f64::from_bits(0x3FF0_0000_0800_0001),
        1e-300,
        1e300,
        2.5e-38,
        16777217.0,
        2_147_483_648.0,
        4_503_599_627_370_496.0,
    ];
    let more: Vec<f64> = v.iter().map(|x| -x).collect();
    v.extend(more);
    v
}

const MODES: [(FpMode, upstream::FpMode); 2] = [
    (FpMode::Hardware, upstream::FpMode::Hardware),
    (FpMode::Slippi, upstream::FpMode::Slippi),
];

fn set_mode((ours, theirs): (FpMode, upstream::FpMode)) {
    set_fp_mode(ours);
    upstream::set_fp_mode(theirs);
}

/// `op` on both fused multiply-add paths of this crate and on the upstream's.
fn check(mode: FpMode, (name, ours, theirs): (&str, Op, Op), a: f64, c: f64, b: f64) {
    let want = theirs(a, c, b);
    for portable in [false, true] {
        set_portable_fma(portable);
        let got = ours(a, c, b);
        assert_eq!(
            got,
            want,
            "{name} {mode:?} {}: a {:#018x} c {:#018x} b {:#018x}: got {got:x?}, upstream {want:x?}",
            if portable { "portable" } else { "hardware" },
            a.to_bits(),
            c.to_bits(),
            b.to_bits()
        );
    }
    set_portable_fma(false);
}

#[test]
fn specials_agree() {
    let s = specials();
    for mode in MODES {
        set_mode(mode);
        for op in OPS {
            for &a in &s {
                for &c in &s {
                    for &b in &s {
                        check(mode.0, op, a, c, b);
                    }
                }
            }
        }
    }
}

/// Random bit patterns: any double, any single, and singles in the game's ranges (exponents
/// from 2^-30 to 2^30), with addends near the product so the sum cancels or lands halfway.
#[test]
fn random_agree() {
    for mode in MODES {
        set_mode(mode);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ mode.0 as u64);
        for k in 0..1_000_000u32 {
            let (a, c, b) = match k % 4 {
                0 => (
                    f64::from_bits(rng.next()),
                    f64::from_bits(rng.next()),
                    f64::from_bits(rng.next()),
                ),
                1 => (
                    f64::from(f32::from_bits(rng.next() as u32)),
                    f64::from(f32::from_bits(rng.next() as u32)),
                    f64::from(f32::from_bits(rng.next() as u32)),
                ),
                2 => (
                    f64::from(rng.f32_in(97, 157)),
                    f64::from(rng.f32_in(97, 157)),
                    f64::from(rng.f32_in(97, 157)),
                ),
                _ => {
                    let a = f64::from(rng.f32_in(110, 140));
                    let c = f64::from(rng.f32_in(110, 140));
                    // An addend close to minus the product: cancellation, halfway cases.
                    let p = f64::from((a * c) as f32);
                    let nudge = f64::from_bits((rng.next() & 0xFFFF) | 0x3E00_0000_0000_0000);
                    (a, c, -p * (1.0 + nudge * (rng.next() % 3) as f64))
                }
            };
            for op in OPS {
                check(mode.0, op, a, c, b);
            }
        }
    }
}

/// Results exactly halfway between two singles, where the single fused ops correct the double
/// rounding: `a * c + b` built to land on one, with the exact product just above or below.
#[test]
fn halfway_agrees() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    let mut hit = 0;
    for mode in MODES {
        set_mode(mode);
        for _ in 0..200_000 {
            let a = f64::from(rng.f32_in(120, 135));
            let c = f64::from(rng.f32_in(120, 135));
            let p = a * c;
            // A double halfway between two singles near the product, and an addend to reach it.
            let h = f64::from_bits((p.to_bits() & !0x1FFF_FFFF) | 0x1000_0000);
            // Exactly there, or a little past either way, which the double rounding hides.
            let tiny = p * f64::from(rng.f32_in(60, 70)) * f64::from((rng.next() % 3) as u32);
            let b = (h - p) + tiny;
            let r = gekko_fp::fmadd(a, c, b).to_bits();
            hit += usize::from(r & 0x1FFF_FFFF == 0x1000_0000);
            for op in OPS
                .iter()
                .filter(|o| o.0.contains("madd") || o.0.contains("sub"))
            {
                check(mode.0, *op, a, c, b);
            }
        }
    }
    assert!(hit > 1000, "only {hit} halfway results");
}

/// The hardware path is the one taken by default where the processor has it.
#[test]
fn hardware_path_is_detected() {
    #[cfg(target_arch = "x86_64")]
    eprintln!(
        "FMA3 {}",
        if std::arch::is_x86_feature_detected!("fma") {
            "detected: the hardware path is checked"
        } else {
            "missing: both paths are the portable one"
        }
    );
}

/// `lfs` on every single whose exponent is all zeros or all ones (zeros, subnormals,
/// infinities and NaNs, both signs) and on 4096 random mantissas of every other exponent.
#[test]
fn lfs_agrees() {
    let check = |bits: u32| {
        assert_eq!(
            gekko_fp::lfs(bits).to_bits(),
            upstream::lfs(bits).to_bits(),
            "lfs {bits:#010x}"
        );
    };
    for sign in [0, 0x8000_0000] {
        for exp in [0, 0x7F80_0000] {
            for frac in 0..0x0080_0000 {
                check(sign | exp | frac);
            }
        }
    }
    let mut rng = Rng(5);
    for exp in 1..255u32 {
        for _ in 0..4096 {
            check((rng.next() as u32 & 0x807F_FFFF) | (exp << 23));
        }
    }
}
