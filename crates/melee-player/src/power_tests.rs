//! The console from power-on through the player's own session (`Session::power_on`): a
//! recorded session played back from power-on, its reads recorded again by the player, gives
//! the same reads, and the game's state after each tick is what the bare session gives
//! (melee-sim's `game::session`, as the oracle runs it), a replay played between ticks
//! included. Skips without `SSBM_DISC` and `SSBM_RS_LOCAL`.

use std::path::PathBuf;

use melee_sim::game::inputs::Recording;
use melee_sim::game::session::Setup;

use melee_sim::state_eq::StateEq;

use crate::Session;

/// The recorded session `name` in ssbm-rs's recordings, if the data is there.
fn recording(name: &str) -> Option<(String, PathBuf)> {
    let disc = std::env::var("SSBM_DISC").ok()?;
    let local = std::env::var_os("SSBM_RS_LOCAL")?;
    let p = PathBuf::from(local).join("lockstep/recordings").join(name);
    p.exists().then_some((disc, p))
}

#[test]
fn a_played_session_records_its_reads_again() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = recording("2026-10-06_17-13-21.inputs") else {
            eprintln!("skipped: no disc or no recordings");
            return;
        };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let rec = Recording::read(&path).unwrap();
        let out =
            std::env::temp_dir().join(format!("melee-hd-power-{}.inputs", std::process::id()));
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc.clone();
        args.mute = true;
        args.bench = Some(0);
        let ticks = 900;
        {
            let mut s = Session::power_on(
                &args,
                None,
                Some(Recording::read(&path).unwrap()),
                Some(&out),
            )
            .expect("the power-on");
            // The bare session beside it, as the oracle runs it.
            let mut bare = melee_sim::game::session::Session::new(Setup {
                disc: rec.disc().unwrap(),
                dsp: rec.dsp(),
                card: rec.card.as_ref().map(|c| std::fs::read(c).unwrap()),
                power_on: rec.power_on,
                dtv: rec.dtv,
                sram_flags: rec.sram_flags,
                card_pull: None,
                card_b: None,
                card_b_pull: None,
                develop: rec.developer(),
            });
            let d = melee_data::disc::Disc::open(&disc).unwrap();
            let mut console = crate::power::Live::new(
                &d,
                crate::power::Source::Reads {
                    reads: &rec.reads,
                    next: 0,
                },
            );
            bare.boot(&mut console);
            let none = [melee_sim::input::pad::PadStatus::default(); 4];
            for t in 0..ticks {
                let f = s.tick(none);
                s.recycle(f);
                let _ = crate::power::step(&mut bare, &mut console);
                if let Some(diff) = s.game().state_diff(&bare.game) {
                    panic!("tick {t}: the player's session differs at game{diff}");
                }
            }
        }
        let again = Recording::read(&out).unwrap();
        let n = again.reads.len();
        assert!(n > ticks, "{n} reads in {ticks} ticks");
        assert_eq!(again.reads[..], rec.reads[..n], "the reads recorded again");
        assert_eq!(
            (again.power_on, again.dtv, again.sram_flags, again.disc_rate),
            (rec.power_on, rec.dtv, rec.sram_flags, rec.disc_rate)
        );
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(out.with_extension("card"));
    });
}

/// A replay played from the Recordings tab sets Melee aside and gives it back: the console
/// played around a replay's match (`Session::start`, as the tab starts one, on the same
/// thread) goes on exactly as the bare session does, tick for tick, and records its reads as
/// if nothing had come between.
#[test]
fn a_replay_leaves_melee_as_it_was() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = recording("2026-10-06_17-13-21.inputs") else {
            eprintln!("skipped: no disc or no recordings");
            return;
        };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let rec = Recording::read(&path).unwrap();
        let out =
            std::env::temp_dir().join(format!("melee-hd-aside-{}.inputs", std::process::id()));
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc.clone();
        args.mute = true;
        args.bench = Some(0);
        let (before, during, after) = (450, 600, 450);
        {
            let mut s = Session::power_on(
                &args,
                None,
                Some(Recording::read(&path).unwrap()),
                Some(&out),
            )
            .expect("the power-on");
            let mut bare = melee_sim::game::session::Session::new(Setup {
                disc: rec.disc().unwrap(),
                dsp: rec.dsp(),
                card: rec.card.as_ref().map(|c| std::fs::read(c).unwrap()),
                power_on: rec.power_on,
                dtv: rec.dtv,
                sram_flags: rec.sram_flags,
                card_pull: None,
                card_b: None,
                card_b_pull: None,
                develop: rec.developer(),
            });
            let d = melee_data::disc::Disc::open(&disc).unwrap();
            let mut console = crate::power::Live::new(
                &d,
                crate::power::Source::Reads {
                    reads: &rec.reads,
                    next: 0,
                },
            );
            bare.boot(&mut console);
            let none = [melee_sim::input::pad::PadStatus::default(); 4];
            let mut step = |s: &mut Session, t: usize| {
                let f = s.tick(none);
                s.recycle(f);
                let _ = crate::power::step(&mut bare, &mut console);
                if let Some(diff) = s.game().state_diff(&bare.game) {
                    panic!("tick {t}: the player's session differs at game{diff}");
                }
            };
            for t in 0..before {
                step(&mut s, t);
            }
            let mut replay_args = args.clone();
            replay_args.replay = Some(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../data/bench/fox-fox-fd.slp")
                    .display()
                    .to_string(),
            );
            replay_args.power_on = false;
            let mut r = Session::start(&replay_args).expect("the replay's match");
            for _ in 0..during {
                let f = r.tick(none);
                r.recycle(f);
            }
            assert!(!r.ended, "the replay plays on");
            drop(r);
            for t in before..before + after {
                step(&mut s, t);
            }
        }
        let again = Recording::read(&out).unwrap();
        let n = again.reads.len();
        assert!(n > before + after, "{n} reads");
        assert_eq!(again.reads[..], rec.reads[..n], "the reads recorded again");
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(out.with_extension("card"));
    });
}
