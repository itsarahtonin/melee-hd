//! `melee-oracle vs-heap`: a VS match a console without Slippi's codes starts from its menus,
//! against ssbm-rs's trace of one. There is no replay to play: the match starts from the
//! settings the menus chose (`Settings::vs_with`) and plays with every controller at rest, as
//! the trace's run held them. The fighters' addresses at the match's first tick must be the
//! trace's, and the allocator model must equal memory after each tick it runs.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use melee_capture::DataMap;
use melee_capture::compare::Comparer;
use melee_capture::import::{Maps, sym};
use melee_data::disc::Disc;
use melee_sim::game::gm::vs_mode::{GamePrefs, GameRules};
use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};
use melee_sim::input::pad::{ERR_NONE, PadStatus};

use crate::trace::{Image, Kind, Reader};

/// The trace tick that ran the match's first engine tick: the scene loop has counted one
/// tick, and the players' slots hold fighters (the menus' scenes before it hold none).
fn is_first_tick(image: &Image) -> bool {
    image.u32(sym("gm_80479D58")) == 1 && !crate::diff::fighter_addresses(image).is_empty()
}

pub fn vs_heap(args: &[&str]) -> ExitCode {
    let (mut stage, mut players, mut ticks, mut shift) = (None, Vec::new(), 0u32, 0u32);
    let mut prefs = GamePrefs::default();
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--stage" => stage = it.next().and_then(|s| s.parse::<u16>().ok()),
            // PORT:CKIND, the port from 0, a human player in the costume and team defaults.
            "--player" => {
                let spec = it.next().expect("--player PORT:CKIND");
                let (port, ckind) = spec.split_once(':').expect("--player PORT:CKIND");
                players.push(Player {
                    character: ckind.parse().expect("--player's CharacterKind"),
                    costume: 0,
                    port: port.parse().expect("--player's port"),
                    kind: 0,
                    cpu_level: 1,
                    team: 0,
                });
            }
            "--ticks" => ticks = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            // The item switch's frequency as the menus set it (0 to 4, or `none`), else a new
            // save's (medium).
            "--item-freq" => {
                prefs.item_freq = match it.next().copied() {
                    Some("none") => 0xFF,
                    Some(f) => f.parse().expect("--item-freq 0-4|none"),
                    None => panic!("--item-freq 0-4|none"),
                }
            }
            // Moves the model's main heap by this many bytes, to see the check fail.
            "--shift-heap" => {
                shift = it
                    .next()
                    .and_then(|t| u32::from_str_radix(t.trim_start_matches("0x"), 16).ok())
                    .expect("--shift-heap BYTES (hex)")
            }
            _ => pos.push(a),
        }
    }
    let (&[trace_path], Some(stage)) = (&pos[..], stage) else {
        eprintln!(
            "usage: melee-oracle vs-heap TRACE --stage STKIND --player PORT:CKIND... [--item-freq F] [--ticks N]"
        );
        return ExitCode::from(2);
    };
    let mut settings = Settings::vs_with(stage, &players, &GameRules::default(), &prefs);
    let Some(layout) = settings.heap.as_mut() else {
        eprintln!("the match runs without the allocator model (no Ice Climber)");
        return ExitCode::FAILURE;
    };
    layout.lo += shift;
    // ssbm-run computes a run without a replay in the hardware's floats.
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let mut reader = Reader::open(trace_path);
    let mut image = Image::default();
    // The random seed as the menus left it, at the end of the tick before the match's scene:
    // the scene's setup draws from it.
    let (mut first, mut seed) = (None, None);
    while let Some(t) = reader.tick() {
        image.apply(&t);
        if image.seen && is_first_tick(&image) {
            first = Some(t.number);
            break;
        }
        seed = t.of(Kind::Seed).next().map(|r| r.u32(0));
    }
    let (Some(first), Some(seed)) = (first, seed) else {
        eprintln!("{trace_path}: no tick runs a match's first, or no memory in the trace");
        return ExitCode::FAILURE;
    };
    settings.seed = seed;
    // The save data's unlocks, which the stage's music choice reads (on some stages it decides
    // whether that draws from the seed): the trace's console runs no unlock codes.
    settings.unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| image.u32(a));
    let started = catch_unwind(AssertUnwindSafe(|| {
        let mut load = |name: &str| DiscFile {
            entrynum: disc
                .entrynum(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
            bytes: disc
                .read_file(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
        };
        Match::start(&settings, &mut load)
    }));
    let mut m = match started {
        Ok(m) => m,
        Err(p) => {
            println!(
                "stopped starting the match: {}",
                crate::diff::panic_text(&p)
            );
            return ExitCode::FAILURE;
        }
    };
    let traced = crate::diff::fighter_addresses(&image);
    let modelled: Vec<u32> = m.game.ft.addresses.values().copied().collect();
    if modelled != traced {
        println!(
            "the fighters' addresses differ: the allocator model's {modelled:x?}, the trace's {traced:x?} (tick {first})"
        );
        return ExitCode::FAILURE;
    }
    println!("the fighters' addresses, the allocator model's and the trace's: {modelled:x?}");
    // Every controller connected and at rest, as the trace's run held them.
    let pads = [PadStatus {
        err: ERR_NONE,
        ..PadStatus::default()
    }; 4];
    let data = DataMap { loaded: Vec::new() };
    let mut tick = first;
    for n in 0..ticks {
        if n > 0 {
            match reader.tick() {
                Some(t) => {
                    image.apply(&t);
                    tick = t.number;
                }
                None => {
                    println!("{n} ticks compared, to the trace's end");
                    return ExitCode::SUCCESS;
                }
            }
        }
        // The trace samples each tick before its frame's display pass, which runs after the
        // comparison.
        let stepped = catch_unwind(AssertUnwindSafe(|| m.step_tick(pads).1));
        let render = match stepped {
            Ok(render) => render,
            Err(p) => {
                println!(
                    "stopped in the match's tick {} (trace tick {tick}): {}",
                    n + 1,
                    crate::diff::panic_text(&p)
                );
                return ExitCode::FAILURE;
            }
        };
        let mut cmp = Comparer::new(&image.mem1, &data, &m.game, Maps::default());
        cmp.heap();
        if !cmp.diffs.is_empty() {
            println!(
                "the allocator model differs from memory after the match's tick {} (trace tick {tick}):",
                n + 1
            );
            for d in cmp.diffs.iter().take(20) {
                println!("  {d}");
            }
            return ExitCode::FAILURE;
        }
        if render && catch_unwind(AssertUnwindSafe(|| m.render())).is_err() {
            println!("stopped in the match's display pass after tick {}", n + 1);
            return ExitCode::FAILURE;
        }
    }
    println!("{ticks} ticks compared, the allocator model equal to memory after each");
    ExitCode::SUCCESS
}
