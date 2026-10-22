//! Big Blue, `melee/gr/grbigblue.c`: the fighters race along on the F-Zero cars and
//! platforms above a road that rushes past below.
//!
//! The road (map 34) is a chain of pieces (`TRACK`) the stage keeps two of shown, the current
//! and the next, moving them left under the camera and turning with the ground's slope;
//! dives, climbs and a loop come by at random. Four big cars (map 33) drive on the road, each
//! one of thirty models under a collision joint of its own; they come in from either side,
//! keep their order and spacing, bob and bounce, and fly off where the road falls away. Three
//! flying platforms (map 32) cross the screen over the road from one side to the other,
//! dipping where the fighters land and steering round the cars; the Falcon Flyer (map 35)
//! waits over the stage at the start and flies off, and a relay platform (map 36) crosses
//! now and then. A wind blows airborne items left (`wind`).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
// Fields keep the original's offset names.
#![allow(non_snake_case)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{BIGBLUE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `fn_801E8560`, the flying platforms' collision joints' callback (`mpJointSetCb1`).
pub const PLATFORM_TOUCHED: u32 = 0x801E_8560;
/// `fn_801EF60C`, the big cars' collision joints' callback.
pub const CAR_TOUCHED: u32 = 0x801E_F60C;
/// `fn_801E6124`, the road's start hook.
const ROAD_START: u32 = 0x801E_6124;
/// `fn_801EFB9C`, the background's render callback.
pub const RENDER_BACKGROUND: u32 = 0x801E_FB9C;

/// `-F32_MAX`, the ground search's "nothing found".
const NONE_Y: f32 = -f32::MAX;

/// The fields of `grBb_YakumonoParam`, by offset.
mod p {
    pub const ROAD_Y: u32 = 0;
    pub const DIVE: u32 = 4;
    pub const CLIMB: u32 = 8;
    pub const CAR_SCALE: u32 = 0xC;
    pub const SPAWN_MAX: u32 = 0x10;
    pub const SPAWN_MIN: u32 = 0x14;
    pub const CARS: u32 = 0x18;
    pub const SPREAD: u32 = 0x1C;
    pub const GAP: u32 = 0x20;
    pub const TOP_SPEED: u32 = 0x24;
    pub const ACCEL: u32 = 0x28;
    pub const RIDE: u32 = 0x2C;
    pub const BOB_BACK: u32 = 0x30;
    pub const BOB: u32 = 0x34;
    pub const BOB_FORE: u32 = 0x38;
    pub const BOB_EASE: u32 = 0x3C;
    pub const BOB_TURN: u32 = 0x40;
    pub const PUSH: u32 = 0x44;
    pub const FALL: u32 = 0x48;
    pub const FALL_MAX: u32 = 0x4C;
    pub const TILT_EASE: u32 = 0x50;
    pub const TILT_BACK: u32 = 0x54;
    pub const TILT_REST: u32 = 0x58;
    pub const WAIT_MIN: u32 = 0x5C;
    pub const WAIT_MAX: u32 = 0x60;
    pub const OFF_SIDE: u32 = 0x68;
    pub const ROAD_X: u32 = 0x6C;
    pub const STEER_MAX: u32 = 0x70;
    pub const TURN: u32 = 0x74;
    pub const DROP: u32 = 0x78;
    pub const DRIFT: u32 = 0x7C;
    pub const AIR_FALL: u32 = 0x80;
    pub const LAUNCH: u32 = 0x84;
    pub const PLAT_WAIT: u32 = 0x88;
    pub const PLAT_WAIT_ADD: u32 = 0x8C;
    pub const PLAT_HEIGHT: u32 = 0x90;
    pub const PLAT_HEIGHT_MAX: u32 = 0x94;
    pub const PLAT_SPEED: u32 = 0x98;
    pub const PLAT_TURN: u32 = 0x9C;
    pub const PLAT_PUSH: u32 = 0xA0;
    pub const PLAT_PUSH_MAX: u32 = 0xA4;
    pub const PLAT_SINK: u32 = 0xA8;
    pub const PLAT_RISE: u32 = 0xAC;
    pub const PLAT_DIP: u32 = 0xB0;
    pub const PLAT_DIP_WAIT: u32 = 0xB4;
    pub const PLAT_ITEM: u32 = 0xB8;
    pub const PLAT_UP: u32 = 0xBC;
    pub const PLAT_UP_MAX: u32 = 0xC0;
    pub const PLAT_DOWN: u32 = 0xC4;
    pub const PLAT_DOWN_MAX: u32 = 0xC8;
    pub const FLYER_HEIGHT: u32 = 0xCC;
    pub const FLYER_SPEED: u32 = 0xD0;
    pub const FLYER_TURN: u32 = 0xD4;
    pub const FLYER_WAIT: u32 = 0xD8;
    pub const FLYER_COUNT: u32 = 0xDC;
    pub const FLYER_COUNT_MAX: u32 = 0xE0;
    pub const FLYER_UP: u32 = 0xE4;
    pub const FLYER_UP_MAX: u32 = 0xE8;
    pub const FLYER_DOWN: u32 = 0xEC;
    pub const FLYER_DOWN_MAX: u32 = 0xF0;
    pub const RELAY_HEIGHT: u32 = 0xF4;
    pub const RELAY_HEIGHT_MAX: u32 = 0xF8;
    pub const RELAY_SPEED_MIN: u32 = 0xFC;
    pub const RELAY_SPEED_MAX: u32 = 0x100;
    pub const RELAY_RISE_MIN: u32 = 0x104;
    pub const RELAY_RISE_MAX: u32 = 0x108;
    pub const RELAY_WAIT_MIN: u32 = 0x10C;
    pub const RELAY_WAIT_MAX: u32 = 0x110;
    pub const RELAY_COUNT: u32 = 0x11C;
    pub const RELAY_COUNT_MAX: u32 = 0x120;
    pub const RELAY_UP: u32 = 0x124;
    pub const RELAY_UP_MAX: u32 = 0x128;
    pub const RELAY_DOWN: u32 = 0x12C;
    pub const RELAY_DOWN_MAX: u32 = 0x130;
    pub const BACK_POS: u32 = 0x134;
    pub const BACK_SCALE: u32 = 0x140;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f64 {
        reg(data.f32(self.0.at(off)))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .bigblue_yakumono
            .expect("grbigblue.c: yakumono_param"),
    )
}

/// The stage's scale (`Ground_801C0498`) in a register.
fn scale(game: &Game) -> f64 {
    reg(super::ground::scale(game))
}

/// An integer as the compiler converts it (`(f32) i`).
fn to_f(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// `ABS` in a register.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// `HSD_Randi(n)` when `n` isn't 0, else 0 (the stage's `range != 0 ? HSD_Randi(range) : 0`).
fn randi(game: &mut Game, n: i32) -> i32 {
    if n != 0 { game.objs.rng.below(n) } else { 0 }
}

/// `grBb_803B8114`: the flying platforms' half widths (and the speeds the platforms cross
/// at), by platform.
const PLATFORM_SIZE: [f32; 3] = [56.0, 40.0, 24.0];

/// `grBb_803B8134`: the road's collision joints the ground search looks through.
const ROAD_JOINTS: [i32; 32] = [
    33, 35, 38, 39, 40, 36, 37, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57,
    58, 59, 60, 61, 62, 63, 64, 65,
];

/// A piece of road (`grBb_TrackEntry`): its joint, the joints at its start and its end (steps
/// into the road's model), whose distance the setup measures (`grBb_TrackEntries[i].delta`,
/// state in [`Globals::track_deltas`]).
struct Piece {
    jobj: i32,
    start: i32,
    end: i32,
}

const fn piece(jobj: i32, start: i32, end: i32) -> Piece {
    Piece { jobj, start, end }
}

/// `grBb_TrackEntries`.
const TRACK: [Piece; 12] = [
    piece(4, 6, 5),
    piece(19, 21, 20),
    piece(7, 9, 8),
    piece(10, 12, 11),
    piece(44, 46, 45),
    piece(1, 3, 2),
    piece(32, 34, 33),
    piece(35, 37, 36),
    piece(38, 40, 39),
    piece(41, 43, 42),
    piece(29, 31, 30),
    piece(13, 18, 17),
];

/// grbigblue.c's statics besides its `yakumono_param`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// Each road piece's length from its start to its end (`grBb_TrackEntries[i].delta`, in the
    /// executable's data), which every Big Blue setup measures and nothing clears.
    pub track_deltas: [Vec3; 12],
}

/// A stage object's process or callback, by address: Big Blue's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801E_5B18 | 0x801E_5B1C | 0x801E_611C | 0x801E_6120 | 0x801E_61FC | 0x801E_6290
        | 0x801E_6294 | 0x801E_635C | 0x801E_6360 | 0x801E_855C | 0x801E_9F38 | 0x801E_AB4C => {
            empty(r, gobj)
        }
        0x801E_5AE4 => init_back(r, gobj),
        0x801E_5B20 => init_background(r, gobj),
        ROAD_START => start_road(r, gobj),
        0x801E_613C => init_road(r, gobj),
        0x801E_61C4 => proc_road(r, gobj),
        0x801E_6200 => init_map2(r, gobj),
        0x801E_6298 => init_map1(r, gobj),
        0x801E_6364 => init_cars(r, gobj),
        0x801E_6884 => proc_cars(r, gobj),
        0x801E_68B8 => free_cars(r, gobj),
        0x801E_6904 => init_platforms(r, gobj),
        0x801E_6C60 => proc_platforms(r, gobj),
        0x801E_8D64 => init_flyer(r, gobj),
        0x801E_93D8 => proc_flyer(r, gobj),
        0x801E_9F3C => init_relay(r, gobj),
        0x801E_A05C => proc_relay(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801E_57C0 => on_init(r),
        0x801E_59C8 => on_load(r),
        0x801E_59CC => on_start(r),
        _ => return false,
    }
    true
}

/// The stage's render callbacks, by address.
pub fn run_render(r: &mut Running, gobj: GObjId, cb: u32, pass: i32) -> bool {
    match cb {
        RENDER_BACKGROUND => render_background(r, gobj, pass),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grBigBlue_801E5B18")]
#[doc(alias = "grBigBlue_801E5B1C")]
#[doc(alias = "grBigBlue_801E611C")]
#[doc(alias = "grBigBlue_801E6120")]
#[doc(alias = "grBigBlue_801E61FC")]
#[doc(alias = "grBigBlue_801E6290")]
#[doc(alias = "grBigBlue_801E6294")]
#[doc(alias = "grBigBlue_801E635C")]
#[doc(alias = "grBigBlue_801E6360")]
#[doc(alias = "grBigBlue_801E855C")]
#[doc(alias = "grBigBlue_801E9F38")]
#[doc(alias = "grBigBlue_801EAB4C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grBigBlue_801E59F8`.
#[doc(alias = "grBigBlue_801E59F8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grBb_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &BIGBLUE_CALLBACKS[map_id as usize]
}

/// Where the cars' setup makes the cars' model object (map 4): a frame below the stage's
/// setup's, less `grBigBlue_801E59F8`'s and the setup's own.
pub const CARS_FRAME: u32 = setup::SETUP_FRAME - 0x28 - 0x68;

/// Big Blue's setup, `grBigBlue_801E57C0`: its parameters, maps 31, 0, 1, 2, 34, 33, 32
/// (hidden), 35 and 36 (hidden), the camera limits and blast zones, the road's joints kept
/// out of the island links, and the wind blowing left.
#[doc(alias = "grBigBlue_801E57C0")]
pub fn on_init(r: &mut Running) {
    r.game.gr.bigblue_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0x1F, 0, 1, 2, 0x22, 0x21] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    let hide = |r: &mut Running, g: Option<GObjId>| {
        let j = g.and_then(|g| jobj_of(r.game, g));
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    };
    let g = create(r, 0x20, setup::SETUP_FRAME);
    hide(r, g);
    create(r, 0x23, setup::SETUP_FRAME);
    let g = create(r, 0x24, setup::SETUP_FRAME);
    hide(r, g);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    for joint in 0x21..=0x41 {
        r.game.geometry_mut().forbid_islands(joint);
    }
    crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(-1.0, 0.0, 0.0),
        -1,
        1.0,
        0.0,
        std::f32::consts::FRAC_PI_3,
        -100000.0,
        100000.0,
        100000.0,
        -100000.0,
    );
}

/// `grBigBlue_801E57BC` does nothing (only a demo runs it).
#[doc(alias = "grBigBlue_801E57BC")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grBigBlue_801E59C8` does nothing.
#[doc(alias = "grBigBlue_801E59C8")]
pub fn on_load(_r: &mut Running) {}

/// Big Blue's start, `grBigBlue_801E59CC`: the enemy spawner, without enemies.
#[doc(alias = "grBigBlue_801E59CC")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// A stage object's `Ground` by map id (`Ground_GetMapGObj(id)->user_data`).
fn map_ground(game: &Game, map: i32) -> GroundId {
    let g = super::ground::map_gobj(game, map).expect("grbigblue.c: a map's object");
    ground_of(game, g)
}

/// A joint's translation (`HSD_JObjGetTranslation`).
fn translation(game: &Game, j: Option<JObjId>) -> Vec3 {
    game.objs.jobjs[j.expect("grbigblue.c: a joint")].translate
}

fn set_tx(game: &mut Game, j: Option<JObjId>, v: f32) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate.x = v);
}

fn set_ty(game: &mut Game, j: Option<JObjId>, v: f32) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate.y = v);
}

fn set_tz(game: &mut Game, j: Option<JObjId>, v: f32) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate.z = v);
}

fn set_translate(game: &mut Game, j: Option<JObjId>, v: Vec3) {
    jobj::set_translate(&mut game.objs.jobjs, j.expect("jobj"), v);
}

fn set_scale(game: &mut Game, j: Option<JObjId>, v: Vec3) {
    jobj::set_scale(&mut game.objs.jobjs, j.expect("jobj"), v);
}

fn rot_z(game: &Game, j: Option<JObjId>) -> f64 {
    reg(game.objs.jobjs[j.expect("jobj")].rotate.z)
}

fn set_rot_z(game: &mut Game, j: Option<JObjId>, v: f32) {
    jobj::set_rotation_z(&mut game.objs.jobjs, j.expect("jobj"), v);
}

/// `HSD_JObjAddRotationZ`.
fn add_rot_z(game: &mut Game, j: Option<JObjId>, d: f64) {
    let j = j.expect("jobj");
    let z = game.objs.jobjs[j].rotate.z;
    jobj::set_rotation_z(&mut game.objs.jobjs, j, mem(fp::fadds(reg(z), d)));
}

/// `HSD_JObjAddTranslationX` and `Y`.
fn add_tx(game: &mut Game, j: Option<JObjId>, d: f64) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.translate.x = mem(fp::fadds(reg(o.translate.x), d));
    });
}

fn add_ty(game: &mut Game, j: Option<JObjId>, d: f64) {
    let j = j.expect("jobj");
    jobj::set_srt(&mut game.objs.jobjs, j, |o| {
        o.translate.y = mem(fp::fadds(reg(o.translate.y), d));
    });
}

/// Map 0's setup, `grBigBlue_801E5AE4`: its animation.
#[doc(alias = "grBigBlue_801E5AE4")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 31), `grBigBlue_801E5B20`: drawn by its own render callback,
/// placed and scaled by the parameters, its model's first two joints at the origin.
#[doc(alias = "grBigBlue_801E5B20")]
pub fn init_background(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(RENDER_BACKGROUND));
    let pm = param(r.game);
    let f = |o: u32| mem(pm.f(r.data, o));
    let (tx, ty, tz, s) = (
        f(p::BACK_POS),
        f(p::BACK_POS + 4),
        f(p::BACK_POS + 8),
        f(p::BACK_SCALE),
    );
    set_tx(r.game, j, tx);
    set_ty(r.game, j, ty);
    set_tz(r.game, j, tz);
    let jj = j.expect("jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.x = s);
    jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.y = s);
    jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.z = s);
    let mut c = r.game.objs.jobjs[jj].child;
    for _ in 0..2 {
        let Some(cj) = c else { break };
        set_tx(r.game, c, 0.0);
        set_ty(r.game, c, 0.0);
        set_tz(r.game, c, 0.0);
        c = r.game.objs.jobjs[cj].child;
    }
}

/// The background's render callback, `fn_801EFB9C`: drawn as any stage object
/// (`grDisplay_801C5DB0`) unless the debug pause or the free camera is on.
#[doc(alias = "fn_801EFB9C")]
pub fn render_background(r: &mut Running, gobj: GObjId, pass: i32) {
    if r.game.db_pause_flag(1)
        || r.game.db_pause_flag(2)
        || crate::game::camera::is_free(&r.game.camera.cam)
    {
        return;
    }
    r.render_stage_object(gobj, pass);
}

/// The road's start hook, `fn_801E6124`: it stops waiting.
#[doc(alias = "fn_801E6124")]
pub fn start_road(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.bigblue_road().set_waiting(false);
}

/// The road's setup (map 34), `grBigBlue_801E613C`: its animation, its collision, its pieces
/// laid out, waiting for the match's start (its start hook).
#[doc(alias = "grBigBlue_801E613C")]
pub fn init_road(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    setup::init_map_coll(r.game, r.data, j, m);
    lay_road(r, gobj);
    r.game.grounds[g.0].u.bigblue_road().set_waiting(true);
    setup::add_start_hook(r.game, gobj, CodeRef(ROAD_START));
}

/// The road's process (map 34), `grBigBlue_801E61C4`: the road's move, the wind sources and
/// the collision.
#[doc(alias = "grBigBlue_801E61C4")]
pub fn proc_road(r: &mut Running, gobj: GObjId) {
    move_road(r, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    update_map_coll(r.game, r.data, gobj);
}

/// Map 2's setup, `grBigBlue_801E6200`: its animation, no shadows, its materials' z writes
/// off and blending on, drawn in layer 1.
#[doc(alias = "grBigBlue_801E6200")]
pub fn init_map2(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b2 = false;
    material::clear_tree_flags(r.game, j, 0x0800_0000);
    material::set_tree_flags(r.game, j, 0x2000_0000);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// Map 1's setup, `grBigBlue_801E6298`: its animation, its six guide points (parts 5, 2, 4, 3,
/// 7 and 6), drawn in layer 2.
#[doc(alias = "grBigBlue_801E6298")]
pub fn init_map1(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let j1 = n(r, 6);
    let j2 = n(r, 7);
    let j3 = n(r, 3);
    let j4 = n(r, 4);
    let j5 = n(r, 2);
    let p = n(r, 5);
    super::ground::set_guide_points(r.game, r.data, [p, j5, j4, j3, j2, j1]);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// `grBb_803E2D84`: the big cars' models' map ids (the first, the model object's own,
/// unused).
const CAR_MAPS: [i16; 30] = [
    4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 3, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 37, 38, 39, 40,
];

/// `grBb_803E2DC0`: the cars' collision joints, steps into the cars' object's model.
const CAR_JOINTS: [i32; 30] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30,
];

/// `lbl_803E2DFC`: the cars' collision joints' ids.
const CAR_COLL_JOINTS: [i32; 30] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32,
];

/// The cars' pilots' map.
const PILOT_MAP: i32 = 30;

/// The big cars' setup (map 33), `grBigBlue_801E6364`: its collision at unit scale, two heap
/// blocks (the thirty collision joints and their slots' use), the thirty car models built
/// under the model object (map 4, set apart) and each moved under its collision joint, turned
/// side on at the cars' scale (the tenth moved 10 forward), the model object destroyed, the
/// first cars, and the wait before the next.
#[doc(alias = "grBigBlue_801E6364")]
pub fn init_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    set_scale(r.game, j, Vec3::new(1.0, 1.0, 1.0));
    let ja = crate::hsd::heap::mem_alloc(120);
    let ra = crate::hsd::heap::mem_alloc(30);
    let joints: Vec<_> = CAR_JOINTS
        .iter()
        .map(|&k| nth_joint(r.game, gobj, k))
        .collect();
    {
        let v = r.game.grounds[g.0].u.bigblue_cars();
        v.collision_jobjs_addr = ja;
        v.ranks_addr = ra;
        v.collision_jobjs = joints.clone();
        v.ranks = vec![0; 30];
    }
    let cars = create(r, 4, CARS_FRAME);
    let cars = cars.expect("grbigblue.c: 783: car_gobj");
    super::fzerocar::build(r, cars, &CAR_MAPS, PILOT_MAP, true);
    let root = jobj_of(r.game, cars);
    let mut cur = root.and_then(|j| r.game.objs.jobjs[j].child);
    let pm = param(r.game);
    for (i, coll) in joints.iter().enumerate() {
        let c = cur.expect("grbigblue.c: a car");
        let child = r.game.objs.jobjs[c].child;
        let next = r.game.objs.jobjs[c].next;
        jobj::reparent(&mut r.game.objs.jobjs, Some(c), *coll);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, c, std::f32::consts::FRAC_PI_2);
        let k = mem(fp::fmuls(pm.f(r.data, p::CAR_SCALE), scale(r.game)));
        set_scale(r.game, *coll, Vec3::new(k, k, k));
        let s = r.game.objs.jobjs[child.expect("grbigblue.c: a car's model")].scale;
        let ratio = fp::fdivs(1.0, scale(r.game));
        let s = Vec3::new(
            mem(fp::fmuls(reg(s.x), ratio)),
            mem(fp::fmuls(reg(s.y), ratio)),
            mem(fp::fmuls(reg(s.z), ratio)),
        );
        set_scale(r.game, child, s);
        if i == 9 {
            set_tx(r.game, child, 10.0);
        }
        cur = next;
    }
    setup::destroy(r, Some(cars));
    first_cars(r, gobj);
    let (hi, lo) = (pm.i(r.data, p::SPAWN_MAX), pm.i(r.data, p::SPAWN_MIN));
    let t = spawn_wait(r.game, hi, lo);
    r.game.grounds[g.0].u.bigblue_cars().spawn_timer = t as i16;
}

/// The wait before the next car, between the parameters' two (the inline `max_val > min_val`
/// draw of `grBigBlue_801E6364` and `grBigBlue_801ECB50`).
fn spawn_wait(game: &mut Game, a: i32, b: i32) -> i32 {
    super::grlib::rand_range(&mut game.objs.rng, a, b)
}

/// The big cars' process (map 33), `grBigBlue_801E6884`: the cars, then the collision.
#[doc(alias = "grBigBlue_801E6884")]
pub fn proc_cars(r: &mut Running, gobj: GObjId) {
    drive_cars(r, gobj);
    update_map_coll(r.game, r.data, gobj);
}

/// The big cars' destroy callback, `grBigBlue_801E68B8`: its two heap blocks freed.
#[doc(alias = "grBigBlue_801E68B8")]
pub fn free_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.bigblue_cars();
    crate::hsd::heap::mem_free(v.collision_jobjs_addr);
    v.collision_jobjs.clear();
    v.collision_jobjs_addr = 0;
    crate::hsd::heap::mem_free(v.ranks_addr);
    v.ranks.clear();
    v.ranks_addr = 0;
}

/// The flying platforms' setup (map 32), `grBigBlue_801E6904`: its collision at unit scale,
/// its animation, its three platforms (parts 1, 6 and 11) waiting at the stage's scale, no
/// events, and the platforms' collision joints' callbacks.
#[doc(alias = "grBigBlue_801E6904")]
pub fn init_platforms(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    r.game.grounds[g.0].x10_flags.b5 = true;
    set_scale(r.game, j, Vec3::new(1.0, 1.0, 1.0));
    anime::start_map(r.game, r.data, gobj, m, 0);
    let parts = [1, 6, 11].map(|k| nth_joint(r.game, gobj, k));
    {
        let v = r.game.grounds[g.0].u.bigblue_manager();
        v.jobjs = parts;
        for (i, d) in v.data.iter_mut().enumerate() {
            d.index = i as u8;
            d.x1 = 0;
            d.x50 = None;
        }
        v.flags = 0;
    }
    post_event(r.game, 0, 0, None);
    let s = super::ground::scale(r.game);
    for pj in parts {
        set_scale(r.game, pj, Vec3::new(s, s, s));
    }
    let user = Some(super::CollUser::Ground(g));
    for joint in 0..3 {
        r.game
            .geometry_mut()
            .set_ledge_callback(joint, user, Some(CodeRef(PLATFORM_TOUCHED)));
    }
}

/// Posts an event to the flying platforms, `grBigBlue_801E8978`: its value, and the joint it
/// names when there is one.
#[doc(alias = "grBigBlue_801E8978")]
pub fn post_event(game: &mut Game, index: usize, value: u32, extra: Option<JObjId>) {
    let g = map_ground(game, 0x20);
    let v = game.grounds[g.0].u.bigblue_manager();
    if extra.is_some() {
        v.event_extra = extra;
    }
    v.event[index] = value;
}

/// An event posted to the flying platforms, `grBigBlue_801E89DC`.
#[doc(alias = "grBigBlue_801E89DC")]
pub fn event(game: &mut Game, index: usize) -> u32 {
    let g = map_ground(game, 0x20);
    game.grounds[g.0].u.bigblue_manager().event[index]
}

/// The highest point of the road below `pos` within `half` above and below it, and the
/// ground's normal there, `grBigBlue_801EC58C`: each of the road's collision joints searched
/// for a floor in turn, the highest hit kept; `NONE_Y` when none.
#[doc(alias = "grBigBlue_801EC58C")]
pub fn road_height(game: &mut Game, pos: Vec3, half: f32) -> (f32, Option<Vec3>) {
    let mut max_y = reg(NONE_Y);
    let mut normal = None;
    let (x, y, h) = (reg(pos.x), reg(pos.y), reg(half));
    let top = mem(fp::fadds(y, h));
    let bottom = mem(fp::fsubs(y, h));
    for joint in ROAD_JOINTS {
        let hit = game
            .geometry_mut()
            .check_multiple(mem(x), top, mem(x), bottom, 1, -1, joint);
        if let Some(hit) = hit
            && max_y < reg(hit.pos.y)
        {
            max_y = reg(hit.pos.y);
            normal = Some(hit.normal);
        }
    }
    (mem(max_y), normal)
}

/// Whether a flying platform other than `exclude` flies within `range_x` across and
/// `range_y` up and down of `pos`, or (`left`) any flies left, `grBigBlue_801E8794`.
#[doc(alias = "grBigBlue_801E8794")]
pub fn near_platform(
    game: &mut Game,
    exclude: Option<JObjId>,
    pos: Vec3,
    left: bool,
    range_x: f64,
    range_y: f64,
) -> bool {
    let g = map_ground(game, 0x20);
    let v = game.grounds[g.0].u.bigblue_manager();
    let (jobjs, data) = (v.jobjs, v.data.clone());
    for i in 0..3 {
        if exclude == jobjs[i] {
            continue;
        }
        if data[i].x1 != 3 {
            continue;
        }
        let t = translation(game, jobjs[i]);
        let dx = abs(fp::fsubs(reg(t.x), reg(pos.x)));
        if dx < range_x {
            let dy = abs(fp::fsubs(reg(t.y), reg(pos.y)));
            if dy < range_y {
                return true;
            }
        }
        if left && data[i].x2 == -1 {
            return true;
        }
    }
    false
}

/// Whether the relay platform is near `pos`, `grBigBlue_801EAB50`, as the original asks it:
/// of the flying platforms' object (map 32), whose first byte (its placed platforms' count's
/// top byte) never reaches the relay's flying state 2 (Melee's bug, docs/melee-bugs.md).
#[doc(alias = "grBigBlue_801EAB50")]
pub fn near_relay(game: &mut Game, pos: Vec3, left: bool, range_x: f64, range_y: f64) -> bool {
    let go = super::ground::map_gobj(game, 0x20).expect("grbigblue.c: map 32");
    let g = ground_of(game, go);
    let j = jobj_of(game, go);
    let v = game.grounds[g.0].u.bigblue_manager();
    let (state, dir) = ((v.flags >> 24) as i8, (v.flags >> 16) as i8);
    let mut result = false;
    if state == 2 {
        let t = translation(game, j);
        let dx = abs(fp::fsubs(reg(t.x), reg(pos.x)));
        if dx < range_x {
            let dy = abs(fp::fsubs(reg(t.y), reg(pos.y)));
            if dy < range_y {
                result = true;
            }
        }
        if left && dir == -1 {
            result = true;
        }
    }
    result
}

/// The highest big car within the box `bottom`..`top` (y) and `left`..`right` (x),
/// `grBigBlue_801E8B84` (the original names its arguments right, left, bottom, top);
/// `NONE_Y` when none.
#[doc(alias = "grBigBlue_801E8B84")]
pub fn car_height(game: &mut Game, top: f32, bottom: f32, left: f32, right: f32) -> f32 {
    let g = map_ground(game, 0x21);
    let v = game.grounds[g.0].u.bigblue_cars();
    let mut result = reg(NONE_Y);
    for c in &v.cars {
        if c.state() == 1 {
            continue;
        }
        let (x, y) = (reg(c.pos.x), reg(c.pos.y));
        if y < reg(top) && y > reg(bottom) && x < reg(right) && x > reg(left) && y > result {
            result = y;
        }
    }
    mem(result)
}

/// [`car_height`] over the camera limits, `grBigBlue_801E8D04`.
#[doc(alias = "grBigBlue_801E8D04")]
pub fn car_height_in_view(game: &mut Game) -> f32 {
    let (right, left) = (super::stage::cam_right(game), super::stage::cam_left(game));
    let (bottom, top) = (super::stage::cam_bottom(game), super::stage::cam_top(game));
    car_height(game, top, bottom, left, right)
}

/// The nearest flying platform (or the relay platform) other than `exclude` overlapping the
/// box `half_x` across and `half_y` up and down of `point`, `grBigBlue_801EACE8`: 1 with its
/// height when one is in the box (the nearest in height), 2 with the highest below the point
/// when only those overlap across, else 0.
#[doc(alias = "grBigBlue_801EACE8")]
pub fn obstacle(
    game: &mut Game,
    exclude: Option<JObjId>,
    point: Vec3,
    half_x: f64,
    half_y: f64,
) -> (i32, f32) {
    let (px, py) = (reg(point.x), reg(point.y));
    let left_bound = fp::fsubs(px, half_x);
    let right_bound = fp::fadds(px, half_x);
    let top_bound = fp::fadds(py, half_y);
    let bottom_bound = fp::fsubs(py, half_y);
    let mut best_in = reg(f32::MAX);
    let mut best_above = reg(NONE_Y);
    let mut consider = |y: f64, left_x: f64, right_x: f64| {
        if (right_x > left_bound && right_x < right_bound)
            || (left_x < right_bound && left_x > left_bound)
        {
            if y > bottom_bound && y < top_bound {
                let d = abs(fp::fsubs(py, y));
                if d < best_in {
                    best_in = y;
                }
            } else if py > y && y > best_above {
                best_above = y;
            }
        }
    };
    let g = map_ground(game, 0x20);
    let v = game.grounds[g.0].u.bigblue_manager();
    let (jobjs, data) = (v.jobjs, v.data.clone());
    for i in 0..3 {
        let j = jobjs[i];
        if exclude == j {
            continue;
        }
        if data[i].x1 != 3 {
            continue;
        }
        let pos = translation(game, j);
        let lx = fp::fsubs(reg(pos.x), fp::fmuls(reg(PLATFORM_SIZE[i]), scale(game)));
        let rx = fp::fadds(reg(pos.x), fp::fmuls(reg(PLATFORM_SIZE[i]), scale(game)));
        consider(reg(pos.y), lx, rx);
    }
    let go = super::ground::map_gobj(game, 0x24).expect("grbigblue.c: map 36");
    let j = jobj_of(game, go);
    let rg = ground_of(game, go);
    let state = game.grounds[rg.0].u.bigblue_relay().state();
    if exclude != j && state == 2 {
        let rp = translation(game, j);
        let lx = fp::fnmsubs(fp::fmuls(68.0, scale(game)), 0.5, reg(rp.x));
        let rx = fp::fmadds(fp::fmuls(68.0, scale(game)), 0.5, reg(rp.x));
        consider(reg(rp.y), lx, rx);
    }
    if best_in != reg(f32::MAX) {
        return (1, mem(best_in));
    }
    if best_above != reg(NONE_Y) {
        return (2, mem(best_above));
    }
    (0, 0.0)
}

/// Gives a flying platform an item, `grBigBlue_801E8A1C`: one of the Beam Sword, the Super
/// Scope, the Bunny Hood, the Fire Flower and the Party Ball the rules have on, at random, a
/// little above platform joint `idx` (as the item drops of a bob-omb rain make it). With
/// items off none is on.
#[doc(alias = "grBigBlue_801E8A1C")]
pub fn platform_item(r: &mut Running, idx: usize) {
    let valid = ITEM_KINDS
        .into_iter()
        .filter(|&k| super::super::it::spawn::kind_on(r.game, k))
        .count();
    if valid == 0 {
        return;
    }
    let _ = idx;
    crate::game::code::unported_call("it_8026BE84 (Big Blue's platform item)");
}

/// `grBb_803B8120`: `It_Kind_Sword`, `It_Kind_S_Scope`, `It_Kind_RabbitC`, `It_Kind_F_Flower`,
/// `It_Kind_Kusudama`.
pub const ITEM_KINDS: [i32; 5] = [12, 21, 31, 25, 4];

/// Eases a joint's z turn toward `target` by `k` of the gap a tick (in degrees' share),
/// stopping at the target (the inline repeated in `grBigBlue_801E6C60` and `801E93D8`).
fn ease_turn(game: &mut Game, j: Option<JObjId>, target: f32, k: f64) {
    let t = reg(target);
    let below = rot_z(game, j) < t;
    let delta = fp::fmuls(
        reg(0.017_453_292),
        fp::fmuls(k, fp::fsubs(t, rot_z(game, j))),
    );
    add_rot_z(game, j, delta);
    let z = rot_z(game, j);
    if (below && z >= t) || (!below && z <= t) {
        set_rot_z(game, j, target);
    }
}

/// A flying platform's half width by the joint it moves, at the stage's scale (the inline
/// `grBigBlue_LaneSpeed`).
fn platform_size(game: &Game, idx: usize) -> f64 {
    fp::fmuls(reg(PLATFORM_SIZE[idx]), scale(game))
}

/// `10 + Stage_GetBlastZoneRightOffset()`, where platforms come and go.
fn off_side(game: &Game) -> f64 {
    fp::fadds(10.0, reg(super::stage::blast_right(game)))
}

/// The flying platforms' process (map 32), `grBigBlue_801E6C60`, for each platform by its
/// state: waiting (0) until the Falcon Flyer has passed (event 0), then counting down (1);
/// placed (2), once the count runs out (and while at most one other is coming or in view),
/// off the side of the road it can come from (right or left at random where the road's
/// heights are close, else from the lower side; from the left while the relay platform is
/// due), above the cars and the other platforms, its item drawn, the platforms' counts
/// for the relay platform and the Falcon Flyer's return advanced; flying (3): tilted with the
/// road below, dipped where fighters landed on it (rising back) and pushed by them, kept at
/// its height over the road, the cars and the other platforms, until it leaves the far side
/// and counts down again.
#[doc(alias = "grBigBlue_801E6C60")]
pub fn proc_platforms(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for i in 0..3 {
        let (idx, x1) = {
            let v = r.game.grounds[g.0].u.bigblue_manager();
            (v.data[i].index as i8, v.data[i].x1 as i8)
        };
        let j = r.game.grounds[g.0].u.bigblue_manager().jobjs[idx as usize];
        match x1 {
            0 => {
                if event(r.game, 0) != 0 {
                    let pm = param(r.game);
                    let add = randi(r.game, pm.i(r.data, p::PLAT_WAIT_ADD));
                    let wait = pm.i(r.data, p::PLAT_WAIT).wrapping_add(add);
                    let pos = translation(r.game, j);
                    let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
                    d.x4 = wait;
                    d.x38 = pos;
                    d.x44 = Vec3::ZERO;
                    d.x18 = Vec3::ZERO;
                    d.x1 = 1;
                }
            }
            1 | 2 => {
                if x1 == 1 && event(r.game, 0) == 0 {
                    r.game.grounds[g.0].u.bigblue_manager().data[i].x1 = 0;
                    continue;
                }
                if r.game.grounds[g.0].u.bigblue_manager().data[i].x4 <= 0 {
                    place_platform(r, g, i, j);
                } else {
                    let v = r.game.grounds[g.0].u.bigblue_manager();
                    let (jobjs, data) = (v.jobjs, v.data.clone());
                    let mut active = 0;
                    for k in 0..3 {
                        if j == jobjs[k] {
                            continue;
                        }
                        match data[k].x1 as i8 {
                            3 => {
                                let x = reg(data[k].x38.x);
                                if (data[k].x2 == 1 && x < reg(super::stage::cam_right(r.game)))
                                    || (data[k].x2 == -1 && x > reg(super::stage::cam_left(r.game)))
                                {
                                    active += 1;
                                }
                            }
                            1 if data[k].x4 == 0 => active += 1,
                            _ => {}
                        }
                    }
                    let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
                    if active <= 1 {
                        d.x4 = 0;
                    } else {
                        d.x4 = d.x4.wrapping_sub(1);
                    }
                }
            }
            3 => fly_platform(r, g, i, idx as usize, j),
            _ => {}
        }
    }
    update_map_coll(r.game, r.data, gobj);
}

/// A platform's placing (state 2 with its count run out) in [`proc_platforms`].
fn place_platform(r: &mut Running, g: GroundId, i: usize, j: Option<JObjId>) {
    let mut pos = Vec3::ZERO;
    let mut neg = Vec3::ZERO;
    pos.x = mem(off_side(r.game));
    neg.x = mem(fp::fneg(off_side(r.game)));
    let right_y = reg(road_height(r.game, pos, 500.0).0);
    let left_y = reg(road_height(r.game, neg, 500.0).0);
    let none = reg(NONE_Y);
    if right_y == none && left_y == none {
        return;
    }
    let pm = param(r.game);
    let lo = pm.f(r.data, p::PLAT_HEIGHT);
    r.game.grounds[g.0].u.bigblue_manager().data[i].x8 = mem(lo);
    let hr = abs(fp::fsubs(pm.f(r.data, p::PLAT_HEIGHT_MAX), lo));
    let hr = fp::fctiwz(hr);
    let rand = randi(r.game, hr);
    let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
    d.x8 = mem(fp::fadds(reg(d.x8), to_f(rand)));
    pos.y = mem(fp::fadds(right_y, reg(d.x8)));
    neg.y = mem(fp::fadds(left_y, reg(d.x8)));
    let dir: i8 = if left_y == none {
        -1
    } else if right_y == none {
        1
    } else if abs(fp::fsubs(right_y, left_y)) < 80.0 {
        if r.game.objs.rng.below(2) != 0 { 1 } else { -1 }
    } else if right_y < left_y {
        -1
    } else {
        1
    };
    r.game.grounds[g.0].u.bigblue_manager().data[i].x2 = dir;
    if event(r.game, 1) != 0 {
        r.game.grounds[g.0].u.bigblue_manager().data[i].x2 = 1;
        pos.x = mem(fp::fneg(off_side(r.game)));
    } else if r.game.grounds[g.0].u.bigblue_manager().data[i].x2 == 1 {
        pos = neg;
    }
    let mut retries = 0;
    let mut found;
    loop {
        let range = fp::fmuls(2.0, platform_size(r.game, 0));
        found = near_platform(r.game, j, pos, false, range, 25.0);
        if !found {
            let range = fp::fmuls(2.0, platform_size(r.game, 0));
            found = near_relay(r.game, pos, false, range, 25.0);
        }
        if !found && reg(pos.y) <= reg(car_height_in_view(r.game)) {
            found = true;
        }
        if found {
            retries += 1;
            pos.y = mem(fp::fadds(reg(pos.y), 25.0));
            if retries < 3 {
                continue;
            }
        }
        break;
    }
    if found {
        pos.y = mem(fp::fadds(10.0, reg(super::stage::cam_top(r.game))));
    }
    if reg(pos.y) == none {
        pos.y = mem(fp::fadds(10.0, reg(super::stage::cam_top(r.game))));
    }
    set_translate(r.game, j, pos);
    let speed = pm.f(r.data, p::PLAT_SPEED);
    {
        let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
        d.xC.z = pos.y;
        d.x38 = pos;
        d.x44.x = mem(fp::fmuls(speed, to_f(i32::from(d.x2))));
        d.x44.z = 0.0;
        d.x44.y = 0.0;
        d.x18 = Vec3::ZERO;
        d.x34 = 0;
        d.x2C = 0;
        d.x1 = 3;
    }
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.grounds[g.0].u.bigblue_manager().data[i].x50 = None;
    let chance = pm.i(r.data, p::PLAT_ITEM);
    if randi(r.game, chance) == 0 {
        platform_item(r, i);
    }
    let v = r.game.grounds[g.0].u.bigblue_manager();
    v.flags = v.flags.wrapping_add(1);
    if event(r.game, 2) == 0 {
        let cnt = r.game.grounds[g.0].u.bigblue_manager().flags as i32;
        if cnt >= pm.i(r.data, p::RELAY_COUNT) {
            let max = pm.i(r.data, p::RELAY_COUNT_MAX);
            if cnt <= max && (cnt == max || r.game.objs.rng.below(2) != 0) {
                post_event(r.game, 2, 1, j);
            }
        }
    }
    if event(r.game, 1) == 0 {
        let cnt = r.game.grounds[g.0].u.bigblue_manager().flags as i32;
        let dir = r.game.grounds[g.0].u.bigblue_manager().data[i].x2;
        if cnt >= pm.i(r.data, p::FLYER_COUNT)
            && dir == 1
            && (cnt >= pm.i(r.data, p::FLYER_COUNT_MAX) || r.game.objs.rng.below(2) != 0)
        {
            post_event(r.game, 0, 0, None);
            post_event(r.game, 1, 1, j);
            r.game.grounds[g.0].u.bigblue_manager().flags = 0;
        }
    }
}

/// A flying platform's flight (state 3) in [`proc_platforms`].
fn fly_platform(r: &mut Running, g: GroundId, i: usize, idx: usize, j: Option<JObjId>) {
    let mut pos = translation(r.game, j);
    let (surface, normal) = road_height(r.game, pos, 500.0);
    let normal = if reg(surface) == reg(NONE_Y) {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        normal.expect("a hit's normal")
    };
    let euler = Vec3::new(
        0.0,
        0.0,
        crate::math::atan2(mem(fp::fneg(reg(normal.x))), normal.y),
    );
    let x2 = r.game.grounds[g.0].u.bigblue_manager().data[i].x2;
    let speed = fp::fmuls(to_f(i32::from(x2)), platform_size(r.game, idx));
    let mut fwd = Vec3::new(mem(speed), 0.0, 0.0);
    fwd.rotate_euler(euler);
    let fwd = fwd + pos;
    let speed = fp::fmuls(to_f(-i32::from(x2)), platform_size(r.game, idx));
    let mut back = Vec3::new(mem(speed), 0.0, 0.0);
    back.rotate_euler(euler);
    let _ = back + pos;
    let pm = param(r.game);
    let turn = pm.f(r.data, p::PLAT_TURN);
    let d = r.game.grounds[g.0].u.bigblue_manager().data[i].clone();
    if d.x2C == 0 || d.x34 == 3 {
        if d.x30 == 0 && d.x2C != 0 {
            let dip = pm.i(r.data, p::PLAT_DIP);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.xC.y = pos.y;
            d.x4 = dip;
            d.x34 = 1;
        }
        r.game.grounds[g.0].u.bigblue_manager().data[i].xC.x = euler.z;
        ease_turn(r.game, j, euler.z, turn);
    } else {
        if d.x30 == 0 {
            let dip = pm.i(r.data, p::PLAT_DIP);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.xC.y = pos.y;
            d.x4 = dip;
            d.x34 = 1;
        }
        let d = r.game.grounds[g.0].u.bigblue_manager().data[i].clone();
        let dir: i32 = if reg(d.x24) < reg(d.x28) { 1 } else { -1 };
        if reg(d.x18.y) != to_f(dir) {
            let dip = pm.i(r.data, p::PLAT_DIP);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.x4 = dip;
            d.x34 = 1;
        }
        r.game.grounds[g.0].u.bigblue_manager().data[i].x18.y = mem(to_f(dir));
        let d = r.game.grounds[g.0].u.bigblue_manager().data[i].clone();
        if d.x34 == 2 {
            pos.y = mem(fp::fadds(reg(pos.y), reg(d.x44.y)));
            r.game.grounds[g.0].u.bigblue_manager().data[i].xC.x = euler.z;
            ease_turn(r.game, j, euler.z, turn);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            if reg(pos.y) >= reg(d.xC.y) {
                d.x44.y = 0.0;
                d.x18.z = 0.0;
                d.x18.y = 0.0;
                d.x34 = 3;
            }
        } else if d.x4 <= 0 {
            let rise = mem(pm.f(r.data, p::PLAT_RISE));
            let wait = pm.i(r.data, p::PLAT_DIP_WAIT);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.x44.y = rise;
            d.x4 = wait;
            d.x34 = 2;
        } else {
            let push = pm.f(r.data, p::PLAT_PUSH);
            let push_max = pm.f(r.data, p::PLAT_PUSH_MAX);
            let sink = pm.f(r.data, p::PLAT_SINK);
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            if reg(d.x24) < reg(d.x28) {
                d.x18.z = mem(fp::fmuls(push, fp::fsubs(reg(d.x28), reg(d.x24))));
                if reg(d.x18.z) >= push_max {
                    d.x18.z = mem(push_max);
                }
                pos.y = mem(fp::fsubs(reg(pos.y), sink));
            } else {
                d.x18.z = mem(fp::fmuls(fp::fneg(push), fp::fsubs(reg(d.x24), reg(d.x28))));
                if reg(d.x18.z) <= fp::fneg(push_max) {
                    d.x18.z = mem(fp::fneg(push_max));
                }
                pos.y = mem(fp::fadds(reg(pos.y), sink));
            }
            let delta = fp::fmuls(reg(0.017_453_292), reg(d.x18.z));
            add_rot_z(r.game, j, delta);
            let deg = fp::fmuls(reg(57.295_78), rot_z(r.game, j));
            if reg(pos.y) < fp::fadds(reg(surface), pm.f(r.data, p::PLAT_HEIGHT)) {
                if deg > 30.0 {
                    set_rot_z(r.game, j, std::f32::consts::FRAC_PI_6);
                }
                if deg < -30.0 {
                    set_rot_z(r.game, j, -std::f32::consts::FRAC_PI_6);
                }
            }
            if deg > 65.0 {
                set_rot_z(r.game, j, 1.134_464);
            }
            if deg < -65.0 {
                set_rot_z(r.game, j, -1.134_464);
            }
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.x4 = d.x4.wrapping_sub(1);
        }
    }
    let cam_top = super::stage::cam_top(r.game);
    let cam_bot = super::stage::cam_bottom(r.game);
    let s3 = platform_size(r.game, idx);
    let left_x = mem(fp::fsubs(reg(pos.x), fp::fadds(20.0, s3)));
    let right_x = mem(fp::fadds(
        reg(pos.x),
        fp::fadds(20.0, platform_size(r.game, idx)),
    ));
    let target_y = reg(car_height(r.game, cam_top, cam_bot, left_x, right_x));
    let probe_y = reg(road_height(r.game, pos, 500.0).0);
    let half_x = fp::fadds(10.0, platform_size(r.game, idx));
    let (coll, coll_y) = obstacle(r.game, j, pos, half_x, 16.5);
    let x8 = reg(r.game.grounds[g.0].u.bigblue_manager().data[i].x8);
    if coll == 0 || (coll == 1 && reg(pos.y) < reg(coll_y)) {
        let z = if target_y <= probe_y {
            if probe_y == reg(NONE_Y) {
                fwd.y
            } else {
                mem(fp::fadds(probe_y, x8))
            }
        } else {
            mem(fp::fadds(target_y, x8))
        };
        r.game.grounds[g.0].u.bigblue_manager().data[i].xC.z = z;
    } else if coll == 1 {
        let y = reg(pos.y);
        r.game.grounds[g.0].u.bigblue_manager().data[i].xC.z =
            mem(fp::fadds(y, fp::fsubs(y, reg(coll_y))));
    }
    let z = reg(r.game.grounds[g.0].u.bigblue_manager().data[i].xC.z);
    let y = reg(pos.y);
    let vy = if abs(fp::fsubs(y, z)) < 0.5 {
        0.0
    } else if y < z {
        let v = fp::fdivs(fp::fsubs(z, y), pm.f(r.data, p::PLAT_UP));
        let max = pm.f(r.data, p::PLAT_UP_MAX);
        if v > max { max } else { v }
    } else {
        let v = fp::fdivs(fp::fsubs(z, y), pm.f(r.data, p::PLAT_DOWN));
        let min = fp::fneg(pm.f(r.data, p::PLAT_DOWN_MAX));
        if v < min { min } else { v }
    };
    pos.y = mem(fp::fadds(y, vy));
    let vx = reg(r.game.grounds[g.0].u.bigblue_manager().data[i].x44.x);
    add_tx(r.game, j, vx);
    set_ty(r.game, j, pos.y);
    {
        let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
        d.x38 = pos;
        d.x30 = d.x2C;
        d.x2C = 0;
        d.x28 = 0.0;
        d.x24 = 0.0;
    }
    let tx = reg(translation(r.game, j).x);
    let edge = fp::fsubs(off_side(r.game), 50.0);
    let vx = reg(r.game.grounds[g.0].u.bigblue_manager().data[i].x44.x);
    if (vx < 0.0 && tx < fp::fneg(edge)) || (vx > 0.0 && tx > edge) {
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        set_rot_z(r.game, j, 0.0);
        {
            let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
            d.x44 = Vec3::ZERO;
            d.x18 = Vec3::ZERO;
            d.xC.x = 0.0;
            d.x34 = 0;
            d.x2C = 0;
        }
        let add = randi(r.game, pm.i(r.data, p::PLAT_WAIT_ADD));
        let d = &mut r.game.grounds[g.0].u.bigblue_manager().data[i];
        d.x4 = pm.i(r.data, p::PLAT_WAIT).wrapping_add(add) / 2;
        d.x1 = 1;
    }
}

/// The flying platforms' collision joints' callback, `fn_801E8560`: a fighter landing or
/// standing on a platform (collision kinds 1 and 3) more than 2 from its joint pushes it on
/// that side, by its distance and its weight (`x50`, at most 1000), and counts.
#[doc(alias = "fn_801E8560")]
pub fn platform_touched(
    objs: &crate::hsd::Objects,
    gp: &mut super::super::structs::Ground,
    joint_id: i32,
    coll: &super::super::structs::CollData,
    x50: i32,
) {
    let k = coll.x34_flags.b1234;
    if k != 1 && k != 3 {
        return;
    }
    let joint_index = match joint_id {
        0..=2 => joint_id,
        _ => panic!("grbigblue.c: a platform joint callback on joint {joint_id}"),
    };
    let v = gp.u.bigblue_manager();
    let j = v.jobjs[joint_index as usize];
    let mut x50 = x50;
    if fp::frsp(f64::from(x50)) > 1000.0 {
        x50 = 1000;
    }
    let pos = objs.jobjs[j.expect("jobj")].translate;
    let dx = fp::fsubs(reg(pos.x), reg(coll.cur_pos.x));
    let dy = fp::fsubs(reg(pos.y), reg(coll.cur_pos.y));
    let d2 = fp::fmadds(dy, dy, fp::fmuls(dx, dx));
    let dist = reg(crate::math::sqrt(mem(d2)));
    if dist > 2.0 {
        let mut active = 0;
        for i in 0..3 {
            if v.data[i].index as i8 as i32 == joint_index {
                break;
            }
            active = i + 1;
        }
        let w = fp::fdivs(to_f(x50), 1000.0);
        let d = &mut v.data[active];
        if reg(pos.x) < reg(coll.cur_pos.x) {
            d.x24 = mem(fp::fmadds(dist, w, reg(d.x24)));
        } else {
            d.x28 = mem(fp::fmadds(dist, w, reg(d.x28)));
        }
        d.x2C = d.x2C.wrapping_add(1);
    }
}

/// Where the Falcon Flyer's setup looks for the road: its `pos` (`r1 + 0x3C` in its frame),
/// which it never sets, so the words the stage's setup left there (Melee's bug,
/// docs/melee-bugs.md). The last call to write them is the setup's hiding of the flying
/// platforms (`HSD_JObjSetFlagsAll` on map 32's tree, just before map 35 is made), which
/// walks two levels a call: each call it makes on a joint four levels below the root saves
/// there its caller's r29 and r30, the joint walked and the child called on, so after the
/// walk the last such joint's parent (x) and the joint (y); `Ground_GetStageGObj`'s `stmw`
/// then puts 0 over the third word (z). The allocator model gives the two joints' addresses
/// (docs/engine.md, "Uninitialized values"). Without it they are still words of the arena
/// (`0x804E_EC00` to `0x817F_8AC0`), floats within 2^-124 of zero, which put the search's
/// segment from exactly 500 above to 500 below on the level road piece that spans the origin
/// at the match's start, 0.00005 high, whichever they are; so the stand-in is the console's own
/// words under Slippi's playback.
fn flyer_stale_pos(game: &Game) -> Vec3 {
    let root = super::ground::map_gobj(game, 0x20).and_then(|g| jobj_of(game, g));
    let jobjs = &game.objs.jobjs;
    let own_children = |j: JObjId| {
        let first = if jobjs[j].flags & jobj::INSTANCE != 0 {
            None
        } else {
            jobjs[j].child
        };
        std::iter::successors(first, |&c| jobjs[c].next)
    };
    let mut last = None;
    for d1 in root.into_iter().flat_map(own_children) {
        for d2 in own_children(d1) {
            for d3 in own_children(d2) {
                for d4 in own_children(d3) {
                    last = Some((d3, d4));
                }
            }
        }
    }
    let (walked, called) = last.expect("grbigblue.c: map 32's joints four levels down");
    let (x, y) = match (jobjs.pool.addr(walked.0), jobjs.pool.addr(called.0)) {
        (0, _) | (_, 0) => (0x80EF_6860, 0x80EF_6900),
        words => words,
    };
    Vec3::new(f32::from_bits(x), f32::from_bits(y), 0.0)
}

/// The Falcon Flyer's setup (map 35), `grBigBlue_801E8D64`: its collision, at the stage's
/// scale, the Falcon Flyer coming (event 1), over the road (searched from `pos`, which it
/// never sets: [`flyer_stale_pos`]) at its height at x 0, waiting over the stage (state 2)
/// with its countdown, its animation, and the players' start points moved by its place.
#[doc(alias = "grBigBlue_801E8D64")]
pub fn init_flyer(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    r.game.grounds[g.0].x10_flags.b5 = true;
    let s = super::ground::scale(r.game);
    set_scale(r.game, j, Vec3::new(s, s, s));
    let mg = map_ground(r.game, 0x20);
    r.game.grounds[mg.0].u.bigblue_manager().event[1] = 1;
    let probe = flyer_stale_pos(r.game);
    let mut y = reg(road_height(r.game, probe, 500.0).0);
    if reg(NONE_Y) == y {
        y = 0.0;
    }
    set_tx(r.game, j, 0.0);
    let pm = param(r.game);
    y = fp::fadds(y, pm.f(r.data, p::FLYER_HEIGHT));
    set_ty(r.game, j, mem(y));
    set_tz(r.game, j, 0.0);
    let wait = fp::fctiwz(pm.f(r.data, p::FLYER_WAIT));
    {
        let v = r.game.grounds[g.0].u.bigblue_flyer();
        v.xD8 = 0.0;
        v.xC8 = wait;
        v.xC4 = 2;
    }
    anime::start_map(r.game, r.data, gobj, m, 0);
    let pos = translation(r.game, j);
    let inv = fp::fdivs(1.0, scale(r.game));
    let pos = Vec3::new(
        mem(fp::fmuls(reg(pos.x), inv)),
        mem(fp::fmuls(reg(pos.y), inv)),
        mem(fp::fmuls(reg(pos.z), inv)),
    );
    for k in 0..4 {
        let pj = super::ground::point_joint(r.game, k);
        let t = translation(r.game, pj) + pos;
        set_translate(r.game, pj, t);
    }
}

/// The flying platforms' states other than waiting (the inline `grBigBlue_CountCars`).
fn platforms_out(game: &mut Game) -> i32 {
    let g = map_ground(game, 0x20);
    let v = game.grounds[g.0].u.bigblue_manager();
    v.data.iter().filter(|d| d.x1 as i8 != 0).count() as i32
}

/// The Falcon Flyer's process (map 35), `grBigBlue_801E93D8`, by its state: waiting off the
/// left (0) until it's about to come (event 1) with at most one platform out, then placed off
/// the left over the road, clear of the platforms and the cars (else high), coming (1); coming
/// (1), stopping once past x 0 or near a platform; waiting over the stage (2) for its countdown,
/// then leaving (3) with a platform set placing; leaving (3) until past the right side, when
/// it hides and lets the platforms count again (event 0). While out it tilts with the road
/// and keeps its height over the road and the cars.
#[doc(alias = "grBigBlue_801E93D8")]
pub fn proc_flyer(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let mut pos = translation(r.game, j);
    let (h, normal) = road_height(r.game, pos, 500.0);
    let normal = if reg(h) == reg(NONE_Y) {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        normal.expect("a hit's normal")
    };
    let euler = Vec3::new(
        0.0,
        0.0,
        crate::math::atan2(mem(fp::fneg(reg(normal.x))), normal.y),
    );
    let mut fwd = Vec3::new(50.0, 0.0, 0.0);
    fwd.rotate_euler(euler);
    let fwd = fwd + pos;
    let mut back = Vec3::new(-50.0, 0.0, 0.0);
    back.rotate_euler(euler);
    let _ = back + pos;
    let pm = param(r.game);
    let state = r.game.grounds[g.0].u.bigblue_flyer().xC4 as i8;
    match state {
        0 => {
            let mg = map_ground(r.game, 0x20);
            let coming = r.game.grounds[mg.0].u.bigblue_manager().event[1] != 0;
            if coming && platforms_out(r.game) <= 1 {
                pos = Vec3::ZERO;
                pos.x = mem(fp::fsubs(reg(super::stage::blast_left(r.game)), 50.0));
                let h = reg(road_height(r.game, pos, 500.0).0);
                if h != reg(NONE_Y) {
                    pos.y = mem(fp::fadds(h, pm.f(r.data, p::FLYER_HEIGHT)));
                    let speed = fp::fmuls(140.0, scale(r.game));
                    let range = fp::fmuls(2.0, fp::fmuls(60.0, scale(r.game)));
                    let mut hit = near_platform(r.game, j, pos, true, range, speed);
                    if !hit {
                        let range = fp::fmuls(2.0, fp::fmuls(60.0, scale(r.game)));
                        hit = near_relay(r.game, pos, true, range, 25.0);
                    }
                    if !hit && reg(pos.y) <= reg(car_height_in_view(r.game)) {
                        hit = true;
                    }
                    if hit {
                        pos.y = mem(fp::fadds(30.0, reg(super::stage::cam_top(r.game))));
                    }
                    assert!(reg(pos.y) != reg(NONE_Y), "*** Not Set Position!(FFlyer)");
                    set_translate(r.game, j, pos);
                    let speed = mem(pm.f(r.data, p::FLYER_SPEED));
                    let v = r.game.grounds[g.0].u.bigblue_flyer();
                    v.xD0 = pos.y;
                    v.xD8 = speed;
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                    r.game.grounds[g.0].u.bigblue_flyer().xC4 = 1;
                }
            }
        }
        1..=3 => {
            match state {
                1 => {
                    if reg(pos.x) > 0.0 {
                        let wait = fp::fctiwz(pm.f(r.data, p::FLYER_WAIT));
                        let v = r.game.grounds[g.0].u.bigblue_flyer();
                        v.xD8 = 0.0;
                        v.xC8 = wait;
                        v.xC4 = 2;
                    } else {
                        let speed = fp::fmuls(140.0, scale(r.game));
                        let range = fp::fmadds(60.0, scale(r.game), 30.0);
                        let near = near_platform(r.game, j, pos, true, range, speed) || {
                            let speed = fp::fmuls(140.0, scale(r.game));
                            let range = fp::fmadds(60.0, scale(r.game), 30.0);
                            near_relay(r.game, pos, true, range, speed)
                        };
                        let v = if near {
                            0.0
                        } else {
                            mem(pm.f(r.data, p::FLYER_SPEED))
                        };
                        r.game.grounds[g.0].u.bigblue_flyer().xD8 = v;
                    }
                }
                2 => {
                    let t = r.game.grounds[g.0].u.bigblue_flyer().xC8;
                    if t <= 0 {
                        let speed = mem(pm.f(r.data, p::FLYER_SPEED));
                        let v = r.game.grounds[g.0].u.bigblue_flyer();
                        v.xD8 = speed;
                        v.xC4 = 3;
                        let mg = map_ground(r.game, 0x20);
                        let m = r.game.grounds[mg.0].u.bigblue_manager();
                        if let Some(d) = m.data.iter_mut().find(|d| d.x1 as i8 == 0) {
                            d.x1 = 2;
                            d.x4 = 0;
                        }
                    } else {
                        r.game.grounds[g.0].u.bigblue_flyer().xC8 = t.wrapping_sub(1);
                    }
                }
                _ => {
                    let edge = fp::fadds(50.0, reg(super::stage::blast_right(r.game)));
                    if reg(pos.x) > edge {
                        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                        r.game.grounds[g.0].u.bigblue_flyer().xD8 = 0.0;
                        let mg = map_ground(r.game, 0x20);
                        let m = r.game.grounds[mg.0].u.bigblue_manager();
                        m.event[1] = 0;
                        if j.is_some() {
                            m.event_extra = j;
                        }
                        m.event[0] = 1;
                        r.game.grounds[g.0].u.bigblue_flyer().xCC = 0.0;
                        set_rot_z(r.game, j, 0.0);
                        r.game.grounds[g.0].u.bigblue_flyer().xC4 = 0;
                    }
                }
            }
            r.game.grounds[g.0].u.bigblue_flyer().xCC = euler.z;
            ease_turn(r.game, j, euler.z, pm.f(r.data, p::FLYER_TURN));
            let (right, left) = (
                super::stage::cam_right(r.game),
                super::stage::cam_left(r.game),
            );
            let (bottom, top) = (
                super::stage::cam_bottom(r.game),
                super::stage::cam_top(r.game),
            );
            let bound = reg(car_height(r.game, top, bottom, left, right));
            let check = reg(road_height(r.game, pos, 500.0).0);
            let height = pm.f(r.data, p::FLYER_HEIGHT);
            let target = if bound <= check {
                if check == reg(NONE_Y) {
                    fwd.y
                } else {
                    mem(fp::fadds(check, height))
                }
            } else {
                mem(fp::fadds(bound, height))
            };
            r.game.grounds[g.0].u.bigblue_flyer().xD0 = target;
            let (y, t) = (reg(pos.y), reg(target));
            let vy = if abs(fp::fsubs(y, t)) < 0.5 {
                0.0
            } else if y < t {
                let v = fp::fdivs(fp::fsubs(t, y), pm.f(r.data, p::FLYER_UP));
                let max = pm.f(r.data, p::FLYER_UP_MAX);
                if v > max { max } else { v }
            } else {
                let v = fp::fdivs(fp::fsubs(t, y), pm.f(r.data, p::FLYER_DOWN));
                let min = fp::fneg(pm.f(r.data, p::FLYER_DOWN_MAX));
                if v < min { min } else { v }
            };
            pos.y = mem(fp::fadds(y, vy));
            let vx = reg(r.game.grounds[g.0].u.bigblue_flyer().xD8);
            add_tx(r.game, j, vx);
            set_ty(r.game, j, pos.y);
        }
        _ => {}
    }
    update_map_coll(r.game, r.data, gobj);
}

/// The relay platform's setup (map 36), `grBigBlue_801E9F3C`: its collision, at the stage's
/// scale, waiting (state 0), the flying platforms' relay event cleared, its animation.
#[doc(alias = "grBigBlue_801E9F3C")]
pub fn init_relay(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    r.game.grounds[g.0].x10_flags.b5 = true;
    let s = super::ground::scale(r.game);
    set_scale(r.game, j, Vec3::new(s, s, s));
    r.game.grounds[g.0].u.bigblue_relay().set_state(0);
    let mg = map_ground(r.game, 0x20);
    r.game.grounds[mg.0].u.bigblue_manager().event[2] = 0;
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A speed drawn in steps of 0.1 between two of the parameters (at least the first), as the
/// relay platform draws them.
fn relay_speed(game: &mut Game, data: &Archives, lo: u32, hi: u32) -> f64 {
    let pm = param(game);
    let range = fp::fdivs(fp::fsubs(pm.f(data, hi), pm.f(data, lo)), reg(0.1));
    let n = randi(game, fp::fctiwz(range));
    let speed = fp::fmuls(to_f(n), reg(0.1));
    if speed == 0.0 { pm.f(data, lo) } else { speed }
}

/// The relay platform's process (map 36), `grBigBlue_801EA05C`, by its state: waiting (0)
/// until the flying platforms call it (event 2); counting down (1), then placed off the side
/// it can come from (as the flying platforms are) when nothing is in the way, flying (2);
/// flying (2), its height kept over the road, the cars and the platforms, crossing at speeds
/// drawn now and then, until it leaves the far side and hides.
#[doc(alias = "grBigBlue_801EA05C")]
pub fn proc_relay(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let mut pos = translation(r.game, j);
    let (h, normal) = road_height(r.game, pos, 500.0);
    let normal = if reg(h) == reg(NONE_Y) {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        normal.expect("a hit's normal")
    };
    let euler = Vec3::new(
        0.0,
        0.0,
        crate::math::atan2(mem(fp::fneg(reg(normal.x))), normal.y),
    );
    let half = |game: &Game| fp::fmuls(fp::fmuls(68.0, scale(game)), 0.5);
    let mut top = Vec3::new(mem(half(r.game)), 0.0, 0.0);
    top.rotate_euler(euler);
    let top = top + pos;
    let mut bot = Vec3::new(mem(fp::fneg(half(r.game))), 0.0, 0.0);
    bot.rotate_euler(euler);
    let _ = bot + pos;
    let state = r.game.grounds[g.0].u.bigblue_relay().state();
    match state {
        0 => {
            let mg = map_ground(r.game, 0x20);
            if r.game.grounds[mg.0].u.bigblue_manager().event[2] != 0 {
                let v = r.game.grounds[g.0].u.bigblue_relay();
                v.xC8_timer = 0;
                v.xD0_timer = 0;
                v.xCC_timer = 0;
                v.set_state(1);
            }
        }
        1 => {
            let t = r.game.grounds[g.0].u.bigblue_relay().xC8_timer;
            if t > 0 {
                r.game.grounds[g.0].u.bigblue_relay().xC8_timer = t.wrapping_sub(1);
            } else {
                place_relay(r, g, j);
            }
        }
        2 => fly_relay(r, g, j, &mut pos, top),
        _ => {}
    }
    update_map_coll(r.game, r.data, gobj);
}

/// The relay platform's placing (state 1, its count run out) in [`proc_relay`].
fn place_relay(r: &mut Running, g: GroundId, j: Option<JObjId>) {
    let pm = param(r.game);
    let mut pos = Vec3::ZERO;
    let mut bot = Vec3::ZERO;
    pos.x = super::stage::blast_right(r.game);
    bot.x = mem(fp::fneg(reg(super::stage::blast_right(r.game))));
    let right_y = reg(road_height(r.game, pos, 500.0).0);
    let left_y = reg(road_height(r.game, bot, 500.0).0);
    let lo = pm.f(r.data, p::RELAY_HEIGHT);
    r.game.grounds[g.0].u.bigblue_relay().height_offset = mem(lo);
    let hr = abs(fp::fsubs(pm.f(r.data, p::RELAY_HEIGHT_MAX), lo));
    let n = randi(r.game, fp::fctiwz(hr));
    let v = r.game.grounds[g.0].u.bigblue_relay();
    v.height_offset = mem(fp::fadds(reg(v.height_offset), to_f(n)));
    let off = reg(v.height_offset);
    pos.y = mem(fp::fadds(right_y, off));
    bot.y = mem(fp::fadds(left_y, off));
    let none = reg(NONE_Y);
    if right_y == none && left_y == none {
        return;
    }
    let dir: i8 = if left_y == none {
        -1
    } else if right_y == none {
        1
    } else if abs(fp::fsubs(right_y, left_y)) < 80.0 {
        if r.game.objs.rng.below(2) != 0 { 1 } else { -1 }
    } else if right_y < left_y {
        -1
    } else {
        1
    };
    r.game.grounds[g.0].u.bigblue_relay().set_dir(dir);
    if r.game.grounds[g.0].u.bigblue_relay().dir() == 1 {
        pos = bot;
    }
    let ph = fp::fmuls(52.0, scale(r.game));
    let range = fp::fmuls(2.0, fp::fmuls(68.0, scale(r.game)));
    let mut hit = near_platform(r.game, j, pos, false, range, ph);
    if !hit && reg(pos.y) <= reg(car_height_in_view(r.game)) {
        hit = true;
    }
    if hit {
        return;
    }
    assert!(reg(pos.y) != none, "*** Not Set Position!(Tyukei)");
    set_translate(r.game, j, pos);
    let v = r.game.grounds[g.0].u.bigblue_relay();
    v.target_y = pos.y;
    v.xEC = 0.0;
    v.velocity.z = 0.0;
    v.velocity.y = 0.0;
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.grounds[g.0].u.bigblue_relay().set_state(2);
}

/// The relay platform's flight (state 2) in [`proc_relay`].
fn fly_relay(r: &mut Running, g: GroundId, j: Option<JObjId>, pos: &mut Vec3, top: Vec3) {
    let pm = param(r.game);
    let cam_top = super::stage::cam_top(r.game);
    let cam_bot = super::stage::cam_bottom(r.game);
    let reach = |game: &Game| fp::fmadds(fp::fmuls(68.0, scale(game)), 0.5, 20.0);
    let left_x = mem(fp::fsubs(reg(pos.x), reach(r.game)));
    let right_x = mem(fp::fadds(reg(pos.x), reach(r.game)));
    let bounds_y = reg(car_height(r.game, cam_top, cam_bot, left_x, right_x));
    let surface = reg(road_height(r.game, *pos, 500.0).0);
    let half_x = fp::fmadds(fp::fmuls(68.0, scale(r.game)), 0.5, 10.0);
    let half_y = fp::fmadds(fp::fmuls(52.0, scale(r.game)), 0.5, 4.0);
    let (ace, y_check) = obstacle(r.game, j, *pos, half_x, half_y);
    let off = reg(r.game.grounds[g.0].u.bigblue_relay().height_offset);
    if ace == 0 || (ace == 1 && reg(pos.y) < reg(y_check)) {
        let t = if bounds_y <= surface {
            if reg(NONE_Y) == surface {
                top.y
            } else {
                mem(fp::fadds(surface, off))
            }
        } else {
            mem(fp::fadds(bounds_y, off))
        };
        r.game.grounds[g.0].u.bigblue_relay().target_y = t;
    } else if ace == 1 {
        let y = reg(pos.y);
        r.game.grounds[g.0].u.bigblue_relay().target_y =
            mem(fp::fadds(y, fp::fsubs(y, reg(y_check))));
    }
    let t = reg(r.game.grounds[g.0].u.bigblue_relay().target_y);
    let y = reg(pos.y);
    let vy = if abs(fp::fsubs(y, t)) < 0.5 {
        0.0
    } else if y < t {
        let v = fp::fdivs(fp::fsubs(t, y), pm.f(r.data, p::RELAY_UP));
        let max = pm.f(r.data, p::RELAY_UP_MAX);
        if v > max { max } else { v }
    } else {
        let v = fp::fdivs(fp::fsubs(t, y), pm.f(r.data, p::RELAY_DOWN));
        let min = fp::fneg(pm.f(r.data, p::RELAY_DOWN_MAX));
        if v < min { min } else { v }
    };
    pos.y = mem(fp::fadds(y, vy));
    set_ty(r.game, j, pos.y);
    let wait_range = |r: &mut Running| {
        let pm = param(r.game);
        let d = pm
            .i(r.data, p::RELAY_WAIT_MAX)
            .wrapping_sub(pm.i(r.data, p::RELAY_WAIT_MIN));
        let d = if d < 0 { d.wrapping_neg() } else { d };
        randi(r.game, d).wrapping_add(pm.i(r.data, p::RELAY_WAIT_MIN))
    };
    let t = r.game.grounds[g.0].u.bigblue_relay().xCC_timer;
    if t <= 0 {
        let w = wait_range(r);
        r.game.grounds[g.0].u.bigblue_relay().xCC_timer = w;
        let speed = relay_speed(r.game, r.data, p::RELAY_SPEED_MIN, p::RELAY_SPEED_MAX);
        let v = r.game.grounds[g.0].u.bigblue_relay();
        v.velocity.y = mem(fp::fmuls(speed, to_f(i32::from(v.dir()))));
    } else {
        r.game.grounds[g.0].u.bigblue_relay().xCC_timer = t.wrapping_sub(1);
    }
    let t = r.game.grounds[g.0].u.bigblue_relay().xD0_timer;
    if t <= 0 {
        let w = wait_range(r);
        r.game.grounds[g.0].u.bigblue_relay().xD0_timer = w;
        let mut speed = relay_speed(r.game, r.data, p::RELAY_RISE_MIN, p::RELAY_RISE_MAX);
        if r.game.objs.rng.below(2) != 0 {
            speed = fp::fmuls(speed, -1.0);
        }
        r.game.grounds[g.0].u.bigblue_relay().velocity.z = mem(speed);
    } else {
        r.game.grounds[g.0].u.bigblue_relay().xD0_timer = t.wrapping_sub(1);
    }
    let v = r.game.grounds[g.0].u.bigblue_relay().velocity;
    add_tx(r.game, j, reg(v.y));
    add_ty(r.game, j, reg(v.z));
    let dir = r.game.grounds[g.0].u.bigblue_relay().dir();
    let tx = reg(translation(r.game, j).x);
    let edge = fp::fsubs(reg(super::stage::blast_right(r.game)), 50.0);
    if (dir == -1 && tx < fp::fneg(edge)) || (dir == 1 && tx > edge) {
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        let v = r.game.grounds[g.0].u.bigblue_relay();
        v.xEC = 0.0;
        v.velocity.z = 0.0;
        v.velocity.y = 0.0;
        let mg = map_ground(r.game, 0x20);
        r.game.grounds[mg.0].u.bigblue_manager().event[2] = 0;
        r.game.grounds[g.0].u.bigblue_relay().set_state(0);
    }
}

/// A road piece's joint, by its steps into the road's model.
fn piece_joint(game: &Game, gobj: GObjId, k: i32) -> Option<JObjId> {
    nth_joint(game, gobj, k)
}

/// Lays the road out, `grBigBlue_801EB004`: the road hidden at unit scale, each piece at the
/// stage's scale with its length from its start to its end measured, the road at rest (its
/// place, turn and flags), and piece 4 shown at the road's height with piece 0 after it.
#[doc(alias = "grBigBlue_801EB004")]
pub fn lay_road(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let root = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, root, jobj::HIDDEN);
    set_scale(r.game, root, Vec3::new(1.0, 1.0, 1.0));
    let s = super::ground::scale(r.game);
    for (i, e) in TRACK.iter().enumerate() {
        let j = piece_joint(r.game, gobj, e.jobj);
        let start = piece_joint(r.game, gobj, e.start);
        let end = piece_joint(r.game, gobj, e.end);
        assert!(
            j.is_some() && start.is_some() && end.is_some(),
            "grbigblue.c: a road piece's joints"
        );
        set_scale(r.game, j, Vec3::new(s, s, s));
        let a = crate::lb::joint::world_position(&mut r.game.objs, r.data, start, None);
        let b = crate::lb::joint::world_position(&mut r.game.objs, r.data, end, None);
        r.game.gr.bigblue.track_deltas[i] = b - a;
    }
    let s = scale(r.game);
    let zero = mem(fp::fmuls(0.0, s));
    {
        let v = r.game.grounds[g.0].u.bigblue_road();
        v.set_prev_lane(0xFFFF);
        v.set_cur_lane(4);
        v.set_next_lane(0);
        v.position.x = mem(fp::fmuls(-1000.0, s));
        v.position.y = mem(fp::fmuls(10.0, s));
        v.position.z = zero;
        v.previous_position = Vec3::new(zero, zero, zero);
        v.direction = 0;
        v.rotation = zero;
        v.set_loop_stage(0);
        v.set_falling(false);
        v.set_looping(false);
    }
    let cur = r.game.grounds[g.0].u.bigblue_road().cur_lane() as usize;
    let j = piece_joint(r.game, gobj, TRACK[cur].jobj);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let pm = param(r.game);
    let mut pos = Vec3::new(
        0.0,
        mem(fp::fmuls(pm.f(r.data, p::ROAD_Y), scale(r.game))),
        0.0,
    );
    set_translate(r.game, j, pos);
    let next = r.game.grounds[g.0].u.bigblue_road().next_lane() as usize;
    let j = piece_joint(r.game, gobj, TRACK[next].jobj);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let cur = r.game.grounds[g.0].u.bigblue_road().cur_lane() as usize;
    pos = pos + r.game.gr.bigblue.track_deltas[cur];
    set_translate(r.game, j, pos);
}

/// The road's next piece, `grBigBlue_801EB4AC`: the current piece hidden, the next made
/// current (its place taken as the road's, its own across and in depth cleared), the climb or
/// dive ended or started by the road's height, and a new next piece drawn at random (the
/// loop only above -700, none twice, only pieces 0, 1 and 4 while the road waits, only
/// climbs or dives with the road's, and none that would rise above the climb's limit),
/// shown after the current one, its collision settled.
#[doc(alias = "grBigBlue_801EB4AC")]
pub fn next_piece(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let cur = r.game.grounds[g.0].u.bigblue_road().cur_lane() as usize;
    let j = piece_joint(r.game, gobj, TRACK[cur].jobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    {
        let v = r.game.grounds[g.0].u.bigblue_road();
        let c = v.cur_lane();
        v.set_prev_lane(c);
        let n = v.next_lane();
        v.set_cur_lane(n);
    }
    let next = r.game.grounds[g.0].u.bigblue_road().next_lane() as usize;
    let j = piece_joint(r.game, gobj, TRACK[next].jobj);
    let sp = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    r.game.grounds[g.0].u.bigblue_road().position.x = sp.x;
    set_tx(r.game, j, 0.0);
    r.game.grounds[g.0].u.bigblue_road().position.z = sp.z;
    set_tz(r.game, j, 0.0);
    let pm = param(r.game);
    let (climb, dive) = (pm.f(r.data, p::CLIMB), pm.f(r.data, p::DIVE));
    let s = scale(r.game);
    let v = r.game.grounds[g.0].u.bigblue_road();
    let y = reg(v.position.y);
    match v.direction {
        1 => {
            if y < fp::fmuls(fp::fmuls(climb, s), 0.25) {
                v.direction = 0;
            }
        }
        -1 => {
            if y > fp::fmuls(0.25, fp::fneg(fp::fmuls(dive, s))) {
                v.direction = 0;
            }
        }
        _ => {
            if y < fp::fneg(fp::fmuls(dive, s)) {
                v.direction = -1;
            } else if y > fp::fmuls(climb, s) {
                v.direction = 1;
            }
        }
    }
    let mut count = 0;
    let lane = loop {
        assert!(count <= 1000, "grbigblue.c: 2414: 0");
        count += 1;
        let lane = r.game.objs.rng.below(12) as usize;
        let v = r.game.grounds[g.0].u.bigblue_road();
        if lane == 11 && reg(v.position.y) < -700.0 {
            continue;
        }
        if v.cur_lane() as usize == lane {
            continue;
        }
        if v.waiting() {
            if lane > 1 && lane != 4 {
                continue;
            }
        } else {
            let d = r.game.gr.bigblue.track_deltas[lane];
            let v = r.game.grounds[g.0].u.bigblue_road();
            if v.direction == 1 && reg(d.y) < 0.0 {
                continue;
            }
            if v.direction == -1 && reg(d.y) > 0.0 {
                continue;
            }
            let cur = r.game.gr.bigblue.track_deltas[v.cur_lane() as usize];
            let reach = fp::fadds(fp::fadds(reg(sp.y), reg(cur.y)), reg(d.y));
            let limit = fp::fmadds(climb, s, reg(v.position.y));
            if reach < limit {
                continue;
            }
        }
        break lane;
    };
    r.game.grounds[g.0]
        .u
        .bigblue_road()
        .set_next_lane(lane as u32);
    let mut sp = translation(r.game, j);
    let nj = piece_joint(r.game, gobj, TRACK[lane].jobj);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, nj, jobj::HIDDEN);
    let cur = r.game.grounds[g.0].u.bigblue_road().cur_lane() as usize;
    let d = r.game.gr.bigblue.track_deltas[cur];
    sp.x = mem(fp::fadds(reg(sp.x), reg(d.x)));
    sp.y = mem(fp::fadds(reg(sp.y), reg(d.y)));
    sp.z = mem(fp::fadds(reg(sp.z), reg(d.z)));
    set_translate(r.game, nj, sp);
    let geo = r.game.geometry_mut();
    if lane == 11 {
        for k in [41, 42, 43] {
            geo.enable_joint(k);
        }
        for k in 44..=49 {
            geo.disable_joint(k);
        }
        r.game.grounds[g.0].u.bigblue_road().set_loop_stage(0);
    }
    let settle: &[i32] = match lane {
        0 => &[35],
        1 => &[50],
        6 => &[52, 53, 54],
        7 => &[55, 56, 57],
        8 => &[58, 59, 60],
        9 => &[61, 62, 63],
        10 => &[51],
        5 => &[33],
        2 => &[38, 39, 40],
        3 => &[36, 37],
        4 => &[64, 65],
        _ => &[41, 42, 43, 44, 45, 46, 47, 48, 49],
    };
    let geo = r.game.geometry_mut();
    for &k in settle {
        geo.settle_vertices(k);
    }
    geo.link_all_joints();
}

/// The road's move, `grBigBlue_801EBAF8`: the next piece once the current one's end has
/// passed -300 (while the road is about level); the loop's turn (on the loop piece, once its
/// end has passed 0), its stages switching its collision joints; the road kept on the ground
/// below the road's height (within 20, or 120 while it falls away), turning with the slope
/// (the ground's own on the loop), else falling away and drifting down; the turn's step
/// (none upward, scaled past a small turn), the road turned about the road's height point,
/// wrapped to a half turn either way, moved left by its speed and back toward depth 0, and
/// maps 1 and 2 moved with it.
#[doc(alias = "grBigBlue_801EBAF8")]
pub fn move_road(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let pm = param(r.game);
    let center = Vec3::new(
        0.0,
        mem(fp::fmuls(pm.f(r.data, p::ROAD_Y), scale(r.game))),
        0.0,
    );
    let prev = r.game.grounds[g.0].u.bigblue_road().position;
    let entry_lane = r.game.grounds[g.0].u.bigblue_road().cur_lane() as usize;
    let end = piece_joint(r.game, gobj, TRACK[entry_lane].end);
    let bone = crate::lb::joint::world_position(&mut r.game.objs, r.data, end, None);
    let rot = reg(r.game.grounds[g.0].u.bigblue_road().rotation);
    if abs(rot) < reg(0.174_532_92) && reg(bone.x) < fp::fmuls(-300.0, scale(r.game)) {
        next_piece(r, gobj);
    }
    let v = r.game.grounds[g.0].u.bigblue_road();
    if abs(reg(v.rotation)) < reg(0.174_532_92) {
        if reg(bone.x) < 0.0 {
            if v.looping() {
                v.set_looping(false);
            }
        } else if !v.looping() && v.cur_lane() == 0xB {
            v.set_looping(true);
        }
    }
    let v = r.game.grounds[g.0].u.bigblue_road();
    if v.cur_lane() == 0xB {
        let stage = v.loop_stage();
        let rot = reg(v.rotation);
        if stage == 0 {
            v.set_loop_stage(1);
        } else if stage == 1 {
            let geo = r.game.geometry_mut();
            for k in [0x29, 0x2A, 0x2B] {
                geo.enable_joint(k);
            }
            for k in [0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x30] {
                geo.disable_joint(k);
            }
            r.game.grounds[g.0].u.bigblue_road().set_loop_stage(2);
        } else if stage == 2 && rot < reg(-std::f32::consts::FRAC_PI_6) {
            let geo = r.game.geometry_mut();
            for k in [0x2C, 0x2D, 0x2E] {
                geo.enable_joint(k);
            }
            r.game.grounds[g.0].u.bigblue_road().set_loop_stage(3);
        } else if stage == 3 && rot < reg(-2.792_526_7) {
            let geo = r.game.geometry_mut();
            for k in [0x29, 0x2A, 0x2B] {
                geo.disable_joint(k);
            }
            for k in [0x2F, 0x30, 0x31] {
                geo.enable_joint(k);
            }
            r.game.grounds[g.0].u.bigblue_road().set_loop_stage(4);
        }
    }
    let falling = r.game.grounds[g.0].u.bigblue_road().falling();
    let reach = if falling { 120.0 } else { 20.0 };
    let (ty, normal) = road_height(r.game, center, mem(fp::fmuls(reach, scale(r.game))));
    let mut target_y = reg(ty);
    let target;
    let rot_z;
    if reg(NONE_Y) != target_y && (!falling || target_y > reg(center.y)) {
        let max = pm.f(r.data, p::STEER_MAX);
        if target_y > max {
            target_y = max;
        } else if target_y < fp::fneg(max) {
            target_y = fp::fneg(max);
        }
        target = Vec3::new(center.x, mem(target_y), center.z);
        let v = r.game.grounds[g.0].u.bigblue_road();
        rot_z = if v.cur_lane() == 0xB && v.looping() {
            let n = normal.expect("a hit's normal");
            fp::fneg(reg(crate::math::atan2(mem(fp::fneg(reg(n.x))), n.y)))
        } else {
            fp::fneg(reg(v.rotation))
        };
        v.set_falling(false);
    } else {
        if !falling {
            let s = scale(r.game);
            let drift = fp::fmuls(pm.f(r.data, p::DRIFT), s);
            let v = r.game.grounds[g.0].u.bigblue_road();
            v.drift = v.previous_position - v.position;
            let sin = reg(crate::math::sin(v.rotation));
            v.lateral_drift = mem(fp::fmuls(sin, drift));
            v.set_falling(true);
        }
        let drop = pm.f(r.data, p::DROP);
        let s = scale(r.game);
        let v = r.game.grounds[g.0].u.bigblue_road();
        v.lateral_drift = if reg(NONE_Y) != target_y {
            mem(fp::fnmsubs(3.0, fp::fmuls(drop, s), reg(v.lateral_drift)))
        } else {
            mem(fp::fnmsubs(drop, s, reg(v.lateral_drift)))
        };
        target = Vec3::new(
            center.x,
            mem(fp::fadds(reg(center.y), reg(v.lateral_drift))),
            center.z,
        );
        rot_z = fp::fneg(reg(v.rotation));
    }
    let mut angular = rot_z;
    if rot_z > 0.0 {
        angular = 0.0;
    } else if abs(rot_z) > reg(0.003_490_658_5) {
        angular = fp::fmuls(angular, pm.f(r.data, p::TURN));
    }
    let v = r.game.grounds[g.0].u.bigblue_road();
    let mut vel = v.position - target;
    if angular != 0.0 {
        let s = reg(crate::math::sin(mem(angular)));
        let c = reg(crate::math::cos(mem(angular)));
        let (x, y) = (reg(vel.x), reg(vel.y));
        let ny = fp::fmadd(x, s, fp::fmul(y, c));
        vel.x = mem(fp::frsp(fp::fmsub(x, c, fp::fmul(y, s))));
        vel.y = mem(fp::frsp(ny));
    }
    let vel = vel + center;
    let v = r.game.grounds[g.0].u.bigblue_road();
    v.position = vel;
    let old = reg(v.rotation);
    v.rotation = mem(fp::fadds(old, angular));
    if !v.looping() && old > 0.0 && reg(v.rotation) <= 0.0 {
        v.rotation = 0.0;
    }
    let rot = reg(v.rotation);
    if rot > std::f64::consts::PI {
        v.rotation = mem(fp::frsp(fp::fsub(rot, std::f64::consts::TAU)));
    } else if rot < -std::f64::consts::PI {
        v.rotation = mem(fp::frsp(fp::fadd(rot, std::f64::consts::TAU)));
    }
    let s = scale(r.game);
    let road_x = pm.f(r.data, p::ROAD_X);
    let v = r.game.grounds[g.0].u.bigblue_road();
    v.position.x = mem(fp::fnmsubs(road_x, s, reg(v.position.x)));
    let d = r.game.gr.bigblue.track_deltas[entry_lane];
    let s = scale(r.game);
    let lat = fp::fmuls(
        reg(0.6),
        fp::fmuls(fp::fdivs(reg(d.z), reg(d.x)), fp::fmuls(road_x, s)),
    );
    let v = r.game.grounds[g.0].u.bigblue_road();
    let bz = reg(bone.z);
    if bz > lat {
        v.position.z = mem(fp::fsubs(reg(v.position.z), lat));
    } else if bz < fp::fneg(lat) {
        v.position.z = mem(fp::fadds(reg(v.position.z), lat));
    } else {
        v.position.z = mem(fp::fsubs(reg(v.position.z), bz));
    }
    let (pos, rotation) = (v.position, v.rotation);
    set_tx(r.game, j, pos.x);
    set_ty(r.game, j, pos.y);
    set_tz(r.game, j, pos.z);
    set_rot_z(r.game, j, rotation);
    let mut vel = vel;
    vel.rotate(crate::mtx::Axis::Z, mem(fp::fneg(reg(rotation))));
    vel.z = 0.0;
    vel.x = 0.0;
    vel.rotate(crate::mtx::Axis::Z, rotation);
    for map in [2, 1] {
        let mj = super::ground::map_gobj(r.game, map).and_then(|m| jobj_of(r.game, m));
        if mj.is_some() {
            set_translate(r.game, mj, vel);
            set_rot_z(r.game, mj, rotation);
        }
    }
    r.game.grounds[g.0].u.bigblue_road().previous_position = prev;
}

/// `lbl_803E3010`: the cars' engine sounds.
const ENGINE_SOUNDS: [u32; 4] = [0x6DDD2, 0x6DDD3, 0x6DDD4, 0x6DDD5];

/// A car's place along the road by its rank among `n` (the inline repeated in
/// `grBigBlue_801EC6C0` and `801ED694`): `(rank + 1)` of the spread over `n + 1`, from half
/// the spread back.
fn rank_place(game: &Game, data: &Archives, rank: i32, n: i32) -> f64 {
    let pm = param(game);
    let spread = fp::fmuls(to_f(pm.i(data, p::SPREAD)), scale(game));
    let step = fp::fdivs(spread, to_f(n.wrapping_add(1)));
    let lerp = fp::fmuls(to_f(rank.wrapping_add(1)), step);
    let half = fp::fneg(fp::fmuls(to_f(pm.i(data, p::SPREAD)), scale(game)));
    fp::fmadds(0.5, half, lerp)
}

/// A car's wait between its decisions, between two of the parameters.
fn car_wait(game: &mut Game, data: &Archives) -> i32 {
    let pm = param(game);
    let (lo, hi) = (
        fp::fctiwz(pm.f(data, p::WAIT_MIN)),
        fp::fctiwz(pm.f(data, p::WAIT_MAX)),
    );
    super::grlib::rand_range(&mut game.objs.rng, lo, hi)
}

/// The first cars, `grBigBlue_801EC6C0`: every collision joint's callback set and its car
/// hidden, each slot free on one side at random, then the parameters' number of cars (the
/// others off) each in a slot of its own at random on the road by its rank, its bob at a
/// random phase, its engine started.
#[doc(alias = "grBigBlue_801EC6C0")]
pub fn first_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let user = Some(super::CollUser::Ground(g));
    for (i, &joint) in CAR_COLL_JOINTS.iter().enumerate() {
        r.game
            .geometry_mut()
            .set_ledge_callback(joint, user, Some(CodeRef(CAR_TOUCHED)));
        let cj = r.game.grounds[g.0].u.bigblue_cars().collision_jobjs[i];
        jobj::set_flags_all(&mut r.game.objs.jobjs, cj, jobj::HIDDEN);
        let v = if r.game.objs.rng.below(2) != 0 { 0 } else { 2 };
        r.game.grounds[g.0].u.bigblue_cars().ranks[i] = v;
    }
    let pm = param(r.game);
    let n = pm.i(r.data, p::CARS);
    assert!(
        n != 0,
        "grbigblue.c: a cars count of 0, which the original writes over"
    );
    for car in 0..4 {
        if car < n {
            r.game.grounds[g.0].u.bigblue_cars().cars[car as usize].set_state(4);
            let line = loop {
                let line = r.game.objs.rng.below(30);
                let v = r.game.grounds[g.0].u.bigblue_cars();
                if !(0..car as usize).any(|i| v.cars[i].slot() == line as usize) {
                    break line;
                }
            };
            place_first_car(r, g, car as usize, line as usize);
        } else {
            r.game.grounds[g.0].u.bigblue_cars().cars[car as usize].set_state(1);
        }
    }
}

/// One of the first cars placed (the inline `grBigBlue_801EC6C0_inline`).
fn place_first_car(r: &mut Running, g: GroundId, car: usize, line: usize) {
    let pm = param(r.game);
    let n = pm.i(r.data, p::CARS);
    let x = mem(rank_place(r.game, r.data, car as i32, n));
    let ride = fp::fmuls(pm.f(r.data, p::RIDE), scale(r.game));
    let y = mem(fp::fmadds(pm.f(r.data, p::ROAD_Y), scale(r.game), ride));
    let target = mem(rank_place(r.game, r.data, car as i32, n));
    let rand = reg(r.game.objs.rng.draw_f32());
    let rotation = mem(fp::frsp(fp::fmul(std::f64::consts::TAU, rand)));
    let amplitude = mem(fp::fmuls(pm.f(r.data, p::BOB), scale(r.game)));
    let threshold = car_wait(r.game, r.data);
    {
        let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
        c.set_slot(line as u32);
        c.set_airborne(false);
        c.pos = Vec3::new(x, y, 0.0);
        c.target = target;
        c.delta = 0.0;
        c.gravity = 0.0;
        c.height = 0.0;
        c.velocity = 0.0;
        c.accel = 0.0;
        c.rotation = rotation;
        c.amplitude = amplitude;
        c.angular_velocity = 0.0;
        c.threshold = threshold;
    }
    let sound = ENGINE_SOUNDS[r.game.objs.rng.below(4) as usize];
    r.object_sound(Some(g), car as i32, sound);
    let v = r.game.grounds[g.0].u.bigblue_cars();
    v.cars[car].alpha = 1.0;
    let (cj, pos) = (v.collision_jobjs[line], v.cars[car].pos);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, cj, jobj::HIDDEN);
    set_translate(r.game, cj, pos);
    r.game.grounds[g.0].u.bigblue_cars().ranks[line] = 1;
}

/// Whether a car is on the road (not off, 1, nor driving out either way, 7 and 8).
fn racing(state: u32) -> bool {
    state != 1 && state != 7 && state != 8
}

/// The cars' upkeep, `grBigBlue_801ECB50`: the free slots kept on both sides; the wait before
/// the next car counted down faster with one car left; the car nearest the middle (more than
/// 60 out) made to pull in (10) when none does and none is near it, the cars driving out its
/// way turned back; and once the wait has run out, an off car brought in from the side the
/// others aren't leaving by, its wait drawn again.
#[doc(alias = "grBigBlue_801ECB50")]
pub fn manage_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let ranks = r.game.grounds[g.0].u.bigblue_cars().ranks.clone();
    let mut free = ranks.iter().filter(|&&v| v == 0).count() as i32;
    let mut reserved = ranks.iter().filter(|&&v| v == 2).count() as i32;
    if free == 0 {
        let mut pick = randi(r.game, reserved);
        let v = r.game.grounds[g.0].u.bigblue_cars();
        for k in 0..30 {
            if v.ranks[k] == 2 {
                pick -= 1;
                if pick < 0 {
                    v.ranks[k] = 0;
                    free += 1;
                    break;
                }
            }
        }
    } else if reserved == 0 {
        let mut pick = randi(r.game, free);
        let v = r.game.grounds[g.0].u.bigblue_cars();
        for k in 0..30 {
            if v.ranks[k] == 0 {
                pick -= 1;
                if pick < 0 {
                    v.ranks[k] = 2;
                    reserved += 1;
                    break;
                }
            }
        }
    }
    let v = r.game.grounds[g.0].u.bigblue_cars();
    let active = v.cars.iter().filter(|c| racing(c.state())).count();
    if active == 1 {
        v.spawn_timer = v.spawn_timer.wrapping_sub(1);
    }
    let mut found_ten = false;
    let mut closest: i32 = -1;
    let mut closest_dist = reg(f32::MAX);
    for (k, c) in v.cars.iter().enumerate() {
        let st = c.state();
        if st == 10 {
            found_ten = true;
            break;
        }
        if racing(st) {
            let d = abs(reg(c.pos.x));
            if d > 60.0 {
                if closest_dist > d {
                    closest_dist = d;
                    closest = k as i32;
                }
            } else {
                found_ten = true;
                break;
            }
        }
    }
    if !found_ten && closest != -1 {
        let t = closest as usize;
        v.cars[t].set_state(10);
        let tx = reg(v.cars[t].pos.x);
        for c in v.cars.iter_mut() {
            let st = c.state();
            let x = reg(c.pos.x);
            if (st == 7 && x < tx) || (st == 8 && x > tx) {
                c.set_state(4);
            }
        }
    }
    let timer = v.spawn_timer;
    v.spawn_timer = timer.wrapping_sub(1);
    if timer >= 0 {
        return;
    }
    let Some(car) = v.cars.iter().position(|c| c.state() == 1) else {
        return;
    };
    let right = v.cars.iter().filter(|c| c.state() == 7).count();
    let left = v.cars.iter().filter(|c| c.state() == 8).count();
    let direction = if right == 0 && left == 0 {
        if r.game.objs.rng.below(2) != 0 { 1 } else { -1 }
    } else if right != 0 {
        -1
    } else {
        1
    };
    let (want, count, state) = if direction == 1 {
        (0u8, free, 5)
    } else {
        (2u8, reserved, 6)
    };
    let mut pick = randi(r.game, count);
    for k in 0..30 {
        if r.game.grounds[g.0].u.bigblue_cars().ranks[k] == want {
            pick -= 1;
            if pick < 0 {
                if change_car(r, gobj, car, state) {
                    let pm = param(r.game);
                    let (a, b) = (pm.i(r.data, p::SPAWN_MAX), pm.i(r.data, p::SPAWN_MIN));
                    let t = spawn_wait(r.game, a, b);
                    r.game.grounds[g.0].u.bigblue_cars().spawn_timer = t as i16;
                }
                return;
            }
        }
    }
}

/// One car for the tick, `grBigBlue_801ED694`: its decision and its state's change, its
/// acceleration, its place along the road (pulling in by its rank among the racing cars
/// unless driving out), its speed capped, its fall and bob, then on the road kept on the
/// ground (else flying off at its tilt's speed) or in the air falling with the road until it
/// lands, its tilt easing toward the ground's slope (or its rest in the air), its joint
/// placed.
#[doc(alias = "grBigBlue_801ED694")]
pub fn drive_car(r: &mut Running, gobj: GObjId, car: usize) {
    let status = decide_car(r, gobj, car);
    if status != 0 {
        change_car(r, gobj, car, status);
    }
    if !accelerate_car(r, gobj, car) {
        return;
    }
    let g = ground_of(r.game, gobj);
    let pm = param(r.game);
    let v = r.game.grounds[g.0].u.bigblue_cars();
    let slot = v.cars[car].slot();
    let j = v.collision_jobjs[slot];
    let mut rot = rot_z(r.game, j);
    let v = r.game.grounds[g.0].u.bigblue_cars();
    let st = v.cars[car].state();
    let target = if st != 7 && st != 8 {
        let mut active = 0;
        let mut behind = 0;
        let my_x = reg(v.cars[car].pos.x);
        for (k, c) in v.cars.iter().enumerate() {
            if racing(c.state()) {
                active += 1;
                if k != car && reg(c.pos.x) < my_x {
                    behind += 1;
                }
            }
        }
        rank_place(r.game, r.data, behind, active)
    } else {
        reg(v.cars[car].target)
    };
    let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
    let current = reg(c.target);
    let diff = fp::fsubs(target, current);
    if diff < reg(-0.5) {
        c.target = mem(fp::fsubs(current, 0.5));
    } else if diff > 0.5 {
        c.target = mem(fp::fadds(reg(c.target), 0.5));
    } else {
        c.target = mem(target);
    }
    c.velocity = mem(fp::fadds(reg(c.velocity), reg(c.accel)));
    let top = fp::fmuls(pm.f(r.data, p::TOP_SPEED), scale(r.game));
    let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
    if reg(c.velocity) > top {
        c.velocity = mem(top);
    } else if reg(c.velocity) < fp::fneg(top) {
        c.velocity = mem(fp::fneg(top));
    }
    c.delta = mem(fp::fadds(reg(c.delta), reg(c.velocity)));
    c.pos.x = mem(fp::fadds(reg(c.target), reg(c.delta)));
    let airborne = c.airborne();
    if !airborne {
        let fall = pm.f(r.data, p::FALL);
        let fall_max = pm.f(r.data, p::FALL_MAX);
        let s = scale(r.game);
        let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
        c.gravity = mem(fp::fmadds(fall, s, reg(c.gravity)));
        c.height = mem(fp::fadds(reg(c.height), reg(c.gravity)));
        if reg(c.height) > 0.0 {
            c.height = 0.0;
            c.gravity = 0.0;
        } else {
            let lim = fp::fneg(fp::fmuls(fall_max, s));
            if reg(c.height) < lim {
                c.height = mem(lim);
            }
        }
    } else {
        let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
        c.height = 0.0;
        c.gravity = 0.0;
    }
    let heading = if !airborne {
        let vel = reg(r.game.grounds[g.0].u.bigblue_cars().cars[car].velocity);
        let s = scale(r.game);
        let bob = fp::fmuls(pm.f(r.data, p::BOB), s);
        if vel > 0.0 {
            let ground = fp::fmuls(pm.f(r.data, p::TOP_SPEED), s);
            let k = fp::fdivs(fp::fmsubs(pm.f(r.data, p::BOB_FORE), s, bob), ground);
            fp::fmadds(pm.f(r.data, p::BOB), s, fp::fmuls(vel, k))
        } else if vel < 0.0 {
            let ground = fp::fmuls(pm.f(r.data, p::TOP_SPEED), s);
            let k = fp::fdivs(fp::fmsubs(pm.f(r.data, p::BOB_BACK), s, bob), ground);
            fp::fmadds(pm.f(r.data, p::BOB), s, fp::fmuls(vel, k))
        } else {
            bob
        }
    } else {
        0.0
    };
    let ease = pm.f(r.data, p::BOB_EASE);
    let turn = pm.f(r.data, p::BOB_TURN);
    let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
    c.amplitude = mem(fp::fmadds(
        ease,
        fp::fsubs(heading, reg(c.amplitude)),
        reg(c.amplitude),
    ));
    c.rotation = mem(fp::fmadds(reg(0.017_453_292), turn, reg(c.rotation)));
    if reg(c.rotation) > std::f64::consts::TAU {
        c.rotation = mem(fp::frsp(fp::fsub(reg(c.rotation), std::f64::consts::TAU)));
    }
    let osc = fp::fmuls(reg(c.amplitude), reg(crate::math::sin(c.rotation)));
    let s = scale(r.game);
    let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
    let floor = fp::fsubs(
        fp::fsubs(
            fp::fnmsubs(pm.f(r.data, p::RIDE), s, reg(c.pos.y)),
            reg(c.height),
        ),
        osc,
    );
    let pos = c.pos;
    let (ground_y, sp) = if airborne {
        let (gy, sp) = road_height(r.game, pos, 500.0);
        let gy = reg(gy);
        let av = reg(r.game.grounds[g.0].u.bigblue_cars().cars[car].angular_velocity);
        if reg(NONE_Y) != gy && gy > floor && av < 0.0 {
            r.game.grounds[g.0].u.bigblue_cars().cars[car].set_airborne(false);
        } else {
            let fall = pm.f(r.data, p::AIR_FALL);
            let s = scale(r.game);
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.angular_velocity = if reg(NONE_Y) != gy {
                mem(fp::fnmsubs(
                    3.0,
                    fp::fmuls(fall, s),
                    reg(c.angular_velocity),
                ))
            } else {
                mem(fp::fnmsubs(fall, s, reg(c.angular_velocity)))
            };
            c.pos.y = mem(fp::fadds(reg(c.pos.y), reg(c.angular_velocity)));
            let road = super::ground::map_gobj(r.game, 0x22).expect("grbigblue.c: 3255: map_gobj");
            let rg = ground_of(r.game, road);
            let rv = r.game.grounds[rg.0].u.bigblue_road();
            let dy = fp::fsubs(reg(rv.position.y), reg(rv.previous_position.y));
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.pos.y = mem(fp::fadds(reg(c.pos.y), dy));
        }
        (gy, sp)
    } else {
        let s = scale(r.game);
        let (gy, sp) = road_height(r.game, pos, mem(fp::fmuls(20.0, s)));
        let gy = reg(gy);
        if reg(NONE_Y) != gy {
            let s = scale(r.game);
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            let lifted = fp::fmadds(pm.f(r.data, p::RIDE), s, gy);
            c.pos.y = mem(fp::fadds(osc, fp::fadds(reg(c.height), lifted)));
        } else {
            let s = scale(r.game);
            let launch = fp::fmuls(pm.f(r.data, p::LAUNCH), s);
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.set_airborne(true);
            let sin = reg(crate::math::sin(mem(rot)));
            c.angular_velocity = mem(fp::fmuls(sin, launch));
        }
        (gy, sp)
    };
    let airborne = r.game.grounds[g.0].u.bigblue_cars().cars[car].airborne();
    if reg(NONE_Y) != ground_y && !airborne {
        let n = sp.expect("a hit's normal");
        let angle = reg(crate::math::atan2(mem(fp::fneg(reg(n.x))), n.y));
        rot = fp::fmadds(pm.f(r.data, p::TILT_EASE), fp::fsubs(angle, rot), rot);
    } else {
        let rest = fp::fmsubs(reg(0.017_453_292), pm.f(r.data, p::TILT_REST), rot);
        rot = fp::fadds(rot, fp::fmuls(pm.f(r.data, p::TILT_BACK), rest));
    }
    let pos = r.game.grounds[g.0].u.bigblue_cars().cars[car].pos;
    set_translate(r.game, j, pos);
    set_rot_z(r.game, j, mem(rot));
}

/// A car's decision, `grBigBlue_801EDF44`: a fading car (9) goes off (1) once silent; a car
/// driving out (7, 8) past the side fades (9) or goes off; a racing car, once its wait runs out,
/// draws its next state (4 most of the time, else out the side with fewer cars, toward a
/// pulling-in car's side, or at random), follows a car driving out past it, and goes off below
/// -2000.
#[doc(alias = "grBigBlue_801EDF44")]
pub fn decide_car(r: &mut Running, gobj: GObjId, index: usize) -> u32 {
    let g = ground_of(r.game, gobj);
    let pm = param(r.game);
    let st = r.game.grounds[g.0].u.bigblue_cars().cars[index].state();
    let mut result = 0;
    match st {
        9 => {
            if reg(r.game.grounds[g.0].u.bigblue_cars().cars[index].alpha) == 0.0 {
                result = 1;
            }
        }
        7 | 8 => {
            let s = scale(r.game);
            let off = pm.f(r.data, p::OFF_SIDE);
            let edge = if st == 7 {
                fp::fmadds(off, s, reg(super::stage::blast_right(r.game)))
            } else {
                fp::fnmsubs(off, s, reg(super::stage::blast_left(r.game)))
            };
            let c = &r.game.grounds[g.0].u.bigblue_cars().cars[index];
            let past = if st == 7 {
                reg(c.pos.x) > edge
            } else {
                reg(c.pos.x) < edge
            };
            if past {
                result = if reg(c.alpha) != 0.0 { 9 } else { 1 };
            }
        }
        2..=6 | 10 => {
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[index];
            let t = c.threshold;
            c.threshold = t.wrapping_sub(1);
            if t < 0 {
                while result == 0 {
                    match r.game.objs.rng.below(4) {
                        0..=2 => result = 4,
                        _ => {
                            let v = r.game.grounds[g.0].u.bigblue_cars();
                            let me = reg(v.cars[index].pos.x);
                            let (mut rc, mut lc, mut r10, mut l10) = (0, 0, 0, 0);
                            for (i, c) in v.cars.iter().enumerate() {
                                let st = c.state();
                                if i == index || !racing(st) {
                                    continue;
                                }
                                let x = reg(c.pos.x);
                                if x > me {
                                    if st == 10 {
                                        r10 += 1;
                                    } else {
                                        rc += 1;
                                    }
                                } else if x < me {
                                    if st == 10 {
                                        l10 += 1;
                                    } else {
                                        lc += 1;
                                    }
                                }
                            }
                            if rc != 0 {
                                if lc != 0 {
                                    if r10 != 0 {
                                        if l10 != 0 {
                                            continue;
                                        }
                                        result = 8;
                                    } else if l10 != 0 {
                                        result = 7;
                                    } else {
                                        result = if r.game.objs.rng.below(2) != 0 { 7 } else { 8 };
                                    }
                                } else {
                                    result = 8;
                                }
                            } else if lc != 0 {
                                result = 7;
                            }
                        }
                    }
                }
            }
            let v = r.game.grounds[g.0].u.bigblue_cars();
            let me = reg(v.cars[index].pos.x);
            for (i, c) in v.cars.iter().enumerate() {
                if i == index {
                    continue;
                }
                let st = c.state();
                let x = reg(c.pos.x);
                if st == 7 && x < me {
                    result = 7;
                    continue;
                }
                if st == 8 && x > me {
                    result = 8;
                }
            }
            if reg(v.cars[index].pos.y) < -2000.0 {
                result = 1;
            }
        }
        _ => {}
    }
    result
}

/// Brings a car on from a side (the inline's cases 5 and 6 of `grBigBlue_801EE398`): at the
/// road's height `x` out, in a free slot at random, at rest, silent, its engine started in
/// the slot named by its model (the original's slip: the sound's slot is the collision slot,
/// so only the first eight models sound).
fn bring_car(r: &mut Running, g: GroundId, car: usize, x: f32, state: u32) -> bool {
    let probe = Vec3::new(x, 0.0, 0.0);
    let y = road_height(r.game, probe, 1000.0).0;
    if reg(NONE_Y) == reg(y) {
        return false;
    }
    let count = r.game.grounds[g.0]
        .u
        .bigblue_cars()
        .ranks
        .iter()
        .filter(|&&v| v == 0)
        .count() as i32;
    if count == 0 {
        return false;
    }
    let mut pick = randi(r.game, count);
    let mut slot = 0;
    let v = r.game.grounds[g.0].u.bigblue_cars();
    while slot < 30 {
        if v.ranks[slot] == 0 {
            pick -= 1;
            if pick < 0 {
                break;
            }
        }
        slot += 1;
    }
    let pm = param(r.game);
    let s = scale(r.game);
    let py = mem(fp::fmadds(pm.f(r.data, p::RIDE), s, reg(y)));
    let rand = reg(r.game.objs.rng.draw_f32());
    let rotation = mem(fp::frsp(fp::fmul(std::f64::consts::TAU, rand)));
    let amplitude = mem(fp::fmuls(pm.f(r.data, p::BOB), scale(r.game)));
    let threshold = car_wait(r.game, r.data);
    {
        let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
        c.set_slot(slot as u32);
        c.set_airborne(false);
        c.pos = Vec3::new(x, py, 0.0);
        c.target = x;
        c.delta = 0.0;
        c.gravity = 0.0;
        c.height = 0.0;
        c.velocity = 0.0;
        c.accel = 0.0;
        c.rotation = rotation;
        c.amplitude = amplitude;
        c.angular_velocity = 0.0;
        c.threshold = threshold;
    }
    let v = r.game.grounds[g.0].u.bigblue_cars();
    v.ranks[slot] = 1;
    v.cars[car].alpha = 0.0;
    let sound = ENGINE_SOUNDS[r.game.objs.rng.below(4) as usize];
    r.object_sound(Some(g), slot as i32, sound);
    let alpha = r.game.grounds[g.0].u.bigblue_cars().cars[car].alpha;
    r.object_sound_volume(Some(g), car as i32, alpha);
    let v = r.game.grounds[g.0].u.bigblue_cars();
    let (cj, pos) = (v.collision_jobjs[slot], v.cars[car].pos);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, cj, jobj::HIDDEN);
    set_translate(r.game, cj, pos);
    r.game.grounds[g.0].u.bigblue_cars().cars[car].set_state(state);
    true
}

/// Changes a car's state, `grBigBlue_801EE398`: off (1) its car hidden and its slot free on
/// the side it left by; pulling in (10) with a wait of 20; racing (2 to 4) with a wait drawn;
/// in from the right (5) or the left (6) as [`bring_car`] places it; driving out (7, 8) with a
/// wait of 1000; fading (9). Whether it changed.
#[doc(alias = "grBigBlue_801EE398")]
pub fn change_car(r: &mut Running, gobj: GObjId, car: usize, state: u32) -> bool {
    let g = ground_of(r.game, gobj);
    let pm = param(r.game);
    match state {
        1 => {
            let v = r.game.grounds[g.0].u.bigblue_cars();
            let slot = v.cars[car].slot();
            let cj = v.collision_jobjs[slot];
            jobj::set_flags_all(&mut r.game.objs.jobjs, cj, jobj::HIDDEN);
            let v = r.game.grounds[g.0].u.bigblue_cars();
            v.ranks[slot] = if reg(v.cars[car].pos.x) > 0.0 { 0 } else { 2 };
            v.cars[car].set_state(state);
            true
        }
        10 => {
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.threshold = 0x14;
            c.set_state(state);
            true
        }
        2..=4 => {
            let t = car_wait(r.game, r.data);
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.threshold = t;
            c.set_state(state);
            true
        }
        5 => {
            let s = scale(r.game);
            let x = mem(fp::fmadds(
                pm.f(r.data, p::OFF_SIDE),
                s,
                reg(super::stage::blast_right(r.game)),
            ));
            bring_car(r, g, car, x, state)
        }
        6 => {
            let s = scale(r.game);
            let x = mem(fp::fnmsubs(
                pm.f(r.data, p::OFF_SIDE),
                s,
                reg(super::stage::blast_left(r.game)),
            ));
            bring_car(r, g, car, x, state)
        }
        7 | 8 => {
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[car];
            c.threshold = 0x3E8;
            c.set_state(state);
            true
        }
        9 => {
            r.game.grounds[g.0].u.bigblue_cars().cars[car].set_state(state);
            false
        }
        _ => false,
    }
}

/// A car's acceleration for its state, `grBigBlue_801EEF00` (false for an off car): pulling in
/// (10) toward the middle, braking once its speed would carry it there; racing right (2) or
/// left (3) until in view; racing (4) toward its target, braking to stop there; coming in (5,
/// 6) at rest, its engine fading in; driving out right (7) or left (8); fading (9) its engine
/// out.
#[doc(alias = "grBigBlue_801EEF00")]
pub fn accelerate_car(r: &mut Running, gobj: GObjId, index: usize) -> bool {
    let g = ground_of(r.game, gobj);
    let pm = param(r.game);
    let st = r.game.grounds[g.0].u.bigblue_cars().cars[index].state();
    let accel = |r: &Running| fp::fmuls(pm.f(r.data, p::ACCEL), scale(r.game));
    let norm = |r: &mut Running| {
        let c = &r.game.grounds[g.0].u.bigblue_cars().cars[index];
        let v = abs(reg(c.velocity));
        let a = accel(r);
        fp::fdivs(v, a)
    };
    let set = |r: &mut Running, a: f64| {
        r.game.grounds[g.0].u.bigblue_cars().cars[index].accel = mem(a);
    };
    match st {
        1 => return false,
        10 => {
            let right = reg(r.game.grounds[g.0].u.bigblue_cars().cars[index].pos.x) > 0.0;
            let n = norm(r);
            let t = to_f(r.game.grounds[g.0].u.bigblue_cars().cars[index].threshold);
            let a = accel(r);
            let far = t > n;
            set(r, if far == right { fp::fneg(a) } else { a });
        }
        2 => {
            let n = norm(r);
            let c = &r.game.grounds[g.0].u.bigblue_cars().cars[index];
            let (t, x) = (to_f(c.threshold), reg(c.pos.x));
            let a = accel(r);
            if t > n && x < reg(super::stage::cam_right(r.game)) {
                set(r, a);
            } else {
                set(r, fp::fneg(a));
            }
        }
        3 => {
            let n = norm(r);
            let c = &r.game.grounds[g.0].u.bigblue_cars().cars[index];
            let (t, x) = (to_f(c.threshold), reg(c.pos.x));
            let a = accel(r);
            if t > n && x > reg(super::stage::cam_left(r.game)) {
                set(r, fp::fneg(a));
            } else {
                set(r, a);
            }
        }
        4 => {
            let n = norm(r);
            let c = r.game.grounds[g.0].u.bigblue_cars().cars[index].clone();
            let (target, vel, x) = (reg(c.target), reg(c.velocity), reg(c.pos.x));
            let a = accel(r);
            if target > x {
                if vel < 0.0 {
                    set(r, a);
                } else {
                    let half = fp::fmuls(n, fp::fmuls(0.5, accel(r)));
                    let predicted = fp::fnmsubs(n, half, fp::fmadds(vel, n, x));
                    set(r, if predicted < target { a } else { fp::fneg(a) });
                }
            } else if vel > 0.0 {
                set(r, fp::fneg(a));
            } else {
                let half = fp::fmuls(n, fp::fmuls(0.5, fp::fneg(accel(r))));
                let predicted = fp::fnmsubs(n, half, fp::fmadds(vel, n, x));
                set(r, if predicted < target { a } else { fp::fneg(a) });
            }
        }
        5 | 6 => {
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[index];
            c.accel = 0.0;
            c.alpha = mem(fp::fadds(reg(c.alpha), reg(1.0 / 60.0)));
            if reg(c.alpha) > 1.0 {
                c.alpha = 1.0;
            }
            let a = c.alpha;
            r.object_sound_volume(Some(g), index as i32, a);
        }
        7 => {
            let a = accel(r);
            set(r, a);
        }
        8 => {
            let a = accel(r);
            set(r, fp::fneg(a));
        }
        9 => {
            let c = &mut r.game.grounds[g.0].u.bigblue_cars().cars[index];
            c.alpha = mem(fp::fsubs(reg(c.alpha), reg(1.0 / 60.0)));
            if reg(c.alpha) < 0.0 {
                c.alpha = 0.0;
            }
            let a = c.alpha;
            r.object_sound_volume(Some(g), index as i32, a);
        }
        _ => {}
    }
    true
}

/// The big cars for the tick, `grBigBlue_801EF424`: their upkeep, each car, then up to ten
/// passes pushing apart the racing cars closer than the parameters' gap, half the overlap
/// each.
#[doc(alias = "grBigBlue_801EF424")]
pub fn drive_cars(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    manage_cars(r, gobj);
    for car in 0..4 {
        drive_car(r, gobj, car);
    }
    let pm = param(r.game);
    let gap_i = pm.i(r.data, p::GAP);
    let mut changed = true;
    let mut k = 0;
    while k < 10 && changed {
        changed = false;
        for i in 0..4 {
            if r.game.grounds[g.0].u.bigblue_cars().cars[i].state() == 1 {
                continue;
            }
            for jx in 0..4 {
                if i == jx {
                    continue;
                }
                let v = r.game.grounds[g.0].u.bigblue_cars();
                if v.cars[jx].state() == 1 {
                    continue;
                }
                let mut diff = fp::fsubs(reg(v.cars[i].pos.x), reg(v.cars[jx].pos.x));
                let gap = fp::fmuls(scale(r.game), to_f(gap_i));
                if abs(diff) < gap {
                    let adj = if diff > 0.0 {
                        fp::fmuls(scale(r.game), to_f(gap_i))
                    } else {
                        fp::fneg(fp::fmuls(scale(r.game), to_f(gap_i)))
                    };
                    diff = fp::fsubs(diff, adj);
                    changed = true;
                    diff = fp::frsp(fp::fmul(diff, 0.5));
                    let v = r.game.grounds[g.0].u.bigblue_cars();
                    let a = &mut v.cars[i];
                    a.delta = mem(fp::fsubs(reg(a.delta), diff));
                    a.pos.x = mem(fp::fsubs(reg(a.pos.x), diff));
                    let b = &mut v.cars[jx];
                    b.delta = mem(fp::fadds(reg(b.delta), diff));
                    b.pos.x = mem(fp::fadds(reg(b.pos.x), diff));
                }
            }
        }
        k += 1;
    }
}

/// The big cars' collision joints' callback, `fn_801EF60C`: a fighter landing on a car
/// (collision kind 1, ground kind 1) pushes the car of that model down by its weight.
#[doc(alias = "fn_801EF60C")]
pub fn car_touched(
    gp: &mut super::super::structs::Ground,
    param: Option<DataRef>,
    data: &Archives,
    joint_id: i32,
    coll: &super::super::structs::CollData,
    kind: i32,
) {
    if coll.x34_flags.b1234 != 1 {
        return;
    }
    if kind != 1 {
        return;
    }
    let car_num = CAR_COLL_JOINTS
        .iter()
        .position(|&k| k == joint_id)
        .expect("grbigblue.c: 0xED9: car_num!=Gr_Fzero_Car_Max");
    let push = reg(data.f32(param.expect("grbigblue.c: yakumono_param").at(p::PUSH)));
    let v = gp.u.bigblue_cars();
    for c in v.cars.iter_mut() {
        if c.slot() == car_num {
            c.gravity = mem(fp::fmuls(fp::fneg(reg(coll.x50)), push));
        }
    }
}

/// The wind Big Blue blows on airborne items, `grBigBlue_801EF7D8`: 10 left while the road
/// stands.
#[doc(alias = "grBigBlue_801EF7D8")]
pub fn wind(game: &Game) -> Vec3 {
    let road = super::ground::map_gobj(game, 0x22);
    if road.is_some_and(|g| game.world.gobj(g).user_data.is_some()) {
        Vec3::new(-10.0, 0.0, 0.0)
    } else {
        Vec3::ZERO
    }
}

/// The stage's lines' hazards, `grBigBlue_801EFC0C`: none.
#[doc(alias = "grBigBlue_801EFC0C")]
pub fn line_hazard() -> Option<DataRef> {
    None
}

/// Whether a joint takes a fighter's shadow, `grBigBlue_801EFC14`: the fighter is above it.
#[doc(alias = "grBigBlue_801EFC14")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let v = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(v.y)
}
