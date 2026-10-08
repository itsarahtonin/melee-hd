//! The display pass's simulation side (`hsd::display`, `game::render`) against ssbm-rs, from
//! captures of a replay run's render pass: each check imports a capture, runs our port and
//! ssbm-rs's from the same state in both float modes, and compares the whole state, the
//! joints' matrices and dirty flags, the materials' texture matrices, the lights' slots and
//! the display statics among it. Some calls are changed first so rarer paths run from real
//! states: other transparency passes and render modes, dirty joints, the camera's links.
//!
//! The captures are game memory, kept out of the repository in `DISPLAY_CALLS` (default
//! below), one directory per replay. ssbm-rs's runner (branch `feat/state-trace-memory`)
//! makes them, with `done.txt` listing every function address but the captured ones:
//!
//! ```text
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$DISPLAY_CALLS/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_EVERY=2003 CAPTURE_FUNCS=HSD_JObjDispAll,HSD_JObjDisp,HSD_GObj_JObjCallback,\
//! HSD_GObj_803910D8,HSD_GObj_LObjCallback,Ground_801C4640,ftCo_8009F54C,Ground_801C1E2C,\
//! HSD_LObjSetupInit LOCKSTEP_DONE=done.txt ssbm-run "$SSBM_DISC" --replay NAME.slp \
//!   --fields 100000 --port all --lockstep --known known/NAME.txt
//! ```
//!
//! for the Fox vs Sheik Final Destination replay `Game_20260129T133652`.

use leaf_check::calls::{Ret, Setup, check_calls_in, gobj_arg, peek_u32, poke_u32};
use melee_sim::game::code::ParticleHooks;
use melee_sim::hsd::display;
use melee_sim::mtx::Mtx34;
use ssbm_rt::capture::Call;

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/display";
const CAPTURED: [&str; 1] = ["Game_20260129T133652"];

fn dirs() -> Vec<String> {
    let calls = std::env::var("DISPLAY_CALLS").unwrap_or_else(|_| CALLS.into());
    CAPTURED
        .iter()
        .map(|name| format!("{calls}/{name}"))
        .filter(|d| std::path::Path::new(d).exists())
        .collect()
}

/// Checks every saved call of `func` in each captured replay, changed by `prepare`.
fn each(
    func: &str,
    min: usize,
    prepare: &leaf_check::calls::Prepare,
    ours: &leaf_check::calls::Ours,
) {
    let dirs = dirs();
    if dirs.is_empty() {
        eprintln!("no display calls in {CALLS}: skipping (see this file's docs to make them)");
    }
    for d in dirs {
        let setup = Setup {
            gx: true,
            ..Setup::default()
        };
        check_calls_in(&d, func, min, prepare, &setup, ours);
    }
}

/// A matrix argument: null, or 12 floats in memory.
fn mtx_arg(call: &Call, a: u32) -> Option<Mtx34> {
    (a != 0).then(|| {
        let mut m = Mtx34::default();
        for (i, v) in m.m.iter_mut().flatten().enumerate() {
            *v = f32::from_bits(peek_u32(call, a + 4 * i as u32));
        }
        m
    })
}

/// `JOBJ_MTX_DIRTY` in a joint's flags.
const MTX_DIRTY: u32 = 1 << 6;

/// `JOBJ_PTCL`: the joint holds a particle list.
const PTCL: u32 = 1 << 5;

/// Changes every joint tree the GObjs hold on some calls, so the drawing's setup has work:
/// every joint dirty (skinned and enveloped shapes' joints included, which setup brings up
/// to date as the shapes draw), or every particle list's entries still to spawn.
fn dirty(call: &mut Call, i: usize) {
    let (all_dirty, spawn) = (i % 3 == 1, i % 5 == 2);
    if !all_dirty && !spawn {
        return;
    }
    let kind = call.mem1[(leaf_check::sym("HSD_GObj_JObjKind") - 0x8000_0000) as usize];
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut seen = std::collections::HashSet::new();
    for link in 0..64 {
        let mut g = peek_u32(call, heads + 4 * link);
        while g != 0 {
            let obj_kind = call.mem1[(g + 6 - 0x8000_0000) as usize];
            let mut stack = vec![];
            if obj_kind == kind {
                stack.push(peek_u32(call, g + 0x28));
            }
            while let Some(j) = stack.pop() {
                if j == 0 || !seen.insert(j) {
                    continue;
                }
                let f = peek_u32(call, j + 0x14);
                if all_dirty {
                    poke_u32(call, j + 0x14, f | MTX_DIRTY);
                }
                if spawn && f & PTCL != 0 {
                    let mut e = peek_u32(call, j + 0x18);
                    while e != 0 {
                        let w = peek_u32(call, e + 4);
                        poke_u32(call, e + 4, w | 0x8000_0000);
                        e = peek_u32(call, e);
                    }
                }
                stack.push(peek_u32(call, j + 8));
                stack.push(peek_u32(call, j + 0x10));
            }
            g = peek_u32(call, g + 8);
        }
    }
}

/// The GObjs on render link `link` whose callback is `cb`, in link order.
fn rendered_all(call: &Call, link: u32, cb: &str) -> Vec<u32> {
    let head = peek_u32(call, leaf_check::sym("HSD_GObjGXLinkHead"));
    let cb = leaf_check::sym(cb);
    let mut out = Vec::new();
    let mut g = peek_u32(call, head + 4 * link);
    while g != 0 {
        if peek_u32(call, g + 0x1C) == cb {
            out.push(g);
        }
        g = peek_u32(call, g + 0x10);
    }
    out
}

/// On some calls, draws another tree instead from the same state: a fighter's (skinned and
/// enveloped shapes) or a stage object's, in the shadow render mode, which leaves out the
/// fighters' and stages' own material classes that no port covers yet.
fn other_tree(call: &mut Call, i: usize) {
    let trees = match i % 6 {
        1 | 4 => rendered_all(call, 5, "ftDrawCommon_80080E18"),
        2 => rendered_all(call, 3, "grDisplay_801C5DB0"),
        _ => return,
    };
    let Some(&g) = trees.get(i / 6 % trees.len().max(1)) else {
        return;
    };
    call.regs.gpr[3] = peek_u32(call, g + 0x28);
    call.regs.gpr[4] = 0;
    call.regs.gpr[5] = [1, 2, 4][i / 6 % 3];
    call.regs.gpr[6] = display::RENDER_SHADOW;
}

/// `HSD_JObjDispAll` from every call: its transparency pass varied over the three and the
/// shadow render mode tried, its joint sometimes dirty.
#[test]
fn disp_all() {
    each(
        "HSD_JObjDispAll",
        20,
        &|call, i| {
            other_tree(call, i);
            dirty(call, i);
            if i % 4 == 3 {
                call.regs.gpr[5] = [1, 2, 4][i / 4 % 3];
            }
            if i % 7 == 5 {
                call.regs.gpr[6] |= display::RENDER_SHADOW;
            }
        },
        &|r, maps, call| {
            // A fighter's materials read the fighter being drawn.
            if let Some(g) = r.game.world.rendering
                && let Some(melee_sim::game::state::UserData::Fighter(f)) =
                    r.game.world.gobj(g).user_data
            {
                r.game.objs.display.fighter = Some(r.material_info(f));
            }
            let g = &call.regs.gpr;
            let j = (g[3] != 0).then(|| maps.jobjs[&g[3]]);
            display::jobj_disp_all(
                &mut r.game.objs,
                r.data,
                &mut ParticleHooks,
                j,
                mtx_arg(call, g[4]),
                g[5],
                g[6],
            );
            Ret::Nothing
        },
    );
}

/// `HSD_JObjDisp`: display objects, and joints' particle lists.
#[test]
fn disp() {
    each(
        "HSD_JObjDisp",
        20,
        &|call, i| {
            dirty(call, i);
            if i % 4 == 3 {
                call.regs.gpr[5] = [1, 2, 4][i / 4 % 3];
            }
        },
        &|r, maps, call| {
            let g = &call.regs.gpr;
            display::jobj_disp(
                &mut r.game.objs,
                r.data,
                &mut ParticleHooks,
                maps.jobjs[&g[3]],
                mtx_arg(call, g[4]),
                g[5],
                g[6],
            );
            Ret::Nothing
        },
    );
}

/// `HSD_GObj_JObjCallback` in each of its passes, on its own GObj or, on some calls, on an
/// effect's (render link 7), whose joints can hold particle lists.
#[test]
fn jobj_callback() {
    each(
        "HSD_GObj_JObjCallback",
        20,
        &|call, i| {
            call.regs.gpr[4] = (i % 3) as u32;
            if i / 3 % 2 == 1 {
                let effects = rendered_all(call, 7, "HSD_GObj_JObjCallback");
                if let Some(&g) = effects.get(i / 6 % effects.len().max(1)) {
                    call.regs.gpr[3] = g;
                }
            }
            dirty(call, i / 3);
        },
        &|r, maps, call| {
            r.render_jobj(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// The first GObj on render link `link` whose callback is `cb`.
fn rendered(call: &Call, link: u32, cb: &str) -> Option<u32> {
    let head = peek_u32(call, leaf_check::sym("HSD_GObjGXLinkHead"));
    let cb = leaf_check::sym(cb);
    let mut g = peek_u32(call, head + 4 * link);
    while g != 0 {
        if peek_u32(call, g + 0x1C) == cb {
            return Some(g);
        }
        g = peek_u32(call, g + 0x10);
    }
    None
}

/// Changes the lights of the light GObj `g` on some calls so every kind of light setup runs:
/// specular parts added, a light made specular only, point and spot lights.
fn vary_lights(call: &mut Call, g: u32, i: usize) {
    let mut l = peek_u32(call, g + 0x28);
    let mut k = 0;
    while l != 0 {
        let w = peek_u32(call, l + 8);
        let flags = (w >> 16) as u16;
        let flags = match (i % 5, flags & 3) {
            // Ambient lights stay ambient.
            (_, 0) | (0, _) => flags,
            (1, _) => flags | 8,
            (2, _) if k == 1 => flags & !4 | 8,
            (3, _) => flags & !3 | 2 | 8,
            (4, _) => flags & !3 | 3,
            _ => flags,
        };
        poke_u32(call, l + 8, u32::from(flags) << 16 | w & 0xFFFF);
        l = peek_u32(call, l + 0xC);
        k += 1;
    }
}

/// The light GObjs' callback: their lights become current and take GX slots. Run from the
/// HUD's light callbacks' states on the HUD's lights (`HSD_GObj_LObjCallback`), the stage's
/// (`Ground_801C4640`, render link 0) and the fighters' (`ftCo_8009F54C`, link 4), the same
/// code, with the lights varied.
#[test]
fn lobj_callback() {
    for (func, link) in [
        ("HSD_GObj_LObjCallback", None),
        ("Ground_801C4640", Some(0)),
        ("ftCo_8009F54C", Some(4)),
    ] {
        let addr = leaf_check::sym(func);
        each(
            "HSD_GObj_LObjCallback",
            1,
            &|call, i| {
                call.addr = addr;
                if let Some(link) = link
                    && let Some(g) = rendered(call, link, func)
                {
                    call.regs.gpr[3] = g;
                }
                let g = call.regs.gpr[3];
                vary_lights(call, g, i);
            },
            &|r, maps, call| {
                r.render_lights(gobj_arg(maps, call));
                Ret::Nothing
            },
        );
    }
}

/// `HSD_LObjSetupInit` for the current lights: its saved calls (the HUD's ambient light), and
/// the stage's lights made current by their callback, varied.
#[test]
fn lobj_setup_init() {
    each("HSD_LObjSetupInit", 1, &|_, _| {}, &|r, maps, call| {
        let c = maps.cobjs[&call.regs.gpr[3]];
        display::lobj_setup_init(&mut r.game.objs, r.data, c);
        Ret::Nothing
    });
}

/// The stage's fog GObj's callback.
#[test]
fn stage_fog() {
    each("Ground_801C1E2C", 1, &|_, _| {}, &|r, maps, call| {
        r.render_stage_fog(gobj_arg(maps, call));
        Ret::Nothing
    });
}

/// The cameras with the generic callback (`HSD_GObj_803910D8`) on the max render link.
fn generic_cameras(call: &Call) -> Vec<u32> {
    let head = peek_u32(call, leaf_check::sym("HSD_GObjGXLinkHead"));
    let mut g = peek_u32(call, head + 4 * 64);
    let mut out = Vec::new();
    while g != 0 {
        if peek_u32(call, g + 0x1C) == leaf_check::sym("HSD_GObj_803910D8") {
            out.push(g);
        }
        g = peek_u32(call, g + 0x10);
    }
    out
}

/// `CObj` flags: the view matrix needs rebuilding.
const VIEW_DIRTY: u32 = 1 << 30;

/// Changes a camera GObj `g` on most calls: the render links its passes run (the stage's
/// lights and fog with the fighters' lights, the particles, the light GObjs, the joint tree
/// on link 15) and, on some, its view matrix left to rebuild.
fn vary_camera(call: &mut Call, g: u32, i: usize) {
    if !i.is_multiple_of(5) {
        let links: u64 = [
            1 << 0 | 1 << 4,
            1 << 7 | 1 << 8,
            1 << 10 | 1 << 0,
            1 << 15,
            1 << 15 | 1 << 10,
        ][i % 5];
        poke_u32(call, g + 0x20, (links >> 32) as u32);
        poke_u32(call, g + 0x24, links as u32);
    }
    if i % 2 == 1 {
        let c = peek_u32(call, g + 0x28);
        if c != 0 {
            let f = peek_u32(call, c + 8);
            poke_u32(call, c + 8, f | VIEW_DIRTY);
        }
    }
}

/// A camera GObj's callback, `HSD_GObj_803910D8`, run from the light callbacks' states (mid
/// pass) on each camera that has it, over the render links its passes run: its own (the
/// HUD's, which no port covers yet) and, on most calls, the stage's lights and fog, the
/// fighters' lights, the particles and the light GObjs instead.
#[test]
fn camera_callback() {
    each(
        "HSD_GObj_LObjCallback",
        1,
        &|call, i| {
            let cams = generic_cameras(call);
            let Some(&g) = cams.get(i % cams.len().max(1)) else {
                return;
            };
            call.addr = leaf_check::sym("HSD_GObj_803910D8");
            call.regs.gpr[3] = g;
            vary_camera(call, g, i);
        },
        &|r, maps, call| {
            if call.addr == leaf_check::sym("HSD_GObj_803910D8") {
                r.render_camera(gobj_arg(maps, call));
            } else {
                r.render_lights(gobj_arg(maps, call));
            }
            Ret::Nothing
        },
    );
}

/// The frame's display pass, `HSD_GObj_80390FC0`, from the light callbacks' states: the
/// cameras on the max render link whose callbacks no port covers yet (the game camera's and
/// the HUD's) are given none, so the generic ones run, over the links of
/// [`camera_callback`].
#[test]
fn frame() {
    each(
        "HSD_GObj_LObjCallback",
        1,
        &|call, i| {
            call.addr = leaf_check::sym("HSD_GObj_80390FC0");
            let generic = generic_cameras(call);
            let head = peek_u32(call, leaf_check::sym("HSD_GObjGXLinkHead"));
            let mut g = peek_u32(call, head + 4 * 64);
            while g != 0 {
                if generic.contains(&g) {
                    vary_camera(call, g, i);
                } else {
                    poke_u32(call, g + 0x1C, 0);
                }
                g = peek_u32(call, g + 0x10);
            }
        },
        &|r, _, _| {
            r.render_frame();
            Ret::Nothing
        },
    );
}

/// The generic fog callback, `HSD_GObj_FogCallback`, on the stage's fog GObj.
#[test]
fn fog_callback() {
    each(
        "Ground_801C1E2C",
        1,
        &|call, _| call.addr = leaf_check::sym("HSD_GObj_FogCallback"),
        &|r, maps, call| {
            r.render_fog(gobj_arg(maps, call));
            Ret::Nothing
        },
    );
}

/// `lbRefract_800222A4` against the state it left: from each light callback's capture,
/// refraction's chains are forgotten and made again from `LbRf.dat`, and the result compared
/// with memory (the reference can't run it: it reads the disc). The capture's users and
/// projection stay as they were.
#[test]
fn refract_init() {
    use melee_capture::compare::Comparer;
    use melee_capture::import::Importer;
    use melee_capture::{Capture, DataMap, find_archives_in};
    use melee_data::archive::Archives;
    let Some(index) = leaf_check::disc_index() else {
        return;
    };
    let mut checked = 0;
    for d in dirs() {
        for path in leaf_check::calls::saved_calls(&d, "HSD_GObj_LObjCallback") {
            let cap = Capture::read(&path);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
            };
            let lb_rf = data
                .loaded
                .iter()
                .find(|l| l.name == "LbRf.dat")
                .expect("LbRf.dat is loaded")
                .id;
            let mut imp = Importer::new(&cap, &data).with_archives(&archives);
            imp.world();
            imp.display();
            // The world's own gaps (fighters' material classes) are other ports'.
            let gaps: Vec<_> = imp
                .unhandled
                .iter()
                .filter(|u| u.contains("refract"))
                .collect();
            assert!(gaps.is_empty(), "{gaps:?}");
            let Importer {
                mut game, mut maps, ..
            } = imp;
            // Each chain is the indirect map then the screen copy.
            for &t in &game.refract.tobjs {
                let next = game.objs.tobjs[t].next.expect("the screen copy");
                assert!(matches!(
                    (
                        game.objs.tobjs[t].imagedesc,
                        game.objs.tobjs[next].imagedesc
                    ),
                    (
                        Some(melee_sim::hsd::tobj::ImageRef::Refract(_)),
                        Some(melee_sim::hsd::tobj::ImageRef::RefractFrame)
                    )
                ));
            }
            let mut chain = game.refract.tobjs.clone();
            while let Some(t) = chain.pop() {
                maps.tobjs.retain(|_, v| *v != t);
                chain.extend(game.objs.tobjs[t].next);
            }
            let users = game.refract.users;
            game.refract.tobjs.clear();
            game.refract.data = None;
            game.refract_init(&archives, lb_rf);
            game.refract.users = users;
            let mut cmp = Comparer::new(&cap.call.mem1, &data, &game, maps);
            cmp.refract();
            assert!(cmp.diffs.is_empty(), "{}: {:?}", path.display(), cmp.diffs);
            checked += 1;
        }
    }
    assert!(checked > 0 || dirs().is_empty());
}

/// Refraction's users counted up (`lbRefract_80022BB8`) and down (`lbRefSetUnuse`), from
/// the light callbacks' states with one to three users.
#[test]
fn refract_users() {
    let users = leaf_check::sym("lbl_804336D0");
    for (func, up) in [("lbRefract_80022BB8", true), ("lbRefSetUnuse", false)] {
        let addr = leaf_check::sym(func);
        each(
            "HSD_GObj_LObjCallback",
            1,
            &|call, i| {
                call.addr = addr;
                poke_u32(call, users, 1 + (i % 3) as u32);
            },
            &|r, _, _| {
                if up {
                    r.game.refract_use();
                } else {
                    r.game.refract_unuse();
                }
                Ret::Nothing
            },
        );
    }
}

/// The screen copy's projection for each camera (`lbRefract_8002247C`), with and without
/// users, and the copy itself (`lbRefract_80022560`), from the light callbacks' states.
#[test]
fn refract_camera() {
    let users = leaf_check::sym("lbl_804336D0");
    let set = leaf_check::sym("lbRefract_8002247C");
    each(
        "HSD_GObj_LObjCallback",
        1,
        &|call, i| {
            let head = peek_u32(call, leaf_check::sym("HSD_GObjGXLinkHead"));
            let mut cams = Vec::new();
            let mut g = peek_u32(call, head + 4 * 64);
            while g != 0 {
                cams.push(peek_u32(call, g + 0x28));
                g = peek_u32(call, g + 0x10);
            }
            call.addr = set;
            call.regs.gpr[3] = cams[i % cams.len()];
            poke_u32(call, users, (i % 4 != 0) as u32);
        },
        &|r, maps, call| {
            let c = maps.cobjs[&call.regs.gpr[3]];
            melee_sim::lb::refract::set_camera(&r.game.objs, &mut r.game.refract, c);
            Ret::Nothing
        },
    );
    let copy = leaf_check::sym("lbRefract_80022560");
    each(
        "HSD_GObj_LObjCallback",
        1,
        &|call, i| {
            call.addr = copy;
            poke_u32(call, users, (i % 2) as u32);
        },
        &|r, _, _| {
            melee_sim::lb::refract::copy_screen(&mut r.game.objs, &r.game.refract);
            Ret::Nothing
        },
    );
}

/// The particles' render callback in the drawing passes (`efLib_render_callback` with
/// `psDispParticles`' app-SRT updates), from the effects captures' particle process states,
/// their app-SRTs' display frames moved on so the update runs.
#[test]
fn particle_render() {
    let root = std::env::var("EFFECT_CALLS")
        .unwrap_or_else(|_| "C:/Projects/ssbm-rs-publication/local/calls/effects".into());
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let render = leaf_check::sym("efLib_render_callback");
    let frame = leaf_check::sym("psFrameNum");
    for d in entries.flatten().filter(|e| e.path().is_dir()) {
        let d = d.path().to_string_lossy().into_owned();
        for from in ["efLib_particles_proc_main", "efLib_particles_proc_aux"] {
            let setup = Setup {
                gx: true,
                ..Setup::default()
            };
            check_calls_in(
                &d,
                from,
                0,
                &|call, i| {
                    let gobjs = rendered_all(call, 7, "efLib_render_callback");
                    let gobjs = if i % 2 == 0 {
                        gobjs
                    } else {
                        rendered_all(call, 8, "efLib_render_callback")
                    };
                    if let Some(&g) = gobjs.first() {
                        call.addr = render;
                        call.regs.gpr[3] = g;
                        call.regs.gpr[4] = 1 + (i / 2 % 2) as u32;
                        // A new display frame: the app-SRTs update.
                        let at = (frame - 0x8000_0000) as usize;
                        call.mem1[at] = call.mem1[at].wrapping_add(1).max(1);
                    }
                },
                &setup,
                &|r, maps, call| {
                    if call.addr == render {
                        r.render_particles(gobj_arg(maps, call), call.regs.gpr[4] as i32);
                    }
                    Ret::Nothing
                },
            );
        }
    }
}

const CALLS2: &str = "C:/Projects/ssbm-rs-publication/local/calls/display2";

/// The second capture set's directories (`DISPLAY2_CALLS`): the game camera's, stages',
/// fighters', the magnifier's and the text's render callbacks.
fn dirs2() -> Vec<String> {
    let calls = std::env::var("DISPLAY2_CALLS").unwrap_or_else(|_| CALLS2.into());
    CAPTURED
        .iter()
        .map(|name| format!("{calls}/{name}"))
        .filter(|d| std::path::Path::new(d).exists())
        .collect()
}

/// [`each`] over the second capture set.
fn each2(
    func: &str,
    min: usize,
    prepare: &leaf_check::calls::Prepare,
    ours: &leaf_check::calls::Ours,
) {
    for d in dirs2() {
        let setup = Setup {
            gx: true,
            ..Setup::default()
        };
        check_calls_in(&d, func, min, prepare, &setup, ours);
    }
}

/// A stage object's render callback in each pass, its joints sometimes dirty.
#[test]
fn stage_object() {
    each2(
        "grDisplay_801C5DB0",
        20,
        &|call, i| {
            call.regs.gpr[4] = (i % 3) as u32;
            dirty(call, i / 3);
        },
        &|r, maps, call| {
            r.render_stage_object(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// A fighter's render callback in each pass, its joints sometimes dirty.
#[test]
fn fighter() {
    each2(
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            call.regs.gpr[4] = (i % 3) as u32;
            dirty(call, i / 3);
        },
        &|r, maps, call| {
            r.render_fighter(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// The fighters' shadows (`lbShadow_8000F38C`), from its saved call and from the fighters'
/// render callbacks' states, with and without the camera's debug flag.
#[test]
fn shadows() {
    let shadow = leaf_check::sym("lbShadow_8000F38C");
    for from in ["lbShadow_8000F38C", "ftDrawCommon_80080E18"] {
        each2(
            from,
            1,
            &|call, i| {
                call.addr = shadow;
                call.regs.gpr[3] = (i % 2) as u32;
                dirty(call, i);
            },
            &|r, _, call| {
                r.shadow_render(call.regs.gpr[3] != 0);
                Ret::Nothing
            },
        );
    }
}

/// The game camera's render callback (`fn_800301D0`), from the stage objects' render
/// states, with its layers sometimes hidden.
#[test]
fn game_camera() {
    let cb = leaf_check::sym("fn_800301D0");
    each2(
        "grDisplay_801C5DB0",
        10,
        &|call, i| {
            let cams = rendered_all(call, 64, "fn_800301D0");
            if let Some(&g) = cams.first() {
                call.addr = cb;
                call.regs.gpr[3] = g;
                call.regs.gpr[4] = 0;
                if i % 3 == 1 {
                    // `game_camera.x398`: some layers hidden.
                    let at = leaf_check::sym("game_camera") + 0x398;
                    call.mem1[(at - 0x8000_0000) as usize] ^= [0x20, 0x08, 0x80][i / 3 % 3];
                }
            }
        },
        &|r, maps, call| {
            if call.addr == cb {
                r.render_game_camera(gobj_arg(maps, call));
            }
            Ret::Nothing
        },
    );
}

/// `lbl_80433658`, the background flash's state.
fn flash_state() -> u32 {
    leaf_check::sym("lbl_80433658")
}

/// Sets the flash's mode (and turns it on), its colors, fade and wipe for variant `v`.
fn flash_variant(call: &mut Call, v: usize) {
    let d = flash_state();
    let byte = |call: &mut Call, a: u32, b: u8| call.mem1[(a - 0x8000_0000) as usize] = b;
    let (mode, x10, x20, x8) = match v {
        // A fade up and down, some channels at their targets or past them.
        0 => (
            0,
            [10.5, 250.0, 3.0, 128.25],
            [3.3, 10.0, -2.0, -200.0],
            [20, 255, 0, 0],
        ),
        1 => (
            2,
            [0.0, 0.0, 0.0, 0.0],
            [0.0042, 255.0, -1.0, 0.5],
            [1, 255, 0, 255],
        ),
        2 => (
            1,
            [254.9, 1.0, 100.0, 7.0],
            [0.2, -0.75, 0.0, -7.0],
            [255, 0, 100, 0],
        ),
        // Wipes, and the hold.
        3 | 4 => (v as u8, [0.0; 4], [0.0; 4], [0; 4]),
        _ => (5, [0.0; 4], [0.0; 4], [0; 4]),
    };
    byte(call, d, mode);
    for i in 0..4u32 {
        leaf_check::calls::poke_f32(call, d + 0x10 + 4 * i, x10[i as usize]);
        leaf_check::calls::poke_f32(call, d + 0x20 + 4 * i, x20[i as usize]);
        byte(call, d + 8 + i, x8[i as usize]);
        byte(call, d + 0xC + i, 0x40 + i as u8 * 0x31);
    }
    // The wipe: by 40 across, strips 16 high; near its start, 5 steps a frame, or near its
    // end, 25 steps, which finish its last row and the wipe.
    let (x30, x34, x38, steps) = if v == 4 {
        (0, 600, 470, 25)
    } else {
        (0, 80, 32, 5)
    };
    for (o, b) in [(0x30, x30), (0x31, 40), (0x32, 16), (0x33, 0)] {
        byte(call, d + o, b);
    }
    poke_u32(call, d + 0x34, x34);
    poke_u32(call, d + 0x38, x38);
    poke_u32(call, d + 0x3C, steps);
}

/// The flash GObj's process (`fn_800204C8`) in each mode, from the fighters' render states.
#[test]
fn bg_flash_proc() {
    let f = leaf_check::sym("fn_800204C8");
    each2(
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            call.addr = f;
            if i % 7 != 6 {
                flash_variant(call, i % 7);
            }
        },
        &|r, _, _| {
            r.game.bgflash.proc();
            Ret::Nothing
        },
    );
}

/// The flash's render callback (`fn_8001FEC4`), off and on in each mode.
#[test]
fn bg_flash_render() {
    each2(
        "fn_8001FEC4",
        5,
        &|call, i| {
            if i % 7 != 6 {
                flash_variant(call, i % 7);
            }
        },
        &|r, _, _| {
            r.render_bg_flash();
            Ret::Nothing
        },
    );
}

/// The overlay GObj's process (`lbBgFlash_Proc`), with `LbBf.dat`'s animations started on
/// it as `lbBgFlash_80021C48` starts them, scaled and with alpha.
#[test]
fn bg_flash_overlay() {
    let f = leaf_check::sym("lbBgFlash_Proc");
    each2(
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            let g = peek_u32(call, leaf_check::sym("flash_gobj"));
            call.addr = f;
            call.regs.gpr[3] = g;
            let ud = peek_u32(call, g + 0x2C);
            let co = ud + 4;
            let id = (i % 16) as u32;
            if i % 4 != 3 {
                let table = peek_u32(call, leaf_check::sym("lbl_804D63DC"));
                let script = peek_u32(call, table + 8 * id);
                poke_u32(call, co + 0x28, id);
                poke_u32(call, co + 4, (i % 3) as u32);
                poke_u32(call, co + 8, script);
                poke_u32(call, co, 0);
                poke_u32(call, co + 0xC, 0);
                call.mem1[(co + 0x7C - 0x8000_0000) as usize] &= 0x3F;
            } else {
                // An overlay that colored and stopped.
                call.mem1[(co + 0x7C - 0x8000_0000) as usize] |= 0x80;
            }
            call.mem1[(ud - 0x8000_0000) as usize] = [0xFF, 0x80, 0x01][i % 3];
            let scale = [1.0, 0.2, 1.7][i / 3 % 3];
            leaf_check::calls::poke_f32(call, leaf_check::sym("flash_scale"), scale);
        },
        &|r, _, _| {
            r.bg_flash_proc();
            Ret::Nothing
        },
    );
}

/// `lbBgFlash_80021C48` with each animation over what the overlay plays.
#[test]
fn bg_flash_anim() {
    let f = leaf_check::sym("lbBgFlash_80021C48");
    each2(
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            call.addr = f;
            call.regs.gpr[3] = (i % 16) as u32;
            call.regs.gpr[4] = (i % 5) as u32 * 7;
            if i % 2 == 1 {
                // Something else playing.
                let g = peek_u32(call, leaf_check::sym("flash_gobj"));
                let co = peek_u32(call, g + 0x2C) + 4;
                poke_u32(call, co + 0x28, (i / 2 % 16) as u32);
            }
        },
        &|r, _, call| {
            let (id, frames) = (call.regs.gpr[3], call.regs.gpr[4]);
            let started = r.game.bg_flash_anim(r.data, id, frames);
            Ret::Int(u32::from(started))
        },
    );
}

/// The flash's fades and holds (`lbBgFlash_800205F0` and the rest), on a flash in each
/// mode.
#[test]
fn bg_flash_setters() {
    let names = [
        "lbBgFlash_800205F0",
        "lbBgFlash_8002063C",
        "lbBgFlash_80020688",
        "lbBgFlash_800206D4",
        "lbBgFlash_InitState",
        "fn_800208B0",
        "lbBgFlash_SetFlashScale",
    ];
    let funcs = names.map(leaf_check::sym);
    let (c1, c2) = (
        leaf_check::sym("lbl_804D3840"),
        leaf_check::sym("lbl_804D3844"),
    );
    each2(
        "ftDrawCommon_80080E18",
        10,
        &|call, i| {
            flash_variant(call, i / 7 % 7);
            call.addr = funcs[i % 7];
            let n = [0, 1, 7, 60, -3, 255][i / 7 % 6] as u32;
            call.regs.gpr[3] = n;
            if i % 7 == 3 || i % 7 == 4 {
                poke_u32(call, c1, 0x10FF_7F00u32.rotate_left(i as u32));
                poke_u32(call, c2, 0x00C0_FF33u32.rotate_right(i as u32));
                call.regs.gpr[3] = c1;
                call.regs.gpr[4] = c2;
                call.regs.gpr[5] = n;
            }
            call.regs.fpr[1].ps0 = [0.2, 1.0, 3.5][i % 3];
        },
        &|r, _, call| {
            let f = &mut r.game.bgflash;
            let n = call.regs.gpr[3] as i32;
            let color = |a: u32| {
                let w = peek_u32(call, a).to_be_bytes();
                melee_sim::game::structs::_GXColor {
                    r: w[0],
                    g: w[1],
                    b: w[2],
                    a: w[3],
                }
            };
            match funcs.iter().position(|&a| a == call.addr) {
                Some(0) => f.fade_white_to_black(n),
                Some(1) => f.fade_to_black(n),
                Some(2) => f.fade_to_white(n),
                Some(3) => f.fade(
                    color(call.regs.gpr[3]),
                    color(call.regs.gpr[4]),
                    call.regs.gpr[5] as i32,
                ),
                Some(4) => f.set_color(color(call.regs.gpr[3])),
                Some(5) => f.set_black(n as u8),
                _ => f.set_scale(call.regs.fpr[1].ps0 as f32),
            }
            Ret::Nothing
        },
    );
}

/// The HUD's hidden flag (`ifAll_IsHUDHidden`'s).
const HUD_HIDDEN: u32 = 0x804D_6D58;

/// The magnifier camera's render callback (`ifMagnify_802FBBDC`), from its saved calls and
/// the fighters' render states, with players ignored or the HUD hidden.
#[test]
fn magnifier() {
    let f = leaf_check::sym("ifMagnify_802FBBDC");
    for from in ["ifMagnify_802FBBDC", "ftDrawCommon_80080E18"] {
        each2(
            from,
            3,
            &|call, i| {
                if call.addr != f {
                    let cams = rendered_all(call, 64, "ifMagnify_802FBBDC");
                    let Some(&g) = cams.first() else {
                        return;
                    };
                    call.addr = f;
                    call.regs.gpr[3] = g;
                    call.regs.gpr[4] = 0;
                }
                let players = leaf_check::sym("ifMagnify_804A1DE0") + 0x14;
                match i % 4 {
                    // A player the magnifier ignores.
                    1 => {
                        let at = players + 0x10 * (i as u32 / 4 % 2) + 0xC;
                        call.mem1[(at - 0x8000_0000) as usize] |= 0x40;
                    }
                    // The HUD hidden (`ifAll_IsHUDHidden`'s flag).
                    2 => call.mem1[(HUD_HIDDEN - 0x8000_0000) as usize] = 1,
                    // The fighters off screen (`x221F_b0`), the second one's shaking.
                    3 => {
                        for (k, g) in rendered_all(call, 5, "ftDrawCommon_80080E18")
                            .into_iter()
                            .enumerate()
                        {
                            let fp = peek_u32(call, g + 0x2C);
                            call.mem1[(fp + 0x221F - 0x8000_0000) as usize] |= 0x80;
                            if k == 1 {
                                leaf_check::calls::poke_f32(call, fp + 0x18B8, 1.5);
                            }
                        }
                    }
                    _ => {}
                }
            },
            &|r, maps, call| {
                if call.addr == f {
                    r.render_magnifier(gobj_arg(maps, call));
                }
                Ret::Nothing
            },
        );
    }
}

/// A player's bubble's render callback (`ifMagnify_802FB8C0`) in each pass.
#[test]
fn bubble() {
    each2(
        "ifMagnify_802FB8C0",
        10,
        &|call, i| {
            call.regs.gpr[4] = [0, 0, 1, 2][i % 4];
            if i % 2 == 0 {
                // The player off screen.
                let ud = peek_u32(call, call.regs.gpr[3] + 0x2C);
                call.mem1[(ud + 0xC - 0x8000_0000) as usize] |= 0x80;
            }
        },
        &|r, maps, call| {
            r.render_bubble(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}
