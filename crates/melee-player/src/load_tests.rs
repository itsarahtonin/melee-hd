//! Scene loads from power-on: a tick that loads runs the load's whole length on Melee's
//! timeline, and the player plays all of it. Skips without `SSBM_DISC`.

use std::time::{Duration, Instant};

use melee_audio::Simulated;
use melee_sim::audio::clock::TB_HZ;
use melee_sim::game::gm::machine::{GM_MENU, GM_VS, Phase};

use crate::Session;
use crate::holds::Holds;
use crate::jit;
use crate::pacing::Mode;

/// The console from power-on, muted: with a blank card and the controllers given, or playing the
/// recorded session `inputs` with its card.
fn console(inputs: Option<&str>) -> Option<Session> {
    let disc = std::env::var("SSBM_DISC").ok()?;
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
    args.disc = disc;
    args.mute = true;
    args.bench = Some(0);
    let feed = inputs.map(|p| {
        melee_sim::game::inputs::Recording::read(std::path::Path::new(p)).expect("the recording")
    });
    Some(Session::power_on(&args, None, feed, None).expect("the power-on"))
}

/// Where Melee's timeline is.
fn now(s: &Session) -> u64 {
    s.game().audio.clock.as_ref().map_or(0, |c| c.now)
}

/// The mode the scene loop runs, if one does.
fn mode(s: &Session) -> Option<u8> {
    match s.game().machine.phase {
        Phase::Scene { mode, .. } => Some(mode),
        _ => None,
    }
}

/// The scripted way into VS mode from power-on and back: Start every half second through the
/// opening movie and the title, then from the main menu's first tick down to VS mode, A into
/// it and A again into Melee's character select, B held back out to the main menu.
fn vs_entry(s: &Session, t: u64, menu: &mut Option<u64>) -> melee_sim::input::controller::PadData {
    const IN_AND_OUT: &str = "down@100-102,a@160-162,a@260-262,b@600-660";
    if menu.is_none() && mode(s) == Some(GM_MENU) {
        *menu = Some(t);
    }
    match *menu {
        Some(m) => Holds::parse(IN_AND_OUT).unwrap().at(t - m),
        None if t % 30 < 3 => Holds::parse("start@0-0").unwrap().at(0),
        None => Holds::default().at(0),
    }
}

/// The player's loop against a simulated device, on a fake clock: each tick starts when the
/// scheduler plans it (the fixed timer), takes the host 1 ms (60 ms when it loads a scene, longer
/// than the sound queued holds), and pushes its samples as the player does; the device's
/// 128-sample callbacks at 48 kHz run as time passes. `rule` false plays it as the player did
/// before long ticks: every push a tick's, no wait. What came of it: the samples mixed, those
/// dropped, the long ticks, the VS mode's character select reached and what the device played.
struct Played {
    mixed: usize,
    dropped: usize,
    skips: u32,
    long: Vec<(u64, Duration, usize)>,
    reached_vs: bool,
    heard: Vec<[f32; 2]>,
}

fn play(rule: bool, ticks: u64) -> Option<Played> {
    let mut s = console(None)?;
    let mut out = Simulated::new(48_000);
    let t0 = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / crate::TICK_RATE);
    let mut jit = jit::Scheduler::new(tick, jit::DEFAULT_MARGIN, t0);
    let (mut clock, mut device) = (t0, t0);
    let callback = Duration::from_secs_f64(128.0 / 48_000.0);
    let mut menu = None;
    let mut played = Played {
        mixed: 0,
        dropped: 0,
        skips: 0,
        long: Vec::new(),
        reached_vs: false,
        heard: Vec::new(),
    };
    for t in 0..ticks {
        let plan = jit.plan(clock, jit::Policy::Timer, None);
        clock = clock.max(plan.start);
        let pads = vs_entry(&s, t, &mut menu);
        let f = s.tick(pads);
        s.recycle(f);
        played.reached_vs |= mode(&s) == Some(GM_VS);
        let length = s.length.expect("a timeline");
        let long = jit::is_long(length, tick);
        // The host's time for the tick, the device playing meanwhile.
        clock += Duration::from_millis(if long { 60 } else { 1 });
        while device + callback <= clock {
            out.fill(128, |x| played.heard.push(x));
            device += callback;
        }
        let samples = s.sound.as_ref().expect("the sound").samples();
        played.mixed += samples.len();
        if long {
            played.long.push((t, length, samples.len()));
        }
        if rule && long {
            out.push_long(samples);
        } else {
            out.push(samples);
        }
        jit.ran(&plan);
        if rule {
            jit.took(length, out.stats().dry);
        }
    }
    let stats = out.stats();
    played.skips = stats.skips;
    played.dropped = out.dropped();
    assert_eq!(out.pushed() + out.overflowed(), played.mixed);
    Some(played)
}

/// The scripted VS mode entry and exit, as the player plays it: every sample the ticks mix
/// plays, the loads' music included. With the rule before long ticks, each load's music past
/// two ticks was dropped (the music skipping ahead at every scene's load, as Sarah heard).
#[test]
fn vs_mode_entry_plays_every_sample_it_mixes() {
    crate::on_game_stack(|| {
        let Some(now) = play(true, 1700) else {
            eprintln!("skipped: no disc");
            return;
        };
        for (t, length, n) in &now.long {
            eprintln!(
                "tick {t}: {:.3} ms of Melee's time, {n} samples",
                length.as_secs_f64() * 1000.0
            );
        }
        assert!(now.reached_vs, "the character select wasn't reached");
        // The title's, the main menu's, VS mode's and the main menu's again.
        assert!(now.long.len() >= 4, "{} long ticks", now.long.len());
        assert_eq!((now.skips, now.dropped), (0, 0));
        let before = play(false, 1700).unwrap();
        eprintln!(
            "before: {} of {} samples dropped in {} skips",
            before.dropped, before.mixed, before.skips
        );
        assert!(before.skips >= 4);
        // `LOAD_WAV=DIR` writes what the device played, before and after, to listen to.
        if let Ok(dir) = std::env::var("LOAD_WAV") {
            for (name, p) in [
                ("vs-entry-before.wav", &before),
                ("vs-entry-after.wav", &now),
            ] {
                let path = std::path::Path::new(&dir).join(name);
                std::fs::write(&path, wav(&p.heard)).unwrap();
                eprintln!("wrote {}", path.display());
            }
        }
    });
}

/// A frame presented: when, the tick that drew it, the scene the game was in then (its phase),
/// the scene loop's rounds drawn since that scene started (`scene.unk_4`, 1 for its first
/// frame) and whether the tick ended the scene (`scene.unk_C`: 1 after drawing its last frame).
#[derive(Clone, Debug)]
struct Presented {
    at: Instant,
    tick: u64,
    scene: String,
    drawn: u32,
    ending: i32,
}

/// The scripted VS mode entry and exit through the player's loop with high refresh off, on a
/// fake clock (the fixed timer; each tick takes the host no time): the frames presented, and
/// the long ticks (the tick, when it was due and how much later its load made the next tick).
/// With it, Interpolate's frames: for each tick that drew, whether its frames blend from a
/// tick before.
#[allow(clippy::type_complexity)]
fn present(
    ticks: u64,
) -> Option<(
    Vec<Presented>,
    Vec<(u64, Instant, Duration)>,
    Vec<(u64, bool)>,
)> {
    let mut s = console(None)?;
    let t0 = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / crate::TICK_RATE);
    let mut jit = jit::Scheduler::new(tick, jit::DEFAULT_MARGIN, t0);
    let mut hold = jit::Hold::default();
    let mut paced = crate::paced::Paced::default();
    let (mut clock, mut menu) = (t0, None);
    let (mut shown, mut long, mut blends) = (Vec::new(), Vec::new(), Vec::new());
    for t in 0..ticks {
        let plan = jit.plan(clock, jit::Policy::Timer, None);
        // The event loop wakes for a held frame's time, then for the tick.
        if let Some(u) = hold.until().filter(|u| *u <= plan.start) {
            clock = clock.max(u);
            shown.extend(
                hold.end(clock)
                    .map(|p: Presented| Presented { at: clock, ..p }),
            );
        }
        clock = clock.max(plan.start);
        assert!(hold.take().is_none(), "tick {t}: a frame still held");
        let pads = vs_entry(&s, t, &mut menu);
        let f = s.tick(pads);
        jit.ran(&plan);
        let later = jit.took(s.length.expect("a timeline"), Duration::ZERO);
        if !later.is_zero() {
            long.push((t, plan.due, later));
            paced.loading();
        }
        let drew = (!f.draws.is_empty()).then(|| Presented {
            at: clock,
            tick: t,
            scene: format!("{:?}", s.game().machine.phase),
            drawn: s.game().scene.unk_4,
            ending: s.game().scene.unk_C,
        });
        shown.extend(hold.tick(drew, plan.due, later));
        if let Some(old) = paced.ran(Mode::Interpolate, s.game(), f, plan.due) {
            s.recycle(old);
        }
        if let Some((_, b)) = paced.frame(Mode::Interpolate, 0.5) {
            blends.push((t, b.from.is_filled()));
        }
    }
    Some((shown, long, blends))
}

/// Through each scene's load the screen keeps the frame shown before it, the old scene's last
/// (drawn in the tick before the load), and the long tick's frame, the new scene's first,
/// shows only once the load's length has passed on Melee's clock, a tick before the next tick;
/// every other tick's frame shows when its tick runs. Interpolate never blends across a load.
#[test]
fn a_scene_s_load_holds_the_frame_from_before_it() {
    crate::on_game_stack(|| {
        let Some((shown, long, blends)) = present(1700) else {
            eprintln!("skipped: no disc");
            return;
        };
        let tick = Duration::from_secs_f64(1.0 / crate::TICK_RATE);
        assert!(long.len() >= 4, "{} long ticks", long.len());
        for &(t, _, later) in &long {
            let i = shown
                .iter()
                .position(|p| p.tick == t)
                .expect("the load's frame");
            let (before, after) = (&shown[i - 1], &shown[i]);
            eprintln!(
                "tick {t}: {:.0} ms later; before it tick {} (ending {}) in {}, after it frame {} in {}",
                later.as_secs_f64() * 1000.0,
                before.tick,
                before.ending,
                before.scene,
                after.drawn,
                after.scene
            );
        }
        let mut loads = 0;
        for &(t, due, later) in &long {
            let i = shown
                .iter()
                .position(|p| p.tick == t)
                .expect("the load's frame");
            let (before, after) = (&shown[i - 1], &shown[i]);
            assert_eq!(
                before.tick,
                t - 1,
                "the old scene's last frame is the tick before's"
            );
            if after.drawn == 1 {
                // A scene's load: the frame before it is the old scene's last.
                assert_eq!(before.ending, 1, "tick {t}: the old scene's last frame");
                loads += 1;
            } else {
                // A tick that ran long within its scene (the boot's two-field tick).
                assert_eq!(before.scene, after.scene);
            }
            assert!(before.at <= due, "tick {t}: shown when its tick ran");
            assert_eq!(after.at, due + later, "tick {t}: shown when the load ends");
            let next = shown.get(i + 1).expect("a frame after");
            if next.tick == t + 1 {
                assert_eq!(next.at, after.at + tick, "the next tick a tick later");
            }
            let blended = blends.iter().find(|(n, _)| *n == t).expect("its frames");
            assert!(!blended.1, "tick {t}: Interpolate blends across the load");
            let then = blends.iter().find(|(n, _)| *n > t).expect("the next's");
            assert!(then.1, "tick {}: blends from the load's frame", then.0);
        }
        // The title's, the main menu's, VS mode's and the main menu's again.
        assert!(loads >= 4, "{loads} loads");
        // Every frame drawn shows, in order, the clock never going back.
        assert_eq!(shown.len(), blends.len());
        for w in shown.windows(2) {
            assert!(w[0].tick < w[1].tick && w[0].at < w[1].at, "{w:?}");
        }
    });
}

/// 48 kHz stereo 16-bit WAV bytes.
fn wav(samples: &[[f32; 2]]) -> Vec<u8> {
    let data = samples.len() as u32 * 4;
    let mut b = Vec::with_capacity(44 + data as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    for v in [16u32, 0x0002_0001, 48_000, 48_000 * 4, 0x0010_0004] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        for c in s {
            b.extend_from_slice(&((c.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    b
}

/// Exploration: `LOAD_HOLDS` (or the recorded session `LOAD_INPUTS`) played for `LOAD_TICKS`
/// ticks, each tick that isn't a field long or changes the scene printed with its length, the
/// samples it mixed, the host's time for it, whether it drew a frame, and whether the tick
/// before drew one and ended its scene (`scene.unk_C`: 1 draws the scene's last frame, 2 not).
#[test]
#[ignore]
fn explore() {
    crate::on_game_stack(|| {
        let inputs = std::env::var("LOAD_INPUTS").ok();
        let Some(mut s) = console(inputs.as_deref()) else {
            eprintln!("skipped: no disc");
            return;
        };
        let holds = Holds::parse(&std::env::var("LOAD_HOLDS").unwrap_or_default()).unwrap();
        // Without a length it would play forever, so `--include-ignored` runs skip it.
        let Some(ticks) = std::env::var("LOAD_TICKS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        else {
            eprintln!("skipped: LOAD_TICKS isn't set");
            return;
        };
        let mut last = now(&s);
        let mut phase = format!("{:?}", s.game().machine.phase);
        let mut before = (false, 0);
        for t in 0..ticks {
            let host = Instant::now();
            let f = s.tick(holds.at(t));
            let host = host.elapsed();
            let drew = !f.draws.is_empty();
            s.recycle(f);
            let n = now(&s);
            let len = n - last;
            last = n;
            let samples = s.sound.as_ref().map_or(0, |x| x.samples().len());
            let p = format!("{:?}", s.game().machine.phase);
            if len.abs_diff(melee_sim::audio::clock::FIELD) > TB_HZ / 2000 || p != phase {
                println!(
                    "tick {t}: {:.3} ms, {samples} samples ({:.3} ms), host {:.1} ms, drew {drew}                      (before: drew {}, ending {}), {p}",
                    len as f64 * 1000.0 / TB_HZ as f64,
                    samples as f64 / 32.0,
                    host.as_secs_f64() * 1000.0,
                    before.0,
                    before.1
                );
                phase = p;
            }
            before = (drew, s.game().scene.unk_C);
            if s.ended {
                break;
            }
        }
    });
}
