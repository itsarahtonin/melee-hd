//! Icicle Mountain, `melee/gr/gricemt.c`: the scenery (map 0), the background (map 8) and, in
//! VS mode, the climb (map 9), which keeps two of the six fields (maps 1 to 6) in place, one
//! above the other, and moves them down or up at a speed it picks at random from the stage's
//! parameters; when the upper one has come down far enough, the lower one goes and another
//! field drawn at random takes the upper place, and the other way round. Each field holds
//! blocks a hit breaks (stage hurtbox items), and fields 2 and 4 have ice that falls when a
//! fighter lands on it. The climb's speed is the wind the stage blows on airborne items.
//!
//! The adventure's Icicle Mountain (stage kind 76) climbs instead with map 10: fields 1 to 5
//! from its parameters' list in turn, at speeds from another list with their waits, faster while
//! the first fighter stands high (`ft_max_y`); with the fields' list run out it brakes to a stop
//! under the last field. When the speeds' list reaches its end the climb's end starts the
//! arena's opponents (`gm_801674C4`). A Polar Bear landing from its jump speeds the climb up
//! for a while (`grIceMt_801FA6D8`).

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::handles::{CodeRef, GroundId};
use super::super::state::{Game, Running};
use super::super::structs::CollData;
use super::data::{ICEMT_CALLBACKS, StageCallbacks};
use super::ground::{ground_of, jobj_of, map_gobj, nth_joint, set_point_joint, update_map_coll};
use super::vars::{IceMtClimbVars, IceMtFieldVars};
use super::{CollUser, anime, datfiles, material, setup, zako};
use crate::fpu::{mem, reg};
use crate::hsd::Objects;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::pool::Pool;
use crate::vec::Vec3;

/// `fn_801F8C64`: a block's item hit.
pub const BLOCK_HIT: u32 = 0x801F_8C64;
/// `fn_801F9338`, `fn_801F9448` and `fn_801F9558`: field 2's two falling ices' collision
/// joints and field 4's, landed on.
pub const ICE_A_TOUCHED: u32 = 0x801F_9338;
pub const ICE_B_TOUCHED: u32 = 0x801F_9448;
pub const ICE_C_TOUCHED: u32 = 0x801F_9558;
/// `fn_801F75EC`, the climb's start hook.
const START_HOOK: u32 = 0x801F_75EC;
/// `stageGObj10_UnkGroundCallback`, the adventure's climb's start hook.
const CLIMB10_START_HOOK: u32 = 0x801F_77B0;
/// `onJointCollision`, the callback the adventure's climb gives each collision joint without
/// one (`mpJointSetCb1`).
pub const ON_JOINT: u32 = 0x801F_A7F0;
/// `fn_801FA4CC`, the callback the climb's end hands the arena's event.
pub const END_EVENT: u32 = 0x801F_A4CC;

/// The stage kind of the adventure's Icicle Mountain, whose setup makes the adventure's climb.
const ADVENTURE: i32 = 76;

/// The fields (`icemt_field`): map id, and the offsets of its top and bottom (times the stage's
/// scale).
const FIELDS: [(i32, f32, f32); 6] = [
    (1, 180.0, -180.0),
    (2, 190.0, -180.0),
    (3, 190.0, -195.0),
    (4, 195.0, -185.0),
    (5, 190.0, -200.0),
    (6, 180.0, -190.0),
];

/// The scenery's parts the setup places (`grIm_803B8220`, after the first).
const SCENERY: [Vec3; 4] = [
    Vec3 {
        x: -8.0,
        y: 15.0,
        z: 0.0,
    },
    Vec3 {
        x: 8.0,
        y: 15.0,
        z: 0.0,
    },
    Vec3 {
        x: -24.0,
        y: 15.0,
        z: 0.0,
    },
    Vec3 {
        x: 24.0,
        y: 15.0,
        z: 0.0,
    },
];

/// Each field's parts for its points of interest and its other twelve words (`x4` to `x30`),
/// and the parts its blocks hang from (`grIm_804DB588` to `grIm_804DB5A4`, `grIm_803B825C`).
struct FieldParts {
    parts: [i32; 12],
    blocks: &'static [i32],
}

const FIELD_PARTS: [FieldParts; 6] = [
    FieldParts {
        parts: [3, 4, 5, 6, 18, 11, 12, 13, 14, 15, 16, 17],
        blocks: &[1, 2],
    },
    FieldParts {
        parts: [6, 7, 8, 9, 21, 14, 15, 16, 17, 18, 19, 20],
        blocks: &[1, 2],
    },
    FieldParts {
        parts: [5, 6, 7, 8, 21, 14, 15, 16, 17, 18, 19, 20],
        blocks: &[1, 2, 3, 4],
    },
    FieldParts {
        parts: [8, 9, 10, 11, 23, 16, 17, 18, 19, 20, 21, 22],
        blocks: &[1, 2, 3, 4],
    },
    FieldParts {
        parts: [6, 7, 8, 9, 22, 15, 16, 17, 18, 19, 20, 21],
        blocks: &[1, 2, 3, 4, 5],
    },
    FieldParts {
        parts: [5, 6, 7, 8, 21, 14, 15, 16, 17, 18, 19, 20],
        blocks: &[1, 2, 3, 4],
    },
];

/// `grIceMt_YakumonoParam`'s fields, by offset.
mod p {
    /// A field's rest count once shown (`x0`).
    pub const FIELD_REST: u32 = 0x0;
    /// A speed's rest count once picked (`x2`).
    pub const SPEED_REST: u32 = 0x2;
    /// The twelve speeds, from offset 4.
    pub const SPEEDS: u32 = 0x4;
    /// The ticks a change of speed takes (`x34`).
    pub const LERP: u32 = 0x34;
    /// The bounds of a speed's ticks (`x36`, `x38`).
    pub const HOLD_A: u32 = 0x36;
    pub const HOLD_B: u32 = 0x38;
    /// A burst's speed (`x3C`, times the stage's scale).
    pub const BURST: u32 = 0x3C;
    /// The gap between two fields (`x40`).
    pub const GAP: u32 = 0x40;
    /// A Polar Bear's landing's burst's ticks (`x3A`).
    pub const BEAR_BURST: u32 = 0x3A;
    /// The height above which the first fighter pushes the adventure's climb (`ft_max_y`, a
    /// halfword at `x98`), the push's top speed (`x9C`), its step and the braking's (`xA0`), its
    /// ticks (`xA4`), the speeds' list's index that ends the climb (`xA6`) and the height the
    /// last field brakes to (`xA8`).
    pub const FT_MAX_Y: u32 = 0x98;
    pub const PUSH_MAX: u32 = 0x9C;
    pub const PUSH_STEP: u32 = 0xA0;
    pub const PUSH_TICKS: u32 = 0xA4;
    pub const END_IDX: u32 = 0xA6;
    pub const BRAKE_Y: u32 = 0xA8;
    /// The adventure's lists (pointers, each ending with -1): the fields' indices (`field_ixs`),
    /// the speeds' indices (`xB0`) and their waits (`xB4`).
    pub const FIELD_IXS: u32 = 0xAC;
    pub const SPEED_IXS: u32 = 0xB0;
    pub const SPEED_WAITS: u32 = 0xB4;
    /// The trophy slot's odds (`xB8`) and the enemy spawner's table (`xBC`, inline).
    pub const TROPHY_ODDS: u32 = 0xB8;
    pub const SPAWNS: u32 = 0xBC;
}

#[derive(Clone, Copy)]
struct Param(DataRef);

impl Param {
    fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }
    /// Entry `k` of the list the pointer at `off` names.
    fn list(self, data: &Archives, off: u32, k: i32) -> i32 {
        let p = data.ptr(self.0.at(off)).expect("gricemt.c: a list");
        i32::from(data.u16(p.at((2 * k) as u32)) as i16)
    }
}

/// The map id of `icemt_field[i]`: the adventure's code indexes the table with its lists'
/// entries, and its setup with ids too, which with the disc's lists stay within it.
fn field_id_at(i: i32) -> i32 {
    match FIELDS.get(i as usize) {
        Some(f) => f.0,
        None => panic!("gricemt.c: icemt_field[{i}] reads past the table"),
    }
}

/// Which climb a stage object runs: VS mode's (map 9) or the adventure's (map 10), which share
/// the speed, the fields' moves and their changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Climb {
    Vs,
    Adventure,
}

/// The climb's speed (`grIceMt_FA364_State`).
#[derive(Clone, Copy)]
struct Speed {
    phase: i16,
    delay: i16,
    lerp_count: i16,
    burst_count: i16,
    idx: i16,
    cur: f32,
}

fn speed(game: &mut Game, g: GroundId, c: Climb) -> Speed {
    match c {
        Climb::Vs => {
            let v = game.grounds[g.0].u.icemt_scroll();
            Speed {
                phase: v.phase,
                delay: v.delay,
                lerp_count: v.lerp_count,
                burst_count: v.burst_count,
                idx: v.idx,
                cur: v.cur,
            }
        }
        Climb::Adventure => {
            let v = game.grounds[g.0].u.icemt_climb();
            Speed {
                phase: v.phase,
                delay: v.delay,
                lerp_count: v.lerp_count,
                burst_count: v.burst_count,
                idx: v.idx,
                cur: v.cur,
            }
        }
    }
}

fn put_speed(game: &mut Game, g: GroundId, c: Climb, s: Speed) {
    macro_rules! put {
        ($v:expr) => {{
            let v = $v;
            v.phase = s.phase;
            v.delay = s.delay;
            v.lerp_count = s.lerp_count;
            v.burst_count = s.burst_count;
            v.idx = s.idx;
            v.cur = s.cur;
        }};
    }
    match c {
        Climb::Vs => put!(game.grounds[g.0].u.icemt_scroll()),
        Climb::Adventure => put!(game.grounds[g.0].u.icemt_climb()),
    }
}

fn climb_ids(game: &mut Game, g: GroundId, c: Climb) -> (i16, i16) {
    match c {
        Climb::Vs => ids(game, g),
        Climb::Adventure => {
            let v = game.grounds[g.0].u.icemt_climb();
            (v.under, v.upper)
        }
    }
}

fn set_ids(game: &mut Game, g: GroundId, c: Climb, under: i16, upper: i16) {
    match c {
        Climb::Vs => {
            let v = game.grounds[g.0].u.icemt_scroll();
            v.under = under;
            v.upper = upper;
        }
        Climb::Adventure => {
            let v = game.grounds[g.0].u.icemt_climb();
            v.under = under;
            v.upper = upper;
        }
    }
}

fn climb10(game: &mut Game, g: GroundId) -> &mut IceMtClimbVars {
    game.grounds[g.0].u.icemt_climb()
}

fn param(game: &Game) -> Param {
    Param(game.gr.icemt_yakumono.expect("gricemt.c: yakumono_param"))
}

/// A stage object's process or callback, by address: Icicle Mountain's.
pub fn run(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        0x801F_75E4 | 0x801F_75E8 | 0x801F_77AC | 0x801F_8C60 => empty(r, gobj),
        0x801F_72D4 => init_scenery(r, gobj),
        START_HOOK => on_climb_start(r, gobj),
        0x801F_75FC => init_climb(r, gobj),
        0x801F_7728 => proc_climb(r, gobj),
        0x801F_7D90 => empty(r, gobj),
        0x801F_785C => init_climb10(r, gobj),
        CLIMB10_START_HOOK => on_climb10_start(r, gobj),
        0x801F_7A2C => proc_climb10(r, gobj),
        0x801F_7D94 => init_field(r, gobj, 1),
        0x801F_7F70 => init_field(r, gobj, 2),
        0x801F_8208 => init_field(r, gobj, 3),
        0x801F_83EC => init_field(r, gobj, 4),
        0x801F_865C => init_field(r, gobj, 5),
        0x801F_8850 => init_field(r, gobj, 6),
        0x801F_7EE8 | 0x801F_8364 | 0x801F_87C8 | 0x801F_89AC => proc_field(r, gobj),
        0x801F_815C => proc_field2(r, gobj),
        0x801F_85C4 => proc_field4(r, gobj),
        0x801F_7F1C => destroy_field(r, gobj, 1),
        0x801F_81B4 => destroy_field(r, gobj, 2),
        0x801F_8398 => destroy_field(r, gobj, 3),
        0x801F_8608 => destroy_field(r, gobj, 4),
        0x801F_87FC => destroy_field(r, gobj, 5),
        0x801F_89E0 => destroy_field(r, gobj, 6),
        0x801F_8A34 => init_bg(r, gobj),
        0x801F_8B10 => proc_bg(r, gobj),
        _ => return false,
    }
    true
}

/// The stage's hooks without arguments, by address.
pub fn run_event(r: &mut Running, f: u32) -> bool {
    match f {
        0x801F_686C => on_init(r),
        0x801F_7080 => on_load(r),
        0x801F_71DC => on_start(r),
        _ => return false,
    }
    true
}

/// The objects' processes and destroy callbacks that do nothing: the scenery's, the climbs'
/// destroy callbacks and the background's.
#[doc(alias = "stageGObj0_GObjProc@801F75E4")]
#[doc(alias = "stageGObj0_Callback3@801F75E8")]
#[doc(alias = "stageGObj9_Callback3")]
#[doc(alias = "stageGObj8_Callback3")]
// Ports stageGObj10_Callback3; aliased once a check reaches it.
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// Creates one of the stage's objects and sets it up, `setupStageCallbacks`.
#[doc(alias = "setupStageCallbacks@801F71E8")]
pub fn create(r: &mut Running, map_id: i32, frame: u32) -> Option<GObjId> {
    setup::create_from_callbacks(r, map_id, callbacks(map_id), frame)
}

/// The stage's objects' setup functions and processes, by map id (`stage_callbacks`).
pub fn callbacks(map_id: i32) -> &'static StageCallbacks {
    &ICEMT_CALLBACKS[map_id as usize]
}

/// The frames the stage's objects are made from (the stack address of the scale joint's
/// descriptor, [`setup::create_from_callbacks`]): the setup's own; the climb's setup's, through
/// `stageGObj9_OnInit` and `grIceMt_801FA0BC` (0x88 below, with `setupStageCallbacks`' 0x20);
/// and the climb's process's, 0xD8 below the process loop's (`corneria::PROC_LOOP`; its own
/// frame 0x20, `grIceMt_801F9ACC`'s 0x98 and `setupStageCallbacks`' 0x20).
pub const SETUP_FRAME: u32 = setup::SETUP_FRAME;
pub const CLIMB_INIT_FRAME: u32 = SETUP_FRAME - 0x88;
pub const CLIMB_PROC_FRAME: u32 = super::corneria::PROC_LOOP - 0xD8;
/// The adventure's climb's: through `stageGObj10_OnInit` (0x18), `grIceMt_801FA0BC` (0x40) and
/// `setupStageCallbacks` (0x20), 0x78 below the setup's; and its process's (0x50, with
/// `grIceMt_801F9ACC`'s 0x98 and `setupStageCallbacks`' 0x20), 0x108 below the process loop's.
pub const CLIMB10_INIT_FRAME: u32 = SETUP_FRAME - 0x78;
pub const CLIMB10_PROC_FRAME: u32 = super::corneria::PROC_LOOP - 0x108;

/// Icicle Mountain's setup, `grIceMt_801F686C`: its parameters; the scenery and the background;
/// the six fields' collision off; in VS mode the climb; in the adventure (stage kind 76) the
/// fields its list puts third to fifth, each a field's height above the last over the first two
/// (whose ids it compares with the list's indices, and then indexes the fields' table with), and
/// its own climb; camera limits and blast zones over them all.
#[doc(alias = "grIceMt_801F686C")]
pub fn on_init(r: &mut Running) {
    r.game.gr.icemt_yakumono = super::ground::yakumono_param(r.game);
    r.game.stage_info.unk8C.b4 = true;
    r.game.stage_info.unk8C.b5 = false;
    create(r, 0, SETUP_FRAME);
    create(r, 8, SETUP_FRAME);
    for m in 1..=6 {
        setup::switch_off(r.game, r.data, m);
    }
    if super::stage::stkind(r.game) == ADVENTURE {
        let prm = param(r.game);
        let data = r.data;
        let ixs = |k: i32| prm.list(data, p::FIELD_IXS, k);
        let pick = |skip: &[i32], line: u32| {
            for f in FIELDS {
                let id = f.0;
                if ixs(0) == id || ixs(1) == id || skip.contains(&id) {
                    continue;
                }
                return id;
            }
            panic!("gricemt.c: {line}: i<ICEMT_FIELD_MAX")
        };
        let f30 = pick(&[], 600);
        let f29 = pick(&[f30], 609);
        let f28 = pick(&[f30, f29], 619);
        let s = reg(super::ground::scale(r.game));
        let gap = |a: i32, b: i32| reg(field_gap(r.game, data, field_id_at(a), field_id_at(b)));
        let y = fp::fadds(fp::fmuls(fp::fneg(20.0), s), gap(ixs(0), ixs(1)));
        let y2 = fp::fadds(y, gap(ixs(1), f30));
        let y3 = fp::fadds(y2, gap(f30, f29));
        let y4 = fp::fadds(y3, gap(f29, f28));
        let mut fields = [None; 3];
        for (k, (y, line)) in [(y2, 636), (y3, 641), (y4, 646)].into_iter().enumerate() {
            let gobj = create(r, field_id_at(ixs(2 + k as i32)), SETUP_FRAME);
            assert!(gobj.is_some(), "gricemt.c: {line}: gobj");
            let j = jobj_of(r.game, gobj.unwrap());
            assert!(j.is_some(), "gricemt.c: {}: jobj", line + 1);
            jobj::set_srt(&mut r.game.objs.jobjs, j.unwrap(), |o| {
                o.translate.y = mem(y)
            });
            fields[k] = gobj;
        }
        r.game.gr.icemt_fields = fields;
        create(r, 10, SETUP_FRAME);
    } else {
        create(r, 9, SETUP_FRAME);
        r.game.gr.icemt_fields = [None; 3];
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// `grIceMt_801F6868` does nothing (only a demo runs it).
#[doc(alias = "grIceMt_801F6868")]
pub fn on_demo_init(_r: &mut Running) {}

/// Icicle Mountain's load, `grIceMt_801F7080`: the adventure's first fields gone, the fields'
/// collision where their models stand, the enemy spawner (with no enemies in VS mode; in the
/// adventure with its table, a trophy slot by the parameters' odds, and a first scan that spawns
/// on camera too), and the stage's effect 0x7530 of bank 30.
#[doc(alias = "grIceMt_801F7080")]
pub fn on_load(r: &mut Running) {
    for g in r.game.gr.icemt_fields {
        if g.is_some() {
            setup::destroy(r, g);
        }
    }
    for m in 1..=6 {
        if let Some(g) = map_gobj(r.game, m) {
            update_map_coll(r.game, r.data, g);
        }
    }
    if super::stage::stkind(r.game) == ADVENTURE {
        let prm = param(r.game);
        zako::start(r.game, r.data, Some(prm.0.at(p::SPAWNS)));
        let n = i32::from(prm.h(r.data, p::TROPHY_ODDS));
        let val = if n != 0 { r.game.objs.rng.below(n) } else { 0 };
        if val == 0 {
            // The point is drawn before the trophy, as the call's arguments are evaluated.
            let point = super::ground::trophy_point(r.game, r.data);
            let trophy = super::ground::x6e4_drawn(r.game);
            zako::set_trophy_slot(r.game, trophy, point);
        }
        zako::scan_anywhere(r);
    } else {
        zako::start(r.game, r.data, None);
    }
    super::grlib::generator_at(r.game, r.data, 0x7530, 0x1E, Vec3::ZERO);
}

/// `grIceMt_801F71DC` does nothing.
#[doc(alias = "grIceMt_801F71DC")]
pub fn on_start(_r: &mut Running) {}

fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// The scenery's setup (map 0), `stageGObj0_OnInit`: its animation, four parts placed, and the
/// camera limits and blast zones measured again.
#[doc(alias = "stageGObj0_OnInit@801F72D4")]
pub fn init_scenery(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    for (k, pos) in SCENERY.iter().enumerate() {
        if let Some(j) = nth_joint(r.game, gobj, 18 + k as i32) {
            jobj::set_translate(&mut r.game.objs.jobjs, j, *pos);
        }
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
}

/// The climb's start hook, `fn_801F75EC`: the climb starts moving.
#[doc(alias = "fn_801F75EC")]
pub fn on_climb_start(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    r.game.grounds[g.0].u.icemt_scroll().x14 = 0;
}

/// Draws a field at random whose rest count is 0, and gives it the rest count once shown.
fn draw_field(r: &mut Running, g: GroundId) -> i32 {
    let rest = param(r.game).h(r.data, p::FIELD_REST);
    let i = loop {
        let i = r.game.objs.rng.below(6) as usize;
        if r.game.grounds[g.0].u.icemt_scroll().x30[i] == 0 {
            break i;
        }
    };
    r.game.grounds[g.0].u.icemt_scroll().x30[i] = rest;
    FIELDS[i].0
}

/// The climb's setup (map 9), `stageGObj9_OnInit`: the rest counts cleared, two fields drawn
/// at random to stand below and above, set up in place, the speed at rest, and held until the
/// match starts.
#[doc(alias = "stageGObj9_OnInit")]
pub fn init_climb(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    {
        let v = r.game.grounds[g.0].u.icemt_scroll();
        v.x18 = [0; 12];
        v.x30 = [0; 10];
    }
    let under = draw_field(r, g);
    r.game.grounds[g.0].u.icemt_scroll().under = under as i16;
    let upper = draw_field(r, g);
    r.game.grounds[g.0].u.icemt_scroll().upper = upper as i16;
    place_fields(r, g);
    let v = r.game.grounds[g.0].u.icemt_scroll();
    v.x16 = 0;
    v.phase = 0;
    v.burst_count = 0;
    v.delay = 0;
    v.lerp_count = 0;
    v.idx = -1;
    v.cur = 0.0;
    v.x14 = 1;
    setup::add_start_hook(r.game, gobj, CodeRef(START_HOOK));
}

/// The climb's process, `stageGObj9_GObjProc`, once the match has started: the speed's step
/// (a new one drawn when it's time), both fields moved by it, a field changed when the upper
/// one has come down or the lower one up far enough, and the background told the speed.
#[doc(alias = "stageGObj9_GObjProc@801F7728")]
pub fn proc_climb(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    if r.game.grounds[g.0].u.icemt_scroll().x14 != 0 {
        return;
    }
    let (y, _) = speed_step(r, g);
    let (under, upper) = ids(r.game, g);
    move_fields(r, under, upper, mem(fp::fneg(reg(y))));
    change_field(r, g);
    tell_bg(r.game, y);
}

fn ids(game: &mut Game, g: GroundId) -> (i16, i16) {
    let v = game.grounds[g.0].u.icemt_scroll();
    (v.under, v.upper)
}

/// The climb's speed this tick, `grIceMt_801FA364`: at rest, its wait counts down and, past 0,
/// the next speed is drawn (`fn_801F8E58`) with its wait, and the change begins; changing, the
/// speed closes on the new one by its distance over the ticks left, and takes it on the last.
/// A burst overrides it while it lasts. Whether no change was under way. VS mode's climb's
/// ([`climb_speed`]).
pub fn speed_step(r: &mut Running, g: GroundId) -> (f32, bool) {
    climb_speed(r, g, Climb::Vs)
}

/// [`speed_step`] for either climb, `grIceMt_801FA364`: VS mode's draws its next speed with
/// `fn_801F8E58`, the adventure's takes the next of its list with `fn_801F9150`.
#[doc(alias = "grIceMt_801FA364")]
pub fn climb_speed(r: &mut Running, g: GroundId, c: Climb) -> (f32, bool) {
    let prm = param(r.game);
    let mut ret = true;
    let mut st = speed(r.game, g, c);
    match st.phase {
        0 => {
            let tmp = st.delay;
            st.delay = tmp.wrapping_sub(1);
            if tmp < 0 {
                // The draws write their own words, never the speed's.
                put_speed(r.game, g, c, st);
                let (idx, next) = match c {
                    Climb::Vs => draw_speed(r, g),
                    Climb::Adventure => next_speed(r, g),
                };
                st.idx = idx as i16;
                st.delay = next as i16;
                st.phase = 1;
                st.lerp_count = prm.h(r.data, p::LERP);
            }
        }
        1 => {
            st.lerp_count = st.lerp_count.wrapping_sub(1);
            let tmp = st.lerp_count;
            let to = prm.f(r.data, p::SPEEDS + 4 * i32::from(st.idx) as u32);
            if tmp != 0 {
                let d = fp::fdivs(fp::fsubs(reg(to), reg(st.cur)), fp::frsp(f64::from(tmp)));
                st.cur = mem(fp::fadds(reg(st.cur), d));
                ret = false;
            } else {
                st.phase = 0;
                st.cur = to;
            }
        }
        _ => {}
    }
    let s = super::ground::scale(r.game);
    let burst = prm.f(r.data, p::BURST);
    let mut result = st.cur;
    if st.burst_count != 0 {
        result = mem(fp::fmuls(reg(burst), reg(s)));
        st.burst_count = st.burst_count.wrapping_sub(1);
    }
    put_speed(r.game, g, c, st);
    (result, ret)
}

/// The adventure's climb's next speed, `fn_801F9150`: the speeds' list's entry at the cursor
/// with its wait, the cursor moved on; at the list's end (-1 in either) the cursor goes back to
/// its start.
#[doc(alias = "fn_801F9150")]
pub fn next_speed(r: &mut Running, g: GroundId) -> (i32, i32) {
    let prm = param(r.game);
    loop {
        let k = i32::from(climb10(r.game, g).x1A);
        let a = prm.list(r.data, p::SPEED_IXS, k);
        let out = prm.list(r.data, p::SPEED_WAITS, k);
        if a != -1 && out != -1 {
            let v = climb10(r.game, g);
            v.x1A = v.x1A.wrapping_add(1);
            return (a, out);
        }
        climb10(r.game, g).x1A = 0;
    }
}

/// The adventure's climb's next field, `fn_801F91A8`: the fields' list's entry at its cursor
/// as a map id (moving the cursor on), or -1 at the list's end.
#[doc(alias = "fn_801F91A8")]
pub fn next_climb_field(r: &mut Running, g: GroundId) -> i32 {
    let prm = param(r.game);
    let k = i32::from(climb10(r.game, g).x1C);
    let index = prm.list(r.data, p::FIELD_IXS, k);
    if index == -1 {
        return -1;
    }
    let id = field_id_at(index);
    let v = climb10(r.game, g);
    v.x1C = v.x1C.wrapping_add(1);
    id
}

/// Draws the climb's next speed, `fn_801F8E58`: one at random of those whose rest count is 0
/// (all twelve in VS mode; the stage kind 212 leaves out the first four), every rest count
/// down one, the speed's set, and a wait drawn between the parameters' two bounds.
#[doc(alias = "fn_801F8E58")]
pub fn draw_speed(r: &mut Running, g: GroundId) -> (i32, i32) {
    let stkind = super::stage::stkind(r.game);
    let prm = param(r.game);
    let rest = r.game.grounds[g.0].u.icemt_scroll().x18;
    let list: Vec<i32> = (0..12)
        .filter(|&i| rest[i as usize] == 0 && (stkind != 212 || i >= 4))
        .collect();
    assert!(!list.is_empty(), "gricemt.c: 2077: max");
    let n = list.len() as i32;
    let chosen = list[if n != 0 {
        r.game.objs.rng.below(n) as usize
    } else {
        0
    }];
    let speed_rest = prm.h(r.data, p::SPEED_REST);
    let v = r.game.grounds[g.0].u.icemt_scroll();
    for c in v.x18.iter_mut() {
        if *c > 0 {
            *c -= 1;
        }
    }
    v.x18[chosen as usize] = speed_rest;
    let a = i32::from(prm.h(r.data, p::HOLD_A));
    let b = i32::from(prm.h(r.data, p::HOLD_B));
    let wait = super::grlib::rand_range(&mut r.game.objs.rng, a, b);
    (chosen, wait)
}

/// Draws the field to come next, `fn_801F9038`: one at random whose rest count is 0 and that
/// isn't shown, every rest count down one, and its set.
#[doc(alias = "fn_801F9038")]
pub fn next_field(r: &mut Running, g: GroundId) -> i32 {
    let rest = param(r.game).h(r.data, p::FIELD_REST);
    let idx = loop {
        let idx = r.game.objs.rng.below(6) as usize;
        let v = r.game.grounds[g.0].u.icemt_scroll();
        let id = FIELDS[idx].0 as i16;
        if v.x30[idx] == 0 && v.under != id && v.upper != id {
            break idx;
        }
    };
    let v = r.game.grounds[g.0].u.icemt_scroll();
    for c in v.x30[..6].iter_mut() {
        if *c > 0 {
            *c -= 1;
        }
    }
    v.x30[idx] = rest;
    FIELDS[idx].0
}

/// Tells the background the climb's speed, `grIceMt_801F9668`.
#[doc(alias = "grIceMt_801F9668")]
pub fn tell_bg(game: &mut Game, y: f32) {
    let bg = map_gobj(game, 8);
    assert!(bg.is_some(), "gricemt.c: 2615: bg_gobj");
    let g = ground_of(game, bg.unwrap());
    game.grounds[g.0].u.icemt_bg().x0 = y;
}

/// Moves the two fields shown up by `y`, `grIceMt_801F96E0` (it returns nothing its caller
/// reads).
#[doc(alias = "grIceMt_801F96E0")]
pub fn move_fields(r: &mut Running, under: i16, upper: i16, y: f32) {
    for id in [under, upper] {
        if id == -1 {
            continue;
        }
        let gobj = map_gobj(r.game, i32::from(id));
        assert!(gobj.is_some(), "gricemt.c: mgobj");
        let j = jobj_of(r.game, gobj.unwrap()).expect("gricemt.c: jobj");
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(y)))
        });
    }
}

/// A field's points of interest, `grIceMt_801F98A8`'s body: if due, its first four parts
/// become the stage's points 0 to 3.
fn give_points(game: &mut Game, g: GroundId) {
    let map = game.grounds[g.0].map_id as u8;
    let v = game.grounds[g.0].u.icemt_field(map);
    if v.flags & IceMtFieldVars::B1 == 0 {
        return;
    }
    v.flags &= !IceMtFieldVars::B1;
    let parts = v.parts;
    for (k, j) in parts[..4].iter().enumerate() {
        if j.is_some() {
            set_point_joint(game, k as i32, *j);
        }
    }
}

/// A field's points of interest when due, `grIceMt_801F98A8`.
#[doc(alias = "grIceMt_801F98A8")]
pub fn update_points(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    give_points(r.game, g);
}

/// The height from one field to the next above, `grIceMt_801F993C`: the upper one's bottom
/// below the lower one's top, plus the gap (times the stage's scale).
#[doc(alias = "grIceMt_801F993C")]
pub fn field_gap(game: &Game, data: &Archives, under_id: i32, upper_id: i32) -> f32 {
    let y = reg(super::ground::scale(game));
    let a = FIELDS.iter().position(|f| f.0 == under_id);
    assert!(a.is_some(), "gricemt.c: 2717: upper_ix<ICEMT_FIELD_MAX");
    let b = FIELDS.iter().position(|f| f.0 == upper_id);
    assert!(b.is_some(), "gricemt.c: 2722: under_ix<ICEMT_FIELD_MAX");
    let gap = param(game).f(data, p::GAP);
    mem(fp::fnmsubs(
        y,
        reg(FIELDS[a.unwrap()].2),
        fp::fmadds(y, reg(FIELDS[b.unwrap()].1), reg(gap)),
    ))
}

fn field_ground(game: &Game, id: i16) -> GroundId {
    let gobj = map_gobj(game, i32::from(id)).expect("gricemt.c: a field");
    ground_of(game, gobj)
}

fn field_vars(game: &mut Game, g: GroundId) -> &mut IceMtFieldVars {
    let map = game.grounds[g.0].map_id as u8;
    game.grounds[g.0].u.icemt_field(map)
}

/// Sets up a new field at the height `y`: in place, its collision on and settled where it
/// stands (`grIceMt_801F9ACC`'s order).
fn bring_field(r: &mut Running, id: i16, y: f32, c: Climb) {
    let frame = match c {
        Climb::Vs => CLIMB_PROC_FRAME,
        Climb::Adventure => CLIMB10_PROC_FRAME,
    };
    let gobj = create(r, i32::from(id), frame);
    assert!(gobj.is_some(), "gricemt.c: mgobj");
    let gobj = gobj.unwrap();
    let j = jobj_of(r.game, gobj).expect("gricemt.c: jobj");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    setup::settle_map_coll(r.game, r.data, i32::from(id));
    update_map_coll(r.game, r.data, gobj);
    setup::switch_on(r.game, r.data, i32::from(id));
}

/// Marks a field's points of interest due and gives them.
fn points_now(game: &mut Game, id: i16) {
    let Some(gobj) = map_gobj(game, i32::from(id)) else {
        return;
    };
    let g = ground_of(game, gobj);
    field_vars(game, g).flags |= IceMtFieldVars::B1;
    give_points(game, g);
}

/// Changes the fields shown when the climb has gone far enough, `grIceMt_801F9ACC` (its height
/// argument unused): the field nearer its place gets its points of interest due, the field
/// whose half is past the middle stands as the climb's; past the upper one's place the lower
/// one goes and the next field (`fn_801F9038`) comes above, past the lower one's the upper one
/// goes and the next comes below, and the scenery's points follow. VS mode's climb's
/// ([`change_climb_field`]).
pub fn change_field(r: &mut Running, g: GroundId) -> bool {
    change_climb_field(r, g, Climb::Vs)
}

/// [`change_field`] for either climb, `grIceMt_801F9ACC`: the next field VS mode's draws
/// (`fn_801F9038`) or the adventure's list gives (`fn_801F91A8`, -1 at its end, when no field
/// comes).
#[doc(alias = "grIceMt_801F9ACC")]
pub fn change_climb_field(r: &mut Running, g: GroundId, c: Climb) -> bool {
    let next = |r: &mut Running| match c {
        Climb::Vs => next_field(r, g),
        Climb::Adventure => next_climb_field(r, g),
    };
    let (under, upper) = climb_ids(r.game, g, c);
    if under == -1 || upper == -1 {
        return false;
    }
    let y0 = reg(field_gap(
        r.game,
        r.data,
        i32::from(under),
        i32::from(upper),
    ));
    let up = map_gobj(r.game, i32::from(upper));
    assert!(up.is_some(), "gricemt.c: 2745: mgobj");
    let j = jobj_of(r.game, up.unwrap()).expect("gricemt.c: 2746: jobj");
    let cur = reg(r.game.objs.jobjs[j].translate.y);
    let abs = |x: f64| if x < 0.0 { fp::fneg(x) } else { x };
    if abs(cur) < 10.0 {
        let fg = field_ground(r.game, upper);
        field_vars(r.game, fg).flags |= IceMtFieldVars::B1;
    } else if abs(fp::fadds(cur, y0)) < 10.0 {
        let fg = field_ground(r.game, under);
        field_vars(r.game, fg).flags |= IceMtFieldVars::B1;
    }
    let (stand, leave) = if cur < fp::fmuls(0.5, fp::fneg(y0)) {
        (under, upper)
    } else {
        (upper, under)
    };
    if stand != -1 {
        let fg = field_ground(r.game, stand);
        let v = field_vars(r.game, fg);
        if v.flags & IceMtFieldVars::B0 == 0 {
            v.flags |= IceMtFieldVars::B0 | IceMtFieldVars::B1;
        }
    }
    if leave != -1 {
        let fg = field_ground(r.game, leave);
        field_vars(r.game, fg).flags &= !IceMtFieldVars::B0;
    }
    let mut result = false;
    if cur > 0.0 {
        if under != -1 {
            let gone = map_gobj(r.game, i32::from(under));
            assert!(gone.is_some(), "gricemt.c: 2803: mgobj");
            setup::destroy(r, gone);
        }
        set_ids(r.game, g, c, upper, upper);
        let new = next(r);
        set_ids(r.game, g, c, upper, new as i16);
        if new != -1 {
            let y1 = reg(field_gap(r.game, r.data, i32::from(upper), new));
            bring_field(r, new as i16, mem(fp::fsubs(cur, y1)), c);
        }
        points_now(r.game, upper);
        result = true;
    } else if cur < fp::fneg(y0) {
        if upper != -1 {
            let gone = map_gobj(r.game, i32::from(upper));
            assert!(gone.is_some(), "gricemt.c: 2835: mgobj");
            setup::destroy(r, gone);
        }
        set_ids(r.game, g, c, under, under);
        let new = next(r);
        set_ids(r.game, g, c, new as i16, under);
        if new != -1 {
            let y1 = reg(field_gap(r.game, r.data, new, i32::from(under)));
            bring_field(r, new as i16, mem(fp::fadds(y1, fp::fadds(cur, y0))), c);
        }
        points_now(r.game, under);
        result = true;
    }
    if result {
        scenery_points(r.game);
    }
    result
}

/// Sets up the climb's first two fields in place, `grIceMt_801FA0BC`: the lower one a field's
/// height below the upper one, both 20 below the middle (times the stage's scale), their
/// collision on and settled, the upper one standing as the climb's with its points of interest,
/// and the scenery's points. VS mode's climb's ([`place_climb_fields`]).
pub fn place_fields(r: &mut Running, g: GroundId) {
    place_climb_fields(r, g, Climb::Vs);
}

/// [`place_fields`] for either climb, `grIceMt_801FA0BC`.
#[doc(alias = "grIceMt_801FA0BC")]
pub fn place_climb_fields(r: &mut Running, g: GroundId, c: Climb) {
    let frame_at = match c {
        Climb::Vs => CLIMB_INIT_FRAME,
        Climb::Adventure => CLIMB10_INIT_FRAME,
    };
    let (under, upper) = climb_ids(r.game, g, c);
    let frame = reg(field_gap(
        r.game,
        r.data,
        i32::from(under),
        i32::from(upper),
    ));
    let s = reg(super::ground::scale(r.game));
    if under != -1 {
        let gobj = create(r, i32::from(under), frame_at);
        assert!(gobj.is_some(), "gricemt.c: 2884: mgobj");
        let gobj = gobj.unwrap();
        let j = jobj_of(r.game, gobj).expect("gricemt.c: 2886: jobj");
        let y = mem(fp::fmadds(fp::fneg(20.0), s, frame));
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
        setup::switch_on(r.game, r.data, i32::from(under));
        update_map_coll(r.game, r.data, gobj);
        setup::settle_map_coll(r.game, r.data, i32::from(under));
    }
    if upper != -1 {
        let gobj = create(r, i32::from(upper), frame_at);
        assert!(gobj.is_some(), "gricemt.c: 2896: mgobj");
        let gobj = gobj.unwrap();
        let j = jobj_of(r.game, gobj).expect("gricemt.c: 2898: jobj");
        let y = mem(fp::fmuls(fp::fneg(20.0), s));
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
        setup::switch_on(r.game, r.data, i32::from(upper));
        update_map_coll(r.game, r.data, gobj);
        setup::settle_map_coll(r.game, r.data, i32::from(upper));
        let fg = ground_of(r.game, gobj);
        field_vars(r.game, fg).flags |= IceMtFieldVars::B0 | IceMtFieldVars::B1;
        give_points(r.game, fg);
    }
    scenery_points(r.game);
}

/// The scenery's parts 18 to 21 as the stage's points 4 to 7, `grIceMt_801FA854`.
#[doc(alias = "grIceMt_801FA854")]
pub fn scenery_points(game: &mut Game) {
    let Some(gobj) = map_gobj(game, 0) else {
        return;
    };
    for k in 0..4 {
        let j = nth_joint(game, gobj, 0x12 + k);
        if j.is_some() {
            set_point_joint(game, 4 + k, j);
        }
    }
}

/// A field's setup (maps 1 to 6), `stageGObj1_OnInit` to `stageGObj6_OnInit`: its collision
/// and animation, looping; its parts; not the climb's; its blocks; and on fields 2 and 4 the
/// falling ice, stopped (field 4's setup also asks the stage's scale, unused).
#[doc(alias = "stageGObj1_OnInit@801F7D94")]
#[doc(alias = "stageGObj2_OnInit@801F7F70")]
#[doc(alias = "stageGObj3_OnInit@801F8208")]
#[doc(alias = "stageGObj4_OnInit@801F83EC")]
#[doc(alias = "stageGObj5_OnInit@801F865C")]
#[doc(alias = "stageGObj6_OnInit@801F8850")]
pub fn init_field(r: &mut Running, gobj: GObjId, field: i32) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    setup::init_map_coll(r.game, r.data, j, m);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, true);
    let fp_ = &FIELD_PARTS[(field - 1) as usize];
    let ice = match field {
        2 => [nth_joint(r.game, gobj, 4), nth_joint(r.game, gobj, 5)],
        4 => [nth_joint(r.game, gobj, 7), nth_joint(r.game, gobj, 5)],
        _ => [None, None],
    };
    let parts = fp_.parts.map(|d| nth_joint(r.game, gobj, d));
    {
        let v = r.game.grounds[g.0].u.icemt_field(field as u8);
        v.parts = parts;
        v.flags &= !IceMtFieldVars::B0;
    }
    make_blocks(r, gobj, fp_.blocks);
    match field {
        2 => {
            let a = part_index(r.game, gobj, ice[0]);
            ice_slot(r, gobj, 0, a, -1, 0x25, 0x109, 0x27E, ICE_A_TOUCHED);
            let b = part_index(r.game, gobj, ice[1]);
            ice_slot(r, gobj, 1, b, -1, 38, 265, 638, ICE_B_TOUCHED);
        }
        4 => {
            let second = part_index(r.game, gobj, ice[1]);
            let first = part_index(r.game, gobj, ice[0]);
            ice_slot(r, gobj, 0, first, second, 117, 265, 638, ICE_C_TOUCHED);
        }
        _ => {}
    }
}

/// A field's blocks, `grIceMt_801F8CDC`: on each part, the stage's block model (its map 7) as
/// a child and a hurtbox item following the part, 6 across from 1 below to 1 above, on, on the
/// ground, whose hit breaks the block.
#[doc(alias = "grIceMt_801F8CDC")]
pub fn make_blocks(r: &mut Running, gobj: GObjId, parts: &[i32]) {
    let g = ground_of(r.game, gobj);
    let desc = datfiles::main_head(r.game).entry(r.data, 7).joint(r.data);
    assert!(
        parts.len() <= 20,
        "gricemt.c: 2004: block_num<=BLOCK_COLL_JOBJ_MAX"
    );
    let parents: Vec<Option<JObjId>> = parts.iter().map(|&d| nth_joint(r.game, gobj, d)).collect();
    for (i, parent) in parents.into_iter().enumerate() {
        assert!(parent.is_some(), "gricemt.c: 2019: coll_jobj");
        let block = jobj::load_joint(&mut r.game.objs, r.data, desc);
        assert!(block.is_some(), "gricemt.c: 2022: block_jobj");
        jobj::add_child(&mut r.game.objs.jobjs, parent, block);
        let item = material::stage_item(r, 8, 0, g, parent, None, Some(CodeRef(BLOCK_HIT)), None);
        if let Some(item) = item {
            material::set_item_hurtbox(
                r.game,
                item,
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                6.0,
            );
            material::item_hurtbox_on(r.game, item);
            material::item_ground_or_air(r.game, item, 0);
        }
        field_vars(r.game, g).items[i] = item;
    }
}

/// A block's item hit, `fn_801F8C64`: its hurtbox off, the block hidden, its effect where it
/// stood, a sound and a small quake.
#[doc(alias = "fn_801F8C64")]
pub fn block_hit(r: &mut Running, item: GObjId, _g: GroundId) {
    material::item_hurtbox_off(r.game, item);
    let anchor = super::super::it::item_mut(r.game, item)
        .xDD4_itemVar
        .yaku()
        .anchor
        .get();
    let j = match anchor {
        super::super::it::kinds::yaku::Anchor::Joint(j) => j,
        other => panic!("gricemt.c: a block's item follows {other:?}"),
    };
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    let pos = super::super::it::item(r.game, item).pos;
    super::super::ef::spawn(r, 0x445, Some(item), &[super::super::ef::Arg::Vec(pos)]);
    r.stage_sound(310);
    super::super::camera::request_quake(r, 2, Some(pos));
}

/// The index of `part` below a stage object's root in depth-first order, not entering
/// instances' children, or -1, `grIceMt_801FA500`: the inverse of [`nth_joint`].
#[doc(alias = "grIceMt_801FA500")]
pub fn part_index(game: &Game, gobj: GObjId, part: Option<JObjId>) -> i32 {
    let jobjs = &game.objs.jobjs;
    let root = jobj_of(game, gobj);
    assert!(root.is_some(), "gricemt.c: 2993: jobj");
    let mut j = jobjs[root.unwrap()].child;
    assert!(j.is_some(), "gricemt.c: 2994: jobj");
    let mut count = 0;
    while let Some(cur) = j {
        if Some(cur) == part {
            break;
        }
        let o = &jobjs[cur];
        j = if o.flags & jobj::INSTANCE == 0 && o.child.is_some() {
            o.child
        } else if o.next.is_some() {
            o.next
        } else {
            let mut up = cur;
            loop {
                let Some(p) = jobjs[up].parent else {
                    break None;
                };
                if jobjs[p].next.is_some() {
                    break jobjs[p].next;
                }
                up = p;
            }
        };
        count += 1;
    }
    if j.is_some() { count } else { -1 }
}

/// Readies a field's falling ice, `grIceMt_801F91EC`: its words (stopped, its parts, its
/// collision joint and the ticks it leaves and comes back), its parts' animations at rate 0,
/// and its collision joint telling the field when it's landed on.
#[doc(alias = "grIceMt_801F91EC")]
#[allow(clippy::too_many_arguments)]
pub fn ice_slot(
    r: &mut Running,
    gobj: GObjId,
    slot: usize,
    part: i32,
    part2: i32,
    joint: i32,
    gone: i32,
    back: i32,
    cb: u32,
) {
    let g = ground_of(r.game, gobj);
    field_vars(r.game, g).slots[slot].0 = [
        0,
        0,
        part as i16,
        part2 as i16,
        joint as i16,
        gone as i16,
        back as i16,
    ];
    anime::set_rate(r.game, gobj, part, 7, 0.0, true);
    if part2 != -1 {
        anime::set_rate(r.game, gobj, part2, 7, 0.0, true);
    }
    r.game
        .geometry_mut()
        .set_ledge_callback(joint, Some(CollUser::Ground(g)), Some(CodeRef(cb)));
}

/// A field's falling ice for a tick, `grIceMt_801F929C`: while it runs, its tick counts, its
/// collision joint leaves the search and comes back at their ticks, and it stops once its part's
/// animation has ended.
#[doc(alias = "grIceMt_801F929C")]
pub fn run_ice(r: &mut Running, gobj: GObjId, slot: usize) {
    let g = ground_of(r.game, gobj);
    let s = &mut field_vars(r.game, g).slots[slot].0;
    if s[0] == 0 {
        return;
    }
    s[1] = s[1].wrapping_add(1);
    let (tick, joint, gone, back, part) = (s[1], s[4], s[5], s[6], s[2]);
    if tick == gone {
        r.game.geometry_mut().disable_joint(i32::from(joint));
    } else if tick == back {
        r.game.geometry_mut().enable_joint(i32::from(joint));
    }
    if anime::finished(r.game, gobj, i32::from(part), 7) {
        field_vars(r.game, g).slots[slot].0[0] = 0;
    }
}

/// Field 2's and field 4's falling ice landed on (`fn_801F9338`, `fn_801F9448` for field 2's two
/// and `fn_801F9558` for field 4's): a fighter landing (`x34_flags.b1234` 1) starts it if it's
/// still, its parts' animations run once from their start; then the adventure's touch
/// (`onJointCollision`), which the caller records ([`ClimbTouch`]).
#[doc(alias = "fn_801F9338")]
#[doc(alias = "fn_801F9448")]
#[doc(alias = "fn_801F9558")]
pub fn ice_touched(
    objs: &mut Objects,
    grounds: &mut Pool<super::super::structs::Ground>,
    map_root: Option<JObjId>,
    g: GroundId,
    cb: u32,
    coll: &CollData,
) {
    let slot = usize::from(cb == ICE_B_TOUCHED);
    let map = grounds[g.0].map_id as u8;
    let s = &mut grounds[g.0].u.icemt_field(map).slots[slot].0;
    if i32::from(coll.x34_flags.b1234) == 1 && s[0] == 0 {
        s[0] = 1;
        s[1] = 0;
        let parts = [s[2], s[3]];
        for (k, part) in parts.into_iter().enumerate() {
            if k == 1 && part == -1 {
                break;
            }
            let part = i32::from(part);
            anime::set_rate_below(objs, map_root, part, 7, 1.0, true);
            anime::req_anim_below(objs, map_root, part, 7, 0.0, true);
            anime::clear_loop_below(objs, map_root, part, 7, true);
        }
    }
}

/// The adventure's touch in a collision pass, applied once mpcoll returns (the joint callback
/// has no access to the players): the touching collision's owner. Its only effect is a flag
/// nothing in a pass reads, so it happens in the original's order.
#[derive(Debug, Default)]
pub struct ClimbTouch(Option<Option<GObjId>>);

impl ClimbTouch {
    /// A collision joint's touch by the collision of `owner` (`onJointCollision`'s call).
    pub fn touched(&mut self, owner: Option<GObjId>) {
        self.0 = Some(owner);
    }

    /// `onJointCollision`: player 1's fighter touching (`Ground_GetP1Fighter` against the
    /// collision's GObj) marks the adventure's climb (map 10) touched this tick (`x14_b4`);
    /// without that climb (VS mode's), nothing.
    #[doc(alias = "onJointCollision@801FA7F0")]
    pub fn apply(self, game: &mut Game) {
        let Some(owner) = self.0 else { return };
        if super::ground::p1_fighter(game) != owner {
            return;
        }
        if let Some(gobj) = map_gobj(game, 10) {
            let g = ground_of(game, gobj);
            climb10(game, g).x14 |= IceMtClimbVars::B4;
        }
    }
}

/// The adventure's climb's setup (map 10), `stageGObj10_OnInit`: the list's first two fields
/// (the second below, the first above) set up in place, the lists' cursors at the fields' third
/// and the speeds' first, no push, braking or end, the speed at rest, and held until the
/// match starts.
#[doc(alias = "stageGObj10_OnInit")]
pub fn init_climb10(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let prm = param(r.game);
    let under = field_id_at(prm.list(r.data, p::FIELD_IXS, 1)) as i16;
    let upper = field_id_at(prm.list(r.data, p::FIELD_IXS, 0)) as i16;
    set_ids(r.game, g, Climb::Adventure, under, upper);
    place_climb_fields(r, g, Climb::Adventure);
    let v = climb10(r.game, g);
    v.x18 = 0;
    v.x1C = 2;
    v.x1A = 0;
    v.x14 &= !(IceMtClimbVars::B1 | IceMtClimbVars::B2 | IceMtClimbVars::B3 | IceMtClimbVars::B4);
    v.x16 = 0;
    v.x20 = 0.0;
    v.phase = 0;
    v.burst_count = 0;
    v.delay = 0;
    v.lerp_count = 0;
    v.idx = -1;
    v.cur = 0.0;
    v.x14 |= IceMtClimbVars::B0;
    setup::add_start_hook(r.game, gobj, CodeRef(CLIMB10_START_HOOK));
}

/// The adventure's climb's start hook, `stageGObj10_UnkGroundCallback`: the climb starts, and
/// every collision joint of the stage's 217 without a callback tells it when touched
/// (`onJointCollision`).
#[doc(alias = "stageGObj10_UnkGroundCallback")]
pub fn on_climb10_start(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    climb10(r.game, g).x14 &= !IceMtClimbVars::B0;
    let geometry = r.game.geometry_mut();
    for joint in 0..217 {
        if geometry.ledge_callback(joint).0.is_none() {
            geometry.set_ledge_callback(joint, Some(CollUser::Ground(g)), Some(CodeRef(ON_JOINT)));
        }
    }
}

/// The adventure's climb's process, `stageGObj10_GObjProc`, once the match has started.
///
/// Climbing, the speed's step comes from the speeds' list ([`climb_speed`]); the first fighter
/// touching the stage this tick above `ft_max_y`, out of hitstun, starts a push of `xA4` ticks,
/// which adds speed by `xA0` a tick up to `x9C` (and takes it off again by as much once it's
/// over). Braking (the fields' list run out), the speed eases off by `xA0` while the stopping
/// distance at that rate passes the upper field's height over `xA8` (never below `xA0`), and is
/// 0 once it's there. Both fields move by it, change, and tell the background; with no field
/// below left, the braking starts. The tick a speed change ends with the speeds' list's cursor
/// at `xA6`, the climb's end: the stage's `x6D8` cleared, the arena's opponents (14, 2 of them,
/// `gm_801674C4`) and the spawner stopped; after it, the arena's end (`x6D8` 1) sets the stage's
/// flag 0x10.
///
/// Braking, the end's test reads the speed step's result, which the braking skips: the machine
/// code compares a register (`r30`) its C never set (docs/melee-bugs.md). It
/// matters only when the cursor is at `xA6`, which braking begins with only if the fields ran out
/// within the last speed's change; that stops here by name.
#[doc(alias = "stageGObj10_GObjProc")]
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn proc_climb10(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let prm = param(r.game);
    let data = r.data;
    if climb10(r.game, g).x14 & IceMtClimbVars::B0 != 0 {
        return;
    }
    let mut done: Option<bool> = None;
    let y = if climb10(r.game, g).x14 & IceMtClimbVars::B2 == 0 {
        let (step, ret) = climb_speed(r, g, Climb::Adventure);
        done = Some(ret);
        if climb10(r.game, g).x14 & IceMtClimbVars::B4 != 0
            && let Some(f) = super::ground::p1_fighter(r.game)
        {
            let pos = r.game.ft_pos(f);
            let max_y = fp::frsp(f64::from(prm.h(data, p::FT_MAX_Y)));
            if climb10(r.game, g).x14 & IceMtClimbVars::B1 == 0
                && !r.game.ft_is_in_hitstun(f)
                && !(reg(pos.y) <= max_y)
            {
                let ticks = prm.h(data, p::PUSH_TICKS);
                let v = climb10(r.game, g);
                v.x14 |= IceMtClimbVars::B1;
                v.x16 = ticks;
            }
        }
        let push_step = reg(prm.f(data, p::PUSH_STEP));
        if climb10(r.game, g).x14 & IceMtClimbVars::B1 != 0 {
            let max = prm.f(data, p::PUSH_MAX);
            let v = climb10(r.game, g);
            v.x20 = mem(fp::fadds(reg(v.x20), push_step));
            if !(reg(v.x20) <= reg(max)) {
                v.x20 = max;
            }
            v.x16 = v.x16.wrapping_sub(1);
            if v.x16 == 0 {
                v.x14 &= !IceMtClimbVars::B1;
            }
        } else {
            let v = climb10(r.game, g);
            v.x20 = mem(fp::fsubs(reg(v.x20), push_step));
            if !(reg(v.x20) >= 0.0) {
                v.x20 = 0.0;
            }
        }
        mem(fp::fadds(reg(step), reg(climb10(r.game, g).x20)))
    } else {
        let upper = climb10(r.game, g).upper;
        let mgobj = map_gobj(r.game, i32::from(upper));
        assert!(mgobj.is_some(), "gricemt.c: 1219: mgobj");
        let j = jobj_of(r.game, mgobj.unwrap());
        assert!(j.is_some(), "gricemt.c: 1221: jobj");
        let ty = reg(r.game.objs.jobjs[j.unwrap()].translate.y);
        let dist = fp::fsubs(ty, fp::frsp(f64::from(prm.h(data, p::BRAKE_Y))));
        let step = reg(prm.f(data, p::PUSH_STEP));
        let v = climb10(r.game, g);
        if !(dist >= 0.0) {
            v.cur = 0.0;
        } else {
            let cur = reg(v.cur);
            let ratio = fp::fdivs(cur, step);
            let half = fp::fmuls(fp::fmuls(0.5, step), ratio);
            let stop = fp::fmsubs(cur, ratio, fp::fmuls(ratio, half));
            if !(stop <= dist) {
                v.cur = mem(fp::fsubs(cur, step));
                if !(reg(v.cur) >= step) {
                    v.cur = mem(step);
                }
            }
        }
        v.cur
    };
    let (under, upper) = climb_ids(r.game, g, Climb::Adventure);
    move_fields(r, under, upper, mem(fp::fneg(reg(y))));
    change_climb_field(r, g, Climb::Adventure);
    tell_bg(r.game, y);
    if climb10(r.game, g).under == -1 {
        climb10(r.game, g).x14 |= IceMtClimbVars::B2;
    }
    if climb10(r.game, g).x14 & IceMtClimbVars::B3 == 0 {
        let at_end = climb10(r.game, g).x1A == prm.h(data, p::END_IDX);
        let done = match done {
            Some(d) => d,
            None if at_end => panic!(
                "stale read: the adventure's climb's end (stageGObj10_GObjProc) tests r30, which                  its C never set while braking: the fields ran out within the last speed's change"
            ),
            None => false,
        };
        if done && at_end {
            climb10(r.game, g).x14 |= IceMtClimbVars::B3;
            super::ground::clear_x6d8(r.game);
            r.start_stage_event(14, 2, 2, 0, CodeRef(END_EVENT));
            zako::clear_enemies(r);
        }
    } else if super::ground::x6d8(r.game) == 1 {
        super::ground::set_flags(r.game, 0x10);
    }
    climb10(r.game, g).x14 &= !IceMtClimbVars::B4;
}

/// The climb's end's callback for the arena's event, `fn_801FA4CC`: on 1, the stage's `x6D8`
/// set to 1, and 1; else 0.
#[doc(alias = "fn_801FA4CC")]
pub fn end_event(game: &mut Game, num: i32) -> i32 {
    if num == 1 {
        super::ground::set_x6d8(game, num);
        1
    } else {
        0
    }
}

/// A Polar Bear's landing from its jump, `grIceMt_801FA6D8`: in the adventure, the climb's
/// burst for the parameters' `x3A` ticks.
#[doc(alias = "grIceMt_801FA6D8")]
pub fn bear_landed(game: &mut Game, data: &Archives) {
    if super::stage::stkind(game) != ADVENTURE {
        return;
    }
    if let Some(gobj) = map_gobj(game, 10) {
        let ticks = param(game).h(data, p::BEAR_BURST);
        let g = ground_of(game, gobj);
        climb10(game, g).burst_count = ticks;
    }
}

/// A field's process (maps 1, 3, 5 and 6), `stageGObj1_GObjProc` and its copies: its points of
/// interest when due, and its collision where its model stands.
#[doc(alias = "stageGObj1_GObjProc@801F7EE8")]
#[doc(alias = "stageGObj3_GObjProc@801F8364")]
#[doc(alias = "stageGObj5_GObjProc@801F87C8")]
#[doc(alias = "stageGObj6_GObjProc@801F89AC")]
pub fn proc_field(r: &mut Running, gobj: GObjId) {
    update_points(r, gobj);
    update_map_coll(r.game, r.data, gobj);
}

/// Field 2's process, `stageGObj2_GObjProc`: its two falling ices, then as the others.
#[doc(alias = "stageGObj2_GObjProc@801F815C")]
pub fn proc_field2(r: &mut Running, gobj: GObjId) {
    run_ice(r, gobj, 0);
    run_ice(r, gobj, 1);
    proc_field(r, gobj);
}

/// Field 4's process, `stageGObj4_GObjProc`: its falling ice, then as the others.
#[doc(alias = "stageGObj4_GObjProc@801F85C4")]
pub fn proc_field4(r: &mut Running, gobj: GObjId) {
    run_ice(r, gobj, 0);
    proc_field(r, gobj);
}

/// A field's destroy callback, `stageGObj1_Callback3` to `stageGObj6_Callback3`: its blocks'
/// items destroyed.
#[doc(alias = "stageGObj1_Callback3@801F7F1C")]
#[doc(alias = "stageGObj2_Callback3@801F81B4")]
#[doc(alias = "stageGObj3_Callback3@801F8398")]
#[doc(alias = "stageGObj4_Callback3@801F8608")]
#[doc(alias = "stageGObj5_Callback3")]
#[doc(alias = "stageGObj6_Callback3")]
pub fn destroy_field(r: &mut Running, gobj: GObjId, field: i32) {
    let g = ground_of(r.game, gobj);
    let n = FIELD_PARTS[(field - 1) as usize].blocks.len();
    for i in 0..n {
        if let Some(item) = r.game.grounds[g.0].u.icemt_field(field as u8).items[i] {
            material::destroy_item(r, item);
        }
    }
}

/// The background's setup (map 8), `stageGObj8_OnInit`: its animation, looping; its parts 1 to
/// 6 as the stage's guide points; at rest, drawn behind.
#[doc(alias = "stageGObj8_OnInit")]
pub fn init_bg(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
    anime::set_loop(r.game, gobj, 0, 7, true);
    let parts = [1, 2, 3, 4, 5, 6].map(|d| nth_joint(r.game, gobj, d));
    super::ground::set_guide_points(r.game, r.data, parts);
    r.game.grounds[g.0].u.icemt_bg().x0 = 0.0;
    r.game.grounds[g.0].x11_flags.b012 = 2;
}

/// The background's process, `stageGObj8_GObjProc`: its part 8 scrolls by 0.3 of the climb's
/// speed, wrapping within 3,500 (times the stage's scale) about the middle.
#[doc(alias = "stageGObj8_GObjProc")]
pub fn proc_bg(r: &mut Running, gobj: GObjId) {
    let g = ground_of(r.game, gobj);
    let x0 = r.game.grounds[g.0].u.icemt_bg().x0;
    let mul = fp::fmuls(0.300_000_011_920_929, reg(x0));
    let j = nth_joint(r.game, gobj, 8);
    assert!(j.is_some(), "gricemt.c: 1935: jobj");
    let j = j.unwrap();
    let s = || reg(super::ground::scale(r.game));
    let mut cur = fp::fsubs(reg(r.game.objs.jobjs[j].translate.y), mul);
    if cur > fp::fmuls(0.5, fp::fmuls(3500.0, s())) {
        cur = fp::fnmsubs(3500.0, s(), cur);
    } else if cur < fp::fmuls(0.5, fp::fneg(fp::fmuls(3500.0, s()))) {
        cur = fp::fmadds(3500.0, s(), cur);
    }
    let y = mem(cur);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
}

/// The wind Icicle Mountain blows on airborne items, `grIceMt_801FA728`: the climb's speed,
/// downward (the adventure's climb's on the stage kind 76), none without a climb.
#[doc(alias = "grIceMt_801FA728")]
pub fn wind(game: &Game) -> Vec3 {
    let map = if super::stage::stkind(game) == ADVENTURE {
        10
    } else {
        9
    };
    let Some(gobj) = map_gobj(game, map) else {
        return Vec3::ZERO;
    };
    let cur = match &game.grounds[ground_of(game, gobj).0].u.vars {
        super::GroundVars::IceMtScroll(v) => v.cur,
        super::GroundVars::IceMtClimb(v) => v.cur,
        super::GroundVars::Bytes(b) => {
            f32::from_bits(u32::from_be_bytes([b[0x10], b[0x11], b[0x12], b[0x13]]))
        }
        other => panic!("Icicle Mountain's climb's variables are {other:?}"),
    };
    Vec3::new(0.0, mem(fp::fneg(reg(cur))), 0.0)
}

/// Icicle Mountain's shadow check, `grIceMt_801FA900`: every shadow renders.
#[doc(alias = "grIceMt_801FA900")]
pub fn check_shadow_render() -> bool {
    true
}
