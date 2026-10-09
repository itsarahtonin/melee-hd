//! How long the fused single ops take on each fused multiply-add path, against a bare
//! `mul_add` and rounding (`cargo run --release --example fused_speed`).

use gekko_fp::{fmadds, fnmsubs, set_portable_fma};
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

fn main() {
    floor();
    for _ in 0..2 {
        set_portable_fma(false);
        run("hardware");
        set_portable_fma(true);
        run("portable");
    }
}
