//! The items the items-on replays spawn (`melee/it/kinds`: the egg, parasol, Ray Gun,
//! Freezie, Super Scope and its shots, Fire Flower, Warp Star, Lip's Stick and Poké Ball), the
//! Super Scope's fighter states (`ftCo_ItemScope*.c`, `ft_0CDD.c`) and the item engine's
//! pieces they reach (an item's dynamic bones, a fighter stepping onto an item, the hold
//! flags, the scrolling stages' wind on items) against ssbm-rs's ports, from real game states:
//! every saved call of one of these functions is imported, run on both sides, and the whole
//! state compared afterwards. Each function also runs from the captures of its kind's (or
//! state's) other functions, the fighter's buttons, script flags and state variables changed,
//! so callbacks no capture reached (and branches few did) run from real states of the same
//! items.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_ON_CALLS` (default
//! below), one directory per run, beside the function lists the runs use: `items-on-funcs.txt`
//! (`ADDR NAME` per line) and `done-nonitemson.txt` (every other function's address).
//! ssbm-rs's runner (`ssbm-run-rf`) made them on the three ranked Slippi 3.16 replays of Dr.
//! Mario against Fox whose players' match-rules code turns items on in playback
//! (`doc-diamond-diamond-a6`: `2dd3ed50` on Pokémon Stadium, `339ac78e` on Yoshi's Story,
//! `86fbfe04` on Fountain of Dreams), as the oracle traces them:
//!
//! ```text
//! D=local/calls/items-on
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/hf-pc-HASH CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/items-on-funcs.txt LOCKSTEP_DONE=$D/done-nonitemson.txt \
//!   ssbm-run "$SSBM_DISC" --replay REPLAY.slp --fields 100000 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{
    egg, fflower, freeze, lgun, lipstick, parasol, sscope, sscopebeam, wstar,
};
use melee_sim::game::it::{self, CLASS_ITEM, item};
use melee_sim::game::state::Running;
use melee_sim::game::structs::{lb_80014638_arg0_t, lb_80014638_arg1_t};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-on";

fn base() -> String {
    std::env::var("ITEMS_ON_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/items-on-funcs.txt", base())) else {
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

/// The units whose functions run from each other's captures, by their address ranges: the
/// Super Scope's fighter states (with `ft_0CDD.c`), and each item kind.
const UNITS: [(u32, u32); 11] = [
    (0x800C_DD14, 0x800D_8990),
    (0x8028_8C88, 0x8028_96CC),
    (0x8028_B08C, 0x8028_B8D8),
    (0x8028_E738, 0x8028_EB88),
    (0x8028_EB88, 0x8028_F9D8),
    (0x8029_1BE0, 0x8029_2360),
    (0x8029_2D48, 0x8029_3660),
    (0x8029_4364, 0x8029_4AD0),
    (0x8029_570C, 0x8029_5C48),
    (0x8029_78DC, 0x8029_8168),
    (0x8029_8DEC, 0x8029_93E0),
];

/// Engine functions on an item's GObj that no capture reached, run from the Fire Flower's
/// (whose standing state calls them): the hold flags (`it_8026BDCC`, `it_8026BE28`) and the
/// hold mark (`it_80274F10`).
const ADOPTED: [(u32, usize); 3] = [(0x8026_BDCC, 6), (0x8026_BE28, 6), (0x8027_4F10, 6)];

/// Which unit a function is in, if any (the engine's other pieces are in none).
fn unit(addr: u32) -> Option<usize> {
    ADOPTED
        .iter()
        .find(|a| a.0 == addr)
        .map(|a| a.1)
        .or_else(|| UNITS.iter().position(|&(lo, hi)| (lo..hi).contains(&addr)))
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u == 0
}

/// Helpers that take what the check can't give (a fighter rather than its GObj, callbacks, a
/// vector to write): they run inside the callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    "fn_800D769C",
    "fn_800D79B4",
    "ftCo_800CDE94",
    "it_80291CCC",
    "grLib_801C9E60",
    "itKusudama_UnkMotion0_Anim",
];

/// The functions that take more than a GObj, run from their own saved calls only (and the
/// shots' entries from their neighbours' with a level).
const TAKES_MORE: &[&str] = &[
    "lb_80014638",
    "it_80291CF4",
    "it_80291D38",
    "it_80291DAC",
    "it_80291F14",
    "it_80291FA8",
    "it_80298DEC",
    "it_80298ED0",
    "it_8028B08C",
    "it_8028B618",
    "it_8028B648",
    "it_8028B6B0",
    "it_8028B718",
    "it_8028B780",
    "it_8028B7E8",
    "it_8028B850",
];

/// The shots' entries and the shot, which take a level in r4: their neighbours' captures run
/// them with one.
const LEVEL_R4: &[&str] = &["fn_800D8140", "fn_800D81D0", "fn_800D84D4"];

/// Item functions taking a number in r4 or f1, run from their kind's captures with one: a
/// level (the scope's spending), a state (the parasol's lengths) or a speed (its steps).
const ITEM_ARGS: &[&str] = &[
    "it_80291F14",
    "it_8028B08C",
    "it_8028B618",
    "it_8028B648",
    "it_8028B6B0",
    "it_8028B718",
    "it_8028B780",
    "it_8028B7E8",
    "it_8028B850",
];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

type Event = fn(&mut Running, GObjId);
type Pred = fn(&mut Running, GObjId) -> bool;

/// The item functions no dispatch reaches by address (they're passed as callbacks), by name.
const ITEM_EVENTS: &[(&str, Event)] = &[
    ("it_80288E6C", egg::landed),
    ("it_80289094", egg::explode),
    ("it_80289158", egg::vanish),
    ("it_8028E7D8", lgun::landed),
    ("it_8028E938", lgun::shot),
    ("it_8028EDBC", freeze::slide),
    ("it_8028EF34", freeze::landed),
    ("it_8028F7C8", freeze::stopped),
    ("it_80292030", sscope::landed),
    ("it_80292E64", fflower::start_shooting),
    ("it_80292EAC", fflower::stop_shooting),
    ("it_80292FF0", fflower::landed),
    ("it_8029455C", wstar::landed),
    ("it_802957AC", lipstick::landed),
    ("Item_80268560", it::item::init_dynamics),
    ("Item_8026A788", it::item::proc_dynamics),
    ("Item_8026A8EC", it::item::destroy),
];

const ITEM_PREDS: &[(&str, Pred)] = &[("it_80288DC4", egg::break_open)];

fn item_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item", name_of(a));
    }
}

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]).
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    if name == "lb_80014638" {
        let (p, q) = (r(3), r(4));
        let feet = lb_80014638_arg0_t {
            x0: vec_at(p),
            xC: vec_at(p + 12),
        };
        let top = lb_80014638_arg1_t {
            unk_x: f32_at(call, q),
            unk_y: f32_at(call, q + 4),
            x8: vec_at(q + 8),
        };
        return b(melee_sim::lb::stomp::stepped_on(&feet, &top));
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let other = |i: usize| {
        *maps
            .gobjs
            .get(&r(i))
            .unwrap_or_else(|| panic!("{SKIP}{name} takes a GObj not imported"))
    };
    if let Some((_, f)) = ITEM_EVENTS.iter().find(|(n, _)| *n == name) {
        item_check(g, gobj, a);
        f(g, gobj);
        return Ret::Nothing;
    }
    if let Some((_, f)) = ITEM_PREDS.iter().find(|(n, _)| *n == name) {
        item_check(g, gobj, a);
        return b(f(g, gobj));
    }
    let level = r(4) as i32;
    match name.as_str() {
        "it_80291CF4" => return Ret::Int(sscope::charge_level(g, gobj, level) as u32),
        "it_80291D38" => return Ret::Int(sscope::cost(g, gobj, level) as u32),
        "it_80291DAC" => return Ret::Int(sscope::affordable(g, gobj, level) as u32),
        "it_80291F14" => sscope::spend(g, gobj, level),
        "it_80291FA8" => sscope::shoot(g, gobj, vec_at(r(4)), r(5) as i32, f1()),
        "it_80298DEC" => sscopebeam::fire(g, gobj, vec_at(r(4)), r(5) as i32, f1()),
        "it_80298ED0" => sscopebeam::launch(g, gobj, other(4)),
        "it_8028B08C" => return Ret::Int(parasol::anim_frames(level) as u32),
        "it_8028B618" => parasol::open_step(g, gobj, f1(), 4),
        "it_8028B648" => parasol::open_step(g, gobj, f1(), 5),
        "it_8028B6B0" => parasol::open_step(g, gobj, f1(), 6),
        "it_8028B718" => parasol::open_step(g, gobj, f1(), 7),
        "it_8028B780" => parasol::open_step(g, gobj, f1(), 8),
        "it_8028B7E8" => parasol::open_step(g, gobj, f1(), 9),
        "it_8028B850" => parasol::open_step(g, gobj, f1(), 10),
        "it_8026BDCC" => it::lib26b1::hold_flags_off(g.game, gobj),
        "it_8026BE28" => it::lib26b1::hold_flags_on(g.game, gobj),
        "it_80274F10" => it::lib2725::hold_still(g.game, gobj),
        "ft_80081A00" => return b(g.stepped_on_item(gobj)),
        "ftCo_Attack_800CDD14" => g.enter_item_shoot(gobj),
        "ftCo_800CDDA0" => g.enter_item_shoot_air(gobj),
        "ftCo_800CDE54" => g.co_item_shoot_phys(gobj),
        "ftCo_800CDE74" => g.co_item_shoot_air_phys(gobj),
        "ft_800D76B8" => g.enter_scope_start(gobj),
        "ft_800D7770" => g.enter_scope_air_start(gobj),
        "fn_800D7830" => g.scope_start_land(gobj),
        "fn_800D78B0" => g.scope_start_air(gobj),
        "fn_800D7BDC" => g.enter_scope_rapid(gobj),
        "fn_800D7C60" => g.enter_scope_air_rapid(gobj),
        "fn_800D7CEC" => g.scope_rapid_land(gobj),
        "fn_800D7D70" => g.scope_rapid_air(gobj),
        "fn_800D8140" => g.enter_scope_fire(gobj, level),
        "fn_800D81D0" => g.enter_scope_air_fire(gobj, level),
        "fn_800D8268" => g.scope_fire_land(gobj),
        "fn_800D82EC" => g.scope_fire_air(gobj),
        "fn_800D84D4" => g.scope_shot(gobj, level),
        "fn_800D86E0" => g.enter_scope_end(gobj),
        "fn_800D874C" => g.enter_scope_air_end(gobj),
        "fn_800D87C0" => g.scope_end_land(gobj),
        "fn_800D8838" => g.scope_end_air(gobj),
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

fn f32_at(call: &Call, p: u32) -> f32 {
    f32::from_bits(peek_u32(call, p))
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if unit(a).is_some_and(fighter_unit) {
        if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
            panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
        }
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    item_check(g, gobj, a);
    if let Some(v) = it::kinds::run_pred(g, gobj, a) {
        return Ret::Int(u32::from(v));
    }
    if it::kinds::run_event(g, gobj, a) {
        return Ret::Nothing;
    }
    panic!("{SKIP}{} takes more than its GObj", name_of(a));
}

/// The fighter's controls, script flags and state variables changed so the scope's branches
/// run: A held, pressed or neither, the script's throw flag 0, the charge's ticks around the
/// level boundaries and the charge that fires on its own, whether A was let go, and a burst's
/// shots left.
fn vary(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x100, 0x100, 0][i % 4];
    let pressed = [0x100u32, 0, 0][(i / 3) % 3];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    poke_bit(call, fp + 0x2210, 0, i % 3 != 2);
    let mv = fp + 0x2340;
    let k = i / 4;
    match peek_u32(call, fp + 0x10) {
        // Charging: the charge's ticks, and whether A was let go.
        158 | 162 | 166 | 170 => {
            let ticks = [0.0, 3.0, 7.0, 8.0, 15.0, 40.0, 59.0, 60.0, 100.0][k % 9];
            poke_f32(call, mv, ticks);
            poke_u32(call, mv + 4, (k as u32 / 9) & 1);
        }
        // Bursting: the shots left.
        159 | 163 | 167 | 171 => poke_u32(call, mv, [1u32, 2, 5][k % 3]),
        // The charged shot: its level, sometimes none.
        160 | 164 | 168 | 172 => poke_u32(call, mv, [0u32, 3, 9, 10, u32::MAX][k % 5]),
        _ => {}
    }
}

/// The items' variations: none (their states come from the captures as they are).
fn vary_item(_: &mut Call, _: usize) {}

/// Clean comparisons by function run.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

fn wanted(name: &str) -> bool {
    std::env::var("ITEMS_ON_SRC").map_or(true, |o| o == name)
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

/// Saved calls that aren't states of the unit's other functions: a spawn callback runs as the
/// item is made, before its kind writes its variables.
const NOT_SOURCES: &[&str] = &[];

/// Each function run from the captures of its unit's other functions.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..UNITS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                unit(*a) == Some(u)
                    && !skipped(n)
                    && (!TAKES_MORE.contains(&n.as_str())
                        || LEVEL_R4.contains(&n.as_str())
                        || ITEM_ARGS.contains(&n.as_str()))
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != Some(u) || !wanted(src) || NOT_SOURCES.contains(&src.as_str()) {
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
        let rounds = targets.len().div_ceil(total).max(2);
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
                            if std::env::var_os("ITEMS_ON_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                                if LEVEL_R4.contains(&n.as_str()) {
                                    call.regs.gpr[4] = [0u32, 1, 5, 9, 10][(shift + i) % 5];
                                }
                            } else {
                                vary_item(call, shift + i);
                                if ITEM_ARGS.contains(&n.as_str()) {
                                    let k = shift + i;
                                    // A level, or one of the held parasol's states (4
                                    // to 10, the ones with an animation).
                                    call.regs.gpr[4] = if n == "it_80291F14" {
                                        (k % 10) as u32
                                    } else {
                                        4 + (k % 7) as u32
                                    };
                                    let speed = [1.0f32, 0.5, 2.0, 0.75][k % 4];
                                    call.regs.fpr[1].ps0 = f64::from(speed);
                                }
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
fn items_on_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_ON_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `ITEMS_ON_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("ITEMS_ON_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_ON_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
    let _ = item;
}
