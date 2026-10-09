//! The bob-omb, `melee/it/kinds/itbombhei.c`: it lies still a while, then stands up and walks,
//! turning at ledges and walls, until its fuse (`xDF0`) runs out or it touches something, when
//! it flashes and explodes. Its walking pose is two values on bone 11 that the state changes
//! keep.

use gekko_fp as fp;

use super::super::ground::{fall, fall_to_floor, ground_or_fall, ground_skid, ground_to_ledge};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itBombHei_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8027_D76C;
pub const FUSE: u32 = 0x8027_D820;
pub const LANDED: u32 = 0x8027_DAC8;
pub const ANIM0: u32 = 0x8027_DC74;
pub const PHYS0: u32 = 0x8027_DDE8;
pub const COLL0: u32 = 0x8027_DDEC;
pub const FELL: u32 = 0x8027_DE18;
pub const ANIM1: u32 = 0x8027_E018;
pub const PHYS1: u32 = 0x8027_E020;
pub const COLL1: u32 = 0x8027_E050;
pub const PICKED_UP: u32 = 0x8027_E0B4;
pub const ANIM8: u32 = 0x8027_E3E4;
pub const PHYS8: u32 = 0x8027_E644;
pub const DROPPED: u32 = 0x8027_E648;
pub const STAND_UP: u32 = 0x8027_E978;
pub const ANIM3: u32 = 0x8027_EB7C;
pub const PHYS3: u32 = 0x8027_EDC0;
pub const COLL3: u32 = 0x8027_EDC4;
pub const WALK: u32 = 0x8027_EE04;
pub const ANIM2: u32 = 0x8027_EFD0;
pub const PHYS2: u32 = 0x8027_F26C;
pub const COLL2: u32 = 0x8027_F270;
pub const TURN: u32 = 0x8027_F42C;
pub const ANIM4: u32 = 0x8027_F5E8;
pub const PHYS4: u32 = 0x8027_F89C;
pub const COLL4: u32 = 0x8027_F8A0;
pub const LIGHT: u32 = 0x8027_F8E0;
pub const ANIM5: u32 = 0x8027_FC44;
pub const PHYS5: u32 = 0x8027_FC78;
pub const COLL5: u32 = 0x8027_FC7C;
pub const LIGHT_AIR: u32 = 0x8027_FCA8;
pub const ANIM6: u32 = 0x8028_000C;
pub const PHYS6: u32 = 0x8028_0030;
pub const LANDED_LIT: u32 = 0x8028_007C;
pub const COLL6: u32 = 0x8028_0338;
pub const THROWN: u32 = 0x8028_0380;
pub const ANIM10: u32 = 0x8028_06CC;
pub const PHYS10: u32 = 0x8028_0928;
pub const LANDED_THROWN: u32 = 0x8028_0974;
pub const COLL10: u32 = 0x8028_0B18;
pub const EXPLODE: u32 = 0x8028_0B60;
pub const DMG_DEALT: u32 = 0x8028_0DF0;
pub const DMG_RECEIVED: u32 = 0x8028_0E30;
pub const ANIM11: u32 = 0x8028_0E70;
pub const PHYS11: u32 = 0x8028_0E90;
pub const COLL11: u32 = 0x8028_0E94;
pub const ENTERED_AIR: u32 = 0x8028_0E9C;
pub const ANIM12: u32 = 0x8028_1030;
pub const PHYS12: u32 = 0x8028_1038;
pub const COLL12: u32 = 0x8028_103C;
pub const CLANKED: u32 = 0x8028_1070;
pub const REFLECTED: u32 = 0x8028_10A4;
pub const HIT_SHIELD: u32 = 0x8028_10C4;
pub const SHIELD_BOUNCED: u32 = 0x8028_1104;

/// Runs one of the bob-omb's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 | ANIM12 => still(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM8 => anim8(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 | COLL4 => coll3(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM4 => anim4(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        COLL10 => coll10(r, gobj),
        ANIM10 => anim10(r, gobj),
        DMG_DEALT | DMG_RECEIVED => dmg(r, gobj),
        ANIM11 => anim11(r, gobj),
        COLL11 => coll11(r, gobj),
        COLL12 => coll12(r, gobj),
        CLANKED => clanked(r, gobj),
        REFLECTED => reflected(r, gobj),
        HIT_SHIELD | SHIELD_BOUNCED => hit_shield(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bob-omb's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        FUSE => fuse(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS3 | PHYS2 | PHYS4 | PHYS5 | PHYS8 | PHYS11 | PHYS12 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => phys1(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        STAND_UP => stand_up(r, gobj),
        WALK => walk(r, gobj),
        TURN => turn(r, gobj),
        LIGHT => light(r, gobj),
        LIGHT_AIR => light_air(r, gobj),
        PHYS6 | PHYS10 => phys_spin(r, gobj),
        LANDED_LIT => landed_lit(r, gobj),
        THROWN => thrown(r, gobj),
        LANDED_THROWN => landed_thrown(r, gobj),
        EXPLODE => explode(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// The bob-omb's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itBombHei_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.bombhei()
}

/// `itBombHeiAttributes`' float at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a bob-omb without attributes");
    r.data.f32(a.at(off))
}

/// `(s32) f`, as the original converts it.
fn to_int(f: f32) -> i32 {
    fp::fctiwz(reg(f))
}

/// `(f32) i`, as the original converts it (exact, then rounded to single).
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// Bone 11, whose height and tilt make the walking pose.
fn pose_bone(game: &Game, gobj: GObjId) -> JObjId {
    let t = item(game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a bob-omb without a bone table");
    game.it.bone_tables[t.0].bones[0xB].expect("a bob-omb without bone 11")
}

/// Changes state keeping bone 11's height and tilt (the file's
/// `itBombhei_UpdateStatePreserveBone` and its variants).
fn change_keeping_pose(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    let j = pose_bone(r.game, gobj);
    let o = &r.game.objs.jobjs[j];
    assert!(
        o.flags & jobj::USE_QUATERNION == 0,
        "!(jobj->flags & JOBJ_USE_QUATERNION)"
    );
    let (y, x) = (o.translate.y, o.rotate.x);
    change_state(r, gobj, msid, flags);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.x = x);
}

/// Moves bone 11 on towards the walking pose (the file's `inline2_UnkMotion0_Anim`).
fn pose(r: &mut Running, gobj: GObjId) {
    let j = pose_bone(r.game, gobj);
    let (dy, dx) = {
        let v = vars(r.game, gobj);
        (v.xDF8.get(), v.xDFC.get())
    };
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(dy)));
    });
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| {
        q.x = mem(fp::fadds(reg(q.x), reg(dx)));
    });
}

/// Counts the integer timer and the fuse total down a frame.
fn tick(r: &mut Running, gobj: GObjId) -> i32 {
    let v = vars(r.game, gobj);
    v.xDD4.update(|old| old.wrapping_sub(1));
    v.xDEC.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    v.xDD4.get()
}

/// Drops a bob-omb at `pos`, lit, `it_8027D670`.
#[doc(alias = "it_8027D670")]
pub fn spawn_at(r: &mut Running, pos: Vec3) -> Option<GObjId> {
    let facing = owner::facing_for(r.game, r.data, pos);
    let mut spawn = SpawnItem {
        kind: super::BOMBHEI,
        pos,
        prev_pos: pos,
        ..Default::default()
    };
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    light(r, gobj);
    Some(gobj)
}

/// Explodes unless held or already exploding, `it_8027D730`.
#[doc(alias = "it_8027D730")]
pub fn detonate(r: &mut Running, gobj: GObjId) {
    if !item(r.game, gobj).xDC8_word.flags.x13 && vars(r.game, gobj).xDDC.get() == 0 {
        explode(r, gobj);
    }
}

/// `itBombhei_Logic6_Spawned`.
#[doc(alias = "itBombhei_Logic6_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (x4, x8, x10, x14) = (
        attr(r, gobj, 4),
        attr(r, gobj, 8),
        attr(r, gobj, 0x10),
        attr(r, gobj, 0x14),
    );
    let scl = reg(item(r.game, gobj).scl);
    let v = vars(r.game, gobj);
    v.xDDC.set(0);
    v.xDD4.set(to_int(x10));
    let walk = fp::fadds(reg(x4), fp::fadds(reg(x10), reg(x14)));
    v.xDEC.set(mem(fp::fadds(reg(x8), walk)));
    v.xDE0.set(0);
    v.xDE4.set(0);
    v.xDF8.set(mem(fp::fmuls(
        fp::fdivs(f64::from(-1.599_999_9_f32), walk),
        scl,
    )));
    v.xDFC.set(mem(fp::fmuls(
        fp::fdivs(f64::from(-0.261_799_4_f32), walk),
        scl,
    )));
    v.xDE8.set(1.0);
    fell(r, gobj);
}

/// The lit fuse, `it_8027D820`: the model pulses bigger and smaller every `x18` frames and it
/// explodes when the fuse runs out.
#[doc(alias = "it_8027D820")]
pub fn fuse(r: &mut Running, gobj: GObjId) {
    let (x18, x1c) = (attr(r, gobj, 0x18), attr(r, gobj, 0x1C));
    let j = jobj_of(r.game, gobj).expect("a bob-omb without a model");
    let v = vars(r.game, gobj);
    v.xDD4.update(|old| old.wrapping_sub(1));
    let s = fp::fmuls(reg(x1c), to_f32(v.xDD8.get()));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.scale.x = mem(fp::fadds(reg(o.scale.x), s));
    });
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.scale.y = mem(fp::fadds(reg(o.scale.y), s));
    });
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.scale.z = mem(fp::fadds(reg(o.scale.z), s));
    });
    let v = vars(r.game, gobj);
    if v.xDD4.get() <= 0 {
        v.xDD4.set(to_int(x18));
        v.xDD8.update(|old| old.wrapping_neg());
    }
    v.xDF0.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    v.xDEC.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    if reg(v.xDF0.get()) <= 0.0 {
        explode_once(r, gobj);
    }
}

/// Lands lying down, `fn_8027DAC8`.
#[doc(alias = "fn_8027DAC8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_keeping_pose(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itBombhei_UnkMotion0_Anim`: lying down until it stands up.
#[doc(alias = "itBombhei_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDD4.get() <= 0 {
        stand_up(r, gobj);
    } else {
        tick(r, gobj);
    }
    pose(r, gobj);
    false
}

/// The states that move nothing, `itBombhei_UnkMotion0_Phys` and the rest.
#[doc(alias = "itBombhei_UnkMotion0_Phys")]
#[doc(alias = "itBombhei_UnkMotion2_Phys")]
#[doc(alias = "itBombhei_UnkMotion3_Phys")]
#[doc(alias = "itBombhei_UnkMotion4_Phys")]
#[doc(alias = "itBombhei_UnkMotion5_Phys")]
#[doc(alias = "itBombhei_UnkMotion8_Phys")]
#[doc(alias = "itBombhei_UnkMotion11_Phys")]
#[doc(alias = "itBombhei_UnkMotion12_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itBombhei_UnkMotion0_Coll`.
#[doc(alias = "itBombhei_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Readies falling: from lying down or standing up it stops spinning, and from standing up
/// its timer starts over at `x14` (the file's `itBombhei_UnkMotion1_Setup`).
fn ready_fall(r: &mut Running, gobj: GObjId) {
    let x14 = attr(r, gobj, 0x14);
    let msid = item(r.game, gobj).msid;
    if msid == 3 || msid == 0 {
        let f = &mut item_mut(r.game, gobj).xDC8_word.flags;
        f.x19 = true;
        f.x17 = 1;
        lib2725::stop_spin(r.game, r.data, gobj);
    }
    if msid == 3 {
        vars(r.game, gobj).xDD4.set(to_int(x14));
    }
}

/// Falls (state 1), `it_8027DE18`.
#[doc(alias = "it_8027DE18")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    ready_fall(r, gobj);
    change_keeping_pose(r, gobj, 1, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itBombhei_UnkMotion1_Anim` and 12.
#[doc(alias = "itBombhei_UnkMotion1_Anim")]
#[doc(alias = "itBombhei_UnkMotion12_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Gravity from the attributes, `itBombhei_UnkMotion1_Phys`.
#[doc(alias = "itBombhei_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itBombhei_UnkMotion1_Coll`: one that has stood up lands walking (or explodes on a hard
/// landing); one that hasn't lands lying down.
#[doc(alias = "itBombhei_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE4.get() != 0 {
        land_hard_or(r, gobj, landed_thrown);
    } else {
        fall(r, gobj, landed);
    }
    false
}

/// Held: lit ones keep burning (state 8), unlit ones wait (state 7).
fn hold(r: &mut Running, gobj: GObjId, unlit: i32, lit: i32, unlit_flags: u32, lit_flags: u32) {
    let x0 = attr(r, gobj, 0);
    let f = &mut item_mut(r.game, gobj).xDC8_word.flags;
    f.x19 = true;
    f.x17 = 1;
    if vars(r.game, gobj).xDE0.get() == 0 {
        item_mut(r.game, gobj).x5D0_animFrameSpeed = x0;
        change_keeping_pose(r, gobj, unlit, unlit_flags);
    } else {
        item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
        change_keeping_pose(r, gobj, lit, lit_flags);
    }
}

/// `itBombhei_Logic6_PickedUp`.
#[doc(alias = "itBombhei_Logic6_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hold(r, gobj, 7, 8, change::ANIM_UPDATE, 1);
}

/// `itBombhei_UnkMotion8_Anim` (7): held, it settles once its animation ends, and a lit one
/// burns.
#[doc(alias = "itBombhei_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        hold(r, gobj, 7, 8, change::ANIM_UPDATE, 1);
    }
    if vars(r.game, gobj).xDE0.get() != 0 {
        fuse(r, gobj);
    }
    false
}

/// `it_3F14_Logic6_Dropped`.
#[doc(alias = "it_3F14_Logic6_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    hold(r, gobj, 9, 10, 6, 5);
}

/// Stands up, turning to face its way over `x4` frames, `it_8027E978`.
#[doc(alias = "it_8027E978")]
pub fn stand_up(r: &mut Running, gobj: GObjId) {
    let x4 = attr(r, gobj, 4);
    maplib::enter_ground(item_mut(r.game, gobj));
    let facing = reg(item(r.game, gobj).facing_dir);
    let v = vars(r.game, gobj);
    v.xDD4.set(to_int(x4));
    let turn = fp::fmul(std::f64::consts::FRAC_PI_2, facing);
    v.xE04
        .set(mem(fp::frsp(fp::fdiv(turn, f64::from(v.xDD4.get())))));
    v.xDE4.set(1);
    change_keeping_pose(r, gobj, 3, change::ANIM_UPDATE);
}

/// `itBombhei_UnkMotion3_Anim`: turns, then walks.
#[doc(alias = "itBombhei_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("a bob-omb without a model");
    let turn = vars(r.game, gobj).xE04.get();
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| {
        q.y = mem(fp::fadds(reg(q.y), reg(turn)));
    });
    if tick(r, gobj) <= 0 {
        let x14 = attr(r, gobj, 0x14);
        item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
        vars(r.game, gobj).xDD4.set(to_int(x14));
        walk(r, gobj);
        play_sfx(r, gobj, 0xF0, 0x7F, 0x40);
    }
    pose(r, gobj);
    false
}

/// `itBombhei_UnkMotion3_Coll` and 4.
#[doc(alias = "itBombhei_UnkMotion3_Coll")]
#[doc(alias = "itBombhei_UnkMotion4_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Sets off walking at `xC` (the file's `fn_80280974_inline`).
fn set_off(r: &mut Running, gobj: GObjId) {
    let xc = attr(r, gobj, 0xC);
    lib26b1::disable_pickup(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(xc), reg(ip.facing_dir)));
    ip.xD5C = 0;
    hitbox::set_hit_ids(r.game, gobj);
}

/// Walks (state 2), `it_8027EE04`.
#[doc(alias = "it_8027EE04")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    set_off(r, gobj);
    change_keeping_pose(r, gobj, 2, 0x1A);
}

/// `itBombhei_UnkMotion2_Anim`: walks until its timer lights it.
#[doc(alias = "itBombhei_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if tick(r, gobj) <= 0 {
        light(r, gobj);
    }
    if !lib2725::is_animating(r.game, gobj) {
        set_off(r, gobj);
        change_keeping_pose(r, gobj, 2, 0x1A);
    }
    pose(r, gobj);
    false
}

/// `itBombhei_UnkMotion2_Coll`: turns at ledges and walls, and falls off the ground.
#[doc(alias = "itBombhei_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
        maplib::tilt_on_floor(r.game, r.data, gobj);
    } else {
        ready_fall(r, gobj);
        change_keeping_pose(r, gobj, 1, change::ANIM_UPDATE);
    }
    false
}

/// Stops to turn around, `it_8027F42C`.
#[doc(alias = "it_8027F42C")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    change_keeping_pose(r, gobj, 4, 0x12);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    let back = -ip.facing_dir;
    vars(r.game, gobj).xDF4.set(back);
}

/// `itBombhei_UnkMotion4_Anim`: turning, it walks back the other way once the animation
/// ends, and lights when its timer runs out.
#[doc(alias = "itBombhei_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = -ip.facing_dir;
        set_off(r, gobj);
        change_keeping_pose(r, gobj, 2, 0x1A);
    }
    if tick(r, gobj) <= 0 {
        let back = vars(r.game, gobj).xDF4.get();
        item_mut(r.game, gobj).facing_dir = back;
        light(r, gobj);
    }
    pose(r, gobj);
    false
}

/// Stops and lights its fuse in `msid` (lit once: a lit one only changes state).
fn light_in(r: &mut Running, gobj: GObjId, msid: i32) {
    let (x8, x18) = (attr(r, gobj, 8), attr(r, gobj, 0x18));
    item_mut(r.game, gobj).x40_vel.x = 0.0;
    lib26b1::enable_pickup(r.game, gobj);
    if vars(r.game, gobj).xDE0.get() == 0 {
        let life = common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
        let v = vars(r.game, gobj);
        v.xDD4.set(to_int(x18));
        v.xDD8.set(1);
        v.xDF0.set(x8);
        v.xDE0.set(1);
        item_mut(r.game, gobj).xD44_lifeTimer = mem(fp::frsp(f64::from(life)));
        change_keeping_pose(r, gobj, msid, change::ANIM_UPDATE);
    } else {
        change_keeping_pose(r, gobj, msid, 1);
    }
}

/// Stops and lights its fuse on the ground, `it_8027F8E0`.
#[doc(alias = "it_8027F8E0")]
pub fn light(r: &mut Running, gobj: GObjId) {
    light_in(r, gobj, 5);
}

/// `itBombhei_UnkMotion5_Anim`.
#[doc(alias = "itBombhei_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE0.get() != 0 {
        fuse(r, gobj);
    }
    false
}

/// `itBombhei_UnkMotion5_Coll`.
#[doc(alias = "itBombhei_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, light_air);
    false
}

/// Lights its fuse in the air (state 6), `fn_8027FCA8`.
#[doc(alias = "fn_8027FCA8")]
pub fn light_air(r: &mut Running, gobj: GObjId) {
    light_in(r, gobj, 6);
}

/// `itBombhei_UnkMotion6_Anim`.
#[doc(alias = "itBombhei_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    fuse(r, gobj);
    false
}

/// Gravity and spin, `itBombhei_UnkMotion6_Phys` and 10 (`Item_ApplyFallingPhysics`).
#[doc(alias = "itBombhei_UnkMotion6_Phys")]
#[doc(alias = "itBombhei_UnkMotion10_Phys")]
pub fn phys_spin(r: &mut Running, gobj: GObjId) {
    phys1(r, gobj);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// Whether it landed harder than the attributes' `x20` in x or y.
fn hard_landing(r: &mut Running, gobj: GObjId) -> bool {
    let (lx, ly) = (attr(r, gobj, 0x20), attr(r, gobj, 0x24));
    let v = vars(r.game, gobj).xE0C.get();
    reg(v.x).abs() > reg(lx) || reg(v.y).abs() > reg(ly)
}

/// Lands from falling lit: explodes on a hard landing, else lights on the ground,
/// `fn_8028007C`.
#[doc(alias = "fn_8028007C")]
pub fn landed_lit(r: &mut Running, gobj: GObjId) {
    if hard_landing(r, gobj) {
        explode_once(r, gobj);
        return;
    }
    light(r, gobj);
}

/// Lands from falling with the speed it landed at kept (`xE0C`).
fn land_hard_or(r: &mut Running, gobj: GObjId, landed: fn(&mut Running, GObjId)) {
    let vel = item(r.game, gobj).x40_vel;
    vars(r.game, gobj).xE0C.set(vel);
    fall_to_floor(r, gobj, landed);
}

/// `itBombhei_UnkMotion6_Coll`.
#[doc(alias = "itBombhei_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    land_hard_or(r, gobj, landed_lit);
    false
}

/// `itBombhei_UnkMotion10_Coll`.
#[doc(alias = "itBombhei_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    land_hard_or(r, gobj, landed_thrown);
    false
}

/// Thrown (state 9, or 10 lit), at the size it was thrown at.
fn throw(r: &mut Running, gobj: GObjId) {
    let x0 = attr(r, gobj, 0);
    if vars(r.game, gobj).xDE0.get() == 0 {
        item_mut(r.game, gobj).x5D0_animFrameSpeed = x0;
        change_keeping_pose(r, gobj, 9, 6);
    } else {
        item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
        change_keeping_pose(r, gobj, 10, 0x104);
    }
    hitbox::clear_hit_ids(r.game, gobj);
    let scale = vars(r.game, gobj).xDE8.get();
    if reg(scale) != 1.0 {
        let j = jobj_of(r.game, gobj).expect("a bob-omb without a model");
        lib2725::rescale(r.game, gobj, j, scale);
    }
}

/// `it_3F14_Logic6_Thrown`.
#[doc(alias = "it_3F14_Logic6_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    throw(r, gobj);
}

/// `itBombhei_UnkMotion10_Anim` (9): rethrown once the animation ends; a lit one burns.
#[doc(alias = "itBombhei_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        throw(r, gobj);
    }
    if vars(r.game, gobj).xDE0.get() != 0 {
        fuse(r, gobj);
    }
    false
}

/// Lands from a throw: explodes on a hard landing, else walks, `fn_80280974`.
#[doc(alias = "fn_80280974")]
pub fn landed_thrown(r: &mut Running, gobj: GObjId) {
    if hard_landing(r, gobj) {
        explode_once(r, gobj);
        return;
    }
    set_off(r, gobj);
    change_keeping_pose(r, gobj, 2, 0x1A);
}

/// Explodes, `it_80280B60`: out of its holder's hands, hidden, it blasts for a while, pushing
/// the wind.
#[doc(alias = "it_80280B60")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDC8_word.flags.x13 {
        code::unported_call("it_8027429C (an item let go of)");
    }
    lib26b1::disable_pickup(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    lib2725::clear_velocity(r.game, gobj);
    vars(r.game, gobj).xDDC.set(1);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    crate::lb::wind::blast(
        &mut r.game.gr.winds,
        pos,
        0x78,
        1.0,
        0.02,
        std::f32::consts::FRAC_PI_3,
    );
    change_keeping_pose(r, gobj, 0xB, 0x12);
}

/// Explodes unless it already is, `it_80280DC0`.
#[doc(alias = "it_80280DC0")]
pub fn explode_once(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).xDDC.get() == 0 {
        explode(r, gobj);
    }
}

/// `it_3F14_Logic6_DmgDealt` and `_DmgReceived`: explodes unless held.
#[doc(alias = "it_3F14_Logic6_DmgDealt")]
#[doc(alias = "it_3F14_Logic6_DmgReceived")]
pub fn dmg(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != 7 && vars(r.game, gobj).xDDC.get() == 0 {
        explode(r, gobj);
    }
    false
}

/// `itBombhei_UnkMotion11_Anim`: the blast lasts its lifetime.
#[doc(alias = "itBombhei_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// `itBombhei_UnkMotion11_Coll`.
#[doc(alias = "itBombhei_UnkMotion11_Coll")]
pub fn coll11(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `it_3F14_Logic6_EnteredAir`.
#[doc(alias = "it_3F14_Logic6_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_keeping_pose(r, gobj, 0xC, change::ANIM_UPDATE);
}

/// `itBombhei_UnkMotion12_Coll`.
#[doc(alias = "itBombhei_UnkMotion12_Coll")]
pub fn coll12(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// `itBombhei_Logic6_Clanked`.
#[doc(alias = "itBombhei_Logic6_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    explode_once(r, gobj);
    false
}

/// `itBombhei_Logic6_Reflected`.
#[doc(alias = "itBombhei_Logic6_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// `it_3F14_Logic6_HitShield` and `_ShieldBounced`: a thrown one explodes.
#[doc(alias = "it_3F14_Logic6_HitShield")]
#[doc(alias = "it_3F14_Logic6_ShieldBounced")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 9 && vars(r.game, gobj).xDDC.get() == 0 {
        explode(r, gobj);
    }
    false
}

/// Forgets a GObj the bob-omb points to, `itBombhei_Logic6_EvtUnk` (`it_8026B894`).
#[doc(alias = "itBombhei_Logic6_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
