//! The Goomba, `melee/it/kinds/itoldkuri.c` (`It_Kind_Kuriboh`): one of the monsters a container
//! releases (`it_8026F6BC`). It stands a while, then walks one way and the other, turning at a
//! wall or after a walk's round; a fighter landing on it flattens it (state 7) or, in the air,
//! knocks it down (state 8). Hit, it's knocked back (state 9) or, past its health, flung away as
//! an enemy KO. A fighter can pick it up and throw it.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_73F0;
pub const DMG_RECEIVED: u32 = 0x802D_7F38;
pub const PICKED_UP: u32 = 0x802D_7C14;
pub const DROPPED: u32 = 0x802D_7CB4;
pub const THROWN: u32 = 0x802D_7D18;
pub const STAND: u32 = 0x802D_747C;
pub const FALL: u32 = 0x802D_7AF0;
/// `it_802D8098`, its jumped-on callback.
pub const STOMPED: u32 = 0x802D_8098;
pub const ANIM0: u32 = 0x802D_74D0;
pub const PHYS0: u32 = 0x802D_74D8;
pub const COLL0: u32 = 0x802D_7550;
pub const ANIM1: u32 = 0x802D_75F8;
pub const PHYS1: u32 = 0x802D_7670;
pub const COLL1: u32 = 0x802D_76EC;
pub const ANIM2: u32 = 0x802D_77C8;
pub const PHYS2: u32 = 0x802D_788C;
pub const COLL2: u32 = 0x802D_795C;
pub const ANIM3: u32 = 0x802D_7A18;
pub const PHYS3: u32 = 0x802D_7A60;
pub const COLL3: u32 = 0x802D_7A90;
pub const ANIM4: u32 = 0x802D_7B3C;
pub const PHYS4: u32 = 0x802D_7B84;
pub const COLL4: u32 = 0x802D_7BB4;
pub const ANIM5: u32 = 0x802D_7C58;
pub const PHYS5: u32 = 0x802D_7CB0;
pub const ANIM6: u32 = 0x802D_7D7C;
pub const PHYS6: u32 = 0x802D_7DC4;
pub const COLL6: u32 = 0x802D_7DF4;
pub const ANIM9: u32 = 0x802D_7E18;
pub const PHYS9: u32 = 0x802D_7EAC;
pub const COLL9: u32 = 0x802D_7ED4;
pub const ANIM7: u32 = 0x802D_8190;
pub const PHYS7: u32 = 0x802D_81D8;
pub const COLL7: u32 = 0x802D_81DC;
pub const ANIM8: u32 = 0x802D_8240;
pub const PHYS8: u32 = 0x802D_8274;
pub const COLL8: u32 = 0x802D_82A4;
pub const ANIM10: u32 = 0x802D_8330;
pub const PHYS10: u32 = 0x802D_8378;
pub const COLL10: u32 = 0x802D_837C;
pub const ANIM11: u32 = 0x802D_8408;
pub const PHYS11: u32 = 0x802D_8450;
pub const COLL11: u32 = 0x802D_846C;

/// Its variables' offsets in the union (`itOldkuri_ItemVars`).
pub mod off {
    /// `xDEC`, the common `x14.y`, cleared as it's flung away.
    pub const XDEC: usize = 0x18;
    /// `xDF4`: its walking velocity.
    pub const SPEED: usize = 0x20;
    /// `xDF8`: the frames it stands.
    pub const TIMER: usize = 0x24;
    /// `xDFC`: a walk's round done, so it may turn.
    pub const TURN: usize = 0x28;
}

/// Its special attributes (`itOldkuriAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Goomba without attributes")
        .at(off)
}

/// `attr->x0`: its health (`[0]`, an integer) and walking speed (`[1]`).
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Goomba's health");
    r.data.u32(p) as i32
}

fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Goomba's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

/// `GA_Air`.
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// Runs one of the Goomba's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        STOMPED => stomped(r, gobj),
        ANIM0 => answers_false(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 | COLL4 => coll_falling(r, gobj),
        ANIM4 => anim4(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 | COLL11 => coll_still(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL8 => coll8(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        ANIM11 => anim11(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Goomba's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        STAND => stand(r, gobj),
        FALL => fall(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 | PHYS4 | PHYS6 | PHYS8 => phys_fall(r, gobj),
        PHYS5 | PHYS7 | PHYS10 => still(r, gobj),
        PHYS9 => phys9(r, gobj),
        PHYS11 => phys_gravity(r, gobj),
        _ => return false,
    }
    true
}

/// The Goomba's setup, `it_802D73F0`: the enemies' common variables, facing most fighters, its
/// collision an enemy's, no spin, no timer or speed, and standing.
#[doc(alias = "it_802D73F0")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = facing;
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    set(r, gobj, off::TIMER, 0);
    zako::bytes(r.game, gobj).set_f32(off::SPEED, 0.0);
    stand(r, gobj);
}

/// Stands, `it_802D747C`: settled, its model turned to the floor, no blink, state 0.
#[doc(alias = "it_802D747C")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    zako::align_floor(r, gobj, 0.0, 5.0);
    hitbox::end_blink(r.game, gobj);
    enter(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states' animation that answers false, `itOldkuri_UnkMotion0_Anim`.
#[doc(alias = "itOldkuri_UnkMotion0_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states' physics that do nothing (`itOldkuri_UnkMotion5_Phys` and its like).
#[doc(alias = "itOldkuri_UnkMotion5_Phys")]
#[doc(alias = "itOldkuri_UnkMotion7_Phys")]
#[doc(alias = "itOldkuri_UnkMotion10_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Standing's physics, `itOldkuri_UnkMotion0_Phys`: its timer done, it walks the way it faces;
/// its model turned to the floor.
#[doc(alias = "itOldkuri_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if reg(item(r.game, gobj).facing_dir) == 1.0 {
            walk_right(r, gobj);
        } else {
            walk_left(r, gobj);
        }
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    zako::align_floor(r, gobj, 0.0, 5.0);
}

/// Standing's collision, `itOldkuri_UnkMotion0_Coll`: off the floor, it falls.
#[doc(alias = "itOldkuri_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Its walk's speed the way it faces (`xDF4`), stopped otherwise.
fn start_walk(r: &mut Running, gobj: GObjId) {
    let speed = reg(walk_speed(r, gobj));
    let v = mem(fp::fmuls(reg(item(r.game, gobj).facing_dir), speed));
    zako::bytes(r.game, gobj).set_f32(off::SPEED, v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = v;
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
}

/// Walks left, `it_802D758C` (and its inline copies): its speed from its facing, facing left,
/// state 1.
#[doc(alias = "it_802D758C")]
pub fn walk_left(r: &mut Running, gobj: GObjId) {
    start_walk(r, gobj);
    item_mut(r.game, gobj).facing_dir = -1.0;
    set(r, gobj, off::TURN, 0);
    enter(r, gobj, 1, change::ANIM_UPDATE);
}

/// Walking left's animation, `itOldkuri_UnkMotion1_Anim`: when it ends, facing left it may
/// turn (the round again, keeping its hits), else it walks right.
#[doc(alias = "itOldkuri_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if reg(item(r.game, gobj).facing_dir) == fp::fneg(1.0) {
            set(r, gobj, off::TURN, 1);
            enter(r, gobj, 1, change::ANIM_UPDATE | change::HIT_PRESERVE);
        } else {
            walk_right(r, gobj);
        }
    }
    false
}

/// Walking left's physics, `itOldkuri_UnkMotion1_Phys`: at the script's turn point once it may
/// turn, it walks right; along the floor.
#[doc(alias = "itOldkuri_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    if get(r, gobj, off::TURN) != 0 && lib2725::xdc0_has(r.game, gobj, 2) {
        walk_right(r, gobj);
    }
    along_floor(r, gobj);
}

/// Along the floor, its model turned to it.
fn along_floor(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, 0.0, 5.0);
}

/// Walking left's collision, `itOldkuri_UnkMotion1_Coll`: off the floor it falls; at a left
/// wall once it may turn, it walks right.
#[doc(alias = "itOldkuri_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    if maplib::touched_wall(r.game, gobj) == 4 && get(r, gobj, off::TURN) != 0 {
        walk_right(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Walks right, `it_802D775C`: its speed from its facing, facing right, state 2.
#[doc(alias = "it_802D775C")]
pub fn walk_right(r: &mut Running, gobj: GObjId) {
    start_walk(r, gobj);
    item_mut(r.game, gobj).facing_dir = 1.0;
    set(r, gobj, off::TURN, 0);
    enter(r, gobj, 2, change::ANIM_UPDATE);
}

/// Walking right's animation, `itOldkuri_UnkMotion2_Anim`: when it ends, facing right it may
/// turn (the round again, keeping its hits), else it walks left.
#[doc(alias = "itOldkuri_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if reg(item(r.game, gobj).facing_dir) == 1.0 {
            set(r, gobj, off::TURN, 1);
            enter(r, gobj, 2, change::ANIM_UPDATE | change::HIT_PRESERVE);
        } else {
            walk_left(r, gobj);
        }
    }
    false
}

/// Walking right's physics, `itOldkuri_UnkMotion2_Phys`: at the turn point once it may turn,
/// it walks left; its walking speed again, along the floor.
#[doc(alias = "itOldkuri_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if get(r, gobj, off::TURN) != 0 && lib2725::xdc0_has(r.game, gobj, 2) {
        walk_left(r, gobj);
    }
    let v = zako::bytes(r.game, gobj).f32(off::SPEED);
    item_mut(r.game, gobj).x40_vel.x = v;
    along_floor(r, gobj);
}

/// Walking right's collision, `itOldkuri_UnkMotion2_Coll`: off the floor it falls; at a right
/// wall once it may turn, it walks left.
#[doc(alias = "itOldkuri_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    if maplib::touched_wall(r.game, gobj) == 8 && get(r, gobj, off::TURN) != 0 {
        walk_left(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Falling left's animation, `itOldkuri_UnkMotion3_Anim`: looped, keeping its hits.
#[doc(alias = "itOldkuri_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 3, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// The falling states' physics (`itOldkuri_UnkMotion3_Phys` and its like): gravity.
#[doc(alias = "itOldkuri_UnkMotion3_Phys")]
#[doc(alias = "itOldkuri_UnkMotion4_Phys")]
#[doc(alias = "itOldkuri_UnkMotion6_Phys")]
#[doc(alias = "itOldkuri_UnkMotion8_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision (`itOldkuri_UnkMotion3_Coll`, `_4_Coll`): its standing frames from
/// `xC`, and landing it stands.
#[doc(alias = "itOldkuri_UnkMotion3_Coll")]
#[doc(alias = "itOldkuri_UnkMotion4_Coll")]
pub fn coll_falling(r: &mut Running, gobj: GObjId) -> bool {
    let t = fp::fctiwz(reg(r.data.f32(attr(r, gobj, 0xC))));
    set(r, gobj, off::TIMER, t);
    ground::fall_to_floor(r, gobj, stand);
    zako::answers_false(r, gobj)
}

/// Falls, `it_802D7AF0`: facing right state 4, else 3, keeping its hits.
#[doc(alias = "it_802D7AF0")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    if reg(item(r.game, gobj).facing_dir) > 0.0 {
        enter(r, gobj, 4, change::ANIM_UPDATE | change::HIT_PRESERVE);
    } else {
        enter(r, gobj, 3, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
}

/// Falling right's animation, `itOldkuri_UnkMotion4_Anim`: looped.
#[doc(alias = "itOldkuri_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 4, change::ANIM_UPDATE);
    }
    false
}

/// Picked up, `itOldKuri_Logic0_PickedUp`: its hit records cleared, no spin, state 5.
#[doc(alias = "itOldKuri_Logic0_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hitbox::clear_hit_ids(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// Held's animation, `itOldkuri_UnkMotion5_Anim`: looped as when picked up.
#[doc(alias = "itOldkuri_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        picked_up(r, gobj);
    }
    false
}

/// Dropped, `it_2725_Logic0_Dropped`: the enemies' drop, then falling its way.
// Ports `it_2725_Logic0_Dropped`; aliased once a check reaches it (no oracle match has a fighter
// pick up a monster).
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `it_2725_Logic0_Thrown`: the enemies' throw, no stomping, no timer, state 6, upright
/// facing nowhere.
// Ports `it_2725_Logic0_Thrown`; aliased once a check reaches it (no oracle match has a fighter
// pick up a monster).
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    lib2725::clear_stomp(r.game, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    zako::face(r, gobj, 0.0);
}

/// Thrown's animation, `itOldkuri_UnkMotion6_Anim`: looped.
#[doc(alias = "itOldkuri_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 6, change::ANIM_UPDATE);
    }
    false
}

/// Thrown's collision, `itOldkuri_UnkMotion6_Coll`: touching the stage ends the flight as a KO.
#[doc(alias = "itOldkuri_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Knocked back's animation, `itOldkuri_UnkMotion9_Anim`: when it ends, in the air again, on
/// the ground it stands (settled, turned to the floor, no blink).
#[doc(alias = "itOldkuri_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if item(r.game, gobj).ground_or_air == GA_AIR {
            change_state(r, gobj, 9, change::ANIM_UPDATE);
        } else {
            stand(r, gobj);
        }
    }
    false
}

/// Knocked back's physics, `itOldkuri_UnkMotion9_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itOldkuri_UnkMotion9_Phys")]
pub fn phys9(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys_gravity(r, gobj);
    }
}

/// Knocked back's collision, `itOldkuri_UnkMotion9_Coll`: in the air it lands and stands, on
/// the ground off the floor it falls.
#[doc(alias = "itOldkuri_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        ground::fall_to_floor(r, gobj, stand);
    } else {
        ground::ground_or_fall(r, gobj, fall);
    }
    zako::answers_false(r, gobj)
}

/// Takes a hit, `it_2725_Logic0_DmgReceived`: the hit's element adds to the damage; past its
/// health (or hit while thrown, 6), a hurt sound, no stomping, blinking, its hit records
/// cleared, an enemy KO, a small quake, and flung away (toward the screen at random); else
/// knocked back along the hit (up at least a little if off the floor), state 9.
#[doc(alias = "it_2725_Logic0_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if damage > health(r, gobj) || msid == 6 {
        zako::hurt_sound(r, gobj);
        lib2725::clear_stomp(r.game, gobj);
        hitbox::start_blink(r.game, gobj);
        hitbox::clear_hit_ids(r.game, gobj);
        zako::ko(r, gobj);
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        let f = reg(r.game.objs.rng.draw_f32());
        let z = r.game.it.zako.expect("no enemies' common data");
        if f < reg(r.data.f32(z.at(8))) {
            fling_to_screen(r, gobj);
        } else {
            fling(r, gobj);
        }
    } else {
        let mut v = item(r.game, gobj).x40_vel;
        let up = zako::knockback_vel(r, gobj, &mut v);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel = v;
        if up {
            // A double 0.2 in the comparison, as the C writes it.
            if reg(ip.x40_vel.y) <= 0.2 {
                ip.x40_vel.y = 0.2;
            }
            maplib::enter_air(ip);
        } else {
            maplib::enter_ground(ip);
        }
        set(r, gobj, off::TIMER, 0);
        change_state(r, gobj, 9, change::ANIM_UPDATE);
    }
    false
}

/// Landed on by a fighter, `it_802D8098`: blinking, no stomping, its hit records cleared, an
/// enemy KO, a small quake; in the air knocked down, on the ground flattened; a squash sound
/// and the stomper's rumble.
#[doc(alias = "it_802D8098")]
pub fn stomped(r: &mut Running, gobj: GObjId) -> bool {
    hitbox::start_blink(r.game, gobj);
    lib2725::clear_stomp(r.game, gobj);
    hitbox::clear_hit_ids(r.game, gobj);
    zako::ko(r, gobj);
    let pos = item(r.game, gobj).pos;
    camera::request_quake(r, QUAKE_SMALL, Some(pos));
    if item(r.game, gobj).ground_or_air == GA_AIR {
        knock_down(r, gobj);
    } else {
        flatten(r, gobj);
    }
    play_sfx(r, gobj, 0x12E, 127, 64);
    let stomper = item(r.game, gobj)
        .xCFC
        .expect("a Goomba landed on by nobody");
    lib2725::fighter_item_rumble(r, stomper, 0xC, 0);
    false
}

/// Flattened, `it_802D813C`: stopped, its model turned to the floor facing nowhere, state 7.
#[doc(alias = "it_802D813C")]
pub fn flatten(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
    zako::align_floor(r, gobj, 0.0, 0.0);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// Flattened's animation, `itOldkuri_UnkMotion7_Anim`: when it ends, a 1P mode's trophy roll,
/// and gone.
#[doc(alias = "itOldkuri_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        zako::trophy_roll(r, gobj);
        return true;
    }
    false
}

/// The collisions that answer false (`itOldkuri_UnkMotion7_Coll`, `_11_Coll`).
#[doc(alias = "itOldkuri_UnkMotion7_Coll")]
#[doc(alias = "itOldkuri_UnkMotion11_Coll")]
pub fn coll_still(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}

/// Knocked down's collision, `itOldkuri_UnkMotion8_Coll`: whether it touched the stage.
#[doc(alias = "itOldkuri_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_touched_any(r, gobj)
}

/// Flung away's collision, `itOldkuri_UnkMotion10_Coll`: bounces off the stage.
#[doc(alias = "itOldkuri_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Knocked down from the air, `it_802D81FC`: slowed by `x8`, state 8.
#[doc(alias = "it_802D81FC")]
pub fn knock_down(r: &mut Running, gobj: GObjId) {
    let k = reg(r.data.f32(attr(r, gobj, 8)));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Knocked down's animation, `itOldkuri_UnkMotion8_Anim`: gone when it ends.
#[doc(alias = "itOldkuri_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// Flung away, `it_802D82C4`: in the air, launched along its knockback, `xDEC` cleared, upright
/// facing nowhere, state 10.
#[doc(alias = "it_802D82C4")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    zako::launch(r, gobj, false);
    set(r, gobj, off::XDEC, 0);
    zako::face(r, gobj, 0.0);
    change_state(r, gobj, 0xA, change::ANIM_UPDATE);
}

/// Flung away's animation, `itOldkuri_UnkMotion10_Anim`: looped.
#[doc(alias = "itOldkuri_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0xA, change::ANIM_UPDATE);
    }
    false
}

/// Flung toward the screen, `it_802D839C` (the inline `Item_UpdateZakoVelocity`): upright facing
/// nowhere, state 11.
#[doc(alias = "it_802D839C")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    zako::face(r, gobj, 0.0);
    change_state(r, gobj, 0xB, change::ANIM_UPDATE);
}

/// Flung toward the screen's animation, `itOldkuri_UnkMotion11_Anim`: looped.
#[doc(alias = "itOldkuri_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0xB, change::ANIM_UPDATE);
    }
    false
}

/// Flung toward the screen's physics, `itOldkuri_UnkMotion11_Phys`: its fall speed off the
/// vertical velocity.
#[doc(alias = "itOldkuri_UnkMotion11_Phys")]
pub fn phys_gravity(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Enters a state it can be landed on in, `it_802D848C`: stompable, its jumped-on callback
/// [`stomped`].
#[doc(alias = "it_802D848C")]
pub fn enter(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    change_state(r, gobj, msid, flags);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(STOMPED));
}

/// Forgets a GObj going away, `it_802D84D8` (`it_8026B894`): the adventure Goomba's (kind 208)
/// and, as `itOldKuri_Logic29_EvtUnk`, kind 190's `evt_unk`.
#[doc(alias = "it_802D84D8")]
#[doc(alias = "itOldKuri_Logic29_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = lib26b1::forget(r.game, gobj, other);
}
