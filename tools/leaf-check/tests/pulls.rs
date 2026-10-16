//! The items Peach's vegetable pull can give besides a turnip and a Bob-omb, against
//! ssbm-rs's ports from real game states: Mr. Saturn (`melee/it/kinds/itdosei`), the Beam
//! Sword (`melee/it/kinds/itsword`), the swings a fighter holding it makes (`ftswing.c`,
//! `ft_0CD1.c`, the sword's part of `ft_0CD3.c`, and the characters' item hooks they call,
//! `ftCommon_8007E79C` and `ftCommon_8007E7E4`), and the player statistics an item picked up or
//! a Bob-omb's hit counts (`pl_8003E854`, `pl_800419AC`, `pl_80041B08`). Every saved call of
//! one of these functions is
//! imported, run on both sides, and the whole state compared afterwards; each also runs from
//! the captures of its unit's other functions, with the fighter's controls and the item's
//! counters changed, so functions no capture reached (Mr. Saturn's states after a spawn of
//! its own, which only the item switch makes) and branches few did run from real states.
//!
//! The captures are game memory, kept out of the repository in `PULLS_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `pulls-funcs.txt` (`ADDR
//! NAME` per line) and `done-nonpulls.txt` (every other function's address). ssbm-rs's runner
//! made them on ranked Slippi 3.14 and 3.15 replays of Peach from `erickfm/melee-ranked-replays`'
//! `PEACH_diamond-diamond_a1` (`local/replays/hf/peach-dd-a1/slp/`), named as the oracle's
//! traces of them are (`docs/oracle.md`):
//!
//! ```text
//! D=local/calls/pulls
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_EVERY=50 CAPTURE_FUNCS=@$D/pulls-funcs.txt LOCKSTEP_DONE=$D/done-nonpulls.txt \
//!   ssbm-run "$SSBM_DISC" --replay REPLAY.slp --fields 100000 --port all --lockstep
//! ```
//!
//! The player statistics' calls are rarer and come from runs of their own (`pl-NAME`), with
//! `pulls-pl-funcs.txt` (their four lines) for `CAPTURE_FUNCS`, `done-nonpl.txt` for
//! `LOCKSTEP_DONE` and `CAPTURE_EVERY=1`. No replay at hand has a player pick up or catch a
//! Bob-omb, which fills the player's list of Bob-ombs that the hits look up, so those run from
//! the pickups of Mr. Saturn and the sword with the item made a Bob-omb and the list filled
//! ([`vary_player`]).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32, poke_bit,
    poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::co::swing;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{dosei, sword};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/pulls";

fn base() -> String {
    std::env::var("PULLS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The code other parts of the port own, standing in on both sides: the crowd and the
/// player library's statistics.
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
    let Ok(text) = std::fs::read_to_string(format!("{}/pulls-funcs.txt", base())) else {
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

/// The units, by their first function's address.
const STARTS: [u32; 8] = [
    0x8003_E854, // plbonuslib (the caught item's statistic)
    0x8004_19AC, // pl_040D (the player's Bob-ombs)
    0x8007_E79C, // ftcommon's item hooks
    0x800C_CE94, // ftswing
    0x800C_D140, // ft_0CD1
    0x800C_D350, // ft_0CD3 (the sword's swing)
    0x8028_1164, // itdosei
    0x8028_4D54, // itsword
];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// The units whose functions run from each other's captures: the statistics', the swings',
/// Mr. Saturn's and the sword's.
fn group(u: usize) -> usize {
    match u {
        0 | 1 => 0,
        2..=5 => 2,
        u => u,
    }
}

/// Whether a unit's functions take a fighter (else an item, or a player's slot).
fn fighter_unit(u: usize) -> bool {
    (2..=5).contains(&u)
}

/// Whether a unit's functions take a player's slot.
fn player_unit(u: usize) -> bool {
    u <= 1
}

/// Helpers that take what the check can't give, or return what it can't compare (a
/// position, out parameters, game data): leaf-check's `items` runs them from the item
/// corpus's swords, and their callers' comparisons here cover them.
const HELPERS: &[&str] = &[
    "itSword_Spawn",
    "it_802852B8",
    "it_80285300",
    "it_80285314",
    "fn_80041BC8",
];

/// The Home-Run Bat's render callback, in the sword's unit: no state of a pull reaches a
/// bat, and it draws.
const BAT_RENDER: &str = "it_80284E10";

/// Which swing item a kind is, which the swing inlines, so no call of it is ever saved:
/// [`swing_types`] runs it.
const SWING_TYPE: &str = "fn_800CCEC4";

/// Functions not run by [`pulls_functions`].
fn skipped(name: &str) -> bool {
    HELPERS.contains(&name) || name == BAT_RENDER || name == SWING_TYPE
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments), but for [`FORGETS`].
const TAKES_MORE: &[&str] = &[
    "pl_8003E854",
    "pl_800419AC",
    "pl_80041B08",
    "ftCo_Attack_800CCF58",
    "ftCo_800CD140",
    "ftCo_800CD2C4",
    "ftCo_800CD350",
    "ftCommon_8007E79C",
    "ftCommon_8007E7E4",
    "it_80284FC4",
    "it_80285024",
    "itDosei_Logic7_EvtUnk",
    "itSword_Logic12_EvtUnk",
];

/// The items' callbacks that forget a GObj (`EvtUnk`, which nothing dispatches yet): run from
/// the other callbacks' captures, told to forget the item's owner.
const FORGETS: &[&str] = &["itDosei_Logic7_EvtUnk", "itSword_Logic12_EvtUnk"];

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]); a call whose arguments the check can't give stops as
/// skipped.
fn run_pulls(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let (r4, r5, r6) = (call.regs.gpr[4], call.regs.gpr[5], call.regs.gpr[6]);
    let f1 = || f32::from_bits(leaf_check::fp::stfs(call.regs.fpr[1].ps0));
    let (slot, sub) = (call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
    match name.as_str() {
        "pl_8003E854" => {
            let it = *maps
                .gobjs
                .get(&r5)
                .unwrap_or_else(|| panic!("{SKIP}pl_8003E854 of an item not imported"));
            g.count_item_caught(slot, sub, it);
            return Ret::Nothing;
        }
        "pl_800419AC" => {
            g.bombhei_thrown(slot, sub, r5 as u16);
            return Ret::Nothing;
        }
        "pl_80041B08" => {
            g.game.count_bombhei_hit(slot, r5 as u16);
            return Ret::Nothing;
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let item = |g: &Running| {
        if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
            panic!("{SKIP}{name} takes an item");
        }
    };
    let other = || {
        (r4 != 0).then(|| {
            *maps
                .gobjs
                .get(&r4)
                .unwrap_or_else(|| panic!("{SKIP}{name} forgets a GObj not imported"))
        })
    };
    match name.as_str() {
        "ftCo_Attack_800CCF58" => g.enter_swing(gobj, r4 as i32),
        "ftCo_800CD140" => g.enter_item_swing(gobj, r4 as i32, r5, r6 as i32, f1()),
        "ftCo_800CD350" => g.enter_sword_swing(gobj, r4 as i32, r5 as i32, f1()),
        "ftCo_800CD1BC" => g.co_swing_anim(gobj),
        "ftCo_800CD204" => g.co_swing_iasa(gobj),
        "ftCo_800CD278" => g.co_swing_phys(gobj),
        "ftCo_800CD2C4" => {
            // Every caller passes `ft_800CD31C`.
            if r4 != swing::TAKE_DMG {
                panic!("{SKIP}ftCo_800CD2C4 with another callback");
            }
            g.co_swing_coll(gobj);
        }
        "ftCommon_8007E79C" => g.item_drop_hook(gobj, r4 != 0),
        "ftCommon_8007E7E4" => g.item_pickup_hook(gobj, r4 != 0),
        "it_80284E30" => {
            item(g);
            sword::scale_blade(g, gobj);
        }
        "it_80285084" => {
            item(g);
            sword::ease_length(g.game, gobj);
        }
        "it_80285140" => {
            item(g);
            sword::ease_width(g.game, gobj);
        }
        "it_802851FC" => {
            item(g);
            sword::ease_swing(g.game, gobj);
        }
        "it_80284FC4" => {
            item(g);
            sword::grow(g.game, gobj, r4 as i32, f1());
        }
        "it_80285024" => {
            item(g);
            sword::shrink(g.game, gobj, r4 as i32);
        }
        "itDosei_Logic7_EvtUnk" => {
            item(g);
            dosei::forget(g, gobj, other());
        }
        "itSword_Logic12_EvtUnk" => {
            item(g);
            sword::forget(g, gobj, other());
        }
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if fighter_unit(unit(a)) {
        if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
            panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
        }
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item, not its parent", name_of(a));
    }
    if let Some(v) = melee_sim::game::it::kinds::run_pred(g, gobj, a) {
        return Ret::Int(u32::from(v));
    }
    if melee_sim::game::it::kinds::run_event(g, gobj, a) {
        return Ret::Nothing;
    }
    panic!("{SKIP}{} takes more than its GObj", name_of(a));
}

/// Whether a call's first argument looks like a GObj with user data.
fn takes_gobj(call: &Call) -> bool {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    in_mem(call.regs.gpr[3]) && in_mem(user_data_arg(call))
}

/// The fighter's controls and swing changed so the swings' branches run: A held or let go,
/// the script's second-swing flag (throw flag 3), the stick pushed, and which swing it is
/// (`swing.x4`, the dash swing sliding with friction), whether A has been held through it
/// (`swing.x0`), and the fighter's floor taken away.
fn vary_fighter(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x100, 0x8000_0000, 0x100][i % 4];
    let pressed = [0x100u32, 0, 0x200, 0][(i / 3) % 4];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let sticks = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (0.6, 0.0),
    ];
    let (x, y) = sticks[i % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    poke_bit(call, fp + 0x2210, 3, i % 4 == 2);
    poke_u32(call, fp + 0x2340, ((i / 2) & 1) as u32);
    poke_u32(call, fp + 0x2344, [0u32, 1, 2, 4][(i / 5) % 4]);
    // Its floor gone now and then (`coll_data.floor.index`), so a swing's collision finds no
    // ground.
    if i % 5 == 3 {
        poke_u32(call, fp + 0x6F0 + 0x14C, u32::MAX);
    }
}

/// Mr. Saturn's counters changed so its states' branches run: the frames it stands after a
/// spawn (`xDD4`), its spawn turn's stage and what's left of it (`xDD8`, `xDDC`), how long it
/// stays dazed (`xDF0`), the slope it last skidded on (`xDF8`), the way it faces, and the floor
/// under it sloped (`x378_itemColl.floor.normal`, which paces its steps).
fn vary_dosei(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let ip = user_data_arg(call);
    if peek_u32(call, ip + 0x10) != 7 {
        return;
    }
    poke_u32(call, ip + 0xDD4, [0u32, 3][i % 2]);
    poke_u32(call, ip + 0xDD8, [0u32, 1, 2][(i / 2) % 3]);
    poke_f32(call, ip + 0xDDC, [0.0f32, 0.05, 1.5][(i / 3) % 3]);
    poke_u32(call, ip + 0xDF0, [0u32, 1, 5][(i / 5) % 3]);
    poke_f32(call, ip + 0xDF8, [0.0f32, -0.3, 0.3][(i / 7) % 3]);
    if i % 4 == 1 {
        let f = f32::from_bits(peek_u32(call, ip + 0x2C));
        poke_f32(call, ip + 0x2C, -f);
    }
    if i % 3 == 2 {
        poke_f32(call, ip + 0x4CC, [0.25f32, -0.4][(i / 3) % 2]);
        poke_f32(call, ip + 0x4D0, 0.95);
    }
}

/// Where a player's list of its last five Bob-ombs is (`pl_StaleMoveTable::x8D8`).
fn bomb_list(slot: u32) -> u32 {
    leaf_check::sym("player_slots") + 0xE90 * slot + 0xBC + 0x8D8
}

/// The player statistics' captures changed: the caught item made a Bob-omb whose fuse has
/// some way to go, or is exploding (`pl_8003E854`, which then lists it), the player's list of
/// Bob-ombs filled, the item's or the hit's number among them or not, those pushed out having
/// hit or not, and the word a pickup clears (`xCA8`) set.
fn vary_player(call: &mut Call, i: usize) {
    let slot = call.regs.gpr[3];
    if slot >= 6 {
        return;
    }
    let list = bomb_list(slot);
    // `xCA8`, which a pickup by the main fighter clears.
    poke_u32(call, list - 0x8D8 + 0xCA8, (i % 2) as u32);
    let caught = call.addr == leaf_check::sym("pl_8003E854");
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if caught && !in_mem(call.regs.gpr[5]) {
        return;
    }
    let number = if caught {
        let ip = peek_u32(call, call.regs.gpr[5] + 0x2C);
        poke_u32(call, ip + 0x10, 6);
        poke_u32(call, ip + 0x24, [4u32, 0xB][(i / 4) % 2]);
        poke_f32(call, ip + 0xDEC, [10.0f32, 1.0e6][(i / 8) % 2]);
        peek_u32(call, ip + 0x1C) & 0xFFFF
    } else {
        call.regs.gpr[5] & 0xFFFF
    };
    poke_u32(call, list, (i % 5) as u32);
    for k in 0..5u32 {
        let listed = match i % 3 {
            0 => 0,
            1 => 0x100 + k,
            _ if k == 2 => number,
            _ => 0x200 + k,
        };
        poke_u32(call, list + 4 + 8 * k, listed);
        // `x4_b0` (it hit) and `x4_b1` (it's listed), the byte's top two bits.
        let bits = match (i / 3 + k as usize) % 3 {
            0 => 0x00,
            1 => 0x40,
            _ => 0xC0,
        };
        poke_u32(call, list + 8 + 8 * k, bits << 24);
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
    std::env::var("PULLS_SRC").map_or(true, |o| o == name)
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
                ours: &run_pulls,
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Each function run from the captures of its unit's other functions, with the controls and
/// counters changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..STARTS.len() {
        if group(u) != u {
            continue;
        }
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                group(unit(*a)) == u
                    && !skipped(n)
                    && (!TAKES_MORE.contains(&n.as_str())
                        || FORGETS.contains(&n.as_str())
                        || player_unit(unit(*a)))
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if group(unit(*a)) != u || !wanted(src) || skipped(src) {
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
                let prepares: Vec<Box<leaf_check::calls::Prepare>> = shifts
                    .iter()
                    .map(|&shift| -> Box<leaf_check::calls::Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if std::env::var_os("PULLS_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            // How many times this target ran before: what's changed follows it,
                            // so each target meets every change.
                            let visit = (shift + i) / targets.len();
                            if player_unit(u) {
                                vary_player(call, visit);
                            } else if fighter_unit(u) {
                                vary_fighter(call, visit);
                            } else if FORGETS.contains(&n.as_str()) {
                                // What the item forgets: its owner.
                                call.regs.gpr[4] = peek_u32(call, user_data_arg(call) + 0x518);
                            } else {
                                vary_dosei(call, visit);
                            }
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run_pulls,
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

/// Every function compares clean from some real state: its own saved calls or its unit's
/// neighbours'.
#[test]
fn pulls_functions() {
    let mut counts = BTreeMap::new();
    // `PULLS_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `PULLS_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("PULLS_TRACE").is_some() {
        for (a, c) in &counts {
            eprintln!("{} compared clean {c} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    let one = std::env::var_os("PULLS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// Which swing item each swing item's kind is (`fn_800CCEC4`), against the original's.
#[test]
fn swing_types() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    for kind in [11, 12, 13, 22, 23, 24] {
        assert_eq!(
            swing::swing_type(kind),
            ssbm_types::fns::fn_800CCEC4(&ctx, kind),
            "kind {kind}"
        );
    }
}
