//! PK Fire's pillar, `melee/it/kinds/itnesspkfirepillar.c`: it burns where the bolt hit,
//! shrinking as its life runs down, falling to the floor, and burning out faster for the damage
//! it takes.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_A494;
pub const START: u32 = 0x802A_A55C;
pub const ANIM: u32 = 0x802A_A5B0;
pub const PHYS: u32 = 0x802A_A6D4;
pub const COLL: u32 = 0x802A_A714;
pub const DMG_RECEIVED: u32 = 0x802A_A75C;
pub const EVT_UNK: u32 = 0x802A_A7C4;

/// `itNessPKFirepillarAttributes`: the lifetime, the life each point of damage burns, and the
/// smallest scale.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const DAMAGE_LIFE: u32 = 0x4;
    pub const SCALE: u32 = 0x8;
}

/// `ItemAttr`'s fall speed, its most, and the model's scale.
const ITEM_FALL_SPEED: u32 = 0x10;
const ITEM_FALL_SPEED_MAX: u32 = 0x14;
const ITEM_SCALE: u32 = 0x60;

/// Runs one of the pillar's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of the pillar's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => physics(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("PK Fire's pillar without attributes");
    r.data.f32(attrs.at(off))
}

fn item_attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = item(r.game, gobj)
        .xCC_item_attr
        .expect("an item without attributes");
    r.data.f32(a.at(off))
}

/// Lights the pillar at `pos`, `itNesspkfirepillar_802AA494`: `bolt` its first parent,
/// `owner` (Ness) its owner.
#[doc(alias = "itNesspkfirepillar_802AA494")]
pub fn spawn(r: &mut Running, bolt: GObjId, owner: Option<GObjId>, pos: Vec3, facing_dir: f32) {
    let mut spawn = SpawnItem {
        kind: super::NESS_PK_FIRE_FLAME,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(bolt);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    if let Some(gobj) = spawn_air(r, &mut spawn) {
        debug_owner_bubbles(r.game, gobj, owner);
        start(r, gobj);
    }
}

/// The pillar's state and lifetime, `itNesspkfirepillar_802AA55C`.
#[doc(alias = "itNesspkfirepillar_802AA55C")]
pub fn start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// `itNesspkfirepillar_UnkMotion0_Anim`: scaled from full down to the smallest scale as its
/// life runs out; once it's out, the pillar goes.
#[doc(alias = "itNesspkfirepillar_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let min = reg(attr_f32(r, gobj, attr::SCALE));
    let life0 = reg(attr_f32(r, gobj, attr::LIFETIME));
    let model = reg(item_attr_f32(r, gobj, ITEM_SCALE));
    let life = reg(item(r.game, gobj).xD44_lifeTimer);
    let s = fp::fadds(min, fp::fdivs(fp::fmuls(life, fp::fsubs(1.0, min)), life0));
    let s = mem(fp::fmuls(s, model));
    let j = jobj_of(r.game, gobj).expect("PK Fire's pillar without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    let ip = item_mut(r.game, gobj);
    if ip.xD44_lifeTimer <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// `itNesspkfirepillar_UnkMotion0_Phys`: it falls. (The original sets it airborne where it
/// meant to test it.)
#[doc(alias = "itNesspkfirepillar_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).ground_or_air = 1;
    let (speed, max) = (
        item_attr_f32(r, gobj, ITEM_FALL_SPEED),
        item_attr_f32(r, gobj, ITEM_FALL_SPEED_MAX),
    );
    lib2725::fall(r.game, gobj, speed, max);
}

/// `itNesspkfirepillar_UnkMotion0_Coll`: it falls to the floor, stopping there. (The original
/// sets it airborne where it meant to test it.)
#[doc(alias = "itNesspkfirepillar_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).ground_or_air = 1;
    ground::fall_to_floor(r, gobj, |r, g| lib2725::clear_velocity(r.game, g));
    false
}

/// Taking damage, `itNesspkfirepillar_Logic24_DmgReceived`: its life burns down by the damage
/// taken, to at least 1.
#[doc(alias = "itNesspkfirepillar_Logic24_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let per = reg(attr_f32(r, gobj, attr::DAMAGE_LIFE));
    let ip = item_mut(r.game, gobj);
    let taken = fp::frsp(f64::from(ip.xC9C));
    ip.xD44_lifeTimer = mem(fp::fnmsubs(taken, per, reg(ip.xD44_lifeTimer)));
    if ip.xD44_lifeTimer <= 0.0 {
        ip.xD44_lifeTimer = 1.0;
    }
    false
}

/// Forgets a GObj the pillar points to, `itNesspkfirepillar_Logic24_EvtUnk` (`it_8026B894`).
// Ports `itNesspkfirepillar_Logic24_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
