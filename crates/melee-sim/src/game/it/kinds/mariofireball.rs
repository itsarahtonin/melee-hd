//! Mario's fireball, `melee/it/kinds/itmariofireball.c`: thrown slightly downward, it falls and
//! bounces along the stage until its lifetime runs out or a bounce leaves it too slow.

use gekko_fp as fp;

use super::super::effect::queue;
use super::super::item::{
    article, attr_of, change, change_state, common_data, play_sfx, spawn_air,
};
use super::super::{data, ground, item, item_mut, jobj_of, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::ef::SpawnKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

#[doc(alias = "itMariofireball_UnkMotion0_Anim")]
pub const ANIM: u32 = 0x8029_B868;
pub const PHYS: u32 = 0x8029_B8A0;
pub const COLL: u32 = 0x8029_B8EC;
#[doc(alias = "itMarioFireball_Logic87_DmgDealt")]
pub const DMG_DEALT: u32 = 0x8029_BA38;
#[doc(alias = "itMarioFireball_Logic87_Reflected")]
pub const REFLECTED: u32 = 0x8029_BA40;
#[doc(alias = "itMarioFireball_Logic87_Clanked")]
pub const CLANKED: u32 = 0x8029_BA60;
#[doc(alias = "itMarioFireball_Logic87_HitShield")]
pub const HIT_SHIELD: u32 = 0x8029_BA68;
pub const ABSORBED: u32 = 0x8029_BA70;
#[doc(alias = "itMarioFireball_Logic87_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x8029_BA78;
pub const EVT_UNK: u32 = 0x8029_BA98;

/// `itUnkAttributes`: the speed, the angle, the lifetime and the slowest speed a bounce keeps
/// it.
mod attr {
    pub const SPEED: u32 = 0x0;
    pub const ANGLE: u32 = 0x4;
    pub const LIFETIME: u32 = 0x8;
    pub const MIN_SPEED: u32 = 0x10;
}

/// `It_Kind_Mario_Fire`, whose bounce has a sound and its own flash, unlike Kirby's copy's.
const KIND_MARIO_FIRE: i32 = 48;

/// The bounce's sound.
const BOUNCE_SFX: i32 = 0x2BF39;

/// Runs one of the fireball's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => lib2725::tick_lifetime(r.game, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => true,
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        SHIELD_BOUNCED => lib2725::bounce_off_shield(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of the fireball's events, if `addr` is one.
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
        .expect("a fireball without attributes");
    r.data.f32(attrs.at(off))
}

/// Throws a fireball for `parent` from `pos`, `it_8029B6F8`.
#[doc(alias = "it_8029B6F8")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, kind: i32, facing_dir: f32) {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    // `it_8026BB68`.
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn).expect("it_8029B7C0 sets up a null fireball");
    setup(r, gobj);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    super::super::lib2725::first_frame(r, gobj);
}

/// The fireball's start, `it_8029B7C0`: its speed at its angle, ahead, its lifetime, its
/// first state.
#[doc(alias = "it_8029B7C0")]
fn setup(r: &mut Running, gobj: GObjId) {
    let speed = reg(attr_f32(r, gobj, attr::SPEED));
    let angle = attr_f32(r, gobj, attr::ANGLE);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let c = reg(crate::math::cos(angle));
    let s = reg(crate::math::sin(angle));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), fp::fmuls(speed, c)));
    ip.x40_vel.y = mem(fp::fmuls(speed, s));
    ip.x40_vel.z = 0.0;
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itMariofireball_UnkMotion0_Phys`: gravity (`Item_ApplyFallingPhysics`).
#[doc(alias = "itMariofireball_UnkMotion0_Phys")]
fn physics(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let x68 = common_data(r.game).f32(r.data, data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itMariofireball_UnkMotion0_Coll`: it stays in the air; when it meets the stage it
/// bounces, and ends if the bounce leaves it slower than its attributes allow, else flashes.
#[doc(alias = "itMariofireball_UnkMotion0_Coll")]
fn collide(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_stay(r, gobj);
    if !maplib::reflect_velocity(r.game, r.data, gobj) {
        return false;
    }
    let j = jobj_of(r.game, gobj);
    let min = attr_f32(r, gobj, attr::MIN_SPEED);
    let v = item(r.game, gobj).x40_vel;
    let sq = fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y)));
    if reg(crate::math::sqrt_accurate(mem(sq))) < reg(min) {
        return true;
    }
    if item(r.game, gobj).kind == KIND_MARIO_FIRE {
        play_sfx(r, gobj, BOUNCE_SFX, 0x7F, 0x40);
        queue(r, gobj, SpawnKind::Pos, 0x47B, j, &[]);
    } else {
        queue(r, gobj, SpawnKind::Pos, 0x4A0, j, &[]);
    }
    false
}
