//! The Poké Ball's Pokémon (milestone 4's task 10: `it_279C.c`'s Pokémon part and the
//! Pokémon units in `melee/it/kinds`), against ssbm-rs's ports, from real game states: every
//! saved call of one of these functions is imported, run on both sides, and the whole state
//! compared afterwards. Each function in a unit also runs from the captures of its unit's other
//! functions, so callbacks no capture reached (and branches few did) run from real states of
//! the same Pokémon.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_POKEMON_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-pokemon-funcs.txt` (`ADDR NAME` per line) and `done-nonpokemon.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the six
//! `items-pokeball` oracle matches (`$SSBM_RS_LOCAL/bench/oracle-m4`), as the oracle traces
//! them:
//!
//! ```text
//! D=local/calls/items-pokemon
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-items-pokeballM CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-pokemon-funcs.txt LOCKSTEP_DONE=$D/done-nonpokemon.txt \
//!   ssbm-run "$SSBM_DISC" --replay items-pokeballM.slp --fp hardware --fields 7440 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, saved_calls,
};
use melee_capture::import::Maps;
use melee_sim::game::it::kinds::kireihana;
use melee_sim::game::it::{self, CLASS_ITEM, pokemon};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-pokemon";

fn base() -> String {
    std::env::var("ITEMS_POKEMON_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The crowd and the player library's statistics, standing in on both sides.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598", "pl_8003E0E8", "pl_8003E114"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// The functions by address and name, from the list in the capture directory: those of the
/// units ported (`UNITS`); the Pokémon no oracle match releases (Articuno, Raikou, Suicune,
/// Ditto, Mew, Celebi and Staryu) aren't.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/items-pokemon-funcs.txt", base())) else {
        return Vec::new();
    };
    let mut v: Vec<(u32, String)> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .filter(|(a, _)| unit(*a).is_some())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The units whose functions run from each other's captures, by their address ranges: the
/// Pokémon's common code, then each Pokémon.
const UNITS: &[(u32, u32)] = &[(0x8027_9C48, 0x8027_B288), (0x802D_007C, 0x802D_09D0)];

/// Which unit a function is in, if any (the others run from their own saved calls only).
fn unit(addr: u32) -> Option<usize> {
    UNITS.iter().position(|&(lo, hi)| (lo..hi).contains(&addr))
}

/// A neighbour's call made ready for `target`, its `k`th: the arguments those taking more than
/// their GObj take.
fn args(call: &mut Call, target: &str, k: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    let gobj = call.regs.gpr[3];
    if !in_mem(gobj) {
        return;
    }
    let g = &mut call.regs.gpr;
    match target {
        "itKireihana_Logic15_EvtUnk" => g[4] = if k.is_multiple_of(2) { gobj } else { 0 },
        "it_80279CDC" | "it_80279D5C" => call.regs.fpr[1].ps0 = [1.0f64, 0.5, 1.3][k % 3],
        _ => {}
    }
}

/// Helpers that take what the check can't give: they run inside the callers that call them,
/// whose comparisons cover them.
const HELPERS: &[&str] = &[];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

type Event = fn(&mut Running, GObjId);

/// The item functions no dispatch reaches by address (they're passed as callbacks), by name.
const ITEM_EVENTS: &[(&str, Event)] = &[
    ("it_802D0138", kireihana::landed),
    ("it_802D01A0", kireihana::landed_dancing),
    ("it_802D03F8", kireihana::fall_dancing),
    ("it_802D05D8", kireihana::leave),
    ("it_802D062C", kireihana::landed_leaving),
    ("it_802D0774", kireihana::fall_leaving),
    ("it_802D07C0", kireihana::fall_leaving_kept),
    ("it_802D08F0", kireihana::enter_air),
    ("it_80279C48", pokemon::face_nearest_opponent),
    ("it_80279E24", pokemon::pulse),
    ("it_80279FF8", pokemon::grow),
    ("it_80279D38", pokemon::launch),
    ("it_8027B564", pokemon::count_attack),
];

/// The callbacks the common landing (`it_8027A118`) is handed, by address.
fn event_at(a: u32) -> Option<Event> {
    let name = name_of(a);
    ITEM_EVENTS.iter().find(|(n, _)| *n == name).map(|e| e.1)
}

fn item_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item", name_of(a));
    }
}

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f = |i: usize| call.regs.fpr[i].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    // The ones taking an `Item*`.
    let item_gobj = |i: usize| {
        let id = *maps
            .items
            .get(&r(i))
            .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
        g.game.items[id.0].gobj.expect("an item without its GObj")
    };
    match name.as_str() {
        "it_8027A364" => {
            let gobj = item_gobj(3);
            return Ret::Int(pokemon::total_weight(g, gobj) as u32);
        }
        "it_8027A4D4" => {
            let gobj = item_gobj(3);
            return Ret::Int(pokemon::draw(g, gobj) as u32);
        }
        "it_8027A160" => {
            let gobj = item_gobj(4);
            let bone = if r(3) == 0 {
                None
            } else {
                Some(
                    *maps
                        .jobjs
                        .get(&r(3))
                        .unwrap_or_else(|| panic!("{SKIP}{name} takes a joint not imported")),
                )
            };
            pokemon::take_bone_motion(g, bone, gobj);
            return Ret::Nothing;
        }
        "it_8027ADEC" => {
            let gobj = *maps
                .gobjs
                .get(&r(4))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a GObj not imported"));
            let j = maps.jobjs.get(&r(5)).copied();
            pokemon::effect_scaled(g, r(3) as i32, gobj, j, f(1));
            return Ret::Nothing;
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let maybe = |i: usize| maps.gobjs.get(&r(i)).copied();
    item_check(g, gobj, a);
    if let Some((_, ev)) = ITEM_EVENTS.iter().find(|(n, _)| *n == name) {
        ev(g, gobj);
        return Ret::Nothing;
    }
    match name.as_str() {
        "it_8027AB64" => return b(pokemon::release(g, gobj)),
        "it_8027AAA0" => {
            let id = *maps
                .items
                .get(&r(4))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
            let other = g.game.items[id.0].gobj.expect("an item without its GObj");
            pokemon::set_release_vars(g, gobj, other, r(5) as i32);
        }
        "it_80279CDC" => pokemon::start(g, gobj, f(1)),
        "it_80279D5C" => pokemon::set_size(g, gobj, f(1)),
        "it_8027A09C" => return b(pokemon::fall(g, gobj)),
        "it_8027A118" => {
            let landed =
                event_at(r(4)).unwrap_or_else(|| panic!("{SKIP}{name} lands into {:#x}", r(4)));
            return b(pokemon::land(g, gobj, landed));
        }
        "it_8027A13C" => pokemon::clear_motion(g.game, gobj),
        "it_8027A344" => pokemon::move_by_bone(g.game, gobj),
        "it_8027AE34" => return b(pokemon::bounce_off_wall(g, gobj)),
        "it_8027AF50" => return b(pokemon::bounce_off_surface(g, gobj)),
        "itKireihana_Logic15_EvtUnk" => kireihana::evt_unk(g, gobj, maybe(4)),
        _ => return run_item(g, gobj, a, &name),
    }
    Ret::Nothing
}

/// The item callbacks that take only their GObj, through the items' dispatch.
fn run_item(g: &mut Running, gobj: GObjId, a: u32, name: &str) -> Ret {
    if let Some(v) = it::kinds::run_pred(g, gobj, a) {
        return Ret::Int(u32::from(v));
    }
    if it::kinds::run_event(g, gobj, a) {
        return Ret::Nothing;
    }
    panic!("{SKIP}{name} takes more than its GObj");
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
    std::env::var("ITEMS_POKEMON_SRC").map_or(true, |o| o == name)
}

fn skipped(name: &str) -> bool {
    HELPERS.contains(&name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
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
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Each function run from the captures of its unit's other functions.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..UNITS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit(*a) == Some(u) && !skipped(n))
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != Some(u) || !wanted(src) {
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
        let mut rounds = targets.len().div_ceil(total).max(2);
        // A small unit runs every target from every call.
        if total * targets.len() <= 400 {
            rounds = rounds.max(targets.len());
        }
        let no = stubs();
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
                            args(call, n, shift / total + i);
                            if std::env::var_os("ITEMS_POKEMON_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run,
                        stubs: &no,
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

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn items_pokemon_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_POKEMON_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_POKEMON_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
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
    if std::env::var_os("ITEMS_POKEMON_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_POKEMON_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
