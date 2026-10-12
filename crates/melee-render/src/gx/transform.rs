//! GXTransform.c: matrices, projection, viewport and scissor.

use super::enums::*;
use super::{Gx, mem, reg, set_field};
use gekko_fp as fp;

/// `GXProject`: a point through `mtx`, the projection `pm` (as `GXGetProjectionv` gives it)
/// and the viewport `vp` (as `GXGetViewportv` gives it), to screen coordinates (sx, sy, sz).
#[doc(alias = "GXProject")]
pub fn project(
    x: f32,
    y: f32,
    z: f32,
    mtx: &[[f32; 4]; 3],
    pm: &[f32; 7],
    vp: &[f32; 6],
) -> (f32, f32, f32) {
    let (x, y, z) = (reg(x), reg(y), reg(z));
    let m = |r: usize, c: usize| reg(mtx[r][c]);
    let row = |r: usize| {
        mem(fp::fadds(
            m(r, 3),
            fp::fadds(
                fp::fmuls(m(r, 2), z),
                fp::fadds(fp::fmuls(m(r, 0), x), fp::fmuls(m(r, 1), y)),
            ),
        ))
    };
    // peye lives on the stack.
    let (px, py, pz) = (reg(row(0)), reg(row(1)), reg(row(2)));
    let p = |i: usize| reg(pm[i]);
    let (xc, yc, zc, wc) = if p(0) == 0.0 {
        (
            fp::fadds(fp::fmuls(px, p(1)), fp::fmuls(pz, p(2))),
            fp::fadds(fp::fmuls(py, p(3)), fp::fmuls(pz, p(4))),
            fp::fadds(p(6), fp::fmuls(pz, p(5))),
            fp::fdivs(1.0, fp::fneg(pz)),
        )
    } else {
        (
            fp::fadds(p(2), fp::fmuls(px, p(1))),
            fp::fadds(p(4), fp::fmuls(py, p(3))),
            fp::fadds(p(6), fp::fmuls(pz, p(5))),
            1.0,
        )
    };
    let v = |i: usize| reg(vp[i]);
    let sx = fp::fadds(
        fp::fdivs(v(2), 2.0),
        fp::fadds(v(0), fp::fmuls(wc, fp::fdivs(fp::fmuls(xc, v(2)), 2.0))),
    );
    let sy = fp::fadds(
        fp::fdivs(v(3), 2.0),
        fp::fadds(
            v(1),
            fp::fmuls(wc, fp::fdivs(fp::fmuls(fp::fneg(yc), v(3)), 2.0)),
        ),
    );
    let sz = fp::fadds(v(5), fp::fmuls(wc, fp::fmuls(zc, fp::fsubs(v(5), v(4)))));
    (mem(sx), mem(sy), mem(sz))
}

impl Gx {
    /// The projection's registers, after `GXSetProjection` or `GXSetProjectionv`.
    fn write_projection(&mut self) {
        self.write_u8(0x10);
        self.write_u32(0x0006_1020);
        for v in self.data.proj_mtx {
            self.write_f32(v);
        }
        self.write_u32(self.data.proj_type);
    }

    #[doc(alias = "GXSetProjection")]
    pub fn set_projection(&mut self, mtx: &[[f32; 4]; 4], ty: u32) {
        let g = &mut self.data;
        g.proj_type = ty;
        g.proj_mtx[0] = mtx[0][0];
        g.proj_mtx[2] = mtx[1][1];
        g.proj_mtx[4] = mtx[2][2];
        g.proj_mtx[5] = mtx[2][3];
        if ty == GX_ORTHOGRAPHIC {
            g.proj_mtx[1] = mtx[0][3];
            g.proj_mtx[3] = mtx[1][3];
        } else {
            g.proj_mtx[1] = mtx[0][2];
            g.proj_mtx[3] = mtx[1][2];
        }
        self.write_projection();
        self.data.bp_sent = 1;
    }

    /// `GXSetProjectionv`: the type as a float, then the six values.
    #[doc(alias = "GXSetProjectionv")]
    pub fn set_projectionv(&mut self, ptr: &[f32; 7]) {
        let g = &mut self.data;
        g.proj_type = melee_sim::ppc::cvt_fp2unsigned(reg(ptr[0]));
        g.proj_mtx.copy_from_slice(&ptr[1..7]);
        self.write_projection();
        self.data.bp_sent = 0;
    }

    /// `GXGetProjectionv`: the type as a float, then the six values.
    #[doc(alias = "GXGetProjectionv")]
    pub fn get_projectionv(&self) -> [f32; 7] {
        let g = &self.data;
        let mut out = [0.0; 7];
        out[0] = mem(fp::frsp(f64::from(g.proj_type)));
        out[1..].copy_from_slice(&g.proj_mtx);
        out
    }

    /// `WriteMTXPS4x3`: 12 floats, rows in order (`psq_st` keeps their bits).
    fn write_mtx_ps_4x3(&mut self, mtx: &[[f32; 4]]) {
        for row in &mtx[..3] {
            for &v in row {
                self.write_f32(v);
            }
        }
    }

    /// `WriteMTXPS4x2`: the first two rows.
    fn write_mtx_ps_4x2(&mut self, mtx: &[[f32; 4]]) {
        for row in &mtx[..2] {
            for &v in row {
                self.write_f32(v);
            }
        }
    }

    #[doc(alias = "GXLoadPosMtxImm")]
    pub fn load_pos_mtx_imm(&mut self, mtx: &[[f32; 4]; 3], id: u32) {
        let addr = id * 4;
        self.write_u8(0x10);
        self.write_u32(addr | 0xB0000);
        self.write_mtx_ps_4x3(mtx);
    }

    #[doc(alias = "GXLoadPosMtxIndx")]
    pub fn load_pos_mtx_indx(&mut self, mtx_indx: u16, id: u32) {
        let mut r = 0u32;
        set_field(&mut r, 12, 0, id * 4);
        set_field(&mut r, 4, 12, 11);
        set_field(&mut r, 16, 16, u32::from(mtx_indx));
        self.write_u8(0x20);
        self.write_u32(r);
    }

    /// `GXLoadNrmMtxImm`: the 3x3 part of `mtx` (`WriteMTXPS3x3from3x4`).
    #[doc(alias = "GXLoadNrmMtxImm")]
    pub fn load_nrm_mtx_imm(&mut self, mtx: &[[f32; 4]; 3], id: u32) {
        let addr = id * 3 + 0x400;
        self.write_u8(0x10);
        self.write_u32(addr | 0x80000);
        for row in mtx {
            for &v in &row[..3] {
                self.write_f32(v);
            }
        }
    }

    #[doc(alias = "GXLoadNrmMtxImm3x3")]
    pub fn load_nrm_mtx_imm3x3(&mut self, mtx: &[[f32; 3]; 3], id: u32) {
        let addr = id * 3 + 0x400;
        self.write_u8(0x10);
        self.write_u32(addr | 0x80000);
        for row in mtx {
            for &v in row {
                self.write_f32(v);
            }
        }
    }

    #[doc(alias = "GXLoadNrmMtxIndx3x3")]
    pub fn load_nrm_mtx_indx3x3(&mut self, mtx_indx: u16, id: u32) {
        let mut r = 0u32;
        set_field(&mut r, 12, 0, id * 3 + 0x400);
        set_field(&mut r, 4, 12, 8);
        set_field(&mut r, 16, 16, u32::from(mtx_indx));
        self.write_u8(0x28);
        self.write_u32(r);
    }

    #[doc(alias = "GXSetCurrentMtx")]
    pub fn set_current_mtx(&mut self, id: u32) {
        set_field(&mut self.data.mat_idx_a, 6, 0, id);
        self.set_matrix_index(GX_VA_PNMTXIDX);
    }

    /// `GXLoadTexMtxImm`: `mtx` has 3 rows for `GX_MTX3X4`, 2 (or more) for `GX_MTX2X4`.
    #[doc(alias = "GXLoadTexMtxImm")]
    pub fn load_tex_mtx_imm(&mut self, mtx: &[[f32; 4]], id: u32, ty: u32) {
        let addr = if id >= GX_PTTEXMTX0 {
            (id - GX_PTTEXMTX0) * 4 + 0x500
        } else {
            id * 4
        };
        let count = if ty == GX_MTX2X4 { 8 } else { 12 };
        self.write_u8(0x10);
        self.write_u32(addr | ((count - 1) << 16));
        if ty == GX_MTX3X4 {
            self.write_mtx_ps_4x3(mtx);
        } else {
            self.write_mtx_ps_4x2(mtx);
        }
    }

    #[doc(alias = "GXLoadTexMtxIndx")]
    pub fn load_tex_mtx_indx(&mut self, mtx_indx: u16, id: u32, ty: u32) {
        let offset = if id >= GX_PTTEXMTX0 {
            (id - GX_PTTEXMTX0) * 4 + 0x500
        } else {
            id * 4
        };
        let count: u32 = if ty == GX_MTX2X4 { 8 } else { 12 };
        let mut r = 0u32;
        set_field(&mut r, 12, 0, offset);
        set_field(&mut r, 4, 12, count - 1);
        set_field(&mut r, 16, 16, u32::from(mtx_indx));
        self.write_u8(0x30);
        self.write_u32(r);
    }

    #[doc(alias = "GXSetViewportJitter")]
    #[allow(clippy::too_many_arguments)]
    pub fn set_viewport_jitter(
        &mut self,
        left: f32,
        top: f32,
        wd: f32,
        ht: f32,
        nearz: f32,
        farz: f32,
        field: u32,
    ) {
        let mut top = reg(top);
        if field == 0 {
            top = fp::fsubs(top, 0.5);
        }
        let (left, wd, ht, nearz, farz) = (reg(left), reg(wd), reg(ht), reg(nearz), reg(farz));
        let sx = fp::fdivs(wd, 2.0);
        let sy = fp::fdivs(fp::fneg(ht), 2.0);
        let ox = fp::fadds(342.0, fp::fadds(left, fp::fdivs(wd, 2.0)));
        let oy = fp::fadds(342.0, fp::fadds(top, fp::fdivs(ht, 2.0)));
        let zmin = fp::fmuls(16_777_215.0, nearz);
        let zmax = fp::fmuls(16_777_215.0, farz);
        let sz = fp::fsubs(zmax, zmin);
        let oz = zmax;
        let g = &mut self.data;
        g.vp_left = mem(left);
        g.vp_top = mem(top);
        g.vp_wd = mem(wd);
        g.vp_ht = mem(ht);
        g.vp_nearz = mem(nearz);
        g.vp_farz = mem(farz);
        if g.fg_range != 0 {
            let side_x = g.fg_side_x;
            self.set_range(mem(nearz), side_x);
        }
        self.write_u8(0x10);
        self.write_u32(0x5101A);
        for v in [sx, sy, sz, ox, oy, oz] {
            self.write_f32_reg(v);
        }
        self.data.bp_sent = 1;
    }

    #[doc(alias = "GXSetViewport")]
    pub fn set_viewport(&mut self, left: f32, top: f32, wd: f32, ht: f32, nearz: f32, farz: f32) {
        self.set_viewport_jitter(left, top, wd, ht, nearz, farz, 1);
    }

    /// `GXGetViewportv`: left, top, width, height, near, far.
    #[doc(alias = "GXGetViewportv")]
    pub fn get_viewportv(&self) -> [f32; 6] {
        let g = &self.data;
        [g.vp_left, g.vp_top, g.vp_wd, g.vp_ht, g.vp_nearz, g.vp_farz]
    }

    #[doc(alias = "GXSetScissor")]
    pub fn set_scissor(&mut self, left: u32, top: u32, wd: u32, ht: u32) {
        let tp = top.wrapping_add(342);
        let lf = left.wrapping_add(342);
        let bm = tp.wrapping_add(ht).wrapping_sub(1);
        let rt = lf.wrapping_add(wd).wrapping_sub(1);
        let g = &mut self.data;
        set_field(&mut g.su_scis0, 11, 0, tp);
        set_field(&mut g.su_scis0, 11, 12, lf);
        set_field(&mut g.su_scis1, 11, 0, bm);
        set_field(&mut g.su_scis1, 11, 12, rt);
        self.write_ras_reg(self.data.su_scis0);
        self.write_ras_reg(self.data.su_scis1);
        self.data.bp_sent = 0;
    }

    /// `GXGetScissor`: (left, top, width, height), with the original's offset of 340.
    #[doc(alias = "GXGetScissor")]
    pub fn get_scissor(&self) -> (u32, u32, u32, u32) {
        let g = &self.data;
        let tp = g.su_scis0 & 0x7FF;
        let lf = (g.su_scis0 & 0x7F_F000) >> 12;
        let bm = g.su_scis1 & 0x7FF;
        let rt = (g.su_scis1 & 0x7F_F000) >> 12;
        (
            lf.wrapping_sub(340),
            tp.wrapping_sub(340),
            rt.wrapping_sub(lf).wrapping_add(1),
            bm.wrapping_sub(tp).wrapping_add(1),
        )
    }

    #[doc(alias = "GXSetScissorBoxOffset")]
    pub fn set_scissor_box_offset(&mut self, x_off: i32, y_off: i32) {
        let hx = (x_off.wrapping_add(342) as u32) >> 1;
        let hy = (y_off.wrapping_add(342) as u32) >> 1;
        let mut r = 0u32;
        set_field(&mut r, 10, 0, hx);
        set_field(&mut r, 10, 10, hy);
        set_field(&mut r, 8, 24, 0x59);
        self.write_ras_reg(r);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetClipMode")]
    pub fn set_clip_mode(&mut self, mode: u32) {
        self.write_xf_reg(5, mode);
        self.data.bp_sent = 1;
    }

    /// `__GXSetMatrixIndex`: the matrix index registers to CP and XF, `A` for the position and
    /// texture coordinates 0-3, `B` for 4-7.
    #[doc(alias = "__GXSetMatrixIndex")]
    pub(crate) fn set_matrix_index(&mut self, mat_idx_attr: u32) {
        if mat_idx_attr < GX_VA_TEX4MTXIDX {
            self.write_cp_reg(0x30, self.data.mat_idx_a);
            self.write_xf_reg(24, self.data.mat_idx_a);
        } else {
            self.write_cp_reg(0x40, self.data.mat_idx_b);
            self.write_xf_reg(25, self.data.mat_idx_b);
        }
        self.data.bp_sent = 1;
    }

    /// `__GXSetRange` (GXStubs.c): empty in this SDK.
    #[doc(alias = "__GXSetRange")]
    pub(crate) fn set_range(&mut self, _nearz: f32, _fg_side_x: f32) {}
}
