//! The target stages, Target Test's 26 stages, one per character (`melee/gr/grtmario.c` to
//! `grtganon.c`, ground kinds 40 to 65, `/GrT*.dat`). Each is the same template: three stage
//! objects, map 0 an animated model, map 1 an animated collision object, map 2 the stage itself
//! (its collision, animation, lights and fog, with the wind sources aged before its collision
//! moves), the camera limits and blast zones measured from the stage's points, a target
//! (`It_Kind_Mato`) at each of the points 199 to 219 the stage has and a Flipper at each of the
//! points 252 to 259, and the enemy spawner started with no table. Breaking the last target sets
//! `stage_info.flags` 0x20, which ends the match.
//!
//! The template's copies differ only in their tables, but for these:
//!
//! - Mario's stage (`grtmario.c`): its water, a fighter device of type 6 its stage's start hook
//!   registers, which splashes a fighter dropping through the surface at -120.
//! - The Ice Climbers' (`grticeclimber.c`): its blocks, a hurtbox item on each of 40 of map 2's
//!   parts, which a hit breaks; and map 2's second animation set started at its setup.
//! - Jigglypuff's (`grtpurin.c`): map 2's process turns seven of its collision joints on while
//!   their parts are left of 130 and off past it.
//! - Falco's, Fox's, Mewtwo's, Jigglypuff's and Ganondorf's: their parameters give some of their
//!   collision joints' lines a hit (the stage's line-touch dynamics).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{StageCallbacks, StageData};
use super::ground::{ground_of, jobj_of};
use super::{GR_KIND_TMARIO, anime, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `lbl_8021FA08`, Mario's stage's start hook.
pub const MARIO_HOOK: u32 = 0x8021_FA08;
/// `lbl_8021FB50`, the water of Mario's stage, a fighter device.
pub const MARIO_WATER: u32 = 0x8021_FB50;
/// `grTIceClimber_80221208`, a block's hit on the Ice Climbers' stage.
pub const ICE_HIT: u32 = 0x8022_1208;

/// How far below 0 Mario's stage's water lies, in its unscaled units.
const MARIO_WATER_Y: f64 = 120.0;

/// The collision joints of the Ice Climbers' stage's map 2 whose parts hold a block
/// (`grTIc_803E8B5C`).
const ICE_BLOCKS: [i16; 40] = [
    0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61,
];

/// The parts of Jigglypuff's stage's map 2 whose collision joints its process switches.
const PURIN_PARTS: [i32; 7] = [55, 57, 59, 61, 63, 65, 67];

/// The template's points: the targets' and the Flippers'.
const TARGET_POINTS: std::ops::Range<usize> = 199..220;
const FLIPPER_POINTS: std::ops::Range<usize> = 252..0x104;

/// `It_Kind_Mato`.
const MATO: i32 = super::super::it::kinds::MATO;
/// `It_Kind_Flipper`.
const FLIPPER: i32 = super::super::it::kinds::FLIPPER;

/// The `yakumono_param` statics of the stages that read their parameters, by
/// [`Target::yakumono`]: Falco's, Fox's, Mewtwo's, Jigglypuff's and Ganondorf's
/// (`melee__gr__grtfalco::yakumono_param` and the others).
pub const YAKUMONO_STATICS: [u32; 5] = [
    0x804D_6AF8,
    0x804D_6B00,
    0x804D_6B08,
    0x804D_6B10,
    0x804D_6B18,
];

/// One target stage: its tables, its create function, its parameters' static and its lines'
/// hits.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub data: StageData,
    /// The stage's copy of the template's create function (`grTMario_8021F8E4`).
    pub create: u32,
    /// The stage's `yakumono_param` static, for the five that read their parameters.
    pub yakumono: Option<usize>,
    pub hazard: Hazard,
}

/// Which lines of a stage's collision joints hit (its line-touch dynamics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    /// None (`return NULL`).
    None,
    /// Falco's: joint 0's floors, ceilings, right and left walls, the parameters' first four.
    Falco,
    /// Fox's: joint 1's, the same.
    Fox,
    /// Mewtwo's: joint 0's, the same, and joint 1's, the next four.
    Mewtwo,
    /// Jigglypuff's: every line of joints 0 and 1, the parameters' first.
    Purin,
    /// Ganondorf's: joint 0's ceilings, right and left walls, the first three.
    Ganon,
}

/// What a stage object's function does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// Map 0's setup: its animation.
    StartAnim,
    /// Map 1's or 2's setup: its collision and animation.
    CollAnim,
    /// Mario's map 2's setup: the same and the start hook.
    MarioFloor,
    /// The Ice Climbers' map 2's setup: the same, its second animation set and the blocks.
    IceFloor,
    /// Map 0's process and every destroy callback.
    Empty,
    /// Map 1's process: its collision follows the model.
    UpdateColl,
    /// Map 2's process: the wind sources aged, then its collision.
    WindColl,
    /// Jigglypuff's map 2's process: its collision joints switched, then as [`Role::WindColl`].
    PurinJoints,
}

/// Whether `grkind` is a target stage's.
pub fn is_target(grkind: i32) -> bool {
    (GR_KIND_TMARIO..GR_KIND_TMARIO + 26).contains(&grkind)
}

/// The stage data of the target stage `grkind`.
pub fn stage_data(grkind: i32) -> &'static StageData {
    &TARGETS[(grkind - GR_KIND_TMARIO) as usize].data
}

fn target(game: &Game) -> &'static Target {
    let k = game.stage_info.grkind;
    assert!(is_target(k), "a target stage's function on stage kind {k}");
    &TARGETS[(k - GR_KIND_TMARIO) as usize]
}

/// A stage object's process or callback, by address: the target stages'.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    if addr == MARIO_HOOK {
        mario_hook(r, gobj);
        return true;
    }
    let Ok(i) = ROLES.binary_search_by_key(&addr, |e| e.0) else {
        return false;
    };
    match ROLES[i].1 {
        Role::StartAnim => init_model(r, gobj),
        Role::CollAnim => init_coll(r, gobj),
        Role::MarioFloor => init_mario_floor(r, gobj),
        Role::IceFloor => init_ice_floor(r, gobj),
        Role::Empty => empty(r, gobj),
        Role::UpdateColl => proc_coll(r, gobj),
        Role::WindColl => proc_wind_coll(r, gobj),
        Role::PurinJoints => proc_purin(r, gobj),
    }
    true
}

/// The stages' hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    for (i, t) in TARGETS.iter().enumerate() {
        if f == t.data.on_init {
            on_init(r, i);
        } else if f == t.data.on_load {
            on_load(r);
        } else if f == t.data.on_start {
            on_start(r);
        } else {
            continue;
        }
        return true;
    }
    false
}

/// Whether `f` is a target stage's demo setup.
pub fn is_demo_init(f: u32) -> bool {
    TARGETS.iter().any(|t| t.data.on_demo_init == f)
}

/// Whether `f` is a target stage's line-touch dynamics.
pub fn is_on_touch_line(f: u32) -> bool {
    TARGETS.iter().any(|t| t.data.on_touch_line == f)
}

/// Whether `f` is a target stage's shadow check.
pub fn is_check_shadow_render(f: u32) -> bool {
    TARGETS.iter().any(|t| t.data.on_check_shadow_render == f)
}

/// The template's setup (`Ground_InitTargetStage`, inlined in each stage's `OnInit`): the
/// parameters for the stages that read them, the stage's `unk8C` flags (b4 off, b5 on), maps 0,
/// 1 and 2 in that order, the camera limits and blast zones measured, the targets and the
/// Flippers.
#[doc(alias = "grTMario_OnInit")]
#[doc(alias = "grTCaptain_OnInit")]
#[doc(alias = "grTCLink_OnInit")]
#[doc(alias = "grTDonkey_OnInit")]
#[doc(alias = "grtDrMario_OnInit")]
#[doc(alias = "grTFalco_802207F4")]
#[doc(alias = "grTFox_80220B84")]
#[doc(alias = "grTIceClimber_OnInit")]
#[doc(alias = "grTKirby_OnInit")]
#[doc(alias = "grTKoopa_OnInit")]
#[doc(alias = "grTLink_OnInit")]
#[doc(alias = "grTLuigi_OnInit")]
#[doc(alias = "grTMars_OnInit")]
#[doc(alias = "grTMewtwo_802221DC")]
#[doc(alias = "grTNess_OnInit")]
#[doc(alias = "grTPeach_OnInit")]
#[doc(alias = "grTPichu_OnInit")]
#[doc(alias = "grTPikachu_OnInit")]
#[doc(alias = "grTPurin_80223164")]
#[doc(alias = "grTSamus_OnInit")]
#[doc(alias = "grTSeak_OnInit")]
#[doc(alias = "grTYoshi_OnInit")]
#[doc(alias = "grTZelda_OnInit")]
#[doc(alias = "grTGameWatch_OnInit")]
#[doc(alias = "grTRoy_OnInit")]
#[doc(alias = "grTGanon_802246DC")]
pub fn on_init(r: &mut Running, stage: usize) {
    let t = &TARGETS[stage];
    if let Some(y) = t.yakumono {
        r.game.gr.target_yakumono[y] = super::ground::yakumono_param(r.game);
    }
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in 0..3 {
        create(r, stage, map_id);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    spawn_targets(r);
    spawn_flippers(r);
}

/// Creates one of the stage's objects and sets it up from its callbacks, each stage's copy of
/// the create function (`grTMario_8021F8E4`).
#[doc(alias = "grTMario_8021F8E4")]
#[doc(alias = "grTCaptain_8021FD04")]
#[doc(alias = "grTCLink_8021FFE8")]
#[doc(alias = "grTDonkey_802202CC")]
#[doc(alias = "grtDrMario_802205B0")]
#[doc(alias = "setupStageCallbacks@8022089C")]
#[doc(alias = "grTFox_80220C2C")]
#[doc(alias = "setupStageCallbacks@80220FB4")]
#[doc(alias = "grTKirby_80221408")]
#[doc(alias = "grTKoopa_802216EC")]
#[doc(alias = "grTLink_802219D0")]
#[doc(alias = "grTLuigi_80221CB4")]
#[doc(alias = "setupStageCallbacks@80221F98")]
#[doc(alias = "grTMewtwo_80222284")]
#[doc(alias = "grTNess_80222674")]
#[doc(alias = "grTPeach_80222958")]
#[doc(alias = "grTPichu_80222C3C")]
#[doc(alias = "grTPikachu_80222F20")]
#[doc(alias = "grTPurin_8022320C")]
#[doc(alias = "grTSamus_80223624")]
#[doc(alias = "grTSeak_80223908")]
#[doc(alias = "grTYoshi_80223BEC")]
#[doc(alias = "grTZelda_80223ED0")]
#[doc(alias = "grTGameWatch_802241B4")]
#[doc(alias = "grTRoy_80224498")]
#[doc(alias = "grTGanon_80224784")]
pub fn create(r: &mut Running, stage: usize, map_id: i32) -> Option<GObjId> {
    let cb = &TARGETS[stage].data.callbacks[map_id as usize];
    setup::create_from_callbacks(r, map_id, cb, setup::SETUP_FRAME)
}

/// The stages' demo setups, which do nothing and are dead: only a cutscene runs a stage's demo
/// setup, on its own stage, and none loads a target stage (ledger/dead.txt).
pub fn on_demo_init(_r: &mut Running) {}

/// The stages' loads, which do nothing.
#[doc(alias = "grTmario_UnkStage0_OnLoad")]
#[doc(alias = "grTCaptain_OnLoad")]
#[doc(alias = "grTclink_UnkStage0_OnLoad")]
#[doc(alias = "grTdonkey_UnkStage0_OnLoad")]
#[doc(alias = "grTdrmario_UnkStage0_OnLoad")]
#[doc(alias = "grTfalco_UnkStage0_OnLoad")]
#[doc(alias = "grTFox_UnkStage0_OnLoad")]
#[doc(alias = "grTiceclimber_UnkStage0_OnLoad")]
#[doc(alias = "grTkirby_UnkStage0_OnLoad")]
#[doc(alias = "grTkoopa_UnkStage0_OnLoad")]
#[doc(alias = "grTlink_UnkStage0_OnLoad")]
#[doc(alias = "grTluigi_UnkStage0_OnLoad")]
#[doc(alias = "grTmars_UnkStage0_OnLoad")]
#[doc(alias = "grTmewtwo_UnkStage0_OnLoad")]
#[doc(alias = "grTness_UnkStage0_OnLoad")]
#[doc(alias = "grTpeach_UnkStage0_OnLoad")]
#[doc(alias = "grTpichu_UnkStage0_OnLoad")]
#[doc(alias = "grTpikachu_UnkStage0_OnLoad")]
#[doc(alias = "grTpurin_UnkStage0_OnLoad")]
#[doc(alias = "grTSamus_OnLoad")]
#[doc(alias = "grTseak_OnLoad")]
#[doc(alias = "grTYoshi_OnLoad")]
#[doc(alias = "grTZelda_OnLoad")]
#[doc(alias = "grTgamewatch_UnkStage0_OnLoad")]
#[doc(alias = "grTemblem_UnkStage0_OnLoad")]
#[doc(alias = "grTganon_UnkStage0_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// The stages' starts: the enemy spawner, with no table.
#[doc(alias = "grTmario_UnkStage0_OnStart")]
#[doc(alias = "grTCaptain_OnStart")]
#[doc(alias = "grTclink_UnkStage0_OnStart")]
#[doc(alias = "grTdonkey_UnkStage0_OnStart")]
#[doc(alias = "grTdrmario_UnkStage0_OnStart")]
#[doc(alias = "grTfalco_UnkStage0_OnStart")]
#[doc(alias = "grTFox_UnkStage0_OnStart")]
#[doc(alias = "grTiceclimber_UnkStage0_OnStart")]
#[doc(alias = "grTkirby_UnkStage0_OnStart")]
#[doc(alias = "grTkoopa_UnkStage0_OnStart")]
#[doc(alias = "grTlink_UnkStage0_OnStart")]
#[doc(alias = "grTluigi_UnkStage0_OnStart")]
#[doc(alias = "grTmars_UnkStage0_OnStart")]
#[doc(alias = "grTmewtwo_UnkStage0_OnStart")]
#[doc(alias = "grTness_UnkStage0_OnStart")]
#[doc(alias = "grTpeach_UnkStage0_OnStart")]
#[doc(alias = "grTpichu_UnkStage0_OnStart")]
#[doc(alias = "grTpikachu_UnkStage0_OnStart")]
#[doc(alias = "grTpurin_UnkStage0_OnStart")]
#[doc(alias = "grTSamus_OnStart")]
#[doc(alias = "grTseak_OnStart")]
#[doc(alias = "grTYoshi_OnStart")]
#[doc(alias = "grTZelda_OnStart")]
#[doc(alias = "grTgamewatch_UnkStage0_OnStart")]
#[doc(alias = "grTemblem_UnkStage0_OnStart")]
#[doc(alias = "grTganon_UnkStage0_OnStart")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// The targets, `Ground_801C4210`: an enemy item of `It_Kind_Mato` at each of the points 199
/// to 219 the stage has, counted into the targets left and in all (`stage_info.x6D4`,
/// `x6D2`).
#[doc(alias = "Ground_801C4210")]
pub fn spawn_targets(r: &mut Running) -> i32 {
    let mut count: i32 = 0;
    for i in TARGET_POINTS {
        let Some(j) = r.game.stage_info.x280[i] else {
            continue;
        };
        if super::super::it::zako::spawn(r, MATO, None, Some(j), None, false).is_some() {
            count = count.wrapping_add(1);
        }
    }
    r.game.stage_info.x6D4 = count as i16;
    r.game.stage_info.x6D2 = count as i16;
    count
}

/// The Flippers, `Ground_801C42AC`: a Flipper fixed at each of the points 252 to 259 the stage
/// has, its flags held (`it_8026BE84` with `x1C.b0`).
#[doc(alias = "Ground_801C42AC")]
pub fn spawn_flippers(r: &mut Running) {
    for i in FLIPPER_POINTS {
        if let Some(j) = r.game.stage_info.x280[i] {
            super::super::it::lib26b1::drop_flipper(r, FLIPPER, j, true);
        }
    }
}

/// Map 0's setup, each stage's `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@8021F9CC")]
#[doc(alias = "stageGObj0_OnInit@8021FDEC")]
#[doc(alias = "grTCLink_802200D0")]
#[doc(alias = "stageGObj0_OnInit@802203B4")]
#[doc(alias = "stageGObj0_OnInit@80220698")]
#[doc(alias = "stageGObj0_OnInit@80220984")]
#[doc(alias = "stageGObj0_OnInit@80220D14")]
#[doc(alias = "stageGObj0_OnInit@8022109C")]
#[doc(alias = "stageGObj0_OnInit@802214F0")]
#[doc(alias = "stageGObj0_OnInit@802217D4")]
#[doc(alias = "stageGObj0_OnInit@80221AB8")]
#[doc(alias = "stageGObj0_OnInit@80221D9C")]
#[doc(alias = "stageGObj0_OnInit@80222080")]
#[doc(alias = "stageGObj0_OnInit@8022236C")]
#[doc(alias = "stageGObj0_OnInit@8022275C")]
#[doc(alias = "grTPeach_80222A40")]
#[doc(alias = "stageGObj0_OnInit@80222D24")]
#[doc(alias = "grTPikachu_80223008")]
#[doc(alias = "grTPurin_802232F4")]
#[doc(alias = "stageGObj0_OnInit@8022370C")]
#[doc(alias = "stageGObj0_OnInit@802239F0")]
#[doc(alias = "stageGObj0_OnInit@80223CD4")]
#[doc(alias = "stageGObj0_OnInit@80223FB8")]
#[doc(alias = "grTGameWatch_8022429C")]
#[doc(alias = "stageGObj0_OnInit@80224580")]
#[doc(alias = "grTGanon_8022486C")]
pub fn init_model(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Maps 1's and 2's setup (`Ground_InitMapCollAndAnim`): the map's collision bound to its
/// model, and its animation.
#[doc(alias = "stageGObj1_OnInit@8021FAD4")]
#[doc(alias = "stageGObj1_OnInit@8021FEB8")]
#[doc(alias = "stageGObj2_OnInit@8021FE28")]
#[doc(alias = "stageGObj1_OnInit@8022019C")]
#[doc(alias = "stageGObj2_OnInit@8022010C")]
#[doc(alias = "stageGObj1_OnInit@80220480")]
#[doc(alias = "stageGObj2_OnInit@802203F0")]
#[doc(alias = "stageGObj1_OnInit@80220764")]
#[doc(alias = "stageGObj2_OnInit@802206D4")]
#[doc(alias = "stageGObj1_OnInit@80220A50")]
#[doc(alias = "stageGObj2_OnInit@802209C0")]
#[doc(alias = "stageGObj1_OnInit@80220DE0")]
#[doc(alias = "stageGObj2_OnInit@80220D50")]
#[doc(alias = "stageGObj1_OnInit@8022118C")]
#[doc(alias = "stageGObj1_OnInit@802215BC")]
#[doc(alias = "stageGObj2_OnInit@8022152C")]
#[doc(alias = "stageGObj1_OnInit@802218A0")]
#[doc(alias = "stageGObj2_OnInit@80221810")]
#[doc(alias = "stageGObj1_OnInit@80221B84")]
#[doc(alias = "stageGObj2_OnInit@80221AF4")]
#[doc(alias = "stageGObj1_OnInit@80221E68")]
#[doc(alias = "stageGObj2_OnInit@80221DD8")]
#[doc(alias = "stageGObj1_OnInit@8022214C")]
#[doc(alias = "stageGObj2_OnInit@802220BC")]
#[doc(alias = "stageGObj1_OnInit@80222438")]
#[doc(alias = "stageGObj2_OnInit@802223A8")]
#[doc(alias = "stageGObj1_OnInit@80222828")]
#[doc(alias = "stageGObj2_OnInit@80222798")]
#[doc(alias = "stageGObj1_OnInit@80222B0C")]
#[doc(alias = "stageGObj2_OnInit@80222A7C")]
#[doc(alias = "stageGObj1_OnInit@80222DF0")]
#[doc(alias = "stageGObj2_OnInit@80222D60")]
#[doc(alias = "stageGObj1_OnInit@802230D4")]
#[doc(alias = "stageGObj2_OnInit@80223044")]
#[doc(alias = "stageGObj1_OnInit@8022347C")]
#[doc(alias = "stageGObj2_OnInit@80223330")]
#[doc(alias = "stageGObj1_OnInit@802237D8")]
#[doc(alias = "stageGObj2_OnInit@80223748")]
#[doc(alias = "stageGObj1_OnInit@80223ABC")]
#[doc(alias = "stageGObj2_OnInit@80223A2C")]
#[doc(alias = "stageGObj1_OnInit@80223DA0")]
#[doc(alias = "stageGObj2_OnInit@80223D10")]
#[doc(alias = "stageGObj1_OnInit@80224084")]
#[doc(alias = "stageGObj2_OnInit@80223FF4")]
#[doc(alias = "stageGObj1_OnInit@80224368")]
#[doc(alias = "stageGObj2_OnInit@802242D8")]
#[doc(alias = "stageGObj1_OnInit@8022464C")]
#[doc(alias = "stageGObj2_OnInit@802245BC")]
#[doc(alias = "stageGObj1_OnInit@80224938")]
#[doc(alias = "stageGObj2_OnInit@802248A8")]
pub fn init_coll(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// Mario's stage's map 2's setup, `grTMario_8021FA34`: as [`init_coll`], then the start hook
/// that registers the water.
#[doc(alias = "grTMario_8021FA34")]
pub fn init_mario_floor(r: &mut Running, gobj: GObjId) {
    init_coll(r, gobj);
    setup::add_start_hook(r.game, gobj, CodeRef(MARIO_HOOK));
}

/// Mario's stage's start hook, `lbl_8021FA08`: the water as a fighter device of type 6
/// (`ftCo_800C07F8`).
#[doc(alias = "lbl_8021FA08")]
pub fn mario_hook(r: &mut Running, gobj: GObjId) {
    r.game
        .gr
        .devices
        .register_coll(gobj, 6, CodeRef(MARIO_WATER));
}

/// The water of Mario's stage, `lbl_8021FB50`: a fighter dropping through its surface (-120
/// in the stage's scale) this tick splashes there, as big as its camera box's depth, and the
/// stage's splash sound plays. It never pushes.
#[doc(alias = "lbl_8021FB50")]
pub fn mario_water(r: &mut Running, fighter: GObjId) {
    let level = fp::fmuls(fp::fneg(MARIO_WATER_Y), reg(super::ground::scale(r.game)));
    let mut pos = r.game.ft_pos(fighter);
    let prev = r.game.fighter(fighter).prev_pos;
    if reg(pos.y) < level && reg(prev.y) > level {
        let size = fp::fdivs(reg(r.game.ft_camera_depth(fighter)), 10.0);
        pos.y = mem(level);
        mario_splash(r, pos, mem(size));
        r.stage_sound(0x77A10);
    }
}

/// A splash on Mario's stage, `grTMario_8021FBE8`: Mushroom Kingdom's route's (the stage's
/// effect 0x7530 scaled by `size`).
#[doc(alias = "grTMario_8021FBE8")]
pub fn mario_splash(r: &mut Running, pos: Vec3, size: f32) {
    super::kinokoroute::splash(r, pos, size);
}

/// The Ice Climbers' stage's map 2's setup, `stageGObj2_OnInit`: as [`init_coll`], then its
/// animation set 69's second (`grAnime_801C7FF8` with 2, 1, 0 and 1) and the blocks.
#[doc(alias = "stageGObj2_OnInit@802210D8")]
pub fn init_ice_floor(r: &mut Running, gobj: GObjId) {
    init_coll(r, gobj);
    anime::start(r.game, r.data, gobj, 69, 2, 1, 0.0, 1.0, true);
    ice_blocks(r, gobj);
}

/// The Ice Climbers' stage's blocks, `grTIceClimber_80221288`: for each collision joint of
/// [`ICE_BLOCKS`] bound to a part of map 2, a hurtbox item on that part (number 0, state 0) that a hit breaks ([`ice_hit`]), its hurtbox from (-1, 0, 0) to (1, 0, 0) of radius 4,
/// on.
#[doc(alias = "grTIceClimber_80221288")]
pub fn ice_blocks(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for joint in ICE_BLOCKS {
        let part = setup::coll_joint_part(r.game, r.data, 2, i32::from(joint));
        if part == -1 {
            continue;
        }
        let j = super::ground::nth_joint(r.game, gobj, part);
        let item = material::stage_item(r, 0, 0, g, j, None, Some(CodeRef(ICE_HIT)), None);
        if let Some(item) = item {
            material::set_item_hurtbox(
                r.game,
                item,
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                4.0,
            );
            material::item_hurtbox_on(r.game, item);
        }
    }
}

/// A block's hit on the Ice Climbers' stage, `grTIceClimber_80221208`: its part hidden, its
/// effect where the part stands, a small quake, a sound, and the item gone.
#[doc(alias = "grTIceClimber_80221208")]
pub fn ice_hit(r: &mut Running, item: GObjId, _g: GroundId) {
    let joint = match super::super::it::kinds::yaku::vars(r.game, item)
        .anchor
        .get()
    {
        super::super::it::kinds::yaku::Anchor::Joint(j) => j,
        other => panic!("grticeclimber.c: a block's item follows {other:?}"),
    };
    jobj::set_flags_all(&mut r.game.objs.jobjs, joint, jobj::HIDDEN);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, joint, None);
    super::super::ef::spawn(r, 0x445, Some(item), &[super::super::ef::Arg::Vec(pos)]);
    super::super::camera::request_quake(r, 2, None);
    r.stage_sound(310);
    material::destroy_item(r, item);
}

/// Map 0's process, which does nothing. The objects' destroy callbacks (`Ground::x1C_callback`,
/// the tables' `callback3`) do nothing too and are dead: one runs only when a stage destroys
/// its own object (`Ground_801C4A08`), which no target stage does (ledger/dead.txt).
#[doc(alias = "grTMario_8021FA00")]
#[doc(alias = "grTCaptain_8021FE20")]
#[doc(alias = "grTCLink_80220104")]
#[doc(alias = "grTDonkey_802203E8")]
#[doc(alias = "grtDrMario_802206CC")]
#[doc(alias = "stageGObj0_GObjProc@802209B8")]
#[doc(alias = "grTFox_80220D48")]
#[doc(alias = "stageGObj0_GObjProc@802210D0")]
#[doc(alias = "grTKirby_80221524")]
#[doc(alias = "grTKoopa_80221808")]
#[doc(alias = "grTLink_80221AEC")]
#[doc(alias = "grTLuigi_80221DD0")]
#[doc(alias = "stageGObj0_GObjProc@802220B4")]
#[doc(alias = "grTMewtwo_802223A0")]
#[doc(alias = "grTNess_80222790")]
#[doc(alias = "grTPeach_80222A74")]
#[doc(alias = "grTPichu_80222D58")]
#[doc(alias = "grTPikachu_8022303C")]
#[doc(alias = "grTPurin_80223328")]
#[doc(alias = "grTSamus_80223740")]
#[doc(alias = "grTSeak_80223A24")]
#[doc(alias = "grTYoshi_80223D08")]
#[doc(alias = "grTZelda_80223FEC")]
#[doc(alias = "grTGameWatch_802242D0")]
#[doc(alias = "grTRoy_802245B4")]
#[doc(alias = "grTGanon_802248A0")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Map 1's process, `stageGObj1_GObjProc`: its collision follows its model.
#[doc(alias = "stageGObj1_GObjProc@8021FB2C")]
#[doc(alias = "stageGObj1_GObjProc@8021FF10")]
#[doc(alias = "stageGObj1_GObjProc@802201F4")]
#[doc(alias = "stageGObj1_GObjProc@802204D8")]
#[doc(alias = "stageGObj1_GObjProc@802207BC")]
#[doc(alias = "stageGObj1_GObjProc@80220AA8")]
#[doc(alias = "stageGObj1_GObjProc@80220E38")]
#[doc(alias = "stageGObj1_GObjProc@802211E4")]
#[doc(alias = "stageGObj1_GObjProc@80221614")]
#[doc(alias = "stageGObj1_GObjProc@802218F8")]
#[doc(alias = "stageGObj1_GObjProc@80221BDC")]
#[doc(alias = "stageGObj1_GObjProc@80221EC0")]
#[doc(alias = "stageGObj1_GObjProc@802221A4")]
#[doc(alias = "stageGObj1_GObjProc@80222490")]
#[doc(alias = "stageGObj1_GObjProc@80222880")]
#[doc(alias = "stageGObj1_GObjProc@80222B64")]
#[doc(alias = "stageGObj1_GObjProc@80222E48")]
#[doc(alias = "stageGObj1_GObjProc@8022312C")]
#[doc(alias = "stageGObj1_GObjProc@802234D4")]
#[doc(alias = "stageGObj1_GObjProc@80223830")]
#[doc(alias = "stageGObj1_GObjProc@80223B14")]
#[doc(alias = "stageGObj1_GObjProc@80223DF8")]
#[doc(alias = "stageGObj1_GObjProc@802240DC")]
#[doc(alias = "stageGObj1_GObjProc@802243C0")]
#[doc(alias = "stageGObj1_GObjProc@802246A4")]
#[doc(alias = "stageGObj1_GObjProc@80224990")]
pub fn proc_coll(r: &mut Running, gobj: GObjId) {
    super::ground::update_map_coll(r.game, r.data, gobj);
}

/// Map 2's process, `stageGObj2_GObjProc` (`Ground_UpdateWindAndMapColl`): the wind sources
/// aged, then its collision follows its model.
#[doc(alias = "stageGObj2_GObjProc@8021FA9C")]
#[doc(alias = "stageGObj2_GObjProc@8021FE80")]
#[doc(alias = "stageGObj2_GObjProc@80220164")]
#[doc(alias = "stageGObj2_GObjProc@80220448")]
#[doc(alias = "stageGObj2_GObjProc@8022072C")]
#[doc(alias = "stageGObj2_GObjProc@80220A18")]
#[doc(alias = "stageGObj2_GObjProc@80220DA8")]
#[doc(alias = "stageGObj2_GObjProc@80221154")]
#[doc(alias = "stageGObj2_GObjProc@80221584")]
#[doc(alias = "stageGObj2_GObjProc@80221868")]
#[doc(alias = "stageGObj2_GObjProc@80221B4C")]
#[doc(alias = "stageGObj2_GObjProc@80221E30")]
#[doc(alias = "stageGObj2_GObjProc@80222114")]
#[doc(alias = "stageGObj2_GObjProc@80222400")]
#[doc(alias = "stageGObj2_GObjProc@802227F0")]
#[doc(alias = "stageGObj2_GObjProc@80222AD4")]
#[doc(alias = "stageGObj2_GObjProc@80222DB8")]
#[doc(alias = "stageGObj2_GObjProc@8022309C")]
#[doc(alias = "stageGObj2_GObjProc@802237A0")]
#[doc(alias = "stageGObj2_GObjProc@80223A84")]
#[doc(alias = "stageGObj2_GObjProc@80223D68")]
#[doc(alias = "stageGObj2_GObjProc@8022404C")]
#[doc(alias = "stageGObj2_GObjProc@80224330")]
#[doc(alias = "stageGObj2_GObjProc@80224614")]
#[doc(alias = "stageGObj2_GObjProc@80224900")]
pub fn proc_wind_coll(r: &mut Running, gobj: GObjId) {
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    super::ground::update_map_coll(r.game, r.data, gobj);
}

/// Jigglypuff's stage's map 2's process, `grTPurin_80223388`: each of the collision joints
/// bound to [`PURIN_PARTS`] on while its part stands left of 130 and off otherwise (or when the
/// part is missing), then as [`proc_wind_coll`].
#[doc(alias = "grTPurin_80223388")]
pub fn proc_purin(r: &mut Running, gobj: GObjId) {
    for part in PURIN_PARTS {
        let joint = setup::coll_joint_id(r.game, r.data, 2, part);
        if joint == -1 {
            continue;
        }
        let j = super::ground::nth_joint(r.game, gobj, part);
        let left = j.is_some_and(|j| {
            let p = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None);
            reg(p.x) < 130.0
        });
        if left {
            r.game.geometry_mut().enable_joint(joint);
        } else {
            r.game.geometry_mut().disable_joint(joint);
        }
    }
    proc_wind_coll(r, gobj);
}

/// The stages' line-touch dynamics without hits (`grTMario_8021FC50` and 20 others): none.
#[doc(alias = "grTMario_8021FC50")]
#[doc(alias = "grTCaptain_OnTouchLine")]
#[doc(alias = "grTCLink_80220218")]
#[doc(alias = "grTDonkey_802204FC")]
#[doc(alias = "grtDrMario_802207E0")]
#[doc(alias = "grTIceClimber_80221354")]
#[doc(alias = "grTKirby_80221638")]
#[doc(alias = "grTKoopa_8022191C")]
#[doc(alias = "grTLink_80221C00")]
#[doc(alias = "grTLuigi_80221EE4")]
#[doc(alias = "grTMars_802221C8")]
#[doc(alias = "grTNess_802228A4")]
#[doc(alias = "grTPeach_80222B88")]
#[doc(alias = "grTPichu_80222E6C")]
#[doc(alias = "grTPikachu_80223150")]
#[doc(alias = "grTSamus_OnTouchLine")]
#[doc(alias = "grTSeak_OnTouchLine")]
#[doc(alias = "grTYoshi_OnTouchLine")]
#[doc(alias = "grTZelda_OnTouchLine")]
#[doc(alias = "grTGameWatch_802243E4")]
#[doc(alias = "grTRoy_802246C8")]
pub fn touch_none() -> Option<DataRef> {
    None
}

/// The line-touch dynamics of the five stages with hits (`grTFalco_80220ACC`, `grTFox_80220E5C`,
/// `grTMewtwo_802224B4`, `grTPurin_802234F8`, `grTGanon_802249B4`): the parameters' hit for
/// the lines of the joints [`Hazard`] names.
#[doc(alias = "grTFalco_80220ACC")]
#[doc(alias = "grTFox_80220E5C")]
#[doc(alias = "grTMewtwo_802224B4")]
#[doc(alias = "grTPurin_802234F8")]
#[doc(alias = "grTGanon_802249B4")]
pub fn line_hazard(game: &Game, data: &Archives, line: i32) -> Option<DataRef> {
    let t = target(game);
    if t.hazard == Hazard::None {
        return touch_none();
    }
    if line == -1 {
        return None;
    }
    let geo = game.geometry.as_ref().expect("no collision geometry");
    let joint = geo.joint_from_line(line);
    if joint == -1 {
        return None;
    }
    let p = game.gr.target_yakumono[t.yakumono.expect("a hazard's parameters")]
        .expect("a target stage's yakumono_param");
    let by_kind = |base: u32, floor: bool| match geo.line_kind(line) {
        crate::mp::FLOOR if floor => data.ptr(p.at(base)),
        crate::mp::FLOOR => None,
        crate::mp::CEILING => data.ptr(p.at(base + if floor { 4 } else { 0 })),
        crate::mp::RIGHT_WALL => data.ptr(p.at(base + if floor { 8 } else { 4 })),
        crate::mp::LEFT_WALL => data.ptr(p.at(base + if floor { 0xC } else { 8 })),
        _ => None,
    };
    match (t.hazard, joint) {
        (Hazard::Falco, 0) | (Hazard::Fox, 1) | (Hazard::Mewtwo, 0) => by_kind(0, true),
        (Hazard::Mewtwo, 1) => by_kind(0x10, true),
        (Hazard::Ganon, 0) => by_kind(0, false),
        (Hazard::Purin, 0 | 1) => {
            let _ = geo.line_kind(line);
            data.ptr(p)
        }
        _ => None,
    }
}

/// The stages' shadow checks (`grTMario_8021FC58` and the others): every joint receives
/// shadows. Dead: the check is asked only for a stage object that receives shadows
/// (`Ground::x10_flags.b5`), which no target stage's object does (ledger/dead.txt).
pub fn check_shadow_render() -> bool {
    true
}

// The tables, generated from the decomp's (gen.py in the task's notes).

const fn cb(
    on_init: u32,
    callback1: u32,
    gobj_proc: u32,
    callback3: u32,
    flags: u32,
) -> StageCallbacks {
    StageCallbacks {
        on_init,
        callback1,
        gobj_proc,
        callback3,
        flags,
    }
}

/// `grTMr_StageCallbacks`.
const TMARIO_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8021f9cc, 0x8021f9f8, 0x8021fa00, 0x8021fa04, 0),
    cb(0x8021fad4, 0x8021fb24, 0x8021fb2c, 0x8021fb4c, 0),
    cb(
        0x8021fa34,
        0x8021fa94,
        0x8021fa9c,
        0x8021fad0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTMr_StageData`.
const TMARIO: StageData = StageData {
    grkind: GR_KIND_TMARIO,
    callbacks: &TMARIO_CALLBACKS,
    file: Some("/GrTMr.dat"),
    on_init: 0x8021f844,
    on_demo_init: 0x8021f840,
    on_load: 0x8021f8b4,
    on_start: 0x8021f8b8,
    callback4: 0x8021f8dc,
    on_touch_line: 0x8021fc50,
    on_check_shadow_render: 0x8021fc58,
    flags2: 1,
    joints: &[],
};

/// grTMr's create function, `grTMario_8021F8E4`.
const TMARIO_TARGET: Target = Target {
    data: TMARIO,
    create: 0x8021f8e4,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTCa_StageCallbacks`.
const TCAPTAIN_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8021fdec, 0x8021fe18, 0x8021fe20, 0x8021fe24, 0),
    cb(0x8021feb8, 0x8021ff08, 0x8021ff10, 0x8021ff30, 0),
    cb(
        0x8021fe28,
        0x8021fe78,
        0x8021fe80,
        0x8021feb4,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTCa_StageData`.
const TCAPTAIN: StageData = StageData {
    grkind: GR_KIND_TMARIO + 1,
    callbacks: &TCAPTAIN_CALLBACKS,
    file: Some("/GrTCa.dat"),
    on_init: 0x8021fc64,
    on_demo_init: 0x8021fc60,
    on_load: 0x8021fcd4,
    on_start: 0x8021fcd8,
    callback4: 0x8021fcfc,
    on_touch_line: 0x8021ff34,
    on_check_shadow_render: 0x8021ff3c,
    flags2: 1,
    joints: &[],
};

/// grTCa's create function, `grTCaptain_8021FD04`.
const TCAPTAIN_TARGET: Target = Target {
    data: TCAPTAIN,
    create: 0x8021fd04,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTCLink_StageCallbacks`.
const TCLINK_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802200d0, 0x802200fc, 0x80220104, 0x80220108, 0),
    cb(0x8022019c, 0x802201ec, 0x802201f4, 0x80220214, 0),
    cb(
        0x8022010c,
        0x8022015c,
        0x80220164,
        0x80220198,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTCLink_StageData`.
const TCLINK: StageData = StageData {
    grkind: GR_KIND_TMARIO + 2,
    callbacks: &TCLINK_CALLBACKS,
    file: Some("/GrTCl.dat"),
    on_init: 0x8021ff48,
    on_demo_init: 0x8021ff44,
    on_load: 0x8021ffb8,
    on_start: 0x8021ffbc,
    callback4: 0x8021ffe0,
    on_touch_line: 0x80220218,
    on_check_shadow_render: 0x80220220,
    flags2: 1,
    joints: &[],
};

/// grTCLink's create function, `grTCLink_8021FFE8`.
const TCLINK_TARGET: Target = Target {
    data: TCLINK,
    create: 0x8021ffe8,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTDk_StageCallbacks`.
const TDONKEY_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802203b4, 0x802203e0, 0x802203e8, 0x802203ec, 0),
    cb(0x80220480, 0x802204d0, 0x802204d8, 0x802204f8, 0),
    cb(
        0x802203f0,
        0x80220440,
        0x80220448,
        0x8022047c,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTDk_StageData`.
const TDONKEY: StageData = StageData {
    grkind: GR_KIND_TMARIO + 3,
    callbacks: &TDONKEY_CALLBACKS,
    file: Some("/GrTDk.dat"),
    on_init: 0x8022022c,
    on_demo_init: 0x80220228,
    on_load: 0x8022029c,
    on_start: 0x802202a0,
    callback4: 0x802202c4,
    on_touch_line: 0x802204fc,
    on_check_shadow_render: 0x80220504,
    flags2: 1,
    joints: &[],
};

/// grTDk's create function, `grTDonkey_802202CC`.
const TDONKEY_TARGET: Target = Target {
    data: TDONKEY,
    create: 0x802202cc,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTDr_StageCallbacks`.
const TDRMARIO_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80220698, 0x802206c4, 0x802206cc, 0x802206d0, 0),
    cb(0x80220764, 0x802207b4, 0x802207bc, 0x802207dc, 0),
    cb(
        0x802206d4,
        0x80220724,
        0x8022072c,
        0x80220760,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTDr_StageData`.
const TDRMARIO: StageData = StageData {
    grkind: GR_KIND_TMARIO + 4,
    callbacks: &TDRMARIO_CALLBACKS,
    file: Some("/GrTDr.dat"),
    on_init: 0x80220510,
    on_demo_init: 0x8022050c,
    on_load: 0x80220580,
    on_start: 0x80220584,
    callback4: 0x802205a8,
    on_touch_line: 0x802207e0,
    on_check_shadow_render: 0x802207e8,
    flags2: 1,
    joints: &[],
};

/// grTDr's create function, `grtDrMario_802205B0`.
const TDRMARIO_TARGET: Target = Target {
    data: TDRMARIO,
    create: 0x802205b0,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTFc_803E8918`.
const TFALCO_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80220984, 0x802209b0, 0x802209b8, 0x802209bc, 0),
    cb(0x80220a50, 0x80220aa0, 0x80220aa8, 0x80220ac8, 0),
    cb(
        0x802209c0,
        0x80220a10,
        0x80220a18,
        0x80220a4c,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTFc_StageData`.
const TFALCO: StageData = StageData {
    grkind: GR_KIND_TMARIO + 5,
    callbacks: &TFALCO_CALLBACKS,
    file: Some("/GrTFc.dat"),
    on_init: 0x802207f4,
    on_demo_init: 0x802207f0,
    on_load: 0x8022086c,
    on_start: 0x80220870,
    callback4: 0x80220894,
    on_touch_line: 0x80220acc,
    on_check_shadow_render: 0x80220b78,
    flags2: 1,
    joints: &[],
};

/// grTFc's create function, `setupStageCallbacks`.
const TFALCO_TARGET: Target = Target {
    data: TFALCO,
    create: 0x8022089c,
    yakumono: Some(0),
    hazard: Hazard::Falco,
};

/// `grTFx_StageCallbacks`.
const TFOX_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80220d14, 0x80220d40, 0x80220d48, 0x80220d4c, 0),
    cb(0x80220de0, 0x80220e30, 0x80220e38, 0x80220e58, 0),
    cb(
        0x80220d50,
        0x80220da0,
        0x80220da8,
        0x80220ddc,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTFx_StageData`.
const TFOX: StageData = StageData {
    grkind: GR_KIND_TMARIO + 6,
    callbacks: &TFOX_CALLBACKS,
    file: Some("/GrTFx.dat"),
    on_init: 0x80220b84,
    on_demo_init: 0x80220b80,
    on_load: 0x80220bfc,
    on_start: 0x80220c00,
    callback4: 0x80220c24,
    on_touch_line: 0x80220e5c,
    on_check_shadow_render: 0x80220f08,
    flags2: 1,
    joints: &[],
};

/// grTFx's create function, `grTFox_80220C2C`.
const TFOX_TARGET: Target = Target {
    data: TFOX,
    create: 0x80220c2c,
    yakumono: Some(1),
    hazard: Hazard::Fox,
};

/// `stage_callbacks`.
const TICECLIMBER_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022109c, 0x802210c8, 0x802210d0, 0x802210d4, 0),
    cb(0x8022118c, 0x802211dc, 0x802211e4, 0x80221204, 0),
    cb(
        0x802210d8,
        0x8022114c,
        0x80221154,
        0x80221188,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTIc_StageData`.
const TICECLIMBER: StageData = StageData {
    grkind: GR_KIND_TMARIO + 7,
    callbacks: &TICECLIMBER_CALLBACKS,
    file: Some("/GrTIc.dat"),
    on_init: 0x80220f14,
    on_demo_init: 0x80220f10,
    on_load: 0x80220f84,
    on_start: 0x80220f88,
    callback4: 0x80220fac,
    on_touch_line: 0x80221354,
    on_check_shadow_render: 0x8022135c,
    flags2: 1,
    joints: &[],
};

/// grTIc's create function, `setupStageCallbacks`.
const TICECLIMBER_TARGET: Target = Target {
    data: TICECLIMBER,
    create: 0x80220fb4,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTKb_StageCallbacks`.
const TKIRBY_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802214f0, 0x8022151c, 0x80221524, 0x80221528, 0),
    cb(0x802215bc, 0x8022160c, 0x80221614, 0x80221634, 0),
    cb(
        0x8022152c,
        0x8022157c,
        0x80221584,
        0x802215b8,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTKb_StageData`.
const TKIRBY: StageData = StageData {
    grkind: GR_KIND_TMARIO + 8,
    callbacks: &TKIRBY_CALLBACKS,
    file: Some("/GrTKb.dat"),
    on_init: 0x80221368,
    on_demo_init: 0x80221364,
    on_load: 0x802213d8,
    on_start: 0x802213dc,
    callback4: 0x80221400,
    on_touch_line: 0x80221638,
    on_check_shadow_render: 0x80221640,
    flags2: 1,
    joints: &[],
};

/// grTKb's create function, `grTKirby_80221408`.
const TKIRBY_TARGET: Target = Target {
    data: TKIRBY,
    create: 0x80221408,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTKp_StageCallbacks`.
const TKOOPA_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802217d4, 0x80221800, 0x80221808, 0x8022180c, 0),
    cb(0x802218a0, 0x802218f0, 0x802218f8, 0x80221918, 0),
    cb(
        0x80221810,
        0x80221860,
        0x80221868,
        0x8022189c,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTKp_StageData`.
const TKOOPA: StageData = StageData {
    grkind: GR_KIND_TMARIO + 9,
    callbacks: &TKOOPA_CALLBACKS,
    file: Some("/GrTKp.dat"),
    on_init: 0x8022164c,
    on_demo_init: 0x80221648,
    on_load: 0x802216bc,
    on_start: 0x802216c0,
    callback4: 0x802216e4,
    on_touch_line: 0x8022191c,
    on_check_shadow_render: 0x80221924,
    flags2: 1,
    joints: &[],
};

/// grTKp's create function, `grTKoopa_802216EC`.
const TKOOPA_TARGET: Target = Target {
    data: TKOOPA,
    create: 0x802216ec,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTLk_StageCallbacks`.
const TLINK_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80221ab8, 0x80221ae4, 0x80221aec, 0x80221af0, 0),
    cb(0x80221b84, 0x80221bd4, 0x80221bdc, 0x80221bfc, 0),
    cb(
        0x80221af4,
        0x80221b44,
        0x80221b4c,
        0x80221b80,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTLk_StageData`.
const TLINK: StageData = StageData {
    grkind: GR_KIND_TMARIO + 10,
    callbacks: &TLINK_CALLBACKS,
    file: Some("/GrTLk.dat"),
    on_init: 0x80221930,
    on_demo_init: 0x8022192c,
    on_load: 0x802219a0,
    on_start: 0x802219a4,
    callback4: 0x802219c8,
    on_touch_line: 0x80221c00,
    on_check_shadow_render: 0x80221c08,
    flags2: 1,
    joints: &[],
};

/// grTLk's create function, `grTLink_802219D0`.
const TLINK_TARGET: Target = Target {
    data: TLINK,
    create: 0x802219d0,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTLg_803E8DF0`.
const TLUIGI_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80221d9c, 0x80221dc8, 0x80221dd0, 0x80221dd4, 0),
    cb(0x80221e68, 0x80221eb8, 0x80221ec0, 0x80221ee0, 0),
    cb(
        0x80221dd8,
        0x80221e28,
        0x80221e30,
        0x80221e64,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTLg_StageData`.
const TLUIGI: StageData = StageData {
    grkind: GR_KIND_TMARIO + 11,
    callbacks: &TLUIGI_CALLBACKS,
    file: Some("/GrTLg.dat"),
    on_init: 0x80221c14,
    on_demo_init: 0x80221c10,
    on_load: 0x80221c84,
    on_start: 0x80221c88,
    callback4: 0x80221cac,
    on_touch_line: 0x80221ee4,
    on_check_shadow_render: 0x80221eec,
    flags2: 1,
    joints: &[],
};

/// grTLg's create function, `grTLuigi_80221CB4`.
const TLUIGI_TARGET: Target = Target {
    data: TLUIGI,
    create: 0x80221cb4,
    yakumono: None,
    hazard: Hazard::None,
};

/// `stage_callbacks`.
const TMARS_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80222080, 0x802220ac, 0x802220b4, 0x802220b8, 0),
    cb(0x8022214c, 0x8022219c, 0x802221a4, 0x802221c4, 0),
    cb(
        0x802220bc,
        0x8022210c,
        0x80222114,
        0x80222148,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTMs_StageData`.
const TMARS: StageData = StageData {
    grkind: GR_KIND_TMARIO + 12,
    callbacks: &TMARS_CALLBACKS,
    file: Some("/GrTMs.dat"),
    on_init: 0x80221ef8,
    on_demo_init: 0x80221ef4,
    on_load: 0x80221f68,
    on_start: 0x80221f6c,
    callback4: 0x80221f90,
    on_touch_line: 0x802221c8,
    on_check_shadow_render: 0x802221d0,
    flags2: 1,
    joints: &[],
};

/// grTMs's create function, `setupStageCallbacks`.
const TMARS_TARGET: Target = Target {
    data: TMARS,
    create: 0x80221f98,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTMewtwo_StageCallbacks`.
const TMEWTWO_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022236c, 0x80222398, 0x802223a0, 0x802223a4, 0),
    cb(0x80222438, 0x80222488, 0x80222490, 0x802224b0, 0),
    cb(
        0x802223a8,
        0x802223f8,
        0x80222400,
        0x80222434,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTMewtwo_StageData`.
const TMEWTWO: StageData = StageData {
    grkind: GR_KIND_TMARIO + 13,
    callbacks: &TMEWTWO_CALLBACKS,
    file: Some("/GrTMt.dat"),
    on_init: 0x802221dc,
    on_demo_init: 0x802221d8,
    on_load: 0x80222254,
    on_start: 0x80222258,
    callback4: 0x8022227c,
    on_touch_line: 0x802224b4,
    on_check_shadow_render: 0x802225c8,
    flags2: 1,
    joints: &[],
};

/// grTMewtwo's create function, `grTMewtwo_80222284`.
const TMEWTWO_TARGET: Target = Target {
    data: TMEWTWO,
    create: 0x80222284,
    yakumono: Some(2),
    hazard: Hazard::Mewtwo,
};

/// `grTNs_StageCallbacks`.
const TNESS_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022275c, 0x80222788, 0x80222790, 0x80222794, 0),
    cb(0x80222828, 0x80222878, 0x80222880, 0x802228a0, 0),
    cb(
        0x80222798,
        0x802227e8,
        0x802227f0,
        0x80222824,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTNs_StageData`.
const TNESS: StageData = StageData {
    grkind: GR_KIND_TMARIO + 14,
    callbacks: &TNESS_CALLBACKS,
    file: Some("/GrTNs.dat"),
    on_init: 0x802225d4,
    on_demo_init: 0x802225d0,
    on_load: 0x80222644,
    on_start: 0x80222648,
    callback4: 0x8022266c,
    on_touch_line: 0x802228a4,
    on_check_shadow_render: 0x802228ac,
    flags2: 1,
    joints: &[],
};

/// grTNs's create function, `grTNess_80222674`.
const TNESS_TARGET: Target = Target {
    data: TNESS,
    create: 0x80222674,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTPe_StageCallbacks`.
const TPEACH_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80222a40, 0x80222a6c, 0x80222a74, 0x80222a78, 0),
    cb(0x80222b0c, 0x80222b5c, 0x80222b64, 0x80222b84, 0),
    cb(
        0x80222a7c,
        0x80222acc,
        0x80222ad4,
        0x80222b08,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTPe_StageData`.
const TPEACH: StageData = StageData {
    grkind: GR_KIND_TMARIO + 15,
    callbacks: &TPEACH_CALLBACKS,
    file: Some("/GrTPe.dat"),
    on_init: 0x802228b8,
    on_demo_init: 0x802228b4,
    on_load: 0x80222928,
    on_start: 0x8022292c,
    callback4: 0x80222950,
    on_touch_line: 0x80222b88,
    on_check_shadow_render: 0x80222b90,
    flags2: 1,
    joints: &[],
};

/// grTPe's create function, `grTPeach_80222958`.
const TPEACH_TARGET: Target = Target {
    data: TPEACH,
    create: 0x80222958,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTPc_StageCallbacks`.
const TPICHU_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80222d24, 0x80222d50, 0x80222d58, 0x80222d5c, 0),
    cb(0x80222df0, 0x80222e40, 0x80222e48, 0x80222e68, 0),
    cb(
        0x80222d60,
        0x80222db0,
        0x80222db8,
        0x80222dec,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTPc_StageData`.
const TPICHU: StageData = StageData {
    grkind: GR_KIND_TMARIO + 16,
    callbacks: &TPICHU_CALLBACKS,
    file: Some("/GrTPc.dat"),
    on_init: 0x80222b9c,
    on_demo_init: 0x80222b98,
    on_load: 0x80222c0c,
    on_start: 0x80222c10,
    callback4: 0x80222c34,
    on_touch_line: 0x80222e6c,
    on_check_shadow_render: 0x80222e74,
    flags2: 0,
    joints: &[],
};

/// grTPc's create function, `grTPichu_80222C3C`.
const TPICHU_TARGET: Target = Target {
    data: TPICHU,
    create: 0x80222c3c,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTPk_StageCallbacks`.
const TPIKACHU_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80223008, 0x80223034, 0x8022303c, 0x80223040, 0),
    cb(0x802230d4, 0x80223124, 0x8022312c, 0x8022314c, 0),
    cb(
        0x80223044,
        0x80223094,
        0x8022309c,
        0x802230d0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTPk_StageData`.
const TPIKACHU: StageData = StageData {
    grkind: GR_KIND_TMARIO + 17,
    callbacks: &TPIKACHU_CALLBACKS,
    file: Some("/GrTPk.dat"),
    on_init: 0x80222e80,
    on_demo_init: 0x80222e7c,
    on_load: 0x80222ef0,
    on_start: 0x80222ef4,
    callback4: 0x80222f18,
    on_touch_line: 0x80223150,
    on_check_shadow_render: 0x80223158,
    flags2: 1,
    joints: &[],
};

/// grTPk's create function, `grTPikachu_80222F20`.
const TPIKACHU_TARGET: Target = Target {
    data: TPIKACHU,
    create: 0x80222f20,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTPr_StageCallbacks`.
const TPURIN_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802232f4, 0x80223320, 0x80223328, 0x8022332c, 0),
    cb(0x8022347c, 0x802234cc, 0x802234d4, 0x802234f4, 0),
    cb(
        0x80223330,
        0x80223380,
        0x80223388,
        0x80223478,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTPr_StageData`.
const TPURIN: StageData = StageData {
    grkind: GR_KIND_TMARIO + 18,
    callbacks: &TPURIN_CALLBACKS,
    file: Some("/GrTPr.dat"),
    on_init: 0x80223164,
    on_demo_init: 0x80223160,
    on_load: 0x802231dc,
    on_start: 0x802231e0,
    callback4: 0x80223204,
    on_touch_line: 0x802234f8,
    on_check_shadow_render: 0x80223578,
    flags2: 1,
    joints: &[],
};

/// grTPr's create function, `grTPurin_8022320C`.
const TPURIN_TARGET: Target = Target {
    data: TPURIN,
    create: 0x8022320c,
    yakumono: Some(3),
    hazard: Hazard::Purin,
};

/// `grTSs_StageCallbacks`.
const TSAMUS_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022370c, 0x80223738, 0x80223740, 0x80223744, 0),
    cb(0x802237d8, 0x80223828, 0x80223830, 0x80223850, 0),
    cb(
        0x80223748,
        0x80223798,
        0x802237a0,
        0x802237d4,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTSs_StageData`.
const TSAMUS: StageData = StageData {
    grkind: GR_KIND_TMARIO + 19,
    callbacks: &TSAMUS_CALLBACKS,
    file: Some("/GrTSs.dat"),
    on_init: 0x80223584,
    on_demo_init: 0x80223580,
    on_load: 0x802235f4,
    on_start: 0x802235f8,
    callback4: 0x8022361c,
    on_touch_line: 0x80223854,
    on_check_shadow_render: 0x8022385c,
    flags2: 1,
    joints: &[],
};

/// grTSs's create function, `grTSamus_80223624`.
const TSAMUS_TARGET: Target = Target {
    data: TSAMUS,
    create: 0x80223624,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTSk_StageCallbacks`.
const TSEAK_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x802239f0, 0x80223a1c, 0x80223a24, 0x80223a28, 0),
    cb(0x80223abc, 0x80223b0c, 0x80223b14, 0x80223b34, 0),
    cb(
        0x80223a2c,
        0x80223a7c,
        0x80223a84,
        0x80223ab8,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTSk_StageData`.
const TSEAK: StageData = StageData {
    grkind: GR_KIND_TMARIO + 20,
    callbacks: &TSEAK_CALLBACKS,
    file: Some("/GrTSk.dat"),
    on_init: 0x80223868,
    on_demo_init: 0x80223864,
    on_load: 0x802238d8,
    on_start: 0x802238dc,
    callback4: 0x80223900,
    on_touch_line: 0x80223b38,
    on_check_shadow_render: 0x80223b40,
    flags2: 1,
    joints: &[],
};

/// grTSk's create function, `grTSeak_80223908`.
const TSEAK_TARGET: Target = Target {
    data: TSEAK,
    create: 0x80223908,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTYs_StageCallbacks`.
const TYOSHI_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80223cd4, 0x80223d00, 0x80223d08, 0x80223d0c, 0),
    cb(0x80223da0, 0x80223df0, 0x80223df8, 0x80223e18, 0),
    cb(
        0x80223d10,
        0x80223d60,
        0x80223d68,
        0x80223d9c,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTYs_StageData`.
const TYOSHI: StageData = StageData {
    grkind: GR_KIND_TMARIO + 21,
    callbacks: &TYOSHI_CALLBACKS,
    file: Some("/GrTYs.dat"),
    on_init: 0x80223b4c,
    on_demo_init: 0x80223b48,
    on_load: 0x80223bbc,
    on_start: 0x80223bc0,
    callback4: 0x80223be4,
    on_touch_line: 0x80223e1c,
    on_check_shadow_render: 0x80223e24,
    flags2: 1,
    joints: &[],
};

/// grTYs's create function, `grTYoshi_80223BEC`.
const TYOSHI_TARGET: Target = Target {
    data: TYOSHI,
    create: 0x80223bec,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTZd_StageCallbacks`.
const TZELDA_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80223fb8, 0x80223fe4, 0x80223fec, 0x80223ff0, 0),
    cb(0x80224084, 0x802240d4, 0x802240dc, 0x802240fc, 0),
    cb(
        0x80223ff4,
        0x80224044,
        0x8022404c,
        0x80224080,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTZd_StageData`.
const TZELDA: StageData = StageData {
    grkind: GR_KIND_TMARIO + 22,
    callbacks: &TZELDA_CALLBACKS,
    file: Some("/GrTZd.dat"),
    on_init: 0x80223e30,
    on_demo_init: 0x80223e2c,
    on_load: 0x80223ea0,
    on_start: 0x80223ea4,
    callback4: 0x80223ec8,
    on_touch_line: 0x80224100,
    on_check_shadow_render: 0x80224108,
    flags2: 1,
    joints: &[],
};

/// grTZd's create function, `grTZelda_80223ED0`.
const TZELDA_TARGET: Target = Target {
    data: TZELDA,
    create: 0x80223ed0,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTGw_StageCallbacks`.
const TGAMEWATCH_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022429c, 0x802242c8, 0x802242d0, 0x802242d4, 0),
    cb(0x80224368, 0x802243b8, 0x802243c0, 0x802243e0, 0),
    cb(
        0x802242d8,
        0x80224328,
        0x80224330,
        0x80224364,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTGw_StageData`.
const TGAMEWATCH: StageData = StageData {
    grkind: GR_KIND_TMARIO + 23,
    callbacks: &TGAMEWATCH_CALLBACKS,
    file: Some("/GrTGw.dat"),
    on_init: 0x80224114,
    on_demo_init: 0x80224110,
    on_load: 0x80224184,
    on_start: 0x80224188,
    callback4: 0x802241ac,
    on_touch_line: 0x802243e4,
    on_check_shadow_render: 0x802243ec,
    flags2: 1,
    joints: &[],
};

/// grTGw's create function, `grTGameWatch_802241B4`.
const TGAMEWATCH_TARGET: Target = Target {
    data: TGAMEWATCH,
    create: 0x802241b4,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTFe_StageCallbacks`.
const TEMBLEM_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x80224580, 0x802245ac, 0x802245b4, 0x802245b8, 0),
    cb(0x8022464c, 0x8022469c, 0x802246a4, 0x802246c4, 0),
    cb(
        0x802245bc,
        0x8022460c,
        0x80224614,
        0x80224648,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTFe_StageData`.
const TEMBLEM: StageData = StageData {
    grkind: GR_KIND_TMARIO + 24,
    callbacks: &TEMBLEM_CALLBACKS,
    file: Some("/GrTFe.dat"),
    on_init: 0x802243f8,
    on_demo_init: 0x802243f4,
    on_load: 0x80224468,
    on_start: 0x8022446c,
    callback4: 0x80224490,
    on_touch_line: 0x802246c8,
    on_check_shadow_render: 0x802246d0,
    flags2: 1,
    joints: &[],
};

/// grTFe's create function, `grTRoy_80224498`.
const TEMBLEM_TARGET: Target = Target {
    data: TEMBLEM,
    create: 0x80224498,
    yakumono: None,
    hazard: Hazard::None,
};

/// `grTGn_StageCallbacks`.
const TGANON_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8022486c, 0x80224898, 0x802248a0, 0x802248a4, 0),
    cb(0x80224938, 0x80224988, 0x80224990, 0x802249b0, 0),
    cb(
        0x802248a8,
        0x802248f8,
        0x80224900,
        0x80224934,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grTGn_StageData`.
const TGANON: StageData = StageData {
    grkind: GR_KIND_TMARIO + 25,
    callbacks: &TGANON_CALLBACKS,
    file: Some("/GrTGn.dat"),
    on_init: 0x802246dc,
    on_demo_init: 0x802246d8,
    on_load: 0x80224754,
    on_start: 0x80224758,
    callback4: 0x8022477c,
    on_touch_line: 0x802249b4,
    on_check_shadow_render: 0x80224a4c,
    flags2: 1,
    joints: &[],
};

/// grTGn's create function, `grTGanon_80224784`.
const TGANON_TARGET: Target = Target {
    data: TGANON,
    create: 0x80224784,
    yakumono: Some(4),
    hazard: Hazard::Ganon,
};

/// The 26 target stages, by ground kind from `Gr_Kind_TMario`.
pub const TARGETS: [Target; 26] = [
    TMARIO_TARGET,
    TCAPTAIN_TARGET,
    TCLINK_TARGET,
    TDONKEY_TARGET,
    TDRMARIO_TARGET,
    TFALCO_TARGET,
    TFOX_TARGET,
    TICECLIMBER_TARGET,
    TKIRBY_TARGET,
    TKOOPA_TARGET,
    TLINK_TARGET,
    TLUIGI_TARGET,
    TMARS_TARGET,
    TMEWTWO_TARGET,
    TNESS_TARGET,
    TPEACH_TARGET,
    TPICHU_TARGET,
    TPIKACHU_TARGET,
    TPURIN_TARGET,
    TSAMUS_TARGET,
    TSEAK_TARGET,
    TYOSHI_TARGET,
    TZELDA_TARGET,
    TGAMEWATCH_TARGET,
    TEMBLEM_TARGET,
    TGANON_TARGET,
];

/// What each stage object's function does, by address (sorted).
const ROLES: [(u32, Role); 234] = [
    (0x8021f9cc, Role::StartAnim),
    (0x8021fa00, Role::Empty),
    (0x8021fa04, Role::Empty),
    (0x8021fa34, Role::MarioFloor),
    (0x8021fa9c, Role::WindColl),
    (0x8021fad0, Role::Empty),
    (0x8021fad4, Role::CollAnim),
    (0x8021fb2c, Role::UpdateColl),
    (0x8021fb4c, Role::Empty),
    (0x8021fdec, Role::StartAnim),
    (0x8021fe20, Role::Empty),
    (0x8021fe24, Role::Empty),
    (0x8021fe28, Role::CollAnim),
    (0x8021fe80, Role::WindColl),
    (0x8021feb4, Role::Empty),
    (0x8021feb8, Role::CollAnim),
    (0x8021ff10, Role::UpdateColl),
    (0x8021ff30, Role::Empty),
    (0x802200d0, Role::StartAnim),
    (0x80220104, Role::Empty),
    (0x80220108, Role::Empty),
    (0x8022010c, Role::CollAnim),
    (0x80220164, Role::WindColl),
    (0x80220198, Role::Empty),
    (0x8022019c, Role::CollAnim),
    (0x802201f4, Role::UpdateColl),
    (0x80220214, Role::Empty),
    (0x802203b4, Role::StartAnim),
    (0x802203e8, Role::Empty),
    (0x802203ec, Role::Empty),
    (0x802203f0, Role::CollAnim),
    (0x80220448, Role::WindColl),
    (0x8022047c, Role::Empty),
    (0x80220480, Role::CollAnim),
    (0x802204d8, Role::UpdateColl),
    (0x802204f8, Role::Empty),
    (0x80220698, Role::StartAnim),
    (0x802206cc, Role::Empty),
    (0x802206d0, Role::Empty),
    (0x802206d4, Role::CollAnim),
    (0x8022072c, Role::WindColl),
    (0x80220760, Role::Empty),
    (0x80220764, Role::CollAnim),
    (0x802207bc, Role::UpdateColl),
    (0x802207dc, Role::Empty),
    (0x80220984, Role::StartAnim),
    (0x802209b8, Role::Empty),
    (0x802209bc, Role::Empty),
    (0x802209c0, Role::CollAnim),
    (0x80220a18, Role::WindColl),
    (0x80220a4c, Role::Empty),
    (0x80220a50, Role::CollAnim),
    (0x80220aa8, Role::UpdateColl),
    (0x80220ac8, Role::Empty),
    (0x80220d14, Role::StartAnim),
    (0x80220d48, Role::Empty),
    (0x80220d4c, Role::Empty),
    (0x80220d50, Role::CollAnim),
    (0x80220da8, Role::WindColl),
    (0x80220ddc, Role::Empty),
    (0x80220de0, Role::CollAnim),
    (0x80220e38, Role::UpdateColl),
    (0x80220e58, Role::Empty),
    (0x8022109c, Role::StartAnim),
    (0x802210d0, Role::Empty),
    (0x802210d4, Role::Empty),
    (0x802210d8, Role::IceFloor),
    (0x80221154, Role::WindColl),
    (0x80221188, Role::Empty),
    (0x8022118c, Role::CollAnim),
    (0x802211e4, Role::UpdateColl),
    (0x80221204, Role::Empty),
    (0x802214f0, Role::StartAnim),
    (0x80221524, Role::Empty),
    (0x80221528, Role::Empty),
    (0x8022152c, Role::CollAnim),
    (0x80221584, Role::WindColl),
    (0x802215b8, Role::Empty),
    (0x802215bc, Role::CollAnim),
    (0x80221614, Role::UpdateColl),
    (0x80221634, Role::Empty),
    (0x802217d4, Role::StartAnim),
    (0x80221808, Role::Empty),
    (0x8022180c, Role::Empty),
    (0x80221810, Role::CollAnim),
    (0x80221868, Role::WindColl),
    (0x8022189c, Role::Empty),
    (0x802218a0, Role::CollAnim),
    (0x802218f8, Role::UpdateColl),
    (0x80221918, Role::Empty),
    (0x80221ab8, Role::StartAnim),
    (0x80221aec, Role::Empty),
    (0x80221af0, Role::Empty),
    (0x80221af4, Role::CollAnim),
    (0x80221b4c, Role::WindColl),
    (0x80221b80, Role::Empty),
    (0x80221b84, Role::CollAnim),
    (0x80221bdc, Role::UpdateColl),
    (0x80221bfc, Role::Empty),
    (0x80221d9c, Role::StartAnim),
    (0x80221dd0, Role::Empty),
    (0x80221dd4, Role::Empty),
    (0x80221dd8, Role::CollAnim),
    (0x80221e30, Role::WindColl),
    (0x80221e64, Role::Empty),
    (0x80221e68, Role::CollAnim),
    (0x80221ec0, Role::UpdateColl),
    (0x80221ee0, Role::Empty),
    (0x80222080, Role::StartAnim),
    (0x802220b4, Role::Empty),
    (0x802220b8, Role::Empty),
    (0x802220bc, Role::CollAnim),
    (0x80222114, Role::WindColl),
    (0x80222148, Role::Empty),
    (0x8022214c, Role::CollAnim),
    (0x802221a4, Role::UpdateColl),
    (0x802221c4, Role::Empty),
    (0x8022236c, Role::StartAnim),
    (0x802223a0, Role::Empty),
    (0x802223a4, Role::Empty),
    (0x802223a8, Role::CollAnim),
    (0x80222400, Role::WindColl),
    (0x80222434, Role::Empty),
    (0x80222438, Role::CollAnim),
    (0x80222490, Role::UpdateColl),
    (0x802224b0, Role::Empty),
    (0x8022275c, Role::StartAnim),
    (0x80222790, Role::Empty),
    (0x80222794, Role::Empty),
    (0x80222798, Role::CollAnim),
    (0x802227f0, Role::WindColl),
    (0x80222824, Role::Empty),
    (0x80222828, Role::CollAnim),
    (0x80222880, Role::UpdateColl),
    (0x802228a0, Role::Empty),
    (0x80222a40, Role::StartAnim),
    (0x80222a74, Role::Empty),
    (0x80222a78, Role::Empty),
    (0x80222a7c, Role::CollAnim),
    (0x80222ad4, Role::WindColl),
    (0x80222b08, Role::Empty),
    (0x80222b0c, Role::CollAnim),
    (0x80222b64, Role::UpdateColl),
    (0x80222b84, Role::Empty),
    (0x80222d24, Role::StartAnim),
    (0x80222d58, Role::Empty),
    (0x80222d5c, Role::Empty),
    (0x80222d60, Role::CollAnim),
    (0x80222db8, Role::WindColl),
    (0x80222dec, Role::Empty),
    (0x80222df0, Role::CollAnim),
    (0x80222e48, Role::UpdateColl),
    (0x80222e68, Role::Empty),
    (0x80223008, Role::StartAnim),
    (0x8022303c, Role::Empty),
    (0x80223040, Role::Empty),
    (0x80223044, Role::CollAnim),
    (0x8022309c, Role::WindColl),
    (0x802230d0, Role::Empty),
    (0x802230d4, Role::CollAnim),
    (0x8022312c, Role::UpdateColl),
    (0x8022314c, Role::Empty),
    (0x802232f4, Role::StartAnim),
    (0x80223328, Role::Empty),
    (0x8022332c, Role::Empty),
    (0x80223330, Role::CollAnim),
    (0x80223388, Role::PurinJoints),
    (0x80223478, Role::Empty),
    (0x8022347c, Role::CollAnim),
    (0x802234d4, Role::UpdateColl),
    (0x802234f4, Role::Empty),
    (0x8022370c, Role::StartAnim),
    (0x80223740, Role::Empty),
    (0x80223744, Role::Empty),
    (0x80223748, Role::CollAnim),
    (0x802237a0, Role::WindColl),
    (0x802237d4, Role::Empty),
    (0x802237d8, Role::CollAnim),
    (0x80223830, Role::UpdateColl),
    (0x80223850, Role::Empty),
    (0x802239f0, Role::StartAnim),
    (0x80223a24, Role::Empty),
    (0x80223a28, Role::Empty),
    (0x80223a2c, Role::CollAnim),
    (0x80223a84, Role::WindColl),
    (0x80223ab8, Role::Empty),
    (0x80223abc, Role::CollAnim),
    (0x80223b14, Role::UpdateColl),
    (0x80223b34, Role::Empty),
    (0x80223cd4, Role::StartAnim),
    (0x80223d08, Role::Empty),
    (0x80223d0c, Role::Empty),
    (0x80223d10, Role::CollAnim),
    (0x80223d68, Role::WindColl),
    (0x80223d9c, Role::Empty),
    (0x80223da0, Role::CollAnim),
    (0x80223df8, Role::UpdateColl),
    (0x80223e18, Role::Empty),
    (0x80223fb8, Role::StartAnim),
    (0x80223fec, Role::Empty),
    (0x80223ff0, Role::Empty),
    (0x80223ff4, Role::CollAnim),
    (0x8022404c, Role::WindColl),
    (0x80224080, Role::Empty),
    (0x80224084, Role::CollAnim),
    (0x802240dc, Role::UpdateColl),
    (0x802240fc, Role::Empty),
    (0x8022429c, Role::StartAnim),
    (0x802242d0, Role::Empty),
    (0x802242d4, Role::Empty),
    (0x802242d8, Role::CollAnim),
    (0x80224330, Role::WindColl),
    (0x80224364, Role::Empty),
    (0x80224368, Role::CollAnim),
    (0x802243c0, Role::UpdateColl),
    (0x802243e0, Role::Empty),
    (0x80224580, Role::StartAnim),
    (0x802245b4, Role::Empty),
    (0x802245b8, Role::Empty),
    (0x802245bc, Role::CollAnim),
    (0x80224614, Role::WindColl),
    (0x80224648, Role::Empty),
    (0x8022464c, Role::CollAnim),
    (0x802246a4, Role::UpdateColl),
    (0x802246c4, Role::Empty),
    (0x8022486c, Role::StartAnim),
    (0x802248a0, Role::Empty),
    (0x802248a4, Role::Empty),
    (0x802248a8, Role::CollAnim),
    (0x80224900, Role::WindColl),
    (0x80224934, Role::Empty),
    (0x80224938, Role::CollAnim),
    (0x80224990, Role::UpdateColl),
    (0x802249b0, Role::Empty),
];
