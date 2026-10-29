//! Extrapolate's prediction has no side effects: a match played with the next tick predicted
//! after every tick (the player's `Session::predict`, `melee_render::predict`) runs the same as
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
        eprintln!("skipped: SSBM_DISC names no disc");
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
        let ended = got.ended;
        straight.recycle(want);
        predicting.recycle(got);
        if ended {
            break;
        }
        if rendered {
            predicting
                .predict(&mut predictor, p)
                .unwrap_or_else(|why| panic!("tick {tick}'s prediction stopped: {why}"));
            seen.made += u32::from(predictor.is_filled());
            let copy = predictor.predicted().expect("the copy");
            assert!(copy.game().audio.mix.is_none(), "the copy logs its mix");
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
            character: roster::character(key).expect("ported").kind,
            costume: 0,
            port,
            kind: 0,
            cpu_level: 0,
            team: port,
        };
        let players = [player("fox", 0), player("marth", 1)];
        let stage = roster::stage("fd").expect("ported").kind;
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
