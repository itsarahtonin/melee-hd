//! Yoshi's Island (the N64 one), `melee/gr/groldyoshi.c`: the stage (map 4), its three clouds
//! (map 2), the creatures that pass behind it now and then (map 3), and the background and
//! foreground (maps 1 and 5).
//!
//! A cloud a fighter stands on sinks; stood on long enough, it puffs away (its collision goes,
//! its model hides), comes back after a while, and rises to where it was.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{OLDYOSHI_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The clouds' map id.
pub const MAP_CLOUDS: i32 = 2;

/// `fn_8020F2A8`, the clouds' collision joints' callback: a fighter stands on one.
pub const CLOUD_TOUCHED: u32 = 0x8020_F2A8;

/// Each cloud's collision joint and model part (`grOy_803E6574`).
const CLOUDS: [(i32, i32); 3] = [(0, 1), (1, 5), (2, 9)];

/// The creatures' parts, one drawn at random (`grOy_803B83F0`).
const GUESTS: [i16; 5] = [1, 2, 3, 4, 5];

/// The fields of `groldyoshi_yakumono_param_t` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn h(self, r: &Running, off: u32) -> i16 {
        r.data.u16(self.0.at(off)) as i16
    }
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .oldyoshi_yakumono
            .expect("groldyoshi.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Yoshi's Island N64's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_E970 | 0x8020_E974 | 0x8020_E9E8 | 0x8020_E9EC | 0x8020_EA88 | 0x8020_EAF4
        | 0x8020_EAF8 | 0x8020_EFC8 | 0x8020_F2A4 => empty(r, gobj),
        0x8020_E93C => init_main(r, gobj),
        0x8020_E978 => init_back(r, gobj),
        0x8020_E9F0 => init_stage(r, gobj),
        0x8020_EA64 => proc_stage(r, gobj),
        0x8020_EA8C => init_front(r, gobj),
        0x8020_EAFC => init_clouds(r, gobj),
        0x8020_EC10 => proc_clouds(r, gobj),
        0x8020_EFCC => init_guest(r, gobj),
        0x8020_F088 => proc_guest(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_E79C => on_init(r),
        0x8020_E798 => on_demo_init(r),
        0x8020_E824 => on_load(r),
        0x8020_E828 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grOldYoshi_8020E970")]
#[doc(alias = "grOldYoshi_8020E974")]
#[doc(alias = "grOldYoshi_8020E9E8")]
#[doc(alias = "grOldYoshi_8020E9EC")]
#[doc(alias = "grOldYoshi_8020EA88")]
#[doc(alias = "grOldYoshi_8020EAF4")]
#[doc(alias = "grOldYoshi_8020EAF8")]
#[doc(alias = "grOldYoshi_8020EFC8")]
#[doc(alias = "grOldYoshi_8020F2A4")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grOldYoshi_8020E854`.
#[doc(alias = "grOldYoshi_8020E854")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grOy_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &OLDYOSHI_CALLBACKS[map_id as usize]
}

/// Yoshi's Island N64's setup, `grOldYoshi_8020E79C`: its parameters, maps 0, 1, 4, 5, 2 and
/// 3, camera limits and blast zones.
#[doc(alias = "grOldYoshi_8020E79C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.oldyoshi_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 1, 4, 5, 2, 3] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grOldYoshi_8020E798` does nothing (only a demo runs it).
#[doc(alias = "grOldYoshi_8020E798")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grOldYoshi_8020E824` does nothing.
#[doc(alias = "grOldYoshi_8020E824")]
pub fn on_load(_r: &mut Running) {}

/// Yoshi's Island N64's start, `grOldYoshi_8020E828`: the enemy spawner, without enemies.
#[doc(alias = "grOldYoshi_8020E828")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@8020E93C")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 1), `grOldYoshi_8020E978`: its animation, and its materials
/// marked with render-mode bits 27, then 29.
#[doc(alias = "grOldYoshi_8020E978")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    material::set_tree_flags(r.game, j, 1 << 27);
    material::set_tree_flags(r.game, j, 1 << 29);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// A map's collision and animation (the inline `Ground_InitMapCollAndAnim`).
fn init_coll_and_anim(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 4), `grOldYoshi_8020E9F0`: its collision and animation, and it
/// receives shadows.
#[doc(alias = "grOldYoshi_8020E9F0")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    init_coll_and_anim(r, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 1;
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The stage's process, `grOldYoshi_8020EA64`: the collision follows the model, and the wind
/// sources.
#[doc(alias = "grOldYoshi_8020EA64")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The foreground's setup (map 5), `grOldYoshi_8020EA8C`: its collision and animation.
#[doc(alias = "grOldYoshi_8020EA8C")]
pub fn init_front(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    init_coll_and_anim(r, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The clouds' setup (map 2), `grOldYoshi_8020EAFC`: their collision and animation, each
/// cloud at rest with its model and height, and its collision joint telling it when a fighter
/// stands on it.
#[doc(alias = "grOldYoshi_8020EAFC")]
pub fn init_clouds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    for (i, &(coll, part)) in CLOUDS.iter().enumerate() {
        let model = nth_joint(r.game, gobj, part);
        let y = r.game.objs.jobjs[model.expect("jobj")].translate.y;
        let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
        c.set_state(0);
        c.set_touched(false);
        c.set_count(0);
        c.xD4 = 0.0;
        c.xD0 = 0.0;
        c.jobj = model;
        c.xCC = y;
        r.game.geometry_mut().set_ledge_callback(
            coll,
            Some(super::CollUser::Ground(g)),
            Some(CodeRef(CLOUD_TOUCHED)),
        );
    }
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The clouds' process, `grOldYoshi_8020EC10`. A cloud at rest (state 0) sinks while stood on,
/// toward the parameters' depth, and rises back otherwise; stood on for more than its ticks,
/// it puffs (state 1: its collision goes partway in, its model hides at the end), stays gone
/// (state 2), and comes back (state 3: shown, its collision back partway in). Then its model
/// sits as deep as it sank, and the collision follows the models.
#[doc(alias = "grOldYoshi_8020EC10")]
pub fn proc_clouds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let (puff_after, gone_for, puff_coll, back_coll) = (
        i32::from(p.h(r, 0)),
        i32::from(p.h(r, 2)),
        i32::from(p.h(r, 0x10)),
        i32::from(p.h(r, 0x12)),
    );
    let (max, rate, deepest) = (p.f(r, 4), p.f(r, 8), p.f(r, 0xC));
    for (i, &(coll, part)) in CLOUDS.iter().enumerate() {
        let c = &r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
        match c.state() {
            0 => {
                let (speed, depth) = (c.xD4, c.xD0);
                if c.touched() {
                    let v = approach(
                        reg(speed),
                        reg(rate),
                        reg(depth),
                        reg(deepest),
                        reg(max),
                        fp::fneg(reg(max)),
                    );
                    let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                    c.xD4 = mem(v);
                    c.xD0 = mem(fp::fadds(reg(c.xD0), reg(c.xD4)));
                    if reg(c.xD0) > reg(deepest) {
                        c.xD0 = deepest;
                    }
                    let n = c.count();
                    c.set_count(n + 1);
                    if n as i32 > puff_after {
                        c.set_state(1);
                        anime::start(r.game, r.data, gobj, part, 2, 1, 0.0, 1.0, true);
                        let model = r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].jobj;
                        grlib::generator_oriented_on(r.game, r.data, 30000, 30, model);
                        r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].set_count(0);
                    }
                } else {
                    let v = approach(
                        reg(speed),
                        reg(rate),
                        reg(depth),
                        0.0,
                        reg(max),
                        fp::fneg(reg(max)),
                    );
                    let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                    c.xD4 = mem(v);
                    c.xD0 = mem(fp::fadds(reg(c.xD0), reg(c.xD4)));
                    if reg(c.xD0) < 0.0 {
                        c.xD0 = 0.0;
                    }
                    c.set_count(0);
                }
            }
            1 => {
                if c.count() as i32 == puff_coll {
                    r.game.geometry_mut().disable_joint(coll);
                }
                let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                c.set_count(c.count() + 1);
                if anime::finished(r.game, gobj, part, 2) {
                    r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].set_state(2);
                    r.game.geometry_mut().disable_joint(coll);
                    let model = r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].jobj;
                    jobj::set_flags_all(&mut r.game.objs.jobjs, model, jobj::HIDDEN);
                    r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].set_count(0);
                }
            }
            2 => {
                let n = c.count();
                let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                c.set_count(n + 1);
                if n as i32 > gone_for {
                    c.set_state(3);
                    anime::start(r.game, r.data, gobj, part, 2, 2, 0.0, 1.0, true);
                    r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i].set_count(0);
                }
            }
            3 => {
                let (n, model) = (c.count(), c.jobj);
                if n == 0 {
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, model, jobj::HIDDEN);
                }
                if n as i32 == back_coll {
                    r.game.geometry_mut().enable_joint(coll);
                }
                let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                c.set_count(c.count() + 1);
                if anime::finished(r.game, gobj, part, 2) {
                    let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
                    c.set_state(0);
                    c.set_count(0);
                }
            }
            _ => {}
        }
        let c = &mut r.game.grounds[g.0].u.oldyoshi_cloud().cloud[i];
        let y = mem(fp::fsubs(reg(c.xCC), reg(c.xD0)));
        let model = c.jobj.expect("jobj");
        c.set_touched(false);
        jobj::set_srt(&mut r.game.objs.jobjs, model, |o| o.translate.y = y);
    }
    update_map_coll(r.game, r.data, gobj);
}

/// A cloud's sinking speed's next step toward `target`, `grOldYoshi_8020F31C`: braking by
/// `accel` once the distance a stop at that rate takes reaches the distance left (and the
/// speed is toward it), else speeding up toward it, within `max` and `min`.
#[doc(alias = "grOldYoshi_8020F31C")]
pub fn approach(speed: f64, accel: f64, pos: f64, target: f64, max: f64, min: f64) -> f64 {
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    let t = fp::fdivs(abs(speed), accel);
    let stop = fp::fmadds(
        t,
        fp::fmuls(fp::fmuls(0.5, fp::fneg(accel)), t),
        fp::fmuls(t, abs(speed)),
    );
    let left = abs(fp::fsubs(target, pos));
    let mut v = speed;
    if left < stop && ((target > pos && v > 0.0) || (target < pos && v < 0.0)) {
        if v > 0.0 {
            v = fp::fsubs(v, accel);
        } else if v < 0.0 {
            v = fp::fadds(v, accel);
        }
    } else if target > pos {
        v = fp::fadds(v, accel);
    } else if target < pos {
        v = fp::fsubs(v, accel);
    }
    if v > max {
        return max;
    }
    if v < min {
        return min;
    }
    v
}

/// The clouds' collision joints' callback, `fn_8020F2A8`: a fighter (collision kind 1) on a
/// cloud's joint marks the cloud stood on.
#[doc(alias = "fn_8020F2A8")]
pub fn cloud_touched(game: &mut Game, cloud: Option<GroundId>, joint_id: i32, kind: u32) {
    let Some(g) = cloud else { return };
    touch(&mut game.grounds[g.0], joint_id, kind);
}

/// [`cloud_touched`] on the cloud's `Ground`, for callers that hold the pool alone.
pub fn touch(gp: &mut super::super::structs::Ground, joint_id: i32, kind: u32) {
    if kind != 1 {
        return;
    }
    let Some(i) = CLOUDS.iter().position(|&(coll, _)| coll == joint_id) else {
        return;
    };
    gp.u.oldyoshi_cloud().cloud[i].set_touched(true);
}

/// The creatures' setup (map 3), `grOldYoshi_8020EFCC`: hidden, a random wait, none passing.
#[doc(alias = "grOldYoshi_8020EFCC")]
pub fn init_guest(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let wait = guest_wait(r);
    let v = r.game.grounds[g.0].u.oldyoshi_guest();
    v.xC4 = wait as i16;
    v.xC6 = -1;
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// A wait between the parameters' two bounds.
fn guest_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let (a, b) = (i32::from(p.h(r, 0x16)), i32::from(p.h(r, 0x14)));
    grlib::rand_range(&mut r.game.objs.rng, a, b)
}

/// The creatures' process, `grOldYoshi_8020F088`. While none passes, the wait counts down
/// (once over, one of the five is drawn), and the model starts over at a random height each
/// tick. One passing is shown until its animation ends; then it hides and a new wait starts.
#[doc(alias = "grOldYoshi_8020F088")]
pub fn proc_guest(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.oldyoshi_guest();
    if v.xC6 == -1 {
        let left = v.xC4;
        v.xC4 = left.wrapping_sub(1);
        if left < 0 {
            let k = r.game.objs.rng.below(5);
            r.game.grounds[g.0].u.oldyoshi_guest().xC6 = GUESTS[k as usize];
        }
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, 0);
        let f = reg(r.game.objs.rng.draw_f32());
        let spread = fp::frsp(f64::from(param(r.game).h(r, 0x18)));
        let y = mem(fp::fmuls(spread, fp::fmsubs(2.0, f, 1.0)));
        let j = jobj_of(r.game, gobj).expect("jobj");
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    } else {
        let part = v.xC6;
        let Some(j) = nth_joint(r.game, gobj, i32::from(part)) else {
            return;
        };
        if jobj::get_flags(&r.game.objs.jobjs, Some(j)) & jobj::HIDDEN != 0 {
            jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        }
        if anime::finished(r.game, gobj, 0, 7) {
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            let wait = guest_wait(r);
            let v = r.game.grounds[g.0].u.oldyoshi_guest();
            v.xC4 = wait as i16;
            v.xC6 = -1;
        }
    }
}

/// The stage's shadow check, `grOldYoshi_8020F40C`: a fighter casts on a joint below a point
/// five units above its feet.
#[doc(alias = "grOldYoshi_8020F40C")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > fp::fadds(fp::fneg(5.0), reg(b.y))
}
