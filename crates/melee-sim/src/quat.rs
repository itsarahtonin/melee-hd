//! Quaternions, with HAL's quaternion library (`sysdolphin/baselib/quatlib.c`).

use std::ops::Mul;

use gekko_fp as fp;

use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `Quaternion` in the original: x, y, z, then w.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// `FLT_MIN`, quatlib's threshold for a zero length.
const FLT_MIN: f32 = f32::MIN_POSITIVE;

/// A float's negation as `fneg` makes it in a register, for a function that takes it as a
/// `float` argument.
fn neg(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ 0x8000_0000)
}

/// Column `c`'s length, as quatlib's and `HSD_MtxGetRotation`'s inline `sqrtf` computes it.
pub(crate) fn column_length(m: &Mtx34, c: usize) -> f32 {
    let g = |r: usize| reg(m.m[r][c]);
    math::sqrt(mem(fp::fadds(
        fp::fmuls(g(2), g(2)),
        fp::fadds(fp::fmuls(g(0), g(0)), fp::fmuls(g(1), g(1))),
    )))
}

impl Quat {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Quat { x, y, z, w }
    }

    fn lanes(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// The rotation of a matrix whose columns may be scaled, `MatToQuat`.
    #[doc(alias = "MatToQuat")]
    pub fn from_matrix(m: &Mtx34) -> Quat {
        const NEXT: [usize; 3] = [1, 2, 0];
        let len = [
            column_length(m, 0),
            column_length(m, 1),
            column_length(m, 2),
        ];
        // Entry (r, c) of the matrix with its columns normalized.
        let d = |r: usize, c: usize| fp::fdivs(reg(m.m[r][c]), reg(len[c]));
        let s = fp::fadds(d(2, 2), fp::fadds(d(0, 0), d(1, 1)));
        if s > 0.0 {
            let s = reg(math::sqrt(mem(fp::fadds(1.0, s))));
            let scale = fp::fdivs(0.5, s);
            return Quat {
                w: mem(fp::fmuls(0.5, s)),
                x: mem(fp::fmuls(scale, fp::fsubs(d(2, 1), d(1, 2)))),
                y: mem(fp::fmuls(scale, fp::fsubs(d(0, 2), d(2, 0)))),
                z: mem(fp::fmuls(scale, fp::fsubs(d(1, 0), d(0, 1)))),
            };
        }
        let mut i = 0;
        if d(1, 1) > d(0, 0) {
            i = 1;
        }
        if d(2, 2) > d(i, i) {
            i = 2;
        }
        let j = NEXT[i];
        let k = NEXT[j];
        let s = reg(math::sqrt(mem(fp::fadds(
            1.0,
            fp::fsubs(fp::fsubs(d(i, i), d(j, j)), d(k, k)),
        ))));
        let scale = fp::fdivs(0.5, s);
        let mut v = [0.0; 3];
        v[i] = mem(fp::fmuls(0.5, s));
        let w = mem(fp::fmuls(scale, fp::fsubs(d(k, j), d(j, k))));
        v[j] = mem(fp::fmuls(scale, fp::fadds(d(j, i), d(i, j))));
        v[k] = mem(fp::fmuls(scale, fp::fadds(d(k, i), d(i, k))));
        Quat::new(copy(v[0]), copy(v[1]), copy(v[2]), w)
    }

    /// The rotation by `angle` radians about `axis`, `HSD_QuatLib_8037ECE0`; `None` when the
    /// axis has no length.
    #[doc(alias = "HSD_QuatLib_8037ECE0")]
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Option<Quat> {
        let (x, y, z) = (reg(axis.x), reg(axis.y), reg(axis.z));
        let len = reg(math::sqrt(mem(fp::fadds(
            fp::fmuls(z, z),
            fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
        ))));
        if fp::fabs(len) < f64::from(FLT_MIN) {
            return None;
        }
        let inv = fp::fdivs(1.0, len);
        let half = mem(fp::fmuls(0.5, reg(angle)));
        let w = math::cos(half);
        let s = reg(math::sin(half));
        Some(Quat {
            x: mem(fp::fmuls(s, fp::fmuls(inv, x))),
            y: mem(fp::fmuls(s, fp::fmuls(inv, y))),
            z: mem(fp::fmuls(s, fp::fmuls(inv, z))),
            w,
        })
    }

    /// The rotation by Euler angles about x, then y, then z, `EulerToQuat`.
    #[doc(alias = "EulerToQuat")]
    pub fn from_euler(angles: Vec3) -> Quat {
        let half = |a: f32| mem(fp::fmuls(0.5, reg(a)));
        let (hx, hy, hz) = (half(angles.x), half(angles.y), half(angles.z));
        let (cx, cy, cz) = (reg(math::cos(hx)), reg(math::cos(hy)), reg(math::cos(hz)));
        let (sx, sy, sz) = (reg(math::sin(hx)), reg(math::sin(hy)), reg(math::sin(hz)));
        let ss = fp::fmuls(sy, sz);
        let cc = fp::fmuls(cy, cz);
        Quat {
            w: mem(fp::fmadds(cx, cc, fp::fmuls(sx, ss))),
            x: mem(fp::fmsubs(sx, cc, fp::fmuls(cx, ss))),
            y: mem(fp::fmadds(
                cz,
                fp::fmuls(cx, sy),
                fp::fmuls(sz, fp::fmuls(sx, cy)),
            )),
            z: mem(fp::fmsubs(
                sz,
                fp::fmuls(cx, cy),
                fp::fmuls(cz, fp::fmuls(sx, sy)),
            )),
        }
    }

    /// The spherical interpolation from `self` to `q` at `t`, `HSD_QuatLib_8037EF28`.
    ///
    /// Opposite quaternions interpolate through `q` along a sine curve; the original first
    /// stores a perpendicular quaternion to the output and then overwrites it, which no
    /// caller can see because none passes an input as the output.
    #[doc(alias = "HSD_QuatLib_8037EF28")]
    pub fn slerp(self, q: Quat, t: f32) -> Quat {
        let (p, q) = (self, q);
        let t = reg(t);
        let one = reg(1.0);
        let dot = fp::fmuls(reg(p.y), reg(q.y));
        let dot = fp::fmadds(reg(p.x), reg(q.x), dot);
        let dot = fp::fmadds(reg(p.z), reg(q.z), dot);
        let cosom = fp::fmadds(reg(p.w), reg(q.w), dot);
        let epsilon = reg(1.0e-10);
        let (sp, sq) = if fp::fadds(one, cosom) > epsilon {
            if fp::fsubs(one, cosom) > epsilon {
                let theta = reg(math::acos(mem(cosom)));
                let sinom = reg(math::sin(mem(theta)));
                let sp = reg(math::sin(mem(fp::fmuls(fp::fsubs(one, t), theta))));
                let sp = fp::fdivs(sp, sinom);
                let sq = reg(math::sin(mem(fp::fmuls(t, theta))));
                (sp, fp::fdivs(sq, sinom))
            } else {
                (fp::frsp(fp::fsub(1.0, t)), t)
            }
        } else {
            let t = if t < reg(0.5) {
                t
            } else {
                fp::fsubs(t, reg(0.5))
            };
            let t2 = fp::fmuls(reg(2.0), t);
            let half_pi = std::f64::consts::FRAC_PI_2;
            let sp = reg(math::sin(mem(fp::frsp(fp::fmul(
                half_pi,
                fp::fsubs(one, t2),
            )))));
            let sq = reg(math::sin(mem(fp::frsp(fp::fmul(half_pi, t2)))));
            (sp, sq)
        };
        let (p, q) = (p.lanes(), q.lanes());
        let lane = |i: usize| mem(fp::fmadds(sp, reg(p[i]), fp::fmuls(sq, reg(q[i]))));
        Quat::new(lane(0), lane(1), lane(2), lane(3))
    }
}

/// The product `self · q`, `HSD_QuatLib_8037EC4C`.
impl Mul for Quat {
    type Output = Quat;
    #[doc(alias = "HSD_QuatLib_8037EC4C")]
    fn mul(self, q: Quat) -> Quat {
        let p = self;
        let (px, py, pz, pw) = (reg(p.x), reg(p.y), reg(p.z), reg(p.w));
        let (qx, qy, qz, qw) = (reg(q.x), reg(q.y), reg(q.z), reg(q.w));
        Quat {
            x: mem(fp::fadds(
                fp::fmadds(qw, px, fp::fmuls(pw, qx)),
                fp::fmsubs(py, qz, fp::fmuls(qy, pz)),
            )),
            y: mem(fp::fadds(
                fp::fmadds(qw, py, fp::fmuls(pw, qy)),
                fp::fmsubs(qx, pz, fp::fmuls(px, qz)),
            )),
            z: mem(fp::fadds(
                fp::fmadds(qw, pz, fp::fmuls(pw, qz)),
                fp::fmsubs(px, qy, fp::fmuls(qx, py)),
            )),
            w: mem(fp::fmsubs(
                pw,
                qw,
                fp::fmadds(pz, qz, fp::fmadds(px, qx, fp::fmuls(py, qy))),
            )),
        }
    }
}

impl Mtx34 {
    /// Euler angles (x, then y, then z) of the rotation, `HSD_QuatLib_8037EB28`; near the
    /// poles z is zero.
    #[doc(alias = "HSD_QuatLib_8037EB28")]
    pub fn euler_angles(&self) -> Vec3 {
        let m = &self.m;
        let (m00, m10) = (reg(m[0][0]), reg(m[1][0]));
        let len = math::sqrt(mem(fp::fadds(fp::fmuls(m00, m00), fp::fmuls(m10, m10))));
        if reg(len) > 1e-5 {
            Vec3::new(
                math::atan2(m[2][1], m[2][2]),
                math::atan2(neg(m[2][0]), len),
                math::atan2(m[1][0], m[0][0]),
            )
        } else {
            Vec3::new(
                math::atan2(neg(m[1][2]), m[1][1]),
                math::atan2(neg(m[2][0]), len),
                0.0,
            )
        }
    }
}
