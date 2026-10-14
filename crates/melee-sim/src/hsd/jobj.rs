//! Joints, HAL's JObj (`sysdolphin/baselib/jobj.c`): the trees models are built from. A joint
//! has a scale, rotation and translation, a world matrix computed from them lazily (when
//! marked dirty), animation, references to other joints (RObj: constraints, limits,
//! expressions, inverse kinematics), and its display objects, particles or spline.

use std::collections::HashSet;
use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::dobj::{self, DObjId};
use super::fobj::{self, Value};
use super::heap;
use super::particle;
use super::pool::Pool;
use super::robj::{self, RObjId, inv_length};
use super::spline;
use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::mtx::Mtx34;
use crate::quat::Quat;
use crate::vec::Vec3;

/// A JObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JObjId(pub u32);

pub const SKELETON: u32 = 1 << 0;
pub const SKELETON_ROOT: u32 = 1 << 1;
pub const ENVELOPE_MODEL: u32 = 1 << 2;
/// Scale isn't inherited through the matrix but carried in `scl`.
pub const CLASSICAL_SCALE: u32 = 1 << 3;
pub const HIDDEN: u32 = 1 << 4;
/// The contents are a particle list.
pub const PTCL: u32 = 1 << 5;
pub const MTX_DIRTY: u32 = 1 << 6;
pub const LIGHTING: u32 = 1 << 7;
pub const TEXGEN: u32 = 1 << 8;
/// The child is another tree, shared, rather than this joint's own.
pub const INSTANCE: u32 = 1 << 12;
/// The contents are a spline.
pub const SPLINE: u32 = 1 << 14;
pub const FLIP_IK: u32 = 1 << 15;
pub const SPECULAR: u32 = 1 << 16;
/// The rotation is a quaternion rather than Euler angles.
pub const USE_QUATERNION: u32 = 1 << 17;
/// What inverse kinematics makes of the joint.
pub const JOINT_MASK: u32 = 3 << 21;
pub const JOINT1: u32 = 1 << 21;
pub const JOINT2: u32 = 2 << 21;
pub const EFFECTOR: u32 = 3 << 21;
/// The game sets the matrix itself.
pub const USER_DEF_MTX: u32 = 1 << 23;
pub const MTX_INDEP_PARENT: u32 = 1 << 24;
pub const MTX_INDEP_SRT: u32 = 1 << 25;
pub const ROOT_OPA: u32 = 1 << 28;
pub const ROOT_XLU: u32 = 1 << 29;
pub const ROOT_TEXEDGE: u32 = 1 << 30;
pub const ROOT_MASK: u32 = ROOT_OPA | ROOT_XLU | ROOT_TEXEDGE;

/// Animation channel types (`HSD_A_J_*`).
pub mod channel {
    pub const ROTX: u8 = 1;
    pub const ROTY: u8 = 2;
    pub const ROTZ: u8 = 3;
    pub const PATH: u8 = 4;
    pub const TRAX: u8 = 5;
    pub const TRAY: u8 = 6;
    pub const TRAZ: u8 = 7;
    pub const SCAX: u8 = 8;
    pub const SCAY: u8 = 9;
    pub const SCAZ: u8 = 10;
    pub const NODE: u8 = 11;
    pub const BRANCH: u8 = 12;
    pub const SETBYTE0: u8 = 20;
    pub const SETFLOAT9: u8 = 39;
    pub const DPTCL: u8 = 0x28;
    pub const JSOUND: u8 = 0x29;
    pub const PTCLTGT: u8 = 0x2A;
}

/// What a joint holds, the original's union `u`, as its flags say at load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contents {
    DObjs(Option<DObjId>),
    /// A particle list in the joint's archive.
    Particles(Option<DataRef>),
    Spline(Option<DataRef>),
}

impl Default for Contents {
    fn default() -> Self {
        Contents::DObjs(None)
    }
}

/// `HSD_JObj`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct JObj {
    /// `HSD_Obj`'s reference counts: by owners (`u16::MAX` for none) and by individual
    /// references (constraints, instances, the current joint).
    pub ref_count: u16,
    pub ref_count_individual: u16,
    pub next: Option<JObjId>,
    pub parent: Option<JObjId>,
    pub child: Option<JObjId>,
    pub flags: u32,
    pub u: Contents,
    pub rotate: Quat,
    /// Whether `rotate.w` is a stack word the original never set (`lb_8001044C`'s rotations;
    /// see engine.md): the comparer leaves it out, and a quaternion read of it counts a stale
    /// read.
    pub stale_w: bool,
    pub scale: Vec3,
    pub translate: Vec3,
    pub mtx: Mtx34,
    /// The accumulated scale (`HSD_VecAlloc`ed in the original), for classical scaling.
    pub scl: Option<Vec3>,
    /// The bind pose's inverse for envelope skinning (`HSD_MtxAlloc`ed).
    pub envelopemtx: Option<Mtx34>,
    pub aobj: Option<AObjId>,
    pub robj: Option<RObjId>,
    /// The joint descriptor it was loaded from, its ID table key.
    pub id: Option<DataRef>,
    /// Its class (`hsdJObj`, or the fighters' `ftJObj` and `ftIntpJObj`), and where the
    /// original's allocators put its `scl` and `envelopemtx` (`docs/engine.md`).
    pub class: heap::Class,
    pub scl_addr: u32,
    pub envelopemtx_addr: u32,
}

impl Default for JObj {
    /// A new joint as `JObjInit` leaves it: dirty, unit scale.
    fn default() -> Self {
        JObj {
            ref_count: 0,
            ref_count_individual: 0,
            next: None,
            parent: None,
            child: None,
            flags: MTX_DIRTY,
            u: Contents::default(),
            rotate: Quat::default(),
            stale_w: false,
            scale: Vec3::new(1.0, 1.0, 1.0),
            translate: Vec3::default(),
            mtx: Mtx34::default(),
            scl: None,
            envelopemtx: None,
            aobj: None,
            robj: None,
            id: None,
            class: heap::Class::JObj,
            scl_addr: 0,
            envelopemtx_addr: 0,
        }
    }
}

impl JObj {
    /// The display objects, if it holds them (`union_type_dobj`).
    pub fn dobj(&self) -> Option<DObjId> {
        match self.u {
            Contents::DObjs(d) if self.flags & (PTCL | SPLINE) == 0 => d,
            _ => None,
        }
    }

    /// `HSD_JObjMtxIsDirty`.
    pub fn mtx_is_dirty(&self) -> bool {
        self.flags & USER_DEF_MTX == 0 && self.flags & MTX_DIRTY != 0
    }
}

/// The JObj pool.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct JObjs {
    pub pool: Pool<JObj>,
}

impl Index<JObjId> for JObjs {
    type Output = JObj;
    fn index(&self, j: JObjId) -> &JObj {
        &self.pool[j.0]
    }
}

impl IndexMut<JObjId> for JObjs {
    fn index_mut(&mut self, j: JObjId) -> &mut JObj {
        &mut self.pool[j.0]
    }
}

/// jobj.c's globals.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Globals {
    /// `current_jobj`.
    pub current: Option<JObjId>,
    /// Whether the game has set `dptcl_callback` (`HSD_JObjSetDPtclCallback`) and
    /// `sptcl_callback` (`HSD_JObjSetSPtclCallback`): efLib does, and they go to the game's
    /// [`particle::Hooks::joint_particles`].
    pub dptcl: bool,
    pub sptcl: bool,
    /// Particle-list entries whose particles are still to spawn: the top bit `JObjLoad` sets
    /// in the loaded data, which the first display clears.
    pub pending_ptcl: HashSet<DataRef>,
    /// The class new joints load as (`HSD_JObjSetDefaultClass`).
    pub default_class: JObjClass,
}

/// A JObj class whose loader differs from HAL's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JObjClass {
    #[default]
    Hsd,
    /// Fighters' `ftIntpJObj` (`ftParts_IntpJObjInfoInit`): the blend skeleton's joints,
    /// loaded without their display objects (`ftParts_IntpJObjLoad`).
    FighterIntp,
}

/// Sets the class new joints load as, `HSD_JObjSetDefaultClass` (HAL's for `Hsd`).
#[doc(alias = "HSD_JObjSetDefaultClass")]
pub fn set_default_class(objs: &mut Objects, class: JObjClass) {
    objs.jobj.default_class = class;
}

/// The joint's children, first to last.
fn children(jobjs: &JObjs, j: JObjId) -> impl Iterator<Item = JObjId> + '_ {
    std::iter::successors(jobjs[j].child, move |&c| jobjs[c].next)
}

/// The joint's first own child: none for an instance, whose child is another tree. Walks
/// over a joint's own children read each next sibling after visiting a child, as the
/// original's loops do; the visits change no tree links.
fn first_own_child(jobjs: &JObjs, j: JObjId) -> Option<JObjId> {
    if jobjs[j].flags & INSTANCE != 0 {
        None
    } else {
        jobjs[j].child
    }
}

/// Marks the joint's matrix dirty if what it depends on is, `HSD_JObjCheckDepend`.
#[doc(alias = "HSD_JObjCheckDepend")]
pub fn check_depend(jobjs: &mut JObjs, j: Option<JObjId>) {
    let Some(j) = j else { return };
    let o = &jobjs[j];
    if o.mtx_is_dirty() {
        return;
    }
    let parent = o.parent.map(|p| &jobjs[p]);
    let dirty = if o.flags & USER_DEF_MTX != 0 {
        o.flags & MTX_INDEP_PARENT == 0 && parent.is_some_and(JObj::mtx_is_dirty)
    } else {
        parent.is_some_and(|p| p.flags & MTX_DIRTY != 0)
            || o.flags & JOINT_MASK != 0
            || o.robj.is_some()
    };
    if dirty {
        jobjs[j].flags |= MTX_DIRTY;
    }
}

/// Resets the joint's SRT to its descriptor's, `JObjResetRST`.
#[doc(alias = "JObjResetRST")]
fn reset_rst_one(jobjs: &mut JObjs, data: &Archives, j: JObjId, joint: DataRef) {
    let o = &mut jobjs[j];
    o.rotate.x = copy(data.f32(joint.at(0x14)));
    o.rotate.y = copy(data.f32(joint.at(0x18)));
    o.rotate.z = copy(data.f32(joint.at(0x1C)));
    o.scale = data_vec(data, joint.at(0x20));
    o.translate = data_vec(data, joint.at(0x2C));
    if o.flags & MTX_INDEP_SRT == 0 {
        set_mtx_dirty(jobjs, Some(j));
    }
}

/// A `Vec3` in game data, copied as a structure.
fn data_vec(data: &Archives, at: DataRef) -> Vec3 {
    Vec3::new(data.f32(at), data.f32(at.at(4)), data.f32(at.at(8)))
}

/// Resets a tree's SRT to its descriptors', `HSD_JObjResetRST`.
#[doc(alias = "HSD_JObjResetRST")]
pub fn reset_rst(jobjs: &mut JObjs, data: &Archives, j: Option<JObjId>, joint: Option<DataRef>) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    reset_rst_one(jobjs, data, j, joint);
    if jobjs[j].flags & INSTANCE == 0 {
        let mut child_joint = data.ptr(joint.at(8));
        let mut cur = jobjs[j].child;
        while let Some(c) = cur {
            reset_rst(jobjs, data, Some(c), child_joint);
            cur = jobjs[c].next;
            child_joint = child_joint.and_then(|cj| data.ptr(cj.at(0xC)));
        }
    }
}

/// Visits a tree in pre-order with each joint's relation to its parent (0 for the root, 1
/// for a first child, 2 for a later one), `HSD_JObjWalkTree` and `HSD_JObjWalkTree0`.
#[doc(alias = "HSD_JObjWalkTree")]
#[doc(alias = "HSD_JObjWalkTree0")]
pub fn walk_tree(jobjs: &JObjs, j: Option<JObjId>, visit: &mut dyn FnMut(JObjId, u32)) {
    fn walk(jobjs: &JObjs, j: JObjId, ty: u32, visit: &mut dyn FnMut(JObjId, u32)) {
        visit(j, ty);
        if jobjs[j].flags & INSTANCE == 0 {
            let mut cur = jobjs[j].child;
            while let Some(c) = cur {
                let parent = jobjs[c].parent.expect("HSD_JObjWalkTree0: no parent");
                let ty = if jobjs[parent].child == Some(c) { 1 } else { 2 };
                walk(jobjs, c, ty, visit);
                cur = jobjs[c].next;
            }
        }
    }
    if let Some(j) = j {
        walk(jobjs, j, 0, visit);
    }
}

/// Computes the joint's world matrix from its SRT and its parent's matrix,
/// `HSD_JObjMakeMatrix`.
#[doc(alias = "HSD_JObjMakeMatrix")]
pub fn make_matrix(objs: &mut Objects, data: &Archives, j: JObjId) {
    let parent = objs.jobjs[j].parent;
    setup_matrix(objs, data, parent);
    let jobjs = &mut objs.jobjs;
    let parent_scl = parent.and_then(|p| jobjs[p].scl);
    let o = &mut jobjs[j];
    // The original keeps `scl` in a block of its own, made when first needed and freed when
    // a classically scaled joint's parent has none.
    if o.scl.is_none() && (parent_scl.is_some() || o.flags & CLASSICAL_SCALE == 0) {
        o.scl_addr = heap::obj_alloc(heap::Kind::Vec);
    } else if o.scl.is_some() && parent_scl.is_none() && o.flags & CLASSICAL_SCALE != 0 {
        heap::obj_free(heap::Kind::Vec, o.scl_addr);
    }
    if o.flags & CLASSICAL_SCALE != 0 {
        o.scl = parent_scl;
    } else {
        o.scl = Some(match parent_scl {
            Some(p) => Vec3::new(
                mem(fp::fmuls(reg(o.scale.x), reg(p.x))),
                mem(fp::fmuls(reg(o.scale.y), reg(p.y))),
                mem(fp::fmuls(reg(o.scale.z), reg(p.z))),
            ),
            None => o.scale,
        });
    }
    o.mtx = if o.flags & USE_QUATERNION != 0 {
        Mtx34::srt_quat(o.scale, o.rotate, o.translate, parent_scl)
    } else {
        let euler = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
        Mtx34::srt(o.scale, euler, o.translate, parent_scl)
    };
    if let Some(p) = parent {
        let m = jobjs[p].mtx.concat(&jobjs[j].mtx);
        jobjs[j].mtx = m;
    }
    let bound = jobjs[j].aobj.and_then(|a| objs.aobjs[a].hsd_obj);
    if let Some(b) = bound {
        setup_matrix(objs, data, Some(b));
        let v = objs.jobjs[b].mtx.transform(objs.jobjs[j].translate);
        let m = &mut objs.jobjs[j].mtx.m;
        m[0][3] = v.x;
        m[1][3] = v.y;
        m[2][3] = v.z;
    }
}

/// Removes the joint's animations the flags name (1: its own, the rest: its display
/// objects' and references'), `HSD_JObjRemoveAnimByFlags`.
#[doc(alias = "HSD_JObjRemoveAnimByFlags")]
pub fn remove_anim_by_flags(objs: &mut Objects, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    if flags & 1 != 0 {
        remove_aobj(objs, objs.jobjs[j].aobj);
        objs.jobjs[j].aobj = None;
    }
    dobj::remove_anim_all_by_flags(objs, objs.jobjs[j].dobj(), flags);
    robj::remove_anim_all_by_flags(objs, objs.jobjs[j].robj, flags);
}

/// `HSD_JObjRemoveAnimAllByFlags`.
#[doc(alias = "HSD_JObjRemoveAnimAllByFlags")]
pub fn remove_anim_all_by_flags(objs: &mut Objects, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    remove_anim_by_flags(objs, Some(j), flags);
    let mut c = first_own_child(&objs.jobjs, j);
    while let Some(x) = c {
        remove_anim_all_by_flags(objs, Some(x), flags);
        c = objs.jobjs[x].next;
    }
}

/// `HSD_JObjRemoveAnim`.
#[doc(alias = "HSD_JObjRemoveAnim")]
pub fn remove_anim(objs: &mut Objects, j: Option<JObjId>) {
    remove_anim_by_flags(objs, j, 0x7FF);
}

/// `HSD_JObjRemoveAnimAll`.
#[doc(alias = "HSD_JObjRemoveAnimAll")]
pub fn remove_anim_all(objs: &mut Objects, j: Option<JObjId>) {
    remove_anim_all_by_flags(objs, j, 0x7FF);
}

/// Requests the joint's animations the flags name from `frame`, `HSD_JObjReqAnimByFlags`.
#[doc(alias = "HSD_JObjReqAnimByFlags")]
pub fn req_anim_by_flags(objs: &mut Objects, j: Option<JObjId>, flags: u32, frame: f32) {
    let Some(j) = j else { return };
    if flags & 1 != 0 {
        let a = objs.jobjs[j].aobj;
        aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
    }
    dobj::req_anim_all_by_flags(objs, objs.jobjs[j].dobj(), frame, flags);
    robj::req_anim_all_by_flags(objs, objs.jobjs[j].robj, frame, flags);
}

/// `HSD_JObjReqAnimAllByFlags`.
#[doc(alias = "HSD_JObjReqAnimAllByFlags")]
pub fn req_anim_all_by_flags(objs: &mut Objects, j: Option<JObjId>, flags: u32, frame: f32) {
    let Some(j) = j else { return };
    req_anim_by_flags(objs, Some(j), flags, frame);
    let mut c = first_own_child(&objs.jobjs, j);
    while let Some(x) = c {
        req_anim_all_by_flags(objs, Some(x), flags, frame);
        c = objs.jobjs[x].next;
    }
}

/// `HSD_JObjReqAnimAll`.
#[doc(alias = "HSD_JObjReqAnimAll")]
pub fn req_anim_all(objs: &mut Objects, j: Option<JObjId>, frame: f32) {
    req_anim_all_by_flags(objs, j, 0x7FF, frame);
}

/// `HSD_JObjReqAnim`.
#[doc(alias = "HSD_JObjReqAnim")]
pub fn req_anim(objs: &mut Objects, j: Option<JObjId>, frame: f32) {
    req_anim_by_flags(objs, j, 0x7FF, frame);
}

/// Object types for [`foreach_anim`]'s mask (`HSD_TypeMask`).
pub mod type_mask {
    pub const DOBJ: u32 = 1 << 2;
    pub const JOBJ: u32 = 1 << 5;
    pub const MOBJ: u32 = 1 << 7;
    pub const POBJ: u32 = 1 << 8;
    pub const ROBJ: u32 = 1 << 9;
    pub const TOBJ: u32 = 1 << 10;
}

/// Hands `f` every AObj of a joint tree whose object type `mask` names: each joint's, its
/// display objects', their materials' and textures', their first shape's, and its references',
/// `HSD_ForeachAnim` on a joint (`JObjForeachAnim`).
#[doc(alias = "HSD_ForeachAnim")]
#[doc(alias = "JObjForeachAnim")]
pub fn foreach_anim(
    objs: &mut Objects,
    j: JObjId,
    mask: u32,
    f: &mut dyn FnMut(&mut Objects, AObjId),
) {
    use type_mask::*;
    if mask & JOBJ != 0
        && let Some(a) = objs.jobjs[j].aobj
    {
        f(objs, a);
    }
    let mut d = objs.jobjs[j].dobj();
    while let Some(id) = d {
        if mask & DOBJ != 0
            && let Some(a) = objs.dobjs[id].aobj
        {
            f(objs, a);
        }
        if let Some(m) = objs.dobjs[id].mobj {
            if mask & MOBJ != 0
                && let Some(a) = objs.mobjs[m].aobj
            {
                f(objs, a);
            }
            let mut t = objs.mobjs[m].tobj;
            while let Some(tid) = t {
                if mask & TOBJ != 0
                    && let Some(a) = objs.tobjs[tid].aobj
                {
                    f(objs, a);
                }
                t = objs.tobjs[tid].next;
            }
        }
        if mask & POBJ != 0
            && let Some(p) = objs.dobjs[id].pobj
            && objs.pobjs[p].ty() == super::pobj::SHAPEANIM
            && let super::pobj::Binding::ShapeSet(set) = &objs.pobjs[p].u
            && let Some(a) = set.aobj
        {
            f(objs, a);
        }
        d = objs.dobjs[id].next;
    }
    let mut r = objs.jobjs[j].robj;
    while let Some(id) = r {
        if mask & ROBJ != 0
            && let Some(a) = objs.robjs[id].aobj
        {
            f(objs, a);
        }
        r = objs.robjs[id].next;
    }
    let mut c = first_own_child(&objs.jobjs, j);
    while let Some(x) = c {
        foreach_anim(objs, x, mask, f);
        c = objs.jobjs[x].next;
    }
}

/// Moves the AObj's first branch-visibility channel to the front, so it runs first,
/// `JObjSortAnim`.
#[doc(alias = "JObjSortAnim")]
pub fn sort_anim(objs: &mut Objects, a: Option<AObjId>) {
    let Some(a) = a else { return };
    let Some(first) = objs.aobjs[a].fobj else {
        return;
    };
    let fobjs = &mut objs.fobjs;
    let mut prev: Option<fobj::FObjId> = None;
    let mut cur = Some(first);
    while let Some(f) = cur {
        if fobjs[f.0].obj_type == channel::BRANCH {
            let next = fobjs[f.0].next;
            match prev {
                Some(p) => fobjs[p.0].next = next,
                None => objs.aobjs[a].fobj = next,
            }
            fobjs[f.0].next = objs.aobjs[a].fobj;
            objs.aobjs[a].fobj = Some(f);
            return;
        }
        prev = cur;
        cur = fobjs[f.0].next;
    }
}

/// Gives the joint the animation of its AnimJoint, and its display objects theirs from its
/// MatAnimJoint and ShapeAnimJoint, `HSD_JObjAddAnim`.
#[doc(alias = "HSD_JObjAddAnim")]
pub fn add_anim(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    anim_joint: Option<DataRef>,
    mat_joint: Option<DataRef>,
    shape_joint: Option<DataRef>,
) {
    let Some(j) = j else { return };
    if let Some(aj) = anim_joint {
        if let Some(a) = objs.jobjs[j].aobj {
            remove_aobj(objs, Some(a));
        }
        let a = load_aobj(objs, data, data.ptr(aj.at(8)));
        objs.jobjs[j].aobj = a;
        sort_anim(objs, a);
        robj::add_anim_all(objs, data, objs.jobjs[j].robj, data.ptr(aj.at(0xC)));
        if data.u32(aj.at(0x10)) & 1 != 0 {
            set_flags(&mut objs.jobjs, Some(j), CLASSICAL_SCALE);
        } else {
            clear_flags(&mut objs.jobjs, Some(j), CLASSICAL_SCALE);
        }
    }
    if let Some(d) = objs.jobjs[j].dobj() {
        let matanim = mat_joint.and_then(|m| data.ptr(m.at(8)));
        let shapeanim = shape_joint.and_then(|s| data.ptr(s.at(8)));
        dobj::add_anim_all(objs, data, Some(d), matanim, shapeanim);
    }
}

/// Gives a tree the animations of its AnimJoint, MatAnimJoint and ShapeAnimJoint trees,
/// `HSD_JObjAddAnimAll`.
#[doc(alias = "HSD_JObjAddAnimAll")]
pub fn add_anim_all(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    anim_joint: Option<DataRef>,
    mat_joint: Option<DataRef>,
    shape_joint: Option<DataRef>,
) {
    let Some(j) = j else { return };
    add_anim(objs, data, Some(j), anim_joint, mat_joint, shape_joint);
    if objs.jobjs[j].flags & INSTANCE != 0 {
        return;
    }
    // AnimJoint and MatAnimJoint: child, next; ShapeAnimJoint too.
    let child = |d: Option<DataRef>| d.and_then(|d| data.ptr(d));
    let next = |d: Option<DataRef>| d.and_then(|d| data.ptr(d.at(4)));
    let (mut aj, mut mj, mut sj) = (child(anim_joint), child(mat_joint), child(shape_joint));
    let mut cur = objs.jobjs[j].child;
    while let Some(c) = cur {
        add_anim_all(objs, data, Some(c), aj, mj, sj);
        cur = objs.jobjs[c].next;
        (aj, mj, sj) = (next(aj), next(mj), next(sj));
    }
}

/// Sets a component of the joint's SRT, marking its matrix dirty unless the SRT is
/// independent (the inline `HSD_JObjSetTranslateX` and friends).
pub fn set_srt(jobjs: &mut JObjs, j: JObjId, set: impl FnOnce(&mut JObj)) {
    let o = &mut jobjs[j];
    set(o);
    if o.flags & MTX_INDEP_SRT == 0 {
        set_mtx_dirty(jobjs, Some(j));
    }
}

/// Sets a rotation angle; the original asserts the joint has no quaternion.
pub fn set_angle(jobjs: &mut JObjs, j: JObjId, set: impl FnOnce(&mut Quat)) {
    assert!(
        jobjs[j].flags & USE_QUATERNION == 0,
        "!(jobj->flags & JOBJ_USE_QUATERNION)"
    );
    set_srt(jobjs, j, |o| set(&mut o.rotate));
}

/// `HSD_JObjSetTranslate` (inline): a bit copy, then dirty unless the SRT is independent.
pub fn set_translate(jobjs: &mut JObjs, j: JObjId, t: Vec3) {
    set_srt(jobjs, j, |o| o.translate = t);
}

/// `HSD_JObjSetScale` (inline).
pub fn set_scale(jobjs: &mut JObjs, j: JObjId, s: Vec3) {
    set_srt(jobjs, j, |o| o.scale = s);
}

/// `HSD_JObjSetRotation` (inline): the whole quaternion, any flags.
pub fn set_rotation(jobjs: &mut JObjs, j: JObjId, q: Quat) {
    set_srt(jobjs, j, |o| {
        o.rotate = q;
        o.stale_w = false;
    });
}

/// `HSD_JObjSetRotation` from a quaternion local whose w the caller never set: the Euler
/// angles, the old w kept and marked stale.
pub fn set_rotation_stale_w(jobjs: &mut JObjs, j: JObjId, r: Vec3) {
    set_srt(jobjs, j, |o| {
        (o.rotate.x, o.rotate.y, o.rotate.z) = (r.x, r.y, r.z);
        o.stale_w = true;
    });
}

/// `HSD_JObjSetRotationX` (inline).
pub fn set_rotation_x(jobjs: &mut JObjs, j: JObjId, v: f32) {
    set_angle(jobjs, j, |q| q.x = v);
}

/// `HSD_JObjSetRotationY` (inline).
pub fn set_rotation_y(jobjs: &mut JObjs, j: JObjId, v: f32) {
    set_angle(jobjs, j, |q| q.y = v);
}

/// `HSD_JObjSetRotationZ` (inline).
pub fn set_rotation_z(jobjs: &mut JObjs, j: JObjId, v: f32) {
    set_angle(jobjs, j, |q| q.z = v);
}

/// Hands a joint one animated or constrained value, `JObjUpdateFunc`: its SRT, visibility,
/// path position, particle events, or matrix columns.
#[doc(alias = "JObjUpdateFunc")]
pub fn update(objs: &mut Objects, data: &Archives, j: JObjId, ty: u8, value: Value) {
    update_with(objs, data, None, j, ty, value);
}

/// [`update`] from an animation, whose particle events reach the game's hooks.
pub fn update_with(
    objs: &mut Objects,
    data: &Archives,
    hooks: Option<&mut dyn particle::Hooks>,
    j: JObjId,
    ty: u8,
    value: Value,
) {
    use channel::*;
    let stale = matches!(value, Value::Stale);
    let fv = || value.or_address();
    let jobjs = &mut objs.jobjs;
    match ty {
        PATH => {
            // Any address is negative, so a stale value is 0 here.
            let mut t = fv();
            if f64::from(t) < 0.0 {
                t = 0.0;
            }
            if 1.0 < f64::from(t) {
                t = 1.0;
            }
            let a = jobjs[j].aobj.expect("jobj->aobj");
            let path = objs.aobjs[a].hsd_obj.expect("jp");
            let Contents::Spline(Some(s)) = objs.jobjs[path].u else {
                panic!("jp->u.spline")
            };
            let Some(p) = spline::arc_length_point(data, s, t) else {
                // The original moves the joint to whatever its stack held.
                objs.stale_reads += 1;
                return;
            };
            let jobjs = &mut objs.jobjs;
            set_srt(jobjs, j, |o| o.translate.x = copy(p.x));
            set_srt(jobjs, j, |o| o.translate.y = copy(p.y));
            set_srt(jobjs, j, |o| o.translate.z = copy(p.z));
        }
        ROTX => {
            if stale {
                objs.stale_reads += 1;
            }
            let v = copy(fv());
            if jobjs[j].flags & JOINT1 != 0
                && let Some(r) = robj::get_by_type(&objs.robjs, jobjs[j].robj, robj::IK_HINT, 0)
                && let robj::Ref::IkHint { bone_length, .. } = objs.robjs[r].u
            {
                objs.robjs[r].u = robj::Ref::IkHint {
                    bone_length,
                    rotate_x: v,
                };
            }
            set_angle(&mut objs.jobjs, j, |q| q.x = v);
        }
        ROTY | ROTZ | TRAX | TRAY | TRAZ => {
            if stale {
                objs.stale_reads += 1;
            }
            let v = copy(fv());
            match ty {
                ROTY => set_angle(jobjs, j, |q| q.y = v),
                ROTZ => set_angle(jobjs, j, |q| q.z = v),
                TRAX => set_srt(jobjs, j, |o| o.translate.x = v),
                TRAY => set_srt(jobjs, j, |o| o.translate.y = v),
                _ => set_srt(jobjs, j, |o| o.translate.z = v),
            }
        }
        SCAX | SCAY | SCAZ => {
            // Any address is smaller than 1e-3, so a stale value becomes 1e-3 here.
            let mut v = copy(fv());
            if reg(f32::from_bits(v.to_bits() & 0x7FFF_FFFF)) < reg(1e-3) {
                v = 1e-3;
            }
            match ty {
                SCAX => set_srt(jobjs, j, |o| o.scale.x = v),
                SCAY => set_srt(jobjs, j, |o| o.scale.y = v),
                _ => set_srt(jobjs, j, |o| o.scale.z = v),
            }
        }
        // Any address is below 0.5, so a stale value hides here.
        BRANCH | NODE => {
            let show = f64::from(fv()) > 0.5;
            match (ty, show) {
                (BRANCH, true) => clear_flags_all(jobjs, Some(j), HIDDEN),
                (BRANCH, false) => set_flags_all(jobjs, Some(j), HIDDEN),
                (_, true) => clear_flags(jobjs, Some(j), HIDDEN),
                (_, false) => set_flags(jobjs, Some(j), HIDDEN),
            }
        }
        // `ufc_callbacks` is never added to, so these reach no one.
        SETBYTE0..=SETFLOAT9 => {}
        DPTCL => {
            if objs.jobj.dptcl {
                // The callback gets the word the value is: the bank in its low six bits, the
                // index above. A stale value's word is a RAM address, which names no particle
                // the original could have meant.
                let Value::Float(v) = value else {
                    objs.stale_reads += 1;
                    return;
                };
                let w = v.to_bits();
                let hooks =
                    hooks.expect("a constraint's particle event (only animations have them)");
                hooks.joint_particles(
                    objs,
                    data,
                    true,
                    (w & 0x3F) as i32,
                    (w >> 6 & 0xFF_FFFF) as i32,
                    j,
                );
            }
        }
        // No setter exists for these callbacks, so they stay null.
        JSOUND | PTCLTGT => {}
        0x32..=0x35 => {
            let Value::Point(p) = value else {
                objs.stale_reads += 1;
                return;
            };
            let col = (ty - 0x32) as usize;
            let m = &mut jobjs[j].mtx.m;
            m[0][col] = copy(p.x);
            m[1][col] = copy(p.y);
            m[2][col] = copy(p.z);
        }
        0x36..=0x39 => {
            let o = &jobjs[j];
            let m = match o.parent {
                Some(p) => jobjs[p].mtx.hsd_inverse_concat(&o.mtx),
                None => o.mtx.copied(),
            };
            let o = &mut jobjs[j];
            if ty == 0x36 || ty == 0x38 {
                o.translate = m.translation_part();
            }
            if ty == 0x36 || ty == 0x37 {
                let r = m.rotation_angles();
                (o.rotate.x, o.rotate.y, o.rotate.z) = (r.x, r.y, r.z);
            }
            if ty == 0x36 || ty == 0x39 {
                o.scale = m.scale_part();
            }
        }
        _ => {}
    }
}

/// Animates the joint: its own channels, its references and its display objects,
/// `HSD_JObjAnim`.
#[doc(alias = "HSD_JObjAnim")]
pub fn anim(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn particle::Hooks,
    j: Option<JObjId>,
) {
    let Some(j) = j else { return };
    check_depend(&mut objs.jobjs, Some(j));
    let a = objs.jobjs[j].aobj;
    let mut up =
        |objs: &mut Objects, ty: u8, v: Value| update_with(objs, data, Some(&mut *hooks), j, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
    robj::anim_all(objs, data, objs.jobjs[j].robj);
    dobj::anim_all(objs, data, objs.jobjs[j].dobj());
}

/// `JObjAnimAll`.
#[doc(alias = "JObjAnimAll")]
fn anim_tree(objs: &mut Objects, data: &Archives, hooks: &mut dyn particle::Hooks, j: JObjId) {
    anim(objs, data, hooks, Some(j));
    if objs.jobjs[j].flags & INSTANCE == 0 {
        let mut cur = objs.jobjs[j].child;
        while let Some(c) = cur {
            anim_tree(objs, data, hooks, c);
            cur = objs.jobjs[c].next;
        }
    }
}

/// Animates a tree, `HSD_JObjAnimAll`.
#[doc(alias = "HSD_JObjAnimAll")]
pub fn anim_all(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn particle::Hooks,
    j: Option<JObjId>,
) {
    let Some(j) = j else { return };
    aobj::init_end_callback(&mut objs.aobjs);
    anim_tree(objs, data, hooks, j);
    aobj::invoke_end_callbacks(&objs.aobjs);
}

/// A joint and its siblings and descendants from their descriptors: `JObjLoadJointSub`,
/// inlined into `JObjLoad`.
fn load_sub(
    objs: &mut Objects,
    data: &Archives,
    joint: Option<DataRef>,
    parent: Option<JObjId>,
) -> Option<JObjId> {
    let joint = joint?;
    assert!(
        data.ptr(joint).is_none(),
        "joint {joint:?} names a class, which isn't supported"
    );
    let j = alloc(objs);
    load(objs, data, j, joint, parent);
    Some(j)
}

/// Fills a new joint from its descriptor and loads its children and later siblings,
/// `JObjLoad`; for fighters' blend skeletons (`ftParts_IntpJObjLoad`), with the descriptor's
/// contents taken as none.
#[doc(alias = "JObjLoad")]
#[doc(alias = "ftParts_IntpJObjLoad")]
fn load(objs: &mut Objects, data: &Archives, j: JObjId, joint: DataRef, parent: Option<JObjId>) {
    load_as(objs, data, j, joint, parent, joint, true);
}

/// `JObjLoad` from a copy of `joint` whose child and next are null when `links` is false,
/// filed in the ID table under `key` (the copy's address).
fn load_as(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    joint: DataRef,
    parent: Option<JObjId>,
    key: DataRef,
    links: bool,
) {
    let flags = data.u32(joint.at(4));
    if flags & INSTANCE == 0 && links {
        let child = load_sub(objs, data, data.ptr(joint.at(8)), Some(j));
        objs.jobjs[j].child = child;
    }
    let next = if links {
        load_sub(objs, data, data.ptr(joint.at(0xC)), parent)
    } else {
        None
    };
    let o = &mut objs.jobjs[j];
    o.next = next;
    o.parent = parent;
    o.flags |= flags;
    let u = match objs.jobj.default_class {
        JObjClass::Hsd => data.ptr(joint.at(0x10)),
        JObjClass::FighterIntp => None,
    };
    if o.flags & SPLINE != 0 {
        o.u = Contents::Spline(u);
    } else if o.flags & PTCL != 0 {
        o.u = Contents::Particles(u);
        let mut entry = u;
        while let Some(e) = entry {
            objs.jobj.pending_ptcl.insert(e);
            entry = data.ptr(e);
        }
    } else {
        let d = dobj::load_desc(objs, data, u);
        objs.jobjs[j].u = Contents::DObjs(d);
    }
    let r = robj::load_desc(objs, data, data.ptr(joint.at(0x3C)));
    let o = &mut objs.jobjs[j];
    o.robj = r;
    o.rotate.x = copy(data.f32(joint.at(0x14)));
    o.rotate.y = copy(data.f32(joint.at(0x18)));
    o.rotate.z = copy(data.f32(joint.at(0x1C)));
    o.scale = data_vec(data, joint.at(0x20));
    o.translate = data_vec(data, joint.at(0x2C));
    o.mtx = Mtx34::IDENTITY;
    o.scl = None;
    if let Some(m) = data.ptr(joint.at(0x38)) {
        let mut e = Mtx34::default();
        for (i, v) in e.m.iter_mut().flatten().enumerate() {
            *v = data.f32(m.at(4 * i as u32));
        }
        o.envelopemtx = Some(e);
        o.envelopemtx_addr = heap::obj_alloc(heap::Kind::Mtx);
    }
    objs.ids.insert(key, j);
    objs.jobjs[j].id = Some(key);
}

/// Loads a joint alone from a copy of its descriptor the caller made on its stack with its
/// child and next cleared (`HSD_JObjLoadJoint` on the copy), the ID table keying it by the
/// copy's address `key` ([`super::id::stack_key`]).
pub fn load_joint_copy(
    objs: &mut Objects,
    data: &Archives,
    joint: DataRef,
    key: DataRef,
) -> JObjId {
    assert!(
        data.ptr(joint).is_none(),
        "joint {joint:?} names a class, which isn't supported"
    );
    let j = alloc(objs);
    load_as(objs, data, j, joint, None, key, false);
    if objs.jobjs[j].flags & INSTANCE == 0 {
        resolve_refs(objs, data, Some(j), Some(joint));
    } else {
        panic!("an instance joint loaded from a copy without its child");
    }
    j
}

/// Loads a joint tree and resolves its references, `HSD_JObjLoadJoint`.
#[doc(alias = "HSD_JObjLoadJoint")]
pub fn load_joint(objs: &mut Objects, data: &Archives, joint: Option<DataRef>) -> Option<JObjId> {
    let j = load_sub(objs, data, joint, None);
    resolve_refs_all(objs, data, j, joint);
    j
}

/// Points the joint's references and instance at the joints their descriptors name,
/// `HSD_JObjResolveRefs`.
#[doc(alias = "HSD_JObjResolveRefs")]
pub fn resolve_refs(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    robj::resolve_refs_all(objs, data, objs.jobjs[j].robj, data.ptr(joint.at(0x3C)));
    if objs.jobjs[j].flags & INSTANCE != 0 {
        unref(objs, objs.jobjs[j].child);
        let target = data.ptr(joint.at(8)).and_then(|c| objs.ids.get(c));
        assert!(target.is_some(), "jobj->child");
        objs.jobjs[j].child = target;
        ref_(&mut objs.jobjs, target);
    }
    if let Some(d) = objs.jobjs[j].dobj() {
        dobj::resolve_refs_all(objs, data, Some(d), data.ptr(joint.at(0x10)));
    }
}

/// `HSD_JObjResolveRefsAll`.
#[doc(alias = "HSD_JObjResolveRefsAll")]
pub fn resolve_refs_all(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (mut j, mut joint) = (j, joint);
    while let (Some(jo), Some(jt)) = (j, joint) {
        resolve_refs(objs, data, Some(jo), Some(jt));
        if objs.jobjs[jo].flags & INSTANCE == 0 {
            resolve_refs_all(objs, data, objs.jobjs[jo].child, data.ptr(jt.at(8)));
        }
        j = objs.jobjs[jo].next;
        joint = data.ptr(jt.at(0xC));
    }
}

/// Takes an owner's reference, `HSD_JObjRef` (`ref_INC`).
pub fn ref_(jobjs: &mut JObjs, j: Option<JObjId>) {
    if let Some(j) = j {
        let o = &mut jobjs[j];
        o.ref_count = o.ref_count.wrapping_add(1);
        assert!(
            o.ref_count != u16::MAX,
            "HSD_OBJ(o)->ref_count != HSD_OBJ_NOREF"
        );
    }
}

/// Takes an individual reference, `HSD_JObjRefThis` (`iref_INC`).
pub fn ref_this(jobjs: &mut JObjs, j: Option<JObjId>) {
    if let Some(j) = j {
        let o = &mut jobjs[j];
        o.ref_count_individual = o.ref_count_individual.wrapping_add(1);
        assert!(
            o.ref_count_individual != 0,
            "HSD_OBJ(o)->ref_count_individual != 0"
        );
    }
}

/// `ref_DEC`: whether that was the last owner's reference.
fn ref_dec(o: &mut JObj) -> bool {
    if o.ref_count == u16::MAX {
        return true;
    }
    let was = o.ref_count;
    o.ref_count = was.wrapping_sub(1);
    was == 0
}

/// `iref_DEC`: whether no individual references remain.
fn iref_dec(o: &mut JObj) -> bool {
    if o.ref_count_individual == 0 {
        return true;
    }
    o.ref_count_individual -= 1;
    o.ref_count_individual == 0
}

/// Drops an owner's reference, releasing the joint's tree and freeing it once nothing refers
/// to it, `HSD_JObjUnref`.
#[doc(alias = "HSD_JObjUnref")]
pub fn unref(objs: &mut Objects, j: Option<JObjId>) {
    let Some(j) = j else { return };
    if !ref_dec(&mut objs.jobjs[j]) {
        return;
    }
    if objs.jobjs[j].ref_count_individual == 0 {
        delete(objs, j);
    } else {
        ref_this(&mut objs.jobjs, Some(j));
        release_child(objs, j);
        if iref_dec(&mut objs.jobjs[j]) {
            delete(objs, j);
        }
    }
}

/// Drops an individual reference, freeing the joint if it was the last of all,
/// `HSD_JObjUnrefThis`.
#[doc(alias = "HSD_JObjUnrefThis")]
pub fn unref_this(objs: &mut Objects, j: Option<JObjId>) {
    let Some(j) = j else { return };
    if iref_dec(&mut objs.jobjs[j]) && objs.jobjs[j].ref_count == u16::MAX {
        delete(objs, j);
    }
}

/// Releases and frees a joint, `hsdDelete` on a JObj (`JObjRelease`).
#[doc(alias = "JObjRelease")]
fn delete(objs: &mut Objects, j: JObjId) {
    release_child(objs, j);
    let id = objs.jobjs[j].id;
    if let Some(id) = id
        && objs.ids.get(id) == Some(j)
    {
        objs.ids.remove(id);
    }
    let o = &objs.jobjs[j];
    if o.scl.is_some() {
        heap::obj_free(heap::Kind::Vec, o.scl_addr);
    }
    if o.envelopemtx.is_some() {
        heap::obj_free(heap::Kind::Mtx, o.envelopemtx_addr);
    }
    heap::class_free(o.class, objs.jobjs.pool.addr(j.0));
    objs.jobjs.pool.free(j.0);
}

/// Unlinks a joint, making its only child take its place, and drops it, `HSD_JObjRemove`.
/// Returns the child.
#[doc(alias = "HSD_JObjRemove")]
pub fn remove(objs: &mut Objects, j: Option<JObjId>) -> Option<JObjId> {
    let j = j?;
    let jobjs = &mut objs.jobjs;
    let child = jobjs[j].child;
    if let Some(c) = child {
        assert!(jobjs[c].next.is_none(), "child->next == NULL");
    }
    let next = child.or(jobjs[j].next);
    let parent = jobjs[j].parent;
    match get_prev(jobjs, Some(j)) {
        Some(prev) => jobjs[prev].next = next,
        None => {
            if let Some(p) = parent {
                jobjs[p].child = next;
            }
        }
    }
    if let Some(n) = next
        && Some(n) == child
    {
        jobjs[n].next = jobjs[j].next;
        jobjs[n].parent = parent;
    }
    let o = &mut jobjs[j];
    o.parent = None;
    o.child = None;
    o.next = None;
    unref(objs, Some(j));
    child
}

/// Unlinks a joint and its later siblings from their parent and drops them,
/// `HSD_JObjRemoveAll`.
#[doc(alias = "HSD_JObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, j: Option<JObjId>) {
    let Some(j) = j else { return };
    if let Some(p) = objs.jobjs[j].parent {
        match get_prev(&objs.jobjs, Some(j)) {
            Some(prev) => objs.jobjs[prev].next = None,
            None => objs.jobjs[p].child = None,
        }
    }
    let mut cur = Some(j);
    while let Some(c) = cur {
        cur = objs.jobjs[c].next;
        let o = &mut objs.jobjs[c];
        o.parent = None;
        o.next = None;
        unref(objs, Some(c));
    }
}

/// Drops the root bits no child still supplies, up the parent chain, `RecalcParentTrspBits`.
#[doc(alias = "RecalcParentTrspBits")]
pub fn recalc_parent_trsp_bits(jobjs: &mut JObjs, j: Option<JObjId>) {
    let mut cur = j;
    while let Some(c) = cur {
        let mut flags = !ROOT_MASK;
        for ch in children(jobjs, c) {
            let f = jobjs[ch].flags;
            flags |= (f | f << 10) & ROOT_MASK;
        }
        if jobjs[c].flags & !flags == 0 {
            break;
        }
        jobjs[c].flags &= flags;
        cur = jobjs[c].next;
    }
}

/// Adds a child's root bits up the parent chain, the static `UpdateParentTrspBits`.
fn update_parent_trsp_bits(jobjs: &mut JObjs, j: Option<JObjId>, child: JObjId) {
    let f = jobjs[child].flags;
    let flags = (f | f << 10) & ROOT_MASK;
    let mut cur = j;
    while let Some(c) = cur {
        if flags & !jobjs[c].flags == 0 {
            break;
        }
        jobjs[c].flags |= flags;
        cur = jobjs[c].parent;
    }
}

/// Appends an orphan as the joint's last child, `HSD_JObjAddChild`.
#[doc(alias = "HSD_JObjAddChild")]
pub fn add_child(jobjs: &mut JObjs, j: Option<JObjId>, child: Option<JObjId>) {
    let (Some(j), Some(child)) = (j, child) else {
        return;
    };
    assert!(jobjs[child].parent.is_none(), "child should be a orphan.");
    assert!(
        jobjs[child].next.is_none(),
        "child should not have siblings"
    );
    match jobjs[j].child {
        None => jobjs[j].child = Some(child),
        Some(first) => {
            assert!(
                jobjs[j].flags & INSTANCE == 0,
                "!(jobj->flags & JOBJ_INSTANCE)"
            );
            let mut last = first;
            while let Some(n) = jobjs[last].next {
                assert!(last != child, "last != child");
                last = n;
            }
            jobjs[last].next = Some(child);
        }
    }
    jobjs[child].parent = Some(j);
    update_parent_trsp_bits(jobjs, Some(j), child);
}

/// Moves a joint (without its later siblings) under `parent`, or out of its tree, returning
/// its old next sibling, `HSD_JObjReparent`.
#[doc(alias = "HSD_JObjReparent")]
pub fn reparent(jobjs: &mut JObjs, j: Option<JObjId>, parent: Option<JObjId>) -> Option<JObjId> {
    let j = j?;
    let next = jobjs[j].next;
    if let Some(p) = jobjs[j].parent {
        if jobjs[p].child == Some(j) {
            jobjs[p].child = next;
        } else {
            let prev = get_prev(jobjs, Some(j)).expect("prev");
            jobjs[prev].next = next;
        }
        recalc_parent_trsp_bits(jobjs, Some(p));
        jobjs[j].parent = None;
    }
    jobjs[j].next = None;
    add_child(jobjs, parent, Some(j));
    next
}

/// Puts `next` in the joint's place with the joint and its siblings as `next`'s last
/// children, `HSD_JObjAddNext`.
#[doc(alias = "HSD_JObjAddNext")]
pub fn add_next(jobjs: &mut JObjs, j: Option<JObjId>, next: Option<JObjId>) {
    let (Some(j), Some(next)) = (j, next) else {
        return;
    };
    let parent = jobjs[j].parent;
    let mut cur = match parent {
        Some(p) => {
            let first = jobjs[p].child;
            jobjs[p].child = None;
            jobjs[j].flags &= !ROOT_MASK;
            first
        }
        None => Some(j),
    };
    reparent(jobjs, Some(next), parent);
    match jobjs[next].child {
        Some(first) => {
            let mut last = first;
            while let Some(n) = jobjs[last].next {
                last = n;
            }
            jobjs[last].next = cur;
        }
        None => jobjs[next].child = cur,
    }
    while let Some(c) = cur {
        jobjs[c].parent = Some(next);
        update_parent_trsp_bits(jobjs, Some(next), c);
        cur = jobjs[c].next;
    }
}

/// The sibling before the joint, `HSD_JObjGetPrev`.
#[doc(alias = "HSD_JObjGetPrev")]
pub fn get_prev(jobjs: &JObjs, j: Option<JObjId>) -> Option<JObjId> {
    let j = j?;
    let parent = jobjs[j].parent?;
    if jobjs[parent].child == Some(j) {
        return None;
    }
    let found = children(jobjs, parent).find(|&c| jobjs[c].next == Some(j));
    assert!(
        found.is_some(),
        "can not find specified jobj. maybe jobj tree is broken."
    );
    found
}

/// The joint's display objects, `HSD_JObjGetDObj`.
#[doc(alias = "HSD_JObjGetDObj")]
pub fn get_dobj(jobjs: &JObjs, j: Option<JObjId>) -> Option<DObjId> {
    jobjs[j?].dobj()
}

/// Puts a display object first in the joint's list (`HSD_JObjAddDObj`, unchecked until its
/// callers are ported).
pub fn add_dobj(objs: &mut Objects, j: Option<JObjId>, d: Option<DObjId>) {
    let (Some(j), Some(d)) = (j, d) else { return };
    let o = &objs.jobjs[j];
    if o.flags & (PTCL | SPLINE) != 0 {
        return;
    }
    let Contents::DObjs(first) = o.u else { return };
    objs.dobjs[d].next = first;
    objs.jobjs[j].u = Contents::DObjs(Some(d));
}

/// Puts an RObj (and the list after it, replaced by the joint's) first in the joint's list,
/// `HSD_JObjPrependRObj`.
#[doc(alias = "HSD_JObjPrependRObj")]
pub fn prepend_robj(objs: &mut Objects, j: Option<JObjId>, r: Option<RObjId>) {
    let (Some(j), Some(r)) = (j, r) else { return };
    objs.robjs[r].next = objs.jobjs[j].robj;
    objs.jobjs[j].robj = Some(r);
}

/// Unlinks an RObj from the joint's list, `HSD_JObjDeleteRObj`.
#[doc(alias = "HSD_JObjDeleteRObj")]
pub fn delete_robj(objs: &mut Objects, j: Option<JObjId>, r: Option<RObjId>) {
    let (Some(j), Some(r)) = (j, r) else { return };
    let mut prev: Option<RObjId> = None;
    let mut cur = objs.jobjs[j].robj;
    while let Some(c) = cur {
        if c == r {
            let next = objs.robjs[c].next;
            match prev {
                Some(p) => objs.robjs[p].next = next,
                None => objs.jobjs[j].robj = next,
            }
            objs.robjs[r].next = None;
            return;
        }
        prev = cur;
        cur = objs.robjs[c].next;
    }
}

/// `HSD_JObjGetFlags`.
#[doc(alias = "HSD_JObjGetFlags")]
pub fn get_flags(jobjs: &JObjs, j: Option<JObjId>) -> u32 {
    j.map_or(0, |j| jobjs[j].flags)
}

/// Sets flags; changing classical scaling dirties the matrix, `HSD_JObjSetFlags`.
#[doc(alias = "HSD_JObjSetFlags")]
pub fn set_flags(jobjs: &mut JObjs, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    if (jobjs[j].flags ^ flags) & CLASSICAL_SCALE != 0 {
        set_mtx_dirty(jobjs, Some(j));
    }
    jobjs[j].flags |= flags;
}

/// `HSD_JObjSetFlagsAll`.
#[doc(alias = "HSD_JObjSetFlagsAll")]
pub fn set_flags_all(jobjs: &mut JObjs, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    set_flags(jobjs, Some(j), flags);
    let mut c = first_own_child(jobjs, j);
    while let Some(x) = c {
        set_flags_all(jobjs, Some(x), flags);
        c = jobjs[x].next;
    }
}

/// Clears flags; changing classical scaling dirties the matrix, `HSD_JObjClearFlags`.
#[doc(alias = "HSD_JObjClearFlags")]
pub fn clear_flags(jobjs: &mut JObjs, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    if (jobjs[j].flags ^ flags) & CLASSICAL_SCALE != 0 {
        set_mtx_dirty(jobjs, Some(j));
    }
    jobjs[j].flags &= !flags;
}

/// `HSD_JObjClearFlagsAll`.
#[doc(alias = "HSD_JObjClearFlagsAll")]
pub fn clear_flags_all(jobjs: &mut JObjs, j: Option<JObjId>, flags: u32) {
    let Some(j) = j else { return };
    clear_flags(jobjs, Some(j), flags);
    let mut c = first_own_child(jobjs, j);
    while let Some(x) = c {
        clear_flags_all(jobjs, Some(x), flags);
        c = jobjs[x].next;
    }
}

/// A new joint of the default class, `HSD_JObjAlloc`.
#[doc(alias = "HSD_JObjAlloc")]
pub fn alloc(objs: &mut Objects) -> JObjId {
    let class = match objs.jobj.default_class {
        JObjClass::Hsd => heap::Class::JObj,
        JObjClass::FighterIntp => heap::Class::FtIntpJObj,
    };
    let j = objs.jobjs.pool.alloc(JObj {
        class,
        ..JObj::default()
    });
    objs.jobjs.pool.set_addr(j, heap::class_alloc(class));
    JObjId(j)
}

/// Makes a joint current, referencing it, `HSD_JObjSetCurrent`.
#[doc(alias = "HSD_JObjSetCurrent")]
pub fn set_current(objs: &mut Objects, j: Option<JObjId>) {
    ref_(&mut objs.jobjs, j);
    unref(objs, objs.jobj.current);
    objs.jobj.current = j;
}

/// `HSD_JObjGetCurrent`.
#[doc(alias = "HSD_JObjGetCurrent")]
pub fn get_current(objs: &Objects) -> Option<JObjId> {
    objs.jobj.current
}

/// The first joint from `j` along its siblings with an IK role (the static
/// `jobj_get_joint2` and `jobj_get_effector`).
fn first_with_role(jobjs: &JObjs, j: Option<JObjId>, role: u32) -> Option<JObjId> {
    std::iter::successors(j, |&c| jobjs[c].next).find(|&c| jobjs[c].flags & JOINT_MASK == role)
}

/// The effector among `j` and its later siblings if it has an active joint constraint of
/// type 1, `jobj_get_effector_checked`.
fn effector_checked(objs: &Objects, j: Option<JObjId>) -> Option<JObjId> {
    let eff = first_with_role(&objs.jobjs, j, EFFECTOR);
    assert!(eff.is_some(), "eff");
    let eff = eff?;
    robj::get_by_type(&objs.robjs, objs.jobjs[eff].robj, robj::JOBJ, 1).map(|_| eff)
}

/// The IK hint of a joint's references, which inverse kinematics asserts.
fn ik_hint(objs: &Objects, j: JObjId) -> (f32, f32, u32) {
    let r = robj::get_by_type(&objs.robjs, objs.jobjs[j].robj, robj::IK_HINT, 0).expect("robj");
    let (bone_length, rotate_x) = objs.robjs[r].ik_hint();
    (bone_length, rotate_x, objs.robjs[r].flags)
}

fn col(m: &Mtx34, i: usize) -> Vec3 {
    Vec3::new(m.m[0][i], m.m[1][i], m.m[2][i])
}

/// The matrix's first three columns `a·s.x`, `b·s.y`, `c·s.z` and its translation `t`.
fn set_basis(m: &mut Mtx34, a: Vec3, b: Vec3, c: Vec3, s: Vec3, t: Vec3) {
    for (i, (v, k)) in [(a, s.x), (b, s.y), (c, s.z)].into_iter().enumerate() {
        let k = reg(k);
        m.m[0][i] = mem(fp::fmuls(reg(v.x), k));
        m.m[1][i] = mem(fp::fmuls(reg(v.y), k));
        m.m[2][i] = mem(fp::fmuls(reg(v.z), k));
    }
    m.m[0][3] = t.x;
    m.m[1][3] = t.y;
    m.m[2][3] = t.z;
}

/// Solves the first joint of a two-bone IK chain: aims it so the chain reaches its effector's
/// target, `resolveIKJoint1`.
#[doc(alias = "resolveIKJoint1")]
fn resolve_ik_joint1(objs: &mut Objects, data: &Archives, j: JObjId) {
    let mut flip = false;
    let mut len2 = 0.0f64;
    let joint2 = first_with_role(&objs.jobjs, objs.jobjs[j].child, JOINT2);
    let mut origin = Vec3::default();
    let s = objs.jobjs[j].scl.unwrap_or(Vec3::new(1.0, 1.0, 1.0));
    let (bone_length, rotate_x, _) = ik_hint(objs, j);
    let len1 = fp::fmuls(reg(bone_length), reg(s.x));
    let eff = match joint2 {
        Some(j2) => {
            let (bl2, _, flags2) = ik_hint(objs, j2);
            len2 = fp::fmuls(reg(s.x), fp::fmuls(reg(bl2), reg(objs.jobjs[j2].scale.x)));
            flip = flags2 & 4 != 0;
            effector_checked(objs, objs.jobjs[j2].child)
        }
        None => effector_checked(objs, objs.jobjs[j].child),
    };
    let Some(eff) = eff else { return };
    let r = objs.jobjs[j].robj;
    if robj::get_by_type(&objs.robjs, r, robj::JOBJ, 3).is_none() && r.is_some() {
        robj::update_all(objs, data, r, j);
        if objs.jobjs[j].mtx_is_dirty() {
            make_mtx(objs, data, j);
            objs.jobjs[j].flags &= !MTX_DIRTY;
        }
    }
    if let Some(p) = objs.jobjs[j].parent {
        origin = objs.jobjs[p].mtx.translation_part();
    }
    if let Some(t) = robj::global_position(objs, data, objs.jobjs[eff].robj, 1) {
        objs.jobjs[eff].translate = t;
    }
    let reach = objs.jobjs[eff].translate.sdk_sub(origin);
    let dist2 = reg(reach.sdk_dot(reach));
    let (along, across, a, b, side, up);
    if dist2 > reg(1e-8) {
        let axis = reach;
        let mut pole;
        let mut normal;
        if let Some(target) = robj::global_position(objs, data, objs.jobjs[j].robj, 3) {
            pole = target.sdk_sub(origin);
            if reg(0.0) != reg(rotate_x) {
                pole = Mtx34::rotation_about(axis, rotate_x).transform(pole);
            }
            normal = axis.sdk_cross(pole);
            pole = normal.sdk_cross(axis);
        } else {
            normal = col(&objs.jobjs[j].mtx, 2);
            pole = normal.sdk_cross(axis);
            normal = axis.sdk_cross(pole);
        }
        side = normal.sdk_scale(inv_length(normal));
        up = pole.sdk_scale(inv_length(pole));
        let l1 = fp::fmuls(len1, len1);
        let l2 = fp::fmuls(len2, len2);
        let d = fp::fsubs(l1, l2);
        let h = fp::fmuls(
            reg(0.25),
            fp::fsubs(
                fp::fmsubs(reg(2.0), fp::fadds(l1, l2), dist2),
                fp::fdivs(fp::fmuls(d, d), dist2),
            ),
        );
        let h = if h >= 0.0 { h } else { 0.0 };
        let t = fp::fdivs(fp::fsubs(l1, h), dist2);
        along = fp::fmuls(
            t,
            reg(mem(math::sqrt_reg(fp::fdivs(
                1.0,
                fp::fadds(reg(1e-10), t),
            )))),
        );
        across = fp::fmuls(
            h,
            reg(mem(math::sqrt_reg(fp::fdivs(
                1.0,
                fp::fadds(reg(1e-10), h),
            )))),
        );
        (a, b) = (l2, h);
    } else {
        // The original reads its pole and normal from stack slots and two registers it never
        // set on this path, where the effector sits on the chain's root.
        objs.stale_reads += 1;
        (side, up) = (Vec3::default(), Vec3::default());
        (a, b) = (0.0, 0.0);
        along = 0.0;
        across = len1;
    }
    let across = if flip { -across } else { across };
    let along = if fp::fsubs(a, b) >= dist2 {
        -along
    } else {
        along
    };
    let mut x = reach.sdk_scale(mem(along));
    x = x.sdk_add(up.sdk_scale(mem(across)));
    x = x.sdk_scale(inv_length(x));
    let y = side.sdk_cross(x);
    set_basis(&mut objs.jobjs[j].mtx, x, y, side, s, origin);
}

/// Solves the second joint of a two-bone IK chain: aims it from the first bone's end at the
/// effector, within its limits, `resolveIKJoint2`.
#[doc(alias = "resolveIKJoint2")]
fn resolve_ik_joint2(objs: &mut Objects, j: JObjId) {
    let Some(eff) = effector_checked(objs, objs.jobjs[j].child) else {
        return;
    };
    let Some(parent) = objs.jobjs[j].parent else {
        return;
    };
    let s = objs.jobjs[j].scl.unwrap_or(Vec3::new(1.0, 1.0, 1.0));
    let pm = objs.jobjs[parent].mtx;
    let base = col(&pm, 3);
    let mut dir = col(&pm, 0);
    dir = dir.sdk_scale(inv_length(dir));
    let parent_scale = objs.jobjs[parent].scl.map_or(1.0, |p| p.x);
    let (bone_length, _, _) = ik_hint(objs, parent);
    dir = dir.sdk_scale(mem(fp::fmuls(reg(bone_length), reg(parent_scale))));
    let elbow = base.sdk_add(dir);
    dir = objs.jobjs[eff].translate.sdk_sub(elbow);
    dir = dir.sdk_scale(inv_length(dir));
    let r = objs.jobjs[j].robj;
    let min = robj::get_by_type(&objs.robjs, r, robj::LIMIT, 5);
    let max = robj::get_by_type(&objs.robjs, r, robj::LIMIT, 6);
    if min.is_some() || max.is_some() {
        let mut limited = false;
        let (_, _, flags) = ik_hint(objs, j);
        let parent_x = col(&pm, 0).sdk_normalize();
        let cos = parent_x.sdk_dot(dir);
        let mut angle = if reg(cos) >= 1.0 {
            0.0
        } else if reg(cos) <= -1.0 {
            mem(fp::frsp(std::f64::consts::PI))
        } else {
            math::acos(cos)
        };
        if flags & 4 == 0 {
            angle = -angle;
        }
        if let Some(lo) = min
            && reg(angle) < reg(objs.robjs[lo].limit())
        {
            angle = objs.robjs[lo].limit();
            limited = true;
        } else if let Some(hi) = max
            && reg(objs.robjs[hi].limit()) < reg(angle)
        {
            angle = objs.robjs[hi].limit();
            limited = true;
        }
        if limited {
            dir = Mtx34::rotation_about(col(&pm, 2), angle).transform(parent_x);
        }
    }
    let z = col(&pm, 2);
    let mut y = z.sdk_cross(dir);
    y = y.sdk_scale(inv_length(y));
    let z = dir.sdk_cross(y);
    set_basis(&mut objs.jobjs[j].mtx, dir, y, z, s, elbow);
}

/// The class's matrix method (`make_mtx`): every joint is the base class so far.
fn make_mtx(objs: &mut Objects, data: &Archives, j: JObjId) {
    make_matrix(objs, data, j);
}

/// Computes a dirty joint's matrix, then applies inverse kinematics or its references,
/// `HSD_JObjSetupMatrixSub`.
#[doc(alias = "HSD_JObjSetupMatrixSub")]
pub fn setup_matrix_sub(objs: &mut Objects, data: &Archives, j: JObjId) {
    let o = &objs.jobjs[j];
    if o.stale_w && o.flags & USE_QUATERNION != 0 {
        objs.stale_reads += 1;
    }
    make_mtx(objs, data, j);
    objs.jobjs[j].flags &= !MTX_DIRTY;
    if objs.jobjs[j].flags & USER_DEF_MTX != 0 {
        return;
    }
    match objs.jobjs[j].flags & JOINT_MASK {
        JOINT1 => resolve_ik_joint1(objs, data, j),
        JOINT2 => resolve_ik_joint2(objs, j),
        EFFECTOR => {
            if let Some(parent) = objs.jobjs[j].parent
                && let Some(r) =
                    robj::get_by_type(&objs.robjs, objs.jobjs[parent].robj, robj::IK_HINT, 0)
            {
                let (bone_length, _) = objs.robjs[r].ik_hint();
                let pm = objs.jobjs[parent].mtx;
                let mut x = col(&pm, 0);
                x = x.sdk_scale(inv_length(x));
                let scale = objs.jobjs[parent].scl.map_or(1.0, |p| p.x);
                x = x.sdk_scale(mem(fp::fmuls(reg(bone_length), reg(scale))));
                let t = col(&pm, 3).sdk_add(x);
                let m = &mut objs.jobjs[j].mtx.m;
                m[0][3] = t.x;
                m[1][3] = t.y;
                m[2][3] = t.z;
            }
        }
        _ => {
            let r = objs.jobjs[j].robj;
            if r.is_some() {
                robj::update_all(objs, data, r, j);
                if objs.jobjs[j].mtx_is_dirty() {
                    make_mtx(objs, data, j);
                    objs.jobjs[j].flags &= !MTX_DIRTY;
                }
            }
        }
    }
    objs.jobjs[j].flags &= !MTX_DIRTY;
}

/// Brings a joint's matrix up to date if it's dirty, the inline `HSD_JObjSetupMatrix`.
pub fn setup_matrix(objs: &mut Objects, data: &Archives, j: Option<JObjId>) {
    if let Some(j) = j
        && objs.jobjs[j].mtx_is_dirty()
    {
        setup_matrix_sub(objs, data, j);
    }
}

/// Marks the joint and its dependent descendants dirty, `HSD_JObjSetMtxDirtySub`.
#[doc(alias = "HSD_JObjSetMtxDirtySub")]
pub fn set_mtx_dirty_sub(jobjs: &mut JObjs, j: JObjId) {
    jobjs[j].flags |= MTX_DIRTY;
    let mut c = first_own_child(jobjs, j);
    while let Some(x) = c {
        let o = &jobjs[x];
        if o.flags & MTX_INDEP_PARENT == 0 && !o.mtx_is_dirty() {
            set_mtx_dirty_sub(jobjs, x);
        }
        c = jobjs[x].next;
    }
}

/// `HSD_JObjSetMtxDirty`.
#[doc(alias = "HSD_JObjSetMtxDirty")]
pub fn set_mtx_dirty(jobjs: &mut JObjs, j: Option<JObjId>) {
    if let Some(j) = j
        && !jobjs[j].mtx_is_dirty()
    {
        set_mtx_dirty_sub(jobjs, j);
    }
}

/// Whether the game handles particle events (`HSD_JObjSetDPtclCallback`).
#[doc(alias = "HSD_JObjSetDPtclCallback")]
pub fn set_dptcl_callback(objs: &mut Objects, set: bool) {
    objs.jobj.dptcl = set;
}

/// Releases what a joint owns: its children (an instance's shared tree by reference), its
/// place in its parent, its display objects, references and animation, `JObjReleaseChild`.
#[doc(alias = "JObjReleaseChild")]
fn release_child(objs: &mut Objects, j: JObjId) {
    if let Some(child) = objs.jobjs[j].child {
        if objs.jobjs[j].flags & INSTANCE != 0 {
            unref(objs, Some(child));
        } else {
            objs.jobjs[child].parent = None;
            remove_all(objs, Some(child));
        }
        objs.jobjs[j].child = None;
    }
    if objs.jobjs[j].parent.is_some() {
        reparent(&mut objs.jobjs, Some(j), None);
    }
    if let Some(d) = objs.jobjs[j].dobj() {
        dobj::remove_all(objs, Some(d));
        objs.jobjs[j].u = Contents::DObjs(None);
    }
    if let Some(r) = objs.jobjs[j].robj {
        robj::remove_all(objs, Some(r));
        objs.jobjs[j].robj = None;
    }
    if let Some(a) = objs.jobjs[j].aobj {
        remove_aobj(objs, Some(a));
        objs.jobjs[j].aobj = None;
    }
}

/// Builds an AObj from its descriptor, loading or referencing the joint it binds,
/// `HSD_AObjLoadDesc`.
pub fn load_aobj(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<AObjId> {
    let (a, binds) = aobj::load_desc_unbound(&mut objs.aobjs, &mut objs.fobjs, data, desc)?;
    if let Some(joint) = binds {
        let bound = match objs.ids.get(joint) {
            Some(j) => {
                ref_(&mut objs.jobjs, Some(j));
                Some(j)
            }
            None => load_joint(objs, data, Some(joint)),
        };
        if let Some(old) = objs.aobjs[a].hsd_obj {
            unref(objs, Some(old));
        }
        objs.aobjs[a].hsd_obj = bound;
    }
    Some(a)
}

/// Frees an AObj, its FObj chain and its reference to a bound joint, `HSD_AObjRemove`.
pub fn remove_aobj(objs: &mut Objects, a: Option<AObjId>) {
    let Some(a) = a else { return };
    if let Some(fobjs) = objs.aobjs[a].fobj {
        fobj::remove_all(&mut objs.fobjs, Some(fobjs));
    }
    objs.aobjs[a].fobj = None;
    if let Some(j) = objs.aobjs[a].hsd_obj.take() {
        unref(objs, Some(j));
    }
    aobj::free(&mut objs.aobjs, Some(a));
}
