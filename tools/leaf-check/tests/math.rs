//! melee-sim's math and random numbers against ssbm-rs's ports, bit for bit, in both float
//! modes.

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{arg, bits, inputs, reference, sym};
use melee_sim::{math, rng::Rng};
use ssbm_types::fns;

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];

fn check_unary(
    name: &str,
    ours: fn(f32) -> f32,
    theirs: fn(&ssbm_rt::Ctx, f64) -> f64,
    range: f32,
) {
    let Some(ctx) = reference() else { return };
    for mode in MODES {
        set_fp_mode(mode);
        for x in inputs(range, 50_000) {
            let want = bits(theirs(&ctx, arg(x)));
            let got = ours(x).to_bits();
            assert_eq!(
                got,
                want,
                "{name}({x:e} = {:#010x}) in {mode:?}",
                x.to_bits()
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn sin_matches() {
    check_unary("sinf", math::sin, fns::sinf, 20.0);
}

#[test]
fn cos_matches() {
    check_unary("cosf", math::cos, fns::cosf, 20.0);
}

#[test]
fn tan_matches() {
    check_unary("tanf", math::tan, fns::tanf, 4.0);
}

#[test]
fn sqrt_matches() {
    check_unary("sqrtf", math::sqrt, fns::sqrtf__Ff, 1.0e4);
}

#[test]
fn atan_matches() {
    check_unary("atanf", math::atan, fns::atanf, 8.0);
}

#[test]
fn acos_matches() {
    check_unary("acosf", math::acos, fns::acosf, 1.2);
}

#[test]
fn asin_matches() {
    check_unary("asinf", math::asin, fns::asinf, 1.2);
}

#[test]
fn atan2_matches() {
    let Some(ctx) = reference() else { return };
    let xs = inputs(100.0, 20_000);
    let ys = inputs(100.0, 20_000);
    for mode in MODES {
        set_fp_mode(mode);
        for (i, &y) in ys.iter().enumerate() {
            // Every special value against every other, then pairs.
            let others: Vec<f32> = if i < 18 {
                xs[..18].to_vec()
            } else {
                vec![xs[i]]
            };
            for x in others {
                let want = bits(fns::atan2f(&ctx, arg(y), arg(x)));
                let got = math::atan2(y, x).to_bits();
                assert_eq!(got, want, "atan2f({y:e}, {x:e}) in {mode:?}");
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn rng_matches() {
    let Some(ctx) = reference() else { return };
    let seed = sym("seed");
    let mut ours = Rng::default();
    ctx.write_u32(seed, ours.seed);
    let maxes = [1, 2, 3, 7, 100, 0x7FFF, 0x10000, -5, i32::MAX, i32::MIN];
    for i in 0..200_000usize {
        match i % 3 {
            0 => assert_eq!(ours.draw(), fns::HSD_Rand(&ctx), "HSD_Rand at draw {i}"),
            1 => assert_eq!(
                ours.draw_f32().to_bits(),
                bits(fns::HSD_Randf(&ctx)),
                "HSD_Randf at draw {i}"
            ),
            _ => {
                let max = maxes[i % maxes.len()];
                assert_eq!(
                    ours.below(max),
                    fns::HSD_Randi(&ctx, max),
                    "HSD_Randi({max}) at draw {i}"
                );
            }
        }
        assert_eq!(ours.seed, ctx.read_u32(seed), "seed after draw {i}");
    }
}

#[test]
fn log_tables_match() {
    let Some(ctx) = reference() else { return };
    for (name, table) in [("__ln_F", &math::LN_F), ("__one_over_F", &math::ONE_OVER_F)] {
        let at = sym(name);
        for (i, &v) in table.iter().enumerate() {
            assert_eq!(v, ctx.read_u32(at + 4 * i as u32), "{name}[{i}]");
        }
    }
}

#[test]
fn log_matches() {
    check_unary("logf", math::log, fns::logf, 1.0e4);
    // Every table entry, both rounding directions, both signs.
    let Some(ctx) = reference() else { return };
    for mode in MODES {
        set_fp_mode(mode);
        for exp in [0x00u32, 0x01, 0x3F, 0x7E, 0x7F, 0x80, 0xFE, 0xFF] {
            for m in 0..0x80u32 {
                for low in [0u32, 1, 0x7FFF, 0x8000, 0xFFFF] {
                    for sign in [0, 0x8000_0000] {
                        let x = f32::from_bits(sign | exp << 23 | m << 16 | low);
                        let want = bits(fns::logf(&ctx, arg(x)));
                        assert_eq!(math::log(x).to_bits(), want, "logf({x:e}) in {mode:?}");
                    }
                }
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

/// Whether the original's `expf` returns for `x`. Its series never settles for NaN, nor
/// where the terms overflow on the step the factorial does or the one before (|x| from
/// about 12.7 to 14.7: infinity over infinity), and loops forever there; the game doesn't
/// pass such values.
fn exp_returns(x: f32) -> bool {
    !x.is_nan() && !(12.0..=15.0).contains(&x.abs())
}

#[test]
fn exp_matches() {
    let Some(ctx) = reference() else { return };
    for mode in MODES {
        set_fp_mode(mode);
        for x in inputs(12.0, 50_000).into_iter().filter(|&x| exp_returns(x)) {
            let want = bits(fns::expf(&ctx, arg(x)));
            assert_eq!(math::exp(x).to_bits(), want, "expf({x:e}) in {mode:?}");
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn pow_matches() {
    let Some(ctx) = reference() else { return };
    let xs = inputs(20.0, 5_000);
    let mut r = leaf_check::Rand(0x5DEE_CE66_D1CE_4E5B);
    for mode in MODES {
        set_fp_mode(mode);
        for &x in &xs {
            // Bases from the special values and the range, and exponents that keep the
            // result's exponential where expf returns (`exp_returns`). A negative base
            // diverges to an infinite logarithm, which an exponent of 0 turns into NaN.
            if !x.is_finite() || x.abs() > 20.0 || (x.abs() < 0.05 && x != 0.0) {
                continue;
            }
            for _ in 0..4 {
                let y = r.range(-3.0, 3.0);
                if x < 0.0 && y == 0.0 {
                    continue;
                }
                let want = bits(fns::powf(&ctx, arg(x), arg(y)));
                assert_eq!(
                    math::pow(x, y).to_bits(),
                    want,
                    "powf({x:e}, {y:e}) in {mode:?}"
                );
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn conversions_match() {
    use melee_sim::ppc;
    let Some(ctx) = reference() else { return };
    let mut r = leaf_check::Rand(0x2545_F491_4F6C_DD1D);
    let specials = [
        0.0f64,
        -0.0,
        0.5,
        -0.5,
        1.0,
        -1.0,
        1.5,
        -2.5,
        4503599627370496.0,
        9.223_372_036_854_776e18,
        -9.223_372_036_854_776e18,
        1.0e300,
        -1.0e300,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::MIN_POSITIVE,
    ];
    for mode in MODES {
        set_fp_mode(mode);
        for i in 0..200_000 {
            let d = match i {
                i if i < specials.len() => specials[i],
                i if i % 3 == 0 => f64::from_bits(r.bits()),
                i if i % 3 == 1 => (r.bits() as i64 >> (r.bits() % 64)) as f64 * 0.75,
                _ => f64::from(r.range(-1.0e6, 1.0e6)),
            };
            assert_eq!(
                ppc::cvt_dbl_usll(d),
                fns::__cvt_dbl_usll(&ctx, d),
                "__cvt_dbl_usll({d:e}) in {mode:?}"
            );
            let x = r.bits() as i64 >> (r.bits() % 64);
            assert_eq!(
                ppc::cvt_sll_flt(x).to_bits(),
                fns::__cvt_sll_flt(&ctx, x).to_bits(),
                "__cvt_sll_flt({x}) in {mode:?}"
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
}
