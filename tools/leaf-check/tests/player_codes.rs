//! The Gecko codes players add to their own setups (`game::player_codes`) against the
//! original machine code: each check applies a code to a saved call's memory (or the
//! reference's) as Slippi's playback applies a replay's, runs the function it patches as its
//! machine code in ssbm-rs's interpreter (no ssbm-rs port has the code) and our port with the
//! rules the importer reads from that memory, from the same state in both float modes, and
//! compares the whole state. Calls are changed first so the code's branches run.
//!
//! The saved calls are the ones other checks use: the fighter processes' (`fighter.rs`), the
//! second display set's (`display.rs`), the replay ones (`replay.rs`) and the audio library's
//! (`audio.rs`), all in ssbm-rs's `local/calls`.

use leaf_check::calls::{
    CODE_AREA, Ret, Setup, apply_code, check_calls_in, gobj_arg, poke_bit, poke_f32, poke_u32,
    user_data_arg,
};
use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{SCRATCH, handle, reference, sym};
use melee_local_data::{slippi_replays, ssbm_rs_local};
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::input::pad::PadStatus;
use ssbm_rt::capture::Call;

/// The fast-fall spark (`ftCommon_CheckFallFast`).
const FALL_FAST_SPARK: &[(u32, u32)] = &[
    (0xC207_D5A0, 0x0000_000E),
    (0x7F83_E378, 0x3880_03F3),
    (0x38A0_0003, 0x38C0_0000),
    (0x38E0_0000, 0x3821_FF94),
    (0x3901_0020, 0x3921_0014),
    (0x801F_002C, 0x2C00_0000),
    (0x4181_000C, 0x3C00_4100),
    (0x4800_0008, 0x3C00_C100),
    (0x9001_0020, 0x3800_0000),
    (0x9001_0014, 0x9001_0018),
    (0x9001_001C, 0x9001_0024),
    (0x9001_0028, 0x3DC0_8009),
    (0x61CE_F834, 0x7DC9_03A6),
    (0x4E80_0421, 0x3821_006C),
    (0x3860_0001, 0x0000_0000),
];

/// Invisible techs (`ftDrawCommon_800805C8`).
const INVISIBLE_TECHS: &[(u32, u32)] = &[
    (0xC208_0A30, 0x0000_0006),
    (0x801F_0010, 0x2800_00C7),
    (0x4180_0020, 0x2800_00C9),
    (0x4181_0018, 0xA01F_0894),
    (0x2800_40A0, 0x4180_000C),
    (0x2800_41A0, 0x4180_0008),
    (0x5460_CFFF, 0x0000_0000),
];

/// The SDI colors (`ftCo_Damage_OnEveryHitlag`) and their reset (`ftCo_Damage_OnExitHitlag`).
const SDI_COLORS: &[(u32, u32)] = &[
    (0xC208_E588, 0x0000_0018),
    (0x88A3_0505, 0x38A5_0001),
    (0x98A3_0505, 0x2C05_0001),
    (0x4182_0074, 0x2C05_0002),
    (0x4182_0038, 0x3C00_437F),
    (0x9003_04B8, 0x9003_04C0),
    (0x9003_04C4, 0x3800_0000),
    (0x9003_04BC, 0x9003_04C8),
    (0x9003_04CC, 0x9003_04D0),
    (0x9003_04D4, 0x3800_0080),
    (0x9803_0504, 0x4800_0068),
    (0x3C00_437F, 0x9003_04C0),
    (0x9003_04C4, 0x3800_0000),
    (0x9003_04B8, 0x9003_04BC),
    (0x9003_04C8, 0x9003_04CC),
    (0x9003_04D0, 0x9003_04D4),
    (0x3800_0080, 0x9803_0504),
    (0x4800_0034, 0x3C00_437F),
    (0x9003_04B8, 0x9003_04BC),
    (0x9003_04C4, 0x3800_0000),
    (0x9003_04C0, 0x9003_04C8),
    (0x9003_04CC, 0x9003_04D0),
    (0x9003_04D4, 0x3800_0080),
    (0x9803_0504, 0x8863_000C),
    (0x6000_0000, 0x0000_0000),
];
const SDI_COLORS_RESET: &[(u32, u32)] = &[
    (0xC208_E734, 0x0000_0002),
    (0x83FE_002C, 0x3800_0000),
    (0x981F_0505, 0x0000_0000),
];

/// The dash grab color (`Fighter_ChangeMotionState`).
const DASH_GRAB_COLOR: &[(u32, u32)] = &[
    (0xC206_A138, 0x0000_000F),
    (0x807A_0010, 0x2C03_00D6),
    (0x4182_0010, 0x2C03_00D7),
    (0x4182_0008, 0x4800_005C),
    (0x3C60_8048, 0x8063_9D30),
    (0x5463_443E, 0x2C03_0208),
    (0x4082_0018, 0x806D_B61C),
    (0x8863_0000, 0x881A_000C),
    (0x7C00_1800, 0x4082_0034),
    (0x3C00_437F, 0x901A_04B8),
    (0x901A_04C4, 0x3800_0000),
    (0x901A_04BC, 0x901A_04C0),
    (0x901A_04C8, 0x901A_04CC),
    (0x901A_04D0, 0x901A_04D4),
    (0x3800_0091, 0x981A_0504),
    (0x8079_000C, 0x0000_0000),
];

/// The quiet announcer (`fn_80024654`).
const QUIET_ANNOUNCER: &[(u32, u32)] = &[(0x0402_49A4, 0x3860_0001)];

/// The X/Z swap (`SPEC2_MakeStatus`).
const SWAP_XZ: &[(u32, u32)] = &[
    (0xC234_E2AC, 0x0000_0002),
    (0x5000_843E, 0x5000_B56A),
    (0x5000_56F6, 0x0000_0000),
];

fn fighter_calls() -> String {
    std::env::var("FIGHTER_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/fighter-fd1"))
}

fn display_calls() -> String {
    let calls = std::env::var("DISPLAY2_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/display2"));
    format!("{calls}/Game_20260129T133652")
}

fn audio_calls() -> String {
    std::env::var("AUDIO_LB_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/audio/lb"))
}

/// The replay captures (`replay.rs`), with their replays for the reference's device.
fn replay_calls() -> Vec<(String, ssbm_slippi::Replay)> {
    let calls = std::env::var("REPLAY_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/replay"));
    let slippi = std::env::var("SLIPPI_REPLAYS").unwrap_or_else(|_| slippi_replays(""));
    [
        ("2026-01", "Game_20260129T133652"),
        ("2026-05", "Game_20260531T174953"),
    ]
    .iter()
    .filter_map(|(month, name)| {
        let bytes = std::fs::read(format!("{slippi}/{month}/{name}.slp")).ok()?;
        let dir = format!("{calls}/{name}");
        std::path::Path::new(&dir)
            .exists()
            .then(|| (dir, ssbm_slippi::Replay::parse(&bytes).expect("a replay")))
    })
    .collect()
}

/// Applies `codes` to the call's memory, one after another in the code area.
fn apply(call: &mut Call, codes: &[&[(u32, u32)]]) {
    let mut at = CODE_AREA;
    for c in codes {
        at = apply_code(call, c, at);
    }
}

fn pick<T: Copy>(values: &[T], n: usize) -> T {
    values[n % values.len()]
}

fn poke_u8(call: &mut Call, addr: u32, v: u8) {
    call.mem1[(addr - 0x8000_0000) as usize] = v;
}

/// The conditions `ftCommon_CheckFallFast` tests, set so most calls start a fast fall.
fn fall_fast_state(call: &mut Call, fp: u32, n: usize) {
    poke_bit(call, fp + 0x221A, 4, n % 7 == 6);
    poke_f32(call, fp + 0x84, pick(&[-1.0f32, -0.25, 0.0, 2.0], n / 2));
    poke_f32(call, fp + 0x624, pick(&[-1.0f32, -0.8, -0.3], n / 3));
    poke_u8(call, fp + 0x671, pick(&[0u8, 1, 3, 254], n / 5));
    // Either facing, or a facing word that isn't a float's ±1.
    poke_f32(call, fp + 0x2C, pick(&[1.0f32, -1.0, 0.0], n));
}

/// The GObj in `r28`, where the fast-fall checks put the fighter's.
fn gobj_r28(maps: &melee_capture::import::Maps, call: &Call) -> melee_sim::hsd::gobj::GObjId {
    maps.gobjs[&call.regs.gpr[28]]
}

/// The fast-fall spark from the fighter processes' states: `ftCommon_CheckFallFast` called on
/// the fighter, with `r28` the GObj and `r31` the fighter as its callers leave them.
#[test]
fn fall_fast_spark() {
    let target = sym("ftCommon_CheckFallFast");
    check_calls_in(
        &fighter_calls(),
        "Fighter_procUpdate",
        20,
        &|call, i| {
            let gobj = call.regs.gpr[3];
            let fp = user_data_arg(call);
            call.addr = target;
            call.regs.gpr[3] = fp;
            call.regs.gpr[28] = gobj;
            call.regs.gpr[31] = fp;
            fall_fast_state(call, fp, i);
            apply(call, &[FALL_FAST_SPARK]);
        },
        &Setup {
            original: &[target],
            ..Setup::default()
        },
        &|g, maps, call| Ret::Int(u32::from(g.check_fall_fast(gobj_r28(maps, call)))),
    );
}

/// The same from `ftCo_FallSpecial_Phys`, whose `r31` is the fighter's attributes: the spark's
/// side comes from `run_animation_scaling`.
#[test]
fn fall_fast_spark_attrs() {
    let target = sym("ftCommon_CheckFallFast");
    check_calls_in(
        &fighter_calls(),
        "Fighter_procUpdate",
        20,
        &|call, i| {
            let gobj = call.regs.gpr[3];
            let fp = user_data_arg(call);
            call.addr = target;
            call.regs.gpr[3] = fp;
            call.regs.gpr[28] = gobj;
            call.regs.gpr[31] = fp + 0x110;
            fall_fast_state(call, fp, i);
            poke_f32(call, fp + 0x13C, pick(&[1.0f32, -2.0, 0.0], i / 2));
            apply(call, &[FALL_FAST_SPARK]);
        },
        &Setup {
            original: &[target],
            ..Setup::default()
        },
        &|g, maps, call| {
            Ret::Int(u32::from(
                g.check_fall_fast_from(gobj_r28(maps, call), true),
            ))
        },
    );
}

/// Invisible techs: the fighters' render callback with them teching or tech-rolling (or in
/// another state) on frames either side of the code's window.
#[test]
fn invisible_techs() {
    let target = sym("ftDrawCommon_800805C8");
    check_calls_in(
        &display_calls(),
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            call.regs.gpr[4] = (i % 3) as u32;
            let fp = user_data_arg(call);
            poke_u32(call, fp + 0x10, pick(&[199u32, 200, 201, 198, 202], i / 3));
            poke_f32(
                call,
                fp + 0x894,
                pick(&[5.0f32, 4.99, 12.0, 19.99, 20.0, -7.0], i / 2),
            );
            // The draw flags a drawn fighter sets the other way round, so a draw shows.
            poke_bit(call, fp + 0x2223, 2, true);
            poke_bit(call, fp + 0x2227, 7, false);
            poke_bit(call, fp + 0x2228, 0, true);
            apply(call, &[INVISIBLE_TECHS]);
        },
        &Setup {
            gx: true,
            original: &[target],
            ..Setup::default()
        },
        &|r, maps, call| {
            r.render_fighter(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// The SDI colors from the replays' hitlag states: a smash DI input (the stick far out, its
/// timers inside the window) with the count at each color and past the byte's wrap.
#[test]
fn sdi_colors() {
    let target = sym("ftCo_Damage_OnEveryHitlag");
    for (dir, replay) in replay_calls() {
        check_calls_in(
            &dir,
            "ftCo_Damage_OnEveryHitlag",
            4,
            &|call, i| {
                let fp = user_data_arg(call);
                poke_bit(call, fp + 0x221A, 2, i % 5 != 4);
                poke_f32(call, fp + 0x620, pick(&[0.9f32, -0.7, 0.0], i));
                poke_f32(call, fp + 0x624, pick(&[0.1f32, 0.7, -1.0], i));
                poke_u8(call, fp + 0x670, pick(&[0u8, 1, 254], i / 2));
                poke_u8(call, fp + 0x671, pick(&[0u8, 254], i / 3));
                poke_u8(call, fp + 0x505, pick(&[0u8, 1, 2, 3, 255], i / 2));
                apply(call, &[SDI_COLORS, SDI_COLORS_RESET]);
            },
            &Setup {
                replay: Some(&replay),
                original: &[target],
                ..Setup::default()
            },
            &|g, maps, call| {
                run_fighter_cb(g, gobj_arg(maps, call), CodeRef(0x8008_E4F0));
                Ret::Nothing
            },
        );
    }
}

/// Their reset as hitlag ends, from the same states.
#[test]
fn sdi_colors_reset() {
    let target = sym("ftCo_Damage_OnExitHitlag");
    for (dir, replay) in replay_calls() {
        check_calls_in(
            &dir,
            "ftCo_Damage_OnEveryHitlag",
            4,
            &|call, i| {
                let fp = user_data_arg(call);
                call.addr = target;
                poke_u8(call, fp + 0x505, pick(&[1u8, 2, 200], i));
                apply(call, &[SDI_COLORS, SDI_COLORS_RESET]);
            },
            &Setup {
                replay: Some(&replay),
                original: &[target],
                ..Setup::default()
            },
            &|g, maps, call| {
                run_fighter_cb(g, gobj_arg(maps, call), CodeRef(0x8008_E714));
                Ret::Nothing
            },
        );
    }
}

/// The dash grab color: `Fighter_ChangeMotionState` from procInput's states into the dash
/// grab, its pull, and states next to them.
#[test]
fn dash_grab_color() {
    let target = sym("Fighter_ChangeMotionState");
    const STATES: [u32; 5] = [214, 215, 212, 216, 14];
    check_calls_in(
        &fighter_calls(),
        "Fighter_procInput",
        10,
        &|call, i| {
            call.addr = target;
            call.regs.gpr[4] = STATES[i % STATES.len()];
            call.regs.gpr[5] = 0;
            float_arg(call, 1, 0.0);
            float_arg(call, 2, 1.0);
            float_arg(call, 3, 0.0);
            call.regs.gpr[6] = 0;
            apply(call, &[DASH_GRAB_COLOR]);
        },
        &Setup {
            original: &[target],
            ..Setup::default()
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            g.change_motion_state(gobj, call.regs.gpr[4] as i32, 0, 0.0, 1.0, 0.0, None);
            Ret::Nothing
        },
    );
}

fn float_arg(call: &mut Call, n: usize, v: f32) {
    let r = leaf_check::fp::lfs(v.to_bits());
    call.regs.fpr[n] = leaf_check::fp::Ps::new(r, r);
}

/// The quiet announcer: the audio library's tick, whose volumes (`fn_80024654`) read the
/// announcer's setting as 1, with the settings and the volumes last sent changed.
#[test]
fn quiet_announcer() {
    let target = sym("fn_80024654");
    check_calls_in(
        &audio_calls(),
        "lbAudioAx_80027DF8",
        10,
        &|call, i| {
            poke_u32(call, 0x804D_3884, pick(&[0x7Fu32, 0, 64, 1], i));
            poke_u32(call, 0x804D_63F4, pick(&[0u32, 2, 0x7F], i / 2));
            apply(call, &[QUIET_ANNOUNCER]);
        },
        &Setup {
            original: &[target],
            ..Setup::default()
        },
        &|g, _, _| {
            g.lb_audio_tick();
            Ret::Nothing
        },
    );
}

/// The X/Z swap: `SPEC2_MakeStatus` with the code, on response words with every button
/// pattern of X, Z and the error bits and random sticks, against [`PadStatus::decode_swapping_xz`].
#[test]
fn swap_xz() {
    let Some(ctx) = reference() else {
        return;
    };
    let site = 0x8034_E2AC;
    let body = CODE_AREA;
    for (i, &(x, y)) in SWAP_XZ[1..].iter().enumerate() {
        ctx.write_u32(body + 8 * i as u32, x);
        ctx.write_u32(body + 8 * i as u32 + 4, y);
    }
    let branch = |from: u32, to: u32| 0x4800_0000 | (to.wrapping_sub(from) & 0x03FF_FFFC);
    let last = body + 8 * 2 - 4;
    ctx.write_u32(last, branch(last, site + 4));
    ctx.write_u32(site, branch(site, body));
    ctx.set_backend(Box::new(ssbm_ppc::Interpreter::default()));
    let spec2 = sym("SPEC2_MakeStatus");
    ctx.set_mode(spec2, ssbm_rt::Mode::Original);
    let (pads, data_at, origin_at) = (SCRATCH, SCRATCH + 0x100, sym("Origin"));
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed as u32
    };
    for mode in [FpMode::Hardware, FpMode::Slippi] {
        set_fp_mode(mode);
        for n in 0..4096u32 {
            // The error bits, X and Z, every combination, over random words.
            let bits = (n & 1) << 30 | (n >> 1 & 1) << 31 | (n >> 2 & 1) << 26 | (n >> 3 & 1) << 20;
            let data = [next() & !0xC410_0000 | bits, next()];
            let origin = PadStatus {
                stick_x: (next() % 20) as i8 - 10,
                stick_y: (next() % 20) as i8 - 10,
                ..PadStatus::default()
            };
            for (k, b) in [
                origin.button.to_be_bytes()[0],
                origin.button.to_be_bytes()[1],
                origin.stick_x as u8,
                origin.stick_y as u8,
                origin.substick_x as u8,
                origin.substick_y as u8,
                origin.trigger_left,
                origin.trigger_right,
                origin.analog_a,
                origin.analog_b,
                origin.err as u8,
                0,
            ]
            .iter()
            .enumerate()
            {
                ctx.write_u8(origin_at + k as u32, *b);
            }
            ctx.write_u32(data_at, data[0]);
            ctx.write_u32(data_at + 4, data[1]);
            ctx.write_u8(pads + 10, (n % 3) as u8);
            ssbm_types::tu::dolphin__pad__pad::SPEC2_MakeStatus(
                &ctx,
                0,
                handle(&ctx, pads),
                handle(&ctx, data_at),
            );
            let mut ours = PadStatus {
                err: (n % 3) as i8,
                ..PadStatus::default()
            };
            ours.decode_swapping_xz(data, &origin);
            let theirs = PadStatus {
                button: ctx.read_u16(pads),
                stick_x: ctx.read_u8(pads + 2) as i8,
                stick_y: ctx.read_u8(pads + 3) as i8,
                substick_x: ctx.read_u8(pads + 4) as i8,
                substick_y: ctx.read_u8(pads + 5) as i8,
                trigger_left: ctx.read_u8(pads + 6),
                trigger_right: ctx.read_u8(pads + 7),
                analog_a: ctx.read_u8(pads + 8),
                analog_b: ctx.read_u8(pads + 9),
                err: ctx.read_u8(pads + 10) as i8,
            };
            assert_eq!(
                ours, theirs,
                "SPEC2_MakeStatus with the X/Z swap, {data:x?}"
            );
        }
    }
}
