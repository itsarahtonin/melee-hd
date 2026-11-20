//! What the Slippi ruleset makes moot, held by tests against [`Rules`] (docs/slippi-ruleset.md,
//! "What the ruleset makes moot"): under Slippi 3.19's online rules a match's gameplay reads
//! neither the display pass's state (off-screen damage, Pokémon Stadium's monitor, Whispy's and
//! items' side) nor the registers a computer player's DI with no knockback reads in the
//! original (r5 and r30), so the player's Slippi matches can leave the CPU's register
//! bookkeeping and the allocator model's addresses aside.
//!
//! Each test plays a benchmark replay (`data/bench`) through `vs_match`'s replay path with the
//! display pass, as `native` does, once as recorded and once disturbed: the pass's marks or the
//! camera it writes scrambled after every pass, so the next tick's logic reads garbage where it
//! reads the pass. Under 3.19's rules every tick's gameplay ([`Digest`]: the random seed, and
//! each fighter's action, position, percent, off-screen count and stocks) must stay the same;
//! under Vanilla's the same disturbance must change it, so the test can fail. A replay played
//! under rules it wasn't recorded with leaves its recording, which doesn't matter here: both
//! runs play the same inputs under the same rules. The live Slippi matches the player will run
//! (the plan's task 4) take the same [`Rules`]. Skips without `SSBM_DISC`.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use melee_data::disc::Disc;
use melee_sim::game::ft::FighterKind;
use melee_sim::game::gr::vars::GroundVars;
use melee_sim::game::it::ItKind;
use melee_sim::game::state::{Game, Obj, Rules};
use melee_sim::game::structs::S32Vec2;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_sim::vec::Vec3;
use melee_slippi::{Device, Replay};

/// The simulation recurses as the original does (joint trees): it runs on a deep stack.
const STACK: usize = 512 << 20;

/// The disc, or none (the test skips) without `SSBM_DISC`.
fn disc() -> Option<Disc> {
    let Ok(path) = std::env::var("SSBM_DISC") else {
        eprintln!("SSBM_DISC isn't set: skipping");
        return None;
    };
    Some(Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
}

/// Runs `f` on a thread with a deep stack in `fp`.
fn on_sim_thread<T: Send>(fp: FpMode, f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(s, move || {
                set_fp_mode(fp);
                let out = f();
                set_fp_mode(FpMode::Hardware);
                out
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

/// A benchmark replay and the disc's files its matches load, read once.
struct Bench {
    replay: Replay,
    disc: Disc,
    files: HashMap<String, (i32, Vec<u8>)>,
}

impl Bench {
    fn open(disc: Disc, name: &str) -> Bench {
        Bench::open_path(disc, &melee_local_data::bench_replay(name))
    }

    fn open_path(disc: Disc, path: &std::path::Path) -> Bench {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        Bench {
            replay: Replay::parse(&bytes).expect("a replay"),
            disc,
            files: HashMap::new(),
        }
    }

    /// The replay's match under `rules`, with the allocator model on or off.
    fn start(&mut self, rules: &Rules, heap: bool) -> Match {
        self.start_with(rules, heap, false)
    }

    /// [`Bench::start`], recording the match as a Slippi replay when `record`.
    fn start_with(&mut self, rules: &Rules, heap: bool, record: bool) -> Match {
        let mut settings =
            Settings::from_replay(&melee_slippi::start(&self.replay)).expect("its settings");
        settings.rules = rules.clone();
        if record {
            settings.record = Some(melee_slippi::write::setup(&self.replay));
        }
        if !heap {
            settings.heap = None;
            settings.playback_heap = None;
        }
        let (disc, files) = (&self.disc, &mut self.files);
        Match::start(&settings, &mut |name| {
            let (entrynum, bytes) = files
                .entry(name.to_owned())
                .or_insert_with(|| (disc.entrynum(name).unwrap(), disc.read_file(name).unwrap()));
            DiscFile {
                entrynum: *entrynum,
                bytes: bytes.clone(),
            }
        })
    }
}

/// One tick and, when it's due, the display pass: whether the pass ran, and whether the match
/// is over (the replay's end or the match's).
fn tick(m: &mut Match, device: &mut Device) -> (bool, bool) {
    let (mut f, render) = m.step_replay_tick(device);
    if render {
        f.draws = m.render();
    }
    let ended = f.ended
        || device.0.frames.keys().next_back().is_some_and(|&last| {
            m.game
                .playback
                .as_ref()
                .is_some_and(|p| p.frame_index >= last)
        });
    m.recycle(f);
    (render, ended)
}

/// A tick's gameplay: the random seed, and each fighter's action, position, percent,
/// off-screen count and stocks.
#[derive(Debug, PartialEq)]
struct Digest {
    seed: u32,
    fighters: Vec<(i32, u32, u32, u32, i32, i32)>,
}

fn digest(g: &Game) -> Digest {
    Digest {
        seed: g.objs.rng.seed,
        fighters: g
            .fighters
            .iter()
            .map(|(_, f)| {
                (
                    f.motion_id,
                    f.cur_pos.x.to_bits(),
                    f.cur_pos.y.to_bits(),
                    f.dmg.x1830_percent.to_bits(),
                    f.dmg.x1910,
                    g.player_stocks(i32::from(f.player_idx)),
                )
            })
            .collect(),
    }
}

/// A small deterministic generator for the scrambles.
struct Junk(u32);

impl Junk {
    fn next(&mut self) -> u32 {
        // xorshift32.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 != 0
    }

    /// A float in -range..range.
    fn float(&mut self, range: f32) -> f32 {
        (self.next() % 2001) as f32 / 1000.0 * range - range
    }
}

/// Plays `bench`'s replay under `rules` twice, as recorded and with `scramble` run on the
/// state after every display pass, and returns the first tick whose gameplay differs, if one
/// does, and what `seen` counted over the undisturbed run.
fn played_both_ways(
    bench: &mut Bench,
    rules: &Rules,
    scramble: &(dyn Fn(&mut Game, &mut Junk) + Sync),
    seen: &(dyn Fn(&mut Game) -> bool + Sync),
) -> (Option<(u64, String)>, u64) {
    let mut plain = bench.start(rules, true);
    let mut moved = bench.start(rules, true);
    let replay = &bench.replay;
    let (mut a, mut b) = (Device(replay), Device(replay));
    let mut junk = Junk(0x1234_5678);
    let mut counted = 0;
    for t in 0u64.. {
        let (_, end_a) = tick(&mut plain, &mut a);
        let (rendered, end_b) = tick(&mut moved, &mut b);
        if seen(&mut plain.game) {
            counted += 1;
        }
        let (da, db) = (digest(&plain.game), digest(&moved.game));
        if da != db {
            return (Some((t, format!("{da:?}\n{db:?}"))), counted);
        }
        if end_a || end_b {
            break;
        }
        if rendered {
            scramble(&mut moved.game, &mut junk);
        }
    }
    (None, counted)
}

/// Vanilla's rules, as the benchmark matches were played.
fn vanilla() -> Rules {
    Rules::default()
}

/// Checks that scrambling with `scramble` changes gameplay under Vanilla's rules and not under
/// 3.19's, where `seen` counted at least one tick that reads what was scrambled.
fn decoupled(
    name: &str,
    what: &str,
    scramble: &(dyn Fn(&mut Game, &mut Junk) + Sync),
    seen: &(dyn Fn(&mut Game) -> bool + Sync),
) {
    let Some(disc) = disc() else { return };
    let mut bench = Bench::open(disc, name);
    let (vanilla_diff, _) = on_sim_thread(FpMode::Hardware, || {
        played_both_ways(&mut bench, &vanilla(), scramble, &|_| false)
    });
    let Some((t, _)) = vanilla_diff else {
        panic!(
            "{name}: scrambling {what} changed nothing under Vanilla's rules, so this checks nothing"
        );
    };
    eprintln!("{name}: under Vanilla's rules scrambling {what} changes gameplay from tick {t}");
    let (slippi_diff, counted) = on_sim_thread(FpMode::Slippi, || {
        played_both_ways(&mut bench, &Rules::slippi_online(), scramble, seen)
    });
    if let Some((t, d)) = slippi_diff {
        panic!(
            "{name}: under Slippi 3.19's rules gameplay follows {what}: it differs from tick {t}\n{d}"
        );
    }
    assert!(
        counted > 0,
        "{name}: under Slippi 3.19's rules no tick reached what reads {what}"
    );
    eprintln!("{name}: under Slippi 3.19's rules {counted} ticks read it, the same either way");
}

/// Off-screen damage: the fighters' off-screen marks the game camera's pass sets
/// (`x221F_b0`, the box's screen position `x2188`) and the magnifier's copy of them
/// (`is_offscreen`), which Vanilla's `Fighter_procAnim` counts, set at random after each pass.
/// Under 3.19's rules BrawlOffscreenDamage counts from the stage's camera limits instead.
/// Captain Falcon flies past the screen's edge in this match (`cosmetic_tests`).
#[test]
fn slippi_off_screen_damage_ignores_the_display_pass() {
    decoupled(
        "ganon-captain-fd",
        "the off-screen marks",
        &|g, junk| {
            let slots: Vec<u32> = g.fighters.iter().map(|(i, _)| i).collect();
            for i in slots {
                let f = &mut g.fighters[i];
                f.x221F_b0 = junk.bool();
                f.x2188 = S32Vec2 {
                    x: junk.next() as i32 % 700,
                    y: junk.next() as i32 % 500,
                };
            }
            for p in g.magnify.player.iter_mut() {
                p.state.is_offscreen = junk.bool();
            }
        },
        // A tick a fighter counts off screen by the stage's limits.
        &|g| g.fighters.iter().any(|(_, f)| f.dmg.x1910 > 0),
    );
}

/// Pokémon Stadium's monitor: the game camera's CObj, which only the display pass writes
/// (`Camera_8002A4AC`) and Vanilla's close-up tests its player against
/// (`grStadium_801D32D0`), its eye, interest and field of view moved at random after each
/// pass. Under 3.19's rules PSCameraIndependentMonitor tests fixed bounds.
#[test]
fn slippi_stadium_monitor_ignores_the_display_pass() {
    decoupled(
        "fox-marth-ps",
        "the game camera",
        &|g, junk| {
            let Some(cam) = g.camera.gobj() else {
                return;
            };
            let Some(Obj::CObj(c)) = g.world.gobj(cam).obj else {
                return;
            };
            let cobj = &mut g.objs.cobjs.pool[c.0];
            cobj.projection[0] += junk.float(20.0);
            for w in [cobj.eyepos, cobj.interest].into_iter().flatten() {
                let pos = &mut g.objs.wobjs[w.0].pos;
                pos.x += junk.float(150.0);
                pos.y += junk.float(100.0);
            }
        },
        // A tick the monitor shows a player close up (mode 8).
        &|g| {
            g.grounds
                .iter()
                .any(|(_, gr)| matches!(&gr.u.vars, GroundVars::StadiumDisplay(v) if v.mode == 8))
        },
    );
}

/// Whispy's and the items' side (`ftLib_GetOpponentsDir`): a fighter KO'd toward the screen is
/// placed by the display pass through the camera (`ftDrawCommon_80080E18`), and Vanilla counts
/// its camera bone toward a side. Under 3.19's rules WhispyBlowDirFix leaves fighters in
/// motions up to 11 out. From states of a match, one fighter put in a KO toward the screen
/// (motion 6) is moved far to the left and far to the right: under 3.19's rules the side and the
/// random seed after it are the same, under Vanilla's they differ.
#[test]
fn slippi_opponents_side_ignores_the_display_pass() {
    use melee_sim::game::it::owner::opponents_dir;
    use melee_sim::vec::Vec3;
    let Some(disc) = disc() else { return };
    let mut bench = Bench::open(disc, "ganon-captain-fd");
    let side = |rules: Rules, fp: FpMode, bench: &mut Bench| {
        on_sim_thread(fp, || {
            let mut m = bench.start(&rules, true);
            let mut device = Device(&bench.replay);
            let mut differ = 0;
            let mut tried = 0;
            for t in 0..6000u32 {
                tick(&mut m, &mut device);
                if t < 600 || t % 200 != 0 {
                    continue;
                }
                let gobjs: Vec<_> = m.game.fighters.iter().filter_map(|(_, f)| f.gobj).collect();
                let Some(&dead) = gobjs.first() else { continue };
                let place = |x: f32| {
                    let mut g = m.game.clone();
                    let gobj = g.fighter_mut(dead);
                    gobj.motion_id = 6;
                    let Some(Obj::JObj(root)) = g.world.gobj(dead).obj else {
                        panic!("a fighter without its root joint");
                    };
                    let mut at = g.objs.jobjs.pool[root.0].translate;
                    at.x = x;
                    melee_sim::hsd::jobj::set_translate(&mut g.objs.jobjs, root, at);
                    let dir = opponents_dir(&mut g, &m.data, Vec3::new(0.0, 0.0, 0.0), None);
                    (dir.to_bits(), g.objs.rng.seed)
                };
                tried += 1;
                if place(-5000.0) != place(5000.0) {
                    differ += 1;
                }
            }
            (tried, differ)
        })
    };
    let (tried, differ) = side(vanilla(), FpMode::Hardware, &mut bench);
    assert!(
        tried > 0 && differ > 0,
        "under Vanilla's rules moving the KO'd fighter changed no side ({tried} states), so this checks nothing"
    );
    let (tried, differ) = side(Rules::slippi_online(), FpMode::Slippi, &mut bench);
    assert!(tried > 0, "no state to try");
    assert_eq!(
        differ, 0,
        "under Slippi 3.19's rules a fighter KO'd toward the screen moved the side in {differ} of {tried} states"
    );
}

/// The computer player's DI with no knockback (`ftCo_800AC5A0`) pushes the stick by r5 and
/// r30, registers it never set: the allocator model gives r30's fighter address, and the
/// frame's register bookkeeping follows r5 (`cpu_ai::Leftover`). Under 3.19's NanaDeterminism
/// both are cleared, so the bookkeeping is never consulted (the frame hands on a word that
/// stops by name if anything reads it) and the model can stay off. `ics-peach-ps` with the
/// model off stops by name where Nana's DI first reads her address (frame 6,808): from the tick
/// before, under 3.19's rules the same tick runs and she pushes the stick to (0, 0); under
/// 3.16's (r30 cleared, r5 followed) it runs too; under Vanilla's it stops by name.
#[test]
fn slippi_cpu_di_reads_neither_register() {
    use melee_sim::game::slippi::CodeSet;
    let Some(disc) = disc() else { return };
    let mut bench = Bench::open(disc, "ics-peach-ps");
    on_sim_thread(FpMode::Hardware, || {
        // The tick that stops, then the match again to the tick before it (ticks are
        // deterministic).
        let mut m = bench.start(&vanilla(), false);
        let mut device = Device(&bench.replay);
        let mut ticks = 0u64;
        let stop = loop {
            match catch_unwind(AssertUnwindSafe(|| tick(&mut m, &mut device))) {
                Ok((_, true)) => panic!("ics-peach-ps played to its end with the model off"),
                Ok(_) => ticks += 1,
                Err(p) => {
                    break p.downcast_ref::<String>().cloned().unwrap_or_default();
                }
            }
        };
        assert!(
            stop.contains("ftCo_800AC5A0: DI with no knockback reads what ftCo_800B2790"),
            "ics-peach-ps stopped elsewhere with the model off: {stop}"
        );
        let mut before = bench.start(&vanilla(), false);
        let mut device = Device(&bench.replay);
        for _ in 0..ticks {
            tick(&mut before, &mut device);
        }
        let frame = before.game.playback.as_ref().unwrap().frame_index;
        let run = |slippi: bool, codes: CodeSet| {
            let mut m = before.clone();
            m.game.rules.slippi = slippi;
            m.game.rules.codes = codes;
            let mut device = Device(&bench.replay);
            // The device answers by the frame playback asks for, so it picks up where the
            // match is.
            catch_unwind(AssertUnwindSafe(|| tick(&mut m, &mut device))).map(|_| {
                m.game
                    .fighters
                    .iter()
                    .find(|(_, f)| f.kind == FighterKind::Nana)
                    .map(|(_, f)| (f.cpu.lstick.x, f.cpu.lstick.y))
                    .expect("Nana")
            })
        };
        let stick = run(true, CodeSet::V3_19)
            .unwrap_or_else(|_| panic!("under 3.19's rules the DI at frame {} stopped", frame + 1));
        assert_eq!(
            stick,
            (0, 0),
            "under 3.19's rules Nana's DI pushes the stick by its cleared registers"
        );
        assert!(
            run(true, CodeSet::V3_16).is_ok(),
            "under 3.16's rules the DI at frame {} stopped",
            frame + 1
        );
        assert!(
            run(false, CodeSet::V3_19).is_err(),
            "under Vanilla's rules the DI at frame {} ran with the model off",
            frame + 1
        );
    });
}

/// Samus's grapple swing from the links' frames without the allocator model (design call 8 of
/// docs/slippi-ruleset.md): its grabbed-victim-only bit is bit 19 of her `ThrowN` joint's
/// address (`samusgrapple::SwingSlot::SavedR31`), which only the model gives and which no fixed
/// address stands in for (the bit depends on where the heap put the joint), so without the
/// model the read stops by name rather than taking 0. The `grapple` set's `grapple-samus-bf`
/// (ssbm-rs's `local/bench/oracle-m4-grapple`, skipped without `SSBM_RS_LOCAL`) swings bounced
/// first at frame 330 (`fn_802B8384`): played under its own rules with the model it goes past
/// the swing, and without it stops there, by name.
#[test]
fn grapple_swing_stops_by_name_without_the_model() {
    let Some(disc) = disc() else { return };
    let dir = melee_local_data::ssbm_rs_local("bench/oracle-m4-grapple");
    let path = std::path::Path::new(&dir).join("grapple-samus-bf.slp");
    if !path.exists() {
        eprintln!("{}: not here, skipping", path.display());
        return;
    }
    let mut bench = Bench::open_path(disc, &path);
    on_sim_thread(FpMode::Hardware, || {
        let rules = Settings::from_replay(&melee_slippi::start(&bench.replay))
            .expect("its settings")
            .rules;
        let frame = |m: &Match| m.game.playback.as_ref().unwrap().frame_index;
        let mut on = bench.start(&rules, true);
        let mut device = Device(&bench.replay);
        while frame(&on) < 400 {
            assert!(!tick(&mut on, &mut device).1, "the match ended");
        }
        let mut off = bench.start(&rules, false);
        let mut device = Device(&bench.replay);
        let (stop, at) = loop {
            let at = frame(&off);
            match catch_unwind(AssertUnwindSafe(|| tick(&mut off, &mut device))) {
                Ok((_, true)) => panic!("grapple-samus-bf played to its end with the model off"),
                Ok(_) => {}
                Err(p) => break (p.downcast_ref::<String>().cloned().unwrap_or_default(), at),
            }
        };
        assert!(
            stop.contains("it_802B7160 from fn_802B8384")
                && stop.contains("bit 19 of the ThrowN joint's address"),
            "grapple-samus-bf stopped elsewhere with the model off: {stop}"
        );
        assert_eq!(at + 1, 330, "the first bounced swing");
    });
}

/// A live match with Samus in it runs with the allocator model (Sarah, 2026-11-18: "Yes, model
/// on"; `Settings::live_needs_model`), laid out as playback's heap, so her grapple swing reads
/// her `ThrowN` joint's address (`samusgrapple::SwingSlot::SavedR31`) as Slippi Dolphin has it
/// instead of stopping by name. `grapple-samus-bf`'s game start, as a live match's settings,
/// keeps playback's layout, the one its playback runs with (`Settings::from_replay`, which the
/// oracle's `diff --heap` compares with ssbm-rs's trace, the joint's address included); played
/// with those settings' model it goes past its first bounced swing (frame 330), the joint's
/// address nonzero, and equals the playback's state there and at frame 400.
#[test]
fn a_live_match_with_samus_reads_her_swing_from_the_model() {
    use melee_sim::state_eq::StateEq;
    let Some(disc) = disc() else { return };
    let dir = melee_local_data::ssbm_rs_local("bench/oracle-m4-grapple");
    let path = std::path::Path::new(&dir).join("grapple-samus-bf.slp");
    if !path.exists() {
        eprintln!("{}: not here, skipping", path.display());
        return;
    }
    let mut bench = Bench::open_path(disc, &path);
    on_sim_thread(FpMode::Hardware, || {
        let start = melee_slippi::start(&bench.replay);
        let live = Settings::live(&start).expect("its settings");
        let played = Settings::from_replay(&start).expect("its settings");
        assert!(
            live.heap.is_some(),
            "a live match with Samus runs without the model"
        );
        assert_eq!(live.heap, played.heap, "not playback's layout");
        let frame = |m: &Match| m.game.playback.as_ref().unwrap().frame_index;
        let mut ours = bench.start(&played.rules, live.heap.is_some());
        let mut theirs = bench.start(&played.rules, true);
        let (mut a, mut b) = (Device(&bench.replay), Device(&bench.replay));
        while frame(&ours) < 400 {
            let at = frame(&ours);
            let stepped = catch_unwind(AssertUnwindSafe(|| tick(&mut ours, &mut a)));
            assert!(
                stepped.is_ok(),
                "the live match's settings stop at frame {}",
                at + 1
            );
            tick(&mut theirs, &mut b);
            if at + 1 == 330 {
                let addr = ours
                    .game
                    .it
                    .links
                    .iter()
                    .filter_map(|(_, l)| l.jobj)
                    .map(|j| ours.game.objs.jobjs.pool.addr(j.0))
                    .next()
                    .expect("the beam's links");
                assert_ne!(addr, 0, "the ThrowN joint without an address");
            }
            if matches!(at + 1, 330 | 400)
                && let Some(d) = ours.game.state_diff(&theirs.game)
            {
                panic!(
                    "frame {}: the live settings' run differs from playback's: {d}",
                    at + 1
                );
            }
        }
    });
}

/// The Super Scope's affordable level without the allocator model, under the Slippi ruleset
/// (`sscope::ADDRESS_STAND_IN`): with less ammunition than a level 1 shot, a charge past level
/// 0 takes the scope's GObj address for a level, which a hit while charging spends (over two
/// billion shots) and a charged shot fires (a click). Every heap address takes the same
/// branches, so a fixed address in the heap's range stands in for the model's.
///
/// The `sscope` oracle set's two matches (ssbm-rs's `local/bench/oracle-m4-sscope`, skipped
/// without `SSBM_RS_LOCAL`) play under 3.19's rules with the model and without it, recording
/// both. The model changes which pool slot each object takes, so the two states don't compare
/// slot by slot; what is compared is what the oracle's replay check compares and what players
/// see and hear: every tick's output events (sounds, rumble) and every event of the two
/// recordings (each frame's seed, each fighter's pre- and post-frame updates, each item's), all
/// equal but the post-frame state word (`mv.co.common.x0`, Slippi's "hitstun" word) where it
/// holds the address, the model's against the stand-in. And every tick each Super Scope's
/// ammunition (`xD4C`) is equal, or both runs' hold the refill and differ by the address alone.
/// Each match must reach its half of the glitch: the refill in `sscope-fd`, the click in
/// `sscope-click-fd`.
#[test]
fn slippi_super_scope_stand_in_matches_the_model() {
    use melee_sim::game::it::kinds::sscope::ADDRESS_STAND_IN;
    use melee_sim::state_eq::StateEq;
    use melee_slippi::replay::event::POST_FRAME;
    let Some(disc) = disc() else { return };
    let dir = melee_local_data::ssbm_rs_local("bench/oracle-m4-sscope");
    let stand_in = ADDRESS_STAND_IN.to_be_bytes();
    let mut disc = Some(disc);
    for (name, wants_refill) in [("sscope-fd", true), ("sscope-click-fd", false)] {
        let path = std::path::Path::new(&dir).join(format!("{name}.slp"));
        if !path.exists() {
            eprintln!("{}: not here, skipping", path.display());
            return;
        }
        let mut bench = Bench::open_path(disc.take().unwrap(), &path);
        let (refills, clicks, addrs, recordings) = on_sim_thread(FpMode::Slippi, || {
            let rules = Rules::slippi_online();
            let mut on = bench.start_with(&rules, true, true);
            let mut off = bench.start_with(&rules, false, true);
            let replay = &bench.replay;
            let (mut a, mut b) = (Device(replay), Device(replay));
            let (mut refills, mut clicks) = (0u32, 0u32);
            let mut addrs: Vec<i32> = Vec::new();
            for t in 0u32.. {
                let step = |m: &mut Match, d: &mut Device| {
                    let (mut f, render) = m.step_replay_tick(d);
                    if render {
                        f.draws = m.render();
                    }
                    let events = std::mem::take(&mut f.events);
                    let ended = f.ended || m.game.recorder.as_ref().is_some_and(|r| r.ended());
                    m.recycle(f);
                    (events, ended)
                };
                let (ea, end_a) = step(&mut on, &mut a);
                let (eb, end_b) = step(&mut off, &mut b);
                if let Some(d) = ea.state_diff(&eb) {
                    panic!("{name}: tick {t}'s events differ without the allocator model: {d}");
                }
                for (i, it) in on.game.items.iter() {
                    if it.kind != ItKind::SuperScope {
                        continue;
                    }
                    let addr = it.entity.map_or(0, |g| on.game.world.gobjs.addr(g.0)) as i32;
                    if !addrs.contains(&addr) {
                        addrs.push(addr);
                    }
                    let (want, got) = (it.xD4C, off.game.items[i].xD4C);
                    if want != got {
                        assert!(
                            want > 1_000_000_000
                                && got
                                    == want
                                        .wrapping_add(addr)
                                        .wrapping_sub(ADDRESS_STAND_IN as i32),
                            "{name}: tick {t}: the scope's ammunition is {got} without the model, {want} with it"
                        );
                        refills += 1;
                    }
                }
                for (i, f) in on.game.fighters.iter() {
                    let (want, got) = (f.mv_i32(0), off.game.fighters[i].mv_i32(0));
                    if want != got && addrs.contains(&want) && got == ADDRESS_STAND_IN as i32 {
                        clicks += 1;
                    }
                }
                if end_a || end_b {
                    assert_eq!(end_a, end_b, "{name}: tick {t}: one run ended");
                    break;
                }
            }
            let at = std::time::SystemTime::UNIX_EPOCH;
            let recording = |m: &Match| {
                let file = melee_slippi::write::recording(m.game.recorder.as_ref().unwrap(), at);
                melee_slippi::Replay::parse(&file)
                    .expect("the recording parses")
                    .events
            };
            (refills, clicks, addrs, (recording(&on), recording(&off)))
        });
        let (with, without) = recordings;
        assert_eq!(
            with.len(),
            without.len(),
            "{name}: the recordings' event counts differ"
        );
        let mut words = 0;
        for (n, (x, y)) in with.iter().zip(&without).enumerate() {
            if x == y {
                continue;
            }
            let at = x.payload.iter().zip(&y.payload).position(|(p, q)| p != q);
            let held = x.command == POST_FRAME
                && x.payload.len() == y.payload.len()
                && at.is_some_and(|at| {
                    (0..4).any(|k| {
                        let w = at.saturating_sub(k);
                        w + 4 <= x.payload.len()
                            && y.payload[w..w + 4] == stand_in
                            && addrs.iter().any(|a| x.payload[w..w + 4] == a.to_be_bytes())
                            && x.payload[..w] == y.payload[..w]
                            && x.payload[w + 4..] == y.payload[w + 4..]
                    })
                });
            assert!(
                held,
                "{name}: the recordings' event {n} (command {:#04x}) differs without the model beyond the state word",
                x.command
            );
            words += 1;
        }
        eprintln!(
            "{name}: {refills} ticks of the refill's ammunition, {clicks} of the state word holding the address ({words} post-frame updates), the rest equal"
        );
        if wants_refill {
            assert!(
                refills > 0,
                "{name}: no hit while charging spent the address"
            );
        } else {
            assert!(
                clicks > 0 && words > 0,
                "{name}: no charged shot took the address for a level"
            );
        }
        disc = Some(bench.disc);
    }
}

/// Onett's cars (design call 9, `live::DisplayPhase`): the coming car's two decisions through
/// the camera's view edges (`grOnett_801E43E0`'s horn past the right edge and its centre) are
/// also taken with the view of the display pass's other phase, which the tripwire keeps for
/// the tick before (`views[1]`, the other phase's view after a pass). The stage's match is played under 3.19's rules with the
/// display pass three times: as it plays; with that view moved far to the right before each
/// tick the car reads the edges (an other phase that decides differently, so those ticks are
/// noted); and with it moved straight up (a view that differs but meets the stage plane at the
/// same edges, so nothing more is noted). Gameplay, the cars' states included, is the same
/// every tick in all three: the tripwire only watches.
#[test]
fn slippi_onett_cars_note_the_other_phase() {
    use melee_sim::game::camera::View;
    use melee_sim::vec::Vec3;
    let Some(disc) = disc() else { return };
    let path = melee_local_data::ssbm_rs_local("bench/oracle-m4/stage-onett.slp");
    let path = std::path::Path::new(&path);
    if !path.exists() {
        eprintln!("{}: not here, skipping", path.display());
        return;
    }
    let mut bench = Bench::open_path(disc, path);
    let coming = |g: &Game| {
        g.grounds.iter().any(|(_, gr)| {
            matches!(&gr.u.vars, GroundVars::OnettCar(v) if matches!(v.state_a, 5 | 6) && v.x110 == 1)
        })
    };
    let car = |g: &Game| {
        g.grounds
            .iter()
            .find_map(|(_, gr)| match &gr.u.vars {
                GroundVars::OnettCar(v) => Some((v.state_a, v.x110, v.curr_car)),
                _ => None,
            })
            .map(|c| (c, g.stage_info.x740))
    };
    let runs = on_sim_thread(FpMode::Slippi, || {
        let rules = Rules::slippi_online();
        let mut play = |shift: Vec3| {
            let mut m = bench.start(&rules, true);
            let mut device = Device(&bench.replay);
            let (mut ticks, mut reads) = (Vec::new(), 0u32);
            loop {
                if coming(&m.game) {
                    reads += 1;
                    let d = &mut m.game.display_phase;
                    if let Some(v) = &mut d.views[1] {
                        *v = View {
                            fov: v.fov,
                            interest: v.interest + shift,
                            eye: v.eye + shift,
                        };
                    }
                }
                let (_, ended) = tick(&mut m, &mut device);
                ticks.push((digest(&m.game), car(&m.game)));
                if ended {
                    break;
                }
            }
            (ticks, reads, m.game.display_phase.differing)
        };
        [
            play(Vec3::ZERO),
            play(Vec3::new(3000.0, 0.0, 0.0)),
            play(Vec3::new(0.0, 500.0, 0.0)),
        ]
    });
    let [
        (plain, reads, noted),
        (right, _, noted_right),
        (up, _, noted_up),
    ] = runs;
    assert!(reads > 0, "no tick read the view's edges");
    for (name, other) in [("right", &right), ("up", &up)] {
        assert_eq!(plain.len(), other.len(), "moved {name}: the match's length");
        if let Some(t) = (0..plain.len()).find(|&t| plain[t] != other[t]) {
            panic!(
                "moved {name}: gameplay differs from tick {t}\n{:?}\n{:?}",
                plain[t], other[t]
            );
        }
    }
    eprintln!(
        "{reads} ticks read the edges; noted {noted} as played, {noted_right} moved right, {noted_up} moved up"
    );
    assert!(
        noted_right > noted,
        "the far view's decisions weren't noted"
    );
    assert_eq!(
        noted_up, noted,
        "a view meeting the plane at the same edges was noted"
    );
}

/// Playback's camera, pulled far back: its view's edges lie far past the stage's.
const PULLED_BACK: Vec3 = Vec3::new(0.0, 0.0, 5000.0);

/// Playback's followed camera (`camera::CameraState::followed`) made the camera's moved by
/// `shift` (both transforms' eye and interest), its boxes the camera's: the camera's readers are
/// then decided through a camera elsewhere (design call 9).
fn displace_playback_camera(g: &mut Game, shift: Vec3) {
    use melee_sim::game::camera::Course;
    if !g.follows_playback_camera() {
        g.start_following();
    }
    let mut c = Course::of(g.camera.raw_parts().cam);
    for t in [&mut c.transform, &mut c.transform_copy] {
        t.interest = t.interest + shift;
        t.position = t.position + shift;
    }
    *g.camera.raw_parts_mut().followed = Some(c);
}

/// One tick of `m` and its display pass when due, unless `skip` (a doubled round: the tick
/// after runs with no pass between): whether the pass was due, and whether the match is over.
fn tick_skipping(m: &mut Match, device: &mut Device, skip: bool) -> (bool, bool) {
    let (mut f, render) = m.step_replay_tick(device);
    if render && !skip {
        f.draws = m.render();
    }
    let ended = f.ended
        || device.0.frames.keys().next_back().is_some_and(|&last| {
            m.game
                .playback
                .as_ref()
                .is_some_and(|p| p.frame_index >= last)
        });
    m.recycle(f);
    (render, ended)
}

/// What a stage object of map `map` holds where its flight starts: each one's root joint's x.
fn objects_x(g: &Game, map: i32) -> Vec<u32> {
    g.grounds
        .iter()
        .filter(|(_, gr)| gr.map_id == map)
        .filter_map(|(_, gr)| melee_sim::game::gr::ground::jobj_of(g, gr.gobj?))
        .map(|j| g.objs.jobjs[j].translate.x.to_bits())
        .collect()
}

/// How [`doubled_round_notes`] finds the read it doubles a round for.
enum Find {
    /// The first read the tripwire notes as the replay plays.
    Noted,
    /// The first tick whose outcome changes (a flight setting off), with the camera pulled far
    /// back two ticks before in both runs, so that its view's edges lie past the stage's
    /// 200 and move from tick to tick as it eases in.
    PulledBack,
}

/// A display-phase read (design call 9, `live::DisplayPhase`) on a real doubled round: the
/// replay at `path` is played under 3.19's rules as it plays, and again with the display pass
/// skipped before the read's tick (`find`; that tick a doubled round's second, as Slippi
/// Dolphin's playback could have it). The doubled round changes what the read decides
/// (`outcome`) in that tick, and the tripwire notes the read in both runs: each run's other
/// phase is the other run. Gameplay is the same until that tick, and with `neutral` (a read
/// that reaches no gameplay) to the match's end. Played again with playback's followed camera
/// pulled far back (as a pause or the first frame's settling leaves it apart, but far), more reads
/// are noted, gameplay the same; and with it followed where the camera is, no more than as played.
fn doubled_round_notes(
    path: &std::path::Path,
    what: &str,
    find: Find,
    outcome: &(dyn Fn(&Game) -> String + Sync),
    neutral: bool,
) {
    let Some(disc) = disc() else { return };
    if !path.exists() {
        eprintln!("{}: not here, skipping", path.display());
        return;
    }
    let mut bench = Bench::open_path(disc, path);
    let rules = Rules::slippi_online();
    on_sim_thread(FpMode::Slippi, || {
        // Each tick: whether its pass was due, gameplay, the outcome, the count noted. The
        // pass after tick `skip` is skipped; the camera's eye is pulled back before `pull`;
        // with `off`, playback's camera is followed moved by it before every tick.
        let mut play = |skip: Option<usize>, pull: Option<usize>, off: Option<Vec3>| {
            let mut m = bench.start(&rules, true);
            let mut device = Device(&bench.replay);
            let mut ticks = Vec::new();
            for t in 0usize.. {
                if let Some(shift) = off {
                    displace_playback_camera(&mut m.game, shift);
                }
                if pull == Some(t) {
                    let p = &mut m.game.camera.raw_parts_mut().cam.transform.position;
                    p.z *= 20.0;
                }
                let (rendered, ended) = tick_skipping(&mut m, &mut device, skip == Some(t));
                ticks.push((
                    rendered,
                    digest(&m.game),
                    outcome(&m.game),
                    m.game.display_phase.differing,
                ));
                if ended {
                    break;
                }
            }
            ticks
        };
        let noted = |ticks: &[(bool, Digest, String, u32)], t: usize| {
            ticks[t].3 > if t == 0 { 0 } else { ticks[t - 1].3 }
        };
        let plain = play(None, None, None);
        let total = plain.last().map_or(0, |t| t.3);
        let (t, pull, plain) = match find {
            Find::Noted => {
                let t = (1..plain.len())
                    .find(|&t| noted(&plain, t) && plain[t - 1].0)
                    .unwrap_or_else(|| panic!("{what}: no read noted as played"));
                (t, None, plain)
            }
            Find::PulledBack => {
                let t = (4..plain.len())
                    .find(|&t| plain[t].2 != plain[t - 1].2 && plain[t - 1].0)
                    .unwrap_or_else(|| panic!("{what}: nothing set off"));
                assert!(!noted(&plain, t), "{what}: noted as played");
                (t, Some(t - 2), play(None, Some(t - 2), None))
            }
        };
        eprintln!(
            "{what}: {total} reads noted as played in {} ticks; the read in tick {t}",
            plain.len()
        );
        assert!(
            noted(&plain, t),
            "{what}: the read in tick {t} wasn't noted"
        );
        let doubled = play(Some(t - 1), pull, None);
        assert!(
            noted(&doubled, t),
            "{what}: the doubled round's read in tick {t} wasn't noted"
        );
        assert_ne!(
            plain[t].2, doubled[t].2,
            "{what}: the doubled round in tick {t} decided the same"
        );
        let same = if neutral { plain.len() } else { t };
        for u in 0..same.min(doubled.len()) {
            assert_eq!(plain[u].1, doubled[u].1, "{what}: gameplay in tick {u}");
        }
        if neutral {
            assert_eq!(plain.len(), doubled.len(), "{what}: the match's length");
        }
        // Playback's camera followed far back decides otherwise, followed where the camera is
        // the same.
        let off = play(None, None, Some(PULLED_BACK));
        let on = play(None, None, Some(Vec3::ZERO));
        let (reads, same) = (off.last().map_or(0, |t| t.3), on.last().map_or(0, |t| t.3));
        eprintln!(
            "{what}: {reads} reads noted with playback's camera far back, {same} with it here"
        );
        assert!(
            reads > total,
            "{what}: with playback's camera far back, no more reads noted"
        );
        assert_eq!(
            same, total,
            "{what}: playback's camera here, more reads noted"
        );
        for run in [&off, &on] {
            assert_eq!(run.len(), plain.len(), "{what}: the match's length");
            for u in 0..plain.len() {
                assert_eq!(
                    plain[u].1, run[u].1,
                    "{what}: gameplay with playback's camera followed, tick {u}"
                );
            }
        }
    });
}

/// Onett's cars (`grOnett_801E43E0`): the one read the stage's benchmark match notes as it
/// plays, on a real doubled round: the coming car decides otherwise with the view a tick old.
#[test]
fn slippi_onett_cars_note_a_doubled_round() {
    let car = |g: &Game| {
        let cars = g.grounds.iter().find_map(|(_, gr)| match &gr.u.vars {
            GroundVars::OnettCar(v) => Some((v.state_a, v.x110, v.curr_car)),
            _ => None,
        });
        format!("{cars:?} {} {:#x}", g.stage_info.x740, g.objs.rng.seed)
    };
    doubled_round_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "bench/oracle-m4/stage-onett.slp",
        )),
        "Onett's cars",
        Find::Noted,
        &car,
        false,
    );
}

/// Kongo Jungle 64's birds (`stageGObj2_GObjProc@802100FC`) start at the camera's view edge
/// where it lies past 200 from the stage's centre: with the camera pulled back as they set
/// off, a doubled round moves where they fly, a stage object's joint the oracle compares and
/// nothing gameplay reads. (As the benchmark match plays, the view's edges lie within 200 at
/// each of its three flights, and nothing is noted.)
#[test]
fn slippi_kongo_jungle_64_birds_note_a_doubled_round() {
    doubled_round_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "bench/oracle-m4/stage-kongo-jungle-64.slp",
        )),
        "Kongo Jungle 64's birds",
        Find::PulledBack,
        &|g| format!("{:?}", objects_x(g, 2)),
        true,
    );
}

/// Dream Land 64's clouds and stars (`grOldPupupu_80210D10`) start 50 past the camera's view
/// edge, or past 200 from the stage's centre where the edge is nearer: with the camera pulled
/// back as a flight sets off, a doubled round moves where it flies, stage objects' joints the
/// oracle compares and nothing gameplay reads. (As this ranked replay plays, the view's edges
/// lie within 200 at each of its three flights, and nothing is noted.)
#[test]
fn slippi_dream_land_64_flights_note_a_doubled_round() {
    doubled_round_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "replays/hf/ylink-dd-a6/slp/diamond-diamond-25f1c7b39bb0e353f2147ff8.slp",
        )),
        "Dream Land 64's flights",
        Find::PulledBack,
        &|g| format!("{:?}", objects_x(g, 2)),
        true,
    );
}

/// Pokémon Stadium's close-up (`grStadium_801D32D0`): where it copies from is its player's camera
/// bone on the screen through the game camera's CObj, which the display pass sets, so a
/// doubled round moves it in a tick the camera moved (docs/melee-bugs.md, "Pokémon Stadium's
/// monitor tests a render-pass camera": under Slippi's rules only where it copies from, which
/// players see in the monitor and the oracle compares, nothing gameplay reads). The first read
/// the benchmark match notes as it plays, on a real doubled round, copies from elsewhere.
#[test]
fn slippi_stadium_close_up_notes_a_doubled_round() {
    use melee_sim::game::gr::stadium::SCREEN_ZOOM;
    doubled_round_notes(
        &melee_local_data::bench_replay("fox-marth-ps"),
        "Pokémon Stadium's close-up",
        Find::Noted,
        &|g| {
            let z = g.gr.stadium.screens[usize::from(SCREEN_ZOOM)].as_ref();
            format!("{:?}", z.map(|z| (z.copy_x, z.copy_y)))
        },
        true,
    );
}

/// The camera's readers while playback's camera is followed apart from the live match's
/// (`docs/engine.md`, "The camera's readers after a pause", design call 9): the replay at `path`
/// played under 3.19's rules as it plays, with playback's camera followed moved by `shift` (its
/// eye and interest) before every tick, and followed where the camera is. Gameplay is the same
/// every tick in all three (the tripwire only watches); moved, more is noted than as played, and
/// with `entering`, each tick in which an item of that kind enters that state (where it reads the
/// camera) is noted; where the camera is, nothing more is noted. `prep` runs on the game before
/// every tick of every run (`true`), and after its logic, before its display pass (`false`).
fn off_course_notes(
    path: &std::path::Path,
    what: &str,
    shift: Vec3,
    entering: Option<(i32, i32)>,
    prep: &(dyn Fn(&mut Game, bool) + Sync),
) {
    let Some(disc) = disc() else { return };
    if !path.exists() {
        eprintln!("{}: not here, skipping", path.display());
        return;
    }
    let mut bench = Bench::open_path(disc, path);
    let rules = Rules::slippi_online();
    on_sim_thread(FpMode::Slippi, || {
        let mut play = |off: Option<Vec3>| {
            let mut m = bench.start(&rules, true);
            let mut device = Device(&bench.replay);
            let (mut ticks, mut entered, mut seen) = (Vec::new(), Vec::new(), Vec::new());
            for t in 0usize.. {
                prep(&mut m.game, true);
                if let Some(shift) = off {
                    displace_playback_camera(&mut m.game, shift);
                }
                let (mut f, render) = m.step_replay_tick(&mut device);
                prep(&mut m.game, false);
                if render {
                    f.draws = m.render();
                }
                let ended = f.ended
                    || device.0.frames.keys().next_back().is_some_and(|&last| {
                        (m.game.playback.as_ref()).is_some_and(|p| p.frame_index >= last)
                    });
                m.recycle(f);
                ticks.push((digest(&m.game), m.game.display_phase.differing));
                if let Some((kind, state)) = entering {
                    let now: Vec<u32> = (m.game.items.iter())
                        .filter(|(_, it)| it.kind == kind && it.msid == state)
                        .map(|(id, _)| id)
                        .collect();
                    if now.iter().any(|id| !seen.contains(id)) {
                        entered.push(t);
                    }
                    seen = now;
                }
                if ended {
                    break;
                }
            }
            (ticks, entered)
        };
        let (plain, _) = play(None);
        let (off, entered) = play(Some(shift));
        let (on, _) = play(Some(Vec3::ZERO));
        for run in [&off, &on] {
            assert_eq!(plain.len(), run.len(), "{what}: the match's length");
            for t in 0..plain.len() {
                assert_eq!(
                    plain[t].0, run[t].0,
                    "{what}: gameplay with playback's camera followed, tick {t}"
                );
            }
        }
        let noted = |ticks: &[(Digest, u32)]| ticks.last().map_or(0, |t| t.1);
        eprintln!(
            "{what}: {} ticks, {} reads noted with playback's camera moved, {} with it here, {} as played; entering in ticks {entered:?}",
            off.len(),
            noted(&off),
            noted(&on),
            noted(&plain)
        );
        assert!(
            noted(&off) > noted(&plain),
            "{what}: no more noted with playback's camera moved"
        );
        assert_eq!(
            noted(&on),
            noted(&plain),
            "{what}: more noted with playback's camera here"
        );
        if entering.is_some() {
            assert!(!entered.is_empty(), "{what}: no read in the match");
            for &t in &entered {
                let before = if t == 0 { 0 } else { off[t - 1].1 };
                assert!(
                    off[t].1 > before,
                    "{what}: the read in tick {t} wasn't noted"
                );
            }
        }
    });
}

/// Corneria's city part under the camera (`grCorneria_801E08CC`, read every tick for the
/// stage's colors and whether the Great Fox may start sinking), noted off course.
#[test]
fn slippi_corneria_city_notes_reads_off_course() {
    off_course_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "bench/oracle-m4/stage-corneria.slp",
        )),
        "Corneria's city part",
        Vec3::new(1000.0, 0.0, 0.0),
        None,
        &|_, _| {},
    );
}

/// Mute City's wrecked cars' smoke (`grMuteCity_801F1A34`, from the camera's eye), noted off
/// course where the wreck lies within the far limits. No benchmark match wrecks a car, so each
/// car's damage is set past the wreck's before every tick (in both runs).
#[test]
fn slippi_mute_city_wrecks_note_reads_off_course() {
    off_course_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "bench/oracle-m4/stage-mute-city.slp",
        )),
        "Mute City's wrecks",
        Vec3::new(0.0, 0.0, -10000.0),
        None,
        &|g, _| {
            for c in &mut g.gr.mutecity.cars {
                c.x10 = 1000.0;
            }
        },
    );
}

/// Pokémon Stadium's close-up (`grStadium_801D32D0`, where it copies from through the CObj),
/// noted off course.
#[test]
fn slippi_stadium_close_up_notes_reads_off_course() {
    off_course_notes(
        &melee_local_data::bench_replay("fox-marth-ps"),
        "Pokémon Stadium's close-up",
        PULLED_BACK,
        None,
        &|_, _| {},
    );
}

/// A benchmark match with Poké Balls (on Final Destination, where nothing else reads the
/// camera).
fn pokeball_match() -> std::path::PathBuf {
    melee_local_data::ssbm_rs_local("bench/oracle-m4/items-pokeball1-fd.slp").into()
}

/// The Unown's swarms (`it_802CE8D0`, toward the camera's interest), noted off course as each
/// Unown starts them (state 1).
#[test]
fn slippi_unown_notes_reads_off_course() {
    off_course_notes(
        &pokeball_match(),
        "the Unown",
        Vec3::new(0.0, 1000.0, 0.0),
        Some((172, 1)),
        &|_, _| {},
    );
}

/// Scizor flying off toward the camera (`itHassam_802CE400`, state 2), noted off course. No
/// benchmark match has a Scizor, so every ball lets one out (the start data's forced Pokémon,
/// `x28`); and its leap lasts 500 frames (`x1C`), by which it has left the blast zones, so it
/// flies off only with the leap cut to 20 frames (both runs).
#[test]
fn slippi_scizor_notes_reads_off_course() {
    off_course_notes(
        &pokeball_match(),
        "Scizor",
        Vec3::new(1000.0, 0.0, 0.0),
        Some((171, 2)),
        &|g, _| {
            g.vs.start.x28 = 171;
            let ids: Vec<u32> = g.items.iter().map(|(i, _)| i).collect();
            for i in ids {
                let it = &mut g.items[i];
                if it.kind == 171 && it.msid == 1 && it.xDAC_itcmd_var0 != 0 {
                    it.xD44_lifeTimer = it.xD44_lifeTimer.min(20.0);
                }
            }
        },
    );
}

/// An enemy flung toward the screen (`it_8027BA54`, here a Goomba entering state 11), noted
/// off course.
#[test]
fn slippi_flung_enemies_note_reads_off_course() {
    off_course_notes(
        std::path::Path::new(&melee_local_data::ssbm_rs_local(
            "bench/oracle-m4-monsters/monsters-goomba-ps.slp",
        )),
        "a Goomba flung toward the screen",
        Vec3::new(1000.0, 0.0, 0.0),
        Some((43, 0xB)),
        &|_, _| {},
    );
}

/// What following playback's camera costs (`camera::CameraState::followed`, design call 9;
/// `docs/performance.md`): `fox-marth-ps` and Corneria's oracle match played under 3.19's rules
/// as a live match plays (`Playback::no_settling`) with playback's camera followed from the first
/// frame until it meets this one (`Game::follow_playback_camera`, the oracle's), again as the
/// player plays it (the switch off: no second camera), and again with it followed to the
/// match's end (its interest's depth moved by 1 on the first tick, which
/// no update eases back, as a pause leaves it), five alternations each. Prints the ticks
/// followed, their time in the first two, the match's in all three, and a snapshot taken while
/// following and in the same tick unfollowed: its size and clone time.
#[test]
#[ignore = "a measurement: run alone on a quiet machine"]
fn measure_following_playbacks_camera() {
    use std::time::{Duration, Instant};
    let Some(disc) = disc() else { return };
    let rules = Rules::slippi_online();
    let mut disc = Some(disc);
    for (name, path) in [
        (
            "fox-marth-ps",
            melee_local_data::bench_replay("fox-marth-ps"),
        ),
        (
            "stage-corneria",
            melee_local_data::ssbm_rs_local("bench/oracle-m4/stage-corneria.slp").into(),
        ),
    ] {
        let mut bench = Bench::open_path(disc.take().unwrap(), &path);
        on_sim_thread(FpMode::Slippi, || {
            // Each tick's time, how many ticks it followed, and a snapshot at tick `SNAP`.
            const SNAP: usize = 30;
            let mut run = |follow: bool, to_the_end: bool| {
                let mut m = bench.start(&rules, false);
                m.game.playback.as_mut().unwrap().no_settling = true;
                m.game.follow_playback_camera = follow;
                let mut device = Device(&bench.replay);
                let (mut times, mut followed, mut snap) = (Vec::new(), 0, None);
                for t in 0usize.. {
                    let at = Instant::now();
                    let (_, ended) = tick(&mut m, &mut device);
                    times.push(at.elapsed());
                    if to_the_end && t == 0 {
                        let f = m
                            .game
                            .camera
                            .raw_parts_mut()
                            .followed
                            .as_mut()
                            .expect("followed");
                        f.transform.interest.z += 1.0;
                    }
                    if m.game.follows_playback_camera() {
                        followed += 1;
                    }
                    if t == SNAP {
                        snap = Some(m.game.clone());
                    }
                    if ended {
                        break;
                    }
                }
                (times, followed, snap.expect("the snapshot's tick"))
            };
            let (mut with, mut without, mut all) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..5 {
                with.push(run(true, false));
                without.push(run(false, false));
                all.push(run(true, true));
            }
            let followed = with[0].1;
            assert_eq!(without[0].1, 0, "the switch off followed");
            let sum = |t: &[Duration]| t.iter().sum::<Duration>();
            let median = |mut v: Vec<Duration>| {
                v.sort();
                v[v.len() / 2]
            };
            let window = |runs: &[(Vec<Duration>, usize, Box<Game>)]| {
                median(runs.iter().map(|r| sum(&r.0[..followed])).collect())
            };
            let whole = |runs: &[(Vec<Duration>, usize, Box<Game>)]| {
                median(runs.iter().map(|r| sum(&r.0)).collect())
            };
            let ticks = with[0].0.len();
            let (w_with, w_without) = (window(&with), window(&without));
            let (m_with, m_without) = (whole(&with), whole(&without));
            eprintln!(
                "{name}: {ticks} ticks, playback's camera followed for {followed}; those ticks {w_with:?} following, {w_without:?} with the switch off ({:.2} us a tick more); the match {m_with:?} following, {m_without:?} with the switch off; the window's extra {:.3}% of the match",
                (w_with.as_secs_f64() - w_without.as_secs_f64()) * 1e6 / followed as f64,
                (w_with.as_secs_f64() - w_without.as_secs_f64()) * 100.0 / m_without.as_secs_f64(),
            );
            let m_all = whole(&all);
            eprintln!(
                "{name}: followed to the end ({} ticks): the match {m_all:?}, {:.2}% more than with the switch off",
                all[0].1,
                (m_all.as_secs_f64() / m_without.as_secs_f64() - 1.0) * 100.0
            );
            for (what, g) in [("following", &with[0].2), ("the switch off", &without[0].2)] {
                let (_, allocs, bytes) = crate::counted(|| g.clone());
                let size = std::mem::size_of::<Game>() + bytes;
                let mut clones = (0..200)
                    .map(|_| {
                        let at = Instant::now();
                        std::hint::black_box(g.clone());
                        at.elapsed()
                    })
                    .collect::<Vec<_>>();
                clones.sort();
                eprintln!(
                    "{name}: tick {SNAP}'s snapshot {what}: {size} bytes, {allocs} allocations, clone {:?}",
                    clones[100]
                );
            }
        });
        disc = Some(bench.disc);
    }
}

/// No read of the camera in the settling window decides otherwise in playback's run (design
/// call 9; Sarah, 2026-11-18, on this evidence: the player runs no second camera there, and its
/// live match counts nothing in its first frames). Each replay is played as a live match plays
/// it (`Playback::no_settling`, Slippi 3.19's rules, the allocator model off) with playback's
/// camera followed (`Game::follow_playback_camera`) from the first frame's settling until it
/// meets the match's, and each read meanwhile decided both ways: none may be counted, and the
/// two must meet before frame 600 (or never part: Poké Floats and Icicle Mountain, whose
/// cameras the stage drives). The replays: the benchmark matches (`data/bench`), the oracle's
/// stage matches (`$SSBM_RS_LOCAL/bench/oracle-m4/stage-*`) and Sarah's (`$SLIPPI_REPLAYS`,
/// where set), each played only as far as the window (a match quit in it, as far as it goes); a
/// replay that doesn't start (another
/// build's characters or stages) is passed over and counted.
#[test]
fn no_settling_window_read_decides_otherwise() {
    use melee_sim::game::playback::FIRST_FRAME;
    let Some(disc) = disc() else { return };
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    let mut add = |dir: std::path::PathBuf, prefix: &str| {
        let mut walk = vec![dir];
        while let Some(d) = walk.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_dir() {
                    walk.push(p);
                } else if name.ends_with(".slp") && name.starts_with(prefix) {
                    paths.push(p);
                }
            }
        }
    };
    add(melee_local_data::bench_dir(), "");
    add(
        melee_local_data::ssbm_rs_local("bench/oracle-m4").into(),
        "stage-",
    );
    if let Some(d) = std::env::var_os("SLIPPI_REPLAYS") {
        add(d.into(), "");
    }
    paths.sort();
    let rules = Rules::slippi_online();
    let (mut disc, mut files) = (Some(disc), HashMap::new());
    let (mut checked, mut unstarted, mut quit, mut failures) = (0, 0, 0, Vec::new());
    let mut last_met = (FIRST_FRAME, String::new());
    for path in &paths {
        let name = path.display().to_string();
        let Ok(replay) = Replay::parse(&std::fs::read(path).expect("the replay")) else {
            unstarted += 1;
            continue;
        };
        let mut bench = Bench {
            replay,
            disc: disc.take().unwrap(),
            files: std::mem::take(&mut files),
        };
        let out = on_sim_thread(FpMode::Slippi, || {
            catch_unwind(AssertUnwindSafe(|| {
                let mut m = bench.start(&rules, false);
                m.game.playback.as_mut().unwrap().no_settling = true;
                m.game.follow_playback_camera = true;
                let mut device = Device(&bench.replay);
                let mut followed = false;
                loop {
                    let (_, ended) = tick(&mut m, &mut device);
                    let frame = m.game.playback.as_ref().unwrap().frame_index;
                    let following = m.game.follows_playback_camera();
                    let d = &m.game.display_phase;
                    if d.differing > 0 {
                        return Err(format!(
                            "{name}: {} reads decided otherwise by frame {frame}, the first at tick {:?}",
                            d.differing, d.first
                        ));
                    }
                    followed |= following;
                    if !following && (followed || frame > FIRST_FRAME) {
                        return Ok(Some(frame));
                    }
                    // A match quit in the window: nothing decided otherwise while it lasted.
                    if ended {
                        return Ok(None);
                    }
                    if frame >= 600 {
                        return Err(format!(
                            "{name}: playback's camera still apart at frame {frame}"
                        ));
                    }
                }
            }))
        });
        disc = Some(bench.disc);
        files = bench.files;
        match out {
            Ok(Ok(Some(frame))) => {
                checked += 1;
                if frame > last_met.0 {
                    last_met = (frame, name);
                }
            }
            Ok(Ok(None)) => {
                checked += 1;
                quit += 1;
            }
            Ok(Err(e)) => failures.push(e),
            Err(_) => unstarted += 1,
        }
    }
    eprintln!(
        "{checked} replays' settling windows decided both ways ({quit} quit before the cameras met), {} failed, {unstarted} didn't start; the cameras met by frame {} at the latest ({})",
        failures.len(),
        last_met.0,
        last_met.1
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(checked > 0, "no replay checked");
}
