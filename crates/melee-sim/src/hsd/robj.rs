//! Reference objects, HAL's RObj (`sysdolphin/baselib/robj.c`): what a joint's matrix depends
//! on beyond its own SRT. Constraints to other joints (position, direction, orientation),
//! limits on its rotation and translation, expressions over other joints' values, and the
//! hints inverse kinematics reads.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::bytecode;
use super::fobj::Value;
use super::jobj::{self, JObjId};
use super::pool::Pool;
use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::math::sqrt_reg;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// An RObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RObjId(pub u32);

/// An Rvalue's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RvalueId(pub u32);

/// The reference type, in the flags' bits 28 to 30.
pub const TYPE_MASK: u32 = 0x7000_0000;
pub const EXP: u32 = 0x0000_0000;
pub const JOBJ: u32 = 0x1000_0000;
pub const LIMIT: u32 = 0x2000_0000;
pub const BYTECODE: u32 = 0x3000_0000;
pub const IK_HINT: u32 = 0x4000_0000;
/// The reference is in effect (its animation can switch it).
pub const ACTIVE: u32 = 0x8000_0000;

/// The animation channel that switches an RObj (`TYPE_ROBJ`).
const TYPE_ROBJ: u8 = 1;

/// What an RObj refers to: the original's union, by the type it was loaded as.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ref {
    /// A constraint's target joint, set when references resolve.
    #[default]
    None,
    JObj(JObjId),
    Exp(Exp),
    Limit(f32),
    IkHint {
        bone_length: f32,
        rotate_x: f32,
    },
}

/// `HSD_Exp`: an expression over other joints' values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Exp {
    /// The bytecode, or none for a C expression (the original's `dummy_func`, 0).
    pub bytecode: Option<DataRef>,
    pub rvalue: Option<RvalueId>,
    /// How many arguments the rvalues supply; `u32::MAX` until first evaluated.
    pub nb_args: u32,
    pub is_bytecode: bool,
}

/// `HSD_RObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct RObj {
    pub next: Option<RObjId>,
    pub flags: u32,
    pub u: Ref,
    pub aobj: Option<AObjId>,
}

impl RObj {
    fn has_type(&self, ty: u32) -> bool {
        self.flags & TYPE_MASK == ty
    }

    fn active(&self) -> bool {
        self.flags & ACTIVE != 0
    }

    /// The target joint of a joint constraint.
    fn target(&self) -> JObjId {
        match self.u {
            Ref::JObj(j) => j,
            _ => panic!("constraint without a target joint"),
        }
    }

    /// A limit's value.
    pub fn limit(&self) -> f32 {
        match self.u {
            Ref::Limit(l) => l,
            _ => panic!("RObj {:#x} has no limit", self.flags),
        }
    }

    /// An IK hint's bone length and rotation.
    pub fn ik_hint(&self) -> (f32, f32) {
        match self.u {
            Ref::IkHint {
                bone_length,
                rotate_x,
            } => (bone_length, rotate_x),
            _ => panic!("RObj {:#x} has no IK hint", self.flags),
        }
    }
}

/// `HSD_Rvalue`: one joint an expression reads, and which of its values.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Rvalue {
    pub next: Option<RvalueId>,
    pub flags: u32,
    pub jobj: Option<JObjId>,
}

/// The RObj pool (`robj_alloc_data`).
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct RObjs {
    pub pool: Pool<RObj, { pool_kind(Kind::RObj) }>,
}

/// The Rvalue pool (`rvalue_alloc_data`).
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Rvalues {
    pub pool: Pool<Rvalue, { pool_kind(Kind::Rvalue) }>,
}

impl Index<RObjId> for RObjs {
    type Output = RObj;
    fn index(&self, r: RObjId) -> &RObj {
        &self.pool[r.0]
    }
}

impl IndexMut<RObjId> for RObjs {
    fn index_mut(&mut self, r: RObjId) -> &mut RObj {
        &mut self.pool[r.0]
    }
}

impl Index<RvalueId> for Rvalues {
    type Output = Rvalue;
    fn index(&self, r: RvalueId) -> &Rvalue {
        &self.pool[r.0]
    }
}

impl IndexMut<RvalueId> for Rvalues {
    fn index_mut(&mut self, r: RvalueId) -> &mut Rvalue {
        &mut self.pool[r.0]
    }
}

/// The RObjs of a list, in order.
fn each(robjs: &RObjs, first: Option<RObjId>) -> impl Iterator<Item = RObjId> + '_ {
    std::iter::successors(first, move |&r| robjs[r].next)
}

/// `HSD_RObjSetFlags`.
#[doc(alias = "HSD_RObjSetFlags")]
pub fn set_flags(robjs: &mut RObjs, r: Option<RObjId>, flags: u32) {
    if let Some(r) = r {
        robjs[r].flags |= flags;
    }
}

/// The first active RObj of a type, and of a subtype unless `subtype` is 0,
/// `HSD_RObjGetByType`.
#[doc(alias = "HSD_RObjGetByType")]
pub fn get_by_type(robjs: &RObjs, first: Option<RObjId>, ty: u32, subtype: u32) -> Option<RObjId> {
    each(robjs, first).find(|&r| {
        let o = &robjs[r];
        o.active() && o.has_type(ty) && (subtype == 0 || subtype == o.flags & 0x0FFF_FFFF)
    })
}

/// Animates each RObj of a list, which can switch it on and off, `HSD_RObjAnimAll` (and
/// `HSD_RObjAnim`, inlined).
#[doc(alias = "HSD_RObjAnimAll")]
pub fn anim_all(objs: &mut Objects, data: &Archives, first: Option<RObjId>) {
    let mut cur = first;
    while let Some(r) = cur {
        let a = objs.robjs[r].aobj;
        // RObjUpdateFunc.
        let mut update = |objs: &mut Objects, ty: u8, v: Value| {
            if ty != TYPE_ROBJ {
                return;
            }
            let v = reg(v.or_address());
            let o = &mut objs.robjs[r];
            if v >= 0.5 {
                o.flags |= ACTIVE;
            } else {
                o.flags &= !ACTIVE;
            }
        };
        aobj::interpret(objs, data, a, Some(&mut update));
        cur = objs.robjs[r].next;
    }
}

/// Removes each RObj's animation if `flags` has the RObj bit, `HSD_RObjRemoveAnimAllByFlags`.
#[doc(alias = "HSD_RObjRemoveAnimAllByFlags")]
pub fn remove_anim_all_by_flags(objs: &mut Objects, first: Option<RObjId>, flags: u32) {
    let mut cur = first;
    while let Some(r) = cur {
        if let Some(a) = objs.robjs[r].aobj
            && flags & 0x80 != 0
        {
            jobj::remove_aobj(objs, Some(a));
            objs.robjs[r].aobj = None;
        }
        cur = objs.robjs[r].next;
    }
}

/// `HSD_RObjRemoveAnimAll`.
#[doc(alias = "HSD_RObjRemoveAnimAll")]
pub fn remove_anim_all(objs: &mut Objects, first: Option<RObjId>) {
    remove_anim_all_by_flags(objs, first, 0x7FF);
}

/// Requests each RObj's animation from `frame` if `flags` has the RObj bit,
/// `HSD_RObjReqAnimAllByFlags`.
#[doc(alias = "HSD_RObjReqAnimAllByFlags")]
pub fn req_anim_all_by_flags(objs: &mut Objects, first: Option<RObjId>, frame: f32, flags: u32) {
    let mut cur = first;
    while let Some(r) = cur {
        if let Some(a) = objs.robjs[r].aobj
            && flags & 0x80 != 0
        {
            aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, Some(a), frame);
        }
        cur = objs.robjs[r].next;
    }
}

/// `HSD_RObjReqAnimAll`.
#[doc(alias = "HSD_RObjReqAnimAll")]
pub fn req_anim_all(objs: &mut Objects, first: Option<RObjId>, frame: f32) {
    req_anim_all_by_flags(objs, first, frame, 0x7FF);
}

/// Gives each RObj the animation of its RObjAnimJoint, `HSD_RObjAddAnimAll`.
#[doc(alias = "HSD_RObjAddAnimAll")]
pub fn add_anim_all(
    objs: &mut Objects,
    data: &Archives,
    first: Option<RObjId>,
    anim: Option<DataRef>,
) {
    let (Some(_), Some(_)) = (first, anim) else {
        return;
    };
    let (mut r, mut j) = (first, anim);
    while let (Some(ro), Some(jo)) = (r, j) {
        // HSD_RObjAddAnim.
        if let Some(a) = objs.robjs[ro].aobj {
            jobj::remove_aobj(objs, Some(a));
        }
        let a = jobj::load_aobj(objs, data, data.ptr(jo.at(4)));
        objs.robjs[ro].aobj = a;
        r = objs.robjs[ro].next;
        j = data.ptr(jo);
    }
}

/// The mean position of the active joint constraints of type `ty`, and how many there are,
/// `HSD_RObjGetGlobalPosition`. `None` when there are none (the original leaves its output
/// alone).
#[doc(alias = "HSD_RObjGetGlobalPosition")]
pub fn global_position(
    objs: &mut Objects,
    data: &Archives,
    first: Option<RObjId>,
    ty: u32,
) -> Option<Vec3> {
    let (mut x, mut y, mut z) = (0.0f32, 0.0f32, 0.0f32);
    let mut n = 0i32;
    let mut cur = first;
    while let Some(r) = cur {
        let o = &objs.robjs[r];
        if o.has_type(JOBJ) && o.active() && ty == o.flags & 0x0FFF_FFFF {
            let target = o.target();
            jobj::setup_matrix(objs, data, Some(target));
            let m = &objs.jobjs[target].mtx.m;
            n = n.wrapping_add(1);
            x = mem(fp::fadds(reg(x), reg(m[0][3])));
            y = mem(fp::fadds(reg(y), reg(m[1][3])));
            z = mem(fp::fadds(reg(z), reg(m[2][3])));
        }
        cur = objs.robjs[r].next;
    }
    if n == 0 {
        return None;
    }
    let f = fp::fdivs(1.0, fp::frsp(f64::from(n)));
    Some(Vec3::new(
        mem(fp::fmuls(f, reg(x))),
        mem(fp::fmuls(f, reg(y))),
        mem(fp::fmuls(f, reg(z))),
    ))
}

/// `sqrtf(1 / (1e-10 + v·v))`, the reciprocal length the constraints and IK normalize by.
pub(crate) fn inv_length(v: Vec3) -> f32 {
    let d = fp::fadds(1.000000013351432e-10, reg(v.sdk_dot(v)));
    mem(sqrt_reg(fp::fdivs(1.0, d)))
}

/// A matrix column's length, `HSD_MtxColMag`.
fn col_mag(m: &Mtx34, col: usize) -> f64 {
    let sq = |r: usize| fp::fmuls(reg(m.m[r][col]), reg(m.m[r][col]));
    sqrt_reg(fp::fadds(sq(2), fp::fadds(sq(1), sq(0))))
}

/// Points the joint's matrix along `dir` with `up` as its second axis: the static
/// `set_dirup_matrix`.
fn set_dirup_matrix(
    objs: &mut Objects,
    data: &Archives,
    j: JObjId,
    mut dir: Vec3,
    up: Vec3,
    scale: Vec3,
) {
    let z = dir.sdk_cross(up);
    dir = dir.sdk_scale(inv_length(dir));
    let z = z.sdk_scale(inv_length(z));
    let up = z.sdk_cross(dir);
    let scaled = |v: Vec3, s: f32| {
        let s = reg(s);
        Vec3::new(
            mem(fp::fmuls(reg(v.x), s)),
            mem(fp::fmuls(reg(v.y), s)),
            mem(fp::fmuls(reg(v.z), s)),
        )
    };
    jobj::update(objs, data, j, 50, Value::Point(scaled(dir, scale.x)));
    jobj::update(objs, data, j, 51, Value::Point(scaled(up, scale.y)));
    jobj::update(objs, data, j, 52, Value::Point(scaled(z, scale.z)));
    jobj::update(objs, data, j, 55, Value::Empty);
}

/// Aims the joint at its direction constraints' targets, the static `resolveCnsDirUp`.
fn resolve_dir_up(objs: &mut Objects, data: &Archives, first: Option<RObjId>, j: JObjId) {
    let Some(pos) = global_position(objs, data, first, 2) else {
        return;
    };
    let m = &objs.jobjs[j].mtx.m;
    let here = Vec3::new(m[0][3], m[1][3], m[2][3]);
    let pos = pos.sdk_sub(here);
    let mut up = Vec3::new(0.0, 1.0, 0.0);
    if let Some(u) = global_position(objs, data, first, 3) {
        up = u.sdk_sub(here);
    } else {
        let k = fp::fsubs(1.0, reg(pos.sdk_dot(up)));
        if reg(f32::from_bits(mem(k).to_bits() & 0x7FFF_FFFF)) < 1.000000013351432e-10 {
            up = Vec3::new(0.0, 0.0, 1.0);
        }
    }
    let scale = objs.jobjs[j].scl.unwrap_or(Vec3::new(1.0, 1.0, 1.0));
    set_dirup_matrix(objs, data, j, pos, up, scale);
}

/// The channels orientation constraints write, a matrix's columns (`HSD_RObj_80406E74`).
const COLUMNS: [u8; 3] = [0x32, 0x33, 0x34];

/// A matrix column as a vector, `HSD_MtxColVec`.
fn column(m: &Mtx34, i: usize) -> Vec3 {
    Vec3::new(m.m[0][i], m.m[1][i], m.m[2][i])
}

fn set_column(m: &mut Mtx34, i: usize, v: Vec3) {
    m.m[0][i] = v.x;
    m.m[1][i] = v.y;
    m.m[2][i] = v.z;
}

/// `v` scaled by `s` componentwise in single precision.
fn times(v: Vec3, s: f64) -> Vec3 {
    Vec3::new(
        mem(fp::fmuls(reg(v.x), s)),
        mem(fp::fmuls(reg(v.y), s)),
        mem(fp::fmuls(reg(v.z), s)),
    )
}

/// 1 over a length, unless the length is about 0.
fn inv_unless_tiny(len: f32) -> f64 {
    let s = reg(len);
    if s > 1.000000013351432e-10 {
        fp::fdivs(1.0, s)
    } else {
        s
    }
}

/// A joint's matrix, brought up to date, the inline `HSD_JObjGetMtxPtr`.
fn mtx_ptr(objs: &mut Objects, data: &Archives, j: JObjId) -> Mtx34 {
    jobj::setup_matrix(objs, data, Some(j));
    objs.jobjs[j].mtx
}

/// Copies its orientation constraint's target's rotation, keeping the joint's own scale, the
/// static `resolveCnsOrientation`.
fn resolve_orientation(objs: &mut Objects, data: &Archives, first: Option<RObjId>, j: JObjId) {
    // inlined_HSD_RObjGetByType
    let Some(r) = get_by_type(&objs.robjs, first, JOBJ, 4) else {
        return;
    };
    let target = objs.robjs[r].target();
    let t = &objs.jobjs[target];
    let classical_parent = t.parent.filter(|_| t.flags & jobj::CLASSICAL_SCALE != 0);
    if let Some(parent) = classical_parent {
        let tm = mtx_ptr(objs, data, target);
        let mut m1 = mtx_ptr(objs, data, parent).hsd_inverse_concat(&tm);
        for i in 0..3 {
            let v = column(&m1, i);
            let s = inv_unless_tiny(v.sdk_length());
            let s = fp::fmuls(s, col_mag(&objs.jobjs[j].mtx, i));
            set_column(&mut m1, i, times(v, s));
        }
        let mut cur = Some(parent);
        while let Some(p) = cur {
            let mut m0 = match objs.jobjs[p].parent {
                Some(pp) => {
                    let pm = mtx_ptr(objs, data, p);
                    mtx_ptr(objs, data, pp).hsd_inverse_concat(&pm)
                }
                None => mtx_ptr(objs, data, p).copied(),
            };
            for i in 0..3 {
                let v = column(&m0, i);
                let s = inv_unless_tiny(v.sdk_length());
                set_column(&mut m0, i, times(v, s));
            }
            m1 = m0.concat(&m1);
            cur = objs.jobjs[p].parent;
        }
        for (i, &ty) in COLUMNS.iter().enumerate() {
            jobj::update(objs, data, j, ty, Value::Point(column(&m1, i)));
        }
    } else {
        let m0 = mtx_ptr(objs, data, target).copied();
        for (i, &ty) in COLUMNS.iter().enumerate() {
            let v = column(&m0, i);
            let s = inv_unless_tiny(v.sdk_length());
            let s = fp::fmuls(s, col_mag(&objs.jobjs[j].mtx, i));
            jobj::update(objs, data, j, ty, Value::Point(times(v, s)));
        }
    }
    jobj::update(objs, data, j, 0x37, Value::Empty);
}

/// Clamps the joint's rotation and translation to its active limits, the static
/// `resolveLimits`.
fn resolve_limits(objs: &mut Objects, data: &Archives, first: Option<RObjId>, j: JObjId) {
    if !each(&objs.robjs, first).any(|r| objs.robjs[r].has_type(LIMIT)) {
        return;
    }
    let mut update_mtx = false;
    let mut cur = first;
    while let Some(r) = cur {
        let o = &objs.robjs[r];
        cur = o.next;
        if !o.has_type(LIMIT) {
            continue;
        }
        let jo = &mut objs.jobjs[j];
        let (field, below) = match o.flags & 0x0FFF_FFFF {
            1 => (&mut jo.rotate.x, true),
            2 => (&mut jo.rotate.x, false),
            3 => (&mut jo.rotate.y, true),
            4 => (&mut jo.rotate.y, false),
            5 => (&mut jo.rotate.z, true),
            6 => (&mut jo.rotate.z, false),
            7 => (&mut jo.translate.x, true),
            8 => (&mut jo.translate.x, false),
            // The original clamps y for 11 and 12 too, where z looks meant.
            9 | 11 => (&mut jo.translate.y, true),
            10 | 12 => (&mut jo.translate.y, false),
            _ => continue,
        };
        let limit = o.limit();
        let past = if below {
            reg(*field) < reg(limit)
        } else {
            reg(*field) > reg(limit)
        };
        if past {
            *field = copy(limit);
        }
        update_mtx = true;
    }
    if update_mtx {
        jobj::make_matrix(objs, data, j);
    }
}

/// Applies a joint's references: position, direction and orientation constraints, limits,
/// then active expressions, handing the results to the joint's update function,
/// `HSD_RObjUpdateAll`.
#[doc(alias = "HSD_RObjUpdateAll")]
pub fn update_all(objs: &mut Objects, data: &Archives, first: Option<RObjId>, j: JObjId) {
    if first.is_none() {
        return;
    }
    if let Some(p) = global_position(objs, data, first, 1) {
        jobj::update(objs, data, j, 0x35, Value::Point(p));
        jobj::update(objs, data, j, 0x38, Value::Empty);
    }
    resolve_dir_up(objs, data, first, j);
    resolve_orientation(objs, data, first, j);
    resolve_limits(objs, data, first, j);
    let mut cur = first;
    while let Some(r) = cur {
        let o = &objs.robjs[r];
        if o.has_type(EXP) && o.active() {
            let ty = o.flags & 0x0FFF_FFFF;
            evaluate(objs, data, r, ty, j);
        }
        cur = objs.robjs[r].next;
    }
}

/// Evaluates an expression and hands its value to the joint, the static `expEvaluate`.
fn evaluate(objs: &mut Objects, data: &Archives, r: RObjId, ty: u32, j: JObjId) {
    let Ref::Exp(mut exp) = objs.robjs[r].u else {
        panic!("expression RObj without an expression")
    };
    if exp.nb_args == u32::MAX {
        exp.nb_args = std::iter::successors(exp.rvalue, |&v| objs.rvalues[v].next)
            .map(|v| objs.rvalues[v].flags.count_ones())
            .sum();
        objs.robjs[r].u = Ref::Exp(exp);
    }
    // The original's argument buffer holds 100.
    assert!(
        exp.nb_args <= 100,
        "Number of argment of expression exceeds the argument buffer"
    );
    let mut args = [0f32; 100];
    let mut n = 0;
    let deg = reg(57.29578);
    let mut cur = exp.rvalue;
    while let Some(v) = cur {
        let rv = &objs.rvalues[v];
        let (flags, next) = (rv.flags, rv.next);
        let jv = rv.jobj.expect("rvalue without a joint");
        jobj::setup_matrix(objs, data, Some(jv));
        let jo = &objs.jobjs[jv];
        let mut bit = 1u32;
        while bit != 0 && bit <= flags {
            let value = match flags & bit {
                0x1 => Some(mem(fp::fmuls(deg, reg(jo.rotate.x)))),
                0x2 => Some(mem(fp::fmuls(deg, reg(jo.rotate.y)))),
                0x4 => Some(mem(fp::fmuls(deg, reg(jo.rotate.z)))),
                0x10 => Some(copy(jo.translate.x)),
                0x20 => Some(copy(jo.translate.y)),
                0x40 => Some(copy(jo.translate.z)),
                0x80 => Some(copy(jo.scale.x)),
                0x100 => Some(copy(jo.scale.y)),
                0x200 => Some(copy(jo.scale.z)),
                0x10000 => Some(mem(fp::fmuls(deg, reg(jo.mtx.rotation_angles().x)))),
                0x20000 => Some(mem(fp::fmuls(deg, reg(jo.mtx.rotation_angles().y)))),
                0x40000 => Some(mem(fp::fmuls(deg, reg(jo.mtx.rotation_angles().z)))),
                0x100000 => Some(copy(jo.mtx.m[0][3])),
                0x200000 => Some(copy(jo.mtx.m[1][3])),
                0x400000 => Some(copy(jo.mtx.m[2][3])),
                0x800000 => Some(jo.mtx.scale_part().x),
                0x1000000 => Some(jo.mtx.scale_part().y),
                0x2000000 => Some(jo.mtx.scale_part().z),
                _ => None,
            };
            if let Some(value) = value {
                args[n] = value;
                n += 1;
            }
            bit <<= 1;
        }
        cur = next;
    }
    let mut value = if exp.is_bytecode {
        bytecode::eval(
            data,
            &mut objs.rng,
            exp.bytecode,
            &args[..exp.nb_args as usize],
        )
    } else {
        // dummy_func: no C expression in the game's data has a function of its own.
        0.0
    };
    if ty.wrapping_sub(1) <= 2 {
        value = mem(fp::fmuls(reg(value), reg(0.017453292)));
    }
    jobj::update(objs, data, j, ty as u8, Value::Float(value));
}

/// Points constraint and expression references at the joints their descriptors name, now
/// loaded, `HSD_RObjResolveRefsAll`.
#[doc(alias = "HSD_RObjResolveRefsAll")]
pub fn resolve_refs_all(
    objs: &mut Objects,
    data: &Archives,
    first: Option<RObjId>,
    desc: Option<DataRef>,
) {
    let (mut r, mut d) = (first, desc);
    while let (Some(ro), Some(de)) = (r, d) {
        // HSD_RObjResolveRefs.
        let o = &objs.robjs[ro];
        match o.flags & TYPE_MASK {
            JOBJ => {
                if let Ref::JObj(old) = o.u {
                    jobj::unref_this(objs, Some(old));
                }
                let joint = data.ptr(de.at(8)).expect("constraint without a joint");
                let target = objs
                    .ids
                    .get(joint)
                    .expect("constraint's joint isn't loaded");
                objs.robjs[ro].u = Ref::JObj(target);
                jobj::ref_this(&mut objs.jobjs, Some(target));
            }
            EXP => {
                let Ref::Exp(exp) = o.u else {
                    panic!("expression RObj without an expression")
                };
                let list = data.ptr(de.at(8)).and_then(|e| data.ptr(e.at(4)));
                resolve_rvalues(objs, data, exp.rvalue, list);
            }
            _ => {}
        }
        r = objs.robjs[ro].next;
        d = data.ptr(de);
    }
}

/// `HSD_RvalueResolveRefsAll`.
#[doc(alias = "HSD_RvalueResolveRefsAll")]
fn resolve_rvalues(
    objs: &mut Objects,
    data: &Archives,
    first: Option<RvalueId>,
    list: Option<DataRef>,
) {
    let Some(mut entry) = list else { return };
    let mut cur = first;
    while let Some(v) = cur {
        let Some(joint) = data.ptr(entry.at(4)) else {
            break;
        };
        // HSD_RvalueResolveRefs.
        jobj::unref_this(objs, objs.rvalues[v].jobj);
        let target = objs.ids.get(joint).expect("rvalue's joint isn't loaded");
        objs.rvalues[v].jobj = Some(target);
        jobj::ref_this(&mut objs.jobjs, Some(target));
        cur = objs.rvalues[v].next;
        entry = entry.at(8);
    }
}

/// Builds an RObj list from its descriptors, `HSD_RObjLoadDesc`.
#[doc(alias = "HSD_RObjLoadDesc")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<RObjId> {
    let desc = desc?;
    let r = alloc(&mut objs.robjs);
    let next = load_desc(objs, data, data.ptr(desc));
    let mut flags = data.u32(desc.at(4));
    let at = desc.at(8);
    let u = match flags & TYPE_MASK {
        JOBJ => Ref::None,
        LIMIT => {
            let raw = data.f32(at);
            Ref::Limit(match flags & 0x0FFF_FFFF {
                1..=6 => mem(fp::fmuls(reg(0.017453292), reg(raw))),
                _ => copy(raw),
            })
        }
        EXP => Ref::Exp(load_exp(objs, data, data.ptr(at), false)),
        BYTECODE => {
            flags &= !TYPE_MASK;
            Ref::Exp(load_exp(objs, data, data.ptr(at), true))
        }
        IK_HINT => {
            let hint = data.ptr(at).expect("IK hint without a descriptor");
            Ref::IkHint {
                bone_length: copy(data.f32(hint)),
                rotate_x: copy(data.f32(hint.at(4))),
            }
        }
        _ => panic!("unexpected type of robj."),
    };
    let o = &mut objs.robjs[r];
    o.next = next;
    o.flags = flags;
    o.u = u;
    Some(r)
}

/// `expLoadDesc` and `bcexpLoadDesc`: both descriptors are a function or bytecode, then an
/// rvalue list.
fn load_exp(objs: &mut Objects, data: &Archives, desc: Option<DataRef>, bytecode: bool) -> Exp {
    let Some(desc) = desc else {
        return Exp::default();
    };
    if !bytecode {
        assert!(
            data.u32(desc) == 0,
            "expression {desc:?} has a C function, which isn't supported"
        );
    }
    Exp {
        bytecode: if bytecode { data.ptr(desc) } else { None },
        rvalue: load_rvalues(&mut objs.rvalues, data, data.ptr(desc.at(4))),
        nb_args: u32::MAX,
        is_bytecode: bytecode,
    }
}

/// An rvalue per entry of a list ending at a null joint, the static `loadRvalue`.
fn load_rvalues(rvalues: &mut Rvalues, data: &Archives, list: Option<DataRef>) -> Option<RvalueId> {
    let mut entry = list?;
    let mut first = None;
    let mut last: Option<RvalueId> = None;
    while data.ptr(entry.at(4)).is_some() {
        let v = RvalueId(rvalues.pool.alloc(Rvalue {
            flags: data.u32(entry),
            ..Rvalue::default()
        }));
        match last {
            Some(l) => rvalues[l].next = Some(v),
            None => first = Some(v),
        }
        last = Some(v);
        entry = entry.at(8);
    }
    first
}

/// Frees an RObj, its rvalues and its animation, `HSD_RObjRemove`.
#[doc(alias = "HSD_RObjRemove")]
pub fn remove(objs: &mut Objects, r: Option<RObjId>) {
    let Some(r) = r else { return };
    match objs.robjs[r].u {
        Ref::JObj(j) if objs.robjs[r].has_type(JOBJ) => jobj::unref_this(objs, Some(j)),
        Ref::Exp(exp) if objs.robjs[r].has_type(EXP) => remove_rvalues(objs, exp.rvalue),
        _ => {}
    }
    jobj::remove_aobj(objs, objs.robjs[r].aobj);
    free(&mut objs.robjs, Some(r));
}

/// `HSD_RObjRemoveAll`.
#[doc(alias = "HSD_RObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, first: Option<RObjId>) {
    let mut cur = first;
    while let Some(r) = cur {
        cur = objs.robjs[r].next;
        remove(objs, Some(r));
    }
}

/// `HSD_RvalueRemoveAll` (and `HSD_RvalueRemove`, inlined).
#[doc(alias = "HSD_RvalueRemoveAll")]
fn remove_rvalues(objs: &mut Objects, first: Option<RvalueId>) {
    let mut cur = first;
    while let Some(v) = cur {
        cur = objs.rvalues[v].next;
        jobj::unref_this(objs, objs.rvalues[v].jobj);
        objs.rvalues.pool.free(v.0);
    }
}

/// A cleared RObj, `HSD_RObjAlloc`.
#[doc(alias = "HSD_RObjAlloc")]
pub fn alloc(robjs: &mut RObjs) -> RObjId {
    RObjId(robjs.pool.alloc(RObj::default()))
}

/// `HSD_RObjFree`.
#[doc(alias = "HSD_RObjFree")]
pub fn free(robjs: &mut RObjs, r: Option<RObjId>) {
    if let Some(r) = r {
        robjs.pool.free(r.0);
    }
}

/// Makes a joint the RObj's constraint target, `HSD_RObjSetConstraintObj`.
#[doc(alias = "HSD_RObjSetConstraintObj")]
pub fn set_constraint_obj(objs: &mut Objects, r: Option<RObjId>, target: JObjId) {
    let Some(r) = r else { return };
    if let Ref::JObj(old) = objs.robjs[r].u {
        jobj::unref_this(objs, Some(old));
        objs.robjs[r].u = Ref::None;
    }
    objs.robjs[r].u = Ref::JObj(target);
    jobj::ref_this(&mut objs.jobjs, Some(target));
}
