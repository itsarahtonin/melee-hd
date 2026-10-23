//! Mute City, `melee/gr/grmutecity.c`: the course (map 30), which carries the fighters'
//! platform (map 29) along the race track, and the thirty F-Zero cars (map 2) racing round it.
//!
//! The course's animation runs the stage: a script read by its frame (`COURSE_SCRIPT`)
//! hides and shows the platform's parts, takes collision joints out of the search and puts
//! them back, moves the camera limits and blast zones, raises the hazard sign before the
//! platform sets down on the track, and while the platform stands on the track (joint 4),
//! makes the track's lines follow the cars' splines between the two places the track's ends
//! hold on them, and makes them hurt (`line_hazard`). The cars race on their own: each keeps a
//! place on the track, a speed and a lane, overtakes, brakes and swerves round the cars near it
//! (`steer_cars`), takes damage from the fighters through a hurtbox item while it passes the
//! stage, smokes, and is wrecked past enough damage.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
// Fields keep the original's offset names.
#![allow(non_snake_case)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Obj, Running};
use super::data::{MUTECITY_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::lobj;
use crate::vec::Vec3;

/// `fn_801F2B58`, the track's collision joint's callback (`mpJointSetCb1`).
pub const TRACK_TOUCHED: u32 = 0x801F_2B58;
/// `grMuteCity_801F1A0C`, a car's hurtbox item's callback for a hit it deals.
pub const CAR_DEALT: u32 = 0x801F_1A0C;
/// `grMuteCity_801F173C`, the first car's callback for a hit it takes; car `i`'s is
/// 0x18 bytes on (`grMc_803E3C6C`).
pub const CAR_HIT: u32 = 0x801F_173C;

/// The track's collision joint and its lines: the left end, the middle and the right end.
const TRACK_JOINT: i32 = 4;
const LINE_LEFT: i32 = 0x31;
const LINE_MID: i32 = 0x33;
const LINE_RIGHT: i32 = 0x35;

/// How many cars race.
pub const CARS: usize = 30;

/// The fields of `grMc_YakumonoParam`, by offset.
mod p {
    /// The color scripts of the course's flash and of the platform's lights.
    pub const COURSE_SCRIPT: u32 = 0;
    pub const LIGHT_SCRIPT: u32 = 4;
    /// The track's lines' hit: its ends' and its middle's.
    pub const END_HIT: u32 = 8;
    pub const MID_HIT: u32 = 0xC;
    /// The damage at which a car smokes and at which it's wrecked.
    pub const SMOKE_AGE: u32 = 0x2C;
    pub const WRECK_AGE: u32 = 0x30;
    /// A car's top speed, in the lead pack's and in the back's.
    pub const TOP_SPEED: u32 = 0x34;
    pub const TOP_SPEED_LEAD: u32 = 0x38;
    pub const TOP_SPEED_BACK: u32 = 0x3C;
    /// Its acceleration, its swerve, its drift back to the middle lanes, its braking.
    pub const ACCEL: u32 = 0x40;
    pub const SWERVE: u32 = 0x44;
    pub const CENTER: u32 = 0x48;
    pub const BRAKE: u32 = 0x4C;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn ptr(self, data: &Archives, off: u32) -> Option<DataRef> {
        data.ptr(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .mutecity_yakumono
            .expect("grmutecity.c: yakumono_param"),
    )
}

/// One car (`grMc_CarEntry`, 0x2C bytes).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Car {
    /// How far it has gone (`x0`), never wrapped, which orders the race.
    pub x0: f32,
    /// Its place on the track's splines (`x4`), 0 to 1.
    pub x4: f32,
    /// Its speed (`x8`) and its lane (`xC`, 0 the right spline, 1 the left).
    pub x8: f32,
    pub xC: f32,
    /// The damage it took (`x10`).
    pub x10: f32,
    /// Where it is (`pos`).
    pub pos: Vec3,
    /// What the race around it asks this tick (`x20`): 1 a car close behind, 2 slower and 4
    /// faster than a close one, 8 one right beside it, 0x10 among the first five, 0x20 among
    /// the last five.
    pub x20: u16,
    /// `x22_flags`: 0x80 wrecked, 0x40 its pass shook the screen.
    pub x22: u8,
    /// Its hurtbox item while it passes the stage (`x24`).
    pub item: Option<GObjId>,
    /// `gen`: what `grMuteCity_801F2AB0` returned, which is never the generator. Its last
    /// call, `Ground_801C0498`, leaves the stage parameters' address in r3 (`stage_info.x6B0`),
    /// so a car with a generator holds that address, and one without (none made, or no
    /// parameters) holds 0: true here stands for the address, which the comparer checks.
    pub generator: bool,
}

impl Car {
    fn wrecked(&self) -> bool {
        self.x22 & 0x80 != 0
    }
    fn shook(&self) -> bool {
        self.x22 & 0x40 != 0
    }
}

/// grmutecity.c's statics.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The cars (`grMc_8049F4B8`).
    pub cars: [Car; CARS],
    /// The cars in race order (`grMc_8049F440`), first first.
    pub order: [i32; CARS],
    /// The word before the order (`grMc_8049F440[-1]`, the last word of Brinstar's bubbles,
    /// `grZe_8049F170[19].x20`): the sort's insertion compares the order's first car with the
    /// car this word names and swaps them when the first has gone further, so the word joins
    /// the order (Melee's bug, docs/melee-bugs.md). Zero from boot unless Brinstar ran first.
    pub order_before: i32,
    /// Whether the track hurts (`grMc_804D69D4`): 1 while the platform stands on it.
    pub track_live: i32,
}

impl Default for Globals {
    fn default() -> Self {
        Globals {
            cars: std::array::from_fn(|_| Car::default()),
            order: [0; CARS],
            order_before: 0,
            track_live: 0,
        }
    }
}

/// A stage object's process or callback, by address: Mute City's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_01B0 | 0x801F_040C | 0x801F_0444 | 0x801F_0448 | 0x801F_04B0 | 0x801F_04B4
        | 0x801F_0948 => empty(r, gobj),
        0x801E_FDF8 => init_course(r, gobj),
        0x801F_0120 => proc_course(r, gobj),
        0x801F_01B4 => init_platform(r, gobj),
        0x801F_0290 => proc_platform(r, gobj),
        0x801F_0410 => init_scenery(r, gobj),
        0x801F_044C => init_cars(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801E_FC6C => on_init(r),
        0x801E_FCDC => on_load(r),
        0x801E_FCE0 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grMuteCity_801F01B0")]
#[doc(alias = "grMuteCity_801F040C")]
#[doc(alias = "grMuteCity_801F0444")]
#[doc(alias = "grMuteCity_801F0448")]
#[doc(alias = "grMuteCity_801F04B0")]
#[doc(alias = "grMuteCity_801F04B4")]
#[doc(alias = "grMuteCity_801F0948")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grMuteCity_801EFD0C`.
#[doc(alias = "grMuteCity_801EFD0C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grMc_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &MUTECITY_CALLBACKS[map_id as usize]
}

/// Where the course's setup makes the platform and the cars: a frame below the stage's
/// setup's ([`setup::SETUP_FRAME`]), less `grMuteCity_801EFD0C`'s and the course's setup's.
pub const COURSE_FRAME: u32 = setup::SETUP_FRAME - 0x30 - 0x48;

/// Mute City's setup, `grMuteCity_801EFC6C`: its parameters, maps 0 and 30 (which makes 29 and
/// 2), the camera limits and blast zones measured between them, and the hazard sign's model,
/// hidden.
#[doc(alias = "grMuteCity_801EFC6C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.mutecity_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = false;
    create(r, 0, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    create(r, 0x1E, setup::SETUP_FRAME);
    let desc = sign_model(r);
    r.create_fzero_hazard(desc.expect("un_802FD708: a model"));
    r.game.hide_hazard();
}

/// `grMuteCity_801EFC68` does nothing (only a demo runs it).
#[doc(alias = "grMuteCity_801EFC68")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grMuteCity_801EFCDC` does nothing.
#[doc(alias = "grMuteCity_801EFCDC")]
pub fn on_load(_r: &mut Running) {}

/// Mute City's start, `grMuteCity_801EFCE0`: the enemy spawner, without enemies.
#[doc(alias = "grMuteCity_801EFCE0")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The hazard sign's model, `grMuteCity_801F28A8`: map 38's entry in its archive.
#[doc(alias = "grMuteCity_801F28A8")]
pub fn sign_model(r: &Running) -> Option<DataRef> {
    let head = super::datfiles::archive_for(r.game, r.data, 38);
    let head = head.expect("grmutecity.c: 2135: archive");
    r.data.ptr(head.0.at(8))?;
    Some(head.entry(r.data, 38).0)
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// A joint's spline (`jobj->u.spline`).
fn spline_of(game: &Game, j: Option<JObjId>) -> Option<DataRef> {
    match game.objs.jobjs[j?].u {
        jobj::Contents::Spline(s) => s,
        _ => None,
    }
}

/// A light GObj's first light (`GET_LOBJ`).
fn first_light(game: &Game, g: GObjId) -> Option<lobj::LObjId> {
    match game.world.gobj(g).obj {
        Some(Obj::LObj(l)) => Some(l),
        _ => None,
    }
}

/// The course's setup (map 30), `grMuteCity_801EFDF8`: its animation looping over the
/// course's 3,600 frames, the script at its start, the track's ends at the start of the
/// cars' two splines (joints 5 and 9), the five joints whose shadows the track cuts, the
/// camera limits' and blast zones' offsets from the camera's center, the platform (map 29)
/// and the cars (map 2) with the cars placed, the platform's and the track's joints in the
/// stage's material class, the track's collision joint always near with its callback, and
/// the stage's point light hidden.
#[doc(alias = "grMuteCity_801EFDF8")]
pub fn init_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::loop_between(r.game, gobj, 0, 7, 0.0, 3600.0);
    let left = nth_joint(r.game, gobj, 5);
    let right = nth_joint(r.game, gobj, 9);
    {
        let v = r.game.grounds[g.0].u.mute_city();
        v.xC4 = 0;
        v.xC6 = 0;
        v.xD2 = 0;
        v.xD8 = 0.932775;
        v.xD4 = 0.932775;
        v.set_track_on(false);
        v.xDC = left;
        v.xE0 = right;
    }
    assert!(
        spline_of(r.game, right).is_some(),
        "not found car spline (R)"
    );
    assert!(
        spline_of(r.game, left).is_some(),
        "not found car spline (L)"
    );
    let shadows = [0xC, 0xD, 0xE, 0xF, 0x10].map(|k| nth_joint(r.game, gobj, k));
    let c = super::stage::cam_center(r.game);
    let bounds = [
        mem(fp::fsubs(reg(super::stage::cam_top(r.game)), reg(c.y))),
        mem(fp::fsubs(reg(super::stage::cam_bottom(r.game)), reg(c.y))),
        mem(fp::fsubs(reg(super::stage::cam_left(r.game)), reg(c.x))),
        mem(fp::fsubs(reg(super::stage::cam_right(r.game)), reg(c.x))),
        mem(fp::fsubs(reg(super::stage::blast_top(r.game)), reg(c.y))),
        mem(fp::fsubs(reg(super::stage::blast_bottom(r.game)), reg(c.y))),
        mem(fp::fsubs(reg(super::stage::blast_left(r.game)), reg(c.x))),
        mem(fp::fsubs(reg(super::stage::blast_right(r.game)), reg(c.x))),
    ];
    {
        let v = r.game.grounds[g.0].u.mute_city();
        v.set_ends(2);
        v.xE4 = Vec3::new(0.0, 0.0, 0.0);
        v.xF0 = Vec3::new(0.0, 0.0, 0.0);
        v.shadow_jobjs = shadows;
        v.bounds = bounds;
    }
    r.game.grounds[g.0].x10_flags.b5 = true;
    let platform = create(r, 0x1D, COURSE_FRAME);
    r.game.grounds[g.0].u.mute_city().xC8 = platform;
    let cars = create(r, 2, COURSE_FRAME);
    r.game.grounds[g.0].u.mute_city().xCC = cars;
    move_cars(r, cars.expect("grmutecity.c: the cars"), gobj);
    for k in [0xC, 0xC, 0xD, 0xE, 0xF, 0x10] {
        let j = nth_joint(r.game, gobj, k);
        material::model_to_stage_class(r.game, j);
    }
    let geo = r.game.geometry_mut();
    geo.set_always_near(TRACK_JOINT);
    let user = Some(super::CollUser::Ground(g));
    geo.set_ledge_callback(TRACK_JOINT, user, Some(CodeRef(TRACK_TOUCHED)));
    let lights = super::ground::light_gobj(r.game);
    r.game.grounds[g.0].u.mute_city().x110 = None;
    if let Some(l) = lights.and_then(|lg| first_light(r.game, lg)) {
        let mut cur = Some(l);
        while let Some(l) = cur {
            if r.game.objs.lobjs[l].flags & lobj::TYPE_MASK == lobj::POINT {
                r.game.grounds[g.0].u.mute_city().x110 = Some(l);
                lobj::set_flags(&mut r.game.objs.lobjs, Some(l), lobj::HIDDEN.into());
            }
            cur = r.game.objs.lobjs[l].next;
        }
    }
}

/// The point light's distance falloff (`light_ref_br`, `light_ref_dist`, `grMc_804D46CC`;
/// the names have the falloff's two values the other way round).
const LIGHT_REF_DIST: f32 = 40000.0;
const LIGHT_REF_BR: f32 = 0.99;
const LIGHT_DIST_FUNC: i32 = 1;

/// The course's process (map 30), `grMuteCity_801F0120`: the point light's falloff, the
/// script, the track's lines while they follow the cars, the race, the cars' moves, the
/// collision and the wind sources.
#[doc(alias = "grMuteCity_801F0120")]
pub fn proc_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let light = r.game.grounds[g.0].u.mute_city().x110;
    if light.is_some() {
        lobj::set_dist_attn(
            &mut r.game.objs.lobjs,
            light,
            LIGHT_REF_DIST,
            LIGHT_REF_BR,
            LIGHT_DIST_FUNC,
        );
    }
    run_script(r, gobj);
    empty(r, gobj);
    if r.game.grounds[g.0].u.mute_city().track_on() {
        follow_track(r, gobj);
    }
    steer_cars(r);
    let cars = r.game.grounds[g.0].u.mute_city().xCC;
    move_cars(r, cars.expect("grmutecity.c: the cars"), gobj);
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The platform's setup (map 29), `grMuteCity_801F01B4`: its collision, its materials in the
/// stage's class, its animation looping over the course's frames with part 6's second
/// animation, and its swaying joint (6) at no sway.
#[doc(alias = "grMuteCity_801F01B4")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    material::model_to_stage_class(r.game, j);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::loop_between(r.game, gobj, 0, 7, 0.0, 3600.0);
    anime::start(r.game, r.data, gobj, 6, 7, 1, 0.0, 1.0, true);
    r.game.grounds[g.0].x10_flags.b5 = true;
    let sway = nth_joint(r.game, gobj, 6);
    let v = r.game.grounds[g.0].u.mute_city2();
    v.xC8 = sway;
    v.xCC = 0.0;
    v.xD0 = 0.0;
    v.xC4 &= !0x80;
}

/// The platform's process (map 29), `grMuteCity_801F0290`: the sway's scale steps toward its
/// target by at most 0.001 a tick and scales the swaying joint's turn; then the lights and
/// the collision.
#[doc(alias = "grMuteCity_801F0290")]
pub fn proc_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.mute_city2();
    let target = reg(v.xCC);
    let current = reg(v.xD0);
    let diff = fp::fsubs(target, current);
    if diff > 0.0 {
        if diff < reg(0.001) {
            v.xD0 = mem(target);
        } else {
            v.xD0 = mem(fp::fadds(reg(v.xD0), reg(0.001)));
        }
    } else if diff < 0.0 {
        if diff > reg(-0.001) {
            v.xD0 = mem(target);
        } else {
            v.xD0 = mem(fp::fsubs(current, reg(0.001)));
        }
    } else {
        v.xD0 = mem(target);
    }
    let (sway, k) = (v.xC8.expect("grmutecity.c: the swaying joint"), v.xD0);
    let z = r.game.objs.jobjs[sway].rotate.z;
    jobj::set_rotation_z(&mut r.game.objs.jobjs, sway, mem(fp::fmuls(reg(z), reg(k))));
    platform_lights(r, gobj);
    update_map_coll(r.game, r.data, gobj);
}

/// Maps 36 and 37 start their map animation, `stageGObj36_37_OnInit` (the inline
/// `Ground_StartMapAnim`).
#[doc(alias = "stageGObj36_37_OnInit")]
pub fn init_scenery(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// `grMc_803E34A4`: the cars' models' map ids (the first, the cars' object's own, unused).
const CAR_MAPS: [i16; 30] = [
    2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 1, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    32, 33, 34, 35,
];

/// The cars' pilots' map.
const PILOT_MAP: i32 = 0x1C;

/// The cars' setup (map 2), `grMuteCity_801F044C`: the thirty cars with their pilots, their
/// places on the grid, and no horn waiting.
#[doc(alias = "grMuteCity_801F044C")]
pub fn init_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    super::fzerocar::build(r, gobj, &CAR_MAPS, PILOT_MAP, false);
    place_cars(r.game);
    r.game.grounds[g.0].u.mute_city_cars().xC4 = 0;
}

/// A command of the course's script (`grMc_CmdEntry`): from the frame on (-1 at once after
/// the last), the command, its number and its value.
#[derive(Clone, Copy)]
struct Cmd {
    frame: i32,
    cmd: i16,
    param: i16,
    fval: f32,
}

const fn c(frame: i32, cmd: i16, param: i16, fval: f32) -> Cmd {
    Cmd {
        frame,
        cmd,
        param,
        fval,
    }
}

/// The course's script, `grMc_803E34E0`.
const COURSE_SCRIPT: [Cmd; 132] = [
    c(-1, 11, 2, 0.0),
    c(-1, 11, 3, 0.0),
    c(-1, 11, 6, 0.0),
    c(-1, 11, 4, 0.0),
    c(-1, 11, 7, 0.0),
    c(-1, 4, 0, 0.0),
    c(-1, 4, 1, 0.0),
    c(-1, 4, 6, 0.0),
    c(-1, 4, 2, 0.0),
    c(-1, 4, 5, 0.0),
    c(-1, 4, 7, 0.0),
    c(-1, 4, 8, 0.0),
    c(-1, 10, 0, 0.0),
    c(-1, 9, 0, 0.932775),
    c(-1, 18, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(-1, 17, 0, 0.0),
    c(-1, 13, 0, 90.0),
    c(180, 17, 0, 0.0),
    c(384, 19, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 10, 0, 0.0),
    c(475, 2, 0, 0.0),
    c(475, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(530, 21, 0, 0.0),
    c(567, 18, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(708, 26, 1, 0.0),
    c(800, 19, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 10, 1, 0.0),
    c(-1, 12, 2, 0.0),
    c(-1, 5, 0, 0.0),
    c(-1, 5, 7, 0.0),
    c(-1, 5, 8, 0.0),
    c(-1, 15, 0, -270.0),
    c(-1, 16, 0, 270.0),
    c(890, 2, 0, 0.0),
    c(890, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(950, 21, 0, 0.0),
    c(980, 18, 0, 0.0),
    c(-1, 4, 0, 0.0),
    c(-1, 4, 7, 0.0),
    c(-1, 4, 8, 0.0),
    c(-1, 11, 2, 0.0),
    c(-1, 7, 0, 0.0),
    c(-1, 17, 0, 0.0),
    c(1000, 13, 0, 40.0),
    c(-1, 14, 0, 0.0),
    c(-1, 15, 0, -50.0),
    c(-1, 16, 0, 50.0),
    c(1080, 23, 1, 0.0),
    c(-1, 24, 1, 0.0),
    c(708, 26, 0, 0.0),
    c(1130, 19, 0, 0.0),
    c(-1, 23, 0, 0.0),
    c(-1, 24, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 17, 0, 0.0),
    c(-1, 14, 0, -30.0),
    c(-1, 10, 0, 0.0),
    c(-1, 12, 7, 0.0),
    c(-1, 5, 5, 0.0),
    c(1223, 2, 0, 0.0),
    c(1223, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(1280, 21, 0, 0.0),
    c(1312, 18, 0, 0.0),
    c(-1, 26, 2, 0.0),
    c(-1, 11, 7, 0.0),
    c(-1, 4, 5, 0.0),
    c(-1, 17, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(1404, 19, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 10, 2, 0.0),
    c(-1, 12, 3, 0.0),
    c(-1, 5, 1, 0.0),
    c(1495, 2, 0, 0.0),
    c(-1, 25, 1, 0.0),
    c(1495, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(1550, 21, 0, 0.0),
    c(1585, 18, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(-1, 25, 0, 0.0),
    c(-1, 11, 3, 0.0),
    c(-1, 4, 1, 0.0),
    c(2045, 19, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 10, 0, 0.0),
    c(-1, 12, 6, 0.0),
    c(-1, 5, 6, 0.0),
    c(-1, 22, 0, 1.0),
    c(2135, 2, 0, 0.0),
    c(2135, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(-1, 22, 0, 0.0),
    c(2190, 21, 0, 0.0),
    c(2225, 18, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(-1, 11, 6, 0.0),
    c(-1, 4, 6, 0.0),
    c(2273, 8, 0, 0.0),
    c(3290, 9, 0, 0.86445),
    c(3300, 13, 0, 90.0),
    c(3390, 19, 0, 0.0),
    c(-1, 6, 0, 0.0),
    c(-1, 10, 0, 0.0),
    c(-1, 12, 4, 0.0),
    c(-1, 5, 2, 0.0),
    c(3510, 2, 0, 0.0),
    c(3510, 1, 340, 0.0),
    c(-1, 20, 120, 0.0),
    c(-1, 1, 60, 0.0),
    c(-1, 3, 0, 0.0),
    c(3570, 21, 0, 0.0),
    c(3599, 18, 0, 0.0),
    c(-1, 7, 0, 0.0),
    c(9999, 0, 0, 0.0),
];

/// How many of the script's commands run before it wraps.
const SCRIPT_LEN: i16 = 0x84;

/// A camera limit's or blast zone's offset, `x114` to `x130`.
fn bound(game: &mut Game, g: GroundId, k: usize) -> f32 {
    game.grounds[g.0].u.mute_city().bounds[k]
}

/// The course's script, `grMuteCity_801F04B8`: unless waiting, each command whose frame the
/// course's animation has reached runs (the script starts over, and the platform's
/// animation with it, when part 26's animation comes round). A wait (1) ends the tick's
/// commands.
#[doc(alias = "grMuteCity_801F04B8")]
pub fn run_script(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let frame = reg(super::ground::anim_frame(r.game, j));
    let v = r.game.grounds[g.0].u.mute_city();
    if v.xC6 > 0 {
        v.xC6 -= 1;
        return;
    }
    let part = nth_joint(r.game, gobj, 0x1A);
    if crate::lb::anim::any_rewound(&r.game.objs, part) {
        r.game.grounds[g.0].u.mute_city().xC4 = 0;
        let platform = r.game.grounds[g.0].u.mute_city().xC8;
        let pj = platform.and_then(|p| jobj_of(r.game, p));
        jobj::req_anim_all(&mut r.game.objs, pj, 1.0);
    }
    loop {
        let at = r.game.grounds[g.0].u.mute_city().xC4;
        let e = COURSE_SCRIPT[at as usize];
        if !(frame >= fp::frsp(f64::from(e.frame))) {
            break;
        }
        let platform = r.game.grounds[g.0].u.mute_city().xC8;
        run_command(r, gobj, g, platform, e);
        let v = r.game.grounds[g.0].u.mute_city();
        v.xC4 += 1;
        if v.xC4 == SCRIPT_LEN {
            v.xC4 = 0;
        }
        if e.cmd == 1 {
            break;
        }
    }
}

/// One command of the course's script (the switch of `grMuteCity_801F04B8`).
fn run_command(r: &mut Running, gobj: GObjId, g: GroundId, platform: Option<GObjId>, e: Cmd) {
    let fval = reg(e.fval);
    let scaled = |r: &Running| mem(fp::fmuls(fval, reg(super::ground::scale(r.game))));
    let platform_part = |r: &Running, k: i16| {
        let p = platform.expect("grmutecity.c: the platform");
        nth_joint(r.game, p, k.into())
    };
    match e.cmd {
        1 => r.game.grounds[g.0].u.mute_city().xC6 = e.param,
        2 => {
            let p = platform.expect("grmutecity.c: the platform");
            anime::set_rate(r.game, gobj, 0, 1, 0.0, true);
            anime::set_rate(r.game, p, 0, 7, 0.0, true);
            anime::set_rate(r.game, gobj, 0x11, 7, 1.0, true);
            anime::set_rate(r.game, p, 6, 7, 1.0, true);
        }
        3 => {
            let p = platform.expect("grmutecity.c: the platform");
            anime::set_rate(r.game, gobj, 0, 1, 1.0, true);
            anime::set_rate(r.game, p, 0, 7, 1.0, true);
        }
        4 => r.game.geometry_mut().disable_joint(e.param.into()),
        5 => r.game.geometry_mut().enable_joint(e.param.into()),
        6 => r.game.gr.mutecity.track_live = 0,
        7 => r.game.gr.mutecity.track_live = 1,
        8 => {
            r.game.geometry_mut().disable_joint(TRACK_JOINT);
            r.game.grounds[g.0].u.mute_city().set_track_on(false);
        }
        18 => r.game.grounds[g.0].u.mute_city().set_loud(true),
        19 => r.game.grounds[g.0].u.mute_city().set_loud(false),
        9 => {
            r.game.geometry_mut().enable_joint(TRACK_JOINT);
            let v = r.game.grounds[g.0].u.mute_city();
            v.set_track_on(true);
            v.xD4 = e.fval;
            v.xD8 = e.fval;
        }
        11 => {
            let j = platform_part(r, e.param);
            if j.is_some() {
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            }
        }
        12 => {
            let j = platform_part(r, e.param);
            if j.is_some() {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            }
        }
        13 => {
            let s = scaled(r);
            super::ground::set_cam_top(r.game, s);
            if fval > reg(bound(r.game, g, 0)) {
                let d = fp::fsubs(reg(bound(r.game, g, 4)), reg(bound(r.game, g, 0)));
                super::ground::set_blast_top(r.game, mem(fp::fadds(fval, d)));
            }
        }
        14 => {
            let s = scaled(r);
            super::ground::set_cam_bottom(r.game, s);
            if fval < reg(bound(r.game, g, 1)) {
                let d = fp::fsubs(reg(bound(r.game, g, 5)), reg(bound(r.game, g, 1)));
                super::ground::set_blast_bottom(r.game, mem(fp::fadds(fval, d)));
            }
        }
        15 => {
            let s = scaled(r);
            super::ground::set_cam_left(r.game, s);
            if fval < reg(bound(r.game, g, 2)) {
                let d = fp::fsubs(reg(bound(r.game, g, 6)), reg(bound(r.game, g, 2)));
                super::ground::set_blast_left(r.game, mem(fp::fadds(fval, d)));
            }
        }
        16 => {
            let s = scaled(r);
            super::ground::set_cam_right(r.game, s);
            if fval > reg(bound(r.game, g, 3)) {
                let d = fp::fsubs(reg(bound(r.game, g, 7)), reg(bound(r.game, g, 3)));
                super::ground::set_blast_right(r.game, mem(fp::fadds(fval, d)));
            }
        }
        17 => {
            let b = r.game.grounds[g.0].u.mute_city().bounds;
            super::ground::set_cam_top(r.game, b[0]);
            super::ground::set_cam_bottom(r.game, b[1]);
            super::ground::set_cam_left(r.game, b[2]);
            super::ground::set_cam_right(r.game, b[3]);
            super::ground::set_blast_top(r.game, b[4]);
            super::ground::set_blast_bottom(r.game, b[5]);
            super::ground::set_blast_left(r.game, b[6]);
            super::ground::set_blast_right(r.game, b[7]);
        }
        21 => {
            let script = param(r.game).ptr(r.data, p::COURSE_SCRIPT);
            material::start_script(r.game, r.data, gobj, script, 0);
        }
        20 => {
            r.game.show_hazard(r.data, e.param.into());
            r.stage_sound(0x5CC6A);
        }
        22 => {
            let p = platform.expect("grmutecity.c: the platform");
            let pg = ground_of(r.game, p);
            r.game.grounds[pg.0].u.mute_city2().xCC = e.fval;
        }
        23 => r.set_ground_send(if e.param != 0 { 2 } else { 1 }),
        25 => {
            let t = if e.param != 0 { 0xC } else { 0 };
            r.game.geometry_mut().set_line_terrain(0x32, t);
            r.game.geometry_mut().set_line_terrain(0x34, t);
        }
        24 => {
            if let Some(bg) = super::ground::map_gobj(r.game, 0x1D) {
                let light = r.game.grounds[g.0].u.mute_city().x110;
                if e.param != 0 {
                    let script = param(r.game).ptr(r.data, p::LIGHT_SCRIPT);
                    material::start_script(r.game, r.data, bg, script, 0);
                    if light.is_some() {
                        lobj::clear_flags(&mut r.game.objs.lobjs, light, lobj::HIDDEN.into());
                    }
                } else {
                    material::reset_overlay(r.game, bg);
                    if light.is_some() {
                        lobj::set_flags(&mut r.game.objs.lobjs, light, lobj::HIDDEN.into());
                    }
                }
            }
        }
        26 => r.game.grounds[g.0].u.mute_city().set_ends(e.param as u8),
        _ => {}
    }
}

/// The point of a car spline at `t`, its ends blended over the seam (the inline
/// `grMc_SplineEvalWrapped`): past 0.93 or before 0.01, the straight line from the point at
/// 0.93 to the point at 0.01. `None` where the spline leaves the output alone.
fn spline_point(data: &Archives, spline: DataRef, t: f32, tangent: bool) -> Option<Vec3> {
    let tr = reg(t);
    let eval = |u: f32| {
        if tangent {
            crate::hsd::spline::tangent(data, spline, u)
        } else {
            crate::hsd::spline::point(data, spline, u)
        }
    };
    let frac = if tr > reg(0.93) {
        fp::fsubs(tr, reg(0.93))
    } else if tr < reg(0.01) {
        fp::fadds(reg(0.069_999_99), tr)
    } else {
        return eval(t);
    };
    let a = eval(0.93).expect("splGetSplinePoint at 0.93");
    let b = eval(0.01).expect("splGetSplinePoint at 0.01");
    let b = b - a;
    let s = fp::fdivs(frac, reg(0.079_999_99));
    let b = Vec3::new(
        mem(fp::fmuls(reg(b.x), s)),
        mem(fp::fmuls(reg(b.y), s)),
        mem(fp::fmuls(reg(b.z), s)),
    );
    Some(b + a)
}

/// A car spline's point (or direction, `tangent`) at `t`, which always lies in [0, 1] here.
fn spline_at(data: &Archives, spline: DataRef, t: f32, tangent: bool) -> Vec3 {
    spline_point(data, spline, t, tangent).expect("grmutecity.c: a spline parameter in [0, 1]")
}

/// A car spline's point at `t` in the world through its joint.
fn world_spline_point(r: &mut Running, j: Option<JObjId>, t: f32) -> Vec3 {
    let spline = spline_of(r.game, j).expect("grmutecity.c: a car spline");
    let local = spline_at(r.data, spline, t, false);
    crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(local))
}

/// Where a track end sits, `grMuteCity_801F094C`: from `t`, steps of 0.0004 along a car
/// spline (wrapping round) toward where the spline crosses the camera's plane (z 0 in the
/// world) until the point at `t` and the last one lie either side of it; returns the new
/// `t` and that point.
#[doc(alias = "grMuteCity_801F094C")]
pub fn find_track_end(r: &mut Running, j: Option<JObjId>, t: f32) -> (f32, Vec3) {
    let mut t = reg(t);
    let mut cur = world_spline_point(r, j, mem(t));
    let mut back = fp::fsubs(t, reg(0.0004));
    if back < 0.0 {
        back = fp::fadds(back, 1.0);
    }
    let mut prev = world_spline_point(r, j, mem(back));
    let toward = if reg(cur.z) > reg(prev.z) {
        reg(0.0004)
    } else {
        fp::fneg(reg(0.0004))
    };
    let away = fp::fneg(toward);
    while (reg(prev.z) > 0.0 && reg(cur.z) > 0.0) || (reg(prev.z) < 0.0 && reg(cur.z) < 0.0) {
        let step = if reg(cur.z) < 0.0 { toward } else { away };
        t = fp::fadds(t, step);
        if t > 1.0 {
            t = fp::fsubs(t, 1.0);
        } else if t < 0.0 {
            t = fp::fadds(t, 1.0);
        }
        prev = cur;
        cur = world_spline_point(r, j, mem(t));
    }
    (mem(t), cur)
}

/// The track's lines follow the cars' splines, `grMuteCity_801F0D20`: each end found again
/// from its last place, the ends' lines (straight down, slanting out by 15 degrees, or
/// flat out by 5, by the shape the script set) and the middle line between them over its
/// middle three tenths, then the joint's lines and islands brought up to date.
#[doc(alias = "grMuteCity_801F0D20")]
pub fn follow_track(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let (right, xd8) = {
        let v = r.game.grounds[g.0].u.mute_city();
        (v.xE0, v.xD8)
    };
    let (t, a) = find_track_end(r, right, xd8);
    r.game.grounds[g.0].u.mute_city().xD8 = t;
    let (left, xd4) = {
        let v = r.game.grounds[g.0].u.mute_city();
        (v.xDC, v.xD4)
    };
    let (t, b) = find_track_end(r, left, xd4);
    r.game.grounds[g.0].u.mute_city().xD4 = t;
    r.game.geometry_mut().settle_vertices(TRACK_JOINT);
    let mode = r.game.grounds[g.0].u.mute_city().ends();
    let geo = r.game.geometry_mut();
    let (ax, ay, bx, by) = (reg(a.x), reg(a.y), reg(b.x), reg(b.y));
    match mode {
        1 => {
            geo.set_line_pos(LINE_LEFT, a.x, mem(fp::fadds(30.0, ay)), a.x, a.y);
            geo.set_line_pos(LINE_RIGHT, b.x, b.y, b.x, mem(fp::fadds(30.0, by)));
        }
        2 => {
            let angle = reg(crate::math::atan2(
                mem(fp::fsubs(by, ay)),
                mem(fp::fsubs(bx, ax)),
            ));
            let back = mem(fp::fsubs(
                fp::fadds(reg(std::f32::consts::PI), angle),
                reg(0.261_799_4),
            ));
            let fwd = mem(fp::fadds(reg(0.261_799_4), angle));
            let y0 = mem(fp::fmadds(10.0, reg(crate::math::sin(back)), ay));
            let x0 = mem(fp::fmadds(10.0, reg(crate::math::cos(back)), ax));
            let geo = r.game.geometry_mut();
            geo.set_line_pos(LINE_LEFT, x0, y0, a.x, a.y);
            let y1 = mem(fp::fmadds(10.0, reg(crate::math::sin(fwd)), by));
            let x1 = mem(fp::fmadds(10.0, reg(crate::math::cos(fwd)), bx));
            let geo = r.game.geometry_mut();
            geo.set_line_pos(LINE_RIGHT, b.x, b.y, x1, y1);
        }
        _ => {
            geo.set_line_pos(LINE_LEFT, mem(fp::fsubs(ax, 5.0)), a.y, a.x, a.y);
            geo.set_line_pos(LINE_RIGHT, b.x, b.y, mem(fp::fadds(5.0, bx)), b.y);
        }
    }
    {
        let v = r.game.grounds[g.0].u.mute_city();
        v.xE4 = a;
        v.xF0 = b;
    }
    let d = b - a;
    let lerp = |k: f32, x: f32, o: f64| mem(fp::fmadds(reg(k), reg(x), o));
    let geo = r.game.geometry_mut();
    geo.set_line_pos(
        LINE_MID,
        lerp(0.35, d.x, ax),
        lerp(0.35, d.y, ay),
        lerp(0.65, d.x, ax),
        lerp(0.65, d.y, ay),
    );
    geo.update_dynamics_and_islands(TRACK_JOINT);
    geo.update_joint_islands(TRACK_JOINT);
}

/// `grMc_803E3B7C`: the cars' places on the grid and their lanes.
const GRID: [(f32, f32); 30] = [
    (0.9999, 0.37),
    (0.9999, 0.87),
    (0.9988, 0.37),
    (0.9988, 0.87),
    (0.9977, 0.37),
    (0.9977, 0.87),
    (0.9966, 0.37),
    (0.9966, 0.87),
    (0.9955, 0.37),
    (0.9955, 0.87),
    (0.9944, 0.37),
    (0.9944, 0.87),
    (0.9933, 0.37),
    (0.9933, 0.87),
    (0.9922, 0.37),
    (0.9922, 0.87),
    (0.9911, 0.37),
    (0.9911, 0.87),
    (0.99, 0.37),
    (0.99, 0.87),
    (0.9889, 0.37),
    (0.9889, 0.87),
    (0.9878, 0.37),
    (0.9878, 0.87),
    (0.9867, 0.37),
    (0.9867, 0.87),
    (0.9856, 0.37),
    (0.9856, 0.87),
    (0.9845, 0.37),
    (0.9845, 0.87),
];

/// The cars on the grid, `grMuteCity_801F0F4C`: each at its place (wrapped into 0 to 1) in
/// its lane, standing, undamaged, unwrecked, with no item or generator, in grid order.
#[doc(alias = "grMuteCity_801F0F4C")]
pub fn place_cars(game: &mut Game) {
    for (i, &(at, lane)) in GRID.iter().enumerate() {
        let mut pos = reg(at);
        if pos > 1.0 {
            pos = fp::fsubs(pos, 1.0);
        } else if pos < 0.0 {
            pos = fp::fadds(pos, 1.0);
        }
        let c = &mut game.gr.mutecity.cars[i];
        c.x0 = mem(pos);
        c.x4 = mem(pos);
        c.xC = lane;
        c.x8 = 0.0;
        c.x10 = 0.0;
        c.x20 = 0;
        c.x22 &= !0xC0;
        c.item = None;
        c.generator = false;
        game.gr.mutecity.order[i] = i as i32;
    }
}

/// One car's speed and lane for the tick, `grMuteCity_801F106C`. A car close behind
/// another brakes beside it, or else now and then brakes or swerves to pass on its faster
/// side; a free car speeds up and drifts back toward the middle lanes; a car beside another
/// swerves away to its faster side; its speed stays within its pack's top speed. A wrecked
/// car keeps a crawl through the pit lane (0.827 to 0.914) and slows to a stop elsewhere.
#[doc(alias = "grMuteCity_801F106C")]
pub fn steer_car(game: &mut Game, data: &Archives, i: usize) {
    let p = param(game);
    let rng = &mut game.objs.rng;
    let c = &mut game.gr.mutecity.cars[i];
    let flags = c.x20;
    if c.wrecked() {
        if reg(c.x4) > reg(0.827) && reg(c.x4) < reg(0.914) {
            if reg(c.x8) < reg(0.001) {
                c.x8 = 0.001;
            }
        } else {
            c.x8 = mem(fp::fmuls(reg(c.x8), reg(0.95)));
            if reg(c.x8) < reg(0.00001) {
                c.x8 = 0.0;
            }
        }
        return;
    }
    let pf = |off: u32| reg(p.f(data, off));
    if flags & 1 != 0 {
        if flags & 8 != 0 {
            c.x8 = mem(fp::fsubs(reg(c.x8), pf(p::BRAKE)));
        } else {
            match rng.below(4) {
                0 => c.x8 = mem(fp::fsubs(reg(c.x8), pf(p::BRAKE))),
                1 | 2 => {
                    if flags & 4 != 0 {
                        c.xC = mem(fp::fadds(reg(c.xC), pf(p::SWERVE)));
                        if reg(c.xC) > 0.9 {
                            c.xC = 0.9;
                        }
                    } else {
                        c.xC = mem(fp::fsubs(reg(c.xC), pf(p::SWERVE)));
                        if reg(c.xC) < 0.1 {
                            c.xC = 0.1;
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        c.x8 = mem(fp::fadds(reg(c.x8), pf(p::ACCEL)));
        let center = pf(p::CENTER);
        if reg(c.xC) > fp::fadds(reg(0.7), center) {
            c.xC = mem(fp::fsubs(reg(c.xC), center));
        } else if reg(c.xC) < fp::fsubs(reg(0.3), center) {
            c.xC = mem(fp::fadds(reg(c.xC), center));
        }
    }
    if flags & 8 != 0 {
        if flags & 4 != 0 {
            c.xC = mem(fp::fadds(reg(c.xC), pf(p::SWERVE)));
            if reg(c.xC) > 1.0 {
                c.xC = 1.0;
            }
        } else {
            c.xC = mem(fp::fsubs(reg(c.xC), pf(p::SWERVE)));
            if reg(c.xC) < 0.0 {
                c.xC = 0.0;
            }
        }
    }
    let top = if flags & 0x10 != 0 {
        pf(p::TOP_SPEED_LEAD)
    } else if flags & 0x20 != 0 {
        pf(p::TOP_SPEED_BACK)
    } else {
        pf(p::TOP_SPEED)
    };
    if reg(c.x8) > top {
        c.x8 = mem(top);
        return;
    }
    if reg(c.x8) < 0.0 {
        c.x8 = 0.0;
    }
}

/// The car at place `k` in the order, where `k` -1 is the word before it.
fn ordered(m: &Globals, k: i32) -> i32 {
    if k < 0 {
        m.order_before
    } else {
        m.order[k as usize]
    }
}

fn set_ordered(m: &mut Globals, k: i32, v: i32) {
    if k < 0 {
        m.order_before = v;
    } else {
        m.order[k as usize] = v;
    }
}

/// The race for the tick, `grMuteCity_801F1328`: the order sorted by distance gone (an
/// insertion that also reaches the word before the order, [`Globals::order_before`]);
/// then for each car still racing, the racing cars within 0.002 of the track ahead of it
/// mark the one behind as close, both beside each other within 30, and which is faster;
/// the first five and the last five marked; and each car's speed and lane.
#[doc(alias = "grMuteCity_801F1328")]
pub fn steer_cars(r: &mut Running) {
    let m = &mut r.game.gr.mutecity;
    for i in 1..CARS as i32 {
        let mut j = i;
        while j >= 0 {
            let temp = ordered(m, j);
            let a = m.cars[temp as usize].x0;
            let b = m.cars[ordered(m, j - 1) as usize].x0;
            if reg(a) > reg(b) {
                let prev = ordered(m, j - 1);
                set_ordered(m, j, prev);
                set_ordered(m, j - 1, temp);
            } else {
                break;
            }
            j -= 1;
        }
    }
    for i in 0..CARS {
        let k = m.order[i] as usize;
        m.cars[k].x20 = 0;
    }
    for i in 0..CARS {
        let me = m.order[i] as usize;
        if m.cars[me].wrecked() {
            continue;
        }
        for j in 1..CARS {
            let other = m.order[(i + j) % CARS] as usize;
            if m.cars[other].wrecked() {
                continue;
            }
            let other_pos = reg(m.cars[other].x4);
            let my_pos = reg(m.cars[me].x4);
            let dist = if my_pos > other_pos {
                fp::fsubs(my_pos, other_pos)
            } else {
                fp::fadds(1.0, fp::fsubs(my_pos, other_pos))
            };
            if dist > 0.002 {
                break;
            }
            m.cars[other].x20 |= 1;
            let (a, b) = (m.cars[me].pos, m.cars[other].pos);
            let sq = |x: f32, y: f32| {
                let d = fp::fsubs(reg(x), reg(y));
                fp::fmuls(d, d)
            };
            let d2 = fp::fadds(sq(a.z, b.z), fp::fadds(sq(a.x, b.x), sq(a.y, b.y)));
            if d2 < 900.0 {
                m.cars[me].x20 |= 8;
                m.cars[other].x20 |= 8;
            }
            if reg(m.cars[me].xC) > reg(m.cars[other].xC) {
                m.cars[me].x20 |= 4;
                m.cars[other].x20 |= 2;
            } else {
                m.cars[me].x20 |= 2;
                m.cars[other].x20 |= 4;
            }
        }
    }
    for i in 0..5 {
        let k = m.order[i] as usize;
        m.cars[k].x20 |= 0x10;
    }
    for i in 25..CARS {
        let k = m.order[i] as usize;
        m.cars[k].x20 |= 0x20;
    }
    for i in 0..CARS {
        steer_car(r.game, r.data, i);
    }
}

/// A car's hurtbox item took a hit, `grMuteCity_801F173C` to `grMuteCity_801F19F4` (one per
/// car, `grMc_803E3C6C`): the car takes the damage.
#[doc(alias = "grMuteCity_801F173C")]
#[doc(alias = "grMuteCity_801F1754")]
#[doc(alias = "grMuteCity_801F176C")]
#[doc(alias = "grMuteCity_801F1784")]
#[doc(alias = "grMuteCity_801F179C")]
#[doc(alias = "grMuteCity_801F17B4")]
#[doc(alias = "grMuteCity_801F17CC")]
#[doc(alias = "grMuteCity_801F17E4")]
#[doc(alias = "grMuteCity_801F17FC")]
#[doc(alias = "grMuteCity_801F1814")]
#[doc(alias = "grMuteCity_801F182C")]
#[doc(alias = "grMuteCity_801F1844")]
#[doc(alias = "grMuteCity_801F185C")]
#[doc(alias = "grMuteCity_801F1874")]
#[doc(alias = "grMuteCity_801F188C")]
#[doc(alias = "grMuteCity_801F18A4")]
#[doc(alias = "grMuteCity_801F18BC")]
#[doc(alias = "grMuteCity_801F18D4")]
#[doc(alias = "grMuteCity_801F18EC")]
#[doc(alias = "grMuteCity_801F1904")]
#[doc(alias = "grMuteCity_801F191C")]
#[doc(alias = "grMuteCity_801F1934")]
#[doc(alias = "grMuteCity_801F194C")]
#[doc(alias = "grMuteCity_801F1964")]
#[doc(alias = "grMuteCity_801F197C")]
#[doc(alias = "grMuteCity_801F1994")]
#[doc(alias = "grMuteCity_801F19AC")]
#[doc(alias = "grMuteCity_801F19C4")]
#[doc(alias = "grMuteCity_801F19DC")]
#[doc(alias = "grMuteCity_801F19F4")]
pub fn car_hit(game: &mut Game, car: usize, damage: f32) {
    let c = &mut game.gr.mutecity.cars[car];
    c.x10 = mem(fp::fadds(reg(c.x10), reg(damage)));
}

/// Which car a hit callback's address names (`grMc_803E3C6C`), if any.
pub fn hit_car(cb: u32) -> Option<usize> {
    let off = cb.checked_sub(CAR_HIT)?;
    (off % 0x18 == 0 && off / 0x18 < CARS as u32).then_some((off / 0x18) as usize)
}

/// A car's hurtbox item dealt a hit, `grMuteCity_801F1A0C`: a crash sound.
#[doc(alias = "grMuteCity_801F1A0C")]
pub fn car_dealt(r: &mut Running) {
    r.stage_sound(0x5CC60);
}

/// A generator of effect `idx` on a car's joint at the stage's scale, its particles in the
/// stage's frame, `grMuteCity_801F2AB0`. Returns what the original leaves in r3 ([`Car::generator`]).
#[doc(alias = "grMuteCity_801F2AB0")]
pub fn car_generator(r: &mut Running, idx: i32, j: Option<JObjId>) -> bool {
    let Some(g) = grlib::generator_on(r.game, r.data, idx, idx / 1000, j) else {
        return false;
    };
    let objs = &mut r.game.objs;
    let s = match objs.generators[g].appsrt {
        Some(s) => s,
        None => crate::hsd::appsrt::add_to_generator(objs, g, 0),
    };
    r.game.objs.app_srts[s].xa2 = 0;
    let k = super::ground::scale(r.game);
    let srt = &mut r.game.objs.app_srts[s];
    srt.scale.z = k;
    srt.scale.y = k;
    srt.scale.x = k;
    let gen_ = &mut r.game.objs.generators[g];
    gen_.ty &= !0x600;
    gen_.ty |= 0x800;
    r.game.objs.app_srts[s].gp = Some(g);
    r.game.gr.param.is_some()
}

/// The cars' moves for the tick, `grMuteCity_801F1A34` (`car_gobj` the cars' object, `gobj`
/// the course). For each car: it goes on by its speed (its place wrapping round); it stands
/// between the two splines by its lane, turned along the track (in the pit lane it faces as
/// at the start line) and lifted 3 off it; a close car beside a faster one sounds its horn
/// (an effect, and a sound by its distance in front of the camera, at most one a tick, then
/// a wait of seven); past its wreck damage a car still racing is wrecked (its smoke stopped,
/// a burst, its model swapped for the wreck's at a random tilt, its item gone), past its
/// smoke damage it smokes; a racing car near the track's middle and the stage shakes the
/// screen with a passing sound (louder while the script asks) once a pass, and in front of
/// the stage it carries a hurtbox item; a wrecked car smokes while in view.
#[doc(alias = "grMuteCity_801F1A34")]
pub fn move_cars(r: &mut Running, car_gobj: GObjId, gobj: GObjId) {
    let cg = ground_of(r.game, car_gobj);
    let g = ground_of(r.game, gobj);
    let car_root = jobj_of(r.game, car_gobj);
    let _ = super::ground::scale(r.game);
    let (xd4, xd8, left, right) = {
        let v = r.game.grounds[g.0].u.mute_city();
        (v.xD4, v.xD8, v.xDC, v.xE0)
    };
    let mid = fp::fmuls(reg(0.5), fp::fadds(reg(xd4), reg(xd8)));
    let mid_next = fp::fadds(1.0, mid);
    let cam = crate::game::camera::position(&r.game.camera.cam);
    let mut j = car_root.and_then(|c| r.game.objs.jobjs[c].child);
    {
        let v = r.game.grounds[cg.0].u.mute_city_cars();
        if v.xC4 != 0 {
            v.xC4 -= 1;
        }
    }
    let rspline = spline_of(r.game, right).expect("grmutecity.c: the right spline");
    let lspline = spline_of(r.game, left).expect("grmutecity.c: the left spline");
    let mut sounds = 0;
    for i in 0..CARS {
        {
            let c = &mut r.game.gr.mutecity.cars[i];
            c.x4 = mem(fp::fadds(reg(c.x4), reg(c.x8)));
            c.x0 = mem(fp::fadds(reg(c.x0), reg(c.x8)));
            if reg(c.x4) > 1.0 {
                c.x4 = mem(fp::fsubs(reg(c.x4), 1.0));
            }
        }
        let mut t = r.game.gr.mutecity.cars[i].x4;
        let p = spline_at(r.data, rspline, t, false);
        let rp = crate::lb::joint::world_position(&mut r.game.objs, r.data, right, Some(p));
        let p = spline_at(r.data, lspline, t, false);
        let lp = crate::lb::joint::world_position(&mut r.game.objs, r.data, left, Some(p));
        let mut across = lp - rp;
        across.normalize();
        if reg(t) > reg(0.827) && reg(t) < reg(0.914) {
            t = 0.0;
        }
        let tangent = |r: &mut Running, spline, j: Option<JObjId>| {
            let d = spline_at(r.data, spline, t, true);
            let mut d = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(d));
            let o = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
            d = d - o;
            d.normalize();
            d
        };
        let mut along = tangent(r, rspline, right);
        let lt = tangent(r, lspline, left);
        along = along + lt;
        along.normalize();
        let mut up = along.sdk_cross(across);
        up.normalize();
        along = across.sdk_cross(up);
        along.normalize();
        let lane = reg(r.game.gr.mutecity.cars[i].xC);
        let place = |rv: f32, lv: f32, u: f32| {
            let on = fp::fmadds(fp::fsubs(reg(lv), reg(rv)), lane, reg(rv));
            mem(fp::frsp(fp::fmadd(3.0, reg(u), on)))
        };
        let pos = Vec3::new(
            place(rp.x, lp.x, up.x),
            place(rp.y, lp.y, up.y),
            place(rp.z, lp.z, up.z),
        );
        r.game.gr.mutecity.cars[i].pos = pos;
        let cj = j.expect("jobj");
        jobj::set_translate(&mut r.game.objs.jobjs, cj, pos);
        let rot = super::ground::euler_from_axes(along, across, up);
        jobj::set_rotation_x(&mut r.game.objs.jobjs, cj, rot.x);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, cj, rot.y);
        jobj::set_rotation_z(&mut r.game.objs.jobjs, cj, rot.z);
        let flags = r.game.gr.mutecity.cars[i].x20;
        if flags & 8 != 0 && flags & 2 != 0 && r.game.grounds[cg.0].u.mute_city_cars().xC4 == 0 {
            grlib::generator_at(r.game, r.data, 0x11A, 0, pos);
            if sounds < 1 {
                if reg(pos.z) < 0.0 {
                    let d2 = fp::fmadds(
                        reg(pos.z),
                        reg(pos.z),
                        fp::fadds(
                            fp::fmuls(reg(pos.x), reg(pos.x)),
                            fp::fmuls(reg(pos.y), reg(pos.y)),
                        ),
                    );
                    let d = reg(crate::math::sqrt(mem(d2)));
                    if d < 300.0 {
                        let id = match r.game.objs.rng.below(3) {
                            0 => Some(0x5CC69),
                            1 => Some(0x5CC68),
                            2 => Some(0x5CC67),
                            _ => None,
                        };
                        if let Some(id) = id {
                            r.object_sound(Some(cg), 0, id);
                        }
                        let v = mem(fp::fsubs(1.0, fp::fdivs(d, 300.0)));
                        r.object_sound_volume(Some(cg), 0, v);
                        sounds += 1;
                    }
                }
                r.game.grounds[cg.0].u.mute_city_cars().xC4 = 7;
            }
        }
        let age = reg(r.game.gr.mutecity.cars[i].x10);
        let pm = param(r.game);
        if age > reg(pm.f(r.data, p::WRECK_AGE)) {
            if !r.game.gr.mutecity.cars[i].wrecked() {
                grlib::kill_joint_generators_and_particles(r.game, r.data, j);
                r.game.gr.mutecity.cars[i].generator = false;
                grlib::generator_at(r.game, r.data, 0xE3, 0, pos);
                let model = r.game.objs.jobjs[cj].child;
                jobj::unref(&mut r.game.objs, model);
                let wreck = super::ground::load_map_joint(r.game, r.data, 5, 0);
                if let Some(w) = wreck {
                    jobj::add_child(&mut r.game.objs.jobjs, Some(cj), Some(w));
                    let rand = reg(r.game.objs.rng.draw_f32());
                    let tilt = mem(fp::fmsubs(
                        reg(std::f32::consts::FRAC_PI_3),
                        rand,
                        reg(std::f32::consts::FRAC_PI_6),
                    ));
                    jobj::set_srt(&mut r.game.objs.jobjs, w, |o| o.translate.y = tilt);
                    jobj::set_translate(&mut r.game.objs.jobjs, w, Vec3::new(0.0, 0.0, 0.0));
                }
                if let Some(item) = r.game.gr.mutecity.cars[i].item.take() {
                    material::destroy_item(r, item);
                }
                r.game.gr.mutecity.cars[i].x22 |= 0x80;
            }
        } else if age > reg(pm.f(r.data, p::SMOKE_AGE)) && !r.game.gr.mutecity.cars[i].generator {
            let gen_ = car_generator(r, 0x116, j);
            r.game.gr.mutecity.cars[i].generator = gen_;
        }
        let near = |pos: Vec3, z_lo: f32| {
            reg(-200.0) < reg(pos.x)
                && reg(pos.x) < reg(200.0)
                && reg(-200.0) < reg(pos.y)
                && reg(pos.y) < reg(200.0)
                && reg(z_lo) < reg(pos.z)
                && reg(pos.z) < reg(50.0)
        };
        if !r.game.gr.mutecity.cars[i].wrecked() {
            let rel = reg(r.game.gr.mutecity.cars[i].x4);
            let dist = if mid < rel {
                fp::fsubs(mid_next, rel)
            } else {
                fp::fsubs(mid, rel)
            };
            if dist < reg(0.2) && dist > reg(-0.2) {
                if near(pos, -1350.0) {
                    crate::game::camera::request_quake(r, crate::game::camera::QUAKE_LOOP, None);
                    if !r.game.gr.mutecity.cars[i].shook() {
                        let loud = r.game.grounds[g.0].u.mute_city().loud();
                        let ids = if loud {
                            [0x5CC66, 0x5CC65, 0x5CC64]
                        } else {
                            [0x5CC63, 0x5CC62, 0x5CC61]
                        };
                        if let Some(&id) = ids.get(r.game.objs.rng.below(3) as usize) {
                            r.stage_sound(id);
                        }
                        r.game.gr.mutecity.cars[i].x22 |= 0x40;
                    }
                } else if r.game.gr.mutecity.cars[i].shook() {
                    r.game.gr.mutecity.cars[i].x22 &= !0x40;
                }
            }
            if near(pos, -100.0) {
                let c = &r.game.gr.mutecity.cars[i];
                if !c.wrecked() && c.item.is_none() {
                    let hit = CodeRef(CAR_HIT + 0x18 * i as u32);
                    let item = material::stage_item(
                        r,
                        0,
                        2,
                        cg,
                        j,
                        Some(CodeRef(CAR_DEALT)),
                        Some(hit),
                        None,
                    );
                    if let Some(item) = item {
                        material::set_item_hurtbox(
                            r.game,
                            item,
                            Vec3::new(0.0, 0.0, -12.0),
                            Vec3::new(0.0, 0.0, 2.0),
                            15.0,
                        );
                        material::item_hurtbox_on(r.game, item);
                        r.game.gr.mutecity.cars[i].item = Some(item);
                    }
                }
            } else if let Some(item) = r.game.gr.mutecity.cars[i].item.take() {
                material::destroy_item(r, item);
            }
        } else if reg(pos.z) > reg(cam.z) || reg(pos.z) > reg(5000.0) || reg(pos.z) < reg(-1500.0) {
            if r.game.gr.mutecity.cars[i].generator {
                grlib::kill_joint_generators_and_particles(r.game, r.data, j);
                r.game.gr.mutecity.cars[i].generator = false;
            }
        } else if !r.game.gr.mutecity.cars[i].generator {
            let gen_ = car_generator(r, 0x119, j);
            r.game.gr.mutecity.cars[i].generator = gen_;
        }
        j = j.and_then(|c| r.game.objs.jobjs[c].next);
    }
}

/// The platform's lights, `grMuteCity_801F290C`: while its color overlay runs, the first four
/// diffuse lights of the stage's lights (class 12) take its color, their own saved the first
/// time; once it has ended, they get their colors back.
#[doc(alias = "grMuteCity_801F290C")]
pub fn platform_lights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let lights = || {
        let mut cur = r.game.world.first_in(3);
        while let Some(lg) = cur {
            if r.game.world.gobj(lg).classifier == 0xC {
                return Some(lg);
            }
            cur = r.game.world.gobj(lg).next;
        }
        None
    };
    let lg = lights();
    let first = lg.and_then(|lg| first_light(r.game, lg));
    let diffuse = |game: &Game, l: lobj::LObjId| game.objs.lobjs[l].flags & lobj::DIFFUSE != 0;
    if grlib::overlay_done(r.game, gobj) {
        if r.game.grounds[g.0].u.mute_city2().xC4 & 0x80 != 0 {
            if lg.is_some() {
                let mut cur = first;
                let mut i = 0;
                while let Some(l) = cur {
                    if diffuse(r.game, l) {
                        let c = r.game.grounds[g.0].u.mute_city2().saved_colors[i];
                        lobj::set_color(&mut r.game.objs.lobjs, l, c);
                        i += 1;
                        if i >= 4 {
                            break;
                        }
                    }
                    cur = r.game.objs.lobjs[l].next;
                }
            }
            r.game.grounds[g.0].u.mute_city2().xC4 &= !0x80;
        }
        return;
    }
    if lg.is_some() {
        let mut cur = first;
        let mut i = 0;
        let hex = &r.game.grounds[g.0].color_overlay.x2C_hex;
        let color = [hex.r, hex.g, hex.b, hex.a];
        while let Some(l) = cur {
            if diffuse(r.game, l) {
                if r.game.grounds[g.0].u.mute_city2().xC4 & 0x80 == 0 {
                    let c = lobj::color(&r.game.objs.lobjs, l);
                    r.game.grounds[g.0].u.mute_city2().saved_colors[i] = c;
                }
                lobj::set_color(&mut r.game.objs.lobjs, l, color);
                i += 1;
                if i >= 4 {
                    break;
                }
            }
            cur = r.game.objs.lobjs[l].next;
        }
    }
    r.game.grounds[g.0].u.mute_city2().xC4 |= 0x80;
}

/// The track's collision joint's callback, `fn_801F2B58`: while the track hurts, an item
/// standing on it (collision kinds 2, 3 and 5, ground kind 2) is marked to burst
/// (`x34_flags.b7`).
#[doc(alias = "fn_801F2B58")]
pub fn track_touched(
    track_live: i32,
    coll: &mut crate::game::structs::CollData,
    kind: i32,
    item: bool,
) {
    if track_live != 1 {
        return;
    }
    let k = coll.x34_flags.b1234;
    if !(2..=3).contains(&k) && k != 5 {
        return;
    }
    if kind != 2 {
        return;
    }
    if coll.x0_gobj.is_none() || !item {
        return;
    }
    coll.x34_flags.b7 = true;
}

/// The stage's lines' hazards, `grMuteCity_801F2BBC`: while the track hurts, its ends' and its
/// middle's hits.
#[doc(alias = "grMuteCity_801F2BBC")]
pub fn line_hazard(game: &Game, data: &Archives, line: i32) -> Option<DataRef> {
    if game.gr.mutecity.track_live != 1 {
        return None;
    }
    let p = param(game);
    match line {
        LINE_LEFT | LINE_RIGHT => p.ptr(data, p::END_HIT),
        0x32..=0x34 => p.ptr(data, p::MID_HIT),
        _ => None,
    }
}

/// Whether a joint takes a fighter's shadow, `grMuteCity_801F2C10`: the joints the track can
/// cover take it only above the track (from 3 below the line between its ends, within
/// them); every other joint takes it.
#[doc(alias = "grMuteCity_801F2C10")]
pub fn check_shadow_render(game: &mut Game, pos: Vec3, j: Option<JObjId>) -> bool {
    let Some(course) = super::ground::map_gobj(game, 0x1E) else {
        return true;
    };
    let g = ground_of(game, course);
    let v = game.grounds[g.0].u.mute_city();
    if !v.shadow_jobjs.contains(&j) {
        return true;
    }
    let x = reg(pos.x);
    let x_min = reg(v.xE4.x);
    if x < x_min {
        return false;
    }
    let x_max = reg(v.xF0.x);
    if x > x_max {
        return false;
    }
    let slope = fp::fdivs(
        fp::fsubs(reg(v.xF0.y), reg(v.xE4.y)),
        fp::fsubs(x_max, x_min),
    );
    let y = fp::fmadds(slope, fp::fsubs(x, x_min), reg(v.xE4.y));
    reg(pos.y) > fp::fsubs(y, 3.0)
}
