//! Yoshi's thrown egg, `melee/it/kinds/ityoshieggthrow.c`: made in his hand as he winds up the
//! up special, thrown in an arc, and broken when it lands, hits something or its time is up.

use gekko_fp as fp;

use super::super::effect::queue;
use super::super::item::{article, attr_of, change, change_state, pick_up, spawn_air};
use super::super::{data, ground, item, item_mut, jobj_of, lib26b1, lib2725, release};
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, SpawnKind};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const PICKED_UP: u32 = 0x802B_2AE0;
pub const FLY_ANIM: u32 = 0x802B_2B5C;
pub const FLY_PHYS: u32 = 0x802B_2BA8;
pub const FLY_COLL: u32 = 0x802B_2BD8;
pub const DMG_DEALT: u32 = 0x802B_2C04;
#[doc(alias = "itYoshieggthrow_UnkMotion2_Anim")]
pub const BREAK_ANIM: u32 = 0x802B_2D30;
pub const CLANKED: u32 = 0x802B_2D50;
pub const REFLECTED: u32 = 0x802B_2E5C;
pub const HIT_SHIELD: u32 = 0x802B_2E7C;
pub const SHIELD_BOUNCED: u32 = 0x802B_2F88;
pub const EVT_UNK: u32 = 0x802B_2FA8;

/// `itYoshiEggThrowAttributes`: the flight's lifetime and the broken shell's.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const BREAK_TIME: u32 = 0x4;
}

/// The broken state.
const MS_BREAK: i32 = 2;

/// Runs one of the egg's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        FLY_ANIM => fly_anim(r, gobj),
        FLY_COLL => fly_coll(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        BREAK_ANIM => lib2725::tick_life(r.game, gobj),
        CLANKED => clanked(r, gobj),
        REFLECTED => reflected(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the egg's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PICKED_UP => picked_up(r, gobj),
        FLY_PHYS => fly_phys(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("an egg without attributes");
    r.data.f32(attrs.at(off))
}

/// The egg in hand taken away, `it_802B2890`: no owner, and gone.
#[doc(alias = "it_802B2890")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
    super::super::item::destroy(r, gobj);
}

/// Throwing the egg, `it_802B28C8`: its flight, let go at `vel`, moved by `offset`, its spin
/// for `spin` (the last argument is unused).
#[doc(alias = "it_802B28C8")]
pub fn throw(r: &mut Running, gobj: GObjId, vel: Vec3, offset: Vec3, spin: f32) {
    let j = jobj_of(r.game, gobj);
    fly(r, gobj);
    release::let_go(r, gobj, vel);
    item_mut(r.game, gobj).xDC8_word.flags.x14 = false;
    lib26b1::disable_pickup(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(offset.x)));
    ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(offset.y)));
    ip.pos.z = mem(fp::fadds(reg(ip.pos.z), reg(offset.z)));
    let pos = ip.pos;
    jobj::set_translate(&mut r.game.objs.jobjs, j.expect("jobj.h: jobj"), pos);
    lib2725::clamp_fall(r.game, gobj, spin);
}

/// An egg for `parent` at `pos`, in its hand `part`, `it_802B2A10`.
#[doc(alias = "it_802B2A10")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::YOSHI_EGG_THROW,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    // `it_8026BB68`.
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    pick_up(r, gobj, parent, part);
    Some(gobj)
}

/// `itYoshiEggThrow_Logic43_PickedUp`: held.
#[doc(alias = "itYoshiEggThrow_Logic43_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The flight, `it_802B2B08`: its state and lifetime.
#[doc(alias = "it_802B2B08")]
fn fly(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// `itYoshieggthrow_UnkMotion1_Anim`: broken once its time is up.
#[doc(alias = "itYoshieggthrow_UnkMotion1_Anim")]
fn fly_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if ip.xD44_lifeTimer <= 0.0 {
        crack(r, gobj);
    } else {
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    }
    false
}

/// `itYoshieggthrow_UnkMotion1_Phys`: gravity.
#[doc(alias = "itYoshieggthrow_UnkMotion1_Phys")]
fn fly_phys(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itYoshieggthrow_UnkMotion1_Coll`: broken on touching the stage.
#[doc(alias = "itYoshieggthrow_UnkMotion1_Coll")]
fn fly_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_until_touch(r, gobj, crack);
    false
}

/// Broken on dealing a hit unless it already is, `it_802B2C04`.
#[doc(alias = "it_802B2C04")]
fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != MS_BREAK {
        crack(r, gobj);
    }
    false
}

/// Breaking, `it_802B2C38`: its broken state, hidden, the owner's rumble, no more hits, still,
/// the shell's time, its two effects and its sound.
#[doc(alias = "it_802B2C38")]
fn crack(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let time = attr_f32(r, gobj, attr::BREAK_TIME);
    change_state(r, gobj, MS_BREAK, 0x12);
    lib26b1::hide(r.game, gobj);
    lib2725::rumble_owner(r, gobj, 12, 10);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    lib2725::clear_velocity(r.game, gobj);
    item_mut(r.game, gobj).xD44_lifeTimer = time;
    queue(r, gobj, SpawnKind::Pos, 0x4CE, j, &[]);
    queue(r, gobj, SpawnKind::PosParam, 0x4CF, j, &[Arg::Float(1.0)]);
    super::super::item::play_sfx(r, gobj, 0x44618, 0x7F, 0x40);
}

/// `it_2725_Logic43_Clanked`: broken unless it already is.
#[doc(alias = "it_2725_Logic43_Clanked")]
fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    dmg_dealt(r, gobj)
}

/// `it_802B2E7C`: broken on a shield unless it already is.
#[doc(alias = "it_802B2E7C")]
fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    dmg_dealt(r, gobj)
}

/// Sent back by a reflection, `it_802B2E5C`.
#[doc(alias = "it_802B2E5C")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounced off a shield, `it_802B2F88`.
#[doc(alias = "it_802B2F88")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Forgets a GObj, `it_802B2FA8`.
#[doc(alias = "it_802B2FA8")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
