//! The Koopas' shells, `melee/it/kinds/itzgshell.c` (`It_Kind_ZGShell`) and the red one's
//! wrappers, `itzrshell.c` (`It_Kind_ZRShell`): what a Koopa Troopa leaves when it's knocked out
//! of its shell. Much like the VS green shell ([`super::gshell`]): kicked, jumped on or hit, it
//! slides along the ground hitting what it meets until it slows. Left still long enough (the
//! attributes' `x2C` frames, state 0 or held), it wakes (state 11): it turns to face a side,
//! shakes, and a Koopa Troopa of its kind (`xE10`) climbs back out, taking over its spawner slot.
//!
//! Both shells share the states (`it_803F86C8`, `it_803F87F0` with the red one's extra state 12,
//! its spawn's pose). Their variables are the union's bytes ([`zako::bytes`]): the enemies'
//! common words, then their own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::ground::{
    air_touches_walls, fall_to_floor, fall_unbreakable, ground_or_fall, ground_skid, snap_down,
};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{coll, data, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::ef::{Arg, SpawnKind};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const TRAIL: u32 = 0x802D_DB38;
pub const GRACE: u32 = 0x802D_DBE8;
pub const LANDED: u32 = 0x802D_DC8C;
pub const KNOCKED: u32 = 0x802D_DD38;
pub const STOMPED: u32 = 0x802D_DEB4;
pub const SPAWNED: u32 = 0x802D_E040;
pub const STOPPED: u32 = 0x802D_E0F0;
pub const ANIM0: u32 = 0x802D_E190;
pub const PHYS0: u32 = 0x802D_E198;
pub const COLL0: u32 = 0x802D_E1FC;
pub const FELL: u32 = 0x802D_E320;
pub const ANIM1: u32 = 0x802D_E35C;
pub const PHYS1: u32 = 0x802D_E3D8;
pub const COLL1: u32 = 0x802D_E408;
pub const PICKED_UP: u32 = 0x802D_E434;
pub const ANIM2: u32 = 0x802D_E490;
pub const PHYS2: u32 = 0x802D_E498;
pub const THROWN: u32 = 0x802D_E4FC;
pub const ANIM3: u32 = 0x802D_E544;
pub const PHYS3: u32 = 0x802D_E5EC;
pub const COLL3: u32 = 0x802D_E61C;
pub const DROPPED: u32 = 0x802D_E648;
pub const ANIM4: u32 = 0x802D_E670;
pub const PHYS4: u32 = 0x802D_E694;
pub const COLL4: u32 = 0x802D_E6C4;
pub const SLIDE: u32 = 0x802D_E6F0;
pub const ANIM6: u32 = 0x802D_E884;
pub const PHYS6: u32 = 0x802D_EA0C;
pub const COLL6: u32 = 0x802D_EB44;
pub const SLIDE_AIR: u32 = 0x802D_EC80;
pub const ANIM8: u32 = 0x802D_EE04;
pub const PHYS8: u32 = 0x802D_EE98;
pub const COLL8: u32 = 0x802D_EEC8;
pub const ENTERED_AIR: u32 = 0x802D_EEF4;
pub const ANIM9: u32 = 0x802D_EF78;
pub const PHYS9: u32 = 0x802D_EF80;
pub const COLL9: u32 = 0x802D_F104;
pub const WAKE: u32 = 0x802D_F230;
pub const ANIM11: u32 = 0x802D_F724;
pub const PHYS11: u32 = 0x802D_F93C;
pub const COLL11: u32 = 0x802D_F970;
pub const BOUNCED: u32 = 0x802D_F9F8;
pub const ANIM10: u32 = 0x802D_FA68;
pub const PHYS10: u32 = 0x802D_FB90;
pub const COLL10: u32 = 0x802D_FBAC;
pub const DMG_DEALT: u32 = 0x802D_FC34;
pub const DMG_RECEIVED: u32 = 0x802D_FC58;
pub const REFLECTED: u32 = 0x802D_FC90;
pub const CLANKED: u32 = 0x802D_FCB0;
pub const HIT_SHIELD: u32 = 0x802D_FD60;
pub const SHIELD_BOUNCED: u32 = 0x802D_FE40;
pub const JUMPED_ON: u32 = 0x802D_FE7C;
pub const DESTROYED: u32 = 0x802D_FEF4;

/// The red shell's wrappers (`itzrshell.c`), each calling the green shell's.
pub mod red {
    pub const SPAWNED: u32 = 0x802E_02E8;
    pub const PICKED_UP: u32 = 0x802E_0308;
    pub const THROWN: u32 = 0x802E_0328;
    pub const DROPPED: u32 = 0x802E_0348;
    pub const ENTERED_AIR: u32 = 0x802E_0368;
    pub const DESTROYED: u32 = 0x802E_0388;
    pub const DMG_DEALT: u32 = 0x802E_03A8;
    pub const DMG_RECEIVED: u32 = 0x802E_03C8;
    pub const REFLECTED: u32 = 0x802E_03E8;
    pub const CLANKED: u32 = 0x802E_0408;
    pub const HIT_SHIELD: u32 = 0x802E_0428;
    pub const SHIELD_BOUNCED: u32 = 0x802E_0448;
}

/// The shells' variables' offsets in the union (`itGShell_ItemVars` as `itzgshell.c` uses it).
pub mod off {
    /// `xDD8`: the common spawner slot, which the Koopa that climbs out takes over.
    pub const SLOT: usize = 4;
    /// `xDF4`: frames until a sliding shell's hits come back.
    pub const HIT_DELAY: usize = 0x20;
    /// `xDF8`: frames until the next dust trail.
    pub const TRAIL: usize = 0x24;
    /// `xDFC`: frames until a hit shell's hitbox hits again.
    pub const GRACE: usize = 0x28;
    /// `xE00`: the waking turn's step, in degrees a frame.
    pub const TURN: usize = 0x2C;
    /// `xE04`: frames until a popped-up shell is stompable again.
    pub const STOMP_DELAY: usize = 0x30;
    /// `xE08`: `b0` counting the grace down, `b1` ending the blink with it.
    pub const FLAGS: usize = 0x34;
    /// `xE0C`: frames still (states 0 and 2), then the waking turn's frames left.
    pub const TIMER: usize = 0x38;
    /// `xE10`: the shell kind of the Koopa that climbs out.
    pub const KOOPA: usize = 0x3C;
    /// `xE14`: frames before a waking shell falls.
    pub const WAKE: usize = 0x40;
    /// `xE18`: whether it woke in a holder's hands.
    pub const HELD: usize = 0x44;
    /// `xE1C`: `b0`, popped up (stompable after `xE04` frames).
    pub const POPPED: usize = 0x48;
    /// `vel`: the moving bone's last translation (`itUpdateVelocityFromBone`), and state 9's last
    /// position.
    pub const BONE: usize = 0x4C;
    /// `x58_hurtInit`: its hurtbox as it spawned (`itGShell_HurtInit`); the state word (`+0`) is
    /// never written, read only by the copy that passes the capsule on.
    pub const HURT: usize = 0x58;
}

const B0: u8 = 0x80;
const B1: u8 = 0x40;

/// `GA_Ground`, `GA_Air`.
const GA_GROUND: i32 = 0;
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`.
const UNK_0X1: u32 = 1;

/// Runs one of the shells' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 | ANIM9 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL3 | COLL4 | COLL8 => coll3(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM11 => anim11(r, gobj),
        COLL11 | COLL10 => coll_waking(r, gobj),
        ANIM10 => anim10(r, gobj),
        DMG_DEALT | red::DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED | red::DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED | red::REFLECTED => reflected(r, gobj),
        CLANKED | red::CLANKED => clanked(r, gobj),
        HIT_SHIELD | red::HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED | red::SHIELD_BOUNCED => shield_bounced(r, gobj),
        JUMPED_ON => jumped_on(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shells' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        TRAIL => trail(r, gobj),
        GRACE => count_grace(r, gobj),
        LANDED => landed(r, gobj),
        KNOCKED => knocked(r, gobj),
        STOMPED => stomped(r, gobj),
        SPAWNED | red::SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS2 => count_still(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 | PHYS4 | PHYS8 => phys_fall(r, gobj),
        PICKED_UP | red::PICKED_UP => picked_up(r, gobj),
        THROWN | red::THROWN => thrown(r, gobj),
        DROPPED | red::DROPPED => dropped(r, gobj),
        SLIDE => slide(r, gobj),
        PHYS6 => phys6(r, gobj),
        SLIDE_AIR => slide_air(r, gobj),
        ENTERED_AIR | red::ENTERED_AIR => entered_air(r, gobj),
        PHYS9 => phys9(r, gobj),
        WAKE => wake(r, gobj),
        PHYS11 => phys11(r, gobj),
        BOUNCED => bounced(r, gobj),
        PHYS10 => phys10(r, gobj),
        DESTROYED | red::DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// `itZGShell_Attrs`' float at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Koopa's shell without attributes");
    r.data.f32(a.at(off))
}

fn get_f32(r: &mut Running, gobj: GObjId, off: usize) -> f32 {
    zako::bytes(r.game, gobj).f32(off)
}

fn set_f32(r: &mut Running, gobj: GObjId, off: usize, v: f32) {
    zako::bytes(r.game, gobj).set_f32(off, v);
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

pub(super) fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

fn bit(r: &mut Running, gobj: GObjId, off: usize, mask: u8) -> bool {
    zako::bytes(r.game, gobj).bit(off, mask)
}

fn set_bit(r: &mut Running, gobj: GObjId, off: usize, mask: u8, on: bool) {
    zako::bytes(r.game, gobj).set_bit(off, mask, on);
}

/// `ABS(x)` as the original's ternary computes it.
fn abs(x: f32) -> f64 {
    let x = reg(x);
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// Stops (the velocity's z, then y, then x).
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Starts a dust trail every `x38` frames, `it_802DDB38` (`it_8028B8D8`'s copy).
#[doc(alias = "it_802DDB38")]
pub fn trail(r: &mut Running, gobj: GObjId) {
    if !(reg(get_f32(r, gobj, off::TRAIL)) <= 0.0) {
        let t = get_f32(r, gobj, off::TRAIL);
        set_f32(r, gobj, off::TRAIL, mem(fp::fsubs(reg(t), 1.0)));
        return;
    }
    let f = |r: &Running, off: u32| attr(r, gobj, off);
    let back = fp::fneg(reg(item(r.game, gobj).facing_dir));
    let v = Vec3::new(
        mem(fp::fmuls(reg(f(r, 0x3C)), back)),
        f(r, 0x40),
        f(r, 0x44),
    );
    let j = jobj_of(r.game, gobj);
    super::super::effect::queue(r, gobj, SpawnKind::PosOffset, 1029, j, &[Arg::Vec(v)]);
    let x = f(r, 0x38);
    set_f32(r, gobj, off::TRAIL, x);
}

/// Counts down how long a hit shell's hitbox can't hit, `it_802DDBE8`: done, it hits again,
/// and its blink ends if the hit started one.
#[doc(alias = "it_802DDBE8")]
pub fn count_grace(r: &mut Running, gobj: GObjId) {
    let x28 = attr(r, gobj, 0x28);
    if !bit(r, gobj, off::FLAGS, B0) {
        return;
    }
    let g = mem(fp::fsubs(reg(get_f32(r, gobj, off::GRACE)), 1.0));
    set_f32(r, gobj, off::GRACE, g);
    if reg(g) <= 0.0 {
        set_bit(r, gobj, off::FLAGS, B0, false);
        set_f32(r, gobj, off::GRACE, x28);
        hitbox::set_hit_flag0(r.game, gobj, 0, true);
        if bit(r, gobj, off::FLAGS, B1) {
            hitbox::end_blink(r.game, gobj);
        }
    }
}

/// Lands from a throw or a drop, `fn_802DDC8C`: its box back; slow, it stops (or slides off a
/// slope), else it slides on.
#[doc(alias = "fn_802DDC8C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let b = item(r.game, gobj).xC0C.clone();
    maplib::set_box(r.game, gobj, b);
    let x8 = attr(r, gobj, 8);
    if abs(item(r.game, gobj).x40_vel.x) < reg(x8) {
        stop(r, gobj);
        if !maplib::slides(r.game, r.data, gobj) {
            stopped(r, gobj);
        } else {
            entered_air(r, gobj);
        }
    } else {
        slide(r, gobj);
    }
}

/// `(f32) i`, as the original converts it.
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// Hit, `it_802DDD38`: sent sliding the way the hit pushed it (still or thrown: at the push;
/// sliding: the push added); a waking shell is knocked into the air instead.
#[doc(alias = "it_802DDD38")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    let x14 = attr(r, gobj, 0x14);
    let ip = item_mut(r.game, gobj);
    let push = fp::fmuls(to_f32(ip.xCA0), reg(x14));
    let away = fp::fneg(reg(ip.xCCC_incDamageDirection));
    match ip.msid {
        0 | 1 | 3 | 4 | 9 | 10 => {
            ip.x40_vel.x = mem(fp::fmuls(away, push));
            set_bit(r, gobj, off::FLAGS, B1, true);
        }
        5..=8 => {
            ip.x40_vel.x = mem(fp::fmadds(away, push, reg(ip.x40_vel.x)));
            set_bit(r, gobj, off::FLAGS, B1, true);
        }
        _ => {}
    }
    if item(r.game, gobj).msid == 0xB {
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        bounced(r, gobj);
        return;
    }
    coll::take_attacker(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    if item(r.game, gobj).ground_or_air == GA_AIR {
        slide_air(r, gobj);
    } else {
        slide(r, gobj);
    }
    set_bit(r, gobj, off::FLAGS, B0, true);
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// Pops up off whoever met a sliding shell (the clank's body, `it_2725_Logic11_Clanked`):
/// blinking, its hits back, bounced back at a random part of its speed and up at `x10`, in the
/// air, stompable after `x34` frames, falling.
fn pop_up(r: &mut Running, gobj: GObjId) {
    let (xc, x10, x34) = (attr(r, gobj, 0xC), attr(r, gobj, 0x10), attr(r, gobj, 0x34));
    hitbox::start_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let f = r.game.objs.rng.draw_f32();
    let ip = item_mut(r.game, gobj);
    let vx = fp::fmuls(fp::fneg(reg(ip.x40_vel.x)), reg(xc));
    ip.x40_vel.x = mem(fp::fmuls(vx, reg(f)));
    ip.x40_vel.y = x10;
    maplib::enter_air(ip);
    set_bit(r, gobj, off::POPPED, B0, true);
    set_f32(r, gobj, off::STOMP_DELAY, x34);
}

/// Jumped on, `it_802DDEB4`: a sliding (or waking-knocked) shell pops up, a still one is kicked
/// away from the jumper; a waking one is knocked into the air instead.
#[doc(alias = "it_802DDEB4")]
pub fn stomped(r: &mut Running, gobj: GObjId) {
    match item(r.game, gobj).msid {
        5..=8 | 10 => {
            pop_up(r, gobj);
            fell(r, gobj);
            set_bit(r, gobj, off::FLAGS, B1, true);
        }
        0 | 1 | 3 | 4 | 9 => {
            let x1c = attr(r, gobj, 0x1C);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(reg(x1c), fp::fneg(reg(ip.xCD0))));
            coll::take_stomper(r.game, gobj);
            set_bit(r, gobj, off::FLAGS, B1, true);
            if item(r.game, gobj).ground_or_air == GA_AIR {
                slide_air(r, gobj);
            } else {
                slide(r, gobj);
            }
        }
        _ => {}
    }
    if item(r.game, gobj).msid == 0xB {
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        bounced(r, gobj);
        return;
    }
    hitbox::start_blink(r.game, gobj);
    set_bit(r, gobj, off::FLAGS, B0, true);
    hitbox::set_hit_flag0(r.game, gobj, 0, false);
}

/// The shell's setup, `it_802DE040` (the red one's `it_802E02E8`): the enemies' common
/// variables, `xDC8`'s bit `x1C`, its counters, its hurtbox kept, falling.
#[doc(alias = "it_802DE040")]
#[doc(alias = "it_802E02E8")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let (x28, x34) = (attr(r, gobj, 0x28), attr(r, gobj, 0x34));
    zako::reset(r.game, gobj);
    zako::set_x1c(r.game, gobj);
    set_bit(r, gobj, off::FLAGS, B0, false);
    set_f32(r, gobj, off::GRACE, x28);
    set_bit(r, gobj, off::FLAGS, B1, true);
    set(r, gobj, off::KOOPA, 0);
    set(r, gobj, off::WAKE, 0);
    set(r, gobj, off::TIMER, 0);
    set_bit(r, gobj, off::POPPED, B0, false);
    set_f32(r, gobj, off::STOMP_DELAY, x34);
    let (a, b, scale) = coll::hurt_capsule(r.game, r.data, gobj, 0);
    let v = zako::bytes(r.game, gobj);
    v.set_vec(off::HURT + 4, a);
    v.set_vec(off::HURT + 0x10, b);
    v.set_f32(off::HURT + 0x1C, scale);
    fell(r, gobj);
}

/// Comes to rest (state 0), `it_802DE0F0`: pickable, its hits back, no blink, still, its still
/// count from 0, stompable.
#[doc(alias = "it_802DE0F0")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    rest(r, gobj, change::ANIM_UPDATE, true);
}

/// Comes to rest in state 0 with `flags` (the body of `it_802DE0F0` and its inline copies; the
/// copies in the slides' physics leave the second hit records alone, `ids2` false).
fn rest(r: &mut Running, gobj: GObjId, flags: u32, ids2: bool) {
    lib26b1::enable_pickup(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    if ids2 {
        hitbox::set_hit_ids2(r.game, gobj);
    }
    hitbox::end_blink(r.game, gobj);
    stop(r, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 0, flags);
    set_bit(r, gobj, off::POPPED, B0, false);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// The states that answer false, `itZrshell_UnkMotion0_Anim` (2, 9).
#[doc(alias = "itZrshell_UnkMotion0_Anim")]
#[doc(alias = "itZrshell_UnkMotion2_Anim")]
// Ports itZrshell_UnkMotion9_Anim; aliased once a check reaches it.
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Still or held, `itZrshell_UnkMotion0_Phys` (and 2): past `x2C` frames it wakes.
#[doc(alias = "itZrshell_UnkMotion0_Phys")]
#[doc(alias = "itZrshell_UnkMotion2_Phys")]
pub fn count_still(r: &mut Running, gobj: GObjId) {
    let counter = get(r, gobj, off::TIMER);
    let x2c = attr(r, gobj, 0x2C);
    if to_f32(counter) > reg(x2c) {
        wake(r, gobj);
    } else {
        set(r, gobj, off::TIMER, counter.wrapping_add(1));
    }
}

/// Rolls the model with the speed on the ground (the inline `Item_UpdateRollingShellRotation`,
/// or its spelled-out copy in state 9's collision).
fn roll(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air != GA_GROUND {
        return;
    }
    let rate = attr(r, gobj, 0x20);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let turn = fp::fmuls(reg(rate), abs(item(r.game, gobj).x40_vel.x));
    let j = child.expect("a shell without a model child");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), turn));
    });
}

/// Still's collision, `itZrshell_UnkMotion0_Coll`: off the floor it falls; on it, it rolls.
#[doc(alias = "itZrshell_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    roll(r, gobj);
    false
}

/// Falls (state 1), not stompable, `it_802DE320`.
#[doc(alias = "it_802DE320")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib2725::clear_stomp(r.game, gobj);
}

/// Falling's animation, `itZrshell_UnkMotion1_Anim`: a popped-up shell turns stompable again
/// after `xE04` frames.
#[doc(alias = "itZrshell_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let stompable = super::super::flag(item(r.game, gobj).xDD0_flag, 0);
    if bit(r, gobj, off::POPPED, B0) && !stompable {
        let d = get_f32(r, gobj, off::STOMP_DELAY);
        if reg(d) <= 0.0 {
            lib2725::make_stompable(r.game, gobj);
            item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
        } else {
            set_f32(r, gobj, off::STOMP_DELAY, mem(fp::fsubs(reg(d), 1.0)));
        }
    }
    false
}

/// Gravity from the attributes, `itZrshell_UnkMotion1_Phys` (3, 4, 8).
#[doc(alias = "itZrshell_UnkMotion1_Phys")]
#[doc(alias = "itZrshell_UnkMotion3_Phys")]
// Ports itZrshell_UnkMotion4_Phys; aliased once a check reaches it.
#[doc(alias = "itZrshell_UnkMotion8_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itZrshell_UnkMotion1_Coll`: landing, it comes to rest.
#[doc(alias = "itZrshell_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    let _ = fall_unbreakable(r, gobj, stopped);
    false
}

/// Picked up, `it_2725_Logic11_PickedUp` (the red one's `itZRShell_Logic12_PickedUp`): its hit
/// records cleared, its still count from 0, state 2.
#[doc(alias = "it_2725_Logic11_PickedUp")]
#[doc(alias = "itZRShell_Logic12_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hitbox::clear_hit_ids(r.game, gobj);
    set(r, gobj, off::TIMER, 0);
    set_bit(r, gobj, off::POPPED, B0, false);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Thrown, `itZGShell_Logic11_Thrown` (the red one's `itZRShell_Logic12_Thrown`).
#[doc(alias = "itZGShell_Logic11_Thrown")]
#[doc(alias = "itZRShell_Logic12_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    play_sfx(r, gobj, 0xF2, 127, 64);
}

/// Thrown's animation, `itZrshell_UnkMotion3_Anim`.
#[doc(alias = "itZrshell_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    count_grace(r, gobj);
    false
}

/// Dropped's animation, `itZrshell_UnkMotion4_Anim` (thrown's).
// Ports itZrshell_UnkMotion4_Anim; aliased once a check reaches it.
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    anim3(r, gobj)
}

/// Thrown's, dropped's and the air slide's collision (`itZrshell_UnkMotion3_Coll`, 4, 8):
/// landing, [`landed`].
#[doc(alias = "itZrshell_UnkMotion3_Coll")]
// Ports itZrshell_UnkMotion4_Coll; aliased once a check reaches it.
#[doc(alias = "itZrshell_UnkMotion8_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    false
}

/// Dropped, `itZGShell_Logic11_Dropped` (the red one's `itZRShell_Logic12_Dropped`).
// Ports itZGShell_Logic11_Dropped; aliased once a check reaches it.
// Ports itZRShell_Logic12_Dropped; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Starts sliding (`it_802DE6F0` on the ground, `it_802DEC80` in the air): not pickable, hit
/// records cleared, its own player's items let hit it, the common lifetime, its hit delay and
/// trail timers, its speed capped at `x4` and facing its way, its bone forgotten (on the
/// ground), then `slow` when under `x8` (pickable) or `fast`; stompable.
fn start_slide(r: &mut Running, gobj: GObjId, fast: i32, slow: i32, ground: bool) {
    let (x4, x8, x24, x38) = (
        attr(r, gobj, 4),
        attr(r, gobj, 8),
        attr(r, gobj, 0x24),
        attr(r, gobj, 0x38),
    );
    lib26b1::disable_pickup(r.game, gobj);
    hitbox::clear_hit_ids(r.game, gobj);
    hitbox::let_own_items_hit(r.game, gobj);
    let life = common_data(r.game).u32(r.data, data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    item_mut(r.game, gobj).xD5C = 0;
    set_f32(r, gobj, off::HIT_DELAY, x24);
    set_f32(r, gobj, off::TRAIL, x38);
    set_bit(r, gobj, off::POPPED, B0, false);
    let ip = item_mut(r.game, gobj);
    if abs(ip.x40_vel.x) > reg(x4) {
        ip.x40_vel.x = if reg(ip.x40_vel.x) < 0.0 { -1.0 } else { 1.0 };
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(x4)));
    }
    lib2725::face_motion(r.game, gobj);
    set(r, gobj, off::TIMER, 0);
    if ground {
        forget_bone(r, gobj);
    }
    if abs(item(r.game, gobj).x40_vel.x) < reg(x8) {
        change_state(r, gobj, slow, change::ANIM_UPDATE);
        lib26b1::enable_pickup(r.game, gobj);
        if ground {
            hitbox::set_hit_ids2(r.game, gobj);
        }
    } else {
        change_state(r, gobj, fast, change::ANIM_UPDATE);
    }
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// Slides on the ground, `it_802DE6F0`.
#[doc(alias = "it_802DE6F0")]
pub fn slide(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 5, 6, true);
}

/// Sliding's animation, `itZrshell_UnkMotion6_Anim` (5 to 8): its hits come back after
/// `xDF4` frames, its grace counts down, and on the ground it trails dust.
#[doc(alias = "itZrshell_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    let d = get_f32(r, gobj, off::HIT_DELAY);
    if reg(d) <= 0.0 {
        if !item(r.game, gobj).xDCD_flag.x0.b5 {
            hitbox::set_hit_ids(r.game, gobj);
        }
    } else {
        set_f32(r, gobj, off::HIT_DELAY, mem(fp::fsubs(reg(d), 1.0)));
    }
    count_grace(r, gobj);
    if matches!(item(r.game, gobj).msid, 5 | 6) {
        trail(r, gobj);
    }
    false
}

/// Sliding's physics, `itZrshell_UnkMotion6_Phys`: under `x8` (and not held) it can be picked
/// up and hits nothing; under `x30` it stops (or slides off a slope).
#[doc(alias = "itZrshell_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let (x8, x30) = (attr(r, gobj, 8), attr(r, gobj, 0x30));
    let ip = item(r.game, gobj);
    if abs(ip.x40_vel.x) < reg(x8) && !ip.xDC8_word.flags.x15 {
        lib26b1::enable_pickup(r.game, gobj);
        lib2725::clear_hitboxes(r.game, r.data, gobj);
        hitbox::set_hit_ids2(r.game, gobj);
    }
    if abs(item(r.game, gobj).x40_vel.x) < reg(x30) {
        stop(r, gobj);
        if !maplib::slides(r.game, r.data, gobj) {
            rest(r, gobj, UNK_0X1, false);
        } else {
            entered_air(r, gobj);
        }
    }
}

/// Sliding's collision, `itZrshell_UnkMotion6_Coll`: off the floor it slides through the air;
/// it rolls, and turns around off walls.
#[doc(alias = "itZrshell_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, slide_air);
    roll(r, gobj);
    if maplib::bounce_off_wall(r.game, r.data, gobj) {
        lib2725::face_motion(r.game, gobj);
    }
    false
}

/// Slides through the air, `it_802DEC80`.
#[doc(alias = "it_802DEC80")]
pub fn slide_air(r: &mut Running, gobj: GObjId) {
    start_slide(r, gobj, 7, 8, false);
}

/// The air slide's animation, `itZrshell_UnkMotion8_Anim` (sliding's).
#[doc(alias = "itZrshell_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    anim6(r, gobj)
}

/// Slides off a slope (state 9), `it_2725_Logic11_EnteredAir` (the red one's
/// `itZRShell_Logic12_EnteredAir`); not while waking (10, 11).
// Ports it_2725_Logic11_EnteredAir; aliased once a check reaches it.
// Ports itZRShell_Logic12_EnteredAir; aliased once a check reaches it.
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    if msid == 0xB || msid == 0xA {
        return;
    }
    forget_bone(r, gobj);
    set_bit(r, gobj, off::POPPED, B0, false);
    item_mut(r.game, gobj).xDC8_word.flags.x1F = true;
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// Sliding off a slope's physics, `itZrshell_UnkMotion9_Phys`: under `x30`, or not moved since
/// last frame, it comes to rest; its position kept.
// Ports itZrshell_UnkMotion9_Phys; aliased once a check reaches it.
pub fn phys9(r: &mut Running, gobj: GObjId) {
    let x30 = attr(r, gobj, 0x30);
    if abs(item(r.game, gobj).x40_vel.x) < reg(x30) {
        stop(r, gobj);
        rest(r, gobj, UNK_0X1, false);
    }
    let last = zako::bytes(r.game, gobj).vec(off::BONE);
    let pos = item(r.game, gobj).pos;
    if pos.x == last.x && pos.y == last.y {
        stop(r, gobj);
        rest(r, gobj, UNK_0X1, false);
    }
    let pos = item(r.game, gobj).pos;
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::BONE, pos.x);
    v.set_f32(off::BONE + 4, pos.y);
}

/// Sliding off a slope's collision, `itZrshell_UnkMotion9_Coll`: skids to rest or falls; on the
/// ground it rolls.
// Ports itZrshell_UnkMotion9_Coll; aliased once a check reaches it.
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, stopped, fell);
    roll(r, gobj);
    false
}

/// Wakes (state 11), `it_802DF230`: no damage, no blink, not pickable, still, its hurtbox
/// raised and doubled, 20 frames to turn to face a side (from its model's angle, a random
/// side when square to the screen), let go if held (falling), 35 frames before it falls.
#[doc(alias = "it_802DF230")]
pub fn wake(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xC9C = 0;
    ip.xCA0 = 0;
    hitbox::end_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x1F = false;
    stop(r, gobj);
    set_hurt(r, gobj, true);
    set(r, gobj, off::TIMER, 20);
    let j = jobj_of(r.game, gobj).expect("a shell without a model");
    let child = r.game.objs.jobjs[j]
        .child
        .expect("a shell without a model child");
    let deg = 57.295_78_f32;
    let angle = fp::fmuls(reg(deg), reg(r.game.objs.jobjs[j].rotate.y));
    let angle = fp::fmadds(reg(deg), reg(r.game.objs.jobjs[child].rotate.y), angle);
    let rad = reg(0.017_453_292_f32);
    if fp::fneg(90.0) == angle || 90.0 == angle {
        item_mut(r.game, gobj).facing_dir = if angle < 0.0 { -1.0 } else { 1.0 };
        set_f32(r, gobj, off::TURN, 0.0);
        let jobjs = &mut r.game.objs.jobjs;
        jobj::set_rotation_y(jobjs, j, mem(fp::fmuls(rad, angle)));
        jobj::set_rotation_y(jobjs, child, 0.0);
    } else {
        assert!(
            !(angle > 2_147_483_648.0 || angle < fp::fneg(2_147_483_648.0)),
            "the original's assertion failed: *** ZGShell Restoration Rot Y Irregul!"
        );
        let angle = fp::frsp(f64::from(fp::fctiwz(angle).wrapping_rem(360)));
        let jobjs = &mut r.game.objs.jobjs;
        jobj::set_rotation_y(jobjs, j, mem(fp::fmuls(rad, angle)));
        jobj::set_rotation_y(jobjs, child, 0.0);
        let factor = if 0.0 == angle {
            let f: f32 = if r.game.objs.rng.below(2) != 0 {
                -1.0
            } else {
                1.0
            };
            item_mut(r.game, gobj).facing_dir = f;
            fp::fmuls(90.0, reg(f))
        } else if angle < 180.0 {
            item_mut(r.game, gobj).facing_dir = 1.0;
            fp::fsubs(90.0, angle)
        } else {
            item_mut(r.game, gobj).facing_dir = -1.0;
            fp::fsubs(270.0, angle)
        };
        let n = get(r, gobj, off::TIMER);
        set_f32(r, gobj, off::TURN, mem(fp::fdivs(factor, to_f32(n))));
    }
    if item(r.game, gobj).xDC8_word.flags.x13 {
        let vel = item(r.game, gobj).x40_vel;
        super::super::release::let_go(r, gobj, vel);
        let max = attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED_MAX);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(max)));
        lib26b1::disable_pickup(r.game, gobj);
        set(r, gobj, off::HELD, 1);
        maplib::enter_air(item_mut(r.game, gobj));
    } else {
        set(r, gobj, off::HELD, 0);
    }
    set(r, gobj, off::WAKE, 0x23);
    forget_bone(r, gobj);
    change_state(r, gobj, 0xB, change::ANIM_UPDATE);
}

/// Waking's animation, `itZrshell_UnkMotion11_Anim`: it turns by `xE00` degrees a frame for its
/// turn's frames; while it animates its bone 1 moves it (after `xE14` frames, in the air with
/// gravity capped at its last speed, on the ground along the floor); at the end on the ground, a
/// Koopa Troopa of its kind where its model is, facing its way, taking over its spawner slot,
/// and the shell goes.
#[doc(alias = "itZrshell_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::TIMER) != 0 {
        let j = jobj_of(r.game, gobj).expect("a shell without a model");
        let step = get_f32(r, gobj, off::TURN);
        let turn = fp::fmuls(reg(0.017_453_292_f32), reg(step));
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.rotate.y = mem(fp::fadds(reg(o.rotate.y), turn));
        });
        let t = get(r, gobj, off::TIMER);
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    if lib2725::is_animating(r.game, gobj) {
        let saved_vy = reg(item(r.game, gobj).x40_vel.y);
        let bone = bone1(r, gobj);
        follow_bone(r, bone, gobj);
        let w = get(r, gobj, off::WAKE);
        if w != 0 {
            set(r, gobj, off::WAKE, w.wrapping_sub(1));
        } else if item(r.game, gobj).ground_or_air == GA_AIR {
            let a = attr_of(item(r.game, gobj));
            let accel = reg(a.f32(r.data, data::attr::FALL_SPEED));
            let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
            let ip = item_mut(r.game, gobj);
            if reg(ip.x40_vel.y) != 0.0 {
                ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
                if reg(ip.x40_vel.y) > saved_vy {
                    ip.x40_vel.y = mem(fp::fsubs(saved_vy, accel));
                }
            } else {
                ip.x40_vel.y = max;
            }
        } else {
            stop(r, gobj);
            maplib::tilt_on_floor(r.game, r.data, gobj);
        }
    } else if item(r.game, gobj).ground_or_air == GA_GROUND {
        let j = jobj_of(r.game, gobj);
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        set_hurt(r, gobj, false);
        hitbox::start_blink(r.game, gobj);
        let (kind, facing) = (
            get(r, gobj, off::KOOPA),
            fp::fctiwz(reg(item(r.game, gobj).facing_dir)),
        );
        if let Some(koopa) = super::nokonoko::spawn(r, kind, pos, None, facing) {
            let slot = get(r, gobj, off::SLOT);
            zako::bytes(r.game, koopa).set_i32(off::SLOT, slot);
            set(r, gobj, off::SLOT, -1);
        }
        return true;
    }
    false
}

/// Waking's physics, `itZrshell_UnkMotion11_Phys`: moving up, it's in the air.
#[doc(alias = "itZrshell_UnkMotion11_Phys")]
pub fn phys11(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if reg(ip.x40_vel.y) > 0.0 {
        maplib::enter_air(ip);
    }
}

/// Waking's and its knock's collision (`itZrshell_UnkMotion11_Coll`, `_10_Coll`): in the air,
/// landing on a floor it stops on the ground; on the ground off the floor it falls.
#[doc(alias = "itZrshell_UnkMotion11_Coll")]
// Ports itZrshell_UnkMotion10_Coll; aliased once a check reaches it.
pub fn coll_waking(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        if air_touches_walls(r, gobj) & 1 != 0 {
            maplib::enter_ground(item_mut(r.game, gobj));
            stop(r, gobj);
        }
    } else {
        ground_or_fall(r, gobj, fell);
    }
    zako::answers_false(r, gobj)
}

/// A waking shell knocked into the air (state 10), `it_802DF9F8`: in the air, its hurtbox
/// back, blinking.
// Ports it_802DF9F8; aliased once a check reaches it.
pub fn bounced(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    maplib::enter_air(ip);
    ip.xDC8_word.flags.x1F = false;
    set_hurt(r, gobj, false);
    hitbox::start_blink(r.game, gobj);
    change_state(r, gobj, 0xA, change::ANIM_UPDATE);
}

/// Knocked's animation, `itZrshell_UnkMotion10_Anim`: done animating on the ground, it comes
/// to rest (or slides off a slope).
// Ports itZrshell_UnkMotion10_Anim; aliased once a check reaches it.
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) && item(r.game, gobj).ground_or_air == GA_GROUND {
        if !maplib::slides(r.game, r.data, gobj) {
            rest(r, gobj, change::ANIM_UPDATE, true);
        } else {
            entered_air(r, gobj);
        }
    }
    false
}

/// Knocked's physics, `itZrshell_UnkMotion10_Phys`: gravity, uncapped.
// Ports itZrshell_UnkMotion10_Phys; aliased once a check reaches it.
pub fn phys10(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// `itZGShell_Logic11_DmgDealt` (the red one's `itZRShell_Logic12_DmgDealt`).
#[doc(alias = "itZGShell_Logic11_DmgDealt")]
#[doc(alias = "itZRShell_Logic12_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    false
}

/// `itZGShell_Logic11_DmgReceived` (the red one's `itZRShell_Logic12_DmgReceived`).
#[doc(alias = "itZGShell_Logic11_DmgReceived")]
#[doc(alias = "itZRShell_Logic12_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    knocked(r, gobj);
    false
}

/// `itZGShell_Logic11_Reflected` (the red one's `itZRShell_Logic12_Reflected`).
// Ports itZGShell_Logic11_Reflected; aliased once a check reaches it.
// Ports itZRShell_Logic12_Reflected; aliased once a check reaches it.
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Clanked, `it_2725_Logic11_Clanked` (the red one's `itZRShell_Logic12_Clanked`): it pops up.
#[doc(alias = "it_2725_Logic11_Clanked")]
// Ports itZRShell_Logic12_Clanked; aliased once a check reaches it.
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    pop_up(r, gobj);
    fell(r, gobj);
    false
}

/// Met a shield, `it_2725_Logic11_HitShield` (the red one's `itZRShell_Logic12_HitShield`): a
/// thrown or dropped shell bounces off, a sliding one pops up.
// Ports it_2725_Logic11_HitShield; aliased once a check reaches it.
// Ports itZRShell_Logic12_HitShield; aliased once a check reaches it.
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    match item(r.game, gobj).msid {
        3 | 4 => lib2725::bounce_off_victim(r.game, r.data, gobj),
        5..=8 => {
            let _ = clanked(r, gobj);
        }
        _ => {}
    }
    false
}

/// `itZGShell_Logic11_ShieldBounced` (the red one's `itZRShell_Logic12_ShieldBounced`).
// Ports itZGShell_Logic11_ShieldBounced; aliased once a check reaches it.
// Ports itZRShell_Logic12_ShieldBounced; aliased once a check reaches it.
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Jumped on, `fn_802DFE7C`: [`stomped`] and a sound.
#[doc(alias = "fn_802DFE7C")]
pub fn jumped_on(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    stomped(r, gobj);
    play_sfx(r, gobj, 0xF1, 127, 64);
    false
}

/// Forgets a GObj going away, `it_802DFED4` and the red one's `it_802E0468` (`it_8026B894`).
// Ports it_802DFED4; aliased once a check reaches it.
// Ports it_802E0468; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = lib26b1::forget(r.game, gobj, other);
}

/// Destroyed, `itZGShell_Logic11_Destroyed` (the red one's `itZRShell_Logic12_Destroyed`):
/// its spawner slot as an enemy KO (`grZakoGenerator_801CACB8`).
#[doc(alias = "itZGShell_Logic11_Destroyed")]
// Ports itZRShell_Logic12_Destroyed; aliased once a check reaches it.
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    zako::ko(r, gobj);
}

/// Its hurtbox as it spawned, raised to 6 and doubled while waking (`raised`), `it_802DFF14`.
#[doc(alias = "it_802DFF14")]
pub fn set_hurt(r: &mut Running, gobj: GObjId, raised: bool) {
    let v = zako::bytes(r.game, gobj);
    let mut a = v.vec(off::HURT + 4);
    let b = v.vec(off::HURT + 0x10);
    let mut scale = v.f32(off::HURT + 0x1C);
    if raised {
        a.y = 6.0;
        scale = mem(fp::fmuls(reg(scale), 2.0));
    }
    coll::set_hurt_capsule(r.game, r.data, gobj, 0, a, b, scale);
}

/// Forgets the moving bone's last translation, `it_802DFFA0`.
#[doc(alias = "it_802DFFA0")]
pub fn forget_bone(r: &mut Running, gobj: GObjId) {
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::BONE, 0.0);
    v.set_f32(off::BONE + 4, 0.0);
    v.set_f32(off::BONE + 8, 0.0);
}

/// Its model's bone 1 (`xBBC_dynamicBoneTable->bones[1]`).
fn bone1(r: &Running, gobj: GObjId) -> Option<crate::hsd::jobj::JObjId> {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a shell without bones");
    r.game.it.bone_tables[t.0].bones[1]
}

/// The bone's translation since last time becomes the velocity, and the bone stays put,
/// `it_802DFFB8` (the inline `itUpdateVelocityFromBone`); nothing for no bone.
#[doc(alias = "it_802DFFB8")]
pub fn follow_bone(r: &mut Running, j: Option<crate::hsd::jobj::JObjId>, gobj: GObjId) {
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

/// Settles a new shell: on the floor below (within 10) it rests or slides off a slope, else
/// it falls (the end of `it_802E0100`, which spells out `it_802DE0F0` and the slope's
/// `it_2725_Logic11_EnteredAir`, and of `it_802E0488`).
pub(super) fn settle(r: &mut Running, gobj: GObjId) {
    if snap_down(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        if !maplib::slides(r.game, r.data, gobj) {
            stopped(r, gobj);
        } else {
            entered_air(r, gobj);
        }
    } else {
        maplib::enter_air(item_mut(r.game, gobj));
        fell(r, gobj);
    }
}

/// Spawns a green shell, `it_802E0100`: an enemy item at `pos` facing `facing` (its model
/// turned that way), the Koopa that wakes from it of `kind`, settled.
#[doc(alias = "it_802E0100")]
pub fn spawn(r: &mut Running, kind: i32, pos: Vec3, facing: i32) -> Option<GObjId> {
    let gobj = zako::spawn(
        r,
        super::ZGSHELL,
        Some(pos),
        None,
        Some(Vec3::new(0.0, 0.0, 0.0)),
        true,
    )?;
    face(r, gobj, facing, kind);
    settle(r, gobj);
    Some(gobj)
}

/// A new shell's facing (and its model's, its collision's) and its Koopa's kind.
fn face(r: &mut Running, gobj: GObjId, facing: i32, kind: i32) {
    let f = mem(fp::frsp(f64::from(facing)));
    item_mut(r.game, gobj).facing_dir = f;
    zako::face(r, gobj, f);
    let ip = item_mut(r.game, gobj);
    let d = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(d);
    set(r, gobj, off::KOOPA, kind);
}
