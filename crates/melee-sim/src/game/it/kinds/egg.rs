//! The egg, `melee/it/kinds/itegg.c`: it falls and lies about; a hit, a throw's landing or
//! a shield breaks it, which explodes one time in its attributes' `x4`, else drops its
//! contents (or coins) and vanishes.

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{drop, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use super::falling_phys;
use crate::game::ef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_8D98;
pub const LANDED: u32 = 0x8028_8E6C;
pub const ANIM0: u32 = 0x8028_8EC4;
pub const PHYS0: u32 = 0x8028_8ECC;
pub const COLL0: u32 = 0x8028_8ED0;
pub const FELL: u32 = 0x8028_8EFC;
pub const ANIM1: u32 = 0x8028_8F24;
pub const PHYS1: u32 = 0x8028_8F2C;
pub const COLL1: u32 = 0x8028_8F5C;
pub const PICKED_UP: u32 = 0x8028_8F88;
pub const ANIM2: u32 = 0x8028_8FB0;
pub const PHYS2: u32 = 0x8028_8FB8;
pub const DROPPED: u32 = 0x8028_8FBC;
pub const THROWN: u32 = 0x8028_8FDC;
pub const PHYS3: u32 = 0x8028_9004;
pub const COLL3: u32 = 0x8028_9050;
pub const ANIM5: u32 = 0x8028_912C;
pub const PHYS5: u32 = 0x8028_914C;
pub const COLL5: u32 = 0x8028_9150;
pub const ANIM6: u32 = 0x8028_91E0;
pub const PHYS6: u32 = 0x8028_920C;
pub const COLL6: u32 = 0x8028_9210;
pub const DMG_DEALT: u32 = 0x8028_9218;
pub const CLANKED: u32 = 0x8028_93D4;
pub const HIT_SHIELD: u32 = 0x8028_9470;
pub const REFLECTED: u32 = 0x8028_950C;
pub const DMG_RECEIVED: u32 = 0x8028_95A8;
pub const ENTERED_AIR: u32 = 0x8028_9644;
pub const ANIM4: u32 = 0x8028_966C;
pub const PHYS4: u32 = 0x8028_9674;
pub const COLL4: u32 = 0x8028_9678;

/// Runs one of the egg's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 | COLL5 | COLL6 => still(r, gobj),
        ANIM5 => exploding(r, gobj),
        ANIM6 => vanishing(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED | DMG_RECEIVED => broken(r, gobj),
        _ => return None,
    })
}

/// Runs one of the egg's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 | PHYS5 | PHYS6 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS3 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itEgg_Logic3_Spawned`: whole, falling.
#[doc(alias = "itEgg_Logic3_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDD4_itemVar.egg().x0.set(0);
    fell(r, gobj);
}

/// The egg's attributes (laid out as `itEgg_ItemVars`): how many items it holds, and the
/// chance (one in `x4`) it explodes.
fn attrs(r: &Running, gobj: GObjId) -> (i32, i32) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("an egg without attributes");
    (r.data.u32(a) as i32, r.data.u32(a.at(4)) as i32)
}

/// Breaks the egg, `it_80288DC4`: whether it explodes (one time in `x4`); else coins in a
/// coin match, or its contents.
#[doc(alias = "it_80288DC4")]
pub fn break_open(r: &mut Running, gobj: GObjId) -> bool {
    let (n, chance) = attrs(r, gobj);
    if r.game.objs.rng.below(chance) == 0 {
        return true;
    }
    let pos = item(r.game, gobj).pos;
    if drop::drop_coins(r, gobj, pos, Some(Vec3::new(0.0, 0.0, 0.0))) {
        return false;
    }
    drop::drop_contents(r, gobj, n, false);
    false
}

/// Stops and lies where it lands, `it_80288E6C`.
#[doc(alias = "it_80288E6C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itEgg_UnkMotion0_Anim` (2, 3 for 1 and 3, 4; and 5's
/// and 6's collision).
#[doc(alias = "itEgg_UnkMotion0_Anim")]
#[doc(alias = "itEgg_UnkMotion2_Anim")]
#[doc(alias = "itEgg_UnkMotion3_Anim")]
#[doc(alias = "itEgg_UnkMotion4_Anim")]
#[doc(alias = "itEgg_UnkMotion5_Coll")]
#[doc(alias = "itEgg_UnkMotion6_Coll")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itEgg_UnkMotion0_Phys` (2, 4, 5, 6).
#[doc(alias = "itEgg_UnkMotion0_Phys")]
#[doc(alias = "itEgg_UnkMotion2_Phys")]
#[doc(alias = "itEgg_UnkMotion4_Phys")]
#[doc(alias = "itEgg_UnkMotion5_Phys")]
#[doc(alias = "itEgg_UnkMotion6_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itEgg_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itEgg_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_80288EFC`.
#[doc(alias = "it_80288EFC")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity, `itEgg_UnkMotion1_Phys`.
#[doc(alias = "itEgg_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itEgg_UnkMotion1_Coll`: lands.
#[doc(alias = "itEgg_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Held, `itEgg_Logic3_PickedUp`.
#[doc(alias = "itEgg_Logic3_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, `itEgg_Logic3_Dropped` and `itEgg_Logic3_Thrown`.
#[doc(alias = "itEgg_Logic3_Dropped")]
#[doc(alias = "itEgg_Logic3_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itEgg_UnkMotion3_Coll`: thrown, it breaks on what it touches.
#[doc(alias = "itEgg_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        return broken(r, gobj);
    }
    false
}

/// Explodes, `it_80289094`: hidden, its blast's hitboxes out until its life runs out.
#[doc(alias = "it_80289094")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.egg();
    v.x0.set(1);
    v.rand_max.set(20);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// Exploding, until its life runs out, `itEgg_UnkMotion5_Anim`.
#[doc(alias = "itEgg_UnkMotion5_Anim")]
pub fn exploding(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Broken open, `it_80289158`: hidden, gone after 40 ticks.
#[doc(alias = "it_80289158")]
pub fn vanish(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.egg();
    v.x0.set(1);
    v.rand_max.set(40);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// `itEgg_UnkMotion6_Anim`: broken open, counting down to gone.
#[doc(alias = "itEgg_UnkMotion6_Anim")]
pub fn vanishing(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.egg();
    let left = v.rand_max.get().wrapping_sub(1);
    v.rand_max.set(left);
    left <= 0
}

/// Breaks once, `itEgg_Logic3_DmgDealt` (and the clank, shield, reflection and damage
/// callbacks, each the same code): it explodes, or cracks open with its sound and effect.
#[doc(alias = "itEgg_Logic3_DmgDealt")]
#[doc(alias = "itEgg_Logic3_Clanked")]
#[doc(alias = "itEgg_Logic3_HitShield")]
#[doc(alias = "itEgg_Logic3_Reflected")]
#[doc(alias = "itEgg_Logic3_DmgReceived")]
pub fn broken(r: &mut Running, gobj: GObjId) -> bool {
    if item_mut(r.game, gobj).xDD4_itemVar.egg().x0.get() != 0 {
        return false;
    }
    if break_open(r, gobj) {
        explode(r, gobj);
        return false;
    }
    let pos = item(r.game, gobj).pos;
    ef::spawn(r, 1232, Some(gobj), &[ef::Arg::Vec(pos)]);
    play_sfx(r, gobj, 244, 127, 64);
    vanish(r, gobj);
    false
}

/// Skidding off a slope, `itEgg_Logic3_EnteredAir`.
#[doc(alias = "itEgg_Logic3_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// `itEgg_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itEgg_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`), `itEgg_UnkMotion3_Phys`.
#[doc(alias = "itEgg_UnkMotion3_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}
