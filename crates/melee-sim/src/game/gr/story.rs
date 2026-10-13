//! Yoshi's Story, `melee/gr/grstory.c`: the main stage with its Shy Guys (map 3), Randall the
//! cloud under the side platform (map 2), and the background (map 1).
//!
//! Every two seconds with no Shy Guy left, the Shy Guys' controller sends a flight of them
//! (`It_Kind_Heiho` items, `it::kinds::heiho`) across the stage from one of six heights on
//! either side, a different one each time. Randall puffs a cloud effect every 10 to 29 ticks.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::state::{Game, Running};
use super::data::{STORY_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The Shy Guys' controller's map id.
pub const MAP_SHYGUYS: i32 = 3;

/// The fields of `grStory_YakumonoParam` the code reads, by offset.
const TIMER_MIN: u32 = 0;
const TIMER_RAND: u32 = 4;
const SPAWNMANY_RARITY: u32 = 8;
const VPOS: u32 = 0xC;

/// A stage object's process or callback, by address: Yoshi's Story's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801E_322C | 0x801E_3230 | 0x801E_336C | 0x801E_3414 => empty(r, gobj),
        0x801E_31C0 => init_background(r, gobj),
        0x801E_3234 => init_shyguys(r, gobj),
        0x801E_3334 => proc_shyguys(r, gobj),
        0x801E_3370 => init_randall(r, gobj),
        0x801E_33E0 => proc_randall(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801E_3030 => on_init(r),
        0x801E_30A8 => on_load(r),
        0x801E_30AC => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grStory_801E322C")]
// Also ports the destroy callbacks `grStory_801E3230`, `grStory_801E336C` and
// `grStory_801E3414`; aliased once a check reaches them (no stage object is destroyed).
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grStory_801E30D8`.
#[doc(alias = "grStory_801E30D8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grSt_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &STORY_CALLBACKS[map_id as usize]
}

fn param(game: &Game) -> DataRef {
    game.gr.story_yakumono.expect("grstory.c: yakumono_param")
}

/// Yoshi's Story's setup, `grStory_801E3030`: its parameters, maps 0, 1, 3 and 2, camera
/// limits and blast zones.
#[doc(alias = "grStory_801E3030")]
pub fn on_init(r: &mut Running) {
    r.game.gr.story_yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, 1, setup::SETUP_FRAME);
    create(r, 3, setup::SETUP_FRAME);
    create(r, 2, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grStory_801E302C` does nothing. Ports `grStory_801E302C`; aliased once a check reaches it
/// (only a demo runs it).
pub fn on_demo_init(_r: &mut Running) {}

/// `grStory_UnkStage0_OnLoad` does nothing; `grStory_801E30D0` (`callback4`) answers false and
/// nothing calls it.
#[doc(alias = "grStory_UnkStage0_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Yoshi's Story's start, `grStory_UnkStage0_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grStory_UnkStage0_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The background's setup (map 1), `grStory_801E31C0`: its animation, and its first part's
/// materials marked with render-mode bit 29.
#[doc(alias = "grStory_801E31C0")]
pub fn init_background(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
    let j = nth_joint(r.game, gobj, 1);
    material::set_tree_flags(r.game, j, 0x2000_0000);
}

/// The Shy Guys' timer reset (the inline `reset_shyguy_timer`): a random wait the parameters
/// give, drawn and then overwritten with 120.
fn reset_timer(game: &mut Game, data: &melee_data::archive::Archives) {
    let p = param(game);
    let max = fp::fctiwz(reg(data.f32(p.at(TIMER_RAND))));
    let drawn = if max != 0 {
        game.objs.rng.below(max)
    } else {
        0
    };
    let _ = fp::fctiwz(fp::fadds(
        reg(data.f32(p.at(TIMER_MIN))),
        fp::frsp(f64::from(drawn)),
    ));
}

/// The Shy Guys' controller's setup (map 3), `grStory_801E3234`: the stage's collision, two
/// parts' animations, and the first wait.
#[doc(alias = "grStory_801E3234")]
pub fn init_shyguys(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 1.0, true);
    anime::start(r.game, r.data, gobj, 5, 7, 1, 0.0, 1.0, true);
    reset_timer(r.game, r.data);
    r.game.grounds[g.0].u.story_shyguys().timer = 120;
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The Shy Guys' controller's process, `grStory_801E3334`: the flights, the collision
/// following the model, and the wind.
#[doc(alias = "grStory_801E3334")]
pub fn proc_shyguys(r: &mut Running, gobj: GObjId) {
    shyguys_tick(r, gobj);
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The Shy Guys' flights, `grStory_801E3418`: once none is left and the wait is over, a new
/// wait, a height other than the last, and two (the parameters' rarity is overwritten) of the
/// three speeds drawn and sent, each 25 ticks after the one before, around that height.
#[doc(alias = "grStory_801E3418")]
pub fn shyguys_tick(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if crate::game::it::lib26b1::count_kind(r.game, crate::game::it::kinds::HEIHO) != 0 {
        return;
    }
    let v = r.game.grounds[g.0].u.story_shyguys();
    if v.timer != 0 {
        v.timer = v.timer.wrapping_sub(1);
        return;
    }
    reset_timer(r.game, r.data);
    r.game.grounds[g.0].u.story_shyguys().timer = 120;
    let pattern = loop {
        let p = r.game.objs.rng.below(6);
        if i32::from(r.game.grounds[g.0].u.story_shyguys().pattern) != p {
            break p;
        }
    };
    r.game.grounds[g.0].u.story_shyguys().pattern = pattern as i8;
    let p = param(r.game);
    let vpos = r.data.f32(p.at(VPOS + 4 * pattern as u32));
    let mut pos = Vec3::new(if pattern < 3 { -292.0 } else { 304.0 }, vpos, 2.0);
    let speed = r.game.objs.rng.below(3);
    let rarity = fp::fctiwz(reg(r.data.f32(p.at(SPAWNMANY_RARITY))));
    spawn_count(r.game, g, rarity);
    spawn_count(r.game, g, 2);
    let mut i = 0;
    while i < i32::from(r.game.grounds[g.0].u.story_shyguys().count) {
        let delay = fp::fctiwz(fp::fmuls(25.0, fp::frsp(f64::from(i))));
        crate::game::it::kinds::heiho::spawn(r, i, pos, speed, delay);
        let f = reg(r.game.objs.rng.draw_f32());
        let amp = fp::fmuls(2.0, fp::fsubs(f, 0.5));
        pos.y = mem(fp::fmadds(3.0, amp, reg(vpos)));
        i += 1;
    }
}

/// How many Shy Guys fly (the inline `set_shyguy_spawn_count`): 3 to 5 one time in `rarity`,
/// else 1.
fn spawn_count(game: &mut Game, g: super::super::handles::GroundId, rarity: i32) {
    let many = if rarity != 0 {
        game.objs.rng.below(rarity)
    } else {
        0
    } == 0;
    let count = if many { game.objs.rng.below(3) + 3 } else { 1 };
    game.grounds[g.0].u.story_shyguys().count = count as i8;
}

/// Randall's setup (map 2), `grStory_801E3370`: its collision, its animation, and the joint
/// it puffs clouds from.
#[doc(alias = "grStory_801E3370")]
pub fn init_randall(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    let puff = nth_joint(r.game, gobj, 1);
    let v = r.game.grounds[g.0].u.story_randall();
    v.timer = 0;
    v.jobj = puff;
}

/// Randall's process, `grStory_801E33E0`: the collision follows it, and its puffs.
#[doc(alias = "grStory_801E33E0")]
pub fn proc_randall(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    puff(r, gobj);
}

/// Randall's puff, `grStory_801E366C`: when its timer runs out, a cloud effect on its joint
/// and a new wait of 10 to 29 ticks.
#[doc(alias = "grStory_801E366C")]
pub fn puff(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.story_randall();
    let t = v.timer;
    v.timer = t.wrapping_sub(1);
    if t >= 0 {
        return;
    }
    if let Some(j) = v.jobj {
        grlib::generator_oriented_on(r.game, r.data, 0x2C, 0, Some(j));
        let t = r.game.objs.rng.below(20).wrapping_add(10);
        r.game.grounds[g.0].u.story_randall().timer = t as i16;
    }
}

/// The stage's shadow check, `grStory_801E36D8`: a fighter casts on a joint below a point
/// one unit above its feet.
#[doc(alias = "grStory_801E36D8")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    fp::fadds(1.0, reg(pos.y)) > reg(b.y)
}
