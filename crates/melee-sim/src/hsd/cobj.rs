//! Camera objects, HAL's CObj (`sysdolphin/baselib/cobj.c`): an eye and an interest point
//! (each a WObj), a roll or up vector, clipping planes and a projection, and the view matrix
//! built from them. The game's camera reads projections and view matrices for gameplay too
//! (screen bounds), so all of it is simulation.
//!
//! Setting a CObj up for drawing (`HSD_CObjSetCurrent` and its per-pass setups, which issue
//! `GXSetViewport`, `GXSetScissor` and `GXSetProjection`, and `HSD_CObjEraseScreen`) belongs to
//! the display pass and isn't ported yet; [`setup_viewing_mtx`] is the state it computes.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj;
use super::object::RefCount;
use super::pool::Pool;
use super::wobj::{self, WObjId};
use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::mtx::{Mtx34, Mtx44};
use crate::vec::Vec3;

/// A CObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CObjId(pub u32);

/// The camera keeps an up vector rather than a roll.
pub const UP_VECTOR: u32 = 1;
/// The view matrix is set directly and never rebuilt.
pub const FIXED_VIEW: u32 = 2;
/// The view matrix needs rebuilding.
pub const VIEW_DIRTY: u32 = 1 << 30;
/// The inverse view matrix needs rebuilding.
pub const INV_VIEW_DIRTY: u32 = 1 << 31;

/// `PROJ_PERSPECTIVE`, `PROJ_FRUSTUM` and `PROJ_ORTHO`.
pub const PERSPECTIVE: u8 = 1;
pub const FRUSTUM: u8 = 2;
pub const ORTHO: u8 = 3;

/// `GXProjectionType`: `GX_PERSPECTIVE`, `GX_ORTHOGRAPHIC`.
pub const GX_PERSPECTIVE: u32 = 0;
pub const GX_ORTHOGRAPHIC: u32 = 1;

/// What the fallbacks of [`eye_vector`], [`up_vector`] and [`left_vector`] give.
pub const NO_EYE_VECTOR: Vec3 = Vec3::new(0.0, 0.0, -1.0);
pub const NO_UP_VECTOR: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const NO_LEFT_VECTOR: Vec3 = Vec3::new(1.0, 0.0, 0.0);

/// π/180 as `DegToRad` has it, in single precision.
const DEG_TO_RAD: f32 = 0.017_453_292;

/// `HSD_CObj`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CObj {
    pub ref_count: RefCount,
    pub flags: u32,
    /// `xmin`, `xmax`, `ymin`, `ymax`.
    pub viewport: [f32; 4],
    /// `left`, `right`, `top`, `bottom`.
    pub scissor: [u16; 4],
    pub eyepos: Option<WObjId>,
    pub interest: Option<WObjId>,
    /// The union `u`: the up vector, or the roll in `x`.
    pub up: Vec3,
    pub near: f32,
    pub far: f32,
    /// The union `projection_param`: `fov`, `aspect` for a perspective; `top`, `bottom`,
    /// `left`, `right` for a frustum or an orthographic projection.
    pub projection: [f32; 4],
    pub projection_type: u8,
    pub view_mtx: Mtx34,
    pub aobj: Option<AObjId>,
    /// The inverse view matrix (the original's `proj_mtx`, from `HSD_MtxAlloc`), allocated on
    /// first use.
    pub inv_view_mtx: Option<Mtx34>,
}

impl CObj {
    fn roll(&self) -> f32 {
        self.up.x
    }
}

/// The CObj pool and the camera being drawn (`current`, set by the display pass's
/// `HSD_CObjSetCurrent`).
#[derive(Clone, Debug, Default)]
pub struct CObjs {
    pub pool: Pool<CObj>,
    pub current: Option<CObjId>,
}

impl Index<CObjId> for CObjs {
    type Output = CObj;
    fn index(&self, c: CObjId) -> &CObj {
        &self.pool[c.0]
    }
}

impl IndexMut<CObjId> for CObjs {
    fn index_mut(&mut self, c: CObjId) -> &mut CObj {
        &mut self.pool[c.0]
    }
}

/// A new CObj with its eye and interest WObjs, its view matrices dirty, `HSD_CObjAlloc` (with
/// the class's init, `CObjInit`).
#[doc(alias = "HSD_CObjAlloc")]
#[doc(alias = "CObjInit")]
pub fn alloc(objs: &mut Objects) -> CObjId {
    let c = CObjId(objs.cobjs.pool.alloc(CObj::default()));
    set_mtx_dirty(&mut objs.cobjs, c);
    objs.cobjs[c].eyepos = Some(wobj::alloc(&mut objs.wobjs));
    objs.cobjs[c].interest = Some(wobj::alloc(&mut objs.wobjs));
    c
}

/// Sets a CObj up from a descriptor (an `HSD_CObjDesc`), `HSD_CObjInit` (`CObjLoad` without
/// the null checks). A projection type other than the three panics, as the original asserts.
#[doc(alias = "HSD_CObjInit")]
#[doc(alias = "CObjLoad")]
pub fn init(objs: &mut Objects, data: &Archives, c: Option<CObjId>, desc: Option<DataRef>) {
    let (Some(c), Some(desc)) = (c, desc) else {
        return;
    };
    let flags = u32::from(data.u16(desc.at(4)));
    // `cobj->flags = desc->flags`, then CObjResetFlags keeps the dirty bits, which that cleared.
    let o = &mut objs.cobjs[c];
    o.flags = flags;
    o.flags = (o.flags & 0xC000_0000) | flags;
    let s16 = |at: u32| fp::frsp(f64::from(data.u16(desc.at(at)) as i16));
    o.viewport = [mem(s16(8)), mem(s16(0xA)), mem(s16(0xC)), mem(s16(0xE))];
    o.scissor = [0x10, 0x12, 0x14, 0x16].map(|at| data.u16(desc.at(at)));
    let (eye, interest) = (o.eyepos, o.interest);
    wobj::init(objs, data, eye, data.ptr(desc.at(0x18)));
    wobj::init(objs, data, interest, data.ptr(desc.at(0x1C)));
    let cobjs = &mut objs.cobjs;
    set_near(cobjs, Some(c), data.f32(desc.at(0x28)));
    set_far(cobjs, Some(c), data.f32(desc.at(0x2C)));
    if flags & UP_VECTOR != 0 {
        let up = match data.ptr(desc.at(0x24)) {
            Some(up) => wobj::vec_at(data, up),
            None => Vec3::new(0.0, 1.0, 0.0),
        };
        set_up_vector(objs, data, Some(c), up);
    } else {
        set_roll(objs, data, Some(c), data.f32(desc.at(0x20)));
    }
    let cobjs = &mut objs.cobjs;
    let p = |i: u32| data.f32(desc.at(0x30 + 4 * i));
    match data.u16(desc.at(6)) {
        1 => set_perspective(cobjs, Some(c), p(0), p(1)),
        3 => set_ortho(cobjs, Some(c), p(0), p(1), p(2), p(3)),
        2 => set_frustum(cobjs, Some(c), p(0), p(1), p(2), p(3)),
        t => panic!("CObjDesc {desc:?} has projection type {t}"),
    }
}

/// A camera descriptor in the executable's data (an `HSD_CObjDesc` whose WObj descriptors
/// hold a position and no references), as `HSD_CObjInit` reads one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticDesc {
    pub flags: u16,
    pub projection_type: u16,
    pub viewport: [i16; 4],
    pub scissor: [u16; 4],
    pub eye: Vec3,
    pub interest: Vec3,
    pub roll: f32,
    pub near: f32,
    pub far: f32,
    /// The projection's parameters: field of view and aspect, or top, bottom, left, right.
    pub projection: [f32; 4],
}

/// A CObj loaded from a descriptor in the executable's data, `HSD_CObjLoadDesc` on it: the
/// steps of [`init`] with the descriptor's values.
pub fn load_static_desc(objs: &mut Objects, data: &Archives, d: &StaticDesc) -> CObjId {
    assert!(
        u32::from(d.flags) & UP_VECTOR == 0,
        "a static CObjDesc with an up vector"
    );
    let c = alloc(objs);
    let flags = u32::from(d.flags);
    let o = &mut objs.cobjs[c];
    o.flags = flags;
    o.flags = (o.flags & 0xC000_0000) | flags;
    let s16 = |v: i16| mem(fp::frsp(f64::from(v)));
    o.viewport = d.viewport.map(s16);
    o.scissor = d.scissor;
    let (eye, interest) = (o.eyepos, o.interest);
    wobj::set_position(&mut objs.wobjs, eye, d.eye);
    wobj::set_position(&mut objs.wobjs, interest, d.interest);
    let cobjs = &mut objs.cobjs;
    set_near(cobjs, Some(c), d.near);
    set_far(cobjs, Some(c), d.far);
    set_roll(objs, data, Some(c), d.roll);
    let cobjs = &mut objs.cobjs;
    let p = d.projection;
    match d.projection_type {
        1 => set_perspective(cobjs, Some(c), p[0], p[1]),
        3 => set_ortho(cobjs, Some(c), p[0], p[1], p[2], p[3]),
        2 => set_frustum(cobjs, Some(c), p[0], p[1], p[2], p[3]),
        t => panic!("a static CObjDesc with projection type {t}"),
    }
    c
}

/// A CObj loaded from a descriptor, `HSD_CObjLoadDesc`. A descriptor naming a class panics:
/// melee-sim has no class registry, and nothing in the game names one.
#[doc(alias = "HSD_CObjLoadDesc")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<CObjId> {
    let desc = desc?;
    assert!(data.ptr(desc).is_none(), "CObjDesc {desc:?} names a class");
    let c = alloc(objs);
    init(objs, data, Some(c), Some(desc));
    Some(c)
}

/// Drops an owner of a CObj, destroying it with the last: the scheduler's release for
/// camera GObjs, `HSD_GObj_80391120`.
#[doc(alias = "HSD_GObj_80391120")]
pub fn unref(objs: &mut Objects, c: Option<CObjId>) {
    let Some(c) = c else { return };
    if objs.cobjs[c].ref_count.dec() {
        delete(objs, c);
    }
}

/// Frees a CObj with its animation, and drops its WObjs, its class's release (`CObjRelease`)
/// and destroy.
#[doc(alias = "CObjRelease")]
pub fn delete(objs: &mut Objects, c: CObjId) {
    jobj::remove_aobj(objs, objs.cobjs[c].aobj);
    let (eye, interest) = (objs.cobjs[c].eyepos, objs.cobjs[c].interest);
    wobj::unref(objs, eye);
    wobj::unref(objs, interest);
    objs.cobjs[c].inv_view_mtx = None;
    objs.cobjs.pool.free(c.0);
}

/// Frees the CObj's animation and its WObjs', `HSD_CObjRemoveAnim`.
#[doc(alias = "HSD_CObjRemoveAnim")]
pub fn remove_anim(objs: &mut Objects, c: Option<CObjId>) {
    let Some(c) = c else { return };
    jobj::remove_aobj(objs, objs.cobjs[c].aobj);
    objs.cobjs[c].aobj = None;
    let (eye, interest) = (objs.cobjs[c].eyepos, objs.cobjs[c].interest);
    wobj::remove_anim(objs, eye);
    wobj::remove_anim(objs, interest);
}

/// Gives a CObj and its WObjs the animations of an `HSD_CameraAnim`, `HSD_CObjAddAnim`.
#[doc(alias = "HSD_CObjAddAnim")]
pub fn add_anim(objs: &mut Objects, data: &Archives, c: Option<CObjId>, anim: Option<DataRef>) {
    let (Some(c), Some(anim)) = (c, anim) else {
        return;
    };
    if let Some(a) = objs.cobjs[c].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    objs.cobjs[c].aobj = jobj::load_aobj(objs, data, data.ptr(anim));
    let (eye, interest) = (objs.cobjs[c].eyepos, objs.cobjs[c].interest);
    wobj::add_anim(objs, data, eye, data.ptr(anim.at(4)));
    wobj::add_anim(objs, data, interest, data.ptr(anim.at(8)));
}

/// `CObjUpdateFunc`: a channel's value moves the eye or interest, or sets the roll, field of
/// view or clipping planes. The interest's three channels all set its x, as the original does.
fn update(objs: &mut Objects, data: &Archives, c: CObjId, ty: u8, value: Value) {
    if matches!(ty, 1..=3 | 5..=7 | 9..=12) && matches!(value, Value::Stale) {
        objs.stale_reads += 1;
    }
    let val = || value.or_address();
    match ty {
        1..=3 => {
            if let Some(mut v) = eye_position(objs, data, c) {
                match ty {
                    1 => v.x = copy(val()),
                    2 => v.y = copy(val()),
                    _ => v.z = copy(val()),
                }
                set_eye_position(objs, c, v);
            }
        }
        5..=7 => {
            if let Some(mut v) = interest(objs, data, c) {
                v.x = copy(val());
                set_interest(objs, c, v);
            }
        }
        9 => set_roll(objs, data, Some(c), val()),
        10 => set_fov(&mut objs.cobjs, Some(c), val()),
        11 => set_near(&mut objs.cobjs, Some(c), val()),
        12 => set_far(&mut objs.cobjs, Some(c), val()),
        _ => {}
    }
}

/// Advances a CObj's animation and its WObjs' one frame, `HSD_CObjAnim`.
#[doc(alias = "HSD_CObjAnim")]
#[doc(alias = "CObjUpdateFunc")]
pub fn anim(objs: &mut Objects, data: &Archives, c: Option<CObjId>) {
    let Some(c) = c else { return };
    let a = objs.cobjs[c].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, data, c, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
    let (eye, interest) = (objs.cobjs[c].eyepos, objs.cobjs[c].interest);
    wobj::interpret_anim(objs, data, eye);
    wobj::interpret_anim(objs, data, interest);
}

/// Restarts a CObj's animation and its WObjs' at `frame`, `HSD_CObjReqAnim`.
#[doc(alias = "HSD_CObjReqAnim")]
pub fn req_anim(objs: &mut Objects, c: Option<CObjId>, frame: f32) {
    let Some(c) = c else { return };
    let a = objs.cobjs[c].aobj;
    aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
    let (eye, interest) = (objs.cobjs[c].eyepos, objs.cobjs[c].interest);
    wobj::req_anim(objs, eye, frame);
    wobj::req_anim(objs, interest, frame);
}

/// The projection matrix and its `GXProjectionType`, `makeProjectionMtx`. For another
/// projection type the original leaves the matrix as it was and returns its caller's `r31`;
/// this gives `None`.
#[doc(alias = "makeProjectionMtx")]
pub fn projection_mtx(cobjs: &CObjs, c: CObjId) -> Option<(u32, Mtx44)> {
    let o = &cobjs[c];
    let [p0, p1, p2, p3] = o.projection;
    match o.projection_type {
        PERSPECTIVE => Some((GX_PERSPECTIVE, Mtx44::perspective(p0, p1, o.near, o.far))),
        FRUSTUM => Some((
            GX_PERSPECTIVE,
            Mtx44::frustum(p0, p1, p2, p3, o.near, o.far),
        )),
        ORTHO => Some((GX_ORTHOGRAPHIC, Mtx44::ortho(p0, p1, p2, p3, o.near, o.far))),
        _ => None,
    }
}

/// Rebuilds the view matrix when it is dirty and not fixed, the body `HSD_CObjSetupViewingMtx`
/// shares with the viewing-matrix getters.
///
/// With a dirty flag but no eye or interest WObj, the original builds the matrix from
/// uninitialized stack and then writes through a null WObj; that panics here.
#[doc(alias = "HSD_CObjSetupViewingMtx")]
pub fn setup_viewing_mtx(objs: &mut Objects, data: &Archives, c: CObjId) {
    if objs.cobjs[c].flags & FIXED_VIEW != 0 || !mtx_is_dirty(objs, c) {
        return;
    }
    let eye = eye_position(objs, data, c).expect("a dirty CObj without an eye WObj");
    let up = up_vector(objs, data, Some(c)).unwrap_or(NO_UP_VECTOR);
    let target = interest(objs, data, c).expect("a dirty CObj without an interest WObj");
    let o = &mut objs.cobjs[c];
    o.view_mtx = Mtx34::look_at(eye, up, target);
    let (e, i) = (o.eyepos.unwrap(), o.interest.unwrap());
    wobj::clear_flags(&mut objs.wobjs, e, wobj::DIRTY);
    wobj::clear_flags(&mut objs.wobjs, i, wobj::DIRTY);
    clear_flags(&mut objs.cobjs, Some(c), VIEW_DIRTY);
    set_flags(&mut objs.cobjs, Some(c), INV_VIEW_DIRTY);
}

/// The view matrix, rebuilt first if dirty, `HSD_CObjGetViewingMtxPtr`.
#[doc(alias = "HSD_CObjGetViewingMtxPtr")]
pub fn viewing_mtx(objs: &mut Objects, data: &Archives, c: CObjId) -> Mtx34 {
    setup_viewing_mtx(objs, data, c);
    objs.cobjs[c].view_mtx
}

/// A copy of the view matrix through `PSMTXCopy`, `HSD_CObjGetViewingMtx`.
#[doc(alias = "HSD_CObjGetViewingMtx")]
pub fn viewing_mtx_copy(objs: &mut Objects, data: &Archives, c: CObjId) -> Mtx34 {
    viewing_mtx(objs, data, c).copied()
}

/// The inverse view matrix, recomputed when dirty, `HSD_CObjGetInvViewingMtxPtrDirect`;
/// `None` if it was never allocated and isn't dirty (the original returns a null matrix).
///
/// `PSMTXInverse` leaves the matrix as it was when the view matrix is singular. A newly
/// allocated one then holds whatever `HSD_MtxAlloc`'s pool last left there, which plain data
/// can't reproduce; here it starts zeroed. In practice only a view matrix never built (zero,
/// as allocated) is singular.
#[doc(alias = "HSD_CObjGetInvViewingMtxPtrDirect")]
pub fn inv_viewing_mtx_direct(cobjs: &mut CObjs, c: CObjId) -> Option<Mtx34> {
    let o = &mut cobjs[c];
    if o.flags & INV_VIEW_DIRTY != 0 {
        let inv = o.inv_view_mtx.get_or_insert_with(Mtx34::default);
        if let Some(m) = o.view_mtx.inverse() {
            *inv = m;
        }
        o.flags &= !INV_VIEW_DIRTY;
    }
    o.inv_view_mtx
}

/// The inverse view matrix, the view matrix rebuilt first if dirty,
/// `HSD_CObjGetInvViewingMtxPtr`.
#[doc(alias = "HSD_CObjGetInvViewingMtxPtr")]
pub fn inv_viewing_mtx(objs: &mut Objects, data: &Archives, c: CObjId) -> Option<Mtx34> {
    setup_viewing_mtx(objs, data, c);
    inv_viewing_mtx_direct(&mut objs.cobjs, c)
}

/// `HSD_CObjGetInterestWObj`.
#[doc(alias = "HSD_CObjGetInterestWObj")]
pub fn interest_wobj(cobjs: &CObjs, c: CObjId) -> Option<WObjId> {
    cobjs[c].interest
}

/// The inline `HSD_CObjSetInterestWObj`.
pub fn set_interest_wobj(cobjs: &mut CObjs, c: CObjId, w: Option<WObjId>) {
    cobjs[c].interest = w;
}

/// `HSD_CObjGetEyePositionWObj`.
#[doc(alias = "HSD_CObjGetEyePositionWObj")]
pub fn eye_position_wobj(cobjs: &CObjs, c: CObjId) -> Option<WObjId> {
    cobjs[c].eyepos
}

/// The inline `HSD_CObjSetEyePositionWObj`.
pub fn set_eye_position_wobj(cobjs: &mut CObjs, c: CObjId, w: Option<WObjId>) {
    cobjs[c].eyepos = w;
}

/// The interest point, `HSD_CObjGetInterest`; `None` without an interest WObj (the original
/// writes nothing).
#[doc(alias = "HSD_CObjGetInterest")]
pub fn interest(objs: &mut Objects, data: &Archives, c: CObjId) -> Option<Vec3> {
    let w = objs.cobjs[c].interest;
    wobj::position(objs, data, w)
}

/// `HSD_CObjSetInterest`.
#[doc(alias = "HSD_CObjSetInterest")]
pub fn set_interest(objs: &mut Objects, c: CObjId, pos: Vec3) {
    wobj::set_position(&mut objs.wobjs, objs.cobjs[c].interest, pos);
}

/// The eye position, `HSD_CObjGetEyePosition`; `None` without an eye WObj.
#[doc(alias = "HSD_CObjGetEyePosition")]
pub fn eye_position(objs: &mut Objects, data: &Archives, c: CObjId) -> Option<Vec3> {
    let w = objs.cobjs[c].eyepos;
    wobj::position(objs, data, w)
}

/// `HSD_CObjSetEyePosition`.
#[doc(alias = "HSD_CObjSetEyePosition")]
pub fn set_eye_position(objs: &mut Objects, c: CObjId, pos: Vec3) {
    wobj::set_position(&mut objs.wobjs, objs.cobjs[c].eyepos, pos);
}

/// The float stored, its sign bit cleared, and loaded again (`fabsf_bitwise`).
fn abs_bitwise(x: f64) -> f64 {
    fp::lfs(fp::stfs(x) & 0x7FFF_FFFF)
}

/// `vec_normalize_check`: the unit vector, or `None` when every component is within
/// `FLT_MIN` of zero.
#[doc(alias = "vec_normalize_check")]
fn normalize_check(v: Vec3) -> Option<Vec3> {
    let tiny = |x: f32| abs_bitwise(reg(x)) <= f64::from(f32::MIN_POSITIVE);
    if tiny(v.x) && tiny(v.y) && tiny(v.z) {
        return None;
    }
    Some(v.sdk_normalize())
}

/// The unit vector from the eye to the interest, `HSD_CObjGetEyeVector`; `None` (the
/// original writes [`NO_EYE_VECTOR`] and returns -1) without both WObjs or when they meet.
#[doc(alias = "HSD_CObjGetEyeVector")]
pub fn eye_vector(objs: &mut Objects, data: &Archives, c: Option<CObjId>) -> Option<Vec3> {
    let c = c?;
    let o = &objs.cobjs[c];
    o.eyepos?;
    o.interest?;
    let eye = eye_position(objs, data, c)?;
    let target = interest(objs, data, c)?;
    normalize_check(target.sdk_sub(eye))
}

/// The distance from the eye to the interest, `HSD_CObjGetEyeDistance`; 0 without a CObj. A
/// CObj missing either WObj panics, as the original asserts.
#[doc(alias = "HSD_CObjGetEyeDistance")]
pub fn eye_distance(objs: &mut Objects, data: &Archives, c: Option<CObjId>) -> f32 {
    let Some(c) = c else { return 0.0 };
    let eye = eye_position(objs, data, c).expect("a CObj without an eye WObj");
    let target = interest(objs, data, c).expect("a CObj without an interest WObj");
    target.sdk_sub(eye).sdk_length()
}

/// The up vector a roll gives, about the eye vector, `roll2upvec`; `None` without an eye
/// vector.
#[doc(alias = "roll2upvec")]
fn roll_to_up(objs: &mut Objects, data: &Archives, c: CObjId, roll: f32) -> Option<Vec3> {
    let eye = eye_vector(objs, data, Some(c))?;
    let (x, y, z) = (reg(eye.x), reg(eye.y), reg(eye.z));
    let sq = |a: f64, b: f64| math::sqrt(mem(fp::fadds(fp::fmuls(a, a), fp::fmuls(b, b))));
    let mut v0 = Vec3::ZERO;
    if fp::fsub(1.0, fp::fabs(y)) < 0.0001 {
        // Looking almost straight up or down: the reference is along x.
        v0.x = sq(y, z);
        v0.y = mem(fp::fmuls(y, fp::fdivs(fp::fneg(x), reg(v0.x))));
        v0.z = mem(fp::fmuls(z, fp::fdivs(fp::fneg(x), reg(v0.x))));
    } else {
        v0.y = sq(x, z);
        v0.x = mem(fp::fmuls(x, fp::fdivs(fp::fneg(y), reg(v0.y))));
        v0.z = mem(fp::fmuls(z, fp::fdivs(fp::fneg(y), reg(v0.y))));
    }
    let m = Mtx34::rotation_about(eye, mem(fp::fneg(reg(roll))));
    Some(m.rotate(v0).sdk_normalize())
}

/// The roll an up vector gives, about the eye vector: the inline `upvec2roll`. 0 without an
/// eye vector or when the up vector is along it.
fn up_to_roll(objs: &mut Objects, data: &Archives, c: CObjId, up: Vec3) -> f32 {
    let Some(eye) = eye_vector(objs, data, Some(c)) else {
        return 0.0;
    };
    let dot = fp::fsubs(1.0, fp::fabs(reg(up.sdk_dot(eye))));
    if dot < f64::from(f32::MIN_POSITIVE) {
        return 0.0;
    }
    let vmtx = Mtx34::look_at(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), eye);
    let v = vmtx.rotate(up);
    let neg_x = fp::fneg(reg(v.x));
    if abs_bitwise(reg(v.y)) == 0.0 {
        let half_pi = std::f64::consts::FRAC_PI_2;
        mem(fp::frsp(if neg_x >= 0.0 {
            half_pi
        } else {
            fp::fneg(half_pi)
        }))
    } else {
        math::atan2(mem(neg_x), v.y)
    }
}

/// The up vector, `HSD_CObjGetUpVector`: the stored one, or the one the roll gives; `None`
/// (the original writes [`NO_UP_VECTOR`] and returns -1) when the roll has no eye vector.
#[doc(alias = "HSD_CObjGetUpVector")]
pub fn up_vector(objs: &mut Objects, data: &Archives, c: Option<CObjId>) -> Option<Vec3> {
    let c = c?;
    let o = &objs.cobjs[c];
    if o.flags & UP_VECTOR != 0 {
        return Some(o.up);
    }
    let roll = o.roll();
    roll_to_up(objs, data, c, roll)
}

/// Sets the up vector (normalized; straight up if it has no length), or the roll it gives,
/// `HSD_CObjSetUpVector`.
#[doc(alias = "HSD_CObjSetUpVector")]
pub fn set_up_vector(objs: &mut Objects, data: &Archives, c: Option<CObjId>, up: Vec3) {
    let Some(c) = c else { return };
    let cobjs = &mut objs.cobjs;
    if cobjs[c].flags & UP_VECTOR != 0 {
        let v = normalize_check(up).unwrap_or(Vec3::new(0.0, 1.0, 0.0));
        let old = cobjs[c].up;
        if reg(old.x) != reg(v.x) || reg(old.y) != reg(v.y) || reg(old.z) != reg(v.z) {
            set_mtx_dirty(cobjs, c);
        }
        cobjs[c].up = v;
    } else {
        let roll = up_to_roll(objs, data, c, up);
        set_roll(objs, data, Some(c), roll);
    }
}

/// The unit vector to the camera's left, `HSD_CObjGetLeftVector`; `None` (the original writes
/// [`NO_LEFT_VECTOR`] and returns -1) without eye or up vectors or when they're parallel.
#[doc(alias = "HSD_CObjGetLeftVector")]
pub fn left_vector(objs: &mut Objects, data: &Archives, c: Option<CObjId>) -> Option<Vec3> {
    let eye = eye_vector(objs, data, c)?;
    let up = up_vector(objs, data, c)?;
    normalize_check(up.sdk_cross(eye))
}

/// `HSD_CObjSetMtxDirty`.
#[doc(alias = "HSD_CObjSetMtxDirty")]
pub fn set_mtx_dirty(cobjs: &mut CObjs, c: CObjId) {
    cobjs[c].flags |= VIEW_DIRTY | INV_VIEW_DIRTY;
}

/// Whether the view matrix needs rebuilding: the CObj says so or a WObj moved,
/// `HSD_CObjMtxIsDirty`.
#[doc(alias = "HSD_CObjMtxIsDirty")]
pub fn mtx_is_dirty(objs: &Objects, c: CObjId) -> bool {
    let o = &objs.cobjs[c];
    let moved = |w: Option<WObjId>| w.is_some_and(|w| objs.wobjs[w.0].flags & wobj::DIRTY != 0);
    o.flags & VIEW_DIRTY != 0 || moved(o.eyepos) || moved(o.interest)
}

/// Sets the roll, or the up vector it gives, `HSD_CObjSetRoll`.
///
/// With an up vector and no eye vector, the original sets the up vector from uninitialized
/// stack, which plain data can't reproduce; that panics here.
#[doc(alias = "HSD_CObjSetRoll")]
pub fn set_roll(objs: &mut Objects, data: &Archives, c: Option<CObjId>, roll: f32) {
    let Some(c) = c else { return };
    if objs.cobjs[c].flags & UP_VECTOR != 0 {
        let up = roll_to_up(objs, data, c, roll)
            .expect("a roll for an up-vector CObj without an eye vector");
        set_up_vector(objs, data, Some(c), up);
    } else {
        let o = &mut objs.cobjs[c];
        if reg(o.up.x) != reg(roll) {
            o.flags |= VIEW_DIRTY | INV_VIEW_DIRTY;
        }
        o.up.x = copy(roll);
    }
}

fn perspective(cobjs: &CObjs, c: Option<CObjId>) -> Option<&CObj> {
    c.map(|c| &cobjs[c])
        .filter(|o| o.projection_type == PERSPECTIVE)
}

/// The vertical field of view in degrees, 0 unless a perspective, `HSD_CObjGetFov`.
#[doc(alias = "HSD_CObjGetFov")]
pub fn fov(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    perspective(cobjs, c).map_or(0.0, |o| o.projection[0])
}

/// `HSD_CObjSetFov`: perspectives only.
#[doc(alias = "HSD_CObjSetFov")]
pub fn set_fov(cobjs: &mut CObjs, c: Option<CObjId>, fov: f32) {
    if perspective(cobjs, c).is_some() {
        cobjs[c.unwrap()].projection[0] = copy(fov);
    }
}

/// `HSD_CObjGetAspect`: 0 unless a perspective.
#[doc(alias = "HSD_CObjGetAspect")]
pub fn aspect(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    perspective(cobjs, c).map_or(0.0, |o| o.projection[1])
}

/// `HSD_CObjSetAspect`: perspectives only.
#[doc(alias = "HSD_CObjSetAspect")]
pub fn set_aspect(cobjs: &mut CObjs, c: Option<CObjId>, aspect: f32) {
    if perspective(cobjs, c).is_some() {
        cobjs[c.unwrap()].projection[1] = copy(aspect);
    }
}

/// `near · tan(fov / 2)` for a perspective, in a register.
fn half_height(o: &CObj) -> f64 {
    let half = fp::fmuls(0.5, fp::fmuls(f64::from(DEG_TO_RAD), reg(o.projection[0])));
    reg(math::tan(mem(half)))
}

/// The top clipping plane at the near plane, `HSD_CObjGetTop`.
#[doc(alias = "HSD_CObjGetTop")]
pub fn top(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    let Some(o) = c.map(|c| &cobjs[c]) else {
        return 0.0;
    };
    match o.projection_type {
        PERSPECTIVE => mem(fp::fmuls(reg(o.near), half_height(o))),
        FRUSTUM | ORTHO => o.projection[0],
        _ => 0.0,
    }
}

/// The bottom clipping plane at the near plane, `HSD_CObjGetBottom`.
#[doc(alias = "HSD_CObjGetBottom")]
pub fn bottom(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    let Some(o) = c.map(|c| &cobjs[c]) else {
        return 0.0;
    };
    match o.projection_type {
        PERSPECTIVE => mem(fp::fmuls(fp::fneg(reg(o.near)), half_height(o))),
        FRUSTUM | ORTHO => o.projection[1],
        _ => 0.0,
    }
}

/// The left clipping plane at the near plane, `HSD_CObjGetLeft`.
#[doc(alias = "HSD_CObjGetLeft")]
pub fn left(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    let Some(o) = c.map(|c| &cobjs[c]) else {
        return 0.0;
    };
    match o.projection_type {
        PERSPECTIVE => mem(fp::fmuls(
            reg(o.projection[1]),
            fp::fmuls(fp::fneg(reg(o.near)), half_height(o)),
        )),
        FRUSTUM | ORTHO => o.projection[2],
        _ => 0.0,
    }
}

/// The right clipping plane at the near plane, `HSD_CObjGetRight`.
#[doc(alias = "HSD_CObjGetRight")]
pub fn right(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    let Some(o) = c.map(|c| &cobjs[c]) else {
        return 0.0;
    };
    match o.projection_type {
        PERSPECTIVE => mem(fp::fmuls(
            reg(o.projection[1]),
            fp::fmuls(reg(o.near), half_height(o)),
        )),
        FRUSTUM | ORTHO => o.projection[3],
        _ => 0.0,
    }
}

/// Sets frustum or orthographic parameter `i`; a perspective ignores it.
fn set_plane(cobjs: &mut CObjs, c: Option<CObjId>, i: usize, v: f32) {
    let Some(c) = c else { return };
    if matches!(cobjs[c].projection_type, FRUSTUM | ORTHO) {
        cobjs[c].projection[i] = copy(v);
    }
}

/// `HSD_CObjSetTop`.
#[doc(alias = "HSD_CObjSetTop")]
pub fn set_top(cobjs: &mut CObjs, c: Option<CObjId>, top: f32) {
    set_plane(cobjs, c, 0, top);
}

/// `HSD_CObjSetBottom`.
#[doc(alias = "HSD_CObjSetBottom")]
pub fn set_bottom(cobjs: &mut CObjs, c: Option<CObjId>, bottom: f32) {
    set_plane(cobjs, c, 1, bottom);
}

/// `HSD_CObjSetLeft`.
#[doc(alias = "HSD_CObjSetLeft")]
pub fn set_left(cobjs: &mut CObjs, c: Option<CObjId>, left: f32) {
    set_plane(cobjs, c, 2, left);
}

/// `HSD_CObjSetRight`.
#[doc(alias = "HSD_CObjSetRight")]
pub fn set_right(cobjs: &mut CObjs, c: Option<CObjId>, right: f32) {
    set_plane(cobjs, c, 3, right);
}

/// `HSD_CObjGetNear`.
#[doc(alias = "HSD_CObjGetNear")]
pub fn near(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    c.map_or(0.0, |c| cobjs[c].near)
}

/// `HSD_CObjSetNear`.
#[doc(alias = "HSD_CObjSetNear")]
pub fn set_near(cobjs: &mut CObjs, c: Option<CObjId>, near: f32) {
    if let Some(c) = c {
        cobjs[c].near = copy(near);
    }
}

/// `HSD_CObjGetFar`.
#[doc(alias = "HSD_CObjGetFar")]
pub fn far(cobjs: &CObjs, c: Option<CObjId>) -> f32 {
    c.map_or(0.0, |c| cobjs[c].far)
}

/// `HSD_CObjSetFar`.
#[doc(alias = "HSD_CObjSetFar")]
pub fn set_far(cobjs: &mut CObjs, c: Option<CObjId>, far: f32) {
    if let Some(c) = c {
        cobjs[c].far = copy(far);
    }
}

/// `HSD_CObjGetScissor`: left, right, top, bottom.
#[doc(alias = "HSD_CObjGetScissor")]
pub fn scissor(cobjs: &CObjs, c: Option<CObjId>) -> Option<[u16; 4]> {
    c.map(|c| cobjs[c].scissor)
}

/// `HSD_CObjSetScissor` and `HSD_CObjSetScissorx4`: left, right, top, bottom.
#[doc(alias = "HSD_CObjSetScissor")]
#[doc(alias = "HSD_CObjSetScissorx4")]
pub fn set_scissor(cobjs: &mut CObjs, c: Option<CObjId>, scissor: [u16; 4]) {
    if let Some(c) = c {
        cobjs[c].scissor = scissor;
    }
}

/// `HSD_CObjGetViewportf`: xmin, xmax, ymin, ymax.
#[doc(alias = "HSD_CObjGetViewportf")]
pub fn viewport(cobjs: &CObjs, c: Option<CObjId>) -> Option<[f32; 4]> {
    c.map(|c| cobjs[c].viewport)
}

/// `HSD_CObjSetViewport`, from whole pixels.
#[doc(alias = "HSD_CObjSetViewport")]
pub fn set_viewport(cobjs: &mut CObjs, c: Option<CObjId>, viewport: [i16; 4]) {
    if let Some(c) = c {
        cobjs[c].viewport = viewport.map(|v| mem(fp::frsp(f64::from(v))));
    }
}

/// `HSD_CObjSetViewportf`, copied as a struct.
#[doc(alias = "HSD_CObjSetViewportf")]
pub fn set_viewportf(cobjs: &mut CObjs, c: Option<CObjId>, viewport: [f32; 4]) {
    if let Some(c) = c {
        cobjs[c].viewport = viewport;
    }
}

/// `HSD_CObjSetViewportfx4`, each value through a register.
#[doc(alias = "HSD_CObjSetViewportfx4")]
pub fn set_viewportfx4(cobjs: &mut CObjs, c: Option<CObjId>, viewport: [f32; 4]) {
    if let Some(c) = c {
        cobjs[c].viewport = viewport.map(copy);
    }
}

/// `HSD_CObjGetProjectionType`: [`PERSPECTIVE`] without a CObj.
#[doc(alias = "HSD_CObjGetProjectionType")]
pub fn projection_type(cobjs: &CObjs, c: Option<CObjId>) -> u8 {
    c.map_or(PERSPECTIVE, |c| cobjs[c].projection_type)
}

/// `HSD_CObjSetProjectionType`.
#[doc(alias = "HSD_CObjSetProjectionType")]
pub fn set_projection_type(cobjs: &mut CObjs, c: Option<CObjId>, ty: u32) {
    if let Some(c) = c {
        cobjs[c].projection_type = ty as u8;
    }
}

/// `HSD_CObjSetPerspective`: field of view in degrees and aspect ratio.
#[doc(alias = "HSD_CObjSetPerspective")]
pub fn set_perspective(cobjs: &mut CObjs, c: Option<CObjId>, fov: f32, aspect: f32) {
    if let Some(c) = c {
        let o = &mut cobjs[c];
        o.projection_type = PERSPECTIVE;
        o.projection[0] = copy(fov);
        o.projection[1] = copy(aspect);
    }
}

fn set_planes(cobjs: &mut CObjs, c: Option<CObjId>, ty: u8, planes: [f32; 4]) {
    if let Some(c) = c {
        let o = &mut cobjs[c];
        o.projection_type = ty;
        o.projection = planes.map(copy);
    }
}

/// `HSD_CObjSetFrustum`.
#[doc(alias = "HSD_CObjSetFrustum")]
pub fn set_frustum(
    cobjs: &mut CObjs,
    c: Option<CObjId>,
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
) {
    set_planes(cobjs, c, FRUSTUM, [top, bottom, left, right]);
}

/// `HSD_CObjSetOrtho`.
#[doc(alias = "HSD_CObjSetOrtho")]
pub fn set_ortho(
    cobjs: &mut CObjs,
    c: Option<CObjId>,
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
) {
    set_planes(cobjs, c, ORTHO, [top, bottom, left, right]);
}

/// Field of view and aspect, `HSD_CObjGetPerspective`; `None` unless a perspective.
#[doc(alias = "HSD_CObjGetPerspective")]
pub fn get_perspective(cobjs: &CObjs, c: Option<CObjId>) -> Option<(f32, f32)> {
    perspective(cobjs, c).map(|o| (o.projection[0], o.projection[1]))
}

/// Top, bottom, left and right, `HSD_CObjGetOrtho`; `None` unless orthographic.
#[doc(alias = "HSD_CObjGetOrtho")]
pub fn get_ortho(cobjs: &CObjs, c: Option<CObjId>) -> Option<[f32; 4]> {
    c.map(|c| &cobjs[c])
        .filter(|o| o.projection_type == ORTHO)
        .map(|o| o.projection)
}

/// `HSD_CObjGetFlags`.
#[doc(alias = "HSD_CObjGetFlags")]
pub fn flags(cobjs: &CObjs, c: CObjId) -> u32 {
    cobjs[c].flags
}

/// `HSD_CObjSetFlags`.
#[doc(alias = "HSD_CObjSetFlags")]
pub fn set_flags(cobjs: &mut CObjs, c: Option<CObjId>, flags: u32) {
    if let Some(c) = c {
        cobjs[c].flags |= flags;
    }
}

/// `HSD_CObjClearFlags`.
#[doc(alias = "HSD_CObjClearFlags")]
pub fn clear_flags(cobjs: &mut CObjs, c: Option<CObjId>, flags: u32) {
    if let Some(c) = c {
        cobjs[c].flags &= !flags;
    }
}

/// The camera the display pass is drawing with, `HSD_CObjGetCurrent`.
#[doc(alias = "HSD_CObjGetCurrent")]
pub fn current(cobjs: &CObjs) -> Option<CObjId> {
    cobjs.current
}
