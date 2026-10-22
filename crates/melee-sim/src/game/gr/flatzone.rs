//! Flat Zone, `melee/gr/grflatzone.c`: the Game & Watch screen. A controller (map 2) waits a
//! while, then starts one of two events: the oil panic (map 3), whose man walks in step by
//! step from a side, pours his bucket and leaves a slick of oil (a dynamic attribute area,
//! terrain 0x11) on the floor below, or the helmet game (map 8), which drops tools from the
//! top of the screen for a while (`it::kinds::tools`). Between events the platforms (map 5)
//! change: each set of them (`SETS`) animates in and out at random; during an event they keep
//! one pose. Maps 4 and 6 are two more events nothing starts, map 1 the background, map 0 the
//! stage's animation.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{FLATZONE_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, update_map_coll};
use super::{anime, dynamic_attr, grlib, material, setup};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The controller's and the platforms' start hooks, `fn_8021706C` and `fn_80217440`.
const CONTROLLER_START: u32 = 0x8021_706C;
const PLATFORMS_START: u32 = 0x8021_7440;

/// The objects' map ids.
const MAP_CONTROLLER: i32 = 2;
const MAP_OIL: i32 = 3;
const MAP_EVENT1: i32 = 4;
const MAP_PLATFORMS: i32 = 5;
const MAP_EVENT2: i32 = 6;
const MAP_TOOLS: i32 = 8;

/// The events the controller draws from (the setup's `vals`): the helmet game's tools and the
/// oil panic. Events 1 and 2 (maps 4 and 6) have cases but are never drawn.
const EVENTS: [u8; 2] = [3, 4];

/// The terrain the oil gives the floor under it (`grDynamicAttr_801CA0F8`'s kind), its
/// circle's radius, and where it lands from the man's position.
const OIL_TERRAIN: i32 = 0x11;
const OIL_RADIUS: f32 = 22.0;
const OIL_REACH: f32 = 36.0;
const OIL_DROP: f32 = 27.0;

/// The platforms' animations by set and row (`grFz_803E7A68`, rows of five): row 0 is a
/// set's pose during an event; -1 marks a row the set doesn't have. Set 8 hides them.
const SETS: [[i16; 5]; 9] = [
    [0, 1, 2, -1, -1],
    [3, 4, 5, 6, 7],
    [8, 9, 10, 11, 12],
    [13, 14, 15, 16, 17],
    [18, -1, -1, -1, -1],
    [19, -1, -1, -1, -1],
    [20, -1, -1, -1, -1],
    [21, -1, -1, -1, -1],
    [0, 0, 0, 0, 0],
];

/// The platforms' hidden set.
const HIDDEN_SET: u8 = 8;

/// `grDynamicAttr_8049EFA0`, the dynamic attribute areas, 0x24 bytes each: the oil's area is
/// a pointer into it.
pub const AREAS_ADDR: u32 = 0x8049_EFA0;

/// An area's index from its address in the original (null for none).
pub fn area_index(p: u32) -> Option<u8> {
    (p != 0).then(|| ((p - AREAS_ADDR) / 0x24) as u8)
}

/// An area's address in the original from its index.
pub fn area_addr(i: Option<u8>) -> u32 {
    i.map_or(0, |i| AREAS_ADDR + 0x24 * u32::from(i))
}

/// The fields of `grFlatzone_YakumonoParam`, by offset.
mod p {
    /// The controller's wait between events, between these two (`unk0`, `unk4`).
    pub const WAIT_A: u32 = 0;
    pub const WAIT_B: u32 = 4;
    /// The platforms' wait between animations (`unk8`, `unkC`).
    pub const PLATFORM_WAIT_A: u32 = 8;
    pub const PLATFORM_WAIT_B: u32 = 0xC;
    /// How long the tools fall (`unk10`), a tool every so many ticks (`unk14`), and between
    /// which x they fall (`unk18`, `unk1C`).
    pub const TOOLS_TIME: u32 = 0x10;
    pub const TOOLS_PERIOD: u32 = 0x14;
    pub const TOOLS_RIGHT: u32 = 0x18;
    pub const TOOLS_LEFT: u32 = 0x1C;
    /// The oil panic's wait before the man walks (`unk20`, halved from the right), where he
    /// starts from the right and the left (`unk24`, `unk28`, floats), a step's ticks
    /// (`unk2C`), between how many steps he takes (`unk30`, `unk34`), a step's length
    /// (`unk38`, a float) and how long the oil lasts (`unk3C`).
    pub const OIL_WAIT: u32 = 0x20;
    pub const OIL_RIGHT_X: u32 = 0x24;
    pub const OIL_LEFT_X: u32 = 0x28;
    pub const STEP_TICKS: u32 = 0x2C;
    pub const STEPS_MIN: u32 = 0x30;
    pub const STEPS_MAX: u32 = 0x34;
    pub const STEP: u32 = 0x38;
    pub const OIL_TIME: u32 = 0x3C;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .flatzone_yakumono
            .expect("grflatzone.c: yakumono_param"),
    )
}

/// C's `%` on the Gekko: `divw`, then the remainder.
fn rem(a: i32, b: i32) -> i32 {
    a.wrapping_sub(crate::ppc::divw(a, b).wrapping_mul(b))
}

/// A stage object's process or callback, by address: Flat Zone's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_7064 | 0x8021_7068 | 0x8021_73D0 | 0x8021_7438 | 0x8021_743C | 0x8021_7644
        | 0x8021_7CD0 | 0x8021_7DA4 | 0x8021_7E78 | 0x8021_7E7C | 0x8021_7E88 | 0x8021_7E8C
        | 0x8021_805C => empty(r, gobj),
        0x8021_7030 => init_main(r, gobj),
        CONTROLLER_START => start_controller(r, gobj),
        0x8021_707C => init_controller(r, gobj),
        0x8021_71D4 => proc_controller(r, gobj),
        0x8021_73D4 => init_back(r, gobj),
        PLATFORMS_START => start_platforms(r, gobj),
        0x8021_7450 => init_platforms(r, gobj),
        0x8021_74EC => proc_platforms(r, gobj),
        0x8021_7648 => init_oil(r, gobj),
        0x8021_76BC => proc_oil(r, gobj),
        0x8021_7CD4 => init_event1(r, gobj),
        0x8021_7D30 => proc_event1(r, gobj),
        0x8021_7DA8 => init_event2(r, gobj),
        0x8021_7E04 => proc_event2(r, gobj),
        0x8021_7E90 => init_tools(r, gobj),
        0x8021_7EF0 => proc_tools(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_6E78 => on_init(r),
        0x8021_6F38 => on_load(r),
        0x8021_6F3C => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes, setups and destroy callbacks that do nothing.
#[doc(alias = "grFlatzone_80217064")]
#[doc(alias = "grFlatzone_80217068")]
#[doc(alias = "grFlatzone_802173D0")]
#[doc(alias = "grFlatzone_80217438")]
#[doc(alias = "grFlatzone_8021743C")]
#[doc(alias = "grFlatzone_80217644")]
#[doc(alias = "grFlatzone_80217CD0")]
#[doc(alias = "grFlatzone_80217DA4")]
#[doc(alias = "grFlatzone_80217E78")]
#[doc(alias = "grFlatzone_80217E7C")]
#[doc(alias = "grFlatzone_80217E88")]
#[doc(alias = "grFlatzone_80217E8C")]
#[doc(alias = "grFlatzone_8021805C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grFlatzone_80216F48`.
#[doc(alias = "grFlatzone_80216F48")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grFz_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &FLATZONE_CALLBACKS[map_id as usize]
}

/// Flat Zone's setup, `grFlatzone_80216E78`: its parameters, maps 0 to 6 and 8, the
/// platforms' first set, camera limits and blast zones, the camera's depth range, and the
/// bottom blast zone 50 lower.
#[doc(alias = "grFlatzone_80216E78")]
pub fn on_init(r: &mut Running) {
    r.game.gr.flatzone_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 1, 2, 3, 4, 5, 6, 8] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    next_platforms(r, true);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    super::super::camera::set_near(&mut r.game.camera.cam, 1.0);
    super::super::camera::set_far(&mut r.game.camera.cam, 1000.0);
    let bottom = fp::fsubs(reg(super::stage::blast_bottom(r.game)), 50.0);
    super::ground::set_blast_bottom(r.game, mem(bottom));
}

/// `grFlatzone_80216E74` does nothing (only a demo runs it).
#[doc(alias = "grFlatzone_80216E74")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grFlatzone_UnkStage0_OnLoad` does nothing.
#[doc(alias = "grFlatzone_UnkStage0_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// `grFlatzone_UnkStage0_OnStart` does nothing: Flat Zone starts no enemy spawner.
#[doc(alias = "grFlatzone_UnkStage0_OnStart")]
pub fn on_start(_r: &mut Running) {}

/// The stage's lines' hazards, `grFlatzone_802182B4`: none.
#[doc(alias = "grFlatzone_802182B4")]
pub fn line_hazard() -> Option<DataRef> {
    None
}

/// The stage's shadow check, `grFlatzone_802182BC`: every joint takes the shadows.
#[doc(alias = "grFlatzone_802182BC")]
pub fn check_shadow_render() -> bool {
    true
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@80217030")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The controller's start hook, `fn_8021706C`: it may count down.
#[doc(alias = "fn_8021706C")]
pub fn start_controller(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.flatzone().xC4 = 0;
}

/// The controller's setup (map 2), `grFlatzone_8021707C`: its collision, the model a unit in
/// front, its animation, waiting for the match's start with an event allowed, its first wait
/// and its start hook.
#[doc(alias = "grFlatzone_8021707C")]
pub fn init_controller(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let jj = j.expect("jobj.h: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.translate.z = 1.0);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let p = param(r.game);
    let (a, b) = (p.i(r.data, p::WAIT_B), p.i(r.data, p::WAIT_A));
    let v = r.game.grounds[g.0].u.flatzone();
    v.xC4 = 1;
    v.xC7 = 1;
    v.xC6 = 0;
    let wait = grlib::rand_range(&mut r.game.objs.rng, a, b);
    r.game.grounds[g.0].u.flatzone().xC8 = wait as i16;
    setup::add_start_hook(r.game, gobj, CodeRef(CONTROLLER_START));
}

/// Starts the event of map `map` (the inline `grFlatzone_802171D4_inline`): its object's
/// `xC4`.
fn start_event(game: &mut Game, map: i32) {
    let Some(gobj) = map_gobj(game, map) else {
        return;
    };
    let g = ground_of(game, gobj);
    let u = &mut game.grounds[g.0].u;
    match map {
        MAP_OIL => u.flatzone_oil().xC4 = 1,
        MAP_TOOLS => u.flatzone_tools().xC4 = 1,
        _ => u.flatzone_event().xC4 = 1,
    }
}

/// Lets the controller start another event: map 2's `xC7`, as each event's end sets it.
fn event_over(game: &mut Game) {
    if let Some(gobj) = map_gobj(game, MAP_CONTROLLER) {
        let g = ground_of(game, gobj);
        game.grounds[g.0].u.flatzone().xC7 = 1;
    }
}

/// The controller's process, `grFlatzone_802171D4`. Once the match has started and no event
/// runs, it counts down; at the end it draws an event other than the last (the tools or the
/// oil panic) and starts it, the platforms taking a pose; with the count already out (an
/// event just ended), it picks the platforms' next set and a new wait. Then the collision and
/// the wind sources.
#[doc(alias = "grFlatzone_802171D4")]
pub fn proc_controller(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.flatzone();
    if v.xC4 == 0 && v.xC7 != 0 {
        if v.xC8 != 0 {
            v.xC8 = v.xC8.wrapping_sub(1);
            if v.xC8 == 0 {
                v.xC7 = 0;
                v.xC6 = v.xC5;
                loop {
                    let e = EVENTS[r.game.objs.rng.below(2) as usize];
                    let v = r.game.grounds[g.0].u.flatzone();
                    v.xC5 = e;
                    if v.xC6 != v.xC5 {
                        break;
                    }
                }
                match r.game.grounds[g.0].u.flatzone().xC5 {
                    1 => {
                        start_event(r.game, MAP_EVENT1);
                        hide_platforms(r.game);
                    }
                    2 => {
                        start_event(r.game, MAP_EVENT2);
                        hide_platforms(r.game);
                    }
                    3 => {
                        start_event(r.game, MAP_TOOLS);
                        pose_platforms(r);
                    }
                    4 => {
                        start_event(r.game, MAP_OIL);
                        pose_platforms(r);
                    }
                    _ => {}
                }
            }
        } else {
            next_platforms(r, false);
            let p = param(r.game);
            let (a, b) = (p.i(r.data, p::WAIT_B), p.i(r.data, p::WAIT_A));
            let wait = grlib::rand_range(&mut r.game.objs.rng, a, b);
            r.game.grounds[g.0].u.flatzone().xC8 = wait as i16;
        }
    }
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The background's setup (map 1), `grFlatzone_802173D4`: its animation, its materials marked
/// with render-mode bits 27 and 29, drawn in the background layer.
#[doc(alias = "grFlatzone_802173D4")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    material::set_tree_flags(r.game, j, 0x2800_0000);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The platforms' start hook, `fn_80217440`: they may change.
#[doc(alias = "fn_80217440")]
pub fn start_platforms(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.flatzone_platforms().xC4 = 0;
}

/// The platforms' setup (map 5), `grFlatzone_80217450`: their collision, hidden, in the
/// hidden set with no row, no animation yet, waiting for the match's start, and their start
/// hook.
#[doc(alias = "grFlatzone_80217450")]
pub fn init_platforms(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let v = r.game.grounds[g.0].u.flatzone_platforms();
    v.xC7 = HIDDEN_SET;
    v.xC8 = HIDDEN_SET;
    v.xC9 = 0;
    v.xCA = 0;
    v.xCC = 0;
    v.xC6 = 0xFF;
    v.xC4 = 1;
    setup::add_start_hook(r.game, gobj, CodeRef(PLATFORMS_START));
}

/// The platforms' process, `grFlatzone_802174EC`. Once the match has started, with a set
/// shown and a row (no event running), each time the count runs out the set takes another of
/// its rows than the last, and a new wait. Then the collision, and every collision joint
/// linked again when the animation changed.
#[doc(alias = "grFlatzone_802174EC")]
pub fn proc_platforms(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.flatzone_platforms();
    if v.xC4 == 0 && v.xC7 != HIDDEN_SET && v.xC9 != 0 {
        let old = v.xCC;
        v.xCC = old.wrapping_sub(1);
        if old < 0 {
            v.xCA = v.xC9;
            let row = loop {
                let next = r.game.objs.rng.below(4) + 1;
                let v = r.game.grounds[g.0].u.flatzone_platforms();
                v.xC9 = next as u8;
                if v.xCA == v.xC9 {
                    continue;
                }
                let row = SETS[v.xC7 as usize][v.xC9 as usize];
                if row != -1 {
                    break row;
                }
            };
            r.game.grounds[g.0].u.flatzone_platforms().xC5 = row as u8;
            let m = map_id(r.game, gobj);
            anime::start_map(r.game, r.data, gobj, m, i32::from(row as u8));
            let p = param(r.game);
            let (a, b) = (
                p.i(r.data, p::PLATFORM_WAIT_B),
                p.i(r.data, p::PLATFORM_WAIT_A),
            );
            let wait = grlib::rand_range(&mut r.game.objs.rng, a, b);
            r.game.grounds[g.0].u.flatzone_platforms().xCC = wait as i16;
        }
    }
    update_map_coll(r.game, r.data, gobj);
    let v = r.game.grounds[g.0].u.flatzone_platforms();
    if v.xC6 != v.xC5 {
        v.xC6 = v.xC5;
        r.game.geometry_mut().link_all_joints();
    }
}

/// The oil panic's setup (map 3), `grFlatzone_80217648`: its collision, hidden, not started,
/// facing right, no oil, idle (state -1).
#[doc(alias = "grFlatzone_80217648")]
pub fn init_oil(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let v = r.game.grounds[g.0].u.flatzone_oil();
    v.xC4 = 0;
    v.xC8 = 1.0;
    v.xCC = None;
    v.xD0 = -1;
    v.timer = 0;
}

/// Starts the animation of map `map` (`grAnime_801C8138` on its object).
fn anim_map(r: &mut Running, map: i32, anim: i32) {
    if let Some(gobj) = map_gobj(r.game, map) {
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, anim);
    }
}

/// The oil panic's process, `grFlatzone_802176BC`. Started by the controller, the man takes a
/// side at random and stands at its start (the controller's screen turning for the left);
/// state 0 waits (half as long from the right), state 1 walks him in step by step for a
/// random number of steps, state 2 plays his pour and, once it ends, puts the oil on the
/// floor below where it lands (if there is one) and lets the controller go on, state 3 waits
/// while the oil lasts and state 4 hides him. Then the collision.
#[doc(alias = "grFlatzone_802176BC")]
pub fn proc_oil(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    let m = map_id(r.game, gobj);
    let p = param(r.game);
    let mut trigger = false;
    let mut pos = Vec3::new(0.0, 0.0, 0.0);
    if r.game.grounds[g.0].u.flatzone_oil().xC4 != 0 {
        if r.game.grounds[g.0].u.flatzone_oil().xD0 == 3 {
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            // Melee's bug: the oil's area is freed only when there is none
            // (`grDynamicAttr_801CA1C0` returns at once on null), so it stays until its timer
            // runs out.
            if r.game.grounds[g.0].u.flatzone_oil().xCC.is_none() {
                r.game.grounds[g.0].u.flatzone_oil().xCC = None;
            }
        }
        let right = r.game.objs.rng.below(2) != 0;
        let v = r.game.grounds[g.0].u.flatzone_oil();
        v.xC4 = 0;
        v.xC8 = if right { 1.0 } else { -1.0 };
        v.xD0 = 0;
        v.timer = p.i(r.data, p::OIL_WAIT);
        if v.xC8 == 1.0 {
            pos.x = p.f(r.data, p::OIL_RIGHT_X);
            let v = r.game.grounds[g.0].u.flatzone_oil();
            v.timer = fp::fctiwz(fp::fdivs(fp::frsp(f64::from(v.timer)), 2.0));
        } else {
            pos.x = p.f(r.data, p::OIL_LEFT_X);
            anim_map(r, MAP_CONTROLLER, 1);
        }
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    }
    let side_anim = |r: &mut Running, right: i32, left: i32| {
        let right_side = r.game.grounds[g.0].u.flatzone_oil().xC8 == 1.0;
        anime::start_map(
            r.game,
            r.data,
            gobj,
            m,
            if right_side { right } else { left },
        );
    };
    match r.game.grounds[g.0].u.flatzone_oil().xD0 {
        0 => {
            let wait = p.i(r.data, p::OIL_WAIT);
            if r.game.grounds[g.0].u.flatzone_oil().timer == crate::ppc::divw(wait, 2) {
                side_anim(r, 0, 2);
                jobj::remove_anim_all(&mut r.game.objs, Some(j));
            }
            let v = r.game.grounds[g.0].u.flatzone_oil();
            v.timer = v.timer.wrapping_sub(1);
            if v.timer <= 0 {
                v.xD0 = 1;
                let (ticks, lo, hi) = (
                    p.i(r.data, p::STEP_TICKS),
                    p.i(r.data, p::STEPS_MIN),
                    p.i(r.data, p::STEPS_MAX),
                );
                let span = hi.wrapping_sub(lo);
                let extra = if span != 0 {
                    r.game.objs.rng.below(span)
                } else {
                    0
                };
                r.game.grounds[g.0].u.flatzone_oil().timer =
                    ticks.wrapping_mul(lo.wrapping_add(extra));
                side_anim(r, 0, 2);
            }
        }
        1 => {
            let ticks = p.i(r.data, p::STEP_TICKS);
            if rem(r.game.grounds[g.0].u.flatzone_oil().timer, ticks) == 1 {
                pos = r.game.objs.jobjs[j].translate;
                let step = p.f(r.data, p::STEP);
                let side = r.game.grounds[g.0].u.flatzone_oil().xC8;
                pos.x = mem(fp::fmadds(reg(step), reg(side), reg(pos.x)));
                jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
            }
            let v = r.game.grounds[g.0].u.flatzone_oil();
            v.timer = v.timer.wrapping_sub(1);
            if v.timer <= 0 {
                v.xD0 = 2;
                v.timer = 0;
                side_anim(r, 1, 3);
            }
        }
        2 => {
            if anime::finished(r.game, gobj, 0, 1) {
                let time = p.i(r.data, p::OIL_TIME);
                let v = r.game.grounds[g.0].u.flatzone_oil();
                v.xD0 = 3;
                v.timer = time;
                let side = v.xC8;
                pos = r.game.objs.jobjs[j].translate;
                pos.x = mem(fp::fmadds(f64::from(OIL_REACH), reg(side), reg(pos.x)));
                pos.y = mem(fp::fsubs(reg(pos.y), f64::from(OIL_DROP)));
                let from_x = pos.x;
                let from_y = mem(fp::fadds(5.0, reg(pos.y)));
                pos.y = mem(fp::fsubs(reg(pos.y), 5.0));
                let hit = r
                    .game
                    .geometry_mut()
                    .check_multiple(from_x, from_y, pos.x, pos.y, 1, -1, 50);
                if let Some(h) = hit.filter(|h| h.line != -1) {
                    let area = dynamic_attr::add(
                        &mut r.game.gr.dynamic_attrs,
                        OIL_TERRAIN,
                        h.pos,
                        h.line,
                        OIL_RADIUS,
                        time,
                    );
                    r.game.grounds[g.0].u.flatzone_oil().xCC = area;
                }
                trigger = true;
            }
        }
        3 => {
            let v = r.game.grounds[g.0].u.flatzone_oil();
            v.timer = v.timer.wrapping_sub(1);
            if v.timer <= 1 {
                v.xD0 = 4;
                v.timer = 0;
            }
        }
        4 => {
            jobj::remove_anim_all(&mut r.game.objs, Some(j));
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            let v = r.game.grounds[g.0].u.flatzone_oil();
            v.xCC = None;
            v.xD0 = -1;
        }
        _ => {}
    }
    if trigger {
        event_over(r.game);
        if r.game.grounds[g.0].u.flatzone_oil().xC8 != 1.0 {
            anim_map(r, MAP_CONTROLLER, 0);
        }
    }
    update_map_coll(r.game, r.data, gobj);
}

/// The first unused event's setup (map 4), `grFlatzone_80217CD4`: its collision, hidden, not
/// started.
#[doc(alias = "grFlatzone_80217CD4")]
pub fn init_event1(r: &mut Running, gobj: GObjId) {
    init_event(r, gobj);
}

/// The second unused event's setup (map 6), `grFlatzone_80217DA8`: the same.
#[doc(alias = "grFlatzone_80217DA8")]
pub fn init_event2(r: &mut Running, gobj: GObjId) {
    init_event(r, gobj);
}

fn init_event(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.grounds[g.0].u.flatzone_event().xC4 = 0;
}

/// The first unused event's process, `grFlatzone_80217D30`: started, it ends at once, letting
/// the controller go on. Then the collision.
#[doc(alias = "grFlatzone_80217D30")]
pub fn proc_event1(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.flatzone_event().xC4 != 0 {
        event_over(r.game);
        r.game.grounds[g.0].u.flatzone_event().xC4 = 0;
    }
    update_map_coll(r.game, r.data, gobj);
}

/// The second unused event's process, `grFlatzone_80217E04`: the first's.
#[doc(alias = "grFlatzone_80217E04")]
pub fn proc_event2(r: &mut Running, gobj: GObjId) {
    proc_event1(r, gobj);
}

/// The tools' setup (map 8), `grFlatzone_80217E90`: its collision, hidden, not started, no
/// count.
#[doc(alias = "grFlatzone_80217E90")]
pub fn init_tools(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let v = r.game.grounds[g.0].u.flatzone_tools();
    v.xC4 = 0;
    v.xC8 = 0;
}

/// The tools' process, `grFlatzone_80217EF0`. Started by the controller, it counts the
/// parameters' time down, dropping a tool every so many ticks: at a random x between the
/// parameters' sides, 5 below the top blast zone, a random tool facing either way, kept to
/// the stage's collision joint 50. At the end it lets the controller go on.
#[doc(alias = "grFlatzone_80217EF0")]
pub fn proc_tools(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    if r.game.grounds[g.0].u.flatzone_tools().xC4 != 0 {
        let time = p.i(r.data, p::TOOLS_TIME);
        let v = r.game.grounds[g.0].u.flatzone_tools();
        v.xC8 = time;
        v.xC4 = 0;
    }
    if r.game.grounds[g.0].u.flatzone_tools().xC8 > 0 {
        let period = p.i(r.data, p::TOOLS_PERIOD);
        if rem(r.game.grounds[g.0].u.flatzone_tools().xC8, period) == 0 {
            let mut x = reg(r.game.objs.rng.draw_f32());
            let left = p.i(r.data, p::TOOLS_LEFT);
            let right = p.i(r.data, p::TOOLS_RIGHT);
            x = fp::fmuls(x, fp::frsp(f64::from(right.wrapping_sub(left))));
            x = fp::fsubs(x, fp::frsp(f64::from(left.wrapping_neg())));
            let top = fp::fsubs(reg(super::stage::blast_top(r.game)), 5.0);
            let pos = Vec3::new(mem(x), mem(top), 0.0);
            let which = r.game.objs.rng.below(5);
            let facing = if r.game.objs.rng.below(2) != 0 {
                1.0
            } else {
                -1.0
            };
            if let Some(item) = super::super::it::kinds::tools::spawn(r, pos, which, facing) {
                super::super::it::item_mut(r.game, item)
                    .x378_itemColl
                    .joint_id_only = 50;
            }
        }
        let v = r.game.grounds[g.0].u.flatzone_tools();
        v.xC8 = v.xC8.wrapping_sub(1);
        if v.xC8 <= 0 {
            event_over(r.game);
        }
    }
}

/// Picks the platforms' next set, `grFlatzone_80218060`: at the setup set 2's first row;
/// otherwise a set other than the hidden one that has a first row, and one of its rows at
/// random. The row's animation starts, and a wait before the next.
#[doc(alias = "grFlatzone_80218060")]
pub fn next_platforms(r: &mut Running, first: bool) {
    let Some(gobj) = map_gobj(r.game, MAP_PLATFORMS) else {
        return;
    };
    let g = ground_of(r.game, gobj);
    if first {
        let v = r.game.grounds[g.0].u.flatzone_platforms();
        v.xC7 = 2;
        v.xC9 = 1;
    } else {
        loop {
            let set = loop {
                let s = r.game.objs.rng.below(8) as u8;
                let v = r.game.grounds[g.0].u.flatzone_platforms();
                v.xC7 = s;
                if v.xC7 != v.xC8 {
                    break v.xC7;
                }
            };
            if SETS[set as usize][1] != -1 {
                break;
            }
        }
        loop {
            let row = (r.game.objs.rng.below(4) + 1) as u8;
            let v = r.game.grounds[g.0].u.flatzone_platforms();
            v.xC9 = row;
            if SETS[v.xC7 as usize][v.xC9 as usize] != -1 {
                break;
            }
        }
    }
    let p = param(r.game);
    let (a, b) = (
        p.i(r.data, p::PLATFORM_WAIT_B),
        p.i(r.data, p::PLATFORM_WAIT_A),
    );
    r.game.grounds[g.0].u.flatzone_platforms().xCA = 0;
    let wait = grlib::rand_range(&mut r.game.objs.rng, a, b);
    let v = r.game.grounds[g.0].u.flatzone_platforms();
    v.xCC = wait as i16;
    v.xC5 = SETS[v.xC7 as usize][v.xC9 as usize] as u8;
    let anim = i32::from(v.xC5);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
}

/// The platforms' pose for an event, `grFlatzone_802181B4`: a set other than the hidden one,
/// its first row, no wait.
#[doc(alias = "grFlatzone_802181B4")]
pub fn pose_platforms(r: &mut Running) {
    let Some(gobj) = map_gobj(r.game, MAP_PLATFORMS) else {
        return;
    };
    let g = ground_of(r.game, gobj);
    loop {
        let s = r.game.objs.rng.below(8) as u8;
        let v = r.game.grounds[g.0].u.flatzone_platforms();
        v.xC7 = s;
        if v.xC7 != v.xC8 {
            break;
        }
    }
    let v = r.game.grounds[g.0].u.flatzone_platforms();
    v.xC9 = 0;
    v.xCA = 0;
    v.xCC = 0;
    v.xC5 = SETS[v.xC7 as usize][v.xC9 as usize] as u8;
    let anim = i32::from(v.xC5);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
}

/// Hides the platforms for an event, `grFlatzone_80218260`: the hidden set.
#[doc(alias = "grFlatzone_80218260")]
pub fn hide_platforms(game: &mut Game) {
    let Some(gobj) = map_gobj(game, MAP_PLATFORMS) else {
        return;
    };
    let g = ground_of(game, gobj);
    game.grounds[g.0].u.flatzone_platforms().xC7 = HIDDEN_SET;
    let j = jobj_of(game, gobj);
    if j.is_some() {
        jobj::set_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
    }
}
