//! The race, `melee/gr/grpushon.c` (stage kind 39, the debug menu's `tukisusume`; Classic's
//! "Race to the Finish"): the course (map 1) with its spikes and its goals, the backdrop (map 0)
//! and the lights (map 2). The camera follows the first fighter, kept between the course's
//! floor and 800 above it; landing on a goal's platform (collision joints 3 to 5) inside a goal
//! point's box ends the race (the stage watcher's search with the course's check). The course
//! lights the fighters itself: nine point lights of the fighters' light set placed and colored
//! from a table, the five nearest the first fighter shown, and a specular light added that
//! takes the second nearest's place and color.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{Game, Obj, Running};
use super::data::{PUSHON_CALLBACKS, StageCallbacks};
use super::ground::{
    ground_of, jobj_of, p1_fighter, scale, set_cam_bottom, set_cam_left, set_cam_offset,
    set_cam_right, set_cam_top,
};
use super::{CollUser, anime, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::lobj::{self, LObjId};
use crate::vec::Vec3;

/// `fn_802192A4`, the spikes' fighter device.
pub const SPIKES_DEVICE: u32 = 0x8021_92A4;
/// `fn_802190A0`, a goal platform's collision callback.
pub const GOAL_TOUCHED: u32 = 0x8021_90A0;
/// `fn_80218678`, the course's check of a goal point.
pub const GOAL_CHECK: u32 = 0x8021_8678;
/// `grPushOn_80219458`, the stage's line-touch dynamics.
pub const ON_TOUCH_LINE: u32 = 0x8021_9458;
/// `grPushOn_80219528`, the stage's shadow check.
pub const CHECK_SHADOW_RENDER: u32 = 0x8021_9528;

/// The course's map id.
const MAP_COURSE: i32 = 1;

/// The scale joint's descriptor of the lights' object, which the load hook makes: Corneria's
/// load hook's (an 8-byte frame, [`super::corneria::LOAD_FRAME`]) less this one's 0x10 more.
const LOAD_FRAME: u32 = super::corneria::LOAD_FRAME - 0x10;

/// A stage object's process or callback, by address: the race's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_8598 | 0x8021_859C | 0x8021_87A4 | 0x8021_8ED0 => empty(r, gobj),
        0x8021_84CC => init_backdrop(r, gobj),
        0x8021_85A0 => init_course(r, gobj),
        0x8021_86C8 => proc_course(r, gobj),
        0x8021_87A8 => init_lights(r, gobj),
        0x8021_8888 => proc_lights(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_82C8 => on_init(r),
        0x8021_8330 => on_load(r),
        0x8021_8378 => on_start(r),
        _ => return false,
    }
    true
}

/// The backdrop's process and the objects' destroy callbacks, which do nothing.
#[doc(alias = "grPushOn_80218598")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The objects' predicates (`callback1`), which nothing calls and answer false (dead,
/// `ledger/dead.txt`).
pub fn answers_false(_gobj: GObjId) -> bool {
    false
}

/// The stage's `callback4`, `grPushOn_802183DC`: false.
pub fn callback4() -> bool {
    false
}

/// `grPushOn_802182C4` does nothing (only a demo runs it).
pub fn on_demo_init(_r: &mut Running) {}

/// The stage's objects' setup functions and processes, by map id (`grPushOn_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &PUSHON_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grPushOn_802183E4`, its scale joint's
/// descriptor at `frame` on the stack.
#[doc(alias = "grPushOn_802183E4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The race's setup, `grPushOn_802182C8`: its parameters, the stage's `unk8C` flags (b4 off, b5
/// on), maps 0 and 1, and collision joint 0 off.
#[doc(alias = "grPushOn_802182C8")]
pub fn on_init(r: &mut Running) {
    r.game.gr.pushon_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, 1, setup::SETUP_FRAME);
    r.game.geometry_mut().disable_joint(0);
}

/// The race's load, `grPushOn_80218330`: the lights' object (map 2), then the camera centred on
/// the first fighter, and there at once.
#[doc(alias = "grPushOn_80218330")]
pub fn on_load(r: &mut Running) {
    create(r, 2, LOAD_FRAME);
    let Some(gobj) = p1_fighter(r.game) else {
        return;
    };
    let pos = r.game.ft_pos(gobj);
    set_cam_offset(r.game, pos.x, pos.y);
    super::super::camera::snap(r);
}

fn param(game: &Game) -> DataRef {
    game.gr.pushon_yakumono.expect("grpushon.c: yakumono_param")
}

/// The race's start, `grPushOn_80218378`: the enemy spawner, without enemies, then, unless a
/// draw of one in the parameters' `x18` (none for 0) comes up nonzero, a trophy slot at a random
/// trophy point.
#[doc(alias = "grPushOn_80218378")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
    let p = param(r.game);
    let n = r.data.u32(p.at(0x18)) as i32;
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

/// The backdrop's setup (map 0), `grPushOn_802184CC`: its animation, the camera limits and
/// blast zones measured, and each camera limit set halfway out to its blast zone, from the
/// camera's centre.
#[doc(alias = "grPushOn_802184CC")]
pub fn init_backdrop(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let c = stage::cam_center(r.game);
    let half = |a: f32, b: f32, off: f32| mem(fp::fmsubs(0.5, fp::fadds(reg(a), reg(b)), reg(off)));
    let top = half(stage::cam_top(r.game), stage::blast_top(r.game), c.y);
    set_cam_top(r.game, top);
    let bottom = half(stage::cam_bottom(r.game), stage::blast_bottom(r.game), c.y);
    set_cam_bottom(r.game, bottom);
    let left = half(stage::cam_left(r.game), stage::blast_left(r.game), c.x);
    set_cam_left(r.game, left);
    let right = half(stage::cam_right(r.game), stage::blast_right(r.game), c.x);
    set_cam_right(r.game, right);
}

/// The course's setup (map 1), `grPushOn_802185A0`: the spikes as a fighter device of type 3
/// (`ftCo_800C07F8`), its animation with parts 0x77A4's looping, no goal landed on, and the
/// goal platforms' collision joints (3 to 5) calling back on a landing.
#[doc(alias = "grPushOn_802185A0")]
pub fn init_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    r.game
        .gr
        .devices
        .register_coll(gobj, 3, CodeRef(SPIKES_DEVICE));
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::apply(r.game, j, true, 0x77A4, anime::AnimOp::Loop);
    r.game.grounds[g.0].u.map().b0 = false;
    for joint in 3..=5 {
        r.game.geometry_mut().set_ledge_callback(
            joint,
            Some(CollUser::Ground(g)),
            Some(CodeRef(GOAL_TOUCHED)),
        );
    }
}

/// The course's check of a goal point (the stage watcher's `stage_info.x90`), `fn_80218678`:
/// whether a goal platform was landed on since the course's last process.
#[doc(alias = "fn_80218678")]
pub fn goal_check(game: &mut Game) -> bool {
    let Some(gobj) = super::ground::map_gobj(game, MAP_COURSE) else {
        return false;
    };
    let g = ground_of(game, gobj);
    game.grounds[g.0].u.map().b0
}

/// The course's process (map 1), `grPushOn_802186C8`: the watcher asked for the goal point the
/// first fighter stands in (a box of 25 by 20, with [`goal_check`]), the camera centred on the
/// first fighter (no lower than 0, no higher than 800 in the stage's scale), the wind, and the
/// goal landing forgotten.
#[doc(alias = "grPushOn_802186C8")]
pub fn proc_course(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    super::ground::watch_points(r.game, Some(CodeRef(GOAL_CHECK)), 25.0, 20.0);
    if let Some(fighter) = p1_fighter(r.game) {
        let mut pos = r.game.ft_pos(fighter);
        if reg(pos.y) < 0.0 {
            pos.y = 0.0;
        } else if reg(pos.y) > fp::fmuls(800.0, reg(scale(r.game))) {
            pos.y = mem(fp::fmuls(800.0, reg(scale(r.game))));
        }
        set_cam_offset(r.game, pos.x, pos.y);
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    r.game.grounds[g.0].u.map().b0 = false;
}

/// A goal platform's collision callback, `fn_802190A0`: a fighter (`x34_flags.b1234` 1)
/// landing on it or standing on it (ground kinds 1 and 2) marks the course's goal landing.
#[doc(alias = "fn_802190A0")]
pub fn goal_touched(gp: &mut super::super::structs::Ground, b1234: u32, kind: i32) {
    if b1234 == 1 && (kind.wrapping_sub(1) as u32) <= 1 {
        gp.u.map().b0 = true;
    }
}

/// The lights' object (map 2), `grPushOn_802187A8`: the first object on render link 4 (the
/// fighters' lights, `HSD_GObjGXLinkHead` read as a GObj, whose `next_gx` is link 4's head),
/// its point lights set up from the table, each of its lights and its flags kept, the two added
/// lights, and the lights' process run once.
#[doc(alias = "grPushOn_802187A8")]
pub fn init_lights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let lights = r.game.world.first_rendered(4);
    r.game.grounds[g.0].u.pushon().gobj = lights;
    let lights = lights.expect("grpushon.c: render link 4 is empty");
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

/// One of the course's point lights (`grPushOn_LightConfig`): its color, its place in the
/// stage's units, its reference brightness and distance and its distance function.
struct LightConfig {
    color: [u8; 4],
    pos: [f32; 3],
    ref_br: f32,
    ref_dist: f32,
    dist_func: i32,
}

/// `light_configs`.
const LIGHT_CONFIGS: [LightConfig; 9] = [
    LightConfig {
        color: [0xBE, 0xE6, 0xE6, 0xFF],
        pos: [-12.0, 735.0, -20.0],
        ref_br: 0.1,
        ref_dist: 800.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xF0, 0xF0, 0xF0, 0xFF],
        pos: [250.0, 708.0, -30.0],
        ref_br: 0.4,
        ref_dist: 500.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xC3, 0x56, 0xCD, 0xFF],
        pos: [550.0, 655.0, -50.0],
        ref_br: 0.2,
        ref_dist: 600.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xF0, 0xF0, 0xF0, 0xFF],
        pos: [860.0, 550.0, -30.0],
        ref_br: 0.1,
        ref_dist: 700.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0x7C, 0xE7, 0xFF, 0xFF],
        pos: [1150.0, 675.0, -60.0],
        ref_br: 0.2,
        ref_dist: 800.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xFF, 0x4B, 0x4B, 0xFF],
        pos: [1400.0, 370.0, -60.0],
        ref_br: 0.01,
        ref_dist: 500.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xE6, 0xE6, 0xE6, 0xFF],
        pos: [1580.0, 20.0, -30.0],
        ref_br: 0.1,
        ref_dist: 500.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xFF, 0x6E, 0x6E, 0xFF],
        pos: [2093.0, 120.0, -20.0],
        ref_br: 0.3,
        ref_dist: 600.0,
        dist_func: 3,
    },
    LightConfig {
        color: [0xBE, 0xE6, 0xFF, 0xFF],
        pos: [1700.0, 630.0, -20.0],
        ref_br: 0.2,
        ref_dist: 800.0,
        dist_func: 3,
    },
];

/// The course's point lights, `grPushOn_802190D0`: the lights after a set's first, up to nine,
/// each made a diffuse point light (it must be a point light already) with the table's color,
/// place and distance falloff, in the stage's scale.
#[doc(alias = "grPushOn_802190D0")]
pub fn set_up_lights(game: &mut Game, gobj: GObjId) {
    let cur = first_lobj(game, gobj).expect("grpushon.c: the fighters' lights");
    let s = reg(scale(game));
    let mut l = game.objs.lobjs[cur].next;
    let mut i = 0;
    while i < 9
        && let Some(o) = l
    {
        let e = &LIGHT_CONFIGS[i];
        assert!(
            lobj::light_type(&game.objs.lobjs, o) == lobj::POINT,
            "grpushon.c:698: HSD_LObjGetType(lobj)==LOBJ_POINT"
        );
        game.objs.lobjs[o].flags = lobj::POINT | lobj::DIFFUSE;
        lobj::set_color(&mut game.objs.lobjs, o, e.color);
        let pos = Vec3::new(
            mem(fp::fmuls(reg(e.pos[0]), s)),
            mem(fp::fmuls(reg(e.pos[1]), s)),
            mem(fp::fmuls(reg(e.pos[2]), s)),
        );
        lobj::set_position(&mut game.objs, o, pos);
        let ref_dist = mem(fp::fmuls(s, reg(e.ref_dist)));
        lobj::set_dist_attn(
            &mut game.objs.lobjs,
            Some(o),
            ref_dist,
            e.ref_br,
            e.dist_func,
        );
        i += 1;
        l = game.objs.lobjs[o].next;
    }
}

/// One of the two lights the course adds (`grPushOn_803E7B74`, `grPushOn_803E7B90`): its
/// flags, whether it starts hidden, and its point parameters as `HSD_LObjLoadDesc` reads them.
/// Each descriptor's parameters point at a lone float, 16 (`grPushOn_804D4934`,
/// `grPushOn_804D4948`), and the load reads an `HSD_LightPointDesc` there: that float as the
/// reference brightness, then the two words after it in the executable's small data (the
/// strings `"lobj.h"` and `"jobj"` that follow) as the reference distance and the distance
/// function (docs/melee-bugs.md).
struct AddedLight {
    flags: u16,
    hidden: bool,
    ref_br: u32,
    ref_dist: u32,
    dist_func: u32,
}

/// `grPushOn_803E7B74`: a diffuse point light, hidden, which follows the first fighter.
const FOLLOWER: AddedLight = AddedLight {
    flags: lobj::POINT | lobj::DIFFUSE,
    hidden: true,
    ref_br: 0x4180_0000,
    // "lobj", ".h\0\0" (0x804D4938).
    ref_dist: 0x6C6F_626A,
    dist_func: 0x2E68_0000,
};

/// `grPushOn_803E7B90`: a specular point light, which takes the second nearest light's place.
const SECOND: AddedLight = AddedLight {
    flags: lobj::POINT | lobj::SPECULAR,
    hidden: false,
    ref_br: 0x4180_0000,
    // 0, "jobj" (0x804D494C).
    ref_dist: 0,
    dist_func: 0x6A6F_626A,
};

/// Adds a light at the end of the set's chain, `grPushOn_80218ED4` and `grPushOn_80218FC0`: the
/// descriptor loaded (white, no position), placed at (0, 100, 0) and, the first, hidden.
#[doc(alias = "grPushOn_80218ED4")]
#[doc(alias = "grPushOn_80218FC0")]
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

/// The lights' process (map 2), `grPushOn_80218888`.
///
/// Each kept light's distance from the first fighter (in x and y; a spot light's in x alone; an
/// ambient light -100), at least 10, sorts the lights nearest first; the five nearest are shown
/// and the rest hidden. The hidden follower moves 20 from the first fighter toward the lights
/// past the nearest (each weighed by the second nearest's distance over its own, less when
/// under 100 above the fighter, none below it), looking at the fighter. The specular light
/// takes the second nearest's place, interest and flags (made specular, not diffuse) and its
/// color, dimmed while the third nearest is within 50 of it.
#[doc(alias = "grPushOn_80218888")]
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
    let light = |i: usize| lobjs[i].expect("grpushon.c: a kept light");
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
    // The second and third nearest are read whatever the count; every set the race lights
    // has more than three lights.
    assert!(
        n > 2,
        "grpushon.c: {n} lights, the third nearest a stack word"
    );
    for (i, &s) in sorted.iter().enumerate().take(n) {
        if i < 5 {
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
        if reg(lp.y) < 100.0 {
            if reg(lp.y) < 0.0 {
                weight = 0.0;
            } else {
                weight = fp::fmuls(weight, fp::fdivs(reg(lp.y), 100.0));
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
    let point = point.expect("grpushon.c: point_light");
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
    let spec = spec.expect("grpushon.c: spot_light");
    lobj::set_color(&mut r.game.objs.lobjs, spec, color);
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let at = lobj::position(&mut r.game.objs, r.data, Some(second)).unwrap_or(zero);
    lobj::set_position(&mut r.game.objs, spec, at);
    let at = lobj::interest(&mut r.game.objs, r.data, Some(second)).unwrap_or(zero);
    lobj::set_interest(&mut r.game.objs, spec, at);
    let flags = r.game.objs.lobjs[second].flags;
    r.game.objs.lobjs[spec].flags = flags & !lobj::DIFFUSE | lobj::SPECULAR;
}

/// The spikes' boxes (`grPushOn_803E7CCC`, by their bits: 1441.429, 1645.629 and 340.575; 1306.433,
/// 1361.633 and 361.574; 1193.435, 1268.635 and 367.574; 1121.160, 1157.682 and 354.607): each
/// one's left and right edges and its top, in the stage's units, 50 deep.
const SPIKES: [[f32; 3]; 4] = [
    [
        f32::from_bits(0x44B4_2DBA),
        f32::from_bits(0x44CD_B420),
        f32::from_bits(0x43AA_4990),
    ],
    [
        f32::from_bits(0x44A3_4DDD),
        f32::from_bits(0x44AA_3443),
        f32::from_bits(0x43B4_C97F),
    ],
    [
        f32::from_bits(0x4495_2DE8),
        f32::from_bits(0x449E_944E),
        f32::from_bits(0x43B7_C976),
    ],
    [
        f32::from_bits(0x448C_2520),
        f32::from_bits(0x4490_B5D3),
        f32::from_bits(0x43B1_4DAC),
    ],
];

/// The spikes' fighter device, `fn_802192A4`: a fighter inside one of the boxes takes the
/// parameters' hit (`x0`).
#[doc(alias = "fn_802192A4")]
pub fn spikes_device(r: &mut Running, fighter: GObjId) -> Option<DataRef> {
    let s = reg(scale(r.game));
    let pos = r.game.ft_pos(fighter);
    for b in &SPIKES {
        if fp::fmuls(s, reg(b[0])) < reg(pos.x)
            && fp::fmuls(s, reg(b[1])) > reg(pos.x)
            && fp::fmuls(s, fp::fadds(fp::fneg(50.0), reg(b[2]))) < reg(pos.y)
            && fp::fmuls(s, reg(b[2])) > reg(pos.y)
        {
            let hit = r.data.ptr(param(r.game));
            return Some(hit.expect("grpushon.c: the spikes' hit"));
        }
    }
    None
}

/// The stage's lines' hits, `grPushOn_80219458`: collision joint 1's lines the parameters'
/// `x4`, joint 2's floors, ceilings, right and left walls `x8`, `xC`, `x10` and `x14`.
#[doc(alias = "grPushOn_80219458")]
pub fn line_hazard(game: &Game, data: &Archives, line: i32) -> Option<DataRef> {
    if line == -1 {
        return None;
    }
    let geo = game.geometry.as_ref().expect("no collision geometry");
    let joint = geo.joint_from_line(line);
    let p = param(game);
    match joint {
        1 => {
            let _ = geo.line_kind(line);
            data.ptr(p.at(4))
        }
        2 => match geo.line_kind(line) {
            crate::mp::FLOOR => data.ptr(p.at(8)),
            crate::mp::CEILING => data.ptr(p.at(0xC)),
            crate::mp::RIGHT_WALL => data.ptr(p.at(0x10)),
            crate::mp::LEFT_WALL => data.ptr(p.at(0x14)),
            _ => None,
        },
        _ => None,
    }
}

/// The stage's shadow check, `grPushOn_80219528`: every joint receives shadows.
pub fn check_shadow_render() -> bool {
    true
}

/// A goal's special score and its kind, `grPushOn_80219204` (Classic's clear screen): the
/// parameters' entry for goal point `point` (from 0x99).
#[doc(alias = "grPushOn_80219204")]
pub fn goal_score(game: &Game, data: &Archives, point: i32) -> (i32, i32) {
    let e = param(game).at(0x1C + 8 * point.wrapping_sub(0x99) as u32);
    (data.u32(e) as i32, i32::from(data.u16(e.at(4)) as i16))
}

/// The race's time for a character, `grPushOn_80219230` (Classic's race setup): the parameters'
/// lookup's value for `key`, up to 33 entries or a key of -1; a key not found stops the game.
pub fn race_time(game: &Game, data: &Archives, key: i32) -> i32 {
    let p = param(game).at(0x10C);
    for i in 0..0x21u32 {
        let e = p.at(8 * i);
        let k = data.u32(e) as i32;
        if k == -1 {
            break;
        }
        if k == key {
            return data.u32(e.at(4)) as i32;
        }
    }
    panic!("grpushon.c:861: 0");
}
