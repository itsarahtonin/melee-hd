//! Kongo Jungle (the N64 one), `melee/gr/groldkongo.c`: the stage (map 3), the barrel that
//! swings and turns below it (map 1), the birds that fly across the background now and then
//! (map 2), and the background (map 0).
//!
//! The barrel turns in spurts to one of eight directions and swings along its animation at a
//! speed that rises and falls. A fighter that falls near it is caught (a stage device the
//! fighter code polls, `ftCo_800C0764`) and shot out a while later toward where it points.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::code::unported_call;
use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{OLDKONGO_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `grOldKongo_80210454`, the barrel's catch test, which the fighter code calls for each
/// fighter every tick once the stage registers it.
pub const BARREL_CATCH: u32 = 0x8021_0454;

/// The catch's device type (`ftCo_800C0764`'s second argument): a barrel.
const DEVICE_BARREL: u32 = 2;

/// `MTXDegToRad`'s factor as the code loads it (a single).
const DEG_TO_RAD: f32 = 0.017_453_292;

/// The fields of `grOldKongo_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn h(self, data: &Archives, off: u32) -> i32 {
        i32::from(data.u16(self.0.at(off)) as i16)
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
}

const BIRD_WAIT_A: u32 = 0x0;
const BIRD_WAIT_B: u32 = 0x2;
const BIRD_OFFSET_Y: u32 = 0x4;
const SHOOT_A: u32 = 0x8;
const SHOOT_B: u32 = 0xC;
const BARREL_IN: u32 = 0x10;
const WAIT_A: u32 = 0x14;
const WAIT_B: u32 = 0x18;
const ROT_ACCEL: u32 = 0x1C;
const ROT_MAX: u32 = 0x20;
const ROLL_A: u32 = 0x24;
const ROLL_B: u32 = 0x28;
/// The eight directions' weights, `s16`s from down-left round to down.
const RATES: u32 = 0x2C;
const INTERVAL_A: u32 = 0x3C;
const INTERVAL_B: u32 = 0x40;
const MOVE_ACCEL: u32 = 0x44;
const MOVE_MAX: u32 = 0x48;
const STOP_A: u32 = 0x4C;
const STOP_B: u32 = 0x50;
const COLOR_SCRIPT: u32 = 0x6C;

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .oldkongo_yakumono
            .expect("groldkongo.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Kongo Jungle N64's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_F64C | 0x8020_F650 | 0x8020_F6E0 | 0x8021_0058 | 0x8021_0450 => empty(r, gobj),
        0x8020_F618 => init_back(r, gobj),
        0x8020_F654 => init_stage(r, gobj),
        0x8020_F6BC => proc_stage(r, gobj),
        0x8020_F6E4 => init_barrel(r, gobj),
        0x8020_F888 => proc_barrel(r, gobj),
        0x8021_005C => init_birds(r, gobj),
        0x8021_00FC => proc_birds(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_F46C => on_init(r),
        0x8020_F468 => on_demo_init(r),
        0x8020_F4E4 => on_load(r),
        0x8020_F4E8 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "stageGObj0_GObjProc@8020F64C")]
#[doc(alias = "stageGObj0_Callback3@8020F650")]
#[doc(alias = "stageGObj3_Callback3@8020F6E0")]
#[doc(alias = "stageGObj1_Callback3@80210058")]
#[doc(alias = "stageGObj2_Callback3@80210450")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `setupStageCallbacks`.
#[doc(alias = "setupStageCallbacks@8020F52C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`stage_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &OLDKONGO_CALLBACKS[map_id as usize]
}

/// Kongo Jungle N64's setup, `grOldKongo_8020F46C`: its parameters, maps 0, 3, 1 and 2, camera
/// limits and blast zones.
#[doc(alias = "grOldKongo_8020F46C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.oldkongo_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 3, 1, 2] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grOldKongo_8020F468` does nothing (only a demo runs it).
#[doc(alias = "grOldKongo_8020F468")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grOldKongo_8020F4E4` does nothing.
#[doc(alias = "grOldKongo_8020F4E4")]
pub fn on_load(_r: &mut Running) {}

/// Kongo Jungle N64's start, `grOldKongo_8020F4E8`: the barrel registered as a device that
/// catches fighters, and the enemy spawner, without enemies.
#[doc(alias = "grOldKongo_8020F4E8")]
pub fn on_start(r: &mut Running) {
    let barrel = map_gobj(r.game, 1).expect("groldkongo.c: the barrel");
    register_catch(r.game, barrel, DEVICE_BARREL, CodeRef(BARREL_CATCH));
    zako::start(r.game, None);
}

/// Registers a stage object as a device that catches fighters, `ftCo_800C0764`: the first of
/// its two slots that is free.
#[doc(alias = "ftCo_800C0764")]
pub fn register_catch(game: &mut Game, gobj: GObjId, kind: u32, cb: CodeRef) {
    let d = &mut game.gr.devices;
    let Some(i) = d.bury.iter().position(Option::is_none) else {
        panic!("fighter chk device catch func num over!");
    };
    d.bury[i] = Some(gobj);
    d.bury_type[i] = kind;
    d.bury_cb[i] = Some(cb);
    d.bury_count = d.bury_count.wrapping_add(1);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The background's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@8020F618")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 3), `stageGObj3_OnInit`: its collision and animation, and it
/// receives shadows.
#[doc(alias = "stageGObj3_OnInit@8020F654")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The stage's process (map 3), `stageGObj3_GObjProc`: the collision follows the model, and the
/// wind sources age.
#[doc(alias = "stageGObj3_GObjProc@8020F6BC")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The barrel's setup (map 1), `stageGObj1_OnInit`: its animation and materials, at rest
/// facing up with a random wait, swinging at full speed for a random while, and its place
/// given to the stage.
#[doc(alias = "stageGObj1_OnInit@8020F6E4")]
pub fn init_barrel(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    material::model_to_stage_class(r.game, j);
    let p = param(r.game);
    let v = r.game.grounds[g.0].u.oldkongo();
    v.xE8 = 1.0;
    v.xEC = 1.0;
    v.xD8 = 0.0;
    v.xDC = 0.0;
    v.xC4 = 0;
    v.xC6 = 0;
    v.xC8 = 2;
    v.keep = None;
    v.xE0 = 0.0;
    v.xE4 = 0.0;
    v.xEC = p.f(r.data, MOVE_MAX);
    let (wb, wa) = (
        fp::fctiwz(reg(p.f(r.data, WAIT_B))),
        fp::fctiwz(reg(p.f(r.data, WAIT_A))),
    );
    let wait = grlib::rand_range(&mut r.game.objs.rng, wb, wa);
    r.game.grounds[g.0].u.oldkongo().xCC = wait as i16;
    let (sb, sa) = (p.i(r.data, STOP_B), p.i(r.data, STOP_A));
    let stop = grlib::rand_range(&mut r.game.objs.rng, sb, sa);
    r.game.grounds[g.0].u.oldkongo().xCE = stop as i16;
    let part = nth_joint(r.game, gobj, 1);
    let pos = world_position(&mut r.game.objs, r.data, part, None);
    let angle = r.game.grounds[g.0].u.oldkongo().xDC;
    super::ground::set_barrel(r.game, Some(gobj), pos, angle);
}

/// The barrel's turning acceleration in radians.
fn rot_accel(data: &Archives, p: Param) -> f64 {
    fp::fmuls(reg(DEG_TO_RAD), reg(p.f(data, ROT_ACCEL)))
}

/// The barrel's process (map 1), `stageGObj1_GObjProc`. Its turn: at rest (0) it slows to a
/// stop, its angle snapping to where it was headed, and after its wait starts turning (1) one
/// way or the other, speeding up for a while; then it heads for one of the eight directions
/// drawn by weight (2), coasting until a stop at its rate would overshoot (3), then at rest
/// again once that much is left. Then the angle turns by the speed, wrapped, the model and the
/// stage's barrel place follow, and the swing's speed runs its own cycle: waiting (0),
/// speeding up to its top speed (1), holding it (2), slowing to a stop (3). Last, a fighter
/// caught: held while its countdown runs (1, or let go when gone), the shot's animation (2),
/// and the shot ten ticks later (3), then clear (4).
#[doc(alias = "stageGObj1_GObjProc@8020F888")]
pub fn proc_barrel(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let part = nth_joint(r.game, gobj, 1);
    let accel = rot_accel(r.data, p);
    let v = r.game.grounds[g.0].u.oldkongo();
    match v.xC4 {
        2 | 3 => {
            let speed = reg(v.xE4);
            let limit = fp::fmuls(0.5, fp::fmuls(speed, fp::fdivs(speed, accel)));
            let mut delta = if speed > 0.0 {
                fp::fsubs(reg(v.xD8), reg(v.xDC))
            } else if speed < 0.0 {
                fp::fsubs(reg(v.xDC), reg(v.xD8))
            } else {
                panic!("groldkongo.c:396: 0");
            };
            if delta < 0.0 {
                delta = fp::frsp(fp::fadd(delta, std::f64::consts::TAU));
            }
            let abs = if speed < 0.0 { fp::fneg(speed) } else { speed };
            if delta < limit || delta < abs {
                if v.xC4 == 3 {
                    v.xC4 = 0;
                }
            } else if v.xC4 == 2 {
                v.xC4 = 3;
            }
        }
        0 => {
            slow_down(v, accel);
            v.xCC = v.xCC.wrapping_sub(1);
            if v.xCC < 0 {
                v.xC4 = 1;
                let speed = if r.game.objs.rng.below(2) != 0 {
                    accel
                } else {
                    fp::fneg(accel)
                };
                let (rb, ra) = (
                    fp::fctiwz(reg(p.f(r.data, ROLL_B))),
                    fp::fctiwz(reg(p.f(r.data, ROLL_A))),
                );
                let roll = grlib::rand_range(&mut r.game.objs.rng, rb, ra);
                let v = r.game.grounds[g.0].u.oldkongo();
                v.xE0 = mem(speed);
                v.xCC = roll as i16;
            }
        }
        1 => {
            let max = fp::fmuls(reg(DEG_TO_RAD), reg(p.f(r.data, ROT_MAX)));
            v.xE4 = mem(fp::fadds(reg(v.xE4), reg(v.xE0)));
            if reg(v.xE4) > max {
                v.xE4 = mem(max);
            } else if reg(v.xE4) < fp::fneg(max) {
                v.xE4 = mem(fp::fneg(max));
            }
            let left = v.xCC;
            v.xCC = left.wrapping_sub(1);
            if left < 0 {
                v.xC4 = 2;
                let (wb, wa) = (
                    fp::fctiwz(reg(p.f(r.data, WAIT_B))),
                    fp::fctiwz(reg(p.f(r.data, WAIT_A))),
                );
                let wait = grlib::rand_range(&mut r.game.objs.rng, wb, wa);
                r.game.grounds[g.0].u.oldkongo().xCC = wait as i16;
                let to = direction(r);
                r.game.grounds[g.0].u.oldkongo().xD8 = to;
            }
        }
        _ => {}
    }
    let v = r.game.grounds[g.0].u.oldkongo();
    v.xDC = mem(fp::fadds(reg(v.xDC), reg(v.xE4)));
    if reg(v.xDC) > std::f64::consts::TAU {
        v.xDC = mem(fp::frsp(fp::fsub(reg(v.xDC), std::f64::consts::TAU)));
    } else if reg(v.xDC) < fp::fneg(std::f64::consts::TAU) {
        v.xDC = mem(fp::frsp(fp::fadd(reg(v.xDC), std::f64::consts::TAU)));
    }
    let angle = v.xDC;
    let j = part.expect("groldkongo.c: the barrel's part");
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.z = angle);
    let pos = world_position(&mut r.game.objs, r.data, part, None);
    super::ground::set_barrel(r.game, Some(gobj), pos, angle);

    let v = r.game.grounds[g.0].u.oldkongo();
    match v.xC8 {
        0 => {
            let left = v.xCE;
            v.xCE = left.wrapping_sub(1);
            if left < 0 {
                v.xC8 = 1;
            }
        }
        1 => {
            v.xEC = mem(fp::fadds(reg(v.xEC), reg(p.f(r.data, MOVE_ACCEL))));
            let max = p.f(r.data, MOVE_MAX);
            if reg(v.xEC) > reg(max) {
                v.xEC = max;
                let (sb, sa) = (p.i(r.data, STOP_B), p.i(r.data, STOP_A));
                let stop = grlib::rand_range(&mut r.game.objs.rng, sb, sa);
                let v = r.game.grounds[g.0].u.oldkongo();
                v.xCE = stop as i16;
                v.xC8 = 2;
            }
        }
        2 => {
            let left = v.xCE;
            v.xCE = left.wrapping_sub(1);
            if left < 0 {
                v.xC8 = 3;
            }
        }
        3 => {
            v.xEC = mem(fp::fsubs(reg(v.xEC), reg(p.f(r.data, MOVE_ACCEL))));
            if reg(v.xEC) < 0.0 {
                v.xEC = 0.0;
                let (ib, ia) = (p.i(r.data, INTERVAL_B), p.i(r.data, INTERVAL_A));
                let wait = grlib::rand_range(&mut r.game.objs.rng, ib, ia);
                let v = r.game.grounds[g.0].u.oldkongo();
                v.xCE = wait as i16;
                v.xC8 = 0;
            }
        }
        _ => {}
    }
    let rate = r.game.grounds[g.0].u.oldkongo().xEC;
    anime::set_rate(r.game, gobj, 0, 7, rate, true);

    let mut state = r.game.grounds[g.0].u.oldkongo().xC6;
    if state == 1 {
        // With no fighter the state clears, and falls into the shot's start all the same
        // (no `break`); nothing leaves a held state without its fighter.
        let v = r.game.grounds[g.0].u.oldkongo();
        if v.keep.is_none() {
            v.xC6 = 0;
        } else {
            let left = v.xCA;
            v.xCA = left.wrapping_sub(1);
            if left >= 0 {
                return;
            }
            v.xC6 = 2;
        }
        state = 2;
    }
    if state == 2 {
        anime::start(r.game, r.data, gobj, 2, 7, 2, 0.0, 1.0, true);
        material::reset_overlay(r.game, gobj);
        let v = r.game.grounds[g.0].u.oldkongo();
        v.hit_timer = 0;
        v.xC6 = 3;
        state = 3;
    }
    match state {
        3 => {
            let v = r.game.grounds[g.0].u.oldkongo();
            let t = v.hit_timer;
            v.hit_timer = t.wrapping_add(1);
            if t > 10 {
                let keep = v.keep.expect("groldkongo.c: the caught fighter");
                if r.game.world.gobj(keep).p_link == 8 {
                    unported_call("ftCo_8009EC70 (the barrel shooting its fighter out)");
                }
                r.game.grounds[g.0].u.oldkongo().xC6 = 4;
                r.object_sound(Some(g), 0, 0x12A);
            }
        }
        4 => r.game.grounds[g.0].u.oldkongo().xC6 = 0,
        _ => {}
    }
}

/// The barrel's turning slows by its acceleration, stopping (its angle snapped to its heading)
/// once less than a step is left: `grOldKongo_8020F888_inline`.
fn slow_down(v: &mut super::OldKongoVars, accel: f64) {
    let speed = reg(v.xE4);
    if speed > 0.0 {
        if speed < accel {
            v.xE4 = 0.0;
            v.xDC = v.xD8;
        } else {
            v.xE4 = mem(fp::fsubs(speed, accel));
        }
    } else if speed < 0.0 {
        if speed > fp::fneg(accel) {
            v.xE4 = 0.0;
            v.xDC = v.xD8;
        } else {
            v.xE4 = mem(fp::fadds(speed, accel));
        }
    }
}

/// One of the barrel's eight directions drawn by the parameters' weights, `grOldKongo_80210650`:
/// from down-left (3π/4) round through up (0) to down (-π).
#[doc(alias = "grOldKongo_80210650")]
pub fn direction(r: &mut Running) -> f32 {
    const ANGLES: [f32; 8] = [
        2.356_194_5,
        1.570_796_4,
        std::f32::consts::FRAC_PI_4,
        0.0,
        -std::f32::consts::FRAC_PI_4,
        -1.570_796_4,
        -2.356_194_5,
        -std::f32::consts::PI,
    ];
    let p = param(r.game);
    let rates: [i32; 8] = std::array::from_fn(|i| p.h(r.data, RATES + 2 * i as u32));
    let total = rates.iter().fold(0i32, |a, &b| a.wrapping_add(b));
    let mut left = if total != 0 {
        r.game.objs.rng.below(total)
    } else {
        0
    };
    for (rate, angle) in rates.into_iter().zip(ANGLES) {
        left = left.wrapping_sub(rate);
        if left < 0 {
            return angle;
        }
    }
    panic!("groldkongo.c:786: 0");
}

/// The barrel's catch test, `grOldKongo_80210454`: while it holds no one, a fighter whose
/// position lies within the parameters' radius of the barrel is caught, held for a random
/// while, with the barrel's sound, animation and flash, an effect where the fighter is, and
/// its controller's rumble.
#[doc(alias = "grOldKongo_80210454")]
pub fn barrel_catch(r: &mut Running, gobj: GObjId, fighter: GObjId) -> bool {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.oldkongo().xC6 != 0 {
        return false;
    }
    let p = param(r.game);
    let (at, _) = super::ground::barrel(r.game);
    let pos = r.game.ft_pos(fighter);
    let d = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
    let (dx, dy, dz) = (d(at.x, pos.x), d(at.y, pos.y), d(at.z, pos.z));
    let dist = fp::fadds(
        fp::fmuls(dz, dz),
        fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
    );
    let radius = reg(p.f(r.data, BARREL_IN));
    if !(dist < fp::fmuls(radius, radius)) {
        return false;
    }
    let f = reg(r.game.objs.rng.draw_f32());
    let (a, b) = (reg(p.f(r.data, SHOOT_A)), reg(p.f(r.data, SHOOT_B)));
    let hold = fp::fctiwz(fp::fmadds(fp::fsubs(b, a), f, a));
    let v = r.game.grounds[g.0].u.oldkongo();
    v.xCA = hold as i16;
    v.keep = Some(fighter);
    v.xC6 = 1;
    r.object_sound(Some(g), 0, 0x129);
    anime::start(r.game, r.data, gobj, 2, 7, 1, 0.0, 1.0, true);
    let script = r.data.ptr(p.0.at(COLOR_SCRIPT));
    material::start_script(r.game, r.data, gobj, script, 0);
    super::super::ef::spawn(r, 0x405, Some(gobj), &[super::super::ef::Arg::Vec(pos)]);
    r.ft_start_rumble(fighter, 13, 30);
    true
}

/// The barrel lets its fighter go when the fighter asks, `grOldKongo_802105AC`: a held
/// fighter's countdown ends early.
#[doc(alias = "grOldKongo_802105AC")]
pub fn barrel_release(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.oldkongo();
    if v.xC6 == 1 {
        v.xC6 = 2;
    }
}

/// The barrel drops its fighter, `grOldKongo_802105C8`: a held fighter that is still a fighter
/// (its GObj's `p_link` is 8) is let go and the barrel's flash ends.
#[doc(alias = "grOldKongo_802105C8")]
pub fn barrel_drop(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.oldkongo();
    if v.xC6 != 1 {
        return;
    }
    let keep = v.keep.expect("groldkongo.c:751: gp->u.taru.keep");
    if game.world.gobj(keep).p_link == 8 {
        let v = game.grounds[g.0].u.oldkongo();
        v.xC6 = 0;
        v.keep = None;
        material::reset_overlay(game, gobj);
    }
}

/// The birds' setup (map 2), `stageGObj2_OnInit`: hidden, a random wait.
#[doc(alias = "stageGObj2_OnInit@8021005C")]
pub fn init_birds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let wait = bird_wait(r);
    r.game.grounds[g.0].u.oldkongo_birds().xC4 = wait;
}

/// A wait between the parameters' two bounds.
fn bird_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let (b, a) = (p.h(r.data, BIRD_WAIT_B), p.h(r.data, BIRD_WAIT_A));
    grlib::rand_range(&mut r.game.objs.rng, b, a)
}

/// The birds' process (map 2), `stageGObj2_GObjProc`. The wait counts down; when it ends their
/// animation starts at a random height behind the stage, at the right edge of the view or
/// further (shown flying one way) or at the left (the other way). Once the animation ends they
/// hide and a new wait starts.
#[doc(alias = "stageGObj2_GObjProc@802100FC")]
pub fn proc_birds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let timer = r.game.grounds[g.0].u.oldkongo_birds().xC4;
    if timer > 0 {
        let left = timer.wrapping_sub(1);
        r.game.grounds[g.0].u.oldkongo_birds().xC4 = left;
        if left != 0 {
            return;
        }
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, 0);
        let p = param(r.game);
        let f = reg(r.game.objs.rng.draw_f32());
        let y = mem(fp::fmadds(
            reg(p.f(r.data, BIRD_OFFSET_Y)),
            fp::fmsubs(2.0, f, 1.0),
            70.0,
        ));
        let root = j.expect("groldkongo.c: the birds' model");
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.y = y);
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.z = -200.0);
        let ([left, _, right], _) = super::super::camera::ground_span(r.game, r.data);
        let (x, part) = if r.game.objs.rng.below(2) != 0 {
            (if 200.0 < right { right } else { 200.0 }, 3)
        } else {
            (if -200.0 > left { left } else { -200.0 }, 1)
        };
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.x = x);
        let shown = nth_joint(r.game, gobj, part);
        jobj::clear_flags_all(&mut r.game.objs.jobjs, shown, jobj::HIDDEN);
    } else if anime::finished(r.game, gobj, 0, 7) {
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        let wait = bird_wait(r);
        r.game.grounds[g.0].u.oldkongo_birds().xC4 = wait;
    }
}

/// The stage's shadow check, `grOldKongo_80210788`: a fighter casts on a joint below its feet.
#[doc(alias = "grOldKongo_80210788")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
