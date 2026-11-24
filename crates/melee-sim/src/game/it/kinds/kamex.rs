//! Blastoise (`It_PKind_Kamex`), `melee/it/kinds/itkamex.c`: out of the ball it lands facing
//! its owner's nearest opponent and fires Hydro Pump a number of times, each shot (its script
//! says when) a water item from alternating cannons and a push back, then leaves. The way out,
//! the rounds and the water's launch are [`shooter`]'s, over [`BLASTOISE`] and [`WATER`].
//!
//! The unit also holds the water's code (`It_Kind_Kamex_HydroPump`): it grows from a fifth of
//! its size, flying straight until its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1};
use super::shooter::{self, ROUNDS, Release, Rounds, Shot, ShotSound};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_A49C;
pub const LANDED_NOTHING: u32 = 0x802C_A534;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_A538;
pub const ANIM0: u32 = 0x802C_A5D8;
pub const PHYS0: u32 = 0x802C_A618;
pub const COLL0: u32 = 0x802C_A654;
pub const ANIM1: u32 = 0x802C_A724;
pub const PHYS1: u32 = 0x802C_A7FC;
pub const COLL1: u32 = 0x802C_A890;
pub const FIRE_ACCESSORY: u32 = 0x802C_A8DC;
pub const ANIM2: u32 = 0x802C_A984;
pub const PHYS2: u32 = 0x802C_A9B8;
pub const COLL2: u32 = 0x802C_A9F4;
pub const ANIM3: u32 = 0x802C_AAA4;
pub const PHYS3: u32 = 0x802C_AAC8;
pub const COLL3: u32 = 0x802C_AAE8;
pub const WATER_SPAWNED: u32 = 0x802C_ACD0;
pub const WATER_HIT_SHIELD: u32 = 0x802C_ADC0;
pub const WATER_DMG_DEALT: u32 = 0x802C_ADC8;
#[doc(alias("WATER_EVT_UNK"))]
pub const WATER_FORGET: u32 = 0x802C_ADD0;
pub const WATER_ANIM: u32 = 0x802C_AE60;
pub const WATER_PHYS: u32 = 0x802C_AE94;
pub const WATER_COLL: u32 = 0x802C_AFB4;

/// Its variables past the shots left (`itPokemon_ItemVars`' `timer`, [`ROUNDS`]): which cannon
/// fires next (`x64`), the push back's speed (`x68`) and its slowing (`x6C`).
const CANNON: usize = 0x64;
const PUSH: usize = 0x68;
const SLOWING: usize = 0x6C;

/// Blastoise's way out (state 3), its stand (0), its rounds (1) and leaving (2), stopping
/// after the last round.
pub const BLASTOISE: Rounds = Rounds {
    release: Release {
        out: 3,
        hitlag: true,
        posed: false,
        landed,
        fallen: None,
    },
    stand: Some(0),
    fire: 1,
    leave: 2,
    accessory: FIRE_ACCESSORY,
    first_round,
    on_round: None,
    each_frame: None,
    stops: true,
};

/// Its water: `x8` ahead, `xC` up, from the cannon whose turn it is (`x10` aside), at `x14`,
/// with a sound panned its way.
pub const WATER: Shot = Shot {
    kind: ItKind::BlastoiseHydroPump,
    ahead: 8,
    up: 0xC,
    speed: 0x14,
    flagged: true,
    depth: Some(cannon),
    sound: ShotSound::Panned {
        right: 0x271D,
        left: 0x271C,
    },
};

/// Runs one of Blastoise's (or its water's) predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => shooter::standing(r, gobj, &BLASTOISE),
        COLL0 | COLL1 | COLL2 => shooter::stays(r, gobj),
        ANIM1 => shooter::rounds(r, gobj, &BLASTOISE),
        ANIM2 => shooter::leaving(r, gobj, &BLASTOISE),
        ANIM3 => shooter::growing(r, gobj),
        COLL3 => shooter::landing(r, gobj, &BLASTOISE.release),
        WATER_HIT_SHIELD => shooter::gone(r, gobj),
        WATER_DMG_DEALT => shooter::kept(r, gobj),
        WATER_ANIM => shooter::life_spent(r, gobj),
        WATER_COLL => shooter::gone_on_touch(r, gobj),
        _ => return None,
    })
}

/// Runs one of Blastoise's (or its water's) events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => shooter::nothing(r, gobj),
        PHYS0 | PHYS2 => shooter::gravity_in_air(r, gobj),
        PHYS1 => phys1(r, gobj),
        FIRE_ACCESSORY => shooter::fire_when_told(r, gobj, shoot, Some(push_back)),
        PHYS3 => shooter::falling(r, gobj, &BLASTOISE.release),
        WATER_SPAWNED => shooter::shot_out(r, gobj, water_fly, small),
        WATER_PHYS => water_phys(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802CA49C`, its spawn: facing its owner's nearest opponent, no shots yet, its script's
/// flag clear, at its first size, into the air out of the ball, its sound.
#[doc(alias = "it_802CA49C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    pokemon::face_nearest_opponent(r, gobj);
    pokemon::vars(r.game, gobj).set_i32(ROUNDS, -1);
    item_mut(r.game, gobj).cmd_flags[0] &= !0x80;
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(CANNON, 0);
    v.set_f32(PUSH, 0.0);
    v.set_f32(SLOWING, 0.0);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    shooter::enter_air(r, gobj, &BLASTOISE.release);
    super::super::item::play_sfx(r, gobj, 0x271E, 0x7F, 0x40);
}

/// Forgets a GObj going away, `it_802CA538` and the water's `itKamex_Logic31_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802CA538")]
#[doc(alias = "itKamex_Logic31_EvtUnk")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball ([`shooter::landed`]).
fn landed(r: &mut Running, gobj: GObjId) {
    shooter::landed(r, gobj, &BLASTOISE);
}

/// Shoots water ([`shooter::shoot`]).
fn shoot(r: &mut Running, gobj: GObjId) {
    shooter::shoot(r, gobj, &WATER);
}

/// The first round's settings (`it_802CA6A0`'s): its shots (`x4`), its push back (`x18`) and
/// the push's slowing (`x1C`).
fn first_round(r: &mut Running, gobj: GObjId) {
    let (shots, push, slowing) = (
        attr_i32(r, gobj, 4),
        attr_f32(r, gobj, 0x18),
        attr_f32(r, gobj, 0x1C),
    );
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(ROUNDS, shots);
    v.set_f32(PUSH, push);
    v.set_f32(SLOWING, slowing);
}

/// The water's depth from the cannon whose turn it is (`x10` aside, behind for the second),
/// the other cannon's turn next (`it_802CAB10`'s).
fn cannon(r: &mut Running, gobj: GObjId, facing: f64, z: f32) -> f32 {
    let side = attr_f32(r, gobj, 0x10);
    let v = pokemon::vars(r.game, gobj);
    let c = v.i32(CANNON);
    v.set_i32(CANNON, c ^ 1);
    if c != 0 {
        mem(fp::fmadds(-1.0, fp::fmuls(reg(side), facing), reg(z)))
    } else {
        mem(fp::fmadds(reg(side), facing, reg(z)))
    }
}

/// Pushed back by its shot, against its facing (`it_802CA8DC`'s).
fn push_back(r: &mut Running, gobj: GObjId) {
    let push = pokemon::vars(r.game, gobj).f32(PUSH);
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fmuls(reg(push), fp::fneg(reg(ip.facing_dir))));
}

/// `itKamex_UnkMotion1_Phys`: the push back slows against its facing, and it falls by its
/// gravity in the air.
#[doc(alias = "itKamex_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (vx, facing) = (reg(ip.vel.x), reg(ip.facing_dir));
    if vx != 0.0 && ((vx > 0.0 && facing <= 0.0) || (vx <= 0.0 && facing > 0.0)) {
        let slowing = pokemon::vars(r.game, gobj).f32(SLOWING);
        let ip = item_mut(r.game, gobj);
        ip.vel.x = mem(fp::fmadds(reg(slowing), facing, reg(ip.vel.x)));
    }
    shooter::gravity_in_air(r, gobj);
}

/// The water at a fifth of its size as it comes out.
fn small(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("the water without a model");
    let s = 0.2f32;
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// The water flies, `it_802CADF0`: its splash effect at its size.
#[doc(alias = "it_802CADF0")]
pub fn water_fly(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, 0x46A, gobj, j, 1.2);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// The water's `it_802CAE94`: growing to full size, by 0.06 a frame in double precision.
#[doc(alias = "it_802CAE94")]
pub fn water_phys(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("the water without a model");
    let mut s = r.game.objs.jobjs[j].scale;
    if reg(s.x) < 1.0 {
        let grow = |c: f32| mem(fp::frsp(fp::fadd(reg(c), 0.06)));
        s = Vec3::new(grow(s.x), grow(s.y), grow(s.z));
        jobj::set_scale(&mut r.game.objs.jobjs, j, s);
    }
}
