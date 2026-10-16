//! Samus's own code (`melee/ft/kinds/ftSamus`) and her items' (`melee/it/kinds/itsamus*`)
//! against ssbm-rs's ports, from real game states: every saved call of a Samus callback is
//! imported, run on both sides, and the whole state compared afterwards. Each callback also
//! runs from the captures of its unit's other functions, with the fighter's buttons, stick,
//! script and state variables changed, so callbacks no capture reached (and branches few did)
//! run from real states of the same moves.
//!
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callbacks that call them ([`HELPERS`]); her item hooks, which no capture reaches,
//! aren't run ([`UNCAPTURED`]). What Samus's code reaches in other parts
//! of the port that stands in for unported code (an item's hold kind, the hold poses) stands in
//! on both sides ([`stubs`]); each check runs twice, with the stand-ins that answer a question
//! answering no, then yes.
//!
//! The captures are game memory, kept out of the repository in `SAMUS_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `samus-funcs.txt` (`ADDR
//! NAME` per line, the six Samus units' and her four items' functions) and `done-nonsamus.txt`
//! (every other function's address). ssbm-rs's runner made them (with `CAPTURE_INTERACTED`,
//! which saves calls that touch hardware) on the oracle's three CPU matches
//! (`local/bench/oracle-chars2/samus-*.slp`):
//!
//! ```text
//! D=local/calls/samus
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/samus-funcs.txt LOCKSTEP_DONE=$D/done-nonsamus.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/oracle-chars2/NAME.slp --fields 100000 \
//!   --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, poke_f32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::ft::fox::ext as fox_ext;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{samusbomb, samuschargeshot, samusgrapple, samusmissile};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/samus";

fn base() -> String {
    std::env::var("SAMUS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// Samus's functions by name, from the unit lists in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/samus-funcs.txt", base())) else {
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
/// position, a link): they run inside the callbacks that call them, whose comparisons cover
/// them.
const HELPERS: &[&str] = &[
    "ftSs_Init_80128770",
    "ftSs_Init_80128A1C",
    "ftSs_SpecialLw_80129100",
    "ftSs_SpecialS_8012A168",
    "it_802B4AC8",
    "itSamusBomb_Logic50_EvtUnk",
    "it_802B5518",
    "it_802B55C8",
    "it_802B56E4",
    "itSamusChargeshot_Logic108_EvtUnk",
    "it_802B62D0",
    "it_2725_Logic52_EvtUnk",
    "it_802B743C",
    "it_802B75FC",
    "it_802B7C18",
    "it_802B900C",
    "it_802B91C4",
    "it_802B9328",
    "it_802B99A0",
    "it_802B9CE8",
    "it_802B9FD4",
    "it_802BA194",
    "it_802BA2D8",
    "it_802BA3BC",
    "it_802BA5DC",
    "it_802BA760",
    "it_802BAAE4",
    "itSamusGrapple_Logic53_EvtUnk",
];

/// Functions no saved call reaches: her item hooks (the CPU matches have no items, and from
/// her other functions' states the original faults on the missing item).
const UNCAPTURED: &[&str] = &[
    "ftSs_Init_OnItemPickup",
    "ftSs_Init_OnItemInvisible",
    "ftSs_Init_OnItemVisible",
    "ftSs_Init_OnItemDrop",
];

/// Functions not run here: fighter creation's and death's (the oracle checks them), the
/// beam's hitbox setup (`it_802B7160`, not ported: it reads a stack bit), the helpers, and
/// those no saved call reaches.
fn skipped(name: &str, _addr: u32) -> bool {
    matches!(
        name,
        "ftSs_Init_OnLoad" | "ftSs_Init_OnDeath" | "ftSs_Init_LoadSpecialAttrs" | "it_802B7160"
    ) || HELPERS.contains(&name)
        || UNCAPTURED.contains(&name)
}

/// The units, by their first function's address: ftsamus, ftsamusspeciallw0,
/// ftsamusspecialn, ftsamusspecials, ftsamusspecialhi, ftsamusspeciallw1, itsamusbomb,
/// itsamuschargeshot, itsamusmissile and itsamusgrapple.
const STARTS: [u32; 10] = [
    0x8012_832C,
    0x8012_8944,
    0x8012_91F0,
    0x8012_A1D8,
    0x8012_A674,
    0x8012_ADF0,
    0x802B_4AC8,
    0x802B_5518,
    0x802B_62D0,
    0x802B_7150,
];

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 5
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

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments).
const TAKES_MORE: &[&str] = &[
    "ftSs_Init_CreateThrowGrappleBeam",
    "ftSs_Init_80128944",
    "ftSs_Init_80128AC8",
    "ftSs_Init_80128B1C",
];

/// Runs Samus's function `call.addr` on its GObj, with the arguments it was saved with for
/// those that take more ([`TAKES_MORE`]); a helper that takes what the check can't give stops
/// the call as skipped.
fn run_samus(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{} takes something else than a GObj", name_of(a));
    }
    let gobj = gobj_arg(maps, call);
    let [r4, r5] = [4, 5].map(|i| call.regs.gpr[i]);
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let f2 = || call.regs.fpr[2].ps0 as f32;
    let f3 = || call.regs.fpr[3].ps0 as f32;
    let _ = r5;
    match name_of(a).as_str() {
        "ftSs_Init_CreateThrowGrappleBeam" => {
            g.samus_throw_beam(gobj, r4 as i32, f1());
            return Ret::Nothing;
        }
        "ftSs_Init_80128944" => {
            g.samus_bomb_jump(gobj, f1(), f2());
            return Ret::Nothing;
        }
        "ftSs_Init_80128AC8" => {
            return Ret::Float(g.samus_bomb_jump_angle(gobj, f1(), f2()));
        }
        "ftSs_Init_80128B1C" => {
            g.samus_bomb_jump_start(gobj, f64::from(f1()), f2(), f3());
            return Ret::Nothing;
        }
        "ftSs_SpecialLw_80129158" => return Ret::Int(u32::from(g.samus_shot_lost(Some(gobj)))),
        "ftSs_SpecialN_801291A8" => {
            return Ret::Int(u32::from(g.samus_left_charge(Some(gobj))));
        }
        "ftSs_SpecialN_801292E4" => return Ret::Int(u32::from(g.samus_shot_appear(gobj))),
        "ftSs_SpecialS_8012A068" => return Ret::Int(g.samus_missiles(gobj) as u32),
        "ftSs_SpecialN_801293BC" => g.samus_shot_fire(gobj),
        // The beam's hooks on its fighter.
        "it_802BAC3C" | "it_802BAC80" | "it_802BACC4" => {
            if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
                panic!("{SKIP}{} takes the fighter, not the item", name_of(a));
            }
            match a {
                0x802B_AC3C => samusgrapple::ft_death(g, gobj),
                0x802B_AC80 => samusgrapple::ft_frame(g, gobj),
                _ => samusgrapple::ft_line_up(g, gobj),
            }
        }
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighter's or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    let item_helper = |g: &mut Running, gobj| {
        if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
            panic!("{SKIP}{} takes its parent, not an item", name_of(a));
        }
    };
    match name_of(a).as_str() {
        "it_802B4BA0" => {
            item_helper(g, gobj);
            samusbomb::start(g, gobj);
            return Ret::Nothing;
        }
        "it_802B5974" => {
            item_helper(g, gobj);
            samuschargeshot::remove(g, gobj);
            return Ret::Nothing;
        }
        "it_802B5CBC" => {
            item_helper(g, gobj);
            samuschargeshot::fired(g, gobj);
            return Ret::Nothing;
        }
        "it_802B63F8" => {
            item_helper(g, gobj);
            samusmissile::life(g, gobj);
            return Ret::Nothing;
        }
        "it_802B64FC" => {
            item_helper(g, gobj);
            samusmissile::home(g, gobj);
            return Ret::Nothing;
        }
        "it_802B66A8" => {
            item_helper(g, gobj);
            samusmissile::start_homing(g, gobj);
            return Ret::Nothing;
        }
        "it_802B6A60" => {
            item_helper(g, gobj);
            samusmissile::start_super(g, gobj);
            return Ret::Nothing;
        }
        _ => {}
    }
    if fighter_unit(unit(a)) {
        if melee_sim::game::ft::samus::run(g, gobj, a) {
            return Ret::Nothing;
        }
    } else {
        if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
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
    // Calls whose first argument isn't a GObj (a fighter, a link) are skipped when run.
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x200, 0x0][i % 4];
    let pressed = [0x8000_0000u32, 0, 0x200][i % 3];
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
    std::env::var("SAMUS_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each of Samus's functions, run as it was saved, with the stand-ins
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
                    ours: &run_samus,
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

/// Saved calls that aren't states of the item's other callbacks: the beam's spawn callback
/// runs as the item is made, before the beam writes its links and fighter, whose words still
/// hold the block's last item's.
const NOT_SOURCES: &[&str] = &["itSamusGrapple_Logic53_Spawned"];

/// Each of Samus's functions run from the captures of its unit's other functions, with the
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
            if unit(*a) != u
                || !wanted(src)
                || skipped(src, *a)
                || NOT_SOURCES.contains(&src.as_str())
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
                            if std::env::var_os("SAMUS_TRACE").is_some() {
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
                        ours: &run_samus,
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

/// Every one of Samus's callbacks compares clean from some real state: its own saved calls or
/// its unit's neighbours'.
#[test]
fn samus_functions() {
    let mut counts = BTreeMap::new();
    // `SAMUS_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `SAMUS_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("SAMUS_TRACE").is_some() {
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
    let one = std::env::var_os("SAMUS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
