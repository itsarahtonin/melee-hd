//! Joint helpers from `melee/lb/lb_00B0.c`.

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{mem, reg};
use crate::hsd::Objects;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// Where `offset` (or the origin, for None) in a joint's space is in the world,
/// `lb_8000B1CC`: through its matrix (brought up to date) when it has a parent, else from its
/// own transform. A missing joint gives `offset` back; the original faults on a missing
/// joint without an offset.
#[doc(alias = "lb_8000B1CC")]
pub fn world_position(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    offset: Option<Vec3>,
) -> Vec3 {
    let Some(j) = j else {
        return offset.expect("lb_8000B1CC: a missing joint with no offset reads address 0");
    };
    let is_zero = |v: &Vec3| reg(v.x) == 0.0 && reg(v.y) == 0.0 && reg(v.z) == 0.0;
    if objs.jobjs[j].parent.is_some() {
        jobj::setup_matrix(objs, data, Some(j));
        let m = &objs.jobjs[j].mtx.m;
        return match offset {
            Some(o) if !is_zero(&o) => objs.jobjs[j].mtx.transform(o),
            _ => Vec3::new(m[0][3], m[1][3], m[2][3]),
        };
    }
    let Some(o) = offset.filter(|o| !is_zero(o)) else {
        return objs.jobjs[j].translate;
    };
    let (r, s) = (objs.jobjs[j].rotate, objs.jobjs[j].scale);
    if reg(r.x) == 0.0
        && reg(r.y) == 0.0
        && reg(r.z) == 0.0
        && reg(s.x) == 1.0
        && reg(s.y) == 1.0
        && reg(s.z) == 1.0
    {
        let t = objs.jobjs[j].translate;
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        return Vec3::new(add(t.x, o.x), add(t.y, o.y), add(t.z, o.z));
    }
    jobj::setup_matrix(objs, data, Some(j));
    objs.jobjs[j].mtx.transform(o)
}

/// ORs `flags` into the render mode of the materials of a joint's display objects and of all
/// its descendants', `lb_80011C18` (through `lb_80011B74` for each list of display objects).
#[doc(alias = "lb_80011C18")]
#[doc(alias = "lb_80011B74")]
pub fn set_mobj_flags(objs: &mut Objects, j: JObjId, flags: u32) {
    fn dobjs(objs: &mut Objects, d: Option<crate::hsd::dobj::DObjId>, flags: u32) {
        let mut cur = d;
        while let Some(dobj) = cur {
            let m = objs.dobjs[dobj].mobj.expect("lb_80011B74: dobj->mobj");
            objs.mobjs[m].rendermode |= flags;
            cur = objs.dobjs[dobj].next;
        }
    }
    fn tree(objs: &mut Objects, j: JObjId, flags: u32) {
        if let Some(c) = objs.jobjs[j].child {
            tree(objs, c, flags);
        }
        if let Some(n) = objs.jobjs[j].next {
            tree(objs, n, flags);
        }
        let d = jobj::get_dobj(&objs.jobjs, Some(j));
        dobjs(objs, d, flags);
    }
    if let Some(c) = objs.jobjs[j].child {
        tree(objs, c, flags);
    }
    let d = jobj::get_dobj(&objs.jobjs, Some(j));
    dobjs(objs, d, flags);
}
