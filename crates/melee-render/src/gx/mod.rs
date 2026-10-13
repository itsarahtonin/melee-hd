//! The GameCube GX SDK: the CPU-side graphics library that encodes GPU commands, ported from
//! the decomp's `libs/dolphin/src/dolphin/gx` as the game's release build runs it.
//!
//! [`Gx`] holds the SDK's shadow state ([`GxData`], `__GXData_struct`) and the command stream it
//! writes: each FIFO write (`GX_WRITE_U8`, `GX_WRITE_RAS_REG`, `GX_WRITE_XF_REG`, the CP writes)
//! pushes its bytes, big-endian, onto [`Gx::fifo`], in the order the SDK writes them, for
//! `melee_gx` to parse. Writes to memory-mapped registers that bypass the FIFO (`__cpReg`,
//! `__peReg`, `__piReg`, `__memReg`, VI) aren't GPU state the stream carries and are skipped,
//! each where it happens. Debug-only code (asserts, `__GXinBegin`, the verifier, `GXSave.c`)
//! isn't ported.
//!
//! Every SDK function is a method named after its C name without the `GX` prefix, in snake
//! case, with the C argument order; the internal `__GX` functions are methods too. Pointers into
//! the console's memory (images, palettes, display lists, vertex arrays) are `u32` addresses,
//! masked as the SDK masks them. Enumerations are the `u32` constants of [`enums`].

#![allow(dead_code)]

mod attr;
mod bump;
mod display_list;
pub mod enums;
mod frame_buf;
mod geometry;
mod init;
mod light;
mod misc;
mod perf;
mod pixel;
mod tev;
mod texture;
mod transform;
mod vert;

#[cfg(test)]
mod tests;

use gekko_fp as fp;

pub use attr::{VtxAttrFmtList, VtxDescList};
pub use frame_buf::{
    GX_MPAL_480_INT_DF, GX_NTSC_480_INT_DF, GX_PAL_528_INT_DF, RenderModeObj, adjust_for_overscan,
};
pub use init::{BUS_CLOCK, VI_MPAL, VI_NTSC, VI_PAL};
pub use light::LightObj;
pub use misc::{compress_z16, decompress_z16};
pub use pixel::FogAdjTable;
pub use texture::{
    TexObj, TexRegion, TexRegionCallback, TexRegionId, TlutObj, TlutRegion, TlutRegionCallback,
    get_image_tile_count, get_tex_buffer_size,
};
pub use transform::project;

/// `GXColor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
}

/// `GXColorS10`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorS10 {
    pub r: i16,
    pub g: i16,
    pub b: i16,
    pub a: i16,
}

/// `struct __GXData_struct`: the SDK's shadow of the GPU registers and its own bookkeeping.
#[derive(Clone, Debug, Default)]
pub struct GxData {
    /// Offset 0x0. Set to `!vNum` by `GXSetMisc(GX_MT_XF_FLUSH)`; `GXBegin` and
    /// `GXCallDisplayList` send a flush primitive when it and `bp_sent` are both 0.
    pub unk: u16,
    pub bp_sent: u16,
    pub v_num: u16,
    pub v_lim: u16,
    pub cp_enable: u32,
    pub cp_status: u32,
    pub cp_clr: u32,
    pub vcd_lo: u32,
    pub vcd_hi: u32,
    pub vat_a: [u32; 8],
    pub vat_b: [u32; 8],
    pub vat_c: [u32; 8],
    pub lp_size: u32,
    pub mat_idx_a: u32,
    pub mat_idx_b: u32,
    pub index_base: [u32; 4],
    pub index_stride: [u32; 4],
    pub amb_color: [u32; 2],
    pub mat_color: [u32; 2],
    pub su_ts0: [u32; 8],
    pub su_ts1: [u32; 8],
    pub su_scis0: u32,
    pub su_scis1: u32,
    pub tref: [u32; 8],
    pub iref: u32,
    pub bp_mask: u32,
    pub ind_tex_scale0: u32,
    pub ind_tex_scale1: u32,
    pub tevc: [u32; 16],
    pub teva: [u32; 16],
    pub tev_ksel: [u32; 8],
    pub cmode0: u32,
    pub cmode1: u32,
    pub zmode: u32,
    pub pe_ctrl: u32,
    pub cp_disp_src: u32,
    pub cp_disp_size: u32,
    pub cp_disp_stride: u32,
    pub cp_disp: u32,
    pub cp_tex_src: u32,
    pub cp_tex_size: u32,
    pub cp_tex_stride: u32,
    pub cp_tex: u32,
    pub cp_tex_z: u8,
    pub gen_mode: u32,
    pub tex_regions: [TexRegion; 8],
    pub tex_regions_ci: [TexRegion; 4],
    pub next_tex_rgn: u32,
    pub next_tex_rgn_ci: u32,
    pub tlut_regions: [TlutRegion; 20],
    /// `texRegionCallback`; `None` before `GXInit` sets the default.
    pub tex_region_callback: Option<TexRegionCallback>,
    /// `tlutRegionCallback`; `None` before `GXInit` sets the default.
    pub tlut_region_callback: Option<TlutRegionCallback>,
    pub nrm_type: u32,
    pub has_nrms: u8,
    pub has_bi_nrms: u8,
    pub proj_type: u32,
    pub proj_mtx: [f32; 6],
    pub vp_left: f32,
    pub vp_top: f32,
    pub vp_wd: f32,
    pub vp_ht: f32,
    pub vp_nearz: f32,
    pub vp_farz: f32,
    pub fg_range: u8,
    pub fg_side_x: f32,
    pub t_image0: [u32; 8],
    pub t_mode0: [u32; 8],
    pub texmap_id: [u32; 16],
    pub tcs_man_enab: u32,
    /// `GXPerf0`.
    pub perf0: u32,
    /// `GXPerf1`.
    pub perf1: u32,
    pub perf_sel: u32,
    pub in_disp_list: u8,
    pub dl_save_context: u8,
    pub dirty_vat: u8,
    pub dirty_state: u32,
}

/// The SDK's state and the command stream it has written.
#[derive(Clone, Debug)]
pub struct Gx {
    /// The bytes written to the GPU's FIFO (or, between `begin_display_list` and
    /// `end_display_list`, to the display list being built).
    pub fifo: Vec<u8>,
    pub data: GxData,
    /// What `VIGetTvFormat` reports to `GXInit`, which picks its render mode: [`VI_NTSC`],
    /// [`VI_PAL`] or [`VI_MPAL`].
    pub tv_format: u32,
    /// `__OSBusClock`, which `GXInit` derives two timing registers from.
    pub bus_clock: u32,
    /// While a display list is built: the FIFO's bytes and, with `dlSaveContext`, the state
    /// to restore (`__savedGXdata`).
    display_list: Option<display_list::Saved>,
}

impl Default for Gx {
    fn default() -> Self {
        Self::new()
    }
}

impl Gx {
    /// The SDK before `GXInit`: its data zeroed as the console's static data starts, on an NTSC
    /// console.
    pub fn new() -> Self {
        Gx {
            fifo: Vec::new(),
            data: GxData::default(),
            tv_format: VI_NTSC,
            bus_clock: BUS_CLOCK,
            display_list: None,
        }
    }

    /// Takes the bytes written so far.
    pub fn take_fifo(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.fifo)
    }

    /// `GX_WRITE_U8`.
    #[inline]
    pub(crate) fn write_u8(&mut self, v: u8) {
        self.fifo.push(v);
    }

    /// `GX_WRITE_U16`.
    #[inline]
    pub(crate) fn write_u16(&mut self, v: u16) {
        self.fifo.extend_from_slice(&v.to_be_bytes());
    }

    /// `GX_WRITE_U32`.
    #[inline]
    pub(crate) fn write_u32(&mut self, v: u32) {
        self.fifo.extend_from_slice(&v.to_be_bytes());
    }

    /// `GX_WRITE_F32` of a value already single precision.
    #[inline]
    pub(crate) fn write_f32(&mut self, v: f32) {
        self.write_u32(v.to_bits());
    }

    /// `GX_WRITE_F32` of a register: `stfs` rounds it to single precision.
    #[inline]
    pub(crate) fn write_f32_reg(&mut self, r: f64) {
        self.write_u32(fp::stfs(r));
    }

    /// `GX_WRITE_RAS_REG`: a BP register write.
    #[inline]
    pub(crate) fn write_ras_reg(&mut self, value: u32) {
        self.write_u8(0x61);
        self.write_u32(value);
    }

    /// `GX_WRITE_XF_REG`: one XF register.
    #[inline]
    pub(crate) fn write_xf_reg(&mut self, addr: u32, value: u32) {
        self.write_u8(0x10);
        self.write_u32(0x1000u32.wrapping_add(addr));
        self.write_u32(value);
    }

    /// `GX_WRITE_SOME_REG4`: a CP register write.
    #[inline]
    pub(crate) fn write_cp_reg(&mut self, reg: u8, value: u32) {
        self.write_u8(0x08);
        self.write_u8(reg);
        self.write_u32(value);
    }
}

/// `SET_REG_FIELD`: puts `val` at `shift`, clearing the field's `size` bits first. Like the
/// original, `val` isn't masked to the field.
#[inline]
pub(crate) fn set_field(reg: &mut u32, size: u32, shift: u32, val: u32) {
    let mask = ((1u32 << size) - 1) << shift;
    *reg = (*reg & !mask) | (val << shift);
}

/// `GET_REG_FIELD`.
#[inline]
pub(crate) fn get_field(reg: u32, size: u32, shift: u32) -> u32 {
    (reg >> shift) & ((1u32 << size) - 1)
}

/// A float as a register holds it after `lfs`.
#[inline]
pub(crate) fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs`.
#[inline]
pub(crate) fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

/// `__cntlzw`.
#[inline]
pub(crate) fn cntlzw(x: u32) -> u32 {
    x.leading_zeros()
}
