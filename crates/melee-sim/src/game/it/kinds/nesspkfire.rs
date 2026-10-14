//! Ness's PK Fire bolt, `melee/it/kinds/itnesspkfire.c`: it flies until its life runs out or it
//! meets a floor or wall, and becomes a pillar of fire ([`super::nesspkfirepillar`]) where it
//! hits a fighter or clanks.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_A054;
pub const START: u32 = 0x802A_A1D8;
pub const ANIM: u32 = 0x802A_A22C;
pub const COLL: u32 = 0x802A_A264;
pub const DMG_DEALT: u32 = 0x802A_A284;
pub const CLANKED: u32 = 0x802A_A2E8;
pub const ABSORBED: u32 = 0x802A_A34C;
pub const HIT_SHIELD: u32 = 0x802A_A354;
pub const REFLECTED: u32 = 0x802A_A35C;
pub const SHIELD_BOUNCED: u32 = 0x802A_A37C;
pub const EVT_UNK: u32 = 0x802A_A474;

/// `itNessPKFirepillarAttributes` (the bolt reads its own as this layout): the lifetime and
/// how far above the bolt the pillar rises.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const PILLAR_RISE: u32 = 0x4;
}

/// Runs one of the bolt's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => ground::air_touched_floor_or_wall(r, gobj),
        DMG_DEALT => burst(r, gobj),
        CLANKED => burst(r, gobj),
        ABSORBED | HIT_SHIELD => true,
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bolt's events, if `addr` is one: it has none.
pub fn run_event(_r: &mut Running, _gobj: GObjId, _addr: u32) -> bool {
    false
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("PK Fire without attributes");
    r.data.f32(attrs.at(off))
}

/// Shoots the bolt from `parent`'s hand at `pos`, `it_802AA054`: at `vel`, facing its way,
/// its model turned to `angle`.
#[doc(alias = "it_802AA054")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, vel: Vec3, facing_dir: f32, angle: f32) {
    let mut spawn = SpawnItem {
        kind: super::NESS_PK_FIRE,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.vel = vel;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(fire) = spawn_air(r, &mut spawn) else {
        return;
    };
    let j = jobj_of(r.game, fire).expect("PK Fire without a model");
    debug_owner_bubbles(r.game, fire, Some(parent));
    start(r, fire);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, angle);
}

/// The bolt's state and lifetime, `it_802AA1D8`.
#[doc(alias = "it_802AA1D8")]
pub fn start(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// `itNesspkfire_UnkMotion0_Anim`: its life counts down; at 0 it goes.
#[doc(alias = "itNesspkfire_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), fp::frsp(1.0)));
    ip.xD44_lifeTimer <= 0.0
}

/// Hitting a fighter or clanking, `it_2725_Logic23_DmgDealt` and `it_2725_Logic23_Clanked`:
/// the pillar rises above it, and the bolt goes.
#[doc(alias = "it_2725_Logic23_DmgDealt")]
#[doc(alias = "it_2725_Logic23_Clanked")]
pub fn burst(r: &mut Running, gobj: GObjId) -> bool {
    let rise = attr_f32(r, gobj, attr::PILLAR_RISE);
    let ip = item(r.game, gobj);
    let mut pos = ip.pos;
    pos.y = mem(fp::fadds(reg(pos.y), reg(rise)));
    let (owner, facing) = (ip.owner, ip.facing_dir);
    super::nesspkfirepillar::spawn(r, gobj, owner, pos, facing);
    true
}

/// Bouncing off a shield, `it_2725_Logic23_ShieldBounced`: its model's turn mirrored.
#[doc(alias = "it_2725_Logic23_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("PK Fire without a model");
    let z = r.game.objs.jobjs[j].rotate.z;
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, mem(fp::fneg(reg(z))));
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Forgets a GObj the bolt points to, `it_802AA474` (`it_8026B894`).
// Ports `it_802AA474`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
