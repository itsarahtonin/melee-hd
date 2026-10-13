//! GXGeometry.c: primitives, dirty state, line and point size, culling.

use super::enums::*;
use super::{Gx, get_field, set_field};

impl Gx {
    #[doc(alias = "__GXSetDirtyState")]
    pub(crate) fn set_dirty_state(&mut self) {
        if self.data.dirty_state & 1 != 0 {
            self.set_su_tex_regs();
        }
        if self.data.dirty_state & 2 != 0 {
            self.update_bp_mask();
        }
        if self.data.dirty_state & 4 != 0 {
            self.set_gen_mode();
        }
        if self.data.dirty_state & 8 != 0 {
            self.set_vcd();
        }
        if self.data.dirty_state & 0x10 != 0 {
            self.set_vat();
        }
        self.data.dirty_state = 0;
    }

    /// `GXBegin`: the vertices follow through the vertex writers ([`Gx::position3f32`] and
    /// the rest).
    #[doc(alias = "GXBegin")]
    pub fn begin(&mut self, ty: u32, vtxfmt: u32, nverts: u16) {
        if self.data.dirty_state != 0 {
            self.set_dirty_state();
        }
        // *(u32*) &gx->unk == 0: both unk and bpSent.
        if self.data.unk == 0 && self.data.bp_sent == 0 {
            self.send_flush_prim();
        }
        self.write_u8((vtxfmt | ty) as u8);
        self.write_u16(nverts);
    }

    /// `GXEnd`: only checks, in a debug build.
    #[inline]
    pub fn end(&mut self) {}

    #[doc(alias = "__GXSendFlushPrim")]
    pub(crate) fn send_flush_prim(&mut self) {
        let num_d = u32::from(self.data.v_num) * u32::from(self.data.v_lim);
        self.write_u8(0x98);
        self.write_u16(self.data.v_num);
        for _ in (0..num_d).step_by(4) {
            self.write_u32(0);
        }
        self.data.bp_sent = 1;
    }

    #[doc(alias = "GXSetLineWidth")]
    pub fn set_line_width(&mut self, width: u8, tex_offsets: u32) {
        set_field(&mut self.data.lp_size, 8, 0, u32::from(width));
        set_field(&mut self.data.lp_size, 3, 16, tex_offsets);
        self.write_ras_reg(self.data.lp_size);
        self.data.bp_sent = 0;
    }

    /// `GXGetLineWidth`: (width, texOffsets).
    pub fn get_line_width(&self) -> (u8, u32) {
        let s = self.data.lp_size;
        (get_field(s, 8, 0) as u8, get_field(s, 3, 16))
    }

    #[doc(alias = "GXSetPointSize")]
    pub fn set_point_size(&mut self, point_size: u8, tex_offsets: u32) {
        set_field(&mut self.data.lp_size, 8, 8, u32::from(point_size));
        set_field(&mut self.data.lp_size, 3, 19, tex_offsets);
        self.write_ras_reg(self.data.lp_size);
        self.data.bp_sent = 0;
    }

    /// `GXGetPointSize`: (pointSize, texOffsets).
    pub fn get_point_size(&self) -> (u8, u32) {
        let s = self.data.lp_size;
        (get_field(s, 8, 8) as u8, get_field(s, 3, 19))
    }

    #[doc(alias = "GXEnableTexOffsets")]
    pub fn enable_tex_offsets(&mut self, coord: u32, line_enable: u8, point_enable: u8) {
        let ts0 = &mut self.data.su_ts0[coord as usize];
        set_field(ts0, 1, 18, u32::from(line_enable));
        set_field(ts0, 1, 19, u32::from(point_enable));
        let v = *ts0;
        self.write_ras_reg(v);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetCullMode")]
    pub fn set_cull_mode(&mut self, mode: u32) {
        let hw_mode = match mode {
            GX_CULL_FRONT => GX_CULL_BACK,
            GX_CULL_BACK => GX_CULL_FRONT,
            _ => mode,
        };
        set_field(&mut self.data.gen_mode, 2, 14, hw_mode);
        self.data.dirty_state |= 4;
    }

    /// `GXGetCullMode`.
    pub fn get_cull_mode(&self) -> u32 {
        match get_field(self.data.gen_mode, 2, 14) {
            GX_CULL_FRONT => GX_CULL_BACK,
            GX_CULL_BACK => GX_CULL_FRONT,
            m => m,
        }
    }

    #[doc(alias = "GXSetCoPlanar")]
    pub fn set_co_planar(&mut self, enable: u8) {
        set_field(&mut self.data.gen_mode, 1, 19, u32::from(enable));
        self.write_ras_reg(0xFE08_0000);
        self.write_ras_reg(self.data.gen_mode);
    }

    #[doc(alias = "__GXSetGenMode")]
    pub(crate) fn set_gen_mode(&mut self) {
        self.write_ras_reg(self.data.gen_mode);
        self.data.bp_sent = 0;
    }
}
