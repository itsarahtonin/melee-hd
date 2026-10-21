//! Zelda's own code (`melee/ft/kinds/ftZelda`) against ssbm-rs's ports, from real game states:
//! every saved call of a Zelda function is imported, run on both sides, and the whole state
//! compared afterwards. Each function also runs from the captures of its unit's other
//! functions, with the fighter's buttons, stick, script and state variables changed, so
//! callbacks no capture reached (and branches few did) run from real states of the same moves.
//!
//! What Zelda's code reaches in other parts of the port that isn't ported (Fox's animation
//! helpers) stands in on both sides ([`stubs`]); each check runs twice, with the stand-ins that
//! answer a question answering no, then yes, and the calls each side made to them are compared
//! with their arguments.
//!
//! The captures are game memory, kept out of the repository in `ZELDA_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `zelda-funcs.txt` (`ADDR
//! NAME` per line, the five Zelda units' and the two Din's Fire units' functions) and
//! `done-nonzelda.txt` (every other function's address). ssbm-rs's runner made them (with
//! `CAPTURE_INTERACTED`, which saves calls that touch hardware: the states' entries load
//! animations) on the Fox against Sheik replay on Final Destination, where Sheik turns into
//! Zelda twice:
//!
//! ```text
//! D=local/calls/zelda
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/zelda-funcs.txt LOCKSTEP_DONE=$D/done-nonzelda.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/2026-01/NAME.slp --fields 100000 --port all \
//!   --lockstep --known local/replays/known/NAME.txt
//! ```
//!
//! for `Game_20260129T133652`, then the same for `Game_20260129T170722`, `Game_20260129T165132`,
//! `Game_20260129T170952` and `Game_20260129T133041`. No replay has Din's Fire, so `script1` is
//! a debug VS match of two Zeldas on Final Destination whose first port a script drives
//! through the specials but the transformation (`MONKEY_HOLD`: Start every ten fields from 30,
//! then from field 400 every 150 fields one of: B with the stick held right for 70 fields, B
//! left for 20, a jump then B right and up, B left then left and down, B alone for 20, B up for
//! 20), the monkey playing on after:
//!
//! ```text
//! MATCH_FIGHTERS=18 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 MONKEY_HOLD=... (the same CAPTURE_ variables, CAPTURE=$D/script1) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```
//!
//! No replay has Zelda holding an item either, so `items1` is a debug VS match of two Zeldas on
//! Final Destination (the first a CPU) with only the home-run bat and the beam sword spawning,
//! the monkey playing the second, saving every call of the item hooks and the knockback hooks
//! (`zeldaitem-funcs.txt`, with `done-nonzeldaitem.txt` every other function's address): the
//! pickups, the held item hidden and shown again, and the drops:
//!
//! ```text
//! MATCH_FIGHTERS=18 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MATCH_ITEMS=11,12 \
//!   MONKEY_B=2 STALL_BEATS=200 CAPTURE_INTERACTED=1 CAPTURE=$D/items1 CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_EVERY=1 CAPTURE_FUNCS=@$D/zeldaitem-funcs.txt \
//!   LOCKSTEP_DONE=$D/done-nonzeldaitem.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 21 --monkey 21 --fields 9000 --port all --lockstep
//! ```
//!
//! The transformation with an item held or under a power-up (`zelda_transform_items`) has its own
//! captures, in `ZELDA_TRANSFORM_CALLS` (default `calls/zelda-transform`), with
//! `zelda-transform-funcs.txt` (the transformation, `ftCommon_8007EFC8`; the items handed over,
//! `it_8026B9A8`; the Starman's color animation copied, `ftCo_800C0358`; the worn items' size,
//! `ftCommon_80080174`) and `done-zelda-transform.txt` beside them, every call saved from the
//! `fighters` oracle set's Zelda and Sheik matches (`$SSBM_RS_LOCAL/bench/oracle-m4`,
//! docs/oracle.md's "Fighters with items"):
//!
//! ```text
//! D=local/calls/zelda-transform
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 //!   CAPTURE_EVERY=1 CAPTURE_FUNCS=@$D/zelda-transform-funcs.txt LOCKSTEP_DONE=$D/done-zelda-transform.txt //!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fp hardware --fields 8700 --port all --lockstep
//! ```

use melee_local_data::ssbm_rs_local;
use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32, poke_bit,
    poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::fox::ext as fox_ext;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it;
use melee_sim::game::it::kinds::{zeldadinfire, zeldadinfireexplode};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/zelda";

fn base() -> String {
    std::env::var("ZELDA_CALLS").unwrap_or_else(|_| ssbm_rs_local(CALLS))
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

/// Zelda's functions by name, from the unit lists in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/zelda-funcs.txt", base())) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .filter(|(a, _)| PORTED_UNITS.contains(&unit(*a)))
        .collect()
}

/// Functions not run here: fighter creation's (the oracle checks them), the dress's removal
/// (`ftZd_Init_801395C8`, not ported), and the fire's owner getter, whose GObj return the
/// check can't compare (the hold's callbacks read it).
fn skipped(name: &str, _addr: u32) -> bool {
    matches!(
        name,
        "ftZd_Init_OnLoad"
            | "ftZd_Init_OnDeath"
            | "ftZd_Init_LoadSpecialAttrs"
            | "ftZd_Init_801395C8"
            | "itZeldaDinFire_GetOwner"
    )
}

/// The units, by their first function's address: ftzelda, ftzeldaspecialhi, ftzeldaspecialn,
/// ftzeldaspeciallw, ftzeldaspecials, itzeldadinfire and itzeldadinfireexplode.
const STARTS: [u32; 7] = [
    0x8013_92DC,
    0x8013_96AC,
    0x8013_A830,
    0x8013_ADB4,
    0x8013_B638,
    0x802C_3AF0,
    0x802C_4580,
];

/// The units ported so far: all.
const PORTED_UNITS: [usize; 7] = [0, 1, 2, 3, 4, 5, 6];

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 4
}

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

/// A vector argument, the three floats at `addr` in the call's memory.
fn vec_at(call: &Call, addr: u32) -> Vec3 {
    let f = |o: u32| f32::from_bits(peek_u32(call, addr + o));
    Vec3::new(f(0), f(4), f(8))
}

/// A float argument in `f1` or `f2`, as single precision.
fn fpr(call: &Call, i: usize) -> f32 {
    call.regs.fpr[i].ps0 as f32
}

/// The GObj in register `r` (none for null).
fn gobj_in(maps: &Maps, call: &Call, r: usize) -> Option<GObjId> {
    let a = call.regs.gpr[r];
    (a != 0).then(|| *maps.gobjs.get(&a).expect("not a GObj the import knows"))
}

/// Runs Zelda's function `call.addr` as its signature takes it.
fn run_zelda(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let r = |i: usize| call.regs.gpr[3 + i];
    let b = |v: bool| Ret::Int(u32::from(v));
    match name.as_str() {
        "ftZd_Init_OnItemPickup" => {
            g.zelda_on_item_pickup(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftZd_Init_OnItemDrop" => {
            g.zelda_on_item_drop(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftZd_SpecialLw_8013B540" => b(g.zelda_din_fire_held(gobj_arg(maps, call))),
        "ftZd_SpecialLw_8013B574" => b(g.zelda_din_fire_released(gobj_arg(maps, call))),
        "it_802C3AFC" => b(zeldadinfire::hit_stage(g, gobj_arg(maps, call))),
        "it_802C3BAC" => {
            let pos = vec_at(call, r(1));
            zeldadinfire::spawn(g, gobj_arg(maps, call), pos, fpr(call, 1));
            Ret::Nothing
        }
        "it_802C3D74" => {
            zeldadinfire::start(g, gobj_arg(maps, call));
            Ret::Nothing
        }
        "it_802C3D44" => {
            zeldadinfire::let_go(g, gobj_in(maps, call, 3));
            Ret::Nothing
        }
        "itZeldaDinFire_Logic65_EvtUnk" => {
            zeldadinfire::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        "it_802C4580" => {
            let pos = vec_at(call, r(2));
            let fire = gobj_in(maps, call, 4).expect("a null fire");
            zeldadinfireexplode::spawn(
                g,
                gobj_arg(maps, call),
                fire,
                pos,
                fpr(call, 1),
                fpr(call, 2),
            );
            Ret::Nothing
        }
        "it_802C46C4" => {
            zeldadinfireexplode::start(g, gobj_arg(maps, call));
            Ret::Nothing
        }
        "itZeldaDinFireExplode_Logic66_EvtUnk" => {
            zeldadinfireexplode::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        _ => {
            let gobj = gobj_arg(maps, call);
            if fighter_unit(unit(call.addr)) {
                run_fighter_cb(g, gobj, CodeRef(call.addr));
                return Ret::Nothing;
            }
            if let Some(v) = melee_sim::game::it::kinds::run_pred(g, gobj, call.addr) {
                return b(v);
            }
            assert!(
                melee_sim::game::it::kinds::run_event(g, gobj, call.addr),
                "{name} isn't an item callback the port knows"
            );
            Ret::Nothing
        }
    }
}

/// Sets up a call of `name` whose arguments aren't only the GObj: the item hooks' flag, the
/// fire's collision data, and from a neighbour's state an item's owner.
fn arguments(call: &mut Call, name: &str, i: usize, neighbour: bool) {
    match name {
        "ftZd_Init_OnItemPickup" | "ftZd_Init_OnItemDrop" => {
            call.regs.gpr[4] = (i % 2) as u32;
        }
        "it_802C3AFC" if neighbour => call.regs.gpr[4] = user_data_arg(call) + 0x378,
        "itZeldaDinFire_Logic65_EvtUnk" | "itZeldaDinFireExplode_Logic66_EvtUnk" if neighbour => {
            call.regs.gpr[4] = peek_u32(call, user_data_arg(call) + 0x518);
        }
        "it_802C46C4" if neighbour => call.regs.gpr[4] = user_data_arg(call),
        _ => {}
    }
}

/// Functions that can't run from a neighbour's state: the items' spawns, which take their
/// parent's arguments (they run as saved).
const NOT_FROM_NEIGHBOURS: &[&str] = &["it_802C3BAC", "it_802C4580"];

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: B held or not, a shield button pressed, the stick pushed every way and flicked, the
/// script's flags, and the state's counters near where they turn.
fn vary(call: &mut Call, i: usize) {
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
    let mv = fp + 0x2340;
    let w = |k: usize| (i / 2 + k) as u32;
    match unit(call.addr) {
        // Farore's Wind: the travel's countdown and frames.
        1 => {
            poke_u32(call, mv, w(0) % 3);
            poke_u32(call, mv + 0xC, w(1) * 4 % 24);
        }
        // Nayru's Love: the reflector's script flag, the hover's frames.
        2 => {
            poke_u32(call, fp + 0x2200, w(0) % 3);
            poke_u32(call, mv, w(1) % 3);
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

fn wanted(name: &str) -> bool {
    std::env::var("ZELDA_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each of Zelda's functions, run as it was saved, with the stand-ins
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
            let prepare = |call: &mut Call, i: usize| arguments(call, name, i, false);
            let variants: Vec<Variant> = [&no, &yes]
                .into_iter()
                .map(|stubs| Variant {
                    prepare: &prepare,
                    ours: &run_zelda,
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

/// Each of Zelda's functions run from the captures of its unit's other functions, with the
/// controls and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in PORTED_UNITS {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                unit(*a) == u && !skipped(n, *a) && !NOT_FROM_NEIGHBOURS.contains(&n.as_str())
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u
                || !wanted(src)
                || skipped(src, *a)
                || NOT_FROM_NEIGHBOURS.contains(&src.as_str())
            {
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
                            if std::env::var_os("ZELDA_TRACE").is_some() {
                                eprintln!("{src} #{i} (yes {yes}) runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                            }
                            arguments(call, n, i, true);
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = shifts
                    .iter()
                    .zip(&prepares)
                    .map(|(&(_, yes), prepare)| Variant {
                        prepare: &**prepare,
                        ours: &run_zelda,
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

/// Every one of Zelda's ported functions compares clean from some real state: its own saved
/// calls or its unit's neighbours'.
#[test]
fn zelda_functions() {
    let mut counts = BTreeMap::new();
    // `ZELDA_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `ZELDA_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("ZELDA_TRACE").is_some() {
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
    let one = std::env::var_os("ZELDA_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// The transformation's captures with items: `ZELDA_TRANSFORM_CALLS` (default below), one
/// directory per oracle match, with `zelda-transform-funcs.txt` and `done-zelda-transform.txt`
/// beside them (module docs).
const TRANSFORM_CALLS: &str = "calls/zelda-transform";

fn transform_base() -> String {
    std::env::var("ZELDA_TRANSFORM_CALLS").unwrap_or_else(|_| ssbm_rs_local(TRANSFORM_CALLS))
}

fn transform_dirs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(transform_base()) else {
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

/// The functions the transformation's captures save.
const TRANSFORM_FUNCS: [&str; 4] = [
    "ftCommon_8007EFC8",
    "it_8026B9A8",
    "ftCo_800C0358",
    "ftCommon_80080174",
];

/// Runs one of [`TRANSFORM_FUNCS`] as its signature takes it.
fn run_transform(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let r = |i: usize| call.regs.gpr[i];
    let fighter = |g: &Running, p: u32| -> GObjId {
        let f = *maps.fighters.get(&p).expect("a fighter the import knows");
        g.game.fighters[f.0]
            .gobj
            .expect("a fighter without its GObj")
    };
    match name.as_str() {
        "ftCommon_8007EFC8" => {
            let gobj = gobj_arg(maps, call);
            g.transform(gobj, CodeRef(r(4)));
        }
        "it_8026B9A8" => {
            let item = gobj_in(maps, call, 3).expect("an item");
            let holder = gobj_in(maps, call, 4).expect("a holder");
            it::lib26b1::hand_over(g, item, holder, r(5) as i32);
        }
        "ftCo_800C0358" => {
            let (src, dst) = (fighter(g, r(3)), fighter(g, r(4)));
            g.copy_color_anim(src, dst, r(5) as i32);
        }
        "ftCommon_80080174" => {
            let gobj = fighter(g, r(3));
            g.scale_worn_items(gobj);
        }
        _ => unreachable!("{name} isn't the transformation's"),
    }
    Ret::Nothing
}

/// `Fighter`'s flags and timers the transformation carries over: the Starman (`x221D` bit 6,
/// its frames `x2004`), metal (`x2223` bit 7, `metal_timer`, `metal_health`), the Cloaking
/// Device (`x2226` bits 4 and 7, its frames `x2030`).
const STAR: (u32, u32) = (0x221D, 6);
const STAR_FRAMES: u32 = 0x2004;
const METAL: (u32, u32) = (0x2223, 7);
const METAL_TIMER: u32 = 0x2028;
const METAL_HEALTH: u32 = 0x202C;
const CLOAK: (u32, u32) = (0x2226, 4);
const CLOAK_ENDING: (u32, u32) = (0x2226, 7);
const CLOAK_FRAMES: u32 = 0x2030;
/// The flower's growth, the damage that grew it.
const FLOWER_GROWN: u32 = 0x2024;

/// A transformation's source fighter given what `case` names on top of what it has: nothing,
/// the Starman's invincibility, metal, the Cloaking Device, the Cloaking Device flashing toward
/// its end, and all three at once; or (case 6) the worn items' size (`ftCommon_80080174`) run
/// on it instead, with the flower or Bunny Hood it has, which its own saved calls (a
/// transformation's, before the items change hands) never have.
fn transform_case(call: &mut Call, case: usize) {
    let fp = user_data_arg(call);
    if case == 6 {
        call.addr = leaf_check::sym(TRANSFORM_FUNCS[3]);
        call.regs.gpr[3] = fp;
        // A flower's growth (`x2024`) moved on, so its size changes rather than staying as it
        // was set.
        let grown = peek_u32(call, fp + FLOWER_GROWN);
        poke_u32(call, fp + FLOWER_GROWN, grown + 7);
        return;
    }
    let star = matches!(case, 1 | 5);
    let metal = matches!(case, 2 | 5);
    let cloak = (3..=5).contains(&case);
    if star {
        poke_bit(call, fp + STAR.0, STAR.1, true);
        poke_u32(call, fp + STAR_FRAMES, 300);
    }
    if metal {
        poke_bit(call, fp + METAL.0, METAL.1, true);
        poke_u32(call, fp + METAL_TIMER, 400);
        poke_u32(call, fp + METAL_HEALTH, 50);
    }
    if cloak {
        poke_bit(call, fp + CLOAK.0, CLOAK.1, true);
        poke_bit(call, fp + CLOAK_ENDING.0, CLOAK_ENDING.1, case == 4);
        poke_u32(call, fp + CLOAK_FRAMES, 500);
        // The cloak counts among refraction's users (`lbRefSetUse`), as a real one would: a
        // flashing one isn't passed on, so the source's end takes its use back.
        let users = leaf_check::sym("lbl_804336D0");
        let n = peek_u32(call, users);
        poke_u32(call, users, n + 1);
    }
}

/// The transformation with an item held or under a power-up compares clean from real states:
/// each of [`TRANSFORM_FUNCS`]' saved calls as saved, and each transformation again with the
/// Starman, metal and the Cloaking Device (steady and flashing) given to the fighter it starts
/// from, which the matches' transformations have only some of, and its worn items sized
/// ([`transform_case`]).
#[test]
fn zelda_transform_items() {
    let dirs = transform_dirs();
    if dirs.is_empty() {
        eprintln!("skipping: no captures in {}", transform_base());
        return;
    }
    let s = stubs(false);
    let mut sets = Vec::new();
    for name in TRANSFORM_FUNCS {
        for dir in &dirs {
            let n = saved_calls(dir, name).len();
            if n > 0 {
                sets.push((name, dir.clone(), n));
            }
        }
    }
    const CASES: usize = 7;
    let results = leaf_check::each_set(
        &sets,
        |set| {
            set.2
                * if set.0 == TRANSFORM_FUNCS[0] {
                    CASES
                } else {
                    1
                }
        },
        |(name, dir, _)| {
            let cases = if *name == TRANSFORM_FUNCS[0] {
                CASES
            } else {
                1
            };
            let prepares: Vec<Box<Prepare>> = (0..cases)
                .map(|case| -> Box<Prepare> {
                    Box::new(move |call: &mut Call, _: usize| transform_case(call, case))
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|prepare| Variant {
                    prepare: &**prepare,
                    ours: &run_transform,
                    stubs: &s,
                })
                .collect();
            check_variants(dir, name, &variants)
        },
    );
    let mut counts = BTreeMap::new();
    for reports in results.iter().flatten() {
        tally(reports, &mut counts);
    }
    let missing: Vec<&str> = TRANSFORM_FUNCS
        .iter()
        .filter(|n| !counts.contains_key(&leaf_check::sym(n)))
        .copied()
        .collect();
    for (a, c) in &counts {
        eprintln!("{} compared clean {c} times", name_of(*a));
    }
    assert!(missing.is_empty(), "never compared clean: {missing:?}");
}
