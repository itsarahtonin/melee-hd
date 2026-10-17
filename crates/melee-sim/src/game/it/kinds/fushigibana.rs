//! Venusaur (`It_PKind_Fushigibana`), `melee/it/kinds/itfushigibana.c`: out of the ball it
//! lands and stands, its script's hitboxes its Solar Beam, roaring now and then, until its life
//! runs out; off a ledge it falls and stands again where it lands.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, effect_hitlag, enter_first_state};
use super::super::{item_mut, lib26b1, lib2725, maplib, set_flag};
use super::drmariopill::set_pose;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802D_6FD0;
pub const ANIM0: u32 = 0x802D_7120;
pub const PHYS0: u32 = 0x802D_7144;
pub const COLL0: u32 = 0x802D_7164;
pub const ANIM1: u32 = 0x802D_71E8;
pub const PHYS1: u32 = 0x802D_72E4;
pub const COLL1: u32 = 0x802D_72E8;
pub const ANIM2: u32 = 0x802D_736C;
pub const PHYS2: u32 = 0x802D_7374;
pub const COLL2: u32 = 0x802D_73A4;

/// Its variables past the release's (`itFushigibana_ItemVars`): the flag byte whose top bit
/// says it's standing (`x60.x0.b0`), and the frames to its next roar (`x64`).
const FLAGS: usize = 0x60;
const STANDING: u8 = 0x80;
const ROAR_FRAMES: usize = 0x64;

/// Runs one of Venusaur's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => false,
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Venusaur's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => {}
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// `itFushigibana_Logic29_Spawned`: at its first size, its sound and life, not standing yet,
/// its roar's frames, into the air out of the ball.
#[doc(alias = "itFushigibana_Logic29_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    play_sfx(r, gobj, 0x272F, 0x7F, 0x40);
    let life = attr_f32(r, gobj, 4);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    pokemon::vars(r.game, gobj).set_bit(FLAGS, STANDING, false);
    let roar = attr_f32(r, gobj, 8);
    pokemon::vars(r.game, gobj).set_f32(ROAR_FRAMES, roar);
    enter_air(r, gobj);
}

/// Lands and stands, `it_802D705C`: still, `xDCE`'s last flag off.
#[doc(alias = "it_802D705C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
    lib2725::clear_velocity(r.game, gobj);
    stand(r, gobj);
}

/// Into the air out of the ball, `it_802D70A4`: its first state at its first pose, no longer
/// kept clear of.
#[doc(alias = "it_802D70A4")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// `itFushigibana_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itFushigibana_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itFushigibana_UnkMotion0_Phys`: falling out of the ball.
#[doc(alias = "itFushigibana_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itFushigibana_UnkMotion0_Coll`: lands and stands.
#[doc(alias = "itFushigibana_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Stands, `it_802D718C` (and `itFushigibana_UnkMotion1_Anim_inline1`).
#[doc(alias = "it_802D718C")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    pokemon::vars(r.game, gobj).set_bit(FLAGS, STANDING, true);
}

/// `itFushigibana_UnkMotion1_Anim`: standing, its animation over and over, a random roar every
/// so often, gone when its life runs out.
#[doc(alias = "itFushigibana_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        stand(r, gobj);
    }
    if reg(pokemon::vars(r.game, gobj).f32(ROAR_FRAMES)) == 0.0 {
        let roar = attr_f32(r, gobj, 8);
        pokemon::vars(r.game, gobj).set_f32(ROAR_FRAMES, roar);
        lib2725::play_random_sfx(r, gobj, 4, 0x2730);
    }
    let v = pokemon::vars(r.game, gobj);
    let left = v.f32(ROAR_FRAMES);
    v.set_f32(ROAR_FRAMES, mem(fp::fsubs(reg(left), 1.0)));
    // `itFushigibana_UnkMotion1_Anim_inline2`.
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// `itFushigibana_UnkMotion1_Phys`: nothing.
#[doc(alias = "itFushigibana_UnkMotion1_Phys")]
pub fn phys1(_: &mut Running, _: GObjId) {}

/// `itFushigibana_UnkMotion1_Coll`: falls off the floor, tilted to the floor.
#[doc(alias = "itFushigibana_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    false
}

/// Falls off the floor, `it_802D7328`.
#[doc(alias = "it_802D7328")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    set_pose(r, gobj, 0, 0.0);
}

/// `itFushigibana_UnkMotion2_Anim`: nothing.
#[doc(alias = "itFushigibana_UnkMotion2_Anim")]
pub fn anim2(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itFushigibana_UnkMotion2_Phys`: falling by its gravity.
#[doc(alias = "itFushigibana_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itFushigibana_UnkMotion2_Coll`: lands and stands.
#[doc(alias = "itFushigibana_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}
