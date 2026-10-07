//! melee-sim's splines against ssbm-rs's `splGetSplinePoint`, `splArcLengthGetParameter` and
//! `splArcLengthPoint`, bit for bit in both float modes, including what they leave in their
//! output vector.
//!
//! The disc's splines are the stages' paths (`map_head`'s spline table in Gr*.dat): linear,
//! B-spline and cardinal curves, no Bézier. Built splines cover Bézier curves and degenerate
//! data: coincident control points, zero-length segments, zero tension and total length.

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{Rand, dat, handle, load_archive, read_f32s, reference, write_f32s};
use melee_data::archive::{Archive, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::spline;
use melee_sim::vec::Vec3;
use ssbm_rt::Ctx;
use ssbm_types::fns;

const OUT: u32 = leaf_check::SCRATCH;
/// What the output vector holds before a call, to see which words a call writes.
const SENTINEL: u32 = 0x7FBA_DBAD;

fn next_up(x: f32) -> f32 {
    f32::from_bits(if x >= 0.0 {
        x.to_bits() + 1
    } else {
        x.to_bits() - 1
    })
}

fn next_down(x: f32) -> f32 {
    if x == 0.0 {
        return -f32::from_bits(1);
    }
    f32::from_bits(if x > 0.0 {
        x.to_bits() - 1
    } else {
        x.to_bits() + 1
    })
}

/// Parameters worth trying on a spline: the ends and just past them, every segment
/// boundary and its neighbours, special values, values in range and random bit patterns.
fn params(r: &mut Rand, boundaries: &[f32]) -> Vec<f32> {
    let mut v = vec![
        0.0,
        -0.0,
        1.0,
        0.5,
        next_down(1.0),
        next_up(1.0),
        next_up(0.0),
        next_down(0.0),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0,
        2.0,
    ];
    for &b in boundaries {
        v.extend([b, next_up(b), next_down(b)]);
    }
    for _ in 0..200 {
        v.push(r.range(0.0, 1.0));
    }
    for _ in 0..20 {
        v.push(f32::from_bits(r.bits() as u32));
    }
    v
}

fn out_words(p: Vec3) -> Vec<u32> {
    vec![p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// One spline compared at every parameter in both float modes; returns the comparisons made.
fn check_spline(
    ctx: &Ctx,
    archives: &Archives,
    ours: DataRef,
    theirs: u32,
    r: &mut Rand,
    what: &str,
) -> usize {
    let numcv = archives.u16(ours.at(2)) as i16;
    let segs = archives.ptr(ours.at(0x10)).unwrap();
    let knots: Vec<f32> = (0..numcv).map(|k| k as f32 / (numcv - 1) as f32).collect();
    let lengths: Vec<f32> = (0..numcv as u32)
        .map(|k| archives.f32(segs.at(4 * k)))
        .collect();
    // splArcLengthGetParameter finds the segment by scanning the table until an entry
    // reaches the parameter; every table ends at 1, so parameters below 1 stay inside it.
    assert_eq!(
        lengths.last(),
        Some(&1.0),
        "{what}: the length table ends at 1"
    );
    let us = params(r, &knots);
    let ss = params(r, &lengths);
    let sentinel = Vec3::new(
        f32::from_bits(SENTINEL),
        f32::from_bits(SENTINEL),
        f32::from_bits(SENTINEL),
    );
    let mut n = 0;
    for mode in [FpMode::Hardware, FpMode::Slippi] {
        set_fp_mode(mode);
        for &u in &us {
            write_f32s(ctx, OUT, &[sentinel.x, sentinel.y, sentinel.z]);
            fns::splGetSplinePoint(
                ctx,
                handle(ctx, OUT),
                handle(ctx, theirs),
                leaf_check::arg(u),
            );
            let got = spline::point(archives, ours, u).unwrap_or(sentinel);
            assert_eq!(
                out_words(got),
                read_f32s(ctx, OUT, 3),
                "{what}: splGetSplinePoint({u:e}) in {mode:?}"
            );
            n += 1;
        }
        for &s in &ss {
            let want = leaf_check::bits(fns::splArcLengthGetParameter(
                ctx,
                handle(ctx, theirs),
                leaf_check::arg(s),
            ));
            let got = spline::arc_length_parameter(archives, ours, s).to_bits();
            assert_eq!(
                got, want,
                "{what}: splArcLengthGetParameter({s:e}) in {mode:?}"
            );
            write_f32s(ctx, OUT, &[sentinel.x, sentinel.y, sentinel.z]);
            fns::splArcLengthPoint(
                ctx,
                handle(ctx, OUT),
                handle(ctx, theirs),
                leaf_check::arg(s),
            );
            let got = spline::arc_length_point(archives, ours, s).unwrap_or(sentinel);
            assert_eq!(
                out_words(got),
                read_f32s(ctx, OUT, 3),
                "{what}: splArcLengthPoint({s:e}) in {mode:?}"
            );
            n += 2;
        }
    }
    set_fp_mode(FpMode::Hardware);
    n
}

#[test]
fn disc_splines_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut r = Rand(0x9E37_79B9_7F4A_7C15);
    let (mut splines, mut kinds, mut checks) = (0, [0; 4], 0);
    for name in disc.files().unwrap() {
        if !(name.starts_with("Gr") && name.ends_with(".dat")) {
            continue;
        }
        let bytes = disc.read_file(&name).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        // map_head: +0x10 an array of spline pointers, +0x14 its length.
        let Some(head) = archive.public("map_head") else {
            continue;
        };
        let Some(table) = archive.ptr(head + 0x10) else {
            continue;
        };
        let count = archive.u32(head + 0x14);
        let data_addr = load_archive(ctx, &bytes);
        let mut archives = Archives::default();
        let id = archives.add(archive);
        for i in 0..count {
            let Some(at) = archives.ptr(DataRef {
                archive: id,
                offset: table + 4 * i,
            }) else {
                continue;
            };
            let kind = archives.u8(at);
            kinds[kind as usize] += 1;
            let what = format!("{name} spline {i} (type {kind})");
            checks += check_spline(ctx, &archives, at, data_addr + at.offset, &mut r, &what);
            splines += 1;
        }
    }
    eprintln!("{splines} disc splines ({kinds:?} by type), {checks} comparisons");
    assert!(splines >= 39, "only {splines} splines");
}

/// A spline of `kind` with `numcv` points as a DAT file: the HSD_Spline at offset 0, then
/// its control points, length table and speed polynomials.
fn built(r: &mut Rand, kind: u8, numcv: i16, degenerate: bool) -> Vec<u8> {
    let n = numcv as usize;
    let cvs = match kind {
        0 => n,
        1 => 3 * (n - 1) + 1,
        _ => n + 2,
    };
    let mut data = vec![0u8; 0x18];
    let put = |data: &mut Vec<u8>, v: f32| data.extend_from_slice(&v.to_be_bytes());
    let cv_at = data.len() as u32;
    let first = [
        r.range(-100.0, 100.0),
        r.range(-100.0, 100.0),
        r.range(-100.0, 100.0),
    ];
    for i in 0..cvs {
        for c in first {
            let v = if degenerate && i % 2 == 0 {
                c
            } else {
                r.range(-200.0, 200.0)
            };
            put(&mut data, v);
        }
    }
    // Arc lengths: from 0 to exactly 1, nondecreasing, with repeats when degenerate.
    let seg_at = data.len() as u32;
    let mut cuts: Vec<f32> = (0..n - 2).map(|_| r.range(0.0, 1.0)).collect();
    if degenerate && !cuts.is_empty() {
        cuts[0] = 0.0;
        if cuts.len() > 1 {
            cuts[1] = cuts[cuts.len() - 1];
        }
    }
    cuts.sort_by(f32::total_cmp);
    put(&mut data, 0.0);
    for c in cuts {
        put(&mut data, c);
    }
    put(&mut data, 1.0);
    let poly_at = data.len() as u32;
    for _ in 0..n {
        let scale = if degenerate { 0.0 } else { 1.0e3 };
        for k in 0..5 {
            // Mostly positive speeds; some at or around -0.001, above which the original
            // clamps a negative speed to 0, and some well below.
            let v = match (k, r.below(8)) {
                (4, 0) => [-0.0005, -0.001, next_up(-0.001), next_down(-0.001), -0.0015]
                    [r.below(5) as usize],
                (4, 1) => r.range(-10.0, 0.0),
                (4, _) => r.range(1.0, 1.0e4),
                _ => r.range(-scale, scale),
            };
            put(&mut data, v);
        }
    }
    let tension = match r.below(4) {
        0 => 0.0,
        1 => 1.0,
        2 => 0.5,
        _ => r.range(-1.0, 2.0),
    };
    let total = if degenerate { 0.0 } else { r.range(1.0, 1.0e4) };
    data[0] = kind;
    data[2..4].copy_from_slice(&numcv.to_be_bytes());
    data[4..8].copy_from_slice(&tension.to_be_bytes());
    data[8..0xC].copy_from_slice(&cv_at.to_be_bytes());
    data[0xC..0x10].copy_from_slice(&total.to_be_bytes());
    data[0x10..0x14].copy_from_slice(&seg_at.to_be_bytes());
    data[0x14..0x18].copy_from_slice(&poly_at.to_be_bytes());
    dat(&data, &[8, 0x10, 0x14])
}

#[test]
fn built_splines_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let mut r = Rand(0x2545_F491_4F6C_DD1D);
    let mut checks = 0;
    for i in 0..160 {
        let kind = (i % 4) as u8;
        let numcv = [2, 3, 4, 5, 9, 17][i / 4 % 6];
        let degenerate = i % 3 == 0;
        let file = built(&mut r, kind, numcv, degenerate);
        let data_addr = load_archive(ctx, &file);
        let mut archives = Archives::default();
        let id = archives.add(Archive::parse(&file).unwrap());
        let what =
            format!("built spline {i} (type {kind}, {numcv} points, degenerate {degenerate})");
        checks += check_spline(
            ctx,
            &archives,
            DataRef {
                archive: id,
                offset: 0,
            },
            data_addr,
            &mut r,
            &what,
        );
    }
    eprintln!("{checks} comparisons");
}
