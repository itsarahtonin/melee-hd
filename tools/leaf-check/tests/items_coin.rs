//! The single-player coin (`melee/it/kinds/itcoin.c`), a beaten enemy's trophy roll
//! (`it_8027CC88`), the match's trophy count (`gm_8017E280`) and the trophy lookups the coin
//! takes (`tyDisplay_8031C5E4`, `tyDisplay_8031C354`, `Toy_803049F4`) against ssbm-rs's ports,
//! from real game states: every saved call of one of these functions is imported, run on both
//! sides, and the whole state compared afterwards. The coin's callbacks that take only their
//! GObj also run from the captures of the coin's other functions.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_COIN_CALLS` (default
//! below), one directory per run, beside the function list (`coin-funcs.txt`, `ADDR NAME` per
//! line) and every other function's address (`done-noncoin.txt`). ssbm-rs's runner made them
//! on the Adventure recording `2026-10-06_12-04-48` (the stages' trophy points drop coins,
//! `Ground_801C58E0`, which the player picks up; Goombas and a bouncing enemy roll and miss),
//! no Training session having a coin to give (docs/oracle.md, "Training's trophy roll"):
//!
//! ```text
//! D=local/calls/coin
//! CAPTURE_INTERACTED=1 CAPTURE=$D/adv-12-04-48 CAPTURE_FROM=2000 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/coin-funcs.txt LOCKSTEP_DONE=$D/done-noncoin.txt \
//!   ssbm-run "$SSBM_DISC" --inputs local/lockstep/recordings/2026-10-06_12-04-48.inputs \
//!   --card COPY_OF_ITS_CARD --fields 43000 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, SKIP, Stub, Variant, check_variants, gobj_arg, peek_u32, saved_calls,
};
use melee_capture::import::Maps;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{self, trophycoin};
use melee_sim::game::it::{self, CLASS_ITEM, zako};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/coin";

fn base() -> String {
    std::env::var("ITEMS_COIN_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The functions by address and name, from the capture directory's list.
fn functions() -> Vec<(u32, String)> {
    let text = std::fs::read_to_string(format!("{}/coin-funcs.txt", base())).unwrap_or_default();
    let mut v: Vec<(u32, String)> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .collect();
    v.sort();
    v
}

/// The coin's unit, whose functions run from each other's captures.
const UNIT: (u32, u32) = (0x802F_13B4, 0x802F_2240);

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// A coin's callbacks take a coin.
fn coin_check(g: &Running, gobj: GObjId, name: &str) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM
        || it::lib26b1::kind(g.game, gobj) != kinds::COIN
    {
        panic!("{SKIP}{name} takes a 1P coin");
    }
}

fn f32_at(call: &Call, p: u32) -> f32 {
    f32::from_bits(peek_u32(call, p))
}

/// The original's address of a GObj the import knows.
fn addr_of(maps: &Maps, v: Option<GObjId>) -> u32 {
    v.map_or(0, |v| {
        maps.gobjs.iter().find(|&(_, &x)| x == v).map_or_else(
            || panic!("{SKIP}a result the import doesn't know"),
            |(&a, _)| a,
        )
    })
}

/// Runs the function `call.addr` with the arguments it was saved with.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    let gobj_or_none = |i: usize| -> Option<GObjId> {
        (r(i) != 0).then(|| {
            *maps
                .gobjs
                .get(&r(i))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a GObj not imported"))
        })
    };
    match name.as_str() {
        // A new coin: its address, which the import can't know, compares where the caller
        // keeps it.
        "it_802F2094" => {
            // A block new from the heap holds bytes the import's heap image doesn't know, which
            // item creation leaves (the script's and colour overlay's words, the collision's
            // contact): a session with `--heap` compares those.
            if g.game.items.free_head().is_none() {
                panic!("{SKIP}{name}'s coin takes a block new from the heap");
            }
            let _ = trophycoin::spawn(g, gobj_or_none(3), vec_at(r(4)), r(5) as i32, r(6) as i32);
            return Ret::Nothing;
        }
        "gm_8017E280" => {
            let n = g.count_match_trophy(r(3) as u16, r(4));
            return Ret::Int(i32::from(n) as u32);
        }
        "tyDisplay_8031C5E4" => {
            let _ = g.coin_trophy_model(r(3) as i32);
            return Ret::Nothing;
        }
        "Toy_803049F4" => return Ret::Int(u32::from(g.toy_marked(r(3) as i32))),
        "tyDisplay_8031C354" => panic!("{SKIP}{name} writes a buffer of its caller's"),
        _ => {}
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    if name == "it_8027CC88" {
        let t = zako::trophy_roll(g, gobj);
        return Ret::Int(addr_of(maps, t));
    }
    coin_check(g, gobj, &name);
    match name.as_str() {
        "it_802F13B4" => g.render_trophy_coin(gobj, r(4) as i32),
        "it_802F1588" => trophycoin::fell(g, gobj),
        "it_802F1630" => trophycoin::landed(g, gobj),
        "it_802F2014" => trophycoin::set_joint_only(g, gobj, r(4) as i32),
        "it_802F2020" => return Ret::Int(trophycoin::trophy(g, gobj) as u32),
        "it_802F202C" => trophycoin::knocked_off(g, gobj),
        "itCoin_Logic116_EvtUnk" => {
            let other = gobj_or_none(4);
            it::evt_unk::run(g, gobj, other, CodeRef(a));
        }
        _ => {
            if let Some(v) = kinds::run_pred(g, gobj, a) {
                return Ret::Int(u32::from(v));
            }
            if !kinds::run_event(g, gobj, a) {
                panic!("{SKIP}{name} takes more than its GObj");
            }
        }
    }
    Ret::Nothing
}

/// Neighbours' calls can't be made ready for these: they take arguments only their callers
/// give (the render callback's pass, and a draw the reference can't make).
const OWN_CALLS_ONLY: &[&str] = &["it_802F13B4"];

/// A neighbour's call made ready for `target`, its `k`th: the arguments those taking more than
/// their GObj take, within what the game passes.
fn args(call: &mut Call, target: &str, k: usize) {
    // A neighbour whose first argument is no GObj (the spawn's parent, none) makes none ready.
    if !(0x8000_0000..0x817F_FFC0).contains(&call.regs.gpr[3]) {
        return;
    }
    let ip = peek_u32(call, call.regs.gpr[3] + 0x2C);
    if !(0x8000_0000..0x817F_F000).contains(&ip) {
        return;
    }
    match target {
        // Another coin of the same trophy at the coin's place, as a trophy point or a roll
        // makes one (no stage object, kind 0).
        "it_802F2094" => {
            let trophy = peek_u32(call, ip + 0xDD4);
            let g = &mut call.regs.gpr;
            g[3] = 0;
            g[4] = ip + 0x4C;
            g[5] = trophy;
            g[6] = 0;
        }
        "it_802F2014" => call.regs.gpr[4] = (k % 2) as u32,
        // The GObj going away: the first fighter.
        "itCoin_Logic116_EvtUnk" => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            call.regs.gpr[4] = peek_u32(call, heads + 4 * 8);
        }
        _ => {}
    }
}

fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

/// Every function compares clean from some real state, unless it's listed in [`UNREACHED`].
#[test]
fn items_coin_functions() {
    let funcs = functions();
    if funcs.is_empty() || dirs().is_empty() {
        eprintln!("no coin captures in {}", base());
        return;
    }
    let no: Vec<Stub> = Vec::new();
    let mut counts = BTreeMap::new();
    // As saved.
    let mut sets = Vec::new();
    for (_, name) in &funcs {
        for dir in dirs() {
            let n = saved_calls(&dir, name).len();
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
        tally(reports, &mut counts);
    }
    // From the unit's neighbours.
    let targets: Vec<&(u32, String)> = funcs
        .iter()
        .filter(|(a, n)| (UNIT.0..UNIT.1).contains(a) && !OWN_CALLS_ONLY.contains(&n.as_str()))
        .collect();
    let mut runs = Vec::new();
    for (a, src) in &funcs {
        if !(UNIT.0..UNIT.1).contains(a) {
            continue;
        }
        for dir in dirs() {
            let n = saved_calls(&dir, src).len();
            if n > 0 {
                runs.push((src.clone(), dir, n));
            }
        }
    }
    let rounds = targets.len();
    let targets = &targets;
    let results = leaf_check::each_set(
        &runs,
        |j| j.2 * rounds,
        |(src, dir, _)| {
            let prepares: Vec<Box<leaf_check::calls::Prepare>> = (0..rounds)
                .map(|round| -> Box<leaf_check::calls::Prepare> {
                    Box::new(move |call: &mut Call, i: usize| {
                        let (a, n) = targets[(round + i) % targets.len()];
                        call.addr = *a;
                        args(call, n, round + i);
                    })
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|p| Variant {
                    prepare: &**p,
                    ours: &run,
                    stubs: &no,
                })
                .collect();
            let _ = src;
            check_variants(dir, src, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, &mut counts);
    }
    let missing: Vec<&str> = funcs
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !UNREACHED.iter().any(|u| u.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    for (a, c) in &counts {
        eprintln!("{} compared clean {c} times", name_of(*a));
    }
    let unblocked: Vec<&str> = UNREACHED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    assert!(missing.is_empty(), "never compared clean: {missing:?}");
    assert!(
        unblocked.is_empty(),
        "compare clean now, take them off UNREACHED: {unblocked:?}"
    );
}

/// Functions no saved state reaches, with why; one that compares clean fails the check until
/// it's taken off.
const UNREACHED: &[(&str, &str)] = &[
    (
        "tyDisplay_8031C354",
        "a roll's hit (it_8027CC88's one in 16): the recording's Goombas and the bouncing enemy roll and miss",
    ),
    (
        "Toy_803049F4",
        "tyDisplay_8031C354's, which no capture reaches",
    ),
];
