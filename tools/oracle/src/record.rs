//! `melee-oracle record REPLAY.slp OUT.slp [--fp hardware|slippi] [--disc-speed fast|drive]
//! [--ticks N] [--known FILE]`: plays a replay with melee-hd's recorder on
//! (`melee_sim::game::record`), writes the recording as a `.slp` file, and compares it with the
//! replay: the frames' events through ssbm-rs's comparison (`ssbm_slippi::compare`, as
//! `compare` does), and the game start and the code list field by field (docs/oracle.md,
//! "Recordings"). `--known` takes the divergences the original code makes playing the same
//! replay, as ssbm-rs keeps them (`local/replays/known/`, one `frame ...` line each): the
//! recording must diverge from the replay in exactly those.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::SystemTime;

use melee_data::disc::Disc;
use melee_sim::audio::clock::DiscSpeed;
use melee_sim::game::playback::FIRST_FRAME;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_slippi::replay::{Event, event};
use melee_slippi::{Device, Replay};
use ssbm_slippi::compare;

use crate::diff::panic_text;

pub fn record(args: &[&str]) -> ExitCode {
    let mut limit = u64::MAX;
    let mut fp_mode = FpMode::Slippi;
    let mut disc_speed = DiscSpeed::Fast;
    let mut known = None;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--fp" => {
                fp_mode = match it.next().copied() {
                    Some("hardware") => FpMode::Hardware,
                    Some("slippi") => FpMode::Slippi,
                    _ => panic!("--fp hardware|slippi"),
                }
            }
            "--disc-speed" => {
                disc_speed = it
                    .next()
                    .and_then(|s| DiscSpeed::parse(s))
                    .expect("--disc-speed fast|drive")
            }
            "--known" => known = Some(*it.next().expect("--known FILE")),
            _ => pos.push(a),
        }
    }
    let [replay_path, out_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle record REPLAY.slp OUT.slp [--fp hardware|slippi] [--disc-speed fast|drive] [--ticks N] [--known FILE]"
        );
        return ExitCode::from(2);
    };
    set_fp_mode(fp_mode);
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let last = replay
        .frames
        .iter()
        .rev()
        .find(|(_, f)| f.complete)
        .map_or(FIRST_FRAME, |(&k, _)| k);
    let mut settings = match Settings::from_replay(&melee_slippi::start(&replay)) {
        Ok(s) => s,
        Err(e) => {
            println!("{replay_path}: refused: {e}");
            return ExitCode::FAILURE;
        }
    };
    settings.disc = disc_speed;
    settings.record = Some(melee_slippi::write::setup(&replay));
    let mut load = |name: &str| DiscFile {
        entrynum: disc
            .entrynum(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
        bytes: disc
            .read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
    };
    let started_at = SystemTime::now();
    let mut m = match catch_unwind(AssertUnwindSafe(|| Match::start(&settings, &mut load))) {
        Ok(m) => m,
        Err(p) => {
            println!("stopped starting the match: {}", panic_text(&p));
            return ExitCode::FAILURE;
        }
    };
    let mut device = Device(&replay);
    let frame = |m: &Match| {
        m.game
            .playback
            .as_ref()
            .map_or(FIRST_FRAME - 1, |p| p.frame_index)
    };
    // Until the game end goes out, the scene ends, or past the replay's last frame (the tick
    // after it decides the outcome).
    let mut tick = 0u64;
    let mut stopped = None;
    while tick < limit {
        let rec = m.game.recorder.as_ref().expect("the recorder");
        if rec.ended() || frame(&m) > last {
            break;
        }
        match catch_unwind(AssertUnwindSafe(|| m.step_replay(&mut device))) {
            Ok(out) => {
                let ended = out.ended;
                m.recycle(out);
                tick += 1;
                if ended {
                    break;
                }
            }
            Err(p) => {
                stopped = Some(format!(
                    "the match stopped in tick {tick} (replay frame {}): {}",
                    frame(&m),
                    panic_text(&p)
                ));
                break;
            }
        }
    }
    let rec = m.game.recorder.as_ref().expect("the recorder");
    let file = melee_slippi::write::recording(rec, started_at);
    std::fs::write(out_path, &file).unwrap_or_else(|e| panic!("{out_path}: {e}"));
    println!(
        "{tick} ticks played to replay frame {}; wrote {out_path} ({} bytes, last frame {:?}, game end {})",
        frame(&m),
        file.len(),
        rec.last_frame(),
        if rec.ended() { "sent" } else { "not sent" }
    );
    if let Some(why) = &stopped {
        println!("{why}");
    }
    let ours = Replay::parse(&file).expect("the recording parses");
    let mut ok = stopped.is_none();
    ok &= compare_frames(&bytes, &file, known);
    ok &= compare_start(&replay, &ours);
    if replay.events == ours.events {
        println!("every event equals the replay's, byte for byte");
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The frames' events through ssbm-rs's comparison: whether nothing diverges or is missing,
/// or with `known`, whether the divergences are exactly the known ones.
fn compare_frames(original: &[u8], file: &[u8], known: Option<&str>) -> bool {
    let original = ssbm_slippi::replay::Replay::parse(original).expect("the replay parses");
    let ours = melee_slippi::replay::raw_events(file).expect("a raw element");
    let report = compare::compare(&original.events, ours);
    let lines: Vec<String> = report.divergences.iter().map(|d| d.to_string()).collect();
    let known: Option<Vec<String>> = known.map(|path| {
        std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
            .lines()
            .filter(|l| l.starts_with("frame "))
            .map(str::to_owned)
            .collect()
    });
    let unexpected: Vec<&String> = lines
        .iter()
        .filter(|l| known.as_ref().is_none_or(|k| !k.contains(l)))
        .collect();
    for d in unexpected.iter().take(40) {
        println!("{d}");
    }
    if unexpected.len() > 40 {
        println!("... {} more", unexpected.len() - 40);
    }
    for (what, frame, port) in &report.first_missing {
        println!("missing: {what} at frame {frame}, port {}", port + 1);
    }
    let mut ok = report.missing == 0 && unexpected.is_empty();
    if let Some(k) = &known {
        let absent: Vec<&String> = k.iter().filter(|l| !lines.contains(l)).collect();
        for l in absent.iter().take(10) {
            println!("known but not diverging: {l}");
        }
        println!(
            "frames: {} known divergences, {} of them here, {} not",
            k.len(),
            k.len() - absent.len(),
            absent.len()
        );
        ok &= absent.is_empty();
    }
    println!(
        "frames: {} events compared through frame {:?}, {} not reached, {} diverge, {} unexplained",
        report.compared,
        report.last_frame_compared,
        report.missing,
        report.divergences.len(),
        unexpected.len()
    );
    ok
}

/// The game start's fields, by offset in its payload (Slippi's spec, `Game Start`).
const START_FIELDS: &[(usize, &str)] = &[
    (0x0, "version"),
    (0x4, "game info block"),
    (0x13C, "random seed"),
    (0x140, "UCF toggles"),
    (0x160, "nametags"),
    (0x1A0, "PAL"),
    (0x1A1, "frozen Pokémon Stadium"),
    (0x1A2, "minor scene"),
    (0x1A3, "major scene"),
    (0x1A4, "display names"),
    (0x220, "connect codes"),
    (0x248, "Slippi UIDs"),
    (0x2BC, "language"),
    (0x2BD, "session ID"),
    (0x2F0, "game number"),
    (0x2F4, "tiebreak number"),
];

fn start_field(at: usize) -> &'static str {
    START_FIELDS
        .iter()
        .rev()
        .find(|(o, _)| *o <= at)
        .map_or("?", |f| f.1)
}

fn first(r: &Replay, command: u8) -> Option<&Event> {
    r.events.iter().find(|e| e.command == command)
}

/// The game start, field by field, and the code list: whether they're equal. Every differing
/// field is listed (a block's bytes by offset).
fn compare_start(replay: &Replay, ours: &Replay) -> bool {
    let (Some(a), Some(b)) = (
        first(replay, event::GAME_START),
        first(ours, event::GAME_START),
    ) else {
        println!("game start: missing");
        return false;
    };
    let n = a.payload.len().min(b.payload.len());
    let mut diffs: Vec<(&str, Vec<usize>)> = Vec::new();
    for i in 0..n {
        if a.payload[i] != b.payload[i] {
            let name = start_field(i);
            match diffs.last_mut() {
                Some((f, at)) if *f == name => at.push(i),
                _ => diffs.push((name, vec![i])),
            }
        }
    }
    for (name, at) in &diffs {
        let show = |p: &[u8]| {
            at.iter()
                .take(12)
                .map(|&i| format!("{:02X}", p[i]))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let offsets: Vec<String> = at.iter().take(12).map(|i| format!("{i:#x}")).collect();
        println!(
            "game start: {name} at {}{}: original {}, ours {}",
            offsets.join(","),
            if at.len() > 12 { ",..." } else { "" },
            show(&a.payload),
            show(&b.payload)
        );
    }
    if a.payload.len() != b.payload.len() {
        println!(
            "game start: {} bytes, ours {}",
            a.payload.len(),
            b.payload.len()
        );
    }
    let codes = replay.gecko_codes == ours.gecko_codes;
    println!(
        "game start: {} fields differ; code list {} ({} bytes)",
        diffs.len(),
        if codes { "equal" } else { "differs" },
        ours.gecko_codes.len()
    );
    diffs.is_empty() && codes && a.payload.len() == b.payload.len()
}
