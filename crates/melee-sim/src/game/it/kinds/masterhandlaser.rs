//! Master Hand's finger beams' lasers, `melee/it/kinds/itmasterhandlaser.c`: a beam from a
//! finger's joint, attached to it, as long as it reaches before the stage cuts it, sparking
//! where it's cut every few frames, until Master Hand ends it.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::Arg;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `It_Kind_MasterHand_Laser`.
pub const KIND: i32 = 125;

#[doc(alias = "itMasterhandlaser_UnkMotion0_Anim")]
pub const ANIM_0: u32 = 0x802F_04F0;
#[doc(alias = "itMasterhandlaser_UnkMotion0_Phys")]
pub const PHYS_0: u32 = 0x802F_059C;
#[doc(alias = "itMasterhandlaser_UnkMotion0_Coll")]
pub const COLL_0: u32 = 0x802F_05A0;
pub const ACCESSORY: u32 = 0x802F_05A8;

/// `itMasterHandLaserAttributes`: the beam's length (`x0`) and the frames between sparks
/// (`x4`, a float).
mod attr {
    pub const LENGTH: u32 = 0x0;
    pub const SPARK_FRAMES: u32 = 0x4;
}

/// The spark where the stage cuts the beam.
const GFX_SPARK: i32 = 0x405;

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a laser without attributes");
    r.data.f32(a.at(off))
}

/// The beam's end bone (`xBBC_dynamicBoneTable->bones[2]`).
fn tip(r: &Running, gobj: GObjId) -> Option<JObjId> {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a laser without bones");
    r.game.it.bone_tables[t.0].bones[2]
}

/// The joint of the owner's part `part`.
fn owner_part(r: &Running, owner: GObjId, part: i32) -> Option<JObjId> {
    let fp = r.game.fighter(owner);
    let parts = fp.parts.expect("a fighter without parts");
    r.game.fighter_parts[parts.0][part as usize].joint
}

/// A laser from Master Hand's part `part` (at `pos`), `it_802F0340`: attached to the part's
/// joint, facing his way.
#[doc(alias = "it_802F0340")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    prev_pos: Vec3,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = prev_pos;
    spawn.pos = pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn);
    let g = gobj.expect("it_802F0484 of no laser reads address 0");
    reset(r, g);
    debug_owner_bubbles(r.game, g, Some(owner));
    lib2725::first_frame(r, g);
    let j = jobj_of(r.game, g);
    let v = item_mut(r.game, g).xDD4_itemVar.masterhand_laser();
    v.x4.set(part);
    v.x10.set(j);
    let target = owner_part(r, owner, part);
    crate::lb::anim::attach(&mut r.game.objs, j, target);
    super::super::item::animate(r, g);
    gobj
}

/// Forgets a GObj the laser points to, `itMasterHandLaser_Logic84_EvtUnk` (`it_8026B894`).
// Ports `itMasterHandLaser_Logic84_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Ends the laser on its next frame, `it_802F046C`.
#[doc(alias = "it_802F046C")]
pub fn end(r: &mut Running, gobj: Option<GObjId>) {
    if let Some(g) = gobj {
        item_mut(r.game, g)
            .xDD4_itemVar
            .masterhand_laser()
            .x0
            .set(1);
    }
}

/// The laser's state, `it_802F0484`: state 0, its accessory, not ended, the spark's count.
#[doc(alias = "it_802F0484")]
pub fn reset(r: &mut Running, gobj: GObjId) {
    let n = attr_f32(r, gobj, attr::SPARK_FRAMES);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(crate::game::handles::CodeRef(ACCESSORY));
    let v = ip.xDD4_itemVar.masterhand_laser();
    v.x0.set(0);
    v.x8.set(fp::fctiwz(reg(n)));
}

/// `itMasterhandlaser_UnkMotion0_Anim`: its animation again when it ends; gone once ended,
/// let go of the finger first.
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        reset(r, gobj);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.masterhand_laser();
    if v.x0.get() != 0 {
        let j = v.x10.get().expect("a laser without its joint");
        crate::game::ft::co::throw::unconstrain(&mut r.game.objs, j);
        return true;
    }
    false
}

/// The beam each frame, `it_802F05A8`: its length, then its hitbox from the finger to its
/// end.
#[doc(alias = "it_802F05A8")]
pub fn accessory(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj).owner.expect("a laser without its owner");
    reach(r, gobj, owner);
    let part = item_mut(r.game, gobj)
        .xDD4_itemVar
        .masterhand_laser()
        .x4
        .get();
    let pj = owner_part(r, owner, part);
    let from = world_position(&mut r.game.objs, r.data, pj, None);
    let t = tip(r, gobj);
    let to = world_position(&mut r.game.objs, r.data, t, None);
    hitbox_off(r, gobj);
    hitbox_at(r, gobj, from, to, 0);
}

/// Each hitbox that's on set to state 4, `it_80275788`.
#[doc(alias = "it_80275788")]
pub fn hitbox_off(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    for k in 0..4 {
        let hit = &mut ip.x5D4_hitboxes[k].hit;
        if hit.state != crate::lb::hitcapsule::DISABLED {
            hit.state = crate::lb::hitcapsule::UNK4;
            super::super::set_flag(&mut ip.xDAA, 2, true);
        }
    }
}

/// Hitbox `idx`'s ends, `it_80275820`, if it's on.
#[doc(alias = "it_80275820")]
pub fn hitbox_at(r: &mut Running, gobj: GObjId, a: Vec3, b: Vec3, idx: usize) {
    let hit = &mut item_mut(r.game, gobj).x5D4_hitboxes[idx].hit;
    if hit.state != crate::lb::hitcapsule::DISABLED {
        hit.x58 = b;
        hit.x4C = a;
    }
}

/// The inline `sqrtf` that stores its result through a volatile, `sqrtf_store`.
fn sqrt_store(x: f64) -> f64 {
    if x > 0.0 {
        reg(mem(crate::math::sqrt_reg(x)))
    } else {
        x
    }
}

/// The beam's length, `it_802F063C`: its model reset, its end `x0` along it; cut where the
/// stage crosses it (from the finger), with a spark there every `x4` frames.
#[doc(alias = "it_802F063C")]
pub fn reach(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let length = attr_f32(r, gobj, attr::LENGTH);
    let root = jobj_of(r.game, gobj);
    let child = root.and_then(|j| r.game.objs.jobjs[j].child);
    let desc = item(r.game, gobj).xC8_joint;
    let desc_child = desc.and_then(|d| r.data.ptr(d.at(8)));
    crate::game::gr::anime::reset_tree_srt(&mut r.game.objs, r.data, child, desc_child);
    let part = item_mut(r.game, gobj)
        .xDD4_itemVar
        .masterhand_laser()
        .x4
        .get();
    let pj = owner_part(r, owner, part);
    let p0 = world_position(&mut r.game.objs, r.data, pj, None);
    let mut translate = Vec3::new(0.0, mem(fp::fneg(reg(length))), 0.0);
    let t = tip(r, gobj).expect("a laser without its end");
    jobj::set_translate(&mut r.game.objs.jobjs, t, translate);
    let p2 = world_position(&mut r.game.objs, r.data, Some(t), None);
    let hit = r
        .game
        .geometry
        .as_mut()
        .expect("no collision geometry")
        .check_multiple(p0.x, p0.y, p2.x, p2.y, 1, -1, -1);
    let Some(hit) = hit else {
        jobj::set_translate(&mut r.game.objs.jobjs, t, translate);
        return;
    };
    let p1 = hit.pos;
    let d = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
    let (dx, dy, dz) = (d(p0.x, p2.x), d(p0.y, p2.y), d(p0.z, p2.z));
    let full = sqrt_store(fp::fadds(
        fp::fmuls(dz, dz),
        fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
    ));
    let (ex, ey) = (d(p0.x, p1.x), d(p0.y, p1.y));
    let cut = sqrt_store(fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey)));
    let flat = sqrt_store(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)));
    translate.y = mem(fp::fmuls(full, fp::fneg(fp::fdivs(cut, flat))));
    jobj::set_translate(&mut r.game.objs.jobjs, t, translate);
    let v = item_mut(r.game, gobj).xDD4_itemVar.masterhand_laser();
    let n = v.x8.get().wrapping_sub(1);
    v.x8.set(n);
    if n < 0 {
        let at = world_position(&mut r.game.objs, r.data, Some(t), None);
        crate::game::ef::spawn(r, GFX_SPARK, Some(gobj), &[Arg::Vec(at)]);
        let frames = attr_f32(r, gobj, attr::SPARK_FRAMES);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .masterhand_laser()
            .x8
            .set(fp::fctiwz(reg(frames)));
    }
}

pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => anim(r, gobj),
        COLL_0 => false,
        _ => return None,
    })
}

pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_0 => {}
        ACCESSORY => accessory(r, gobj),
        _ => return false,
    }
    true
}
