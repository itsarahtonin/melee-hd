//! Three-component vectors: the SDK's paired-single routines (`dolphin/mtx/vec.c`), named
//! `sdk_*`, and the game's own helpers (`melee/lb/lbvector.c`), whose C fuses and rounds
//! differently.

use std::ops::{Add, Neg, Sub};

use gekko_fp::{self as fp, Ps};

use crate::math;
use crate::pairs::{load1, load2, splat, store, store_paired};

/// `Vec3` in the original: three floats.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Componentwise single-precision arithmetic. `put` is how the result reaches memory: `stfs`
/// keeps a subnormal result, `psq_st` flushes it to zero on hardware.
fn each(a: Vec3, b: Vec3, op: fn(f64, f64) -> f64, put: fn(f64) -> f32) -> Vec3 {
    let lane = |p: f32, q: f32| put(op(fp::lfs(p.to_bits()), fp::lfs(q.to_bits())));
    Vec3::new(lane(a.x, b.x), lane(a.y, b.y), lane(a.z, b.z))
}

/// C's component-by-component addition (`lbVector_Add` and inline code).
impl Add for Vec3 {
    type Output = Vec3;
    #[doc(alias = "lbVector_Add")]
    fn add(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fadds, store)
    }
}

/// C's component-by-component subtraction.
impl Sub for Vec3 {
    type Output = Vec3;
    #[doc(alias = "lbVector_Sub")]
    #[doc(alias = "lbVector_Diff")]
    fn sub(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fsubs, store)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        let lane = |p: f32| store(fp::fneg(fp::lfs(p.to_bits())));
        Vec3::new(lane(self.x), lane(self.y), lane(self.z))
    }
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    fn xy(self) -> Ps {
        load2(self.x, self.y)
    }

    fn z1(self) -> Ps {
        load1(self.z)
    }

    fn from_lanes(xy: Ps, z: Ps) -> Vec3 {
        Vec3::new(
            store_paired(xy.ps0),
            store_paired(xy.ps1),
            store_paired(z.ps0),
        )
    }

    /// `PSVECAdd`: the sum, stored through `psq_st`.
    #[doc(alias = "PSVECAdd")]
    pub fn sdk_add(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fadds, store_paired)
    }

    /// `PSVECSubtract`: the difference, stored through `psq_st`.
    #[doc(alias = "PSVECSubtract")]
    pub fn sdk_sub(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fsubs, store_paired)
    }

    /// Every component times `s`, `PSVECScale`.
    #[doc(alias = "PSVECScale")]
    pub fn sdk_scale(self, s: f32) -> Vec3 {
        let s = splat(s);
        Vec3::from_lanes(fp::ps_muls0(self.xy(), s), fp::ps_muls0(self.z1(), s))
    }

    /// The dot product, `PSVECDotProduct`: (x·x' + y·y') + z·z'.
    #[doc(alias = "PSVECDotProduct")]
    pub fn sdk_dot(self, b: Vec3) -> f32 {
        let yz = fp::ps_mul(load2(self.y, self.z), load2(b.y, b.z));
        let sum = fp::ps_madd(self.xy(), b.xy(), yz);
        store(fp::ps_sum0(sum, yz, yz).ps0)
    }

    /// The cross product, `PSVECCrossProduct`.
    #[doc(alias = "PSVECCrossProduct")]
    pub fn sdk_cross(self, b: Vec3) -> Vec3 {
        let (bxy, az, axy, bz) = (b.xy(), splat(self.z), self.xy(), splat(b.z));
        let byx = fp::ps_merge10(bxy, bxy);
        let b_az = fp::ps_mul(bxy, az);
        let b_ax = fp::ps_muls0(bxy, axy);
        let first = fp::ps_msub(axy, bz, b_az);
        let second = fp::ps_msub(axy, byx, b_ax);
        let x = fp::ps_merge11(first, first);
        let yz = fp::ps_neg(fp::ps_merge01(first, second));
        Vec3::new(
            store_paired(x.ps0),
            store_paired(yz.ps0),
            store_paired(yz.ps1),
        )
    }

    /// The squared length's reciprocal square root, by one Newton step on `frsqrte`, as
    /// `PSVECNormalize` and `PSVECMag` compute it.
    fn length_squared_and_rsqrt(self, sum: Ps) -> (Ps, f64) {
        let e = fp::frsqrte(sum.ps0);
        let ee = fp::fmuls(e, e);
        let half_e = fp::fmuls(e, 0.5);
        let r = fp::fmuls(fp::fnmsubs(ee, sum.ps0, 3.0), half_e);
        (sum, r)
    }

    /// The unit vector, `PSVECNormalize`.
    #[doc(alias = "PSVECNormalize")]
    pub fn sdk_normalize(self) -> Vec3 {
        let (xy, z) = (self.xy(), self.z1());
        let sq = fp::ps_mul(xy, xy);
        let sum = fp::ps_sum0(fp::ps_madd(z, z, sq), z, sq);
        let (_, r) = self.length_squared_and_rsqrt(sum);
        let r = Ps::splat(r);
        Vec3::from_lanes(fp::ps_muls0(xy, r), fp::ps_muls0(z, r))
    }

    /// The length, `PSVECMag`: zero for a zero vector.
    #[doc(alias = "PSVECMag")]
    pub fn sdk_length(self) -> f32 {
        let sq = fp::ps_mul(self.xy(), self.xy());
        let z = splat(self.z);
        let sum = fp::ps_sum0(fp::ps_madd(z, z, sq), sq, sq);
        let (sum, r) = self.length_squared_and_rsqrt(sum);
        let r = fp::fsel(r, r, sum.ps0);
        store(fp::fmuls(sum.ps0, r))
    }
}

/// A float as a register holds it after `lfs`.
fn r(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// The game's own vector helpers (`melee/lb/lbvector.c` and its inline functions).
impl Vec3 {
    /// The length as `lbVector_Len` computes it: sqrt(z² + (x² + y²)), nothing fused.
    pub fn length(self) -> f32 {
        let (x, y, z) = (r(self.x), r(self.y), r(self.z));
        let xy = fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y));
        math::sqrt(store(fp::fadds(fp::fmuls(z, z), xy)))
    }

    /// The length with the squares fused, as other inline copies compute it:
    /// sqrt(z·z + (x·x + y·y)).
    pub fn length_fused(self) -> f32 {
        let (x, y, z) = (r(self.x), r(self.y), r(self.z));
        let sq = fp::fmadds(z, z, fp::fmadds(x, x, fp::fmuls(y, y)));
        math::sqrt(store(sq))
    }

    /// Scales to unit length and returns the old length, `lbVector_Normalize`; a zero vector
    /// stays as it is and returns 0.
    #[doc(alias = "lbVector_Normalize")]
    pub fn normalize(&mut self) -> f32 {
        let len = r(self.length());
        if len == 0.0 {
            return 0.0;
        }
        let inv = fp::fdivs(1.0, len);
        self.x = store(fp::fmuls(r(self.x), inv));
        self.y = store(fp::fmuls(r(self.y), inv));
        self.z = store(fp::fmuls(r(self.z), inv));
        store(len)
    }

    /// The same in the XY plane, `lbVector_NormalizeXY`; z is left alone.
    #[doc(alias = "lbVector_NormalizeXY")]
    pub fn normalize_xy(&mut self) -> f32 {
        let len = r(self.length_xy_accurate());
        if len == 0.0 {
            return 0.0;
        }
        let inv = fp::fdivs(1.0, len);
        self.x = store(fp::fmuls(r(self.x), inv));
        self.y = store(fp::fmuls(r(self.y), inv));
        store(len)
    }

    /// sqrt(x² + y²) with `sqrtf_accurate`.
    fn length_xy_accurate(self) -> f32 {
        let (x, y) = (r(self.x), r(self.y));
        math::sqrt_accurate(store(fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y))))
    }

    /// x and y summed, z kept, `lbVector_Add_xy`.
    #[doc(alias = "lbVector_Add_xy")]
    pub fn add_xy(self, b: Vec3) -> Vec3 {
        Vec3::new(
            store(fp::fadds(r(self.x), r(b.x))),
            store(fp::fadds(r(self.y), r(b.y))),
            self.z,
        )
    }

    /// The normalized cross product, `lbVector_CrossprodNormalized`.
    #[doc(alias = "lbVector_CrossprodNormalized")]
    pub fn cross_normalized(self, b: Vec3) -> Vec3 {
        let mut c = self.sdk_cross(b);
        c.normalize();
        c
    }

    /// Clamps a cosine to [-1, 1] and takes its arc cosine.
    fn acos_clamped(mut cosine: f64) -> f32 {
        if cosine > 1.0 {
            cosine = 1.0;
        }
        if cosine < fp::fneg(1.0) {
            cosine = fp::fneg(1.0);
        }
        math::acos(store(cosine))
    }

    /// The angle between two vectors, `lbVector_Angle`: 0 when either is about zero.
    // A NaN length returns 0, as the original's branch does.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    #[doc(alias = "lbVector_Angle")]
    pub fn angle(self, b: Vec3) -> f32 {
        let lb = r(b.length());
        let lengths = fp::fmuls(r(self.length()), lb);
        if !(lengths > f64::from(1.0e-10_f32)) {
            return 0.0;
        }
        let (a, b) = (self, b);
        let dot = fp::fmadds(
            r(a.z),
            r(b.z),
            fp::fmadds(r(a.x), r(b.x), fp::fmuls(r(a.y), r(b.y))),
        );
        Vec3::acos_clamped(fp::fdivs(dot, lengths))
    }

    /// The angle between two vectors in the XY plane, `lbVector_AngleXY`.
    #[doc(alias = "lbVector_AngleXY")]
    pub fn angle_xy(self, b: Vec3) -> f32 {
        let lb = r(b.length_xy_accurate());
        let lengths = fp::fmuls(r(self.length_xy_accurate()), lb);
        if lengths == 0.0 {
            return 0.0;
        }
        let dot = fp::fmadds(r(self.x), r(b.x), fp::fmuls(r(self.y), r(b.y)));
        Vec3::acos_clamped(fp::fdivs(dot, lengths))
    }

    /// The cosine of the angle in the XY plane, unclamped, `lbVector_CosAngle`.
    #[doc(alias = "lbVector_CosAngle")]
    pub fn cos_angle_xy(self, b: Vec3) -> f32 {
        let len_xy = |v: Vec3| {
            let (x, y) = (r(v.x), r(v.y));
            r(math::sqrt(store(fp::fadds(
                fp::fmuls(x, x),
                fp::fmuls(y, y),
            ))))
        };
        let lb = len_xy(b);
        let lengths = fp::fmuls(len_xy(self), lb);
        let dot = fp::fmadds(r(self.x), r(b.x), fp::fmuls(r(self.y), r(b.y)));
        store(fp::fdivs(dot, lengths))
    }

    /// Reflects x and y across the line through the origin normal to `unit_axis`,
    /// `lbVector_Mirror`.
    #[doc(alias = "lbVector_Mirror")]
    pub fn mirror_xy(&mut self, unit_axis: Vec3) {
        let (ux, uy) = (r(unit_axis.x), r(unit_axis.y));
        let dot = fp::fmadds(ux, r(self.x), fp::fmuls(uy, r(self.y)));
        let f = fp::fmuls(fp::fneg(2.0), dot);
        self.x = store(fp::fmadds(ux, f, r(self.x)));
        self.y = store(fp::fmadds(uy, f, r(self.y)));
    }

    /// `self + (b - self)·t`, `lbVector_Lerp`.
    #[doc(alias = "lbVector_Lerp")]
    pub fn lerp(self, b: Vec3, t: f32) -> Vec3 {
        let d = b - self;
        let t = r(t);
        let scaled = Vec3::new(
            store(fp::fmuls(r(d.x), t)),
            store(fp::fmuls(r(d.y), t)),
            store(fp::fmuls(r(d.z), t)),
        );
        scaled + self
    }

    /// The point on the line through `a` and `b` closest to `c`, and its distance from `c`,
    /// `lbVector_8000E838`. When `a` and `b` nearly coincide, the point is `a`.
    #[doc(alias = "lbVector_8000E838")]
    pub fn closest_on_line(a: Vec3, b: Vec3, c: Vec3) -> (Vec3, f32) {
        let ba = b - a;
        let (bx, by, bz) = (r(ba.x), r(ba.y), r(ba.z));
        let sq = fp::fmadds(bz, bz, fp::fmadds(bx, bx, fp::fmuls(by, by)));
        let ca = c - a;
        let tiny = f64::from(1.0e-5_f32);
        if sq < tiny && sq > fp::fneg(tiny) {
            return (a, ca.length_fused());
        }
        let dot = fp::fmadds(bz, r(ca.z), fp::fmadds(bx, r(ca.x), fp::fmuls(by, r(ca.y))));
        let f = fp::fdivs(dot, sq);
        let d = Vec3::new(
            store(fp::fmadds(bx, f, r(a.x))),
            store(fp::fmadds(by, f, r(a.y))),
            store(fp::fmadds(bz, f, r(a.z))),
        );
        (d, (c - d).length_fused())
    }
}
