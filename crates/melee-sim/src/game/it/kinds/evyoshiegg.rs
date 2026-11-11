//! The event Yoshi egg, `melee/it/kinds/itevyoshiegg.c`: Event 13's egg, which the event's
//! process drops at the start (`gm_801BD028`) and the player keeps whole to the time's end.
//! It falls and lies about like the egg (`itegg.c`, whose code it copies); a hit bounces it
//! up, and damage past its attributes' `x0` breaks it with its sound and two effects, which
//! fails the event (`gm_801BEB68`) as it vanishes.
//!
//! Its variables (`itEvYoshiEgg_ItemVars`: whether it broke, the countdown to gone) are the
//! egg's first two words, so it shares the egg's member of the union (`x0`, `rand_max`).

use super::super::ground::{air, fall_unbreakable, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, play_sfx, spawn_air};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725};
use super::falling_phys;
use crate::game::ef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x8029_B188;
pub const SPAWNED: u32 = 0x8029_B1AC;
pub const LANDED: u32 = 0x8029_B1D8;
pub const ANIM0: u32 = 0x8029_B230;
pub const PHYS0: u32 = 0x8029_B238;
pub const COLL0: u32 = 0x8029_B23C;
pub const FELL: u32 = 0x8029_B268;
pub const ANIM1: u32 = 0x8029_B290;
pub const PHYS1: u32 = 0x8029_B298;
pub const COLL1: u32 = 0x8029_B2C8;
pub const PICKED_UP: u32 = 0x8029_B2F4;
pub const ANIM2: u32 = 0x8029_B31C;
pub const PHYS2: u32 = 0x8029_B324;
pub const DROPPED: u32 = 0x8029_B328;
pub const THROWN: u32 = 0x8029_B348;
pub const PHYS3: u32 = 0x8029_B370;
pub const COLL3: u32 = 0x8029_B3BC;
pub const ANIM5: u32 = 0x8029_B400;
pub const PHYS5: u32 = 0x8029_B460;
pub const COLL5: u32 = 0x8029_B464;
pub const DMG_DEALT: u32 = 0x8029_B46C;
pub const CLANKED: u32 = 0x8029_B4AC;
pub const HIT_SHIELD: u32 = 0x8029_B4EC;
pub const REFLECTED: u32 = 0x8029_B52C;
pub const DMG_RECEIVED: u32 = 0x8029_B56C;
pub const ENTERED_AIR: u32 = 0x8029_B670;
pub const ANIM4: u32 = 0x8029_B698;
pub const PHYS4: u32 = 0x8029_B6A0;
pub const COLL4: u32 = 0x8029_B6A4;

/// Runs one of the egg's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 | COLL5 => still(r, gobj),
        ANIM5 => vanishing(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => bounced(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of the egg's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 | PHYS5 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS3 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// Drops the event's egg at `pos`, `itEvYoshiEgg_Spawn`: no owner, facing right, still, with
/// a first collision; falling, whole.
#[doc(alias = "itEvYoshiEgg_Spawn")]
pub fn spawn(r: &mut Running, pos: Vec3) {
    let mut spawn = SpawnItem {
        kind: super::EV_YOSHI_EGG,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = pos;
    // `Item_InitSpawnCommonFields(&spawn, NULL, 1.0F, true)`.
    spawn.facing_dir = 1.0;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    if let Some(gobj) = spawn_air(r, &mut spawn) {
        fell(r, gobj);
        item_mut(r.game, gobj).xDD4_itemVar.egg().x0.set(0);
    }
}

/// `itEvYoshiEgg_Logic42_Destroyed`: the event failed (`gm_801BEB68(1)`).
#[doc(alias = "itEvYoshiEgg_Logic42_Destroyed")]
pub fn destroyed(r: &mut Running, _: GObjId) {
    r.game.game_data.vs.unk_530.x18 = 1;
}

/// `itEvYoshiEgg_Logic42_Spawned`: whole, falling.
#[doc(alias = "itEvYoshiEgg_Logic42_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDD4_itemVar.egg().x0.set(0);
    fell(r, gobj);
}

/// Stops and lies where it lands, `it_8029B1D8`.
#[doc(alias = "it_8029B1D8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itEvyoshiegg_UnkMotion0_Anim` (2 for 1 and 3, 4; and
/// 5's collision).
#[doc(alias = "itEvyoshiegg_UnkMotion0_Anim")]
#[doc(alias = "itEvyoshiegg_UnkMotion2_Anim")]
#[doc(alias = "itEvyoshiegg_UnkMotion3_Anim")]
#[doc(alias = "itEvyoshiegg_UnkMotion4_Anim")]
#[doc(alias = "itEvyoshiegg_UnkMotion5_Coll")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itEvyoshiegg_UnkMotion0_Phys` (2, 4, 5).
#[doc(alias = "itEvyoshiegg_UnkMotion0_Phys")]
#[doc(alias = "itEvyoshiegg_UnkMotion2_Phys")]
#[doc(alias = "itEvyoshiegg_UnkMotion4_Phys")]
#[doc(alias = "itEvyoshiegg_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itEvyoshiegg_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itEvyoshiegg_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029B268`.
#[doc(alias = "it_8029B268")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itEvyoshiegg_UnkMotion1_Phys`.
#[doc(alias = "itEvyoshiegg_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itEvyoshiegg_UnkMotion1_Coll`: lands, never breaking on it as the egg can (it calls
/// `it_8026E32C` where `itegg.c` calls `it_8026E15C`, so no landing draws a number).
#[doc(alias = "itEvyoshiegg_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_unbreakable(r, gobj, landed);
    false
}

/// Held, `itEvYoshiEgg_Logic42_PickedUp`.
#[doc(alias = "itEvYoshiEgg_Logic42_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, `itEvYoshiEgg_Logic42_Dropped` and `itEvYoshiEgg_Logic42_Thrown`.
#[doc(alias = "itEvYoshiEgg_Logic42_Dropped")]
#[doc(alias = "itEvYoshiEgg_Logic42_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`),
/// `itEvyoshiegg_UnkMotion3_Phys`.
#[doc(alias = "itEvyoshiegg_UnkMotion3_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// `itEvyoshiegg_UnkMotion3_Coll`: thrown, it bounces off what it touches.
#[doc(alias = "itEvyoshiegg_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        return bounced(r, gobj);
    }
    false
}

/// Broken, counting down to gone: the event failed 20 ticks in (`gm_801BEB68(1)` at 40 of
/// its 60), `itEvyoshiegg_UnkMotion5_Anim`.
#[doc(alias = "itEvyoshiegg_UnkMotion5_Anim")]
#[doc(alias = "gm_801BEB68")]
pub fn vanishing(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj).xDD4_itemVar.egg().rand_max.get() == 40 {
        r.game.game_data.vs.unk_530.x18 = 1;
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.egg();
    let left = v.rand_max.get().wrapping_sub(1);
    v.rand_max.set(left);
    left <= 0
}

/// Bounces up out of a hit, `it_3F14_Logic42_DmgDealt` (and the clank, shield and
/// reflection callbacks, each the same code).
#[doc(alias = "it_3F14_Logic42_DmgDealt")]
#[doc(alias = "it_3F14_Logic42_Clanked")]
// Ports `it_3F14_Logic42_HitShield`; aliased once a check reaches it.
// Ports `it_3F14_Logic42_Reflected`; aliased once a check reaches it.
pub fn bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 1.5;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    false
}

/// `itEvYoshiEgg_Logic42_DmgReceived`: once its damage reaches its attributes' `x0`, it
/// breaks: two of its effects, its sound, hidden and blinking, still, no longer picked up,
/// gone 60 ticks later.
#[doc(alias = "itEvYoshiEgg_Logic42_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj).xDD4_itemVar.egg().x0.get() != 0 {
        return false;
    }
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("the event egg without attributes");
    let ip = item(r.game, gobj);
    if ip.xC9C < r.data.u32(a) as i32 {
        return false;
    }
    let pos = ip.pos;
    for _ in 0..2 {
        ef::spawn(
            r,
            1231,
            Some(gobj),
            &[ef::Arg::Vec(pos), ef::Arg::Float(1.5)],
        );
    }
    play_sfx(r, gobj, 244, 127, 64);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    let v = ip.xDD4_itemVar.egg();
    v.x0.set(1);
    v.rand_max.set(60);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    false
}

/// Skidding off a slope, `itEvYoshiEgg_Logic42_EnteredAir`.
#[doc(alias = "itEvYoshiEgg_Logic42_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itEvyoshiegg_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itEvyoshiegg_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}
