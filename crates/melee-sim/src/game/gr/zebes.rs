//! Brinstar, `melee/gr/grzebes.c`: the stage (map 6) with its acid-shaking pillar and the
//! organic platform of bubbles on its right, the floating island (map 7) with its own shaking
//! part, the lava (map 8) that rises and falls through a table of levels, a background that
//! reacts to the lava (map 1), a creature in the background (map 4), the lava's splashes (map
//! 5), the bubbles' objects (map 3), and map 0.
//!
//! The lava is a fighter device (`ftCo_800C07F8`) that hurts whoever is below its surface
//! ([`lava_device`]). The organic platform is a chain of 20 bubbles (`grZe_8049F170`): seven
//! along its top between two anchors that follow the stage and the island, held together by
//! springs, the rest packed below; its collision joint's six vertices follow the bubbles' tops,
//! a hurtbox item (`it::kinds::yaku`) spans them, and a hit pops a bubble, after which the
//! platform comes apart and grows back.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{StageCallbacks, ZEBES_CALLBACKS};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, set_x724, x724_and_step};
use super::vars::AcidState;
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// The stage's map ids.
pub const MAP_BACK: i32 = 1;
pub const MAP_BUBBLE: i32 = 3;
pub const MAP_CREATURE: i32 = 4;
pub const MAP_SPLASH: i32 = 5;
pub const MAP_STAGE: i32 = 6;
pub const MAP_ISLAND: i32 = 7;
pub const MAP_LAVA: i32 = 8;

/// `grZebes_801DCBFC`, the lava as a fighter device (`ftCo_800C07F8`, type 3).
pub const LAVA_DEVICE: u32 = 0x801D_CBFC;
/// The pillar's and the island's hurtbox items' callbacks for a hit they take
/// (`fn_801DA9D8`, `fn_801DA9F0`), and the organic platform's (`fn_801DAC90`).
pub const PILLAR_HIT: u32 = 0x801D_A9D8;
pub const ISLAND_HIT: u32 = 0x801D_A9F0;
pub const BUBBLES_HIT: u32 = 0x801D_AC90;
/// The start hooks of the creature (`fn_801D94F0`) and the lava (`grZebes_801D9758`).
const CREATURE_START: u32 = 0x801D_94F0;
const LAVA_START: u32 = 0x801D_9758;
/// The spawn configurations' callbacks: the stage's and the island's lava bubbles
/// (`grZebes_801DCB64`, `grZebes_801DCBB0`).
pub const STAGE_BUBBLES: u32 = 0x801D_CB64;
pub const ISLAND_BUBBLES: u32 = 0x801D_CBB0;

/// Where the stage's objects' calls put their scale joints' descriptors on the stack: the
/// setup at `setup::SETUP_FRAME`'s depth (its frame is 8 bytes, as Yoshi's Story's); the island
/// from the stage's setup (0x60 below it: the stage's setup's frame and its create call's);
/// the platform's first bubbles through `grZebes_801DC9DC`, `grZebes_801DC744` and
/// `grZebes_801DAE70` (0x1C0 below); bubbles growing back from the stage's process through
/// `grZebes_801DC408` (0x1D0 below the process loop's, `corneria::PROC_LOOP`) or directly (0x160
/// below); a splash from the lava's device.
pub const ISLAND_FRAME: u32 = setup::SETUP_FRAME - 0x60;
pub const SETUP_BUBBLE_FRAME: u32 = setup::SETUP_FRAME - 0x1C0;
pub const GROW_BUBBLE_FRAME: u32 = super::corneria::PROC_LOOP - 0x1D0;
pub const REGROW_BUBBLE_FRAME: u32 = super::corneria::PROC_LOOP - 0x160;
pub const SPLASH_FRAME: u32 = 0x804E_E7C0;

/// The fields of `grZe_YakumonoParam`, by offset.
const PLATFORM_WAIT_MIN: u32 = 0;
const PLATFORM_WAIT_MAX: u32 = 4;
const POP_WAIT_MIN: u32 = 8;
const POP_WAIT_MAX: u32 = 0xC;
const SPAWN_EVERY: u32 = 0x10;
/// The lava's hit, which the device hands the fighter code (`((HSD_GObj*)yakumono_param)->
/// user_data`, the word at 0x2C).
const LAVA_HIT: u32 = 0x2C;
const SHAKE_MAX: u32 = 0x30;
const SHAKE_SPEED_MAX: u32 = 0x34;
const SHAKE_PULL: u32 = 0x38;
const SHAKE_DAMP: u32 = 0x3C;
const SHAKE_REST_SPEED: u32 = 0x40;
const SHAKE_REST: u32 = 0x44;
const DAMAGE_DECAY: u32 = 0x48;
const DAMAGE_MAX: u32 = 0x4C;
const BROKEN_MIN: u32 = 0x50;
const BROKEN_MAX: u32 = 0x54;
const SPAWN_SIZE_MIN: u32 = 0x58;
const SPAWN_SIZE_MAX: u32 = 0x5C;
const GROW_WAIT_MIN: u32 = 0x60;
const GROW_WAIT_MAX: u32 = 0x64;
const GROW_SPEED: u32 = 0x68;
const BUBBLE_FAR: u32 = 0x6C;
const BUBBLE_NEAR: u32 = 0x70;
const BUBBLE_SPACING: u32 = 0x74;
const BUBBLE_ATTRACT: u32 = 0x78;
const BUBBLE_PULL_MAX: u32 = 0x80;
const BUBBLE_SINK: u32 = 0x84;
const BUBBLE_SPEED_MAX: u32 = 0x88;
const LAVA_TOP: u32 = 0x8C;
const LAVA_BOTTOM: u32 = 0x90;
const LAVA_SPREAD: u32 = 0x94;
const LAVA_SPEED: u32 = 0x98;
const LAVA_ACCEL: u32 = 0x9C;
/// The lava's table of levels (`grZe_AcidLevelEntry`, 30 of 8 bytes): a wait, its random
/// extra's two bounds, and a level.
const LEVELS: u32 = 0xA0;

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
    /// The level table's entry `i`: base wait, delay bounds and level.
    fn level(self, data: &Archives, i: i32) -> [i16; 4] {
        let at = LEVELS + 8 * i as u32;
        std::array::from_fn(|k| data.u16(self.0.at(at + 2 * k as u32)) as i16)
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.zebes_yakumono.expect("grzebes.c: yakumono_param"))
}

/// One of the organic platform's bubbles (`grZe_BubbleEntry`): whether it's there and how
/// (`x00`: 0 none, 1 in place, 2 popped, 3 falling apart, 4 new and growing), a countdown
/// (`x02`), the pop's model (`x04`), its place and speed (`x08` to `x14`), its size and the
/// size it grows to (`x18`, `x1C`) and its object (`x20`, map 3; none for the two anchors).
/// A bubble that moved up to the top leaves its pop's pointer behind (`x04_left`), dangling
/// once the pop goes, which nothing reads before the place is added again: not compared while
/// left, and a read of it counts as a stale read.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
#[allow(non_snake_case)]
pub struct Bubble {
    pub x00: u8,
    pub x02: i16,
    pub x04: Option<JObjId>,
    pub x08: f32,
    pub x0C: f32,
    pub x10: f32,
    pub x14: f32,
    pub x18: f32,
    pub x1C: f32,
    pub x20: Option<GObjId>,
    pub x04_left: bool,
}

impl Bubble {
    /// The pop's model (`x04`), a stale read if a bubble that moved up left it.
    fn pop(&self, stale_reads: &mut u32) -> Option<JObjId> {
        if self.x04_left {
            *stale_reads += 1;
        }
        self.x04
    }
}

/// grzebes.c's statics.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The organic platform's bounds (`grZe_8049F140`): its top's left and right ends.
    pub bounds: [Vec3; 2],
    /// Its anchors (`grZe_8049F158`), bubbles 0 and 6.
    pub anchors: [Vec3; 2],
    /// The bubbles (`grZe_8049F170`).
    pub bubbles: [Bubble; 20],
    /// The wait after a pop before bubbles grow back (`grZe_804D6994`).
    pub pop_wait: i32,
    /// The lava's splashes out (`grZe_804D6998`), at most 10.
    pub splashes: i16,
}

impl Globals {
    /// The float at `off` past `grZe_8049F140` (the bounds, the anchors and the bubbles laid
    /// out one after another), as the platform's spawns read their places
    /// (`grZe_BubbleSpawnPos` over the same memory).
    fn float_at(&self, off: u32) -> f32 {
        match off {
            0x00..0x18 => {
                let v = self.bounds[off as usize / 0xC];
                [v.x, v.y, v.z][off as usize % 0xC / 4]
            }
            0x18..0x30 => {
                let v = self.anchors[(off as usize - 0x18) / 0xC];
                [v.x, v.y, v.z][(off as usize - 0x18) % 0xC / 4]
            }
            _ => {
                let b = &self.bubbles[(off as usize - 0x30) / 0x24];
                match (off - 0x30) % 0x24 {
                    0x08 => b.x08,
                    0x0C => b.x0C,
                    other => panic!("grzebes.c: a bubble's word at {other:#x} read as a place"),
                }
            }
        }
    }
}

/// `grZe_803B8044`'s sizes of the top's seven bubbles and the bounds and anchors the platform
/// starts from.
const TOP_SIZES: [f32; 7] = [1.0, 1.1, 1.0, 1.2, 1.1, 1.0, 1.0];
const START_BOUNDS: [Vec3; 2] = [Vec3::new(7.59, 2.5, 0.0), Vec3::new(24.05, 2.2, 0.0)];
const START_ANCHORS: [Vec3; 2] = [Vec3::new(8.2, -4.55, 0.0), Vec3::new(24.1, -4.6, 0.0)];

/// `grZe_803B7FF0`: the organic platform's item's place, the left anchor and bound in the
/// stage's joint 14, and its collision's ends in that joint and in the island's joint 1.
const BUBBLES_ITEM_POS: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const LEFT_ANCHOR: Vec3 = Vec3::new(8.2, -4.55, 0.0);
const LEFT_BOUND: Vec3 = Vec3::new(7.59, 2.5, 0.0);
const UPPER_END: Vec3 = Vec3::new(8.589, 1.215, 0.0);
const LOWER_END: Vec3 = Vec3::new(23.151, 1.207, 0.0);
/// The right anchor and bound in the island's joint 1.
const RIGHT_ANCHOR: Vec3 = Vec3::new(24.1, -4.6, 0.0);
const RIGHT_BOUND: Vec3 = Vec3::new(24.05, 2.2, 0.0);

/// The spawn configurations' lines, pairs of ends (`grZe_803E1B90`, the stage's, and
/// `grZe_803E1C80`, the island's).
pub const STAGE_LINES: u32 = 0x803E_1B90;
pub const ISLAND_LINES: u32 = 0x803E_1C80;
const STAGE_LINE_ENDS: [Vec3; 20] = [
    Vec3::new(-40.5, -10.0, 13.0),
    Vec3::new(-40.5, -24.0, 13.0),
    Vec3::new(-5.5, -13.0, 23.5),
    Vec3::new(-5.5, -27.0, 23.5),
    Vec3::new(-5.5, 0.5, 0.0),
    Vec3::new(-5.5, -10.0, 0.0),
    Vec3::new(-8.0, 4.5, 32.0),
    Vec3::new(-8.0, -14.5, 32.0),
    Vec3::new(-34.0, 0.5, 24.0),
    Vec3::new(-34.0, -17.5, 24.0),
    Vec3::new(-23.5, -12.0, 5.5),
    Vec3::new(-23.5, -23.0, 5.5),
    Vec3::new(6.5, 4.0, 18.0),
    Vec3::new(6.5, -10.5, 18.0),
    Vec3::new(10.0, 0.5, 9.5),
    Vec3::new(10.0, -10.0, 9.5),
    Vec3::new(-21.5, 2.0, 15.5),
    Vec3::new(-21.5, -16.0, 15.5),
    Vec3::new(-59.0, 27.5, 6.0),
    Vec3::new(-59.0, 17.0, 6.0),
];
const ISLAND_LINE_ENDS: [Vec3; 8] = [
    Vec3::new(42.5, -11.0, 0.0),
    Vec3::new(42.5, -25.0, 0.0),
    Vec3::new(30.0, 1.0, 11.0),
    Vec3::new(30.0, -17.0, 11.0),
    Vec3::new(45.0, -5.0, 7.5),
    Vec3::new(45.0, -14.0, 7.5),
    Vec3::new(56.0, 15.0, 6.0),
    Vec3::new(56.0, 4.5, 6.0),
];

/// A spawn configuration's lines by their table's address.
pub fn lines(addr: u32) -> &'static [Vec3] {
    match addr {
        STAGE_LINES => &STAGE_LINE_ENDS,
        ISLAND_LINES => &ISLAND_LINE_ENDS,
        other => panic!("a spawn configuration's lines at {other:#010x}"),
    }
}

/// The lava's colors for the fog by level (`table`): the levels' thresholds and the colors
/// between them.
const FOG_TABLE: [[f32; 4]; 3] = [
    [-320.0, 0.0, 0.0, 0.0],
    [-80.0, 1.0, 20.0, 30.0],
    [90.0, 90.0, 75.0, 0.0],
];

/// A stage object's process or callback, by address: Brinstar's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801D_90FC | 0x801D_93D8 | 0x801D_94EC | 0x801D_9754 | 0x801D_9F2C | 0x801D_A0C0 => {
            empty(r, gobj)
        }
        0x801D_8644 => init_stage(r, gobj),
        0x801D_881C => proc_stage(r, gobj),
        0x801D_9100 => init_island(r, gobj),
        0x801D_925C => proc_island(r, gobj),
        0x801D_93DC => init_splash(r, gobj),
        0x801D_9410 => proc_splash(r, gobj),
        CREATURE_START => start_creature(r, gobj),
        0x801D_9508 => init_creature(r, gobj),
        0x801D_95B8 => proc_creature(r, gobj),
        LAVA_START => start_lava(r, gobj),
        0x801D_9798 => init_lava(r, gobj),
        0x801D_99E0 => proc_lava(r, gobj),
        0x801D_9F30 => init_back(r, gobj),
        0x801D_9F84 => proc_back(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801D_84A4 => on_init(r),
        0x801D_8528 => on_load(r),
        0x801D_852C => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grZebes_801D90FC")]
#[doc(alias = "grZebes_801D93D8")]
#[doc(alias = "grZebes_801D94EC")]
#[doc(alias = "grZebes_801D9754")]
#[doc(alias = "grZebes_801D9F2C")]
#[doc(alias = "grZebes_801DA0C0")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grZebes_801D8558`.
#[doc(alias = "grZebes_801D8558")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grZe_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &ZEBES_CALLBACKS[map_id as usize]
}

/// Brinstar's setup, `grZebes_801D84A4`: its parameters, maps 0, 1, 6 (which makes 7), 4 and
/// 8, camera limits, blast zones, and no splashes.
#[doc(alias = "grZebes_801D84A4")]
pub fn on_init(r: &mut Running) {
    r.game.gr.zebes_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, MAP_BACK, MAP_STAGE, MAP_CREATURE, MAP_LAVA] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    reset_splashes(r.game);
}

/// `grZebes_801D84A0` does nothing (only a demo runs it).
#[doc(alias = "grZebes_801D84A0")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grZebes_801D8528` does nothing.
#[doc(alias = "grZebes_801D8528")]
pub fn on_load(_r: &mut Running) {}

/// Brinstar's start, `grZebes_801D852C`: the enemy spawner, without enemies.
#[doc(alias = "grZebes_801D852C")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

fn translate_x(game: &Game, j: Option<JObjId>) -> f32 {
    game.objs.jobjs[j.expect("grzebes.c: jobj")].translate.x
}

/// The stage's setup (map 6), `grZebes_801D8644`: the island, its collision, its animation, the
/// pillar's hurtbox item (on joint 30) and its shaking at rest, the organic platform's item and
/// its bubbles, the lava bubbles' spawn configuration, and the collision's first joint kept in
/// reach.
#[doc(alias = "grZebes_801D8644")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let island = create(r, MAP_ISLAND, ISLAND_FRAME);
    r.game.grounds[g.0].u.zebes_stage().xF0 = island;
    let j = jobj_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let child = nth_joint(r.game, gobj, 0x1E);
    let item = material::stage_item(r, 7, 0, g, child, None, Some(CodeRef(PILLAR_HIT)), None)
        .expect("grzebes.c: the pillar's item");
    let (a, b) = (Vec3::new(0.0, -7.0, 0.0), Vec3::new(0.0, 7.0, 0.0));
    material::set_item_hurtbox(r.game, item, a, b, 3.0);
    material::item_hurtbox_on(r.game, item);
    let base_x = translate_x(r.game, child);
    let jobj2 = nth_joint(r.game, gobj, 0x20);
    let v = r.game.grounds[g.0].u.zebes_stage();
    v.xC4 = 0;
    v.acid = AcidState {
        x00: 0xFF,
        x01: 0,
        x02: 0,
        x04: base_x,
        x08: 0.0,
        x0C: 0.0,
        x10: 0.0,
        x14: child,
        x18: jobj2,
        x1C: Some(item),
        x20: 0x1C,
    };
    v.xEC = 0;
    v.xF4 = 0;
    let bubbles = material::stage_item_at(
        r,
        0,
        0,
        g,
        BUBBLES_ITEM_POS,
        false,
        None,
        Some(CodeRef(BUBBLES_HIT)),
        None,
    )
    .expect("grzebes.c: the bubbles' item");
    material::item_hurtbox_on(r.game, bubbles);
    r.game.grounds[g.0].u.zebes_stage().x100 = Some(bubbles);
    reset_bubbles(r, gobj);
    let config = zako::new_config(r.game, r.data, STAGE_LINES, 10, CodeRef(STAGE_BUBBLES), 1.0);
    r.game.grounds[g.0].u.zebes_stage().xFC = config;
    r.game.geometry_mut().set_always_near(0);
    super::ground::update_map_coll(r.game, r.data, gobj);
}

/// Places a stage object's model joint's point `local` in the world.
fn world(r: &mut Running, j: Option<JObjId>, local: Vec3) -> Vec3 {
    crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(local))
}

/// The stage's process (map 6), `grZebes_801D881C`. The pillar shakes (its animation when it
/// breaks and comes back); now and then a bubble's pop shows; the bubbles move, take their
/// places, span the item and grow back. The organic platform's state (`xC4`): whole (0) until a
/// bubble falls apart, then breaking (1), broken for a random while (2), and growing back (3)
/// as new bubbles appear in pairs from the ends. While whole, its collision's six vertices sit
/// on the bubbles' tops between its two ends, each at most a column's width below its
/// neighbours; otherwise its collision is out. Then the lava bubbles, the collision and the
/// wind sources.
#[doc(alias = "grZebes_801D881C")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let island = r.game.grounds[g.0].u.zebes_stage().xF0;
    let result = shake(r, gobj, g, Acid::Stage, 1, 2);
    if r.game.grounds[g.0].u.zebes_stage().xEC as i32 != result {
        r.game.grounds[g.0].u.zebes_stage().xEC = result as u32;
        if result == 1 {
            anime::start(r.game, r.data, gobj, 0x15, 1, 1, 30.0, 1.0, true);
            anime::clear_loop(r.game, gobj, 0x15, 1, true);
        } else if result == 3 {
            anime::start(r.game, r.data, gobj, 0x15, 1, 2, 0.0, 1.0, true);
            anime::clear_loop(r.game, gobj, 0x15, 1, true);
        }
    }
    let t = r.game.grounds[g.0].u.zebes_stage().xF8;
    r.game.grounds[g.0].u.zebes_stage().xF8 = t.wrapping_sub(1);
    if t < 0 {
        show_pop(r);
        let p = param(r.game);
        let rand = reg(r.game.objs.rng.draw_f32());
        let (lo, hi) = (
            reg(p.f(r.data, POP_WAIT_MIN)),
            reg(p.f(r.data, POP_WAIT_MAX)),
        );
        let wait = fp::fctiwz(fp::fmadds(fp::fsubs(hi, lo), rand, lo));
        r.game.grounds[g.0].u.zebes_stage().xF8 = wait as i16;
    }
    let popped = move_bubbles(r, gobj);
    place_bubbles(r);
    let item = r.game.grounds[g.0].u.zebes_stage().x100;
    span_bubbles(r, item.expect("grzebes.c: 2200: yaku"));
    grow_bubbles(r, gobj);
    let island = island.expect("grzebes.c: the island");
    match r.game.grounds[g.0].u.zebes_stage().xC4 {
        0 => {
            if popped {
                r.game.grounds[g.0].u.zebes_stage().xC4 = 1;
                anime::start(r.game, r.data, gobj, 0xE, 1, 3, 0.0, 1.0, false);
                anime::clear_loop(r.game, gobj, 0xE, 1, false);
                anime::start(r.game, r.data, island, 1, 1, 3, 0.0, 1.0, false);
                anime::clear_loop(r.game, island, 1, 1, false);
            }
        }
        1 => {
            if anime::finished(r.game, gobj, 0xE, 1) {
                r.game.grounds[g.0].u.zebes_stage().xC4 = 2;
                let p = param(r.game);
                let rand = reg(r.game.objs.rng.draw_f32());
                let (lo, hi) = (
                    reg(p.f(r.data, PLATFORM_WAIT_MIN)),
                    reg(p.f(r.data, PLATFORM_WAIT_MAX)),
                );
                let wait = fp::fctiwz(fp::fmadds(fp::fsubs(hi, lo), rand, lo));
                r.game.grounds[g.0].u.zebes_stage().xC6 = wait as i16;
            }
        }
        2 => {
            let v = r.game.grounds[g.0].u.zebes_stage();
            v.xC6 = v.xC6.wrapping_sub(1);
            if v.xC6 < 0 {
                v.xC4 = 3;
                anime::start(r.game, r.data, gobj, 0xE, 1, 4, 0.0, 1.0, false);
                anime::clear_loop(r.game, gobj, 0xE, 1, false);
                anime::start(r.game, r.data, island, 1, 1, 4, 0.0, 1.0, false);
                anime::clear_loop(r.game, island, 1, 1, false);
                r.game.grounds[g.0].u.zebes_stage().xF6 = 0;
                r.game.gr.zebes.pop_wait = 0;
            }
        }
        3 => {
            let v = r.game.grounds[g.0].u.zebes_stage();
            v.xF6 = v.xF6.wrapping_add(1);
            let n = i32::from(v.xF6);
            let every = param(r.game).i(r.data, SPAWN_EVERY);
            if n.wrapping_rem(every) == 0 {
                let phase = n.wrapping_div(every);
                let mirror = 6 - phase;
                if phase < mirror {
                    let p = param(r.game);
                    let rand = reg(r.game.objs.rng.draw_f32());
                    let lo = reg(p.f(r.data, SPAWN_SIZE_MIN));
                    let hi = reg(p.f(r.data, SPAWN_SIZE_MAX));
                    let z = &r.game.gr.zebes;
                    let x = z.float_at(0x24 * phase as u32 + 0x14);
                    let y = z.float_at(0x24 * phase as u32 + 0x18);
                    let size = mem(fp::fmadds(fp::fsubs(hi, lo), rand, lo));
                    add_bubble(r, phase, 4, x, y, size, REGROW_BUBBLE_FRAME);
                }
                if phase <= mirror {
                    let rand = reg(r.game.objs.rng.draw_f32());
                    let z = &r.game.gr.zebes;
                    let at = 0x24 * (mirror + 2) as u32;
                    let (x, y) = (z.float_at(at + 0x14), z.float_at(at + 0x18));
                    let size = mem(fp::frsp(fp::fmadd(0.5, rand, 1.0)));
                    add_bubble(r, mirror, 4, x, y, size, REGROW_BUBBLE_FRAME);
                }
            }
            if anime::finished(r.game, gobj, 0xE, 1) {
                r.game.grounds[g.0].u.zebes_stage().xC4 = 0;
                for b in &mut r.game.gr.zebes.bubbles {
                    if b.x00 == 4 {
                        b.x00 = 1;
                    }
                }
            }
        }
        _ => {}
    }
    let j = nth_joint(r.game, gobj, 0xE);
    if j.is_some() {
        r.game.gr.zebes.anchors[0] = world(r, j, LEFT_ANCHOR);
        r.game.gr.zebes.bounds[0] = world(r, j, LEFT_BOUND);
    }
    if r.game.grounds[g.0].u.zebes_stage().xC4 == 0 {
        platform_collision(r, gobj, island);
    } else {
        r.game.geometry_mut().disable_joint(0);
    }
    let (level, _) = x724_and_step(r.game);
    let config = r.game.grounds[g.0].u.zebes_stage().xFC;
    let j = nth_joint(r.game, gobj, 0xE);
    zako::run_config(r, config, j, level);
    super::ground::update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The organic platform's collision while it's whole (`grZebes_801D881C`'s second half): its
/// joint back in the search list and settled, its six vertices on the bubbles' tops spread
/// over five columns between its two ends, each at most a column's width below its
/// neighbours, and its lines updated.
fn platform_collision(r: &mut Running, gobj: GObjId, island: GObjId) {
    r.game.geometry_mut().enable_joint(0);
    r.game.geometry_mut().settle_vertices(0);
    let z = &r.game.gr.zebes;
    let left_x = reg(z.bounds[0].x);
    let width = fp::fdivs(fp::fsubs(reg(z.bounds[1].x), left_x), 5.0);
    let mut heights = [-9999.0f32; 6];
    // The columns' places no bubble's edge reaches keep what the original's stack held, which
    // plain data can't give: such a place counts as a stale read.
    let mut xs = [0.0f32; 6];
    let mut set = [false; 6];
    for b in z.bubbles.iter().filter(|b| b.x00 == 1) {
        let off = fp::fsubs(reg(b.x08), left_x);
        let top = mem(fp::frsp(fp::fmadd(1.8, reg(b.x18), reg(b.x0C))));
        let left = fp::frsp(fp::fsub(off, 0.9));
        let right = fp::frsp(fp::fadd(0.9, off));
        for edge in [left, right] {
            let k = fp::fctiwz(fp::fadd(0.5, fp::fdivs(edge, width))).clamp(0, 5);
            xs[k as usize] = mem(fp::fmadds(f64::from(k), width, left_x));
            set[k as usize] = true;
            if reg(top) > reg(heights[k as usize]) {
                heights[k as usize] = top;
            }
        }
    }
    let j = nth_joint(r.game, gobj, 0xE);
    assert!(j.is_some(), "grzebes.c: 659: sima_jobj");
    let upper = world(r, j, UPPER_END);
    xs[0] = upper.x;
    heights[0] = upper.y;
    let j = nth_joint(r.game, island, 1);
    assert!(j.is_some(), "grzebes.c: 665: sima_jobj");
    let lower = world(r, j, LOWER_END);
    xs[5] = lower.x;
    heights[5] = lower.y;
    if set[1..5].contains(&false) {
        r.game.objs.stale_reads += 1;
    }
    for i in 0..5 {
        if i != 0 {
            let below = mem(fp::fsubs(reg(heights[i + 1]), width));
            if reg(heights[i]) < reg(below) {
                heights[i] = below;
            }
        }
        let below = mem(fp::fsubs(reg(heights[i]), width));
        if reg(heights[i + 1]) < reg(below) {
            heights[i + 1] = below;
        }
    }
    let geo = r.game.geometry_mut();
    for i in 0..6 {
        geo.set_vertex_pos(i as i32, xs[i], heights[i]);
    }
    geo.update_dynamics_and_islands(0);
}

/// Which shaking part a [`shake`] moves: the stage's pillar (map 6's) or the island's.
#[derive(Clone, Copy)]
pub enum Acid {
    Stage,
    Island,
}

fn acid(game: &mut Game, g: GroundId, which: Acid) -> &mut AcidState {
    match which {
        Acid::Stage => &mut game.grounds[g.0].u.zebes_stage().acid,
        Acid::Island => &mut game.grounds[g.0].u.zebes_island().acid,
    }
}

/// A shaking part's tick, `grZebes_801DA528`, its state's start when it changes (`x00` takes
/// `x01`): at rest (0, its animation `anim` from its start, its item's hurtbox on), breaking
/// (1, its item's hurtbox off), broken for a random while (2) and coming back (3, its animation
/// `back`). At rest, its damage decays every so many ticks; a hit's push (`x0C`) swings it
/// within its bounds, pulled back and damped, until it's still near its middle; its two joints
/// follow; and past its damage's limit it breaks with a sound and a rumble, otherwise its
/// animation shows the damage. Its state.
#[doc(alias = "grZebes_801DA528")]
pub fn shake(r: &mut Running, gobj: GObjId, g: GroundId, which: Acid, anim: i32, back: i32) -> i32 {
    let p = param(r.game);
    let d = r.data;
    let st = acid(r.game, g, which).clone();
    if st.x00 != st.x01 {
        acid(r.game, g, which).x00 = st.x01;
        let part = i32::from(st.x20);
        match st.x01 {
            0 => {
                anime::start(r.game, r.data, gobj, part, 1, anim, 0.0, 1.0, true);
                material::item_hurtbox_on(r.game, st.x1C.expect("a shaking part's item"));
                let a = acid(r.game, g, which);
                a.x02 = 0;
                a.x08 = 0.0;
                a.x0C = 0.0;
            }
            1 => {
                anime::clear_loop(r.game, gobj, part, 1, true);
                material::item_hurtbox_off(r.game, st.x1C.expect("a shaking part's item"));
            }
            2 => {
                let rand = reg(r.game.objs.rng.draw_f32());
                let (lo, hi) = (reg(p.f(d, BROKEN_MIN)), reg(p.f(d, BROKEN_MAX)));
                let t = fp::fctiwz(fp::fmadds(fp::fsubs(hi, lo), rand, lo));
                acid(r.game, g, which).x02 = t as i16;
            }
            3 => {
                anime::start(r.game, r.data, gobj, part, 1, back, 0.0, 1.0, true);
                anime::clear_loop(r.game, gobj, part, 1, true);
                acid(r.game, g, which).x10 = 0.0;
            }
            _ => {}
        }
    }
    let st = acid(r.game, g, which).clone();
    let part = i32::from(st.x20);
    match st.x00 {
        0 => {
            let a = acid(r.game, g, which);
            let t = a.x02;
            a.x02 = t.wrapping_add(1);
            if fp::frsp(f64::from(t)) > reg(p.f(d, DAMAGE_DECAY)) {
                a.x02 = 0;
                a.x10 = mem(fp::fsubs(reg(a.x10), 1.0));
                if reg(a.x10) < 0.0 {
                    a.x10 = 0.0;
                }
            }
            let rest = p.f(d, SHAKE_REST);
            if reg(a.x08) > reg(rest) {
                a.x0C = mem(fp::fsubs(reg(a.x0C), reg(p.f(d, SHAKE_PULL))));
            } else if reg(a.x08) < fp::fneg(reg(rest)) {
                a.x0C = mem(fp::fadds(reg(a.x0C), reg(p.f(d, SHAKE_PULL))));
            }
            a.x0C = mem(fp::fmuls(
                reg(a.x0C),
                fp::fsubs(1.0, reg(p.f(d, SHAKE_DAMP))),
            ));
            let vmax = p.f(d, SHAKE_SPEED_MAX);
            if reg(a.x0C) > reg(vmax) {
                a.x0C = vmax;
            } else if reg(a.x0C) < fp::fneg(reg(vmax)) {
                a.x0C = mem(fp::fneg(reg(vmax)));
            }
            a.x08 = mem(fp::fadds(reg(a.x08), reg(a.x0C)));
            let max = p.f(d, SHAKE_MAX);
            if reg(a.x08) > reg(max) {
                a.x08 = max;
            } else if reg(a.x08) < fp::fneg(reg(max)) {
                a.x08 = mem(fp::fneg(reg(max)));
            }
            let mut off = reg(a.x08);
            if off < 0.0 {
                off = fp::fneg(off);
            }
            if off < reg(rest) {
                let mut v = reg(a.x0C);
                if v < 0.0 {
                    v = fp::fneg(v);
                }
                if v < reg(p.f(d, SHAKE_REST_SPEED)) {
                    a.x08 = 0.0;
                    a.x0C = 0.0;
                }
            }
            let x = mem(fp::fadds(reg(a.x04), reg(a.x08)));
            let (j1, j2) = (a.x14, a.x18);
            for j in [j1, j2] {
                jobj::set_srt(
                    &mut r.game.objs.jobjs,
                    j.expect("a shaking part's joint"),
                    |o| o.translate.x = x,
                );
            }
            let dmg = reg(acid(r.game, g, which).x10);
            let max = reg(p.f(d, DAMAGE_MAX));
            let frame = if dmg > max {
                acid(r.game, g, which).x01 = 1;
                r.stage_sound(0x61A86);
                crate::game::ft::ftlib::start_rumble_all(r, 1, 0x5A);
                30.0
            } else {
                mem(fp::frsp(fp::fdiv(fp::fmuls(dmg, max), 30.0)))
            };
            anime::req_anim(r.game, gobj, part, 1, frame, true);
        }
        1 => {
            if anime::finished(r.game, gobj, part, 1) {
                acid(r.game, g, which).x01 = 2;
            }
        }
        2 => {
            let a = acid(r.game, g, which);
            a.x02 = a.x02.wrapping_sub(1);
            if a.x02 < 0 {
                a.x01 = 3;
            }
        }
        3 if anime::finished(r.game, gobj, part, 1) => acid(r.game, g, which).x01 = 0,
        _ => {}
    }
    i32::from(acid(r.game, g, which).x00)
}

/// A hit the pillar's item took, `fn_801DA9D8`: its damage counted and its push taken from the
/// hit's direction.
#[doc(alias = "fn_801DA9D8")]
pub fn pillar_hit(r: &mut Running, g: GroundId, dir: Vec3, damage: f32) {
    let a = &mut r.game.grounds[g.0].u.zebes_stage().acid;
    a.x10 = mem(fp::fadds(reg(a.x10), reg(damage)));
    a.x0C = dir.x;
}

/// A hit the island's item took, `fn_801DA9F0`: likewise.
#[doc(alias = "fn_801DA9F0")]
pub fn island_hit(r: &mut Running, g: GroundId, dir: Vec3, damage: f32) {
    let a = &mut r.game.grounds[g.0].u.zebes_island().acid;
    a.x10 = mem(fp::fadds(reg(a.x10), reg(damage)));
    a.x0C = dir.x;
}

/// The island's setup (map 7), `grZebes_801D9100`: its animation, its collision, its hurtbox
/// item (on joint 15) and its shaking at rest (joints 15 and 17, animation part 13), and its
/// lava bubbles' spawn configuration.
#[doc(alias = "grZebes_801D9100")]
pub fn init_island(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let child = nth_joint(r.game, gobj, 0xF);
    let item = material::stage_item(r, 7, 0, g, child, None, Some(CodeRef(ISLAND_HIT)), None)
        .expect("grzebes.c: the island's item");
    let (a, b) = (Vec3::new(0.0, -5.0, 0.0), Vec3::new(0.0, 5.0, 0.0));
    material::set_item_hurtbox(r.game, item, a, b, 2.0);
    material::item_hurtbox_on(r.game, item);
    let base_x = translate_x(r.game, child);
    let jobj2 = nth_joint(r.game, gobj, 0x11);
    let v = r.game.grounds[g.0].u.zebes_island();
    v.acid = AcidState {
        x00: 0xFF,
        x01: 0,
        x02: 0,
        x04: base_x,
        x08: 0.0,
        x0C: 0.0,
        x10: 0.0,
        x14: child,
        x18: jobj2,
        x1C: Some(item),
        x20: 0xD,
    };
    v.xE8 = 0;
    let config = zako::new_config(
        r.game,
        r.data,
        ISLAND_LINES,
        4,
        CodeRef(ISLAND_BUBBLES),
        1.0,
    );
    r.game.grounds[g.0].u.zebes_island().xEC = config;
    super::ground::update_map_coll(r.game, r.data, gobj);
}

/// The island's process (map 7), `grZebes_801D925C`: its shaking part (its animation when it
/// breaks and comes back), the organic platform's right anchor and bound from its joint 1, its
/// lava bubbles and its collision.
#[doc(alias = "grZebes_801D925C")]
pub fn proc_island(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let result = shake(r, gobj, g, Acid::Island, 1, 2);
    if r.game.grounds[g.0].u.zebes_island().xE8 != result {
        r.game.grounds[g.0].u.zebes_island().xE8 = result;
        if result == 1 {
            anime::start(r.game, r.data, gobj, 6, 1, 1, 30.0, 1.0, true);
            anime::clear_loop(r.game, gobj, 6, 1, true);
        } else if result == 3 {
            anime::start(r.game, r.data, gobj, 6, 1, 2, 0.0, 1.0, true);
            anime::clear_loop(r.game, gobj, 6, 1, true);
        }
    }
    let j = nth_joint(r.game, gobj, 1);
    if j.is_some() {
        r.game.gr.zebes.anchors[1] = world(r, j, RIGHT_ANCHOR);
        r.game.gr.zebes.bounds[1] = world(r, j, RIGHT_BOUND);
    }
    let (level, _) = x724_and_step(r.game);
    let config = r.game.grounds[g.0].u.zebes_island().xEC;
    let j = nth_joint(r.game, gobj, 1);
    zako::run_config(r, config, j, level);
    super::ground::update_map_coll(r.game, r.data, gobj);
}

/// A splash's setup (map 5), `stageGObj5_OnInit`: its animation.
#[doc(alias = "stageGObj5_OnInit@801D93DC")]
pub fn init_splash(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A splash's process, `grZebes_801D9410`: on the lava's surface, gone when its animation
/// ends.
#[doc(alias = "grZebes_801D9410")]
pub fn proc_splash(r: &mut Running, gobj: GObjId) {
    let (level, _) = x724_and_step(r.game);
    let j = jobj_of(r.game, gobj).expect("a splash's model");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = level);
    if anime::finished(r.game, gobj, 0, 7) {
        remove_splash(r, gobj);
    }
}

/// The creature's start hook, `fn_801D94F0`: it starts moving.
#[doc(alias = "fn_801D94F0")]
pub fn start_creature(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.zebes_creature().set_waiting(false);
}

/// The creature's setup (map 4), `grZebes_801D9508`: it follows the stage's joint 14, first
/// waits 3,000 to 3,600 ticks, and holds still until the match starts.
#[doc(alias = "grZebes_801D9508")]
pub fn init_creature(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let stage = map_gobj(r.game, MAP_STAGE);
    assert!(stage.is_some(), "grzebes.c: 909: map_a_gobj");
    let j = nth_joint(r.game, stage.expect("map 6"), 14);
    let t = r.game.objs.rng.below(600).wrapping_add(3000);
    let v = r.game.grounds[g.0].u.zebes_creature();
    v.stored_jobj = j;
    v.x8 = 1;
    v.xA = t as i16;
    v.set_waiting(true);
    setup::add_start_hook(r.game, gobj, CodeRef(CREATURE_START));
}

/// The creature's process, `grZebes_801D95B8`: it follows the stage's joint; once the match
/// runs, when its wait is over it plays its two animations in turn, then waits again once one
/// ends.
#[doc(alias = "grZebes_801D95B8")]
pub fn proc_creature(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("the creature's model");
    if let Some(s) = r.game.grounds[g.0].u.zebes_creature().stored_jobj {
        let pos = r.game.objs.jobjs[s].translate;
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    }
    if r.game.grounds[g.0].u.zebes_creature().waiting() {
        return;
    }
    let m = map_id(r.game, gobj);
    let v = r.game.grounds[g.0].u.zebes_creature();
    if v.xA > 0 {
        v.xA = v.xA.wrapping_sub(1);
        if v.xA == 0 {
            if v.x8 > 0 {
                anime::start_map(r.game, r.data, gobj, m, 0);
                r.game.grounds[g.0].u.zebes_creature().x8 = -1;
                return;
            }
            anime::start_map(r.game, r.data, gobj, m, 1);
            r.game.grounds[g.0].u.zebes_creature().x8 = 1;
        }
    } else if anime::finished(r.game, gobj, 0, 7) {
        let t = r.game.objs.rng.below(600).wrapping_add(3000);
        r.game.grounds[g.0].u.zebes_creature().xA = t as i16;
    }
}

/// The lava's start hook, `grZebes_801D9758`: the lava becomes a fighter device and starts
/// moving.
#[doc(alias = "grZebes_801D9758")]
pub fn start_lava(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game
        .gr
        .devices
        .register_coll(gobj, 3, CodeRef(LAVA_DEVICE));
    r.game.grounds[g.0].u.zebes_lava().xC8 = 1;
}

/// A level table entry's wait: its base plus a random part of its extra.
fn level_wait(r: &mut Running, entry: [i16; 4]) -> i16 {
    let (min, mut max) = (i32::from(entry[1]), i32::from(entry[2]));
    if max > min {
        let diff = max.wrapping_sub(min);
        max = min.wrapping_add(if diff != 0 {
            r.game.objs.rng.below(diff)
        } else {
            0
        });
    } else if max < min {
        let diff = min.wrapping_sub(max);
        max = max.wrapping_add(if diff != 0 {
            r.game.objs.rng.below(diff)
        } else {
            0
        });
    }
    i32::from(entry[0]).wrapping_add(max) as i16
}

fn entry_empty(e: [i16; 4]) -> bool {
    e == [0; 4]
}

/// The lava's setup (map 8), `grZebes_801D9798`: its animation, the first entry's wait, still,
/// in the stage's layer; it starts at the table's last level plus a random spread, its joint
/// and `stage_info`'s level there (twice), and moves once the match starts.
#[doc(alias = "grZebes_801D9798")]
pub fn init_lava(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let p = param(r.game);
    r.game.grounds[g.0].u.zebes_lava().xC4 = 0;
    let wait = level_wait(r, p.level(r.data, 0));
    let v = r.game.grounds[g.0].u.zebes_lava();
    v.xC6 = wait;
    v.xC8 = 0;
    v.xCC = 0.0;
    v.xD0 = 0.0;
    v.xDC = None;
    r.game.grounds[g.0].x11_flags.b012 = 1;
    let mut last = 0;
    while last < 0x1D && !entry_empty(p.level(r.data, last + 1)) {
        last += 1;
    }
    let rand = reg(r.game.objs.rng.draw_f32());
    let level = fp::frsp(f64::from(p.level(r.data, last)[3]));
    let y = mem(fp::fmadds(reg(p.f(r.data, LAVA_SPREAD)), rand, level));
    let v = r.game.grounds[g.0].u.zebes_lava();
    v.xD8 = y;
    v.xD4 = y;
    if let Some(j) = nth_joint(r.game, gobj, 0) {
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
        let s = mem(fp::fadds(fp::fneg(55.0), reg(y)));
        set_x724(r.game, s);
        set_x724(r.game, s);
    }
    setup::add_start_hook(r.game, gobj, CodeRef(LAVA_START));
}

/// The lava's process (map 8), `grZebes_801D99E0`. Waiting (1) with the camera shaking at the
/// end, then it picks its entry's level plus a random spread within its bounds, and the top
/// speed that reaches it in time, with a sound up or down (2, then 3); it speeds up toward it
/// until it would overshoot while slowing (4), then slows to a stop near it and waits on the
/// next entry's wait (the table wrapping). Every tick the fog's and the light's colors follow
/// its level, its joint and `stage_info`'s level too.
#[doc(alias = "grZebes_801D99E0")]
pub fn proc_lava(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let d = r.data;
    let state = r.game.grounds[g.0].u.zebes_lava().xC8;
    match state {
        1 => {
            let v = r.game.grounds[g.0].u.zebes_lava();
            v.xC6 = v.xC6.wrapping_sub(1);
            if v.xC6 < 0x78 {
                super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
            }
            if r.game.grounds[g.0].u.zebes_lava().xC6 < 0 {
                let rand = reg(r.game.objs.rng.draw_f32());
                let idx = r.game.grounds[g.0].u.zebes_lava().xC4;
                let level = fp::frsp(f64::from(p.level(d, i32::from(idx))[3]));
                let v = r.game.grounds[g.0].u.zebes_lava();
                v.xD4 = mem(fp::fmadds(reg(p.f(d, LAVA_SPREAD)), rand, level));
                let (top, bottom) = (p.f(d, LAVA_TOP), p.f(d, LAVA_BOTTOM));
                if reg(v.xD4) > reg(top) {
                    v.xD4 = top;
                } else if reg(v.xD4) < reg(bottom) {
                    v.xD4 = bottom;
                }
                v.xD0 = 0.0;
                v.xC8 = 2;
                v.xC6 = 0;
                let delta = fp::fsubs(reg(v.xD4), reg(v.xD8));
                let diff = if delta < 0.0 { fp::fneg(delta) } else { delta };
                let (speed, accel) = (reg(p.f(d, LAVA_SPEED)), reg(p.f(d, LAVA_ACCEL)));
                let disc = fp::fsubs(
                    fp::fmuls(speed, speed),
                    fp::fdivs(fp::fmuls(4.0, diff), accel),
                );
                let v = r.game.grounds[g.0].u.zebes_lava();
                if disc < 0.0 {
                    v.xCC = 999.0;
                } else {
                    let root = crate::math::sqrt_reg(disc);
                    v.xCC = mem(fp::frsp(fp::fmul(
                        0.5,
                        fp::fmuls(accel, fp::fsubs(speed, root)),
                    )));
                    if reg(v.xCC) < 0.0 {
                        v.xCC = 999.0;
                    }
                }
                let up = reg(v.xD4) > reg(v.xD8);
                r.stage_sound(if up { 0x61A80 } else { 0x61A81 });
            }
        }
        2 => r.game.grounds[g.0].u.zebes_lava().xC8 = 3,
        3 => {
            let v = r.game.grounds[g.0].u.zebes_lava();
            v.xC6 = v.xC6.wrapping_add(1);
            if v.xC6 < 60 {
                super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
            }
            let v = r.game.grounds[g.0].u.zebes_lava();
            let vel = reg(v.xD0);
            let accel = reg(p.f(d, LAVA_ACCEL));
            let t = fp::fdivs(vel, accel);
            let threshold = fp::frsp(fp::fnmsub(
                fp::fmul(0.5, accel),
                fp::fmuls(t, t),
                fp::fmuls(vel, t),
            ));
            let delta = fp::fsubs(reg(v.xD4), reg(v.xD8));
            let abs = if delta < 0.0 { fp::fneg(delta) } else { delta };
            if abs < threshold || abs < accel {
                v.xC8 = 4;
            } else {
                v.xD0 = mem(fp::fadds(reg(v.xD0), accel));
                if reg(v.xD0) > reg(v.xCC) {
                    v.xD0 = v.xCC;
                }
            }
            step_level(v);
        }
        4 => {
            let v = r.game.grounds[g.0].u.zebes_lava();
            let delta = fp::fsubs(reg(v.xD4), reg(v.xD8));
            v.xD0 = mem(fp::fsubs(reg(v.xD0), reg(p.f(d, LAVA_ACCEL))));
            let abs = if delta < 0.0 { fp::fneg(delta) } else { delta };
            if reg(v.xD0) < 0.0 || abs < reg(v.xD0) {
                v.xC8 = 1;
                v.xD0 = 0.0;
                v.xC4 = v.xC4.wrapping_add(1);
                let idx = v.xC4;
                if idx == 0x1E || entry_empty(p.level(d, i32::from(idx))) {
                    v.xC4 = 0;
                }
                let idx = r.game.grounds[g.0].u.zebes_lava().xC4;
                let wait = level_wait(r, p.level(d, i32::from(idx)));
                r.game.grounds[g.0].u.zebes_lava().xC6 = wait;
            }
            step_level(r.game.grounds[g.0].u.zebes_lava());
        }
        _ => {}
    }
    let level = r.game.grounds[g.0].u.zebes_lava().xD8;
    fog_color(r.game, level);
    light_color(r, gobj, level);
    if let Some(j) = nth_joint(r.game, gobj, 0) {
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = level);
        let s = mem(fp::fadds(fp::fneg(55.0), reg(level)));
        set_x724(r.game, s);
    }
}

/// The lava's level moved toward its target by its speed.
fn step_level(v: &mut super::vars::ZebesLavaVars) {
    v.xD8 = if reg(v.xD4) > reg(v.xD8) {
        mem(fp::fadds(reg(v.xD8), reg(v.xD0)))
    } else {
        mem(fp::fsubs(reg(v.xD8), reg(v.xD0)))
    };
}

/// The background's setup (map 1), `grZebes_801D9F30`: its animation, still, in the stage's
/// layer.
#[doc(alias = "grZebes_801D9F30")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].u.zebes_back().xC4 = 0;
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The background's process, `grZebes_801D9F84`: still (0), it reacts once the lava's level
/// rises past -20 (animation 1) or 5 (2) with a sound, and is still again when that ends.
#[doc(alias = "grZebes_801D9F84")]
pub fn proc_back(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let state = r.game.grounds[g.0].u.zebes_back().xC4;
    match state {
        0 => {
            let (level, _) = x724_and_step(r.game);
            let anim = if reg(level) > 5.0 {
                2
            } else if reg(level) > fp::fneg(20.0) {
                1
            } else {
                return;
            };
            anime::start(r.game, r.data, gobj, 3, 7, anim, 0.0, 1.0, true);
            anime::clear_loop(r.game, gobj, 3, 7, true);
            r.game.grounds[g.0].u.zebes_back().xC4 = 1;
            r.stage_sound(0x61A85);
        }
        1 if anime::finished(r.game, gobj, 3, 7) => {
            anime::start(r.game, r.data, gobj, 3, 7, 0, 0.0, 1.0, true);
            r.game.grounds[g.0].u.zebes_back().xC4 = 0;
        }
        _ => {}
    }
}

/// The fog's color by the lava's level, `grZebes_801DA0C4`: between the table's colors around
/// the level (the ends keep the fog as it is).
#[doc(alias = "grZebes_801DA0C4")]
pub fn fog_color(game: &mut Game, level: f32) {
    let level = reg(level);
    let mut idx = 0usize;
    while idx < 3 && level > reg(FOG_TABLE[idx][0]) {
        idx += 1;
    }
    if idx == 0 || idx == 3 {
        return;
    }
    let (lo, hi) = (FOG_TABLE[idx - 1], FOG_TABLE[idx]);
    let t0 = reg(lo[0]);
    let t = fp::fdivs(fp::fsubs(level, t0), fp::fsubs(reg(hi[0]), t0));
    if let Some(mut c) = super::ground::fog_color(game) {
        let mix = |k: usize| {
            let a = reg(lo[k]);
            fp::fctiwz(fp::fmadds(t, fp::fsubs(reg(hi[k]), a), a)) as u8
        };
        c.r = mix(1);
        c.g = mix(2);
        c.b = mix(3);
        super::ground::set_fog_color(game, c);
    }
}

/// The stage light's color by the lava's level, `grZebes_801DA254`: the first light of the
/// lights' link with its lowest flag (found once and kept in `xDC`), between a dark and a
/// bright color from the lava's lowest to its highest level.
#[doc(alias = "grZebes_801DA254")]
pub fn light_color(r: &mut Running, gobj: GObjId, level: f32) {
    let g = ground_of(r.game, gobj);
    let mut l = r.game.grounds[g.0].u.zebes_lava().xDC;
    if l.is_none() {
        if let Some(lg) = r.game.world.first_rendered(4) {
            let mut cur = match r.game.world.gobj(lg).obj {
                Some(super::super::state::Obj::LObj(l)) => Some(l),
                _ => None,
            };
            while let Some(c) = cur {
                if r.game.objs.lobjs[c].flags & 1 != 0 {
                    break;
                }
                cur = r.game.objs.lobjs[c].next;
            }
            l = cur;
        }
        r.game.grounds[g.0].u.zebes_lava().xDC = l;
    }
    let Some(l) = l else { return };
    let p = param(r.game);
    let (top, bottom) = (reg(p.f(r.data, LAVA_TOP)), reg(p.f(r.data, LAVA_BOTTOM)));
    let t = fp::fdivs(fp::fsubs(reg(level), bottom), fp::fsubs(top, bottom));
    let mix = |hi: u8, lo: u8| {
        let d = fp::frsp(f64::from(i32::from(hi) - i32::from(lo)));
        fp::fctiwz(fp::fmadds(t, d, fp::frsp(f64::from(lo)))) as u8
    };
    let c = [mix(0xE6, 0x1E), mix(0xFF, 0x1E), mix(0x96, 0), 0xFF];
    crate::hsd::lobj::set_color(&mut r.game.objs.lobjs, l, c);
}

/// No splashes yet, `grZebes_801DA3E8`.
#[doc(alias = "grZebes_801DA3E8")]
pub fn reset_splashes(game: &mut Game) {
    game.gr.zebes.splashes = 0;
}

/// A splash on the lava at `pos`, `grZebes_801DA3F4` (also `Ground_801C43A4`), if fewer than
/// 10 are out.
#[doc(alias = "grZebes_801DA3F4")]
#[doc(alias = "Ground_801C43A4")]
pub fn add_splash(r: &mut Running, pos: Vec3) {
    if r.game.gr.zebes.splashes >= 10 {
        return;
    }
    let Some(gobj) = create(r, MAP_SPLASH, SPLASH_FRAME) else {
        return;
    };
    match jobj_of(r.game, gobj) {
        Some(j) => {
            jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
            r.game.gr.zebes.splashes = r.game.gr.zebes.splashes.wrapping_add(1);
        }
        None => setup::destroy(r, Some(gobj)),
    }
}

/// A splash gone, `grZebes_801DA4FC`.
#[doc(alias = "grZebes_801DA4FC")]
pub fn remove_splash(r: &mut Running, gobj: GObjId) {
    r.game.gr.zebes.splashes = r.game.gr.zebes.splashes.wrapping_sub(1);
    setup::destroy(r, Some(gobj));
}

/// Shows a pop on one of the top's bubbles in place, `grZebes_801DAA08`: one at random of those
/// without one, its model hidden and the pop's (map 2's first joint, with map 2's animations)
/// under it, from its first frame. Whether one showed.
#[doc(alias = "grZebes_801DAA08")]
pub fn show_pop(r: &mut Running) -> bool {
    let z = &r.game.gr.zebes;
    let stale = &mut r.game.objs.stale_reads;
    let candidates: Vec<usize> = (0..20)
        .filter(|&i| {
            let b = &z.bubbles[i];
            b.x00 == 1 && i != 0 && i != 6 && b.pop(stale).is_none()
        })
        .collect();
    if candidates.is_empty() {
        return false;
    }
    let pick = r.game.objs.rng.below(candidates.len() as i32);
    let sel = candidates[pick as usize];
    let Some(pop) = super::ground::load_map_joint(r.game, r.data, 2, 0) else {
        return false;
    };
    r.game.gr.zebes.bubbles[sel].x04 = Some(pop);
    let gobj = r.game.gr.zebes.bubbles[sel].x20.expect("a bubble's object");
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::set_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    jobj::add_child(&mut r.game.objs.jobjs, child, Some(pop));
    let head = super::datfiles::archive_for(r.game, r.data, 2).expect("grDatFiles_801C6330(2)");
    let entry = head.entry(r.data, 2);
    let first = |k: u32| entry.anims(r.data, k).and_then(|l| r.data.ptr(l));
    let (aj, mj, sj) = (first(0), first(1), first(2));
    jobj::add_anim_all(&mut r.game.objs, r.data, Some(pop), aj, mj, sj);
    jobj::req_anim_all(&mut r.game.objs, Some(pop), 0.0);
    let mut hooks = crate::game::code::ParticleHooks;
    jobj::anim_all(&mut r.game.objs, r.data, &mut hooks, Some(pop));
    anime::clear_loop(r.game, gobj, 0, 7, true);
    true
}

/// A hit the organic platform's item took, `fn_801DAC90`: the first of the top's bubbles in
/// place pops, and bubbles wait 120 ticks before growing back.
#[doc(alias = "fn_801DAC90")]
pub fn bubbles_hit(r: &mut Running) {
    let z = &mut r.game.gr.zebes;
    if let Some(i) = (0..20).find(|&i| z.bubbles[i].x00 == 1 && i != 0 && i != 6) {
        z.bubbles[i].x00 = 2;
        z.pop_wait = 0x78;
        z.bubbles[i].x1C = mem(fp::frsp(0.001));
    }
}

/// Adds bubble `i` at `x`, `y` with `size` and state `state`, `grZebes_801DAE70`, unless it's
/// there or still has an object: its object (map 3) placed and scaled, but for the anchors.
/// `frame` is where the caller's call puts the object's scale joint's descriptor on the stack.
#[doc(alias = "grZebes_801DAE70")]
pub fn add_bubble(r: &mut Running, i: i32, state: u8, x: f32, y: f32, size: f32, frame: u32) {
    let b = r.game.gr.zebes.bubbles[i as usize];
    if b.x00 != 0 || b.x20.is_some() {
        return;
    }
    let b = &mut r.game.gr.zebes.bubbles[i as usize];
    b.x08 = x;
    b.x0C = y;
    b.x10 = 0.0;
    b.x14 = 0.0;
    b.x1C = size;
    b.x18 = size;
    let gobj = if i == 0 || i == 6 {
        None
    } else {
        let gobj = create(r, MAP_BUBBLE, frame);
        if let Some(o) = gobj {
            let g = ground_of(r.game, o);
            let j = jobj_of(r.game, o).expect("a bubble's model");
            jobj::set_translate(&mut r.game.objs.jobjs, j, Vec3::new(x, y, 0.0));
            let s = Vec3::new(size, size, mem(fp::frsp(0.001)));
            jobj::set_scale(&mut r.game.objs.jobjs, j, s);
            let v = r.game.grounds[g.0].u.zebes_bubble();
            v.xC4 = None;
            v.xC8 = i;
        }
        gobj
    };
    let b = &mut r.game.gr.zebes.bubbles[i as usize];
    b.x00 = state;
    b.x20 = gobj;
    b.x04 = None;
    b.x04_left = false;
}

/// Bubble `i`'s tick, `grZebes_801DB088`: its size grows toward its target, a finished pop's
/// model goes, and a bubble falling apart (3) goes when its countdown ends, with an effect and,
/// through the stage object's sound slot, a sound; a popped one (2) goes at once, with the
/// effect, and on the top (between the anchors) the nearest bubble below within 8 takes its
/// place, or the rest below fall apart (then whether they did). `gobj` is the stage's object,
/// which the original passes where a `Ground` belongs ([`pop_sound`]).
#[doc(alias = "grZebes_801DB088")]
pub fn tick_bubble(r: &mut Running, gobj: GObjId, i: usize) -> bool {
    let b = r.game.gr.zebes.bubbles[i];
    if b.x00 == 0 {
        return false;
    }
    let Some(obj) = b.x20 else { return false };
    let og = ground_of(r.game, obj);
    let item = r.game.grounds[og.0].u.zebes_bubble().xC4;
    if let Some(it) = item {
        if b.x00 == 1 {
            material::item_hurtbox_on(r.game, it);
        } else {
            material::item_hurtbox_off(r.game, it);
        }
    }
    let speed = reg(param(r.game).f(r.data, GROW_SPEED));
    let (target, cur) = (reg(b.x1C), reg(b.x18));
    let diff = fp::fsubs(target, cur);
    let bb = &mut r.game.gr.zebes.bubbles[i];
    bb.x18 = if diff > speed {
        mem(fp::fadds(cur, speed))
    } else if diff < fp::fneg(speed) {
        mem(fp::fsubs(cur, speed))
    } else {
        b.x1C
    };
    if let Some(pop) = b.pop(&mut r.game.objs.stale_reads)
        && anime::finished(r.game, obj, 0, 7)
    {
        jobj::remove_all(&mut r.game.objs, Some(pop));
        r.game.gr.zebes.bubbles[i].x04 = None;
        let j = jobj_of(r.game, obj);
        if j.is_some() {
            jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        }
    }
    let mut result = false;
    match r.game.gr.zebes.bubbles[i].x00 {
        3 => {
            let bb = &mut r.game.gr.zebes.bubbles[i];
            let t = bb.x02;
            bb.x02 = t.wrapping_sub(1);
            if t < 0 {
                if let Some(it) = item {
                    material::destroy_item(r, it);
                }
                setup::destroy(r, Some(obj));
                pop_effect(r, i);
                pop_sound(r, gobj);
                let bb = &mut r.game.gr.zebes.bubbles[i];
                bb.x20 = None;
                bb.x04 = None;
                bb.x00 = 0;
            }
        }
        2 => {
            if let Some(it) = item {
                material::destroy_item(r, it);
            }
            setup::destroy(r, Some(obj));
            pop_effect(r, i);
            let bb = &mut r.game.gr.zebes.bubbles[i];
            bb.x20 = None;
            bb.x04 = None;
            bb.x00 = 0;
            if i < 7 && i != 0 && i != 6 {
                let z = &mut r.game.gr.zebes;
                let (ex, ey) = (reg(z.bubbles[i].x08), reg(z.bubbles[i].x0C));
                let mut best = 64.0;
                let mut best_idx = None;
                for j in 7..20 {
                    let c = &z.bubbles[j];
                    if c.x00 == 1 {
                        let dx = fp::fsubs(reg(c.x08), ex);
                        let dy = fp::fsubs(reg(c.x0C), ey);
                        let dist = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
                        if dist < best {
                            best_idx = Some(j);
                            best = dist;
                        }
                    }
                }
                if let Some(j) = best_idx {
                    let c = z.bubbles[j];
                    let bb = &mut z.bubbles[i];
                    bb.x00 = c.x00;
                    bb.x04 = c.x04;
                    bb.x18 = c.x18;
                    bb.x1C = c.x1C;
                    bb.x20 = c.x20;
                    z.bubbles[j].x00 = 0;
                    z.bubbles[j].x20 = None;
                    z.bubbles[j].x04_left = true;
                } else {
                    for j in 7..20 {
                        if r.game.gr.zebes.bubbles[j].x00 == 1 {
                            r.game.gr.zebes.bubbles[j].x00 = 3;
                            let t = r.game.objs.rng.below(0x3C).wrapping_add(0xA);
                            r.game.gr.zebes.bubbles[j].x02 = t as i16;
                        }
                    }
                    result = true;
                }
            }
        }
        _ => {}
    }
    result
}

/// A gone bubble's effect, at its place a little in front.
fn pop_effect(r: &mut Running, i: usize) {
    let b = r.game.gr.zebes.bubbles[i];
    let pos = Vec3::new(b.x08, b.x0C, 4.0);
    let _ = grlib::generator_at(r.game, r.data, 0x7530, 0x1E, pos);
}

/// A bubble falling apart's sound, `Ground_801C5440(gp, 0, 0x61A86)` with the stage's GObj
/// where its `Ground` belongs: the "sound slot" is the GObj's word at 0x20, the high half of
/// its camera link mask (`gxlink_prios`), which the sound handle overwrites (and whose old value
/// is stopped as a handle unless it's -1).
fn pop_sound(r: &mut Running, gobj: GObjId) {
    let mask = r.game.world.gobj(gobj).gxlink_prios;
    let old = (mask >> 32) as u32 as i32;
    if old != -1 {
        r.sfx_stop(old);
    }
    let h = r.sfx_play(0x61A86, 0x7F, 0x40);
    let o = r.game.world.gobj_mut(gobj);
    o.gxlink_prios = (mask & 0xFFFF_FFFF) | u64::from(h as u32) << 32;
}

/// The bubbles' tick, `grZebes_801DB3CC`: each bubble's own tick (whether any fell apart), then
/// their speeds: sinking; the top's five pulled toward their neighbours past their spacing (one
/// without both neighbours falls apart, unless it's new); every pair pushed apart closer than
/// their spacing scaled by their sizes and drawn together between their near and far reaches,
/// the anchors and the top unmoved by it, pushes never downward; then they move, kept out of
/// the corners beside the two ends, at bounded speeds, the anchors on their places.
#[doc(alias = "grZebes_801DB3CC")]
pub fn move_bubbles(r: &mut Running, gobj: GObjId) -> bool {
    let mut popped = false;
    for i in 0..20 {
        if r.game.gr.zebes.bubbles[i].x00 != 0 && tick_bubble(r, gobj, i) {
            popped = true;
        }
    }
    let p = param(r.game);
    let d = r.data;
    let sink = p.f(d, BUBBLE_SINK);
    for b in &mut r.game.gr.zebes.bubbles {
        if b.x00 != 0 {
            b.x10 = 0.0;
            b.x14 = mem(fp::fneg(reg(sink)));
        }
    }
    let spacing = reg(p.f(d, BUBBLE_SPACING));
    let pull_max = reg(p.f(d, BUBBLE_PULL_MAX));
    for idx in 0..7 {
        let active = r.game.gr.zebes.bubbles[idx].x00;
        if !(active == 1 || active == 4) || idx == 0 || idx == 6 {
            continue;
        }
        let (mut fx, mut fy) = (0.0, 0.0);
        let mut neighbours = 0;
        for (n, other) in [(0, idx - 1), (1, idx + 1)] {
            let z = &r.game.gr.zebes;
            let o = z.bubbles[other];
            if o.x00 == 0 {
                continue;
            }
            let cur = z.bubbles[idx];
            let mut dx = fp::fsubs(reg(o.x08), reg(cur.x08));
            let dy = fp::fsubs(reg(o.x0C), reg(cur.x0C));
            let mut dist = crate::math::sqrt_reg(fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
            if dist > spacing {
                dx = fp::fdivs(dx, dist);
                let mut push = fp::fsubs(dist, spacing);
                dist = fp::fdivs(dy, dist);
                if push > pull_max {
                    push = pull_max;
                }
                fx = fp::fmadds(dx, push, fx);
                fy = fp::fmadds(dist, push, fy);
            }
            neighbours = if n == 0 { 1 } else { neighbours + 1 };
        }
        let cur = &mut r.game.gr.zebes.bubbles[idx];
        cur.x08 = mem(fp::fadds(reg(cur.x08), fx));
        cur.x0C = mem(fp::fadds(reg(cur.x0C), fy));
        if cur.x00 != 4 && neighbours != 2 {
            cur.x00 = 3;
            let t = r.game.objs.rng.below(0x3C).wrapping_add(0xA);
            r.game.gr.zebes.bubbles[idx].x02 = t as i16;
        }
    }
    let (far, near, attract) = (
        p.f(d, BUBBLE_FAR),
        p.f(d, BUBBLE_NEAR),
        p.f(d, BUBBLE_ATTRACT),
    );
    let bubbles = &mut r.game.gr.zebes.bubbles;
    for a in 0..19 {
        if bubbles[a].x00 == 0 {
            continue;
        }
        for b in a + 1..20 {
            if bubbles[b].x00 == 0 {
                continue;
            }
            let (ea, eb) = (bubbles[a], bubbles[b]);
            let mut dx = fp::fsubs(reg(eb.x08), reg(ea.x08));
            let mut dy = fp::fsubs(reg(eb.x0C), reg(ea.x0C));
            let mut dist = crate::math::sqrt_reg(fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
            let sizes = fp::fadds(reg(ea.x18), reg(eb.x18));
            let col = fp::fmuls(fp::fdivs(spacing, 2.0), sizes);
            let near_r = fp::fmuls(fp::fdivs(reg(near), 2.0), sizes);
            let far_r = fp::fmuls(fp::fdivs(reg(far), 2.0), sizes);
            if dist < 0.001 {
                dy = fp::frsp(0.001);
                dx = 0.0;
                dist = fp::frsp(0.001);
            }
            if dist < col {
                let s = fp::frsp(fp::fdiv(fp::fsub(fp::fdivs(col, dist), 1.0), 2.0));
                let s = fp::frsp(fp::fmul(s, 0.9));
                if a >= 7 {
                    let vx = fp::fmuls(fp::fneg(dx), s);
                    let vy = fp::fmuls(fp::fneg(dy), s);
                    let vy = if vy < 0.0 { 0.0 } else { vy };
                    let e = &mut bubbles[a];
                    e.x10 = mem(fp::fadds(reg(e.x10), vx));
                    e.x14 = mem(fp::fadds(reg(e.x14), vy));
                }
                if b >= 7 {
                    let vx = fp::fmuls(dx, s);
                    let vy = fp::fmuls(dy, s);
                    let vy = if vy < 0.0 { 0.0 } else { vy };
                    let e = &mut bubbles[b];
                    e.x10 = mem(fp::fadds(reg(e.x10), vx));
                    e.x14 = mem(fp::fadds(reg(e.x14), vy));
                }
            } else if !(dist < near_r) && dist < far_r {
                let k = fp::fdivs(reg(attract), dist);
                if a >= 7 {
                    let e = &mut bubbles[a];
                    e.x10 = mem(fp::fadds(reg(e.x10), fp::fmuls(dx, k)));
                    e.x14 = mem(fp::fadds(reg(e.x14), fp::fmuls(dy, k)));
                }
                if b >= 7 {
                    let e = &mut bubbles[b];
                    e.x10 = mem(fp::fadds(reg(e.x10), fp::fmuls(fp::fneg(dx), k)));
                    e.x14 = mem(fp::fadds(reg(e.x14), fp::fmuls(fp::fneg(dy), k)));
                }
            }
        }
    }
    let z = &mut r.game.gr.zebes;
    let (la, ra) = (z.anchors[0], z.anchors[1]);
    let (lb, rb) = (z.bounds[0], z.bounds[1]);
    let vmax = p.f(d, BUBBLE_SPEED_MAX);
    for (k, b) in z.bubbles.iter_mut().enumerate() {
        if b.x00 == 0 {
            continue;
        }
        if k == 0 || k == 6 {
            let a = if k == 0 { la } else { ra };
            b.x10 = 0.0;
            b.x14 = 0.0;
            b.x08 = a.x;
            b.x0C = a.y;
            continue;
        }
        let mut nx = mem(fp::fadds(reg(b.x08), reg(b.x10)));
        let mut ny = mem(fp::fadds(reg(b.x0C), reg(b.x14)));
        if reg(ny) < reg(lb.y) && reg(nx) < reg(lb.x) {
            if reg(b.x0C) >= reg(lb.y) {
                ny = lb.y;
            } else if reg(b.x08) >= reg(lb.x) {
                nx = lb.x;
            }
        }
        if reg(ny) < reg(rb.y) && reg(nx) > reg(rb.x) {
            if reg(b.x0C) >= reg(rb.y) {
                ny = rb.y;
            } else if reg(b.x08) <= reg(rb.x) {
                nx = rb.x;
            }
        }
        b.x08 = nx;
        b.x0C = ny;
        if reg(b.x10) > reg(vmax) {
            b.x10 = vmax;
        } else if reg(b.x10) < fp::fneg(reg(vmax)) {
            b.x10 = mem(fp::fneg(reg(vmax)));
        }
        if reg(b.x14) > reg(vmax) {
            b.x14 = vmax;
        } else if reg(b.x14) < fp::fneg(reg(vmax)) {
            b.x14 = mem(fp::fneg(reg(vmax)));
        }
    }
    popped
}

/// Spans the organic platform's hurtbox item over its top's bubbles in place,
/// `grZebes_801DBB60`: off with none; a ball on one, or on two at the same place; otherwise a
/// capsule between the two furthest apart, widened and stretched until it covers each bubble.
/// Whether it's on.
#[doc(alias = "grZebes_801DBB60")]
pub fn span_bubbles(r: &mut Running, item: GObjId) -> bool {
    let top = |i: usize, b: &Bubble| b.x00 == 1 && i != 0 && i != 6;
    let z = &r.game.gr.zebes;
    let mut count = 0;
    let mut last = 0usize;
    for (i, b) in z.bubbles.iter().enumerate() {
        if top(i, b) {
            count += 1;
            last = i;
            if count > 2 {
                break;
            }
        }
    }
    if count == 0 {
        material::item_hurtbox_off(r.game, item);
        return false;
    }
    let ball = |r: &mut Running, i: usize| {
        let b = r.game.gr.zebes.bubbles[i];
        let p = Vec3::new(b.x08, b.x0C, 0.0);
        let size = mem(fp::frsp(fp::fmul(2.0, reg(b.x18))));
        material::set_item_hurtbox(r.game, item, p, p, size);
    };
    material::item_hurtbox_on(r.game, item);
    if count == 1 {
        ball(r, last);
        return true;
    }
    let z = &r.game.gr.zebes;
    let mut max = reg(-1.0);
    let (mut x1, mut y1, mut x2, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..20 {
        let ei = z.bubbles[i];
        if !top(i, &ei) {
            continue;
        }
        for j in i + 1..20 {
            let ej = z.bubbles[j];
            if !top(j, &ej) {
                continue;
            }
            let dx = fp::fsubs(reg(ei.x08), reg(ej.x08));
            let dy = fp::fsubs(reg(ei.x0C), reg(ej.x0C));
            let dist = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
            if !(max >= dist) {
                max = dist;
                x1 = reg(ei.x08);
                y1 = reg(ei.x0C);
                x2 = reg(ej.x08);
                y2 = reg(ej.x0C);
                last = if reg(ei.x18) > reg(ej.x18) { i } else { j };
            }
        }
    }
    assert!(max >= 0.0, "grzebes.c: 2258: 0");
    if max < reg(0.0001) {
        ball(r, last);
        return true;
    }
    let sqrt = |x: f64| reg(mem(crate::math::sqrt_reg(x)));
    let mut dy = fp::fsubs(y2, y1);
    let mut dx = fp::fsubs(x2, x1);
    let mut width = 1.0;
    let mut inv = fp::fdivs(1.0, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
    for k in 0..20 {
        let b = r.game.gr.zebes.bubbles[k];
        if !top(k, &b) {
            continue;
        }
        let (by, bx) = (reg(b.x0C), reg(b.x08));
        let py = fp::fsubs(by, y1);
        let px = fp::fsubs(bx, x1);
        let t = fp::fmuls(inv, fp::fmadds(dx, px, fp::fmuls(dy, py)));
        let d2 = if t < 0.0 {
            fp::fmadds(px, px, fp::fmuls(py, py))
        } else if t > 1.0 {
            let ex = fp::fsubs(bx, x2);
            let ey = fp::fsubs(by, y2);
            fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey))
        } else {
            let ex = fp::fsubs(fp::fmadds(dx, t, x1), bx);
            let ey = fp::fsubs(fp::fmadds(dy, t, y1), by);
            fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey))
        };
        let dist = if d2 > 0.0 { sqrt(d2) } else { d2 };
        let reach = fp::frsp(fp::fmadd(2.0, reg(b.x18), dist));
        if !(reach > width) {
            continue;
        }
        let near_first = t < reg(0.5);
        let (ax, ay) = if near_first { (x1, y1) } else { (x2, y2) };
        let ex = fp::fsubs(bx, ax);
        let ey = fp::fsubs(by, ay);
        let d = fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey));
        let d = if d > 0.0 { sqrt(d) } else { d };
        let new_width = fp::frsp(fp::fmul(
            0.5,
            fp::fmadd(2.0, reg(b.x18), fp::fadds(d, width)),
        ));
        let (mut ax, mut ay) = (ax, ay);
        if d > reg(0.001) {
            let s = fp::fdivs(fp::fsubs(new_width, width), d);
            ax = fp::fmadds(s, fp::fsubs(bx, ax), ax);
            ay = fp::fmadds(s, fp::fsubs(by, ay), ay);
        }
        let (ox, oy) = if near_first { (x2, y2) } else { (x1, y1) };
        let qy = fp::fsubs(ay, oy);
        let qx = fp::fsubs(ax, ox);
        let len = fp::fmadds(qx, qx, fp::fmuls(qy, qy));
        let len = if len > 0.0 { sqrt(len) } else { len };
        let s = fp::fdivs(fp::fsubs(new_width, width), len);
        width = new_width;
        let ox = fp::fmadds(qx, s, ox);
        let oy = fp::fmadds(qy, s, oy);
        if near_first {
            (x1, y1, x2, y2) = (ax, ay, ox, oy);
        } else {
            (x1, y1, x2, y2) = (ox, oy, ax, ay);
        }
        dy = fp::fsubs(y2, y1);
        dx = fp::fsubs(x2, x1);
        inv = fp::fdivs(1.0, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
    }
    let a = Vec3::new(mem(x1), mem(y1), 0.0);
    let b = Vec3::new(mem(x2), mem(y2), 0.0);
    material::set_item_hurtbox(r.game, item, a, b, mem(width));
    true
}

/// Places and sizes the bubbles' objects, `grZebes_801DC260`.
#[doc(alias = "grZebes_801DC260")]
pub fn place_bubbles(r: &mut Running) {
    for i in 0..20 {
        let b = r.game.gr.zebes.bubbles[i];
        if b.x00 == 0 {
            continue;
        }
        let Some(obj) = b.x20 else { continue };
        let j = jobj_of(r.game, obj).expect("a bubble's model");
        jobj::set_translate(&mut r.game.objs.jobjs, j, Vec3::new(b.x08, b.x0C, 0.0));
        jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(b.x18, b.x18, b.x18));
    }
}

/// Grows bubbles back, `grZebes_801DC408`: while the platform is whole and after a pop's wait,
/// every so often (a random wait) a new tiny bubble in the first free place, under 15, at a
/// random place between the ends, with a sound.
#[doc(alias = "grZebes_801DC408")]
pub fn grow_bubbles(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.zebes_stage().xC4 != 0 {
        return;
    }
    if r.game.gr.zebes.pop_wait != 0 {
        r.game.gr.zebes.pop_wait -= 1;
        return;
    }
    let v = r.game.grounds[g.0].u.zebes_stage();
    v.xF4 = v.xF4.wrapping_sub(1);
    if v.xF4 >= 0 {
        return;
    }
    let p = param(r.game);
    let d = r.data;
    let z = &r.game.gr.zebes;
    let count = z.bubbles.iter().filter(|b| b.x00 != 0).count();
    let free = z.bubbles.iter().position(|b| b.x00 == 0);
    if count < 15 {
        let y_min = reg(z.anchors[0].y);
        let r_ = reg(p.f(d, BUBBLE_SPACING));
        let x_base = fp::fadds(reg(z.anchors[0].x), r_);
        let x_range = fp::fnmsubs(2.0, r_, fp::fsubs(reg(z.bounds[1].x), reg(z.anchors[0].x)));
        let y_range = fp::fsubs(reg(z.bounds[1].y), y_min);
        let rscale = reg(r.game.objs.rng.draw_f32());
        let ry = reg(r.game.objs.rng.draw_f32());
        let rx = reg(r.game.objs.rng.draw_f32());
        let x = mem(fp::fmadds(x_range, rx, x_base));
        let y = mem(fp::fmadds(y_range, ry, y_min));
        let size = mem(fp::frsp(fp::fmadd(0.5, rscale, 1.0)));
        let i = free.expect("grzebes.c: a free bubble under 15");
        add_bubble(r, i as i32, 1, x, y, size, GROW_BUBBLE_FRAME);
        r.game.gr.zebes.bubbles[i].x18 = mem(fp::frsp(0.001));
        r.stage_sound(0x61A83);
    }
    let lo = fp::fctiwz(reg(p.f(d, GROW_WAIT_MIN)));
    let hi = fp::fctiwz(reg(p.f(d, GROW_WAIT_MAX)));
    let t = grlib::rand_range(&mut r.game.objs.rng, hi, lo);
    r.game.grounds[g.0].u.zebes_stage().xF4 = t as i16;
}

/// The platform's starting bubbles, `grZebes_801DC744`: `which` bit 0 the top's seven, evenly
/// between the anchors, bit 1 nine below in a fixed pattern, each in state `state`. `frame` is
/// where the caller's call puts the objects' scale joints' descriptors on the stack.
#[doc(alias = "grZebes_801DC744")]
pub fn start_bubbles(r: &mut Running, which: i32, state: u8, frame: u32) {
    let z = &r.game.gr.zebes;
    let (a0, a1, b1) = (z.anchors[0], z.anchors[1], z.bounds[1]);
    if which & 1 != 0 {
        let (x0, y0) = (reg(a0.x), reg(a0.y));
        let sx = fp::fdivs(fp::fsubs(reg(a1.x), x0), 6.0);
        let sy = fp::fdivs(fp::fsubs(reg(a1.y), y0), 6.0);
        for (i, &size) in TOP_SIZES.iter().enumerate() {
            let k = i as f64;
            let x = mem(fp::fmadds(sx, k, x0));
            let y = mem(fp::fmadds(sy, k, y0));
            add_bubble(r, i as i32, state, x, y, size, frame);
        }
    }
    if which & 2 != 0 {
        let x0 = reg(a0.x);
        let y0 = reg(a0.y);
        let r_ = reg(param(r.game).f(r.data, BUBBLE_SPACING));
        let x_base = fp::fadds(x0, r_);
        let x_range = fp::fnmsubs(2.0, r_, fp::fsubs(reg(b1.x), x0));
        let y_range = fp::fsubs(reg(b1.y), y0);
        let at = |kx: f64, ky: f64| {
            (
                mem(fp::frsp(fp::fmadd(kx, x_range, x_base))),
                mem(fp::frsp(fp::fmadd(ky, y_range, y0))),
            )
        };
        let top_y = mem(fp::frsp(fp::fadd(y0, y_range)));
        let spots: [(i32, f64, Option<f64>, f32); 9] = [
            (7, 0.5, Some(0.5), 1.0),
            (8, 0.2, Some(0.8), 1.2),
            (9, 0.8, Some(0.2), 1.1),
            (10, 0.8, Some(0.8), 1.1),
            (11, 0.5, Some(0.2), 1.2),
            (12, 0.5, Some(0.9), 1.3),
            (13, 0.5, Some(0.9), 1.3),
            (14, 0.6, None, 1.1),
            (15, 0.2, None, 1.0),
        ];
        for (i, kx, ky, size) in spots {
            let (x, y) = match ky {
                Some(ky) => at(kx, ky),
                None => (at(kx, 0.0).0, top_y),
            };
            add_bubble(r, i, state, x, y, size, frame);
        }
    }
}

/// Sets the organic platform up, `grZebes_801DC9DC`: no bubbles or pop wait, the bounds and
/// anchors where they start, the starting bubbles in place, and 100 of the bubbles' ticks to
/// settle them.
#[doc(alias = "grZebes_801DC9DC")]
pub fn reset_bubbles(r: &mut Running, gobj: GObjId) {
    let z = &mut r.game.gr.zebes;
    z.pop_wait = 0;
    for b in &mut z.bubbles {
        b.x00 = 0;
        b.x20 = None;
    }
    z.anchors = START_ANCHORS;
    z.bounds = START_BOUNDS;
    start_bubbles(r, 3, 1, SETUP_BUBBLE_FRAME);
    for _ in 0..100 {
        move_bubbles(r, gobj);
    }
}

/// The spawn configurations' callbacks, `grZebes_801DCB64` and `grZebes_801DCBB0`: a lava
/// bubble's effect at `pos`, 186 for the first lines, 187 for the rest (from the stage's ninth
/// and the island's third).
#[doc(alias = "grZebes_801DCB64")]
#[doc(alias = "grZebes_801DCBB0")]
pub fn lava_bubble(r: &mut Running, cb: u32, pos: Vec3, line: i32) {
    let first = if cb == STAGE_BUBBLES { 9 } else { 3 };
    let idx = if line < first { 0xBA } else { 0xBB };
    let _ = grlib::generator_at(r.game, r.data, idx, 0, pos);
}

/// The lava as a fighter device, `grZebes_801DCBFC`: a fighter below its surface takes its hit
/// (the parameters' word at 0x2C); one that just fell in splashes, with two sounds.
#[doc(alias = "grZebes_801DCBFC")]
pub fn lava_device(r: &mut Running, fighter: GObjId) -> Option<DataRef> {
    let (level, step) = x724_and_step(r.game);
    let fp_ = r.game.fighter(fighter);
    let pos = fp_.cur_pos;
    let prev_y = mem(fp::fadds(reg(fp_.prev_pos.y), reg(step)));
    if !(reg(pos.y) < reg(level)) {
        return None;
    }
    let p = param(r.game);
    let hit = r.data.ptr(p.0.at(LAVA_HIT));
    if reg(prev_y) > reg(level) {
        add_splash(r, pos);
        r.stage_sound(0x61A82);
        r.stage_sound(0x66);
    }
    Some(hit.expect("grzebes.c: the lava's hit"))
}

/// The stage's shadow check, `grZebes_801DCCC0`: every joint receives shadows.
#[doc(alias = "grZebes_801DCCC0")]
pub fn check_shadow_render() -> bool {
    true
}

/// The lowest the camera's eye goes on Brinstar, `grZebes_801DCCC8`: 5 above the lava.
#[doc(alias = "grZebes_801DCCC8")]
pub fn camera_floor(s: &super::super::structs::StageInfo) -> f32 {
    mem(fp::fadds(5.0, reg(s.x724)))
}
