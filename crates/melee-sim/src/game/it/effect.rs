//! Item effects, `melee/it/iteffect.c`: the router that spawns an effect on an item, now
//! (`efSync_Spawn`) or through the item's queue (`efAsync_Spawn` on `xBC0`), with the
//! arguments each effect id takes.

use gekko_fp as fp;

use super::{item, item_mut, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg, SpawnKind};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// Spawns effect `ef_id` on the item's root, now, `it_802787B4` (`it_80278800` with no
/// offsets).
#[doc(alias = "it_802787B4")]
pub fn spawn_simple(r: &mut Running, gobj: GObjId, ef_id: i32) {
    let zero = Vec3::new(0.0, 0.0, 0.0);
    spawn(r, gobj, ef_id, 0, zero, zero, true);
}

/// How the router hands an effect id over (the blocks of `it_80278800`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
enum Route {
    /// On the bone.
    Attach,
    /// On the bone, with the offset vector.
    AttachVec,
    /// On the bone, with the color `0xF25959`, now.
    AttachColor,
    /// On the bone, with the float 1.
    AttachOne,
    /// On the bone, with the facing and the float 1, now.
    AttachFacing,
    /// On the bone, with the float 0, queued.
    AttachZero,
    /// At the randomized offset (`block_4A8`).
    At,
    /// At the offset, with the facing (`block_4FC`).
    AtFacing,
    /// At the offset, with the facing and 0, queued (`block_558`).
    AtFacingZero,
    /// At the offset, turned around when facing left (`block_59C`).
    AtTurned,
    /// At the offset, with 0 (`block_61C`).
    AtZero,
    /// A camera quake of kind 2, 3 or 4 at the offset, queued (`block_680` to `block_6E8`).
    Quake(i32),
    /// Nothing, after the offset's draws.
    None,
}

/// The ids the router handles before drawing an offset (its switch's cases).
fn explicit_route(ef_id: i32) -> Option<Route> {
    Some(match ef_id {
        0x402 | 0x403 | 0x409 | 0x412 | 0x413 | 0x414 | 0x41B | 0x422 | 0x433 | 0x449 | 0x473 => {
            Route::Attach
        }
        0x446 | 0x448 => Route::AttachVec,
        0x417..=0x41A => Route::AttachColor,
        0x415
        | 0x41E
        | 0x428..=0x42A
        | 0x438
        | 0x439
        | 0x43E
        | 0x44D
        | 0x45B
        | 0x45C
        | 0x45F..=0x472 => Route::AttachOne,
        0x44E..=0x452 | 0x457..=0x45A => Route::AttachFacing,
        0x423 | 0x424 => Route::AttachZero,
        _ => return None,
    })
}

/// Where the rest go, after the offset's draws (the original's search tree, flattened).
fn offset_route(ef_id: i32) -> Route {
    match ef_id {
        ..0x250 => Route::At,
        0x3E9..=0x3EC
        | 0x3EE
        | 0x3F3
        | 0x3F4
        | 0x3F6
        | 0x3FA..=0x3FC
        | 0x405
        | 0x407
        | 0x40C..=0x411
        | 0x416
        | 0x41C
        | 0x41D
        | 0x41F..=0x421
        | 0x425..=0x427
        | 0x42D
        | 0x434
        | 0x441
        | 0x442
        | 0x444
        | 0x445
        | 0x44A..=0x44C
        | 0x4D0 => Route::At,
        0x3ED
        | 0x3EF..=0x3F2
        | 0x3F5
        | 0x3FE
        | 0x400
        | 0x401
        | 0x42E..=0x431
        | 0x43A..=0x43C
        | 0x443 => Route::AtFacing,
        0x3F7..=0x3F9 | 0x3FD | 0x3FF => Route::AtFacingZero,
        0x435..=0x437 | 0x440 => Route::AtTurned,
        0x404 | 0x406 | 0x408 => Route::AtZero,
        0x513 => Route::Quake(2),
        0x514 => Route::Quake(3),
        0x515 => Route::Quake(4),
        _ => Route::None,
    }
}

/// Spawns effect `ef_id` for the item on its bone `bone`, `it_80278800`: now when `sync`, else
/// through its queue. Most ids are placed at `offset` in the bone's space, moved by up to
/// `spread` either way at random.
#[doc(alias = "it_80278800")]
pub fn spawn(
    r: &mut Running,
    gobj: GObjId,
    ef_id: i32,
    bone: i32,
    offset: Vec3,
    spread: Vec3,
    sync: bool,
) {
    let j = lib2725::bone(r.game, r.data, gobj, bone);
    let facing = item(r.game, gobj).facing_dir;
    if let Some(route) = explicit_route(ef_id) {
        let args: &[Arg] = match route {
            Route::Attach => &[Arg::JObj(j)],
            Route::AttachVec => &[Arg::JObj(j), Arg::Vec(offset)],
            Route::AttachColor => &[Arg::JObj(j), Arg::Int(0xF2_5959)],
            Route::AttachOne => &[Arg::JObj(j), Arg::Float(1.0)],
            Route::AttachFacing => &[Arg::JObj(j), Arg::Float(facing), Arg::Float(1.0)],
            _ => &[Arg::JObj(j), Arg::Float(0.0)],
        };
        let kind = match route {
            Route::Attach => SpawnKind::Attach,
            Route::AttachVec => SpawnKind::AttachOffset,
            _ => SpawnKind::AttachParam,
        };
        let now = match route {
            Route::AttachColor | Route::AttachFacing => true,
            Route::AttachZero => false,
            _ => sync,
        };
        if now {
            ef::spawn(r, ef_id, Some(gobj), args);
        } else {
            queue(r, gobj, kind, ef_id, j, &args[1..]);
        }
        return;
    }
    let mut at = offset;
    for (o, s) in [
        (&mut at.x, spread.x),
        (&mut at.y, spread.y),
        (&mut at.z, spread.z),
    ] {
        let f = r.game.objs.rng.draw_f32();
        let t = fp::fsubs(reg(f), 0.5);
        *o = mem(fp::fmadds(fp::fmuls(2.0, reg(s)), t, reg(*o)));
    }
    let world = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(at));
    let turned = mem(fp::frsp(if reg(facing) < 0.0 {
        std::f64::consts::PI
    } else {
        0.0
    }));
    // The position, then the route's one parameter if it has one.
    let (kind, param) = match offset_route(ef_id) {
        Route::At => (SpawnKind::PosOffset, None),
        Route::AtFacing => (SpawnKind::PosOffsetParam, Some(facing)),
        Route::AtFacingZero => {
            let args = [Arg::Vec(at), Arg::Float(facing), Arg::Float(0.0)];
            queue(r, gobj, SpawnKind::PosOffsetParam2, ef_id, j, &args);
            return;
        }
        Route::AtTurned => (SpawnKind::PosOffsetParam, Some(turned)),
        Route::AtZero => (SpawnKind::PosOffsetParam, Some(0.0)),
        Route::Quake(kind) => {
            queue(r, gobj, SpawnKind::CameraShake, kind, j, &[Arg::Vec(at)]);
            return;
        }
        _ => return,
    };
    let args = [
        Arg::Vec(if sync { world } else { at }),
        Arg::Float(param.unwrap_or(0.0)),
    ];
    let args = &args[..1 + usize::from(param.is_some())];
    if sync {
        ef::spawn(r, ef_id, Some(gobj), args);
    } else {
        queue(r, gobj, kind, ef_id, j, args);
    }
}

/// `efAsync_Spawn` on the item's queue.
pub fn queue(
    r: &mut Running,
    gobj: GObjId,
    kind: SpawnKind,
    ef_id: i32,
    j: Option<JObjId>,
    args: &[Arg],
) {
    let mut head = item(r.game, gobj).xBC0;
    ef::queue(r, Some(gobj), &mut head, kind, ef_id, j, args);
    item_mut(r.game, gobj).xBC0 = head;
}

/// Destroys the item's effects (`efLib_DestroyAll`).
pub fn destroy_all(r: &mut Running, gobj: GObjId) {
    crate::game::ef::destroy_all(r, gobj);
}
