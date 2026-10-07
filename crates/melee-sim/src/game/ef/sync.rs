//! efSync (`melee/ef/efsync.c`): spawning an effect by id, and the cases for ids 0x4BB on
//! (characters' effects from Kirby's on).

use gekko_fp as fp;

use super::super::code::ParticleHooks;
use super::super::state::Running;
use super::eflib::{self, drain_anim_queue};
use super::{Arg, Args, EffectId, LOAD_UNCOUNTED, Spawned, Update};
use crate::fpu::{copy, mem, reg};
use crate::hsd::generator::{self, GeneratorId};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;

/// `M_TAU` as a double.
pub(super) const TAU: f64 = std::f64::consts::TAU;
/// `M_PI_2` as a single.
pub(super) const HALF_PI: f32 = 1.570_796_4;

/// `±M_PI_2` for a facing, as a float.
pub(super) fn facing_rot(f: f32) -> f32 {
    if reg(f) < 0.0 { -HALF_PI } else { HALF_PI }
}

/// `M_TAU * HSD_Randf()` as a float.
pub(super) fn random_turn(r: &mut Running) -> f32 {
    let rnd = reg(r.game.objs.rng.draw_f32());
    mem(fp::frsp(fp::fmul(TAU, rnd)))
}

/// An effect's joint.
pub(super) fn ej(r: &Running, e: EffectId) -> JObjId {
    let g = r.game.ef.effects[e.0].gobj.expect("an effect's GObj");
    super::super::gr::ground::jobj_of(r.game, g).expect("jobj")
}

pub(super) fn set_rot_y(r: &mut Running, e: EffectId, v: f32) {
    let j = ej(r, e);
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, v);
}

pub(super) fn set_rot_z(r: &mut Running, e: EffectId, v: f32) {
    let j = ej(r, e);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, v);
}

pub(super) fn set_scale(r: &mut Running, e: EffectId, s: Vec3) {
    let j = ej(r, e);
    jobj::set_scale(&mut r.game.objs.jobjs, j, s);
}

/// A GObj's joint's scale (`HSD_JObjGetScale`).
pub(super) fn gobj_scale(r: &Running, gobj: Option<GObjId>) -> Vec3 {
    let j = super::super::gr::ground::jobj_of(r.game, gobj.expect("a spawner"))
        .expect("HSD_JObjGetScale: jobj");
    r.game.objs.jobjs[j].scale
}

/// A generator following a joint (`hsd_8039EFAC(0, bank, idx, jobj)`).
pub(super) fn gen_jobj(r: &mut Running, bank: i32, idx: i32, j: Option<JObjId>) -> Spawned {
    Spawned::from_generator(generator::spawn_on_jobj(
        &mut r.game.objs,
        r.data,
        &mut ParticleHooks,
        0,
        bank,
        idx,
        j,
    ))
}

/// `efLib_CreateGenerator`.
pub(super) fn gen_at(r: &mut Running, gfx_id: i32, pos: Vec3) -> Option<GeneratorId> {
    eflib::create_generator(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, pos)
}

/// `efLib_CreateGenerator_AddAppSRT`.
pub(super) fn gen_appsrt(r: &mut Running, gfx_id: i32) -> Option<GeneratorId> {
    eflib::create_generator_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id)
}

/// A generator's app-SRT.
pub(super) fn srt<'r>(r: &'r mut Running, g: GeneratorId) -> &'r mut crate::hsd::appsrt::AppSrt {
    let s = r.game.objs.generators[g].appsrt.expect("an app-SRT");
    &mut r.game.objs.app_srts[s]
}

/// Sets all three of an app-SRT's scales from a float register.
pub(super) fn srt_scale(r: &mut Running, g: GeneratorId, v: f32) {
    let v = copy(v);
    let s = srt(r, g);
    s.scale.x = v;
    s.scale.y = v;
    s.scale.z = v;
}

/// Spawns effect `gfx_id` for `gobj`, `efSync_Spawn`: a plain common generator below 0x250 and
/// for bank 30 (the stage's), efAsync's table up to 0x477, efAlt's up to 0x4B9, and this
/// table's cases from 0x4BB on (uncounted). Effect 0x479 is common generator 0x506 while
/// Mario's file isn't loaded. Joints effects made with animations animate once before it
/// returns.
#[doc(alias = "efSync_Spawn")]
pub fn spawn(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, args: &[Arg]) -> Spawned {
    spawn_with(r, gfx_id, gobj, &mut Args::new(args))
}

/// [`spawn`] with arguments from wherever `va` reads them.
pub fn spawn_with(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, va: &mut Args) -> Spawned {
    r.game.ef.load_kind = 0;
    r.game.ef.anim_count = 0;
    let mut gfx_id = gfx_id;
    if gfx_id == 0x479 && r.game.ef.tables[1].is_none() {
        gfx_id = 0x506;
    }
    if gfx_id < 0x250 || gfx_id / 1000 == 0x1E {
        let pos = va.vec();
        return Spawned::from_generator(gen_at(r, gfx_id, pos));
    }
    if gfx_id < 0x478 {
        return super::dispatch(r, gfx_id, gobj, va);
    }
    if gfx_id < 0x4BA {
        return super::spawn_alt(r, gfx_id, gobj, va);
    }
    r.game.ef.load_kind = LOAD_UNCOUNTED;
    let ret = cases(r, gfx_id, gobj, va);
    drain_anim_queue(r);
    ret
}

/// Two effects in one spawn, the second the first's `next`.
fn pair(
    r: &mut Running,
    first: Option<EffectId>,
    second: impl FnOnce(&mut Running) -> Option<EffectId>,
) -> Spawned {
    if let Some(e) = first {
        let n = second(r);
        r.game.ef.effects[e.0].next = n;
    }
    Spawned::from_effect(first)
}

fn cases(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, va: &mut Args) -> Spawned {
    use eflib::{
        create, create_attach, create_attach_child, create_attach_child_scale, create_attach_pos,
        create_attach_scale, create_attach_scale_facing, create_generator_appsrt_facing,
        create_generator_attach as gen_attach, create_generator_attach_appsrt as gen_attach_appsrt,
        create_generator_attach_scale as gen_attach_scale,
    };
    let ret_e = Spawned::from_effect;
    let ret_g = Spawned::from_generator;
    // An effect at the spawner's facing in z from the next float.
    let facing_z = |r: &mut Running, e: Option<EffectId>, va: &mut Args| {
        if let Some(e) = e {
            let z = copy(va.float());
            set_rot_z(r, e, z);
        }
        ret_e(e)
    };
    // An effect turned to the facing the next float gives.
    let facing_y = |r: &mut Running, e: Option<EffectId>, va: &mut Args| {
        if let Some(e) = e {
            let f = va.float();
            set_rot_y(r, e, facing_rot(f));
        }
        ret_e(e)
    };
    // A generator with an app-SRT at the next vector, at the spawner's y scale.
    let gen_scaled = |r: &mut Running, id: i32, va: &mut Args, rot_y: bool| {
        let g = gen_appsrt(r, id);
        if let Some(g) = g {
            let t = va.vec();
            srt(r, g).translate = t;
            let s = gobj_scale(r, gobj);
            srt_scale(r, g, s.y);
            if rot_y {
                srt(r, g).rot.y = HALF_PI;
            }
        }
        ret_g(g)
    };
    match gfx_id {
        0x4BB | 0x4BC => {
            let (a, b) = if gfx_id == 0x4BB {
                (0x1770, 0x1772)
            } else {
                (0x1771, 0x1773)
            };
            let j = va.jobj();
            let e = create_attach_scale_facing(r, a, gobj, j);
            pair(r, e, |r| create_attach_child_scale(r, b, gobj, va.jobj()))
        }
        0x4BD => gen_jobj(r, 7, 0x1B58, va.jobj()),
        0x4BE => gen_jobj(r, 7, 0x1B5C, va.jobj()),
        0x4BF => {
            let j = va.jobj();
            let ret = gen_jobj(r, 7, 0x1B5D, j);
            gen_jobj(r, 0, 0x5F, j);
            ret
        }
        0x4C0 => ret_e(create_attach(r, 0x1B58, gobj, va.jobj())),
        0x4C1 | 0x4C2 | 0x4C4 | 0x4C5 => {
            let id = match gfx_id {
                0x4C1 => 0x1B59,
                0x4C2 => 0x1B5A,
                0x4C4 => 0x1B5B,
                _ => 0x1B5C,
            };
            let e = create_attach_scale_facing(r, id, gobj, va.jobj());
            facing_z(r, e, va)
        }
        0x4C3 => {
            let g = gen_appsrt(r, 0x24C);
            if let Some(g) = g {
                let t = va.vec();
                srt(r, g).translate = t;
            }
            ret_g(g)
        }
        0x4C6 => ret_e(create_attach_scale_facing(r, 0x1F40, gobj, va.jobj())),
        0x4C7 => ret_e(create_attach_scale_facing(r, 0x1F41, gobj, va.jobj())),
        0x4C8 | 0x4C9 => {
            let id = if gfx_id == 0x4C8 { 0x1F42 } else { 0x1F43 };
            let e = create_attach_scale(r, id, gobj, va.jobj());
            if let Some(e) = e {
                let f = va.float();
                set_rot_y(r, e, facing_rot(f));
                let j = ej(r, e);
                jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, Some(j));
            }
            ret_e(e)
        }
        0x4CA => ret_e(create_attach_scale(r, 0x1F44, gobj, va.jobj())),
        0x4CB => ret_e(create_attach_scale(r, 0x1F45, gobj, va.jobj())),
        0x4CC => {
            let e = create_attach_pos(r, 0x1F46, gobj, va.vec());
            if let Some(e) = e {
                let s = gobj_scale(r, gobj);
                set_scale(r, e, s);
                r.game.ef.effects[e.0].update = Some(Update::SetRotYFromFighterDir);
            }
            ret_e(e)
        }
        0x4CD => ret_e(create_attach(r, 0x2328, gobj, va.jobj())),
        0x4CE => {
            let pos = va.vec();
            ret_g(gen_at(r, 0x2328, pos))
        }
        0x4CF | 0x4D0 => {
            let t = va.vec();
            let scale = if gfx_id == 0x4CF {
                copy(va.float())
            } else {
                1.0
            };
            let mut first = None;
            let mut last: Option<EffectId> = None;
            for i in 0..12 {
                let low = reg(r.game.objs.rng.draw_f32()) < 0.5;
                let id = match (low, gfx_id == 0x4D0) {
                    (true, true) => 0x1C,
                    (true, false) => 0x1E,
                    (false, true) => 0x1D,
                    (false, false) => 0x1F,
                };
                let Some(e) = create_attach_pos(r, id, gobj, t) else {
                    break;
                };
                let ef = &mut r.game.ef.effects[e.0];
                ef.update = Some(Update::SetOffsetFromParams);
                ef.lifetime = 0x32;
                let ry = random_turn(r);
                let rx = random_turn(r);
                let j = ej(r, e);
                let jobjs = &mut r.game.objs.jobjs;
                jobj::set_srt(jobjs, j, |o| o.scale.x = scale);
                jobj::set_srt(jobjs, j, |o| o.scale.y = scale);
                jobj::set_srt(jobjs, j, |o| o.scale.z = scale);
                jobj::set_rotation_y(jobjs, j, ry);
                jobj::set_rotation_x(jobjs, j, rx);
                let two = |v: f32| fp::fmuls(2.0, reg(v));
                let sin_ry = reg(math::sin(ry));
                let ef = &mut r.game.ef.effects[e.0];
                ef.params.x = mem(fp::fmuls(two(math::cos(rx)), sin_ry));
                ef.params.y = mem(two(math::sin(rx)));
                let cos_ry = reg(math::cos(ry));
                ef.params.z = mem(fp::fmuls(two(math::cos(rx)), cos_ry));
                match last {
                    Some(l) if i != 0 => r.game.ef.effects[l.0].next = Some(e),
                    _ => first = Some(e),
                }
                last = Some(e);
            }
            ret_e(first)
        }
        0x4EE => ret_e(create_attach(r, 0x2710, gobj, va.jobj())),
        0x4EF => ret_e(create_attach(r, 0x2711, gobj, va.jobj())),
        0x4F0 => ret_e(create_attach_scale(r, 0x2712, gobj, va.jobj())),
        0x4D5 => gen_jobj(r, 0xB, 0x2AF8, va.jobj()),
        0x4D6 => ret_e(create_attach_scale(r, 0x2AF8, gobj, va.jobj())),
        0x4D7 => gen_jobj(r, 0xB, 0x2AF9, va.jobj()),
        0x4D8 => gen_scaled(r, 0x61, va, true),
        0x4D9 => {
            let e = create_attach_pos(r, 0x2EE0, gobj, va.vec());
            if let Some(e) = e {
                let f = va.float();
                set_rot_y(r, e, facing_rot(f));
                let s = gobj_scale(r, gobj);
                set_scale(r, e, s);
            }
            ret_e(e)
        }
        0x4DA => {
            let j = va.jobj();
            let e = create_attach_scale(r, 0x2EE1, gobj, j);
            if let Some(e) = e {
                let n = create_attach_scale(r, 0x2EE2, gobj, j);
                r.game.ef.effects[e.0].next = n;
                if n.is_some() {
                    r.game.ef.effects[e.0].update = Some(Update::KoopaSpecialHi);
                }
            }
            ret_e(e)
        }
        0x4DB => ret_g(gen_attach_scale(r, 0x2EE5, va, gobj)),
        0x4DC => ret_g(gen_attach_scale(r, 0x2EE6, va, gobj)),
        0x4DD => ret_g(gen_attach_scale(r, 0x2EE7, va, gobj)),
        0x4DE => ret_g(gen_attach_scale(r, 0x2EE8, va, gobj)),
        0x4DF => ret_g(gen_attach_scale(r, 0x143, va, gobj)),
        0x4E0 => ret_e(create_attach(r, 0x32C8, gobj, va.jobj())),
        0x4E1 => {
            let e = create(r, 0x32C9, gobj);
            if let Some(e) = e {
                let j = va.jobj();
                let ef = &mut r.game.ef.effects[e.0];
                ef.attach_jobj = j;
                ef.update = Some(Update::SetOffsetYFromParamY);
                let mut root = j.expect("HSD_JObjGetParent: jobj");
                while let Some(p) = r.game.objs.jobjs[root].parent {
                    root = p;
                }
                let sy = r.game.objs.jobjs[root].scale.y;
                let p = va.vec();
                let ef = &mut r.game.ef.effects[e.0];
                ef.params = p;
                ef.params.y = mem(fp::fmuls(reg(ef.params.y), reg(sy)));
            }
            ret_e(e)
        }
        0x4E2 => ret_e(create_attach_pos(r, 0x32CA, gobj, va.vec())),
        0x4E3 | 0x4E4 | 0x4E7 => {
            let id = match gfx_id {
                0x4E3 => 0x18A,
                0x4E4 => 0x194,
                _ => 0x196,
            };
            let pos = va.vec();
            ret_g(gen_at(r, id, pos))
        }
        0x4E5 => gen_jobj(r, 0, 0x17D, va.jobj()),
        0x4E6 => gen_jobj(r, 0, 0x17E, va.jobj()),
        0x4E8 => ret_e(create_attach_pos(r, 0x32CB, gobj, va.vec())),
        0x4E9 => gen_jobj(r, 0xE, 0x36B0, va.jobj()),
        0x4EA => gen_jobj(r, 0xE, 0x36B1, va.jobj()),
        0x4EB => ret_g(create_generator_appsrt_facing(r, 0x36B6, va)),
        0x4EC => gen_jobj(r, 0xE, 0x36B7, va.jobj()),
        0x4ED => ret_e(create_attach_child(r, 0x36B0, gobj, va.jobj())),
        0x4D1 => gen_jobj(r, 0, 0x64, va.jobj()),
        0x4D2 => ret_e(create_attach_pos(r, 0x3A98, gobj, va.vec())),
        0x4D3 => {
            let j = va.jobj();
            let g = gen_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, 0x172, j);
            if g.is_some() {
                gen_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, 0x173, j);
            }
            ret_g(g)
        }
        0x4D4 => {
            let j = va.jobj();
            ret_g(gen_attach(
                &mut r.game.objs,
                r.data,
                &mut ParticleHooks,
                0x11E,
                j,
            ))
        }
        0x4F1 => ret_g(create_generator_appsrt_facing(r, 0x3E80, va)),
        0x4F2 => ret_e(create_attach_scale_facing(r, 0x3E80, gobj, va.jobj())),
        0x4F3 => ret_e(create_attach_scale_facing(r, 0x3E81, gobj, va.jobj())),
        0x4F4 | 0x4F5 => {
            let id = if gfx_id == 0x4F4 { 0x4268 } else { 0x4269 };
            let e = create_attach_child(r, id, gobj, va.jobj());
            if let Some(e) = e {
                let s = gobj_scale(r, gobj);
                set_scale(r, e, s);
            }
            ret_e(e)
        }
        0x4F6 => ret_e(create_attach_scale(r, 0x426A, gobj, va.jobj())),
        0x4F7 => ret_e(create_attach_scale(r, 0x426B, gobj, va.jobj())),
        0x4F8 => ret_g(gen_attach_scale(r, 0x6E, va, gobj)),
        0x4F9 => ret_g(gen_attach_scale(r, 0x1C8, va, gobj)),
        0x4FA => ret_g(gen_attach_scale(r, 0x166, va, gobj)),
        0x4FB => gen_jobj(r, 0, 0x71, va.jobj()),
        0x4FE => ret_e(create_attach(r, 0x426C, gobj, va.jobj())),
        0x4FF | 0x500 => {
            let j = fighter_part(r, gobj, 1);
            ret_e(create_attach_scale(r, 0x426D, gobj, j))
        }
        0x501 => {
            let j = fighter_part(r, gobj, 1);
            let e = create_attach_scale(r, 0x426E, gobj, j);
            if let Some(e) = e {
                let facing = r.game.fighter(gobj.expect("a fighter")).facing_dir;
                set_rot_y(r, e, facing_rot(facing));
                let attach = fighter_part(r, gobj, 85);
                let ef = &mut r.game.ef.effects[e.0];
                ef.attach_jobj = attach;
                ef.update = Some(Update::LifetimeEndSpawn);
                ef.lifetime = 6;
            }
            ret_e(e)
        }
        0x502 => gen_jobj(r, 0, 0x1A6, va.jobj()),
        0x4FC => gen_jobj(r, 0x11, 0x426D, va.jobj()),
        0x4FD => gen_jobj(r, 0x11, 0x4271, va.jobj()),
        0x503 => {
            let pos = va.vec();
            ret_g(gen_at(r, 0x6A, pos))
        }
        0x504 => gen_scaled(r, 0x6D, va, false),
        0x505 => gen_scaled(r, 0x79, va, false),
        0x506 => {
            let pos = va.vec();
            ret_g(gen_at(r, 0x4658, pos))
        }
        0x507 => {
            let j = va.jobj();
            let e = create_attach(r, 0x4650, gobj, j);
            if let Some(e) = e {
                let f = va.float();
                set_rot_y(r, e, facing_rot(f));
            }
            gen_jobj(r, 0x12, 0x4650, j);
            ret_e(e)
        }
        0x508 => {
            let pos = va.vec();
            ret_g(gen_at(r, 0x4652, pos))
        }
        0x509 => {
            let e = create_attach_scale(r, 0x4651, gobj, va.jobj());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::LuigiSpecialLw);
            }
            ret_e(e)
        }
        0x50A => gen_jobj(r, 0, 0x5F, va.jobj()),
        0x50B => {
            let e = create_attach_scale_facing(r, 0x4A38, gobj, va.jobj());
            pair(r, e, |r| {
                create_attach_scale_facing(r, 0x4A39, gobj, va.jobj())
            })
        }
        0x50C => {
            let e = create_attach(r, 0x4A3A, gobj, va.jobj());
            if let Some(e) = e {
                let z = copy(va.float());
                let ef = &mut r.game.ef.effects[e.0];
                ef.params.z = z;
                ef.update = Some(Update::SetRotYZFromParamZFighterDir);
            }
            ret_e(e)
        }
        0x50D => ret_e(create_attach(r, 0x4A3B, gobj, va.jobj())),
        0x50E | 0x50F => {
            let id = if gfx_id == 0x50E { 0x4A3C } else { 0x4A3D };
            let e = create_attach_scale(r, id, gobj, va.jobj());
            facing_y(r, e, va)
        }
        0x510 => ret_g(create_generator_appsrt_facing(r, 0xBF68, va)),
        0x511 => ret_e(create_attach_scale_facing(r, 0xBF68, gobj, va.jobj())),
        0x512 => ret_e(create_attach_scale_facing(r, 0xBF69, gobj, va.jobj())),
        _ => Spawned::None,
    }
}

/// A fighter's part's joint (`fp->parts[part].joint`).
pub(super) fn fighter_part(r: &Running, gobj: Option<GObjId>, part: usize) -> Option<JObjId> {
    let fp_ = r.game.fighter(gobj.expect("a fighter"));
    let parts = fp_.parts.expect("fp->parts");
    r.game.fighter_parts[parts.0]
        .get(part)
        .expect("fp->parts[part]: past the character's parts")
        .joint
}
