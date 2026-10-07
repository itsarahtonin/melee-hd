//! 3×4 and 4×4 matrices, with the SDK's matrix routines (`dolphin/mtx/mtx.c`, `mtxvec.c`,
//! `mtx44.c`). The paired-single ones follow their assembly lane by lane.

use gekko_fp::{self as fp, Ps};

use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::pairs::{load1, load2, moved, moved_paired, splat, store_paired};
use crate::quat::Quat;
use crate::vec::Vec3;

/// A coordinate axis: `'x'`, `'y'` and `'z'` to `MTXRotRad`, 1, 2 and 4 to `lbVector_Rotate`.
/// Every caller passes one of those, so the original's do-nothing path for other values
/// never runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// `Mtx` in the original: three rows of four floats, the fourth column a translation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mtx34 {
    pub m: [[f32; 4]; 3],
}

impl Mtx34 {
    #[doc(alias = "PSMTXIdentity")]
    pub const IDENTITY: Mtx34 = Mtx34 {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    /// A translation, `PSMTXTrans`.
    #[doc(alias = "PSMTXTrans")]
    pub fn translation(x: f32, y: f32, z: f32) -> Mtx34 {
        let mut t = Mtx34::IDENTITY;
        t.m[0][3] = moved(x);
        t.m[1][3] = moved(y);
        t.m[2][3] = moved(z);
        t
    }

    /// A scale, `PSMTXScale`.
    #[doc(alias = "PSMTXScale")]
    pub fn scale(x: f32, y: f32, z: f32) -> Mtx34 {
        let mut s = Mtx34::default();
        s.m[0][0] = moved(x);
        s.m[1][1] = moved(y);
        s.m[2][2] = moved(z);
        s
    }

    /// Floats `i` and `i + 1` of the matrix as `psq_l` loads them.
    fn pair(&self, i: usize) -> Ps {
        let m = self.m.as_flattened();
        load2(m[i], m[i + 1])
    }

    fn one(&self, i: usize) -> Ps {
        load1(self.m.as_flattened()[i])
    }

    /// Stores both lanes at floats `i` and `i + 1`, which may straddle two rows.
    fn put(&mut self, i: usize, lanes: Ps) {
        let m = self.m.as_flattened_mut();
        m[i] = store_paired(lanes.ps0);
        m[i + 1] = store_paired(lanes.ps1);
    }

    fn put1(&mut self, i: usize, lanes: Ps) {
        self.m.as_flattened_mut()[i] = store_paired(lanes.ps0);
    }

    /// A copy as `PSMTXCopy` makes it, through registers.
    #[doc(alias = "PSMTXCopy")]
    pub fn copied(&self) -> Mtx34 {
        let mut out = Mtx34::default();
        for i in (0..12).step_by(2) {
            out.put(i, self.pair(i));
        }
        out
    }

    /// `self · b`, `PSMTXConcat`.
    #[doc(alias = "PSMTXConcat")]
    pub fn concat(&self, b: &Mtx34) -> Mtx34 {
        use fp::{ps_madds0 as madds0, ps_madds1 as madds1, ps_muls0 as muls0};
        let a = self;
        let unit01 = Ps::new(0.0, 1.0);
        let (a0, a2, a4, a6, a8, a10) = (
            a.pair(0),
            a.pair(2),
            a.pair(4),
            a.pair(6),
            a.pair(8),
            a.pair(10),
        );
        let (b0, b2, b4, b6, b8, b10) = (
            b.pair(0),
            b.pair(2),
            b.pair(4),
            b.pair(6),
            b.pair(8),
            b.pair(10),
        );
        let mut out = Mtx34::default();
        let r00 = madds0(b8, a2, madds1(b4, a0, muls0(b0, a0)));
        let r02 = madds1(unit01, a2, madds0(b10, a2, madds1(b6, a0, muls0(b2, a0))));
        let r10 = madds0(b8, a6, madds1(b4, a4, muls0(b0, a4)));
        let r12 = madds1(unit01, a6, madds0(b10, a6, madds1(b6, a4, muls0(b2, a4))));
        let r20 = madds0(b8, a10, madds1(b4, a8, muls0(b0, a8)));
        let r22 = madds1(unit01, a10, madds0(b10, a10, madds1(b6, a8, muls0(b2, a8))));
        for (i, r) in [(0, r00), (2, r02), (4, r10), (6, r12), (8, r20), (10, r22)] {
            out.put(i, r);
        }
        out
    }

    /// The transpose of the rotation part, translation zero, `PSMTXTranspose`.
    #[doc(alias = "PSMTXTranspose")]
    pub fn transpose(&self) -> Mtx34 {
        let m = &self.m;
        let mut out = Mtx34::default();
        for (r, row) in out.m.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().take(3).enumerate() {
                *v = moved_paired(m[c][r]);
            }
        }
        // The last row's diagonal goes through `stfs`, the rest through `psq_st`.
        out.m[2][2] = moved(m[2][2]);
        out
    }

    /// The inverse, or `None` when the rotation part's determinant is zero, `PSMTXInverse`.
    #[doc(alias = "PSMTXInverse")]
    pub fn inverse(&self) -> Option<Mtx34> {
        use fp::{
            ps_add, ps_madd, ps_merge00, ps_merge10, ps_merge11, ps_msub, ps_mul, ps_muls0,
            ps_nmadd, ps_nmsub, ps_sub,
        };
        let f0 = self.one(0);
        let f1 = self.pair(1);
        let f2 = self.one(4);
        let f6 = ps_merge10(f1, f0);
        let f3 = self.pair(5);
        let f4 = self.one(8);
        let f7 = ps_merge10(f3, f2);
        let f5 = self.pair(9);
        let f11 = ps_mul(f3, f6);
        let f13 = ps_mul(f5, f7);
        let f8 = ps_merge10(f5, f4);
        let f11 = ps_msub(f1, f7, f11);
        let f12 = ps_mul(f1, f8);
        let f13 = ps_msub(f3, f8, f13);
        let f10 = ps_mul(f3, f4);
        let f12 = ps_msub(f5, f6, f12);
        let f9 = ps_mul(f0, f5);
        let f8 = ps_mul(f1, f2);
        let zero = ps_sub(f6, f6);
        let f10 = ps_msub(f2, f5, f10);
        let det = ps_mul(f0, f13);
        let f9 = ps_msub(f1, f4, f9);
        let det = ps_madd(f2, f12, det);
        let f8 = ps_msub(f0, f3, f8);
        let det = ps_madd(f4, f11, det);
        if det.ps0 == zero.ps0 {
            return None;
        }
        // 1/det: the `fres` estimate, refined by one Newton step.
        let e = Ps::splat(fp::fres(det.ps0));
        let inv = ps_nmsub(det, ps_mul(e, e), ps_add(e, e));
        let (t0, t1, t2) = (
            splat(self.m[0][3]),
            splat(self.m[1][3]),
            splat(self.m[2][3]),
        );
        let f13 = ps_muls0(f13, inv);
        let f12 = ps_muls0(f12, inv);
        let f11 = ps_muls0(f11, inv);
        let f10 = ps_muls0(f10, inv);
        let f9 = ps_muls0(f9, inv);
        let f8 = ps_muls0(f8, inv);
        let mut out = Mtx34::default();
        out.put(0, ps_merge00(f13, f12));
        out.put(4, ps_merge11(f13, f12));
        let col3 = ps_nmadd(f11, t2, ps_madd(f12, t1, ps_mul(f13, t0)));
        out.put1(8, f10);
        out.put1(9, f9);
        out.put1(10, f8);
        out.put(2, ps_merge00(f11, col3));
        out.put(6, ps_merge11(f11, col3));
        let row2 = ps_nmadd(f8, t2, ps_madd(f9, t1, ps_mul(f10, t0)));
        out.put1(11, row2);
        Some(out)
    }

    /// `self · v` with translation, `PSMTXMultVec`.
    #[doc(alias = "PSMTXMultVec")]
    pub fn transform(&self, v: Vec3) -> Vec3 {
        let (vxy, vz1) = (load2(v.x, v.y), load1(v.z));
        let row = |r: usize| {
            let s = fp::ps_madd(self.pair(r * 4 + 2), vz1, fp::ps_mul(self.pair(r * 4), vxy));
            store_paired(fp::ps_sum0(s, s, s).ps0)
        };
        Vec3::new(row(0), row(1), row(2))
    }

    /// `self · v` without translation, `PSMTXMultVecSR`.
    #[doc(alias = "PSMTXMultVecSR")]
    pub fn rotate(&self, v: Vec3) -> Vec3 {
        let (vxy, vz1) = (load2(v.x, v.y), load1(v.z));
        let row = |r: usize| {
            let p = fp::ps_mul(self.pair(r * 4), vxy);
            let p = fp::ps_sum0(p, p, p);
            store_paired(fp::ps_madd(self.pair(r * 4 + 2), vz1, p).ps0)
        };
        Vec3::new(row(0), row(1), row(2))
    }

    /// A rotation about `axis` given its sine and cosine, `PSMTXRotTrig`.
    #[doc(alias = "PSMTXRotTrig")]
    pub fn rotation_trig(axis: Axis, sin: f32, cos: f32) -> Mtx34 {
        let (zero, one) = (splat(0.0), splat(1.0));
        let (s, c) = (splat(sin), splat(cos));
        let neg_s = fp::ps_neg(s);
        let mut m = Mtx34::default();
        match axis {
            Axis::X => {
                m.put1(0, one);
                m.put(1, zero);
                m.put(3, zero);
                m.put(7, zero);
                m.put1(11, zero);
                m.put(9, fp::ps_merge00(s, c));
                m.put(5, fp::ps_merge00(c, neg_s));
            }
            Axis::Y => {
                m.put(6, zero);
                m.put(0, fp::ps_merge00(c, zero));
                m.put(10, fp::ps_merge00(c, zero));
                m.put(4, fp::ps_merge00(zero, one));
                m.put(2, fp::ps_merge00(s, zero));
                m.put(8, fp::ps_merge00(neg_s, zero));
            }
            Axis::Z => {
                m.put(2, zero);
                m.put(6, zero);
                m.put(8, zero);
                m.put(4, fp::ps_merge00(s, c));
                m.put(0, fp::ps_merge00(c, neg_s));
                m.put(10, fp::ps_merge00(one, zero));
            }
        }
        m
    }

    /// A rotation by `rad` radians about `axis`, `MTXRotRad`.
    #[doc(alias = "MTXRotRad")]
    pub fn rotation(axis: Axis, rad: f32) -> Mtx34 {
        Mtx34::rotation_trig(axis, math::sin(rad), math::cos(rad))
    }

    /// A rotation by `rad` radians about `axis`, which needn't be a unit vector,
    /// `PSMTXRotAxisRad`.
    #[doc(alias = "PSMTXRotAxisRad")]
    pub fn rotation_about(axis: Vec3, rad: f32) -> Mtx34 {
        use fp::{ps_merge00, ps_muls0, ps_muls1, ps_neg, ps_sum0, ps_sum1};
        let zero = splat(0.0);
        let s = reg(math::sin(rad));
        let c = reg(math::cos(rad));
        let t = Ps::splat(fp::fsubs(1.0, c));
        let n = axis.sdk_normalize();
        let nxy = load2(n.x, n.y);
        let nz = splat(n.z);
        let cc = ps_merge00(Ps::splat(c), Ps::splat(c));
        let f4 = ps_muls0(nxy, t);
        let f5 = ps_muls0(nz, t);
        let f3 = ps_muls1(f4, nxy);
        let f2 = ps_muls0(f4, nxy);
        let sxy = ps_muls0(nxy, Ps::splat(s));
        let f4 = ps_muls0(f4, nz);
        let f6 = Ps::splat(fp::fnmsubs(nz.ps0, s, f3.ps0));
        let f7 = Ps::splat(fp::fmadds(nz.ps0, s, f3.ps0));
        let f9 = ps_neg(sxy);
        let f8 = ps_sum0(f4, zero, sxy);
        let f2 = ps_sum0(f2, f6, cc);
        let f3 = ps_sum1(cc, f7, f3);
        let f6 = ps_sum0(f9, zero, f4);
        let f9 = ps_sum0(f4, f4, f9);
        let f5 = ps_muls0(f5, nz);
        let f4 = ps_sum1(sxy, f9, f4);
        let f5 = ps_sum0(f5, zero, cc);
        let mut m = Mtx34::default();
        for (i, r) in [(2, f8), (0, f2), (4, f3), (6, f6), (8, f4), (10, f5)] {
            m.put(i, r);
        }
        m
    }

    /// The rotation a quaternion describes, `PSMTXQuat` (and `HSD_MtxQuat`, which calls it).
    /// The quaternion needn't be a unit one: its squared norm, through an `fres` estimate,
    /// divides it out.
    #[doc(alias = "PSMTXQuat")]
    #[doc(alias = "HSD_MtxQuat")]
    pub fn from_quat(q: Quat) -> Mtx34 {
        use fp::{
            ps_madd, ps_madds0, ps_merge00, ps_merge01, ps_merge10, ps_msub, ps_mul, ps_muls1,
            ps_nmsub, ps_sum0, ps_sum1,
        };
        let one = splat(1.0);
        let xy = load2(q.x, q.y);
        let zw = load2(q.z, q.w);
        let zero = Ps::splat(fp::fsubs(one.ps0, one.ps0));
        let two = Ps::splat(fp::fadds(one.ps0, one.ps0));
        let f6 = ps_mul(xy, xy);
        let yx = ps_merge10(xy, xy);
        let f8 = ps_madd(zw, zw, f6);
        let f7 = ps_mul(zw, zw);
        let norm = ps_sum0(f8, f8, f8);
        let f10 = ps_muls1(yx, zw);
        let e = Ps::splat(fp::fres(norm.ps0));
        let f8 = ps_sum1(f7, f8, f6);
        let f3 = ps_nmsub(norm, e, two);
        let f7 = ps_muls1(zw, zw);
        let f3 = ps_mul(e, f3);
        let f6 = ps_sum0(f6, f6, f6);
        let f3 = Ps::splat(fp::fmuls(f3.ps0, two.ps0));
        let f11 = ps_madd(xy, yx, f7);
        let f7 = ps_msub(xy, yx, f7);
        let mut m = Mtx34::default();
        m.put1(3, zero);
        let f6 = ps_nmsub(f6, f3, one);
        let f8 = ps_nmsub(f8, f3, one);
        m.put1(11, zero);
        let f11 = ps_mul(f11, f3);
        let f7 = ps_mul(f7, f3);
        m.put1(10, f6);
        let f9 = ps_madds0(xy, zw, f10);
        let f5 = ps_merge00(f11, f8);
        let f10 = ps_nmsub(f10, two, f9);
        let f4 = ps_merge10(f8, f7);
        m.put(4, f5);
        let f9 = ps_mul(f9, f3);
        let f10 = ps_mul(f10, f3);
        m.put(0, f4);
        m.put1(2, f9);
        m.put(6, ps_merge10(f10, zero));
        m.put(8, ps_merge01(f10, f9));
        m
    }

    /// The view matrix of a camera at `eye` looking at `target`, `C_MTXLookAt`.
    #[doc(alias = "C_MTXLookAt")]
    pub fn look_at(eye: Vec3, up: Vec3, target: Vec3) -> Mtx34 {
        let look = Vec3::new(
            mem(fp::fsubs(reg(eye.x), reg(target.x))),
            mem(fp::fsubs(reg(eye.y), reg(target.y))),
            mem(fp::fsubs(reg(eye.z), reg(target.z))),
        )
        .sdk_normalize();
        let right = up.sdk_cross(look).sdk_normalize();
        let up = look.sdk_cross(right);
        let row = |v: Vec3| {
            let d = fp::fadds(
                fp::fmuls(reg(eye.z), reg(v.z)),
                fp::fadds(
                    fp::fmuls(reg(eye.x), reg(v.x)),
                    fp::fmuls(reg(eye.y), reg(v.y)),
                ),
            );
            [copy(v.x), copy(v.y), copy(v.z), mem(fp::fneg(d))]
        };
        Mtx34 {
            m: [row(right), row(up), row(look)],
        }
    }

    /// A texture projection for a light's frustum, `MTXLightFrustum`.
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "MTXLightFrustum")]
    pub fn light_frustum(
        t: f32,
        b: f32,
        l: f32,
        r: f32,
        n: f32,
        scale_s: f32,
        scale_t: f32,
        trans_s: f32,
        trans_t: f32,
    ) -> Mtx34 {
        let (t, b, l, r, n) = (reg(t), reg(b), reg(l), reg(r), reg(n));
        let (ss, st, ts, tt) = (reg(scale_s), reg(scale_t), reg(trans_s), reg(trans_t));
        let mut m = Mtx34::default();
        let tmp = fp::fdivs(1.0, fp::fsubs(r, l));
        m.m[0][0] = mem(fp::fmuls(ss, fp::fmuls(fp::fmuls(2.0, n), tmp)));
        m.m[0][2] = mem(fp::fsubs(
            fp::fmuls(ss, fp::fmuls(tmp, fp::fadds(r, l))),
            ts,
        ));
        let tmp = fp::fdivs(1.0, fp::fsubs(t, b));
        m.m[1][1] = mem(fp::fmuls(st, fp::fmuls(fp::fmuls(2.0, n), tmp)));
        m.m[1][2] = mem(fp::fsubs(
            fp::fmuls(st, fp::fmuls(tmp, fp::fadds(t, b))),
            tt,
        ));
        m.m[2][2] = -1.0;
        m
    }

    /// A texture projection for a light's perspective, `MTXLightPerspective`: `fov_y` in
    /// degrees.
    #[doc(alias = "MTXLightPerspective")]
    pub fn light_perspective(
        fov_y: f32,
        aspect: f32,
        scale_s: f32,
        scale_t: f32,
        trans_s: f32,
        trans_t: f32,
    ) -> Mtx34 {
        let cot = cotangent_of_half(fov_y);
        let mut m = Mtx34::default();
        m.m[0][0] = mem(fp::fmuls(reg(scale_s), fp::fdivs(cot, reg(aspect))));
        m.m[0][2] = mem(fp::fneg(reg(trans_s)));
        m.m[1][1] = mem(fp::fmuls(cot, reg(scale_t)));
        m.m[1][2] = mem(fp::fneg(reg(trans_t)));
        m.m[2][2] = -1.0;
        m
    }

    /// A texture projection for a light's orthographic view, `MTXLightOrtho`.
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "MTXLightOrtho")]
    pub fn light_ortho(
        t: f32,
        b: f32,
        l: f32,
        r: f32,
        scale_s: f32,
        scale_t: f32,
        trans_s: f32,
        trans_t: f32,
    ) -> Mtx34 {
        let (t, b, l, r) = (reg(t), reg(b), reg(l), reg(r));
        let (ss, st, ts, tt) = (reg(scale_s), reg(scale_t), reg(trans_s), reg(trans_t));
        let mut m = Mtx34::default();
        let tmp = fp::fdivs(1.0, fp::fsubs(r, l));
        m.m[0][0] = mem(fp::fmuls(fp::fmuls(2.0, tmp), ss));
        m.m[0][3] = mem(fp::fadds(
            ts,
            fp::fmuls(ss, fp::fmuls(tmp, fp::fneg(fp::fadds(r, l)))),
        ));
        let tmp = fp::fdivs(1.0, fp::fsubs(t, b));
        m.m[1][1] = mem(fp::fmuls(fp::fmuls(2.0, tmp), st));
        m.m[1][3] = mem(fp::fadds(
            tt,
            fp::fmuls(st, fp::fmuls(tmp, fp::fneg(fp::fadds(t, b)))),
        ));
        m.m[2][3] = 1.0;
        m
    }
}

/// `1 / tan(fov_y / 2)` for a field of view in degrees, as `MTXPerspective` and
/// `MTXLightPerspective` compute it.
fn cotangent_of_half(fov_y: f32) -> f64 {
    let half = fp::fmuls(0.5, reg(fov_y));
    let angle = fp::fmuls(f64::from(DEG_TO_RAD), half);
    fp::fdivs(1.0, reg(math::tan(mem(angle))))
}

/// π / 180 as the SDK's `MTXDegToRad` has it.
const DEG_TO_RAD: f32 = 0.017_453_292;

/// `Mtx44` in the original: a projection matrix, four rows of four floats.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mtx44 {
    pub m: [[f32; 4]; 4],
}

impl Mtx44 {
    /// A perspective projection of the frustum with these clipping planes, `MTXFrustum`.
    #[doc(alias = "MTXFrustum")]
    pub fn frustum(t: f32, b: f32, l: f32, r: f32, n: f32, f: f32) -> Mtx44 {
        let (t, b, l, r, n, f) = (reg(t), reg(b), reg(l), reg(r), reg(n), reg(f));
        let mut m = Mtx44::default();
        let tmp = fp::fdivs(1.0, fp::fsubs(r, l));
        m.m[0][0] = mem(fp::fmuls(fp::fmuls(2.0, n), tmp));
        m.m[0][2] = mem(fp::fmuls(tmp, fp::fadds(r, l)));
        let tmp = fp::fdivs(1.0, fp::fsubs(t, b));
        m.m[1][1] = mem(fp::fmuls(fp::fmuls(2.0, n), tmp));
        m.m[1][2] = mem(fp::fmuls(tmp, fp::fadds(t, b)));
        let tmp = fp::fdivs(1.0, fp::fsubs(f, n));
        m.m[2][2] = mem(fp::fmuls(fp::fneg(n), tmp));
        m.m[2][3] = mem(fp::fmuls(tmp, fp::fneg(fp::fmuls(f, n))));
        m.m[3][2] = -1.0;
        m
    }

    /// A perspective projection: `fov_y` in degrees, `MTXPerspective`.
    #[doc(alias = "MTXPerspective")]
    pub fn perspective(fov_y: f32, aspect: f32, n: f32, f: f32) -> Mtx44 {
        let cot = cotangent_of_half(fov_y);
        let (n, f) = (reg(n), reg(f));
        let mut m = Mtx44::default();
        m.m[0][0] = mem(fp::fdivs(cot, reg(aspect)));
        m.m[1][1] = mem(cot);
        let tmp = fp::fdivs(1.0, fp::fsubs(f, n));
        m.m[2][2] = mem(fp::fmuls(fp::fneg(n), tmp));
        m.m[2][3] = mem(fp::fmuls(tmp, fp::fneg(fp::fmuls(f, n))));
        m.m[3][2] = -1.0;
        m
    }

    /// An orthographic projection, `MTXOrtho`.
    #[doc(alias = "MTXOrtho")]
    pub fn ortho(t: f32, b: f32, l: f32, r: f32, n: f32, f: f32) -> Mtx44 {
        let (t, b, l, r, n, f) = (reg(t), reg(b), reg(l), reg(r), reg(n), reg(f));
        let mut m = Mtx44::default();
        let tmp = fp::fdivs(1.0, fp::fsubs(r, l));
        m.m[0][0] = mem(fp::fmuls(2.0, tmp));
        m.m[0][3] = mem(fp::fmuls(tmp, fp::fneg(fp::fadds(r, l))));
        let tmp = fp::fdivs(1.0, fp::fsubs(t, b));
        m.m[1][1] = mem(fp::fmuls(2.0, tmp));
        m.m[1][3] = mem(fp::fmuls(tmp, fp::fneg(fp::fadds(t, b))));
        let tmp = fp::fdivs(1.0, fp::fsubs(f, n));
        m.m[2][2] = mem(fp::fmuls(-1.0, tmp));
        m.m[2][3] = mem(fp::fmuls(fp::fneg(f), tmp));
        m.m[3][3] = 1.0;
        m
    }
}
