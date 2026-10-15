//! Display objects, HAL's DObj (`sysdolphin/baselib/dobj.c`): a joint's list of drawable
//! parts, each a material (MObj) and its shapes (PObj), and their animation.

use std::ops::{Index, IndexMut};

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::AObjId;
use super::jobj;
use super::mobj::{self, MObjId};
use super::pobj::{self, PObjId};
use super::pool::Pool;

/// A DObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct DObjId(pub u32);

/// The display-object animation flag of `HSD_*ByFlags` (`DOBJ_ANIM`).
pub const DOBJ_ANIM: u32 = 0x2;
/// Every animation flag (`ALL_ANIM`).
pub const ALL_ANIM: u32 = 0x7FF;

/// `HSD_DObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DObj {
    pub next: Option<DObjId>,
    pub mobj: Option<MObjId>,
    pub pobj: Option<PObjId>,
    /// Nothing in the game gives a DObj an animation of its own; removal frees one.
    pub aobj: Option<AObjId>,
    pub flags: u32,
}

/// The DObj pool and `current_dobj`.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct DObjs {
    pub pool: Pool<DObj>,
    /// Set by `HSD_DObjSetCurrent`; nothing reads it.
    pub current: Option<DObjId>,
}

impl Index<DObjId> for DObjs {
    type Output = DObj;
    fn index(&self, d: DObjId) -> &DObj {
        &self.pool[d.0]
    }
}

impl IndexMut<DObjId> for DObjs {
    fn index_mut(&mut self, d: DObjId) -> &mut DObj {
        &mut self.pool[d.0]
    }
}

impl DObjs {
    /// A list's DObjs, first to last.
    pub fn list(&self, first: Option<DObjId>) -> impl Iterator<Item = DObjId> + '_ {
        std::iter::successors(first, |&d| self[d].next)
    }
}

/// `HSD_DObjSetCurrent`.
#[doc(alias = "HSD_DObjSetCurrent")]
pub fn set_current(dobjs: &mut DObjs, d: Option<DObjId>) {
    dobjs.current = d;
}

/// `HSD_DObjGetFlags`.
#[doc(alias = "HSD_DObjGetFlags")]
pub fn get_flags(dobjs: &DObjs, d: Option<DObjId>) -> u32 {
    d.map_or(0, |d| dobjs[d].flags)
}

/// `HSD_DObjSetFlags`.
#[doc(alias = "HSD_DObjSetFlags")]
pub fn set_flags(dobjs: &mut DObjs, d: Option<DObjId>, flags: u32) {
    if let Some(d) = d {
        dobjs[d].flags |= flags;
    }
}

/// `HSD_DObjClearFlags`.
#[doc(alias = "HSD_DObjClearFlags")]
pub fn clear_flags(dobjs: &mut DObjs, d: Option<DObjId>, flags: u32) {
    if let Some(d) = d {
        dobjs[d].flags &= !flags;
    }
}

/// Sets `flags` under `mask`, `HSD_DObjModifyFlags`.
#[doc(alias = "HSD_DObjModifyFlags")]
pub fn modify_flags(dobjs: &mut DObjs, d: Option<DObjId>, flags: u32, mask: u32) {
    if let Some(d) = d {
        let o = &mut dobjs[d];
        o.flags = (o.flags & !mask) | (flags & mask);
    }
}

/// Removes the animations `flags` names from each DObj of a list: its own if it has
/// [`DOBJ_ANIM`], then its shapes' and its material's by their flags,
/// `HSD_DObjRemoveAnimAllByFlags` (each through the inline `HSD_DObjRemoveAnimByFlags`).
#[doc(alias = "HSD_DObjRemoveAnimAllByFlags")]
pub fn remove_anim_all_by_flags(objs: &mut Objects, d: Option<DObjId>, flags: u32) {
    let mut cur = d;
    while let Some(d) = cur {
        if flags & DOBJ_ANIM != 0 {
            jobj::remove_aobj(objs, objs.dobjs[d].aobj);
            objs.dobjs[d].aobj = None;
        }
        pobj::remove_anim_all_by_flags(objs, objs.dobjs[d].pobj, flags);
        mobj::remove_anim_by_flags(objs, objs.dobjs[d].mobj, flags);
        cur = objs.dobjs[d].next;
    }
}

/// Gives each DObj of a list its shapes' animations from a chain of `HSD_ShapeAnimDObj`s and
/// its material's from a chain of `HSD_MatAnim`s, in step, `HSD_DObjAddAnimAll` (each through
/// the inline `HSD_DObjAddAnim`).
#[doc(alias = "HSD_DObjAddAnimAll")]
pub fn add_anim_all(
    objs: &mut Objects,
    data: &Archives,
    d: Option<DObjId>,
    matanim: Option<DataRef>,
    shapeanim_dobj: Option<DataRef>,
) {
    let (mut cur, mut ma, mut sd) = (d, matanim, shapeanim_dobj);
    while let Some(d) = cur {
        let shapeanim = sd.and_then(|sd| data.ptr(sd.at(4)));
        pobj::add_anim_all(objs, data, objs.dobjs[d].pobj, shapeanim);
        mobj::add_anim(objs, data, objs.dobjs[d].mobj, ma);
        cur = objs.dobjs[d].next;
        ma = ma.and_then(|m| data.ptr(m));
        sd = sd.and_then(|s| data.ptr(s));
    }
}

/// Restarts each DObj's shape and material animations by `flags` at `frame`,
/// `HSD_DObjReqAnimAllByFlags` (each through the inline `HSD_DObjReqAnimByFlags`).
#[doc(alias = "HSD_DObjReqAnimAllByFlags")]
pub fn req_anim_all_by_flags(objs: &mut Objects, d: Option<DObjId>, frame: f32, flags: u32) {
    let mut cur = d;
    while let Some(d) = cur {
        pobj::req_anim_all_by_flags(objs, objs.dobjs[d].pobj, frame, flags);
        mobj::req_anim_by_flags(objs, objs.dobjs[d].mobj, frame, flags);
        cur = objs.dobjs[d].next;
    }
}

/// Restarts every animation of each DObj of a list at `frame`, `HSD_DObjReqAnimAll`.
#[doc(alias = "HSD_DObjReqAnimAll")]
pub fn req_anim_all(objs: &mut Objects, d: Option<DObjId>, frame: f32) {
    req_anim_all_by_flags(objs, d, frame, ALL_ANIM);
}

/// Advances each DObj's shape animations, then its material's, one frame, `HSD_DObjAnimAll`
/// (each through the inline `HSD_DObjAnim`).
#[doc(alias = "HSD_DObjAnimAll")]
pub fn anim_all(objs: &mut Objects, data: &Archives, d: Option<DObjId>) {
    let mut cur = d;
    while let Some(d) = cur {
        pobj::anim_all(objs, data, objs.dobjs[d].pobj);
        mobj::anim(objs, data, objs.dobjs[d].mobj);
        cur = objs.dobjs[d].next;
    }
}

/// `HSD_DObjAlloc`.
#[doc(alias = "HSD_DObjAlloc")]
pub fn alloc(dobjs: &mut DObjs) -> DObjId {
    let d = dobjs.pool.alloc(DObj::default());
    dobjs
        .pool
        .set_addr(d, super::heap::class_alloc(super::heap::Class::DObj));
    DObjId(d)
}

/// Builds a DObj list, its materials and its shapes from their descriptors, each DObj before
/// those after it, `HSD_DObjLoadDesc` (and `DObjLoad`). The material's render mode sets the
/// blending flags.
#[doc(alias = "HSD_DObjLoadDesc")]
#[doc(alias = "DObjLoad")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<DObjId> {
    let desc = desc?;
    assert!(
        data.ptr(desc).is_none(),
        "DObjDesc {desc:?} names a class, which isn't supported"
    );
    let d = alloc(&mut objs.dobjs);
    let next = load_desc(objs, data, data.ptr(desc.at(4)));
    objs.dobjs[d].next = next;
    let m = mobj::load_desc(objs, data, data.ptr(desc.at(8)));
    objs.dobjs[d].mobj = m;
    let p = pobj::load_desc(&mut objs.pobjs, data, data.ptr(desc.at(0xC)));
    objs.dobjs[d].pobj = p;
    if let Some(m) = m {
        let mode = objs.mobjs[m].rendermode;
        let flags = match mode & 0x6000_0000 {
            0 => 2,
            0x4000_0000 => 8,
            0x6000_0000 => 4,
            _ => panic!("mobj has unexpected blending flags ({mode:#x})"),
        };
        modify_flags(&mut objs.dobjs, Some(d), flags, 0xE);
    }
    Some(d)
}

/// Frees a DObj's material, shapes and animation, `DObjRelease`.
#[doc(alias = "DObjRelease")]
fn release(objs: &mut Objects, d: DObjId) {
    mobj::remove(objs, objs.dobjs[d].mobj);
    pobj::remove_all(objs, objs.dobjs[d].pobj);
    jobj::remove_aobj(objs, objs.dobjs[d].aobj);
}

/// Frees a DObj list, first first, `HSD_DObjRemoveAll` (each through the inline
/// `HSD_DObjRemove`).
#[doc(alias = "HSD_DObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, d: Option<DObjId>) {
    let mut cur = d;
    while let Some(d) = cur {
        cur = objs.dobjs[d].next;
        release(objs, d);
        super::heap::class_free(super::heap::Class::DObj, objs.dobjs.pool.addr(d.0));
        objs.dobjs.pool.free(d.0);
    }
}

/// Binds each DObj's shapes to the joints their descriptors name, `HSD_DObjResolveRefsAll`.
#[doc(alias = "HSD_DObjResolveRefsAll")]
pub fn resolve_refs_all(
    objs: &mut Objects,
    data: &Archives,
    d: Option<DObjId>,
    desc: Option<DataRef>,
) {
    let (mut d, mut de) = (d, desc);
    while let (Some(dobj), Some(desc)) = (d, de) {
        pobj::resolve_refs_all(objs, data, objs.dobjs[dobj].pobj, data.ptr(desc.at(0xC)));
        d = objs.dobjs[dobj].next;
        de = data.ptr(desc.at(4));
    }
}
