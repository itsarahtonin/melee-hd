//! Animation objects, HAL's AObj (`sysdolphin/baselib/aobj.c`): the clock of one animated
//! object (a joint, a material, a texture...). Each frame it advances the object's FObj
//! channels by its rate, loops them back to its rewind frame or stops them at its end frame,
//! and the channels hand their values to the object's update function.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::fobj::{self, FObjId, FObjs, Update, again};
use super::jobj::JObjId;
use super::pool::Pool;
use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::math;

/// An AObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct AObjId(pub u32);

/// The last frame looped back to the rewind frame (or held at the end frame).
pub const REWINDED: u32 = 1 << 26;
/// The next frame plays the requested frame itself rather than advancing past it.
pub const FIRST_PLAY: u32 = 1 << 27;
/// Advance the channels without handing their values to the update function.
pub const NO_UPDATE: u32 = 1 << 28;
/// Loop from the end frame back to the rewind frame instead of stopping.
pub const LOOP: u32 = 1 << 29;
/// Stopped.
pub const NO_ANIM: u32 = 1 << 30;

/// `HSD_AObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct AObj {
    pub flags: u32,
    pub curr_frame: f32,
    pub rewind_frame: f32,
    pub end_frame: f32,
    pub framerate: f32,
    pub fobj: Option<FObjId>,
    /// The joint the descriptor binds (`hsd_obj`): a path animation's spline, or the frame
    /// its translation is in. The AObj holds a reference to it.
    pub hsd_obj: Option<JObjId>,
}

/// The AObj pool (`aobj_alloc_data`) and the counts `HSD_AObjInvokeCallBacks` reads. A new
/// one is `HSD_AObjInitAllocData`.
#[doc(alias = "HSD_AObjInitAllocData")]
#[doc(alias = "HSD_AObjGetAllocData")]
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct AObjs {
    pub pool: Pool<AObj, { pool_kind(Kind::AObj) }>,
    /// AObjs that were stopped after being interpreted, since `init_end_callback`
    /// (`HSD_AObj_804D762C`).
    pub stopped: i32,
    /// AObjs still playing after being interpreted (`HSD_AObj_804D7630`).
    pub playing: i32,
}

impl Index<AObjId> for AObjs {
    type Output = AObj;
    fn index(&self, a: AObjId) -> &AObj {
        &self.pool[a.0]
    }
}

impl IndexMut<AObjId> for AObjs {
    fn index_mut(&mut self, a: AObjId) -> &mut AObj {
        &mut self.pool[a.0]
    }
}

/// A new AObj, stopped, at rate 1, `HSD_AObjAlloc`.
#[doc(alias = "HSD_AObjAlloc")]
pub fn alloc(aobjs: &mut AObjs) -> AObjId {
    AObjId(aobjs.pool.alloc(AObj {
        flags: NO_ANIM,
        framerate: 1.0,
        ..AObj::default()
    }))
}

/// Returns an AObj to the pool, `HSD_AObjFree`.
#[doc(alias = "HSD_AObjFree")]
pub fn free(aobjs: &mut AObjs, a: Option<AObjId>) {
    if let Some(a) = a {
        aobjs.pool.free(a.0);
    }
}

/// `HSD_AObjGetFlags`.
#[doc(alias = "HSD_AObjGetFlags")]
pub fn flags(aobjs: &AObjs, a: Option<AObjId>) -> u32 {
    a.map_or(0, |a| aobjs[a].flags)
}

/// Sets [`LOOP`] and [`NO_UPDATE`] where `flags` has them, `HSD_AObjSetFlags`.
#[doc(alias = "HSD_AObjSetFlags")]
pub fn set_flags(aobjs: &mut AObjs, a: Option<AObjId>, flags: u32) {
    if let Some(a) = a {
        aobjs[a].flags |= flags & (LOOP | NO_UPDATE);
    }
}

/// Clears [`LOOP`] and [`NO_UPDATE`] where `flags` has them, `HSD_AObjClearFlags`.
#[doc(alias = "HSD_AObjClearFlags")]
pub fn clear_flags(aobjs: &mut AObjs, a: Option<AObjId>, flags: u32) {
    if let Some(a) = a {
        aobjs[a].flags &= !(flags & (LOOP | NO_UPDATE));
    }
}

/// Gives an AObj a new FObj chain, freeing the one it had, `HSD_AObjSetFObj`.
#[doc(alias = "HSD_AObjSetFObj")]
pub fn set_fobj(aobjs: &mut AObjs, fobjs: &mut FObjs, a: Option<AObjId>, fobj: Option<FObjId>) {
    let Some(a) = a else { return };
    if let Some(old) = aobjs[a].fobj {
        fobj::remove_all(fobjs, Some(old));
    }
    aobjs[a].fobj = fobj;
}

/// Starts counting stopped and playing AObjs afresh, `HSD_AObjInitEndCallBack`.
#[doc(alias = "HSD_AObjInitEndCallBack")]
pub fn init_end_callback(aobjs: &mut AObjs) {
    aobjs.stopped = 0;
    aobjs.playing = 0;
}

/// `HSD_AObjInvokeCallBacks`: once AObjs have stopped and none still plays, the original
/// calls each function on `endcallback_list`. Nothing in the game adds one (the list is only
/// ever cleared, by `_HSD_AObjForgetMemory`), so there is never anything to call.
#[doc(alias = "HSD_AObjInvokeCallBacks")]
pub fn invoke_end_callbacks(_aobjs: &AObjs) {}

/// Restarts an AObj and its channels at `frame`, `HSD_AObjReqAnim`.
#[doc(alias = "HSD_AObjReqAnim")]
pub fn req_anim(aobjs: &mut AObjs, fobjs: &mut FObjs, a: Option<AObjId>, frame: f32) {
    let Some(a) = a else { return };
    let o = &mut aobjs[a];
    o.curr_frame = copy(frame);
    o.flags = (o.flags & !NO_ANIM) | FIRST_PLAY;
    fobj::req_anim_all(fobjs, o.fobj, frame);
}

/// Stops an AObj, first delivering its channels' pending keys at its rate,
/// `HSD_AObjStopAnim`.
#[doc(alias = "HSD_AObjStopAnim")]
pub fn stop_anim(objs: &mut Objects, data: &Archives, a: Option<AObjId>, update: Update) {
    let Some(a) = a else { return };
    let o = &objs.aobjs[a];
    fobj::stop_anim_all(objs, data, o.fobj, o.framerate, update);
    objs.aobjs[a].flags |= NO_ANIM;
}

/// The current frame wrapped into `[rewind, end)`: the inline `getLoopedFrame`.
fn looped_frame(o: &AObj) -> f64 {
    let y = fp::fsubs(reg(o.end_frame), reg(o.rewind_frame));
    let x = fp::fsubs(reg(o.curr_frame), reg(o.rewind_frame));
    fp::fadds(reg(o.rewind_frame), math::fmod(x, y))
}

/// Advances an AObj one frame: by its rate, or not at all on the first frame after a
/// request; loops or stops it at its end frame; and runs its channels, handing their values
/// to `update` unless [`NO_UPDATE`] is set. `HSD_AObjInterpretAnim`.
#[doc(alias = "HSD_AObjInterpretAnim")]
pub fn interpret(objs: &mut Objects, data: &Archives, a: Option<AObjId>, mut update: Update) {
    let Some(a) = a else { return };
    let o = &mut objs.aobjs[a];
    if o.flags & NO_ANIM != 0 {
        return;
    }
    let mut rate;
    if o.flags & FIRST_PLAY != 0 {
        o.flags &= 0xF7FF_FFFF;
        rate = 0.0;
    } else {
        rate = o.framerate;
        o.curr_frame = mem(fp::fadds(reg(o.curr_frame), reg(o.framerate)));
    }
    if o.flags & LOOP != 0 && reg(o.end_frame) <= reg(o.curr_frame) {
        if reg(o.rewind_frame) < reg(o.end_frame) {
            let first = o.fobj;
            fobj::stop_anim_all(objs, data, first, rate, again(&mut update));
            let o = &mut objs.aobjs[a];
            o.curr_frame = mem(looped_frame(o));
            let (first, frame) = (o.fobj, o.curr_frame);
            fobj::req_anim_all(&mut objs.fobjs, first, frame);
        } else {
            o.curr_frame = copy(o.end_frame);
        }
        rate = 0.0;
        objs.aobjs[a].flags |= REWINDED;
    } else {
        o.flags &= 0xFBFF_FFFF;
    }
    let o = &objs.aobjs[a];
    let first = o.fobj;
    if o.flags & NO_UPDATE != 0 {
        fobj::interpret_all(objs, data, first, rate, None);
    } else {
        fobj::interpret_all(objs, data, first, rate, again(&mut update));
    }
    // `f1`: the end frame the compare below loads, or past the end the rate and whatever
    // the channels' stop leaves; a looping AObj leaves what its channels did.
    let o = &objs.aobjs[a];
    let ended = o.flags & LOOP == 0 && reg(o.end_frame) <= reg(o.curr_frame);
    objs.aobj_f1 = (o.flags & LOOP == 0 && !ended).then_some(o.end_frame);
    if ended {
        // HSD_AObjStopAnim, inlined.
        let (first, rate) = (o.fobj, o.framerate);
        fobj::stop_anim_all(objs, data, first, rate, update);
        objs.aobjs[a].flags |= NO_ANIM;
    }
    let aobjs = &mut objs.aobjs;
    if aobjs[a].flags & NO_ANIM != 0 {
        aobjs.stopped = aobjs.stopped.wrapping_add(1);
    } else {
        aobjs.playing = aobjs.playing.wrapping_add(1);
    }
}

/// Builds an AObj and its FObj chain from a descriptor that binds no joint,
/// `HSD_AObjLoadDesc`. [`jobj::load_aobj`](super::jobj::load_aobj) handles any descriptor.
#[doc(alias = "HSD_AObjLoadDesc")]
pub fn load_desc(
    aobjs: &mut AObjs,
    fobjs: &mut FObjs,
    data: &Archives,
    desc: Option<DataRef>,
) -> Option<AObjId> {
    let (a, binds) = load_desc_unbound(aobjs, fobjs, data, desc)?;
    assert!(
        binds.is_none(),
        "AObjDesc {desc:?} binds a joint: use jobj::load_aobj"
    );
    Some(a)
}

/// `HSD_AObjLoadDesc` up to binding: the AObj, and the joint descriptor it binds.
pub(crate) fn load_desc_unbound(
    aobjs: &mut AObjs,
    fobjs: &mut FObjs,
    data: &Archives,
    desc: Option<DataRef>,
) -> Option<(AObjId, Option<DataRef>)> {
    let desc = desc?;
    let a = alloc(aobjs);
    set_flags(aobjs, Some(a), data.u32(desc));
    set_rewind_frame(aobjs, Some(a), 0.0);
    set_end_frame(aobjs, Some(a), data.f32(desc.at(4)));
    let f = fobj::load_desc(fobjs, data, data.ptr(desc.at(8)));
    set_fobj(aobjs, fobjs, Some(a), f);
    Some((a, data.ptr(desc.at(0xC))))
}

/// Frees an AObj and its FObj chain, `HSD_AObjRemove`.
#[doc(alias = "HSD_AObjRemove")]
///
/// An AObj bound to a joint is removed with [`jobj::remove_aobj`](super::jobj::remove_aobj),
/// which drops its reference.
pub fn remove(aobjs: &mut AObjs, fobjs: &mut FObjs, a: Option<AObjId>) {
    let Some(a) = a else { return };
    assert!(
        aobjs[a].hsd_obj.is_none(),
        "AObj bound to a joint: use jobj::remove_aobj"
    );
    if let Some(f) = aobjs[a].fobj {
        fobj::remove_all(fobjs, Some(f));
    }
    aobjs[a].fobj = None;
    free(aobjs, Some(a));
}

/// `HSD_AObjSetRate`.
#[doc(alias = "HSD_AObjSetRate")]
pub fn set_rate(aobjs: &mut AObjs, a: Option<AObjId>, rate: f32) {
    if let Some(a) = a {
        aobjs[a].framerate = copy(rate);
    }
}

/// `HSD_AObjSetRewindFrame`.
#[doc(alias = "HSD_AObjSetRewindFrame")]
pub fn set_rewind_frame(aobjs: &mut AObjs, a: Option<AObjId>, frame: f32) {
    if let Some(a) = a {
        aobjs[a].rewind_frame = copy(frame);
    }
}

/// `HSD_AObjSetEndFrame`.
#[doc(alias = "HSD_AObjSetEndFrame")]
pub fn set_end_frame(aobjs: &mut AObjs, a: Option<AObjId>, frame: f32) {
    if let Some(a) = a {
        aobjs[a].end_frame = copy(frame);
    }
}

/// Moves a playing AObj and its channels to `frame`; a stopped one stays as it is,
/// `HSD_AObjSetCurrentFrame`.
#[doc(alias = "HSD_AObjSetCurrentFrame")]
pub fn set_current_frame(aobjs: &mut AObjs, fobjs: &mut FObjs, a: Option<AObjId>, frame: f32) {
    let Some(a) = a else { return };
    let o = &mut aobjs[a];
    if o.flags & NO_ANIM == 0 {
        o.curr_frame = copy(frame);
        o.flags = (o.flags & 0xBFFF_FFFF) | FIRST_PLAY;
        fobj::req_anim_all(fobjs, o.fobj, frame);
    }
}
