//! World objects, HAL's WObj (`sysdolphin/baselib/wobj.c`): a position a camera or light looks
//! from or at, with an animation that moves it (along its bound joint's spline, or axis by
//! axis) and reference objects (RObjs) the animation can switch.

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj::{self, Contents};
use super::object::RefCount;
use super::pool::Pool;
use super::robj::{self, RObjId};
use super::spline;
use crate::fpu::copy;
use crate::vec::Vec3;

/// A WObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct WObjId(pub u32);

/// The position is a point on the animation's path, in its joint's frame: it is transformed
/// by the joint's matrix before use.
pub const PATH_RELATIVE: u32 = 1;
/// The position changed since a camera last built its view matrix from it.
pub const DIRTY: u32 = 2;

/// `HSD_WObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct WObj {
    pub ref_count: RefCount,
    pub flags: u32,
    pub pos: Vec3,
    pub aobj: Option<AObjId>,
    pub robj: Option<RObjId>,
}

/// The WObj pool. The original allocates WObjs as class objects (`hsdNew`), from memory pieces
/// shared with other classes of the same size; a typed pool keeps the reuse order among WObjs.
pub type WObjs = Pool<WObj>;

/// A three-float vector in game data, copied as the original copies structs (bit for bit).
pub(crate) fn vec_at(data: &Archives, r: DataRef) -> Vec3 {
    Vec3::new(data.f32(r), data.f32(r.at(4)), data.f32(r.at(8)))
}

/// Brings a path-relative position into world space, by the path joint's matrix (set up
/// first) when the animation still binds one.
fn resolve(objs: &mut Objects, data: &Archives, w: WObjId) {
    if objs.wobjs[w.0].flags & PATH_RELATIVE == 0 {
        return;
    }
    let path = objs.wobjs[w.0].aobj.and_then(|a| objs.aobjs[a].hsd_obj);
    if let Some(j) = path {
        jobj::setup_matrix(objs, data, Some(j));
        let o = &mut objs.wobjs[w.0];
        o.pos = objs.jobjs[j].mtx.transform(o.pos);
    }
    objs.wobjs[w.0].flags &= !PATH_RELATIVE;
}

fn set(o: &mut WObj, pos: Vec3) {
    o.pos = pos;
    o.flags |= DIRTY;
    o.flags &= !PATH_RELATIVE;
}

fn set_axis(objs: &mut Objects, data: &Archives, w: WObjId, axis: usize, val: f32) {
    resolve(objs, data, w);
    let o = &mut objs.wobjs[w.0];
    let v = copy(val);
    match axis {
        0 => o.pos.x = v,
        1 => o.pos.y = v,
        _ => o.pos.z = v,
    }
    o.flags |= DIRTY;
}

/// Hands a WObj one animated value, `WObjUpdateFunc`: it moves the position, to a point along
/// the path joint's spline (4) or on one axis (5 to 7).
#[doc(alias = "WObjUpdateFunc")]
pub fn update(objs: &mut Objects, data: &Archives, w: WObjId, ty: u8, value: Value) {
    match ty {
        4 => {
            // Any address is negative, so a stale value is 0 here.
            let mut t = value.or_address();
            if f64::from(t) < 0.0 {
                t = 0.0;
            }
            if 1.0 < f64::from(t) {
                t = 1.0;
            }
            let a = objs.wobjs[w.0].aobj.expect("wobj->aobj");
            let path = objs.aobjs[a].hsd_obj.expect("jp");
            let Contents::Spline(Some(s)) = objs.jobjs[path].u else {
                panic!("jp->u.spline")
            };
            let Some(p) = spline::arc_length_point(data, s, t) else {
                // The original moves the position to whatever its stack held.
                objs.stale_reads += 1;
                return;
            };
            let o = &mut objs.wobjs[w.0];
            set(o, p);
            o.flags |= PATH_RELATIVE;
        }
        5..=7 => {
            if matches!(value, Value::Stale) {
                objs.stale_reads += 1;
            }
            set_axis(objs, data, w, usize::from(ty - 5), value.or_address());
        }
        _ => {}
    }
}

/// A new WObj, `HSD_WObjAlloc`.
#[doc(alias = "HSD_WObjAlloc")]
pub fn alloc(wobjs: &mut WObjs) -> WObjId {
    let w = wobjs.alloc(WObj::default());
    wobjs.set_addr(w, super::heap::class_alloc(super::heap::Class::WObj));
    WObjId(w)
}

/// Sets a WObj up from a descriptor, its position and RObjs, `HSD_WObjInit` (`WObjLoad`
/// without the null checks).
#[doc(alias = "HSD_WObjInit")]
#[doc(alias = "WObjLoad")]
pub fn init(objs: &mut Objects, data: &Archives, w: Option<WObjId>, desc: Option<DataRef>) {
    let (Some(w), Some(desc)) = (w, desc) else {
        return;
    };
    set(&mut objs.wobjs[w.0], vec_at(data, desc.at(4)));
    if let Some(r) = objs.wobjs[w.0].robj {
        robj::remove_all(objs, Some(r));
    }
    let rdesc = data.ptr(desc.at(0x10));
    let r = robj::load_desc(objs, data, rdesc);
    objs.wobjs[w.0].robj = r;
    robj::resolve_refs_all(objs, data, r, rdesc);
}

/// A WObj loaded from a descriptor in the executable's data: at `pos`, with no references.
pub fn load_static(objs: &mut Objects, pos: Vec3) -> WObjId {
    let w = alloc(&mut objs.wobjs);
    set(&mut objs.wobjs[w.0], pos);
    w
}

/// A WObj loaded from a descriptor, `HSD_WObjLoadDesc`. A descriptor naming a class panics:
/// melee-sim has no class registry, and nothing in the game names one.
#[doc(alias = "HSD_WObjLoadDesc")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<WObjId> {
    let desc = desc?;
    assert!(data.ptr(desc).is_none(), "WObjDesc {desc:?} names a class");
    let w = alloc(&mut objs.wobjs);
    init(objs, data, Some(w), Some(desc));
    Some(w)
}

/// `HSD_WObjSetPosition`.
#[doc(alias = "HSD_WObjSetPosition")]
pub fn set_position(wobjs: &mut WObjs, w: Option<WObjId>, pos: Vec3) {
    if let Some(w) = w {
        set(&mut wobjs[w.0], pos);
    }
}

/// `HSD_WObjSetPositionX`.
#[doc(alias = "HSD_WObjSetPositionX")]
pub fn set_position_x(objs: &mut Objects, data: &Archives, w: Option<WObjId>, x: f32) {
    if let Some(w) = w {
        set_axis(objs, data, w, 0, x);
    }
}

/// `HSD_WObjSetPositionY`.
#[doc(alias = "HSD_WObjSetPositionY")]
pub fn set_position_y(objs: &mut Objects, data: &Archives, w: Option<WObjId>, y: f32) {
    if let Some(w) = w {
        set_axis(objs, data, w, 1, y);
    }
}

/// `HSD_WObjSetPositionZ`.
#[doc(alias = "HSD_WObjSetPositionZ")]
pub fn set_position_z(objs: &mut Objects, data: &Archives, w: Option<WObjId>, z: f32) {
    if let Some(w) = w {
        set_axis(objs, data, w, 2, z);
    }
}

/// The position in world space, `HSD_WObjGetPosition`; `None` (the original writes nothing)
/// without a WObj.
#[doc(alias = "HSD_WObjGetPosition")]
pub fn position(objs: &mut Objects, data: &Archives, w: Option<WObjId>) -> Option<Vec3> {
    let w = w?;
    resolve(objs, data, w);
    Some(objs.wobjs[w.0].pos)
}

/// The inline `HSD_WObjClearFlags`.
pub fn clear_flags(wobjs: &mut WObjs, w: WObjId, flags: u32) {
    wobjs[w.0].flags &= !flags;
}

/// Frees a WObj's animation and its RObjs', `HSD_WObjRemoveAnim`.
#[doc(alias = "HSD_WObjRemoveAnim")]
pub fn remove_anim(objs: &mut Objects, w: Option<WObjId>) {
    let Some(w) = w else { return };
    jobj::remove_aobj(objs, objs.wobjs[w.0].aobj);
    objs.wobjs[w.0].aobj = None;
    robj::remove_anim_all(objs, objs.wobjs[w.0].robj);
}

/// Restarts a WObj's animation and its RObjs' at `frame`, `HSD_WObjReqAnim`.
#[doc(alias = "HSD_WObjReqAnim")]
pub fn req_anim(objs: &mut Objects, w: Option<WObjId>, frame: f32) {
    let Some(w) = w else { return };
    let a = objs.wobjs[w.0].aobj;
    aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
    robj::req_anim_all(objs, objs.wobjs[w.0].robj, frame);
}

/// Gives a WObj and its RObjs the animations of an `HSD_WObjAnim`, replacing the WObj's,
/// `HSD_WObjAddAnim`.
#[doc(alias = "HSD_WObjAddAnim")]
pub fn add_anim(objs: &mut Objects, data: &Archives, w: Option<WObjId>, anim: Option<DataRef>) {
    let (Some(w), Some(anim)) = (w, anim) else {
        return;
    };
    if let Some(a) = objs.wobjs[w.0].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    objs.wobjs[w.0].aobj = jobj::load_aobj(objs, data, data.ptr(anim));
    robj::add_anim_all(objs, data, objs.wobjs[w.0].robj, data.ptr(anim.at(4)));
}

/// Advances a WObj's animation and its RObjs' one frame, moving its position,
/// `HSD_WObjInterpretAnim`.
#[doc(alias = "HSD_WObjInterpretAnim")]
pub fn interpret_anim(objs: &mut Objects, data: &Archives, w: Option<WObjId>) {
    let Some(w) = w else { return };
    let a = objs.wobjs[w.0].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, data, w, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
    robj::anim_all(objs, data, objs.wobjs[w.0].robj);
}

/// Drops an owner of a WObj, destroying it with the last, the inline `HSD_WObjUnref`.
pub fn unref(objs: &mut Objects, w: Option<WObjId>) {
    let Some(w) = w else { return };
    if objs.wobjs[w.0].ref_count.dec() {
        delete(objs, w);
    }
}

/// Frees a WObj with its RObjs and animation, its class's release (`WObjRelease`) and
/// destroy.
#[doc(alias = "WObjRelease")]
pub fn delete(objs: &mut Objects, w: WObjId) {
    super::export::with_image(|img| super::export::wobj(objs, img, w));
    robj::remove_all(objs, objs.wobjs[w.0].robj);
    jobj::remove_aobj(objs, objs.wobjs[w.0].aobj);
    super::heap::class_free(super::heap::Class::WObj, objs.wobjs.addr(w.0));
    objs.wobjs.free(w.0);
}
