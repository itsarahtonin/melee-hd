//! Kongo Jungle, `melee/gr/grkongo.c`: the stage with its rope bridge (map 10), the barrel
//! that swings below it (map 5), the floating platforms that drift by (maps 1 and 2), the
//! river's path that carries a Klap Trap (map 11), the logs that float past behind (maps 7, 8
//! and 9), the background (maps 3, 4 and 6) and the sky (map 0).
//!
//! The bridge is fifteen planks on a static table, each swinging back toward its rest angle and
//! pulled by its neighbours, kicked by whatever lands on it; its collision lines follow their
//! heights and its two end posts lean with the slope. The stage's process draws an event now
//! and then: a Klap Trap on the river (an enemy item, `it::kinds::klap_trap`), a box on it, or a
//! platform or a log. The barrel turns in spurts and swings as Kongo Jungle N64's does; it
//! catches a Klap Trap that comes near or a fighter that falls in, and shoots it out.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running, UserData};
use super::super::structs::CollData;
use super::data::{KONGO_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_801D8134`, the barrel's catch test for a fighter.
pub const BARREL_CATCH: u32 = 0x801D_8134;
/// `fn_801D7700`, the bridge's landing callback.
pub const BRIDGE_LANDED: u32 = 0x801D_7700;
/// `fn_801D7E60`, a floating platform's landing callback.
pub const PLATFORM_LANDED: u32 = 0x801D_7E60;
/// `fn_801D542C`, the stage's start hook.
const START_HOOK: u32 = 0x801D_542C;
/// Where an event's objects' call (the stage's process, `grKongo_801D7BBC`, then
/// `grKongo_801D5340`) puts their scale joints' descriptors on the stack
/// (`setup::SETUP_FRAME`): 0x70 below the process loop's frame, against Battlefield's
/// background's 0x60.
const EVENT_FRAME: u32 = 0x804E_E9E0;
/// The barrel's device type (`ftCo_800C0764`'s second argument).
const DEVICE_BARREL: u32 = 2;
/// The bridge's collision joint.
const BRIDGE_JOINT: i32 = 4;

/// `MTXDegToRad`'s factor as the code loads it (a single).
const DEG_TO_RAD: f32 = 0.017_453_292;

/// `grKongo_YakumonoParam`, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
}

/// The events' wait (`x0` to `x4`).
const EVENT_WAIT_A: u32 = 0x0;
const EVENT_WAIT_B: u32 = 0x4;
/// The events' weights: a Klap Trap (`x8`), a box (`xC`), a platform or a log (`x10`, split by
/// `x18`); a repeat is drawn again unless a draw passes `x1C`.
const KLAP_WEIGHT: u32 = 0x8;
const BOX_WEIGHT: u32 = 0xC;
const SCENERY_WEIGHT: u32 = 0x10;
const PATH_SPEED: u32 = 0x14;
const PLATFORM_SHARE: u32 = 0x18;
const REPEAT: u32 = 0x1C;
/// The barrel's hold (`x20` to `x24`) and reach (`x28`).
const HOLD_A: u32 = 0x20;
const HOLD_B: u32 = 0x24;
const BARREL_IN: u32 = 0x28;
const WAIT_A: u32 = 0x2C;
const WAIT_B: u32 = 0x30;
const ROT_ACCEL: u32 = 0x34;
const ROT_MAX: u32 = 0x38;
const ROLL_A: u32 = 0x3C;
const ROLL_B: u32 = 0x40;
/// The eight directions' weights, `s16`s from down-left round to down.
const RATES: u32 = 0x44;
/// The swing's pause (raw words `x54`, `x58`), acceleration, top speed and run.
const PAUSE_A: u32 = 0x54;
const PAUSE_B: u32 = 0x58;
const MOVE_ACCEL: u32 = 0x5C;
const MOVE_MAX: u32 = 0x60;
const RUN_A: u32 = 0x64;
const RUN_B: u32 = 0x68;
/// The shot's hit description's fields, raw words.
const SHOT_HIT: u32 = 0x6C;
const COLOR_SCRIPT: u32 = 0x84;
/// A platform sinks after this many landings (`x88`) or ticks (`x8C`).
const PLATFORM_LANDINGS: u32 = 0x88;
const PLATFORM_TICKS: u32 = 0x8C;
/// The bridge: the planks' swing limit (`x90`), their pull home (`x94`) and speed limit
/// (`x98`), the slack between neighbours (`x9C`) and its pull (`xA0`), a landing's kick
/// (`xA4`), the posts' lean (`xA8`) and tilt limit (`xAC`), the tilt that jolts the posts
/// (`xB0`), and the posts' sway step (`xB4`) and limit (`xB8`).
const PLANK_LIMIT: u32 = 0x90;
const PLANK_PULL: u32 = 0x94;
const PLANK_SPEED: u32 = 0x98;
const SLACK: u32 = 0x9C;
const SLACK_PULL: u32 = 0xA0;
const KICK: u32 = 0xA4;
const POST_LEAN: u32 = 0xA8;
const TILT_LIMIT: u32 = 0xAC;
const JOLT: u32 = 0xB0;
const SWAY_STEP: u32 = 0xB4;
const SWAY_LIMIT: u32 = 0xB8;

fn param(game: &Game) -> Param {
    Param(game.gr.kongo_yakumono.expect("grkongo.c: yakumono_param"))
}

/// One of the bridge's planks (`grKg_803E188C`'s entries): the joint it turns (`unk4`), its
/// angle (`unkC`), its swing's speed (`unk10`) and its height at the bridge's middle line
/// (`unk14`). Each entry's part (`unk0`), flag (`unk2`, 0 in all fifteen) and rest angle
/// (`unk8`) are constants: [`PLANKS`].
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Plank {
    pub jobj: Option<JObjId>,
    pub angle: f32,
    pub speed: f32,
    pub height: f32,
}

/// The planks' parts and rest angles (`grKg_803E188C`'s constant fields).
pub const PLANKS: [(i32, f32); 15] = [
    (8, 0.069_813_17),
    (9, 0.0),
    (0xA, -0.052_359_88),
    (0x17, -0.104_719_76),
    (0x18, -0.139_626_34),
    (0x19, -0.157_079_64),
    (0x1A, -0.209_439_52),
    (0x1B, -0.235_619_44),
    (0x1C, -0.183_259_58),
    (0x1D, -0.130_899_7),
    (0x1E, -0.078_539_82),
    (0x1F, -0.026_179_94),
    (0x20, 0.0),
    (7, 0.052_359_88),
    (0x2D, 0.104_719_76),
];

/// grkongo.c's state outside its objects: the bridge's planks (`grKg_803E188C`, a static table
/// that keeps its values from one match to the next) and its end posts (`grKg_804D6984`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    pub planks: [Plank; 15],
    pub posts: [Option<JObjId>; 2],
}

/// A stage object's process or callback, by address: Kongo Jungle's.
#[melee_callbacks::callbacks(Proc)]
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801D_55D4 | 0x801D_5FA4 | 0x801D_5FDC | 0x801D_5FE0 | 0x801D_6030 | 0x801D_6034
        | 0x801D_606C | 0x801D_6070 | 0x801D_6378 | 0x801D_6518 | 0x801D_69AC => empty(r, gobj),
        0x801D_5490 => init_stage(r, gobj),
        0x801D_557C => proc_stage(r, gobj),
        START_HOOK => start_bridge(r, gobj),
        0x801D_55D8 => init_barrel(r, gobj, &BARREL),
        0x801D_577C => proc_barrel(r, gobj),
        0x801D_5FA8 | 0x801D_6038 => init_anim(r, gobj),
        0x801D_5FE4 => init_front(r, gobj),
        0x801D_6074 => init_platform(r, gobj),
        0x801D_6198 => proc_platform(r, gobj),
        0x801D_637C => init_log(r, gobj),
        0x801D_64BC => proc_log(r, gobj),
        0x801D_651C => init_path(r, gobj),
        0x801D_6668 => proc_path(r, gobj),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grKongo_801D55D4")]
#[doc(alias = "grKongo_801D5FA4")]
#[doc(alias = "grKongo_801D5FDC")]
#[doc(alias = "grKongo_801D5FE0")]
#[doc(alias = "grKongo_801D6030")]
#[doc(alias = "grKongo_801D6034")]
#[doc(alias = "grKongo_801D606C")]
#[doc(alias = "grKongo_801D6070")]
#[doc(alias = "grKongo_801D6378")]
#[doc(alias = "grKongo_801D6518")]
#[doc(alias = "grKongo_801D69AC")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grKongo_801D5340`.
#[doc(alias = "grKongo_801D5340")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grKg_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &KONGO_CALLBACKS[map_id as usize]
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Kongo Jungle's setup, `grKongo_801D523C`: its parameters, maps 0, 10, 5, 3, 6 and 4,
/// camera limits and blast zones, joints 0 and 1 (the platforms') off, and the river's sound
/// unless the stage is the single-player one's or the match is a mode without it.
#[doc(alias = "grKongo_801D523C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.kongo_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 10, 5, 3, 6, 4] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    r.game.geometry_mut().disable_joint(0);
    r.game.geometry_mut().disable_joint(1);
    if super::stage::stkind(r.game) != 0x3D && r.game.vs_x6() == 0 {
        r.stage_sound(0x5_A551);
    }
}

/// `grKongo_801D5238` does nothing (only a demo runs it).
#[doc(alias = "grKongo_801D5238")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grKongo_801D52F8` does nothing.
#[doc(alias = "grKongo_801D52F8")]
pub fn on_load(_r: &mut Running) {}

/// Kongo Jungle's start, `grKongo_801D52FC`: the barrel registered as a device that catches
/// fighters, and the enemy spawner, without enemies.
#[doc(alias = "grKongo_801D52FC")]
pub fn on_start(r: &mut Running) {
    let barrel = map_gobj(r.game, 5).expect("grkongo.c: the barrel");
    super::oldkongo::register_catch(r.game, barrel, DEVICE_BARREL, CodeRef(BARREL_CATCH));
    zako::start(r.game, r.data, None);
}

/// An object that only animates (maps 3 and 4), `grKongo_801D5FA8` and `grKongo_801D6038`.
#[doc(alias = "grKongo_801D5FA8")]
#[doc(alias = "grKongo_801D6038")]
pub fn init_anim(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The front scenery (map 6), `grKongo_801D5FE4`: its animation, drawn in the front layer.
#[doc(alias = "grKongo_801D5FE4")]
pub fn init_front(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The stage's setup (map 10), `grKongo_801D5490`: its collision and animation, it receives
/// shadows, no event yet, the bridge's posts' joints and the bridge at rest, the start hook
/// queued, and the collision brought up to date.
#[doc(alias = "grKongo_801D5490")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b5 = true;
    let joints = [0x13, 0xD, 0x2B, 0x28].map(|d| nth_joint(r.game, gobj, d));
    let v = r.game.grounds[g.0].u.kongo();
    v.xE4 = -1;
    v.xE6 = -1;
    v.xC4 = 0.0;
    v.xC8 = 0.0;
    v.xCC = joints[0];
    v.xD0 = joints[1];
    v.xD4 = 0.0;
    v.xD8 = 0.0;
    v.xDC = joints[2];
    v.xE0 = joints[3];
    init_bridge(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    update_map_coll(r.game, r.data, gobj);
}

/// The stage's start hook, `fn_801D542C`: the bridge's landing callback, and the first
/// event's wait.
#[doc(alias = "fn_801D542C")]
pub fn start_bridge(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.geometry_mut().set_ledge_callback(
        BRIDGE_JOINT,
        Some(super::CollUser::Ground(g)),
        Some(CodeRef(BRIDGE_LANDED)),
    );
    event_wait(r, g);
}

/// A wait between the events' bounds, `(s16)(range * rand + min)`.
fn event_wait(r: &mut Running, g: GroundId) {
    let f = reg(r.game.objs.rng.draw_f32());
    let p = param(r.game);
    let min = reg(p.f(r.data, EVENT_WAIT_A));
    let range = fp::fsubs(reg(p.f(r.data, EVENT_WAIT_B)), min);
    r.game.grounds[g.0].u.kongo().xE4 = fp::fctiwz(fp::fmadds(range, f, min)) as i16;
}

/// The stage's process (map 10), `grKongo_801D557C`: the bridge, the posts' sway, the events,
/// the wind sources age and the collision follows the model, and the bridge's islands are
/// rebuilt.
#[doc(alias = "grKongo_801D557C")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    bridge(r, gobj, false);
    sway(r, gobj, false);
    events(r, gobj);
    super::ground::quake_list_tick(r.game);
    update_map_coll(r.game, r.data, gobj);
    r.game.geometry_mut().update_joint_islands(BRIDGE_JOINT);
}

/// The bridge's setup, `grKongo_801D69B0`: each plank's joint, at its rest angle; the posts'
/// joints; the bridge and the posts brought to rest; and the bridge's joint kept in every
/// collision search.
#[doc(alias = "grKongo_801D69B0")]
pub fn init_bridge(r: &mut Running, gobj: GObjId) {
    for (i, &(part, rest)) in PLANKS.iter().enumerate() {
        let j = nth_joint(r.game, gobj, part);
        let p = &mut r.game.gr.kongo.planks[i];
        p.jobj = j;
        p.angle = rest;
        set_rotation_x(r.game, j, rest);
    }
    r.game.gr.kongo.posts = [nth_joint(r.game, gobj, 0xB), nth_joint(r.game, gobj, 0x21)];
    bridge(r, gobj, true);
    sway(r, gobj, true);
    r.game.geometry_mut().set_always_near(BRIDGE_JOINT);
}

/// `HSD_JObjSetRotationX` on a joint that may be missing (the original asserts).
fn set_rotation_x(game: &mut Game, j: Option<JObjId>, v: f32) {
    let j = j.expect("jobj.h: jobj");
    jobj::set_rotation_x(&mut game.objs.jobjs, j, v);
}

fn set_rotation_z(game: &mut Game, j: Option<JObjId>, v: f32) {
    let j = j.expect("jobj.h: jobj");
    jobj::set_rotation_z(&mut game.objs.jobjs, j, v);
}

/// `Ground_ClampSymmetric`.
fn clamp_symmetric(v: f64, limit: f64) -> f64 {
    if v > limit {
        limit
    } else if v < fp::fneg(limit) {
        fp::fneg(limit)
    } else {
        v
    }
}

/// The planks' swing, `grKongo_801D6AFC`: each plank's speed eases (by 0.99) after a pull
/// toward its rest angle once it is past the parameters' slack, is limited, and turns it, the
/// angle limited too; then neighbours more than the slack apart pull each other.
#[doc(alias = "grKongo_801D6AFC")]
pub fn swing_planks(game: &mut Game, data: &Archives) {
    let p = param(game);
    let deg = reg(DEG_TO_RAD);
    let pull = reg(p.f(data, PLANK_PULL));
    let planks = &mut game.gr.kongo.planks;
    for (pl, &(_, rest)) in planks.iter_mut().zip(PLANKS.iter()) {
        // The inline `rad_compare_b`.
        let a = fp::fsubs(reg(pl.angle), reg(rest));
        let limit = fp::fmuls(deg, pull);
        let toward = if a > limit {
            fp::fneg(limit)
        } else if a < fp::fneg(limit) {
            limit
        } else {
            0.0
        };
        pl.speed = mem(fp::frsp(fp::fmul(0.99, fp::fadds(toward, reg(pl.speed)))));
    }
    // A pass that adds nothing (the original sums an array of zeros in).
    for pl in planks.iter_mut() {
        pl.speed = mem(fp::fadds(reg(pl.speed), 0.0));
    }
    let max = fp::fmuls(deg, reg(p.f(data, PLANK_SPEED)));
    for pl in planks.iter_mut() {
        pl.speed = mem(clamp_symmetric(reg(pl.speed), max));
    }
    let swing = fp::fmuls(deg, reg(p.f(data, PLANK_LIMIT)));
    for pl in planks.iter_mut() {
        pl.angle = mem(fp::fadds(reg(pl.angle), reg(pl.speed)));
        pl.angle = mem(clamp_symmetric(reg(pl.angle), swing));
    }
    let slack = fp::fmuls(deg, reg(p.f(data, SLACK)));
    let k = reg(p.f(data, SLACK_PULL));
    let mut pulls = [0.0f64; 15];
    for i in 0..15 {
        let add = |other: usize, pulls: &mut [f64; 15]| {
            let input = fp::fsubs(reg(planks[other].angle), reg(planks[i].angle));
            if input > slack {
                pulls[i] = fp::fmadds(k, fp::fsubs(input, slack), pulls[i]);
            } else if input < fp::fneg(slack) {
                pulls[i] = fp::fmadds(k, fp::fadds(input, slack), pulls[i]);
            }
        };
        if i != 0 {
            add(i - 1, &mut pulls);
        }
        if i != 14 {
            add(i + 1, &mut pulls);
        }
    }
    for (pl, &d) in planks.iter_mut().zip(pulls.iter()) {
        if d != 0.0 {
            pl.angle = mem(fp::fadds(reg(pl.angle), d));
        }
    }
}

/// A post's tilt from the planks' heights about plank `i` (the inline `grKongo_calc_angle`):
/// the mean of the slopes on either side, as an angle (π/4 per unit), limited.
fn post_tilt(game: &Game, data: &Archives, i: usize) -> f64 {
    let p = param(game);
    let h = |k: usize| reg(game.gr.kongo.planks[k].height);
    let left = fp::fdivs(fp::fsubs(h(i), h(i - 1)), 6.0);
    let right = fp::fdivs(fp::fsubs(h(i + 1), h(i)), 6.0);
    let mut a = fp::frsp(fp::fmul(
        std::f64::consts::FRAC_PI_4,
        fp::fmul(0.5, fp::fadds(left, right)),
    ));
    let limit = fp::fmuls(reg(DEG_TO_RAD), reg(p.f(data, TILT_LIMIT)));
    if a > limit {
        a = limit;
    } else if a < fp::fneg(limit) {
        a = fp::fneg(limit);
    }
    a
}

/// The bridge, `grKongo_801D7134`: the planks swing, each plank's height follows its angle
/// (37.8 times the tangent), the posts lean with the end planks and tilt with the slope about
/// them (a tilt past the parameters' jolts the posts' sway), the planks' joints turn, and the
/// bridge's collision lines (40 to 70) move to their planks' heights.
#[doc(alias = "grKongo_801D7134")]
pub fn bridge(r: &mut Running, gobj: GObjId, _first: bool) {
    let g = ground_of(r.game, gobj);
    swing_planks(r.game, r.data);
    for pl in r.game.gr.kongo.planks.iter_mut() {
        let t = reg(crate::math::tan(mem(fp::fneg(reg(pl.angle)))));
        pl.height = mem(fp::frsp(fp::fmul(37.8, t)));
    }
    let p = param(r.game);
    let deg = reg(DEG_TO_RAD);
    let lean = fp::fsubs(
        fp::fdivs(
            fp::fmuls(deg, reg(p.f(r.data, POST_LEAN))),
            fp::fmuls(deg, reg(p.f(r.data, PLANK_LIMIT))),
        ),
        1.0,
    );
    let jolt = fp::fmuls(deg, reg(p.f(r.data, JOLT)));
    for (post, plank) in [(0, 2), (1, 12)] {
        let displacement = fp::fmuls(reg(r.game.gr.kongo.planks[plank].angle), lean);
        let angle = post_tilt(r.game, r.data, plank);
        let j = r.game.gr.kongo.posts[post];
        set_rotation_x(r.game, j, mem(displacement));
        let old = reg(r.game.objs.jobjs[j.expect("jobj.h: jobj")].rotate.z);
        set_rotation_z(r.game, j, mem(angle));
        let d = fp::fsubs(angle, old);
        let ad = if d < 0.0 { fp::fneg(d) } else { d };
        if ad > jolt {
            let v = r.game.grounds[g.0].u.kongo();
            if post == 0 {
                v.xC8 = mem(fp::fneg(d));
            } else {
                v.xD8 = mem(fp::fneg(d));
            }
        }
    }
    for i in 0..15 {
        let (j, a) = {
            let pl = &r.game.gr.kongo.planks[i];
            (pl.jobj, pl.angle)
        };
        set_rotation_x(r.game, j, a);
    }
    r.game.geometry_mut().settle_vertices(BRIDGE_JOINT);
    for i in 0..15i32 {
        let h = r.game.gr.kongo.planks[i as usize].height;
        let line = 0x28 + 2 * i;
        let geo = r.game.geometry_mut();
        geo.offset_line(line, 0.0, h, 0.0, h);
        if i == 0 {
            geo.offset_line(line - 1, 0.0, h, 0.0, h);
        } else if i == 14 {
            geo.offset_line(line + 1, 0.0, h, 0.0, h);
        }
    }
}

/// The planks the bridge's landing callback kicked during a collision pass (`fn_801D7700`),
/// given their kicks once the pass returns: the callback runs mid-pass with the stage's state
/// out of reach, and nothing in a pass reads a plank's speed.
#[derive(Debug, Default)]
pub struct Kicks(Vec<u8>);

impl Kicks {
    /// The bridge's landing callback, `fn_801D7700`: a fighter or an item (collision kind 1
    /// or 3) landing (ground kind 1) kicks the plank under it, found by its place between
    /// the bridge's end vertices (29 and 26) in fifteen equal parts.
    #[doc(alias = "fn_801D7700")]
    pub fn landed(&mut self, g: &super::Geometry, coll: &CollData, ground_kind: i32) {
        let ty = coll.x34_flags.b1234;
        if !(ty == 1 || ty == 3) || ground_kind != 1 {
            return;
        }
        let (x1, _) = g.vertex_pos(0x1D);
        let (x2, _) = g.vertex_pos(0x1A);
        let size = fp::fdivs(fp::fsubs(reg(x2), reg(x1)), 15.0);
        let ratio = fp::fdivs(fp::fsubs(reg(coll.cur_pos.x), reg(x1)), size);
        let idx = fp::fctiwz(ratio);
        let idx = if idx < 0 {
            0
        } else if idx as u32 >= 15 {
            14
        } else {
            idx
        };
        self.0.push(idx as u8);
    }

    /// Gives the kicks, in order: each adds the parameters' kick (in degrees) to its plank's
    /// speed.
    pub fn apply(self, game: &mut Game, data: &Archives) {
        if self.0.is_empty() {
            return;
        }
        let kick = reg(param(game).f(data, KICK));
        for i in self.0 {
            let pl = &mut game.gr.kongo.planks[i as usize];
            pl.speed = mem(fp::fmadds(reg(DEG_TO_RAD), kick, reg(pl.speed)));
        }
    }
}

/// The posts' sway, `grKongo_801D77E0`: at the setup both at rest; then each of the two
/// (angle `xC4` and speed `xC8`, angle `xD4` and speed `xD8`) decelerates toward upright (at
/// half the step while still moving away), turns, stops at its limit, and comes to rest once
/// both are within a step. The posts' joints follow.
#[doc(alias = "grKongo_801D77E0")]
pub fn sway(r: &mut Running, gobj: GObjId, first: bool) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let deg = reg(DEG_TO_RAD);
    let step_full = fp::fmuls(deg, reg(p.f(r.data, SWAY_STEP)));
    let limit = fp::fmuls(
        deg,
        fp::fnmsubs(
            deg,
            reg(p.f(r.data, TILT_LIMIT)),
            reg(p.f(r.data, SWAY_LIMIT)),
        ),
    );
    let v = r.game.grounds[g.0].u.kongo();
    if first {
        v.xC4 = 0.0;
        v.xC8 = 0.0;
        v.xD4 = 0.0;
        v.xD8 = 0.0;
    } else {
        for (angle, speed) in [(&mut v.xC4, &mut v.xC8), (&mut v.xD4, &mut v.xD8)] {
            let a = reg(*angle);
            if a > 0.0 {
                let step = if reg(*speed) > 0.0 {
                    step_full
                } else {
                    fp::fdivs(step_full, 2.0)
                };
                *speed = mem(fp::fsubs(reg(*speed), step));
            } else if a < 0.0 {
                let step = if reg(*speed) < 0.0 {
                    step_full
                } else {
                    fp::fdivs(step_full, 2.0)
                };
                *speed = mem(fp::fadds(reg(*speed), step));
            }
            *angle = mem(fp::fadds(reg(*angle), reg(*speed)));
            let a = reg(*angle);
            let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
            if a > limit {
                *angle = mem(limit);
                *speed = 0.0;
            } else if a < fp::fneg(limit) {
                *angle = mem(fp::fneg(limit));
                *speed = 0.0;
            } else if abs(a) < step_full && abs(reg(*speed)) < step_full {
                *angle = 0.0;
                *speed = 0.0;
            }
        }
    }
    let v = r.game.grounds[g.0].u.kongo();
    let (a, b) = (v.xC4, v.xD4);
    let (j0, j1, j2, j3) = (v.xCC, v.xD0, v.xDC, v.xE0);
    set_rotation_z(r.game, j0, a);
    set_rotation_z(r.game, j1, a);
    set_rotation_z(r.game, j2, b);
    set_rotation_z(r.game, j3, b);
}

/// The events, `grKongo_801D7BBC`. Once the wait (`xE4`) runs out (none before the start
/// hook's first), a new wait starts and an event is drawn by weight: a Klap Trap on the river
/// (1), a box on it (2, only with boxes on and outside special modes), or a platform (map 1
/// or 2) or a log (map 7, 8 or 9) (3); nothing past the weights. The same event as the last
/// is drawn again unless a random draw passes the parameters'. A Klap Trap or box rides a new
/// river path (map 11), which goes when its rider can't be made.
#[doc(alias = "grKongo_801D7BBC")]
pub fn events(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.kongo();
    if v.xE4 > 0 {
        v.xE4 -= 1;
    }
    if v.xE4 == -1 || v.xE4 != 0 {
        return;
    }
    let p = param(r.game);
    let boxes = |r: &Running| {
        r.game.vs_x6() == 0 && super::super::it::spawn::kind_on(r.game, ItKind::Crate.id())
    };
    let box_w = if boxes(r) {
        reg(p.f(r.data, BOX_WEIGHT))
    } else {
        0.0
    };
    let f = reg(r.game.objs.rng.draw_f32());
    let klap = reg(p.f(r.data, KLAP_WEIGHT));
    let scenery = reg(p.f(r.data, SCENERY_WEIGHT));
    let share = reg(p.f(r.data, PLATFORM_SHARE));
    let total = fp::fadds(klap, box_w);
    let total = fp::fmadds(scenery, share, total);
    let total = fp::fmadds(scenery, fp::fsubs(1.0, share), total);
    let mut roll = fp::fmuls(f, total);
    event_wait(r, g);
    roll = fp::fsubs(roll, klap);
    let mut map = 0;
    let event = if roll < 0.0 {
        1
    } else {
        let box_w = if boxes(r) {
            reg(p.f(r.data, BOX_WEIGHT))
        } else {
            0.0
        };
        roll = fp::fsubs(roll, box_w);
        if roll < 0.0 {
            2
        } else {
            roll = fp::fsubs(roll, fp::fmuls(scenery, share));
            if roll < 0.0 {
                map = if r.game.objs.rng.below(2) != 0 { 1 } else { 2 };
                3
            } else {
                roll = fp::fnmsubs(scenery, fp::fsubs(1.0, share), roll);
                if !(roll < 0.0) {
                    return;
                }
                map = match r.game.objs.rng.below(3) {
                    0 => 7,
                    1 => 8,
                    _ => 9,
                };
                3
            }
        }
    };
    if i32::from(r.game.grounds[g.0].u.kongo().xE6) == event {
        let f = reg(r.game.objs.rng.draw_f32());
        if f > reg(p.f(r.data, REPEAT)) {
            return;
        }
    }
    r.game.grounds[g.0].u.kongo().xE6 = event as i16;
    match event {
        1 => {
            if let Some(path) = create(r, 11, EVENT_FRAME)
                && super::super::it::kinds::klap_trap::spawn(r, Some(path)).is_none()
            {
                setup::destroy(r, Some(path));
            }
        }
        2 => {
            if let Some(path) = create(r, 11, EVENT_FRAME)
                && super::super::it::kinds::crate_box::spawn_floating(r, Some(path)).is_none()
            {
                setup::destroy(r, Some(path));
            }
        }
        _ => {
            create(r, map, EVENT_FRAME);
        }
    }
}

/// A barrel's variables, over Kongo Jungle's layout (`KongoBarrelVars`) or Kongo Jungle N64's
/// (`OldKongoVars`, a shot timer more after `xCE`): its turn's state (`xC4`), what it holds and
/// its shot's state (`xC6`), its swing's state (`xC8`), the hold's countdown (`xCA`), the turn's
/// and the swing's timers (`xCC`, `xCE`), what it holds, the heading it turns to, its angle, its
/// turn's step and speed, a word set to 1 and never read, and the swing's rate.
pub struct BarrelView<'a> {
    pub turn: &'a mut i16,
    pub hold: &'a mut i16,
    pub swing: &'a mut i16,
    pub held_for: &'a mut i16,
    pub turn_timer: &'a mut i16,
    pub swing_timer: &'a mut i16,
    pub keep: &'a mut Option<GObjId>,
    pub heading: &'a mut f32,
    pub angle: &'a mut f32,
    pub step: &'a mut f32,
    pub speed: &'a mut f32,
    pub one: &'a mut f32,
    pub rate: &'a mut f32,
}

/// What Kongo Jungle's and Kongo Jungle N64's barrels differ in. Their turn and swing, setup,
/// directions and catch are one implementation over it (`grkongo.c`'s and `groldkongo.c`'s
/// copies, both originals' aliases on each shared function); each stage keeps its own process's
/// end (what the barrel holds and its shot) and its constant ([`BARREL`] here,
/// `oldkongo::BARREL`).
pub struct Barrel {
    /// The barrel's variables.
    pub vars: for<'a> fn(&'a mut Game, GroundId) -> BarrelView<'a>,
    /// The stage's `yakumono_param` and in it, by offset: the hold's bounds (a fighter's, and on
    /// Kongo Jungle a Klap Trap's), the catch's reach, the turn's wait, acceleration, top speed
    /// and spurt, the directions' weights, the swing's pause, acceleration, top speed and run,
    /// and the catch's color script.
    pub param: fn(&Game) -> DataRef,
    pub hold_a: u32,
    pub hold_b: u32,
    pub reach: u32,
    pub wait_a: u32,
    pub wait_b: u32,
    pub rot_accel: u32,
    pub rot_max: u32,
    pub roll_a: u32,
    pub roll_b: u32,
    pub rates: u32,
    pub pause_a: u32,
    pub pause_b: u32,
    pub move_accel: u32,
    pub move_max: u32,
    pub run_a: u32,
    pub run_b: u32,
    pub color_script: u32,
    /// Kongo Jungle N64's barrel plays its catch's animation (part 2, animation 1) when it
    /// catches a fighter; Kongo Jungle's has none there.
    pub catch_anim: bool,
}

impl Barrel {
    fn f(&self, data: &Archives, game: &Game, off: u32) -> f32 {
        data.f32((self.param)(game).at(off))
    }
    /// A raw word (the swing's run and pause).
    fn word(&self, data: &Archives, game: &Game, off: u32) -> i32 {
        data.u32((self.param)(game).at(off)) as i32
    }
}

fn barrel_vars(game: &mut Game, g: GroundId) -> BarrelView<'_> {
    let v = game.grounds[g.0].u.kongo_barrel();
    BarrelView {
        turn: &mut v.xC4,
        hold: &mut v.xC6,
        swing: &mut v.xC8,
        held_for: &mut v.xCA,
        turn_timer: &mut v.xCC,
        swing_timer: &mut v.xCE,
        keep: &mut v.keep,
        heading: &mut v.xD4,
        angle: &mut v.xD8,
        step: &mut v.xDC,
        speed: &mut v.xE0,
        one: &mut v.xE4,
        rate: &mut v.xE8,
    }
}

fn yakumono(game: &Game) -> DataRef {
    param(game).0
}

/// Kongo Jungle's barrel.
pub const BARREL: Barrel = Barrel {
    vars: barrel_vars,
    param: yakumono,
    hold_a: HOLD_A,
    hold_b: HOLD_B,
    reach: BARREL_IN,
    wait_a: WAIT_A,
    wait_b: WAIT_B,
    rot_accel: ROT_ACCEL,
    rot_max: ROT_MAX,
    roll_a: ROLL_A,
    roll_b: ROLL_B,
    rates: RATES,
    pause_a: PAUSE_A,
    pause_b: PAUSE_B,
    move_accel: MOVE_ACCEL,
    move_max: MOVE_MAX,
    run_a: RUN_A,
    run_b: RUN_B,
    color_script: COLOR_SCRIPT,
    catch_anim: false,
};

/// The barrel's setup (Kongo Jungle's map 5, Kongo Jungle N64's map 1), `grKongo_801D55D8` and
/// `stageGObj1_OnInit`: its animation and materials, at rest facing up with a random wait,
/// swinging at full speed for a random run, and its place given to the stage.
#[doc(alias = "grKongo_801D55D8")]
#[doc(alias = "stageGObj1_OnInit@8020F6E4")]
pub fn init_barrel(r: &mut Running, gobj: GObjId, d: &Barrel) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, m, 0);
    material::model_to_stage_class(r.game, j);
    let max = d.f(r.data, r.game, d.move_max);
    let v = (d.vars)(r.game, g);
    *v.one = 1.0;
    *v.rate = 1.0;
    *v.heading = 0.0;
    *v.angle = 0.0;
    *v.turn = 0;
    *v.hold = 0;
    *v.swing = 2;
    *v.keep = None;
    *v.step = 0.0;
    *v.speed = 0.0;
    *v.rate = max;
    let (wb, wa) = (
        fp::fctiwz(reg(d.f(r.data, r.game, d.wait_b))),
        fp::fctiwz(reg(d.f(r.data, r.game, d.wait_a))),
    );
    let wait = grlib::rand_range(&mut r.game.objs.rng, wb, wa);
    *(d.vars)(r.game, g).turn_timer = wait as i16;
    let (rb, ra) = (
        d.word(r.data, r.game, d.run_b),
        d.word(r.data, r.game, d.run_a),
    );
    let run = grlib::rand_range(&mut r.game.objs.rng, rb, ra);
    *(d.vars)(r.game, g).swing_timer = run as i16;
    let part = nth_joint(r.game, gobj, 1);
    let pos = world_position(&mut r.game.objs, r.data, part, None);
    let angle = *(d.vars)(r.game, g).angle;
    super::ground::set_barrel(r.game, Some(gobj), pos, angle);
}

/// The barrel's turn and swing, the first part of both barrels' processes
/// (`grKongo_801D577C`, `stageGObj1_GObjProc@8020F888`). Its turn: at rest (0) it slows to a
/// stop, its angle snapping to where it was headed, and after its wait starts turning (1) one
/// way or the other, speeding up for a while; then it heads for one of the eight directions
/// drawn by weight (2), coasting until a stop at its rate would overshoot (3), then at rest
/// again once that much is left. Then the angle turns by the speed, wrapped, the model and the
/// stage's barrel place follow, and the swing's speed runs its own cycle: waiting (0),
/// speeding up to its top speed (1), holding it (2), slowing to a stop (3). Answers the
/// barrel's place.
pub fn turn_and_swing(r: &mut Running, gobj: GObjId, d: &Barrel) -> Vec3 {
    let g = ground_of(r.game, gobj);
    let part = nth_joint(r.game, gobj, 1);
    let deg = reg(DEG_TO_RAD);
    let accel = fp::fmuls(deg, reg(d.f(r.data, r.game, d.rot_accel)));
    let max = fp::fmuls(deg, reg(d.f(r.data, r.game, d.rot_max)));
    let v = (d.vars)(r.game, g);
    match *v.turn {
        2 | 3 => {
            let speed = reg(*v.speed);
            let limit = fp::fmuls(0.5, fp::fmuls(speed, fp::fdivs(speed, accel)));
            let mut delta = if speed > 0.0 {
                fp::fsubs(reg(*v.heading), reg(*v.angle))
            } else if speed < 0.0 {
                fp::fsubs(reg(*v.angle), reg(*v.heading))
            } else {
                panic!("the barrel coasts at no speed (grkongo.c:505, groldkongo.c:396: 0)");
            };
            if delta < 0.0 {
                delta = fp::frsp(fp::fadd(delta, std::f64::consts::TAU));
            }
            let abs = if speed < 0.0 { fp::fneg(speed) } else { speed };
            if delta < limit || delta < abs {
                if *v.turn == 3 {
                    *v.turn = 0;
                }
            } else if *v.turn == 2 {
                *v.turn = 3;
            }
        }
        0 => {
            let speed = reg(*v.speed);
            if speed > 0.0 {
                if speed < accel {
                    *v.speed = 0.0;
                    *v.angle = *v.heading;
                } else {
                    *v.speed = mem(fp::fsubs(speed, accel));
                }
            } else if speed < 0.0 {
                if speed > fp::fneg(accel) {
                    *v.speed = 0.0;
                    *v.angle = *v.heading;
                } else {
                    *v.speed = mem(fp::fadds(speed, accel));
                }
            }
            *v.turn_timer = v.turn_timer.wrapping_sub(1);
            if *v.turn_timer < 0 {
                *v.turn = 1;
                let step = if r.game.objs.rng.below(2) != 0 {
                    accel
                } else {
                    fp::fneg(accel)
                };
                *(d.vars)(r.game, g).step = mem(step);
                let (rb, ra) = (
                    fp::fctiwz(reg(d.f(r.data, r.game, d.roll_b))),
                    fp::fctiwz(reg(d.f(r.data, r.game, d.roll_a))),
                );
                let roll = grlib::rand_range(&mut r.game.objs.rng, rb, ra);
                *(d.vars)(r.game, g).turn_timer = roll as i16;
            }
        }
        1 => {
            *v.speed = mem(fp::fadds(reg(*v.speed), reg(*v.step)));
            *v.speed = mem(clamp_symmetric(reg(*v.speed), max));
            let left = *v.turn_timer;
            *v.turn_timer = left.wrapping_sub(1);
            if left < 0 {
                *v.turn = 2;
                let (wb, wa) = (
                    fp::fctiwz(reg(d.f(r.data, r.game, d.wait_b))),
                    fp::fctiwz(reg(d.f(r.data, r.game, d.wait_a))),
                );
                let wait = grlib::rand_range(&mut r.game.objs.rng, wb, wa);
                *(d.vars)(r.game, g).turn_timer = wait as i16;
                let to = direction(r, d);
                *(d.vars)(r.game, g).heading = to;
            }
        }
        _ => {}
    }
    let v = (d.vars)(r.game, g);
    *v.angle = mem(fp::fadds(reg(*v.angle), reg(*v.speed)));
    if reg(*v.angle) > std::f64::consts::TAU {
        *v.angle = mem(fp::frsp(fp::fsub(reg(*v.angle), std::f64::consts::TAU)));
    } else if reg(*v.angle) < fp::fneg(std::f64::consts::TAU) {
        *v.angle = mem(fp::frsp(fp::fadd(reg(*v.angle), std::f64::consts::TAU)));
    }
    let angle = *v.angle;
    set_rotation_z(r.game, part, angle);
    let at = world_position(&mut r.game.objs, r.data, part, None);
    let angle = *(d.vars)(r.game, g).angle;
    super::ground::set_barrel(r.game, Some(gobj), at, angle);

    let move_accel = reg(d.f(r.data, r.game, d.move_accel));
    let move_max = d.f(r.data, r.game, d.move_max);
    let v = (d.vars)(r.game, g);
    match *v.swing {
        0 => {
            let left = *v.swing_timer;
            *v.swing_timer = left.wrapping_sub(1);
            if left < 0 {
                *v.swing = 1;
            }
        }
        1 => {
            *v.rate = mem(fp::fadds(reg(*v.rate), move_accel));
            if reg(*v.rate) > reg(move_max) {
                *v.rate = move_max;
                let (b, a) = (
                    d.word(r.data, r.game, d.run_b),
                    d.word(r.data, r.game, d.run_a),
                );
                let run = grlib::rand_range(&mut r.game.objs.rng, b, a);
                let v = (d.vars)(r.game, g);
                *v.swing_timer = run as i16;
                *v.swing = 2;
            }
        }
        2 => {
            let left = *v.swing_timer;
            *v.swing_timer = left.wrapping_sub(1);
            if left < 0 {
                *v.swing = 3;
            }
        }
        3 => {
            *v.rate = mem(fp::fsubs(reg(*v.rate), move_accel));
            if reg(*v.rate) < 0.0 {
                *v.rate = 0.0;
                let (b, a) = (
                    d.word(r.data, r.game, d.pause_b),
                    d.word(r.data, r.game, d.pause_a),
                );
                let pause = grlib::rand_range(&mut r.game.objs.rng, b, a);
                let v = (d.vars)(r.game, g);
                *v.swing_timer = pause as i16;
                *v.swing = 0;
            }
        }
        _ => {}
    }
    let rate = *(d.vars)(r.game, g).rate;
    anime::set_rate(r.game, gobj, 0, 7, rate, true);
    at
}

/// One of the barrel's eight directions drawn by the parameters' weights, `grKongo_801D8314`
/// and `grOldKongo_80210650`: from down-left (3π/4) round through up (0) to down (-π).
#[doc(alias = "grKongo_801D8314")]
#[doc(alias = "grOldKongo_80210650")]
pub fn direction(r: &mut Running, d: &Barrel) -> f32 {
    const ANGLES: [f32; 8] = [
        2.356_194_5,
        1.570_796_4,
        std::f32::consts::FRAC_PI_4,
        0.0,
        -std::f32::consts::FRAC_PI_4,
        -1.570_796_4,
        -2.356_194_5,
        -std::f32::consts::PI,
    ];
    let p = (d.param)(r.game);
    let rates: [i32; 8] =
        std::array::from_fn(|i| i32::from(r.data.u16(p.at(d.rates + 2 * i as u32)) as i16));
    let total = rates.iter().fold(0i32, |a, &b| a.wrapping_add(b));
    let mut left = if total != 0 {
        r.game.objs.rng.below(total)
    } else {
        0
    };
    for (rate, angle) in rates.into_iter().zip(ANGLES) {
        left = left.wrapping_sub(rate);
        if left < 0 {
            return angle;
        }
    }
    panic!("no direction drawn (grkongo.c:1753, groldkongo.c:786: 0)");
}

/// The barrel's catch test for a fighter, `fn_801D8134` and `grOldKongo_80210454`: while it
/// holds nothing, a fighter within its reach is caught, held for a random while, with the
/// barrel's sound (on Kongo Jungle N64 its catch's animation, [`Barrel::catch_anim`]) and
/// flash, an effect where the fighter is, and its controller's rumble.
#[doc(alias = "fn_801D8134")]
#[doc(alias = "grOldKongo_80210454")]
pub fn catch_fighter(r: &mut Running, gobj: GObjId, fighter: GObjId, d: &Barrel) -> bool {
    let g = ground_of(r.game, gobj);
    if *(d.vars)(r.game, g).hold != 0 {
        return false;
    }
    let (at, _) = super::ground::barrel(r.game);
    let pos = r.game.ft_pos(fighter);
    let dd = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
    let (dx, dy, dz) = (dd(at.x, pos.x), dd(at.y, pos.y), dd(at.z, pos.z));
    let dist = fp::fadds(
        fp::fmuls(dz, dz),
        fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
    );
    let reach = reg(d.f(r.data, r.game, d.reach));
    if !(dist < fp::fmuls(reach, reach)) {
        return false;
    }
    let f = reg(r.game.objs.rng.draw_f32());
    let (a, b) = (
        reg(d.f(r.data, r.game, d.hold_a)),
        reg(d.f(r.data, r.game, d.hold_b)),
    );
    let hold = fp::fctiwz(fp::fmadds(fp::fsubs(b, a), f, a));
    let v = (d.vars)(r.game, g);
    *v.held_for = hold as i16;
    *v.keep = Some(fighter);
    *v.hold = 1;
    r.object_sound(Some(g), 0, 0x129);
    if d.catch_anim {
        anime::start(r.game, r.data, gobj, 2, 7, 1, 0.0, 1.0, true);
    }
    let script = r.data.ptr((d.param)(r.game).at(d.color_script));
    material::start_script(r.game, r.data, gobj, script, 0);
    super::super::ef::spawn(r, 0x405, Some(gobj), &[super::super::ef::Arg::Vec(pos)]);
    crate::game::ft::ftlib::start_rumble(r, fighter, 13, 30);
    true
}

/// The barrel lets what it holds go early when the fighter asks, `grKongo_801D8270` and
/// `grOldKongo_802105AC`.
#[doc(alias = "grKongo_801D8270")]
#[doc(alias = "grOldKongo_802105AC")]
pub fn let_go(game: &mut Game, gobj: GObjId, d: &Barrel) {
    let g = ground_of(game, gobj);
    let v = (d.vars)(game, g);
    if *v.hold == 1 {
        *v.hold = 2;
    }
}

/// The barrel drops its fighter, `grKongo_801D828C` and `grOldKongo_802105C8`: a held fighter
/// (its GObj's `p_link` is 8) is let go and the barrel's flash ends.
#[doc(alias = "grKongo_801D828C")]
#[doc(alias = "grOldKongo_802105C8")]
pub fn drop_fighter(game: &mut Game, gobj: GObjId, d: &Barrel) {
    let g = ground_of(game, gobj);
    let v = (d.vars)(game, g);
    if *v.hold != 1 {
        return;
    }
    let keep = v.keep.expect("the barrel's held fighter (gp->u.taru.keep)");
    if game.world.gobj(keep).p_link == 8 {
        let v = (d.vars)(game, g);
        *v.hold = 0;
        *v.keep = None;
        material::reset_overlay(game, gobj);
    }
}

/// The barrel's catch test for a fighter, as the fighter code calls it ([`catch_fighter`]).
pub fn barrel_catch(r: &mut Running, gobj: GObjId, fighter: GObjId) -> bool {
    catch_fighter(r, gobj, fighter, &BARREL)
}

/// The barrel lets its fighter go early, as the fighter code asks it ([`let_go`]).
pub fn barrel_release(game: &mut Game, gobj: GObjId) {
    let_go(game, gobj, &BARREL)
}

/// The barrel drops its fighter, as the fighter code asks it ([`drop_fighter`]).
pub fn barrel_drop(game: &mut Game, gobj: GObjId) {
    drop_fighter(game, gobj, &BARREL)
}

/// The barrel's process (map 5), `grKongo_801D577C`: its turn and swing ([`turn_and_swing`],
/// as Kongo Jungle N64's barrel's), then what it holds: with nothing, a Klap Trap within its reach is caught (held for a random
/// while, with the barrel's sound and flash); held, the countdown runs (or with nothing held
/// it clears), then the shot: the held fighter or Klap Trap is launched from the barrel's
/// place at the parameters' hit, perpendicular to its heading, with the barrel's sound; then
/// it waits for no Klap Trap to be near before catching again.
#[doc(alias = "grKongo_801D577C")]
pub fn proc_barrel(r: &mut Running, gobj: GObjId) {
    let at = turn_and_swing(r, gobj, &BARREL);
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let mut state = r.game.grounds[g.0].u.kongo_barrel().xC6;
    match state {
        0 => {
            if let Some(klap) = klap_near(r) {
                super::super::it::kinds::klap_trap::enter_barrel(r, klap);
                let f = reg(r.game.objs.rng.draw_f32());
                let (a, b) = (reg(p.f(r.data, HOLD_A)), reg(p.f(r.data, HOLD_B)));
                let hold = fp::fctiwz(fp::fmadds(fp::fsubs(b, a), f, a));
                let v = r.game.grounds[g.0].u.kongo_barrel();
                v.xCA = hold as i16;
                v.keep = Some(klap);
                v.xC6 = 1;
                r.object_sound(Some(g), 0, 0x129);
                let script = r.data.ptr(p.0.at(COLOR_SCRIPT));
                material::start_script(r.game, r.data, gobj, script, 0);
            }
            return;
        }
        1 => {
            // With nothing held the state clears and falls into the shot all the same (no
            // `break`); nothing leaves a held state without what it holds.
            let v = r.game.grounds[g.0].u.kongo_barrel();
            if v.keep.is_none() {
                v.xC6 = 0;
            } else {
                let left = v.xCA;
                v.xCA = left.wrapping_sub(1);
                if left >= 0 {
                    return;
                }
                v.xC6 = 2;
            }
            state = 2;
        }
        _ => {}
    }
    match state {
        2 => {
            let words: [u32; 6] =
                std::array::from_fn(|i| r.data.u32(p.0.at(SHOT_HIT + 4 * i as u32)));
            let mut a = fp::frsp(fp::fadd(
                std::f64::consts::FRAC_PI_2,
                reg(r.game.grounds[g.0].u.kongo_barrel().xD8),
            ));
            if a < 0.0 {
                a = fp::frsp(fp::fadd(a, std::f64::consts::TAU));
            } else if a > std::f64::consts::TAU {
                a = fp::frsp(fp::fsub(a, std::f64::consts::TAU));
            }
            let a = mem(fp::fmuls(reg(57.295_78), a));
            let keep = r.game.grounds[g.0].u.kongo_barrel().keep;
            let keep = keep.expect("grkongo.c: the barrel holds nothing");
            match r.game.world.gobj(keep).p_link {
                8 => r.shoot_from_stage_barrel(keep, at, &shot_hit(words), a),
                9 => super::super::it::kinds::klap_trap::shot(r, keep, at, shot_hit(words), a),
                _ => {}
            }
            r.game.grounds[g.0].u.kongo_barrel().xC6 = 3;
            r.object_sound(Some(g), 0, 0x12A);
            material::reset_overlay(r.game, gobj);
        }
        3 if klap_near(r).is_none() => {
            r.game.grounds[g.0].u.kongo_barrel().xC6 = 0;
        }
        _ => {}
    }
}

/// The shot's hit (`grKg_803B7FB0` with the parameters' state, damage, angle, knockback
/// growth, set knockback, base knockback and element), as `lbColl_80008D30` copies one.
pub(super) fn shot_hit(w: [u32; 6]) -> super::super::structs::HitCapsule {
    super::super::structs::HitCapsule {
        state: 1,
        damage: mem(fp::frsp(f64::from(w[0]))),
        kb_angle: w[1] as i32,
        x24: w[2],
        x28: w[3],
        x2C: w[4],
        element: w[5],
        sfx_severity: 0,
        sfx_kind: 0,
        unk_count: w[0],
        ..Default::default()
    }
}

/// The first Klap Trap within the barrel's reach, `grKongo_801D8078`.
#[doc(alias = "grKongo_801D8078")]
pub fn klap_near(r: &Running) -> Option<GObjId> {
    let (at, _) = super::ground::barrel(r.game);
    let reach = reg(param(r.game).f(r.data, BARREL_IN));
    let mut cur = r.game.world.first_in(super::super::it::P_LINK_ITEM);
    while let Some(c) = cur {
        if super::super::it::item(r.game, c).kind == ItKind::KlapTrap {
            let pos = super::super::it::item(r.game, c).pos;
            let d = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
            let (dx, dy, dz) = (d(at.x, pos.x), d(at.y, pos.y), d(at.z, pos.z));
            let dist = fp::fadds(
                fp::fmuls(dz, dz),
                fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
            );
            if dist < fp::fmuls(reach, reach) {
                return Some(c);
            }
        }
        cur = r.game.world.gobj(c).next;
    }
    None
}

/// A floating platform's setup (maps 1 and 2), `grKongo_801D6074`: its rising animation, idle,
/// its heights' extremes reset, and a random place across (map 1's wider).
#[doc(alias = "grKongo_801D6074")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let v = r.game.grounds[g.0].u.kongo_platform();
    v.xC4 = 0;
    v.xC6 = 0;
    v.xC8 = 0;
    v.xCA = 0;
    v.xD0 = -99999.0;
    v.xD4 = f32::MAX;
    let f = reg(r.game.objs.rng.draw_f32());
    let x = if m == 1 {
        fp::frsp(fp::fmadd(45.0, f, -15.0))
    } else {
        fp::frsp(fp::fmul(9.0, f))
    };
    r.game.grounds[g.0].u.kongo_platform().xCC = mem(x);
    let j = j.expect("jobj.h: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = mem(x));
}

/// A floating platform's process (maps 1 and 2), `grKongo_801D6198`. Rising (0), once its
/// animation ends its collision comes on (joint 0 for map 1, 1 for map 2) with the landing
/// callback (1). Afloat, it counts ticks and notes landings (`xC6` set by the callback, `xC8`
/// the last tick's); past the parameters' landings or ticks, or with a log just behind it, it
/// sinks (its second animation, its collision off) (2), and goes when that ends.
#[doc(alias = "grKongo_801D6198")]
pub fn proc_platform(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let joint = if m == 1 { 0 } else { 1 };
    let state = r.game.grounds[g.0].u.kongo_platform().xC4;
    match state {
        0 => {
            if anime::finished(r.game, gobj, 0, 1) {
                let j = jobj_of(r.game, gobj);
                setup::init_map_coll(r.game, r.data, j, m);
                update_map_coll(r.game, r.data, gobj);
                let geo = r.game.geometry_mut();
                geo.enable_joint(joint);
                geo.set_ledge_callback(
                    joint,
                    Some(super::CollUser::Ground(g)),
                    Some(CodeRef(PLATFORM_LANDED)),
                );
                r.game.grounds[g.0].u.kongo_platform().xC4 = 1;
            }
        }
        1 => {
            let v = r.game.grounds[g.0].u.kongo_platform();
            v.xCA = v.xCA.wrapping_add(1);
            if v.xC8 == v.xC6 {
                v.xC6 = 0;
                v.xC8 = 0;
            } else {
                v.xC8 = v.xC6;
            }
            let (landings, ticks) = (v.xC6, v.xCA);
            let p = param(r.game);
            let over = |n: i16, off: u32| fp::frsp(f64::from(n)) > reg(p.f(r.data, off));
            if over(landings, PLATFORM_LANDINGS)
                || over(ticks, PLATFORM_TICKS)
                || log_behind(r, gobj)
            {
                anime::start_map(r.game, r.data, gobj, m, 1);
                let geo = r.game.geometry_mut();
                geo.disable_joint(joint);
                geo.clear_ledge_callback(joint);
                r.game.grounds[g.0].u.kongo_platform().xC4 = 2;
            }
        }
        2 if anime::finished(r.game, gobj, 0, 1) => {
            setup::destroy(r, Some(gobj));
        }
        _ => {}
    }
}

/// A floating platform's landing callback, `fn_801D7E60`: a landing (ground kind 2) notes one
/// more than the last tick's count.
#[doc(alias = "fn_801D7E60")]
pub fn platform_landed(gp: &mut super::super::structs::Ground, ground_kind: i32) {
    if ground_kind == 2 {
        let v = gp.u.kongo_platform();
        v.xC6 = v.xC8.wrapping_add(1);
    }
}

/// Where a drifting object is, `grKongo_801D7E78`: a platform's or a log's part 2, the river
/// path's model; none for others or without one.
#[doc(alias = "grKongo_801D7E78")]
pub fn drift_pos(game: &mut Game, data: &Archives, gobj: GObjId) -> Option<Vec3> {
    let Some(UserData::Ground(g)) = game.world.gobjs[gobj.0].user_data else {
        return None;
    };
    match game.grounds[g.0].map_id {
        1 | 2 | 7 | 8 | 9 => {
            let j = nth_joint(game, gobj, 2)?;
            Some(world_position(&mut game.objs, data, Some(j), None))
        }
        11 => {
            let j = jobj_of(game, gobj)?;
            Some(game.objs.jobjs[j].translate)
        }
        _ => None,
    }
}

/// Whether another drifting object is just behind this one (100 to 150 back, within 80
/// across), `grKongo_801D7F78`.
#[doc(alias = "grKongo_801D7F78")]
pub fn log_behind(r: &mut Running, gobj: GObjId) -> bool {
    let Some(me) = drift_pos(r.game, r.data, gobj) else {
        return false;
    };
    let mut cur = r.game.world.first_in(5);
    while let Some(c) = cur {
        cur = r.game.world.gobj(c).next;
        if c == gobj {
            continue;
        }
        let Some(other) = drift_pos(r.game, r.data, c) else {
            continue;
        };
        let dz = fp::fsubs(reg(other.z), reg(me.z));
        if dz < -100.0 && dz > -150.0 {
            let dx = fp::fsubs(reg(other.x), reg(me.x));
            if fp::fmuls(dx, dx) < 6400.0 {
                return true;
            }
        }
    }
    false
}

/// A log's setup (maps 7, 8 and 9), `grKongo_801D637C`: its animation, not looping, and a
/// random place across (by map).
#[doc(alias = "grKongo_801D637C")]
pub fn init_log(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::clear_loop(r.game, gobj, 0, 7, true);
    let f = reg(r.game.objs.rng.draw_f32());
    let x = match m {
        7 => fp::frsp(fp::fmadd(45.0, f, -15.0)),
        8 => fp::frsp(fp::fmadd(20.0, f, -10.0)),
        _ => fp::frsp(fp::fmul(9.0, f)),
    };
    r.game.grounds[g.0].u.kongo_log().xCC = mem(x);
    let j = j.expect("jobj.h: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.x = mem(x));
}

/// A log's process (maps 7, 8 and 9), `grKongo_801D64BC`: it goes when its animation ends or
/// comes round.
#[doc(alias = "grKongo_801D64BC")]
pub fn proc_log(r: &mut Running, gobj: GObjId) {
    if anime::finished(r.game, gobj, 0, 1) || anime::rewound(r.game, gobj, 0, 1) {
        setup::destroy(r, Some(gobj));
    }
}

/// The river path's setup (map 11), `grKongo_801D651C`: one of three splines (in maps 7, 8 or
/// 9's archives) drawn at random, at its start, its heights' and depths' extremes reset.
#[doc(alias = "grKongo_801D651C")]
pub fn init_path(r: &mut Running, gobj: GObjId) {
    const SPLINES: [(i32, i32); 3] = [(7, 4), (8, 5), (9, 6)];
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let (file, i) = SPLINES[r.game.objs.rng.below(3) as usize];
    let spline = super::ground::spline(r.game, r.data, file, i);
    let v = r.game.grounds[g.0].u.kongo_path();
    v.xC4 = spline;
    v.xC8 = 0.0;
    v.xD0 = 0.0;
    v.xCC = 0.0;
    v.xD8 = -99999.0;
    v.xDC = f32::MAX;
    let spline = spline.expect("grkongo.c: the river's spline");
    let at = crate::hsd::spline::arc_length_point(r.data, spline, 0.0)
        .expect("splArcLengthPoint: the start of a spline");
    let j = j.expect("jobj.h: jobj");
    jobj::set_translate(&mut r.game.objs.jobjs, j, at);
}

/// The river path's process (map 11), `grKongo_801D6668`: it moves along its spline at the
/// parameters' speed (the arc length's step scaled by the distance a thousandth covers), its
/// model at the spline's point turned along it; as it falls more than 5 below its highest
/// point the waterfall's sound plays, louder the nearer it has come.
#[doc(alias = "grKongo_801D6668")]
pub fn proc_path(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    let spline = r.game.grounds[g.0].u.kongo_path().xC4;
    let spline = spline.expect("grkongo.c: the river's spline");
    let s = r.game.grounds[g.0].u.kongo_path().xCC;
    let step = reg(0.001f32);
    if fp::fadds(reg(s), step) <= 1.0 {
        let a = crate::hsd::spline::arc_length_point(r.data, spline, s);
        let b = crate::hsd::spline::arc_length_point(r.data, spline, mem(fp::fadds(reg(s), step)));
        let (a, b) = (
            a.expect("splArcLengthPoint: a point on the spline"),
            b.expect("splArcLengthPoint: a point on the spline"),
        );
        let dx = fp::fsubs(reg(a.x), reg(b.x));
        let dy = fp::fsubs(reg(a.y), reg(b.y));
        let dz = fp::fsubs(reg(a.z), reg(b.z));
        let mut dist = fp::fadds(
            fp::fmuls(dz, dz),
            fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
        );
        if dist > 0.0 {
            dist = crate::math::sqrt_reg(dist);
        }
        if dist > 0.0 {
            let speed = reg(param(r.game).f(r.data, PATH_SPEED));
            let scaled = fp::fmuls(step, fp::fdivs(speed, dist));
            let v = r.game.grounds[g.0].u.kongo_path();
            v.xCC = mem(fp::fadds(reg(v.xCC), scaled));
            if reg(v.xCC) > 1.0 {
                v.xCC = 1.0;
            }
        }
    }
    let s = r.game.grounds[g.0].u.kongo_path().xCC;
    let (at, rot) = super::ground::spline_pose(r.data, spline, s);
    let at = at.expect("splGetSplinePoint: a point on the spline");
    let before = r.game.objs.jobjs[j].translate;
    jobj::set_translate(&mut r.game.objs.jobjs, j, at);
    jobj::set_rotation(
        &mut r.game.objs.jobjs,
        j,
        crate::quat::Quat {
            x: rot.x,
            y: rot.y,
            z: rot.z,
            w: 1.0,
        },
    );
    let v = r.game.grounds[g.0].u.kongo_path();
    if reg(v.xD8) < reg(at.y) {
        v.xD8 = at.y;
    }
    if reg(v.xDC) > reg(at.z) {
        v.xDC = at.z;
    }
    let top = reg(v.xD8);
    let deepest = reg(v.xDC);
    if fp::fsubs(top, reg(before.y)) < 5.0 && fp::fsubs(top, reg(at.y)) > 5.0 {
        r.object_sound(Some(g), 0, 0x5_A550);
        let vol = mem(fp::fsubs(1.0, fp::fdivs(reg(at.z), deepest)));
        r.object_sound_volume(Some(g), 0, vol);
    }
}

/// Destroys a river path (its Klap Trap gone), `grKongo_801D8058`.
#[doc(alias = "grKongo_801D8058")]
pub fn destroy_path(r: &mut Running, path: GObjId) {
    setup::destroy(r, Some(path));
}

/// The stage's shadow check, `grKongo_801D844C`: a fighter casts on a joint below its feet.
#[doc(alias = "grKongo_801D844C")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
