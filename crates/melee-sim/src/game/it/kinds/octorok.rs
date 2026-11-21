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

use super::super::item::{change, change_state, play_sfx};
use super::super::zako::{self, GA_GROUND, Knock, attr, attr_f32, set_coll_facing, stop};
use super::super::{ground, item, item_mut, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
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

/// Its variables' offsets in the union (`itOctarock_ItemVars`; its `x18` is the common `x14.y`,
/// [`zako::BOUNCES`]).
pub mod off {
    /// `x20`: the moving bone's last translation (`itUpdateVelocityFromBone`).
    pub const BONE: usize = 0x20;
    /// `x2C`: whether its hop has left the ground.
    pub const AIRBORNE: usize = 0x2C;
    /// `x32` (a halfword): a state's timer.
    pub const TIMER: usize = 0x32;
}

fn timer(r: &mut Running, gobj: GObjId) -> i16 {
    zako::bytes(r.game, gobj).i16(off::TIMER)
}

fn set_timer(r: &mut Running, gobj: GObjId, v: i16) {
    zako::bytes(r.game, gobj).set_i16(off::TIMER, v);
}

/// The Octorok's settings for the enemies' shared code (its attributes are
/// `itOctarockAttributes`, through [`zako::attr`]).
pub const OCTOROK: zako::Enemy = zako::Enemy {
    thrown: 6,
    flung: (7, change::ANIM_UPDATE),
    to_screen: (8, zako::UNK_0X1 | change::ANIM_UPDATE),
    on_hit: None,
    keeps_facing: true,
    on_beaten: None,
    stompable: false,
    clears_hits: true,
    knocked: knocked_back,
    flung_keeps_facing: false,
    flung_upright: false,
    flung_checks_blast_zones: false,
};

/// Runs one of the Octorok's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => zako::damaged(r, gobj, &OCTOROK),
        ANIM0 => zako::looped(r, gobj, 0, change::ANIM_UPDATE),
        COLL0 => coll0(r, gobj),
        ANIM2 | ANIM8 => zako::answers_false(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => zako::knocked_anim(r, gobj, 3, stand),
        COLL3 => zako::knocked_coll(r, gobj, stand, fall),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM5 => zako::looped(r, gobj, 5, change::ANIM_UPDATE),
        ANIM6 => zako::looped(r, gobj, 6, change::ANIM_UPDATE),
        COLL6 => zako::flight_lands(r, gobj, None),
        ANIM7 => zako::looped(r, gobj, 7, change::ANIM_UPDATE),
        COLL7 => zako::bounce(r, gobj),
        COLL8 => zako::answers_false(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Octorok's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        STAND => stand(r, gobj),
        FALL => fall(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS2 | PHYS6 => zako::fall_physics(r, gobj),
        PHYS3 => zako::knocked_physics(r, gobj),
        PHYS4 => phys4(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS5 | PHYS7 => zako::still(r, gobj),
        PHYS8 => zako::flight_physics(r, gobj),
        _ => return false,
    }
    true
}

/// The Octorok's setup, `it_802E4A44`: the enemies' common variables, facing most fighters,
/// upright, its collision an enemy's, no timer, and falling.
#[doc(alias = "it_802E4A44")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    set_coll_facing(r.game, gobj);
    zako::face(r, gobj, facing);
    zako::init_collision(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set_timer(r, gobj, 0);
    fall(r, gobj);
}

/// Stands, `it_802E4C08`: settled, its timer the check's period `x1C`, state 0.
#[doc(alias = "it_802E4C08")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    let p = r.data.u16(attr(r, gobj, 0x1C)) as i16;
    set_timer(r, gobj, p);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
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
    zako::along_floor(r.game, gobj);
    let facing = item(r.game, gobj).facing_dir;
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
    change_state(r, gobj, 2, zako::UNK_0X1 | change::ANIM_UPDATE);
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
    zako::knock_back(r, gobj, Knock::AtLeast(0.2f32 as f64));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r.game, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Shoots, `it_802E503C`: its timer 21, stopped, on the ground, the bone's motion forgotten, in
/// the air (its hop), state 4.
#[doc(alias = "it_802E503C")]
pub fn shoot(r: &mut Running, gobj: GObjId) {
    set_timer(r, gobj, 0x15);
    stop(r.game, gobj);
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
        // The stone's spin takes its way from a stack word this lookup leaves, the joint's
        // address (a negative float, `octarockstone::spawn`): without the bone it would be
        // another word.
        if j.is_none() {
            panic!(
                "it_802E8ADC: the stone's spin reads a stack word the mouth bone's lookup leaves, and with no bone 14 it isn't the joint's address: that isn't modeled"
            );
        }
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
    stop(r.game, gobj);
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
        set_coll_facing(r.game, gobj);
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

/// Dropped, `itOctarock_Logic2_Dropped`: the enemies' drop, then falling.
#[doc(alias = "itOctarock_Logic2_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `itOctarock_Logic2_Thrown`: the enemies' throw, its timer cleared, state 6.
#[doc(alias = "itOctarock_Logic2_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set_timer(r, gobj, 0);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
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
