//! melee-sim's rotation, quaternion, camera and projection matrices against ssbm-rs's ports of
//! the SDK's `mtx.c` and `mtx44.c`, bit for bit, in both float modes.

use leaf_check::{
    SCRATCH, arg, each_mode, handle, mtx_bits, put_quat, put_vec, read_f32s, write_f32s,
};
use melee_sim::mtx::{Axis, Mtx34, Mtx44};
use ssbm_rt::{ArrV, Ctx, F32};
use ssbm_types::fns;
use ssbm_types::records::{Quaternion, Vec as V};

const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x40;
const C: u32 = SCRATCH + 0x80;
const OUT: u32 = SCRATCH + 0xC0;

/// What no port writes, so an entry left alone shows.
const UNWRITTEN: f32 = f32::from_bits(0xDEAD_BEEF);

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

fn q<'a>(ctx: &'a Ctx, addr: u32) -> Quaternion<'a> {
    handle(ctx, addr)
}

/// A clean output matrix.
fn out(ctx: &Ctx) -> ArrV<'_, F32, 4> {
    write_f32s(ctx, OUT, &[UNWRITTEN; 16]);
    handle(ctx, OUT)
}

fn mtx44_bits(m: &Mtx44) -> Vec<u32> {
    m.m.as_flattened().iter().map(|v| v.to_bits()).collect()
}

#[test]
fn rotations_match() {
    let axes = [
        (b'x', Axis::X),
        (b'y', Axis::Y),
        (b'z', Axis::Z),
        (b'X', Axis::X),
        (b'Y', Axis::Y),
        (b'Z', Axis::Z),
    ];
    each_mode(20_000, 10.0, |ctx, f, mode| {
        for (name, axis) in axes {
            let (s, c, rad) = (f.next(), f.next(), f.next());
            let case = (name as char, s, c, rad, mode);
            fns::PSMTXRotTrig(ctx, out(ctx), name as i8, arg(s), arg(c));
            assert_eq!(
                mtx_bits(&Mtx34::rotation_trig(axis, s, c)),
                read_f32s(ctx, OUT, 12),
                "PSMTXRotTrig {case:?}"
            );
            fns::MTXRotRad(ctx, out(ctx), name as i8, arg(rad));
            assert_eq!(
                mtx_bits(&Mtx34::rotation(axis, rad)),
                read_f32s(ctx, OUT, 12),
                "MTXRotRad {case:?}"
            );
        }
        let (axis, rad) = (f.vec(), f.next());
        put_vec(ctx, A, axis);
        fns::PSMTXRotAxisRad(ctx, out(ctx), v(ctx, A), arg(rad));
        assert_eq!(
            mtx_bits(&Mtx34::rotation_about(axis, rad)),
            read_f32s(ctx, OUT, 12),
            "PSMTXRotAxisRad {axis:?} {rad} {mode:?}"
        );
        let quat = f.quat();
        put_quat(ctx, A, quat);
        let want = mtx_bits(&Mtx34::from_quat(quat));
        fns::PSMTXQuat(ctx, out(ctx), q(ctx, A));
        assert_eq!(want, read_f32s(ctx, OUT, 12), "PSMTXQuat {quat:?} {mode:?}");
        fns::HSD_MtxQuat(ctx, out(ctx), q(ctx, A));
        assert_eq!(
            want,
            read_f32s(ctx, OUT, 12),
            "HSD_MtxQuat {quat:?} {mode:?}"
        );
    });
}

#[test]
fn look_at_matches() {
    each_mode(40_000, 100.0, |ctx, f, mode| {
        let (eye, up, target) = (f.vec(), f.vec(), f.vec());
        put_vec(ctx, A, eye);
        put_vec(ctx, B, up);
        put_vec(ctx, C, target);
        fns::C_MTXLookAt(ctx, out(ctx), v(ctx, A), v(ctx, B), v(ctx, C));
        assert_eq!(
            mtx_bits(&Mtx34::look_at(eye, up, target)),
            read_f32s(ctx, OUT, 12),
            "C_MTXLookAt {eye:?} {up:?} {target:?} {mode:?}"
        );
    });
}

#[test]
fn projections_match() {
    each_mode(40_000, 100.0, |ctx, f, mode| {
        let p: [f32; 9] = std::array::from_fn(|_| f.next());
        let a = p.map(arg);
        let case = (p, mode);
        fns::MTXLightFrustum(
            ctx,
            out(ctx),
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[5],
            a[6],
            a[7],
            a[8],
        );
        let ours = Mtx34::light_frustum(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]);
        assert_eq!(
            mtx_bits(&ours),
            read_f32s(ctx, OUT, 12),
            "MTXLightFrustum {case:?}"
        );
        fns::MTXLightPerspective(ctx, out(ctx), a[0], a[1], a[2], a[3], a[4], a[5]);
        let ours = Mtx34::light_perspective(p[0], p[1], p[2], p[3], p[4], p[5]);
        assert_eq!(
            mtx_bits(&ours),
            read_f32s(ctx, OUT, 12),
            "MTXLightPerspective {case:?}"
        );
        fns::MTXLightOrtho(
            ctx,
            out(ctx),
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[5],
            a[6],
            a[7],
        );
        let ours = Mtx34::light_ortho(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
        assert_eq!(
            mtx_bits(&ours),
            read_f32s(ctx, OUT, 12),
            "MTXLightOrtho {case:?}"
        );
        fns::MTXFrustum(ctx, out(ctx), a[0], a[1], a[2], a[3], a[4], a[5]);
        let ours = Mtx44::frustum(p[0], p[1], p[2], p[3], p[4], p[5]);
        assert_eq!(
            mtx44_bits(&ours),
            read_f32s(ctx, OUT, 16),
            "MTXFrustum {case:?}"
        );
        fns::MTXPerspective(ctx, out(ctx), a[0], a[1], a[2], a[3]);
        let ours = Mtx44::perspective(p[0], p[1], p[2], p[3]);
        assert_eq!(
            mtx44_bits(&ours),
            read_f32s(ctx, OUT, 16),
            "MTXPerspective {case:?}"
        );
        fns::MTXOrtho(ctx, out(ctx), a[0], a[1], a[2], a[3], a[4], a[5]);
        let ours = Mtx44::ortho(p[0], p[1], p[2], p[3], p[4], p[5]);
        assert_eq!(
            mtx44_bits(&ours),
            read_f32s(ctx, OUT, 16),
            "MTXOrtho {case:?}"
        );
    });
}

/// Fields of view in the range cameras use, where the tangent is well away from its poles.
#[test]
fn perspectives_match_for_camera_angles() {
    each_mode(40_000, 1.0, |ctx, f, mode| {
        let fov = 1.0 + (f.next().to_bits() % 17_800) as f32 / 100.0;
        let (aspect, n, far) = (f.next(), f.next(), f.next());
        fns::MTXPerspective(ctx, out(ctx), arg(fov), arg(aspect), arg(n), arg(far));
        assert_eq!(
            mtx44_bits(&Mtx44::perspective(fov, aspect, n, far)),
            read_f32s(ctx, OUT, 16),
            "MTXPerspective {fov} {aspect} {n} {far} {mode:?}"
        );
    });
}
