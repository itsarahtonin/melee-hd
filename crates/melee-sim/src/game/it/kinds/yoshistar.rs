//! Yoshi's ground pound stars, `melee/it/kinds/ityoshistar.c`: one each way from his landing,
//! speeding along the ground until their animation ends or they hit something.

use gekko_fp as fp;

use super::super::item::{article, attr_of, change, change_state, spawn_air};
use super::super::{data, ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

#[doc(alias = "it_802B309C")]
pub const DMG_DEALT: u32 = 0x802B_309C;
pub const CLANKED: u32 = 0x802B_30C0;
pub const HIT_SHIELD: u32 = 0x802B_30E4;
pub const ABSORBED: u32 = 0x802B_3108;
pub const SHIELD_BOUNCED: u32 = 0x802B_312C;
pub const REFLECTED: u32 = 0x802B_314C;
pub const ANIM: u32 = 0x802B_3294;
pub const PHYS: u32 = 0x802B_32C8;
pub const COLL: u32 = 0x802B_3324;
pub const EVT_UNK: u32 = 0x802B_3348;

/// `StarAttrs`: the speed it starts with and its acceleration, both along its facing.
mod attr {
    pub const SPEED: u32 = 0x0;
    pub const ACCEL: u32 = 0x4;
}

/// Runs one of the star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => burst(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        ANIM => anim(r, gobj),
        COLL => collide(r, gobj),
        _ => return None,
    })
}

/// Runs one of the star's events, if `addr` is one.
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
        .expect("a star without attributes");
    r.data.f32(attrs.at(off))
}

/// A star for `parent` from `pos` going `dir`, `it_802B2FC8`.
#[doc(alias = "it_802B2FC8")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::YOSHI_STAR,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    // `it_8026BB68`.
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    setup(r, gobj, parent);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// Its burst on a hit, `it_80272BA4`: the effect at its place, the sound, and its hits no
/// longer counted.
#[doc(alias = "it_80272BA4")]
fn burst(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, 0x411, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    super::super::item::play_sfx(r, gobj, 0x73, 0x7F, 0x40);
    lib2725::uncount_hits(r.game, gobj);
    true
}

/// Bounced off a shield, `it_802B312C`.
#[doc(alias = "it_802B312C")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Sent back by a reflection, turned to face its new way, `it_802B314C`
/// (`itReflectItemAndUpdateRotation`).
#[doc(alias = "it_802B314C")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
    let y = mem(fp::fmuls(half_pi, reg(item(r.game, gobj).facing_dir)));
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    lib2725::set_rotation_y(r.game, j, y);
    false
}

/// The star's start, `it_802B322C`: its owner, its speed ahead and up, no pickup, its state.
#[doc(alias = "it_802B322C")]
fn setup(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let speed = attr_f32(r, gobj, attr::SPEED);
    let up = attr_of(item(r.game, gobj)).f32(r.data, data::attr::X18);
    let ip = item_mut(r.game, gobj);
    ip.owner = Some(owner);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    ip.x40_vel.y = up;
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itYoshistar_UnkMotion0_Anim`: gone once its animation ends.
#[doc(alias = "itYoshistar_UnkMotion0_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    !lib2725::is_animating(r.game, gobj)
}

/// `itYoshistar_UnkMotion0_Phys`: gravity, and its acceleration along its facing.
#[doc(alias = "itYoshistar_UnkMotion0_Phys")]
fn physics(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    let push = attr_f32(r, gobj, attr::ACCEL);
    lib2725::fall(r.game, gobj, accel, max);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmadds(reg(push), reg(ip.facing_dir), reg(ip.x40_vel.x)));
}

/// `itYoshistar_UnkMotion0_Coll`: it stays in the air.
#[doc(alias = "itYoshistar_UnkMotion0_Coll")]
fn collide(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_stay(r, gobj);
    false
}

/// Forgets a GObj, `it_802B3348`.
#[doc(alias = "it_802B3348")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
