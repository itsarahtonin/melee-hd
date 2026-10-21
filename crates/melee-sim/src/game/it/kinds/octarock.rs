//! The Octorok, `melee/it/kinds/itoctarock.c` (`It_Kind_Octarock`): one of the monsters a
//! container releases (`it_8026F6BC`). It stands, and when the nearest fighter comes within its
//! range it turns to face it (state 1) or hops and spits a stone at it (state 4,
//! [`super::octarockstone`]), moving with its model's bone. Hit, it's knocked back or, past its
//! health, flung away as an enemy KO. A fighter can pick it up and throw it.
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
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802E_4A44;
pub const DMG_RECEIVED: u32 = 0x802E_4B00;
pub const PICKED_UP: u32 = 0x802E_561C;
pub const DROPPED: u32 = 0x802E_5690;
pub const THROWN: u32 = 0x802E_56E8;
pub const STAND: u32 = 0x802E_4C08;
pub const FALL: u32 = 0x802E_4DB4;
pub const ANIM0: u32 = 0x802E_4C68;
pub const PHYS0: u32 = 0x802E_4CB0;
pub const COLL0: u32 = 0x802E_4D78;
pub const ANIM2: u32 = 0x802E_4DF8;
pub const PHYS2: u32 = 0x802E_4E00;
pub const COLL2: u32 = 0x802E_4E30;
pub const ANIM3: u32 = 0x802E_4F18;
pub const PHYS3: u32 = 0x802E_4FB0;
pub const COLL3: u32 = 0x802E_4FD8;
pub const ANIM4: u32 = 0x802E_50B0;
pub const PHYS4: u32 = 0x802E_51C4;
pub const COLL4: u32 = 0x802E_521C;
pub const ANIM1: u32 = 0x802E_5428;
pub const PHYS1: u32 = 0x802E_5514;
pub const COLL1: u32 = 0x802E_5554;
pub const ANIM5: u32 = 0x802E_5644;
pub const PHYS5: u32 = 0x802E_568C;
pub const ANIM6: u32 = 0x802E_5738;
pub const PHYS6: u32 = 0x802E_5780;
pub const COLL6: u32 = 0x802E_57B0;
pub const ANIM7: u32 = 0x802E_5834;
pub const PHYS7: u32 = 0x802E_587C;
pub const COLL7: u32 = 0x802E_5880;
pub const ANIM8: u32 = 0x802E_5900;
pub const PHYS8: u32 = 0x802E_5908;
pub const COLL8: u32 = 0x802E_5924;

/// Its variables' offsets in the union (`itOctarock_ItemVars`).
pub mod off {
    /// `x18`, the common `x14.y`, cleared as it's flung away.
    pub const X18: usize = 0x18;
    /// `x20`: the moving bone's last translation (`itUpdateVelocityFromBone`).
    pub const BONE: usize = 0x20;
    /// `x2C`: whether its hop has left the ground.
    pub const AIRBORNE: usize = 0x2C;
    /// `x32` (a halfword): a state's timer.
    pub const TIMER: usize = 0x32;
}

/// Its special attributes (`itOctarockAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("an Octorok without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// `*attr->x0`: its health.
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("an Octorok's health");
    r.data.u32(p) as i32
}

fn timer(r: &mut Running, gobj: GObjId) -> i16 {
    zako::bytes(r.game, gobj).i16(off::TIMER)
}

fn set_timer(r: &mut Running, gobj: GObjId, v: i16) {
    zako::bytes(r.game, gobj).set_i16(off::TIMER, v);
}

/// `GA_Ground` and `GA_Air`.
const GA_GROUND: i32 = 0;
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;

/// The collision's facing from the item's (the inline `itOctarock_SetFacingDir`).
fn set_coll_facing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(f);
}

/// Stops (the inline `itResetVelocity`).
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Runs one of the Octorok's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM2 | ANIM8 => answers_false(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 => coll7(r, gobj),
        COLL8 => coll8(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Octorok's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        STAND => stand(r, gobj),
        FALL => fall(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS2 | PHYS6 => phys_fall(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS5 | PHYS7 => still(r, gobj),
        PHYS8 => phys_gravity(r, gobj),
        _ => return false,
    }
    true
}

/// The Octorok's setup, `it_802E4A44`: the enemies' common variables, facing most fighters,
/// upright, its collision an enemy's (`Item_InitZakoCollision`), no timer, and falling.
#[doc(alias = "it_802E4A44")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    set_coll_facing(r, gobj);
    zako::face(r, gobj, facing);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set_timer(r, gobj, 0);
    fall(r, gobj);
}

/// Takes a hit, `it_802E4B00`: the hit's element adds to the damage; past its health (or hit
/// while thrown, 6) it's beaten (`Item_ZakoDefeat`) and flung away (toward the screen at
/// random); else knocked back.
#[doc(alias = "it_802E4B00")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if damage > health(r, gobj) || msid == 6 {
        zako::hurt_sound(r, gobj);
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
        knocked_back(r, gobj);
    }
    false
}

/// Stands, `it_802E4C08`: settled, its timer the check's period `x1C`, state 0.
#[doc(alias = "it_802E4C08")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    let p = r.data.u16(attr(r, gobj, 0x1C)) as i16;
    set_timer(r, gobj, p);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Standing's animation, `itOctarock_UnkMotion0_Anim`: looped.
#[doc(alias = "itOctarock_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Standing's physics, `itOctarock_UnkMotion0_Phys`: each period it looks for the nearest
/// fighter ([`sees`]): none in range, another period; behind it, it turns; ahead, it shoots.
/// Along the floor.
#[doc(alias = "itOctarock_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = timer(r, gobj);
    if t == 0 {
        match sees(r, gobj) {
            0 => {
                let p = r.data.u16(attr(r, gobj, 0x1C)) as i16;
                set_timer(r, gobj, p);
            }
            1 => turn(r, gobj),
            2 => shoot(r, gobj),
            _ => {}
        }
    } else {
        set_timer(r, gobj, t.wrapping_sub(1));
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, facing, 0.0);
}

/// Standing's collision, `itOctarock_UnkMotion0_Coll`: off the floor it falls.
#[doc(alias = "itOctarock_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Falls, `it_802E4DB4`: slowed by `x4`, state 2.
#[doc(alias = "it_802E4DB4")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    change_state(r, gobj, 2, UNK_0X1 | change::ANIM_UPDATE);
}

/// The states' animation callbacks that answer false (`itOctarock_UnkMotion2_Anim`, `_8_Anim`).
#[doc(alias = "itOctarock_UnkMotion2_Anim")]
#[doc(alias = "itOctarock_UnkMotion8_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states' physics that do nothing (`itOctarock_UnkMotion5_Phys`, `_7_Phys`).
#[doc(alias = "itOctarock_UnkMotion5_Phys")]
#[doc(alias = "itOctarock_UnkMotion7_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// The falling states' physics (`itOctarock_UnkMotion2_Phys`, `_6_Phys`): gravity.
#[doc(alias = "itOctarock_UnkMotion2_Phys")]
#[doc(alias = "itOctarock_UnkMotion6_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itOctarock_UnkMotion2_Coll`: landing it stands.
#[doc(alias = "itOctarock_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, stand);
    zako::answers_false(r, gobj)
}

/// Knocked back, `it_802E4E6C`: along the hit's knockback (up at least a little, then in the
/// air, if off the floor), facing as before the hit, state 3.
#[doc(alias = "it_802E4E6C")]
pub fn knocked_back(r: &mut Running, gobj: GObjId) {
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        if reg(ip.x40_vel.y) <= reg(0.2) {
            ip.x40_vel.y = 0.2;
        }
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
    }
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Knocked back's animation, `itOctarock_UnkMotion3_Anim`: when it ends, in the air again, on
/// the ground it stands.
#[doc(alias = "itOctarock_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if item(r.game, gobj).ground_or_air == GA_AIR {
            change_state(r, gobj, 3, change::ANIM_UPDATE);
        } else {
            stand(r, gobj);
        }
    }
    false
}

/// Knocked back's physics, `itOctarock_UnkMotion3_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itOctarock_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys_gravity(r, gobj);
    }
}

/// Knocked back's collision, `itOctarock_UnkMotion3_Coll`: in the air landing it stands, on
/// the ground off the floor it falls.
#[doc(alias = "itOctarock_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        ground::fall_to_floor(r, gobj, stand);
    } else {
        ground::ground_or_fall(r, gobj, fall);
    }
    zako::answers_false(r, gobj)
}

/// Shoots, `it_802E503C`: its timer 21, stopped, on the ground, the bone's motion forgotten, in
/// the air (its hop), state 4.
#[doc(alias = "it_802E503C")]
pub fn shoot(r: &mut Running, gobj: GObjId) {
    set_timer(r, gobj, 0x15);
    stop(r, gobj);
    zako::bytes(r.game, gobj).set_i32(off::AIRBORNE, 0);
    forget_bone(r, gobj);
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// The model's bone `i` from its bone table.
fn bone(r: &Running, gobj: GObjId, i: usize) -> Option<JObjId> {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("an Octorok without a bone table");
    r.game.it.bone_tables[t.0].bones[i]
}

/// Shooting's animation, `itOctarock_UnkMotion4_Anim`: moved by its bone; at its timer's end a
/// stone from its mouth's bone (14) and its spit sound; when the animation ends, it stands (its
/// timer counting on from the period).
#[doc(alias = "itOctarock_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    let b = bone(r, gobj, 1);
    follow_bone(r, b, gobj);
    if timer(r, gobj) == 0 {
        let j = bone(r, gobj, 14);
        let v = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        let dir = fp::fctiwz(reg(item(r.game, gobj).facing_dir));
        let (speed, lo, hi) = (
            attr_f32(r, gobj, 0x10),
            attr_f32(r, gobj, 0x14),
            attr_f32(r, gobj, 0x18),
        );
        super::octarockstone::spawn(r, gobj, v, dir, speed, lo, hi);
        play_sfx(r, gobj, 0x137, 127, 64);
    }
    if !lib2725::is_animating(r.game, gobj) {
        let _ = ground::air(r, gobj);
        set_timer(r, gobj, 0);
        stand(r, gobj);
    }
    let t = timer(r, gobj).wrapping_sub(1);
    set_timer(r, gobj, t);
    false
}

/// Shooting's physics, `itOctarock_UnkMotion4_Phys`: once it moves up it's in the air.
#[doc(alias = "itOctarock_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let airborne = zako::bytes(r.game, gobj).i32(off::AIRBORNE);
    if airborne == 0 && reg(item(r.game, gobj).x40_vel.y) > 0.0 {
        maplib::enter_air(item_mut(r.game, gobj));
        zako::bytes(r.game, gobj).set_i32(off::AIRBORNE, 1);
    }
}

/// Shooting's collision, `itOctarock_UnkMotion4_Coll`: on the ground off the floor it falls; in
/// the air landing it's back down, else it keeps its collision's position (stopped at a
/// ceiling).
#[doc(alias = "itOctarock_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_GROUND {
        if !ground::ground_probe(r, gobj) {
            fall(r, gobj);
        }
    } else if ground::air_probe(r, gobj) {
        zako::bytes(r.game, gobj).set_i32(off::AIRBORNE, 0);
    } else {
        maplib::touched_ceiling(r.game, gobj);
        let ip = item_mut(r.game, gobj);
        ip.pos = ip.x378_itemColl.cur_pos;
    }
    zako::answers_false(r, gobj)
}

/// Where the nearest fighter is, `it_802E52E0`: 0 out of range (`x8` across, `xC` up or down),
/// 1 behind it, 2 ahead.
#[doc(alias = "it_802E52E0")]
pub fn sees(r: &mut Running, gobj: GObjId) -> i32 {
    let pos = item(r.game, gobj).pos;
    let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) else {
        return 0;
    };
    let v = r.game.ft_pos(f);
    let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
    if abs(fp::fsubs(reg(v.x), reg(pos.x))) <= reg(attr_f32(r, gobj, 8))
        && abs(fp::fsubs(reg(v.y), reg(pos.y))) <= reg(attr_f32(r, gobj, 0xC))
    {
        let facing = if reg(pos.x) < reg(v.x) {
            1.0
        } else {
            fp::fneg(1.0)
        };
        return if facing != reg(item(r.game, gobj).facing_dir) {
            1
        } else {
            2
        };
    }
    0
}

/// Turns, `it_802E53C8`: stopped, its timer 22, its hop on the ground, the bone's motion
/// forgotten, state 1.
#[doc(alias = "it_802E53C8")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set_timer(r, gobj, 0x16);
    zako::bytes(r.game, gobj).set_i32(off::AIRBORNE, 0);
    forget_bone(r, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Turning's animation, `itOctarock_UnkMotion1_Anim`: moved by its bone; at its timer's end it
/// faces the other way; when the animation ends, it stands.
#[doc(alias = "itOctarock_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let b = bone(r, gobj, 1);
    follow_bone(r, b, gobj);
    if timer(r, gobj) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r, gobj);
    }
    if !lib2725::is_animating(r.game, gobj) {
        let _ = ground::ground_probe(r, gobj);
        set_timer(r, gobj, 0);
        stand(r, gobj);
        return false;
    }
    let t = timer(r, gobj).wrapping_sub(1);
    set_timer(r, gobj, t);
    false
}

/// Turning's physics, `itOctarock_UnkMotion1_Phys`: on the ground moving up, it's in the air.
#[doc(alias = "itOctarock_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air == GA_GROUND && reg(ip.x40_vel.y) > 0.0 {
        maplib::enter_air(ip);
    }
}

/// Turning's collision, `itOctarock_UnkMotion1_Coll`: on the ground off the floor it falls; in
/// the air landing it's on the ground, else it keeps its collision's position.
#[doc(alias = "itOctarock_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_GROUND {
        if !ground::ground_probe(r, gobj) {
            fall(r, gobj);
        }
    } else if ground::air_probe(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
    } else {
        maplib::touched_ceiling(r.game, gobj);
        let ip = item_mut(r.game, gobj);
        ip.pos = ip.x378_itemColl.cur_pos;
    }
    zako::answers_false(r, gobj)
}

/// Picked up, `itOctarock_Logic2_PickedUp`: state 5.
#[doc(alias = "itOctarock_Logic2_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// Held's animation, `itOctarock_UnkMotion5_Anim`: looped.
#[doc(alias = "itOctarock_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 5, change::ANIM_UPDATE);
    }
    false
}

/// Dropped, `itOctarock_Logic2_Dropped`: the enemies' drop, then falling.
// Ports `itOctarock_Logic2_Dropped`; aliased once a check reaches it (no oracle match has a
// fighter pick up a monster).
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `itOctarock_Logic2_Thrown`: the enemies' throw, its timer cleared, state 6.
// Ports `itOctarock_Logic2_Thrown`; aliased once a check reaches it (no oracle match has a
// fighter pick up a monster).
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set_timer(r, gobj, 0);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Thrown's animation, `itOctarock_UnkMotion6_Anim`: looped.
#[doc(alias = "itOctarock_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 6, change::ANIM_UPDATE);
    }
    false
}

/// Thrown's collision, `itOctarock_UnkMotion6_Coll`: touching the stage ends the flight as a KO.
#[doc(alias = "itOctarock_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Flung away's collision, `itOctarock_UnkMotion7_Coll`: bounces off the stage.
#[doc(alias = "itOctarock_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Flung toward the screen's collision, `itOctarock_UnkMotion8_Coll`: false.
#[doc(alias = "itOctarock_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}

/// Flung away, `it_802E57D4`: in the air, launched along its knockback, `x18` cleared, state 7.
#[doc(alias = "it_802E57D4")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    zako::launch(r, gobj, false);
    zako::bytes(r.game, gobj).set_i32(off::X18, 0);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// Flung away's animation, `itOctarock_UnkMotion7_Anim`: looped.
#[doc(alias = "itOctarock_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 7, change::ANIM_UPDATE);
    }
    false
}

/// Flung toward the screen, `it_802E58A0` (the inline `Item_UpdateZakoVelocity`): state 8.
#[doc(alias = "it_802E58A0")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 8, UNK_0X1 | change::ANIM_UPDATE);
}

/// Flung toward the screen's physics, `itOctarock_UnkMotion8_Phys` (and the knockback's in the
/// air): its fall speed off the vertical velocity.
#[doc(alias = "itOctarock_UnkMotion8_Phys")]
pub fn phys_gravity(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Forgets the moving bone's last translation, `it_802E5944`.
#[doc(alias = "it_802E5944")]
pub fn forget_bone(r: &mut Running, gobj: GObjId) {
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::BONE, 0.0);
    v.set_f32(off::BONE + 4, 0.0);
    v.set_f32(off::BONE + 8, 0.0);
}

/// The bone's translation since last time becomes the velocity, and the bone stays put,
/// `it_802E595C` (the inline `itUpdateVelocityFromBone`); nothing for no bone.
#[doc(alias = "it_802E595C")]
pub fn follow_bone(r: &mut Running, j: Option<JObjId>, gobj: GObjId) {
    let Some(j) = j else {
        return;
    };
    let pos = r.game.objs.jobjs[j].translate;
    let prev = zako::bytes(r.game, gobj).vec(off::BONE);
    let facing = reg(item(r.game, gobj).facing_dir);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(facing, fp::fsubs(reg(pos.z), reg(prev.z))));
    ip.x40_vel.y = mem(fp::fsubs(reg(pos.y), reg(prev.y)));
    ip.x40_vel.z = mem(fp::fsubs(reg(pos.x), reg(prev.x)));
    zako::bytes(r.game, gobj).set_vec(off::BONE, pos);
    jobj::set_translate(&mut r.game.objs.jobjs, j, Vec3::new(0.0, 0.0, 0.0));
}

/// Forgets a GObj the Octorok points to, `it_802E5AA4` (`it_8026B894`).
#[doc(alias = "it_802E5AA4")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
