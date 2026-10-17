//! Hyrule Temple, `melee/gr/grshrine.c`: the temple (map 2), the background (map 1) and the
//! sky (map 0), with a breeze over the whole stage.
//!
//! The stage does nothing during play beyond its collision following the model and the wind
//! sources aging; the computer player's route through the temple is the CPU AI's
//! (`grshrineroute.c` is a single-player stage's).

use melee_data::archive::Archives;

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{SHRINE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, update_map_coll};
use super::{anime, setup, zako};
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `grShrine_80201E98`, the temple's start hook, which does nothing.
const START_HOOK: u32 = 0x8020_1E98;

/// A stage object's process or callback, by address: Hyrule Temple's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_1E3C | 0x8020_1E40 | 0x8020_1E90 | 0x8020_1E94 | 0x8020_1E98 | 0x8020_1F40 => {
            empty(r, gobj)
        }
        0x8020_1E08 => init_sky(r, gobj),
        0x8020_1E44 => init_back(r, gobj),
        0x8020_1E9C => init_temple(r, gobj),
        0x8020_1F1C => proc_temple(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_1C64 => on_init(r),
        0x8020_1C60 => on_demo_init(r),
        0x8020_1CF0 => on_load(r),
        0x8020_1CF4 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes, destroy callbacks and start hook that do nothing.
#[doc(alias = "grShrine_80201E3C")]
#[doc(alias = "grShrine_80201E40")]
#[doc(alias = "grShrine_80201E90")]
#[doc(alias = "grShrine_80201E94")]
#[doc(alias = "grShrine_80201E98")]
#[doc(alias = "grShrine_80201F40")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grShrine_80201D20`.
#[doc(alias = "grShrine_80201D20")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grSh_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &SHRINE_CALLBACKS[map_id as usize]
}

/// Hyrule Temple's setup, `grShrine_80201C64`: its parameters (which nothing reads), maps 0 to
/// 2, camera limits and blast zones, and a gentle push over the whole stage that sways
/// jointed parts.
#[doc(alias = "grShrine_80201C64")]
pub fn on_init(r: &mut Running) {
    r.game.gr.shrine_yakumono = super::ground::yakumono_param(r.game);
    for map_id in 0..3 {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let angle = gekko_fp::frsp(gekko_fp::fdiv(std::f64::consts::PI, 3.0));
    crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(0.5, 0.0, 0.0),
        -1,
        0.5,
        0.0,
        angle as f32,
        -10000.0,
        10000.0,
        10000.0,
        -10000.0,
    );
}

/// `grShrine_80201C60` does nothing (only a demo runs it).
#[doc(alias = "grShrine_80201C60")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grShrine_UnkStage0_OnLoad` does nothing.
#[doc(alias = "grShrine_UnkStage0_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Hyrule Temple's start, `grShrine_UnkStage0_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grShrine_UnkStage0_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The sky's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@80201E08")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 1), `grShrine_80201E44`: its animation, drawn behind.
#[doc(alias = "grShrine_80201E44")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The temple's setup (map 2), `grShrine_80201E9C`: its collision and animation, a start hook
/// that does nothing, the collision moved to the model at once, and it receives shadows.
#[doc(alias = "grShrine_80201E9C")]
pub fn init_temple(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    update_map_coll(r.game, r.data, gobj);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The temple's process (map 2), `grShrine_80201F1C`: the collision follows the model, and the
/// wind sources age.
#[doc(alias = "grShrine_80201F1C")]
pub fn proc_temple(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The stage's shadow check, `grShrine_80201F4C`: a fighter casts on a joint below its feet.
#[doc(alias = "grShrine_80201F4C")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
