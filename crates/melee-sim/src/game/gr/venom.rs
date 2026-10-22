//! Venom, `melee/gr/grvenom.c`: the Great Fox flying through Venom's canyon (map 7, its model and
//! animation the whole stage's), with the platforms the fighters stand on riding it (map 5, its
//! collision joint copied by map 0), the background (map 4), the second collision (map 9), the
//! Arwings (map 2 a flight path, maps 3 and 6 the Arwing flying it, maps 10 to 15 their wings)
//! and the Star Fox team's talk (map 8, its window map 1), which Corneria's code plays
//! (`gr::corneria`).
//!
//! The Great Fox's animation is the stage's clock: at set frames it sounds, raises its lights'
//! colors and turns the Arwings' groups on and off, and under the canyon's two tunnels it opens
//! its lower deck (collision joint 2) and turns the sounds' send. Arwings fly in one at a time
//! while no group is held back, the close ones (types 1 to 7) standing in for platforms through
//! their wings, the far ones (8 to 11) firing lasers (`it::kinds::arwinglaser`) at a fighter on
//! their side of the stage.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::it::kinds::arwinglaser;
use super::super::state::{Game, Running};
use super::corneria::{Stage, update_sequence, update_wing_visibility};
use super::data::{StageCallbacks, VENOM_CALLBACKS};
use super::ground::{
    ParamColor, ground_of, jobj_of, map_gobj, nth_joint, scale, set_param_color, update_map_coll,
};
use super::{anime, grlib, material, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::game::structs::_GXColor;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// The fields of `grVenom_YakumonoParam`, by offset.
mod p {
    /// The first Arwing's wait (between these two), the waits after (between these two), and
    /// the chance of the far group.
    pub const ARWING_FIRST_MIN: u32 = 0;
    pub const ARWING_FIRST_MAX: u32 = 4;
    pub const ARWING_WAIT_MIN: u32 = 8;
    pub const ARWING_WAIT_MAX: u32 = 0xC;
    pub const ARWING_FAR: u32 = 0x10;
    /// An Arwing's wait between rolls.
    pub const ROLL_WAIT: u32 = 0x2C;
    /// The Arwings' scale.
    pub const ARWING_SCALE: u32 = 0x34;
    /// The color script an Arwing's shot flashes it with.
    pub const FIRE_SCRIPT: u32 = 0x38;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn i(self, data: &Archives, off: u32) -> i32 {
        fp::fctiwz(reg(self.f(data, off)))
    }
}

fn param(game: &Game) -> Param {
    Param(game.gr.venom_yakumono.expect("grvenom.c: yakumono_param"))
}

/// grvenom.c's statics besides its `yakumono_param`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The three Arwing slots' flight paths (`grVe_803E5348.arwing.arwing_gobj`), types
    /// (`arwing_type`: 1 to 7 close, 8 to 11 far) and groups (`grVe_803E5380`: 1 to 3 by slot,
    /// 4 the far one).
    pub arwings: [Option<GObjId>; 3],
    pub types: [i32; 3],
    pub groups: [i32; 3],
    /// The slot being made (`grVe_804D6A34`).
    pub slot: u32,
    /// The wait before the next Arwing (`grVe_804D6A38`).
    pub wait: i32,
    /// Whether the demo plays (`grVe_804D6A40`).
    pub demo: i32,
}

/// Whether the Arwings hold still (`grVe_804D6A3C`): nothing writes it, so it stays the zero
/// `.sbss` starts with.
const FROZEN: bool = false;

fn g(game: &mut Game) -> &mut Globals {
    &mut game.gr.venom
}

/// A wait drawn between two of the parameters (`grVe_RandRange`).
fn rand_range(r: &mut Running, lo: u32, hi: u32) -> i32 {
    let p = param(r.game);
    let (lo, hi) = (p.i(r.data, lo), p.i(r.data, hi));
    grlib::rand_range(&mut r.game.objs.rng, lo, hi)
}

/// The group's map (`grVe_803E5348` + 0x2A8, by group): the close groups' Arwing (map 3) and the
/// far group's (map 6).
const GROUP_MAPS: [i32; 5] = [3, 3, 3, 3, 6];

/// The close groups' wings' maps (`grVe_803E566C`).
const GROUP_WINGS: [i32; 5] = [0xA, 0xA, 0xB, 0xC, 0xD];

/// The wings' collision joints by group (`grVe_803E5680`).
const GROUP_JOINTS: [i32; 5] = [4, 4, 4, 4, 3];

/// The part an Arwing rolls by group (`grVe_803E56A0`).
const GROUP_PARTS: [i32; 6] = [1, 1, 1, 1, 1, 0];

/// The roll animations, by roll and the group's fire kind plus one (`grVe_803E5644`, which the
/// original indexes from the roll's pair with the fire kind, -1 for none).
const ROLL_ANIMS: [i32; 10] = [-1, -1, 1, 0, 2, 2, 4, 4, 5, 5];

/// Each type's offset from its path's joint 5 (`grVe_803E5348` + 0x218, by type): all zero.
const TYPE_OFFSETS: [Vec3; 12] = [Vec3::new(0.0, 0.0, 0.0); 12];

/// Each map's animation by type (`grVe_803E5530`, by type): -1 for none.
const TYPE_ANIMS: [i32; 12] = [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/// A stage object's process or callback, by address: Venom's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_3FCC | 0x8020_3FD0 | 0x8020_40AC | 0x8020_40B0 | 0x8020_4424 | 0x8020_4B84
        | 0x8020_4CE0 | 0x8020_4F1C | 0x8020_56AC | 0x8020_5AD0 | 0x8020_6870 | 0x8020_6B90
        | 0x8020_6BEC => empty(r, gobj),
        0x8020_3F98 => init_copy(r, gobj),
        0x8020_3FD4 => init_background(r, gobj),
        START_HOOK => platform_start(r, gobj),
        0x8020_40F0 => init_platform(r, gobj),
        0x8020_4284 => proc_platform(r, gobj),
        0x8020_4428 => init_ship(r, gobj),
        0x8020_454C => proc_ship(r, gobj),
        0x8020_4CEC => proc_talk(r, gobj),
        0x8020_4DB0 => talk_destroyed(r, gobj),
        0x8020_4DD4 => init_coll(r, gobj),
        0x8020_4EFC => proc_coll(r, gobj),
        0x8020_4F20 => init_path(r, gobj),
        0x8020_53B0 => proc_path(r, gobj),
        0x8020_56B0 => init_wing(r, gobj),
        0x8020_5758 => proc_wing(r, gobj),
        0x8020_5AD4 => init_arwing(r, gobj),
        0x8020_5F30 => proc_arwing(r, gobj),
        0x8020_6874 => init_far_arwing(r, gobj),
        0x8020_6B70 => proc_far_arwing(r, gobj),
        0x8020_6B94 => init_window(r, gobj),
        0x8020_6BC4 => proc_window(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_3B18 => on_init(r),
        0x8020_3DD0 => on_load(r),
        0x8020_3E80 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grVenom_80203FCC")]
#[doc(alias = "grVenom_80203FD0")]
#[doc(alias = "grVenom_802040AC")]
#[doc(alias = "grVenom_802040B0")]
#[doc(alias = "grVenom_80204424")]
#[doc(alias = "grVenom_80204B84")]
#[doc(alias = "grVenom_80204CE0")]
#[doc(alias = "grVenom_80204F1C")]
#[doc(alias = "grVenom_802056AC")]
#[doc(alias = "grVenom_80205AD0")]
#[doc(alias = "grVenom_80206870")]
#[doc(alias = "grVenom_80206B90")]
#[doc(alias = "grVenom_80206BEC")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

fn gp_map(game: &Game, gobj: GObjId) -> i32 {
    let gid = ground_of(game, gobj);
    game.grounds[gid.0].map_id
}

/// The demo's setup, `grVenom_80203B14`: nothing.
#[doc(alias = "grVenom_80203B14")]
pub fn on_demo_init(_r: &mut Running) {}

/// Where the stage's setup keeps the descriptor of the scale joints of the objects it makes:
/// the other stages' setups' ([`setup::SETUP_FRAME`]) less its frame's extra 0x30 (0x60 against
/// 0x30).
pub const INIT_FRAME: u32 = setup::SETUP_FRAME - 0x30;

/// Venom's setup, `grVenom_80203B18`: its parameters, the background (map 4), the copy (map 0),
/// the Arwing slots and the first wait, the platforms (map 5, copied by map 0), the second
/// collision (map 9) and the Great Fox (map 7) with its animation, camera limits and blast
/// zones, two wind sources, collision joints 3 and 4 off, and the lights' animations looping.
#[doc(alias = "grVenom_80203B18")]
pub fn on_init(r: &mut Running) {
    r.game.gr.venom_yakumono = super::ground::yakumono_param(r.game);
    create(r, 4, INIT_FRAME);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    let copy = create(r, 0, INIT_FRAME);
    {
        let c = g(r.game);
        c.arwings = [None; 3];
        c.types = [0; 3];
        c.groups = [0; 3];
    }
    let demo = i32::from(stage::stkind(r.game) == 0xE9);
    g(r.game).demo = demo;
    g(r.game).wait = if demo == 1 {
        0x78
    } else {
        rand_range(r, p::ARWING_FIRST_MIN, p::ARWING_FIRST_MAX)
    };
    let platform = create(r, 5, INIT_FRAME).expect("grvenom.c: the platforms");
    let pg = ground_of(r.game, platform);
    r.game.grounds[pg.0].u.venom_platform().target = copy;
    create(r, 9, INIT_FRAME);
    let ship = create(r, 7, INIT_FRAME).expect("grvenom.c: the Great Fox");
    anime::start_map(r.game, r.data, ship, 7, 0);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let _ = crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(-1.0, 1.0, -1.0),
        -1,
        1.0,
        0.0,
        std::f32::consts::FRAC_PI_3,
        -100_000.0,
        10_000.0,
        0.0,
        -100_000.0,
    );
    let _ = crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(1.0, 1.0, -1.0),
        -1,
        1.0,
        0.0,
        std::f32::consts::FRAC_PI_3,
        0.0,
        10_000.0,
        100_000.0,
        -100_000.0,
    );
    r.game.geometry_mut().disable_joint(3);
    r.game.geometry_mut().disable_joint(4);
    if let Some(lg) = super::ground::light_gobj(r.game)
        && let Some(super::super::state::Obj::LObj(first)) = r.game.world.gobj(lg).obj
    {
        for a in setup::light_anims(r.game, first, u32::MAX) {
            crate::hsd::aobj::set_flags(&mut r.game.objs.aobjs, Some(a), crate::hsd::aobj::LOOP);
        }
    }
}

/// Venom's load hook, `grVenom_80203DD0`: every animation of the stage's lights (the first light
/// GObj of class 12) loops.
#[doc(alias = "grVenom_80203DD0")]
pub fn on_load(r: &mut Running) {
    let mut cur = r.game.world.first_in(3);
    while let Some(gobj) = cur {
        let o = r.game.world.gobj(gobj);
        if o.classifier == 0xC {
            if let Some(super::super::state::Obj::LObj(first)) = o.obj {
                for a in setup::light_anims(r.game, first, u32::MAX) {
                    crate::hsd::aobj::set_flags(
                        &mut r.game.objs.aobjs,
                        Some(a),
                        crate::hsd::aobj::LOOP,
                    );
                }
            }
            return;
        }
        cur = o.next;
    }
}

/// Venom's start, `grVenom_80203E80`: the enemy spawner, without enemies.
#[doc(alias = "grVenom_80203E80")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// Creates one of the stage's objects and sets it up, `grVenom_80203EAC`.
#[doc(alias = "grVenom_80203EAC")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    let gobj = setup::create_from_callbacks(r, map_id, callbacks(map_id), frame);
    if gobj.is_none() {
        crate::game::code::os_report();
    }
    gobj
}

/// The stage's objects' setup functions and processes, by map id (`grVe_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &VENOM_CALLBACKS[map_id as usize]
}

/// The copy's setup (map 0), `stageGObj0_OnInit` (`Ground_StartMapAnim`): its animation.
#[doc(alias = "stageGObj0_OnInit@80203F98")]
pub fn init_copy(r: &mut Running, gobj: GObjId) {
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The background's setup (map 4), `grVenom_80203FD4`: its animation, drawn in the background
/// layer, the guide points, and its own lights off.
#[doc(alias = "grVenom_80203FD4")]
pub fn init_background(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[gid.0].x11_flags.b012 = 2;
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let points = [n(r, 1), n(r, 4), n(r, 3), n(r, 2), n(r, 5), n(r, 6)];
    super::ground::set_guide_points(r.game, r.data, points);
    r.game.grounds[gid.0].x10_flags.b2 = false;
}

/// The platforms' start hook (`fn_802040B4`).
const START_HOOK: u32 = 0x8020_40B4;

/// The platforms' start hook, `fn_802040B4`: on the event stage (stage kind 0xE4) the window's
/// line about to start.
#[doc(alias = "fn_802040B4")]
pub fn platform_start(r: &mut Running, gobj: GObjId) {
    if stage::stkind(r.game) == 0xE4 {
        let gid = ground_of(r.game, gobj);
        r.game.grounds[gid.0].u.venom_platform().timer = 1;
    }
}

/// The platforms' setup (map 5), `grVenom_802040F0`: their collision, their animations, the
/// start hook, no line coming, the upper and lower platforms' joints (the higher one upper),
/// and its collision kept with its model.
#[doc(alias = "grVenom_802040F0")]
pub fn init_platform(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start(r.game, r.data, gobj, 0, 7, 0, 0.0, 1.0, true);
    anime::start(r.game, r.data, gobj, 0, 7, 1, 0.0, 1.0, false);
    for part in [0xB, 0xD, 0xF] {
        anime::start(r.game, r.data, gobj, part, 7, 2, 0.0, 1.0, true);
    }
    for part in [4, 7] {
        anime::start(r.game, r.data, gobj, part, 7, 3, 0.0, 1.0, true);
    }
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
    let (upper, lower) = (nth_joint(r.game, gobj, 2), nth_joint(r.game, gobj, 3));
    {
        let v = r.game.grounds[gid.0].u.venom_platform();
        v.timer = -1;
        v.upper = upper;
        v.lower = lower;
    }
    let a = world_position(&mut r.game.objs, r.data, upper, None);
    let b = world_position(&mut r.game.objs, r.data, lower, None);
    if reg(b.y) > reg(a.y) {
        let v = r.game.grounds[gid.0].u.venom_platform();
        std::mem::swap(&mut v.upper, &mut v.lower);
    }
    r.game.grounds[gid.0].x10_flags.b5 = true;
}

/// The stack address of the scale joint's descriptor for the event's window (map 1), which the
/// platforms' process makes: below the process loop by its frame (0x40) and the create
/// function's.
pub const EVENT_WINDOW_FRAME: u32 = super::corneria::PROC_LOOP - 0x40 - 0x20;

/// The platforms' process (map 5), `grVenom_80204284`: the copy (map 0) at its place, camera
/// limits and blast zones, the event's line (a second after the start hook, the players'
/// displays hidden until its window goes), the wind sources, the Arwings and its collision.
#[doc(alias = "grVenom_80204284")]
pub fn proc_platform(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grvenom.c: jobj");
    let target = r.game.grounds[gid.0].u.venom_platform().target;
    let pos = r.game.objs.jobjs[j].translate;
    let tj = target
        .and_then(|t| jobj_of(r.game, t))
        .expect("grvenom.c: the copy's jobj");
    jobj::set_translate(&mut r.game.objs.jobjs, tj, pos);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let timer = r.game.grounds[gid.0].u.venom_platform().timer;
    if timer > 0 {
        let t = timer.wrapping_add(1);
        r.game.grounds[gid.0].u.venom_platform().timer = t;
        if t >= 0x3C {
            r.game.hide_damage_digits();
            r.game.hide_scores();
            if t == 0x3C {
                if let Some(w) = create(r, 1, EVENT_WINDOW_FRAME) {
                    super::corneria::start_line(r, w, 4, 6, 0x6B6CC);
                }
            } else if map_gobj(r.game, 1).is_none() {
                r.game.show_damage_digits();
                r.show_scores();
                r.game.grounds[gid.0].u.venom_platform().timer = -1;
            }
        }
    }
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
    spawn_arwings(r);
    update_map_coll(r.game, r.data, gobj);
}

/// The Great Fox's setup (map 7), `grVenom_80204428`: its seven joints whose visibility it
/// keeps, its flags clear, its animation's frame 0, its hum, and its lower deck's collision
/// (joint 2) off.
#[doc(alias = "grVenom_80204428")]
pub fn init_ship(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let joints = [4, 0xE, 0x10, 0x11, 0x12, 0x13, 0x15].map(|k| nth_joint(r.game, gobj, k));
    {
        let v = r.game.grounds[gid.0].u.venom_ship();
        v.jobjs = joints;
        for b in 0..7 {
            v.set_flag(b, false);
        }
        v.frame = 0.0;
    }
    r.object_sound(Some(gid), 0, 0x6B6C3);
    r.game.geometry_mut().disable_joint(2);
}

/// Whether the Great Fox's animation crossed `at` this tick: last tick's frame below it, this
/// tick's at or past it.
fn crossed(last: f32, frame: f32, at: f32) -> bool {
    reg(last) < reg(at) && reg(at) <= reg(frame)
}

/// The Great Fox's process (map 7), `grVenom_8020454C`: its joints shown while their depth is
/// a number, the lights' animations restarted with its own, its animation's cues (sounds, the
/// Arwings' groups held and let go, the lower deck's collision and hum under the tunnels), the
/// sounds' send, and the stage's colors.
#[doc(alias = "grVenom_8020454C")]
pub fn proc_ship(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let joints = r.game.grounds[gid.0].u.venom_ship().jobjs;
    for j in joints.into_iter().flatten() {
        let pos = world_position(&mut r.game.objs, r.data, Some(j), None);
        let z = reg(pos.z);
        let visible = !(!(z < 1000.0) && !(z > -60000.0));
        let hidden = r.game.objs.jobjs[j].flags & jobj::HIDDEN != 0;
        if hidden {
            if visible {
                jobj::clear_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
            }
        } else if !visible {
            jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
        }
    }
    if anime::rewound(r.game, gobj, 0, 7)
        && let Some(lg) = super::ground::light_gobj(r.game)
        && let Some(super::super::state::Obj::LObj(first)) = r.game.world.gobj(lg).obj
    {
        for a in setup::light_anims(r.game, first, u32::MAX) {
            let objs = &mut r.game.objs;
            crate::hsd::aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, Some(a), 0.0);
        }
    }
    if let Some(a) = anime::first_anim(r.game, gobj, 0, 7) {
        let frame = r.game.objs.aobjs[a].curr_frame;
        let last = r.game.grounds[gid.0].u.venom_ship().frame;
        let any = |ats: &[f32]| ats.iter().any(|&at| crossed(last, frame, at));
        if any(&[1033.0, 1086.0, 1185.0, 1238.0]) {
            r.stage_sound(0x6B6C5);
        }
        if any(&[1063.0, 1116.0, 1215.0, 1268.0]) {
            r.stage_sound(0x6B6C6);
        }
        if any(&[2460.0, 6440.0, 9330.0]) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(6, true);
        }
        if any(&[
            2460.0, 2747.0, 3030.0, 3393.0, 3727.0, 6440.0, 6880.0, 9330.0, 9600.0, 9835.0,
        ]) {
            r.stage_sound(0x6B6C7);
        }
        if any(&[4070.0, 7220.0]) || reg(last) > reg(frame) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(6, false);
        }
        if crossed(last, frame, 4200.0) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(0, true);
        }
        if crossed(last, frame, 3800.0) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(1, true);
        }
        if crossed(last, frame, 4400.0) {
            r.object_sound(Some(gid), 0, 0x6B6C4);
            r.game.grounds[gid.0].u.venom_ship().set_flag(2, true);
            r.game.geometry_mut().enable_joint(2);
        }
        if crossed(last, frame, 6100.0) {
            r.object_sound(Some(gid), 0, 0x6B6C3);
            let v = r.game.grounds[gid.0].u.venom_ship();
            v.set_flag(0, false);
            v.set_flag(1, false);
            v.set_flag(2, false);
            r.game.geometry_mut().disable_joint(2);
        }
        if crossed(last, frame, 7210.0) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(3, true);
        }
        if crossed(last, frame, 6810.0) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(4, true);
        }
        if crossed(last, frame, 7410.0) {
            r.game.grounds[gid.0].u.venom_ship().set_flag(5, true);
        }
        if crossed(last, frame, 9200.0) {
            let v = r.game.grounds[gid.0].u.venom_ship();
            v.set_flag(3, false);
            v.set_flag(4, false);
            v.set_flag(5, false);
        }
        r.game.grounds[gid.0].u.venom_ship().frame = frame;
    }
    let deck = r.game.grounds[gid.0].u.venom_ship().flag(2);
    r.set_ground_send(if deck { 2 } else { 1 });
    ship_colors(r, gobj);
}

/// The stage's colors in the tunnels (`grVe_804DB7D8` to `grVe_804DB7E4`): black under the lower
/// deck's, red when flag 6 is up, else the canyon's two.
const BLACK: [u8; 4] = [0, 0, 0, 0];
const RED: [u8; 4] = [190, 10, 0, 0];
const CANYON: [u8; 4] = [110, 60, 25, 0];
const CANYON_2: [u8; 4] = [110, 90, 0, 0];

fn color([r, g, b, a]: [u8; 4]) -> _GXColor {
    _GXColor { r, g, b, a }
}

/// The stage parameters' colors by the Great Fox's flags, `grVenom_80204B88`: once per change,
/// all of them black under the lower deck's tunnel (flag 2), `xD4` red with flag 6, else the
/// canyon's.
#[doc(alias = "grVenom_80204B88")]
pub fn ship_colors(r: &mut Running, gobj: GObjId) {
    use ParamColor::*;
    let gid = ground_of(r.game, gobj);
    let v = r.game.grounds[gid.0].u.venom_ship();
    let (deck, red, state) = (v.flag(2), v.flag(6), v.state());
    if deck {
        if state != 1 {
            for which in [B8, BC, C0, C4, C8, CC, D0, D4, D8] {
                set_param_color(r.game, which, color(BLACK));
            }
            r.game.grounds[gid.0].u.venom_ship().set_state(1);
        }
    } else if red {
        if state != 2 {
            set_param_color(r.game, D4, color(RED));
            r.game.grounds[gid.0].u.venom_ship().set_state(2);
        }
    } else if state != 0 {
        for which in [B8, BC, C0, C4, C8, CC] {
            set_param_color(r.game, which, color(CANYON));
        }
        for which in [D0, D4, D8] {
            set_param_color(r.game, which, color(CANYON_2));
        }
        r.game.grounds[gid.0].u.venom_ship().set_state(0);
    }
}

/// The talk's process (map 8), `grVenom_80204CEC`: Corneria's (`Ground_UpdateStarFoxSequence`)
/// with Venom's window (map 1).
#[doc(alias = "grVenom_80204CEC")]
pub fn proc_talk(r: &mut Running, gobj: GObjId) {
    update_sequence(r, gobj, 1, Stage::Venom);
}

/// The talk's destroy callback (map 8), `grVenom_80204DB0`: the players' displays shown again.
#[doc(alias = "grVenom_80204DB0")]
pub fn talk_destroyed(r: &mut Running, _gobj: GObjId) {
    r.game.show_damage_digits();
    r.show_scores();
}

/// The second collision's setup (map 9), `grVenom_80204DD4`: its collision, and its scale 1 in
/// x and y.
#[doc(alias = "grVenom_80204DD4")]
pub fn init_coll(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let j = j.expect("grvenom.c: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.x = 1.0);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = 1.0);
}

/// The second collision's process (map 9), `stageGObj9_GObjProc`: its collision follows it.
#[doc(alias = "stageGObj9_GObjProc@80204EFC")]
pub fn proc_coll(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
}

/// Where the platforms' process runs the Arwings' spawn: the stack address of a flight path's
/// scale joint's descriptor, below the process loop by the platforms' process's frame (0x40),
/// the spawn's (0x68) and the create function's.
pub const PATH_FRAME: u32 = super::corneria::PROC_LOOP - 0x40 - 0x68 - 0x20;

/// An Arwing's, below its path's by the path's setup's frame (0x38) and the create function's.
pub const ARWING_FRAME: u32 = PATH_FRAME - 0x38 - 0x20;

/// A wing's, below its Arwing's by the Arwing's setup's frame (0x48) and the create function's.
pub const WING_FRAME: u32 = ARWING_FRAME - 0x48 - 0x20;

/// The Great Fox's flags that hold the Arwings back: 1 for the close group's (0 to 2), 2 for
/// the far group's (3 to 5).
fn held_groups(game: &mut Game) -> i32 {
    let ship = map_gobj(game, 7).expect("grvenom.c: the Great Fox");
    let gid = ground_of(game, ship);
    let v = game.grounds[gid.0].u.venom_ship();
    let near = v.flag(0) || v.flag(1) || v.flag(2);
    let far = v.flag(3) || v.flag(4) || v.flag(5);
    i32::from(near) | if far { 2 } else { 0 }
}

/// The far group's chance (or the close one after it), as a match picks it.
fn pick_group(r: &mut Running) -> i32 {
    if g(r.game).groups[0] == 4 {
        return 1;
    }
    let chance = reg(param(r.game).f(r.data, p::ARWING_FAR));
    if reg(r.game.objs.rng.draw_f32()) > chance {
        1
    } else {
        4
    }
}

fn send_arwing(r: &mut Running, slot: usize, type_id: i32, group: i32) {
    let c = g(r.game);
    c.slot = slot as u32;
    c.types[slot] = type_id;
    c.groups[slot] = group;
    let a = create(r, 2, PATH_FRAME);
    let c = g(r.game);
    c.arwings[c.slot as usize] = a;
}

/// Sends the next Arwing, `grVenom_8020362C`. In a match one at a time, once the last has gone
/// and its wait runs out: any of the eleven types unlike the last while no group is held, one
/// of the first four while only the far group is held, none while the close group is; in group
/// 1 or, by chance and not twice running, the far group 4; then another wait. In the demo up to
/// three at once.
#[doc(alias = "grVenom_8020362C")]
pub fn spawn_arwings(r: &mut Running) {
    if g(r.game).demo == 0 {
        if g(r.game).arwings[0].is_none() {
            let c = g(r.game);
            c.wait = c.wait.wrapping_sub(1);
            if c.wait <= 0 {
                let n = match held_groups(r.game) {
                    0 => 11,
                    2 => 4,
                    _ => return,
                };
                let mut type_id = g(r.game).types[0];
                while type_id == g(r.game).types[0] {
                    type_id = r.game.objs.rng.below(n).wrapping_add(1);
                }
                let group = pick_group(r);
                send_arwing(r, 0, type_id, group);
            }
        } else {
            g(r.game).wait = rand_range(r, p::ARWING_WAIT_MIN, p::ARWING_WAIT_MAX);
        }
        return;
    }
    let count = (0..3)
        .find(|&i| g(r.game).arwings[i].is_none())
        .unwrap_or(3);
    if count >= 3 {
        g(r.game).wait = 10;
        return;
    }
    let c = g(r.game);
    c.wait = c.wait.wrapping_sub(1);
    if c.wait > 0 {
        return;
    }
    let has_near = (0..3).any(|i| {
        let c = &r.game.gr.venom;
        i != count && (1..=7).contains(&c.types[i]) && c.arwings[i].is_some()
    });
    let pick = |r: &mut Running, n: i32, add: i32| {
        let mut type_id = g(r.game).types[count];
        while g(r.game).types.contains(&type_id) {
            type_id = r.game.objs.rng.below(n).wrapping_add(add);
        }
        type_id
    };
    if !has_near {
        let type_id = pick(r, 11, 1);
        send_arwing(r, count, type_id, count as i32 + 1);
    } else if held_groups(r.game) == 0 {
        let type_id = pick(r, 4, 8);
        send_arwing(r, count, type_id, count as i32 + 1);
    }
}

/// An Arwing's flight path's setup (map 2), `grVenom_80204F20`: it takes its slot, makes its
/// group's Arwing (map 3 or 6) with the same slot, and is scaled (a close type's depth by the
/// Arwings' scale); its animation starts on its first tick.
#[doc(alias = "grVenom_80204F20")]
pub fn init_path(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grvenom.c: jobj");
    let slot = g(r.game).slot;
    r.game.grounds[gid.0].u.arwing_path().xC8 = slot;
    g(r.game).arwings[slot as usize] = Some(gobj);
    let group = g(r.game).groups[slot as usize];
    if let Some(a) = create(r, GROUP_MAPS[group as usize], ARWING_FRAME) {
        let ag = ground_of(r.game, a);
        r.game.grounds[ag.0].x10_flags.b2 = false;
        r.game.grounds[ag.0].u.arwing().xC8 = slot as i32;
    }
    let s = reg(scale(r.game));
    match g(r.game).types[slot as usize] {
        1..=7 => {
            let k = reg(param(r.game).f(r.data, p::ARWING_SCALE));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.x = mem(s));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = mem(s));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
                o.scale.z = mem(fp::fmuls(s, k))
            });
        }
        8..=11 => {
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.x = mem(s));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = mem(s));
            jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.z = mem(s));
        }
        _ => {}
    }
    let v = r.game.grounds[gid.0].u.arwing_path();
    v.xD4 = 1;
    v.xD8 = 0;
}

/// Where a flight path's Arwing is (its joint 5 in the stage, with its type's offset), or the
/// origin without a path: the body of `grVenom_802052E0` and its inline copies.
fn arwing_pos(r: &mut Running, path: Option<GObjId>) -> Vec3 {
    let Some(path) = path else {
        return Vec3::new(0.0, 0.0, 0.0);
    };
    let slot = {
        let pg = ground_of(r.game, path);
        r.game.grounds[pg.0].u.arwing_path().xC8 as usize
    };
    let j = nth_joint(r.game, path, 5);
    let pos = world_position(&mut r.game.objs, r.data, j, None);
    let o = TYPE_OFFSETS[g(r.game).types[slot] as usize];
    Vec3::new(
        mem(fp::fadds(reg(pos.x), reg(o.x))),
        mem(fp::fadds(reg(pos.y), reg(o.y))),
        mem(fp::fadds(reg(pos.z), reg(o.z))),
    )
}

/// Where a flight path's Arwing is, `grVenom_802052E0`.
#[doc(alias = "grVenom_802052E0")]
pub fn path_pos(r: &mut Running, path: Option<GObjId>) -> Vec3 {
    arwing_pos(r, path)
}

/// Whether a point is outside the blast zones, `grVenom_80205DF8` (`Stage_IsOutsideBlastZone`).
#[doc(alias = "grVenom_80205DF8")]
pub fn outside_blast_zones(game: &Game, pos: Vec3) -> bool {
    super::corneria::outside_blast_zones(game, pos)
}

/// Whether a point is within 20 of the blast zones or past them, `grVenom_80205E84`
/// (`Stage_IsOutsideBlastZoneWithMargin`).
#[doc(alias = "grVenom_80205E84")]
pub fn near_blast_zones(game: &Game, pos: Vec3) -> bool {
    let (x, y) = (reg(pos.x), reg(pos.y));
    x > fp::fsubs(reg(stage::blast_right(game)), 20.0)
        || x < fp::fadds(reg(stage::blast_left(game)), 20.0)
        || y > fp::fsubs(reg(stage::blast_top(game)), 20.0)
        || y < fp::fadds(reg(stage::blast_bottom(game)), 20.0)
}

/// A flight path's process (map 2), `grVenom_802053B0`: on its first tick its type's animation
/// starts; then its sounds (its fly-in once inside the blast zones, its fly-off once it nears
/// them again; a far one also turned face on) until its animation ends and it frees its slot
/// and goes. A path whose slot another holds goes at once.
#[doc(alias = "grVenom_802053B0")]
pub fn proc_path(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grvenom.c: jobj");
    let slot = r.game.grounds[gid.0].u.arwing_path().xC8 as usize;
    if g(r.game).arwings[slot] != Some(gobj) {
        setup::destroy(r, Some(gobj));
        return;
    }
    if r.game.grounds[gid.0].u.arwing_path().xD4 == 1 {
        r.game.grounds[gid.0].u.arwing_path().xD4 = 0;
        let anim = TYPE_ANIMS[g(r.game).types[slot] as usize];
        let m = gp_map(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, anim);
        return;
    }
    let ty = g(r.game).types[slot];
    if (1..=11).contains(&ty) {
        if ty >= 8 {
            jobj::set_rotation_y(&mut r.game.objs.jobjs, j, 0.0);
        }
        let pos = arwing_pos(r, Some(gobj));
        let sound = r.game.grounds[gid.0].u.arwing_path().xD8;
        match sound {
            0 => {
                if !outside_blast_zones(r.game, pos) {
                    r.sfx_play(0x6B6C0, 0x7F, 0x40);
                    r.game.grounds[gid.0].u.arwing_path().xD8 = 1;
                }
            }
            1 => {
                if !near_blast_zones(r.game, pos) {
                    r.game.grounds[gid.0].u.arwing_path().xD8 = 2;
                }
            }
            2 if near_blast_zones(r.game, pos) => {
                r.sfx_play(0x6B6C2, 0x7F, 0x40);
                r.game.grounds[gid.0].u.arwing_path().xD8 = 3;
            }
            _ => {}
        }
    }
    if anime::finished(r.game, gobj, 0, 7) {
        g(r.game).arwings[slot] = None;
        setup::destroy(r, Some(gobj));
    }
}

/// An Arwing's wing's setup (maps 10 to 15), `grVenom_802056B0`: its slot, no roll, at the
/// origin, and its group's collision joint on.
#[doc(alias = "grVenom_802056B0")]
pub fn init_wing(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    let slot = g(r.game).slot;
    {
        let v = r.game.grounds[gid.0].u.arwing_coll();
        v.xC8 = slot;
        v.xD0 = 0;
        v.xDC = 0.0;
        v.xE0 = Vec3::new(0.0, 0.0, 0.0);
    }
    let group = g(r.game).groups[slot as usize];
    r.game
        .geometry_mut()
        .enable_joint(GROUP_JOINTS[group as usize]);
    setup::init_map_coll(r.game, r.data, j, m);
}

/// An Arwing's wing's process (maps 10 to 15), `grVenom_80205758`: while its slot's path lives,
/// it shows only near the stage's plane and level enough
/// (`Ground_UpdateStarFoxArwingVisibility`, within 50), at its Arwing's place and scale; then its
/// joint goes off and it goes.
#[doc(alias = "grVenom_80205758")]
pub fn proc_wing(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let slot = r.game.grounds[gid.0].u.arwing_coll().xC8 as usize;
    if g(r.game).arwings[slot].is_some() {
        update_wing_visibility(r.game, gid, j, 50.0);
        let s = reg(scale(r.game));
        let k = reg(param(r.game).f(r.data, p::ARWING_SCALE));
        let jj = j.expect("grvenom.c: jobj");
        let v = mem(fp::fmuls(s, k));
        jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.x = v);
        jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.y = v);
        jobj::set_srt(&mut r.game.objs.jobjs, jj, |o| o.scale.z = v);
        update_map_coll(r.game, r.data, gobj);
    } else {
        let group = g(r.game).groups[slot];
        r.game
            .geometry_mut()
            .disable_joint(GROUP_JOINTS[group as usize]);
        setup::destroy(r, Some(gobj));
    }
}

/// Sets a flying Arwing up (`grVenom_80205AD4` and `grVenom_80206874`): its slot, its
/// animations, and for a close type its group's wing (linked, its slot the same), for a far one
/// its model pinned to its path's joint 5; scaled, its roll at rest.
fn init_flying(r: &mut Running, gobj: GObjId, with_background: bool) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grvenom.c: jobj");
    let s = reg(scale(r.game));
    let slot = g(r.game).slot as i32;
    {
        let v = r.game.grounds[gid.0].u.arwing();
        v.xC8 = slot;
        v.xE0 = [None; 4];
    }
    if with_background {
        anime::start(r.game, r.data, gobj, 7, 7, 0, 0.0, 1.0, true);
        anime::start(r.game, r.data, gobj, 8, 7, 0, 0.0, 1.0, true);
        anime::start(r.game, r.data, gobj, 2, 7, 3, 0.0, 1.0, false);
    } else {
        anime::start(r.game, r.data, gobj, 0, 7, 1, 0.0, 1.0, true);
        anime::start(r.game, r.data, gobj, 2, 7, 3, 0.0, 1.0, false);
    }
    match g(r.game).types[slot as usize] {
        1..=7 => {
            let group = g(r.game).groups[slot as usize];
            let wing = create(r, GROUP_WINGS[group as usize], WING_FRAME);
            r.game.grounds[gid.0].u.arwing().xDC = wing;
            if let Some(w) = wing {
                let wg = ground_of(r.game, w);
                r.game.grounds[wg.0].u.arwing_coll().xC8 = slot as u32;
            }
        }
        8..=11 => {
            let path = g(r.game).arwings[slot as usize].expect("grvenom.c: a path");
            let a = nth_joint(r.game, gobj, 0);
            let b = nth_joint(r.game, path, 5);
            crate::lb::anim::attach(&mut r.game.objs, a, b);
            r.game.grounds[gid.0].u.arwing().xDC = None;
        }
        // `Ground_DisableStarFoxArwingGObjs`: no type reaches it.
        other => unreachable!("an Arwing of type {other}"),
    }
    let k = reg(param(r.game).f(r.data, p::ARWING_SCALE));
    let v = mem(fp::fmuls(s, k));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.x = v);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = v);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.z = v);
    let wait = param(r.game).i(r.data, p::ROLL_WAIT);
    let v = r.game.grounds[gid.0].u.arwing();
    v.b0 = false;
    v.xD4 = 0;
    v.xF0 = 0;
    v.xF4 = 0;
    v.xF8 = wait;
    v.xFC = 0;
}

/// A close group's Arwing's setup (map 3), `grVenom_80205AD4`: as [`init_flying`] with its
/// background animations, and which gun fires first.
#[doc(alias = "grVenom_80205AD4")]
pub fn init_arwing(r: &mut Running, gobj: GObjId) {
    init_flying(r, gobj, true);
    let gun = r.game.objs.rng.below(2);
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.arwing().x100 = Some(gun);
}

/// The far group's Arwing's setup (map 6), `grVenom_80206874`: as [`init_flying`].
#[doc(alias = "grVenom_80206874")]
pub fn init_far_arwing(r: &mut Running, gobj: GObjId) {
    init_flying(r, gobj, false);
}

/// The fire kind of an Arwing's group: -1 for none, 0 for the close groups, 1 for the far one.
fn fire_kind(game: &Game, slot: usize) -> i32 {
    match game.gr.venom.groups[slot] {
        1..=3 => 0,
        4 => 1,
        _ => -1,
    }
}

/// A flying Arwing's process (maps 3 and 6), `grVenom_80205F30`: nothing while the Arwings are
/// held. While its path lives: a close one faces the stage and now and then rolls (its rolling
/// part flat between rolls), follows its path's place with its wing at its rolling part's place
/// and roll; a far one, every eighth tick of its flight's window (by type) with odds of one in
/// eight, fires at a random player's fighter on its side of the stage while it is between 100
/// and 2000 deep, from alternating guns. Once its path is gone, it and its articles go.
#[doc(alias = "grVenom_80205F30")]
pub fn proc_arwing(r: &mut Running, gobj: GObjId) {
    if FROZEN {
        return;
    }
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grvenom.c: jobj");
    let slot = r.game.grounds[gid.0].u.arwing().xC8 as usize;
    let path = g(r.game).arwings[slot];
    if path.is_none() {
        let articles = r.game.grounds[gid.0].u.arwing().xE0;
        for a in articles.into_iter().flatten() {
            setup::destroy(r, Some(a));
        }
        setup::destroy(r, Some(gobj));
        return;
    }
    let group = g(r.game).groups[slot];
    if group == 4 {
        let a = nth_joint(r.game, gobj, 1).expect("grvenom.c: jobj");
        jobj::set_rotation_z(&mut r.game.objs.jobjs, a, 0.0);
    }
    let part = GROUP_PARTS[group as usize];
    match g(r.game).types[slot] {
        1..=7 => {
            jobj::set_rotation_y(&mut r.game.objs.jobjs, j, 0.0);
            let roll_state = r.game.grounds[gid.0].u.arwing().xF4;
            match roll_state {
                0 => {
                    if r.game.grounds[gid.0].u.arwing().xF8 <= 0 {
                        let roll = r.game.objs.rng.below(4).wrapping_add(1);
                        r.game.grounds[gid.0].u.arwing().xF4 = roll;
                        let k = fire_kind(r.game, slot);
                        let anim = ROLL_ANIMS[(roll * 2 + k) as usize];
                        anime::start(r.game, r.data, gobj, part, 7, anim, 0.0, 1.0, false);
                    } else {
                        let pj = nth_joint(r.game, gobj, part).expect("grvenom.c: jobj");
                        jobj::set_rotation_z(&mut r.game.objs.jobjs, pj, 0.0);
                    }
                    let v = r.game.grounds[gid.0].u.arwing();
                    v.xF8 = v.xF8.wrapping_sub(1);
                }
                1..=4 if anime::finished(r.game, gobj, 0, 7) => {
                    let wait = param(r.game).i(r.data, p::ROLL_WAIT);
                    let v = r.game.grounds[gid.0].u.arwing();
                    v.xF4 = 0;
                    v.xF8 = wait;
                }
                _ => {}
            }
            let pos = arwing_pos(r, path);
            jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
            let pj = nth_joint(r.game, gobj, part);
            let pos = world_position(&mut r.game.objs, r.data, pj, None);
            let wing = r.game.grounds[gid.0].u.arwing().xDC;
            if let Some(w) = wing {
                let wg = ground_of(r.game, w);
                r.game.grounds[wg.0].u.arwing_coll().xE0 = pos;
            }
            let rot_z = r.game.objs.jobjs[pj.expect("grvenom.c: jobj")].rotate.z;
            if let Some(w) = wing {
                let wg = ground_of(r.game, w);
                r.game.grounds[wg.0].u.arwing_coll().xDC = rot_z;
            }
        }
        ty @ 8..=11 => {
            let flown = r.game.grounds[gid.0].u.arwing().xF0;
            let fires = if flown & 7 == 0 && r.game.objs.rng.below(8) == 0 {
                let (lo, hi) = match ty {
                    8 => (0x3C, 0xE6),
                    9 => (0x32, 0xE6),
                    10 => (0x5A, 0x104),
                    _ => (0x46, 0xF0),
                };
                i32::from(flown > lo && flown < hi)
            } else {
                0
            };
            r.game.grounds[gid.0].u.arwing().xFC = fires;
            let pos = arwing_pos(r, path);
            let z = reg(pos.z);
            if -100.0 > z && z > -2000.0 && r.game.grounds[gid.0].u.arwing().xFC != 0 {
                r.game.grounds[gid.0].u.arwing().xFC = 0;
                let mut player = r.game.objs.rng.below(4);
                for tries in 0..4 {
                    player = player.wrapping_add(tries) & 3;
                    // Gm_PKind_NA.
                    if r.game.player_pkind(player) != 3 {
                        break;
                    }
                }
                let mut target = r.game.player_pos(player);
                let (tx, px) = (reg(target.x), reg(pos.x));
                if (tx > 0.0 && px > 0.0) || (tx < 0.0 && px < 0.0) {
                    target.y = mem(fp::fadds(reg(target.y), 5.0));
                    r.sfx_play(0x6B6C9, 0x7F, 0x40);
                    let k = param(r.game).f(r.data, p::ARWING_SCALE);
                    if fire_kind(r.game, slot) == 1 {
                        let b = nth_joint(r.game, gobj, 7);
                        arwinglaser::spawn_aimed(r, gobj, b, target, 3, 0, k, 0x6B6C9);
                    } else {
                        let gun =
                            r.game.grounds[gid.0].u.arwing().x100.expect(
                                "grvenom.c: an Arwing's gun read before its setup wrote it",
                            );
                        let b = nth_joint(r.game, gobj, if gun != 0 { 5 } else { 6 });
                        arwinglaser::spawn_aimed(r, gobj, b, target, 1, 0, k, 0x6B6C9);
                        r.game.grounds[gid.0].u.arwing().x100 = Some(gun.wrapping_add(1) & 1);
                    }
                    let script = r.data.ptr(param(r.game).0.at(p::FIRE_SCRIPT));
                    material::start_script(r.game, r.data, gobj, script, 0);
                }
            }
        }
        _ => {}
    }
    let v = r.game.grounds[gid.0].u.arwing();
    v.xF0 = v.xF0.wrapping_add(1);
}

/// The far group's Arwing's process (map 6), `grVenom_80206B70`: a flying Arwing's.
#[doc(alias = "grVenom_80206B70")]
pub fn proc_far_arwing(r: &mut Running, gobj: GObjId) {
    proc_arwing(r, gobj);
}

/// The talk window's setup (map 1), `grVenom_80206B94`.
#[doc(alias = "grVenom_80206B94")]
pub fn init_window(r: &mut Running, gobj: GObjId) {
    super::corneria::window_setup(r, gobj);
}

/// The talk window's process (map 1), `grVenom_80206BC4`.
#[doc(alias = "grVenom_80206BC4")]
pub fn proc_window(r: &mut Running, gobj: GObjId) {
    super::corneria::proc_line(r, gobj);
}

/// The stack address of the talk's scale joint's descriptor: Corneria's (which its taunt makes
/// in `grCorneria_801E2B80`'s frame) less `grVenom_80206BF0`'s frame (0x38).
pub const TALK_FRAME: u32 = super::corneria::TALK_FRAME - 0x38;

/// Starts the team's talk for Fox's taunt (2) or Falco's (20), `grVenom_80206BF0`: unless one
/// plays, one of five from talk 8 or 13, its first line after 10 ticks.
#[doc(alias = "grVenom_80206BF0")]
pub fn start_talk(r: &mut Running, which: i32) -> bool {
    if map_gobj(r.game, 8).is_some() {
        return false;
    }
    let talk = match which {
        2 => r.game.objs.rng.below(5).wrapping_add(8),
        20 => r.game.objs.rng.below(5).wrapping_add(13),
        _ => return false,
    };
    let w = create(r, 8, TALK_FRAME).expect("grvenom.c: 2244: wgobj");
    let gid = ground_of(r.game, w);
    let v = r.game.grounds[gid.0].u.starfox_talk();
    v.xCC = 10;
    v.xC8 = 0;
    v.xC4 = talk as u32;
    true
}

/// Stops the talk, `grVenom_80206CB0`: with one playing, its object too if `destroy_talk`, its
/// window and line with their text.
#[doc(alias = "grVenom_80206CB0")]
pub fn stop_talk(r: &mut Running, destroy_talk: bool) {
    let Some(t) = map_gobj(r.game, 8) else {
        return;
    };
    if destroy_talk {
        setup::destroy(r, Some(t));
    }
    if let Some(w) = map_gobj(r.game, 1) {
        setup::destroy(r, Some(w));
    }
    r.close_font(1);
}

/// The stage's line hazard, `grVenom_80206D74`: none.
#[doc(alias = "grVenom_80206D74")]
pub fn line_hazard() -> Option<DataRef> {
    None
}

/// Whether a fighter's shadow falls on a joint, `grVenom_80206D7C`: the feet above it, but not
/// above the upper platform when it's the lower one.
#[doc(alias = "grVenom_80206D7C")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let at = world_position(&mut game.objs, data, j, None);
    if let Some(p) = map_gobj(game, 5) {
        let gid = ground_of(game, p);
        let v = game.grounds[gid.0].u.venom_platform();
        let (upper, lower) = (v.upper, v.lower);
        if lower == j {
            let top = world_position(&mut game.objs, data, upper, None);
            if reg(pos.y) > reg(top.y) {
                return false;
            }
        }
    }
    reg(pos.y) > reg(at.y)
}
