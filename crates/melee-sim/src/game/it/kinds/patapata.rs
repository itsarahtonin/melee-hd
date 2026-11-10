//! The Koopa Paratroopa, `melee/it/kinds/itpatapata.c` (`It_Kind_Patapata`): an adventure
//! enemy the stages' spawner makes (`grZakoGenerator_801CA8B4`). After a short pause (state 1)
//! it flies (state 2) in one of three ways by its kind (`x40`): 0 hops along the floor, 1 weaves
//! along a sine path between its turns, 2 bobs up and down where it is; it turns (state 3) at
//! walls and at its path's ends. A hit or a stomp knocks the Koopa out of it: a Koopa Troopa of
//! its kind (a green shell under 2, else red) knocked back where it was, while its wings flutter
//! off (state 4) and go. Held (5), dropped (7) or thrown (6), a throw ends as an enemy KO when
//! it hits the stage.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802E_05A0;
pub const SETTLE: u32 = 0x802E_0678;
pub const ANIM1: u32 = 0x802E_06B4;
pub const PHYS1: u32 = 0x802E_06BC;
pub const COLL1: u32 = 0x802E_06F8;
pub const FLY: u32 = 0x802E_0734;
pub const ANIM2: u32 = 0x802E_0974;
pub const PHYS3: u32 = 0x802E_09BC;
pub const COLL3: u32 = 0x802E_0B78;
pub const TURN: u32 = 0x802E_0D9C;
pub const ANIM3: u32 = 0x802E_0E10;
pub const DMG_RECEIVED: u32 = 0x802E_0EE0;
/// `it_802E0F1C`, its jumped-on callback.
pub const STOMPED: u32 = 0x802E_0F1C;
pub const PICKED_UP: u32 = 0x802E_0F70;
pub const ANIM5: u32 = 0x802E_0F98;
pub const PHYS5: u32 = 0x802E_0FE0;
pub const DROPPED: u32 = 0x802E_0FE4;
pub const ANIM7: u32 = 0x802E_1040;
pub const PHYS7: u32 = 0x802E_10A0;
pub const COLL7: u32 = 0x802E_10A4;
pub const THROWN: u32 = 0x802E_10C8;
pub const ANIM6: u32 = 0x802E_1128;
pub const PHYS6: u32 = 0x802E_1170;
pub const COLL6: u32 = 0x802E_11A0;
pub const ANIM4: u32 = 0x802E_13EC;
pub const PHYS4: u32 = 0x802E_1498;
pub const COLL4: u32 = 0x802E_1578;

/// Its variables' offsets in the union (`itPatapata_ItemVars`).
pub mod off {
    /// `x4`: the common spawner slot, which the Koopa knocked out of it takes over.
    pub const SLOT: usize = 4;
    /// `x20`: frames left of a stretch of its path (sideways for kind 1, up or down for 2), or
    /// of its wings' flight.
    pub const X20: usize = 0x20;
    /// `x24`: kind 1's frames left of an up or down stretch.
    pub const X24: usize = 0x24;
    /// `x28`: a countdown (its pause, kind 1's turn point, the wings' flicker).
    pub const X28: usize = 0x28;
    /// `x2C`: kind 1's sideways acceleration; the wings' drift.
    pub const X2C: usize = 0x2C;
    /// `x30`: its vertical acceleration.
    pub const X30: usize = 0x30;
    /// `x40`: its kind (how it flies, and its Koopa's shell).
    pub const KIND: usize = 0x40;
    /// `x44`: whether the wings are hidden this flicker.
    pub const X44: usize = 0x44;
}

/// `GA_Air`.
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_ANIM_UPDATE`, as the file passes it (2).
const ANIM_UPDATE: u32 = super::super::item::change::ANIM_UPDATE;

/// Runs one of the Paratroopa's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => answers_false(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM3 => anim3(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        STOMPED => stomped(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 => coll7(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Paratroopa's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        SETTLE => settle(r, gobj),
        PHYS1 => phys1(r, gobj),
        FLY => fly(r, gobj),
        PHYS3 => phys3(r, gobj),
        TURN => turn(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS5 | PHYS7 => still(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS6 => phys6(r, gobj),
        PHYS4 => phys4(r, gobj),
        _ => return false,
    }
    true
}

/// Its special attributes (`itPatapataAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Koopa Paratroopa without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

/// `x0->x4`: its speed.
fn speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r
        .data
        .ptr(attr(r, gobj, 0))
        .expect("a Koopa Paratroopa's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

fn get_f32(r: &mut Running, gobj: GObjId, off: usize) -> f32 {
    zako::bytes(r.game, gobj).f32(off)
}

fn set_f32(r: &mut Running, gobj: GObjId, off: usize, v: f32) {
    zako::bytes(r.game, gobj).set_f32(off, v);
}

/// `(f32) i`.
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// The collision's facing from the item's (`mpCollSetFacingDir`).
fn set_coll_facing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(f);
}

/// Its model's bone `i` (`xBBC_dynamicBoneTable->bones[i]`).
fn bone(r: &Running, gobj: GObjId, i: usize) -> Option<JObjId> {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a Koopa Paratroopa without bones");
    r.game.it.bone_tables[t.0].bones[i]
}

/// The Paratroopa's setup, `it_802E05A0`: the enemies' common variables, facing most
/// fighters, its collision an enemy's (the inline `Item_InitZakoCollision`), a 10-frame pause,
/// kind 2, in the air, settled.
#[doc(alias = "it_802E05A0")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let f = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = f;
    set_coll_facing(r, gobj);
    zako::face(r, gobj, f);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::X28, 0xA);
    set(r, gobj, off::X24, 0);
    set(r, gobj, off::X20, 0);
    set(r, gobj, off::KIND, 2);
    maplib::enter_air(item_mut(r.game, gobj));
    settle(r, gobj);
}

/// Pauses, `it_802E0678`: settled as an enemy, state 1.
#[doc(alias = "it_802E0678")]
pub fn settle(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    enter(r, gobj, 1, ANIM_UPDATE);
}

/// The pause's animation, `itPatapata_UnkMotion1_Anim`.
#[doc(alias = "itPatapata_UnkMotion1_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The held and dropped states' physics, which do nothing (`itPatapata_UnkMotion5_Phys`, 7).
// Ports itPatapata_UnkMotion5_Phys; aliased once a check reaches it.
// Ports itPatapata_UnkMotion7_Phys; aliased once a check reaches it.
pub fn still(_: &mut Running, _: GObjId) {}

/// The pause's physics, `itPatapata_UnkMotion1_Phys`: its countdown done, it flies.
#[doc(alias = "itPatapata_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::X28);
    if t == 0 {
        fly(r, gobj);
        return;
    }
    set(r, gobj, off::X28, t.wrapping_sub(1));
}

/// The pause's collision, `itPatapata_UnkMotion1_Coll`: the air's or the ground's.
#[doc(alias = "itPatapata_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        let _ = ground::air(r, gobj);
    } else {
        let _ = ground::ground(r, gobj);
    }
    false
}

/// Flies, `it_802E0734`: by its kind, 0 forward at its speed with a fall over `x10` frames
/// for a hop `xC` high, 1 forward with a sideways turn over half of `x14` and a bob half of
/// `x18` high over half of `x1C` frames, 2 a bob half of `x20` high over half of `x24` frames;
/// its model facing its way, no blink, state 2.
#[doc(alias = "it_802E0734")]
pub fn fly(r: &mut Running, gobj: GObjId) {
    let speed = reg(speed(r, gobj));
    let facing = reg(item(r.game, gobj).facing_dir);
    match get(r, gobj, off::KIND) {
        0 => {
            let x10 = attr_i32(r, gobj, 0x10);
            let x10sq = x10.wrapping_mul(x10);
            let xc = reg(attr_f32(r, gobj, 0xC));
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(facing, speed));
            ip.x40_vel.y = 0.0;
            let a = fp::fmuls(fp::fneg(1.0), fp::fdivs(fp::fmuls(2.0, xc), to_f32(x10sq)));
            set_f32(r, gobj, off::X30, mem(a));
        }
        1 => {
            item_mut(r.game, gobj).x40_vel.x = mem(fp::fmuls(facing, speed));
            let half_width = fp::fdivs(reg(attr_f32(r, gobj, 0x14)), to_f32(2));
            let frames = fp::fctiwz(fp::fdivs(half_width, speed));
            set(r, gobj, off::X20, frames);
            set(r, gobj, off::X28, frames / 2);
            let accel = fp::fmuls(
                fp::fneg(facing),
                fp::fdivs(
                    fp::fmuls(2.0, half_width),
                    to_f32(frames.wrapping_mul(frames)),
                ),
            );
            set_f32(r, gobj, off::X2C, mem(accel));
            let x24 = attr_i32(r, gobj, 0x1C) / 2;
            let half_height = fp::fdivs(reg(attr_f32(r, gobj, 0x18)), to_f32(2));
            set(r, gobj, off::X24, x24);
            item_mut(r.game, gobj).x40_vel.y = mem(fp::fmuls(
                fp::fneg(1.0),
                fp::fdivs(half_height, to_f32(x24)),
            ));
            let a = fp::fdivs(fp::fmuls(2.0, half_height), to_f32(x24.wrapping_mul(x24)));
            set_f32(r, gobj, off::X30, mem(a));
        }
        2 => {
            let x20 = attr_i32(r, gobj, 0x24) / 2;
            let half_height = fp::fdivs(reg(attr_f32(r, gobj, 0x20)), to_f32(2));
            set(r, gobj, off::X20, x20);
            item_mut(r.game, gobj).x40_vel.y = mem(fp::fmuls(
                fp::fneg(1.0),
                fp::fdivs(half_height, to_f32(x20)),
            ));
            let a = fp::fdivs(fp::fmuls(2.0, half_height), to_f32(x20.wrapping_mul(x20)));
            set_f32(r, gobj, off::X30, mem(a));
        }
        _ => {}
    }
    item_mut(r.game, gobj).x40_vel.z = 0.0;
    let f = item(r.game, gobj).facing_dir;
    zako::face(r, gobj, f);
    hitbox::end_blink(r.game, gobj);
    enter(r, gobj, 2, ANIM_UPDATE);
}

/// Flying's animation, `itPatapata_UnkMotion2_Anim`: looped.
#[doc(alias = "itPatapata_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 2, ANIM_UPDATE);
    }
    false
}

/// Flying's and turning's physics, `itPatapata_UnkMotion3_Phys`: flying, at the script's turn
/// point it turns; then by its kind, 0 falls, 1 runs its sideways stretch (turning at its
/// middle, the acceleration flipping at its end) and its bob, 2 its bob.
#[doc(alias = "itPatapata_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    if lib2725::xdc0_has(r.game, gobj, 2) && item(r.game, gobj).msid == 2 {
        turn(r, gobj);
    }
    match get(r, gobj, off::KIND) {
        0 => {
            let a = reg(get_f32(r, gobj, off::X30));
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), a));
        }
        1 => {
            if get(r, gobj, off::X20) == 0 {
                let half = fp::fdivs(reg(attr_f32(r, gobj, 0x14)), to_f32(2));
                let frames = fp::fctiwz(fp::fdivs(half, reg(speed(r, gobj))));
                set(r, gobj, off::X20, frames);
                let a = reg(get_f32(r, gobj, off::X2C));
                set_f32(r, gobj, off::X2C, mem(fp::fmuls(a, to_f32(-1))));
            } else {
                let a = reg(get_f32(r, gobj, off::X2C));
                let ip = item_mut(r.game, gobj);
                ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), a));
                let n = get(r, gobj, off::X20).wrapping_sub(1);
                set(r, gobj, off::X20, n);
                if get(r, gobj, off::X28) == get(r, gobj, off::X20) {
                    turn(r, gobj);
                }
            }
            bob(r, gobj, off::X24, attr_i32(r, gobj, 0x1C) / 2);
        }
        2 => {
            let n = attr_i32(r, gobj, 0x24) / 2;
            bob(r, gobj, off::X20, n);
        }
        _ => {}
    }
}

/// One frame of a bob: its stretch's frames (at `count`) done, a new stretch of `frames` the
/// other way; else its vertical acceleration.
fn bob(r: &mut Running, gobj: GObjId, count: usize, frames: i32) {
    if get(r, gobj, count) == 0 {
        set(r, gobj, count, frames);
        let a = reg(get_f32(r, gobj, off::X30));
        set_f32(r, gobj, off::X30, mem(fp::fmuls(a, to_f32(-1))));
    } else {
        let a = reg(get_f32(r, gobj, off::X30));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), a));
        let n = get(r, gobj, count).wrapping_sub(1);
        set(r, gobj, count, n);
    }
}

/// Flying's and turning's collision, `itPatapata_UnkMotion3_Coll`: at a wall, kind 0 turns
/// (flying), kind 1 starts its sideways stretch back the other way (and turns, flying); on a
/// floor, kind 0 hops again, 1 bounces its bob, 2 springs up a full bob.
#[doc(alias = "itPatapata_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    let landed = ground::air(r, gobj);
    if maplib::touched_wall(r.game, gobj) != 0 {
        match get(r, gobj, off::KIND) {
            0 => {
                if item(r.game, gobj).msid == 2 {
                    turn(r, gobj);
                }
            }
            1 => {
                let speed = reg(speed(r, gobj));
                let ip = item_mut(r.game, gobj);
                ip.x40_vel.x = mem(fp::fmuls(fp::fneg(reg(ip.facing_dir)), speed));
                let width = reg(attr_f32(r, gobj, 0x14));
                let frames = fp::fctiwz(fp::fdivs(width, speed));
                set(r, gobj, off::X20, frames);
                set(r, gobj, off::X28, frames / 2);
                let facing = reg(item(r.game, gobj).facing_dir);
                let accel = fp::fmuls(
                    facing,
                    fp::fdivs(fp::fmuls(2.0, width), to_f32(frames.wrapping_mul(frames))),
                );
                set_f32(r, gobj, off::X2C, mem(accel));
                if item(r.game, gobj).msid == 2 {
                    turn(r, gobj);
                }
            }
            _ => {}
        }
    }
    if landed {
        match get(r, gobj, off::KIND) {
            0 => {
                let x10 = attr_i32(r, gobj, 0x10);
                let dir = reg(item(r.game, gobj).facing_dir);
                let xc = reg(attr_f32(r, gobj, 0xC));
                let speed = reg(speed(r, gobj));
                let ip = item_mut(r.game, gobj);
                ip.x40_vel.x = mem(fp::fmuls(dir, speed));
                ip.x40_vel.y = mem(fp::fdivs(xc, to_f32(x10)));
            }
            1 => {
                let ip = item_mut(r.game, gobj);
                ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), fp::fneg(1.0)));
                let a = reg(get_f32(r, gobj, off::X30));
                set_f32(r, gobj, off::X30, mem(fp::fmuls(a, fp::fneg(1.0))));
            }
            2 => {
                let frames = attr_i32(r, gobj, 0x24);
                let height = reg(attr_f32(r, gobj, 0x20));
                set(r, gobj, off::X20, frames);
                item_mut(r.game, gobj).x40_vel.y = mem(fp::fdivs(height, to_f32(frames)));
                let a = fp::fmuls(
                    fp::fneg(1.0),
                    fp::fdivs(fp::fmuls(2.0, height), to_f32(frames.wrapping_mul(frames))),
                );
                set_f32(r, gobj, off::X30, mem(a));
            }
            _ => {}
        }
    }
    false
}

/// Turns, `it_802E0D9C`: kind 0 heads back at its speed; state 3, its facing flipped.
#[doc(alias = "it_802E0D9C")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    if get(r, gobj, off::KIND) == 0 {
        let speed = reg(speed(r, gobj));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(fp::fneg(reg(ip.facing_dir)), speed));
    }
    enter(r, gobj, 3, ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
}

/// Turning's animation, `itPatapata_UnkMotion3_Anim`: at its end, its collision and model
/// facing its new way, its sideways speed turned to match, flying.
#[doc(alias = "itPatapata_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        set_coll_facing(r, gobj);
        let f = item(r.game, gobj).facing_dir;
        zako::face(r, gobj, f);
        let ip = item_mut(r.game, gobj);
        let (vx, facing) = (reg(ip.x40_vel.x), reg(ip.facing_dir));
        if vx < 0.0 && facing == 1.0 || vx > 0.0 && facing == fp::fneg(1.0) {
            ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::fneg(1.0)));
        }
        enter(r, gobj, 2, ANIM_UPDATE);
    }
    false
}

/// Takes a hit, `it_2725_Logic4_DmgReceived`: thrown (6) it ends as a KO, else the Koopa is
/// knocked out of it.
#[doc(alias = "it_2725_Logic4_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 6 {
        ko(r, gobj);
    } else {
        unwing(r, gobj);
    }
    false
}

/// Landed on by a fighter, `it_802E0F1C`: the Koopa knocked out of it, a sound and the
/// stomper's rumble.
#[doc(alias = "it_802E0F1C")]
pub fn stomped(r: &mut Running, gobj: GObjId) -> bool {
    unwing(r, gobj);
    play_sfx(r, gobj, 0x12E, 127, 64);
    let stomper = item(r.game, gobj)
        .xCFC
        .expect("a Koopa Paratroopa landed on by nobody");
    lib2725::fighter_item_rumble(r, stomper, 12, 0);
    false
}

/// Picked up, `itPatapata_Logic4_PickedUp`: state 5.
// Ports itPatapata_Logic4_PickedUp; aliased once a check reaches it.
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, ANIM_UPDATE);
}

/// Held's animation, `itPatapata_UnkMotion5_Anim`: looped.
// Ports itPatapata_UnkMotion5_Anim; aliased once a check reaches it.
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 5, ANIM_UPDATE);
    }
    false
}

/// Dropped, `it_2725_Logic4_Dropped`: the enemies' drop, its counts cleared, a 2-frame pause,
/// state 7.
// Ports it_2725_Logic4_Dropped; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    set(r, gobj, off::X24, 0);
    set(r, gobj, off::X20, 0);
    set(r, gobj, off::X28, 2);
    change_state(r, gobj, 7, ANIM_UPDATE);
}

/// Dropped's animation, `itPatapata_UnkMotion7_Anim`: its pause done, settled, state 1.
// Ports itPatapata_UnkMotion7_Anim; aliased once a check reaches it.
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::X28);
    if t <= 0 {
        zako::settle(r, gobj);
        enter(r, gobj, 1, ANIM_UPDATE);
    } else {
        set(r, gobj, off::X28, t.wrapping_sub(1));
    }
    false
}

/// Dropped's collision, `itPatapata_UnkMotion7_Coll`: the air's.
// Ports itPatapata_UnkMotion7_Coll; aliased once a check reaches it.
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    let _ = ground::air(r, gobj);
    false
}

/// Thrown, `it_2725_Logic4_Thrown`: the enemies' throw, no stomping, its counts cleared,
/// state 6.
// Ports it_2725_Logic4_Thrown; aliased once a check reaches it.
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    lib2725::clear_stomp(r.game, gobj);
    set(r, gobj, off::X28, 0);
    set(r, gobj, off::X24, 0);
    set(r, gobj, off::X20, 0);
    change_state(r, gobj, 6, ANIM_UPDATE);
}

/// Thrown's animation, `itPatapata_UnkMotion6_Anim`: looped.
// Ports itPatapata_UnkMotion6_Anim; aliased once a check reaches it.
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 6, ANIM_UPDATE);
    }
    false
}

/// Thrown's physics, `itPatapata_UnkMotion6_Phys`: gravity.
// Ports itPatapata_UnkMotion6_Phys; aliased once a check reaches it.
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Thrown's collision, `itPatapata_UnkMotion6_Coll`: touching the stage, it ends as a KO.
// Ports itPatapata_UnkMotion6_Coll; aliased once a check reaches it.
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    if ground::air_touches(r, gobj) & 0xF != 0 {
        ko(r, gobj);
    }
    false
}

/// The Koopa knocked out of it, `it_802E11E0`: its facing kept through the knockback's
/// velocity, its body hidden and its wings shown, blinking, not stompable, its hit records
/// cleared, the Koopa spawned ([`spawn_koopa`]); the wings flutter off for `x28` frames,
/// flickering every `x2C`, drifting and pushed sideways by random steps of 0.1 from `x30`;
/// state 4.
#[doc(alias = "it_802E11E0")]
pub fn unwing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let mut v = item(r.game, gobj).x40_vel;
    let _ = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    let (b0, b18, b11) = (bone(r, gobj, 0), bone(r, gobj, 18), bone(r, gobj, 11));
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_flags_all(jobjs, b0, jobj::HIDDEN);
    jobj::clear_flags_all(jobjs, b18, jobj::HIDDEN);
    jobj::clear_flags_all(jobjs, b11, jobj::HIDDEN);
    hitbox::start_blink(r.game, gobj);
    lib2725::clear_stomp(r.game, gobj);
    hitbox::clear_hit_ids(r.game, gobj);
    spawn_koopa(r, gobj);
    let x28 = attr_f32(r, gobj, 0x28);
    set(r, gobj, off::X20, fp::fctiwz(reg(x28)));
    let x2c = attr_i32(r, gobj, 0x2C);
    set(r, gobj, off::X28, x2c);
    set(r, gobj, off::X44, 0);
    let (x30, x34) = (reg(attr_f32(r, gobj, 0x30)), reg(attr_f32(r, gobj, 0x34)));
    let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
    let range = fp::fctiwz(fp::fadds(abs(x30), abs(x34)));
    let step = |r: &mut Running| {
        let mut n = r.game.objs.rng.below(range);
        let mut val = x30;
        while n != 0 {
            val = fp::fadds(val, reg(0.1_f32));
            n = n.wrapping_sub(1);
        }
        val
    };
    let drift = step(r);
    set_f32(r, gobj, off::X2C, mem(drift));
    let push = step(r);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), push));
    change_state(r, gobj, 4, ANIM_UPDATE);
}

/// The wings' animation, `itPatapata_UnkMotion4_Anim`: every `x2C` frames they're hidden or
/// shown in turn.
#[doc(alias = "itPatapata_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::X28);
    if t == 0 {
        let x2c = attr_i32(r, gobj, 0x2C);
        set(r, gobj, off::X28, x2c);
        let toggle = get(r, gobj, off::X44) ^ 1;
        set(r, gobj, off::X44, toggle);
        let (b18, b11) = (bone(r, gobj, 18), bone(r, gobj, 11));
        let jobjs = &mut r.game.objs.jobjs;
        if toggle != 0 {
            jobj::set_flags_all(jobjs, b18, jobj::HIDDEN);
            jobj::set_flags_all(jobjs, b11, jobj::HIDDEN);
        } else {
            jobj::clear_flags_all(jobjs, b18, jobj::HIDDEN);
            jobj::clear_flags_all(jobjs, b11, jobj::HIDDEN);
        }
    } else {
        set(r, gobj, off::X28, t.wrapping_sub(1));
    }
    false
}

/// The wings' physics, `itPatapata_UnkMotion4_Phys`: bone 18 drifts by `x2C` in z, and they
/// fall at `x38` up to `x3C`.
#[doc(alias = "itPatapata_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let b18 = bone(r, gobj, 18).expect("a Koopa Paratroopa without its wing bone");
    let drift = reg(get_f32(r, gobj, off::X2C));
    jobj::set_srt(&mut r.game.objs.jobjs, b18, |o| {
        o.translate.z = mem(fp::fadds(reg(o.translate.z), drift));
    });
    let (x38, x3c) = (attr_f32(r, gobj, 0x38), attr_f32(r, gobj, 0x3C));
    lib2725::fall(r.game, gobj, x38, x3c);
}

/// The wings' collision, `itPatapata_UnkMotion4_Coll`: after `x20` frames they go
/// (`xDCC_flag.b3` set).
#[doc(alias = "itPatapata_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    let n = get(r, gobj, off::X20);
    if n == 0 {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
        return true;
    }
    set(r, gobj, off::X20, n.wrapping_sub(1));
    false
}

/// The Koopa knocked out of it, `it_802E15B0`: a small quake; a Koopa Troopa of its kind where
/// its model is, moving as it moved, facing its way, with its knockback and the spawner slot,
/// knocked back as hit ([`super::nokonoko::knock_back_kept`]).
#[doc(alias = "it_802E15B0")]
pub fn spawn_koopa(r: &mut Running, gobj: GObjId) {
    let j = super::super::jobj_of(r.game, gobj);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    let at = item(r.game, gobj).pos;
    camera::request_quake(r, QUAKE_SMALL, Some(at));
    let kind = get(r, gobj, off::KIND);
    let ip = item(r.game, gobj);
    let (vel, facing) = (ip.x40_vel, fp::fctiwz(reg(ip.facing_dir)));
    let koopa = super::nokonoko::spawn(r, kind, pos, Some(vel), facing)
        .expect("it_802E15B0: no Koopa Troopa spawned");
    let ip = item(r.game, gobj);
    let (kb, angle, dir) = (ip.xCC8_knockback, ip.xCAC_angle, ip.xCCC_incDamageDirection);
    let slot = get(r, gobj, off::SLOT);
    let v = zako::bytes(r.game, koopa);
    v.set_f32(super::nokonoko::off::KB, kb);
    v.set_i32(super::nokonoko::off::KB_ANGLE, angle);
    v.set_f32(super::nokonoko::off::KB_DIR, dir);
    v.set_i32(super::nokonoko::off::SLOT, slot);
    set(r, gobj, off::SLOT, -1);
    super::nokonoko::knock_back_kept(r, koopa);
}

/// Enters a state it can be landed on in, `it_802E1648`: stompable, its jumped-on callback
/// [`stomped`].
#[doc(alias = "it_802E1648")]
pub fn enter(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    change_state(r, gobj, msid, flags);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(STOMPED));
}

/// An enemy KO, `it_802E1694`: an effect and a sound, the KO, and gone.
// Ports it_802E1694; aliased once a check reaches it.
pub fn ko(r: &mut Running, gobj: GObjId) {
    lib2725::destroy_effect(r, gobj, 0x444, 0x130);
    zako::ko(r, gobj);
    super::super::item::destroy(r, gobj);
}

/// Forgets a GObj going away, `it_802E16D8` (`it_8026B894`).
#[doc(alias = "it_802E16D8")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = lib26b1::forget(r.game, gobj, other);
}

/// Spawns a Koopa Paratroopa, `it_802E16F8`: at `pos` of kind `kind` facing `facing` (its
/// model and collision that way); kinds under 2 posed in state 0 first; settled, state 1.
#[doc(alias = "it_802E16F8")]
pub fn spawn(r: &mut Running, kind: i32, pos: Vec3, facing: i32) -> Option<GObjId> {
    let gobj = zako::spawn(
        r,
        super::PATAPATA,
        Some(pos),
        None,
        Some(Vec3::new(0.0, 0.0, 0.0)),
        true,
    )?;
    let f = mem(fp::frsp(f64::from(facing)));
    item_mut(r.game, gobj).facing_dir = f;
    zako::face(r, gobj, f);
    set_coll_facing(r, gobj);
    set(r, gobj, off::KIND, kind);
    if kind < 2 {
        change_state(r, gobj, 0, ANIM_UPDATE);
        let j = super::super::jobj_of(r.game, gobj);
        jobj::anim_all(
            &mut r.game.objs,
            r.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
    }
    zako::settle(r, gobj);
    enter(r, gobj, 1, ANIM_UPDATE);
    Some(gobj)
}
