//! Lugia and Ho-oh as one bird (`melee/it/kinds/itlugia.c` and `ithouou.c`, near-copies of each
//! other): out of the ball it grows and falls (state 0), poses (1), flies up off the top of the
//! screen (2), comes back into the background towards the camera (3), sinks back to where it
//! came out (4), and attacks on its script's word until its animation ends (5). Its shots, the
//! Aeroblasts and the Sacred Fire, burn or fly out their life until they touch the stage.
//!
//! Each bird's [`Legendary`] holds what differs: where its variables keep its climb, its script's
//! second word, its take-off's speed, and its sink and attack, which are each bird's own
//! (Lugia's sink is the speed that brings it to rest where it came out and it fires once at
//! rest; Ho-oh's sinks by its attributes, stops 10 below, and calls its fire when the sink's
//! animation ends).

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, enter_first_state};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// Its variables past the release's (`itLugia_ItemVars`, `itHouou_ItemVars`), the same in both:
/// the frames it comes towards the camera (`x60`; the Sacred Fire keeps its stage line there)
/// and where it came out (`x64`).
pub const COME_FRAMES: usize = 0x60;
pub const HOME: usize = 0x64;

/// One bird's settings.
pub struct Legendary {
    /// Where its variables keep its climb's or sink's speed (Lugia's `xE50.x`, Ho-oh's
    /// `vel_accum`).
    pub climb: usize,
    /// Its script has a second word (Ho-oh's: its fire), cleared at its spawn.
    pub second_script_word: bool,
    /// Its take-off climbs from its attributes' speed at this offset, upward (Lugia's `x4`),
    /// or from a standstill.
    pub take_off_speed: Option<u32>,
    /// It sinks (`it_802D1830`, `it_802D290C`): into state 4 at its speed.
    pub sink: fn(&mut Running, GObjId),
    /// When the sink's animation ends: Lugia plays it again (it fires once at rest, in its
    /// physics), Ho-oh calls its fire.
    pub sink_ended: fn(&mut Running, GObjId),
}

/// Its root joint's child's motion taken as its own.
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// Into state `msid`, its effects pausing in hitlag.
pub fn enter(r: &mut Running, gobj: GObjId, msid: i32) {
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `it_2725_Logic17_Spawned` and `it_2725_Logic18_Spawned`: where it came out kept, facing
/// nowhere, its script's words clear, its frames towards the camera from its attributes, not
/// kept in the blast zones, into the air out of the ball at its first size, not climbing.
#[doc(alias = "it_2725_Logic17_Spawned")]
#[doc(alias = "it_2725_Logic18_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId, b: &Legendary) {
    let frames = attr_i32(r, gobj, 0x14);
    let pos = item(r.game, gobj).pos;
    pokemon::vars(r.game, gobj).set_vec(HOME, pos);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.cmd_var0 = 0;
    if b.second_script_word {
        ip.cmd_var1 = 0;
    }
    pokemon::vars(r.game, gobj).set_i32(COME_FRAMES, frames);
    item_mut(r.game, gobj).state_flags.checks_blast_zones = false;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    pokemon::vars(r.game, gobj).set_f32(b.climb, 0.0);
}

/// Landing out of the ball, `it_802D14D0` and `it_802D25B8`: nothing.
#[doc(alias = "it_802D14D0")]
#[doc(alias = "it_802D25B8")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away (`it_8026B894`): the birds', `itLugia_Logic17_EvtUnk` and
/// `it_802D25BC`, and their shots', `it_802D23D4` and `it_802D2ED0`.
#[doc(alias = "itLugia_Logic17_EvtUnk")]
#[doc(alias = "it_802D23D4")]
#[doc(alias = "it_802D25BC")]
#[doc(alias = "it_802D2ED0")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itLugia_UnkMotion1_Anim` and `itHouou_UnkMotion1_Anim`: out of the ball, its bone's motion;
/// it takes off when its animation ends.
#[doc(alias = "itLugia_UnkMotion1_Anim")]
#[doc(alias = "itHouou_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId, b: &Legendary) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        take_off(r, gobj, b);
    }
    false
}

/// `itLugia_UnkMotion1_Phys` and `_UnkMotion5_Phys`, `itHouou_UnkMotion1_Phys` and
/// `_UnkMotion5_Phys`: the bone's motion.
#[doc(alias = "itLugia_UnkMotion1_Phys")]
#[doc(alias = "itLugia_UnkMotion5_Phys")]
#[doc(alias = "itHouou_UnkMotion1_Phys")]
#[doc(alias = "itHouou_UnkMotion5_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itLugia_UnkMotion1_Coll` to `_UnkMotion5_Coll`, and Ho-oh's: nothing.
#[doc(alias = "itLugia_UnkMotion1_Coll")]
#[doc(alias = "itLugia_UnkMotion2_Coll")]
#[doc(alias = "itLugia_UnkMotion3_Coll")]
#[doc(alias = "itLugia_UnkMotion4_Coll")]
#[doc(alias = "itLugia_UnkMotion5_Coll")]
#[doc(alias = "itHouou_UnkMotion1_Coll")]
#[doc(alias = "itHouou_UnkMotion2_Coll")]
#[doc(alias = "itHouou_UnkMotion3_Coll")]
#[doc(alias = "itHouou_UnkMotion4_Coll")]
#[doc(alias = "itHouou_UnkMotion5_Coll")]
pub fn coll_nothing(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Takes off, `it_802D1580` and `it_802D2668`: climbing from its take-off speed, or from a
/// standstill.
#[doc(alias = "it_802D1580")]
#[doc(alias = "it_802D2668")]
pub fn take_off(r: &mut Running, gobj: GObjId, b: &Legendary) {
    let climb = match b.take_off_speed {
        Some(off) => mem(fp::fneg(reg(attr_f32(r, gobj, off)))),
        None => 0.0,
    };
    pokemon::vars(r.game, gobj).set_f32(b.climb, climb);
    enter(r, gobj, 2);
}

/// `itLugia_UnkMotion2_Anim` and `itHouou_UnkMotion2_Anim`: its bone's motion.
#[doc(alias = "itLugia_UnkMotion2_Anim")]
#[doc(alias = "itHouou_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    false
}

/// `itLugia_UnkMotion2_Phys` and `itHouou_UnkMotion2_Phys`: the bone's motion and its climb,
/// faster by `x8` a frame while its animation plays and by `xC` after; past the top of the blast
/// zone it stops and comes back.
#[doc(alias = "itLugia_UnkMotion2_Phys")]
#[doc(alias = "itHouou_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId, b: &Legendary) {
    pokemon::move_by_bone(r.game, gobj);
    let accel = if lib2725::is_animating(r.game, gobj) {
        attr_f32(r, gobj, 8)
    } else {
        attr_f32(r, gobj, 0xC)
    };
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(b.climb)), reg(accel)));
    v.set_f32(b.climb, climb);
    let ip = item_mut(r.game, gobj);
    ip.vel.y = mem(fp::fadds(reg(ip.vel.y), reg(climb)));
    let top = stage::blast_top(r.game);
    if reg(item(r.game, gobj).pos.y) > reg(top) {
        item_mut(r.game, gobj).vel.y = 0.0;
        come_back(r, gobj);
    }
}

/// Comes back towards the camera, `it_802D16D4` and `it_802D27B0` (and their inline copies in
/// the third states' animation callbacks).
#[doc(alias = "it_802D16D4")]
#[doc(alias = "it_802D27B0")]
pub fn come_back(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 3);
}

/// `itLugia_UnkMotion3_Anim` and `itHouou_UnkMotion3_Anim`: its bone's motion, its animation
/// over and over.
#[doc(alias = "itLugia_UnkMotion3_Anim")]
#[doc(alias = "itHouou_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        come_back(r, gobj);
    }
    false
}

/// `itLugia_UnkMotion3_Phys` and `itHouou_UnkMotion3_Phys`: the bone's motion; towards the
/// camera at `x10` for its frames, then it stops and sinks.
#[doc(alias = "itLugia_UnkMotion3_Phys")]
#[doc(alias = "itHouou_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId, b: &Legendary) {
    pokemon::move_by_bone(r.game, gobj);
    let v = pokemon::vars(r.game, gobj);
    let left = v.i32(COME_FRAMES);
    v.set_i32(COME_FRAMES, left.wrapping_sub(1));
    if left != 0 {
        let speed = attr_f32(r, gobj, 0x10);
        item_mut(r.game, gobj).vel.z = speed;
        return;
    }
    item_mut(r.game, gobj).vel.z = 0.0;
    (b.sink)(r, gobj);
}

/// `itLugia_UnkMotion4_Anim` and `itHouou_UnkMotion4_Anim`: its bone's motion, and its
/// [`Legendary::sink_ended`] when the animation ends.
#[doc(alias = "itLugia_UnkMotion4_Anim")]
#[doc(alias = "itHouou_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId, b: &Legendary) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        (b.sink_ended)(r, gobj);
    }
    false
}

/// `itLugia_UnkMotion5_Anim` and `itHouou_UnkMotion5_Anim`: attacking; gone when its animation
/// ends.
#[doc(alias = "itLugia_UnkMotion5_Anim")]
#[doc(alias = "itHouou_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// Into the air out of the ball, `it_802D1D40` and `it_802D2BE0`
/// (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D1D40")]
#[doc(alias = "it_802D2BE0")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `it_802D1DB4` and `it_802D2C54`: growing out of the ball.
#[doc(alias = "it_802D1DB4")]
#[doc(alias = "it_802D2C54")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802D1DD8` and `it_802D2C78`: falling out of the ball; once its frames are up it stops
/// and poses.
#[doc(alias = "it_802D1DD8")]
#[doc(alias = "it_802D2C78")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        enter(r, gobj, 1);
        set_flag(&mut item_mut(r.game, gobj).phys_flags, 1, true);
    }
}

/// `it_802D1E64` and `it_802D2D04`: lands out of the ball, doing nothing.
#[doc(alias = "it_802D1E64")]
#[doc(alias = "it_802D2D04")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}

/// A shot sets out, `it_802D23F4` (an Aeroblast) and `it_2725_Logic42_Spawned` (the Sacred
/// Fire, its `it_802D2EF0` inline): its life, not spinning, into its state, not pickable.
#[doc(alias = "it_802D23F4")]
#[doc(alias = "it_2725_Logic42_Spawned")]
pub fn shot_out(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).life_timer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    enter(r, gobj, 0);
    lib26b1::disable_pickup(r.game, gobj);
}

/// A shot's `it_802D246C` and `it_802D2F3C` (`Item_TickLifetime`): gone once its life has run
/// out, else a frame less.
#[doc(alias = "it_802D246C")]
#[doc(alias = "it_802D2F3C")]
pub fn shot_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.life_timer) <= 0.0 {
        return true;
    }
    ip.life_timer = mem(fp::fsubs(reg(ip.life_timer), 1.0));
    false
}

/// A shot's `it_802D24FC` and `it_802D2FE8`: gone when it touches the stage (`it_8026DA08`).
#[doc(alias = "it_802D24FC")]
#[doc(alias = "it_802D2FE8")]
pub fn shot_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj)
}
