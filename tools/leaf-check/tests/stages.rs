//! The VS stages beyond the tournament six (milestone 4's stage tasks), against ssbm-rs's ports,
//! from real game states: every saved call of a stage's functions is imported, run on both
//! sides, and the whole state compared afterwards, in both float modes. Each process also runs
//! from its own captures with its object's variables changed (a cloud's state and count, which
//! creature passes), so the branches one match takes rarely run from real states too, and the
//! stage's callbacks no capture reached (the empty processes, the destroy callbacks, the demo
//! and load hooks, each object's setup) run from the captures of its other functions.
//!
//! The captures are game memory, kept out of the repository in `STAGES_CALLS` (default below),
//! one directory per stage, `oracle-stage-NAME`, beside the function lists its run used:
//! `NAME-funcs.txt` (`ADDR NAME` per line) and `done-NAME.txt` (every other function's
//! address). ssbm-rs's runner (`ssbm-run-rf`) made them from the stage's oracle match
//! (`$SSBM_RS_LOCAL/bench/oracle-m4/stage-NAME.slp`, [the oracle](../../docs/oracle.md#oracle-sets)),
//! played as the oracle traces it (`capture-NAME.sh` beside them):
//!
//! ```text
//! D=local/calls/stages
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-stage-yoshis-island-64 CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/yi64-funcs.txt LOCKSTEP_DONE=$D/done-yi64.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/oracle-m4/stage-yoshis-island-64.slp --fp hardware \
//!   --fields 14640 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use leaf_check::calls::{Prepare, Ret, check_calls_with, gobj_arg, peek_u32, poke_u32};
use melee_capture::import::Maps;
use melee_sim::game::code::{run_demo_init, run_event, run_fighter_cb, run_proc};
use melee_sim::game::ft::co::barrel_wait;
use melee_sim::game::gr::{
    castle, garden, icemt, inishie1, inishie2, kongo, oldkongo, oldyoshi, pura, rcruise, shrine,
    yorster,
};
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{kyasarin, kyasarinegg};
use melee_sim::game::state::Running;
use melee_sim::game::structs::HitCapsule;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/stages";

fn dir(set: &str) -> String {
    let base =
        std::env::var("STAGES_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS));
    format!("{base}/{set}")
}

fn f32_at(call: &Call, a: u32) -> f32 {
    f32::from_bits(peek_u32(call, a))
}

fn as_saved(_: &mut Call, _: usize) {}

/// A process or callback taking its stage object, run by its address on our side.
fn by_address(r: &mut Running, maps: &Maps, call: &Call) -> Ret {
    run_proc(r, gobj_arg(maps, call), CodeRef(call.addr));
    Ret::Nothing
}

/// A call made to run `target` instead, with the same object.
fn retarget(target: u32) -> impl Fn(&mut Call, usize) + Sync {
    move |call: &mut Call, _| call.addr = target
}

// Yoshi's Island N64 (`groldyoshi.c`).

const YI64: &str = "oracle-stage-yoshis-island-64";

/// groldyoshi.c's `yakumono_param`.
const YI64_PARAM: u32 = 0x804D_6A88;

/// The parameters' `s16` at `off`.
fn yi64_param(call: &Call, off: u32) -> u32 {
    let p = peek_u32(call, YI64_PARAM);
    let w = peek_u32(call, p + (off & !3));
    let shift = if off & 2 == 0 { 16 } else { 0 };
    (w >> shift) & 0xFFFF
}

/// The stage object's variables (`Ground::u`) in a call made with the object in r3.
fn vars(call: &Call) -> u32 {
    peek_u32(call, call.regs.gpr[3] + 0x2C) + 0xC4
}

/// Every cloud's bit fields set to `state`, `touched` and `count`.
fn clouds(call: &mut Call, state: u32, touched: bool, count: u32) {
    let v = vars(call);
    for i in 0..3 {
        let at = v + 0x14 * i;
        let w = peek_u32(call, at) & 0x7FFFF;
        poke_u32(
            call,
            at,
            w | (state & 0xF) << 28 | u32::from(touched) << 27 | (count & 0xFF) << 19,
        );
    }
}

#[test]
fn oldyoshi_clouds() {
    let d = dir(YI64);
    let f = "grOldYoshi_8020EC10";
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        oldyoshi::proc_clouds(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let prepares: [&Prepare; 9] = [
        &as_saved,
        // Stood on, at its count's limit and past it: sinking, then puffing.
        &|c, _| clouds(c, 0, true, yi64_param(c, 0)),
        &|c, _| clouds(c, 0, true, yi64_param(c, 0) + 1),
        // Puffing as its collision goes, gone for long enough, coming back shown, and its
        // collision back.
        &|c, _| clouds(c, 1, false, yi64_param(c, 0x10)),
        &|c, _| clouds(c, 2, false, yi64_param(c, 2) + 1),
        &|c, _| clouds(c, 3, false, 0),
        &|c, _| clouds(c, 3, false, yi64_param(c, 0x12)),
        // A count that wraps, and a state past the four.
        &|c, _| clouds(c, 0, true, 0xFF),
        &|c, _| clouds(c, 5, true, 7),
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &ours);
    }
}

#[test]
fn oldyoshi_guest() {
    let d = dir(YI64);
    let f = "grOldYoshi_8020F088";
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        oldyoshi::proc_guest(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let set = |c: &mut Call, wait: i16, part: i16| {
        let v = vars(c);
        poke_u32(c, v, (wait as u16 as u32) << 16 | part as u16 as u32);
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        // The wait over: a creature drawn; still waiting.
        &|c, _| set(c, -1, -1),
        &|c, _| set(c, 12, -1),
        // Each creature passing.
        &|c, k| set(c, 0, (k % 5) as i16 + 1),
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &ours);
    }
}

#[test]
fn oldyoshi_stage_and_touch() {
    let d = dir(YI64);
    check_calls_with(&d, "grOldYoshi_8020EA64", 1, &as_saved, &by_address);
    // A fighter on each cloud's joint, on another joint, and an object of another kind.
    let touch = |r: &mut Running, maps: &Maps, call: &Call| {
        let kind = peek_u32(call, call.regs.gpr[5] + 0x34) >> 27 & 0xF;
        let cloud = maps.grounds.get(&call.regs.gpr[3]).copied();
        oldyoshi::cloud_touched(r.game, cloud, call.regs.gpr[4] as i32, kind);
        Ret::Nothing
    };
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, k| c.regs.gpr[4] = (k % 4) as u32,
        &|c, _| {
            let at = c.regs.gpr[5] + 0x34;
            let w = peek_u32(c, at);
            poke_u32(c, at, (w & !(0xF << 27)) | 2 << 27);
        },
    ];
    for p in prepares {
        check_calls_with(&d, "fn_8020F2A8", 1, p, &touch);
    }
}

#[test]
fn oldyoshi_shadow_and_approach() {
    let d = dir(YI64);
    let shadow = |r: &mut Running, maps: &Maps, call: &Call| {
        let p = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        let j = maps.jobjs.get(&call.regs.gpr[5]).copied();
        Ret::Int(u32::from(oldyoshi::check_shadow_render(
            r.game, r.data, pos, j,
        )))
    };
    // The fighter's feet put just above and below the point five units over the joint (the
    // joint's matrix's place, when it's up to date) and at it.
    let around = |c: &mut Call, k: usize| {
        let (p, j) = (c.regs.gpr[3], c.regs.gpr[5]);
        let y = f32_at(c, j + 0x44 + 0x1C) - 5.0;
        let dy = [0.25f32, -0.25, 0.0, 0.75, -0.75][k % 5];
        poke_u32(c, p + 4, (y + dy).to_bits());
    };
    for p in [&as_saved as &Prepare, &around] {
        check_calls_with(&d, "grOldYoshi_8020F40C", 1, p, &shadow);
    }
    let approach = |_: &mut Running, _: &Maps, call: &Call| {
        let f = |i: usize| call.regs.fpr[i].ps0;
        Ret::Float(oldyoshi::approach(f(1), f(2), f(3), f(4), f(5), f(6)))
    };
    check_calls_with(&d, "grOldYoshi_8020F31C", 1, &as_saved, &approach);
}

/// The cloud speed's step on inputs spread over its range and special values, on inputs around
/// the point where it starts braking (the distance left between half and one and a half times
/// what a stop at its rate takes, toward the target or away), and where the distance left is
/// exactly that, in both float modes.
#[test]
fn oldyoshi_approach_inputs() {
    use leaf_check::{arg, bits};
    let exact = [
        [2.0f32, 1.0, 0.0, 2.0, 10.0, -10.0],
        [-2.0, 1.0, 0.0, -2.0, 10.0, -10.0],
        [1.0, 0.5, 3.0, 4.0, 10.0, -10.0],
        [0.0, 1.0, 5.0, 5.0, 10.0, -10.0],
    ];
    let mut k = 0usize;
    let mut rng = leaf_check::Rand(0x0D0D);
    leaf_check::each_mode(4000, 20.0, |ctx, floats, mode| {
        let i = k % 4000;
        k += 1;
        let v: Vec<f32> = if let Some(e) = exact.get(i) {
            e.to_vec()
        } else if i.is_multiple_of(2) {
            let accel = rng.range(0.05, 2.0);
            let speed = rng.range(-5.0, 5.0);
            let stop = speed * speed / (2.0 * accel);
            let left = stop * rng.range(0.5, 1.5);
            let pos = rng.range(-30.0, 30.0);
            let toward = if rng.below(4) == 0 { -1.0 } else { 1.0 };
            let target = pos + left * speed.signum() * toward;
            vec![speed, accel, pos, target, 4.0, -4.0]
        } else {
            (0..6).map(|_| floats.next()).collect()
        };
        let theirs = bits(ssbm_types::fns::grOldYoshi_8020F31C(
            ctx,
            arg(v[0]),
            arg(v[1]),
            arg(v[2]),
            arg(v[3]),
            arg(v[4]),
            arg(v[5]),
        ));
        let ours = bits(oldyoshi::approach(
            arg(v[0]),
            arg(v[1]),
            arg(v[2]),
            arg(v[3]),
            arg(v[4]),
            arg(v[5]),
        ));
        assert_eq!(ours, theirs, "{mode:?}: {v:?}");
    });
}

#[test]
fn oldyoshi_setup() {
    let d = dir(YI64);
    let event = |r: &mut Running, _: &Maps, call: &Call| {
        run_event(r, call.addr);
        Ret::Nothing
    };
    check_calls_with(&d, "grOldYoshi_8020E79C", 1, &as_saved, &event);
    check_calls_with(&d, "grOldYoshi_8020E828", 1, &as_saved, &event);
    // Each object made from the setup's first creation.
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let g = oldyoshi::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        let _ = g;
        Ret::Nothing
    };
    for map in 0..6u32 {
        check_calls_with(
            &d,
            "grOldYoshi_8020E854",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "grOldYoshi_8020E978",
        "grOldYoshi_8020E9F0",
        "grOldYoshi_8020EA8C",
        "grOldYoshi_8020EAFC",
        "grOldYoshi_8020EFCC",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
}

/// The stage's callbacks no capture reaches, from the clouds' captures: the empty processes
/// and destroy callbacks on the clouds' object, and the demo and load hooks.
#[test]
fn oldyoshi_unreached_callbacks() {
    let d = dir(YI64);
    let from = "grOldYoshi_8020EC10";
    for target in [
        leaf_check::sym("grOldYoshi_8020E970"),
        leaf_check::sym("grOldYoshi_8020E974"),
        leaf_check::sym("grOldYoshi_8020E9E8"),
        leaf_check::sym("grOldYoshi_8020E9EC"),
        leaf_check::sym("grOldYoshi_8020EA88"),
        leaf_check::sym("grOldYoshi_8020EAF4"),
        leaf_check::sym("grOldYoshi_8020EAF8"),
        leaf_check::sym("grOldYoshi_8020EFC8"),
        leaf_check::sym("grOldYoshi_8020F2A4"),
    ] {
        check_calls_with(&d, from, 1, &retarget(target), &by_address);
    }
    let event = |r: &mut Running, _: &Maps, call: &Call| {
        run_event(r, call.addr);
        Ret::Nothing
    };
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grOldYoshi_8020E824")),
        &event,
    );
    let demo = |r: &mut Running, _: &Maps, call: &Call| {
        run_demo_init(r, call.addr, call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grOldYoshi_8020E798")),
        &demo,
    );
}

// The hazard stages (milestone 4's task 12). Their captures are in `STAGES_HAZARD_CALLS`
// (default `$SSBM_RS_LOCAL/calls/stages-hazard`), made as above by `capture.sh KEY MATCH`
// beside them (the same command with `KEY-funcs.txt` and `done-KEY.txt`).

const HAZARD_CALLS: &str = "calls/stages-hazard";

fn hazard_dir(set: &str) -> String {
    let base = std::env::var("STAGES_HAZARD_CALLS")
        .unwrap_or_else(|_| melee_local_data::ssbm_rs_local(HAZARD_CALLS));
    format!("{base}/{set}")
}

/// `stage_info`.
const STAGE_INFO: u32 = 0x8049_E6C8;

/// The variables (`Ground::u`) of the stage object of map `map`.
fn map_vars(call: &Call, map: u32) -> u32 {
    let g = peek_u32(call, STAGE_INFO + 0x180 + 4 * map);
    peek_u32(call, g + 0x2C) + 0xC4
}

fn poke_u8(call: &mut Call, a: u32, v: u8) {
    let w = peek_u32(call, a & !3);
    let shift = 24 - 8 * (a & 3);
    poke_u32(call, a & !3, (w & !(0xFF << shift)) | u32::from(v) << shift);
}

/// [`check_calls_with`] with `OSReport` stood in for on the reference's side: the hazard stages'
/// code reports to the debugger (Kraid's hand, the crane's target), which the reference's
/// hardware doesn't take, and melee-hd leaves out.
fn check_hazard(
    dir: &str,
    func: &str,
    min: usize,
    prepare: &Prepare,
    ours: &leaf_check::calls::Ours,
) {
    let stubs = [(leaf_check::sym("OSReport"), None)];
    let setup = leaf_check::calls::Setup {
        stubs: &stubs,
        ..Default::default()
    };
    leaf_check::calls::check_calls_in(dir, func, min, prepare, &setup, ours);
}

/// [`check_hazard`] for code that draws: the reference's hardware registers take the drawing.
fn check_hazard_gx(
    dir: &str,
    func: &str,
    min: usize,
    prepare: &Prepare,
    ours: &leaf_check::calls::Ours,
) {
    let stubs = [(leaf_check::sym("OSReport"), None)];
    let setup = leaf_check::calls::Setup {
        stubs: &stubs,
        gx: true,
        ..Default::default()
    };
    leaf_check::calls::check_calls_in(dir, func, min, prepare, &setup, ours);
}

fn by_event(r: &mut Running, _: &Maps, call: &Call) -> Ret {
    run_event(r, call.addr);
    Ret::Nothing
}

fn by_demo(r: &mut Running, _: &Maps, call: &Call) -> Ret {
    run_demo_init(r, call.addr, call.regs.gpr[3] as i32);
    Ret::Nothing
}

// Brinstar Depths (`grkraid.c`).

const KRAID: &str = "oracle-stage-brinstar-depths";

#[test]
fn kraid_setup() {
    use melee_sim::game::gr::kraid;
    let d = hazard_dir(KRAID);
    check_hazard(&d, "grKraid_OnInit", 1, &as_saved, &by_event);
    check_hazard(&d, "grKraid_OnStart", 1, &as_saved, &by_event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = kraid::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..5u32 {
        check_hazard(
            &d,
            "grKraid_801FE0C4",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "grKraid_801FE1EC",
        "grKraid_801FE360",
        "grKraid_801FE3B4",
        "grKraid_801FE818",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
}

/// The rock's variables set: state, rumbling part, direction, speed, step and delay.
fn rock(c: &mut Call, state: u8, part: u8, dir: f32, speed: f32, step: f32, delay: i32) {
    let v = map_vars(c, 3);
    poke_u8(c, v, state);
    poke_u8(c, v + 1, part);
    poke_u32(c, v + 4, dir.to_bits());
    poke_u32(c, v + 8, speed.to_bits());
    poke_u32(c, v + 0xC, step.to_bits());
    poke_u32(c, v + 0x10, delay as u32);
}

#[test]
fn kraid_rock() {
    let d = hazard_dir(KRAID);
    let prepares: [&Prepare; 9] = [
        &as_saved,
        // A swipe landed either way: the turn starts.
        &|c, _| rock(c, 0, 0, 1.0, 0.0, 0.0, 0),
        &|c, _| rock(c, 0, 1, -1.0, 0.0, 0.0, 0),
        // Turning: still in its delay, easing off, and at its end (speed below a step), each
        // way.
        &|c, _| rock(c, 1, 0, 1.0, 2.5, 0.05, 7),
        &|c, _| rock(c, 1, 1, 1.0, 2.5, 0.05, 0),
        &|c, _| rock(c, 1, 0, -1.0, -2.5, -0.05, 0),
        &|c, _| rock(c, 1, 0, 1.0, 0.04, 0.05, 0),
        // Eased off to exactly a step: it turns once more.
        &|c, _| rock(c, 1, 0, 1.0, 0.5, 0.25, 0),
        // A state past the two.
        &|c, _| rock(c, 3, 1, 0.0, 0.0, 0.0, 0),
    ];
    for p in prepares {
        check_hazard(&d, "grKraid_801FE440", 1, p, &by_address);
    }
}

/// Kraid's variables set: state, animation, last place, swipe side and tick count; the rock's
/// direction too.
fn kraid_state(c: &mut Call, state: u8, anim: u8, last: u8, side: f32, t: i32, rock_dir: f32) {
    let v = vars(c);
    poke_u8(c, v, state);
    poke_u8(c, v + 1, anim);
    poke_u8(c, v + 2, last);
    poke_u32(c, v + 8, side.to_bits());
    poke_u32(c, v + 0xC, t as u32);
    let r = map_vars(c, 3);
    poke_u32(c, r + 4, rock_dir.to_bits());
}

#[test]
fn kraid_kraid() {
    let d = hazard_dir(KRAID);
    let f = "grKraid_801FEA00";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Waiting: over with the rock still or turning, and counting down.
    for (t, dir) in [(0, 0.0f32), (0, 1.0), (-3, 0.0), (12, 0.0)] {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| kraid_state(c, 0, 3, 2, 0.0, t, dir),
            &by_address,
        );
    }
    // Risen, idling (each idle at its sounds' ticks), swiping (each side, around the window's
    // ends and the hit, the rock still or turning), sinking, and a state past them.
    let mut cases: Vec<(u8, u8, f32, i32, f32)> = vec![(1, 3, 0.0, 0, 0.0)];
    for (anim, t) in [
        (0, 80),
        (1, 45),
        (2, 55),
        (2, 161),
        (2, 246),
        (0, 3),
        (1, 80),
    ] {
        cases.push((2, anim, 1.0, t, 0.0));
    }
    for t in [122, 123, 129, 130, 138, 139, 146, 147, 157, 158, 169, 170] {
        cases.push((3, 5, 0.0, t, 0.0));
        cases.push((3, 6, 1.0, t, 0.0));
    }
    cases.push((3, 5, 0.0, 158, 1.0));
    cases.push((3, 6, 1.0, 130, -1.0));
    cases.push((4, 4, 0.0, 9, 0.0));
    cases.push((7, 4, 0.0, 9, 0.0));
    for (state, anim, side, t, dir) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| kraid_state(c, state, anim, 1, side, t, dir),
            &by_address,
        );
    }
}

#[test]
fn kraid_helpers() {
    use melee_sim::game::gr::kraid;
    let d = hazard_dir(KRAID);
    // Where he rises: after a turn (any place but the last), else the furthest left or right.
    let place = |r: &mut Running, maps: &Maps, call: &Call| {
        kraid::place(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, k| {
            let v = vars(c);
            poke_u8(c, v + 5, 1);
            poke_u8(c, v + 2, (k % 6) as u8);
        },
        &|c, _| poke_u8(c, vars(c) + 5, 0),
    ];
    for p in prepares {
        check_hazard(&d, "grKraid_801FEE54", 1, p, &place);
    }
    let set_anim = |r: &mut Running, maps: &Maps, call: &Call| {
        kraid::set_anim(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    for p in [&as_saved as &Prepare, &|c, k| {
        c.regs.gpr[4] = (k % 7) as u32
    }] {
        check_hazard(&d, "grKraid_801FF068", 1, p, &set_anim);
    }
    let reached = |r: &mut Running, maps: &Maps, call: &Call| {
        let hit = kraid::swipe_reached(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Int(u32::from(hit))
    };
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &|c, _| poke_u32(c, vars(c) + 0xC, 0x9D),
        &|c, _| poke_u32(c, vars(c) + 0xC, 0x81),
        &|c, _| {
            poke_u32(c, vars(c) + 0xC, 0x9E);
            c.regs.gpr[4] = 1;
        },
        &|c, _| {
            poke_u32(c, vars(c) + 0xC, 0x82);
            c.regs.gpr[4] = 0;
        },
    ];
    for p in prepares {
        check_hazard(&d, "grKraid_801FF0E0", 1, p, &reached);
    }
    // A swipe landing on a still rock, either way, on one already turning, and with the hand
    // where the speed falls between its bounds (the saved swipes all reach one).
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let hand = maps.jobjs.get(&call.regs.gpr[3]).copied();
        kraid::swipe_hit(r, hand, call.regs.fpr[1].ps0);
        Ret::Nothing
    };
    // The hand put where the speed falls between the bounds (its matrix's x).
    let open_bounds = |c: &mut Call, _: usize| {
        let p = peek_u32(c, KRAID_PARAM);
        let (lo, hi) = (f32_at(c, p + 0xC), f32_at(c, p + 0x10));
        let x = 66.0 * (lo + hi) / hi;
        poke_u32(c, c.regs.gpr[3] + 0x44 + 0xC, x.to_bits());
    };
    let prepares: [&Prepare; 5] = [
        &open_bounds,
        &as_saved,
        &|c, _| c.regs.fpr[1].ps0 = -c.regs.fpr[1].ps0,
        &|c, _| {
            let r = map_vars(c, 3);
            poke_u32(c, r + 4, 1.0f32.to_bits());
        },
        &|c, _| {
            let r = map_vars(c, 3);
            poke_u32(c, r + 4, (-1.0f32).to_bits());
        },
    ];
    for p in prepares {
        check_hazard(&d, "grKraid_801FE6D8", 1, p, &hit);
    }
}

/// grkraid.c's `yakumono_param`.
const KRAID_PARAM: u32 = 0x804D_6A08;

/// `HSD_PadMasterStatus[1].trigger`.
const MASTER_TRIGGER_2: u32 = 0x804C_1FAC + 0x44 + 8;

/// Kraid's copy's debugging button: A on the second controller's master status steps its
/// animation, wrapping past the seventh; anything else leaves it.
#[test]
fn kraid_copy_button() {
    let d = hazard_dir(KRAID);
    let anim = 0x804D_6A0C;
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &|c, k| {
            poke_u32(c, MASTER_TRIGGER_2, 0x100);
            poke_u32(c, anim, (k % 7) as u32);
        },
        &|c, _| {
            poke_u32(c, MASTER_TRIGGER_2, 0x100);
            poke_u32(c, anim, 5);
        },
        &|c, _| {
            poke_u32(c, MASTER_TRIGGER_2, 0x100);
            poke_u32(c, anim, 6);
        },
        &|c, _| poke_u32(c, MASTER_TRIGGER_2, 0x200),
    ];
    for p in prepares {
        check_hazard(&d, "grKraid_801FE2D0", 1, p, &by_address);
    }
}

/// The callbacks no capture reaches, from the rock's captures: the empty processes and destroy
/// callbacks, the load and demo hooks, and the shadow check (every joint takes shadows).
#[test]
fn kraid_unreached_callbacks() {
    use melee_sim::game::gr::kraid;
    let d = hazard_dir(KRAID);
    let from = "grKraid_801FE440";
    for name in [
        "grKraid_801FE1E4",
        "grKraid_801FE1E8",
        "grKraid_801FE35C",
        "grKraid_801FE3AC",
        "grKraid_801FE3B0",
        "grKraid_801FE6D4",
        "grKraid_801FF14C",
        "grKraid_801FF150",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grKraid_OnLoad"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grKraid_OnDemoInit"));
    check_hazard(&d, from, 1, &demo, &by_demo);
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(kraid::check_shadow_render()));
    let to_shadow = retarget(leaf_check::sym("grKraid_OnCheckShadowRender"));
    check_hazard(&d, from, 1, &to_shadow, &shadow);
}

// Fourside (`grfourside.c`).

const FOURSIDE: &str = "oracle-stage-fourside";

#[test]
fn fourside_setup() {
    use melee_sim::game::gr::fourside;
    let d = hazard_dir(FOURSIDE);
    check_hazard(&d, "grFourside_801F2D10", 1, &as_saved, &by_event);
    check_hazard(&d, "grFourside_801F2DA4", 1, &as_saved, &by_event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = fourside::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..7u32 {
        check_hazard(
            &d,
            "grFourside_801F2DD0",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "stageGObj2_OnInit",
        "grFourside_801F2F34",
        "grFourside_801F30F0",
        "grFourside_801F3154",
        "grFourside_801F37FC",
        "grFourside_801F3C40",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
    // The guide points, from the city's setup's call.
    let guide = |r: &mut Running, maps: &Maps, call: &Call| {
        let j = |i: usize| maps.jobjs.get(&call.regs.gpr[3 + i]).copied();
        let joints = [j(0), j(1), j(2), j(3), j(4), j(5)];
        melee_sim::game::gr::ground::set_guide_points(r.game, r.data, joints);
        Ret::Nothing
    };
    check_hazard(&d, "Ground_801C4E70", 1, &as_saved, &guide);
}

/// The crane's variables set: state, flags, count, and the girder's rest, place, target,
/// starting speed, speed and pull.
#[allow(clippy::too_many_arguments)]
fn crane(c: &mut Call, state: u8, flags: u8, t: i32, rest: f32, y: f32, to: f32, v: [f32; 3]) {
    let a = vars(c);
    poke_u8(c, a, state);
    poke_u8(c, a + 1, flags);
    poke_u32(c, a + 4, t as u32);
    for (k, f) in [rest, y, to, v[0], v[1], v[2]].into_iter().enumerate() {
        poke_u32(c, a + 8 + 4 * k as u32, f.to_bits());
    }
}

#[test]
fn fourside_crane() {
    let d = hazard_dir(FOURSIDE);
    let f = "grFourside_801F3274";
    let z = [0.0f32; 3];
    let prepares: [&Prepare; 23] = [
        &as_saved,
        // Waiting, over and not: swinging over or back.
        &|c, _| crane(c, 0, 0, 0, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 0, 0x80, 0, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 0, 0, 5, 40.0, 40.0, 40.0, z),
        // Swinging, at its end and before it, each way.
        &|c, _| crane(c, 1, 0, 300, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 1, 0, 299, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 2, 0x80, 300, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 2, 0x80, 12, 40.0, 40.0, 40.0, z),
        // The girder's wait, then moving down (above its rest) or up.
        &|c, _| crane(c, 3, 0, 0, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 4, 0, 0, 40.0, 55.0, 40.0, z),
        &|c, _| crane(c, 4, 0x40, 0, 40.0, 25.0, 40.0, z),
        &|c, _| crane(c, 4, 0, 0, 40.0, 40.0, 40.0, z),
        &|c, _| crane(c, 4, 0, 7, 40.0, 40.0, 40.0, z),
        // Going down and up, arriving and not.
        &|c, _| crane(c, 5, 0x40, 0, 40.0, 30.0, 29.9, z),
        &|c, _| crane(c, 5, 0x40, 0, 40.0, 30.0, 10.0, z),
        // Arriving exactly: one step down lands on its target.
        &|c, _| {
            let spd = f32_at(c, peek_u32(c, FOURSIDE_PARAM) + 0x2C);
            crane(c, 5, 0x40, 0, 40.0, 30.0, 30.0 - spd, z)
        },
        &|c, _| crane(c, 6, 0, 0, 40.0, 30.0, 30.1, z),
        &|c, _| crane(c, 6, 0, 0, 40.0, 30.0, 60.0, z),
        // Bobbing about its target: pulled either way, turning as it passes, and settled.
        &|c, _| crane(c, 7, 0, 0, 40.0, 30.5, 30.0, [1.0, 0.4, -0.1]),
        &|c, _| crane(c, 7, 0, 0, 40.0, 29.5, 30.0, [1.0, -0.4, 0.1]),
        &|c, _| crane(c, 7, 0, 0, 40.0, 30.0, 30.0, [1.0, 0.2, 1.5]),
        &|c, _| crane(c, 7, 0, 0, 40.0, 30.0, 30.0, [1.0, 0.2, -1.0]),
        // A state past them.
        &|c, _| crane(c, 9, 0, 0, 40.0, 40.0, 40.0, z),
    ];
    for p in prepares {
        check_hazard(&d, f, 1, p, &by_address);
    }
}

/// The UFO's variables set: state, building, waits counted, sound state, count and roof touches.
fn ufo(c: &mut Call, state: u8, building: u8, waited: u8, sound: u8, t: i32, roof: i32) {
    let a = vars(c);
    poke_u8(c, a, state);
    poke_u8(c, a + 1, building);
    poke_u8(c, a + 2, waited);
    poke_u8(c, a + 3, sound);
    poke_u32(c, a + 4, t as u32);
    poke_u32(c, a + 8, roof as u32);
}

/// The helicopter's variables set: state, sound flag and count.
/// grfourside.c's `yakumono_param`.
const FOURSIDE_PARAM: u32 = 0x804D_69D8;

/// The `depth`th joint below stage object `gobj`'s root, as `Ground_801C3FA4` finds it.
fn nth_jobj(c: &Call, gobj: u32, depth: u32) -> u32 {
    let mut j = peek_u32(c, peek_u32(c, gobj + 0x28) + 0x10);
    for _ in 0..depth {
        let child = peek_u32(c, j + 0x10);
        if peek_u32(c, j + 0x14) & 0x1000 == 0 && child != 0 {
            j = child;
            continue;
        }
        let mut up = j;
        loop {
            let next = peek_u32(c, up + 8);
            if next != 0 {
                j = next;
                break;
            }
            up = peek_u32(c, up + 0xC);
        }
    }
    j
}

fn heli(c: &mut Call, map: u32, state: u8, flag: u8, t: i32) {
    let a = map_vars(c, map);
    poke_u8(c, a, state);
    poke_u8(c, a + 1, flag);
    poke_u32(c, a + 4, t as u32);
}

#[test]
fn fourside_ufo() {
    let d = hazard_dir(FOURSIDE);
    let f = "grFourside_801F3894";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Waiting: not yet, its wait over with no one on the roof, with someone on it, and after
    // its challenge's count of waits; each building last, the helicopter about or not.
    let mut cases: Vec<(u8, u8, u8, i32, i32, u8, i32)> = vec![(0, 0xFF, 0, 9, 0, 0, 0)];
    for building in [0xFF, 0, 1, 2] {
        for (waited, roof) in [(0, 0), (0, 1), (200, 0), (200, 2)] {
            cases.push((0, building, waited, 0, roof, 0, 0));
            cases.push((0, building, waited, 0, roof, 1, 0));
        }
    }
    // Staying (over and not, its animation rewound or not) and leaving (at its end and not).
    for t in [0, 5] {
        cases.push((2, 1, 0, t, 0, 0, 0));
    }
    for t in [59, 60, 61] {
        cases.push((4, 2, 0, t, 0, 0, 0));
    }
    cases.push((3, 2, 0, 0, 0, 0, 0));
    for (state, building, waited, t, roof, heli_state, heli_t) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| {
                ufo(c, state, building, waited, 0, t, roof);
                heli(c, 3, heli_state, 0, heli_t);
            },
            &by_address,
        );
    }
    let frame = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::fourside::frame_ufo(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grFourside_801F3B70", 1, &as_saved, &frame);
    // Arriving at each building, the camera's top lowered so the UFO is low enough.
    for b in 0..3u8 {
        check_hazard(
            &d,
            "grFourside_801F3B70",
            1,
            &move |c, _| poke_u8(c, vars(c) + 1, b),
            &frame,
        );
    }
}

#[test]
fn fourside_heli() {
    let d = hazard_dir(FOURSIDE);
    let f = "grFourside_801F3CC8";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Each state with its count over or not, the UFO out (over a building but the third) or
    // not.
    for state in 0..5u8 {
        for t in [0, 3, 700, 701] {
            for (ufo_state, building) in [(0u8, 0u8), (1, 0), (2, 2), (2, 1)] {
                check_hazard(
                    &d,
                    f,
                    1,
                    &move |c, _| {
                        heli(c, 3, state, (t & 1) as u8, t);
                        let u = map_vars(c, 5);
                        poke_u8(c, u, ufo_state);
                        poke_u8(c, u + 1, building);
                    },
                    &by_address,
                );
            }
        }
    }
    // Flying in, the helicopter just above, at and below its sound's height: its part's matrix
    // set and left clean so the position read takes it.
    for dy in [50.5f32, 50.0, 49.5] {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| {
                heli(c, 3, 1, 0, 5);
                poke_u8(c, map_vars(c, 5), 0);
                let top = f32_at(c, STAGE_INFO + 8) + f32_at(c, STAGE_INFO + 0x14);
                let j = nth_jobj(c, peek_u32(c, STAGE_INFO + 0x180 + 4 * 3), 2);
                poke_u32(c, j + 0x14, peek_u32(c, j + 0x14) & !0x40);
                poke_u32(c, j + 0x60, (top + dy).to_bits());
            },
            &by_address,
        );
    }
    let about = |r: &mut Running, _: &Maps, _: &Call| {
        Ret::Int(u32::from(melee_sim::game::gr::fourside::heli_about(r.game)))
    };
    check_hazard(&d, "grFourside_801F3F10", 1, &as_saved, &about);
    for (state, flag, t) in [
        (1u8, 0u8, 5),
        (1, 1, 5),
        (3, 0, 700),
        (3, 0, 701),
        (0, 0, 0),
    ] {
        check_hazard(
            &d,
            "grFourside_801F3F10",
            1,
            &move |c, _| heli(c, 3, state, flag, t),
            &about,
        );
    }
}

#[test]
fn fourside_roof_and_city() {
    let d = hazard_dir(FOURSIDE);
    check_hazard(&d, "grFourside_801F3078", 1, &as_saved, &by_address);
    // The roof touched by a fighter, an item and another kind of object.
    let touch = |r: &mut Running, maps: &Maps, call: &Call| {
        let kind = peek_u32(call, call.regs.gpr[5] + 0x34) >> 27 & 0xF;
        let u = peek_u32(call, STAGE_INFO + 0x180 + 4 * 5);
        let ufo = maps
            .grounds
            .get(&peek_u32(call, u + 0x2C))
            .copied()
            .expect("the UFO's object");
        melee_sim::game::gr::fourside::touch(&mut r.game.grounds, Some(ufo), kind);
        Ret::Nothing
    };
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, k| {
            let at = c.regs.gpr[5] + 0x34;
            let w = peek_u32(c, at);
            poke_u32(c, at, (w & !(0xF << 27)) | ((k % 4) as u32) << 27);
        },
        &|c, _| {
            let u = map_vars(c, 5);
            poke_u32(c, u + 8, 7);
        },
    ];
    for p in prepares {
        check_hazard(&d, "grFourside_801F30A0", 1, p, &touch);
    }
    // The skyline turning with the camera, in the display pass and another, and the camera
    // looking almost straight down.
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::fourside::render_city(
            r,
            gobj_arg(maps, call),
            call.regs.gpr[4] as i32,
        );
        Ret::Nothing
    };
    for p in [&as_saved as &Prepare, &|c, _| c.regs.gpr[4] ^= 3] {
        check_hazard_gx(&d, "fn_801F3F74", 1, p, &render);
    }
    let shadow = |r: &mut Running, maps: &Maps, call: &Call| {
        let p = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        let j = maps.jobjs.get(&call.regs.gpr[5]).copied();
        Ret::Int(u32::from(
            melee_sim::game::gr::fourside::check_shadow_render(r.game, r.data, pos, j),
        ))
    };
    let around = |c: &mut Call, k: usize| {
        let (p, j) = (c.regs.gpr[3], c.regs.gpr[5]);
        let y = f32_at(c, j + 0x44 + 0x1C);
        let dy = [0.25f32, -0.25, 0.0][k % 3];
        poke_u32(c, p + 4, (y + dy).to_bits());
    };
    for p in [&as_saved as &Prepare, &around] {
        check_hazard(&d, "grFourside_801F41E8", 1, p, &shadow);
    }
    // The stage's fog fitted to the camera, as saved and with the camera's eye turned about.
    let fog = |r: &mut Running, maps: &Maps, call: &Call| {
        let c = *maps.cobjs.get(&call.regs.gpr[3]).expect("a camera");
        r.stage_fog_update(c);
        Ret::Nothing
    };
    check_hazard(&d, "Ground_801C4FAC", 1, &as_saved, &fog);
    // The guide points all at the origin and the eye at a distance from it: inside both
    // bounds, inside the far one only, and past both.
    for dist in [3.0f32, 7.0, 12.0] {
        check_hazard(
            &d,
            "Ground_801C4FAC",
            1,
            &move |c, _| {
                for k in 0..18 {
                    poke_u32(c, STAGE_INFO + 0x130 + 4 * k, 0);
                }
                let eye = peek_u32(c, c.regs.gpr[3] + 0x24);
                for (k, v) in [0.0f32, 0.0, dist].into_iter().enumerate() {
                    poke_u32(c, eye + 0xC + 4 * k as u32, v.to_bits());
                }
            },
            &fog,
        );
    }
}

/// The callbacks no capture reaches, from the city's process's captures: the empty processes
/// and destroy callbacks, and the load and demo hooks.
#[test]
fn fourside_unreached_callbacks() {
    let d = hazard_dir(FOURSIDE);
    let from = "grFourside_801F3078";
    for name in [
        "grFourside_801F2EF0",
        "grFourside_801F2EF4",
        "grFourside_801F2F2C",
        "grFourside_801F2F30",
        "grFourside_801F309C",
        "grFourside_801F314C",
        "grFourside_801F3150",
        "grFourside_801F37F8",
        "grFourside_801F3B6C",
        "grFourside_801F3F0C",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grFourside_801F2DA0"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grFourside_801F2D0C"));
    check_hazard(&d, from, 1, &demo, &by_demo);
}

// Great Bay (`grgreatbay.c`) and Tingle (`ittincle.c`).

const GREATBAY: &str = "oracle-stage-great-bay";

#[test]
fn greatbay_setup() {
    use melee_sim::game::gr::greatbay;
    let d = hazard_dir(GREATBAY);
    check_hazard(&d, "grGreatBay_801F4240", 1, &as_saved, &by_event);
    check_hazard(&d, "grGreatBay_801F42D4", 1, &as_saved, &by_event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = greatbay::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    check_hazard(&d, "grGreatBay_801F4300", 1, &as_saved, &create);
    for f in [
        "stageGObj0_OnInit",
        "grGreatBay_801F4440",
        "grGreatBay_801F44B0",
        "grGreatBay_801F454C",
        "grGreatBay_801F4694",
        "grGreatBay_801F5460",
        "grGreatBay_801F5A00",
        "fn_801F4520",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
    let platform = |r: &mut Running, maps: &Maps, call: &Call| {
        greatbay::init_platform(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grGreatBay_801F5D4C", 1, &as_saved, &platform);
    // A giant's setup, from the moon's setup's call (its giants made).
    check_hazard(&d, "grGreatBay_801F598C", 0, &as_saved, &by_address);
}

/// The turtle's variables set: its state, the way it faces and whether its item is done, its
/// tick count.
fn turtle(c: &mut Call, state: u8, dir: u8, item_done: bool, t: i16) {
    let v = vars(c);
    poke_u8(c, v, state);
    poke_u8(c, v + 1, dir << 1 | u8::from(item_done));
    let w = peek_u32(c, v) & 0xFFFF_0000;
    poke_u32(c, v, w | t as u16 as u32);
}

#[test]
fn greatbay_turtle() {
    let d = hazard_dir(GREATBAY);
    let f = "grGreatBay_801F499C";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Down: its wait over or not; diving at its splash and not; surfacing at its sounds and
    // its fins' splashes, each way; staying down, over or not, from each way; a state past
    // them.
    let mut cases: Vec<(u8, u8, bool, i16)> = vec![(0, 0, true, -1), (0, 1, true, 7)];
    for t in [89, 90, 91] {
        cases.push((1, 0, true, t));
    }
    for t in [59, 60, 89, 90, 119, 120, 149, 150, 151] {
        cases.push((2, 0, true, t));
    }
    for dir in [0, 1, 3] {
        cases.push((2, dir, false, 10));
        cases.push((3, dir, true, -1));
        cases.push((3, dir, true, 0));
    }
    cases.push((5, 0, true, 0));
    for (state, dir, done, t) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| turtle(c, state, dir, done, t),
            &by_address,
        );
    }
    let dir = |r: &mut Running, _: &Maps, call: &Call| {
        Ret::Int(melee_sim::game::gr::greatbay::turtle_dir(r, call.regs.gpr[3] as i32) as u32)
    };
    check_hazard(&d, "grGreatBay_801F62F8", 0, &as_saved, &dir);
    let item = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        Ret::Int(u32::from(melee_sim::game::gr::greatbay::turtle_item(r, g)))
    };
    check_hazard(&d, "grGreatBay_801F63F4", 0, &as_saved, &item);
    let verts = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::greatbay::follow_vertices(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grGreatBay_801F660C", 1, &as_saved, &verts);
    // Whether the turtle is out (a CPU's question), from its process's calls, each state.
    let out = |r: &mut Running, _: &Maps, _: &Call| {
        Ret::Int(u32::from(melee_sim::game::gr::greatbay::turtle_out(r.game)))
    };
    for state in 0..4u8 {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| {
                poke_u8(c, map_vars(c, 1), state);
                c.addr = leaf_check::sym("grGreatBay_801F66A4");
            },
            &out,
        );
    }
}

/// The platform's variables set: this tick's pushes (tilt, slide, sink) and where it is.
fn platform(c: &mut Call, push: [f32; 3], at: [f32; 3]) {
    let v = map_vars(c, 2);
    poke_u32(c, v + 8, push[0].to_bits());
    poke_u32(c, v + 0xC, push[1].to_bits());
    poke_u32(c, v + 0x10, push[2].to_bits());
    for (k, x) in at.into_iter().enumerate() {
        poke_u32(c, v + 0x14 + 4 * k as u32, x.to_bits());
    }
}

#[test]
fn greatbay_platform() {
    let d = hazard_dir(GREATBAY);
    let step = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::greatbay::proc_platform(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let prepares: [&Prepare; 5] = [
        &as_saved,
        // Pushed away from rest and back toward it, each way, and sinking and rising.
        &|c, _| platform(c, [0.1, 2.0, -3.0], [0.02, 0.5, -1.0]),
        &|c, _| platform(c, [-0.1, -2.0, 0.0], [0.02, 0.5, -1.0]),
        &|c, _| platform(c, [0.0, 0.0, -3.0], [-0.05, -1.5, -0.5]),
        &|c, _| platform(c, [0.05, 1.0, 0.0], [0.0, 0.0, 0.0]),
    ];
    for p in prepares {
        check_hazard(&d, "grGreatBay_801F5E28", 1, p, &step);
    }
    check_hazard(&d, "grGreatBay_801F4658", 1, &as_saved, &by_address);
    // Its collision joint's callback: each collision kind, landing or not, each side, up to
    // its limits.
    let touched = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = maps
            .grounds
            .get(&call.regs.gpr[3])
            .copied()
            .expect("the island");
        let cd = coll_arg(r, maps, call);
        melee_sim::game::gr::greatbay::platform_touched(
            &mut r.game.objs,
            r.data,
            &mut r.game.grounds,
            r.game.gr.greatbay_yakumono,
            g,
            &cd,
            call.regs.gpr[6] as i32,
            call.regs.gpr[7] as i32,
        );
        Ret::Nothing
    };
    let kind = |c: &mut Call, k: u32| {
        let at = c.regs.gpr[5] + 0x34;
        let w = peek_u32(c, at);
        poke_u32(c, at, (w & !(0xF << 27)) | k << 27);
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &move |c, k| kind(c, (k % 4) as u32),
        &|c, _| c.regs.gpr[7] = 1,
        &|c, _| c.regs.gpr[7] = 2,
        &|c, _| platform(c, [0.5, 30.0, -40.0], [0.0, 0.0, 0.0]),
        &|c, _| platform(c, [-0.5, -30.0, -40.0], [0.0, 0.0, 0.0]),
    ];
    for p in prepares {
        check_hazard(&d, "grGreatBay_801F60C4", 0, p, &touched);
    }
}

/// The collision data a joint callback's call hands it (r5): a fighter's, or an item's.
fn coll_arg(r: &Running, maps: &Maps, call: &Call) -> melee_sim::game::structs::CollData {
    let a = call.regs.gpr[5];
    for (&fp, &f) in &maps.fighters {
        if a == fp + 0x6F0 {
            return r.game.fighters[f.0].coll_data.clone();
        }
    }
    for (&ip, &i) in &maps.items {
        if a == ip + 0x378 {
            return r.game.items[i.0].x378_itemColl.clone();
        }
    }
    panic!("r5 {a:#x} is neither a fighter's nor an item's collision data");
}

/// Tingle's stage variables set: last state, popped, splashed, a ledge hang, stood on this
/// tick and last, and their weight.
#[allow(clippy::too_many_arguments)]
fn tingle(
    c: &mut Call,
    last: i32,
    popped: i32,
    splashed: i32,
    hang: i32,
    on: i32,
    was: i32,
    w: f32,
) {
    let v = vars(c);
    for (k, x) in [last, popped, splashed, hang, on, was]
        .into_iter()
        .enumerate()
    {
        poke_u32(c, v + 4 + 4 * k as u32, x as u32);
    }
    poke_u32(c, v + 0x1C, w.to_bits());
}

#[test]
fn greatbay_tingle() {
    let d = hazard_dir(GREATBAY);
    let f = "grGreatBay_801F5600";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Stood on or not (hanging from a ledge or not), the count changed or not, popped and
    // splashed or not, a new state or not.
    let cases: [(i32, i32, i32, i32, i32, i32, f32); 8] = [
        (-1, 0, 0, 0, 1, 0, 0.8),
        (-1, 0, 0, 0, 0, 1, 0.0),
        (-1, 0, 0, 1, 2, 2, 1.6),
        (-1, 1, 1, 0, 0, 0, 0.0),
        (-1, 0, 1, 0, 1, 1, 0.8),
        (12, 1, 0, 0, 1, 1, 0.8),
        (9, 0, 0, 0, 0, 0, 0.0),
        (2, 0, 0, 0, 3, 1, 2.4),
    ];
    for (last, popped, splashed, hang, on, was, w) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| {
                let l = if last < 0 {
                    peek_u32(c, vars(c)) as i32
                } else {
                    last
                };
                tingle(c, l, popped, splashed, hang, on, was, w);
            },
            &by_address,
        );
    }
    // The balloon's collision joint's callback: each collision kind, hanging from its ledge or
    // not.
    let touched = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = maps
            .grounds
            .get(&call.regs.gpr[3])
            .copied()
            .expect("Tingle");
        let cd = coll_arg(r, maps, call);
        melee_sim::game::gr::greatbay::balloon_touched(
            &mut r.game.grounds[g.0],
            &cd,
            call.regs.gpr[6] as i32,
        );
        Ret::Nothing
    };
    check_hazard(&d, "grGreatBay_801F5914", 0, &as_saved, &touched);
    for kind in 0..5u32 {
        check_hazard(
            &d,
            "grGreatBay_801F5914",
            0,
            &move |c, _| {
                let at = c.regs.gpr[5] + 0x34;
                let w = peek_u32(c, at);
                poke_u32(c, at, (w & !(0xF << 27)) | kind << 27);
            },
            &touched,
        );
    }
    check_hazard(
        &d,
        "grGreatBay_801F5914",
        0,
        &|c, _| {
            let at = c.regs.gpr[5] + 0x84;
            let w = peek_u32(c, at);
            poke_u32(c, at, w | 0x0100_0000);
        },
        &touched,
    );
}

#[test]
fn greatbay_moon_and_sea() {
    let d = hazard_dir(GREATBAY);
    let f = "grGreatBay_801F5AF8";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Its wait about to end, ended (each flag clear or set), and counting.
    for (wait, flags) in [(1i16, 0u8), (0, 0), (0, 0xF0), (0, 0x50), (30, 0)] {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| {
                let v = vars(c);
                let w = peek_u32(c, v + 0x10) & 0x0000_00FF;
                poke_u32(
                    c,
                    v + 0x10,
                    (wait as u16 as u32) << 16 | u32::from(flags) << 8 | w,
                );
            },
            &by_address,
        );
    }
    // The sea: a fighter crossing y = -5 downward or not, or hidden.
    let sea = |r: &mut Running, maps: &Maps, call: &Call| {
        let fighter = *maps.gobjs.get(&call.regs.gpr[4]).expect("a fighter");
        Ret::Int(u32::from(melee_sim::game::gr::greatbay::sea_device(
            r, fighter,
        )))
    };
    let fighter = |c: &Call| peek_u32(c, c.regs.gpr[4] + 0x2C);
    let cross = |c: &mut Call, cur: f32, prev: f32| {
        let fp = fighter(c);
        poke_u32(c, fp + 0xB0 + 4, cur.to_bits());
        poke_u32(c, fp + 0xBC + 4, prev.to_bits());
    };
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &move |c, _| cross(c, -6.0, -4.0),
        &move |c, _| cross(c, -5.0, -4.0),
        &move |c, _| cross(c, -6.0, -5.0),
        &move |c, _| cross(c, 3.0, 4.0),
    ];
    for p in prepares {
        check_hazard(&d, "grGreatBay_801F6708", 1, p, &sea);
    }
    let shadow = |r: &mut Running, maps: &Maps, call: &Call| {
        let p = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        let j = maps.jobjs.get(&call.regs.gpr[5]).copied();
        Ret::Int(u32::from(
            melee_sim::game::gr::greatbay::check_shadow_render(r.game, r.data, pos, j),
        ))
    };
    let around = |c: &mut Call, k: usize| {
        let (p, j) = (c.regs.gpr[3], c.regs.gpr[5]);
        let y = f32_at(c, j + 0x44 + 0x1C);
        let dy = [0.25f32, -0.25, 0.0][k % 3];
        poke_u32(c, p + 4, (y + dy).to_bits());
    };
    for p in [&as_saved as &Prepare, &around] {
        check_hazard(&d, "grGreatBay_801F6814", 1, p, &shadow);
    }
}

/// The empty processes and destroy callbacks and the load and demo hooks, from the island's
/// process's calls.
#[test]
fn greatbay_unreached_callbacks() {
    let d = hazard_dir(GREATBAY);
    let from = "grGreatBay_801F4658";
    for name in [
        "grGreatBay_801F4438",
        "grGreatBay_801F443C",
        "grGreatBay_801F44A8",
        "grGreatBay_801F44AC",
        "grGreatBay_801F4518",
        "grGreatBay_801F451C",
        "grGreatBay_801F4690",
        "grGreatBay_801F545C",
        "grGreatBay_801F5988",
        "grGreatBay_801F59F8",
        "grGreatBay_801F59FC",
        "grGreatBay_801F5D48",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grGreatBay_801F42D0"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grGreatBay_801F423C"));
    check_hazard(&d, from, 1, &demo, &by_demo);
}

/// An item's callback by its address.
fn item_cb(r: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let g = gobj_arg(maps, call);
    if let Some(v) = melee_sim::game::it::kinds::run_pred(r, g, call.addr) {
        return Ret::Int(u32::from(v));
    }
    assert!(
        melee_sim::game::it::kinds::run_event(r, g, call.addr),
        "{:#x} isn't an item callback",
        call.addr
    );
    Ret::Nothing
}

/// Tingle's variables (`xDD4`) in a call made with him in r3.
/// Tingle's state entries and orders the stage or his states call, his destruction last.
const TINCLE_ENTRIES: [&str; 17] = [
    "it_802EB6DC",
    "it_802EB870",
    "it_802EBA00",
    "it_802EBE5C",
    "it_802EC35C",
    "it_802EC3F4",
    "it_802EC604",
    "it_802EC9E8",
    "it_802ECC8C",
    "it_802EBD14",
    "it_802EBFAC",
    "it_802EC18C",
    "it_802EC1F4",
    "it_802EC870",
    "it_802EC69C",
    "it_802EC4D0",
    "it_802EC830",
];

fn tincle_vars(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[3] + 0x2C) + 0xDD4
}

fn tincle_set(c: &mut Call, off: u32, v: u32) {
    let a = tincle_vars(c) + off;
    poke_u32(c, a, v);
}

#[test]
fn tincle_states() {
    let d = hazard_dir(GREATBAY);
    for f in [
        "it_802EB5C8",
        "itTincle_Logic13_DmgReceived",
        "itTincle_Logic13_DmgDealt",
        "itTincle_UnkMotion0_Phys",
        "itTincle_UnkMotion0_Coll",
        "itTincle_UnkMotion1_Phys",
        "itTincle_UnkMotion1_Coll",
        "itTincle_UnkMotion2_Anim",
        "itTincle_UnkMotion2_Coll",
        "itTincle_UnkMotion3_Phys",
        "itTincle_UnkMotion3_Coll",
        "itTincle_UnkMotion4_Phys",
        "itTincle_UnkMotion4_Coll",
        "itTincle_UnkMotion5_Phys",
        "itTincle_UnkMotion5_Coll",
        "itTincle_UnkMotion7_Anim",
        "itTincle_UnkMotion7_Coll",
        "itTincle_UnkMotion8_Phys",
        "itTincle_UnkMotion8_Coll",
        "itTincle_UnkMotion9_Anim",
        "itTincle_UnkMotion9_Coll",
        "itTincle_UnkMotion10_Anim",
        "itTincle_UnkMotion10_Coll",
        "itTincle_UnkMotion11_Anim",
        "itTincle_UnkMotion11_Coll",
        "itTincle_UnkMotion12_Phys",
        "itTincle_UnkMotion12_Coll",
    ] {
        check_hazard(&d, f, 1, &as_saved, &item_cb);
    }
    // Floating: the bob turning (its half cycles out, its count out), its wait over with no
    // drift (a drift drawn or not), a drift running, at its middle and at its end.
    let f = "itTincle_UnkMotion2_Phys";
    let prepares: [&Prepare; 7] = [
        &as_saved,
        &|c, _| tincle_set(c, 0x3C, 0.0f32.to_bits()),
        &|c, _| tincle_set(c, 0x2C, 0),
        &|c, _| {
            tincle_set(c, 0x20, 0);
            tincle_set(c, 0x24, 0);
        },
        &|c, _| {
            tincle_set(c, 0x20, 0);
            tincle_set(c, 0x24, 40);
            tincle_set(c, 0x28, 20);
        },
        &|c, _| {
            tincle_set(c, 0x20, 0);
            tincle_set(c, 0x24, 40);
            tincle_set(c, 0x28, 40);
        },
        &|c, _| {
            tincle_set(c, 0x20, 0);
            tincle_set(c, 0x24, 40);
            tincle_set(c, 0x28, 7);
        },
    ];
    for p in prepares {
        check_hazard(&d, f, 1, p, &item_cb);
    }
    // Popping: its countdown at zero, running and past, in each popping state.
    for t in [0i32, 3, -1] {
        check_hazard(
            &d,
            "itTincle_UnkMotion7_Anim",
            1,
            &move |c, _| tincle_set(c, 0x20, t as u32),
            &item_cb,
        );
    }
    // Sitting: its wait over and sunk past the limit or not.
    for (t, from) in [(0, 0.0f32), (0, 1000.0), (5, 0.0)] {
        check_hazard(
            &d,
            "itTincle_UnkMotion4_Phys",
            1,
            &move |c, _| {
                tincle_set(c, 0x20, t);
                let y = f32_at(c, peek_u32(c, c.regs.gpr[3] + 0x2C) + 0x50);
                tincle_set(c, 0x4C, (y + from).to_bits());
            },
            &item_cb,
        );
    }
    // Lying: the last loop and not.
    for t in [1, 2] {
        check_hazard(
            &d,
            "itTincle_UnkMotion10_Anim",
            1,
            &move |c, _| tincle_set(c, 0x20, t),
            &item_cb,
        );
    }
    // The stage's questions and orders, from Tingle's calls.
    // Its answers go to Tingle's stage object's variables (map 10), as the stage passes them.
    let ask = |r: &mut Running, maps: &Maps, call: &Call| {
        use melee_sim::game::it::kinds::tincle;
        let g = gobj_arg(maps, call);
        let (state, pos) = tincle::state_and_place(r.game, g);
        let vars = map_vars(call, 10);
        assert_eq!(
            (call.regs.gpr[4], call.regs.gpr[5]),
            (vars, vars + 0x20),
            "it_802ECCA4 answering elsewhere than Tingle's stage object"
        );
        let u = peek_u32(call, STAGE_INFO + 0x180 + 4 * 10);
        let gr = *maps
            .grounds
            .get(&peek_u32(call, u + 0x2C))
            .expect("Tingle's stage object");
        let v = r.game.grounds[gr.0].u.greatbay4();
        v.xC4 = state;
        v.xE4 = pos;
        Ret::Nothing
    };
    check_hazard(&d, "it_802ECCA4", 1, &as_saved, &ask);
    let weight = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        melee_sim::game::it::kinds::tincle::set_weight(r.game, g, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_hazard(&d, "it_802ECC98", 1, &as_saved, &weight);
    // Pressed down, left (below his height or not, from floating or rising), popped either
    // way, and turned, from the floating state's calls.
    let from = "itTincle_UnkMotion2_Phys";
    let order = |target: &'static str| {
        let a = leaf_check::sym(target);
        let ours = move |r: &mut Running, maps: &Maps, call: &Call| {
            use melee_sim::game::it::kinds::tincle;
            let g = gobj_arg(maps, call);
            match target {
                "it_802EBD14" => tincle::press(r, g),
                "it_802EBFAC" => tincle::release(r, g),
                "it_802EC18C" => tincle::pop(r, g),
                "it_802EC1F4" => tincle::pop_hit(r, g),
                "it_802EC870" => tincle::turn(r, g, call.regs.gpr[4] as i32),
                "it_802EC69C" => tincle::rise_again(r, g),
                "it_802EC4D0" => tincle::lie(r, g),
                _ => unreachable!(),
            }
            Ret::Nothing
        };
        (a, ours)
    };
    for target in [
        "it_802EBD14",
        "it_802EC18C",
        "it_802EC1F4",
        "it_802EC69C",
        "it_802EC4D0",
    ] {
        let (a, ours) = order(target);
        check_hazard(&d, from, 1, &move |c, _| c.addr = a, &ours);
    }
    for back in [0u32, 1] {
        let (a, ours) = order("it_802EC870");
        check_hazard(
            &d,
            from,
            1,
            &move |c, _| {
                c.addr = a;
                c.regs.gpr[4] = back;
            },
            &ours,
        );
    }
    // Starting a drift from several random seeds, so the 1 in 3 turn away goes either way.
    for seed in [1u32, 2, 3, 5, 8, 0x1234_5678, 0x9E37_79B9, 0xDEAD_BEEF] {
        let (a, ours) = order("it_802EC870");
        check_hazard(
            &d,
            from,
            1,
            &move |c, _| {
                c.addr = a;
                c.regs.gpr[4] = 0;
                poke_u32(c, leaf_check::sym("seed"), seed);
            },
            &ours,
        );
    }
    // The states' entries, his box and his timer, from their own calls.
    let entry = |r: &mut Running, maps: &Maps, call: &Call| {
        use melee_sim::game::it::kinds::tincle;
        let g = gobj_arg(maps, call);
        let name = TINCLE_ENTRIES
            .iter()
            .copied()
            .find(|&n| leaf_check::sym(n) == call.addr)
            .unwrap_or("?");
        match name {
            "it_802EB6DC" => tincle::wait(r, g),
            "it_802EB870" => tincle::drop_in(r, g),
            "it_802EBA00" => tincle::float(r, g),
            "it_802EBE5C" => tincle::sit(r, g),
            "it_802EC35C" => tincle::fall(r, g),
            "it_802EC3F4" => tincle::land(r, g),
            "it_802EC604" => tincle::get_up(r, g),
            "it_802EC9E8" => tincle::set_box(r, g),
            "it_802ECC8C" => return Ret::Int(tincle::timer(r.game, g) as u32),
            "it_802EBD14" => tincle::press(r, g),
            "it_802EBFAC" => tincle::release(r, g),
            "it_802EC18C" => tincle::pop(r, g),
            "it_802EC1F4" => tincle::pop_hit(r, g),
            "it_802EC870" => tincle::turn(r, g, call.regs.gpr[4] as i32),
            "it_802EC69C" => tincle::rise_again(r, g),
            "it_802EC4D0" => tincle::lie(r, g),
            "it_802EC830" => tincle::destroy(r, g),
            other => panic!("{other} isn't one of Tingle's entries"),
        }
        Ret::Nothing
    };
    for f in &TINCLE_ENTRIES[..16] {
        check_hazard(&d, f, 1, &as_saved, &entry);
    }
    // What no saved call reaches: the still states' animation callbacks and the states
    // without physics, each from its state's own calls, and his destruction, from the
    // floating state's.
    for (f, from) in [
        ("itTincle_UnkMotion0_Anim", "itTincle_UnkMotion0_Phys"),
        ("itTincle_UnkMotion1_Anim", "itTincle_UnkMotion1_Phys"),
        ("itTincle_UnkMotion3_Anim", "itTincle_UnkMotion3_Phys"),
        ("itTincle_UnkMotion4_Anim", "itTincle_UnkMotion4_Phys"),
        ("itTincle_UnkMotion5_Anim", "itTincle_UnkMotion5_Phys"),
        ("itTincle_UnkMotion8_Anim", "itTincle_UnkMotion8_Phys"),
        ("itTincle_UnkMotion12_Anim", "itTincle_UnkMotion12_Phys"),
        ("itTincle_UnkMotion7_Phys", "itTincle_UnkMotion7_Anim"),
        ("itTincle_UnkMotion9_Phys", "itTincle_UnkMotion9_Anim"),
        ("itTincle_UnkMotion10_Phys", "itTincle_UnkMotion10_Anim"),
        ("itTincle_UnkMotion11_Phys", "itTincle_UnkMotion11_Anim"),
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(f)), &item_cb);
    }
    check_hazard(
        &d,
        "itTincle_UnkMotion2_Phys",
        1,
        &retarget(leaf_check::sym("it_802EC830")),
        &entry,
    );
    for (was, low) in [(2.0f32, false), (5.0, false), (2.0, true)] {
        let (a, ours) = order("it_802EBFAC");
        check_hazard(
            &d,
            from,
            1,
            &move |c, _| {
                c.addr = a;
                tincle_set(c, 0x30, was.to_bits());
                if low {
                    let y = f32_at(c, peek_u32(c, c.regs.gpr[3] + 0x2C) + 0x50);
                    tincle_set(c, 0x5C, (y + 500.0).to_bits());
                }
            },
            &ours,
        );
    }
}

// Shared helpers for the stages below.

/// A halfword at `addr` (big-endian, aligned to two).
fn poke_u16(call: &mut Call, addr: u32, v: u16) {
    let at = addr & !3;
    let w = peek_u32(call, at);
    let w = if addr & 2 == 0 {
        (w & 0xFFFF) | u32::from(v) << 16
    } else {
        (w & 0xFFFF_0000) | u32::from(v)
    };
    poke_u32(call, at, w);
}

/// Sets the random seed (`*HSD_RandSeedPtr`).
fn set_seed(call: &mut Call, seed: u32) {
    let at = peek_u32(call, leaf_check::sym("HSD_RandSeedPtr"));
    poke_u32(call, at, seed);
}

/// The first fighter's GObj (`HSD_GObjPLinkHead[8]`).
fn first_fighter(call: &Call) -> u32 {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    peek_u32(call, heads + 4 * 8)
}

/// Runs a stage hook without arguments (setup, start, load) by its address.
fn event(r: &mut Running, _: &Maps, call: &Call) -> Ret {
    run_event(r, call.addr);
    Ret::Nothing
}

/// Runs a stage's demo setup by its address.
fn demo(r: &mut Running, _: &Maps, call: &Call) -> Ret {
    run_demo_init(r, call.addr, call.regs.gpr[3] as i32);
    Ret::Nothing
}

/// The shadow check's arguments: the fighter's position and the joint.
fn shadow_args(maps: &Maps, call: &Call) -> (Vec3, Option<melee_sim::hsd::jobj::JObjId>) {
    let p = call.regs.gpr[3];
    let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    (pos, maps.jobjs.get(&call.regs.gpr[5]).copied())
}

/// A stage's shadow check from its saved calls, as saved and with the fighter's feet just
/// above, below and at the joint's height (its matrix's place, when it's up to date) plus `dy0`.
fn check_shadow(d: &str, f: &str, ours: fn(&mut Running, &Maps, &Call) -> bool, dy0: f32) {
    let shadow =
        move |r: &mut Running, maps: &Maps, call: &Call| Ret::Int(u32::from(ours(r, maps, call)));
    let around = move |c: &mut Call, k: usize| {
        let (p, j) = (c.regs.gpr[3], c.regs.gpr[5]);
        let y = f32_at(c, j + 0x44 + 0x1C) + dy0;
        let dy = [0.25f32, -0.25, 0.0, 0.75, -0.75][k % 5];
        poke_u32(c, p + 4, (y + dy).to_bits());
    };
    for p in [&as_saved as &Prepare, &around] {
        check_calls_with(d, f, 1, p, &shadow);
    }
}

// Kongo Jungle N64 (`groldkongo.c`).

const OK64: &str = "oracle-stage-kongo-jungle-64";

/// groldkongo.c's `yakumono_param`.
const OK64_PARAM: u32 = 0x804D_6A90;

/// The barrel's variables, by offset into `Ground::u`.
const TARU_C4: u32 = 0;
const TARU_C6: u32 = 2;
const TARU_C8: u32 = 4;
const TARU_CA: u32 = 6;
const TARU_CC: u32 = 8;
const TARU_CE: u32 = 0xA;
const TARU_HIT: u32 = 0xC;
const TARU_KEEP: u32 = 0x10;
const TARU_D8: u32 = 0x14;
const TARU_DC: u32 = 0x18;
const TARU_E4: u32 = 0x20;
const TARU_EC: u32 = 0x28;

/// The barrel's variables set: its turn's state, speed and timer, its swing's state and
/// timer, and what it does with a fighter.
fn taru(c: &mut Call, turn: i16, speed: f32, timer: i16, swing: (i16, i16), held: i16) {
    let v = vars(c);
    poke_u16(c, v + TARU_C4, turn as u16);
    poke_u32(c, v + TARU_E4, speed.to_bits());
    poke_u16(c, v + TARU_CC, timer as u16);
    poke_u16(c, v + TARU_C8, swing.0 as u16);
    poke_u16(c, v + TARU_CE, swing.1 as u16);
    poke_u16(c, v + TARU_C6, held as u16);
}

#[test]
fn oldkongo_barrel() {
    let d = dir(OK64);
    let f = "stageGObj1_GObjProc";
    let rad = 0.017_453_292_f32;
    let prepares: [&Prepare; 18] = [
        &as_saved,
        // At rest slowing down (either way, less than a step left or more), its wait over:
        // a turn starts one way or the other.
        &|c, k| {
            let s = [0.5f32 * rad, -0.5 * rad, 3.0 * rad, -3.0 * rad][k % 4];
            set_seed(c, k as u32 * 0x9E37);
            taru(c, 0, s, 0, (0, 5), 0);
        },
        // Turning, its time up: a direction drawn; and still turning, past the clamp.
        &|c, k| {
            set_seed(c, k as u32 * 0x3141);
            taru(c, 1, 2.0 * rad, -1, (0, 5), 0);
        },
        &|c, k| taru(c, 1, [100.0f32, -100.0][k % 2] * rad, 4, (0, 5), 0),
        // Heading for a direction and coasting: far from it, and with a stop overshooting.
        &|c, k| {
            taru(
                c,
                2 + (k % 2) as i16,
                [5.0f32, -5.0][k / 2 % 2] * rad,
                3,
                (0, 5),
                0,
            )
        },
        &|c, k| {
            let v = vars(c);
            let at = f32_at(c, v + TARU_DC);
            poke_u32(c, v + TARU_D8, (at + 0.01).to_bits());
            taru(
                c,
                2 + (k % 2) as i16,
                [5.0f32, -5.0][k / 2 % 2] * rad,
                3,
                (0, 5),
                0,
            );
        },
        // The swing's cycle: each state's timer running out, the top speed reached and the
        // stop.
        &|c, _| taru(c, 0, 0.0, 9, (0, -1), 0),
        &|c, _| taru(c, 0, 0.0, 9, (1, 0), 0),
        &|c, k| {
            set_seed(c, k as u32 * 0x2718);
            let v = vars(c);
            poke_u32(c, v + TARU_EC, 9.0f32.to_bits());
            taru(c, 0, 0.0, 9, (1, 0), 0);
        },
        &|c, _| taru(c, 0, 0.0, 9, (2, -1), 0),
        &|c, k| {
            set_seed(c, k as u32 * 0x1618);
            let v = vars(c);
            poke_u32(c, v + TARU_EC, 0.001f32.to_bits());
            taru(c, 0, 0.0, 9, (3, 0), 0);
        },
        // Holding: no one there (the state clears and falls into the shot's start); a fighter
        // with its countdown running and over.
        &|c, _| {
            taru(c, 0, 0.0, 9, (0, 5), 1);
            let v = vars(c);
            poke_u32(c, v + TARU_KEEP, 0);
        },
        &|c, k| {
            taru(c, 0, 0.0, 9, (0, 5), 1);
            let v = vars(c);
            let ft = first_fighter(c);
            poke_u32(c, v + TARU_KEEP, ft);
            poke_u16(c, v + TARU_CA, [3u16, 0, 0xFFFF][k % 3]);
        },
        // The shot's animation, its timer short of the shot, and clearing.
        &|c, _| taru(c, 0, 0.0, 9, (0, 5), 2),
        &|c, _| {
            taru(c, 0, 0.0, 9, (0, 5), 3);
            let v = vars(c);
            poke_u16(c, v + TARU_HIT, 4);
        },
        // The shot at something that isn't a fighter (the barrel itself): the sound alone.
        &|c, _| {
            taru(c, 0, 0.0, 9, (0, 5), 3);
            let v = vars(c);
            poke_u16(c, v + TARU_HIT, 11);
            let me = c.regs.gpr[3];
            poke_u32(c, v + TARU_KEEP, me);
        },
        // The shot at a fighter (`ftCo_8009EC70`), at the barrel's angle as saved.
        &|c, _| {
            taru(c, 0, 0.0, 9, (0, 5), 3);
            let v = vars(c);
            poke_u16(c, v + TARU_HIT, 11);
            let ft = first_fighter(c);
            poke_u32(c, v + TARU_KEEP, ft);
        },
        &|c, _| taru(c, 0, 0.0, 9, (0, 5), 4),
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &by_address);
    }
}

#[test]
fn oldkongo_birds() {
    let d = dir(OK64);
    let f = "stageGObj2_GObjProc";
    let prepares: [&Prepare; 4] = [
        &as_saved,
        // The wait ending: they fly in from either side.
        &|c, k| {
            set_seed(c, k as u32 * 0x5851);
            let v = vars(c);
            poke_u32(c, v, 1);
        },
        // Still waiting, and flying (the animation over or not).
        &|c, _| {
            let v = vars(c);
            poke_u32(c, v, 40);
        },
        &|c, k| {
            set_seed(c, k as u32 * 0x7071);
            let v = vars(c);
            poke_u32(c, v, 0);
        },
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &by_address);
    }
}

#[test]
fn oldkongo_direction() {
    let d = dir(OK64);
    let ours = |r: &mut Running, _: &Maps, _: &Call| Ret::Float(f64::from(oldkongo::direction(r)));
    check_calls_with(&d, "grOldKongo_80210650", 1, &as_saved, &ours);
    // Seeds spread so every direction with a weight comes out.
    for s in 0..24u32 {
        check_calls_with(
            &d,
            "grOldKongo_80210650",
            1,
            &move |c, k| set_seed(c, (s * 7 + k as u32).wrapping_mul(0x6F4A_7C15)),
            &ours,
        );
    }
}

/// Moves the fighter in `gpr[reg]` to the barrel's place, `dx` along x.
fn to_barrel(c: &mut Call, reg: usize, dx: f32) {
    let si = leaf_check::sym("stage_info");
    let fp = peek_u32(c, c.regs.gpr[reg] + 0x2C);
    let x = f32_at(c, si + 0x730);
    poke_u32(c, fp + 0xB0, (x + dx).to_bits());
    for i in 1..3 {
        let w = peek_u32(c, si + 0x730 + 4 * i);
        poke_u32(c, fp + 0xB0 + 4 * i, w);
    }
}

#[test]
fn oldkongo_catch() {
    let d = dir(OK64);
    let f = "grOldKongo_80210454";
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        let ground = gobj_arg(maps, call);
        let ft = *maps
            .gobjs
            .get(&call.regs.gpr[4])
            .expect("the fighter's GObj");
        Ret::Int(u32::from(oldkongo::barrel_catch(r, ground, ft)))
    };
    // The fighter at the barrel, just inside and outside the radius either side, and at the
    // barrel while it holds someone.
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &|c, k| {
            set_seed(c, k as u32 * 0x1357);
            to_barrel(c, 4, 0.0);
        },
        &|c, k| {
            let p = peek_u32(c, OK64_PARAM);
            let r = f32_at(c, p + 0x10);
            to_barrel(c, 4, r * [0.999f32, 1.001, 1.0, -0.999][k % 4]);
        },
        &|c, _| {
            to_barrel(c, 4, 0.0);
            let v = vars(c);
            poke_u16(c, v + TARU_C6, 2);
        },
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &ours);
    }
}

#[test]
fn oldkongo_devices() {
    let d = dir(OK64);
    // The fighter's catch tests: as saved, at the barrel (where its being taken in, not yet
    // ported, stops), with its barrel timer running, and out of stamina.
    let loop_ours = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::ft::fighter::catch_devices(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let fp_of = |c: &Call| peek_u32(c, c.regs.gpr[3] + 0x2C);
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &|c, _| to_barrel(c, 3, 0.0),
        &|c, _| {
            to_barrel(c, 3, 0.0);
            let fp = fp_of(c);
            poke_u32(c, fp + 0x232C, 5);
        },
        &|c, _| {
            to_barrel(c, 3, 0.0);
            let fp = fp_of(c);
            leaf_check::calls::poke_bit(c, fp + 0x2224, 2, true);
        },
    ];
    for p in prepares {
        check_calls_with(&d, "ftColl_8007BA0C", 1, p, &loop_ours);
    }
    // Whether a device acts on the fighter, for each type and each reason a barrel's doesn't.
    let applies = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(
            r.device_applies(gobj_arg(maps, call), call.regs.gpr[5]),
        ))
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &|c, k| c.regs.gpr[5] = [1, 2, 3, 4][k % 4],
        &move |c, k| {
            c.regs.gpr[5] = [2, 3][k % 2];
            let fp = fp_of(c);
            poke_u32(c, fp + 0x232C, 1);
        },
        &move |c, _| {
            let fp = fp_of(c);
            poke_u32(c, fp + 0x10, 293);
        },
        &move |c, _| {
            let fp = fp_of(c);
            let w = peek_u32(c, fp + 0x1A68);
            poke_u32(c, fp + 0x1A68, w | 0x20);
        },
        &move |c, _| {
            let fp = fp_of(c);
            leaf_check::calls::poke_bit(c, fp + 0x2224, 2, true);
        },
    ];
    for p in prepares {
        check_calls_with(&d, "ftCo_800C0A28", 1, p, &applies);
    }
}

#[test]
fn oldkongo_setup() {
    let d = dir(OK64);
    check_calls_with(&d, "grOldKongo_8020F46C", 1, &as_saved, &event);
    check_calls_with(&d, "grOldKongo_8020F4E8", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        oldkongo::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..4u32 {
        check_calls_with(
            &d,
            "setupStageCallbacks",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "stageGObj3_OnInit",
        "stageGObj1_OnInit",
        "stageGObj2_OnInit",
        "stageGObj3_GObjProc",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    // The barrel registered as a device: as saved, and with its first slot taken.
    let register = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        oldkongo::register_catch(r.game, g, call.regs.gpr[4], CodeRef(call.regs.gpr[5]));
        Ret::Nothing
    };
    let taken = |c: &mut Call, _: usize| {
        let at = leaf_check::sym("ftDevice_BuryThings");
        let g = c.regs.gpr[3];
        poke_u32(c, at, g);
        poke_u32(c, at + 4, 3);
        poke_u32(c, at + 8, oldkongo::BARREL_CATCH);
        poke_u32(c, leaf_check::sym("ftDevice_BuryThingCount"), 1);
    };
    for p in [&as_saved as &Prepare, &taken] {
        check_calls_with(&d, "ftCo_800C0764", 1, p, &register);
    }
}

#[test]
fn oldkongo_shadow() {
    let d = dir(OK64);
    fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
        let (pos, j) = shadow_args(maps, call);
        oldkongo::check_shadow_render(r.game, r.data, pos, j)
    }
    check_shadow(&d, "grOldKongo_80210788", ours, 0.0);
}

/// The stage's callbacks no capture reaches, from the barrel's captures: the empty processes
/// and destroy callbacks, the load and demo hooks, and the barrel letting its fighter go and
/// dropping it.
#[test]
fn oldkongo_unreached_callbacks() {
    let d = dir(OK64);
    let from = "stageGObj1_GObjProc";
    for target in [
        "stageGObj0_GObjProc",
        "stageGObj0_Callback3",
        "stageGObj3_Callback3",
        "stageGObj1_Callback3",
        "stageGObj2_Callback3",
    ] {
        let t = sym_at(target, 0x8020_F468, 0x8021_07DC);
        check_calls_with(&d, from, 1, &retarget(t), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grOldKongo_8020F4E4")),
        &event,
    );
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grOldKongo_8020F468")),
        &demo,
    );
    // Letting go and dropping: held (a fighter, or the barrel itself), and not.
    let hold = |c: &mut Call, k: usize| {
        let v = vars(c);
        poke_u16(c, v + TARU_C6, [1u16, 0, 2][k % 3]);
        let keep = if k.is_multiple_of(2) {
            first_fighter(c)
        } else {
            c.regs.gpr[3]
        };
        poke_u32(c, v + TARU_KEEP, keep);
    };
    let release = |r: &mut Running, maps: &Maps, call: &Call| {
        oldkongo::barrel_release(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    let drop = |r: &mut Running, maps: &Maps, call: &Call| {
        oldkongo::barrel_drop(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    let t = leaf_check::sym("grOldKongo_802105AC");
    let prep = move |c: &mut Call, k: usize| {
        c.addr = t;
        hold(c, k);
    };
    check_calls_with(&d, from, 1, &prep, &release);
    let t = leaf_check::sym("grOldKongo_802105C8");
    let prep = move |c: &mut Call, k: usize| {
        c.addr = t;
        hold(c, k);
    };
    check_calls_with(&d, from, 1, &prep, &drop);
}

// Poké Floats (`grpura.c`).

const PF: &str = "oracle-stage-poke-floats";

/// grpura.c's code, where its `stageGObjN_*` symbols lie.
const PF_LO: u32 = 0x8021_1CFC;
const PF_HI: u32 = 0x8021_3300;

/// `JOBJ_HIDDEN` and a joint's flags.
const HIDDEN: u32 = 0x10;
const JOBJ_FLAGS: u32 = 0x14;

/// Sets or clears a joint's hidden flag.
fn hide(c: &mut Call, j: u32, hidden: bool) {
    let w = peek_u32(c, j + JOBJ_FLAGS);
    poke_u32(
        c,
        j + JOBJ_FLAGS,
        if hidden { w | HIDDEN } else { w & !HIDDEN },
    );
}

/// Runs a function of a stage object.
fn of_gobj(
    f: fn(&mut Running, melee_sim::hsd::gobj::GObjId),
) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, maps: &Maps, call: &Call| {
        f(r, gobj_arg(maps, call));
        Ret::Nothing
    }
}

#[test]
fn pura_setup() {
    let d = dir(PF);
    check_calls_with(&d, "grPura_80211D00", 1, &as_saved, &event);
    check_calls_with(&d, "grPura_80211DDC", 1, &as_saved, &event);
    // Each of the setup's objects made from its first creation (the floats come with the
    // stage's).
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        pura::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in [0u32, 1, 4] {
        check_calls_with(
            &d,
            "setupStageCallbacks",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "stageGObj1_OnInit",
        "stageGObj2_OnInit",
        "stageGObj4_OnInit",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    check_calls_with(
        &d,
        "grPura_80212CD4",
        1,
        &as_saved,
        &of_gobj(pura::init_subjects),
    );
    let floats = of_gobj(pura::make_floats);
    check_calls_with(&d, "grPura_802125F0", 1, &as_saved, &floats);
    // The stage's joints scaled 2 or more, which a float's factor leaves alone.
    let big = |c: &mut Call, _: usize| {
        let root = peek_u32(c, c.regs.gpr[3] + 0x28);
        let mut stack = vec![peek_u32(c, root + 0x10)];
        while let Some(j) = stack.pop() {
            if j == 0 {
                continue;
            }
            poke_u32(c, j + 0x2C, 2.5f32.to_bits());
            stack.push(peek_u32(c, j + 8));
            if peek_u32(c, j + JOBJ_FLAGS) & 0x1000 == 0 {
                stack.push(peek_u32(c, j + 0x10));
            }
        }
    };
    check_calls_with(&d, "grPura_802125F0", 1, &big, &floats);
    let vtx = |r: &mut Running, _: &Maps, _: &Call| {
        pura::find_vtx_joints(r);
        Ret::Nothing
    };
    check_calls_with(&d, "grPura_80212FC0", 1, &as_saved, &vtx);
    // The joint search the other way, as saved and for joints of the stage's map and others'.
    let part = |r: &mut Running, _: &Maps, call: &Call| {
        let (map, joint) = (call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
        let p = melee_sim::game::gr::setup::coll_joint_part(r.game, r.data, map, joint);
        Ret::Int(p as u32)
    };
    check_calls_with(&d, "Ground_801C33C0", 1, &as_saved, &part);
    for (map, joint) in [
        (4u32, 0u32),
        (4, 5),
        (4, 25),
        (4, 60),
        (4, u32::MAX),
        (1, 3),
        (2, 0),
    ] {
        let set = move |c: &mut Call, _: usize| {
            c.regs.gpr[3] = map;
            c.regs.gpr[4] = joint;
        };
        check_calls_with(&d, "Ground_801C33C0", 1, &set, &part);
    }
}

/// Runs a TEV compile by its address with its argument (a joint or a display object).
fn by_address_tev(r: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.regs.gpr[3];
    match call.addr {
        0x8021_3250 => {
            let j = *maps.jobjs.get(&a).expect("a joint");
            pura::compile_tree(r, j);
        }
        0x8021_3128 => {
            let d = *maps.dobjs.get(&a).expect("a display object");
            pura::compile_dobjs(r, d);
        }
        _ => pura::compile_one(r, maps.dobjs.get(&a).copied()),
    }
    Ret::Nothing
}

/// The TEV compiles a float's setup runs, from their saved calls.
#[test]
fn pura_compile() {
    let d = dir(PF);
    for f in ["grPura_80213250", "grPura_80213128", "grPura_80213224"] {
        check_calls_with(&d, f, 1, &as_saved, &by_address_tev);
    }
}

/// The stage's process and the subjects' upkeep, as saved and with the joints the subjects
/// follow hidden and shown, and the floats' vertices moved.
#[test]
fn pura_stage() {
    let d = dir(PF);
    let flip = |c: &mut Call, k: usize| {
        let v = vars(c);
        for i in 0..25u32 {
            let j = peek_u32(c, v + 4 * i);
            if j != 0 {
                hide(c, j, (i as usize + k).is_multiple_of(2));
            }
        }
    };
    for p in [&as_saved as &Prepare, &flip] {
        check_calls_with(&d, "stageGObj4_GObjProc", 1, p, &by_address);
        check_calls_with(&d, "grPura_80212EF4", 1, p, &of_gobj(pura::update_subjects));
    }
    let mv = |r: &mut Running, _: &Maps, _: &Call| {
        pura::move_vertices(r);
        Ret::Nothing
    };
    check_calls_with(&d, "grPura_80213030", 1, &as_saved, &mv);
}

/// The sky's process, as saved and at the fade's start, middle, last tick, end and past it,
/// with pairs of colors.
#[test]
fn pura_sky() {
    let d = dir(PF);
    check_calls_with(&d, "stageGObj1_GObjProc", 1, &as_saved, &by_address);
    for t in [0u16, 1, 1800, 0xE0F, 0xE10, 0xE11, 0x7FFF] {
        for (from, to) in [(0u16, 1u16), (1, 3), (3, 2), (2, 0), (3, 3)] {
            let set = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u16(c, v, from);
                poke_u16(c, v + 2, to);
                poke_u16(c, v + 4, t);
            };
            check_calls_with(&d, "stageGObj1_GObjProc", 1, &set, &by_address);
        }
    }
}

/// A float's process, as saved and with its joint and the stage's it copies each hidden and
/// shown.
#[test]
fn pura_float() {
    let d = dir(PF);
    check_calls_with(&d, "stageGObj2_GObjProc", 1, &as_saved, &by_address);
    for (src, own) in [(false, false), (false, true), (true, false), (true, true)] {
        let set = move |c: &mut Call, _: usize| {
            let s = peek_u32(c, vars(c) + 4);
            hide(c, s, src);
            let j = peek_u32(c, c.regs.gpr[3] + 0x28);
            hide(c, j, own);
        };
        check_calls_with(&d, "stageGObj2_GObjProc", 1, &set, &by_address);
    }
}

/// The floats' render callback, in the pass saved and another.
#[test]
fn pura_render() {
    let d = dir(PF);
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        pura::render_float(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let setup = leaf_check::calls::Setup {
        gx: true,
        ..Default::default()
    };
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        c.regs.gpr[4] ^= 3
    }] {
        leaf_check::calls::check_calls_in(&d, "fn_802130D0", 1, p, &setup, &render);
    }
}

/// The stage's callbacks no capture reaches, from the sky's process's captures: the empty
/// processes and destroy callbacks, the lights' setup, the shadow check, and the load and demo
/// hooks.
#[test]
fn pura_unreached_callbacks() {
    let d = dir(PF);
    let from = "stageGObj1_GObjProc";
    for name in [
        "stageGObj0_GObjProc",
        "stageGObj0_Callback3",
        "stageGObj27_OnInit",
        "stageGObj27_GObjProc",
        "stageGObj27_Callback3",
        "stageGObj4_Callback3",
        "stageGObj1_Callback3",
        "stageGObj2_Callback3",
    ] {
        let t = sym_at(name, PF_LO, PF_HI);
        check_calls_with(&d, from, 1, &retarget(t), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(pura::check_shadow_render()));
    let t = leaf_check::sym("grPura_802130C8");
    check_calls_with(&d, from, 1, &retarget(t), &shadow);
    let t = leaf_check::sym("grPura_80211DD8");
    check_calls_with(&d, from, 1, &retarget(t), &event);
    let t = leaf_check::sym("grPura_80211CFC");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
}

// Rainbow Cruise (`grrcruise.c`).

const RC: &str = "oracle-stage-rainbow-cruise";

/// The stage object the call's r3 points to: its `Ground` and joint.
fn rc_object(
    r: &Running,
    maps: &Maps,
    call: &Call,
) -> (
    melee_sim::game::handles::GroundId,
    Option<melee_sim::hsd::jobj::JObjId>,
) {
    let go = gobj_arg(maps, call);
    let g = melee_sim::game::gr::ground::ground_of(r.game, go);
    (g, melee_sim::game::gr::ground::jobj_of(r.game, go))
}

/// A stage object's `Ground` from the address of its variables (`&gp->u`) in `a`.
fn rc_ground(maps: &Maps, a: u32) -> melee_sim::game::handles::GroundId {
    *maps
        .grounds
        .get(&(a - 0xC4))
        .expect("a stage object's variables")
}

/// Sets the collision kind (`x34_flags.b1234`) of the collision data in r5.
fn rc_kind(c: &mut Call, k: u32) {
    let at = c.regs.gpr[5] + 0x34;
    let w = peek_u32(c, at);
    poke_u32(c, at, (w & !(0xF << 27)) | k << 27);
}

#[test]
fn rcruise_setup() {
    let d = dir(RC);
    check_calls_with(&d, "grRCruise_801FF168", 1, &as_saved, &event);
    check_calls_with(&d, "grRCruise_801FF29C", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        rcruise::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..7u32 {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "grRCruise_801FF2C8", 1, &set, &create);
    }
    for f in [
        "grRCruise_801FF3B4",
        "grRCruise_801FF3F0",
        "grRCruise_801FF444",
        "grRCruise_801FF5B4",
        "grRCruise_801FF73C",
        "grRCruise_801FF7A4",
        "grRCruise_801FF924",
        "grRCruise_80200074",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    let of = |f: fn(&mut Running, melee_sim::hsd::gobj::GObjId)| {
        move |r: &mut Running, maps: &Maps, call: &Call| {
            f(r, gobj_arg(maps, call));
            Ret::Nothing
        }
    };
    check_calls_with(
        &d,
        "grRCruise_80200540",
        1,
        &as_saved,
        &of(rcruise::reset_tilt),
    );
    check_calls_with(
        &d,
        "grRCruise_80200B48",
        1,
        &as_saved,
        &of(rcruise::init_planks),
    );
    check_calls_with(
        &d,
        "grRCruise_80201410",
        1,
        &as_saved,
        &of(rcruise::init_vanish),
    );
    let settle = |r: &mut Running, _: &Maps, call: &Call| {
        let map = call.regs.gpr[3] as i32;
        melee_sim::game::gr::setup::settle_map_coll(r.game, r.data, map);
        Ret::Nothing
    };
    check_calls_with(&d, "Ground_801C32AC", 1, &as_saved, &settle);
    for map in [0u32, 4, 6] {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "Ground_801C32AC", 1, &set, &settle);
    }
    // The flag's chain from the archive's description, and its swing.
    let chain = |r: &mut Running, maps: &Maps, call: &Call| {
        let root = maps.jobjs.get(&call.regs.gpr[3]).copied();
        let archive = melee_sim::game::gr::datfiles::main_archive(r.game).archive;
        let offset = (r.data.get(archive))
            .public("dynamicsdata_shipflag")
            .expect("the flag's dynamics");
        let src = melee_data::archive::DataRef { archive, offset };
        rcruise::chain(r, root, src, rc_ground(maps, call.regs.gpr[5]));
        Ret::Nothing
    };
    check_calls_with(&d, "grLib_801C9B20", 1, &as_saved, &chain);
    let swing = |r: &mut Running, maps: &Maps, call: &Call| {
        rcruise::swing(r, rc_ground(maps, call.regs.gpr[3]));
        Ret::Nothing
    };
    check_calls_with(&d, "grLib_801C9B8C", 1, &as_saved, &swing);
    check_calls_with(&d, "grRCruise_801FF8E4", 1, &as_saved, &by_address);
}

/// The ship's process with the match started and not, and its parts: the tilt, the planks,
/// the parts that appear with the camera and the wind's steering, each also from states set
/// to its branches.
#[test]
fn rcruise_ship() {
    let d = dir(RC);
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| poke_u32(c, vars(c) + 0x10, 0),
        &|c: &mut Call, _: usize| poke_u32(c, vars(c) + 0x10, 1),
    ] {
        check_calls_with(&d, "grRCruise_801FF6D4", 1, p, &by_address);
    }
    let of = |f: fn(&mut Running, melee_sim::hsd::gobj::GObjId)| {
        move |r: &mut Running, maps: &Maps, call: &Call| {
            f(r, gobj_arg(maps, call));
            Ret::Nothing
        }
    };
    // The tilt: each state, with riders on either side and none, still swinging and level.
    let tilt = of(rcruise::tilt);
    check_calls_with(&d, "grRCruise_8020071C", 1, &as_saved, &tilt);
    for state in [0u32, 1] {
        for (riders, left, right, ease, rot) in [
            (0u32, 0.0f32, 0.0f32, 0u32, 3.5f32),
            (0, 0.0, 0.0, 0, 0.1),
            (0, 0.0, 0.0, 0, 0.25),
            (0, 0.0, 0.0, 0, -0.25),
            (0, 0.0, 0.0, 0, -361.0),
            (2, 40.0, 5.0, 0, 1.0),
            (2, 5.0, 400.0, 0, -1.0),
            (1, 3.0, 3.0, 4, 2.0),
            (1, 3.0, 3.0, 4, 0.0),
        ] {
            let set = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u32(c, v + 0x2C, state);
                poke_u32(c, v + 0x34, riders);
                poke_u32(c, v + 0x24, left.to_bits());
                poke_u32(c, v + 0x28, right.to_bits());
                poke_u32(c, v + 0x38, ease);
                poke_u32(c, v + 0x18, rot.to_bits());
                poke_u32(c, v + 0x20, 0.005f32.to_bits());
                poke_u32(c, v + 0x1C, (-1.0f32).to_bits());
            };
            check_calls_with(&d, "grRCruise_8020071C", 1, &set, &tilt);
        }
    }
    // The planks: every plank in each state, at a count of 0, 1 and past every wait, touched
    // and not.
    let planks = of(rcruise::update_planks);
    check_calls_with(&d, "grRCruise_80200C04", 1, &as_saved, &planks);
    for state in 0..6u32 {
        for (count, touched) in [(0u32, 0u32), (1, 1), (1, 0), (5000, 1)] {
            let set = move |c: &mut Call, k: usize| {
                let e = peek_u32(c, vars(c) + 0x6C);
                for i in 0..17 {
                    let at = e + 0x18 * i;
                    let w = peek_u32(c, at) & 0x00FF_FFFF;
                    let flag = if (i as usize + k).is_multiple_of(2) {
                        0x80 << 16
                    } else {
                        0
                    };
                    poke_u32(c, at, (w & !(0x80 << 16)) | flag | state << 24);
                    poke_u32(c, at + 4, count);
                    poke_u32(c, at + 8, touched);
                    poke_u32(c, at + 0x10, (-0.5f32 * i as f32).to_bits());
                }
            };
            check_calls_with(&d, "grRCruise_80200C04", 1, &set, &planks);
        }
    }
    // The parts that appear with the camera: every part in each state.
    let vanish = of(rcruise::update_vanish);
    check_calls_with(&d, "grRCruise_80201588", 1, &as_saved, &vanish);
    for state in 0..4u32 {
        let set = move |c: &mut Call, _: usize| {
            let e = peek_u32(c, vars(c) + 0x70);
            for i in 0..20 {
                let w = peek_u32(c, e + 8 * i);
                poke_u32(c, e + 8 * i, (w & 0xFFFF) | state << 16);
            }
        };
        check_calls_with(&d, "grRCruise_80201588", 1, &set, &vanish);
    }
    // The wind's steering: on and past its turns and gusts, with gusts on and off, near its
    // limits.
    let steer = of(rcruise::steer_wind);
    check_calls_with(&d, "grRCruise_80201110", 1, &as_saved, &steer);
    for (age, gust, x) in [
        (0u32, 0u32, 1.0f32),
        (30, 1, 0.5),
        (300, 1, 0.51),
        (600, 0, 0.49),
        (31, 0, -1.0),
    ] {
        let set = move |c: &mut Call, _: usize| {
            let v = vars(c);
            let w = peek_u32(c, v + 4);
            poke_u32(c, w + 0x30, age);
            poke_u32(c, v + 0xC, gust);
            poke_u32(c, w + 4, x.to_bits());
            poke_u32(c, w + 0xC, (1.0f32 - x * x).max(0.0).sqrt().to_bits());
        };
        check_calls_with(&d, "grRCruise_80201110", 1, &set, &steer);
    }
    // The display objects' flags: a part's, and a tree's as the walk sets and clears them.
    let show = |r: &mut Running, maps: &Maps, call: &Call| {
        let j = maps.jobjs.get(&call.regs.gpr[3]).copied();
        rcruise::show_dobjs(&mut r.game.objs, j, call.regs.gpr[4] != 0);
        Ret::Nothing
    };
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        c.regs.gpr[4] ^= 1
    }] {
        check_calls_with(&d, "grRCruise_80201B60", 1, p, &show);
    }
    let tree = |r: &mut Running, maps: &Maps, call: &Call| {
        let j = maps.jobjs.get(&call.regs.gpr[3]).copied();
        let set = call.regs.gpr[4] == leaf_check::sym("HSD_DObjSetFlags");
        rcruise::tree_dobj_flags(&mut r.game.objs, j, set, call.regs.gpr[5]);
        Ret::Nothing
    };
    let other = |c: &mut Call, _: usize| {
        let (s, cl) = (
            leaf_check::sym("HSD_DObjSetFlags"),
            leaf_check::sym("HSD_DObjClearFlags"),
        );
        c.regs.gpr[4] = if c.regs.gpr[4] == s { cl } else { s };
    };
    for p in [&as_saved as &Prepare, &other] {
        check_calls_with(&d, "grRCruise_80201288", 1, p, &tree);
    }
}

/// The blocks' process, every block in each state, and a block touched by each kind of
/// collision in each state.
#[test]
fn rcruise_blocks() {
    let d = dir(RC);
    check_calls_with(&d, "grRCruise_80200154", 1, &as_saved, &by_address);
    let set_blocks = |c: &mut Call, state: u32, count: u32, touched: u32, flag: bool| {
        let v = vars(c);
        for i in 0..3 {
            let at = v + 0x3C + 0x10 * i;
            let w = peek_u32(c, at) & 0xFFFF;
            poke_u32(c, at, w | state << 24 | if flag { 0x80 << 16 } else { 0 });
            poke_u32(c, at + 4, count);
            poke_u32(c, at + 8, touched);
        }
    };
    for state in 0..6u32 {
        for (count, touched, flag) in [(0u32, 0u32, false), (1, 1, true), (5000, 0, false)] {
            let set = move |c: &mut Call, _: usize| set_blocks(c, state, count, touched, flag);
            check_calls_with(&d, "grRCruise_80200154", 1, &set, &by_address);
        }
    }
    let touch = |r: &mut Running, maps: &Maps, call: &Call| {
        let (g, root) = rc_object(r, maps, call);
        let kind = peek_u32(call, call.regs.gpr[5] + 0x34) >> 27 & 0xF;
        let joint = call.regs.gpr[4] as i32;
        rcruise::block_touched(&mut r.game.objs, &mut r.game.grounds, root, g, joint, kind);
        Ret::Nothing
    };
    check_calls_with(&d, "fn_80200460", 1, &as_saved, &touch);
    for state in 0..6u32 {
        for kind in [1u32, 2] {
            let set = move |c: &mut Call, _: usize| {
                set_blocks(c, state, 3, 0, false);
                rc_kind(c, kind);
            };
            check_calls_with(&d, "fn_80200460", 1, &set, &touch);
        }
    }
}

/// The deck's and the planks' touches: as saved, by each kind of collision, a heavy rider, and
/// the planks in each state.
#[test]
fn rcruise_touches() {
    let d = dir(RC);
    let deck = |r: &mut Running, maps: &Maps, call: &Call| {
        let (g, root) = rc_object(r, maps, call);
        let cd = coll_arg(r, maps, call);
        let x50 = call.regs.gpr[6] as i32;
        let (objs, grounds) = (&mut r.game.objs, &mut r.game.grounds);
        rcruise::deck_touched(objs, r.data, grounds, root, g, &cd, x50);
        Ret::Nothing
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &|c, k| rc_kind(c, (k % 5) as u32),
        &|c, _| c.regs.gpr[6] = 2500,
        &|c, _| c.regs.gpr[6] = 0,
    ];
    for p in prepares {
        check_calls_with(&d, "grRCruise_80200578", 1, p, &deck);
    }
    // The rider just past and within the pivot's reach (part 8's matrix), on either side.
    for dx in [4.5f32, -4.5, 3.5, -3.5] {
        let near = move |c: &mut Call, _: usize| {
            let pivot = nth_jobj(c, c.regs.gpr[3], 8);
            let (x, y) = (
                f32_at(c, pivot + 0x44 + 0xC),
                f32_at(c, pivot + 0x44 + 0x1C),
            );
            let cd = c.regs.gpr[5];
            poke_u32(c, cd + 4, (x + dx).to_bits());
            poke_u32(c, cd + 8, y.to_bits());
            rc_kind(c, 1);
        };
        check_calls_with(&d, "grRCruise_80200578", 1, &near, &deck);
    }
    let plank = |r: &mut Running, maps: &Maps, call: &Call| {
        let (g, _) = rc_object(r, maps, call);
        let kind = peek_u32(call, call.regs.gpr[5] + 0x34) >> 27 & 0xF;
        rcruise::plank_touched(&mut r.game.grounds, g, call.regs.gpr[4] as i32, kind);
        Ret::Nothing
    };
    check_calls_with(&d, "grRCruise_802010A4", 1, &as_saved, &plank);
    for state in [0u32, 1, 3] {
        for kind in [1u32, 3] {
            let set = move |c: &mut Call, _: usize| {
                let e = peek_u32(c, vars(c) + 0x6C);
                for i in 0..17 {
                    let w = peek_u32(c, e + 0x18 * i) & 0x00FF_FFFF;
                    poke_u32(c, e + 0x18 * i, w | state << 24);
                }
                rc_kind(c, kind);
            };
            check_calls_with(&d, "grRCruise_802010A4", 1, &set, &plank);
        }
    }
}

/// The course's process, as saved and frozen and not, and its render callback.
#[test]
fn rcruise_course() {
    let d = dir(RC);
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| poke_u8(c, vars(c), 0x80),
        &|c: &mut Call, _: usize| poke_u8(c, vars(c), 0),
    ] {
        check_calls_with(&d, "grRCruise_801FFADC", 1, p, &by_address);
    }
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        rcruise::render_course(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let setup = leaf_check::calls::Setup {
        gx: true,
        ..Default::default()
    };
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        c.regs.gpr[4] ^= 3
    }] {
        leaf_check::calls::check_calls_in(&d, "fn_80201BE0", 1, p, &setup, &render);
    }
}

/// The callbacks no capture reaches, from the course's captures: the empty processes and
/// destroy callbacks, the shadow check, the load and demo hooks, the course's wind (written
/// over the course's offset) and the planks' line test.
#[test]
fn rcruise_unreached_callbacks() {
    let d = dir(RC);
    let from = "grRCruise_801FFADC";
    for name in [
        "grRCruise_801FF3E8",
        "grRCruise_801FF3EC",
        "grRCruise_801FF43C",
        "grRCruise_801FF440",
        "grRCruise_801FF738",
        "grRCruise_801FF79C",
        "grRCruise_801FF7A0",
        "grRCruise_801FF920",
        "grRCruise_80200070",
        "grRCruise_8020045C",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(rcruise::check_shadow_render()));
    let t = leaf_check::sym("grRCruise_80201C58");
    check_calls_with(&d, from, 1, &retarget(t), &shadow);
    let t = leaf_check::sym("grRCruise_801FF298");
    check_calls_with(&d, from, 1, &retarget(t), &event);
    let t = leaf_check::sym("grRCruise_801FF164");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
    // The wind, into the course's own offset (`x10`) of the object in r3.
    let t = leaf_check::sym("grRCruise_80201918");
    let into = move |c: &mut Call, _: usize| {
        let to = vars(c) + 0x10;
        c.addr = t;
        c.regs.gpr[3] = to;
    };
    let wind = |r: &mut Running, maps: &Maps, call: &Call| {
        let w = rcruise::wind(r.game);
        let g = rc_ground(maps, call.regs.gpr[3] - 0x10);
        r.game.grounds[g.0].u.scroll().x10 = w;
        Ret::Nothing
    };
    check_calls_with(&d, from, 1, &into, &wind);
    // The planks' line test on lines across the stage.
    let t = leaf_check::sym("grRCruise_80201988");
    let line = |r: &mut Running, _: &Maps, call: &Call| {
        Ret::Int(u32::from(rcruise::plank_line(
            r.game,
            call.regs.gpr[3] as i32,
        )))
    };
    for l in [u32::MAX, 0, 5, 40, 80, 120] {
        let set = move |c: &mut Call, _: usize| {
            c.addr = t;
            c.regs.gpr[3] = l;
        };
        check_calls_with(&d, from, 1, &set, &line);
    }
    // A line of each joint about the planks' (the first line of its first lines it has, from
    // the collision data's joint descriptions).
    for joint in [26u32, 27, 28, 35, 43, 44] {
        let set = move |c: &mut Call, _: usize| {
            let coll = peek_u32(c, STAGE_INFO + 0x6AC);
            let desc = peek_u32(c, coll + 0x24) + 0x28 * joint;
            c.addr = t;
            for k in 0..5 {
                let w = peek_u32(c, desc + 4 * k);
                if w & 0xFFFF != 0 {
                    c.regs.gpr[3] = w >> 16;
                    return;
                }
            }
        };
        check_calls_with(&d, from, 1, &set, &line);
    }
}

/// A symbol's address among those of its name, the one between `lo` and `hi` (each stage has
/// its own `stageGObj0_OnInit` and the like).
fn sym_at(name: &str, lo: u32, hi: u32) -> u32 {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.2 == name && (lo..hi).contains(&s.0))
        .unwrap_or_else(|| panic!("no {name} between {lo:#x} and {hi:#x}"))
        .0
}

// Hyrule Temple (`grshrine.c`).

const HT: &str = "oracle-stage-hyrule-temple";

#[test]
fn shrine_setup() {
    let d = dir(HT);
    check_calls_with(&d, "grShrine_80201C64", 1, &as_saved, &event);
    check_calls_with(&d, "grShrine_UnkStage0_OnStart", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        shrine::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..3u32 {
        check_calls_with(
            &d,
            "grShrine_80201D20",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "grShrine_80201E44",
        "grShrine_80201E9C",
        "grShrine_80201F1C",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
}

#[test]
fn shrine_shadow() {
    let d = dir(HT);
    fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
        let (pos, j) = shadow_args(maps, call);
        shrine::check_shadow_render(r.game, r.data, pos, j)
    }
    check_shadow(&d, "grShrine_80201F4C", ours, 0.0);
}

/// The stage's callbacks no capture reaches, from the temple's process's captures: the empty
/// processes, destroy callbacks and start hook, and the load and demo hooks.
#[test]
fn shrine_unreached_callbacks() {
    let d = dir(HT);
    let from = "grShrine_80201F1C";
    for target in [
        "grShrine_80201E3C",
        "grShrine_80201E40",
        "grShrine_80201E90",
        "grShrine_80201E94",
        "grShrine_80201E98",
        "grShrine_80201F40",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(target)), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grShrine_UnkStage0_OnLoad")),
        &event,
    );
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grShrine_80201C60")),
        &demo,
    );
}

/// The ceiling tech, which the temple's ceilings reach: the check (as saved, and with the
/// fighter hugging a ceiling or not), clinging, and the cling's animation callback with the
/// script's drift flag set and clear.
#[test]
fn shrine_ceiling_tech() {
    let d = dir(HT);
    let check = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(r.passive_ceil_check(gobj_arg(maps, call))))
    };
    let fp_of = |c: &Call| peek_u32(c, c.regs.gpr[3] + 0x2C);
    let hug = move |c: &mut Call, k: usize| {
        // `coll_data.env_flags` (fp+0x6F0+0x134): the ceiling hug bit set and clear.
        let at = fp_of(c) + 0x824;
        let w = peek_u32(c, at);
        let w = if k.is_multiple_of(2) {
            w | 0x4000
        } else {
            w & !0x4000
        };
        poke_u32(c, at, w);
    };
    for p in [&as_saved as &Prepare, &hug] {
        check_calls_with(&d, "ftCo_800C23A0", 1, p, &check);
    }
    let cling = |r: &mut Running, maps: &Maps, call: &Call| {
        r.passive_ceil(gobj_arg(maps, call));
        Ret::Nothing
    };
    // Clinging clears the script's flags: set them all first.
    let flags = move |c: &mut Call, _: usize| {
        let fp = fp_of(c);
        let w = peek_u32(c, fp + 0x2210);
        poke_u32(c, fp + 0x2210, w | 0xFF00_0000);
    };
    for p in [&as_saved as &Prepare, &flags] {
        check_calls_with(&d, "ftCo_800C23FC", 1, p, &cling);
    }
    let anim = |r: &mut Running, maps: &Maps, call: &Call| {
        r.co_passive_ceil_anim(gobj_arg(maps, call));
        Ret::Nothing
    };
    let flag = move |c: &mut Call, k: usize| {
        let fp = fp_of(c);
        leaf_check::calls::poke_bit(c, fp + 0x2210, 3, k.is_multiple_of(2));
    };
    for p in [&as_saved as &Prepare, &flag] {
        check_calls_with(&d, "ftCo_PassiveCeil_Anim", 1, p, &anim);
    }
}

// Yoshi's Island (`gryorster.c`) and the stages' hurtbox items (`ityaku.c`).

const YT: &str = "oracle-stage-yoshis-island";

/// gryorster.c's `grYt_804D6A20` (its first word the parameters).
const YT_PARAM: u32 = 0x804D_6A20;

/// The parameters' float or word at `off`.
fn yt_param(call: &Call, off: u32) -> u32 {
    let p = peek_u32(call, YT_PARAM);
    peek_u32(call, p + off)
}

/// Block `i`'s variables (`grYorster_TrackElement`) in a call made with the stage in r3.
fn block(call: &Call, i: u32) -> u32 {
    vars(call) + 4 + 0x20 * i
}

/// Every block set to state `state` with damage `x04`, landing speed `x08` and count `x0C`.
fn blocks(c: &mut Call, state: u8, x04: f32, x08: f32, x0c: i32) {
    for i in 0..9 {
        let b = block(c, i);
        let w = peek_u32(c, b);
        poke_u32(c, b, (w & 0xFF00_FFFF) | u32::from(state) << 16);
        poke_u32(c, b + 4, x04.to_bits());
        poke_u32(c, b + 8, x08.to_bits());
        poke_u32(c, b + 0xC, x0c as u32);
    }
}

#[test]
fn yorster_blocks() {
    let d = dir(YT);
    let f = "grYorster_8020266C";
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        yorster::proc_blocks(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let p = |c: &Call, off: u32| f32::from_bits(yt_param(c, off));
    let prepares: [&Prepare; 12] = [
        &as_saved,
        // Hit for the parameters' damage: just over it, past the flip's full damage, and just
        // short of it.
        &|c, _| {
            let m = p(c, 8);
            blocks(c, 0, m + 0.5, 0.0, 0);
        },
        &|c, _| {
            let (m, full) = (p(c, 8), p(c, 0xC));
            blocks(c, 1, m + full + 3.0, 0.0, 0);
        },
        &|c, _| {
            let m = p(c, 8);
            blocks(c, 0, m - 0.5, 0.0, 0);
        },
        // Landed on at the parameters' speed: just over it, past the full speed, and short.
        &|c, _| {
            let m = p(c, 0);
            blocks(c, 0, 0.0, m + 0.25, 0);
        },
        &|c, _| {
            let (m, full) = (p(c, 0), p(c, 4));
            blocks(c, 1, 0.0, m + full + 1.0, 0);
        },
        &|c, _| {
            let m = p(c, 0);
            blocks(c, 0, 0.0, m - 0.25, 0);
        },
        // Waiting to flip: the delay over and running.
        &|c, k| blocks(c, 2, 0.0, 0.0, [0, 5, -1][k % 3]),
        // Spinning: short of its 323 ticks, a tick short and at them (something near or not,
        // whichever the state has).
        &|c, k| blocks(c, 3, 0.0, 0.0, [12, 0x142, 0x143, 0x144][k % 4]),
        // Trying again: short of 25 ticks, a tick short and at them.
        &|c, k| blocks(c, 4, 0.0, 0.0, [3, 0x18, 0x19][k % 3]),
        // A state past the five, and each block in another state.
        &|c, _| blocks(c, 6, 0.0, 0.0, 0),
        &|c, _| {
            for i in 0..9u32 {
                let b = block(c, i);
                let w = peek_u32(c, b);
                poke_u32(c, b, (w & 0xFF00_FFFF) | (i % 5) << 16);
                poke_u32(c, b + 0xC, [0, 0x143, 30, 2, 0][(i % 5) as usize]);
            }
        },
    ];
    for prep in prepares {
        check_calls_with(&d, f, 1, prep, &ours);
    }
}

#[test]
fn yorster_setup() {
    let d = dir(YT);
    check_calls_with(&d, "grYorster_80201FA4", 1, &as_saved, &event);
    check_calls_with(&d, "grYorster_80202010", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        yorster::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..2u32 {
        check_calls_with(
            &d,
            "grYorster_8020203C",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "grYorster_802021AC",
        "grYorster_80202160",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    let blocks_setup = |r: &mut Running, maps: &Maps, call: &Call| {
        yorster::init_blocks(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grYorster_802022A4", 1, &as_saved, &blocks_setup);
    // The stage's process, its blocks running and waiting for the match's start.
    let waiting = |c: &mut Call, k: usize| {
        let v = vars(c);
        poke_u32(c, v, (k % 2) as u32);
    };
    for p in [&as_saved as &Prepare, &waiting] {
        check_calls_with(&d, "grYorster_80202254", 1, p, &by_address);
    }
}

#[test]
fn yorster_hit_and_landing() {
    let d = dir(YT);
    // A block's item hit: as saved, and by something that isn't a fighter.
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let item = gobj_arg(maps, call);
        let g = *maps
            .grounds
            .get(&call.regs.gpr[4])
            .expect("the block's stage");
        let p = call.regs.gpr[5];
        let dir = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        let fighter = maps.gobjs.get(&call.regs.gpr[6]).copied();
        yorster::block_hit(r, item, g, dir, fighter, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    let not_fighter = |c: &mut Call, _: usize| c.regs.gpr[6] = c.regs.gpr[3];
    for p in [&as_saved as &Prepare, &not_fighter] {
        check_calls_with(&d, "grYorster_80202428", 1, p, &hit);
    }
    // A landing on a block: as saved, faster and slower than the parameters' speed, by an
    // item, by a fighter in its entrance, and on another joint.
    let landed = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps
            .grounds
            .get(&call.regs.gpr[3])
            .expect("the blocks' stage");
        let coll = coll_at(r, maps, call, call.regs.gpr[5]);
        let dy = call.regs.fpr[1].ps0 as f32;
        yorster::block_landed_now(r, g, call.regs.gpr[4] as i32, &coll, dy);
        Ret::Nothing
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        // The landing's speed (the height moved, an item's) far over and under the threshold.
        &|c, k| c.regs.fpr[1].ps0 = [100.0, -100.0][k % 2],
        // Another joint than a block's.
        &|c, _| c.regs.gpr[4] = 0,
        // The fighter's position's change (what a fighter's landing counts) large.
        &|c, _| {
            let coll = c.regs.gpr[5];
            let fp = coll - 0x6F0;
            poke_u32(c, fp + 0xC8 + 4, 50.0f32.to_bits());
        },
    ];
    for p in prepares {
        check_calls_with(&d, "grYorster_802024F0", 1, p, &landed);
    }
}

/// The collision data at `addr` in a call, as our state holds it: a fighter's
/// (`Fighter::coll_data`, +0x6F0) or an item's (`Item::x378_itemColl`).
fn coll_at(r: &Running, maps: &Maps, call: &Call, addr: u32) -> melee_sim::game::structs::CollData {
    for (&a, &g) in &maps.gobjs {
        let user = peek_u32(call, a + 0x2C);
        if user + 0x6F0 == addr {
            return r.game.fighter(g).coll_data.clone();
        }
        if user + 0x378 == addr {
            return melee_sim::game::it::item(r.game, g).x378_itemColl.clone();
        }
    }
    panic!("no object's collision data at {addr:#x}")
}

/// The stage's callbacks no capture reaches, and the hurtbox item's, from the blocks'
/// captures and the items'.
#[test]
fn yorster_unreached_callbacks() {
    let d = dir(YT);
    let from = "grYorster_8020266C";
    for target in [
        "grYorster_80202158",
        "grYorster_8020215C",
        "grYorster_802022A0",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(target)), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grYorster_8020200C")),
        &event,
    );
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grYorster_80201FA0")),
        &demo,
    );
}

/// A code address argument as a callback, none for null.
fn code_arg(a: u32) -> Option<CodeRef> {
    (a != 0).then_some(CodeRef(a))
}

/// The stages' hurtbox items: made through the stage glue and directly (on a joint, at a point,
/// falling or not, and with neither), each state's physics (following a joint or standing at a
/// point), a hit taken, and the hit callbacks and the forgetting no capture reaches.
#[test]
fn yaku_items() {
    use melee_sim::game::it::kinds::yaku;
    let d = dir(YT);
    let glue = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let ground = *maps.grounds.get(&g[5]).expect("the item's stage object");
        let joint = maps.jobjs.get(&g[6]).copied();
        let item = melee_sim::game::gr::material::stage_item(
            r,
            g[3] as i32,
            g[4] as i32,
            ground,
            joint,
            code_arg(g[7]),
            code_arg(g[8]),
            code_arg(g[9]),
        );
        let _ = item;
        Ret::Nothing
    };
    check_calls_with(&d, "grMaterial_801C8CFC", 1, &as_saved, &glue);
    let spawn = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let ground = maps.grounds.get(&g[3]).copied();
        let joint = maps.jobjs.get(&g[6]).copied();
        let pos = (g[7] != 0).then(|| {
            Vec3::new(
                f32_at(call, g[7]),
                f32_at(call, g[7] + 4),
                f32_at(call, g[7] + 8),
            )
        });
        let touched = peek_u32(call, g[1] + 8);
        let item = yaku::spawn(
            r,
            ground,
            g[4] as i32,
            g[5] as i32,
            joint,
            pos,
            g[8] != 0,
            code_arg(g[9]),
            code_arg(g[10]),
            code_arg(touched),
        );
        let _ = item;
        Ret::Nothing
    };
    let at_point = |c: &mut Call, k: usize| {
        let si = leaf_check::sym("stage_info");
        c.regs.gpr[6] = 0;
        c.regs.gpr[7] = si + 0x730;
        c.regs.gpr[8] = (k % 2) as u32;
    };
    let neither = |c: &mut Call, _: usize| {
        c.regs.gpr[6] = 0;
        c.regs.gpr[7] = 0;
    };
    for p in [&as_saved as &Prepare, &at_point, &neither] {
        check_calls_with(&d, "it_802E6AEC", 1, p, &spawn);
    }
    let phys = |r: &mut Running, maps: &Maps, call: &Call| {
        yaku::phys(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let point = |c: &mut Call, _: usize| {
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u16(c, ip + 0xDD4 + 2, 2);
    };
    for p in [&as_saved as &Prepare, &point] {
        check_calls_with(&d, "it_2E6A_UnkMotion19_Phys", 1, p, &phys);
    }
    let received = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(yaku::dmg_received(r, gobj_arg(maps, call))))
    };
    check_calls_with(&d, "it_2E6A_Logic117_DmgReceived", 1, &as_saved, &received);
    // From the physics' captures: a hit dealt and a touch (with and without the stage's
    // callbacks), and forgetting a fighter.
    let pred = |f: fn(&mut Running, melee_sim::hsd::gobj::GObjId) -> bool| {
        move |r: &mut Running, maps: &Maps, call: &Call| {
            Ret::Int(u32::from(f(r, gobj_arg(maps, call))))
        }
    };
    let dealt = pred(yaku::dmg_dealt);
    let touched = pred(yaku::touched);
    let from = "it_2E6A_UnkMotion19_Phys";
    let callbacks = |name: &'static str| {
        let t = leaf_check::sym(name);
        move |c: &mut Call, _: usize| {
            c.addr = t;
            // The blocks' items have neither callback.
            let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
            poke_u32(c, ip + 0xDD4 + 0x14, 0);
            poke_u32(c, ip + 0xDD4 + 0x1C, 0);
        }
    };
    check_calls_with(&d, from, 1, &callbacks("it_2E6A_Logic117_DmgDealt"), &dealt);
    // A hit dealt by an item with Jungle Japes' Klaptrap's callback.
    let t = leaf_check::sym("it_2E6A_Logic117_DmgDealt");
    let bit = leaf_check::sym("grGarden_80203498");
    let klaptrap = move |c: &mut Call, _: usize| {
        c.addr = t;
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u32(c, ip + 0xDD4 + 0x14, bit);
    };
    check_calls_with(&d, from, 1, &klaptrap, &dealt);
    check_calls_with(&d, from, 1, &callbacks("it_802E7054"), &touched);
    let forget = |r: &mut Running, maps: &Maps, call: &Call| {
        let other = maps.gobjs.get(&call.regs.gpr[4]).copied();
        yaku::evt_unk(r, gobj_arg(maps, call), other);
        Ret::Nothing
    };
    let t = leaf_check::sym("it_2E6A_Logic117_EvtUnk");
    let with_fighter = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[4] = first_fighter(c);
    };
    check_calls_with(&d, from, 1, &with_fighter, &forget);
}

/// The stage glue's small functions, `Ground_801C32D4` for each part, and the fighters' hits on
/// items' hurtboxes, the hurtbox item's first halfword set to the two numbers its sound tests.
#[test]
fn yaku_glue_and_hits() {
    let d = dir(YT);
    let hurtbox = |r: &mut Running, maps: &Maps, call: &Call| {
        let f = |i: usize| call.regs.fpr[i].ps0 as f32;
        melee_sim::game::gr::material::set_item_hurtbox(
            r.game,
            gobj_arg(maps, call),
            Vec3::new(f(1), f(2), f(3)),
            Vec3::new(f(4), f(5), f(6)),
            f(7),
        );
        Ret::Nothing
    };
    check_calls_with(&d, "grMaterial_801C8DE0", 1, &as_saved, &hurtbox);
    let on = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::material::item_hurtbox_on(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grMaterial_801C8E08", 1, &as_saved, &on);
    let off = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::material::item_hurtbox_off(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grMaterial_801C8E28", 1, &as_saved, &off);
    let air = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        melee_sim::game::gr::material::item_ground_or_air(r.game, g, call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let either = |c: &mut Call, k: usize| c.regs.gpr[4] = (k % 2) as u32;
    for p in [&as_saved as &Prepare, &either] {
        check_calls_with(&d, "grMaterial_801C8E68", 1, p, &air);
    }
    let joint = |r: &mut Running, _: &Maps, call: &Call| {
        let id = melee_sim::game::gr::setup::coll_joint_id(
            r.game,
            r.data,
            call.regs.gpr[3] as i32,
            call.regs.gpr[4] as i32,
        );
        Ret::Int(id as u32)
    };
    let parts = |c: &mut Call, k: usize| {
        c.regs.gpr[3] = (k % 3) as u32;
        c.regs.gpr[4] = (k * 7 % 24) as u32;
    };
    for p in [&as_saved as &Prepare, &parts] {
        check_calls_with(&d, "Ground_801C32D4", 1, p, &joint);
    }
    let hits = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::it::coll::hit_by_fighters(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let number = |c: &mut Call, k: usize| {
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u16(c, ip + 0xDD4, [7u16, 8][k % 2]);
    };
    for p in [&as_saved as &Prepare, &number] {
        check_calls_with(&d, "it_802703E8", 1, p, &hits);
    }
}

// Jungle Japes (`grgarden.c`).

const GD: &str = "oracle-stage-jungle-japes";

/// grgarden.c's `yakumono_param`.
const GD_PARAM: u32 = 0x804D_6A28;

#[test]
fn garden_setup() {
    let d = dir(GD);
    check_calls_with(&d, "grGarden_80202B70", 1, &as_saved, &event);
    check_calls_with(&d, "grGarden_80202C24", 1, &as_saved, &event);
    check_calls_with(&d, "grGarden_80202C4C", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        garden::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..7u32 {
        check_calls_with(
            &d,
            "grGarden_80202C78",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "stageGObj0_OnInit",
        "grGarden_80202DC8",
        "grGarden_80202D9C",
        "grGarden_80202E48",
        "grGarden_80202E6C",
        "grGarden_80202F44",
        "grGarden_80202FA4",
        "grGarden_80203004",
        "grGarden_802031A4",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
}

#[test]
fn garden_cranky() {
    let d = dir(GD);
    // Each of his four animations with its count run out, and with runs left.
    let set = |c: &mut Call, anim: u32, left: u32| {
        let v = vars(c);
        poke_u32(c, v, anim);
        poke_u32(c, v + 4, left);
    };
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &move |c, k| set(c, (k % 4) as u32, 0),
        &move |c, k| set(c, (k % 4) as u32, 3),
    ];
    for p in prepares {
        check_calls_with(&d, "grGarden_80203098", 1, p, &by_address);
    }
}

#[test]
fn garden_klaptrap() {
    let d = dir(GD);
    // Waiting: its wait over (from several seeds) and running; leaping: at each tick a splash
    // sounds and at another.
    let set = |c: &mut Call, state: u8, count: u32| {
        let v = vars(c);
        let w = peek_u32(c, v + 4);
        poke_u32(c, v + 4, (w & 0x00FF_FFFF) | u32::from(state) << 24);
        poke_u32(c, v + 8, count);
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &move |c, k| {
            set_seed(c, k as u32 * 0x4F1B);
            set(c, 0, 0);
        },
        &move |c, _| set(c, 0, 9),
        &move |c, k| set(c, 1, [38, 110, 148, 210, 5][k % 5]),
    ];
    for p in prepares {
        check_calls_with(&d, "grGarden_80203250", 1, p, &by_address);
    }
}

#[test]
fn garden_river() {
    let d = dir(GD);
    // The current goes where the prepares point the callback's vector: the stage's barrel
    // place, compared state that nothing on Jungle Japes uses.
    let river = |r: &mut Running, maps: &Maps, call: &Call| {
        let ground = gobj_arg(maps, call);
        let fighter = *maps
            .gobjs
            .get(&call.regs.gpr[4])
            .expect("the fighter's GObj");
        let pull = garden::river(r, ground, fighter);
        if let Some(v) = pull {
            r.game.stage_info.x730 = v;
        }
        Ret::Int(u32::from(pull.is_some()))
    };
    let water = |c: &mut Call| c.regs.gpr[5] = leaf_check::sym("stage_info") + 0x730;
    // The fighter above the surface, below it, and just falling in (with the splash).
    let place = move |c: &mut Call, now: f32, before: f32| {
        water(c);
        let p = peek_u32(c, GD_PARAM);
        let surface = f32_at(c, p);
        let fp = peek_u32(c, c.regs.gpr[4] + 0x2C);
        poke_u32(c, fp + 0xB0 + 4, (surface + now).to_bits());
        poke_u32(c, fp + 0xBC + 4, (surface + before).to_bits());
    };
    let prepares: [&Prepare; 4] = [
        &move |c, _| water(c),
        &move |c, _| place(c, 5.0, 5.0),
        &move |c, _| place(c, -5.0, -3.0),
        &move |c, _| place(c, -2.0, 1.0),
    ];
    for p in prepares {
        check_calls_with(&d, "grGarden_8020349C", 1, p, &river);
    }
}

#[test]
fn garden_shadow() {
    let d = dir(GD);
    fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
        let (pos, j) = shadow_args(maps, call);
        garden::check_shadow_render(r.game, r.data, pos, j)
    }
    check_shadow(&d, "grGarden_802035D0", ours, 0.0);
}

/// The stage's callbacks no capture reaches, from Cranky's captures: the empty processes and
/// destroy callbacks, the demo hook, the Klaptrap's hit callback, the splash and the camera's
/// floor.
#[test]
fn garden_unreached_callbacks() {
    let d = dir(GD);
    let from = "grGarden_80203098";
    for target in [
        "grGarden_80202D94",
        "grGarden_80202D98",
        "grGarden_80202E68",
        "grGarden_80202F3C",
        "grGarden_80202F40",
        "grGarden_80202F9C",
        "grGarden_80202FA0",
        "grGarden_80202FFC",
        "grGarden_80203000",
        "grGarden_802031A0",
        "grGarden_80203494",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(target)), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grGarden_80202B6C")),
        &demo,
    );
    let bit = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = melee_sim::game::gr::ground::ground_of(r.game, gobj_arg(maps, call));
        garden::klaptrap_bit(r, gobj_arg(maps, call), g);
        Ret::Nothing
    };
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grGarden_80203498")),
        &bit,
    );
    let splash = |r: &mut Running, _: &Maps, call: &Call| {
        let at = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, at), f32_at(call, at + 4), f32_at(call, at + 8));
        garden::splash(r, call.regs.fpr[1].ps0 as f32, pos);
        Ret::Nothing
    };
    let t = leaf_check::sym("grGarden_80203560");
    let prep = move |c: &mut Call, k: usize| {
        c.addr = t;
        c.regs.fpr[1].ps0 = [0.5, 1.0, 2.0][k % 3];
        // The splash's place, as the river passes it, in a vector nothing on Jungle Japes
        // uses (the stage's barrel place): the stack below the frame would be overwritten.
        let at = leaf_check::sym("stage_info") + 0x730;
        poke_u32(c, at + 4, (-30.0f32).to_bits());
        poke_u32(c, at + 8, 0);
        c.regs.gpr[3] = at;
    };
    check_calls_with(&d, from, 1, &prep, &splash);
    let floor = |_: &mut Running, _: &Maps, _: &Call| Ret::Float(f64::from(garden::camera_floor()));
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grGarden_80203624")),
        &floor,
    );
}

/// The stage fog fitted to the camera (`Ground_801C4FAC`, Jungle Japes has guide points), from
/// its captures: as saved, the eye moved to either side of what it looks at and behind it, the
/// guide points all at the eye (the fog's least start and end) and the near ones far past the
/// far ones (the end just past the start).
#[test]
fn garden_fog() {
    let d = dir(GD);
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        let c = *maps
            .cobjs
            .get(&call.regs.gpr[3])
            .expect("the camera's CObj");
        r.stage_fog_update(c);
        Ret::Nothing
    };
    let wobj_pos = |c: &Call, at: u32| {
        let w = peek_u32(c, c.regs.gpr[3] + at);
        (w + 0xC, f32_at(c, w + 0xC), f32_at(c, w + 0x14))
    };
    let eye_off = |c: &mut Call, dx: f32, dz: f32| {
        let (eye, _, _) = wobj_pos(c, 0x24);
        let (_, ix, iz) = wobj_pos(c, 0x28);
        poke_u32(c, eye, (ix + dx).to_bits());
        poke_u32(c, eye + 8, (iz + dz).to_bits());
    };
    let points = |c: &mut Call, near: (f32, f32, f32), far: (f32, f32, f32)| {
        let si = leaf_check::sym("stage_info");
        for (k, p) in [near, far, near, far, near, far].into_iter().enumerate() {
            let at = si + 0x130 + 12 * k as u32;
            poke_u32(c, at, p.0.to_bits());
            poke_u32(c, at + 4, p.1.to_bits());
            poke_u32(c, at + 8, p.2.to_bits());
        }
    };
    let at_eye = move |c: &mut Call, _: usize| {
        let (eye, _, _) = wobj_pos(c, 0x24);
        let p = (f32_at(c, eye), f32_at(c, eye + 4), f32_at(c, eye + 8));
        points(c, p, p);
    };
    let crossed = move |c: &mut Call, _: usize| {
        let (eye, _, _) = wobj_pos(c, 0x24);
        let p = (f32_at(c, eye), f32_at(c, eye + 4), f32_at(c, eye + 8));
        points(c, (p.0, p.1, p.2 - 5000.0), (p.0, p.1, p.2 - 20.0));
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &move |c, _| eye_off(c, 60.0, 200.0),
        &move |c, _| eye_off(c, -60.0, 200.0),
        &move |c, _| eye_off(c, 30.0, -200.0),
        &at_eye,
        &crossed,
    ];
    for p in prepares {
        check_calls_with(&d, "Ground_801C4FAC", 1, p, &ours);
    }
}

/// The items' state changes in the match (`Item_80268E5C`), the Klaptrap's among them, whose
/// bite's script is the stage's (`ALDYakuAll`), and a fighter's camera box depth.
#[test]
fn garden_items_and_camera() {
    let d = dir(GD);
    let change = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        melee_sim::game::it::item::change_state(r, gobj_arg(maps, call), g[4] as i32, g[5]);
        Ret::Nothing
    };
    check_calls_with(&d, "Item_80268E5C", 1, &as_saved, &change);
    let depth = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Float(f64::from(r.game.ft_camera_depth(gobj_arg(maps, call))))
    };
    check_calls_with(&d, "ftLib_80086B80", 1, &as_saved, &depth);
}

// Kongo Jungle (`grkongo.c`) and its Klap Trap (`itklap.c`).

const KG: &str = "oracle-stage-kongo-jungle";

/// grkongo.c's `yakumono_param`.
const KG_PARAM: u32 = 0x804D_6980;

fn kg_param(c: &Call, off: u32) -> f32 {
    let p = peek_u32(c, KG_PARAM);
    f32_at(c, p + off)
}

#[test]
fn kongo_setup() {
    let d = dir(KG);
    check_calls_with(&d, "grKongo_801D523C", 1, &as_saved, &event);
    check_calls_with(&d, "grKongo_801D52FC", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        kongo::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..12u32 {
        check_calls_with(
            &d,
            "grKongo_801D5340",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "grKongo_801D5490",
        "grKongo_801D55D8",
        "grKongo_801D5FA8",
        "grKongo_801D5FE4",
        "grKongo_801D6038",
        "grKongo_801D6074",
        "grKongo_801D637C",
        "grKongo_801D651C",
        "fn_801D542C",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    let bridge = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::init_bridge(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grKongo_801D69B0", 1, &as_saved, &bridge);
}

/// The stage's variables: the event's wait and the last event.
fn kg_events(c: &mut Call, wait: i16, last: i16) {
    let v = vars(c);
    poke_u16(c, v + 0x20, wait as u16);
    poke_u16(c, v + 0x22, last as u16);
}

#[test]
fn kongo_stage() {
    let d = dir(KG);
    // The event's wait running out from many seeds (each event, and a repeat drawn again or
    // not), still running, and before the start.
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &|c, k| {
            set_seed(c, (k as u32).wrapping_mul(0x9E37_79B9));
            kg_events(c, 1, -1);
        },
        &|c, k| {
            set_seed(c, (k as u32 + 7).wrapping_mul(0x6F4A_7C15));
            kg_events(c, 1, [1, 3][k % 2]);
        },
        &|c, _| kg_events(c, 9, -1),
        &|c, _| kg_events(c, -1, -1),
    ];
    for p in prepares {
        check_calls_with(&d, "grKongo_801D557C", 1, p, &by_address);
    }
    let events = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::events(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grKongo_801D7BBC", 1, &as_saved, &events);
}

/// The bridge's planks' speeds and angles kicked: plank `i` swung by `a` radians and pushed
/// by `s`.
fn kg_planks(c: &mut Call, a: f32, s: f32) {
    let t = leaf_check::sym("grKg_803E188C");
    for i in 0..15u32 {
        let at = t + 0x18 * i;
        let rest = f32_at(c, at + 8);
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        poke_u32(c, at + 0xC, (rest + sign * a * (i as f32 / 7.0)).to_bits());
        poke_u32(c, at + 0x10, (sign * s).to_bits());
    }
}

#[test]
fn kongo_bridge() {
    let d = dir(KG);
    let rad = 0.017_453_292_f32;
    // The planks at rest, swung a little and far (past the slack, the pull and the limits),
    // and the posts' sway at each branch.
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &move |c, _| kg_planks(c, 2.0 * rad, 0.5 * rad),
        &move |c, _| kg_planks(c, 30.0 * rad, 10.0 * rad),
        &move |c, k| kg_planks(c, [8.0f32, -8.0][k % 2] * rad, [-3.0f32, 3.0][k % 2] * rad),
    ];
    let swing = |r: &mut Running, _: &Maps, _: &Call| {
        kongo::swing_planks(r.game, r.data);
        Ret::Nothing
    };
    let bridge = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::bridge(r, gobj_arg(maps, call), call.regs.gpr[4] != 0);
        Ret::Nothing
    };
    for p in prepares {
        check_calls_with(&d, "grKongo_801D6AFC", 1, p, &swing);
        check_calls_with(&d, "grKongo_801D7134", 1, p, &bridge);
    }
    let sway = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::sway(r, gobj_arg(maps, call), call.regs.gpr[4] != 0);
        Ret::Nothing
    };
    let posts = |c: &mut Call, a: f32, s: f32| {
        let v = vars(c);
        for at in [0, 0x10] {
            poke_u32(c, v + at, a.to_bits());
            poke_u32(c, v + at + 4, s.to_bits());
        }
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &move |c, _| posts(c, 0.2, 0.01),
        &move |c, _| posts(c, -0.2, 0.01),
        &move |c, _| posts(c, 0.2, -0.01),
        &move |c, _| posts(c, 3.0, 1.0),
        &move |c, _| posts(c, 0.0001, 0.0001),
    ];
    for p in prepares {
        check_calls_with(&d, "grKongo_801D77E0", 1, p, &sway);
    }
}

#[test]
fn kongo_bridge_landing() {
    let d = dir(KG);
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let coll = coll_at(r, maps, call, g[5]);
        let mut kicks = kongo::Kicks::default();
        kicks.landed(r.game.geometry_mut(), &coll, g[7] as i32);
        kicks.apply(r.game, r.data);
        Ret::Nothing
    };
    // Landings as saved, of another ground kind, and at each end and past them.
    let prepares: [&Prepare; 3] = [&as_saved, &|c, k| c.regs.gpr[7] = [0, 2][k % 2], &|c, k| {
        let x = [-500.0f32, 500.0, -40.0, 40.0][k % 4];
        poke_u32(c, c.regs.gpr[5] + 0x84, x.to_bits());
    }];
    for p in prepares {
        check_calls_with(&d, "fn_801D7700", 1, p, &ours);
    }
}

/// The barrel's variables (`grKongo_GroundVars3`), by offset into `Ground::u`.
const KG_TARU_C4: u32 = 0;
const KG_TARU_C6: u32 = 2;
const KG_TARU_C8: u32 = 4;
const KG_TARU_CA: u32 = 6;
const KG_TARU_CC: u32 = 8;
const KG_TARU_CE: u32 = 0xA;
const KG_TARU_KEEP: u32 = 0xC;
const KG_TARU_D4: u32 = 0x10;
const KG_TARU_D8: u32 = 0x14;
const KG_TARU_E0: u32 = 0x1C;
const KG_TARU_E8: u32 = 0x24;

fn kg_taru(c: &mut Call, turn: i16, speed: f32, timer: i16, swing: (i16, i16), held: i16) {
    let v = vars(c);
    poke_u16(c, v + KG_TARU_C4, turn as u16);
    poke_u32(c, v + KG_TARU_E0, speed.to_bits());
    poke_u16(c, v + KG_TARU_CC, timer as u16);
    poke_u16(c, v + KG_TARU_C8, swing.0 as u16);
    poke_u16(c, v + KG_TARU_CE, swing.1 as u16);
    poke_u16(c, v + KG_TARU_C6, held as u16);
}

#[test]
fn kongo_barrel() {
    let d = dir(KG);
    let f = "grKongo_801D577C";
    let rad = 0.017_453_292_f32;
    let prepares: [&Prepare; 17] = [
        &as_saved,
        &|c, k| {
            let s = [0.5f32 * rad, -0.5 * rad, 3.0 * rad, -3.0 * rad][k % 4];
            set_seed(c, k as u32 * 0x9E37);
            kg_taru(c, 0, s, 0, (0, 5), 3);
        },
        &|c, k| {
            set_seed(c, k as u32 * 0x3141);
            kg_taru(c, 1, 2.0 * rad, -1, (0, 5), 3);
        },
        &|c, k| kg_taru(c, 1, [100.0f32, -100.0][k % 2] * rad, 4, (0, 5), 3),
        &|c, k| {
            kg_taru(
                c,
                2 + (k % 2) as i16,
                [5.0f32, -5.0][k / 2 % 2] * rad,
                3,
                (0, 5),
                3,
            )
        },
        &|c, k| {
            let v = vars(c);
            let at = f32_at(c, v + KG_TARU_D8);
            poke_u32(c, v + KG_TARU_D4, (at + 0.01).to_bits());
            kg_taru(
                c,
                2 + (k % 2) as i16,
                [5.0f32, -5.0][k / 2 % 2] * rad,
                3,
                (0, 5),
                3,
            );
        },
        // Heading for a direction between half and the whole of its stopping distance
        // (eight steps of speed, 24 steps away), either way.
        &|c, k| {
            let a = kg_param(c, 0x34) * 0.017_453_292;
            let sign = [1.0f32, -1.0][k % 2];
            let v = vars(c);
            let at = f32_at(c, v + KG_TARU_D8);
            poke_u32(c, v + KG_TARU_D4, (at + sign * 24.0 * a).to_bits());
            kg_taru(c, 2 + (k / 2 % 2) as i16, sign * 8.0 * a, 3, (0, 5), 3);
        },
        &|c, _| kg_taru(c, 0, 0.0, 9, (0, -1), 3),
        &|c, k| {
            set_seed(c, k as u32 * 0x2718);
            let v = vars(c);
            poke_u32(c, v + KG_TARU_E8, 9.0f32.to_bits());
            kg_taru(c, 0, 0.0, 9, (1, 0), 3);
        },
        &|c, _| kg_taru(c, 0, 0.0, 9, (2, -1), 3),
        &|c, k| {
            set_seed(c, k as u32 * 0x1618);
            let v = vars(c);
            poke_u32(c, v + KG_TARU_E8, 0.001f32.to_bits());
            kg_taru(c, 0, 0.0, 9, (3, 0), 3);
        },
        // Holding nothing (the state clears and falls into the shot, which reads null: not
        // reached by any call), a fighter's countdown running.
        &|c, k| {
            kg_taru(c, 0, 0.0, 9, (0, 5), 1);
            let v = vars(c);
            let ft = first_fighter(c);
            poke_u32(c, v + KG_TARU_KEEP, ft);
            poke_u16(c, v + KG_TARU_CA, [3u16, 0][k % 2]);
        },
        // The shot at something that is neither a fighter nor an item (the barrel itself).
        &|c, _| {
            kg_taru(c, 0, 0.0, 9, (0, 5), 2);
            let v = vars(c);
            let me = c.regs.gpr[3];
            poke_u32(c, v + KG_TARU_KEEP, me);
        },
        // The shot at a fighter (`ftCo_8009EC70`), at the barrel's heading as saved.
        &|c, _| {
            kg_taru(c, 0, 0.0, 9, (0, 5), 2);
            let v = vars(c);
            let ft = first_fighter(c);
            poke_u32(c, v + KG_TARU_KEEP, ft);
        },
        // Waiting for nothing near, catching (with nothing near, or a Klap Trap if there is
        // one), and a state past the four.
        &|c, _| kg_taru(c, 0, 0.0, 9, (0, 5), 3),
        &|c, _| kg_taru(c, 0, 0.0, 9, (0, 5), 0),
        &|c, _| kg_taru(c, 0, 0.0, 9, (0, 5), 4),
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &by_address);
    }
    let ours = |r: &mut Running, _: &Maps, _: &Call| Ret::Float(f64::from(kongo::direction(r)));
    for s in 0..24u32 {
        check_calls_with(
            &d,
            "grKongo_801D8314",
            1,
            &move |c, k| set_seed(c, (s * 7 + k as u32).wrapping_mul(0x6F4A_7C15)),
            &ours,
        );
    }
}

#[test]
fn kongo_catch() {
    let d = dir(KG);
    let ours = |r: &mut Running, maps: &Maps, call: &Call| {
        let ground = gobj_arg(maps, call);
        let ft = *maps
            .gobjs
            .get(&call.regs.gpr[4])
            .expect("the fighter's GObj");
        Ret::Int(u32::from(kongo::barrel_catch(r, ground, ft)))
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &|c, k| {
            set_seed(c, k as u32 * 0x1357);
            to_barrel(c, 4, 0.0);
            let v = vars(c);
            poke_u16(c, v + KG_TARU_C6, 0);
        },
        &|c, k| {
            let r = kg_param(c, 0x28);
            to_barrel(c, 4, r * [0.999f32, 1.001, 1.0, -0.999][k % 4]);
            let v = vars(c);
            poke_u16(c, v + KG_TARU_C6, 0);
        },
        &|c, _| {
            to_barrel(c, 4, 0.0);
            let v = vars(c);
            poke_u16(c, v + KG_TARU_C6, 2);
        },
    ];
    for p in prepares {
        check_calls_with(&d, "fn_801D8134", 1, p, &ours);
    }
    // The barrel letting go and dropping, from the barrel's calls: holding a fighter, holding
    // something else, and not holding.
    let release = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::barrel_release(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    let drop = |r: &mut Running, maps: &Maps, call: &Call| {
        kongo::barrel_drop(r.game, gobj_arg(maps, call));
        Ret::Nothing
    };
    for (target, ours) in [
        (
            "grKongo_801D8270",
            &release as &(dyn Fn(&mut Running, &Maps, &Call) -> Ret + Sync),
        ),
        ("grKongo_801D828C", &drop),
    ] {
        let t = leaf_check::sym(target);
        let prepares: [&Prepare; 3] = [
            &move |c, _| {
                c.addr = t;
                let v = vars(c);
                poke_u16(c, v + KG_TARU_C6, 1);
                let ft = first_fighter(c);
                poke_u32(c, v + KG_TARU_KEEP, ft);
            },
            &move |c, _| {
                c.addr = t;
                let v = vars(c);
                poke_u16(c, v + KG_TARU_C6, 1);
                let me = c.regs.gpr[3];
                poke_u32(c, v + KG_TARU_KEEP, me);
            },
            &move |c, _| {
                c.addr = t;
                let v = vars(c);
                poke_u16(c, v + KG_TARU_C6, 0);
            },
        ];
        for p in prepares {
            check_calls_with(&d, "grKongo_801D577C", 1, p, ours);
        }
    }
}

/// A floating platform's variables set.
fn kg_platform(c: &mut Call, state: i16, landings: i16, last: i16, ticks: i16) {
    let v = vars(c);
    poke_u16(c, v, state as u16);
    poke_u16(c, v + 2, landings as u16);
    poke_u16(c, v + 4, last as u16);
    poke_u16(c, v + 6, ticks as u16);
}

#[test]
fn kongo_platforms_and_logs() {
    let d = dir(KG);
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &|c, _| kg_platform(c, 0, 0, 0, 0),
        &|c, k| {
            let n = kg_param(c, 0x88) as i16;
            kg_platform(c, 1, n + [0, 1][k % 2], n, 3);
        },
        &|c, k| {
            let n = kg_param(c, 0x8C) as i16;
            kg_platform(c, 1, 0, 0, n + [0, -1][k % 2]);
        },
        &|c, _| kg_platform(c, 2, 0, 0, 0),
        &|c, _| kg_platform(c, 3, 0, 0, 0),
    ];
    for p in prepares {
        check_calls_with(&d, "grKongo_801D6198", 1, p, &by_address);
    }
    let behind = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(kongo::log_behind(r, gobj_arg(maps, call))))
    };
    check_calls_with(&d, "grKongo_801D7F78", 1, &as_saved, &behind);
    check_calls_with(&d, "grKongo_801D64BC", 1, &as_saved, &by_address);
    // A landing on a platform, of each ground kind, from its process's calls.
    let landed = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps
            .grounds
            .get(&call.regs.gpr[3])
            .expect("the platform's Ground");
        kongo::platform_landed(&mut r.game.grounds[g.0], call.regs.gpr[7] as i32);
        Ret::Nothing
    };
    let t = leaf_check::sym("fn_801D7E60");
    for kind in [1u32, 2] {
        let prep = move |c: &mut Call, _: usize| {
            c.addr = t;
            let gp = peek_u32(c, c.regs.gpr[3] + 0x2C);
            c.regs.gpr[3] = gp;
            c.regs.gpr[7] = kind;
        };
        check_calls_with(&d, "grKongo_801D6198", 1, &prep, &landed);
    }
}

/// The river path's spline: the one path's in the state (the calls run with one out).
fn river_spline(r: &mut Running) -> melee_data::archive::DataRef {
    use melee_sim::game::gr::GroundVars;
    r.game
        .grounds
        .iter()
        .find_map(|(_, g)| match &g.u {
            GroundVars::KongoPath(v) => v.xC4,
            _ => None,
        })
        .expect("a river path with its spline")
}

#[test]
fn kongo_path() {
    let d = dir(KG);
    // The path as saved, near its end, and at it.
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, _| {
            let v = vars(c);
            poke_u32(c, v + 8, 0.9995f32.to_bits());
        },
        &|c, _| {
            let v = vars(c);
            poke_u32(c, v + 8, 1.0f32.to_bits());
        },
    ];
    for p in prepares {
        check_calls_with(&d, "grKongo_801D6668", 1, p, &by_address);
    }
    // The spline's pose and tangent, written where the prepares point them: the stage's
    // guide points (compared, unused on Kongo Jungle).
    let pose = |r: &mut Running, _: &Maps, call: &Call| {
        let spline = river_spline(r);
        let (at, rot) =
            melee_sim::game::gr::ground::spline_pose(r.data, spline, call.regs.fpr[1].ps0 as f32);
        if let Some(at) = at {
            r.game.stage_info.x130 = at;
        }
        r.game.stage_info.x13C = rot;
        Ret::Nothing
    };
    let si = leaf_check::sym("stage_info");
    let out = move |c: &mut Call, k: usize| {
        c.regs.gpr[4] = si + 0x130;
        c.regs.gpr[5] = si + 0x13C;
        if k % 3 == 1 {
            c.regs.fpr[1].ps0 = 0.0;
        } else if k % 3 == 2 {
            c.regs.fpr[1].ps0 = 1.0;
        }
    };
    check_calls_with(&d, "Ground_801C4B50", 1, &out, &pose);
    let tangent = |r: &mut Running, _: &Maps, call: &Call| {
        let spline = river_spline(r);
        if let Some(v) =
            melee_sim::hsd::spline::tangent(r.data, spline, call.regs.fpr[1].ps0 as f32)
        {
            r.game.stage_info.x130 = v;
        }
        Ret::Nothing
    };
    let out = move |c: &mut Call, k: usize| {
        c.regs.gpr[3] = si + 0x130;
        c.regs.fpr[1].ps0 = [c.regs.fpr[1].ps0, 0.0, 1.0, 0.25, -0.5, 1.5][k % 6];
    };
    check_calls_with(&d, "lbShadow_8000E9F0", 1, &out, &tangent);
}

#[test]
fn kongo_shadow() {
    let d = dir(KG);
    fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
        let (pos, j) = shadow_args(maps, call);
        kongo::check_shadow_render(r.game, r.data, pos, j)
    }
    check_shadow(&d, "grKongo_801D844C", ours, 0.0);
}

/// The stage's callbacks no capture reaches, from the barrel's calls: the empty processes and
/// destroy callbacks, the demo and load hooks, and the path's destruction.
#[test]
fn kongo_unreached_callbacks() {
    let d = dir(KG);
    let from = "grKongo_801D577C";
    for target in [
        "grKongo_801D55D4",
        "grKongo_801D5FA4",
        "grKongo_801D5FDC",
        "grKongo_801D5FE0",
        "grKongo_801D6030",
        "grKongo_801D6034",
        "grKongo_801D606C",
        "grKongo_801D6070",
        "grKongo_801D6378",
        "grKongo_801D6518",
        "grKongo_801D69AC",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(target)), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grKongo_801D5238")),
        &demo,
    );
    let t = leaf_check::sym("grKongo_801D52F8");
    check_calls_with(&d, from, 1, &move |c, _| c.addr = t, &event);
}

#[test]
fn klap() {
    use melee_sim::game::it::kinds::klap;
    let d = dir(KG);
    let item = |f: fn(&mut Running, melee_sim::hsd::gobj::GObjId)| {
        move |r: &mut Running, maps: &Maps, call: &Call| {
            f(r, gobj_arg(maps, call));
            Ret::Nothing
        }
    };
    let pred = |f: fn(&mut Running, melee_sim::hsd::gobj::GObjId) -> bool| {
        move |r: &mut Running, maps: &Maps, call: &Call| {
            Ret::Int(u32::from(f(r, gobj_arg(maps, call))))
        }
    };
    for (name, f) in [
        ("it_802E1820", klap::spawned as fn(&mut Running, _)),
        ("itKlap_Logic10_Destroyed", klap::destroyed),
        ("it_802E1930", klap::swim),
        ("it_802E1C84", klap::fall),
        ("itKlap_UnkMotion2_Phys", klap::phys2),
        ("itKlap_UnkMotion4_Phys", klap::phys4),
        ("it_802E20D8", klap::enter_barrel),
        ("it_802E215C", klap::in_barrel),
    ] {
        check_calls_with(&d, name, 1, &as_saved, &item(f));
    }
    // Swimming or snapping: as saved and from many seeds (the switch drawn on a rewound
    // animation).
    for p in [&as_saved as &Prepare, &|c, k| {
        set_seed(c, (k as u32).wrapping_mul(0x2545_F491))
    }] {
        check_calls_with(&d, "itKlap_UnkMotion1_Phys", 1, p, &item(klap::phys1));
    }
    check_calls_with(
        &d,
        "itKlap_UnkMotion1_Coll",
        1,
        &as_saved,
        &pred(klap::coll1),
    );
    // What the match never did to it, from its calls: snapping (from swimming's), and a hit
    // taken and its knockback (from falling's and spinning's, with a knockback set).
    let t = leaf_check::sym("it_802E1C4C");
    check_calls_with(
        &d,
        "it_802E1930",
        1,
        &move |c, _| c.addr = t,
        &item(klap::snap),
    );
    for (target, f) in [
        ("it_2725_Logic10_DmgReceived", None),
        ("it_802E1E94", Some(klap::knocked as fn(&mut Running, _))),
    ] {
        let t = leaf_check::sym(target);
        let hit = move |c: &mut Call, k: usize| {
            c.addr = t;
            let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
            poke_u32(c, ip + 0xCC8, [0.0f32, 30.0, 120.0][k % 3].to_bits());
            poke_u32(c, ip + 0xCAC, [45u32, 361, 270][k % 3]);
            poke_u32(c, ip + 0xCCC, [1.0f32, -1.0][k % 2].to_bits());
        };
        for from in ["itKlap_UnkMotion2_Phys", "itKlap_UnkMotion4_Phys"] {
            match f {
                Some(f) => check_calls_with(&d, from, 1, &hit, &item(f)),
                None => check_calls_with(&d, from, 1, &hit, &pred(klap::damaged)),
            }
        }
    }
    let spawn = |r: &mut Running, maps: &Maps, call: &Call| {
        let path = maps.gobjs.get(&call.regs.gpr[3]).copied();
        let _ = klap::spawn(r, path);
        Ret::Nothing
    };
    check_calls_with(&d, "it_802E18B4", 1, &as_saved, &spawn);
    let shot = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let pos = Vec3::new(
            f32_at(call, g[4]),
            f32_at(call, g[4] + 4),
            f32_at(call, g[4] + 8),
        );
        let w = |o: u32| peek_u32(call, g[5] + o);
        let hit = melee_sim::game::structs::HitCapsule {
            state: w(0) as i32,
            damage: w(4) as f32,
            kb_angle: w(8) as i32,
            x24: w(0xC),
            x28: w(0x10),
            x2C: w(0x14),
            element: w(0x18),
            sfx_severity: w(0x1C) as i32,
            sfx_kind: w(0x20) as i32,
            unk_count: w(4),
            ..Default::default()
        };
        klap::shot(
            r,
            gobj_arg(maps, call),
            pos,
            hit,
            call.regs.fpr[1].ps0 as f32,
        );
        Ret::Nothing
    };
    for p in [&as_saved as &Prepare, &|c, k| {
        c.regs.fpr[1].ps0 = [-45.0, 0.0, 361.0, 90.0][k % 4]
    }] {
        check_calls_with(&d, "it_802E2330", 1, p, &shot);
    }
    let forget = |r: &mut Running, maps: &Maps, call: &Call| {
        let other = maps.gobjs.get(&call.regs.gpr[4]).copied();
        klap::evt_unk(r, gobj_arg(maps, call), other);
        Ret::Nothing
    };
    let t = leaf_check::sym("it_802E2450");
    let with_fighter = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[4] = first_fighter(c);
    };
    check_calls_with(&d, "itKlap_UnkMotion2_Phys", 1, &with_fighter, &forget);
    let kb = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let w = |o: u32| peek_u32(call, g[4] + o);
        let hit = melee_sim::game::structs::HitCapsule {
            damage: f32::from_bits(w(0x8)),
            x24: w(0x24),
            x28: w(0x28),
            x2C: w(0x2C),
            ..Default::default()
        };
        let gobj = (maps.gobjs.iter())
            .find(|&(&a, _)| peek_u32(call, a + 0x2C) == g[3])
            .map(|(_, &o)| o)
            .expect("the item's GObj");
        Ret::Float(f64::from(melee_sim::game::it::coll::knockback_of(
            r, gobj, &hit,
        )))
    };
    check_calls_with(&d, "it_80270CD8", 1, &as_saved, &kb);
    // The states' trivial callbacks, from the falling Klap Trap's calls.
    for target in [
        "itKlap_UnkMotion1_Anim",
        "itKlap_UnkMotion2_Anim",
        "itKlap_UnkMotion3_Anim",
        "itKlap_UnkMotion4_Anim",
        "itKlap_UnkMotion2_Coll",
        "itKlap_UnkMotion3_Coll",
        "itKlap_UnkMotion4_Coll",
    ] {
        let t = leaf_check::sym(target);
        check_calls_with(
            &d,
            "itKlap_UnkMotion2_Phys",
            1,
            &move |c, _| c.addr = t,
            &pred(klap::answers_false),
        );
    }
    let t = leaf_check::sym("itKlap_UnkMotion3_Phys");
    check_calls_with(
        &d,
        "itKlap_UnkMotion2_Phys",
        1,
        &move |c, _| c.addr = t,
        &item(klap::phys3),
    );
}

// The stages' barrels' ride (`ftCo_BarrelWait.c`, `Ground_801C4DD0`, `Ground_801C4E20`), from
// the barrel set's two matches, in which scripted walkers fall off Kongo Jungle N64 and Kongo
// Jungle into their barrels and are shot out ([the oracle](../../docs/oracle.md#stages), "The
// barrels' ride"). Captured as the stage sets are, from
// `$SSBM_RS_LOCAL/bench/oracle-m4/NAME.slp` into `calls/stages/oracle-NAME`, with
// `CAPTURE_FUNCS=@$D/barrel-funcs.txt` and `LOCKSTEP_DONE=$D/done-barrel.txt` (every other
// function).

/// The sets, each with where its barrel keeps what it holds in `Ground::u` (Kongo Jungle N64's
/// `keep` at 0x10, Kongo Jungle's at 0xC).
const BARREL_SETS: [(&str, u32); 2] = [
    ("oracle-barrel-kongo-jungle-64", 0x10),
    ("oracle-barrel-kongo-jungle", 0xC),
];

/// A fighter's callback (a state's, or a hook), run by its address.
fn by_fighter_cb(r: &mut Running, maps: &Maps, call: &Call) -> Ret {
    run_fighter_cb(r, gobj_arg(maps, call), CodeRef(call.addr));
    Ret::Nothing
}

/// The fighter's `Fighter*` in a call made with its GObj in r3.
fn fighter_of(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[3] + 0x2C)
}

/// The stage's barrel's variables (`stage_info.x72C`'s `Ground::u`).
fn barrel_vars(c: &Call) -> u32 {
    let g = peek_u32(c, leaf_check::sym("stage_info") + 0x72C);
    peek_u32(c, g + 0x2C) + 0xC4
}

/// The barrel's hold state (`xC6`, 1 holding) and what it holds (`keep`, at `keep_at`).
fn barrel_hold(c: &mut Call, keep_at: u32, state: u16, keep: u32) {
    let v = barrel_vars(c);
    poke_u16(c, v + 2, state);
    poke_u32(c, v + keep_at, keep);
}

/// The hit description (`lbColl_80008D30_arg1`) at `addr`, as `lbColl_80008D30` copies it into
/// the shot's hit.
fn hit_desc(c: &Call, addr: u32) -> HitCapsule {
    let w = |i: u32| peek_u32(c, addr + 4 * i);
    HitCapsule {
        state: w(0) as i32,
        damage: w(1) as f32,
        kb_angle: w(2) as i32,
        x24: w(3),
        x28: w(4),
        x2C: w(5),
        element: w(6),
        sfx_severity: w(7) as i32,
        sfx_kind: w(8) as i32,
        unk_count: w(1),
        ..HitCapsule::default()
    }
}

#[test]
fn barrel_ride() {
    let fp_of = fighter_of;
    // Taken in (`ftCo_8009EB18`): as saved, and from every state the catch loop saw a fighter
    // in (walking, falling, tumbling, holding an item or not), and again from inside.
    let enter = |r: &mut Running, maps: &Maps, call: &Call| {
        r.enter_stage_barrel(gobj_arg(maps, call));
        Ret::Nothing
    };
    let t = leaf_check::sym("ftCo_8009EB18");
    for (set, keep_at) in BARREL_SETS {
        let d = dir(set);
        // The barrel's own process from these matches' states, with fighters about it and in
        // it, which the stage matches never had.
        let process = if keep_at == 0x10 {
            "stageGObj1_GObjProc"
        } else {
            "grKongo_801D577C"
        };
        check_calls_with(&d, process, 1, &as_saved, &by_address);
        check_calls_with(&d, "ftCo_8009EB18", 1, &as_saved, &enter);
        for from in ["ftColl_8007BA0C", "ftCo_BarrelWait_IASA", "ftCo_800C0874"] {
            check_calls_with(&d, from, 1, &retarget(t), &enter);
        }
        // Waiting: the empty callbacks, keeping to the barrel, and A or B (or neither, or
        // another button) asking it to shoot, with the barrel holding the fighter or not.
        for f in [
            "ftCo_BarrelWait_Anim",
            "ftCo_BarrelWait_Phys",
            "ftCo_BarrelWait_Coll",
            "ftCo_8009EC44",
            "ftCo_BarrelWait_IASA",
        ] {
            check_calls_with(&d, f, 1, &as_saved, &by_fighter_cb);
        }
        for buttons in [0x100u32, 0x200, 0x300, 0, 0x10] {
            for state in [1u16, 0, 2] {
                let press = move |c: &mut Call, _: usize| {
                    let fp = fp_of(c);
                    poke_u32(c, fp + 0x668, buttons);
                    let me = c.regs.gpr[3];
                    barrel_hold(c, keep_at, state, me);
                };
                check_calls_with(&d, "ftCo_BarrelWait_IASA", 1, &press, &by_fighter_cb);
            }
        }
        // The death hook (`ftCo_8009EAF8`) and the stage's two barrel calls, from the waiting
        // fighter's calls: the barrel holding it, holding another fighter's GObj, its shot
        // started, and empty.
        let hold = |c: &mut Call, k: usize| {
            let me = c.regs.gpr[3];
            let other = first_fighter(c);
            let (state, keep) = [(1u16, me), (1, other), (2, me), (0, 0)][k % 4];
            barrel_hold(c, keep_at, state, keep);
        };
        let t = leaf_check::sym("ftCo_8009EAF8");
        let death = move |c: &mut Call, k: usize| {
            c.addr = t;
            hold(c, k);
        };
        check_calls_with(&d, "ftCo_BarrelWait_IASA", 1, &death, &by_fighter_cb);
        let release = |r: &mut Running, _: &Maps, _: &Call| {
            Ret::Int(u32::from(barrel_wait::barrel_release(r)))
        };
        let letgo =
            |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(barrel_wait::barrel_letgo(r)));
        for (target, ours) in [
            (
                "Ground_801C4DD0",
                &release as &(dyn Fn(&mut Running, &Maps, &Call) -> Ret + Sync),
            ),
            ("Ground_801C4E20", &letgo),
        ] {
            let t = leaf_check::sym(target);
            check_calls_with(&d, target, 0, &as_saved, ours);
            let prep = move |c: &mut Call, k: usize| {
                c.addr = t;
                hold(c, k);
            };
            check_calls_with(&d, "ftCo_BarrelWait_IASA", 1, &prep, ours);
        }
        // Shot out (`ftCo_8009EC70`): as saved, at angles all round (negative too) and with the
        // fighter facing either way, and with the shot's damage and knockback changed.
        let shoot = |r: &mut Running, maps: &Maps, call: &Call| {
            let pos = vec_at(call, call.regs.gpr[4]);
            let hit = hit_desc(call, call.regs.gpr[5]);
            let angle = call.regs.fpr[1].ps0 as f32;
            r.shoot_from_stage_barrel(gobj_arg(maps, call), pos, &hit, angle);
            Ret::Nothing
        };
        check_calls_with(&d, "ftCo_8009EC70", 1, &as_saved, &shoot);
        for angle in [-45.0f32, 0.0, 30.0, 90.0, 180.0, 270.0, 359.5] {
            for facing in [1.0f32, -1.0] {
                let turn = move |c: &mut Call, _: usize| {
                    c.regs.fpr[1].ps0 = f64::from(angle);
                    let fp = fp_of(c);
                    poke_u32(c, fp + 0x2C, facing.to_bits());
                };
                check_calls_with(&d, "ftCo_8009EC70", 1, &turn, &shoot);
            }
        }
        // Damage, growth and set knockback: none at all, a little, and set knockback alone.
        for (damage, growth, set) in [(0u32, 50u32, 0u32), (7, 100, 0), (40, 0, 80)] {
            let power = move |c: &mut Call, _: usize| {
                let h = c.regs.gpr[5];
                poke_u32(c, h + 4, damage);
                poke_u32(c, h + 0xC, growth);
                poke_u32(c, h + 0x10, set);
            };
            check_calls_with(&d, "ftCo_8009EC70", 1, &power, &shoot);
        }
    }
}

/// A vector argument, the three floats at `addr` in the call's memory.
fn vec_at(call: &Call, addr: u32) -> Vec3 {
    Vec3::new(
        f32_at(call, addr),
        f32_at(call, addr + 4),
        f32_at(call, addr + 8),
    )
}

// Green Greens (`grgreens.c`) and Whispy's apples (`itwhispyapple.c`).

const GREENS: &str = "oracle-stage-green-greens";

/// grgreens.c's `yakumono_param`, its demo and waiting flags.
const GREENS_PARAM: u32 = 0x804D_6AA8;
const GREENS_DEMO: u32 = 0x804D_6AAC;
const GREENS_WAITING: u32 = 0x804D_6AAD;

/// The parameters' word at `off`.
fn greens_param(c: &Call, off: u32) -> u32 {
    peek_u32(c, peek_u32(c, GREENS_PARAM) + off)
}

#[test]
fn greens_setup() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    check_hazard(&d, "grGreens_Init", 1, &as_saved, &by_event);
    check_hazard(&d, "grGreens_802134F8", 1, &as_saved, &by_event);
    // The block objects the blocks' tick makes, and every map's object from the same calls.
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = greens::create(r, call.regs.gpr[3] as i32, greens::PROC_BLOCK_FRAME);
        Ret::Nothing
    };
    check_hazard(&d, "grGreens_80213524", 1, &as_saved, &create);
    for map in [0u32, 1, 2, 3, 4] {
        check_hazard(
            &d,
            "grGreens_80213524",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "grGreens_8021360C",
        "grGreens_802138BC",
        "stageGObj1_OnInit",
        "stageGObj3_OnInit",
        "grGreens_80213988",
        "grGreens_802139C4",
        "grGreens_80214674",
        "fn_80214658",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
    // Map 0's setup after a demo's: the start points' joints moved.
    let demo = |c: &mut Call, _: usize| poke_u8(c, GREENS_DEMO, 1);
    check_hazard(&d, "grGreens_8021360C", 1, &demo, &by_address);
    // The blocks' setup after a demo's keeps the last layout.
    check_hazard(&d, "grGreens_80214674", 1, &demo, &by_address);
    let grid = |r: &mut Running, maps: &Maps, call: &Call| {
        greens::measure_grid(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grGreens_8021483C", 1, &as_saved, &grid);
    let lay_out = |r: &mut Running, _: &Maps, _: &Call| {
        greens::lay_out(r);
        Ret::Nothing
    };
    let seeds = |c: &mut Call, k: usize| set_seed(c, 0x1234_5679u32.wrapping_mul(k as u32 + 1));
    for p in [&as_saved as &Prepare, &seeds] {
        check_hazard(&d, "grGreens_80214B58", 1, p, &lay_out);
    }
    let place = |r: &mut Running, maps: &Maps, call: &Call| {
        greens::place_blocks(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grGreens_80214FA8", 1, &as_saved, &place);
}

/// Whispy's variables set: state, step, count, whether the state is new, side, apples left and
/// the next apple's side.
#[allow(clippy::too_many_arguments)]
fn whispy(
    c: &mut Call,
    state: i32,
    step: i32,
    t: i32,
    new: i32,
    side: i32,
    apples: i32,
    aside: i32,
) {
    let v = map_vars(c, 5);
    for (off, x) in [
        (4, state),
        (8, step),
        (0xC, t),
        (0x10, new),
        (0x14, side),
        (0x20, apples),
        (0x24, aside),
    ] {
        poke_u32(c, v + off, x as u32);
    }
}

#[test]
fn greens_whispy() {
    let d = hazard_dir(GREENS);
    let f = "grGreens_80213C10";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    check_hazard(&d, f, 1, &|c, _| poke_u8(c, GREENS_DEMO, 1), &by_address);
    check_hazard(&d, f, 1, &|c, _| poke_u8(c, GREENS_WAITING, 1), &by_address);
    // Idle: new, counting down, and at its end; blowing: new (turned to the fighters), then each
    // step with its count around its ends; dropping apples: new, at the first apple's tick and
    // an interval after, between them, and with none left; and a state past the three.
    let breath = |c: &Call| f32::from_bits(greens_param(c, 0x50)) as i32;
    let blow_max = |c: &Call| f32::from_bits(greens_param(c, 0x54)) as i32;
    let blow_min = |c: &Call| f32::from_bits(greens_param(c, 0x58)) as i32;
    let first = |c: &Call| greens_param(c, 0x64) as i32;
    let every = |c: &Call| greens_param(c, 0x68) as i32;
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(|c, _| whispy(c, 0, 0, 30, 1, 0, 0, 0)),
        Box::new(|c, _| whispy(c, 0, 3, 30, 0, 0, 0, 0)),
        Box::new(|c, _| whispy(c, 0, 3, 1, 0, 1, 0, 0)),
        Box::new(|c, _| whispy(c, 0, 7, -4, 0, 1, 0, 0)),
        Box::new(|c, _| whispy(c, 1, 0, 0, 1, 0, 0, 0)),
        Box::new(|c, k| whispy(c, 1, (k % 7) as i32, 0, 0, (k % 2) as i32, 0, 0)),
        Box::new(move |c, _| whispy(c, 1, 2, breath(c), 0, 0, 0, 0)),
        Box::new(move |c, _| whispy(c, 1, 2, breath(c), 0, 1, 0, 0)),
        Box::new(move |c, _| whispy(c, 1, 4, blow_max(c), 0, 1, 0, 0)),
        Box::new(move |c, _| whispy(c, 1, 4, blow_min(c), 0, 0, 0, 0)),
        Box::new(move |c, _| whispy(c, 1, 4, blow_min(c), 0, 1, 0, 0)),
        Box::new(|c, _| whispy(c, 2, 0, 0, 1, 0, 0, 0)),
        Box::new(move |c, _| whispy(c, 2, 0, first(c), 0, 0, 3, 0)),
        Box::new(move |c, _| whispy(c, 2, 0, first(c) + every(c), 0, 0, 2, 1)),
        Box::new(move |c, _| whispy(c, 2, 0, first(c) + 1, 0, 0, 2, 1)),
        Box::new(move |c, _| whispy(c, 2, 0, first(c), 0, 0, -1, 0)),
        Box::new(|c, _| whispy(c, 3, 0, 0, 0, 0, 0, 0)),
    ];
    for p in &prepares {
        check_hazard(&d, f, 1, p.as_ref(), &by_address);
    }
}

#[test]
fn greens_wind() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    // The push goes where the prepares point the callback's vector: the stage's barrel place,
    // compared state that nothing on Green Greens uses.
    let wind = |r: &mut Running, maps: &Maps, call: &Call| {
        let ground = gobj_arg(maps, call);
        let fighter = *maps
            .gobjs
            .get(&call.regs.gpr[4])
            .expect("the fighter's GObj");
        let push = greens::wind_push(r, ground, fighter);
        r.game.stage_info.x730 = push.unwrap_or(Vec3::ZERO);
        Ret::Int(u32::from(push.is_some()))
    };
    let out = |c: &mut Call| c.regs.gpr[5] = leaf_check::sym("stage_info") + 0x730;
    // Whispy blowing either way or not, the fighter at the box's middle on his side, outside
    // it, and on its edge.
    let at = move |c: &mut Call, blowing: i32, x: f32, y: f32| {
        out(c);
        poke_u32(c, map_vars(c, 5) + 0x18, blowing as u32);
        let fp = peek_u32(c, c.regs.gpr[4] + 0x2C);
        poke_u32(c, fp + 0xB0, x.to_bits());
        poke_u32(c, fp + 0xB0 + 4, y.to_bits());
    };
    let p = |c: &Call, off: u32| f32::from_bits(greens_param(c, off));
    let mid_y = move |c: &Call| (p(c, 0x48) + p(c, 0x4C)) / 2.0;
    let mid_x = move |c: &Call| (p(c, 0x40) + p(c, 0x44)) / 2.0;
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(move |c, _| out(c)),
        Box::new(move |c, _| at(c, 1, -mid_x(c), mid_y(c))),
        Box::new(move |c, _| at(c, 2, mid_x(c), mid_y(c))),
        Box::new(move |c, _| at(c, 1, mid_x(c), mid_y(c))),
        Box::new(move |c, _| at(c, 2, mid_x(c), p(c, 0x48))),
        Box::new(move |c, _| at(c, 2, p(c, 0x40), mid_y(c))),
        Box::new(move |c, _| at(c, 0, mid_x(c), mid_y(c))),
        Box::new(move |c, _| at(c, 3, mid_x(c), mid_y(c))),
    ];
    for pr in &prepares {
        check_hazard(&d, "fn_80213B1C", 1, pr.as_ref(), &wind);
    }
    let in_box = |_: &mut Running, _: &Maps, call: &Call| {
        let v = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, v), f32_at(call, v + 4), f32_at(call, v + 8));
        let f = |i: usize| call.regs.fpr[i].ps0 as f32;
        Ret::Int(u32::from(greens::in_box(pos, f(1), f(2), f(3), f(4))))
    };
    // The bounds in either order, and the point inside, outside and on an edge.
    let bounds = |c: &mut Call, k: usize| {
        let v = c.regs.gpr[3];
        let b = [(-60.0f32, -20.0f32), (-20.0, -60.0)][k % 2];
        let y = [(80.0f32, 0.0f32), (0.0, 80.0)][k / 2 % 2];
        let fpr = &mut c.regs.fpr;
        (fpr[1].ps0, fpr[2].ps0, fpr[3].ps0, fpr[4].ps0) = (
            f64::from(b.0),
            f64::from(b.1),
            f64::from(y.0),
            f64::from(y.1),
        );
        let x = [-40.0f32, -10.0, -60.0, -20.0001][k % 4];
        poke_u32(c, v, x.to_bits());
        poke_u32(c, v + 4, [40.0f32, 0.0, 79.9][k % 3].to_bits());
    };
    for pr in [&as_saved as &Prepare, &bounds] {
        check_hazard(&d, "grGreens_80213AB4", 1, pr, &in_box);
    }
}

/// The address of the block at `row` and `col` in a call's state.
fn greens_block(c: &Call, row: u32, col: u32) -> u32 {
    peek_u32(c, map_vars(c, 6) + 8) + 0x20 * (row * 6 + col)
}

/// A block's status (the top four bits) and flag `x1_n` (`n` from 1 to 7).
fn set_block_status(c: &mut Call, row: u32, col: u32, status: u32) {
    let a = greens_block(c, row, col);
    let w = peek_u32(c, a);
    poke_u32(c, a, (w & 0x0FFF_FFFF) | status << 28);
}

fn set_block_flag(c: &mut Call, row: u32, col: u32, n: u32, v: bool) {
    let a = greens_block(c, row, col);
    let w = peek_u32(c, a);
    let bit = 1 << (23 - n);
    poke_u32(c, a, if v { w | bit } else { w & !bit });
}

fn block_status(c: &Call, row: u32, col: u32) -> u32 {
    peek_u32(c, greens_block(c, row, col)) >> 28
}

/// The first block in use, row by row.
fn first_block(c: &Call) -> Option<(u32, u32)> {
    (0..30)
        .map(|i| (i / 6, i % 6))
        .find(|&(r, k)| block_status(c, r, k) != 0)
}

#[test]
fn greens_blocks() {
    let d = hazard_dir(GREENS);
    let f = "grGreens_802166C4";
    let tick = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::greens::proc_blocks(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    // A new block due now (from several seeds), with a side full (a block at the top of its
    // first column) or both; and a block that broke.
    let due = |c: &mut Call, k: usize| {
        poke_u32(c, map_vars(c, 6) + 0xC, (-1i32) as u32);
        set_seed(c, 0x0F0F_0F0Fu32.wrapping_mul(k as u32 + 3));
    };
    let full = move |c: &mut Call, k: usize, left: bool, right: bool| {
        due(c, k);
        if left {
            set_block_status(c, 4, 0, 3);
        }
        if right {
            set_block_status(c, 4, 3, 3);
        }
    };
    let broke = |c: &mut Call, _: usize| {
        if let Some((r, k)) = first_block(c) {
            set_block_flag(c, r, k, 3, true);
        }
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &due,
        &move |c, k| full(c, k, true, false),
        &move |c, k| full(c, k, false, true),
        &move |c, k| full(c, k, true, true),
        &broke,
    ];
    for p in prepares {
        check_hazard(&d, f, 1, p, &tick);
    }
    // The stage's process, its blocks waiting for the start or not.
    let f = "grGreens_8021479C";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    let waiting = |c: &mut Call, _: usize| {
        let v = map_vars(c, 6);
        poke_u32(c, v, peek_u32(c, v) | 0x8000_0000);
    };
    check_hazard(&d, f, 1, &waiting, &by_address);
}

/// A block's fall: its speed and height (above its place by `dy`), and its status.
fn falling(c: &mut Call, row: u32, col: u32, status: u32, speed: f32, dy: f32) {
    set_block_status(c, row, col, status);
    let a = greens_block(c, row, col);
    let grid = peek_u32(c, map_vars(c, 6) + 4) + 0xC * (row * 6 + col);
    let y = f32_at(c, grid + 4) + dy;
    poke_u32(c, a + 4, speed.to_bits());
    poke_u32(c, a + 8, y.to_bits());
}

#[test]
fn greens_block_moves() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    let update = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        greens::update_block(r, gobj_arg(maps, call), g[4] as i32, g[5] as i32);
        Ret::Nothing
    };
    let here = |c: &Call| (c.regs.gpr[5], c.regs.gpr[4]);
    // The called block falling (slowly, fast, landing now, landed on), on a falling block
    // below, bombing, broken, and resting with falling blocks above it.
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 0.0, 40.0);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 50.0, 40.0);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 1.0, 0.25);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 1.0, 20.0);
                set_block_flag(c, r, k, 4, true);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if r > 0 && block_status(c, r, k) != 0 && block_status(c, r - 1, k) != 0 {
                falling(c, r - 1, k, 1, 1.0, 30.0);
                falling(c, r, k, 1, 1.0, 31.0);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                set_block_flag(c, r, k, 2, true);
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            set_block_flag(c, r, k, 3, true);
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 1.0, 0.25);
                for up in r + 1..5 {
                    if block_status(c, up, k) != 0 {
                        set_block_status(c, up, k, 2);
                    }
                }
            }
        }),
        Box::new(move |c, _| {
            let (r, k) = here(c);
            if block_status(c, r, k) != 0 {
                falling(c, r, k, 1, 1.0, 20.0);
                for up in r + 1..5 {
                    if block_status(c, up, k) != 0 {
                        set_block_status(c, up, k, 2);
                    }
                }
            }
        }),
    ];
    let f = "grGreens_80215ED8";
    check_hazard(&d, f, 1, &as_saved, &update);
    for p in &prepares {
        check_hazard(&d, f, 1, p.as_ref(), &update);
    }
    // A block added: as saved, the other kind, and at rest or falling.
    let add = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        greens::add_block(
            r,
            gobj_arg(maps, call),
            g[4] as i32,
            g[5] as i32,
            g[6] as i32,
            g[7],
            greens::PROC_BLOCK_FRAME,
        );
        Ret::Nothing
    };
    let f = "grGreens_80215358";
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, _| c.regs.gpr[6] = 3 - c.regs.gpr[6],
        &|c, _| c.regs.gpr[7] = if c.regs.gpr[7] == 1 { 3 } else { 1 },
    ];
    for p in prepares {
        check_hazard(&d, f, 1, p, &add);
    }
    // A column settling, as saved and with falling blocks over an empty place.
    let settle = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        greens::settle_column(r, gobj_arg(maps, call), g[4] as i32, g[5] as i32);
        Ret::Nothing
    };
    let gap = |c: &mut Call, k: usize| {
        let col = c.regs.gpr[4];
        let row = (k % 3) as u32;
        set_block_status(c, row, col, 0);
        for up in row + 1..5 {
            if block_status(c, up, col) != 0 {
                set_block_status(c, up, col, [1, 2][up as usize % 2]);
            }
        }
    };
    for p in [&as_saved as &Prepare, &gap] {
        check_hazard(&d, "grGreens_80215D54", 1, p, &settle);
    }
}

#[test]
fn greens_block_links() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    let corners = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        greens::link_corners(r, gobj_arg(maps, call), g[4] as i32, g[5] as i32);
        Ret::Nothing
    };
    // Every neighbour resting, only the diagonals, only the sides, and the grid's first place
    // with its corner's neighbours, which reads before the blocks' array.
    let around = move |c: &mut Call, mask: u32| {
        let (col, row) = (c.regs.gpr[4] as i32, c.regs.gpr[5] as i32);
        let mut i = 0;
        for dr in -1..=1 {
            for dc in -1..=1 {
                if (dr, dc) == (0, 0) {
                    continue;
                }
                let (r, k) = (row + dr, col + dc);
                if (0..5).contains(&r) && (0..6).contains(&k) {
                    let s = if mask >> i & 1 != 0 { 3 } else { 0 };
                    set_block_status(c, r as u32, k as u32, s);
                }
                i += 1;
            }
        }
    };
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &move |c, _| around(c, 0xFF),
        &move |c, _| around(c, 0b1010_0101),
        &move |c, _| around(c, 0b0101_1010),
        &move |c, _| {
            c.regs.gpr[4] = 0;
            c.regs.gpr[5] = 0;
            set_block_status(c, 1, 1, 3);
            set_block_status(c, 1, 0, 0);
        },
    ];
    for p in prepares {
        check_hazard(&d, "grGreens_802150C4", 1, p, &corners);
    }
    let links = |r: &mut Running, maps: &Maps, call: &Call| {
        greens::link_blocks(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    // Every resting block to link, and every empty place's neighbours.
    let all = |c: &mut Call, k: usize| {
        for i in 0..30u32 {
            let (r, col) = (i / 6, i % 6);
            match block_status(c, r, col) {
                3 => set_block_flag(c, r, col, 5, true),
                0 if k.is_multiple_of(2) => set_block_flag(c, r, col, 6, true),
                _ => {}
            }
        }
    };
    for p in [&as_saved as &Prepare, &all] {
        check_hazard(&d, "grGreens_80216C20", 1, p, &links);
    }
}

#[test]
fn greens_block_hits() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    let ground = |maps: &Maps, a: u32| *maps.grounds.get(&a).expect("the stage's object");
    let brk = move |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let by = maps.gobjs.get(&g[6]).copied();
        greens::break_block(r, ground(maps, g[3]), g[4] as i32, g[5] as i32, by);
        Ret::Nothing
    };
    // As saved, the other kind of block, and one already broken.
    let prepares: [&Prepare; 3] = [
        &as_saved,
        &|c, _| {
            let (col, row) = (c.regs.gpr[4], c.regs.gpr[5]);
            let a = greens_block(c, row, col);
            poke_u32(c, a, peek_u32(c, a) ^ 1 << 22);
        },
        &|c, _| {
            let (col, row) = (c.regs.gpr[4], c.regs.gpr[5]);
            set_block_flag(c, row, col, 7, true);
        },
    ];
    for p in prepares {
        check_hazard(&d, "grGreens_802159B8", 1, p, &brk);
    }
    let hit = move |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let by = maps.gobjs.get(&g[6]).copied();
        greens::block_hit(r, gobj_arg(maps, call), ground(maps, g[4]), by);
        Ret::Nothing
    };
    check_hazard(&d, "fn_80215B84", 1, &as_saved, &hit);
    let landed = move |r: &mut Running, maps: &Maps, call: &Call| {
        let g = ground(maps, call.regs.gpr[3]);
        greens::block_landed(&mut r.game.grounds[g.0], call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    // On its own joint, another block's, and a joint no block has.
    let joints = |c: &mut Call, k: usize| c.regs.gpr[4] = [3u32, 17, 40][k % 3];
    for p in [&as_saved as &Prepare, &joints] {
        check_hazard(&d, "fn_80216DE4", 1, p, &landed);
    }
    // The items' callbacks that do nothing, from the hits' calls.
    let nothing = move |r: &mut Running, maps: &Maps, call: &Call| {
        greens::block_item_nothing(r, gobj_arg(maps, call), ground(maps, call.regs.gpr[4]));
        Ret::Nothing
    };
    for name in ["fn_802159B4", "fn_80215D50"] {
        check_hazard(
            &d,
            "fn_80215B84",
            1,
            &retarget(leaf_check::sym(name)),
            &nothing,
        );
    }
    // The stage glue the blocks use: a state change (the bomb's explosion), whether its
    // script is done, and an item destroyed.
    let state = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        melee_sim::game::gr::material::item_state(r, g, call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    check_hazard(&d, "grMaterial_801C8D98", 1, &as_saved, &state);
    let done = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        Ret::Int(u32::from(melee_sim::game::gr::material::item_script_done(
            r.game, g,
        )))
    };
    check_hazard(&d, "grMaterial_801C8E48", 1, &as_saved, &done);
    let destroy = |r: &mut Running, maps: &Maps, call: &Call| {
        melee_sim::game::gr::material::destroy_item(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let to_destroy = retarget(leaf_check::sym("grMaterial_801C8CDC"));
    check_hazard(&d, "it_2E6A_UnkMotion19_Phys", 1, &to_destroy, &destroy);
}

#[test]
fn greens_unreached_callbacks() {
    use melee_sim::game::gr::greens;
    let d = hazard_dir(GREENS);
    let from = "grGreens_8021479C";
    for name in [
        "grGreens_802138B4",
        "grGreens_802138B8",
        "grGreens_80213908",
        "grGreens_8021390C",
        "grGreens_80213944",
        "grGreens_80213948",
        "grGreens_80213980",
        "grGreens_80213984",
        "grGreens_802139BC",
        "grGreens_802139C0",
        "grGreens_80214654",
        "grGreens_80214804",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grGreens_802134F4"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grGreens_80213458"));
    check_hazard(&d, from, 1, &demo, &by_demo);
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(greens::check_shadow_render()));
    let to_shadow = retarget(leaf_check::sym("grGreens_80216E6C"));
    check_hazard(&d, from, 1, &to_shadow, &shadow);
}

/// An apple's variables (`xDD4`) in a call made with it in r3.
fn apple_vars(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[3] + 0x2C) + 0xDD4
}

#[test]
fn whispy_apples() {
    use melee_sim::game::it::kinds::whispyapple;
    let d = hazard_dir(GREENS);
    let spawn = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let parent = maps.gobjs.get(&g[3]).copied();
        let pos = Vec3::new(
            f32_at(call, g[4]),
            f32_at(call, g[4] + 4),
            f32_at(call, g[4] + 8),
        );
        let f = |i: usize| call.regs.fpr[i].ps0 as f32;
        let _ = whispyapple::spawn(r, parent, pos, f(1), f(2));
        Ret::Nothing
    };
    // As saved, kept or bursting for sure, healing for sure (healing items off in the match,
    // so a plain apple), and from several seeds.
    let odds = |keep: f64, heal: f64| {
        move |c: &mut Call, _: usize| {
            c.regs.fpr[1].ps0 = keep;
            c.regs.fpr[2].ps0 = heal;
        }
    };
    let seeds = |c: &mut Call, k: usize| set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &odds(1.0, 0.0),
        &odds(0.0, 0.0),
        &odds(0.0, 1.0),
        &seeds,
    ];
    for p in prepares {
        check_hazard(&d, "it_802EE200", 1, p, &spawn);
    }
    let start = |r: &mut Running, maps: &Maps, call: &Call| {
        whispyapple::start(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "it_802EE374", 1, &as_saved, &start);
    let bursts = |c: &mut Call, _: usize| poke_u32(c, apple_vars(c) + 4, 1);
    for f in [
        "itWhispyapple_UnkMotion0_Anim",
        "itWhispyapple_UnkMotion0_Phys",
        "itWhispyapple_UnkMotion0_Coll",
        "it_802EE6A0",
        "itWhispyapple_UnkMotion1_Anim",
        "itWhispyapple_UnkMotion1_Coll",
    ] {
        for p in [&as_saved as &Prepare, &bursts] {
            check_hazard(&d, f, 1, p, &item_cb);
        }
    }
    // Falling apples at the stage's plane and short of it.
    let at_plane = |c: &mut Call, k: usize| {
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u32(c, ip + 0x4C + 8, [0.0f32, 0.5, -0.5][k % 3].to_bits());
        poke_u32(c, apple_vars(c) + 4, (k / 3 % 2) as u32);
    };
    check_hazard(&d, "itWhispyapple_UnkMotion0_Coll", 1, &at_plane, &item_cb);
    // Ageing: near its end, behind the stage, and burst.
    let age = |c: &mut Call, k: usize| {
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u32(c, ip + 0xD44, [1.0f32, 30.0, 400.0][k % 3].to_bits());
        poke_u32(c, ip + 0x4C + 8, [0.0f32, -5.0][k / 3 % 2].to_bits());
    };
    let burst_state = |c: &mut Call, _: usize| {
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        poke_u32(c, ip + 0x24, 7);
    };
    for p in [&age as &Prepare, &burst_state] {
        check_hazard(&d, "itWhispyapple_UnkMotion1_Anim", 1, p, &item_cb);
    }
    // The states and events no capture reached, from a lying apple's calls.
    for name in [
        "itWhispyapple_UnkMotion1_Phys",
        "fn_802EE7FC",
        "itWhispyapple_UnkMotion5_Anim",
        "itWhispyapple_UnkMotion5_Phys",
        "itWhispyapple_UnkMotion5_Coll",
        "it_802EEA08",
        "itWhispyapple_UnkMotion3_Anim",
        "itWhispyapple_UnkMotion3_Phys",
        "it_802EEB28",
        "itWhispyApple_Logic18_EnteredAir",
        "itWhispyapple_UnkMotion6_Anim",
        "itWhispyapple_UnkMotion6_Phys",
        "itWhispyapple_UnkMotion6_Coll",
        "it_802EED00",
        "itWhispyapple_UnkMotion7_Anim",
        "itWhispyapple_UnkMotion7_Phys",
        "itWhispyapple_UnkMotion7_Coll",
        "it_802EEED0",
        "it_802EEED8",
        "it_802EEF10",
        "it_802EEF30",
        "it_802EEF68",
    ] {
        let from = "itWhispyapple_UnkMotion1_Anim";
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &item_cb);
    }
    // Let go of, from a lying apple's calls with a fighter holding it.
    let drop_at = leaf_check::sym("it_802EEA70");
    let held = move |c: &mut Call, _: usize| {
        c.addr = drop_at;
        let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
        let f = first_fighter(c);
        poke_u32(c, ip + 0x518, f);
    };
    check_hazard(&d, "itWhispyapple_UnkMotion1_Anim", 1, &held, &item_cb);
    let forget = |r: &mut Running, maps: &Maps, call: &Call| {
        let other = maps.gobjs.get(&call.regs.gpr[4]).copied();
        whispyapple::forget(r, gobj_arg(maps, call), other);
        Ret::Nothing
    };
    let t = leaf_check::sym("it_802EEF88");
    let with_fighter = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[4] = first_fighter(c);
    };
    check_hazard(
        &d,
        "itWhispyapple_UnkMotion1_Anim",
        1,
        &with_fighter,
        &forget,
    );
}

// Brinstar (`grzebes.c`).

const ZEBES: &str = "oracle-stage-brinstar";

/// grzebes.c's `yakumono_param`, the pop wait, and the bubbles.
const ZEBES_PARAM: u32 = 0x804D_6990;
const ZEBES_POP_WAIT: u32 = 0x804D_6994;
const ZEBES_BUBBLES: u32 = 0x8049_F170;

/// The parameters' word at `off`.
fn zebes_param(c: &Call, off: u32) -> u32 {
    peek_u32(c, peek_u32(c, ZEBES_PARAM) + off)
}

#[test]
fn zebes_setup() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    check_hazard(&d, "grZebes_801D84A4", 1, &as_saved, &by_event);
    check_hazard(&d, "grZebes_801D852C", 1, &as_saved, &by_event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = zebes::create(r, call.regs.gpr[3] as i32, zebes::GROW_BUBBLE_FRAME);
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801D8558", 1, &as_saved, &create);
    for map in [0u32, 1, 2, 3, 4, 5] {
        check_hazard(
            &d,
            "grZebes_801D8558",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "grZebes_801D8644",
        "grZebes_801D9100",
        "stageGObj5_OnInit",
        "grZebes_801D9508",
        "grZebes_801D9798",
        "grZebes_801D9F30",
        "fn_801D94F0",
        "grZebes_801D9758",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
    let reset = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps.gobjs.get(&call.regs.gpr[3]).expect("the stage's GObj");
        zebes::reset_bubbles(r, g);
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DC9DC", 1, &as_saved, &reset);
    let start = |r: &mut Running, _: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        zebes::start_bubbles(r, g[3] as i32, g[4] as u8, zebes::SETUP_BUBBLE_FRAME);
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DC744", 1, &as_saved, &start);
    let splashes = |r: &mut Running, _: &Maps, _: &Call| {
        zebes::reset_splashes(r.game);
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DA3E8", 1, &as_saved, &splashes);
}

/// The lava's variables set: entry, wait, state, top speed, speed, target and level.
#[allow(clippy::too_many_arguments)]
fn lava(c: &mut Call, entry: i16, wait: i16, state: u32, top: f32, v: f32, to: f32, at: f32) {
    let a = map_vars(c, 8);
    poke_u32(c, a, (entry as u16 as u32) << 16 | wait as u16 as u32);
    poke_u32(c, a + 4, state);
    for (k, f) in [top, v, to, at].into_iter().enumerate() {
        poke_u32(c, a + 8 + 4 * k as u32, f.to_bits());
    }
}

#[test]
fn zebes_lava() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    let f = "grZebes_801D99E0";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Waiting (with and without the quake, at its end, on the table's last entry), starting
    // (2), speeding up toward a target above and below (and when it would overshoot), and
    // slowing to a stop (still going, overshooting, reached), and a state past them.
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(|c, _| lava(c, 0, 200, 1, 0.0, 0.0, 0.0, -50.0)),
        Box::new(|c, _| lava(c, 2, 100, 1, 0.0, 0.0, 0.0, -50.0)),
        Box::new(|c, k| lava(c, (k % 5) as i16, 0, 1, 0.0, 0.0, 0.0, -50.0)),
        Box::new(|c, _| lava(c, 29, 0, 1, 0.0, 0.0, 0.0, 10.0)),
        Box::new(|c, _| lava(c, 1, 3, 2, 0.5, 0.0, 20.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 3, 0.5, 0.1, 20.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 70, 3, 0.5, 0.4, -60.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 3, 0.5, 0.5, -49.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 4, 0.5, 0.3, 20.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 4, 0.5, 0.02, 20.0, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 4, 0.5, 0.3, -49.9, -50.0)),
        Box::new(|c, _| lava(c, 1, 3, 7, 0.5, 0.3, -49.9, -50.0)),
    ];
    for p in &prepares {
        check_hazard(&d, f, 1, p.as_ref(), &by_address);
    }
    let fog = |r: &mut Running, _: &Maps, call: &Call| {
        zebes::fog_color(r.game, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    let levels = |c: &mut Call, k: usize| {
        c.regs.fpr[1].ps0 = f64::from([-400.0f32, -320.0, -200.0, -80.0, 0.0, 90.0, 120.0][k % 7]);
    };
    for p in [&as_saved as &Prepare, &levels] {
        check_hazard(&d, "grZebes_801DA0C4", 1, p, &fog);
    }
    let light = |r: &mut Running, maps: &Maps, call: &Call| {
        zebes::light_color(r, gobj_arg(maps, call), call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    let forget = |c: &mut Call, k: usize| {
        poke_u32(c, map_vars(c, 8) + 0x18, 0);
        c.regs.fpr[1].ps0 = f64::from([-80.0f32, 0.0, 40.0][k % 3]);
    };
    for p in [&as_saved as &Prepare, &forget] {
        check_hazard(&d, "grZebes_801DA254", 1, p, &light);
    }
    // The device: the fighter above the lava, below it, and just fallen in (a splash, up to
    // the splashes' limit).
    let device = |r: &mut Running, maps: &Maps, call: &Call| {
        let fighter = *maps
            .gobjs
            .get(&call.regs.gpr[4])
            .expect("the fighter's GObj");
        let hit = zebes::lava_device(r, fighter);
        // The hit it hands back is the parameters' (their word at 0x2C), which the original
        // stores through r5.
        if let Some(h) = hit {
            let param = r.game.gr.zebes_yakumono.expect("yakumono_param");
            assert_eq!(Some(h), r.data.ptr(param.at(0x2C)));
            let word = peek_u32(call, peek_u32(call, ZEBES_PARAM) + 0x2C);
            r.game.stage_info.x730.x = f32::from_bits(word);
        }
        Ret::Int(u32::from(hit.is_some()))
    };
    let at = |c: &mut Call, now: f32, before: f32, splashes: Option<u16>| {
        c.regs.gpr[5] = leaf_check::sym("stage_info") + 0x730;
        let level = f32_at(c, leaf_check::sym("stage_info") + 0x724);
        let fp = peek_u32(c, c.regs.gpr[4] + 0x2C);
        poke_u32(c, fp + 0xB0 + 4, (level + now).to_bits());
        poke_u32(c, fp + 0xBC + 4, (level + before).to_bits());
        if let Some(n) = splashes {
            poke_u16(c, 0x804D_6998, n);
        }
    };
    let prepares: [&Prepare; 4] = [
        &move |c, _| at(c, 5.0, 5.0, None),
        &move |c, _| at(c, -5.0, -3.0, None),
        &move |c, _| at(c, -2.0, 4.0, None),
        &move |c, _| at(c, -2.0, 4.0, Some(10)),
    ];
    for p in prepares {
        check_hazard(&d, "grZebes_801DCBFC", 1, p, &device);
    }
    let floor = |r: &mut Running, _: &Maps, _: &Call| {
        Ret::Float(f64::from(zebes::camera_floor(&r.game.stage_info)))
    };
    check_hazard(&d, "grZebes_801DCCC8", 1, &as_saved, &floor);
}

/// A shaking part's state (at `a`) set: state and next, countdown, swing, speed and damage.
#[allow(clippy::too_many_arguments)]
fn acid(c: &mut Call, a: u32, state: u8, next: u8, t: i16, swing: f32, v: f32, dmg: f32) {
    let w = peek_u32(c, a + 0x20) & 0xFFFF;
    poke_u32(
        c,
        a,
        u32::from(state) << 24 | u32::from(next) << 16 | t as u16 as u32,
    );
    poke_u32(c, a + 8, swing.to_bits());
    poke_u32(c, a + 0xC, v.to_bits());
    poke_u32(c, a + 0x10, dmg.to_bits());
    poke_u32(c, a + 0x20, peek_u32(c, a + 0x20) & !0xFFFF_0000 | w);
}

#[test]
fn zebes_shaking() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    let shake = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = gobj_arg(maps, call);
        let ground = *maps
            .grounds
            .get(&peek_u32(call, call.regs.gpr[3] + 0x2C))
            .expect("ground");
        let st = call.regs.gpr[4] - peek_u32(call, call.regs.gpr[3] + 0x2C);
        let which = if st == 0xC4 + 4 {
            zebes::Acid::Stage
        } else {
            zebes::Acid::Island
        };
        let res = zebes::shake(
            r,
            g,
            ground,
            which,
            call.regs.gpr[5] as i32,
            call.regs.gpr[6] as i32,
        );
        Ret::Int(res as u32)
    };
    // Each state's start and tick: at rest (damage decaying at its count, swinging past either
    // side, settling, breaking past its limit), breaking, broken at its count's end, coming
    // back; and a state past them.
    let st = |c: &Call| c.regs.gpr[4];
    let max = |c: &Call| f32::from_bits(zebes_param(c, 0x4C));
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 1, 0, 0, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 0, 1, 0, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 1, 2, 0, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 2, 3, 0, 0.0, 0.0, 5.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 0, 0, 1000, 3.0, 0.5, 4.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 0, 0, 2, -3.0, -0.5, 0.5)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 0, 0, 2, 0.01, 0.001, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            let m = max(c);
            acid(c, a, 0, 0, 2, 0.0, 0.0, m + 1.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 2, 2, 0, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 2, 2, 5, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 3, 3, 0, 0.0, 0.0, 0.0)
        }),
        Box::new(move |c, _| {
            let a = st(c);
            acid(c, a, 9, 9, 0, 0.0, 0.0, 0.0)
        }),
    ];
    check_hazard(&d, "grZebes_801DA528", 1, &as_saved, &shake);
    for p in &prepares {
        check_hazard(&d, "grZebes_801DA528", 1, p.as_ref(), &shake);
    }
    let pillar = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let ground = *maps.grounds.get(&g[4]).expect("ground");
        let dir = Vec3::new(
            f32_at(call, g[5]),
            f32_at(call, g[5] + 4),
            f32_at(call, g[5] + 8),
        );
        zebes::pillar_hit(r, ground, dir, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_hazard(&d, "fn_801DA9D8", 1, &as_saved, &pillar);
    let island = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let ground = *maps.grounds.get(&g[4]).expect("ground");
        let dir = Vec3::new(
            f32_at(call, g[5]),
            f32_at(call, g[5] + 4),
            f32_at(call, g[5] + 8),
        );
        zebes::island_hit(r, ground, dir, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_hazard(&d, "fn_801DA9F0", 1, &as_saved, &island);
    check_hazard(&d, "grZebes_801D925C", 1, &as_saved, &by_address);
}

/// The organic platform's state, wait and regrowth count set.
fn organic(c: &mut Call, state: i16, wait: i16, count: i16, pop_wait: i16) {
    let v = map_vars(c, 6);
    poke_u32(c, v, (state as u16 as u32) << 16 | wait as u16 as u32);
    let w = peek_u32(c, v + 0x30);
    poke_u32(c, v + 0x30, (w & 0xFFFF_0000) | count as u16 as u32);
    let w = peek_u32(c, v + 0x34);
    poke_u32(c, v + 0x34, (w & 0xFFFF) | (pop_wait as u16 as u32) << 16);
}

#[test]
fn zebes_stage() {
    let d = hazard_dir(ZEBES);
    let f = "grZebes_801D881C";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // Whole with a pop due, breaking, broken (counting, at its end), growing back at each
    // spawn's count (and its end), and a state past them.
    let every = |c: &Call| zebes_param(c, 0x10) as i16;
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(|c, _| organic(c, 0, 0, 0, -1)),
        Box::new(|c, _| organic(c, 1, 0, 0, 5)),
        // Breaking with its animation over: broken for a random while.
        Box::new(|c, _| {
            organic(c, 1, 0, 0, 5);
            let g = peek_u32(c, 0x8049_E6C8 + 0x180 + 4 * 6);
            let a = peek_u32(c, nth_jobj(c, g, 0xE) + 0x7C);
            assert!(a != 0, "the platform's joint without its animation");
            poke_u32(c, a, peek_u32(c, a) | 1 << 30);
        }),
        Box::new(|c, _| organic(c, 2, 9, 0, 5)),
        Box::new(|c, _| organic(c, 2, 0, 0, 5)),
        Box::new(move |c, k| {
            let n = every(c) * (k % 4) as i16 - 1;
            organic(c, 3, 0, n, 5)
        }),
        Box::new(|c, _| organic(c, 3, 0, 2, 5)),
        Box::new(|c, _| organic(c, 5, 0, 2, 5)),
    ];
    for p in &prepares {
        check_hazard(&d, f, 1, p.as_ref(), &by_address);
    }
}

/// Bubble `i`'s state and countdown set.
fn bubble(c: &mut Call, i: u32, state: u8, t: i16) {
    let a = ZEBES_BUBBLES + 0x24 * i;
    let w = peek_u32(c, a);
    poke_u32(
        c,
        a,
        u32::from(state) << 24 | (w & 0x00FF_0000) | t as u16 as u32,
    );
}

#[test]
fn zebes_bubbles() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    let moves = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(zebes::move_bubbles(r, gobj_arg(maps, call))))
    };
    // As saved, a top bubble popped (one below to take its place or not), and one falling apart
    // at its count's end.
    let popped = |c: &mut Call, k: usize| bubble(c, 1 + (k % 5) as u32, 2, 0);
    let falling = |c: &mut Call, k: usize| bubble(c, 1 + (k % 5) as u32, 3, -1);
    for p in [&as_saved as &Prepare, &popped, &falling] {
        check_hazard(&d, "grZebes_801DB3CC", 1, p, &moves);
    }
    let tick = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps.gobjs.get(&call.regs.gpr[3]).expect("the stage's GObj");
        Ret::Int(u32::from(zebes::tick_bubble(
            r,
            g,
            call.regs.gpr[4] as usize,
        )))
    };
    let this = |c: &mut Call, state: u8, t: i16| {
        let i = c.regs.gpr[4];
        bubble(c, i, state, t)
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &move |c, _| this(c, 2, 0),
        &move |c, _| this(c, 3, -1),
        &move |c, _| this(c, 3, 4),
    ];
    for p in prepares {
        check_hazard(&d, "grZebes_801DB088", 1, p, &tick);
    }
    let span = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(zebes::span_bubbles(r, gobj_arg(maps, call))))
    };
    // None, one or two of the top in place.
    let keep = |c: &mut Call, n: usize| {
        let mut kept = 0;
        for i in 1..20u32 {
            let a = ZEBES_BUBBLES + 0x24 * i;
            if i != 6 && peek_u32(c, a) >> 24 == 1 {
                if kept >= n {
                    bubble(c, i, 4, 0);
                }
                kept += 1;
            }
        }
    };
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &move |c, _| keep(c, 0),
        &move |c, _| keep(c, 1),
        &move |c, _| keep(c, 2),
    ];
    for p in prepares {
        check_hazard(&d, "grZebes_801DBB60", 1, p, &span);
    }
    let place = |r: &mut Running, _: &Maps, _: &Call| {
        zebes::place_bubbles(r);
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DC260", 1, &as_saved, &place);
    let grow = |r: &mut Running, maps: &Maps, call: &Call| {
        zebes::grow_bubbles(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    // After a pop's wait, due (from several seeds), and with 15 bubbles.
    let due = |c: &mut Call, k: usize| {
        poke_u32(c, ZEBES_POP_WAIT, 0);
        let v = map_vars(c, 6);
        let w = peek_u32(c, v + 0x30);
        poke_u32(c, v + 0x30, w | 0xFFFF_0000);
        set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
    };
    let full = move |c: &mut Call, k: usize| {
        due(c, k);
        for i in 0..20u32 {
            let a = ZEBES_BUBBLES + 0x24 * i;
            if peek_u32(c, a) >> 24 == 0 && i < 16 {
                bubble(c, i, 4, 0);
                // No pop left behind by a bubble that moved up.
                poke_u32(c, a + 4, 0);
            }
        }
    };
    let waiting = |c: &mut Call, _: usize| poke_u32(c, ZEBES_POP_WAIT, 7);
    for p in [&as_saved as &Prepare, &due, &full, &waiting] {
        check_hazard(&d, "grZebes_801DC408", 1, p, &grow);
    }
    let add = |r: &mut Running, _: &Maps, call: &Call| {
        let f = |i: usize| call.regs.fpr[i].ps0 as f32;
        let g = &call.regs.gpr;
        zebes::add_bubble(
            r,
            g[3] as i32,
            g[4] as u8,
            f(1),
            f(2),
            f(3),
            zebes::GROW_BUBBLE_FRAME,
        );
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DAE70", 1, &as_saved, &add);
    let pop = |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(zebes::show_pop(r)));
    let seeds = |c: &mut Call, k: usize| set_seed(c, 0x2468_1357u32.wrapping_mul(k as u32 + 1));
    for p in [&as_saved as &Prepare, &seeds] {
        check_hazard(&d, "grZebes_801DAA08", 1, p, &pop);
    }
    let hit = |r: &mut Running, _: &Maps, _: &Call| {
        zebes::bubbles_hit(r);
        Ret::Nothing
    };
    check_hazard(&d, "fn_801DAC90", 1, &as_saved, &hit);
}

#[test]
fn zebes_creatures_and_splashes() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    let creature = |c: &mut Call, waiting: bool, next: i16, t: i16| {
        let v = map_vars(c, 4);
        let w = peek_u32(c, v);
        poke_u32(c, v, (w & 0x7FFF_FFFF) | u32::from(waiting) << 31);
        poke_u32(c, v + 8, (next as u16 as u32) << 16 | t as u16 as u32);
    };
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(move |c, _| creature(c, true, 1, 5)),
        Box::new(move |c, _| creature(c, false, 1, 1)),
        Box::new(move |c, _| creature(c, false, -1, 1)),
        Box::new(move |c, _| creature(c, false, 1, 9)),
        Box::new(move |c, _| creature(c, false, 1, 0)),
    ];
    check_hazard(&d, "grZebes_801D95B8", 1, &as_saved, &by_address);
    for p in &prepares {
        check_hazard(&d, "grZebes_801D95B8", 1, p.as_ref(), &by_address);
    }
    let back = |c: &mut Call, state: i16, level: f32| {
        let v = map_vars(c, 1);
        let w = peek_u32(c, v);
        poke_u32(c, v, (w & 0xFFFF) | (state as u16 as u32) << 16);
        poke_u32(c, leaf_check::sym("stage_info") + 0x724, level.to_bits());
    };
    let prepares: Vec<Box<Prepare>> = vec![
        Box::new(move |c, _| back(c, 0, -30.0)),
        Box::new(move |c, _| back(c, 0, -10.0)),
        Box::new(move |c, _| back(c, 0, 10.0)),
        Box::new(move |c, _| back(c, 1, 10.0)),
        Box::new(move |c, _| back(c, 2, 10.0)),
    ];
    check_hazard(&d, "grZebes_801D9F84", 1, &as_saved, &by_address);
    for p in &prepares {
        check_hazard(&d, "grZebes_801D9F84", 1, p.as_ref(), &by_address);
    }
    check_hazard(&d, "grZebes_801D9410", 1, &as_saved, &by_address);
    let splash = |r: &mut Running, _: &Maps, call: &Call| {
        let a = call.regs.gpr[3];
        zebes::add_splash(
            r,
            Vec3::new(f32_at(call, a), f32_at(call, a + 4), f32_at(call, a + 8)),
        );
        Ret::Nothing
    };
    let full = |c: &mut Call, _: usize| poke_u16(c, 0x804D_6998, 10);
    for p in [&as_saved as &Prepare, &full] {
        check_hazard(&d, "grZebes_801DA3F4", 1, p, &splash);
    }
    let remove = |r: &mut Running, maps: &Maps, call: &Call| {
        zebes::remove_splash(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grZebes_801DA4FC", 1, &as_saved, &remove);
}

#[test]
fn zebes_lava_bubbles() {
    use melee_sim::game::gr::{zako, zebes};
    let d = hazard_dir(ZEBES);
    let cb = |r: &mut Running, _: &Maps, call: &Call| {
        let a = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, a), f32_at(call, a + 4), f32_at(call, a + 8));
        zebes::lava_bubble(r, call.addr, pos, call.regs.gpr[4] as i32);
        Ret::Int(0)
    };
    let lines = |c: &mut Call, k: usize| c.regs.gpr[4] = (k % 11) as u32;
    for f in ["grZebes_801DCB64", "grZebes_801DCBB0"] {
        for p in [&as_saved as &Prepare, &lines] {
            check_hazard(&d, f, 1, p, &cb);
        }
    }
    // The stage's and the island's spawn configurations, from their processes' calls, at the
    // lava's level and other heights.
    let run = |r: &mut Running, maps: &Maps, call: &Call| {
        let j = maps.jobjs.get(&call.regs.gpr[4]).copied();
        let n = zako::run_config(r, call.regs.gpr[3], j, call.regs.fpr[1].ps0 as f32);
        Ret::Int(n as u32)
    };
    let t = leaf_check::sym("grZakoGenerator_801CA43C");
    let config = move |c: &mut Call, map: u32, off: u32, joint: u32, h: f32| {
        c.addr = t;
        let v = map_vars(c, map);
        c.regs.gpr[3] = peek_u32(c, v + off);
        let g = peek_u32(c, 0x8049_E6C8 + 0x180 + 4 * map);
        c.regs.gpr[4] = nth_jobj(c, g, joint);
        c.regs.fpr[1].ps0 = f64::from(h);
    };
    for h in [-60.0f32, -20.0, 0.0, 20.0] {
        check_hazard(
            &d,
            "grZebes_801D881C",
            1,
            &move |c, _| config(c, 6, 0x38, 0xE, h),
            &run,
        );
        check_hazard(
            &d,
            "grZebes_801D925C",
            1,
            &move |c, _| config(c, 7, 0x28, 1, h),
            &run,
        );
    }
}

#[test]
fn zebes_unreached_callbacks() {
    use melee_sim::game::gr::zebes;
    let d = hazard_dir(ZEBES);
    let from = "grZebes_801D925C";
    for name in [
        "grZebes_801D90FC",
        "grZebes_801D93D8",
        "grZebes_801D94EC",
        "grZebes_801D9754",
        "grZebes_801D9F2C",
        "grZebes_801DA0C0",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grZebes_801D8528"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grZebes_801D84A0"));
    check_hazard(&d, from, 1, &demo, &by_demo);
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(zebes::check_shadow_render()));
    let to_shadow = retarget(leaf_check::sym("grZebes_801DCCC0"));
    check_hazard(&d, from, 1, &to_shadow, &shadow);
}

// Onett (`gronett.c`).

const ONETT: &str = "oracle-stage-onett";

/// gronett.c's `yakumono_param`.
const ONETT_PARAM: u32 = 0x804D_69C0;

/// Onett's parameters' float at `off`.
fn onett_param(c: &Call, off: u32) -> f32 {
    f32_at(c, peek_u32(c, ONETT_PARAM) + off)
}

/// The stage object a call passes as its `Ground` at `gp`.
fn onett_ground(maps: &Maps, gp: u32) -> melee_sim::game::handles::GroundId {
    *maps.grounds.get(&gp).expect("the stage object")
}

/// A joint callback's collision's kind (`x34_flags.b1234`), and setting it.
fn coll_kind(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[5] + 0x34) >> 27 & 0xF
}

fn set_coll_kind(c: &mut Call, kind: u32) {
    let at = c.regs.gpr[5] + 0x34;
    let w = peek_u32(c, at);
    poke_u32(c, at, (w & !(0xF << 27)) | kind << 27);
}

#[test]
fn onett_setup() {
    use melee_sim::game::gr::onett;
    let d = hazard_dir(ONETT);
    check_hazard(&d, "grOnett_801E3738", 1, &as_saved, &by_event);
    check_hazard(&d, "grOnett_801E37C8", 1, &as_saved, &by_event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = onett::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E37F4", 1, &as_saved, &create);
    for map in [0u32, 2, 3, 4, 5] {
        check_hazard(
            &d,
            "grOnett_801E37F4",
            1,
            &move |c, _| c.regs.gpr[3] = map,
            &create,
        );
    }
    for f in [
        "grOnett_801E38DC",
        "grOnett_801E3A34",
        "grOnett_801E3CE4",
        "grOnett_801E41C8",
        "grOnett_801E3930",
        "grOnett_801E41B0",
    ] {
        check_hazard(&d, f, 1, &as_saved, &by_address);
    }
    let seeds = |c: &mut Call, k: usize| set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
    check_hazard(&d, "grOnett_801E41C8", 1, &seeds, &by_address);
    check_hazard(&d, "grOnett_801E3A34", 1, &seeds, &by_address);
}

/// The drugstore's variables set: state, next state, landings, ticks in the state and count.
fn store(c: &mut Call, state: i16, next: i16, hits: i32, frame: i32, timer: i32) {
    let a = vars(c);
    poke_u16(c, a, state as u16);
    poke_u16(c, a + 2, next as u16);
    poke_u32(c, a + 4, hits as u32);
    poke_u32(c, a + 8, frame as u32);
    poke_u32(c, a + 0xC, timer as u32);
}

#[test]
fn onett_store() {
    let d = hazard_dir(ONETT);
    let f = "grOnett_801E3DA0";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    let mut cases: Vec<(i16, i16, i32, i32, i32)> = Vec::new();
    // Each state at its ticks' edges and its count's (the blink's bit too), staying and
    // changing.
    for (state, frames) in [
        (0i16, &[0, 5][..]),
        (1, &[0, 30]),
        (2, &[0x8B, 0x8C, 0x8D]),
        (3, &[0]),
        (4, &[0, 12]),
        (5, &[0x13, 0x14, 0x45, 0x46, 0x59, 0x5A, 0x81, 0x82]),
    ] {
        for &frame in frames {
            cases.push((state, state, 0, frame - 1, 10));
        }
    }
    for timer in [-1, 0, 1, 2, 3] {
        cases.push((6, 6, 0, 0, timer));
        cases.push((7, 7, 0, 0, timer));
    }
    // A change of state into each, and the first tick (state -1).
    for next in [0i16, 1, 2, 3, 4, 5, 6, 7] {
        cases.push((3, next, 4, 9, 0));
    }
    cases.push((-1, 0, 0, 0, 0));
    for (state, next, hits, frame, timer) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| store(c, state, next, hits, frame, timer),
            &by_address,
        );
    }
}

#[test]
fn onett_roof() {
    let d = hazard_dir(ONETT);
    let f = "grOnett_801E40E4";
    let roof = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = onett_ground(maps, call.regs.gpr[3]);
        let param = r.game.gr.onett_yakumono;
        melee_sim::game::gr::onett::roof_landed(
            &mut r.game.grounds[g.0],
            param,
            r.data,
            call.regs.gpr[7] as i32,
        );
        Ret::Nothing
    };
    check_hazard(&d, f, 1, &as_saved, &roof);
    // Each collision kind, in each state, with the count just short of and at the crack's and
    // the fall's.
    let crack = |c: &Call| onett_param(c, 0x34) as i32;
    let fall = |c: &Call| onett_param(c, 0x38) as i32;
    for kind in [0u32, 1, 2] {
        for state in 0..8i16 {
            for edge in [0, 1] {
                check_hazard(
                    &d,
                    f,
                    1,
                    &move |c, _| {
                        c.regs.gpr[7] = kind;
                        let hits = if state == 3 { fall(c) } else { crack(c) } - 2 + edge;
                        let v = c.regs.gpr[3] + 0xC4;
                        poke_u16(c, v, state as u16);
                        poke_u32(c, v + 4, hits as u32);
                    },
                    &roof,
                );
            }
        }
    }
}

/// An awning's variables set: push, sag, swing speed, landings this tick and last, quiet ticks
/// and whether one landed.
#[allow(clippy::too_many_arguments)]
fn awning(
    c: &mut Call,
    i: u32,
    push: f32,
    sag: f32,
    speed: f32,
    n: i16,
    last: i16,
    quiet: i16,
    flag: i16,
) {
    let a = vars(c) + 0x1C * i;
    poke_u32(c, a + 8, push.to_bits());
    poke_u32(c, a + 0xC, sag.to_bits());
    poke_u32(c, a + 0x10, speed.to_bits());
    poke_u16(c, a + 0x14, n as u16);
    poke_u16(c, a + 0x16, last as u16);
    poke_u16(c, a + 0x18, quiet as u16);
    poke_u16(c, a + 0x1A, flag as u16);
}

#[test]
fn onett_awnings() {
    use melee_sim::game::gr::onett;
    let d = hazard_dir(ONETT);
    let swing = |r: &mut Running, maps: &Maps, call: &Call| {
        onett::swing_awnings(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E5214", 1, &as_saved, &swing);
    // Pushed past the limit, sagging either way, springing back fast and slow, settled
    // (close and slow, close and fast, far and slow), at the cap either way, rustled by a
    // landing or a changed count, and quiet.
    let max = |c: &Call| onett_param(c, 0x1C);
    let prepares: [&Prepare; 14] = [
        &|c, _| awning(c, 0, -100.0, 0.0, 0.0, 0, 0, 0, 0),
        &|c, _| awning(c, 1, -2.0, 0.0, 0.0, 0, 0, 0, 0),
        &|c, _| awning(c, 0, 0.0, -2.0, 0.0, 0, 0, 0, 0),
        &|c, _| awning(c, 0, 0.0, 2.0, 0.5, 0, 0, 0, 0),
        &|c, _| awning(c, 1, -1.0, -1.0, 0.0, 0, 0, 0, 0),
        &|c, _| awning(c, 0, -1.0, -1.0001, 0.0001, 0, 0, 0, 0),
        &|c, _| awning(c, 0, -1.0, -1.0001, 3.0, 0, 0, 0, 0),
        &|c, _| awning(c, 0, -1.0, -3.0, 0.0001, 0, 0, 0, 0),
        &|c, _| {
            let m = max(c);
            awning(c, 0, -m, m, 50.0, 0, 0, 0, 0)
        },
        &|c, _| {
            let m = max(c);
            awning(c, 1, 0.0, -m, -50.0, 0, 0, 0, 0)
        },
        &|c, _| awning(c, 0, -1.0, 0.0, 0.0, 2, 0, 0, 1),
        &|c, _| awning(c, 1, -1.0, 0.0, 0.0, 1, 2, 0, 0),
        &|c, _| awning(c, 0, -1.0, 0.0, 0.0, 1, 2, 4, 1),
        &|c, _| awning(c, 1, -1.0, 0.0, 0.0, 0, 0, 1, 0),
    ];
    for p in prepares {
        check_hazard(&d, "grOnett_801E5214", 1, p, &swing);
    }
    let landed = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = onett_ground(maps, call.regs.gpr[3]);
        let param = r.game.gr.onett_yakumono;
        let coll_kind = coll_kind(call);
        onett::awning_landed(
            &mut r.game.grounds[g.0],
            param,
            r.data,
            call.regs.gpr[4] as i32,
            coll_kind,
            call.regs.gpr[7] as i32,
        );
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E54B4", 1, &as_saved, &landed);
    // Each joint, collision kind and collision state.
    for joint in [0u32, 1] {
        for kind in [0u32, 1, 2] {
            for state in 0..5u32 {
                check_hazard(
                    &d,
                    "grOnett_801E54B4",
                    1,
                    &move |c, _| {
                        c.regs.gpr[4] = joint;
                        c.regs.gpr[7] = kind;
                        set_coll_kind(c, state);
                    },
                    &landed,
                );
            }
        }
    }
}

#[test]
fn onett_street() {
    use melee_sim::game::gr::onett;
    let d = hazard_dir(ONETT);
    check_hazard(&d, "grOnett_801E3C60", 1, &as_saved, &by_address);
    // The cars' camera subject with the warning up and down.
    for warn in [0u32, 1, 2] {
        check_hazard(
            &d,
            "grOnett_801E3C60",
            1,
            &move |c, _| poke_u32(c, STAGE_INFO + 0x740, warn),
            &by_address,
        );
    }
    let smoke = |r: &mut Running, maps: &Maps, call: &Call| {
        onett::smoke(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E5538", 1, &as_saved, &smoke);
    // Its wait running out with a smoke and without, and not.
    for timer in [-1i32, 0, 1, 7] {
        check_hazard(
            &d,
            "grOnett_801E5538",
            1,
            &move |c, _| poke_u32(c, vars(c) + 0x38, timer as u32),
            &smoke,
        );
    }
    let shadow = |r: &mut Running, maps: &Maps, call: &Call| {
        let a = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, a), f32_at(call, a + 4), f32_at(call, a + 8));
        let j = maps.jobjs.get(&call.regs.gpr[5]).copied();
        Ret::Int(u32::from(onett::check_shadow_render(
            r.game, r.data, pos, j,
        )))
    };
    check_hazard(&d, "grOnett_801E5768", 1, &as_saved, &shadow);
}

/// The cars' variables set: state, count and honk state of the car from the right with its
/// car and speed, and the car from the left's.
#[allow(clippy::too_many_arguments)]
fn cars(
    c: &mut Call,
    a_state: u8,
    x108: i32,
    x110: i32,
    curr: i8,
    speed: f32,
    b_state: u8,
    b_timer: i32,
    next: i8,
) {
    let a = vars(c);
    poke_u8(c, a + 0x40, curr as u8);
    poke_u8(c, a + 0x41, a_state);
    poke_u32(c, a + 0x44, x108 as u32);
    poke_u32(c, a + 0x4C, x110 as u32);
    poke_u32(c, a + 0x50, speed.to_bits());
    poke_u8(c, a + 0x54, next as u8);
    poke_u8(c, a + 0x55, b_state);
    poke_u32(c, a + 0x58, b_timer as u32);
}

#[test]
fn onett_cars() {
    use melee_sim::game::gr::onett;
    let d = hazard_dir(ONETT);
    let f = "grOnett_801E43E0";
    check_hazard(&d, f, 1, &as_saved, &by_address);
    // The right car's state, count, honk state, car and speed; the left car's state, count and
    // car.
    type Case = (u8, i32, i32, i8, f32, u8, i32, i8);
    let mut cases: Vec<Case> = Vec::new();
    // The right car in each state, its counts over or not; driving, at each honk state.
    for a_state in 0..8u8 {
        for x108 in [0, 1] {
            for x110 in [0, 1, 2, 4] {
                cases.push((a_state, x108, x110, 1, -8.0, 3, 5, 2));
            }
        }
    }
    // The left car in each state, its counts over or not.
    for b_state in 0..6u8 {
        for t in [0, 1] {
            cases.push((7, 0, 0, 1, -8.0, b_state, t, 2));
        }
    }
    for (a, x108, x110, curr, speed, b, bt, next) in cases {
        check_hazard(
            &d,
            f,
            1,
            &move |c, _| cars(c, a, x108, x110, curr, speed, b, bt, next),
            &by_address,
        );
    }
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let item = gobj_arg(maps, call);
        let g = onett_ground(maps, call.regs.gpr[4]);
        onett::car_hit(r, item, g);
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E5030", 1, &as_saved, &hit);
    // Spinning, driving fast or slow either way, from several seeds.
    for (state, speed) in [(4u8, -8.0f32), (5, -8.0), (5, -0.6), (5, 0.2), (6, -3.0)] {
        let p = move |c: &mut Call, k: usize| {
            let v = c.regs.gpr[4] + 0xC4;
            poke_u8(c, v + 0x41, state);
            poke_u32(c, v + 0x50, speed.to_bits());
            set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
        };
        check_hazard(&d, "grOnett_801E5030", 4, &p, &hit);
    }
    let horn = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = onett_ground(maps, call.regs.gpr[3]);
        onett::car_sound(r, g, call.regs.gpr[4] as i32, call.regs.gpr[5] as i32);
        Ret::Nothing
    };
    check_hazard(&d, "grOnett_801E5194", 1, &as_saved, &horn);
    for car in 0..4u32 {
        for kind in [4u32, 5] {
            check_hazard(
                &d,
                "grOnett_801E5194",
                1,
                &move |c, _| {
                    c.regs.gpr[4] = car;
                    c.regs.gpr[5] = kind;
                },
                &horn,
            );
        }
    }
    let scream = |r: &mut Running, _: &Maps, call: &Call| {
        onett::scream(r, call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    for car in 0..4u32 {
        check_hazard(
            &d,
            "grOnett_801E5140",
            1,
            &move |c, _| c.regs.gpr[3] = car,
            &scream,
        );
    }
}

#[test]
fn onett_unreached_callbacks() {
    let d = hazard_dir(ONETT);
    let from = "grOnett_801E3C60";
    for name in [
        "grOnett_801E3928",
        "grOnett_801E392C",
        "grOnett_801E3CE0",
        "grOnett_801E40E0",
        "grOnett_801E502C",
    ] {
        check_hazard(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let load = retarget(leaf_check::sym("grOnett_801E37C4"));
    check_hazard(&d, from, 1, &load, &by_event);
    let demo = retarget(leaf_check::sym("grOnett_801E3734"));
    check_hazard(&d, from, 1, &demo, &by_demo);
}

// Flat Zone (`grflatzone.c`) and its falling tools (`ittools.c`).

const FZ: &str = "oracle-stage-flat-zone";

/// grflatzone.c's `yakumono_param`.
const FZ_PARAM: u32 = 0x804D_6AB0;

/// Flat Zone's parameters' word at `off`.
fn fz_param(c: &Call, off: u32) -> i32 {
    peek_u32(c, peek_u32(c, FZ_PARAM) + off) as i32
}

/// One of several seeds for each call.
fn fz_seeds(c: &mut Call, k: usize) {
    set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
}

#[test]
fn flatzone_setup() {
    use melee_sim::game::gr::flatzone;
    let d = dir(FZ);
    check_calls_with(&d, "grFlatzone_80216E78", 1, &as_saved, &event);
    check_calls_with(&d, "grFlatzone_80216E78", 1, &fz_seeds, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = flatzone::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    check_calls_with(&d, "grFlatzone_80216F48", 1, &as_saved, &create);
    for map in [0u32, 1, 2, 3, 4, 5, 6, 8] {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "grFlatzone_80216F48", 1, &set, &create);
    }
    for f in [
        "stageGObj0_OnInit",
        "grFlatzone_802173D4",
        "grFlatzone_8021707C",
        "grFlatzone_80217648",
        "grFlatzone_80217CD4",
        "grFlatzone_80217450",
        "grFlatzone_80217DA8",
        "grFlatzone_80217E90",
        "fn_8021706C",
        "fn_80217440",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    check_calls_with(&d, "grFlatzone_8021707C", 1, &fz_seeds, &by_address);
    // The platforms' next set: the setup's and drawn, as saved and from several seeds, with
    // the hidden set before or another.
    let next = |r: &mut Running, _: &Maps, call: &Call| {
        flatzone::next_platforms(r, call.regs.gpr[3] != 0);
        Ret::Nothing
    };
    check_calls_with(&d, "grFlatzone_80218060", 1, &as_saved, &next);
    for first in [0u32, 1] {
        for before in [8u8, 0, 2, 3] {
            let set = move |c: &mut Call, k: usize| {
                c.regs.gpr[3] = first;
                poke_u8(c, map_vars(c, 5) + 4, before);
                fz_seeds(c, k);
            };
            check_calls_with(&d, "grFlatzone_80218060", 1, &set, &next);
        }
    }
    // The pose for an event and the hidden set, from the next set's calls.
    let pose = |r: &mut Running, _: &Maps, _: &Call| {
        flatzone::pose_platforms(r);
        Ret::Nothing
    };
    let to_pose = leaf_check::sym("grFlatzone_802181B4");
    for before in [8u8, 1, 5] {
        let set = move |c: &mut Call, k: usize| {
            c.addr = to_pose;
            poke_u8(c, map_vars(c, 5) + 4, before);
            fz_seeds(c, k);
        };
        check_calls_with(&d, "grFlatzone_80218060", 1, &set, &pose);
    }
    let hide = |r: &mut Running, _: &Maps, _: &Call| {
        flatzone::hide_platforms(r.game);
        Ret::Nothing
    };
    let to_hide = retarget(leaf_check::sym("grFlatzone_80218260"));
    check_calls_with(&d, "grFlatzone_80218060", 1, &to_hide, &hide);
}

/// The controller's variables set: waiting, the last event and the one before, whether it
/// may start one, and its count.
fn fz_controller(c: &mut Call, wait: u8, last: u8, before: u8, may: u8, count: i16) {
    let v = vars(c);
    poke_u32(
        c,
        v,
        u32::from(wait) << 24 | u32::from(last) << 16 | u32::from(before) << 8 | u32::from(may),
    );
    poke_u16(c, v + 4, count as u16);
}

/// The controller's process, as saved and with its variables set to each branch: waiting,
/// an event running, counting, the count running out after each event (from several seeds),
/// and the count already out.
#[test]
fn flatzone_controller() {
    let d = dir(FZ);
    let f = "grFlatzone_802171D4";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    let prepares: [&Prepare; 9] = [
        &|c, _| fz_controller(c, 1, 3, 4, 1, 1),
        &|c, _| fz_controller(c, 0, 3, 4, 0, 1),
        &|c, _| fz_controller(c, 0, 3, 4, 1, 50),
        &|c, _| fz_controller(c, 0, 3, 4, 1, -1),
        &|c, k| {
            fz_controller(c, 0, 3, 4, 1, 1);
            fz_seeds(c, k);
        },
        &|c, k| {
            fz_controller(c, 0, 4, 3, 1, 1);
            fz_seeds(c, k);
        },
        &|c, k| {
            fz_controller(c, 0, 0, 0, 1, 1);
            fz_seeds(c, k);
        },
        &|c, k| {
            fz_controller(c, 0, 3, 4, 1, 0);
            fz_seeds(c, k);
        },
        &|c, k| {
            fz_controller(c, 0, 4, 3, 1, 0);
            fz_seeds(c, k + 7);
        },
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &by_address);
    }
}

/// The platforms' variables set: waiting, the animation and the last, the set (the set
/// before hidden), the row and the row before, and the count.
#[allow(clippy::too_many_arguments)]
fn fz_platforms(
    c: &mut Call,
    wait: u8,
    anim: u8,
    last: u8,
    set: u8,
    row: u8,
    before: u8,
    count: i16,
) {
    let v = vars(c);
    poke_u32(
        c,
        v,
        u32::from(wait) << 24 | u32::from(anim) << 16 | u32::from(last) << 8 | u32::from(set),
    );
    poke_u8(c, v + 4, 8);
    poke_u8(c, v + 5, row);
    poke_u8(c, v + 6, before);
    poke_u16(c, v + 8, count as u16);
}

/// The platforms' process, as saved and with its variables set to each branch: waiting,
/// hidden, posed for an event, counting, and the count running out in each set with each of
/// its rows (from several seeds), with the animation changed and not.
#[test]
fn flatzone_platforms() {
    let d = dir(FZ);
    let f = "grFlatzone_802174EC";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    let fixed: [&Prepare; 5] = [
        &|c, _| fz_platforms(c, 1, 3, 3, 1, 1, 0, -1),
        &|c, _| fz_platforms(c, 0, 3, 3, 8, 1, 0, -1),
        &|c, _| fz_platforms(c, 0, 3, 4, 1, 0, 0, -1),
        &|c, _| fz_platforms(c, 0, 3, 3, 1, 2, 0, 0),
        &|c, _| fz_platforms(c, 0, 3, 3, 1, 2, 0, 30),
    ];
    for p in fixed {
        check_calls_with(&d, f, 1, p, &by_address);
    }
    // Set 0 has rows 1 and 2 only; a set past 3 has no row a draw can reach.
    for set in 0..4u8 {
        for row in 1..=4u8 {
            if row > 2 && set == 0 {
                continue;
            }
            let p = move |c: &mut Call, k: usize| {
                fz_platforms(c, 0, 0, (k % 2) as u8, set, row, 0, -1 - (k % 3) as i16);
                fz_seeds(c, k + usize::from(set) * 5 + usize::from(row));
            };
            check_calls_with(&d, f, 1, &p, &by_address);
        }
    }
}

/// The oil panic's variables set: started, side, state and count (its area as saved).
fn fz_oil(c: &mut Call, start: u8, side: f32, state: i32, timer: i32) {
    let v = vars(c);
    poke_u8(c, v, start);
    poke_u32(c, v + 4, side.to_bits());
    poke_u32(c, v + 0xC, state as u32);
    poke_u32(c, v + 0x10, timer as u32);
}

/// The oil panic's process, as saved and with its variables set to each branch: started
/// from each state (from several seeds, for both sides), waiting at its half and its end,
/// walking on and off a step and at its end, pouring (also off the stage, where no floor is
/// below, and with no area free), the oil lasting and ending, and hiding.
#[test]
fn flatzone_oil() {
    let d = dir(FZ);
    let f = "grFlatzone_802176BC";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    for state in [-1, 0, 3, 4] {
        let p = move |c: &mut Call, k: usize| {
            fz_oil(c, 1, 1.0, state, 5);
            fz_seeds(c, k + (state + 1) as usize);
        };
        check_calls_with(&d, f, 1, &p, &by_address);
    }
    // Started while the oil lasts, with an area (which the bug keeps) and none.
    let area = leaf_check::sym("grDynamicAttr_8049EFA0");
    for a in [0, area] {
        let p = move |c: &mut Call, k: usize| {
            fz_oil(c, 1, -1.0, 3, 5);
            poke_u32(c, vars(c) + 8, a);
            fz_seeds(c, k);
        };
        check_calls_with(&d, f, 1, &p, &by_address);
    }
    for side in [1.0f32, -1.0] {
        let half = move |c: &mut Call, _: usize| {
            let t = fz_param(c, 0x20) / 2;
            fz_oil(c, 0, side, 0, t);
        };
        let ends = move |c: &mut Call, k: usize| {
            fz_oil(c, 0, side, 0, 1);
            fz_seeds(c, k);
        };
        let step = move |c: &mut Call, k: usize| {
            let ticks = fz_param(c, 0x2C);
            fz_oil(
                c,
                0,
                side,
                1,
                ticks * (k as i32 % 3 + 1) + 1 - (k as i32 % 2),
            );
        };
        let walked = move |c: &mut Call, _: usize| fz_oil(c, 0, side, 1, 1);
        let pour = move |c: &mut Call, _: usize| fz_oil(c, 0, side, 2, 0);
        let lasts = move |c: &mut Call, k: usize| fz_oil(c, 0, side, 3, 2 + k as i32 % 2);
        let hides = move |c: &mut Call, _: usize| fz_oil(c, 0, side, 4, 0);
        let prepares: [&Prepare; 7] = [&half, &ends, &step, &walked, &pour, &lasts, &hides];
        for p in prepares {
            check_calls_with(&d, f, 1, p, &by_address);
        }
    }
    let off = |c: &mut Call, _: usize| {
        fz_oil(c, 0, 1.0, 2, 0);
        let j = peek_u32(c, c.regs.gpr[3] + 0x28);
        poke_u32(c, j + 0x38, 900.0f32.to_bits());
    };
    let full = |c: &mut Call, _: usize| {
        fz_oil(c, 0, -1.0, 2, 0);
        poke_u32(c, leaf_check::sym("grDynamicAttr_804D6964"), 0);
    };
    for p in [&off as &Prepare, &full] {
        check_calls_with(&d, f, 1, p, &by_address);
    }
}

/// The events nothing starts and the tools' process, as saved and started; the tools also
/// counting on and off a drop (from several seeds), ending, and with no count.
#[test]
fn flatzone_events() {
    let d = dir(FZ);
    for f in ["grFlatzone_80217D30", "grFlatzone_80217E04"] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
        check_calls_with(&d, f, 1, &|c, _| poke_u8(c, vars(c), 1), &by_address);
    }
    let f = "grFlatzone_80217EF0";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    let tools = |c: &mut Call, start: u8, count: i32| {
        let v = vars(c);
        poke_u8(c, v, start);
        poke_u32(c, v + 4, count as u32);
    };
    let prepares: [&Prepare; 6] = [
        &|c, _| tools(c, 1, 0),
        &|c, k| {
            let period = fz_param(c, 0x14);
            tools(c, 0, period * (k as i32 % 4 + 1));
            fz_seeds(c, k);
        },
        &|c, _| {
            let period = fz_param(c, 0x14);
            tools(c, 0, period + 1);
        },
        &|c, _| tools(c, 0, 1),
        &|c, _| tools(c, 0, 0),
        &|c, _| tools(c, 0, -3),
    ];
    for p in prepares {
        check_calls_with(&d, f, 1, p, &by_address);
    }
}

/// A tool's item in a call made with it in r3.
fn tool_item(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[3] + 0x2C)
}

/// A tool's knock (`it_802EF548`).
fn tool_knock(r: &mut Running, maps: &Maps, call: &Call) -> Ret {
    melee_sim::game::it::kinds::tools::knock(r, gobj_arg(maps, call));
    Ret::Nothing
}

#[test]
fn flatzone_tools() {
    use melee_sim::game::it::kinds::tools;
    let d = dir(FZ);
    let spawn = |r: &mut Running, _: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let pos = Vec3::new(
            f32_at(call, g[3]),
            f32_at(call, g[3] + 4),
            f32_at(call, g[3] + 8),
        );
        let _ = tools::spawn(r, pos, g[4] as i32, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_calls_with(&d, "it_802EEFA8", 1, &as_saved, &spawn);
    for which in 0..5u32 {
        for facing in [1.0f64, -1.0] {
            let p = move |c: &mut Call, _: usize| {
                c.regs.gpr[4] = which;
                c.regs.fpr[1].ps0 = facing;
            };
            check_calls_with(&d, "it_802EEFA8", 1, &p, &spawn);
        }
    }
    let start = |r: &mut Running, maps: &Maps, call: &Call| {
        tools::start(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    check_calls_with(&d, "it_802EF098", 1, &as_saved, &start);
    let lie = |r: &mut Running, maps: &Maps, call: &Call| {
        tools::lie(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "it_802EF320", 1, &as_saved, &lie);
    for f in [
        "itTools_UnkMotion4_Anim",
        "itTools_UnkMotion4_Phys",
        "itTools_UnkMotion4_Coll",
        "itTools_UnkMotion9_Anim",
        "itTools_UnkMotion9_Coll",
        "itTools_Logic22_DmgDealt",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &item_cb);
    }
    // Falling at its life's end, and still; lying moving, at the blink's turns and its end.
    let life = |c: &mut Call, v: f32| poke_u32(c, tool_item(c) + 0xD44, v.to_bits());
    let still = |c: &mut Call, _: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0x40, 0);
        poke_u32(c, ip + 0x44, 0);
    };
    check_calls_with(
        &d,
        "itTools_UnkMotion4_Anim",
        1,
        &|c, _| life(c, 1.0),
        &item_cb,
    );
    check_calls_with(
        &d,
        "itTools_UnkMotion4_Anim",
        1,
        &|c, _| life(c, 0.5),
        &item_cb,
    );
    check_calls_with(&d, "itTools_UnkMotion4_Coll", 1, &still, &item_cb);
    check_calls_with(&d, "itTools_UnkMotion9_Coll", 1, &still, &item_cb);
    let moving = |c: &mut Call, k: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0x40, [0.0f32, 1.5][k % 2].to_bits());
        poke_u32(c, ip + 0x44, (-2.0f32).to_bits());
    };
    check_calls_with(&d, "itTools_UnkMotion9_Coll", 1, &moving, &item_cb);
    for v in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.5] {
        let p = move |c: &mut Call, _: usize| life(c, v);
        check_calls_with(&d, "itTools_UnkMotion9_Anim", 1, &p, &item_cb);
    }
    // Knocked: as saved, from a fighter on either side, without one (the hit's direction
    // either way), by something not a fighter, flying too fast, and with its damage capped.
    check_calls_with(&d, "it_802EF548", 1, &as_saved, &tool_knock);
    let from_fighter = |c: &mut Call, k: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0xCEC, first_fighter(c));
        poke_u32(c, ip + 0x4C, [-80.0f32, 80.0][k % 2].to_bits());
    };
    let no_fighter = |c: &mut Call, k: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0xCEC, 0);
        poke_u32(c, ip + 0xCCC, [1.0f32, -1.0][k % 2].to_bits());
    };
    let not_fighter = |c: &mut Call, _: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0xCEC, c.regs.gpr[3]);
    };
    let fast = |c: &mut Call, k: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0x40, [9.0f32, -9.0, 0.1][k % 3].to_bits());
    };
    let capped = |c: &mut Call, _: usize| {
        let ip = tool_item(c);
        poke_u32(c, ip + 0xCA0, 500);
    };
    let prepares: [&Prepare; 5] = [&from_fighter, &no_fighter, &not_fighter, &fast, &capped];
    for p in prepares {
        check_calls_with(&d, "it_802EF548", 1, p, &tool_knock);
    }
    // The reactions and the state no capture reached, from a falling tool's calls.
    for name in [
        "itTools_Logic22_DmgReceived",
        "it_2725_Logic22_Clanked",
        "it_2725_Logic22_HitShield",
        "it_2725_Logic22_Absorbed",
        "itTools_Logic22_ShieldBounced",
        "itTools_Logic22_Reflected",
        "itTools_UnkMotion9_Phys",
    ] {
        let from = "itTools_UnkMotion4_Anim";
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &item_cb);
    }
    let forget = |r: &mut Running, maps: &Maps, call: &Call| {
        let other = maps.gobjs.get(&call.regs.gpr[4]).copied();
        tools::forget(r, gobj_arg(maps, call), other);
        Ret::Nothing
    };
    let t = leaf_check::sym("it_802EFA24");
    let with_fighter = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[4] = first_fighter(c);
    };
    check_calls_with(&d, "itTools_UnkMotion4_Anim", 1, &with_fighter, &forget);
}

/// The stage's callbacks no capture reaches, from the controller's captures: the empty
/// processes, setups and destroy callbacks, the shadow check, and the load, start and demo
/// hooks.
#[test]
fn flatzone_unreached_callbacks() {
    use melee_sim::game::gr::flatzone;
    let d = dir(FZ);
    let from = "grFlatzone_802171D4";
    for name in [
        "grFlatzone_80217064",
        "grFlatzone_80217068",
        "grFlatzone_802173D0",
        "grFlatzone_80217438",
        "grFlatzone_8021743C",
        "grFlatzone_80217644",
        "grFlatzone_80217CD0",
        "grFlatzone_80217DA4",
        "grFlatzone_80217E78",
        "grFlatzone_80217E7C",
        "grFlatzone_80217E88",
        "grFlatzone_80217E8C",
        "grFlatzone_8021805C",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(flatzone::check_shadow_render()));
    let t = leaf_check::sym("grFlatzone_802182BC");
    check_calls_with(&d, from, 1, &retarget(t), &shadow);
    for name in [
        "grFlatzone_UnkStage0_OnLoad",
        "grFlatzone_UnkStage0_OnStart",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &event);
    }
    let t = leaf_check::sym("grFlatzone_80216E74");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
}

// Mushroom Kingdom (`grinishie1.c`).

const MK: &str = "oracle-stage-mushroom-kingdom";

/// Mushroom Kingdom's stage object's variables (map 3's `Ground::u`).
fn mk_vars(c: &Call) -> u32 {
    map_vars(c, 3)
}

/// Brick block `i` (`grInishie1_Block`) in the stage's block.
fn mk_block(c: &Call, i: u32) -> u32 {
    peek_u32(c, mk_vars(c) + 0x18) + 0x24 * i
}

/// Items on for the match: a frequency (`StartMeleeRules::item_freq`) and the switch's every
/// kind (the spawner's mask, `it_804A0E30` +0x18).
fn mk_items_on(c: &mut Call) {
    poke_u8(c, 0x8046_DB73, 2);
    let sp = leaf_check::sym("it_804A0E30");
    poke_u32(c, sp + 0x18, !0);
    poke_u32(c, sp + 0x1C, !0);
}

/// Every block's state (`x2`), what it holds, and its counts.
fn mk_blocks(c: &mut Call, x2: u16, status: u16, x4: u16, x20: u16, x22: u16) {
    for i in 0..19 {
        let b = mk_block(c, i);
        poke_u16(c, b, status);
        poke_u16(c, b + 2, x2);
        poke_u16(c, b + 4, x4);
        poke_u16(c, b + 0x20, x20);
        poke_u16(c, b + 0x22, x22 + (i as u16 & 1));
    }
}

/// A function of a stage object, run on the object in r3.
fn mk_of(
    f: fn(&mut Running, melee_sim::hsd::gobj::GObjId),
) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, maps: &Maps, call: &Call| {
        f(r, gobj_arg(maps, call));
        Ret::Nothing
    }
}

#[test]
fn inishie1_setup() {
    let d = dir(MK);
    check_calls_with(&d, "grInishie1_801FA90C", 1, &as_saved, &event);
    check_calls_with(&d, "grInishie1_801FA988", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        inishie1::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in 0..4u32 {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "setupStageCallbacks", 1, &set, &create);
    }
    for f in ["stageGObj0_OnInit", "grInishie1_801FAADC"] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    // The blocks, with items off as saved, and on: two blocks under ? blocks.
    let blocks = mk_of(inishie1::init_blocks);
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        mk_items_on(c)
    }] {
        check_calls_with(&d, "grInishie1_801FAD84", 1, p, &blocks);
    }
    let lifts = mk_of(inishie1::init_lifts);
    check_calls_with(&d, "grInishie1_801FC018", 1, &as_saved, &lifts);
}

/// The stage's process and the blocks': as saved, every block in each state with its counts
/// run out and not, a bump rising and falling from each height, the ? blocks' waits running
/// out (the first's, the second's), every block's item at once (the blocks broken, blinking
/// in or at rest under their new ? blocks), and the wait after it.
#[test]
fn inishie1_blocks() {
    let d = dir(MK);
    check_calls_with(&d, "grInishie1_801FAB68", 1, &as_saved, &by_address);
    let update = mk_of(inishie1::update_blocks);
    check_calls_with(&d, "grInishie1_801FB3F0", 1, &as_saved, &update);
    for x2 in 0..6u16 {
        for (x4, x20, x22) in [(0u16, 0u16, 0u16), (3, 1, 2), (0, 0, 5)] {
            let set = move |c: &mut Call, _: usize| mk_blocks(c, x2, 0, x4, x20, x22);
            check_calls_with(&d, "grInishie1_801FB3F0", 1, &set, &update);
        }
    }
    for y in [0.5f32, 4.5, 5.5, -0.5] {
        let set = move |c: &mut Call, _: usize| {
            mk_blocks(c, 4, 0, 0, 0, 0);
            for i in 0..19 {
                let b = mk_block(c, i);
                poke_u32(c, b + 0xC, y.to_bits());
                let speed = if i % 2 == 0 { 1.2f32 } else { -1.2 };
                poke_u32(c, b + 0x10, speed.to_bits());
            }
        };
        check_calls_with(&d, "grInishie1_801FB3F0", 1, &set, &update);
    }
    for (c6, c8, d8, x2) in [
        (1u16, 0u16, 0u32, 2u16),
        (0, 1, 0, 2),
        (1, 1, 0, 2),
        (1, 2, 0, 0),
        (2, 1, 0, 1),
        (1, 2, 0, 1),
        (1, 2, 5, 2),
        (300, 0, 0, 2),
    ] {
        let set = move |c: &mut Call, _: usize| {
            mk_blocks(c, x2, 0, 0, 1, 3);
            let v = mk_vars(c);
            poke_u16(c, v + 2, c6);
            poke_u16(c, v + 4, c8);
            poke_u32(c, v + 0x14, d8);
        };
        check_calls_with(&d, "grInishie1_801FB3F0", 1, &set, &update);
    }
}

/// A block bumped from below (the ceiling's callback, then what it gave once the pass was
/// over) and hit (its item's damage callback), each holding nothing and an item, and a
/// block's giving up and breaking on their own.
#[test]
fn inishie1_breaks() {
    let d = dir(MK);
    let bumped = |r: &mut Running, _: &Maps, call: &Call| {
        let mut b = inishie1::Bumps::default();
        b.bumped(call.regs.gpr[4] as i32, call.regs.fpr[1].ps0 as f32);
        b.apply(r);
        Ret::Nothing
    };
    let prepares: [&Prepare; 6] = [
        &as_saved,
        &|c, _| c.regs.fpr[1].ps0 = 0.5,
        &|c, _| c.regs.fpr[1].ps0 = -0.75,
        &|c, _| c.regs.fpr[1].ps0 = f64::from(0.700_000_1f32),
        &|c, _| mk_blocks(c, 2, 1, 0, 0, 0),
        &|c, _| mk_blocks(c, 2, 3, 0, 0, 0),
    ];
    for p in prepares {
        check_calls_with(&d, "fn_801FBEB8", 1, p, &bumped);
    }
    let ground = |maps: &Maps, a: u32| *maps.grounds.get(&a).expect("the stage's object");
    let hit = move |r: &mut Running, maps: &Maps, call: &Call| {
        inishie1::block_hit(r, gobj_arg(maps, call), ground(maps, call.regs.gpr[4]));
        Ret::Nothing
    };
    for status in [None, Some(1u16), Some(2), Some(3)] {
        let set = move |c: &mut Call, _: usize| {
            if let Some(s) = status {
                mk_blocks(c, 2, s, 0, 0, 0);
            }
        };
        check_calls_with(&d, "fn_801FBF6C", 1, &set, &hit);
    }
    let give_up = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie1::give_up(r, gobj_arg(maps, call), call.regs.gpr[4] as usize);
        Ret::Nothing
    };
    let brk = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie1::break_block(r, gobj_arg(maps, call), call.regs.gpr[4] as usize);
        Ret::Nothing
    };
    for status in [None, Some(0u16), Some(1), Some(2), Some(3)] {
        let set = move |c: &mut Call, _: usize| {
            if let Some(s) = status {
                mk_blocks(c, 2, s, 0, 0, 0);
                // Another block holding every block's item too, or nothing.
                let other = mk_block(c, (c.regs.gpr[4] + 1) % 19);
                poke_u16(c, other, if s == 3 { 3 } else { 0 });
            }
        };
        check_calls_with(&d, "grInishie1_801FB0AC", 1, &set, &give_up);
        check_calls_with(&d, "grInishie1_801FBCEC", 1, &set, &brk);
    }
}

/// The ? blocks, from the blocks' process's calls: one put over a block broken, blinking in
/// and at rest, one taken off, an item popped out with items off and on, and a ? block's setup
/// and process on a stage object stood in for one.
#[test]
fn inishie1_hatena() {
    let d = dir(MK);
    let from = "grInishie1_801FB3F0";
    let add = |r: &mut Running, maps: &Maps, call: &Call| {
        let gobj = gobj_arg(maps, call);
        let ix = call.regs.gpr[4] as usize;
        inishie1::add_hatena(r, gobj, ix, inishie1::PROC_HATENA_FRAME);
        Ret::Nothing
    };
    let t = leaf_check::sym("grInishie1_801FBAA0");
    for (ix, x2) in [(0u32, 0u16), (7, 1), (18, 2)] {
        let set = move |c: &mut Call, _: usize| {
            c.addr = t;
            c.regs.gpr[4] = ix;
            mk_blocks(c, x2, 0, 0, 0, 0);
        };
        check_calls_with(&d, from, 1, &set, &add);
    }
    let remove = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie1::remove_hatena(r, gobj_arg(maps, call), call.regs.gpr[4] as usize);
        Ret::Nothing
    };
    let t = leaf_check::sym("grInishie1_801FBC4C");
    let set = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[4] = 4;
    };
    check_calls_with(&d, from, 1, &set, &remove);
    let pop = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie1::pop_item(r, maps.jobjs.get(&call.regs.gpr[4]).copied());
        Ret::Nothing
    };
    let t = leaf_check::sym("grInishie1_801FBA34");
    for items in [false, true] {
        let set = move |c: &mut Call, _: usize| {
            c.addr = t;
            c.regs.gpr[4] = peek_u32(c, mk_block(c, 3) + 0x14);
            if items {
                mk_items_on(c);
            }
        };
        check_calls_with(&d, from, 1, &set, &pop);
    }
    // A ? block's setup and its process (blinking in at a count odd and even and at its last,
    // done, hidden for good; following block 3), on map 0's object stood in for one: its
    // variables taken as the ? block's from memory, as the importer would type them.
    let t = leaf_check::sym("grInishie1_801FAC04");
    check_calls_with(&d, "stageGObj0_OnInit", 1, &retarget(t), &by_address);
    let proc = |r: &mut Running, maps: &Maps, call: &Call| {
        let gobj = gobj_arg(maps, call);
        let g = melee_sim::game::gr::ground::ground_of(r.game, gobj);
        let v = vars(call);
        r.game.grounds[g.0].u = melee_sim::game::gr::GroundVars::Inishie1Hatena(
            melee_sim::game::gr::vars::Inishie1HatenaVars {
                xC4: maps.jobjs.get(&peek_u32(call, v)).copied(),
                xC8: peek_u32(call, v + 4) as i32,
                xCC: peek_u32(call, v + 8) as i32,
            },
        );
        inishie1::proc_hatena(r, gobj);
        Ret::Nothing
    };
    let t = leaf_check::sym("grInishie1_801FAC58");
    for (count, hidden, on_block) in [
        (3u32, 0u32, true),
        (4, 0, true),
        (1, 0, true),
        (0, 0, false),
        (3, 1, true),
    ] {
        let set = move |c: &mut Call, _: usize| {
            c.addr = t;
            // The object's own model stands in for a block's joint (the stage's isn't made
            // yet at this call).
            let joint = if on_block {
                peek_u32(c, c.regs.gpr[3] + 0x28)
            } else {
                0
            };
            let v = vars(c);
            poke_u32(c, v, joint);
            poke_u32(c, v + 4, count);
            poke_u32(c, v + 8, hidden);
        };
        check_calls_with(&d, "stageGObj0_OnInit", 1, &set, &proc);
    }
}

/// The lifts: as saved; balancing with no weight (drifting back from either side), weight on
/// one side and both, more than the cap, past the most they part (falling); falling, below
/// the blast zone and not; gone with the wait running out and not; coming back from either
/// side; and their collision joints touched by each kind.
#[test]
fn inishie1_lifts() {
    let d = dir(MK);
    let lifts = mk_of(inishie1::update_lifts);
    check_calls_with(&d, "grInishie1_801FC664", 1, &as_saved, &lifts);
    let set_lifts = |c: &mut Call, ee: u16, e0: f32, e4: f32, f0: f32, f4: f32, ec: u16| {
        let v = mk_vars(c);
        poke_u32(c, v + 0x1C, e0.to_bits());
        poke_u32(c, v + 0x20, e4.to_bits());
        poke_u16(c, v + 0x28, ec);
        poke_u16(c, v + 0x2A, ee);
        poke_u32(c, v + 0x2C, f0.to_bits());
        poke_u32(c, v + 0x30, f4.to_bits());
    };
    let cases: [(u16, f32, f32, f32, f32, u16); 16] = [
        (0, 0.0, 0.0, 3.0, 0.0, 0),
        (0, 0.0, 0.0, -3.0, 0.0, 0),
        (0, 0.0, 0.0, 0.0, 0.0, 0),
        (0, 150.0, 0.0, 0.0, 0.0, 0),
        (0, 0.0, 150.0, 0.0, 0.5, 0),
        (0, 120.0, 90.0, 1.0, -0.5, 0),
        (0, 200_000.0, 0.0, 0.0, 0.0, 0),
        (0, 0.0, 400.0, 0.0, 40.0, 0),
        (0, 400.0, 0.0, -500.0, -40.0, 0),
        (0, 0.0, 400.0, 500.0, 40.0, 0),
        (1, 0.0, 0.0, 30.0, 0.5, 0),
        (1, 0.0, 0.0, -30.0, 500.0, 0),
        (2, 0.0, 0.0, 30.0, 0.0, 1),
        (2, 0.0, 0.0, 30.0, 0.0, 5),
        (3, 0.0, 0.0, 0.1, 0.0, 0),
        (3, 0.0, 0.0, -50.0, 0.0, 0),
    ];
    for (ee, e0, e4, f0, f4, ec) in cases {
        let set = move |c: &mut Call, _: usize| set_lifts(c, ee, e0, e4, f0, f4, ec);
        check_calls_with(&d, "grInishie1_801FC664", 1, &set, &lifts);
    }
    // Falling with both platforms already far below (their joints' translation y).
    let below = move |c: &mut Call, _: usize| {
        set_lifts(c, 1, 0.0, 0.0, 30.0, 2.0, 0);
        let v = mk_vars(c);
        for at in [0x3C, 0x40] {
            let j = peek_u32(c, v + at);
            poke_u32(c, j + 0x3C, (-100_000.0f32).to_bits());
        }
    };
    check_calls_with(&d, "grInishie1_801FC664", 1, &below, &lifts);
    let balance = mk_of(inishie1::balance);
    check_calls_with(&d, "grInishie1_801FC110", 1, &as_saved, &balance);
    for (_, e0, e4, f0, f4, _) in cases.iter().copied().filter(|c| c.0 == 0) {
        let set = move |c: &mut Call, _: usize| set_lifts(c, 0, e0, e4, f0, f4, 0);
        check_calls_with(&d, "grInishie1_801FC110", 1, &set, &balance);
    }
    let level = mk_of(inishie1::level_out);
    let t = leaf_check::sym("grInishie1_801FC4A0");
    for f0 in [0.0f32, 0.1, -0.1, 50.0, -50.0] {
        let set = move |c: &mut Call, _: usize| {
            set_lifts(c, 3, 0.0, 0.0, f0, 0.0, 0);
            c.addr = t;
        };
        check_calls_with(&d, "grInishie1_801FC664", 1, &set, &level);
    }
    let touched = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps
            .grounds
            .get(&call.regs.gpr[3])
            .expect("the stage's object");
        let param = r.game.gr.inishie1_yakumono;
        let a = &call.regs.gpr;
        let (joint, x50, kind) = (a[4] as i32, a[6] as i32, a[7] as i32);
        inishie1::lift_touched(&mut r.game.grounds, param, r.data, g, joint, x50, kind);
        Ret::Nothing
    };
    let prepares: [&Prepare; 5] = [
        &as_saved,
        &|c, _| c.regs.gpr[4] ^= 1,
        &|c, _| c.regs.gpr[4] = 3,
        &|c, _| c.regs.gpr[7] = 1,
        &|c, _| c.regs.gpr[7] = 2,
    ];
    for p in prepares {
        check_calls_with(&d, "fn_801FC9AC", 1, p, &touched);
    }
}

/// The music's speed-up (as saved, and already sped up), and the callbacks no capture
/// reaches, from the stage's process's calls: the empty processes and destroy callbacks, map
/// 1's setup, the shadow check, and the load and demo hooks.
#[test]
fn inishie1_unreached_callbacks() {
    let d = dir(MK);
    // The song's change itself (`lbAudioAx_80023F28`) is stood in for on the reference's side:
    // a captured state's audio has no timeline to start a stream on, so ours tells the
    // frontend instead. The oracle's match plays the change (its fast song from tick 13,326).
    let music = mk_of(inishie1::music);
    let stubs = [(leaf_check::sym("lbAudioAx_80023F28"), None)];
    let setup = leaf_check::calls::Setup {
        stubs: &stubs,
        ..Default::default()
    };
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| poke_u8(c, mk_vars(c), 0x80),
        &|c: &mut Call, _: usize| poke_u8(c, mk_vars(c), 0x7F),
    ] {
        leaf_check::calls::check_calls_in(&d, "grInishie1_801FCB10", 1, p, &setup, &music);
    }
    let from = "grInishie1_801FAB68";
    for name in [
        "grInishie1_801FAAD4",
        "grInishie1_801FAAD8",
        "grInishie1_801FABB0",
        "grInishie1_801FABFC",
        "grInishie1_801FAC00",
        "grInishie1_801FAD80",
        "grInishie1_801FABB4",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(inishie1::check_shadow_render()));
    let t = leaf_check::sym("grInishie1_801FCBB8");
    check_calls_with(&d, from, 1, &retarget(t), &shadow);
    let t = leaf_check::sym("grInishie1_801FA984");
    check_calls_with(&d, from, 1, &retarget(t), &event);
    let t = leaf_check::sym("grInishie1_801FA908");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
}

// Mushroom Kingdom II (`grinishie2.c`), Birdo (`itkyasarin.c`) and her eggs
// (`itkyasarinegg.c`).

const MK2: &str = "oracle-stage-mushroom-kingdom-2";

/// The item in r3's user data (`Item*`).
fn mk2_item(c: &Call) -> u32 {
    peek_u32(c, c.regs.gpr[3] + 0x2C)
}

/// The stage's flags (map 2's `xC4_flags`) and one of its waits (`xC6` + 2·k).
fn mk2_stage(c: &mut Call, flags: u8, wait: Option<(u32, u16)>) {
    let v = map_vars(c, 2);
    poke_u8(c, v, flags);
    if let Some((k, w)) = wait {
        poke_u16(c, v + 2 + 2 * k, w);
    }
}

/// A predicate of an item, run on the item in r3.
fn mk2_pred(
    f: fn(&mut Running, melee_sim::hsd::gobj::GObjId) -> bool,
) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, maps: &Maps, call: &Call| Ret::Int(u32::from(f(r, gobj_arg(maps, call))))
}

#[test]
fn inishie2_setup() {
    let d = dir(MK2);
    check_calls_with(&d, "grInishie2_801FCBC4", 1, &as_saved, &event);
    check_calls_with(&d, "grInishie2_801FCCB0", 1, &as_saved, &event);
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        inishie2::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    for map in [0u32, 1, 2, 3, 4, 15] {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "grInishie2_801FCCDC", 1, &set, &create);
    }
    for f in [
        "stageGObj0_OnInit",
        "grInishie2_801FCE04",
        "grInishie2_801FD198",
        "grInishie2_801FD4F0",
        "grInishie2_801FDE8C",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    for (f, port) in [
        (
            "grInishie2_801FD018",
            inishie2::init_logs as fn(&mut Running, _),
        ),
        ("grInishie2_801FD368", inishie2::init_birdo_wait),
        ("grInishie2_801FD744", inishie2::init_pidgit_wait),
        ("grInishie2_801FD824", inishie2::pidgit_in),
    ] {
        check_calls_with(&d, f, 1, &as_saved, &mk_of(port));
    }
}

/// The stage's process and its sending: as saved, and each wait running out, at zero and
/// running, with its flag on and off; and the music (its change stood in for, as Mushroom
/// Kingdom's).
#[test]
fn inishie2_stage() {
    let d = dir(MK2);
    check_calls_with(&d, "grInishie2_801FCFCC", 1, &as_saved, &by_address);
    for (f, port, k, flag) in [
        (
            "grInishie2_801FD0B4",
            inishie2::send_logs as fn(&mut Running, _),
            0u32,
            0x40u8,
        ),
        ("grInishie2_801FD0B4", inishie2::send_logs, 1, 0x20),
        ("grInishie2_801FD3CC", inishie2::send_birdo, 2, 0x10),
        ("grInishie2_801FD7A8", inishie2::send_pidgit, 3, 0x08),
    ] {
        let ours = mk_of(port);
        check_calls_with(&d, f, 1, &as_saved, &ours);
        for (flags, w) in [
            (flag, 0xFFFFu16),
            (flag, 0),
            (flag, 30),
            (0, 0xFFFF),
            (0xF8, 0xFFFF),
        ] {
            let set = move |c: &mut Call, _: usize| mk2_stage(c, flags, Some((k, w)));
            check_calls_with(&d, f, 1, &set, &ours);
        }
    }
    let music = mk_of(inishie2::music);
    let stubs = [(leaf_check::sym("lbAudioAx_80023F28"), None)];
    let setup = leaf_check::calls::Setup {
        stubs: &stubs,
        ..Default::default()
    };
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| mk2_stage(c, 0x80, None),
        &|c: &mut Call, _: usize| mk2_stage(c, 0x7F, None),
    ] {
        leaf_check::calls::check_calls_in(&d, "grInishie2_801FCF2C", 1, p, &setup, &music);
    }
}

/// The logs' process: as saved, its collision back already and not, its wait running out,
/// for either log.
#[test]
fn inishie2_logs() {
    let d = dir(MK2);
    check_calls_with(&d, "grInishie2_801FD224", 1, &as_saved, &by_address);
    for (flags, w, which) in [
        (0u8, 0xFFFFu16, 0u16),
        (0, 0xFFFF, 1),
        (0, 0, 0),
        (0, 7, 1),
        (0x80, 0xFFFF, 0),
        (0x7F, 0xFFFF, 1),
    ] {
        let set = move |c: &mut Call, _: usize| {
            let v = vars(c);
            poke_u8(c, v, flags);
            poke_u16(c, v + 2, w);
            poke_u16(c, v + 4, which);
        };
        check_calls_with(&d, "grInishie2_801FD224", 1, &set, &by_address);
    }
}

/// Birdo's platform: its process as saved and with her gone, her being gone (the stage's wait
/// again, the platform destroyed), and her knocked off (its collision out).
#[test]
fn inishie2_birdo_platform() {
    let d = dir(MK2);
    check_calls_with(&d, "grInishie2_801FD654", 1, &as_saved, &by_address);
    let gone = |c: &mut Call, _: usize| poke_u32(c, vars(c), 0);
    check_calls_with(&d, "grInishie2_801FD654", 1, &gone, &by_address);
    let birdo_gone = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie2::birdo_gone(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "grInishie2_801FD448", 1, &as_saved, &birdo_gone);
    let down = |r: &mut Running, maps: &Maps, call: &Call| {
        inishie2::birdo_down(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let t = leaf_check::sym("grInishie2_801FD4CC");
    check_calls_with(&d, "grInishie2_801FD448", 1, &retarget(t), &down);
}

/// Pidgit's carpet: its process and flight as saved and with its flags and turns set to each
/// branch (coming in, on its way out, its first turns, later turns, leaving), and the draw.
#[test]
fn inishie2_pidgit() {
    let d = dir(MK2);
    for f in ["grInishie2_801FDED8", "grInishie2_801FD9EC"] {
        let ours = if f == "grInishie2_801FDED8" {
            mk_of(inishie2::proc_pidgit)
        } else {
            mk_of(inishie2::pidgit_fly)
        };
        check_calls_with(&d, f, 1, &as_saved, &ours);
        for (flags, turns) in [
            (0x40u8, 0u16),
            (0x00, 0),
            (0x10, 1),
            (0x00, 2),
            (0x10, 5),
            (0x20, 3),
            (0x80, 3),
            (0xA0, 3),
            (0x0F, 4),
        ] {
            let set = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u8(c, v + 4, flags);
                poke_u16(c, v + 6, turns);
            };
            check_calls_with(&d, f, 1, &set, &ours);
        }
    }
    let draw = |r: &mut Running, _: &Maps, call: &Call| {
        Ret::Int(inishie2::random_below(r, call.regs.gpr[3] as i32) as u32)
    };
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| c.regs.gpr[3] = 0,
        &|c: &mut Call, _: usize| c.regs.gpr[3] = 7,
    ] {
        check_calls_with(&d, "grInishie2_801FDFB4", 1, p, &draw);
    }
}

/// The callbacks no capture reaches or a fixed answer, from the stage's process's calls: the
/// empty processes and destroy callbacks, the line hook and the shadow check, the load and demo
/// hooks.
#[test]
fn inishie2_unreached_callbacks() {
    let d = dir(MK2);
    let from = "grInishie2_801FCFCC";
    for name in [
        "grInishie2_801FCDFC",
        "grInishie2_801FCE00",
        "grInishie2_801FD014",
        "grInishie2_801FD364",
        "grInishie2_801FD740",
        "grInishie2_801FDFE4",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(inishie2::check_shadow_render()));
    let t = leaf_check::sym("grInishie2_801FDFF0");
    check_calls_with(&d, from, 1, &retarget(t), &shadow);
    let none = |_: &mut Running, _: &Maps, _: &Call| Ret::Int(0);
    check_calls_with(&d, "grInishie2_801FDFE8", 1, &as_saved, &none);
    let t = leaf_check::sym("grInishie2_801FCCAC");
    check_calls_with(&d, from, 1, &retarget(t), &event);
    let t = leaf_check::sym("grInishie2_801FCBC0");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
}

/// Birdo's spawn (facing either way), destroy, and every state's callbacks, each as saved and
/// with her variables set to its branches; her hits (a flinch, a turn away, knocked off).
#[test]
fn kyasarin_states() {
    let d = dir(MK2);
    for f in [
        "it_802ECE90",
        "it_802ECEB0",
        "it_802ECFE0",
        "it_802ED0D0",
        "it_802ED25C",
        "it_802ED4F8",
        "it_802ED774",
        "it_802ED8BC",
        "itKyasarin_Logic25_Destroyed",
    ] {
        let port: fn(&mut Running, melee_sim::hsd::gobj::GObjId) = match f {
            "it_802ECE90" => kyasarin::spawned,
            "it_802ECEB0" => kyasarin::walk_in,
            "it_802ECFE0" => kyasarin::wait_in,
            "it_802ED0D0" => kyasarin::back_off,
            "it_802ED25C" => kyasarin::walk_in_again,
            "it_802ED4F8" => kyasarin::stand,
            "it_802ED774" => kyasarin::spit,
            "it_802ED8BC" => kyasarin::turn,
            _ => kyasarin::destroyed,
        };
        check_calls_with(&d, f, 1, &as_saved, &mk_of(port));
    }
    // The states' callbacks: each from its own calls, with her place and facing set either
    // side of her marks, her waits at and past zero, her stand's and egg's waits, her eggs
    // left, the state a flinch goes back to.
    let set_birdo =
        |c: &mut Call, facing: f32, x: f32, x24: f32, x28: f32, x2c: f32, x34: u32, x3c: u32| {
            let ip = mk2_item(c);
            poke_u32(c, ip + 0x2C, facing.to_bits());
            poke_u32(c, ip + 0x4C, x.to_bits());
            let v = ip + 0xDD4;
            poke_u32(c, v + 0x24, x24.to_bits());
            poke_u32(c, v + 0x28, x28.to_bits());
            poke_u32(c, v + 0x2C, x2c.to_bits());
            poke_u32(c, v + 0x34, x34);
            poke_u32(c, v + 0x3C, x3c);
        };
    let cases: [(f32, f32, f32, f32, f32, u32); 8] = [
        (1.0, 200.0, 0.5, 5.0, 5.0, 2),
        (1.0, -200.0, -0.5, -0.5, 5.0, 1),
        (-1.0, 200.0, 0.0, 5.0, -0.5, 0),
        (-1.0, -200.0, 10.0, -0.5, -0.5, 3),
        (1.0, 0.0, 0.9, 0.9, 0.9, 1),
        (-1.0, 0.0, 1.5, 1.5, 1.5, 4),
        (1.0, 40.0, 0.0, 0.0, 0.0, 0),
        (-1.0, -40.0, 0.0, 0.0, 0.0, 2),
    ];
    for (f, port) in [
        (
            "itKyasarin_UnkMotion0_Anim",
            kyasarin::anim0 as fn(&mut Running, _) -> bool,
        ),
        ("itKyasarin_UnkMotion0_Coll", kyasarin::coll0),
        ("itKyasarin_UnkMotion1_Anim", kyasarin::anim1),
        ("itKyasarin_UnkMotion2_Anim", kyasarin::anim2),
        ("itKyasarin_UnkMotion2_Coll", kyasarin::coll2),
        ("itKyasarin_UnkMotion3_Anim", kyasarin::anim3),
        ("itKyasarin_UnkMotion4_Anim", kyasarin::anim4),
        ("itKyasarin_UnkMotion4_Coll", kyasarin::coll4),
        ("itKyasarin_UnkMotion5_Anim", kyasarin::anim5),
        ("itKyasarin_UnkMotion6_Anim", kyasarin::anim6),
        ("itKyasarin_UnkMotion7_Anim", kyasarin::anim7),
        ("itKyasarin_UnkMotion8_Anim", kyasarin::anim8),
    ] {
        let ours = mk2_pred(port);
        check_calls_with(&d, f, 1, &as_saved, &ours);
        for (facing, x, x24, x28, x2c, x34) in cases {
            for x3c in [x34, x34 + 5] {
                let set =
                    move |c: &mut Call, _: usize| set_birdo(c, facing, x, x24, x28, x2c, x34, x3c);
                check_calls_with(&d, f, 1, &set, &ours);
            }
        }
    }
    // The other states' callbacks, from the standing animation's calls: falling toward the
    // screen and bouncing away.
    for (name, port) in [
        (
            "itKyasarin_UnkMotion9_Anim",
            kyasarin::anim9 as fn(&mut Running, _) -> bool,
        ),
        ("itKyasarin_UnkMotion10_Anim", kyasarin::anim10),
        ("itKyasarin_UnkMotion9_Coll", kyasarin::coll9),
        ("itKyasarin_UnkMotion10_Coll", kyasarin::coll10),
    ] {
        let t = leaf_check::sym(name);
        check_calls_with(
            &d,
            "itKyasarin_UnkMotion6_Anim",
            1,
            &retarget(t),
            &mk2_pred(port),
        );
    }
    for (name, port) in [
        (
            "itKyasarin_UnkMotion9_Phys",
            kyasarin::phys9 as fn(&mut Running, _),
        ),
        ("itKyasarin_UnkMotion10_Phys", kyasarin::phys10),
    ] {
        let t = leaf_check::sym(name);
        check_calls_with(
            &d,
            "itKyasarin_UnkMotion6_Anim",
            1,
            &retarget(t),
            &mk_of(port),
        );
    }
    // Her hits: the damage she has taken and the hit's set for a flinch, a turn away and a
    // fall (toward the screen or bouncing, by the seed).
    let damaged = mk2_pred(kyasarin::damaged);
    check_calls_with(&d, "it_802EDDC0", 1, &as_saved, &damaged);
    for (taken, hit, seed) in [
        (0u32, 1u32, 1u32),
        (20, 10, 1),
        (30, 40, 1),
        (60, 30, 7),
        (60, 30, 0x1234_5678),
        (90, 90, 0x9ABC_DEF0),
    ] {
        let set = move |c: &mut Call, _: usize| {
            let ip = mk2_item(c);
            poke_u32(c, ip + 0xDD4 + 0x38, taken);
            poke_u32(c, ip + 0xCA0, hit);
            set_seed(c, seed);
        };
        check_calls_with(&d, "it_802EDDC0", 1, &set, &damaged);
    }
}

/// Birdo's spawn on her platform, as saved and facing the other way.
#[test]
fn kyasarin_spawn() {
    let d = dir(MK2);
    let spawn = |r: &mut Running, maps: &Maps, call: &Call| {
        let platform = gobj_arg(maps, call);
        let p = call.regs.gpr[4];
        let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        kyasarin::spawn(r, platform, pos, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        c.regs.fpr[1].ps0 = -c.regs.fpr[1].ps0
    }] {
        check_calls_with(&d, "it_802ECD3C", 1, p, &spawn);
    }
}

/// Birdo's eggs: an egg spat (from her spit's calls, either way), its roll's collision as saved
/// and from each state; its bounce, its throw's and drop's states, the falls' physics and
/// collisions, its break (rolling or bouncing, thrown: empty or dropping items, by the
/// seed), its fade, and a shield bounce, from the roll's calls.
#[test]
fn kyasarin_eggs() {
    let d = dir(MK2);
    let spit = |r: &mut Running, maps: &Maps, call: &Call| {
        let parent = maps.gobjs.get(&call.regs.gpr[3]).copied();
        let p = call.regs.gpr[4];
        let pos = Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
        kyasarinegg::spawn(r, parent, pos, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| c.regs.fpr[1].ps0 = -c.regs.fpr[1].ps0,
        &|c: &mut Call, _: usize| c.regs.gpr[3] = 0,
    ] {
        check_calls_with(&d, "it_802EFA44", 1, p, &spit);
    }
    check_calls_with(&d, "it_802EFB0C", 1, &as_saved, &mk_of(kyasarinegg::roll));
    let from = "itKyasarinegg_UnkMotion0_Coll";
    check_calls_with(&d, from, 1, &as_saved, &mk2_pred(kyasarinegg::coll0));
    // The egg in each state (its msid), broken already or not, with the seed set.
    let state = |msid: u32, broken: u32, seed: u32| {
        move |c: &mut Call, _: usize| {
            let ip = mk2_item(c);
            poke_u32(c, ip + 0x24, msid);
            poke_u32(c, ip + 0xDD4 + 0x20, broken);
            set_seed(c, seed);
        }
    };
    for (name, port) in [
        (
            "itKyasarinegg_UnkMotion0_Coll",
            kyasarinegg::coll0 as fn(&mut Running, _) -> bool,
        ),
        ("itKyasarinegg_UnkMotion1_Coll", kyasarinegg::coll1),
        ("itKyasarinegg_UnkMotion3_Coll", kyasarinegg::coll3),
        ("itKyasarinegg_UnkMotion4_Anim", kyasarinegg::anim4),
        ("it_2725_Logic28_DmgDealt", kyasarinegg::broken),
        ("it_2725_Logic28_Clanked", kyasarinegg::broken),
        ("it_2725_Logic28_HitShield", kyasarinegg::broken),
        ("it_2725_Logic28_Reflected", kyasarinegg::broken),
        ("it_802F022C", kyasarinegg::broken),
        (
            "itKyasarinEgg_Logic28_ShieldBounced",
            kyasarinegg::shield_bounced,
        ),
    ] {
        let t = leaf_check::sym(name);
        for (msid, broken, seed) in [
            (0u32, 0u32, 1u32),
            (1, 0, 5),
            (3, 0, 1),
            (3, 0, 0x5555_1234),
            (3, 1, 9),
        ] {
            let prep = state(msid, broken, seed);
            let set = move |c: &mut Call, k: usize| {
                prep(c, k);
                c.addr = t;
            };
            check_calls_with(&d, from, 1, &set, &mk2_pred(port));
        }
    }
    for (name, port) in [
        ("it_802EFCC0", kyasarinegg::bounce as fn(&mut Running, _)),
        ("it_802EFD84", kyasarinegg::break_empty),
        ("itKyasarinEgg_Logic28_PickedUp", kyasarinegg::picked_up),
        ("itKyasarinEgg_Logic28_Dropped", kyasarinegg::thrown),
        ("itKyasarinEgg_Logic28_Thrown", kyasarinegg::thrown),
        ("itKyasarinegg_UnkMotion1_Phys", kyasarinegg::falling),
        ("itKyasarinegg_UnkMotion3_Phys", kyasarinegg::falling),
    ] {
        let t = leaf_check::sym(name);
        check_calls_with(&d, from, 1, &retarget(t), &mk_of(port));
    }
}

// Icicle Mountain (`gricemt.c`).

const IM: &str = "oracle-stage-icicle-mountain";

/// Writes an `s16` at `a`.
fn poke_h(c: &mut Call, a: u32, v: i16) {
    let w = peek_u32(c, a & !3);
    let w = if a & 2 == 0 {
        (w & 0xFFFF) | (v as u16 as u32) << 16
    } else {
        (w & 0xFFFF_0000) | v as u16 as u32
    };
    poke_u32(c, a & !3, w);
}

fn peek_h(c: &Call, a: u32) -> i16 {
    let w = peek_u32(c, a & !3);
    (if a & 2 == 0 { w >> 16 } else { w }) as u16 as i16
}

/// A field's model's translation y (the map's GObj's joint), set.
fn im_field_y(c: &mut Call, map: u32, y: f32) {
    let gobj = peek_u32(c, STAGE_INFO + 0x180 + 4 * map);
    if gobj == 0 {
        return;
    }
    let j = peek_u32(c, gobj + 0x28);
    poke_u32(c, j + 0x3C, y.to_bits());
}

/// The climb's speed state set: phase, delay, the change's ticks, a burst's and the speed.
fn im_state(c: &mut Call, v: u32, phase: i16, delay: i16, lerp: i16, burst: i16, cur: f32) {
    poke_h(c, v + 4, phase);
    poke_h(c, v + 6, delay);
    poke_h(c, v + 8, lerp);
    poke_h(c, v + 0xA, burst);
    poke_u32(c, v + 0x10, cur.to_bits());
}

#[test]
fn icemt_setup() {
    let d = dir(IM);
    for f in ["grIceMt_801F686C", "grIceMt_801F7080", "grIceMt_801F71DC"] {
        check_calls_with(&d, f, 1, &as_saved, &event);
    }
    for f in [
        "stageGObj0_OnInit",
        "stageGObj9_OnInit",
        "stageGObj8_OnInit",
        "fn_801F75EC",
        "stageGObj1_OnInit",
        "stageGObj2_OnInit",
        "stageGObj3_OnInit",
        "stageGObj4_OnInit",
        "stageGObj5_OnInit",
        "stageGObj6_OnInit",
    ] {
        check_calls_with(&d, f, 0, &as_saved, &by_address);
    }
    // setupStageCallbacks for each map from its saved calls. The climb's process calls it
    // right after destroying a field, whose `Ground` block the new object's takes; the
    // importer brings that freed block in as a live object (the ice's collision callbacks
    // still point at it), so it's freed on our side first, as the original's heap had it.
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let stale: Vec<u32> = (r.game.grounds.iter())
            .filter(|(i, g)| {
                g.gobj.is_none_or(|go| {
                    r.game.world.gobjs.is_free(go.0)
                        || r.game.world.gobjs[go.0].user_data
                            != Some(melee_sim::game::state::UserData::Ground(
                                melee_sim::game::handles::GroundId(*i),
                            ))
                })
            })
            .map(|(i, _)| i)
            .collect();
        for i in stale {
            r.game.grounds.free(i);
        }
        // The setup's call, or the climb's process's (whose r1 is lower).
        let frame = if call.regs.gpr[1] < 0x804E_E9A0 {
            icemt::CLIMB_PROC_FRAME
        } else {
            icemt::SETUP_FRAME
        };
        icemt::create(r, call.regs.gpr[3] as i32, frame);
        Ret::Nothing
    };
    check_calls_with(&d, "setupStageCallbacks", 1, &as_saved, &create);
    for map in [0u32, 8] {
        let set = move |c: &mut Call, _: usize| c.regs.gpr[3] = map;
        check_calls_with(&d, "setupStageCallbacks", 1, &set, &create);
    }
    // The climb's first two fields, as saved and as each other pair.
    let place = |r: &mut Running, maps: &Maps, call: &Call| {
        icemt::place_fields(r, rc_ground(maps, call.regs.gpr[3]));
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801FA0BC", 1, &as_saved, &place);
    for (under, upper) in [(5i16, 6i16), (6, 1), (3, 4)] {
        let set = move |c: &mut Call, _: usize| {
            let v = c.regs.gpr[3];
            poke_h(c, v, under);
            poke_h(c, v + 2, upper);
        };
        check_calls_with(&d, "grIceMt_801FA0BC", 1, &set, &place);
    }
    // A field's blocks, the falling ice's setup, a part's index and the scenery's points.
    let blocks = |r: &mut Running, maps: &Maps, call: &Call| {
        let parts: Vec<i32> = (0..call.regs.gpr[5])
            .map(|i| i32::from(peek_h(call, call.regs.gpr[4] + 2 * i)))
            .collect();
        icemt::make_blocks(r, gobj_arg(maps, call), &parts);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801F8CDC", 1, &as_saved, &blocks);
    let slot = |r: &mut Running, maps: &Maps, call: &Call| {
        let gobj = gobj_arg(maps, call);
        let at = call.regs.gpr[4] - vars(call);
        let s = usize::from(at == 0x4A);
        let g = call.regs.gpr.map(|v| v as i32);
        icemt::ice_slot(r, gobj, s, g[5], g[6], g[7], g[8], g[9], call.regs.gpr[10]);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801F91EC", 1, &as_saved, &slot);
    let index = |r: &mut Running, maps: &Maps, call: &Call| {
        let part = maps.jobjs.get(&call.regs.gpr[4]).copied();
        Ret::Int(icemt::part_index(r.game, gobj_arg(maps, call), part) as u32)
    };
    for p in [&as_saved as &Prepare, &|c: &mut Call, _: usize| {
        c.regs.gpr[4] = 0x8000_0000
    }] {
        check_calls_with(&d, "grIceMt_801FA500", 1, p, &index);
    }
    let scenery = |r: &mut Running, _: &Maps, _: &Call| {
        icemt::scenery_points(r.game);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801FA854", 1, &as_saved, &scenery);
}

/// The climb's process, its speed, the draws and the field changes, as saved and from states
/// set to each branch.
#[test]
fn icemt_climb() {
    let d = dir(IM);
    let prepares: [&Prepare; 9] = [
        &as_saved,
        // Held until the match starts.
        &|c, _| poke_h(c, vars(c) + 0x14, 1),
        // At rest with the wait over, changing with ticks left and on the last, a burst.
        &|c, _| im_state(c, vars(c), 0, -1, 0, 0, 0.5),
        &|c, _| im_state(c, vars(c), 1, 30, 5, 0, -0.25),
        &|c, _| im_state(c, vars(c), 1, 30, 1, 0, 0.75),
        &|c, _| im_state(c, vars(c), 0, 10, 0, 3, 0.0),
        // The upper field just past its place, either way, and near it.
        &|c, k| {
            let up = peek_h(c, vars(c) + 2) as u32;
            im_field_y(c, up, if k % 2 == 0 { 0.5 } else { 3.0 });
        },
        &|c, _| {
            let up = peek_h(c, vars(c) + 2) as u32;
            im_field_y(c, up, -900.0);
        },
        &|c, _| {
            let up = peek_h(c, vars(c) + 2) as u32;
            im_field_y(c, up, -8.0);
        },
    ];
    for p in prepares {
        check_calls_with(&d, "stageGObj9_GObjProc", 1, p, &by_address);
    }
    let speed = |r: &mut Running, maps: &Maps, call: &Call| {
        let (_, ret) = icemt::speed_step(r, rc_ground(maps, call.regs.gpr[3] - 4));
        Ret::Int(u32::from(ret))
    };
    let st: [&Prepare; 6] = [
        &as_saved,
        &|c, _| im_state(c, c.regs.gpr[3] - 4, 0, -1, 0, 0, 0.5),
        &|c, _| im_state(c, c.regs.gpr[3] - 4, 0, 0, 0, 0, 0.5),
        &|c, _| im_state(c, c.regs.gpr[3] - 4, 1, 3, 4, 0, -1.5),
        &|c, _| im_state(c, c.regs.gpr[3] - 4, 1, 3, 1, 2, -1.5),
        &|c, _| im_state(c, c.regs.gpr[3] - 4, 2, 3, 1, 1, 2.0),
    ];
    for p in st {
        check_calls_with(&d, "grIceMt_801FA364", 1, p, &speed);
    }
    // The next speed: as saved, and with rest counts left on some.
    let draw = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = rc_ground(maps, vars(call));
        Ret::Int(icemt::draw_speed(r, g).0 as u32)
    };
    let rests: [&Prepare; 3] = [
        &as_saved,
        &|c, _| {
            for i in 0..11 {
                poke_h(c, vars(c) + 0x18 + 2 * i, (i % 3) as i16);
            }
        },
        &|c, k| {
            for i in 0..12 {
                let v = if i == (k as u32 % 12) { 0 } else { 2 };
                poke_h(c, vars(c) + 0x18 + 2 * i, v);
            }
        },
    ];
    for p in rests {
        check_calls_with(&d, "fn_801F8E58", 1, p, &draw);
    }
    let next = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = rc_ground(maps, vars(call));
        Ret::Int(icemt::next_field(r, g) as u32)
    };
    // One field not shown left with no rest count, the others resting (the draw loops until
    // it finds one, as the original does).
    let rests: [&Prepare; 2] = [&as_saved, &|c, k| {
        let v = vars(c);
        let shown = [peek_h(c, v), peek_h(c, v + 2)];
        let free = (0..6u32)
            .map(|i| (i + k as u32) % 6)
            .find(|&i| !shown.contains(&(i as i16 + 1)))
            .unwrap();
        for i in 0..6 {
            poke_h(c, v + 0x30 + 2 * i, i16::from(i != free));
        }
    }];
    // The two fields shown apart, the upper one and one other free: only the other may come.
    let apart = |c: &mut Call, k: usize| {
        let v = vars(c);
        let (under, upper) = (1 + (k as u32 % 6), 1 + ((k as u32 + 2) % 6));
        poke_h(c, v, under as i16);
        poke_h(c, v + 2, upper as i16);
        let other = 1 + ((k as u32 + 4) % 6);
        for i in 0..6 {
            let id = i + 1;
            poke_h(c, v + 0x30 + 2 * i, i16::from(id != upper && id != other));
        }
    };
    let rests: [&Prepare; 3] = [rests[0], rests[1], &apart];
    for p in rests {
        check_calls_with(&d, "fn_801F9038", 1, p, &next);
    }
    // The field change, as saved and with the upper field moved to each side.
    let change = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(u32::from(icemt::change_field(
            r,
            rc_ground(maps, call.regs.gpr[3]),
        )))
    };
    let ys: [f32; 9] = [
        0.25, 9.0, 11.0, -9.0, -11.0, -200.0, -380.0, -420.0, -1000.0,
    ];
    check_calls_with(&d, "grIceMt_801F9ACC", 1, &as_saved, &change);
    for y in ys {
        let set = move |c: &mut Call, _: usize| {
            let up = peek_h(c, c.regs.gpr[3] + 2) as u32;
            im_field_y(c, up, y);
        };
        check_calls_with(&d, "grIceMt_801F9ACC", 1, &set, &change);
    }
    let set_none = |c: &mut Call, _: usize| poke_h(c, c.regs.gpr[3], -1);
    check_calls_with(&d, "grIceMt_801F9ACC", 1, &set_none, &change);
    let moved = |r: &mut Running, _: &Maps, call: &Call| {
        let ids = call.regs.gpr[3];
        let (under, upper) = (peek_h(call, ids), peek_h(call, ids + 2));
        icemt::move_fields(r, under, upper, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801F96E0", 1, &as_saved, &moved);
    let bg = |r: &mut Running, _: &Maps, call: &Call| {
        icemt::tell_bg(r.game, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801F9668", 1, &as_saved, &bg);
    let gap = |r: &mut Running, _: &Maps, call: &Call| {
        let (a, b) = (call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
        Ret::Float(f64::from(icemt::field_gap(r.game, r.data, a, b)))
    };
    check_calls_with(&d, "grIceMt_801F993C", 1, &as_saved, &gap);
    for (a, b) in [(1u32, 2u32), (2, 3), (3, 4), (4, 5), (5, 6), (6, 1), (2, 1)] {
        let set = move |c: &mut Call, _: usize| {
            c.regs.gpr[3] = a;
            c.regs.gpr[4] = b;
        };
        check_calls_with(&d, "grIceMt_801F993C", 1, &set, &gap);
    }
}

/// The fields' processes, their points of interest and falling ice, their destroy callbacks,
/// and the background's process.
#[test]
fn icemt_fields() {
    let d = dir(IM);
    for f in [
        "stageGObj1_GObjProc",
        "stageGObj2_GObjProc",
        "stageGObj3_GObjProc",
        "stageGObj4_GObjProc",
        "stageGObj5_GObjProc",
        "stageGObj6_GObjProc",
    ] {
        let due = |c: &mut Call, _: usize| {
            let v = vars(c);
            let w = peek_u32(c, v);
            poke_u32(c, v, w ^ 0x4000_0000);
        };
        for p in [&as_saved as &Prepare, &due] {
            check_calls_with(&d, f, 0, p, &by_address);
        }
    }
    // The falling ice for a tick: still, running, at the ticks its joint leaves and comes
    // back, and past them.
    let ice = |r: &mut Running, maps: &Maps, call: &Call| {
        let gobj = gobj_arg(maps, call);
        let s = usize::from(call.regs.gpr[4] - vars(call) == 0x4A);
        icemt::run_ice(r, gobj, s);
        Ret::Nothing
    };
    check_calls_with(&d, "grIceMt_801F929C", 1, &as_saved, &ice);
    for (on, tick) in [(1i16, 0i16), (1, 264), (1, 637), (1, 700), (0, 264)] {
        let set = move |c: &mut Call, _: usize| {
            let s = c.regs.gpr[4];
            poke_h(c, s, on);
            poke_h(c, s + 2, tick);
        };
        check_calls_with(&d, "grIceMt_801F929C", 1, &set, &ice);
    }
    let points = |r: &mut Running, maps: &Maps, call: &Call| {
        icemt::update_points(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    let due = |c: &mut Call, _: usize| {
        let v = vars(c);
        let w = peek_u32(c, v);
        poke_u32(c, v, w | 0x4000_0000);
    };
    for p in [&as_saved as &Prepare, &due] {
        check_calls_with(&d, "grIceMt_801F98A8", 1, p, &points);
    }
    for f in [
        "stageGObj1_Callback3",
        "stageGObj2_Callback3",
        "stageGObj3_Callback3",
        "stageGObj4_Callback3",
        "stageGObj5_Callback3",
        "stageGObj6_Callback3",
    ] {
        check_calls_with(&d, f, 0, &as_saved, &by_address);
    }
    // The background: as saved, at rest, and scrolled past either end of its wrap.
    let prepares: [&Prepare; 4] = [
        &as_saved,
        &|c, _| poke_u32(c, vars(c), 0f32.to_bits()),
        &|c, _| {
            poke_u32(c, vars(c), (-30.0f32).to_bits());
            let j = nth_jobj(c, c.regs.gpr[3], 8);
            poke_u32(c, j + 0x3C, 100_000.0f32.to_bits());
        },
        &|c, _| {
            poke_u32(c, vars(c), 30.0f32.to_bits());
            let j = nth_jobj(c, c.regs.gpr[3], 8);
            poke_u32(c, j + 0x3C, (-100_000.0f32).to_bits());
        },
    ];
    for p in prepares {
        check_calls_with(&d, "stageGObj8_GObjProc", 1, p, &by_address);
    }
}

/// The falling ice landed on and the blocks hit: as saved, by each collision kind, with the ice
/// running, and the adventure's touch.
#[test]
fn icemt_touches() {
    let d = dir(IM);
    let touched = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = rc_ground(maps, call.regs.gpr[3] + 0xC4);
        let cb = call.addr;
        let map = if cb == icemt::ICE_C_TOUCHED { 4 } else { 2 };
        let root = melee_sim::game::gr::ground::map_gobj(r.game, map)
            .and_then(|go| melee_sim::game::gr::ground::jobj_of(r.game, go));
        let cd = coll_arg(r, maps, call);
        let (objs, grounds) = (&mut r.game.objs, &mut r.game.grounds);
        icemt::ice_touched(objs, grounds, root, g, cb, &cd, None);
        Ret::Nothing
    };
    for f in ["fn_801F9338", "fn_801F9448", "fn_801F9558"] {
        let prepares: [&Prepare; 4] = [
            &as_saved,
            &|c, k| rc_kind(c, (k % 4) as u32),
            &|c, _| rc_kind(c, 1),
            &|c, _| {
                rc_kind(c, 1);
                let off = match c.addr {
                    0x801F_9448 => 0x4A,
                    0x801F_9558 => 0x44,
                    _ => 0x3C,
                };
                let s = c.regs.gpr[3] + 0xC4 + off;
                poke_h(c, s, 1);
            },
        ];
        for p in prepares {
            check_calls_with(&d, f, 0, p, &touched);
        }
    }
    let adventure = |_: &mut Running, _: &Maps, _: &Call| {
        icemt::adventure_touch(None);
        Ret::Nothing
    };
    check_calls_with(&d, "onJointCollision", 1, &as_saved, &adventure);
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let item = gobj_arg(maps, call);
        let g = *maps
            .grounds
            .get(&call.regs.gpr[4])
            .expect("the block's field");
        icemt::block_hit(r, item, g);
        Ret::Nothing
    };
    check_calls_with(&d, "fn_801F8C64", 0, &as_saved, &hit);
}

/// The callbacks no capture reaches, from the background's captures: the empty processes and
/// destroy callbacks, the shadow check, the touch-line callback, the demo hook and the wind.
#[test]
fn icemt_unreached_callbacks() {
    let d = dir(IM);
    let from = "stageGObj8_GObjProc";
    for addr in [0x801F_75E4u32, 0x801F_75E8, 0x801F_77AC, 0x801F_8C60] {
        check_calls_with(&d, from, 1, &retarget(addr), &by_address);
    }
    let shadow =
        |_: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(icemt::check_shadow_render()));
    check_calls_with(&d, from, 1, &retarget(0x801F_A900), &shadow);
    check_calls_with(&d, from, 1, &retarget(0x801F_6868), &demo);
    // The wind, into the climb's first six rest counts (from its process's calls, r3 its
    // object).
    let into = move |c: &mut Call, _: usize| {
        let to = vars(c) + 0x18;
        c.addr = 0x801F_A728;
        c.regs.gpr[3] = to;
    };
    let wind = |r: &mut Running, maps: &Maps, call: &Call| {
        let w = icemt::wind(r.game);
        let g = rc_ground(maps, call.regs.gpr[3] - 0x18);
        let v = r.game.grounds[g.0].u.icemt_scroll();
        for (k, c) in [w.x, w.y, w.z].into_iter().enumerate() {
            let b = c.to_bits();
            v.x18[2 * k] = (b >> 16) as u16 as i16;
            v.x18[2 * k + 1] = b as u16 as i16;
        }
        Ret::Nothing
    };
    check_calls_with(&d, "stageGObj9_GObjProc", 1, &into, &wind);
}

// Corneria (`grcorneria.c`), its Arwings' lasers (`itarwinglaser.c`) and the Great Fox's
// (`itgreatfoxlaser.c`): the stage's oracle match, and the taunt set's Corneria matches for the
// Star Fox team's talk (`oracle-taunt-fox-corneria`, which plays a talk through, and
// `oracle-taunt-falco-corneria`, whose match ends during one), captured as the stage is with
// `cn-taunt-funcs.txt` (the stage's functions and Fox's and Falco's side taunt's) and
// `done-cn-taunt.txt` (`capture-cn-taunt.sh`).

const CN: &str = "oracle-stage-corneria";
const CN_TALK: &str = "oracle-taunt-fox-corneria";
const CN_TALK_END: &str = "oracle-taunt-falco-corneria";

/// grcorneria.c's `yakumono_param`.
const CN_PARAM: u32 = 0x804D_69A0;
/// `arwing_gobjs`, `arwing_types` and `arwing_groups`.
const CN_ARWINGS: u32 = 0x803E_1D68;
const CN_TYPES: u32 = 0x803E_1D74;
const CN_GROUPS: u32 = 0x803E_1D80;
/// The wait before the next Arwing, whether the Arwings are held and whether the demo plays
/// (`grCn_804D69A8` to `grCn_804D69B0`).
const CN_WAIT: u32 = 0x804D_69A8;
const CN_FROZEN: u32 = 0x804D_69AC;
const CN_DEMO: u32 = 0x804D_69B0;

/// Corneria's parameters' word at `off`.
fn cn_param(c: &Call, off: u32) -> u32 {
    peek_u32(c, peek_u32(c, CN_PARAM) + off)
}

/// A stage object's process or callback taking its object, by name.
fn of_ground(
    f: fn(&mut Running, melee_sim::hsd::gobj::GObjId),
) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, maps: &Maps, call: &Call| {
        f(r, gobj_arg(maps, call));
        Ret::Nothing
    }
}

/// A hook taking nothing.
fn of_running(f: fn(&mut Running)) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, _: &Maps, _: &Call| {
        f(r);
        Ret::Nothing
    }
}

/// The vector a call's register `reg` points to.
fn vec_arg(call: &Call, reg: usize) -> Vec3 {
    vec_at(call, call.regs.gpr[reg])
}

fn seeded(k: usize, mul: u32) -> impl Fn(&mut Call, usize) + Sync {
    move |c: &mut Call, i: usize| set_seed(c, mul.wrapping_mul((k + i) as u32 + 1))
}

#[test]
fn corneria_setup() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    for f in [
        "grCorneria_801DD350",
        "grCorneria_801DD478",
        "grCorneria_801DD508",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &event);
    }
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = corneria::create(
            r,
            call.regs.gpr[3] as i32,
            melee_sim::game::gr::setup::SETUP_FRAME,
        );
        Ret::Nothing
    };
    check_calls_with(&d, "grCorneria_801DD534", 1, &as_saved, &create);
    for f in [
        "grCorneria_801DD620",
        "grCorneria_801DD65C",
        "grCorneria_801DD674",
        "grCorneria_801DDAC4",
        "grCorneria_Arwing_801DE4C0",
        "grCorneria_801DE8E4",
        "grCorneria_801DF8D0",
        "grCorneria_801DFC2C",
        "grCorneria_801DFEB8",
        "grCorneria_801E0140",
        "grCorneria_801E0C3C",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    // The first Arwing's wait, from several seeds, in a match and as the demo's stages.
    let reset = of_running(corneria::reset_arwings);
    check_calls_with(&d, "grCorneria_801DCCFC", 1, &as_saved, &reset);
    check_calls_with(
        &d,
        "grCorneria_801DCCFC",
        1,
        &seeded(0, 0x2468_ACE1),
        &reset,
    );
    for kind in [0xE9u16, 0x47, 0x46] {
        let demo = move |c: &mut Call, _: usize| {
            poke_u32(c, leaf_check::sym("selected_stage"), u32::from(kind));
        };
        check_calls_with(&d, "grCorneria_801DCCFC", 1, &demo, &reset);
    }
}

/// The Great Fox's variables' guns fields set: state, wait, shot wait, shots, kind and side.
fn cn_guns(c: &mut Call, state: u32, wait: i32, shot_wait: i32, shots: u8, kind: u8, side: u8) {
    let v = vars(c);
    poke_u32(c, v + 0x44, state);
    poke_u32(c, v + 0x48, wait as u32);
    poke_u32(c, v + 0x4C, shot_wait as u32);
    poke_u8(c, v + 0x54, shots);
    poke_u8(c, v + 0x55, kind);
    poke_u8(c, v + 0x56, side);
}

#[test]
fn corneria_ship() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    // The Great Fox's process, as saved and waiting for the match's start.
    check_calls_with(&d, "grCorneria_801DD9A8", 1, &as_saved, &by_address);
    let waiting = |c: &mut Call, _: usize| {
        let v = vars(c);
        poke_u8(c, v, 0x80);
    };
    check_calls_with(&d, "grCorneria_801DD9A8", 1, &waiting, &by_address);
    let move_copy = of_ground(corneria::move_copy);
    for set in [CN, CN_TALK] {
        check_calls_with(&dir(set), "grCorneria_801DD9A8", 1, &as_saved, &by_address);
        check_calls_with(&dir(set), "grCorneria_801E1878", 1, &as_saved, &move_copy);
    }
    check_calls_with(
        &d,
        "grCorneria_801E2110",
        1,
        &as_saved,
        &of_running(corneria::city_colors),
    );
    check_calls_with(
        &d,
        "grCorneria_801E0E40",
        1,
        &as_saved,
        &of_running(corneria::debug_talk),
    );
    // The drift: turning (its wait run out, from several seeds) or not, its speeds past their
    // caps either way, and past the box's edges either way.
    let drift = of_ground(corneria::drift);
    check_calls_with(&d, "grCorneria_801E1970", 1, &as_saved, &drift);
    for (turn, vel, off) in [
        (0i32, 0.0f32, 0.0f32),
        (-1, 0.0, 0.0),
        (1, 100.0, 0.0),
        (1, -100.0, 0.0),
        (1, 0.0, 1000.0),
        (1, 0.0, -1000.0),
        (0, 100.0, -1000.0),
    ] {
        let p = move |c: &mut Call, k: usize| {
            let v = vars(c);
            poke_u32(c, v + 0x40, turn as u32);
            for at in [0x28, 0x2C] {
                poke_u32(c, v + at, vel.to_bits());
            }
            for at in [0x18, 0x1C] {
                poke_u32(c, v + at, off.to_bits());
            }
            set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
        };
        check_calls_with(&d, "grCorneria_801E1970", 1, &p, &drift);
    }
    // The sinking: not sinking over each part, with its chance had or not, from several seeds;
    // sinking down (fast, slow, near its limit) and back up, at the turn's tick and either side.
    let sink = of_ground(corneria::sink);
    check_calls_with(&d, "grCorneria_801E2228", 1, &as_saved, &sink);
    for drawn in [false, true] {
        let p = move |c: &mut Call, k: usize| {
            let v = vars(c);
            poke_u8(c, v, if drawn { 0x20 } else { 0 });
            set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
        };
        check_calls_with(&d, "grCorneria_801E2228", 1, &p, &sink);
    }
    for t in [0i32, 0x383, 0x384, 0x385] {
        for (depth, speed) in [
            (0.0f32, 0.0f32),
            (-10.0, 3.0),
            (-120.0, 0.5),
            (-199.0, 2.0),
            (-400.0, 0.005),
            (-1.0, 1.0),
        ] {
            let p = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u8(c, v, 0x40);
                poke_u32(c, v + 0xC, depth.to_bits());
                poke_u32(c, v + 0x38, speed.to_bits());
                poke_u32(c, v + 0x3C, t as u32);
            };
            check_calls_with(&d, "grCorneria_801E2228", 1, &p, &sink);
        }
    }
}

#[test]
fn corneria_guns() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    let guns = of_ground(corneria::guns);
    check_calls_with(&d, "grCorneria_801E1348", 1, &as_saved, &guns);
    // Each state at its waits' ends and short of them: resting into a big shot or a volley
    // (several seeds), charging, aiming, firing from each side and kind and resting after,
    // broken, and done.
    type Case = (u32, i32, i32, u8, u8, u8);
    let mut cases: Vec<Case> = vec![(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 0, 0)];
    for wait in [0, 1] {
        cases.push((1, wait, 0, 0, 0, 0));
        cases.push((2, wait, 0, 0, 0, 0));
    }
    for (shots, shot_wait) in [(0u8, 0i32), (3, 1), (3, 0), (1, 0)] {
        for (kind, side) in [(0u8, 0u8), (1, 1), (1, 0), (0, 1)] {
            cases.push((3, 0, shot_wait, shots, kind, side));
        }
    }
    cases.push((4, 0, 0, 0, 0, 0));
    cases.push((5, 0, 0, 0, 0, 0));
    for (state, wait, shot_wait, shots, kind, side) in cases {
        let p = move |c: &mut Call, k: usize| {
            cn_guns(c, state, wait, shot_wait, shots, kind, side);
            set_seed(c, 0x0F0F_1357u32.wrapping_mul(k as u32 + 1));
        };
        check_calls_with(&d, "grCorneria_801E1348", 1, &p, &guns);
    }
}

/// The guns' hurtbox items' hit callback, from the guns' calls: a hit adding up short of the
/// guns' damage, exactly to it and past it.
#[test]
fn corneria_gun_hit() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let gid = *maps.grounds.get(&call.regs.gpr[4]).expect("the Great Fox");
        corneria::gun_hit(r, gid, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    let at = leaf_check::sym("fn_801E12D4");
    for (taken, damage) in [(0.0f32, 1.0f64), (10.0, 0.0), (0.0, 1000.0), (5.0, -1.0)] {
        let p = move |c: &mut Call, _: usize| {
            let gp = peek_u32(c, c.regs.gpr[3] + 0x2C);
            let v = gp + 0xC4;
            let hp = f32::from_bits(cn_param(c, 0x38));
            poke_u32(c, v + 0x50, taken.to_bits());
            c.addr = at;
            c.regs.gpr[3] = peek_u32(c, v + 0x5C);
            c.regs.gpr[4] = gp;
            // -1: exactly what the guns can take.
            c.regs.fpr[1].ps0 = if damage < 0.0 {
                f64::from(hp - taken)
            } else {
                damage
            };
        };
        check_hazard(&d, "grCorneria_801E1348", 1, &p, &hit);
    }
    // And the hit they deal, which does nothing.
    let dealt = |r: &mut Running, maps: &Maps, call: &Call| {
        let gid = *maps.grounds.get(&call.regs.gpr[4]).expect("the Great Fox");
        corneria::gun_dealt(r, gobj_arg(maps, call), gid);
        Ret::Nothing
    };
    let at = leaf_check::sym("fn_801E12D0");
    let p = move |c: &mut Call, _: usize| {
        let gp = peek_u32(c, c.regs.gpr[3] + 0x2C);
        c.addr = at;
        c.regs.gpr[3] = peek_u32(c, gp + 0xC4 + 0x5C);
        c.regs.gpr[4] = gp;
    };
    check_calls_with(&d, "grCorneria_801E1348", 1, &p, &dealt);
}

/// An Arwing slot's path, type and group set.
fn cn_slot(c: &mut Call, slot: u32, path: u32, ty: i32, group: i32) {
    poke_u32(c, CN_ARWINGS + 4 * slot, path);
    poke_u32(c, CN_TYPES + 4 * slot, ty as u32);
    poke_u32(c, CN_GROUPS + 4 * slot, group as u32);
}

#[test]
fn corneria_spawns() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    let spawn = of_running(corneria::spawn_arwings);
    check_calls_with(&d, "grCorneria_801DCE1C", 1, &as_saved, &spawn);
    // In a match: the wait running out after each type and group (near and far, several
    // seeds), not yet, and an Arwing out (a new wait).
    for wait in [1i32, 2, 0] {
        for (ty, group) in [(0i32, 0i32), (3, 1), (9, 1), (12, 4), (5, 4)] {
            let p = move |c: &mut Call, k: usize| {
                poke_u32(c, CN_DEMO, 0);
                poke_u32(c, CN_WAIT, wait as u32);
                cn_slot(c, 0, 0, ty, group);
                set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
            };
            check_calls_with(&d, "grCorneria_801DCE1C", 1, &p, &spawn);
        }
    }
    let out = |c: &mut Call, k: usize| {
        poke_u32(c, CN_DEMO, 0);
        let f = first_fighter(c);
        cn_slot(c, 0, f, 3, 1);
        set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
    };
    check_calls_with(&d, "grCorneria_801DCE1C", 1, &out, &spawn);
    // The demo: all three out, one free after near and far ones, the wait running out or not.
    for (free, wait) in [(3u32, 1i32), (0, 1), (0, 2), (1, 1), (2, 1), (2, 5)] {
        for near in [false, true] {
            let p = move |c: &mut Call, k: usize| {
                poke_u32(c, CN_DEMO, 1);
                poke_u32(c, CN_WAIT, wait as u32);
                let f = first_fighter(c);
                for slot in 0..3u32 {
                    let (path, ty) = if slot == free {
                        (0, 0)
                    } else if near {
                        (f, slot as i32 + 2)
                    } else {
                        (f, slot as i32 + 10)
                    };
                    cn_slot(c, slot, path, ty, slot as i32 + 1);
                }
                set_seed(c, 0x0F0F_1357u32.wrapping_mul(k as u32 + 1));
            };
            check_calls_with(&d, "grCorneria_801DCE1C", 1, &p, &spawn);
        }
    }
    // Held still on the stage's other kind (0x46).
    let other = |c: &mut Call, _: usize| poke_u32(c, leaf_check::sym("selected_stage"), 0x46);
    check_calls_with(&d, "grCorneria_801DCE1C", 1, &other, &spawn);
}

/// The Arwing slot a flight path or an Arwing in r3 holds.
fn cn_slot_of(c: &Call) -> u32 {
    peek_u32(c, vars(c) + 4)
}

#[test]
fn corneria_paths() {
    use melee_sim::game::gr::corneria;
    for set in [CN, CN_TALK_END] {
        let d = dir(set);
        check_calls_with(&d, "grCorneria_801DE024", 1, &as_saved, &by_address);
        // Its first tick, each sound state, each type, and its slot taken by another.
        let first = |c: &mut Call, _: usize| poke_u32(c, vars(c) + 0x10, 1);
        check_calls_with(&d, "grCorneria_801DE024", 1, &first, &by_address);
        for sound in 0..4u32 {
            for ty in [2i32, 7, 10, 13] {
                let p = move |c: &mut Call, _: usize| {
                    let v = vars(c);
                    poke_u32(c, v + 0x10, 0);
                    poke_u32(c, v + 0x14, sound);
                    let slot = cn_slot_of(c);
                    poke_u32(c, CN_TYPES + 4 * slot, ty as u32);
                };
                check_calls_with(&d, "grCorneria_801DE024", 1, &p, &by_address);
            }
        }
        let taken = |c: &mut Call, _: usize| {
            let slot = cn_slot_of(c);
            poke_u32(c, CN_ARWINGS + 4 * slot, 0);
        };
        check_calls_with(&d, "grCorneria_801DE024", 1, &taken, &by_address);
        let near = of_ground(corneria::path_sound_near);
        for sound in [None, Some(0u32), Some(1), Some(2), Some(3)] {
            let p = move |c: &mut Call, _: usize| {
                if let Some(s) = sound {
                    poke_u32(c, vars(c) + 0x14, s);
                }
            };
            check_calls_with(&d, "grCorneria_801DDE88", 1, &p, &near);
        }
        let pos = |r: &mut Running, maps: &Maps, call: &Call| {
            let _ = corneria::path_pos(r, maps.gobjs.get(&call.regs.gpr[3]).copied());
            Ret::Nothing
        };
        check_calls_with(&d, "grCorneria_801DDDA8", 1, &as_saved, &pos);
        let step = |r: &mut Running, _: &Maps, _: &Call| {
            let _ = corneria::ship_step(r.game);
            Ret::Nothing
        };
        check_calls_with(&dir(CN_TALK), "grCorneria_801DDCF0", 1, &as_saved, &step);
        let at = |r: &mut Running, _: &Maps, _: &Call| {
            let _ = corneria::ship_pos(r.game);
            Ret::Nothing
        };
        check_calls_with(&d, "grCorneria_801DDD4C", 1, &as_saved, &at);
    }
}

#[test]
fn corneria_arwings() {
    let d = dir(CN);
    for f in ["grCorneria_801DED50", "grCorneria_801DFBCC"] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
        // Held, its path gone, each roll state (rolling or not, from several seeds), firing
        // (every tick of the period, from several seeds).
        let held = |c: &mut Call, _: usize| poke_u32(c, CN_FROZEN, 1);
        check_calls_with(&d, f, 1, &held, &by_address);
        let gone = |c: &mut Call, _: usize| {
            let slot = cn_slot_of(c);
            poke_u32(c, CN_ARWINGS + 4 * slot, 0);
        };
        check_calls_with(&d, f, 1, &gone, &by_address);
        for (roll, wait) in [(0u32, 0i32), (0, 1), (1, 0), (3, 0), (4, 0), (5, 0)] {
            let p = move |c: &mut Call, k: usize| {
                let v = vars(c);
                poke_u32(c, v + 0x30, roll);
                poke_u32(c, v + 0x34, wait as u32);
                set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
            };
            check_calls_with(&d, f, 1, &p, &by_address);
        }
        let fire = |c: &mut Call, k: usize| {
            let period = cn_param(c, 0x74);
            poke_u32(c, vars(c) + 0x2C, period.wrapping_mul(3));
            set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
        };
        check_calls_with(&d, f, 1, &fire, &by_address);
        let demo = |c: &mut Call, k: usize| {
            poke_u32(c, CN_DEMO, 1);
            let period = cn_param(c, 0x7C);
            poke_u32(c, vars(c) + 0x2C, period.wrapping_mul(2));
            set_seed(c, 0x0F0F_1357u32.wrapping_mul(k as u32 + 1));
        };
        check_calls_with(&d, f, 1, &demo, &by_address);
    }
    // An Arwing's collision: as saved, its path gone, rolled and off the plane.
    let f = "grCorneria_801DE568";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    let gone = |c: &mut Call, _: usize| {
        let slot = cn_slot_of(c);
        poke_u32(c, CN_ARWINGS + 4 * slot, 0);
    };
    check_calls_with(&d, f, 1, &gone, &by_address);
    for (roll, z) in [
        (0.0f32, 0.0f32),
        (1.0, 0.0),
        (1.1, 0.0),
        (1.06, 0.0),
        (-1.06, 0.0),
        (-4.0, 0.0),
        (7.0, 9.9),
        (0.5, 10.0),
        (0.0, -12.0),
    ] {
        let p = move |c: &mut Call, _: usize| {
            let v = vars(c);
            poke_u32(c, v + 0x18, roll.to_bits());
            poke_u32(c, v + 0x1C + 8, z.to_bits());
        };
        check_calls_with(&d, f, 1, &p, &by_address);
    }
    // The blast zones' checks, as saved and moved about the zones and the plane.
    let outside = |r: &mut Running, _: &Maps, call: &Call| {
        let pos = vec_arg(call, 3);
        Ret::Int(u32::from(
            melee_sim::game::gr::corneria::outside_blast_zones(r.game, pos),
        ))
    };
    let near = |r: &mut Running, _: &Maps, call: &Call| {
        let pos = vec_arg(call, 3);
        Ret::Int(melee_sim::game::gr::corneria::near_blast_zones(r.game, pos) as u32)
    };
    for p in [
        (0.0f32, 50.0f32, 0.0f32),
        (1e4, 0.0, 0.0),
        (-1e4, 0.0, 0.0),
        (0.0, 1e4, 0.0),
        (0.0, -1e4, 0.0),
        (0.0, 50.0, -40.0),
        (1e4, 0.0, -40.0),
    ] {
        let put = move |c: &mut Call, _: usize| {
            let a = c.regs.gpr[3];
            poke_u32(c, a, p.0.to_bits());
            poke_u32(c, a + 4, p.1.to_bits());
            poke_u32(c, a + 8, p.2.to_bits());
        };
        for set in [CN, CN_TALK_END] {
            check_calls_with(&dir(set), "grCorneria_801DEC08", 1, &put, &outside);
            check_calls_with(&dir(set), "grCorneria_801DEC94", 1, &put, &near);
        }
    }
    for set in [CN, CN_TALK_END] {
        check_calls_with(&dir(set), "grCorneria_801DEC08", 1, &as_saved, &outside);
        check_calls_with(&dir(set), "grCorneria_801DEC94", 1, &as_saved, &near);
    }
}

#[test]
fn corneria_city() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    // Each part's process: as saved, waiting, still, and each state far behind and past its
    // edges, and stepping to just short of, onto and just past them.
    for (f, still_at, still_bit, width) in [
        ("grCorneria_801E01A8", 2u32, 0x80u8, 4800.0f32),
        ("grCorneria_801DFF20", 2, 0x80, 3200.0),
        ("grCorneria_801DFC98", 0x1C, 0x80, 3200.0),
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
        let waiting = |c: &mut Call, _: usize| poke_u8(c, vars(c), 1);
        check_calls_with(&d, f, 1, &waiting, &by_address);
        let still = move |c: &mut Call, _: usize| {
            let a = vars(c) + still_at;
            let w = peek_u32(c, a & !3);
            let shift = 24 - 8 * (a & 3);
            poke_u32(c, a & !3, w | u32::from(still_bit) << shift);
        };
        check_calls_with(&d, f, 1, &still, &by_address);
        for state in [0u8, 1, 2] {
            for x in [-1e5f32, 1e5] {
                let p = move |c: &mut Call, _: usize| {
                    poke_u8(c, vars(c) + 1, state);
                    let j = peek_u32(c, c.regs.gpr[3] + 0x28);
                    poke_u32(c, j + 0x38, x.to_bits());
                };
                check_calls_with(&d, f, 1, &p, &by_address);
            }
        }
        for (state, add) in [(0u8, -1400.0f32), (1, 1400.0)] {
            for d_x in [-0.5f32, 0.0, 0.5] {
                let p = move |c: &mut Call, _: usize| {
                    poke_u8(c, vars(c) + 1, state);
                    let scale = f32_at(c, peek_u32(c, STAGE_INFO + 0x6B0));
                    let speed = f32::from_bits(cn_param(c, 0x88));
                    let x = width * scale * 0.5 + add - speed + d_x;
                    let j = peek_u32(c, c.regs.gpr[3] + 0x28);
                    poke_u32(c, j + 0x38, x.to_bits());
                };
                check_calls_with(&d, f, 1, &p, &by_address);
            }
        }
    }
    let next = |r: &mut Running, maps: &Maps, call: &Call| {
        corneria::next_city_part(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    check_calls_with(&d, "grCorneria_801E03C8", 1, &as_saved, &next);
    let under =
        |r: &mut Running, _: &Maps, _: &Call| Ret::Int(corneria::city_under_camera(r.game) as u32);
    check_calls_with(&d, "grCorneria_801E08CC", 1, &as_saved, &under);
    let at = |r: &mut Running, _: &Maps, call: &Call| {
        let x = f32_at(call, call.regs.gpr[3]);
        Ret::Int(corneria::city_at(r.game, x) as u32)
    };
    check_calls_with(&d, "grCorneria_801E0A74", 1, &as_saved, &at);
    check_calls_with(&d, "grCorneria_801E0D30", 1, &as_saved, &by_address);
}

#[test]
fn corneria_hooks() {
    use melee_sim::game::gr::corneria;
    for set in [CN, CN_TALK, CN_TALK_END] {
        let d = dir(set);
        check_calls_with(
            &d,
            "grCorneria_801E2A6C",
            1,
            &as_saved,
            &of_running(corneria::match_end),
        );
        let depth = |r: &mut Running, _: &Maps, _: &Call| {
            Ret::Float(f64::from(corneria::sink_depth(r.game)))
        };
        check_calls_with(&d, "grCorneria_801E2EA0", 1, &as_saved, &depth);
        let hazard = |_: &mut Running, _: &Maps, _: &Call| {
            assert!(corneria::line_hazard().is_none());
            Ret::Int(0)
        };
        check_calls_with(&d, "grCorneria_801E2EE4", 1, &as_saved, &hazard);
        let floor =
            |r: &mut Running, _: &Maps, _: &Call| Ret::Float(corneria::camera_floor(r.game));
        check_calls_with(&d, "grCorneria_801E2FCC", 1, &as_saved, &floor);
        fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
            let (pos, j) = shadow_args(maps, call);
            corneria::check_shadow_render(r.game, r.data, pos, j)
        }
        check_shadow(&d, "grCorneria_801E2EEC", ours, 0.0);
    }
    let taunt_stage =
        |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(r.corneria_taunt_stage()));
    check_calls_with(
        &dir(CN_TALK),
        "grCorneria_801E2CE8",
        1,
        &as_saved,
        &taunt_stage,
    );
}

/// The talk object's variables set: talk, next line and wait.
fn cn_talk(c: &mut Call, talk: u32, line: i32, wait: i32) {
    let v = vars(c);
    poke_u32(c, v, talk);
    poke_u32(c, v + 4, line as u32);
    poke_u32(c, v + 8, wait as u32);
}

#[test]
fn corneria_talk() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN_TALK);
    let set = |r: &mut Running, maps: &Maps, call: &Call| {
        corneria::set_talk(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    for val in [
        None,
        Some(-1i32),
        Some(0),
        Some(5),
        Some(19),
        Some(20),
        Some(100),
    ] {
        let p = move |c: &mut Call, _: usize| {
            if let Some(v) = val {
                c.regs.gpr[4] = v as u32;
            }
        };
        check_calls_with(&d, "grCorneria_801E0F34", 1, &p, &set);
    }
    // The talk's process: as saved, and each talk at its lines' ends and past them, with the
    // wait running out or not.
    check_calls_with(&d, "grCorneria_801E0F6C", 1, &as_saved, &by_address);
    for talk in [0u32, 8, 12, 13, 19] {
        for line in [0i32, 1, 2, 3, 4] {
            for wait in [-1i32, 0, 4] {
                let p = move |c: &mut Call, _: usize| cn_talk(c, talk, line, wait);
                check_calls_with(&d, "grCorneria_801E0F6C", 1, &p, &by_address);
            }
        }
    }
    for f in [
        "grCorneria_801E1030",
        "grCorneria_801E0DE4",
        "grCorneria_801E0E14",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    let has = |_: &mut Running, _: &Maps, call: &Call| {
        Ret::Int(u32::from(corneria::talk_has_line(
            call.regs.gpr[3],
            call.regs.gpr[4],
        )))
    };
    for talk in [0u32, 7, 19] {
        for line in [0u32, 2, 4] {
            let p = move |c: &mut Call, _: usize| {
                c.regs.gpr[3] = talk;
                c.regs.gpr[4] = line;
            };
            check_calls_with(&d, "grCorneria_801E2598", 1, &p, &has);
        }
    }
    let start = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        corneria::start_line(
            r,
            gobj_arg(maps, call),
            g[5] as i32,
            g[6] as i32,
            g[7] as i32,
        );
        Ret::Nothing
    };
    check_calls_with(&d, "grCorneria_801E25C4", 1, &as_saved, &start);
    for speaker in 0..5u32 {
        let p = move |c: &mut Call, _: usize| c.regs.gpr[5] = speaker;
        check_calls_with(&d, "grCorneria_801E25C4", 1, &p, &start);
    }
    let start_talk = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        corneria::start_talk_line(r, gobj_arg(maps, call), g[5], g[6]);
        Ret::Nothing
    };
    check_calls_with(&d, "grCorneria_801E2738", 1, &as_saved, &start_talk);
    let setup = |r: &mut Running, maps: &Maps, call: &Call| {
        corneria::window_setup(r, gobj_arg(maps, call));
        Ret::Nothing
    };
    check_calls_with(&d, "smashTaunt_801E2550", 1, &as_saved, &setup);
    // A line's process: as saved, and each state with its timer at its edges.
    let line = of_ground(corneria::proc_line);
    check_calls_with(&d, "grCorneria_801E277C", 1, &as_saved, &line);
    for state in [0i16, 1, 2, 3] {
        for timer in [-1i16, 0, 1] {
            let p = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u16(c, v, state as u16);
                poke_u16(c, v + 2, timer as u16);
            };
            check_calls_with(&d, "grCorneria_801E277C", 1, &p, &line);
        }
    }
    let talk = |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(corneria::fox_talk(r)));
    check_calls_with(&d, "grCorneria_801E2B80", 1, &as_saved, &talk);
    check_calls_with(&d, "grCorneria_801E2B80", 1, &seeded(0, 0x2468_ACE1), &talk);
    let falco = leaf_check::sym("grCorneria_801E2C34");
    let as_falco = move |c: &mut Call, k: usize| {
        c.addr = falco;
        set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
    };
    let falco_talk =
        |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(corneria::falco_talk(r)));
    check_calls_with(&d, "grCorneria_801E2B80", 1, &as_falco, &falco_talk);
    let interrupted = of_running(corneria::taunt_interrupted);
    check_calls_with(&d, "grCorneria_801E2AF4", 1, &as_saved, &interrupted);
    let talking =
        |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(corneria::talking(r.game)));
    let t = leaf_check::sym("grCorneria_801E2D14");
    check_calls_with(&d, "grCorneria_801E0F6C", 1, &retarget(t), &talking);
    check_calls_with(&dir(CN), "grCorneria_801DD9A8", 1, &retarget(t), &talking);
    // The window's render callback, in the pass saved and another.
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        corneria::window_render(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let gx = leaf_check::calls::Setup {
        gx: true,
        ..Default::default()
    };
    // Paused for debugging (the window and its text hidden).
    let paused = |c: &mut Call, _: usize| poke_u8(c, leaf_check::sym("gm_80479D58") + 0x10, 2);
    for p in [
        &as_saved as &Prepare,
        &|c: &mut Call, _: usize| c.regs.gpr[4] ^= 3,
        &paused,
    ] {
        leaf_check::calls::check_calls_in(&d, "smashTaunt_801E24AC", 1, p, &gx, &render);
    }
    // The window's camera's render callback, on the camera of the window the saved calls draw.
    let cam_render = |r: &mut Running, maps: &Maps, call: &Call| {
        r.render_stage_camera(gobj_arg(maps, call));
        Ret::Nothing
    };
    let at = leaf_check::sym("grDisplay_801C5F60");
    let on_camera = move |c: &mut Call, _: usize| {
        c.addr = at;
        let gp = peek_u32(c, c.regs.gpr[3] + 0x2C);
        c.regs.gpr[3] = peek_u32(c, gp + 0x18);
    };
    leaf_check::calls::check_calls_in(&d, "smashTaunt_801E24AC", 1, &on_camera, &gx, &cam_render);
    // The text system's font closing.
    let free = |r: &mut Running, _: &Maps, call: &Call| {
        r.free_font_texts(call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    check_calls_with(&d, "HSD_SisLib_803A5DA0", 1, &as_saved, &free);
    let close = |r: &mut Running, _: &Maps, call: &Call| {
        r.close_font(call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    check_calls_with(&d, "HSD_SisLib_803A5F50", 1, &as_saved, &close);
}

/// The talk at the match's end, from the Falco match: the match's end stopping it.
#[test]
fn corneria_talk_end() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN_TALK_END);
    check_calls_with(
        &d,
        "grCorneria_801E2A6C",
        1,
        &as_saved,
        &of_running(corneria::match_end),
    );
    let line = of_ground(corneria::proc_line);
    check_calls_with(&d, "grCorneria_801E277C", 1, &as_saved, &line);
    check_calls_with(&d, "grCorneria_801E0F6C", 1, &as_saved, &by_address);
    let talk = |r: &mut Running, _: &Maps, _: &Call| Ret::Int(u32::from(corneria::falco_talk(r)));
    check_calls_with(&d, "grCorneria_801E2C34", 1, &as_saved, &talk);
}

/// Fox's and Falco's side taunt on Corneria (`ftfoxappeals.c`), the taunt counts it keeps
/// (`pl_800386D8`, `pl_80040120`) and its end (`ft_8008A324`).
#[test]
fn corneria_side_taunt() {
    for set in [CN_TALK, CN_TALK_END] {
        let d = dir(set);
        for f in [
            "ftFx_AppealS_Enter",
            "ftFx_AppealS_Anim",
            "ftFx_AppealS_Coll",
            "ftFx_AppealS_OnTakeDamage",
        ] {
            // Falco's match ends during his taunt, before it ends or is cut short.
            if set == CN_TALK || !leaf_check::calls::saved_calls(&d, f).is_empty() {
                check_calls_with(&d, f, 1, &as_saved, &by_fighter_cb);
            }
        }
        let wait = |r: &mut Running, maps: &Maps, call: &Call| {
            r.enter_wait_neutral(gobj_arg(maps, call));
            Ret::Nothing
        };
        if set == CN_TALK {
            check_calls_with(&d, "ft_8008A324", 1, &as_saved, &wait);
        }
        let check = |r: &mut Running, maps: &Maps, call: &Call| {
            Ret::Int(u32::from(r.fox_appeal_s_check_input(gobj_arg(maps, call))))
        };
        check_calls_with(&d, "ftFx_AppealS_CheckInput", 1, &as_saved, &check);
        // The talk's cue at each part.
        let cue = |c: &mut Call, k: usize| {
            let fp = fighter_of(c);
            let at = fp + 0x2210;
            let w = peek_u32(c, at);
            poke_u32(c, at, w | 0x1000_0000);
            poke_u32(c, fp + 0x2344, (k % 3) as u32);
        };
        check_calls_with(&d, "ftFx_AppealS_Anim", 1, &cue, &by_fighter_cb);
        let stats = |r: &mut Running, _: &Maps, call: &Call| {
            r.taunt_stats(call.regs.gpr[3] as i32, call.regs.gpr[4] != 0);
            Ret::Nothing
        };
        check_calls_with(&d, "pl_80040120", 1, &as_saved, &stats);
    }
    // The count, by the player whose stats r3 points into.
    let by_hi = |r: &mut Running, _: &Maps, call: &Call| {
        use melee_sim::game::mirror::Mirror;
        use melee_sim::game::structs::_StaticPlayer;
        let off = call.regs.gpr[3] - leaf_check::sym("player_slots");
        let slot = (off / _StaticPlayer::SIZE) as i32;
        let stats = r.game.player_action_stats(slot);
        let n = melee_sim::game::ft::fox::appeal::by_attack_hi(stats, call.regs.gpr[4] as i32);
        Ret::Int(n)
    };
    for set in [CN, CN_TALK] {
        check_calls_with(&dir(set), "pl_800386D8", 1, &as_saved, &by_hi);
    }
}

#[test]
fn corneria_unreached_callbacks() {
    use melee_sim::game::gr::corneria;
    let d = dir(CN);
    let from = "grCorneria_801DD9A8";
    // The demo's city, laid out from the match's.
    let demo_city = retarget(leaf_check::sym("grCorneria_801E0678"));
    check_calls_with(&d, from, 1, &demo_city, &of_running(corneria::demo_city));
    for name in [
        "grCorneria_801DD654",
        "grCorneria_801DD658",
        "grCorneria_801DDAC0",
        "grCorneria_801DE4BC",
        "grCorneria_801DE8E0",
        "grCorneria_801DF8CC",
        "grCorneria_801DFBEC",
        "grCorneria_801DFC24",
        "grCorneria_801DFC28",
        "grCorneria_801DFEB4",
        "grCorneria_801E013C",
        "grCorneria_801E03C4",
        "grCorneria_801E0DE0",
        "grCorneria_801E0E3C",
        "grCorneria_801E0F30",
        "grCorneria_801E1054",
        "grCorneria_801E12CC",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    // Map 6's setup (no match makes map 6), on the sky's object (map 7).
    let map6 = |c: &mut Call, _: usize| {
        c.addr = leaf_check::sym("stageGObj6_OnInit");
        c.regs.gpr[3] = peek_u32(c, STAGE_INFO + 0x180 + 4 * 7);
    };
    check_calls_with(&d, "Ground_801C0C2C", 1, &map6, &by_address);
}

/// An Arwing's laser and the Great Fox's: their spawns and every callback a match reached, as
/// saved and changed, and the callbacks none did from others' calls.
#[test]
fn starfox_lasers() {
    use melee_sim::game::it::kinds::{arwinglaser, greatfoxlaser};
    let straight = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let gun = maps.jobjs.get(&g[4]).copied();
        let f = |i: usize| call.regs.fpr[i].ps0 as f32;
        let _ = arwinglaser::spawn_straight(r, gobj_arg(maps, call), gun, g[5] as i32, f(1), f(2));
        Ret::Nothing
    };
    let aimed = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = &call.regs.gpr;
        let gun = maps.jobjs.get(&g[4]).copied();
        let target = vec_arg(call, 5);
        let scale = call.regs.fpr[1].ps0 as f32;
        // The shot's sound, which its caller played just before (Corneria's or Venom's).
        let sfx = if r.game.stage_info.grkind == melee_sim::game::gr::GR_KIND_VENOM {
            0x6B6C9
        } else {
            0x55734
        };
        arwinglaser::spawn_aimed(
            r,
            gobj_arg(maps, call),
            gun,
            target,
            g[6] as i32,
            g[7] as i32,
            scale,
            sfx,
        );
        Ret::Nothing
    };
    let great = |r: &mut Running, maps: &Maps, call: &Call| {
        let off = vec_arg(call, 4);
        let _ = greatfoxlaser::spawn(r, gobj_arg(maps, call), off, call.regs.gpr[5] as i32);
        Ret::Nothing
    };
    for set in [CN, CN_TALK, CN_TALK_END] {
        let d = dir(set);
        for ty in [None, Some(0u32), Some(2)] {
            let p = move |c: &mut Call, _: usize| {
                if let Some(t) = ty {
                    c.regs.gpr[5] = t;
                }
            };
            check_calls_with(&d, "it_802E72E0", 1, &p, &straight);
        }
        for ty in [None, Some(1u32), Some(3)] {
            let p = move |c: &mut Call, _: usize| {
                if let Some(t) = ty {
                    c.regs.gpr[6] = t;
                }
            };
            check_calls_with(&d, "it_802E7654", 1, &p, &aimed);
        }
        for ty in [None, Some(0u32), Some(1)] {
            let p = move |c: &mut Call, _: usize| {
                if let Some(t) = ty {
                    c.regs.gpr[5] = t;
                }
            };
            check_calls_with(&d, "it_802EAF34", 1, &p, &great);
        }
        for f in [
            "it_802E79C8",
            "it_802E838C",
            "itArwinglaser_UnkMotion2_Anim",
            "itArwinglaser_UnkMotion2_Phys",
            "itArwinglaser_UnkMotion2_Coll",
            "itArwinglaser_UnkMotion3_Anim",
            "itArwinglaser_UnkMotion3_Phys",
            "itArwinglaser_UnkMotion3_Coll",
            "itArwinglaser_UnkMotion5_Coll",
            "it_802EB1EC",
            "itGreatfoxlaser_UnkMotion1_Anim",
            "itGreatfoxlaser_UnkMotion1_Phys",
        ] {
            check_calls_with(&d, f, 1, &as_saved, &item_cb);
        }
        let meets = |r: &mut Running, maps: &Maps, call: &Call| {
            Ret::Int(u32::from(arwinglaser::meets_stage(r, gobj_arg(maps, call))))
        };
        check_calls_with(&d, "it_802E70BC", 1, &as_saved, &meets);
        let enter = |r: &mut Running, maps: &Maps, call: &Call| {
            arwinglaser::enter(r, gobj_arg(maps, call));
            Ret::Nothing
        };
        check_calls_with(&d, "it_802E7A4C", 1, &as_saved, &enter);
        let great_enter = |r: &mut Running, maps: &Maps, call: &Call| {
            greatfoxlaser::enter(r, gobj_arg(maps, call));
            Ret::Nothing
        };
        check_calls_with(&d, "it_802EB268", 1, &as_saved, &great_enter);
    }
    // The lasers' lifetimes running out, from their animation callbacks.
    let d = dir(CN);
    for f in [
        "itArwinglaser_UnkMotion2_Anim",
        "itArwinglaser_UnkMotion3_Anim",
        "itGreatfoxlaser_UnkMotion1_Anim",
    ] {
        let ends = |c: &mut Call, k: usize| {
            let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
            poke_u32(c, ip + 0xD44, [1.0f32, 0.0, 2.0][k % 3].to_bits());
        };
        check_calls_with(&d, f, 1, &ends, &item_cb);
    }
    // The hit events no match reached, from the lasers' animation callbacks.
    for (from, name) in [
        ("itArwinglaser_UnkMotion3_Anim", "it_802E8418"),
        ("itArwinglaser_UnkMotion3_Anim", "it_802E8420"),
        ("itArwinglaser_UnkMotion3_Anim", "it_802E85F4"),
        ("itArwinglaser_UnkMotion3_Anim", "it_802E8784"),
        ("itArwinglaser_UnkMotion2_Anim", "it_802E8420"),
        (
            "itGreatfoxlaser_UnkMotion1_Anim",
            "itGreatFoxLaser_Logic27_DmgDealt",
        ),
        (
            "itGreatfoxlaser_UnkMotion1_Anim",
            "itGreatFoxLaser_Logic27_Clanked",
        ),
        (
            "itGreatfoxlaser_UnkMotion1_Anim",
            "itGreatFoxLaser_Logic27_Absorbed",
        ),
        (
            "itGreatfoxlaser_UnkMotion1_Anim",
            "it_2725_Logic27_Reflected",
        ),
    ] {
        // A reflection turns the laser back at its owner or its reflector: a fighter.
        let at = leaf_check::sym(name);
        let p = move |c: &mut Call, _: usize| {
            c.addr = at;
            let ip = peek_u32(c, c.regs.gpr[3] + 0x2C);
            let f = first_fighter(c);
            poke_u32(c, ip + 0x518, f);
            poke_u32(c, ip + 0xC64, f);
        };
        check_calls_with(&d, from, 1, &p, &item_cb);
    }
}

// Venom (`grvenom.c`): the stage's oracle match and the starfox set's Venom taunt matches, captured
// as Corneria's are with `ve-funcs.txt` and `done-ve.txt` (`capture-ve.sh`).

const VE: &str = "oracle-stage-venom";
const VE_TALK: &str = "oracle-taunt-fox-venom";
const VE_TALK_END: &str = "oracle-taunt-falco-venom";

/// grvenom.c's `yakumono_param`.
const VE_PARAM: u32 = 0x804D_6A30;
/// The Arwing slots (`grVe_803E5348.arwing`, `grVe_803E5380`).
const VE_ARWINGS: u32 = 0x803E_5368;
const VE_TYPES: u32 = 0x803E_5374;
const VE_GROUPS: u32 = 0x803E_5380;
/// The wait and the demo flag (`grVe_804D6A38`, `grVe_804D6A40`).
const VE_WAIT: u32 = 0x804D_6A38;
const VE_DEMO: u32 = 0x804D_6A40;

/// Venom's parameters' word at `off`.
fn ve_param(c: &Call, off: u32) -> u32 {
    peek_u32(c, peek_u32(c, VE_PARAM) + off)
}

/// An Arwing slot's path, type and group set.
fn ve_slot(c: &mut Call, slot: u32, path: u32, ty: i32, group: i32) {
    poke_u32(c, VE_ARWINGS + 4 * slot, path);
    poke_u32(c, VE_TYPES + 4 * slot, ty as u32);
    poke_u32(c, VE_GROUPS + 4 * slot, group as u32);
}

/// The Great Fox's flags and colors' state (`xE0`, a halfword) and its animation's last frame.
fn ve_ship(c: &mut Call, xe0: u16, frame: f32) {
    let v = map_vars(c, 7);
    poke_u16(c, v + 0x1C, xe0);
    poke_u32(c, v + 0x20, frame.to_bits());
}

#[test]
fn venom_setup() {
    use melee_sim::game::gr::venom;
    let d = dir(VE);
    for f in ["grVenom_80203B18", "grVenom_80203DD0", "grVenom_80203E80"] {
        check_calls_with(&d, f, 1, &as_saved, &event);
    }
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        let _ = venom::create(r, call.regs.gpr[3] as i32, venom::INIT_FRAME);
        Ret::Nothing
    };
    check_calls_with(&d, "grVenom_80203EAC", 1, &as_saved, &create);
    for f in [
        "stageGObj0_OnInit",
        "grVenom_80203FD4",
        "grVenom_802040F0",
        "grVenom_80204428",
        "grVenom_80204DD4",
        "grVenom_80204F20",
        "grVenom_80205AD4",
    ] {
        check_calls_with(&d, f, 1, &as_saved, &by_address);
    }
    let dt = dir(VE_TALK_END);
    for f in ["grVenom_802056B0", "grVenom_80206874"] {
        check_calls_with(&dt, f, 1, &as_saved, &by_address);
    }
    // The platforms' start hook, on the event stage and not.
    let start = of_ground(venom::platform_start);
    check_calls_with(&d, "fn_802040B4", 1, &as_saved, &start);
    let event_stage = |c: &mut Call, _: usize| poke_u32(c, leaf_check::sym("selected_stage"), 0xE4);
    check_calls_with(&d, "fn_802040B4", 1, &event_stage, &start);
}

#[test]
fn venom_ship() {
    use melee_sim::game::gr::venom;
    let d = dir(VE);
    check_calls_with(&d, "grVenom_8020454C", 1, &as_saved, &by_address);
    // Each cue crossed (the last frame far behind, or each just below), the animation
    // rewinding (the last frame past it), and each flag and colors' state.
    let mut lasts = vec![0.0f32, -1.0, 1e9];
    for at in [
        1033.0f32, 1063.0, 2460.0, 3800.0, 4070.0, 4200.0, 4400.0, 6100.0, 6810.0, 7210.0, 7410.0,
        9200.0,
    ] {
        lasts.push(at - 0.5);
    }
    for last in lasts {
        for xe0 in [0u16, 0x2000, 0x0200, 0xFE00, 0x0080, 0x0100] {
            let p = move |c: &mut Call, _: usize| ve_ship(c, xe0, last);
            check_calls_with(&d, "grVenom_8020454C", 1, &p, &by_address);
        }
    }
    let colors = of_ground(venom::ship_colors);
    check_calls_with(&d, "grVenom_80204B88", 1, &as_saved, &colors);
    for xe0 in [
        0u16, 0x0080, 0x0100, 0x0180, 0x2000, 0x2080, 0x0200, 0x0280, 0x2200,
    ] {
        let p = move |c: &mut Call, _: usize| ve_ship(c, xe0, 0.0);
        check_calls_with(&d, "grVenom_80204B88", 1, &p, &colors);
    }
    let send = |r: &mut Running, _: &Maps, call: &Call| {
        r.set_ground_send(call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    for col in [None, Some(1u32), Some(2)] {
        let p = move |c: &mut Call, _: usize| {
            if let Some(v) = col {
                c.regs.gpr[3] = v;
            }
        };
        check_calls_with(&d, "lbAudioAx_80024D78", 1, &p, &send);
    }
}

#[test]
fn venom_platform() {
    let d = dir(VE);
    check_calls_with(&d, "grVenom_80204284", 1, &as_saved, &by_address);
    // The event's line: about to start, its window opening, waiting for it to go (gone or not).
    for timer in [-1i32, 1, 0x3A, 0x3B, 0x3C, 0x3D] {
        let p = move |c: &mut Call, _: usize| poke_u32(c, vars(c) + 4, timer as u32);
        check_calls_with(&d, "grVenom_80204284", 1, &p, &by_address);
    }
    check_calls_with(&d, "stageGObj9_GObjProc", 1, &as_saved, &by_address);
}

#[test]
fn venom_spawns() {
    use melee_sim::game::gr::venom;
    let d = dir(VE);
    let spawn = of_running(venom::spawn_arwings);
    check_calls_with(&d, "grVenom_8020362C", 1, &as_saved, &spawn);
    // In a match: the wait running out with no group held, the far group held, the close one,
    // after each group (several seeds); not yet; an Arwing out.
    for wait in [1i32, 2] {
        for xe0 in [0u16, 0x1000, 0x0800, 0x0400, 0x8000, 0x4000, 0x2000, 0x9000] {
            for (ty, group) in [(0i32, 0i32), (3, 1), (9, 4)] {
                let p = move |c: &mut Call, k: usize| {
                    poke_u32(c, VE_DEMO, 0);
                    poke_u32(c, VE_WAIT, wait as u32);
                    ve_slot(c, 0, 0, ty, group);
                    let v = map_vars(c, 7);
                    poke_u16(c, v + 0x1C, xe0);
                    set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
                };
                check_calls_with(&d, "grVenom_8020362C", 1, &p, &spawn);
            }
        }
    }
    let out = |c: &mut Call, k: usize| {
        poke_u32(c, VE_DEMO, 0);
        let f = first_fighter(c);
        ve_slot(c, 0, f, 3, 1);
        set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
    };
    check_calls_with(&d, "grVenom_8020362C", 1, &out, &spawn);
    // The demo: all three out, one free after close and far ones, the wait running out or not,
    // with groups held or not.
    for (free, wait) in [(3u32, 1i32), (0, 1), (1, 1), (2, 1), (2, 5)] {
        for near in [false, true] {
            for xe0 in [0u16, 0x8000] {
                let p = move |c: &mut Call, k: usize| {
                    poke_u32(c, VE_DEMO, 1);
                    poke_u32(c, VE_WAIT, wait as u32);
                    let f = first_fighter(c);
                    for slot in 0..3u32 {
                        let (path, ty) = if slot == free {
                            (0, 0)
                        } else if near {
                            (f, slot as i32 + 2)
                        } else {
                            (f, slot as i32 + 8)
                        };
                        ve_slot(c, slot, path, ty, slot as i32 + 1);
                    }
                    let v = map_vars(c, 7);
                    poke_u16(c, v + 0x1C, xe0);
                    set_seed(c, 0x0F0F_1357u32.wrapping_mul(k as u32 + 1));
                };
                check_calls_with(&d, "grVenom_8020362C", 1, &p, &spawn);
            }
        }
    }
}

#[test]
fn venom_paths() {
    use melee_sim::game::gr::venom;
    for set in [VE, VE_TALK] {
        let d = dir(set);
        check_calls_with(&d, "grVenom_802053B0", 1, &as_saved, &by_address);
        let first = |c: &mut Call, _: usize| poke_u32(c, vars(c) + 0x10, 1);
        check_calls_with(&d, "grVenom_802053B0", 1, &first, &by_address);
        for sound in 0..4u32 {
            for ty in [2i32, 7, 8, 11, 0] {
                let p = move |c: &mut Call, _: usize| {
                    let v = vars(c);
                    poke_u32(c, v + 0x10, 0);
                    poke_u32(c, v + 0x14, sound);
                    let slot = cn_slot_of(c);
                    poke_u32(c, VE_TYPES + 4 * slot, ty as u32);
                };
                check_calls_with(&d, "grVenom_802053B0", 1, &p, &by_address);
            }
        }
        let taken = |c: &mut Call, _: usize| {
            let slot = cn_slot_of(c);
            poke_u32(c, VE_ARWINGS + 4 * slot, 0);
        };
        check_calls_with(&d, "grVenom_802053B0", 1, &taken, &by_address);
        let pos = |r: &mut Running, maps: &Maps, call: &Call| {
            let _ = venom::path_pos(r, maps.gobjs.get(&call.regs.gpr[3]).copied());
            Ret::Nothing
        };
        check_calls_with(&d, "grVenom_802052E0", 1, &as_saved, &pos);
        let outside = |r: &mut Running, _: &Maps, call: &Call| {
            Ret::Int(u32::from(venom::outside_blast_zones(
                r.game,
                vec_arg(call, 3),
            )))
        };
        let near = |r: &mut Running, _: &Maps, call: &Call| {
            Ret::Int(u32::from(venom::near_blast_zones(r.game, vec_arg(call, 3))))
        };
        for p in [
            (0.0f32, 50.0f32),
            (1e4, 0.0),
            (-1e4, 0.0),
            (0.0, 1e4),
            (0.0, -1e4),
        ] {
            let put = move |c: &mut Call, _: usize| {
                let a = c.regs.gpr[3];
                poke_u32(c, a, p.0.to_bits());
                poke_u32(c, a + 4, p.1.to_bits());
            };
            check_calls_with(&d, "grVenom_80205DF8", 1, &put, &outside);
            check_calls_with(&d, "grVenom_80205E84", 1, &put, &near);
        }
        check_calls_with(&d, "grVenom_80205DF8", 1, &as_saved, &outside);
        check_calls_with(&d, "grVenom_80205E84", 1, &as_saved, &near);
    }
}

#[test]
fn venom_arwings() {
    let d = dir(VE);
    let dt = dir(VE_TALK_END);
    for (d, f) in [(&d, "grVenom_80205F30"), (&dt, "grVenom_80206B70")] {
        check_calls_with(d, f, 1, &as_saved, &by_address);
        let gone = |c: &mut Call, _: usize| {
            let slot = cn_slot_of(c);
            poke_u32(c, VE_ARWINGS + 4 * slot, 0);
        };
        check_calls_with(d, f, 1, &gone, &by_address);
        // Each roll state, rolling or not (several seeds).
        for (roll, wait) in [(0u32, 0i32), (0, 1), (1, 0), (4, 0), (5, 0)] {
            let p = move |c: &mut Call, k: usize| {
                let v = vars(c);
                poke_u32(c, v + 0x30, roll);
                poke_u32(c, v + 0x34, wait as u32);
                set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
            };
            check_calls_with(d, f, 1, &p, &by_address);
        }
        // A far Arwing in each type's window, at its edges and outside (several seeds).
        for ty in [8i32, 9, 10, 11] {
            for flown in [
                0x30u32, 0x38, 0x40, 0x48, 0x60, 0x80, 0xE0, 0xE8, 0xF0, 0x100, 0x108,
            ] {
                let p = move |c: &mut Call, k: usize| {
                    let slot = cn_slot_of(c);
                    let t = peek_u32(c, VE_TYPES + 4 * slot) as i32;
                    if (8..=11).contains(&t) {
                        poke_u32(c, VE_TYPES + 4 * slot, ty as u32);
                    }
                    poke_u32(c, vars(c) + 0x2C, flown);
                    set_seed(c, 0x1357_9BDFu32.wrapping_mul(k as u32 + 1));
                };
                check_calls_with(d, f, 1, &p, &by_address);
            }
        }
    }
    // A wing: as saved, its path gone, rolled and off the plane either side of its limits.
    let f = "grVenom_80205758";
    check_calls_with(&d, f, 1, &as_saved, &by_address);
    let gone = |c: &mut Call, _: usize| {
        let slot = cn_slot_of(c);
        poke_u32(c, VE_ARWINGS + 4 * slot, 0);
    };
    check_calls_with(&d, f, 1, &gone, &by_address);
    for (roll, z) in [
        (0.0f32, 0.0f32),
        (1.04, 0.0),
        (1.06, 0.0),
        (-4.0, 0.0),
        (0.5, 49.9),
        (0.5, 50.0),
        (0.0, -51.0),
    ] {
        let p = move |c: &mut Call, _: usize| {
            let v = vars(c);
            poke_u32(c, v + 0x18, roll.to_bits());
            poke_u32(c, v + 0x1C + 8, z.to_bits());
        };
        check_calls_with(&d, f, 1, &p, &by_address);
    }
    let _ = ve_param;
}

#[test]
fn venom_talk() {
    use melee_sim::game::gr::venom;
    for set in [VE_TALK, VE_TALK_END] {
        let d = dir(set);
        check_calls_with(&d, "grVenom_80204CEC", 1, &as_saved, &by_address);
        for (line, wait) in [(0i32, -1i32), (1, 0), (3, -1), (4, -1)] {
            let p = move |c: &mut Call, _: usize| {
                let v = vars(c);
                poke_u32(c, v + 4, line as u32);
                poke_u32(c, v + 8, wait as u32);
            };
            check_calls_with(&d, "grVenom_80204CEC", 1, &p, &by_address);
        }
        for f in ["grVenom_80206B94", "grVenom_80206BC4"] {
            check_calls_with(&d, f, 1, &as_saved, &by_address);
        }
        let start = |r: &mut Running, _: &Maps, call: &Call| {
            Ret::Int(u32::from(venom::start_talk(r, call.regs.gpr[3] as i32)))
        };
        for which in [None, Some(2u32), Some(20), Some(5)] {
            let p = move |c: &mut Call, k: usize| {
                if let Some(w) = which {
                    c.regs.gpr[3] = w;
                }
                set_seed(c, 0x2468_ACE1u32.wrapping_mul(k as u32 + 1));
            };
            check_calls_with(&d, "grVenom_80206BF0", 1, &p, &start);
        }
        let stop = |r: &mut Running, _: &Maps, call: &Call| {
            venom::stop_talk(r, call.regs.gpr[3] != 0);
            Ret::Nothing
        };
        for destroy in [None, Some(0u32), Some(1)] {
            let p = move |c: &mut Call, _: usize| {
                if let Some(v) = destroy {
                    c.regs.gpr[3] = v;
                }
            };
            check_calls_with(&d, "grVenom_80206CB0", 1, &p, &stop);
        }
    }
    check_calls_with(&dir(VE_TALK), "grVenom_80204DB0", 1, &as_saved, &by_address);
    // Corneria's talk code on Venom: the taunts' calls, the match's end and the cut short taunt.
    let d = dir(VE_TALK);
    let fox = |r: &mut Running, _: &Maps, _: &Call| {
        Ret::Int(u32::from(melee_sim::game::gr::corneria::fox_talk(r)))
    };
    check_calls_with(&d, "grCorneria_801E2B80", 1, &as_saved, &fox);
    let falco = |r: &mut Running, _: &Maps, _: &Call| {
        Ret::Int(u32::from(melee_sim::game::gr::corneria::falco_talk(r)))
    };
    check_calls_with(
        &dir(VE_TALK_END),
        "grCorneria_801E2C34",
        1,
        &as_saved,
        &falco,
    );
    let end = of_running(melee_sim::game::gr::corneria::match_end);
    for set in [VE, VE_TALK, VE_TALK_END] {
        check_calls_with(&dir(set), "grCorneria_801E2A6C", 1, &as_saved, &end);
    }
    let cut = of_running(melee_sim::game::gr::corneria::taunt_interrupted);
    check_calls_with(&d, "grCorneria_801E2AF4", 1, &as_saved, &cut);
}

#[test]
fn venom_hooks() {
    use melee_sim::game::gr::venom;
    for set in [VE, VE_TALK] {
        let d = dir(set);
        let hazard = |_: &mut Running, _: &Maps, _: &Call| {
            assert!(venom::line_hazard().is_none());
            Ret::Int(0)
        };
        check_calls_with(&d, "grVenom_80206D74", 1, &as_saved, &hazard);
        fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
            let (pos, j) = shadow_args(maps, call);
            venom::check_shadow_render(r.game, r.data, pos, j)
        }
        check_shadow(&d, "grVenom_80206D7C", ours, 0.0);
    }
}

#[test]
fn venom_unreached_callbacks() {
    let d = dir(VE);
    let from = "grVenom_8020454C";
    for name in [
        "grVenom_80203FCC",
        "grVenom_80203FD0",
        "grVenom_802040AC",
        "grVenom_802040B0",
        "grVenom_80204424",
        "grVenom_80204B84",
        "grVenom_80204CE0",
        "grVenom_80204F1C",
        "grVenom_802056AC",
        "grVenom_80205AD0",
        "grVenom_80206870",
        "grVenom_80206B90",
        "grVenom_80206BEC",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    check_calls_with(
        &d,
        from,
        1,
        &retarget(leaf_check::sym("grVenom_80203B14")),
        &demo,
    );
}

// Princess Peach's Castle (`grcastle.c`).

const CS: &str = "oracle-stage-peachs-castle";
/// The walkers' match (`castle-walkers`), which falls past the moat and rides the lifts.
const CSW: &str = "oracle-castle-walkers";

/// grcastle.c's `yakumono_param`.
const CS_PARAM: u32 = 0x804D_6970;

/// The stage's scale (`Ground_801C0498`: the stage parameters' first float).
fn cs_scale(call: &Call) -> f32 {
    f32_at(call, peek_u32(call, STAGE_INFO + 0x6B0))
}

/// The stack address of the scale joint's descriptor a stage object's creation in this call
/// puts in the ID table: `setup::SETUP_FRAME` when the setup's create runs with its stack
/// pointer at `CS_SETUP_SP`, as deep below as the call's is.
const CS_SETUP_SP: u32 = 0x804E_EA80;

fn cs_frame(call: &Call) -> u32 {
    melee_sim::game::gr::setup::SETUP_FRAME
        .wrapping_add(call.regs.gpr[1])
        .wrapping_sub(CS_SETUP_SP)
}

/// The start hooks' list left out (`stage_info.x6A4`): the castle's start hook runs from
/// `Ground_801C0FB8`'s loop, which frees each node after its call and clears the list's head
/// only after the loop, so the objects the hook makes take the lifts' freed node, which nothing
/// reads, and a call made in it finds a list ending in another object's memory. Without the
/// head neither side keeps the nodes, and a node added (the setup's) starts a list of its own
/// on both.
fn cs_saved(c: &mut Call, _: usize) {
    poke_u32(c, STAGE_INFO + 0x6A4, 0);
}

/// A function taking its stage object, as a check's port.
fn cs_of(
    f: fn(&mut Running, melee_sim::hsd::gobj::GObjId),
) -> impl Fn(&mut Running, &Maps, &Call) -> Ret + Sync {
    move |r: &mut Running, maps: &Maps, call: &Call| {
        f(r, gobj_arg(maps, call));
        Ret::Nothing
    }
}

/// A flag's chain from its address in the castle's variables: the castle's `Ground` and which.
fn cs_flag(maps: &Maps, a: u32) -> (melee_sim::game::handles::GroundId, usize) {
    for (&gp, &g) in &maps.grounds {
        let first = gp + 0xC4 + 0x1C;
        if a >= first && a < first + 12 * 0x14 && (a - first).is_multiple_of(0x14) {
            return (g, ((a - first) / 0x14) as usize);
        }
    }
    panic!("{a:#x} isn't a flag's chain")
}

#[test]
fn castle_setup() {
    let d = dir(CS);
    // Every check runs, and the test fails naming each that differed.
    let mut failed: Vec<&str> = Vec::new();
    let mut each = |name: &'static str, check: &dyn Fn()| {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).is_err() {
            failed.push(name);
        }
    };
    for f in ["grCastle_801CD37C", "grCastle_801CD4A4"] {
        each(f, &|| check_calls_with(&d, f, 1, &cs_saved, &event));
    }
    let create = |r: &mut Running, _: &Maps, call: &Call| {
        castle::create(r, call.regs.gpr[3] as i32, cs_frame(call));
        Ret::Nothing
    };
    each("grCastle_801CD4D0", &|| {
        check_calls_with(&d, "grCastle_801CD4D0", 1, &cs_saved, &create)
    });
    for f in [
        "grCastle_801CD5BC",
        "grCastle_801CD610",
        "grCastle_801CD658",
        "grCastle_801CD9B4",
        "grCastle_801CDA0C",
        "grCastle_801CE260",
        "grCastle_801CE7E8",
        "grCastle_801CE8E8",
        "grCastle_801CEACC",
        "grCastle_801CF0F4",
    ] {
        each(f, &|| check_calls_with(&d, f, 1, &cs_saved, &by_address));
    }
    each("grCastle_801CF7B0", &|| {
        check_calls_with(
            &d,
            "grCastle_801CF7B0",
            1,
            &cs_saved,
            &cs_of(castle::init_switches),
        )
    });
    each("grCastle_801CDFD8", &|| {
        check_calls_with(
            &d,
            "grCastle_801CDFD8",
            1,
            &cs_saved,
            &cs_of(castle::reset_bill_wait),
        )
    });
    // The flags' chains from the archive's description, their swing and their release.
    let chain = |r: &mut Running, maps: &Maps, call: &Call| {
        let root = maps.jobjs.get(&call.regs.gpr[3]).copied();
        let archive = melee_sim::game::gr::datfiles::main_archive(r.game).archive;
        let offset = (r.data.get(archive))
            .public("dynamicsdata_flag6")
            .expect("the flags' dynamics");
        let src = melee_data::archive::DataRef { archive, offset };
        let (g, i) = cs_flag(maps, call.regs.gpr[5]);
        castle::chain(r, root, src, g, i);
        Ret::Nothing
    };
    each("grLib_801C9B20", &|| {
        check_calls_with(&d, "grLib_801C9B20", 1, &cs_saved, &chain)
    });
    let swing = |r: &mut Running, maps: &Maps, call: &Call| {
        let (g, i) = cs_flag(maps, call.regs.gpr[3]);
        castle::swing(r, g, i);
        Ret::Nothing
    };
    each("grLib_801C9B8C", &|| {
        check_calls_with(&d, "grLib_801C9B8C", 1, &cs_saved, &swing)
    });
    let free = |r: &mut Running, maps: &Maps, call: &Call| {
        let (g, i) = cs_flag(maps, call.regs.gpr[3]);
        let mut desc = r.game.grounds[g.0].u.castle().dynamics[i].clone();
        melee_sim::game::gr::grlib::free_chain(r.game, &mut desc);
        r.game.grounds[g.0].u.castle().dynamics[i] = desc;
        Ret::Nothing
    };
    let t = leaf_check::sym("grLib_801C9B6C");
    each("grLib_801C9B6C", &|| {
        check_calls_with(&d, "grLib_801C9B8C", 1, &retarget(t), &free)
    });
    assert!(failed.is_empty(), "these differ: {failed:?}");
}

/// The castle's process and its parts: the switches' and Bullet Bills' timers, a path's draw,
/// the wind's steering and strength and the flags' turn, each also from states set to its
/// branches.
#[test]
fn castle_stage() {
    let d = dir(CS);
    check_calls_with(&d, "grCastle_801CD8A8", 1, &cs_saved, &by_address);
    // The switches' timer: none out and one out at rest or not, the wait running out, the
    // last switch's weight divided.
    let timer = cs_of(castle::switch_timer);
    check_calls_with(&d, "grCastle_801CF868", 1, &cs_saved, &timer);
    for (slot, wait, state) in [
        (-1i32, 0u32, 0u32),
        (0, 0, 0),
        (1, 0, 0),
        (2, 0, 0),
        (1, 0, 3),
        (2, 5, 0),
        (-1, 1, 0),
        (-1, 0x8000, 0),
    ] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let v = vars(c);
            poke_u16(c, v + 0xC, slot as u16);
            poke_u16(c, v + 0xE, wait as u16);
            let s = if slot >= 0 {
                peek_u32(c, v + 4 * slot as u32)
            } else {
                0
            };
            if s != 0 {
                let sv = peek_u32(c, s + 0x2C) + 0xC4;
                poke_u16(c, sv, state as u16);
            }
            // A draw's other outcomes: the seed moved on by the call's index.
            let seed = leaf_check::sym("seed");
            let w = peek_u32(c, seed);
            poke_u32(c, seed, w.wrapping_add(k as u32 * 0x9E37_79B9));
        };
        check_calls_with(&d, "grCastle_801CF868", 1, &set, &timer);
    }
    // The Bullet Bills' timer, due and not, as the wait runs out.
    let bills = cs_of(castle::bill_timer);
    check_calls_with(&d, "grCastle_801CE19C", 1, &cs_saved, &bills);
    for (due, wait) in [(true, 0u16), (true, 0xFFFF), (false, 0xFFFF), (true, 7)] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let v = vars(c);
            poke_u8(c, v + 0x1A, if due { 0x80 } else { 0 });
            poke_u16(c, v + 0x10, wait);
        };
        check_calls_with(&d, "grCastle_801CE19C", 1, &set, &bills);
    }
    // A path's draw with the last three paths each way, none flown and the ring's turn at
    // each place.
    let pick = |r: &mut Running, maps: &Maps, call: &Call| {
        Ret::Int(castle::pick_path(r, gobj_arg(maps, call)) as u32)
    };
    let t = leaf_check::sym("grCastle_801CE054");
    for (recent, at) in [
        ([-1i16, -1, -1], 0u16),
        ([0, 1, 2], 1),
        ([6, 7, 8], 2),
        ([3, 3, 3], 0),
        ([8, 0, 4], 2),
    ] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.addr = t;
            let v = vars(c);
            for (i, p) in recent.iter().enumerate() {
                poke_u16(c, v + 0x14 + 2 * i as u32, *p as u16);
            }
            poke_u16(c, v + 0x12, at);
        };
        check_calls_with(&d, "grCastle_801CE19C", 1, &set, &pick);
    }
    // The wind's steering: on and past its turns and gusts, with gusts on and off, near its
    // limits.
    let steer = |r: &mut Running, _: &Maps, _: &Call| {
        castle::steer_wind(r);
        Ret::Nothing
    };
    check_calls_with(&d, "grCastle_801D0BBC", 1, &cs_saved, &steer);
    for (age, gust, x, step) in [
        (0u32, 0u8, 1.0f32, 0.017_453_292f32),
        (30, 1, 0.5, -0.017_453_292),
        (300, 1, 0.51, 0.017_453_292),
        (600, 0, 0.49, 0.017_453_292),
        (31, 0, -1.0, -0.017_453_292),
    ] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let w = peek_u32(c, leaf_check::sym("grCs_804D6974"));
            poke_u32(c, w + 0x30, age);
            poke_u32(c, w + 4, x.to_bits());
            poke_u32(c, w + 0xC, (1.0f32 - x * x).max(0.0).sqrt().to_bits());
            poke_u8(c, leaf_check::sym("grCs_804D45E4"), gust);
            poke_u32(c, leaf_check::sym("grCs_804D45E0"), step.to_bits());
        };
        check_calls_with(&d, "grCastle_801D0BBC", 1, &set, &steer);
    }
    let rescale = |r: &mut Running, _: &Maps, _: &Call| {
        castle::rescale_wind(r);
        Ret::Nothing
    };
    for gust in [None, Some(0u8), Some(1)] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            if let Some(g) = gust {
                poke_u8(c, leaf_check::sym("grCs_804D45E4"), g);
            }
        };
        check_calls_with(&d, "grCastle_801D0D24", 1, &set, &rescale);
    }
    // A flag's turn into the wind, as saved and from poles turned every way, a half turn and
    // past it.
    let turn = |r: &mut Running, maps: &Maps, call: &Call| {
        castle::turn_to_wind(r, maps.jobjs.get(&call.regs.gpr[3]).copied());
        Ret::Nothing
    };
    check_calls_with(&d, "grCastle_801D0D84", 1, &cs_saved, &turn);
    for y in [0.0f32, 1.5, -1.5, 3.13, -3.13, 3.2, -3.2, 6.0] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let j = c.regs.gpr[3];
            poke_u32(c, j + 0x1C + 4, y.to_bits());
        };
        check_calls_with(&d, "grCastle_801D0D84", 1, &set, &turn);
    }
}

/// The lifts' process, every lift in each state stood on and not, at and past its waits, near
/// the top and the bottom, and a lift's joint touched by each kind of collision.
#[test]
fn castle_lifts() {
    for d in [dir(CS), dir(CSW)] {
        check_calls_with(&d, "grCastle_801CDC44", 1, &cs_saved, &by_address);
        for state in 0..5u16 {
            for (wind, timer, at) in [
                (0.0f32, 0u16, 0.5f32),
                (1.0, 0, 0.5),
                (2.0, 5000, 0.999),
                (0.0, 5000, 0.001),
                (0.5, 0x7FFF, 0.0),
                // Between the sink's wait (x3C, 20) and the rise's (x38, 30).
                (1.0, 25, 0.5),
                (0.0, 25, 0.5),
                (0.0, 0x8000, 1.0),
            ] {
                let set = move |c: &mut Call, k: usize| {
                    cs_saved(c, k);
                    let v = vars(c);
                    let p = peek_u32(c, CS_PARAM);
                    let (top, bottom) = (f32_at(c, p + 0x10), f32_at(c, p + 0x14));
                    for i in 0..2 {
                        let l = v + 0x10 * i;
                        poke_u16(c, l + 8, state);
                        poke_u16(c, l + 0xA, timer);
                        poke_u32(c, l + 0xC, wind.to_bits());
                        poke_u32(c, l + 4, (bottom + (top - bottom) * at).to_bits());
                    }
                };
                check_calls_with(&d, "grCastle_801CDC44", 1, &set, &by_address);
            }
        }
        let touch = |r: &mut Running, maps: &Maps, call: &Call| {
            let g = *maps
                .grounds
                .get(&call.regs.gpr[3])
                .expect("the lifts' Ground");
            let kind = peek_u32(call, call.regs.gpr[5] + 0x34) >> 27 & 0xF;
            let (joint, x50) = (call.regs.gpr[4] as i32, call.regs.gpr[6] as i32);
            castle::lift_touched(&mut r.game.grounds[g.0], joint, kind, x50);
            Ret::Nothing
        };
        let prepares: [&Prepare; 4] = [
            &cs_saved,
            &|c, k| rc_kind(c, (k % 5) as u32),
            &|c, _| c.regs.gpr[4] ^= 1,
            &|c, _| c.regs.gpr[6] = 2500,
        ];
        for p in prepares {
            check_calls_with(&d, "grCastle_801CF750", 1, p, &touch);
        }
    }
}

/// The Bullet Bills, their blasters and blasts: each process as saved and from its branches
/// (the blaster's wait run out, the flight ended, the lingering run out), the blaster's place,
/// and where the first Bullet Bill is.
#[test]
fn castle_bills() {
    let d = dir(CS);
    check_calls_with(&d, "grCastle_801CE578", 1, &cs_saved, &by_address);
    // In flight (its blaster's wait run out, with none yet, or not), and lingering (run out or
    // not). A Bullet Bill whose flight has ended keeps pointing at its freed blaster, so the
    // flight's changes leave those as saved.
    for (delay, linger, done) in [
        (0xFFFFu16, 3u16, false),
        (4, 3, false),
        (0xFFFF, 0xFFFF, true),
        (4, 0, true),
        (4, 4, true),
    ] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let v = vars(c);
            let ended = peek_u32(c, v) & 0x8000_0000 != 0;
            if done {
                poke_u8(c, v, 0x80);
                poke_u16(c, v + 4, linger);
            } else if !ended {
                poke_u16(c, v + 6, delay);
                if delay == 0xFFFF {
                    poke_u32(c, v + 8, 0);
                }
            }
        };
        check_calls_with(&d, "grCastle_801CE578", 1, &set, &by_address);
    }
    check_calls_with(
        &d,
        "grCastle_801CE3AC",
        1,
        &cs_saved,
        &cs_of(castle::place_blaster),
    );
    check_calls_with(&d, "grCastle_801CE860", 1, &cs_saved, &by_address);
    for anim in 0..5u32 {
        for timer in [0xFFFFu16, 3] {
            let set = move |c: &mut Call, k: usize| {
                cs_saved(c, k);
                let v = vars(c);
                poke_u32(c, v, anim.min(3));
                poke_u16(c, v + 4, timer);
            };
            check_calls_with(&d, "grCastle_801CE860", 1, &set, &by_address);
        }
    }
    check_calls_with(&d, "grCastle_801CE9E8", 1, &cs_saved, &by_address);
    let at = |r: &mut Running, _: &Maps, _: &Call| {
        let found = castle::bill_position(r.game, r.data);
        if let Some(v) = found {
            r.game.stage_info.x730 = v;
        }
        Ret::Int(u32::from(found.is_some()))
    };
    // The place goes where the prepare points it: the stage's barrel place, compared state
    // nothing on Peach's Castle uses.
    let into = |c: &mut Call, _: usize| c.regs.gpr[3] = leaf_check::sym("stage_info") + 0x730;
    check_calls_with(&d, "grCastle_801CDF54", 1, &into, &at);
}

/// The blocks' process in each state with its timer at and past its ends, their growing and
/// shrinking with items on and off, bumped blocks at every step and state, and their render
/// callback.
#[test]
fn castle_blocks() {
    let d = dir(CS);
    check_calls_with(&d, "grCastle_801CEF04", 1, &cs_saved, &by_address);
    for state in 0..6u16 {
        // 60 and 61: the flicker's limit (x44) and past it.
        for timer in [0u32, 0xFFFF_FFFF, 5, 60, 61, 0x7FFF_FFFF] {
            let set = move |c: &mut Call, k: usize| {
                cs_saved(c, k);
                let v = vars(c);
                poke_u16(c, v, state);
                poke_u32(c, v + 8, timer);
            };
            check_calls_with(&d, "grCastle_801CEF04", 1, &set, &by_address);
        }
    }
    let grow = |r: &mut Running, maps: &Maps, call: &Call| {
        let done = castle::grow(r, gobj_arg(maps, call), call.regs.gpr[4] != 0);
        Ret::Int(u32::from(done))
    };
    // Items on (frequency 1) and off, with the bob-omb rain on and off.
    let items = |c: &mut Call, freq: u8, rain: u8| {
        // `controller.start` (`gm_GetStartMeleeRules`): `item_freq` and `x6`.
        let start = leaf_check::sym("controller") + 0x24C8;
        poke_u8(c, start + 0xB, freq);
        poke_u8(c, start + 6, rain);
    };
    for p in [
        &cs_saved as &Prepare,
        &|c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.regs.gpr[4] ^= 1;
        },
        &move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.regs.gpr[4] = 1;
            items(c, 1, 0);
        },
        &move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.regs.gpr[4] = 1;
            items(c, 1, 1);
        },
    ] {
        check_calls_with(&d, "grCastle_801CFBD4", 1, p, &grow);
    }
    let bump = cs_of(castle::bump);
    check_calls_with(&d, "grCastle_801D02B8", 1, &cs_saved, &bump);
    for state in 0..5u16 {
        for step in 0..6u8 {
            let set = move |c: &mut Call, k: usize| {
                cs_saved(c, k);
                let v = vars(c);
                poke_u16(c, v, state);
                for i in 0..5 {
                    poke_u8(
                        c,
                        v + 0x70 + i,
                        u8::from(!(i as usize + k).is_multiple_of(3)),
                    );
                    poke_u8(c, v + 0x75 + i, step);
                }
            };
            check_calls_with(&d, "grCastle_801D02B8", 1, &set, &bump);
        }
    }
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        castle::render_blocks(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let setup = leaf_check::calls::Setup {
        gx: true,
        ..Default::default()
    };
    for p in [
        &cs_saved as &Prepare,
        &|c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.regs.gpr[4] ^= 3;
        },
        &|c: &mut Call, k: usize| {
            cs_saved(c, k);
            let v = vars(c);
            poke_u16(c, v + 4, 1);
        },
        // The blocks shown (out in the match, they draw), flickered on and off.
        &|c: &mut Call, k: usize| {
            cs_saved(c, k);
            let v = vars(c);
            for i in 0..5 {
                let j = peek_u32(c, v + 0xC + 4 * i);
                if j != 0 {
                    let fl = peek_u32(c, j + 0x14);
                    poke_u32(c, j + 0x14, fl & !0x10);
                }
            }
            poke_u16(c, v + 4, (k % 2) as u16);
        },
    ] {
        for d in [&d, &dir(CSW)] {
            leaf_check::calls::check_calls_in(d, "grCastle_801D0520", 1, p, &setup, &render);
        }
    }
}

/// A block's item hit and touched (`grCastle_801D0550` to `801D08AC`): hit, and touched by a
/// fighter and by a stage item (its own, which does nothing). With items off the blocks never
/// hold items (only a shown ? block takes one), and no capture holds them out with items, so
/// the calls come from a Bullet Bill's process: the bill's hitbox item made a block's of the
/// first switch's blocks (map 17), with the item in r3 and the blocks' `Ground` in r4 (the
/// callbacks only destroy it). A call whose Bullet Bill has no item left runs the blocks'
/// empty destroy callback instead.
#[test]
fn castle_block_hits() {
    let d = dir(CS);
    let from = "grCastle_801CE578";
    let empty = leaf_check::sym("grCastle_801CF0F0");
    let names = [
        ("grCastle_801D0550", "grCastle_801D06CC"),
        ("grCastle_801D059C", "grCastle_801D0744"),
        ("grCastle_801D05E8", "grCastle_801D07BC"),
        ("grCastle_801D0634", "grCastle_801D0834"),
        ("grCastle_801D0680", "grCastle_801D08AC"),
    ];
    // The bill's item made block `i`'s, the call retargeted to `t` with the item in r3, the
    // blocks' `Ground` in r4 and the toucher in r5.
    let to = move |c: &mut Call, i: usize, t: u32, fighter: bool| {
        cs_saved(c, 0);
        let item = peek_u32(c, vars(c) + 0xC);
        let blocks = peek_u32(c, STAGE_INFO + 0x180 + 4 * 17);
        if item == 0 || blocks == 0 {
            c.addr = empty;
            return;
        }
        let gp = peek_u32(c, blocks + 0x2C);
        poke_u32(c, gp + 0xC4 + 0x48 + 4 * i as u32, item);
        let toucher = if fighter {
            let heads = peek_u32(c, leaf_check::sym("HSD_GObjPLinkHead"));
            peek_u32(c, heads + 4 * 8)
        } else {
            item
        };
        c.addr = t;
        c.regs.gpr[3] = item;
        c.regs.gpr[4] = gp;
        c.regs.gpr[5] = toucher;
    };
    for (i, (hit, touched)) in names.into_iter().enumerate() {
        let ht = leaf_check::sym(hit);
        let hit_port = move |r: &mut Running, maps: &Maps, call: &Call| {
            if call.addr == empty {
                return by_address(r, maps, call);
            }
            let g = *maps
                .grounds
                .get(&call.regs.gpr[4])
                .expect("the blocks' Ground");
            castle::block_hit(r, g, i);
            Ret::Nothing
        };
        check_calls_with(&d, from, 1, &move |c, _| to(c, i, ht, false), &hit_port);
        let tt = leaf_check::sym(touched);
        let touch_port = move |r: &mut Running, maps: &Maps, call: &Call| {
            if call.addr == empty {
                return by_address(r, maps, call);
            }
            let g = *maps
                .grounds
                .get(&call.regs.gpr[4])
                .expect("the blocks' Ground");
            let by = maps.gobjs.get(&call.regs.gpr[5]).copied();
            castle::block_touched(r, g, by, i);
            Ret::Nothing
        };
        for fighter in [true, false] {
            check_calls_with(&d, from, 1, &move |c, _| to(c, i, tt, fighter), &touch_port);
        }
    }
}

/// The switches' process in each state with its waits at and past their ends, and the
/// switches pressed by a hit and a touch.
#[test]
fn castle_switches() {
    let d = dir(CS);
    check_calls_with(&d, "grCastle_801CF308", 1, &cs_saved, &by_address);
    for state in 0..7u16 {
        for (blink, delay, count) in [
            (0u16, 0u16, 0u16),
            (36, 0, 0xFFFF),
            (5, 3, 2),
            (0xFFFF, 0, 0),
        ] {
            let set = move |c: &mut Call, k: usize| {
                cs_saved(c, k);
                let v = vars(c);
                // A switch's spot: its own when it has one, else the castle's part 16.
                if peek_u32(c, v + 0x10) == 0 {
                    let castle = peek_u32(c, STAGE_INFO + 0x180 + 4 * 3);
                    poke_u32(c, v + 0x10, nth_jobj(c, castle, 16));
                }
                poke_u16(c, v, state);
                poke_u16(c, v + 4, blink);
                poke_u16(c, v + 6, delay);
                poke_u16(c, v + 8, count);
            };
            check_calls_with(&d, "grCastle_801CF308", 1, &set, &by_address);
        }
    }
    // Sent out (state 1) with its model's scale off its rest: the part reset puts it back.
    let resized = |c: &mut Call, k: usize| {
        cs_saved(c, k);
        let v = vars(c);
        if peek_u32(c, v + 0x10) == 0 {
            let castle = peek_u32(c, STAGE_INFO + 0x180 + 4 * 3);
            poke_u32(c, v + 0x10, nth_jobj(c, castle, 16));
        }
        poke_u16(c, v, 1);
        let j = peek_u32(c, c.regs.gpr[3] + 0x28);
        poke_u32(c, j + 0x2C + 4, 2.0f32.to_bits());
    };
    check_calls_with(&d, "grCastle_801CF308", 1, &resized, &by_address);
    let hit = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps
            .grounds
            .get(&call.regs.gpr[4])
            .expect("the switch's Ground");
        let item = *maps
            .gobjs
            .get(&call.regs.gpr[3])
            .expect("the switch's item");
        let by = maps.gobjs.get(&call.regs.gpr[6]).copied();
        castle::switch_hit(r, item, g, Vec3::ZERO, by);
        Ret::Nothing
    };
    let touched = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = *maps
            .grounds
            .get(&call.regs.gpr[4])
            .expect("the switch's Ground");
        let item = *maps
            .gobjs
            .get(&call.regs.gpr[3])
            .expect("the switch's item");
        let by = maps.gobjs.get(&call.regs.gpr[5]).copied();
        castle::switch_touched(r, item, g, by);
        Ret::Nothing
    };
    check_calls_with(&d, "fn_801CFAFC", 0, &cs_saved, &hit);
    check_calls_with(&d, "fn_801CFB68", 0, &cs_saved, &touched);
    // From a hit's captures: no fighter, and the touch with the hit's fighter.
    check_calls_with(&d, "fn_801CFAFC", 1, &|c, _| c.regs.gpr[6] = 0, &hit);
    let t = leaf_check::sym("fn_801CFB68");
    let as_touch = move |c: &mut Call, _: usize| {
        c.addr = t;
        c.regs.gpr[5] = c.regs.gpr[6];
    };
    check_calls_with(&d, "fn_801CFAFC", 1, &as_touch, &touched);
}

/// The moat's wind device on a fighter above the surface, below it, falling fast and slowly,
/// and just falling in (with the splash).
#[test]
fn castle_moat() {
    for d in [dir(CS), dir(CSW)] {
        // The push goes where the prepares point the callback's vector: the stage's barrel
        // place, compared state that nothing on Peach's Castle uses.
        let moat = |r: &mut Running, maps: &Maps, call: &Call| {
            let ground = gobj_arg(maps, call);
            let fighter = *maps
                .gobjs
                .get(&call.regs.gpr[4])
                .expect("the fighter's GObj");
            let push = castle::moat(r, ground, fighter);
            if let Some(v) = push {
                r.game.stage_info.x730 = v;
            }
            Ret::Int(u32::from(push.is_some()))
        };
        let water = |c: &mut Call| c.regs.gpr[5] = leaf_check::sym("stage_info") + 0x730;
        let place = move |c: &mut Call, now: f32, before: f32, dy: f32| {
            water(c);
            let surface = 14.0 * cs_scale(c);
            let fp = peek_u32(c, c.regs.gpr[4] + 0x2C);
            poke_u32(c, fp + 0xB0 + 4, (surface + now).to_bits());
            poke_u32(c, fp + 0xBC + 4, (surface + before).to_bits());
            poke_u32(c, fp + 0xC8 + 4, dy.to_bits());
        };
        let prepares: [&Prepare; 6] = [
            &move |c, _| water(c),
            &move |c, _| place(c, 5.0, 5.0, 0.0),
            &move |c, _| place(c, -5.0, -3.0, -2.0),
            &move |c, _| place(c, -5.0, -3.0, -0.5),
            &move |c, _| place(c, -2.0, 1.0, -3.0),
            &move |c, _| place(c, -0.0, 0.0, 0.0),
        ];
        for p in prepares {
            check_calls_with(&d, "grCastle_801D09B8", 1, p, &moat);
        }
    }
    // The splashes the walkers make.
    let splash = |r: &mut Running, _: &Maps, call: &Call| {
        let at = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, at), f32_at(call, at + 4), f32_at(call, at + 8));
        castle::splash(r, pos, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    check_calls_with(&dir(CSW), "grCastle_801D0A9C", 1, &cs_saved, &splash);
}

#[test]
fn castle_shadow() {
    let d = dir(CS);
    fn ours(r: &mut Running, maps: &Maps, call: &Call) -> bool {
        let (pos, j) = shadow_args(maps, call);
        castle::check_shadow_render(r.game, r.data, pos, j)
    }
    check_shadow(&d, "grCastle_801D0B0C", ours, -1.0);
    // Right of 192 below 137 (in the stage's scale), and on either edge.
    let shadow =
        |r: &mut Running, maps: &Maps, call: &Call| Ret::Int(u32::from(ours(r, maps, call)));
    for (x, y) in [
        (200.0f32, 100.0f32),
        (192.0, 100.0),
        (200.0, 137.0),
        (100.0, 100.0),
    ] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            let s = cs_scale(c);
            let p = c.regs.gpr[3];
            poke_u32(c, p, (x * s).to_bits());
            poke_u32(c, p + 4, (y * s).to_bits());
        };
        check_calls_with(&d, "grCastle_801D0B0C", 1, &set, &shadow);
    }
}

/// The callbacks no capture reaches, from the castle's process's captures: the empty
/// processes and destroy callbacks, the load and demo hooks, the Bullet Bills' and blasts'
/// item callbacks, the castle's destroy callback and render callback, the splash and the
/// camera's floor.
#[test]
fn castle_unreached_callbacks() {
    let d = dir(CS);
    let from = "grCastle_801CD8A8";
    for name in [
        "grCastle_801CD608",
        "grCastle_801CD60C",
        "grCastle_801CDF50",
        "grCastle_801CE7E4",
        "grCastle_801CE8E4",
        "grCastle_801CEAC8",
        "grCastle_801CF0F0",
        "grCastle_801CF74C",
        "grCastle_801CD960",
    ] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &by_address);
    }
    let t = leaf_check::sym("grCastle_801CD4A0");
    check_calls_with(&d, from, 1, &retarget(t), &event);
    let t = leaf_check::sym("grCastle_801CD338");
    check_calls_with(&d, from, 1, &retarget(t), &demo);
    let nothing = |r: &mut Running, maps: &Maps, call: &Call| {
        let g = melee_sim::game::gr::ground::ground_of(r.game, gobj_arg(maps, call));
        castle::item_nothing(r, gobj_arg(maps, call), g);
        Ret::Nothing
    };
    for name in ["fn_801CE3A0", "fn_801CE9DC"] {
        check_calls_with(&d, from, 1, &retarget(leaf_check::sym(name)), &nothing);
    }
    let render = |r: &mut Running, maps: &Maps, call: &Call| {
        castle::render_stage(r, gobj_arg(maps, call), call.regs.gpr[4] as i32);
        Ret::Nothing
    };
    let setup = leaf_check::calls::Setup {
        gx: true,
        ..Default::default()
    };
    let t = leaf_check::sym("fn_801D0924");
    for pass in [0u32, 1, 2] {
        let set = move |c: &mut Call, k: usize| {
            cs_saved(c, k);
            c.addr = t;
            c.regs.gpr[4] = pass;
        };
        leaf_check::calls::check_calls_in(&d, from, 1, &set, &setup, &render);
    }
    let splash = |r: &mut Running, _: &Maps, call: &Call| {
        let at = call.regs.gpr[3];
        let pos = Vec3::new(f32_at(call, at), f32_at(call, at + 4), f32_at(call, at + 8));
        castle::splash(r, pos, call.regs.fpr[1].ps0 as f32);
        Ret::Nothing
    };
    let t = leaf_check::sym("grCastle_801D0A9C");
    let prep = move |c: &mut Call, k: usize| {
        c.addr = t;
        c.regs.fpr[1].ps0 = [0.5, 1.0, 2.0][k % 3];
        let at = leaf_check::sym("stage_info") + 0x730;
        poke_u32(c, at, (-40.0f32).to_bits());
        poke_u32(c, at + 4, 7.0f32.to_bits());
        poke_u32(c, at + 8, 0);
        c.regs.gpr[3] = at;
    };
    check_calls_with(&d, from, 1, &prep, &splash);
    let floor =
        |r: &mut Running, _: &Maps, _: &Call| Ret::Float(f64::from(castle::camera_floor(r.game)));
    let t = leaf_check::sym("grCastle_801D0FF0");
    check_calls_with(&d, from, 1, &retarget(t), &floor);
}
