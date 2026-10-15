//! Hit collision geometry (`melee/lb/lbcollision.c`): the segment, sphere and capsule tests
//! that decide whether hitboxes, hurtboxes, shields and other collision bubbles touch.
//!
//! A moving bubble sweeps a capsule from where it was last frame to where it is now. Each test
//! first rejects by the box around the first segment grown by both radii, and then writes the
//! closest points whether or not the shapes touch. A rejected test writes nothing, so results
//! past that box come back as `Some`.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use crate::fpu::{copy, mem, reg};
use crate::math::sqrt_reg;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// A vector held in registers, each component as `lfs` leaves it.
#[derive(Clone, Copy, melee_derive::StateEq)]
struct R {
    x: f64,
    y: f64,
    z: f64,
}

impl R {
    fn of(v: Vec3) -> R {
        R {
            x: reg(v.x),
            y: reg(v.y),
            z: reg(v.z),
        }
    }

    /// Componentwise `fsubs`.
    fn sub(self, b: R) -> R {
        R {
            x: fp::fsubs(self.x, b.x),
            y: fp::fsubs(self.y, b.y),
            z: fp::fsubs(self.z, b.z),
        }
    }

    /// `self·t + base`, fused per component.
    fn along(self, t: f64, base: R) -> R {
        R {
            x: fp::fmadds(self.x, t, base.x),
            y: fp::fmadds(self.y, t, base.y),
            z: fp::fmadds(self.z, t, base.z),
        }
    }

    /// Stored with `stfs`.
    fn stored(self) -> Vec3 {
        Vec3::new(mem(self.x), mem(self.y), mem(self.z))
    }

    /// Stored and loaded back, as a local the original keeps on the stack.
    fn spilled(self) -> R {
        R::of(self.stored())
    }
}

/// The dot product as the original fuses it: a.z·b.z + (a.x·b.x + a.y·b.y).
fn dot(a: R, b: R) -> f64 {
    fp::fmadds(a.z, b.z, fp::fmadds(a.x, b.x, fp::fmuls(a.y, b.y)))
}

/// The same in the XY plane: a.x·b.x + a.y·b.y.
fn dot_xy(a: R, b: R) -> f64 {
    fp::fmadds(a.x, b.x, fp::fmuls(a.y, b.y))
}

/// `approximatelyZero`: strictly within 1e-5 of zero.
fn near_zero(x: f64) -> bool {
    let bound = f64::from(1.0e-5_f32);
    x < bound && x > fp::fneg(bound)
}

/// A segment parameter clamped to [0, 1]. NaN stays NaN, as the original's branches leave it.
// Spelled as the original's branches rather than `clamp`, so NaN and -0.0 visibly pass through.
#[allow(clippy::manual_clamp)]
fn clamp_unit(t: f64) -> f64 {
    if t > 1.0 {
        1.0
    } else if t < 0.0 {
        0.0
    } else {
        t
    }
}

/// Whether point `p` lies outside segment `a`–`b` grown by `reach`, on one axis.
fn outside_point(a: f64, b: f64, p: f64, reach: f64) -> bool {
    if a > b {
        fp::fadds(a, reach) < p || fp::fsubs(b, reach) > p
    } else {
        fp::fsubs(a, reach) > p || fp::fadds(b, reach) < p
    }
}

/// Whether both ends `c` and `d` lie on the same side outside segment `a`–`b` grown by `reach`,
/// on one axis.
fn outside_both(a: f64, b: f64, c: f64, d: f64, reach: f64) -> bool {
    let above = |x: f64| x < c && x < d;
    let below = |x: f64| x > c && x > d;
    if a > b {
        above(fp::fadds(a, reach)) || below(fp::fsubs(b, reach))
    } else {
        below(fp::fsubs(a, reach)) || above(fp::fadds(b, reach))
    }
}

/// Whether segment `b0`–`b1` is outside segment `a0`–`a1`'s box grown by `reach`, on x, y and,
/// unless `xy_only`, z.
fn outside_box(a0: R, a1: R, b0: R, b1: R, reach: f64, xy_only: bool) -> bool {
    outside_both(a0.x, a1.x, b0.x, b1.x, reach)
        || outside_both(a0.y, a1.y, b0.y, b1.y, reach)
        || (!xy_only && outside_both(a0.z, a1.z, b0.z, b1.z, reach))
}

/// The inline projection the parallel cases use: the parameter on segment `start`–`end` of
/// the point closest to `p`, clamped. The segment's direction goes through the stack.
fn project_clamped(start: Vec3, end: Vec3, p: Vec3) -> f64 {
    let (s, p) = (R::of(start), R::of(p));
    let d = R::of(end).sub(s).spilled();
    clamp_unit(fp::fdivs(fp::fneg(dot(d, s.sub(p))), dot(d, d)))
}

/// The inline `sqrtf` whose result goes through the stack before it is used.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn sqrt_spilled(x: f64) -> f64 {
    if !(x > 0.0) {
        return x;
    }
    reg(mem(sqrt_reg(x)))
}

/// Where a segment passes a sphere: the point on the segment closest to the center.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct SegmentSphere {
    pub closest: Vec3,
    /// Whether that point is within the two radii's sum of the center.
    pub touching: bool,
}

/// Tests a sphere of radius `radius + other_radius` around `center` against segment
/// `start`–`end`, `lbColl_80005C44`.
#[doc(alias = "lbColl_80005C44")]
pub fn segment_sphere(
    start: Vec3,
    end: Vec3,
    center: Vec3,
    radius: f32,
    other_radius: f32,
) -> Option<SegmentSphere> {
    let reach = fp::fadds(reg(radius), reg(other_radius));
    let (a, b, c) = (R::of(start), R::of(end), R::of(center));
    if outside_point(a.x, b.x, c.x, reach)
        || outside_point(a.y, b.y, c.y, reach)
        || outside_point(a.z, b.z, c.z, reach)
    {
        return None;
    }
    let d1 = b.sub(a).spilled();
    let d2 = a.sub(c);
    let len_sq = dot(d1, d1);
    let t = if near_zero(len_sq) {
        0.0
    } else {
        clamp_unit(fp::fdivs(fp::fneg(dot(d1, d2)), len_sq))
    };
    let closest = d1.along(t, a).stored();
    let gap = R::of(closest).sub(c);
    Some(SegmentSphere {
        closest,
        touching: !(fp::fmuls(reach, reach) < dot(gap, gap)),
    })
}

/// The point on a segment closest to another point.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct SegmentPoint {
    /// The squared distance between them.
    pub distance_sq: f32,
    /// How far along the segment the closest point lies, 0 at its start and 1 at its end.
    pub t: f32,
}

/// The point on segment `start`–`end` closest to `p`, `lbColl_80005EBC`. A segment of zero
/// length divides by zero, as the original does.
#[doc(alias = "lbColl_80005EBC")]
pub fn closest_on_segment(start: Vec3, end: Vec3, p: Vec3) -> SegmentPoint {
    let (a, b, p) = (R::of(start), R::of(end), R::of(p));
    let d1 = b.sub(a).spilled();
    let d2 = a.sub(p).spilled();
    let t = clamp_unit(fp::fdivs(fp::fneg(dot(d1, d2)), dot(d1, d1)));
    let off = d1.along(t, a).sub(p);
    SegmentPoint {
        distance_sq: mem(dot(off, off)),
        t: mem(t),
    }
}

/// The same in the XY plane, `lbColl_80005FC0`.
#[doc(alias = "lbColl_80005FC0")]
pub fn closest_on_segment_xy(start: Vec3, end: Vec3, p: Vec3) -> SegmentPoint {
    let (a, b, p) = (R::of(start), R::of(end), R::of(p));
    let d1 = b.sub(a).spilled();
    let d2 = a.sub(p).spilled();
    let t = clamp_unit(fp::fdivs(fp::fneg(dot_xy(d1, d2)), dot_xy(d1, d1)));
    let off = d1.along(t, a).sub(p);
    SegmentPoint {
        distance_sq: mem(dot_xy(off, off)),
        t: mem(t),
    }
}

/// The closest points between two segments.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct ClosestPoints {
    pub on_first: Vec3,
    pub on_second: Vec3,
    /// Whether they are within the two radii's sum of each other.
    pub touching: bool,
}

/// When the unconstrained closest points leave either segment: the better of the two
/// endpoint projections, as segment parameters `(s, t)`.
fn endpoint_params(
    s: f64,
    t: f64,
    first: (Vec3, Vec3),
    second: (Vec3, Vec3),
    closest: fn(Vec3, Vec3, Vec3) -> SegmentPoint,
) -> (f64, f64) {
    let (s_end, from_first) = if s < 0.0 {
        (0.0, closest(second.0, second.1, first.0))
    } else {
        (1.0, closest(second.0, second.1, first.1))
    };
    let (t_end, from_second) = if t < 0.0 {
        (0.0, closest(first.0, first.1, second.0))
    } else {
        (1.0, closest(first.0, first.1, second.1))
    };
    if reg(from_first.distance_sq) < reg(from_second.distance_sq) {
        (s_end, reg(from_first.t))
    } else {
        (reg(from_second.t), t_end)
    }
}

fn out_of_unit(s: f64, t: f64) -> bool {
    s > 1.0 || s < 0.0 || t > 1.0 || t < 0.0
}

/// Tests two capsules, segments `first_start`–`first_end` and `second_start`–`second_end` with
/// radii summed, `lbColl_80006094`.
#[doc(alias = "lbColl_80006094")]
pub fn capsules(
    first_start: Vec3,
    first_end: Vec3,
    second_start: Vec3,
    second_end: Vec3,
    first_radius: f32,
    second_radius: f32,
) -> Option<ClosestPoints> {
    capsules_reg(
        (first_start, first_end),
        (second_start, second_end),
        reg(first_radius),
        reg(second_radius),
    )
}

fn capsules_reg(
    first: (Vec3, Vec3),
    second: (Vec3, Vec3),
    first_radius: f64,
    second_radius: f64,
) -> Option<ClosestPoints> {
    let reach = fp::fadds(first_radius, second_radius);
    let (a0, a1, b0, b1) = (
        R::of(first.0),
        R::of(first.1),
        R::of(second.0),
        R::of(second.1),
    );
    if outside_box(a0, a1, b0, b1, reach, false) {
        return None;
    }
    let (d1, d2, between) = (a1.sub(a0), b1.sub(b0), a0.sub(b0));
    let d1_len_sq = dot(d1, d1);
    let d2_len_sq = dot(d2, d2);
    let d1_dot_d2 = dot(d1, d2);
    let d2_dot_between = dot(d2, between);
    let d1_dot_between = dot(d1, between);
    let denom = fp::fmsubs(d1_len_sq, d2_len_sq, fp::fmuls(d1_dot_d2, d1_dot_d2));
    let (s, t) = if near_zero(d2_len_sq) {
        if near_zero(d1_len_sq) {
            (0.0, 0.0)
        } else {
            let s = fp::fdivs(fp::fneg(d1_dot_between), d1_len_sq);
            (clamp_unit(s), 0.0)
        }
    } else if near_zero(denom) {
        // Parallel: from whichever end of the first segment is nearer the second's midpoint.
        let half = |d: f64, base: f64| fp::frsp(fp::fmadd(0.5, d, base));
        let mid = R {
            x: half(d2.x, b0.x),
            y: half(d2.y, b0.y),
            z: half(d2.z, b0.z),
        }
        .spilled();
        let (near, far) = (a0.sub(mid), a1.sub(mid));
        if dot(near, near) < dot(far, far) {
            (0.0, project_clamped(second.0, second.1, first.0))
        } else {
            (1.0, project_clamped(second.0, second.1, first.1))
        }
    } else {
        let s = fp::fdivs(
            fp::fmsubs(
                d1_dot_d2,
                d2_dot_between,
                fp::fmuls(d2_len_sq, d1_dot_between),
            ),
            denom,
        );
        let t = fp::fdivs(
            fp::fmsubs(
                d1_len_sq,
                d2_dot_between,
                fp::fmuls(d1_dot_d2, d1_dot_between),
            ),
            denom,
        );
        if out_of_unit(s, t) {
            endpoint_params(s, t, first, second, closest_on_segment)
        } else {
            (s, t)
        }
    };
    let on_first = d1.along(s, a0).stored();
    let on_second = d2.along(t, b0).stored();
    let gap = R::of(on_first).sub(R::of(on_second));
    Some(ClosestPoints {
        on_first,
        on_second,
        touching: !(fp::fmuls(reach, reach) < dot(gap, gap)),
    })
}

/// The same in the XY plane, `lbColl_800067F8`: the closest points come back with z = 0.
// The parallel case projects in 3D, as the original does.
#[doc(alias = "lbColl_800067F8")]
pub fn capsules_xy(
    first_start: Vec3,
    first_end: Vec3,
    second_start: Vec3,
    second_end: Vec3,
    first_radius: f32,
    second_radius: f32,
) -> Option<ClosestPoints> {
    capsules_xy_reg(
        (first_start, first_end),
        (second_start, second_end),
        reg(first_radius),
        reg(second_radius),
    )
}

fn capsules_xy_reg(
    first: (Vec3, Vec3),
    second: (Vec3, Vec3),
    first_radius: f64,
    second_radius: f64,
) -> Option<ClosestPoints> {
    let reach = fp::fadds(first_radius, second_radius);
    let (a, b, c, d) = (
        R::of(first.0),
        R::of(first.1),
        R::of(second.0),
        R::of(second.1),
    );
    if outside_box(a, b, c, d, reach, true) {
        return None;
    }
    let ba = b.sub(a).spilled();
    let dc = d.sub(c).spilled();
    let ac = a.sub(c);
    let ba_len_sq = dot_xy(ba, ba);
    let dc_len_sq = dot_xy(dc, dc);
    let ba_dot_dc = dot_xy(ba, dc);
    let dc_dot_ac = dot_xy(dc, ac);
    let ba_dot_ac = dot_xy(ba, ac);
    let det = fp::fmsubs(ba_len_sq, dc_len_sq, fp::fmuls(ba_dot_dc, ba_dot_dc));
    let (s, t) = if near_zero(dc_len_sq) {
        if near_zero(ba_len_sq) {
            (0.0, 0.0)
        } else {
            (clamp_unit(fp::fdivs(fp::fneg(ba_dot_ac), ba_len_sq)), 0.0)
        }
    } else if near_zero(det) {
        let half = |d: f64, base: f64| reg(mem(fp::frsp(fp::fmadd(0.5, d, base))));
        let mid_y = half(dc.y, c.y);
        let mid_x = half(dc.x, c.x);
        let near = R {
            x: fp::fsubs(a.x, mid_x),
            y: fp::fsubs(a.y, mid_y),
            z: 0.0,
        };
        let far = R {
            x: fp::fsubs(b.x, mid_x),
            y: fp::fsubs(b.y, mid_y),
            z: 0.0,
        };
        if dot_xy(near, near) < dot_xy(far, far) {
            (0.0, project_clamped(second.0, second.1, first.0))
        } else {
            (1.0, project_clamped(second.0, second.1, first.1))
        }
    } else {
        let s = fp::fdivs(
            fp::fmsubs(ba_dot_dc, dc_dot_ac, fp::fmuls(dc_len_sq, ba_dot_ac)),
            det,
        );
        let t = fp::fdivs(
            fp::fmsubs(ba_len_sq, dc_dot_ac, fp::fmuls(ba_dot_dc, ba_dot_ac)),
            det,
        );
        if out_of_unit(s, t) {
            endpoint_params(s, t, first, second, closest_on_segment_xy)
        } else {
            (s, t)
        }
    };
    let on_first = Vec3::new(
        mem(fp::fmadds(ba.x, s, a.x)),
        mem(fp::fmadds(ba.y, s, a.y)),
        0.0,
    );
    let on_second = Vec3::new(
        mem(fp::fmadds(dc.x, t, c.x)),
        mem(fp::fmadds(dc.y, t, c.y)),
        0.0,
    );
    let gap = R::of(on_first).sub(R::of(on_second));
    Some(ClosestPoints {
        on_first,
        on_second,
        touching: !(fp::fmuls(reach, reach) < dot_xy(gap, gap)),
    })
}

/// A hitbox's capsule against a hurtbox's or shield's.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct HitHurt {
    /// The closest point on the hitbox's segment.
    pub hit_closest: Vec3,
    /// The closest point on the hurtbox's segment.
    pub hurt_closest: Vec3,
    /// Where they meet, on the hurtbox's surface.
    pub contact: Vec3,
    /// The radii's sum, the hurtbox's measured in its bone's space, minus the distance.
    pub overlap: f32,
    pub touching: bool,
}

/// Tests a hitbox's capsule against a hurtbox's or shield's, `lbColl_80006E58`. The hurtbox's
/// radius is measured along the contact direction in `hurt_mtx`'s space, so a scaled bone
/// stretches it; `broadphase_scale` grows it only for the first rejection.
#[doc(alias = "lbColl_80006E58")]
#[allow(clippy::too_many_arguments)]
pub fn hit_hurt(
    hit_start: Vec3,
    hit_end: Vec3,
    hurt_start: Vec3,
    hurt_end: Vec3,
    hurt_mtx: &Mtx34,
    hit_radius: f32,
    hurt_radius: f32,
    broadphase_scale: f32,
) -> Option<HitHurt> {
    let (hit_r, hurt_r) = (reg(hit_radius), reg(hurt_radius));
    let reach = fp::fmadds(hurt_r, reg(broadphase_scale), hit_r);
    let (hs, he, us, ue) = (
        R::of(hit_start),
        R::of(hit_end),
        R::of(hurt_start),
        R::of(hurt_end),
    );
    if outside_box(hs, he, us, ue, reach, false) {
        return None;
    }
    let hit_delta = he.sub(hs).spilled();
    let axis = ue.sub(us).spilled();
    // The hit segment's squared length: its squares stored and summed unfused, y's taken from
    // a copy of the delta's y.
    let delta_y = reg(mem(hit_delta.y));
    let square = |v: f64, w: f64| reg(mem(fp::fmuls(v, w)));
    let (sq_y, sq_x, sq_z) = (
        square(delta_y, delta_y),
        square(hit_delta.x, hit_delta.x),
        square(hit_delta.z, hit_delta.z),
    );
    let hit_len_sq = fp::fadds(sq_z, fp::fadds(sq_x, sq_y));
    let offset = hs.sub(us).spilled();
    let hurt_start_dot = dot(axis, offset);
    let delta = R {
        y: delta_y,
        ..hit_delta
    };
    let segment_dot = dot(delta, axis);
    let hit_start_dot = dot(delta, offset);
    let hurt_len_sq = dot(axis, axis);
    let denom = fp::fmsubs(hit_len_sq, hurt_len_sq, fp::fmuls(segment_dot, segment_dot));
    let (hit_t, hurt_t) = if near_zero(hurt_len_sq) {
        if near_zero(hit_len_sq) {
            (0.0, 0.0)
        } else {
            (
                clamp_unit(fp::fdivs(fp::fneg(hit_start_dot), hit_len_sq)),
                0.0,
            )
        }
    } else if near_zero(denom) {
        let half = |d: f64, base: f64| fp::frsp(fp::fmadd(0.5, d, base));
        let mid = R {
            x: half(axis.x, us.x),
            y: half(axis.y, us.y),
            z: half(axis.z, us.z),
        }
        .spilled();
        let (near, far) = (hs.sub(mid).spilled(), he.sub(mid).spilled());
        if dot(near, near) < dot(far, far) {
            (0.0, project_clamped(hurt_start, hurt_end, hit_start))
        } else {
            (1.0, project_clamped(hurt_start, hurt_end, hit_end))
        }
    } else {
        let hit_t = fp::fdivs(
            fp::fmsubs(
                segment_dot,
                hurt_start_dot,
                fp::fmuls(hurt_len_sq, hit_start_dot),
            ),
            denom,
        );
        let hurt_t = fp::fdivs(
            fp::fmsubs(
                hit_len_sq,
                hurt_start_dot,
                fp::fmuls(segment_dot, hit_start_dot),
            ),
            denom,
        );
        if out_of_unit(hit_t, hurt_t) {
            endpoint_params(
                hit_t,
                hurt_t,
                (hit_start, hit_end),
                (hurt_start, hurt_end),
                closest_on_segment,
            )
        } else {
            (hit_t, hurt_t)
        }
    };
    let hit_closest = hit_delta.along(hit_t, hs).stored();
    let hurt_closest = axis.along(hurt_t, us).stored();
    let (hc, uc) = (R::of(hit_closest), R::of(hurt_closest));
    let gap = hc.sub(uc).spilled();
    let distance = reg(mem(sqrt_reg(dot(gap, gap))));
    if near_zero(distance) {
        return Some(HitHurt {
            hit_closest,
            hurt_closest,
            contact: hit_closest,
            overlap: mem(fp::fsubs(fp::fadds(hit_r, hurt_r), distance)),
            touching: true,
        });
    }
    let inv = hurt_mtx.hsd_inverse();
    let local_hit = R::of(inv.transform(hit_closest));
    let local_hurt = R::of(inv.transform(hurt_closest));
    let local_gap = local_hit.sub(local_hurt).spilled();
    let local_distance = sqrt_reg(dot(local_gap, local_gap));
    let scaled_hurt_r = fp::fdivs(fp::fmuls(hurt_r, distance), local_distance);
    let lerp = fp::fdivs(scaled_hurt_r, distance);
    let allowed = fp::fadds(hit_r, scaled_hurt_r);
    let toward = |h: f64, u: f64| mem(fp::fmadds(lerp, fp::fsubs(h, u), u));
    Some(HitHurt {
        hit_closest,
        hurt_closest,
        contact: Vec3::new(toward(hc.x, uc.x), toward(hc.y, uc.y), toward(hc.z, uc.z)),
        overlap: mem(fp::fsubs(allowed, distance)),
        touching: !(allowed < distance),
    })
}

/// Where a segment enters a sphere, seen from its center.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct SphereEntry {
    /// The angle in the XY plane between `normal` and the segment's direction; π for a
    /// segment of zero length.
    pub angle: f32,
    /// The unit direction from the center to the entry point; zero for a segment of zero
    /// length.
    pub normal: Vec3,
    /// The point at the sphere's own radius along `normal`; `None` for a segment of zero
    /// length, which leaves it unwritten.
    pub point: Option<Vec3>,
}

/// Where segment `start`–`end` first comes within `radius + extra` of `center`,
/// `lbColl_800077A0`. `radius` is measured through `mtx`, so a scaled bone stretches it; past
/// the sphere's reach, the segment's nearest point stands in.
#[doc(alias = "lbColl_800077A0")]
pub fn sphere_entry(
    center: Vec3,
    mtx: &Mtx34,
    start: Vec3,
    end: Vec3,
    radius: f32,
    extra: f32,
) -> SphereEntry {
    let (a, b, c) = (R::of(center), R::of(start), R::of(end));
    let along = c.sub(b).stored();
    let cb = R::of(along);
    if !(cb.x != 0.0 || cb.y != 0.0 || cb.z != 0.0) {
        return SphereEntry {
            angle: mem(fp::frsp(std::f64::consts::PI)),
            normal: Vec3::ZERO,
            point: None,
        };
    }
    let rim = mtx.transform(Vec3::new(copy(radius), 0.0, 0.0));
    let origin = mtx.transform(Vec3::ZERO);
    let spoke = R::of(rim).sub(R::of(origin));
    let measured = sqrt_spilled(dot(spoke, spoke));
    let reach = fp::fadds(measured, reg(extra));
    let ba = b.sub(a).spilled();
    let cb_len_sq = fp::fadds(
        fp::fmuls(cb.z, cb.z),
        fp::fadds(fp::fmuls(cb.x, cb.x), fp::fmuls(cb.y, cb.y)),
    );
    let t = if near_zero(cb_len_sq) {
        0.0
    } else {
        // The smaller root of |b + t·(c - b) - a|² = reach².
        let twice = |v: f64| fp::fmuls(2.0, v);
        let n0 = fp::fmadds(
            twice(cb.z),
            ba.z,
            fp::fmadds(twice(cb.x), ba.x, fp::fmuls(twice(cb.y), ba.y)),
        );
        let ba_len_sq = dot(ba, ba);
        let mut disc = fp::fmsubs(
            n0,
            n0,
            fp::fmuls(
                fp::fmuls(4.0, cb_len_sq),
                fp::fnmsubs(reach, reach, ba_len_sq),
            ),
        );
        if disc < 0.0 {
            disc = 0.0;
        }
        fp::fdivs(
            fp::fsubs(fp::fneg(n0), sqrt_spilled(disc)),
            fp::fmuls(2.0, cb_len_sq),
        )
    };
    let toward = R {
        x: fp::fsubs(fp::fmadds(t, cb.x, b.x), a.x),
        y: fp::fsubs(fp::fmadds(t, cb.y, b.y), a.y),
        z: fp::fsubs(fp::fmadds(t, cb.z, b.z), a.z),
    }
    .stored();
    let normal = toward.sdk_normalize();
    let angle = normal.angle_xy(along);
    let n = R::of(normal);
    let point = Vec3::new(
        mem(fp::fmadds(measured, n.x, a.x)),
        mem(fp::fmadds(measured, n.y, a.y)),
        mem(fp::fmadds(measured, n.z, a.z)),
    );
    SphereEntry {
        angle,
        normal,
        point: Some(point),
    }
}

/// A hitbox's shape: the parts of `HitCapsule` the collision tests read and write.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct HitShape {
    /// `scale`: the radius, before the owner's scale.
    pub radius: f32,
    /// `x43_b1`: the radius ignores the owner's scale.
    pub fixed_radius: bool,
    /// `x4C`: where the hitbox is this frame.
    pub pos: Vec3,
    /// `x58`: where it was last frame; the capsule sweeps from here to `pos`.
    pub prev_pos: Vec3,
    /// `hurt_coll_pos`: where the last test met another shape.
    pub contact: Vec3,
}

impl HitShape {
    /// The radius the tests use, in a register.
    fn scaled_radius(&self, owner_scale: f32) -> f64 {
        if self.fixed_radius {
            reg(self.radius)
        } else {
            fp::fmuls(reg(self.radius), reg(owner_scale))
        }
    }
}

/// Tests two hitboxes against each other, as clashing attacks do, `lbColl_80007AFC`. Each
/// one's `contact` gets its closest point to the other unless the first rejection skips them.
#[doc(alias = "lbColl_80007AFC")]
pub fn hitboxes_touch(a: &mut HitShape, b: &mut HitShape, a_scale: f32, b_scale: f32) -> bool {
    let a_radius = a.scaled_radius(a_scale);
    let b_radius = b.scaled_radius(b_scale);
    let Some(points) = capsules_reg((b.prev_pos, b.pos), (a.prev_pos, a.pos), b_radius, a_radius)
    else {
        return false;
    };
    b.contact = points.on_first;
    a.contact = points.on_second;
    points.touching
}

/// A circle swept in the XY plane: the parts of `Fighter_x1614_t` (a fighter's coin pickup
/// bubbles, and a coin's) the collision tests read and write.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SweptCircle {
    /// `x0`: the radius, before the owner's scale.
    pub radius: f32,
    /// `x8`: where the circle is this frame.
    pub pos: Vec3,
    /// `x14`: where it was last frame.
    pub prev_pos: Vec3,
    /// `x20`: where the last test met another shape.
    pub contact: Vec3,
}

/// Tests two swept circles against each other in the XY plane, `lbColl_80007B78`. Each one's
/// `contact` gets its closest point to the other unless the first rejection skips them.
#[doc(alias = "lbColl_80007B78")]
pub fn swept_circles_touch(
    a: &mut SweptCircle,
    b: &mut SweptCircle,
    a_scale: f32,
    b_scale: f32,
) -> bool {
    let b_radius = fp::fmuls(reg(b.radius), reg(b_scale));
    let a_radius = fp::fmuls(reg(a.radius), reg(a_scale));
    let Some(points) =
        capsules_xy_reg((b.prev_pos, b.pos), (a.prev_pos, a.pos), b_radius, a_radius)
    else {
        return false;
    };
    b.contact = points.on_first;
    a.contact = points.on_second;
    points.touching
}
