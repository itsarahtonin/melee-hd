//! Mushroom Kingdom, `melee/gr/grinishie1.c`: the stage (map 3) with its grid of brick blocks
//! and its two lifts on a pulley, the ? blocks over brick blocks (map 2), and the scenery
//! (map 0; map 1's callbacks have no caller).
//!
//! A brick block is a collision joint and a hurtbox item (`it::kinds::yaku`): hit, or bumped
//! from below fast enough (its ceiling's joint callback), it breaks with an effect, a sound
//! and a small quake, and comes back after a while, blinking in. With items on, two random
//! blocks hold an item from the start, shown by a ? block over them, and another comes after
//! one is broken; now and then every block holds one. The lifts balance the weight on them:
//! the heavier side sinks and the other rises, and pulled too far they fall away and come back
//! later. Under 20 seconds left, the music speeds up.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{INISHIE1_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::vars::Inishie1Block;
use super::{CollUser, anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::dobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// The stage's map id, which holds the blocks and the lifts, and the ? blocks'.
pub const MAP_STAGE: i32 = 3;
pub const MAP_HATENA: i32 = 2;

/// `fn_801FBEB8`, the brick blocks' collision joints' callback (`mpJointSetCb2`, a ceiling's).
pub const BLOCK_BUMPED: u32 = 0x801F_BEB8;
/// `fn_801FBF6C`, a brick block's hurtbox item's callback for a hit it takes.
pub const BLOCK_HIT: u32 = 0x801F_BF6C;
/// `fn_801FC9AC`, the lifts' collision joints' callback (`mpJointSetCb1`).
pub const LIFT_TOUCHED: u32 = 0x801F_C9AC;

/// Where the calls that make the ? blocks put their scale joints' descriptors on the stack: the
/// stage's setup through `grInishie1_801FAADC`, `grInishie1_801FAD84` and
/// `grInishie1_801FBAA0` (0xE0 below `setup::SETUP_FRAME`, the setup's frame being 8 bytes, as
/// Yoshi's Story's), and the stage's process through `grInishie1_801FB3F0` and
/// `grInishie1_801FBAA0` (0x140 below the process loop's frame, as Kongo Jungle's events are
/// 0x70 below it).
pub const SETUP_HATENA_FRAME: u32 = setup::SETUP_FRAME - 0xE0;
pub const PROC_HATENA_FRAME: u32 = 0x804E_E910;

/// The brick blocks, 3, 13 and 3 in rows (`grI1_803E49B8`): each block's collision joint and its
/// joint in the stage's model.
const BLOCKS: [(i32, i32); 19] = [
    (13, 3),
    (14, 14),
    (15, 15),
    (16, 16),
    (17, 17),
    (18, 18),
    (19, 19),
    (11, 20),
    (12, 21),
    (1, 4),
    (2, 5),
    (3, 6),
    (4, 7),
    (5, 8),
    (6, 9),
    (7, 10),
    (8, 11),
    (9, 12),
    (10, 13),
];
const BLOCK_COUNT: usize = BLOCKS.len();

/// A ? block's item's speed out of its brick block (`grI1_803B8268`).
const ITEM_VEL: Vec3 = Vec3 {
    x: 0.0,
    y: 5.0,
    z: 0.0,
};

/// The tries a new ? block gets at a free block (`HATENA_APPEAR_CHECKLOOP`).
const HATENA_TRIES: i32 = 0x14;

/// The lifts' collision joints.
const LIFT_JOINTS: [i32; 2] = [0x14, 0x15];

/// `DOBJ_HIDDEN`.
const DOBJ_HIDDEN: u32 = 1;

/// `grInishie1_YakumonoParam`'s fields, by offset.
mod p {
    /// The first ? block's wait after one is broken, between two bounds (floats).
    pub const WAIT1_MAX: u32 = 0x0;
    pub const WAIT1_MIN: u32 = 0x4;
    /// The second's.
    pub const WAIT2_MAX: u32 = 0x8;
    pub const WAIT2_MIN: u32 = 0xC;
    /// The wait after every block's items before ? blocks come again (float).
    pub const ALL_WAIT: u32 = 0x10;
    /// Below this, the two waits ending a tick apart give every block an item (halfword).
    pub const ALL_WINDOW: u32 = 0x14;
    /// A ? block's blink in (halfword).
    pub const HATENA_BLINK: u32 = 0x16;
    /// A brick block's blink in (unsigned halfword).
    pub const BLOCK_BLINK: u32 = 0x18;
    /// A broken block's wait to come back, between two bounds (halfwords).
    pub const BACK_MIN: u32 = 0x1A;
    pub const BACK_MAX: u32 = 0x1C;
    /// The lifts: the farthest they part, the pull's rate and its fastest.
    pub const LIFT_MAX: u32 = 0x20;
    pub const LIFT_RATE: u32 = 0x24;
    pub const LIFT_SPEED: u32 = 0x28;
    /// The pull's damping with weight on both sides and with one, and its slowing with one.
    pub const LIFT_DAMP_BOTH: u32 = 0x2C;
    pub const LIFT_DAMP_ONE: u32 = 0x30;
    pub const LIFT_SLOW: u32 = 0x34;
    /// The lifts' drift back with no weight, the weight they ignore, and a landing's weight
    /// factor.
    pub const LIFT_DRIFT: u32 = 0x38;
    pub const LIFT_IGNORE: u32 = 0x3C;
    pub const LIFT_LANDING: u32 = 0x40;
    /// Their fall: its acceleration and fastest, the wait before they come back, and their
    /// return's speed.
    pub const FALL_ACCEL: u32 = 0x44;
    pub const FALL_SPEED: u32 = 0x48;
    pub const FALL_WAIT: u32 = 0x4C;
    pub const RETURN_SPEED: u32 = 0x50;
}

/// The fields of `grInishie1_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }
    fn hu(self, data: &Archives, off: u32) -> u16 {
        data.u16(self.0.at(off))
    }
    /// A float field taken as an integer (`fctiwz`).
    fn fi(self, data: &Archives, off: u32) -> i32 {
        fp::fctiwz(reg(self.f(data, off)))
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .inishie1_yakumono
            .expect("grinishie1.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Mushroom Kingdom's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_AAD4 | 0x801F_AAD8 | 0x801F_ABB0 | 0x801F_ABFC | 0x801F_AC00 | 0x801F_AD80 => {
            empty(r, gobj)
        }
        0x801F_AAA0 => init_back(r, gobj),
        0x801F_AADC => init_stage(r, gobj),
        0x801F_AB68 => proc_stage(r, gobj),
        0x801F_ABB4 => init_map1(r, gobj),
        0x801F_AC04 => init_hatena(r, gobj),
        0x801F_AC58 => proc_hatena(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_A90C => on_init(r),
        0x801F_A984 => on_load(r),
        0x801F_A988 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grInishie1_801FAAD4")]
#[doc(alias = "grInishie1_801FAAD8")]
#[doc(alias = "grInishie1_801FABB0")]
#[doc(alias = "grInishie1_801FABFC")]
#[doc(alias = "grInishie1_801FAC00")]
#[doc(alias = "grInishie1_801FAD80")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `setupStageCallbacks`.
#[doc(alias = "setupStageCallbacks@801FA9B4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grI1_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &INISHIE1_CALLBACKS[map_id as usize]
}

/// Mushroom Kingdom's setup, `grInishie1_801FA90C`: its parameters; the scenery and the stage;
/// camera limits and blast zones; the camera's near plane at 1 and far plane at 4000.
#[doc(alias = "grInishie1_801FA90C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.inishie1_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, MAP_STAGE, setup::SETUP_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    super::super::camera::set_near(&mut r.game.camera.cam, 1.0);
    super::super::camera::set_far(&mut r.game.camera.cam, 4000.0);
}

/// `grInishie1_801FA908` does nothing (only a demo runs it).
#[doc(alias = "grInishie1_801FA908")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grInishie1_801FA984` does nothing.
#[doc(alias = "grInishie1_801FA984")]
pub fn on_load(_r: &mut Running) {}

/// Mushroom Kingdom's start, `grInishie1_801FA988`: the enemy spawner, without enemies.
#[doc(alias = "grInishie1_801FA988")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The scenery's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@801FAAA0")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Map 1's setup, `grInishie1_801FABB4`: its animation, and no callbacks. Nothing makes a map 1
/// object.
#[doc(alias = "grInishie1_801FABB4")]
pub fn init_map1(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
}

/// The stage's setup (map 3), `grInishie1_801FAADC`: its collision and animation, the blocks,
/// the lifts, the music not yet sped up, and the joint 45's tree in the stage's class.
#[doc(alias = "grInishie1_801FAADC")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    init_blocks(r, gobj);
    init_lifts(r, gobj);
    r.game.grounds[g.0].u.inishie1().xC4 &= !0x8000;
    let j = nth_joint(r.game, gobj, 0x2D);
    material::set_tree_flags(r.game, j, 0x2000_0000);
}

/// The stage's process, `grInishie1_801FAB68`: the blocks, the lifts and the music; then the
/// wind sources age and the collision follows the model.
#[doc(alias = "grInishie1_801FAB68")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    update_blocks(r, gobj);
    update_lifts(r, gobj);
    music(r, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    update_map_coll(r.game, r.data, gobj);
}

/// A ? block's setup (map 2), `grInishie1_801FAC04`: its animation, no block yet, its blink.
#[doc(alias = "grInishie1_801FAC04")]
pub fn init_hatena(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let blink = param(r.game).h(r.data, p::HATENA_BLINK);
    let v = r.game.grounds[g.0].u.inishie1_hatena();
    v.xC4 = None;
    v.xC8 = i32::from(blink);
    v.xCC = 0;
}

/// A ? block's process, `grInishie1_801FAC58`: unless it stays hidden, it blinks in (shown on
/// odd counts); it sits where its brick block is.
#[doc(alias = "grInishie1_801FAC58")]
pub fn proc_hatena(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let Some(j) = jobj_of(r.game, gobj) else {
        return;
    };
    let v = r.game.grounds[g.0].u.inishie1_hatena();
    if v.xCC == 0 && v.xC8 > 0 {
        let show = v.xC8 & 1 != 0;
        v.xC8 -= 1;
        if show {
            jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        } else {
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        }
    }
    let on = r.game.grounds[g.0].u.inishie1_hatena().xC4;
    if on.is_some() {
        let pos = world_position(&mut r.game.objs, r.data, on, None);
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    }
}

fn blocks(game: &mut Game, g: GroundId) -> &mut Vec<Inishie1Block> {
    &mut game.grounds[g.0].u.inishie1().blocks
}

/// The block whose collision joint is `joint`, as `get_block_id` finds it; the original hangs
/// on another.
fn block_of_joint(joint: i32) -> usize {
    match BLOCKS.iter().position(|b| b.0 == joint) {
        Some(i) => i,
        None => super::super::code::unported_call("grinishie1.c:496: oioi.. (the game hangs)"),
    }
}

/// The brick blocks' setup, `grInishie1_801FAD84`: their block from the heap; each block's
/// joint, at rest at its height, in state 2, its collision joint's ceiling callback, holding
/// nothing; each one's hurtbox item; with items on (and any random kind on), two different
/// random blocks holding an item, each under a ? block that stays hidden; no ? blocks to come.
#[doc(alias = "grInishie1_801FAD84")]
pub fn init_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let addr = crate::hsd::heap::mem_alloc(0x24 * BLOCK_COUNT as u32);
    // The fields the setup never writes (`x6`, `x20`, `x22`) hold the block's leftovers
    // (docs/engine.md, a session's heap leftovers), else zero.
    let left = crate::hsd::heap::image_on()
        .then(|| {
            crate::hsd::heap::with(|h| {
                h.image
                    .leftover_bytes(r.data, addr, 0x24 * BLOCK_COUNT as u32)
                    .0
            })
        })
        .flatten();
    let half = |i: usize, o: usize| {
        left.as_ref().map_or(0, |b| {
            i16::from_be_bytes([b[0x24 * i + o], b[0x24 * i + o + 1]])
        })
    };
    let v = r.game.grounds[g.0].u.inishie1();
    v.blocks_addr = addr;
    v.blocks = (0..BLOCK_COUNT)
        .map(|i| super::vars::Inishie1Block {
            x6: half(i, 6),
            x20: half(i, 0x20),
            x22: half(i, 0x22),
            ..Default::default()
        })
        .collect();
    for (i, &(joint, part)) in BLOCKS.iter().enumerate() {
        let Some(j) = nth_joint(r.game, gobj, part) else {
            super::super::code::unported_call("grinishie1.c:541: oioi.. (the game hangs)")
        };
        let y = r.game.objs.jobjs[j].translate.y;
        let b = &mut blocks(r.game, g)[i];
        b.x2 = 2;
        b.x4 = 0;
        b.jobj2 = Some(j);
        b.hatena_gobj = None;
        b.x8 = y;
        b.xC = 0.0;
        b.x10 = 0.0;
        r.game.geometry_mut().set_callback(
            joint,
            Some(CollUser::Ground(g)),
            Some(CodeRef(BLOCK_BUMPED)),
        );
        blocks(r.game, g)[i].status = 0;
    }
    for i in 0..BLOCK_COUNT {
        let j = blocks(r.game, g)[i].jobj2;
        let item = material::stage_item(r, 8, 0, g, j, None, Some(CodeRef(BLOCK_HIT)), None);
        if let Some(item) = item {
            material::set_item_hurtbox(
                r.game,
                item,
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                6.0,
            );
            material::item_hurtbox_on(r.game, item);
            material::item_ground_or_air(r.game, item, 0);
        }
        blocks(r.game, g)[i].item_gobj = item;
    }
    if r.game.vs_item_freq() != -1 && r.game.vs_x6() == 0 {
        let any = (super::super::it::kinds::CAPSULE..super::super::it::kinds::L_GUN_RAY)
            .any(|k| super::super::it::spawn::kind_on(r.game, k));
        if any {
            let first = r.game.objs.rng.below(BLOCK_COUNT as i32);
            let second = loop {
                let k = r.game.objs.rng.below(BLOCK_COUNT as i32);
                if k != first {
                    break k;
                }
            };
            for (k, status) in [(first, 1), (second, 2)] {
                blocks(r.game, g)[k as usize].status = status;
                add_hatena(r, gobj, k as usize, SETUP_HATENA_FRAME);
                let h = blocks(r.game, g)[k as usize]
                    .hatena_gobj
                    .expect("a ? block just made");
                let hg = ground_of(r.game, h);
                r.game.grounds[hg.0].u.inishie1_hatena().xCC = 1;
            }
        }
    }
    let v = r.game.grounds[g.0].u.inishie1();
    v.xC6 = 0;
    v.xC8 = 0;
    v.xCA = -1;
    v.xCC = -1;
    v.xD8 = 0;
}

/// A block broken gives up what it holds, `grInishie1_801FB0AC`: an item from a ? block's block
/// pops out and its ? block goes; the next ? block (the first for an item of status 1, the
/// second for 2) is a random wait away; after every block's items, once none is left, both
/// are, and ? blocks wait a while longer. A block holding nothing has no ? block (asserted).
#[doc(alias = "grInishie1_801FB0AC")]
pub fn give_up(r: &mut Running, gobj: GObjId, ix: usize) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let b = &blocks(r.game, g)[ix];
    let (status, j) = (b.status, b.jobj2);
    match status {
        1..=3 => {
            pop_item(r, j);
            blocks(r.game, g)[ix].status = 0;
            remove_hatena(r, gobj, ix);
            let rng = &mut r.game.objs.rng;
            if status == 1 {
                let w =
                    grlib::rand_range(rng, p.fi(r.data, p::WAIT1_MIN), p.fi(r.data, p::WAIT1_MAX));
                r.game.grounds[g.0].u.inishie1().xC6 = w as i16;
            } else if status == 2 {
                let w =
                    grlib::rand_range(rng, p.fi(r.data, p::WAIT2_MIN), p.fi(r.data, p::WAIT2_MAX));
                r.game.grounds[g.0].u.inishie1().xC8 = w as i16;
            } else {
                if !blocks(r.game, g).iter().any(|b| b.status == 3) {
                    let rng = &mut r.game.objs.rng;
                    let w1 = grlib::rand_range(
                        rng,
                        p.fi(r.data, p::WAIT1_MIN),
                        p.fi(r.data, p::WAIT1_MAX),
                    );
                    r.game.grounds[g.0].u.inishie1().xC6 = w1 as i16;
                    let rng = &mut r.game.objs.rng;
                    let w2 = grlib::rand_range(
                        rng,
                        p.fi(r.data, p::WAIT2_MIN),
                        p.fi(r.data, p::WAIT2_MAX),
                    );
                    r.game.grounds[g.0].u.inishie1().xC8 = w2 as i16;
                }
                r.game.grounds[g.0].u.inishie1().xD8 = p.fi(r.data, p::ALL_WAIT);
            }
        }
        _ => {
            assert!(
                blocks(r.game, g)[ix].hatena_gobj.is_none(),
                "grinishie1.c:669: 0"
            );
        }
    }
}

/// A ? block appearing over a free block for the item `status`, as `HATENA_APPEAR_CHECKLOOP`
/// does when its wait runs out: up to 20 random tries for a block other than the last that
/// holds nothing (the original asserts one is found).
fn appear(r: &mut Running, gobj: GObjId, g: GroundId, status: i16, line: u32) {
    let last = if status == 1 {
        r.game.grounds[g.0].u.inishie1().xCA
    } else {
        r.game.grounds[g.0].u.inishie1().xCC
    };
    let mut index = 0;
    let mut i = 0;
    while i < HATENA_TRIES {
        let result = r.game.objs.rng.below(BLOCK_COUNT as i32);
        index = result;
        if result != i32::from(last) && blocks(r.game, g)[result as usize].status == 0 {
            break;
        }
        i += 1;
    }
    assert!(
        i != HATENA_TRIES,
        "grinishie1.c:{line}: i!=HATENA_APPEAR_CHECKLOOP"
    );
    let v = r.game.grounds[g.0].u.inishie1();
    if status == 1 {
        v.xCA = index as i16;
    } else {
        v.xCC = index as i16;
    }
    blocks(r.game, g)[index as usize].status = status;
    add_hatena(r, gobj, index as usize, PROC_HATENA_FRAME);
    blocks(r.game, g)[index as usize].x20 = 0;
}

/// Sets a block's display objects' hidden flag (the inline `grInishie1_UpdateDObjFlags`).
fn block_dobjs(game: &mut Game, j: Option<JObjId>, hidden: bool) {
    let mut d = jobj::get_dobj(&game.objs.jobjs, j);
    while let Some(x) = d {
        if hidden {
            dobj::set_flags(&mut game.objs.dobjs, Some(x), DOBJ_HIDDEN);
        } else {
            dobj::clear_flags(&mut game.objs.dobjs, Some(x), DOBJ_HIDDEN);
        }
        d = game.objs.dobjs[x].next;
    }
}

/// The blocks' process, `grInishie1_801FB3F0`. The wait after every block's items counts down;
/// with none left, the two ? blocks' waits ending a tick apart (one at 1, the other between 0
/// and the window) give every block an item and a ? block instead; otherwise each wait that
/// runs out brings its ? block. Then each block: at rest (2) its count runs down; bumped (3)
/// it gives up its item and rises (4) 5 and back at 1.2 a tick, then rests 10 ticks; broken
/// (0), once its wait is over and nothing is within 15 of it, it shows again with its hurtbox
/// and blinks in (1, its ? block too), hiding its ? block meanwhile.
#[doc(alias = "grInishie1_801FB3F0")]
pub fn update_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let window = i32::from(p.h(r.data, p::ALL_WINDOW));
    let v = r.game.grounds[g.0].u.inishie1();
    if v.xD8 > 0 {
        v.xD8 -= 1;
    }
    let (c6, c8) = (i32::from(v.xC6), i32::from(v.xC8));
    if v.xD8 == 0 && ((c6 == 1 && c8 > 0 && c8 < window) || (c8 == 1 && c6 > 0 && c6 < window)) {
        v.xC8 = 0;
        v.xC6 = 0;
        for i in 0..BLOCK_COUNT {
            blocks(r.game, g)[i].status = 3;
            add_hatena(r, gobj, i, PROC_HATENA_FRAME);
            blocks(r.game, g)[i].x20 = 0;
        }
    } else {
        let v = r.game.grounds[g.0].u.inishie1();
        if v.xC6 > 0 {
            v.xC6 -= 1;
            if v.xC6 == 0 {
                appear(r, gobj, g, 1, 0x2D0);
            }
        }
        let v = r.game.grounds[g.0].u.inishie1();
        if v.xC8 > 0 {
            v.xC8 -= 1;
            if v.xC8 == 0 {
                appear(r, gobj, g, 2, 0x2EB);
            }
        }
    }
    for i in 0..BLOCK_COUNT {
        match blocks(r.game, g)[i].x2 {
            2 => {
                let b = &mut blocks(r.game, g)[i];
                if b.x4 != 0 {
                    b.x4 -= 1;
                }
            }
            x2 @ (3 | 4) => {
                if x2 == 3 {
                    blocks(r.game, g)[i].x10 = 1.2;
                    give_up(r, gobj, i);
                    blocks(r.game, g)[i].x2 = 4;
                }
                let b = &mut blocks(r.game, g)[i];
                b.xC = mem(fp::fadds(reg(b.xC), reg(b.x10)));
                if reg(b.xC) > 5.0 {
                    b.x10 = -1.2;
                } else if reg(b.xC) < 0.0 {
                    b.xC = 0.0;
                    b.x2 = 2;
                    b.x4 = 10;
                }
                let (j, y) = (b.jobj2, mem(fp::fadds(reg(b.x8), reg(b.xC))));
                let j = j.expect("a block's joint");
                jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
            }
            0 => {
                if blocks(r.game, g)[i].x20 != 0 {
                    blocks(r.game, g)[i].x20 -= 1;
                } else {
                    let j = blocks(r.game, g)[i].jobj2;
                    let pos = world_position(&mut r.game.objs, r.data, j, None);
                    if !grlib::point_near_object(r.game, pos, 15.0) {
                        block_dobjs(r.game, j, false);
                        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                        let item = blocks(r.game, g)[i].item_gobj;
                        material::item_hurtbox_on(r.game, item.expect("a block's item"));
                        let blink = p.hu(r.data, p::BLOCK_BLINK) as i16;
                        let b = &mut blocks(r.game, g)[i];
                        b.x2 = 1;
                        b.x22 = blink;
                    }
                }
                if let Some(h) = blocks(r.game, g)[i].hatena_gobj {
                    let hj = jobj_of(r.game, h);
                    let flags = r.game.objs.jobjs[hj.expect("a ? block's model")].flags;
                    if flags & jobj::HIDDEN == 0 {
                        jobj::set_flags_all(&mut r.game.objs.jobjs, hj, jobj::HIDDEN);
                    }
                }
            }
            1 => {
                let b = &blocks(r.game, g)[i];
                let (n, j, h) = (b.x22, b.jobj2, b.hatena_gobj);
                if n > 0 {
                    match h {
                        Some(h) => {
                            let hj = jobj_of(r.game, h);
                            if n & 1 != 0 {
                                jobj::clear_flags_all(&mut r.game.objs.jobjs, hj, jobj::HIDDEN);
                            } else {
                                jobj::set_flags_all(&mut r.game.objs.jobjs, hj, jobj::HIDDEN);
                            }
                            block_dobjs(r.game, j, false);
                        }
                        None => block_dobjs(r.game, j, n & 1 == 0),
                    }
                    blocks(r.game, g)[i].x22 -= 1;
                } else {
                    let b = &mut blocks(r.game, g)[i];
                    b.x2 = 2;
                    b.x4 = 0;
                }
            }
            _ => {}
        }
    }
}

/// Pops a random item out of a block at joint `j`, 5 above it, straight up,
/// `grInishie1_801FBA34`.
#[doc(alias = "grInishie1_801FBA34")]
pub fn pop_item(r: &mut Running, j: Option<JObjId>) {
    let mut pos = world_position(&mut r.game.objs, r.data, j, None);
    pos.y = mem(fp::fadds(reg(pos.y), 5.0));
    super::super::it::drop::drop_random(r, pos, Some(ITEM_VEL));
}

/// Puts a ? block over block `ix`, `grInishie1_801FBAA0`: where the block is, following it,
/// hidden for good if the block is broken or blinking in; the block's own display objects
/// hide. A block already under one is asserted against.
#[doc(alias = "grInishie1_801FBAA0")]
pub fn add_hatena(r: &mut Running, gobj: GObjId, ix: usize, frame: u32) {
    let g = ground_of(r.game, gobj);
    let Some(hatena) = create(r, MAP_HATENA, frame) else {
        return;
    };
    let hg = ground_of(r.game, hatena);
    assert!(
        blocks(r.game, g)[ix].hatena_gobj.is_none(),
        "grinishie1.c:926: !mapgp->u.map.block[ix].hatena_gobj"
    );
    blocks(r.game, g)[ix].hatena_gobj = Some(hatena);
    let (j, x2) = (blocks(r.game, g)[ix].jobj2, blocks(r.game, g)[ix].x2);
    let pos = world_position(&mut r.game.objs, r.data, j, None);
    let hj = jobj_of(r.game, hatena).expect("a ? block's model");
    jobj::set_translate(&mut r.game.objs.jobjs, hj, pos);
    let v = r.game.grounds[hg.0].u.inishie1_hatena();
    v.xC4 = j;
    if x2 == 0 || x2 == 1 {
        v.xCC = 1;
    }
    block_dobjs(r.game, j, true);
}

/// Takes the ? block off block `ix`, `grInishie1_801FBC4C`: the block shows again.
#[doc(alias = "grInishie1_801FBC4C")]
pub fn remove_hatena(r: &mut Running, gobj: GObjId, ix: usize) {
    let g = ground_of(r.game, gobj);
    let j = blocks(r.game, g)[ix].jobj2;
    block_dobjs(r.game, j, false);
    let h = blocks(r.game, g)[ix].hatena_gobj;
    setup::destroy(r, h);
    blocks(r.game, g)[ix].hatena_gobj = None;
}

/// Breaks block `ix`, `grInishie1_801FBCEC`: broken (state 0) for a random while, its first
/// display object hidden and its hurtbox off; a ? block's item still in it (after its block
/// gave it up, none is) pops out and the ? block goes; it hides, and an effect at its item, a
/// sound and a small quake there mark it.
#[doc(alias = "grInishie1_801FBCEC")]
pub fn break_block(r: &mut Running, gobj: GObjId, ix: usize) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    blocks(r.game, g)[ix].x2 = 0;
    let (lo, hi) = (
        i32::from(p.h(r.data, p::BACK_MIN)),
        i32::from(p.h(r.data, p::BACK_MAX)),
    );
    let wait = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
    blocks(r.game, g)[ix].x20 = wait as i16;
    let b = &blocks(r.game, g)[ix];
    let (j, item, status) = (b.jobj2, b.item_gobj, b.status);
    let d = jobj::get_dobj(&r.game.objs.jobjs, j);
    dobj::set_flags(&mut r.game.objs.dobjs, d, DOBJ_HIDDEN);
    let item = item.expect("a block's item");
    material::item_hurtbox_off(r.game, item);
    if status == 1 || (status.wrapping_sub(2) as u16) <= 1 {
        let h = blocks(r.game, g)[ix].hatena_gobj;
        setup::destroy(r, h);
        blocks(r.game, g)[ix].hatena_gobj = None;
        pop_item(r, j);
    }
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let pos = super::super::it::lib26b1::position(r.game, item);
    super::super::ef::spawn(r, 0x442, Some(item), &[super::super::ef::Arg::Vec(pos)]);
    r.stage_sound_on_track(0x136, 0xBA);
    super::super::camera::request_quake(r, 2, Some(pos));
}

/// The bumps the blocks' ceilings took in a collision pass, which the pass's caller gives
/// once mpcoll returns (the joint callback has no access to the rest of the game). Each is the
/// callback's last act in its pass, and nothing a pass reads changes with a break (the block's
/// collision stays, and a pass draws no random numbers, makes no effects and plays no sounds),
/// so they happen in the original's order.
#[derive(Debug, Default)]
pub struct Bumps(Vec<u8>);

impl Bumps {
    /// A brick block's ceiling touched, `fn_801FBEB8`: a block hit from below fast enough
    /// (more than 0.7 a tick) gives up its item and breaks.
    #[doc(alias = "fn_801FBEB8")]
    pub fn bumped(&mut self, joint_id: i32, dy: f32) {
        let id = block_of_joint(joint_id);
        let abs = if reg(dy) < 0.0 {
            fp::fneg(reg(dy))
        } else {
            reg(dy)
        };
        if abs > 0.7 {
            self.0.push(id as u8);
        }
    }

    /// Gives the bumps, in order.
    pub fn apply(self, r: &mut Running) {
        for id in self.0 {
            let gobj = map_gobj(r.game, MAP_STAGE).expect("Ground_GetMapGObj(3)");
            give_up(r, gobj, id.into());
            break_block(r, gobj, id.into());
        }
    }
}

/// A hit a brick block's item took, `fn_801FBF6C`: the block whose joint the item follows gives
/// up its item and breaks.
#[doc(alias = "fn_801FBF6C")]
pub fn block_hit(r: &mut Running, item: GObjId, g: GroundId) {
    let anchor = super::super::it::kinds::yaku::vars(r.game, item)
        .anchor
        .get();
    let joint = match anchor {
        super::super::it::kinds::yaku::Anchor::Joint(j) => j,
        _ => None,
    };
    let Some(i) = blocks(r.game, g)
        .iter()
        .position(|b| b.jobj2.is_some() && b.jobj2 == joint)
    else {
        return;
    };
    let gobj = map_gobj(r.game, MAP_STAGE);
    let gobj = gobj.expect("grinishie1.c:1061: map_gobj");
    give_up(r, gobj, i);
    break_block(r, gobj, i);
}

/// The lifts' setup, `grInishie1_801FC018`: balanced and still, their arms (joints 26 and 28)
/// and platforms (the arms' children), their height at rest, and their collision joints'
/// callbacks.
#[doc(alias = "grInishie1_801FC018")]
pub fn init_lifts(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let a = nth_joint(r.game, gobj, 0x1A);
    let b = nth_joint(r.game, gobj, 0x1C);
    let y = r.game.objs.jobjs[a.expect("the lift's arm")].translate.y;
    let child = |r: &Running, j: Option<JObjId>| j.and_then(|j| r.game.objs.jobjs[j].child);
    let (ca, cb) = (child(r, a), child(r, b));
    let v = r.game.grounds[g.0].u.inishie1();
    v.xEE = 0;
    v.xE0 = 0.0;
    v.xE4 = 0.0;
    v.xE8 = 0;
    v.xEA = 0;
    v.xF0 = 0.0;
    v.xF4 = 0.0;
    v.x108 = a;
    v.x10C = b;
    v.xFC = y;
    v.x100 = ca;
    v.x104 = cb;
    let geo = r.game.geometry_mut();
    for joint in [0x15, 0x14] {
        geo.set_ledge_callback(
            joint,
            Some(CollUser::Ground(g)),
            Some(CodeRef(LIFT_TOUCHED)),
        );
    }
}

/// Puts the lifts' arms at their offset from rest, one up and one down.
fn place_arms(game: &mut Game, g: GroundId) {
    let v = game.grounds[g.0].u.inishie1();
    let (a, b) = (v.x108, v.x10C);
    let up = mem(fp::fadds(reg(v.xFC), reg(v.xF0)));
    let down = mem(fp::fsubs(reg(v.xFC), reg(v.xF0)));
    for (j, y) in [(a, up), (b, down)] {
        let j = j.expect("the lift's arm");
        jobj::set_srt(&mut game.objs.jobjs, j, |o| o.translate.y = y);
    }
}

/// The lifts balancing, `grInishie1_801FC110`. The weights past what they ignore (capped at
/// 100,000) pull: with none, they drift back level; else the difference speeds the pull (at
/// most the fastest), damped more with weight on both sides, and with one it also slows, and
/// they move by it. Parted farther than the most, they stop there and fall; the arms follow,
/// and this tick's weights start over.
#[doc(alias = "grInishie1_801FC110")]
pub fn balance(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let pf = |off| reg(p.f(r.data, off));
    let (ignore, drift) = (pf(p::LIFT_IGNORE), pf(p::LIFT_DRIFT));
    let (rate, speed) = (pf(p::LIFT_RATE), pf(p::LIFT_SPEED));
    let (both, one, slow) = (
        pf(p::LIFT_DAMP_BOTH),
        pf(p::LIFT_DAMP_ONE),
        pf(p::LIFT_SLOW),
    );
    let max = pf(p::LIFT_MAX);
    let v = r.game.grounds[g.0].u.inishie1();
    if reg(v.xE0) > 100000.0 || reg(v.xE4) > 100000.0 {
        v.xE4 = 100000.0;
        v.xE0 = 100000.0;
    }
    let mut lo = fp::fsubs(reg(v.xE0), ignore);
    if lo < 0.0 {
        lo = 0.0;
    }
    let mut hi = fp::fsubs(reg(v.xE4), ignore);
    if hi < 0.0 {
        hi = 0.0;
    }
    if lo == 0.0 && hi == 0.0 {
        let f0 = reg(v.xF0);
        let abs = if f0 < 0.0 { fp::fneg(f0) } else { f0 };
        if abs > 0.0 {
            if f0 < 0.0 {
                v.xF0 = mem(fp::fadds(f0, drift));
                if reg(v.xF0) > 0.0 {
                    v.xF0 = 0.0;
                }
            } else {
                v.xF0 = mem(fp::fsubs(f0, drift));
                if reg(v.xF0) < 0.0 {
                    v.xF0 = 0.0;
                }
            }
        }
        v.xF4 = 0.0;
    } else {
        let accel = fp::fsubs(hi, lo);
        v.xF4 = mem(fp::fmadds(accel, rate, reg(v.xF4)));
        if reg(v.xF4) > speed {
            v.xF4 = mem(speed);
        } else if reg(v.xF4) < fp::fneg(speed) {
            v.xF4 = mem(fp::fneg(speed));
        }
        if lo != 0.0 && hi != 0.0 && reg(v.xF4) != 0.0 {
            v.xF4 = mem(fp::fmuls(reg(v.xF4), both));
        } else {
            v.xF4 = mem(fp::fmuls(reg(v.xF4), one));
            if reg(v.xF4) > 0.0 {
                v.xF4 = mem(fp::fsubs(reg(v.xF4), slow));
                if reg(v.xF4) < 0.0 {
                    v.xF4 = 0.0;
                }
            } else if reg(v.xF4) < 0.0 {
                v.xF4 = mem(fp::fadds(reg(v.xF4), slow));
                if reg(v.xF4) > 0.0 {
                    v.xF4 = 0.0;
                }
            }
        }
        v.xF0 = mem(fp::fadds(reg(v.xF0), reg(v.xF4)));
    }
    let f0 = reg(v.xF0);
    let abs = if f0 < 0.0 { fp::fneg(f0) } else { f0 };
    if abs > max {
        v.xF0 = mem(if f0 < 0.0 { fp::fneg(max) } else { max });
        v.xEE = 1;
        v.xF4 = 0.0;
    }
    place_arms(r.game, g);
    let v = r.game.grounds[g.0].u.inishie1();
    v.xE8 = 0;
    v.xEA = 0;
    v.xE0 = 0.0;
    v.xE4 = 0.0;
}

/// The lifts coming back level, `grInishie1_801FC4A0`: their offset shrinks by the return's
/// speed until it crosses 0, where they are balanced and still again; the arms follow.
#[doc(alias = "grInishie1_801FC4A0")]
pub fn level_out(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let back = reg(param(r.game).f(r.data, p::RETURN_SPEED));
    let v = r.game.grounds[g.0].u.inishie1();
    let mut reached = false;
    if reg(v.xF0) != 0.0 {
        if reg(v.xF0) < 0.0 {
            v.xF0 = mem(fp::fadds(reg(v.xF0), back));
            if reg(v.xF0) >= 0.0 {
                reached = true;
            }
        } else {
            v.xF0 = mem(fp::fsubs(reg(v.xF0), back));
            if reg(v.xF0) <= 0.0 {
                reached = true;
            }
        }
    }
    if reached {
        v.xF0 = 0.0;
        v.xF4 = 0.0;
        v.xE0 = 0.0;
        v.xE4 = 0.0;
        v.xEE = 0;
    }
    place_arms(r.game, g);
}

/// The lifts, `grInishie1_801FC664`. Balancing (state 0); falling (1), both platforms dropping
/// ever faster (at most the fastest) until both are 30 below the bottom blast zone; gone (2)
/// for a while, then back on their arms, their collision put back where they are (3), coming
/// back level.
#[doc(alias = "grInishie1_801FC664")]
pub fn update_lifts(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    match r.game.grounds[g.0].u.inishie1().xEE {
        0 => balance(r, gobj),
        1 => {
            let (accel, fastest) = (
                reg(p.f(r.data, p::FALL_ACCEL)),
                reg(p.f(r.data, p::FALL_SPEED)),
            );
            let v = r.game.grounds[g.0].u.inishie1();
            v.xF4 = mem(fp::fadds(reg(v.xF4), accel));
            if reg(v.xF4) > fastest {
                v.xF4 = mem(fastest);
            }
            let (a, b, down) = (v.x100, v.x104, mem(fp::fneg(reg(v.xF4))));
            for j in [a, b] {
                let j = j.expect("a lift's platform");
                jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
                    o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(down)))
                });
            }
            let bottom = fp::fadds(-30.0, reg(super::stage::blast_bottom(r.game)));
            let pa = world_position(&mut r.game.objs, r.data, a, None);
            let pb = world_position(&mut r.game.objs, r.data, b, None);
            if reg(pa.y) < bottom && reg(pb.y) < bottom {
                let wait = p.fi(r.data, p::FALL_WAIT) as i16;
                let v = r.game.grounds[g.0].u.inishie1();
                v.xEE = 2;
                v.xF4 = 0.0;
                v.xEC = wait;
            }
        }
        2 => {
            let v = r.game.grounds[g.0].u.inishie1();
            v.xEC = v.xEC.wrapping_sub(1);
            if v.xEC == 0 {
                v.xEE = 3;
                let (a, b) = (v.x100, v.x104);
                for j in [a, b] {
                    let j = j.expect("a lift's platform");
                    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = 0.0);
                }
                for joint in LIFT_JOINTS {
                    super::ground::update_coll_joint(r.game, r.data, joint);
                }
                for joint in LIFT_JOINTS {
                    r.game.geometry_mut().settle_vertices(joint);
                }
            }
        }
        3 => level_out(r, gobj),
        _ => {}
    }
}

/// A lift's collision joint touched, `fn_801FC9AC`: each counts its touches and adds the
/// toucher's weight (its `x50`), a landing's scaled by the parameters' factor.
#[doc(alias = "fn_801FC9AC")]
pub fn lift_touched(
    grounds: &mut crate::hsd::pool::Pool<super::super::structs::Ground>,
    param: Option<DataRef>,
    data: &Archives,
    g: GroundId,
    joint_id: i32,
    x50: i32,
    kind: i32,
) {
    let landing = reg(data.f32(
        param
            .expect("grinishie1.c: yakumono_param")
            .at(p::LIFT_LANDING),
    ));
    let v = grounds[g.0].u.inishie1();
    let w = fp::frsp(f64::from(x50));
    let add = |acc: f32| {
        if kind == 1 {
            mem(fp::fmadds(w, landing, reg(acc)))
        } else {
            mem(fp::fadds(reg(acc), w))
        }
    };
    if joint_id == 0x14 {
        v.xE8 = v.xE8.wrapping_add(1);
        v.xE0 = add(v.xE0);
    } else if joint_id == 0x15 {
        v.xEA = v.xEA.wrapping_add(1);
        v.xE4 = add(v.xE4);
    }
}

/// The music under 20 seconds left, `grInishie1_801FCB10`, once: the stage's song (41 or 43)
/// changes to its fast version (42 or 44; another song stops); a match without a time limit
/// never changes it.
#[doc(alias = "grInishie1_801FCB10")]
pub fn music(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.inishie1().xC4 & 0x8000 != 0 {
        return;
    }
    match r.game.vs_match_timer() {
        Some(left) => {
            if left < 20 {
                let bgm = fast_song(super::ground::x98(r.game)).unwrap_or(-1);
                r.lb_play_music(bgm);
                r.game.grounds[g.0].u.inishie1().xC4 |= 0x8000;
            }
        }
        None => r.game.grounds[g.0].u.inishie1().xC4 |= 0x8000,
    }
}

/// The fast version of the stage's song `bgm` that its last 20 seconds play, if it has one.
pub fn fast_song(bgm: i32) -> Option<i32> {
    match bgm {
        0x29 => Some(0x2A),
        0x2B => Some(0x2C),
        _ => None,
    }
}

/// The stage's shadow check, `grInishie1_801FCBB8`: every joint takes the shadows.
#[doc(alias = "grInishie1_801FCBB8")]
pub fn check_shadow_render() -> bool {
    true
}
