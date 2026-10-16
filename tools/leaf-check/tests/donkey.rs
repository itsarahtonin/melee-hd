//! Donkey Kong's code (`melee/ft/kinds/ftDonkey`) and the common code his moves reach: the
//! cargo carry and its throws (`ftCo_Cargo*.c`, `ftCo_09C4.c`), being carried
//! (`ftCo_Shouldered.c`), being buried (`ftCo_Bury.c`) and the throws his cargo throws run
//! (`ftCo_Throw.c`), against ssbm-rs's ports, from real game states: every saved call of one
//! of these functions is imported, run on both sides, and the whole state compared afterwards.
//! Each function also runs from the captures of its unit's other functions, with the fighter's
//! buttons, stick, script and state variables changed, so callbacks no capture reached (and
//! branches few did) run from real states of the same moves.
//!
//! The captures are game memory, kept out of the repository in `DONKEY_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `dk-funcs.txt` (`ADDR NAME`
//! per line) and `done-nondk.txt` (every other function's address). ssbm-rs's runner made them
//! on ranked Slippi replays of Donkey Kong from the `erickfm/melee-ranked-replays` dataset's
//! `DK_diamond-diamond_a6` archive (`local/replays/hf/dk-dd-a6/slp/`, named by their hashes;
//! docs/oracle.md), the eight the oracle traces, as the oracle traces them:
//!
//! ```text
//! D=local/calls/donkey
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/dk-funcs.txt LOCKSTEP_DONE=$D/done-nondk.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```
//!
//! What the code reaches in other parts of the port that isn't ported (the crowd and the
//! player library's statistics) stands in on both sides ([`stubs`]).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, poke_f32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const ENV: &str = "DONKEY";
const CALLS: &str = "calls/donkey";
const FUNCS: &str = "dk-funcs.txt";

fn var(what: &str) -> Option<String> {
    std::env::var(format!("{ENV}_{what}")).ok()
}

fn base() -> String {
    var("CALLS").unwrap_or_else(|| melee_local_data::ssbm_rs_local(CALLS))
}

/// `DONKEY_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'.
fn wanted(name: &str) -> bool {
    var("SRC").is_none_or(|o| o == name)
}

/// `DONKEY_TRACE` names what each neighbour call runs.
fn trace() -> bool {
    var("TRACE").is_some()
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

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/{FUNCS}", base())) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .collect()
}

/// Functions not run here: creation's and the spawn's (the oracle checks them), the item hooks
/// (no item-free match picks one up, and they're unaliased), the stage devices that bury
/// (`ftCo_800C0874`, `ftCo_800C08A0`, `ftCo_800C0C88`, not ported), the bury functions other
/// checks cover (`ftCo_800C09B4`, `ftCo_800C0A28`, `ftCo_800C0A98`, `ftCo_800C0B20`,
/// `ftCo_800C0CB8`), the bury effect's callback (`efLib_Cb_ftCo_Bury`, `leaf-check effects`)
/// and the follow it calls (`ftCo_800C0FCC`, which only the effects' update reaches; the
/// oracle's buried Falco checks it), the down special's second end (state 386, which no code
/// enters; `ledger/dead.txt`), and the helpers that run only inside their callers
/// ([`HELPERS`]).
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftDk_Init_OnLoad"
            | "ftDk_Init_OnDeath"
            | "ftDk_Init_OnItemPickup"
            | "ftDk_Init_OnItemInvisible"
            | "ftDk_Init_OnItemVisible"
            | "ftDk_Init_OnItemDrop"
            | "ftCo_800C0874"
            | "ftCo_800C08A0"
            | "ftCo_800C0C88"
            | "ftCo_800C09B4"
            | "ftCo_800C0A28"
            | "ftCo_800C0A98"
            | "ftCo_800C0B20"
            | "ftCo_800C0CB8"
            | "efLib_Cb_ftCo_Bury"
            | "ftCo_800C0FCC"
            | "ftDk_SpecialLwEnd1_Anim"
            | "ftDk_SpecialLwEnd1_IASA"
            | "ftDk_SpecialLwEnd1_Phys"
            | "ftDk_SpecialLwEnd1_Coll"
    ) || HELPERS.contains(&name)
}

/// The helpers whose ports are folded into their callers, which reach them in the clean
/// comparisons: the Headbutt's and the Hand Slap's static helpers.
const HELPERS: &[&str] = &[
    "doAirTransition",
    "doGroundTransition",
    "doAnim",
    "doTransition",
];

/// The functions that take more than a GObj, run from their own saved calls only.
const TAKES_MORE: &[&str] = &[
    "ftCo_8009B6C8",
    "ftCo_8009B9C8",
    "ftCo_8009C02C",
    "ftCo_8009C5A4",
    "ftCo_8009C640",
];

/// The code other parts of the port own, standing in on both sides: the crowd and the player
/// library's statistics.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598", "pl_8003E0E8", "pl_8003E114"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// Which unit a function is in: Donkey Kong's callbacks, down, side, neutral and up specials;
/// the cargo states, the shouldered states, the buried states; the throws and the rest.
fn unit(addr: u32) -> usize {
    match addr {
        0x8010_D740..0x8010_DC80 => 0,
        0x8010_DC80..0x8010_E1C4 => 1,
        0x8010_E1C4..0x8010_E574 => 2,
        0x8010_E574..0x8010_FB1C => 3,
        0x8010_FB1C..0x8011_0094 => 4,
        0x8009_B4D0..0x8009_C5A4 => 5,
        0x8009_C5A4..0x8009_CA30 => 6,
        0x800C_0874..0x800C_1600 => 7,
        _ => 8,
    }
}

const UNITS: usize = 9;

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// Runs the function `call.addr` as its signature takes it, its arguments from the saved
/// registers.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj: GObjId = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4] as i32;
    let f1 = call.regs.fpr[1].ps0;
    let b = |v: bool| Ret::Int(u32::from(v));
    // The attribute copy and the state changes other code calls directly.
    let entry: Option<fn(&mut Running, GObjId)> = match name.as_str() {
        "ftDk_Init_LoadSpecialAttrs" => Some(|g, o| g.dk_load_special_attrs(o)),
        "ftCo_8009B518" => Some(|g, o| g.enter_cargo_wait(o)),
        "ftCo_8009B56C" => Some(|g, o| g.enter_cargo_start(o)),
        "ftCo_8009B860" => Some(|g, o| g.enter_cargo_turn(o)),
        "ftCo_8009BB64" => Some(|g, o| g.enter_cargo_jump(o)),
        "ftCo_8009BC58" => Some(|g, o| g.enter_cargo_fall(o)),
        "ftCo_8009BD4C" => Some(|g, o| g.enter_cargo_landing(o)),
        "ftCo_8009C540" => Some(|g, o| g.enter_cargo_pass(o)),
        "ftCo_8009C170" => Some(|g, o| g.cargo_throw_to_air(o)),
        "ftCo_8009C45C" => Some(|g, o| g.cargo_throw_to_ground(o)),
        "ftCo_8009C744" => Some(|g, o| g.shouldered_break_free(o)),
        "ftCo_800C0D0C" => Some(|g, o| g.enter_bury(o)),
        "ftCo_800C124C" => Some(|g, o| g.enter_bury_wait(o)),
        "ftCo_800C13BC" => Some(|g, o| g.enter_bury_jump(o)),
        "ftCo_Jump_Phys_Inner" => Some(|g, o| g.co_cargo_jump_phys(o)),
        _ => None,
    };
    if let Some(f) = entry {
        f(g, gobj);
        return Ret::Nothing;
    }
    match name.as_str() {
        "ftCo_8009B4D0" => b(g.cargo_wait_check(gobj)),
        "ftCo_8009B67C" => b(g.cargo_walk_check(gobj)),
        "ftCo_8009B818" => b(g.cargo_turn_check(gobj)),
        "ftCo_8009BB1C" => b(g.cargo_jump_check(gobj)),
        "ftCo_8009BF3C" => b(g.cargo_throw_check(gobj)),
        "ftCo_8009C4F8" => b(g.cargo_pass_check(gobj)),
        "ft_8008A1FC" => b(g.stick_not_forward(gobj)),
        "ftCo_8009B6C8" => {
            g.enter_cargo_walk(gobj, f32::from_bits(gekko_fp::stfs(f1)));
            Ret::Nothing
        }
        "ftCo_8009B9C8" => {
            g.enter_cargo_knee_bend(gobj, r4);
            Ret::Nothing
        }
        "ftCo_8009C02C" => {
            g.enter_cargo_throw(gobj, r4);
            Ret::Nothing
        }
        "ftCo_8009C5A4" => {
            g.enter_shouldered(gobj, r4);
            Ret::Nothing
        }
        "ftCo_8009C640" => {
            g.enter_shouldered_start(gobj, r4);
            Ret::Nothing
        }
        // The rest take the GObj alone: state callbacks, accessories, hooks, and the state
        // changes other code calls by address.
        _ => {
            run_fighter_cb(g, gobj, CodeRef(call.addr));
            Ret::Nothing
        }
    }
}

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: the buttons (A, B, the shoulders), the stick pushed every way, the script's flags and
/// throw flags, the mash timer, and the state's and the character's variables.
///
/// `i` counts the runs and `visit` counts a target's runs, so each target sees every button.
fn vary(call: &mut Call, i: usize, visit: usize) {
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x100, 0x0][(i + visit) % 4];
    let pressed = [0x200u32, 0, 0x100, 0x8000_0000, 0][(i + visit) % 5];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let sticks = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.6, 0.0),
        (0.05, 0.02),
        (-0.95, -0.1),
    ];
    let (x, y) = sticks[i % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
    }
    // The script's throw flags: flag 3 (the Hand Slap's hitboxes) raised every other run.
    poke_u32(call, fp + 0x2210, [0, 0x1000_0000][i % 2]);
    let mv = fp + 0x2340;
    let w = |k: usize| (i / 2 + k) as u32;
    match unit(call.addr) {
        // The Giant Punch's swings stored.
        0 => poke_u32(call, fp + 0x222C, [0, 4, 10][w(0) as usize % 3]),
        // Another slap asked for: not yet when A or B is pressed (so the press's flag shows),
        // else already.
        1 => poke_u32(call, mv, u32::from(pressed & 0x300 == 0)),
        // The punch: a cancel asked for, the lunge's and the effect's phases, the swings let go
        // with, the stored swings.
        3 => {
            poke_u32(call, mv, w(0) % 2);
            poke_u32(call, mv + 4, w(1) % 3);
            poke_u32(call, mv + 8, w(2) % 3);
            poke_u32(call, mv + 0xC, w(3) % 11);
            poke_u32(call, fp + 0x222C, [0, 9, 10][w(0) as usize % 3]);
        }
        // The cargo states: the turn's turned flag and frames, the knee bend's and the
        // landing's lag.
        5 => {
            poke_u32(call, mv, w(0) % 2);
            poke_f32(call, mv + 4, [0.0, 1.0, 3.0][w(1) as usize % 3]);
            poke_f32(call, mv + 8, [0.0, 1.0, 2.0][w(2) as usize % 3]);
            poke_f32(call, mv + 0x10, [0.0, 1.0, 4.0][w(3) as usize % 3]);
        }
        // Carried: the mash's speed-up frames and the last mash, the mash timer near its end.
        6 => {
            poke_f32(call, mv, [0.0, 1.0, 5.0][w(0) as usize % 3]);
            poke_u32(call, mv + 4, w(1) % 2);
            poke_f32(call, fp + 0x1A4C, [0.5, 30.0, 90.0][w(2) as usize % 3]);
        }
        // Buried: the sinking's frames left, the mash timer near its end, the jump's frames.
        7 => {
            poke_u32(call, mv, [1, 2, 9][w(0) as usize % 3]);
            poke_f32(call, fp + 0x1A4C, [0.5, 30.0, 90.0][w(1) as usize % 3]);
        }
        _ => {}
    }
}

/// Clean comparisons by function run.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let s = stubs();
    let mut sets = Vec::new();
    for (_, name) in functions() {
        if skipped(&name) || !wanted(&name) {
            continue;
        }
        for dir in dirs() {
            let n = saved_calls(&dir, &name).len();
            if n > 0 {
                sets.push((name.clone(), dir, n));
            }
        }
    }
    let results = leaf_check::each_set(
        &sets,
        |s| s.2,
        |(name, dir, _)| {
            let prepare = |_: &mut Call, _: usize| {};
            let variants = [Variant {
                prepare: &prepare,
                ours: &run,
                stubs: &s,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Each function run from the captures of its unit's other functions, with the controls and
/// variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    let s = stubs();
    for u in 0..UNITS {
        let usable = |n: &str| !skipped(n) && !TAKES_MORE.contains(&n);
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit(*a) == u && usable(n))
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u || !wanted(src) || !usable(src) {
                continue;
            }
            for dir in dirs() {
                let n = saved_calls(&dir, src).len();
                if n > 0 {
                    runs.push((src.clone(), dir, n));
                }
            }
        }
        let total: usize = runs.iter().map(|r| r.2).sum();
        if total == 0 {
            continue;
        }
        // Enough rounds that every target runs from at least five states.
        let rounds = (5 * targets.len()).div_ceil(total).max(1);
        let targets = &targets;
        let jobs: Vec<(usize, &(String, String, usize))> = runs.iter().enumerate().collect();
        let results = leaf_check::each_set(
            &jobs,
            |j| j.1.2 * rounds,
            |&(j, (src, dir, _))| {
                let before: usize = runs[..j].iter().map(|r| r.2).sum();
                let shifts: Vec<usize> = (0..rounds).map(|round| round * total + before).collect();
                let prepares: Vec<Box<Prepare>> = shifts
                    .iter()
                    .map(|&shift| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if trace() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            vary(call, shift + i, (shift + i) / targets.len());
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run,
                        stubs: &s,
                    })
                    .collect();
                check_variants(dir, src, &variants)
            },
        );
        for reports in results.iter().flatten() {
            tally(reports, counts);
        }
    }
}

/// Functions no saved state can finish, with what stops them; one that compares clean fails
/// the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every one of the functions compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn donkey_functions() {
    let funcs = functions();
    if funcs.is_empty() {
        eprintln!("skipping: no function list in {}", base());
        return;
    }
    let mut counts = BTreeMap::new();
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(_, n)| !skipped(n)).collect();
    let missing: Vec<&str> = ours
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !BLOCKED.iter().any(|b| b.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        ours.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        ours.len()
    );
    if trace() {
        for (a, n) in &counts {
            eprintln!("{} compared clean {n} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    let src = var("SRC").is_some();
    assert!(
        src || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        src || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
