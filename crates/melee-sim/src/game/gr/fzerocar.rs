//! The F-Zero cars, `melee/gr/grfzerocar.c`: Mute City's and Big Blue's thirty cars, each a
//! model of the stage's under a joint of its own that the stage moves, with its pilots.

use gekko_fp as fp;

use super::super::state::Running;
use super::datfiles;
use super::ground::{jobj_of, load_map_joint, scale};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `grFZeroCar_803E0BD8`: for each car, the joints (steps into its model, depth first) its
/// pilots sit on, -1 for none.
const PILOTS: [[i16; 4]; 30] = [
    [1, 3, -1, -1],
    [1, 3, 2, 4],
    [1, 2, 3, -1],
    [1, 2, -1, -1],
    [1, 3, 2, 4],
    [1, -1, -1, -1],
    [1, 2, -1, -1],
    [1, 2, -1, -1],
    [1, 3, 2, 4],
    [1, 2, -1, -1],
    [1, 2, -1, -1],
    [1, -1, -1, -1],
    [1, 2, -1, -1],
    [1, -1, -1, -1],
    [1, 2, -1, -1],
    [1, 2, 3, -1],
    [1, 2, -1, -1],
    [1, 2, -1, -1],
    [2, 3, 1, -1],
    [1, 2, -1, -1],
    [1, 3, 2, 4],
    [1, 2, 3, -1],
    [1, -1, -1, -1],
    [1, 2, 3, -1],
    [1, 3, 2, 4],
    [1, 2, 3, -1],
    [1, 2, 3, -1],
    [1, -1, -1, -1],
    [1, -1, -1, -1],
    [1, 3, 2, 4],
];

/// `HSD_ForeachAnim`'s object types for the pilots' animations' rate (`0x76A4`).
const PILOT_ANIM_TYPES: u32 = 0x76A4;

/// A joint that only places its child (`HSD_JObjAlloc` with an identity matrix and no
/// scale vector).
fn placing_joint(r: &mut Running) -> JObjId {
    let j = jobj::alloc(&mut r.game.objs);
    let o = &mut r.game.objs.jobjs[j];
    o.mtx = Mtx34::IDENTITY;
    o.scl = None;
    j
}

/// The joint `steps` into a car's model, depth first (`Ground_801C4100` that many times),
/// or none.
fn pilot_seat(r: &Running, model: JObjId, steps: i16) -> Option<JObjId> {
    if steps == -1 {
        return None;
    }
    let mut j = Some(model);
    let mut n = steps;
    while let Some(c) = j {
        if n == 0 {
            break;
        }
        n -= 1;
        j = super::ground::next_in_tree(&r.game.objs.jobjs, c);
    }
    j
}

/// A pilot on its seat (the inline `setup_car_child`): map `map_id`'s model, its animations
/// from the map's entry started at full rate, and the seat scaled by `k` sideways and up.
fn seat_pilot(r: &mut Running, seat: JObjId, map_id: i32, k: f32) {
    let head = datfiles::archive_for(r.game, r.data, map_id);
    let head = head.expect("grfzerocar.c: 95: archive");
    let pilot = load_map_joint(r.game, r.data, map_id, 0);
    if let Some(p) = pilot {
        let entry = head.entry(r.data, map_id);
        let first = |d: Option<melee_data::archive::DataRef>| d.and_then(|d| r.data.ptr(d));
        let anims = entry.anims(r.data, 0);
        let matanims = entry.anims(r.data, 1);
        if anims.is_some() && matanims.is_some() {
            let (a, m) = (first(anims), first(matanims));
            super::anime::add_tree_anims(&mut r.game.objs, r.data, Some(p), a, m, None);
            jobj::req_anim_all_by_flags(&mut r.game.objs, Some(p), 0x497, 0.0);
            jobj::foreach_anim(&mut r.game.objs, p, PILOT_ANIM_TYPES, &mut |objs, a| {
                crate::hsd::aobj::set_rate(&mut objs.aobjs, Some(a), 1.0)
            });
        }
        jobj::add_child(&mut r.game.objs.jobjs, Some(seat), Some(p));
    }
    jobj::set_srt(&mut r.game.objs.jobjs, seat, |o| o.scale.x = k);
    jobj::set_srt(&mut r.game.objs.jobjs, seat, |o| o.scale.y = k);
}

/// Builds the cars under a stage object's joint, `grFZeroCar_801CAFBC`: the object's own model
/// is the first car, `maps[i]`'s the others (each under a placing joint of its own, at the
/// stage's scale), every model at the origin, and each car's pilots (map `pilot_map`'s
/// model, scaled by how many sit in it). `set_apart` sets each model's display objects apart
/// (`lb_8000F9F8`).
#[doc(alias = "grFZeroCar_801CAFBC")]
pub fn build(r: &mut Running, gobj: GObjId, maps: &[i16; 30], pilot_map: i32, set_apart: bool) {
    let root = jobj_of(r.game, gobj).expect("grfzerocar.c: the object's joint");
    let s = reg(scale(r.game));
    jobj::set_scale(&mut r.game.objs.jobjs, root, Vec3::new(1.0, 1.0, 1.0));
    let mut model = r.game.objs.jobjs[root].child;
    if set_apart {
        super::setup::set_apart(r.game, model.expect("grfzerocar.c: a model"));
    }
    let mut place = placing_joint(r);
    jobj::add_next(&mut r.game.objs.jobjs, model, Some(place));
    for (i, seats) in PILOTS.iter().enumerate() {
        if i != 0 {
            model = load_map_joint(r.game, r.data, i32::from(maps[i]), 0);
            let Some(m) = model else {
                panic!("grfzerocar.c: 195: 0");
            };
            place = placing_joint(r);
            jobj::add_next(&mut r.game.objs.jobjs, Some(m), Some(place));
            jobj::add_child(&mut r.game.objs.jobjs, Some(root), Some(place));
            if set_apart {
                super::setup::set_apart(r.game, m);
            }
        }
        let m = model.expect("grfzerocar.c: a model");
        jobj::set_translate(&mut r.game.objs.jobjs, m, Vec3::new(0.0, 0.0, 0.0));
        jobj::set_rotation(
            &mut r.game.objs.jobjs,
            m,
            crate::quat::Quat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        );
        let ps = r.game.objs.jobjs[place].scale;
        let scaled = Vec3::new(
            mem(fp::fmuls(reg(ps.x), s)),
            mem(fp::fmuls(reg(ps.y), s)),
            mem(fp::fmuls(reg(ps.z), s)),
        );
        jobj::set_scale(&mut r.game.objs.jobjs, place, scaled);
        let n = seats.iter().filter(|&&k| k != -1).count();
        let k = match n {
            1 => 1.1,
            4 => 0.9,
            _ => 1.0,
        };
        let found = seats.map(|steps| pilot_seat(r, m, steps));
        for seat in found.into_iter().flatten() {
            seat_pilot(r, seat, pilot_map, k);
        }
    }
}
