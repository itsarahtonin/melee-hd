//! `melee-oracle sync-test REPLAY.slp [--rollback N] [--fp hardware|slippi] [--ticks N]
//! [--draw-offscreen-fighters]`: plays a replay and, after every tick, rolls back and plays again, as rollback netplay does,
//! failing on the first difference (docs/oracle.md, "Sync test").
//!
//! The match plays straight from the replay's settings, as `melee-bench native` plays it (its
//! own boot; no trace needed). Before each tick its state is kept in a ring of snapshots
//! (`clone_from` into the ring's storage). From tick N on, after each tick a second match
//! restores the snapshot from N ticks before (`clone_from`, as a rollback restores) and plays
//! those N ticks again with the same inputs, display passes included; its state must then
//! equal the straight run's bit for bit (`melee_sim::state_eq`, every field of `Game`), and
//! each replayed tick's output (sounds, rumble, draws) the straight run's for that tick. With
//! N = 1 (the default) every tick is played twice from the same state; Slippi's rollback
//! window is 7 or 8 ticks. `--draw-offscreen-fighters` turns on that cosmetic toggle, as the
//! player does in widescreen (`Cosmetic`, which the snapshots carry with the rest).
//!
//! Anything the simulation reads that the snapshot leaves out shows up as a difference: the
//! second run reads what the first left there. The archives (`Match::data`) are loaded data
//! that a tick can only read (`Running` borrows them shared), so they stay out of the
//! snapshot.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_data::disc::Disc;
use melee_sim::audio::clock::DiscSpeed;
use melee_sim::game::playback::FIRST_FRAME;
use melee_sim::game::vs_match::{DiscFile, Frame, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_sim::state_eq::StateEq;
use melee_slippi::{Device, Replay};

use crate::diff::panic_text;

/// The replay frame the match's playback last counted.
fn frame(m: &Match) -> i32 {
    m.game
        .playback
        .as_ref()
        .map_or(FIRST_FRAME - 1, |p| p.frame_index)
}

/// One tick and, when the round is over, its frame's display pass: the tick's output.
fn step(m: &mut Match, device: &mut Device) -> Frame {
    let (mut f, render) = m.step_replay_tick(device);
    if render {
        f.draws = m.render();
    }
    f
}

/// Times, in microseconds, summarized.
#[derive(Default)]
struct Times(Vec<f64>);

impl Times {
    fn push(&mut self, d: Duration) {
        self.0.push(d.as_secs_f64() * 1e6);
    }

    fn summary(&mut self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        self.0.sort_by(f64::total_cmp);
        let at = |q: f64| self.0[((self.0.len() - 1) as f64 * q).round() as usize];
        format!(
            "median {:.1} µs, p99 {:.1} µs, max {:.1} µs",
            at(0.5),
            at(0.99),
            at(1.0)
        )
    }
}

pub fn sync_test(args: &[&str]) -> ExitCode {
    let mut rollback = 1usize;
    let mut limit = u64::MAX;
    // `--perturb TICK`: the rollback that replays up to tick TICK first flips the lowest bit of
    // the restored state's first fighter's x, which the check must catch (a check of the
    // check; the random seed wouldn't do, as Slippi's playback restores it every tick).
    let mut perturb = None;
    let mut fp_mode = FpMode::Slippi;
    let mut disc_speed = DiscSpeed::Fast;
    let mut draw_offscreen_fighters = false;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--rollback" => {
                rollback = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|&n| n >= 1)
                    .expect("--rollback N, at least 1")
            }
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--perturb" => {
                perturb = Some(
                    it.next()
                        .and_then(|t| t.parse::<u64>().ok())
                        .expect("--perturb TICK"),
                )
            }
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
            "--draw-offscreen-fighters" => draw_offscreen_fighters = true,
            _ => pos.push(a),
        }
    }
    let [replay_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle sync-test REPLAY.slp [--rollback N] [--ticks N] [--fp hardware|slippi] [--disc-speed fast|drive] [--draw-offscreen-fighters]"
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
    let mut settings = Settings::from_replay(&melee_slippi::start(&replay))
        .unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    settings.disc = disc_speed;
    let mut load = |name: &str| DiscFile {
        entrynum: disc
            .entrynum(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
        bytes: disc
            .read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
    };
    let mut m = match catch_unwind(AssertUnwindSafe(|| Match::start(&settings, &mut load))) {
        Ok(m) => m,
        Err(p) => {
            println!("stopped starting the match: {}", panic_text(&p));
            return ExitCode::FAILURE;
        }
    };
    m.game.cosmetic.draw_offscreen_fighters = draw_offscreen_fighters;
    let mut device = Device(&replay);
    // The states before the last `rollback` ticks (a ring, slot `tick % len`), and those ticks'
    // outputs, as the straight run made them.
    let len = rollback + 1;
    let mut ring: Vec<melee_sim::game::state::Game> = (0..len).map(|_| m.game.clone()).collect();
    let mut outputs: Vec<Frame> = (0..len).map(|_| Frame::default()).collect();
    let mut work = m.clone();
    let (mut snapshots, mut restores, mut rollbacks) =
        (Times::default(), Times::default(), Times::default());
    let mut tick = 0u64;
    let mut checked = 0u64;
    let started = Instant::now();
    let (mut straight, mut comparing) = (Duration::ZERO, Duration::ZERO);
    let failure = loop {
        if tick >= limit {
            break None;
        }
        let at = frame(&m);
        if at >= last {
            println!("the replay's last frame ({last}) played after tick {tick}");
            break None;
        }
        // The state before tick `tick`, kept.
        let slot = (tick % len as u64) as usize;
        let t = Instant::now();
        ring[slot].clone_from(&m.game);
        snapshots.push(t.elapsed());
        let t = Instant::now();
        let stepped = catch_unwind(AssertUnwindSafe(|| step(&mut m, &mut device)));
        straight += t.elapsed();
        let out = match stepped {
            Ok(f) => f,
            Err(p) => {
                println!(
                    "{checked} ticks checked, no difference; the match stopped in tick {tick} (replay frame {at}): {}",
                    panic_text(&p)
                );
                return ExitCode::FAILURE;
            }
        };
        let ended = out.ended;
        outputs[slot].clone_from(&out);
        m.recycle(out);
        tick += 1;
        if tick >= rollback as u64 {
            // Back `rollback` ticks, and those ticks again.
            let from = tick - rollback as u64;
            let t = Instant::now();
            work.game.clone_from(&ring[(from % len as u64) as usize]);
            restores.push(t.elapsed());
            if perturb == Some(tick - 1) {
                let x = &mut work.game.fighters[0].cur_pos.x;
                *x = f32::from_bits(x.to_bits() ^ 1);
            }
            let replayed = catch_unwind(AssertUnwindSafe(|| {
                for k in from..tick {
                    let out = step(&mut work, &mut device);
                    let want = &outputs[(k % len as u64) as usize];
                    if let Some(d) = out.state_diff(want) {
                        return Some(format!(
                            "tick {k}'s output played again differs from the straight run's at frame{d}"
                        ));
                    }
                    work.recycle(out);
                }
                None
            }));
            rollbacks.push(t.elapsed());
            let replayed = match replayed {
                Ok(r) => r,
                Err(p) => Some(format!(
                    "playing ticks {from} to {} again stopped: {}",
                    tick - 1,
                    panic_text(&p)
                )),
            };
            if let Some(why) = replayed {
                break Some(why);
            }
            let t = Instant::now();
            let differs = work.game.state_diff(&m.game);
            comparing += t.elapsed();
            if let Some(d) = differs {
                break Some(format!(
                    "the state after tick {}, restored from before tick {from} and played again, differs from the straight run's at game{d}",
                    tick - 1
                ));
            }
            checked += 1;
        }
        if ended {
            println!("the match ended in tick {}", tick - 1);
            break None;
        }
    };
    println!(
        "snapshot (clone_from into the ring): {}",
        snapshots.summary()
    );
    println!("restore (clone_from): {}", restores.summary());
    println!(
        "restore and {rollback} tick{} again: {}",
        if rollback == 1 { "" } else { "s" },
        rollbacks.summary()
    );
    let total = |t: &Times| t.0.iter().sum::<f64>() / 1e6;
    println!(
        "times: straight run {:.1} s, snapshots {:.1} s, restores and replays {:.1} s (restores {:.1} s), state comparison {:.1} s, all {:.1} s",
        straight.as_secs_f64(),
        total(&snapshots),
        total(&rollbacks),
        total(&restores),
        comparing.as_secs_f64(),
        started.elapsed().as_secs_f64()
    );
    match failure {
        None => {
            println!(
                "{checked} ticks checked, rolled back {rollback} each, no difference ({tick} ticks played, to replay frame {}, in {:.1} s)",
                frame(&m),
                started.elapsed().as_secs_f64()
            );
            ExitCode::SUCCESS
        }
        Some(why) => {
            println!(
                "{checked} ticks checked, rolled back {rollback} each; replay frame {}: {why}",
                frame(&m)
            );
            ExitCode::FAILURE
        }
    }
}
