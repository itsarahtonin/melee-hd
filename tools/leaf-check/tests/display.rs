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

/// Marks the joint in r3 dirty on some calls, so setup recomputes its matrix.
fn dirty(call: &mut Call, i: usize) {
    if i % 3 == 1 {
        let j = call.regs.gpr[3];
        if j != 0 {
            let f = peek_u32(call, j + 0x14);
            poke_u32(call, j + 0x14, f | MTX_DIRTY);
        }
    }
}

/// `HSD_JObjDispAll` from every call: its transparency pass varied over the three and the
/// shadow render mode tried, its joint sometimes dirty.
#[test]
fn disp_all() {
    each(
        "HSD_JObjDispAll",
        20,
        &|call, i| {
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

/// `HSD_GObj_JObjCallback` in each of its passes.
#[test]
fn jobj_callback() {
    each(
        "HSD_GObj_JObjCallback",
        20,
        &|call, i| call.regs.gpr[4] = (i % 3) as u32,
        &|r, maps, call| {
            r.render_jobj(gobj_arg(maps, call), call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// The light GObjs' callback: their lights become current and take GX slots. The stage's
/// (`Ground_801C4640`) and the fighters' (`ftCo_8009F54C`) are the same code, run from the
/// same states.
#[test]
fn lobj_callback() {
    for func in ["HSD_GObj_LObjCallback", "Ground_801C4640", "ftCo_8009F54C"] {
        let addr = leaf_check::sym(func);
        each(
            "HSD_GObj_LObjCallback",
            1,
            &|call, _| call.addr = addr,
            &|r, maps, call| {
                r.render_lights(gobj_arg(maps, call));
                Ret::Nothing
            },
        );
    }
}

/// `HSD_LObjSetupInit` for the current lights.
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
            if i % 4 != 0 {
                let links: u64 = [1 << 0 | 1 << 4, 1 << 7 | 1 << 8, 1 << 10 | 1 << 0][i % 3];
                poke_u32(call, g + 0x20, (links >> 32) as u32);
                poke_u32(call, g + 0x24, links as u32);
            }
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
                    if i % 4 != 0 {
                        let links: u64 = [1 << 0 | 1 << 4, 1 << 10, 1 << 0 | 1 << 10][i % 3];
                        poke_u32(call, g + 0x20, (links >> 32) as u32);
                        poke_u32(call, g + 0x24, links as u32);
                    }
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
