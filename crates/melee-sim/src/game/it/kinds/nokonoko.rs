//! The Koopa Troopa, `melee/it/kinds/itnokonoko.c` (`It_Kind_Nokonoko`): an adventure enemy the
//! stages' spawner makes (`grZakoGenerator_801CA8B4`). It walks the way it faces for a while
//! (`x8` over its speed in frames), turns (state 3) and walks back, falls off ledges and walks
//! on when it lands. A fighter landing on it, or a hit past its health, knocks it out of its
//! shell (state 5), which leaves the shell (a green one, or red for `x40` 2 and up) where it
//! lands; a lesser hit knocks it back (state 7). Held and thrown (states 9 and 10), it ends as
//! an enemy KO when it hits the stage.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::_itECB;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_C69C;
pub const DMG_RECEIVED: u32 = 0x802D_C774;
/// `it_802DC83C`, its jumped-on callback.
pub const STOMPED: u32 = 0x802D_C83C;
pub const PICKED_UP: u32 = 0x802D_D4F4;
pub const DROPPED: u32 = 0x802D_D59C;
pub const THROWN: u32 = 0x802D_D67C;
pub const EVT_UNK: u32 = 0x802D_D7D0;
pub const ANIM1: u32 = 0x802D_C910;
pub const PHYS1: u32 = 0x802D_C918;
pub const COLL1: u32 = 0x802D_C954;
pub const ANIM2: u32 = 0x802D_CA5C;
pub const PHYS2: u32 = 0x802D_CAA4;
pub const COLL2: u32 = 0x802D_CB34;
pub const ANIM4: u32 = 0x802D_CC58;
pub const PHYS4: u32 = 0x802D_CC60;
pub const COLL6: u32 = 0x802D_CC90;
pub const ANIM8: u32 = 0x802D_CD54;
pub const PHYS7: u32 = 0x802D_CD9C;
pub const COLL8: u32 = 0x802D_CDC4;
pub const PHYS8: u32 = 0x802D_CE74;
pub const ANIM5: u32 = 0x802D_D0A0;
pub const PHYS5: u32 = 0x802D_D1B4;
pub const COLL5: u32 = 0x802D_D1DC;
pub const ANIM6: u32 = 0x802D_D25C;
pub const PHYS6: u32 = 0x802D_D264;
pub const ANIM3: u32 = 0x802D_D328;
pub const PHYS3: u32 = 0x802D_D468;
pub const COLL3: u32 = 0x802D_D46C;
pub const ANIM9: u32 = 0x802D_D53C;
pub const PHYS9: u32 = 0x802D_D598;
pub const ANIM10: u32 = 0x802D_D6D4;
pub const PHYS10: u32 = 0x802D_D71C;
pub const COLL10: u32 = 0x802D_D74C;

/// Its variables' offsets in the union (`itNokoNoko_ItemVars`).
pub mod off {
    /// `x4`: the common spawner slot, which a shell it leaves takes over.
    pub const SLOT: usize = 4;
    /// `x20`: the frames left of a walk.
    pub const WALK: usize = 0x20;
    /// `x28`: a countdown (standing, turning).
    pub const TIMER: usize = 0x28;
    /// `x34`, `x38`, `x3C`: a knockback, its direction and angle kept for `it_802DCE00`.
    pub const KB: usize = 0x34;
    pub const KB_DIR: usize = 0x38;
    pub const KB_ANGLE: usize = 0x3C;
    /// `x40`: its shell's kind (green under 2, else red).
    pub const SHELL: usize = 0x40;
    /// `x74`: its collision box as it spawned.
    pub const ECB: usize = 0x74;
}

/// Its special attributes (`itNokoNoko_DatAttrs`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Koopa Troopa without attributes")
        .at(off)
}

/// `da->x0->x0`: its health.
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r
        .data
        .ptr(attr(r, gobj, 0))
        .expect("a Koopa Troopa's health");
    r.data.u32(p) as i32
}

/// `da->x0->x4`: its walking speed.
fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r
        .data
        .ptr(attr(r, gobj, 0))
        .expect("a Koopa Troopa's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

/// `GA_Ground`, `GA_Air`.
const GA_GROUND: i32 = 0;
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// Runs one of the Koopa Troopa's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        STOMPED => stomped(r, gobj),
        ANIM1 | ANIM4 | ANIM6 => answers_false(r, gobj),
        COLL1 | COLL3 => coll_standing(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        COLL6 => coll_falling(r, gobj),
        ANIM8 => anim8(r, gobj),
        COLL8 => coll8(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM9 => anim9(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Koopa Troopa's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS4 | PHYS10 => phys_fall(r, gobj),
        PHYS7 => phys7(r, gobj),
        PHYS8 => phys8(r, gobj),
        PHYS5 => phys5(r, gobj),
        PHYS6 => phys6(r, gobj),
        PHYS3 | PHYS9 => still(r, gobj),
        _ => return false,
    }
    true
}

/// The collision's facing from the item's (the inline `coll_inline`, `mpCollSetFacingDir`).
fn set_coll_facing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(f);
}

/// Stops (the inline `itResetVelocity`): z, then y, then x.
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// The Koopa Troopa's setup, `it_802DC69C`: the enemies' common variables, its model facing its
/// way, its collision an enemy's, no timers, a green shell, its box kept, and it falls.
#[doc(alias = "it_802DC69C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let facing = item(r.game, gobj).facing_dir;
    zako::face(r, gobj, facing);
    set_coll_facing(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::SHELL, 0);
    set(r, gobj, off::WALK, 0);
    let b = item(r.game, gobj).xC1C.clone();
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::ECB, b.top);
    v.set_f32(off::ECB + 4, b.bottom);
    v.set_f32(off::ECB + 8, b.right);
    v.set_f32(off::ECB + 0xC, b.left);
    fall(r, gobj);
}

/// Takes a hit, `itNokonoko_Logic3_DmgReceived`: its facing kept, the hit's element adds to
/// the damage; thrown (10) it ends as a KO, past its health it leaves its shell, else it's
/// knocked back.
#[doc(alias = "itNokonoko_Logic3_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if msid == 0xA {
        ko(r, gobj);
    } else if damage > health(r, gobj) {
        unshell(r, gobj);
    } else {
        knock_back(r, gobj);
    }
    false
}

/// Landed on by a fighter, `it_802DC83C`: its facing kept, no stomping, out of its shell, a
/// small quake, a sound and the stomper's rumble.
#[doc(alias = "it_802DC83C")]
pub fn stomped(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    lib2725::clear_stomp(r.game, gobj);
    unshell(r, gobj);
    let pos = item(r.game, gobj).pos;
    camera::request_quake(r, QUAKE_SMALL, Some(pos));
    play_sfx(r, gobj, 0x12E, 127, 64);
    let stomper = item(r.game, gobj)
        .xCFC
        .expect("a Koopa Troopa landed on by nobody");
    lib2725::fighter_item_rumble(r, stomper, 0xC, 0);
    false
}

/// Lands, `fn_802DC8B8`: settled, its model turned to the floor, state 1.
#[doc(alias = "fn_802DC8B8")]
pub fn land(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    zako::align_floor(r, gobj, facing, 5.0);
    enter(r, gobj, 1, change::ANIM_UPDATE);
}

/// The states' animation that answers false (`itNokonoko_UnkMotion1_Anim` and its like).
#[doc(alias = "itNokonoko_UnkMotion1_Anim")]
#[doc(alias = "itNokonoko_UnkMotion4_Anim")]
// Ports itNokonoko_UnkMotion6_Anim; aliased once a check reaches it.
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states' physics that do nothing (`itNokonoko_UnkMotion3_Phys`, `_9_Phys`).
#[doc(alias = "itNokonoko_UnkMotion3_Phys")]
// Ports itNokonoko_UnkMotion9_Phys; aliased once a check reaches it.
pub fn still(_: &mut Running, _: GObjId) {}

/// Standing's physics, `itNokonoko_UnkMotion1_Phys`: its countdown done, it walks.
#[doc(alias = "itNokonoko_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t <= 0 {
        walk(r, gobj);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
}

/// Standing's and turning's collision (`itNokonoko_UnkMotion1_Coll`, `_3_Coll`): off the floor
/// it falls.
// Ports itNokonoko_UnkMotion1_Coll; aliased once a check reaches it.
#[doc(alias = "itNokonoko_UnkMotion3_Coll")]
pub fn coll_standing(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Walks, `it_802DC990`: at its speed the way it faces, no blink, its walk's frames (`x8` over
/// its speed) if none are left, state 2.
#[doc(alias = "it_802DC990")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    let speed = walk_speed(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), reg(speed)));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    set_coll_facing(r, gobj);
    hitbox::end_blink(r.game, gobj);
    if get(r, gobj, off::WALK) == 0 {
        let x8 = r.data.f32(attr(r, gobj, 8));
        let frames = fp::fctiwz(fp::fdivs(reg(x8), reg(speed)));
        set(r, gobj, off::WALK, frames);
    }
    enter(r, gobj, 2, change::ANIM_UPDATE);
}

/// Walking's animation, `itNokonoko_UnkMotion2_Anim`: looped, keeping its hits.
#[doc(alias = "itNokonoko_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 2, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// Walking's physics, `itNokonoko_UnkMotion2_Phys`: its walk done, or at the script's turn
/// point, it turns; along the floor, its model turned to it.
#[doc(alias = "itNokonoko_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let w = get(r, gobj, off::WALK);
    if w == 0 {
        turn(r, gobj);
    } else {
        set(r, gobj, off::WALK, w.wrapping_sub(1));
    }
    if lib2725::xdc0_has(r.game, gobj, 2) {
        turn(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    let facing = item(r.game, gobj).facing_dir;
    zako::align_floor(r, gobj, facing, 5.0);
}

/// Walking's collision, `itNokonoko_UnkMotion2_Coll`: at a ledge or a wall it turns; off the
/// floor it falls.
#[doc(alias = "itNokonoko_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
    } else {
        fall(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Falls, `it_802DCB9C`: its sideways speed slowed by `x4`, no countdown, turned away from a
/// nudge against its facing, state 4.
#[doc(alias = "it_802DCB9C")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    let k = r.data.f32(attr(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(k)));
    set(r, gobj, off::TIMER, 0);
    let ip = item(r.game, gobj);
    let (facing, nudge) = (reg(ip.facing_dir), reg(ip.x70_nudge.x));
    if facing > 0.0 && nudge < 0.0 || facing < 0.0 && nudge > 0.0 {
        item_mut(r.game, gobj).facing_dir = mem(fp::fneg(facing));
        set_coll_facing(r, gobj);
    }
    enter(r, gobj, 4, change::ANIM_UPDATE);
}

/// Falling's and thrown's physics (`itNokonoko_UnkMotion4_Phys`, `_10_Phys`): gravity.
#[doc(alias = "itNokonoko_UnkMotion4_Phys")]
// Ports itNokonoko_UnkMotion10_Phys; aliased once a check reaches it.
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itNokonoko_UnkMotion6_Coll`: landing, it stands.
#[doc(alias = "itNokonoko_UnkMotion6_Coll")]
pub fn coll_falling(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, land);
    zako::answers_false(r, gobj)
}

/// Knocked back, `it_802DCCCC`: no countdown, its hit records cleared, along its knockback (up
/// at least 0.2), in the air, its facing as it was hit, state 7.
#[doc(alias = "it_802DCCCC")]
pub fn knock_back(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    hitbox::clear_hit_ids(r.game, gobj);
    let mut v = item(r.game, gobj).x40_vel;
    let _ = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    // A double 0.2 in the comparison, as the C writes it.
    if reg(ip.x40_vel.y) <= 0.2 {
        ip.x40_vel.y = mem(fp::frsp(0.2));
    }
    maplib::enter_air(ip);
    ip.facing_dir = ip.init_facing_dir;
    enter(r, gobj, 7, change::ANIM_UPDATE);
}

/// Knocked back's animation, `itNokonoko_UnkMotion8_Anim` (states 7 and 8): looped.
#[doc(alias = "itNokonoko_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 7, change::ANIM_UPDATE);
    }
    false
}

/// Knocked back's physics, `itNokonoko_UnkMotion7_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itNokonoko_UnkMotion7_Phys")]
pub fn phys7(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        gravity(r, gobj);
    }
}

fn gravity(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Knocked back's collision, `itNokonoko_UnkMotion8_Coll`: landing, it recovers.
#[doc(alias = "itNokonoko_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, recover);
    zako::answers_false(r, gobj)
}

/// Knocked out of a Paratroopa, `it_802DCE00`: in the air, a 10-frame countdown, blinking, the
/// kept knockback (the Paratroopa's, [`super::patapata::spawn_koopa`]), state 8.
#[doc(alias = "it_802DCE00")]
pub fn knock_back_kept(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    set(r, gobj, off::TIMER, 10);
    hitbox::start_blink(r.game, gobj);
    let v = zako::bytes(r.game, gobj);
    let (kb, angle, dir) = (v.f32(off::KB), v.i32(off::KB_ANGLE), v.f32(off::KB_DIR));
    let ip = item_mut(r.game, gobj);
    ip.xCC8_knockback = kb;
    ip.xCAC_angle = angle;
    ip.xCCC_incDamageDirection = dir;
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// State 8's physics, `itNokonoko_UnkMotion8_Phys`: gravity, and its blink ends with its
/// countdown.
#[doc(alias = "itNokonoko_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    gravity(r, gobj);
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        hitbox::end_blink(r.game, gobj);
        return;
    }
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
}

/// Recovers from a knockback, `it_802DCEC4`: no countdown or blink; hit from behind it turns,
/// else it walks.
#[doc(alias = "it_802DCEC4")]
pub fn recover(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    hitbox::end_blink(r.game, gobj);
    let ip = item(r.game, gobj);
    if reg(ip.xCCC_incDamageDirection) == reg(ip.facing_dir) {
        turn_keep(r, gobj);
        return;
    }
    walk(r, gobj);
}

/// Out of its shell, `it_802DCFBC`: blinking, in the air or on the ground as its knockback
/// leaves it, its facing as it was hit, no countdown, state 5, its box 4.5 high and 3 wide.
#[doc(alias = "it_802DCFBC")]
pub fn unshell(r: &mut Running, gobj: GObjId) {
    hitbox::start_blink(r.game, gobj);
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
    }
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let mut b: _itECB = item(r.game, gobj).xC1C.clone();
    b.top = 4.5;
    b.right = 3.0;
    b.left = 3.0;
    maplib::set_box(r.game, gobj, b);
}

/// Out of its shell's animation, `itNokonoko_UnkMotion5_Anim`: when it ends on the ground, its
/// shell (green for `x40` under 2, else red) just above it facing as its model turns, taking
/// over its spawner slot; the Koopa goes.
#[doc(alias = "itNokonoko_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) && item(r.game, gobj).ground_or_air == GA_GROUND {
        let j = jobj_of(r.game, gobj).expect("a Koopa Troopa without a model");
        let rot_y = r.game.objs.jobjs[j].rotate.y;
        let deg = fp::fmuls(57.295_78, reg(rot_y));
        let facing = if deg < 0.0 { fp::fneg(1.0) } else { 1.0 };
        let mut pos = item(r.game, gobj).pos;
        pos.y = mem(fp::fadds(reg(pos.y), reg(0.1)));
        let kind = get(r, gobj, off::SHELL);
        let shell = if kind < 2 {
            super::zgshell::spawn(r, kind, pos, fp::fctiwz(facing))
        } else {
            super::zrshell::spawn(r, kind, pos, fp::fctiwz(facing))
        };
        if let Some(shell) = shell {
            let slot = get(r, gobj, off::SLOT);
            zako::bytes(r.game, shell).set_i32(off::SLOT, slot);
            set(r, gobj, off::SLOT, -1);
        }
        return true;
    }
    false
}

/// Out of its shell's physics, `itNokonoko_UnkMotion5_Phys`: in the air, gravity.
#[doc(alias = "itNokonoko_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        gravity(r, gobj);
    }
}

/// Out of its shell's collision, `itNokonoko_UnkMotion5_Coll`: in the air, landing on a floor
/// it stops on the ground; on the ground it stays on it or leaves it.
#[doc(alias = "itNokonoko_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        if ground::air_touches_walls(r, gobj) & 1 != 0 {
            maplib::enter_ground(item_mut(r.game, gobj));
            stop(r, gobj);
        }
    } else {
        ground::ground_or_air(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// State 6's physics, `itNokonoko_UnkMotion6_Phys`: its model turned to the floor at once.
// Ports itNokonoko_UnkMotion6_Phys; aliased once a check reaches it.
pub fn phys6(r: &mut Running, gobj: GObjId) {
    let facing = item(r.game, gobj).facing_dir;
    zako::align_floor(r, gobj, facing, 0.0);
}

/// Turns (the inline `it_802DD290_inline`): stopped, no walk left, 38 frames, state 3.
fn start_turn(r: &mut Running, gobj: GObjId, flags: u32) {
    stop(r, gobj);
    set(r, gobj, off::WALK, 0);
    set(r, gobj, off::TIMER, 0x26);
    enter(r, gobj, 3, flags);
}

/// Turns, `it_802DD290`, keeping its hits.
#[doc(alias = "it_802DD290")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    start_turn(r, gobj, change::ANIM_UPDATE | change::HIT_PRESERVE);
}

/// Turns, `it_802DD2DC`.
// Ports it_802DD2DC; aliased once a check reaches it.
pub fn turn_keep(r: &mut Running, gobj: GObjId) {
    start_turn(r, gobj, change::ANIM_UPDATE);
}

/// Turning's animation, `itNokonoko_UnkMotion3_Anim`: its facing flips when the countdown
/// reaches 0; at the end, the floor probed and it walks.
#[doc(alias = "itNokonoko_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r, gobj);
    }
    let t = get(r, gobj, off::TIMER);
    set(r, gobj, off::TIMER, t.wrapping_sub(1));
    if !lib2725::is_animating(r.game, gobj) {
        let _ = ground::ground_probe(r, gobj);
        set(r, gobj, off::TIMER, 0);
        walk(r, gobj);
    }
    false
}

/// Enters a state it can be landed on in, `it_802DD4A8`: stompable, its jumped-on callback
/// [`stomped`].
#[doc(alias = "it_802DD4A8")]
pub fn enter(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    change_state(r, gobj, msid, flags);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(STOMPED));
}

/// Picked up, `it_802DD4F4`: no countdown, no spin, state 9.
// Ports it_802DD4F4; aliased once a check reaches it.
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// Held's animation, `itNokonoko_UnkMotion9_Anim`: looped as when picked up.
// Ports itNokonoko_UnkMotion9_Anim; aliased once a check reaches it.
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        picked_up(r, gobj);
    }
    false
}

/// Dropped, `it_802DD59C`: the enemies' drop, then it falls.
// Ports it_802DD59C; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `it_802DD67C`: the enemies' throw, no stomping, no countdown, state 10.
// Ports it_802DD67C; aliased once a check reaches it.
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    lib2725::clear_stomp(r.game, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 0xA, change::ANIM_UPDATE);
}

/// Thrown's animation, `itNokonoko_UnkMotion10_Anim`: looped.
// Ports itNokonoko_UnkMotion10_Anim; aliased once a check reaches it.
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0xA, change::ANIM_UPDATE);
    }
    false
}

/// Thrown's collision, `itNokonoko_UnkMotion10_Coll`: touching the stage, it ends as a KO.
// Ports itNokonoko_UnkMotion10_Coll; aliased once a check reaches it.
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    if ground::air_touches(r, gobj) & 0xF != 0 {
        ko(r, gobj);
    }
    false
}

/// An enemy KO, `it_802DD78C`: an effect and a sound, the KO, and gone.
// Ports it_802DD78C; aliased once a check reaches it.
pub fn ko(r: &mut Running, gobj: GObjId) {
    lib2725::destroy_effect(r, gobj, 0x444, 0x130);
    zako::ko(r, gobj);
    super::super::item::destroy(r, gobj);
}

/// Forgets a GObj going away, `it_802DD7D0` (`it_8026B894`).
// Ports it_802DD7D0; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = lib26b1::forget(r.game, gobj, other);
}

/// Spawns a Koopa Troopa, `it_802DD7F0`: at `pos` with its shell's kind and its facing (`slot`
/// is the spawner's -1, a float facing), its model turned that way; with a red shell (2) its
/// first state's animation set; on the floor below (within 10) it lands and stands, else it
/// falls.
#[doc(alias = "it_802DD7F0")]
pub fn spawn(
    r: &mut Running,
    x2: i32,
    pos: Vec3,
    vel: Option<Vec3>,
    facing: i32,
) -> Option<GObjId> {
    let gobj = zako::spawn(r, super::NOKONOKO, Some(pos), None, vel, true)?;
    let j = jobj_of(r.game, gobj).expect("a Koopa Troopa without a model");
    let f = mem(fp::frsp(f64::from(facing)));
    item_mut(r.game, gobj).facing_dir = f;
    set_coll_facing(r, gobj);
    let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(f))));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, rot);
    set(r, gobj, off::SHELL, x2);
    if x2 == 2 {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        jobj::anim_all(
            &mut r.game.objs,
            r.data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }
    if ground::snap_down(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        land(r, gobj);
    } else {
        maplib::enter_air(item_mut(r.game, gobj));
        fall(r, gobj);
    }
    Some(gobj)
}
