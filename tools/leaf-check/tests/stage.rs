//! The stage system against ssbm-rs from real game states: each test imports every capture of
//! one function, runs ssbm-rs's side (`run_capture`) and ours from the same state, and compares
//! the whole state afterwards, in both float modes.
//!
//! The captures are game memory, kept out of the repository. Regenerate them with ssbm-rs's
//! runner (its main since the state trace landed there):
//!
//! ```text
//! cargo build --release -p ssbm-run
//! CAPTURE=$STAGE_CALLS/fd1 CAPTURE_FUNCS=<names> CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   LOCKSTEP_DONE=<every function but those> target/release/ssbm-run "$SSBM_DISC" \
//!   --replay Game_20260129T170722.slp --fields 3000 --port all --lockstep \
//!   --known local/replays/known/Game_20260129T170722.txt
//! ```
//!
//! `STAGE_CALLS` names the directory (default below); tests skip when it has no capture of
//! their function.

use melee_local_data::ssbm_rs_local;
use std::path::PathBuf;

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, reference, run_capture};
use melee_capture::compare::Comparer;
use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, DiscIndex, find_archives_in};
use melee_data::archive::Archives;
use melee_sim::game::gr::ground;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::Ctx;

const DEFAULT_CALLS: &str = "calls/stage";

fn calls_dir() -> PathBuf {
    std::env::var("STAGE_CALLS")
        .unwrap_or_else(|_| ssbm_rs_local(DEFAULT_CALLS))
        .into()
}

/// Every capture of `func` under the calls directory, in name order.
fn captures(func: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(calls_dir()) else {
        return out;
    };
    for d in dirs.flatten() {
        for f in melee_capture::pack::entries(d.path()) {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            if name.starts_with(&format!("{func}-call")) && name.ends_with(".call") {
                out.push(f);
            }
        }
    }
    out.sort();
    out
}

struct Env {
    ctx: Ctx,
    /// The disc's archives, indexed once for every check.
    index: &'static DiscIndex,
}

fn env() -> Option<Env> {
    let ctx = reference()?;
    Some(Env {
        ctx,
        index: leaf_check::disc_index()?,
    })
}

/// What a test runs on our side, given the imported state and the captured call.
type Ours = dyn Fn(&mut Running, &Capture, &melee_capture::import::Maps);

/// Runs every capture of `func` on both sides and requires the states to agree.
fn check(func: &str, ours: &Ours) {
    let files = captures(func);
    if files.is_empty() {
        eprintln!(
            "no captures of {func} in {}: skipping (see this file's docs to make them)",
            calls_dir().display()
        );
        return;
    }
    let Some(env) = env() else { return };
    let mut blocked = Vec::new();
    'captures: for path in &files {
        let cap = Capture::read(path);
        // The state, imported once (in the first mode, as each mode's run did) for both modes'
        // runs, each from its own copy.
        set_fp_mode(MODES[0]);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(env.index, &cap.call.mem1, &mut archives),
        };
        let mut imp = Importer::new(&cap, &data);
        imp.world();
        imp.stage();
        imp.ids();
        let Importer {
            game: imported,
            maps: imported_maps,
            ..
        } = imp;
        for mode in MODES {
            set_fp_mode(mode);
            let mut game = imported.clone();
            let maps = imported_maps.clone();
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ours(
                    &mut Running {
                        game: &mut game,
                        data: &archives,
                    },
                    &cap,
                    &maps,
                )
            }));
            if let Err(p) = ran {
                match unported(&*p) {
                    Some(why) => {
                        blocked.push(format!(
                            "{}: {why}",
                            path.file_name().unwrap().to_string_lossy()
                        ));
                        continue 'captures;
                    }
                    None => std::panic::resume_unwind(p),
                }
            }
            let after = run_capture(&env.ctx, &cap.call);
            let mut cmp = Comparer::new(&after, &data, &game, maps);
            cmp.world();
            cmp.stage();
            cmp.ids();
            let name = path.file_name().unwrap().to_string_lossy();
            if !cmp.diffs.is_empty() {
                for d in cmp.diffs.iter().take(30) {
                    eprintln!("  {d}");
                }
                panic!("{name} ({mode:?}): {} differences", cmp.diffs.len());
            }
            assert_eq!(game.objs.stale_reads, 0, "{name}: stale reads");
        }
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "{func}: {} captures agree, {} reach unported code",
        files.len() - blocked.len(),
        blocked.len()
    );
    for b in &blocked {
        eprintln!("  {b}");
    }
    assert!(
        blocked.len() < files.len(),
        "{func}: every capture reaches unported code"
    );
}

/// Why our side stopped, if it reached code no port covers yet (`unimplemented!`, an unported
/// function pointer, or a particle event the effects would handle): a boundary, not a
/// difference.
fn unported(p: &(dyn std::any::Any + Send)) -> Option<String> {
    let msg = p
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))?;
    (msg.starts_with("not implemented")
        || msg.contains("isn't ported")
        || msg.contains("need the effect port"))
    .then_some(msg)
}

/// The GObj a process was called with (r3).
fn gobj_arg(cap: &Capture, maps: &melee_capture::import::Maps) -> GObjId {
    maps.gobjs[&cap.call.regs.gpr[3]]
}

/// A GObj process: our port of the captured function, called with its GObj.
fn check_proc(func: &str, run: fn(&mut Running, GObjId)) {
    check(func, &move |r, cap, maps| run(r, gobj_arg(cap, maps)));
}

#[test]
fn ground_proc_anim() {
    check_proc("Ground_801C1CD0", ground::proc_anim);
}

#[test]
fn ground_proc_callback() {
    check_proc("Ground_801C1D38", ground::proc_callback);
}

#[test]
fn ground_proc_watch() {
    check_proc("Ground_801C0C2C", ground::proc_watch);
}

#[test]
fn ground_proc_lights() {
    check_proc("Ground_801C461C", ground::proc_lights);
}

#[test]
fn stage_collision_proc() {
    check_proc("mpLib_800587FC", |r, gobj| {
        melee_sim::game::code::run_proc(r, gobj, melee_sim::game::handles::CodeRef(0x8005_87FC))
    });
}

#[test]
fn last_proc_main() {
    check_proc("grLast_8021AAB0", melee_sim::game::gr::last::proc_main);
}

#[test]
fn last_proc_sway() {
    check_proc("grLast_8021ADD0", melee_sim::game::gr::last::proc_sway);
}

#[test]
fn last_empty_proc() {
    check_proc("grLast_8021A914", |r, gobj| {
        melee_sim::game::code::run_proc(r, gobj, melee_sim::game::handles::CodeRef(0x8021_A914))
    });
}

/// The ported stage processes, by address: what a stage tick runs.
const STAGE_PROCS: &[u32] = &[
    0x801C_1CD0,
    0x801C_1D38,
    0x801C_0C2C,
    0x801C_461C,
    0x8005_87FC,
    0x8021_A914,
    0x8021_A968,
    0x8021_A9A4,
    0x8021_AAB0,
    0x8021_AB80,
    0x8021_ABD4,
    0x8021_AC28,
    0x8021_ADD0,
    0x8021_B28C,
    0x8021_B2E0,
    0x801C_ADBC,
    0x8021_A114,
    0x8021_A174,
    0x8021_A204,
    0x8021_A26C,
    0x8021_A2D4,
    0x8021_A33C,
    0x8021_A3BC,
    // The stage's quake models (grLib_801C9BC8, grLib_801C9C40).
    0x801C_9BC8,
    0x801C_9C40,
    // efLib's particle processes, which run the generators the stages' animations spawn.
    0x8005_C9A4,
    0x8005_C9D0,
];

/// Runs the stage's processes alone, in the scheduler's order, for `ticks` ticks on both sides
/// from a captured state, comparing every `every` ticks: the stage's logic over a long stretch
/// of play (Final Destination's scenes change every 30 seconds or so), from one capture.
/// The pairs a comparison of a later tick starts from: none. The original's heap hands a
/// freed object's block to the next allocation that fits, where our pools reuse the slot freed
/// last, so an address can change hands between two comparisons (Final Destination's phase 14
/// frees five pieces and makes a sixth); each comparison pairs what it reaches afresh, and
/// within it every object still pairs with one address.
fn fresh(_: Maps) -> Maps {
    Maps::default()
}

fn stage_ticks(path: &std::path::Path, ticks: u32, every: u32) {
    let Some(env) = env() else { return };
    let cap = Capture::read(path);
    set_fp_mode(FpMode::Slippi);
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives_in(env.index, &cap.call.mem1, &mut archives),
    };
    let mut imp = Importer::new(&cap, &data);
    imp.world();
    imp.stage();
    imp.ids();
    let Importer {
        mut game, mut maps, ..
    } = imp;
    cap.call.load(&env.ctx);
    // A replay's Slippi codes patch functions the processes run (Final Destination's cycle
    // keeps the random seed to itself): ssbm-rs runs those as the run did.
    leaf_check::register_patched(&env.ctx);
    let ctx = &env.ctx;
    let mut mem = vec![0u8; cap.call.mem1.len()];
    // STAGE_DEBUG=TICK compares after every process from that tick on.
    let debug_from: Option<u32> = std::env::var("STAGE_DEBUG")
        .ok()
        .map(|v| v.parse().unwrap());
    for tick in 1..=ticks {
        let w = &game.world;
        let mut order = Vec::new();
        for &head in &w.proc_head {
            let mut p = head;
            while let Some(id) = p {
                let pr = &w.procs[id.0];
                if STAGE_PROCS.contains(&pr.on_invoke.0) {
                    order.push((pr.gobj, pr.on_invoke));
                }
                p = pr.next;
            }
        }
        let mut addr_of: std::collections::HashMap<GObjId, u32> =
            maps.gobjs.iter().map(|(&a, &g)| (g, a)).collect();
        for (gobj, f) in order {
            // A process an earlier one removed, with its GObj, doesn't run.
            let w = &game.world;
            let live = std::iter::successors(w.gobjs[gobj.0].proc, |&p| w.procs[p.0].child)
                .any(|p| w.procs[p.0].on_invoke == f && w.procs[p.0].gobj == gobj);
            if !live {
                continue;
            }
            if !addr_of.contains_key(&gobj) {
                // A GObj made since the last comparison: compare again, which pairs it.
                let _ = ctx.mem.read_bytes(0x8000_0000, &mut mem);
                let mut cmp = Comparer::new(&mem, &data, &game, fresh(maps));
                cmp.world();
                cmp.stage();
                cmp.ids();
                if !cmp.diffs.is_empty() {
                    for d in cmp.diffs.iter().take(30) {
                        eprintln!("  {d}");
                    }
                    panic!("tick {tick}: {} differences", cmp.diffs.len());
                }
                maps = cmp.maps;
                addr_of = maps.gobjs.iter().map(|(&a, &g)| (g, a)).collect();
            }
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                melee_sim::game::code::run_proc(
                    &mut Running {
                        game: &mut game,
                        data: &archives,
                    },
                    gobj,
                    f,
                )
            }));
            if let Err(p) = ran {
                let Some(why) = unported(&*p) else {
                    eprintln!("tick {tick}: {f:#x?} on {gobj:?} panicked");
                    std::panic::resume_unwind(p)
                };
                set_fp_mode(FpMode::Hardware);
                assert!(
                    tick > every,
                    "tick {tick}: unported code before the first comparison"
                );
                eprintln!(
                    "ticks agree up to tick {}; tick {tick} reaches unported code: {why}",
                    tick - 1
                );
                return;
            }
            ctx.regs.restore(&cap.call.regs);
            let Some(&their_gobj) = addr_of.get(&gobj) else {
                panic!("tick {tick}: {f:#x?} on {gobj:?}, which ssbm-rs's state has no GObj for");
            };
            ctx.regs.set_r(3, their_gobj);
            ctx.invoke(f.0);
            if debug_from.is_some_and(|d| tick >= d) {
                let _ = ctx.mem.read_bytes(0x8000_0000, &mut mem);
                let mut cmp = Comparer::new(&mem, &data, &game, maps.clone());
                cmp.world();
                cmp.stage();
                if !cmp.diffs.is_empty() {
                    for d in cmp.diffs.iter().take(10) {
                        eprintln!("  {d}");
                    }
                    panic!("tick {tick}: {f:#x?} on {gobj:?} differs");
                }
            }
        }
        if std::env::var_os("STAGE_TRACE").is_some() {
            let g = game.stage_info.map_gobjs[3].unwrap();
            let gp = melee_sim::game::gr::ground::ground_of(&game, g);
            if let melee_sim::game::gr::GroundVars::Map(v) = &game.grounds[gp.0].u {
                eprintln!("tick {tick}: phase {} b26 {} xC8 {}", v.phase, v.b26, v.xC8);
            }
        }
        if tick % every == 0 || tick == ticks {
            let _ = ctx.mem.read_bytes(0x8000_0000, &mut mem);
            let mut cmp = Comparer::new(&mem, &data, &game, fresh(maps));
            cmp.world();
            cmp.stage();
            cmp.ids();
            if !cmp.diffs.is_empty() {
                for d in cmp.diffs.iter().take(30) {
                    eprintln!("  {d}");
                }
                panic!(
                    "{}: tick {tick}: {} differences",
                    path.display(),
                    cmp.diffs.len()
                );
            }
            maps = cmp.maps;
        }
    }
    set_fp_mode(FpMode::Hardware);
    let main = game.stage_info.map_gobjs[3].map(|g| {
        let gp = melee_sim::game::gr::ground::ground_of(&game, g);
        game.grounds[gp.0].u.clone()
    });
    eprintln!("{ticks} ticks agree; map 3 ends at {main:?}");
}

/// Final Destination's stage processes over eleven minutes of play from the first capture with
/// the cycle running, with the particle system's processes and the quake model's: the whole
/// background cycle three times over (phase 9's generator following the swaying piece, the
/// quake through phases 10 and 11, the fog blends, the closing scenes' material scripts and
/// pieces, and the cycle's return to phase 1).
#[test]
fn last_stage_ticks() {
    // The first capture once the match has started the cycle (the start hook clears `b0`).
    let started = |p: &std::path::Path| {
        let cap = Capture::read(p);
        let info = leaf_check::sym("stage_info");
        let map3 = cap.u32(info + 0x180 + 4 * 3);
        let gp = cap.u32(map3 + 0x2C);
        cap.u32(gp + 0xC4) & 0x8000_0000 == 0
    };
    let Some(first) = captures("grLast_8021AAB0").into_iter().find(|p| started(p)) else {
        eprintln!("no capture of grLast_8021AAB0 with the cycle running: skipping");
        return;
    };
    let every = std::env::var("STAGE_EVERY").map_or(600, |v| v.parse().unwrap());
    let ticks = std::env::var("STAGE_TICKS").map_or(40000, |v| v.parse().unwrap());
    stage_ticks(&first, ticks, every);
}

/// The call's `i`th integer argument (r3 on).
fn int_arg(cap: &Capture, i: usize) -> u32 {
    cap.call.regs.gpr[3 + i]
}

/// The stage kind of a `StageIdPair*` argument.
fn pair_grkind(cap: &Capture) -> i32 {
    cap.u32(int_arg(cap, 0)) as i32
}

use melee_sim::game::gr::{last, setup};

/// Captures from the other tournament stages (Pokémon Stadium, Yoshi's Story, Fountain of Dreams,
/// Dream Land) and Final Destination's quake, one directory each: see
/// `local/calls/stage-more/capture.sh` in ssbm-rs's tree for how they were made.
const MORE_CALLS: &str = "calls/stage-more";

/// Every capture of [`MORE_CALLS`] imports and compares with its own memory with no difference:
/// the stages' objects, their archives (Pokémon Stadium's transformation files among them) and
/// the effects and particles they hold.
#[test]
fn tournament_stage_imports_round_trip() {
    let Some(env) = env() else { return };
    let dir = std::env::var("STAGE_MORE_CALLS").unwrap_or_else(|_| ssbm_rs_local(MORE_CALLS));
    let Ok(dirs) = std::fs::read_dir(&dir) else {
        eprintln!("no captures in {dir}: skipping");
        return;
    };
    let (mut n, mut failed) = (0, Vec::new());
    for d in dirs.flatten().filter(|d| d.path().is_dir()) {
        let files = melee_capture::pack::call_files(d.path());
        for path in files {
            let cap = Capture::read(&path);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(env.index, &cap.call.mem1, &mut archives),
            };
            let mut imp = Importer::new(&cap, &data);
            imp.world();
            imp.stage();
            imp.ids();
            let Importer { game, maps, .. } = imp;
            let mut cmp = Comparer::new(&cap.call.mem1, &data, &game, maps);
            cmp.world();
            cmp.stage();
            cmp.ids();
            if cmp.diffs.is_empty() {
                n += 1;
            } else {
                failed.push(format!("{}: {}", path.display(), cmp.diffs[0]));
            }
        }
    }
    eprintln!("{n} captures round-trip");
    for f in &failed {
        eprintln!("  {f}");
    }
    assert!(
        failed.is_empty(),
        "{} captures don't round-trip",
        failed.len()
    );
}

// The quake model, from the quake captures of every tournament stage but Battlefield (the
// `quake` directory: Final Destination's looping quake, and the fighters' landings' small
// ones elsewhere).

#[test]
fn quake_model() {
    check("grLib_801C9CEC", &|r, cap, _| {
        let _ = melee_sim::game::gr::grlib::quake_model(r, int_arg(cap, 0) as i32);
    });
}

#[test]
fn quake_proc() {
    check_proc("grLib_801C9C40", |r, g| {
        melee_sim::game::gr::grlib::quake_proc(r, g, false)
    });
}

#[test]
fn quake_loop_proc() {
    check_proc("grLib_801C9BC8", |r, g| {
        melee_sim::game::gr::grlib::quake_proc(r, g, true)
    });
}

#[test]
fn request_quake() {
    check("Camera_RequestQuake", &|r, cap, _| {
        let kind = int_arg(cap, 0) as i32;
        let pos = int_arg(cap, 1);
        let pos = (pos != 0).then(|| {
            let f = |o: u32| f32::from_bits(cap.u32(pos + o));
            melee_sim::vec::Vec3::new(f(0), f(4), f(8))
        });
        melee_sim::game::camera::request_quake(r, kind, pos);
    });
}

#[test]
fn ground_reset() {
    check("Ground_801BFFB0", &|r, _, _| setup::reset(r.game));
}

#[test]
fn ground_prepare() {
    check("Ground_801C0378", &|r, _, _| setup::prepare(r.game));
}

#[test]
fn ground_setup() {
    check("Ground_801C0800", &|r, cap, _| {
        setup::setup(r, pair_grkind(cap))
    });
}

#[test]
fn ground_on_load() {
    check("Ground_OnLoad", &|r, cap, _| {
        setup::on_load(r, pair_grkind(cap))
    });
}

#[test]
fn ground_start() {
    check("Ground_801C0FB8", &|r, cap, _| {
        setup::start(r, pair_grkind(cap))
    });
}

#[test]
fn ground_fog() {
    check("Ground_801C1E94", &|r, _, _| {
        setup::create_fog(r.game, r.data)
    });
}

#[test]
fn ground_lights() {
    check("Ground_801C466C", &|r, _, _| {
        setup::create_lights(r.game, r.data)
    });
}

#[test]
fn ground_stage_gobj() {
    check("Ground_GetStageGObj", &|r, cap, _| {
        setup::stage_gobj(r, int_arg(cap, 0) as i32, setup::SETUP_FRAME);
    });
}

#[test]
fn collision_load() {
    check("mpLibLoad", &|r, _, _| {
        setup::load_collision(r.game, r.data)
    });
}

#[test]
fn collision_gobj() {
    check("mpLib_80058820", &|r, _, _| {
        setup::create_collision_gobj(r.game)
    });
}

#[test]
fn last_on_init() {
    check("grLast_OnInit", &|r, _, _| last::on_init(r));
}

#[test]
fn last_on_start() {
    check("grLast_OnStart", &|r, _, _| last::on_start(r));
}

#[test]
fn last_create() {
    check("grLast_8021A7F4", &|r, cap, _| {
        last::create(r, int_arg(cap, 0) as i32, setup::SETUP_FRAME);
    });
}

#[test]
fn last_init_main() {
    check_proc("grLast_8021A9C4", last::init_main);
}

#[test]
fn last_set_phase() {
    check("grLast_8021B920", &|r, cap, maps| {
        last::set_phase(
            r,
            gobj_arg(cap, maps),
            int_arg(cap, 1) as i32,
            last::CYCLE_PIECES_FRAME,
        )
    });
}

// Differential tests on captured states: functions no capture reached, run on both sides from
// a real stage.

use leaf_check::handle;
use melee_capture::import::Maps;
use ssbm_types::fns;
use ssbm_types::records::HSD_GObj;

/// The first capture of `func` with the cycle running (the state these tests start from).
fn running_state() -> Option<PathBuf> {
    captures("grLast_8021AAB0").into_iter().nth(2)
}

/// A step of [`on_state`]: its name, the reference's side and ours.
type Step<'a> = (
    &'a str,
    &'a dyn Fn(&Ctx, &Maps),
    &'a dyn Fn(&mut Running, &Maps),
);

/// Imports a capture's state, runs `steps` on both sides (each step its ssbm-rs side, then
/// ours), and requires the states to agree after each.
fn on_state(path: &std::path::Path, steps: &[Step]) {
    on_state_with(path, steps, false);
}

/// [`on_state`], with the text system and the HUD's globals imported and compared too when
/// `hud` (Pokémon Stadium's screens write text).
fn on_state_with(path: &std::path::Path, steps: &[Step], hud: bool) {
    on_state_opts(path, steps, hud, false);
}

/// [`on_state_with`], with the audio state imported and compared too when `audio` (a stage
/// object's sounds).
fn on_state_opts(path: &std::path::Path, steps: &[Step], hud: bool, audio: bool) {
    let Some(env) = env() else { return };
    let cap = Capture::read(path);
    for mode in MODES {
        set_fp_mode(mode);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(env.index, &cap.call.mem1, &mut archives),
        };
        let bank = audio.then(|| melee_capture::audio::find_bank(&cap.call.mem1, &mut archives));
        let mut imp = Importer::new(&cap, &data);
        if audio {
            imp = imp.with_archives(&archives);
        }
        imp.world();
        imp.stage();
        imp.ids();
        if hud {
            imp.hud();
        }
        if let Some(bank) = bank {
            imp.audio(bank);
        }
        let Importer {
            mut game, mut maps, ..
        } = imp;
        cap.call.load(&env.ctx);
        let mut mem = vec![0u8; cap.call.mem1.len()];
        for (name, theirs, ours) in steps {
            env.ctx.regs.restore(&cap.call.regs);
            theirs(&env.ctx, &maps);
            ours(
                &mut Running {
                    game: &mut game,
                    data: &archives,
                },
                &maps,
            );
            let _ = env.ctx.mem.read_bytes(0x8000_0000, &mut mem);
            // Objects pair afresh by where they're reached from: a freed object's address or
            // slot may be reused by a different object on each side (the heap reuses blocks by
            // address, pools by last freed).
            let mut cmp = Comparer::new(&mem, &data, &game, Maps::default());
            cmp.world();
            cmp.stage();
            cmp.ids();
            if hud {
                cmp.hud();
            }
            if audio {
                cmp.audio();
            }
            if !cmp.diffs.is_empty() {
                for d in cmp.diffs.iter().take(30) {
                    eprintln!("  {d}");
                }
                panic!("{name} ({mode:?}): {} differences", cmp.diffs.len());
            }
            maps = cmp.maps;
        }
    }
    set_fp_mode(FpMode::Hardware);
}

/// The address of map `id`'s stage object in the captured state.
fn map_addr(ctx: &Ctx, id: u32) -> u32 {
    ctx.read_u32(leaf_check::sym("stage_info") + 0x180 + 4 * id)
}

/// The reference's stage object for map `id`.
fn piece(ctx: &Ctx, id: u32) -> HSD_GObj<'_> {
    handle::<HSD_GObj>(ctx, map_addr(ctx, id))
}

/// Our stage object for map `id`.
fn our_map(r: &Running, id: u32) -> GObjId {
    r.game.stage_info.map_gobjs[id as usize].expect("a map object")
}

thread_local! {
    /// What the reference's side of a step answered, for ours to compare.
    static ANSWER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn answer(v: u32) {
    ANSWER.with(|a| a.set(v));
}

fn answered() -> u32 {
    ANSWER.with(|a| a.get())
}

/// The granime helpers no capture reached, on background pieces' parts.
#[test]
fn anime_helpers() {
    use melee_sim::game::gr::{anime, grlib};
    let Some(path) = running_state() else { return };
    let f = leaf_check::arg;
    for map in [4u32, 5, 7] {
        on_state(
            &path,
            &[
                (
                    "grAnime_801C775C",
                    &move |ctx, _| {
                        fns::grAnime_801C775C(ctx, piece(ctx, map), 1, 7, f(5.0), f(50.0))
                    },
                    &move |r, _| anime::loop_between(r.game, our_map(r, map), 1, 7, 5.0, 50.0),
                ),
                (
                    "grAnime_801C77FC",
                    &move |ctx, _| fns::grAnime_801C77FC(ctx, piece(ctx, map), 0, 3),
                    &move |r, _| anime::set_loop(r.game, our_map(r, map), 0, 3, true),
                ),
                (
                    "grAnime_801C787C",
                    &move |ctx, _| fns::grAnime_801C787C(ctx, piece(ctx, map), 2, 6),
                    &move |r, _| anime::set_loop(r.game, our_map(r, map), 2, 6, false),
                ),
                (
                    "grAnime_801C78FC",
                    &move |ctx, _| fns::grAnime_801C78FC(ctx, piece(ctx, map), 1, 7),
                    &move |r, _| anime::clear_loop(r.game, our_map(r, map), 1, 7, true),
                ),
                (
                    "grAnime_801C7A04",
                    &move |ctx, _| fns::grAnime_801C7A04(ctx, piece(ctx, map), 1, 5, f(0.5)),
                    &move |r, _| anime::set_rate(r.game, our_map(r, map), 1, 5, 0.5, true),
                ),
                (
                    "grAnime_801C7B24",
                    &move |ctx, _| fns::grAnime_801C7B24(ctx, piece(ctx, map), 1, 7, f(12.0)),
                    &move |r, _| anime::req_anim(r.game, our_map(r, map), 1, 7, 12.0, true),
                ),
                (
                    "grAnime_801C7BA0",
                    &move |ctx, _| fns::grAnime_801C7BA0(ctx, piece(ctx, map), 2, 3, f(3.0)),
                    &move |r, _| anime::req_anim(r.game, our_map(r, map), 2, 3, 3.0, false),
                ),
                (
                    "grAnime_801C84A4",
                    &move |ctx, _| answer(fns::grAnime_801C84A4(ctx, piece(ctx, map), 1, 7) as u32),
                    &move |r, _| {
                        let ours = anime::rewound(r.game, our_map(r, map), 1, 7);
                        assert_eq!(ours, answered() != 0, "grAnime_801C84A4 on map {map}");
                    },
                ),
                (
                    "grLib_801C96E8",
                    &move |ctx, _| answer(fns::grLib_801C96E8(ctx, piece(ctx, map)) as u32),
                    &move |r, _| {
                        let ours = grlib::overlay_done(r.game, our_map(r, map));
                        assert_eq!(ours, answered() != 0, "grLib_801C96E8 on map {map}");
                    },
                ),
            ],
        );
    }
}

/// Calls an original function (or ssbm-rs's port of it) by address with integer and float
/// arguments.
fn invoke(ctx: &Ctx, addr: u32, ints: &[u32], floats: &[f32]) {
    for (i, &v) in ints.iter().enumerate() {
        ctx.regs.set_r(3 + i, v);
    }
    for (i, &v) in floats.iter().enumerate() {
        ctx.regs.set_f(1 + i, leaf_check::arg(v));
    }
    ctx.invoke(addr);
}

/// The fog color helpers, the stage's fog switch, Final Destination's color blends and
/// scene colors, on a real stage.
#[test]
fn ground_colors() {
    use melee_sim::game::gr::ground;
    use melee_sim::game::structs::_GXColor;
    let Some(path) = running_state() else { return };
    let at = leaf_check::SCRATCH;
    on_state(
        &path,
        &[
            (
                "Ground_801C1E00",
                &|ctx, _| fns::Ground_801C1E00(ctx, 0),
                &|r, _| ground::set_fog_enabled(r.game, false),
            ),
            (
                "Ground_801C205C",
                &|ctx, _| {
                    ctx.write_u32(at, 0x1234_5678);
                    fns::Ground_801C205C(ctx, handle(ctx, at));
                },
                &|r, _| {
                    let c = _GXColor {
                        r: 0x12,
                        g: 0x34,
                        b: 0x56,
                        a: 0x78,
                    };
                    ground::set_fog_color(r.game, c)
                },
            ),
            (
                "Ground_801C2090",
                &|ctx, _| {
                    let found = fns::Ground_801C2090(ctx, handle(ctx, at + 4));
                    answer(if found != 0 { ctx.read_u32(at + 4) } else { 0 });
                },
                &|r, _| {
                    let c = ground::fog_color(r.game).expect("the stage's fog");
                    let ours = u32::from_be_bytes([c.r, c.g, c.b, c.a]);
                    assert_eq!(ours, answered(), "Ground_801C2090");
                },
            ),
            (
                "grLast_8021C40C",
                &|ctx, _| {
                    ctx.write_u32(at + 8, 0x0A14_1E28);
                    invoke(ctx, 0x8021_C40C, &[map_addr(ctx, 3), at + 8], &[45.0]);
                },
                &|r, _| {
                    let c = _GXColor {
                        r: 0x0A,
                        g: 0x14,
                        b: 0x1E,
                        a: 0x28,
                    };
                    last::blend_to(r.game, our_map(r, 3), c, 45.0)
                },
            ),
            (
                "grLast_8021C6AC",
                &|ctx, _| invoke(ctx, 0x8021_C6AC, &[], &[]),
                &|r, _| last::bright_colors(r.game),
            ),
            (
                "grLast_8021C640",
                &|ctx, _| invoke(ctx, 0x8021_C640, &[], &[]),
                &|r, _| last::dark_colors(r.game),
            ),
        ],
    );
}

fn destroy_theirs(map: u32) -> impl Fn(&Ctx, &Maps) {
    move |ctx: &Ctx, _: &Maps| fns::Ground_801C4A08(ctx, piece(ctx, map))
}

fn destroy_ours(map: u32) -> impl Fn(&mut Running, &Maps) {
    move |r: &mut Running, _: &Maps| {
        let g = our_map(r, map);
        setup::destroy(r, Some(g))
    }
}

/// Destroying each stage object in turn, with the closing scene's piece (map 9) created in
/// between, on a real stage: the destroy callbacks, collision switched off, points forgotten.
#[test]
fn ground_destroy_and_create() {
    let Some(path) = running_state() else { return };
    let t: Vec<_> = (0..10).map(destroy_theirs).collect();
    let o: Vec<_> = (0..10).map(destroy_ours).collect();
    on_state(
        &path,
        &[
            ("destroy map 4", &t[4], &o[4]),
            ("destroy map 5", &t[5], &o[5]),
            ("destroy map 6", &t[6], &o[6]),
            ("destroy map 7", &t[7], &o[7]),
            ("destroy map 8", &t[8], &o[8]),
            (
                "grLast_8021A7F4(9)",
                &|ctx, _| invoke(ctx, 0x8021_A7F4, &[9], &[]),
                &|r, _| {
                    last::create(r, 9, last::CYCLE_PIECES_FRAME);
                },
            ),
            ("destroy map 9", &t[9], &o[9]),
            ("destroy map 0", &t[0], &o[0]),
            ("destroy map 1", &t[1], &o[1]),
            ("destroy map 2", &t[2], &o[2]),
            ("destroy map 3", &t[3], &o[3]),
        ],
    );
}

/// The stage's queries against `stage_info` values spread over each field's useful range, in
/// both float modes.
#[test]
fn stage_queries() {
    use melee_sim::game::gr::stage;
    use melee_sim::game::state::Game;
    leaf_check::each_mode(2000, 500.0, |ctx, floats, mode| {
        let info = leaf_check::sym("stage_info");
        let mut game = Game::default();
        let vals: Vec<f32> = (0..15).map(|_| floats.next()).collect();
        let c = &mut game.stage_info.cam_info;
        c.cam_bounds.left = vals[0];
        c.cam_bounds.right = vals[1];
        c.cam_bounds.top = vals[2];
        c.cam_bounds.bottom = vals[3];
        c.cam_x_offset = vals[4];
        c.cam_y_offset = vals[5];
        c.cam_pan_degrees = vals[6];
        c.cam_angle_up = vals[7];
        c.cam_angle_down = vals[8];
        c.cam_angle_left = vals[9];
        c.cam_angle_right = vals[10];
        let b = &mut game.stage_info.blast_zone;
        b.left = vals[11];
        b.right = vals[12];
        b.top = vals[13];
        b.bottom = vals[14];
        let offsets = [
            0x0, 0x4, 0x8, 0xC, 0x10, 0x14, 0x1C, 0x4C, 0x50, 0x54, 0x58, 0x74, 0x78, 0x7C, 0x80,
        ];
        for (off, v) in offsets.iter().zip(&vals) {
            ctx.write_u32(info + off, v.to_bits());
        }
        let b = leaf_check::bits;
        let theirs = [
            b(fns::Stage_GetCamBoundsLeftOffset(ctx)),
            b(fns::Stage_GetCamBoundsRightOffset(ctx)),
            b(fns::Stage_GetCamBoundsTopOffset(ctx)),
            b(fns::Stage_GetCamBoundsBottomOffset(ctx)),
            b(fns::Stage_GetCamPanAngleRadians(ctx)),
            b(fns::Stage_GetBlastZoneLeftOffset(ctx)),
            b(fns::Stage_GetBlastZoneRightOffset(ctx)),
            b(fns::Stage_GetBlastZoneTopOffset(ctx)),
            b(fns::Stage_GetBlastZoneBottomOffset(ctx)),
            b(fns::Stage_CalcUnkCamY(ctx)),
            b(fns::Stage_CalcUnkCamYBounds(ctx)),
            b(fns::Stage_GetCamAngleRadiansUp(ctx)),
            b(fns::Stage_GetCamAngleRadiansDown(ctx)),
            b(fns::Stage_GetCamAngleRadiansLeft(ctx)),
            b(fns::Stage_GetCamAngleRadiansRight(ctx)),
        ];
        let a = stage::cam_angles(&game);
        let ours = [
            stage::cam_left(&game),
            stage::cam_right(&game),
            stage::cam_top(&game),
            stage::cam_bottom(&game),
            stage::cam_pan(&game),
            stage::blast_left(&game),
            stage::blast_right(&game),
            stage::blast_top(&game),
            stage::blast_bottom(&game),
            stage::below_camera(&game),
            stage::below_camera_bounds(&game),
            a[0],
            a[1],
            a[2],
            a[3],
        ]
        .map(f32::to_bits);
        assert_eq!(ours, theirs, "{mode:?}: {vals:?}");
        fns::Stage_UnkSetVec3TCam_Offset(ctx, handle(ctx, leaf_check::SCRATCH));
        assert_eq!(
            leaf_check::vec_bits(stage::cam_center(&game)),
            leaf_check::read_f32s(ctx, leaf_check::SCRATCH, 3),
            "{mode:?}"
        );
        for k in [0x3A, 0x3B, 0x3F, 0x42, 0x49, 0x4C, 0x4D, 32] {
            assert_eq!(
                stage::is_special_target_stage(k),
                fns::Stage_80224DC8(ctx, k) != 0
            );
        }
    });
}

/// The dynamic attribute areas through random additions and removals, against the
/// reference's lists.
#[test]
fn dynamic_attributes() {
    use melee_sim::game::gr::{DynamicAttrs, dynamic_attr};
    let Some(ctx) = reference() else { return };
    let areas = leaf_check::sym("grDynamicAttr_8049EFA0");
    let index = |p: u32| (p != 0).then(|| ((p - areas) / 0x24) as u8);
    let mut rand = leaf_check::Rand(0x5EED);
    for round in 0..50 {
        fns::grDynamicAttr_801CA0B4(&ctx);
        let mut ours = DynamicAttrs::default();
        dynamic_attr::reset(&mut ours);
        let mut live: Vec<u8> = Vec::new();
        for step in 0..60 {
            if rand.below(3) < 2 {
                let pos = melee_sim::vec::Vec3::new(
                    rand.range(-100.0, 100.0),
                    rand.range(-100.0, 100.0),
                    0.0,
                );
                leaf_check::put_vec(&ctx, leaf_check::SCRATCH, pos);
                let attr = rand.below(8) as i32;
                let floor = rand.below(30) as i32;
                let timer = rand.below(5) as i32 - 1;
                let radius = rand.range(1.0, 30.0);
                let theirs = fns::grDynamicAttr_801CA0F8(
                    &ctx,
                    attr,
                    handle(&ctx, leaf_check::SCRATCH),
                    floor,
                    leaf_check::arg(radius),
                    timer,
                );
                let got = dynamic_attr::add(&mut ours, attr, pos, floor, radius, timer);
                let theirs = index(ssbm_rt::Handle::addr(theirs));
                assert_eq!(got, theirs, "round {round} step {step}");
                live.extend(got);
            } else if !live.is_empty() {
                let i = live.remove(rand.below(live.len() as u64) as usize);
                fns::grDynamicAttr_801CA1C0(&ctx, handle(&ctx, areas + 0x24 * u32::from(i)));
                dynamic_attr::remove(&mut ours, i);
            }
            let active = index(ctx.read_u32(leaf_check::sym("grDynamicAttr_804D6960")));
            let free = index(ctx.read_u32(leaf_check::sym("grDynamicAttr_804D6964")));
            assert_eq!(
                (ours.active, ours.free),
                (active, free),
                "round {round} step {step}"
            );
            for (k, a) in ours.areas.iter().enumerate() {
                let at = areas + 0x24 * k as u32;
                let next = index(ctx.read_u32(at));
                assert_eq!(a.next, next, "round {round} step {step} area {k}");
                if live.contains(&(k as u8)) {
                    let theirs = (
                        ctx.read_u32(at + 4) as i32,
                        leaf_check::read_f32s(&ctx, at + 8, 3),
                        ctx.read_u32(at + 0x14) as i32,
                        ctx.read_u32(at + 0x18),
                        ctx.read_u32(at + 0x1C) as i32,
                    );
                    let ours = (
                        a.attr,
                        leaf_check::vec_bits(a.pos),
                        a.floor,
                        a.radius.to_bits(),
                        a.timer,
                    );
                    assert_eq!(ours, theirs, "round {round} step {step} area {k}");
                }
            }
        }
    }
}

/// Marking a model's materials set apart (`lb_8000F9F8`), on each stage object's model, and
/// the entry points and stage kind queries, on a real stage.
#[test]
fn ground_set_apart_and_points() {
    let Some(path) = running_state() else { return };
    let model = |ctx: &Ctx, map: u32| {
        let gobj = map_addr(ctx, map);
        let scale = ctx.read_u32(gobj + 0x28);
        ctx.read_u32(scale + 0x10)
    };
    let apart =
        |map: u32| move |ctx: &Ctx, _: &Maps| fns::lb_8000F9F8(ctx, handle(ctx, model(ctx, map)));
    let ours = |map: u32| {
        move |r: &mut Running, _: &Maps| {
            let root = melee_sim::game::gr::ground::jobj_of(r.game, our_map(r, map)).unwrap();
            let m = r.game.objs.jobjs[root].child.unwrap();
            setup::set_apart(r.game, m)
        }
    };
    let (t3, o3, t4, o4) = (apart(3), ours(3), apart(4), ours(4));
    let entry = |slot: i32| {
        move |ctx: &Ctx, _: &Maps| {
            leaf_check::put_vec(
                ctx,
                leaf_check::SCRATCH,
                melee_sim::vec::Vec3::new(-1.0, -2.0, -3.0),
            );
            fns::Stage_80224E38(ctx, handle(ctx, leaf_check::SCRATCH), slot);
            let v = leaf_check::read_f32s(ctx, leaf_check::SCRATCH, 3);
            ENTRY.with(|e| e.set([v[0], v[1], v[2]]));
        }
    };
    let entry_ours = |slot: i32| {
        move |r: &mut Running, _: &Maps| {
            let p = melee_sim::game::gr::stage::entry_point(r.game, r.data, slot)
                .unwrap_or(melee_sim::vec::Vec3::new(-1.0, -2.0, -3.0));
            let ours = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
            assert_eq!(ours, ENTRY.with(|e| e.get()), "Stage_80224E38({slot})");
        }
    };
    let te: Vec<_> = (0..6).map(entry).collect();
    let oe: Vec<_> = (0..6).map(entry_ours).collect();
    on_state(
        &path,
        &[
            ("lb_8000F9F8 on map 3", &t3, &o3),
            ("lb_8000F9F8 on map 4", &t4, &o4),
            ("Stage_80224E38(0)", &te[0], &oe[0]),
            ("Stage_80224E38(1)", &te[1], &oe[1]),
            ("Stage_80224E38(2)", &te[2], &oe[2]),
            ("Stage_80224E38(3)", &te[3], &oe[3]),
            ("Stage_80224E38(4)", &te[4], &oe[4]),
            ("Stage_80224E38(5)", &te[5], &oe[5]),
            (
                "Stage_80225194",
                &|ctx, _| answer(fns::Stage_80225194(ctx) as u32),
                &|r, _| {
                    let ours = melee_sim::game::gr::stage::stkind(r.game);
                    assert_eq!(ours as u32, answered(), "Stage_80225194");
                },
            ),
        ],
    );
}

thread_local! {
    static ENTRY: std::cell::Cell<[u32; 3]> = const { std::cell::Cell::new([0; 3]) };
}

use melee_sim::game::gr::battle;

/// Battlefield's setup: its backgrounds' animations spawn particles from the stage's bank.
#[test]
fn battle_on_init() {
    check("grBattle_OnInit", &|r, _, _| battle::on_init(r));
}

#[test]
fn battle_on_load() {
    check("grBattle_OnLoad", &|r, _, _| battle::on_load(r));
}

#[test]
fn battle_on_start() {
    check("grBattle_OnStart", &|r, _, _| battle::on_start(r));
}

#[test]
fn battle_create() {
    check("grBattle_80219D84", &|r, cap, _| {
        battle::create(r, int_arg(cap, 0) as i32, setup::SETUP_FRAME);
    });
}

#[test]
fn battle_init_main() {
    check_proc("grBattle_GObj0_Callback0", battle::init_main);
}

#[test]
fn battle_init_floor() {
    check_proc("grBattle_GObj6_Callback0", battle::init_floor);
}

#[test]
fn battle_proc_floor() {
    check_proc("grBattle_GObj6_Callback2", battle::proc_floor);
}

#[test]
fn battle_init_background() {
    check_proc("grBattle_GObj1_Callback0", battle::init_background);
    check_proc("grBattle_GObj2_Callback0", battle::init_background);
}

#[test]
fn battle_empty_procs() {
    for f in [
        "grBattle_GObj0_Callback2",
        "grBattle_GObj1_Callback2",
        "grBattle_GObj2_Callback2",
    ] {
        check_proc(f, battle::empty);
    }
}

#[test]
fn battle_init_sky() {
    check_proc("grBattle_BG_Callback0", battle::init_sky);
}

#[test]
fn battle_proc_sky() {
    check_proc("grBattle_BG_Callback2", battle::proc_sky);
}

#[test]
fn zako_proc() {
    check_proc("fn_801CADBC", melee_sim::game::gr::zako::proc);
}

/// Battlefield's stage processes from the first capture of its sky, compared every 10 ticks,
/// with the particle system's processes running the generators its backgrounds' animations
/// spawn.
#[test]
fn battle_stage_ticks() {
    let Some(first) = captures("grBattle_BG_Callback2").into_iter().next() else {
        eprintln!("no captures of grBattle_BG_Callback2: skipping");
        return;
    };
    let every = std::env::var("STAGE_EVERY").map_or(10, |v| v.parse().unwrap());
    stage_ticks(&first, 9000, every);
}

/// The stages' own color scripts (Final Destination's four and Battlefield's two) started on a
/// real stage's object and run to their ends on both sides.
#[test]
fn stage_color_scripts() {
    use melee_sim::game::gr::material;
    for (path, n) in [
        (running_state(), 4u32),
        (captures("grBattle_BG_Callback2").into_iter().next(), 2),
    ] {
        let Some(path) = path else { continue };
        for script in 0..n {
            let start_theirs = move |ctx: &Ctx, _: &Maps| {
                let yaku = ctx.read_u32(leaf_check::sym("stage_info") + 0x6C0);
                let s = ctx.read_u32(yaku + 4 * script);
                fns::grMaterial_801C9604(ctx, piece(ctx, 0), handle(ctx, s), 0);
            };
            let start_ours = move |r: &mut Running, _: &Maps| {
                let yaku = r.game.stage_info.yakumono_param.expect("yakumono_param");
                let s = r.data.ptr(yaku.at(4 * script));
                material::start_script(r.game, r.data, our_map(r, 0), s, 0);
            };
            let tick_theirs = |ctx: &Ctx, _: &Maps| fns::grMaterial_801C9698(ctx, piece(ctx, 0));
            let tick_ours = |r: &mut Running, _: &Maps| {
                material::run_ground_overlay(r.game, r.data, our_map(r, 0))
            };
            let mut steps: Vec<Step> = vec![("grMaterial_801C9604", &start_theirs, &start_ours)];
            for _ in 0..260 {
                steps.push(("grMaterial_801C9698", &tick_theirs, &tick_ours));
            }
            on_state(&path, &steps);
        }
    }
}

/// A capture importer for synthetic data: data pointers into one archive loaded at `base`.
struct Synthetic {
    archive: melee_data::archive::ArchiveId,
    base: u32,
    len: u32,
}

impl melee_sim::game::mirror::Cx for Synthetic {
    fn import_handle(&mut self, _: &'static str, addr: u32) -> Option<u32> {
        assert_eq!(addr, 0, "a handle in synthetic data");
        None
    }

    fn import_data(&mut self, addr: u32) -> Option<melee_data::archive::DataRef> {
        if addr == 0 {
            return None;
        }
        assert!(
            (self.base..self.base + self.len).contains(&addr),
            "{addr:#x}"
        );
        Some(melee_data::archive::DataRef {
            archive: self.archive,
            offset: addr - self.base,
        })
    }

    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {}

    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}

    fn mismatch(&mut self, _: &str, _: String, _: String) {}
}

/// Random color scripts of every command a color overlay runs (the overlay's own eleven, the
/// reset and the jump) on both sides, the whole overlay compared after each tick, in both
/// float modes.
#[test]
fn color_overlay_scripts() {
    use melee_sim::game::gr::material;
    use melee_sim::game::mirror::{Mem, Mirror};
    use melee_sim::game::structs::ColorOverlay;
    let Some(ctx) = reference() else { return };
    let mut rand = leaf_check::Rand(0xC010_0E5C);
    for mode in MODES {
        set_fp_mode(mode);
        for round in 0..300 {
            // A script: commands, then an end (or a reset), with jumps forward.
            let mut words: Vec<u32> = Vec::new();
            let mut relocs = Vec::new();
            let mut jumps = Vec::new();
            let mut starts = Vec::new();
            for _ in 0..rand.below(12) + 1 {
                starts.push(words.len() as u32);
                let op = [11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 7][rand.below(11) as usize];
                let color = rand.bits() as u32;
                let timer = (rand.below(6) + 1) as u32;
                match op {
                    11 => words.push(op << 26 | (rand.below(4) as u32)),
                    13 | 16 => {
                        words.push(op << 26 | (rand.bits() as u32 & 0x03FF_FFFF));
                        if op == 13 {
                            words.push(color);
                        }
                    }
                    14 | 18 => words.extend([op << 26, color]),
                    15 | 19 => words.extend([op << 26 | timer, color]),
                    7 => {
                        words.push(op << 26);
                        jumps.push(words.len());
                        words.push(0);
                    }
                    _ => words.push(op << 26),
                }
            }
            words.push(if rand.below(4) == 0 { 0 } else { 10 << 26 });
            let end = words.len() as u32 - 1;
            starts.push(end);
            for &j in &jumps {
                // A command after the jump.
                let later: Vec<u32> = starts.iter().copied().filter(|&s| s > j as u32).collect();
                words[j] = 4 * later[rand.below(later.len() as u64) as usize];
                relocs.push(4 * j as u32);
            }
            let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
            let file = leaf_check::dat(&bytes, &relocs);
            let base = leaf_check::load_archive(&ctx, &file);
            let mut archives = melee_data::archive::Archives::default();
            let id = archives.add(melee_data::archive::Archive::parse(&file).unwrap());
            let mut cx = Synthetic {
                archive: id,
                base,
                len: bytes.len() as u32,
            };
            let at = leaf_check::SCRATCH;
            for i in 0..0x80 / 4 {
                ctx.write_u32(at + 4 * i, 0);
            }
            ctx.write_u32(at + 4, rand.below(5) as u32);
            ctx.write_u32(at + 8, base);
            let mem = |ctx: &Ctx| {
                let mut b = vec![0u8; 0x80];
                let _ = ctx.mem.read_bytes(at, &mut b);
                b
            };
            let bytes0 = mem(&ctx);
            let mut ours = ColorOverlay::import(
                &Mem {
                    bytes: &bytes0,
                    base: at,
                },
                at,
                &mut cx,
            );
            for step in 0..40 {
                let theirs_end =
                    fns::lb_80014258(&ctx, handle(&ctx, 0), handle(&ctx, at), handle(&ctx, 0)) != 0;
                let ours_end = material::run_overlay(&archives, &mut ours);
                let b = mem(&ctx);
                let theirs = ColorOverlay::import(
                    &Mem {
                        bytes: &b,
                        base: at,
                    },
                    at,
                    &mut cx,
                );
                assert_eq!(
                    format!("{ours:?}"),
                    format!("{theirs:?}"),
                    "{mode:?} round {round} step {step}: {words:08x?}"
                );
                assert_eq!(ours_end, theirs_end, "{mode:?} round {round} step {step}");
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

/// The players' start points and Battlefield's demo switch, on a real stage.
#[test]
fn start_points_and_demo() {
    use melee_sim::game::gr::grlib;
    let Some(path) = running_state() else { return };
    let get = |i: i32| {
        move |ctx: &Ctx, _: &Maps| {
            fns::grLib_801C9A70(ctx, i, handle(ctx, leaf_check::SCRATCH));
            let v = leaf_check::read_f32s(ctx, leaf_check::SCRATCH, 3);
            ENTRY.with(|e| e.set([v[0], v[1], v[2]]));
        }
    };
    let get_ours = |i: i32| {
        move |r: &mut Running, _: &Maps| {
            let p = grlib::start_point(r.game, i);
            let ours = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
            assert_eq!(ours, ENTRY.with(|e| e.get()), "grLib_801C9A70({i})");
        }
    };
    let g: Vec<_> = (0..4).map(get).collect();
    let o: Vec<_> = (0..4).map(get_ours).collect();
    on_state(
        &path,
        &[
            (
                "grLib_801C9A10",
                &|ctx, _| {
                    fns::grLib_801C9A10(ctx);
                },
                &|r, _| grlib::measure_start_points(r.game, r.data),
            ),
            ("grLib_801C9A70(0)", &g[0], &o[0]),
            ("grLib_801C9A70(1)", &g[1], &o[1]),
            ("grLib_801C9A70(2)", &g[2], &o[2]),
            ("grLib_801C9A70(3)", &g[3], &o[3]),
            (
                "grBattle_OnDemoInit",
                &|ctx, _| invoke(ctx, 0x8021_9C98, &[0], &[]),
                &|r, _| battle::on_demo_init(r.game),
            ),
        ],
    );
}

/// Battlefield's objects destroyed one by one on a real stage: their destroy callbacks. (Its
/// plain background, map 5, spawns particles when created, like the others.)
#[test]
fn battle_destroy() {
    let Some(path) = captures("grBattle_BG_Callback2").into_iter().next() else {
        return;
    };
    let t: Vec<_> = (0..7).map(destroy_theirs).collect();
    let o: Vec<_> = (0..7).map(destroy_ours).collect();
    on_state(
        &path,
        &[
            ("destroy map 1", &t[1], &o[1]),
            ("destroy map 3", &t[3], &o[3]),
            ("destroy map 6", &t[6], &o[6]),
            ("destroy map 0", &t[0], &o[0]),
        ],
    );
}

/// Battlefield's empty processes and destroy callbacks (including those of the objects play never
/// creates), then a demo fight's setup and the main object set up again in it, on a real stage.
#[test]
fn battle_empty_callbacks_and_demo() {
    let Some(path) = captures("grBattle_BG_Callback2").into_iter().next() else {
        return;
    };
    let call = |name: &'static str| {
        move |ctx: &Ctx, _: &Maps| invoke(ctx, leaf_check::sym(name), &[map_addr(ctx, 0)], &[])
    };
    let names = [
        "grBattle_GObj0_Callback3",
        "grBattle_GObj1_Callback3",
        "grBattle_GObj2_Callback3",
        "grBattle_GObj4_Callback2",
        "grBattle_GObj4_Callback3",
        "grBattle_GObj5_Callback2",
        "grBattle_GObj5_Callback3",
        "grBattle_GObj6_Callback3",
        "grBattle_BG_Callback3",
    ];
    let theirs: Vec<_> = names.iter().map(|&n| call(n)).collect();
    let ours = |r: &mut Running, _: &Maps| battle::empty(r, our_map(r, 0));
    let mut steps: Vec<Step> = names
        .iter()
        .zip(&theirs)
        .map(|(&n, t)| {
            (
                n,
                t as &dyn Fn(&Ctx, &Maps),
                &ours as &dyn Fn(&mut Running, &Maps),
            )
        })
        .collect();
    let demo_theirs =
        |ctx: &Ctx, _: &Maps| invoke(ctx, leaf_check::sym("grBattle_OnDemoInit"), &[], &[]);
    let demo_ours = |r: &mut Running, _: &Maps| battle::on_demo_init(r.game);
    let main_theirs = call("grBattle_GObj0_Callback0");
    let main_ours = |r: &mut Running, _: &Maps| battle::init_main(r, our_map(r, 0));
    steps.push(("grBattle_OnDemoInit", &demo_theirs, &demo_ours));
    steps.push(("grBattle_GObj0_Callback0", &main_theirs, &main_ours));
    on_state(&path, &steps);
}

/// Pokémon Stadium's entry points no full-match diff reaches (Ready outside sudden death, GAME!,
/// whether the screen is zoomed on a player, the demo's init), from a capture of its
/// transformation cycle (`local/calls/stage-more/ps1`).
#[test]
fn stadium_unreached_entries() {
    use melee_sim::game::gr::stadium;
    let dir = std::env::var("STAGE_MORE_CALLS").unwrap_or_else(|_| ssbm_rs_local(MORE_CALLS));
    let path = PathBuf::from(dir).join("ps1/grStadium_801D4548-call10.call");
    if !melee_capture::pack::exists(&path) {
        eprintln!("no capture {}: skipping", path.display());
        return;
    }
    let zoomed = |slot: i32| {
        (
            move |ctx: &Ctx, _: &Maps| answer(fns::grStadium_801D4FF8(ctx, slot) as u32),
            move |r: &mut Running, _: &Maps| {
                assert_eq!(
                    u32::from(stadium::zoomed_on(r.game, slot)),
                    answered(),
                    "grStadium_801D4FF8({slot})"
                );
            },
        )
    };
    let z: Vec<_> = (0..6).map(zoomed).collect();
    let mut steps: Vec<Step> = z
        .iter()
        .map(|(t, o)| {
            (
                "grStadium_801D4FF8",
                t as &dyn Fn(&Ctx, &Maps),
                o as &dyn Fn(&mut Running, &Maps),
            )
        })
        .collect();
    let demo_theirs = |ctx: &Ctx, _: &Maps| fns::grStadium_OnDemoInit(ctx, 0);
    let demo_ours = |r: &mut Running, _: &Maps| stadium::on_demo_init(r);
    let ready_theirs = |ctx: &Ctx, _: &Maps| fns::grStadium_801D410C(ctx);
    let ready_ours = |r: &mut Running, _: &Maps| r.stadium_ready(false);
    let game_theirs = |ctx: &Ctx, _: &Maps| fns::grStadium_801D40C8(ctx);
    let game_ours = |r: &mut Running, _: &Maps| r.stadium_game();
    steps.push(("grStadium_OnDemoInit", &demo_theirs, &demo_ours));
    steps.push(("grStadium_801D410C", &ready_theirs, &ready_ours));
    steps.push(("grStadium_801D40C8", &game_theirs, &game_ours));
    // After GAME! the screen shows text, so ask again whether it's zoomed.
    for (t, o) in &z {
        steps.push(("grStadium_801D4FF8", t, o));
    }
    on_state_with(&path, &steps, true);
    // The cycle's other captures, the screen in whatever mode each found it.
    let zsteps: Vec<Step> = steps[..z.len()].to_vec();
    let mut others: Vec<_> = melee_capture::pack::entries(path.parent().unwrap())
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("grStadium_801D4548-"))
        })
        .collect();
    others.sort();
    for p in others {
        on_state_with(&p, &zsteps, true);
    }
}

// The shared stage code's settings, queries and helpers (milestone 4's task 4), and the
// tournament stages' callbacks no match reaches.

/// The first saved call of Yoshi's Island N64's clouds' process in its oracle match's set
/// (`leaf-check stages` has the recipe): a running stage with a fog, lights and three
/// stage objects with variables.
fn yi64_state() -> Option<PathBuf> {
    let base = std::env::var("STAGES_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/stages"));
    let dir = format!("{base}/oracle-stage-yoshis-island-64");
    let mut v: Vec<PathBuf> = melee_capture::pack::call_files(&dir)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("grOldYoshi_8020EC10-call"))
        })
        .collect();
    v.sort();
    if v.is_empty() {
        eprintln!("no captures in {dir}: skipping");
    }
    v.into_iter().nth(3)
}

/// The address of map `id`'s stage object's `Ground` in the captured state.
fn ground_addr(ctx: &Ctx, id: u32) -> u32 {
    ctx.read_u32(map_addr(ctx, id) + 0x2C)
}

/// A step that only asks: the reference's answer (r3, or f1's bits) against ours.
fn ask<'a>(
    name: &'a str,
    theirs: impl Fn(&Ctx) -> u32 + 'a,
    ours: impl Fn(&mut Running, &Maps) -> u32 + 'a,
) -> OwnedStep<'a> {
    (
        name,
        Box::new(move |ctx: &Ctx, _: &Maps| answer(theirs(ctx))),
        Box::new(move |r: &mut Running, m: &Maps| {
            assert_eq!(ours(r, m), answered(), "{name}");
        }),
    )
}

type OwnedStep<'a> = (
    &'a str,
    Box<dyn Fn(&Ctx, &Maps) + 'a>,
    Box<dyn Fn(&mut Running, &Maps) + 'a>,
);

fn run_owned(path: &std::path::Path, steps: &[OwnedStep], audio: bool) {
    let borrowed: Vec<Step> = steps
        .iter()
        .map(|(n, t, o)| {
            (
                *n,
                &**t as &dyn Fn(&Ctx, &Maps),
                &**o as &dyn Fn(&mut Running, &Maps),
            )
        })
        .collect();
    on_state_opts(path, &borrowed, false, audio);
}

/// A stage parameters' color getter: its name, the reference's (the color's address) and which
/// color it is.
type ColorGetter = (
    &'static str,
    fn(&Ctx) -> u32,
    melee_sim::game::gr::ground::ParamColor,
);

/// `stage_info`'s settings and queries, the stage parameters' colors, the camera's and the
/// fixed camera's getters, and the enemy spawner's switches, each run on both sides of a real
/// stage (Yoshi's Island N64 mid-match) in turn, answers and whole states compared.
#[test]
fn ground_settings_and_queries() {
    use melee_sim::game::gr::ground::{self as g, ParamColor};
    use melee_sim::game::gr::{grlib, stage, zako};
    let Some(path) = yi64_state() else { return };
    let at = leaf_check::SCRATCH;
    let f = leaf_check::arg;
    let b = leaf_check::bits;
    let gobj_of = |m: &Maps, a: u32| (a != 0).then(|| m.gobjs[&a]);
    let colors: [ColorGetter; 9] = [
        (
            "Ground_801C0604",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0604(c)),
            ParamColor::B8,
        ),
        (
            "Ground_801C0618",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0618(c)),
            ParamColor::C0,
        ),
        (
            "Ground_801C062C",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C062C(c)),
            ParamColor::D0,
        ),
        (
            "Ground_801C0640",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0640(c)),
            ParamColor::D8,
        ),
        (
            "Ground_801C0654",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0654(c)),
            ParamColor::C4,
        ),
        (
            "Ground_801C0668",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0668(c)),
            ParamColor::CC,
        ),
        (
            "Ground_801C067C",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C067C(c)),
            ParamColor::D4,
        ),
        (
            "Ground_801C0690",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C0690(c)),
            ParamColor::BC,
        ),
        (
            "Ground_801C06A4",
            |c| ssbm_rt::Handle::addr(fns::Ground_801C06A4(c)),
            ParamColor::C8,
        ),
    ];
    let mut steps: Vec<OwnedStep> = Vec::new();
    for (name, theirs, which) in colors {
        steps.push((
            name,
            Box::new(move |ctx: &Ctx, _: &Maps| answer(ctx.read_u32(theirs(ctx)))),
            Box::new(move |r: &mut Running, _: &Maps| {
                let c = g::param_color(r.game, which);
                assert_eq!(
                    u32::from_be_bytes([c.r, c.g, c.b, c.a]),
                    answered(),
                    "{name}"
                );
            }),
        ));
    }
    macro_rules! q {
        ($name:literal, $theirs:expr, $ours:expr) => {
            steps.push(ask($name, $theirs, $ours));
        };
    }
    macro_rules! set {
        ($name:literal, $theirs:expr, $ours:expr) => {
            steps.push(($name, Box::new($theirs), Box::new($ours)));
        };
    }
    q!(
        "Ground_801C0508",
        |c: &Ctx| fns::Ground_801C0508(c) as u32,
        |r: &mut Running, _: &Maps| g::param_x4(r.game) as u32
    );
    set!(
        "Ground_SetParamY",
        |c: &Ctx, _: &Maps| fns::Ground_SetParamY(c, f(1.25)),
        |r: &mut Running, _: &Maps| g::set_scale(r.game, 1.25)
    );
    set!(
        "Ground_801C1D6C",
        |c: &Ctx, _: &Maps| fns::Ground_801C1D6C(c, 0x120),
        |r: &mut Running, _: &Maps| g::set_flags(r.game, 0x120)
    );
    q!(
        "Ground_801C1D84",
        |c: &Ctx| fns::Ground_801C1D84(c),
        |r: &mut Running, _: &Maps| g::flags(r.game, 0x330)
    );
    q!(
        "Ground_801C1D98",
        |c: &Ctx| fns::Ground_801C1D98(c),
        |r: &mut Running, _: &Maps| g::flags(r.game, 0x20)
    );
    q!(
        "Ground_801C1DAC",
        |c: &Ctx| fns::Ground_801C1DAC(c),
        |r: &mut Running, _: &Maps| g::flags(r.game, 0x100)
    );
    q!(
        "Ground_801C1DC0",
        |c: &Ctx| fns::Ground_801C1DC0(c),
        |r: &mut Running, _: &Maps| g::flags(r.game, 0x80)
    );
    q!(
        "Ground_801C1DD4",
        |c: &Ctx| fns::Ground_801C1DD4(c) as u32,
        |r: &mut Running, _: &Maps| g::x6d0(r.game) as u32
    );
    q!(
        "Ground_801C1E18",
        |c: &Ctx| fns::Ground_801C1E18(c) as u32,
        |r: &mut Running, _: &Maps| u32::from(g::fog_enabled(r.game))
    );
    q!(
        "Ground_801C1E84",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_801C1E84(c)),
        |r: &mut Running, m: &Maps| {
            let ours = g::fog_gobj(r.game);
            assert_eq!(ours, gobj_of(m, answered()), "Ground_801C1E84");
            answered()
        }
    );
    q!(
        "Ground_ApplyStageBackgroundColor",
        |c: &Ctx| {
            fns::Ground_ApplyStageBackgroundColor(c);
            c.read_u32(leaf_check::sym("game_camera") + 8) >> 8
        },
        |r: &mut Running, _: &Maps| {
            g::apply_background_color(r.game);
            let b = &r.game.camera.cam.background_color;
            u32::from_be_bytes([0, b.r, b.g, b.b])
        }
    );
    q!(
        "Ground_801C20D0",
        |c: &Ctx| b(fns::Ground_801C20D0(c)),
        |r: &mut Running, _: &Maps| g::cam_vertical_tilt(r.game).to_bits()
    );
    q!(
        "Ground_801C247C",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_801C247C(c, 4, 0)),
        |r: &mut Running, _: &Maps| {
            let ours = g::spline(r.game, r.data, 4, 0);
            assert_eq!(ours.is_some(), answered() != 0, "Ground_801C247C");
            answered()
        }
    );
    q!(
        "Ground_801C2CF4",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_801C2CF4(c, 0)),
        |r: &mut Running, m: &Maps| {
            let ours = g::point_joint(r.game, 0);
            let theirs = (answered() != 0).then(|| m.jobjs[&answered()]);
            assert_eq!(ours, theirs, "Ground_801C2CF4");
            answered()
        }
    );
    set!(
        "Ground_801C2D0C",
        |c: &Ctx, _: &Maps| {
            let j = c.read_u32(leaf_check::sym("stage_info") + 0x280 + 4);
            fns::Ground_801C2D0C(c, 7, handle(c, j));
        },
        |r: &mut Running, _: &Maps| {
            let j = g::point_joint(r.game, 1);
            g::set_point_joint(r.game, 7, j);
        }
    );
    set!(
        "Ground_801C3880",
        |c: &Ctx, _: &Maps| fns::Ground_801C3880(c, f(77.5)),
        |r: &mut Running, _: &Maps| g::set_cam_top(r.game, 77.5)
    );
    set!(
        "Ground_801C3890",
        |c: &Ctx, _: &Maps| fns::Ground_801C3890(c, f(-31.0)),
        |r: &mut Running, _: &Maps| g::set_cam_bottom(r.game, -31.0)
    );
    set!(
        "Ground_801C38A0",
        |c: &Ctx, _: &Maps| fns::Ground_801C38A0(c, f(-120.25)),
        |r: &mut Running, _: &Maps| g::set_cam_left(r.game, -120.25)
    );
    set!(
        "Ground_801C38AC",
        |c: &Ctx, _: &Maps| fns::Ground_801C38AC(c, f(130.0)),
        |r: &mut Running, _: &Maps| g::set_cam_right(r.game, 130.0)
    );
    set!(
        "Ground_801C38BC",
        |c: &Ctx, _: &Maps| fns::Ground_801C38BC(c, f(3.5), f(-2.0)),
        |r: &mut Running, _: &Maps| g::set_cam_offset(r.game, 3.5, -2.0)
    );
    set!(
        "Ground_801C3980",
        |c: &Ctx, _: &Maps| fns::Ground_801C3980(c, f(250.0)),
        |r: &mut Running, _: &Maps| g::set_blast_top(r.game, 250.0)
    );
    set!(
        "Ground_801C3990",
        |c: &Ctx, _: &Maps| fns::Ground_801C3990(c, f(-140.0)),
        |r: &mut Running, _: &Maps| g::set_blast_bottom(r.game, -140.0)
    );
    set!(
        "Ground_801C39A0",
        |c: &Ctx, _: &Maps| fns::Ground_801C39A0(c, f(-230.0)),
        |r: &mut Running, _: &Maps| g::set_blast_left(r.game, -230.0)
    );
    set!(
        "Ground_801C39B0",
        |c: &Ctx, _: &Maps| fns::Ground_801C39B0(c, f(240.0)),
        |r: &mut Running, _: &Maps| g::set_blast_right(r.game, 240.0)
    );
    q!(
        "Ground_801C3D44",
        |c: &Ctx| fns::Ground_801C3D44(c, handle(c, 0), f(31.0), f(17.0)) as u32,
        |r: &mut Running, _: &Maps| g::watch_points(r.game, None, 31.0, 17.0) as u32
    );
    q!(
        "Ground_801C3DB4",
        |c: &Ctx| fns::Ground_801C3DB4(c, handle(c, 0), f(31.0), f(17.0)) as u32,
        |r: &mut Running, _: &Maps| g::watch_points2(r.game, None, 31.0, 17.0) as u32
    );
    q!(
        "Ground_801C3F20",
        |c: &Ctx| b(fns::Ground_801C3F20(
            c,
            handle(c, c.read_u32(map_addr(c, 2) + 0x28))
        )),
        |r: &mut Running, _: &Maps| {
            let j = g::jobj_of(r.game, our_map(r, 2));
            g::anim_frame(r.game, j).to_bits()
        }
    );
    set!(
        "Ground_801C4338",
        |c: &Ctx, _: &Maps| fns::Ground_801C4338(c),
        |r: &mut Running, _: &Maps| g::target_broken(r.game)
    );
    set!(
        "Ground_801C438C",
        |c: &Ctx, _: &Maps| fns::Ground_801C438C(c, f(12.5)),
        |r: &mut Running, _: &Maps| g::set_x724(r.game, 12.5)
    );
    q!(
        "Ground_801C4368",
        |c: &Ctx| {
            fns::Ground_801C4368(c, handle(c, at), handle(c, at + 4));
            c.read_u32(at) ^ c.read_u32(at + 4).rotate_left(16)
        },
        |r: &mut Running, _: &Maps| {
            let (s, i) = g::x724_and_step(r.game);
            s.to_bits() ^ i.to_bits().rotate_left(16)
        }
    );
    q!(
        "Ground_801C498C",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_801C498C(c)),
        |r: &mut Running, m: &Maps| {
            assert_eq!(
                g::light_gobj(r.game),
                gobj_of(m, answered()),
                "Ground_801C498C"
            );
            answered()
        }
    );
    set!(
        "Ground_801C4D70",
        |c: &Ctx, _: &Maps| {
            leaf_check::put_vec(c, at, melee_sim::vec::Vec3::new(1.5, -2.0, 3.25));
            fns::Ground_801C4D70(c, piece(c, 2), handle(c, at), f(0.75));
        },
        |r: &mut Running, _: &Maps| {
            let gobj = Some(our_map(r, 2));
            g::set_barrel(
                r.game,
                gobj,
                melee_sim::vec::Vec3::new(1.5, -2.0, 3.25),
                0.75,
            );
        }
    );
    q!(
        "Ground_801C4DA0",
        |c: &Ctx| {
            fns::Ground_801C4DA0(c, handle(c, at + 0x20), handle(c, at + 0x2C));
            leaf_check::read_f32s(c, at + 0x20, 4)
                .iter()
                .fold(0, |h, w| h.rotate_left(7) ^ w)
        },
        |r: &mut Running, _: &Maps| {
            let (v, s) = g::barrel(r.game);
            [v.x, v.y, v.z, s]
                .iter()
                .fold(0, |h, w| h.rotate_left(7) ^ w.to_bits())
        }
    );
    set!(
        "Ground_801C4E70",
        |c: &Ctx, _: &Maps| {
            let j = |i: u32| handle(c, c.read_u32(leaf_check::sym("stage_info") + 0x280 + 4 * i));
            fns::Ground_801C4E70(c, j(0), j(1), j(2), j(3), j(4), j(5));
        },
        |r: &mut Running, _: &Maps| {
            let j = std::array::from_fn(|i| g::point_joint(r.game, i as i32));
            g::set_guide_points(r.game, r.data, j);
        }
    );
    set!(
        "Ground_801C5740",
        |c: &Ctx, _: &Maps| fns::Ground_801C5740(c, 9),
        |r: &mut Running, _: &Maps| g::set_x6d8(r.game, 9)
    );
    q!(
        "Ground_801C5764",
        |c: &Ctx| fns::Ground_801C5764(c) as u32,
        |r: &mut Running, _: &Maps| g::x6d8(r.game) as u32
    );
    set!(
        "Ground_801C5750",
        |c: &Ctx, _: &Maps| fns::Ground_801C5750(c),
        |r: &mut Running, _: &Maps| g::clear_x6d8(r.game)
    );
    q!(
        "Ground_801C5774",
        |c: &Ctx| fns::Ground_801C5774(c) as u32,
        |r: &mut Running, _: &Maps| g::x6dc(r.game) as u32
    );
    set!(
        "Ground_801C5784",
        |c: &Ctx, _: &Maps| fns::Ground_801C5784(c, 3),
        |r: &mut Running, _: &Maps| g::set_x740(r.game, 3)
    );
    q!(
        "Ground_801C5794",
        |c: &Ctx| fns::Ground_801C5794(c) as u32,
        |r: &mut Running, _: &Maps| g::x740(r.game) as u32
    );
    q!(
        "Ground_GetP1Fighter",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_GetP1Fighter(c)),
        |r: &mut Running, m: &Maps| {
            assert_eq!(
                g::p1_fighter(r.game),
                gobj_of(m, answered()),
                "Ground_GetP1Fighter"
            );
            answered()
        }
    );
    q!(
        "Ground_GetP1Fighter2",
        |c: &Ctx| ssbm_rt::Handle::addr(fns::Ground_GetP1Fighter2(c)),
        |r: &mut Running, m: &Maps| {
            assert_eq!(
                g::p1_fighter2(r.game),
                gobj_of(m, answered()),
                "Ground_GetP1Fighter2"
            );
            answered()
        }
    );
    q!(
        "Ground_801C57F0",
        |c: &Ctx| b(fns::Ground_801C57F0(c, 2)),
        |r: &mut Running, _: &Maps| g::x6e0(r.game, 2).to_bits()
    );
    q!(
        "Ground_801C5840",
        |c: &Ctx| fns::Ground_801C5840(c) as u32,
        |r: &mut Running, _: &Maps| g::x6e4_drawn(r.game) as u32
    );
    set!(
        "Ground_801C5A84",
        |c: &Ctx, _: &Maps| fns::Ground_801C5A84(c, 5),
        |r: &mut Running, _: &Maps| g::set_x98(r.game, 5)
    );
    q!(
        "Ground_801C5A94",
        |c: &Ctx| fns::Ground_801C5A94(c) as u32,
        |r: &mut Running, _: &Maps| g::x98(r.game) as u32
    );
    set!(
        "Ground_801C5AA4",
        |c: &Ctx, _: &Maps| fns::Ground_801C5AA4(c, 1),
        |r: &mut Running, _: &Maps| g::set_b1(r.game, true)
    );
    q!(
        "Ground_801C5ABC",
        |c: &Ctx| fns::Ground_801C5ABC(c) as u32,
        |r: &mut Running, _: &Maps| u32::from(g::b1(r.game))
    );
    for k in [10, 12, 16, 28, 29, 36, 37] {
        steps.push(ask(
            "Ground_801C5AD0",
            move |c: &Ctx| fns::Ground_801C5AD0(c, k),
            move |_: &mut Running, _: &Maps| g::flags2(k),
        ));
    }
    set!(
        "Ground_801C1154",
        |c: &Ctx, _: &Maps| fns::Ground_801C1154(c),
        |r: &mut Running, _: &Maps| g::ready(r.game)
    );
    set!(
        "Ground_801C11AC",
        |c: &Ctx, _: &Maps| fns::Ground_801C11AC(c, piece(c, 1)),
        |r: &mut Running, _: &Maps| g::reset_scale(r.game, our_map(r, 1))
    );
    set!(
        "Ground_801C2374",
        |c: &Ctx, _: &Maps| {
            let light = ssbm_rt::Handle::addr(fns::Ground_801C498C(c));
            fns::Ground_801C2374(c, handle(c, c.read_u32(light + 0x28)));
        },
        |r: &mut Running, _: &Maps| {
            let light = g::light_gobj(r.game).expect("a light GObj");
            let l = match r.game.world.gobj(light).obj {
                Some(melee_sim::game::state::Obj::LObj(l)) => Some(l),
                other => panic!("the light GObj holds {other:?}"),
            };
            g::scale_lights(r.game, r.data, l);
        }
    );
    q!(
        "Stage_GetCamMaxDepth",
        |c: &Ctx| b(fns::Stage_GetCamMaxDepth(c)),
        |r: &mut Running, _: &Maps| stage::cam_max_depth(r.game).to_bits()
    );
    q!(
        "Stage_GetCamZoomRate",
        |c: &Ctx| b(fns::Stage_GetCamZoomRate(c)),
        |r: &mut Running, _: &Maps| stage::cam_zoom_rate(r.game).to_bits()
    );
    q!(
        "Stage_GetCamInfoX20",
        |c: &Ctx| b(fns::Stage_GetCamInfoX20(c)),
        |r: &mut Running, _: &Maps| stage::cam_x20(r.game).to_bits()
    );
    q!(
        "Stage_GetCamInfoX24",
        |c: &Ctx| b(fns::Stage_GetCamInfoX24(c)),
        |r: &mut Running, _: &Maps| stage::cam_x24(r.game).to_bits()
    );
    q!(
        "Stage_GetCamFixedZoom",
        |c: &Ctx| b(fns::Stage_GetCamFixedZoom(c)),
        |r: &mut Running, _: &Maps| stage::cam_fixed_zoom(r.game).to_bits()
    );
    q!(
        "Stage_GetCamTrackRatio",
        |c: &Ctx| b(fns::Stage_GetCamTrackRatio(c)),
        |r: &mut Running, _: &Maps| stage::cam_track_ratio(r.game).to_bits()
    );
    q!(
        "Stage_GetCamTrackSmooth",
        |c: &Ctx| b(fns::Stage_GetCamTrackSmooth(c)),
        |r: &mut Running, _: &Maps| stage::cam_track_smooth(r.game).to_bits()
    );
    q!(
        "Stage_GetPauseCamZPosMin",
        |c: &Ctx| b(fns::Stage_GetPauseCamZPosMin(c)),
        |r: &mut Running, _: &Maps| stage::pause_cam_z_min(r.game).to_bits()
    );
    q!(
        "Stage_GetPauseCamZPosInit",
        |c: &Ctx| b(fns::Stage_GetPauseCamZPosInit(c)),
        |r: &mut Running, _: &Maps| stage::pause_cam_z_init(r.game).to_bits()
    );
    q!(
        "Stage_GetPauseCamZPosMax",
        |c: &Ctx| b(fns::Stage_GetPauseCamZPosMax(c)),
        |r: &mut Running, _: &Maps| stage::pause_cam_z_max(r.game).to_bits()
    );
    q!(
        "Stage_GetCamFixedFov",
        |c: &Ctx| b(fns::Stage_GetCamFixedFov(c)),
        |r: &mut Running, _: &Maps| stage::fixed_cam_fov(r.game).to_bits()
    );
    q!(
        "Stage_SetVecToFixedCamPos",
        |c: &Ctx| {
            fns::Stage_SetVecToFixedCamPos(c, handle(c, at));
            leaf_check::read_f32s(c, at, 3)
                .iter()
                .fold(0, |h, w| h.rotate_left(9) ^ w)
        },
        |r: &mut Running, _: &Maps| {
            let v = stage::fixed_cam_pos(r.game);
            [v.x, v.y, v.z]
                .iter()
                .fold(0, |h, w| h.rotate_left(9) ^ w.to_bits())
        }
    );
    q!(
        "Stage_80224CAC",
        |c: &Ctx| {
            fns::Stage_80224CAC(c, handle(c, at));
            leaf_check::read_f32s(c, at, 3)
                .iter()
                .fold(0, |h, w| h.rotate_left(9) ^ w)
        },
        |r: &mut Running, _: &Maps| {
            let v = stage::fixed_cam_interest(&r.game.stage_info);
            [v.x, v.y, v.z]
                .iter()
                .fold(0, |h, w| h.rotate_left(9) ^ w.to_bits())
        }
    );
    set!(
        "Stage_8022532C",
        |c: &Ctx, _: &Maps| fns::Stage_8022532C(c, 0x1D, 0),
        |r: &mut Running, _: &Maps| stage::demo_init(r, 0x1D, 0)
    );
    set!(
        "Ground_DemoInit",
        |c: &Ctx, _: &Maps| {
            c.write_u32(at, 29);
            c.write_u32(at + 4, 29);
            fns::Ground_DemoInit(c, handle(c, at), 1);
        },
        |r: &mut Running, _: &Maps| g::demo_init(r, 29, 1)
    );
    set!(
        "grLib_801C9E50",
        |c: &Ctx, _: &Maps| fns::grLib_801C9E50(c, -7),
        |r: &mut Running, _: &Maps| grlib::set_x708(r.game, -7)
    );
    q!(
        "grLib_801C9E40",
        |c: &Ctx| fns::grLib_801C9E40(c) as u32,
        |r: &mut Running, _: &Maps| grlib::x708(r.game) as u32
    );
    // Points near each fighter's feet and center and between them, at offsets either side of
    // touching.
    let near = [
        -8.0f32, -5.0, -2.0, 1.0, 4.0, 7.0, 10.0, 13.0, 16.0, 19.0, 22.0, 26.0, 1000.0,
    ]
    .into_iter()
    .flat_map(|dy| [(dy, 0.0f32), (dy, 1.5)]);
    for (dy, off) in near {
        for slot in 0..2u32 {
            steps.push((
                "grLib_801C9EE8",
                Box::new(move |c: &Ctx, _: &Maps| {
                    let fighter = ssbm_rt::Handle::addr(fns::Player_GetEntity(c, slot as i32));
                    let fp = c.read_u32(fighter + 0x2C);
                    let v = leaf_check::read_f32s(c, fp + 0xB0, 3);
                    leaf_check::write_f32s(
                        c,
                        at,
                        &[
                            f32::from_bits(v[0]),
                            f32::from_bits(v[1]) + dy,
                            f32::from_bits(v[2]),
                        ],
                    );
                    answer(fns::grLib_801C9EE8(c, handle(c, at), f(off)) as u32);
                    answer_vec(leaf_check::read_f32s(c, at, 3));
                }),
                Box::new(move |r: &mut Running, _: &Maps| {
                    let v = asked_vec();
                    let p = melee_sim::vec::Vec3::new(
                        f32::from_bits(v[0]),
                        f32::from_bits(v[1]),
                        f32::from_bits(v[2]),
                    );
                    let ours = grlib::point_near_object(r.game, p, off);
                    assert_eq!(
                        u32::from(ours),
                        answered(),
                        "grLib_801C9EE8 at {p:?}, {off}"
                    );
                }),
            ));
        }
    }
    // Parts reset to their descriptors, after the clouds' animation moved them.
    let parts = (0..14u32)
        .map(|p| (2u32, p))
        .chain((0..8).map(|p| (1, p)))
        .chain((0..6).map(|p| (4, p)));
    for (map, part) in parts {
        steps.push((
            "grAnime_801C86D4",
            Box::new(move |c: &Ctx, _: &Maps| {
                fns::grAnime_801C86D4(c, map as i32, piece(c, map), part as i32)
            }),
            Box::new(move |r: &mut Running, _: &Maps| {
                let g = our_map(r, map);
                melee_sim::game::gr::anime::reset_part_srt(
                    r.game,
                    r.data,
                    map as i32,
                    g,
                    part as i32,
                )
            }),
        ));
    }
    // The bob-omb rain's starting points, twenty in a row: over a player, at an item point or
    // a start point, as the draws fall.
    for _ in 0..20 {
        steps.push((
            "Ground_801C0A70",
            Box::new(move |c: &Ctx, _: &Maps| {
                leaf_check::put_vec(c, at, melee_sim::vec::Vec3::new(-1.0, -2.0, -3.0));
                invoke(c, leaf_check::sym("Ground_801C0A70"), &[at], &[]);
                answer(c.regs.r(3));
                answer_vec(leaf_check::read_f32s(c, at, 3));
            }),
            Box::new(|r: &mut Running, _: &Maps| {
                let start = melee_sim::vec::Vec3::new(-1.0, -2.0, -3.0);
                let (found, p) = g::bomb_rain_point(r.game, r.data, start);
                assert_eq!(u32::from(found), answered(), "Ground_801C0A70");
                assert_eq!(leaf_check::vec_bits(p), asked_vec(), "Ground_801C0A70");
            }),
        ));
    }
    set!(
        "grZakoGenerator_801CAEF0",
        |c: &Ctx, _: &Maps| fns::grZakoGenerator_801CAEF0(c, 0),
        |r: &mut Running, _: &Maps| zako::set_active(r.game, false)
    );
    set!(
        "grZakoGenerator_801CAEF0",
        |c: &Ctx, _: &Maps| fns::grZakoGenerator_801CAEF0(c, 1),
        |r: &mut Running, _: &Maps| zako::set_active(r.game, true)
    );
    set!(
        "grZakoGenerator_801CAEB0",
        |c: &Ctx, _: &Maps| fns::grZakoGenerator_801CAEB0(c, 0x12, 0xE0),
        |r: &mut Running, _: &Maps| zako::set_trophy_slot(r.game, 0x12, 0xE0)
    );
    set!(
        "grZakoGenerator_801CADE0",
        |c: &Ctx, _: &Maps| fns::grZakoGenerator_801CADE0(c),
        |r: &mut Running, _: &Maps| zako::scan_anywhere(r.game, r.data)
    );
    set!(
        "grMaterial_801C8A04",
        |c: &Ctx, _: &Maps| fns::grMaterial_801C8A04(
            c,
            handle(c, c.read_u32(map_addr(c, 1) + 0x28)),
            0x2000_0000
        ),
        |r: &mut Running, _: &Maps| {
            let j = g::jobj_of(r.game, our_map(r, 1));
            melee_sim::game::gr::material::clear_tree_flags(r.game, j, 0x2000_0000)
        }
    );
    set!(
        "grMaterial_801C897C",
        |c: &Ctx, _: &Maps| invoke(
            c,
            leaf_check::sym("grMaterial_801C897C"),
            &[
                c.read_u32(c.read_u32(map_addr(c, 4) + 0x28) + 0x10),
                0x0800_0000
            ],
            &[]
        ),
        |r: &mut Running, _: &Maps| {
            let root = g::jobj_of(r.game, our_map(r, 4)).expect("a model");
            let j = r.game.objs.jobjs[root].child;
            melee_sim::game::gr::material::clear_own_flags(r.game, j, 0x0800_0000)
        }
    );
    run_owned(&path, &steps, false);
}

thread_local! {
    static ASKED_VEC: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn answer_vec(v: Vec<u32>) {
    ASKED_VEC.with(|a| *a.borrow_mut() = v);
}

fn asked_vec() -> Vec<u32> {
    ASKED_VEC.with(|a| a.borrow().clone())
}

/// A stage object's sounds in its eight slots: played, asked about, given a volume and a pan,
/// stopped, replaced and all stopped, on both sides of a real stage, the audio state included.
#[test]
fn ground_object_sounds() {
    let Some(path) = yi64_state() else { return };
    fn gp(c: &Ctx, map: u32) -> ssbm_types::records::Ground<'_> {
        handle(c, ground_addr(c, map))
    }
    let ours_g = |r: &Running, map: u32| {
        let g = our_map(r, map);
        Some(melee_sim::game::gr::ground::ground_of(r.game, g))
    };
    let mut steps: Vec<OwnedStep> = Vec::new();
    steps.push((
        "Ground_801C5414",
        Box::new(|c: &Ctx, _: &Maps| fns::Ground_801C5414(c, 0x2B, 0x10)),
        Box::new(|r: &mut Running, _: &Maps| r.stage_sound_on_track(0x2B, 0x10)),
    ));
    for (slot, id) in [
        (0, 0x2Bu32),
        (3, 0x2C),
        (7, 0x2D),
        (8, 0x2B),
        (-1, 0x2B),
        (2, 540_000),
    ] {
        steps.push((
            "Ground_801C5440",
            Box::new(move |c: &Ctx, _: &Maps| fns::Ground_801C5440(c, gp(c, 2), slot, id)),
            Box::new(move |r: &mut Running, _: &Maps| {
                let g = ours_g(r, 2);
                r.object_sound(g, slot, id)
            }),
        ));
    }
    for slot in [0, 1, 3, 7, 9] {
        steps.push((
            "Ground_801C54DC",
            Box::new(move |c: &Ctx, _: &Maps| {
                answer(fns::Ground_801C54DC(c, gp(c, 2), slot) as u32)
            }),
            Box::new(move |r: &mut Running, _: &Maps| {
                let g = ours_g(r, 2);
                assert_eq!(
                    u32::from(r.object_sound_playing(g, slot)),
                    answered(),
                    "Ground_801C54DC({slot})"
                );
            }),
        ));
        steps.push((
            "Ground_801C5630",
            Box::new(move |c: &Ctx, _: &Maps| {
                fns::Ground_801C5630(c, gp(c, 2), slot, leaf_check::arg(0.4))
            }),
            Box::new(move |r: &mut Running, _: &Maps| {
                let g = ours_g(r, 2);
                r.object_sound_volume(g, slot, 0.4)
            }),
        ));
        steps.push((
            "Ground_801C5694",
            Box::new(move |c: &Ctx, _: &Maps| {
                fns::Ground_801C5694(c, gp(c, 2), slot, leaf_check::arg(0.9))
            }),
            Box::new(move |r: &mut Running, _: &Maps| {
                let g = ours_g(r, 2);
                r.object_sound_pan(g, slot, 0.9)
            }),
        ));
    }
    steps.push((
        "Ground_801C5440",
        Box::new(move |c: &Ctx, _: &Maps| fns::Ground_801C5440(c, gp(c, 2), 3, 540_001)),
        Box::new(move |r: &mut Running, _: &Maps| {
            let g = ours_g(r, 2);
            r.object_sound(g, 3, 540_001)
        }),
    ));
    steps.push((
        "Ground_801C5544",
        Box::new(move |c: &Ctx, _: &Maps| fns::Ground_801C5544(c, gp(c, 2), 0)),
        Box::new(move |r: &mut Running, _: &Maps| {
            let g = ours_g(r, 2);
            r.stop_object_sound(g, 0)
        }),
    ));
    steps.push((
        "Ground_801C55AC",
        Box::new(move |c: &Ctx, _: &Maps| {
            invoke(
                c,
                leaf_check::sym("Ground_801C55AC"),
                &[ground_addr(c, 2)],
                &[],
            )
        }),
        Box::new(move |r: &mut Running, _: &Maps| {
            let g = ours_g(r, 2);
            r.stop_object_sounds(g)
        }),
    ));
    run_owned(&path, &steps, true);
}

/// Euler angles from three axes as `Ground_801C5AEC` clamps them, on random unit and
/// near-degenerate axes, in both float modes.
#[test]
fn ground_euler_from_axes() {
    use melee_sim::game::gr::ground;
    use melee_sim::vec::Vec3;
    // Axes that make the angles NaN or infinite, which the clamp zeroes: zero, parallel and
    // too long ones.
    let degenerate = [
        (
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        ),
        (
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
            Vec3::new(0.0, 0.0, 0.0),
        ),
        (
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
        ),
        (
            Vec3::new(f32::NAN, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
    ];
    let mut k = 0;
    leaf_check::each_mode(3000, 2.0, |ctx, floats, mode| {
        let (a, b, c) = match degenerate.get(k % 3000) {
            Some(&d) => d,
            None => (floats.vec(), floats.vec(), floats.vec()),
        };
        k += 1;
        let at = leaf_check::SCRATCH;
        for (i, v) in [a, b, c].iter().enumerate() {
            leaf_check::put_vec(ctx, at + 0x10 + 0xC * i as u32, *v);
        }
        fns::Ground_801C5AEC(
            ctx,
            handle(ctx, at),
            handle(ctx, at + 0x10),
            handle(ctx, at + 0x1C),
            handle(ctx, at + 0x28),
        );
        let theirs = leaf_check::read_f32s(ctx, at, 3);
        let ours = leaf_check::vec_bits(ground::euler_from_axes(a, b, c));
        assert_eq!(ours, theirs, "{mode:?}: {a:?} {b:?} {c:?}");
    });
}

/// A map's model's joints loaded as trees of their own (`Ground_801C13D0`, which the moving
/// stages and Corneria use), from Yoshi's Island N64's states with the fighters' shadows on its
/// materials: the new trees, which the ID table now names, compared whole.
#[test]
fn ground_load_map_joint() {
    use leaf_check::calls::{Ret, check_calls_with};
    let base = std::env::var("STAGES_CALLS").unwrap_or_else(|_| ssbm_rs_local("calls/stages"));
    let dir = format!("{base}/oracle-stage-yoshis-island-64");
    let ours = |r: &mut Running, _: &Maps, call: &ssbm_rt::capture::Call| {
        let (map, depth) = (call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
        melee_sim::game::gr::ground::load_map_joint(r.game, r.data, map, depth);
        Ret::Nothing
    };
    let load = fns::addr::Ground_801C13D0;
    for (map, depth) in [(4u32, 0u32), (2, 0), (2, 5), (1, 3), (5, 300), (9, 0)] {
        let prepare = move |call: &mut ssbm_rt::capture::Call, _: usize| {
            call.addr = load;
            call.regs.gpr[3] = map;
            call.regs.gpr[4] = depth;
        };
        check_calls_with(&dir, "grOldYoshi_8020EC10", 1, &prepare, &ours);
    }
}

/// The tournament stages' callbacks no VS match reaches (each object's destroy callback, the
/// processes and setups of objects or states play rarely makes, the demos' setups), run by
/// their addresses on both sides from each stage's saved calls (`stage-more`'s sets, and
/// Battlefield's and Final Destination's own), with the stage object they belong to, and the
/// whole state compared.
#[test]
fn tournament_unreached_callbacks() {
    use leaf_check::calls::{Ret, check_calls_with, gobj_arg};
    let more = std::env::var("STAGE_MORE_CALLS").unwrap_or_else(|_| ssbm_rs_local(MORE_CALLS));
    let mine = calls_dir().to_string_lossy().into_owned();
    // A set, the function whose saved calls the callbacks run from, the map whose object r3
    // holds, and the callbacks by address.
    let cases: &[(String, &str, u32, &[u32])] = &[
        (
            format!("{more}/ps1"),
            "Ground_801C0C2C",
            2,
            &[
                0x801D_13C4,
                0x801D_156C,
                0x801D_16FC,
                0x801D_1624,
                0x801D_1E1C,
                0x801D_1E18,
                0x801D_1DE4,
            ],
        ),
        (
            format!("{more}/ps1"),
            "Ground_801C0C2C",
            5,
            &[
                0x801D_19D8,
                0x801D_16DC,
                0x801D_1604,
                0x801D_17E8,
                0x801D_1648,
            ],
        ),
        (
            format!("{more}/ys1"),
            "Ground_801C0C2C",
            1,
            &[0x801E_3230, 0x801E_3414, 0x801E_336C],
        ),
        (
            format!("{more}/dl1"),
            "Ground_801C0C2C",
            5,
            &[
                0x8021_09D4,
                0x8021_0B00,
                0x8021_0C78,
                0x8021_1198,
                0x8021_0B54,
                0x8021_0BE0,
                0x8021_1C9C,
                0x8021_1C0C,
                0x8021_110C,
            ],
        ),
        (
            format!("{more}/fod1"),
            "Ground_801C0C2C",
            3,
            &[
                0x801C_BE0C,
                0x801C_BE60,
                0x801C_CB14,
                0x801C_C338,
                0x801C_CA10,
                0x801C_CA60,
                0x801C_CA5C,
                0x801C_CA14,
            ],
        ),
        (
            format!("{mine}/bf1"),
            "grBattle_BG_Callback2",
            1,
            &[0x8021_A20C, 0x8021_A274, 0x8021_A2DC, 0x8021_A19C],
        ),
    ];
    let ours = |r: &mut Running, maps: &Maps, call: &ssbm_rt::capture::Call| {
        if call.regs.gpr[3] == 0 {
            panic!("{}the stage object isn't there", leaf_check::calls::SKIP);
        }
        melee_sim::game::code::run_proc(
            r,
            gobj_arg(maps, call),
            melee_sim::game::handles::CodeRef(call.addr),
        );
        Ret::Nothing
    };
    for (dir, from, map, targets) in cases {
        for &target in *targets {
            let map = *map;
            let prepare = move |call: &mut ssbm_rt::capture::Call, _: usize| {
                let info = leaf_check::sym("stage_info");
                call.regs.gpr[3] = leaf_check::calls::peek_u32(call, info + 0x180 + 4 * map);
                call.addr = target;
            };
            check_calls_with(dir, from, 1, &prepare, &ours);
        }
    }
    // The demos' setups: Final Destination's for demo 26 and another, Yoshi's Story's, Dream
    // Land's and Fountain of Dreams'.
    let demo = |r: &mut Running, _: &Maps, call: &ssbm_rt::capture::Call| {
        melee_sim::game::code::run_demo_init(r, call.addr, call.regs.gpr[3] as i32);
        Ret::Nothing
    };
    for (dir, from, target, arg) in [
        (
            format!("{mine}/fd1"),
            "grLast_8021AAB0",
            0x8021_A620u32,
            26u32,
        ),
        (format!("{mine}/fd1"), "grLast_8021AAB0", 0x8021_A620, 3),
        (format!("{more}/ys1"), "Ground_801C0C2C", 0x801E_302C, 0),
        (format!("{more}/dl1"), "Ground_801C0C2C", 0x8021_07DC, 0),
        (format!("{more}/fod1"), "Ground_801C0C2C", 0x801C_BB84, 0),
    ] {
        let prepare = move |call: &mut ssbm_rt::capture::Call, _: usize| {
            call.regs.gpr[3] = arg;
            call.addr = target;
        };
        check_calls_with(&dir, from, 1, &prepare, &demo);
    }
}

/// Every stage kind's ground kind (`stage_id_map`), against the executable's table.
#[test]
fn stage_ground_kinds() {
    let Some(ctx) = reference() else { return };
    for k in 0..286 {
        let theirs = fns::Stage_8022519C(&ctx, k);
        assert_eq!(
            melee_sim::lb::audio::ground_kind(k) as i32,
            theirs,
            "stage kind {k}"
        );
    }
}

/// Pokémon Stadium's grass and default stages' destroy callbacks right after the grass stage's
/// setup put its collision joint in the search list, on a real stage (`stage-more`'s `ps1`),
/// so the joints they take out are there to take.
#[test]
fn stadium_piece_destroys() {
    use melee_sim::game::gr::stadium;
    let more = std::env::var("STAGE_MORE_CALLS").unwrap_or_else(|_| ssbm_rs_local(MORE_CALLS));
    let mut paths: Vec<PathBuf> = melee_capture::pack::call_files(format!("{more}/ps1"))
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("Ground_801C0C2C-call"))
        })
        .collect();
    paths.sort();
    let Some(path) = paths.first() else {
        eprintln!("no ps1 captures: skipping");
        return;
    };
    let init_t = |c: &Ctx, _: &Maps| invoke(c, 0x801D_1648, &[map_addr(c, 5)], &[]);
    let init_o = |r: &mut Running, _: &Maps| stadium::init_grass(r, our_map(r, 5));
    let grass_t = |c: &Ctx, _: &Maps| invoke(c, 0x801D_16FC, &[map_addr(c, 5)], &[]);
    let grass_o = |r: &mut Running, _: &Maps| stadium::destroy_grass(r, our_map(r, 5));
    let default_t = |c: &Ctx, _: &Maps| invoke(c, 0x801D_1624, &[map_addr(c, 5)], &[]);
    let default_o = |r: &mut Running, _: &Maps| stadium::destroy_default(r, our_map(r, 5));
    on_state(
        path,
        &[
            ("grStadium_801D1648", &init_t, &init_o),
            ("grStadium_801D16FC", &grass_t, &grass_o),
            ("grStadium_801D1648", &init_t, &init_o),
            ("grStadium_801D1624", &default_t, &default_o),
        ],
    );
}

/// The camera's clear color from the stage's fog (`Ground_ApplyStageBackgroundColor`), on
/// Final Destination, whose fog Yoshi's Island N64 lacks: the color compared explicitly.
#[test]
fn ground_background_color() {
    use melee_sim::game::gr::ground;
    let Some(path) = running_state() else { return };
    let theirs = |c: &Ctx, _: &Maps| {
        fns::Ground_ApplyStageBackgroundColor(c);
        answer(c.read_u32(leaf_check::sym("game_camera") + 8) >> 8);
    };
    let ours = |r: &mut Running, _: &Maps| {
        assert!(ground::fog_color(r.game).is_some(), "the stage's fog");
        ground::apply_background_color(r.game);
        let b = &r.game.camera.cam.background_color;
        let c = u32::from_be_bytes([0, b.r, b.g, b.b]);
        assert_eq!(c, answered(), "Ground_ApplyStageBackgroundColor");
    };
    on_state(&path, &[("Ground_ApplyStageBackgroundColor", &theirs, &ours)]);
}
