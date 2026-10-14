//! Nana's computer-player frame (`ftCo_800B3900`, `game::ft::cpu_ai`) against the original's
//! machine code, from real states: every saved call is imported and run on both sides, and the
//! whole state compared afterwards. The reference runs the original's machine code throughout
//! ([`leaf_check::run_as_machine_code`]), not ssbm-rs's ports at the tag: with no knockback,
//! Nana's DI (`ftCo_800AC5A0`) takes her stick's x from a register the frame's calls leave
//! behind (r5), which those ports don't write, and melee-sim models what they leave
//! (`cpu_ai::Leftover`, docs/engine.md).
//!
//! Each call also runs changed so that her DI pushes the stick with no knockback: in a frame
//! where she DIs (behavior 18, in hitstun), her knockback speed is zeroed, her script finished,
//! Popo's percent raised to make her level 9 and her frame counter set so the level's chance
//! pushes. Then again with Popo hanging from a ledge, which leaves his state in r5 (253, so
//! her stick's x 0xFD) where she otherwise finds no held item (0).
//!
//! The captures are game memory, kept out of the repository in `ICS_CALLS` (default below),
//! one directory per run. ssbm-rs's runner (main, since its regflow fix; any build runs the
//! machine code the reference compares with) made them on the Ice Climbers' ranked replay
//! against Fox on Final Destination, `fd-cpu-a` from field 255 and `fd-cpu-b` from field
//! 1640, each saving 400 calls in a row:
//!
//! ```text
//! D=local/calls/ics
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$D/fd-cpu-a CAPTURE_FROM=255 CAPTURE_EVERY=1 \
//!   CAPTURE_FUNCS=ftCo_800B3900 LOCKSTEP_DONE=$D/done-noncpu.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/replays/hf/ics-dd-a5/diamond-diamond-c93e2e19a89a5d982c30bc2c.slp \
//!   --fields 700 --port all --lockstep
//! ```
//!
//! (`done-noncpu.txt`: every function's address but `ftCo_800B3900`'s, so the run checks only
//! it.)

use std::collections::BTreeSet;
use std::sync::Mutex;

use leaf_check::calls::{
    Report, Ret, Variant, check_variants, gobj_arg, peek_u32, poke_f32, poke_u32, saved_calls,
};
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/ics";

fn base() -> String {
    std::env::var("ICS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
}

/// The capture sets, one per run.
fn dirs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(base()) else {
        return Vec::new();
    };
    let mut v: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .collect();
    v.sort();
    v
}

/// `Fighter` offsets.
const KIND: u32 = 0x4;
const PLAYER: u32 = 0xC;
const MOTION: u32 = 0x10;
const KB_VEL: u32 = 0x8C;
const PERCENT: u32 = 0x1830;
const X221A: u32 = 0x221A;
/// `CpuFighter` (`Fighter::x1A88`) offsets.
const CPU: u32 = 0x1A88;
const BEHAVIOR: u32 = CPU + 0x18;
const FRAMES: u32 = CPU + 0x7C;
const SCRIPT: u32 = CPU + 0x44C;
const SCRIPT_WAIT: u32 = CPU + 0x450;
/// `Ft_Kind_Nana`, the DI behavior, `ftCo_MS_CliffWait`.
const KIND_NANA: u32 = 11;
const DI: u32 = 18;
const CLIFF_WAIT: u32 = 253;

/// The other fighter of Nana's player, Popo: the first of the fighters' GObjs
/// (`HSD_GObjPLinkHead[8]`) with her player that isn't her.
fn popo(call: &Call, nana: u32) -> Option<u32> {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 8 * 4);
    let slot = peek_u32(call, nana + PLAYER) >> 24;
    while g != 0 {
        let fp = peek_u32(call, g + 0x2C);
        if fp != nana && peek_u32(call, fp + PLAYER) >> 24 == slot {
            return Some(fp);
        }
        g = peek_u32(call, g + 0x8);
    }
    None
}

/// In a frame where Nana DIs, sets her up to push the stick with no knockback (and with
/// `hanging`, Popo hanging from a ledge): whether it did; other frames stay as saved.
fn zero_knockback(call: &mut Call, hanging: bool) -> bool {
    let nana = peek_u32(call, call.regs.gpr[3] + 0x2C);
    let hitstun = peek_u32(call, nana + X221A) >> 24 & 0x10 != 0;
    if peek_u32(call, nana + KIND) != KIND_NANA || peek_u32(call, nana + BEHAVIOR) != DI || !hitstun
    {
        return false;
    }
    let Some(popo) = popo(call, nana) else {
        return false;
    };
    poke_f32(call, nana + KB_VEL, 0.0);
    poke_f32(call, nana + KB_VEL + 4, 0.0);
    poke_u32(call, nana + SCRIPT, 0);
    poke_u32(call, nana + SCRIPT_WAIT, 0);
    // 60 % 120 is past 120 - 12 * 9: the level's chance pushes.
    poke_u32(call, nana + FRAMES, 60);
    poke_f32(call, popo + PERCENT, 180.0);
    if hanging {
        poke_u32(call, popo + MOTION, CLIFF_WAIT);
    }
    true
}

fn run_frame(
    r: &mut melee_sim::game::state::Running,
    maps: &melee_capture::import::Maps,
    call: &Call,
) -> Ret {
    r.cpu_frame(gobj_arg(maps, call));
    Ret::Nothing
}

/// The clean comparisons among `reports` of the calls at `changed` (indices into `paths`), or
/// of every call with `changed` empty.
fn compared(reports: &[Report], paths: &[std::path::PathBuf], changed: &BTreeSet<usize>) -> usize {
    reports
        .iter()
        .flat_map(|r| &r.clean)
        .filter(|(p, _)| {
            changed.is_empty()
                || paths
                    .iter()
                    .position(|q| q == p)
                    .is_some_and(|i| changed.contains(&i))
        })
        .count()
}

/// Every saved frame compares clean as saved, and the frames where Nana DIs compare clean
/// with the stick pushed by the registers her DI never set.
#[test]
fn nana_cpu_frame() {
    leaf_check::run_as_machine_code();
    let changed: [Mutex<BTreeSet<usize>>; 2] = Default::default();
    let none = |_: &mut Call, _: usize| {};
    let zero = |c: &mut Call, i: usize| {
        if zero_knockback(c, false) {
            changed[0].lock().unwrap().insert(i);
        }
    };
    let hanging = |c: &mut Call, i: usize| {
        if zero_knockback(c, true) {
            changed[1].lock().unwrap().insert(i);
        }
    };
    let variants = [
        Variant {
            prepare: &none,
            ours: &run_frame,
            stubs: &[],
        },
        Variant {
            prepare: &zero,
            ours: &run_frame,
            stubs: &[],
        },
        Variant {
            prepare: &hanging,
            ours: &run_frame,
            stubs: &[],
        },
    ];
    let mut totals = [0; 3];
    let dirs = dirs();
    for dir in &dirs {
        for c in &changed {
            c.lock().unwrap().clear();
        }
        let paths = saved_calls(dir, "ftCo_800B3900");
        let reports = check_variants(dir, "ftCo_800B3900", &variants);
        for (v, r) in reports.iter().enumerate() {
            totals[v] += match v {
                0 => compared(r, &paths, &BTreeSet::new()),
                _ => {
                    let c = changed[v - 1].lock().unwrap();
                    if c.is_empty() {
                        0
                    } else {
                        compared(r, &paths, &c)
                    }
                }
            };
        }
    }
    eprintln!("compared clean, as saved, with no knockback, and Popo hanging: {totals:?}");
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    assert!(
        totals.iter().all(|&t| t > 0),
        "a variant never compared: {totals:?}"
    );
}
