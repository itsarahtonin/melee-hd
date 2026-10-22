//! Mushroom Kingdom II, `melee/gr/grinishie2.c`: the stage (map 2) with its waterfall's
//! animations, the two logs that ride the waterfall (maps 3 and 4), Birdo's platform (map
//! 15), Pidgit's carpet (map 1), and the scenery (map 0).
//!
//! The stage sends each of them after a random wait and waits again once it's gone: a log
//! comes down the waterfall, its collision on after a while, until its animation ends; Birdo's
//! platform comes in from a random side carrying Birdo (`it::kinds::kyasarin`), its collision
//! following her, and goes with her; Pidgit's carpet flies in from a random side, turning
//! about between the camera's edges a few times before it leaves. Under 20 seconds left, the
//! music speeds up.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::state::{Game, Running};
use super::data::{INISHIE2_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, nth_joint, update_map_coll};
use super::{anime, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The stage's map id, and Pidgit's carpet's, Birdo's platform's and the logs'.
pub const MAP_STAGE: i32 = 2;
pub const MAP_PIDGIT: i32 = 1;
pub const MAP_BIRDO: i32 = 15;
pub const MAP_LOGS: [i32; 2] = [3, 4];

/// The logs' collision joints, Birdo's platform's and Pidgit's carpet's.
const LOG_JOINTS: [i32; 2] = [0xC, 0xD];
const BIRDO_JOINT: i32 = 0xE;
const PIDGIT_JOINT: i32 = 0;

/// The stage's flags (`grInishie2_GroundVars::xC4_flags`): the music sped up, the logs' waits
/// running, Birdo's, Pidgit's.
const MUSIC: u8 = 0x80;
const LOG_WAITS: [u8; 2] = [0x40, 0x20];
const BIRDO_WAIT: u8 = 0x10;
const PIDGIT_WAIT: u8 = 0x08;
/// A log's flag: its collision back.
const LOG_ON: u8 = 0x80;
/// Pidgit's carpet's flags (`grInishie2_GroundVars3::xC8_flags`): leaving, coming in, on its
/// way out, the side it heads for (its animation).
const LEAVING: u8 = 0x80;
const COMING_IN: u8 = 0x40;
const GOING: u8 = 0x20;
const SIDE: u8 = 0x10;

/// Where the stage's process puts the scale joints' descriptors of the objects it makes on the
/// stack: through `grInishie2_801FD0B4` for a log (0x68 below the process loop's frame, as
/// Kongo Jungle's events are 0x70 below it), through `grInishie2_801FD3CC` or
/// `grInishie2_801FD7A8` for Birdo's platform or Pidgit's carpet (0x58 below).
pub const LOG_FRAME: u32 = 0x804E_E9E8;
pub const SEND_FRAME: u32 = 0x804E_E9F8;

/// `grInishie2_YakumonoParam`'s fields, by offset.
mod p {
    /// The logs' waits: a base and a random spread each (halfwords).
    pub const LOG1_WAIT: u32 = 0x0;
    pub const LOG1_SPREAD: u32 = 0x2;
    pub const LOG2_WAIT: u32 = 0x4;
    pub const LOG2_SPREAD: u32 = 0x6;
    /// Birdo's wait, and Pidgit's.
    pub const BIRDO_WAIT: u32 = 0x8;
    pub const BIRDO_SPREAD: u32 = 0xA;
    pub const PIDGIT_WAIT: u32 = 0xC;
    pub const PIDGIT_SPREAD: u32 = 0xE;
    /// Each log's wait before its collision comes on (halfwords).
    pub const LOG_ON: u32 = 0x10;
    /// Where Birdo comes in from the right and the left, beyond the blast zones (two Vec3).
    pub const BIRDO_FROM: u32 = 0x14;
    /// Her platform's height above her.
    pub const BIRDO_PLATFORM: u32 = 0x2C;
    /// Where Pidgit comes in from (two Vec3).
    pub const PIDGIT_FROM: u32 = 0x30;
    /// Pidgit's odds of keeping its course (halfwords): kept against turned.
    pub const PIDGIT_KEEP: u32 = 0x48;
    pub const PIDGIT_TURN: u32 = 0x4A;
}

/// The fields of `grInishie2_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }
    fn v(self, data: &Archives, off: u32) -> Vec3 {
        Vec3::new(
            self.f(data, off),
            self.f(data, off + 4),
            self.f(data, off + 8),
        )
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .inishie2_yakumono
            .expect("grinishie2.c: yakumono_param"),
    )
}

/// A base plus a random draw below `spread` (none when it's 0), as a halfword (`test_random`).
fn wait(r: &mut Running, base: i16, spread: i16) -> i16 {
    let spread = i32::from(spread);
    let k = if spread != 0 {
        r.game.objs.rng.below(spread)
    } else {
        0
    };
    i32::from(base).wrapping_add(k) as i16
}

/// A stage object's process or callback, by address: Mushroom Kingdom II's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_CDFC | 0x801F_CE00 | 0x801F_D014 | 0x801F_D364 | 0x801F_D740 | 0x801F_DFE4 => {
            empty(r, gobj)
        }
        0x801F_CDC8 => init_back(r, gobj),
        0x801F_CE04 => init_stage(r, gobj),
        0x801F_CFCC => proc_stage(r, gobj),
        0x801F_D198 => init_log(r, gobj),
        0x801F_D224 => proc_log(r, gobj),
        0x801F_D4F0 => init_birdo(r, gobj),
        0x801F_D654 => proc_birdo(r, gobj),
        0x801F_DE8C => init_pidgit(r, gobj),
        0x801F_DED8 => proc_pidgit(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_CBC4 => on_init(r),
        0x801F_CCAC => on_load(r),
        0x801F_CCB0 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grInishie2_801FCDFC")]
#[doc(alias = "grInishie2_801FCE00")]
#[doc(alias = "grInishie2_801FD014")]
#[doc(alias = "grInishie2_801FD364")]
#[doc(alias = "grInishie2_801FD740")]
#[doc(alias = "grInishie2_801FDFE4")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `grInishie2_801FCCDC`.
#[doc(alias = "grInishie2_801FCCDC")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`grI2_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &INISHIE2_CALLBACKS[map_id as usize]
}

/// Mushroom Kingdom II's setup, `grInishie2_801FCBC4`: its parameters; the scenery and the
/// stage; the logs', Birdo's, Pidgit's and the other moving parts' collision joints out of the
/// search; camera limits and blast zones; the camera's near plane at 1 and far plane at 4000.
#[doc(alias = "grInishie2_801FCBC4")]
pub fn on_init(r: &mut Running) {
    r.game.gr.inishie2_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 0, setup::SETUP_FRAME);
    create(r, MAP_STAGE, setup::SETUP_FRAME);
    for joint in [0, 0xC, 0xD, 0xE, 3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 2] {
        r.game.geometry_mut().disable_joint(joint);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    super::super::camera::set_near(&mut r.game.camera.cam, 1.0);
    super::super::camera::set_far(&mut r.game.camera.cam, 4000.0);
}

/// `grInishie2_801FCBC0` does nothing (only a demo runs it).
#[doc(alias = "grInishie2_801FCBC0")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grInishie2_801FCCAC` does nothing.
#[doc(alias = "grInishie2_801FCCAC")]
pub fn on_load(_r: &mut Running) {}

/// Mushroom Kingdom II's start, `grInishie2_801FCCB0`: the enemy spawner, without enemies.
#[doc(alias = "grInishie2_801FCCB0")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The scenery's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@801FCDC8")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 2), `grInishie2_801FCE04`: the waterfall's three animations (parts
/// 1, 6 and 5), no callbacks, the logs', Birdo's and Pidgit's waits, the stage's guide points
/// (parts 9, 12, 8, 11, 10 and 13), the music not yet sped up.
#[doc(alias = "grInishie2_801FCE04")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for (part, anim) in [(1, 0), (6, 1), (5, 2)] {
        anime::start(r.game, r.data, gobj, part, 7, anim, 0.0, 1.0, true);
    }
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    init_logs(r, gobj);
    init_birdo_wait(r, gobj);
    init_pidgit_wait(r, gobj);
    let joints = [9, 0xC, 8, 0xB, 0xA, 0xD].map(|k| nth_joint(r.game, gobj, k));
    super::ground::set_guide_points(r.game, r.data, joints);
    r.game.grounds[g.0].u.inishie2().xC4 &= !MUSIC;
}

/// The music under 20 seconds left, `grInishie2_801FCF2C` (a copy of Mushroom Kingdom's,
/// `grInishie1_801FCB10`), once.
#[doc(alias = "grInishie2_801FCF2C")]
pub fn music(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.inishie2().xC4 & MUSIC != 0 {
        return;
    }
    match r.game.vs_match_timer() {
        Some(left) => {
            if left < 20 {
                let bgm = super::inishie1::fast_song(super::ground::x98(r.game)).unwrap_or(-1);
                r.lb_play_music(bgm);
                r.game.grounds[g.0].u.inishie2().xC4 |= MUSIC;
            }
        }
        None => r.game.grounds[g.0].u.inishie2().xC4 |= MUSIC,
    }
}

/// The stage's process, `grInishie2_801FCFCC`: the logs, Birdo, Pidgit and the music; the
/// wind sources age.
#[doc(alias = "grInishie2_801FCFCC")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    send_logs(r, gobj);
    send_birdo(r, gobj);
    send_pidgit(r, gobj);
    music(r, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The logs' waits, `grInishie2_801FD018`: both running, each random.
#[doc(alias = "grInishie2_801FD018")]
pub fn init_logs(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let pa = param(r.game);
    r.game.grounds[g.0].u.inishie2().xC4 |= LOG_WAITS[0] | LOG_WAITS[1];
    let w = wait(r, pa.h(r.data, p::LOG1_WAIT), pa.h(r.data, p::LOG1_SPREAD));
    r.game.grounds[g.0].u.inishie2().xC6 = w;
    let w = wait(r, pa.h(r.data, p::LOG2_WAIT), pa.h(r.data, p::LOG2_SPREAD));
    r.game.grounds[g.0].u.inishie2().xC8 = w;
}

/// Sending the logs, `grInishie2_801FD0B4`: each wait that runs out sends its log, its
/// collision joint off, and stops until the log is gone.
#[doc(alias = "grInishie2_801FD0B4")]
pub fn send_logs(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    for k in 0..2 {
        let v = r.game.grounds[g.0].u.inishie2();
        if v.xC4 & LOG_WAITS[k] == 0 {
            continue;
        }
        let w = if k == 0 { &mut v.xC6 } else { &mut v.xC8 };
        let timer = *w;
        *w = timer.wrapping_sub(1);
        if timer < 0 {
            let log = create(r, MAP_LOGS[k], LOG_FRAME);
            r.game.geometry_mut().disable_joint(LOG_JOINTS[k]);
            r.game.grounds[g.0].u.inishie2().xC4 &= !LOG_WAITS[k];
            if let Some(log) = log {
                let lg = ground_of(r.game, log);
                r.game.grounds[lg.0].u.inishie2_log().xCC = Some(gobj);
            }
        }
    }
}

/// A log's setup (maps 3 and 4), `grInishie2_801FD198`: its collision and animation, its
/// collision not yet back, which log it is and its wait for it.
#[doc(alias = "grInishie2_801FD198")]
pub fn init_log(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let which = (m - 3) as i16;
    let on = param(r.game).h(r.data, p::LOG_ON + 2 * which as u32);
    let v = r.game.grounds[g.0].u.inishie2_log();
    v.xC4 &= !LOG_ON;
    v.xC8 = which;
    v.xC6 = on;
}

/// A log's process, `grInishie2_801FD224`: its collision comes back once its wait runs out,
/// and follows the model; its animation over, the stage's wait for it starts again and it's
/// gone.
#[doc(alias = "grInishie2_801FD224")]
pub fn proc_log(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.inishie2_log();
    if v.xC4 & LOG_ON == 0 {
        let counter = v.xC6;
        v.xC6 = counter.wrapping_sub(1);
        if counter < 0 {
            let which = v.xC8;
            r.game
                .geometry_mut()
                .enable_joint(if which == 0 { 0xC } else { 0xD });
            r.game.grounds[g.0].u.inishie2_log().xC4 |= LOG_ON;
        }
    }
    update_map_coll(r.game, r.data, gobj);
    if anime::finished(r.game, gobj, 0, 1) {
        let v = r.game.grounds[g.0].u.inishie2_log();
        let (stage_gobj, which) = (v.xCC, v.xC8);
        let sg = ground_of(r.game, stage_gobj.expect("a log's stage object"));
        let pa = param(r.game);
        if which == 0 {
            r.game.grounds[sg.0].u.inishie2().xC4 |= LOG_WAITS[0];
            let w = wait(r, pa.h(r.data, p::LOG1_WAIT), pa.h(r.data, p::LOG1_SPREAD));
            r.game.grounds[sg.0].u.inishie2().xC6 = w;
        } else {
            r.game.grounds[sg.0].u.inishie2().xC4 |= LOG_WAITS[1];
            let w = wait(r, pa.h(r.data, p::LOG2_WAIT), pa.h(r.data, p::LOG2_SPREAD));
            r.game.grounds[sg.0].u.inishie2().xC8 = w;
        }
        setup::destroy(r, Some(gobj));
    }
}

/// Birdo's wait, `grInishie2_801FD368`: running, random.
#[doc(alias = "grInishie2_801FD368")]
pub fn init_birdo_wait(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let pa = param(r.game);
    r.game.grounds[g.0].u.inishie2().xC4 |= BIRDO_WAIT;
    let w = wait(
        r,
        pa.h(r.data, p::BIRDO_WAIT),
        pa.h(r.data, p::BIRDO_SPREAD),
    );
    r.game.grounds[g.0].u.inishie2().xCA = w;
}

/// Sending Birdo, `grInishie2_801FD3CC`: once her wait runs out, her platform comes, and the
/// wait stops until she's gone.
#[doc(alias = "grInishie2_801FD3CC")]
pub fn send_birdo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.inishie2();
    if v.xC4 & BIRDO_WAIT == 0 {
        return;
    }
    let counter = v.xCA;
    v.xCA = counter.wrapping_sub(1);
    if counter < 0 {
        let platform = create(r, MAP_BIRDO, SEND_FRAME);
        r.game.grounds[g.0].u.inishie2().xC4 &= !BIRDO_WAIT;
        if let Some(platform) = platform {
            let pg = ground_of(r.game, platform);
            r.game.grounds[pg.0].u.inishie2_birdo().xC8 = Some(gobj);
        }
    }
}

/// Birdo gone, `grInishie2_801FD448` (from her destroy callback): the stage's wait for her
/// starts again and her platform goes.
#[doc(alias = "grInishie2_801FD448")]
pub fn birdo_gone(r: &mut Running, platform: GObjId) {
    let g = ground_of(r.game, platform);
    let stage_gobj = r.game.grounds[g.0].u.inishie2_birdo().xC8;
    let sg = ground_of(r.game, stage_gobj.expect("Birdo's platform's stage object"));
    let pa = param(r.game);
    r.game.grounds[sg.0].u.inishie2().xC4 |= BIRDO_WAIT;
    let w = wait(
        r,
        pa.h(r.data, p::BIRDO_WAIT),
        pa.h(r.data, p::BIRDO_SPREAD),
    );
    r.game.grounds[sg.0].u.inishie2().xCA = w;
    setup::destroy(r, Some(platform));
}

/// Birdo knocked off, `grInishie2_801FD4CC`: her platform's collision joint out of the search.
#[doc(alias = "grInishie2_801FD4CC")]
pub fn birdo_down(r: &mut Running, _platform: GObjId) {
    r.game.geometry_mut().disable_joint(BIRDO_JOINT);
}

/// Birdo's platform's setup (map 15), `grInishie2_801FD4F0`: a random side; its collision
/// joint in the search; beyond that side's blast zone, at the parameters' place, with Birdo
/// there facing in; its collision where it stands.
#[doc(alias = "grInishie2_801FD4F0")]
pub fn init_birdo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let side = r.game.objs.rng.below(2);
    r.game.geometry_mut().enable_joint(BIRDO_JOINT);
    let from = param(r.game).v(r.data, p::BIRDO_FROM + 0xC * side as u32);
    let edge = if side == 0 {
        stage::blast_right(r.game)
    } else {
        stage::blast_left(r.game)
    };
    let pos = Vec3::new(mem(fp::fadds(reg(from.x), reg(edge))), from.y, from.z);
    jobj::set_translate(
        &mut r.game.objs.jobjs,
        j.expect("Birdo's platform's model"),
        pos,
    );
    let facing = if side == 0 { -1.0 } else { 1.0 };
    let birdo = super::super::it::kinds::kyasarin::spawn(r, gobj, pos, facing);
    r.game.grounds[g.0].u.inishie2_birdo().xC4 = birdo;
    let m = map_id(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
}

/// Birdo's platform's process, `grInishie2_801FD654`: while she's there, it stands her
/// platform's height above her; its collision follows the model.
#[doc(alias = "grInishie2_801FD654")]
pub fn proc_birdo(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    if let Some(birdo) = r.game.grounds[g.0].u.inishie2_birdo().xC4 {
        let mut pos = super::super::it::kinds::kyasarin::position(r.game, birdo);
        let up = param(r.game).f(r.data, p::BIRDO_PLATFORM);
        pos.y = mem(fp::fadds(reg(pos.y), reg(up)));
        jobj::set_translate(
            &mut r.game.objs.jobjs,
            j.expect("Birdo's platform's model"),
            pos,
        );
    }
    update_map_coll(r.game, r.data, gobj);
}

/// Pidgit's wait, `grInishie2_801FD744`: running, random.
#[doc(alias = "grInishie2_801FD744")]
pub fn init_pidgit_wait(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let pa = param(r.game);
    r.game.grounds[g.0].u.inishie2().xC4 |= PIDGIT_WAIT;
    let w = wait(
        r,
        pa.h(r.data, p::PIDGIT_WAIT),
        pa.h(r.data, p::PIDGIT_SPREAD),
    );
    r.game.grounds[g.0].u.inishie2().xCC = w;
}

/// Sending Pidgit, `grInishie2_801FD7A8`: once his wait runs out, his carpet comes, and the
/// wait stops until it's gone.
#[doc(alias = "grInishie2_801FD7A8")]
pub fn send_pidgit(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.inishie2();
    if v.xC4 & PIDGIT_WAIT == 0 {
        return;
    }
    let counter = v.xCC;
    v.xCC = counter.wrapping_sub(1);
    if counter < 0 {
        let carpet = create(r, MAP_PIDGIT, SEND_FRAME);
        r.game.grounds[g.0].u.inishie2().xC4 &= !PIDGIT_WAIT;
        if let Some(carpet) = carpet {
            let cg = ground_of(r.game, carpet);
            r.game.grounds[cg.0].u.inishie2_pidgit().xC4 = Some(gobj);
        }
    }
}

/// Pidgit's carpet coming in, `grInishie2_801FD824`: a random side; its collision joint in
/// the search; beyond that side's blast zone, at the parameters' place; heading for the other
/// side (its animation), coming in, no turns yet; its part 0 there.
#[doc(alias = "grInishie2_801FD824")]
pub fn pidgit_in(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let side = r.game.objs.rng.below(2);
    r.game.geometry_mut().enable_joint(PIDGIT_JOINT);
    let from = param(r.game).v(r.data, p::PIDGIT_FROM + 0xC * side as u32);
    let edge = if side == 0 {
        stage::blast_right(r.game)
    } else {
        stage::blast_left(r.game)
    };
    let pos = Vec3::new(mem(fp::fadds(reg(from.x), reg(edge))), from.y, from.z);
    let v = r.game.grounds[g.0].u.inishie2_pidgit();
    if side == 0 {
        v.xC8 |= SIDE;
    } else {
        v.xC8 &= !SIDE;
    }
    let anim = i32::from(v.xC8 & SIDE != 0);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, anim);
    let v = r.game.grounds[g.0].u.inishie2_pidgit();
    v.xC8 |= COMING_IN;
    v.xC8 &= !(LEAVING | GOING);
    v.xCA = 0;
    v.xD8 = pos;
    let part = nth_joint(r.game, gobj, 0).expect("Pidgit's carpet's part 0");
    jobj::set_translate(&mut r.game.objs.jobjs, part, pos);
}

/// Where Pidgit's carpet is against the camera's edges: 2 past the right, 1 past the left, 0
/// between.
fn past_edge(game: &Game, x: f32) -> u8 {
    if reg(x) > reg(stage::cam_right(game)) {
        2
    } else if reg(x) < reg(stage::cam_left(game)) {
        1
    } else {
        0
    }
}

/// Pidgit's carpet's flight, `grInishie2_801FD9EC`. Each time its animation's loop ends: coming
/// in, it stops coming in once it's between the camera's edges; else, unless on its way out,
/// it turns about (its first two loops always; after them, once past an edge it heads out
/// that way and leaves, else it turns at the parameters' odds); its animation starts again
/// for its heading, and its part 0 moves on by the loop's step, its place kept. Between loops,
/// the step is part 1's place.
#[doc(alias = "grInishie2_801FD9EC")]
pub fn pidgit_fly(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if anime::finished(r.game, gobj, 1, 1) {
        let v = r.game.grounds[g.0].u.inishie2_pidgit();
        let x = mem(fp::fadds(reg(v.xD8.x), reg(v.xCC.x)));
        if v.xC8 & COMING_IN != 0 {
            if past_edge(r.game, x) == 0 {
                r.game.grounds[g.0].u.inishie2_pidgit().xC8 &= !COMING_IN;
            }
        } else {
            let v = r.game.grounds[g.0].u.inishie2_pidgit();
            if v.xC8 & GOING == 0 {
                if v.xCA < 2 {
                    v.xC8 ^= SIDE;
                } else {
                    let past = past_edge(r.game, x);
                    if past != 0 {
                        let v = r.game.grounds[g.0].u.inishie2_pidgit();
                        v.xC8 |= GOING;
                        // A one-bit field: 2 (past the right) keeps 0.
                        if past & 1 != 0 {
                            v.xC8 |= SIDE;
                        } else {
                            v.xC8 &= !SIDE;
                        }
                        v.xC8 |= LEAVING;
                    } else {
                        let pa = param(r.game);
                        let keep = pa.h(r.data, p::PIDGIT_KEEP);
                        let turn = pa.h(r.data, p::PIDGIT_TURN);
                        let total = i32::from(keep).wrapping_add(i32::from(turn)) as i16;
                        let k = if total != 0 {
                            r.game.objs.rng.below(i32::from(total))
                        } else {
                            0
                        };
                        if k >= i32::from(keep) {
                            r.game.grounds[g.0].u.inishie2_pidgit().xC8 ^= SIDE;
                        }
                    }
                }
            }
            let v = r.game.grounds[g.0].u.inishie2_pidgit();
            v.xCA = v.xCA.wrapping_add(1);
        }
        let v = r.game.grounds[g.0].u.inishie2_pidgit();
        let anim = i32::from(v.xC8 & SIDE != 0);
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, anim);
        let v = r.game.grounds[g.0].u.inishie2_pidgit();
        let (place, step) = (v.xD8, v.xCC);
        let part = nth_joint(r.game, gobj, 0).expect("Pidgit's carpet's part 0");
        let jobjs = &mut r.game.objs.jobjs;
        jobj::set_translate(jobjs, part, place);
        jobj::set_srt(jobjs, part, |o| {
            o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(step.x)))
        });
        jobj::set_srt(jobjs, part, |o| {
            o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(step.y)))
        });
        jobj::set_srt(jobjs, part, |o| {
            o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(step.z)))
        });
        let t = jobjs[part].translate;
        r.game.grounds[g.0].u.inishie2_pidgit().xD8 = t;
        return;
    }
    let part = nth_joint(r.game, gobj, 1).expect("Pidgit's carpet's part 1");
    let t = r.game.objs.jobjs[part].translate;
    r.game.grounds[g.0].u.inishie2_pidgit().xCC = t;
}

/// Pidgit's carpet's setup (map 1), `grInishie2_801FDE8C`: coming in, its collision.
#[doc(alias = "grInishie2_801FDE8C")]
pub fn init_pidgit(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    pidgit_in(r, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
}

/// Pidgit's carpet's process, `grInishie2_801FDED8`: its collision follows the model, it
/// flies; once leaving and past a blast zone (its part 0), the stage's wait for it starts
/// again and it's gone.
#[doc(alias = "grInishie2_801FDED8")]
pub fn proc_pidgit(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    pidgit_fly(r, gobj);
    let g = ground_of(r.game, gobj);
    let part = nth_joint(r.game, gobj, 0);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, part, None);
    let out =
        reg(pos.x) > reg(stage::blast_right(r.game)) || reg(pos.x) < reg(stage::blast_left(r.game));
    let v = r.game.grounds[g.0].u.inishie2_pidgit();
    if out && v.xC8 & LEAVING != 0 {
        let stage_gobj = v.xC4.expect("Pidgit's carpet's stage object");
        let sg = ground_of(r.game, stage_gobj);
        let pa = param(r.game);
        r.game.grounds[sg.0].u.inishie2().xC4 |= PIDGIT_WAIT;
        let base = pa.h(r.data, p::PIDGIT_WAIT);
        let spread = pa.h(r.data, p::PIDGIT_SPREAD);
        let k = random_below(r, i32::from(spread));
        r.game.grounds[sg.0].u.inishie2().xCC = i32::from(base).wrapping_add(k) as i16;
        setup::destroy(r, Some(gobj));
    }
}

/// A random draw below `n`, none when it's 0, `grInishie2_801FDFB4`.
#[doc(alias = "grInishie2_801FDFB4")]
pub fn random_below(r: &mut Running, n: i32) -> i32 {
    if n != 0 { r.game.objs.rng.below(n) } else { 0 }
}

/// The stage's shadow check, `grInishie2_801FDFF0`: every joint takes the shadows.
#[doc(alias = "grInishie2_801FDFF0")]
pub fn check_shadow_render() -> bool {
    true
}
