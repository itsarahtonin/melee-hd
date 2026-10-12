//! GXPixel.c: fog, blending, Z and the pixel format.

use super::enums::*;
use super::{Color, Gx, mem, reg, set_field};
use gekko_fp as fp;
use melee_sim::ppc::{cvt_fp2unsigned, slw};

/// `GXFogAdjTable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FogAdjTable {
    pub r: [u16; 10],
}

/// The inline `sqrtf` of MSL's math_ppc.h as the SDK was compiled: three Newton steps on
/// `frsqrte` in double precision, no fused multiply-adds. Zero, negatives and NaN come back as
/// they are.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub(crate) fn sqrtf(x: f64) -> f64 {
    if !(x > 0.0) {
        return x;
    }
    let mut guess = fp::frsqrte(x);
    for _ in 0..3 {
        guess = fp::fmul(
            fp::fmul(0.5, guess),
            fp::fsub(3.0, fp::fmul(x, fp::fmul(guess, guess))),
        );
    }
    // y is a volatile float: stored and loaded back.
    reg(mem(fp::frsp(fp::fmul(x, guess))))
}

impl Gx {
    #[doc(alias = "GXSetFog")]
    pub fn set_fog(
        &mut self,
        ty: u32,
        startz: f32,
        endz: f32,
        nearz: f32,
        farz: f32,
        color: Color,
    ) {
        let (startz, endz, nearz, farz) = (reg(startz), reg(endz), reg(nearz), reg(farz));
        let (a, b, c) = if farz == nearz || endz == startz {
            (0.0, 0.5, 0.0)
        } else {
            (
                fp::fdivs(
                    fp::fmuls(farz, nearz),
                    fp::fmuls(fp::fsubs(farz, nearz), fp::fsubs(endz, startz)),
                ),
                fp::fdivs(farz, fp::fsubs(farz, nearz)),
                fp::fdivs(startz, fp::fsubs(endz, startz)),
            )
        };
        let mut b_mant = b;
        let mut b_expn = 0u32;
        while b_mant > 1.0 {
            b_mant = fp::fmuls(b_mant, 0.5);
            b_expn = b_expn.wrapping_add(1);
        }
        while b_mant > 0.0 && b_mant < 0.5 {
            b_mant = fp::fmuls(b_mant, 2.0);
            b_expn = b_expn.wrapping_sub(1);
        }
        let shift = slw(1, b_expn.wrapping_add(1)) as i32;
        let a = mem(fp::fdivs(a, fp::frsp(f64::from(shift))));
        let b_m = cvt_fp2unsigned(fp::fmuls(8_388_638.0, b_mant));
        let b_s = b_expn.wrapping_add(1);
        let c = mem(c);

        let mut fog1 = 0u32;
        set_field(&mut fog1, 24, 0, b_m);
        set_field(&mut fog1, 8, 24, 0xEF);
        let mut fog2 = 0u32;
        set_field(&mut fog2, 5, 0, b_s);
        set_field(&mut fog2, 8, 24, 0xF0);
        let a_hex = a.to_bits();
        let c_hex = c.to_bits();
        let mut fog0 = 0u32;
        set_field(&mut fog0, 11, 0, (a_hex >> 12) & 0x7FF);
        set_field(&mut fog0, 8, 11, (a_hex >> 23) & 0xFF);
        set_field(&mut fog0, 1, 19, a_hex >> 31);
        set_field(&mut fog0, 8, 24, 0xEE);
        let mut fog3 = 0u32;
        set_field(&mut fog3, 11, 0, (c_hex >> 12) & 0x7FF);
        set_field(&mut fog3, 8, 11, (c_hex >> 23) & 0xFF);
        set_field(&mut fog3, 1, 19, c_hex >> 31);
        set_field(&mut fog3, 1, 20, 0);
        set_field(&mut fog3, 3, 21, ty);
        set_field(&mut fog3, 8, 24, 0xF1);
        let mut fogclr = 0u32;
        set_field(&mut fogclr, 8, 0, u32::from(color.b));
        set_field(&mut fogclr, 8, 8, u32::from(color.g));
        set_field(&mut fogclr, 8, 16, u32::from(color.r));
        set_field(&mut fogclr, 8, 24, 0xF2);
        self.write_ras_reg(fog0);
        self.write_ras_reg(fog1);
        self.write_ras_reg(fog2);
        self.write_ras_reg(fog3);
        self.write_ras_reg(fogclr);
        self.data.bp_sent = 0;
    }

    /// `GXInitFogAdjTable`.
    pub fn init_fog_adj_table(&self, table: &mut FogAdjTable, width: u16, projmtx: &[[f32; 4]; 4]) {
        let p = |r: usize, c: usize| reg(projmtx[r][c]);
        let (near_z, side_x) = if p(3, 3) == 0.0 {
            let near_z = fp::fdivs(p(2, 3), fp::fsubs(p(2, 2), 1.0));
            (
                near_z,
                fp::fdivs(fp::fmuls(near_z, fp::fadds(1.0, p(0, 2))), p(0, 0)),
            )
        } else {
            (
                fp::fdivs(fp::fadds(1.0, p(2, 3)), p(2, 2)),
                fp::fdivs(fp::fneg(fp::fsubs(p(0, 3), 1.0)), p(0, 0)),
            )
        };
        let iw = fp::fdivs(2.0, fp::frsp(f64::from(width)));
        let nn = fp::fmuls(near_z, near_z);
        for i in 0..10u32 {
            let mut xi = fp::frsp(f64::from((i + 1) << 5));
            xi = fp::fmuls(xi, iw);
            xi = fp::fmuls(xi, side_x);
            let range_val = sqrtf(fp::fadds(1.0, fp::fdivs(fp::fmuls(xi, xi), nn)));
            table.r[i as usize] = (cvt_fp2unsigned(fp::fmuls(256.0, range_val)) & 0xFFF) as u16;
        }
    }

    /// `GXSetFogRangeAdj`: the table is read only when enabled.
    #[doc(alias = "GXSetFogRangeAdj")]
    pub fn set_fog_range_adj(&mut self, enable: u8, center: u16, table: Option<&FogAdjTable>) {
        if enable != 0 {
            let table = table.copied().unwrap_or_default();
            for i in (0..10u32).step_by(2) {
                let mut range_adj = 0u32;
                set_field(&mut range_adj, 12, 0, u32::from(table.r[i as usize]));
                set_field(&mut range_adj, 12, 12, u32::from(table.r[i as usize + 1]));
                set_field(&mut range_adj, 8, 24, (i >> 1) + 0xE9);
                self.write_ras_reg(range_adj);
            }
        }
        let mut range_c = 0u32;
        set_field(&mut range_c, 10, 0, u32::from(center) + 342);
        set_field(&mut range_c, 1, 10, u32::from(enable));
        set_field(&mut range_c, 8, 24, 0xE8);
        self.write_ras_reg(range_c);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetBlendMode")]
    pub fn set_blend_mode(&mut self, ty: u32, src_factor: u32, dst_factor: u32, op: u32) {
        let c = &mut self.data.cmode0;
        set_field(
            c,
            1,
            0,
            u32::from(ty == GX_BM_BLEND || ty == GX_BM_SUBTRACT),
        );
        set_field(c, 1, 11, u32::from(ty == GX_BM_SUBTRACT));
        set_field(c, 1, 1, u32::from(ty == GX_BM_LOGIC));
        set_field(c, 4, 12, op);
        set_field(c, 3, 8, src_factor);
        set_field(c, 3, 5, dst_factor);
        set_field(c, 8, 24, 0x41);
        self.write_ras_reg(self.data.cmode0);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetColorUpdate")]
    pub fn set_color_update(&mut self, update_enable: u8) {
        set_field(&mut self.data.cmode0, 1, 3, u32::from(update_enable));
        self.write_ras_reg(self.data.cmode0);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetAlphaUpdate")]
    pub fn set_alpha_update(&mut self, update_enable: u8) {
        set_field(&mut self.data.cmode0, 1, 4, u32::from(update_enable));
        self.write_ras_reg(self.data.cmode0);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetZMode")]
    pub fn set_z_mode(&mut self, compare_enable: u8, func: u32, update_enable: u8) {
        let z = &mut self.data.zmode;
        set_field(z, 1, 0, u32::from(compare_enable));
        set_field(z, 3, 1, func);
        set_field(z, 1, 4, u32::from(update_enable));
        self.write_ras_reg(self.data.zmode);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetZCompLoc")]
    pub fn set_z_comp_loc(&mut self, before_tex: u8) {
        set_field(&mut self.data.pe_ctrl, 1, 6, u32::from(before_tex));
        self.write_ras_reg(self.data.pe_ctrl);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetPixelFmt")]
    pub fn set_pixel_fmt(&mut self, pix_fmt: u32, z_fmt: u32) {
        const P2F: [u32; 8] = [0, 1, 2, 3, 4, 4, 4, 5];
        let old_pe_ctrl = self.data.pe_ctrl;
        let p2f = P2F[pix_fmt as usize];
        set_field(&mut self.data.pe_ctrl, 3, 0, p2f);
        set_field(&mut self.data.pe_ctrl, 3, 3, z_fmt);
        if old_pe_ctrl != self.data.pe_ctrl {
            self.write_ras_reg(self.data.pe_ctrl);
            let aa = u32::from(pix_fmt == GX_PF_RGB565_Z16);
            set_field(&mut self.data.gen_mode, 1, 9, aa);
            self.data.dirty_state |= 4;
        }
        if p2f == 4 {
            set_field(&mut self.data.cmode1, 2, 9, (pix_fmt.wrapping_sub(4)) & 3);
            set_field(&mut self.data.cmode1, 8, 24, 0x42);
            self.write_ras_reg(self.data.cmode1);
        }
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetDither")]
    pub fn set_dither(&mut self, dither: u8) {
        set_field(&mut self.data.cmode0, 1, 2, u32::from(dither));
        self.write_ras_reg(self.data.cmode0);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetDstAlpha")]
    pub fn set_dst_alpha(&mut self, enable: u8, alpha: u8) {
        set_field(&mut self.data.cmode1, 8, 0, u32::from(alpha));
        set_field(&mut self.data.cmode1, 1, 8, u32::from(enable));
        self.write_ras_reg(self.data.cmode1);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetFieldMask")]
    pub fn set_field_mask(&mut self, odd_mask: u8, even_mask: u8) {
        let mut r = 0u32;
        set_field(&mut r, 1, 0, u32::from(even_mask));
        set_field(&mut r, 1, 1, u32::from(odd_mask));
        set_field(&mut r, 8, 24, 0x44);
        self.write_ras_reg(r);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetFieldMode")]
    pub fn set_field_mode(&mut self, field_mode: u8, half_aspect_ratio: u8) {
        set_field(&mut self.data.lp_size, 1, 22, u32::from(half_aspect_ratio));
        self.write_ras_reg(self.data.lp_size);
        self.flush_texture_state();
        self.write_ras_reg(u32::from(field_mode) | 0x6800_0000);
        self.flush_texture_state();
    }
}
