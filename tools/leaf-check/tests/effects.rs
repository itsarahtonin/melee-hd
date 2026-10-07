//! The effects (efLib, efSync, efAsync, efAlt) against ssbm-rs, from real game states: each
//! saved call of an effect function is imported into melee-sim, run on both sides, and the
//! whole state compared afterwards (`leaf_check::calls`): the GObjs with every effect and joint
//! tree, efLib's globals, the fighters' deferred spawns and the particle system. From the same
//! states, `every_effect` spawns every effect id with arguments of the kinds each case reads.
//!
//! The calls are game memory, kept out of the repository. Regenerate them with ssbm-rs's
//! runner, `capture-funcs.txt` listing the functions below and `done-nonef.txt` every other
//! function, over a replay per stage with Fox and Marth (and others' for their effects):
//!
//! ```text
//! CAPTURE=$EFFECT_CALLS/fd4 CAPTURE_FUNCS=@capture-funcs.txt CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   LOCKSTEP_DONE=done-nonef.txt ssbm-run DISC \
//!   --replay Game_20260531T183349.slp --fields 7000 --port all --lockstep \
//!   --known local/replays/known/Game_20260531T183349.txt
//! ```
//!
//! (`bf4` from `Game_20260531T175747.slp`.) `EFFECT_CALLS` names the directory, by default
//! below; the tests skip without it.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::calls::{Ret, check_calls, gobj_arg, spawned_diff};
use leaf_check::{MODES, Rand, disc_index, reference};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps};
use melee_capture::{Capture, DataMap, MEM1, find_archives_after, find_archives_in};
use melee_data::archive::Archives;
use melee_sim::game::ef::{self, Arg, ArgKind, Args, SpawnKind, Spawned};
use melee_sim::game::state::Running;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const DEFAULT_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/effects";

fn calls_root() -> PathBuf {
    std::env::var("EFFECT_CALLS")
        .unwrap_or_else(|_| DEFAULT_CALLS.into())
        .into()
}

/// The capture directories (one per replay), in name order.
fn dirs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(calls_root()) else {
        return Vec::new();
    };
    let mut v: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn word(call: &Call, a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(call.mem1[i..i + 4].try_into().unwrap())
}

fn word_ctx(ctx: &ssbm_rt::Ctx, a: u32) -> u32 {
    ctx.read_u32(a)
}

fn float_at(call: &Call, a: u32) -> f32 {
    f32::from_bits(word(call, a))
}

/// Variadic argument `i` of a call, read as `kind`: registers from `first` (r5 for
/// `efSync_Spawn`) up to r10, then the caller's parameter area.
fn decode(maps: &Maps, call: &Call, first: usize, i: usize, kind: ArgKind) -> Arg {
    let n = first + i;
    let raw = if n <= 10 {
        call.regs.gpr[n]
    } else {
        word(call, call.regs.gpr[1] + 8 + 4 * (n - 11) as u32)
    };
    match kind {
        ArgKind::JObj => Arg::JObj((raw != 0).then(|| {
            *maps
                .jobjs
                .get(&raw)
                .unwrap_or_else(|| panic!("a joint argument {raw:#x} the import doesn't know"))
        })),
        ArgKind::Vec => Arg::Vec(Vec3::new(
            float_at(call, raw),
            float_at(call, raw + 4),
            float_at(call, raw + 8),
        )),
        ArgKind::Float => Arg::Float(float_at(call, raw)),
        ArgKind::Int => Arg::Int(raw),
    }
}

fn each_dir(func: &str, min: usize, ours: &leaf_check::calls::Ours) {
    let dirs = dirs();
    if dirs.is_empty() {
        eprintln!(
            "no effect calls in {}: skipping (see this file's docs to make them)",
            calls_root().display()
        );
        return;
    }
    for d in dirs {
        check_calls(&d, func, min, ours);
    }
}

#[test]
fn spawn_calls() {
    each_dir("efSync_Spawn", 1, &|r, maps, call| {
        let gfx = call.regs.gpr[3] as i32;
        let gobj = maps.gobjs.get(&call.regs.gpr[4]).copied();
        let mut get = |i, kind| decode(maps, call, 5, i, kind);
        Ret::Spawned(ef::spawn_with(r, gfx, gobj, &mut Args::lazy(&mut get)))
    });
}

/// The fighter whose queue head `head` (a pointer to `Fighter::x60C`) is.
fn queue_owner(maps: &Maps, head: u32) -> melee_sim::game::handles::FighterId {
    *maps
        .fighters
        .get(&(head - 0x60C))
        .unwrap_or_else(|| panic!("queue head {head:#x} isn't a fighter's"))
}

#[test]
fn queue_calls() {
    each_dir("efAsync_Spawn", 1, &|r, maps, call| {
        let g = &call.regs.gpr;
        let gobj = maps.gobjs.get(&g[3]).copied();
        let f = queue_owner(maps, g[4]);
        let kind = SpawnKind::from_u8(g[5] as u8).expect("a spawn kind");
        let jobj = (g[7] != 0).then(|| maps.jobjs[&g[7]]);
        let mut get = |i, k| decode(maps, call, 8, i, k);
        let mut head = r.game.fighters[f.0].x60C;
        ef::queue_with(
            r,
            gobj,
            &mut head,
            kind,
            g[6] as i32,
            jobj,
            &mut Args::lazy(&mut get),
        );
        r.game.fighters[f.0].x60C = head;
        Ret::Nothing
    });
}

#[test]
fn flush_calls() {
    each_dir("efAsync_QueueFlush", 1, &|r, maps, call| {
        let gobj = maps.gobjs.get(&call.regs.gpr[3]).copied();
        let f = queue_owner(maps, call.regs.gpr[4]);
        let mut head = r.game.fighters[f.0].x60C;
        ef::flush(r, gobj, &mut head);
        r.game.fighters[f.0].x60C = head;
        Ret::Nothing
    });
}

#[test]
fn update_calls() {
    each_dir("efLib_Update", 1, &|r, maps, call| {
        ef::update(r, gobj_arg(maps, call));
        Ret::Nothing
    });
}

#[test]
fn particle_proc_calls() {
    each_dir("efLib_particles_proc_main", 1, &|r, _, _| {
        ef::particles_proc_main(r);
        Ret::Nothing
    });
    each_dir("efLib_particles_proc_aux", 1, &|r, _, _| {
        ef::particles_proc_aux(r);
        Ret::Nothing
    });
}

#[test]
fn destroy_all_calls() {
    each_dir("efLib_DestroyAll", 1, &|r, maps, call| {
        ef::destroy_all(r, gobj_arg(maps, call));
        Ret::Nothing
    });
}

#[test]
fn pause_and_flag_calls() {
    each_dir("efLib_PauseAll", 1, &|r, maps, call| {
        ef::pause_all(r.game, gobj_arg(maps, call));
        Ret::Nothing
    });
}

/// The effect ids with a case of their own: efAsync's, efAlt's and efSync's tables.
fn table_ids() -> Vec<i32> {
    (0x3E8..=0x512).collect()
}

/// A few plain generators of each kind of link, and ids past the tables.
fn plain_ids() -> Vec<i32> {
    vec![
        0, 1, 0x2D, 0x96, 0xF7, 0xFC, 0xFF, 0x121, 0x24F, 0x250, 0x3E7, 0x478, 0x4BA, 0x513,
    ]
}

/// `ftCo_800C0FCC`, the bury effect's update.
const BURY: u32 = 0x800C_0FCC;

thread_local! {
    static THEIR_BURY: std::cell::RefCell<Vec<(u32, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn their_bury(ctx: &ssbm_rt::Ctx) {
    let call = (ctx.regs.r(3), ctx.regs.r(4));
    THEIR_BURY.with(|b| b.borrow_mut().push(call));
}

/// Values for a spawn's arguments as it reads them, recorded for the reference.
struct Synth<'a> {
    rand: Rand,
    joints: &'a [(u32, JObjId)],
    at: Vec3,
    made: Vec<Arg>,
}

impl Synth<'_> {
    fn arg(&mut self, kind: ArgKind) -> Arg {
        let r = &mut self.rand;
        let a = match kind {
            ArgKind::JObj => {
                let (_, j) = self.joints[r.below(self.joints.len() as u64) as usize];
                Arg::JObj(Some(j))
            }
            ArgKind::Vec => Arg::Vec(Vec3::new(
                self.at.x + r.range(-20.0, 20.0),
                self.at.y + r.range(-5.0, 30.0),
                self.at.z + r.range(-1.0, 1.0),
            )),
            ArgKind::Float => Arg::Float(match r.below(4) {
                0 => -1.0,
                1 => 1.0,
                _ => r.range(-3.0, 3.0),
            }),
            ArgKind::Int => Arg::Int(if r.below(2) == 0 {
                0x0060_6060u32.wrapping_add(0x0060_0000)
            } else {
                (r.bits() as u32) & 0x00FF_FFFF
            }),
        };
        self.made.push(a);
        a
    }
}

/// Every effect id spawned by a fighter in a real state, with arguments of the kinds each case
/// reads (the first saved `efSync_Spawn` of each directory). An id whose file isn't loaded there
/// is skipped: the original reads low memory for its descriptor.
#[test]
fn every_effect() {
    let Some(index) = disc_index() else {
        eprintln!("SSBM_DISC is unset: skipping");
        return;
    };
    let dirs = dirs();
    if dirs.is_empty() {
        eprintln!("no effect calls: skipping");
        return;
    }
    // EFFECT_IDS=ID,... (hex) and EFFECT_DIRS=NAME,... narrow the run, for mutation checks.
    let ids: Vec<i32> = match std::env::var("EFFECT_IDS") {
        Ok(v) => v
            .split(',')
            .map(|x| i32::from_str_radix(x.trim().trim_start_matches("0x"), 16).unwrap())
            .collect(),
        Err(_) => plain_ids().into_iter().chain(table_ids()).collect(),
    };
    let only_dirs: Option<Vec<String>> = std::env::var("EFFECT_DIRS")
        .ok()
        .map(|v| v.split(',').map(str::to_owned).collect());
    let spawn_addr = leaf_check::sym("efSync_Spawn");
    let mut failed = Vec::new();
    let mut compared = 0;
    let mut skipped = std::collections::BTreeMap::<String, Vec<i32>>::new();
    for d in &dirs {
        if let Some(only) = &only_dirs
            && !only.iter().any(|o| d.ends_with(o.as_str()))
        {
            continue;
        }
        // The first spawn by a fighter (user data kind 4).
        let Some((path, cap)) = leaf_check::calls::saved_calls(d, "efSync_Spawn")
            .into_iter()
            .map(|p| {
                let c = Capture::read(&p);
                (p, c)
            })
            .find(|(_, c)| {
                let g = c.call.regs.gpr[4];
                g != 0 && c.call.mem1[(g + 7 - MEM1) as usize] == 4
            })
        else {
            continue;
        };
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
        };
        let imported = catch_unwind(AssertUnwindSafe(|| {
            let mut imp = Importer::new(&cap, &data).with_archives(&archives);
            imp.world();
            imp.stage();
            imp.ids();
            let Importer { game, maps, .. } = imp;
            (game, maps)
        }));
        let Ok((game, maps)) = imported else {
            failed.push(format!("{}: the import panicked", path.display()));
            continue;
        };
        let gobj_addr = cap.call.regs.gpr[4];
        let gobj = maps.gobjs[&gobj_addr];
        // The spawner's joints: its fighter's parts.
        let fp = game.fighter(gobj);
        let parts = &game.fighter_parts[fp.parts.expect("parts").0];
        let addr_of: std::collections::HashMap<JObjId, u32> =
            maps.jobjs.iter().map(|(&a, &j)| (j, a)).collect();
        let joints: Vec<(u32, JObjId)> = parts
            .iter()
            .filter_map(|b| b.joint)
            .filter_map(|j| addr_of.get(&j).map(|&a| (a, j)))
            .collect();
        let at = fp.cur_pos;
        let ctx = reference().expect("SSBM_DISC");
        // The bury effect's update calls the fighter code's `ftCo_800C0FCC`, unported: both
        // sides stand it in and log its arguments.
        ctx.register(BURY, their_bury);
        melee_sim::game::code::stubs::set(BURY, melee_sim::game::code::Val::None);
        for mode in MODES {
            set_fp_mode(mode);
            for &gfx in &ids {
                let _ = melee_sim::game::code::stubs::take();
                THEIR_BURY.with(|b| b.borrow_mut().clear());
                let mut game = game.clone();
                let maps = maps.clone();
                let mut archives = archives.clone();
                let mut synth = Synth {
                    rand: Rand(0xEF5E_ED00 ^ gfx as u64 ^ (path.as_os_str().len() as u64) << 32),
                    joints: &joints,
                    at,
                    made: Vec::new(),
                };
                let ours = catch_unwind(AssertUnwindSafe(|| {
                    let mut get = |_, kind| synth.arg(kind);
                    ef::spawn_with(
                        &mut Running {
                            game: &mut game,
                            data: &archives,
                        },
                        gfx,
                        Some(gobj),
                        &mut Args::lazy(&mut get),
                    )
                }));
                let spawned: Spawned = match ours {
                    Ok(s) => s,
                    Err(p) => {
                        let why = p
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                            .unwrap_or_default();
                        // A case for another character reads parts the spawner lacks.
                        if why.contains("isn't loaded")
                            || why.contains("isn't ported")
                            || why.contains("past the character's parts")
                        {
                            skipped.entry(why).or_default().push(gfx);
                            continue;
                        }
                        failed.push(format!(
                            "{}: {gfx:#x} ({mode:?}): ours panicked: {why}",
                            path.display()
                        ));
                        continue;
                    }
                };
                // Their side: the same call, the arguments where the spawn reads them.
                cap.call.load(&ctx);
                let sp = cap.call.regs.gpr[1];
                let mut scratch = sp - 0x4000;
                ctx.regs.set_r(3, gfx as u32);
                ctx.regs.set_r(4, gobj_addr);
                for (i, a) in synth.made.iter().enumerate() {
                    let v = match *a {
                        Arg::JObj(j) => j.map_or(0, |j| addr_of[&j]),
                        Arg::Vec(p) => {
                            let at = scratch;
                            leaf_check::put_vec(&ctx, at, p);
                            scratch += 0x10;
                            at
                        }
                        Arg::Float(x) => {
                            let at = scratch;
                            ctx.write_u32(at, x.to_bits());
                            scratch += 0x10;
                            at
                        }
                        Arg::Int(x) => x,
                        Arg::Params(_) => unreachable!(),
                    };
                    assert!(i < 6, "more variadic arguments than registers");
                    ctx.regs.set_r(5 + i, v);
                }
                ctx.invoke(spawn_addr);
                let r3 = ctx.regs.r(3);
                let mut after = vec![0u8; cap.call.mem1.len()];
                let _ = ctx.mem.read_bytes(0x8000_0000, &mut after);
                let data2 = DataMap {
                    loaded: find_archives_after(index, &after, &mut archives, &data.loaded),
                };
                let mut cmp = Comparer::new(&after, &data2, &game, maps);
                cmp.world();
                cmp.stage();
                cmp.ids();
                if let Some(d) = spawned_diff(&cmp.maps, spawned, r3) {
                    cmp.diffs.push(d);
                }
                let mut diffs = std::mem::take(&mut cmp.diffs);
                let cmp_maps = cmp.maps.clone();
                drop(cmp);
                // Then the effect's first frames: its update and update callback, while it
                // lives on.
                if diffs.is_empty()
                    && let Spawned::Effect(e) = spawned
                {
                    let maps = cmp_maps.clone();
                    let update = leaf_check::sym("efLib_Update");
                    let mut frames = 0;
                    for _ in 0..3 {
                        let ef_ = &game.ef.effects[e.0];
                        if ef_.lifetime == 1 {
                            break;
                        }
                        let g = ef_.gobj.expect("an effect's GObj");
                        let ran = catch_unwind(AssertUnwindSafe(|| {
                            ef::update(
                                &mut Running {
                                    game: &mut game,
                                    data: &archives,
                                },
                                g,
                            )
                        }));
                        if let Err(p) = ran {
                            let why = p.downcast_ref::<String>().cloned().unwrap_or_default();
                            assert!(why.contains("isn't ported"), "{gfx:#x}'s update: {why}");
                            skipped
                                .entry(format!("updates: {why}"))
                                .or_default()
                                .push(gfx);
                            frames = 0;
                            break;
                        }
                        ctx.regs.set_r(3, word_ctx(&ctx, r3 + 4));
                        ctx.invoke(update);
                        frames += 1;
                    }
                    // The stood-in calls the updates made, with the same GObjs.
                    let gobj_addr: std::collections::HashMap<_, _> =
                        maps.gobjs.iter().map(|(&a, &g)| (g, a)).collect();
                    let ours: Vec<(u32, u32)> = melee_sim::game::code::stubs::take()
                        .into_iter()
                        .map(|(_, args)| {
                            let a = |v: &melee_sim::game::code::Val| {
                                v.gobj()
                                    .map_or(0, |g| gobj_addr.get(&g).copied().unwrap_or(1))
                            };
                            (a(&args[0]), a(&args[1]))
                        })
                        .collect();
                    let theirs = THEIR_BURY.with(|b| std::mem::take(&mut *b.borrow_mut()));
                    if ours != theirs {
                        diffs.push(format!(
                            "ftCo_800C0FCC calls: ours {ours:x?}, theirs {theirs:x?}"
                        ));
                    }
                    if frames > 0 {
                        let _ = ctx.mem.read_bytes(0x8000_0000, &mut after);
                        let mut again = Comparer::new(&after, &data2, &game, maps);
                        again.world();
                        again.stage();
                        again.ids();
                        for d in again.diffs {
                            diffs.push(format!("after {frames} updates: {d}"));
                        }
                    }
                }
                if diffs.is_empty() {
                    compared += 1;
                } else {
                    failed.push(format!(
                        "{}: {gfx:#x} ({mode:?}) with {:?}: {} differences:\n    {}",
                        path.display(),
                        synth.made,
                        diffs.len(),
                        diffs[..diffs.len().min(12)].join("\n    ")
                    ));
                }
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
    for (why, ids) in &skipped {
        eprintln!(
            "skipped {} ids: {why}: {:x?}",
            ids.len(),
            &ids[..ids.len().min(40)]
        );
    }
    for f in failed.iter().take(40) {
        eprintln!("{f}");
    }
    eprintln!("{compared} spawns compared clean, {} differ", failed.len());
    assert!(failed.is_empty(), "{} spawns differ", failed.len());
    assert!(compared > 0, "no spawn compared");
}

/// Runs `ours` against the original at `func` from every saved call of `from` in every
/// directory, each changed by `prepare` (given the call's index, to vary inputs) first.
fn each_dir_with(
    from: &str,
    func: &str,
    prepare: &leaf_check::calls::Prepare,
    ours: &leaf_check::calls::Ours,
) {
    each_dir_rounds(from, func, 1, prepare, ours);
}

/// [`each_dir_with`] `rounds` times over, the call indices running on across rounds.
fn each_dir_rounds(
    from: &str,
    func: &str,
    rounds: usize,
    prepare: &leaf_check::calls::Prepare,
    ours: &leaf_check::calls::Ours,
) {
    let target = leaf_check::sym(func);
    for d in dirs() {
        let n = leaf_check::calls::saved_calls(&d, from).len();
        for round in 0..rounds {
            leaf_check::calls::check_calls_with(
                &d,
                from,
                0,
                &|call, i| {
                    call.addr = target;
                    prepare(call, round * n + i);
                },
                ours,
            );
        }
    }
}

/// The `i`th GObj of process link `link` in a call's memory, if it has that many.
fn nth_in_link(call: &Call, link: u32, i: usize) -> Option<u32> {
    let mut g = word(
        call,
        word(call, leaf_check::sym("HSD_GObjPLinkHead")) + 4 * link,
    );
    for _ in 0..i {
        if g == 0 {
            return None;
        }
        g = word(call, g + 8);
    }
    (g != 0).then_some(g)
}

#[test]
fn set_flags_resume_and_pause() {
    // The spawner of a real spawn, whose effects these go through.
    each_dir_with(
        "efSync_Spawn",
        "efLib_SetFlags",
        &|call, i| {
            call.regs.gpr[3] = call.regs.gpr[4];
            call.regs.gpr[4] = 1 << (i % 3);
        },
        &|r, maps, call| {
            ef::set_flags(r.game, gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
    each_dir_with(
        "efSync_Spawn",
        "efLib_ResumeAll",
        &|call, _| call.regs.gpr[3] = call.regs.gpr[4],
        &|r, maps, call| {
            ef::resume_all(r.game, gobj_arg(maps, call));
            Ret::Nothing
        },
    );
    // An effect's parent, from an effect's update.
    each_dir_with(
        "efLib_Update",
        "efLib_PauseAll",
        &|call, _| {
            let ud = word(call, call.regs.gpr[3] + 0x2C);
            call.regs.gpr[3] = word(call, ud + 8);
        },
        &|r, maps, call| {
            ef::pause_all(r.game, gobj_arg(maps, call));
            Ret::Nothing
        },
    );
}

#[test]
fn destroy_calls() {
    // An effect of the state, or another GObj of its link.
    each_dir_with(
        "efLib_Update",
        "efLib_Destroy",
        &|call, i| {
            if let Some(g) = nth_in_link(call, 11, i % 6) {
                call.regs.gpr[3] = g;
            }
        },
        &|r, maps, call| {
            ef::destroy(r, gobj_arg(maps, call));
            Ret::Nothing
        },
    );
    // The spawner's effects and its own joints' generators.
    each_dir_with(
        "efSync_Spawn",
        "efLib_DestroyAll",
        &|call, _| call.regs.gpr[3] = call.regs.gpr[4],
        &|r, maps, call| {
            ef::destroy_all(r, gobj_arg(maps, call));
            Ret::Nothing
        },
    );
}

#[test]
fn remove_last_calls() {
    // As if 64 or 65 counted effects were alive: the first ones counted go.
    each_dir_with(
        "efLib_Update",
        "efLib_RemoveLast",
        &|call, i| {
            let at = leaf_check::sym("efLib_EffectCount");
            let n = 64 + (i % 2) as u32;
            let idx = (at - MEM1) as usize;
            call.mem1[idx..idx + 4].copy_from_slice(&n.to_be_bytes());
        },
        &|r, _, _| {
            ef::remove_last(r);
            Ret::Nothing
        },
    );
}

/// A joint in a call's state: its spawner's, or else an effect's.
fn some_joint(call: &Call) -> u32 {
    let spawner = call.regs.gpr[4];
    if spawner != 0 && word(call, spawner + 0x28) != 0 {
        return word(call, spawner + 0x28);
    }
    (0..)
        .map_while(|i| nth_in_link(call, 11, i))
        .map(|g| word(call, g + 0x28))
        .find(|&j| j != 0)
        .expect("a joint in the state")
}

/// Ids `efLib_SpawnParticleEffect` treats apart, and plain ones of each link and bank.
const PARTICLE_IDS: &[i32] = &[
    0x4A38, 0x4A39, 0x4A3A, 0xB798, 0xB799, 0xB79A, 0x2D, 0x2E, 0x31, 0x127, 0x2, 0x6, 0xA3, 0xA7,
    0xAA, 0xF2, 0x12A, 0x132, 0x133, 0x16D, 0x16E, 0x16F, 0x170, 0x7E2, 0xD4, 0x243, 0xE3, 0xFC,
    0xFF, 0xF7, 0x7918, 0x0, 0x96, 0x21B, 0xBB8, 0xFA0, 0x3E8, 0x7D0, 0x1388, 0x3E80,
];

/// A joint's particle entry spawned on the spawner's root joint (`efLib_Cb_SPtcl`, which the
/// display pass calls) and an animation's particle event (`efLib_Cb_DPtcl`), for every id
/// efLib treats apart.
#[test]
fn joint_particle_calls() {
    for (func, dynamic) in [("efLib_Cb_SPtcl", false), ("efLib_Cb_DPtcl", true)] {
        each_dir_rounds(
            "efSync_Spawn",
            func,
            5,
            &|call, i| {
                let id = PARTICLE_IDS[i % PARTICLE_IDS.len()];
                let j = some_joint(call);
                call.regs.gpr[3] = 0;
                call.regs.gpr[4] = (id / 1000) as u32;
                call.regs.gpr[5] = id as u32;
                call.regs.gpr[6] = j;
            },
            &move |r, maps, call| {
                let j = maps.jobjs[&call.regs.gpr[6]];
                let (bank, id) = (call.regs.gpr[4] as i32, call.regs.gpr[5] as i32);
                let hooks = &mut melee_sim::game::code::ParticleHooks;
                if dynamic {
                    ef::cb_dptcl(&mut r.game.objs, r.data, hooks, bank, id, j);
                } else {
                    ef::cb_sptcl(&mut r.game.objs, r.data, hooks, bank, id, j);
                }
                Ret::Nothing
            },
        );
    }
}

/// The particle GObjs' render callback in each pass: the display frame advances, or the
/// links sort (`efLib_render_callback`).
#[test]
fn render_calls() {
    for from in ["efLib_particles_proc_main", "efLib_particles_proc_aux"] {
        each_dir_with(
            from,
            "efLib_render_callback",
            &|call, i| call.regs.gpr[4] = (i % 4) as u32,
            &|r, maps, call| {
                let _ = ef::render(r.game, gobj_arg(maps, call), call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        );
    }
}

/// `efLib_Init` from the match's start.
#[test]
fn init_calls() {
    each_dir("efLib_Init", 1, &|r, _, _| {
        ef::init(r);
        Ret::Nothing
    });
}

/// `efAsync_LoadSync`'s result, against the state the original reached once it had loaded
/// the file (the reference can't read the disc): each loaded file forgotten (its table and
/// banks) and loaded again, and a loaded file loaded again unchanged.
#[test]
fn load_sync_from_loaded_states() {
    let Some(index) = disc_index() else { return };
    let mut checked = 0;
    for d in dirs() {
        for path in leaf_check::calls::saved_calls(&d, "efSync_Spawn")
            .into_iter()
            .take(1)
        {
            let cap = Capture::read(&path);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
            };
            let mut imp = Importer::new(&cap, &data).with_archives(&archives);
            imp.world();
            imp.stage();
            imp.ids();
            let Importer { game, maps, .. } = imp;
            for i in 0..50 {
                let Some(t) = game.ef.tables[i] else { continue };
                let mut g = game.clone();
                g.ef.tables[i] = None;
                g.objs.particles.banks[i] = None;
                for _ in 0..2 {
                    ef::load_sync(
                        &mut Running {
                            game: &mut g,
                            data: &archives,
                        },
                        i as i32,
                    );
                    assert_eq!(
                        g.ef.tables[i],
                        Some(t),
                        "{}: bank {i}'s table",
                        path.display()
                    );
                    let mut cmp = Comparer::new(&cap.call.mem1, &data, &g, maps.clone());
                    cmp.world();
                    assert!(
                        cmp.diffs.is_empty(),
                        "{}: bank {i} loaded again: {:?}",
                        path.display(),
                        &cmp.diffs[..cmp.diffs.len().min(10)]
                    );
                }
                checked += 1;
            }
        }
    }
    eprintln!("{checked} effect files loaded again");
}

/// A fighter's deferred spawns freed unrun (`efAsync_QueueClear`), from the states its queue
/// was flushed in.
#[test]
fn queue_clear_calls() {
    each_dir_with(
        "efAsync_QueueFlush",
        "efAsync_QueueClear",
        &|call, _| call.regs.gpr[3] = call.regs.gpr[4],
        &|r, maps, call| {
            let f = queue_owner(maps, call.regs.gpr[3]);
            let mut head = r.game.fighters[f.0].x60C;
            ef::queue_clear(r, &mut head);
            r.game.fighters[f.0].x60C = head;
            Ret::Nothing
        },
    );
}

/// Battlefield's captured states, whose backgrounds have generators from the stage's bank.
fn stage_dir() -> String {
    std::env::var("STAGE_CALLS")
        .unwrap_or_else(|_| "C:/Projects/ssbm-rs-publication/local/calls/stage".into())
        + "/bf1"
}

/// The first generator in a call's state, and the joint it follows.
fn first_generator(call: &Call) -> (u32, u32) {
    let g = word(call, leaf_check::sym("hsd_804D78FC"));
    (g, if g == 0 { 0 } else { word(call, g + 0x10) })
}

/// grLib's particle helpers on Battlefield's states: generators of the stage's bank on a
/// background's joint, plain, oriented and at a point, and generators removed, alone, with
/// their particles, and by joint and tree.
#[test]
fn stage_particle_helpers() {
    use melee_sim::game::gr::grlib;
    let dir = stage_dir();
    let from = "grBattle_BG_Callback2";
    let target = |name: &str| leaf_check::sym(name);
    // A generator of the stage's bank on the object's joint.
    let on_joint = |name: &'static str| {
        let t = target(name);
        move |call: &mut Call, i: usize| {
            let j = word(call, call.regs.gpr[3] + 0x28);
            call.addr = t;
            call.regs.gpr[3] = 30000 + (i % 4) as u32;
            call.regs.gpr[4] = 0x1E;
            call.regs.gpr[5] = j;
        }
    };
    leaf_check::calls::check_calls_with(
        &dir,
        from,
        1,
        &on_joint("grLib_801C9808"),
        &|r, maps, call| {
            let g = &call.regs.gpr;
            let j = (g[5] != 0).then(|| maps.jobjs[&g[5]]);
            Ret::Spawned(Spawned::from_generator(grlib::generator_on(
                r.game,
                r.data,
                g[3] as i32,
                g[4] as i32,
                j,
            )))
        },
    );
    leaf_check::calls::check_calls_with(
        &dir,
        from,
        1,
        &on_joint("grLib_801C97DC"),
        &|r, maps, call| {
            let g = &call.regs.gpr;
            let j = (g[5] != 0).then(|| maps.jobjs[&g[5]]);
            grlib::generator_oriented_on(r.game, r.data, g[3] as i32, g[4] as i32, j);
            Ret::Nothing
        },
    );
    let at = target("grLib_801C96F8");
    leaf_check::calls::check_calls_with(
        &dir,
        from,
        1,
        &|call, i| {
            let p = call.regs.gpr[1] - 0x4000;
            for (k, v) in [i as f32 * 3.0 - 10.0, 25.0, -1.5].iter().enumerate() {
                let o = (p + 4 * k as u32 - MEM1) as usize;
                call.mem1[o..o + 4].copy_from_slice(&v.to_bits().to_be_bytes());
            }
            // A stage scale other than 1 (the stage's parameters' first word).
            let param = word(call, leaf_check::sym("stage_info") + 0x6B0);
            let o = (param - MEM1) as usize;
            call.mem1[o..o + 4].copy_from_slice(&(1.0f32 + i as f32 / 8.0).to_bits().to_be_bytes());
            call.addr = at;
            call.regs.gpr[3] = 30000 + (i % 4) as u32;
            call.regs.gpr[4] = 0x1E;
            call.regs.gpr[5] = p;
        },
        &|r, _, call| {
            let g = &call.regs.gpr;
            let p = Vec3::new(
                float_at(call, g[5]),
                float_at(call, g[5] + 4),
                float_at(call, g[5] + 8),
            );
            Ret::Spawned(Spawned::from_generator(grlib::generator_at(
                r.game,
                r.data,
                g[3] as i32,
                g[4] as i32,
                p,
            )))
        },
    );
    for (name, which) in [
        ("grLib_801C9834", 0),
        ("grLib_801C9874", 1),
        ("grLib_801C98A0", 2),
        ("grLib_801C9908", 3),
    ] {
        let t = target(name);
        leaf_check::calls::check_calls_with(
            &dir,
            from,
            0,
            &|call, _| {
                let (g, j) = first_generator(call);
                call.addr = t;
                call.regs.gpr[3] = if which < 2 { g } else { j };
            },
            &|r, maps, call| {
                let a = call.regs.gpr[3];
                match which {
                    0 | 1 if a == 0 => panic!("removing a null generator isn't ported"),
                    0 => grlib::kill_generator(r.game, r.data, maps.generators[&a]),
                    1 => grlib::kill_generator_and_particles(r.game, r.data, maps.generators[&a]),
                    2 => grlib::kill_joint_generators_and_particles(
                        r.game,
                        r.data,
                        (a != 0).then(|| maps.jobjs[&a]),
                    ),
                    _ => grlib::kill_tree_generators_and_particles(
                        r.game,
                        r.data,
                        (a != 0).then(|| maps.jobjs[&a]),
                    ),
                }
                Ret::Nothing
            },
        );
    }
}

/// A GObj's stored alpha and effect id (`efLib_SetParamAlpha`, `efLib_SetParamGfxId`), for
/// the spawner and for no GObj, over the table's own entries.
#[test]
fn param_calls() {
    for func in ["efLib_SetParamAlpha", "efLib_SetParamGfxId"] {
        each_dir_rounds(
            "efSync_Spawn",
            func,
            2,
            &|call, i| {
                if i % 3 == 0 {
                    call.regs.gpr[4] = 0;
                }
                call.regs.gpr[3] = call.regs.gpr[4];
                call.regs.gpr[4] = 0x400 + 0x17 * i as u32;
            },
            &move |r, maps, call| {
                let g = maps.gobjs.get(&call.regs.gpr[3]).copied();
                let v = call.regs.gpr[4];
                if func == "efLib_SetParamAlpha" {
                    ef::set_param_alpha(r.game, g, v as u8);
                } else {
                    ef::set_param_gfx_id(r.game, g, v as i32);
                }
                Ret::Nothing
            },
        );
    }
}
