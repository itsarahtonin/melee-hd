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
