//! efAlt (`melee/ef/efalt.c`): the cases for ids 0x479 to 0x4B9 (the first characters'
//! effects), all uncounted.

use super::super::code::ParticleHooks;
use super::super::state::Running;
use super::eflib::{self, drain_anim_queue};
use super::sync::{
    HALF_PI, ej, facing_rot, fighter_part, gen_at, gen_jobj, gobj_scale, set_rot_y, set_rot_z,
    set_scale,
};
use super::{Args, EffectId, LOAD_UNCOUNTED, Spawned, Update};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use gekko_fp as fp;

/// Peach's, Zelda's and the first characters' table, `efAlt_Spawn` (ids 0x479 to 0x4B9, from
/// `efSync_Spawn`).
#[doc(alias = "efAlt_Spawn")]
pub fn spawn_alt(r: &mut Running, gfx_id: i32, gobj: Option<GObjId>, va: &mut Args) -> Spawned {
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
        create_attach, create_attach_child, create_attach_child_scale, create_attach_pos,
        create_attach_scale, create_attach_scale_facing, create_generator_appsrt_facing,
        create_generator_attach_scale as gen_attach_scale,
    };
    let ret_e = Spawned::from_effect;
    let ret_g = Spawned::from_generator;
    // An effect turned to the facing the next float gives.
    let facing_y = |r: &mut Running, e: Option<EffectId>, va: &mut Args| {
        if let Some(e) = e {
            let f = va.float();
            set_rot_y(r, e, facing_rot(f));
        }
        ret_e(e)
    };
    // The same, then animated once.
    let facing_y_anim = |r: &mut Running, e: Option<EffectId>, va: &mut Args| {
        if let Some(e) = e {
            let f = va.float();
            set_rot_y(r, e, facing_rot(f));
            let j = ej(r, e);
            jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, Some(j));
        }
        ret_e(e)
    };
    // An effect at the next vector turned to face +z and tilted by the next float.
    let pos_tilted = |r: &mut Running, id: i32, va: &mut Args| {
        let e = create_attach_pos(r, id, gobj, va.vec());
        if let Some(e) = e {
            set_rot_y(r, e, HALF_PI);
            let z = copy(va.float());
            set_rot_z(r, e, z);
        }
        ret_e(e)
    };
    // An effect on the next joint at the spawner's scale.
    let child_scaled = |r: &mut Running, id: i32, va: &mut Args| {
        let e = create_attach_child(r, id, gobj, va.jobj());
        if let Some(e) = e {
            let s = gobj_scale(r, gobj);
            set_scale(r, e, s);
        }
        ret_e(e)
    };
    match gfx_id {
        0x479 | 0x47B | 0x483 | 0x49E | 0x4A0 | 0x4B2 => {
            let id = match gfx_id {
                0x479 => 0x3F2,
                0x47B => 0x3EB,
                0x483 => 0x7D7,
                0x49E => 0x206,
                0x4A0 => 0x7D02,
                _ => 0x908A,
            };
            let pos = va.vec();
            ret_g(gen_at(r, id, pos))
        }
        0x47A | 0x49F | 0x4B1 => {
            let (id, bank) = match gfx_id {
                0x47A => (0x3E8, 1),
                0x49F => (0x7D00, 0x20),
                _ => (0x9088, 0x25),
            };
            let j = va.jobj();
            let e = create_attach(r, id, gobj, j);
            let ret = facing_y(r, e, va);
            gen_jobj(r, bank, id + i32::from(gfx_id == 0x47A), j);
            ret
        }
        0x47C => {
            let e = create_attach_scale(r, 0x3E9, gobj, va.jobj());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::MarioSpecialLw);
            }
            ret_e(e)
        }
        0x47D => gen_jobj(r, 1, 0x3F0, va.jobj()),
        0x47E => gen_jobj(r, 1, 0x3F1, va.jobj()),
        0x47F => gen_jobj(r, 2, 0x7D4, va.jobj()),
        0x480 => gen_jobj(r, 2, 0x7D2, va.jobj()),
        0x481 => gen_jobj(r, 2, 0x7D3, va.jobj()),
        0x482 => {
            let e = create_attach(r, 0x7D0, gobj, va.jobj());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::SetScaleRotYFromFighter);
            }
            ret_e(e)
        }
        0x484 => gen_jobj(r, 2, 0x7DB, va.jobj()),
        0x485 => gen_jobj(r, 2, 0x7DE, va.jobj()),
        0x486 | 0x4A4 => {
            let id = if gfx_id == 0x486 { 0x7D1 } else { 0x84D0 };
            let e = create_attach_pos(r, id, gobj, va.vec());
            facing_y(r, e, va)
        }
        0x487 => ret_e(create_attach_child_scale(r, 0x7D2, gobj, va.jobj())),
        0x488 => ret_e(create_attach_scale(r, 0xBB8, gobj, va.jobj())),
        0x489 => ret_e(create_attach_scale(r, 0xBB9, gobj, va.jobj())),
        0x48A => ret_e(create_attach_scale(r, 0xBBA, gobj, va.jobj())),
        0x48B => ret_e(create_attach(r, 0xBBB, gobj, va.jobj())),
        0x48C => {
            let e = create_attach(r, 0xBBC, gobj, va.jobj());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::SetRotYZFromFighter);
            }
            ret_e(e)
        }
        0x48D => ret_g(create_generator_appsrt_facing(r, 0xBC0, va)),
        0x48E => pos_tilted(r, 0xBBD, va),
        0x48F | 0x4AB | 0x4B3 => {
            let (a, b) = match gfx_id {
                0x48F => (0xFA0, 0xFA1),
                0x4AB => (0x9471, 0x9470),
                _ => (0xB799, 0xB798),
            };
            let e = create_attach_scale_facing(r, a, gobj, va.jobj());
            pair(r, e, |r| create_attach_scale_facing(r, b, gobj, va.jobj()))
        }
        0x490 => {
            let e = create_attach(r, 0xFA2, gobj, va.jobj());
            if let Some(e) = e {
                let z = copy(va.float());
                let ef = &mut r.game.ef.effects[e.0];
                ef.params.z = z;
                ef.update = Some(Update::SetRotYZFromParamZFighterDir);
            }
            ret_e(e)
        }
        0x491 => ret_e(create_attach(r, 0xFA4, gobj, va.jobj())),
        0x492 | 0x493 | 0x495 => {
            let id = match gfx_id {
                0x492 => 0xFA3,
                0x493 => 0xFA5,
                _ => 0x138B,
            };
            let e = create_attach_scale(r, id, gobj, va.jobj());
            facing_y(r, e, va)
        }
        0x494 => {
            let fighter = r.game.fighter_id(gobj.expect("a fighter"));
            let j = fighter_part(r, gobj, R3RDNA);
            let j2 = fighter_part(r, gobj, TRANSN);
            let e1 = create_attach_child(r, 0x1388, gobj, j);
            if let Some(e1) = e1 {
                let ef = &mut r.game.ef.effects[e1.0];
                ef.update = Some(Update::SetRotYAndTransition);
                ef.lifetime = 0x41;
                ef.user_data = None;
                ef.left.user_data = super::Held::Written;
                let e2 = create_attach(r, 0x1389, gobj, j);
                r.game.ef.effects[e1.0].next = e2;
                if let Some(e2) = e2 {
                    let ef = &mut r.game.ef.effects[e2.0];
                    ef.update = Some(Update::SetRotYAndTransition);
                    ef.lifetime = 0x41;
                    ef.user_data = Some(fighter);
                    ef.left.user_data = super::Held::Written;
                    let e3 = create_attach(r, 0x138A, gobj, j2);
                    r.game.ef.effects[e2.0].next = e3;
                    if let Some(e3) = e3 {
                        let ef = &mut r.game.ef.effects[e3.0];
                        ef.update = Some(Update::SetRotYAndTransition);
                        ef.lifetime = 0x41;
                        ef.user_data = Some(fighter);
                        ef.left.user_data = super::Held::Written;
                    }
                }
            }
            ret_e(e1)
        }
        0x496 => ret_e(create_attach(r, 0x138C, gobj, va.jobj())),
        0x497 => {
            let e = create_attach(r, 0x138D, gobj, va.jobj());
            if let Some(e) = e {
                r.game.ef.effects[e.0].update = Some(Update::SetRotYFromFighterDir);
            }
            ret_e(e)
        }
        0x498 => ret_e(create_attach_child(r, 0x138E, gobj, va.jobj())),
        0x499 => ret_e(create_attach_child(r, 0x138F, gobj, va.jobj())),
        0x49A => ret_g(create_generator_appsrt_facing(r, 0x138B, va)),
        0x49B => gen_jobj(r, 5, 0x138F, va.jobj()),
        0x49C => gen_jobj(r, 5, 0x1395, va.jobj()),
        0x49D => {
            let e = create_attach_pos(r, 0x1390, gobj, va.vec());
            if let Some(e) = e {
                let gj = super::super::gr::ground::jobj_of(r.game, gobj.expect("a spawner"))
                    .expect("HSD_JObjGetScaleY: jobj");
                let j = ej(r, e);
                let jobjs = &mut r.game.objs.jobjs;
                let sy = copy(jobjs[gj].scale.y);
                jobj::set_srt(jobjs, j, |o| o.scale.x = sy);
                let sy = copy(jobjs[gj].scale.y);
                jobj::set_srt(jobjs, j, |o| o.scale.y = sy);
                let sy = copy(jobjs[gj].scale.y);
                jobj::set_srt(jobjs, j, |o| o.scale.z = sy);
                let c = jobjs[j].child.expect("HSD_JObjSetTranslateZ: jobj");
                let z = mem(fp::fsubs(reg(jobjs[j].translate.z), reg(6.0)));
                jobj::set_srt(jobjs, c, |o| o.translate.z = z);
                if reg(va.float()) < 0.0 {
                    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, -HALF_PI);
                    let z = mem(fp::fneg(reg(va.float())));
                    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
                } else {
                    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, HALF_PI);
                    let z = copy(va.float());
                    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
                }
                r.game.ef.effects[e.0].update = Some(Update::SetJObjOffsetZ);
            }
            ret_e(e)
        }
        0x4A5 => ret_g(gen_attach_scale(r, 0xA028, va, gobj)),
        0x4A6 => ret_g(gen_attach_scale(r, 0xA029, va, gobj)),
        0x4A7 => ret_g(gen_attach_scale(r, 0xA02A, va, gobj)),
        0x4A8 => ret_g(gen_attach_scale(r, 0xA02B, va, gobj)),
        0x4A1 => gen_jobj(r, 0x22, 0x84D4, va.jobj()),
        0x4A2 => gen_jobj(r, 0x22, 0x84D2, va.jobj()),
        0x4A3 => gen_jobj(r, 0x22, 0x84D3, va.jobj()),
        0x4A9 | 0x4AA => {
            let id = if gfx_id == 0x4A9 { 0x9858 } else { 0x9859 };
            let e = create_attach_scale(r, id, gobj, va.jobj());
            facing_y_anim(r, e, va)
        }
        0x4AC => pos_tilted(r, 0x80E8, va),
        0x4AD => gen_jobj(r, 0x24, 0x8CA0, va.jobj()),
        0x4AE => gen_jobj(r, 0x2E, 0xB3B0, va.jobj()),
        0x4AF => gen_jobj(r, 0x2E, 0xB3B1, va.jobj()),
        0x4B0 => ret_g(create_generator_appsrt_facing(r, 0xB3B6, va)),
        0x4B4 => ret_e(create_attach_scale_facing(r, 0x4E20, gobj, va.jobj())),
        0x4B5 => ret_e(create_attach_scale_facing(r, 0x4E21, gobj, va.jobj())),
        0x4B6 => child_scaled(r, 0x5208, va),
        0x4B7 => child_scaled(r, 0x5209, va),
        0x4B8 => ret_e(create_attach_scale_facing(r, 0xBB80, gobj, va.jobj())),
        0x4B9 => ret_e(create_attach_scale_facing(r, 0xBB81, gobj, va.jobj())),
        _ => Spawned::None,
    }
}

/// `FtPart_R3rdNa` and `FtPart_TransN`, the parts Ness's PK Thunder effect follows.
const R3RDNA: usize = 44;
const TRANSN: usize = 1;
