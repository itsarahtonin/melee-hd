//! Checks melee-hd's simulation against its references.
//!
//! `melee-oracle compare ORIGINAL.slp OURS.slp` compares a recording's events with the
//! original replay's, field by field, as ssbm-rs checks its own runs (`ssbm_slippi::compare`).
//!
//! `melee-oracle trace-check TRACE REPLAY.slp` checks an ssbm-rs state trace against the
//! replay it ran: every frame's recorded fighter positions and percents must be in the trace,
//! a tick apart. The comparison with melee-hd's own state builds on these traces; see
//! `docs/oracle.md`.

mod trace;

use std::collections::HashMap;
use std::process::ExitCode;

use ssbm_slippi::{compare, replay};
use trace::Kind;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["compare", original, ours] => compare_files(original, ours),
        ["trace-check", trace, replay] => trace_check(trace, replay),
        _ => {
            eprintln!("usage: melee-oracle compare ORIGINAL.slp OURS.slp");
            eprintln!("       melee-oracle trace-check TRACE REPLAY.slp");
            ExitCode::from(2)
        }
    }
}

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn compare_files(original: &str, ours: &str) -> ExitCode {
    let original_bytes = read(original);
    let original =
        replay::Replay::parse(&original_bytes).unwrap_or_else(|e| panic!("{original}: {e}"));
    let ours_bytes = read(ours);
    let ours = replay::raw_events(&ours_bytes).unwrap_or_else(|e| panic!("{ours}: {e}"));
    let report = compare::compare(&original.events, ours);
    for d in &report.divergences {
        println!("{d}");
    }
    println!(
        "{} events compared through frame {:?}, {} not reached, {} diverge",
        report.compared,
        report.last_frame_compared,
        report.missing,
        report.divergences.len()
    );
    if report.divergences.is_empty() && report.missing == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Fighter fields the trace check reads (`Fighter` in the decomp's ft/types.h).
const FP_PLAYER_IDX: usize = 0xC;
const FP_POS_X: usize = 0xB0;
const FP_POS_Y: usize = 0xB4;
const FP_PERCENT: usize = 0x1830;

/// A fighter's port and the bits of its position and percent.
type Seen = (u8, u32, u32, u32);

fn trace_check(trace_path: &str, replay_path: &str) -> ExitCode {
    let ticks = trace::read(trace_path);
    let bytes = read(replay_path);
    let replay = replay::Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let seen: Vec<Vec<Seen>> = ticks
        .iter()
        .map(|t| {
            t.of(Kind::Fighter)
                .map(|f| {
                    (
                        f.u8(FP_PLAYER_IDX),
                        f.u32(FP_POS_X),
                        f.u32(FP_POS_Y),
                        f.u32(FP_PERCENT),
                    )
                })
                .collect()
        })
        .collect();
    // Post-frame events: each leader's state as its update for the frame ends. A frame
    // recorded again replaces the earlier one, as ssbm-rs's comparison follows the timeline
    // the game settled on.
    let mut by_frame: std::collections::BTreeMap<i32, Vec<Seen>> = Default::default();
    for e in replay
        .events
        .iter()
        .filter(|e| e.command == replay::event::POST_FRAME)
    {
        let p = &e.payload;
        let be = |at: usize| u32::from_be_bytes(p[at..at + 4].try_into().unwrap());
        if p[5] != 0 {
            continue;
        }
        let players = by_frame.entry(be(0) as i32).or_default();
        players.retain(|s| s.0 != p[4]);
        players.push((p[4], be(0x9), be(0xD), be(0x15)));
    }
    let wanted: Vec<(i32, Vec<Seen>)> = by_frame.into_iter().collect();
    let matches = |tick: usize, players: &[Seen]| {
        seen.get(tick)
            .is_some_and(|s| players.iter().all(|p| s.contains(p)))
    };
    let Some((first_frame, _)) = wanted.first() else {
        eprintln!("{replay_path}: no frames");
        return ExitCode::FAILURE;
    };
    // Each frame's recorded state is one tick's: the one the most frames agree with. Fighters
    // stand still at first, so the first frame alone doesn't decide it.
    let agree = |offset: i64| {
        wanted
            .iter()
            .filter(|(f, p)| {
                let t = i64::from(*f) + offset;
                t >= 0 && matches(t as usize, p)
            })
            .count()
    };
    let start = -i64::from(*first_frame);
    let offset = (start - 200..start + 200)
        .max_by_key(|&o| agree(o))
        .unwrap();
    let first_tick = i64::from(*first_frame) + offset;
    let mut checked = 0;
    let mut by_kind: HashMap<&str, usize> = HashMap::new();
    let mut bad = Vec::new();
    for (frame, players) in &wanted {
        let tick = i64::from(*frame) + offset;
        if tick < 0 || tick as usize >= seen.len() {
            *by_kind.entry("past the trace").or_default() += 1;
            continue;
        }
        checked += 1;
        if !matches(tick as usize, players) {
            *by_kind.entry("differ").or_default() += 1;
            bad.push(*frame);
        }
    }
    let first_bad = bad.first().copied();
    if !bad.is_empty() {
        println!("differing frames: {:?}", &bad[..bad.len().min(20)]);
    }
    println!(
        "{} ticks; frame {first_frame} is tick {first_tick}; {checked} frames checked, {} differ, {} past the trace",
        ticks.len(),
        by_kind.get("differ").copied().unwrap_or(0),
        by_kind.get("past the trace").copied().unwrap_or(0)
    );
    for &frame in bad.iter().take(5) {
        let tick = (i64::from(frame) + offset) as usize;
        let players = &wanted.iter().find(|w| w.0 == frame).unwrap().1;
        println!("frame {frame} (tick {tick}):");
        println!("  replay (port, x, y, percent bits): {players:08X?}");
        println!("  trace: {:08X?}", seen[tick]);
    }
    if first_bad.is_some() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
