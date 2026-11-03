//! The Underground Maze, `melee/gr/grshrineroute.c` (stage kind 63, ground kind 32, Adventure's
//! "Underground Maze"): the maze (map 4) with its collision, its pit and the backdrop's three
//! pieces that follow the camera, a background (map 2) and a backdrop (map 0), six symbols on the
//! maze's points 0xBD to 0xC2 (maps 1 and 3), one of them, drawn at the match's start, the way
//! out (map 3), the arena's portal (map 5) and the lights (map 6).
//!
//! The camera follows the first fighter. Both of player 1's climbers standing on a symbol's
//! floor (collision joints 8 to 13) inside its point's box takes it: the way out ends the match
//! as a bonus stage's end; another symbol shuts the maze (its collision joints 0 to 7 off, the
//! symbols' floors but the taken one's hidden), opens the portal there, and once its color
//! script ends starts the symbol's arena (`gm_801674C4`, the 1P setups' match event, which
//! brings the arena's opponents in). The arena won, the maze opens again and that symbol can't
//! be taken twice.
//!
//! The maze lights the fighters itself, as the race does: fifteen of the fighters' lights set
//! from a table (ten point lights and five spot lights), the six nearest the first fighter
//! shown, a hidden light added that follows the fighter and a specular one that takes the
//! second nearest's place and color.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{Game, Obj, Running};
use super::data::{SHRINEROUTE_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, map_gobj, nth_joint, p1_fighter, point, point_joint, scale, set_cam_offset,
};
use super::{CollUser, anime, grlib, material, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::lobj::{self, LObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_80208A38`, the maze's start hook.
const START_HOOK: u32 = 0x8020_8A38;
/// `grShrineRoute_80208F14`, the watcher's check of a symbol's point.
pub const SYMBOL_CHECK: u32 = 0x8020_8F14;
/// `onJointCollision`, a symbol's floor's collision callback.
pub const SYMBOL_TOUCHED: u32 = 0x8020_AD58;
/// `grShrineRoute_8020AE08`, the pit's fighter device.
pub const PIT_DEVICE: u32 = 0x8020_AE08;
/// `grShrineRoute_8020AD24`, an arena's event callback.
const ARENA_EVENT: u32 = 0x8020_AD24;
/// `grShrineRoute_OnCheckShadowRender`, the stage's shadow check.
pub const CHECK_SHADOW_RENDER: u32 = 0x8020_B10C;

/// The maze's map id.
const MAP_MAZE: i32 = 4;
/// The first symbol's point (`0xBD`); the six are 0xBD to 0xC2.
const FIRST_SYMBOL: i32 = 0xBD;

/// A stage object's process or callback, by address: the maze's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_89E0 | 0x8020_89E4 | 0x8020_8A30 | 0x8020_8A34 | 0x8020_9AEC | 0x8020_A100
        | 0x8020_A864 | 0x8020_A89C | 0x8020_A8A0 => empty(r, gobj),
        0x8020_89AC | 0x8020_A868 => start_anim(r, gobj),
        0x8020_89E8 => init_back(r, gobj),
        0x8020_8D14 => init_maze(r, gobj),
        0x8020_8F70 => proc_maze(r, gobj),
        START_HOOK => place_symbols(r, gobj),
        0x8020_9AF0 => init_portal(r, gobj),
        0x8020_9BEC => proc_portal(r, gobj),
        0x8020_A104 => init_lights(r, gobj),
        0x8020_A21C => proc_lights(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_87BC => on_init(r),
        0x8020_882C => on_load(r),
        0x8020_8850 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grShrineRoute_80208A30")]
#[doc(alias = "grShrineRoute_8020A89C")]
#[doc(alias = "grShrineRoute_802089E0")]
// Ports grShrineRoute_802089E4; aliased once a check reaches it.
// Ports grShrineRoute_80208A34; aliased once a check reaches it.
// Ports grShrineRoute_80209AEC; aliased once a check reaches it.
// Ports grShrineRoute_8020A100; aliased once a check reaches it.
// Ports grShrineRoute_8020A864; aliased once a check reaches it.
// Ports grShrineRoute_8020A8A0; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// `grShrineRoute_OnDemoInit` does nothing (only a demo runs it).
// Ports grShrineRoute_OnDemoInit; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// The stage's line-touch dynamics, `grShrineRoute_OnTouchLine`: none.
#[doc(alias = "grShrineRoute_OnTouchLine")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stage's objects' setup functions and processes, by map id (`stage_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &SHRINEROUTE_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grShrineRoute_802088C0`, its scale
/// joint's descriptor at `frame` on the stack.
#[doc(alias = "grShrineRoute_802088C0")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The scale joint's descriptor of the lights' object, which the load hook makes, as the
/// sessions' traces show it: the ID table's entry under it holds the lights' scale joint
/// (`m6-oracle/meikyu`, `m6-underground-maze/mz-arena`).
const LOAD_FRAME: u32 = 0x804E_EA14;
/// The symbols' scale joints' descriptors (the start hook's), as the traces show it: the ID
/// table's entry under it holds the last symbol's scale joint, and the portal's build from the
/// maze's process (`PROC_FRAME`) lands on the same address, so it takes that entry over
/// rather than a new one (`m6-underground-maze/mz-arena`, tick 2824).
const HOOK_FRAME: u32 = 0x804E_E970;
/// The portal's scale joint's descriptor: below the process loop by the maze's process's frame
/// (0xC0) and the create function's (0x20).
const PROC_FRAME: u32 = super::corneria::PROC_LOOP - 0xC0 - 0x20;

/// `grShrineRoute_YakumonoParam`.
fn param(game: &Game) -> DataRef {
    game.gr
        .shrineroute_yakumono
        .expect("grshrineroute.c: yakumono_param")
}

/// One of the parameters' color scripts (`x0` to `xC`).
fn script(r: &Running, off: u32) -> Option<DataRef> {
    r.data.ptr(param(r.game).at(off))
}

/// The Underground Maze's setup, `grShrineRoute_OnInit`: its parameters, the stage's `unk8C`
/// flags (b4 and b5 off), maps 0, 4 and 2 in that order, the camera limits and blast zones
/// measured, and the symbols' lines off.
#[doc(alias = "grShrineRoute_OnInit")]
pub fn on_init(r: &mut Running) {
    r.game.gr.shrineroute_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = false;
    for map_id in [0, 4, 2] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    lines_off(r.game);
}

/// The maze's load, `grShrineRoute_OnLoad`: the lights' object (map 6).
#[doc(alias = "grShrineRoute_OnLoad")]
pub fn on_load(r: &mut Running) {
    create(r, 6, LOAD_FRAME);
}

/// The maze's start, `grShrineRoute_OnStart`: the enemy spawner with the parameters' table
/// (`x28`), then, by a draw of one in the parameters' `x24` (none for 0, always taken), a
/// trophy slot at a random trophy point.
#[doc(alias = "grShrineRoute_OnStart")]
pub fn on_start(r: &mut Running) {
    let p = param(r.game);
    zako::start(r.game, Some(p.at(0x28)));
    let n = r.data.u32(p.at(0x24)) as i32;
    let val = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if val == 0 {
        // The point is drawn before the trophy, as the call's arguments are evaluated.
        let point = super::ground::trophy_point(r.game, r.data);
        let trophy = super::ground::x6e4_drawn(r.game);
        zako::set_trophy_slot(r.game, trophy, point);
    }
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The backdrop's and the symbols' setup (maps 0, 1 and 3; `stageGObj0_OnInit` and
/// `stageGObj1_3_OnInit`, each `Ground_StartMapAnim`): its animation.
#[doc(alias = "stageGObj0_OnInit@802089AC")]
#[doc(alias = "stageGObj1_3_OnInit@8020A868")]
pub fn start_anim(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 2), `grShrineRoute_802089E8`: its animation, its model's
/// materials the stage's.
#[doc(alias = "grShrineRoute_802089E8")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    material::model_to_stage_class(r.game, j);
}

/// Places the six symbols, the maze's start hook `fn_80208A38`: one drawn of six is the way out
/// (map 3), the others symbols (map 1), each made at its point (0xBD on) where the stage has the
/// point, at the parameters' scale (`x14`).
#[doc(alias = "fn_80208A38")]
pub fn place_symbols(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let mut flags = [false; 6];
    let idx = r.game.objs.rng.below(6);
    flags[idx as usize] = true;
    let s = r.data.f32(param(r.game).at(0x14));
    for (i, &exit) in flags.iter().enumerate() {
        let Some(j) = point_joint(r.game, i as i32 + FIRST_SYMBOL) else {
            continue;
        };
        let sym = create(r, if exit { 3 } else { 1 }, HOOK_FRAME);
        r.game.grounds[g.0].u.shrineroute().symbols[i] = sym;
        let pos = world_position(&mut r.game.objs, r.data, Some(j), None);
        if let Some(e) = sym
            && let Some(ej) = jobj_of(r.game, e)
        {
            jobj::set_translate(&mut r.game.objs.jobjs, ej, pos);
            jobj::set_srt(&mut r.game.objs.jobjs, ej, |o| o.scale.x = s);
            jobj::set_srt(&mut r.game.objs.jobjs, ej, |o| o.scale.y = s);
            jobj::set_srt(&mut r.game.objs.jobjs, ej, |o| o.scale.z = s);
        }
    }
}

/// The maze's setup (map 4), `grShrineRoute_80208D14`: the collision linked, its animation, its
/// part 1's materials the stage's (twice), no symbol taken, drawn behind, the zone 0, no
/// symbols yet; the pit as a fighter device of type 3 (`ftCo_800C07F8`), the symbols' floors
/// (collision joints 8 to 13) calling back on a touch, each of points 0x7F to 0x81's joint and
/// its place from point 0x94's (none without that point), and the start hook that places the
/// symbols.
#[doc(alias = "grShrineRoute_80208D14")]
pub fn init_maze(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.geometry_mut().link_all_joints();
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    for _ in 0..2 {
        let j = nth_joint(r.game, gobj, 1);
        material::model_to_stage_class(r.game, j);
    }
    let gp = &mut r.game.grounds[g.0];
    {
        let v = gp.u.shrineroute();
        v.state = 0;
        v.taken = 0;
        v.point = 0;
        v.cam_timer = 0;
        v.portal = None;
        v.touched = 0;
        v.both_touched = 0;
    }
    gp.x11_flags.b012 = 2;
    r.game.stage_info.x6DC = 0;
    r.game.grounds[g.0].u.shrineroute().symbols = [None; 6];
    r.game
        .gr
        .devices
        .register_coll(gobj, 3, CodeRef(PIT_DEVICE));
    for joint in 8..=13 {
        r.game.geometry_mut().set_ledge_callback(
            joint,
            Some(CollUser::Ground(g)),
            Some(CodeRef(SYMBOL_TOUCHED)),
        );
    }
    if let Some(center) = point(r.game, r.data, 0x94) {
        for i in 0..3 {
            let j = point_joint(r.game, i as i32 + 0x7F);
            let offset = j.map(|j| r.game.objs.jobjs[j].translate - center);
            let p = &mut r.game.grounds[g.0].u.shrineroute().platforms[i];
            p.jobj = j;
            if let Some(o) = offset {
                p.offset = o;
            }
        }
    } else {
        for p in &mut r.game.grounds[g.0].u.shrineroute().platforms {
            p.jobj = None;
        }
    }
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
}

/// The watcher's check of a symbol's point, `grShrineRoute_80208F14`: both of player 1's
/// climbers (or player 1 alone) stood on a symbol's floor since the maze's last process.
#[doc(alias = "grShrineRoute_80208F14")]
pub fn symbol_check(game: &mut Game) -> bool {
    let Some(gobj) = map_gobj(game, MAP_MAZE) else {
        return false;
    };
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.shrineroute();
    v.touched != 0 && v.both_touched != 0
}

/// A point the maze reads whether or not the stage has it: every point it reads this way is on
/// the stage, and a missing one would leave a stack vector unwritten.
fn point_of(game: &mut Game, data: &Archives, id: i32) -> Vec3 {
    point(game, data, id).unwrap_or_else(|| panic!("stale read: point {id:#x} of the maze"))
}

/// The maze's collision joints 0 to 7 off (`mpLib_80057BC0`) or on (`mpJointListAdd`).
fn maze_joints(game: &mut Game, on: bool) {
    for id in 0..8 {
        if on {
            game.geometry_mut().enable_joint(id);
        } else {
            game.geometry_mut().disable_joint(id);
        }
    }
}

/// The symbols' floors, by point: each symbol's model part on the maze and its collision joint.
const SYMBOL_FLOORS: [(i32, i32, i32); 6] = [
    (0xBD, 0x13, 8),
    (0xBE, 0x14, 9),
    (0xBF, 0x12, 0xA),
    (0xC0, 0x11, 0xB),
    (0xC1, 0x10, 0xC),
    (0xC2, 0xF, 0xD),
];

/// The maze's process (map 4), `grShrineRoute_80208F70`.
///
/// In the maze (state 0) the watcher looks for the symbol's point player 1 stands in (a box of
/// the parameters' `x1C` by `x20`, in the stage's scale, with [`symbol_check`]); a symbol not
/// taken before takes player 1's input away and, the way out (map 3), ends the match from the
/// next tick (state 6); another starts its arena (state 1): the maze's collision off, the portal
/// (map 5) made hidden at the point, the maze and background's color script `x0`, every other
/// symbol's floor hidden with its collision joint off, the zone 1, the respawn point (point 5)
/// moved to the symbol's (its point less 0xA), the spawner stopped, the other symbols hidden.
/// Once the maze's script ends (state 2) the maze is hidden, its animation stopped, its
/// particles gone and the portal's script `x4` started; once the portal shows and its script
/// ends, the arena's match event starts (state 3). The arena won (`Ground_801C5764` answering 1,
/// both climbers on the floor), the symbol is marked taken, the spawner started, the maze's
/// collision back, the symbols shown, the portal's script `x8` and the zone 0 (state 4); once
/// that ends the portal goes and the maze's script `xC` starts (state 5), which shows the maze
/// again with its floors and collision back, and back to the maze once it ends.
///
/// The camera's target is the first fighter (its centre held while it's in hitstun or dead), or
/// 30 above the symbol in an arena's states; the camera's centre eases toward it by a 1/timer
/// share while the timer runs, else goes straight there. The followers (points 0x7F to 0x81)
/// keep their places from the camera's centre.
#[doc(alias = "grShrineRoute_80208F70")]
pub fn proc_maze(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let player = p1_fighter(r.game);
    let player_pos = match player {
        Some(f) => r.game.ft_pos(f),
        None => Vec3::new(0.0, 0.0, 0.0),
    };
    let mut track_plat = false;
    let state = r.game.grounds[g.0].u.shrineroute().state;
    match state {
        0 => {
            let s = reg(scale(r.game));
            let p = param(r.game);
            let radius = mem(fp::fmuls(reg(r.data.f32(p.at(0x20))), s));
            let w = mem(fp::fmuls(reg(r.data.f32(p.at(0x1C))), s));
            let result =
                super::ground::watch_points2(r.game, Some(CodeRef(SYMBOL_CHECK)), w, radius);
            if result != -1 {
                let ix = result.wrapping_sub(FIRST_SYMBOL);
                let taken = i32::from(r.game.grounds[g.0].u.shrineroute().taken);
                if taken & 1i32.wrapping_shl(ix as u32) == 0 {
                    r.game.grounds[g.0].u.shrineroute().point = result as u16;
                    let sym = r.game.grounds[g.0].u.shrineroute().symbols[ix as usize]
                        .expect("grshrineroute.c:531: gp->u.map.symbol[ix]");
                    let mid = map_id(r.game, sym);
                    if let Some(f) = player {
                        let slot = i32::from(r.game.fighter(f).player_idx);
                        r.player_disable_input(slot);
                    }
                    if mid == 3 {
                        let v = r.game.grounds[g.0].u.shrineroute();
                        v.state = 6;
                        v.xd0 = Some(0x3C);
                    } else {
                        enter_arena(r, gobj, result);
                    }
                }
            }
        }
        1 => {
            track_plat = true;
            if grlib::overlay_done(r.game, gobj) {
                r.game.grounds[g.0].u.shrineroute().state = 2;
                for _ in 0..2 {
                    let j = nth_joint(r.game, gobj, 1);
                    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                }
                if let Some(back) = map_gobj(r.game, 2) {
                    let j = nth_joint(r.game, back, 0);
                    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                }
                anime::set_rate(r.game, gobj, 0, 7, 0.0, true);
                let j = jobj_of(r.game, gobj);
                grlib::kill_tree_generators_and_particles(r.game, r.data, j);
                if let Some(portal) = r.game.grounds[g.0].u.shrineroute().portal {
                    let s = script(r, 4);
                    material::start_script(r.game, r.data, portal, s, 0);
                }
            }
        }
        2 => {
            track_plat = true;
            let portal = r.game.grounds[g.0].u.shrineroute().portal;
            if let Some(portal) = portal
                && let Some(ej) = jobj_of(r.game, portal)
                && r.game.objs.jobjs[ej].flags & jobj::HIDDEN != 0
            {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(ej), jobj::HIDDEN);
                let m = map_id(r.game, portal);
                anime::start_map(r.game, r.data, portal, m, 0);
            }
            if portal.is_none_or(|p| grlib::overlay_done(r.game, p)) {
                r.game.grounds[g.0].u.shrineroute().state = 3;
                super::ground::clear_x6d8(r.game);
                let point = i32::from(r.game.grounds[g.0].u.shrineroute().point);
                r.start_stage_event(6, 1, 1, point.wrapping_sub(0xA) as i8, CodeRef(ARENA_EVENT));
                r.players_enable_input();
                take_symbol(r, gobj, point);
            }
        }
        3 => {
            track_plat = true;
            let v = r.game.grounds[g.0].u.shrineroute();
            let (touched, both) = (v.touched, v.both_touched);
            if super::ground::x6d8(r.game) == 1 && touched != 0 && both != 0 {
                leave_arena(r, gobj);
            }
        }
        4 => {
            let portal = r.game.grounds[g.0].u.shrineroute().portal;
            if portal.is_none_or(|p| grlib::overlay_done(r.game, p)) {
                r.game.grounds[g.0].u.shrineroute().state = 5;
                // The portal's joint is read before the check for it: a maze without its
                // portal would read through null here (docs/melee-bugs.md, latent defects).
                let portal = portal.unwrap_or_else(|| {
                    panic!(
                        "Melee crashes: the Underground Maze's arena ends without its portal \
                         (grShrineRoute_80208F70 reads a null GObj's joint)"
                    )
                });
                let j = jobj_of(r.game, portal);
                grlib::kill_tree_generators_and_particles(r.game, r.data, j);
                setup::destroy(r, Some(portal));
                r.game.grounds[g.0].u.shrineroute().portal = None;
                let s = script(r, 0xC);
                material::start_script(r.game, r.data, gobj, s, 0);
                if let Some(back) = map_gobj(r.game, 2) {
                    material::start_script(r.game, r.data, back, s, 0);
                }
            }
        }
        5 => {
            let j = nth_joint(r.game, gobj, 1);
            if let Some(j19) = j
                && r.game.objs.jobjs[j19].flags & jobj::HIDDEN != 0
            {
                for _ in 0..2 {
                    let j = nth_joint(r.game, gobj, 1);
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                }
                if let Some(back) = map_gobj(r.game, 2) {
                    let j = nth_joint(r.game, back, 0);
                    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                }
                let m = map_id(r.game, gobj);
                anime::start_map(r.game, r.data, gobj, m, 0);
                let pid = i32::from(r.game.grounds[g.0].u.shrineroute().point);
                for &(p, part, joint) in &SYMBOL_FLOORS {
                    if pid != p {
                        let j = nth_joint(r.game, gobj, part);
                        if j.is_some() {
                            jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                        }
                        r.game.geometry_mut().enable_joint(joint);
                    }
                }
                r.game.geometry_mut().link_all_joints();
            } else if j.is_none() {
                panic!("stale read: the maze's part 1");
            }
            if grlib::overlay_done(r.game, gobj) {
                r.game.grounds[g.0].u.shrineroute().state = 0;
            }
        }
        6 => r.game.stage_info.flags |= 0x10,
        _ => {}
    }

    let mut target;
    if !track_plat {
        match player {
            Some(f) if r.game.ft_is_in_hitstun(f) => {
                target = stage::cam_center(r.game);
                r.game.grounds[g.0].u.shrineroute().cam_timer = 0x3C;
            }
            Some(f) if r.game.ft_is_dead(f) => {
                target = stage::cam_center(r.game);
                r.game.grounds[g.0].u.shrineroute().cam_timer = 0;
            }
            Some(_) => target = player_pos,
            None => target = stage::cam_center(r.game),
        }
        super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, true);
    } else {
        let pid = i32::from(r.game.grounds[g.0].u.shrineroute().point);
        target = point_of(r.game, r.data, pid);
        target.y = mem(fp::fadds(reg(target.y), 30.0));
        super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, false);
    }
    let timer = r.game.grounds[g.0].u.shrineroute().cam_timer;
    if timer == 0 {
        set_cam_offset(r.game, target.x, target.y);
    } else {
        let t = fp::fdivs(1.0, fp::frsp(f64::from(timer)));
        let c = stage::cam_center(r.game);
        let mut d = target - c;
        d.x = mem(fp::fmuls(reg(d.x), t));
        d.y = mem(fp::fmuls(reg(d.y), t));
        d.z = mem(fp::fmuls(reg(d.z), t));
        let c = c + d;
        set_cam_offset(r.game, c.x, c.y);
        r.game.grounds[g.0].u.shrineroute().cam_timer = timer.wrapping_sub(1);
    }
    let c = stage::cam_center(r.game);
    for k in 0..3 {
        let p = r.game.grounds[g.0].u.shrineroute().platforms[k];
        if let Some(j) = p.jobj {
            let tx = mem(fp::fadds(reg(p.offset.x), reg(c.x)));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = tx);
            let ty = mem(fp::fadds(reg(p.offset.y), reg(c.y)));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = ty);
        }
    }
    let v = r.game.grounds[g.0].u.shrineroute();
    v.touched = 0;
    v.both_touched = 0;
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// A symbol other than the way out taken (state 0's arena branch of `grShrineRoute_80208F70`).
fn enter_arena(r: &mut Running, gobj: GObjId, result: i32) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.shrineroute();
    v.state = 1;
    v.cam_timer = 0x3C;
    maze_joints(r.game, false);
    let portal = create(r, 5, PROC_FRAME);
    r.game.grounds[g.0].u.shrineroute().portal = portal;
    if let Some(portal) = portal {
        let ej = jobj_of(r.game, portal);
        jobj::set_flags_all(&mut r.game.objs.jobjs, ej, jobj::HIDDEN);
        let at = point_of(r.game, r.data, result);
        jobj::set_translate(&mut r.game.objs.jobjs, ej.expect("the portal's joint"), at);
    }
    let s = script(r, 0);
    material::start_script(r.game, r.data, gobj, s, 0);
    if let Some(back) = map_gobj(r.game, 2) {
        material::start_script(r.game, r.data, back, s, 0);
    }
    for &(p, part, joint) in &SYMBOL_FLOORS {
        if result != p {
            let j = nth_joint(r.game, gobj, part);
            if j.is_some() {
                jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            }
            r.game.geometry_mut().disable_joint(joint);
        }
    }
    r.game.stage_info.x6DC = 1;
    let dst = point_joint(r.game, 5);
    let src = point_joint(r.game, result.wrapping_sub(0xA));
    if let (Some(dst), Some(src)) = (dst, src) {
        let t = r.game.objs.jobjs[src].translate;
        jobj::set_translate(&mut r.game.objs.jobjs, dst, t);
    }
    zako::clear_enemies(r);
    let point = i32::from(r.game.grounds[g.0].u.shrineroute().point);
    show_symbols(r.game, gobj, point, false);
}

/// The arena won (state 3's branch of `grShrineRoute_80208F70`).
// Ports the arena's end of grShrineRoute_80208F70, which only gm_801674C4's arena reaches.
fn leave_arena(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.shrineroute();
    v.taken |= 1u16.wrapping_shl(u32::from(v.point).wrapping_sub(FIRST_SYMBOL as u32));
    zako::set_active(r.game, true);
    show_symbols(r.game, gobj, -1, true);
    r.game.grounds[g.0].u.shrineroute().cam_timer = 0x3C;
    maze_joints(r.game, true);
    lines_off(r.game);
    if let Some(portal) = r.game.grounds[g.0].u.shrineroute().portal {
        let s = script(r, 8);
        material::start_script(r.game, r.data, portal, s, 0);
    }
    r.game.stage_info.x6DC = 0;
    r.game.grounds[g.0].u.shrineroute().state = 4;
}

/// The portal's setup (map 5), `grShrineRoute_80209AF0`: drawn behind, its model's materials
/// the stage's, its part 10 (a spinning piece) placed at random, no sway, and its sway's pushes
/// drawn (each between 1 and 5 degrees a frame², either way).
#[doc(alias = "grShrineRoute_80209AF0")]
pub fn init_portal(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x11_flags.b012 = 2;
    material::model_to_stage_class(r.game, j);
    let spin = nth_joint(r.game, gobj, 10);
    r.game.grounds[g.0].u.shrineroute3().spin = spin;
    place_spin(r, gobj);
    let v = r.game.grounds[g.0].u.shrineroute3();
    v.sway_y = 0.0;
    v.sway_x = 0.0;
    let push = |r: &mut Running| {
        let f = reg(r.game.objs.rng.draw_f32());
        let v = fp::fmadds(SWAY_RANGE, f, SWAY_MIN);
        let sign = if r.game.objs.rng.below(2) != 0 {
            1.0
        } else {
            fp::fneg(1.0)
        };
        (v, sign)
    };
    let (v, sign) = push(r);
    let x = mem(v);
    r.game.grounds[g.0].u.shrineroute3().push_x = mem(fp::fmuls(reg(x), sign));
    let (v, sign) = push(r);
    let y = mem(v);
    r.game.grounds[g.0].u.shrineroute3().push_y = mem(fp::fmuls(reg(y), sign));
}

/// The sway's pushes' range and least (4 and 1 degrees, `0.00006981317`, `0.000017453292`).
const SWAY_RANGE: f64 = 6.981316983001307e-05;
const SWAY_MIN: f64 = 1.7453292457503267e-05;
/// The sway's speed limit (0.05 degrees) and its angles' limits (15 and 10 degrees).
const SWAY_MAX: f64 = 0.0008726646192371845;
const TILT_X: f64 = 0.2617993950843811;
const TILT_Y: f64 = 0.1745329201221466;
/// A turn (`6.283185307179586`).
const TAU: f64 = std::f64::consts::TAU;

/// The absolute value as the original's `ABS`.
fn abs(v: f64) -> f64 {
    if v < fp::frsp(0.0) { fp::fneg(v) } else { v }
}

/// The portal's process (map 5), `grShrineRoute_80209BEC`: it sways about x and y, each speed
/// pushed, damped by 0.99 and kept under 0.05 degrees, each angle kept within 15 (x) and 10 (y)
/// degrees, bouncing back with a new push at the limits; its spinning part turns by its own
/// speeds, wrapped within a turn; and when an animation of its model rewinds, the spinning part
/// is placed again.
#[doc(alias = "grShrineRoute_80209BEC")]
pub fn proc_portal(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("the portal's joint");
    let damp = |v: &mut f32, push: f32| {
        *v = mem(fp::fadds(reg(*v), reg(push)));
        *v = mem(fp::fmuls(reg(*v), 0.9900000095367432));
        if reg(*v) > SWAY_MAX {
            *v = mem(SWAY_MAX);
        } else if reg(*v) < fp::fneg(SWAY_MAX) {
            *v = mem(fp::fneg(SWAY_MAX));
        }
    };
    {
        let v = r.game.grounds[g.0].u.shrineroute3();
        let (px, py) = (v.push_x, v.push_y);
        damp(&mut v.sway_x, px);
        damp(&mut v.sway_y, py);
    }
    let sway_x = r.game.grounds[g.0].u.shrineroute3().sway_x;
    let mut rot = fp::fadds(reg(sway_x), reg(r.game.objs.jobjs[j].rotate.x));
    if rot > TILT_X {
        rot = TILT_X;
        let v = r.game.grounds[g.0].u.shrineroute3();
        v.sway_x = mem(fp::fneg(abs(reg(v.sway_x))));
        let tmp = reg(r.game.objs.rng.draw_f32());
        r.game.grounds[g.0].u.shrineroute3().push_x =
            mem(fp::fmsubs(SWAY_RANGE, fp::fneg(tmp), SWAY_MIN));
    } else if rot < fp::fneg(TILT_X) {
        rot = fp::fneg(TILT_X);
        let v = r.game.grounds[g.0].u.shrineroute3();
        v.sway_x = mem(abs(reg(v.sway_x)));
        let f = reg(r.game.objs.rng.draw_f32());
        r.game.grounds[g.0].u.shrineroute3().push_x = mem(fp::fmadds(SWAY_RANGE, f, SWAY_MIN));
    }
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.rotate.x = mem(rot));
    let sway_y = r.game.grounds[g.0].u.shrineroute3().sway_y;
    let mut rot = fp::fadds(reg(sway_y), reg(r.game.objs.jobjs[j].rotate.y));
    if rot > TILT_Y {
        rot = TILT_Y;
        let v = r.game.grounds[g.0].u.shrineroute3();
        v.sway_y = mem(fp::fneg(abs(reg(v.sway_y))));
        let tmp = reg(r.game.objs.rng.draw_f32());
        r.game.grounds[g.0].u.shrineroute3().push_y =
            mem(fp::fmsubs(SWAY_RANGE, fp::fneg(tmp), SWAY_MIN));
    } else if rot < fp::fneg(TILT_Y) {
        rot = fp::fneg(TILT_Y);
        let v = r.game.grounds[g.0].u.shrineroute3();
        v.sway_y = mem(abs(reg(v.sway_y)));
        let f = reg(r.game.objs.rng.draw_f32());
        r.game.grounds[g.0].u.shrineroute3().push_y = mem(fp::fmadds(SWAY_RANGE, f, SWAY_MIN));
    }
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.rotate.y = mem(rot));
    let v = r.game.grounds[g.0].u.shrineroute3();
    if let Some(spin) = v.spin {
        let (sx, sy) = (v.spin_x, v.spin_y);
        let wrap = |rot: f64| {
            if rot > TAU {
                fp::frsp(fp::fsub(rot, TAU))
            } else if rot < fp::fneg(TAU) {
                fp::frsp(fp::fadd(rot, TAU))
            } else {
                rot
            }
        };
        let rot = wrap(fp::fadds(reg(sx), reg(r.game.objs.jobjs[spin].rotate.x)));
        jobj::set_srt(&mut r.game.objs.jobjs, spin, |o| o.rotate.x = mem(rot));
        let rot = wrap(fp::fadds(reg(sy), reg(r.game.objs.jobjs[spin].rotate.y)));
        jobj::set_srt(&mut r.game.objs.jobjs, spin, |o| o.rotate.y = mem(rot));
    }
    if crate::lb::anim::any_rewound(&r.game.objs, Some(j)) {
        place_spin(r, gobj);
    }
}

/// Places the portal's spinning part, `grShrineRoute_8020A8A4`: 300 from the centre at a random
/// angle, its speeds about x and y drawn within 2.5 degrees a frame either way.
#[doc(alias = "grShrineRoute_8020A8A4")]
pub fn place_spin(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let Some(spin) = r.game.grounds[g.0].u.shrineroute3().spin else {
        return;
    };
    let f = reg(r.game.objs.rng.draw_f32());
    let angle = mem(fp::frsp(fp::fmul(TAU, f)));
    let x = mem(fp::fmuls(300.0, reg(crate::math::cos(angle))));
    jobj::set_srt(&mut r.game.objs.jobjs, spin, |o| o.translate.x = x);
    let y = mem(fp::fmuls(300.0, reg(crate::math::sin(angle))));
    jobj::set_srt(&mut r.game.objs.jobjs, spin, |o| o.translate.y = y);
    let k = 0.04363323003053665;
    let f = reg(r.game.objs.rng.draw_f32());
    r.game.grounds[g.0].u.shrineroute3().spin_x = mem(fp::fmuls(k, fp::fmsubs(2.0, f, 1.0)));
    let f = reg(r.game.objs.rng.draw_f32());
    r.game.grounds[g.0].u.shrineroute3().spin_y = mem(fp::fmuls(k, fp::fmsubs(2.0, f, 1.0)));
}

/// The lights' object (map 6), `grShrineRoute_8020A104`: the fighters' lights (the first GObj
/// of classifier 0xC on the lights' link), fifteen of them set up from the table, each of its
/// lights and its flags kept, the two added lights, and the lights' process run once.
#[doc(alias = "grShrineRoute_8020A104")]
pub fn init_lights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let mut lg = r.game.world.first_in(3);
    while let Some(o) = lg {
        if r.game.world.gobj(o).classifier == 0xC {
            break;
        }
        lg = r.game.world.gobj(o).next;
    }
    let lights = lg.expect("grshrineroute.c:1106: lgobj");
    r.game.grounds[g.0].u.pushon().gobj = Some(lights);
    set_up_lights(r.game, lights);
    let mut cur = first_lobj(r.game, lights);
    r.game.grounds[g.0].u.pushon().count = 0;
    while let Some(l) = cur {
        let flags = lobj::flags(&r.game.objs.lobjs, Some(l));
        let v = r.game.grounds[g.0].u.pushon();
        let n = v.count as usize;
        v.lobjs[n] = Some(l);
        v.lobj_flags[n] = flags;
        cur = r.game.objs.lobjs[l].next;
        r.game.grounds[g.0].u.pushon().count += 1;
    }
    let point = add_light(r.game, lights, &FOLLOWER);
    r.game.grounds[g.0].u.pushon().point_light = point;
    let spec = add_light(r.game, lights, &SECOND);
    r.game.grounds[g.0].u.pushon().spot_light = spec;
    proc_lights(r, gobj);
}

fn first_lobj(game: &Game, gobj: GObjId) -> Option<LObjId> {
    match game.world.gobjs[gobj.0].obj {
        Some(Obj::LObj(l)) => Some(l),
        None => None,
        ref other => panic!("the fighters' lights' GObj holds {other:?}"),
    }
}

/// One of the maze's lights (`grSh_Route_LightConfig`): its color, place, interest (a spot
/// light's), whether it's a spot light, a spot light's cutoff and function, its reference
/// brightness and distance (its distance function is steep for all).
struct LightConfig {
    color: [u8; 4],
    pos: [f32; 3],
    interest: [f32; 3],
    spot: bool,
    cutoff: f32,
    ref_br: f32,
    ref_dist: f32,
}

const fn point_light(color: [u8; 4], pos: [f32; 3], ref_br: f32, ref_dist: f32) -> LightConfig {
    LightConfig {
        color,
        pos,
        interest: [0.0; 3],
        spot: false,
        cutoff: 0.0,
        ref_br,
        ref_dist,
    }
}

const fn spot_light(
    color: [u8; 4],
    pos: [f32; 3],
    interest: [f32; 3],
    cutoff: f32,
    ref_dist: f32,
) -> LightConfig {
    LightConfig {
        color,
        pos,
        interest,
        spot: true,
        cutoff,
        ref_br: 0.99,
        ref_dist,
    }
}

/// `light_config`.
const LIGHT_CONFIGS: [LightConfig; 15] = [
    point_light([0x80, 0x66, 0x4D, 0xFF], [790.0, -320.0, 70.0], 0.01, 800.0),
    point_light([0x80, 0x80, 0x80, 0xFF], [510.0, -10.0, 70.0], 0.01, 500.0),
    point_light([0xB3, 0x99, 0x66, 0xFF], [840.0, 210.0, 40.0], 0.1, 260.0),
    point_light([0xFF, 0xCC, 0x66, 0xFF], [600.0, 120.0, 30.0], 0.01, 500.0),
    point_light([0x80, 0x80, 0x80, 0xFF], [800.0, -130.0, 28.0], 0.01, 400.0),
    point_light([0xCC, 0x99, 0xA5, 0xFF], [710.0, -50.0, 20.0], 0.01, 500.0),
    point_light([0x90, 0xC2, 0xFF, 0xFF], [10.0, 18.0, 9.0], 0.03, 810.0),
    point_light(
        [0xFF, 0x82, 0x82, 0xFF],
        [-260.0, -220.0, 20.0],
        0.03,
        300.0,
    ),
    point_light([0x78, 0xB4, 0xFF, 0xFF], [244.0, -288.0, 22.0], 0.03, 300.0),
    point_light([0xF0, 0xF0, 0xF0, 0xFF], [-66.0, 140.0, 27.0], 0.03, 400.0),
    spot_light(
        [0x80, 0x99, 0x66, 0xFF],
        [500.0, -150.0, 60.0],
        [500.0, -330.0, -20.0],
        30.0,
        200.0,
    ),
    spot_light(
        [0xC8, 0xF0, 0xFF, 0xFF],
        [560.0, 310.0, 160.0],
        [560.0, 290.0, -10.0],
        45.0,
        24.0,
    ),
    spot_light(
        [0xA4, 0xD8, 0xFF, 0xFF],
        [260.0, 444.0, 88.0],
        [242.0, -166.0, -33.0],
        25.0,
        450.0,
    ),
    spot_light(
        [0xFF, 0x4D, 0x4D, 0xFF],
        [-162.0, -300.0, 23.0],
        [-314.0, 111.0, -5.0],
        31.0,
        370.0,
    ),
    spot_light(
        [0x64, 0x80, 0x4D, 0xFF],
        [-88.0, -131.0, 66.0],
        [-91.0, -227.0, -25.0],
        45.0,
        125.0,
    ),
];

/// The maze's lights, `grShrineRoute_8020AA40`: the fifteen lights after the set's first, each
/// given the table's color, place and distance falloff, then made a diffuse point light, or a
/// diffuse spot light with the table's interest and spot. A set of fewer lights leaves the rest
/// of the table unused (with a report).
#[doc(alias = "grShrineRoute_8020AA40")]
pub fn set_up_lights(game: &mut Game, gobj: GObjId) {
    let first = first_lobj(game, gobj).expect("grshrineroute.c: the fighters' lights");
    let mut l = game.objs.lobjs[first].next;
    for e in &LIGHT_CONFIGS {
        let Some(o) = l else {
            // "oioi..."
            continue;
        };
        lobj::set_color(&mut game.objs.lobjs, o, e.color);
        lobj::set_position(&mut game.objs, o, Vec3::new(e.pos[0], e.pos[1], e.pos[2]));
        lobj::set_dist_attn(
            &mut game.objs.lobjs,
            Some(o),
            e.ref_dist,
            e.ref_br,
            lobj::GX_DA_STEEP as i32,
        );
        if e.spot {
            game.objs.lobjs[o].flags = lobj::SPOT | lobj::DIFFUSE;
            let at = Vec3::new(e.interest[0], e.interest[1], e.interest[2]);
            lobj::set_interest(&mut game.objs, o, at);
            lobj::set_spot(
                &mut game.objs.lobjs,
                Some(o),
                e.cutoff,
                lobj::GX_SP_COS2 as i32,
            );
        } else {
            game.objs.lobjs[o].flags = lobj::POINT | lobj::DIFFUSE;
        }
        l = game.objs.lobjs[o].next;
    }
}

/// One of the two lights the maze adds (`lobj0`, `lobj1`): its flags, whether it starts hidden,
/// and its point parameters as `HSD_LObjLoadDesc` reads them. Each descriptor's parameters point
/// at a lone float, 16 (`lobj0_shininess` at 0x804D4824, `lobj1_shininess` at 0x804D4838), and
/// the load reads an `HSD_LightPointDesc` there: that float as the reference brightness, then
/// the two words after it in the executable's small data as the reference distance and the
/// distance function, as the race's lights do (docs/melee-bugs.md).
struct AddedLight {
    flags: u16,
    hidden: bool,
    ref_br: u32,
    ref_dist: u32,
    dist_func: u32,
}

/// `lobj0`: a diffuse point light, hidden, which follows the first fighter.
const FOLLOWER: AddedLight = AddedLight {
    flags: lobj::POINT | lobj::DIFFUSE,
    hidden: true,
    ref_br: 0x4180_0000,
    // "lobj", ".h\0\0" (0x804D4828, the string "lobj.h").
    ref_dist: 0x6C6F_626A,
    dist_func: 0x2E68_0000,
};

/// `lobj1`: a specular point light, which takes the second nearest light's place.
const SECOND: AddedLight = AddedLight {
    flags: lobj::POINT | lobj::SPECULAR,
    hidden: false,
    ref_br: 0x4180_0000,
    // 0, "gobj" (0x804D483C, the padding before the string "gobj").
    ref_dist: 0,
    dist_func: 0x676F_626A,
};

/// Adds a light at the end of the set's chain, `grShrineRoute_8020AB58` and
/// `grShrineRoute_8020AC44`: the descriptor loaded (white, no position), placed at (0, 100, 0)
/// and, the first, hidden.
#[doc(alias = "grShrineRoute_8020AB58")]
#[doc(alias = "grShrineRoute_8020AC44")]
fn add_light(game: &mut Game, gobj: GObjId, desc: &AddedLight) -> Option<LObjId> {
    let mut last = first_lobj(game, gobj)?;
    while let Some(n) = game.objs.lobjs[last].next {
        last = n;
    }
    let l = lobj::load_static_point(
        &mut game.objs,
        desc.flags,
        [0xFF; 4],
        [desc.ref_br, desc.ref_dist, desc.dist_func],
    );
    game.objs.lobjs[last].next = Some(l);
    lobj::set_position(&mut game.objs, l, Vec3::new(0.0, 100.0, 0.0));
    if desc.hidden {
        lobj::set_flags(&mut game.objs.lobjs, Some(l), u32::from(lobj::HIDDEN));
    }
    Some(l)
}

/// A light's place, for the lights' process: every light it reads has one (a light without
/// would leave a stack vector unwritten).
fn position_of(game: &mut Game, data: &Archives, l: LObjId) -> Vec3 {
    lobj::position(&mut game.objs, data, Some(l))
        .unwrap_or_else(|| panic!("stale read: the position of light {l:?}"))
}

/// The lights' process (map 6), `grShrineRoute_8020A21C`.
///
/// Each kept light's distance from the first fighter (in x and y; a spot light's in x alone; an
/// ambient light -100), at least 10, sorts the lights nearest first; the six nearest are shown
/// and the rest hidden. The hidden follower moves 20 from the first fighter toward the lights
/// past the nearest (each weighed by the second nearest's distance over its own, less when
/// under 15 above the fighter, none below it), looking at the fighter. The specular light takes
/// the second nearest's place, interest and flags (made specular, not diffuse) and its color,
/// dimmed while the third nearest is within 50 of it.
#[doc(alias = "grShrineRoute_8020A21C")]
pub fn proc_lights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let player = match p1_fighter(r.game) {
        Some(f) => r.game.ft_pos(f),
        None => Vec3::new(0.0, 0.0, 0.0),
    };
    if r.game.grounds[g.0].u.pushon().gobj.is_none() {
        return;
    }
    let v = r.game.grounds[g.0].u.pushon();
    let count = v.count;
    let lobjs = v.lobjs;
    let n = count.max(0) as usize;
    let light = |i: usize| lobjs[i].expect("grshrineroute.c: a kept light");
    let mut distances = [0.0f32; 20];
    let mut sorted = [0usize; 20];
    for i in 0..n {
        let l = light(i);
        let kind = r.game.objs.lobjs[l].flags & 3;
        distances[i] = match kind {
            0 => mem(fp::fneg(100.0)),
            1 | 2 => {
                let p = position_of(r.game, r.data, l);
                let dx = fp::fsubs(reg(player.x), reg(p.x));
                let dy = fp::fsubs(reg(player.y), reg(p.y));
                mem(crate::math::sqrt_reg(fp::fadds(
                    fp::fmuls(dx, dx),
                    fp::fmuls(dy, dy),
                )))
            }
            _ => {
                let p = position_of(r.game, r.data, l);
                let d = fp::fsubs(reg(player.x), reg(p.x));
                mem(if d < fp::frsp(0.0) { fp::fneg(d) } else { d })
            }
        };
        if reg(distances[i]) < 10.0 {
            distances[i] = 10.0;
        }
        sorted[i] = i;
    }
    for i in 0..n {
        let mut min_dist = f64::from(f32::MAX);
        let mut best = 0;
        for j in i..n {
            if min_dist > reg(distances[sorted[j]]) {
                best = j;
                min_dist = reg(distances[sorted[j]]);
            }
        }
        sorted.swap(i, best);
    }
    // The second and third nearest are read whatever the count; the maze's set has more than
    // three lights.
    assert!(
        n > 2,
        "grshrineroute.c: {n} lights, the third nearest a stack word"
    );
    for (i, &s) in sorted.iter().enumerate().take(n) {
        if i < 6 {
            lobj::clear_flags(
                &mut r.game.objs.lobjs,
                Some(light(s)),
                u32::from(lobj::HIDDEN),
            );
        } else {
            lobj::set_flags(
                &mut r.game.objs.lobjs,
                Some(light(s)),
                u32::from(lobj::HIDDEN),
            );
        }
    }
    let mut dir = Vec3::new(0.0, 0.0, 0.0);
    let ref_dist = reg(distances[sorted[1]]);
    for &s in sorted.iter().take(n).skip(1) {
        let Some(mut lp) = lobj::position(&mut r.game.objs, r.data, Some(light(s))) else {
            continue;
        };
        let mut weight = fp::fdivs(ref_dist, reg(distances[s]));
        lp = lp - player;
        if reg(lp.y) < 15.0 {
            if reg(lp.y) < 0.0 {
                weight = 0.0;
            } else {
                weight = fp::fmuls(weight, fp::fdivs(reg(lp.y), 15.0));
            }
        }
        lp.normalize();
        lp.x = mem(fp::fmuls(reg(lp.x), weight));
        lp.y = mem(fp::fmuls(reg(lp.y), weight));
        lp.z = mem(fp::fmuls(reg(lp.z), weight));
        dir = dir + lp;
    }
    let sq = |a: f32| fp::fmuls(reg(a), reg(a));
    let len = crate::math::sqrt_reg(fp::fadds(sq(dir.z), fp::fadds(sq(dir.x), sq(dir.y))));
    if len < reg(0.01) {
        dir.y = 100.0;
    }
    dir.normalize();
    dir.x = mem(fp::fmuls(reg(dir.x), 20.0));
    dir.y = mem(fp::fmuls(reg(dir.y), 20.0));
    dir.z = mem(fp::fmuls(reg(dir.z), 20.0));
    dir = dir + player;
    let v = r.game.grounds[g.0].u.pushon();
    let (point, spec) = (v.point_light, v.spot_light);
    let point = point.expect("grshrineroute.c: the follower");
    lobj::set_position(&mut r.game.objs, point, dir);
    lobj::set_interest(&mut r.game.objs, point, player);
    lobj::set_flags(&mut r.game.objs.lobjs, Some(point), 0x400);
    let second = light(sorted[1]);
    let dist_diff = fp::fsubs(reg(distances[sorted[2]]), reg(distances[sorted[1]]));
    let mut color = lobj::color(&r.game.objs.lobjs, second);
    if fp::fsubs(reg(distances[sorted[2]]), reg(distances[sorted[1]])) < 50.0 {
        let ratio = fp::fdivs(dist_diff, 50.0);
        for c in &mut color[..3] {
            *c = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(*c)), ratio)) as u8;
        }
    }
    let spec = spec.expect("grshrineroute.c: the specular light");
    lobj::set_color(&mut r.game.objs.lobjs, spec, color);
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let at = lobj::position(&mut r.game.objs, r.data, Some(second)).unwrap_or(zero);
    lobj::set_position(&mut r.game.objs, spec, at);
    let at = lobj::interest(&mut r.game.objs, r.data, Some(second)).unwrap_or(zero);
    lobj::set_interest(&mut r.game.objs, spec, at);
    let flags = r.game.objs.lobjs[second].flags;
    r.game.objs.lobjs[spec].flags = flags & !lobj::DIFFUSE | lobj::SPECULAR;
}

/// An arena's event callback, `grShrineRoute_8020AD24`: on 1, the stage's `x6D8` set to 1,
/// answering true.
// Ports grShrineRoute_8020AD24; aliased once a check reaches it.
pub fn arena_event(game: &mut Game, arg: i32) -> bool {
    if arg == 1 {
        super::ground::set_x6d8(game, arg);
        return true;
    }
    false
}

/// A symbol's floor touched in a collision pass (`onJointCollision`), applied once mpcoll
/// returns (the joint callback has no access to the players): the touching collisions' owners,
/// in order. Its only effects are the maze's two flags, which nothing in a pass reads, so they
/// happen in the original's order.
#[derive(Debug, Default)]
pub struct SymbolTouch(Vec<(super::super::handles::GroundId, Option<GObjId>)>);

impl SymbolTouch {
    /// A fighter's collision (`x34_flags.b1234` 1) standing on a symbol's floor (ground kind 2)
    /// of the maze object `g` (the joint's user data).
    pub fn touched(
        &mut self,
        g: super::super::handles::GroundId,
        coll_kind: u8,
        owner: Option<GObjId>,
        kind: i32,
    ) {
        if coll_kind == 1 && kind == 2 {
            self.0.push((g, owner));
        }
    }

    /// `onJointCollision`: the player's main fighter touching marks the maze's `xCA`, and
    /// `xCC` too unless the player has a partner awake (Nana); the partner touching marks
    /// `xCC`.
    #[doc(alias = "onJointCollision@8020AD58")]
    pub fn apply(self, game: &mut Game) {
        for (g, owner) in self.0 {
            let owner = owner.expect("a symbol's floor touched by no fighter");
            let slot = i32::from(game.fighter(owner).player_idx);
            if Some(owner) == game.player_entity(slot) {
                game.grounds[g.0].u.shrineroute().touched = 1;
                if let Some(partner) = game.player_entity_at(slot, 1)
                    && !game.ft_is_sleeping(partner)
                {
                    continue;
                }
                game.grounds[g.0].u.shrineroute().both_touched = 1;
                continue;
            }
            game.grounds[g.0].u.shrineroute().both_touched = 1;
        }
    }
}

/// The pit's fighter device, `grShrineRoute_8020AE08`: outside an arena's entry (states 1 and
/// 3), a fighter below -226.1 between x -285.93 and -161.56 (in the stage's scale) takes the
/// parameters' hit (`x10`).
#[doc(alias = "grShrineRoute_8020AE08")]
pub fn pit_device(r: &mut Running, fighter: GObjId) -> Option<DataRef> {
    let gobj = r.game.gr.devices.device2.expect("the pit's object");
    let g = ground_of(r.game, gobj);
    let state = r.game.grounds[g.0].u.shrineroute().state;
    if state == 1 || state == 3 {
        return None;
    }
    let s = reg(scale(r.game));
    let k = |v: f32| mem(fp::fmuls(reg(v), s));
    let (lo_x, lo_y) = (k(-285.93), k(-226.1));
    let hi_x = k(-161.56);
    let pos = r.game.ft_pos(fighter);
    if reg(pos.y) < reg(lo_y) && reg(lo_x) < reg(pos.x) && reg(pos.x) < reg(hi_x) {
        let hit = r.data.ptr(param(r.game).at(0x10));
        return Some(hit.expect("grshrineroute.c: the pit's hit"));
    }
    None
}

/// The symbol taken leaves for its arena, `grShrineRoute_8020AF38`: an effect (0x428) at its
/// point at 0.7 the symbol's scale (`x14`, or the way out's `x18`), the symbol gone and the
/// first fighter's rumble.
// Ports grShrineRoute_8020AF38; aliased once a check reaches it.
pub fn take_symbol(r: &mut Running, gobj: GObjId, point: i32) {
    let g = ground_of(r.game, gobj);
    let ix = point.wrapping_sub(FIRST_SYMBOL) as usize;
    let player = p1_fighter(r.game);
    let Some(sym) = r.game.grounds[g.0].u.shrineroute().symbols[ix] else {
        return;
    };
    let p = param(r.game);
    let k = if map_id(r.game, sym) == 1 { 0x14 } else { 0x18 };
    let scale = mem(fp::fmuls(reg(0.7), reg(r.data.f32(p.at(k)))));
    let j = point_joint(r.game, point);
    super::super::ef::spawn(
        r,
        0x428,
        Some(gobj),
        &[
            super::super::ef::Arg::JObj(j),
            super::super::ef::Arg::Float(scale),
        ],
    );
    setup::destroy(r, Some(sym));
    r.game.grounds[g.0].u.shrineroute().symbols[ix] = None;
    if let Some(f) = player {
        super::super::ft::ftlib::start_rumble(r, f, 0xB, 0x1E);
    }
}

/// Hides or shows the symbols but the one at point `except` (all for -1),
/// `grShrineRoute_8020B020`.
#[doc(alias = "grShrineRoute_8020B020")]
pub fn show_symbols(game: &mut Game, gobj: GObjId, except: i32, show: bool) {
    let g = ground_of(game, gobj);
    let comp = except.wrapping_sub(FIRST_SYMBOL);
    for i in 0..6 {
        if except == -1 || i != comp {
            let Some(sym) = game.grounds[g.0].u.shrineroute().symbols[i as usize] else {
                continue;
            };
            let Some(j) = jobj_of(game, sym) else {
                continue;
            };
            if show {
                jobj::clear_flags_all(&mut game.objs.jobjs, Some(j), jobj::HIDDEN);
            } else {
                jobj::set_flags_all(&mut game.objs.jobjs, Some(j), jobj::HIDDEN);
            }
        }
    }
}

/// The maze's lines the symbols' arenas shut (`grSh_Route_803E58E0`).
const ARENA_LINES: [i32; 7] = [51, 79, 101, 102, 115, 116, 131];

/// The arenas' lines off, `grShrineRoute_8020B0AC`.
#[doc(alias = "grShrineRoute_8020B0AC")]
pub fn lines_off(game: &mut Game) {
    for line in ARENA_LINES {
        game.geometry_mut().disable_line(line);
    }
}

/// The stage's shadow check, `grShrineRoute_OnCheckShadowRender`: a fighter casts on a joint
/// below its feet.
// Ports grShrineRoute_OnCheckShadowRender; aliased once a check reaches it.
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
