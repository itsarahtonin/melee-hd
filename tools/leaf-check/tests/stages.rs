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
use melee_sim::game::code::{run_demo_init, run_event, run_proc};
use melee_sim::game::gr::oldyoshi;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
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
