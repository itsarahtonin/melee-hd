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
use melee_sim::game::gr::vars::GroundVars;
use melee_sim::game::state::{Game, Obj, Rules};
use melee_sim::game::structs::S32Vec2;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
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
            let Some(cam) = melee_sim::game::camera::gobj(&g.camera.cam) else {
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
/// model off stops by name where Nana's DI reads her address (frame 10,137): from the tick
/// before, under 3.19's rules the same tick runs and she pushes the stick to (0, 0); under
/// 3.16's (r30 cleared, r5 followed) it runs too; under Vanilla's it stops by name.
#[test]
fn slippi_cpu_di_reads_neither_register() {
    use melee_sim::game::ft::popo::KIND_NANA;
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
                    .find(|(_, f)| f.kind == KIND_NANA)
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
    use melee_sim::game::it::kinds::{S_SCOPE, sscope::ADDRESS_STAND_IN};
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
                    if it.kind != S_SCOPE {
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
