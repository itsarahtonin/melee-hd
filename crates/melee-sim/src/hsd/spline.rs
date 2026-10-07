//! HAL's splines (`sysdolphin/baselib/spline.c`).

use gekko_fp as fp;

/// A Hermite curve's value at `time` between `p0` and `p1` with slopes `d0` and `d1`, where
/// `inv_fterm` is 1 over the segment's length, `splGetHelmite`. All register values.
#[doc(alias = "splGetHelmite")]
pub fn hermite(inv_fterm: f64, time: f64, p0: f64, p1: f64, d0: f64, d1: f64) -> f64 {
    let t2 = fp::fmuls(time, time);
    let inv2 = fp::fmuls(inv_fterm, inv_fterm);
    let t2_t = fp::fmuls(t2, inv_fterm);
    let t3_t2 = fp::fmuls(inv2, fp::fmuls(t2, time));
    let two_t3_t3 = fp::fmuls(fp::fmuls(2.0, t3_t2), inv_fterm);
    let three_t2_t2 = fp::fmuls(fp::fmuls(3.0, t2), inv2);
    let p1_part = fp::fmuls(p1, fp::fadds(fp::fneg(two_t3_t3), three_t2_t2));
    let p0_part = fp::fmadds(
        p0,
        fp::fadds(1.0, fp::fsubs(two_t3_t3, three_t2_t2)),
        p1_part,
    );
    let d0_factor = fp::fadds(time, fp::fsubs(fp::fsubs(t3_t2, t2_t), t2_t));
    let d0_part = fp::fmadds(d0, d0_factor, p0_part);
    fp::fmadds(d1, fp::fsubs(t3_t2, t2_t), d0_part)
}
