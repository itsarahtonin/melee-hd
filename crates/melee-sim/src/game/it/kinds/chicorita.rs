//! Chikorita (`It_PKind_Chicorita`), `melee/it/kinds/itchicorita.c`: out of the ball it lands
//! facing its owner's nearest opponent and fires Razor Leaf a number of times, each leaf (its
//! script says when) an item flying ahead, then leaves.
//!
//! The unit also holds the leaf's code (`It_Kind_Chicorita_Leaf`): it flies straight until its
//! life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::ground::{air_touched_any, fall, ground_or_fall};
use super::super::item::{attr_of, change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32};
use super::super::{data, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_9588;
pub const LANDED_NOTHING: u32 = 0x802C_9618;
pub const EVT_UNK: u32 = 0x802C_961C;
pub const ANIM0: u32 = 0x802C_96CC;
pub const PHYS0: u32 = 0x802C_9798;
pub const COLL0: u32 = 0x802C_9850;
pub const FIRE_ACCESSORY: u32 = 0x802C_989C;
pub const ANIM1: u32 = 0x802C_990C;
pub const PHYS1: u32 = 0x802C_9970;
pub const COLL1: u32 = 0x802C_9A28;
pub const ANIM2: u32 = 0x802C_9AB4;
pub const PHYS2: u32 = 0x802C_9AD8;
pub const COLL2: u32 = 0x802C_9AF8;
pub const LEAF_SPAWNED: u32 = 0x802C_9C20;
pub const LEAF_KEPT: u32 = 0x802C_9C78;
pub const LEAF_EVT_UNK: u32 = 0x802C_9C80;
pub const LEAF_REFLECTED: u32 = 0x802C_9CA0;
pub const LEAF_ANIM: u32 = 0x802C_9CE8;
pub const LEAF_PHYS: u32 = 0x802C_9D1C;
pub const LEAF_COLL: u32 = 0x802C_9D20;

/// `It_Kind_Chicorita_Leaf`.
pub const CHICORITA_LEAF: i32 = 0xBF;

/// Its variables past the release's (`itChicorita_ItemVars`): the leaves left (`x60`, -1
/// until the first) and its own fall speed (`x64`).
const SHOTS: usize = 0x60;
const FALL_SPEED: usize = 0x64;

/// Runs one of Chikorita's (or its leaf's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL1 => coll(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        LEAF_KEPT => false,
        LEAF_REFLECTED => leaf_reflected(r, gobj),
        LEAF_ANIM => leaf_anim(r, gobj),
        LEAF_COLL => leaf_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Chikorita's (or its leaf's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING | LEAF_PHYS => {}
        PHYS0 | PHYS1 => phys(r, gobj),
        FIRE_ACCESSORY => fire_when_told(r, gobj),
        PHYS2 => phys2(r, gobj),
        LEAF_SPAWNED => leaf_spawned(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802C9588`, its spawn: facing its owner's nearest opponent, no leaves yet, its script's
/// flag clear, at its first size, not falling, into the air out of the ball, its sound.
#[doc(alias = "it_802C9588")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    pokemon::face_nearest_opponent(r, gobj);
    pokemon::vars(r.game, gobj).set_i32(SHOTS, -1);
    item_mut(r.game, gobj).xDBC[0] &= !0x80;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
    enter_air(r, gobj);
    play_sfx(r, gobj, 0x2711, 0x7F, 0x40);
}

/// Landing in its states, `it_802C9618`: nothing.
#[doc(alias = "it_802C9618")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `itChicorita_Logic1_EvtUnk` and the leaf's
/// `itChicoritaLeaf_Logic30_EvtUnk` (`it_8026B894`).
#[doc(alias = "itChicorita_Logic1_EvtUnk")]
#[doc(alias = "itChicoritaLeaf_Logic30_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802C963C`: still.
#[doc(alias = "it_802C963C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    fire(r, gobj);
}

/// Fires, `it_802C9670` (and its inline copy in `itChicorita_UnkMotion0_Anim`): the first
/// time, its leaves from its attributes; firing when its script says.
#[doc(alias = "it_802C9670")]
pub fn fire(r: &mut Running, gobj: GObjId) {
    if pokemon::vars(r.game, gobj).i32(SHOTS) == -1 {
        let shots = attr_i32(r, gobj, 4);
        pokemon::vars(r.game, gobj).set_i32(SHOTS, shots);
    }
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FIRE_ACCESSORY));
}

/// Its root joint's child's motion taken as its own.
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// `itChicorita_UnkMotion0_Anim`: at each leaf's end, fires again, or with none left leaves.
#[doc(alias = "itChicorita_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let shots = v.i32(SHOTS);
        if shots <= 0 {
            v.set_i32(SHOTS, 0);
            leave(r, gobj);
        } else {
            v.set_i32(SHOTS, shots.wrapping_sub(1));
            fire(r, gobj);
        }
    }
    false
}

/// `itChicorita_UnkMotion0_Phys` and `_1_Phys` (the inline `itChicorita_Phys`): the bone's
/// motion; in the air it falls by its own speed, sped up by its gravity unless already past
/// the limit against it; on the floor it doesn't fall.
#[doc(alias = "itChicorita_UnkMotion0_Phys")]
#[doc(alias = "itChicorita_UnkMotion1_Phys")]
pub fn phys(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    if item(r.game, gobj).ground_or_air != 1 {
        pokemon::vars(r.game, gobj).set_f32(FALL_SPEED, 0.0);
        return;
    }
    let a = attr_of(item(r.game, gobj));
    let (g, max) = (
        a.f32(r.data, data::attr::FALL_SPEED),
        a.f32(r.data, data::attr::FALL_SPEED_MAX),
    );
    let g_dir = if reg(g) < 0.0 { -1 } else { 1 };
    let v = pokemon::vars(r.game, gobj);
    let mut s = reg(v.f32(FALL_SPEED));
    let s_dir = if s < 0.0 { -1 } else { 1 };
    let speed_up = if s_dir != g_dir {
        if s < 0.0 {
            s = fp::fneg(s);
        }
        s < reg(max)
    } else {
        true
    };
    if speed_up {
        let s = v.f32(FALL_SPEED);
        v.set_f32(FALL_SPEED, mem(fp::fsubs(reg(s), reg(g))));
    }
    let s = v.f32(FALL_SPEED);
    item_mut(r.game, gobj).x40_vel.y = s;
}

/// Lands in the air, falls off the floor on it, doing nothing either way
/// (`itChicorita_UnkMotion0_Coll`, `_1_Coll`).
#[doc(alias = "itChicorita_UnkMotion0_Coll")]
#[doc(alias = "itChicorita_UnkMotion1_Coll")]
pub fn coll(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        fall(r, gobj, landed_nothing);
    } else {
        ground_or_fall(r, gobj, landed_nothing);
    }
    false
}

/// Its `on_accessory`, `it_802C989C`: fires a leaf when its script says.
#[doc(alias = "it_802C989C")]
pub fn fire_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDBC[0] & 0x80 != 0 {
        shoot(r, gobj);
        item_mut(r.game, gobj).xDBC[0] &= !0x80;
    }
}

/// Leaves, `it_802C98E4`.
#[doc(alias = "it_802C98E4")]
pub fn leave(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itChicorita_UnkMotion1_Anim`: gone when its animation ends.
#[doc(alias = "itChicorita_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// Into the air out of the ball, `it_802C9A74`.
#[doc(alias = "it_802C9A74")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itChicorita_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itChicorita_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itChicorita_UnkMotion2_Phys`: falling out of the ball.
#[doc(alias = "itChicorita_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itChicorita_UnkMotion2_Coll`: lands out of the ball.
#[doc(alias = "itChicorita_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Fires a leaf, `it_802C9B20`: ahead of and above it, flying its way, with a sound.
#[doc(alias = "it_802C9B20")]
pub fn shoot(r: &mut Running, gobj: GObjId) {
    let (ahead, up, speed) = (
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 0xC),
        attr_f32(r, gobj, 0x10),
    );
    let ip = item(r.game, gobj);
    let (pos, facing, owner) = (ip.pos, reg(ip.facing_dir), ip.owner);
    let mut prev = pos;
    prev.x = mem(fp::fmadds(reg(ahead), facing, reg(prev.x)));
    prev.y = mem(fp::fadds(reg(prev.y), reg(up)));
    let mut spawn = SpawnItem {
        kind: CHICORITA_LEAF,
        ..Default::default()
    };
    spawn.prev_pos = prev;
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.facing_dir = mem(facing);
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(mem(fp::fmuls(reg(speed), facing)), 0.0, 0.0);
    spawn.x40 = 0;
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    if spawn_air(r, &mut spawn).is_some() {
        play_sfx(r, gobj, 0x2710, 0x7F, 0x40);
    }
}

/// The leaf's `itChicoritaLeaf_Logic30_Spawned`: its life, not spinning, flying, not
/// pickable, its hits not counted.
#[doc(alias = "itChicoritaLeaf_Logic30_Spawned")]
pub fn leaf_spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    leaf_fly(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::uncount_hits(r.game, gobj);
}

/// The leaf's `itChicoritaLeaf_Logic30_HitShield` (also its damage dealt, clank and
/// absorption): kept.
#[doc(alias = "itChicoritaLeaf_Logic30_HitShield")]
pub fn leaf_kept(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The leaf's `itChicoritaLeaf_Logic30_Reflected`: reflected back.
#[doc(alias = "itChicoritaLeaf_Logic30_Reflected")]
pub fn leaf_reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// The leaf flies, `it_802C9CC0`.
#[doc(alias = "it_802C9CC0")]
pub fn leaf_fly(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The leaf's `itChicoritaleaf_UnkMotion0_Anim`: gone once its life is out, else one less.
#[doc(alias = "itChicoritaleaf_UnkMotion0_Anim")]
pub fn leaf_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// The leaf's `itChicoritaleaf_UnkMotion0_Phys`: nothing.
#[doc(alias = "itChicoritaleaf_UnkMotion0_Phys")]
pub fn leaf_phys(_: &mut Running, _: GObjId) {}

/// The leaf's `itChicoritaleaf_UnkMotion0_Coll`: gone when it touches the stage.
#[doc(alias = "itChicoritaleaf_UnkMotion0_Coll")]
pub fn leaf_coll(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}
