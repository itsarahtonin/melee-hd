//! Entei (`It_PKind_Entei`), `melee/it/kinds/itentei.c`: out of the ball it lands with a large
//! quake and roars up its Fire Spin (its script says when), roaring again every so many frames
//! while its effects burn, and goes when its animation ends.
//!
//! Raikou's and Suicune's units (`itraikou.c`, `itsuikun.c`) are copies of Entei's with their
//! own sounds and effect: each of the three is a [`Beast`] this module's code runs.

use super::super::ground::{Event, fall_keep_hits, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::game::camera;
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_F3E0;
pub const LANDED_NOTHING: u32 = 0x802C_F44C;
pub const EVT_UNK: u32 = 0x802C_F450;
pub const OUT_LANDED: u32 = 0x802C_F470;
pub const ANIM0: u32 = 0x802C_F544;
pub const PHYS0: u32 = 0x802C_F640;
pub const COLL0: u32 = 0x802C_F67C;
pub const FIRE_ACCESSORY: u32 = 0x802C_F6C8;
pub const ANIM1: u32 = 0x802C_F7A8;
pub const PHYS1: u32 = 0x802C_F7CC;
pub const COLL1: u32 = 0x802C_F7EC;

/// What tells the legendary beasts apart: their callbacks' addresses, the effect and sound
/// their script starts, the three roars, and the callbacks their landings run.
pub struct Beast {
    /// Its callbacks, by the order of Entei's constants above.
    pub spawned: u32,
    pub landed_nothing: u32,
    pub out_landed: u32,
    pub anim0: u32,
    pub phys0: u32,
    pub coll0: u32,
    pub accessory: u32,
    pub anim1: u32,
    pub phys1: u32,
    pub coll1: u32,
    /// The effect on its model and its sound when its script says (`efSync_Spawn`'s and
    /// `Item_8026AE84`'s).
    pub effect: i32,
    pub cry: i32,
    /// The roars it picks one of at random.
    pub roars: [i32; 3],
    /// Its landings' callbacks, the ports of `landed_nothing` and `out_landed`.
    pub on_landed: Event,
    pub on_out_landed: Event,
}

/// Entei.
pub const ENTEI: Beast = Beast {
    spawned: SPAWNED,
    landed_nothing: LANDED_NOTHING,
    out_landed: OUT_LANDED,
    anim0: ANIM0,
    phys0: PHYS0,
    coll0: COLL0,
    accessory: FIRE_ACCESSORY,
    anim1: ANIM1,
    phys1: PHYS1,
    coll1: COLL1,
    effect: 0x468,
    cry: 0x2741,
    roars: [0x2742, 0x2743, 0x2744],
    on_landed: landed_nothing,
    on_out_landed: out_landed,
};

/// Its variable past the release's (`itPokemon_ItemVars`' `timer`): the frames to its next
/// roar.
const ROAR_FRAMES: usize = 0x60;

/// `QuakeKind_Large`.
const QUAKE_LARGE: i32 = 4;

/// Runs one of Entei's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    run_pred_of(&ENTEI, r, gobj, addr)
}

/// Runs one of Entei's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    run_event_of(&ENTEI, r, gobj, addr)
}

/// Runs one of a beast's predicates, if `addr` is one.
pub fn run_pred_of(b: &Beast, r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        a if a == b.anim0 => anim0_of(b, r, gobj),
        a if a == b.coll0 => coll0_of(b, r, gobj),
        a if a == b.anim1 => anim1_of(r, gobj),
        a if a == b.coll1 => coll1_of(b, r, gobj),
        _ => return None,
    })
}

/// Runs one of a beast's events, if `addr` is one.
pub fn run_event_of(b: &Beast, r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        a if a == b.spawned => spawned_of(r, gobj),
        a if a == b.landed_nothing => {}
        a if a == b.out_landed => out_landed_of(b, r, gobj),
        a if a == b.phys0 => phys0_of(r, gobj),
        a if a == b.accessory => fire_when_told_of(b, r, gobj),
        a if a == b.phys1 => phys1_of(r, gobj),
        _ => return false,
    }
    true
}

/// Entei out of the ball, `it_802CF3E0`.
#[doc(alias = "it_802CF3E0")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    spawned_of(r, gobj);
}

/// A beast out of the ball: facing its owner's nearest opponent, its script's flags clear, at
/// its first size, into the air out of the ball.
pub fn spawned_of(r: &mut Running, gobj: GObjId) {
    pokemon::face_nearest_opponent(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDB4_itcmd_var2 = 0;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
}

/// Landing or leaving the floor in its first state, `it_802CF44C`: nothing.
#[doc(alias = "it_802CF44C")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CF450` (`it_8026B894`).
#[doc(alias = "it_802CF450")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Entei lands out of the ball, `it_802CF470`.
#[doc(alias = "it_802CF470")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    out_landed_of(&ENTEI, r, gobj);
}

/// A beast lands out of the ball: a large quake, its landing sound, stopped, and roaring.
pub fn out_landed_of(b: &Beast, r: &mut Running, gobj: GObjId) {
    camera::request_quake(r, QUAKE_LARGE, None);
    play_sfx(r, gobj, 9, 0x7F, 0x40);
    lib2725::clear_velocity(r.game, gobj);
    roar_of(b, r, gobj);
}

/// Entei roars, `it_802CF4D4`.
#[doc(alias = "it_802CF4D4")]
pub fn roar(r: &mut Running, gobj: GObjId) {
    roar_of(&ENTEI, r, gobj);
}

/// A beast roars: its effect on its script's word, the frames to its next roar from
/// `itPokemonAttributes`' `timer`.
pub fn roar_of(b: &Beast, r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(b.accessory));
    let frames = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(ROAR_FRAMES, frames);
}

/// `itEntei_UnkMotion0_Anim`.
#[doc(alias = "itEntei_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    anim0_of(&ENTEI, r, gobj)
}

/// A beast's first state's animation: when its script says, its effects go out and it stops
/// roaring; gone when its animation ends; while its effects run, one of three roars every so
/// many frames.
pub fn anim0_of(b: &Beast, r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let ip = item_mut(r.game, gobj);
        ip.xDB0_itcmd_var1 = 0;
        ip.xDB4_itcmd_var2 = 0;
        ef::destroy_all(r, gobj);
    }
    if !lib2725::is_animating(r.game, gobj) {
        return true;
    }
    if item(r.game, gobj).xDB4_itcmd_var2 != 0 {
        let v = pokemon::vars(r.game, gobj);
        let left = v.i32(ROAR_FRAMES).wrapping_sub(1);
        v.set_i32(ROAR_FRAMES, left);
        if left == 0 {
            let sfx = match r.game.objs.rng.below(3) {
                n @ 0..=2 => b.roars[n as usize],
                n => unreachable!("HSD_Randi(3) gave {n}"),
            };
            play_sfx(r, gobj, sfx, 0x7F, 0x40);
            let frames = attr_i32(r, gobj, 4);
            pokemon::vars(r.game, gobj).set_i32(ROAR_FRAMES, frames);
        }
    }
    false
}

/// `itEntei_UnkMotion0_Phys`.
#[doc(alias = "itEntei_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    phys0_of(r, gobj);
}

/// A beast's first state's physics: gravity in the air.
pub fn phys0_of(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == 1 {
        super::gravity(r, gobj);
    }
}

/// `itEntei_UnkMotion0_Coll`.
#[doc(alias = "itEntei_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    coll0_of(&ENTEI, r, gobj)
}

/// A beast's first state's collision: lands in the air (its hits kept) or walks off the floor,
/// doing nothing either way.
pub fn coll0_of(b: &Beast, r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        fall_keep_hits(r, gobj, b.on_landed);
    } else {
        ground_or_fall(r, gobj, b.on_landed);
    }
    false
}

/// Entei's `on_accessory`, `it_802CF6C8`.
#[doc(alias = "it_802CF6C8")]
pub fn fire_when_told(r: &mut Running, gobj: GObjId) {
    fire_when_told_of(&ENTEI, r, gobj);
}

/// A beast's `on_accessory`: when its script says, its effect on its model and its sound, and
/// it starts roaring.
pub fn fire_when_told_of(b: &Beast, r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        return;
    }
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB4_itcmd_var2 = 1;
    ef::spawn(r, b.effect, Some(gobj), &[Arg::JObj(j), Arg::Float(1.0)]);
    play_sfx(r, gobj, b.cry, 0x7F, 0x40);
}

/// Into the air out of the ball, `it_802CF744` (`Item_EnterAirStateWithHitlag`, state 1).
#[doc(alias = "it_802CF744")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itEntei_UnkMotion1_Anim`.
#[doc(alias = "itEntei_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    anim1_of(r, gobj)
}

/// A beast growing out of the ball.
pub fn anim1_of(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itEntei_UnkMotion1_Phys`.
#[doc(alias = "itEntei_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    phys1_of(r, gobj);
}

/// A beast falling out of the ball.
pub fn phys1_of(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itEntei_UnkMotion1_Coll`.
#[doc(alias = "itEntei_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    coll1_of(&ENTEI, r, gobj)
}

/// A beast lands out of the ball.
pub fn coll1_of(b: &Beast, r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, b.on_out_landed)
}
