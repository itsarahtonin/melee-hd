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

/// Sets a joint's z rotation (`HSD_JObjSetRotationZ`).
pub fn set_rotation_z(game: &mut Game, j: JObjId, z: f32) {
    jobj::set_angle(&mut game.objs.jobjs, j, |q| q.z = z);
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

/// The first word of the kind's own attributes, `it_80272818` (`it_2728_DatAttrs::fighter`:
/// the Starman's invincibility frames).
#[doc(alias = "it_80272818")]
pub fn special_word(game: &Game, data: &Archives, gobj: GObjId) -> i32 {
    let a = article(item(game, gobj))
        .special(data)
        .expect("an item without its attributes");
    data.u32(a) as i32
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

/// The joint its model attaches by (its model's `x8_bone_attach_id`), `it_80272C90`.
#[doc(alias = "it_80272C90")]
pub fn attach_bone(game: &Game, data: &Archives, gobj: GObjId) -> Option<JObjId> {
    let id = article(item(game, gobj)).model(data).bone_attach_id(data);
    bone(game, data, gobj, id)
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
pub fn pickup_sfx(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDCD_flag.x0.b2 {
        let sfx = ip.xD70;
        super::item::play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
}

/// Spawns a destroy effect and plays a destroy sound, `it_8027327C`.
#[doc(alias = "it_8027327C")]
pub fn destroy_effect(r: &mut Running, gobj: GObjId, gfx: i32, sfx: i32) {
    if gfx != -1 {
        super::effect::spawn_simple(r, gobj, gfx);
    }
    if !item(r.game, gobj).xDCD_flag.x0.b2 {
        play_sfx(r, gobj, sfx);
        if sfx == 0x12F {
            code::unported_call("lbAudioAx_80024DC4");
        }
    }
}

/// Plays a sound unless the item is quiet, `it_802732E4`.
#[doc(alias = "it_802732E4")]
pub fn play_sfx(r: &mut Running, gobj: GObjId, sfx: i32) {
    if !item(r.game, gobj).xDCD_flag.x0.b2 {
        super::item::play_sfx(r, gobj, sfx, 0x7F, 0x40);
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

/// The item's landing dust and sound at `offset` from it, `it_80272AC4`: no more counted
/// hits.
// Ports `it_80272AC4`; aliased once a check reaches it.
pub fn landing_dust_at(r: &mut Running, gobj: GObjId, offset: Vec3) {
    let p = item(r.game, gobj).pos;
    let pos = Vec3::new(
        mem(fp::fadds(reg(offset.x), reg(p.x))),
        mem(fp::fadds(reg(offset.y), reg(p.y))),
        mem(fp::fadds(reg(offset.z), reg(p.z))),
    );
    crate::game::ef::spawn(r, 0x40C, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    super::item::play_sfx(r, gobj, 116, 127, 64);
    uncount_hits(r.game, gobj);
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

/// The spun joint's angle about the kind's spin axis, `it_80274990`.
#[doc(alias = "it_80274990")]
pub fn spin_angle(game: &Game, data: &Archives, gobj: GObjId) -> f32 {
    let j = spin_joint(game, data, gobj).expect("an item without a model");
    let r = &game.objs.jobjs[j].rotate;
    match item(game, gobj).xDC8_word.flags.x17 {
        0 => r.z,
        1 => r.x,
        _ => r.y,
    }
}

/// The bounce's sound (`xD84`), unless the item makes none, `it_8027321C`.
#[doc(alias = "it_8027321C")]
pub fn bounce_sfx(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if ip.xDCD_flag.x0.b2 {
        return;
    }
    let sfx = ip.xD84;
    if ip.kind == super::kinds::UNK4 {
        r.sfx_play_track(sfx, 127, 64, 154);
        return;
    }
    super::item::play_sfx(r, gobj, sfx, 127, 64);
}

/// Stops the item's spin and squares its spun joint, `it_80274740`.
#[doc(alias = "it_80274740")]
pub fn stop_spin(game: &mut Game, data: &Archives, gobj: GObjId) {
    let j = spin_joint(game, data, gobj).expect("an item without a model");
    let ip = item_mut(game, gobj);
    ip.xD3C_spinSpeed = 0.0;
    let (axis, v) = (ip.xDC8_word.flags.x17, ip.xD3C_spinSpeed);
    let jobjs = &mut game.objs.jobjs;
    match axis {
        0 => jobj::set_rotation_z(jobjs, j, v),
        1 => jobj::set_rotation_x(jobjs, j, v),
        _ => jobj::set_rotation_y(jobjs, j, v),
    }
}

/// Stops the item, `it_80273454` (`itResetVelocity`).
#[doc(alias = "it_80273454")]
pub fn clear_velocity(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
}

/// Starts the item's vanishing countdown (the common data's `xF8`), `it_8027518C`.
#[doc(alias = "it_8027518C")]
pub fn start_vanishing(game: &mut Game, data: &Archives, gobj: GObjId) {
    let t = common_data(game).f32(data, common::XF8);
    let ip = item_mut(game, gobj);
    ip.xD44_lifeTimer = t;
    set_flag(&mut ip.xDCF_flag, 2, true);
    set_flag(&mut ip.xDD1_flag, 0, true);
    super::lib26b1::stop_keeping_clear(game, gobj);
}

/// Counts a tick off the item's life; whether it ran out, `it_802751D8`.
#[doc(alias = "it_802751D8")]
pub fn tick_life(game: &mut Game, gobj: GObjId) -> bool {
    let ip = item_mut(game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// Gravity: the fall speed grows by `accel` until it reaches `max`, `it_80272860`.
// NaN takes the original's branches, which the negated comparisons spell out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "it_80272860")]
pub fn fall(game: &mut Game, gobj: GObjId, accel: f32, max: f32) {
    let ip = item_mut(game, gobj);
    let sign = |v: f64| if v < 0.0 { -1 } else { 1 };
    let mut vy = reg(ip.x40_vel.y);
    if sign(vy) != sign(reg(accel)) {
        if vy < 0.0 {
            vy = fp::fneg(vy);
        }
        if !(vy < reg(max)) {
            return;
        }
    }
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(accel)));
}

/// Bounces the item off a shield (`xC58`'s normal), turning it the way it now moves,
/// `itColl_BounceOffShield`.
// NaN takes the original's branches, which the negated comparisons spell out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "itColl_BounceOffShield")]
pub fn bounce_off_shield(game: &mut Game, gobj: GObjId) -> bool {
    let ip = item_mut(game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let vx = reg(ip.x40_vel.x);
    let abs = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if !(abs < f64::from(1.0e-5_f32)) || !(reg(ip.facing_dir) != 0.0) {
        ip.facing_dir = if vx >= 0.0 { 1.0 } else { -1.0 };
    }
    let dir = if fp::fneg(1.0) == reg(ip.facing_dir) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(dir);
    false
}

/// Knocked back off what it hit, `itColl_BounceOffVictim`: its speed scaled by the common
/// data's `x58` across and `x5C` up, plus `x60`.
#[doc(alias = "itColl_BounceOffVictim")]
pub fn bounce_off_victim(game: &mut Game, data: &Archives, gobj: GObjId) {
    let c = common_data(game);
    let (sx, sy, up) = (
        reg(c.f32(data, common::X58)),
        reg(c.f32(data, common::X5C)),
        reg(c.f32(data, common::X60)),
    );
    let ip = item_mut(game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), sx));
    ip.x40_vel.y = mem(fp::fmadds(reg(ip.x40_vel.y), sy, up));
}

/// Sent back the way it came by a reflection, at the reflection's speed (`xC70`), with half
/// its life left, `it_80273030`.
#[doc(alias = "it_80273030")]
pub fn reflected_back(game: &mut Game, gobj: GObjId) -> bool {
    let ip = item_mut(game, gobj);
    let k = reg(ip.xC70);
    ip.x40_vel.x = mem(fp::fmuls(fp::fneg(reg(ip.x40_vel.x)), k));
    ip.x40_vel.y = mem(fp::fmuls(fp::fneg(reg(ip.x40_vel.y)), k));
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.xD44_lifeTimer = ip.xD48_halfLifeTimer;
    false
}

/// Sets the half lifetime from `lifetime`, `it_80275174`.
#[doc(alias = "it_80275174")]
pub fn set_half_life(game: &mut Game, data: &Archives, gobj: GObjId, lifetime: f32) {
    let x4c = common_data(game).f32(data, common::X4C);
    item_mut(game, gobj).xD48_halfLifeTimer = mem(fp::fmuls(reg(lifetime), reg(x4c)));
}

/// Faces the way it moves, unless it has all but stopped (and faces some way),
/// `it_80272980`.
// NaN takes the original's branch, which the negated comparison spells out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "it_80272980")]
pub fn face_motion(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let vx = reg(ip.x40_vel.x).abs();
    if !(vx < fp::frsp(0.00001)) || reg(ip.facing_dir) == 0.0 {
        ip.facing_dir = if reg(ip.x40_vel.x) >= 0.0 { 1.0 } else { -1.0 };
    }
    let dir = if reg(ip.facing_dir) == -1.0 { -1 } else { 1 };
    ip.x378_itemColl.set_facing_dir(dir);
}

/// Lets fighters jump on it, `it_80274CAC` (showing the range with the debug menu's
/// `ShowEnemyStompRange`).
#[doc(alias = "it_80274CAC")]
pub fn make_stompable(game: &mut Game, gobj: GObjId) {
    let show = game.it.db_menu.ShowEnemyStompRange;
    let ip = item_mut(game, gobj);
    set_flag(&mut ip.xDD0_flag, 0, true);
    if show {
        set_flag(&mut ip.xDAA, 3, true);
    }
}

/// The new item's physics and collision run once, as if a frame passed, `it_802750F8`. They
/// can free the item (a Blizzard puff spawned into the stage); the original then sets the
/// flag in the freed block, which the block's next item keeps (`Pool::alloc_stale`).
#[doc(alias = "it_802750F8")]
pub fn first_frame(r: &mut Running, gobj: GObjId) {
    let id = super::item_of(r.game, gobj);
    r.game.items[id.0].xDCC_flag.b3 = false;
    super::item::proc_phys(r, gobj);
    super::item::proc_coll(r, gobj);
    r.game.items[id.0].xDCC_flag.b3 = true;
}

/// Turns hitbox `idx` on where its joint is now, `it_80272674`.
#[doc(alias = "it_80272674")]
pub fn start_hitbox(game: &mut Game, data: &Archives, gobj: GObjId, idx: usize) {
    let ip = item_mut(game, gobj);
    hitcapsule::enable(&mut ip.x5D4_hitboxes[idx].hit);
    ip.xDC8_word.flags.x16 = true;
    super::coll::update_hitbox(game, data, gobj, idx);
}

/// Marks the item holding still for its animation (`xDC8_word.x2`), `it_80274F10`.
#[doc(alias = "it_80274F10")]
pub fn hold_still(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x2 = true;
}

/// Whether the model still plays an animation, `it_80272C6C`.
#[doc(alias = "it_80272C6C")]
pub fn is_animating(game: &Game, gobj: GObjId) -> bool {
    crate::lb::anim::any_animated(&game.objs, jobj_of(game, gobj))
}

/// Resizes the item to `scale`: its model, hitboxes (to `x3C`) and ranges, `it_80274484`.
#[doc(alias = "it_80274484")]
pub fn rescale(game: &mut Game, gobj: GObjId, j: JObjId, scale: f32) {
    item_mut(game, gobj).scl = scale;
    set_uniform_scale(game, j, scale);
    let x3c = item(game, gobj).x3C;
    super::hitbox::set_hit_scale(game, gobj, x3c);
    scale_ranges(game, gobj);
    scale_boxes(game, gobj);
}

/// Scales the pickup ranges by the item's scale, `it_80274DFC`.
#[doc(alias = "it_80274DFC")]
pub fn scale_ranges(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let s = reg(ip.scl);
    for v in [&mut ip.xBCC_unk, &mut ip.xBD4_grabRange] {
        v.x = mem(fp::fmuls(reg(v.x), s));
        v.y = mem(fp::fmuls(reg(v.y), s));
    }
}

/// Scales the two boxes `xBEC` and `xBDC` by the item's scale, `it_80274E44`.
#[doc(alias = "it_80274E44")]
pub fn scale_boxes(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let s = reg(ip.scl);
    for b in [&mut ip.xBEC, &mut ip.xBDC] {
        b.top = mem(fp::fmuls(reg(b.top), s));
        b.bottom = mem(fp::fmuls(reg(b.bottom), s));
        b.right = mem(fp::fmuls(reg(b.right), s));
        b.left = mem(fp::fmuls(reg(b.left), s));
    }
}

/// Sets `xD09`, `it_80274ECC`.
#[doc(alias = "it_80274ECC")]
pub fn set_xd09(game: &mut Game, gobj: GObjId, on: bool) {
    item_mut(game, gobj).xD09 = u8::from(on);
}

/// Arms the item to grab a fighter (`xDD0_flag.b5`), `it_80274F28`: fighters whose `x1A6A`
/// shares a bit with `mask` are passed over; `dealt` runs on the item when it grabs one, then
/// `victim` on the fighter.
#[doc(alias = "it_80274F28")]
pub fn arm_grab(
    ip: &mut crate::game::structs::Item,
    mask: u8,
    dealt: crate::game::handles::CodeRef,
    victim: crate::game::handles::CodeRef,
) {
    super::set_flag(&mut ip.xDD0_flag, 5, true);
    ip.xD08 = mask;
    ip.grab_dealt = Some(dealt);
    ip.grabbed_for_victim = Some(victim);
}

/// Counts an item the spawner dropped, `it_80274ED8`.
#[doc(alias = "it_80274ED8")]
pub fn count_spawned(game: &mut Game) {
    game.it.spawned = game.it.spawned.wrapping_add(1);
}

/// Counts a food in (hold kind 2), `it_80274EE8`.
#[doc(alias = "it_80274EE8")]
pub fn count_food(game: &mut Game) {
    game.it.x6d08 = game.it.x6d08.wrapping_add(1);
}

/// A small blast, `it_80272B40`: its effect (0x40C), its sound, and its hits counted afresh.
#[doc(alias = "it_80272B40")]
pub fn blast_small(r: &mut Running, gobj: GObjId) {
    blast_with(r, gobj, 0x40C);
}

/// A large blast, `it_80272A60`: its effect (0x40E), its sound, and its hits counted afresh.
#[doc(alias = "it_80272A60")]
pub fn blast_large(r: &mut Running, gobj: GObjId) {
    blast_with(r, gobj, 0x40E);
}

fn blast_with(r: &mut Running, gobj: GObjId, gfx: i32) {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, gfx, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    super::item::play_sfx(r, gobj, 116, 127, 64);
    uncount_hits(r.game, gobj);
}

/// The blast of an exploding item, `it_80272C08`: its effect (0x410), its sound, and its hits
/// counted afresh.
#[doc(alias = "it_80272C08")]
pub fn blast_effect(r: &mut Running, gobj: GObjId) {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, 0x410, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    super::item::play_sfx(r, gobj, 116, 127, 64);
    uncount_hits(r.game, gobj);
}

/// Starts a fighter's item rumble (id 1) of kind `kind` for `frames` if its controller rumbles
/// and it's awake and alive (`ftLib_StartItemRumble`).
#[doc(alias = "ftLib_StartItemRumble")]
fn fighter_item_rumble(r: &mut Running, fighter: GObjId, kind: i32, frames: i32) {
    let f = super::owner::fighter_of(r.game, fighter);
    let fp = &r.game.fighters[f.0];
    if !r
        .game
        .rumble_enabled(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
    {
        return;
    }
    if !fp.is_sleeping && !fp.stamina_dead {
        let port = fp.pad_port;
        r.rumble_start(port, 1, kind, frames);
    }
}

/// Rumbles the holder's controller, `it_80273598`.
#[doc(alias = "it_80273598")]
pub fn rumble_owner(r: &mut Running, gobj: GObjId, kind: i32, frames: i32) {
    let owner = item(r.game, gobj).owner;
    if let Some(o) = owner
        && super::item::is_fighter(r.game, owner)
    {
        fighter_item_rumble(r, o, kind, frames);
    }
}

/// Stops the holder's item rumble, `it_80273600` (`ftLib_StopItemRumble`).
#[doc(alias = "it_80273600")]
#[doc(alias = "ftLib_StopItemRumble")]
pub fn stop_owner_rumble(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj).owner;
    if let Some(o) = owner
        && super::item::is_fighter(r.game, owner)
    {
        let f = super::owner::fighter_of(r.game, o);
        let fp = &r.game.fighters[f.0];
        if r.game
            .rumble_enabled(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
        {
            let port = fp.pad_port;
            r.game.pads.rumble.remove_id(port, 1);
        }
    }
}

/// Rumbles every fighter's controller, `it_80273648` (`ftLib_StartItemRumbleAll`).
#[doc(alias = "it_80273648")]
#[doc(alias = "ftLib_StartItemRumbleAll")]
pub fn rumble_all(r: &mut Running, kind: i32, frames: i32) {
    let mut cur = r.game.world.first_in(super::owner::P_LINK_FIGHTER);
    while let Some(g) = cur {
        cur = r.game.world.gobjs[g.0].next;
        fighter_item_rumble(r, g, kind, frames);
    }
}

/// Hides a joint tree, `it_80272A3C`.
#[doc(alias = "it_80272A3C")]
pub fn hide_tree(game: &mut Game, j: Option<JObjId>) {
    jobj::set_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
}

/// Shows a joint tree, `it_80272A18`.
#[doc(alias = "it_80272A18")]
pub fn show_tree(game: &mut Game, j: Option<JObjId>) {
    jobj::clear_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
}

/// The item scaled by its owner's size (`ftLib_GetScale`), its model, hitboxes and ranges
/// with it, `it_80274594`.
#[doc(alias = "it_80274594")]
pub fn scale_by_owner(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj).owner.expect("an item without an owner");
    let scale = super::owner::fighter(r.game, owner).x34_scale.y;
    let ip = item_mut(r.game, gobj);
    ip.scl = mem(fp::fmuls(reg(ip.scl), reg(scale)));
    let scl = ip.scl;
    let j = super::jobj_of(r.game, gobj).expect("an item without a model");
    set_uniform_scale(r.game, j, scl);
    let x3c = item(r.game, gobj).x3C;
    super::hitbox::set_hit_scale(r.game, gobj, x3c);
    scale_ranges(r.game, gobj);
    scale_boxes(r.game, gobj);
}
