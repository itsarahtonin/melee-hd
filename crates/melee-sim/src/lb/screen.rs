//! Projecting a world point onto the screen through a camera, as the game tests what is
//! on screen: `lbVector_WorldToScreen` (`melee/lb/lbvector.c`) with the SDK's `GXProject`.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{copy, mem, reg};
use crate::hsd::Objects;
use crate::hsd::cobj::{self, CObjId};
use crate::mtx::{Mtx34, Mtx44};
use crate::vec::Vec3;

/// Projects `p` through the view matrix `m`, the projection parameters `pm` (`pm[0]` is 0 for
/// a perspective) and the viewport `vp` (x, y, width, height, near, far) to window
/// coordinates, `GXProject`.
#[doc(alias = "GXProject")]
pub fn gx_project(p: [f64; 3], m: &Mtx34, pm: &[f32; 7], vp: &[f32; 6]) -> Vec3 {
    let [x, y, z] = p;
    let row = |r: &[f32; 4]| {
        mem(fp::fadds(
            reg(r[3]),
            fp::fadds(
                fp::fmuls(reg(r[2]), z),
                fp::fadds(fp::fmuls(reg(r[0]), x), fp::fmuls(reg(r[1]), y)),
            ),
        ))
    };
    let eye = Vec3::new(row(&m.m[0]), row(&m.m[1]), row(&m.m[2]));
    let pm = pm.map(reg);
    let vp = vp.map(reg);
    let (ex, ey, ez) = (reg(eye.x), reg(eye.y), reg(eye.z));
    let (xc, yc, zc, wc) = if pm[0] == 0.0 {
        (
            fp::fadds(fp::fmuls(ex, pm[1]), fp::fmuls(ez, pm[2])),
            fp::fadds(fp::fmuls(ey, pm[3]), fp::fmuls(ez, pm[4])),
            fp::fadds(pm[6], fp::fmuls(ez, pm[5])),
            fp::fdivs(1.0, fp::fneg(ez)),
        )
    } else {
        (
            fp::fadds(pm[2], fp::fmuls(ex, pm[1])),
            fp::fadds(pm[4], fp::fmuls(ey, pm[3])),
            fp::fadds(pm[6], fp::fmuls(ez, pm[5])),
            1.0,
        )
    };
    let sx = fp::fadds(
        fp::fdivs(vp[2], 2.0),
        fp::fadds(vp[0], fp::fmuls(wc, fp::fdivs(fp::fmuls(xc, vp[2]), 2.0))),
    );
    let sy = fp::fadds(
        fp::fdivs(vp[3], 2.0),
        fp::fadds(
            vp[1],
            fp::fmuls(wc, fp::fdivs(fp::fmuls(fp::fneg(yc), vp[3]), 2.0)),
        ),
    );
    let sz = fp::fadds(vp[5], fp::fmuls(wc, fp::fmuls(zc, fp::fsubs(vp[5], vp[4]))));
    Vec3::new(mem(sx), mem(sy), mem(sz))
}

/// Where `pos` lands on the screen through camera `c`, `lbVector_WorldToScreen`: with
/// `look_at` the view matrix is built afresh from the eye, up vector and interest, otherwise
/// the camera's own (rebuilt if dirty) is used. A point behind the eye is first pulled just in
/// front of it. `None` for a frustum projection; a coordinate past ±50000 panics, as the
/// original asserts.
#[doc(alias = "lbVector_WorldToScreen")]
pub fn world_to_screen(
    objs: &mut Objects,
    data: &Archives,
    c: CObjId,
    pos: Vec3,
    look_at: bool,
) -> Option<Vec3> {
    let lim = reg(50000.0);
    for (v, line) in [(pos.x, 677), (pos.y, 678), (pos.z, 679)] {
        let v = reg(v);
        if !(v > fp::fneg(lim) && v < lim) {
            panic!("lbvector.c:{line}: a point past 50000 units");
        }
    }
    let mut point = pos;
    let o = &objs.cobjs[c];
    let pm: [f32; 7] = match o.projection_type {
        cobj::PERSPECTIVE => {
            let [fov, aspect, ..] = o.projection;
            let p = Mtx44::perspective(fov, aspect, o.near, o.far).m;
            [0.0, p[0][0], p[0][2], p[1][1], p[1][2], p[2][2], p[2][3]].map(copy)
        }
        cobj::ORTHO => {
            let [t, b, l, r] = o.projection;
            let p = Mtx44::ortho(t, b, l, r, o.near, o.far).m;
            [1.0, p[0][0], p[0][3], p[1][1], p[1][3], p[2][2], p[2][3]].map(copy)
        }
        _ => return None,
    };
    let v = o.viewport;
    let vp = [
        copy(v[0]),
        copy(v[2]),
        mem(fp::fsubs(reg(v[1]), reg(v[0]))),
        mem(fp::fsubs(reg(v[3]), reg(v[2]))),
        0.0,
        1.0,
    ];
    let m = if look_at {
        let eye = cobj::eye_position(objs, data, c).expect("the camera's eye");
        let up = cobj::up_vector(objs, data, Some(c)).expect("the camera's up vector");
        let target = cobj::interest(objs, data, c).expect("the camera's interest");
        Mtx34::look_at(eye, up, target)
    } else {
        cobj::viewing_mtx(objs, data, c)
    };
    let r = &m.m[2];
    let depth = fp::fadds(
        reg(r[3]),
        fp::fmadds(
            reg(r[2]),
            reg(pos.z),
            fp::fmadds(reg(r[0]), reg(pos.x), fp::fmuls(reg(r[1]), reg(pos.y))),
        ),
    );
    let near = f64::from(0.01_f32);
    if depth > fp::fneg(near) {
        let f = fp::fsubs(fp::fneg(depth), near);
        point.x = mem(fp::fmadds(reg(r[0]), f, reg(point.x)));
        point.y = mem(fp::fmadds(reg(r[1]), f, reg(point.y)));
        point.z = mem(fp::fmadds(reg(r[2]), f, reg(point.z)));
    }
    Some(gx_project(
        [reg(point.x), reg(point.y), reg(point.z)],
        &m,
        &pm,
        &vp,
    ))
}
