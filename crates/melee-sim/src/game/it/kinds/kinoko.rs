//! The mushrooms, `melee/it/kinds/itkinoko.c` (the Super Mushroom) and `itdkinoko.c` (the
//! Poison Mushroom), the same code twice: each walks away from the stage's middle, turning at
//! walls and following the floor's slope; a fighter it touches grows or shrinks
//! (`ftColl_80077C60`).

use gekko_fp as fp;

use super::super::ground::{fall_to_floor, fall_unstick, ground_or_then_air};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{item, item_mut, lib26b1, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::mp::coll::{FLOOR_HUG, FLOOR_PUSH};

pub const SPAWNED: u32 = 0x8029_367C;
pub const ANIM0: u32 = 0x8029_3754;
pub const PHYS0: u32 = 0x8029_375C;
pub const COLL0: u32 = 0x8029_3798;
pub const ANIM1: u32 = 0x8029_38AC;
pub const PHYS1: u32 = 0x8029_38B4;
pub const COLL1: u32 = 0x8029_3964;
pub const DMG_DEALT: u32 = 0x8029_39E0;

pub const D_SPAWNED: u32 = 0x8029_3A08;
pub const D_ANIM0: u32 = 0x8029_3AE0;
pub const D_PHYS0: u32 = 0x8029_3AE8;
pub const D_COLL0: u32 = 0x8029_3B24;
pub const D_ANIM1: u32 = 0x8029_3C38;
pub const D_PHYS1: u32 = 0x8029_3C40;
pub const D_COLL1: u32 = 0x8029_3CF0;
pub const D_DMG_DEALT: u32 = 0x8029_3D6C;

/// Runs one of the mushrooms' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | D_ANIM0 | D_ANIM1 => still(r, gobj),
        COLL0 | D_COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        D_COLL1 => d_coll1(r, gobj),
        DMG_DEALT | D_DMG_DEALT => dmg_dealt(r, gobj),
        _ => return None,
    })
}

/// Runs one of the mushrooms' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED | D_SPAWNED => spawned(r, gobj),
        PHYS0 | D_PHYS0 => walk(r, gobj),
        PHYS1 | D_PHYS1 => phys1(r, gobj),
        _ => return false,
    }
    true
}

/// `KinokoAttrs`' float at `off`: its walking speed (0) and its deceleration in the air (4).
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a mushroom without attributes");
    r.data.f32(a.at(off))
}

/// `itKinoko_Logic26_Spawned` and `itDKinoko_Logic27_Spawned`: facing the stage's middle, at
/// rest on a flat floor's normal, falling.
#[doc(alias = "itKinoko_Logic26_Spawned")]
#[doc(alias = "itDKinoko_Logic27_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = if reg(ip.pos.x) > 0.0 { -1.0 } else { 1.0 };
    ip.xD5C = 0;
    let v = ip.xDD4_itemVar.kinoko();
    v.x0.set(0.0);
    v.x4.set_x(0.0);
    v.x4.set_y(1.0);
    v.x4.set_z(0.0);
    fell(r, gobj);
}

/// Lands: walking on at its speed, with its landing sound, `it_802936E4` and `it_80293A70`.
#[doc(alias = "it_802936E4")]
#[doc(alias = "it_80293A70")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let speed = ip.xDD4_itemVar.kinoko().x0.get();
    ip.x40_vel.x = speed;
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    lib26b1::enable_pickup(r.game, gobj);
    play_sfx(r, gobj, 0x107, 0x7F, 0x40);
    change_state(r, gobj, 0, 0x1 | change::HIT_PRESERVE);
}

/// The states that animate nothing (`itKinoko_UnkMotion0_Anim`, 1, and the Poison
/// Mushroom's).
#[doc(alias = "itKinoko_UnkMotion0_Anim")]
#[doc(alias = "itKinoko_UnkMotion1_Anim")]
#[doc(alias = "itDkinoko_UnkMotion0_Anim")]
#[doc(alias = "itDkinoko_UnkMotion1_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itKinoko_UnkMotion0_Phys`: on the ground, its speed toward where it faces, along the
/// slope.
#[doc(alias = "itKinoko_UnkMotion0_Phys")]
#[doc(alias = "itDkinoko_UnkMotion0_Phys")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    let speed = attr(r, gobj, 0);
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air == 1 {
        return;
    }
    let s = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    let v = ip.xDD4_itemVar.kinoko();
    v.x0.set(s);
    let s = v.x0.get();
    let ny = v.x4.y();
    ip.x40_vel.x = mem(fp::fmuls(reg(s), reg(ny)));
}

/// Keeps the floor's normal (`it_80276408`), when it touched a surface.
fn keep_normal(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    if let Some(n) = maplib::surface_normal(&ip.x378_itemColl) {
        let v = ip.xDD4_itemVar.kinoko();
        v.x4.set_x(n.x);
        v.x4.set_y(n.y);
        v.x4.set_z(n.z);
    }
}

/// `itKinoko_UnkMotion0_Coll`: walking, it falls off ledges, keeps the floor's normal (none on
/// the flat), and turns back at walls.
#[doc(alias = "itKinoko_UnkMotion0_Coll")]
#[doc(alias = "itDkinoko_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_then_air(r, gobj, walked_off);
    if item(r.game, gobj).ground_or_air == 1 {
        return false;
    }
    if item(r.game, gobj).x378_itemColl.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        keep_normal(r, gobj);
    } else {
        item_mut(r.game, gobj).xDD4_itemVar.kinoko().x4.set_x(0.0);
    }
    let env = item(r.game, gobj).x378_itemColl.env_flags;
    if env & 0x3F != 0 || env & 0xFC0 != 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        let v = ip.xDD4_itemVar.kinoko();
        let s = v.x0.get();
        v.x0.set(mem(fp::fneg(reg(s))));
        let s = v.x0.get();
        let ny = v.x4.y();
        ip.x40_vel.x = mem(fp::fmuls(reg(s), reg(ny)));
    }
    false
}

/// Falls, `it_8029385C` and `it_80293BE8`.
#[doc(alias = "it_8029385C")]
#[doc(alias = "it_80293BE8")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Walks off a ledge, keeping its hitboxes, `it_80293884` and `it_80293C10`.
#[doc(alias = "it_80293884")]
#[doc(alias = "it_80293C10")]
pub fn walked_off(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 0x1 | change::HIT_PRESERVE);
}

/// `itKinoko_UnkMotion1_Phys`: gravity, its sideways speed slowing to a stop.
#[doc(alias = "itKinoko_UnkMotion1_Phys")]
#[doc(alias = "itDkinoko_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let decel = reg(attr(r, gobj, 4));
    super::gravity(r, gobj);
    let ip = item_mut(r.game, gobj);
    let vx = reg(ip.x40_vel.x);
    let speed = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if speed < decel {
        ip.x40_vel.x = 0.0;
        return;
    }
    if reg(ip.x40_vel.x) > 0.0 {
        ip.x40_vel.x = mem(fp::fsubs(reg(ip.x40_vel.x), decel));
    }
    if reg(ip.x40_vel.x) < 0.0 {
        ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), decel));
    }
}

/// `itKinoko_UnkMotion1_Coll`: lands freeing itself off walls, keeping the floor's normal.
#[doc(alias = "itKinoko_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_unstick(r, gobj, landed);
    keep_floor_normal(r, gobj);
    false
}

/// `itDkinoko_UnkMotion1_Coll`: the Poison Mushroom's, bouncing off walls and ceilings
/// instead (`it_8026E414`).
#[doc(alias = "itDkinoko_UnkMotion1_Coll")]
pub fn d_coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    keep_floor_normal(r, gobj);
    false
}

/// On the ground and touching a floor, its normal kept.
fn keep_floor_normal(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if ip.ground_or_air != 1 && ip.x378_itemColl.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        keep_normal(r, gobj);
    }
}

/// `itKinoko_Logic26_DmgDealt`: gone once it touched a fighter.
#[doc(alias = "itKinoko_Logic26_DmgDealt")]
#[doc(alias = "itDKinoko_Logic27_DmgDealt")]
pub fn dmg_dealt(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Forgets a GObj a mushroom points to, `itKinoko_Logic26_EvtUnk` (`it_8026B894`).
#[doc(alias = "itKinoko_Logic26_EvtUnk")]
#[doc(alias = "itDKinoko_Logic27_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
