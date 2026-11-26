//! melee-sim's versions of the game's own vector helpers (`melee/lb/lbvector.c`) against
//! ssbm-rs's ports, bit for bit, in both float modes.

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{
    SCRATCH, arg, bits, each_mode, handle, inputs, mtx_bits, read_f32s, reference, write_f32s,
};
use melee_sim::mtx::{Axis, Mtx34};
use melee_sim::{math, vec::Vec3};
use ssbm_rt::Ctx;
use ssbm_types::fns;
use ssbm_types::records::Vec as V;

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];
const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x20;
const C: u32 = SCRATCH + 0x40;
const OUT: u32 = SCRATCH + 0x60;

fn put(ctx: &Ctx, addr: u32, v: Vec3) {
    write_f32s(ctx, addr, &[v.x, v.y, v.z]);
}

fn got(ctx: &Ctx, addr: u32) -> std::vec::Vec<u32> {
    read_f32s(ctx, addr, 3)
}

fn b3(v: Vec3) -> std::vec::Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

#[test]
fn lbvector_helpers_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let floats = inputs(100.0, 200_000);
    let mut at = 0;
    let mut next = || {
        at = (at + 5) % floats.len();
        floats[at]
    };
    for mode in MODES {
        set_fp_mode(mode);
        for _ in 0..40_000 {
            let a = Vec3::new(next(), next(), next());
            let b = Vec3::new(next(), next(), next());
            let c = Vec3::new(next(), next(), next());
            let t = next();
            let ctx_ = (mode, a, b, c, t);

            put(ctx, A, a);
            let len = bits(fns::lbVector_Normalize(ctx, v(ctx, A)));
            let mut ours = a;
            assert_eq!(ours.normalize().to_bits(), len, "Normalize len {ctx_:?}");
            assert_eq!(b3(ours), got(ctx, A), "Normalize {ctx_:?}");

            put(ctx, A, a);
            let len = bits(fns::lbVector_NormalizeXY(ctx, v(ctx, A)));
            let mut ours = a;
            assert_eq!(
                ours.normalize_xy().to_bits(),
                len,
                "NormalizeXY len {ctx_:?}"
            );
            assert_eq!(b3(ours), got(ctx, A), "NormalizeXY {ctx_:?}");

            put(ctx, A, a);
            put(ctx, B, b);
            fns::lbVector_Add(ctx, v(ctx, A), v(ctx, B));
            assert_eq!(b3(a + b), got(ctx, A), "Add {ctx_:?}");
            put(ctx, A, a);
            fns::lbVector_Add_xy(ctx, v(ctx, A), v(ctx, B));
            assert_eq!(b3(a.add_xy(b)), got(ctx, A), "Add_xy {ctx_:?}");
            put(ctx, A, a);
            fns::lbVector_Sub(ctx, v(ctx, A), v(ctx, B));
            assert_eq!(b3(a - b), got(ctx, A), "Sub {ctx_:?}");
            put(ctx, A, a);
            fns::lbVector_Diff(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT));
            assert_eq!(b3(a - b), got(ctx, OUT), "Diff {ctx_:?}");

            fns::lbVector_CrossprodNormalized(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT));
            assert_eq!(
                b3(a.cross_normalized(b)),
                got(ctx, OUT),
                "CrossprodNormalized {ctx_:?}"
            );

            let angle = bits(fns::lbVector_Angle(ctx, v(ctx, A), v(ctx, B)));
            assert_eq!(a.angle(b).to_bits(), angle, "Angle {ctx_:?}");
            let angle = bits(fns::lbVector_AngleXY(ctx, v(ctx, A), v(ctx, B)));
            assert_eq!(a.angle_xy(b).to_bits(), angle, "AngleXY {ctx_:?}");
            let cos = bits(fns::lbVector_CosAngle(ctx, v(ctx, A), v(ctx, B)));
            assert_eq!(a.cos_angle_xy(b).to_bits(), cos, "CosAngle {ctx_:?}");

            let axis = Vec3::new(c.x, c.y, 0.0);
            put(ctx, C, axis);
            fns::lbVector_Mirror(ctx, v(ctx, A), v(ctx, C));
            let mut ours = a;
            ours.mirror_xy(axis);
            assert_eq!(b3(ours), got(ctx, A), "Mirror {ctx_:?}");

            put(ctx, A, a);
            fns::lbVector_Lerp(ctx, v(ctx, A), v(ctx, B), v(ctx, OUT), arg(t));
            assert_eq!(b3(a.lerp(b, t)), got(ctx, OUT), "Lerp {ctx_:?}");

            put(ctx, C, c);
            let dist = bits(fns::lbVector_8000E838(
                ctx,
                v(ctx, A),
                v(ctx, B),
                v(ctx, C),
                v(ctx, OUT),
            ));
            let (point, d) = Vec3::closest_on_line(a, b, c);
            assert_eq!(d.to_bits(), dist, "8000E838 distance {ctx_:?}");
            assert_eq!(b3(point), got(ctx, OUT), "8000E838 point {ctx_:?}");

            let s = bits(fns::lbVector_sqrtf_accurate(ctx, arg(t)));
            assert_eq!(
                math::sqrt_accurate(t).to_bits(),
                s,
                "sqrtf_accurate {ctx_:?}"
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn rotations_match() {
    // lbVector_Rotate's axis numbers; every caller passes one of these.
    let axes = [(1, Axis::X), (2, Axis::Y), (4, Axis::Z)];
    each_mode(40_000, 20.0, |ctx, f, mode| {
        let (a, axis, angles) = (f.vec(), f.vec(), f.vec());
        let angle = f.next();
        let case = (a, axis, angles, angle, mode);
        for (number, ax) in axes {
            put(ctx, A, a);
            fns::lbVector_Rotate(ctx, v(ctx, A), number, arg(angle));
            let mut ours = a;
            ours.rotate(ax, angle);
            assert_eq!(b3(ours), got(ctx, A), "Rotate {number} {case:?}");
        }
        put(ctx, A, a);
        put(ctx, B, axis);
        fns::lbVector_RotateAboutUnitAxis(ctx, v(ctx, A), v(ctx, B), arg(angle));
        let mut ours = a;
        ours.rotate_about(axis, angle);
        assert_eq!(b3(ours), got(ctx, A), "RotateAboutUnitAxis {case:?}");
        // The same about a unit axis, as callers pass.
        let unit = axis.sdk_normalize();
        put(ctx, A, a);
        put(ctx, B, unit);
        fns::lbVector_RotateAboutUnitAxis(ctx, v(ctx, A), v(ctx, B), arg(angle));
        let mut ours = a;
        ours.rotate_about(unit, angle);
        assert_eq!(b3(ours), got(ctx, A), "RotateAboutUnitAxis unit {case:?}");
        put(ctx, A, a);
        put(ctx, C, angles);
        fns::lbVector_ApplyEulerRotation(ctx, v(ctx, A), v(ctx, C));
        let mut ours = a;
        ours.rotate_euler(angles);
        assert_eq!(b3(ours), got(ctx, A), "ApplyEulerRotation {case:?}");
        write_f32s(ctx, C, &[angles.x, angles.y, angles.z, f.next()]);
        write_f32s(ctx, OUT, &[f32::from_bits(0xDEAD_BEEF); 12]);
        fns::lbVector_CreateEulerMatrix(ctx, handle(ctx, OUT), handle(ctx, C));
        assert_eq!(
            mtx_bits(&Mtx34::from_euler_approx(angles)),
            read_f32s(ctx, OUT, 12),
            "CreateEulerMatrix {case:?}"
        );
    });
}

#[test]
fn curves_and_bases_match() {
    each_mode(40_000, 2.0, |ctx, f, mode| {
        let p = [f.vec(), f.vec(), f.vec()];
        let t = f.next();
        let case = (p, t, mode);
        for (i, point) in p.iter().enumerate() {
            put(ctx, A + 12 * i as u32, *point);
        }
        fns::lbVector_8000DE38(ctx, v(ctx, A), v(ctx, OUT), arg(t));
        assert_eq!(
            b3(Vec3::quadratic(p, t)),
            got(ctx, OUT),
            "8000DE38 {case:?}"
        );
        let [a, b, c] = p;
        put(ctx, A, a);
        put(ctx, B, b);
        put(ctx, C, c);
        fns::lbVector_EulerAnglesFromONB(ctx, v(ctx, OUT), v(ctx, A), v(ctx, B), v(ctx, C));
        assert_eq!(
            b3(Vec3::euler_angles_from_basis(a, b, c)),
            got(ctx, OUT),
            "EulerAnglesFromONB {case:?}"
        );
        fns::lbVector_EulerAnglesFromPartialONB(ctx, v(ctx, OUT), v(ctx, A), v(ctx, C));
        assert_eq!(
            b3(Vec3::euler_angles_from_partial_basis(a, c)),
            got(ctx, OUT),
            "EulerAnglesFromPartialONB {case:?}"
        );
    });
}
