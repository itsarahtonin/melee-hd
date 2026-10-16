//! Jigglypuff's own code (`melee/ft/kinds/ftPurin`) against ssbm-rs's ports, from real game
//! states: every saved call of a Jigglypuff callback is imported, run on both sides, and the
//! whole state compared afterwards. Each callback also runs from the captures of its unit's
//! other functions, with the fighter's buttons, stick, script and state variables changed, so
//! callbacks no capture reached (and branches few did) run from real states of the same moves.
//!
//! The functions that take more than the GObj run from their own saved calls, with the
//! arguments saved ([`TAKES_MORE`]); her item hooks, which no capture reaches, aren't run
//! ([`UNCAPTURED`]). What Jigglypuff's code reaches in other parts of the port that stands in for
//! unported code (an item's hold kind, the hold poses) stands in on both sides ([`stubs`]);
//! each check runs twice, with the stand-ins that answer a question answering no, then yes.
//!
//! The captures are game memory, kept out of the repository in `PURIN_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `purin-funcs.txt` (`ADDR
//! NAME` per line, the five Jigglypuff units' functions) and `done-nonpurin.txt` (every other
//! function's address). ssbm-rs's runner made them (with `CAPTURE_INTERACTED`, which saves
//! calls that touch hardware) on her two replays against Sheik, Pokémon Stadium's and Final
//! Destination's (names off, the known divergences of `local/replays/known-names0/`):
//!
//! ```text
//! D=local/calls/purin
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/purin-funcs.txt LOCKSTEP_DONE=$D/done-nonpurin.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/NAME.slp --fields 100000 --port all --lockstep \
//!   --known local/replays/known-names0/NAME.txt
//! ```
//!
//! for `Game_20260129T131206` and `Game_20260524T180613`. Her grounded specials and most of
//! Rollout's states are rare there, so `script1` is a debug VS match of two Jigglypuffs on Final
//! Destination whose first port a script drives through them (`MONKEY_HOLD`: Start every ten
//! fields from 30, then from field 400 every 150 fields one of: B held 70 fields then the stick
//! back; B briefly then the stick forward; a jump then B held; B forward, up or down; a jump
//! then B down, up, or forward twice; six jumps), the monkey playing on between:
//!
//! ```text
//! MATCH_FIGHTERS=15 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 MONKEY_HOLD=... (the same CAPTURE_ variables, CAPTURE=$D/script1) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::ft::fox::ext as fox_ext;
use melee_sim::game::state::Running;
use melee_sim::mtx::Mtx34;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/purin";

fn base() -> String {
    std::env::var("PURIN_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The code other parts of the port own, standing in on both sides: the stand-ins that answer
/// a question answer `yes`.
fn stubs(yes: bool) -> Vec<Stub> {
    let mut v: Vec<Stub> = [
        fox_ext::FTANIM_80070FB4,
        fox_ext::FTANIM_80070C48,
        fox_ext::FTANIM_80070CC4,
    ]
    .iter()
    .map(|&a| (a, StubRet::Nothing))
    .collect();
    // An item's hold kind: the one-handed pose, or none.
    v.push((
        fox_ext::IT_GET_HOLD_KIND,
        StubRet::Int(if yes { 3 } else { 0 }),
    ));
    // The common states' crowd and statistics, which their ports stand in for.
    for n in ["un_80322598", "pl_8003E0E8", "pl_8003E114"] {
        v.push((leaf_check::sym(n), StubRet::Nothing));
    }
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// Jigglypuff's functions by name, from the unit lists in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/purin-funcs.txt", base())) else {
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

/// Functions no saved call reaches: her item hooks (she never picks up or drops an item in
/// the captured runs).
const UNCAPTURED: &[&str] = &["ftPr_Init_OnItemPickup", "ftPr_Init_OnItemDrop"];

/// Functions not run here: fighter creation's, spawn's and removal's (the oracle checks them),
/// the scene's reset of the hats' joints, and those no saved call reaches.
fn skipped(name: &str, _addr: u32) -> bool {
    matches!(
        name,
        "ftPr_Init_OnLoad"
            | "ftPr_Init_OnDeath"
            | "ftPr_Init_LoadSpecialAttrs"
            | "ftPr_Init_8013C2F8"
            | "ftPr_Init_OnUserDataRemove"
            | "ftPr_Init_8013C494"
            | "ftPr_Init_UnkMotionStates6"
    ) || UNCAPTURED.contains(&name)
}

/// The units, by their first function's address: ftpurin, ftpurinspecialhi, ftpurinspeciallw,
/// ftpurinspecials and ftpurinspecialn.
const STARTS: [u32; 5] = [
    0x8013_C2F8,
    0x8013_C9C8,
    0x8013_CE8C,
    0x8013_D234,
    0x8013_D658,
];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments).
const TAKES_MORE: &[&str] = &[
    "ftPr_Init_UnkIntBoolFunc0",
    "ftPr_Init_UnkMtxFunc0",
    "ftPr_SpecialS_8013DA24",
    "ftPr_SpecialS_8013DD54",
];

/// Runs Jigglypuff's function `call.addr` on her GObj, with the arguments it was saved with
/// for those that take more ([`TAKES_MORE`]).
fn run_purin(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let [r3, r4, r5] = [3, 4, 5].map(|i| call.regs.gpr[i]);
    if name_of(a) == "ftPr_Init_UnkIntBoolFunc0" {
        let Some(&f) = maps.fighters.get(&r3) else {
            panic!("{SKIP}ftPr_Init_UnkIntBoolFunc0 takes something else than a fighter");
        };
        g.purin_hat_vis(f, r4 as usize, r5 != 0);
        return Ret::Nothing;
    }
    if !maps.gobjs.contains_key(&r3) {
        panic!("{SKIP}{} takes something else than a GObj", name_of(a));
    }
    let gobj = gobj_arg(maps, call);
    match name_of(a).as_str() {
        "ftPr_Init_8013C360" => g.purin_load_hat(gobj),
        "ftPr_SpecialS_8013DD54" => g.purin_roll_tick(gobj, r4 != 0),
        "ftPr_SpecialS_8013DA24" => {
            g.purin_roll_end(gobj, r4 != 0, r5, call.regs.fpr[1].ps0 as f32);
        }
        "ftPr_Init_UnkMtxFunc0" => {
            let vmtx = (r5 != 0).then(|| Mtx34 {
                m: std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        f32::from_bits(peek_u32(call, r5 + 4 * (4 * i + j) as u32))
                    })
                }),
            });
            g.purin_draw_hat(gobj, vmtx, r4 as i32);
        }
        _ => {
            if melee_sim::game::ft::purin::run(g, gobj, a) {
                return Ret::Nothing;
            }
            panic!("{SKIP}{} takes more than its GObj", name_of(a));
        }
    }
    Ret::Nothing
}

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: B held or not, a shield button pressed, the stick pushed every way and flicked, the
/// script's flags, and the state's counters near where they turn.
fn vary(call: &mut Call, i: usize) {
    // Calls whose first argument isn't a GObj (a fighter, a matrix) are skipped when run.
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x200, 0x0][i % 4];
    let pressed = [0x8000_0000u32, 0, 0][i % 3];
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
    ];
    let (x, y) = sticks[i % sticks.len()];
    let (px, py) = sticks[(i / 2 + 3) % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    poke_f32(call, fp + 0x628, px);
    poke_f32(call, fp + 0x62C, py);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
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

fn wanted(name: &str) -> bool {
    std::env::var("PURIN_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each of Jigglypuff's functions, run as it was saved, with the stand-ins
/// answering no and yes.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let (no, yes) = (stubs(false), stubs(true));
    let mut sets = Vec::new();
    for (addr, name) in functions() {
        if skipped(&name, addr) || !wanted(&name) {
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
            let variants: Vec<Variant> = [&no, &yes]
                .into_iter()
                .map(|stubs| Variant {
                    prepare: &prepare,
                    ours: &run_purin,
                    stubs,
                })
                .collect();
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Each of Jigglypuff's functions run from the captures of its unit's other functions, with the
/// controls and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..STARTS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit(*a) == u && !skipped(n, *a) && !TAKES_MORE.contains(&n.as_str()))
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u || !wanted(src) || skipped(src, *a) {
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
        let rounds = targets.len().div_ceil(total).max(1);
        // Each set of saved calls runs every round's targets, with the stand-ins answering no
        // and yes, in one pass ([`check_variants`]): run `r`'s calls start `offset` targets
        // further on, `offset` counting the calls of the runs before.
        let (no, yes_stubs) = (stubs(false), stubs(true));
        let targets = &targets;
        let jobs: Vec<(usize, &(String, String, usize))> = runs.iter().enumerate().collect();
        let results = leaf_check::each_set(
            &jobs,
            |j| j.1.2 * rounds,
            |&(j, (src, dir, _))| {
                let before: usize = runs[..j].iter().map(|r| r.2).sum();
                let shifts: Vec<(usize, bool)> = (0..rounds)
                    .flat_map(|round| {
                        [false, true].map(|yes| {
                            let shift =
                                round * total + before + usize::from(yes) * targets.len() / 2;
                            (shift, yes)
                        })
                    })
                    .collect();
                let prepares: Vec<Box<Prepare>> = shifts
                    .iter()
                    .map(|&(shift, yes)| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if std::env::var_os("PURIN_TRACE").is_some() {
                                eprintln!("{src} #{i} (yes {yes}) runs {n}");
                            }
                            vary(call, shift + i);
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = shifts
                    .iter()
                    .zip(&prepares)
                    .map(|(&(_, yes), prepare)| Variant {
                        prepare: &**prepare,
                        ours: &run_purin,
                        stubs: if yes { &yes_stubs } else { &no },
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

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every one of Jigglypuff's callbacks compares clean from some real state: its own saved calls or
/// its unit's neighbours'.
#[test]
fn purin_functions() {
    let mut counts = BTreeMap::new();
    // `PURIN_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `PURIN_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(a, n)| !skipped(n, *a)).collect();
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
    if std::env::var_os("PURIN_TRACE").is_some() {
        for (a, c) in &counts {
            eprintln!("{} compared clean {c} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    for (n, why) in BLOCKED {
        eprintln!("{n} is blocked: {why}");
    }
    let one = std::env::var_os("PURIN_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
