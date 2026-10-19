//! Widescreen's fighters in the added sides change nothing in the match: a replay played at the
//! game's 73:60 and at 16:9 (the player's `Session::tick`, its frames drawn from a copy when a
//! fighter is off the game's view, `melee_render::wide`) runs the same, every tick's state equal
//! bit for bit (`StateEq`, as the sync test compares), every tick's sounds, rumble and events
//! the same, and every frame's draws the same unless the copy drew them. The widened frames must
//! draw a fighter the game's own pass left out. Skips without `SSBM_DISC`.
//!
//! `WIDE_TEST_REPLAY=FILE.slp` and `WIDE_TEST_TICKS=N` play another replay (with
//! `WIDE_TEST_FP=slippi` for a Slippi replay), listing the frames that draw a fighter more.

use std::time::{Duration, Instant};

use melee_data::disc::Disc;
use melee_sim::game::state::Game;
use melee_sim::hsd::display::Draw;
use melee_sim::input::pad::PadStatus;
use melee_sim::state_eq::StateEq;
use melee_slippi::Replay;

use crate::Session;
use crate::predict_tests::disc;

/// Captain Falcon vs Ganondorf's benchmark match: Captain Falcon flies into the added right
/// side at ticks 4,836 to 4,879.
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/ganon-captain-fd.slp"
);

/// The shapes drawn while a fighter's render callback runs (a fighter's own trees, not its
/// magnifier bubble's).
fn fighter_shapes(game: &Game, draws: &[Draw]) -> usize {
    let mut fighter = false;
    let mut n = 0;
    for d in draws {
        match d {
            Draw::Rendering(g) => {
                fighter = g.is_some_and(|g| game.fighters.iter().any(|(_, f)| f.gobj == Some(g)));
            }
            Draw::MagnifierErase(_) => fighter = false,
            Draw::Shape { .. } if fighter => n += 1,
            _ => {}
        }
    }
    n
}

#[test]
fn widescreen_changes_nothing_in_the_match() {
    let Some(disc) = disc() else { return };
    let fp = match std::env::var("WIDE_TEST_FP").as_deref() {
        Ok("slippi") => melee_sim::gekko_fp::FpMode::Slippi,
        _ => melee_sim::gekko_fp::FpMode::Hardware,
    };
    melee_sim::gekko_fp::set_fp_mode(fp);
    let path = std::env::var("WIDE_TEST_REPLAY").unwrap_or_else(|_| REPLAY.to_owned());
    let ticks: u32 =
        std::env::var("WIDE_TEST_TICKS").map_or(5000, |n| n.parse().expect("WIDE_TEST_TICKS=N"));
    let bytes = std::fs::read(&path).expect("the replay");
    let start = |d: &Disc, widescreen: Option<f32>| {
        let replay = Replay::parse(&bytes).expect("a replay");
        let settings =
            melee_sim::game::vs_match::Settings::from_replay(&melee_slippi::start(&replay))
                .expect("its settings");
        let mut s = Session::begin(d, &settings, Some(replay), false).expect("the match starts");
        s.wide.set(widescreen);
        s
    };
    let mut straight = start(&disc, None);
    let mut wide = start(&disc, Some(16.0 / 9.0));
    let none = [PadStatus::default(); 4];
    let (mut rendered, mut copied, mut more) = (0u32, 0u32, Vec::new());
    let (mut took, mut took_wide) = (Duration::ZERO, Duration::ZERO);
    for tick in 0..ticks {
        let t0 = Instant::now();
        let want = straight.tick(none);
        let t1 = Instant::now();
        let got = wide.tick(none);
        took += t1 - t0;
        took_wide += t1.elapsed();
        if let Some(d) = wide.m.game.state_diff(&straight.m.game) {
            panic!("the state after tick {tick} differs at 16:9 at game{d}");
        }
        if let Some(d) = got.events.state_diff(&want.events) {
            panic!("tick {tick}'s events differ at 16:9 at {d}");
        }
        assert!(
            got.pads.state_diff(&want.pads).is_none() && got.ended == want.ended,
            "tick {tick}'s output differs at 16:9"
        );
        if !want.draws.is_empty() {
            if wide.wide.is_shown() {
                copied += 1;
                let (a, b) = (
                    fighter_shapes(straight.game(), &want.draws),
                    fighter_shapes(wide.game(), &got.draws),
                );
                if b > a {
                    more.push((rendered, tick, b - a));
                }
            } else {
                assert!(
                    got.draws == want.draws,
                    "tick {tick}'s draws differ at 16:9"
                );
            }
            rendered += 1;
        }
        let ended = got.ended;
        straight.m.recycle(want);
        wide.m.recycle(got);
        if ended {
            break;
        }
    }
    eprintln!(
        "ticks took {took:?} at 73:60 and {took_wide:?} at 16:9; {rendered} frames, {copied} \
         drawn from the copy, {} with a fighter drawn more (frame, tick, shapes): {more:?}",
        more.len()
    );
    assert!(
        !more.is_empty(),
        "no frame drew a fighter in the added sides"
    );
}
