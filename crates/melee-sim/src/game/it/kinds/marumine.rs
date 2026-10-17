//! Electrode (`It_PKind_Marumine`), `melee/it/kinds/itmarumine.c`: out of the ball it drops to
//! the floor and sits, sparking, while a fuse runs; it can be picked up and thrown; it explodes
//! when its animation ends, when thrown into something, or when its collision bursts it.

use gekko_fp as fp;

use super::super::ground::{air, fall, fall_until_touch_unstick, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, table_bone};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, release};
use crate::fpu::reg;
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_0A0C;
pub const LANDED_NOTHING: u32 = 0x802D_0AAC;
pub const EVT_UNK: u32 = 0x802D_0AB0;
pub const DMG_DEALT: u32 = 0x802D_0AD0;
pub const LANDED: u32 = 0x802D_0B04;
pub const ANIM1: u32 = 0x802D_0B74;
pub const PHYS1: u32 = 0x802D_0BB4;
pub const COLL1: u32 = 0x802D_0BB8;
pub const ANIM2: u32 = 0x802D_0CB4;
pub const PHYS2: u32 = 0x802D_0CBC;
pub const COLL2: u32 = 0x802D_0CEC;
pub const PICKED_UP: u32 = 0x802D_0D18;
pub const ANIM3: u32 = 0x802D_0D70;
pub const PHYS3: u32 = 0x802D_0DB0;
pub const COLL3: u32 = 0x802D_0DB4;
pub const HELD_ACCESSORY: u32 = 0x802D_0DBC;
pub const DROPPED: u32 = 0x802D_0E30;
pub const THROWN: u32 = 0x802D_0E90;
pub const ANIM4: u32 = 0x802D_0EFC;
pub const PHYS4: u32 = 0x802D_0F3C;
pub const COLL4: u32 = 0x802D_0F6C;
pub const THROWN_ACCESSORY: u32 = 0x802D_0F98;
pub const FUSE: u32 = 0x802D_100C;
pub const ANIM5: u32 = 0x802D_1078;
pub const PHYS5: u32 = 0x802D_10B8;
pub const COLL5: u32 = 0x802D_10F4;
pub const FUSE_ACCESSORY: u32 = 0x802D_1140;
pub const EXPLODE: u32 = 0x802D_1204;
pub const ANIM6: u32 = 0x802D_12F4;
pub const PHYS6: u32 = 0x802D_1314;
pub const COLL6: u32 = 0x802D_1318;
pub const ANIM0: u32 = 0x802D_1384;
pub const PHYS0: u32 = 0x802D_13A8;
pub const COLL0: u32 = 0x802D_1410;

/// Its variables past the release's (`itPokemon_ItemVars`): the frames before its fuse starts
/// to spark and make it pickable (`timer`), and the frames to its next spark (`x64`).
const FUSE_FRAMES: usize = 0x60;
const SPARK_FRAMES: usize = 0x64;

/// Runs one of Electrode's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT => dmg_dealt(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 | COLL3 | COLL6 => false,
        COLL2 => coll2(r, gobj),
        ANIM3 | ANIM4 | ANIM5 => explode_at_end(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM6 => anim6(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        _ => return None,
    })
}

/// Runs one of Electrode's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING | PHYS1 | PHYS3 | PHYS6 => {}
        LANDED => landed(r, gobj),
        PHYS2 | PHYS4 => super::gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        HELD_ACCESSORY | THROWN_ACCESSORY => spark(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        FUSE => fuse(r, gobj),
        PHYS5 => phys5(r, gobj),
        FUSE_ACCESSORY => fuse_when_told(r, gobj),
        EXPLODE => explode(r, gobj),
        PHYS0 => phys0(r, gobj),
        _ => return false,
    }
    true
}

/// Its collision burst it, `it_802D09D0` (from `it_8026C368`): it explodes unless held or
/// already exploding.
#[doc(alias = "it_802D09D0")]
pub fn burst(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDC8_word.flags.x13 && ip.msid != 6 {
        explode(r, gobj);
    }
}

/// `itMarumine_Logic16_Spawned`: facing nowhere, held for another (`xDC8` bit 0), its fuse's
/// delay, at its first size, still, into the air out of the ball, its spark's frames, its
/// script's flag set.
#[doc(alias = "itMarumine_Logic16_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDC8_word.flags.x0 = true;
    let max = attr_i32(r, gobj, 8);
    pokemon::vars(r.game, gobj).set_i32(FUSE_FRAMES, 0xB4_i32.wrapping_sub(max));
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    lib2725::clear_velocity(r.game, gobj);
    enter_air(r, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x0 = true;
    let sparks = attr_i32(r, gobj, 0xC);
    pokemon::vars(r.game, gobj).set_i32(SPARK_FRAMES, sparks);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
}

/// Landing in its states, `it_802D0AAC`: nothing.
#[doc(alias = "it_802D0AAC")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802D0AB0` (`it_8026B894`).
#[doc(alias = "it_802D0AB0")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `it_802D0AD0`, its damage dealt: thrown, it explodes.
#[doc(alias = "it_802D0AD0")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 4 {
        explode(r, gobj);
    }
    false
}

/// Its animation's speed, joints and all.
fn set_anim_speed(r: &mut Running, gobj: GObjId, speed: f32) {
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x5D0_animFrameSpeed = speed;
    let speed = ip.x5D0_animFrameSpeed;
    crate::lb::anim::set_framerate(&mut r.game.objs, j, speed);
}

/// Lands and sits, `it_802D0B04`: its animation running.
#[doc(alias = "it_802D0B04")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    set_anim_speed(r, gobj, 1.0);
    change_state(r, gobj, 1, 1);
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion1_Anim`: the fuse lit when its animation ends.
#[doc(alias = "itMarumine_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fuse(r, gobj);
    }
    false
}

/// `itMarumine_UnkMotion1_Phys`, `_3_Phys` and `_6_Phys`: nothing.
#[doc(alias = "itMarumine_UnkMotion1_Phys")]
#[doc(alias = "itMarumine_UnkMotion3_Phys")]
#[doc(alias = "itMarumine_UnkMotion6_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// `itMarumine_UnkMotion1_Coll`: off the floor past its attribute's frame, it falls with its
/// animation stopped.
#[doc(alias = "itMarumine_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if !air(r, gobj) {
        let frame = attr_i32(r, gobj, 4);
        let at = fp::frsp(f64::from(frame));
        if reg(item(r.game, gobj).x5CC_currentAnimFrame) >= at {
            fell(r, gobj);
        }
    }
    false
}

/// Falls off the floor, `it_802D0C44`: its animation stopped.
#[doc(alias = "it_802D0C44")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    set_anim_speed(r, gobj, 0.0);
    change_state(r, gobj, 2, 1);
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion2_Anim`, `_3_Coll` and `_6_Coll`: nothing.
#[doc(alias = "itMarumine_UnkMotion2_Anim")]
#[doc(alias = "itMarumine_UnkMotion3_Coll")]
#[doc(alias = "itMarumine_UnkMotion6_Coll")]
pub fn nothing(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itMarumine_UnkMotion2_Phys` and `_4_Phys`: falling by its gravity.
#[doc(alias = "itMarumine_UnkMotion2_Phys")]
#[doc(alias = "itMarumine_UnkMotion4_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itMarumine_UnkMotion2_Coll`: lands and sits.
#[doc(alias = "itMarumine_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Picked up, `it_802D0D18`: sparking in its holder's hand.
#[doc(alias = "it_802D0D18")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, 1);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(HELD_ACCESSORY));
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion3_Anim`, `_4_Anim` and `_5_Anim`: explodes when its animation ends.
#[doc(alias = "itMarumine_UnkMotion3_Anim")]
#[doc(alias = "itMarumine_UnkMotion4_Anim")]
#[doc(alias = "itMarumine_UnkMotion5_Anim")]
pub fn explode_at_end(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        explode(r, gobj);
    }
    false
}

/// A spark every so often, `it_802D0DBC` (held) and `fn_802D0F98` (thrown): an effect on its
/// fourth bone.
#[doc(alias = "it_802D0DBC")]
#[doc(alias = "fn_802D0F98")]
pub fn spark(r: &mut Running, gobj: GObjId) {
    let v = pokemon::vars(r.game, gobj);
    let n = v.i32(SPARK_FRAMES).wrapping_sub(1);
    v.set_i32(SPARK_FRAMES, n);
    if n == 0 {
        spark_effect(r, gobj);
    }
}

fn spark_effect(r: &mut Running, gobj: GObjId) {
    let bone = table_bone(r.game, gobj, 3);
    ef::spawn(r, 0x471, Some(gobj), &[Arg::JObj(bone), Arg::Float(1.0)]);
    let sparks = attr_i32(r, gobj, 0xC);
    pokemon::vars(r.game, gobj).set_i32(SPARK_FRAMES, sparks);
}

/// Dropped, `it_802D0E30`: falls, not pickable.
#[doc(alias = "it_802D0E30")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, 1);
    lib26b1::disable_pickup(r.game, gobj);
    effect_hitlag(r.game, gobj);
}

/// Thrown, `it_802D0E90`: flies, not pickable, sparking.
#[doc(alias = "it_802D0E90")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, 1);
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(THROWN_ACCESSORY));
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion4_Coll`: explodes when it touches the stage.
#[doc(alias = "itMarumine_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    fall_until_touch_unstick(r, gobj, explode);
    false
}

/// Lights its fuse, `it_802D100C`: not pickable yet, counting down.
#[doc(alias = "it_802D100C")]
pub fn fuse(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FUSE_ACCESSORY));
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion5_Phys`: falling by its gravity in the air.
#[doc(alias = "itMarumine_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == 1 {
        super::gravity(r, gobj);
    }
}

/// `itMarumine_UnkMotion5_Coll`: lands in the air, falls off the floor on it, doing nothing
/// either way.
#[doc(alias = "itMarumine_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        fall(r, gobj, landed_nothing);
    } else {
        ground_or_fall(r, gobj, landed_nothing);
    }
    false
}

/// Its `on_accessory` with the fuse lit, `it_802D1140`: after the delay, sparking, pickable,
/// with its fuse's sound once.
#[doc(alias = "it_802D1140")]
pub fn fuse_when_told(r: &mut Running, gobj: GObjId) {
    let v = pokemon::vars(r.game, gobj);
    let t = v.i32(FUSE_FRAMES).wrapping_sub(1);
    v.set_i32(FUSE_FRAMES, t);
    if t < 0 {
        spark(r, gobj);
        lib26b1::enable_pickup(r.game, gobj);
        if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
            item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
            play_sfx(r, gobj, 0x271F, 0x7F, 0x40);
        }
    }
}

/// Explodes, `it_802D1204`: its sound; let go of if held; hidden, vanishing, still, its
/// explosion's hitboxes on (its script's, state 6).
#[doc(alias = "it_802D1204")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    play_sfx(r, gobj, 0x2720, 0x7F, 0x40);
    if item(r.game, gobj).xDC8_word.flags.x13 {
        release::throw(r, gobj, Vec3::new(0.0, 0.0, 0.0));
    }
    lib26b1::disable_pickup(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    lib2725::clear_velocity(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion6_Anim`: gone when its life runs out.
#[doc(alias = "itMarumine_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Into the air out of the ball, `it_802D1320` (`Item_EnterAirStateWithHitlag`, state 0).
#[doc(alias = "it_802D1320")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itMarumine_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itMarumine_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itMarumine_UnkMotion0_Phys`: out of the ball (no gravity for it), it sits once its frames
/// are up.
#[doc(alias = "itMarumine_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        change_state(r, gobj, 1, 1);
        effect_hitlag(r.game, gobj);
    }
}

/// `itMarumine_UnkMotion0_Coll`: lands out of the ball, doing nothing.
#[doc(alias = "itMarumine_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}
