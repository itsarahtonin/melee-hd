//! HAL's own matrix routines (`sysdolphin/baselib/mtx.c`), which compute in scalar code where
//! the SDK's use paired singles.

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::mtx::Mtx34;

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
        // The magnitude goes through memory: stored, its sign bit cleared, loaded back.
        let magnitude = fp::lfs(fp::stfs(det) & !0x8000_0000);
        if magnitude < f64::from(1.0e-10_f32) {
            return Mtx34::IDENTITY;
        }
        let m = |r: usize, c: usize| reg(self.m[r][c]);
        let inv = fp::fdivs(1.0, det);
        // Cofactors alternate between `a·c - b` and `-(a·c - b)`.
        let plus = |a, c, b| mem(fp::fmuls(inv, fp::fmsubs(a, c, b)));
        let minus = |a, c, b| mem(fp::fmuls(inv, fp::fnmsubs(a, c, b)));
        let mut out = Mtx34::default();
        out.m[0][0] = plus(m(1, 1), m(2, 2), fp::fmuls(m(2, 1), m(1, 2)));
        out.m[0][1] = minus(m(0, 1), m(2, 2), fp::fmuls(m(2, 1), m(0, 2)));
        out.m[0][2] = plus(m(0, 1), m(1, 2), fp::fmuls(m(1, 1), m(0, 2)));
        out.m[1][0] = minus(m(1, 0), m(2, 2), fp::fmuls(m(2, 0), m(1, 2)));
        out.m[1][1] = plus(m(0, 0), m(2, 2), fp::fmuls(m(2, 0), m(0, 2)));
        out.m[1][2] = minus(m(0, 0), m(1, 2), fp::fmuls(m(1, 0), m(0, 2)));
        out.m[2][0] = plus(m(1, 0), m(2, 1), fp::fmuls(m(2, 0), m(1, 1)));
        out.m[2][1] = minus(m(0, 0), m(2, 1), fp::fmuls(m(2, 0), m(0, 1)));
        out.m[2][2] = plus(m(0, 0), m(1, 1), fp::fmuls(m(1, 0), m(0, 1)));
        // The translation reads the rotation back from the stored result.
        for r in 0..3 {
            let d = |c: usize| reg(out.m[r][c]);
            let partial = fp::fmsubs(fp::fneg(d(0)), m(0, 3), fp::fmuls(d(1), m(1, 3)));
            out.m[r][3] = mem(fp::fnmsubs(d(2), m(2, 3), partial));
        }
        out
    }
}
