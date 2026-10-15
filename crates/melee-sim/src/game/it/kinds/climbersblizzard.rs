//! The Ice Climbers' Blizzard, `melee/it/kinds/itclimbersblizzard.c`: a puff of freezing wind
//! blown from the hand at a random angle in its spread, rising as it goes, gone at the end
//! of its lifetime or when it meets the stage.

use gekko_fp as fp;

use super::super::item::{attr_of, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{data, ground, item, item_mut, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const ANIM: u32 = 0x802C_2380;
pub const PHYS: u32 = 0x802C_23B4;
pub const COLL: u32 = 0x802C_23D4;
pub const DMG_DEALT: u32 = 0x802C_2424;
pub const REFLECTED: u32 = 0x802C_242C;
pub const CLANKED: u32 = 0x802C_244C;
pub const HIT_SHIELD: u32 = 0x802C_2454;
pub const ABSORBED: u32 = 0x802C_245C;
pub const SHIELD_BOUNCED: u32 = 0x802C_2464;
pub const EVT_UNK: u32 = 0x802C_246C;

/// `itClimbersBlizzardAttributes` offsets: its lifetime, speed, rise, and its angle's spread.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED: u32 = 0x4;
    pub const RISE: u32 = 0x8;
    pub const ANGLE_MIN: u32 = 0xC;
    pub const ANGLE_MAX: u32 = 0x10;
}

/// Runs one of the Blizzard's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => tick_lifetime(r, gobj),
        COLL => {
            ground::air_stay(r, gobj);
            let hit = maplib::touched_wall(r.game, gobj) | maplib::touched_ceiling(r.game, gobj);
            hit != 0
        }
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED | SHIELD_BOUNCED => true,
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of the Blizzard's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => {
            let rise = attr_f32(r, gobj, attr::RISE);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(rise)));
        }
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = super::super::item::article(item(r.game, gobj))
        .special(r.data)
        .expect("a Blizzard without attributes");
    r.data.f32(attrs.at(off))
}

/// `Item_TickLifetime`: done once it reaches 0, else one less.
fn tick_lifetime(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// Blows a puff from `parent` at `pos`, `itClimbersBlizzard_Spawn`: at its full size, sent
/// off (`itClimbersBlizzard_802C2248`), not to be picked up.
// Ports `itClimbersBlizzard_Spawn`; aliased once a check reaches it.
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::ICE_CLIMBER_BLIZZARD,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let scale = attr_of(item(r.game, gobj)).f32(r.data, data::attr::SCALE);
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_blizzard();
    v.flag0.set(false);
    v.x0.set(scale);
    launch(r, gobj);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    lib26b1::disable_pickup(r.game, gobj);
    super::super::lib2725::first_frame(r, gobj);
    Some(gobj)
}

/// Sends the puff off, `itClimbersBlizzard_802C2248`: at a random angle in its spread
/// (mirrored facing left), measured from straight down, at its speed, with its lifetime;
/// then its state (`itClimbersBlizzard_802C2358`).
// Ports `itClimbersBlizzard_802C2248`; aliased once a check reaches it.
// Ports `itClimbersBlizzard_802C2358`; aliased once a check reaches it.
fn launch(r: &mut Running, gobj: GObjId) {
    let rand = reg(r.game.objs.rng.draw_f32());
    let (min, max) = (
        reg(attr_f32(r, gobj, attr::ANGLE_MIN)),
        reg(attr_f32(r, gobj, attr::ANGLE_MAX)),
    );
    let speed = reg(attr_f32(r, gobj, attr::SPEED));
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    let a = fp::fmadds(fp::fsubs(max, min), rand, min);
    let a = if f64::from(item(r.game, gobj).facing_dir) == 1.0 {
        a
    } else {
        fp::fneg(a)
    };
    let tau = std::f64::consts::TAU;
    let mut angle = fp::frsp(fp::fsub(a, fp::fdiv(std::f64::consts::PI, 2.0)));
    while angle < 0.0 {
        angle = fp::frsp(fp::fadd(angle, tau));
    }
    while angle > tau {
        angle = fp::frsp(fp::fsub(angle, tau));
    }
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(math::cos(mem(angle)))));
    ip.x40_vel.y = mem(fp::fmuls(speed, reg(math::sin(mem(angle)))));
    ip.x40_vel.z = 0.0;
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_blizzard()
        .flag0
        .set(true);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Forgets a GObj the Blizzard points to, `itClimbersBlizzard_EvtUnk` (`it_8026B894`).
// Ports `itClimbersBlizzard_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
