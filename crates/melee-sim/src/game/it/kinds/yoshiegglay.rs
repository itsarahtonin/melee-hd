//! The egg an item swallowed by Yoshi becomes, `melee/it/kinds/ityoshiegglay.c`: laid like a
//! fighter's egg, it falls and sits until its time is up (hits shorten it), then breaks and
//! is gone a little later.

use gekko_fp as fp;

use super::super::effect::queue;
use super::super::item::{attr_of, change, change_state, play_sfx, spawn_air};
use super::super::{data, ground, item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, SpawnKind};
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itYoshiEggLay_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SIT_TO_FALL: u32 = 0x802F_3020;
pub const FALL_ANIM: u32 = 0x802F_3078;
pub const FALL_PHYS: u32 = 0x802F_3120;
pub const FALL_COLL: u32 = 0x802F_3150;
pub const LAND: u32 = 0x802F_317C;
pub const SIT_ANIM: u32 = 0x802F_31B8;
pub const SIT_PHYS: u32 = 0x802F_3260;
pub const SIT_COLL: u32 = 0x802F_3264;
pub const BREAK_ANIM: u32 = 0x802F_32F8;
pub const DMG_RECEIVED: u32 = 0x802F_3330;
pub const EVT_UNK: u32 = 0x802F_3374;

/// What the egg lay hands the egg (`itYoshiEggLay_DatAttrs`): where it's laid and how fast,
/// its facing, its lifetime, its damage behavior (`x24`, the item's `xC98`) and how much each
/// point of damage shortens its life (`float3`).
#[derive(Clone, Copy, Debug)]
pub struct Laid {
    pub kind: i32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub facing_dir: f32,
    pub lifetime: f32,
    pub x24: f32,
    pub damage_mul: f32,
}

/// Runs one of the egg's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        FALL_ANIM => fall_anim(r, gobj),
        FALL_COLL => fall_coll(r, gobj),
        SIT_ANIM => sit_anim(r, gobj),
        SIT_COLL => sit_coll(r, gobj),
        BREAK_ANIM => break_anim(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        _ => return None,
    })
}

/// Runs one of the egg's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SIT_TO_FALL => fall(r, gobj),
        FALL_PHYS => fall_phys(r, gobj),
        LAND => land(r, gobj),
        SIT_PHYS => {}
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itYoshiEggLay_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.yoshi_egg_lay()
}

/// Lays the egg, `it_802F2F34` (its first argument, Yoshi, unused): at `pos` (on the plane),
/// its speed, facing, damage factor, behavior and lifetime, falling.
#[doc(alias = "it_802F2F34")]
pub fn spawn(r: &mut Running, laid: &Laid) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: laid.kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, true)`.
    spawn.prev_pos = laid.pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = laid.facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = laid.vel;
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    vars(r.game, gobj).x0.set(laid.damage_mul);
    item_mut(r.game, gobj).xC98 = laid.x24;
    lib2725::set_lifetime(r.game, r.data, gobj, laid.lifetime);
    fall(r, gobj);
    Some(gobj)
}

/// Falling, `it_802F3020`: no pickup, its falling state, `xDCE` bit 7 clear.
#[doc(alias = "it_802F3020")]
fn fall(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    set_flag(&mut item_mut(r.game, gobj).xDCE_flag, 7, false);
}

/// Its time counted down: at its end it breaks with its sound and effect (`it_27CF_UnkMotion0_Anim`
/// and `it_27CF_UnkMotion1_Anim`, the same code).
fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if ip.xD44_lifeTimer <= 0.0 {
        let j = jobj_of(r.game, gobj);
        play_sfx(r, gobj, 0x44618, 0x7F, 0x40);
        queue(r, gobj, SpawnKind::PosParam, 0x4CF, j, &[Arg::Float(1.0)]);
        crack(r, gobj);
    }
    false
}

/// `it_27CF_UnkMotion1_Anim`.
#[doc(alias = "it_27CF_UnkMotion1_Anim")]
fn fall_anim(r: &mut Running, gobj: GObjId) -> bool {
    tick(r, gobj)
}

/// `it_27CF_UnkMotion1_Phys`: gravity.
#[doc(alias = "it_27CF_UnkMotion1_Phys")]
fn fall_phys(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `it_27CF_UnkMotion1_Coll`: sitting once it lands.
#[doc(alias = "it_27CF_UnkMotion1_Coll")]
fn fall_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall(r, gobj, land);
    false
}

/// Landed, `it_802F317C`: still, sitting.
#[doc(alias = "it_802F317C")]
fn land(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `it_27CF_UnkMotion0_Anim`.
#[doc(alias = "it_27CF_UnkMotion0_Anim")]
fn sit_anim(r: &mut Running, gobj: GObjId) -> bool {
    tick(r, gobj)
}

/// `it_27CF_UnkMotion0_Coll`: falling again once off the ground.
#[doc(alias = "it_27CF_UnkMotion0_Coll")]
fn sit_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    false
}

/// Broken, `it_802F3290`: still, 40 frames left, hidden, blinking, its broken state.
#[doc(alias = "it_802F3290")]
fn crack(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.xD44_lifeTimer = 40.0;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    super::super::hitbox::start_blink(r.game, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `it_27CF_UnkMotion2_Anim`: gone once its last frames are up.
#[doc(alias = "it_27CF_UnkMotion2_Anim")]
fn break_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    ip.xD44_lifeTimer <= 0.0
}

/// A hit shortens its life by the damage times its factor, `it_27CF_Logic114_DmgReceived`.
#[doc(alias = "it_27CF_Logic114_DmgReceived")]
fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let mul = vars(r.game, gobj).x0.get();
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fnmsubs(
        fp::frsp(f64::from(ip.xCA0)),
        reg(mul),
        reg(ip.xD44_lifeTimer),
    ));
    false
}

/// Forgets a GObj, `it_27CF_Logic114_EvtUnk`.
#[doc(alias = "it_27CF_Logic114_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
