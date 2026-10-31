//! Green Greens, `melee/gr/grgreens.c`: the stage with its falling blocks (map 6), Whispy Woods
//! (map 5) and the background that moves with him (map 4), a block and a bomb block as they fall
//! (maps 1 and 3), and maps 0 and 2.
//!
//! Whispy idles, blows the fighters away from the side most of them are on (a wind device the
//! fighter code polls, [`wind_push`], and a wind source that sways jointed parts), and drops
//! apples (`it::kinds::whispyapple`), in a fixed cycle. Blocks fall into a grid of six columns
//! of five on the stage's two sides, each a collision joint and a hurtbox item
//! (`it::kinds::yaku`); a hit or a landing breaks a block, and a bomb block explodes. Blocks
//! above a broken one fall into its place.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{GREENS_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, point_joint, scale};
use super::vars::GreensBlock;
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// Whispy's map id, and the background's that moves with him.
pub const MAP_WHISPY: i32 = 5;
pub const MAP_BACK: i32 = 4;
/// The stage's map id, which holds the blocks.
pub const MAP_BLOCKS: i32 = 6;

/// `ftCo_800C06E8`'s device type for Whispy's wind.
const WIND_DEVICE: u32 = 9;
/// `fn_80213B1C`, Whispy's wind as the fighter code polls it.
pub const WIND_PUSH: u32 = 0x8021_3B1C;
/// `fn_80216DE4`, the blocks' collision joints' callback (`mpJointSetCb2`).
pub const BLOCK_LANDED: u32 = 0x8021_6DE4;
/// `fn_802159B4`, `fn_80215B84` and `fn_80215D50`: a block's hurtbox item's callbacks for a hit
/// it deals, one it takes and a touch.
pub const BLOCK_DEALT: u32 = 0x8021_59B4;
pub const BLOCK_HIT: u32 = 0x8021_5B84;
pub const BLOCK_TOUCHED: u32 = 0x8021_5D50;
/// `fn_80214658`, the blocks' start hook.
const START_HOOK: u32 = 0x8021_4658;

/// Where the calls that make the stage's objects put their scale joints' descriptors on the
/// stack: the stage's setup, as deep as Dream Land's (0x10 below `setup::SETUP_FRAME`, its
/// frame being 0x18 bytes); the blocks' setup through `grGreens_80214FA8` and
/// `grGreens_80215358` (0x100 below that); and the blocks' process through
/// `grGreens_802166C4` and `grGreens_80215358` (0x118 below the process loop's,
/// `corneria::PROC_LOOP`).
pub const SETUP_FRAME: u32 = 0x804E_E9F0;
pub const SETUP_BLOCK_FRAME: u32 = 0x804E_E8F0;
pub const PROC_BLOCK_FRAME: u32 = super::corneria::PROC_LOOP - 0x118;

/// The blocks' joints in the stage's model (`grGr_803E7840`) and their collision joints
/// (`grGr_803E787C`), by block number.
const BLOCK_JOINTS: [i32; 30] = [
    5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22,
];
const BLOCK_COLL_JOINTS: [i32; 30] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
];

/// The grid's rows and columns.
pub const ROWS: usize = 5;
pub const COLS: usize = 6;

/// Whispy's cycle of states (`grGr_803E7734`): idle, blowing, dropping apples.
const CYCLE: [i32; 10] = [0, 1, 0, 1, 0, 2, 0, 1, 0, 2];
/// Whispy's idle animations and the background's (`grGr_803E775C`, `grGr_803E77B4`).
const IDLE: [i32; 8] = [0, 0, 0, 14, 0, 0, 0, 14];
const IDLE_BACK: [i32; 8] = [0, 0, 0, 14, 0, 0, 0, 14];
/// His blowing's animations by step and side, and the background's (`grGr_803E777C`,
/// `grGr_803E77D4`).
const BLOW: [[i32; 2]; 7] = [[1, 8], [15, 16], [2, 9], [3, 10], [4, 11], [5, 12], [6, 13]];
const BLOW_BACK: [[i32; 2]; 7] = [[1, 8], [15, 16], [2, 9], [3, 10], [4, 11], [5, 12], [6, 13]];
/// The wind's directions by side (`grGr_803E780C`).
const WIND_DIR: [Vec3; 2] = [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];

/// The fields of `grGreens_YakumonoParam`, by offset.
const BLOCK_TIMER_MIN: u32 = 0;
const BLOCK_TIMER_MAX: u32 = 4;
/// The columns' weights by their top block's row (4 down to 0), and an empty column's.
const WEIGHT_ROW4: u32 = 8;
const WEIGHT_ROW3: u32 = 0xC;
const WEIGHT_ROW2: u32 = 0x10;
const WEIGHT_ROW1: u32 = 0x14;
const WEIGHT_ROW0: u32 = 0x18;
const WEIGHT_EMPTY: u32 = 0x1C;
/// One in this many blocks is a plain one; the rest are bomb blocks.
const BOMB_ODDS: u32 = 0x20;
/// The hurtboxes' radii for a block and a bomb block.
const BLOCK_RADIUS: u32 = 0x24;
const BOMB_RADIUS: u32 = 0x28;
/// A falling block's top speed and its acceleration.
const FALL_MAX: u32 = 0x2C;
const FALL_ACCEL: u32 = 0x30;
const WIND_TIMER_MIN: u32 = 0x34;
const WIND_TIMER_MAX: u32 = 0x38;
const WIND_SPEED: u32 = 0x3C;
const WIND_LEFT: u32 = 0x40;
const WIND_RIGHT: u32 = 0x44;
const WIND_TOP: u32 = 0x48;
const WIND_BOTTOM: u32 = 0x4C;
/// How long Whispy draws breath, blows at most, and blows at least before he stops for
/// fighters gone to his other side.
const BREATH: u32 = 0x50;
const BLOW_MAX: u32 = 0x54;
const BLOW_MIN: u32 = 0x58;
/// The apples' count's bounds, their first tick and their interval.
const APPLES_MAX: u32 = 0x5C;
const APPLES_MIN: u32 = 0x60;
const APPLE_START: u32 = 0x64;
const APPLE_EVERY: u32 = 0x68;
/// The apples' heights' bounds, and their odds of being kept and of healing.
const APPLE_Y_MIN: u32 = 0x6C;
const APPLE_Y_MAX: u32 = 0x70;
const APPLE_KEEP: u32 = 0x74;
const APPLE_HEAL: u32 = 0x78;

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
    Param(game.gr.greens_yakumono.expect("grgreens.c: yakumono_param"))
}

/// grgreens.c's statics.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// Whether a demo set the stage up (`grGr_804D6AAC`): the setup keeps the last layout and
    /// puts the players' start points' joints on their start points.
    pub demo: u8,
    /// Whether the blocks wait for the match's start (`grGr_804D6AAD`), which holds Whispy too.
    pub waiting: u8,
    /// The blocks the match starts with, by grid place (`grGr_8049F9E0`): 1 a block, 2 a bomb
    /// block, 0 none.
    pub layout: [u8; 0x20],
}

/// A stage object's process or callback, by address: Green Greens'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_38B4 | 0x8021_38B8 | 0x8021_3908 | 0x8021_390C | 0x8021_3944 | 0x8021_3948
        | 0x8021_3980 | 0x8021_3984 | 0x8021_39BC | 0x8021_39C0 | 0x8021_4654 => empty(r, gobj),
        0x8021_360C => init_main(r, gobj),
        0x8021_38BC => init_two(r, gobj),
        0x8021_3910 | 0x8021_394C => init_block(r, gobj),
        0x8021_3988 => init_back(r, gobj),
        0x8021_39C4 => init_whispy(r, gobj),
        0x8021_3C10 => proc_whispy(r, gobj),
        START_HOOK => start_blocks(r, gobj),
        0x8021_4674 => init_stage(r, gobj),
        0x8021_479C => proc_stage(r, gobj),
        0x8021_4804 => destroy_stage(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8021_3464 => on_init(r),
        0x8021_34F4 => on_load(r),
        0x8021_34F8 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grGreens_802138B4")]
#[doc(alias = "grGreens_802138B8")]
#[doc(alias = "grGreens_80213908")]
#[doc(alias = "grGreens_8021390C")]
#[doc(alias = "grGreens_80213944")]
#[doc(alias = "grGreens_80213948")]
#[doc(alias = "grGreens_80213980")]
#[doc(alias = "grGreens_80213984")]
#[doc(alias = "grGreens_802139BC")]
#[doc(alias = "grGreens_802139C0")]
#[doc(alias = "grGreens_80214654")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grGreens_80213524`.
#[doc(alias = "grGreens_80213524")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grGr_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &GREENS_CALLBACKS[map_id as usize]
}

/// Green Greens' setup, `grGreens_Init`: its parameters, maps 0, 2, 6, 5 and 4, camera limits,
/// blast zones and the players' start points; a demo's setup is over.
#[doc(alias = "grGreens_Init")]
pub fn on_init(r: &mut Running) {
    r.game.gr.greens_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 2, MAP_BLOCKS, MAP_WHISPY, MAP_BACK] {
        create(r, map_id, SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    grlib::measure_start_points(r.game, r.data);
    r.game.gr.greens.demo = 0;
}

/// A demo's setup, `grGreens_80213458`: the next setup is a demo's.
#[doc(alias = "grGreens_80213458")]
pub fn on_demo_init(r: &mut Running) {
    r.game.gr.greens.demo = 1;
}

/// `grGreens_802134F4` does nothing.
#[doc(alias = "grGreens_802134F4")]
pub fn on_load(_r: &mut Running) {}

/// Green Greens' start, `grGreens_802134F8`: the enemy spawner, without enemies.
#[doc(alias = "grGreens_802134F8")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Map 0's setup, `grGreens_8021360C`: its animation, and in a demo the players' start points'
/// joints moved to the start points.
#[doc(alias = "grGreens_8021360C")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    if r.game.gr.greens.demo != 0 {
        for i in 0..4 {
            let v = grlib::start_point(r.game, i);
            let j = point_joint(r.game, i).expect("Ground_801C2CF4");
            jobj::set_translate(&mut r.game.objs.jobjs, j, v);
        }
    }
}

/// Map 2's setup, `grGreens_802138BC`: its animation, drawn in the background layer.
#[doc(alias = "grGreens_802138BC")]
pub fn init_two(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// A block's and a bomb block's setup (maps 1 and 3), `stageGObj1_OnInit` and
/// `stageGObj3_OnInit`: their animation.
#[doc(alias = "stageGObj1_OnInit@80213910")]
#[doc(alias = "stageGObj3_OnInit@8021394C")]
pub fn init_block(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 4), `grGreens_80213988`: its animation.
#[doc(alias = "grGreens_80213988")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// `randrange`: between the two, whichever is lower, drawn only if they differ.
fn randrange(r: &mut Running, min: i32, max: i32) -> i32 {
    grlib::rand_range(&mut r.game.objs.rng, min, max)
}

/// Whispy's setup (map 5), `grGreens_802139C4`: his wind registered with the fighter code, the
/// first wait, and a side at random.
#[doc(alias = "grGreens_802139C4")]
pub fn init_whispy(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    super::oldpupupu::register_wind(r.game, gobj, WIND_DEVICE, CodeRef(WIND_PUSH));
    let p = param(r.game);
    let (max, min) = (p.i(r.data, WIND_TIMER_MAX), p.i(r.data, WIND_TIMER_MIN));
    let v = r.game.grounds[g.0].u.greens2();
    v.x0 = 0;
    v.x4 = 0;
    v.x8 = 0;
    let wait = randrange(r, max, min);
    let v = r.game.grounds[g.0].u.greens2();
    v.xC = wait;
    v.x10 = 1;
    v.x1C = 0;
    let side = r.game.objs.rng.below(2);
    let v = r.game.grounds[g.0].u.greens2();
    v.x14 = side;
    v.x18 = 0;
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Whether `pos` lies strictly inside the box the two x and two y bounds make, in either
/// order, `grGreens_80213AB4` (Dream Land's [`super::oldpupupu::in_box`]).
#[doc(alias = "grGreens_80213AB4")]
pub fn in_box(pos: Vec3, max_x: f32, min_x: f32, max_y: f32, min_y: f32) -> bool {
    super::oldpupupu::in_box(pos, max_x, min_x, max_y, min_y)
}

/// Whispy's wind on a fighter, `fn_80213B1C`: while he blows (`x18` 1 to the left, 2 to the
/// right), a fighter in the box on his side is pushed at the parameters' speed.
#[doc(alias = "fn_80213B1C")]
pub fn wind_push(r: &mut Running, gobj: GObjId, fighter: GObjId) -> Option<Vec3> {
    let pos = r.game.fighter(fighter).cur_pos;
    let p = param(r.game);
    let g = ground_of(r.game, gobj);
    let blowing = r.game.grounds[g.0].u.greens2().x18;
    let d = r.data;
    let neg = |x: f32| mem(fp::fneg(reg(x)));
    let (top, bottom) = (p.f(d, WIND_TOP), p.f(d, WIND_BOTTOM));
    let (left, right) = (p.f(d, WIND_LEFT), p.f(d, WIND_RIGHT));
    let x = match blowing {
        1 if in_box(pos, neg(left), neg(right), top, bottom) => neg(p.f(d, WIND_SPEED)),
        2 if in_box(pos, right, left, top, bottom) => p.f(d, WIND_SPEED),
        _ => return None,
    };
    Some(Vec3::new(x, 0.0, 0.0))
}

/// Whispy's next state in the cycle.
fn advance(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.greens2();
    v.x0 = v.x0.wrapping_add(1).wrapping_rem(10);
    v.x4 = CYCLE[v.x0 as usize];
    v.x10 = 1;
}

/// Starts Whispy's and the background's animations.
fn animate(r: &mut Running, gobj: GObjId, back: GObjId, anim: i32, back_anim: i32) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
    let bm = map_id(r.game, back);
    anime::start_map(r.game, r.data, back, bm, back_anim);
}

/// The blowing step's animations for the step and side Whispy is on.
fn blow_anims(game: &mut Game, gobj: GObjId) -> (i32, i32) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.greens2();
    let (step, side) = (v.x8 as usize, v.x14 as usize);
    (BLOW[step][side], BLOW_BACK[step][side])
}

/// The side most fighters are on as Whispy sees it: 1 right, 0 left.
fn whispy_dir(r: &mut Running, gobj: GObjId) -> i32 {
    let j = jobj_of(r.game, gobj).expect("Whispy's model");
    let pos = r.game.objs.jobjs[j].translate;
    let dir = super::super::it::owner::opponents_dir(r.game, r.data, pos, None);
    i32::from(reg(dir) == 1.0)
}

/// A wind source on Whispy's side that sways the fighters' parts.
fn blow_wind(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let side = r.game.grounds[g.0].u.greens2().x14;
    let p = param(r.game);
    let d = r.data;
    let neg = |x: f32| mem(fp::fneg(reg(x)));
    let (top, bottom) = (p.f(d, WIND_TOP), p.f(d, WIND_BOTTOM));
    let (left, right) = (p.f(d, WIND_LEFT), p.f(d, WIND_RIGHT));
    let (x0, x1) = if side == 0 {
        (neg(right), neg(left))
    } else {
        (left, right)
    };
    let dir = WIND_DIR[usize::try_from(side).expect("Whispy's side")];
    crate::lb::wind::push(
        &mut r.game.gr.winds,
        dir,
        15,
        0.5,
        0.0,
        0.0,
        x0,
        top,
        x1,
        bottom,
    );
}

/// Whispy's process (map 5), `grGreens_80213C10`, which waits while the blocks do. Idle (0),
/// he plays random idles for a random while. Blowing (1), he turns to the side most fighters
/// are on, draws breath with its sounds, and blows (pushing fighters on his side, `x18`, with
/// a looping quake and a wind source) until he has blown his longest, or long enough and the
/// fighters have gone to his other side. Dropping apples (2), he shakes, dropping a random
/// count of apples on his side, every so many ticks, alternating left and right.
#[doc(alias = "grGreens_80213C10")]
pub fn proc_whispy(r: &mut Running, gobj: GObjId) {
    let back = map_gobj(r.game, MAP_BACK).expect("Ground_GetMapGObj(4)");
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.greens2().x18 = 0;
    if r.game.gr.greens.demo != 0 || r.game.gr.greens.waiting != 0 {
        return;
    }
    let p = param(r.game);
    let d = r.data;
    let v = r.game.grounds[g.0].u.greens2();
    v.x1C = v.x1C.wrapping_add(1);
    match v.x4 {
        0 => {
            if v.x10 != 0 {
                v.x10 = 0;
                let idle = r.game.objs.rng.below(8);
                r.game.grounds[g.0].u.greens2().x8 = idle;
                let wait = randrange(r, p.i(d, WIND_TIMER_MAX), p.i(d, WIND_TIMER_MIN));
                r.game.grounds[g.0].u.greens2().xC = wait;
                animate(r, gobj, back, IDLE[idle as usize], IDLE_BACK[idle as usize]);
                return;
            }
            v.xC = v.xC.wrapping_sub(1);
            if anime::rewound(r.game, gobj, 0, 7) {
                if r.game.grounds[g.0].u.greens2().xC > 0 {
                    let idle = r.game.objs.rng.below(8);
                    r.game.grounds[g.0].u.greens2().x8 = idle;
                    animate(r, gobj, back, IDLE[idle as usize], IDLE_BACK[idle as usize]);
                    return;
                }
                advance(r.game, gobj);
            }
        }
        1 => {
            if v.x10 != 0 {
                v.x10 = 0;
                v.x8 = 0;
                let dir = whispy_dir(r, gobj);
                let v = r.game.grounds[g.0].u.greens2();
                v.x14 = dir;
                v.xC = 0;
                let (a, b) = blow_anims(r.game, gobj);
                animate(r, gobj, back, a, b);
                return;
            }
            let next = |r: &mut Running| {
                let v = r.game.grounds[g.0].u.greens2();
                v.x8 = v.x8.wrapping_add(1);
            };
            let step = v.x8;
            match step {
                0 | 5 => {
                    if anime::finished(r.game, gobj, 0, 7) {
                        next(r);
                        let (a, b) = blow_anims(r.game, gobj);
                        animate(r, gobj, back, a, b);
                    }
                }
                1 => {
                    if anime::finished(r.game, gobj, 0, 7) {
                        next(r);
                        let (a, b) = blow_anims(r.game, gobj);
                        animate(r, gobj, back, a, b);
                        r.stage_sound(0x68FB8);
                    }
                }
                2 => {
                    if anime::rewound(r.game, gobj, 0, 7) {
                        let v = r.game.grounds[g.0].u.greens2();
                        v.xC = v.xC.wrapping_add(1);
                        if fp::frsp(f64::from(v.xC)) > reg(p.f(d, BREATH)) {
                            v.x8 = v.x8.wrapping_add(1);
                            v.xC = 0;
                            let side = v.x14;
                            let (a, b) = blow_anims(r.game, gobj);
                            animate(r, gobj, back, a, b);
                            r.stage_sound(if side == 0 { 0x68FB0 } else { 0x68FB1 });
                        }
                    }
                }
                3 => {
                    if anime::finished(r.game, gobj, 0, 7) {
                        next(r);
                        let (a, b) = blow_anims(r.game, gobj);
                        let m = map_id(r.game, gobj);
                        anime::start_map(r.game, r.data, gobj, m, a);
                        blow_wind(r, gobj);
                        let bm = map_id(r.game, back);
                        anime::start_map(r.game, r.data, back, bm, b);
                    }
                }
                4 => {
                    super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
                    let v = r.game.grounds[g.0].u.greens2();
                    v.x18 = v.x14.wrapping_add(1);
                    if anime::rewound(r.game, gobj, 0, 7) {
                        let v = r.game.grounds[g.0].u.greens2();
                        v.xC = v.xC.wrapping_add(1);
                        let t = fp::frsp(f64::from(v.xC));
                        let side = v.x14;
                        let done = t > reg(p.f(d, BLOW_MAX))
                            || t > reg(p.f(d, BLOW_MIN)) && side != whispy_dir(r, gobj);
                        if done {
                            let v = r.game.grounds[g.0].u.greens2();
                            v.x8 = v.x8.wrapping_add(1);
                            v.xC = 0;
                            let (a, b) = blow_anims(r.game, gobj);
                            animate(r, gobj, back, a, b);
                            return;
                        }
                        blow_wind(r, gobj);
                    }
                }
                6 if anime::finished(r.game, gobj, 0, 7) => advance(r.game, gobj),
                _ => {}
            }
        }
        2 => {
            if v.x10 != 0 {
                v.x10 = 0;
                v.x8 = 0;
                v.xC = 0;
                animate(r, gobj, back, 7, 7);
                r.stage_sound(0x68FB2);
                let max = p.i(d, APPLES_MAX).wrapping_sub(1);
                let count = randrange(r, p.i(d, APPLES_MIN), max);
                r.game.grounds[g.0].u.greens2().x20 = count;
                let side = r.game.objs.rng.below(2);
                r.game.grounds[g.0].u.greens2().x24 = side;
            } else if anime::finished(r.game, gobj, 0, 7) {
                if r.game.grounds[g.0].u.greens2().x20 < 0 {
                    advance(r.game, gobj);
                } else {
                    animate(r, gobj, back, 7, 7);
                }
            } else if r.game.grounds[g.0].u.greens2().x20 >= 0 {
                let (start, every) = (p.i(d, APPLE_START), p.i(d, APPLE_EVERY));
                let t = r.game.grounds[g.0].u.greens2().xC;
                if t >= start && t.wrapping_sub(start).wrapping_rem(every) == 0 {
                    drop_apple(r, gobj);
                }
            }
            let v = r.game.grounds[g.0].u.greens2();
            v.xC = v.xC.wrapping_add(1);
        }
        _ => {}
    }
}

/// Drops an apple from Whispy's branches on the side `x24` names, at a random distance out
/// and a random height, and turns to the other side.
fn drop_apple(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    let rand = reg(r.game.objs.rng.draw_f32());
    let sign = if r.game.grounds[g.0].u.greens2().x24 == 0 {
        fp::fneg(1.0)
    } else {
        1.0
    };
    let x = mem(fp::fmuls(sign, fp::fmuls(40.0, rand)));
    let rand = reg(r.game.objs.rng.draw_f32());
    let (lo, hi) = (reg(p.f(d, APPLE_Y_MIN)), reg(p.f(d, APPLE_Y_MAX)));
    let y = mem(fp::fmadds(fp::fsubs(hi, lo), rand, lo));
    let pos = Vec3::new(x, y, -40.0);
    r.stage_sound(0x68FB3);
    let (keep, heal) = (p.f(d, APPLE_KEEP), p.f(d, APPLE_HEAL));
    super::super::it::kinds::whispyapple::spawn(r, Some(gobj), pos, keep, heal);
    let v = r.game.grounds[g.0].u.greens2();
    v.x20 = v.x20.wrapping_sub(1);
    v.x24 = v.x24.wrapping_add(1) & 1;
}

/// The blocks' start hook, `fn_80214658`: the blocks and Whispy run.
#[doc(alias = "fn_80214658")]
pub fn start_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.greens().set_running(false);
    r.game.gr.greens.waiting = 0;
}

/// The stage's setup (map 6), `grGreens_80214674`: its collision, the grid and the blocks
/// (from the heap, cleared), the grid measured from the blocks' joints, a new layout unless a
/// demo keeps the last, the blocks the layout names, the wait for the match's start and for the
/// first new block.
#[doc(alias = "grGreens_80214674")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let grid = crate::hsd::heap::mem_alloc((ROWS * COLS * 12) as u32);
    let v = r.game.grounds[g.0].u.greens();
    v.x4 = grid;
    v.grid.clear();
    v.grid.resize(ROWS * COLS, Vec3::ZERO);
    let blocks = crate::hsd::heap::mem_alloc((ROWS * COLS * 0x20) as u32);
    let v = r.game.grounds[g.0].u.greens();
    v.x8 = blocks;
    v.blocks.clear();
    v.blocks.resize(ROWS * COLS, GreensBlock::default());
    measure_grid(r, gobj);
    if r.game.gr.greens.demo == 0 {
        lay_out(r);
    }
    r.game.gr.greens.demo = 0;
    place_blocks(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    let p = param(r.game);
    let (min, max) = (p.i(r.data, BLOCK_TIMER_MIN), p.i(r.data, BLOCK_TIMER_MAX));
    let wait = randrange(r, min, max);
    let v = r.game.grounds[g.0].u.greens();
    v.xC = wait;
    v.set_running(true);
    r.game.gr.greens.waiting = 1;
}

/// The stage's process (map 6), `grGreens_8021479C`: once the match runs, the blocks; the
/// collision follows the model, then the blocks' collision joints are linked; the wind sources
/// age.
#[doc(alias = "grGreens_8021479C")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if !r.game.grounds[g.0].u.greens().running() {
        proc_blocks(r, gobj);
    }
    super::ground::update_map_coll(r.game, r.data, gobj);
    if !r.game.grounds[g.0].u.greens().running() {
        link_blocks(r, gobj);
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The stage's destroy callback (map 6), `grGreens_80214804`: the grid and the blocks back to
/// the heap.
#[doc(alias = "grGreens_80214804")]
pub fn destroy_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.greens();
    let (grid, blocks) = (v.x4, v.x8);
    crate::hsd::heap::mem_free(grid);
    crate::hsd::heap::mem_free(blocks);
}

/// Measures the grid from the blocks' joints, `grGreens_8021483C`: on each side (left of
/// x = 0 and right), three columns spread evenly between the leftmost and rightmost joint and
/// five rows between the lowest and highest.
#[doc(alias = "grGreens_8021483C")]
pub fn measure_grid(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let (lo, hi) = (reg(-3.402_823_5e38), reg(3.402_823_5e38));
    let (mut l_max, mut l_min, mut l_top, mut l_bottom) = (lo, hi, lo, hi);
    let (mut r_max, mut r_top, mut r_min, mut r_bottom) = (lo, lo, hi, hi);
    for &joint in &BLOCK_JOINTS {
        let j = nth_joint(r.game, gobj, joint);
        assert!(j.is_some(), "grgreens.c: 1014: jobj");
        let v = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
        let (x, y) = (reg(v.x), reg(v.y));
        if x < 0.0 {
            if l_max < x {
                l_max = x;
            }
            if l_min > x {
                l_min = x;
            }
            if l_top < y {
                l_top = y;
            }
            if l_bottom > y {
                l_bottom = y;
            }
        } else {
            if r_max < x {
                r_max = x;
            }
            if r_min > x {
                r_min = x;
            }
            if r_top < y {
                r_top = y;
            }
            if r_bottom > y {
                r_bottom = y;
            }
        }
    }
    let grid = &mut r.game.grounds[g.0].u.greens().grid;
    for (cols, max, min, top, bottom) in [
        (0..3, l_max, l_min, l_top, l_bottom),
        (3..6, r_max, r_min, r_top, r_bottom),
    ] {
        let x_step = fp::fmuls(fp::fsubs(max, min), 0.5);
        let y_step = fp::fmuls(fp::fsubs(top, bottom), 0.25);
        let xs = [0.0, 1.0, 2.0].map(|k| mem(fp::fmadds(x_step, k, min)));
        for row in 0..ROWS {
            let y = mem(fp::fmadds(y_step, row as f64, bottom));
            for (k, col) in cols.clone().enumerate() {
                grid[row * COLS + col] = Vec3::new(xs[k], y, 0.0);
            }
        }
    }
}

/// A block in the layout, one in [`BOMB_ODDS`] a plain block (1), else a bomb block (2).
fn layout_block(r: &mut Running) -> u8 {
    let odds = param(r.game).i(r.data, BOMB_ODDS);
    let roll = if odds != 0 {
        r.game.objs.rng.below(odds)
    } else {
        0
    };
    if roll != 0 { 1 } else { 2 }
}

/// Draws the layout the match starts with, `grGreens_80214B58`: the bottom three rows of each
/// column but the outer ones' (18 places), each a block or a bomb block.
#[doc(alias = "grGreens_80214B58")]
pub fn lay_out(r: &mut Running) {
    r.game.gr.greens.layout[..30].fill(0);
    for i in [
        0x12, 0x0C, 0x0D, 0x06, 0x07, 0x08, 0x00, 0x01, 0x02, 0x17, 0x10, 0x11, 0x09, 0x0A, 0x0B,
        0x03, 0x04, 0x05,
    ] {
        let b = layout_block(r);
        r.game.gr.greens.layout[i] = b;
    }
}

/// Sets the blocks up, `grGreens_80214FA8`: their collision joints tell the stage what lands
/// on them, their joints hidden, and the layout's blocks placed (at rest).
#[doc(alias = "grGreens_80214FA8")]
pub fn place_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for &joint in &BLOCK_COLL_JOINTS {
        r.game.geometry_mut().set_callback(
            joint,
            Some(super::CollUser::Ground(g)),
            Some(CodeRef(BLOCK_LANDED)),
        );
    }
    for &joint in &BLOCK_JOINTS {
        let j = nth_joint(r.game, gobj, joint);
        assert!(j.is_some(), "grgreens.c: 1203: 0");
        jobj::set_flags(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    }
    for row in 0..ROWS {
        for col in 0..COLS {
            let kind = r.game.gr.greens.layout[row * COLS + col];
            if kind == 1 || kind == 2 {
                add_block(
                    r,
                    gobj,
                    col as i32,
                    row as i32,
                    kind.into(),
                    3,
                    SETUP_BLOCK_FRAME,
                );
            }
        }
    }
}

/// The blocks, row by row.
fn blocks(game: &mut Game, g: GroundId) -> &mut Vec<GreensBlock> {
    &mut game.grounds[g.0].u.greens().blocks
}

fn at(row: i32, col: i32) -> usize {
    (row * COLS as i32 + col) as usize
}

/// The status of the block `i` places after the grid's first, where `i` may be -1: the 32 bytes
/// before the blocks' array are the heap cell's header, whose first word is a link (an address
/// whose top four bits, the status's, are 8, or null), never a resting block's 3.
fn status_at(blocks: &[GreensBlock], i: i32) -> u32 {
    usize::try_from(i).map_or(8, |i| blocks[i].status())
}

/// Links a block's collision with its diagonal neighbours' where both of them rest beside it,
/// `grGreens_802150C4` (after a block between them broke). The last corner tests the block
/// left of `col` where the others test the one at the corner's side, as the original does.
#[doc(alias = "grGreens_802150C4")]
pub fn link_corners(r: &mut Running, gobj: GObjId, col: i32, row: i32) {
    let g = ground_of(r.game, gobj);
    let b = blocks(r.game, g).clone();
    let st = |rr: i32, cc: i32| status_at(&b, rr * COLS as i32 + cc);
    let joint = |rr: i32, cc: i32| b[at(rr, cc)].x18;
    let link = |r: &mut Running, a: i32, c: i32| r.game.geometry_mut().link_joints(a, c);
    if row > 0 && col > 0 && st(row - 1, col - 1) == 3 {
        if st(row - 1, col) == 3 {
            link(r, joint(row - 1, col - 1), joint(row - 1, col));
        } else if st(row, col - 1) == 3 {
            link(r, joint(row - 1, col - 1), joint(row, col - 1));
        }
    }
    if row < 4 && col > 0 && st(row + 1, col - 1) == 3 {
        if st(row + 1, col) == 3 {
            link(r, joint(row + 1, col - 1), joint(row + 1, col));
        } else if st(row, col - 1) == 3 {
            link(r, joint(row + 1, col - 1), joint(row, col - 1));
        }
    }
    if row > 0 && col < 5 && st(row - 1, col + 1) == 3 {
        if st(row - 1, col) == 3 {
            link(r, joint(row - 1, col + 1), joint(row - 1, col));
        } else if st(row, col + 1) == 3 {
            link(r, joint(row - 1, col + 1), joint(row, col + 1));
        }
    }
    if row < 4 && col < 5 && st(row + 1, col + 1) == 3 {
        if st(row + 1, col) == 3 {
            link(r, joint(row + 1, col + 1), joint(row + 1, col));
            return;
        }
        if st(row, col - 1) == 3 {
            link(r, joint(row + 1, col + 1), joint(row, col + 1));
        }
    }
}

/// Places a block's object and its joint in the stage's model (which takes the stage's scale
/// back out) at the grid's column and `y`.
fn place(r: &mut Running, b: &GreensBlock, x: f32, y: f32) {
    let mut pos = Vec3::new(x, y, 0.0);
    let root =
        b.xC.and_then(|o| jobj_of(r.game, o))
            .expect("a block's object");
    jobj::set_translate(&mut r.game.objs.jobjs, root, pos);
    let f = fp::fdivs(1.0, reg(scale(r.game)));
    pos.x = mem(fp::fmuls(reg(pos.x), f));
    pos.y = mem(fp::fmuls(reg(pos.y), f));
    pos.z = mem(fp::fmuls(reg(pos.z), f));
    jobj::set_translate(&mut r.game.objs.jobjs, b.x14.expect("a block's joint"), pos);
}

/// Adds a block (`kind` 1) or a bomb block (2) at the grid's `col` and `row`,
/// `grGreens_80215358`, with `status` 1 (falling from the top of the blast zone) or 3 (at
/// rest): the first free block number's joint shown, its object (map 1 or 3) made, a hurtbox
/// item on the joint (on the ground, its radius the block's or the bomb's), and its collision
/// joint moved to it. `frame` is where the caller's call puts the object's scale joint's
/// descriptor on the stack.
#[doc(alias = "grGreens_80215358")]
#[allow(clippy::too_many_arguments)]
pub fn add_block(
    r: &mut Running,
    gobj: GObjId,
    col: i32,
    row: i32,
    kind: i32,
    status: u32,
    frame: u32,
) {
    let g = ground_of(r.game, gobj);
    let b = blocks(r.game, g);
    assert!(
        b[at(row, col)].status() == 0,
        "grgreens.c: 1305: block->status==Gr_Greens_Block_Status_None"
    );
    let mut used = [false; 30];
    for blk in b.iter() {
        if blk.status() != 0 {
            used[blk.index() as usize] = true;
        }
    }
    let num = used.iter().position(|&u| !u).unwrap_or(30);
    assert!(num < 30, "grgreens.c: 1316: num<Gr_Greens_Block_Max");
    let map = match kind {
        1 => 1,
        2 => 3,
        _ => panic!("grgreens.c: 1324: 0"),
    };
    let block_gobj = create(r, map, frame);
    assert!(block_gobj.is_some(), "grgreens.c: 1325: block_gobj");
    let joint = nth_joint(r.game, gobj, BLOCK_JOINTS[num]);
    jobj::clear_flags(&mut r.game.objs.jobjs, joint, jobj::HIDDEN);
    let item = material::stage_item(
        r,
        8,
        0,
        g,
        joint,
        Some(CodeRef(BLOCK_DEALT)),
        Some(CodeRef(BLOCK_HIT)),
        Some(CodeRef(BLOCK_TOUCHED)),
    )
    .expect("grgreens.c: a block's item");
    let p = param(r.game);
    let radius = p.i(r.data, if kind == 2 { BOMB_RADIUS } else { BLOCK_RADIUS });
    let s = reg(scale(r.game));
    let radius = mem(fp::fmuls(fp::frsp(f64::from(radius)), s));
    let up = mem(fp::fmuls(5.0, s));
    let a = Vec3::new(0.0, up, 0.0);
    material::set_item_hurtbox(r.game, item, a, a, radius);
    material::item_hurtbox_on(r.game, item);
    material::item_ground_or_air(r.game, item, 0);
    let top = super::stage::blast_top(r.game);
    let coll = setup::coll_joint_id(r.game, r.data, MAP_BLOCKS, BLOCK_JOINTS[num]);
    let v = r.game.grounds[g.0].u.greens();
    let cell = v.grid[at(row, col)];
    let b = &mut v.blocks[at(row, col)];
    b.set_status(status);
    b.set_index(num as u32);
    b.set_flag(1, kind == 2);
    b.set_flag(2, false);
    b.set_flag(3, false);
    b.x8 = if status == 1 { top } else { cell.y };
    b.x4 = 0.0;
    b.xC = block_gobj;
    b.x10 = Some(item);
    b.x14 = joint;
    b.x18 = coll;
    b.x1C = None;
    b.set_flag(4, false);
    b.set_flag(7, false);
    b.set_flag(5, status == 3);
    let b = b.clone();
    place(r, &b, cell.x, b.x8);
    super::ground::update_coll_joint(r.game, r.data, coll);
    r.game.geometry_mut().settle_vertices(coll);
}

/// Breaks the block at the grid's `col` and `row`, `grGreens_802159B8`, once: its hurtbox off
/// and what hit it kept. A bomb block hides, its item explodes (its state 1) with its effect; a
/// block shakes the camera and puffs with a sound, and is gone next.
#[doc(alias = "grGreens_802159B8")]
pub fn break_block(r: &mut Running, g: GroundId, col: i32, row: i32, by: Option<GObjId>) {
    let b = blocks(r.game, g)[at(row, col)].clone();
    let Some(item) = b.x10 else { return };
    if b.flag(7) {
        return;
    }
    let blk = &mut blocks(r.game, g)[at(row, col)];
    blk.set_flag(7, true);
    material::item_hurtbox_off(r.game, item);
    blocks(r.game, g)[at(row, col)].x1C = by;
    let s = reg(scale(r.game));
    if b.flag(1) {
        let j = b.xC.and_then(|o| jobj_of(r.game, o));
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        material::item_state(r, item, 1);
        super::super::it::hitbox::let_own_items_hit(r.game, item);
        blocks(r.game, g)[at(row, col)].set_flag(2, true);
        let mut v = item_translation(r.game, item);
        v.y = mem(fp::fmadds(5.0, s, reg(v.y)));
        super::super::ef::spawn(r, 1039, Some(item), &[super::super::ef::Arg::Vec(v)]);
    } else {
        blocks(r.game, g)[at(row, col)].set_flag(3, true);
        super::super::camera::request_quake(r, 2, None);
        let mut v = item_translation(r.game, item);
        v.y = mem(fp::fmadds(5.0, s, reg(v.y)));
        super::super::ef::spawn(
            r,
            1032,
            Some(item),
            &[
                super::super::ef::Arg::Vec(v),
                super::super::ef::Arg::Float(0.0),
            ],
        );
        r.stage_sound_on_track(430_007, 186);
    }
}

fn item_translation(game: &Game, item: GObjId) -> Vec3 {
    let j = jobj_of(game, item).expect("an item's model");
    game.objs.jobjs[j].translate
}

/// A block's item's callbacks for a hit it deals and a touch, `fn_802159B4` and `fn_80215D50`:
/// nothing.
#[doc(alias = "fn_802159B4")]
#[doc(alias = "fn_80215D50")]
pub fn block_item_nothing(_r: &mut Running, _item: GObjId, _g: GroundId) {}

/// A hit a block's item took, `fn_80215B84`: the block breaks.
#[doc(alias = "fn_80215B84")]
pub fn block_hit(r: &mut Running, item: GObjId, g: GroundId, by: Option<GObjId>) {
    let found = blocks(r.game, g)
        .iter()
        .position(|b| b.status() != 0 && b.x10 == Some(item));
    let i = found.expect("grgreens.c: 1465: 0") as i32;
    break_block(r, g, i % COLS as i32, i / COLS as i32, by);
}

/// Drops a column's falling blocks into the places below them left empty,
/// `grGreens_80215D54`: each empty place moves to the column's top, and a block that was
/// coming to rest on what broke falls again.
#[doc(alias = "grGreens_80215D54")]
pub fn settle_column(r: &mut Running, gobj: GObjId, col: i32, _row: i32) {
    let g = ground_of(r.game, gobj);
    let b = blocks(r.game, g);
    let mut row = 1;
    while row < 5 {
        let s = b[at(row, col)].status();
        if (s == 1 || s == 2) && b[at(row - 1, col)].status() == 0 {
            for rr in row..5 {
                b.swap(at(rr, col), at(rr - 1, col));
            }
            if b[at(row - 1, col)].status() == 2 {
                b[at(row - 1, col)].set_status(1);
            }
            row = 0;
        }
        row += 1;
    }
}

/// Moves the block at the grid's `col` and `row`, `grGreens_80215ED8`. A bomb block's waits for
/// its explosion's script to end. A falling block (1) breaks if something landed on it,
/// otherwise falls faster up to its top speed, coming to rest on the block below it (2); at its
/// place it rests (3) with the blocks resting on it, and the blocks resting on it follow it.
#[doc(alias = "grGreens_80215ED8")]
pub fn update_block(r: &mut Running, gobj: GObjId, col: i32, row: i32) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let i = at(row, col);
    let b = blocks(r.game, g)[i].clone();
    if b.status() == 0 || b.flag(3) {
        return;
    }
    if b.flag(2) {
        let item = b.x10.expect("a block's item");
        if material::item_script_done(r.game, item) {
            blocks(r.game, g)[i].set_flag(3, true);
        }
        return;
    }
    if b.status() != 1 {
        return;
    }
    let grid_y = |game: &mut Game, rr: i32| game.grounds[g.0].u.greens().grid[at(rr, col)].y;
    let grid_x = |game: &mut Game, rr: i32| game.grounds[g.0].u.greens().grid[at(rr, col)].x;
    if b.flag(4) {
        let blk = &mut blocks(r.game, g)[i];
        blk.x4 = 0.0;
        blk.set_flag(4, false);
        break_block(r, g, col, row, None);
    } else {
        let (accel, max) = (p.f(r.data, FALL_ACCEL), p.f(r.data, FALL_MAX));
        let spacing = if row > 0 {
            Some(mem(fp::fsubs(
                reg(grid_y(r.game, row)),
                reg(grid_y(r.game, row - 1)),
            )))
        } else {
            None
        };
        let bs = blocks(r.game, g);
        let blk = &mut bs[i];
        blk.x4 = mem(fp::fadds(reg(blk.x4), reg(accel)));
        if reg(blk.x4) > reg(max) {
            blk.x4 = max;
        }
        blk.x8 = mem(fp::fsubs(reg(blk.x8), reg(blk.x4)));
        if row > 0 {
            let below = bs[at(row - 1, col)].clone();
            if below.status() == 1 || below.status() == 2 {
                let spacing = reg(spacing.expect("a row above the first"));
                let blk = &mut bs[i];
                if fp::fsubs(reg(blk.x8), reg(below.x8)) < spacing {
                    blk.set_status(2);
                    blk.x8 = mem(fp::fadds(spacing, reg(below.x8)));
                }
            }
        }
    }
    let y = grid_y(r.game, row);
    if reg(blocks(r.game, g)[i].x8) < reg(y) {
        let blk = &mut blocks(r.game, g)[i];
        blk.x8 = y;
        blk.set_status(3);
        blk.set_flag(5, true);
        for next in row + 1..5 {
            let n = at(next, col);
            if blocks(r.game, g)[n].status() != 2 {
                break;
            }
            let (x, y) = (grid_x(r.game, next), grid_y(r.game, next));
            let blk = &mut blocks(r.game, g)[n];
            blk.x8 = y;
            blk.set_status(3);
            blk.set_flag(5, true);
            let blk = blk.clone();
            place(r, &blk, x, y);
        }
    } else {
        for next in row + 1..5 {
            let n = at(next, col);
            if blocks(r.game, g)[n].status() != 2 {
                break;
            }
            let x = grid_x(r.game, next);
            let spacing = fp::fsubs(reg(grid_y(r.game, next)), reg(grid_y(r.game, next - 1)));
            let below = blocks(r.game, g)[at(next - 1, col)].x8;
            let y = mem(fp::fadds(spacing, reg(below)));
            let blk = &mut blocks(r.game, g)[n];
            blk.x8 = y;
            let blk = blk.clone();
            place(r, &blk, x, y);
        }
    }
    let x = grid_x(r.game, row);
    let blk = blocks(r.game, g)[i].clone();
    place(r, &blk, x, blk.x8);
}

/// The blocks' tick, `grGreens_802166C4`. When the wait for a new block is over, a column is
/// drawn by its weight (by how high its blocks reach; a side with a full column is left out),
/// and a block or a bomb block falls into it, then a new wait. Every block moves, and every
/// block that broke is gone: its object, its item, its joint hidden, and the blocks above it
/// fall in.
#[doc(alias = "grGreens_802166C4")]
pub fn proc_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    let v = r.game.grounds[g.0].u.greens();
    let t = v.xC;
    v.xC = t.wrapping_sub(1);
    if t < 0 {
        let row_weights = [
            WEIGHT_ROW0,
            WEIGHT_ROW1,
            WEIGHT_ROW2,
            WEIGHT_ROW3,
            WEIGHT_ROW4,
        ];
        let mut weights = [0u8; 6];
        for (col, w) in weights.iter_mut().enumerate() {
            *w = p.i(d, WEIGHT_EMPTY) as u8;
            for row in (0..5).rev() {
                if blocks(r.game, g)[at(row, col as i32)].status() != 0 {
                    *w = p.i(d, row_weights[row as usize]) as u8;
                    break;
                }
            }
        }
        let left_full = weights[..3].contains(&0);
        let right_full = weights[3..].contains(&0);
        let pick_from = |r: &mut Running, ws: &[u8], first: i32| {
            let total: i32 = ws.iter().map(|&w| i32::from(w)).sum();
            let mut pick = if total != 0 {
                r.game.objs.rng.below(total)
            } else {
                0
            };
            let mut ix = first;
            for &w in ws {
                pick -= i32::from(w);
                if pick < 0 {
                    return ix;
                }
                ix += 1;
            }
            ix
        };
        let ix = if !left_full {
            if !right_full {
                let ix = pick_from(r, &weights, 0);
                assert!(ix < 6, "grgreens.c: 1693: ix<Gr_Greens_Block_Colum*2");
                ix
            } else {
                let ix = pick_from(r, &weights[..3], 0);
                assert!(ix < 3, "grgreens.c: 1702: ix<Gr_Greens_Block_Colum");
                ix
            }
        } else if !right_full {
            let ix = pick_from(r, &weights[3..], 3);
            assert!(ix < 6, "grgreens.c: 1711: ix<Gr_Greens_Block_Colum*2");
            ix
        } else {
            -1
        };
        if ix != -1 {
            let mut row = 4;
            while row > 0 {
                if blocks(r.game, g)[at(row - 1, ix)].status() != 0 {
                    break;
                }
                row -= 1;
            }
            let odds = p.i(d, BOMB_ODDS);
            let roll = if odds != 0 {
                r.game.objs.rng.below(odds)
            } else {
                0
            };
            let kind = if roll != 0 { 1 } else { 2 };
            add_block(r, gobj, ix, row, kind, 1, PROC_BLOCK_FRAME);
        }
        let (min, max) = (p.i(d, BLOCK_TIMER_MIN), p.i(d, BLOCK_TIMER_MAX));
        let wait = randrange(r, min, max);
        r.game.grounds[g.0].u.greens().xC = wait;
    }
    for row in 0..5 {
        for col in 0..6 {
            update_block(r, gobj, col, row);
        }
    }
    'restart: loop {
        for row in 0..5 {
            for col in 0..6 {
                let b = blocks(r.game, g)[at(row, col)].clone();
                if b.flag(3) {
                    let blk = &mut blocks(r.game, g)[at(row, col)];
                    blk.set_flag(3, false);
                    blk.set_flag(6, true);
                    blk.set_status(0);
                    setup::destroy(r, b.xC);
                    material::destroy_item(r, b.x10.expect("a block's item"));
                    jobj::set_flags(&mut r.game.objs.jobjs, b.x14, jobj::HIDDEN);
                    settle_column(r, gobj, col, row);
                    continue 'restart;
                }
            }
        }
        break;
    }
}

/// Links the collision of each block that came to rest with its resting neighbours', and of
/// the blocks around a place that broke with each other, `grGreens_80216C20`.
#[doc(alias = "grGreens_80216C20")]
pub fn link_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for row in 0..5i32 {
        for col in 0..6i32 {
            let b = blocks(r.game, g).clone();
            let me = &b[at(row, col)];
            if me.flag(5) {
                let x18 = me.x18;
                let resting = |rr: i32, cc: i32| b[at(rr, cc)].status() == 3;
                if row > 0 && resting(row - 1, col) {
                    r.game
                        .geometry_mut()
                        .link_joints(x18, b[at(row - 1, col)].x18);
                }
                if col > 0 && resting(row, col - 1) {
                    r.game
                        .geometry_mut()
                        .link_joints(x18, b[at(row, col - 1)].x18);
                }
                if row < 4 && resting(row + 1, col) {
                    r.game
                        .geometry_mut()
                        .link_joints(x18, b[at(row + 1, col)].x18);
                }
                if col < 5 && resting(row, col + 1) {
                    r.game
                        .geometry_mut()
                        .link_joints(x18, b[at(row, col + 1)].x18);
                }
                blocks(r.game, g)[at(row, col)].set_flag(5, false);
            } else if me.flag(6) {
                link_corners(r, gobj, col, row);
                blocks(r.game, g)[at(row, col)].set_flag(6, false);
            }
        }
    }
}

/// The blocks' collision joints' callback, `fn_80216DE4`: something touched a block's joint,
/// which notes it for the block's next tick.
#[doc(alias = "fn_80216DE4")]
pub fn block_landed(gp: &mut super::super::structs::Ground, joint_id: i32) {
    let blocks = &mut gp.u.greens().blocks;
    if let Some(b) = blocks
        .iter_mut()
        .find(|b| b.status() != 0 && b.x18 == joint_id)
    {
        b.set_flag(4, true);
    }
}

/// The stage's shadow check, `grGreens_80216E6C`: every joint receives shadows.
#[doc(alias = "grGreens_80216E6C")]
pub fn check_shadow_render() -> bool {
    true
}

/// A block's joint (for the checks).
pub fn block_joint(game: &Game, gobj: GObjId, num: usize) -> Option<JObjId> {
    nth_joint(game, gobj, BLOCK_JOINTS[num])
}
