//! Dream Land (the N64 one), `melee/gr/groldpupupu.c`: Whispy Woods (map 7), who turns toward
//! the side most fighters are on and blows them away, his face's animations (map 1, which he
//! keeps in step), the clouds and stars that pass behind the stage now and then (map 8 spawns
//! map 2's objects), and the stage itself (map 5).
//!
//! Whispy's wind is a stage device the fighter code polls (`ftColl_GetWindOffsetVec`,
//! [`wind_push`]), and also a wind source that sways jointed parts (`lb_80011A50`).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{OLDPUPUPU_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, scale};
use super::{anime, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// Whispy's map id.
pub const MAP_WHISPY: i32 = 7;
/// Whispy's face, which follows him.
pub const MAP_FACE: i32 = 1;

/// `ftCo_800C06E8`'s device type for Whispy's wind.
const WIND_DEVICE: u32 = 10;
/// `fn_802112F4`, Whispy's wind as the fighter code polls it.
pub const WIND_PUSH: u32 = 0x8021_12F4;

/// The fields of `grOldpupupu_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy, melee_derive::StateEq)]
struct Param(DataRef);

impl Param {
    fn h(self, r: &Running, off: u32) -> i16 {
        r.data.u16(self.0.at(off)) as i16
    }
    fn i(self, r: &Running, off: u32) -> i32 {
        r.data.u32(self.0.at(off)) as i32
    }
    fn f(self, r: &Running, off: u32) -> f32 {
        r.data.f32(self.0.at(off))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .oldpupupu_yakumono
            .expect("groldpupupu.c: yakumono_param"),
    )
}

/// The clouds' and stars' flights (`grOp_803E67B0`): from the left (-1) or right (1), whether
/// they can come three at once, and the joint shown.
const FLIGHTS: [(i8, i8, i16); 10] = [
    (-1, 1, 1),
    (1, 1, 3),
    (1, 1, 5),
    (-1, 1, 7),
    (-1, 0, 9),
    (1, 0, 11),
    (-1, 0, 13),
    (1, 0, 15),
    (-1, 0, 17),
    (1, 0, 19),
];

/// The cycle of Whispy's states (`grOp_803E67D8`): idle, turning, blowing.
const CYCLE: [i32; 3] = [0, 1, 2];

/// The wind's directions by side (`grOp_803E67E4`).
const WIND_DIR: [Vec3; 2] = [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];

/// The face's animations by state and side (`grOp_804D48A0`, `grOp_804D48B0`, `grOp_804D48A8`).
const FACE_IDLE: [i32; 2] = [4, 0];
const FACE_TURN: [i32; 2] = [2, 3];
const FACE_BLOW: [i32; 2] = [5, 1];
/// Whispy's animations (`grOp_804D48C8`, `grOp_804D48D0`, `grOp_804D48D8`) and the leaves'
/// (`grOp_804D48E0`, map 6) by side.
const WHISPY_IDLE: [i32; 2] = [4, 0];
const WHISPY_BLOW: [i32; 2] = [5, 1];
const WHISPY_TURN: [i32; 2] = [2, 3];
const LEAVES_BLOW: [i32; 2] = [1, 0];

/// A stage object's process or callback, by address: Dream Land's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_09D0 | 0x8021_09D4 | 0x8021_0B00 | 0x8021_0B50 | 0x8021_0B54 | 0x8021_0BE0
        | 0x8021_0C78 | 0x8021_110C | 0x8021_1194 | 0x8021_1198 | 0x8021_1C0C | 0x8021_1C10
        | 0x8021_1C9C => empty(r, gobj),
        0x8021_099C => init_main(r, gobj),
        0x8021_09D8 => init_face(r, gobj),
        0x8021_0A24 => proc_face(r, gobj),
        0x8021_0B04 => init_back(r, gobj),
        0x8021_0B58 => init_stage(r, gobj),
        0x8021_0BC0 => proc_stage(r, gobj),
        0x8021_0BE4 => init_flyer(r, gobj),
        0x8021_0C34 => proc_flyer(r, gobj),
        0x8021_0C7C => init_flights(r, gobj),
        0x8021_0D10 => proc_flights(r, gobj),
        0x8021_1110 => init_sky(r, gobj),
        0x8021_119C => init_whispy(r, gobj),
        0x8021_13E0 => proc_whispy(r, gobj),
        0x8021_1C1C => proc_leaves(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_07E0 => on_init(r),
        0x8021_0884 => on_load(r),
        0x8021_0888 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grOldPupupu_802109D0")]
#[doc(alias = "grOldPupupu_80210B50")]
#[doc(alias = "grOldPupupu_80211194")]
#[doc(alias = "grOldPupupu_80211C10")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Where Dream Land's setup and the spawn's call put their scale joints' descriptors on the
/// stack (its setup's frame is deeper than other stages', `setup::SETUP_FRAME`).
const SETUP_FRAME: u32 = 0x804E_E9F0;
const SPAWN_FRAME: u32 = 0x804E_E9A0;

/// Creates one of the stage's objects and sets it up, `grOldPupupu_802108B4`.
#[doc(alias = "grOldPupupu_802108B4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grOp_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &OLDPUPUPU_CALLBACKS[map_id as usize]
}

/// Dream Land's setup, `grOldPupupu_802107E0`: its parameters, maps 0, 3, 7, 5, 4, 6, 1 and
/// 8, camera limits and blast zones.
#[doc(alias = "grOldPupupu_802107E0")]
pub fn on_init(r: &mut Running) {
    r.game.gr.oldpupupu_yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 3, 7, 5, 4, 6, 1, 8] {
        create(r, map_id, SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    // `grOp_804D6A9C`, which Whispy's and his face's processes test, is set to null here and
    // nothing else writes it.
}

/// `grOldPupupu_802107DC` does nothing. Ports `grOldPupupu_802107DC`; aliased once a check
/// reaches it (only a demo runs it).
pub fn on_demo_init(_r: &mut Running) {}

/// `grOldPupupu_80210884` does nothing; `grOldPupupu_802108AC` (`callback4`) answers false
/// and nothing calls it.
#[doc(alias = "grOldPupupu_80210884")]
pub fn on_load(_r: &mut Running) {}

/// Dream Land's start, `grOldPupupu_80210888`: the enemy spawner, without enemies.
#[doc(alias = "grOldPupupu_80210888")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@8021099C")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Whispy's face's setup (map 1), `grOldPupupu_802109D8`: idle, its animation to apply.
#[doc(alias = "grOldPupupu_802109D8")]
pub fn init_face(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.oldpupupu();
    v.xC4 = 0;
    v.xC8 = 0;
    v.xCC = 0;
    v.xD0 = 1;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Whispy's face's process, `grOldPupupu_80210A24`: when Whispy has handed it a new state
/// (`xD0`), the state's animation for the side he faces (`xD4`).
#[doc(alias = "grOldPupupu_80210A24")]
pub fn proc_face(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.oldpupupu();
    if v.xD0 != 1 {
        return;
    }
    v.xD0 = 0;
    let (state, row, side) = (v.xC8, v.xCC, v.xD4);
    let table = match state {
        0 => FACE_IDLE,
        1 => FACE_TURN,
        2 => FACE_BLOW,
        _ => return,
    };
    let anim = pick(table, row, side);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
}

/// An entry of a `[1][2]` animation table by row and side: only row 0 exists.
fn pick(table: [i32; 2], row: i32, side: i32) -> i32 {
    assert!(
        row == 0,
        "groldpupupu.c: an animation table's row {row} past its one"
    );
    table[usize::try_from(side).expect("groldpupupu.c: a negative side")]
}

/// Map 4's setup, `grOldPupupu_80210B04`: its animation, and it receives shadows.
#[doc(alias = "grOldPupupu_80210B04")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The stage's setup (map 5), `grOldPupupu_80210B58`: its collision and animation.
#[doc(alias = "grOldPupupu_80210B58")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The stage's process, `grOldPupupu_80210BC0`: the wind sources.
#[doc(alias = "grOldPupupu_80210BC0")]
pub fn proc_stage(r: &mut Running, _gobj: GObjId) {
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// A cloud or star's setup (map 2), `grOldPupupu_80210BE4`: hidden until its flight shows it.
#[doc(alias = "grOldPupupu_80210BE4")]
pub fn init_flyer(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// A cloud or star's process, `grOldPupupu_80210C34`: it goes once its flight's animation
/// ends.
#[doc(alias = "grOldPupupu_80210C34")]
pub fn proc_flyer(r: &mut Running, gobj: GObjId) {
    if anime::finished(r.game, gobj, 0, 7) {
        setup::destroy(r, Some(gobj));
    }
}

/// A random wait between the parameters' two shorts at 0 and 2, drawn only if they differ.
fn flight_wait(r: &mut Running) -> i16 {
    let p = param(r.game);
    let (lo, hi) = (i32::from(p.h(r, 0)), i32::from(p.h(r, 2)));
    let randi = |r: &mut Running, n: i32| if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    let v = if hi > lo {
        lo.wrapping_add(randi(r, hi.wrapping_sub(lo)))
    } else if hi < lo {
        hi.wrapping_add(randi(r, lo.wrapping_sub(hi)))
    } else {
        hi
    };
    v as i16
}

/// The flights' setup (map 8), `grOldPupupu_80210C7C`: the first wait.
#[doc(alias = "grOldPupupu_80210C7C")]
pub fn init_flights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let t = flight_wait(r);
    r.game.grounds[g.0].u.oldpupupu2().xC4 = t;
}

/// The flights' process, `grOldPupupu_80210D10`: when the wait runs out, one of the ten
/// flights (three at once one time in five if it can) behind the stage from just past the
/// camera's edge, at a random height, and a new wait.
#[doc(alias = "grOldPupupu_80210D10")]
pub fn proc_flights(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.oldpupupu2();
    let timer = v.xC4;
    v.xC4 = timer.wrapping_sub(1);
    if timer >= 0 {
        return;
    }
    let direction = reg(scale(r.game));
    let index = r.game.objs.rng.below(10) as usize;
    let (from, can_triple, joint) = FLIGHTS[index];
    let count = if can_triple != 0 && r.game.objs.rng.below(5) == 0 {
        3
    } else {
        1
    };
    let ([mut left, _, mut right], _) = super::super::camera::ground_span(r.game, r.data);
    let n = fp::frsp(f64::from(count));
    let (step, mut x) = if from == 1 {
        if reg(right) < 200.0 {
            right = 200.0;
        }
        let step = fp::fneg(10.0);
        (step, fp::fnmsubs(n, step, fp::fadds(50.0, reg(right))))
    } else {
        if reg(left) > fp::fneg(200.0) {
            left = -200.0;
        }
        let step = 10.0;
        (step, fp::fnmsubs(n, step, fp::fsubs(reg(left), 50.0)))
    };
    let rand = reg(r.game.objs.rng.draw_f32());
    let spread = fp::frsp(f64::from(param(r.game).h(r, 4)));
    let y = fp::fmuls(
        direction,
        fp::fmadds(spread, fp::fmsubs(2.0, rand, 1.0), 20.0),
    );
    let z = fp::fmuls(fp::fneg(150.0), direction);
    for i in 0..count {
        if let Some(spawn) = create(r, 2, SPAWN_FRAME) {
            let j = jobj_of(r.game, spawn).expect("groldpupupu.c: jobj");
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
                o.translate.x = mem(fp::fmuls(direction, x))
            });
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = mem(y));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.z = mem(z));
            let frame = mem(fp::fmuls(40.0, fp::frsp(f64::from(i))));
            anime::start(r.game, r.data, spawn, 0, 7, 0, frame, 1.0, true);
            let shown = nth_joint(r.game, spawn, i32::from(joint));
            jobj::clear_flags_all(&mut r.game.objs.jobjs, shown, jobj::HIDDEN);
        }
        x = fp::fadds(x, step);
    }
    let t = flight_wait(r);
    r.game.grounds[g.0].u.oldpupupu2().xC4 = t;
}

/// The sky's setup (map 3), `grOldPupupu_80211110`: its animation, no shadows, and its
/// materials marked with render-mode bits 27 and 29.
#[doc(alias = "grOldPupupu_80211110")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b2 = false;
    material::set_tree_flags(r.game, j, 0x2800_0000);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// `randrange`: between the two, whichever is lower, drawn only if they differ.
fn randrange(r: &mut Running, min: i32, max: i32) -> i32 {
    let randi = |r: &mut Running, n: i32| if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if min > max {
        let d = min.wrapping_sub(max);
        max.wrapping_add(randi(r, d))
    } else if min < max {
        let d = max.wrapping_sub(min);
        min.wrapping_add(randi(r, d))
    } else {
        min
    }
}

/// Whispy's setup (map 7), `grOldPupupu_8021119C`: his wind registered with the fighter code
/// (`ftCo_800C06E8`), idle for a random while facing right, his animation.
#[doc(alias = "grOldPupupu_8021119C")]
pub fn init_whispy(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    register_wind(r.game, gobj, WIND_DEVICE, CodeRef(WIND_PUSH));
    let p = param(r.game);
    let (min, max) = (p.i(r, 0xC), p.i(r, 8));
    let v = r.game.grounds[g.0].u.oldpupupu();
    v.xC4 = 0;
    v.xC8 = 0;
    v.xCC = 0;
    let wait = randrange(r, min, max);
    let v = r.game.grounds[g.0].u.oldpupupu();
    v.xD0 = wait;
    v.xD4 = 1;
    v.xE0 = 0;
    v.xD8 = 1;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// Registers a stage object's wind with the fighter code, `ftCo_800C06E8`: its one slot.
#[doc(alias = "ftCo_800C06E8")]
pub fn register_wind(game: &mut Game, gobj: GObjId, kind: u32, cb: CodeRef) {
    let d = &mut game.gr.devices;
    assert!(d.device.is_none(), "fighter chk device wind func num over!");
    d.device = Some(gobj);
    d.device_type = kind;
    d.device_cb = Some(cb);
    d.x6578 = d.x6578.wrapping_add(1);
}

/// Whether `pos` lies strictly inside the box the two x and two y bounds make, in either
/// order, `grOldPupupu_8021128C`.
#[doc(alias = "grOldPupupu_8021128C")]
pub fn in_box(pos: Vec3, max_x: f32, min_x: f32, max_y: f32, min_y: f32) -> bool {
    let (mut lo_x, mut lo_y) = (reg(min_x), reg(min_y));
    let hi_x = if reg(max_x) > lo_x {
        reg(max_x)
    } else {
        let h = lo_x;
        lo_x = reg(max_x);
        h
    };
    let hi_y = if reg(max_y) > lo_y {
        reg(max_y)
    } else {
        let h = lo_y;
        lo_y = reg(max_y);
        h
    };
    let (x, y) = (reg(pos.x), reg(pos.y));
    lo_x < x && x < hi_x && lo_y < y && y < hi_y
}

/// Whispy's wind on a fighter, `fn_802112F4`: while he blows (`xDC` 1 to the left, 2 to the
/// right), a fighter in the box on his side is pushed at the parameters' speed.
#[doc(alias = "fn_802112F4")]
pub fn wind_push(r: &mut Running, gobj: GObjId, fighter: GObjId) -> Option<Vec3> {
    let pos = r.game.fighter(fighter).cur_pos;
    let p = param(r.game);
    let g = ground_of(r.game, gobj);
    let blowing = r.game.grounds[g.0].u.oldpupupu().xDC;
    let (y1, y0) = (p.f(r, 0x24), p.f(r, 0x28));
    let x = match blowing {
        1 if in_box(pos, p.f(r, 0x1C), p.f(r, 0x20), y1, y0) => mem(fp::fneg(reg(p.f(r, 0x10)))),
        2 if in_box(pos, p.f(r, 0x18), p.f(r, 0x14), y1, y0) => p.f(r, 0x10),
        _ => return None,
    };
    Some(Vec3::new(x, 0.0, 0.0))
}

/// Hands Whispy's state to his face (the inline `grOldPupupu_SyncSecondary`).
fn sync_face(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.oldpupupu();
    let (c4, c8, d8) = (v.xC4, v.xC8, v.xD8);
    let face = map_gobj(game, MAP_FACE).expect("Ground_GetMapGObj(1)");
    let fg = ground_of(game, face);
    let o = game.grounds[fg.0].u.oldpupupu();
    o.xC4 = c4;
    o.xC8 = c8;
    o.xCC = 0;
    o.xD4 = d8;
    o.xD0 = 1;
}

/// Whispy's next state in the cycle (the inline `grOldPupupu_Advance`).
fn advance(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.oldpupupu();
    v.xC4 = v.xC4.wrapping_add(1);
    if v.xC4 >= 3 {
        v.xC4 = 0;
    }
    v.xC8 = CYCLE[v.xC4 as usize];
    v.xD4 = 1;
}

/// A random count between the parameters' floats at 0x30 and 0x2C, as integers.
fn blink_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let (a, b) = (fp::fctiwz(reg(p.f(r, 0x30))), fp::fctiwz(reg(p.f(r, 0x2C))));
    randrange(r, a, b)
}

/// Whispy's process, `grOldPupupu_802113E0`: idle for a while (blinking), then he turns to the
/// side most fighters are on (or blows at once if he faces it), then blows: from frame 45 his
/// sound, then until frame 320 a looping quake, his wind on fighters (`xDC`), and every 10th
/// frame a wind source that sways their parts.
#[doc(alias = "grOldPupupu_802113E0")]
pub fn proc_whispy(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let v = r.game.grounds[g.0].u.oldpupupu();
    v.xE0 = v.xE0.wrapping_add(1);
    v.xDC = 0;
    match v.xC8 {
        0 => {
            if r.game.grounds[g.0].u.oldpupupu().xD4 != 0 {
                let p = param(r.game);
                let (min, max) = (p.i(r, 0xC), p.i(r, 8));
                let v = r.game.grounds[g.0].u.oldpupupu();
                v.xD4 = 0;
                v.xCC = 0;
                let wait = randrange(r, min, max);
                let v = r.game.grounds[g.0].u.oldpupupu();
                v.xD0 = wait;
                let anim = pick(WHISPY_IDLE, v.xCC, v.xD8);
                anime::start_map(r.game, r.data, gobj, m, anim);
                sync_face(r.game, gobj);
                let b = blink_wait(r);
                r.game.grounds[g.0].u.oldpupupu().xCC = b;
            }
            let v = r.game.grounds[g.0].u.oldpupupu();
            v.xD0 = v.xD0.wrapping_sub(1);
            if anime::finished(r.game, gobj, 0, 7) {
                if r.game.grounds[g.0].u.oldpupupu().xD0 > 0 {
                    let v = r.game.grounds[g.0].u.oldpupupu();
                    v.xCC = v.xCC.wrapping_sub(1);
                    if v.xCC <= 0 {
                        v.xCC = 0;
                        let anim = pick(WHISPY_IDLE, v.xCC, v.xD8);
                        anime::start_map(r.game, r.data, gobj, m, anim);
                        let b = blink_wait(r);
                        r.game.grounds[g.0].u.oldpupupu().xCC = b;
                    }
                    sync_face(r.game, gobj);
                    return;
                }
                advance(r.game, gobj);
                sync_face(r.game, gobj);
            }
        }
        1 => {
            if r.game.grounds[g.0].u.oldpupupu().xD4 != 0 {
                r.game.grounds[g.0].u.oldpupupu().xCC = 0;
                let j = jobj_of(r.game, gobj).expect("Whispy's model");
                let pos = r.game.objs.jobjs[j].translate;
                let dir = super::super::it::owner::opponents_dir(r.game, r.data, pos, None);
                let side_f = fp::frsp(if reg(dir) == 1.0 { 1.0 } else { 0.0 });
                let v = r.game.grounds[g.0].u.oldpupupu();
                if fp::frsp(f64::from(v.xD8)) == side_f {
                    v.xC8 = 2;
                    v.xD4 = 0;
                    v.xCC = 0;
                    v.xD0 = 0;
                    let anim = pick(WHISPY_BLOW, v.xCC, v.xD8);
                    anime::start_map(r.game, r.data, gobj, m, anim);
                    advance(r.game, gobj);
                    sync_face(r.game, gobj);
                    sync_face(r.game, gobj);
                } else {
                    v.xD8 = fp::fctiwz(side_f);
                    v.xD4 = 0;
                    v.xCC = 0;
                    v.xD0 = 0;
                    let anim = pick(WHISPY_TURN, v.xCC, v.xD8);
                    anime::start_map(r.game, r.data, gobj, m, anim);
                    sync_face(r.game, gobj);
                }
            }
            if r.game.grounds[g.0].u.oldpupupu().xC8 == 1 && anime::finished(r.game, gobj, 0, 7) {
                advance(r.game, gobj);
                sync_face(r.game, gobj);
            }
        }
        2 => {
            if r.game.grounds[g.0].u.oldpupupu().xD4 != 0 {
                let v = r.game.grounds[g.0].u.oldpupupu();
                v.xD4 = 0;
                v.xCC = 0;
                v.xD0 = 0;
                let anim = pick(WHISPY_BLOW, v.xCC, v.xD8);
                anime::start_map(r.game, r.data, gobj, m, anim);
                sync_face(r.game, gobj);
            }
            if anime::finished(r.game, gobj, 0, 7) {
                advance(r.game, gobj);
                sync_face(r.game, gobj);
                return;
            }
            let v = r.game.grounds[g.0].u.oldpupupu();
            v.xD0 = v.xD0.wrapping_add(1);
            let (t, side) = (v.xD0, v.xD8);
            if t == 45 {
                r.sfx_play(if side == 0 { 0x72BF0 } else { 0x72BF1 }, 0x7F, 0x40);
            }
            if t > 45 && t < 0x140 {
                super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
                r.game.grounds[g.0].u.oldpupupu().xDC = side.wrapping_add(1);
                if t % 10 == 0 {
                    let p = param(r.game);
                    let (x0, x1) = if side == 0 {
                        (p.f(r, 0x20), p.f(r, 0x1C))
                    } else {
                        (p.f(r, 0x14), p.f(r, 0x18))
                    };
                    let (y0, y1) = (p.f(r, 0x24), p.f(r, 0x28));
                    let dir = WIND_DIR[usize::try_from(side).expect("Whispy's side")];
                    crate::lb::wind::push(
                        &mut r.game.gr.winds,
                        dir,
                        15,
                        0.5,
                        0.0,
                        0.0,
                        x0,
                        y0,
                        x1,
                        y1,
                    );
                }
            }
        }
        _ => {}
    }
}

/// The leaves' process (map 6), `grOldPupupu_80211C1C`: when Whispy starts blowing, the
/// leaves' animation for his side.
#[doc(alias = "grOldPupupu_80211C1C")]
pub fn proc_leaves(r: &mut Running, gobj: GObjId) {
    let w = map_gobj(r.game, MAP_WHISPY).expect("Ground_GetMapGObj(7)");
    let wg = ground_of(r.game, w);
    let o = r.game.grounds[wg.0].u.oldpupupu();
    if o.xC8 == 2 && o.xD4 != 0 {
        let anim = pick(LEAVES_BLOW, o.xCC, o.xD8);
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, anim);
    }
}

/// The stage's shadow check, `grOldPupupu_80211CA8`: a fighter casts on a joint below its
/// feet.
#[doc(alias = "grOldPupupu_80211CA8")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = crate::lb::joint::world_position(&mut game.objs, data, j, None);
    reg(pos.y) > reg(b.y)
}
