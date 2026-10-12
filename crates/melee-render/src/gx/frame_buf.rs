//! GXFrameBuf.c: render modes, EFB copies and their filters.

use super::enums::*;
use super::texture::get_image_tile_count;
use super::{Color, Gx, get_field, reg, set_field};
use gekko_fp as fp;
use melee_sim::ppc::cvt_fp2unsigned;

/// `GXRenderModeObj`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderModeObj {
    pub vi_tv_mode: u32,
    pub fb_width: u16,
    pub efb_height: u16,
    pub xfb_height: u16,
    pub vi_x_origin: u16,
    pub vi_y_origin: u16,
    pub vi_width: u16,
    pub vi_height: u16,
    pub xfb_mode: u32,
    pub field_rendering: u8,
    pub aa: u8,
    pub sample_pattern: [[u8; 2]; 12],
    pub vfilter: [u8; 7],
}

const SAMPLE_6: [[u8; 2]; 12] = [[6, 6]; 12];
const DEFLICKER: [u8; 7] = [8, 8, 10, 12, 10, 8, 8];

/// `GXNtsc480IntDf`, the render mode `GXInit` takes on an NTSC console.
pub const GX_NTSC_480_INT_DF: RenderModeObj = RenderModeObj {
    vi_tv_mode: 0,
    fb_width: 640,
    efb_height: 480,
    xfb_height: 480,
    vi_x_origin: 40,
    vi_y_origin: 0,
    vi_width: 640,
    vi_height: 480,
    xfb_mode: 1,
    field_rendering: 0,
    aa: 0,
    sample_pattern: SAMPLE_6,
    vfilter: DEFLICKER,
};

/// `GXPal528IntDf`, for a PAL console.
pub const GX_PAL_528_INT_DF: RenderModeObj = RenderModeObj {
    vi_tv_mode: 4,
    fb_width: 640,
    efb_height: 528,
    xfb_height: 528,
    vi_x_origin: 40,
    vi_y_origin: 23,
    vi_width: 640,
    vi_height: 528,
    xfb_mode: 1,
    field_rendering: 0,
    aa: 0,
    sample_pattern: SAMPLE_6,
    vfilter: DEFLICKER,
};

/// `GXMpal480IntDf`, for an MPAL console.
pub const GX_MPAL_480_INT_DF: RenderModeObj = RenderModeObj {
    vi_tv_mode: 8,
    fb_width: 640,
    efb_height: 480,
    xfb_height: 480,
    vi_x_origin: 40,
    vi_y_origin: 0,
    vi_width: 640,
    vi_height: 480,
    xfb_mode: 1,
    field_rendering: 0,
    aa: 0,
    sample_pattern: SAMPLE_6,
    vfilter: DEFLICKER,
};

/// `GXAdjustForOverscan`.
#[doc(alias = "GXAdjustForOverscan")]
pub fn adjust_for_overscan(rmin: &RenderModeObj, hor: u16, ver: u16) -> RenderModeObj {
    let hor2 = hor.wrapping_mul(2);
    let ver2 = ver.wrapping_mul(2);
    let mut rmout = *rmin;
    rmout.fb_width = rmin.fb_width.wrapping_sub(hor2);
    let verf = (u32::from(ver2) * u32::from(rmin.efb_height)) / u32::from(rmin.xfb_height);
    rmout.efb_height = rmin.efb_height.wrapping_sub(verf as u16);
    // VI_XFBMODE_SF is 0.
    if rmin.xfb_mode == 0 && (rmin.vi_tv_mode & 2) != 2 {
        rmout.xfb_height = rmin.xfb_height.wrapping_sub(ver);
    } else {
        rmout.xfb_height = rmin.xfb_height.wrapping_sub(ver2);
    }
    rmout.vi_width = rmin.vi_width.wrapping_sub(hor2);
    rmout.vi_height = rmin.vi_height.wrapping_sub(ver2);
    rmout.vi_x_origin = rmin.vi_x_origin.wrapping_add(hor);
    rmout.vi_y_origin = rmin.vi_y_origin.wrapping_add(ver);
    rmout
}

impl Gx {
    #[doc(alias = "GXSetDispCopySrc")]
    pub fn set_disp_copy_src(&mut self, left: u16, top: u16, wd: u16, ht: u16) {
        let g = &mut self.data;
        g.cp_disp_src = 0;
        set_field(&mut g.cp_disp_src, 10, 0, u32::from(left));
        set_field(&mut g.cp_disp_src, 10, 10, u32::from(top));
        set_field(&mut g.cp_disp_src, 8, 24, 0x49);
        g.cp_disp_size = 0;
        set_field(&mut g.cp_disp_size, 10, 0, u32::from(wd).wrapping_sub(1));
        set_field(&mut g.cp_disp_size, 10, 10, u32::from(ht).wrapping_sub(1));
        set_field(&mut g.cp_disp_size, 8, 24, 0x4A);
    }

    #[doc(alias = "GXSetTexCopySrc")]
    pub fn set_tex_copy_src(&mut self, left: u16, top: u16, wd: u16, ht: u16) {
        let g = &mut self.data;
        g.cp_tex_src = 0;
        set_field(&mut g.cp_tex_src, 10, 0, u32::from(left));
        set_field(&mut g.cp_tex_src, 10, 10, u32::from(top));
        set_field(&mut g.cp_tex_src, 8, 24, 0x49);
        g.cp_tex_size = 0;
        set_field(&mut g.cp_tex_size, 10, 0, u32::from(wd).wrapping_sub(1));
        set_field(&mut g.cp_tex_size, 10, 10, u32::from(ht).wrapping_sub(1));
        set_field(&mut g.cp_tex_size, 8, 24, 0x4A);
    }

    #[doc(alias = "GXSetDispCopyDst")]
    pub fn set_disp_copy_dst(&mut self, wd: u16, _ht: u16) {
        let stride = wd.wrapping_mul(2);
        let g = &mut self.data;
        g.cp_disp_stride = 0;
        set_field(&mut g.cp_disp_stride, 10, 0, u32::from(stride >> 5));
        set_field(&mut g.cp_disp_stride, 8, 24, 0x4D);
    }

    #[doc(alias = "GXSetTexCopyDst")]
    pub fn set_tex_copy_dst(&mut self, wd: u16, ht: u16, fmt: u32, mipmap: u8) {
        let g = &mut self.data;
        g.cp_tex_z = 0;
        let mut pe_tex_fmt = fmt & 0xF;
        if fmt == GX_TF_Z16 {
            pe_tex_fmt = 0xB;
        }
        match fmt {
            GX_TF_I4 | GX_TF_I8 | GX_TF_IA4 | GX_TF_IA8 | GX_CTF_YUVA8 => {
                set_field(&mut g.cp_tex, 2, 15, 3)
            }
            _ => set_field(&mut g.cp_tex, 2, 15, 2),
        }
        g.cp_tex_z = u8::from((fmt & GX_TF_ZTF) == GX_TF_ZTF);
        let pe_tex_fmt_h = (pe_tex_fmt >> 3) & 1;
        set_field(&mut g.cp_tex, 1, 3, pe_tex_fmt_h);
        let pe_tex_fmt = pe_tex_fmt & 7;
        let (row_tiles, _col_tiles, cmp_tiles) = get_image_tile_count(fmt, wd, ht);
        g.cp_tex_stride = 0;
        set_field(&mut g.cp_tex_stride, 10, 0, row_tiles * cmp_tiles);
        set_field(&mut g.cp_tex_stride, 8, 24, 0x4D);
        set_field(&mut g.cp_tex, 1, 9, u32::from(mipmap));
        set_field(&mut g.cp_tex, 3, 4, pe_tex_fmt);
    }

    #[doc(alias = "GXSetDispCopyFrame2Field")]
    pub fn set_disp_copy_frame2_field(&mut self, mode: u32) {
        set_field(&mut self.data.cp_disp, 2, 12, mode);
        set_field(&mut self.data.cp_tex, 2, 12, 0);
    }

    #[doc(alias = "GXSetCopyClamp")]
    pub fn set_copy_clamp(&mut self, clamp: u32) {
        let clmp_t = u32::from((clamp & 1) == 1);
        let clmp_b = u32::from((clamp & 2) == 2);
        let g = &mut self.data;
        set_field(&mut g.cp_disp, 1, 0, clmp_t);
        set_field(&mut g.cp_disp, 1, 1, clmp_b);
        set_field(&mut g.cp_tex, 1, 0, clmp_t);
        set_field(&mut g.cp_tex, 1, 1, clmp_b);
    }

    /// `GXSetDispCopyYScale`: returns the number of XFB lines the copy makes.
    #[doc(alias = "GXSetDispCopyYScale")]
    pub fn set_disp_copy_y_scale(&mut self, vscale: f32) -> u32 {
        let i_scale = cvt_fp2unsigned(fp::fdivs(256.0, reg(vscale))) & 0x1FF;
        let f_scale = fp::fdivs(256.0, fp::frsp(f64::from(i_scale)));
        let enable = u32::from(i_scale != 256);
        let mut r = 0u32;
        set_field(&mut r, 9, 0, i_scale);
        set_field(&mut r, 8, 24, 0x4E);
        self.write_ras_reg(r);
        self.data.bp_sent = 0;
        set_field(&mut self.data.cp_disp, 1, 10, enable);
        let ht = get_field(self.data.cp_disp_size, 10, 10) + 1;
        cvt_fp2unsigned(fp::fmuls(fp::frsp(f64::from(ht)), f_scale))
    }

    #[doc(alias = "GXSetCopyClear")]
    pub fn set_copy_clear(&mut self, clear_clr: Color, clear_z: u32) {
        let mut r = 0u32;
        set_field(&mut r, 8, 0, u32::from(clear_clr.r));
        set_field(&mut r, 8, 8, u32::from(clear_clr.a));
        set_field(&mut r, 8, 24, 0x4F);
        self.write_ras_reg(r);
        let mut r = 0u32;
        set_field(&mut r, 8, 0, u32::from(clear_clr.b));
        set_field(&mut r, 8, 8, u32::from(clear_clr.g));
        set_field(&mut r, 8, 24, 0x50);
        self.write_ras_reg(r);
        let mut r = 0u32;
        set_field(&mut r, 24, 0, clear_z);
        set_field(&mut r, 8, 24, 0x51);
        self.write_ras_reg(r);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetCopyFilter")]
    pub fn set_copy_filter(
        &mut self,
        aa: u8,
        sample_pattern: &[[u8; 2]; 12],
        vf: u8,
        vfilter: &[u8; 7],
    ) {
        let mut ms_loc = [0u32; 4];
        if aa != 0 {
            for (k, loc) in ms_loc.iter_mut().enumerate() {
                for j in 0..3 {
                    let p = sample_pattern[k * 3 + j];
                    set_field(loc, 4, j as u32 * 8, u32::from(p[0]));
                    set_field(loc, 4, j as u32 * 8 + 4, u32::from(p[1]));
                }
                set_field(loc, 8, 24, k as u32 + 1);
            }
        } else {
            ms_loc = [0x0166_6666, 0x0266_6666, 0x0366_6666, 0x0466_6666];
        }
        for loc in ms_loc {
            self.write_ras_reg(loc);
        }
        let mut coeff0 = 0u32;
        set_field(&mut coeff0, 8, 24, 0x53);
        let mut coeff1 = 0u32;
        set_field(&mut coeff1, 8, 24, 0x54);
        let v = if vf != 0 {
            *vfilter
        } else {
            [0, 0, 21, 22, 21, 0, 0]
        };
        set_field(&mut coeff0, 6, 0, u32::from(v[0]));
        set_field(&mut coeff0, 6, 6, u32::from(v[1]));
        set_field(&mut coeff0, 6, 12, u32::from(v[2]));
        set_field(&mut coeff0, 6, 18, u32::from(v[3]));
        set_field(&mut coeff1, 6, 0, u32::from(v[4]));
        set_field(&mut coeff1, 6, 6, u32::from(v[5]));
        set_field(&mut coeff1, 6, 12, u32::from(v[6]));
        self.write_ras_reg(coeff0);
        self.write_ras_reg(coeff1);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetDispCopyGamma")]
    pub fn set_disp_copy_gamma(&mut self, gamma: u32) {
        set_field(&mut self.data.cp_disp, 2, 7, gamma);
    }

    /// `GXCopyDisp`: `dest` is the address the game passes; the copy takes its physical address.
    #[doc(alias = "GXCopyDisp")]
    pub fn copy_disp(&mut self, dest: u32, clear: u8) {
        if clear != 0 {
            let mut r = self.data.zmode;
            set_field(&mut r, 1, 0, 1);
            set_field(&mut r, 3, 1, 7);
            self.write_ras_reg(r);
            let mut r = self.data.cmode0;
            set_field(&mut r, 1, 0, 0);
            set_field(&mut r, 1, 1, 0);
            self.write_ras_reg(r);
        }
        let mut change_pe_ctrl = false;
        let pe_ctrl = self.data.pe_ctrl;
        if (clear != 0 || get_field(pe_ctrl, 3, 0) == 3) && get_field(pe_ctrl, 1, 6) == 1 {
            change_pe_ctrl = true;
            let mut temp = pe_ctrl;
            set_field(&mut temp, 1, 6, 0);
            self.write_ras_reg(temp);
        }
        self.write_ras_reg(self.data.cp_disp_src);
        self.write_ras_reg(self.data.cp_disp_size);
        self.write_ras_reg(self.data.cp_disp_stride);
        let phy_addr = dest & 0x3FFF_FFFF;
        let mut r = 0u32;
        set_field(&mut r, 21, 0, phy_addr >> 5);
        set_field(&mut r, 8, 24, 0x4B);
        self.write_ras_reg(r);
        set_field(&mut self.data.cp_disp, 1, 11, u32::from(clear));
        set_field(&mut self.data.cp_disp, 1, 14, 1);
        set_field(&mut self.data.cp_disp, 8, 24, 0x52);
        self.write_ras_reg(self.data.cp_disp);
        if clear != 0 {
            self.write_ras_reg(self.data.zmode);
            self.write_ras_reg(self.data.cmode0);
        }
        if change_pe_ctrl {
            self.write_ras_reg(self.data.pe_ctrl);
        }
        self.data.bp_sent = 0;
    }

    /// `GXCopyTex`: `dest` is the address the game passes; the copy takes its physical address.
    #[doc(alias = "GXCopyTex")]
    pub fn copy_tex(&mut self, dest: u32, clear: u8) {
        if clear != 0 {
            let mut r = self.data.zmode;
            set_field(&mut r, 1, 0, 1);
            set_field(&mut r, 3, 1, 7);
            self.write_ras_reg(r);
            let mut r = self.data.cmode0;
            set_field(&mut r, 1, 0, 0);
            set_field(&mut r, 1, 1, 0);
            self.write_ras_reg(r);
        }
        let mut change_pe_ctrl = false;
        let mut temp = self.data.pe_ctrl;
        if self.data.cp_tex_z != 0 && (temp & 7) != 3 {
            change_pe_ctrl = true;
            temp = (temp & 0xFFFF_FFF8) | 3;
        }
        if (clear != 0 || (temp & 7) == 3) && ((temp >> 6) & 1) == 1 {
            change_pe_ctrl = true;
            temp &= 0xFFFF_FFBF;
        }
        if change_pe_ctrl {
            self.write_ras_reg(temp);
        }
        self.write_ras_reg(self.data.cp_tex_src);
        self.write_ras_reg(self.data.cp_tex_size);
        self.write_ras_reg(self.data.cp_tex_stride);
        let phy_addr = dest & 0x3FFF_FFFF;
        let mut r = 0u32;
        set_field(&mut r, 21, 0, phy_addr >> 5);
        set_field(&mut r, 8, 24, 0x4B);
        self.write_ras_reg(r);
        set_field(&mut self.data.cp_tex, 1, 11, u32::from(clear));
        set_field(&mut self.data.cp_tex, 1, 14, 0);
        set_field(&mut self.data.cp_tex, 8, 24, 0x52);
        self.write_ras_reg(self.data.cp_tex);
        if clear != 0 {
            self.write_ras_reg(self.data.zmode);
            self.write_ras_reg(self.data.cmode0);
        }
        if change_pe_ctrl {
            self.write_ras_reg(self.data.pe_ctrl);
        }
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXClearBoundingBox")]
    pub fn clear_bounding_box(&mut self) {
        self.write_ras_reg(0x5500_03FF);
        self.write_ras_reg(0x5600_03FF);
        self.data.bp_sent = 0;
    }

    /// `GXReadBoundingBox`: reads PE registers, not the stream; always zeros here.
    #[doc(alias = "GXReadBoundingBox")]
    pub fn read_bounding_box(&self) -> (u16, u16, u16, u16) {
        (0, 0, 0, 0)
    }
}
