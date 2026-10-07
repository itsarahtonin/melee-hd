//! The stage system against ssbm-rs from real game states: each test imports every capture of
//! one function, runs ssbm-rs's side (`run_capture`) and ours from the same state, and compares
//! the whole state afterwards, in both float modes.
//!
//! The captures are game memory, kept out of the repository. Regenerate them with ssbm-rs's
//! runner (branch `feat/state-trace`), from `C:/Projects/ssbm-rs-state-trace`:
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

use std::path::PathBuf;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, reference, run_capture};
use melee_capture::compare::Comparer;
use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, DiscIndex, disc_index, find_archives_in};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_sim::game::gr::ground;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::Ctx;

const DEFAULT_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/stage";

fn calls_dir() -> PathBuf {
    std::env::var("STAGE_CALLS")
        .unwrap_or_else(|_| DEFAULT_CALLS.into())
        .into()
}

/// Every capture of `func` under the calls directory, in name order.
fn captures(func: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(calls_dir()) else {
        return out;
    };
    for d in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(d.path()) else {
            continue;
        };
        for f in files.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{func}-call")) && name.ends_with(".call") {
                out.push(f.path());
            }
        }
    }
    out.sort();
    out
}

struct Env {
    ctx: Ctx,
    index: DiscIndex,
}

fn env() -> Option<Env> {
    let ctx = reference()?;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    Some(Env {
        ctx,
        index: disc_index(&disc),
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
        for mode in MODES {
            set_fp_mode(mode);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(&env.index, &cap.call.mem1, &mut archives),
            };
            let mut imp = Importer::new(&cap, &data);
            imp.world();
            imp.stage();
            imp.ids();
            let Importer { mut game, maps, .. } = imp;
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
        loaded: find_archives_in(&env.index, &cap.call.mem1, &mut archives),
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
const MORE_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/stage-more";

/// Every capture of [`MORE_CALLS`] imports and compares with its own memory with no difference:
/// the stages' objects, their archives (Pokémon Stadium's transformation files among them) and
/// the effects and particles they hold.
#[test]
fn tournament_stage_imports_round_trip() {
    let Some(env) = env() else { return };
    let dir = std::env::var("STAGE_MORE_CALLS").unwrap_or_else(|_| MORE_CALLS.to_owned());
    let Ok(dirs) = std::fs::read_dir(&dir) else {
        eprintln!("no captures in {dir}: skipping");
        return;
    };
    let (mut n, mut failed) = (0, Vec::new());
    for d in dirs.flatten().filter(|d| d.path().is_dir()) {
        let mut files: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .map(|f| f.path())
            .filter(|p| p.extension().is_some_and(|e| e == "call"))
            .collect();
        files.sort();
        for path in files {
            let cap = Capture::read(&path);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(&env.index, &cap.call.mem1, &mut archives),
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
        setup::stage_gobj(r, int_arg(cap, 0) as i32);
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
        last::create(r, int_arg(cap, 0) as i32);
    });
}

#[test]
fn last_init_main() {
    check_proc("grLast_8021A9C4", last::init_main);
}

#[test]
fn last_set_phase() {
    check("grLast_8021B920", &|r, cap, maps| {
        last::set_phase(r, gobj_arg(cap, maps), int_arg(cap, 1) as i32)
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
    let Some(env) = env() else { return };
    let cap = Capture::read(path);
    for mode in MODES {
        set_fp_mode(mode);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(&env.index, &cap.call.mem1, &mut archives),
        };
        let mut imp = Importer::new(&cap, &data);
        imp.world();
        imp.stage();
        imp.ids();
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
                    last::create(r, 9);
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
        battle::create(r, int_arg(cap, 0) as i32);
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
