//! Mr. Saturn, `melee/it/kinds/itdosei.c`: an item that falls, lands and stands a while, then
//! walks, turning at ledges and walls, until someone picks it up. Thrown, it flies with its
//! holder's size (`xDE0`); hit, it hops back and stands dazed (`xDF0` frames) before walking
//! on. Its model's root turns to face the way it walks (`xDDC` is the turn still to go after it
//! lands from a spawn), and it remembers where it stood last (`xDE4`), which a turn takes it
//! back to.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall, ground_skid, ground_to_ledge};
use super::super::item::{
    article, attr_of, change, change_state, common_data, play_sfx1, seek_anim,
};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::itDosei_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_1164;
pub const LANDED_HIT: u32 = 0x8028_1390;
pub const ANIM0: u32 = 0x8028_13F8;
pub const PHYS0: u32 = 0x8028_16F0;
pub const COLL0: u32 = 0x8028_16F4;
pub const SKIDDED: u32 = 0x8028_1734;
pub const WALK: u32 = 0x8028_17A0;
pub const ANIM1: u32 = 0x8028_1AB4;
pub const PHYS1: u32 = 0x8028_1B44;
pub const COLL1: u32 = 0x8028_1B7C;
pub const TURN: u32 = 0x8028_1C6C;
pub const ANIM2: u32 = 0x8028_1D00;
pub const PHYS2: u32 = 0x8028_1E30;
pub const COLL2: u32 = 0x8028_1E34;
pub const FALL: u32 = 0x8028_2074;
pub const ANIM3: u32 = 0x8028_20CC;
pub const PHYS3: u32 = 0x8028_2130;
pub const COLL5: u32 = 0x8028_2160;
pub const PICKED_UP: u32 = 0x8028_218C;
pub const ANIM4: u32 = 0x8028_24B8;
pub const PHYS4: u32 = 0x8028_26F4;
pub const DROPPED: u32 = 0x8028_26F8;
pub const THROWN: u32 = 0x8028_29FC;
pub const ANIM5: u32 = 0x8028_2AC0;
pub const PHYS5: u32 = 0x8028_2AC8;
pub const ENTERED_AIR: u32 = 0x8028_2B14;
pub const ANIM6: u32 = 0x8028_2B84;
pub const PHYS6: u32 = 0x8028_2BA8;
pub const COLL6: u32 = 0x8028_2BAC;
pub const SPAWN_FALL: u32 = 0x8028_2BFC;
pub const ANIM8: u32 = 0x8028_2C54;
pub const PHYS8: u32 = 0x8028_2C78;
pub const COLL8: u32 = 0x8028_2CA8;
pub const SPAWN_LANDED: u32 = 0x8028_2CD4;
pub const ANIM7: u32 = 0x8028_2D48;
pub const PHYS7: u32 = 0x8028_2DA0;
pub const COLL7: u32 = 0x8028_2DA4;
pub const SPAWN_TURN: u32 = 0x8028_2DE4;
pub const ANIM9: u32 = 0x8028_30F4;
pub const PHYS9: u32 = 0x8028_32A8;
pub const COLL9: u32 = 0x8028_32AC;
pub const ANIM10: u32 = 0x8028_3504;
pub const PHYS10: u32 = 0x8028_3550;
pub const COLL10: u32 = 0x8028_3554;
pub const DMG_RECEIVED: u32 = 0x8028_3588;
pub const ANIM11: u32 = 0x8028_38FC;
pub const PHYS11: u32 = 0x8028_3960;
pub const COLL11: u32 = 0x8028_3990;
pub const DMG_DEALT: u32 = 0x8028_39BC;
pub const REFLECTED: u32 = 0x8028_3A3C;
pub const CLANKED: u32 = 0x8028_3A5C;
pub const HIT_SHIELD: u32 = 0x8028_3A80;
pub const SHIELD_BOUNCED: u32 = 0x8028_3AA4;

/// The sounds its hits make, one by chance (`it_803F56B8`).
const HIT_SFX: [i32; 3] = [0x109, 0x10A, 0x10B];

/// `M_PI` as the original spells it, a double.
const PI: f64 = std::f64::consts::PI;

/// Runs one of Mr. Saturn's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM4 => anim4(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM6 | ANIM8 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        COLL8 => coll8(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 => coll7(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        ANIM11 => anim11(r, gobj),
        COLL11 => coll11(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED | HIT_SHIELD => bounce(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of Mr. Saturn's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_HIT => landed_hit(r, gobj),
        PHYS0 | PHYS2 | PHYS4 | PHYS6 | PHYS7 | PHYS9 | PHYS10 => idle(r, gobj),
        SKIDDED => skidded(r, gobj),
        WALK => walk(r, gobj),
        PHYS1 => phys1(r, gobj),
        TURN => turn(r, gobj),
        FALL => fall_start(r, gobj),
        PHYS3 | PHYS8 | PHYS11 => phys_fall(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS5 => phys5(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        SPAWN_FALL => spawn_fall(r, gobj),
        SPAWN_LANDED => spawn_landed(r, gobj),
        SPAWN_TURN => spawn_turn(r, gobj),
        _ => return false,
    }
    true
}

/// Mr. Saturn's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itDosei_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.dosei()
}

/// `itDoseiAttributes`' word at `off`: the held and thrown animation's speed (`unk0`, a
/// float), the frames it stands after a spawn (`unk4`), its walking speed (`unk8`, a float),
/// the frames of its turn after a spawn (`unkC`) and a hit's push (`unk10`, `unk14`).
fn attr_word(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Mr. Saturn without attributes");
    r.data.u32(a.at(off))
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    f32::from_bits(attr_word(r, gobj, off))
}

/// Its model's root.
fn root(game: &Game, gobj: GObjId) -> jobj::JObjId {
    jobj_of(game, gobj).expect("a Mr. Saturn without a model")
}

/// The animation runs at `speed` (`itDosei_SetSpeed`).
fn set_speed(r: &mut Running, gobj: GObjId, speed: f32) {
    let j = jobj_of(r.game, gobj);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, speed);
}

/// `itResetVelocity`.
fn stop(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
}

/// The way it faces, turned by `m` (`itDosei_FacingAngle`): `facing * π/2 + facing * m`.
fn facing_angle(game: &Game, gobj: GObjId, m: f64) -> f32 {
    let facing = reg(item(game, gobj).facing_dir);
    mem(fp::frsp(fp::fmadd(
        fp::fdiv(PI, 2.0),
        facing,
        fp::fmuls(facing, m),
    )))
}

/// The root's rotation cleared, then turned to `angle` about y (`HSD_JObjSetRotationZero` and
/// `HSD_JObjSetRotationY`, or their `WithMtxDirty` variants, which dirty it the same way).
fn face(r: &mut Running, gobj: GObjId, angle: f32) {
    let j = root(r.game, gobj);
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_rotation_x(jobjs, j, 0.0);
    jobj::set_rotation_y(jobjs, j, 0.0);
    jobj::set_rotation_z(jobjs, j, 0.0);
    jobj::set_rotation_y(jobjs, j, angle);
}

/// Faces the way it walks.
fn face_ahead(r: &mut Running, gobj: GObjId) {
    let angle = facing_angle(r.game, gobj, 0.0);
    face(r, gobj, angle);
}

/// Where it stands now, remembered for a turn.
fn remember_pos(game: &mut Game, gobj: GObjId) {
    let pos = item(game, gobj).pos;
    vars(game, gobj).xDE4.set(pos);
}

/// Its model shown again (when let go of).
fn show(r: &mut Running, gobj: GObjId) {
    let child = r.game.objs.jobjs[root(r.game, gobj)].child;
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
}

fn set_x17_x19(game: &mut Game, gobj: GObjId) {
    let f = &mut item_mut(game, gobj).xDC8_word.flags;
    f.x17 = 1;
    f.x19 = true;
}

/// `itDosei_Logic7_Spawned`: still, standing for `unk4` frames once it lands, falling.
#[doc(alias = "itDosei_Logic7_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    let frames = attr_word(r, gobj, 4) as i32;
    let v = vars(r.game, gobj);
    v.xDD4.set(frames);
    v.xDF0.set(0);
    v.xDE0.set(1.0);
    lib26b1::enable_pickup(r.game, gobj);
    spawn_fall(r, gobj);
    let j = root(r.game, gobj);
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_rotation_x(jobjs, j, 0.0);
    jobj::set_rotation_y(jobjs, j, 0.0);
    jobj::set_rotation_z(jobjs, j, 0.0);
}

/// Lands from a hit, standing dazed, `itDosei_80281390`.
#[doc(alias = "itDosei_80281390")]
pub fn landed_hit(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    change_state(r, gobj, 0, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// `itDosei_UnkMotion0_Anim`: dazed, it walks on once its count (`xDF0`) runs out.
#[doc(alias = "itDosei_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    face_ahead(r, gobj);
    let v = vars(r.game, gobj);
    v.xDF0.update(|n| n.wrapping_sub(1));
    if v.xDF0.get() <= 0 {
        v.xDF0.set(0);
        walk(r, gobj);
    }
    false
}

/// The states that move nothing, `itDosei_UnkMotion0_Phys` (2, 4, 6, 7, 9, 10).
#[doc(alias = "itDosei_UnkMotion0_Phys")]
#[doc(alias = "itDosei_UnkMotion2_Phys")]
#[doc(alias = "itDosei_UnkMotion4_Phys")]
#[doc(alias = "itDosei_UnkMotion6_Phys")]
#[doc(alias = "itDosei_UnkMotion7_Phys")]
#[doc(alias = "itDosei_UnkMotion9_Phys")]
#[doc(alias = "itDosei_UnkMotion10_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itDosei_UnkMotion0_Coll`: falls off ledges, tilted to the floor.
#[doc(alias = "itDosei_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fall_start);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Stops skidding: it turns if the floor slopes up ahead, else walks, `itDosei_80281734`.
#[doc(alias = "itDosei_80281734")]
pub fn skidded(r: &mut Running, gobj: GObjId) {
    let facing = item(r.game, gobj).facing_dir;
    let nx = reg(vars(r.game, gobj).xDF8.get().x);
    let uphill = if reg(facing) == 1.0 {
        nx < 0.0
    } else {
        nx > 0.0
    };
    if uphill {
        turn(r, gobj);
    } else {
        walk(r, gobj);
    }
}

/// Walks on the ground the way it faces, `itDosei_802817A0`.
#[doc(alias = "itDosei_802817A0")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let speed = attr_f32(r, gobj, 8);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x17 = 1;
    ip.xDC8_word.flags.x19 = true;
    ip.owner = None;
    maplib::enter_ground(ip);
    change_state(r, gobj, 1, 0xB);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    face_ahead(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), reg(speed)));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
}

/// `itDosei_UnkMotion1_Anim`: walking, its steps quicken downhill and slow uphill.
#[doc(alias = "itDosei_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    let ip = item(r.game, gobj);
    let speed = mem(fp::fmadds(
        0.5,
        fp::fmuls(reg(ip.x378_itemColl.floor.normal.x), reg(ip.facing_dir)),
        1.0,
    ));
    set_speed(r, gobj, speed);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
    }
    false
}

/// `itDosei_UnkMotion1_Phys`: its walking speed by its steps' pace.
#[doc(alias = "itDosei_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let speed = attr_f32(r, gobj, 8);
    let ip = item_mut(r.game, gobj);
    let v = fp::fmuls(reg(speed), reg(ip.x5D0_animFrameSpeed));
    let v = if v < fp::frsp(0.0) { fp::fneg(v) } else { v };
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), v));
}

/// `itDosei_UnkMotion1_Coll`: it turns at a ledge or a wall, skids down a steep slope, and
/// falls when the ground goes.
#[doc(alias = "itDosei_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
            return false;
        }
        let ip = item_mut(r.game, gobj);
        let nx = reg(ip.x378_itemColl.floor.normal.x);
        let angle = if nx < fp::frsp(0.0) { fp::fneg(nx) } else { nx };
        if angle >= fp::fdivs(fp::frsp(PI), 4.0) {
            ip.xD5C = 1;
            ip.xDC8_word.flags.x1F = true;
        } else {
            ip.xD5C = 0;
            ip.xDC8_word.flags.x1F = false;
        }
        if item(r.game, gobj).xD5C == 1 {
            entered_air(r, gobj);
        } else {
            maplib::tilt_on_floor(r.game, r.data, gobj);
        }
    } else {
        fall_start(r, gobj);
    }
    false
}

/// Turns around where it stood last, `itDosei_80281C6C`.
#[doc(alias = "itDosei_80281C6C")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    let v = vars(r.game, gobj);
    v.xDD8.set(0);
    let pos = v.xDE4.get();
    v.xDDC.set(0.0);
    let ip = item_mut(r.game, gobj);
    ip.pos = pos;
    ip.x40_vel.x = 0.0;
    maplib::enter_ground(ip);
    change_state(r, gobj, 2, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// `itDosei_UnkMotion2_Anim`: at the turn's end it faces the other way and walks.
#[doc(alias = "itDosei_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        let angle = facing_angle(r.game, gobj, 0.0);
        let j = root(r.game, gobj);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, angle);
    }
    false
}

/// Walks again from a turn: as [`walk`], dirtying the root's matrix outright
/// (`itDosei_SetupWalk_FC`).
fn walk_on(r: &mut Running, gobj: GObjId) {
    walk(r, gobj);
}

/// `itDosei_UnkMotion2_Coll`: falls off ledges; a spawn's turn done (`xDD8` 2), it walks.
#[doc(alias = "itDosei_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fall_start);
    if vars(r.game, gobj).xDD8.get() == 2 {
        walk_on(r, gobj);
    }
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Falls, `itDosei_80282074`.
#[doc(alias = "itDosei_80282074")]
pub fn fall_start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// `itDosei_UnkMotion3_Anim`: falling, its animation looping.
#[doc(alias = "itDosei_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 3, change::ANIM_UPDATE);
    }
    false
}

/// Falling, by its attributes, `itDosei_UnkMotion3_Phys` (8, 11).
#[doc(alias = "itDosei_UnkMotion3_Phys")]
#[doc(alias = "itDosei_UnkMotion8_Phys")]
#[doc(alias = "itDosei_UnkMotion11_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Lands and walks, `itDosei_UnkMotion5_Coll` (and state 3's).
#[doc(alias = "itDosei_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, walk);
    false
}

/// Held, shown, its animation at the held pace, `itDosei_Logic7_PickedUp`.
#[doc(alias = "itDosei_Logic7_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hold(r, gobj);
}

/// Held: the held state, entered afresh (shown, at the held pace) unless already held. The
/// root's rotation after it reads the state the change just set, so it never runs.
fn hold(r: &mut Running, gobj: GObjId) {
    let pace = attr_f32(r, gobj, 0);
    if item(r.game, gobj).msid != 4 {
        change_state(r, gobj, 4, change::ANIM_UPDATE);
        show(r, gobj);
        set_speed(r, gobj, pace);
        set_x17_x19(r.game, gobj);
    } else {
        change_state(r, gobj, 4, change::ANIM_UPDATE);
    }
    if item(r.game, gobj).msid != 4 {
        face_ahead(r, gobj);
    }
}

/// `itDosei_UnkMotion4_Anim`: held, its animation starting over at its end.
#[doc(alias = "itDosei_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        hold(r, gobj);
    }
    false
}

/// Let go of: it falls, shown, `itDosei_Logic7_Dropped`.
#[doc(alias = "itDosei_Logic7_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    let pace = attr_f32(r, gobj, 0);
    lib26b1::enable_pickup(r.game, gobj);
    show(r, gobj);
    set_x17_x19(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE | change::DROP_UPDATE);
    set_speed(r, gobj, pace);
    face_ahead(r, gobj);
}

/// Thrown: it flies at its holder's size (`xDE0`), `itDosei_Logic7_Thrown`.
#[doc(alias = "itDosei_Logic7_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let pace = attr_f32(r, gobj, 0);
    if item(r.game, gobj).msid != 5 {
        lib26b1::enable_pickup(r.game, gobj);
        show(r, gobj);
        change_state(r, gobj, 5, change::ANIM_UPDATE | change::DROP_UPDATE);
        set_speed(r, gobj, pace);
    } else {
        change_state(r, gobj, 5, change::ANIM_UPDATE | change::DROP_UPDATE);
    }
    let scale = vars(r.game, gobj).xDE0.get();
    let j = root(r.game, gobj);
    lib2725::rescale(r.game, gobj, j, scale);
}

/// `itDosei_UnkMotion5_Anim`: thrown, nothing.
#[doc(alias = "itDosei_UnkMotion5_Anim")]
pub fn anim5(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itDosei_UnkMotion5_Phys`: thrown, falling (`Item_ApplyFallingPhysics`).
#[doc(alias = "itDosei_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    phys_fall(r, gobj);
    let x68 = common_data(r.game).f32(r.data, super::super::data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// Skids off a slope: from a walk (state 6), or else from a spawn (10),
/// `itDosei_Logic7_EnteredAir`.
#[doc(alias = "itDosei_Logic7_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    let msid = if item(r.game, gobj).xDC8_word.flags.x19 {
        6
    } else {
        10
    };
    change_state(r, gobj, msid, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// Where it is, remembered, `itDosei_UnkMotion6_Anim` (and 8).
#[doc(alias = "itDosei_UnkMotion6_Anim")]
#[doc(alias = "itDosei_UnkMotion8_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    false
}

/// `itDosei_UnkMotion6_Coll`: skidding, it keeps the floor's slope, and stops or falls.
#[doc(alias = "itDosei_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    let n = item(r.game, gobj).x378_itemColl.floor.normal;
    vars(r.game, gobj).xDF8.set(n);
    ground_skid(r, gobj, skidded, fall_start);
    false
}

/// Falls from its spawn, `itDosei_80282BFC`.
#[doc(alias = "itDosei_80282BFC")]
pub fn spawn_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 8, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// `itDosei_UnkMotion8_Coll`: lands from its spawn.
#[doc(alias = "itDosei_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, spawn_landed);
    false
}

/// Landed from its spawn, it stands still, `itDosei_80282CD4`.
#[doc(alias = "itDosei_80282CD4")]
pub fn spawn_landed(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    maplib::enter_ground(item_mut(r.game, gobj));
    change_state(r, gobj, 7, 3);
    set_speed(r, gobj, 1.0);
    item_mut(r.game, gobj).owner = None;
}

/// `itDosei_UnkMotion7_Anim`: standing after its spawn, it turns once its count (`xDD4`) runs
/// out.
#[doc(alias = "itDosei_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    let v = vars(r.game, gobj);
    if v.xDD4.get() > 0 {
        v.xDD4.update(|n| n.wrapping_sub(1));
    } else {
        spawn_turn(r, gobj);
    }
    false
}

/// `itDosei_UnkMotion7_Coll`: falls off ledges back into its spawn's fall.
#[doc(alias = "itDosei_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, spawn_fall);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Turns from facing the screen to the way it walks, `itDosei_80282DE4`: a quarter turn
/// (`xDDC`) still to go.
#[doc(alias = "itDosei_80282DE4")]
pub fn spawn_turn(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let v = vars(r.game, gobj);
    v.xDD8.set(1);
    v.xDDC.set(mem(fp::frsp(fp::fdiv(PI, 2.0))));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.xDC8_word.flags.x17 = 1;
    ip.xDC8_word.flags.x19 = true;
    ip.owner = None;
    change_state(r, gobj, 9, 3);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    let left = reg(vars(r.game, gobj).xDDC.get());
    let angle = facing_angle(r.game, gobj, fp::fneg(left));
    face(r, gobj, angle);
}

/// `itDosei_UnkMotion9_Anim`: turning by π over `unkC` frames; the turn done (`xDD8` 2), it
/// sets off at its walking speed.
#[doc(alias = "itDosei_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    let frames = attr_word(r, gobj, 0xC) as i32;
    let speed = attr_f32(r, gobj, 8);
    let v = vars(r.game, gobj);
    let left = reg(v.xDDC.get());
    v.xDDC.set(mem(fp::frsp(fp::fsub(
        left,
        fp::fdiv(PI, f64::from(frames)),
    ))));
    if v.xDD8.get() == 1 {
        let left = reg(v.xDDC.get());
        let angle = facing_angle(r.game, gobj, fp::fneg(left));
        let j = root(r.game, gobj);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, angle);
        let v = vars(r.game, gobj);
        if reg(v.xDDC.get()) <= 0.0 {
            v.xDD8.set(2);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), reg(speed)));
        }
    }
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 9, change::ANIM_UPDATE);
    }
    false
}

/// `itDosei_UnkMotion9_Coll`: falls off ledges; the turn done, it walks on from the same
/// frame of its animation.
#[doc(alias = "itDosei_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fall_start);
    if vars(r.game, gobj).xDD8.get() == 2 {
        let frame = item(r.game, gobj).x5CC_currentAnimFrame;
        walk_on(r, gobj);
        seek_anim(r, gobj, frame);
    }
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// `itDosei_UnkMotion10_Anim`: skidding after its spawn.
#[doc(alias = "itDosei_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    set_speed(r, gobj, 1.0);
    false
}

/// `itDosei_UnkMotion10_Coll`: stops into its stand or falls into its spawn's fall.
#[doc(alias = "itDosei_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, spawn_landed, spawn_fall);
    false
}

/// Hit: it hops back, its life a second shorter, dazed for 20 frames once it lands,
/// `itDosei_Logic7_DmgReceived`: whether its life ran out.
#[doc(alias = "itDosei_Logic7_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj);
    let (push_x, push_y) = (
        attr_word(r, gobj, 0x10) as i32,
        attr_word(r, gobj, 0x14) as i32,
    );
    change_state(r, gobj, 11, 3);
    maplib::enter_air(item_mut(r.game, gobj));
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(fp::frsp(f64::from(push_x)), reg(ip.facing_dir)));
    let up = if push_y < 0 {
        push_y.wrapping_neg()
    } else {
        push_y
    };
    ip.x40_vel.y = mem(fp::frsp(f64::from(up)));
    ip.x40_vel.z = 0.0;
    ip.xDC8_word.flags.x17 = 1;
    ip.xDC8_word.flags.x19 = true;
    vars(r.game, gobj).xDF0.set(20);
    face_ahead(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 60.0));
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itDosei_UnkMotion11_Anim`: hopping back, its animation looping.
#[doc(alias = "itDosei_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r.game, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 11, change::ANIM_UPDATE);
    }
    false
}

/// `itDosei_UnkMotion11_Coll`: lands dazed.
#[doc(alias = "itDosei_UnkMotion11_Coll")]
pub fn coll11(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed_hit);
    false
}

/// A hit it deals makes one of its sounds; thrown, it bounces off and stops hitting,
/// `itDosei_Logic7_DmgDealt`.
#[doc(alias = "itDosei_Logic7_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    let sfx = HIT_SFX[r.game.objs.rng.below(3) as usize];
    play_sfx1(r, gobj, sfx, 127, 64);
    if item(r.game, gobj).msid == 5 {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
        lib2725::clear_hitboxes(r.game, r.data, gobj);
    }
    false
}

/// Sent back by a reflection, `itDosei_Logic7_Reflected`.
#[doc(alias = "itDosei_Logic7_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a clank or a shield, `itDosei_Logic7_Clanked` and `itDosei_Logic7_HitShield`.
#[doc(alias = "itDosei_Logic7_Clanked")]
#[doc(alias = "itDosei_Logic7_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Bounces off a shield, `itDosei_Logic7_ShieldBounced`.
#[doc(alias = "itDosei_Logic7_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Forgets a GObj it points to, `itDosei_Logic7_EvtUnk` (`it_8026B894`).
#[doc(alias = "itDosei_Logic7_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
