//! GXBump.c: indirect texturing.

use super::enums::*;
use super::{Gx, get_field, reg, set_field};
use gekko_fp as fp;

impl Gx {
    #[doc(alias = "GXSetTevIndirect")]
    #[allow(clippy::too_many_arguments)]
    pub fn set_tev_indirect(
        &mut self,
        tev_stage: u32,
        ind_stage: u32,
        format: u32,
        bias_sel: u32,
        matrix_sel: u32,
        wrap_s: u32,
        wrap_t: u32,
        add_prev: u8,
        utc_lod: u8,
        alpha_sel: u32,
    ) {
        let mut reg = 0u32;
        set_field(&mut reg, 2, 0, ind_stage);
        set_field(&mut reg, 2, 2, format);
        set_field(&mut reg, 3, 4, bias_sel);
        set_field(&mut reg, 2, 7, alpha_sel);
        set_field(&mut reg, 4, 9, matrix_sel);
        set_field(&mut reg, 3, 13, wrap_s);
        set_field(&mut reg, 3, 16, wrap_t);
        set_field(&mut reg, 1, 19, u32::from(utc_lod));
        set_field(&mut reg, 1, 20, u32::from(add_prev));
        set_field(&mut reg, 8, 24, tev_stage + 16);
        self.write_ras_reg(reg);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetIndTexMtx")]
    pub fn set_ind_tex_mtx(&mut self, mtx_id: u32, offset: &[[f32; 3]; 2], scale_exp: i8) {
        let id = match mtx_id {
            GX_ITM_0..=GX_ITM_2 => mtx_id - 1,
            GX_ITM_S0..=GX_ITM_S2 => mtx_id - 5,
            GX_ITM_T0..=GX_ITM_T2 => mtx_id - 9,
            _ => 0,
        };
        // (int) (1024.0f * offset) & 0x7FF
        let m = |x: f32| (fp::fctiwz(fp::fmuls(1024.0, reg(x))) & 0x7FF) as u32;
        let scale_exp = scale_exp.wrapping_add(0x11) as i32;
        for col in 0..3u32 {
            let mut r = 0u32;
            set_field(&mut r, 11, 0, m(offset[0][col as usize]));
            set_field(&mut r, 11, 11, m(offset[1][col as usize]));
            set_field(&mut r, 2, 22, ((scale_exp >> (2 * col)) & 3) as u32);
            set_field(&mut r, 8, 24, id * 3 + 6 + col);
            self.write_ras_reg(r);
        }
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetIndTexCoordScale")]
    pub fn set_ind_tex_coord_scale(&mut self, ind_state: u32, scale_s: u32, scale_t: u32) {
        let g = &mut self.data;
        let reg = match ind_state {
            GX_INDTEXSTAGE0 => {
                set_field(&mut g.ind_tex_scale0, 4, 0, scale_s);
                set_field(&mut g.ind_tex_scale0, 4, 4, scale_t);
                set_field(&mut g.ind_tex_scale0, 8, 24, 0x25);
                Some(g.ind_tex_scale0)
            }
            GX_INDTEXSTAGE1 => {
                set_field(&mut g.ind_tex_scale0, 4, 8, scale_s);
                set_field(&mut g.ind_tex_scale0, 4, 12, scale_t);
                set_field(&mut g.ind_tex_scale0, 8, 24, 0x25);
                Some(g.ind_tex_scale0)
            }
            GX_INDTEXSTAGE2 => {
                set_field(&mut g.ind_tex_scale1, 4, 0, scale_s);
                set_field(&mut g.ind_tex_scale1, 4, 4, scale_t);
                set_field(&mut g.ind_tex_scale1, 8, 24, 0x26);
                Some(g.ind_tex_scale1)
            }
            GX_INDTEXSTAGE3 => {
                set_field(&mut g.ind_tex_scale1, 4, 8, scale_s);
                set_field(&mut g.ind_tex_scale1, 4, 12, scale_t);
                set_field(&mut g.ind_tex_scale1, 8, 24, 0x26);
                Some(g.ind_tex_scale1)
            }
            _ => None,
        };
        if let Some(reg) = reg {
            self.write_ras_reg(reg);
        }
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetIndTexOrder")]
    pub fn set_ind_tex_order(&mut self, ind_stage: u32, tex_coord: u32, tex_map: u32) {
        let g = &mut self.data;
        if ind_stage < 4 {
            let shift = ind_stage * 6;
            set_field(&mut g.iref, 3, shift, tex_map);
            set_field(&mut g.iref, 3, shift + 3, tex_coord);
        }
        self.write_ras_reg(self.data.iref);
        self.data.dirty_state |= 3;
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetNumIndStages")]
    pub fn set_num_ind_stages(&mut self, n_ind_stages: u8) {
        set_field(&mut self.data.gen_mode, 3, 16, u32::from(n_ind_stages));
        self.data.dirty_state |= 6;
    }

    #[doc(alias = "GXSetTevDirect")]
    pub fn set_tev_direct(&mut self, tev_stage: u32) {
        self.set_tev_indirect(
            tev_stage,
            GX_INDTEXSTAGE0,
            GX_ITF_8,
            GX_ITB_NONE,
            GX_ITM_OFF,
            GX_ITW_OFF,
            GX_ITW_OFF,
            0,
            0,
            0,
        );
    }

    #[doc(alias = "GXSetTevIndWarp")]
    pub fn set_tev_ind_warp(
        &mut self,
        tev_stage: u32,
        ind_stage: u32,
        signed_offset: u8,
        replace_mode: u8,
        matrix_sel: u32,
    ) {
        let wrap = if replace_mode != 0 {
            GX_ITW_0
        } else {
            GX_ITW_OFF
        };
        let bias = if signed_offset != 0 {
            GX_ITB_STU
        } else {
            GX_ITB_NONE
        };
        self.set_tev_indirect(
            tev_stage, ind_stage, GX_ITF_8, bias, matrix_sel, wrap, wrap, 0, 0, 0,
        );
    }

    #[doc(alias = "GXSetTevIndTile")]
    #[allow(clippy::too_many_arguments)]
    pub fn set_tev_ind_tile(
        &mut self,
        tev_stage: u32,
        ind_stage: u32,
        tilesize_s: u16,
        tilesize_t: u16,
        tilespacing_s: u16,
        tilespacing_t: u16,
        format: u32,
        matrix_sel: u32,
        bias_sel: u32,
        alpha_sel: u32,
    ) {
        let wrap = |size: u16| match size {
            256 => GX_ITW_256,
            128 => GX_ITW_128,
            64 => GX_ITW_64,
            32 => GX_ITW_32,
            16 => GX_ITW_16,
            _ => GX_ITW_OFF,
        };
        let (wrap_s, wrap_t) = (wrap(tilesize_s), wrap(tilesize_t));
        let div = |x: u16| super::mem(fp::fdivs(fp::frsp(f64::from(x)), 1024.0));
        let mtx = [
            [div(tilespacing_s), 0.0, 0.0],
            [0.0, div(tilespacing_t), 0.0],
        ];
        self.set_ind_tex_mtx(matrix_sel, &mtx, 0xA);
        self.set_tev_indirect(
            tev_stage, ind_stage, format, bias_sel, matrix_sel, wrap_s, wrap_t, 0, 1, alpha_sel,
        );
    }

    #[doc(alias = "GXSetTevIndBumpST")]
    pub fn set_tev_ind_bump_st(&mut self, tev_stage: u32, ind_stage: u32, matrix_sel: u32) {
        // An invalid matrix leaves sm and tm uninitialized in the original; here they're off.
        let (sm, tm) = match matrix_sel {
            GX_ITM_0 => (GX_ITM_S0, GX_ITM_T0),
            GX_ITM_1 => (GX_ITM_S1, GX_ITM_T1),
            GX_ITM_2 => (GX_ITM_S2, GX_ITM_T2),
            _ => (GX_ITM_OFF, GX_ITM_OFF),
        };
        self.set_tev_indirect(
            tev_stage, ind_stage, GX_ITF_8, GX_ITB_ST, sm, GX_ITW_0, GX_ITW_0, 0, 0, 0,
        );
        self.set_tev_indirect(
            tev_stage + 1,
            ind_stage,
            GX_ITF_8,
            GX_ITB_ST,
            tm,
            GX_ITW_0,
            GX_ITW_0,
            1,
            0,
            0,
        );
        self.set_tev_indirect(
            tev_stage + 2,
            ind_stage,
            GX_ITF_8,
            GX_ITB_NONE,
            GX_ITM_OFF,
            GX_ITW_OFF,
            GX_ITW_OFF,
            1,
            0,
            0,
        );
    }

    #[doc(alias = "GXSetTevIndBumpXYZ")]
    pub fn set_tev_ind_bump_xyz(&mut self, tev_stage: u32, ind_stage: u32, matrix_sel: u32) {
        self.set_tev_indirect(
            tev_stage, ind_stage, GX_ITF_8, GX_ITB_STU, matrix_sel, GX_ITW_OFF, GX_ITW_OFF, 0, 0, 0,
        );
    }

    #[doc(alias = "GXSetTevIndRepeat")]
    pub fn set_tev_ind_repeat(&mut self, tev_stage: u32) {
        self.set_tev_indirect(
            tev_stage,
            GX_INDTEXSTAGE0,
            GX_ITF_8,
            GX_ITB_NONE,
            GX_ITM_OFF,
            GX_ITW_0,
            GX_ITW_0,
            1,
            0,
            0,
        );
    }

    #[doc(alias = "__GXUpdateBPMask")]
    pub(crate) fn update_bp_mask(&mut self) {
        let g = &self.data;
        let n_ind_stages = get_field(g.gen_mode, 3, 16);
        let mut new_imask = 0u32;
        // tmap keeps its last value for stages past 3, as the machine code's register does.
        let mut tmap = 0;
        for i in 0..n_ind_stages {
            if i < 4 {
                tmap = get_field(g.iref, 3, i * 6);
            }
            new_imask |= 1 << tmap;
        }
        if g.bp_mask & 0xFF != new_imask {
            set_field(&mut self.data.bp_mask, 8, 0, new_imask);
            self.write_ras_reg(self.data.bp_mask);
            self.data.bp_sent = 0;
        }
    }

    #[doc(alias = "__GXFlushTextureState")]
    pub(crate) fn flush_texture_state(&mut self) {
        self.write_ras_reg(self.data.bp_mask);
        self.data.bp_sent = 0;
    }
}
