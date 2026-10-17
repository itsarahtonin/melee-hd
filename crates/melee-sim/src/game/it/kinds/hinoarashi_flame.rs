//! Cyndaquil's flame's state and callbacks (`it_802D66F8` to `it_802D6850`), at the top of
//! `melee/it/kinds/itmaril.c`. Temporary: the Marill stream ports them in `maril.rs`, and this
//! module goes at the merge.

use gekko_fp as fp;

use super::super::ground::air_stay;
use super::super::pokemon::{self, attr_f32};
use super::super::{item_mut, lib26b1, lib2725, maplib};
use super::hinoarashi::{HEADING, touch_normal, turn_off_surface};
use crate::fpu::{mem, reg};
use crate::game::ef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const FLAME_ANIM: u32 = 0x802D_66F8;

pub const FLAME_PHYS: u32 = 0x802D_6740;

pub const FLAME_COLL: u32 = 0x802D_6798;

pub const FLAME_DMG_DEALT: u32 = 0x802D_6808;

pub const FLAME_REFLECTED: u32 = 0x802D_6810;

pub const FLAME_CLANKED: u32 = 0x802D_6830;

pub const FLAME_ABSORBED: u32 = 0x802D_6838;

pub const FLAME_SHIELD_BOUNCED: u32 = 0x802D_6840;

pub const FLAME_HIT_SHIELD: u32 = 0x802D_6848;

pub const FLAME_EVT_UNK: u32 = 0x802D_6850;

/// Runs one of the flame's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        FLAME_ANIM => flame_anim(r, gobj),
        FLAME_COLL => flame_coll(r, gobj),
        FLAME_DMG_DEALT => flame_dmg_dealt(r, gobj),
        FLAME_REFLECTED => flame_reflected(r, gobj),
        FLAME_CLANKED => flame_clanked(r, gobj),
        FLAME_ABSORBED => flame_absorbed(r, gobj),
        FLAME_SHIELD_BOUNCED => flame_shield_bounced(r, gobj),
        FLAME_HIT_SHIELD => flame_hit_shield(r, gobj),
        _ => return None,
    })
}

/// Runs one of the flame's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        FLAME_PHYS => flame_phys(r, gobj),
        _ => return false,
    }
    true
}

/// `it_802D66F8`: gone, with its effects, when its life runs out.
#[doc(alias = "it_802D66F8")]
pub fn flame_anim(r: &mut Running, gobj: GObjId) -> bool {
    if lib2725::tick_lifetime(r.game, gobj) {
        ef::destroy_all(r, gobj);
        return true;
    }
    false
}

/// `it_802D6740`: rising by its lift (`xC`), its heading its velocity made unit.
#[doc(alias = "it_802D6740")]
pub fn flame_phys(r: &mut Running, gobj: GObjId) {
    let lift = attr_f32(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(lift)));
    let mut h = ip.x40_vel;
    pokemon::vars(r.game, gobj).set_vec(HEADING, h);
    let v = pokemon::vars(r.game, gobj);
    h = v.vec(HEADING);
    h.normalize();
    v.set_vec(HEADING, h);
}

/// `it_802D6798`: touching the stage (floor, walls, ceiling), its angle turns off it.
#[doc(alias = "it_802D6798")]
pub fn flame_coll(r: &mut Running, gobj: GObjId) -> bool {
    let mut coll = i32::from(air_stay(r, gobj));
    coll |= maplib::touched_wall(r.game, gobj);
    coll |= maplib::touched_ceiling(r.game, gobj);
    if coll != 0 {
        touch_normal(r, gobj, coll);
        turn_off_surface(r, gobj, coll);
    }
    false
}

/// `it_802D6808`: hitting something, it stays.
#[doc(alias = "it_802D6808")]
pub fn flame_dmg_dealt(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// `it_802D6810`: reflected back (`it_80273030`).
#[doc(alias = "it_802D6810")]
pub fn flame_reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// `it_802D6830`: clanking, it goes.
#[doc(alias = "it_802D6830")]
pub fn flame_clanked(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// `it_802D6838`: absorbed, it stays.
#[doc(alias = "it_802D6838")]
pub fn flame_absorbed(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// `it_802D6840`: bouncing off a shield, it goes.
#[doc(alias = "it_802D6840")]
pub fn flame_shield_bounced(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// `it_802D6848`: hitting a shield, it goes.
#[doc(alias = "it_802D6848")]
pub fn flame_hit_shield(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// Forgets a GObj going away, `it_802D6850` (`it_8026B894`).
#[doc(alias = "it_802D6850")]
pub fn flame_evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
