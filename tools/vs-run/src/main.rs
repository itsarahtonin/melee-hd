//! Plays a Slippi replay into melee-sim's VS match and reports how far it gets.
//!
//! `melee-vs-run DISC REPLAY.slp [--from CAPTURE.call] [--frames N]`
//!
//! The match plays the replay as a replay run of the original does: Slippi's playback codes
//! set it up from the replay's game start and feed each frame's recorded inputs to the
//! fighters (`game::playback`), with the rules the replay's codes make (`game::slippi`).
//! Without `--from`, the match starts from the replay's settings through
//! `vs_match::Match::start`, loading files from the disc. With `--from`, it takes up a state
//! ssbm-rs captured while playing the same replay, at a call of
//! `gm_EvaluateAllControllerInputs` (the button maps, partway into an engine tick; see
//! `tools/leaf-check/tests/vs.rs` for how to make captures): it finishes that tick, then steps
//! on. Either way it runs engine ticks until `--frames` run out, the match ends, or the match
//! reaches code no port covers yet, which it names.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, disc_index, find_archives_in};
use melee_data::archive::{Archive, Archives};
use melee_data::disc::Disc;
use melee_replay::Device;
use melee_sim::game::gm::scene::{self, Resume};
use melee_sim::game::playback::Playback;
use melee_sim::game::state::Running;
use melee_sim::game::vs_match::{Match, Settings};
use ssbm_slippi::replay::Replay;

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

fn run(args: Vec<String>) -> ExitCode {
    let (mut from, mut frames) = (None, u32::MAX);
    let mut pos = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = it.next(),
            "--frames" => frames = it.next().and_then(|n| n.parse().ok()).expect("--frames N"),
            _ => pos.push(a),
        }
    }
    let [disc_path, replay_path] = &pos[..] else {
        eprintln!("usage: melee-vs-run DISC REPLAY.slp [--from CAPTURE.call] [--frames N]");
        return ExitCode::from(2);
    };
    let disc = Disc::open(disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let settings = Settings::from_replay(&melee_replay::start(&replay))
        .unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    println!(
        "replay: stage {}, {} frames, seed {:#010x}",
        settings.start.rules.stkind,
        replay.frames.len(),
        settings.seed
    );
    let started = catch_unwind(AssertUnwindSafe(|| match &from {
        None => {
            let mut load = |name: &str| {
                let file = disc
                    .read_file(name)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                Archive::parse(&file).unwrap_or_else(|e| panic!("{name}: {e:?}"))
            };
            Match::start(&settings, &mut load)
        }
        Some(path) => {
            let cap = Capture::read(std::path::Path::new(path));
            let mut data = Archives::default();
            let map = DataMap {
                loaded: find_archives_in(&disc_index(&disc), &cap.call.mem1, &mut data),
            };
            let mut imp = Importer::new(&cap, &map);
            imp.world();
            imp.stage();
            imp.ids();
            imp.match_state();
            println!(
                "took up {path}: {} GObjs, {} pointers the importer doesn't model",
                imp.game.world.gobjs.used(),
                imp.unhandled.len()
            );
            assert!(
                imp.game.playback.is_some(),
                "{path}: not a capture of a replay run"
            );
            let mut m = Match::resume(imp.game, data);
            let answers = Playback::answers(&m.game, &mut Device(&replay));
            m.game.playback.as_mut().unwrap().answers = Some(answers);
            scene::finish_tick(
                &mut Running {
                    game: &mut m.game,
                    data: &m.data,
                },
                Resume::ButtonMaps,
            );
            m
        }
    }));
    let mut m = match started {
        Ok(m) => m,
        Err(p) => {
            println!("stopped before the first frame: {}", panic_text(&p));
            return ExitCode::FAILURE;
        }
    };
    println!("match running");
    let mut stepped = 0;
    let mut events = 0;
    let mut device = Device(&replay);
    while stepped < frames {
        match catch_unwind(AssertUnwindSafe(|| m.step_replay(&mut device))) {
            Ok(f) => {
                stepped += 1;
                events += f.events.len();
                if f.ended {
                    println!("the match ended after {stepped} frames");
                    break;
                }
            }
            Err(p) => {
                println!("stopped after {stepped} frames: {}", panic_text(&p));
                return ExitCode::FAILURE;
            }
        }
    }
    println!("{stepped} frames run, {events} events");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The simulation recurses as the original does (joint trees): a deep stack.
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(args))
        .unwrap()
        .join()
        .unwrap()
}
