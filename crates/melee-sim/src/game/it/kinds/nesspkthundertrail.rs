//! PK Thunder's trails, `melee/it/kinds/itnesspkthundertrail.c`: six pieces that follow the
//! ball ([`super::nesspkthunder`]) along its kept positions, flickering, each turned toward the
//! next and stretched by the ball's speed.

use gekko_fp as fp;

use super::super::item::{change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_C43C;
pub const LET_GO: u32 = 0x802A_C58C;
pub const ORPHAN: u32 = 0x802A_C5D8;
pub const START: u32 = 0x802A_C604;
pub const ANIM: u32 = 0x802A_C62C;
pub const PHYS: u32 = 0x802A_C6B0;
#[doc(alias = "itNesspkthundertrail_UnkMotion0_Coll")]
pub const COLL: u32 = 0x802A_C8A0;

/// `it_803F6C18`: each trail's kind.
const KINDS: [i32; 6] = [
    ItKind::NessPKThunderTrail1.id(),
    ItKind::NessPKThunderTrail1.id(),
    ItKind::NessPKThunderTrail1.id(),
    ItKind::NessPKThunderTrail2.id(),
    ItKind::NessPKThunderTrail3.id(),
    ItKind::NessPKThunderTrail4.id(),
];

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = 1 << 4;

/// Runs one of the trail's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => false,
        _ => return None,
    })
}

/// Runs one of the trail's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => physics(r, gobj),
        _ => return false,
    }
    true
}

/// Makes trail `idx` of `ball` at `pos`, `it_802AC43C`: `owner` Ness or none.
#[doc(alias = "it_802AC43C")]
pub fn spawn(
    r: &mut Running,
    owner: Option<GObjId>,
    ball: GObjId,
    pos: Vec3,
    idx: i32,
    attack_id: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    if !(0..=5).contains(&idx) {
        return None;
    }
    let mut spawn = SpawnItem {
        kind: KINDS[idx as usize],
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, true)`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = owner;
    spawn.maker = Some(ball);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = attack_id as _;
    let trail = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, trail);
    // `Item_ClearCmdVars`.
    ip.cmd_var3 = 0;
    ip.cmd_var2 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var0 = 0;
    let v = ip.vars.ness_pk_thunder_trail();
    v.ball.set(Some(ball));
    v.index.set(idx);
    v.frames.set(0);
    lib26b1::disable_pickup(r.game, trail);
    item_mut(r.game, trail).flags.flags.held = false;
    lib2725::show(r.game, trail);
    let ip = item_mut(r.game, trail);
    ip.vel = Vec3::new(0.0, 0.0, 0.0);
    ip.state_flags.checks_blast_zones = false;
    if owner.is_some() {
        debug_owner_bubbles(r.game, trail, owner);
    }
    start(r, trail);
    Some(trail)
}

/// The ball lets go of a trail, `it_802AC58C`: its hitboxes off, no ball and no owner.
#[doc(alias = "it_802AC58C")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.vars.ness_pk_thunder_trail().ball.set(None);
    ip.owner = None;
    ip.flags.flags.held = false;
}

/// The trail without an owner, `it_802AC5D8`.
#[doc(alias = "it_802AC5D8")]
pub fn orphan(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.flags.flags.held = false;
}

/// `it_802AC604`: state 0.
#[doc(alias = "it_802AC604")]
pub fn start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itNesspkthundertrail_UnkMotion0_Anim`: its model shown every other frame; once the ball
/// lets go of it, it goes.
#[doc(alias = "itNesspkthundertrail_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj);
    let child = j.and_then(|j| r.game.objs.jobjs[j].child);
    let n = item_mut(r.game, gobj)
        .vars
        .ness_pk_thunder_trail()
        .frames
        .get();
    if n & 1 != 0 {
        jobj::set_flags_all(&mut r.game.objs.jobjs, child, HIDDEN);
    } else {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, child, HIDDEN);
    }
    let v = item_mut(r.game, gobj).vars.ness_pk_thunder_trail();
    v.frames.update(|n| n.wrapping_add(1));
    v.ball.get().is_none()
}

/// `itNesspkthundertrail_UnkMotion0_Phys`: at the ball's position for its place in the trail,
/// stretched by the ball's speed, and turned toward the next place.
#[doc(alias = "itNesspkthundertrail_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a trail without a model");
    let v = item_mut(r.game, gobj).vars.ness_pk_thunder_trail();
    let Some(ball) = v.ball.get() else { return };
    let idx = v.index.get();
    let pos = super::nesspkthunder::trail_pos(r, ball, idx);
    item_mut(r.game, gobj).pos = pos;
    let radius = super::nesspkthunder::speed(r, ball);
    let z = mem(fp::fdivs(fp::fmuls(2.0, reg(radius)), fp::frsp(4.0)));
    lib2725::set_scale_z(r.game, j, z);
    let next = super::nesspkthunder::trail_pos(r, ball, idx.wrapping_add(1));
    let ip = item_mut(r.game, gobj);
    let (dy, dx) = (
        mem(fp::fsubs(reg(ip.pos.y), reg(next.y))),
        mem(fp::fsubs(reg(ip.pos.x), reg(next.x))),
    );
    let angle = if ip.facing_dir == 1.0 {
        reg(math::atan2(dy, dx))
    } else {
        fp::frsp(fp::fadd(std::f64::consts::PI, reg(math::atan2(dy, dx))))
    };
    let facing = reg(ip.facing_dir);
    lib2725::set_rotation_x(r.game, j, mem(fp::fmuls(angle, fp::fneg(facing))));
}
