//! GXInit.c: `GXInit` and `__GXInitGX`, and the default region callbacks.

use super::enums::*;
use super::frame_buf::{GX_MPAL_480_INT_DF, GX_NTSC_480_INT_DF, GX_PAL_528_INT_DF};
use super::texture::{TexObj, TexRegionId};
use super::{Color, Gx, GxData, set_field};

/// `VI_NTSC`, one of `VIGetTvFormat`'s answers.
pub const VI_NTSC: u32 = 0;
/// `VI_PAL`.
pub const VI_PAL: u32 = 1;
/// `VI_MPAL`.
pub const VI_MPAL: u32 = 2;

/// `__OSBusClock` on a GameCube: 162 MHz.
pub const BUS_CLOCK: u32 = 162_000_000;

/// `__GXDefaultTexRegionCallback`: the 8 regions in turn, the 4 CI regions in turn for color
/// indexed textures.
pub(crate) fn default_tex_region_callback(g: &mut GxData, t_obj: &TexObj, _id: u32) -> TexRegionId {
    let fmt = t_obj.fmt;
    if fmt != GX_TF_C4 && fmt != GX_TF_C8 && fmt != GX_TF_C14X2 {
        let i = g.next_tex_rgn & 7;
        g.next_tex_rgn = g.next_tex_rgn.wrapping_add(1);
        TexRegionId::Normal(i as usize)
    } else {
        let i = g.next_tex_rgn_ci & 3;
        g.next_tex_rgn_ci = g.next_tex_rgn_ci.wrapping_add(1);
        TexRegionId::Ci(i as usize)
    }
}

/// `__GXDefaultTlutRegionCallback`.
pub(crate) fn default_tlut_region_callback(idx: u32) -> Option<usize> {
    (idx < 0x14).then_some(idx as usize)
}

impl Gx {
    /// `GXInit`, after [`Gx::new`]: the shadow state and the commands a freshly initialized SDK
    /// sends. The FIFO objects (`GXInitFifoBase`, `GXSetCPUFifo`, `GXSetGPFifo`), the PE
    /// interrupt handlers (`__GXPEInit`) and the write-gather pipe are hardware setup with no
    /// command of their own, and aren't ported.
    #[doc(alias = "GXInit")]
    pub fn init(&mut self) {
        let g = &mut self.data;
        g.in_disp_list = 0;
        g.dl_save_context = 1;
        g.tcs_man_enab = 0;
        g.v_num = 0;
        g.gen_mode = 0;
        set_field(&mut g.gen_mode, 8, 24, 0);
        g.bp_mask = 255;
        set_field(&mut g.bp_mask, 8, 24, 0x0F);
        g.lp_size = 0;
        set_field(&mut g.lp_size, 8, 24, 0x22);
        for i in 0..16u32 {
            let (iu, h) = (i as usize, (i / 2) as usize);
            g.tevc[iu] = 0;
            g.teva[iu] = 0;
            g.tref[h] = 0;
            g.texmap_id[iu] = GX_TEXMAP_NULL;
            set_field(&mut g.tevc[iu], 8, 24, 0xC0 + i * 2);
            set_field(&mut g.teva[iu], 8, 24, 0xC1 + i * 2);
            set_field(&mut g.tev_ksel[h], 8, 24, 0xF6 + i / 2);
            set_field(&mut g.tref[h], 8, 24, 0x28 + i / 2);
        }
        g.iref = 0;
        set_field(&mut g.iref, 8, 24, 0x27);
        for i in 0..8u32 {
            let iu = i as usize;
            g.su_ts0[iu] = 0;
            g.su_ts1[iu] = 0;
            set_field(&mut g.su_ts0[iu], 8, 24, 0x30 + i * 2);
            set_field(&mut g.su_ts1[iu], 8, 24, 0x31 + i * 2);
        }
        set_field(&mut g.su_scis0, 8, 24, 0x20);
        set_field(&mut g.su_scis1, 8, 24, 0x21);
        set_field(&mut g.cmode0, 8, 24, 0x41);
        set_field(&mut g.cmode1, 8, 24, 0x42);
        set_field(&mut g.zmode, 8, 24, 0x40);
        set_field(&mut g.pe_ctrl, 8, 24, 0x43);
        set_field(&mut g.cp_tex, 2, 7, 0);
        g.dirty_state = 0;
        g.dirty_vat = 0;

        let freq_base = self.bus_clock / 500;
        self.flush_texture_state();
        self.write_ras_reg((freq_base >> 11) | 0x400 | 0x6900_0000);
        self.flush_texture_state();
        self.write_ras_reg((freq_base / 0x1080) | 0x200 | 0x4600_0000);

        for i in 0..8usize {
            set_field(&mut self.data.vat_a[i], 1, 30, 1);
            set_field(&mut self.data.vat_b[i], 1, 31, 1);
            self.write_cp_reg(i as u8 | 0x80, self.data.vat_b[i]);
        }
        {
            let mut reg1 = 0u32;
            for bit in 0..6 {
                set_field(&mut reg1, 1, bit, 1);
            }
            self.write_xf_reg(0, reg1);
            let mut reg2 = 0u32;
            set_field(&mut reg2, 1, 0, 1);
            self.write_xf_reg(0x12, reg2);
        }
        {
            let mut reg = 0u32;
            for bit in 0..4 {
                set_field(&mut reg, 1, bit, 1);
            }
            set_field(&mut reg, 8, 24, 0x58);
            self.write_ras_reg(reg);
        }

        for i in 0..8u32 {
            let mut r = self.data.tex_regions[i as usize];
            self.init_tex_cache_region(&mut r, 0, i * 0x8000, 0, 0x80000 + i * 0x8000, 0);
            self.data.tex_regions[i as usize] = r;
        }
        for i in 0..4u32 {
            let mut r = self.data.tex_regions_ci[i as usize];
            self.init_tex_cache_region(&mut r, 0, (i * 2 + 8) * 0x8000, 0, (i * 2 + 9) * 0x8000, 0);
            self.data.tex_regions_ci[i as usize] = r;
        }
        for i in 0..16u32 {
            let mut r = self.data.tlut_regions[i as usize];
            self.init_tlut_region(&mut r, 0xC0000 + i * 0x2000, 16);
            self.data.tlut_regions[i as usize] = r;
        }
        for i in 0..4u32 {
            let mut r = self.data.tlut_regions[i as usize + 16];
            self.init_tlut_region(&mut r, 0xE0000 + i * 0x8000, 64);
            self.data.tlut_regions[i as usize + 16] = r;
        }

        // __cpReg[3] = 0: a CP register, not through the FIFO.
        set_field(&mut self.data.perf_sel, 4, 4, 0);
        self.write_cp_reg(0x20, self.data.perf_sel);
        self.write_xf_reg(6, 0);
        self.write_ras_reg(0x2300_0000);
        self.write_ras_reg(0x2400_0000);
        self.write_ras_reg(0x6700_0000);
        self.set_tmem_config(0);
        self.init_gx();
    }

    /// `__GXInitGX`: the default state, for the render mode of the console's TV format.
    #[doc(alias = "__GXInitGX")]
    pub(crate) fn init_gx(&mut self) {
        let clear = Color::new(64, 64, 64, 255);
        let black = Color::new(0, 0, 0, 0);
        let white = Color::new(255, 255, 255, 255);
        let rmode = match self.tv_format {
            VI_PAL => &GX_PAL_528_INT_DF,
            VI_MPAL => &GX_MPAL_480_INT_DF,
            _ => &GX_NTSC_480_INT_DF,
        };
        self.set_copy_clear(clear, 0xFF_FFFF);
        for i in 0..8 {
            self.set_tex_coord_gen(GX_TEXCOORD0 + i, GX_TG_MTX2X4, GX_TG_TEX0 + i, GX_IDENTITY);
        }
        self.set_num_tex_gens(1);
        self.clear_vtx_desc();
        self.invalidate_vtx_cache();
        self.set_line_width(6, GX_TO_ZERO);
        self.set_point_size(6, GX_TO_ZERO);
        for i in 0..8 {
            self.enable_tex_offsets(GX_TEXCOORD0 + i, 0, 0);
        }
        let identity_mtx = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        self.load_pos_mtx_imm(&identity_mtx, GX_PNMTX0);
        self.load_nrm_mtx_imm(&identity_mtx, GX_PNMTX0);
        self.set_current_mtx(GX_PNMTX0);
        self.load_tex_mtx_imm(&identity_mtx, GX_IDENTITY, GX_MTX3X4);
        self.load_tex_mtx_imm(&identity_mtx, GX_PTIDENTITY, GX_MTX3X4);
        self.set_viewport(
            0.0,
            0.0,
            f32::from(rmode.fb_width),
            f32::from(rmode.xfb_height),
            0.0,
            1.0,
        );
        self.set_co_planar(GX_DISABLE);
        self.set_cull_mode(GX_CULL_BACK);
        self.set_clip_mode(GX_CLIP_ENABLE);
        self.set_scissor(0, 0, u32::from(rmode.fb_width), u32::from(rmode.efb_height));
        self.set_scissor_box_offset(0, 0);
        self.set_num_chans(0);
        self.set_chan_ctrl(
            GX_COLOR0A0,
            GX_DISABLE,
            GX_SRC_REG,
            GX_SRC_VTX,
            GX_LIGHT_NULL,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        self.set_chan_amb_color(GX_COLOR0A0, black);
        self.set_chan_mat_color(GX_COLOR0A0, white);
        self.set_chan_ctrl(
            GX_COLOR1A1,
            GX_DISABLE,
            GX_SRC_REG,
            GX_SRC_VTX,
            GX_LIGHT_NULL,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        self.set_chan_amb_color(GX_COLOR1A1, black);
        self.set_chan_mat_color(GX_COLOR1A1, white);
        self.invalidate_tex_all();

        self.data.next_tex_rgn = 0;
        self.data.next_tex_rgn_ci = 0;
        self.set_tex_region_callback(default_tex_region_callback);
        self.set_tlut_region_callback(default_tlut_region_callback);
        for i in 0..8 {
            self.set_tev_order(
                GX_TEVSTAGE0 + i,
                GX_TEXCOORD0 + i,
                GX_TEXMAP0 + i,
                GX_COLOR0A0,
            );
        }
        for i in 8..16 {
            self.set_tev_order(
                GX_TEVSTAGE0 + i,
                GX_TEXCOORD_NULL,
                GX_TEXMAP_NULL,
                GX_COLOR_NULL,
            );
        }
        self.set_num_tev_stages(1);
        self.set_tev_op(GX_TEVSTAGE0, GX_REPLACE);
        self.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_AND, GX_ALWAYS, 0);
        self.set_z_texture(GX_ZT_DISABLE, GX_TF_Z8, 0);
        for i in GX_TEVSTAGE0..GX_MAX_TEVSTAGE {
            self.set_tev_k_color_sel(i, GX_TEV_KCSEL_1_4);
            self.set_tev_k_alpha_sel(i, GX_TEV_KASEL_1);
            self.set_tev_swap_mode(i, GX_TEV_SWAP0, GX_TEV_SWAP0);
        }
        self.set_tev_swap_mode_table(
            GX_TEV_SWAP0,
            GX_CH_RED,
            GX_CH_GREEN,
            GX_CH_BLUE,
            GX_CH_ALPHA,
        );
        self.set_tev_swap_mode_table(GX_TEV_SWAP1, GX_CH_RED, GX_CH_RED, GX_CH_RED, GX_CH_ALPHA);
        self.set_tev_swap_mode_table(
            GX_TEV_SWAP2,
            GX_CH_GREEN,
            GX_CH_GREEN,
            GX_CH_GREEN,
            GX_CH_ALPHA,
        );
        self.set_tev_swap_mode_table(
            GX_TEV_SWAP3,
            GX_CH_BLUE,
            GX_CH_BLUE,
            GX_CH_BLUE,
            GX_CH_ALPHA,
        );
        for i in GX_TEVSTAGE0..GX_MAX_TEVSTAGE {
            self.set_tev_direct(i);
        }
        self.set_num_ind_stages(0);
        for i in 0..4 {
            self.set_ind_tex_coord_scale(GX_INDTEXSTAGE0 + i, GX_ITS_1, GX_ITS_1);
        }
        self.set_fog(GX_FOG_NONE, 0.0, 1.0, 0.1, 1.0, black);
        self.set_fog_range_adj(GX_DISABLE, 0, None);
        self.set_blend_mode(GX_BM_NONE, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        self.set_color_update(GX_ENABLE);
        self.set_alpha_update(GX_ENABLE);
        self.set_z_mode(GX_TRUE, GX_LEQUAL, GX_TRUE);
        self.set_z_comp_loc(GX_TRUE);
        self.set_dither(GX_ENABLE);
        self.set_dst_alpha(GX_DISABLE, 0);
        self.set_pixel_fmt(GX_PF_RGB8_Z24, GX_ZC_LINEAR);
        self.set_field_mask(GX_ENABLE, GX_ENABLE);
        let half = if u32::from(rmode.vi_height) == 2 * u32::from(rmode.xfb_height) {
            GX_ENABLE
        } else {
            GX_DISABLE
        };
        self.set_field_mode(rmode.field_rendering, half);
        self.set_disp_copy_src(0, 0, rmode.fb_width, rmode.efb_height);
        self.set_disp_copy_dst(rmode.fb_width, rmode.efb_height);
        let yscale = super::mem(gekko_fp::fdivs(
            f64::from(rmode.xfb_height),
            f64::from(rmode.efb_height),
        ));
        self.set_disp_copy_y_scale(yscale);
        self.set_copy_clamp(GX_CLAMP_TOP | GX_CLAMP_BOTTOM);
        self.set_copy_filter(rmode.aa, &rmode.sample_pattern, GX_TRUE, &rmode.vfilter);
        self.set_disp_copy_gamma(GX_GM_1_0);
        self.set_disp_copy_frame2_field(GX_COPY_PROGRESSIVE);
        self.clear_bounding_box();

        self.poke_color_update(GX_TRUE);
        self.poke_alpha_update(GX_TRUE);
        self.poke_dither(GX_FALSE);
        self.poke_blend_mode(GX_BM_NONE, GX_BL_ZERO, GX_BL_ONE, GX_LO_SET);
        self.poke_alpha_mode(GX_ALWAYS, 0);
        self.poke_alpha_read(GX_READ_FF);
        self.poke_dst_alpha(GX_DISABLE, 0);
        self.poke_z_mode(GX_TRUE, GX_ALWAYS, GX_TRUE);
        self.set_gp_metric(GX_PERF0_NONE, GX_PERF1_NONE);
        self.clear_gp_metric();
    }
}
