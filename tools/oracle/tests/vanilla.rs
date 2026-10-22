//! A VS match a console without Slippi's codes starts from its menus, with an Ice Climber
//! (Nana's computer DI reads her `Fighter*`'s address, `docs/engine.md`): the allocator model,
//! laid out as `Settings::vs_with` lays it out (`Layout::vanilla_vs`), gives the fighters the
//! addresses ssbm-rs's run gives them, and equals memory tick by tick in ssbm-rs's trace.
//!
//! The trace (`TRACE` below, in `$SSBM_RS_LOCAL`) is of ssbm-run booting with a new save
//! through the title screen and the main menu into VS mode's Melee, Ice Climbers on port 1 and
//! Fox on port 2, Fountain of Dreams, every controller at rest from there (`docs/engine.md`
//! has the run). The tests skip without `SSBM_DISC`, and the trace's without the trace.
//!
//! `TRACE_CPU` is the same run with the item switch at none, Fox on both ports and port 2's
//! Fox switched to a computer player (level 1, the select's default) on the character select
//! screen: a match with a computer player lays out the heap the same way.

use std::process::Command;

use melee_data::disc::Disc;
use melee_local_data::ssbm_rs_local;
use melee_sim::game::gm::vs_mode::{GamePrefs, GameRules};
use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};

const TRACE: &str = "traces/vanilla/menus-ics-fox-fod.trc";
/// The same run with the item switch set to none first (VS mode, Custom Rules, Item Switch),
/// as the player's own matches have it (`Settings::vs`).
const TRACE_NO_ITEMS: &str = "traces/vanilla/menus-ics-fox-fod-noitems.trc";
/// Fox against a computer Fox, the item switch at none (`traces/vanilla/menus-cpu-fox-fox-fod`,
/// whose `.holds.txt` has the menus' inputs; every field is held, so the runner's random input
/// never plays).
const TRACE_CPU: &str = "traces/vanilla/menus-cpu-fox-fox-fod.trc";

/// Fountain of Dreams' and Pokémon Stadium's `StKind`s.
const FOUNTAIN: u16 = 2;
const STADIUM: u16 = 3;

/// Ice Climbers on port 1 and Fox on port 2, human players with the select's defaults.
fn players() -> [Player; 2] {
    let p = |character, port| Player {
        character,
        costume: 0,
        port,
        kind: 0,
        cpu_level: 1,
        team: 0,
    };
    [p(14, 0), p(2, 1)]
}

/// The fighters' addresses as the match starts from the menus' new-save rules on `stage`, the
/// model's main heap moved by `shift` bytes; none without the disc.
fn addresses(stage: u16, shift: u32) -> Option<Vec<u32>> {
    let Ok(path) = std::env::var("SSBM_DISC") else {
        eprintln!("no SSBM_DISC: skipping");
        return None;
    };
    let disc = Disc::open(&path).unwrap();
    let mut s = Settings::vs_with(
        stage,
        &players(),
        &GameRules::default(),
        &GamePrefs::default(),
    );
    s.heap
        .as_mut()
        .expect("the allocator model, for the Ice Climbers")
        .lo += shift;
    let mut load = |name: &str| DiscFile {
        entrynum: disc.entrynum(name).unwrap(),
        bytes: disc.read_file(name).unwrap(),
    };
    let m = Match::start(&s, &mut load);
    Some(m.game.ft.addresses.values().copied().collect())
}

/// Popo's, Nana's and Fox's `Fighter*` in ssbm-rs's runs through the menus (the trace's on
/// Fountain of Dreams, and a run that picks Pokémon Stadium the same way), whichever way the
/// runs reach the match; a heap moved by 32 bytes moves them.
#[test]
fn fighters_get_the_consoles_addresses() {
    let Some(fountain) = addresses(FOUNTAIN, 0) else {
        return;
    };
    assert_eq!(fountain, [0x80C7_E000, 0x80CA_F900, 0x80CE_1200]);
    assert_eq!(
        addresses(STADIUM, 0).unwrap(),
        [0x80C6_51C0, 0x80C9_6AC0, 0x80CC_7FA0]
    );
    assert_eq!(
        addresses(FOUNTAIN, 0x20).unwrap(),
        [0x80C7_E020, 0x80CA_F920, 0x80CE_1220]
    );
}

/// `melee-oracle vs-heap` on the trace with the Ice Climbers and Fox: its exit status and
/// output, or none when the trace or the disc isn't here.
fn vs_heap(trace: &str, args: &[&str]) -> Option<(bool, String)> {
    vs_heap_with(trace, &["--player", "0:14", "--player", "1:2"], args)
}

/// [`vs_heap`] with these players.
fn vs_heap_with(trace: &str, players: &[&str], args: &[&str]) -> Option<(bool, String)> {
    let trace = ssbm_rs_local(trace);
    if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
        eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["vs-heap", &trace, "--stage", "2"])
        .args(players)
        .args(["--ticks", "400"])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some((out.status.success(), text))
}

/// The fighters' addresses as the trace's match starts, then the whole allocator model against
/// memory after each of the trace's 304 ticks of play.
#[test]
fn allocator_model_follows_the_trace() {
    let Some((ok, out)) = vs_heap(TRACE, &[]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(
        out.contains("the allocator model's and the trace's: [80c7e000, 80caf900, 80ce1200]"),
        "{out}"
    );
    assert!(
        out.contains("304 ticks compared, to the trace's end"),
        "{out}"
    );
}

/// The check fails on a heap laid out 32 bytes off.
#[test]
fn a_shifted_heap_fails_the_check() {
    let Some((ok, out)) = vs_heap(TRACE, &["--shift-heap", "20"]) else {
        return;
    };
    assert!(!ok, "{out}");
    assert!(out.contains("the fighters' addresses differ"), "{out}");
}

/// With the item switch at none the random items' tables aren't made, so the fighters sit six
/// blocks lower; the model follows the trace's 313 ticks, and a model with the items on fails.
#[test]
fn allocator_model_follows_the_trace_without_items() {
    let Some((ok, out)) = vs_heap(TRACE_NO_ITEMS, &["--item-freq", "none"]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(
        out.contains("the allocator model's and the trace's: [80c7dc20, 80caf520, 80ce0e20]"),
        "{out}"
    );
    assert!(
        out.contains("313 ticks compared, to the trace's end"),
        "{out}"
    );
    let (ok, out) = vs_heap(TRACE_NO_ITEMS, &[]).unwrap();
    assert!(!ok, "{out}");
    assert!(out.contains("the fighters' addresses differ"), "{out}");
}

/// With a computer player (port 2's Fox at level 1) the model lays the heap out the same way:
/// the fighters' addresses and the model after each of the trace's 343 ticks, the computer
/// player playing; settings with a human there instead are refused.
#[test]
fn allocator_model_follows_a_match_with_a_computer_player() {
    let cpu = [
        "--player",
        "0:2",
        "--player",
        "1:2:1",
        "--item-freq",
        "none",
    ];
    let Some((ok, out)) = vs_heap_with(TRACE_CPU, &cpu, &[]) else {
        return;
    };
    assert!(ok, "{out}");
    assert!(
        out.contains("the allocator model's and the trace's: [80c7dc20, 80cc2c00]"),
        "{out}"
    );
    assert!(
        out.contains("343 ticks compared, to the trace's end"),
        "{out}"
    );
    let human = ["--player", "0:2", "--player", "1:2", "--item-freq", "none"];
    let (ok, out) = vs_heap_with(TRACE_CPU, &human, &[]).unwrap();
    assert!(!ok, "{out}");
}
