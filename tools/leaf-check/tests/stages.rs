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
use melee_sim::game::gr::{garden, kongo, oldkongo, oldyoshi, shrine, yorster};
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
    let prepares: [&Prepare; 17] = [
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
        melee_sim::game::gr::ground::fit_fog(r.game, r.data, c);
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
    let prepares: [&Prepare; 16] = [
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
