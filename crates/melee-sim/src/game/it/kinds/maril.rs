//! Marill (`It_PKind_Maril`), `melee/it/kinds/itmaril.c`: out of the ball it walks along,
//! its body a hitbox, turning round at walls, falling off ledges, until its life runs out or
//! enough damage knocks it away.
//!
//! The unit also holds Cyndaquil's flame's (`It_Kind_Hinoarashi_Flame`) state and callbacks,
//! which the compiler placed here.

use gekko_fp as fp;

use super::super::ground::{air_stay, air_touches_walls, fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, effect_hitlag, table_bone, take_bone_motion};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use super::drmariopill::set_pose;
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::ef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

pub const FLAME_ANIM: u32 = 0x802D_66F8;
pub const FLAME_PHYS: u32 = 0x802D_6740;
pub const FLAME_COLL: u32 = 0x802D_6798;
pub const FLAME_DMG_DEALT: u32 = 0x802D_6808;
pub const FLAME_REFLECTED: u32 = 0x802D_6810;
pub const FLAME_CLANKED: u32 = 0x802D_6830;
pub const FLAME_ABSORBED: u32 = 0x802D_6838;
pub const FLAME_SHIELD_BOUNCED: u32 = 0x802D_6840;
pub const FLAME_HIT_SHIELD: u32 = 0x802D_6848;
pub const FLAME_EVT_UNK: u32 = 0x802D_6850;
pub const SPAWNED: u32 = 0x802D_6870;
pub const ANIM0: u32 = 0x802D_6978;
pub const PHYS0: u32 = 0x802D_699C;
pub const COLL0: u32 = 0x802D_69BC;
pub const ANIM1: u32 = 0x802D_6AD4;
pub const PHYS1: u32 = 0x802D_6C2C;
pub const COLL1: u32 = 0x802D_6C60;
pub const ANIM2: u32 = 0x802D_6E04;
pub const PHYS2: u32 = 0x802D_6E3C;
pub const COLL2: u32 = 0x802D_6E6C;
pub const ANIM3: u32 = 0x802D_6E98;
pub const PHYS3: u32 = 0x802D_6ED0;
pub const DMG_RECEIVED: u32 = 0x802D_6F00;
pub const EVT_UNK: u32 = 0x802D_6FB0;

/// Its variables past the release's (`itMaril_ItemVars`): the flag byte whose top bit says
/// it's turning round (`x60.x0_s.x0`), its walking speed (`x64`), `x68`, and the turn's step
/// and frames left (`x6C`'s x and y). The flame keeps its direction in `x6C`.
const FLAGS: usize = 0x60;
const TURNING: u8 = 0x80;
const SPEED: usize = 0x64;
const X68: usize = 0x68;
const TURN_STEP: usize = 0x6C;
const TURN_FRAMES: usize = 0x70;
const FLAME_DIR: usize = 0x6C;

/// Runs one of Marill's (or the flame's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        FLAME_ANIM => flame_anim(r, gobj),
        FLAME_COLL => flame_coll(r, gobj),
        FLAME_DMG_DEALT | FLAME_ABSORBED => false,
        FLAME_REFLECTED => flame_reflected(r, gobj),
        FLAME_CLANKED | FLAME_SHIELD_BOUNCED | FLAME_HIT_SHIELD => true,
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 | ANIM3 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        _ => return None,
    })
}

/// Runs one of Marill's (or the flame's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        FLAME_PHYS => flame_phys(r, gobj),
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 | PHYS3 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// The flame's `it_802D66F8`: gone with its effects when its life runs out.
#[doc(alias = "it_802D66F8")]
pub fn flame_anim(r: &mut Running, gobj: GObjId) -> bool {
    if lib2725::tick_lifetime(r.game, gobj) {
        ef::destroy_all(r, gobj);
        return true;
    }
    false
}

/// The flame's `it_802D6740`: rising by its attribute, its direction its velocity's.
#[doc(alias = "it_802D6740")]
pub fn flame_phys(r: &mut Running, gobj: GObjId) {
    let rise = attr_f32(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(rise)));
    let vel = ip.x40_vel;
    let v = pokemon::vars(r.game, gobj);
    v.set_vec(FLAME_DIR, vel);
    let mut d = v.vec(FLAME_DIR);
    d.normalize();
    v.set_vec(FLAME_DIR, d);
}

/// The flame's `it_802D6798`: touching a floor, wall or ceiling, Cyndaquil's code bounces it
/// (`it_802D61C8`, `it_802D6310`, in `ithinoarashi.c`).
#[doc(alias = "it_802D6798")]
pub fn flame_coll(r: &mut Running, gobj: GObjId) -> bool {
    let mut coll = i32::from(air_stay(r, gobj));
    coll |= maplib::touched_wall(r.game, gobj);
    coll |= maplib::touched_ceiling(r.game, gobj);
    if coll != 0 {
        code::unported_call("it_802D61C8 (Cyndaquil's flame touching the stage)");
    }
    false
}

/// The flame's `it_802D6810`: reflected back.
#[doc(alias = "it_802D6810")]
pub fn flame_reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// The flame's `it_802D6808` (damage dealt) and `it_802D6838` (absorbed): kept.
#[doc(alias = "it_802D6808")]
#[doc(alias = "it_802D6838")]
pub fn flame_kept(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The flame's `it_802D6830` (clanked), `it_802D6840` (shield bounced) and `it_802D6848`
/// (hit a shield): gone.
#[doc(alias = "it_802D6830")]
#[doc(alias = "it_802D6840")]
#[doc(alias = "it_802D6848")]
pub fn flame_gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Forgets a GObj going away, the flame's `it_802D6850` and Marill's `it_802D6FB0`
/// (`it_8026B894`).
#[doc(alias = "it_802D6850")]
#[doc(alias = "it_802D6FB0")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itMaril_Logic28_Spawned`: not turning, its animation's speed, its first size, its sound
/// and life, into the air out of the ball.
#[doc(alias = "itMaril_Logic28_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    pokemon::vars(r.game, gobj).set_bit(FLAGS, TURNING, false);
    let speed = attr_f32(r, gobj, 0x10);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x273D, 0x7F, 0x40);
    let life = attr_f32(r, gobj, 4);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    enter_air(r, gobj);
}

/// Into the air out of the ball, `it_802D68FC`: its first state at its first pose, no longer
/// kept clear of.
#[doc(alias = "it_802D68FC")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_state(r, gobj, 0);
    set_pose(r, gobj, 0, 0.0);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// `Item_EnterStateWithEffectHitlag`: in the air, into state `msid`, its effects paused in
/// hitlag.
fn enter_state(r: &mut Running, gobj: GObjId, msid: i32) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itMaril_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itMaril_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itMaril_UnkMotion0_Phys`: falling out of the ball.
#[doc(alias = "itMaril_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itMaril_UnkMotion0_Coll`: lands and walks.
#[doc(alias = "itMaril_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Lands and walks (again), `it_802D69E4`.
#[doc(alias = "it_802D69E4")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    maplib::enter_air(item_mut(r.game, gobj));
    walk(r, gobj);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    effect_hitlag(r.game, gobj);
    pokemon::vars(r.game, gobj).set_f32(X68, 8.0);
}

/// Walks, `it_802D6A54`: its speed its way, kept clear of, its hitbox on and hitting.
#[doc(alias = "it_802D6A54")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let speed = attr_f32(r, gobj, 0xC);
    let facing = item(r.game, gobj).facing_dir;
    pokemon::vars(r.game, gobj).set_f32(SPEED, mem(fp::fmuls(reg(speed), reg(facing))));
    lib26b1::keep_clear(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    hitbox::clear_hit_flag2(r.game, gobj);
}

/// Its life counted down: whether it ran out (the inline `itMaril_UnkMotion2_Anim_inline`,
/// as `it_80273130`).
fn life_out(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itMaril_UnkMotion1_Anim`: walking by its bone's motion, turning round a step a frame,
/// gone when its life runs out.
#[doc(alias = "itMaril_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let bone = table_bone(r.game, gobj, 1);
    take_bone_motion(r, bone, gobj);
    if pokemon::vars(r.game, gobj).bit(FLAGS, TURNING) {
        let j = jobj_of(r.game, gobj).expect("Marill without a model");
        let step = pokemon::vars(r.game, gobj).f32(TURN_STEP);
        add_rotation_y(r, j, step);
        let v = pokemon::vars(r.game, gobj);
        let left = mem(fp::fsubs(reg(v.f32(TURN_FRAMES)), 1.0));
        v.set_f32(TURN_FRAMES, left);
        if reg(left) <= 0.0 {
            v.set_bit(FLAGS, TURNING, false);
            item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
        }
    }
    life_out(r, gobj)
}

/// `HSD_JObjAddRotationY` (inline).
fn add_rotation_y(r: &mut Running, j: jobj::JObjId, v: f32) {
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), reg(v)));
    });
}

/// `itMaril_UnkMotion1_Phys`: the bone's motion, at its walking speed.
#[doc(alias = "itMaril_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let speed = pokemon::vars(r.game, gobj).f32(SPEED);
    item_mut(r.game, gobj).x40_vel.x = speed;
}

/// `itMaril_UnkMotion1_Coll`: at each step's end it walks on, or falls off the floor; mid-step
/// a wall turns it round.
#[doc(alias = "itMaril_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    let coll = air_touches_walls(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        if coll & 1 != 0 {
            landed(r, gobj);
        } else {
            fall_off(r, gobj);
        }
    } else if coll & 0xC != 0 {
        turn(r, gobj);
    }
    false
}

/// Turns round at a wall, unless turning (the inline `itMaril_UnkMotion1_Coll_inline`): half a
/// turn over its turn's frames, facing and walking the other way, its model turned by hand.
fn turn(r: &mut Running, gobj: GObjId) {
    if pokemon::vars(r.game, gobj).bit(FLAGS, TURNING) {
        return;
    }
    let frames = attr_f32(r, gobj, 8);
    let facing = item(r.game, gobj).facing_dir;
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(TURN_FRAMES, frames);
    let step = fp::frsp(fp::fdiv(
        fp::fmul(std::f64::consts::PI, fp::fneg(reg(facing))),
        reg(frames),
    ));
    v.set_f32(TURN_STEP, mem(step));
    v.set_bit(FLAGS, TURNING, true);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.xDC8_word.flags.x19 = false;
    let v = pokemon::vars(r.game, gobj);
    let speed = v.f32(SPEED);
    v.set_f32(SPEED, mem(fp::fneg(reg(speed))));
}

/// Falls off the floor, `it_802D6DDC`.
#[doc(alias = "it_802D6DDC")]
pub fn fall_off(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, 1);
}

/// `itMaril_UnkMotion2_Anim` and `_3_Anim`: gone when its life runs out.
#[doc(alias = "itMaril_UnkMotion2_Anim")]
#[doc(alias = "itMaril_UnkMotion3_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    life_out(r, gobj)
}

/// `itMaril_UnkMotion2_Phys` and `_3_Phys`: falling by its gravity.
#[doc(alias = "itMaril_UnkMotion2_Phys")]
#[doc(alias = "itMaril_UnkMotion3_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itMaril_UnkMotion2_Coll`: lands and walks.
#[doc(alias = "itMaril_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// `it_802D6F00`, its damage taken: enough knocks it away, flying off blinking, its effects
/// gone.
#[doc(alias = "it_802D6F00")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f32(r, gobj, 0x14);
    let damage = fp::frsp(f64::from(item(r.game, gobj).xC9C));
    if damage >= reg(limit) {
        pokemon::launch(r, gobj);
        knocked_away(r, gobj);
    }
    false
}

/// Knocked away (the inline `it_802D6F00_inline`).
fn knocked_away(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    lib26b1::stop_keeping_clear(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    ef::destroy_all(r, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    set_pose(r, gobj, 0, 0.0);
}
