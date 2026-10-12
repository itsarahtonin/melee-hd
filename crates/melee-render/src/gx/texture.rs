//! GXTexture.c: texture and palette objects, texture memory regions, loads.

use super::enums::*;
use super::{Gx, GxData, cntlzw, get_field, reg, set_field};
use gekko_fp as fp;

/// `__GXTexObjInt_struct`, the private layout of `GXTexObj`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TexObj {
    pub mode0: u32,
    pub mode1: u32,
    pub image0: u32,
    pub image3: u32,
    /// `userData`, a pointer the SDK only keeps.
    pub user_data: u32,
    /// `GXTexFmt`.
    pub fmt: u32,
    pub tlut_name: u32,
    pub load_cnt: u16,
    pub load_fmt: u8,
    /// Bit 0: mipmapped; bit 1: not color indexed.
    pub flags: u8,
}

/// `__GXTexRegionInt_struct`, the private layout of `GXTexRegion`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TexRegion {
    pub image1: u32,
    pub image2: u32,
    pub size_even: u16,
    pub size_odd: u16,
    pub is_32b_mipmap: u8,
    pub is_cached: u8,
}

/// `__GXTlutObjInt_struct`, the private layout of `GXTlutObj`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TlutObj {
    pub tlut: u32,
    pub load_tlut0: u32,
    pub num_entries: u16,
}

/// `__GXTlutRegionInt_struct`, the private layout of `GXTlutRegion`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TlutRegion {
    pub load_tlut1: u32,
    /// The palette last loaded into the region.
    pub tlut_obj: TlutObj,
}

/// The texture region a [`TexRegionCallback`] picks, from [`GxData::tex_regions`] or
/// [`GxData::tex_regions_ci`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexRegionId {
    Normal(usize),
    Ci(usize),
}

/// `GXTexRegionCallback`: the region a texture loads into.
pub type TexRegionCallback = fn(&mut GxData, &TexObj, u32) -> TexRegionId;

/// `GXTlutRegionCallback`: the index in [`GxData::tlut_regions`] of a palette's region, or
/// `None` for an invalid name.
pub type TlutRegionCallback = fn(u32) -> Option<usize>;

const TEX_MODE0_IDS: [u8; 8] = [0x80, 0x81, 0x82, 0x83, 0xA0, 0xA1, 0xA2, 0xA3];
const TEX_MODE1_IDS: [u8; 8] = [0x84, 0x85, 0x86, 0x87, 0xA4, 0xA5, 0xA6, 0xA7];
const TEX_IMAGE0_IDS: [u8; 8] = [0x88, 0x89, 0x8A, 0x8B, 0xA8, 0xA9, 0xAA, 0xAB];
const TEX_IMAGE1_IDS: [u8; 8] = [0x8C, 0x8D, 0x8E, 0x8F, 0xAC, 0xAD, 0xAE, 0xAF];
const TEX_IMAGE2_IDS: [u8; 8] = [0x90, 0x91, 0x92, 0x93, 0xB0, 0xB1, 0xB2, 0xB3];
const TEX_IMAGE3_IDS: [u8; 8] = [0x94, 0x95, 0x96, 0x97, 0xB4, 0xB5, 0xB6, 0xB7];
const TEX_TLUT_IDS: [u8; 8] = [0x98, 0x99, 0x9A, 0x9B, 0xB8, 0xB9, 0xBA, 0xBB];
const GX2HW_FILT_CONV: [u8; 6] = [0x00, 0x04, 0x01, 0x05, 0x02, 0x06];
const HW2GX_FILT_CONV: [u8; 8] = [0x00, 0x02, 0x04, 0x00, 0x01, 0x03, 0x05, 0x00];

/// `__GXGetTexTileShift`: (row, column) tile shifts.
fn get_tex_tile_shift(fmt: u32) -> (u32, u32) {
    match fmt {
        GX_TF_I4 | GX_TF_C4 | GX_TF_CMPR | GX_CTF_R4 | GX_CTF_Z4 => (3, 3),
        GX_TF_I8 | GX_TF_IA4 | GX_TF_C8 | GX_TF_Z8 | GX_CTF_RA4 | GX_TF_A8 | GX_CTF_R8
        | GX_CTF_G8 | GX_CTF_B8 | GX_CTF_Z8M | GX_CTF_Z8L => (3, 2),
        GX_TF_IA8 | GX_TF_RGB565 | GX_TF_RGB5A3 | GX_TF_RGBA8 | GX_TF_C14X2 | GX_TF_Z16
        | GX_TF_Z24X8 | GX_CTF_RA8 | GX_CTF_RG8 | GX_CTF_GB8 | GX_CTF_Z16L => (2, 2),
        _ => (0, 0),
    }
}

/// `__GetImageTileCount`: (row tiles, column tiles, tiles per block).
pub fn get_image_tile_count(fmt: u32, wd: u16, ht: u16) -> (u32, u32, u32) {
    let (row_shift, col_shift) = get_tex_tile_shift(fmt);
    let wd = u32::from(wd.max(1));
    let ht = u32::from(ht.max(1));
    let row_tiles = (wd + (1 << row_shift) - 1) >> row_shift;
    let col_tiles = (ht + (1 << col_shift) - 1) >> col_shift;
    let cmp_tiles = if fmt == GX_TF_RGBA8 || fmt == GX_TF_Z24X8 {
        2
    } else {
        1
    };
    (row_tiles, col_tiles, cmp_tiles)
}

/// The float-to-`u8` conversions of the LOD fields: `fctiwz`, then the low byte.
fn lod_byte(r: f64) -> u32 {
    u32::from(fp::fctiwz(r) as u8)
}

/// `GXGetTexBufferSize`: the bytes a texture's image takes, with `max_lod` levels if
/// `mipmap` is 1.
#[doc(alias = "GXGetTexBufferSize")]
pub fn get_tex_buffer_size(width: u16, height: u16, format: u32, mipmap: u8, max_lod: u8) -> u32 {
    let (shift_x, shift_y) = get_tex_tile_shift(format);
    let tile_bytes = if format == GX_TF_RGBA8 || format == GX_TF_Z24X8 {
        64
    } else {
        32
    };
    let (mut width, mut height) = (u32::from(width), u32::from(height));
    if mipmap == 1 {
        let mut buffer_size = 0u32;
        for _ in 0..max_lod {
            let nx = (width + (1 << shift_x) - 1) >> shift_x;
            let ny = (height + (1 << shift_y) - 1) >> shift_y;
            buffer_size = buffer_size.wrapping_add(tile_bytes * (nx * ny));
            if width == 1 && height == 1 {
                break;
            }
            width = if width > 1 { width >> 1 } else { 1 };
            height = if height > 1 { height >> 1 } else { 1 };
        }
        buffer_size
    } else {
        let nx = (width + (1 << shift_x) - 1) >> shift_x;
        let ny = (height + (1 << shift_y) - 1) >> shift_y;
        nx * ny * tile_bytes
    }
}

impl Gx {
    /// [`get_tex_buffer_size`] as a method, like the other SDK functions.
    #[doc(alias = "GXGetTexBufferSize")]
    pub fn get_tex_buffer_size(
        &self,
        width: u16,
        height: u16,
        format: u32,
        mipmap: u8,
        max_lod: u8,
    ) -> u32 {
        get_tex_buffer_size(width, height, format, mipmap, max_lod)
    }

    /// `GXInitTexObj`: `image_ptr` is the image's address; the object keeps it in 32-byte
    /// units, `(addr >> 5) & 0x01FFFFFF`.
    #[doc(alias = "GXInitTexObj")]
    #[allow(clippy::too_many_arguments)]
    pub fn init_tex_obj(
        &self,
        obj: &mut TexObj,
        image_ptr: u32,
        width: u16,
        height: u16,
        format: u32,
        wrap_s: u32,
        wrap_t: u32,
        mipmap: u8,
    ) {
        let t = obj;
        *t = TexObj::default();
        set_field(&mut t.mode0, 2, 0, wrap_s);
        set_field(&mut t.mode0, 2, 2, wrap_t);
        set_field(&mut t.mode0, 1, 4, 1);
        if mipmap != 0 {
            t.flags |= 1;
            if format.wrapping_sub(8) <= 2 {
                t.mode0 = (t.mode0 & 0xFFFF_FF1F) | 0xA0;
            } else {
                t.mode0 = (t.mode0 & 0xFFFF_FF1F) | 0xC0;
            }
            let max_lod = if width > height {
                31u32.wrapping_sub(cntlzw(u32::from(width)))
            } else {
                31u32.wrapping_sub(cntlzw(u32::from(height)))
            };
            let lmax = lod_byte(fp::fmuls(16.0, fp::frsp(f64::from(max_lod))));
            set_field(&mut t.mode1, 8, 8, lmax);
        } else {
            t.mode0 = (t.mode0 & 0xFFFF_FF1F) | 0x80;
        }
        t.fmt = format;
        set_field(&mut t.image0, 10, 0, u32::from(width).wrapping_sub(1));
        set_field(&mut t.image0, 10, 10, u32::from(height).wrapping_sub(1));
        set_field(&mut t.image0, 4, 20, format & 0xF);
        let image_base = (image_ptr >> 5) & 0x01FF_FFFF;
        set_field(&mut t.image3, 21, 0, image_base);
        let (load_fmt, row_t, col_t) = match format & 0xF {
            0 | 8 => (1, 3, 3),
            1 | 2 | 9 => (2, 3, 2),
            3 | 4 | 5 | 10 => (2, 2, 2),
            6 => (3, 2, 2),
            14 => (0, 3, 3),
            _ => (2, 2, 2),
        };
        t.load_fmt = load_fmt;
        let row_c = (u32::from(width) + (1 << row_t) - 1) >> row_t;
        let col_c = (u32::from(height) + (1 << col_t) - 1) >> col_t;
        t.load_cnt = ((row_c * col_c) & 0x7FFF) as u16;
        t.flags |= 2;
    }

    #[doc(alias = "GXInitTexObjCI")]
    #[allow(clippy::too_many_arguments)]
    pub fn init_tex_obj_ci(
        &self,
        obj: &mut TexObj,
        image_ptr: u32,
        width: u16,
        height: u16,
        format: u32,
        wrap_s: u32,
        wrap_t: u32,
        mipmap: u8,
        tlut_name: u32,
    ) {
        self.init_tex_obj(
            obj, image_ptr, width, height, format, wrap_s, wrap_t, mipmap,
        );
        obj.flags &= !2;
        obj.tlut_name = tlut_name;
    }

    #[doc(alias = "GXInitTexObjLOD")]
    #[allow(clippy::too_many_arguments)]
    pub fn init_tex_obj_lod(
        &self,
        obj: &mut TexObj,
        min_filt: u32,
        mag_filt: u32,
        min_lod: f32,
        max_lod: f32,
        lod_bias: f32,
        bias_clamp: u8,
        do_edge_lod: u8,
        max_aniso: u32,
    ) {
        let t = obj;
        let lod_bias = if lod_bias < -4.0 {
            -4.0
        } else if lod_bias >= 4.0 {
            3.99
        } else {
            lod_bias
        };
        let lbias = lod_byte(fp::fmuls(32.0, reg(lod_bias)));
        set_field(&mut t.mode0, 8, 9, lbias);
        set_field(&mut t.mode0, 1, 4, u32::from(mag_filt == GX_LINEAR));
        set_field(
            &mut t.mode0,
            3,
            5,
            u32::from(GX2HW_FILT_CONV[min_filt as usize]),
        );
        set_field(&mut t.mode0, 1, 8, u32::from(do_edge_lod == 0));
        t.mode0 &= 0xFFFD_FFFF;
        t.mode0 &= 0xFFFB_FFFF;
        set_field(&mut t.mode0, 2, 19, max_aniso);
        set_field(&mut t.mode0, 1, 21, u32::from(bias_clamp));
        // NaN passes through both clamps' comparisons, as f32::clamp lets it.
        let min_lod = min_lod.clamp(0.0, 10.0);
        let lmin = lod_byte(fp::fmuls(16.0, reg(min_lod)));
        let max_lod = max_lod.clamp(0.0, 10.0);
        let lmax = lod_byte(fp::fmuls(16.0, reg(max_lod)));
        set_field(&mut t.mode1, 8, 0, lmin);
        set_field(&mut t.mode1, 8, 8, lmax);
    }

    /// `GXInitTexObjData`.
    pub fn init_tex_obj_data(&self, obj: &mut TexObj, image_ptr: u32) {
        set_field(&mut obj.image3, 21, 0, (image_ptr >> 5) & 0x01FF_FFFF);
    }

    /// `GXInitTexObjWrapMode`.
    pub fn init_tex_obj_wrap_mode(&self, obj: &mut TexObj, sm: u32, tm: u32) {
        set_field(&mut obj.mode0, 2, 0, sm);
        set_field(&mut obj.mode0, 2, 2, tm);
    }

    /// `GXInitTexObjTlut`.
    pub fn init_tex_obj_tlut(&self, obj: &mut TexObj, tlut_name: u32) {
        obj.tlut_name = tlut_name;
    }

    /// `GXInitTexObjFilter` (not in this SDK's C; GXInitTexObjLOD's filter half, as later
    /// SDKs define it).
    pub fn init_tex_obj_filter(&self, obj: &mut TexObj, min_filt: u32, mag_filt: u32) {
        set_field(&mut obj.mode0, 1, 4, u32::from(mag_filt == GX_LINEAR));
        set_field(
            &mut obj.mode0,
            3,
            5,
            u32::from(GX2HW_FILT_CONV[min_filt as usize]),
        );
    }

    /// `GXInitTexObjUserData`.
    pub fn init_tex_obj_user_data(&self, obj: &mut TexObj, user_data: u32) {
        obj.user_data = user_data;
    }

    /// `GXGetTexObjUserData`.
    pub fn get_tex_obj_user_data(&self, obj: &TexObj) -> u32 {
        obj.user_data
    }

    /// `GXGetTexObjData`: the image's address, `image3`'s field back in bytes.
    pub fn get_tex_obj_data(&self, to: &TexObj) -> u32 {
        get_field(to.image3, 21, 0) << 5
    }

    /// `GXGetTexObjWidth`.
    pub fn get_tex_obj_width(&self, to: &TexObj) -> u16 {
        (get_field(to.image0, 10, 0) + 1) as u16
    }

    /// `GXGetTexObjHeight`.
    pub fn get_tex_obj_height(&self, to: &TexObj) -> u16 {
        (get_field(to.image0, 10, 10) + 1) as u16
    }

    /// `GXGetTexObjFmt`.
    pub fn get_tex_obj_fmt(&self, to: &TexObj) -> u32 {
        to.fmt
    }

    /// `GXGetTexObjWrapS`.
    pub fn get_tex_obj_wrap_s(&self, to: &TexObj) -> u32 {
        get_field(to.mode0, 2, 0)
    }

    /// `GXGetTexObjWrapT`.
    pub fn get_tex_obj_wrap_t(&self, to: &TexObj) -> u32 {
        get_field(to.mode0, 2, 2)
    }

    /// `GXGetTexObjMipMap`.
    pub fn get_tex_obj_mip_map(&self, to: &TexObj) -> u8 {
        u8::from(to.flags & 1 == 1)
    }

    /// `GXGetTexObjMinFilt`.
    pub fn get_tex_obj_min_filt(&self, to: &TexObj) -> u32 {
        u32::from(HW2GX_FILT_CONV[get_field(to.mode0, 3, 5) as usize])
    }

    /// `GXGetTexObjMagFilt`.
    pub fn get_tex_obj_mag_filt(&self, to: &TexObj) -> u32 {
        get_field(to.mode0, 1, 4)
    }

    /// `GXGetTexObjTlut`.
    pub fn get_tex_obj_tlut(&self, to: &TexObj) -> u32 {
        to.tlut_name
    }

    /// `GXLoadTexObjPreLoaded`. The original also stamps the registers' ids into the object's
    /// and region's words; they're rewritten before every use, so here a copy takes them.
    #[doc(alias = "GXLoadTexObjPreLoaded")]
    pub fn load_tex_obj_preloaded(&mut self, obj: &TexObj, region: &mut TexRegion, id: u32) {
        let mut t = *obj;
        let i = id as usize;
        set_field(&mut t.mode0, 8, 24, u32::from(TEX_MODE0_IDS[i]));
        set_field(&mut t.mode1, 8, 24, u32::from(TEX_MODE1_IDS[i]));
        set_field(&mut t.image0, 8, 24, u32::from(TEX_IMAGE0_IDS[i]));
        set_field(&mut region.image1, 8, 24, u32::from(TEX_IMAGE1_IDS[i]));
        set_field(&mut region.image2, 8, 24, u32::from(TEX_IMAGE2_IDS[i]));
        set_field(&mut t.image3, 8, 24, u32::from(TEX_IMAGE3_IDS[i]));
        self.write_ras_reg(t.mode0);
        self.write_ras_reg(t.mode1);
        self.write_ras_reg(t.image0);
        self.write_ras_reg(region.image1);
        self.write_ras_reg(region.image2);
        self.write_ras_reg(t.image3);
        if t.flags & 2 == 0 {
            let cb = self
                .data
                .tlut_region_callback
                .expect("GXLoadTexObj: Tlut Region Callback not set");
            let r = cb(t.tlut_name).expect("GXLoadTexObj: Tlut Region Callback returns NULL");
            let tlr = &mut self.data.tlut_regions[r];
            set_field(&mut tlr.tlut_obj.tlut, 8, 24, u32::from(TEX_TLUT_IDS[i]));
            let v = tlr.tlut_obj.tlut;
            self.write_ras_reg(v);
        }
        self.data.t_image0[i] = t.image0;
        self.data.t_mode0[i] = t.mode0;
        self.data.dirty_state |= 1;
        self.data.bp_sent = 0;
    }

    /// `GXLoadTexObj`: into the region the texture region callback picks.
    #[doc(alias = "GXLoadTexObj")]
    pub fn load_tex_obj(&mut self, obj: &TexObj, id: u32) {
        let cb = self
            .data
            .tex_region_callback
            .expect("GXLoadTexObj: Tex Region Callback not set");
        let which = cb(&mut self.data, obj, id);
        let mut region = match which {
            TexRegionId::Normal(i) => self.data.tex_regions[i],
            TexRegionId::Ci(i) => self.data.tex_regions_ci[i],
        };
        self.load_tex_obj_preloaded(obj, &mut region, id);
        match which {
            TexRegionId::Normal(i) => self.data.tex_regions[i] = region,
            TexRegionId::Ci(i) => self.data.tex_regions_ci[i] = region,
        }
    }

    /// `GXInitTlutObj`: `lut` is the palette's address; the object keeps its physical address
    /// in 32-byte units.
    #[doc(alias = "GXInitTlutObj")]
    pub fn init_tlut_obj(&self, tlut_obj: &mut TlutObj, lut: u32, fmt: u32, n_entries: u16) {
        let t = tlut_obj;
        t.tlut = 0;
        set_field(&mut t.tlut, 2, 10, fmt);
        set_field(&mut t.load_tlut0, 21, 0, (lut & 0x3FFF_FFFF) >> 5);
        set_field(&mut t.load_tlut0, 8, 24, 0x64);
        t.num_entries = n_entries;
    }

    /// `GXGetTlutObjData`.
    pub fn get_tlut_obj_data(&self, t: &TlutObj) -> u32 {
        get_field(t.load_tlut0, 21, 0) << 5
    }

    /// `GXGetTlutObjFmt`.
    pub fn get_tlut_obj_fmt(&self, t: &TlutObj) -> u32 {
        get_field(t.tlut, 2, 10)
    }

    /// `GXGetTlutObjNumEntries`.
    pub fn get_tlut_obj_num_entries(&self, t: &TlutObj) -> u16 {
        t.num_entries
    }

    /// `GXLoadTlut`. The original also writes the region's offset into the caller's object
    /// before copying it to the region; here only the region's copy takes it.
    #[doc(alias = "GXLoadTlut")]
    pub fn load_tlut(&mut self, tlut_obj: &TlutObj, tlut_name: u32) {
        let cb = self
            .data
            .tlut_region_callback
            .expect("GXLoadTlut: Tlut Region Callback not set");
        let r = cb(tlut_name).expect("GXLoadTlut: Tlut Region Callback returns NULL");
        let load_tlut1 = self.data.tlut_regions[r].load_tlut1;
        self.flush_texture_state();
        self.write_ras_reg(tlut_obj.load_tlut0);
        self.write_ras_reg(load_tlut1);
        self.flush_texture_state();
        let mut t = *tlut_obj;
        set_field(&mut t.tlut, 10, 0, load_tlut1 & 0x3FF);
        self.data.tlut_regions[r].tlut_obj = t;
    }

    /// `GXInitTexCacheRegion`. An invalid size keeps the previous width exponent, as the
    /// machine code's register does (0 for the even size, the even size's for the odd).
    #[doc(alias = "GXInitTexCacheRegion")]
    pub fn init_tex_cache_region(
        &self,
        region: &mut TexRegion,
        is_32b_mipmap: u8,
        tmem_even: u32,
        size_even: u32,
        tmem_odd: u32,
        size_odd: u32,
    ) {
        let t = region;
        let mut width_exp2 = match size_even {
            GX_TEXCACHE_32K => 3,
            GX_TEXCACHE_128K => 4,
            GX_TEXCACHE_512K => 5,
            _ => 0,
        };
        t.image1 = 0;
        set_field(&mut t.image1, 15, 0, tmem_even >> 5);
        set_field(&mut t.image1, 3, 15, width_exp2);
        set_field(&mut t.image1, 3, 18, width_exp2);
        t.image1 &= 0xFFDF_FFFF;
        width_exp2 = match size_odd {
            GX_TEXCACHE_32K => 3,
            GX_TEXCACHE_128K => 4,
            GX_TEXCACHE_512K => 5,
            GX_TEXCACHE_NONE => 0,
            _ => width_exp2,
        };
        t.image2 = 0;
        set_field(&mut t.image2, 15, 0, tmem_odd >> 5);
        set_field(&mut t.image2, 3, 15, width_exp2);
        set_field(&mut t.image2, 3, 18, width_exp2);
        t.is_32b_mipmap = is_32b_mipmap;
        t.is_cached = 1;
    }

    /// `GXInitTexPreLoadRegion`.
    pub fn init_tex_pre_load_region(
        &self,
        region: &mut TexRegion,
        tmem_even: u32,
        size_even: u32,
        tmem_odd: u32,
        size_odd: u32,
    ) {
        let t = region;
        t.image1 = 0;
        set_field(&mut t.image1, 15, 0, tmem_even >> 5);
        t.image1 &= 0xFFFC_7FFF;
        t.image1 &= 0xFFE3_FFFF;
        t.image1 = (t.image1 & 0xFFDF_FFFF) | 0x20_0000;
        t.image2 = 0;
        set_field(&mut t.image2, 15, 0, tmem_odd >> 5);
        t.image2 &= 0xFFFC_7FFF;
        t.image2 &= 0xFFE3_FFFF;
        t.is_32b_mipmap = 0;
        t.is_cached = 0;
        t.size_even = (size_even >> 5) as u16;
        t.size_odd = (size_odd >> 5) as u16;
    }

    #[doc(alias = "GXInitTlutRegion")]
    pub fn init_tlut_region(&self, region: &mut TlutRegion, tmem_addr: u32, tlut_size: u32) {
        let t = region;
        t.load_tlut1 = 0;
        let tmem_addr = tmem_addr.wrapping_sub(0x80000);
        set_field(&mut t.load_tlut1, 10, 0, tmem_addr >> 9);
        set_field(&mut t.load_tlut1, 11, 10, tlut_size);
        set_field(&mut t.load_tlut1, 8, 24, 0x65);
    }

    /// `GXInvalidateTexRegion`.
    pub fn invalidate_tex_region(&mut self, region: &TexRegion) {
        let r = region;
        let wle = (get_field(r.image1, 3, 15) as i32 - 1).max(0);
        let hle = (get_field(r.image1, 3, 18) as i32 - 1).max(0);
        let wlo = (get_field(r.image2, 3, 15) as i32 - 1).max(0);
        let hlo = (get_field(r.image2, 3, 18) as i32 - 1).max(0);
        let mut count = wle + hle;
        if r.is_32b_mipmap != 0 {
            count += wlo + hlo - 2;
        }
        let mut reg0 = 0u32;
        set_field(&mut reg0, 9, 0, get_field(r.image1, 9, 6));
        set_field(&mut reg0, 4, 9, count as u32);
        set_field(&mut reg0, 8, 24, 0x66);
        let mut reg1 = 0u32;
        if wlo != 0 {
            let mut count = wlo + hlo;
            if r.is_32b_mipmap != 0 {
                count += wle + hle - 2;
            }
            set_field(&mut reg1, 9, 0, get_field(r.image2, 9, 6));
            set_field(&mut reg1, 4, 9, count as u32);
            set_field(&mut reg1, 8, 24, 0x66);
        }
        self.flush_texture_state();
        self.write_ras_reg(reg0);
        if wlo != 0 {
            self.write_ras_reg(reg1);
        }
        self.flush_texture_state();
    }

    #[doc(alias = "GXInvalidateTexAll")]
    pub fn invalidate_tex_all(&mut self) {
        self.flush_texture_state();
        self.write_ras_reg(0x6600_1000);
        self.write_ras_reg(0x6600_1100);
        self.flush_texture_state();
    }

    #[doc(alias = "GXSetTexRegionCallback")]
    pub fn set_tex_region_callback(&mut self, f: TexRegionCallback) -> Option<TexRegionCallback> {
        self.data.tex_region_callback.replace(f)
    }

    #[doc(alias = "GXSetTlutRegionCallback")]
    pub fn set_tlut_region_callback(
        &mut self,
        f: TlutRegionCallback,
    ) -> Option<TlutRegionCallback> {
        self.data.tlut_region_callback.replace(f)
    }

    /// `GXPreLoadEntireTexture`.
    pub fn pre_load_entire_texture(&mut self, tex_obj: &TexObj, region: &TexRegion) {
        let t = tex_obj;
        let r = region;
        let is_mip_map = t.flags & 1 == 1;
        let is_32bit = get_field(t.image0, 4, 20) == 6;
        let mut load_image0 = 0u32;
        set_field(&mut load_image0, 8, 24, 0x60);
        let mut base = t.image3 & 0x1F_FFFF;
        set_field(&mut load_image0, 21, 0, base);
        let mut load_image1 = 0u32;
        set_field(&mut load_image1, 8, 24, 0x61);
        let mut tmem1 = r.image1 & 0x7FFF;
        set_field(&mut load_image1, 15, 0, tmem1);
        let mut load_image2 = 0u32;
        set_field(&mut load_image2, 8, 24, 0x62);
        let mut tmem2 = r.image2 & 0x7FFF;
        set_field(&mut load_image2, 15, 0, tmem2);
        let mut load_image3 = 0u32;
        set_field(&mut load_image3, 8, 24, 0x63);
        set_field(&mut load_image3, 15, 0, u32::from(t.load_cnt));
        set_field(&mut load_image3, 2, 15, u32::from(t.load_fmt));
        let mut max_level_index = 0u32;
        let mut n_tiles = u32::from(t.load_cnt);
        let (mut wd, mut ht) = (0, 0);
        if is_mip_map {
            wd = get_field(t.image0, 10, 0) + 1;
            ht = get_field(t.image0, 10, 10) + 1;
            max_level_index = if wd > ht {
                31u32.wrapping_sub(cntlzw(wd)) & 0xFFFF
            } else {
                31u32.wrapping_sub(cntlzw(ht)) & 0xFFFF
            };
        }
        self.flush_texture_state();
        self.write_ras_reg(load_image0);
        self.write_ras_reg(load_image1);
        self.write_ras_reg(load_image2);
        self.write_ras_reg(load_image3);
        if max_level_index != 0 {
            let mut tmem_ar = tmem1;
            let mut tmem_gb = tmem2;
            for i in 0..max_level_index {
                if is_32bit {
                    base += n_tiles * 2;
                    tmem_ar += n_tiles;
                    tmem_gb += n_tiles;
                } else {
                    base += n_tiles;
                    if i & 1 != 0 {
                        tmem_gb += n_tiles;
                    } else {
                        tmem_ar += n_tiles;
                    }
                }
                tmem1 = if i & 1 != 0 { tmem_ar } else { tmem_gb };
                tmem2 = if i & 1 != 0 { tmem_gb } else { tmem_ar };
                let (row_tiles, col_tiles, _) =
                    get_image_tile_count(t.fmt, (wd >> (i + 1)) as u16, (ht >> (i + 1)) as u16);
                n_tiles = row_tiles * col_tiles;
                set_field(&mut load_image0, 21, 0, base);
                set_field(&mut load_image1, 15, 0, tmem1);
                set_field(&mut load_image2, 15, 0, tmem2);
                set_field(&mut load_image3, 15, 0, n_tiles);
                self.write_ras_reg(load_image0);
                self.write_ras_reg(load_image1);
                self.write_ras_reg(load_image2);
                self.write_ras_reg(load_image3);
            }
        }
        self.flush_texture_state();
    }

    /// `GXSetTexCoordScaleManually`.
    pub fn set_tex_coord_scale_manually(&mut self, coord: u32, enable: u8, ss: u16, ts: u16) {
        let g = &mut self.data;
        g.tcs_man_enab = (g.tcs_man_enab & !(1 << coord)) | (u32::from(enable) << coord);
        if enable != 0 {
            let c = coord as usize;
            set_field(&mut g.su_ts0[c], 16, 0, u32::from(ss.wrapping_sub(1)));
            set_field(&mut g.su_ts1[c], 16, 0, u32::from(ts.wrapping_sub(1)));
            let (a, b) = (g.su_ts0[c], g.su_ts1[c]);
            self.write_ras_reg(a);
            self.write_ras_reg(b);
            self.data.bp_sent = 1;
        }
    }

    /// `GXSetTexCoordCylWrap`.
    pub fn set_tex_coord_cyl_wrap(&mut self, coord: u32, s_enable: u8, t_enable: u8) {
        self.set_tex_coord_su_bit(coord, 17, s_enable, t_enable);
    }

    /// `GXSetTexCoordBias`.
    pub fn set_tex_coord_bias(&mut self, coord: u32, s_enable: u8, t_enable: u8) {
        self.set_tex_coord_su_bit(coord, 16, s_enable, t_enable);
    }

    /// `GXSetTexCoordCylWrap`'s and `GXSetTexCoordBias`'s body: sent only for a coordinate
    /// scaled manually.
    fn set_tex_coord_su_bit(&mut self, coord: u32, bit: u32, s_enable: u8, t_enable: u8) {
        let c = coord as usize;
        let g = &mut self.data;
        set_field(&mut g.su_ts0[c], 1, bit, u32::from(s_enable));
        set_field(&mut g.su_ts1[c], 1, bit, u32::from(t_enable));
        if g.tcs_man_enab & (1 << coord) != 0 {
            let (a, b) = (g.su_ts0[c], g.su_ts1[c]);
            self.write_ras_reg(a);
            self.write_ras_reg(b);
            self.data.bp_sent = 1;
        }
    }

    #[doc(alias = "__SetSURegs")]
    fn set_su_regs(&mut self, tmap: u32, tcoord: u32) {
        let g = &mut self.data;
        let (m, c) = (tmap as usize, tcoord as usize);
        let w = get_field(g.t_image0[m], 10, 0);
        let h = get_field(g.t_image0[m], 10, 10);
        set_field(&mut g.su_ts0[c], 16, 0, w);
        set_field(&mut g.su_ts1[c], 16, 0, h);
        let s_bias = u32::from(get_field(g.t_mode0[m], 2, 0) == 1);
        let t_bias = u32::from(get_field(g.t_mode0[m], 2, 2) == 1);
        set_field(&mut g.su_ts0[c], 1, 16, s_bias);
        set_field(&mut g.su_ts1[c], 1, 16, t_bias);
        let (a, b) = (g.su_ts0[c], g.su_ts1[c]);
        self.write_ras_reg(a);
        self.write_ras_reg(b);
        self.data.bp_sent = 0;
    }

    /// `__GXSetSUTexRegs`. ssbm-rs transliterates it from its machine code (for variables a
    /// switch may leave unset); the instructions do what the C says.
    #[doc(alias = "__GXSetSUTexRegs")]
    pub(crate) fn set_su_tex_regs(&mut self) {
        if self.data.tcs_man_enab == 0xFF {
            return;
        }
        let n_stages = get_field(self.data.gen_mode, 4, 10) + 1;
        let n_ind_stages = get_field(self.data.gen_mode, 3, 16);
        // tmap and coord keep their last values for stages past 3, like the machine code's
        // registers.
        let (mut tmap, mut coord) = (0, 0);
        for i in 0..n_ind_stages {
            if i < 4 {
                tmap = get_field(self.data.iref, 3, i * 6);
                coord = get_field(self.data.iref, 3, i * 6 + 3);
            }
            if self.data.tcs_man_enab & (1 << coord) == 0 {
                self.set_su_regs(tmap, coord);
            }
        }
        for i in 0..n_stages {
            let tref = self.data.tref[(i / 2) as usize];
            let map = self.data.texmap_id[i as usize];
            let tmap = map & 0xFFFF_FEFF;
            let coord = if i & 1 != 0 {
                get_field(tref, 3, 15)
            } else {
                get_field(tref, 3, 3)
            };
            if tmap != 0xFF && self.data.tcs_man_enab & (1 << coord) == 0 {
                self.set_su_regs(tmap, coord);
            }
        }
    }

    /// `__GXGetSUTexSize`: (width, height).
    pub(crate) fn get_su_tex_size(&self, coord: u32) -> (u16, u16) {
        let c = coord as usize;
        (
            (self.data.su_ts0[c] as u16).wrapping_add(1),
            (self.data.su_ts1[c] as u16).wrapping_add(1),
        )
    }

    #[doc(alias = "__GXSetTmemConfig")]
    pub(crate) fn set_tmem_config(&mut self, config: u32) {
        const LARGE: [u32; 16] = [
            0x8C0D_8000,
            0x900D_C000,
            0x8D0D_8800,
            0x910D_C800,
            0x8E0D_9000,
            0x920D_D000,
            0x8F0D_9800,
            0x930D_D800,
            0xAC0D_A000,
            0xB00D_E000,
            0xAD0D_A800,
            0xB10D_E800,
            0xAE0D_B000,
            0xB20D_F000,
            0xAF0D_B800,
            0xB30D_F800,
        ];
        const DEFAULT: [u32; 16] = [
            0x8C0D_8000,
            0x900D_C000,
            0x8D0D_8400,
            0x910D_C400,
            0x8E0D_8800,
            0x920D_C800,
            0x8F0D_8C00,
            0x930D_CC00,
            0xAC0D_9000,
            0xB00D_D000,
            0xAD0D_9400,
            0xB10D_D400,
            0xAE0D_9800,
            0xB20D_D800,
            0xAF0D_9C00,
            0xB30D_DC00,
        ];
        let values = if config == 1 { &LARGE } else { &DEFAULT };
        for &v in values {
            self.write_ras_reg(v);
        }
    }
}
