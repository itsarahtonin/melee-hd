//! 3×4 matrices, with the SDK's paired-single matrix routines (`dolphin/mtx/mtx.c`,
//! `mtxvec.c`).

use gekko_fp::{self as fp, Ps};

use crate::pairs::{load1, load2, moved, moved_paired, splat, store_paired};
use crate::vec::Vec3;

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
        load2(self.m[i / 4][i % 4], self.m[i / 4][i % 4 + 1])
    }

    fn one(&self, i: usize) -> Ps {
        load1(self.m[i / 4][i % 4])
    }

    fn put(&mut self, i: usize, lanes: Ps) {
        self.m[i / 4][i % 4] = store_paired(lanes.ps0);
        self.m[i / 4][i % 4 + 1] = store_paired(lanes.ps1);
    }

    fn put1(&mut self, i: usize, lanes: Ps) {
        self.m[i / 4][i % 4] = store_paired(lanes.ps0);
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
}
