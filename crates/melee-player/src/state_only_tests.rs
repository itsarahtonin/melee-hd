//! The state-only display pass (`Display::state_only`, docs/refactor.md's task 9) writes the
//! same state as the full one: a match and a session played side by side, one with the flag on
//! and one with it off, run the same, every tick's sounds, rumble, events and mixed sound the
//! same, and every tick's state the same bit for bit (`StateEq`, as the sync test compares) but
//! for the pass's output (`Display::OUTPUT`: the draws' snapshots and the bookkeeping that
//! names them, which each screen pass starts afresh and only the draws read). The state-only
//! pass builds no draw. Plays the Fox against Marth benchmark match on Pokémon Stadium (its
//! screens, their text and TEVs) and Captain Falcon against Ganondorf on Final Destination,
//! and `pause-fd`, a session under the Slippi ruleset through its menus, its live match's
//! handoff and handback. Skips without `SSBM_DISC` (the session without `SSBM_RS_LOCAL`).
//!
//! `STATE_ONLY_TEST_REPLAYS=A.slp;B.slp` (the platform's path list) plays those replays instead
//! of the benchmark matches, with `STATE_ONLY_TEST_FP=slippi` for Slippi replays.

use melee_data::disc::Disc;
use melee_sim::hsd::display::Display;
use melee_sim::input::pad::PadStatus;
use melee_sim::state_eq::StateEq;
use melee_slippi::Replay;

use crate::Session;
use crate::predict_tests::disc;

const BENCH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bench/");

/// What playing both ways found.
#[derive(Debug, Default)]
struct Played {
    ticks: u32,
    /// Ticks whose full pass built draws.
    drew: u32,
    /// Ticks after which the pass's output differed (the state-only pass's left as the
    /// screen pass started it).
    output_differed: u32,
    /// Ticks in a live match (a session's).
    live: u32,
}

/// Plays `full` and `quiet` (the state-only pass's) side by side for at most `limit` ticks or
/// to the end, comparing every tick; fails on any difference but in the pass's output.
fn compare(what: &str, mut full: Session, mut quiet: Session, limit: u32) -> Played {
    quiet.game_mut().objs.display.state_only = true;
    let none = [PadStatus::default(); 4];
    let mut played = Played::default();
    let mut diffs = Vec::new();
    for tick in 0..limit {
        let want = full.tick(none);
        let got = quiet.tick(none);
        assert!(
            quiet.game().objs.display.state_only,
            "{what}: tick {tick} left the state-only pass"
        );
        assert!(
            got.draws.is_empty(),
            "{what}: tick {tick}'s state-only pass built {} draws",
            got.draws.len()
        );
        played.drew += u32::from(!want.draws.is_empty());
        if let Some(d) = got.events.state_diff(&want.events) {
            panic!("{what}: tick {tick}'s events differ with the state-only pass at {d}");
        }
        assert!(
            got.pads.state_diff(&want.pads).is_none() && got.ended == want.ended,
            "{what}: tick {tick}'s output differs with the state-only pass"
        );
        if let (Some(a), Some(b)) = (full.sound.as_ref(), quiet.sound.as_ref()) {
            assert!(
                a.samples() == b.samples(),
                "{what}: tick {tick}'s mixed sound differs with the state-only pass"
            );
        }
        diffs.clear();
        quiet.game().state_diffs(full.game(), &mut diffs);
        if let Some(d) = diffs.iter().find(|d| !Display::is_output(&d.path())) {
            panic!(
                "{what}: the state after tick {tick} differs with the state-only pass at game{d}"
            );
        }
        played.output_differed += u32::from(!diffs.is_empty());
        played.live += u32::from(quiet.in_live_match());
        played.ticks = tick + 1;
        let ended = full.ended || quiet.ended;
        assert_eq!(full.ended, quiet.ended, "{what}: tick {tick}'s end differs");
        full.recycle(want);
        quiet.recycle(got);
        if ended {
            break;
        }
    }
    eprintln!("{what}: {played:?}");
    assert!(played.drew > 0, "{what}: the full pass drew nothing");
    assert!(
        played.output_differed > 0,
        "{what}: the state-only pass left the pass's output as the full one did"
    );
    played
}

/// A replay's match both ways, to its end.
fn replay(disc: &Disc, path: &str) -> Played {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let start = || {
        let replay = Replay::parse(&bytes).expect("a replay");
        let settings =
            melee_sim::game::vs_match::Settings::from_replay(&melee_slippi::start(&replay))
                .expect("its settings");
        Session::begin(disc, &settings, Some(replay), false).expect("the match")
    };
    compare(path, start(), start(), u32::MAX)
}

#[test]
fn the_state_only_pass_writes_the_same_state_in_matches() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        let fp = match std::env::var("STATE_ONLY_TEST_FP").as_deref() {
            Ok("slippi") => melee_sim::gekko_fp::FpMode::Slippi,
            _ => melee_sim::gekko_fp::FpMode::Hardware,
        };
        melee_sim::gekko_fp::set_fp_mode(fp);
        let replays: Vec<String> = match std::env::var_os("STATE_ONLY_TEST_REPLAYS") {
            Some(list) => std::env::split_paths(&list)
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            None => ["fox-marth-ps.slp", "ganon-captain-fd.slp"]
                .iter()
                .map(|n| format!("{BENCH}{n}"))
                .collect(),
        };
        for path in &replays {
            replay(&disc, path);
        }
    });
}

#[test]
fn the_state_only_pass_writes_the_same_state_in_a_session() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = crate::live_tests::pause_fd() else {
            melee_local_data::skip!("skipped: no disc or no ruleset sessions");
            return;
        };
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.bench = Some(0);
        let played = compare(
            "pause-fd",
            crate::live_tests::powered_on(&args, &path),
            crate::live_tests::powered_on(&args, &path),
            u32::MAX,
        );
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        // Through the menus, the live match and back.
        assert!(
            played.live > 0 && played.ticks > played.live,
            "pause-fd: {played:?}"
        );
    });
}
