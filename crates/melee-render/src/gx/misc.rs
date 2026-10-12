//! GXMisc.c: flushing, draw syncs and the PE pokes.

use super::enums::*;
use super::{Gx, cntlzw};

impl Gx {
    #[doc(alias = "GXSetMisc")]
    pub fn set_misc(&mut self, token: u32, val: u32) {
        match token {
            GX_MT_XF_FLUSH => {
                self.data.v_num = val as u16;
                self.data.unk = u16::from(self.data.v_num == 0);
                self.data.bp_sent = 1;
                if self.data.v_num != 0 {
                    self.data.dirty_state |= 8;
                }
            }
            GX_MT_DL_SAVE_CONTEXT => self.data.dl_save_context = u8::from(val > 0),
            _ => {}
        }
    }

    /// `GXFlush`: the dirty state, then 32 bytes of zeros (NOPs) that push the write-gather
    /// pipe out. `PPCSync` has no counterpart.
    #[doc(alias = "GXFlush")]
    pub fn flush(&mut self) {
        if self.data.dirty_state != 0 {
            self.set_dirty_state();
        }
        for _ in 0..8 {
            self.write_u32(0);
        }
    }

    /// `GXResetWriteGatherPipe`: CPU registers only.
    #[doc(alias = "GXResetWriteGatherPipe")]
    pub fn reset_write_gather_pipe(&mut self) {}

    /// `GXAbortFrame`: PI registers and a wait; nothing in the stream.
    #[doc(alias = "GXAbortFrame")]
    pub fn abort_frame(&mut self) {}

    #[doc(alias = "GXSetDrawSync")]
    pub fn set_draw_sync(&mut self, token: u16) {
        let mut reg = u32::from(token) | 0x4800_0000;
        self.write_ras_reg(reg);
        super::set_field(&mut reg, 16, 0, u32::from(token));
        super::set_field(&mut reg, 8, 24, 0x47);
        self.write_ras_reg(reg);
        self.flush();
        self.data.bp_sent = 1;
    }

    /// `GXReadDrawSync`: reads a PE register, not the stream; always 0 here.
    #[doc(alias = "GXReadDrawSync")]
    pub fn read_draw_sync(&self) -> u16 {
        0
    }

    /// `GXSetDrawDone`: the `PE_DONE` write and a flush (`DrawDone = 0` is the interrupt's
    /// business, not modelled).
    #[doc(alias = "GXSetDrawDone")]
    pub fn set_draw_done(&mut self) {
        self.write_ras_reg(0x4500_0002);
        self.flush();
    }

    /// `GXWaitDrawDone`: waits for the GPU's interrupt; nothing to do here.
    #[doc(alias = "GXWaitDrawDone")]
    pub fn wait_draw_done(&mut self) {}

    #[doc(alias = "GXDrawDone")]
    pub fn draw_done(&mut self) {
        self.set_draw_done();
        self.wait_draw_done();
    }

    #[doc(alias = "GXPixModeSync")]
    pub fn pix_mode_sync(&mut self) {
        self.write_ras_reg(self.data.pe_ctrl);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXTexModeSync")]
    pub fn tex_mode_sync(&mut self) {
        self.write_ras_reg(0x6300_0000);
        self.data.bp_sent = 1;
    }

    /// `GXPokeAlphaMode`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeAlphaMode")]
    pub fn poke_alpha_mode(&mut self, _func: u32, _threshold: u8) {}

    /// `GXPokeAlphaRead`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeAlphaRead")]
    pub fn poke_alpha_read(&mut self, _mode: u32) {}

    /// `GXPokeAlphaUpdate`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeAlphaUpdate")]
    pub fn poke_alpha_update(&mut self, _update_enable: u8) {}

    /// `GXPokeBlendMode`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeBlendMode")]
    pub fn poke_blend_mode(&mut self, _ty: u32, _src_factor: u32, _dst_factor: u32, _op: u32) {}

    /// `GXPokeColorUpdate`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeColorUpdate")]
    pub fn poke_color_update(&mut self, _update_enable: u8) {}

    /// `GXPokeDstAlpha`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeDstAlpha")]
    pub fn poke_dst_alpha(&mut self, _enable: u8, _alpha: u8) {}

    /// `GXPokeDither`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeDither")]
    pub fn poke_dither(&mut self, _dither: u8) {}

    /// `GXPokeZMode`: writes `__peReg`, not the stream.
    #[doc(alias = "GXPokeZMode")]
    pub fn poke_z_mode(&mut self, _compare_enable: u8, _func: u32, _update_enable: u8) {}
}

/// `GXCompressZ16`.
#[doc(alias = "GXCompressZ16")]
pub fn compress_z16(z24: u32, zfmt: u32) -> u32 {
    let z24n = !(z24 << 8);
    let temp = cntlzw(z24n) as i32;
    // (exp cap, shift at the cap, base shift, mantissa mask)
    let (cap, cap_shift, base, mask): (i32, i32, i32, u32) = match zfmt {
        GX_ZC_LINEAR => return (z24 >> 8) & 0xFFFF,
        GX_ZC_NEAR => (3, 7, 9, 0x3FFF),
        GX_ZC_MID => (7, 4, 10, 0x1FFF),
        GX_ZC_FAR => (12, 0, 11, 0xFFF),
        // OSPanic in the original.
        _ => return 0,
    };
    let exp = temp.min(cap);
    let shift = if exp == cap { cap_shift } else { base - exp };
    let mant_bits = 32 - mask.leading_zeros();
    ((z24 >> shift) & mask) | ((exp as u32) << mant_bits)
}

/// `GXDecompressZ16`.
#[doc(alias = "GXDecompressZ16")]
pub fn decompress_z16(z16: u32, zfmt: u32) -> u32 {
    let (exp, shift, mant) = match zfmt {
        GX_ZC_LINEAR => return (z16 << 8) & 0xFF_FF00,
        GX_ZC_NEAR => {
            let exp = ((z16 >> 14) & 3) as i32;
            (exp, if exp == 3 { 7 } else { 9 - exp }, z16 & 0x3FFF)
        }
        GX_ZC_MID => {
            let exp = ((z16 >> 13) & 7) as i32;
            (exp, if exp == 7 { 4 } else { 10 - exp }, z16 & 0x1FFF)
        }
        GX_ZC_FAR => {
            let exp = ((z16 >> 12) & 0xF) as i32;
            (exp, if exp == 12 { 0 } else { 11 - exp }, z16 & 0xFFF)
        }
        _ => return 0,
    };
    let cb1 = melee_sim::ppc::slw(u32::MAX, (24 - exp) as u32);
    (cb1 | melee_sim::ppc::slw(mant, shift as u32)) & 0xFF_FFFF
}
