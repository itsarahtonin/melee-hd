//! Brinstar Depths, `melee/gr/grkraid.c`: the rock the fighters stand on (map 3), Kraid (map
//! 4) and his copy behind it (map 1), the background (map 2) and map 0.
//!
//! Kraid waits, rises at one of six places, and swipes; a swipe that lands turns the rock, which
//! spins a while and eases back to rest.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::state::{Game, Running};
use super::data::{KRAID_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::{anime, grlib, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// The fields of `grKraid_YakumonoParam`, by offset.
const MAP_TIME_MIN: u32 = 0;
const MAP_TIME_MAX: u32 = 4;
const MAP_TIME_ACL: u32 = 8;
const MAP_ROT_SPD_MIN: u32 = 0xC;
const MAP_ROT_SPD_MAX: u32 = 0x10;
const KRAID_WAIT_TIME: u32 = 0x14;
const KRAID_WAIT_TIME_ADD: u32 = 0x18;
const KRAID_POS_X: u32 = 0x1C;

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn w(self, r: &Running, off: u32) -> u32 {
        r.data.u32(self.0.at(off))
    }
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
    fn pos_x(self, r: &Running, i: usize) -> f32 {
        self.f(r, KRAID_POS_X + 4 * i as u32)
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.kraid_yakumono.expect("grkraid.c: yakumono_param"))
}

/// A stage object's process or callback, by address: Brinstar Depths'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_E1E4 | 0x801F_E1E8 | 0x801F_E35C | 0x801F_E3AC | 0x801F_E3B0 | 0x801F_E6D4
        | 0x801F_F14C | 0x801F_F150 => empty(r, gobj),
        0x801F_E1B0 => init_main(r, gobj),
        0x801F_E1EC => init_copy(r, gobj),
        0x801F_E2D0 => proc_copy(r, gobj),
        0x801F_E360 => init_back(r, gobj),
        0x801F_E3B4 => init_rock(r, gobj),
        0x801F_E440 => proc_rock(r, gobj),
        0x801F_E818 => init_kraid(r, gobj),
        0x801F_EA00 => proc_kraid(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_DFFC => on_init(r),
        0x801F_E094 => on_load(r),
        0x801F_E098 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing, and the rock's process's
/// empty last call (`grKraid_801FF150`).
#[doc(alias = "grKraid_801FE1E4")]
#[doc(alias = "grKraid_801FE1E8")]
#[doc(alias = "grKraid_801FE35C")]
#[doc(alias = "grKraid_801FE3AC")]
#[doc(alias = "grKraid_801FE3B0")]
#[doc(alias = "grKraid_801FE6D4")]
#[doc(alias = "grKraid_801FF14C")]
#[doc(alias = "grKraid_801FF150")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grKraid_801FE0C4`.
#[doc(alias = "grKraid_801FE0C4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grKr_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &KRAID_CALLBACKS[map_id as usize]
}

/// Brinstar Depths' setup, `grKraid_OnInit`: its parameters, maps 0, 2, 3, 4 and 1 (Kraid and
/// his copy hidden), camera limits and blast zones.
#[doc(alias = "grKraid_OnInit")]
pub fn on_init(r: &mut Running) {
    r.game.gr.kraid_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 2, 3] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    for map_id in [4, 1] {
        let g = create(r, map_id, setup::SETUP_FRAME).expect("grKraid_801FE0C4");
        let j = jobj_of(r.game, g);
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grKraid_OnDemoInit` does nothing (only a demo runs it).
#[doc(alias = "grKraid_OnDemoInit")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grKraid_OnLoad` does nothing.
#[doc(alias = "grKraid_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Brinstar Depths' start, `grKraid_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grKraid_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@801FE1B0")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Kraid's copy's setup (map 1), `grKraid_801FE1EC`: scaled by the stage's scale.
#[doc(alias = "grKraid_801FE1EC")]
pub fn init_copy(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let s = super::ground::scale(r.game);
    let j = jobj_of(r.game, gobj).expect("jobj");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// Kraid's copy's process (map 1), `grKraid_801FE2D0`, a debugging leftover: A pressed on the
/// second controller (the master status, not the fighters' inputs) steps through the copy's
/// seven animations.
#[doc(alias = "grKraid_801FE2D0")]
pub fn proc_copy(r: &mut Running, gobj: GObjId) {
    if r.game.pads.master[1].trigger & crate::input::controller::BUTTON_A == 0 {
        return;
    }
    let mut n = r.game.gr.kraid_anim.wrapping_add(1);
    if n > 6 {
        n = 0;
    }
    r.game.gr.kraid_anim = n;
    crate::game::code::os_report();
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, n);
}

/// The background's setup (map 2), `grKraid_801FE360`: its animation.
#[doc(alias = "grKraid_801FE360")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The rock's setup (map 3), `grKraid_801FE3B4`: its collision, at rest, and its animations.
#[doc(alias = "grKraid_801FE3B4")]
pub fn init_rock(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let v = r.game.grounds[g.0].u.kraid();
    v.x0 = 0;
    v.x4 = 0.0;
    v.x8 = 0.0;
    v.xC = 0.0;
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::start(r.game, r.data, gobj, 0xD, 7, 2, 0.0, 1.0, true);
}

/// The rock's process (map 3), `grKraid_801FE440`. At rest (state 0), a swipe's direction
/// (`x4`, set by [`swipe_hit`]) starts a turn after a random delay, with its rumbling part's
/// animation and sound; turning (state 1), the delay counts down, then the speed (`x8`) eases
/// off by its step (`xC`) each tick and the rock turns by it, with the camera shaking, until
/// the speed is below a step, when it stops. The rumbling part goes back to rest when its
/// animation ends; then the wind and the collision follow.
#[doc(alias = "grKraid_801FE440")]
pub fn proc_rock(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = nth_joint(r.game, gobj, 0);
    let p = param(r.game);
    let v = r.game.grounds[g.0].u.kraid();
    match v.x0 {
        0 => {
            if reg(v.x4) != 0.0 {
                let (max, min) = (p.w(r, MAP_TIME_MAX) as i32, p.w(r, MAP_TIME_MIN) as i32);
                let delay = grlib::rand_range(&mut r.game.objs.rng, max, min);
                let v = r.game.grounds[g.0].u.kraid();
                v.x10 = delay;
                v.x0 = 1;
                anime::start(r.game, r.data, gobj, 18, 7, 1, 0.0, 1.0, true);
                r.stage_sound(420_007);
                r.game.grounds[g.0].u.kraid().x1 = 1;
            }
        }
        1 => {
            if v.x10 == 0 {
                v.x8 = mem(fp::fsubs(reg(v.x8), reg(v.xC)));
            } else {
                v.x10 = v.x10.wrapping_sub(1);
            }
            let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
            let step = abs(reg(v.xC));
            let speed = reg(v.x8);
            if abs(speed) < step {
                v.xC = 0.0;
                v.x8 = 0.0;
                v.x4 = 0.0;
                v.x0 = 0;
            } else {
                let turn = fp::fmuls(fp::fdivs(reg(std::f32::consts::PI), fp::frsp(180.0)), speed);
                let j = j.expect("jobj");
                jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
                    o.rotate.z = mem(fp::fadds(reg(o.rotate.z), turn));
                });
                crate::game::camera::request_quake(
                    r,
                    crate::game::camera::QUAKE_LOOP,
                    Some(Vec3::ZERO),
                );
            }
        }
        _ => {}
    }
    if r.game.grounds[g.0].u.kraid().x1 != 0 && anime::finished(r.game, gobj, 18, 7) {
        anime::start(r.game, r.data, gobj, 18, 7, 0, 0.0, 1.0, true);
        r.game.grounds[g.0].u.kraid().x1 = 0;
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    update_map_coll(r.game, r.data, gobj);
}

/// A swipe that lands, `grKraid_801FE6D8`: unless the rock already turns, it turns
/// `direction` (1 or -1) at a speed by how far out the hand is, within the parameters' bounds,
/// easing off over `map_time_acl` ticks, with a sound and a quake at the hand.
#[doc(alias = "grKraid_801FE6D8")]
pub fn swipe_hit(r: &mut Running, hand: Option<JObjId>, direction: f64) {
    let rock = map_gobj(r.game, 3).expect("Ground_GetMapGObj(3)");
    let g = ground_of(r.game, rock);
    if reg(r.game.grounds[g.0].u.kraid().x4) != 0.0 {
        return;
    }
    r.game.grounds[g.0].u.kraid().x4 = mem(direction);
    let hand_pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, hand, None);
    crate::game::code::os_report();
    let p = param(r.game);
    let (max, min) = (reg(p.f(r, MAP_ROT_SPD_MAX)), reg(p.f(r, MAP_ROT_SPD_MIN)));
    let rot = fp::fmuls(fp::fdivs(fp::fdivs(max, min), 132.0), min);
    let x = reg(hand_pos.x);
    let ax = if x < 0.0 { fp::fneg(x) } else { x };
    let v = r.game.grounds[g.0].u.kraid();
    v.x8 = mem(fp::fmuls(rot, ax));
    let min = p.f(r, MAP_ROT_SPD_MIN);
    let max = p.f(r, MAP_ROT_SPD_MAX);
    let v = r.game.grounds[g.0].u.kraid();
    if reg(v.x8) < reg(min) {
        v.x8 = min;
    }
    if reg(v.x8) > reg(max) {
        v.x8 = max;
    }
    v.x8 = mem(fp::fmuls(reg(v.x8), reg(v.x4)));
    let acl = fp::frsp(f64::from(p.w(r, MAP_TIME_ACL) as i32));
    let v = r.game.grounds[g.0].u.kraid();
    v.xC = mem(fp::fdivs(reg(v.x8), acl));
    r.stage_sound(420_005);
    crate::game::camera::request_quake(r, crate::game::camera::QUAKE_MEDIUM, Some(hand_pos));
}

/// A wait before Kraid next rises: the parameters' wait and a random part of its extra.
fn kraid_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let add = p.w(r, KRAID_WAIT_TIME_ADD);
    let extra = if add as i32 != 0 {
        r.game.objs.rng.below(add as i32)
    } else {
        0
    };
    p.w(r, KRAID_WAIT_TIME).wrapping_add(extra as u32) as i32
}

/// Kraid's setup (map 4), `grKraid_801FE818`: his two hands' joints, the places furthest left
/// (`x3`) and furthest right (`x4`) among the six, and a wait.
#[doc(alias = "grKraid_801FE818")]
pub fn init_kraid(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let (right_hand, left_hand) = (nth_joint(r.game, gobj, 31), nth_joint(r.game, gobj, 47));
    let p = param(r.game);
    let pos: [f32; 6] = std::array::from_fn(|i| p.pos_x(r, i));
    let v = r.game.grounds[g.0].u.kraid2();
    v.x10 = right_hand;
    v.x14 = left_hand;
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    v.x4 = 0;
    v.x3 = 0;
    for (i, &x) in pos.iter().enumerate() {
        let x = reg(x);
        if x < lo {
            v.x3 = i as i8;
            lo = x;
        } else if x > hi {
            hi = x;
            v.x4 = i as i8;
        }
    }
    v.x0 = 0;
    let wait = kraid_wait(r);
    let v = r.game.grounds[g.0].u.kraid2();
    v.xC = wait;
    v.x8 = 0.0;
    v.x2 = 0;
    v.x1 = 0;
    v.x5 = 1;
}

/// Kraid's process (map 4), `grKraid_801FEA00`. Waiting (state 0), once the wait is over and
/// the rock is still, he rises at a new place (shown with his copy); risen (state 1), he picks
/// one of three idles; idling (state 2) with its sounds, he then swipes toward the stage
/// (animation 5 or 6, state 3), and a swipe reaching the rock in its window turns it; then he
/// sinks (state 4), hides and waits again.
#[doc(alias = "grKraid_801FEA00")]
pub fn proc_kraid(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let rock_still = |r: &mut Running| {
        let rock = map_gobj(r.game, 3).expect("Ground_GetMapGObj(3)");
        let rg = ground_of(r.game, rock);
        fp::fctiwz(reg(r.game.grounds[rg.0].u.kraid().x4)) == 0
    };
    let copy = |r: &Running| {
        let c = map_gobj(r.game, 1).expect("Ground_GetMapGObj(1)");
        jobj_of(r.game, c)
    };
    match r.game.grounds[g.0].u.kraid2().x0 {
        0 => {
            if r.game.grounds[g.0].u.kraid2().xC <= 0 {
                if rock_still(r) {
                    place(r, gobj);
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                    let c = copy(r);
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, c, jobj::HIDDEN);
                    set_anim(r, gobj, 3);
                    r.stage_sound(420_001);
                    r.stage_sound(420_008);
                    r.game.grounds[g.0].u.kraid2().x0 = 1;
                }
            } else {
                let v = r.game.grounds[g.0].u.kraid2();
                v.xC = v.xC.wrapping_sub(1);
            }
        }
        1 => {
            if anime::finished(r.game, gobj, 0, 7) {
                let k = r.game.objs.rng.below(3);
                set_anim(r, gobj, k);
                let v = r.game.grounds[g.0].u.kraid2();
                v.xC = 0;
                v.x0 = 2;
            }
        }
        2 => {
            let v = r.game.grounds[g.0].u.kraid2();
            let (idle, t) = (v.x1, v.xC);
            if idle == 0 && t == 80 || idle == 1 && t == 45 {
                r.stage_sound(420_000);
            } else if idle == 2 && t == 55 {
                r.object_sound(Some(g), 0, 420_000);
            }
            let v = r.game.grounds[g.0].u.kraid2();
            let (idle, t) = (v.x1, v.xC);
            if idle == 2 && t == 161 {
                r.stop_object_sound(Some(g), 0);
                r.stage_sound(420_004);
            } else if idle == 2 && t == 246 {
                r.stage_sound(420_004);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                let side = r.game.grounds[g.0].u.kraid2().x8;
                set_anim(r, gobj, fp::fctiwz(fp::fadds(5.0, reg(side))));
                let v = r.game.grounds[g.0].u.kraid2();
                v.xC = 0;
                v.x0 = 3;
            }
            let v = r.game.grounds[g.0].u.kraid2();
            v.xC = v.xC.wrapping_add(1);
        }
        3 => {
            let left = r.game.grounds[g.0].u.kraid2().x1 == 5;
            let (from, to, hit_at) = if left { (147, 169, 1) } else { (123, 138, 0) };
            let t = r.game.grounds[g.0].u.kraid2().xC;
            if rock_still(r) && t >= from && t <= to {
                if t == from {
                    r.stage_sound(420_003);
                }
                if swipe_reached(r, gobj, hit_at) {
                    let v = r.game.grounds[g.0].u.kraid2();
                    v.x5 = 1;
                    v.x8 = 1.0;
                    let (hand, dir) = if left { (v.x14, -1.0) } else { (v.x10, 1.0) };
                    swipe_hit(r, hand, dir);
                }
            }
            let v = r.game.grounds[g.0].u.kraid2();
            v.xC = v.xC.wrapping_add(1);
            if anime::finished(r.game, gobj, 0, 7) {
                set_anim(r, gobj, 4);
                r.stage_sound(420_002);
                r.stage_sound(420_008);
                r.game.grounds[g.0].u.kraid2().x0 = 4;
            }
        }
        4 => {
            if anime::finished(r.game, gobj, 0, 7) {
                let p = param(r.game);
                let full = p
                    .w(r, KRAID_WAIT_TIME)
                    .wrapping_add(p.w(r, KRAID_WAIT_TIME_ADD)) as i32;
                r.game.grounds[g.0].u.kraid2().xC = full;
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                let c = copy(r);
                jobj::set_flags_all(&mut r.game.objs.jobjs, c, jobj::HIDDEN);
                let wait = kraid_wait(r);
                let v = r.game.grounds[g.0].u.kraid2();
                v.xC = wait;
                v.x0 = 0;
            }
        }
        _ => {}
    }
}

/// Picks where Kraid rises, `grKraid_801FEE54`: after a swipe that turned the rock (`x5`), any
/// of the six places but the last; otherwise the furthest left or right at random. He and his
/// copy move there, and he faces the stage (`x8`: 0 to swipe right, 1 left; at the middle, at
/// random).
#[doc(alias = "grKraid_801FEE54")]
pub fn place(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj");
    let v = r.game.grounds[g.0].u.kraid2();
    let i = if v.x5 != 0 {
        let last = i32::from(v.x2);
        loop {
            let i = r.game.objs.rng.below(6);
            if last != i {
                break i;
            }
        }
    } else {
        let (lo, hi) = (v.x3, v.x4);
        if r.game.objs.rng.below(2) != 0 {
            i32::from(lo)
        } else {
            i32::from(hi)
        }
    };
    let v = r.game.grounds[g.0].u.kraid2();
    v.x5 = 0;
    v.x2 = i as i8;
    let x = param(r.game).pos_x(r, i as usize);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
    let c = map_gobj(r.game, 1).expect("Ground_GetMapGObj(1)");
    let cj = jobj_of(r.game, c).expect("jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, cj, |o| o.translate.x = x);
    let side = if reg(x) == 0.0 {
        let k = i32::from(r.game.objs.rng.below(2) != 0);
        fp::frsp(f64::from(k))
    } else if reg(x) < 0.0 {
        0.0
    } else {
        1.0
    };
    r.game.grounds[g.0].u.kraid2().x8 = mem(side);
}

/// Starts animation `anim` on Kraid and his copy, `grKraid_801FF068`: each's `x1`, and the
/// copy's state set to 1.
#[doc(alias = "grKraid_801FF068")]
pub fn set_anim(r: &mut Running, gobj: GObjId, anim: i32) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.kraid2().x1 = anim as i8;
    let a = i32::from(r.game.grounds[g.0].u.kraid2().x1);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, a);
    let c = map_gobj(r.game, 1).expect("Ground_GetMapGObj(1)");
    let cg = ground_of(r.game, c);
    r.game.grounds[cg.0].u.kraid().x1 = a as i8;
    let cm = map_id(r.game, c);
    let a = i32::from(r.game.grounds[cg.0].u.kraid().x1);
    anime::start_map(r.game, r.data, c, cm, a);
    r.game.grounds[cg.0].u.kraid().x0 = 1;
}

/// Whether a swipe has reached the rock, `grKraid_801FF0E0`: its tick count at 158 (the left
/// swipe, `left` 1) or 130; then every controller rumbles.
#[doc(alias = "grKraid_801FF0E0")]
pub fn swipe_reached(r: &mut Running, gobj: GObjId, left: i32) -> bool {
    let g = ground_of(r.game, gobj);
    let at = if left == 1 { 0x9E } else { 0x82 };
    let reached = r.game.grounds[g.0].u.kraid2().xC >= at;
    if reached {
        crate::game::ft::ftlib::start_rumble_all(r, 0xC, 0);
    }
    reached
}

/// The stage's shadow check, `grKraid_OnCheckShadowRender`: every joint receives shadows.
#[doc(alias = "grKraid_OnCheckShadowRender")]
pub fn check_shadow_render() -> bool {
    true
}
