//! Battlefield, `melee/gr/grbattle.c`: a main platform with three floating ones, and a sky
//! that changes every 40 to 60 seconds, the new background fading in over the old.
//!
//! The background object (map 3) runs the change in its `grNBa_BG_Vars`: it waits out a
//! random timer, plays its own transition, then creates one of the three backgrounds (maps 1,
//! 2 and 4) other than the current one and cross-fades the two with color scripts, destroying
//! the old one when its fade ends.

use super::super::state::{Game, Running};
use super::anime;
use super::data::{BATTLE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, update_map_coll};
use super::{grlib, material, setup, zako};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// The backgrounds the sky changes between, by map id.
const BACKGROUNDS: [i32; 3] = [1, 2, 4];

/// The background object's states.
const WAITING: i32 = 0;
const TRANSITIONING: i32 = 1;
const DONE: i32 = 2;

/// A stage object's process or callback, by address: Battlefield's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_A114 | 0x8021_A118 | 0x8021_A198 | 0x8021_A204 | 0x8021_A208 | 0x8021_A26C
        | 0x8021_A270 | 0x8021_A2D4 | 0x8021_A2D8 | 0x8021_A33C | 0x8021_A340 | 0x8021_A60C => {
            empty(r, gobj)
        }
        0x8021_9E6C => init_main(r, gobj),
        0x8021_A11C => init_floor(r, gobj),
        0x8021_A174 => proc_floor(r, gobj),
        0x8021_A19C => init_plain(r, gobj),
        0x8021_A20C | 0x8021_A274 | 0x8021_A2DC => init_background(r, gobj),
        0x8021_A344 => init_sky(r, gobj),
        0x8021_A3BC => proc_sky(r, gobj),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing (`grBattle_GObj0_Callback2`
/// and friends).
#[doc(alias = "grBattle_GObj0_Callback2")]
#[doc(alias = "grBattle_GObj0_Callback3")]
#[doc(alias = "grBattle_GObj6_Callback3")]
#[doc(alias = "grBattle_GObj5_Callback2")]
#[doc(alias = "grBattle_GObj5_Callback3")]
#[doc(alias = "grBattle_GObj1_Callback2")]
#[doc(alias = "grBattle_GObj1_Callback3")]
#[doc(alias = "grBattle_GObj2_Callback2")]
#[doc(alias = "grBattle_GObj2_Callback3")]
#[doc(alias = "grBattle_GObj4_Callback2")]
#[doc(alias = "grBattle_GObj4_Callback3")]
#[doc(alias = "grBattle_BG_Callback3")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Where a new background's call puts its scale joint's descriptor on the stack
/// (`setup::SETUP_FRAME`).
const BG_FRAME: u32 = 0x804E_E9F0;

/// Creates one of the stage's objects and sets it up, `grBattle_80219D84`.
#[doc(alias = "grBattle_80219D84")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grNBa_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &BATTLE_CALLBACKS[map_id as usize]
}

/// Battlefield's setup, `grBattle_OnInit`: its color scripts, the main object, the sky and
/// first background (in single-player modes and two event stages, the plain background map 5
/// instead), the floor, camera limits and blast zones, and the players' start points.
///
/// Unchecked: creating a background runs its animation, which spawns particles (the particle
/// system isn't ported), so no capture of it can run to the end yet.
pub fn on_init(r: &mut Running) {
    r.game.gr.battle_yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = true;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    // `gm_8016B3D8`: Classic, Adventure and All-Star.
    let single = (3..=5).contains(&r.game.gr.game_mode);
    if single || r.game.gr.stkind == 273 || r.game.gr.stkind == 245 {
        create(r, 5, setup::SETUP_FRAME);
    } else {
        create(r, 3, setup::SETUP_FRAME);
        create(r, 1, setup::SETUP_FRAME);
    }
    create(r, 6, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    grlib::measure_start_points(r.game, r.data);
    r.game.gr.battle_demo = false;
}

/// `grBattle_OnDemoInit`: the stage runs a demo fight.
#[doc(alias = "grBattle_OnDemoInit")]
pub fn on_demo_init(game: &mut Game) {
    game.gr.battle_demo = true;
}

/// `grBattle_OnLoad` does nothing; `grBattle_80219D7C` (`callback4`) answers false and nothing
/// calls it.
#[doc(alias = "grBattle_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Battlefield's start, `grBattle_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grBattle_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The main object's setup, `grBattle_GObj0_Callback0`: its animation, and in a demo fight
/// the players' start points moved to where the demo starts them.
#[doc(alias = "grBattle_GObj0_Callback0")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let map_id = r.game.grounds[ground_of(r.game, gobj).0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    if r.game.gr.battle_demo {
        for i in 0..4 {
            let v = grlib::start_point(r.game, i);
            let j = r.game.stage_info.x280[i as usize].expect("HSD_JObjSetTranslate: jobj");
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate = v);
        }
    }
}

/// The floor's setup (map 6), `grBattle_GObj6_Callback0`: its collision and animation (the
/// inline `Ground_InitMapCollAndAnim`).
#[doc(alias = "grBattle_GObj6_Callback0")]
pub fn init_floor(r: &mut Running, gobj: GObjId) {
    let map_id = r.game.grounds[ground_of(r.game, gobj).0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    anime::start_map(r.game, r.data, gobj, map_id, 0);
}

/// The floor's process, `grBattle_GObj6_Callback2`: the collision follows the model.
#[doc(alias = "grBattle_GObj6_Callback2")]
pub fn proc_floor(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    super::ground::quake_list_tick(r.game);
}

/// The plain background's setup (map 5), `grBattle_GObj5_Callback0`.
///
/// Unchecked: map 5's animation spawns particles too (see [`on_init`]).
pub fn init_plain(r: &mut Running, gobj: GObjId) {
    init_floor(r, gobj);
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// A background's setup (maps 1, 2 and 4: `grBattle_GObj1_Callback0`, `GObj2_Callback0`,
/// `GObj4_Callback0`): its animation and the stage class for its materials.
///
/// Unchecked, for the same reason as [`on_init`].
pub fn init_background(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    material::model_to_stage_class(r.game, j);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The sky's setup (map 3), `grBattle_BG_Callback0`: hidden, waiting 40 to 60 seconds.
#[doc(alias = "grBattle_BG_Callback0")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 2;
    let timer = r.game.objs.rng.below(1200) + 2400;
    let v = r.game.grounds[g.0].u.battle_bg();
    v.state = WAITING;
    v.timer = timer;
    v.curr = -1;
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
}

/// The sky's process, `grBattle_BG_Callback2`: the wait, the transition (then a random other
/// background fades in over the current one), and the old background's end.
#[doc(alias = "grBattle_BG_Callback2")]
pub fn proc_sky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let v = r.game.grounds[g.0].u.battle_bg();
    match v.state {
        WAITING => {
            let t = v.timer;
            v.timer = t.wrapping_sub(1);
            if t < 0 {
                v.state = TRANSITIONING;
                anime::start_map(r.game, r.data, gobj, map_id, 0);
            }
        }
        TRANSITIONING => {
            if jobj::get_flags(&r.game.objs.jobjs, j) & jobj::HIDDEN != 0 {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            }
            if !anime::finished(r.game, gobj, 0, 7) {
                return;
            }
            if r.game.grounds[g.0].u.battle_bg().curr == -1 {
                let first = BACKGROUNDS
                    .iter()
                    .copied()
                    .find(|&b| map_gobj(r.game, b).is_some())
                    .expect("grbattle.c: i<BATTLE_BG_MAX");
                r.game.grounds[g.0].u.battle_bg().curr = first;
            }
            let prev = r.game.grounds[g.0].u.battle_bg().curr;
            r.game.grounds[g.0].u.battle_bg().prev = prev;
            let curr = loop {
                let b = BACKGROUNDS[r.game.objs.rng.below(3) as usize];
                r.game.grounds[g.0].u.battle_bg().curr = b;
                if b != prev {
                    break b;
                }
            };
            let yaku = r.game.gr.battle_yakumono.expect("grBattle yakumono_param");
            let old = map_gobj(r.game, prev).expect("grbattle.c: bg_gobj");
            let fade_out = r.data.ptr(yaku.at(4));
            material::start_script(r.game, r.data, old, fade_out, 0);
            let new = create(r, curr, BG_FRAME).expect("grbattle.c: bg_gobj");
            let fade_in = r.data.ptr(yaku);
            material::start_script(r.game, r.data, new, fade_in, 0);
            r.game.grounds[g.0].u.battle_bg().state = DONE;
        }
        DONE => {
            let prev = v.prev;
            let old = map_gobj(r.game, prev).expect("grbattle.c: bg_gobj");
            if grlib::overlay_done(r.game, old) {
                setup::destroy(r, Some(old));
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                let timer = r.game.objs.rng.below(1200) + 2400;
                let v = r.game.grounds[g.0].u.battle_bg();
                v.state = WAITING;
                v.timer = timer;
            }
        }
        _ => {}
    }
}
