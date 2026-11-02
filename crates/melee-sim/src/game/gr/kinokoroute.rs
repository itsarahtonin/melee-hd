//! Mushroom Kingdom's adventure route, `melee/gr/grkinokoroute.c`: the route (map 3) with its
//! 51 brick blocks and the castle's arena at its end, the background (map 2), a backdrop drawn
//! behind (map 1) and four pieces that follow the first fighter (map 0). The camera follows the
//! first fighter along the route, kept between its ends; reaching the arena's watched point
//! shuts the route behind it and stops the enemy spawner (`grZakoGenerator_801CAF08`), and the
//! arena's end opens it again. The enemy spawner runs the stage's table from the start.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{KINOKOROUTE_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, map_gobj, nth_joint, p1_fighter, point, point_joint, scale, set_cam_bottom,
    set_cam_left, set_cam_offset, set_cam_right, set_cam_top, update_map_coll,
};
use super::{anime, material, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `grKinokoRoute_80207B30`, the route's start hook.
const START_HOOK: u32 = 0x8020_7B30;
/// `grKinokoRoute_80208660`, the water's fighter device.
pub const WATER_DEVICE: u32 = 0x8020_8660;
/// `grKinokoRoute_802084B4`, a brick block's hit.
pub const BLOCK_HIT: u32 = 0x8020_84B4;
/// `grKinokoRoute_80208480`, the arena's event callback.
const ARENA_EVENT: u32 = 0x8020_8480;

/// The route's map id.
const MAP_ROUTE: i32 = 3;

/// A stage object's process or callback, by address: the route's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_7A94 | 0x8020_7AD4 | 0x8020_7AD8 | 0x8020_7B28 | 0x8020_7B2C | 0x8020_8368 => {
            empty(r, gobj)
        }
        0x8020_7634 => init_followers(r, gobj),
        0x8020_78F0 => proc_followers(r, gobj),
        0x8020_7A98 => init_back(r, gobj),
        0x8020_7ADC => init_backdrop(r, gobj),
        0x8020_7B5C => init_route(r, gobj),
        0x8020_7C88 => proc_route(r, gobj),
        START_HOOK => start_route(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_7420 => on_init(r),
        0x8020_741C => on_demo_init(r),
        0x8020_7490 => on_load(r),
        0x8020_74D8 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
// Ports grKinokoRoute_80207A94; aliased once a check reaches it.
#[doc(alias = "grKinokoRoute_80207AD4")]
// Ports grKinokoRoute_80207AD8; aliased once a check reaches it.
#[doc(alias = "grKinokoRoute_80207B28")]
// Ports grKinokoRoute_80207B2C; aliased once a check reaches it.
// Ports grKinokoRoute_80208368; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The objects' predicates (`callback1`), which nothing calls and answer false.
// Ports grKinokoRoute_802078E8; aliased once a check reaches it.
// Ports grKinokoRoute_80207ACC; aliased once a check reaches it.
// Ports grKinokoRoute_80207B20; aliased once a check reaches it.
// Ports grKinokoRoute_80207C80; aliased once a check reaches it.
pub fn answers_false(_gobj: GObjId) -> bool {
    false
}

/// The stage's `callback4`, `grKinokoRoute_80207544`: false.
// Ports grKinokoRoute_80207544; aliased once a check reaches it.
pub fn callback4() -> bool {
    false
}

/// The stage's line-touch dynamics, `grKinokoRoute_80208754`: none.
#[doc(alias = "grKinokoRoute_80208754")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The eye's floor for the camera, `grKinokoRoute_802087B0`: 10.
#[doc(alias = "grKinokoRoute_802087B0")]
pub fn camera_floor() -> f32 {
    10.0
}

/// The stage's objects' setup functions and processes, by map id (`grNKr_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &KINOKOROUTE_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grKinokoRoute_8020754C`, its scale
/// joint's descriptor at `frame` on the stack.
#[doc(alias = "grKinokoRoute_8020754C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The scale joints' descriptors of the objects the route's process makes again: below the
/// process loop by the process's frame (0xA0) and the create function's (0x20).
const PROC_FRAME: u32 = super::corneria::PROC_LOOP - 0xA0 - 0x20;

/// Mushroom Kingdom's route's setup, `grKinokoRoute_80207420`: its parameters, the stage's
/// `unk8C` flags (b4 off, b5 on), and maps 2, 0, 1 and 3 in that order.
#[doc(alias = "grKinokoRoute_80207420")]
pub fn on_init(r: &mut Running) {
    r.game.gr.kinokoroute_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [2, 0, 1, 3] {
        create(r, map_id, setup::SETUP_FRAME);
    }
}

/// `grKinokoRoute_8020741C` does nothing (only a demo runs it).
// Ports grKinokoRoute_8020741C; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// The route's load, `grKinokoRoute_80207490`: the camera centred 20 above the first fighter,
/// and there at once.
#[doc(alias = "grKinokoRoute_80207490")]
pub fn on_load(r: &mut Running) {
    let Some(gobj) = p1_fighter(r.game) else {
        return;
    };
    let pos = r.game.ft_pos(gobj);
    set_cam_offset(r.game, pos.x, mem(fp::fadds(20.0, reg(pos.y))));
    super::super::camera::snap(r);
}

/// The route's start, `grKinokoRoute_802074D8`: the enemy spawner with the parameters' table
/// (from their fourth byte), then, by a draw of one in the parameters' first word (none for 0,
/// always taken), a trophy slot at a random trophy point, and every due spawn at once.
#[doc(alias = "grKinokoRoute_802074D8")]
pub fn on_start(r: &mut Running) {
    let p = r
        .game
        .gr
        .kinokoroute_yakumono
        .expect("grkinokoroute.c: yakumono_param");
    zako::start(r.game, Some(p.at(4)));
    let n = r.data.u32(p) as i32;
    let val = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if val == 0 {
        // The point is drawn before the trophy, as the call's arguments are evaluated.
        let point = super::ground::trophy_point(r.game, r.data);
        let trophy = super::ground::x6e4_drawn(r.game);
        zako::set_trophy_slot(r.game, trophy, point);
    }
    zako::scan_anywhere(r);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// `HSD_JObjGetTranslation` of a point's joint.
fn translation(game: &Game, j: JObjId) -> Vec3 {
    game.objs.jobjs[j].translate
}

/// The followers' setup (map 0), `grKinokoRoute_80207634`: each of points 0x7F to 0x82's joint
/// and its place from point 0x94's (none without that point), the camera limits and blast zones
/// measured, and the camera limits set to one and a half times their reach from the camera's
/// centre.
#[doc(alias = "grKinokoRoute_80207634")]
pub fn init_followers(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let mut entries = [KinokoRouteEntry::default(); 4];
    if let Some(origin_j) = point_joint(r.game, 0x94) {
        let origin = translation(r.game, origin_j);
        for (i, e) in entries.iter_mut().enumerate() {
            e.jobj = point_joint(r.game, 0x7F + i as i32);
            if let Some(j) = e.jobj {
                let t = translation(r.game, j);
                e.pos = t - origin;
            }
        }
        // The positions of joints that weren't found stay as the block held them; nothing
        // reads them (the process skips an entry without a joint).
        let v = r.game.grounds[g.0].u.kinokoroute();
        for (i, e) in entries.into_iter().enumerate() {
            v.entries[i].jobj = e.jobj;
            if e.jobj.is_some() {
                v.entries[i].pos = e.pos;
            }
        }
    } else {
        let v = r.game.grounds[g.0].u.kinokoroute();
        for e in &mut v.entries {
            e.jobj = None;
        }
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let c = stage::cam_center(r.game);
    let k = |a: f32, b: f32| mem(fp::fmuls(1.5, fp::fsubs(reg(a), reg(b))));
    let top = k(stage::cam_top(r.game), c.y);
    set_cam_top(r.game, top);
    let bottom = k(stage::cam_bottom(r.game), c.y);
    set_cam_bottom(r.game, bottom);
    let left = k(stage::cam_left(r.game), c.x);
    set_cam_left(r.game, left);
    let right = k(stage::cam_right(r.game), c.x);
    set_cam_right(r.game, right);
}

/// One follower's joint and place (`grKinokoRoute_GroundVars_Entry`), for the setup.
#[derive(Default, Clone, Copy)]
struct KinokoRouteEntry {
    pos: Vec3,
    jobj: Option<JObjId>,
}

/// The followers' process (map 0), `grKinokoRoute_802078F0`: each follower at its place from
/// the first fighter, in the stage's unscaled units.
#[doc(alias = "grKinokoRoute_802078F0")]
pub fn proc_followers(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let Some(fighter) = p1_fighter(r.game) else {
        return;
    };
    let pos = r.game.ft_pos(fighter);
    let s = reg(scale(r.game));
    let k = if s > reg(0.0001) {
        fp::fdivs(1.0, s)
    } else {
        1.0
    };
    for i in 0..4 {
        let e = r.game.grounds[g.0].u.kinokoroute().entries[i];
        if let Some(j) = e.jobj {
            let x = mem(fp::fmuls(k, fp::fadds(reg(e.pos.x), reg(pos.x))));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = x);
            let y = mem(fp::fmuls(k, fp::fadds(reg(e.pos.y), reg(pos.y))));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
        }
    }
}

/// The background's setup (map 2), `grKinokoRoute_80207A98`: its animation, then the camera
/// limits and blast zones measured.
#[doc(alias = "grKinokoRoute_80207A98")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// The backdrop's setup (map 1), `grKinokoRoute_80207ADC`: its animation, drawn behind.
#[doc(alias = "grKinokoRoute_80207ADC")]
pub fn init_backdrop(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The route's start hook, `grKinokoRoute_80207B30`: the water as a fighter device of type 6
/// (`ftCo_800C07F8`).
#[doc(alias = "grKinokoRoute_80207B30")]
pub fn start_route(r: &mut Running, gobj: GObjId) {
    r.game
        .gr
        .devices
        .register_coll(gobj, 6, CodeRef(WATER_DEVICE));
}

/// The route's setup (map 3), `grKinokoRoute_80207B5C`: its collision and animation, it
/// receives shadows, the route's state at its start (no watched point reached), the start hook,
/// the brick blocks, the collision linked on the first tick, and where point 4's joint (the
/// arena's respawn point) stands.
#[doc(alias = "grKinokoRoute_80207B5C")]
pub fn init_route(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    gp.x10_flags.b5 = true;
    let v = gp.u.kinokoroute2();
    v.phase = 0;
    v.zone_idx = 0;
    v.cam_timer = 0;
    v.spawn_idx = -1;
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    init_blocks(r, gobj);
    r.game.grounds[g.0].u.kinokoroute2().flags_0 = true;
    let reb0 = point_joint(r.game, 4).expect("grkinokoroute.c:467: reb0_jobj");
    let t = translation(r.game, reb0);
    r.game.grounds[g.0].u.kinokoroute2().reb0_pos = t;
}

/// A point the original reads whether or not the stage has it: every point the route reads
/// this way is on the stage, and a missing one would read a stack vector.
fn point_of(game: &mut Game, data: &Archives, id: i32) -> Vec3 {
    point(game, data, id).unwrap_or_else(|| panic!("stale read: point {id:#x} of the route"))
}

/// The route's process (map 3), `grKinokoRoute_80207C88`.
///
/// Before the arena (phase under 2) the camera's target is the first fighter, 20 to 250 high
/// in the stage's scale and kept right of point 0 and left of point 0xBD (or right of the
/// watched point once one is reached); the camera's centre stays put while the fighter is in
/// hitstun or dead. A fighter within 60 of a watched point starts the arena: the route shut
/// behind (maps 0 and 2 made again), the spawner stopped and its enemies and coins gone. The
/// zone (`stage_info.x6DC`) counts the points 5 and 6 passed. In the arena the camera looks at
/// the watched point, the respawn point stands 50 above point 0xBD, and the arena's end
/// (`Ground_801C5764` answering 1) opens the route again.
///
/// The camera's centre then eases toward the target: by a 1/timer share while the timer runs,
/// else 10 a tick toward it when farther than 10.
#[doc(alias = "grKinokoRoute_80207C88")]
pub fn proc_route(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let s = reg(scale(r.game));
    let Some(fighter) = p1_fighter(r.game) else {
        return;
    };
    let mut fighter_pos = r.game.ft_pos(fighter);
    let cam_target;
    if r.game.grounds[g.0].u.kinokoroute2().phase != 2 {
        if r.game.fighter(fighter).x221C_b6 {
            cam_target = stage::cam_center(r.game);
            r.game.grounds[g.0].u.kinokoroute2().cam_timer = 0x3C;
        } else if r.game.ft_is_dead(fighter) {
            cam_target = stage::cam_center(r.game);
            r.game.grounds[g.0].u.kinokoroute2().cam_timer = 0;
        } else {
            let lo = fp::fmuls(20.0, s);
            let hi = fp::fmuls(250.0, s);
            if reg(fighter_pos.y) < lo {
                fighter_pos.y = mem(lo);
            } else if reg(fighter_pos.y) > hi {
                fighter_pos.y = mem(hi);
            }
            let spawn_idx = r.game.grounds[g.0].u.kinokoroute2().spawn_idx;
            if spawn_idx != -1 {
                let p = point_of(r.game, r.data, spawn_idx.into());
                if fighter_pos.x < p.x {
                    fighter_pos.x = p.x;
                }
            } else {
                let p = point_of(r.game, r.data, 0);
                if fighter_pos.x < p.x {
                    fighter_pos.x = p.x;
                }
                let p = point_of(r.game, r.data, 0xBD);
                if fighter_pos.x > p.x {
                    fighter_pos.x = p.x;
                }
            }
            cam_target = fighter_pos;
            super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, true);
        }
        super::ground::watch_points(r.game, None, 30.0, 10000.0);
        if r.game.grounds[g.0].u.kinokoroute2().phase < 2 {
            let spawn_idx = super::ground::watch_points2(r.game, None, 60.0, 10000.0) as i16;
            r.game.grounds[g.0].u.kinokoroute2().spawn_idx = spawn_idx;
            if spawn_idx != -1 {
                let v = r.game.grounds[g.0].u.kinokoroute2();
                v.phase = 2;
                v.cam_timer = 0x3C;
                super::ground::clear_x6d8(r.game);
                super::super::gm::start_stage_event(r, 0x11, 0xA, 3, 0xB3, CodeRef(ARENA_EVENT));
                r.game.grounds[g.0].u.kinokoroute2().zone_idx = 2;
                rebuild(r, [0, 2], PROC_FRAME);
                set_cam_offset(r.game, cam_target.x, cam_target.y);
                zako::clear_enemies(r);
                r.game.grounds[g.0].u.kinokoroute2().cam_timer = 0x3C;
            }
        }
        let zone_idx = i32::from(r.game.grounds[g.0].u.kinokoroute2().zone_idx) + 5;
        if 7 > zone_idx
            && let Some(p) = point(r.game, r.data, zone_idx)
            && fighter_pos.x > p.x
        {
            r.game.grounds[g.0].u.kinokoroute2().zone_idx += 1;
        }
        r.game.stage_info.x6DC = r.game.grounds[g.0].u.kinokoroute2().zone_idx;
    } else {
        let spawn_idx = r.game.grounds[g.0].u.kinokoroute2().spawn_idx;
        fighter_pos = point_of(r.game, r.data, spawn_idx.into());
        fighter_pos.y = mem(fp::fadds(reg(fighter_pos.y), 30.0));
        cam_target = fighter_pos;
        super::super::camera::set_hold_off_plane(&mut r.game.camera.cam, false);
        r.game.stage_info.x6DC = 0;
        let reb0 = point_joint(r.game, 4);
        let eve = point_joint(r.game, 0xBD);
        let (Some(reb0), Some(eve)) = (reb0, eve) else {
            panic!("grkinokoroute.c:617: reb0_jobj&&eve_jobj");
        };
        let mut t = translation(r.game, eve);
        t.y = mem(fp::fadds(reg(t.y), 50.0));
        jobj::set_translate(&mut r.game.objs.jobjs, reb0, t);
        if r.game.grounds[g.0].u.kinokoroute2().cam_timer == 0 {
            shut(r, gobj, false);
        }
        if super::ground::x6d8(r.game) == 1 {
            let v = r.game.grounds[g.0].u.kinokoroute2();
            v.phase = 3;
            v.cam_timer = 0x3C;
            shut(r, gobj, true);
            zako::set_active(r.game, true);
            let reb0 = point_joint(r.game, 4).expect("grkinokoroute.c:654: reb0_jobj");
            let back = r.game.grounds[g.0].u.kinokoroute2().reb0_pos;
            jobj::set_translate(&mut r.game.objs.jobjs, reb0, back);
            rebuild(r, [2, 0], PROC_FRAME);
            set_cam_offset(r.game, cam_target.x, cam_target.y);
        }
    }
    let cam_offset = stage::cam_center(r.game);
    let timer = r.game.grounds[g.0].u.kinokoroute2().cam_timer;
    if timer == 0 {
        let diff = cam_offset - cam_target;
        let sq = |v: f32| fp::fmuls(reg(v), reg(v));
        let len = crate::math::sqrt_reg(fp::fadds(sq(diff.z), fp::fadds(sq(diff.x), sq(diff.y))));
        if len > 10.0 {
            let mut d = diff;
            d.normalize();
            d.x = mem(fp::fmuls(reg(d.x), 10.0));
            d.y = mem(fp::fmuls(reg(d.y), 10.0));
            d.z = mem(fp::fmuls(reg(d.z), 10.0));
            set_cam_offset(
                r.game,
                mem(fp::fadds(reg(cam_target.x), reg(d.x))),
                mem(fp::fadds(reg(cam_target.y), reg(d.y))),
            );
        }
    } else {
        let step = fp::fdivs(1.0, fp::frsp(f64::from(i32::from(timer))));
        let mut t = cam_target - cam_offset;
        t.x = mem(fp::fmuls(reg(t.x), step));
        t.y = mem(fp::fmuls(reg(t.y), step));
        t.z = mem(fp::fmuls(reg(t.z), step));
        let c = cam_offset + t;
        set_cam_offset(r.game, c.x, c.y);
        r.game.grounds[g.0].u.kinokoroute2().cam_timer -= 1;
    }
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    if r.game.grounds[g.0].u.kinokoroute2().flags_0 {
        r.game.geometry_mut().link_all_joints();
        r.game.grounds[g.0].u.kinokoroute2().flags_0 = false;
    }
}

/// Destroys maps `maps[0]`'s and `maps[1]`'s objects (`Ground_801C4A08`) and makes them again
/// in that order.
fn rebuild(r: &mut Running, maps: [i32; 2], frame: u32) {
    for m in [0, 2] {
        let gobj = map_gobj(r.game, m);
        if gobj.is_some() {
            setup::destroy(r, gobj);
        }
    }
    for m in maps {
        create(r, m, frame);
    }
}

/// Shuts the route behind the arena or opens it, `grKinokoRoute_8020836C`: joint 0x53 shown and
/// collision joints 0x3C, 0x33 and 0xC to 0xF on, the stage items' hurtboxes on, or the
/// reverse.
// Ports grKinokoRoute_8020836C; aliased once a check reaches it.
pub fn shut(r: &mut Running, gobj: GObjId, on: bool) {
    let Some(j) = nth_joint(r.game, gobj, 0x53) else {
        return;
    };
    let mut items = Vec::new();
    let mut cur = r.game.world.first_in(super::super::it::P_LINK_ITEM);
    while let Some(c) = cur {
        items.push(c);
        cur = r.game.world.gobj(c).next;
    }
    if on {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        r.game.geometry_mut().enable_joint(0x3C);
        r.game.geometry_mut().enable_joint(0x33);
        for item in items {
            if super::super::it::item(r.game, item).kind == super::super::it::kinds::PKIND_RANDOM {
                material::item_hurtbox_on(r.game, item);
            }
        }
        for id in 0xC..=0xF {
            r.game.geometry_mut().enable_joint(id);
        }
    } else {
        jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        r.game.geometry_mut().disable_joint(0x3C);
        r.game.geometry_mut().disable_joint(0x33);
        for item in items {
            if super::super::it::item(r.game, item).kind == super::super::it::kinds::PKIND_RANDOM {
                material::item_hurtbox_off(r.game, item);
            }
        }
        for id in 0xC..=0xF {
            r.game.geometry_mut().disable_joint(id);
        }
    }
}

/// The arena's event callback, `grKinokoRoute_80208480`: on 1, the stage's `x6D8` set to 1,
/// answering true.
// Ports grKinokoRoute_80208480; aliased once a check reaches it.
pub fn arena_event(game: &mut Game, arg: i32) -> bool {
    if arg == 1 {
        super::ground::set_x6d8(game, arg);
        return true;
    }
    false
}

/// A brick block's item hit, `grKinokoRoute_802084B4`: its joint hidden, the route's collision
/// linked again next tick, an effect at the joint, a small quake, a sound and the item gone.
#[doc(alias = "grKinokoRoute_802084B4")]
pub fn block_hit(r: &mut Running, item: GObjId, _g: GroundId) {
    let joint = match super::super::it::kinds::yaku::vars(r.game, item)
        .anchor
        .get()
    {
        super::super::it::kinds::yaku::Anchor::Joint(j) => j,
        _ => panic!("a brick block's item without a joint"),
    };
    jobj::set_flags_all(&mut r.game.objs.jobjs, joint, jobj::HIDDEN);
    if let Some(route) = map_gobj(r.game, MAP_ROUTE) {
        let g = ground_of(r.game, route);
        r.game.grounds[g.0].u.kinokoroute2().flags_0 = true;
    }
    let pos = world_position(&mut r.game.objs, r.data, joint, None);
    super::super::ef::spawn(r, 0x442, Some(item), &[super::super::ef::Arg::Vec(pos)]);
    super::super::camera::request_quake(r, 2, None);
    r.stage_sound_on_track(0x136, 0xBA);
    material::destroy_item(r, item);
}

/// The brick blocks, `grKinokoRoute_80208564`: a hurtbox item (state 0, number 8) on each of the
/// route's joints 1 to 51, its hurtbox a point of radius 7, on, on the ground.
#[doc(alias = "grKinokoRoute_80208564")]
pub fn init_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for depth in 1..=51 {
        let j = nth_joint(r.game, gobj, depth);
        let item = material::stage_item(r, 8, 0, g, j, None, Some(CodeRef(BLOCK_HIT)), None);
        if let Some(item) = item {
            let z = Vec3::new(0.0, 0.0, 0.0);
            material::set_item_hurtbox(r.game, item, z, z, 7.0);
            material::item_hurtbox_on(r.game, item);
            material::item_ground_or_air(r.game, item, 0);
        }
    }
}

/// The water's fighter device, `grKinokoRoute_80208660`: a fighter dropping through the water's
/// surface (5.5) this tick splashes there, as big as its camera box's depth, and the stage's
/// splash sound plays. It never pushes.
#[doc(alias = "grKinokoRoute_80208660")]
pub fn water_device(r: &mut Running, fighter: GObjId) {
    let mut pos = r.game.ft_pos(fighter);
    let prev = r.game.fighter(fighter).prev_pos;
    if reg(pos.y) < reg(5.5) && reg(prev.y) > reg(5.5) {
        let size = fp::fdivs(reg(r.game.ft_camera_depth(fighter)), 10.0);
        pos.y = 5.5;
        splash(r, pos, mem(size));
        r.stage_sound(0x77A10);
    }
}

/// A splash at `pos`, `grKinokoRoute_802086EC`: the stage's effect 0x7530, scaled by `size`.
#[doc(alias = "grKinokoRoute_802086EC")]
pub fn splash(r: &mut Running, pos: Vec3, size: f32) {
    let Some(g) = super::grlib::generator_at(r.game, r.data, 0x7530, 0x1E, pos) else {
        return;
    };
    let s = r.game.objs.generators[g]
        .appsrt
        .expect("a generator's app-SRT");
    let srt = &mut r.game.objs.app_srts[s];
    srt.scale.x = mem(fp::fmuls(reg(srt.scale.x), reg(size)));
    srt.scale.y = mem(fp::fmuls(reg(srt.scale.y), reg(size)));
    srt.scale.z = mem(fp::fmuls(reg(srt.scale.z), reg(size)));
}

/// The stage's shadow check, `grKinokoRoute_8020875C`: a fighter casts on a joint below its
/// feet.
#[doc(alias = "grKinokoRoute_8020875C")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
