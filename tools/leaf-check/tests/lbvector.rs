//! melee-sim's versions of the game's own vector helpers (`melee/lb/lbvector.c`) against
//! ssbm-rs's ports, bit for bit, in both float modes.

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{SCRATCH, arg, bits, handle, inputs, read_f32s, reference, write_f32s};
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
