//! Ness's own code (`melee/ft/kinds/ftNess`) and his items' (PK Fire and its pillar, PK Flash
//! and its blast, PK Thunder's ball and trails, the bat and the yo-yo) against ssbm-rs's ports,
//! from real game states: every saved call of a Ness callback is imported, run on both sides,
//! and the whole state compared afterwards. Each callback also runs from the captures of its
//! unit's other functions, with the fighter's buttons, stick, script and state variables
//! changed, so callbacks no capture reached (and branches few did) run from real states of the
//! same moves.
//!
//! The helpers that take more than the GObj (the yo-yo's, the items' spawns) run only inside
//! the callbacks that call them ([`HELPERS`]). What Ness's code reaches in other parts of the
//! port that stands in for unported code (Fox's animation helpers, an item's hold kind) stands
//! in on both sides ([`stubs`]); each check runs twice, with the stand-ins that answer a
//! question answering no, then yes, and the calls each side made to them are compared with
//! their arguments.
//!
//! The captures are game memory, kept out of the repository in `NESS_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `ness-funcs.txt` (`ADDR
//! NAME` per line, the eight Ness units', his two functions in the common jump's and the eight
//! Ness item units' functions) and `done-nonness.txt` (every other function's address).
//! ssbm-rs's runner made them (with `CAPTURE_INTERACTED`, which saves calls that touch
//! hardware: the states' entries load animations) on the three Ness CPU matches the oracle
//! diffs (`tools/bench`'s `ness-fox-bf`, `ness-marth-fd` and `ness-fox-ps`):
//!
//! ```text
//! D=local/calls/ness
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/ness-funcs.txt LOCKSTEP_DONE=$D/done-nonness.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/oracle-chars2/NAME.slp --fields 100000 \
//!   --port all --lockstep --fp hardware
//! ```
//!
//! The computer players never charge a smash, use PK Flash in the air or PSI Magnet, so
//! `script1` is a debug VS match of two Nesses on Final Destination whose first port a script
//! drives through those (`MONKEY_HOLD`: Start every ten fields from 30, then from field 400
//! every 150 fields one of: B held 60 fields; a jump then B; the C-stick up, down or forward
//! held; B down held, then turned; the same in the air; B up, then the stick around; the same
//! in the air; B forward; a jump then B back), the second port firing PK Fire both ways while
//! the first holds PSI Magnet, the monkey playing on between:
//!
//! ```text
//! MATCH_FIGHTERS=11 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 MONKEY_HOLD=... (the same CAPTURE_ variables, CAPTURE=$D/script1) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, poke_f32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::ft::fox::ext as fox_ext;
use melee_sim::game::state::Running;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/ness";

fn base() -> String {
    std::env::var("NESS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// Ness's functions by name, from the unit lists in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/ness-funcs.txt", base())) else {
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

/// Helpers that take more than the GObj, or return what the check can't compare (a GObj, a
/// position): they run inside the callbacks that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    "ftNs_Init_GetAbsorbHeal",
    "ftNs_Init_OnItemPickup",
    "ftNs_Init_OnItemDrop",
    "ftNs_AttackS4_CheckNessBatRemove",
    "ftNs_AttackHi4_YoyoApplyDamage",
    "ftNs_AttackHi4_YoyoCheckEnvColl",
    "ftNs_AttackHi4_YoyoSetUnkPos",
    "ftNs_AttackHi4_YoyoSetHitPosUnk",
    "ftNs_AttackHi4_YoyoCheckNoObstruct",
    "ftNs_AttackHi4_YoyoThink_IsRemove",
    "ftNs_SpecialN_CheckSpecialNHold",
    "ftNs_SpecialHi_ItemPKThunder_CheckNessCollide",
    "ftNs_SpecialHi_CheckSpecialHiHold",
    "ftNs_SpecialHi_ItemPKThunderCheckOwn",
    "ftNs_SpecialAirHi_CollisionModVel",
    "ftNs_SpecialLwHold_GroundOrAir",
    "it_802AA054",
    "it_802AA1D8",
    "it_802AA474",
    "itNesspkfirepillar_802AA494",
    "itNesspkfirepillar_802AA55C",
    "itNesspkfirepillar_Logic24_EvtUnk",
    "it_802AA7E4",
    "it_802AA7F0",
    "it_802AA810",
    "it_802AA8C0",
    "it_802AAA50",
    "it_802AAA80",
    "itNesspkflash_Logic102_EvtUnk",
    "it_802AB3F0",
    "it_802AB468",
    "it_802AB4B8",
    "it_802AB568",
    "it_802AB58C",
    "it_802AB9C0",
    "it_802ABA4C",
    "it_802AC41C",
    "it_802AC43C",
    "it_802AC58C",
    "it_802AC5D8",
    "it_802AC604",
    "it_802AD478",
    "it_802AD590",
    "it_802AD6B8",
    "itNessBat_Logic58_EvtUnk",
    "it_802AF940",
    "it_802AFA70",
    "itNessPKFlashExplode_Logic103_EvtUnk",
    "it_802BE598",
    "it_802BE5B8",
    "it_802BE5D8",
    "it_802BE65C",
    "it_802BE958",
    "it_802BE9D8",
    "it_802BF030",
    "it_802BF180",
    "it_802BF28C",
    "it_802BF4A0",
    "it_802BF800",
    "it_802BF900",
    "it_802BFAFC",
    "it_802BFE5C",
    "it_802BFEC4",
    "it_802C0010",
    "it_2725_Logic59_EvtUnk",
];

/// Functions not run here: fighter creation's and death's (the oracle checks them), the bat's
/// second state (Kirby's copy of it, not ported), and the helpers.
fn skipped(name: &str, _addr: u32) -> bool {
    matches!(
        name,
        "ftNs_Init_OnLoad"
            | "ftNs_Init_OnDeath"
            | "ftNs_Init_LoadSpecialAttrs"
            | "itNessbat_UnkMotion1_Anim"
            | "itNessbat_UnkMotion1_Phys"
            | "itNessbat_UnkMotion1_Coll"
    ) || HELPERS.contains(&name)
}

/// The units, by their first function's address: the common jump's two (`ftNs_JumpAerial`),
/// ftness, ftnessattacks4, ftnessattackhi4, ftnessattacklw4, ftnessspecials, ftnessspecialn,
/// ftnessspecialhi, ftnessspeciallw, itnesspkfire, itnesspkfirepillar, itnesspkflash,
/// itnesspkthunderball, itnesspkthundertrail, itnessbat, itnesspkflashexplode and itnessyoyo.
const STARTS: [u32; 17] = [
    0x800C_BD18,
    0x8011_47C0,
    0x8011_4BF4,
    0x8011_4EB8,
    0x8011_659C,
    0x8011_6B70,
    0x8011_6EBC,
    0x8011_7B70,
    0x8011_9E14,
    0x802A_A054,
    0x802A_A494,
    0x802A_A7E4,
    0x802A_B3F0,
    0x802A_C43C,
    0x802A_D478,
    0x802A_F940,
    0x802B_E598,
];

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 8
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

/// Runs Ness's callback `call.addr` on its GObj; a helper that takes more stops the call as
/// skipped.
fn run_ness(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{} takes something else than a GObj", name_of(a));
    }
    let gobj = gobj_arg(maps, call);
    if name_of(a) == "ftNs_JumpAerial_Enter" {
        g.ness_jump_aerial_enter(gobj);
        return Ret::Nothing;
    }
    if fighter_unit(unit(a)) {
        if melee_sim::game::ft::ness::run(g, gobj, a) {
            return Ret::Nothing;
        }
    } else {
        if g.game.world.gobj(gobj).classifier != melee_sim::game::it::CLASS_ITEM {
            panic!("{SKIP}{} takes its parent, not an item", name_of(a));
        }
        if let Some(v) = melee_sim::game::it::kinds::run_pred(g, gobj, a) {
            return Ret::Int(u32::from(v));
        }
        if melee_sim::game::it::kinds::run_event(g, gobj, a) {
            return Ret::Nothing;
        }
    }
    panic!("{SKIP}{} takes more than its GObj", name_of(a));
}

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
    std::env::var("NESS_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each of Ness's functions, run as it was saved, with the stand-ins
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
                    ours: &run_ness,
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

/// Each of Ness's functions run from the captures of its unit's other functions, with the
/// controls and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..STARTS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit(*a) == u && !skipped(n, *a))
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
                            if std::env::var_os("NESS_TRACE").is_some() {
                                eprintln!("{src} #{i} (yes {yes}) runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                            }
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = shifts
                    .iter()
                    .zip(&prepares)
                    .map(|(&(_, yes), prepare)| Variant {
                        prepare: &**prepare,
                        ours: &run_ness,
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
const BLOCKED: &[(&str, &str)] = &[
    (
        "ftNs_Init_OnItemInvisible",
        "Ness never holds an item in the captures",
    ),
    (
        "ftNs_Init_OnItemVisible",
        "Ness never holds an item in the captures",
    ),
    (
        "ftNs_SpecialHi_Enter",
        "PK Thunder never hits Ness on the ground in the captures",
    ),
    (
        "ftNs_SpecialAirHi_Enter",
        "no capture holds a call of it; the oracle's Ness matches run it",
    ),
];

/// Every one of Ness's callbacks compares clean from some real state: its own saved calls or
/// its unit's neighbours'.
#[test]
fn ness_functions() {
    let mut counts = BTreeMap::new();
    // `NESS_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `NESS_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("NESS_TRACE").is_some() {
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
    let one = std::env::var_os("NESS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
