//! efAsync (`melee/ef/efasync.c`): the cases for ids 0x3E8 to 0x477 (the common effects), and
//! deferring spawns made during fighters' and items' processes.

use gekko_fp as fp;

use super::super::code::ParticleHooks;
use super::super::state::Running;
use super::eflib::{self, drain_anim_queue};
use super::sync::{
    HALF_PI, TAU, ej, facing_rot, gen_appsrt, gen_at, gen_jobj, gobj_scale, random_turn, set_rot_y,
    set_rot_z, set_scale, srt, srt_scale,
};
use super::{
    Arg, Args, EffectId, LOAD_UNCOUNTED, Queued, QueuedId, SCALE_INHERIT, STATE_ASYNC, SpawnKind,
    Spawned, Update,
};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// The common effects' table, `efAsync_Dispatch` (ids 0x3E8 to 0x477, from `efSync_Spawn`).
#[doc(alias = "efAsync_Dispatch")]
pub fn dispatch(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, va: &mut Args) -> Spawned {
    let ret = cases(r, gfx_id, gobj, va);
    drain_anim_queue(r);
    ret
}

fn mark_async(r: &mut Running, e: EffectId) {
    r.game.ef.effects[e.0].state_flags |= STATE_ASYNC;
}

/// The child joint of an effect's joint (`jobj == NULL ? NULL : jobj->child`).
fn child(r: &Running, e: EffectId) -> Option<JObjId> {
    let j = ej(r, e);
    r.game.objs.jobjs[j].child
}

/// `HSD_JObjGetNext`.
fn next_of(r: &Running, j: Option<JObjId>) -> Option<JObjId> {
    r.game.objs.jobjs[j.expect("HSD_JObjGetNext: jobj")].next
}

/// `efLib_CreateGenerator_Translate_FacingDir`.
fn gen_facing(r: &mut Running, gfx_id: i32, t: Vec3, f: f32) -> Spawned {
    Spawned::from_generator(eflib::create_generator_facing(
        &mut r.game.objs,
        r.data,
        &mut ParticleHooks,
        gfx_id,
        t,
        f,
    ))
}

/// The TEV color a recoloring effect's spawner passes: grey for one color, else white.
fn konst_for(color: u32) -> u32 {
    if color.wrapping_add(0xFFA0_0000) == 0x6060 {
        0x80_8080
    } else {
        0xFF_FFFF
    }
}

fn cases(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, va: &mut Args) -> Spawned {
    use eflib::{
        create_attach, create_attach_child, create_attach_pos,
        create_generator_appsrt_facing_scale as gen_facing_scale,
        create_generator_appsrt_pos as gen_pos, create_generator_appsrt_scale as gen_scale,
        set_param_gfx_id, set_tev_konst_color,
    };
    let ret_e = Spawned::from_effect;
    let ret_g = Spawned::from_generator;
    // An effect at the next vector, marked async.
    let pos_async = |r: &mut Running, id: i32, va: &mut Args| {
        let e = create_attach_pos(r, id, gobj, va.vec());
        if let Some(e) = e {
            mark_async(r, e);
        }
        e
    };
    // An effect at the next vector, marked async, turned to the next float's facing and then
    // tilted by the float after.
    let pos_facing_z = |r: &mut Running, id: i32, va: &mut Args| {
        let e = pos_async(r, id, va);
        if let Some(e) = e {
            let f = va.float();
            set_rot_y(r, e, facing_rot(f));
            let z = copy(va.float());
            set_rot_z(r, e, z);
        }
        ret_e(e)
    };
    // A generator with an app-SRT at the next vector, tilted by the next float.
    let gen_tilt = |r: &mut Running, id: i32, va: &mut Args| {
        let g = gen_appsrt(r, id);
        if let Some(g) = g {
            let t = va.vec();
            srt(r, g).translate = t;
            let z = copy(va.float());
            srt(r, g).rot.z = z;
        }
        ret_g(g)
    };
    // A generator with an app-SRT at the next vector and the next float's scale; the spawn
    // returns nothing.
    let gen_scaled_none = |r: &mut Running, id: i32, va: &mut Args| {
        if let Some(g) = gen_appsrt(r, id) {
            let t = va.vec();
            srt(r, g).translate = t;
            let s = va.float();
            srt_scale(r, g, s);
        }
        Spawned::None
    };
    // A generator with an app-SRT at the next joint's position and the next float's scale.
    let gen_at_jobj = |r: &mut Running, id: i32, va: &mut Args| {
        let g = gen_appsrt(r, id);
        if let Some(g) = g {
            let j = va.jobj();
            let t = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
            srt(r, g).translate = t;
            let s = va.float();
            srt_scale(r, g, s);
        }
        g
    };
    // A generator facing the next float at the next joint's position, scaled by the float
    // after.
    let gen_facing_at_jobj = |r: &mut Running, id: i32, va: &mut Args| {
        let j = va.jobj();
        let t = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        let f = va.float();
        let g = gen_facing(r, id, t, f).generator();
        if let Some(g) = g {
            let s = va.float();
            srt_scale(r, g, s);
        }
        ret_g(g)
    };
    // An effect on the next joint at the next float's scale.
    let attach_scaled = |r: &mut Running, id: i32, va: &mut Args| {
        let e = create_attach(r, id, gobj, va.jobj());
        if let Some(e) = e {
            let s = copy(va.float());
            set_scale(r, e, Vec3::new(s, s, s));
        }
        ret_e(e)
    };
    // A recolored effect on the next joint: its child's TEV colors from the next word, its
    // joint's scale inherited, and the spawner's stored alpha applied each frame.
    let recolored = |r: &mut Running, id: i32, tobj: i32, va: &mut Args| {
        r.game.ef.load_kind = LOAD_UNCOUNTED;
        let e = create_attach(r, id, gobj, va.jobj());
        if let Some(e) = e {
            let color = va.int();
            let c = child(r, e);
            set_tev_konst_color(r.game, c, tobj, konst_for(color), color);
            r.game.ef.effects[e.0].scale_flags |= SCALE_INHERIT;
            set_param_gfx_id(r.game, gobj, gfx_id);
            r.game.ef.effects[e.0].update = Some(Update::ApplyStoredAlpha);
        }
        ret_e(e)
    };
    match gfx_id {
        0x3E8 => {
            let id = if r.game.objs.rng.below(8) == 0 {
                9
            } else {
                0xA
            };
            let e = create_attach_pos(r, id, gobj, va.vec());
            if let Some(e) = e {
                mark_async(r, e);
                let mut s = fp::fmadds(reg(0.04), reg(va.float()), reg(0.3));
                if s < reg(0.3) {
                    s = reg(0.3);
                }
                if s > reg(1.5) {
                    s = reg(1.5);
                }
                let s = mem(s);
                set_scale(r, e, Vec3::new(s, s, s));
            }
            ret_e(e)
        }
        0x3E9 | 0x3EA | 0x3F3 | 0x3F4 | 0x405 | 0x407 | 0x40C | 0x40E | 0x40F | 0x410 | 0x411
        | 0x416 | 0x41C | 0x41F | 0x420 | 0x421 | 0x425 | 0x426 | 0x434 | 0x43A | 0x43B | 0x43C
        | 0x441 | 0x442 | 0x444 | 0x445 | 0x477 | 0x474 | 0x475 | 0x476 => {
            let id = match gfx_id {
                0x3E9 => 0xC,
                0x3EA => 0x14,
                0x3F3 => 0xB,
                0x3F4 => 0x48,
                0x405 => 0x2C,
                0x407 => 0x3C,
                0x40C => 0x19,
                0x40E => 0x43,
                0x40F => 0xE3,
                0x410 => 0x22A,
                0x411 => 0x4B,
                0x416 => 0x196,
                0x41C => 0x5D,
                0x41F => 0x5C,
                0x420 => 0x159,
                0x421 => 0x3F,
                0x425 => 0x7E,
                0x426 => 0x7F,
                0x434 => 0x14D,
                0x43A => 0x193,
                0x43B => 0x192,
                0x43C => 0x1A0,
                0x441 => 0x1FB,
                0x442 => 0x1DC,
                0x444 => 0x1FF,
                0x445 => 0x209,
                0x477 => 0x7918,
                0x474 => 0xF7,
                0x475 => 0xFC,
                _ => 0xFF,
            };
            let pos = va.vec();
            ret_g(gen_at(r, id, pos))
        }
        0x3EB => ret_e(pos_async(r, 7, va)),
        0x3EC => {
            let e = pos_async(r, 8, va);
            if let Some(e) = e {
                let z = random_turn(r);
                set_rot_z(r, e, z);
            }
            ret_e(e)
        }
        0x3ED => {
            let t = va.vec();
            let g = gen_at(r, 0x50, t);
            if g.is_some()
                && let Some(g2) = gen_appsrt(r, 0x54)
            {
                srt(r, g2).translate = t;
                let rot = if reg(va.float()) < 0.0 {
                    0.0
                } else {
                    -std::f64::consts::PI as f32
                };
                srt(r, g2).rot.y = rot;
            }
            ret_g(g)
        }
        0x3EE => {
            let e = create_attach_pos(r, 0x27, gobj, va.vec());
            if let Some(e) = e {
                let z = random_turn(r);
                set_rot_z(r, e, z);
            }
            ret_e(e)
        }
        0x3EF | 0x3F0 | 0x3F1 | 0x3F2 | 0x400 | 0x401 => {
            let t = va.vec();
            let f = va.float();
            let f = if matches!(gfx_id, 0x3F0 | 0x3F2 | 0x401) {
                copy(f)
            } else {
                mem(fp::fneg(reg(f)))
            };
            let id = match gfx_id {
                0x3EF | 0x3F0 => 0x42,
                0x3F1 | 0x3F2 => 0x14B,
                _ => 0x5A,
            };
            gen_facing(r, id, t, f)
        }
        0x3F5 => {
            let e = pos_async(r, 0x10, va);
            if let Some(e) = e {
                let f = va.float();
                set_rot_y(r, e, facing_rot(f));
            }
            ret_e(e)
        }
        0x3F6 => ret_e(pos_async(r, 0x11, va)),
        0x3F7 => pos_facing_z(r, 0x12, va),
        0x3F8 => pos_facing_z(r, 0x13, va),
        0x3F9 => pos_facing_z(r, 0x14, va),
        0x3FA => ret_e(pos_async(r, 0x15, va)),
        0x3FB => ret_e(pos_async(r, 0x16, va)),
        0x3FC => ret_e(pos_async(r, 0x17, va)),
        0x3FD => pos_facing_z(r, 3, va),
        0x3FE | 0x42E | 0x430 | 0x440 => {
            let id = match gfx_id {
                0x3FE => 0x107,
                0x42E => 0x13C,
                0x430 => 0x140,
                _ => 0x1D8,
            };
            let t = va.vec();
            let f = copy(va.float());
            gen_facing(r, id, t, f)
        }
        0x3FF => pos_facing_z(r, 5, va),
        0x402 => gen_jobj(r, 0, 0x59, va.jobj()),
        0x403 => gen_jobj(r, 0, 0x5E, va.jobj()),
        0x404 | 0x406 => {
            let id = if gfx_id == 0x404 { 0x18 } else { 4 };
            let e = pos_async(r, id, va);
            if let Some(e) = e {
                let z = copy(va.float());
                set_rot_z(r, e, z);
            }
            ret_e(e)
        }
        0x408 => gen_tilt(r, 0x3E, va),
        0x409 => gen_jobj(r, 0, 0xE2, va.jobj()),
        0x40A => gen_tilt(r, 0x241, va),
        0x40B => gen_tilt(r, 0x242, va),
        0x40D => {
            let g = gen_appsrt(r, 0x19);
            if let Some(g) = g {
                let t = va.vec();
                srt(r, g).translate = t;
                let s = gobj_scale(r, gobj);
                srt_scale(r, g, s.y);
            }
            ret_g(g)
        }
        0x412 => gen_jobj(r, 0, 0x13, va.jobj()),
        0x413 => gen_jobj(r, 0, 0x37, va.jobj()),
        0x414 => gen_jobj(r, 0, 0xE1, va.jobj()),
        0x415 => {
            let e = create_attach_child(r, 0x25, gobj, va.jobj());
            if let Some(e) = e {
                mark_async(r, e);
                let s = copy(va.float());
                set_scale(r, e, Vec3::new(s, s, s));
            }
            ret_e(e)
        }
        0x417 => recolored(r, 0xB, 1, va),
        0x418 => recolored(r, 0xC, 0, va),
        0x419 => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let e = create_attach(r, 0xD, gobj, va.jobj());
            if let Some(e) = e {
                let color = va.int();
                let konst = konst_for(color);
                let mut j = child(r, e);
                set_tev_konst_color(r.game, j, 0, konst, color);
                for _ in 0..6 {
                    j = next_of(r, j);
                    set_tev_konst_color(r.game, j, 0, konst, color);
                }
                r.game.ef.effects[e.0].scale_flags |= SCALE_INHERIT;
                set_param_gfx_id(r.game, gobj, gfx_id);
                r.game.ef.effects[e.0].update = Some(Update::ApplyStoredAlpha);
            }
            ret_e(e)
        }
        0x41A => recolored(r, 0xE, 0, va),
        0x41B => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            if let Some(g) = gen_appsrt(r, 0x31) {
                let j = va.jobj();
                let t = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
                srt(r, g).translate = t;
                let s = r.game.objs.jobjs[j.expect("HSD_JObjGetScale: jobj")].scale;
                srt_scale(r, g, s.y);
            }
            Spawned::None
        }
        0x41D => ret_e(pos_async(r, 0xF, va)),
        0x41E => ret_g(gen_scale(r, 0x55, va)),
        0x422 => gen_jobj(r, 0, 0x5B, va.jobj()),
        0x423 | 0x424 => {
            let id = if gfx_id == 0x423 { 1 } else { 2 };
            let e = create_attach(r, id, gobj, va.jobj());
            if let Some(e) = e {
                let z = copy(va.float());
                set_rot_z(r, e, z);
            }
            ret_e(e)
        }
        0x427 => {
            let t = va.vec();
            let mut first = None;
            let mut last: Option<EffectId> = None;
            for i in 0..6 {
                let Some(e) = create_attach_pos(r, 0x1B, gobj, t) else {
                    break;
                };
                let ef = &mut r.game.ef.effects[e.0];
                ef.update = Some(Update::FallFromParamY);
                ef.lifetime = 0x28;
                let rnd = reg(r.game.objs.rng.draw_f32());
                r.game.ef.effects[e.0].params.y = mem(fp::fmadds(reg(0.4), rnd, reg(2.8)));
                let rnd = reg(r.game.objs.rng.draw_f32());
                let turn = mem(fp::frsp(fp::fmul(TAU, rnd)));
                set_rot_y(r, e, turn);
                match last {
                    Some(l) if i != 0 => r.game.ef.effects[l.0].next = Some(e),
                    _ => first = Some(e),
                }
                last = Some(e);
            }
            ret_e(first)
        }
        0x428 => ret_g(gen_scale(r, 0xCA, va)),
        0x429 => ret_g(gen_scale(r, 0xCE, va)),
        0x42A => ret_g(gen_scale(r, 0xCF, va)),
        0x42B => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let e = create_attach_pos(r, 0x19, gobj, va.vec());
            if let Some(e) = e {
                let z = copy(va.float());
                set_rot_z(r, e, z);
                let s = copy(va.float());
                set_scale(r, e, Vec3::new(s, s, s));
                let c = child(r, e);
                // MWCC reads the call's last argument first.
                let tev0 = va.int();
                let konst = va.int();
                set_tev_konst_color(r.game, c, 0, konst, tev0);
            }
            ret_e(e)
        }
        0x42C => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let e = create_attach_pos(r, 0x1A, gobj, va.vec());
            if let Some(e) = e {
                let z = copy(va.float());
                set_rot_z(r, e, z);
            }
            ret_e(e)
        }
        0x42D => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let pos = va.vec();
            ret_g(gen_at(r, 0x121, pos))
        }
        0x42F | 0x431 => {
            let id = if gfx_id == 0x42F { 0x20 } else { 0x21 };
            let e = create_attach_pos(r, id, gobj, va.vec());
            if let Some(e) = e {
                let f = va.float();
                set_rot_y(r, e, facing_rot(f));
            }
            ret_e(e)
        }
        0x432 => gen_scaled_none(r, 0x145, va),
        0x433 => gen_jobj(r, 0, 0x115, va.jobj()),
        0x435..=0x437 => {
            let id = match gfx_id {
                0x435 => 0x14E,
                0x436 => 0x153,
                _ => 0x156,
            };
            let g = gen_appsrt(r, id);
            if let Some(g) = g {
                let t = va.vec();
                srt(r, g).translate = t;
                srt(r, g).rot.y = HALF_PI;
                let z = copy(va.float());
                srt(r, g).rot.z = z;
            }
            ret_g(g)
        }
        0x438 => {
            let e = create_attach(r, 0x22, gobj, va.jobj());
            if let Some(e) = e {
                let gj = super::super::gr::ground::jobj_of(r.game, gobj.expect("a spawner"))
                    .expect("HSD_JObjGetRotationY: jobj");
                assert!(
                    r.game.objs.jobjs[gj].flags & jobj::USE_QUATERNION == 0,
                    "!(jobj->flags & JOBJ_USE_QUATERNION)"
                );
                let ry = copy(r.game.objs.jobjs[gj].rotate.y);
                set_rot_y(r, e, ry);
                let px = copy(va.float());
                let ef = &mut r.game.ef.effects[e.0];
                ef.update = Some(Update::SetScaleFromParamX);
                ef.params.x = px;
            }
            ret_e(e)
        }
        0x439 => {
            let e = create_attach(r, 0x23, gobj, va.jobj());
            if let Some(e) = e {
                let s = copy(va.float());
                scale_xyz(r, e, s);
            }
            ret_e(e)
        }
        0x43D => gen_jobj(r, 0, 0x1AF, va.jobj()),
        0x43E => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let e = create_attach(r, 0x24, gobj, va.jobj());
            if let Some(e) = e {
                let s = copy(va.float());
                scale_xyz(r, e, s);
            }
            ret_e(e)
        }
        0x43F => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            gen_scaled_none(r, 0xCA, va)
        }
        0x443 => gen_scaled_none(r, 0x1F1, va),
        0x446 => {
            let j = va.jobj();
            let v = va.vec();
            let e = gen_pos(r, 0x1B, gobj, j, v);
            if let Some(e) = e {
                r.game.ef.effects[e.0].lifetime = 0x16;
            }
            ret_e(e)
        }
        0x447 => {
            r.game.ef.load_kind = LOAD_UNCOUNTED;
            let e = create_attach_pos(r, 0x28, gobj, va.vec());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::Bury);
                let s = copy(va.float());
                set_scale(r, e, Vec3::new(s, s, s));
            }
            ret_e(e)
        }
        0x448 => {
            let j = va.jobj();
            let v = va.vec();
            let e = gen_pos(r, 0x92, gobj, j, v);
            if let Some(e) = e {
                r.game.ef.effects[e.0].lifetime = 0xB4;
            }
            ret_e(e)
        }
        0x449 => ret_e(create_attach(r, 0x26, gobj, va.jobj())),
        0x44A..=0x44C => {
            let s = match gfx_id {
                0x44A => 0.5,
                0x44B => 1.0,
                _ => 2.0,
            };
            if let Some(g) = gen_appsrt(r, 0x237) {
                let t = va.vec();
                srt(r, g).translate = t;
                srt_scale(r, g, s);
            }
            Spawned::None
        }
        0x44D => ret_g(gen_scale(r, 0x48, va)),
        0x44E | 0x457 => ret_g(gen_facing_scale(r, 0xDA, va)),
        0x44F | 0x458 => ret_g(gen_facing_scale(r, 0xDB, va)),
        0x450 | 0x459 => ret_g(gen_facing_scale(r, 0xDC, va)),
        0x451 | 0x45A => ret_g(gen_facing_scale(r, 0xDD, va)),
        0x453 => gen_facing_at_jobj(r, 0x234, va),
        0x454 => gen_facing_at_jobj(r, 0x235, va),
        0x455 => gen_facing_at_jobj(r, 0x236, va),
        0x456 => gen_facing_at_jobj(r, 0x23D, va),
        0x452 => ret_g(gen_at_jobj(r, 0x21E, va)),
        0x45B => ret_g(gen_scale(r, 0x8C, va)),
        0x45C => ret_g(gen_scale(r, 0x8D, va)),
        0x45D => ret_g(gen_at_jobj(r, 0x23F, va)),
        0x45E => ret_g(gen_at_jobj(r, 0x240, va)),
        0x45F => ret_g(gen_scale(r, 0x8E, va)),
        0x460 => ret_g(gen_scale(r, 0x99, va)),
        0x461 => ret_g(gen_scale(r, 0x95, va)),
        0x462 => ret_g(gen_scale(r, 0x219, va)),
        0x463 => attach_scaled(r, 0x29, va),
        0x464 => attach_scaled(r, 0x2A, va),
        0x465 => ret_g(gen_scale(r, 0x88, va)),
        0x466 => ret_g(gen_scale(r, 0x89, va)),
        0x467 => ret_g(gen_scale(r, 0x8A, va)),
        0x468 => attach_scaled(r, 0x2C, va),
        0x469 => attach_scaled(r, 0x2E, va),
        0x46A | 0x46C => {
            let id = if gfx_id == 0x46A { 0xAC } else { 0x9E };
            let g = gen_at_jobj(r, id, va);
            if let Some(g) = g {
                let gj = super::super::gr::ground::jobj_of(r.game, gobj.expect("a spawner"))
                    .expect("HSD_JObjGetRotationY: jobj");
                assert!(
                    r.game.objs.jobjs[gj].flags & jobj::USE_QUATERNION == 0,
                    "!(jobj->flags & JOBJ_USE_QUATERNION)"
                );
                let ry = copy(r.game.objs.jobjs[gj].rotate.y);
                srt(r, g).rot.y = ry;
            }
            ret_g(g)
        }
        0x46B => attach_scaled(r, 0x2B, va),
        0x46D => attach_scaled(r, 0x2D, va),
        0x46E => ret_g(gen_scale(r, 0xAE, va)),
        0x46F => ret_g(gen_scale(r, 0xA0, va)),
        0x470 => ret_g(gen_scale(r, 0x21B, va)),
        0x471 => ret_g(gen_scale(r, 0x220, va)),
        0x472 => ret_g(gen_scale(r, 0x131, va)),
        0x473 => gen_jobj(r, 0, 0x82, va.jobj()),
        _ => Spawned::None,
    }
}

/// Sets an effect's three scales one by one (the inline `efAsync_SetEffectScaleXYZ`).
fn scale_xyz(r: &mut Running, e: EffectId, s: f32) {
    let j = ej(r, e);
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_srt(jobjs, j, |o| o.scale.x = s);
    jobj::set_srt(jobjs, j, |o| o.scale.y = s);
    jobj::set_srt(jobjs, j, |o| o.scale.z = s);
}

/// Runs a deferred spawn and frees it, `efAsync_QueueProcessDeferred`: on the joint, at its
/// position or an offset from it, with the floats the spawn kept, or a camera quake there.
#[doc(alias = "efAsync_QueueProcessDeferred")]
pub fn process_deferred(r: &mut Running, gobj: Option<GObjId>, q: QueuedId) {
    let Queued {
        kind,
        gfx_id,
        jobj,
        params,
        extra1,
        extra2,
        ..
    } = r.game.ef.queued[q.0].clone();
    let at = |r: &mut Running, offset: Option<Vec3>| {
        crate::lb::joint::world_position(&mut r.game.objs, r.data, jobj, offset)
    };
    match SpawnKind::from_u8(kind) {
        Some(SpawnKind::Attach) => {
            super::spawn(r, gfx_id, gobj, &[Arg::JObj(jobj)]);
        }
        Some(SpawnKind::Pos) => {
            let p = at(r, None);
            super::spawn(r, gfx_id, gobj, &[Arg::Vec(p)]);
        }
        Some(SpawnKind::PosOffset) => {
            let p = at(r, Some(params));
            super::spawn(r, gfx_id, gobj, &[Arg::Vec(p)]);
        }
        Some(SpawnKind::AttachParam) | Some(SpawnKind::AttachOffset) => {
            // The spawn reads `params` as a vector or its first word as a float.
            super::spawn(r, gfx_id, gobj, &[Arg::JObj(jobj), Arg::Params(params)]);
        }
        Some(SpawnKind::PosParam) => {
            let p = at(r, None);
            super::spawn(r, gfx_id, gobj, &[Arg::Vec(p), Arg::Params(params)]);
        }
        Some(SpawnKind::PosOffsetParam) => {
            let p = at(r, Some(params));
            super::spawn(r, gfx_id, gobj, &[Arg::Vec(p), Arg::Float(extra1)]);
        }
        Some(SpawnKind::PosOffsetParam2) => {
            let p = at(r, Some(params));
            super::spawn(
                r,
                gfx_id,
                gobj,
                &[Arg::Vec(p), Arg::Float(extra1), Arg::Float(extra2)],
            );
        }
        Some(SpawnKind::CameraShake) => {
            let p = at(r, Some(params));
            super::super::camera::request_quake(r.game, gfx_id, Some(p));
        }
        None => panic!("[EfASync] unknown type {kind}"),
    }
    r.game.ef.queued.free(q.0);
}

/// Runs every deferred spawn of a queue, oldest last as they were pushed, and empties it,
/// `efAsync_QueueFlush`.
#[doc(alias = "efAsync_QueueFlush")]
pub fn flush(r: &mut Running, gobj: Option<GObjId>, head: &mut Option<QueuedId>) {
    let mut cur = head.take();
    while let Some(q) = cur {
        cur = r.game.ef.queued[q.0].next;
        process_deferred(r, gobj, q);
    }
}

/// Frees a queue's deferred spawns unrun, `efAsync_QueueClear`.
#[doc(alias = "efAsync_QueueClear")]
pub fn queue_clear(game: &mut super::super::state::Game, head: &mut Option<QueuedId>) {
    let mut cur = head.take();
    while let Some(q) = cur {
        cur = game.ef.queued[q.0].next;
        game.ef.queued.free(q.0);
    }
}

/// Spawns an effect for `gobj` on or at `jobj`, `efAsync_Spawn`: during a process of priority
/// below 9 (fighters' and items' own), it goes onto the front of `head` for the owner to run
/// later; otherwise it runs now. `args` are what `kind` takes: an offset, floats.
#[doc(alias = "efAsync_Spawn")]
pub fn queue(
    r: &mut Running,
    gobj: Option<GObjId>,
    head: &mut Option<QueuedId>,
    kind: SpawnKind,
    gfx_id: i32,
    jobj: Option<JObjId>,
    args: &[Arg],
) {
    queue_with(r, gobj, head, kind, gfx_id, jobj, &mut Args::new(args));
}

/// [`queue`] with arguments from wherever `va` reads them.
#[allow(clippy::too_many_arguments)]
pub fn queue_with(
    r: &mut Running,
    gobj: Option<GObjId>,
    head: &mut Option<QueuedId>,
    kind: SpawnKind,
    gfx_id: i32,
    jobj: Option<JObjId>,
    va: &mut Args,
) {
    let mut q = Queued {
        kind: kind as u8,
        gfx_id,
        jobj,
        ..Queued::default()
    };
    match kind {
        SpawnKind::Attach | SpawnKind::Pos => {}
        SpawnKind::PosOffset | SpawnKind::AttachOffset | SpawnKind::CameraShake => {
            q.params = va.vec();
        }
        SpawnKind::AttachParam | SpawnKind::PosParam => q.params.x = copy(va.float()),
        SpawnKind::PosOffsetParam => {
            q.params = va.vec();
            q.extra1 = copy(va.float());
        }
        SpawnKind::PosOffsetParam2 => {
            q.params = va.vec();
            q.extra1 = copy(va.float());
            q.extra2 = copy(va.float());
        }
    }
    let id = QueuedId(r.game.ef.queued.alloc(q));
    let w = &r.game.world;
    if w.current_proc().is_some() && w.current_s_link < 9 {
        r.game.ef.queued[id.0].next = *head;
        *head = Some(id);
        return;
    }
    process_deferred(r, gobj, id);
}
