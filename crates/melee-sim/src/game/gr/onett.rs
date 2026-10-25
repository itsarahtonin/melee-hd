//! Onett, `melee/gr/gronett.c`: the street with its awnings (map 5), the drugstore whose roof
//! gives way (map 4), the cars (map 3), and maps 0 and 2.
//!
//! The two awnings sag and spring back under the fighters who land on them. The drugstore's
//! roof cracks after enough landings and, after more, falls in, its platforms gone a while
//! before it stands again, blinking. Now and then a car drives through from the right, its
//! sign up first, and runs into the fighters on the street (each a hurtbox item,
//! `it::kinds::yaku`); another drives behind from the left. The chimney smokes at times.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::camera;
use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{ONETT_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, set_guide_points, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `grOnett_801E54B4`, the awnings' collision joints' callback (`mpJointSetCb1`).
pub const AWNING_LANDED: u32 = 0x801E_54B4;
/// `grOnett_801E40E4`, the drugstore roof's collision joints' callback (`mpJointSetCb1`).
pub const ROOF_LANDED: u32 = 0x801E_40E4;
/// `grOnett_801E5030`, a car's hurtbox item's callback for a hit it deals.
pub const CAR_HIT: u32 = 0x801E_5030;
/// `grOnett_801E3930` and `grOnett_801E41B0`, the street's and the cars' start hooks.
const STREET_START: u32 = 0x801E_3930;
const CARS_START: u32 = 0x801E_41B0;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

/// Where a car rests and turns back, off either end of the street, and where the left car
/// sounds its horn.
const RIGHT_END: f32 = 726.0;
const LEFT_END: f32 = -642.0;
const LEFT_HORN: f32 = -513.0;
/// Where the right car's sign and horn come up.
const RIGHT_HORN: f32 = 580.0;

/// The fields of `grOnett_StageParam`, by offset.
mod p {
    pub const AWNING_INITIAL: u32 = 0;
    pub const MAX_VELOCITY: u32 = 4;
    pub const VEL_THRESHOLD: u32 = 8;
    pub const POS_THRESHOLD: u32 = 0xC;
    pub const DAMPING: u32 = 0x10;
    pub const SPRING_FORCE: u32 = 0x14;
    pub const SPRING_CONSTANT: u32 = 0x18;
    pub const MAX_DISPLACEMENT: u32 = 0x1C;
    pub const AWNING_DELTA: u32 = 0x20;
    /// The chimney's waits between smokes and its smokes' lengths.
    pub const SMOKE_WAIT_MAX: u32 = 0x24;
    pub const SMOKE_WAIT_MIN: u32 = 0x28;
    pub const SMOKE_MAX: u32 = 0x2C;
    pub const SMOKE_MIN: u32 = 0x30;
    /// The roof's landings before it cracks and before it falls.
    pub const CRACK_HITS: u32 = 0x34;
    pub const FALL_HITS: u32 = 0x38;
    /// How long the fallen roof stays down.
    pub const DOWN_MIN: u32 = 0x3C;
    pub const DOWN_MAX: u32 = 0x40;
    /// A car's wait before it comes, its wait before it drives, and its waits once a fighter
    /// is on the street (by the fighter's standing).
    pub const CAR_WAIT: u32 = 0x44;
    pub const CAR_DRIVE_WAIT: u32 = 0x48;
    pub const CAR_WAIT_OTHERS: u32 = 0x4C;
    pub const CAR_WAIT_LAST: u32 = 0x50;
    /// A car's speed and its spread.
    pub const CAR_SPEED: u32 = 0x54;
    pub const CAR_SPEED_ADD: u32 = 0x58;
    /// A struck car's wheels' spin, in degrees.
    pub const CAR_SPIN: u32 = 0x5C;
    /// The sign's time up and where the horn sounds, past the camera's right edge.
    pub const SIGN_TIME: u32 = 0x60;
    pub const HORN_X: u32 = 0x64;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        fp::fctiwz(reg(self.f(data, off)))
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.onett_yakumono.expect("gronett.c: yakumono_param"))
}

/// A stage object's process or callback, by address: Onett's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801E_3928 | 0x801E_392C | 0x801E_3CE0 | 0x801E_40E0 | 0x801E_502C => empty(r, gobj),
        0x801E_38DC => init_back(r, gobj),
        STREET_START => start_street(r, gobj),
        0x801E_3A34 => init_street(r, gobj),
        0x801E_3C60 => proc_street(r, gobj),
        0x801E_3CE4 => init_store(r, gobj),
        0x801E_3DA0 => proc_store(r, gobj),
        CARS_START => start_cars(r, gobj),
        0x801E_41C8 => init_cars(r, gobj),
        0x801E_43E0 => proc_cars(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801E_3738 => on_init(r),
        0x801E_37C4 => on_load(r),
        0x801E_37C8 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grOnett_801E3928")]
#[doc(alias = "grOnett_801E392C")]
#[doc(alias = "grOnett_801E3CE0")]
#[doc(alias = "grOnett_801E40E0")]
#[doc(alias = "grOnett_801E502C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grOnett_801E37F4`.
#[doc(alias = "grOnett_801E37F4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grOt_803E27E0`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &ONETT_CALLBACKS[map_id as usize]
}

/// Onett's setup, `grOnett_801E3738`: its parameters, maps 0, 2, 5, 4 and 3, the car sign's
/// model (moved right) hidden, camera limits and blast zones.
#[doc(alias = "grOnett_801E3738")]
pub fn on_init(r: &mut Running) {
    r.game.gr.onett_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 2, 5, 4, 3] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    let desc = sign_model(r);
    r.create_onett_hazard(desc.expect("un_802FD708: a model"));
    r.game.hide_hazard();
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grOnett_801E3734` does nothing (only a demo runs it).
#[doc(alias = "grOnett_801E3734")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grOnett_801E37C4` does nothing.
#[doc(alias = "grOnett_801E37C4")]
pub fn on_load(_r: &mut Running) {}

/// Onett's start, `grOnett_801E37C8`: the enemy spawner, without enemies.
#[doc(alias = "grOnett_801E37C8")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The car sign's model, `grOnett_801E56FC`: map 1's entry in its archive.
#[doc(alias = "grOnett_801E56FC")]
pub fn sign_model(r: &Running) -> Option<DataRef> {
    let head = super::datfiles::archive_for(r.game, r.data, 1);
    let head = head.expect("gronett.c: 1319: archive");
    r.data.ptr(head.0.at(8))?;
    Some(head.entry(r.data, 1).0)
}

/// The stage's lines' hazards, `grOnett_801E5760`: none.
#[doc(alias = "grOnett_801E5760")]
pub fn line_hazard() -> Option<DataRef> {
    None
}

/// Whether a joint takes a fighter's shadow, `grOnett_801E5768`: the fighter is above it.
#[doc(alias = "grOnett_801E5768")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    j: Option<jobj::JObjId>,
) -> bool {
    let v = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(v.y)
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 2's setup, `grOnett_801E38DC`: its animation, drawn in the background layer.
#[doc(alias = "grOnett_801E38DC")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The street's start hook, `grOnett_801E3930`: the awnings' callbacks, and a camera subject
/// for the cars, its extent set and inactive.
#[doc(alias = "grOnett_801E3930")]
pub fn start_street(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let user = Some(super::CollUser::Ground(g));
    for joint in [0, 1] {
        r.game
            .geometry_mut()
            .set_ledge_callback(joint, user, Some(CodeRef(AWNING_LANDED)));
    }
    let b = camera::alloc_box(&mut r.game.camera.boxes, camera::ACTIVE);
    r.game.grounds[g.0].u.onett().subject = Some(b);
    let cam = &mut r.game.camera.boxes[b];
    cam.target_ext.h.x = -40.0;
    cam.target_ext.h.y = 40.0;
    cam.target_ext.v.x = 20.0;
    cam.target_ext.v.y = -20.0;
    cam.target_ext.v.z = 1.0;
    cam.pos = Vec3::new(0.0, 0.0, 0.0);
    cam.bone_pos = cam.pos;
    cam.state = camera::INACTIVE;
}

/// The street's setup (map 5), `grOnett_801E3A34`: its collision and animation, no camera
/// subject yet, the awnings at rest on their joints (14 and 15), a wait before the chimney
/// smokes, the guide points, and its start hook.
#[doc(alias = "grOnett_801E3A34")]
pub fn init_street(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b5 = true;
    r.game.grounds[g.0].u.onett().subject = None;
    for (i, part) in [(0, 0xE), (1, 0xF)] {
        let aj = nth_joint(r.game, gobj, part);
        let y = r.game.objs.jobjs[aj.expect("gronett.c: an awning's joint")]
            .translate
            .y;
        let a = &mut r.game.grounds[g.0].u.onett().awnings[i];
        a.initial = 0.0;
        a.jobj = aj;
        a.initial_y = y;
        a.accumulator = 0.0;
        a.velocity = 0.0;
        a.counter_prev = 0;
        a.counter = 0;
        a.flag = 0;
        a.cooldown = 0;
    }
    let p = param(r.game);
    let (lo, hi) = (
        p.i(r.data, p::SMOKE_WAIT_MIN),
        p.i(r.data, p::SMOKE_WAIT_MAX),
    );
    let t = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
    let v = r.game.grounds[g.0].u.onett();
    v.timer = t;
    v.smoke = None;
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let points = [n(r, 16), n(r, 17), n(r, 19), n(r, 20), n(r, 18), n(r, 21)];
    set_guide_points(r.game, r.data, points);
    setup::add_start_hook(r.game, gobj, CodeRef(STREET_START));
    update_map_coll(r.game, r.data, gobj);
}

/// The street's process (map 5), `grOnett_801E3C60`: the awnings, the chimney, the cars'
/// camera subject active while a car comes (`Ground_801C5794`), the wind sources and the
/// collision.
#[doc(alias = "grOnett_801E3C60")]
pub fn proc_street(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    swing_awnings(r, gobj);
    smoke(r, gobj);
    if let Some(b) = r.game.grounds[g.0].u.onett().subject {
        let state = if super::ground::x740(r.game) != 0 {
            camera::ACTIVE
        } else {
            camera::INACTIVE
        };
        r.game.camera.boxes[b].state = state;
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    update_map_coll(r.game, r.data, gobj);
}

/// The drugstore's setup (map 4), `grOnett_801E3CE4`: its collision, its animation stopped
/// at the start, standing (state -1 going to 0) with no landings, and its roof's callbacks.
#[doc(alias = "grOnett_801E3CE4")]
pub fn init_store(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 0.0, true);
    let v = r.game.grounds[g.0].u.onett_building();
    v.state = -1;
    v.next_state = 0;
    v.hit_count = 0;
    v.frame = 0;
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x10_flags.b5 = true;
    let user = Some(super::CollUser::Ground(g));
    for joint in [3, 4] {
        r.game
            .geometry_mut()
            .set_ledge_callback(joint, user, Some(CodeRef(ROOF_LANDED)));
    }
}

/// The drugstore's process (map 4), `grOnett_801E3DA0`. Standing (0), a landing cracks the
/// roof (1, then back to 0) or, after enough, breaks it (2, its sound at tick 140, then 3);
/// cracked, a landing shakes it (4, back to 3) or, after enough, it falls in (5: its sounds at
/// ticks 70 and 130, its platforms gone at ticks 20 and 90), and stays down a random while
/// (6), then stands again blinking for a second (7) with its platforms back.
#[doc(alias = "grOnett_801E3DA0")]
pub fn proc_store(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.onett_building();
    v.frame = v.frame.wrapping_add(1);
    match v.state {
        7 => {
            if v.timer & 2 != 0 {
                for part in [5, 2, 4] {
                    let pj = nth_joint(r.game, gobj, part);
                    jobj::set_flags_all(&mut r.game.objs.jobjs, pj, jobj::HIDDEN);
                }
            } else {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            }
            let v = r.game.grounds[g.0].u.onett_building();
            let t = v.timer as i32;
            v.timer = t.wrapping_sub(1) as u32;
            if t < 0 {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                r.game.grounds[g.0].u.onett_building().next_state = 0;
            }
        }
        1 => {
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.onett_building().next_state = 0;
            }
        }
        2 => {
            if v.frame == 0x8C {
                r.object_sound(Some(g), 0, 0x5F373);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.onett_building().next_state = 3;
            }
        }
        4 => {
            if anime::finished(r.game, gobj, 0, 7) {
                r.game.grounds[g.0].u.onett_building().next_state = 3;
            }
        }
        5 => {
            if v.frame == 0x46 || v.frame == 0x82 {
                r.object_sound(Some(g), 0, 0x5F374);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                let rand = reg(r.game.objs.rng.draw_f32());
                let p = param(r.game);
                let lo = reg(p.f(r.data, p::DOWN_MIN));
                let range = fp::fsubs(reg(p.f(r.data, p::DOWN_MAX)), lo);
                let v = r.game.grounds[g.0].u.onett_building();
                v.timer = fp::fctiwz(fp::fmadds(range, rand, lo)) as u32;
                v.next_state = 6;
            }
            let frame = r.game.grounds[g.0].u.onett_building().frame;
            if frame == 0x14 {
                r.game.geometry_mut().disable_joint(3);
            } else if frame == 0x5A {
                r.game.geometry_mut().disable_joint(4);
            }
        }
        6 => {
            let t = v.timer as i32;
            v.timer = t.wrapping_sub(1) as u32;
            if t < 0 {
                v.next_state = 7;
                v.timer = 0x3C;
            }
        }
        _ => {}
    }
    let v = r.game.grounds[g.0].u.onett_building();
    if v.state != v.next_state {
        v.state = v.next_state;
        let anim = match v.state {
            7 => Some(0),
            1 => Some(1),
            2 => Some(3),
            4 => Some(4),
            5 => Some(5),
            6 => {
                v.hit_count = 0;
                None
            }
            _ => None,
        };
        if let Some(a) = anim {
            anime::start(r.game, r.data, gobj, 0, 7, a, 0.0, 1.0, true);
        }
        if r.game.grounds[g.0].u.onett_building().state == 7 {
            r.game.geometry_mut().enable_joint(3);
            r.game.geometry_mut().enable_joint(4);
        }
        r.game.grounds[g.0].u.onett_building().frame = 0;
    }
    update_map_coll(r.game, r.data, gobj);
}

/// The drugstore roof's collision joints' callback, `grOnett_801E40E4`: a fighter landing
/// (collision kind 1) counts, and standing or cracked, the roof cracks, breaks, shakes or
/// falls by the count.
#[doc(alias = "grOnett_801E40E4")]
pub fn roof_landed(
    gp: &mut super::super::structs::Ground,
    param: Option<DataRef>,
    data: &Archives,
    kind: i32,
) {
    if kind != 1 {
        return;
    }
    let p = Param(param.expect("gronett.c: yakumono_param"));
    let v = gp.u.onett_building();
    v.hit_count = v.hit_count.wrapping_add(1);
    let count = fp::frsp(f64::from(v.hit_count));
    match v.state {
        0 => {
            v.next_state = if count < reg(p.f(data, p::CRACK_HITS)) {
                1
            } else {
                2
            };
        }
        3 => {
            v.next_state = if count < reg(p.f(data, p::FALL_HITS)) {
                4
            } else {
                5
            };
        }
        _ => {}
    }
}

/// The cars' start hook, `grOnett_801E41B0`: they stop waiting.
#[doc(alias = "grOnett_801E41B0")]
pub fn start_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.onett_car().set_waiting(false);
}

/// The cars' setup (map 3), `grOnett_801E41C8`: its animation, the four cars' joints and their
/// wheels', each car hidden off the street's right end with a hurtbox item on it, no car
/// driving either way, and its start hook.
#[doc(alias = "grOnett_801E41C8")]
pub fn init_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let cars = [n(r, 0xB), n(r, 1), n(r, 6), n(r, 0x10), n(r, 0x10)];
    let wheels = [n(r, 0xF), n(r, 5), n(r, 0xA), n(r, 0x14), n(r, 0x14)];
    let v = r.game.grounds[g.0].u.onett_car();
    v.car_jobjs = cars;
    v.car_jobjs2 = wheels;
    for (i, car) in cars.into_iter().take(4).enumerate() {
        let cj = car.expect("gronett.c: a car's joint");
        jobj::set_srt(&mut r.game.objs.jobjs, cj, |o| o.translate.x = RIGHT_END);
        jobj::set_flags_all(&mut r.game.objs.jobjs, car, jobj::HIDDEN);
        let item = material::stage_item(r, 0, 0, g, car, Some(CodeRef(CAR_HIT)), None, None);
        r.game.grounds[g.0].u.onett_car().car_items[i] = item;
        let item = item.expect("gronett.c: a car's item");
        material::item_hurtbox_on(r.game, item);
        super::super::it::lib2725::uncount_hits(r.game, item);
    }
    let first = r.game.objs.rng.below(4) as i8;
    let v = r.game.grounds[g.0].u.onett_car();
    v.curr_car = first;
    v.next_car = 4;
    v.curr_car = 4;
    v.state_b = 0;
    v.state_a = 0;
    r.game.gr.onett_x69c4 = 0;
    setup::add_start_hook(r.game, gobj, CodeRef(CARS_START));
    r.game.grounds[g.0].u.onett_car().set_waiting(true);
}

/// The fighters, first to last (`HSD_GObjPLinkHead[HSD_GOBJ_PLINK_FIGHTER]`).
fn fighters(game: &Game) -> Vec<GObjId> {
    let mut out = Vec::new();
    let mut at = game.world.first_in(P_LINK_FIGHTER);
    while let Some(f) = at {
        out.push(f);
        at = game.world.gobjs[f.0].next;
    }
    out
}

/// A car's joint's x.
fn car_x(game: &Game, j: Option<jobj::JObjId>) -> f32 {
    game.objs.jobjs[j.expect("gronett.c: a car's joint")]
        .translate
        .x
}

/// Moves a car's joint along by `speed`.
fn drive(game: &mut Game, j: Option<jobj::JObjId>, speed: f32) {
    let j = j.expect("gronett.c: a car's joint");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(speed)));
    });
}

/// Puts a car at an end of the street, `x` and `z`, its body turned by `turn`.
fn place_car(game: &mut Game, j: Option<jobj::JObjId>, x: f32, z: f32, turn: f32) {
    let cj = j.expect("gronett.c: a car's joint");
    jobj::set_srt(&mut game.objs.jobjs, cj, |o| o.translate.x = x);
    jobj::set_srt(&mut game.objs.jobjs, cj, |o| o.translate.z = z);
    let body = game.objs.jobjs[cj].child.expect("gronett.c: a car's body");
    jobj::set_angle(&mut game.objs.jobjs, body, |q| q.y = turn);
}

/// Puts a car's item in a state with its animation (`Item_80268E5C`).
fn car_state(r: &mut Running, item: Option<GObjId>, state: i32) {
    let item = item.expect("gronett.c: a car's item");
    super::super::it::item::change_state(
        r,
        item,
        state,
        super::super::it::item::change::ANIM_UPDATE,
    );
}

/// The fighters on the street (at most a unit above the ground).
fn on_street(r: &Running, f: GObjId) -> bool {
    reg(r.game.ft_pos(f).y) <= 1.0
}

/// The cars' process (map 3), `grOnett_801E43E0`, once the match has started. The car from
/// the right (`curr_car`): a new car other than the last and the left one's, at the right
/// end (state 0); a wait (1), then a speed and its item's driving state (2); waiting while
/// no fighter is on the street, and once one is, a wait by their standing (3, none for the
/// leader or a lone fighter); then it drives (5), its sign up and its horn at x 580, the warning
/// up and a fighter's scream once past the camera's right edge with someone on the street,
/// and gone past the left end. A car that hit someone spins off (4) or drives on (6). The
/// car from the left (`next_car`), behind: a car other than the last and the right one's,
/// a wait (1), a speed (2), and it drives (5), its horn at x -513, to the right end.
#[doc(alias = "grOnett_801E43E0")]
pub fn proc_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let saved = r.game.grounds[g.0].u.onett_car().curr_car;
    if r.game.grounds[g.0].u.onett_car().waiting() {
        return;
    }
    let (span, _) = camera::ground_span(r.game, r.data);
    let (center, right) = (span[1], span[2]);
    let p = param(r.game);
    let v = r.game.grounds[g.0].u.onett_car();
    let car_j = v.car_jobjs[saved as usize];
    let wheels_j = v.car_jobjs2[saved as usize];
    match v.state_a as i8 {
        0 => {
            let old = v.curr_car;
            while v.curr_car == old || v.curr_car == v.next_car {
                v.curr_car = r.game.objs.rng.below(4) as i8;
            }
            let j = v.car_jobjs[v.curr_car as usize];
            place_car(r.game, j, RIGHT_END, 30.0, 0.0);
            let wait = p.i(r.data, p::CAR_WAIT);
            let v = r.game.grounds[g.0].u.onett_car();
            v.state_a = 1;
            v.x108 = wait;
            super::ground::set_x740(r.game, 0);
        }
        1 => {
            if v.x108 != 0 {
                v.x108 = v.x108.wrapping_sub(1);
            } else {
                let car = v.curr_car;
                v.x108 = p.i(r.data, p::CAR_DRIVE_WAIT);
                let rand = reg(r.game.objs.rng.draw_f32());
                let speed = fp::fmadds(
                    reg(p.f(r.data, p::CAR_SPEED_ADD)),
                    rand,
                    reg(p.f(r.data, p::CAR_SPEED)),
                );
                let v = r.game.grounds[g.0].u.onett_car();
                v.car_speed = mem(speed);
                if reg(v.car_speed) > 0.0 {
                    v.car_speed = mem(fp::fmuls(reg(v.car_speed), fp::fneg(1.0)));
                }
                v.state_a = 2;
                let item = v.car_items[car as usize];
                car_state(r, item, i32::from(saved) + 2);
            }
        }
        2 => {
            if v.x108 != 0 {
                let all = fighters(r.game);
                let count = all.len() as i32;
                let v = r.game.grounds[g.0].u.onett_car();
                v.x108 = v.x108.wrapping_sub(1);
                for f in all {
                    if !on_street(r, f) {
                        continue;
                    }
                    let rank = r.fighter_rank(f);
                    if count == 1 || rank == 0 {
                        r.game.grounds[g.0].u.onett_car().x108 = 0;
                        jobj::clear_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                        r.game.grounds[g.0].u.onett_car().state_a = 5;
                    } else if count == rank.wrapping_add(1) {
                        let wait = p.i(r.data, p::CAR_WAIT_LAST);
                        let v = r.game.grounds[g.0].u.onett_car();
                        v.x108 = wait;
                        v.state_a = 3;
                    } else {
                        let wait = p.i(r.data, p::CAR_WAIT_OTHERS);
                        let v = r.game.grounds[g.0].u.onett_car();
                        v.x108 = wait;
                        v.state_a = 3;
                    }
                    r.game.grounds[g.0].u.onett_car().x110 = 0;
                }
            } else {
                v.x108 = 0;
                v.x110 = 0;
                jobj::clear_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                r.game.grounds[g.0].u.onett_car().state_a = 5;
                super::ground::set_x740(r.game, 1);
            }
        }
        3 => {
            if v.x108 != 0 {
                v.x108 = v.x108.wrapping_sub(1);
            } else {
                v.x108 = 0;
                v.x110 = 0;
                jobj::clear_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                r.game.grounds[g.0].u.onett_car().state_a = 5;
                super::ground::set_x740(r.game, 1);
            }
        }
        4 => {
            let spin = fp::fmuls(f64::from(0.017453292_f32), reg(p.f(r.data, p::CAR_SPIN)));
            let cj = car_j.expect("gronett.c: a car's joint");
            let body = r.game.objs.jobjs[cj]
                .child
                .expect("gronett.c: a car's body");
            jobj::set_srt(&mut r.game.objs.jobjs, body, |o| {
                o.rotate.y = mem(fp::fadds(reg(o.rotate.y), spin));
            });
            grlib::generator_on(r.game, r.data, 0x2C, 0, wheels_j);
            let speed = r.game.grounds[g.0].u.onett_car().car_speed;
            drive(r.game, car_j, speed);
            if reg(car_x(r.game, car_j)) <= reg(LEFT_END) {
                jobj::set_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                let v = r.game.grounds[g.0].u.onett_car();
                v.state_a = 0;
                let item = v.car_items[saved as usize];
                car_state(r, item, 0);
                return;
            }
        }
        5 | 6 => {
            let speed = v.car_speed;
            drive(r.game, car_j, speed);
            let x110 = r.game.grounds[g.0].u.onett_car().x110;
            let mut check_center = false;
            match x110 {
                0 => {
                    if reg(car_x(r.game, car_j)) <= reg(RIGHT_HORN) {
                        let car = r.game.grounds[g.0].u.onett_car().curr_car;
                        car_sound(r, g, i32::from(car), 5);
                        let t = p.i(r.data, p::SIGN_TIME);
                        r.game.show_hazard(r.data, t);
                        r.game.grounds[g.0].u.onett_car().x110 = 1;
                    }
                }
                1 => {
                    let edge = fp::fadds(reg(right), reg(p.f(r.data, p::HORN_X)));
                    if reg(car_x(r.game, car_j)) <= edge {
                        for f in fighters(r.game) {
                            if on_street(r, f) {
                                r.game.objs.rng.below(3);
                                scream(r, i32::from(saved));
                                r.game.grounds[g.0].u.onett_car().x110 = 2;
                                super::ground::set_x740(r.game, 2);
                                break;
                            }
                        }
                    }
                    check_center = true;
                }
                2 => check_center = true,
                _ => {}
            }
            if check_center && reg(car_x(r.game, car_j)) <= fp::fadds(100.0, reg(center)) {
                r.game.grounds[g.0].u.onett_car().x110 = 4;
            }
            if reg(car_x(r.game, car_j)) <= reg(LEFT_END) {
                jobj::set_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                let v = r.game.grounds[g.0].u.onett_car();
                v.state_a = 0;
                let item = v.car_items[saved as usize];
                car_state(r, item, 0);
                return;
            }
        }
        _ => {}
    }
    proc_back_car(r, g, p);
}

/// The car from the left's part of [`proc_cars`].
fn proc_back_car(r: &mut Running, g: GroundId, p: Param) {
    let v = r.game.grounds[g.0].u.onett_car();
    let next = v.next_car;
    let car_j = v.car_jobjs[next as usize];
    match v.state_b as i8 {
        0 => {
            while v.next_car == next || v.next_car == v.curr_car {
                v.next_car = r.game.objs.rng.below(4) as i8;
            }
            let j = v.car_jobjs[v.next_car as usize];
            place_car(r.game, j, LEFT_END, 56.0, std::f32::consts::PI);
            let wait = p.i(r.data, p::CAR_WAIT);
            let v = r.game.grounds[g.0].u.onett_car();
            v.state_b = 1;
            v.timer_b = wait;
        }
        1 => {
            if v.timer_b != 0 {
                v.timer_b = v.timer_b.wrapping_sub(1);
                return;
            }
            v.timer_b = p.i(r.data, p::CAR_DRIVE_WAIT);
            let rand = reg(r.game.objs.rng.draw_f32());
            let speed = fp::fmadds(
                reg(p.f(r.data, p::CAR_SPEED_ADD)),
                rand,
                reg(p.f(r.data, p::CAR_SPEED)),
            );
            let v = r.game.grounds[g.0].u.onett_car();
            v.speed_b = mem(speed);
            v.state_b = 2;
        }
        2 => {
            if v.timer_b != 0 {
                v.timer_b = v.timer_b.wrapping_sub(1);
                return;
            }
            v.timer_b = 0;
            jobj::clear_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
            let v = r.game.grounds[g.0].u.onett_car();
            v.sub_state_b = 0;
            v.state_b = 5;
            let item = v.car_items[next as usize];
            car_state(r, item, 0);
        }
        5 => {
            let speed = v.speed_b;
            drive(r.game, car_j, speed);
            if r.game.grounds[g.0].u.onett_car().sub_state_b == 0
                && reg(car_x(r.game, car_j)) >= reg(LEFT_HORN)
            {
                let car = r.game.grounds[g.0].u.onett_car().next_car;
                car_sound(r, g, i32::from(car), 4);
                r.game.grounds[g.0].u.onett_car().sub_state_b = 4;
            }
            if reg(car_x(r.game, car_j)) >= reg(RIGHT_END) {
                jobj::set_flags_all(&mut r.game.objs.jobjs, car_j, jobj::HIDDEN);
                r.game.grounds[g.0].u.onett_car().state_b = 0;
            }
        }
        _ => {}
    }
}

/// A car's hurtbox item's callback for a hit it deals, `grOnett_801E5030`. A spinning car
/// (state 4) crashes again; otherwise, at even odds, it spins off faster (at least 0.5) with
/// a crash, its horn stopped, or drives on (state 6).
#[doc(alias = "grOnett_801E5030")]
pub fn car_hit(r: &mut Running, _item: GObjId, g: GroundId) {
    if r.game.grounds[g.0].u.onett_car().state_a as i32 == 4 {
        let s = if r.game.objs.rng.below(2) != 0 {
            0x5F37C
        } else {
            0x5F37D
        };
        r.stage_sound(s);
    } else if r.game.objs.rng.below(2) != 0 {
        let rand = reg(r.game.objs.rng.draw_f32());
        let v = r.game.grounds[g.0].u.onett_car();
        v.car_speed = mem(fp::fadds(reg(v.car_speed), fp::fadds(0.5, rand)));
        let speed = reg(v.car_speed);
        let abs = if speed < 0.0 { fp::fneg(speed) } else { speed };
        if abs < 0.5 {
            v.car_speed = 0.5;
        }
        v.state_a = 4;
        v.x110 = 4;
        v.x108 = 1;
        let sfx = if r.game.objs.rng.below(2) != 0 {
            0x5F37C
        } else {
            0x5F37D
        };
        r.stop_object_sound(Some(g), 5);
        r.stage_sound(sfx);
    } else {
        r.game.grounds[g.0].u.onett_car().state_a = 6;
    }
}

/// A fighter's scream at the car, by car, `grOnett_801E5140`.
#[doc(alias = "grOnett_801E5140")]
pub fn scream(r: &mut Running, idx: i32) {
    let table = [0x5F370, 0x5F370, 0x5F371, 0x5F372];
    r.stage_sound(table[idx as usize]);
}

/// A car's horn in the cars' slot `kind` (4 from the left, 5 from the right), by car,
/// `grOnett_801E5194`.
#[doc(alias = "grOnett_801E5194")]
pub fn car_sound(r: &mut Running, g: GroundId, car: i32, kind: i32) {
    const SOUNDS: [[u32; 2]; 4] = [
        [0x5F377, 0x5F376],
        [0x5F377, 0x5F376],
        [0x5F379, 0x5F378],
        [0x5F37B, 0x5F37A],
    ];
    let id = SOUNDS[car as usize][(kind - 4) as usize];
    r.object_sound(Some(g), kind, id);
}

/// The awnings' springs, `grOnett_801E5214`: each sags toward this tick's push (no deeper
/// than its limit) and springs back, damped and capped, settling once close and slow; its
/// model follows. A landing on it this tick, or a count unlike last tick's, rustles it (an
/// effect and a sound, then ten ticks' quiet).
#[doc(alias = "grOnett_801E5214")]
pub fn swing_awnings(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    let max_disp = reg(p.f(d, p::MAX_DISPLACEMENT));
    let abs = |x: f64| if x < fp::frsp(0.0) { fp::fneg(x) } else { x };
    for i in 0..2 {
        let a = &mut r.game.grounds[g.0].u.onett().awnings[i];
        if reg(a.accumulator) < fp::fneg(max_disp) {
            a.accumulator = mem(fp::fneg(max_disp));
        }
        let disp = reg(a.accumulator);
        let error = fp::fsubs(disp, reg(a.velocity));
        let mut force = reg(a.initial);
        let spring = reg(p.f(d, p::SPRING_CONSTANT));
        force = fp::fadds(force, spring);
        let mut ratio = abs(error);
        ratio = fp::fdivs(ratio, max_disp);
        let spring_force = reg(p.f(d, p::SPRING_FORCE));
        if error < 0.0 {
            force = fp::fnmsubs(spring_force, ratio, force);
        } else {
            force = fp::fmadds(spring_force, ratio, force);
        }
        force = fp::fsubs(force, spring);
        force = fp::fmuls(force, fp::fsubs(1.0, reg(p.f(d, p::DAMPING))));
        let max_vel = reg(p.f(d, p::MAX_VELOCITY));
        if force > max_vel {
            force = max_vel;
        } else if force < fp::fneg(max_vel) {
            force = fp::fneg(max_vel);
        }
        let vel = reg(a.velocity);
        if abs(fp::fsubs(vel, disp)) < reg(p.f(d, p::POS_THRESHOLD))
            && abs(force) < reg(p.f(d, p::VEL_THRESHOLD))
        {
            a.velocity = mem(disp);
            a.initial = 0.0;
        } else {
            a.initial = mem(force);
            a.velocity = mem(fp::fadds(reg(a.velocity), force));
            let lim = max_disp;
            if reg(a.velocity) > lim {
                a.velocity = mem(lim);
            } else if reg(a.velocity) < fp::fneg(lim) {
                a.velocity = mem(fp::fneg(lim));
            }
        }
        let y = mem(fp::fadds(
            reg(a.velocity),
            fp::fadds(reg(a.initial_y), reg(a.accumulator)),
        ));
        let aj = a.jobj.expect("gronett.c: an awning's joint");
        jobj::set_srt(&mut r.game.objs.jobjs, aj, |o| o.translate.y = y);
        let a = &r.game.grounds[g.0].u.onett().awnings[i];
        if a.cooldown == 0 && (a.counter_prev != a.counter || a.flag != 0) {
            let aj = a.jobj;
            grlib::generator_on(r.game, r.data, 0x7533, 30, aj);
            r.object_sound(Some(g), 0, 0x5F375);
            r.game.grounds[g.0].u.onett().awnings[i].cooldown = 0xA;
        }
        let a = &mut r.game.grounds[g.0].u.onett().awnings[i];
        a.accumulator = 0.0;
        a.counter_prev = a.counter;
        a.counter = 0;
        a.flag = 0;
        if a.cooldown != 0 {
            a.cooldown = a.cooldown.wrapping_sub(1);
        }
    }
}

/// The awnings' collision joints' callback, `grOnett_801E54B4`: a fighter on it (its
/// collision's `x34` kind 1 to 3) pushes it down; one landing (collision kind 1) swings it.
#[doc(alias = "grOnett_801E54B4")]
pub fn awning_landed(
    gp: &mut super::super::structs::Ground,
    param: Option<DataRef>,
    data: &Archives,
    joint_id: i32,
    coll_kind: u32,
    kind: i32,
) {
    if !(1..=3).contains(&coll_kind) {
        return;
    }
    let p = Param(param.expect("gronett.c: yakumono_param"));
    let idx = if joint_id == 0 { 0 } else { 1 };
    let a = &mut gp.u.onett().awnings[idx];
    if kind == 1 {
        a.flag = 1;
        a.initial = p.f(data, p::AWNING_INITIAL);
    }
    a.accumulator = mem(fp::fadds(
        reg(a.accumulator),
        reg(p.f(data, p::AWNING_DELTA)),
    ));
    a.counter = a.counter.wrapping_add(1);
}

/// The chimney's smoke, `grOnett_801E5538`: when its wait runs out, a smoke stops (with a
/// wait before the next) or one starts from its joint (6) with a wait before it stops.
#[doc(alias = "grOnett_801E5538")]
pub fn smoke(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.onett();
    let t = v.timer;
    v.timer = t.wrapping_sub(1);
    if t >= 0 {
        return;
    }
    let p = param(r.game);
    if let Some(gen_) = r.game.grounds[g.0].u.onett().smoke {
        let (lo, hi) = (
            p.i(r.data, p::SMOKE_WAIT_MIN),
            p.i(r.data, p::SMOKE_WAIT_MAX),
        );
        let t = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
        r.game.grounds[g.0].u.onett().timer = t;
        grlib::kill_generator(r.game, r.data, gen_);
        r.game.grounds[g.0].u.onett().smoke = None;
        return;
    }
    let (lo, hi) = (p.i(r.data, p::SMOKE_MIN), p.i(r.data, p::SMOKE_MAX));
    let t = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
    r.game.grounds[g.0].u.onett().timer = t;
    let j = nth_joint(r.game, gobj, 6);
    if j.is_some() {
        let rot = Vec3::new(15.75, 40.0, 28.25);
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(rot));
        let gen_ = grlib::generator_at(r.game, r.data, 0x7530, 30, pos);
        r.game.grounds[g.0].u.onett().smoke = gen_;
    }
}
