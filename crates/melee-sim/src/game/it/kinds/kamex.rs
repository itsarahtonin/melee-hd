//! Blastoise (`It_PKind_Kamex`), `melee/it/kinds/itkamex.c`: out of the ball it lands facing
//! its owner's nearest opponent and fires Hydro Pump a number of times, each shot (its script
//! says when) a water item from alternating cannons and a push back, then leaves.
//!
//! The unit also holds the water's code (`It_Kind_Kamex_HydroPump`): it grows from a fifth of
//! its size, flying straight until its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::ground::{air_touched_any, fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::sound_obj::SoundParams;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_A49C;
pub const LANDED_NOTHING: u32 = 0x802C_A534;
pub const EVT_UNK: u32 = 0x802C_A538;
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
pub const WATER_EVT_UNK: u32 = 0x802C_ADD0;
pub const WATER_ANIM: u32 = 0x802C_AE60;
pub const WATER_PHYS: u32 = 0x802C_AE94;
pub const WATER_COLL: u32 = 0x802C_AFB4;

/// `It_Kind_Kamex_HydroPump`.
pub const KAMEX_HYDRO_PUMP: i32 = 0xC0;

/// Its variables past the release's (`itPokemon_ItemVars`): the shots left (`timer`, -1 until
/// the first), which cannon fires next (`x64`), the push back's speed (`x68`) and its slowing
/// (`x6C`).
const SHOTS: usize = 0x60;
const CANNON: usize = 0x64;
const PUSH: usize = 0x68;
const SLOWING: usize = 0x6C;

/// Runs one of Blastoise's (or its water's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL1 | COLL2 => coll(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        WATER_HIT_SHIELD => true,
        WATER_DMG_DEALT => false,
        WATER_ANIM => water_anim(r, gobj),
        WATER_COLL => water_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Blastoise's (or its water's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => {}
        PHYS0 | PHYS2 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        FIRE_ACCESSORY => fire_when_told(r, gobj),
        PHYS3 => phys3(r, gobj),
        WATER_SPAWNED => water_spawned(r, gobj),
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
    pokemon::vars(r.game, gobj).set_i32(SHOTS, -1);
    item_mut(r.game, gobj).xDBC[0] &= !0x80;
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(CANNON, 0);
    v.set_f32(PUSH, 0.0);
    v.set_f32(SLOWING, 0.0);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    play_sfx(r, gobj, 0x271E, 0x7F, 0x40);
}

/// Landing in its states, `it_802CA534`: nothing.
#[doc(alias = "it_802CA534")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CA538` and the water's `itKamex_Logic31_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802CA538")]
#[doc(alias = "itKamex_Logic31_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `it_802CA558`: still.
#[doc(alias = "it_802CA558")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    ready(r, gobj);
}

/// Gets ready to fire, `it_802CA58C`.
#[doc(alias = "it_802CA58C")]
pub fn ready(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `it_802CA5D8`: fires when its animation ends.
#[doc(alias = "it_802CA5D8")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fire(r, gobj);
    }
    false
}

/// Falls by its gravity in the air (`it_802CA618`, `itKamex_UnkMotion2_Phys`).
#[doc(alias = "it_802CA618")]
#[doc(alias = "itKamex_UnkMotion2_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == 1 {
        super::gravity(r, gobj);
    }
}

/// Lands in the air, falls off the floor on it, doing nothing either way (`it_802CA654`,
/// `itKamex_UnkMotion1_Coll`, `itKamex_UnkMotion2_Coll`).
#[doc(alias = "it_802CA654")]
#[doc(alias = "itKamex_UnkMotion1_Coll")]
#[doc(alias = "itKamex_UnkMotion2_Coll")]
pub fn coll(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        fall(r, gobj, landed_nothing);
    } else {
        ground_or_fall(r, gobj, landed_nothing);
    }
    false
}

/// Fires, `it_802CA6A0` (and its inline copy in `itKamex_UnkMotion1_Anim`): the first time,
/// its shots and push back from its attributes; firing when its script says.
#[doc(alias = "it_802CA6A0")]
pub fn fire(r: &mut Running, gobj: GObjId) {
    if pokemon::vars(r.game, gobj).i32(SHOTS) == -1 {
        let (shots, push, slowing) = (
            attr_i32(r, gobj, 4),
            attr_f32(r, gobj, 0x18),
            attr_f32(r, gobj, 0x1C),
        );
        let v = pokemon::vars(r.game, gobj);
        v.set_i32(SHOTS, shots);
        v.set_f32(PUSH, push);
        v.set_f32(SLOWING, slowing);
    }
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FIRE_ACCESSORY));
}

/// `itKamex_UnkMotion1_Anim`: at each shot's end, fires again, or with none left stops and
/// leaves.
#[doc(alias = "itKamex_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let shots = v.i32(SHOTS);
        if shots <= 0 {
            v.set_i32(SHOTS, 0);
            item_mut(r.game, gobj).x40_vel.x = 0.0;
            leave(r, gobj);
        } else {
            v.set_i32(SHOTS, shots.wrapping_sub(1));
            fire(r, gobj);
        }
    }
    false
}

/// `itKamex_UnkMotion1_Phys`: the push back slows against its facing, and it falls by its
/// gravity in the air.
#[doc(alias = "itKamex_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (vx, facing) = (reg(ip.x40_vel.x), reg(ip.facing_dir));
    if vx != 0.0 && ((vx > 0.0 && facing <= 0.0) || (vx <= 0.0 && facing > 0.0)) {
        let slowing = pokemon::vars(r.game, gobj).f32(SLOWING);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmadds(reg(slowing), facing, reg(ip.x40_vel.x)));
    }
    phys0(r, gobj);
}

/// Its `on_accessory`, `it_802CA8DC`: fires a shot when its script says, pushed back.
#[doc(alias = "it_802CA8DC")]
pub fn fire_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDBC[0] & 0x80 != 0 {
        shoot(r, gobj);
        let push = pokemon::vars(r.game, gobj).f32(PUSH);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(push), fp::fneg(reg(ip.facing_dir))));
        ip.xDBC[0] &= !0x80;
    }
}

/// Leaves, `it_802CA938`.
#[doc(alias = "it_802CA938")]
pub fn leave(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itKamex_UnkMotion2_Anim`: gone when its animation ends.
#[doc(alias = "itKamex_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// Into the air out of the ball, `it_802CAA40` (`Item_EnterAirStateWithHitlag`, state 3).
#[doc(alias = "it_802CAA40")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itKamex_UnkMotion3_Anim`: growing out of the ball.
#[doc(alias = "itKamex_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itKamex_UnkMotion3_Phys`: falling out of the ball.
#[doc(alias = "itKamex_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itKamex_UnkMotion3_Coll`: lands out of the ball.
#[doc(alias = "itKamex_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Shoots water, `it_802CAB10`: ahead of and above it, from the cannon whose turn it is (the
/// other next), flying its way, with a sound panned its way.
#[doc(alias = "it_802CAB10")]
pub fn shoot(r: &mut Running, gobj: GObjId) {
    let (ahead, up, side, speed) = (
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 0xC),
        attr_f32(r, gobj, 0x10),
        attr_f32(r, gobj, 0x14),
    );
    let ip = item(r.game, gobj);
    let (pos, facing, owner) = (ip.pos, reg(ip.facing_dir), ip.owner);
    let mut prev = pos;
    prev.x = mem(fp::fmadds(reg(ahead), facing, reg(prev.x)));
    prev.y = mem(fp::fadds(reg(prev.y), reg(up)));
    prev.z = if pokemon::vars(r.game, gobj).i32(CANNON) != 0 {
        mem(fp::fmadds(-1.0, fp::fmuls(reg(side), facing), reg(prev.z)))
    } else {
        mem(fp::fmadds(reg(side), facing, reg(prev.z)))
    };
    let mut spawn = SpawnItem {
        kind: KAMEX_HYDRO_PUMP,
        ..Default::default()
    };
    spawn.prev_pos = prev;
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.facing_dir = mem(facing);
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(mem(fp::fmuls(reg(speed), facing)), 0.0, 0.0);
    let v = pokemon::vars(r.game, gobj);
    let cannon = v.i32(CANNON);
    v.set_i32(CANNON, cannon ^ 1);
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    if spawn_air(r, &mut spawn).is_some() {
        let sfx = if facing == 1.0 { 0x271D } else { 0x271C };
        r.lb_create_sound_object(&SoundParams {
            owner: gobj,
            behavior: 1,
            sfx_id: sfx,
            start_vol: 0x7F,
            end_vol: 0x7F,
            pan_left: 0,
            pan_right: 0x7F,
            end_frame: 0x3C,
            direction: 3.0,
            track: 0,
            voice: -1,
        });
    }
}

/// The water's `it_2725_Logic31_Spawned`: its life, not spinning, flying, not pickable, at a
/// fifth of its size.
#[doc(alias = "it_2725_Logic31_Spawned")]
pub fn water_spawned(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("the water without a model");
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    water_fly(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    let s = 0.2f32;
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// The water's `itKamex_Logic31_HitShield`: gone.
#[doc(alias = "itKamex_Logic31_HitShield")]
pub fn water_hit_shield(_: &mut Running, _: GObjId) -> bool {
    true
}

/// The water's `itKamex_Logic31_DmgDealt`: kept.
#[doc(alias = "itKamex_Logic31_DmgDealt")]
pub fn water_dmg_dealt(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The water flies, `it_802CADF0`: its splash effect at its size.
#[doc(alias = "it_802CADF0")]
pub fn water_fly(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, 0x46A, gobj, j, 1.2);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// The water's `it_802CAE60`: gone when its life runs out.
#[doc(alias = "it_802CAE60")]
pub fn water_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
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

/// The water's `it_802CAFB4`: gone when it touches the stage.
#[doc(alias = "it_802CAFB4")]
pub fn water_coll(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}
