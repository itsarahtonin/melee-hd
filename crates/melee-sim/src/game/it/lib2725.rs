//! Item helpers from `melee/it/it_2725.c`: hitboxes on and off, motion and spin, lifetimes,
//! the model's visibility and scale, and holding.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::data::common;
use super::item::{article, common_data};
use super::{item, item_mut, jobj_of, set_flag};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::hitcapsule;
use crate::vec::Vec3;

/// Sets a joint's y rotation (the inline `HSD_JObjSetRotationY`, which asserts the joint has
/// no quaternion).
pub fn set_rotation_y(game: &mut Game, j: JObjId, y: f32) {
    jobj::set_angle(&mut game.objs.jobjs, j, |q| q.y = y);
}

/// Sets a joint's x rotation (`HSD_JObjSetRotationX`).
pub fn set_rotation_x(game: &mut Game, j: JObjId, x: f32) {
    jobj::set_angle(&mut game.objs.jobjs, j, |q| q.x = x);
}

/// Sets a joint's z scale (`HSD_JObjSetScaleZ`).
pub fn set_scale_z(game: &mut Game, j: JObjId, z: f32) {
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.scale.z = z);
}

/// Turns one hitbox off, `it_80272560`, and brings every hitbox's position up to date.
#[doc(alias = "it_80272560")]
pub fn clear_hitbox(game: &mut Game, data: &Archives, gobj: GObjId, idx: usize) {
    let ip = item_mut(game, gobj);
    hitcapsule::disable(&mut ip.x5D4_hitboxes[idx].hit);
    set_flag(&mut ip.xDAA, 2, false);
    for i in 0..4 {
        super::coll::update_hitbox(game, data, gobj, i);
    }
}

/// Turns every hitbox off, `it_802725D4`.
#[doc(alias = "it_802725D4")]
pub fn clear_hitboxes(game: &mut Game, data: &Archives, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    for h in &mut ip.x5D4_hitboxes {
        hitcapsule::disable(&mut h.hit);
    }
    for i in 0..4 {
        super::coll::update_hitbox(game, data, gobj, i);
    }
    let ip = item_mut(game, gobj);
    ip.xDC8_word.flags.x16 = false;
    set_flag(&mut ip.xDAA, 2, false);
}

/// Shows the model when its lifetime count's tick isn't a multiple of `x44`, else hides it:
/// the blink of an item about to vanish, `it_802728C8`.
#[doc(alias = "it_802728C8")]
pub fn flicker(game: &mut Game, data: &Archives, gobj: GObjId) {
    let root = jobj_of(game, gobj);
    let first = root.and_then(|j| game.objs.jobjs[j].child);
    let period = common_data(game).u32(data, common::X44);
    let life = fp::fctiwz(reg(item(game, gobj).xD44_lifeTimer)) as u32;
    if !life.is_multiple_of(period) {
        jobj::clear_flags_all(&mut game.objs.jobjs, first, jobj::HIDDEN);
    } else {
        jobj::set_flags_all(&mut game.objs.jobjs, first, jobj::HIDDEN);
    }
}

/// Shows the model, `it_80272940`.
#[doc(alias = "it_80272940")]
pub fn show(game: &mut Game, gobj: GObjId) {
    let root = jobj_of(game, gobj);
    let first = root.and_then(|j| game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut game.objs.jobjs, first, jobj::HIDDEN);
}

/// Sets a joint's scale in all three axes, `it_80272F7C`.
#[doc(alias = "it_80272F7C")]
pub fn set_uniform_scale(game: &mut Game, j: JObjId, scale: f32) {
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.scale = Vec3::new(scale, scale, scale)
    });
}

/// The joint a model's index names, `it_80272CC0`: from the bone table, or the `idx`th
/// first child down.
#[doc(alias = "it_80272CC0")]
pub fn bone(game: &Game, data: &Archives, gobj: GObjId, idx: i32) -> Option<JObjId> {
    let ip = item(game, gobj);
    let mut j = jobj_of(game, gobj);
    if article(ip).model(data).bone_count(data) != 0 {
        let t = ip
            .xBBC_dynamicBoneTable
            .expect("an item without a bone table");
        return game.it.bone_tables[t.0].bones[idx as usize];
    }
    for _ in 0..idx.max(0) {
        j = j.and_then(|j| game.objs.jobjs[j].child);
    }
    j
}

/// Counts the lifetime down: whether it ran out, `it_80273130`.
#[doc(alias = "it_80273130")]
pub fn tick_lifetime(game: &mut Game, gobj: GObjId) -> bool {
    let ip = item_mut(game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// Plays the pickup sound, `it_80273168`.
#[doc(alias = "it_80273168")]
pub fn pickup_sfx(game: &mut Game, gobj: GObjId) {
    let ip = item(game, gobj);
    if !ip.xDCD_flag.x0.b2 {
        let sfx = ip.xD70;
        super::item::play_sfx(game, gobj, sfx, 0x7F, 0x40);
    }
}

/// Spawns a destroy effect and plays a destroy sound, `it_8027327C`.
#[doc(alias = "it_8027327C")]
pub fn destroy_effect(r: &mut Running, gobj: GObjId, gfx: i32, sfx: i32) {
    if gfx != -1 {
        super::effect::spawn_simple(r, gobj, gfx);
    }
    if !item(r.game, gobj).xDCD_flag.x0.b2 {
        play_sfx(r.game, gobj, sfx);
        if sfx == 0x12F {
            code::unported_call("lbAudioAx_80024DC4");
        }
    }
}

/// Plays a sound unless the item is quiet, `it_802732E4`.
#[doc(alias = "it_802732E4")]
pub fn play_sfx(game: &mut Game, gobj: GObjId, sfx: i32) {
    if !item(game, gobj).xDCD_flag.x0.b2 {
        super::item::play_sfx(game, gobj, sfx, 0x7F, 0x40);
    }
}

/// Clears the floor's motion, `it_8027346C`.
#[doc(alias = "it_8027346C")]
pub fn clear_x64(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).x64_vec_unk2 = Vec3::new(0.0, 0.0, 0.0);
}

/// Clears `x58`, `it_80273484`.
#[doc(alias = "it_80273484")]
pub fn clear_x58(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).x58_vec_unk = Vec3::new(0.0, 0.0, 0.0);
}

/// Clears the nudge, `it_8027349C`.
#[doc(alias = "it_8027349C")]
pub fn clear_nudge(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).x70_nudge = Vec3::new(0.0, 0.0, 0.0);
}

/// Clears the knockback vectors, `it_802734B4`.
#[doc(alias = "it_802734B4")]
pub fn clear_knockback(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.x7C = Vec3::new(0.0, 0.0, 0.0);
    ip.x88 = Vec3::new(0.0, 0.0, 0.0);
    ip.x94 = Vec3::new(0.0, 0.0, 0.0);
    ip.xA0 = Vec3::new(0.0, 0.0, 0.0);
    ip.xAC_unk.x = 0.0;
    ip.xAC_unk.z = 0.0;
    ip.xAC_unk.y = 1.0;
}

/// Sets the velocity and clears the other motion, `it_80273500`.
#[doc(alias = "it_80273500")]
pub fn set_velocity(game: &mut Game, gobj: GObjId, v: Vec3) {
    item_mut(game, gobj).x40_vel = v;
    clear_x64(game, gobj);
    clear_x58(game, gobj);
    clear_nudge(game, gobj);
    clear_knockback(game, gobj);
}

/// Sets the spin from the kind's spin speed and `arg` degrees, turned to the motion's way,
/// `it_80274658`; returns the motion's direction.
#[doc(alias = "it_80274658")]
pub fn clamp_fall(game: &mut Game, gobj: GObjId, arg: f32) -> i32 {
    let ip = item_mut(game, gobj);
    let mut spin = 0.0;
    let s = reg(ip.spin_spd);
    if s != spin {
        spin = fp::fmuls(
            fp::fmuls(f64::from(0.01_f32), s),
            fp::fmuls(f64::from(0.017453292_f32), reg(arg)),
        );
    }
    ip.xD3C_spinSpeed = mem(spin);
    let facing = if reg(ip.facing_dir) < 0.0 { -1 } else { 1 };
    let moving = if reg(ip.x40_vel.x) < 0.0 { -1 } else { 1 };
    if moving != facing {
        ip.xD3C_spinSpeed = mem(fp::fneg(reg(ip.xD3C_spinSpeed)));
    }
    if !ip.xDC8_word.flags.x19 {
        ip.xD3C_spinSpeed = mem(fp::fmuls(
            reg(ip.xD3C_spinSpeed),
            fp::fneg(reg(ip.facing_dir)),
        ));
    }
    moving
}

/// The joint the item spins, `it_802746F8`: the root, or a first child down as many levels as
/// the model's bit field says.
#[doc(alias = "it_802746F8")]
pub fn spin_joint(game: &Game, data: &Archives, gobj: GObjId) -> Option<JObjId> {
    let mut j = jobj_of(game, gobj);
    let levels = article(item(game, gobj)).model(data).bit_field(data) >> 6 & 3;
    for _ in 0..levels {
        j = j.and_then(|j| game.objs.jobjs[j].child);
    }
    j
}

/// Turns the spun joint by the spin, about the kind's axis, `it_80274A64` (the inline
/// `HSD_JObjAddRotationZ` and friends).
#[doc(alias = "it_80274A64")]
pub fn spin(game: &mut Game, data: &Archives, gobj: GObjId) {
    let j = spin_joint(game, data, gobj).expect("an item without a model");
    let ip = item(game, gobj);
    let (axis, s) = (ip.xDC8_word.flags.x17, reg(ip.xD3C_spinSpeed));
    jobj::set_angle(&mut game.objs.jobjs, j, |q| {
        let r = match axis {
            0 => &mut q.z,
            1 => &mut q.x,
            _ => &mut q.y,
        };
        *r = mem(fp::fadds(reg(*r), s));
    });
}

/// Turns off the hit-counting flag, `it_80274C60`.
#[doc(alias = "it_80274C60")]
pub fn uncount_hits(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.xC = false;
}

/// Whether the item's hits count toward its hitlag, `it_80274C78`.
#[doc(alias = "it_80274C78")]
pub fn is_hit_counted(game: &Game, gobj: GObjId) -> bool {
    item(game, gobj).xDC8_word.flags.xC
}

/// Makes the item's joint `bone` follow fighter `holder`'s part `part`, `it_80274F48`.
#[doc(alias = "it_80274F48")]
pub fn attach_to_part(
    game: &mut Game,
    data: &Archives,
    gobj: GObjId,
    bone: i32,
    holder: GObjId,
    part: i32,
) {
    let j = self::bone(game, data, gobj, bone);
    let f = super::owner::fighter_of(game, holder);
    let parts = game.fighters[f.0].parts.expect("a fighter without parts");
    let target = game.fighter_parts[parts.0][part as usize].joint;
    crate::lb::anim::attach(&mut game.objs, j, target);
}

/// Clears the stomp flags, `it_80274C88`.
#[doc(alias = "it_80274C88")]
pub fn clear_stomp(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    set_flag(&mut ip.xDD0_flag, 0, false);
    set_flag(&mut ip.xDAA, 3, false);
}

/// The ECB's center offset this tick from last (`it_80274D04`), into `xB54`.
#[doc(alias = "it_80274D04")]
pub fn ecb_motion(game: &mut Game, gobj: GObjId) {
    let a = super::lib26b1::ecb_center(game, gobj);
    let b = super::lib26b1::last_ecb_center(game, gobj);
    let ip = item_mut(game, gobj);
    ip.xB54.unk_x = mem(fp::fsubs(reg(a.x), reg(b.x)));
    ip.xB54.unk_y = mem(fp::fsubs(reg(a.y), reg(b.y)));
}

/// The ECB's extent in the world, into `xB54.x8`, `it_80274D6C`.
#[doc(alias = "it_80274D6C")]
pub fn ecb_extent(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let e = &ip.x378_itemColl.ecb;
    let (right_x, right_y, left_x, top_y) = (e.right.x, e.right.y, e.left.x, e.top.y);
    ip.xB54.x8.y = mem(fp::fadds(reg(right_x), reg(ip.pos.x)));
    ip.xB54.x8.x = mem(fp::fadds(reg(left_x), reg(ip.pos.x)));
    ip.xB54.x8.z = mem(fp::fadds(
        reg(top_y),
        fp::fadds(reg(right_y), reg(ip.pos.y)),
    ));
}

/// Clears the ECB motion and sets the extent, `it_80274DAC`.
#[doc(alias = "it_80274DAC")]
pub fn init_spin(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.xB54.unk_x = 0.0;
    ip.xB54.unk_y = 0.0;
    ecb_extent(game, gobj);
}

/// Clears the attack id flag, `it_80274EF8`.
#[doc(alias = "it_80274EF8")]
pub fn clear_attack_id(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x2 = false;
}

/// Sets the lifetime and the half lifetime it reflects with, `it_80275158`.
#[doc(alias = "it_80275158")]
pub fn set_lifetime(game: &mut Game, data: &Archives, gobj: GObjId, lifetime: f32) {
    let x4c = common_data(game).f32(data, common::X4C);
    let ip = item_mut(game, gobj);
    ip.xD44_lifeTimer = lifetime;
    ip.xD48_halfLifeTimer = mem(fp::fmuls(reg(lifetime), reg(x4c)));
}

/// Sets the damage taken and clears the record, `it_802753BC`.
#[doc(alias = "it_802753BC")]
pub fn set_damage_taken(game: &mut Game, gobj: GObjId, damage: i16) {
    let ip = item_mut(game, gobj);
    ip.xC94 = 0;
    ip.xC9C = i32::from(damage);
    ip.xCA0 = 0;
    ip.xCA4 = 0;
}

/// Lets go of its owner and stops being pickable, `it_80275390`.
#[doc(alias = "it_80275390")]
pub fn drop_owner(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).owner = None;
    super::lib26b1::disable_pickup(game, gobj);
}
