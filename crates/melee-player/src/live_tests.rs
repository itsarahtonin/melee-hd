//! The Slippi ruleset's live matches in the player (`docs/slippi-ruleset.md`, task 6), on
//! ssbm-rs's ruleset sessions: the player's console runs a session under the ruleset as the bare
//! session runs it, through a live match's handoff and handback, in Slippi's float mode on
//! whichever thread ticks it; and a live match draws with a frame builder and renderer of its
//! own while the menus' wait, as they'd draw had the other never drawn
//! ([`crate::handoff`]). Skips without `SSBM_DISC` and `SSBM_RS_LOCAL`.

use std::path::PathBuf;

use melee_sim::game::inputs::Recording;
use melee_sim::game::session::Setup;
use melee_sim::gekko_fp::FpMode;
use melee_sim::state_eq::StateEq;

use crate::Session;

/// `pause-fd` (`docs/oracle.md`, "The Slippi ruleset's live matches"): the menus under the
/// ruleset into a live match on Final Destination from tick 1018, its handback before tick 1757,
/// then the results and the character select, to tick 3136.
pub(crate) fn pause_fd() -> Option<(String, PathBuf)> {
    ruleset_session("m6s-pause/pause-fd.inputs")
}

/// A session under the ruleset in ssbm-rs's `lockstep` folder, with the disc, if both are there.
pub(crate) fn ruleset_session(name: &str) -> Option<(String, PathBuf)> {
    let disc = std::env::var("SSBM_DISC").ok()?;
    let local = std::env::var_os("SSBM_RS_LOCAL")?;
    let p = PathBuf::from(local).join("lockstep").join(name);
    p.exists().then_some((disc, p))
}

/// The player's own console (`Session::power_on`) for a recorded session's reads, powered on on a
/// thread of its own as the player powers on, with `args`.
pub(crate) fn powered_on(args: &crate::Args, path: &std::path::Path) -> Session {
    let (args, path) = (args.clone(), path.to_owned());
    std::thread::Builder::new()
        .stack_size(crate::GAME_STACK)
        .spawn(move || {
            Session::power_on(&args, None, Some(Recording::read(&path).unwrap()), None)
                .expect("the power-on")
        })
        .expect("the power-on's thread")
        .join()
        .unwrap_or_else(|p| std::panic::resume_unwind(p))
}

/// The player's console runs `pause-fd`, a session under the Slippi ruleset, as the bare session
/// does tick by tick, through its live match's handoff and handback: the power-on runs on
/// another thread, as the player's does, and this thread is in the console's float mode before
/// each of the player's ticks, so the session sets Slippi's itself.
#[test]
fn a_ruleset_session_plays_as_the_bare_one() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = pause_fd() else {
            eprintln!("skipped: no disc or no ruleset sessions");
            return;
        };
        let rec = Recording::read(&path).unwrap();
        assert_eq!(rec.ruleset, melee_sim::game::state::Ruleset::Slippi);
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc.clone();
        args.mute = true;
        args.bench = Some(0);
        let mut s = powered_on(&args, &path);
        assert_eq!(s.fp, FpMode::Slippi);
        let mut bare = melee_sim::game::session::Session::new(Setup {
            disc: rec.disc().unwrap(),
            dsp: rec.dsp(),
            card: None,
            power_on: rec.power_on,
            dtv: rec.dtv,
            sram_flags: rec.sram_flags,
            ruleset: rec.ruleset,
            ..Setup::default()
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
        let (mut live, mut back) = (0, 0);
        for t in 0..3100 {
            melee_sim::gekko_fp::set_fp_mode(FpMode::Hardware);
            let f = s.tick(none);
            s.recycle(f);
            assert_eq!(melee_sim::gekko_fp::fp_mode(), FpMode::Slippi);
            let _ = crate::power::step(&mut bare, &mut console);
            if let Some(diff) = s.game().state_diff(&bare.game) {
                panic!("tick {t}: the player's session differs at game{diff}");
            }
            assert_eq!(s.in_live_match(), bare.in_live_match(), "tick {t}");
            if s.in_live_match() {
                live += 1;
            } else if live > 0 {
                back += 1;
            }
        }
        melee_sim::gekko_fp::set_fp_mode(FpMode::Hardware);
        assert!(live > 600, "{live} live ticks");
        assert!(back > 1000, "{back} ticks after the handback");
    });
}

/// `--replay` (and the Recordings tab) plays a Slippi replay, one that records a code list
/// (Slippi Dolphin's, or the ruleset's in melee-hd's recordings), in Slippi Dolphin's float mode,
/// and one with none (melee-hd's Vanilla recordings) or ssbm-rs's (the benchmark matches, made
/// in the console's) in the console's; the match starts in it, and every tick sets it.
#[test]
fn a_slippi_replay_plays_in_slippis_float_mode() {
    let mut r = melee_slippi::Replay::default();
    assert_eq!(crate::replay_fp(&r), FpMode::Hardware);
    r.gecko_codes = melee_sim::game::record::ruleset_codes();
    assert_eq!(crate::replay_fp(&r), FpMode::Slippi);
    r.played_on = Some("melee-hd".into());
    assert_eq!(crate::replay_fp(&r), FpMode::Slippi);
    r.played_on = Some("ssbm-rs".into());
    assert_eq!(crate::replay_fp(&r), FpMode::Hardware);
    // A replay's own file says what played it.
    let bench = melee_slippi::Replay::parse(&std::fs::read(crate::predict_tests::REPLAY).unwrap())
        .expect("the benchmark replay");
    assert_eq!(bench.played_on.as_deref(), Some("ssbm-rs"));
    if let Some(dir) = std::env::var_os("SLIPPI_REPLAYS") {
        let p = PathBuf::from(dir).join("2026-01/Game_20260129T130802.slp");
        if let Ok(bytes) = std::fs::read(&p) {
            let r = melee_slippi::Replay::parse(&bytes).expect("a Slippi replay");
            assert_eq!(r.played_on.as_deref(), Some("dolphin"));
            assert_eq!(crate::replay_fp(&r), FpMode::Slippi);
        }
    }
    crate::on_game_stack(|| {
        let Ok(disc) = std::env::var("SSBM_DISC") else {
            eprintln!("skipped: no disc");
            return;
        };
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.replay = Some(crate::predict_tests::REPLAY.to_owned());
        melee_sim::gekko_fp::set_fp_mode(FpMode::Slippi);
        let s = Session::start(&args).expect("the replay's match");
        assert_eq!(s.fp, FpMode::Hardware, "ssbm-rs's benchmark replay");
    });
}

/// A pair drawing frames headless at the console's size, each frame read back.
struct Pair {
    frames: melee_render::frame::Frames,
    gpu: melee_render::gpu::Gpu,
}

impl Pair {
    fn new(gpu: &crate::present::Gpu, dol: &[u8]) -> Pair {
        let mut frames = melee_render::frame::Frames::new();
        frames.space.load_dol(dol);
        Pair {
            frames,
            gpu: renderer(gpu),
        }
    }

    fn draw(&mut self, s: &Session, draws: &[melee_sim::hsd::display::Draw]) -> Vec<u8> {
        draw(&mut self.frames, &mut self.gpu, s, draws)
    }
}

fn renderer(gpu: &crate::present::Gpu) -> melee_render::gpu::Gpu {
    let mut r = crate::pipelines::renderer(gpu, 1, 1, crate::present::Aspect::GAME);
    r.xfb_readback = true;
    crate::gpu_for(r, true)
}

fn draw(
    frames: &mut melee_render::frame::Frames,
    gpu: &mut melee_render::gpu::Gpu,
    s: &Session,
    draws: &[melee_sim::hsd::display::Draw],
) -> Vec<u8> {
    let fifo = frames.frame(s.game(), s.data(), draws);
    let image = gpu.run(&frames.space, s.data(), &fifo, None).pop();
    image.expect("the frame copied").rgba
}

/// A live match draws with a frame builder and renderer of its own, and the menus' wait across
/// it ([`crate::handoff`], as the player and its benchmark follow the session): `pause-fd`'s
/// every frame, drawn by the player's pair, is the frame a pair that only ever drew its own
/// session draws, the live match's (made at the handoff) or the menus' (which drew no frame of
/// the match). Skips without a hardware GPU too.
#[test]
fn a_live_match_draws_with_its_own_frame_builder() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = pause_fd() else {
            eprintln!("skipped: no disc or no ruleset sessions");
            return;
        };
        let Some((instance, adapter)) = melee_gpu::testing::instance_and_adapter() else {
            eprintln!("skipped: no hardware GPU");
            return;
        };
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.bench = Some(0);
        args.scale = 1;
        args.shot = Some(String::new());
        let mut s = powered_on(&args, &path);
        let gpu = crate::present::Gpu::with_adapter(instance, adapter);
        let mut frames = melee_render::frame::Frames::new();
        frames.space.load_dol(&s.dol);
        let mut player = renderer(&gpu);
        let mut handoff = crate::handoff::Handoff::default();
        let shape = crate::handoff::Shape {
            scale: 1,
            efb_width: crate::present::Aspect::GAME.efb_width(1),
            msaa: 1,
        };
        let mut menus = Pair::new(&gpu, &s.dol);
        let mut live: Option<Pair> = None;
        let none = [melee_sim::input::pad::PadStatus::default(); 4];
        let (mut live_frames, mut after) = (0, 0);
        for t in 0..3100 {
            let f = s.tick(none);
            crate::bench_follow(
                &mut handoff,
                &mut s,
                &mut frames,
                &mut player,
                shape,
                &gpu,
                &args,
            );
            if !f.draws.is_empty() {
                let shown = draw(&mut frames, &mut player, &s, &f.draws);
                let want = if s.in_live_match() {
                    live_frames += 1;
                    live.get_or_insert_with(|| Pair::new(&gpu, &s.dol))
                        .draw(&s, &f.draws)
                } else {
                    live = None;
                    after += usize::from(live_frames > 0);
                    menus.draw(&s, &f.draws)
                };
                assert!(
                    shown == want,
                    "tick {t}'s frame differs from its own pair's ({})",
                    if s.in_live_match() {
                        "the live match's"
                    } else {
                        "the menus'"
                    }
                );
            }
            s.recycle(f);
        }
        assert!(live_frames > 600, "{live_frames} frames of the live match");
        assert!(after > 1000, "{after} frames after its handback");
    });
}
