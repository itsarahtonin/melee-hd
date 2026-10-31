//! Corneria, `melee/gr/grcorneria.c`: the Great Fox the fighters stand on (map 3, its copy map 0)
//! flying over the city (maps 8, 9 and 4, each part making the next as it passes), the sky (map
//! 7), and the Arwings (map 1 a flight path, maps 2 and 10 the Arwing flying it, maps 13 to 18
//! their wings the fighters stand on), with the Star Fox team's talk (maps 12 and 5).
//!
//! The Great Fox drifts within a box and, over the city's first part, now and then sinks a while.
//! Its two guns charge, fire a volley of lasers (`it::kinds::greatfoxlaser`) over the deck and
//! rest; enough damage to the guns' hurtbox items breaks them for good. Arwings fly in from the
//! distance on paths of their own, some close enough to stand on, and shoot lasers
//! (`it::kinds::arwinglaser`) ahead or at a fighter. Fox's and Falco's side taunt calls the team,
//! whose talk plays in a window (shared with Venom). The functions both stages call (the talk,
//! the match's end, the stage queries) are here, where the original keeps them.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::CodeRef;
use super::super::it::kinds::{arwinglaser, greatfoxlaser};
use super::super::state::{Game, Running};
use super::data::{CORNERIA_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, scale, update_map_coll};
use super::{GR_KIND_CORNERIA, GR_KIND_VENOM, anime, grlib, material, setup, stage, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `fn_801E12D0`, the guns' hurtbox items' callback for a hit they deal: nothing.
pub const GUN_DEALT: u32 = 0x801E_12D0;
/// `fn_801E12D4`, the guns' hurtbox items' callback for a hit they take.
pub const GUN_HIT: u32 = 0x801E_12D4;
/// `grCorneria_801DD65C`, the Great Fox's start hook.
const SHIP_START: u32 = 0x801D_D65C;
/// `grCorneria_801E2454` and `grCorneria_801E2480`, the Great Fox's splashes' callbacks.
pub const SPLASH_FRONT: u32 = 0x801E_2454;
pub const SPLASH_BACK: u32 = 0x801E_2480;
/// `smashTaunt_801E24AC`, the talk window's render callback.
pub const WINDOW_RENDER: u32 = 0x801E_24AC;

/// The spawn configurations' lines (`grCn_803E1FE8`, `grCn_803E2000`): the Great Fox's hull
/// where it meets the sea, front and back.
pub const FRONT_LINES: u32 = 0x803E_1FE8;
pub const BACK_LINES: u32 = 0x803E_2000;
const FRONT_LINE_ENDS: [Vec3; 2] = [Vec3::new(-59.0, -58.0, 18.0), Vec3::new(-22.0, -73.0, 0.0)];
const BACK_LINE_ENDS: [Vec3; 4] = [
    Vec3::new(-81.0, -21.0, 28.0),
    Vec3::new(14.0, -59.0, 128.0),
    Vec3::new(14.0, -59.0, 128.0),
    Vec3::new(0.0, -75.0, 173.0),
];

/// A spawn configuration's lines by their table's address.
pub fn lines(addr: u32) -> &'static [Vec3] {
    match addr {
        FRONT_LINES => &FRONT_LINE_ENDS,
        BACK_LINES => &BACK_LINE_ENDS,
        other => panic!("a spawn configuration's lines at {other:#010x}"),
    }
}

/// The places on the Great Fox the stage follows (`grCn_803E1F70`), in its frame.
const SHIP_POINTS: [Vec3; 5] = [
    Vec3::new(-274.14, 48.58, 0.0),
    Vec3::new(-200.81, -20.1, 0.0),
    Vec3::new(-59.31, -58.74, 0.0),
    Vec3::new(-40.19, -73.77, 0.0),
    Vec3::new(78.44, -73.48, 0.0),
];

/// Each Arwing type's animation (`grCn_803E2030`).
const TYPE_ANIMS: [i32; 14] = [-1, 0, 1, 2, 3, 4, 5, 6, 6, 6, 7, 8, 9, 10];
/// Each Arwing type's offset from its path's joint (`lbl_803E2068`).
const TYPE_OFFSETS: [Vec3; 14] = [
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(-40.0, 10.0, 0.0),
    Vec3::new(15.0, 5.0, 0.0),
    Vec3::new(190.0, -10.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
];
/// The Arwing's map by its group (`grCn_803E2110`).
const GROUP_ARWINGS: [i32; 5] = [2, 2, 2, 2, 10];
/// A roll's animation by its state and the group's offset (`grCn_803E214C`).
const ROLL_ANIMS: [[i32; 2]; 6] = [[-1, -1], [1, 0], [2, 2], [4, 4], [5, 5], [-1, -1]];
/// The collision object's map by group (`grCn_803E217C`).
const GROUP_COLLS: [i32; 5] = [0xD, 0xD, 0xE, 0xF, 0x10];
/// The collision joint by group (`grCn_803E2190`).
const GROUP_JOINTS: [i32; 5] = [0, 0, 1, 2, 5];
/// The rolling part by group (`grCn_803E21B0`).
const GROUP_PARTS: [i32; 5] = [1, 1, 1, 1, 1];
/// The speakers' joint pairs in the talk window (`grCn_803E21F0`).
const SPEAKER_JOINTS: [[i16; 2]; 5] = [[0, 0], [1, 6], [2, 7], [3, 8], [4, 9]];

/// The talks (`grCn_803E2204`): twenty of up to five lines, each its speaker, its text's entry
/// and its voice; a speaker of 0 ends the talk.
const TALKS: [[[i32; 3]; 5]; 20] = [
    [
        [1, 7, 0x5573E],
        [4, 8, 0x5573F],
        [2, 9, 0x55740],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [2, 0xA, 0x55741],
        [1, 0xB, 0x55742],
        [4, 0xC, 0x55743],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0xD, 0x55744],
        [1, 0xE, 0x55745],
        [2, 0xF, 0x55746],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [2, 0x10, 0x55747],
        [1, 0x11, 0x55748],
        [4, 0x12, 0x55749],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [1, 0x2F, 0x18717],
        [4, 0x30, 0x18718],
        [3, 0x31, 0x18719],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [3, 0x24, 0x1870C],
        [1, 0xB, 0x55742],
        [4, 0xC, 0x55743],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0x2C, 0x18714],
        [1, 0x2D, 0x18715],
        [3, 0x2E, 0x18716],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0x32, 0x1871A],
        [1, 0x33, 0x1871B],
        [3, 0x34, 0x1871C],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0x13, 0x1AE20],
        [2, 0x14, 0x1AE21],
        [4, 0x15, 0x1AE22],
        [1, 0x16, 0x1AE23],
        [0, 0, 0],
    ],
    [
        [1, 0x17, 0x1AE24],
        [4, 0x18, 0x1AE25],
        [2, 0x19, 0x1AE26],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0x1A, 0x1AE27],
        [1, 0x1B, 0x1AE28],
        [2, 0x1C, 0x1AE29],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [1, 0x1D, 0x1AE2A],
        [1, 0x1E, 0x1AE2B],
        [1, 0x1F, 0x1AE2C],
        [4, 0x20, 0x1AE2D],
        [0, 0, 0],
    ],
    [
        [2, 0x21, 0x1AE2E],
        [1, 0x22, 0x1AE2F],
        [4, 0x23, 0x1AE30],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [3, 0x24, 0x1870C],
        [4, 0x25, 0x1870D],
        [1, 0x26, 0x1870E],
        [1, 0x27, 0x1870F],
        [0, 0, 0],
    ],
    [
        [1, 0x28, 0x18710],
        [4, 0x29, 0x18711],
        [3, 0x2A, 0x18712],
        [4, 0x2B, 0x18713],
        [0, 0, 0],
    ],
    [
        [4, 0x2C, 0x18714],
        [1, 0x2D, 0x18715],
        [3, 0x2E, 0x18716],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [1, 0x2F, 0x18717],
        [4, 0x30, 0x18718],
        [3, 0x31, 0x18719],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [
        [4, 0x32, 0x1871A],
        [1, 0x33, 0x1871B],
        [3, 0x34, 0x1871C],
        [0, 0, 0],
        [0, 0, 0],
    ],
    [[4, 5, 0x5573C], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    [[4, 6, 0x5573D], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
];

/// The city's parts' widths before the stage's scale: the first and middle parts' (maps 8 and
/// 9) and the last's (map 4).
const CITY_WIDTH: f32 = 3200.0;
const CITY_WIDTH_LAST: f32 = 4800.0;

/// The fields of `grCorneria_YakumonoParam`, by offset.
mod p {
    /// The drift's ticks between turns (between these two), its acceleration, its speed limit
    /// and its box.
    pub const DRIFT_TIME_MIN: u32 = 0;
    pub const DRIFT_TIME_MAX: u32 = 4;
    pub const DRIFT_ACCEL: u32 = 8;
    pub const DRIFT_SPEED: u32 = 0xC;
    pub const DRIFT_X: u32 = 0x10;
    pub const DRIFT_Y: u32 = 0x14;
    /// The guns' rest: its spread and its least.
    pub const GUN_REST_ADD: u32 = 0x18;
    pub const GUN_REST: u32 = 0x1C;
    /// The guns' charged wait before firing.
    pub const GUN_AIM: u32 = 0x20;
    /// The chance of a single big shot, and the charges of a big shot and of a volley.
    pub const GUN_BIG_CHANCE: u32 = 0x24;
    pub const GUN_BIG_CHARGE: u32 = 0x28;
    pub const GUN_CHARGE: u32 = 0x2C;
    /// A volley's shots (between these two).
    pub const GUN_SHOTS_MIN: u32 = 0x30;
    pub const GUN_SHOTS_MAX: u32 = 0x34;
    /// The damage that breaks the guns.
    pub const GUN_HP: u32 = 0x38;
    /// The first Arwing's wait (between these two), the waits after (between these two), and
    /// the chance of a far Arwing.
    pub const ARWING_FIRST_MIN: u32 = 0x3C;
    pub const ARWING_FIRST_MAX: u32 = 0x40;
    pub const ARWING_WAIT_MIN: u32 = 0x44;
    pub const ARWING_WAIT_MAX: u32 = 0x48;
    pub const ARWING_FAR: u32 = 0x4C;
    /// An Arwing's wait between rolls.
    pub const ROLL_WAIT: u32 = 0x68;
    /// The Arwings' scale.
    pub const ARWING_SCALE: u32 = 0x70;
    /// How often an Arwing may fire and the odds it does, in a match and in the demo.
    pub const FIRE_PERIOD: u32 = 0x74;
    pub const FIRE_ODDS: u32 = 0x78;
    pub const FIRE_PERIOD_DEMO: u32 = 0x7C;
    pub const FIRE_ODDS_DEMO: u32 = 0x80;
    /// The color script an Arwing's shot flashes it with.
    pub const FIRE_SCRIPT: u32 = 0x84;
    /// The city's speed.
    pub const CITY_SPEED: u32 = 0x88;
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
    fn word(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .corneria_yakumono
            .expect("grcorneria.c: yakumono_param"),
    )
}

/// grcorneria.c's statics besides its `yakumono_param`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The three Arwing slots' flight paths (`arwing_gobjs`), types (`arwing_types`: 1 to 9
    /// close, 10 to 13 far) and groups (`arwing_groups`: 1 to 3 by slot, 4 the far one).
    pub arwings: [Option<GObjId>; 3],
    pub types: [i32; 3],
    pub groups: [i32; 3],
    /// The slot being made (`grCn_804D69A4`).
    pub slot: u32,
    /// The wait before the next Arwing (`grCn_804D69A8`).
    pub wait: i32,
    /// Whether the Arwings hold still, before the match starts (`grCn_804D69AC`).
    pub frozen: bool,
    /// Whether the demo plays (`grCn_804D69B0`).
    pub demo: i32,
    /// Where the Great Fox's points are this tick (`grCn_803E1FAC`): a table in the executable,
    /// which outlives a match.
    pub points: [Vec3; 5],
    /// The talk the debugging buttons last chose (`grCn_804D466C`): a static, -1 at boot.
    pub talk: i32,
}

impl Default for Globals {
    fn default() -> Self {
        Globals {
            arwings: [None; 3],
            types: [0; 3],
            groups: [0; 3],
            slot: 0,
            wait: 0,
            frozen: false,
            demo: 0,
            points: [Vec3::default(); 5],
            talk: -1,
        }
    }
}

fn g(game: &mut Game) -> &mut Globals {
    &mut game.gr.corneria
}

/// A stage object's process or callback, by address: Corneria's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801D_D654 | 0x801D_D658 | 0x801D_DAC0 | 0x801D_E4BC | 0x801D_E8E0 | 0x801D_F8CC
        | 0x801D_FBEC | 0x801D_FC24 | 0x801D_FC28 | 0x801D_FEB4 | 0x801E_013C | 0x801E_03C4
        | 0x801E_0DE0 | 0x801E_0E3C | 0x801E_0F30 | 0x801E_1054 | 0x801E_12CC => empty(r, gobj),
        0x801D_D620 => init_copy(r, gobj),
        SHIP_START => start_ship(r, gobj),
        0x801D_D674 => init_ship(r, gobj),
        0x801D_D9A8 => proc_ship(r, gobj),
        0x801D_DAC4 => init_path(r, gobj),
        0x801D_E024 => proc_path(r, gobj),
        0x801D_E4C0 => init_wing(r, gobj),
        0x801D_E568 => proc_wing(r, gobj),
        0x801D_E8E4 => init_arwing(r, gobj),
        0x801D_ED50 => proc_arwing(r, gobj),
        0x801D_F8D0 => init_far_arwing(r, gobj),
        0x801D_FBCC => proc_far_arwing(r, gobj),
        0x801D_FBF0 => init_map6(r, gobj),
        0x801D_FC2C => init_city_middle(r, gobj),
        0x801D_FC98 => proc_city_middle(r, gobj),
        0x801D_FEB8 => init_city_last(r, gobj),
        0x801D_FF20 => proc_city_last(r, gobj),
        0x801E_0140 => init_city_first(r, gobj),
        0x801E_01A8 => proc_city_first(r, gobj),
        0x801E_0C3C => init_sky(r, gobj),
        0x801E_0D30 => proc_sky(r, gobj),
        0x801E_0DE4 => init_window(r, gobj),
        0x801E_0E14 => proc_window(r, gobj),
        0x801E_0F6C => proc_talk(r, gobj),
        0x801E_1030 => talk_destroyed(r, gobj),
        0x801E_1060 => proc_map11(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801D_D350 => on_init(r),
        0x801D_D478 => on_load(r),
        0x801D_D508 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "grCorneria_801DD654")]
#[doc(alias = "grCorneria_801DD658")]
#[doc(alias = "grCorneria_801DDAC0")]
#[doc(alias = "grCorneria_801DE4BC")]
#[doc(alias = "grCorneria_801DE8E0")]
#[doc(alias = "grCorneria_801DF8CC")]
#[doc(alias = "grCorneria_801DFBEC")]
#[doc(alias = "grCorneria_801DFC24")]
#[doc(alias = "grCorneria_801DFC28")]
#[doc(alias = "grCorneria_801DFEB4")]
#[doc(alias = "grCorneria_801E013C")]
#[doc(alias = "grCorneria_801E03C4")]
#[doc(alias = "grCorneria_801E0DE0")]
#[doc(alias = "grCorneria_801E0E3C")]
#[doc(alias = "grCorneria_801E0F30")]
#[doc(alias = "grCorneria_801E1054")]
#[doc(alias = "grCorneria_801E12CC")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

fn gp_map(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// Clears the Arwing slots and draws the first Arwing's wait, `grCorneria_801DCCFC`: in the
/// demo (stages 0xE9 and 0x47) two seconds.
#[doc(alias = "grCorneria_801DCCFC")]
pub fn reset_arwings(r: &mut Running) {
    let c = g(r.game);
    c.arwings = [None; 3];
    c.types = [0; 3];
    c.groups = [0; 3];
    let stage = stage::stkind(r.game);
    g(r.game).demo = i32::from(matches!(stage, 0xE9 | 0x47));
    if g(r.game).demo == 1 {
        g(r.game).wait = 0x78;
        return;
    }
    let p = param(r.game);
    let lo = p.i(r.data, p::ARWING_FIRST_MIN);
    let hi = p.i(r.data, p::ARWING_FIRST_MAX);
    g(r.game).wait = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
}

/// Whether a point is beyond the city's first and last parts' 300 to its right
/// (`grCn_CheckFar`, on [`city_at`]): 0 if it is over the first or last part and 300 left of
/// it is too, else 1. Moves the point 300 left when it is.
fn check_far(r: &mut Running, pos: &mut Vec3) -> i32 {
    if matches!(city_at(r.game, pos.x), 4 | 9) {
        pos.x = mem(fp::fsubs(reg(pos.x), 300.0));
        if matches!(city_at(r.game, pos.x), 4 | 9) {
            return 0;
        }
    }
    1
}

/// Sends the next Arwing, `grCorneria_801DCE1C`. In a match one at a time: once the last has
/// gone and its wait runs out, a type unlike the last (one of the close nine if the Great Fox
/// is far from the city's end, any of thirteen otherwise) in group 1 or, by chance and not
/// twice running, the far group 4; then another wait. In the demo up to three at once.
#[doc(alias = "grCorneria_801DCE1C")]
pub fn spawn_arwings(r: &mut Running) {
    if stage::stkind(r.game) == 0x46 {
        g(r.game).arwings = [None; 3];
        return;
    }
    if g(r.game).demo == 0 {
        if g(r.game).arwings[0].is_none() {
            let c = g(r.game);
            c.wait = c.wait.wrapping_sub(1);
            if c.wait <= 0 {
                let mut pos = ship_pos(r.game);
                let far = check_far(r, &mut pos);
                let mut type_id = g(r.game).types[0];
                while type_id == g(r.game).types[0] {
                    let n = if far == 1 { 9 } else { 13 };
                    type_id = r.game.objs.rng.below(n).wrapping_add(1);
                }
                let group = if g(r.game).groups[0] == 4 {
                    1
                } else {
                    let chance = reg(param(r.game).f(r.data, p::ARWING_FAR));
                    if reg(r.game.objs.rng.draw_f32()) > chance {
                        1
                    } else {
                        4
                    }
                };
                let c = g(r.game);
                c.slot = 0;
                c.types[0] = type_id;
                c.groups[0] = group;
                let a = create(r, 1, ARWING_FRAME);
                let c = g(r.game);
                c.arwings[c.slot as usize] = a;
            }
        } else {
            let p = param(r.game);
            let hi = p.i(r.data, p::ARWING_WAIT_MAX);
            let lo = p.i(r.data, p::ARWING_WAIT_MIN);
            g(r.game).wait = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
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
        let c = &r.game.gr.corneria;
        i != count && (1..=9).contains(&c.types[i]) && c.arwings[i].is_some()
    });
    let pick = |r: &mut Running, n: i32, add: i32| {
        let mut type_id = g(r.game).types[count];
        while g(r.game).types.contains(&type_id) {
            type_id = r.game.objs.rng.below(n).wrapping_add(add);
        }
        type_id
    };
    if !has_near {
        let type_id = pick(r, 13, 1);
        demo_arwing(r, count, type_id);
    } else {
        let mut pos = ship_pos(r.game);
        if check_far(r, &mut pos) == 0 {
            let type_id = pick(r, 4, 10);
            demo_arwing(r, count, type_id);
        }
    }
}

fn demo_arwing(r: &mut Running, count: usize, type_id: i32) {
    let c = g(r.game);
    c.slot = count as u32;
    c.types[count] = type_id;
    c.groups[count] = count as i32 + 1;
    let a = create(r, 1, ARWING_FRAME);
    let c = g(r.game);
    c.arwings[c.slot as usize] = a;
}

/// The demo's setup (`grCorneria_801DD2C0`): the Great Fox hidden, the city laid out, and a talk,
/// up to the demo's own scene code, which only the title screen's demo runs.
pub fn on_demo_init(r: &mut Running) {
    if let Some(ship) = map_gobj(r.game, 3)
        && let Some(j) = jobj_of(r.game, ship)
    {
        jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
    }
    demo_city(r);
    let _ = create(r, 12, setup::SETUP_FRAME);
    super::super::code::unported_call("gm_8017E7E0");
}

/// The stack address of the scale joint's descriptor for the objects Corneria's setup makes
/// (`grCorneria_801DD350`, a 0x30-byte frame).
pub const INIT_FRAME: u32 = setup::init_frame(0x30);

/// Corneria's setup, `grCorneria_801DD350`: its parameters, the sky (map 7), the Great Fox (map
/// 3), the Arwing slots, the city's three parts waiting (maps 8, 9 and 4), map 11, camera limits
/// and blast zones, a wind source, the Arwings held, and the Arwings' collision joints off.
#[doc(alias = "grCorneria_801DD350")]
pub fn on_init(r: &mut Running) {
    r.game.gr.corneria_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    create(r, 7, INIT_FRAME);
    create(r, 3, INIT_FRAME);
    reset_arwings(r);
    let gobj = create(r, 8, INIT_FRAME).expect("grcorneria.c: map 8");
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.corneria_city8().xC4 = 1;
    for map in [9, 4] {
        let gobj = create(r, map, INIT_FRAME).expect("grcorneria.c: the city");
        let gid = ground_of(r.game, gobj);
        r.game.grounds[gid.0].u.corneria_city().xC4 = 1;
    }
    create(r, 0xB, INIT_FRAME);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    let _ = crate::lb::wind::push(
        &mut r.game.gr.winds,
        Vec3::new(1.0, 0.0, 0.0),
        -1,
        1.0,
        0.0,
        std::f32::consts::FRAC_PI_3,
        -100_000.0,
        100_000.0,
        100_000.0,
        -100_000.0,
    );
    g(r.game).frozen = true;
    for joint in [0, 1, 2, 5, 6, 7] {
        r.game.geometry_mut().disable_joint(joint);
    }
}

/// Corneria's load hook, `grCorneria_801DD478`: map 11 and the city's waiting parts go, the
/// city starts again from its middle part (map 8), the Arwings move, and the stage's sound.
#[doc(alias = "grCorneria_801DD478")]
pub fn on_load(r: &mut Running) {
    for map in [0xB, 8, 9, 4] {
        if let Some(gobj) = map_gobj(r.game, map) {
            setup::destroy(r, Some(gobj));
        }
    }
    let gobj = create(r, 8, LOAD_FRAME).expect("grcorneria.c: map 8");
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.corneria_city8().xC5 = 0;
    g(r.game).frozen = false;
    r.stage_sound(0x55737);
}

/// The stack address of the scale joint's descriptor for the city part the load hook makes,
/// which runs outside the setup (its key doesn't follow the setup's frame, [`INIT_FRAME`]), as
/// the sessions' traces show it (`select-sessions/stage-corneria`).
pub const LOAD_FRAME: u32 = setup::SETUP_FRAME + 0x58;

/// Corneria's start, `grCorneria_801DD508`: the enemy spawner, without enemies.
#[doc(alias = "grCorneria_801DD508")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, None);
}

/// Where a stage object's process runs (`HSD_GObj_RunProcs`' frame): the stack addresses below
/// it key the scale joints the processes' objects make in the ID table. The sessions' traces
/// show the Arwings', their planes' and the city parts' keys 0x18 above what 0x804EEA38 gave
/// (`select-sessions/stage-corneria`), as Battlefield's background's and Kongo Jungle's events'
/// frames have it.
pub const PROC_LOOP: u32 = 0x804E_EA50;

/// The stack address of the scale joint's descriptor for a flight path (map 1): below the
/// process loop by the Great Fox's process's frame (0x30), the Arwings' (0x60) and the create
/// function's (0x20).
pub const ARWING_FRAME: u32 = PROC_LOOP - 0x30 - 0x60 - 0x20;

/// Creates one of the stage's objects and sets it up, `grCorneria_801DD534`.
#[doc(alias = "grCorneria_801DD534")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    let gobj = setup::create_from_callbacks(r, map_id, callbacks(map_id), frame);
    if gobj.is_none() {
        crate::game::code::os_report();
    }
    gobj
}

/// The stage's objects' setup functions and processes, by map id (`grCn_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &CORNERIA_CALLBACKS[map_id as usize]
}

/// The Great Fox's copy's setup (map 0), `grCorneria_801DD620`: its animation.
#[doc(alias = "grCorneria_801DD620")]
pub fn init_copy(r: &mut Running, gobj: GObjId) {
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The Great Fox's start hook, `grCorneria_801DD65C`: it stops waiting.
#[doc(alias = "grCorneria_801DD65C")]
pub fn start_ship(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.corneria().set_waiting(false);
}

/// The Great Fox's setup (map 3), `grCorneria_801DD674`: its collision, waiting, at its place
/// (0, 250) with no drift, its animation, its copy (map 0) at its place, camera limits and blast
/// zones, the guns at rest with their hurtbox items, the broken guns hidden, its splashes'
/// spawn configurations, its start hook, and the deck's joint.
#[doc(alias = "grCorneria_801DD674")]
pub fn init_ship(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    {
        let v = r.game.grounds[gid.0].u.corneria();
        v.set_waiting(true);
        v.set_sinking(false);
        v.base_x = 0.0;
        v.base_y = 250.0;
        v.xD0 = 0.0;
        v.offset_x = 0.0;
        v.offset_y = 0.0;
        v.xE4 = Vec3::new(0.0, 0.0, 0.0);
        v.xF0 = 0.0;
        v.xF4 = 0.0;
        v.xF8 = 0.0;
        v.x104 = 0;
    }
    r.game.grounds[gid.0].x10_flags.b5 = true;
    anime::start_map(r.game, r.data, gobj, m, 0);
    super::izumi::check_archive(r.game, r.data, 3);
    let copy = create(r, 0, COPY_FRAME);
    let v = r.game.grounds[gid.0].u.corneria();
    v.x128 = copy;
    let t = Vec3::new(
        mem(fp::fadds(reg(v.base_x), reg(v.offset_x))),
        mem(fp::fadds(reg(v.base_y), reg(v.offset_y))),
        0.0,
    );
    jobj::set_translate(&mut r.game.objs.jobjs, j.expect("grcorneria.c: jobj"), t);
    move_copy(r, gobj);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    {
        let v = r.game.grounds[gid.0].u.corneria();
        v.x108 = 0;
        v.x10C = 0;
        v.x118 = 0;
        v.x11A = 0;
        v.x110 = 0;
        v.x114 = 0.0;
    }
    let hurt_a = Vec3::new(4.0, 0.0, 0.0);
    let hurt_b = Vec3::new(16.0, 0.0, 0.0);
    let left_joint = nth_joint(r.game, gobj, 2);
    let left = material::stage_item(
        r,
        0,
        0,
        gid,
        left_joint,
        Some(CodeRef(GUN_DEALT)),
        Some(CodeRef(GUN_HIT)),
        None,
    );
    r.game.grounds[gid.0].u.corneria().left_cannon = left;
    let left = left.expect("grcorneria.c: the left gun");
    material::set_item_hurtbox(r.game, left, hurt_a, hurt_b, 4.0);
    material::item_hurtbox_on(r.game, left);
    let right_joint = nth_joint(r.game, gobj, 3);
    let right = material::stage_item(
        r,
        0,
        2,
        gid,
        right_joint,
        Some(CodeRef(GUN_DEALT)),
        Some(CodeRef(GUN_HIT)),
        None,
    );
    r.game.grounds[gid.0].u.corneria().right_cannon = right;
    let right = right.expect("grcorneria.c: the right gun");
    material::item_hurtbox_on(r.game, right);
    material::set_item_hurtbox(r.game, right, hurt_a, hurt_b, 4.0);
    let whole = nth_joint(r.game, gobj, 6);
    jobj::clear_flags(&mut r.game.objs.jobjs, whole, jobj::HIDDEN);
    let broken = nth_joint(r.game, gobj, 5);
    jobj::set_flags(&mut r.game.objs.jobjs, broken, jobj::HIDDEN);
    let front = zako::new_config(r.game, r.data, FRONT_LINES, 1, CodeRef(SPLASH_FRONT), 0.3);
    let back = zako::new_config(r.game, r.data, BACK_LINES, 2, CodeRef(SPLASH_BACK), 0.3);
    let v = r.game.grounds[gid.0].u.corneria();
    v.xC8 = front;
    v.xCC = back;
    setup::add_start_hook(r.game, gobj, CodeRef(SHIP_START));
    r.game.grounds[gid.0].x11_flags.b012 = 1;
    let deck = nth_joint(r.game, gobj, 8);
    r.game.grounds[gid.0].u.corneria().x12C = deck;
}

/// The stack address of the copy's scale joint's descriptor: the stage's setup's, less the
/// create function's frame (0x20) and the Great Fox's setup's (0x30).
pub const COPY_FRAME: u32 = INIT_FRAME - 0x50;

/// The Great Fox's process (map 3), `grCorneria_801DD9A8`: once the match has started, its
/// drift, its sinking, its splashes where its hull meets the sea, its guns, the debugging
/// talk and the Arwings; then, every tick, its points, its collision, its copy, the city's
/// colors and the wind sources.
#[doc(alias = "grCorneria_801DD9A8")]
pub fn proc_ship(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    if !r.game.grounds[gid.0].u.corneria().waiting() {
        drift(r, gobj);
        sink(r, gobj);
        for front in [true, false] {
            let depth = reg(r.game.grounds[gid.0].u.corneria().xD0);
            let height = mem(fp::fmsubs(-35.0, reg(scale(r.game)), depth));
            let v = r.game.grounds[gid.0].u.corneria();
            let config = if front { v.xC8 } else { v.xCC };
            let part = nth_joint(r.game, gobj, 0);
            zako::run_config(r, config, part, height);
        }
        guns(r, gobj);
        debug_talk(r);
        spawn_arwings(r);
    }
    for (i, pt) in SHIP_POINTS.iter().enumerate() {
        let w = world_position(&mut r.game.objs, r.data, j, Some(*pt));
        g(r.game).points[i] = w;
    }
    update_map_coll(r.game, r.data, gobj);
    move_copy(r, gobj);
    city_colors(r);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// An Arwing's flight path's setup (map 1), `grCorneria_801DDAC4`: it takes its slot, makes
/// its group's Arwing (map 2 or 10) with the same slot, and is scaled; its animation starts on
/// its first tick.
#[doc(alias = "grCorneria_801DDAC4")]
pub fn init_path(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let slot = g(r.game).slot;
    r.game.grounds[gid.0].u.arwing_path().xC8 = slot;
    g(r.game).arwings[slot as usize] = Some(gobj);
    let group = g(r.game).groups[slot as usize];
    let arwing = create(r, GROUP_ARWINGS[group as usize], ARWING_PLANE_FRAME);
    if let Some(a) = arwing {
        let ag = ground_of(r.game, a);
        r.game.grounds[ag.0].x10_flags.b2 = false;
        r.game.grounds[ag.0].u.arwing().xC8 = slot as i32;
    }
    let s = reg(scale(r.game));
    let k = reg(param(r.game).f(r.data, p::ARWING_SCALE));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.scale.x = mem(fp::fmuls(s, k))
    });
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = mem(s));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.z = mem(s));
    let v = r.game.grounds[gid.0].u.arwing_path();
    v.xD4 = 1;
    v.xD8 = 0;
}

/// The stack address of an Arwing's scale joint's descriptor: below its path's by the path's
/// setup's frame (0x38) and the create function's.
pub const ARWING_PLANE_FRAME: u32 = ARWING_FRAME - 0x38 - 0x20;

/// How far the Great Fox drifted this tick, `grCorneria_801DDCF0`.
#[doc(alias = "grCorneria_801DDCF0")]
pub fn ship_step(game: &mut Game) -> Vec3 {
    let gobj = map_gobj(game, 3).expect("grcorneria.c: the Great Fox");
    let gid = ground_of(game, gobj);
    let v = game.grounds[gid.0].u.corneria();
    Vec3::new(
        mem(fp::fsubs(reg(v.offset_x), reg(v.xE4.x))),
        mem(fp::fsubs(reg(v.offset_y), reg(v.xE4.y))),
        0.0,
    )
}

/// Where the Great Fox is, `grCorneria_801DDD4C`.
#[doc(alias = "grCorneria_801DDD4C")]
pub fn ship_pos(game: &mut Game) -> Vec3 {
    let gobj = map_gobj(game, 3).expect("grcorneria.c: the Great Fox");
    let gid = ground_of(game, gobj);
    let v = game.grounds[gid.0].u.corneria();
    Vec3::new(
        mem(fp::fadds(reg(v.base_x), reg(v.offset_x))),
        mem(fp::fadds(reg(v.base_y), reg(v.offset_y))),
        0.0,
    )
}

/// Where a flight path's Arwing is in the stage (its joint 4 seen from the side, by the Great
/// Fox's drift and its type's offset), at least 100 high: the body of `grCorneria_801DDDA8` and
/// its inline copies.
fn arwing_pos(r: &mut Running, path: Option<GObjId>) -> Vec3 {
    let mut v = match path {
        Some(path) => {
            let slot = {
                let pg = ground_of(r.game, path);
                r.game.grounds[pg.0].u.arwing_path().xC8
            };
            let ship = map_gobj(r.game, 3).expect("grcorneria.c: the Great Fox");
            let sg = ground_of(r.game, ship);
            let off_x = r.game.grounds[sg.0].u.corneria().offset_x;
            let j = nth_joint(r.game, path, 4);
            let pos = world_position(&mut r.game.objs, r.data, j, None);
            let o = TYPE_OFFSETS[g(r.game).types[slot as usize] as usize];
            Vec3::new(
                mem(fp::fadds(
                    reg(off_x),
                    fp::fadds(fp::fneg(reg(pos.z)), reg(o.x)),
                )),
                mem(fp::fadds(reg(pos.y), reg(o.y))),
                mem(fp::fadds(reg(pos.x), reg(o.z))),
            )
        }
        None => Vec3::new(0.0, 0.0, 0.0),
    };
    if reg(v.y) <= 100.0 {
        v.y = 100.0;
    }
    v
}

/// Where a flight path's Arwing is, `grCorneria_801DDDA8`.
#[doc(alias = "grCorneria_801DDDA8")]
pub fn path_pos(r: &mut Running, path: Option<GObjId>) -> Vec3 {
    arwing_pos(r, path)
}

/// A close Arwing's sounds, `grCorneria_801DDE88`: its fly-in once it is inside the blast
/// zones, then its fly-off once it nears them in front.
#[doc(alias = "grCorneria_801DDE88")]
pub fn path_sound_near(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let pos = arwing_pos(r, Some(gobj));
    let sound = r.game.grounds[gid.0].u.arwing_path().xD8;
    match sound {
        0 => {
            if !outside_blast_zones(r.game, pos) {
                r.sfx_play(0x55730, 0x7F, 0x40);
                r.game.grounds[gid.0].u.arwing_path().xD8 = 1;
            }
        }
        1 => {
            if near_blast_zones(r.game, pos) == 0 {
                r.game.grounds[gid.0].u.arwing_path().xD8 = 2;
            }
        }
        2 if near_blast_zones(r.game, pos) == 1 => {
            r.sfx_play(0x55732, 0x7F, 0x40);
            r.game.grounds[gid.0].u.arwing_path().xD8 = 3;
        }
        _ => {}
    }
}

/// The far Arwings' bounds of sound (`grCn_ArwingMinX`, `grCn_ArwingMaxX`).
const ARWING_MIN_X: f32 = -10.0;
const ARWING_MAX_X: f32 = 2000.0;

/// A far Arwing's sounds (`grCorneria_801DE024_inline`): its fly-by once it is between
/// [`ARWING_MIN_X`] and [`ARWING_MAX_X`], its fly-off once it leaves.
fn path_sound_far(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let pos = path_pos(r, Some(gobj));
    let x = reg(pos.x);
    match r.game.grounds[gid.0].u.arwing_path().xD8 {
        0 => {
            if reg(ARWING_MIN_X) < x && x < reg(ARWING_MAX_X) {
                r.sfx_play(0x55731, 0x7F, 0x40);
                r.game.grounds[gid.0].u.arwing_path().xD8 = 1;
            }
        }
        1 if x < reg(ARWING_MIN_X) || reg(ARWING_MAX_X) < x => {
            r.sfx_play(0x55732, 0x7F, 0x40);
            r.game.grounds[gid.0].u.arwing_path().xD8 = 2;
        }
        _ => {}
    }
}

/// A flight path's process (map 1), `grCorneria_801DE024`: it rides at the Great Fox's height
/// (its joint 1 349 lower); on its first tick its type's animation starts; then its sounds (a
/// far one also turned side on and lowered with the sinking Great Fox) until its animation ends
/// and it frees its slot and goes. A path whose slot another holds goes at once.
#[doc(alias = "grCorneria_801DE024")]
pub fn proc_path(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let ship = map_gobj(r.game, 3);
    if let Some(sj) = ship.and_then(|s| jobj_of(r.game, s)) {
        let y = r.game.objs.jobjs[sj].translate.y;
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
        let j1 = nth_joint(r.game, gobj, 1).expect("grcorneria.c: jobj");
        let y1 = mem(fp::fsubs(reg(r.game.objs.jobjs[j1].translate.y), 349.0));
        jobj::set_srt(&mut r.game.objs.jobjs, j1, |o| o.translate.y = y1);
    }
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
    match g(r.game).types[slot] {
        1..=9 => path_sound_near(r, gobj),
        10..=13 => {
            jobj::set_rotation_y(&mut r.game.objs.jobjs, j, -std::f32::consts::FRAC_PI_2);
            path_sound_far(r, gobj);
            let vf = reg(sink_depth(r.game)).clamp(-150.0, 0.0);
            let j0 = nth_joint(r.game, gobj, 0).expect("grcorneria.c: jobj");
            let mut t = r.game.objs.jobjs[j0].translate;
            t.y = mem(if vf < 0.0 { fp::fneg(vf) } else { vf });
            jobj::set_translate(&mut r.game.objs.jobjs, j0, t);
        }
        _ => {}
    }
    if anime::finished(r.game, gobj, 0, 7) {
        g(r.game).arwings[slot] = None;
        setup::destroy(r, Some(gobj));
    }
}

/// An Arwing's collision's setup (maps 13 to 18), `grCorneria_Arwing_801DE4C0`: its slot, no
/// roll, at the origin, and its group's collision joint on.
#[doc(alias = "grCorneria_Arwing_801DE4C0")]
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

/// An Arwing's collision's process (maps 13 to 18), `grCorneria_801DE568`: while its slot's
/// path lives, it shows only near the stage's plane and level enough
/// (`Ground_UpdateStarFoxArwingVisibility`), at its Arwing's place and scale; then its joint goes
/// off and it goes.
#[doc(alias = "grCorneria_801DE568")]
pub fn proc_wing(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let slot = r.game.grounds[gid.0].u.arwing_coll().xC8 as usize;
    if g(r.game).arwings[slot].is_some() {
        update_wing_visibility(r.game, gid, j, 10.0);
        let s = reg(scale(r.game));
        let k = reg(param(r.game).f(r.data, p::ARWING_SCALE));
        let jj = j.expect("grcorneria.c: jobj");
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

/// `Ground_UpdateStarFoxArwingVisibility`: an Arwing's collision shows only within `depth` of
/// the stage's plane (then flattened onto it) and with its roll within 60 degrees of level,
/// and moves to its place.
pub fn update_wing_visibility(
    game: &mut Game,
    gid: super::super::handles::GroundId,
    j: Option<JObjId>,
    depth: f64,
) {
    let v = game.grounds[gid.0].u.arwing_coll();
    let z = reg(v.xE0.z);
    let angle = if z < 0.0 { fp::fneg(z) } else { z };
    let hidden = if angle < depth {
        v.xE0.z = 0.0;
        let pi = std::f64::consts::PI;
        let tau = std::f64::consts::TAU;
        while reg(v.xDC) < -pi {
            v.xDC = mem(fp::frsp(fp::fadd(reg(v.xDC), tau)));
        }
        while reg(v.xDC) > pi {
            v.xDC = mem(fp::frsp(fp::fsub(reg(v.xDC), tau)));
        }
        let a = reg(v.xDC);
        let a = if a < 0.0 { fp::fneg(a) } else { a };
        !(a < reg(std::f32::consts::FRAC_PI_3))
    } else {
        true
    };
    if hidden {
        jobj::set_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
    } else {
        jobj::clear_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
    }
    let t = game.grounds[gid.0].u.arwing_coll().xE0;
    jobj::set_translate(&mut game.objs.jobjs, j.expect("a wing's jobj"), t);
}

/// Sets a flying Arwing up (`grCorneria_801DE8E4` and `grCorneria_801DF8D0`): its slot, its
/// animations, and for a close type its group's collision object (linked, its slot the same),
/// for a far one its model pinned to its path's joint 4; scaled, its roll at rest.
fn init_flying(r: &mut Running, gobj: GObjId, with_background: bool) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
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
        1..=9 => {
            let group = g(r.game).groups[slot as usize];
            let coll = create(r, GROUP_COLLS[group as usize], WING_FRAME);
            r.game.grounds[gid.0].u.arwing().xDC = coll;
            if let Some(c) = coll {
                let cg = ground_of(r.game, c);
                r.game.grounds[cg.0].u.arwing_coll().xC8 = slot as u32;
            }
        }
        10..=13 => {
            let path = g(r.game).arwings[slot as usize].expect("grcorneria.c: a path");
            let a = nth_joint(r.game, gobj, 0);
            let b = nth_joint(r.game, path, 4);
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

/// The stack address of a wing's scale joint's descriptor: below its Arwing's by the Arwing's
/// setup's frame (0x48) and the create function's.
pub const WING_FRAME: u32 = ARWING_PLANE_FRAME - 0x48 - 0x20;

/// A close group's Arwing's setup (map 2), `grCorneria_801DE8E4`: as [`init_flying`] with its
/// background animations, and which gun fires first.
#[doc(alias = "grCorneria_801DE8E4")]
pub fn init_arwing(r: &mut Running, gobj: GObjId) {
    init_flying(r, gobj, true);
    let gun = r.game.objs.rng.below(2);
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.arwing().x100 = Some(gun);
}

/// Whether a point is outside the blast zones, `grCorneria_801DEC08`.
#[doc(alias = "grCorneria_801DEC08")]
pub fn outside_blast_zones(game: &Game, pos: Vec3) -> bool {
    let (x, y) = (reg(pos.x), reg(pos.y));
    x > reg(stage::blast_right(game))
        || x < reg(stage::blast_left(game))
        || y > reg(stage::blast_top(game))
        || y < reg(stage::blast_bottom(game))
}

/// Whether a point in front of the stage (z above -30) is within 30 of the blast zones or past
/// them, `grCorneria_801DEC94`.
#[doc(alias = "grCorneria_801DEC94")]
pub fn near_blast_zones(game: &Game, pos: Vec3) -> i32 {
    if reg(pos.z) > -30.0 {
        let (x, y) = (reg(pos.x), reg(pos.y));
        if x > fp::fsubs(reg(stage::blast_right(game)), 30.0)
            || x < fp::fadds(30.0, reg(stage::blast_left(game)))
            || y > fp::fsubs(reg(stage::blast_top(game)), 30.0)
            || y < fp::fadds(30.0, reg(stage::blast_bottom(game)))
        {
            return 1;
        }
    }
    0
}

/// The group's offset into the roll animations (`get_grp_off`): 0 for groups 1 to 3, 1 for the
/// far group 4, -1 for none.
fn group_offset(game: &mut Game, gid: super::super::handles::GroundId) -> i32 {
    let slot = game.grounds[gid.0].u.arwing().xC8 as usize;
    match game.gr.corneria.groups[slot] {
        1..=3 => 0,
        4 => 1,
        _ => -1,
    }
}

/// Whether an Arwing fires this tick: every `period` ticks flown, with odds of one in `odds`
/// (the match's or the demo's).
fn fires(r: &mut Running, gid: super::super::handles::GroundId) -> i32 {
    let p = param(r.game);
    let (period, odds) = if g(r.game).demo == 0 {
        (p.word(r.data, p::FIRE_PERIOD), p.word(r.data, p::FIRE_ODDS))
    } else {
        (
            p.word(r.data, p::FIRE_PERIOD_DEMO),
            p.word(r.data, p::FIRE_ODDS_DEMO),
        )
    };
    let flown = r.game.grounds[gid.0].u.arwing().xF0;
    // C's `%`, which truncates.
    if flown.wrapping_rem(period) == 0 && (odds == 0 || r.game.objs.rng.below(odds) == 0) {
        1
    } else {
        0
    }
}

/// A flying Arwing's process (maps 2 and 10), `grCorneria_801DED50`: nothing while the
/// Arwings are held. While its path lives: a close one turns side on and now and then rolls
/// (its rolling part flat between rolls), follows its path's place with its collision object
/// at its rolling part's place and roll, and, inside the blast zones and level, now and then
/// fires a laser straight ahead with a flash; a far one fires at a random player's fighter,
/// over the stage's right, from alternating guns. Once its path is gone, it and its articles go.
#[doc(alias = "grCorneria_801DED50")]
pub fn proc_arwing(r: &mut Running, gobj: GObjId) {
    if g(r.game).frozen {
        return;
    }
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
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
        let a = nth_joint(r.game, gobj, 1).expect("grcorneria.c: jobj");
        jobj::set_rotation_z(&mut r.game.objs.jobjs, a, 0.0);
    }
    let part = GROUP_PARTS[group as usize];
    match g(r.game).types[slot] {
        1..=9 => {
            jobj::set_rotation_y(&mut r.game.objs.jobjs, j, -std::f32::consts::FRAC_PI_2);
            let roll = r.game.grounds[gid.0].u.arwing().xF4;
            match roll {
                0 => {
                    if r.game.grounds[gid.0].u.arwing().xF8 <= 0 {
                        let state = r.game.objs.rng.below(5).wrapping_add(1);
                        r.game.grounds[gid.0].u.arwing().xF4 = state;
                        let off = group_offset(r.game, gid);
                        // An offset of -1 (no group) reads the row before's second entry.
                        let flat = 2 * state + off;
                        let anim = ROLL_ANIMS[(flat / 2) as usize][(flat % 2) as usize];
                        anime::start(r.game, r.data, gobj, part, 7, anim, 0.0, 1.0, false);
                    } else {
                        let pj = nth_joint(r.game, gobj, part).expect("grcorneria.c: jobj");
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
            let mut pos = arwing_pos(r, path);
            jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
            let pj = nth_joint(r.game, gobj, part);
            pos = world_position(&mut r.game.objs, r.data, pj, None);
            let coll = r.game.grounds[gid.0].u.arwing().xDC;
            if let Some(c) = coll {
                let cg = ground_of(r.game, c);
                r.game.grounds[cg.0].u.arwing_coll().xE0 = pos;
            }
            let rot_z = r.game.objs.jobjs[pj.expect("grcorneria.c: jobj")].rotate.z;
            let mut angle = reg(rot_z);
            if let Some(c) = coll {
                let cg = ground_of(r.game, c);
                r.game.grounds[cg.0].u.arwing_coll().xDC = rot_z;
            }
            let f = fires(r, gid);
            r.game.grounds[gid.0].u.arwing().xFC = f;
            if r.game.grounds[gid.0].u.arwing().xFC != 0 {
                r.game.grounds[gid.0].u.arwing().xFC = 0;
                if !outside_blast_zones(r.game, pos) {
                    let pi = std::f64::consts::PI;
                    let tau = std::f64::consts::TAU;
                    while angle < -pi {
                        angle = fp::frsp(fp::fadd(angle, tau));
                    }
                    while angle > pi {
                        angle = fp::frsp(fp::fsub(angle, tau));
                    }
                    let abs = if angle < 0.0 { fp::fneg(angle) } else { angle };
                    if abs < reg(0.052_359_88) || abs > reg(3.089_232_7) {
                        r.sfx_play(0x55733, 0x7F, 0x40);
                        let k = param(r.game).f(r.data, p::ARWING_SCALE);
                        if group_offset(r.game, gid) == 1 {
                            let b = nth_joint(r.game, gobj, 7);
                            arwinglaser::spawn_straight(r, gobj, b, 2, -1.0, k);
                        } else {
                            let b = nth_joint(r.game, gobj, 5);
                            arwinglaser::spawn_straight(r, gobj, b, 0, -1.0, k);
                        }
                        let script = r.data.ptr(param(r.game).0.at(p::FIRE_SCRIPT));
                        material::start_script(r.game, r.data, gobj, script, 0);
                    }
                }
            }
        }
        10..=13 => {
            let f = fires(r, gid);
            r.game.grounds[gid.0].u.arwing().xFC = f;
            let mut pos = arwing_pos(r, path);
            let x = reg(pos.x);
            if 500.0 < x && x < reg(ARWING_MAX_X) && r.game.grounds[gid.0].u.arwing().xFC != 0 {
                r.game.grounds[gid.0].u.arwing().xFC = 0;
                let mut player = r.game.objs.rng.below(4);
                for tries in 0..4 {
                    player = player.wrapping_add(tries) & 3;
                    // Gm_PKind_NA.
                    if r.game.player_pkind(player) != 3 {
                        break;
                    }
                }
                pos = r.game.player_pos(player);
                pos.y = mem(fp::fadds(reg(pos.y), 5.0));
                r.sfx_play(0x55734, 0x7F, 0x40);
                let k = param(r.game).f(r.data, p::ARWING_SCALE);
                if group_offset(r.game, gid) == 1 {
                    let b = nth_joint(r.game, gobj, 7);
                    arwinglaser::spawn_aimed(r, gobj, b, pos, 3, 3, k, 0x55734);
                } else {
                    let gun = r.game.grounds[gid.0]
                        .u
                        .arwing()
                        .x100
                        .expect("grcorneria.c: an Arwing's gun read before its setup wrote it");
                    let b = nth_joint(r.game, gobj, if gun != 0 { 5 } else { 6 });
                    arwinglaser::spawn_aimed(r, gobj, b, pos, 1, 3, k, 0x55734);
                    r.game.grounds[gid.0].u.arwing().x100 = Some(gun.wrapping_add(1) & 1);
                }
                let script = r.data.ptr(param(r.game).0.at(p::FIRE_SCRIPT));
                material::start_script(r.game, r.data, gobj, script, 0);
            }
        }
        _ => {}
    }
    let v = r.game.grounds[gid.0].u.arwing();
    v.xF0 = v.xF0.wrapping_add(1);
}

/// The far group's Arwing's setup (map 10), `grCorneria_801DF8D0`: as [`init_flying`].
#[doc(alias = "grCorneria_801DF8D0")]
pub fn init_far_arwing(r: &mut Running, gobj: GObjId) {
    init_flying(r, gobj, false);
}

/// The far group's Arwing's process (map 10), `grCorneria_801DFBCC`: a flying Arwing's.
#[doc(alias = "grCorneria_801DFBCC")]
pub fn proc_far_arwing(r: &mut Running, gobj: GObjId) {
    proc_arwing(r, gobj);
}

/// Map 6's setup, `stageGObj6_OnInit` (`Ground_StartMapAnim`): its animation.
#[doc(alias = "stageGObj6_OnInit@801DFBF0")]
pub fn init_map6(r: &mut Running, gobj: GObjId) {
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The city's middle part's setup (map 8), `grCorneria_801DFC2C`: moving, not waiting.
#[doc(alias = "grCorneria_801DFC2C")]
pub fn init_city_middle(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let v = r.game.grounds[gid.0].u.corneria_city8();
    v.xC6 = 0;
    v.still = false;
    v.xC8 = [0; 6];
    v.xC4 = 0;
    r.game.grounds[gid.0].x11_flags.b012 = 1;
}

/// A city part's move (the body of `grCorneria_801DFC98` and its two siblings): it slides by
/// the city's speed, at the Great Fox's sinking depth; once it reaches its end less 1400 it
/// makes the next part (`next`), and past its end plus 1400 it goes.
fn proc_city(r: &mut Running, gobj: GObjId, width: f32, next: i32) {
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let speed = reg(param(r.game).f(r.data, p::CITY_SPEED));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.translate.x = mem(fp::fadds(reg(o.translate.x), speed))
    });
    let y = mem(fp::fneg(reg(sink_depth(r.game))));
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    let gid = ground_of(r.game, gobj);
    let state = match r.game.grounds[gid.0].u {
        super::GroundVars::CorneriaCity8(ref v) => v.xC5,
        super::GroundVars::CorneriaCity(ref v) => v.xC5,
        _ => panic!("grcorneria.c: a city part's variables"),
    } as i8;
    let edge = |r: &Running, add: f64| {
        let half = fp::fmuls(reg(width), reg(scale(r.game)));
        fp::fmadds(half, 0.5, add)
    };
    match state {
        0 => {
            let x = reg(r.game.objs.jobjs[j].translate.x);
            if x >= edge(r, -1400.0) {
                next_city_part(r, gobj, next);
                set_city_state(r.game, gid, 1);
            }
        }
        1 => {
            let x = reg(r.game.objs.jobjs[j].translate.x);
            if x >= edge(r, 1400.0) {
                setup::destroy(r, Some(gobj));
            }
        }
        _ => {}
    }
}

fn set_city_state(game: &mut Game, gid: super::super::handles::GroundId, s: u8) {
    match &mut game.grounds[gid.0].u {
        super::GroundVars::CorneriaCity8(v) => v.xC5 = s,
        super::GroundVars::CorneriaCity(v) => v.xC5 = s,
        _ => panic!("grcorneria.c: a city part's variables"),
    }
}

/// The city's middle part's process (map 8), `grCorneria_801DFC98`: unless waiting or still, it
/// moves and makes the last part (map 9).
#[doc(alias = "grCorneria_801DFC98")]
pub fn proc_city_middle(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let v = r.game.grounds[gid.0].u.corneria_city8();
    if v.xC4 == 0 && !v.still {
        proc_city(r, gobj, CITY_WIDTH, 9);
    }
}

/// The city's last part's setup (map 9), `grCorneria_801DFEB8`: moving, its animation, not
/// waiting.
#[doc(alias = "grCorneria_801DFEB8")]
pub fn init_city_last(r: &mut Running, gobj: GObjId) {
    init_city_end(r, gobj);
}

fn init_city_end(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.corneria_city().still = false;
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[gid.0].u.corneria_city().xC4 = 0;
    r.game.grounds[gid.0].x11_flags.b012 = 1;
}

/// The city's last part's process (map 9), `grCorneria_801DFF20`: it moves and makes the first
/// part (map 4).
#[doc(alias = "grCorneria_801DFF20")]
pub fn proc_city_last(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let v = r.game.grounds[gid.0].u.corneria_city();
    if v.xC4 == 0 && !v.still {
        proc_city(r, gobj, CITY_WIDTH, 4);
    }
}

/// The city's first part's setup (map 4), `grCorneria_801E0140`: as the last part's.
#[doc(alias = "grCorneria_801E0140")]
pub fn init_city_first(r: &mut Running, gobj: GObjId) {
    init_city_end(r, gobj);
}

/// The city's first part's process (map 4), `grCorneria_801E01A8`: it moves (its width 4800)
/// and makes the middle part (map 8).
#[doc(alias = "grCorneria_801E01A8")]
pub fn proc_city_first(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let v = r.game.grounds[gid.0].u.corneria_city();
    if v.xC4 == 0 && !v.still {
        proc_city(r, gobj, CITY_WIDTH_LAST, 8);
    }
}

/// Makes the city's next part behind this one, `grCorneria_801E03C8`: map `id` placed the two
/// parts' half widths before it, moving, and drawn in the stage's layer.
#[doc(alias = "grCorneria_801E03C8")]
pub fn next_city_part(r: &mut Running, gobj: GObjId, id: i32) {
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let x = reg(r.game.objs.jobjs[j].translate.x);
    let new = create(r, id, CITY_FRAME).expect("grcorneria.c: a city part");
    let (this, next) = match id {
        8 => (CITY_WIDTH_LAST, CITY_WIDTH),
        9 => (CITY_WIDTH, CITY_WIDTH),
        4 => (CITY_WIDTH, CITY_WIDTH_LAST),
        _ => (0.0, 0.0),
    };
    if matches!(id, 4 | 8 | 9) {
        let s = reg(scale(r.game));
        let half = fp::fmuls(fp::fmuls(reg(next), s), 0.5);
        let nx = mem(fp::fsubs(x, fp::fmadds(fp::fmuls(reg(this), s), 0.5, half)));
        let nj = jobj_of(r.game, new).expect("grcorneria.c: jobj");
        jobj::set_srt(&mut r.game.objs.jobjs, nj, |o| o.translate.x = nx);
        let ng = ground_of(r.game, new);
        set_city_state(r.game, ng, 0);
    }
    r.game.world.gx_relink(new, 3, 0);
}

/// The stack address of a city part's scale joint's descriptor: below the process loop by a
/// part's process's frame (0x40), `grCorneria_801E03C8`'s (0x48) and the create function's.
pub const CITY_FRAME: u32 = PROC_LOOP - 0x40 - 0x48 - 0x20;

/// The stack address of a talk window's scale joint's descriptor: below the process loop by the
/// talk's process's frame (0x20) and the create function's.
pub const WINDOW_FRAME: u32 = PROC_LOOP - 0x20 - 0x20;

/// The key of the talk's scale joint, which a fighter's taunt makes (`grCorneria_801E2B80`'s frame):
/// distinct from every live one by the time it is made, which the setup's, its objects gone with
/// the load hook, stands in for.
pub const TALK_FRAME: u32 = INIT_FRAME;

/// Lays the city out for the demo, `grCorneria_801E0678`: map 11 goes, and the three parts
/// stand still side by side.
#[doc(alias = "grCorneria_801E0678")]
pub fn demo_city(r: &mut Running) {
    if let Some(gobj) = map_gobj(r.game, 0xB) {
        setup::destroy(r, Some(gobj));
    }
    let s8 = map_gobj(r.game, 8).expect("grcorneria.c: map 8");
    let j8 = jobj_of(r.game, s8).expect("grcorneria.c: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j8, |o| o.translate.x = 0.0);
    let g8 = ground_of(r.game, s8);
    r.game.grounds[g8.0].u.corneria_city8().still = true;
    let s9 = map_gobj(r.game, 9).expect("grcorneria.c: map 9");
    let g9 = ground_of(r.game, s9);
    r.game.grounds[g9.0].u.corneria_city().still = true;
    let s = reg(scale(r.game));
    let half = fp::fmuls(fp::fmuls(reg(CITY_WIDTH), s), 0.5);
    let x9 = mem(fp::fnmadds(fp::fmuls(reg(CITY_WIDTH), s), 0.5, half));
    let j9 = jobj_of(r.game, s9).expect("grcorneria.c: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j9, |o| o.translate.x = x9);
    let s4 = map_gobj(r.game, 4).expect("grcorneria.c: map 4");
    let g4 = ground_of(r.game, s4);
    r.game.grounds[g4.0].u.corneria_city().still = true;
    let half = fp::fmuls(fp::fmuls(reg(CITY_WIDTH_LAST), s), 0.5);
    let x4 = mem(fp::fmadds(fp::fmuls(reg(CITY_WIDTH), s), 0.5, half));
    let j4 = jobj_of(r.game, s4).expect("grcorneria.c: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j4, |o| o.translate.x = x4);
}

/// Which city part a place's x is over, by the parts' widths from their places: 8, 9 or 4.
/// The original asserts when none is, which hangs it.
fn city_part_at(game: &Game, x: f64) -> i32 {
    let s = reg(scale(game));
    for (map, width) in [(8, CITY_WIDTH), (9, CITY_WIDTH), (4, CITY_WIDTH_LAST)] {
        if let Some(gobj) = map_gobj(game, map) {
            let j = jobj_of(game, gobj).expect("grcorneria.c: jobj");
            let px = reg(game.objs.jobjs[j].translate.x);
            let w = fp::fmuls(reg(width), s);
            if x > fp::fnmsubs(w, 0.5, px) && x < fp::fmadds(w, 0.5, px) {
                return map;
            }
        }
    }
    panic!("grcorneria.c: no city part under x {x} (the original's assertion)")
}

/// Which city part the camera looks at, `grCorneria_801E08CC`.
#[doc(alias = "grCorneria_801E08CC")]
pub fn city_under_camera(game: &Game) -> i32 {
    let cam = super::super::camera::interest(&game.camera.cam);
    city_part_at(game, reg(cam.x))
}

/// Which city part an x is over, `grCorneria_801E0A74`.
#[doc(alias = "grCorneria_801E0A74")]
pub fn city_at(game: &Game, x: f32) -> i32 {
    city_part_at(game, reg(x))
}

/// The sky's setup (map 7), `grCorneria_801E0C3C`: its animation, its own fog off, the guide
/// points, its materials' flags, drawn in the background layer.
#[doc(alias = "grCorneria_801E0C3C")]
pub fn init_sky(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let m = gp_map(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[gid.0].x10_flags.b2 = false;
    let n = |r: &Running, k: i32| nth_joint(r.game, gobj, k);
    let points = [n(r, 5), n(r, 2), n(r, 4), n(r, 3), n(r, 7), n(r, 6)];
    super::ground::set_guide_points(r.game, r.data, points);
    material::clear_tree_flags(r.game, j, 1 << 27);
    material::set_tree_flags(r.game, j, 1 << 29);
    r.game.grounds[gid.0].x11_flags.b012 = 2;
}

/// The sky's process (map 7), `grCorneria_801E0D30`: it rides the Great Fox's sinking.
#[doc(alias = "grCorneria_801E0D30")]
pub fn proc_sky(r: &mut Running, gobj: GObjId) {
    if let Some(j) = jobj_of(r.game, gobj) {
        let y = mem(fp::fneg(reg(sink_depth(r.game))));
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    }
}

/// The talk window's setup (map 5), `grCorneria_801E0DE4`.
#[doc(alias = "grCorneria_801E0DE4")]
pub fn init_window(r: &mut Running, gobj: GObjId) {
    window_setup(r, gobj);
}

/// The talk window's process (map 5), `grCorneria_801E0E14`.
#[doc(alias = "grCorneria_801E0E14")]
pub fn proc_window(r: &mut Running, gobj: GObjId) {
    proc_line(r, gobj);
}

/// L and R on the second controller (`PAD_TRIGGER_L`, `PAD_TRIGGER_R`).
const TRIGGER_L: u32 = 0x40;
const TRIGGER_R: u32 = 0x20;

/// The debugging talk, `grCorneria_801E0E40`: in the debug cameras, L or R on the second
/// controller (the master status) steps the talk number and starts that talk, if none plays.
#[doc(alias = "grCorneria_801E0E40")]
pub fn debug_talk(r: &mut Running) {
    let cam = &r.game.camera.cam;
    if !super::super::camera::is_debug_free(cam) && !super::super::camera::is_debug_follow(cam) {
        return;
    }
    let trigger = r.game.pads.master[1].trigger;
    if trigger & (TRIGGER_L | TRIGGER_R) == 0 {
        return;
    }
    if map_gobj(r.game, 0xC).is_some() {
        return;
    }
    let c = g(r.game);
    if trigger & TRIGGER_L != 0 {
        c.talk = c.talk.wrapping_sub(1);
    } else {
        c.talk = c.talk.wrapping_add(1);
    }
    if c.talk < 0 {
        c.talk = 0;
    }
    if c.talk as u32 >= 0x14 {
        c.talk = 0x13;
    }
    crate::game::code::os_report();
    let w = create(r, 0xC, TALK_FRAME).expect("grcorneria.c: the talk");
    let talk = g(r.game).talk;
    set_talk(r, w, talk);
}

/// Sets which talk the talk object (map 12) plays, `grCorneria_801E0F34`, from its first line
/// at once.
#[doc(alias = "grCorneria_801E0F34")]
pub fn set_talk(r: &mut Running, gobj: GObjId, val: i32) {
    let gid = ground_of(r.game, gobj);
    let mut val = val;
    let v = r.game.grounds[gid.0].u.starfox_talk();
    v.xCC = 0;
    v.xC8 = 0;
    if val < 0 {
        val = 0;
    }
    if val as u32 >= 0x14 {
        val = 0x13;
    }
    v.xC4 = val as u32;
}

/// The talk's process (map 12), `grCorneria_801E0F6C` (`Ground_UpdateStarFoxSequence` with the
/// window, map 5): the players' displays hidden; with no line showing, the next line starts once
/// its wait runs out, or the talk ends.
#[doc(alias = "grCorneria_801E0F6C")]
pub fn proc_talk(r: &mut Running, gobj: GObjId) {
    update_sequence(r, gobj, 5, Stage::Corneria);
}

/// Which stage's objects a talk makes.
#[derive(Clone, Copy)]
pub enum Stage {
    Corneria,
    Venom,
}

/// `Ground_UpdateStarFoxSequence`: the talk's line machine, its window made by the stage's
/// create function.
pub fn update_sequence(r: &mut Running, gobj: GObjId, window: i32, stage: Stage) {
    r.game.hide_damage_digits();
    r.game.hide_scores();
    if map_gobj(r.game, window).is_some() {
        return;
    }
    let gid = ground_of(r.game, gobj);
    let (talk, line) = {
        let v = r.game.grounds[gid.0].u.starfox_talk();
        (v.xC4, v.xC8 as u32)
    };
    if talk_has_line(talk, line) {
        let v = r.game.grounds[gid.0].u.starfox_talk();
        let wait = v.xCC;
        v.xCC = wait.wrapping_sub(1);
        if wait < 0 {
            let w = match stage {
                Stage::Corneria => create(r, window, WINDOW_FRAME),
                Stage::Venom => super::venom::create(r, window, WINDOW_FRAME),
            }
            .expect("a talk's window");
            start_talk_line(r, w, talk, line);
            let v = r.game.grounds[gid.0].u.starfox_talk();
            v.xC8 = v.xC8.wrapping_add(1);
            v.xCC = 0;
        }
    } else {
        let v = r.game.grounds[gid.0].u.starfox_talk();
        let wait = v.xCC;
        v.xCC = wait.wrapping_sub(1);
        if wait < 0 {
            setup::destroy(r, Some(gobj));
        }
    }
}

/// The talk's destroy callback (map 12), `grCorneria_801E1030`: the players' displays shown
/// again.
#[doc(alias = "grCorneria_801E1030")]
pub fn talk_destroyed(r: &mut Running, _gobj: GObjId) {
    r.game.show_damage_digits();
    r.show_scores();
}

/// Map 11's process (`grCorneria_801E1060`): the city laid out by a scene's progress, which only
/// another mode runs (the stage's load hook destroys map 11 before its first tick).
pub fn proc_map11(_r: &mut Running, _gobj: GObjId) {
    super::super::code::unported_call("gm_801883C0");
}

/// The guns' hurtbox items' callback for a hit they deal, `fn_801E12D0`: nothing.
#[doc(alias = "fn_801E12D0")]
pub fn gun_dealt(_r: &mut Running, _item: GObjId, _g: super::super::handles::GroundId) {}

/// The guns' hurtbox items' callback for a hit they take, `fn_801E12D4`: the damage adds up,
/// and enough breaks the guns (state 4), their hurtboxes off.
#[doc(alias = "fn_801E12D4")]
pub fn gun_hit(r: &mut Running, gid: super::super::handles::GroundId, damage: f32) {
    let hp = reg(param(r.game).f(r.data, p::GUN_HP));
    let v = r.game.grounds[gid.0].u.corneria();
    v.x114 = mem(fp::fadds(reg(v.x114), reg(damage)));
    crate::game::code::os_report();
    if reg(v.x114) >= hp {
        v.x108 = 4;
        let (l, rt) = (v.left_cannon, v.right_cannon);
        material::item_hurtbox_off(r.game, l.expect("grcorneria.c: the left gun"));
        material::item_hurtbox_off(r.game, rt.expect("grcorneria.c: the right gun"));
    }
}

/// The guns, `grCorneria_801E1348`. Resting (0), once their wait runs out they charge (1) a
/// single big shot or, mostly, a volley, with a sound and the charge's effects, their items in
/// the charge state and their hurtboxes off; charged, they aim (2) until their wait runs out or
/// a fighter stands level with them, hurtboxes on; firing (3), a shot every 10 ticks from
/// alternate guns, then a random rest. Broken (4): smoke, a quake, a sound, the broken guns
/// shown and their collision off (5).
#[doc(alias = "grCorneria_801E1348")]
pub fn guns(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let p = param(r.game);
    match r.game.grounds[gid.0].u.corneria().x108 {
        0 => {
            if r.game.grounds[gid.0].u.corneria().x110 != 0 {
                let v = r.game.grounds[gid.0].u.corneria();
                v.x110 = v.x110.wrapping_sub(1);
                return;
            }
            let chance = reg(p.f(r.data, p::GUN_BIG_CHANCE));
            if reg(r.game.objs.rng.draw_f32()) <= chance {
                let charge = p.i(r.data, p::GUN_BIG_CHARGE);
                let v = r.game.grounds[gid.0].u.corneria();
                v.x119 = 0;
                v.x118 = 1;
                v.x10C = charge;
                r.stage_sound(0x55739);
            } else {
                let lo = p.i(r.data, p::GUN_SHOTS_MIN);
                let hi = p.i(r.data, p::GUN_SHOTS_MAX);
                let shots = grlib::rand_range(&mut r.game.objs.rng, lo, hi);
                let charge = p.i(r.data, p::GUN_CHARGE);
                let v = r.game.grounds[gid.0].u.corneria();
                v.x119 = 1;
                v.x118 = shots as i8;
                v.x10C = charge;
                r.stage_sound(0x5573A);
            }
            for part in [2, 3] {
                let j = nth_joint(r.game, gobj, part);
                let _ = grlib::generator_on(r.game, r.data, 0xBC, 0, j);
            }
            let v = r.game.grounds[gid.0].u.corneria();
            let (l, rt) = (
                v.left_cannon.expect("a gun"),
                v.right_cannon.expect("a gun"),
            );
            let anim = crate::game::it::item::change::ANIM_UPDATE;
            crate::game::it::item::change_state(r, l, 2, anim);
            crate::game::it::item::change_state(r, rt, 2, anim);
            material::item_hurtbox_off(r.game, l);
            material::item_hurtbox_off(r.game, rt);
            r.game.grounds[gid.0].u.corneria().x108 = 1;
        }
        1 => {
            if r.game.grounds[gid.0].u.corneria().x10C != 0 {
                let v = r.game.grounds[gid.0].u.corneria();
                v.x10C = v.x10C.wrapping_sub(1);
                return;
            }
            for part in [2, 3] {
                let j = nth_joint(r.game, gobj, part);
                crate::hsd::generator::kill_on_jobj(
                    &mut r.game.objs,
                    r.data,
                    &mut crate::game::code::ParticleHooks,
                    j,
                );
            }
            let v = r.game.grounds[gid.0].u.corneria();
            let (l, rt) = (
                v.left_cannon.expect("a gun"),
                v.right_cannon.expect("a gun"),
            );
            let anim = crate::game::it::item::change::ANIM_UPDATE;
            crate::game::it::item::change_state(r, l, 0, anim);
            crate::game::it::item::change_state(r, rt, 0, anim);
            let aim = p.i(r.data, p::GUN_AIM);
            let v = r.game.grounds[gid.0].u.corneria();
            v.x10C = aim;
            v.x108 = 2;
        }
        2 => {
            let j = nth_joint(r.game, gobj, 2);
            let gun = world_position(&mut r.game.objs, r.data, j, None);
            let mut found = false;
            let mut f = r.game.world.first_in(P_LINK_FIGHTER);
            while let Some(fighter) = f {
                let fpos = r.game.ft_pos(fighter);
                let d = fp::fsubs(reg(gun.y), reg(fpos.y));
                let d = if d < 0.0 { fp::fneg(d) } else { d };
                if d <= 10.0 {
                    found = true;
                    break;
                }
                f = r.game.world.gobj(fighter).next;
            }
            let v = r.game.grounds[gid.0].u.corneria();
            if found {
                v.x10C = 0;
            }
            if v.x10C == 0 {
                v.x108 = 3;
                let (l, rt) = (
                    v.left_cannon.expect("a gun"),
                    v.right_cannon.expect("a gun"),
                );
                material::item_hurtbox_on(r.game, l);
                material::item_hurtbox_on(r.game, rt);
                return;
            }
            v.x10C = v.x10C.wrapping_sub(1);
        }
        3 => {
            let v = r.game.grounds[gid.0].u.corneria();
            if v.x118 != 0 {
                if v.x110 != 0 {
                    v.x110 = v.x110.wrapping_sub(1);
                    return;
                }
                let side = v.x11A;
                let kind = v.x119 as i8;
                let mut off = Vec3::new(
                    mem(fp::fadds(20.0, reg(SHIP_POINTS[1].x))),
                    mem(fp::fadds(2.0, reg(SHIP_POINTS[1].y))),
                    0.0,
                );
                let gun = if side != 0 {
                    off.z = 9.0;
                    nth_joint(r.game, gobj, 2)
                } else {
                    off.z = -9.0;
                    nth_joint(r.game, gobj, 3)
                };
                let _ = grlib::generator_on(r.game, r.data, 0xC0, 0, gun);
                greatfoxlaser::spawn(r, gobj, off, i32::from(kind));
                r.stage_sound(0x55735);
                let v = r.game.grounds[gid.0].u.corneria();
                v.x11A ^= 1;
                v.x110 = 0xA;
                v.x118 = v.x118.wrapping_sub(1);
                return;
            }
            let range = p.i(r.data, p::GUN_REST_ADD);
            let add = if range != 0 {
                r.game.objs.rng.below(range)
            } else {
                0
            };
            let rest = fp::fctiwz(fp::fadds(
                reg(p.f(r.data, p::GUN_REST)),
                fp::frsp(f64::from(add)),
            ));
            let v = r.game.grounds[gid.0].u.corneria();
            v.x110 = rest;
            v.x10C = 0;
            v.x108 = 0;
        }
        4 => {
            for part in [2, 3] {
                let j = nth_joint(r.game, gobj, part);
                let _ = grlib::generator_on(r.game, r.data, 0xE3, 0, j);
            }
            let at = g(r.game).points[1];
            super::super::camera::request_quake(r, 4, Some(at));
            r.stage_sound(0x76);
            let whole = nth_joint(r.game, gobj, 6);
            jobj::set_flags(&mut r.game.objs.jobjs, whole, jobj::HIDDEN);
            let broken = nth_joint(r.game, gobj, 5);
            jobj::clear_flags(&mut r.game.objs.jobjs, broken, jobj::HIDDEN);
            r.game.geometry_mut().disable_joint(4);
            r.game.grounds[gid.0].u.corneria().x108 = 5;
        }
        _ => {}
    }
}

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

/// Puts the Great Fox's copy (map 0) where the Great Fox is, `grCorneria_801E1878`, and
/// measures the camera limits and blast zones again.
#[doc(alias = "grCorneria_801E1878")]
pub fn move_copy(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let copy = r.game.grounds[gid.0].u.corneria().x128;
    let cj = copy
        .and_then(|c| jobj_of(r.game, c))
        .expect("grcorneria.c: the copy's jobj");
    let pos = r.game.objs.jobjs[j].translate;
    jobj::set_translate(&mut r.game.objs.jobjs, cj, pos);
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// The Great Fox's drift, `grCorneria_801E1970`: every so often new random accelerations; its
/// speeds capped, its drift stepped (last tick's kept), the accelerations turned back past the
/// box's edges, and its place set.
#[doc(alias = "grCorneria_801E1970")]
pub fn drift(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("grcorneria.c: jobj");
    let p = param(r.game);
    let accel = reg(p.f(r.data, p::DRIFT_ACCEL));
    if r.game.grounds[gid.0].u.corneria().x104 <= 0 {
        let rand = reg(r.game.objs.rng.draw_f32());
        let lo = reg(p.f(r.data, p::DRIFT_TIME_MIN));
        let a = fp::fsubs(reg(p.f(r.data, p::DRIFT_TIME_MAX)), lo);
        let t = fp::fctiwz(fp::fmadds(a, rand, lo));
        r.game.grounds[gid.0].u.corneria().x104 = t;
        let rand = reg(r.game.objs.rng.draw_f32());
        let ax = mem(fp::fmuls(accel, fp::fmuls(2.0, fp::fsubs(rand, 0.5))));
        r.game.grounds[gid.0].u.corneria().xF4 = ax;
        let rand = reg(r.game.objs.rng.draw_f32());
        let ay = mem(fp::fmuls(accel, fp::fmuls(2.0, fp::fsubs(rand, 0.5))));
        r.game.grounds[gid.0].u.corneria().xF8 = ay;
    }
    let max = reg(p.f(r.data, p::DRIFT_SPEED));
    let lim_x = reg(p.f(r.data, p::DRIFT_X));
    let lim_y = reg(p.f(r.data, p::DRIFT_Y));
    let v = r.game.grounds[gid.0].u.corneria();
    v.x104 = v.x104.wrapping_sub(1);
    v.xE4.z = mem(fp::fadds(reg(v.xE4.z), reg(v.xF4)));
    v.xF0 = mem(fp::fadds(reg(v.xF0), reg(v.xF8)));
    let vel = reg(v.xE4.z);
    if vel > max {
        v.xE4.z = mem(max);
    } else if vel < fp::fneg(max) {
        v.xE4.z = mem(fp::fneg(max));
    }
    let vel = reg(v.xF0);
    if vel > max {
        v.xF0 = mem(max);
    } else if vel < fp::fneg(max) {
        v.xF0 = mem(fp::fneg(max));
    }
    v.xE4.x = v.offset_x;
    v.xE4.y = v.offset_y;
    v.offset_x = mem(fp::fadds(reg(v.offset_x), reg(v.xE4.z)));
    v.offset_y = mem(fp::fadds(reg(v.offset_y), reg(v.xF0)));
    let o = reg(v.offset_x);
    if o > lim_x {
        v.xF4 = mem(fp::fneg(accel));
    } else if o < fp::fneg(lim_x) {
        v.xF4 = mem(accel);
    }
    let o = reg(v.offset_y);
    if o > lim_y {
        v.xF8 = mem(fp::fneg(accel));
    } else if o < fp::fneg(lim_y) {
        v.xF8 = mem(accel);
    }
    let pos = Vec3::new(
        mem(fp::fadds(reg(v.base_x), reg(v.offset_x))),
        mem(fp::fadds(reg(v.base_y), reg(v.offset_y))),
        0.0,
    );
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
}

/// The city parts' colors (`grCn_804DB218` to `grCn_804DB238`): the stage's parameter colors
/// over each part.
const CITY_COLORS: [[[u8; 4]; 3]; 3] = [
    [
        [0xAE, 0xBA, 0xD6, 0xFF],
        [0xD7, 0xE4, 0xF6, 0xFF],
        [0xE0, 0xEC, 0xFA, 0xFF],
    ],
    [
        [0x6A, 0x9E, 0xAD, 0xFF],
        [0x4F, 0x94, 0xB5, 0xFF],
        [0x49, 0x6E, 0x3F, 0xFF],
    ],
    [
        [0x6A, 0x9E, 0xAD, 0xFF],
        [0x4F, 0x94, 0xB5, 0xFF],
        [0x00, 0x50, 0x91, 0xFF],
    ],
];

/// The stage's parameter colors by the city part under the camera, `grCorneria_801E2110`.
#[doc(alias = "grCorneria_801E2110")]
pub fn city_colors(r: &mut Running) {
    use super::ground::{ParamColor, set_param_color};
    let k = match city_under_camera(r.game) {
        8 => 0,
        9 => 1,
        4 => 2,
        _ => return,
    };
    let [a, b, c] = CITY_COLORS[k].map(|[r_, g_, b_, a_]| crate::game::structs::_GXColor {
        r: r_,
        g: g_,
        b: b_,
        a: a_,
    });
    // Ground_801C058C, 801C05EC, 801C05A4, 801C055C, 801C05BC and 801C0574.
    set_param_color(r.game, ParamColor::C4, b.clone());
    set_param_color(r.game, ParamColor::C8, a);
    set_param_color(r.game, ParamColor::CC, b);
    set_param_color(r.game, ParamColor::D0, c.clone());
    set_param_color(r.game, ParamColor::D4, c.clone());
    set_param_color(r.game, ParamColor::D8, c);
}

/// The Great Fox's sinking, `grCorneria_801E2228`. Not sinking: over the city's first part (map
/// 4) it may start, one chance in three once each time the part comes round (the chance reset
/// over the last part, map 9). Sinking, for 900 ticks it eases down to its depth limit, then
/// eases back up to 0 and stops.
#[doc(alias = "grCorneria_801E2228")]
pub fn sink(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    if !r.game.grounds[gid.0].u.corneria().sinking() {
        if !r.game.grounds[gid.0].u.corneria().drawn() && city_under_camera(r.game) == 4 {
            if r.game.objs.rng.below(3) == 0 {
                let v = r.game.grounds[gid.0].u.corneria();
                v.set_sinking(true);
                v.x100 = 0;
                v.xFC = 0.0;
            }
            r.game.grounds[gid.0].u.corneria().set_drawn(true);
        }
        if city_under_camera(r.game) == 9 {
            r.game.grounds[gid.0].u.corneria().set_drawn(false);
        }
        return;
    }
    let s = reg(scale(r.game));
    let v = r.game.grounds[gid.0].u.corneria();
    let timer = v.x100;
    v.x100 = timer.wrapping_add(1);
    let step = reg(0.005);
    let speed = reg(v.xFC);
    let t = fp::fdivs(speed, step);
    let travel = fp::fmadds(speed, t, fp::fmuls(fp::fmuls(-0.0025, t), t));
    if timer < 0x384 {
        let limit = fp::fmsubs(50.0, s, 250.0);
        if fp::fsubs(reg(v.xD0), limit) > travel {
            v.xFC = mem(fp::fadds(reg(v.xFC), step));
            if reg(v.xFC) > 3.0 {
                v.xFC = 3.0;
            }
        } else {
            v.xFC = mem(fp::fsubs(reg(v.xFC), step));
            if reg(v.xFC) < step {
                v.xFC = mem(step);
            }
        }
        v.xD0 = mem(fp::fsubs(reg(v.xD0), reg(v.xFC)));
        if reg(v.xD0) < limit {
            v.xD0 = mem(limit);
        }
    } else {
        if fp::fneg(reg(v.xD0)) > travel {
            v.xFC = mem(fp::fadds(speed, step));
            if reg(v.xFC) > 3.0 {
                v.xFC = 3.0;
            }
        } else {
            v.xFC = mem(fp::fsubs(speed, step));
            if reg(v.xFC) < step {
                v.xFC = mem(step);
            }
        }
        v.xD0 = mem(fp::fadds(reg(v.xD0), reg(v.xFC)));
        if reg(v.xD0) > 0.0 {
            v.xD0 = 0.0;
            v.set_sinking(false);
        }
    }
}

/// The front splash's callback, `grCorneria_801E2454`: the stage's effect 0x7534 (bank 30).
#[doc(alias = "grCorneria_801E2454")]
pub fn splash_front(r: &mut Running, pos: Vec3) {
    let _ = grlib::generator_at(r.game, r.data, 0x7534, 30, pos);
}

/// The back splash's callback, `grCorneria_801E2480`: the stage's effect 0x7530 (bank 30).
#[doc(alias = "grCorneria_801E2480")]
pub fn splash_back(r: &mut Running, pos: Vec3) {
    let _ = grlib::generator_at(r.game, r.data, 0x7530, 30, pos);
}

/// A splash configuration's callback by address.
pub fn splash(r: &mut Running, cb: u32, pos: Vec3) {
    match cb {
        SPLASH_FRONT => splash_front(r, pos),
        SPLASH_BACK => splash_back(r, pos),
        other => panic!("a spawn configuration with callback {other:#010x}"),
    }
}

/// The talk window's render callback, `smashTaunt_801E24AC`: hidden with its text while the
/// game is paused for debugging or the camera is free, else drawn as a stage object.
#[doc(alias = "smashTaunt_801E24AC")]
pub fn window_render(r: &mut Running, gobj: GObjId, pass: i32) {
    let gid = ground_of(r.game, gobj);
    let text = r.game.grounds[gid.0].u.smash_taunt().text;
    // Once the line closes its font the text is freed, but the window keeps pointing at it and
    // writes its `hidden` byte into the freed block, which the next text made there writes
    // before anything reads it: a freed text takes no write here.
    let set_hidden = |r: &mut Running, h: u8| {
        if let Some(t) = text.and_then(|t| r.game.sis.texts.get_mut(&t.0)) {
            t.hidden = h;
        }
    };
    if r.game.db_pause_flag(1)
        || r.game.db_pause_flag(2)
        || super::super::camera::is_free(&r.game.camera.cam)
    {
        set_hidden(r, 1);
        return;
    }
    set_hidden(r, 0);
    r.render_stage_object(gobj, pass);
}

/// Sets a talk line's window up, `smashTaunt_801E2550`: no text yet, its scale 1, the players'
/// displays hidden, and its own render callback.
#[doc(alias = "smashTaunt_801E2550")]
pub fn window_setup(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    r.game.grounds[gid.0].u.smash_taunt().text = None;
    super::ground::reset_scale(r.game, gobj);
    r.game.hide_damage_digits();
    r.game.hide_scores();
    r.game.world.gobj_mut(gobj).render = Some(CodeRef(WINDOW_RENDER));
}

/// Whether a talk has a line `line`, `grCorneria_801E2598`.
#[doc(alias = "grCorneria_801E2598")]
pub fn talk_has_line(talk: u32, line: u32) -> bool {
    TALKS[talk as usize][line as usize][0] != 0
}

/// Starts a talk line in its window, `grCorneria_801E25C4`: the other speakers' joints hidden,
/// the speaker's two joints and the window's (joint 5) opening, its text and voice kept for
/// when the window has opened.
#[doc(alias = "grCorneria_801E25C4")]
pub fn start_line(r: &mut Running, gobj: GObjId, line: i32, text: i32, sound: i32) {
    let gid = ground_of(r.game, gobj);
    {
        let v = r.game.grounds[gid.0].u.smash_taunt();
        v.line = line as i16;
        v.sis_data_idx = text as i16;
        v.sound_id = sound;
    }
    let speaker = r.game.grounds[gid.0].u.smash_taunt().line;
    let [j0, j1] = SPEAKER_JOINTS[speaker as usize].map(i32::from);
    for (i, pair) in SPEAKER_JOINTS.iter().enumerate() {
        if i != 0 && speaker as usize != i {
            for k in pair {
                let j = nth_joint(r.game, gobj, (*k).into());
                if j.is_some() {
                    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
                }
            }
        }
    }
    let (a, b, c) = (
        nth_joint(r.game, gobj, j0),
        nth_joint(r.game, gobj, j1),
        nth_joint(r.game, gobj, 5),
    );
    {
        let v = r.game.grounds[gid.0].u.smash_taunt();
        v.jobj0 = a;
        v.jobj1 = b;
        v.jobj2 = c;
        v.joint_idx0 = j0 as i16;
        v.joint_idx1 = j1 as i16;
        v.joint_idx2 = 5;
        v.state = 0;
        v.timer = 0xA;
    }
    for part in [j0, j1, 5] {
        anime::start(r.game, r.data, gobj, part, 7, 0, 0.0, 1.0, false);
    }
    let root = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        root,
    );
}

/// Starts talk `talk`'s line `line` in a window, `grCorneria_801E2738`.
#[doc(alias = "grCorneria_801E2738")]
pub fn start_talk_line(r: &mut Running, gobj: GObjId, talk: u32, line: u32) {
    let [speaker, text, sound] = TALKS[talk as usize][line as usize];
    start_line(r, gobj, speaker, text, sound);
}

/// A talk line's process (map 5), `grCorneria_801E277C`: its joints marked each tick
/// (`JOBJ_UNK_B27`). Opening (0), a sound on its tenth tick; opened, it loops, shows its text
/// (the stage's own font, font 1, on a canvas of its own) and plays its voice (1); after a
/// second, once the voice has ended, it closes (2), the text gone; closed, it goes.
#[doc(alias = "grCorneria_801E277C")]
pub fn proc_line(r: &mut Running, gobj: GObjId) {
    let gid = ground_of(r.game, gobj);
    let (a, b, c) = {
        let v = r.game.grounds[gid.0].u.smash_taunt();
        (v.jobj0, v.jobj1, v.jobj2)
    };
    for j in [a, b, c].into_iter().flatten() {
        material::set_own_flags(r.game, j, 1 << 27);
    }
    let v = r.game.grounds[gid.0].u.smash_taunt();
    let parts = [v.joint_idx0, v.joint_idx1, v.joint_idx2].map(i32::from);
    let state = v.state;
    match state {
        0 => {
            let v = r.game.grounds[gid.0].u.smash_taunt();
            let timer = v.timer;
            v.timer = timer.wrapping_sub(1);
            if timer == 0 {
                r.stage_sound(0xB2);
            }
            if anime::finished(r.game, gobj, parts[0], 2) {
                let v = r.game.grounds[gid.0].u.smash_taunt();
                v.state = 1;
                v.timer = 0x3C;
                for part in parts {
                    anime::start(r.game, r.data, gobj, part, 7, 1, 0.0, 1.0, false);
                }
                for part in parts {
                    anime::set_loop(r.game, gobj, part, 7, false);
                }
                let archive = super::datfiles::main_archive(r.game).archive;
                r.add_canvas(1, None, 9, 0xD, 0, 1, 0, 7);
                r.set_font_table(1, archive, "SIS_GrCorneriaData");
                let t = r.create_text(1, 0, Vec3::new(140.0, 380.0, 0.0), 288.0, 96.0);
                r.game.sis.text_mut(t).text_color = [0xFF, 0xFF, 0xFF, 0xFF];
                let idx = r.game.grounds[gid.0].u.smash_taunt().sis_data_idx;
                r.game.sis.reset_text(r.data, t, idx.into());
                r.game.grounds[gid.0].u.smash_taunt().text = Some(t);
                let sound = r.game.grounds[gid.0].u.smash_taunt().sound_id;
                r.object_sound(Some(gid), 0, sound as u32);
            }
        }
        1 => {
            let v = r.game.grounds[gid.0].u.smash_taunt();
            v.timer = v.timer.wrapping_sub(1);
            if v.timer < 0 && !r.object_sound_playing(Some(gid), 0) {
                r.game.grounds[gid.0].u.smash_taunt().state = 2;
                for part in parts {
                    anime::start(r.game, r.data, gobj, part, 7, 2, 0.0, 1.0, false);
                }
                for part in parts {
                    anime::clear_loop(r.game, gobj, part, 7, false);
                }
                r.close_font(1);
            }
        }
        2 if anime::finished(r.game, gobj, parts[0], 2) => {
            setup::destroy(r, Some(gobj));
        }
        _ => {}
    }
}

/// The match's end on Corneria and Venom (`Ground_801C1158`), `grCorneria_801E2A6C`: a talk
/// playing stops, its window and line with their sound and text.
#[doc(alias = "grCorneria_801E2A6C")]
pub fn match_end(r: &mut Running) {
    match r.game.stage_info.grkind {
        GR_KIND_CORNERIA => {
            if map_gobj(r.game, 12).is_none() {
                return;
            }
            if let Some(w) = map_gobj(r.game, 5) {
                let wg = ground_of(r.game, w);
                r.stop_object_sound(Some(wg), 0);
                setup::destroy(r, Some(w));
            }
            r.close_font(1);
        }
        GR_KIND_VENOM => super::venom::stop_talk(r, false),
        _ => {}
    }
}

/// Fox's or Falco's taunt cut short (`ftFx_AppealS_OnTakeDamage`), `grCorneria_801E2AF4`: the
/// talk stops, its object too.
#[doc(alias = "grCorneria_801E2AF4")]
pub fn taunt_interrupted(r: &mut Running) {
    match r.game.stage_info.grkind {
        GR_KIND_CORNERIA => {
            let Some(t) = map_gobj(r.game, 12) else {
                return;
            };
            setup::destroy(r, Some(t));
            if let Some(w) = map_gobj(r.game, 5) {
                let wg = ground_of(r.game, w);
                r.stop_object_sound(Some(wg), 0);
                setup::destroy(r, Some(w));
            }
            r.close_font(1);
        }
        GR_KIND_VENOM => super::venom::stop_talk(r, true),
        _ => {}
    }
}

/// Starts a talk for the taunt (`startTalk`): on Corneria one of five from `talk` unless one
/// plays, on Venom the stage's own.
fn start_talk(r: &mut Running, talk: i32, venom_talk: i32) -> bool {
    match r.game.stage_info.grkind {
        GR_KIND_CORNERIA => {
            if map_gobj(r.game, 12).is_some() {
                return false;
            }
            let n = r.game.objs.rng.below(5).wrapping_add(talk);
            let w = create(r, 12, TALK_FRAME).expect("grcorneria.c: the talk");
            set_talk(r, w, n);
            true
        }
        GR_KIND_VENOM => super::venom::start_talk(r, venom_talk),
        _ => true,
    }
}

/// Fox's taunt's talk, `grCorneria_801E2B80`: Corneria's talks 8 to 12, Venom's 2.
#[doc(alias = "grCorneria_801E2B80")]
pub fn fox_talk(r: &mut Running) -> bool {
    start_talk(r, 8, 2)
}

/// Falco's taunt's talk, `grCorneria_801E2C34`: Corneria's talks 13 to 17, Venom's 20.
#[doc(alias = "grCorneria_801E2C34")]
pub fn falco_talk(r: &mut Running) -> bool {
    start_talk(r, 13, 20)
}

/// Whether a talk plays, `grCorneria_801E2D14`: Corneria's map 12, Venom's map 8.
#[doc(alias = "grCorneria_801E2D14")]
pub fn talking(game: &Game) -> bool {
    match game.stage_info.grkind {
        GR_KIND_CORNERIA => map_gobj(game, 12).is_some(),
        GR_KIND_VENOM => map_gobj(game, 8).is_some(),
        _ => false,
    }
}

/// How far the Great Fox has sunk (0 or less), `grCorneria_801E2EA0`.
#[doc(alias = "grCorneria_801E2EA0")]
pub fn sink_depth(game: &mut Game) -> f32 {
    match map_gobj(game, 3) {
        Some(gobj) => {
            let gid = ground_of(game, gobj);
            game.grounds[gid.0].u.corneria().xD0
        }
        None => 0.0,
    }
}

/// The stage's lines' hazards, `grCorneria_801E2EE4`: none.
#[doc(alias = "grCorneria_801E2EE4")]
pub fn line_hazard() -> Option<DataRef> {
    None
}

/// Whether a joint takes a fighter's shadow, `grCorneria_801E2EEC`: the fighter is above it, and
/// on the Great Fox's deck also left of the deck's slope.
#[doc(alias = "grCorneria_801E2EEC")]
pub fn check_shadow_render(game: &mut Game, data: &Archives, pos: Vec3, j: Option<JObjId>) -> bool {
    let w = world_position(&mut game.objs, data, j, None);
    if let Some(ship) = map_gobj(game, 3) {
        let gid = ground_of(game, ship);
        if game.grounds[gid.0].u.corneria().x12C == j {
            let a = fp::fmuls(106.0, reg(scale(game)));
            let b = fp::fdivs(fp::fmuls(107.0, reg(scale(game))), a);
            let edge = fp::fmadds(fp::fsubs(reg(pos.y), reg(w.y)), b, reg(w.x));
            if reg(pos.x) > fp::fnmsubs(107.0, reg(scale(game)), edge) {
                return false;
            }
        }
    }
    reg(pos.y) > reg(w.y)
}

/// The camera's floor on Corneria, `grCorneria_801E2FCC`: 5 above the sea, which the Great
/// Fox's sinking lowers.
#[doc(alias = "grCorneria_801E2FCC")]
pub fn camera_floor(game: &Game) -> f64 {
    match map_gobj(game, 3) {
        Some(gobj) => {
            let gid = ground_of(game, gobj);
            let super::GroundVars::Corneria(v) = &game.grounds[gid.0].u else {
                panic!("grcorneria.c: the Great Fox's variables");
            };
            fp::fadds(5.0, fp::fmsubs(-35.0, reg(scale(game)), reg(v.xD0)))
        }
        None => fp::fneg(f64::from(f32::MAX)),
    }
}
