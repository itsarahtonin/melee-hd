//! Wobbuffet (`It_PKind_Sonans`), `melee/it/kinds/itsonans.c`: out of the ball it stands
//! where it lands, and a hit swings it like a punching bag: the swing's speed from the hit's
//! damage and side, its counter's hitbox as strong as the hit and fading, until its life is up.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, table_bone};
use super::super::{coll, hitbox, item, item_mut, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

pub const SPAWNED: u32 = 0x802C_D44C;
pub const LANDED: u32 = 0x802C_D4D8;
pub const EVT_UNK: u32 = 0x802C_D4DC;
pub const SWING: u32 = 0x802C_D4FC;
pub const DMG_DEALT: u32 = 0x802C_D7B8;
pub const DMG_RECEIVED: u32 = 0x802C_D7D4;
pub const ANIM0: u32 = 0x802C_D8E4;
pub const PHYS0: u32 = 0x802C_D954;
pub const COLL0: u32 = 0x802C_D974;
pub const STAND: u32 = 0x802C_D9C0;
pub const ANIM1: u32 = 0x802C_DA0C;
pub const PHYS1: u32 = 0x802C_DA3C;
pub const COLL1: u32 = 0x802C_DA5C;
pub const ENTER_AIR: u32 = 0x802C_DAA8;
pub const ANIM2: u32 = 0x802C_DB0C;
pub const PHYS2: u32 = 0x802C_DB30;
pub const COLL2: u32 = 0x802C_DBB8;

/// Its variables past the release's (`itSonans_ItemVars`): the swing's speed (`x60`) and
/// angle in degrees (`x64`), and its counter's damage (`x68`). Its script's `xDAC_itcmd_var0`
/// says it's swinging.
const SWING_SPEED: usize = 0x60;
const SWING_ANGLE: usize = 0x64;
const DAMAGE: usize = 0x68;

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// Runs one of Wobbuffet's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL1 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Wobbuffet's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        SWING | PHYS0 | PHYS1 => swing(r, gobj),
        STAND => stand(r, gobj),
        ENTER_AIR => enter_air(r, gobj),
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802CD44C`, its spawn: facing nowhere, not swinging, its life from its attributes, at
/// its first size, its hit flags set, into the air out of the ball.
#[doc(alias = "it_802CD44C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_i32(r, gobj, 0x24);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xD44_lifeTimer = mem(fp::frsp(f64::from(life)));
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(SWING_SPEED, 0.0);
    v.set_f32(SWING_ANGLE, 0.0);
    v.set_f32(DAMAGE, 0.0);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    hitbox::set_hit_ids(r.game, gobj);
    enter_air(r, gobj);
}

/// Lands and nothing happens, `it_802CD4D8`.
#[doc(alias = "it_802CD4D8")]
pub fn landed(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802CD4DC` (`it_8026B894`).
#[doc(alias = "it_802CD4DC")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `it_802CD4FC` (states 0 and 1's physics): its gravity in the air; swinging, the swing
/// pushed on while it swings away and pulled back as it returns, stopped when slow near the
/// middle, bounced off its limits, its body's bone turned to it; and its counter's damage set
/// and fading.
#[doc(alias = "it_802CD4FC")]
pub fn swing(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        super::gravity(r, gobj);
    }
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        let push = attr_f32(r, gobj, 0x14);
        let pull = attr_f32(r, gobj, 0x10);
        let rest = reg(attr_f32(r, gobj, 0x1C));
        let max = attr_f32(r, gobj, 0x18);
        let v = pokemon::vars(r.game, gobj);
        let angle = reg(v.f32(SWING_ANGLE));
        let add = |v: &mut pokemon::PokemonVars, d: f64| {
            let s = v.f32(SWING_SPEED);
            v.set_f32(SWING_SPEED, mem(fp::fadds(reg(s), d)));
        };
        let sub = |v: &mut pokemon::PokemonVars, d: f64| {
            let s = v.f32(SWING_SPEED);
            v.set_f32(SWING_SPEED, mem(fp::fsubs(reg(s), d)));
        };
        if angle > 0.0 && reg(v.f32(SWING_SPEED)) > 0.0 {
            add(v, reg(push));
        } else if angle <= 0.0 && reg(v.f32(SWING_SPEED)) <= 0.0 {
            sub(v, reg(push));
        } else if angle > 0.0 && reg(v.f32(SWING_SPEED)) <= 0.0 {
            sub(v, reg(pull));
        } else if angle <= 0.0 && reg(v.f32(SWING_SPEED)) > 0.0 {
            add(v, reg(pull));
        }
        let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
        let stopped = abs(reg(v.f32(SWING_ANGLE))) < rest && abs(reg(v.f32(SWING_SPEED))) < rest;
        if stopped {
            v.set_f32(SWING_ANGLE, 0.0);
            v.set_f32(SWING_SPEED, 0.0);
            item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        } else {
            let (a, s) = (v.f32(SWING_ANGLE), v.f32(SWING_SPEED));
            v.set_f32(SWING_ANGLE, mem(fp::fadds(reg(a), reg(s))));
        }
        let v = pokemon::vars(r.game, gobj);
        let angle = reg(v.f32(SWING_ANGLE));
        let max = reg(max);
        if angle > max {
            bounce(v);
            v.set_f32(SWING_ANGLE, mem(max));
        } else if angle < fp::fneg(max) {
            bounce(v);
            v.set_f32(SWING_ANGLE, mem(fp::fneg(max)));
        }
        let angle = pokemon::vars(r.game, gobj).f32(SWING_ANGLE);
        let bone = table_bone(r.game, gobj, 4).expect("Wobbuffet without its body's bone");
        let z = mem(fp::fmuls(0.017_453_292_384_743_69, reg(angle)));
        jobj::set_rotation_z(&mut r.game.objs.jobjs, bone, z);
    }
    let damage = pokemon::vars(r.game, gobj).f32(DAMAGE);
    let d = crate::ppc::cvt_fp2unsigned(reg(damage));
    coll::set_hit_damage(r.game, r.data, gobj, 0, d);
    let fade = attr_f32(r, gobj, 0x20);
    let v = pokemon::vars(r.game, gobj);
    let damage = v.f32(DAMAGE);
    v.set_f32(DAMAGE, mem(fp::fsubs(reg(damage), reg(fade))));
}

/// The swing turned back at a limit, at nine tenths (in double precision).
fn bounce(v: &mut pokemon::PokemonVars) {
    let s = v.f32(SWING_SPEED);
    v.set_f32(SWING_SPEED, mem(fp::frsp(fp::fmul(reg(s), fp::fneg(0.9)))));
}

/// `itSonans_Logic9_DmgDealt`: its counter hitting turns its swing back.
#[doc(alias = "itSonans_Logic9_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    let v = pokemon::vars(r.game, gobj);
    let s = v.f32(SWING_SPEED);
    v.set_f32(SWING_SPEED, mem(fp::fmuls(reg(s), fp::fneg(1.0))));
    false
}

/// `it_802CD7D4`, its damage taken: a swing from the hit's damage and side (a faster one if
/// already swinging), and its counter's damage from the hit's, up to its limit.
#[doc(alias = "it_802CD7D4")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let (k, counter, limit) = (
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 4),
        attr_f32(r, gobj, 0xC),
    );
    let ip = item(r.game, gobj);
    let (hit, dir, swinging) = (
        fp::frsp(f64::from(ip.xCA0)),
        reg(ip.xCCC_incDamageDirection),
        ip.xDAC_itcmd_var0 != 0,
    );
    let speed = || fp::fmuls(reg(k), fp::fmuls(hit, dir));
    let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
    if !swinging {
        pokemon::vars(r.game, gobj).set_f32(SWING_SPEED, mem(speed()));
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
    } else {
        let v = pokemon::vars(r.game, gobj);
        if abs(reg(v.f32(SWING_SPEED))) < abs(speed()) {
            v.set_f32(SWING_SPEED, mem(speed()));
        }
    }
    let d = fp::fmuls(hit, reg(counter));
    let v = pokemon::vars(r.game, gobj);
    if d > reg(limit) {
        v.set_f32(DAMAGE, limit);
    } else {
        v.set_f32(DAMAGE, mem(d));
    }
    false
}

/// `itSonans_UnkMotion0_Anim`: standing, its animation's end starts the next state's, with
/// its sound; its life counts down.
#[doc(alias = "itSonans_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        stand(r, gobj);
        play_sfx(r, gobj, 0x272B, 0x7F, 0x40);
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// `itSonans_UnkMotion0_Phys` and `_1_Phys`: its swing.
#[doc(alias = "itSonans_UnkMotion0_Phys")]
#[doc(alias = "itSonans_UnkMotion1_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    swing(r, gobj);
}

/// `itSonans_UnkMotion0_Coll` and `_1_Coll`: lands in the air, stays on the floor on it.
#[doc(alias = "itSonans_UnkMotion0_Coll")]
#[doc(alias = "itSonans_UnkMotion1_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        fall(r, gobj, landed);
    } else {
        ground_or_fall(r, gobj, landed);
    }
    false
}

/// Stands on, `it_802CD9C0`.
#[doc(alias = "it_802CD9C0")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itSonans_UnkMotion1_Anim`: gone when its life reaches exactly zero.
#[doc(alias = "itSonans_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let life = fp::fsubs(reg(ip.xD44_lifeTimer), 1.0);
    ip.xD44_lifeTimer = mem(life);
    life == fp::frsp(0.0)
}

/// Into the air out of the ball, `it_802CDAA8` (`Item_EnterAirStateWithHitlag`).
#[doc(alias = "it_802CDAA8")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itSonans_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itSonans_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itSonans_UnkMotion2_Phys`: falling out of the ball, then standing, its hitbox on.
#[doc(alias = "itSonans_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        hitbox::end_blink(r.game, gobj);
        hitbox::set_hit_ids2(r.game, gobj);
        hitbox::clear_xdce_b2(r.game, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
    }
}

/// `itSonans_UnkMotion2_Coll`: lands out of the ball.
#[doc(alias = "itSonans_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
