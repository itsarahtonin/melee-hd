//! GXPerf.c: the GPU's performance counters, as far as they write to the stream.

use super::enums::*;
use super::{Gx, set_field};

impl Gx {
    /// `GXSetGPMetric`: turns the old metrics' counters off, then the new ones on. The CP
    /// metrics' `__cpReg[3]` writes aren't in the stream and are skipped.
    #[doc(alias = "GXSetGPMetric")]
    pub fn set_gp_metric(&mut self, perf0: u32, perf1: u32) {
        match self.data.perf0 {
            GX_PERF0_VERTICES..=GX_PERF0_CLIP_RATIO | GX_PERF0_CLOCKS => self.write_xf_reg(6, 0),
            GX_PERF0_TRIANGLES..=GX_PERF0_TRIANGLES_2CLR => self.write_ras_reg(0x2300_0000),
            GX_PERF0_QUAD_0CVG..=GX_PERF0_AVG_QUAD_CNT => self.write_ras_reg(0x2400_0000),
            _ => {}
        }
        match self.data.perf1 {
            GX_PERF1_TEXELS..=GX_PERF1_TC_MISS | GX_PERF1_CLOCKS => self.write_ras_reg(0x6700_0000),
            GX_PERF1_VC_ELEMQ_FULL..=GX_PERF1_VERTICES => {
                set_field(&mut self.data.perf_sel, 4, 4, 0);
                self.write_cp_reg(0x20, self.data.perf_sel);
            }
            // GX_PERF1_FIFO_REQ to GX_PERF1_CP_ALL_REQ: __cpReg[3] = 0.
            _ => {}
        }

        self.data.perf0 = perf0;
        let xf = match perf0 {
            GX_PERF0_VERTICES => Some(0x273),
            GX_PERF0_CLIP_VTX => Some(0x14A),
            GX_PERF0_CLIP_CLKS => Some(0x16B),
            GX_PERF0_XF_WAIT_IN => Some(0x84),
            GX_PERF0_XF_WAIT_OUT => Some(0xC6),
            GX_PERF0_XF_XFRM_CLKS => Some(0x210),
            GX_PERF0_XF_LIT_CLKS => Some(0x252),
            GX_PERF0_XF_BOT_CLKS => Some(0x231),
            GX_PERF0_XF_REGLD_CLKS => Some(0x1AD),
            GX_PERF0_XF_REGRD_CLKS => Some(0x1CE),
            GX_PERF0_CLIP_RATIO => Some(0x153),
            GX_PERF0_CLOCKS => Some(0x21),
            _ => None,
        };
        let ras = match perf0 {
            GX_PERF0_TRIANGLES => Some(0x2303_AE7F),
            GX_PERF0_TRIANGLES_CULLED => Some(0x2303_8E7F),
            GX_PERF0_TRIANGLES_PASSED => Some(0x2303_9E7F),
            GX_PERF0_TRIANGLES_SCISSORED => Some(0x2303_1E7F),
            GX_PERF0_TRIANGLES_0TEX => Some(0x2303_AC3F),
            GX_PERF0_TRIANGLES_1TEX => Some(0x2303_AC7F),
            GX_PERF0_TRIANGLES_2TEX => Some(0x2303_ACBF),
            GX_PERF0_TRIANGLES_3TEX => Some(0x2303_ACFF),
            GX_PERF0_TRIANGLES_4TEX => Some(0x2303_AD3F),
            GX_PERF0_TRIANGLES_5TEX => Some(0x2303_AD7F),
            GX_PERF0_TRIANGLES_6TEX => Some(0x2303_ADBF),
            GX_PERF0_TRIANGLES_7TEX => Some(0x2303_ADFF),
            GX_PERF0_TRIANGLES_8TEX => Some(0x2303_AE3F),
            GX_PERF0_TRIANGLES_0CLR => Some(0x2303_A27F),
            GX_PERF0_TRIANGLES_1CLR => Some(0x2303_A67F),
            GX_PERF0_TRIANGLES_2CLR => Some(0x2303_AA7F),
            GX_PERF0_QUAD_0CVG => Some(0x2402_C0C6),
            GX_PERF0_QUAD_NON0CVG => Some(0x2402_C16B),
            GX_PERF0_QUAD_1CVG => Some(0x2402_C0E7),
            GX_PERF0_QUAD_2CVG => Some(0x2402_C108),
            GX_PERF0_QUAD_3CVG => Some(0x2402_C129),
            GX_PERF0_QUAD_4CVG => Some(0x2402_C14A),
            GX_PERF0_AVG_QUAD_CNT => Some(0x2402_C1AD),
            _ => None,
        };
        if let Some(v) = xf {
            self.write_xf_reg(6, v);
        }
        if let Some(v) = ras {
            self.write_ras_reg(v);
        }

        self.data.perf1 = perf1;
        let ras = match perf1 {
            GX_PERF1_TEXELS => Some(0x6700_0042),
            GX_PERF1_TX_IDLE => Some(0x6700_0084),
            GX_PERF1_TX_REGS => Some(0x6700_0063),
            GX_PERF1_TX_MEMSTALL => Some(0x6700_0129),
            GX_PERF1_TC_CHECK1_2 => Some(0x6700_014B),
            GX_PERF1_TC_CHECK3_4 => Some(0x6700_018D),
            GX_PERF1_TC_CHECK5_6 => Some(0x6700_01CF),
            GX_PERF1_TC_CHECK7_8 => Some(0x6700_0211),
            GX_PERF1_TC_MISS => Some(0x6700_0252),
            GX_PERF1_CLOCKS => Some(0x6700_0021),
            _ => None,
        };
        let sel = match perf1 {
            GX_PERF1_VC_ELEMQ_FULL => Some(2),
            GX_PERF1_VC_MISSQ_FULL => Some(3),
            GX_PERF1_VC_MEMREQ_FULL => Some(4),
            GX_PERF1_VC_STATUS7 => Some(5),
            GX_PERF1_VC_MISSREP_FULL => Some(6),
            GX_PERF1_VC_STREAMBUF_LOW => Some(7),
            GX_PERF1_VC_ALL_STALLS => Some(9),
            GX_PERF1_VERTICES => Some(8),
            _ => None,
        };
        if let Some(v) = ras {
            self.write_ras_reg(v);
        }
        if let Some(s) = sel {
            set_field(&mut self.data.perf_sel, 4, 4, s);
            self.write_cp_reg(0x20, self.data.perf_sel);
        }
        // GX_PERF1_FIFO_REQ to GX_PERF1_CP_ALL_REQ write __cpReg[3], not the stream.
        self.data.bp_sent = 0;
    }

    /// `GXClearGPMetric`: writes `__cpReg[2]`, not the stream.
    #[doc(alias = "GXClearGPMetric")]
    pub fn clear_gp_metric(&mut self) {}
}
