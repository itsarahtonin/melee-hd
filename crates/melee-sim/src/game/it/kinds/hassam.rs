//! Scizor (`It_PKind_Hassam`), `melee/it/kinds/ithassam.c`: out of the ball it lands, charges
//! along the floor, then leaps at the nearest opponent above it, and after its leap flies off
//! towards the camera.

use gekko_fp as fp;

use super::super::ground::{fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx};
use super::super::pokemon::{
    self, attr_f32, attr_i32, effect_hitlag, table_bone, take_bone_motion,
};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::it::data;
use crate::game::it::item::attr_of;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::mtx::Axis;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_DBE0;
pub const LANDED: u32 = 0x802C_DC5C;
pub const EVT_UNK: u32 = 0x802C_DC60;
pub const OUT_LANDED: u32 = 0x802C_DC80;
pub const ANIM0: u32 = 0x802C_DD24;
pub const PHYS0: u32 = 0x802C_DD78;
pub const COLL0: u32 = 0x802C_DDD0;
pub const ANIM1: u32 = 0x802C_E0C4;
pub const PHYS1: u32 = 0x802C_E308;
pub const COLL1: u32 = 0x802C_E3A8;
pub const ANIM2: u32 = 0x802C_E588;
pub const PHYS2: u32 = 0x802C_E5DC;
pub const COLL2: u32 = 0x802C_E638;
pub const ANIM3: u32 = 0x802C_E6A4;
pub const PHYS3: u32 = 0x802C_E6C8;
pub const COLL3: u32 = 0x802C_E6E8;

/// Its variables (`itHassam_ItemVars`): its own velocity added each frame (`x5C`) and its
/// model's turn (`x68`). The decomp's struct puts them 4 bytes early: the code reads them at
/// `xDD4 + 0x60` and `+ 0x6C`, past the release's variables.
const VEL: usize = 0x60;
const TURN: usize = 0x6C;

/// Runs one of Scizor's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 => coll3(r, gobj),
        _ => return None,
    })
}

/// Runs one of Scizor's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        OUT_LANDED => out_landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        _ => return false,
    }
    true
}

fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = table_bone(r.game, gobj, 2);
    take_bone_motion(r, bone, gobj);
}

fn is_air(r: &Running, gobj: GObjId) -> bool {
    item(r.game, gobj).ground_or_air == 1
}

/// Scizor out of the ball, `itHassam_802CDBE0`: facing the nearest opponent, at its first
/// size, falling out of the ball, its sound.
#[doc(alias = "itHassam_802CDBE0")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    pokemon::face_nearest_opponent(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
    play_sfx(r, gobj, 0x2721, 0x7F, 0x40);
}

/// Lands or leaves the floor: nothing, `itHassam_802CDC5C`.
#[doc(alias = "itHassam_802CDC5C")]
pub fn landed(_r: &mut Running, _gobj: GObjId) {}

/// Forgets a GObj going away, `itHassam_802CDC60` (`it_8026B894`).
#[doc(alias = "itHassam_802CDC60")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Lands out of the ball, `itHassam_802CDC80`: stopped, and posing.
#[doc(alias = "itHassam_802CDC80")]
pub fn out_landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    pose(r, gobj);
}

/// Poses, `itHassam_802CDCB4`: its bone motion from the start.
#[doc(alias = "itHassam_802CDCB4")]
pub fn pose(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    pokemon::clear_motion(r.game, gobj);
    bone_motion(r, gobj);
}

/// `itHassam_UnkMotion0_Anim`: posing; at the end it charges.
#[doc(alias = "itHassam_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        charge(r, gobj);
    }
    false
}

/// Falls by the item's gravity in the air.
fn gravity(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let (g, max) = (
        a.f32(r.data, data::attr::FALL_SPEED),
        a.f32(r.data, data::attr::FALL_SPEED_MAX),
    );
    lib2725::fall(r.game, gobj, g, max);
}

/// `itHassam_UnkMotion0_Phys`: the bone's motion, and gravity in the air.
#[doc(alias = "itHassam_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    if is_air(r, gobj) {
        gravity(r, gobj);
    }
}

/// Falls to the floor, or walks off it.
fn floor_or_fall(r: &mut Running, gobj: GObjId) {
    if is_air(r, gobj) {
        fall(r, gobj, landed);
    } else {
        ground_or_fall(r, gobj, landed);
    }
}

/// `itHassam_UnkMotion0_Coll`.
#[doc(alias = "itHassam_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    floor_or_fall(r, gobj);
    false
}

/// The fighter it leaps at, `itHassam_802CDE1C`: of the opponents of `owner` awake (not
/// teammates in a team match), the nearest whose camera bone is above `pos`, else the nearest
/// opponent.
#[doc(alias = "itHassam_802CDE1C")]
pub fn target(r: &mut Running, pos: Vec3, owner: Option<GObjId>) -> Option<GObjId> {
    let my_team = owner.map(|g| owner::fighter(r.game, g).team);
    let mut best = f64::from(f32::MAX);
    let mut result = None;
    let mut cur = r.game.world.first_in(owner::P_LINK_FIGHTER);
    while let Some(g) = cur {
        cur = r.game.world.gobjs[g.0].next;
        if r.is_same_player(owner, Some(g)) {
            continue;
        }
        let other = owner::fighter(r.game, g);
        if other.is_sleeping {
            continue;
        }
        if r.game.vs_is_teams() && my_team.is_some_and(|t| other.team == t) {
            continue;
        }
        let v = owner::camera_bone_pos(r.game, r.data, g);
        let dy = fp::fsubs(reg(pos.y), reg(v.y));
        let dx = fp::fsubs(reg(pos.x), reg(v.x));
        let dist = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
        if dist < best && reg(v.y) > reg(pos.y) {
            best = dist;
            result = Some(g);
        }
    }
    if result.is_none() {
        result = owner::nearest_opponent(r.game, r.data, pos, owner);
    }
    result
}

/// Leaps, `itHassam_802CDF28`: at its leap speed (`x4`) towards a little above (`x8`) its
/// target's camera bone (or its owner's), plus a lift (`xC`), facing its way.
#[doc(alias = "itHassam_802CDF28")]
pub fn leap(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (pos, owner) = (ip.pos, ip.owner);
    let target = target(r, pos, owner)
        .or(owner)
        .expect("Scizor without a target or owner");
    let mut at = owner::camera_bone_pos(r.game, r.data, target);
    let (speed, above, lift) = (
        attr_f32(r, gobj, 4),
        attr_f32(r, gobj, 8),
        attr_f32(r, gobj, 0xC),
    );
    at.y = mem(fp::fadds(reg(at.y), reg(above)));
    let d = at - item(r.game, gobj).pos;
    let v = pokemon::vars(r.game, gobj);
    v.set_vec(VEL, Vec3::new(speed, 0.0, 0.0));
    let mut vel = v.vec(VEL);
    vel.rotate(Axis::Z, crate::math::atan2(d.y, d.x));
    v.set_vec(VEL, vel);
    let y = mem(fp::fadds(reg(v.f32(VEL + 4)), reg(lift)));
    v.set_f32(VEL + 4, y);
    let vel = v.vec(VEL);
    item_mut(r.game, gobj).x40_vel = vel;
    lib2725::face_motion(r.game, gobj);
}

/// Charges, `itHassam_802CE008`: for its charge's frames (`x18`) at its speed (`x4`) its way.
#[doc(alias = "itHassam_802CE008")]
pub fn charge(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    bone_motion(r, gobj);
    let frames = attr_i32(r, gobj, 0x18);
    let speed = attr_f32(r, gobj, 4);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::frsp(f64::from(frames)));
    let vx = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    ip.x40_vel.x = vx;
    let vx = mem(fp::fmuls(reg(speed), reg(ip.facing_dir)));
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(VEL, vx);
    v.set_f32(VEL + 4, 0.0);
    v.set_f32(VEL + 8, 0.0);
}

/// `itHassam_UnkMotion1_Anim`: charging, its animation looping; turning round over `x20`
/// frames when its leap turned it; when its frames run out it leaps (for `x1C` frames), and
/// after the leap it flies off.
#[doc(alias = "itHassam_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
    }
    bone_motion(r, gobj);
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let j = jobj_of(r.game, gobj).expect("Scizor without a model");
        let frames = attr_i32(r, gobj, 0x20);
        let now = r.game.objs.jobjs[j].rotate.y;
        let v = pokemon::vars(r.game, gobj);
        v.set_f32(TURN, now);
        let step = fp::frsp(f64::from(180i32.wrapping_div(frames)));
        let turn = mem(fp::fmadds(reg(0.017453292f32), step, reg(v.f32(TURN))));
        v.set_f32(TURN, turn);
        lib2725::set_rotation_y(r.game, j, turn);
        let ip = item_mut(r.game, gobj);
        ip.xDB0_itcmd_var1 = ip.xDB0_itcmd_var1.wrapping_add(1);
        if ip.xDB0_itcmd_var1 > frames as u32 {
            ip.xDB0_itcmd_var1 = 0;
        }
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) < 0.0 {
        if ip.xDAC_itcmd_var0 != 0 {
            fly_off(r, gobj);
        } else {
            let before = ip.facing_dir;
            leap(r, gobj);
            let frames = attr_i32(r, gobj, 0x1C);
            let ip = item_mut(r.game, gobj);
            if reg(before) != reg(ip.facing_dir) {
                ip.xDB0_itcmd_var1 = 1;
            }
            ip.xDAC_itcmd_var0 = 1;
            ip.xD44_lifeTimer = mem(fp::frsp(f64::from(frames)));
            maplib::enter_air(ip);
        }
    }
    false
}

/// `itHassam_UnkMotion1_Phys`: the bone's motion plus its own velocity, which falls (`x14`, to
/// `x10`) in the air.
#[doc(alias = "itHassam_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    if is_air(r, gobj) {
        let (max, g) = (attr_f32(r, gobj, 0x10), attr_f32(r, gobj, 0x14));
        let v = pokemon::vars(r.game, gobj);
        let y = mem(fp::fsubs(reg(v.f32(VEL + 4)), reg(g)));
        v.set_f32(VEL + 4, y);
        if reg(y) < fp::fneg(reg(max)) {
            v.set_f32(VEL + 4, mem(fp::fneg(reg(max))));
        }
    }
    let m = pokemon::vars(r.game, gobj).vec(VEL);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), reg(m.x)));
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(m.y)));
    ip.x40_vel.z = mem(fp::fadds(reg(ip.x40_vel.z), reg(m.z)));
}

/// `itHassam_UnkMotion1_Coll`: until it leaps.
#[doc(alias = "itHassam_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        floor_or_fall(r, gobj);
    }
    false
}

/// Flies off, `itHassam_802CE400`: towards the camera at its speed (`x4`), facing nowhere,
/// unturned.
#[doc(alias = "itHassam_802CE400")]
pub fn fly_off(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("Scizor without a model");
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    bone_motion(r, gobj);
    let cam = camera::position(&r.game.camera.cam);
    let mut d = cam - item(r.game, gobj).pos;
    d.normalize();
    let speed = reg(attr_f32(r, gobj, 4));
    let d = Vec3::new(
        mem(fp::fmuls(reg(d.x), speed)),
        mem(fp::fmuls(reg(d.y), speed)),
        mem(fp::fmuls(reg(d.z), speed)),
    );
    pokemon::vars(r.game, gobj).set_vec(VEL, d);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    ip.facing_dir = 0.0;
    lib2725::set_rotation_y(r.game, j, 0.0);
}

/// `itHassam_UnkMotion2_Anim`: gone when its animation ends.
#[doc(alias = "itHassam_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// `itHassam_UnkMotion2_Phys`: the bone's motion, and moved by its own velocity.
#[doc(alias = "itHassam_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let m = pokemon::vars(r.game, gobj).vec(VEL);
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(m.x)));
    ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(m.y)));
    ip.pos.z = mem(fp::fadds(reg(ip.pos.z), reg(m.z)));
}

/// `itHassam_UnkMotion2_Coll`: nothing.
#[doc(alias = "itHassam_UnkMotion2_Coll")]
pub fn coll2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Into the air falling out of the ball, `it_802CE640` (`Item_EnterAirStateWithHitlag`).
#[doc(alias = "it_802CE640")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itHassam_UnkMotion3_Anim`: growing out of the ball.
#[doc(alias = "itHassam_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itHassam_UnkMotion3_Phys`: falling out of the ball.
#[doc(alias = "itHassam_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `itHassam_UnkMotion3_Coll`: lands and poses.
#[doc(alias = "itHassam_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, out_landed)
}
