//! HAL's own matrix routines (`sysdolphin/baselib/mtx.c`), which compute in scalar code where
//! the SDK's use paired singles: inverses, decomposition, and building a joint's matrix from
//! its scale, rotation and translation.

use gekko_fp as fp;

use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::mtx::Mtx34;
use crate::quat::{Quat, column_length};
use crate::vec::Vec3;

/// `FLT_MIN`, the threshold for a zero length.
const FLT_MIN: f32 = f32::MIN_POSITIVE;

/// `fabsf_bitwise`: the float stored, its sign bit cleared, and loaded again.
fn abs_bitwise(x: f64) -> f64 {
    fp::lfs(fp::stfs(x) & 0x7FFF_FFFF)
}

/// Whether a determinant is too small to invert by, below 1e-10 in magnitude.
fn singular(det: f64) -> bool {
    abs_bitwise(det) < f64::from(1.0e-10_f32)
}

/// The cofactors of the rotation part over the determinant, transposed: the rotation part of
/// the inverse, `[r][c]` as `HSD_MtxInverse` writes it, in registers.
fn adjugate_over(m: &Mtx34, det: f64) -> [[f64; 3]; 3] {
    let inv = fp::fdivs(1.0, det);
    let g = |r: usize, c: usize| reg(m.m[r][c]);
    // Cofactors alternate between `a·c - b` and `-(a·c - b)`.
    let plus = |a, c, b| fp::fmuls(inv, fp::fmsubs(a, c, b));
    let minus = |a, c, b| fp::fmuls(inv, fp::fnmsubs(a, c, b));
    [
        [
            plus(g(1, 1), g(2, 2), fp::fmuls(g(2, 1), g(1, 2))),
            minus(g(0, 1), g(2, 2), fp::fmuls(g(2, 1), g(0, 2))),
            plus(g(0, 1), g(1, 2), fp::fmuls(g(1, 1), g(0, 2))),
        ],
        [
            minus(g(1, 0), g(2, 2), fp::fmuls(g(2, 0), g(1, 2))),
            plus(g(0, 0), g(2, 2), fp::fmuls(g(2, 0), g(0, 2))),
            minus(g(0, 0), g(1, 2), fp::fmuls(g(1, 0), g(0, 2))),
        ],
        [
            plus(g(1, 0), g(2, 1), fp::fmuls(g(2, 0), g(1, 1))),
            minus(g(0, 0), g(2, 1), fp::fmuls(g(2, 0), g(0, 1))),
            plus(g(0, 0), g(1, 1), fp::fmuls(g(1, 0), g(0, 1))),
        ],
    ]
}

/// `atan2f(y, x)`, with ±π/2 when `x` is about zero (`calcVal` in `HSD_MtxGetRotation`).
fn angle_of(x: f32, y: f32) -> f32 {
    if abs_bitwise(reg(x)) <= f64::from(FLT_MIN) {
        let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
        mem(if reg(y) >= 0.0 {
            half_pi
        } else {
            fp::fneg(half_pi)
        })
    } else {
        math::atan2(y, x)
    }
}

impl Mtx34 {
    /// The rotation part's determinant, `HSD_CalcDeterminantMatrix3x4`, in a register.
    fn hsd_determinant(&self) -> f64 {
        let m = |r: usize, c: usize| reg(self.m[r][c]);
        let sum = fp::fmuls(m(2, 0), fp::fmuls(m(0, 1), m(1, 2)));
        let sum = fp::fmadds(m(2, 2), fp::fmuls(m(0, 0), m(1, 1)), sum);
        let sum = fp::fmadds(m(2, 1), fp::fmuls(m(0, 2), m(1, 0)), sum);
        let sum = fp::fnmsubs(m(0, 2), fp::fmuls(m(2, 0), m(1, 1)), sum);
        let sum = fp::fnmsubs(m(2, 2), fp::fmuls(m(1, 0), m(0, 1)), sum);
        fp::fnmsubs(m(1, 2), fp::fmuls(m(0, 0), m(2, 1)), sum)
    }

    /// The inverse as HAL computes it, `HSD_MtxInverse`: by cofactors, scaled by `1 / det`.
    /// A matrix whose determinant is below 1e-10 in magnitude gives the identity.
    #[doc(alias = "HSD_MtxInverse")]
    pub fn hsd_inverse(&self) -> Mtx34 {
        let det = self.hsd_determinant();
        if singular(det) {
            return Mtx34::IDENTITY;
        }
        let mut out = Mtx34::default();
        let src = self.m;
        out.write_hsd_inverse(self, det, |_, r| src[r][3]);
        out
    }

    /// Inverts in place, as `HSD_MtxInverse(m, m)` does: the rotation comes from a copy made
    /// with `PSMTXCopy` (which flushes subnormals), and each row's translation reads the
    /// translations of the rows already inverted.
    pub fn hsd_invert(&mut self) {
        let det = self.hsd_determinant();
        if singular(det) {
            *self = Mtx34::IDENTITY;
            return;
        }
        let copy = self.copied();
        self.write_hsd_inverse(&copy, det, |out, r| out.m[r][3]);
    }

    /// Writes the inverse of `m` into `self`: the translation of source row `r` is
    /// `src3(self, r)`. The translation reads the rotation back from the stored result.
    fn write_hsd_inverse(&mut self, m: &Mtx34, det: f64, src3: impl Fn(&Mtx34, usize) -> f32) {
        let rot = adjugate_over(m, det);
        for (r, row) in rot.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                self.m[r][c] = mem(*v);
            }
        }
        for r in 0..3 {
            let d = |c: usize| reg(self.m[r][c]);
            let s = |i: usize| reg(src3(self, i));
            let partial = fp::fmsubs(fp::fneg(d(0)), s(0), fp::fmuls(d(1), s(1)));
            let t = fp::fnmsubs(d(2), s(2), partial);
            self.m[r][3] = mem(t);
        }
    }

    /// `self⁻¹ · src`, `HSD_MtxInverseConcat`: `src` as `PSMTXCopy` copies it when `self` is
    /// about singular. No caller passes an input as the output.
    #[doc(alias = "HSD_MtxInverseConcat")]
    pub fn hsd_inverse_concat(&self, src: &Mtx34) -> Mtx34 {
        let det = self.hsd_determinant();
        if singular(det) {
            return src.copied();
        }
        let g = |r: usize, c: usize| reg(self.m[r][c]);
        let s = |r: usize, c: usize| reg(src.m[r][c]);
        let mut out = Mtx34::default();
        for (r, &[a, b, c]) in adjugate_over(self, det).iter().enumerate() {
            let shift = fp::fnmsubs(
                c,
                g(2, 3),
                fp::fmsubs(fp::fneg(a), g(0, 3), fp::fmuls(b, g(1, 3))),
            );
            for col in 0..4 {
                let v = fp::fmadds(
                    c,
                    s(2, col),
                    fp::fmadds(a, s(0, col), fp::fmuls(b, s(1, col))),
                );
                out.m[r][col] = mem(if col == 3 { fp::fadds(shift, v) } else { v });
            }
        }
        out
    }

    /// The inverse's rotation part transposed, translation zero, `HSD_MtxInverseTranspose`:
    /// the matrix that transforms normals. A copy through `PSMTXCopy` when the rotation part
    /// is about singular. No caller passes the input as the output.
    #[doc(alias = "HSD_MtxInverseTranspose")]
    pub fn hsd_inverse_transpose(&self) -> Mtx34 {
        let det = self.hsd_determinant();
        if singular(det) {
            return self.copied();
        }
        let mut out = Mtx34::default();
        for (r, row) in adjugate_over(self, det).iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                out.m[c][r] = mem(*v);
            }
        }
        out
    }

    /// Euler angles (x, then y, then z) of the rotation of a matrix whose columns may be
    /// scaled, `HSD_MtxGetRotation`; zero when a column has no length.
    // NaN lengths take the original's branches, which the negated comparisons spell out.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    #[doc(alias = "HSD_MtxGetRotation")]
    pub fn rotation_angles(&self) -> Vec3 {
        let m = &self.m;
        let min = f64::from(FLT_MIN);
        let len0 = column_length(self, 0);
        if !(reg(len0) < min) {
            let len1 = column_length(self, 1);
            if !(reg(len1) < min) {
                let len2 = column_length(self, 2);
                if !(reg(len2) < min) {
                    let sin_y = fp::fdivs(fp::fneg(reg(m[2][0])), reg(len0));
                    let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
                    let y = mem(if sin_y >= 1.0 {
                        half_pi
                    } else if sin_y <= -1.0 {
                        fp::frsp(fp::fdiv(fp::fneg(std::f64::consts::PI), 2.0))
                    } else {
                        reg(math::asin(mem(sin_y)))
                    });
                    if reg(math::cos(y)) >= min {
                        let cos_x = mem(fp::fdivs(reg(m[2][2]), reg(len2)));
                        let sin_x = mem(fp::fdivs(reg(m[2][1]), reg(len1)));
                        return Vec3::new(angle_of(cos_x, sin_x), y, angle_of(m[0][0], m[1][0]));
                    }
                    return Vec3::new(angle_of(m[1][1], m[0][1]), y, 0.0);
                }
            }
        }
        Vec3::ZERO
    }

    /// The translation column, `HSD_MtxGetTranslate`.
    #[doc(alias = "HSD_MtxGetTranslate")]
    pub fn translation_part(&self) -> Vec3 {
        Vec3::new(copy(self.m[0][3]), copy(self.m[1][3]), copy(self.m[2][3]))
    }

    /// Each axis's scale, by Gram-Schmidt on the columns, `HSD_MtxGetScale`; all negative
    /// when the matrix mirrors.
    #[doc(alias = "HSD_MtxGetScale")]
    pub fn scale_part(&self) -> Vec3 {
        let col = |c: usize| Vec3::new(copy(self.m[0][c]), copy(self.m[1][c]), copy(self.m[2][c]));
        // `b` less its projection on the unit vector `u`.
        let reject = |b: Vec3, u: Vec3| b.sdk_sub(u.sdk_scale(u.sdk_dot(b)));
        let v1 = col(0);
        let x = v1.sdk_length();
        let v1 = v1.sdk_normalize();
        let v2 = reject(col(1), v1);
        let y = v2.sdk_length();
        let v2 = v2.sdk_normalize();
        let v3 = reject(reject(col(2), v2), v1);
        let z = v3.sdk_length();
        let v3 = v3.sdk_normalize();
        let v4 = v2.sdk_cross(v3);
        let mut s = Vec3::new(x, y, z);
        if reg(v1.sdk_dot(v4)) < 0.0 {
            let flip = |v: f32| mem(fp::frsp(fp::fmul(reg(v), fp::fneg(1.0))));
            s = Vec3::new(flip(s.x), flip(s.y), flip(s.z));
        }
        s
    }

    /// The rotation by Euler angles about x, then y, then z, `HSD_MkRotationMtx`.
    #[doc(alias = "HSD_MkRotationMtx")]
    pub fn from_euler(angles: Vec3) -> Mtx34 {
        let (sx, cx) = (reg(math::sin(angles.x)), reg(math::cos(angles.x)));
        let (sy, cy) = (reg(math::sin(angles.y)), reg(math::cos(angles.y)));
        let (sz, cz) = (reg(math::sin(angles.z)), reg(math::cos(angles.z)));
        let sxsy = fp::fmuls(sx, sy);
        let cxsy = fp::fmuls(cx, sy);
        Mtx34 {
            m: [
                [
                    mem(fp::fmuls(cy, cz)),
                    mem(fp::fmsubs(cz, sxsy, fp::fmuls(cx, sz))),
                    mem(fp::fmadds(cz, cxsy, fp::fmuls(sx, sz))),
                    0.0,
                ],
                [
                    mem(fp::fmuls(cy, sz)),
                    mem(fp::fmadds(sz, sxsy, fp::fmuls(cx, cz))),
                    mem(fp::fmsubs(sz, cxsy, fp::fmuls(sx, cz))),
                    0.0,
                ],
                [
                    mem(fp::fneg(sy)),
                    mem(fp::fmuls(sx, cy)),
                    mem(fp::fmuls(cx, cy)),
                    0.0,
                ],
            ],
        }
    }

    /// A joint's matrix from its scale, Euler rotation and translation, `HSD_MtxSRT`. With
    /// `parent_scale`, each row's scale is divided by the parent's on that axis, so the
    /// parent's scale doesn't shear the child.
    #[doc(alias = "HSD_MtxSRT")]
    pub fn srt(scale: Vec3, rotate: Vec3, translate: Vec3, parent_scale: Option<Vec3>) -> Mtx34 {
        let (sx, cx) = (reg(math::sin(rotate.x)), reg(math::cos(rotate.x)));
        let (sy, cy) = (reg(math::sin(rotate.y)), reg(math::cos(rotate.y)));
        let (sz, cz) = (reg(math::sin(rotate.z)), reg(math::cos(rotate.z)));
        // The scale each row uses: `s[r]` = (x, y, z) for row r.
        let base = [reg(scale.x), reg(scale.y), reg(scale.z)];
        let mut s = [base; 3];
        if let Some(p) = parent_scale {
            let p = [reg(p.x), reg(p.y), reg(p.z)];
            let inv = p.map(|v| fp::frsp(fp::fdiv(1.0, v)));
            s[0][1] = fp::fmuls(s[0][1], fp::fmuls(p[1], inv[0]));
            s[0][2] = fp::fmuls(s[0][2], fp::fmuls(p[2], inv[0]));
            s[1][0] = fp::fmuls(s[1][0], fp::fmuls(p[0], inv[1]));
            s[1][2] = fp::fmuls(s[1][2], fp::fmuls(p[2], inv[1]));
            s[2][0] = fp::fmuls(s[2][0], fp::fmuls(p[0], inv[2]));
            s[2][1] = fp::fmuls(s[2][1], fp::fmuls(p[1], inv[2]));
        }
        Mtx34 {
            m: [
                [
                    mem(fp::fmuls(cz, fp::fmuls(s[0][0], cy))),
                    mem(fp::fmuls(
                        s[0][1],
                        fp::fmsubs(cz, fp::fmuls(sx, sy), fp::fmuls(cx, sz)),
                    )),
                    mem(fp::fmuls(
                        s[0][2],
                        fp::fmadds(cz, fp::fmuls(cx, sy), fp::fmuls(sx, sz)),
                    )),
                    copy(translate.x),
                ],
                [
                    mem(fp::fmuls(sz, fp::fmuls(s[1][0], cy))),
                    mem(fp::fmuls(
                        s[1][1],
                        fp::fmadds(sz, fp::fmuls(sx, sy), fp::fmuls(cx, cz)),
                    )),
                    mem(fp::fmuls(
                        s[1][2],
                        fp::fmsubs(sz, fp::fmuls(cx, sy), fp::fmuls(sx, cz)),
                    )),
                    copy(translate.y),
                ],
                [
                    mem(fp::fmuls(fp::fneg(s[2][0]), sy)),
                    mem(fp::fmuls(cy, fp::fmuls(s[2][1], sx))),
                    mem(fp::fmuls(cy, fp::fmuls(s[2][2], cx))),
                    copy(translate.z),
                ],
            ],
        }
    }

    /// A joint's matrix from its scale, quaternion rotation and translation,
    /// `HSD_MtxSRTQuat`, by concatenating each step; `parent_scale` as in [`Mtx34::srt`].
    #[doc(alias = "HSD_MtxSRTQuat")]
    pub fn srt_quat(
        scale: Vec3,
        rotate: Quat,
        translate: Vec3,
        parent_scale: Option<Vec3>,
    ) -> Mtx34 {
        let mut m = Mtx34::scale(scale.x, scale.y, scale.z);
        if let Some(p) = parent_scale {
            m = Mtx34::scale(p.x, p.y, p.z).concat(&m);
        }
        m = Mtx34::from_quat(rotate).concat(&m);
        if let Some(p) = parent_scale {
            let inv = |v: f32| mem(fp::frsp(fp::fdiv(1.0, reg(v))));
            m = Mtx34::scale(inv(p.x), inv(p.y), inv(p.z)).concat(&m);
        }
        Mtx34::translation(translate.x, translate.y, translate.z).concat(&m)
    }

    /// `s·self + b`, element by element, `HSD_MtxScaledAdd`.
    #[doc(alias = "HSD_MtxScaledAdd")]
    pub fn scaled_add(&self, b: &Mtx34, s: f32) -> Mtx34 {
        let s = reg(s);
        let mut out = Mtx34::default();
        for r in 0..3 {
            for c in 0..4 {
                out.m[r][c] = mem(fp::fmadds(s, reg(self.m[r][c]), reg(b.m[r][c])));
            }
        }
        out
    }
}
