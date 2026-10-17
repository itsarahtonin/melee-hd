//! Weezing (`It_PKind_Matadogas`), `melee/it/kinds/itmatadogas.c`: out of the ball it floats
//! where it stops, its body a hitbox, puffing poison gas every few frames once its script says,
//! until its animation ends.
//!
//! The unit also holds the gas's code (`It_Kind_Matadogas_Gas1` and `_Gas2`): each puff
//! drifts out, slowing, until its life runs out.

use gekko_fp as fp;

use super::super::ground::fall;
use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib, set_flag};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_AFD4;
pub const LANDED_NOTHING: u32 = 0x802C_B0F4;
pub const EVT_UNK: u32 = 0x802C_B0F8;
pub const ANIM0: u32 = 0x802C_B118;
pub const PHYS0: u32 = 0x802C_B14C;
pub const COLL0: u32 = 0x802C_B150;
pub const ANIM1: u32 = 0x802C_B17C;
pub const PHYS1: u32 = 0x802C_B280;
pub const COLL1: u32 = 0x802C_B284;
pub const GAS_ACCESSORY: u32 = 0x802C_B2B0;
pub const ANIM2: u32 = 0x802C_B3B4;
pub const PHYS2: u32 = 0x802C_B3D8;
pub const COLL2: u32 = 0x802C_B4C8;
pub const GAS1_SPAWNED: u32 = 0x802C_B690;
pub const GAS2_SPAWNED: u32 = 0x802C_B704;
pub const GAS_EVT_UNK: u32 = 0x802C_B778;
pub const GAS_ANIM: u32 = 0x802C_B810;
pub const GAS_PHYS: u32 = 0x802C_B844;
pub const GAS_COLL: u32 = 0x802C_B8A4;

/// `It_Kind_Matadogas_Gas1` and `_Gas2`.
pub const MATADOGAS_GAS1: i32 = 0xC1;
pub const MATADOGAS_GAS2: i32 = 0xC2;

/// Its variables past the release's (`itMatadogas_ItemVars`): the frames to its next puff
/// (`x60`), and whether the next puff is silent (`x64`, flipped each puff).
const PUFF_FRAMES: usize = 0x60;
const SOUND_TURN: usize = 0x64;

/// Runs one of Weezing's (or its gas's) predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL1 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        GAS_ANIM => gas_anim(r, gobj),
        GAS_COLL => false,
        _ => return None,
    })
}

/// Runs one of Weezing's (or its gas's) events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING | PHYS0 | PHYS1 => {}
        GAS_ACCESSORY => puff_when_told(r, gobj),
        PHYS2 => phys2(r, gobj),
        GAS1_SPAWNED => gas1_spawned(r, gobj),
        GAS2_SPAWNED => gas2_spawned(r, gobj),
        GAS_PHYS => gas_phys(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802CAFD4`, its spawn: its script's flag clear, its puff's frames, at its first size,
/// into the air out of the ball, unturned, its sound.
#[doc(alias = "it_802CAFD4")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("Weezing without a model");
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    let frames = attr_f32(r, gobj, 4);
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(PUFF_FRAMES, fp::fctiwz(reg(frames)));
    v.set_i32(SOUND_TURN, 0);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, 0.0);
    play_sfx(r, gobj, 0x2715, 0x7F, 0x40);
}

/// Landing in its states, `it_802CB0F4`: nothing.
#[doc(alias = "it_802CB0F4")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CB0F8` and the gas's `itMatadogas_Logic33_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "it_802CB0F8")]
#[doc(alias = "itMatadogas_Logic33_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `it_802CB118`: gone when its animation ends.
#[doc(alias = "it_802CB118")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// `it_802CB14C` and `itMatadogas_UnkMotion1_Phys`: nothing.
#[doc(alias = "it_802CB14C")]
#[doc(alias = "itMatadogas_UnkMotion1_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// `it_802CB150` and `itMatadogas_UnkMotion1_Coll`: lands doing nothing.
#[doc(alias = "it_802CB150")]
#[doc(alias = "itMatadogas_UnkMotion1_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed_nothing);
    false
}

/// `itMatadogas_UnkMotion1_Anim`: at its animation's end, into state 0, unturned.
#[doc(alias = "itMatadogas_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let j = jobj_of(r.game, gobj).expect("Weezing without a model");
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, 0.0);
    }
    false
}

/// Its `on_accessory`, `it_802CB2B0`: once its script says, a puff every few frames, of
/// either gas at even odds.
#[doc(alias = "it_802CB2B0")]
pub fn puff_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        return;
    }
    let v = pokemon::vars(r.game, gobj);
    let n = v.i32(PUFF_FRAMES).wrapping_sub(1);
    v.set_i32(PUFF_FRAMES, n);
    if n != 0 {
        return;
    }
    let frames = attr_f32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(PUFF_FRAMES, fp::fctiwz(reg(frames)));
    if r.game.objs.rng.below(2) == 0 {
        let speed = attr_f32(r, gobj, 8);
        puff(r, gobj, MATADOGAS_GAS1, speed);
    } else {
        let speed = attr_f32(r, gobj, 0xC);
        puff(r, gobj, MATADOGAS_GAS2, speed);
    }
}

/// Into the air out of the ball, `it_802CB350` (`Item_EnterAirStateWithHitlag`, state 2).
#[doc(alias = "it_802CB350")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itMatadogas_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itMatadogas_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itMatadogas_UnkMotion2_Phys`: out of the ball, once its frames are up it stops, unturned,
/// and puffs when its script says.
#[doc(alias = "itMatadogas_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if !pokemon::fall(r, gobj) {
        return;
    }
    lib2725::clear_velocity(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("Weezing without a model");
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, 0.0);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(GAS_ACCESSORY));
    set_flag(&mut ip.xDD1_flag, 1, true);
}

/// `itMatadogas_UnkMotion2_Coll`: lands out of the ball, doing nothing.
#[doc(alias = "itMatadogas_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}

/// Puffs gas of `kind`, `it_802CB4F0`: from it, at `speed` a random whole degree round; every
/// other puff makes one of three sounds.
#[doc(alias = "it_802CB4F0")]
pub fn puff(r: &mut Running, gobj: GObjId, kind: i32, speed: f32) {
    let deg = fp::frsp(f64::from(r.game.objs.rng.below(360)));
    let ip = item(r.game, gobj);
    let (pos, facing, owner) = (ip.pos, ip.facing_dir, ip.owner);
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    let rad = mem(fp::fmuls(reg(0.017453292f32), deg));
    let vx = mem(fp::fmuls(reg(speed), reg(crate::math::cos(rad))));
    let vy = mem(fp::fmuls(reg(speed), reg(crate::math::sin(rad))));
    spawn.vel = Vec3::new(vx, vy, 0.0);
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    if spawn_air(r, &mut spawn).is_some() {
        let v = pokemon::vars(r.game, gobj);
        let turn = v.i32(SOUND_TURN) ^ 1;
        v.set_i32(SOUND_TURN, turn);
        if turn != 0 {
            let sfx = match r.game.objs.rng.below(3) {
                0 => Some(0x2712),
                1 => Some(0x2713),
                2 => Some(0x2714),
                _ => None,
            };
            if let Some(sfx) = sfx {
                play_sfx(r, gobj, sfx, 0x7F, 0x40);
            }
        }
    }
}

/// The first gas's `it_2725_Logic32_Spawned`: its two effects, drifting.
#[doc(alias = "it_2725_Logic32_Spawned")]
pub fn gas1_spawned(r: &mut Running, gobj: GObjId) {
    gas_effects(r, gobj, 0x45B, 0x45D);
    gas_drift(r, gobj);
}

/// The second gas's `it_2725_Logic33_Spawned`: its two effects, drifting.
#[doc(alias = "it_2725_Logic33_Spawned")]
pub fn gas2_spawned(r: &mut Running, gobj: GObjId) {
    gas_effects(r, gobj, 0x45C, 0x45E);
    gas_drift(r, gobj);
}

fn gas_effects(r: &mut Running, gobj: GObjId, a: i32, b: i32) {
    let j = jobj_of(r.game, gobj);
    ef::spawn(r, a, Some(gobj), &[Arg::JObj(j), Arg::Float(1.0)]);
    ef::spawn(r, b, Some(gobj), &[Arg::JObj(j), Arg::Float(1.0)]);
}

/// The gas drifts, `it_802CB798`: its life, not spinning, not pickable.
#[doc(alias = "it_802CB798")]
pub fn gas_drift(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The gas's `it_802CB810`: gone once its life is out, else one less (`Item_TickLifetime`).
#[doc(alias = "it_802CB810")]
pub fn gas_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// The gas's `it_802CB844`: slowing by its kind's attribute (the first gas its `x4`, the
/// second its `x8`).
#[doc(alias = "it_802CB844")]
pub fn gas_phys(r: &mut Running, gobj: GObjId) {
    let scale = match item(r.game, gobj).kind {
        MATADOGAS_GAS1 => attr_f32(r, gobj, 4),
        MATADOGAS_GAS2 => attr_f32(r, gobj, 8),
        _ => 0.0,
    };
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(scale)));
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), reg(scale)));
    ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), reg(scale)));
}

/// The gas's `it_802CB8A4`: nothing.
#[doc(alias = "it_802CB8A4")]
pub fn gas_coll(_: &mut Running, _: GObjId) -> bool {
    false
}
