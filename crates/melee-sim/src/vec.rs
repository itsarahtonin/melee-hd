//! Three-component vectors, with the SDK's paired-single vector routines (`dolphin/mtx/vec.c`).

use std::ops::{Add, Neg, Sub};

use gekko_fp::{self as fp, Ps};

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
    fn add(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fadds, store)
    }
}

/// C's component-by-component subtraction.
impl Sub for Vec3 {
    type Output = Vec3;
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
    pub fn sdk_add(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fadds, store_paired)
    }

    /// `PSVECSubtract`: the difference, stored through `psq_st`.
    pub fn sdk_sub(self, b: Vec3) -> Vec3 {
        each(self, b, fp::fsubs, store_paired)
    }

    /// Every component times `s`, `PSVECScale`.
    pub fn scale(self, s: f32) -> Vec3 {
        let s = splat(s);
        Vec3::from_lanes(fp::ps_muls0(self.xy(), s), fp::ps_muls0(self.z1(), s))
    }

    /// The dot product, `PSVECDotProduct`: (x·x' + y·y') + z·z'.
    pub fn dot(self, b: Vec3) -> f32 {
        let yz = fp::ps_mul(load2(self.y, self.z), load2(b.y, b.z));
        let sum = fp::ps_madd(self.xy(), b.xy(), yz);
        store(fp::ps_sum0(sum, yz, yz).ps0)
    }

    /// The cross product, `PSVECCrossProduct`.
    pub fn cross(self, b: Vec3) -> Vec3 {
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
    pub fn normalize(self) -> Vec3 {
        let (xy, z) = (self.xy(), self.z1());
        let sq = fp::ps_mul(xy, xy);
        let sum = fp::ps_sum0(fp::ps_madd(z, z, sq), z, sq);
        let (_, r) = self.length_squared_and_rsqrt(sum);
        let r = Ps::splat(r);
        Vec3::from_lanes(fp::ps_muls0(xy, r), fp::ps_muls0(z, r))
    }

    /// The length, `PSVECMag`: zero for a zero vector.
    pub fn length(self) -> f32 {
        let sq = fp::ps_mul(self.xy(), self.xy());
        let z = splat(self.z);
        let sum = fp::ps_sum0(fp::ps_madd(z, z, sq), sq, sq);
        let (sum, r) = self.length_squared_and_rsqrt(sum);
        let r = fp::fsel(r, r, sum.ps0);
        store(fp::fmuls(sum.ps0, r))
    }
}
