//! Kirby's Final Cutter wave, `melee/it/kinds/itkirbycutterbeam.c`: sent from where his blade
//! lands, it travels along the floor at its angle, slowing, until its life runs out or it meets
//! a wall ahead of it.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use super::foxlaser::normalize;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

/// `It_Kind_Kirby_CBeam`.
pub const KIND: i32 = super::KIRBY_CBEAM;

pub const SPAWN: u32 = 0x8029_BAB8;
pub const START: u32 = 0x8029_BB90;
pub const ANIM: u32 = 0x8029_BC88;
pub const PHYS: u32 = 0x8029_BCA8;
pub const COLL: u32 = 0x8029_BDD4;
pub const DMG_DEALT: u32 = 0x8029_BFB4;
pub const CLANKED: u32 = 0x8029_BFBC;
pub const ABSORBED: u32 = 0x8029_BFC4;
pub const REFLECTED: u32 = 0x8029_BFCC;
pub const SHIELD_BOUNCED: u32 = 0x8029_C208;
pub const HIT_SHIELD: u32 = 0x8029_C4AC;
pub const EVT_UNK: u32 = 0x8029_C4B4;

/// `itKirbyCutterBeamAttributes`: its speed, its life and its slowing.
mod attr {
    pub const SPEED: u32 = 0x0;
    pub const LIFETIME: u32 = 0x8;
    pub const DECEL: u32 = 0xC;
}

/// `Collide_LeftWallMask` and `Collide_RightWallMask`.
const LEFT_WALL: i32 = 0x3F;
const RIGHT_WALL: i32 = 0xFC0;

/// Runs one of the wave's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        CLANKED | ABSORBED | HIT_SHIELD => ends(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the wave's events, if `addr` is one.
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
        .expect("the wave without attributes");
    r.data.f32(attrs.at(off))
}

/// Sends the wave from `pos` facing `facing_dir`, `it_8029BAB8`.
#[doc(alias = "it_8029BAB8")]
pub fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing_dir: f32) {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    spawn.prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    start(r, gobj, owner);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(owner));
    lib2725::first_frame(r, gobj);
}

/// Its velocity from its speed and angle (`facing · speed · cos`, `-speed · sin`).
fn velocity_from_angle(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let facing = reg(ip.facing_dir);
    let v = ip.xDD4_itemVar.kirby_cutter_beam();
    let (speed, angle) = (v.speed.get(), v.angle.get());
    let c = reg(math::cos(angle));
    let vx = mem(fp::fmuls(facing, fp::fmuls(reg(speed), c)));
    let s = reg(math::sin(angle));
    let vy = mem(fp::fmuls(fp::fneg(reg(speed)), s));
    ip.x40_vel.x = vx;
    ip.x40_vel.y = vy;
}

/// The wave set off, `it_8029BB90`: at the angle Kirby's body leans (`ftKb_AttackDashAir_800F21C0`),
/// its speed, its life, on the ground, and its direction and start kept.
#[doc(alias = "it_8029BB90")]
pub fn start(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let angle = r.kb_lean_angle(owner);
    let speed = attr_f32(r, gobj, attr::SPEED);
    let v = item_mut(r.game, gobj).xDD4_itemVar.kirby_cutter_beam();
    v.angle.set(angle);
    v.speed.set(speed);
    velocity_from_angle(r, gobj);
    item_mut(r.game, gobj).x40_vel.z = 0.0;
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    maplib::enter_ground(ip);
    let facing = reg(ip.facing_dir);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.kirby_cutter_beam();
    let angle = v.angle.get();
    let dx = mem(fp::fmuls(facing, reg(math::sin(angle))));
    let dy = math::cos(angle);
    v.dir.set(Vec3::new(dx, dy, 0.0));
    v.init_pos.set(pos);
}

/// `itKirbycutterbeam_UnkMotion0_Anim`: done once its life runs out (`it_80273130`).
#[doc(alias = "itKirbycutterbeam_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itKirbycutterbeam_UnkMotion0_Phys`: its velocity from its angle (pressed a little more into
/// a downward slope on the ground), slowing to no less than 0.01, and hidden its last five
/// frames.
#[doc(alias = "itKirbycutterbeam_UnkMotion0_Phys")]
pub fn physics(r: &mut Running, gobj: GObjId) {
    let decel = reg(attr_f32(r, gobj, attr::DECEL));
    let j = jobj_of(r.game, gobj).expect("the wave without a model");
    let child = r.game.objs.jobjs[j].child;
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.xDD4_itemVar.kirby_cutter_beam().init_pos.set(pos);
    velocity_from_angle(r, gobj);
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air == 0 && reg(ip.x40_vel.y) < 0.0 {
        let vy = reg(ip.x40_vel.y);
        ip.x40_vel.y = mem(fp::fmadds(f64::from(0.05_f32), vy, vy));
    }
    ip.x40_vel.z = 0.0;
    let v = ip.xDD4_itemVar.kirby_cutter_beam();
    v.speed.update(|s| mem(fp::fsubs(reg(s), decel)));
    if reg(v.speed.get()) < f64::from(0.01_f32) {
        v.speed.set(0.01);
    }
    let hidden = reg(ip.xD44_lifeTimer) <= 5.0;
    let jobjs = &mut r.game.objs.jobjs;
    if hidden {
        jobj::set_flags_all(jobjs, child, jobj::HIDDEN);
    } else {
        jobj::clear_flags_all(jobjs, child, jobj::HIDDEN);
    }
}

/// The floor's angle under it, its way (`facing · atan2(normal.x, normal.y)`).
fn floor_angle(r: &mut Running, gobj: GObjId) -> f32 {
    let ip = item(r.game, gobj);
    let n = ip.x378_itemColl.floor.normal;
    mem(fp::fmuls(reg(ip.facing_dir), reg(math::atan2(n.x, n.y))))
}

/// `itKirbycutterbeam_UnkMotion0_Coll`: along the floor at its angle, off ledges a little
/// above it, landing at the floor's angle; done when it meets a wall ahead of it (either wall
/// when still).
#[doc(alias = "itKirbycutterbeam_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 0 {
        if ground::ground(r, gobj) {
            let a = floor_angle(r, gobj);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .kirby_cutter_beam()
                .angle
                .set(a);
        } else {
            let ip = item_mut(r.game, gobj);
            maplib::enter_air(ip);
            ip.pos.y = mem(fp::fadds(reg(ip.pos.y), f64::from(0.001_f32)));
            let y = &mut ip.x378_itemColl.last_pos.y;
            *y = mem(fp::fadds(reg(*y), f64::from(0.001_f32)));
        }
    } else if ground::air(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        let a = floor_angle(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .kirby_cutter_beam()
            .angle
            .set(a);
    }
    let j = jobj_of(r.game, gobj).expect("the wave without a model");
    let angle = item_mut(r.game, gobj)
        .xDD4_itemVar
        .kirby_cutter_beam()
        .angle
        .get();
    lib2725::set_rotation_x(r.game, j, angle);
    let ip = item(r.game, gobj);
    let flags = ip.x378_itemColl.env_flags;
    let vx = reg(ip.x40_vel.x);
    if vx == 0.0 {
        flags & LEFT_WALL != 0 || flags & RIGHT_WALL != 0
    } else if vx > 0.0 {
        flags & LEFT_WALL != 0
    } else {
        flags & RIGHT_WALL != 0
    }
}

/// `itKirbyCutterBeam_Logic7_DmgDealt`: a hit it deals doesn't end it.
#[doc(alias = "itKirbyCutterBeam_Logic7_DmgDealt")]
pub fn dmg_dealt(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// A clank, an absorb or a shield ends it, `itKirbyCutterBeam_Logic7_Clanked`,
/// `itKirbyCutterBeam_Logic7_Absorbed` and `itKirbyCutterBeam_Logic7_HitShield`.
#[doc(alias = "itKirbyCutterBeam_Logic7_Clanked")]
#[doc(alias = "itKirbyCutterBeam_Logic7_Absorbed")]
#[doc(alias = "itKirbyCutterBeam_Logic7_HitShield")]
pub fn ends(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// Its angle after a turn: the floor's on the ground, else its flight's, and its model turned
/// to it and its facing.
fn reangle(r: &mut Running, gobj: GObjId) {
    let a = if item(r.game, gobj).ground_or_air == 0 {
        floor_angle(r, gobj)
    } else {
        let ip = item(r.game, gobj);
        let base = if reg(ip.facing_dir) == 1.0 {
            0.0
        } else {
            std::f64::consts::PI
        };
        let t = reg(math::atan2(ip.x40_vel.y, ip.x40_vel.x));
        mem(fp::frsp(fp::fadd(base, t)))
    };
    let a = normalize(a);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .kirby_cutter_beam()
        .angle
        .set(a);
    let j = jobj_of(r.game, gobj).expect("the wave without a model");
    lib2725::set_rotation_x(r.game, j, a);
    let facing = reg(item(r.game, gobj).facing_dir);
    let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
    let y = mem(fp::frsp(fp::fmul(half_pi, facing)));
    lib2725::set_rotation_y(r.game, j, y);
}

/// Reflected, `it_2725_Logic7_Reflected`: turned around and its velocity reversed.
#[doc(alias = "it_2725_Logic7_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    reangle(r, gobj);
    false
}

/// Bounced off a shield, `it_2725_Logic7_ShieldBounced`: its velocity mirrored in the shield's
/// normal, facing the way it now goes.
#[doc(alias = "it_2725_Logic7_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let a = normalize(math::atan2(ip.x40_vel.y, ip.x40_vel.x));
    ip.xDD4_itemVar.kirby_cutter_beam().angle.set(a);
    ip.facing_dir = if reg(ip.x40_vel.x) >= 0.0 { 1.0 } else { -1.0 };
    reangle(r, gobj);
    false
}

/// Forgets a GObj, `it_8029C4B4` (`it_8026B894`).
// Ports `it_8029C4B4`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
