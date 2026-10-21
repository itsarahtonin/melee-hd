//! The items toggle's leftovers (docs/player.md, "Items toggle: what's left", tasks 1 to 3)
//! against ssbm-rs's ports, from real game states: a Shy Guy's food (`it_8028F8E4`,
//! `it_8028F968`, `it_8028F9B8`, `it_8028F9D8` and the Shy Guy's calls of them), a crushed
//! bomb's burst (`it_8026C368`'s `it_8027D730` and `it_8028FE90`), a Bob-omb exploding in its
//! holder's hands (`it_80280B60`, `it_8027429C`), the destroy sound 0x12F (`it_8027327C`),
//! Kongo Jungle's river box (`grKongo_801D7BBC`, `it_80286088`, `itBox_Logic1_Destroyed`,
//! `grKongo_801D8058`), the motion-sensor bomb's and the Flipper's reflect and shield
//! callbacks, and on the fighter's side tumbling under a parasol (`ftCo_80090780`,
//! `ftCo_800CF4DC`, `ftCo_ItemParasolDamageFall_*`, the common parasol's
//! `ftCommon_8007E83C`), the Screw Attack's hit (`ftCo_800D2FA4`, `ftCo_800D3004`,
//! `ftCo_DamageScrew_*`), a frozen fighter's KOs (`ftCo_800D41C4`, `ftCo_800D47B8` and the KO
//! states' frozen spin) and a hit taking off the Bunny Hood (`Fighter_8006CDA4`). Every saved
//! call is imported, run on both sides, and the whole state compared afterwards; each also
//! runs the other functions of its side (the item's or the fighter's) that take only a GObj,
//! so entries and callbacks no capture reached run from real states of the same matches: the
//! KO states with their frozen flag set and clear, the Screw Attack's landing flag both ways,
//! a fast fall on and off (the star and screen KOs' entries are captured as sources for theirs).
//! The kinds' own functions run only on their kind, and the item side also from leaf-check
//! `items_shoot`'s captures of the motion-sensor bombs, Flippers and foods ([`SHOOT_SOURCES`]).
//!
//! The captures are game memory, kept out of the repository in `ITEMS_LEFTOVERS_CALLS`
//! (default below), one directory per run, beside the function lists the runs use:
//! `items-leftovers-funcs.txt` (`ADDR NAME` per line) and `done-leftovers.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the `leftovers` oracle
//! matches (`$SSBM_RS_LOCAL/bench/oracle-m4`, docs/oracle.md's oracle sets), as the oracle
//! traces them:
//!
//! ```text
//! D=local/calls/items-leftovers
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-NAME CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-leftovers-funcs.txt LOCKSTEP_DONE=$D/done-leftovers.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fp hardware --fields FIELDS --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::gr;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{bombhei, crate_box, foods, heiho, msbomb};
use melee_sim::game::it::{self, CLASS_ITEM};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-leftovers";

fn base() -> String {
    std::env::var("ITEMS_LEFTOVERS_CALLS")
        .unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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
    let Ok(text) = std::fs::read_to_string(format!("{}/items-leftovers-funcs.txt", base())) else {
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

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// Whether a function takes a fighter (else an item, or a stage object for Kongo Jungle's).
fn fighter_side(name: &str) -> bool {
    name.starts_with("ft") || name.starts_with("Fighter_")
}

/// The functions that take more than a GObj (or a `Fighter*`, or a stage object), run from
/// their own saved calls only.
const TAKES_MORE: &[&str] = &[
    "it_8028F9D8",
    "it_8028FAF4",
    "it_8027429C",
    "it_8027327C",
    "it_80286088",
    "grKongo_801D7BBC",
    "grKongo_801D8058",
    "ftCommon_8007E83C",
    "Fighter_8006CDA4",
];

/// The item kind a kind's own function takes (its variables are that kind's), if it's one.
fn kind_of(name: &str) -> Option<i32> {
    use melee_sim::game::it::kinds::{BOMBHEI, BOX, FLIPPER, FOODS, HEIHO};
    Some(match name {
        "it_8028F8E4" | "it_8028F968" | "it_8028F9B8" | "it_8028F9D8" => FOODS,
        "it_802D8688" | "it_802D8EC8" | "it_802D96B0" | "it_802D9714" => HEIHO,
        "it_80280B60" | "it_8027D730" => BOMBHEI,
        "it_8028FE90" | "itMSBomb_Logic19_Reflected" => 19,
        "itFlipper_Reflected" | "itFlipper_ShieldBounced" => FLIPPER,
        "itBox_Logic1_Destroyed" => BOX,
        _ => return None,
    })
}

fn f32_at(call: &Call, p: u32) -> f32 {
    f32::from_bits(peek_u32(call, p))
}

fn item_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item", name_of(a));
    }
}

fn fighter_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
        panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
    }
}

/// Runs the function `call.addr` on its GObj (or its `Fighter*`), with the arguments it was
/// saved with for those that take more ([`TAKES_MORE`]).
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    if name == "Fighter_8006CDA4" {
        let fid = *maps
            .fighters
            .get(&r(3))
            .unwrap_or_else(|| panic!("{SKIP}{name} takes a fighter not imported"));
        g.drop_items_on_hit(fid);
        return Ret::Nothing;
    }
    if name == "it_8028FAF4" && r(3) == 0 {
        foods::spawn(g, None, vec_at(r(4)));
        return Ret::Nothing;
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    if fighter_side(&name) {
        fighter_check(g, gobj, a);
    } else if !name.starts_with("gr") && name != "it_80286088" {
        // (Kongo Jungle's box takes the river path, a stage object, to ride.)
        item_check(g, gobj, a);
        if let Some(k) = kind_of(&name)
            && it::item(g.game, gobj).kind != k
        {
            panic!("{SKIP}{name} takes an item of kind {k}");
        }
    }
    match name.as_str() {
        "it_8028F8E4" => foods::let_fall(g, gobj),
        "it_8028F968" => foods::carried(g, gobj),
        "it_8028F9B8" => foods::gone(g, gobj),
        "it_8028F9D8" => foods::place(g, gobj, vec_at(r(4)), call.regs.fpr[1].ps0),
        "it_8028FAF4" => {
            foods::spawn(g, Some(gobj), vec_at(r(4)));
        }
        "it_802D96B0" => heiho::carry(g, gobj),
        "it_802D9714" => heiho::blast_zones(g, gobj),
        "it_80280B60" => bombhei::explode(g, gobj),
        "it_8027429C" => it::release::let_go(g, gobj, vec_at(r(4))),
        "it_8026C368" => it::lib26b1::check_burst(g, gobj),
        "it_8027D730" => bombhei::detonate(g, gobj),
        "it_8028FE90" => msbomb::detonate(g, gobj),
        "it_8027327C" => it::lib2725::destroy_effect(g, gobj, r(4) as i32, r(5) as i32),
        "it_80286088" => {
            crate_box::spawn_floating(g, Some(gobj));
        }
        "grKongo_801D7BBC" => gr::kongo::events(g, gobj),
        "grKongo_801D8058" => gr::kongo::destroy_path(g, gobj),
        "ftCo_80090780" => g.enter_damage_fall(gobj),
        "ftCo_800CF4DC" => g.enter_parasol_damage_fall(gobj),
        "ftCommon_8007E83C" => g.parasol_step(gobj, r(4) as i32, f1()),
        "ftCo_800D2FA4" => return b(g.screw_check(gobj)),
        "ftCo_800D3004" => g.enter_damage_screw(gobj),
        "ftCo_800D40B8" => g.enter_dead_up_star(gobj),
        "ftCo_800D41C4" => g.enter_dead_up_star_ice(gobj),
        "ftCo_800D4780" => g.enter_dead_up_fall(gobj),
        "ftCo_800D47B8" => g.enter_dead_up_fall_ice(gobj),
        _ if fighter_side(&name) => {
            run_fighter_cb(g, gobj, CodeRef(a));
        }
        _ => {
            if let Some(v) = it::kinds::run_pred(g, gobj, a) {
                return b(v);
            }
            if !it::kinds::run_event(g, gobj, a) {
                panic!("{SKIP}{name} takes more than its GObj");
            }
        }
    }
    Ret::Nothing
}

/// `Fighter::mv` and the KO states' frozen flag (`unk_deadup.x68`) and the Screw Attack's
/// landing flag (`cmd_vars[0]`).
const MV: u32 = 0x2340;
const DEAD_FROZEN: u32 = MV + 0x28;
const CMD_VAR0: u32 = 0x2200;
/// `Fighter::fall_fast` (`x221A`'s bit 4), which a state change keeps only when asked.
const FALL_FAST: (u32, u32) = (0x221A, 4);

/// The fighter's state varied so the leftovers' branches run: the star and screen KOs
/// (states 4 to 10) with their frozen flag set and clear, the script's landing flag, and a
/// fast fall.
fn vary_fighter(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    if (4..=10).contains(&peek_u32(call, fp + 0x10)) {
        poke_u32(call, fp + DEAD_FROZEN, (i % 2) as u32);
    }
    poke_u32(call, fp + CMD_VAR0, ((i / 2) % 2) as u32);
    poke_bit(call, fp + FALL_FAST.0, FALL_FAST.1, (i / 4) % 2 == 1);
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
    std::env::var("ITEMS_LEFTOVERS_SRC").map_or(true, |o| o == name)
}

/// Whether `ITEMS_LEFTOVERS_TARGET` leaves the function to run: all of them unless it names one.
fn targeted(name: &str) -> bool {
    std::env::var("ITEMS_LEFTOVERS_TARGET").map_or(true, |t| t == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    let mut sets = Vec::new();
    for (_, name) in functions() {
        if !wanted(&name) || !targeted(&name) {
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

/// Each function that takes only a GObj run from every capture of its side's functions, the
/// fighter's state varied ([`vary_fighter`]).
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for fighter in [false, true] {
        let side = |n: &str| fighter_side(n) == fighter && !n.starts_with("gr");
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(_, n)| side(n) && !TAKES_MORE.contains(&n.as_str()) && targeted(n))
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (_, src) in &funcs {
            if !side(src) || !wanted(src) {
                continue;
            }
            for dir in dirs() {
                let n = saved_calls(&dir, src).len();
                if n > 0 {
                    runs.push((src.clone(), dir, n));
                }
            }
        }
        if !fighter {
            for (dir, src) in shoot_sources() {
                let n = saved_calls(&dir, src).len();
                if n > 0 && wanted(src) {
                    runs.push((src.to_owned(), dir, n));
                }
            }
        }
        if runs.is_empty() {
            continue;
        }
        // Every call runs every target once, a round each.
        let rounds = targets.len();
        let no = stubs();
        let targets = &targets;
        let results = leaf_check::each_set(
            &runs,
            |r| r.2 * rounds,
            |(src, dir, _)| {
                let prepares: Vec<Box<Prepare>> = (0..rounds)
                    .map(|round| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(round + i) % targets.len()];
                            call.addr = *a;
                            if std::env::var_os("ITEMS_LEFTOVERS_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighter {
                                vary_fighter(call, i);
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

/// The motion-sensor bombs', Flippers' and foods' states in flight, hitting and lying, from
/// leaf-check `items_shoot`'s captures of its oracle matches (`calls/items-shoot`), which the
/// kinds' callbacks here also run from.
const SHOOT_SOURCES: &[&str] = &[
    "itMSBomb_Logic19_DmgDealt",
    "itMSBomb_Logic19_HitShield",
    "itMsbomb_UnkMotion3_Phys",
    "itMsbomb_UnkMotion4_Coll",
    "itMsbomb_UnkMotion5_Coll",
    "itFlipper_DmgDealt",
    "itFlipper_HitShield",
    "itFlipper_Inflight_Phys",
    "itFoods_UnkMotion0_Phys",
    "itFoods_UnkMotion1_Phys",
];

fn shoot_sources() -> Vec<(String, &'static str)> {
    let base = std::env::var("ITEMS_SHOOT_CALLS")
        .unwrap_or_else(|_| melee_local_data::ssbm_rs_local("calls/items-shoot"));
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .collect();
    dirs.sort();
    dirs.iter()
        .flat_map(|d| SHOOT_SOURCES.iter().map(move |s| (d.clone(), *s)))
        .collect()
}

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every function compares clean from some real state: its own saved calls or its side's
/// neighbours'.
#[test]
fn items_leftovers_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_LEFTOVERS_SRC=NAME` runs only NAME's saved calls, as saved and as its
    // neighbours' (with `ITEMS_LEFTOVERS_TRACE`, naming what each runs), and
    // `ITEMS_LEFTOVERS_TARGET=NAME` runs only NAME, from its saved calls and its neighbours',
    // to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let missing: Vec<&str> = funcs
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !BLOCKED.iter().any(|b| b.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        funcs.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        funcs.len()
    );
    for (a, c) in &counts {
        eprintln!("{} compared clean {c} times", name_of(*a));
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    for (n, why) in BLOCKED {
        eprintln!("{n} is blocked: {why}");
    }
    let one = std::env::var_os("ITEMS_LEFTOVERS_SRC").is_some()
        || std::env::var_os("ITEMS_LEFTOVERS_TARGET").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
