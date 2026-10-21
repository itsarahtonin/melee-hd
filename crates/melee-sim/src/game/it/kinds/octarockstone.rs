//! The Octorok's stone, `melee/it/kinds/itoctarockstone.c` (`It_Kind_Octarock_Stone`): spat at
//! the nearest fighter's camera bone with a little random height, it falls; touching the stage
//! it bounces off and drops away (state 1) until it touches something again. Its hits end it.

use gekko_fp as fp;

use super::super::item::{attr_of, change, change_state, play_sfx, spawn_air};
use super::super::{data, ground, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802E_878C;
pub const DMG_DEALT: u32 = 0x802E_87D8;
pub const CLANKED: u32 = 0x802E_87E0;
pub const HIT_SHIELD: u32 = 0x802E_87E8;
pub const ABSORBED: u32 = 0x802E_87F0;
pub const REFLECTED: u32 = 0x802E_87F8;
pub const SHIELD_BOUNCED: u32 = 0x802E_881C;
pub const ANIM0: u32 = 0x802E_8864;
pub const PHYS0: u32 = 0x802E_8898;
pub const COLL0: u32 = 0x802E_889C;
pub const ANIM1: u32 = 0x802E_896C;
pub const PHYS1: u32 = 0x802E_8974;
pub const COLL1: u32 = 0x802E_8990;

/// Runs one of the stone's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED => ends(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => answers_false(r, gobj),
        COLL1 => coll1(r, gobj),
        _ => return None,
    })
}

/// Runs one of the stone's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => still(r, gobj),
        PHYS1 => phys1(r, gobj),
        _ => return false,
    }
    true
}

/// The stone's setup, `itOctarockstone_802E878C`: its hit records cleared, flying.
#[doc(alias = "itOctarockstone_802E878C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_hit_ids(r.game, gobj);
    fly(r, gobj);
}

/// Its hits end it: dealt, clanked, on a shield, absorbed (`itOctarockstone_Logic4_DmgDealt`
/// and the like).
#[doc(alias = "itOctarockstone_Logic4_DmgDealt")]
#[doc(alias = "itOctarockstone_Logic4_Clanked")]
#[doc(alias = "itOctarockstone_Logic4_HitShield")]
#[doc(alias = "itOctarockstone_Logic4_Absorbed")]
pub fn ends(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Reflected, `itOctarockstone_Logic4_Reflected` (`it_80273030`).
#[doc(alias = "itOctarockstone_Logic4_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounced off a shield, `itOctarockstone_Logic4_ShieldBounced` (`itColl_BounceOffShield`).
#[doc(alias = "itOctarockstone_Logic4_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Flies, `itOctarockstone_802E883C`: state 0.
#[doc(alias = "itOctarockstone_802E883C")]
pub fn fly(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Flying's animation, `itOctarockstone_UnkMotion0_Anim`: gravity.
#[doc(alias = "itOctarockstone_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    false
}

/// The physics that do nothing, `itOctarockstone_UnkMotion0_Phys`.
#[doc(alias = "itOctarockstone_UnkMotion0_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Flying's collision, `itOctarockstone_UnkMotion0_Coll`: touching the stage it bounces off it
/// (on the ground for a floor) and drops away.
#[doc(alias = "itOctarockstone_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    let coll = ground::air_touches(r, gobj);
    if coll & 0xF != 0 {
        let _ = maplib::reflect_velocity(r.game, r.data, gobj);
        if coll & 1 != 0 {
            maplib::enter_ground(item_mut(r.game, gobj));
        }
        drop_away(r, gobj);
    }
    false
}

/// Drops away, `itOctarockstone_802E890C`: in the air, state 1, its bounce sound.
#[doc(alias = "itOctarockstone_802E890C")]
pub fn drop_away(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    play_sfx(r, gobj, 0x138, 127, 64);
}

/// Dropping's animation, `itOctarockstone_UnkMotion1_Anim`: false.
#[doc(alias = "itOctarockstone_UnkMotion1_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Dropping's collision, `itOctarockstone_UnkMotion1_Coll`: whether it touched the stage (its
/// end).
#[doc(alias = "itOctarockstone_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_touched_any(r, gobj)
}

/// Dropping's physics, `itOctarockstone_UnkMotion1_Phys`: its fall speed off the vertical
/// velocity.
#[doc(alias = "itOctarockstone_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Forgets a GObj the stone points to, `itOctarockstone_802E89B0` (`it_8026B894`).
#[doc(alias = "itOctarockstone_802E89B0")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Aims the stone, `it_802E89D0`: `speed` its way, and up enough to come down by the nearest
/// fighter's camera bone, plus a random tenth of the height between them, kept within
/// `min_vy` and `max_vy`; level with no fighter.
///
/// ssbm-rs transliterates it from its machine code for its first product, whose factors MWCC
/// swaps (`speed * facing`).
#[doc(alias = "it_802E89D0")]
pub fn aim(r: &mut Running, gobj: GObjId, speed: f32, min_vy: f32, max_vy: f32) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    ip.x40_vel.z = 0.0;
    let pos = ip.pos;
    let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) else {
        item_mut(r.game, gobj).x40_vel.y = 0.0;
        return;
    };
    let fid = r.game.fighter_id(f);
    let target = r.camera_bone_pos(fid);
    let mut dx = fp::fsubs(reg(pos.x), reg(target.x));
    if dx < 0.0 {
        dx = fp::fneg(dx);
    }
    let t = fp::fdivs(dx, reg(speed));
    let g = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let h = fp::fmuls(fp::fmuls(g, fp::fmuls(t, t)), 0.5);
    let vy = mem(fp::fdivs(h, t));
    item_mut(r.game, gobj).x40_vel.y = vy;
    let rand = reg(r.game.objs.rng.draw_f32());
    let rise = fp::fdivs(fp::fsubs(reg(target.y), reg(pos.y)), 10.0);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fmadds(rise, rand, reg(ip.x40_vel.y)));
    if reg(ip.x40_vel.y) > reg(max_vy) {
        ip.x40_vel.y = max_vy;
    }
    if reg(ip.x40_vel.y) < reg(min_vy) {
        ip.x40_vel.y = min_vy;
    }
}

/// Spits a stone, `it_802E8ADC`: from `pos` (on the stage's plane) toward where the Octorok's
/// ECB's middle is, facing `dir`, the Octorok its second parent, aimed ([`aim`]). Its spin's way
/// comes from a stack word (docs/engine.md).
#[doc(alias = "it_802E8ADC")]
pub fn spawn(
    r: &mut Running,
    gobj: GObjId,
    pos: Vec3,
    dir: i32,
    speed: f32,
    min_vy: f32,
    max_vy: f32,
) {
    let mut spawn = SpawnItem {
        kind: super::OCTAROCK_STONE,
        ..SpawnItem::default()
    };
    spawn.prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.facing_dir = mem(fp::frsp(f64::from(dir)));
    spawn.x3C_damage = 0;
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    // The original never sets the spawn's velocity, a stack vector whose x holds a RAM address
    // an earlier call left (a negative float). The spawn reads only that sign, for the spin's
    // way (`it_80274658`), before [`aim`] sets all three.
    spawn.vel = Vec3::new(-f32::MIN_POSITIVE, 0.0, 0.0);
    if let Some(stone) = spawn_air(r, &mut spawn) {
        aim(r, stone, speed, min_vy, max_vy);
    }
}
