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
    if i % 5 != 0 {
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
