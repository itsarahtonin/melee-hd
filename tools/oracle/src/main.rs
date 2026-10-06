//! Checks melee-hd's simulation against its references.
//!
//! `melee-oracle compare ORIGINAL.slp OURS.slp` compares a recording's events with the
//! original replay's, field by field, as ssbm-rs checks its own runs (`ssbm_slippi::compare`).
//! The per-tick comparison with ssbm-rs's full game state comes next; see `docs/oracle.md`.

use std::process::ExitCode;

use ssbm_slippi::{compare, replay};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["compare", original, ours] => compare_files(original, ours),
        _ => {
            eprintln!("usage: melee-oracle compare ORIGINAL.slp OURS.slp");
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
