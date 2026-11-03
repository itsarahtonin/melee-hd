//! All-Star's rest area (`melee/gr/grheal.c`, ground kind 0x42, `/GrHe.dat`): between
//! All-Star's rounds, a platform with the run's hearts left (one at each of joints 7 to 9 the
//! run hasn't taken, `gm_80473A18.x90`), a coin and a trophy every few rests, the next round's
//! characters' trophies on the stand and those met so far around it, and a portal that ends
//! the match when the first fighter lands on it.

use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::data::StageCallbacks;
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint};
use super::vars::MapVars;
use super::{anime, setup, zako};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, HIDDEN};

/// The stage's hooks (`grHeal_StageData`).
pub const ON_INIT: u32 = 0x8021_EF3C;
pub const ON_DEMO_INIT: u32 = 0x8021_EF38;
pub const ON_LOAD: u32 = 0x8021_EFBC;
pub const ON_START: u32 = 0x8021_EFC0;
pub const CALLBACK4: u32 = 0x8021_EFE4;
pub const ON_TOUCH_LINE: u32 = 0x8021_F830;
pub const CHECK_SHADOW_RENDER: u32 = 0x8021_F838;
/// `fn_8021F424`, the watcher's check of the portal's point; `onJointCollision`, the
/// portal's collision callback.
pub const PORTAL_CHECK: u32 = 0x8021_F424;
pub const ON_JOINT: u32 = 0x8021_F4C0;

/// `stage_callbacks`: map 0 the platform with the hearts' joints, map 1 the stand and the
/// portal (with the stage's lights and fog), map 2 a trophy met, map 3 the background, map 4
/// a next round's trophy.
pub const CALLBACKS: [StageCallbacks; 5] = [
    super::data::cb(0x8021_F0D8, 0x8021_F170, 0x8021_F178, 0x8021_F17C, 0),
    super::data::cb(
        0x8021_F180,
        0x8021_F41C,
        0x8021_F474,
        0x8021_F4BC,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    super::data::cb(0x8021_F5C8, 0x8021_F5CC, 0x8021_F5D4, 0x8021_F5D8, 0),
    super::data::cb(0x8021_F5DC, 0x8021_F618, 0x8021_F620, 0x8021_F624, 0),
    super::data::cb(0x8021_F6F8, 0x8021_F6FC, 0x8021_F704, 0x8021_F708, 0),
];

/// `grHeal_803E83B8`: the stand's joints for the characters met, by `x76`'s slot.
const MET_JOINTS: [i32; 26] = [
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54,
];

/// `frame_to_character_id`: each trophy animation frame's character, -1 after the last.
const FRAME_CHARACTERS: [i32; 26] = [
    0, 21, 1, 22, 20, 2, 25, 3, 14, 4, 5, 7, 6, 8, 9, 10, 11, 24, 12, 13, 15, 16, 17, 18, 23, -1,
];

/// `grHeal_804D49D8`: the hearts' joints.
const HEART_JOINTS: [i32; 3] = [7, 8, 9];

/// `ChKind_None` in `x76`, and the hearts' item kind.
const CHKIND_NONE: i32 = 33;
const HEART_KIND: i32 = 8;

/// A stage object's process or callback, by address: the rest area's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8021_F0D8 => init_platform(r, gobj),
        0x8021_F180 => init_stand(r, gobj),
        0x8021_F474 => proc_stand(r, gobj),
        0x8021_F5DC => init_background(r, gobj),
        0x8021_F178 | 0x8021_F17C | 0x8021_F4BC | 0x8021_F5C8 | 0x8021_F5D4 | 0x8021_F5D8
        | 0x8021_F620 | 0x8021_F624 | 0x8021_F6F8 | 0x8021_F704 | 0x8021_F708 => {}
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        ON_INIT => on_init(r),
        ON_LOAD => {}
        ON_START => {
            zako::start(r.game, None);
        }
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing, the demo setup and the load.
// Ports grHeal_8021EF38; aliased once a check reaches it.
// Ports grHeal_8021EFBC; aliased once a check reaches it.
// Ports stageGObj0_GObjProc@8021F178; aliased once a check reaches it.
// Ports stageGObj0_Callback3@8021F17C; aliased once a check reaches it.
// Ports stageGObj1_Callback3@8021F4BC; aliased once a check reaches it.
// Ports stageGObj2_OnInit@8021F5C8; aliased once a check reaches it.
// Ports stageGObj2_GObjProc@8021F5D4; aliased once a check reaches it.
// Ports stageGObj2_Callback3@8021F5D8; aliased once a check reaches it.
// Ports stageGObj3_GObjProc@8021F620; aliased once a check reaches it.
// Ports stageGObj3_Callback3@8021F624; aliased once a check reaches it.
// Ports stageGObj4_OnInit@8021F6F8; aliased once a check reaches it.
// Ports stageGObj4_GObjProc@8021F704; aliased once a check reaches it.
// Ports stageGObj4_Callback3@8021F708; aliased once a check reaches it.
pub fn empty() {}

/// The stage's own objects' first callbacks, which answer no (`stageGObjN_Callback1`), and the
/// fourth hook (`grHeal_8021EFE4`, which nothing calls).
// Ports stageGObj0_Callback1@8021F170; aliased once a check reaches it.
// Ports stageGObj1_Callback1@8021F41C; aliased once a check reaches it.
// Ports stageGObj2_Callback1@8021F5CC; aliased once a check reaches it.
// Ports stageGObj3_Callback1@8021F618; aliased once a check reaches it.
// Ports stageGObj4_Callback1@8021F6FC; aliased once a check reaches it.
pub fn no() -> bool {
    false
}

/// The line-touch dynamics: none (`grHeal_8021F830`).
// Ports grHeal_8021F830; aliased once a check reaches it.
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// Every shadow is drawn (`grHeal_8021F838`).
// Ports grHeal_8021F838; aliased once a check reaches it.
pub fn check_shadow_render() -> bool {
    true
}

/// Creates one of the stage's objects, `grHeal_8021EFEC`.
// Ports grHeal_8021EFEC; aliased once a check reaches it.
fn create(r: &mut Running, map_id: i32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, &CALLBACKS[map_id as usize], setup::SETUP_FRAME)
}

/// The stage's setup, `grHeal_8021EF3C`: its parameters, `unk8C` (b4 off, b5 on), maps 0, 1
/// and 3, the camera limits and blast zones measured, and the blast zone's top twice the
/// stage's offset.
// Ports grHeal_8021EF3C; aliased once a check reaches it.
// Ports grZakoGenerator_801CAE04; aliased once a check reaches it.
pub fn on_init(r: &mut Running) {
    r.game.gr.heal_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 1, 3] {
        create(r, map_id);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let top = super::stage::blast_top(r.game);
    let top = crate::fpu::mem(gekko_fp::fmuls(2.0, crate::fpu::reg(top)));
    super::ground::set_blast_top(r.game, top);
}

/// The stage's parameters' word at `off` (`grHeal_804D6AF0[0]`).
fn param(r: &Running, off: u32) -> i32 {
    let p = r
        .game
        .gr
        .heal_yakumono
        .expect("grheal.c: the stage's parameters");
    r.data.u32(p.at(off)) as i32
}

fn map_id(r: &Running, gobj: GObjId) -> i32 {
    r.game.grounds[ground_of(r.game, gobj).0].map_id
}

/// The platform (map 0), `stageGObj0_OnInit`: its animation, and a heart at each of its
/// joints the run still has (`x90`).
// Ports stageGObj0_OnInit@8021F0D8; aliased once a check reaches it.
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    for i in 0..param(r, 0) {
        if r.game.one_player.allstar.x90[i as usize] != 0 {
            drop_heart(r, HEART_KIND, HEART_JOINTS[i as usize], i);
        }
    }
}

/// A heart at the platform's joint `joint` for the run's slot `slot`, `grHeal_8021F79C`.
// Ports grHeal_8021F79C; aliased once a check reaches it.
fn drop_heart(r: &mut Running, kind: i32, joint: i32, slot: i32) {
    let gp = map_gobj(r.game, 0).expect("grheal.c: the platform");
    let j = nth_joint(r.game, gp, joint);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    super::super::it::lib26b1::drop_food(r, kind, gp, pos, slot, true);
}

/// The trophy animation frame of character `ckind` (Sheik's is Zelda's), `grHeal_8021F70C`;
/// the first for one with none.
// Ports grHeal_8021F70C; aliased once a check reaches it.
fn trophy_frame(ckind: i32) -> i32 {
    let ckind = if ckind == 19 { 18 } else { ckind };
    FRAME_CHARACTERS
        .iter()
        .take_while(|&&c| c != -1)
        .position(|&c| c == ckind)
        .map_or(0, |f| f as i32)
}

/// A trophy of map `map_id` at animation frame `frame` moved onto `parent` (shown when
/// `show`), its own object destroyed: `grHeal_8021F4E8` (map 4, the next round's) and
/// `grHeal_8021F628` (map 2, one met).
// Ports grHeal_8021F4E8; aliased once a check reaches it.
// Ports grHeal_8021F628; aliased once a check reaches it.
fn place_trophy(
    r: &mut Running,
    map_id: i32,
    frame: i32,
    parent: Option<crate::hsd::jobj::JObjId>,
    show: bool,
) {
    let gobj = create(r, map_id)
        .unwrap_or_else(|| panic!("the original's assertion failed: grheal.c (a trophy's gobj)"));
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    anime::start(r.game, r.data, gobj, 0, 7, 0, frame as f32, 0.0, true);
    let j = jobj_of(r.game, gobj);
    let child = j.and_then(|j| r.game.objs.jobjs[j].child);
    jobj::reparent(&mut r.game.objs.jobjs, child, parent);
    if show {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, child, HIDDEN);
    }
    setup::destroy(r, Some(gobj));
}

/// The stand (map 1), `stageGObj1_OnInit`: its animation, a coin and a trophy every
/// parameter-`x4`th rest, the next round's characters' trophies on joints 58 to 60, those met
/// on the stand's joints, and the portal's collision callback.
// Ports stageGObj1_OnInit@8021F180; aliased once a check reaches it.
pub fn init_stand(r: &mut Running, gobj: GObjId) {
    let m = map_id(r, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let a = r.game.one_player.allstar;
    if i32::from(a.x94[0]) % param(r, 4) == 0 {
        super::super::code::unported_call(
            "the rest area's coin and trophy (it_802F2094, Toy_80304A58)",
        );
    }
    let next = usize::from(a.x94[1]);
    let joints = [58, 59, 60].map(|d| nth_joint(r.game, gobj, d));
    for (k, &ckind) in a.x96[..next].iter().enumerate() {
        place_trophy(r, 4, trophy_frame(i32::from(ckind)), joints[k], true);
    }
    let met = MET_JOINTS.map(|d| nth_joint(r.game, gobj, d));
    for (k, &ckind) in a.x76.iter().enumerate() {
        if i32::from(ckind) != CHKIND_NONE {
            place_trophy(r, 2, trophy_frame(i32::from(ckind)), met[k], false);
        }
    }
    let g = ground_of(r.game, gobj);
    r.game.geometry_mut().set_ledge_callback(
        0,
        Some(super::CollUser::Ground(g)),
        Some(CodeRef(ON_JOINT)),
    );
    r.game.grounds[g.0].u.map().set_word(0);
}

/// Whether the portal was landed on since the stand's last process, `fn_8021F424`.
// Ports fn_8021F424; aliased once a check reaches it.
pub fn portal_check(game: &mut super::super::state::Game) -> bool {
    let Some(gobj) = map_gobj(game, 1) else {
        return false;
    };
    let g = ground_of(game, gobj);
    game.grounds[g.0].u.map().word() != 0
}

/// The stand's process (map 1), `stageGObj1_GObjProc`: the watcher asked for the portal's
/// point the first fighter stands in (a box of 10 by 20, with [`portal_check`]), the wind,
/// and the portal's landing forgotten.
// Ports stageGObj1_GObjProc@8021F474; aliased once a check reaches it.
pub fn proc_stand(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    super::ground::watch_points(r.game, Some(CodeRef(PORTAL_CHECK)), 10.0, 20.0);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    r.game.grounds[g.0].u.map().set_word(0);
}

/// The portal's collision callback, `onJointCollision`: a fighter (`x34_flags.b1234` 1)
/// landing on it or standing on it (ground kinds 1 and 2) marks the landing.
// Ports onJointCollision@8021F4C0; aliased once a check reaches it.
pub fn portal_touched(gp: &mut super::super::structs::Ground, b1234: u32, kind: i32) {
    if b1234 == 1 && (kind.wrapping_sub(1) as u32) <= 1 {
        gp.u.map().set_word(1);
    }
}

/// The background (map 3), `stageGObj3_OnInit`: its flag `x10_flags.b2` off and its
/// animation.
// Ports stageGObj3_OnInit@8021F5DC; aliased once a check reaches it.
pub fn init_background(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].x10_flags.b2 = false;
    let m = map_id(r, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

impl MapVars {
    /// Writes the word at `xC4` whole.
    pub fn set_word(&mut self, w: u32) {
        let bit = |i: u32| w >> (31 - i) & 1 != 0;
        self.b0 = bit(0);
        self.b1 = bit(1);
        self.phase = (w >> 14) as u16;
        self.b26 = bit(18);
        self.b27 = bit(19);
        self.b28 = bit(20);
        self.b29 = bit(21);
        self.b30 = bit(22);
        self.b31 = bit(23);
        self.low = w as u8;
    }
}
