//! Brinstar's escape, Adventure's stage after Brinstar (`melee/gr/grzebesroute.c`, stage kind
//! 33, `/GrNZr.dat`): a shaft the first fighter climbs (map 1, with the stage's lights and fog)
//! between two animated models (maps 0 and 2). The camera follows the first fighter up and down
//! the shaft's middle, never below -50; once the parameters' timer has run out the screen
//! shakes every tick; and the first fighter landing on the collision joint 1's floors (the way
//! out) ends the match (`stage_info.flags` 0x10, which `Ground_801C1D84` reports as a bonus
//! stage's end). The stage's setup moves and colors three of the stage's lights.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Game, Running};
use super::data::{StageCallbacks, ZEBESROUTE_CALLBACKS};
use super::ground::{ground_of, p1_fighter, scale, set_cam_offset};
use super::{anime, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::lobj;
use crate::vec::Vec3;

/// `fn_8020B4D8`, the way out's collision callback (joint 1).
pub const END_TOUCHED: u32 = 0x8020_B4D8;

/// The way out's collision joint.
const END_JOINT: i32 = 1;

/// `stage_info.flags`' bit for a reached way out, which ends the match.
const ESCAPED: u32 = 0x10;

/// `grZe_Route_803B83A0` (`grZebesRoute_LightData`): the lights' places in the stage's unscaled
/// units (its first vector, the camera's target before the first fighter's, is the origin).
const SPOT_POS: Vec3 = Vec3::new(0.0, -30.0, 50.0);
const SPOT_INTEREST: Vec3 = Vec3::new(0.0, 680.0, -20.0);
const UPPER_POINT_POS: Vec3 = Vec3::new(-5.0, 980.0, 5.0);
const LOWER_POINT_POS: Vec3 = Vec3::new(0.0, 215.0, -50.0);

/// A stage object's process or callback, by address: the escape's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x8020_B37C | 0x8020_B380 | 0x8020_B3B8 | 0x8020_B3BC | 0x8020_B4D4 => empty(r, gobj),
        0x8020_B348 | 0x8020_B384 => init_model(r, gobj),
        0x8020_B3C0 => init_shaft(r, gobj),
        0x8020_B42C => proc_shaft(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x8020_B164 => on_init(r),
        0x8020_B1D4 => on_load(r),
        0x8020_B1F4 => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing: the two models' processes, and
/// the destroy callbacks (`Ground::x1C_callback`), which only a stage's own destroying of an
/// object runs (`Ground_801C4A08`), as this stage never does.
#[doc(alias = "grZebesRoute_8020B37C")]
#[doc(alias = "grZebesRoute_8020B3B8")]
// Ports grZebesRoute_8020B380; aliased once a check reaches it.
// Ports grZebesRoute_8020B3BC; aliased once a check reaches it.
// Ports grZebesRoute_8020B4D4; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The stage's line-touch dynamics, `grZebesRoute_8020B854`: none.
#[doc(alias = "grZebesRoute_8020B854")]
pub fn on_touch_line(_line: i32) -> Option<DataRef> {
    None
}

/// The stage's shadow check, `grZebesRoute_8020B85C`: every shadow is drawn.
// Ports grZebesRoute_8020B85C; aliased once a check reaches it.
pub fn check_shadow_render() -> bool {
    true
}

/// The stage's objects' setup functions and processes, by map id (`grZe_Route_StageCallbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &ZEBESROUTE_CALLBACKS[map_id as usize]
}

/// Creates one of the stage's objects and sets it up, `grZebesRoute_8020B260`, its scale
/// joint's descriptor at `frame` on the stack.
#[doc(alias = "grZebesRoute_8020B260")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The escape's setup, `grZebesRoute_8020B164`: its parameters, the stage's `unk8C` flags (b4
/// off, b5 on), maps 0, 1 and 2 in that order, then the camera limits and blast zones measured.
#[doc(alias = "grZebesRoute_8020B164")]
pub fn on_init(r: &mut Running) {
    r.game.gr.zebesroute_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = false;
    r.game.stage_info.unk8C.b5 = true;
    for map_id in 0..3 {
        create(r, map_id, setup::SETUP_FRAME);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grZebesRoute_8020B160` does nothing (only a demo runs it).
// Ports grZebesRoute_8020B160; aliased once a check reaches it.
pub fn on_demo_init(_r: &mut Running) {}

/// The escape's load, `grZebesRoute_8020B1D4`: the lights ([`set_up_lights`]).
#[doc(alias = "grZebesRoute_8020B1D4")]
pub fn on_load(r: &mut Running) {
    set_up_lights(r);
}

/// The escape's start, `grZebesRoute_8020B1F4`: the enemy spawner with no table, then, by a
/// draw of one in the parameters' second word (none for 0, always taken), a trophy slot at a
/// random trophy point.
#[doc(alias = "grZebesRoute_8020B1F4")]
pub fn on_start(r: &mut Running) {
    zako::start(r.game, r.data, None);
    let p = param(r.game);
    let n = r.data.u32(p.at(4)) as i32;
    let val = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
    if val == 0 {
        // The point is drawn before the trophy, as the call's arguments are evaluated.
        let point = super::ground::trophy_point(r.game, r.data);
        let trophy = super::ground::x6e4_drawn(r.game);
        zako::set_trophy_slot(r.game, trophy, point);
    }
}

/// The parameters (`grZebesRoute_YakumonoParam`): the camera's timer and the trophy slot's
/// odds.
fn param(game: &Game) -> DataRef {
    game.gr
        .zebesroute_yakumono
        .expect("grzebesroute.c: yakumono_param")
}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The two models' setup (maps 0 and 2), `stageGObj0_OnInit` and `stageGObj2_OnInit`: their
/// animations.
#[doc(alias = "stageGObj0_OnInit@8020B348")]
#[doc(alias = "stageGObj2_OnInit@8020B384")]
pub fn init_model(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// The shaft's setup (map 1), `grZebesRoute_8020B3C0`: its animation, the way out's collision
/// callback on joint 1, and the camera's timer from the parameters' first word (`zebes2.xC4`, a
/// halfword).
#[doc(alias = "grZebesRoute_8020B3C0")]
pub fn init_shaft(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    let gp = &mut r.game.grounds[g.0];
    gp.x8_callback = None;
    gp.xC_callback = None;
    r.game.geometry_mut().set_ledge_callback(
        END_JOINT,
        Some(super::CollUser::Ground(g)),
        Some(CodeRef(END_TOUCHED)),
    );
    let timer = r.data.u32(param(r.game)) as i16;
    r.game.grounds[g.0].u.zebes_back().xC4 = timer;
}

/// The shaft's process (map 1), `grZebesRoute_8020B42C`: the camera's offset at the shaft's
/// middle and the first fighter's height (no lower than -50), the camera's timer counted down
/// and, once it has run out, the screen shaken (`QuakeKind_Loop`) every tick, and the wind
/// sources aged.
#[doc(alias = "grZebesRoute_8020B42C")]
pub fn proc_shaft(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    // The target starts as `player_pos` (the origin), which only the fighter's place replaces.
    if let Some(fighter) = p1_fighter(r.game) {
        let mut pos = r.game.ft_pos(fighter);
        if pos.y < -50.0 {
            pos.y = -50.0;
        }
        pos.x = 0.0;
        set_cam_offset(r.game, pos.x, pos.y);
    }
    let timer = i32::from(r.game.grounds[g.0].u.zebes_back().xC4);
    if timer > 0 {
        r.game.grounds[g.0].u.zebes_back().xC4 = (timer - 1) as i16;
    } else {
        super::super::camera::request_quake(r, super::super::camera::QUAKE_LOOP, None);
    }
    super::ground::quake_list_tick(r.game);
}

/// The way out's collision callback (joint 1), `fn_8020B4D8`: a fighter's collision
/// (`x34_flags.b1234` 1) on its floor (ground kind 1) asks, for [`EndTouch::apply`], whether it
/// is the first fighter's.
#[doc(alias = "fn_8020B4D8")]
pub fn end_touched(end: &mut EndTouch, coll_kind: u8, owner: Option<GObjId>, ground_kind: i32) {
    if coll_kind == 1 && ground_kind == 1 {
        end.0 = Some(owner);
    }
}

/// The way out touched in a collision pass, applied once mpcoll returns (the joint callback has
/// no access to the players): the touching collision's owner. Setting the flag is the
/// callback's only effect and nothing a pass reads depends on it, so it happens in the
/// original's order.
#[derive(Debug, Default)]
pub struct EndTouch(Option<Option<GObjId>>);

impl EndTouch {
    /// Marks the escape reached if the first fighter (`Ground_GetP1Fighter`) touched the way
    /// out.
    pub fn apply(self, game: &mut Game) {
        if let Some(owner) = self.0
            && p1_fighter(game) == owner
        {
            game.stage_info.flags |= ESCAPED;
        }
    }
}

/// Moves and colors the stage's lights, `grZebesRoute_8020B548`: of the first three lights
/// after the first in the lights' GX link (4), a spot light shines from below up the shaft (a
/// 45° cutoff, falling off at 600 in the stage's scale), a point light above 500 is the shaft's
/// top (at 1000), and one below 500 a blue glow low in it (at 400), each placed in the stage's
/// scale.
#[doc(alias = "grZebesRoute_8020B548")]
pub fn set_up_lights(r: &mut Running) {
    let lg = r
        .game
        .world
        .first_rendered(4)
        .expect("grzebesroute.c:361: gobj");
    let first = match r.game.world.gobj(lg).obj {
        Some(super::super::state::Obj::LObj(l)) => Some(l),
        _ => None,
    };
    let s = reg(scale(r.game));
    let mut l = first.and_then(|l| r.game.objs.lobjs[l].next);
    let spot_ref_dist = mem(fp::fmuls(600.0, s));
    let upper_ref_dist = mem(fp::fmuls(1000.0, s));
    let lower_ref_dist = mem(fp::fmuls(400.0, s));
    let split_y = 500.0;
    let scaled = |v: Vec3| {
        Vec3::new(
            mem(fp::fmuls(reg(v.x), s)),
            mem(fp::fmuls(reg(v.y), s)),
            mem(fp::fmuls(reg(v.z), s)),
        )
    };
    let mut i = 0;
    while i < 3 {
        let Some(cur) = l else { break };
        let current = lobj::position(&mut r.game.objs, r.data, Some(cur))
            .expect("grzebesroute.c:372: a light without a position");
        let ty = r.game.objs.lobjs[cur].flags & lobj::TYPE_MASK;
        if ty == lobj::SPOT {
            lobj::set_color(&mut r.game.objs.lobjs, cur, [0xFF, 0xFF, 0xCC, 0xFF]);
            lobj::set_position(&mut r.game.objs, cur, scaled(SPOT_POS));
            lobj::set_interest(&mut r.game.objs, cur, scaled(SPOT_INTEREST));
            lobj::set_spot(&mut r.game.objs.lobjs, Some(cur), 45.0, 3);
            lobj::set_dist_attn(&mut r.game.objs.lobjs, Some(cur), spot_ref_dist, 0.99, 3);
        } else if ty == lobj::POINT && current.y > split_y {
            lobj::set_color(&mut r.game.objs.lobjs, cur, [0xFD, 0xFD, 0xBF, 0xFF]);
            lobj::set_position(&mut r.game.objs, cur, scaled(UPPER_POINT_POS));
            lobj::set_dist_attn(&mut r.game.objs.lobjs, Some(cur), upper_ref_dist, 0.03, 3);
        } else if ty == lobj::POINT && current.y < split_y {
            lobj::set_color(&mut r.game.objs.lobjs, cur, [0x00, 0x00, 0xF7, 0xFF]);
            lobj::set_position(&mut r.game.objs, cur, scaled(LOWER_POINT_POS));
            lobj::set_dist_attn(&mut r.game.objs.lobjs, Some(cur), lower_ref_dist, 0.03, 3);
        }
        i += 1;
        l = r.game.objs.lobjs[cur].next;
    }
}
