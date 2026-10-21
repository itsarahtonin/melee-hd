//! The star Kirby spits an inhaled item out as, `melee/it/kinds/it_2F28.c` (`It_Kind_Unk2`): it
//! flies ahead of him slowing by a fixed amount a frame, until its lifetime runs out or it
//! touches the stage; and the inhaled item's end (`it_802F28C8`), counted as his hit on an
//! enemy and gone, whether spat or swallowed.

use gekko_fp as fp;

use super::super::item::{change, change_state, destroy, spawn_air};
use super::super::{coll, hitbox, item, item_mut, lib26b1, lib2725, zako};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// `It_Kind_Unk2`.
pub const KIND: i32 = super::KIRBY_UNK2;

pub const ANIM: u32 = 0x802F_2AB0;
pub const PHYS: u32 = 0x802F_2AE8;
pub const COLL: u32 = 0x802F_2BA8;

/// `itUnk2_ItemVars::x0`: its slowing a frame.
const DECEL: usize = 0x0;

/// Runs one of the star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of the star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

/// The inhaled item gone, `it_802F28C8`: counted as its holder's hit on an enemy, with `damage`
/// in his statistics, its destruction kind cleared when `clear`, its victims forgotten, knocked
/// out as an enemy is and destroyed.
#[doc(alias = "it_802F28C8")]
pub fn release(r: &mut Running, gobj: GObjId, clear: bool, damage: f32) {
    let kirby = item(r.game, gobj)
        .grab_victim
        .expect("an inhaled item without Kirby");
    coll::enemy_hit_by_fighter(r, kirby, gobj);
    coll::enemy_hit_stats(r, kirby, gobj, reg(damage));
    let ip = item_mut(r.game, gobj);
    if clear {
        ip.destroy_type = 0;
    }
    ip.atk_victim = None;
    ip.grab_victim = None;
    zako::ko(r, gobj);
    destroy(r, gobj);
}

/// Spits the star out, `it_802F295C`: at `pos` (on the plane) moving at `vel`, facing left, his
/// own, slowing by `decel` a frame for `lifetime` frames; its hitboxes' most damage, 0 if none
/// was made.
#[doc(alias = "it_802F295C")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    vel: Vec3,
    lifetime: f32,
    decel: f32,
) -> f32 {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = mem(fp::fneg(1.0));
    spawn.x3C_damage = 0;
    spawn.vel = vel;
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return 0.0;
    };
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .kirby_spit_star()
        .set_f32(DECEL, decel);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    setup(r, gobj);
    hitbox::max_damage(r.game, gobj)
}

/// Its only state, `it_802F2A58`: out of reach of a pickup.
#[doc(alias = "it_802F2A58")]
fn setup(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// `it_2F28_UnkMotion0_Anim`: gone when its lifetime runs out.
#[doc(alias = "it_2F28_UnkMotion0_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `it_2F28_UnkMotion0_Phys`: its speed less its slowing, along its way; stopped across once
/// it's no faster than that.
#[doc(alias = "it_2F28_UnkMotion0_Phys")]
fn phys(r: &mut Running, gobj: GObjId) {
    let decel = item_mut(r.game, gobj)
        .xDD4_itemVar
        .kirby_spit_star()
        .f32(DECEL);
    let ip = item_mut(r.game, gobj);
    let (vx, vy) = (reg(ip.x40_vel.x), reg(ip.x40_vel.y));
    let speed = math::sqrt_reg(fp::fadds(fp::fmuls(vx, vx), fp::fmuls(vy, vy)));
    if speed > reg(decel) {
        let left = fp::fsubs(speed, reg(decel));
        ip.x40_vel.x = mem(fp::fdivs(fp::fmuls(vx, left), speed));
        let vy = reg(ip.x40_vel.y);
        let left = fp::fsubs(speed, reg(decel));
        ip.x40_vel.y = mem(fp::fdivs(fp::fmuls(vy, left), speed));
    } else {
        ip.x40_vel.x = 0.0;
    }
}

/// `it_2F28_UnkMotion0_Coll`: gone when it touches the stage.
#[doc(alias = "it_2F28_UnkMotion0_Coll")]
fn coll(r: &mut Running, gobj: GObjId) -> bool {
    super::super::ground::air_touched_any(r, gobj)
}
