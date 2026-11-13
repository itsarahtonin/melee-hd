//! The F-Zero race, `melee/gr/grbigblueroute.c`: Adventure's run along the Big Blue track
//! (stage kind 34, `St_Kind_BigBlueRoute`). The track (map 31) carries the camera with the first
//! fighter, kept between its left end and a band of heights, and counts the checkpoints
//! (points 5 to 7) passed as the zone (`stage_info.x6DC`); four backdrop pieces follow the first
//! fighter. The cars (map 4, made at the match's start) are thirty F-Zero machines
//! (`grFZeroCar_801CAFBC`) driven along three splines of map 33: each comes down the road, rises
//! onto the track's air line with a random roll, rides it, and drops back onto the road, over
//! and over. The first car out (the thirty-first entry) has no model: it raises the hazard sign
//! and its sound as it nears the first fighter. A car near the first fighter shakes the camera
//! and may roar, and its hurtbox item hits whoever it meets. The F-Zero cutscene (`vi0801`)
//! takes thirty of the cars onto its own racers' joints (`grBigBlueRoute_8020DAB4`).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{BIGBLUEROUTE_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, map_gobj, nth_joint, p1_fighter, point, point_joint, scale, set_cam_offset,
};
use super::vars::BigBlueRouteCar;
use super::{anime, material, setup};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `grBigBlueRoute_8020BC34`, the track's start hook: the cars' object.
const START_HOOK: u32 = 0x8020_BC34;
/// `fn_8020DEAC`, a car's hit.
pub const CAR_DEALT: u32 = 0x8020_DEAC;

/// The cars' map id.
const MAP_CARS: i32 = 4;

/// The pilots' map (`grFZeroCar_801CAFBC`'s third argument).
const PILOT_MAP: i32 = 30;

/// The cars the cars' object holds (`grBb_Route_carJObjIds` names thirty).
const CARS: usize = 30;

/// `grBb_Route_carJObjIds`: the maps of the thirty cars' models.
const CAR_MAPS: [i16; 30] = [
    4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 3, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 34, 35, 36, 37,
];

/// `grBb_Route_803B83E0`: up.
const UP: Vec3 = Vec3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// The fields of `grBigBlueRoute_YakumonoParam`, by offset.
mod p {
    /// One in this many runs gets a trophy slot (0 for always).
    pub const TROPHY_ODDS: u32 = 0;
    /// The cars' scale.
    pub const CAR_SCALE: u32 = 4;
    /// A car's speed down the road.
    pub const ROAD_SPEED: u32 = 0x20;
    /// A car's speed along the air line.
    pub const AIR_SPEED: u32 = 0x3C;
    /// How many cars come out after the first.
    pub const CARS: u32 = 0x40;
    /// The bounds of the wait before the next car.
    pub const WAIT_A: u32 = 0x44;
    pub const WAIT_B: u32 = 0x48;
    /// The first car's wait, and the hazard sign's frames.
    pub const FIRST_WAIT: u32 = 0x4C;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .bigblueroute_yakumono
            .expect("grbigblueroute.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: the race's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_BB08 | 0x8020_BB0C | 0x8020_BB60 | 0x8020_BB64 | 0x8020_BC2C | 0x8020_BC30
        | 0x8020_C13C => empty(r, gobj),
        0x8020_BABC => init_backdrop(r, gobj),
        0x8020_BB10 => init_far(r, gobj),
        0x8020_BB68 => init_demo(r, gobj),
        0x8020_BC68 => init_track(r, gobj),
        0x8020_BF38 => proc_track(r, gobj),
        START_HOOK => start_track(r, gobj),
        0x8020_C140 => init_cars(r, gobj),
        0x8020_C1DC => proc_cars(r, gobj),
        0x8020_C210 => free_cars(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_B89C => on_init(r),
        0x8020_B864 => on_demo_init(r),
        0x8020_B920 => on_load(r),
        0x8020_B95C => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grBigBlueRoute_8020BB08")]
#[doc(alias = "grBigBlueRoute_8020BB60")]
// Ports grBigBlueRoute_8020BB0C; aliased once a check reaches it.
// Ports grBigBlueRoute_8020BB64; aliased once a check reaches it.
#[doc(alias = "grBigBlueRoute_8020BC2C")]
// Ports grBigBlueRoute_8020BC30; aliased once a check reaches it.
#[doc(alias = "grBigBlueRoute_8020C13C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The objects' predicates (`callback1`), which nothing calls and answer false.
// Ports grBigBlueRoute_8020BB00; aliased once a check reaches it.
// Ports grBigBlueRoute_8020BB58; aliased once a check reaches it.
// Ports grBigBlueRoute_8020BC24; aliased once a check reaches it.
// Ports grBigBlueRoute_8020BF30; aliased once a check reaches it.
// Ports grBigBlueRoute_8020C1D4; aliased once a check reaches it.
pub fn answers_false(_gobj: GObjId) -> bool {
    false
}

/// The stage's `callback4`, `grBigBlueRoute_8020B9CC`: false.
// Ports grBigBlueRoute_8020B9CC; aliased once a check reaches it.
pub fn callback4() -> bool {
    false
}

/// The stage's line-touch dynamics, `grBigBlueRoute_8020DF78`: none.
#[doc(alias = "grBigBlueRoute_8020DF78")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stage's shadow check, `grBigBlueRoute_8020DF80`: a fighter casts a shadow on a joint it's
/// above, less 3.
#[doc(alias = "grBigBlueRoute_8020DF80")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > fp::fadds(fp::fneg(3.0), reg(b.y))
}

/// The stage's objects' setup functions and processes, by map id (`grBb_Route_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &BIGBLUEROUTE_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grBigBlueRoute_8020B9D4`, its scale
/// joint's descriptor at `frame` on the stack.
#[doc(alias = "grBigBlueRoute_8020B9D4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    // Without an object it only reports "couldn t get gobj".
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// Where the track's start hook makes the cars' object: its scale joint's descriptor on the
/// stack, under the match start (`Ground_801C0FB8`), the hook (0x18) and the create function. The
/// address is only the joint's ID table key, which nothing looks up or compares; what shows is
/// that it takes an entry of its own (a key already there would be replaced, and the table's heap
/// block not taken), which any address no other build uses gives, as the sessions' heaps confirm.
pub const START_FRAME: u32 = setup::SETUP_FRAME - 0x10;

/// The race's setup, `grBigBlueRoute_8020B89C`: its parameters, the stage's `unk8C` flags (b4
/// off, b5 on), maps 0, 32 and 31 in that order, the camera limits and blast zones, the camera
/// held off its plane, and the hazard sign's model, hidden.
#[doc(alias = "grBigBlueRoute_8020B89C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.bigblueroute_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 0x20, 0x1F] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, true);
    let desc = sign_model(r);
    r.create_route_hazard(desc.expect("un_802FD708: a model"));
    r.game.hide_hazard();
}

/// The race's demo setup, `grBigBlueRoute_8020B864`: the track destroyed if it's there, and the
/// demo's object (map 1).
#[doc(alias = "grBigBlueRoute_8020B864")]
pub fn on_demo_init(r: &mut Running) {
    let gobj = map_gobj(r.game, 0x1F);
    if gobj.is_some() {
        setup::destroy(r, gobj);
    }
    create(r, 1, setup::SETUP_FRAME);
}

/// The race's load, `grBigBlueRoute_8020B920`: the camera on the first fighter (kept to the
/// track's bounds), and there at once.
#[doc(alias = "grBigBlueRoute_8020B920")]
pub fn on_load(r: &mut Running) {
    let Some(gobj) = p1_fighter(r.game) else {
        return;
    };
    let pos = r.game.ft_pos(gobj);
    follow(r.game, pos);
    super::super::camera::snap(r);
}

/// The race's start, `grBigBlueRoute_8020B95C`: the enemy spawner with no table, and in a
/// single-player run, by a draw of one in the parameters' first word (none for 0, always taken),
/// a trophy slot at a random trophy point.
#[doc(alias = "grBigBlueRoute_8020B95C")]
pub fn on_start(r: &mut Running) {
    super::zako::start(r.game, r.data, None);
    if !r.game.is_1p_run() {
        return;
    }
    let p = param(r.game);
    let n = p.i(r.data, p::TROPHY_ODDS);
    let val = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if val == 0 {
        // The point is drawn before the trophy, as the call's arguments are evaluated.
        let point = super::ground::trophy_point(r.game, r.data);
        let trophy = super::ground::x6e4_drawn(r.game);
        super::zako::set_trophy_slot(r.game, trophy, point);
    }
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The backdrop's setup (map 0), `grBigBlueRoute_8020BABC`: its animation, drawn behind.
#[doc(alias = "grBigBlueRoute_8020BABC")]
pub fn init_backdrop(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The far background's setup (map 32), `grBigBlueRoute_8020BB10`: its animation, looping.
#[doc(alias = "grBigBlueRoute_8020BB10")]
pub fn init_far(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, false);
}

/// The demo's object's setup (map 1), `grBigBlueRoute_8020BB68`: its animation, looping, and
/// its joints 6, 3, 5, 4, 8 and 7 as the stage's guide points.
#[doc(alias = "grBigBlueRoute_8020BB68")]
pub fn init_demo(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, false);
    let j = |r: &Running, d: i32| nth_joint(r.game, gobj, d);
    let joints = [j(r, 6), j(r, 3), j(r, 5), j(r, 4), j(r, 8), j(r, 7)];
    super::ground::set_guide_points(r.game, r.data, joints);
}

/// `HSD_JObjGetTranslation` of a point's joint.
fn translation(game: &Game, j: JObjId) -> Vec3 {
    game.objs.jobjs[j].translate
}

/// The track's setup (map 31), `grBigBlueRoute_8020BC68`: its animation, it receives shadows, no
/// checkpoint passed, each of points 0x7F to 0x82's joint and its place from point 0x94 (none
/// without that point), where point 4's joint stands, and the start hook that makes the cars.
#[doc(alias = "grBigBlueRoute_8020BC68")]
pub fn init_track(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x10_flags.b5 = true;
    gp.u.bigblueroute().zone = 0;
    if let Some(origin) = point(r.game, r.data, 148) {
        for i in 0..4 {
            let j = point_joint(r.game, 127 + i as i32);
            let t = j.map(|j| translation(r.game, j));
            let v = r.game.grounds[g.0].u.bigblueroute();
            v.tracks[i].jobj = j;
            if let Some(t) = t {
                v.tracks[i].offset = t - origin;
            }
        }
    } else {
        let v = r.game.grounds[g.0].u.bigblueroute();
        for t in &mut v.tracks {
            t.jobj = None;
        }
    }
    let reb0 = point_joint(r.game, 4).expect("grbigblueroute.c:452: reb0_jobj");
    let t = translation(r.game, reb0);
    r.game.grounds[g.0].u.bigblueroute().reb0_pos = t;
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
}

/// The track's start hook, `grBigBlueRoute_8020BC34`: the cars' object (map 4).
#[doc(alias = "grBigBlueRoute_8020BC34")]
pub fn start_track(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let cars = create(r, MAP_CARS, START_FRAME);
    r.game.grounds[g.0].u.bigblueroute().cars = cars;
}

/// The track's process (map 31), `grBigBlueRoute_8020BF38`: the watched points; out of hitstun,
/// the camera on the first fighter, the first three backdrop pieces following it, the camera held
/// off its plane; the next checkpoint passed (points 5 to 7), the zone; the wind.
#[doc(alias = "grBigBlueRoute_8020BF38")]
pub fn proc_track(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let fighter = p1_fighter(r.game);
    super::ground::watch_points(r.game, None, 30.0, 4000.0);
    if let Some(fighter) = fighter {
        let pos = r.game.ft_pos(fighter);
        if !r.game.ft_is_in_hitstun(fighter) {
            follow(r.game, pos);
            for i in 0..3 {
                let t = r.game.grounds[g.0].u.bigblueroute().tracks[i];
                if let Some(j) = t.jobj {
                    let x = mem(fp::fadds(reg(t.offset.x), reg(pos.x)));
                    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
                    let y = mem(fp::fadds(reg(t.offset.y), reg(pos.y)));
                    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
                }
            }
            super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, true);
        }
        let zone = r.game.grounds[g.0].u.bigblueroute().zone;
        let idx = i32::from(zone) + 5;
        if idx <= 7
            && let Some(checkpoint) = point(r.game, r.data, idx)
            && reg(pos.x) > reg(checkpoint.x)
        {
            r.game.grounds[g.0].u.bigblueroute().zone = zone.wrapping_add(1);
        }
        r.game.stage_info.x6DC = r.game.grounds[g.0].u.bigblueroute().zone;
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The cars' setup (map 4), `grBigBlueRoute_8020C140`: the thirty cars built under its joint,
/// map 33's splines (1 the air line, 0 and 2 the road's two edges), the cars hidden at their
/// scale with their entries, and the hazard sign not shown.
#[doc(alias = "grBigBlueRoute_8020C140")]
pub fn init_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    super::fzerocar::build(r, gobj, &CAR_MAPS, PILOT_MAP, false);
    let air = super::ground::spline(r.game, r.data, 33, 1);
    let left = super::ground::spline(r.game, r.data, 33, 0);
    let right = super::ground::spline(r.game, r.data, 33, 2);
    {
        let v = r.game.grounds[g.0].u.bigblueroute_cars();
        v.air = air;
        v.left = left;
        v.right = right;
    }
    reset_cars(r, gobj);
    r.game.grounds[g.0]
        .u
        .bigblueroute_cars()
        .set_sign_shown(false);
}

/// The cars' process (map 4), `grBigBlueRoute_8020C1DC`: the next car out, then the cars driven.
#[doc(alias = "grBigBlueRoute_8020C1DC")]
pub fn proc_cars(r: &mut Running, gobj: GObjId) {
    spawn_due(r, gobj);
    drive(r, gobj);
}

/// The cars' destroy callback, `grBigBlueRoute_8020C210`: their entries back to the heap.
#[doc(alias = "grBigBlueRoute_8020C210")]
pub fn free_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let addr = r.game.grounds[g.0].u.bigblueroute_cars().cars_addr;
    crate::hsd::heap::mem_free(addr);
}

/// The point of a spline at `t`, which every caller's `t` keeps in range.
fn at(data: &Archives, spline: Option<DataRef>, t: f32) -> Vec3 {
    crate::hsd::spline::point(data, spline.expect("grbigblueroute.c: a spline"), t)
        .expect("grbigblueroute.c: a spline point in range")
}

/// The direction of a spline at `t`.
fn dir(data: &Archives, spline: Option<DataRef>, t: f32) -> Vec3 {
    crate::hsd::spline::tangent(data, spline.expect("grbigblueroute.c: a spline"), t)
        .expect("grbigblueroute.c: a spline tangent in range")
}

fn scaled(v: Vec3, s: f64) -> Vec3 {
    Vec3::new(
        mem(fp::fmuls(reg(v.x), s)),
        mem(fp::fmuls(reg(v.y), s)),
        mem(fp::fmuls(reg(v.z), s)),
    )
}

/// The cars' models hidden at the parameters' scale, their entries zeroed in a new block from the
/// heap, no car out and none due, and the ends of the air line and of the road's middle,
/// `grBigBlueRoute_8020C238`.
#[doc(alias = "grBigBlueRoute_8020C238")]
pub fn reset_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let root = jobj_of(r.game, gobj);
    if let Some(root) = root {
        let mut j = r.game.objs.jobjs[root].child;
        while let Some(c) = j {
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(c), jobj::HIDDEN);
            let s = param(r.game).f(r.data, p::CAR_SCALE);
            jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.x = s);
            let s = param(r.game).f(r.data, p::CAR_SCALE);
            jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.y = s);
            let s = param(r.game).f(r.data, p::CAR_SCALE);
            jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.z = s);
            j = r.game.objs.jobjs[c].next;
        }
    }
    let addr = crate::hsd::heap::mem_alloc(0x554);
    let (air, left, right) = {
        let v = r.game.grounds[g.0].u.bigblueroute_cars();
        (v.air, v.left, v.right)
    };
    let half = |a: Vec3, b: Vec3| scaled(a + b, 0.5);
    let air_start = at(r.data, air, 0.0);
    let air_end = at(r.data, air, 1.0);
    let road_start = half(at(r.data, left, 0.0), at(r.data, right, 0.0));
    let road_end = half(at(r.data, left, 1.0), at(r.data, right, 1.0));
    let v = r.game.grounds[g.0].u.bigblueroute_cars();
    v.cars_addr = addr;
    v.cars = vec![BigBlueRouteCar::default(); CARS + 1];
    v.wait = 0;
    v.out = 0;
    v.air_start = air_start;
    v.air_end = air_end;
    v.road_start = road_start;
    v.road_end = road_end;
}

/// A random car not yet out, or -1 with all thirty out, `grBigBlueRoute_8020C530`.
#[doc(alias = "grBigBlueRoute_8020C530")]
pub fn pick_car(r: &mut Running, gobj: GObjId) -> i32 {
    let g = ground_of(r.game, gobj);
    let free = |r: &mut Running| -> Vec<usize> {
        let v = r.game.grounds[g.0].u.bigblueroute_cars();
        (0..CARS).filter(|&i| !v.cars[i].out()).collect()
    };
    let count = free(r).len() as i32;
    if count == 0 {
        return -1;
    }
    let mut n = r.game.objs.rng.below(count);
    for i in free(r) {
        n -= 1;
        if n < 0 {
            return i as i32;
        }
    }
    panic!("grbigblueroute.c:741: 0");
}

/// The next car out when its wait is over, `grBigBlueRoute_8020C85C` with its inline
/// `grBigBlueRoute_SpawnRoute`: while fewer than one more than the parameters' count are out.
/// The first is the thirty-first entry, with no model (the sign's), and the first wait the
/// parameters'; each after a random car not yet out, the next wait random between the
/// parameters' bounds. A car starts down the road from its top at the road's speed, toward a
/// random lane, with a random roll for the air line; a modelled one is shown, unturned, with a
/// hurtbox item that hits but takes nothing.
#[doc(alias = "grBigBlueRoute_8020C85C")]
pub fn spawn_due(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let out = r.game.grounds[g.0].u.bigblueroute_cars().out;
    if !(fp::frsp(f64::from(out)) < fp::fadds(1.0, reg(p.f(r.data, p::CARS)))) {
        return;
    }
    let wait = r.game.grounds[g.0].u.bigblueroute_cars().wait;
    r.game.grounds[g.0].u.bigblueroute_cars().wait = wait.wrapping_sub(1);
    if wait >= 0 {
        return;
    }
    let idx = if out == 0 {
        let first = fp::fctiwz(reg(p.f(r.data, p::FIRST_WAIT))) as i16;
        let v = r.game.grounds[g.0].u.bigblueroute_cars();
        v.cars[CARS].set_sign(true);
        v.wait = first;
        CARS as i32
    } else {
        let idx = pick_car(r, gobj);
        if idx == -1 {
            super::super::code::unported_call(
                "grbigblueroute.c: a car out with all thirty out (the entry before the block)",
            );
        }
        r.game.grounds[g.0].u.bigblueroute_cars().cars[idx as usize].set_sign(false);
        let a = fp::fctiwz(reg(p.f(r.data, p::WAIT_A)));
        let b = fp::fctiwz(reg(p.f(r.data, p::WAIT_B)));
        let next = if b > a {
            let range = b.wrapping_sub(a);
            a.wrapping_add(if range != 0 {
                r.game.objs.rng.below(range)
            } else {
                0
            })
        } else if b < a {
            let range = a.wrapping_sub(b);
            b.wrapping_add(if range != 0 {
                r.game.objs.rng.below(range)
            } else {
                0
            })
        } else {
            b
        };
        r.game.grounds[g.0].u.bigblueroute_cars().wait = next as i16;
        idx
    };
    let i = idx as usize;
    let road_speed = p.f(r.data, p::ROAD_SPEED);
    let air_speed = p.f(r.data, p::AIR_SPEED);
    {
        let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
        c.set_out(true);
        c.set_roared(false);
        c.set_phase(1);
        c.x4 = 1.0;
        c.x8 = road_speed;
        c.xC = 0.5;
    }
    let lane = r.game.objs.rng.draw_f32();
    {
        let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
        c.x10 = lane;
        c.x14 = 0.0;
        c.x18 = air_speed;
        c.x1C = 0.0;
    }
    let roll = random_roll(r);
    r.game.grounds[g.0].u.bigblueroute_cars().cars[i].x20 = roll;
    if !r.game.grounds[g.0].u.bigblueroute_cars().cars[i].sign() {
        let root = jobj_of(r.game, gobj);
        if let Some(root) = root {
            let mut j = r.game.objs.jobjs[root].child;
            if j.is_some() {
                let mut k = 0;
                while k < idx {
                    let Some(c) = j else { break };
                    k += 1;
                    j = r.game.objs.jobjs[c].next;
                }
                let car = j.expect("grbigblueroute.c:901: jobj");
                jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(car), jobj::HIDDEN);
                jobj::set_rotation_x(&mut r.game.objs.jobjs, car, 0.0);
                jobj::set_rotation_y(&mut r.game.objs.jobjs, car, 0.0);
                jobj::set_rotation_z(&mut r.game.objs.jobjs, car, 0.0);
                let item = material::stage_item(
                    r,
                    0,
                    1,
                    g,
                    Some(car),
                    Some(CodeRef(CAR_DEALT)),
                    None,
                    None,
                );
                r.game.grounds[g.0].u.bigblueroute_cars().cars[i].item = item;
                if let Some(item) = item {
                    material::item_hurtbox_off(r.game, item);
                }
            }
        }
    }
    let v = r.game.grounds[g.0].u.bigblueroute_cars();
    v.out = v.out.wrapping_add(1);
}

/// A random roll for the air line, within 150 degrees either way.
fn random_roll(r: &mut Running) -> f32 {
    let rand = r.game.objs.rng.draw_f32();
    mem(fp::fmsubs(reg(5.235_987_7), reg(rand), reg(2.617_993_8)))
}

/// A point on the road at `t` between its edges by `frac` (0 the left), and the road's heading
/// there as a car's angles.
fn on_road(
    r: &Running,
    left: Option<DataRef>,
    right: Option<DataRef>,
    t: f32,
    frac: f32,
) -> (Vec3, Vec3) {
    let p0 = at(r.data, left, t);
    let p1 = at(r.data, right, t);
    let d = dir(r.data, left, t);
    let pos = scaled(p1 - p0, reg(frac)) + p0;
    let rot = Vec3::new(
        mem(fp::fneg(reg(crate::math::atan2(d.y, d.x)))),
        1.570_796_4,
        0.0,
    );
    (pos, rot)
}

/// A point on the air line at `t` rolled by `angle`, 45 (at the stage's scale) above the line
/// along the car's up, and the car's angles there; the inline frame both the air cases build.
fn on_air(r: &Running, air: Option<DataRef>, t: f32, angle: f32) -> (Vec3, Vec3) {
    let pos = at(r.data, air, t);
    let mut along = dir(r.data, air, t);
    unit_clamped(&mut along);
    let mut side = UP.sdk_cross(along);
    side.rotate_about(along, angle);
    unit_clamped(&mut side);
    let mut up = along.sdk_cross(side);
    unit_clamped(&mut up);
    let rot = super::ground::euler_from_axes(along, side, up);
    let s = fp::fmuls(45.0, reg(scale(r.game)));
    (scaled(up, s) + pos, rot)
}

/// Drives the cars, `grBigBlueRoute_8020CD20` (with the first fighter only): each car out
/// moves on along the road and the air line at its speeds, then by its phase: 0 down the road
/// easing toward its lane, until near its end it rises (2) from the road onto the air line, which
/// it rides (1) rolling toward its roll, until near its end it drops (3) back onto the road's
/// start; a modelled car's joint follows. A modelled car within 260 of the first fighter (x and
/// z) plays the approach sound once until a car drops; within 100 it shakes the camera and, the
/// first time, roars two times in five; the sign's car within 100 raises the hazard sign once
/// until it's left behind.
#[doc(alias = "grBigBlueRoute_8020CD20")]
pub fn drive(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let Some(fighter) = p1_fighter(r.game) else {
        return;
    };
    let fpos = r.game.ft_pos(fighter);
    let Some(root) = jobj_of(r.game, gobj) else {
        return;
    };
    let mut j = r.game.objs.jobjs[root].child;
    if j.is_none() {
        return;
    }
    let (air, left, right) = {
        let v = r.game.grounds[g.0].u.bigblueroute_cars();
        (v.air, v.left, v.right)
    };
    let one = reg(1.0);
    for i in 0..=CARS {
        if r.game.grounds[g.0].u.bigblueroute_cars().cars[i].out() {
            let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
            c.x4 = mem(fp::fadds(reg(c.x4), reg(c.x8)));
            if reg(c.x4) > one {
                c.x4 = 1.0;
            }
            c.x14 = mem(fp::fadds(reg(c.x14), reg(c.x18)));
            if reg(c.x14) > one {
                c.x14 = 1.0;
            }
            let (pos, rot) = match c.phase() {
                0 => {
                    if reg(c.xC) < reg(c.x10) {
                        c.xC = mem(fp::fadds(reg(c.xC), reg(0.01)));
                        if reg(c.xC) > reg(c.x10) {
                            c.xC = c.x10;
                        }
                    } else if reg(c.xC) > reg(c.x10) {
                        c.xC = mem(fp::fsubs(reg(c.xC), reg(0.01)));
                        if reg(c.xC) < reg(c.x10) {
                            c.xC = c.x10;
                        }
                    }
                    let (t, frac) = (c.x4, c.xC);
                    let out = on_road(r, left, right, t, frac);
                    let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                    if reg(c.x4) > reg(0.966) {
                        c.set_phase(2);
                        c.x14 = 0.0;
                        c.x1C = 0.0;
                        let roll = random_roll(r);
                        let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                        c.x20 = roll;
                        c.x24 = c.x4;
                    }
                    out
                }
                1 => {
                    let step = reg(0.017_453_292);
                    if reg(c.x1C) < reg(c.x20) {
                        c.x1C = mem(fp::fadds(reg(c.x1C), step));
                        if reg(c.x1C) > reg(c.x20) {
                            c.x1C = c.x20;
                        }
                    } else if reg(c.x1C) > reg(c.x20) {
                        c.x1C = mem(fp::fsubs(reg(c.x1C), step));
                        if reg(c.x1C) < reg(c.x20) {
                            c.x1C = c.x20;
                        }
                    }
                    let (t, angle) = (c.x14, c.x1C);
                    let out = on_air(r, air, t, angle);
                    let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                    if reg(c.x14) > reg(0.967) {
                        c.set_phase(3);
                        c.x4 = 0.0;
                        c.xC = 0.5;
                        let lane = r.game.objs.rng.draw_f32();
                        let v = r.game.grounds[g.0].u.bigblueroute_cars();
                        let c = &mut v.cars[i];
                        c.x10 = lane;
                        c.x24 = c.x14;
                        c.set_roared(false);
                        if c.sign() {
                            v.set_approached(false);
                        }
                    }
                    out
                }
                2 => {
                    let t = c.x4;
                    let prog = fp::fdivs(fp::fsubs(reg(t), reg(c.x24)), fp::fsubs(one, reg(c.x24)));
                    let frac = fp::fmadds(fp::fsubs(one, prog), fp::fsubs(reg(c.xC), 0.5), 0.5);
                    let (t14, angle) = (c.x14, c.x1C);
                    let (road, road_rot) = on_road(r, left, right, t, mem(frac));
                    let (air_pos, _) = on_air(r, air, t14, angle);
                    let pos = scaled(air_pos - road, prog) + road;
                    let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                    if reg(c.x4) == one {
                        c.set_phase(1);
                    }
                    (pos, road_rot)
                }
                3 => {
                    let t = c.x14;
                    let prog = fp::fdivs(fp::fsubs(reg(t), reg(c.x24)), fp::fsubs(one, reg(c.x24)));
                    let angle = mem(fp::fmuls(fp::fsubs(one, prog), reg(c.x1C)));
                    let (air_pos, air_rot) = on_air(r, air, t, angle);
                    let c = &r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                    let (road_t, frac) = (c.x4, c.xC);
                    // The road's heading is computed and thrown away (`atan2f`, pure).
                    let (road, _) = on_road(r, left, right, road_t, frac);
                    let pos = scaled(road - air_pos, prog) + air_pos;
                    let c = &mut r.game.grounds[g.0].u.bigblueroute_cars().cars[i];
                    if reg(c.x14) == one {
                        c.set_phase(0);
                    }
                    (pos, air_rot)
                }
                other => panic!("grbigblueroute.c: a car's phase {other} reads unset stack slots"),
            };
            let sign = r.game.grounds[g.0].u.bigblueroute_cars().cars[i].sign();
            if !sign {
                let car = j.expect("grbigblueroute.c:916: jobj");
                jobj::set_translate(&mut r.game.objs.jobjs, car, pos);
                jobj::set_rotation_x(&mut r.game.objs.jobjs, car, rot.x);
                jobj::set_rotation_y(&mut r.game.objs.jobjs, car, rot.y);
                jobj::set_rotation_z(&mut r.game.objs.jobjs, car, rot.z);
            }
            let near = |d: f64| {
                fp::fsubs(reg(pos.x), d) < reg(fpos.x)
                    && reg(fpos.x) < fp::fadds(d, reg(pos.x))
                    && fp::fsubs(reg(pos.z), d) < reg(fpos.z)
                    && reg(fpos.z) < fp::fadds(d, reg(pos.z))
            };
            if near(260.0) && !sign && !r.game.grounds[g.0].u.bigblueroute_cars().approached() {
                r.game.grounds[g.0]
                    .u
                    .bigblueroute_cars()
                    .set_approached(true);
                r.stage_sound(0x77A16);
            }
            if near(100.0) {
                if sign {
                    if !r.game.grounds[g.0].u.bigblueroute_cars().sign_shown() {
                        let frames = fp::fctiwz(reg(param(r.game).f(r.data, p::FIRST_WAIT)));
                        r.game.show_hazard(r.data, frames);
                        r.stage_sound(0x77A11);
                        r.game.grounds[g.0]
                            .u
                            .bigblueroute_cars()
                            .set_sign_shown(true);
                    }
                } else {
                    super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
                    if !r.game.grounds[g.0].u.bigblueroute_cars().cars[i].roared() {
                        r.game.grounds[g.0].u.bigblueroute_cars().cars[i].set_roared(true);
                        if r.game.objs.rng.below(100) < 40 {
                            match r.game.objs.rng.below(3) {
                                0 => r.stage_sound(0x77A15),
                                1 => r.stage_sound(0x77A14),
                                2 => r.stage_sound(0x77A13),
                                _ => {}
                            }
                        }
                    }
                }
            } else if sign && r.game.grounds[g.0].u.bigblueroute_cars().sign_shown() {
                r.game.grounds[g.0]
                    .u
                    .bigblueroute_cars()
                    .set_sign_shown(false);
            }
        }
        j = next_joint(r.game, j);
    }
}

/// A joint's next sibling, or none for none, `grBigBlueRoute_8020DA9C`.
#[doc(alias = "grBigBlueRoute_8020DA9C")]
pub fn next_joint(game: &Game, j: Option<JObjId>) -> Option<JObjId> {
    game.objs.jobjs[j?].next
}

/// Puts random cars on joints, `grBigBlueRoute_8020DAB4`: for each joint of `joints` up to
/// `count` (stopping at the first missing), a random one of the cars left in a new cars'
/// object, shown at `scale`, moved under it; then the object is destroyed (the cars moved stay).
/// The F-Zero cutscene's racers.
#[doc(alias = "grBigBlueRoute_8020DAB4")]
pub fn cars_on_joints(r: &mut Running, joints: &[Option<JObjId>], s: f32, count: i32, frame: u32) {
    let gobj = create(r, MAP_CARS, frame).expect("grbigblueroute.c:1414: gobj");
    let mut left = 30;
    for &joint in joints.iter().take(count as usize) {
        let Some(target) = joint else { break };
        let mut n = if left != 0 {
            r.game.objs.rng.below(left)
        } else {
            0
        };
        let root = jobj_of(r.game, gobj).expect("grbigblueroute.c:1434: jobj");
        let mut j = r.game.objs.jobjs[root].child;
        while n != 0 {
            let c = j.expect("grbigblueroute.c:1437: jobj");
            n -= 1;
            j = r.game.objs.jobjs[c].next;
        }
        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        let c = j.expect("grbigblueroute.c: a car");
        jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.x = s);
        jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.y = s);
        jobj::set_srt(&mut r.game.objs.jobjs, c, |o| o.scale.z = s);
        jobj::reparent(&mut r.game.objs.jobjs, j, Some(target));
        left -= 1;
    }
    setup::destroy(r, Some(gobj));
}

/// Normalizes a vector, any component past ±1 made the unit vector along it, and its size before,
/// `grBigBlueRoute_8020DD64`.
#[doc(alias = "grBigBlueRoute_8020DD64")]
pub fn unit_clamped(v: &mut Vec3) -> f32 {
    let mag = v.normalize();
    let x = reg(v.x);
    if x > 1.0 {
        *v = Vec3::new(1.0, 0.0, 0.0);
    } else if x < fp::fneg(1.0) {
        *v = Vec3::new(-1.0, 0.0, 0.0);
    } else if reg(v.y) > 1.0 {
        *v = Vec3::new(0.0, 1.0, 0.0);
    } else if reg(v.y) < fp::fneg(1.0) {
        *v = Vec3::new(0.0, -1.0, 0.0);
    } else if reg(v.z) > 1.0 {
        *v = Vec3::new(0.0, 0.0, 1.0);
    } else if reg(v.z) < fp::fneg(1.0) {
        *v = Vec3::new(0.0, 0.0, -1.0);
    }
    mag
}

/// The hazard sign's model: map 2's entry, `grBigBlueRoute_8020DE48`.
#[doc(alias = "grBigBlueRoute_8020DE48")]
pub fn sign_model(r: &Running) -> Option<DataRef> {
    let head = super::datfiles::archive_for(r.game, r.data, 2);
    let head = head.expect("grbigblueroute.c: 1495: archive");
    r.data.ptr(head.0.at(8))?;
    Some(head.entry(r.data, 2).0)
}

/// A car's hit, `fn_8020DEAC`: its sound.
#[doc(alias = "fn_8020DEAC")]
pub fn car_dealt(r: &mut Running) {
    r.stage_sound(0x77A12);
}

/// The camera on `pos`, kept right of -1140 and between 20 and 250 high (at the stage's scale),
/// `grBigBlueRoute_8020DED4`.
#[doc(alias = "grBigBlueRoute_8020DED4")]
pub fn follow(game: &mut Game, pos: Vec3) {
    let s = || reg(scale(game));
    let mut x = reg(pos.x);
    let mut y = reg(pos.y);
    let left = fp::fmuls(fp::frsp(-1140.0), s());
    if x < left {
        x = fp::fmuls(fp::frsp(-1140.0), s());
    }
    if y < fp::fmuls(20.0, s()) {
        y = fp::fmuls(20.0, s());
    } else if y > fp::fmuls(fp::frsp(250.0), s()) {
        y = fp::fmuls(fp::frsp(250.0), s());
    }
    set_cam_offset(game, mem(x), mem(y));
}
