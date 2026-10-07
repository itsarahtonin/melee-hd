//! Texture objects, HAL's TObj (`sysdolphin/baselib/tobj.c`): one texture of a material, with
//! its coordinate transform (scale, rotation, translation and the matrix made from them), its
//! image and palette, an optional TEV color stage, and the animation that swaps images and
//! palettes and moves its transform and colors.
//!
//! What the display pass does with a TObj (texture coordinate generation, TEV stages, texture
//! and palette loads, `HSD_TObjSetup`'s GX calls) isn't ported yet; [`setup`] and
//! [`setup_mtx`] port the state those functions change.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj;
use super::pool::Pool;
use crate::fpu::{copy, mem, reg};
use crate::mtx::Mtx34;
use crate::quat::Quat;
use crate::vec::Vec3;

/// A TObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TObjId(pub u32);

/// The texture-animation flag of `HSD_*ReqAnimByFlags` (`TOBJ_ANIM`).
pub const TOBJ_ANIM: u32 = 0x10;

/// `flags`: how texture coordinates are generated (`TEX_COORD_*`).
pub const COORD_MASK: u32 = 0xF;
pub const COORD_UV: u32 = 0;
pub const COORD_REFLECTION: u32 = 1;
pub const COORD_HILIGHT: u32 = 2;
pub const COORD_SHADOW: u32 = 3;
pub const COORD_TOON: u32 = 4;
/// `flags`: a bump map (`TEX_BUMP`).
pub const BUMP: u32 = 1 << 24;
/// `flags`: the matrix must be remade from the transform (`TEX_MTX_DIRTY`).
pub const MTX_DIRTY: u32 = 1 << 31;

/// GX's texture map, coordinate, matrix and generation-source numbers.
pub const GX_TEXMAP_NULL: u32 = 0xFF;
const GX_TEXMAP7: u32 = 7;
const GX_TEXMTX0: u32 = 30;
const GX_TEXMTX9: u32 = 57;
pub const GX_IDENTITY: u32 = 60;
const GX_PTTEXMTX0: u32 = 64;
const GX_TG_TEX0: u32 = 4;
const GX_TG_TEXCOORD0: u32 = 12;
const GX_TG_COLOR0: u32 = 19;
const GX_MIRROR: u32 = 2;
const GX_LINEAR: u32 = 1;

/// Animated channels (`HSD_A_T_*`).
const A_TIMG: u8 = 1;
const A_TRAU: u8 = 2;
const A_TRAV: u8 = 3;
const A_SCAU: u8 = 4;
const A_SCAV: u8 = 5;
const A_ROTX: u8 = 6;
const A_ROTY: u8 = 7;
const A_ROTZ: u8 = 8;
const A_BLEND: u8 = 9;
const A_TCLT: u8 = 10;
const A_LOD_BIAS: u8 = 11;
const A_KONST_R: u8 = 12;
const A_TEV1_A: u8 = 23;
const A_TS_BLEND: u8 = 24;

/// `FLT_EPSILON` as tobj.c defines it (1.00000001335e-10F), below which a scale counts as zero.
const FLT_EPSILON: f32 = f32::from_bits(0x2EDB_E6FF);

/// `GXColor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    fn read(data: &Archives, at: DataRef) -> Color {
        Color {
            r: data.u8(at),
            g: data.u8(at.at(1)),
            b: data.u8(at.at(2)),
            a: data.u8(at.at(3)),
        }
    }
}

/// An image descriptor a TObj shows (`HSD_ImageDesc*`): one in loaded game data, or one in the
/// game executable's own data (`HSD_MObjSetToonTextureImage`'s callers pass a static one), by
/// address. Images the game makes at run time (EFB copies) will be further variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRef {
    Data(DataRef),
    Static(u32),
}

/// A texture animation's palettes (`tluttbl`), by its table of `HSD_TlutDesc` pointers.
///
/// The original copies each palette descriptor into an `HSD_Tlut`; the copies differ from the
/// descriptors only in `tlut_name`, which `HSD_TObjSetup` writes before every read, so a TObj
/// refers to the descriptors instead (as it does for `tlut`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlutTable {
    pub table: DataRef,
    pub len: u16,
}

/// `HSD_TObjTev`: a TEV stage that colors the texture with constant and register colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TObjTev {
    pub color_op: u8,
    pub alpha_op: u8,
    pub color_bias: u8,
    pub alpha_bias: u8,
    pub color_scale: u8,
    pub alpha_scale: u8,
    pub color_clamp: u8,
    pub alpha_clamp: u8,
    /// `color_a` to `color_d`.
    pub color_in: [u8; 4],
    /// `alpha_a` to `alpha_d`.
    pub alpha_in: [u8; 4],
    pub konst: Color,
    pub tev0: Color,
    pub tev1: Color,
    pub active: u32,
}

impl TObjTev {
    /// A copy of a descriptor, `HSD_TObjTevLoadDesc` (into a zeroed `HSD_TObjTevAlloc`).
    #[doc(alias = "HSD_TObjTevLoadDesc")]
    #[doc(alias = "HSD_TObjTevAlloc")]
    pub fn load(data: &Archives, desc: Option<DataRef>) -> Option<TObjTev> {
        let d = desc?;
        let b = |o| data.u8(d.at(o));
        Some(TObjTev {
            color_op: b(0),
            alpha_op: b(1),
            color_bias: b(2),
            alpha_bias: b(3),
            color_scale: b(4),
            alpha_scale: b(5),
            color_clamp: b(6),
            alpha_clamp: b(7),
            color_in: [b(8), b(9), b(0xA), b(0xB)],
            alpha_in: [b(0xC), b(0xD), b(0xE), b(0xF)],
            konst: Color::read(data, d.at(0x10)),
            tev0: Color::read(data, d.at(0x14)),
            tev1: Color::read(data, d.at(0x18)),
            active: data.u32(d.at(0x1C)),
        })
    }
}

/// `HSD_TObj`. Its `HSD_Obj` reference counts are never used for TObjs, so they aren't kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TObj {
    pub next: Option<TObjId>,
    /// `GXTexMapID`, reassigned by [`assign_resources`].
    pub id: u32,
    /// `GXTexGenSrc`.
    pub src: u32,
    /// `GXTexMtx` or `GXPTTexMtx`.
    pub mtxid: u32,
    /// Euler angles in x, y and z; `w` is never set.
    pub rotate: Quat,
    pub scale: Vec3,
    pub translate: Vec3,
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub repeat_s: u8,
    pub repeat_t: u8,
    pub flags: u32,
    pub blending: f32,
    pub mag_filt: u32,
    pub imagedesc: Option<ImageRef>,
    /// The palette's `HSD_TlutDesc` (see [`TlutTable`]).
    pub tlut: Option<DataRef>,
    /// `HSD_TexLODDesc`, in loaded data.
    pub lod: Option<DataRef>,
    pub aobj: Option<AObjId>,
    /// The texture animation's image table (`HSD_ImageDesc*[]`).
    pub imagetbl: Option<DataRef>,
    pub tluttbl: Option<TlutTable>,
    /// The palette animation's choice from `tluttbl`, 0xFF for `tlut`.
    pub tlut_no: u8,
    pub mtx: Mtx34,
    /// `GXTexCoordID`.
    pub coord: u32,
    pub tev: Option<TObjTev>,
}

/// The TObj pool and `tobj_head`.
#[derive(Clone, Debug, Default)]
pub struct TObjs {
    pub pool: Pool<TObj>,
    /// The chain the display pass last set up (`tobj_head`).
    pub head: Option<TObjId>,
}

impl Index<TObjId> for TObjs {
    type Output = TObj;
    fn index(&self, t: TObjId) -> &TObj {
        &self.pool[t.0]
    }
}

impl IndexMut<TObjId> for TObjs {
    fn index_mut(&mut self, t: TObjId) -> &mut TObj {
        &mut self.pool[t.0]
    }
}

impl TObjs {
    /// A chain's TObjs, first to last.
    pub fn chain(&self, first: Option<TObjId>) -> impl Iterator<Item = TObjId> + '_ {
        std::iter::successors(first, |&t| self[t].next)
    }
}

/// A new, zeroed TObj, `HSD_TObjAlloc`.
///
/// The original allocates HAL class objects from `hsdAllocMemPiece`, whose free lists are
/// shared by every object of a size class, so its addresses don't follow a TObj pool. Only live
/// TObjs' identities are compared (the shadow list, [`assign_resources`]), which a pool keeps.
#[doc(alias = "HSD_TObjAlloc")]
pub fn alloc(tobjs: &mut TObjs) -> TObjId {
    TObjId(tobjs.pool.alloc(TObj::default()))
}

/// Returns a TObj to the pool without releasing what it holds (its AObj stays allocated),
/// `HSD_TObjFree`.
#[doc(alias = "HSD_TObjFree")]
pub fn free(tobjs: &mut TObjs, t: Option<TObjId>) {
    if let Some(t) = t {
        tobjs.pool.free(t.0);
    }
}

/// Frees a TObj's animation, `TObjRelease` (its palettes and TEV stage are values).
#[doc(alias = "TObjRelease")]
fn release(objs: &mut Objects, t: TObjId) {
    jobj::remove_aobj(objs, objs.tobjs[t].aobj);
}

/// Frees a chain of TObjs, first first, `HSD_TObjRemoveAll`.
#[doc(alias = "HSD_TObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, t: Option<TObjId>) {
    let mut cur = t;
    while let Some(t) = cur {
        cur = objs.tobjs[t].next;
        release(objs, t);
        objs.tobjs.pool.free(t.0);
    }
}

/// `HSD_TObjGetNext`.
#[doc(alias = "HSD_TObjGetNext")]
pub fn next(tobjs: &TObjs, t: Option<TObjId>) -> Option<TObjId> {
    t.and_then(|t| tobjs[t].next)
}

/// Builds a chain of TObjs from a chain of descriptors, `HSD_TObjLoadDesc`. Each TObj is
/// allocated before those after it.
///
/// A descriptor naming a class (`class_name`) would be built by that class, found with
/// `hsdSearchClassInfo`; none on the disc does, so such a descriptor panics here.
#[doc(alias = "HSD_TObjLoadDesc")]
#[doc(alias = "TObjLoad")]
pub fn load_desc(tobjs: &mut TObjs, data: &Archives, desc: Option<DataRef>) -> Option<TObjId> {
    let td = desc?;
    assert!(
        data.ptr(td).is_none(),
        "TObjDesc {td:?} names a class, which melee-sim doesn't look up"
    );
    let t = alloc(tobjs);
    let next = load_desc(tobjs, data, data.ptr(td.at(4)));
    let f = |o| data.f32(td.at(o));
    let raw = |o| f32::from_bits(data.u32(td.at(o)));
    let o = &mut tobjs[t];
    o.next = next;
    o.id = data.u32(td.at(8));
    o.src = data.u32(td.at(0xC));
    o.mtxid = GX_IDENTITY;
    o.rotate.x = copy(f(0x10));
    o.rotate.y = copy(f(0x14));
    o.rotate.z = copy(f(0x18));
    // Struct assignments: copied as words.
    o.scale = Vec3::new(raw(0x1C), raw(0x20), raw(0x24));
    o.translate = Vec3::new(raw(0x28), raw(0x2C), raw(0x30));
    o.wrap_s = data.u32(td.at(0x34));
    o.wrap_t = data.u32(td.at(0x38));
    o.repeat_s = data.u8(td.at(0x3C));
    o.repeat_t = data.u8(td.at(0x3D));
    o.flags = data.u32(td.at(0x40));
    o.blending = copy(f(0x44));
    o.mag_filt = data.u32(td.at(0x48));
    o.imagedesc = data.ptr(td.at(0x4C)).map(ImageRef::Data);
    o.tlut = data.ptr(td.at(0x50));
    o.lod = data.ptr(td.at(0x54));
    o.aobj = None;
    o.flags |= MTX_DIRTY;
    o.tlut_no = 0xFF;
    o.tev = TObjTev::load(data, data.ptr(td.at(0x58)));
    Some(t)
}

/// The toon-shading TObj `HSD_MObjSetToonTextureImage` makes: `HSD_TObjLoadDesc` of mobj.c's
/// static `tobj_toon_desc` with `image`.
pub(crate) fn load_toon(tobjs: &mut TObjs, image: Option<ImageRef>) -> TObjId {
    let t = alloc(tobjs);
    let o = &mut tobjs[t];
    o.id = GX_TEXMAP7;
    o.src = GX_TG_COLOR0;
    o.mtxid = GX_IDENTITY;
    o.flags = COORD_TOON | MTX_DIRTY;
    o.blending = 1.0;
    o.mag_filt = GX_LINEAR;
    o.imagedesc = image;
    o.tlut_no = 0xFF;
    t
}

/// Frees a TObj's animation, the inline `HSD_TObjRemoveAnim`, for each TObj of a chain,
/// `HSD_TObjRemoveAnimAll`.
#[doc(alias = "HSD_TObjRemoveAnimAll")]
pub fn remove_anim_all(objs: &mut Objects, t: Option<TObjId>) {
    let mut cur = t;
    while let Some(t) = cur {
        jobj::remove_aobj(objs, objs.tobjs[t].aobj);
        objs.tobjs[t].aobj = None;
        cur = objs.tobjs[t].next;
    }
}

/// The first texture animation for map `id` in a chain of them, `lookupTextureAnim`.
pub fn lookup_texanim(data: &Archives, id: u32, texanim: Option<DataRef>) -> Option<DataRef> {
    let mut ta = texanim;
    while let Some(a) = ta {
        if data.u32(a.at(4)) == id {
            return Some(a);
        }
        ta = data.ptr(a);
    }
    None
}

/// Gives a TObj the animation for its texture map from a chain of `HSD_TexAnim`s, if it has
/// one, `HSD_TObjAddAnim`.
#[doc(alias = "HSD_TObjAddAnim")]
pub fn add_anim(objs: &mut Objects, data: &Archives, t: Option<TObjId>, texanim: Option<DataRef>) {
    let Some(t) = t else { return };
    let Some(ta) = lookup_texanim(data, objs.tobjs[t].id, texanim) else {
        return;
    };
    if let Some(a) = objs.tobjs[t].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    let a = jobj::load_aobj(objs, data, data.ptr(ta.at(8)));
    let o = &mut objs.tobjs[t];
    o.aobj = a;
    o.imagetbl = data.ptr(ta.at(0xC));
    let len = data.u16(ta.at(0x16));
    o.tluttbl = (len != 0).then(|| TlutTable {
        // A count with no table would make the original read through null.
        table: data.ptr(ta.at(0x10)).expect("a TexAnim's palettes"),
        len,
    });
    o.tlut_no = 0xFF;
}

/// [`add_anim`] for each TObj of a chain, `HSD_TObjAddAnimAll`.
#[doc(alias = "HSD_TObjAddAnimAll")]
pub fn add_anim_all(
    objs: &mut Objects,
    data: &Archives,
    t: Option<TObjId>,
    texanim: Option<DataRef>,
) {
    let mut cur = t;
    while let Some(t) = cur {
        add_anim(objs, data, Some(t), texanim);
        cur = objs.tobjs[t].next;
    }
}

/// Restarts each TObj's animation of a chain at `frame` if `flags` has [`TOBJ_ANIM`],
/// `HSD_TObjReqAnimAllByFlags` (each through the inline `HSD_TObjReqAnimByFlags`).
#[doc(alias = "HSD_TObjReqAnimAllByFlags")]
pub fn req_anim_all_by_flags(objs: &mut Objects, t: Option<TObjId>, frame: f32, flags: u32) {
    let mut cur = t;
    while let Some(t) = cur {
        if flags & TOBJ_ANIM != 0 {
            aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, objs.tobjs[t].aobj, frame);
        }
        cur = objs.tobjs[t].next;
    }
}

/// Restarts a TObj's animation at `frame`, `HSD_TObjReqAnim`.
#[doc(alias = "HSD_TObjReqAnim")]
pub fn req_anim(objs: &mut Objects, t: Option<TObjId>, frame: f32) {
    if let Some(t) = t {
        aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, objs.tobjs[t].aobj, frame);
    }
}

/// Restarts each TObj's animation of a chain at `frame`, `HSD_TObjReqAnimAll`.
#[doc(alias = "HSD_TObjReqAnimAll")]
pub fn req_anim_all(objs: &mut Objects, t: Option<TObjId>, frame: f32) {
    req_anim_all_by_flags(objs, t, frame, TOBJ_ANIM);
}

/// A color channel's value as the original stores it: `(u8) (255.0 * value)`.
pub(crate) fn color_channel(v: f32) -> u8 {
    fp::fctiwz(fp::fmul(255.0, reg(v))) as u8
}

/// Hands an animated value to a TObj, `TObjUpdateFunc`.
///
/// A stale value (`Value::Stale`) is a RAM address, a tiny negative float: as an image or
/// palette index or a color it gives 0 whatever the address, and elsewhere it's stored as
/// [`Value::or_address`] and counted in [`Objects::stale_reads`].
#[doc(alias = "TObjUpdateFunc")]
pub fn update(objs: &mut Objects, data: &Archives, t: TObjId, ty: u8, value: Value) {
    let v = value.or_address();
    let stale = u32::from(value == Value::Stale);
    let o = &mut objs.tobjs.pool[t.0];
    match ty {
        A_TIMG => {
            let tbl = o
                .imagetbl
                .expect("an image animation without an image table");
            let n = fp::fctiwz(reg(v));
            let slot = DataRef {
                archive: tbl.archive,
                offset: tbl.offset.wrapping_add((n as u32).wrapping_mul(4)),
            };
            if let Some(image) = data.ptr(slot) {
                o.imagedesc = Some(ImageRef::Data(image));
            }
        }
        A_TCLT => {
            if o.tluttbl.is_some() {
                o.tlut_no = fp::fctiwz(reg(v)) as u8;
            }
        }
        A_BLEND | A_TS_BLEND => {
            o.blending = copy(v);
            objs.stale_reads += stale;
        }
        A_TRAU..=A_ROTZ => {
            let field = match ty {
                A_ROTX => &mut o.rotate.x,
                A_ROTY => &mut o.rotate.y,
                A_ROTZ => &mut o.rotate.z,
                A_TRAU => &mut o.translate.x,
                A_TRAV => &mut o.translate.y,
                A_SCAU => &mut o.scale.x,
                A_SCAV => &mut o.scale.y,
                _ => unreachable!(),
            };
            *field = copy(v);
            o.flags |= MTX_DIRTY;
            objs.stale_reads += stale;
        }
        // The original writes the bias into the loaded LOD descriptor, shared by every TObj
        // loaded from it. No animation on the disc has the channel.
        A_LOD_BIAS => panic!("a TObj animates its LOD bias, which writes into loaded data"),
        A_KONST_R..=A_TEV1_A => {
            let tev = o
                .tev
                .as_mut()
                .expect("a TEV color animation without a TEV stage");
            let c = color_channel(v);
            let i = ty - A_KONST_R;
            let color = match i / 4 {
                0 => &mut tev.konst,
                1 => &mut tev.tev0,
                _ => &mut tev.tev1,
            };
            match i % 4 {
                0 => color.r = c,
                1 => color.g = c,
                2 => color.b = c,
                _ => color.a = c,
            }
        }
        _ => {}
    }
}

/// Advances a TObj's animation one frame, `HSD_TObjAnim`.
#[doc(alias = "HSD_TObjAnim")]
pub fn anim(objs: &mut Objects, data: &Archives, t: Option<TObjId>) {
    let Some(t) = t else { return };
    let a = objs.tobjs[t].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, data, t, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
}

/// [`anim`] for each TObj of a chain, `HSD_TObjAnimAll`.
#[doc(alias = "HSD_TObjAnimAll")]
pub fn anim_all(objs: &mut Objects, data: &Archives, t: Option<TObjId>) {
    let mut cur = t;
    while let Some(t) = cur {
        anim(objs, data, Some(t));
        cur = objs.tobjs[t].next;
    }
}

/// The TObj after `from` (or the first of [`TObjs::head`]) whose coordinates are generated as
/// `mapping`, `_HSD_TObjGetCurrentByType`.
#[doc(alias = "_HSD_TObjGetCurrentByType")]
pub fn current_by_type(tobjs: &TObjs, from: Option<TObjId>, mapping: u32) -> Option<TObjId> {
    let first = match from {
        None => tobjs.head,
        Some(f) => tobjs[f].next,
    };
    tobjs
        .chain(first)
        .find(|&t| tobjs[t].flags & COORD_MASK == mapping)
}

/// The texture map's position-texture matrix, `HSD_TexMapID2PTTexMtx`.
#[doc(alias = "HSD_TexMapID2PTTexMtx")]
pub fn texmap_pt_texmtx(id: u32) -> u32 {
    assert!(id < 8, "unexpected texmap id {id}");
    GX_PTTEXMTX0 + 3 * id
}

/// Makes a TObj's texture matrix from its scale (with its repeats), rotation and translation,
/// `MakeTextureMtx`.
#[doc(alias = "MakeTextureMtx")]
pub fn make_mtx(tobjs: &mut TObjs, t: TObjId) {
    let o = &mut tobjs[t];
    assert!(
        o.repeat_s != 0 && o.repeat_t != 0,
        "a texture without repeats"
    );
    let eps = reg(FLT_EPSILON);
    let repeat_s = fp::frsp(f64::from(o.repeat_s));
    let repeat_t = fp::frsp(f64::from(o.repeat_t));
    let ratio = |repeat: f64, s: f32| {
        if fp::fabs(reg(s)) < eps {
            0.0
        } else {
            mem(fp::fdivs(repeat, reg(s)))
        }
    };
    let scale = Vec3::new(
        ratio(repeat_s, o.scale.x),
        ratio(repeat_t, o.scale.y),
        copy(o.scale.z),
    );
    let rot = Vec3::new(
        copy(o.rotate.x),
        copy(o.rotate.y),
        mem(fp::fneg(reg(o.rotate.z))),
    );
    let mirror = if o.wrap_t == GX_MIRROR {
        fp::fdivs(1.0, fp::fdivs(repeat_t, reg(o.scale.y)))
    } else {
        0.0
    };
    let trans = Vec3::new(
        mem(fp::fneg(reg(o.translate.x))),
        mem(fp::fneg(fp::fadds(reg(o.translate.y), mirror))),
        copy(o.translate.z),
    );
    let mtx = Mtx34::translation(trans.x, trans.y, trans.z);
    let mtx = Mtx34::from_euler(rot).concat(&mtx);
    o.mtx = Mtx34::scale(scale.x, scale.y, scale.z).concat(&mtx);
}

/// Remakes a TObj's matrix if its transform changed, `TObjSetupMtx`. A toon texture has none.
///
/// The original then loads the matrix GX uses, adjusted for reflection, highlight (from the
/// current light and camera) or shadow coordinates: that's the display pass's.
#[doc(alias = "TObjSetupMtx")]
pub fn setup_mtx(tobjs: &mut TObjs, t: TObjId) {
    if tobjs[t].flags & COORD_MASK == COORD_TOON {
        return;
    }
    if tobjs[t].flags & MTX_DIRTY != 0 {
        make_mtx(tobjs, t);
        tobjs[t].flags &= !MTX_DIRTY;
    }
}

/// Assigns a chain's texture maps, matrices and coordinates in order, keeping room for one
/// toon and one bump texture; the others past the hardware's eight maps get none.
/// Returns how many coordinates it used, `HSD_TObjAssignResources`.
#[doc(alias = "HSD_TObjAssignResources")]
pub fn assign_resources(tobjs: &mut TObjs, top: Option<TObjId>) -> u32 {
    if top.is_none() {
        return 0;
    }
    let coord = |t: &TObj| t.flags & COORD_MASK;
    let (mut toon, mut bump) = (None, None);
    let mut cur = top;
    while let Some(t) = cur {
        if coord(&tobjs[t]) == COORD_TOON {
            toon = Some(t);
        } else if tobjs[t].flags & BUMP != 0 {
            bump = Some(t);
        }
        cur = tobjs[t].next;
    }
    let mut limit = 8u32;
    if toon.is_some() {
        limit -= 1;
    }
    if bump.is_some() {
        limit -= 2;
    }
    let (mut texmap_no, mut texcoord_no) = (0u32, 0u32);
    let mut cur = top;
    while let Some(t) = cur {
        let o = &mut tobjs[t];
        if coord(o) == COORD_TOON {
            if Some(t) != toon {
                o.id = GX_TEXMAP_NULL;
            }
        } else if o.flags & BUMP != 0 {
            if Some(t) != bump {
                o.id = GX_TEXMAP_NULL;
            }
        } else if texmap_no < limit {
            o.id = index_to_texmap(texmap_no);
            texmap_no += 1;
            o.mtxid = texmap_pt_texmtx(o.id);
            if matches!(coord(o), COORD_REFLECTION | COORD_HILIGHT | COORD_SHADOW) {
                o.coord = index_to_texcoord(texcoord_no);
                texcoord_no += 1;
            }
        } else {
            o.id = GX_TEXMAP_NULL;
        }
        cur = o.next;
    }
    let mut cur = top;
    while let Some(t) = cur {
        let o = &mut tobjs[t];
        if o.id != GX_TEXMAP_NULL && o.flags & BUMP == 0 && coord(o) == COORD_UV {
            o.coord = index_to_texcoord(texcoord_no);
            texcoord_no += 1;
        }
        cur = o.next;
    }
    if let Some(b) = bump {
        let o = &mut tobjs[b];
        o.id = index_to_texmap(texmap_no);
        texmap_no += 1;
        o.mtxid = GX_TEXMTX9;
        o.coord = index_to_texcoord(texcoord_no);
        texcoord_no += 2;
    }
    if let Some(t) = toon {
        let o = &mut tobjs[t];
        o.id = index_to_texmap(texmap_no);
        o.coord = index_to_texcoord(texcoord_no);
        texcoord_no += 1;
    }
    texcoord_no
}

/// The state `HSD_TObjSetup` changes as the display pass sets up a chain for drawing: it
/// becomes [`TObjs::head`], its resources are assigned, and each TObj with a texture map has
/// its matrix remade if needed.
///
/// The original then loads each texture and palette into GX; the palettes' `tlut_name`s it
/// assigns are written before every read (see [`TlutTable`]). That's the display pass's.
#[doc(alias = "HSD_TObjSetup")]
pub fn setup(tobjs: &mut TObjs, top: Option<TObjId>) {
    tobjs.head = top;
    if top.is_none() {
        return;
    }
    assign_resources(tobjs, top);
    let mut cur = top;
    while let Some(t) = cur {
        if tobjs[t].id != GX_TEXMAP_NULL {
            setup_mtx(tobjs, t);
        }
        cur = tobjs[t].next;
    }
}

/// Forgets `tobj_head` as HAL's class library is forgotten, `TObjAmnesia`. The class counts
/// and memory it also forgets go with the heap (`HSD_CreateMainHeap`).
#[doc(alias = "TObjAmnesia")]
pub fn amnesia(tobjs: &mut TObjs) {
    tobjs.head = None;
}

/// `HSD_TGTex2Index`: a texture-coordinate generation source's index.
#[doc(alias = "HSD_TGTex2Index")]
pub fn tg_tex_to_index(src: u32) -> u32 {
    assert!(
        (GX_TG_TEX0..GX_TG_TEX0 + 8).contains(&src),
        "texgen source {src}"
    );
    src - GX_TG_TEX0
}

/// `HSD_TexCoordID2TexGenSrc`: the generation source that reads a coordinate. There is none
/// for the last coordinate.
#[doc(alias = "HSD_TexCoordID2TexGenSrc")]
pub fn texcoord_to_texgen_src(coord: u32) -> u32 {
    assert!(coord < 7, "texcoord {coord}");
    GX_TG_TEXCOORD0 + coord
}

/// `HSD_TexCoord2Index`.
#[doc(alias = "HSD_TexCoord2Index")]
pub fn texcoord_to_index(coord: u32) -> u32 {
    assert!(coord < 8, "texcoord {coord}");
    coord
}

/// `HSD_Index2TexCoord`.
#[doc(alias = "HSD_Index2TexCoord")]
pub fn index_to_texcoord(index: u32) -> u32 {
    assert!(index < 8, "texcoord index {index}");
    index
}

/// `HSD_TexMtx2Index`: a texture matrix's index, 10 for the identity.
#[doc(alias = "HSD_TexMtx2Index")]
pub fn texmtx_to_index(mtx: u32) -> u32 {
    match mtx {
        GX_IDENTITY => 10,
        m if (GX_TEXMTX0..=GX_TEXMTX9).contains(&m) && (m - GX_TEXMTX0).is_multiple_of(3) => {
            (m - GX_TEXMTX0) / 3
        }
        m => panic!("texmtx {m} doesn't exist"),
    }
}

/// `HSD_Index2TexMtx`.
#[doc(alias = "HSD_Index2TexMtx")]
pub fn index_to_texmtx(index: u32) -> u32 {
    match index {
        0..=9 => GX_TEXMTX0 + 3 * index,
        10 => GX_IDENTITY,
        i => panic!("texmtx index {i} exceeds the hardware's"),
    }
}

/// `HSD_Index2TexMap`.
#[doc(alias = "HSD_Index2TexMap")]
pub fn index_to_texmap(index: u32) -> u32 {
    assert!(index < 8, "texmap index {index}");
    index
}

/// `HSD_TexMap2Index`.
#[doc(alias = "HSD_TexMap2Index")]
pub fn texmap_to_index(map: u32) -> u32 {
    assert!(map < 8, "texmap {map}");
    map
}
