//! The events' trophy stages (`melee/gr/grfigure1.c`, `grfigure2.c` and `grfigure3.c`, ground
//! kinds 68 to 70, `/GrEF1.dat` to `/GrEF3.dat`), which only Event Match's events reach (stage
//! kinds 215, 227 and 248). The three are copies of one another: an animated model (map 0), the
//! collision with the stage's lights and fog (map 1, its joints following the model), and a
//! model with its own collision and animation (map 2), with no hazard, no line dynamics, every
//! shadow drawn and the enemy spawner started with no table.
//!
//! No session reaches them before Event Match (milestone 6's task 21), whose sessions check
//! them, so every function is held unaliased until then.

use melee_data::archive::DataRef;

use super::super::state::Running;
use super::data::{FIGURE1_CALLBACKS, FIGURE2_CALLBACKS, FIGURE3_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, update_map_coll};
use super::{anime, setup, zako};
use crate::hsd::gobj::GObjId;

/// The three stages' objects' functions, by `grkind - GR_KIND_FIGURE1`.
const CALLBACKS: [&[StageCallbacks; 3]; 3] =
    [&FIGURE1_CALLBACKS, &FIGURE2_CALLBACKS, &FIGURE3_CALLBACKS];

/// The stages' line-touch dynamics (`grFigure1_OnTouchLine`, `grFigure2_8020E4F4`,
/// `grFigure3_8020E788`), which answer none.
pub const ON_TOUCH_LINE: [u32; 3] = [0x8020_E260, 0x8020_E4F4, 0x8020_E788];

/// The stages' shadow checks (`grFigure1_OnCheckShadowRender`, `grFigure2_8020E4FC`,
/// `grFigure3_8020E790`), which draw every shadow.
pub const CHECK_SHADOW_RENDER: [u32; 3] = [0x8020_E268, 0x8020_E4FC, 0x8020_E790];

/// The stages' demo setups (`grFigure1_OnDemoInit`, `grFigure1_8020E270`, grfigure2.c's, and
/// `grFigure3_8020E504`), which do nothing.
pub const ON_DEMO_INIT: [u32; 3] = [0x8020_DFDC, 0x8020_E270, 0x8020_E504];

/// A stage object's process or callback, by address: the trophy stages'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_E19C | 0x8020_E1A0 | 0x8020_E1FC | 0x8020_E258 | 0x8020_E25C => empty(r, gobj),
        0x8020_E430 | 0x8020_E434 | 0x8020_E490 | 0x8020_E4EC | 0x8020_E4F0 => empty(r, gobj),
        0x8020_E6C4 | 0x8020_E6C8 | 0x8020_E724 | 0x8020_E780 | 0x8020_E784 => empty(r, gobj),
        0x8020_E168 | 0x8020_E3FC | 0x8020_E690 => init_model(r, gobj),
        0x8020_E1A4 | 0x8020_E438 | 0x8020_E6CC => init_coll(r, gobj),
        0x8020_E1D8 | 0x8020_E46C | 0x8020_E700 => proc_coll(r, gobj),
        0x8020_E200 | 0x8020_E494 | 0x8020_E728 => init_coll_and_model(r, gobj),
        _ => return false,
    }
    true
}

/// The stages' hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_DFE0 => on_init(r, 0),
        0x8020_E274 => on_init(r, 1),
        0x8020_E508 => on_init(r, 2),
        0x8020_E050 | 0x8020_E2E4 | 0x8020_E578 => on_load(r),
        0x8020_E054 | 0x8020_E2E8 | 0x8020_E57C => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing: the two models' processes,
/// and the destroy callbacks (`Ground::x1C_callback`), which only a stage's own destroying of an
/// object runs (`Ground_801C4A08`), as these stages never do.
// Ports grFigure1_8020E19C; aliased once a check reaches it.
// Ports grFigure1_8020E1A0; aliased once a check reaches it.
// Ports grFigure1_8020E1FC; aliased once a check reaches it.
// Ports grFigure1_8020E258; aliased once a check reaches it.
// Ports grFigure1_8020E25C; aliased once a check reaches it.
// Ports grFigure2_8020E430; aliased once a check reaches it.
// Ports grFigure2_8020E434; aliased once a check reaches it.
// Ports grFigure2_8020E490; aliased once a check reaches it.
// Ports grFigure2_8020E4EC; aliased once a check reaches it.
// Ports grFigure2_8020E4F0; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E6C4")]
// Ports grFigure3_8020E6C8; aliased once a check reaches it.
// Ports grFigure3_8020E724; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E780")]
// Ports grFigure3_8020E784; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The stages' line-touch dynamics: none.
// Ports grFigure1_OnTouchLine; aliased once a check reaches it.
// Ports grFigure2_8020E4F4; aliased once a check reaches it.
// Ports grFigure3_8020E788; aliased once a check reaches it.
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stages' shadow checks: every shadow is drawn.
// Ports grFigure1_OnCheckShadowRender; aliased once a check reaches it.
// Ports grFigure2_8020E4FC; aliased once a check reaches it.
// Ports grFigure3_8020E790; aliased once a check reaches it.
pub fn check_shadow_render() -> bool {
    true
}

/// The stages' demo setups do nothing (only a cutscene's `Stage_8022532C` runs one).
// Ports grFigure1_OnDemoInit; aliased once a check reaches it.
// Ports grFigure1_8020E270; aliased once a check reaches it.
// Ports grFigure3_8020E504; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// Creates one of a stage's objects and sets it up (`grFigure1_8020E080`, `grFigure2_8020E314`,
/// `grFigure3_8020E5A8`), its scale joint's descriptor at `frame` on the stack.
// Ports grFigure1_8020E080; aliased once a check reaches it.
// Ports grFigure2_8020E314; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E5A8")]
pub fn create(r: &mut Running, stage: usize, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, &CALLBACKS[stage][map_id as usize], frame)
}

/// A stage's setup (`grFigure1_OnInit`, `grFigure2_8020E274`, `grFigure3_8020E508`): its
/// parameters, the stage's `unk8C` flags (b4 off, b5 on), maps 0, 2 and 1 in that order, then
/// the camera limits and blast zones measured.
// Ports grFigure1_OnInit; aliased once a check reaches it.
// Ports grFigure2_8020E274; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E508")]
pub fn on_init(r: &mut Running, stage: usize) {
    r.game.gr.figure_yakumono[stage] = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 2, 1] {
        create(r, stage, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// A stage's load does nothing (`grFigure1_OnLoad`, `grFigure2_OnLoad`, `grFigure3_OnLoad`).
// Ports grFigure1_OnLoad; aliased once a check reaches it.
// Ports grFigure2_OnLoad; aliased once a check reaches it.
#[doc(alias = "grFigure3_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// A stage's start (`grFigure1_OnStart`, `grFigure2_OnStart`, `grFigure3_OnStart`): the enemy
/// spawner, with no table.
// Ports grFigure1_OnStart; aliased once a check reaches it.
// Ports grFigure2_OnStart; aliased once a check reaches it.
#[doc(alias = "grFigure3_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(r: &Running, gobj: GObjId) -> i32 {
    r.game.grounds[ground_of(r.game, gobj).0].map_id
}

/// The model's setup (map 0, each stage's `stageGObj0_OnInit`): its animation.
// Ports stageGObj0_OnInit@8020E168; aliased once a check reaches it.
// Ports stageGObj0_OnInit@8020E3FC; aliased once a check reaches it.
#[doc(alias = "stageGObj0_OnInit@8020E690")]
pub fn init_model(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The collision's setup (map 1: `grFigure1_8020E1A4`, `grFigure2_8020E438`,
/// `grFigure3_8020E6CC`): the map's collision joints bound to its model.
// Ports grFigure1_8020E1A4; aliased once a check reaches it.
// Ports grFigure2_8020E438; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E6CC")]
pub fn init_coll(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
}

/// The collision's process (map 1: `grFigure1_8020E1D8`, `grFigure2_8020E46C`,
/// `grFigure3_8020E700`): the collision follows the model, and the wind sources age.
// Ports grFigure1_8020E1D8; aliased once a check reaches it.
// Ports grFigure2_8020E46C; aliased once a check reaches it.
#[doc(alias = "grFigure3_8020E700")]
pub fn proc_coll(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    super::ground::quake_list_tick(r.game);
}

/// The second model's setup (map 2, each stage's `stageGObj2_OnInit`): its collision and
/// animation (the inline `Ground_InitMapCollAndAnim`).
// Ports stageGObj2_OnInit@8020E200; aliased once a check reaches it.
// Ports stageGObj2_OnInit@8020E494; aliased once a check reaches it.
#[doc(alias = "stageGObj2_OnInit@8020E728")]
pub fn init_coll_and_model(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
}
