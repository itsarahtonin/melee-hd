//! Kongo Jungle (the N64 one), `melee/gr/groldkongo.c`: the stage (map 3), the barrel that
//! swings and turns below it (map 1), the birds that fly across the background now and then
//! (map 2), and the background (map 0).
//!
//! The barrel turns in spurts to one of eight directions and swings along its animation at a
//! speed that rises and falls. A fighter that falls near it is caught (a stage device the
//! fighter code polls, `ftCo_800C0764`) and shot out a while later toward where it points.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::data::{OLDKONGO_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, update_map_coll};
use super::kongo::{
    Barrel, BarrelView, catch_fighter, drop_fighter, init_barrel, let_go, turn_and_swing,
};
use super::{anime, grlib, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `grOldKongo_80210454`, the barrel's catch test, which the fighter code calls for each
/// fighter every tick once the stage registers it.
pub const BARREL_CATCH: u32 = 0x8021_0454;

/// The catch's device type (`ftCo_800C0764`'s second argument): a barrel.
const DEVICE_BARREL: u32 = 2;

/// The fields of `grOldKongo_YakumonoParam` the code reads, by offset.
#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn h(self, data: &Archives, off: u32) -> i32 {
        i32::from(data.u16(self.0.at(off)) as i16)
    }
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
}

const BIRD_WAIT_A: u32 = 0x0;
const BIRD_WAIT_B: u32 = 0x2;
const BIRD_OFFSET_Y: u32 = 0x4;
const SHOOT_A: u32 = 0x8;
const SHOOT_B: u32 = 0xC;
const BARREL_IN: u32 = 0x10;
const WAIT_A: u32 = 0x14;
const WAIT_B: u32 = 0x18;
const ROT_ACCEL: u32 = 0x1C;
const ROT_MAX: u32 = 0x20;
const ROLL_A: u32 = 0x24;
const ROLL_B: u32 = 0x28;
/// The eight directions' weights, `s16`s from down-left round to down.
const RATES: u32 = 0x2C;
const INTERVAL_A: u32 = 0x3C;
const INTERVAL_B: u32 = 0x40;
const MOVE_ACCEL: u32 = 0x44;
const MOVE_MAX: u32 = 0x48;
const STOP_A: u32 = 0x4C;
const STOP_B: u32 = 0x50;
/// The shot's damage, angle, growth, set and base knockback and element, six words.
const SHOT_HIT: u32 = 0x54;
const COLOR_SCRIPT: u32 = 0x6C;

fn param(game: &Game) -> Param {
    Param(
        game.gr
            .oldkongo_yakumono
            .expect("groldkongo.c: yakumono_param"),
    )
}

/// A stage object's process or callback, by address: Kongo Jungle N64's.
#[melee_callbacks::callbacks(Proc)]
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_F64C | 0x8020_F650 | 0x8020_F6E0 | 0x8021_0058 | 0x8021_0450 => empty(r, gobj),
        0x8020_F618 => init_back(r, gobj),
        0x8020_F654 => init_stage(r, gobj),
        0x8020_F6BC => proc_stage(r, gobj),
        0x8020_F6E4 => init_barrel(r, gobj, &BARREL),
        0x8020_F888 => proc_barrel(r, gobj),
        0x8021_005C => init_birds(r, gobj),
        0x8021_00FC => proc_birds(r, gobj),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing.
#[doc(alias = "stageGObj0_GObjProc@8020F64C")]
#[doc(alias = "stageGObj0_Callback3@8020F650")]
#[doc(alias = "stageGObj3_Callback3@8020F6E0")]
#[doc(alias = "stageGObj1_Callback3@80210058")]
#[doc(alias = "stageGObj2_Callback3@80210450")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `setupStageCallbacks`.
#[doc(alias = "setupStageCallbacks@8020F52C")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`stage_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &OLDKONGO_CALLBACKS[map_id as usize]
}

/// Kongo Jungle N64's setup, `grOldKongo_8020F46C`: its parameters, maps 0, 3, 1 and 2, camera
/// limits and blast zones.
#[doc(alias = "grOldKongo_8020F46C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.oldkongo_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in [0, 3, 1, 2] {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grOldKongo_8020F468` does nothing (only a demo runs it).
#[doc(alias = "grOldKongo_8020F468")]
pub fn on_demo_init(_r: &mut Running) {}

/// `grOldKongo_8020F4E4` does nothing.
#[doc(alias = "grOldKongo_8020F4E4")]
pub fn on_load(_r: &mut Running) {}

/// Kongo Jungle N64's start, `grOldKongo_8020F4E8`: the barrel registered as a device that
/// catches fighters, and the enemy spawner, without enemies.
#[doc(alias = "grOldKongo_8020F4E8")]
pub fn on_start(r: &mut Running) {
    let barrel = map_gobj(r.game, 1).expect("groldkongo.c: the barrel");
    register_catch(r.game, barrel, DEVICE_BARREL, CodeRef(BARREL_CATCH));
    zako::start(r.game, r.data, None);
}

/// Registers a stage object as a device that catches fighters, `ftCo_800C0764`: the first of
/// its two slots that is free.
#[doc(alias = "ftCo_800C0764")]
pub fn register_catch(game: &mut Game, gobj: GObjId, kind: u32, cb: CodeRef) {
    let d = &mut game.gr.devices;
    let Some(i) = d.bury.iter().position(Option::is_none) else {
        panic!("fighter chk device catch func num over!");
    };
    d.bury[i] = Some(gobj);
    d.bury_type[i] = kind;
    d.bury_cb[i] = Some(cb);
    d.bury_count = d.bury_count.wrapping_add(1);
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The background's setup (map 0), `stageGObj0_OnInit`: its animation.
#[doc(alias = "stageGObj0_OnInit@8020F618")]
pub fn init_back(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The stage's setup (map 3), `stageGObj3_OnInit`: its collision and animation, and it
/// receives shadows.
#[doc(alias = "stageGObj3_OnInit@8020F654")]
pub fn init_stage(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    r.game.grounds[g.0].x10_flags.b5 = true;
}

/// The stage's process (map 3), `stageGObj3_GObjProc`: the collision follows the model, and the
/// wind sources age.
#[doc(alias = "stageGObj3_GObjProc@8020F6BC")]
pub fn proc_stage(r: &mut Running, gobj: GObjId) {
    update_map_coll(r.game, r.data, gobj);
    let level = r.game.gr.wind_level;
    r.game.gr.wind_level = crate::lb::wind::tick(&mut r.game.gr.winds, level);
}

/// The barrel's process (map 1), `stageGObj1_GObjProc`: its turn and swing (Kongo Jungle's,
/// `kongo::turn_and_swing`), then a fighter caught: held while its countdown runs (1, or let go when gone), the shot's animation (2),
/// and the shot ten ticks later (3), then clear (4).
#[doc(alias = "stageGObj1_GObjProc@8020F888")]
pub fn proc_barrel(r: &mut Running, gobj: GObjId) {
    let pos = turn_and_swing(r, gobj, &BARREL);
    let g = ground_of(r.game, gobj);
    let p = param(r.game);
    let mut state = r.game.grounds[g.0].u.oldkongo().xC6;
    if state == 1 {
        // With no fighter the state clears, and falls into the shot's start all the same
        // (no `break`); nothing leaves a held state without its fighter.
        let v = r.game.grounds[g.0].u.oldkongo();
        if v.keep.is_none() {
            v.xC6 = 0;
        } else {
            let left = v.xCA;
            v.xCA = left.wrapping_sub(1);
            if left >= 0 {
                return;
            }
            v.xC6 = 2;
        }
        state = 2;
    }
    if state == 2 {
        anime::start(r.game, r.data, gobj, 2, 7, 2, 0.0, 1.0, true);
        material::reset_overlay(r.game, gobj);
        let v = r.game.grounds[g.0].u.oldkongo();
        v.hit_timer = 0;
        v.xC6 = 3;
        state = 3;
    }
    match state {
        3 => {
            let v = r.game.grounds[g.0].u.oldkongo();
            let t = v.hit_timer;
            v.hit_timer = t.wrapping_add(1);
            if t > 10 {
                let keep = v.keep.expect("groldkongo.c: the caught fighter");
                // The shot: `{1, 1, 361, 0, 0, 180}` with the parameters' damage, angle,
                // growth, set and base knockback and element, as kongo's (no sound).
                let words: [u32; 6] =
                    std::array::from_fn(|i| r.data.u32(p.0.at(SHOT_HIT + 4 * i as u32)));
                let mut a = fp::frsp(fp::fadd(
                    std::f64::consts::FRAC_PI_2,
                    reg(r.game.grounds[g.0].u.oldkongo().xDC),
                ));
                if a < 0.0 {
                    a = fp::frsp(fp::fadd(a, std::f64::consts::TAU));
                } else if a > std::f64::consts::TAU {
                    a = fp::frsp(fp::fsub(a, std::f64::consts::TAU));
                }
                let a = mem(fp::fmuls(reg(57.295_78), a));
                if r.game.world.gobj(keep).p_link == 8 {
                    r.shoot_from_stage_barrel(keep, pos, &super::kongo::shot_hit(words), a);
                }
                r.game.grounds[g.0].u.oldkongo().xC6 = 4;
                r.object_sound(Some(g), 0, 0x12A);
            }
        }
        4 => r.game.grounds[g.0].u.oldkongo().xC6 = 0,
        _ => {}
    }
}

fn barrel_vars(game: &mut Game, g: GroundId) -> BarrelView<'_> {
    let v = game.grounds[g.0].u.oldkongo();
    BarrelView {
        turn: &mut v.xC4,
        hold: &mut v.xC6,
        swing: &mut v.xC8,
        held_for: &mut v.xCA,
        turn_timer: &mut v.xCC,
        swing_timer: &mut v.xCE,
        keep: &mut v.keep,
        heading: &mut v.xD8,
        angle: &mut v.xDC,
        step: &mut v.xE0,
        speed: &mut v.xE4,
        one: &mut v.xE8,
        rate: &mut v.xEC,
    }
}

fn yakumono(game: &Game) -> DataRef {
    param(game).0
}

/// Kongo Jungle N64's barrel (Kongo Jungle's, `kongo::Barrel`): its swing's stop and interval
/// are Kongo Jungle's run and pause, its shot's bounds its hold's.
pub const BARREL: Barrel = Barrel {
    vars: barrel_vars,
    param: yakumono,
    hold_a: SHOOT_A,
    hold_b: SHOOT_B,
    reach: BARREL_IN,
    wait_a: WAIT_A,
    wait_b: WAIT_B,
    rot_accel: ROT_ACCEL,
    rot_max: ROT_MAX,
    roll_a: ROLL_A,
    roll_b: ROLL_B,
    rates: RATES,
    pause_a: INTERVAL_A,
    pause_b: INTERVAL_B,
    move_accel: MOVE_ACCEL,
    move_max: MOVE_MAX,
    run_a: STOP_A,
    run_b: STOP_B,
    color_script: COLOR_SCRIPT,
    catch_anim: true,
};

/// The barrel's catch test for a fighter, as the fighter code calls it
/// (`kongo::catch_fighter`).
pub fn barrel_catch(r: &mut Running, gobj: GObjId, fighter: GObjId) -> bool {
    catch_fighter(r, gobj, fighter, &BARREL)
}

/// The barrel lets its fighter go early, as the fighter code asks it (`kongo::let_go`).
pub fn barrel_release(game: &mut Game, gobj: GObjId) {
    let_go(game, gobj, &BARREL)
}

/// The barrel drops its fighter, as the fighter code asks it (`kongo::drop_fighter`).
pub fn barrel_drop(game: &mut Game, gobj: GObjId) {
    drop_fighter(game, gobj, &BARREL)
}

/// The birds' setup (map 2), `stageGObj2_OnInit`: hidden, a random wait.
#[doc(alias = "stageGObj2_OnInit@8021005C")]
pub fn init_birds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let wait = bird_wait(r);
    r.game.grounds[g.0].u.oldkongo_birds().xC4 = wait;
}

/// A wait between the parameters' two bounds.
fn bird_wait(r: &mut Running) -> i32 {
    let p = param(r.game);
    let (b, a) = (p.h(r.data, BIRD_WAIT_B), p.h(r.data, BIRD_WAIT_A));
    grlib::rand_range(&mut r.game.objs.rng, b, a)
}

/// The birds' process (map 2), `stageGObj2_GObjProc`. The wait counts down; when it ends their
/// animation starts at a random height behind the stage, at the right edge of the view or
/// further (shown flying one way) or at the left (the other way). Once the animation ends they
/// hide and a new wait starts.
#[doc(alias = "stageGObj2_GObjProc@802100FC")]
pub fn proc_birds(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    let timer = r.game.grounds[g.0].u.oldkongo_birds().xC4;
    if timer > 0 {
        let left = timer.wrapping_sub(1);
        r.game.grounds[g.0].u.oldkongo_birds().xC4 = left;
        if left != 0 {
            return;
        }
        let m = map_id(r.game, gobj);
        anime::start_map(r.game, r.data, gobj, m, 0);
        let p = param(r.game);
        let f = reg(r.game.objs.rng.draw_f32());
        let y = mem(fp::fmadds(
            reg(p.f(r.data, BIRD_OFFSET_Y)),
            fp::fmsubs(2.0, f, 1.0),
            70.0,
        ));
        let root = j.expect("groldkongo.c: the birds' model");
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.y = y);
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.z = -200.0);
        let ([left, _, right], _) = super::super::camera::ground_span(r.game, r.data);
        let from_right = r.game.objs.rng.below(2) != 0;
        let x = birds_x(from_right, left, right);
        let part = if from_right { 3 } else { 1 };
        // Under Slippi's rules, where they'd start through the views playback's run can leave
        // the camera's CObj (`Game::playback_spans`, design call 9).
        if r.game.rules.notes_playback()
            && r.game.playback_spans(r.data).is_none_or(|spans| {
                (spans.into_iter().flatten()).any(|[left, _, right]| {
                    birds_x(from_right, left, right).to_bits() != x.to_bits()
                })
            })
        {
            let tick = r.game.scene.unk_8;
            r.game.display_phase.note(tick);
        }
        jobj::set_srt(&mut r.game.objs.jobjs, root, |o| o.translate.x = x);
        let shown = nth_joint(r.game, gobj, part);
        jobj::clear_flags_all(&mut r.game.objs.jobjs, shown, jobj::HIDDEN);
    } else if anime::finished(r.game, gobj, 0, 7) {
        jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        let wait = bird_wait(r);
        r.game.grounds[g.0].u.oldkongo_birds().xC4 = wait;
    }
}

/// Where the birds start through a view whose ground span's edges are `left` and `right`:
/// `from_right`, at the right edge where it lies past 200, else at 200; otherwise at the
/// left edge where it lies past -200, else at -200.
fn birds_x(from_right: bool, left: f32, right: f32) -> f32 {
    if from_right {
        if 200.0 < right { right } else { 200.0 }
    } else if -200.0 > left {
        left
    } else {
        -200.0
    }
}

/// The stage's shadow check, `grOldKongo_80210788`: a fighter casts on a joint below its feet.
#[doc(alias = "grOldKongo_80210788")]
pub fn check_shadow_render(
    game: &mut Game,
    data: &Archives,
    pos: Vec3,
    j: Option<crate::hsd::jobj::JObjId>,
) -> bool {
    let b = world_position(&mut game.objs, data, j, None);
    pos.y > b.y
}
