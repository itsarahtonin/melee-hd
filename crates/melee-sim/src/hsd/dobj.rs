//! Display objects, HAL's DObj (`sysdolphin/baselib/dobj.c`): a joint's list of drawable
//! parts, each a material (MObj) and its shapes (PObj).
//!
//! Materials aren't ported yet: loading reads the material descriptor's render mode for the
//! blending flags, as `DObjLoad` does after loading the material, and keeps the descriptor.

use std::ops::{Index, IndexMut};

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::pobj::{self, PObjId};
use super::pool::Pool;

/// A DObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DObjId(pub u32);

/// `HSD_DObj`, without its material yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DObj {
    pub next: Option<DObjId>,
    /// The material's descriptor, until MObj loads it.
    pub mobj_desc: Option<DataRef>,
    pub pobj: Option<PObjId>,
    pub flags: u32,
}

/// The DObj pool.
#[derive(Clone, Debug, Default)]
pub struct DObjs {
    pub pool: Pool<DObj>,
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

/// Sets `flags` under `mask`, `HSD_DObjModifyFlags`.
fn modify_flags(d: &mut DObj, flags: u32, mask: u32) {
    d.flags = (d.flags & !mask) | (flags & mask);
}

/// Builds a DObj list from its descriptors, `HSD_DObjLoadDesc` (pending materials).
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<DObjId> {
    let desc = desc?;
    assert!(
        data.ptr(desc).is_none(),
        "DObjDesc {desc:?} names a class, which isn't supported"
    );
    let d = DObjId(objs.dobjs.pool.alloc(DObj::default()));
    let next = load_desc(objs, data, data.ptr(desc.at(4)));
    let mobj_desc = data.ptr(desc.at(8));
    let p = pobj::load_desc(&mut objs.pobjs, data, data.ptr(desc.at(0xC)));
    let o = &mut objs.dobjs[d];
    o.next = next;
    o.mobj_desc = mobj_desc;
    o.pobj = p;
    if let Some(m) = mobj_desc {
        match data.u32(m.at(4)) & 0x6000_0000 {
            0 => modify_flags(o, 2, 0xE),
            0x4000_0000 => modify_flags(o, 8, 0xE),
            0x6000_0000 => modify_flags(o, 4, 0xE),
            mode => panic!("mobj has unexpected blending flags ({mode:#x})"),
        }
    }
    Some(d)
}

/// Frees a DObj list and its shapes, `HSD_DObjRemoveAll` (pending materials).
pub fn remove_all(objs: &mut Objects, d: Option<DObjId>) {
    let mut cur = d;
    while let Some(d) = cur {
        cur = objs.dobjs[d].next;
        pobj::remove_all(objs, objs.dobjs[d].pobj);
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
