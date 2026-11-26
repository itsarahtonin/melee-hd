//! melee-sim's versions of HAL's matrix helpers (`sysdolphin/baselib/mtx.c`) against
//! ssbm-rs's ports, bit for bit, in both float modes.

use leaf_check::{
    SCRATCH, arg, each_mode, handle, mtx_bits, put_mtx, put_quat, put_vec, read_f32s, vec_bits,
    write_f32s,
};
use melee_sim::mtx::Mtx34;
use ssbm_rt::{ArrV, Ctx, F32};
use ssbm_types::fns;
use ssbm_types::records::{Quaternion, Vec as V};

const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x40;
const C: u32 = SCRATCH + 0x80;
const D: u32 = SCRATCH + 0xC0;
const OUT: u32 = SCRATCH + 0x100;

/// What no port writes, so an entry left alone shows.
const UNWRITTEN: f32 = f32::from_bits(0xDEAD_BEEF);

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

fn m<'a>(ctx: &'a Ctx, addr: u32) -> ArrV<'a, F32, 4> {
    handle(ctx, addr)
}

/// A clean output matrix.
fn out(ctx: &Ctx) -> ArrV<'_, F32, 4> {
    write_f32s(ctx, OUT, &[UNWRITTEN; 12]);
    handle(ctx, OUT)
}

/// A clean output vector.
fn out_vec(ctx: &Ctx) -> V<'_> {
    write_f32s(ctx, OUT, &[UNWRITTEN; 3]);
    handle(ctx, OUT)
}

/// Matrices for the inverses: random ones, and every fourth one with a rotation part near
/// singular, so the identity and copy paths run too.
fn matrix(f: &mut leaf_check::Floats, i: &mut u32) -> Mtx34 {
    let mut a = f.mtx();
    *i += 1;
    if (*i).is_multiple_of(4) {
        a.m[2] = [a.m[0][0], a.m[0][1], a.m[0][2], a.m[2][3]];
    }
    a
}

#[test]
fn inverses_match() {
    let mut i = 0;
    each_mode(40_000, 50.0, |ctx, f, mode| {
        let (a, b) = (matrix(f, &mut i), f.mtx());
        let case = (a, b, mode);
        put_mtx(ctx, A, &a);
        put_mtx(ctx, B, &b);
        fns::HSD_MtxInverse(ctx, m(ctx, A), out(ctx));
        assert_eq!(
            mtx_bits(&a.hsd_inverse()),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxInverse {case:?}"
        );
        fns::HSD_MtxInverse(ctx, m(ctx, A), m(ctx, A));
        let mut ours = a;
        ours.hsd_invert();
        assert_eq!(
            mtx_bits(&ours),
            read_f32s(ctx, A, 12),
            "HSD_MtxInverse in place {case:?}"
        );
        put_mtx(ctx, A, &a);
        fns::HSD_MtxInverseConcat(ctx, m(ctx, A), m(ctx, B), out(ctx));
        assert_eq!(
            mtx_bits(&a.hsd_inverse_concat(&b)),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxInverseConcat {case:?}"
        );
        fns::HSD_MtxInverseTranspose(ctx, m(ctx, A), out(ctx));
        assert_eq!(
            mtx_bits(&a.hsd_inverse_transpose()),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxInverseTranspose {case:?}"
        );
    });
}

#[test]
fn decompositions_match() {
    each_mode(40_000, 20.0, |ctx, f, mode| {
        let a = f.mtx();
        put_mtx(ctx, A, &a);
        fns::HSD_MtxGetRotation(ctx, m(ctx, A), out_vec(ctx));
        assert_eq!(
            vec_bits(a.rotation_angles()),
            read_f32s(ctx, OUT, 3),
            "HSD_MtxGetRotation {a:?} {mode:?}"
        );
        fns::HSD_MtxGetTranslate(ctx, m(ctx, A), out_vec(ctx));
        assert_eq!(
            vec_bits(a.translation_part()),
            read_f32s(ctx, OUT, 3),
            "HSD_MtxGetTranslate {a:?} {mode:?}"
        );
        fns::HSD_MtxGetScale(ctx, m(ctx, A), out_vec(ctx));
        assert_eq!(
            vec_bits(a.scale_part()),
            read_f32s(ctx, OUT, 3),
            "HSD_MtxGetScale {a:?} {mode:?}"
        );
    });
}

/// Matrices built from Euler angles go through `HSD_MtxGetRotation`'s inner branches, which
/// random matrices rarely reach.
#[test]
fn rotations_of_rotation_matrices_match() {
    each_mode(40_000, 4.0, |ctx, f, mode| {
        let angles = f.vec();
        let mut a = Mtx34::from_euler(angles);
        a.m[0][0] = if f.next().to_bits() % 3 == 0 {
            f.next()
        } else {
            a.m[0][0]
        };
        put_mtx(ctx, A, &a);
        fns::HSD_MtxGetRotation(ctx, m(ctx, A), out_vec(ctx));
        assert_eq!(
            vec_bits(a.rotation_angles()),
            read_f32s(ctx, OUT, 3),
            "HSD_MtxGetRotation {a:?} {mode:?}"
        );
    });
}

/// `HSD_MtxGetRotation`'s boundaries: a sine of exactly ±1, and entries of exactly `FLT_MIN`,
/// which take the ±π/2 shortcut instead of `atan2f`.
#[test]
fn rotation_boundaries_match() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let ctx = &ctx;
    let min = f32::MIN_POSITIVE;
    let below = f32::from_bits(min.to_bits() - 1);
    let mut cases = Vec::new();
    for (tiny, sin) in [(min, 0.5), (min, -0.5), (below, 0.5), (-min, -0.5)] {
        // Column 0 of length 0.5 with m[2][0] = ∓0.5 gives a sine of ±1.
        cases.push(Mtx34 {
            m: [
                [tiny, tiny, 0.0, 0.0],
                [tiny, tiny, 1.0, 0.0],
                [sin, 1.0, 0.0, 0.0],
            ],
        });
        // A cosine well above zero, with the z angle's x exactly at the threshold.
        cases.push(Mtx34 {
            m: [
                [tiny, 0.0, 1.0, 0.0],
                [tiny, 1.0, 0.0, 0.0],
                [sin, 0.0, tiny, 0.0],
            ],
        });
    }
    for mode in leaf_check::MODES {
        leaf_check::fp::set_fp_mode(mode);
        for a in &cases {
            put_mtx(ctx, A, a);
            fns::HSD_MtxGetRotation(ctx, m(ctx, A), out_vec(ctx));
            assert_eq!(
                vec_bits(a.rotation_angles()),
                read_f32s(ctx, OUT, 3),
                "HSD_MtxGetRotation {a:?} {mode:?}"
            );
        }
    }
    leaf_check::fp::set_fp_mode(leaf_check::fp::FpMode::Hardware);
}

#[test]
fn builders_match() {
    each_mode(40_000, 10.0, |ctx, f, mode| {
        let (scale, rotate, translate, parent) = (f.vec(), f.vec(), f.vec(), f.vec());
        let quat = f.quat();
        let case = (scale, rotate, translate, parent, quat, mode);
        put_vec(ctx, A, scale);
        put_vec(ctx, B, rotate);
        put_vec(ctx, C, translate);
        put_vec(ctx, D, parent);
        fns::HSD_MkRotationMtx(ctx, out(ctx), v(ctx, B));
        assert_eq!(
            mtx_bits(&Mtx34::from_euler(rotate)),
            read_f32s(ctx, OUT, 12),
            "HSD_MkRotationMtx {case:?}"
        );
        let null = v(ctx, 0);
        fns::HSD_MtxSRT(ctx, out(ctx), v(ctx, A), v(ctx, B), v(ctx, C), null);
        assert_eq!(
            mtx_bits(&Mtx34::srt(scale, rotate, translate, None)),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxSRT {case:?}"
        );
        fns::HSD_MtxSRT(ctx, out(ctx), v(ctx, A), v(ctx, B), v(ctx, C), v(ctx, D));
        assert_eq!(
            mtx_bits(&Mtx34::srt(scale, rotate, translate, Some(parent))),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxSRT with parent scale {case:?}"
        );
        put_quat(ctx, B, quat);
        let q: Quaternion = handle(ctx, B);
        fns::HSD_MtxSRTQuat(ctx, out(ctx), v(ctx, A), q, v(ctx, C), null);
        assert_eq!(
            mtx_bits(&Mtx34::srt_quat(scale, quat, translate, None)),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxSRTQuat {case:?}"
        );
        fns::HSD_MtxSRTQuat(ctx, out(ctx), v(ctx, A), q, v(ctx, C), v(ctx, D));
        assert_eq!(
            mtx_bits(&Mtx34::srt_quat(scale, quat, translate, Some(parent))),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxSRTQuat with parent scale {case:?}"
        );
    });
}

#[test]
fn scaled_adds_match() {
    each_mode(40_000, 50.0, |ctx, f, mode| {
        let (a, b, s) = (f.mtx(), f.mtx(), f.next());
        put_mtx(ctx, A, &a);
        put_mtx(ctx, B, &b);
        fns::HSD_MtxScaledAdd(ctx, m(ctx, A), m(ctx, B), out(ctx), arg(s));
        assert_eq!(
            mtx_bits(&a.scaled_add(&b, s)),
            read_f32s(ctx, OUT, 12),
            "HSD_MtxScaledAdd {a:?} {b:?} {s} {mode:?}"
        );
    });
}
