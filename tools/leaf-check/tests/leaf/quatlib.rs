//! melee-sim's quaternions against ssbm-rs's ports of HAL's quaternion library
//! (`sysdolphin/baselib/quatlib.c`), bit for bit, in both float modes.

use leaf_check::{
    SCRATCH, arg, each_mode, handle, put_mtx, put_quat, put_vec, quat_bits, read_f32s, vec_bits,
    write_f32s,
};
use melee_sim::mtx::Mtx34;
use melee_sim::quat::Quat;
use ssbm_rt::{ArrV, Ctx, F32};
use ssbm_types::fns;
use ssbm_types::records::{Quaternion, Vec as V};

const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x40;
const OUT: u32 = SCRATCH + 0x80;

/// What no port writes, so an entry left alone shows.
const UNWRITTEN: f32 = f32::from_bits(0xDEAD_BEEF);

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

fn q<'a>(ctx: &'a Ctx, addr: u32) -> Quaternion<'a> {
    handle(ctx, addr)
}

fn m<'a>(ctx: &'a Ctx, addr: u32) -> ArrV<'a, F32, 4> {
    handle(ctx, addr)
}

/// A clean output quaternion.
fn out(ctx: &Ctx) -> Quaternion<'_> {
    write_f32s(ctx, OUT, &[UNWRITTEN; 4]);
    handle(ctx, OUT)
}

#[test]
fn matrix_conversions_match() {
    each_mode(40_000, 20.0, |ctx, f, mode| {
        // Random matrices, and rotations (scaled per column) so each of MatToQuat's branches
        // runs.
        let a = if f.next().to_bits() % 2 == 0 {
            f.mtx()
        } else {
            let mut r = Mtx34::from_euler(f.vec());
            for c in 0..3 {
                let s = f.next();
                for row in &mut r.m {
                    row[c] *= s;
                }
            }
            r
        };
        put_mtx(ctx, A, &a);
        assert_eq!(fns::MatToQuat(ctx, m(ctx, A), out(ctx)), 0);
        assert_eq!(
            quat_bits(Quat::from_matrix(&a)),
            read_f32s(ctx, OUT, 4),
            "MatToQuat {a:?} {mode:?}"
        );
        write_f32s(ctx, OUT, &[UNWRITTEN; 3]);
        assert_eq!(fns::HSD_QuatLib_8037EB28(ctx, m(ctx, A), v(ctx, OUT)), 0);
        assert_eq!(
            vec_bits(a.euler_angles()),
            read_f32s(ctx, OUT, 3),
            "HSD_QuatLib_8037EB28 {a:?} {mode:?}"
        );
    });
}

#[test]
fn constructions_match() {
    each_mode(40_000, 10.0, |ctx, f, mode| {
        let (axis, angle, euler) = (f.vec(), f.next(), f.vec());
        let case = (axis, angle, euler, mode);
        put_vec(ctx, A, axis);
        let ret = fns::HSD_QuatLib_8037ECE0(ctx, v(ctx, A), out(ctx), arg(angle));
        match Quat::from_axis_angle(axis, angle) {
            Some(ours) => {
                assert_eq!(ret, 0, "HSD_QuatLib_8037ECE0 result {case:?}");
                assert_eq!(
                    quat_bits(ours),
                    read_f32s(ctx, OUT, 4),
                    "HSD_QuatLib_8037ECE0 {case:?}"
                );
            }
            None => {
                assert_eq!(ret, -1, "HSD_QuatLib_8037ECE0 result {case:?}");
                assert_eq!(
                    vec![UNWRITTEN.to_bits(); 4],
                    read_f32s(ctx, OUT, 4),
                    "HSD_QuatLib_8037ECE0 wrote {case:?}"
                );
            }
        }
        put_vec(ctx, A, euler);
        assert_eq!(fns::EulerToQuat(ctx, v(ctx, A), out(ctx)), 0);
        assert_eq!(
            quat_bits(Quat::from_euler(euler)),
            read_f32s(ctx, OUT, 4),
            "EulerToQuat {case:?}"
        );
    });
}

/// Axes with no length, which random streams almost never give.
#[test]
fn zero_axes_have_no_rotation() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let ctx = &ctx;
    let tiny = f32::from_bits(1);
    for axis in [
        melee_sim::vec::Vec3::ZERO,
        melee_sim::vec::Vec3::new(-0.0, tiny, 0.0),
        melee_sim::vec::Vec3::new(1.0e-30, 0.0, -1.0e-30),
    ] {
        put_vec(ctx, A, axis);
        let ret = fns::HSD_QuatLib_8037ECE0(ctx, v(ctx, A), out(ctx), arg(1.0));
        assert_eq!(ret, -1, "{axis:?}");
        assert_eq!(read_f32s(ctx, OUT, 4), vec![UNWRITTEN.to_bits(); 4]);
        assert_eq!(Quat::from_axis_angle(axis, 1.0), None, "{axis:?}");
    }
}

#[test]
fn products_and_interpolations_match() {
    each_mode(40_000, 2.0, |ctx, f, mode| {
        let (p, r, t) = (f.quat(), f.quat(), f.next());
        // Every eighth pair opposite or equal, so the two degenerate paths of the slerp run.
        let r = match f.next().to_bits() % 8 {
            0 => Quat::new(-p.x, -p.y, -p.z, -p.w),
            1 => p,
            _ => r,
        };
        let case = (p, r, t, mode);
        put_quat(ctx, A, p);
        put_quat(ctx, B, r);
        assert_eq!(
            fns::HSD_QuatLib_8037EC4C(ctx, q(ctx, A), q(ctx, B), out(ctx)),
            0
        );
        assert_eq!(
            quat_bits(p * r),
            read_f32s(ctx, OUT, 4),
            "HSD_QuatLib_8037EC4C {case:?}"
        );
        assert_eq!(
            fns::HSD_QuatLib_8037EF28(ctx, q(ctx, A), q(ctx, B), out(ctx), arg(t)),
            0
        );
        assert_eq!(
            quat_bits(p.slerp(r, t)),
            read_f32s(ctx, OUT, 4),
            "HSD_QuatLib_8037EF28 {case:?}"
        );
    });
}

/// Interpolations between unit quaternions, as the game makes them with `EulerToQuat`: equal,
/// opposite and unrelated pairs, at the ends, the middle and in between.
#[test]
fn unit_interpolations_match() {
    each_mode(20_000, 4.0, |ctx, f, mode| {
        let p = Quat::from_euler(f.vec());
        let other = Quat::from_euler(f.vec());
        let opposite = Quat::new(-p.x, -p.y, -p.z, -p.w);
        for r in [p, opposite, other] {
            for t in [0.0, 0.25, 0.5, 0.75, 1.0, f.next()] {
                put_quat(ctx, A, p);
                put_quat(ctx, B, r);
                fns::HSD_QuatLib_8037EF28(ctx, q(ctx, A), q(ctx, B), out(ctx), arg(t));
                assert_eq!(
                    quat_bits(p.slerp(r, t)),
                    read_f32s(ctx, OUT, 4),
                    "HSD_QuatLib_8037EF28 {p:?} {r:?} {t} {mode:?}"
                );
            }
        }
    });
}

/// `MatToQuat` on half turns, whose traces are not positive, including ties between diagonal
/// entries, where the first largest one is picked.
#[test]
fn half_turns_match() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let ctx = &ctx;
    let row = |a: f32, b: f32, c: f32| [a, b, c, 0.0];
    let cases = [
        [row(0.0, 1.0, 0.0), row(1.0, 0.0, 0.0), row(0.0, 0.0, -1.0)],
        [row(0.0, 0.0, 1.0), row(0.0, -1.0, 0.0), row(1.0, 0.0, 0.0)],
        [row(-1.0, 0.0, 0.0), row(0.0, 0.0, 1.0), row(0.0, 1.0, 0.0)],
        [
            row(-1.0, 0.0, 0.0),
            row(0.0, -1.0, 0.0),
            row(0.0, 0.0, -1.0),
        ],
        [row(1.0, 0.0, 0.0), row(0.0, -1.0, 0.0), row(0.0, 0.0, -1.0)],
        [row(-1.0, 0.0, 0.0), row(0.0, 1.0, 0.0), row(0.0, 0.0, -1.0)],
        [row(-1.0, 0.0, 0.0), row(0.0, -1.0, 0.0), row(0.0, 0.0, 1.0)],
    ];
    for mode in leaf_check::MODES {
        leaf_check::fp::set_fp_mode(mode);
        for m3 in cases {
            let a = Mtx34 { m: m3 };
            put_mtx(ctx, A, &a);
            fns::MatToQuat(ctx, m(ctx, A), out(ctx));
            assert_eq!(
                quat_bits(Quat::from_matrix(&a)),
                read_f32s(ctx, OUT, 4),
                "MatToQuat {a:?} {mode:?}"
            );
        }
    }
    leaf_check::fp::set_fp_mode(leaf_check::fp::FpMode::Hardware);
}
