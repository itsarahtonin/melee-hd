//! Plays a Slippi replay's inputs into melee-sim's VS match and reports how far it gets.
//!
//! `melee-vs-run DISC REPLAY.slp [--from CAPTURE.call] [--frames N]`
//!
//! Without `--from`, the match starts from the replay's settings (its game info block and
//! random seed) through `vs_match::Match::start`, loading files from the disc. With `--from`,
//! it takes up a state ssbm-rs captured at a call of `gm_EvaluateAllControllerInputs` (the
//! button maps, partway into an engine tick; see `tools/leaf-check/tests/vs.rs` for how to
//! make captures): it finishes that tick, then steps on. Either way each frame queues the
//! replay's inputs for the frame and runs an engine tick, until the replay ends, `--frames`
//! run out, the match ends, or the match reaches code no port covers yet, which it names.
//!
//! The inputs are an approximation of what Slippi's playback codes inject (they write the
//! recorded, processed inputs into the game's pad statuses, and those codes aren't ported): a
//! poll per frame built from the pre-frame's raw sticks, processed buttons and trigger.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, disc_index, find_archives_in};
use melee_data::archive::{Archive, Archives};
use melee_data::disc::Disc;
use melee_sim::game::gm::scene::{self, Resume};
use melee_sim::game::state::Running;
use melee_sim::game::vs_match::{Match, Settings};
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{ERR_NO_CONTROLLER, ERR_NONE, PadStatus};
use ssbm_slippi::replay::{GAME_FIRST_FRAME, PlayerFrame, Replay};

/// The bits of a pre-frame's processed buttons that are the controller's own.
const PAD_BUTTONS: u32 = 0x1F7F;

/// One poll of the four ports from a replay frame's pre-frame updates.
fn poll(frame: Option<&ssbm_slippi::replay::Frame>) -> PadData {
    core::array::from_fn(|port| match frame.and_then(|f| f.players[port].as_ref()) {
        Some(p) => status(p),
        None => PadStatus::error(ERR_NO_CONTROLLER),
    })
}

fn status(p: &PlayerFrame) -> PadStatus {
    let trigger = (p.trigger.clamp(0.0, 1.0) * 140.0) as u8;
    PadStatus {
        button: (p.buttons & PAD_BUTTONS) as u16,
        stick_x: p.joystick_x_raw as i8,
        stick_y: p.joystick_y_raw as i8,
        substick_x: p.cstick_x_raw as i8,
        substick_y: p.cstick_y_raw as i8,
        trigger_left: trigger,
        trigger_right: 0,
        analog_a: 0,
        analog_b: 0,
        err: ERR_NONE,
    }
}

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
    let info: Vec<u8> = replay.header.iter().flat_map(|w| w.to_be_bytes()).collect();
    let settings = Settings::from_game_info(&info, replay.random_seed);
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
            let mut m = Match::resume(imp.game, data);
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
    for (n, frame) in (GAME_FIRST_FRAME..).enumerate() {
        if n as u32 >= frames || !replay.frames.contains_key(&frame) {
            break;
        }
        let pads = poll(replay.frames.get(&frame));
        match catch_unwind(AssertUnwindSafe(|| m.step(pads))) {
            Ok(f) => {
                stepped += 1;
                events += f.events.len();
                if f.ended {
                    println!("the match ended after {stepped} frames");
                    break;
                }
            }
            Err(p) => {
                println!(
                    "stopped in frame {frame} after {stepped} frames: {}",
                    panic_text(&p)
                );
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
