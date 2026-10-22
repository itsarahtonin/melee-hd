//! GXTev.c: the texture environment.

use super::enums::*;
use super::{Color, ColorS10, Gx, set_field};

impl Gx {
    #[doc(alias = "GXSetTevOp")]
    pub fn set_tev_op(&mut self, id: u32, mode: u32) {
        let (carg, aarg) = if id != GX_TEVSTAGE0 {
            (GX_CC_CPREV, GX_CA_APREV)
        } else {
            (GX_CC_RASC, GX_CA_RASA)
        };
        match mode {
            GX_MODULATE => {
                self.set_tev_color_in(id, GX_CC_ZERO, GX_CC_TEXC, carg, GX_CC_ZERO);
                self.set_tev_alpha_in(id, GX_CA_ZERO, GX_CA_TEXA, aarg, GX_CA_ZERO);
            }
            GX_DECAL => {
                self.set_tev_color_in(id, carg, GX_CC_TEXC, GX_CC_TEXA, GX_CC_ZERO);
                self.set_tev_alpha_in(id, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, aarg);
            }
            GX_BLEND => {
                self.set_tev_color_in(id, carg, GX_CC_ONE, GX_CC_TEXC, GX_CC_ZERO);
                self.set_tev_alpha_in(id, GX_CA_ZERO, GX_CA_TEXA, aarg, GX_CA_ZERO);
            }
            GX_REPLACE => {
                self.set_tev_color_in(id, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, GX_CC_TEXC);
                self.set_tev_alpha_in(id, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_TEXA);
            }
            GX_PASSCLR => {
                self.set_tev_color_in(id, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, carg);
                self.set_tev_alpha_in(id, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, aarg);
            }
            _ => {}
        }
        self.set_tev_color_op(
            id,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_TRUE,
            GX_TEVPREV,
        );
        self.set_tev_alpha_op(
            id,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_TRUE,
            GX_TEVPREV,
        );
    }

    #[doc(alias = "GXSetTevColorIn")]
    pub fn set_tev_color_in(&mut self, stage: u32, a: u32, b: u32, c: u32, d: u32) {
        let r = &mut self.data.tevc[stage as usize];
        set_field(r, 4, 12, a);
        set_field(r, 4, 8, b);
        set_field(r, 4, 4, c);
        set_field(r, 4, 0, d);
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevAlphaIn")]
    pub fn set_tev_alpha_in(&mut self, stage: u32, a: u32, b: u32, c: u32, d: u32) {
        let r = &mut self.data.teva[stage as usize];
        set_field(r, 3, 13, a);
        set_field(r, 3, 10, b);
        set_field(r, 3, 7, c);
        set_field(r, 3, 4, d);
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    /// The shared body of `GXSetTevColorOp` and `GXSetTevAlphaOp`.
    fn tev_op(r: &mut u32, op: u32, bias: u32, scale: u32, clamp: u8, out_reg: u32) {
        set_field(r, 1, 18, op & 1);
        if op <= 1 {
            set_field(r, 2, 20, scale);
            set_field(r, 2, 16, bias);
        } else {
            set_field(r, 2, 20, (op >> 1) & 3);
            set_field(r, 2, 16, 3);
        }
        set_field(r, 1, 19, u32::from(clamp));
        set_field(r, 2, 22, out_reg);
    }

    #[doc(alias = "GXSetTevColorOp")]
    pub fn set_tev_color_op(
        &mut self,
        stage: u32,
        op: u32,
        bias: u32,
        scale: u32,
        clamp: u8,
        out_reg: u32,
    ) {
        let r = &mut self.data.tevc[stage as usize];
        Self::tev_op(r, op, bias, scale, clamp, out_reg);
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevAlphaOp")]
    pub fn set_tev_alpha_op(
        &mut self,
        stage: u32,
        op: u32,
        bias: u32,
        scale: u32,
        clamp: u8,
        out_reg: u32,
    ) {
        let r = &mut self.data.teva[stage as usize];
        Self::tev_op(r, op, bias, scale, clamp, out_reg);
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    /// `GXSetTevColor`: the BG half goes three times, as the original sends it.
    #[doc(alias = "GXSetTevColor")]
    pub fn set_tev_color(&mut self, id: u32, color: Color) {
        let mut ra = 0u32;
        set_field(&mut ra, 11, 0, u32::from(color.r));
        set_field(&mut ra, 11, 12, u32::from(color.a));
        set_field(&mut ra, 8, 24, 224u32.wrapping_add(id.wrapping_mul(2)));
        let mut bg = 0u32;
        set_field(&mut bg, 11, 0, u32::from(color.b));
        set_field(&mut bg, 11, 12, u32::from(color.g));
        set_field(&mut bg, 8, 24, 225u32.wrapping_add(id.wrapping_mul(2)));
        self.write_ras_reg(ra);
        self.write_ras_reg(bg);
        self.write_ras_reg(bg);
        self.write_ras_reg(bg);
        self.data.bp_sent = 0;
        self.unset.color[(id & 3) as usize] = 0;
    }

    /// `GXSetTevColorS10`: like [`Gx::set_tev_color`], with 11-bit signed components.
    pub fn set_tev_color_s10(&mut self, id: u32, color: ColorS10) {
        let c = |v: i16| (v as u32) & 0x7FF;
        let mut ra = 0u32;
        set_field(&mut ra, 11, 0, c(color.r));
        set_field(&mut ra, 11, 12, c(color.a));
        set_field(&mut ra, 8, 24, 224u32.wrapping_add(id.wrapping_mul(2)));
        let mut bg = 0u32;
        set_field(&mut bg, 11, 0, c(color.b));
        set_field(&mut bg, 11, 12, c(color.g));
        set_field(&mut bg, 8, 24, 225u32.wrapping_add(id.wrapping_mul(2)));
        self.write_ras_reg(ra);
        self.write_ras_reg(bg);
        self.write_ras_reg(bg);
        self.write_ras_reg(bg);
        self.data.bp_sent = 0;
        self.unset.color[(id & 3) as usize] = 0;
    }

    #[doc(alias = "GXSetTevKColor")]
    pub fn set_tev_k_color(&mut self, id: u32, color: Color) {
        let mut ra = 0u32;
        set_field(&mut ra, 8, 0, u32::from(color.r));
        set_field(&mut ra, 8, 12, u32::from(color.a));
        set_field(&mut ra, 4, 20, 8);
        set_field(&mut ra, 8, 24, 224u32.wrapping_add(id.wrapping_mul(2)));
        let mut bg = 0u32;
        set_field(&mut bg, 8, 0, u32::from(color.b));
        set_field(&mut bg, 8, 12, u32::from(color.g));
        set_field(&mut bg, 4, 20, 8);
        set_field(&mut bg, 8, 24, 225u32.wrapping_add(id.wrapping_mul(2)));
        self.write_ras_reg(ra);
        self.write_ras_reg(bg);
        self.data.bp_sent = 0;
        self.unset.konst[(id & 3) as usize] = 0;
    }

    /// Marks components of the color register `id` (`GX_TEVPREV`, `GX_TEVREG0`..) just
    /// written as holding what the original left unset.
    pub fn mark_unset_color(&mut self, id: u32, comps: u8) {
        self.unset.color[(id & 3) as usize] = comps;
    }

    /// Marks components of the konst register `id` (`GX_KCOLOR0`..) just written as holding
    /// what the original left unset.
    pub fn mark_unset_konst(&mut self, id: u32, comps: u8) {
        self.unset.konst[(id & 3) as usize] = comps;
    }

    /// Counts a draw whose active stages read a component marked unset.
    #[inline]
    pub(crate) fn check_unset(&mut self) {
        if self.unset.is_empty() {
            return;
        }
        let d = &self.data;
        let used =
            melee_gx::tev::usage_of(d.gen_mode, |s| (d.tevc[s], d.teva[s]), |i| d.tev_ksel[i]);
        if used.meets(&self.unset) {
            self.unset_reads += 1;
        }
    }

    #[doc(alias = "GXSetTevKColorSel")]
    pub fn set_tev_k_color_sel(&mut self, stage: u32, sel: u32) {
        let k = &mut self.data.tev_ksel[(stage >> 1) as usize];
        if stage & 1 != 0 {
            set_field(k, 5, 14, sel);
        } else {
            set_field(k, 5, 4, sel);
        }
        let v = *k;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevKAlphaSel")]
    pub fn set_tev_k_alpha_sel(&mut self, stage: u32, sel: u32) {
        let k = &mut self.data.tev_ksel[(stage >> 1) as usize];
        if stage & 1 != 0 {
            set_field(k, 5, 19, sel);
        } else {
            set_field(k, 5, 9, sel);
        }
        let v = *k;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevSwapMode")]
    pub fn set_tev_swap_mode(&mut self, stage: u32, ras_sel: u32, tex_sel: u32) {
        let r = &mut self.data.teva[stage as usize];
        set_field(r, 2, 0, ras_sel);
        set_field(r, 2, 2, tex_sel);
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevSwapModeTable")]
    pub fn set_tev_swap_mode_table(
        &mut self,
        table: u32,
        red: u32,
        green: u32,
        blue: u32,
        alpha: u32,
    ) {
        let i = (table * 2) as usize;
        let k = &mut self.data.tev_ksel[i];
        set_field(k, 2, 0, red);
        set_field(k, 2, 2, green);
        let v = *k;
        self.write_ras_reg(v);
        let k = &mut self.data.tev_ksel[i + 1];
        set_field(k, 2, 0, blue);
        set_field(k, 2, 2, alpha);
        let v = *k;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    /// `GXSetTevClampMode`: not on this hardware; does nothing.
    #[doc(alias = "GXSetTevClampMode")]
    pub fn set_tev_clamp_mode(&mut self, _stage: u32, _mode: u32) {}

    #[doc(alias = "GXSetAlphaCompare")]
    pub fn set_alpha_compare(&mut self, comp0: u32, ref0: u8, op: u32, comp1: u32, ref1: u8) {
        let mut r = 0u32;
        set_field(&mut r, 8, 0, u32::from(ref0));
        set_field(&mut r, 8, 8, u32::from(ref1));
        set_field(&mut r, 3, 16, comp0);
        set_field(&mut r, 3, 19, comp1);
        set_field(&mut r, 2, 22, op);
        set_field(&mut r, 8, 24, 0xF3);
        self.write_ras_reg(r);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetZTexture")]
    pub fn set_z_texture(&mut self, op: u32, fmt: u32, bias: u32) {
        let mut zenv0 = 0u32;
        set_field(&mut zenv0, 24, 0, bias);
        set_field(&mut zenv0, 8, 24, 0xF4);
        let ty = match fmt {
            GX_TF_Z8 => 0,
            GX_TF_Z16 => 1,
            _ => 2,
        };
        let mut zenv1 = 0u32;
        set_field(&mut zenv1, 2, 0, ty);
        set_field(&mut zenv1, 2, 2, op);
        set_field(&mut zenv1, 8, 24, 0xF5);
        self.write_ras_reg(zenv0);
        self.write_ras_reg(zenv1);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetTevOrder")]
    pub fn set_tev_order(&mut self, stage: u32, coord: u32, map: u32, color: u32) {
        const C2R: [u32; 9] = [0, 1, 0, 1, 0, 1, 7, 5, 6];
        self.data.texmap_id[stage as usize] = map;
        let tmap = map & !0x100;
        let tmap = if tmap >= GX_MAX_TEXMAP {
            GX_TEXMAP0
        } else {
            tmap
        };
        let tcoord = if coord >= GX_MAX_TEXCOORD {
            GX_TEXCOORD0
        } else {
            coord
        };
        let chan = if color == GX_COLOR_NULL {
            7
        } else {
            C2R[color as usize]
        };
        let enable = u32::from(map != GX_TEXMAP_NULL && map & 0x100 == 0);
        let r = &mut self.data.tref[(stage / 2) as usize];
        if stage & 1 != 0 {
            set_field(r, 3, 12, tmap);
            set_field(r, 3, 15, tcoord);
            set_field(r, 3, 19, chan);
            set_field(r, 1, 18, enable);
        } else {
            set_field(r, 3, 0, tmap);
            set_field(r, 3, 3, tcoord);
            set_field(r, 3, 7, chan);
            set_field(r, 1, 6, enable);
        }
        let v = *r;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
        self.data.dirty_state |= 1;
    }

    #[doc(alias = "GXSetNumTevStages")]
    pub fn set_num_tev_stages(&mut self, n_stages: u8) {
        set_field(
            &mut self.data.gen_mode,
            4,
            10,
            u32::from(n_stages).wrapping_sub(1),
        );
        self.data.dirty_state |= 4;
    }
}
