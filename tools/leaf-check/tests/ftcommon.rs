//! The fighters' common action states (`melee/ft/kinds/ftCommon`) against ssbm-rs's ports, run
//! from real game states: each saved call of a fighter process is imported, the action state's
//! own callback the process would run (its input, animation, physics or collision callback) is
//! run on both sides from the same state, and the whole state compared afterwards. The states'
//! entries run directly from the same states.
//!
//! What the states reach in other parts of the port (a character's specials, a knockdown's effect,
//! the crowd, the player library's statistics) stands in on both sides (`STUBS`), and the calls
//! each side made to it are compared with their arguments.
//!
//! The captures come from Slippi replays on Final Destination and Battlefield (whose matches
//! start on fresh memory, which the importer needs), made by ssbm-rs's runner with only the
//! processes checked:
//!
//! ```text
//! CAPTURE=$CALLS/r4fd CAPTURE_EVERY=200 \
//!   CAPTURE_FUNCS=Fighter_procInput,Fighter_procAnim,Fighter_procUpdate,Fighter_procMap \
//!   LOCKSTEP_DONE=<every function but those four> \
//!   ssbm-run DISC --replay Game_20260913T223312.slp --fields 10200 --port all --lockstep \
//!   --known local/replays/known/Game_20260913T223312.txt
//! ```
//!
//! and likewise `r4bf` (Game_20260531T184713, every 300th call, every fifth capture kept) and
//! `r2fd` (Game_20260129T224257, every 150th, every fifth kept). `$CALLS` defaults to the
//! directory below; set `FTCOMMON_CALLS` to use another. The Fox and Sheik captures of
//! `fighter.rs` are run too.

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, Stub, StubRet, check_calls_stubbed, gobj_arg, peek_u32, user_data_arg,
};
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::co::kind_callbacks as kinds;
use melee_sim::game::handles::CodeRef;

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/ftcommon";
const FIGHTER_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/fighter-fd1";

/// The capture sets.
fn dirs() -> Vec<String> {
    let base = std::env::var("FTCOMMON_CALLS").unwrap_or_else(|_| CALLS.to_owned());
    let mut v: Vec<String> = ["r4fd", "r4bf", "r2fd"]
        .iter()
        .map(|d| format!("{base}/{d}"))
        .collect();
    v.push(std::env::var("FIGHTER_CALLS").unwrap_or_else(|_| FIGHTER_CALLS.to_owned()));
    v
}

/// The code other parts of the port own, standing in on both sides.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = [
        // The crowd.
        "un_80322598",
    ]
    .iter()
    .map(|n| (leaf_check::sym(n), StubRet::Nothing))
    .collect();
    // The crowd's landing reaction: no reaction, so the knockback it would clear stays.
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    // Every character's specials' entries.
    for table in [
        &kinds::SPECIAL_N,
        &kinds::SPECIAL_S,
        &kinds::SPECIAL_HI,
        &kinds::SPECIAL_LW,
        &kinds::SPECIAL_AIR_N,
        &kinds::SPECIAL_AIR_S,
        &kinds::SPECIAL_AIR_HI,
        &kinds::SPECIAL_AIR_LW,
    ] {
        for &a in table.iter() {
            if a != 0 && !v.iter().any(|s| s.0 == a) {
                v.push((a, StubRet::Nothing));
            }
        }
    }
    v
}

/// Clean comparisons by function run, over every capture set and float mode.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// Runs the callback each fighter holds at `field` (an offset into `Fighter`) from every saved
/// call of `func`, on both sides; returns the clean comparisons by callback.
fn check_held(func: &str, field: u32, min: usize) -> BTreeMap<u32, usize> {
    let stubs = stubs();
    let mut counts = BTreeMap::new();
    let mut total = 0;
    for dir in dirs() {
        let reports = check_calls_stubbed(
            &dir,
            func,
            0,
            &stubs,
            &|call, _| {
                let cb = peek_u32(call, user_data_arg(call) + field);
                // A fighter without the callback: one both sides have that does little.
                call.addr = if cb != 0 {
                    cb
                } else {
                    leaf_check::sym("ftColl_800764DC")
                };
            },
            &|g, maps, call| {
                let gobj = gobj_arg(maps, call);
                if call.addr == leaf_check::sym("ftColl_800764DC") {
                    g.combo_tick(gobj);
                } else {
                    run_fighter_cb(g, gobj, CodeRef(call.addr));
                }
                Ret::Nothing
            },
        );
        total += reports.iter().map(|r| r.compared).sum::<usize>();
        tally(&reports, &mut counts);
    }
    eprintln!("{func}'s held callbacks compared clean:");
    for (a, n) in &counts {
        eprintln!("  {n:4} {}", name_of(*a));
    }
    if leaf_check::reference().is_some() {
        assert!(
            total >= min,
            "{func}: {total} clean comparisons, expected {min}"
        );
    }
    counts
}

/// Asserts each of `names` compared clean at least once.
fn expect_clean(counts: &BTreeMap<u32, usize>, names: &[&str]) {
    if counts.is_empty() {
        return;
    }
    let missing: Vec<&&str> = names
        .iter()
        .filter(|n| !counts.contains_key(&leaf_check::sym(n)))
        .collect();
    assert!(missing.is_empty(), "never compared clean: {missing:?}");
}

#[test]
fn input_callbacks() {
    let c = check_held("Fighter_procInput", 0x219C, 200);
    expect_clean(
        &c,
        &[
            "ftCo_Wait_IASA",
            "ftCo_Walk_IASA",
            "ftCo_Dash_IASA",
            "ftCo_Run_IASA",
            "ftCo_Jump_IASA",
            "ftCo_JumpAerial_IASA",
            "ftCo_Fall_IASA",
            "ftCo_Landing_IASA",
            "ftCo_KneeBend_IASA",
        ],
    );
}

#[test]
fn anim_callbacks() {
    let c = check_held("Fighter_procAnim", 0x21A0, 200);
    expect_clean(
        &c,
        &["ftCo_Wait_Anim", "ftCo_Run_Anim", "ftCo_Landing_Anim"],
    );
}

#[test]
fn phys_callbacks() {
    let c = check_held("Fighter_procUpdate", 0x21A4, 200);
    expect_clean(&c, &["ftCo_Dash_Phys", "ftCo_Run_Phys", "ftCo_Fall_Phys"]);
}

#[test]
fn coll_callbacks() {
    let c = check_held("Fighter_procMap", 0x21A8, 200);
    expect_clean(&c, &["ftCo_Wait_Coll", "ftCo_Run_Coll", "ftCo_Fall_Coll"]);
}
