//! melee-sim's vectors and matrices against ssbm-rs's ports of the SDK's paired-single
//! routines, bit for bit, in both float modes.

use leaf_check::{
    SCRATCH, arg, bits, each_mode, handle, mtx_bits, put_mtx, put_vec, read_f32s, reference,
    vec_bits, write_f32s,
};
use melee_sim::mtx::Mtx34;
use ssbm_rt::{ArrV, Ctx, F32};
use ssbm_types::fns;
use ssbm_types::records::Vec as V;

const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x40;
const OUT: u32 = SCRATCH + 0x80;

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

fn m<'a>(ctx: &'a Ctx, addr: u32) -> ArrV<'a, F32, 4> {
    handle(ctx, addr)
}

#[test]
fn vector_routines_match() {
    each_mode(40_000, 200.0, |ctx, f, mode| {
        let (a, b, s) = (f.vec(), f.vec(), f.next());
        put_vec(ctx, A, a);
        put_vec(ctx, B, b);
        fns::PSVECAdd(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.sdk_add(b)),
            read_f32s(ctx, OUT, 3),
            "add {a:?} {b:?} {mode:?}"
        );
        fns::PSVECSubtract(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.sdk_sub(b)),
            read_f32s(ctx, OUT, 3),
            "sub {a:?} {b:?} {mode:?}"
        );
        fns::PSVECScale(ctx, v(ctx, A), v(ctx, OUT), arg(s));
        assert_eq!(
            vec_bits(a.sdk_scale(s)),
            read_f32s(ctx, OUT, 3),
            "scale {a:?} {s} {mode:?}"
        );
        fns::PSVECNormalize(ctx, v(ctx, A), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.sdk_normalize()),
            read_f32s(ctx, OUT, 3),
            "normalize {a:?} {mode:?}"
        );
        let mag = bits(fns::PSVECMag(ctx, v(ctx, A)));
        assert_eq!(a.sdk_length().to_bits(), mag, "mag {a:?} {mode:?}");
        let dot = bits(fns::PSVECDotProduct(ctx, v(ctx, A), v(ctx, B)));
        assert_eq!(a.sdk_dot(b).to_bits(), dot, "dot {a:?} {b:?} {mode:?}");
        fns::PSVECCrossProduct(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.sdk_cross(b)),
            read_f32s(ctx, OUT, 3),
            "cross {a:?} {b:?} {mode:?}"
        );
    });
}

#[test]
fn matrix_routines_match() {
    each_mode(20_000, 50.0, |ctx, f, mode| {
        let (a, b, p) = (f.mtx(), f.mtx(), f.vec());
        put_mtx(ctx, A, &a);
        put_mtx(ctx, B, &b);
        fns::PSMTXConcat(ctx, m(ctx, A), m(ctx, B), m(ctx, OUT));
        assert_eq!(
            mtx_bits(&a.concat(&b)),
            read_f32s(ctx, OUT, 12),
            "concat {a:?} {b:?} {mode:?}"
        );
        fns::PSMTXTranspose(ctx, m(ctx, A), m(ctx, OUT));
        assert_eq!(
            mtx_bits(&a.transpose()),
            read_f32s(ctx, OUT, 12),
            "transpose {a:?} {mode:?}"
        );
        fns::PSMTXCopy(ctx, m(ctx, A), m(ctx, OUT));
        assert_eq!(
            mtx_bits(&a.copied()),
            read_f32s(ctx, OUT, 12),
            "copy {a:?} {mode:?}"
        );
        write_f32s(ctx, OUT, &[f32::from_bits(0xDEAD_BEEF); 12]);
        let ok = fns::PSMTXInverse(ctx, m(ctx, A), m(ctx, OUT)) != 0;
        let ours = a.inverse();
        assert_eq!(ours.is_some(), ok, "inverse exists {a:?} {mode:?}");
        if let Some(inv) = ours {
            assert_eq!(
                mtx_bits(&inv),
                read_f32s(ctx, OUT, 12),
                "inverse {a:?} {mode:?}"
            );
        }
        put_vec(ctx, B, p);
        fns::PSMTXMultVec(ctx, m(ctx, A), v(ctx, B), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.transform(p)),
            read_f32s(ctx, OUT, 3),
            "multvec {a:?} {p:?} {mode:?}"
        );
        fns::PSMTXMultVecSR(ctx, m(ctx, A), v(ctx, B), v(ctx, OUT));
        assert_eq!(
            vec_bits(a.rotate(p)),
            read_f32s(ctx, OUT, 3),
            "multvecsr {a:?} {p:?} {mode:?}"
        );
        let (x, y, z) = (p.x, p.y, p.z);
        fns::PSMTXTrans(ctx, m(ctx, OUT), arg(x), arg(y), arg(z));
        assert_eq!(
            mtx_bits(&Mtx34::translation(x, y, z)),
            read_f32s(ctx, OUT, 12),
            "trans {p:?}"
        );
        fns::PSMTXScale(ctx, m(ctx, OUT), arg(x), arg(y), arg(z));
        assert_eq!(
            mtx_bits(&Mtx34::scale(x, y, z)),
            read_f32s(ctx, OUT, 12),
            "scale {p:?}"
        );
        fns::PSMTXIdentity(ctx, m(ctx, OUT));
        assert_eq!(
            mtx_bits(&Mtx34::IDENTITY),
            read_f32s(ctx, OUT, 12),
            "identity"
        );
    });
}

#[test]
fn singular_matrices_have_no_inverse() {
    let Some(ctx) = reference() else { return };
    let mut a = Mtx34::IDENTITY;
    a.m[2] = [0.0, 0.0, 0.0, 5.0];
    put_mtx(&ctx, A, &a);
    assert_eq!(fns::PSMTXInverse(&ctx, m(&ctx, A), m(&ctx, OUT)), 0);
    assert!(a.inverse().is_none());
}
