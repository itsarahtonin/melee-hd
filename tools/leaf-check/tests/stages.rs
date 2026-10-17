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
        } else if i % 2 == 0 {
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
