//! The Lip's Stick's spores, `melee/it/kinds/itlipstickspore.c`: a swing's puff, drifting out
//! and slowing to a stop (state 0), or a thrown stick's, still where it burst (state 1),
//! until their life runs out or they touch the stage; a hit, clank, shield or absorber ends
//! them. Their hit plants a flower on a fighter's head.

use gekko_fp as fp;

use super::super::ground::air_touched_any;
use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{coll, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const ANIM0: u32 = 0x8029_A504;
pub const PHYS0: u32 = 0x8029_A524;
pub const COLL0: u32 = 0x8029_A58C;
pub const ANIM1: u32 = 0x8029_A5C0;
pub const PHYS1: u32 = 0x8029_A5E0;
pub const COLL1: u32 = 0x8029_A5F8;
pub const DMG_DEALT: u32 = 0x8029_A600;
pub const CLANKED: u32 = 0x8029_A608;
pub const HIT_SHIELD: u32 = 0x8029_A610;
pub const ABSORBED: u32 = 0x8029_A618;
pub const REFLECTED: u32 = 0x8029_A620;
pub const SHIELD_BOUNCED: u32 = 0x8029_A704;

/// Runs one of the spores' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 => anim(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => gone(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the spores' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        _ => return false,
    }
    true
}

/// `itLipstickSporeAttributes` at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("spores without attributes")
        .at(off)
}

/// A spore item from `owner` at `pos`, facing `facing`, of kind `kind` (`xDD4`).
fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing: f32, kind: i32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::LIPSTICK_SPORE,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .lipstick_spore()
        .xDD4
        .set(kind);
    Some(gobj)
}

/// A swing's spores from `owner` at `pos` (`kind` 0 the smash's, 1 the tilt's, 2 the jab's),
/// with their puff, `it_8029A114`.
#[doc(alias = "it_8029A114")]
pub fn swing(r: &mut Running, owner: GObjId, pos: Vec3, facing: f32, kind: i32) {
    let Some(gobj) = spawn(r, owner, pos, facing, kind) else {
        return;
    };
    launch(r, gobj);
    debug_owner_bubbles(r.game, gobj, Some(owner));
    ef::spawn(
        r,
        0x440,
        Some(gobj),
        &[ef::Arg::Vec(pos), ef::Arg::Float(facing)],
    );
}

/// A thrown stick's spores from `owner` at `pos`, with their puff, `it_8029A218`.
#[doc(alias = "it_8029A218")]
pub fn burst(r: &mut Running, owner: GObjId, pos: Vec3, facing: f32, kind: i32) {
    let Some(gobj) = spawn(r, owner, pos, facing, kind) else {
        return;
    };
    hang(r, gobj);
    debug_owner_bubbles(r.game, gobj, Some(owner));
    ef::spawn(
        r,
        0x441,
        Some(gobj),
        &[ef::Arg::Vec(pos), ef::Arg::Float(facing)],
    );
}

/// A swing's spores sent off, `it_8029A31C`: the smash's far and long, the tilt's nearer,
/// the jab's still; each slows by an even share of its speed over its life.
#[doc(alias = "it_8029A31C")]
pub fn launch(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let kind = item_mut(r.game, gobj)
        .xDD4_itemVar
        .lipstick_spore()
        .xDD4
        .get();
    let speed = reg(r.data.f32(attr(r, gobj, 0)));
    let fly = |r: &mut Running, life: u32, mul: u32, damage: u32| {
        let life = r.data.f32(attr(r, gobj, life));
        lib2725::set_lifetime(r.game, r.data, gobj, life);
        let mul = reg(r.data.f32(attr(r, gobj, mul)));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), fp::fmuls(speed, mul)));
        let step = fp::fdivs(reg(ip.x40_vel.x), reg(life));
        let step = if step < 0.0 { fp::fneg(step) } else { step };
        ip.xDD4_itemVar.lipstick_spore().xDD8.set(mem(step));
        let damage = r.data.u32(attr(r, gobj, damage));
        coll::set_hit_damage(r.game, r.data, gobj, 0, damage);
    };
    match kind {
        0 => fly(r, 0x10, 8, 0x18),
        1 => fly(r, 0xC, 4, 0x14),
        2 => {
            let life = r.data.f32(attr(r, gobj, 0xC));
            lib2725::set_lifetime(r.game, r.data, gobj, life);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = 0.0;
            ip.xDD4_itemVar.lipstick_spore().xDD8.set(0.0);
            let damage = r.data.u32(attr(r, gobj, 0x14));
            coll::set_hit_damage(r.game, r.data, gobj, 0, damage);
        }
        _ => {}
    }
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// A thrown stick's spores, still for 5 frames, `it_8029A498`.
#[doc(alias = "it_8029A498")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    lib2725::set_lifetime(r.game, r.data, gobj, 5.0);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    super::super::set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// `itLipstickspore_UnkMotion0_Anim` (and 1): gone once its life runs out.
#[doc(alias = "itLipstickspore_UnkMotion0_Anim")]
#[doc(alias = "itLipstickspore_UnkMotion1_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itLipstickspore_UnkMotion0_Phys`: slows by its share each frame, stopping once it would
/// pass zero.
#[doc(alias = "itLipstickspore_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let step = reg(ip.xDD4_itemVar.lipstick_spore().xDD8.get());
    let vx = reg(ip.x40_vel.x);
    if vx == 0.0 || step == 0.0 {
        return;
    }
    let vx = if vx > 0.0 {
        fp::fsubs(vx, step)
    } else {
        fp::fadds(vx, step)
    };
    ip.x40_vel.x = mem(vx);
    let vx = reg(ip.x40_vel.x);
    let abs = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if abs < step {
        ip.x40_vel.x = 0.0;
    }
}

/// `itLipstickspore_UnkMotion0_Coll`: gone once it touches the stage.
#[doc(alias = "itLipstickspore_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}

/// `itLipstickspore_UnkMotion1_Phys`: still.
#[doc(alias = "itLipstickspore_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
}

/// `itLipstickspore_UnkMotion1_Coll`: nothing.
#[doc(alias = "itLipstickspore_UnkMotion1_Coll")]
pub fn coll1(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Gone on a hit, a clank, a shield or an absorber, `itLipstickSpore_Logic37_DmgDealt` (and
/// `Clanked`, `HitShield`, `Absorbed`).
#[doc(alias = "itLipstickSpore_Logic37_DmgDealt")]
#[doc(alias = "itLipstickSpore_Logic37_Clanked")]
#[doc(alias = "itLipstickSpore_Logic37_HitShield")]
#[doc(alias = "itLipstickSpore_Logic37_Absorbed")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Sent back by a reflection, pointing its new way, `itLipstickSpore_Logic37_Reflected`.
#[doc(alias = "itLipstickSpore_Logic37_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let v = item(r.game, gobj).x40_vel;
    let j = jobj_of(r.game, gobj).expect("spores without a model");
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, math::atan2(v.y, v.x));
    false
}

/// Bounced off a shield, `itLipstickSpore_Logic37_ShieldBounced`.
#[doc(alias = "itLipstickSpore_Logic37_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj);
    false
}
