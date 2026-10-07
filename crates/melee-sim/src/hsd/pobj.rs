//! Shapes, HAL's PObj (`sysdolphin/baselib/pobj.c`): a display object's geometry. The vertex
//! data and display lists stay in the archive; the state holds what play changes or refers
//! to: the joints a skinned or enveloped shape is bound to, and a morphing shape's blend.

use std::ops::{Index, IndexMut};

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::jobj::{self, JObjId};
use super::pool::Pool;
use crate::fpu::copy;

/// A PObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PObjId(pub u32);

/// The shape type, in the flags' bits 12 and 13 (`pobj_type`).
pub const TYPE_MASK: u16 = 0x3000;
/// Bound rigidly to one joint (or none).
pub const SKIN: u16 = 0;
/// Morphing between shapes.
pub const SHAPEANIM: u16 = 1 << 12;
/// Weighted between joints.
pub const ENVELOPE: u16 = 2 << 12;

/// A morphing shape's blend weights adds the shapes rather than interpolating two.
pub const SHAPESET_ADDITIVE: u16 = 1 << 1;

/// One joint's weight in an envelope (`HSD_Envelope`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Envelope {
    pub jobj: Option<JObjId>,
    pub weight: f32,
}

/// `HSD_ShapeSet`: the shapes a morphing shape blends, in the archive, and its blend state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeSet {
    pub flags: u16,
    pub nb_shape: u16,
    pub nb_vertex_index: i32,
    pub vertex_desc: Option<DataRef>,
    pub vertex_idx_list: Option<DataRef>,
    pub nb_normal_index: i32,
    pub normal_desc: Option<DataRef>,
    pub normal_idx_list: Option<DataRef>,
    /// The blend: one weight per shape when additive, else one position between shapes.
    pub blend: Vec<f32>,
    pub aobj: Option<super::aobj::AObjId>,
}

/// What a shape is bound to, the original's union `u`, by its type.
#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Skin(Option<JObjId>),
    ShapeSet(ShapeSet),
    /// The envelopes, one per matrix the display lists index: each a weighted list of joints
    /// (`HSD_SList`s of `HSD_Envelope` chains in the original).
    Envelopes(Vec<Vec<Envelope>>),
}

/// `HSD_PObj`.
#[derive(Clone, Debug, PartialEq)]
pub struct PObj {
    pub next: Option<PObjId>,
    pub verts: Option<DataRef>,
    pub flags: u16,
    pub n_display: u16,
    pub display: Option<DataRef>,
    pub u: Binding,
}

impl PObj {
    pub fn ty(&self) -> u16 {
        self.flags & TYPE_MASK
    }
}

/// The PObj pool.
#[derive(Clone, Debug, Default)]
pub struct PObjs {
    pub pool: Pool<PObj>,
}

impl Index<PObjId> for PObjs {
    type Output = PObj;
    fn index(&self, p: PObjId) -> &PObj {
        &self.pool[p.0]
    }
}

impl IndexMut<PObjId> for PObjs {
    fn index_mut(&mut self, p: PObjId) -> &mut PObj {
        &mut self.pool[p.0]
    }
}

/// `HSD_PObjGetFlags`.
#[doc(alias = "HSD_PObjGetFlags")]
pub fn get_flags(pobjs: &PObjs, p: Option<PObjId>) -> u32 {
    p.map_or(0, |p| u32::from(pobjs[p].flags))
}

/// A shape list from its descriptors, `HSD_PObjLoadDesc` (and `PObjLoad`).
#[doc(alias = "HSD_PObjLoadDesc")]
#[doc(alias = "PObjLoad")]
pub fn load_desc(pobjs: &mut PObjs, data: &Archives, desc: Option<DataRef>) -> Option<PObjId> {
    let desc = desc?;
    assert!(
        data.ptr(desc).is_none(),
        "PObjDesc {desc:?} names a class, which isn't supported"
    );
    let p = PObjId(pobjs.pool.alloc(PObj {
        next: None,
        verts: None,
        flags: 0,
        n_display: 0,
        display: None,
        u: Binding::Skin(None),
    }));
    let next = load_desc(pobjs, data, data.ptr(desc.at(4)));
    let flags = data.u16(desc.at(0xC));
    let at = desc.at(0x14);
    let u = match flags & TYPE_MASK {
        SHAPEANIM => Binding::ShapeSet(load_shape_set(data, data.ptr(at).expect("shape set"))),
        ENVELOPE => Binding::Envelopes(load_envelopes(data, data.ptr(at))),
        SKIN => Binding::Skin(None),
        _ => panic!("pobj: unexected type."),
    };
    pobjs[p] = PObj {
        next,
        verts: data.ptr(desc.at(8)),
        flags,
        n_display: data.u16(desc.at(0xE)),
        display: data.ptr(desc.at(0x10)),
        u,
    };
    Some(p)
}

/// The static `loadShapeSetDesc`.
fn load_shape_set(data: &Archives, d: DataRef) -> ShapeSet {
    let flags = data.u16(d);
    let nb_shape = data.u16(d.at(2));
    ShapeSet {
        flags,
        nb_shape,
        nb_vertex_index: data.u32(d.at(4)) as i32,
        vertex_desc: data.ptr(d.at(8)),
        vertex_idx_list: data.ptr(d.at(0xC)),
        nb_normal_index: data.u32(d.at(0x10)) as i32,
        normal_desc: data.ptr(d.at(0x14)),
        normal_idx_list: data.ptr(d.at(0x18)),
        blend: vec![
            0.0;
            if flags & SHAPESET_ADDITIVE != 0 {
                nb_shape as usize
            } else {
                1
            }
        ],
        aobj: None,
    }
}

/// The envelopes' weights, joints unresolved, the static `loadEnvelopeDesc`.
fn load_envelopes(data: &Archives, list: Option<DataRef>) -> Vec<Vec<Envelope>> {
    let mut out = Vec::new();
    let Some(mut slot) = list else { return out };
    while let Some(mut edesc) = data.ptr(slot) {
        let mut env = Vec::new();
        while data.ptr(edesc).is_some() {
            env.push(Envelope {
                jobj: None,
                weight: copy(data.f32(edesc.at(4))),
            });
            edesc = edesc.at(8);
        }
        out.push(env);
        slot = slot.at(4);
    }
    out
}

/// Frees a shape, dropping its references to joints, `HSD_PObjRemove` (`PObjRelease`).
#[doc(alias = "PObjRelease")]
pub fn remove(objs: &mut Objects, p: Option<PObjId>) {
    let Some(p) = p else { return };
    let u = std::mem::replace(&mut objs.pobjs[p].u, Binding::Skin(None));
    match u {
        Binding::ShapeSet(set) => super::jobj::remove_aobj(objs, set.aobj),
        Binding::Envelopes(envs) => {
            for e in envs.iter().flatten() {
                jobj::unref_this(objs, e.jobj);
            }
        }
        Binding::Skin(j) => jobj::unref_this(objs, j),
    }
    objs.pobjs.pool.free(p.0);
}

/// `HSD_PObjRemoveAll`.
#[doc(alias = "HSD_PObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, p: Option<PObjId>) {
    let mut cur = p;
    while let Some(p) = cur {
        cur = objs.pobjs[p].next;
        remove(objs, Some(p));
    }
}

/// Binds the shape's envelopes or skin to the joints its descriptor names, now loaded,
/// `HSD_PObjResolveRefs`.
pub fn resolve_refs(objs: &mut Objects, data: &Archives, p: Option<PObjId>, desc: Option<DataRef>) {
    let (Some(p), Some(desc)) = (p, desc) else {
        return;
    };
    match objs.pobjs[p].ty() {
        ENVELOPE => {
            // resolveEnvelope
            let Some(mut slot) = data.ptr(desc.at(0x14)) else {
                return;
            };
            let count = match &objs.pobjs[p].u {
                Binding::Envelopes(e) => e.len(),
                _ => 0,
            };
            for i in 0..count {
                let Some(mut edesc) = data.ptr(slot) else {
                    break;
                };
                let mut k = 0;
                loop {
                    let len = match &objs.pobjs[p].u {
                        Binding::Envelopes(e) => e[i].len(),
                        _ => 0,
                    };
                    let Some(joint) = data.ptr(edesc) else { break };
                    if k >= len {
                        break;
                    }
                    let old = env_at(objs, p, i, k).jobj;
                    jobj::unref_this(objs, old);
                    let target = objs.ids.get(joint);
                    assert!(target.is_some(), "env->jobj");
                    env_at(objs, p, i, k).jobj = target;
                    jobj::ref_this(&mut objs.jobjs, target);
                    edesc = edesc.at(8);
                    k += 1;
                }
                slot = slot.at(4);
            }
        }
        SKIN => {
            if let Binding::Skin(old) = objs.pobjs[p].u {
                jobj::unref_this(objs, old);
            }
            objs.pobjs[p].u = Binding::Skin(None);
            if let Some(joint) = data.ptr(desc.at(0x14)) {
                let target = objs.ids.get(joint);
                assert!(target.is_some(), "pobj->u.jobj");
                objs.pobjs[p].u = Binding::Skin(target);
                jobj::ref_this(&mut objs.jobjs, target);
            }
        }
        _ => {}
    }
}

fn env_at(objs: &mut Objects, p: PObjId, i: usize, k: usize) -> &mut Envelope {
    match &mut objs.pobjs[p].u {
        Binding::Envelopes(e) => &mut e[i][k],
        _ => unreachable!(),
    }
}

/// `HSD_PObjResolveRefsAll`.
#[doc(alias = "HSD_PObjResolveRefsAll")]
pub fn resolve_refs_all(
    objs: &mut Objects,
    data: &Archives,
    p: Option<PObjId>,
    desc: Option<DataRef>,
) {
    let (mut p, mut d) = (p, desc);
    while let (Some(po), Some(de)) = (p, d) {
        resolve_refs(objs, data, Some(po), Some(de));
        p = objs.pobjs[po].next;
        d = data.ptr(de.at(4));
    }
}
