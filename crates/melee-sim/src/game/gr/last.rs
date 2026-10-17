//! Final Destination, `melee/gr/grlast.c`: a static platform under a background that cycles
//! through scenes, a swaying background piece, and fog and light colors that blend between
//! scenes.
//!
//! The main object (map 3) runs the cycle in its `Map_GroundVars`: `phase` counts through 17
//! scenes, each started by [`set_phase`] and left when [`next_phase`] says its animations or
//! color blends are done; `b26` lets the cycle advance, and the 30-second timer `xC8` sets it in
//! phase 1. Map 7 sways in its `Last_GroundVars`.

use super::super::camera;
use super::super::slippi::CodeSet;
use super::super::state::{Game, Running};
use super::super::structs::_GXColor;
use super::anime;
use super::data::{LAST_CALLBACKS, StageCallbacks};
use super::ground::{self, ParamColor, ground_of, jobj_of, map_gobj, nth_joint, set_param_color};
use super::{grlib, material, setup};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;

/// `grNLa_803E8010`: the background pieces the cycle animates, by slot: the map id it creates,
/// the model part its scene animations run on, the part an opening animation runs on (or -1)
/// and that animation.
const PIECES: [[i16; 4]; 6] = [
    [4, 1, 2, 0],
    [5, 1, 2, 4],
    [6, 1, 2, 6],
    [7, 1, 2, 8],
    [8, 1, 2, 10],
    [9, 0, -1, 0],
];

/// `grLast_803B8480`, `grLast_803B848C`, `grLast_803B8498`.
const ONES: crate::vec::Vec3 = crate::vec::Vec3::new(1.0, 1.0, 1.0);

/// The fog color phase 10 blends to (`grNLa_804DBBD4`).
const BLEND_PHASE_10: _GXColor = _GXColor {
    r: 0xAA,
    g: 0xCA,
    b: 0xFF,
    a: 0,
};

/// The fog color phase 13 blends to (`grNLa_804DBBD8`).
const BLEND_PHASE_13: _GXColor = _GXColor {
    r: 0xC8,
    g: 0xC8,
    b: 0xC8,
    a: 0xFF,
};

/// The fog color phases 14 and 16 blend to (`grNLa_804DBBDC`, `grNLa_804DBBE0`).
const BLACK: _GXColor = _GXColor {
    r: 0,
    g: 0,
    b: 0,
    a: 0,
};

/// The swaying piece's limits, as `grLast_804DBB7C` and friends: angular speeds and angles in
/// radians, single precision as compiled.
struct Sway {
    /// `(π/180)/250`: the random part of a new speed.
    speed_range: f64,
    /// `(π/180)/1000`: its least.
    speed_min: f64,
    /// `±(π/180)/20`: the speed's limits.
    max_speed: f64,
    min_speed: f64,
    /// `±(π/180)·15` about x, `±(π/180)·10` about y.
    max_x: f64,
    min_x: f64,
    max_y: f64,
    min_y: f64,
}

fn sway() -> Sway {
    let deg = fp::fdiv(std::f64::consts::PI, 180.0);
    let neg = fp::fneg(deg);
    Sway {
        speed_range: fp::frsp(fp::fdiv(deg, 250.0)),
        speed_min: fp::frsp(fp::fdiv(deg, 1000.0)),
        max_speed: fp::frsp(fp::fdiv(deg, 20.0)),
        min_speed: fp::frsp(fp::fdiv(neg, 20.0)),
        max_x: fp::frsp(fp::fmul(deg, 15.0)),
        min_x: fp::frsp(fp::fmul(neg, 15.0)),
        max_y: fp::frsp(fp::fmul(deg, 10.0)),
        min_y: fp::frsp(fp::fmul(neg, 10.0)),
    }
}

/// A stage object's process or callback, by address: Final Destination's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        // The objects' processes and destroy callbacks that do nothing.
        0x8021_A914 | 0x8021_A918 | 0x8021_A968 | 0x8021_A96C | 0x8021_A9A4 | 0x8021_A9A8
        | 0x8021_AB30 | 0x8021_AB80 | 0x8021_AB84 | 0x8021_ABD4 | 0x8021_ABD8 | 0x8021_AC28
        | 0x8021_AC2C | 0x8021_B23C | 0x8021_B28C | 0x8021_B290 | 0x8021_B2E0 | 0x8021_B2E4 => {
            empty(r, gobj)
        }
        0x8021_A8E0 | 0x8021_A970 => init_piece_anim(r, gobj),
        0x8021_A91C => init_map1(r, gobj),
        0x8021_A9AC => start_cycle(r, gobj),
        0x8021_A9C4 => init_main(r, gobj),
        0x8021_AAB0 => proc_main(r, gobj),
        0x8021_AB34 | 0x8021_AB88 | 0x8021_ABDC | 0x8021_B240 | 0x8021_B294 => init_piece(r, gobj),
        0x8021_AC30 => init_sway(r, gobj),
        0x8021_ADD0 => proc_sway(r, gobj),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing: `grLast_8021A914` and
/// friends.
#[doc(alias = "grLast_8021A914")]
#[doc(alias = "grLast_8021A918")]
#[doc(alias = "grLast_8021A968")]
#[doc(alias = "grLast_8021A96C")]
#[doc(alias = "grLast_8021A9A4")]
#[doc(alias = "grLast_8021A9A8")]
#[doc(alias = "grLast_8021AB30")]
#[doc(alias = "grLast_8021AB80")]
#[doc(alias = "grLast_8021AB84")]
#[doc(alias = "grLast_8021ABD4")]
#[doc(alias = "grLast_8021ABD8")]
#[doc(alias = "grLast_8021AC28")]
#[doc(alias = "grLast_8021AC2C")]
#[doc(alias = "grLast_8021B23C")]
#[doc(alias = "grLast_8021B28C")]
#[doc(alias = "grLast_8021B290")]
#[doc(alias = "grLast_8021B2E0")]
#[doc(alias = "grLast_8021B2E4")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Pieces 0 and 2 start their map animation, `stageGObj0_OnInit` and `stageGObj2_OnInit`
/// (each the inline `Ground_StartMapAnim`).
#[doc(alias = "stageGObj0_OnInit@8021A8E0")]
#[doc(alias = "stageGObj2_OnInit@8021A970")]
pub fn init_piece_anim(r: &mut Running, gobj: GObjId) {
    let map_id = r.game.grounds[ground_of(r.game, gobj).0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
}

/// Map 1 starts its map animation and marks its kind, `grLast_8021A91C`.
#[doc(alias = "grLast_8021A91C")]
pub fn init_map1(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    r.game.grounds[g.0].x11_flags.b012 = 1;
}

/// The background pieces give their materials the stage class and mark their kind
/// (`grLast_8021AB34` and the identical `8021AB88`, `8021ABDC`, `8021B240`, `8021B294`).
#[doc(alias = "grLast_8021AB34")]
#[doc(alias = "grLast_8021AB88")]
#[doc(alias = "grLast_8021ABDC")]
#[doc(alias = "grLast_8021B240")]
#[doc(alias = "grLast_8021B294")]
pub fn init_piece(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    material::model_to_stage_class(r.game, j);
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The start hook that lets the cycle run, `grLast_8021A9AC`.
#[doc(alias = "grLast_8021A9AC")]
pub fn start_cycle(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.map().b0 = false;
}

/// The main object's setup, `grLast_8021A9C4`: binds its collision, starts its animation,
/// creates the first scene's pieces and holds the cycle until the match starts.
#[doc(alias = "grLast_8021A9C4")]
pub fn init_main(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let map_id = r.game.grounds[g.0].map_id;
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, map_id);
    anime::start_map(r.game, r.data, gobj, map_id, 0);
    let v = r.game.grounds[g.0].u.map();
    v.b0 = true;
    v.lv_gobj = [None; 6];
    v.phase = 0;
    v.b26 = false;
    v.b27 = true;
    set_phase(r, gobj, 1, SETUP_PIECES_FRAME);
    let gp = &mut r.game.grounds[g.0];
    gp.x11_flags.b012 = 1;
    gp.u.map().b1 = false;
    setup::add_start_hook(r.game, gobj, super::super::handles::CodeRef(0x8021_A9AC));
}

/// The main object's process, `grLast_8021AAB0`: once the match runs, the scene's timers and
/// color blends, and the next scene when it's due; then the collision follows the model.
///
/// Slippi's determinism fix (`8021AAE4`) keeps the closing scene's random colors off the
/// random seed: 3.19's codes save and restore the seed around [`advance`], 3.16's skip it, so
/// the scene never moves on from its first phase.
#[doc(alias = "grLast_8021AAB0")]
pub fn proc_main(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.map();
    if !v.b1 && !v.b0 {
        let rules = &r.game.rules;
        if !rules.slippi {
            advance(r, gobj);
        } else if rules.codes == CodeSet::V3_19 {
            let seed = r.game.objs.rng;
            advance(r, gobj);
            r.game.objs.rng = seed;
        }
        if r.game.grounds[g.0].u.map().b26 {
            let next = next_phase(r, gobj);
            if next != 0 {
                set_phase(r, gobj, next, CYCLE_PIECES_FRAME);
            }
        }
    }
    ground::update_map_coll(r.game, r.data, gobj);
    ground::quake_list_tick(r.game);
}

/// Times the scene, `grLast_8021B2E8`: the 30-second wait in phase 1 (in the boss stages, the
/// boss's health instead), the swaying piece fading in and out, the closing scene's random
/// fog colors, and the fog blend.
#[doc(alias = "grLast_8021B2E8")]
pub fn advance(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let stkind = r.game.gr.stkind;
    if stkind == 0xB0 || stkind == 0xFB {
        unimplemented!("the boss stages' cycle needs gm_8016ECE8 (the boss's health)");
    }
    let v = r.game.grounds[g.0].u.map();
    if reg(v.xC8) > 1800.0 {
        v.b26 = true;
    }
    v.xC8 = mem(fp::fadds(reg(v.xC8), 1.0));
    let phase = v.phase;
    let lv3 = v.lv_gobj[3];
    let b27 = v.b27;
    match phase {
        9 | 10 => {
            let piece = ground_of(r.game, lv3.expect("piece 3"));
            let l = r.game.grounds[piece.0].u.last();
            if phase == 9 {
                l.xDC = mem(fp::fadds(reg(l.xDC), fp::fdivs(1.0, 180.0)));
                if reg(l.xDC) > 1.0 {
                    l.xDC = 1.0;
                }
            } else {
                l.xDC = mem(fp::fsubs(reg(l.xDC), fp::fdivs(1.0, 58.0)));
                if reg(l.xDC) < 0.0 {
                    l.xDC = 0.0;
                }
                camera::request_quake(r, camera::QUAKE_LOOP, None);
            }
        }
        11 => camera::request_quake(r, camera::QUAKE_LOOP, None),
        15 if b27 => {
            let current =
                ground::fog_color(r.game).expect("Ground_801C2090 leaves its color unset");
            let rng = &mut r.game.objs.rng;
            let c = loop {
                let c = _GXColor {
                    r: rng.below(2).wrapping_mul(30) as u8,
                    g: rng.below(2).wrapping_mul(30) as u8,
                    b: rng.below(2).wrapping_mul(30) as u8,
                    a: 0,
                };
                // The original compares green twice and never blue.
                let gray = c.r == 30 && c.g == 30 && c.b == 30;
                let same = c.r == current.r && c.g == current.g && c.g == current.g;
                if !(gray || same) {
                    break c;
                }
            };
            let frames = rng.below(60).wrapping_add(60);
            blend_to(r.game, gobj, c, mem(fp::frsp(f64::from(frames))));
        }
        _ => {}
    }
    blend_step(r.game, gobj);
}

/// Whether piece `i`'s scene animation has finished.
fn piece_done(r: &mut Running, gobj: GObjId, i: usize) -> bool {
    let g = ground_of(r.game, gobj);
    let piece = r.game.grounds[g.0].u.map().lv_gobj[i];
    anime::finished(r.game, piece.expect("a piece"), PIECES[i][1].into(), 7)
}

/// Whether every piece's scene animation has finished (the inline `animate_all`).
fn all_done(r: &mut Running, gobj: GObjId) -> bool {
    (0..5).all(|i| piece_done(r, gobj, i))
}

/// The phase that follows the current one when it's done, else 0, `grLast_8021B5C4`.
#[doc(alias = "grLast_8021B5C4")]
pub fn next_phase(r: &mut Running, gobj: GObjId) -> i32 {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.map();
    let (phase, b27, lv) = (v.phase, v.b27, v.lv_gobj);
    let overlay_done = |r: &Running, i: usize| grlib::overlay_done(r.game, lv[i].expect("a piece"));
    match phase {
        1 => 2,
        3 => 4,
        5 => 6,
        7 => 8,
        12 => 13,
        15 => 16,
        2 | 4 | 6 | 8 | 11 if all_done(r, gobj) => i32::from(phase) + 1,
        9 => {
            let piece = lv[3].expect("piece 3");
            let a =
                anime::first_anim(r.game, piece, PIECES[3][2].into(), 7).expect("grlast.c: aobj");
            if reg(r.game.objs.aobjs[a].curr_frame) == 100.0 {
                10
            } else {
                0
            }
        }
        10 if anime::rewound(r.game, lv[3].expect("piece 3"), PIECES[3][2].into(), 7) => 11,
        13 if b27 && overlay_done(r, 4) => 14,
        14 if overlay_done(r, 5) => 15,
        16 if b27 && overlay_done(r, 5) => 17,
        17 if overlay_done(r, 0) => 1,
        _ => 0,
    }
}

/// Starts scene `n` on piece `i` (the inline `do_anime`): a part's animation alone with its
/// loop cleared, or, for a piece without an opening animation, the part and its descendants.
fn piece_anim(r: &mut Running, piece: GObjId, i: usize, n: i32) {
    let part = PIECES[i][1].into();
    if PIECES[i][2] != -1 {
        anime::start(r.game, r.data, piece, part, 7, n, 0.0, 1.0, false);
        anime::clear_loop(r.game, piece, part, 7, false);
    } else {
        anime::start(r.game, r.data, piece, part, 7, n, 0.0, 1.0, true);
    }
}

/// Starts the color script at `offset` in the stage's `yakumono_param` on a piece and runs its
/// first tick (`grMaterial_801C9604`, then `grMaterial_801C9698` again).
fn piece_script(r: &mut Running, piece: GObjId, offset: u32) {
    let yaku = r.game.gr.last_yakumono.expect("grLast yakumono_param");
    let script = r.data.ptr(yaku.at(offset));
    material::start_script(r.game, r.data, piece, script, 0);
    material::run_ground_overlay(r.game, r.data, piece);
}

/// Starts scene `n` on the first five pieces.
fn pieces_anim(r: &mut Running, gobj: GObjId, n: i32) {
    for i in 0..5 {
        let g = ground_of(r.game, gobj);
        let piece = r.game.grounds[g.0].u.map().lv_gobj[i];
        piece_anim(r, piece.expect("a piece"), i, n);
    }
}

/// Creates piece `i` under the main object and starts its opening and first scene.
fn create_piece(r: &mut Running, gobj: GObjId, i: usize, frame: u32) -> GObjId {
    let piece = create(r, PIECES[i][0].into(), frame).expect("grlast.c: lv_gobj");
    r.game.world.gx_relink_before(piece, gobj);
    if PIECES[i][2] != -1 {
        let (part, anim) = (PIECES[i][2].into(), PIECES[i][3].into());
        anime::start(r.game, r.data, piece, part, 7, anim, 0.0, 1.0, true);
    }
    piece
}

/// Enters phase `phase`, `grLast_8021B920`: each scene's animations, pieces and colors.
#[doc(alias = "grLast_8021B920")]
pub fn set_phase(r: &mut Running, gobj: GObjId, phase: i32, frame: u32) {
    let g = ground_of(r.game, gobj);
    let v = r.game.grounds[g.0].u.map();
    v.xC8 = 0.0;
    v.phase = phase as u16;
    let b26 = match phase {
        1 => {
            for i in 0..5 {
                if r.game.grounds[g.0].u.map().lv_gobj[i].is_none() {
                    let piece = create_piece(r, gobj, i, frame);
                    r.game.grounds[g.0].u.map().lv_gobj[i] = Some(piece);
                    piece_anim(r, piece, i, 0);
                }
            }
            let map4 = map_gobj(r.game, 4).expect("map 4");
            anime::start(r.game, r.data, map4, 0x10, 7, 0, 0.0, 1.0, false);
            r.game.grounds[g.0].u.map().b26 = false;
            dark_colors(r.game);
            return;
        }
        2 => {
            pieces_anim(r, gobj, 1);
            let map4 = map_gobj(r.game, 4).expect("map 4");
            anime::set_rate(r.game, map4, 0x10, 7, 0.0, false);
            let j = jobj_of(r.game, map4);
            grlib::kill_joint_generators(r.game, r.data, j);
            true
        }
        3 | 4 | 6 | 8 | 11 => {
            pieces_anim(r, gobj, scene_of(phase));
            true
        }
        5 | 7 | 12 => {
            pieces_anim(r, gobj, scene_of(phase));
            false
        }
        9 => {
            pieces_anim(r, gobj, 8);
            // The swaying piece's generator, at its part 5.
            let sway = map_gobj(r.game, 7).expect("map 7");
            let part = nth_joint(r.game, sway, 5);
            let at = crate::lb::joint::world_position(&mut r.game.objs, r.data, part, None);
            let gen_ = grlib::generator_at(r.game, r.data, 0x7531, 0x1E, at);
            let sg = ground_of(r.game, sway);
            r.game.grounds[sg.0].u.last().xE0 = gen_;
            false
        }
        10 => {
            blend_to(r.game, gobj, BLEND_PHASE_10, 120.0);
            let sway = map_gobj(r.game, 7).expect("map 7");
            anime::set_rate(r.game, sway, 5, 7, 0.0, false);
            let sg = ground_of(r.game, sway);
            let gen_ = r.game.grounds[sg.0].u.last().xE0;
            grlib::kill_generator(r.game, r.data, gen_.expect("grLib_801C9834: the generator"));
            r.game.grounds[sg.0].u.last().xE0 = None;
            r.game.grounds[g.0].u.map().b26 = true;
            bright_colors(r.game);
            return;
        }
        13 => {
            blend_to(r.game, gobj, BLEND_PHASE_13, 120.0);
            for i in 0..5 {
                let piece = r.game.grounds[g.0].u.map().lv_gobj[i].expect("a piece");
                piece_script(r, piece, 0);
            }
            true
        }
        14 => {
            blend_to(r.game, gobj, BLACK, 60.0);
            for i in 0..5 {
                let piece = r.game.grounds[g.0].u.map().lv_gobj[i].take();
                setup::destroy(r, piece);
            }
            let piece = create_piece(r, gobj, 5, frame);
            r.game.grounds[g.0].u.map().lv_gobj[5] = Some(piece);
            piece_anim(r, piece, 5, 0);
            piece_script(r, piece, 4);
            ground::set_fog_enabled(r.game, false);
            r.game.grounds[g.0].u.map().b26 = true;
            dark_colors(r.game);
            return;
        }
        15 => false,
        16 => {
            blend_to(r.game, gobj, BLACK, 60.0);
            let piece = r.game.grounds[g.0].u.map().lv_gobj[5].expect("piece 5");
            piece_script(r, piece, 8);
            true
        }
        17 => {
            for i in 0..5 {
                let piece = create_piece(r, gobj, i, frame);
                r.game.grounds[g.0].u.map().lv_gobj[i] = Some(piece);
                piece_anim(r, piece, i, 0);
                piece_script(r, piece, 0xC);
            }
            let piece = r.game.grounds[g.0].u.map().lv_gobj[5].take();
            setup::destroy(r, piece);
            ground::set_fog_enabled(r.game, true);
            true
        }
        _ => return,
    };
    r.game.grounds[g.0].u.map().b26 = b26;
}

/// The scene a phase's animations run (`do_anime`'s `n`).
fn scene_of(phase: i32) -> i32 {
    match phase {
        3 => 2,
        4 => 3,
        5 => 4,
        6 => 5,
        7 => 6,
        8 => 7,
        11 => 9,
        12 => 10,
        _ => unreachable!(),
    }
}

/// Starts a fog color blend toward `c` over `frames`, `grLast_8021C40C`.
#[doc(alias = "grLast_8021C40C")]
pub fn blend_to(game: &mut Game, gobj: GObjId, c: _GXColor, frames: f32) {
    let from = ground::fog_color(game).expect("Ground_801C2090 leaves its color unset");
    let f = |x: u8| mem(fp::frsp(f64::from(x)));
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.map();
    v.xE4 = f(c.r);
    v.xE8 = f(c.g);
    v.xEC = f(c.b);
    v.xF4 = f(from.r);
    v.xF8 = f(from.g);
    v.xFC = f(from.b);
    v.x104 = frames;
    v.b27 = false;
}

/// Advances the fog color blend, `grLast_8021C500`: the fog and the camera's clear color.
#[doc(alias = "grLast_8021C500")]
pub fn blend_step(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let v = game.grounds[g.0].u.map();
    if v.b27 {
        return;
    }
    v.x104 = mem(fp::fsubs(reg(v.x104), 1.0));
    let byte = |x: f32| fp::fctiwz(reg(x)) as u8;
    let c = if reg(v.x104) > 0.0 {
        let step = |cur: f32, to: f32, n: f32| {
            mem(fp::fadds(
                reg(cur),
                fp::fdivs(fp::fsubs(reg(to), reg(cur)), reg(n)),
            ))
        };
        v.xF4 = step(v.xF4, v.xE4, v.x104);
        v.xF8 = step(v.xF8, v.xE8, v.x104);
        v.xFC = step(v.xFC, v.xEC, v.x104);
        (byte(v.xF4), byte(v.xF8), byte(v.xFC))
    } else {
        v.b27 = true;
        (byte(v.xE4), byte(v.xE8), byte(v.xEC))
    };
    // The color's alpha is whatever the stack held: the low byte of a stack or heap address
    // another function saved there, or 0 (`docs/engine.md`). Only the fog takes it.
    ground::set_fog_rgb(game, c.0, c.1, c.2);
    camera::set_background_color(&mut game.camera.cam, c.0, c.1, c.2);
}

/// The scene colors for the dark scenes, `grLast_8021C640`.
#[doc(alias = "grLast_8021C640")]
pub fn dark_colors(game: &mut Game) {
    let c = _GXColor {
        r: 0x0C,
        g: 0x06,
        b: 0x28,
        a: 0xFF,
    };
    for which in [
        ParamColor::B8,
        ParamColor::BC,
        ParamColor::C0,
        ParamColor::C4,
        ParamColor::C8,
        ParamColor::CC,
        ParamColor::D0,
        ParamColor::D4,
        ParamColor::D8,
    ] {
        set_param_color(game, which, c.clone());
    }
}

/// The scene colors for the bright scene, `grLast_8021C6AC`.
#[doc(alias = "grLast_8021C6AC")]
pub fn bright_colors(game: &mut Game) {
    let rgb = |r, g, b| _GXColor { r, g, b, a: 0xFF };
    let (c1c, c18, c14, c10, c0c, c08) = (
        rgb(0x8C, 0xB4, 0xBE),
        rgb(0x96, 0xB4, 0xBE),
        rgb(0x6E, 0x78, 0x8C),
        rgb(0, 0, 0),
        rgb(0x73, 0xA0, 0x91),
        rgb(0x87, 0xA5, 0xA0),
    );
    for (which, c) in [
        (ParamColor::B8, c1c.clone()),
        (ParamColor::BC, c18),
        (ParamColor::C0, c1c),
        (ParamColor::C4, c14.clone()),
        (ParamColor::C8, c10),
        (ParamColor::CC, c14),
        (ParamColor::D0, c0c.clone()),
        (ParamColor::D4, c08),
        (ParamColor::D8, c0c),
    ] {
        set_param_color(game, which, c);
    }
}

/// The swaying piece's setup, `grLast_8021AC30`: random speeds about x and y, and the part the
/// generator follows at unit scale.
#[doc(alias = "grLast_8021AC30")]
pub fn init_sway(r: &mut Running, gobj: GObjId) {
    let s = sway();
    let j = jobj_of(r.game, gobj);
    material::model_to_stage_class(r.game, j);
    let g = ground_of(r.game, gobj);
    let rng = &mut r.game.objs.rng;
    let mut speed = || {
        let x = mem(fp::fmadds(reg(rng.draw_f32()), s.speed_range, s.speed_min));
        let sign = if rng.below(2) != 0 { 1.0 } else { -1.0 };
        mem(fp::fmuls(reg(x), sign))
    };
    let d4 = speed();
    let d8 = speed();
    let l = r.game.grounds[g.0].u.last();
    l.xC4 = 0.0;
    l.xC8 = 0.0;
    l.xCC = 0.0;
    l.xD0 = 0.0;
    l.xD4 = d4;
    l.xD8 = d8;
    l.xDC = 0.0;
    l.xE0 = None;
    let part = nth_joint(r.game, gobj, 5).expect("HSD_JObjSetScale: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, part, |o| o.scale = ONES);
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The swaying piece's process, `grLast_8021ADD0`: the sway speeds drift and damp, bounce off
/// their limits with new random speeds, and turn the model, scaled by the fade `xDC`; with a
/// generator, it follows part 5.
#[doc(alias = "grLast_8021ADD0")]
pub fn proc_sway(r: &mut Running, gobj: GObjId) {
    let s = sway();
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("the swaying piece's model");
    let rng = &mut r.game.objs.rng;
    let l = r.game.grounds[g.0].u.last();
    let damp = |x: f32, dx: f32| {
        let x = mem(fp::fadds(reg(x), reg(dx)));
        let x = mem(fp::fmuls(reg(x), fp::lfs(0.99f32.to_bits())));
        if reg(x) > s.max_speed {
            mem(s.max_speed)
        } else if reg(x) < s.min_speed {
            mem(s.min_speed)
        } else {
            x
        }
    };
    let abs = |x: f32| {
        if reg(x) < 0.0 {
            mem(fp::fneg(reg(x)))
        } else {
            x
        }
    };
    l.xCC = damp(l.xCC, l.xD4);
    l.xD0 = damp(l.xD0, l.xD8);
    l.xC4 = mem(fp::fadds(reg(l.xC4), reg(l.xCC)));
    if reg(l.xC4) > s.max_x {
        l.xC4 = mem(s.max_x);
        l.xCC = mem(fp::fneg(reg(abs(l.xCC))));
        let rf = rng.draw_f32();
        l.xD4 = mem(fp::fmsubs(s.speed_range, fp::fneg(reg(rf)), s.speed_min));
    } else if reg(l.xC4) < s.min_x {
        l.xC4 = mem(s.min_x);
        l.xCC = abs(l.xCC);
        l.xD4 = mem(fp::fmadds(s.speed_range, reg(rng.draw_f32()), s.speed_min));
    }
    let (rx, fade) = (l.xC4, l.xDC);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| {
        q.x = mem(fp::fmuls(reg(rx), reg(fade)))
    });
    let l = r.game.grounds[g.0].u.last();
    l.xC8 = mem(fp::fadds(reg(l.xC8), reg(l.xD0)));
    let rng = &mut r.game.objs.rng;
    if reg(l.xC8) > s.max_y {
        l.xC8 = mem(s.max_y);
        l.xD0 = mem(fp::fneg(reg(abs(l.xD0))));
        let rf = rng.draw_f32();
        l.xD8 = mem(fp::fmsubs(s.speed_range, fp::fneg(reg(rf)), s.speed_min));
    } else if reg(l.xC8) < s.min_y {
        l.xC8 = mem(s.min_y);
        l.xD0 = abs(l.xD0);
        l.xD8 = mem(fp::fmadds(s.speed_range, reg(rng.draw_f32()), s.speed_min));
    }
    let (ry, fade, generator) = (l.xC8, l.xDC, l.xE0);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| {
        q.y = mem(fp::fmuls(reg(ry), reg(fade)))
    });
    let Some(srt) = generator.and_then(|g| r.game.objs.generators[g].appsrt) else {
        return;
    };
    // The generator takes part 5's position and its frame's turn.
    let part = nth_joint(r.game, gobj, 5);
    let world = |r: &mut Running, v: Option<Vec3>| {
        crate::lb::joint::world_position(&mut r.game.objs, r.data, part, v)
    };
    let at = world(r, None);
    let mut up = world(r, Some(Vec3::new(0.0, 1.0, 0.0))) - at;
    up.normalize();
    let mut fwd = world(r, Some(Vec3::new(0.0, 0.0, 1.0))) - at;
    fwd.normalize();
    let rot = Vec3::euler_angles_from_partial_basis(fwd, up);
    let s = &mut r.game.objs.app_srts[srt];
    s.translate = at;
    s.rot.x = copy(rot.x);
    s.rot.y = copy(rot.y);
    s.rot.z = copy(rot.z);
    s.status = 0;
}

/// Final Destination's setup, `grLast_OnInit`: its material scripts, its first four objects
/// (the main object creates the background's pieces), camera limits and blast zones from its
/// points, and the camera's depth range.
#[doc(alias = "grLast_OnInit")]
pub fn on_init(r: &mut Running) {
    r.game.gr.last_yakumono = r.game.stage_info.yakumono_param;
    r.game.stage_info.unk8C.b4 = true;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in 0..4 {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    camera::set_near(&mut r.game.camera.cam, 1.0);
    camera::set_far(&mut r.game.camera.cam, 30000.0);
}

/// Final Destination's demo setup, `grLast_OnDemoInit`: for demo 26, the background's
/// pieces (maps 4 to 9) destroyed and the scene's flag `xC4_b1` set.
#[doc(alias = "grLast_OnDemoInit")]
pub fn on_demo_init(r: &mut Running, arg: i32) {
    if arg != 26 {
        return;
    }
    for map_id in 4..=9 {
        if let Some(g) = super::ground::map_gobj(r.game, map_id) {
            setup::destroy(r, Some(g));
        }
    }
    assert!(
        super::ground::map_gobj(r.game, 1).is_some(),
        "grlast.c: gobj"
    );
    let g = super::ground::map_gobj(r.game, 3).expect("grlast.c: gobj");
    let gp = super::ground::ground_of(r.game, g);
    r.game.grounds[gp.0].u.map().b1 = true;
}

/// `grLast_OnLoad` does nothing; `grLast_8021A7EC` (`callback4`) answers false, and nothing
/// calls it.
#[doc(alias = "grLast_OnLoad")]
pub fn on_load(_r: &mut Running) {}

/// Final Destination's start, `grLast_OnStart`: the enemy spawner, without enemies.
#[doc(alias = "grLast_OnStart")]
pub fn on_start(r: &mut Running) {
    super::zako::start(r.game, None);
}

/// The stage's objects' setup functions and processes, by map id (`grLast_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &LAST_CALLBACKS[map_id as usize]
}

/// Where the background pieces' calls put their scale joints' descriptors on the stack
/// (`setup::SETUP_FRAME`): the cycle's first phase starts from the setup's frame, the later
/// ones from its process's.
const SETUP_PIECES_FRAME: u32 = 0x804E_E948;
pub const CYCLE_PIECES_FRAME: u32 = 0x804E_E9A8;

/// Creates one of the stage's objects and sets it up, `grLast_8021A7F4`: its render callback,
/// destroy callback, setup and process.
#[doc(alias = "grLast_8021A7F4")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}
