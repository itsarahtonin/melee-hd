//! HAL's splines (`sysdolphin/baselib/spline.c`): the Hermite curve of animation keyframes, and
//! `HSD_Spline` curves in game data (stage paths such as Mute City's and Big Blue's tracks)
//! with the arc-length tables that move objects along them at an even speed.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::{mem, reg};
use crate::math;
use crate::vec::Vec3;

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

/// `1.0f / 6.0f`.
const SIXTH: u32 = 0x3E2A_AAAB;

/// `HSD_Spline`'s fields, read from game data (the game never writes them).
struct Spline {
    /// 0: linear, 1: Bézier, 2: uniform B-spline, 3: cardinal.
    kind: u8,
    numcv: i16,
    tension: f32,
    cv: Option<DataRef>,
    total_length: f32,
    /// The arc length at the start of each segment, as a fraction of the total.
    seg_length: Option<DataRef>,
    /// Each segment's speed squared as a quartic in its parameter: five coefficients.
    seg_poly: Option<DataRef>,
}

impl Spline {
    fn read(data: &Archives, at: DataRef) -> Spline {
        Spline {
            kind: data.u8(at),
            numcv: data.u16(at.at(2)) as i16,
            tension: data.f32(at.at(4)),
            cv: data.ptr(at.at(8)),
            total_length: data.f32(at.at(0xC)),
            seg_length: data.ptr(at.at(0x10)),
            seg_poly: data.ptr(at.at(0x14)),
        }
    }
}

/// `base` moved by a signed byte offset, as the original's pointer arithmetic does.
fn offset(base: DataRef, by: i32) -> DataRef {
    DataRef {
        archive: base.archive,
        offset: base.offset.wrapping_add(by as u32),
    }
}

/// Control point `index`'s component `k`, as a register holds it.
fn cv(data: &Archives, cvs: DataRef, index: i32, k: i32) -> f64 {
    reg(data.f32(offset(cvs, index * 12 + k * 4)))
}

/// The four control points from `first` weighted by `w`, component by component, in the
/// order the original accumulates them.
fn blend(data: &Archives, cvs: DataRef, first: i32, w: [f64; 4]) -> Vec3 {
    let c = |k| {
        let p = |i| cv(data, cvs, first + i, k);
        let s = fp::fmuls(p(1), w[1]);
        let s = fp::fmadds(p(0), w[0], s);
        let s = fp::fmadds(p(2), w[2], s);
        mem(fp::fmadds(p(3), w[3], s))
    };
    Vec3::new(c(0), c(1), c(2))
}

/// Control point `index` copied word for word.
fn copied(data: &Archives, cvs: DataRef, index: i32) -> Vec3 {
    let w = |k| data.f32(offset(cvs, index * 12 + k * 4));
    Vec3::new(w(0), w(1), w(2))
}

/// The point at parameter `u` of the spline at `spline`, `splGetSplinePoint`. `None` where
/// the original leaves its output alone: `u` outside [0, 1], or an unknown spline type.
// NaN takes the original's branches, which the negated comparison spells out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "splGetSplinePoint")]
pub fn point(data: &Archives, spline: DataRef, u: f32) -> Option<Vec3> {
    let u = reg(u);
    if u < 0.0 || u > 1.0 {
        return None;
    }
    let s = Spline::read(data, spline);
    let cvs = s.cv.expect("a spline has control points");
    let last = i32::from(s.numcv) - 1;
    if !(u < 1.0) {
        // The end of the curve (NaN comes here too).
        let idx = i32::from(last as i16);
        return match s.kind {
            0 => Some(copied(data, cvs, idx)),
            1 => Some(copied(data, cvs, idx * 3)),
            2 => {
                let k = reg(f32::from_bits(SIXTH));
                let zero = fp::fadds(fp::fneg(1.0), 1.0);
                let b2 = fp::fmuls(k, fp::fmadds(3.0, fp::fadds(1.0, zero), 1.0));
                Some(blend(data, cvs, idx - 1, [0.0, k, b2, k]))
            }
            3 => Some(copied(data, cvs, idx + 1)),
            _ => None,
        };
    }
    let t = fp::fmuls(u, fp::frsp(f64::from(last)));
    let idx = fp::fctiwz(t) as i16;
    let t = fp::fsubs(t, fp::frsp(f64::from(idx)));
    let idx = i32::from(idx);
    match s.kind {
        0 => {
            let c = |k| {
                let a = cv(data, cvs, idx, k);
                mem(fp::fmadds(t, fp::fsubs(cv(data, cvs, idx + 1, k), a), a))
            };
            Some(Vec3::new(c(0), c(1), c(2)))
        }
        1 => {
            let u1 = fp::fsubs(1.0, t);
            let u2 = fp::fmuls(t, t);
            let u1_2 = fp::fmuls(u1, u1);
            let b1 = fp::fmuls(fp::fmuls(3.0, t), u1_2);
            let b0 = fp::fmuls(u1_2, u1);
            let b2 = fp::fmuls(fp::fmuls(3.0, u2), u1);
            let b3 = fp::fmuls(u2, t);
            Some(blend(data, cvs, idx * 3, [b0, b1, b2, b3]))
        }
        2 => {
            let k = reg(f32::from_bits(SIXTH));
            let u2 = fp::fmuls(t, t);
            let u1 = fp::fsubs(1.0, t);
            let u3 = fp::fmuls(u2, t);
            let cubic = fp::fmsubs(3.0, u3, fp::fmuls(6.0, u2));
            let b0 = fp::fmuls(u1, fp::fmuls(u1, fp::fmuls(k, u1)));
            let b1 = fp::fmuls(k, fp::fadds(4.0, cubic));
            let inner = fp::fadds(t, fp::fadds(fp::fneg(u3), u2));
            let b3 = fp::fmuls(k, u3);
            let b2 = fp::fmuls(k, fp::fmadds(3.0, inner, 1.0));
            Some(blend(data, cvs, idx, [b0, b1, b2, b3]))
        }
        3 => {
            let tension = reg(s.tension);
            let u2 = fp::fmuls(t, t);
            let u3 = fp::fmuls(u2, t);
            let a = fp::fmuls(fp::fsubs(tension, 3.0), u2);
            let three_minus = fp::fnmsubs(2.0, tension, 3.0);
            let c0 = fp::fsubs(fp::fmadds(2.0, u2, fp::fneg(u3)), t);
            let c1 = fp::fadds(1.0, fp::fmadds(fp::fsubs(2.0, tension), u3, a));
            let b = fp::fmuls(three_minus, u2);
            let c0 = fp::fmuls(tension, c0);
            let c2 = fp::fmadds(fp::fsubs(tension, 2.0), u3, b);
            let c3 = fp::fmuls(tension, fp::fsubs(u3, u2));
            let c2 = fp::fmadds(tension, t, c2);
            Some(blend(data, cvs, idx, [c0, c1, c2, c3]))
        }
        _ => None,
    }
}

/// The square root of a segment's speed polynomial at `t`, with small negative values
/// clamped to 0 (`splArcLengthPolynomial`, inlined).
fn speed(data: &Archives, coeffs: DataRef, t: f64) -> f64 {
    let c = |i: u32| reg(data.f32(coeffs.at(4 * i)));
    let t2 = fp::fmuls(t, t);
    let t3 = fp::fmuls(t2, t);
    let t4 = fp::fmuls(t3, t);
    let s = fp::fmuls(c(1), t3);
    let s = fp::fmadds(c(0), t4, s);
    let s = fp::fmadds(c(2), t2, s);
    let s = fp::fmadds(c(3), t, s);
    let mut r = fp::fadds(c(4), s);
    if r < 0.0 && r > reg(-0.001) {
        r = 0.0;
    }
    reg(math::sqrt(mem(r)))
}

/// The curve parameter at fraction `s` of the spline's arc length,
/// `splArcLengthGetParameter`: the segment from the length table, then, for curved types,
/// bisection with Simpson's rule over the segment's speed.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "splArcLengthGetParameter")]
pub fn arc_length_parameter(data: &Archives, spline: DataRef, s: f32) -> f32 {
    let s = reg(s);
    if s <= 0.0 {
        return 0.0;
    }
    if s >= 1.0 {
        return 1.0;
    }
    let sp = Spline::read(data, spline);
    let seg = sp.seg_length.expect("a spline has a length table");
    let seg_at = |i: i32| reg(data.f32(offset(seg, 4 * i)));
    let mut idx = 0i32;
    while seg_at(idx + 1) < s {
        idx += 1;
    }
    let result = match sp.kind {
        0 => {
            let start = seg_at(idx);
            fp::fdivs(fp::fsubs(s, start), fp::fsubs(seg_at(idx + 1), start))
        }
        1..=3 => {
            let polys = sp.seg_poly.expect("a curved spline has speed polynomials");
            let coeffs = offset(polys, idx * 0x14);
            let tolerance = reg(1.0e-5);
            let mut remaining = fp::fmuls(reg(sp.total_length), fp::fsubs(s, seg_at(idx)));
            let (mut start, mut end) = (0.0, 1.0);
            let mut mid = 0.0;
            loop {
                let mut d = fp::fsubs(start, end);
                if d < 0.0 {
                    d = fp::fneg(d);
                }
                if !(d >= tolerance) {
                    break;
                }
                mid = fp::fmuls(fp::fadds(start, end), 0.5);
                let dx = fp::fmuls(fp::fsubs(mid, start), 0.125);
                let mut t = fp::fadds(start, dx);
                let mut inner = 0.0;
                for i in 2..=8 {
                    let w = if i & 1 == 0 { 4.0 } else { 2.0 };
                    inner = fp::fmadds(w, speed(data, coeffs, t), inner);
                    t = fp::fadds(t, dx);
                }
                let ends = fp::fadds(
                    fp::fadds(inner, speed(data, coeffs, start)),
                    speed(data, coeffs, mid),
                );
                let simpson = fp::fdivs(fp::fmuls(dx, ends), 3.0);
                if remaining < fp::fadds(tolerance, simpson) {
                    end = mid;
                } else {
                    start = mid;
                    remaining = fp::fsubs(remaining, simpson);
                }
            }
            mid
        }
        // The original returns whatever its caller left in a register; no spline on the
        // disc has another type.
        kind => panic!("spline type {kind} has no arc-length parameterization"),
    };
    mem(fp::fdivs(
        fp::fadds(result, fp::frsp(f64::from(idx))),
        fp::fsubs(fp::frsp(f64::from(sp.numcv)), 1.0),
    ))
}

/// The point at fraction `s` of the spline's arc length, `splArcLengthPoint`.
#[doc(alias = "splArcLengthPoint")]
pub fn arc_length_point(data: &Archives, spline: DataRef, s: f32) -> Option<Vec3> {
    point(data, spline, arc_length_parameter(data, spline, s))
}

/// The spline's direction (its derivative, unnormalized) at parameter `u`, `lbShadow_8000E9F0`
/// with its inline `splGetBezierTangent`, `splGetBSplineTangent` and `splGetCardinalTangent`.
/// `None` where the original leaves its output alone: `u` outside [0, 1], or an unknown type.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "lbShadow_8000E9F0")]
pub fn tangent(data: &Archives, spline: DataRef, u: f32) -> Option<Vec3> {
    let orig = reg(u);
    if orig < 0.0 || orig > 1.0 {
        return None;
    }
    let s = Spline::read(data, spline);
    let cvs = s.cv.expect("a spline has control points");
    let last = i32::from(s.numcv) - 1;
    let t = fp::fmuls(orig, fp::frsp(f64::from(last)));
    let whole = fp::fctiwz(t);
    let idx = i32::from(whole as i16);
    let u = fp::fsubs(t, fp::frsp(f64::from(idx)));
    match s.kind {
        0 => {
            let idx = if orig == 1.0 {
                i32::from(whole.wrapping_sub(1) as i16)
            } else {
                idx
            };
            let c = |k| mem(fp::fsubs(cv(data, cvs, idx + 1, k), cv(data, cvs, idx, k)));
            Some(Vec3::new(c(0), c(1), c(2)))
        }
        1 => {
            let u2 = fp::fmuls(u, u);
            let u_1 = fp::fsubs(u, 1.0);
            let b0 = fp::fmuls(fp::fmuls(-3.0, u_1), u_1);
            // 3u² is rounded before the sum (the machine code doesn't fuse it).
            let b1 = fp::fmuls(3.0, fp::fadds(fp::fnmsubs(4.0, u, 1.0), fp::fmuls(3.0, u2)));
            let b2 = fp::fmuls(3.0, fp::fmsubs(2.0, u, fp::fmuls(3.0, u2)));
            let b3 = fp::fmuls(3.0, u2);
            Some(blend(data, cvs, idx * 3, [b0, b1, b2, b3]))
        }
        2 => {
            let u2 = fp::fmuls(u, u);
            let u_1 = fp::fsubs(1.0, u);
            let b0 = fp::fmuls(u_1, fp::fmuls(-0.5, u_1));
            let b1 = fp::fmuls(0.5, fp::fmsubs(3.0, u2, fp::fmuls(4.0, u)));
            let b2 = fp::fmuls(0.5, fp::fadds(1.0, fp::fmadds(-3.0, u2, fp::fmuls(2.0, u))));
            let b3 = fp::fmuls(0.5, u2);
            Some(blend(data, cvs, idx, [b0, b1, b2, b3]))
        }
        3 => {
            let k = reg(s.tension);
            let u2 = fp::fmuls(u, u);
            let c0 = fp::fmuls(k, fp::fsubs(fp::fmadds(-3.0, u2, fp::fmuls(4.0, u)), 1.0));
            let c1 = fp::fmadds(
                fp::fmuls(3.0, fp::fsubs(2.0, k)),
                u2,
                fp::fmuls(fp::fmuls(2.0, fp::fsubs(k, 3.0)), u),
            );
            let c2 = fp::fadds(
                k,
                fp::fmadds(
                    fp::fmuls(3.0, fp::fsubs(k, 2.0)),
                    u2,
                    fp::fmuls(fp::fmuls(2.0, fp::fnmsubs(2.0, k, 3.0)), u),
                ),
            );
            let c3 = fp::fmuls(k, fp::fmsubs(3.0, u2, fp::fmuls(2.0, u)));
            Some(blend(data, cvs, idx, [c0, c1, c2, c3]))
        }
        _ => None,
    }
}
