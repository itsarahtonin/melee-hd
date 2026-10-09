//! How long the fused single ops take on each fused multiply-add path, against a bare
//! `mul_add` and rounding (`cargo run --release --example fused_speed`).

use gekko_fp::{Ps, fmadds, fnmsubs, ps_madd, ps_madds0, set_portable_fma};
use std::hint::black_box;
use std::time::Instant;

fn run(label: &str) {
    let n = 20_000_000u32;
    let mut x = 1.0f64;
    let t = Instant::now();
    for i in 0..n {
        let a = black_box(1.000_001 + f64::from(i & 7));
        x = fmadds(a, black_box(0.999_999), x * 0.5);
        x = fnmsubs(black_box(0.25), x, a);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / f64::from(2 * n);
    println!("{label}: {ns:.2} ns per op ({x})");
}

fn floor() {
    let n = 20_000_000u32;
    let mut x = 1.0f64;
    let t = Instant::now();
    for i in 0..n {
        let a = black_box(1.000_001 + f64::from(i & 7));
        x = f64::from(a.mul_add(black_box(0.999_999), x * 0.5) as f32);
        x = f64::from(-(0.25f64.mul_add(x, -a)) as f32);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / f64::from(2 * n);
    println!("bare mul_add + frsp: {ns:.2} ns per op ({x})");
}

/// Chained paired ops: each lane's result feeds the next op's frC.
fn paired(label: &str) {
    let n = 20_000_000u32;
    let mut x = Ps::new(1.0, 2.0);
    let t = Instant::now();
    for i in 0..n {
        let a = black_box(Ps::splat(0.5 + f64::from(i & 7) / 64.0));
        x = ps_madd(a, x, black_box(Ps::new(0.25, 0.5)));
        x = ps_madds0(black_box(Ps::new(0.5, 0.25)), x, a);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / f64::from(2 * n);
    println!("{label} paired: {ns:.2} ns per op ({x:?})");
}

fn main() {
    floor();
    for _ in 0..2 {
        set_portable_fma(false);
        run("hardware");
        paired("hardware");
        set_portable_fma(true);
        run("portable");
        paired("portable");
    }
}
