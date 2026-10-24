//! Interpolate and Extrapolate across a tick that draws nothing (Melee's render skip, about
//! every 1001 ticks, [`crate::paced`]). Plays the Fox ditto's benchmark replay through the
//! player's [`Paced`] and builds each frame's GX stream beside Off's: every frame the paced
//! modes draw at their ends (Interpolate at t = 1, Extrapolate at t = 0) is Off's frame command
//! for command; on the tick that draws nothing they draw nothing and the image shown holds, as
//! Off presents nothing; Extrapolate's image held through it is the tick before's own; and the
//! next tick's Interpolate blends from the last tick that drew. It also shows why holding
//! matters: the tick before's draws on the skipped tick's state make a different stream from
//! the image they drew. Skips without `SSBM_DISC`.

use std::time::Instant;

use melee_render::blend::{Blend, Capture};
use melee_render::frame::Frames;
use melee_sim::game::state::Game;
use melee_sim::hsd::display::Draw;
use melee_sim::input::pad::PadStatus;

use crate::Session;
use crate::paced::Paced;
use crate::pacing::Mode;
use crate::predict_tests::{REPLAY, disc};

/// The bench replay's first tick that draws nothing (ticks counted from 0).
const SKIP: u32 = 1986;

/// A frame builder with no history, for streams compared one against one.
fn fresh(dol: &[u8]) -> Frames {
    let mut f = Frames::new();
    f.space.load_dol(dol);
    f
}

#[test]
fn paced_frames_hold_through_a_tick_that_draws_nothing() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let bytes = std::fs::read(REPLAY).expect("the benchmark replay");
        let replay = melee_slippi::Replay::parse(&bytes).expect("a replay");
        let settings =
            melee_sim::game::vs_match::Settings::from_replay(&melee_slippi::start(&replay))
                .expect("its settings");
        let mut session = Session::begin(&disc, &settings, Some(replay), false).expect("the match");
        let dol = session.dol.clone();
        let none = [PadStatus::default(); 4];
        // Off's frames, and Interpolate's and Extrapolate's at their ends, each built by a frame
        // builder of its own that builds one frame for each tick that draws, as Off's does.
        let (mut off, mut at_1, mut at_0) = (fresh(&dol), fresh(&dol), fresh(&dol));
        let (mut interpolate, mut extrapolate) = (Paced::default(), Paced::default());
        // The last tick that drew: its state and draws.
        let mut last: Option<(Game, Vec<Draw>)> = None;
        // The last tick that drew whose next tick Extrapolate couldn't predict (it drew nothing).
        let mut unpredicted = None;
        let mut skips = Vec::new();
        let mut after_skip = false;
        let mut tick = 0u32;
        while tick < SKIP + 4 {
            let f = session.tick(none);
            assert!(!f.ended, "the replay ended at tick {tick}, skips {skips:?}");
            let due = Instant::now();
            let draws = f.draws.clone();
            let game = &session.m.game;
            let old = [
                interpolate.ran(Mode::Interpolate, game, f.clone(), due),
                extrapolate.ran(Mode::Extrapolate, game, f, due),
            ];
            for f in old.into_iter().flatten() {
                session.m.recycle(f);
            }
            let m = &session.m;
            let predicted = extrapolate.predict(|p| session.predict(p, none));
            if draws.is_empty() {
                if last.is_some() {
                    skips.push(tick);
                }
                for t in [0.0, 0.25, 0.5, 1.0] {
                    assert!(
                        interpolate.frame(Mode::Interpolate, t).is_none()
                            && extrapolate.frame(Mode::Extrapolate, t).is_none(),
                        "tick {tick} draws nothing, but a paced frame at t = {t} draws"
                    );
                }
                assert!(predicted.is_none(), "tick {tick}: predicted after no draws");
                if let Some((game, drawn)) = &last {
                    assert_eq!(
                        unpredicted,
                        Some(tick - 1),
                        "Extrapolate predicted tick {tick}, which draws nothing"
                    );
                    // The bug the hold fixes: the last draws on this tick's state aren't the image
                    // they drew.
                    let held = fresh(&dol).frame(game, &m.data, drawn);
                    let stale = fresh(&dol).frame(&m.game, &m.data, drawn);
                    let joints: std::collections::BTreeSet<u32> = drawn
                        .iter()
                        .filter_map(|d| match d {
                            Draw::Shape { jobj, .. } => Some(jobj.0),
                            _ => None,
                        })
                        .collect();
                    let (was, now) = (&game.objs.jobjs.pool, &m.game.objs.jobjs.pool);
                    let moved = joints.iter().filter(|&&j| was[j].mtx != now[j].mtx).count();
                    let dirty = joints.iter().filter(|&&j| now[j].mtx_is_dirty()).count();
                    let differ = held.iter().zip(&stale).filter(|(a, b)| a != b).count()
                        + held.len().abs_diff(stale.len());
                    eprintln!(
                        "tick {tick} draws nothing: of the {} joints whose shapes the tick before \
                     drew, {moved} have new world matrices and {dirty} wait for a display pass \
                     to recompute theirs; its draws on this state differ from its frame in \
                     {differ} of {} stream bytes",
                        joints.len(),
                        held.len()
                    );
                    assert!(held != stale, "tick {tick}: the state didn't change");
                    after_skip = true;
                }
            } else {
                let own = off.frame(&m.game, &m.data, &draws);
                let (d, blend) = interpolate.frame(Mode::Interpolate, 1.0).expect("a frame");
                assert!(d == draws.as_slice(), "tick {tick}: Interpolate's draws");
                assert!(
                    at_1.frame_blended(&m.game, &m.data, d, Some(blend)) == own,
                    "tick {tick}: Interpolate's frame at t = 1 isn't Off's"
                );
                let (d, blend) = extrapolate.frame(Mode::Extrapolate, 0.0).expect("a frame");
                assert!(
                    at_0.frame_blended(&m.game, &m.data, d, Some(blend)) == own,
                    "tick {tick}: Extrapolate's frame at t = 0 isn't Off's"
                );
                if let Some(Err(why)) = &predicted {
                    panic!("tick {tick}'s prediction stopped: {why}");
                }
                if !extrapolate.predictor.is_filled() {
                    // Nothing to blend toward: every frame until the next tick that draws is this
                    // tick's own, which is the image held through a tick that draws nothing.
                    unpredicted = Some(tick);
                    let (d, blend) = extrapolate.frame(Mode::Extrapolate, 0.5).expect("a frame");
                    assert!(
                        fresh(&dol).frame_blended(&m.game, &m.data, d, Some(blend))
                            == fresh(&dol).frame(&m.game, &m.data, &draws),
                        "tick {tick}: Extrapolate's unpredicted frame isn't the tick's own"
                    );
                }
                if std::mem::take(&mut after_skip) {
                    // Interpolate's A is the last tick that drew, before the skip.
                    let (game, drawn) = last.as_ref().expect("a tick before");
                    let mut a = Capture::default();
                    a.fill(game, drawn);
                    for t in [0.0, 0.5] {
                        let (d, blend) = interpolate.frame(Mode::Interpolate, t).expect("a frame");
                        let want = Blend { from: &a, t };
                        assert!(
                            fresh(&dol).frame_blended(&m.game, &m.data, d, Some(blend))
                                == fresh(&dol).frame_blended(&m.game, &m.data, &draws, Some(want)),
                            "tick {tick}: Interpolate at t = {t} doesn't blend from the last tick that drew"
                        );
                    }
                }
                match &mut last {
                    Some((game, drawn)) => {
                        game.clone_from(&m.game);
                        drawn.clone_from(&draws);
                    }
                    None => last = Some((m.game.clone(), draws)),
                }
            }
            tick += 1;
        }
        assert_eq!(skips, [SKIP], "the ticks that draw nothing");
    });
}
