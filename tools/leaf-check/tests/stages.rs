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
use melee_sim::game::gr::{oldkongo, oldyoshi, shrine};
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
        let keep = if k % 2 == 0 {
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
