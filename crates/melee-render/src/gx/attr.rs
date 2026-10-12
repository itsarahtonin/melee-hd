//! GXAttr.c: vertex descriptors, attribute formats, arrays and texture coordinate generation.

use super::enums::*;
use super::{Gx, get_field, set_field};

/// `GXVtxDescList`: one entry of `GXSetVtxDescv`'s list, which ends at `GX_VA_NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VtxDescList {
    pub attr: u32,
    pub ty: u32,
}

/// `GXVtxAttrFmtList`: one entry of `GXSetVtxAttrFmtv`'s list, which ends at `GX_VA_NULL`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VtxAttrFmtList {
    pub attr: u32,
    pub cnt: u32,
    pub ty: u32,
    pub frac: u8,
}

impl Gx {
    #[doc(alias = "__GXXfVtxSpecs")]
    fn xf_vtx_specs(&mut self) {
        let g = &self.data;
        let mut n_cols = u32::from(get_field(g.vcd_lo, 2, 13) != 0);
        n_cols += u32::from(get_field(g.vcd_lo, 2, 15) != 0);
        let n_nrm = if g.has_bi_nrms != 0 {
            2
        } else if g.has_nrms != 0 {
            1
        } else {
            0
        };
        let n_tex = (0..8)
            .map(|i| u32::from(get_field(g.vcd_hi, 2, i * 2) != 0))
            .sum::<u32>();
        let reg = n_cols | (n_nrm << 2) | (n_tex << 4);
        self.write_xf_reg(8, reg);
        self.data.bp_sent = 1;
    }

    /// `SETVCDATTR`.
    fn set_vcd_attr(&mut self, attr: u32, ty: u32) {
        let g = &mut self.data;
        match attr {
            GX_VA_PNMTXIDX..=GX_VA_TEX7MTXIDX => set_field(&mut g.vcd_lo, 1, attr, ty),
            GX_VA_POS => set_field(&mut g.vcd_lo, 2, 9, ty),
            GX_VA_NRM => {
                if ty != GX_NONE {
                    g.has_nrms = 1;
                    g.has_bi_nrms = 0;
                    g.nrm_type = ty;
                } else {
                    g.has_nrms = 0;
                }
            }
            GX_VA_NBT => {
                if ty != GX_NONE {
                    g.has_bi_nrms = 1;
                    g.has_nrms = 0;
                    g.nrm_type = ty;
                } else {
                    g.has_bi_nrms = 0;
                }
            }
            GX_VA_CLR0 => set_field(&mut g.vcd_lo, 2, 13, ty),
            GX_VA_CLR1 => set_field(&mut g.vcd_lo, 2, 15, ty),
            GX_VA_TEX0..=GX_VA_TEX7 => set_field(&mut g.vcd_hi, 2, (attr - GX_VA_TEX0) * 2, ty),
            _ => {}
        }
    }

    /// The normal's type into the descriptor, after `SETVCDATTR`.
    fn set_vcd_nrm(&mut self) {
        let g = &mut self.data;
        if g.has_nrms != 0 || g.has_bi_nrms != 0 {
            set_field(&mut g.vcd_lo, 2, 11, g.nrm_type);
        } else {
            set_field(&mut g.vcd_lo, 2, 11, 0);
        }
        g.dirty_state |= 8;
    }

    #[doc(alias = "GXSetVtxDesc")]
    pub fn set_vtx_desc(&mut self, attr: u32, ty: u32) {
        self.set_vcd_attr(attr, ty);
        self.set_vcd_nrm();
    }

    /// `GXSetVtxDescv`: the list ends at a `GX_VA_NULL` entry or the slice's end.
    #[doc(alias = "GXSetVtxDescv")]
    pub fn set_vtx_descv(&mut self, list: &[VtxDescList]) {
        for d in list.iter().take_while(|d| d.attr != GX_VA_NULL) {
            self.set_vcd_attr(d.attr, d.ty);
        }
        self.set_vcd_nrm();
    }

    #[doc(alias = "__GXSetVCD")]
    pub(crate) fn set_vcd(&mut self) {
        const TBL1: [u32; 4] = [0, 4, 1, 2];
        const TBL2: [u32; 4] = [0, 8, 1, 2];
        const TBL3: [u32; 4] = [0, 12, 1, 2];
        self.write_cp_reg(0x50, self.data.vcd_lo);
        self.write_cp_reg(0x60, self.data.vcd_hi);
        self.xf_vtx_specs();
        let g = &mut self.data;
        if g.v_num != 0 {
            let vl = g.vcd_lo;
            let vh = g.vcd_hi;
            let mut vlm = (0..9).map(|i| get_field(vl, 1, i)).sum::<u32>();
            vlm += TBL3[get_field(vl, 2, 9) as usize];
            let b = (u32::from(g.has_bi_nrms) << 1) + 1;
            vlm += TBL3[get_field(vl, 2, 11) as usize] * b;
            vlm += TBL1[get_field(vl, 2, 13) as usize];
            vlm += TBL1[get_field(vl, 2, 15) as usize];
            for i in 0..8 {
                vlm += TBL2[get_field(vh, 2, i * 2) as usize];
            }
            g.v_lim = vlm as u16;
        }
    }

    /// `GXGetVtxDesc`.
    #[doc(alias = "GXGetVtxDesc")]
    pub fn get_vtx_desc(&self, attr: u32) -> u32 {
        let g = &self.data;
        match attr {
            GX_VA_PNMTXIDX..=GX_VA_TEX7MTXIDX => get_field(g.vcd_lo, 1, attr),
            GX_VA_POS => get_field(g.vcd_lo, 2, 9),
            GX_VA_NRM if g.has_nrms != 0 => get_field(g.vcd_lo, 2, 11),
            GX_VA_NBT if g.has_bi_nrms != 0 => get_field(g.vcd_lo, 2, 11),
            GX_VA_CLR0 => get_field(g.vcd_lo, 2, 13),
            GX_VA_CLR1 => get_field(g.vcd_lo, 2, 15),
            GX_VA_TEX0..=GX_VA_TEX7 => get_field(g.vcd_hi, 2, (attr - GX_VA_TEX0) * 2),
            _ => 0,
        }
    }

    /// `GXGetVtxDescv`: every attribute's type, then the `GX_VA_NULL` end.
    #[doc(alias = "GXGetVtxDescv")]
    pub fn get_vtx_descv(&self) -> Vec<VtxDescList> {
        let mut v: Vec<_> = (0..GX_VA_MAX_ATTR)
            .map(|attr| VtxDescList {
                attr,
                ty: self.get_vtx_desc(attr),
            })
            .collect();
        v.push(VtxDescList {
            attr: GX_VA_NULL,
            ty: 0,
        });
        v
    }

    #[doc(alias = "GXClearVtxDesc")]
    pub fn clear_vtx_desc(&mut self) {
        let g = &mut self.data;
        g.vcd_lo = 0;
        set_field(&mut g.vcd_lo, 2, 9, 1);
        g.vcd_hi = 0;
        g.has_nrms = 0;
        g.has_bi_nrms = 0;
        g.dirty_state |= 8;
    }

    /// `SETVAT`.
    fn set_vat_attr(&mut self, fmt: usize, attr: u32, cnt: u32, ty: u32, shft: u8) {
        let g = &mut self.data;
        let (va, vb, vc) = (&mut g.vat_a[fmt], &mut g.vat_b[fmt], &mut g.vat_c[fmt]);
        let shft = u32::from(shft);
        match attr {
            GX_VA_POS => {
                set_field(va, 1, 0, cnt);
                set_field(va, 3, 1, ty);
                set_field(va, 5, 4, shft);
            }
            GX_VA_NRM | GX_VA_NBT => {
                set_field(va, 3, 10, ty);
                if cnt == GX_NRM_NBT3 {
                    set_field(va, 1, 9, 1);
                    set_field(va, 1, 31, 1);
                } else {
                    set_field(va, 1, 9, cnt);
                    set_field(va, 1, 31, 0);
                }
            }
            GX_VA_CLR0 => {
                set_field(va, 1, 13, cnt);
                set_field(va, 3, 14, ty);
            }
            GX_VA_CLR1 => {
                set_field(va, 1, 17, cnt);
                set_field(va, 3, 18, ty);
            }
            GX_VA_TEX0 => {
                set_field(va, 1, 21, cnt);
                set_field(va, 3, 22, ty);
                set_field(va, 5, 25, shft);
            }
            GX_VA_TEX1 => {
                set_field(vb, 1, 0, cnt);
                set_field(vb, 3, 1, ty);
                set_field(vb, 5, 4, shft);
            }
            GX_VA_TEX2 => {
                set_field(vb, 1, 9, cnt);
                set_field(vb, 3, 10, ty);
                set_field(vb, 5, 13, shft);
            }
            GX_VA_TEX3 => {
                set_field(vb, 1, 18, cnt);
                set_field(vb, 3, 19, ty);
                set_field(vb, 5, 22, shft);
            }
            GX_VA_TEX4 => {
                set_field(vb, 1, 27, cnt);
                set_field(vb, 3, 28, ty);
                set_field(vc, 5, 0, shft);
            }
            GX_VA_TEX5 => {
                set_field(vc, 1, 5, cnt);
                set_field(vc, 3, 6, ty);
                set_field(vc, 5, 9, shft);
            }
            GX_VA_TEX6 => {
                set_field(vc, 1, 14, cnt);
                set_field(vc, 3, 15, ty);
                set_field(vc, 5, 18, shft);
            }
            GX_VA_TEX7 => {
                set_field(vc, 1, 23, cnt);
                set_field(vc, 3, 24, ty);
                set_field(vc, 5, 27, shft);
            }
            _ => {}
        }
    }

    #[doc(alias = "GXSetVtxAttrFmt")]
    pub fn set_vtx_attr_fmt(&mut self, vtxfmt: u32, attr: u32, cnt: u32, ty: u32, frac: u8) {
        self.set_vat_attr(vtxfmt as usize, attr, cnt, ty, frac);
        self.data.dirty_state |= 0x10;
        self.data.dirty_vat |= 1u8 << (vtxfmt as u8);
    }

    /// `GXSetVtxAttrFmtv`: the list ends at a `GX_VA_NULL` entry or the slice's end.
    #[doc(alias = "GXSetVtxAttrFmtv")]
    pub fn set_vtx_attr_fmtv(&mut self, vtxfmt: u32, list: &[VtxAttrFmtList]) {
        for l in list.iter().take_while(|l| l.attr != GX_VA_NULL) {
            self.set_vat_attr(vtxfmt as usize, l.attr, l.cnt, l.ty, l.frac);
        }
        self.data.dirty_state |= 0x10;
        self.data.dirty_vat |= 1u8 << (vtxfmt as u8);
    }

    #[doc(alias = "__GXSetVAT")]
    pub(crate) fn set_vat(&mut self) {
        for i in 0..8u8 {
            if self.data.dirty_vat & (1 << i) != 0 {
                let i_ = usize::from(i);
                self.write_cp_reg(i | 0x70, self.data.vat_a[i_]);
                self.write_cp_reg(i | 0x80, self.data.vat_b[i_]);
                self.write_cp_reg(i | 0x90, self.data.vat_c[i_]);
            }
        }
        self.data.dirty_vat = 0;
    }

    /// `GXGetVtxAttrFmt`: the attribute's (count, type, frac).
    #[doc(alias = "GXGetVtxAttrFmt")]
    pub fn get_vtx_attr_fmt(&self, fmt: u32, attr: u32) -> (u32, u32, u8) {
        let g = &self.data;
        let (va, vb, vc) = (
            g.vat_a[fmt as usize],
            g.vat_b[fmt as usize],
            g.vat_c[fmt as usize],
        );
        let f = |r: u32, s: u32| ((r >> s) & 0x1F) as u8;
        match attr {
            GX_VA_POS => (get_field(va, 1, 0), get_field(va, 3, 1), f(va, 4)),
            GX_VA_NRM | GX_VA_NBT => {
                let mut cnt = get_field(va, 1, 9);
                if cnt == GX_TEX_ST && (va >> 31) as u8 != 0 {
                    cnt = GX_NRM_NBT3;
                }
                (cnt, get_field(va, 3, 10), 0)
            }
            GX_VA_CLR0 => (get_field(va, 1, 13), get_field(va, 3, 14), 0),
            GX_VA_CLR1 => (get_field(va, 1, 17), get_field(va, 3, 18), 0),
            GX_VA_TEX0 => (get_field(va, 1, 21), get_field(va, 3, 22), f(va, 25)),
            GX_VA_TEX1 => (get_field(vb, 1, 0), get_field(vb, 3, 1), f(vb, 4)),
            GX_VA_TEX2 => (get_field(vb, 1, 9), get_field(vb, 3, 10), f(vb, 13)),
            GX_VA_TEX3 => (get_field(vb, 1, 18), get_field(vb, 3, 19), f(vb, 22)),
            GX_VA_TEX4 => (get_field(vb, 1, 27), get_field(vb, 3, 28), f(vc, 0)),
            GX_VA_TEX5 => (get_field(vc, 1, 5), get_field(vc, 3, 6), f(vc, 9)),
            GX_VA_TEX6 => (get_field(vc, 1, 14), get_field(vc, 3, 15), f(vc, 18)),
            GX_VA_TEX7 => (get_field(vc, 1, 23), get_field(vc, 3, 24), (vc >> 27) as u8),
            _ => (GX_TEX_ST, GX_RGB565, 0),
        }
    }

    /// `GXGetVtxAttrFmtv`: every attribute from `GX_VA_POS` on, then the `GX_VA_NULL` end.
    #[doc(alias = "GXGetVtxAttrFmtv")]
    pub fn get_vtx_attr_fmtv(&self, fmt: u32) -> Vec<VtxAttrFmtList> {
        let mut v: Vec<_> = (GX_VA_POS..GX_VA_MAX_ATTR)
            .map(|attr| {
                let (cnt, ty, frac) = self.get_vtx_attr_fmt(fmt, attr);
                VtxAttrFmtList {
                    attr,
                    cnt,
                    ty,
                    frac,
                }
            })
            .collect();
        v.push(VtxAttrFmtList {
            attr: GX_VA_NULL,
            ..Default::default()
        });
        v
    }

    /// `GXSetArray`: `base_ptr` is the address the game passes; the SDK writes its physical
    /// address (`& 0x3FFFFFFF`).
    #[doc(alias = "GXSetArray")]
    pub fn set_array(&mut self, attr: u32, base_ptr: u32, stride: u8) {
        let attr = if attr == GX_VA_NBT { GX_VA_NRM } else { attr };
        let cp_attr = attr.wrapping_sub(GX_VA_POS);
        let phy_addr = base_ptr & 0x3FFF_FFFF;
        // GX_WRITE_SOME_REG2 and 3 keep the light, matrix arrays' (12-15) base and stride.
        self.write_cp_reg((cp_attr | 0xA0) as u8, phy_addr);
        let slot = cp_attr.wrapping_sub(12) as i32;
        if (0..4).contains(&slot) {
            self.data.index_base[slot as usize] = phy_addr;
        }
        self.write_cp_reg((cp_attr | 0xB0) as u8, u32::from(stride));
        if (0..4).contains(&slot) {
            self.data.index_stride[slot as usize] = u32::from(stride);
        }
    }

    #[doc(alias = "GXInvalidateVtxCache")]
    pub fn invalidate_vtx_cache(&mut self) {
        self.write_u8(0x48);
    }

    /// `GXSetTexCoordGen2`. ssbm-rs transliterates it from its machine code (for an unused
    /// variable of the C); the instructions do what the C says.
    #[doc(alias = "GXSetTexCoordGen2")]
    pub fn set_tex_coord_gen2(
        &mut self,
        dst_coord: u32,
        func: u32,
        src_param: u32,
        mtx: u32,
        normalize: u8,
        pt_texmtx: u32,
    ) {
        let mut reg = 0u32;
        let mut form = 0;
        let row = match src_param {
            GX_TG_POS => {
                form = 1;
                0
            }
            GX_TG_NRM => {
                form = 1;
                1
            }
            GX_TG_BINRM => {
                form = 1;
                3
            }
            GX_TG_TANGENT => {
                form = 1;
                4
            }
            GX_TG_COLOR0 | GX_TG_COLOR1 => 2,
            GX_TG_TEX0..=GX_TG_TEX7 => 5 + (src_param - GX_TG_TEX0),
            _ => 5,
        };
        match func {
            GX_TG_MTX2X4 => {
                set_field(&mut reg, 1, 1, 0);
                set_field(&mut reg, 1, 2, form);
                set_field(&mut reg, 3, 4, 0);
                set_field(&mut reg, 5, 7, row);
            }
            GX_TG_MTX3X4 => {
                set_field(&mut reg, 1, 1, 1);
                set_field(&mut reg, 1, 2, form);
                set_field(&mut reg, 3, 4, 0);
                set_field(&mut reg, 5, 7, row);
            }
            GX_TG_BUMP0..=GX_TG_BUMP7 => {
                set_field(&mut reg, 1, 1, 0);
                set_field(&mut reg, 1, 2, form);
                set_field(&mut reg, 3, 4, 1);
                set_field(&mut reg, 5, 7, row);
                set_field(&mut reg, 3, 12, src_param.wrapping_sub(12));
                set_field(&mut reg, 3, 15, func - 2);
            }
            GX_TG_SRTG => {
                set_field(&mut reg, 1, 1, 0);
                set_field(&mut reg, 1, 2, form);
                if src_param == GX_TG_COLOR0 {
                    set_field(&mut reg, 3, 4, 2);
                } else {
                    set_field(&mut reg, 3, 4, 3);
                }
                set_field(&mut reg, 5, 7, 2);
            }
            _ => {}
        }
        self.write_xf_reg(dst_coord + 0x40, reg);
        let mut reg = 0u32;
        set_field(&mut reg, 6, 0, pt_texmtx.wrapping_sub(64));
        set_field(&mut reg, 1, 8, u32::from(normalize));
        self.write_xf_reg(dst_coord + 0x50, reg);
        let g = &mut self.data;
        match dst_coord {
            GX_TEXCOORD0 => set_field(&mut g.mat_idx_a, 6, 6, mtx),
            GX_TEXCOORD1 => set_field(&mut g.mat_idx_a, 6, 12, mtx),
            GX_TEXCOORD2 => set_field(&mut g.mat_idx_a, 6, 18, mtx),
            GX_TEXCOORD3 => set_field(&mut g.mat_idx_a, 6, 24, mtx),
            GX_TEXCOORD4 => set_field(&mut g.mat_idx_b, 6, 0, mtx),
            GX_TEXCOORD5 => set_field(&mut g.mat_idx_b, 6, 6, mtx),
            GX_TEXCOORD6 => set_field(&mut g.mat_idx_b, 6, 12, mtx),
            _ => set_field(&mut g.mat_idx_b, 6, 18, mtx),
        }
        self.set_matrix_index(dst_coord + 1);
    }

    /// `GXSetTexCoordGen` (GXGeometry.h's inline): no normalization, the identity
    /// post-transform matrix.
    #[doc(alias = "GXSetTexCoordGen")]
    pub fn set_tex_coord_gen(&mut self, dst_coord: u32, func: u32, src_param: u32, mtx: u32) {
        self.set_tex_coord_gen2(dst_coord, func, src_param, mtx, GX_FALSE, GX_PTIDENTITY);
    }

    #[doc(alias = "GXSetNumTexGens")]
    pub fn set_num_tex_gens(&mut self, n_tex_gens: u8) {
        set_field(&mut self.data.gen_mode, 4, 0, u32::from(n_tex_gens));
        self.write_xf_reg(0x3F, u32::from(n_tex_gens));
        self.data.dirty_state |= 4;
    }
}
