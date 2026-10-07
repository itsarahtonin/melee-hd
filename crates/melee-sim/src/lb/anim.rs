//! Joint animation helpers from `melee/lb/lb_00B0.c`.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::{copy, mem, reg};
use crate::hsd::Objects;
use crate::hsd::aobj::NO_ANIM;
use crate::hsd::jobj::{self, JObjId, JObjs, MTX_DIRTY, MTX_INDEP_SRT, USE_QUATERNION};
use crate::quat::Quat;
use crate::vec::Vec3;

/// Whether the joint has an animation that plays, `lb_8000B074`.
#[doc(alias = "lb_8000B074")]
pub fn is_animated(objs: &Objects, j: JObjId) -> bool {
    objs.jobjs[j]
        .aobj
        .is_some_and(|a| objs.aobjs[a].flags & NO_ANIM == 0)
}

/// The first joint in the tree (the joint, its descendants, then its later siblings') that
/// has an animation, `lbFindJObjWithAObj`.
#[doc(alias = "lbFindJObjWithAObj")]
fn with_aobj(jobjs: &JObjs, j: Option<JObjId>) -> Option<JObjId> {
    let j = j?;
    if jobjs[j].aobj.is_some() {
        return Some(j);
    }
    with_aobj(jobjs, jobjs[j].child).or_else(|| with_aobj(jobjs, jobjs[j].next))
}

/// The tree's animation rate, `lbGetJObjFramerate`.
#[doc(alias = "lbGetJObjFramerate")]
pub fn framerate(objs: &Objects, j: Option<JObjId>) -> f32 {
    match with_aobj(&objs.jobjs, j) {
        Some(j) => objs.aobjs[objs.jobjs[j].aobj.unwrap()].framerate,
        None => 0.0,
    }
}

/// The tree's animation's last frame, `lbGetJObjEndFrame`.
#[doc(alias = "lbGetJObjEndFrame")]
pub fn end_frame(objs: &Objects, j: Option<JObjId>) -> f32 {
    match with_aobj(&objs.jobjs, j) {
        Some(j) => objs.aobjs[objs.jobjs[j].aobj.unwrap()].end_frame,
        None => 0.0,
    }
}

/// The tree's animation frame, `lbGetJObjCurrFrame`.
#[doc(alias = "lbGetJObjCurrFrame")]
pub fn current_frame(objs: &Objects, j: Option<JObjId>) -> f32 {
    match with_aobj(&objs.jobjs, j) {
        Some(j) => objs.aobjs[objs.jobjs[j].aobj.unwrap()].curr_frame,
        None => 0.0,
    }
}

/// Copies one joint's scale, rotation and translation to another, quaternion flag included,
/// and dirties it, `lbCopyJObjSRT`.
#[doc(alias = "lbCopyJObjSRT")]
pub fn copy_srt(jobjs: &mut JObjs, src: JObjId, dst: JObjId) {
    let s = &jobjs[src];
    let (r, sc, t, q) = (s.rotate, s.scale, s.translate, s.flags & USE_QUATERNION);
    let d = &mut jobjs[dst];
    (d.rotate, d.scale, d.translate) = (r, sc, t);
    if q != 0 {
        jobj::set_flags(jobjs, Some(dst), USE_QUATERNION);
    } else {
        jobj::clear_flags(jobjs, Some(dst), USE_QUATERNION);
    }
    jobj::set_flags(jobjs, Some(dst), MTX_DIRTY);
}

/// Blends two joints' poses into `out`, `lb_8000C490`: scale and translation linearly
/// (`a` weighted `t`, `b` weighted `t_inv`), rotation by quaternion slerp unless both are
/// Euler angles within 0.0001 of each other, when `a`'s are kept.
#[doc(alias = "lb_8000C490")]
pub fn blend(jobjs: &mut JObjs, a: JObjId, b: JObjId, out: JObjId, t: f64, t_inv: f64) {
    let (ja, jb) = (jobjs[a].clone(), jobjs[b].clone());
    let lerp = |x: f32, y: f32| mem(fp::fmadds(reg(x), t, fp::fmuls(reg(y), t_inv)));
    {
        let o = &mut jobjs[out];
        o.translate.x = lerp(ja.translate.x, jb.translate.x);
        o.translate.y = lerp(ja.translate.y, jb.translate.y);
        o.translate.z = lerp(ja.translate.z, jb.translate.z);
        o.scale.x = lerp(ja.scale.x, jb.scale.x);
        o.scale.y = lerp(ja.scale.y, jb.scale.y);
        o.scale.z = lerp(ja.scale.z, jb.scale.z);
    }
    let quat_a = ja.flags & USE_QUATERNION != 0;
    let quat_b = jb.flags & USE_QUATERNION != 0;
    if !quat_a && !quat_b {
        let abs = |d: f64| if d < 0.0 { fp::fneg(d) } else { d };
        let d = |x: f32, y: f32| abs(fp::fsubs(reg(x), reg(y)));
        let (ra, rb) = (ja.rotate, jb.rotate);
        if d(ra.x, rb.x) <= 9.999999747378752e-05
            && d(ra.y, rb.y) <= 9.999999747378752e-05
            && d(ra.z, rb.z) <= 9.999999747378752e-05
        {
            jobjs[out].rotate = ra;
            jobj::clear_flags(jobjs, Some(out), USE_QUATERNION);
            jobj::set_flags(jobjs, Some(out), MTX_DIRTY);
            return;
        }
    }
    let as_quat = |o: &jobj::JObj, q: bool| {
        if q {
            o.rotate
        } else {
            Quat::from_euler(Vec3::new(
                copy(o.rotate.x),
                copy(o.rotate.y),
                copy(o.rotate.z),
            ))
        }
    };
    let q1 = as_quat(&ja, quat_a);
    let mut q2 = as_quat(&jb, quat_b);
    let sq = |x: f64| fp::fmuls(x, x);
    let (x1, y1, z1, w1) = (reg(q1.x), reg(q1.y), reg(q1.z), reg(q1.w));
    let (x2, y2, z2, w2) = (reg(q2.x), reg(q2.y), reg(q2.z), reg(q2.w));
    let (sx, sy) = (sq(fp::fadds(x1, x2)), sq(fp::fadds(y1, y2)));
    let (sz, sw) = (sq(fp::fadds(z1, z2)), sq(fp::fadds(w1, w2)));
    let (dx, dy) = (sq(fp::fsubs(x1, x2)), sq(fp::fsubs(y1, y2)));
    let (dz, dw) = (sq(fp::fsubs(z1, z2)), sq(fp::fsubs(w1, w2)));
    let sums = fp::fadds(sw, fp::fadds(sz, fp::fadds(sx, sy)));
    let diffs = fp::fadds(dw, fp::fadds(dz, fp::fadds(dx, dy)));
    if diffs > sums {
        q2 = Quat {
            x: mem(fp::fneg(x2)),
            y: mem(fp::fneg(y2)),
            z: mem(fp::fneg(z2)),
            w: mem(fp::fneg(w2)),
        };
    }
    jobjs[out].rotate = q1.slerp(q2, mem(t_inv));
    jobj::set_flags(jobjs, Some(out), USE_QUATERNION);
    jobj::set_flags(jobjs, Some(out), MTX_DIRTY);
}

/// A joint descriptor's (`HSD_Joint`) rotation, scale and translation.
fn joint_srt(data: &Archives, joint: DataRef) -> (Vec3, Vec3, Vec3) {
    let v = |at: u32| Vec3 {
        x: data.f32(joint.at(at)),
        y: data.f32(joint.at(at + 4)),
        z: data.f32(joint.at(at + 8)),
    };
    (v(0x14), v(0x20), v(0x2C))
}

/// Dirties the joint's matrix unless its SRT is independent.
fn dirty(jobjs: &mut JObjs, j: JObjId) {
    if jobjs[j].flags & MTX_INDEP_SRT == 0 {
        jobj::set_mtx_dirty(jobjs, Some(j));
    }
}

/// Resets a joint to its descriptor's pose, as Euler angles, `lb_8000B4FC`.
#[doc(alias = "lb_8000B4FC")]
pub fn reset_pose(jobjs: &mut JObjs, data: &Archives, j: Option<JObjId>, joint: Option<DataRef>) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    let (r, s, t) = joint_srt(data, joint);
    let o = &mut jobjs[j];
    (o.rotate.x, o.rotate.y, o.rotate.z) = (r.x, r.y, r.z);
    o.scale = s;
    o.translate = t;
    jobj::clear_flags(jobjs, Some(j), USE_QUATERNION);
    dirty(jobjs, j);
}

/// [`reset_pose`] keeping the joint's scale, `lb_8000B5DC`.
#[doc(alias = "lb_8000B5DC")]
pub fn reset_rotation_translation(
    jobjs: &mut JObjs,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    let (r, _, t) = joint_srt(data, joint);
    let o = &mut jobjs[j];
    (o.rotate.x, o.rotate.y, o.rotate.z) = (r.x, r.y, r.z);
    o.translate = t;
    jobj::clear_flags(jobjs, Some(j), USE_QUATERNION);
    dirty(jobjs, j);
}

/// Resets a joint's scale and translation to its descriptor's, `lb_8000B6A4`.
#[doc(alias = "lb_8000B6A4")]
pub fn reset_scale_translation(
    jobjs: &mut JObjs,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    let (_, s, t) = joint_srt(data, joint);
    let o = &mut jobjs[j];
    o.scale = s;
    o.translate = t;
    dirty(jobjs, j);
}

/// Resets a joint's translation to its descriptor's, `lb_8000B760`.
#[doc(alias = "lb_8000B760")]
pub fn reset_translation(
    jobjs: &mut JObjs,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    let (_, _, t) = joint_srt(data, joint);
    jobjs[j].translate = t;
    dirty(jobjs, j);
}

/// Sets the animations of a tree's joints below its root to run at `rate`, `lb_8000BA0C`
/// (`lb_8000B9D8` skips the root `HSD_JObjWalkTree` hands it first).
#[doc(alias = "lb_8000BA0C")]
#[doc(alias = "lb_8000B9D8")]
pub fn set_framerate(objs: &mut Objects, j: Option<JObjId>, rate: f32) {
    let mut joints = Vec::new();
    crate::hsd::jobj::walk_tree(&objs.jobjs, j, &mut |j, ty| {
        if ty != 0 {
            joints.push(j);
        }
    });
    for j in joints {
        let a = objs.jobjs[j].aobj;
        crate::hsd::aobj::set_rate(&mut objs.aobjs, a, rate);
    }
}

/// Makes a joint follow another's position and orientation (two constraint RObjs, flags
/// `0x90000001` and `0x90000004`, prepended in that order), `lb_8000C2F8`.
#[doc(alias = "lb_8000C2F8")]
pub fn attach(objs: &mut Objects, j: Option<JObjId>, target: Option<JObjId>) {
    for flags in [0x9000_0001, 0x9000_0004] {
        let r = Some(crate::hsd::robj::alloc(&mut objs.robjs));
        crate::hsd::robj::set_flags(&mut objs.robjs, r, flags);
        if let Some(t) = target {
            crate::hsd::robj::set_constraint_obj(objs, r, t);
        }
        jobj::prepend_robj(objs, j, r);
    }
}

/// Whether any joint from `j` on (its descendants, its later siblings, and its ancestors'
/// later siblings, in pre-order) still has an animation playing, `lb_8000B09C`.
#[doc(alias = "lb_8000B09C")]
pub fn any_animated(objs: &Objects, j: Option<JObjId>) -> bool {
    let jobjs = &objs.jobjs;
    let mut cur = j;
    while let Some(c) = cur {
        if is_animated(objs, c) {
            return true;
        }
        let o = &jobjs[c];
        cur = if (o.child.is_none() && o.next.is_none()) || o.flags & jobj::INSTANCE != 0 {
            let mut up = c;
            loop {
                let Some(p) = jobjs[up].parent else {
                    break None;
                };
                if let Some(n) = jobjs[p].next {
                    break Some(n);
                }
                up = p;
            }
        } else if o.child.is_some() {
            o.child
        } else {
            o.next
        };
    }
    false
}

/// Adds the `i`th animations of three lists (joint, material and shape animations, any
/// missing) to a joint tree, `lb_8000C07C`.
#[doc(alias = "lb_8000C07C")]
pub fn add_anim_index(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    i: u32,
    anims: Option<DataRef>,
    matanims: Option<DataRef>,
    shapeanims: Option<DataRef>,
) {
    let pick = |l: Option<DataRef>| l.and_then(|l| data.ptr(l.at(4 * i)));
    jobj::add_anim_all(objs, data, j, pick(anims), pick(matanims), pick(shapeanims));
}

/// [`add_anim_index`] with a `DynamicModelDesc`'s lists, `lb_8000C0E8`.
#[doc(alias = "lb_8000C0E8")]
pub fn add_model_anim(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    i: u32,
    model: DataRef,
) {
    let list = |o: u32| data.ptr(model.at(o));
    add_anim_index(objs, data, j, i, list(4), list(8), list(0xC));
}
