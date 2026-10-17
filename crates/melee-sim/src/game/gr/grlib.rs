//! Shared stage helpers, `melee/gr/grlib.c`.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::super::camera;
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_JOBJ, Obj, Running};
use super::ground::ground_of;
use crate::fpu::{mem, reg};
use crate::hsd::aobj;
use crate::hsd::generator::{self, GeneratorId};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};

/// Measures the players' start points (points of interest 0 to 3), `grLib_801C9A10`: a point
/// the stage lacks keeps its last measure.
#[doc(alias = "grLib_801C9A10")]
pub fn measure_start_points(game: &mut Game, data: &Archives) {
    for i in 0..4 {
        if let Some(p) = super::ground::point(game, data, i) {
            game.gr.start_points[i as usize] = p;
        }
    }
}

/// A player's start point as last measured, `grLib_801C9A70`.
#[doc(alias = "grLib_801C9A70")]
pub fn start_point(game: &Game, i: i32) -> crate::vec::Vec3 {
    assert!((0..4).contains(&i), "grlib.c: a start point");
    game.gr.start_points[i as usize]
}

/// The wind a scrolling stage blows on an airborne item, `grLib_801C9E60`: whether the stage
/// has one (Rainbow Cruise, Big Blue, Icicle Mountain, not ported), else none.
#[doc(alias = "grLib_801C9E60")]
pub fn item_wind(game: &Game, v: &mut crate::vec::Vec3) -> bool {
    match game.stage_info.grkind {
        0x03 => crate::game::code::unported_call("grRCruise_80201918 (Rainbow Cruise's wind)"),
        0x13 => crate::game::code::unported_call("grBigBlue_801EF7D8 (Big Blue's wind)"),
        0x16 => crate::game::code::unported_call("grIceMt_801FA728 (Icicle Mountain's wind)"),
        _ => {}
    }
    *v = crate::vec::Vec3::new(0.0, 0.0, 0.0);
    false
}

/// Whether a stage object's color overlay has ended (`x10_flags.b4`), `grLib_801C96E8`.
#[doc(alias = "grLib_801C96E8")]
pub fn overlay_done(game: &Game, gobj: GObjId) -> bool {
    game.grounds[ground_of(game, gobj).0].x10_flags.b4
}

/// Removes the particle generators bound to a joint tree, `grLib_801C9854` (`hsd_8039D5DC`).
#[doc(alias = "grLib_801C9854")]
pub fn kill_joint_generators(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    j: Option<crate::hsd::jobj::JObjId>,
) {
    let mut hooks = crate::game::code::ParticleHooks;
    crate::hsd::generator::kill_on_tree(&mut game.objs, data, &mut hooks, j);
}

/// A generator of the stage's effect `idx` from bank `bank` at `pos`, at the stage's scale,
/// `grLib_801C96F8`.
#[doc(alias = "grLib_801C96F8")]
pub fn generator_at(
    game: &mut Game,
    data: &Archives,
    idx: i32,
    bank: i32,
    pos: crate::vec::Vec3,
) -> Option<GeneratorId> {
    let mut hooks = crate::game::code::ParticleHooks;
    let objs = &mut game.objs;
    let g = generator::spawn(objs, data, &mut hooks, 0, bank, idx)?;
    let s = match objs.generators[g].appsrt {
        Some(s) => {
            objs.app_srts[s].xa2 = 0;
            s
        }
        None => crate::hsd::appsrt::add_to_generator(objs, g, 1),
    };
    objs.app_srts[s].translate = pos;
    let k = reg(super::ground::scale(game));
    let srt = &mut game.objs.app_srts[s];
    srt.scale.x = mem(fp::fmuls(reg(srt.scale.x), k));
    srt.scale.y = mem(fp::fmuls(reg(srt.scale.y), k));
    srt.scale.z = mem(fp::fmuls(reg(srt.scale.z), k));
    let o = &mut game.objs.generators[g];
    o.pos.z = 0.0;
    o.pos.y = 0.0;
    o.pos.x = 0.0;
    Some(g)
}

/// A generator following a joint that turns into its frame first, `grLib_801C97DC`.
#[doc(alias = "grLib_801C97DC")]
pub fn generator_oriented_on(
    game: &mut Game,
    data: &Archives,
    idx: i32,
    bank: i32,
    j: Option<JObjId>,
) {
    let mut hooks = crate::game::code::ParticleHooks;
    generator::spawn_oriented_on_jobj(&mut game.objs, data, &mut hooks, 0, bank, idx, j);
}

/// A generator following a joint, `grLib_801C9808`.
#[doc(alias = "grLib_801C9808")]
pub fn generator_on(
    game: &mut Game,
    data: &Archives,
    idx: i32,
    bank: i32,
    j: Option<JObjId>,
) -> Option<GeneratorId> {
    let mut hooks = crate::game::code::ParticleHooks;
    generator::spawn_on_jobj(&mut game.objs, data, &mut hooks, 0, bank, idx, j)
}

/// Removes a generator, `grLib_801C9834`.
#[doc(alias = "grLib_801C9834")]
pub fn kill_generator(game: &mut Game, data: &Archives, g: GeneratorId) {
    let mut hooks = crate::game::code::ParticleHooks;
    generator::kill(&mut game.objs, data, &mut hooks, g);
}

/// Removes a generator with its particles, `grLib_801C9874`.
#[doc(alias = "grLib_801C9874")]
pub fn kill_generator_and_particles(game: &mut Game, data: &Archives, g: GeneratorId) {
    game.objs.generators[g].ty |= generator::KILL_PARTICLES;
    kill_generator(game, data, g);
}

/// Removes the generators following a joint, with their particles, `grLib_801C98A0`.
#[doc(alias = "grLib_801C98A0")]
pub fn kill_joint_generators_and_particles(game: &mut Game, data: &Archives, j: Option<JObjId>) {
    let Some(j) = j else { return };
    let mut cur = game.objs.generators.head;
    while let Some(g) = cur {
        cur = game.objs.generators[g].next;
        if game.objs.generators[g].jobj == Some(j) {
            kill_generator_and_particles(game, data, g);
        }
    }
}

/// [`kill_joint_generators_and_particles`] over a joint's tree, `grLib_801C9908`.
#[doc(alias = "grLib_801C9908")]
pub fn kill_tree_generators_and_particles(game: &mut Game, data: &Archives, j: Option<JObjId>) {
    let Some(j) = j else { return };
    kill_joint_generators_and_particles(game, data, Some(j));
    if game.objs.jobjs[j].flags & crate::hsd::jobj::INSTANCE != 0 {
        return;
    }
    let mut c = game.objs.jobjs[j].child;
    while let Some(ch) = c {
        kill_tree_generators_and_particles(game, data, Some(ch));
        c = game.objs.jobjs[ch].next;
    }
}

/// A joint's particles from the stage's bank, from efLib's joint callbacks: plainly following
/// the joint for an animated event (`dynamic`), else oriented by it, `grLib_801C99C0`.
#[doc(alias = "grLib_801C99C0")]
pub fn joint_particles(
    objs: &mut crate::hsd::Objects,
    data: &Archives,
    hooks: &mut dyn crate::hsd::particle::Hooks,
    bank: i32,
    idx: i32,
    j: JObjId,
    dynamic: bool,
) {
    if dynamic {
        generator::spawn_on_jobj(objs, data, hooks, 0, bank, idx, Some(j));
    } else {
        generator::spawn_oriented_on_jobj(objs, data, hooks, 0, bank, idx, Some(j));
    }
}

/// The looping quake model's process, `grLib_801C9BC8`.
pub const QUAKE_LOOP_PROC: u32 = 0x801C_9BC8;
/// The other quake models' process, `grLib_801C9C40`.
pub const QUAKE_PROC: u32 = 0x801C_9C40;

/// Starts the stage's quake model for a quake (`CmQuakeKind` 1 to 4), whose animation shakes
/// the camera: none for another kind or a stage without one, `grLib_801C9CEC`.
#[doc(alias = "grLib_801C9CEC")]
pub fn quake_model(r: &mut Running, kind: i32) -> Option<GObjId> {
    if !(1..=4).contains(&kind) {
        return None;
    }
    let idx = (kind - 1) as u32;
    let set = r.game.stage_info.quake_model_set?;
    let gobj = r.game.world.create(3, 18, kind as u8);
    let j = jobj::load_joint(&mut r.game.objs, r.data, r.data.ptr(set))
        .expect("grLib_801C9CEC: the quake model's joint");
    r.game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
    let proc = if kind == camera::QUAKE_LOOP {
        QUAKE_LOOP_PROC
    } else {
        QUAKE_PROC
    };
    r.game.world.setup_proc(gobj, CodeRef(proc), 1);
    let anim = r
        .data
        .ptr(set.at(4))
        .and_then(|l| r.data.ptr(l.at(4 * idx)));
    jobj::add_anim_all(&mut r.game.objs, r.data, Some(j), anim, None, None);
    jobj::req_anim_all(&mut r.game.objs, Some(j), 0.0);
    if kind == camera::QUAKE_LOOP {
        jobj::foreach_anim(
            &mut r.game.objs,
            j,
            jobj::type_mask::JOBJ,
            &mut |objs, a| aobj::set_flags(&mut objs.aobjs, Some(a), aobj::LOOP),
        );
    }
    Some(gobj)
}

/// A quake model's process: animates the model and shakes the camera by its root's x and y,
/// then frees a model whose animation has ended unless it loops, `grLib_801C9BC8` and
/// `grLib_801C9C40`.
#[doc(alias = "grLib_801C9BC8")]
#[doc(alias = "grLib_801C9C40")]
pub fn quake_proc(r: &mut Running, gobj: GObjId, looping: bool) {
    let j = super::ground::jobj_of(r.game, gobj).expect("a quake model's joint");
    let a = r.game.objs.jobjs[j].aobj;
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        Some(j),
    );
    let t = r.game.objs.jobjs[j].translate;
    camera::set_quake_offset(&mut r.game.camera.cam, f64::from(t.x), f64::from(t.y));
    if !looping && a.is_none_or(|a| r.game.objs.aobjs[a].flags & aobj::NO_ANIM != 0) {
        crate::hsd::gobj::free(r, gobj);
    }
}
