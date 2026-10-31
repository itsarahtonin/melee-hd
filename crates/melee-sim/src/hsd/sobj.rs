//! HAL's sprite objects (`sysdolphin/baselib/sobjlib.c`): images drawn as screen-space quads
//! through an orthographic camera, chained on a GObj by priority. The opening movie draws its
//! frames through one (three planes combined by four TEV stages), and the title its mark.
//!
//! A sprite's object comes from its pool (`HSD_SObjLib_804D10E0`, 0x9C bytes), which every
//! scene's setup makes anew. Drawing is the display pass's: [`Draw::SObj`] hands the sprite to
//! the presentation, which sets GX up as `HSD_SObjLib_803A4A68` does.
//!
//! [`Draw::SObj`]: super::display::Draw::SObj

use melee_data::archive::{Archives, DataRef};

use super::gobj::GObjId;
use super::heap;
use super::pool::{Pool, kind};
use crate::game::handles::CodeRef;

/// The GObj object kind sprites register as (`HSD_SObjLib_804D7960`): the scene's setup
/// registers it first, before `HSD_GObjInit` adds the library's own four (camera, light,
/// joint, fog) after it.
pub const OBJ_SOBJ: u8 = 0;

/// `HSD_SObjLib_803A49E0`, a sprite GObj's render callback.
pub const RENDER: u32 = 0x803A_49E0;
/// `HSD_SObjLib_803A54EC`, the sprites' camera's render callback.
pub const CAMERA_RENDER: u32 = 0x803A_54EC;

/// A sprite's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct SObjId(pub u32);

/// Where a sprite's image comes from: an `HSD_ImageDesc` in loaded data, or the movie
/// player's descriptor in the executable (`lbl_803BAFE8`, no image of its own: the movie's
/// planes are loaded as textures before it draws).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Image {
    #[default]
    None,
    Data(DataRef),
    Movie {
        width: u16,
        height: u16,
    },
    /// An `HSD_ImageDesc` in the executable whose image is a heap block the game decodes into
    /// (the Yoshiki Test's snapshot, `un_803FD274`): the block's address, the image's size and
    /// format.
    Buffer {
        addr: u32,
        width: u16,
        height: u16,
        format: u32,
    },
}

/// The image's texture as `GXInitTexObj` sets it up: its size and format, and the wrap modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Tex {
    pub width: u16,
    pub height: u16,
    pub format: u32,
    pub mipmap: u8,
    pub wrap_s: u32,
    pub wrap_t: u32,
    /// The palette (`HSD_Tlut` in loaded data) for a color-indexed format.
    pub tlut: Option<DataRef>,
}

/// `HSD_SObj`.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SObj {
    pub x0: u32,
    pub next: Option<SObjId>,
    pub prev: Option<SObjId>,
    pub gobj: Option<GObjId>,
    /// The top-left corner (or the center, with flag 2), rotation and scale.
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// The texture coordinates' corners.
    pub s0: f32,
    pub t0: f32,
    pub s1: f32,
    pub t1: f32,
    /// The quad's size in pixels.
    pub width: u16,
    pub height: u16,
    /// The intensity formats' two colors, and the color others multiply by.
    pub color0: [u8; 4],
    pub color1: [u8; 4],
    /// 1 hidden, 2 placed by its center, 4 with a depth texture, 8 not writing depth, 0x10
    /// the movie's YUV planes.
    pub flags: u32,
    pub priority: u8,
    pub x48: u32,
    pub callback: Option<CodeRef>,
    pub image: Image,
    pub tex: Tex,
    pub image2: Option<DataRef>,
    /// The texture object has been loaded to texture map 0 (`GXLoadTexObj`, which writes the
    /// map's register IDs into the object's top bytes), as the sprite's first draw does.
    pub loaded: bool,
}

/// The sprites' pool.
pub type SObjs = Pool<SObj, { kind(heap::Kind::SObj) }>;

/// `__GXTexObjInt_struct`, the private layout of `GXTexObj`, as GX's `GXInitTexObj` packs it:
/// a sprite's own (its block holds them, which the heap's image lays out), and melee-render's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GxTexObj {
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

/// `SET_REG_FIELD`.
fn set_field(reg: &mut u32, size: u32, shift: u32, val: u32) {
    let mask = ((1u32 << size) - 1) << shift;
    *reg = (*reg & !mask) | (val << shift);
}

/// `GXInitTexObj`'s object: `image_ptr` is the image's address, which the object keeps in
/// 32-byte units, `(addr >> 5) & 0x01FFFFFF`.
#[allow(clippy::too_many_arguments)]
pub fn gx_init_tex_obj(
    image_ptr: u32,
    width: u16,
    height: u16,
    format: u32,
    wrap_s: u32,
    wrap_t: u32,
    mipmap: u8,
) -> GxTexObj {
    let mut t = GxTexObj::default();
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
            31u32.wrapping_sub(u32::from(width).leading_zeros())
        } else {
            31u32.wrapping_sub(u32::from(height).leading_zeros())
        };
        let lmax = u32::from(gekko_fp::fctiwz(gekko_fp::fmuls(
            16.0,
            gekko_fp::frsp(f64::from(max_lod)),
        )) as u8);
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
    t
}

/// The texture map registers' IDs `GXLoadTexObj` writes into an object's top bytes for map
/// `id` (`mode0`, `mode1`, `image0`, `image3`).
pub const TEX_MODE0_IDS: [u8; 8] = [0x80, 0x81, 0x82, 0x83, 0xA0, 0xA1, 0xA2, 0xA3];
pub const TEX_MODE1_IDS: [u8; 8] = [0x84, 0x85, 0x86, 0x87, 0xA4, 0xA5, 0xA6, 0xA7];
pub const TEX_IMAGE0_IDS: [u8; 8] = [0x88, 0x89, 0x8A, 0x8B, 0xA8, 0xA9, 0xAA, 0xAB];
pub const TEX_IMAGE3_IDS: [u8; 8] = [0x94, 0x95, 0x96, 0x97, 0xB4, 0xB5, 0xB6, 0xB7];

impl GxTexObj {
    /// The object as `GXLoadTexObj` leaves it in memory for texture map `id`: the registers'
    /// IDs in its words' top bytes.
    pub fn loaded(mut self, id: usize) -> GxTexObj {
        set_field(&mut self.mode0, 8, 24, u32::from(TEX_MODE0_IDS[id]));
        set_field(&mut self.mode1, 8, 24, u32::from(TEX_MODE1_IDS[id]));
        set_field(&mut self.image0, 8, 24, u32::from(TEX_IMAGE0_IDS[id]));
        set_field(&mut self.image3, 8, 24, u32::from(TEX_IMAGE3_IDS[id]));
        self
    }
}

/// `HSD_ImageDesc`'s fields: the image, its size and format, and whether it has mipmaps.
fn image_desc(data: &Archives, d: DataRef) -> (u16, u16, u32, u8) {
    (
        data.u16(d.at(4)),
        data.u16(d.at(6)),
        data.u32(d.at(8)),
        data.u32(d.at(0xC)) as u8,
    )
}

/// Links `s` into `head`'s chain by priority after those of its priority or less,
/// `HSD_SObjLib_803A44D4`; returns the chain's new head. (The original unlinks a sprite
/// already chained first; nothing calls it on one.)
#[doc(alias = "HSD_SObjLib_803A44D4")]
fn link(sobjs: &mut SObjs, head: Option<SObjId>, s: SObjId, priority: u8) -> SObjId {
    let new_head = match head {
        None => s,
        Some(first) => {
            let mut next = first;
            while let Some(n) = sobjs[next.0].next
                && sobjs[next.0].priority <= priority
            {
                next = n;
            }
            if sobjs[next.0].next.is_none() && sobjs[next.0].priority <= priority {
                sobjs[next.0].next = Some(s);
                sobjs[s.0].prev = Some(next);
                first
            } else if next == first {
                sobjs[s.0].next = Some(first);
                sobjs[first.0].prev = Some(s);
                s
            } else {
                let p = sobjs[next.0].prev.expect("a sprite before");
                sobjs[p.0].next = Some(s);
                sobjs[s.0].prev = Some(p);
                sobjs[next.0].prev = Some(s);
                sobjs[s.0].next = Some(next);
                first
            }
        }
    };
    sobjs[s.0].priority = priority;
    new_head
}

/// A sprite of `desc`'s image on GObj `gobj`, whose chain starts at `head`,
/// `HSD_SObjLib_803A477C`; returns it and the chain's head. `desc` is an `HSD_SObjDesc` in
/// loaded data, or the movie's descriptor.
#[doc(alias = "HSD_SObjLib_803A477C")]
#[allow(clippy::too_many_arguments)]
pub fn create(
    sobjs: &mut SObjs,
    data: &Archives,
    gobj: GObjId,
    head: Option<SObjId>,
    desc: Image,
    wrap: (u32, u32),
    priority: u8,
    secondary: Option<DataRef>,
) -> (SObjId, SObjId) {
    let (width, height, tex) = match desc {
        Image::Data(d) => {
            let image = data.ptr(d).expect("a sprite's image");
            let tlut = data.ptr(d.at(4));
            let (w, h, f, m) = image_desc(data, image);
            (
                w,
                h,
                Tex {
                    width: w,
                    height: h,
                    format: f,
                    mipmap: m,
                    wrap_s: wrap.0,
                    wrap_t: wrap.1,
                    tlut,
                },
            )
        }
        Image::Movie { width, height } => (
            width,
            height,
            Tex {
                width,
                height,
                // `GX_TF_RGBA8`, as the descriptor says; the movie's planes replace it.
                format: 6,
                mipmap: 0,
                wrap_s: wrap.0,
                wrap_t: wrap.1,
                tlut: None,
            },
        ),
        Image::Buffer {
            width,
            height,
            format,
            ..
        } => (
            width,
            height,
            Tex {
                width,
                height,
                format,
                mipmap: 0,
                wrap_s: wrap.0,
                wrap_t: wrap.1,
                tlut: None,
            },
        ),
        Image::None => panic!("a sprite without an image"),
    };
    let image = match desc {
        Image::Data(d) => Image::Data(data.ptr(d).expect("a sprite's image")),
        other => other,
    };
    let s = SObjId(sobjs.alloc(SObj {
        gobj: Some(gobj),
        scale_x: 1.0,
        scale_y: 1.0,
        color0: [0xFF; 4],
        color1: [0xFF; 4],
        flags: if secondary.is_some() { 4 } else { 0 },
        width,
        height,
        image,
        tex,
        image2: secondary.and_then(|d| data.ptr(d.at(8))),
        ..SObj::default()
    }));
    // `1.0F / (f32) GXGetTexObjWidth(...)`, then the image's size times it.
    let inv_w = gekko_fp::fdivs(1.0, gekko_fp::frsp(f64::from(tex.width)));
    let inv_h = gekko_fp::fdivs(1.0, gekko_fp::frsp(f64::from(tex.height)));
    let o = &mut sobjs[s.0];
    o.s1 = gekko_fp::fmuls(gekko_fp::frsp(f64::from(width)), inv_w) as f32;
    o.t1 = gekko_fp::fmuls(gekko_fp::frsp(f64::from(height)), inv_h) as f32;
    let head = link(sobjs, head, s, priority);
    (s, head)
}

/// Frees a sprite and unlinks it, `HSD_SObjLib_803A466C`; returns its chain's new head when
/// it was the head.
#[doc(alias = "HSD_SObjLib_803A466C")]
pub fn free(sobjs: &mut SObjs, s: SObjId) -> Option<Option<SObjId>> {
    let (prev, next) = (sobjs[s.0].prev, sobjs[s.0].next);
    let mut head = None;
    if prev.is_some() || next.is_some() {
        match prev {
            Some(p) => sobjs[p.0].next = next,
            None => {
                let n = next.expect("a sprite after");
                sobjs[n.0].prev = None;
                head = Some(Some(n));
            }
        }
        match next {
            Some(n) => sobjs[n.0].prev = prev,
            None => sobjs[prev.expect("a sprite before").0].next = None,
        }
        sobjs[s.0].next = None;
        sobjs[s.0].prev = None;
    } else {
        head = Some(None);
    }
    sobjs.free(s.0);
    head
}

/// Frees a GObj's chain of sprites from `s`, `HSD_SObjLib_803A4740`, the object kind's release.
#[doc(alias = "HSD_SObjLib_803A4740")]
pub fn free_all(sobjs: &mut SObjs, s: Option<SObjId>) {
    let mut cur = s;
    while let Some(c) = cur {
        cur = sobjs[c.0].next;
        free(sobjs, c);
    }
}
