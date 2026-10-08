//! Slippi replay playback (`game::playback`) and the Slippi ruleset's codes (`game::slippi`,
//! `game::ucf`) against ssbm-rs's ports made with the codes (`ssbm_game::playback`), from
//! captures of replay runs: each check imports a capture, runs our port and ssbm-rs's patched
//! port from the same state in both float modes, with the replay's Slippi device answering
//! the reference, and compares the whole state. Most calls are changed first, so the codes'
//! branches run from real states: the frame's recorded inputs, UCF's stick history, timers,
//! positions, costumes, stocks.
//!
//! The captures are game memory, kept out of the repository in `REPLAY_CALLS` (default
//! below), one directory per replay, named for it. ssbm-rs's runner (branch
//! `feat/state-trace-memory`) makes them, with `done.txt` listing every function address but
//! the captured ones:
//!
//! ```text
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$REPLAY_CALLS/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=Fighter_procInput,ftCo_Damage_OnEveryHitlag,Fighter_procAnim,fn_8016E2BC,\
//! fn_8016DCC0,fn_8016CFE0,fn_8016B918,HSD_PadRenewRawStatus,HSD_PadRenewMasterStatus,\
//! gm_EvaluateAllControllerInputs LOCKSTEP_DONE=done.txt ssbm-run "$SSBM_DISC" \
//!   --replay NAME.slp --fields 100000 --port all --lockstep --known known/NAME.txt
//! ```
//!
//! for the Fox vs Sheik Final Destination replay `Game_20260129T133652` and the four-player
//! team one `Game_20260531T174953`, both in `SLIPPI_REPLAYS` (default below).

use leaf_check::calls::{
    Ret, Setup, check_calls_in, gobj_arg, peek_u32, poke_bit, poke_f32, poke_u32, user_data_arg,
};
use melee_replay::Device;
use melee_sim::game::code::{run_fighter_cb, run_proc};
use melee_sim::game::handles::CodeRef;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::playback::{CHARACTER_LEN, Playback};
use melee_sim::game::slippi::placed;
use melee_sim::game::structs::StartMeleeData;
use ssbm_rt::capture::Call;
use ssbm_slippi::Replay;

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/replay";
const REPLAYS: &str = "C:/Users/USER/Documents/Slippi";
/// The captured replays, by the month folder they're in.
const CAPTURED: [(&str, &str); 2] = [
    ("2026-01", "Game_20260129T133652"),
    ("2026-05", "Game_20260531T174953"),
];

/// Each captured replay's directory and parsed replay, where both are here.
fn replays() -> Vec<(String, Replay)> {
    let calls = std::env::var("REPLAY_CALLS").unwrap_or_else(|_| CALLS.into());
    let slippi = std::env::var("SLIPPI_REPLAYS").unwrap_or_else(|_| REPLAYS.into());
    CAPTURED
        .iter()
        .filter_map(|(month, name)| {
            let bytes = std::fs::read(format!("{slippi}/{month}/{name}.slp")).ok()?;
            let dir = format!("{calls}/{name}");
            std::path::Path::new(&dir)
                .exists()
                .then(|| (dir, Replay::parse(&bytes).expect("a replay")))
        })
        .collect()
}

/// `lbl_804D6660`: playback's buffers, the first its frame response.
const BUFFERS: u32 = 0x804D_6660;
/// `mnCharSel_804D6CF4`'s word, where playback counts the replay's frame.
const FRAME_INDEX: u32 = 0x804D_6CF4;

/// Where the frame response holds `port`'s leader.
fn character(call: &Call, port: u32) -> u32 {
    peek_u32(call, peek_u32(call, BUFFERS)) + 7 + port * 2 * CHARACTER_LEN as u32
}

/// Where UCF's pad buffer keeps `port`'s history.
fn ucf_pad(call: &Call, port: u32) -> u32 {
    let site = melee_sim::game::slippi::UCF_PAD_BUFFER;
    placed(site, peek_u32(call, site)).expect("UCF's pad buffer") + 4 + 12 * port
}

fn poke_u8(call: &mut Call, addr: u32, v: u8) {
    let i = (addr - 0x8000_0000) as usize;
    call.mem1[i] = v;
}

/// A value from `values` picked by `n`, so calls cover each in turn.
fn pick<T: Copy>(values: &[T], n: usize) -> T {
    values[n % values.len()]
}

/// Fighter_procInput with the recorded inputs, UCF's pad buffer and cardinals, and the first
/// frame's settling: the frame's sticks (raw values at the cardinals' thresholds, processed
/// ones at the down-flick's), the fighter's stick history and down-flick count, and the frame
/// number changed, in several passes over the captures.
#[test]
fn recorded_inputs() {
    for (dir, replay) in replays() {
        for pass in 0..PASSES {
            check_calls_in(
                &dir,
                "Fighter_procInput",
                15,
                &|call, i| {
                    let n = i + 37 * pass;
                    let fp = user_data_arg(call);
                    // No action input callback: most aren't ported.
                    poke_u32(call, fp + 0x219C, 0);
                    let port = u32::from(call.mem1[(fp + 0xC - 0x8000_0000) as usize]);
                    let pad = u32::from(call.mem1[(fp + 0x618 - 0x8000_0000) as usize]);
                    if n % 6 == 0 {
                        return;
                    }
                    let c = character(call, port);
                    // Raw sticks around the cardinals' thresholds (80 along, 6 across).
                    let raw = [80i8, -80, 79, -79, 127, -128, 0, 6, 7, -6, -7, 44, -40];
                    // Processed sticks: some down past the flick's line, some whose distance
                    // in units of 1/80 lands either side of 80.
                    let sticks = [
                        (0.0f32, -1.0f32),
                        (0.13, -0.975),
                        (0.12, -0.975),
                        (0.0, -0.95),
                        (0.6, -0.7),
                        (0.0, -0.609_375),
                        (1.0, 0.0),
                        (0.3, 0.3),
                    ];
                    let (sx, sy) = pick(&sticks, n * 3 + n / 8);
                    poke_f32(call, c + 4, sx);
                    poke_f32(call, c + 8, sy);
                    let (cx, cy) = pick(&sticks, n * 5 + 1);
                    poke_f32(call, c + 0xC, cx);
                    poke_f32(call, c + 0x10, cy);
                    poke_u8(call, c + 0x2C, pick(&raw, n) as u8);
                    poke_u8(call, c + 0x2D, pick(&raw, n / 3 + 4) as u8);
                    poke_u8(call, c + 0x32, pick(&raw, n / 2 + 1) as u8);
                    poke_u8(call, c + 0x33, pick(&raw, n / 5 + 7) as u8);
                    // The stick held still last frame, so this frame's is a fresh flick.
                    if n % 3 != 2 {
                        poke_f32(call, fp + 0x620, 0.0);
                        poke_f32(call, fp + 0x624, 0.0);
                    }
                    let u = ucf_pad(call, pad);
                    let index = (n % 4) as u8;
                    poke_u8(call, u + 8, index);
                    poke_u8(call, u + 9, pick(&[0, 0, 0, 1, 5, 255], n / 2));
                    // The entry two before the one this frame writes, for the flick's travel:
                    // its y far from or near this frame's raw y, its x the other way round.
                    let before = u32::from(index.wrapping_sub(1) & 3);
                    let (bx, by) = pick(&[(0i8, -40i8), (90, 44), (-60, 0), (44, 127)], n / 4);
                    poke_u8(call, u + 2 * before, bx as u8);
                    poke_u8(call, u + 2 * before + 1, by as u8);
                    if n % 5 == 1 {
                        poke_u32(call, FRAME_INDEX, (-123i32) as u32);
                    }
                    if n % 7 == 2 {
                        // Zelda in action 349 keeps her sticks.
                        poke_u32(call, fp + 4, 0x13);
                        poke_u32(call, fp + 0x10, 349);
                    }
                },
                &Setup {
                    stubs: &[],
                    replay: Some(&replay),
                    gx: false,
                },
                &|g, maps, call| {
                    run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_AD10));
                    Ret::Nothing
                },
            );
        }
    }
}

/// How many passes the changed checks make over a function's captures, each changing them
/// differently.
const PASSES: usize = 6;

/// ftCo_Damage_OnEveryHitlag with UCF's SDI: the stick past the SDI threshold with its timers
/// past the window, the sticky timers, the stick's previous position and its raw travel
/// changed, in several passes over the captures.
#[test]
fn ucf_sdi() {
    for (dir, replay) in replays() {
        for pass in 0..2 * PASSES {
            check_calls_in(
                &dir,
                "ftCo_Damage_OnEveryHitlag",
                4,
                &|call, i| {
                    let n = i + 7 * pass;
                    let fp = user_data_arg(call);
                    let pad = u32::from(call.mem1[(fp + 0x618 - 0x8000_0000) as usize]);
                    poke_bit(call, fp + 0x221A, 2, true);
                    let big = [(0.9f32, 0.1f32), (-0.7, 0.7), (0.0, -1.0)];
                    let (x, y) = pick(&big, n);
                    poke_f32(call, fp + 0x620, x);
                    poke_f32(call, fp + 0x624, y);
                    // Last frame's stick: inside the threshold mostly, outside now and then.
                    let before = [(0.0f32, 0.0f32), (0.1, 0.1), (0.3, 0.0), (0.9, 0.0)];
                    let (px, py) = pick(&before, n / 3);
                    poke_f32(call, fp + 0x628, px);
                    poke_f32(call, fp + 0x62C, py);
                    // Both stick timers past the window, so the SDI is UCF's to allow.
                    let window = [254u8, 254, 8];
                    poke_u8(call, fp + 0x670, pick(&window, n));
                    poke_u8(call, fp + 0x671, pick(&window, n / 2));
                    let sticky = [(0u8, 0u8), (2, 0), (0, 2), (2, 2), (1, 5)];
                    let (sx, sy) = pick(&sticky, n / 4);
                    poke_u8(call, fp + 0x673, sx);
                    poke_u8(call, fp + 0x674, sy);
                    let u = ucf_pad(call, pad);
                    let index = u32::from(call.mem1[(u + 8 - 0x8000_0000) as usize] & 3);
                    let now = u + 2 * index;
                    let at = u + 2 * ((index + 2) & 3);
                    // This frame's raw stick and two frames ago's: travels either side of 62,
                    // along x, along y, or split.
                    let travel = [
                        (0i8, 0i8, 63i8, 0i8),
                        (0, 0, 0, 63),
                        (0, 0, 44, 44),
                        (0, 0, 43, 43),
                        (0, 0, 62, 0),
                    ];
                    let (nx, ny, bx, by) = pick(&travel, n / 2 + n);
                    poke_u8(call, now, nx as u8);
                    poke_u8(call, now + 1, ny as u8);
                    poke_u8(call, at, bx as u8);
                    poke_u8(call, at + 1, by as u8);
                },
                &Setup {
                    stubs: &[],
                    replay: Some(&replay),
                    gx: false,
                },
                &|g, maps, call| {
                    run_fighter_cb(g, gobj_arg(maps, call), CodeRef(0x8008_E4F0));
                    Ret::Nothing
                },
            );
        }
    }
}

/// Fighter_procAnim's off-screen damage from the stage's limits: the fighter moved past each
/// limit or inside them, with its off-screen count, percent, `x221F_b1` and action changed.
#[test]
fn offscreen_limits() {
    for (dir, replay) in replays() {
        check_calls_in(
            &dir,
            "Fighter_procAnim",
            8,
            &|call, i| {
                let fp = user_data_arg(call);
                let places = [
                    (0.0f32, 10.0f32),
                    (-500.0, 0.0),
                    (500.0, 0.0),
                    (0.0, 500.0),
                    (0.0, -500.0),
                ];
                let (x, y) = pick(&places, i);
                poke_f32(call, fp + 0xB0, x);
                poke_f32(call, fp + 0xB4, y);
                poke_f32(call, fp + 0x1830, pick(&[0.0f32, 30.0, 149.0], i / 2));
                poke_u32(call, fp + 0x1910, pick(&[0u32, 1, 30], i / 3));
                // The camera's off-screen damage switch.
                poke_f32(call, leaf_check::sym("game_camera") + 0x2BC, 1.0);
                if i % 6 == 4 {
                    poke_bit(call, fp + 0x221F, 1, true);
                }
                if i % 8 == 5 {
                    poke_u32(call, fp + 0x10, pick(&[4u32, 6], i));
                }
            },
            &Setup {
                stubs: &[],
                replay: Some(&replay),
                gx: false,
            },
            &|g, maps, call| {
                run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_A360));
                Ret::Nothing
            },
        );
    }
}

/// Pointers a replay's `StartMeleeData` holds aren't followed.
struct NoPointers;

impl Cx for NoPointers {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        None
    }
    fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
        None
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {}
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}
    fn mismatch(&mut self, _: &str, _: String, _: String) {}
}

/// fn_8016DCC0 with the general codes' costume rules: the players' characters, costumes and
/// shades changed so costumes run to and past their characters' counts and players repeat
/// each other, in many passes over the captures.
#[test]
fn costumes() {
    use melee_sim::game::playback::costume_count;
    for (dir, replay) in replays() {
        for pass in 0..4 * PASSES {
            check_calls_in(
                &dir,
                "fn_8016DCC0",
                1,
                &|call, _| {
                    let d = call.regs.gpr[3];
                    for s in 0..6u32 {
                        let p = d + 0x60 + 0x24 * s;
                        let n = pass * 7 + s as usize;
                        if s < 4 {
                            poke_u8(call, p + 1, pick(&[0, 0, 0, 1, 3], n));
                        }
                        // Pairs of slots share a character, so shades repeat.
                        let ckind = ((pass * 3 + (s as usize / 2) * 5) % 28) as u8;
                        poke_u8(call, p, ckind);
                        let count = costume_count(ckind);
                        let costume = count.wrapping_sub(pick(&[0u8, 1, 1, 2], n / 2 + pass));
                        poke_u8(call, p + 3, if pass % 8 == 7 { 0 } else { costume });
                        poke_u8(call, p + 7, pick(&[0u8, 0, 1, 2], n / 5));
                    }
                },
                &Setup {
                    stubs: &[],
                    replay: Some(&replay),
                    gx: false,
                },
                &|g, _, call| {
                    let mem = Mem {
                        bytes: &call.mem1,
                        base: 0x8000_0000,
                    };
                    let d = StartMeleeData::import(&mem, call.regs.gpr[3], &mut NoPointers);
                    g.game.init_vs(&d);
                    Ret::Nothing
                },
            );
        }
    }
}

/// The unported code the match's frame reaches, stood in for on both sides, as the VS checks
/// stand it in: the fighter's rebirth (`ftCo_800D4FF4`).
const FRAME_STUBS: [(u32, Option<u32>); 1] = [(0x800D_4FF4, None)];

/// fn_8016CFE0 run from the button maps' states (which come just before it in a tick), with
/// the frame counted from `index` (none: as captured) and read.
fn check_frames(index: impl Fn(&Replay) -> Option<i32>) {
    for (dir, replay) in replays() {
        let at = index(&replay);
        check_calls_in(
            &dir,
            "gm_EvaluateAllControllerInputs",
            3,
            &|call, _| {
                call.addr = leaf_check::sym("fn_8016CFE0");
                if let Some(at) = at {
                    poke_u32(call, FRAME_INDEX, at as u32);
                }
            },
            &Setup {
                stubs: &FRAME_STUBS,
                replay: Some(&replay),
                gx: false,
            },
            &|g, _, _| {
                let answers = Playback::answers(g.game, &mut Device(&replay));
                g.game.playback.as_mut().unwrap().answers = Some(answers);
                g.vs_frame();
                Ret::Nothing
            },
        );
    }
}

/// fn_8016CFE0's playback: the frame counted and read.
#[test]
fn frame_reads() {
    check_frames(|_| None);
}

/// fn_8016CFE0's playback past the replay's end: the match quits, with its end sounds and
/// music reset going through the audio driver.
#[test]
fn replay_end() {
    check_frames(|r| Some(*r.frames.keys().next_back().expect("frames")));
}

/// fn_8016B918's stock steals as the replay answers them: players out of stocks with their
/// fighters asleep, and teammates with stocks to give, in several passes over the captures.
/// The steal's own effects reach unported scoring, so every row's two flying icons are taken
/// and the HUD (`ifStock_802F7EFC`) answers that the steal shows already; whether each side
/// asks it says whether the replay's answer was read.
#[test]
fn stock_steals() {
    for (dir, replay) in replays() {
        for pass in 0..PASSES {
            check_calls_in(
                &dir,
                "fn_8016B918",
                3,
                &|call, i| {
                    let players = leaf_check::sym("player_slots");
                    for s in 0..4u32 {
                        let block = players + 0xE90 * s;
                        let out = (s as usize + i + pass).is_multiple_of(3);
                        poke_u8(call, block + 0x8E, if out { 0 } else { 3 });
                        // `ifStock_804A157C[s].anim[5..7]`: both flights taken.
                        for k in 5..7 {
                            poke_u8(call, 0x804A_157C + 0x54 * s + 5 + k, 1);
                        }
                        for e in 0..2 {
                            let gobj = peek_u32(call, block + 0xB0 + 4 * e);
                            if gobj != 0 && out {
                                let fp = peek_u32(call, gobj + 0x2C);
                                poke_bit(call, fp + 0x221F, 3, true);
                            }
                        }
                    }
                },
                &Setup {
                    stubs: &[],
                    replay: Some(&replay),
                    gx: false,
                },
                &|g, _, _| {
                    let answers = Playback::answers(g.game, &mut Device(&replay));
                    g.game.playback.as_mut().unwrap().answers = Some(answers);
                    g.share_stocks();
                    Ret::Nothing
                },
            );
        }
    }
}

/// Playback's per-frame process, which restores the frame's recorded seed: run where playback
/// placed it, with the frame's seed flag and seed changed.
#[test]
fn seed_restore() {
    for (dir, replay) in replays() {
        check_calls_in(
            &dir,
            "Fighter_procInput",
            15,
            &|call, i| {
                let site = 0x8016_E748;
                call.addr = placed(site, peek_u32(call, site)).expect("playback's code") + 0x4B0;
                let buf = peek_u32(call, BUFFERS);
                let at = peek_u32(call, buf);
                poke_u8(call, at + 2, (i % 3 != 0) as u8);
                poke_u32(call, at + 3, (i as u32).wrapping_mul(0x9E37_79B9));
            },
            &Setup {
                stubs: &[],
                replay: Some(&replay),
                gx: false,
            },
            &|g, _, _| {
                g.game.playback_restore_seed();
                Ret::Nothing
            },
        );
    }
}

/// `Settings::from_replay` against the game a replay run set up: at the match's setup
/// (`fn_8016DCC0`'s call), the `StartMeleeData` playback wrote, the game mode, the ruleset and
/// UCF's pad buffer must be what the replay's settings say.
#[test]
fn settings_from_game_start() {
    use melee_sim::game::vs_match::Settings;
    for (dir, replay) in replays() {
        let calls = leaf_check::calls::saved_calls(&dir, "fn_8016DCC0");
        let Some(path) = calls.first() else {
            eprintln!("no capture of fn_8016DCC0 in {dir}: skipping");
            continue;
        };
        let cap = melee_capture::Capture::read(path);
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: 0x8000_0000,
        };
        let s = Settings::from_replay(&melee_replay::start(&replay)).expect("known codes");
        let ours = format!("{:?}", s.start);
        let theirs = format!(
            "{:?}",
            StartMeleeData::import(&mem, cap.call.regs.gpr[3], &mut NoPointers)
        );
        assert_eq!(ours, theirs, "{dir}: the match's StartMeleeData");
        let mode = cap.call.mem1[(leaf_check::sym("state_machine") - 0x8000_0000) as usize];
        assert_eq!(s.game_mode, mode, "{dir}: the game mode");
        let read = |a: u32| peek_u32(&cap.call, a);
        let rules = melee_sim::game::state::Rules::from_memory(&read).expect("known codes");
        assert_eq!(s.rules, rules, "{dir}: the ruleset");
        assert_eq!(
            Some(s.ucf),
            melee_sim::game::ucf::Pads::from_memory(&read),
            "{dir}: UCF's pad buffer"
        );
    }
}

/// `gm_GetNumCostumesForCKind` for every character kind a byte holds.
#[test]
fn costume_counts() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    for ckind in 0..=255u8 {
        assert_eq!(
            melee_sim::game::playback::costume_count(ckind),
            ssbm_types::fns::gm_GetNumCostumesForCKind(&ctx, ckind),
            "character kind {ckind}"
        );
    }
}
