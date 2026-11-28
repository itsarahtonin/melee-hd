//! Extrapolate's prediction has no side effects: a match played with the next tick predicted
//! after every tick (the player's `Session::predict`, `melee_render::predict`; every other one on
//! the prediction's own thread beside a frame built from the real state, as the player makes it
//! beside a tick's first frame, `Session::predict_beside`) runs the same as
//! one played straight, every tick's state equal bit for bit (`StateEq`, as the sync test
//! compares), every tick's output (sounds, rumble, draws) the same, and its sound mixed to the
//! same samples. Plays the Fox ditto's benchmark replay, whose predictions come out exact, and a
//! match of the player's own with scripted controllers, whose predictions with the controllers
//! held miss where an input changes. Skips without `SSBM_DISC`.

use melee_data::disc::Disc;
use melee_render::predict::Predictor;
use melee_sim::game::roster;
use melee_sim::game::vs_match::{Player, Settings};
use melee_sim::input::pad::PadStatus;
use melee_sim::state_eq::StateEq;
use melee_slippi::Replay;

use crate::Session;

pub(crate) const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/fox-fox-fd.slp"
);

pub(crate) fn disc() -> Option<Disc> {
    let Some(path) = std::env::var_os("SSBM_DISC") else {
        melee_local_data::skip!("skipped: SSBM_DISC names no disc");
        return None;
    };
    Some(Disc::open(&path).expect("the disc"))
}

/// How the predictions compared with the ticks that followed.
#[derive(Debug, Default)]
struct Predictions {
    made: u32,
    exact: u32,
    missed: u32,
    /// Predictions a console's tick stopped (a disc read).
    stopped: u32,
    /// Sound samples mixed in a live match's ticks, and how many of them weren't silent.
    live_samples: usize,
    live_audible: usize,
}

/// Plays `ticks` ticks of two copies of a match, `straight` and `predicting`, with the
/// controllers `pads` gives each tick, and after each of `predicting`'s ticks predicts the next
/// with the controllers held. Fails on the first difference between them: in the state, in a
/// tick's output, or in its mixed sound.
fn compare(
    mut straight: Session,
    mut predicting: Session,
    ticks: u32,
    pads: impl Fn(u32) -> [PadStatus; 4],
) -> Predictions {
    let mut predictor = Predictor::<crate::power::Run>::default();
    // The real tick's frame, built beside the predictions made on their own thread.
    let mut frames = melee_render::frame::Frames::new();
    frames.space.load_dol(&predicting.dol);
    let mut drawn = Vec::new();
    let mut seen = Predictions::default();
    let mut samples = 0usize;
    for tick in 0..ticks {
        let p = pads(tick);
        let want = straight.tick(p);
        let got = predicting.tick(p);
        if let Some(d) = got.state_diff(&want) {
            panic!("tick {tick}'s output differs from the straight run's at frame{d}");
        }
        let (a, b) = (
            straight.sound.as_ref().expect("sound"),
            predicting.sound.as_ref().expect("sound"),
        );
        assert!(
            a.samples() == b.samples(),
            "tick {tick}'s mixed sound differs from the straight run's"
        );
        samples += a.samples().len();
        if predicting.in_live_match() {
            seen.live_samples += b.samples().len();
            seen.live_audible += b.samples().iter().filter(|s| **s != [0, 0]).count();
        }
        if let Some(d) = predicting.game().state_diff(straight.game()) {
            panic!("the state after tick {tick} differs from the straight run's at game{d}");
        }
        // The last prediction against the tick it predicted.
        if predictor.is_filled() {
            let copy = predictor.predicted().expect("the copy");
            match copy.game().state_diff(predicting.game()) {
                None => seen.exact += 1,
                Some(_) => seen.missed += 1,
            }
        }
        let rendered = !got.draws.is_empty();
        drawn.clone_from(&got.draws);
        // The session's end (a console's frame ends only its scene).
        let ended = predicting.ended;
        straight.recycle(want);
        predicting.recycle(got);
        if ended {
            break;
        }
        if rendered {
            // A played-back session's controllers are its next read, held.
            let held = (predicting.console.as_ref())
                .and_then(|c| c.feed.as_ref())
                .map_or(p, |(reads, next)| reads[(*next).min(reads.len() - 1)]);
            // As the player makes them: on odd ticks on the prediction's own thread beside the
            // tick's first frame, built here from the real state meanwhile (the player's t = 0
            // frame); on even ticks on this thread, as a frame at a later t makes it.
            let made = if tick % 2 == 1 {
                let real = predicting.game();
                let (made, stream, _) = predicting.predict_beside(&mut predictor, held, || {
                    frames.frame(real, predicting.data(), &drawn)
                });
                assert!(
                    !stream.is_empty(),
                    "tick {tick}: no frame beside the prediction"
                );
                made
            } else {
                predicting.predict(&mut predictor, held)
            };
            if let Err(why) = made {
                // A console's prediction stops where the tick would read the disc (a scene's
                // song), and the frames draw the real tick (`power::step_predicted`).
                assert!(
                    predicting.console.is_some(),
                    "tick {tick}'s prediction stopped: {why}"
                );
                seen.stopped += 1;
                predictor.forget();
                continue;
            }
            seen.made += u32::from(predictor.is_filled());
            let copy = predictor.predicted().expect("the copy");
            assert!(copy.game().audio.mix.is_none(), "the copy logs its mix");
            // A live match too: it plays on the console's audio, its mix log with it.
            assert!(
                predicting.game().audio.mix.is_some(),
                "the match's mix log is off"
            );
            // Nothing written back.
            if let Some(d) = predicting.game().state_diff(straight.game()) {
                panic!(
                    "the state after tick {tick}'s prediction differs from the straight run's at game{d}"
                );
            }
        } else {
            predictor.forget();
        }
    }
    assert!(samples > 0, "no sound mixed");
    eprintln!("{seen:?}, {samples} samples mixed alike");
    seen
}

#[test]
fn predicting_a_replay_changes_nothing() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let bytes = std::fs::read(REPLAY).expect("the benchmark replay");
        let start = |d: &Disc| {
            let replay = Replay::parse(&bytes).expect("a replay");
            let settings =
                Settings::from_replay(&melee_slippi::start(&replay)).expect("its settings");
            Session::begin(d, &settings, Some(replay), false).expect("the match starts")
        };
        let none = [PadStatus::default(); 4];
        let seen = compare(start(&disc), start(&disc), 1800, |_| none);
        // A replay's inputs are known: every prediction is the tick that followed it.
        assert!(seen.made > 1700, "{seen:?}");
        assert_eq!(seen.missed, 0, "{seen:?}");
        assert!(seen.exact + 1 >= seen.made, "{seen:?}");
    });
}

#[test]
fn predicting_with_held_controllers_changes_nothing() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let player = |key: &str, port: u8| Player {
            character: roster::character(key).expect("ported").vs_kind(),
            costume: 0,
            port,
            kind: 0,
            cpu_level: 0,
            team: port,
        };
        let players = [player("fox", 0), player("marth", 1)];
        let stage = roster::stage("fd").expect("ported").vs_kind();
        let start = |d: &Disc| {
            let settings = Settings::vs(stage, &players, 4, 8);
            Session::begin(d, &settings, None, false).expect("the match starts")
        };
        // Each port's stick and buttons change every few ticks: running, turning, jabs and jumps.
        let pads = |tick: u32| {
            let mut p = [PadStatus::default(); 4];
            for (port, pad) in p.iter_mut().enumerate().take(2) {
                let phase = tick / (11 + 4 * port as u32);
                pad.stick_x = [0, 90, 90, -90, 0, -60][(phase % 6) as usize];
                if phase % 7 == 3 && tick % 4 < 2 {
                    pad.button |= 0x0100;
                }
                if phase % 5 == 2 && tick % 9 < 3 {
                    pad.button |= 0x0800;
                }
            }
            p
        };
        let seen = compare(start(&disc), start(&disc), 900, pads);
        assert!(seen.made > 800, "{seen:?}");
        // Held controllers predict most ticks exactly and miss where an input changed.
        assert!(seen.missed > 10, "{seen:?}");
        assert!(seen.exact > seen.missed, "{seen:?}");
    });
}

/// A session under the Slippi ruleset predicts as it runs, its live match included: the
/// prediction's copy steps a live match's tick and display pass as the session does (its own
/// game, not the menus'), and nothing reaches the session; a song playing doesn't stop it.
/// `pause-fd`'s reads drive both sessions, through its live match's handoff and handback; a
/// prediction holds the recording's next read.
/// Skips without `SSBM_DISC` and `SSBM_RS_LOCAL`.
#[test]
fn predicting_a_ruleset_session_changes_nothing() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = crate::live_tests::pause_fd() else {
            melee_local_data::skip!("skipped: no disc or no ruleset sessions");
            return;
        };
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.bench = Some(0);
        let none = [PadStatus::default(); 4];
        let seen = compare(
            crate::live_tests::powered_on(&args, &path),
            crate::live_tests::powered_on(&args, &path),
            3100,
            |_| none,
        );
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        // Every tick predicted but a scene's entry's (its files are the real tick's to read),
        // each the tick that followed it.
        assert!(seen.made > 3000, "{seen:?}");
        assert_eq!((seen.stopped, seen.missed), (0, 0), "{seen:?}");
        // The live match plays its sound (the console's audio, carried into it): mixed, and
        // not silence.
        assert!(seen.live_samples > 0, "{seen:?}");
        assert!(seen.live_audible > seen.live_samples / 10, "{seen:?}");
    });
}
