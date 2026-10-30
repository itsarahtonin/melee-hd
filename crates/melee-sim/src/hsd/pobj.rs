//! Shapes, HAL's PObj (`sysdolphin/baselib/pobj.c`): a display object's geometry. The vertex
//! data and display lists stay in the archive; the state holds what play changes or refers
//! to: the joints a skinned or enveloped shape is bound to, and a morphing shape's blend.

use std::ops::{Index, IndexMut};

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj;
use super::fobj::Value;
use super::jobj::{self, JObjId};
use super::pool::Pool;
use crate::fpu::copy;

/// A PObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct PObjId(pub u32);

/// The shape type, in the flags' bits 12 and 13 (`pobj_type`).
pub const TYPE_MASK: u16 = 0x3000;
/// Bound rigidly to one joint (or none).
pub const SKIN: u16 = 0;
/// Morphing between shapes.
pub const SHAPEANIM: u16 = 1 << 12;
/// Weighted between joints.
pub const ENVELOPE: u16 = 2 << 12;

/// A morphing shape's blend interpolates between two neighbouring shapes
/// (`SHAPESET_AVERAGE`, which `drawShapeAnim` tests).
pub const SHAPESET_AVERAGE: u16 = 1 << 0;
/// A morphing shape's blend weights adds the shapes rather than interpolating two.
pub const SHAPESET_ADDITIVE: u16 = 1 << 1;

/// `GX_VA_NBT`.
const GX_VA_NBT: u32 = 25;
/// `GX_INDEX16`, `GX_F32` and the integer component types.
const GX_INDEX16: u32 = 3;
const GX_U8: u32 = 0;
const GX_S8: u32 = 1;
const GX_U16: u32 = 2;
const GX_S16: u32 = 3;
const GX_F32: u32 = 4;

/// A morphing shape's blended vertices and normals as `drawShapeAnim` leaves them in its
/// buffers (`vertex_buffer`, `normal_buffer`): its vertices' positions, and its normals (three
/// vectors each for a normal, binormal and tangent set, `GX_VA_NBT`). An averaging set
/// interpolates between the shape its blend falls in and the next; an additive one adds each
/// shape after the first by its weight. The float steps follow the machine code (ssbm-rs
/// transliterates `drawShapeAnim`): each component's difference, then a fused multiply-add.
// Ports `get_shape_normal_xyz` and `get_shape_nbt_xyz` (in `get`); aliased once a check
// reaches a morphing shape with normals.
#[doc(alias = "drawShapeAnim")]
#[doc(alias = "get_shape_vertex_xyz")]
pub fn blend_shape(data: &Archives, set: &ShapeSet) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    use crate::fpu::{mem, reg};
    use gekko_fp as fp;
    let nbt = set.normal_desc.is_some_and(|d| data.u32(d) == GX_VA_NBT);
    let per = if nbt { 9 } else { 3 };
    let mut vertices = vec![[0.0f32; 3]; set.nb_vertex_index.max(0) as usize];
    let normals_n = if set.normal_desc.is_some() {
        set.nb_normal_index.max(0) as usize
    } else {
        0
    };
    let mut normals = vec![[0.0f32; 3]; normals_n * per / 3];
    let last = i32::from(set.nb_shape) - 1;
    // `get_shape_vertex_xyz`, `get_shape_normal_xyz` and `get_shape_nbt_xyz`: shape `shape`'s
    // `n` components of entry `i`, decoded as single floats in registers.
    let get = |desc: DataRef, list: DataRef, shape: i32, i: i32, n: usize| -> [f64; 9] {
        let index = data
            .ptr(list.at(4 * shape as u32))
            .expect("a shape's index array");
        let idx = if data.u32(desc.at(4)) == GX_INDEX16 {
            (u32::from(data.u8(index.at(2 * i as u32))) << 8)
                + u32::from(data.u8(index.at(2 * i as u32 + 1)))
        } else {
            u32::from(data.u8(index.at(i as u32)))
        };
        let base = data
            .ptr(desc.at(0x14))
            .expect("a shape's vertex array")
            .at(idx * u32::from(data.u16(desc.at(0x12))));
        let mut out = [0.0; 9];
        let ty = data.u32(desc.at(0xC));
        let scale = fp::frsp(f64::from(1i32 << data.u8(desc.at(0x10))));
        for (k, o) in out.iter_mut().take(n).enumerate() {
            let k = k as u32;
            *o = match ty {
                GX_F32 => fp::lfs(data.u32(base.at(4 * k))),
                GX_U8 => fp::fdivs(fp::frsp(f64::from(data.u8(base.at(k)))), scale),
                GX_S8 => fp::fdivs(fp::frsp(f64::from(data.u8(base.at(k)) as i8)), scale),
                GX_U16 => fp::fdivs(fp::frsp(f64::from(data.u16(base.at(2 * k)))), scale),
                GX_S16 => fp::fdivs(fp::frsp(f64::from(data.u16(base.at(2 * k)) as i16)), scale),
                t => panic!("a shape's components of type {t} (HSD_Panic: unexpected type)"),
            };
        }
        out
    };
    let flat = |v: &[[f32; 3]], at: usize, k: usize| -> f64 { reg(v[at + k / 3][k % 3]) };
    let store = |v: &mut Vec<[f32; 3]>, at: usize, k: usize, x: f64| {
        v[at + k / 3][k % 3] = mem(x);
    };
    if set.flags & SHAPESET_AVERAGE != 0 {
        assert!(
            set.flags & SHAPESET_ADDITIVE == 0,
            "an averaging shape set whose blend is additive"
        );
        let bl = reg(set.blend[0]);
        let shape = fp::fctiwz(bl).max(0).min(last);
        let d = fp::fsubs(bl, f64::from(shape));
        // `pobj_min(MAX(0.0, d), 1.0f)`, the maximum taken again when it's below 1.
        let up = if 0.0 > d { 0.0 } else { d };
        let blend = fp::frsp(if up < 1.0 { up } else { 1.0 });
        let next = (shape + 1).min(last);
        let mix =
            |out: &mut Vec<[f32; 3]>, desc: DataRef, list: DataRef, count: usize, n: usize| {
                for i in 0..count {
                    let s0 = get(desc, list, shape, i as i32, n);
                    let s1 = get(desc, list, next, i as i32, n);
                    for k in 0..n {
                        let v = fp::fmadds(blend, fp::fsubs(s1[k], s0[k]), s0[k]);
                        store(out, i * n / 3, k, v);
                    }
                }
            };
        if let (Some(desc), Some(list)) = (set.vertex_desc, set.vertex_idx_list) {
            mix(
                &mut vertices,
                desc,
                list,
                set.nb_vertex_index.max(0) as usize,
                3,
            );
        }
        if let (Some(desc), Some(list)) = (set.normal_desc, set.normal_idx_list) {
            mix(&mut normals, desc, list, normals_n, per);
        }
    } else {
        assert!(
            set.flags & SHAPESET_ADDITIVE != 0 && set.blend.len() == usize::from(set.nb_shape),
            "an additive blend without its weights"
        );
        let add =
            |out: &mut Vec<[f32; 3]>, desc: DataRef, list: DataRef, count: usize, n: usize| {
                for i in 0..count {
                    let at = i * n / 3;
                    let s = get(desc, list, 0, i as i32, n);
                    for (k, &x) in s.iter().enumerate().take(n) {
                        store(out, at, k, x);
                    }
                    for j in 0..i32::from(set.nb_shape) {
                        let w = reg(set.blend[j as usize]);
                        let b = fp::frsp(if 0.0 > w { 0.0 } else { w });
                        let s = get(desc, list, j + 1, i as i32, n);
                        for (k, &x) in s.iter().enumerate().take(n) {
                            let v = fp::fmadds(x, b, flat(out, at, k));
                            store(out, at, k, v);
                        }
                    }
                }
            };
        if let (Some(desc), Some(list)) = (set.vertex_desc, set.vertex_idx_list) {
            add(
                &mut vertices,
                desc,
                list,
                set.nb_vertex_index.max(0) as usize,
                3,
            );
        }
        if let (Some(desc), Some(list)) = (set.normal_desc, set.normal_idx_list) {
            add(&mut normals, desc, list, normals_n, per);
        }
    }
    (vertices, normals)
}

/// One joint's weight in an envelope (`HSD_Envelope`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Envelope {
    pub jobj: Option<JObjId>,
    pub weight: f32,
    /// Where the original's allocators put it, and the list node holding its chain
    /// (`docs/engine.md`).
    pub addr: u32,
    pub node: u32,
}

/// `HSD_ShapeSet`: the shapes a morphing shape blends, in the archive, and its blend state.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
    /// Where the original's allocators put it and its weights (`docs/engine.md`).
    pub addr: u32,
    pub blend_addr: u32,
}

/// What a shape is bound to, the original's union `u`, by its type.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum Binding {
    Skin(Option<JObjId>),
    ShapeSet(ShapeSet),
    /// The envelopes, one per matrix the display lists index: each a weighted list of joints
    /// (`HSD_SList`s of `HSD_Envelope` chains in the original).
    Envelopes(Vec<Vec<Envelope>>),
}

/// `HSD_PObj`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct PObj {
    pub next: Option<PObjId>,
    pub verts: Option<DataRef>,
    pub flags: u16,
    pub n_display: u16,
    pub display: Option<DataRef>,
    pub u: Binding,
    /// Its class: HAL's, or the fighters' `ftPObj`.
    pub class: super::heap::Class,
}

impl PObj {
    pub fn ty(&self) -> u16 {
        self.flags & TYPE_MASK
    }
}

/// The PObj pool.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct PObjs {
    pub pool: Pool<PObj>,
    /// The class new shapes load as (`HSD_PObjSetDefaultClass`).
    pub default_class: PObjClass,
    /// Display lists `lbRefract_PObjLoad` has rewritten in the loaded data: each vertex's
    /// position-matrix index copied over its last texture-matrix index. By display list: the
    /// vertex stride and the two indexes' offsets in a vertex, for the display pass to apply.
    pub refract_patches: std::collections::HashMap<DataRef, (u8, u8, u8)>,
    /// Envelope lists freed shapes leave, emptied, for later shapes' to reuse: no state, so a
    /// restore keeps the target's and a snapshot starts without.
    #[clone_from(keep)]
    pub spare_lists: Vec<Vec<Vec<Envelope>>>,
    #[clone_from(keep)]
    pub spare_sets: Vec<Vec<Envelope>>,
    /// `vertex_buffer` and `normal_buffer`: the blocks of the scene's heap the first shape
    /// animation drawn in a scene takes for its blended vertices and normals (2,000 of each,
    /// `drawShapeAnim`), 0 until then; the heap's amnesia forgets them (`PObjAmnesia`).
    pub vertex_buffer: u32,
    pub normal_buffer: u32,
}

/// Which of `spares` to reuse for `n` items: the smallest that holds them, else the largest.
fn best_fit<T>(spares: &[Vec<T>], n: usize) -> Option<usize> {
    let fits = (0..spares.len())
        .filter(|&i| spares[i].capacity() >= n)
        .min_by_key(|&i| spares[i].capacity());
    fits.or_else(|| (0..spares.len()).max_by_key(|&i| spares[i].capacity()))
}

/// How many spare envelope lists and sets a [`PObjs`] keeps at most.
const SPARES: usize = 256;

impl PObjs {
    /// An emptied envelope list for `n` sets: the smallest spare that holds them, else the
    /// largest (grown), else a new one.
    fn take_list(&mut self, n: usize) -> Vec<Vec<Envelope>> {
        match best_fit(&self.spare_lists, n) {
            Some(i) => self.spare_lists.swap_remove(i),
            None => Vec::with_capacity(n),
        }
    }

    /// An empty envelope set for `n` envelopes, likewise.
    fn take_set(&mut self, n: usize) -> Vec<Envelope> {
        match best_fit(&self.spare_sets, n) {
            Some(i) => self.spare_sets.swap_remove(i),
            None => Vec::with_capacity(n),
        }
    }

    /// Keeps a freed shape's envelope lists for reuse, up to [`SPARES`] of each.
    fn give_back(&mut self, mut list: Vec<Vec<Envelope>>) {
        for mut set in list.drain(..) {
            if self.spare_sets.len() < SPARES {
                set.clear();
                if self.spare_sets.capacity() == 0 {
                    self.spare_sets.reserve_exact(SPARES);
                }
                self.spare_sets.push(set);
            }
        }
        if self.spare_lists.len() < SPARES {
            if self.spare_lists.capacity() == 0 {
                self.spare_lists.reserve_exact(SPARES);
            }
            self.spare_lists.push(list);
        }
    }
}

/// A PObj class: HAL's, or one the game derives from it whose loader differs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum PObjClass {
    #[default]
    Hsd,
    /// Fighters' `ftPObj` (`ftParts_PObjInfoInit`), loaded by `lbRefract_PObjLoad`.
    Fighter,
}

/// Sets the class new shapes load as, `HSD_PObjSetDefaultClass` (`None`, HAL's).
#[doc(alias = "HSD_PObjSetDefaultClass")]
pub fn set_default_class(pobjs: &mut PObjs, class: PObjClass) {
    pobjs.default_class = class;
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

/// The shape-animation flag of `HSD_*ByFlags` (`POBJ_ANIM`).
pub const POBJ_ANIM: u32 = 1 << 3;
/// The first blend-weight channel (`HSD_A_S_W0`).
const A_W0: u8 = 2;

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
    let class = match pobjs.default_class {
        PObjClass::Hsd => super::heap::Class::PObj,
        PObjClass::Fighter => super::heap::Class::FtPObj,
    };
    let blank = || PObj {
        next: None,
        verts: None,
        flags: 0,
        n_display: 0,
        display: None,
        u: Binding::Skin(None),
        class,
    };
    let p = PObjId(pobjs.pool.alloc(blank()));
    pobjs.pool.set_addr(p.0, super::heap::class_alloc(class));
    let next = load_desc(pobjs, data, data.ptr(desc.at(4)));
    let flags = data.u16(desc.at(0xC));
    let at = desc.at(0x14);
    let u = match flags & TYPE_MASK {
        SHAPEANIM => Binding::ShapeSet(load_shape_set(data, data.ptr(at).expect("shape set"))),
        ENVELOPE => Binding::Envelopes(load_envelopes(pobjs, data, data.ptr(at))),
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
        class,
    };
    if pobjs.default_class == PObjClass::Fighter
        && let Some(patch) = refract_load(&pobjs[p], data)
    {
        let display = pobjs[p].display.expect("a shape without a display list");
        pobjs.refract_patches.insert(display, patch);
        if super::heap::image_on() {
            let list = refract_list(data, display, pobjs[p].n_display, patch);
            super::heap::with(|h| h.image.file_bytes(display, &list));
        }
    }
    Some(p)
}

/// The display list `lbRefract_PObjLoad` leaves in the loaded file, for the heap's image where
/// the scene loaded the file into its heap (a debug VS match's fighters): each vertex's last
/// texture-matrix index a copy of its position-matrix index, through every primitive until
/// the list's end or a command without one (`(cmd & 0xF8) == 0`).
fn refract_list(data: &Archives, display: DataRef, n_display: u16, patch: (u8, u8, u8)) -> Vec<u8> {
    let (stride, pnmtx, last) = (patch.0 as usize, patch.1 as usize, patch.2 as usize);
    let total = usize::from(n_display) << 5;
    let a = data.get(display.archive).data();
    let mut list = a[display.offset as usize..][..total].to_vec();
    let mut at = 0;
    while at < total {
        let cmd = list[at];
        at += 1;
        if cmd & 0xF8 == 0 {
            break;
        }
        let count = usize::from(u16::from_be_bytes([list[at], list[at + 1]]));
        at += 2;
        for _ in 0..count {
            list[at + last] = list[at + pnmtx];
            at += stride;
        }
    }
    list
}

/// `lbRefract_PObjLoad`'s part after HAL's loader: for a shape flagged 0x2000 with a
/// position-matrix index and a texture-matrix index past the first two attributes, it copies
/// each vertex's position-matrix index over its last texture-matrix index in the loaded
/// display list. That rewrites game data only the display pass reads; the rewrite comes back
/// as the vertex stride and the two offsets, for [`PObjs::refract_patches`].
#[doc(alias = "lbRefract_PObjLoad")]
fn refract_load(p: &PObj, data: &Archives) -> Option<(u8, u8, u8)> {
    if p.flags & 0x2000 == 0 {
        return None;
    }
    let (mut stride, mut pnmtx, mut last) = (0i32, -1i32, -1i32);
    let mut v = p.verts;
    while let Some(at) = v {
        let attr = data.u32(at);
        // GX_VA_NULL.
        if attr == 0xFF {
            break;
        }
        let ty = data.u32(at.at(4));
        match attr {
            1..=8 => {
                last = stride;
                stride += 1;
            }
            0 => {
                pnmtx = stride;
                stride += 1;
            }
            9 | 10 | 13..=20 => stride += if ty == 3 { 2 } else { 1 },
            11 | 12 => match ty {
                3 => stride += 2,
                2 => stride += 1,
                _ => match data.u32(at.at(0xC)) {
                    0 | 3 => stride += 2,
                    1 | 4 => stride += 3,
                    2 => stride += 4,
                    _ => {}
                },
            },
            _ => {}
        }
        v = Some(at.at(0x18));
    }
    if pnmtx == -1 || last < 2 {
        return None;
    }
    Some((stride as u8, pnmtx as u8, last as u8))
}

/// The static `loadShapeSetDesc`.
fn load_shape_set(data: &Archives, d: DataRef) -> ShapeSet {
    let flags = data.u16(d);
    let nb_shape = data.u16(d.at(2));
    let addr = super::heap::piece_alloc(0x24);
    let blend_addr = if flags & SHAPESET_ADDITIVE != 0 {
        super::heap::mem_alloc(u32::from(nb_shape) * 4)
    } else {
        0
    };
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
        addr,
        blend_addr,
    }
}

/// The envelopes' weights, joints unresolved, the static `loadEnvelopeDesc`, in freed shapes'
/// lists where there are spares.
fn load_envelopes(pobjs: &mut PObjs, data: &Archives, list: Option<DataRef>) -> Vec<Vec<Envelope>> {
    let Some(mut slot) = list else {
        return pobjs.take_list(0);
    };
    let mut sets = 0;
    let mut s = slot;
    while data.ptr(s).is_some() {
        sets += 1;
        s = s.at(4);
    }
    let mut out = pobjs.take_list(sets);
    while let Some(edesc) = data.ptr(slot) {
        let mut n = 0;
        let mut e = edesc;
        while data.ptr(e).is_some() {
            n += 1;
            e = e.at(8);
        }
        assert!(n != 0, "an envelope with no joints");
        let mut env = pobjs.take_set(n);
        let mut e = edesc;
        while data.ptr(e).is_some() {
            env.push(Envelope {
                jobj: None,
                weight: copy(data.f32(e.at(4))),
                addr: super::heap::piece_alloc(0xC),
                node: 0,
            });
            e = e.at(8);
        }
        // `HSD_SListAlloc`: the node that holds the chain, after its envelopes.
        let node = super::heap::obj_alloc(super::heap::Kind::SList);
        for e in &mut env {
            e.node = node;
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
    super::export::with_image(|img| super::export::pobj(objs, img, p));
    let u = std::mem::replace(&mut objs.pobjs[p].u, Binding::Skin(None));
    match u {
        Binding::ShapeSet(set) => {
            if set.flags & SHAPESET_ADDITIVE != 0 {
                super::heap::mem_free(set.blend_addr);
            }
            super::jobj::remove_aobj(objs, set.aobj);
            super::heap::piece_free(set.addr, 0x24);
        }
        Binding::Envelopes(envs) => {
            // `HSD_EnvelopeListFree`: each chain's envelopes, then its node.
            for set in &envs {
                for e in set {
                    jobj::unref_this(objs, e.jobj);
                    super::heap::piece_free(e.addr, 0xC);
                }
                if let Some(e) = set.first() {
                    super::heap::obj_free(super::heap::Kind::SList, e.node);
                }
            }
            objs.pobjs.give_back(envs);
        }
        Binding::Skin(j) => jobj::unref_this(objs, j),
    }
    let class = objs.pobjs[p].class;
    super::heap::class_free(class, objs.pobjs.pool.addr(p.0));
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

/// A morphing shape's blend state, if the shape morphs.
pub fn shape_set(pobjs: &mut PObjs, p: PObjId) -> Option<&mut ShapeSet> {
    let ty = pobjs[p].ty();
    match &mut pobjs[p].u {
        Binding::ShapeSet(set) if ty == SHAPEANIM => Some(set),
        _ => None,
    }
}

/// Frees each morphing shape's animation of a list if `flags` has [`POBJ_ANIM`],
/// `HSD_PObjRemoveAnimAllByFlags` (each through the inline `HSD_PObjRemoveAnimByFlags`).
#[doc(alias = "HSD_PObjRemoveAnimAllByFlags")]
pub fn remove_anim_all_by_flags(objs: &mut Objects, p: Option<PObjId>, flags: u32) {
    let mut cur = p;
    while let Some(p) = cur {
        if flags & POBJ_ANIM != 0
            && let Some(set) = shape_set(&mut objs.pobjs, p)
        {
            let a = set.aobj.take();
            jobj::remove_aobj(objs, a);
        }
        cur = objs.pobjs[p].next;
    }
}

/// Gives each shape of a list its morph animation from a chain of `HSD_ShapeAnim`s, in step,
/// `HSD_PObjAddAnimAll` (each through the inline `HSD_PObjAddAnim`). Every shape of the list
/// must morph (the original asserts), even past the chain's end.
#[doc(alias = "HSD_PObjAddAnimAll")]
pub fn add_anim_all(
    objs: &mut Objects,
    data: &Archives,
    p: Option<PObjId>,
    shapeanim: Option<DataRef>,
) {
    if shapeanim.is_none() {
        return;
    }
    let (mut cur, mut sa) = (p, shapeanim);
    while let Some(p) = cur {
        assert!(
            shape_set(&mut objs.pobjs, p).is_some(),
            "pobj_type(pobj) == POBJ_SHAPEANIM && pobj->u.shape_set"
        );
        if let Some(anim) = sa {
            if let Some(old) = shape_set(&mut objs.pobjs, p).and_then(|s| s.aobj) {
                jobj::remove_aobj(objs, Some(old));
            }
            let a = jobj::load_aobj(objs, data, data.ptr(anim.at(4)));
            if let Some(set) = shape_set(&mut objs.pobjs, p) {
                set.aobj = a;
            }
        }
        cur = objs.pobjs[p].next;
        sa = sa.and_then(|s| data.ptr(s));
    }
}

/// Restarts each morphing shape's animation of a list at `frame` if `flags` has
/// [`POBJ_ANIM`], `HSD_PObjReqAnimAllByFlags` (each through the inline
/// `HSD_PObjReqAnimByFlags`).
#[doc(alias = "HSD_PObjReqAnimAllByFlags")]
pub fn req_anim_all_by_flags(objs: &mut Objects, p: Option<PObjId>, frame: f32, flags: u32) {
    let mut cur = p;
    while let Some(p) = cur {
        if flags & POBJ_ANIM != 0
            && let Some(a) = shape_set(&mut objs.pobjs, p).map(|s| s.aobj)
        {
            aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
        }
        cur = objs.pobjs[p].next;
    }
}

/// Hands an animated blend value to a morphing shape, `PObjUpdateFunc` (through the inline
/// `ShapeSetSetAnimResult`): an additive set's weight for the channel's shape, or the morph's
/// position. A stale value (`Value::Stale`) is stored as [`Value::or_address`] and counted in
/// [`Objects::stale_reads`].
#[doc(alias = "PObjUpdateFunc")]
pub fn update(objs: &mut Objects, p: PObjId, ty: u8, value: Value) {
    let Some(set) = shape_set(&mut objs.pobjs, p) else {
        return;
    };
    let v = copy(value.or_address());
    if set.flags & SHAPESET_ADDITIVE != 0 {
        // The original writes past the weights for a channel out of their range.
        let i = usize::from(ty.wrapping_sub(A_W0));
        assert!(
            i < set.blend.len(),
            "shape weight {i} of {}",
            set.blend.len()
        );
        set.blend[i] = v;
    } else {
        set.blend[0] = v;
    }
    objs.stale_reads += u32::from(value == Value::Stale);
}

/// Advances each morphing shape's animation of a list one frame, `HSD_PObjAnimAll` (each
/// through the inline `HSD_PObjAnim`).
#[doc(alias = "HSD_PObjAnimAll")]
pub fn anim_all(objs: &mut Objects, data: &Archives, p: Option<PObjId>) {
    let mut cur = p;
    while let Some(p) = cur {
        if let Some(a) = shape_set(&mut objs.pobjs, p).map(|s| s.aobj) {
            let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, p, ty, v);
            aobj::interpret(objs, data, a, Some(&mut up));
        }
        cur = objs.pobjs[p].next;
    }
}
