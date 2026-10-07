//! Material objects, HAL's MObj (`sysdolphin/baselib/mobj.c`): a display object's material,
//! with its render mode, colors, pixel-engine settings, chain of textures and the animation
//! of its colors.
//!
//! What the display pass does with an MObj (TEV expressions and stages, material colors and
//! render modes in GX) isn't ported yet; [`compile_tev`] and [`setup`] port the state those
//! functions change.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj;
use super::pool::Pool;
use super::tobj::{self, Color, ImageRef, TOBJ_ANIM, TObjId, TObjs};
use crate::fpu::{copy, mem, reg};

/// An MObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MObjId(pub u32);

/// The material-animation flag of `HSD_*ReqAnimByFlags` (`MOBJ_ANIM`).
pub const MOBJ_ANIM: u32 = 0x4;
/// Every animation flag (`ALL_ANIM`).
pub const ALL_ANIM: u32 = 0x7FF;

/// `rendermode` bits (`RENDER_*`) melee-sim reads.
pub const RENDER_TOON: u32 = 1 << 12;
pub const RENDER_SHADOW: u32 = 1 << 26;

/// Animated channels (`HSD_A_M_*`).
const A_AMBIENT_R: u8 = 1;
const A_AMBIENT_G: u8 = 2;
const A_AMBIENT_B: u8 = 3;
const A_DIFFUSE_R: u8 = 4;
const A_DIFFUSE_G: u8 = 5;
const A_DIFFUSE_B: u8 = 6;
const A_SPECULAR_R: u8 = 7;
const A_SPECULAR_G: u8 = 8;
const A_SPECULAR_B: u8 = 9;
const A_ALPHA: u8 = 10;
const A_PE_REF0: u8 = 11;
const A_PE_REF1: u8 = 12;
const A_PE_DSTALPHA: u8 = 13;

/// Which class an MObj belongs to: `hsdMObj` or one the game derives from it, whose methods
/// replace HAL's (fighters' `ftMObj`, stages' `gr_mobj`, items' `it_mobj`). Only HAL's is
/// ported yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MObjClass {
    #[default]
    Hsd,
}

/// `HSD_Material`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Material {
    pub ambient: Color,
    pub diffuse: Color,
    pub specular: Color,
    pub alpha: f32,
    pub shininess: f32,
}

impl Material {
    /// A black, opaque material, `HSD_MaterialAlloc`.
    #[doc(alias = "HSD_MaterialAlloc")]
    pub fn new() -> Material {
        Material {
            alpha: 1.0,
            ..Material::default()
        }
    }

    /// A copy of a descriptor, as `memcpy` makes it.
    fn read(data: &Archives, d: DataRef) -> Material {
        let color = |o| {
            let at = d.at(o);
            Color {
                r: data.u8(at),
                g: data.u8(at.at(1)),
                b: data.u8(at.at(2)),
                a: data.u8(at.at(3)),
            }
        };
        Material {
            ambient: color(0),
            diffuse: color(4),
            specular: color(8),
            alpha: f32::from_bits(data.u32(d.at(0xC))),
            shininess: f32::from_bits(data.u32(d.at(0x10))),
        }
    }
}

/// `HSD_PEDesc`: pixel-engine settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeDesc {
    pub flags: u8,
    pub ref0: u8,
    pub ref1: u8,
    pub dst_alpha: u8,
    pub ty: u8,
    pub src_factor: u8,
    pub dst_factor: u8,
    pub logic_op: u8,
    pub z_comp: u8,
    pub alpha_comp0: u8,
    pub alpha_op: u8,
    pub alpha_comp1: u8,
}

impl PeDesc {
    fn read(data: &Archives, d: DataRef) -> PeDesc {
        let b = |o| data.u8(d.at(o));
        PeDesc {
            flags: b(0),
            ref0: b(1),
            ref1: b(2),
            dst_alpha: b(3),
            ty: b(4),
            src_factor: b(5),
            dst_factor: b(6),
            logic_op: b(7),
            z_comp: b(8),
            alpha_comp0: b(9),
            alpha_op: b(10),
            alpha_comp1: b(11),
        }
    }
}

/// `HSD_MObj`. Its compiled TEV (`tevdesc`, `texp`) is the display pass's, made from this
/// state, and isn't kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MObj {
    pub class: MObjClass,
    pub rendermode: u32,
    pub tobj: Option<TObjId>,
    /// Null in an MObj allocated but never loaded.
    pub mat: Option<Material>,
    pub pe: Option<PeDesc>,
    pub aobj: Option<AObjId>,
}

impl MObj {
    fn mat(&mut self) -> &mut Material {
        self.mat.as_mut().expect("an MObj without a material")
    }
}

/// The MObj pool and mobj.c's globals.
#[derive(Clone, Debug, Default)]
pub struct MObjs {
    pub pool: Pool<MObj>,
    /// The MObj the display pass is drawing (`current_mobj`).
    pub current: Option<MObjId>,
    /// The toon-shading texture put before toon materials' textures (`tobj_toon`).
    pub toon: Option<TObjId>,
    /// Shadow textures put after shadowed materials' textures (`tobj_shadows`).
    pub shadows: Option<TObjId>,
}

impl Index<MObjId> for MObjs {
    type Output = MObj;
    fn index(&self, m: MObjId) -> &MObj {
        &self.pool[m.0]
    }
}

impl IndexMut<MObjId> for MObjs {
    fn index_mut(&mut self, m: MObjId) -> &mut MObj {
        &mut self.pool[m.0]
    }
}

/// `HSD_MObjSetCurrent`.
#[doc(alias = "HSD_MObjSetCurrent")]
pub fn set_current(mobjs: &mut MObjs, m: Option<MObjId>) {
    mobjs.current = m;
}

/// Sets render-mode bits, `HSD_MObjSetFlags`.
#[doc(alias = "HSD_MObjSetFlags")]
pub fn set_flags(mobjs: &mut MObjs, m: Option<MObjId>, flags: u32) {
    if let Some(m) = m {
        mobjs[m].rendermode |= flags;
    }
}

/// Clears render-mode bits, `HSD_MObjClearFlags`.
#[doc(alias = "HSD_MObjClearFlags")]
pub fn clear_flags(mobjs: &mut MObjs, m: Option<MObjId>, flags: u32) {
    if let Some(m) = m {
        mobjs[m].rendermode &= !flags;
    }
}

/// Frees the material's animation if `flags` has [`MOBJ_ANIM`], and its textures' if it has
/// [`TOBJ_ANIM`], `HSD_MObjRemoveAnimByFlags`.
#[doc(alias = "HSD_MObjRemoveAnimByFlags")]
pub fn remove_anim_by_flags(objs: &mut Objects, m: Option<MObjId>, flags: u32) {
    let Some(m) = m else { return };
    if flags & MOBJ_ANIM != 0 {
        jobj::remove_aobj(objs, objs.mobjs[m].aobj);
        objs.mobjs[m].aobj = None;
    }
    if flags & TOBJ_ANIM != 0 {
        tobj::remove_anim_all(objs, objs.mobjs[m].tobj);
    }
}

/// Gives a material its animation from an `HSD_MatAnim`, and its textures theirs from the
/// MatAnim's texture animations, `HSD_MObjAddAnim`. (A MatAnim's render animation isn't used.)
#[doc(alias = "HSD_MObjAddAnim")]
pub fn add_anim(objs: &mut Objects, data: &Archives, m: Option<MObjId>, matanim: Option<DataRef>) {
    let (Some(m), Some(ma)) = (m, matanim) else {
        return;
    };
    if let Some(a) = objs.mobjs[m].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    objs.mobjs[m].aobj = jobj::load_aobj(objs, data, data.ptr(ma.at(4)));
    tobj::add_anim_all(objs, data, objs.mobjs[m].tobj, data.ptr(ma.at(8)));
}

/// Restarts the material's animation at `frame` if `flags` has [`MOBJ_ANIM`], and its
/// textures' by `flags`, `HSD_MObjReqAnimByFlags`.
#[doc(alias = "HSD_MObjReqAnimByFlags")]
pub fn req_anim_by_flags(objs: &mut Objects, m: Option<MObjId>, frame: f32, flags: u32) {
    let Some(m) = m else { return };
    if flags & MOBJ_ANIM != 0 {
        aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, objs.mobjs[m].aobj, frame);
    }
    tobj::req_anim_all_by_flags(objs, objs.mobjs[m].tobj, frame, flags);
}

/// Restarts every animation of a material at `frame`, `HSD_MObjReqAnim`.
#[doc(alias = "HSD_MObjReqAnim")]
pub fn req_anim(objs: &mut Objects, m: Option<MObjId>, frame: f32) {
    req_anim_by_flags(objs, m, frame, ALL_ANIM);
}

/// Hands an animated value to a material, `MObjUpdateFunc`.
///
/// A stale value (`Value::Stale`), a RAM address, gives every channel the same result
/// whatever the address: 0 as a color, and an alpha of 1.
#[doc(alias = "MObjUpdateFunc")]
pub fn update(mobjs: &mut MObjs, m: MObjId, ty: u8, value: Value) {
    let v = value.or_address();
    let o = &mut mobjs[m];
    let c = || tobj::color_channel(v);
    match ty {
        A_AMBIENT_R => o.mat().ambient.r = c(),
        A_AMBIENT_G => o.mat().ambient.g = c(),
        A_AMBIENT_B => o.mat().ambient.b = c(),
        A_DIFFUSE_R => o.mat().diffuse.r = c(),
        A_DIFFUSE_G => o.mat().diffuse.g = c(),
        A_DIFFUSE_B => o.mat().diffuse.b = c(),
        A_ALPHA => o.mat().alpha = mem(fp::fsubs(1.0, reg(v))),
        A_SPECULAR_R => o.mat().specular.r = c(),
        A_SPECULAR_G => o.mat().specular.g = c(),
        A_SPECULAR_B => o.mat().specular.b = c(),
        A_PE_REF0 | A_PE_REF1 | A_PE_DSTALPHA => {
            if let Some(pe) = o.pe.as_mut() {
                let field = match ty {
                    A_PE_REF0 => &mut pe.ref0,
                    A_PE_REF1 => &mut pe.ref1,
                    _ => &mut pe.dst_alpha,
                };
                *field = c();
            }
        }
        _ => {}
    }
}

/// Advances a material's animation one frame, then its textures', `HSD_MObjAnim`.
#[doc(alias = "HSD_MObjAnim")]
pub fn anim(objs: &mut Objects, data: &Archives, m: Option<MObjId>) {
    let Some(m) = m else { return };
    let a = objs.mobjs[m].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(&mut objs.mobjs, m, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
    tobj::anim_all(objs, data, objs.mobjs[m].tobj);
}

/// A new, empty MObj of HAL's class, `HSD_MObjAlloc`. Its addresses in the original come from
/// `hsdAllocMemPiece`, as a TObj's do (see `tobj::alloc`).
#[doc(alias = "HSD_MObjAlloc")]
pub fn alloc(mobjs: &mut MObjs) -> MObjId {
    MObjId(mobjs.pool.alloc(MObj::default()))
}

/// Builds an MObj and its textures from a descriptor and assigns the textures' resources,
/// `HSD_MObjLoadDesc`.
///
/// A descriptor naming a class (`class_name`) would be built by that class; none on the disc
/// does, so such a descriptor panics here.
#[doc(alias = "HSD_MObjLoadDesc")]
#[doc(alias = "MObjLoad")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<MObjId> {
    let d = desc?;
    assert!(
        data.ptr(d).is_none(),
        "MObjDesc {d:?} names a class, which melee-sim doesn't look up"
    );
    let m = alloc(&mut objs.mobjs);
    let tobj = tobj::load_desc(&mut objs.tobjs, data, data.ptr(d.at(8)));
    // A descriptor without a material would make the original copy from null.
    let mat = Material::read(data, data.ptr(d.at(0xC)).expect("an MObjDesc's material"));
    let o = &mut objs.mobjs[m];
    o.rendermode = data.u32(d.at(4)) | RENDER_TOON;
    o.tobj = tobj;
    o.mat = Some(mat);
    if let Some(pe) = data.ptr(d.at(0x14)) {
        o.pe = Some(PeDesc::read(data, pe));
    }
    o.aobj = None;
    compile_tev(objs, Some(m));
    Some(m)
}

/// Where a borrowed shadow chain was hung from: a local variable, or a TObj's `next`.
enum Tail {
    Local,
    Next(TObjId),
}

/// A material's textures as the display sees them: after the toon texture (left pointing at
/// them) for a toon material, and with the shadow textures hung after them for a shadowed one.
fn borrow_chain(
    mobjs: &MObjs,
    tobjs: &mut TObjs,
    m: MObjId,
    rendermode: u32,
) -> (Option<TObjId>, Option<Tail>) {
    let mut first = mobjs[m].tobj;
    let mut tail = None;
    if rendermode & RENDER_SHADOW != 0
        && let Some(shadows) = mobjs.shadows
    {
        match tobjs.chain(first).last() {
            None => {
                first = Some(shadows);
                tail = Some(Tail::Local);
            }
            Some(last) => {
                tobjs[last].next = Some(shadows);
                tail = Some(Tail::Next(last));
            }
        }
    }
    if rendermode & RENDER_TOON != 0
        && let Some(toon) = mobjs.toon
        && tobjs[toon].imagedesc.is_some()
    {
        tobjs[toon].next = first;
        first = Some(toon);
    }
    (first, tail)
}

/// Unhangs the shadow textures [`borrow_chain`] hung.
fn return_chain(tobjs: &mut TObjs, tail: Option<Tail>) {
    if let Some(Tail::Next(last)) = tail {
        tobjs[last].next = None;
    }
}

/// Assigns the textures' resources for the material's TEV, with the toon and shadow
/// textures it uses, `HSD_MObjCompileTev`.
///
/// The original then builds the material's TEV expressions and compiles them into stages
/// (`MObjMakeTExp`, `HSD_TExpCompile`), which only the display pass reads.
#[doc(alias = "HSD_MObjCompileTev")]
pub fn compile_tev(objs: &mut Objects, m: Option<MObjId>) {
    let Some(m) = m else { return };
    let (mobjs, tobjs) = (&objs.mobjs, &mut objs.tobjs);
    let (first, tail) = borrow_chain(mobjs, tobjs, m, mobjs[m].rendermode);
    tobj::assign_resources(tobjs, first);
    return_chain(tobjs, tail);
}

/// The state `HSD_MObjSetup` changes as the display pass sets a material up: its textures,
/// with the toon and shadow textures it uses, go through `HSD_TObjSetup` ([`tobj::setup`]).
///
/// The original also sets the material's colors, texture coordinates, TEV stages and render
/// mode in GX: that's the display pass's.
#[doc(alias = "HSD_MObjSetup")]
pub fn setup(objs: &mut Objects, m: MObjId) {
    let (mobjs, tobjs) = (&objs.mobjs, &mut objs.tobjs);
    let (first, tail) = borrow_chain(mobjs, tobjs, m, mobjs[m].rendermode);
    tobj::setup(tobjs, first);
    return_chain(tobjs, tail);
}

/// `HSD_MObjUnset`: clears [`TObjs::head`], as `HSD_TObjSetup(NULL)` does.
#[doc(alias = "HSD_MObjUnset")]
pub fn unset(tobjs: &mut TObjs) {
    tobj::setup(tobjs, None);
}

/// Sets the toon-shading texture's image, making the texture the first time,
/// `HSD_MObjSetToonTextureImage`.
#[doc(alias = "HSD_MObjSetToonTextureImage")]
pub fn set_toon_texture_image(objs: &mut Objects, image: Option<ImageRef>) {
    let (mobjs, tobjs) = (&mut objs.mobjs, &mut objs.tobjs);
    let toon = *mobjs
        .toon
        .get_or_insert_with(|| tobj::load_toon(tobjs, image));
    tobjs[toon].imagedesc = image;
}

/// `HSD_MObjSetDiffuseColor`.
#[doc(alias = "HSD_MObjSetDiffuseColor")]
pub fn set_diffuse_color(mobjs: &mut MObjs, m: MObjId, r: u8, g: u8, b: u8) {
    let mat = mobjs[m].mat();
    mat.diffuse.r = r;
    mat.diffuse.g = g;
    mat.diffuse.b = b;
}

/// `HSD_MObjSetAlpha`.
#[doc(alias = "HSD_MObjSetAlpha")]
pub fn set_alpha(mobjs: &mut MObjs, m: MObjId, alpha: f32) {
    mobjs[m].mat().alpha = copy(alpha);
}

/// `HSD_MObjGetTObj`.
#[doc(alias = "HSD_MObjGetTObj")]
pub fn tobj(mobjs: &MObjs, m: Option<MObjId>) -> Option<TObjId> {
    m.and_then(|m| mobjs[m].tobj)
}

/// Frees an MObj with its animation and textures, `HSD_MObjRemove` (releasing through
/// `MObjRelease`).
#[doc(alias = "HSD_MObjRemove")]
#[doc(alias = "MObjRelease")]
pub fn remove(objs: &mut Objects, m: Option<MObjId>) {
    let Some(m) = m else { return };
    jobj::remove_aobj(objs, objs.mobjs[m].aobj);
    tobj::remove_all(objs, objs.mobjs[m].tobj);
    objs.mobjs.pool.free(m.0);
}

/// Puts a texture at the front of the shadow textures unless it's there already,
/// `HSD_MObjAddShadowTexture`.
#[doc(alias = "HSD_MObjAddShadowTexture")]
pub fn add_shadow_texture(objs: &mut Objects, t: TObjId) {
    let (mobjs, tobjs) = (&mut objs.mobjs, &mut objs.tobjs);
    if tobjs.chain(mobjs.shadows).any(|s| s == t) {
        return;
    }
    tobjs[t].next = mobjs.shadows;
    mobjs.shadows = Some(t);
}

/// Takes a texture out of the shadow textures, or with `None` empties them,
/// `HSD_MObjDeleteShadowTexture`.
#[doc(alias = "HSD_MObjDeleteShadowTexture")]
pub fn delete_shadow_texture(objs: &mut Objects, t: Option<TObjId>) {
    let (mobjs, tobjs) = (&mut objs.mobjs, &mut objs.tobjs);
    match t {
        Some(t) => {
            let mut prev: Option<TObjId> = None;
            let mut cur = mobjs.shadows;
            while let Some(c) = cur {
                if c == t {
                    let next = tobjs[t].next;
                    match prev {
                        None => mobjs.shadows = next,
                        Some(p) => tobjs[p].next = next,
                    }
                    tobjs[t].next = None;
                    return;
                }
                prev = Some(c);
                cur = tobjs[c].next;
            }
        }
        None => {
            while let Some(s) = mobjs.shadows {
                mobjs.shadows = tobjs[s].next;
                tobjs[s].next = None;
            }
        }
    }
}

/// Forgets the toon and shadow textures as HAL's class library is forgotten, `MObjAmnesia`.
/// The class counts and memory it also forgets go with the heap (`HSD_CreateMainHeap`).
#[doc(alias = "MObjAmnesia")]
pub fn amnesia(mobjs: &mut MObjs) {
    mobjs.toon = None;
    mobjs.shadows = None;
}
