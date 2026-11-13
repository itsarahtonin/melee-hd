//! The Test stage (`melee/gr/grtest.c`, ground kinds 1 and 0x23, `/GrTe.dat`), which only the
//! debug menu's Versus page reaches (stage kind 1, "TEST"), at a raised debug level: an
//! animated model (map 0), a model at 80 times its size (map 1), and a collision whose model
//! the second and third controllers turn (map 2, with the stage's lights and fog): A on the
//! second shows or hides its joint 16, A on the third its joint 17, and X and Y held on the
//! second turn its joint 11 a step a frame either way up to 70 degrees.

use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::data::{StageCallbacks, TEST_CALLBACKS};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;

/// The stage's setup (`grTest_80206E30`).
pub const ON_INIT: u32 = 0x8020_6E30;
/// Its demo setup (`grTest_80206E2C`), which does nothing.
pub const ON_DEMO_INIT: u32 = 0x8020_6E2C;
/// Its load (`grTest_UnkStage0_OnLoad`), which does nothing.
pub const ON_LOAD: u32 = 0x8020_7014;
/// Its start (`grTest_UnkStage0_OnStart`).
pub const ON_START: u32 = 0x8020_7018;
/// Its `callback4` (`grTest_8020703C`), which answers false and nothing calls.
pub const CALLBACK4: u32 = 0x8020_703C;
/// Its line-touch dynamics (`grTest_8020740C`), which answer none.
pub const ON_TOUCH_LINE: u32 = 0x8020_740C;
/// Its shadow check (`grTest_80207414`), which draws every shadow.
pub const CHECK_SHADOW_RENDER: u32 = 0x8020_7414;
/// Map 2's collision update (`stageGObj2_X8Callback`), its model's `x8_callback`.
const X8_CALLBACK: u32 = 0x8020_73B0;

/// The frame `grTest_80206E30` (0x20 bytes) puts its objects' scale joints' descriptors under.
const INIT_FRAME: u32 = setup::init_frame(0x20);

/// `PAD_BUTTON_A`, `PAD_BUTTON_X` and `PAD_BUTTON_Y`.
const PAD_A: u32 = 0x100;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;

/// A stage object's process or callback, by address.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_7130 | 0x8020_73D0 => init_model(r, gobj),
        0x8020_716C => init_coll_and_model(r, gobj),
        0x8020_7164 | 0x8020_7404 => empty(r, gobj),
        0x8020_71C4 => proc_turn(r, gobj),
        X8_CALLBACK => update_coll(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        ON_INIT => on_init(r),
        ON_LOAD => on_load(r),
        ON_START => on_start(r),
        _ => return false,
    }
    true
}

/// The processes of maps 0 and 1 (`grTest_80207164`, `grTest_80207404`), which do nothing.
#[doc(alias = "grTest_80207164")]
#[doc(alias = "grTest_80207404")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The stage's line-touch dynamics, `grTest_8020740C`: none.
#[doc(alias = "grTest_8020740C")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stage's shadow check, `grTest_80207414`: every shadow is drawn. Never asked, since
/// no Test stage object receives shadows (`Ground::x10_flags.b5`): it's in `ledger/dead.txt`.
pub fn check_shadow_render() -> bool {
    true
}

/// The stage's demo setup does nothing (only a cutscene's `Stage_8022532C` runs one, and no
/// cutscene loads the Test stage): it's in `ledger/dead.txt`.
pub fn on_demo_init(_r: &mut Running) {}

/// Creates one of the stage's objects and sets it up from its callbacks, `grTest_80207044`
/// (`Ground_GetStageGObj`, then the inline `Ground_SetupStageCallbacks`). The original
/// reports a missing object and goes on with none.
#[doc(alias = "grTest_80207044")]
pub fn create(r: &mut Running, map_id: i32) -> Option<GObjId> {
    let cb: &StageCallbacks = &TEST_CALLBACKS[map_id as usize];
    let gobj = setup::create_from_callbacks(r, map_id, cb, INIT_FRAME);
    if gobj.is_none() {
        crate::game::code::os_report();
    }
    gobj
}

/// The stage's setup, `grTest_80206E30`: the stage's `unk8C` flags (b4 off, b5 on); map 0;
/// map 2, its collision updated after each animation (`stageGObj2_X8Callback`); map 1, at
/// 80 times its size; then the camera limits and blast zones measured.
#[doc(alias = "grTest_80206E30")]
pub fn on_init(r: &mut Running) {
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0);
    let g = create(r, 2).expect("grtest.c: map 2");
    let gp = ground_of(r.game, g);
    r.game.grounds[gp.0].x8_callback = Some(CodeRef(X8_CALLBACK));
    let g = create(r, 1).expect("grtest.c: map 1");
    let j = jobj_of(r.game, g).expect("map 1's model");
    // `HSD_JObjSetScaleX`, `Y` and `Z`, each marking the matrix dirty.
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(80.0, 80.0, 80.0));
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// The stage's load does nothing, `grTest_UnkStage0_OnLoad`.
#[doc(alias = "grTest_UnkStage0_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// The stage's start, `grTest_UnkStage0_OnStart`: the enemy spawner, with no table.
#[doc(alias = "grTest_UnkStage0_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, r.data, None);
}

fn map_id(r: &Running, gobj: GObjId) -> i32 {
    r.game.grounds[ground_of(r.game, gobj).0].map_id
}

/// The animated models' setup (maps 0 and 1, `stageGObj0_OnInit`, `stageGObj1_OnInit`):
/// their animation (the inline `Ground_StartMapAnim`).
#[doc(alias = "stageGObj0_OnInit@80207130")]
#[doc(alias = "stageGObj1_OnInit@802073D0")]
pub fn init_model(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Map 2's setup (`stageGObj2_OnInit`): its collision and animation (the inline
/// `Ground_InitMapCollAndAnim`).
#[doc(alias = "stageGObj2_OnInit@8020716C")]
pub fn init_coll_and_model(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Map 2's `x8_callback`, after each animation step (`stageGObj2_X8Callback`): its
/// collision follows its model (`Ground_UpdateMapColl`).
#[doc(alias = "stageGObj2_X8Callback@802073B0")]
pub fn update_coll(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
}

/// Shows joint `depth` if hidden, else hides it, and the turn back to straight
/// (`grTe_804D6A48`).
fn toggle(r: &mut Running, gobj: GObjId, depth: i32) {
    if let Some(j) = nth_joint(r.game, gobj, depth) {
        let jobjs = &mut r.game.objs.jobjs;
        if jobj::get_flags(jobjs, Some(j)) & jobj::HIDDEN != 0 {
            jobj::clear_flags(jobjs, Some(j), jobj::HIDDEN);
        } else {
            jobj::set_flags(jobjs, Some(j), jobj::HIDDEN);
        }
        r.game.gr.test_turn = 0.0;
    }
}

/// Map 2's process, `grTest_802071C4`: A on the second controller shows or hides joint 16,
/// A on the third joint 17 (either back to straight), then X and Y held on the second turn
/// joint 11 five degrees a frame, down to -70 and up to 70 degrees (`grTe_804D6A48`).
#[doc(alias = "grTest_802071C4")]
pub fn proc_turn(r: &mut Running, gobj: GObjId) {
    if r.game.pads.master[1].trigger & PAD_A != 0 {
        toggle(r, gobj, 0x10);
    }
    if r.game.pads.master[2].trigger & PAD_A != 0 {
        toggle(r, gobj, 0x11);
    }
    let button = r.game.pads.master[1].button;
    if button & PAD_X != 0 {
        let t = reg(r.game.gr.test_turn);
        if t > reg(-1.221_730_5_f32) {
            r.game.gr.test_turn = mem(fp::fsubs(t, reg(0.087_266_46_f32)));
        }
    }
    if button & PAD_Y != 0 {
        let t = reg(r.game.gr.test_turn);
        if t < reg(1.221_730_5_f32) {
            r.game.gr.test_turn = mem(fp::fadds(t, reg(0.087_266_46_f32)));
        }
    }
    if let Some(j) = nth_joint(r.game, gobj, 11) {
        jobj::set_rotation_z(&mut r.game.objs.jobjs, j, r.game.gr.test_turn);
    }
}
