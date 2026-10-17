//! Unown (`It_PKind_Unknown`), `melee/it/kinds/itunknown.c`: out of the ball it takes a random
//! letter's model, floats up and away until it leaves the blast zones, then, hidden, sends
//! swarms of little Unown (`It_Kind_Unknown_Swarm`) across the stage from the edge it left by,
//! aimed at the camera's interest.

use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::gr::stage;
use crate::game::it::item::article;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mtx::Axis;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802C_E710;
pub const LANDED: u32 = 0x802C_E7CC;
pub const EVT_UNK: u32 = 0x802C_E7D0;
pub const ANIM0: u32 = 0x802C_E7F0;
pub const PHYS0: u32 = 0x802C_E870;
pub const COLL0: u32 = 0x802C_E8C8;
pub const ANIM1: u32 = 0x802C_EB9C;
pub const PHYS1: u32 = 0x802C_EC18;
pub const COLL1: u32 = 0x802C_EC1C;
pub const ANIM2: u32 = 0x802C_EC88;
pub const PHYS2: u32 = 0x802C_ECAC;
pub const COLL2: u32 = 0x802C_ED2C;
pub const SWARM_SPAWNED: u32 = 0x802C_EEC8;
pub const SWARM_EVT_UNK: u32 = 0x802C_F0B4;
pub const SWARM_ANIM: u32 = 0x802C_F120;
pub const SWARM_PHYS: u32 = 0x802C_F154;
pub const SWARM_COLL: u32 = 0x802C_F3D8;

/// `It_Kind_Unknown_Swarm`.
pub const SWARM: i32 = super::UNKNOWN_SWARM;

/// Unown's variables (`itUnknown_ItemVars`): its drift's way (`x60`, ±1), the frames to the
/// next swarm (`x64.i`) and the swarms left (`x68.i`), the edge point it sends them from
/// (`x6C`), their velocity (`x78.vec`) and which edge (`x84`: right, left, top, bottom).
const DRIFT: usize = 0x60;
const SWARM_FRAMES: usize = 0x64;
const SWARMS: usize = 0x68;
const EDGE: usize = 0x6C;
const SWARM_VEL: usize = 0x78;
const SIDE: usize = 0x84;

/// A swarm's variables (the same union): its spin's angles in degrees (`x60`, `x64.f`,
/// `x68.f`) and their speeds (`x6C`), and whether it flies mostly up or down (`x78.dir`: its
/// x speed slows, else its y speed).
const SPIN_X: usize = 0x60;
const SPIN_Y: usize = 0x64;
const SPIN_Z: usize = 0x68;
const SPIN_SPEED: usize = 0x6C;
const VERTICAL: usize = 0x78;

/// Runs one of Unown's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        SWARM_ANIM => swarm_anim(r, gobj),
        SWARM_COLL => swarm_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Unown's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        SWARM_SPAWNED => swarm_spawned(r, gobj),
        SWARM_PHYS => swarm_phys(r, gobj),
        _ => return false,
    }
    true
}

/// One of the 26 letters' models (`x24`), drawn.
fn letter(r: &mut Running, gobj: GObjId) -> Option<melee_data::archive::DataRef> {
    let i = r.game.objs.rng.below(26) as u32;
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("an Unown without attributes");
    r.data.ptr(a.at(0x24 + 4 * i))
}

/// Unown out of the ball, `it_802CE710`: a random letter, a random way to drift, its swarm
/// count, no turning, its first size, and falling out of the ball.
#[doc(alias = "it_802CE710")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let joint = letter(r, gobj);
    lib2725::set_model(r, gobj, joint);
    let drift = if r.game.objs.rng.below(2) != 0 {
        1.0
    } else {
        -1.0
    };
    let swarms = attr_i32(r, gobj, 0x18);
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(DRIFT, drift);
    v.set_i32(SWARM_FRAMES, 0);
    v.set_i32(SWARMS, swarms);
    let ip = item_mut(r.game, gobj);
    ip.xDCC_flag.b3 = false;
    ip.facing_dir = 0.0;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    enter_air(r, gobj);
}

/// Lands out of the ball: nothing, `it_802CE7CC`.
#[doc(alias = "it_802CE7CC")]
pub fn landed(_r: &mut Running, _gobj: GObjId) {}

/// Forgets a GObj going away, `it_802CE7D0` (`it_8026B894`).
#[doc(alias = "it_802CE7D0")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itUnknown_UnkMotion0_Anim`: past a blast zone, it goes hidden and sends its swarms.
#[doc(alias = "itUnknown_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    let g = &*r.game;
    if reg(pos.x) > reg(stage::blast_right(g))
        || reg(pos.x) < reg(stage::blast_left(g))
        || reg(pos.y) > reg(stage::blast_top(g))
        || reg(pos.y) < reg(stage::blast_bottom(g))
    {
        start_swarms(r, gobj);
    }
    false
}

/// `itUnknown_UnkMotion0_Phys`: rising by its own gravity, drifting faster its way.
#[doc(alias = "itUnknown_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let (g, max, accel) = (
        attr_f32(r, gobj, 0xC),
        attr_f32(r, gobj, 0x10),
        attr_f32(r, gobj, 4),
    );
    lib2725::fall(r.game, gobj, g, max);
    let drift = pokemon::vars(r.game, gobj).f32(DRIFT);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmadds(reg(accel), reg(drift), reg(ip.x40_vel.x)));
}

/// `itUnknown_UnkMotion0_Coll`: nothing.
#[doc(alias = "itUnknown_UnkMotion0_Coll")]
pub fn coll0(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// A random whole number below `n` (`HSD_Randi` of a float truncated, `randi_perm`), as a
/// float.
fn randi_f(r: &mut Running, n: f64) -> f64 {
    let i = r.game.objs.rng.below(fp::fctiwz(n));
    fp::frsp(f64::from(i))
}

/// A random point along the blast zones' span from `lo` to `hi`.
fn along(
    r: &mut Running,
    lo: fn(&crate::game::state::Game) -> f32,
    hi: fn(&crate::game::state::Game) -> f32,
) -> f32 {
    let (l, h) = (lo(r.game), hi(r.game));
    let n = randi_f(r, fp::fsubs(reg(h), reg(l)));
    mem(fp::fadds(n, reg(lo(r.game))))
}

/// Hides and starts sending swarms, `it_802CE8D0`: from a random edge of the blast zones at a
/// random point along it, towards the camera's interest at its swarm speed (`x14`); its
/// sound, one of two.
#[doc(alias = "it_802CE8D0")]
pub fn start_swarms(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    let side = r.game.objs.rng.below(4);
    let (side, x, y) = match side {
        0 => {
            let x = stage::blast_right(r.game);
            (0, x, along(r, stage::blast_bottom, stage::blast_top))
        }
        1 => {
            let x = stage::blast_left(r.game);
            (1, x, along(r, stage::blast_bottom, stage::blast_top))
        }
        2 => {
            let x = along(r, stage::blast_left, stage::blast_right);
            (2, x, stage::blast_top(r.game))
        }
        _ => {
            let x = along(r, stage::blast_left, stage::blast_right);
            (3, x, stage::blast_bottom(r.game))
        }
    };
    let speed = attr_f32(r, gobj, 0x14);
    let interest = camera::interest(&r.game.camera.cam);
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(SIDE, side);
    v.set_f32(EDGE, x);
    v.set_f32(EDGE + 4, y);
    v.set_f32(EDGE + 8, 0.0);
    let dir = interest - v.vec(EDGE);
    v.set_vec(SWARM_VEL, Vec3::new(speed, 0.0, 0.0));
    let mut vel = v.vec(SWARM_VEL);
    vel.rotate(Axis::Z, crate::math::atan2(dir.y, dir.x));
    v.set_vec(SWARM_VEL, vel);
    lib2725::face_motion(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    let sfx = if r.game.objs.rng.below(2) != 0 {
        0x271A
    } else {
        0x271B
    };
    play_sfx(r, gobj, sfx, 127, 64);
    effect_hitlag(r.game, gobj);
}

/// `itUnknown_UnkMotion1_Anim`: a swarm every few frames (`x20` and up to `x1C` more), gone
/// after the last.
#[doc(alias = "itUnknown_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let v = pokemon::vars(r.game, gobj);
    let n = v.i32(SWARM_FRAMES).wrapping_sub(1);
    v.set_i32(SWARM_FRAMES, n);
    if n < 0 {
        spawn_swarm(r, gobj);
        let v = pokemon::vars(r.game, gobj);
        let left = v.i32(SWARMS).wrapping_sub(1);
        v.set_i32(SWARMS, left);
        if left == 0 {
            return true;
        }
        let spread = attr_i32(r, gobj, 0x1C);
        let d = r.game.objs.rng.below(spread);
        let frames = attr_i32(r, gobj, 0x20).wrapping_add(d);
        pokemon::vars(r.game, gobj).set_i32(SWARM_FRAMES, frames);
    }
    false
}

/// `itUnknown_UnkMotion1_Phys`: nothing.
#[doc(alias = "itUnknown_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itUnknown_UnkMotion1_Coll`: nothing.
#[doc(alias = "itUnknown_UnkMotion1_Coll")]
pub fn coll1(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Into the air falling out of the ball, `it_802CEC24` (`Item_EnterAirStateWithHitlag`).
#[doc(alias = "it_802CEC24")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itUnknown_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itUnknown_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itUnknown_UnkMotion2_Phys`: falling out of the ball; at the end it rises (`x8`) and
/// floats.
#[doc(alias = "itUnknown_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        let up = attr_f32(r, gobj, 8);
        item_mut(r.game, gobj).x40_vel.y = up;
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
    }
}

/// `itUnknown_UnkMotion2_Coll`: lands doing nothing.
#[doc(alias = "itUnknown_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Sends a swarm, `it_802CED54`: from its edge point, scattered along the edge by up to a
/// sixth of the camera limits' height either way, at the swarm velocity.
#[doc(alias = "it_802CED54")]
pub fn spawn_swarm(r: &mut Running, gobj: GObjId) {
    let bottom = stage::cam_bottom(r.game);
    let top = stage::cam_top(r.game);
    let range = fp::fdivs(fp::fsubs(reg(top), reg(bottom)), 3.0);
    let side = pokemon::vars(r.game, gobj).i32(SIDE);
    let edge = pokemon::vars(r.game, gobj).vec(EDGE);
    let scatter = |r: &mut Running| {
        let n = randi_f(r, range);
        fp::fnmsubs(range, 0.5, n)
    };
    let (x, y) = match side {
        0 | 1 => {
            let d = scatter(r);
            (edge.x, mem(fp::fadds(reg(edge.y), d)))
        }
        2 | 3 => {
            let d = scatter(r);
            (mem(fp::fadds(reg(edge.x), d)), edge.y)
        }
        _ => panic!(
            "Unown's edge {side} is none of the four: the original spawns from stack garbage"
        ),
    };
    let ip = item(r.game, gobj);
    let (facing, owner) = (ip.facing_dir, ip.owner);
    let mut spawn = SpawnItem {
        prev_pos: Vec3::new(x, y, edge.z),
        pos: lib26b1::ecb_center(r.game, gobj),
        facing_dir: facing,
        x3C_damage: 0,
        vel: pokemon::vars(r.game, gobj).vec(SWARM_VEL),
        x40: 0,
        kind: SWARM,
        x0_parent_gobj: owner,
        x4_parent_gobj2: Some(gobj),
        ..Default::default()
    };
    spawn.x44_flag[0] &= !0x80;
    spawn_air(r, &mut spawn);
}

/// A swarm made, `it_2725_Logic38_Spawned`: a random letter, its life, no spin, its slower
/// axis slowed (`x1C`), and random spin speeds (`x4` to `x18`).
#[doc(alias = "it_2725_Logic38_Spawned")]
pub fn swarm_spawned(r: &mut Running, gobj: GObjId) {
    let joint = letter(r, gobj);
    lib2725::set_model(r, gobj, joint);
    let life = attr_i32(r, gobj, 0);
    let ip = item_mut(r.game, gobj);
    ip.xDCC_flag.b3 = false;
    ip.xD44_lifeTimer = mem(fp::frsp(f64::from(life)));
    lib2725::stop_spin(r.game, r.data, gobj);
    let slow = attr_f32(r, gobj, 0x1C);
    let ip = item_mut(r.game, gobj);
    let abs = |v: f32| {
        let v = reg(v);
        if v < 0.0 { fp::fneg(v) } else { v }
    };
    let vertical = if abs(ip.x40_vel.x) > abs(ip.x40_vel.y) {
        ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), reg(slow)));
        0
    } else {
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(slow)));
        1
    };
    let v = pokemon::vars(r.game, gobj);
    v.set_i32(VERTICAL, vertical);
    v.set_f32(SPIN_X, 0.0);
    v.set_f32(SPIN_Y, 0.0);
    v.set_f32(SPIN_Z, 0.0);
    for (i, (hi, lo)) in [(4, 8), (0xC, 0x10), (0x14, 0x18)].into_iter().enumerate() {
        let (h, l) = (attr_f32(r, gobj, hi), attr_f32(r, gobj, lo));
        let n = r.game.objs.rng.below(fp::fctiwz(fp::fadds(reg(h), reg(l))));
        let s = fp::fsubs(fp::frsp(f64::from(n)), reg(l));
        pokemon::vars(r.game, gobj).set_f32(SPIN_SPEED + 4 * i, mem(s));
    }
    swarm_fly(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
}

/// Forgets a GObj going away, `itUnknown_Logic38_EvtUnk` (`it_8026B894`).
#[doc(alias = "itUnknown_Logic38_EvtUnk")]
pub fn swarm_evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Flies, `it_802CF0D4`.
#[doc(alias = "it_802CF0D4")]
pub fn swarm_fly(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `it_802CF120`: gone when its life runs out.
#[doc(alias = "it_802CF120")]
pub fn swarm_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    reg(ip.xD44_lifeTimer) <= 0.0
}

/// `it_802CF154`: spins, and its slower axis slows on (`x20`).
#[doc(alias = "it_802CF154")]
pub fn swarm_phys(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a swarm without a model");
    let slow = attr_f32(r, gobj, 0x20);
    let v = pokemon::vars(r.game, gobj);
    let mut angles = [0.0f32; 3];
    for (i, a) in angles.iter_mut().enumerate() {
        let off = SPIN_X + 4 * i;
        let s = mem(fp::fadds(reg(v.f32(off)), reg(v.f32(SPIN_SPEED + 4 * i))));
        v.set_f32(off, s);
        *a = s;
    }
    let vertical = v.i32(VERTICAL);
    let rad = |d: f32| mem(fp::fmuls(reg(0.017453292f32), reg(d)));
    lib2725::set_rotation_x(r.game, j, rad(angles[0]));
    lib2725::set_rotation_y(r.game, j, rad(angles[1]));
    lib2725::set_rotation_z(r.game, j, rad(angles[2]));
    let ip = item_mut(r.game, gobj);
    if vertical != 0 {
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(slow)));
    } else {
        ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), reg(slow)));
    }
}

/// `it_802CF3D8`: nothing.
#[doc(alias = "it_802CF3D8")]
pub fn swarm_coll(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}
