//! The HUD and the debug text (`melee/if`, and the HSD text system it uses) against
//! ssbm-rs's ports, run from real game states (see `support/calls.rs`): each check imports a
//! captured match, runs the same call on both sides, and compares the whole state, the HUD's
//! globals included, and what was drawn.
//!
//! Besides the VS match's captures (`vs.rs`), the HUD's own are in `HUD_CALLS`, made with
//! ssbm-rs's runner in the debug VS mode's random matches, every HUD function named and every
//! other port left unchecked (`local/calls/hud/capture.sh` in ssbm-rs's checkout):
//!
//! ```text
//! MATCH_STAGES=31,32 MATCH_FIGHTERS=2,19,20,9 MATCH_TIME=30 MATCH_PLAYERS=2 \
//!   CAPTURE=$HUD_CALLS/hvs1 CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_FUNCS=@hud-funcs.txt \
//!   LOCKSTEP_DONE=done.txt ssbm-run "$SSBM_DISC" --mode e --matches 1 --monkey 1 \
//!   --fields 6000 --port all --lockstep
//! ```
//!
//! where `hud-funcs.txt` lists the addresses of `melee/if`'s and the text system's ports and
//! `done.txt` every other port's.

#[path = "support/calls.rs"]
mod calls;

use calls::{Check, Logged, Out, log};
use leaf_check::{Rand, SCRATCH, handle};
use melee_capture::import::sym;
use melee_sim::game::hud::devtext::{self, Draw, Link, POOL};
use melee_sim::game::state::Running;
use ssbm_rt::Ctx;
use ssbm_types::fns;

// The debug text.

#[test]
fn scene_text() {
    Check::new("fn_8016C7D0", &|r, _| {
        r.game.setup_scene_text(r.data);
        vec![]
    })
    .with_theirs(&|ctx, _| {
        fns::un_802FF78C(ctx);
        (0, 0)
    })
    .run();
}

/// An operation on the debug text screens, on screens by creation order.
#[derive(Clone, Copy, Debug)]
enum Op {
    Create {
        id: i8,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    Show(usize),
    /// `DevText_Remove` on the draw list, or on its first screen's `next`.
    Remove(bool),
    SetCursorXY(usize, i32, i32),
    SetCursorX(usize, i32),
    Print(usize, usize),
    PrintInt(usize, i32),
    SetScale(usize, f32, f32),
    SetXY(usize, i32, i32),
    StoreColor(usize, u8),
    SetTextColor(usize, [u8; 4]),
    SetBgColor(usize, [u8; 4]),
    Flags(usize, u8),
    Erase(usize),
    EraseFirstLine(usize),
    /// Drawn this many times in a row.
    Draw(usize, u8),
}

const STRINGS: [&[u8]; 4] = [
    b"FPS 60\0",
    b"a\nbc\n\nlong line of text\0",
    b"\0",
    b"0123456789abcdef\0",
];

fn ops(seed: u32) -> Vec<Op> {
    let mut r = Rand(u64::from(seed) | 1);
    let mut out = Vec::new();
    let mut made = 0;
    for _ in 0..60 {
        let k = r.below(16);
        if made == 0 || k == 0 || (k == 1 && made < 6) {
            out.push(Op::Create {
                id: [0, 1, 5, 5, -3, 100][r.below(6) as usize],
                x: r.below(600) as i32 - 20,
                y: r.below(400) as i32,
                w: 1 + r.below(40) as i32,
                h: 1 + r.below(12) as i32,
            });
            made += 1;
            continue;
        }
        let t = r.below(made) as usize;
        let n = |r: &mut Rand| r.below(60) as i32 - 10;
        out.push(match k {
            1 | 2 => Op::Show(t),
            3 => Op::Remove(r.below(3) == 0),
            4 => Op::SetCursorXY(t, n(&mut r), n(&mut r)),
            5 => Op::SetCursorX(t, n(&mut r)),
            6 => Op::Print(t, r.below(STRINGS.len() as u64) as usize),
            7 => Op::PrintInt(t, [0, -1, 7, 123_456, i32::MIN][r.below(5) as usize]),
            8 => Op::SetScale(t, r.range(1.0, 20.0), r.range(1.0, 20.0)),
            9 => Op::SetXY(t, n(&mut r), n(&mut r)),
            10 => Op::StoreColor(t, r.below(4) as u8),
            11 => Op::SetTextColor(t, (r.bits() as u32).to_be_bytes()),
            12 => Op::SetBgColor(t, (r.bits() as u32).to_be_bytes()),
            13 => Op::Flags(t, r.below(256) as u8),
            14 => {
                if r.below(2) == 0 {
                    Op::Erase(t)
                } else {
                    Op::EraseFirstLine(t)
                }
            }
            _ => Op::Draw(t, 1 + r.below(24) as u8),
        });
    }
    out
}

/// Ours: the screens made, in creation order (a failed creation's slot holds none).
fn run_ours(r: &mut Running, ops: &[Op]) -> Vec<Logged> {
    let d = &mut r.game.hud.devtext;
    let mut made: Vec<Option<u8>> = Vec::new();
    let mut drawn = Vec::new();
    for &op in ops {
        let text = |i: usize| made.get(i).copied().flatten();
        // The draw list as the original walks it; a cycle (the original's lists can make
        // one) would hang both sides, so the operations stop there.
        let mut shown = Vec::new();
        let mut cur = d.draw_list;
        while let Some(c) = cur {
            if shown.len() > POOL {
                return drawn;
            }
            shown.push(c);
            cur = d.pool[usize::from(c)].next;
        }
        match op {
            Op::Create { id, x, y, w, h } => made.push(d.create(id, x, y, w, h)),
            Op::Show(i) => {
                if let Some(t) = text(i).filter(|t| !shown.contains(t)) {
                    d.show(t);
                }
            }
            Op::Remove(next) => {
                let link = match (next, d.draw_list) {
                    (false, Some(_)) => Link::DrawList,
                    (true, Some(h)) if d.pool[usize::from(h)].next.is_some() => Link::NextOf(h),
                    _ => continue,
                };
                d.remove(link);
            }
            Op::Draw(i, n) => {
                if let Some(t) = text(i) {
                    for _ in 0..n {
                        let mut out = Vec::new();
                        d.pool[usize::from(t)].draw(&mut out);
                        drawn.extend(out.iter().map(logged));
                    }
                }
            }
            op => {
                let Some(t) = text(op_target(op)) else {
                    continue;
                };
                let s = &mut d.pool[usize::from(t)];
                match op {
                    Op::SetCursorXY(_, x, y) => s.set_cursor_xy(x, y),
                    Op::SetCursorX(_, x) => s.set_cursor_x(x),
                    Op::Print(_, k) => s.print(STRINGS[k]),
                    Op::PrintInt(_, v) => s.print_int(v),
                    Op::SetScale(_, x, y) => s.set_scale(x, y),
                    Op::SetXY(_, x, y) => s.set_xy(x, y),
                    Op::StoreColor(_, c) => {
                        s.store_color_index(c);
                    }
                    Op::SetTextColor(_, c) => {
                        s.set_text_color(c);
                    }
                    Op::SetBgColor(_, c) => {
                        s.set_bg_color(c);
                    }
                    Op::Flags(_, f) => {
                        for (bit, on, off) in [
                            (0x10, devtext::DevText::hide_cursor as fn(&mut _), None),
                            (0x20, devtext::DevText::no_wrap, None),
                            (
                                0x40,
                                devtext::DevText::hide_background,
                                Some(devtext::DevText::show_background as fn(&mut _)),
                            ),
                            (
                                0x80,
                                devtext::DevText::hide_text,
                                Some(devtext::DevText::show_text),
                            ),
                        ] {
                            if f & bit != 0 {
                                on(s);
                            } else if let Some(off) = off {
                                off(s);
                            }
                        }
                    }
                    Op::Erase(_) => s.erase(),
                    Op::EraseFirstLine(_) => s.erase_first_line(),
                    _ => unreachable!(),
                }
            }
        }
    }
    drawn
}

fn op_target(op: Op) -> usize {
    match op {
        Op::SetCursorXY(i, ..)
        | Op::SetCursorX(i, _)
        | Op::Print(i, _)
        | Op::PrintInt(i, _)
        | Op::SetScale(i, ..)
        | Op::SetXY(i, ..)
        | Op::StoreColor(i, _)
        | Op::SetTextColor(i, _)
        | Op::SetBgColor(i, _)
        | Op::Flags(i, _)
        | Op::Erase(i)
        | Op::EraseFirstLine(i)
        | Op::Show(i)
        | Op::Draw(i, _) => i,
        Op::Create { .. } | Op::Remove(_) => unreachable!(),
    }
}

/// A draw command as ssbm-rs's loggers record it.
fn logged(d: &Draw) -> Logged {
    let color = |c: [u8; 4]| u32::from_be_bytes(c);
    match *d {
        Draw::TextScale { x, y, line_width } => Logged {
            addr: sym("hsd_80391A04"),
            gpr: vec![line_width as u32],
            fpr: vec![x.to_bits(), y.to_bits()],
        },
        Draw::Rect {
            x,
            y,
            w,
            h,
            color: c,
        } => Logged {
            addr: sym("DrawRectangle"),
            gpr: vec![color(c)],
            fpr: vec![x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits()],
        },
        Draw::Ascii {
            chr,
            x,
            y,
            color: c,
        } => Logged {
            addr: sym("DrawASCII"),
            gpr: vec![chr as u32, color(c)],
            fpr: vec![x.to_bits(), y.to_bits()],
        },
        Draw::NoFog => Logged {
            addr: sym("HSD_FogSet"),
            gpr: vec![0],
            fpr: vec![],
        },
        // The screens' camera, which their logger names as devtext_cobj.
        Draw::Camera(_) => Logged {
            addr: sym("HSD_CObjSetCurrent"),
            gpr: vec![1],
            fpr: vec![],
        },
    }
}

fn read_color(ctx: &Ctx, a: u32) -> u32 {
    ctx.read_u32(a)
}

fn log_text_scale(ctx: &Ctx) {
    let r = &ctx.regs;
    log(Logged {
        addr: sym("hsd_80391A04"),
        gpr: vec![r.r(3)],
        fpr: vec![r.f(1).to_bits(), r.f(2).to_bits()],
    });
}

fn log_rect(ctx: &Ctx) {
    let r = &ctx.regs;
    log(Logged {
        addr: sym("DrawRectangle"),
        gpr: vec![read_color(ctx, r.r(3))],
        fpr: (1..5).map(|i| r.f(i).to_bits()).collect(),
    });
}

fn log_ascii(ctx: &Ctx) {
    let r = &ctx.regs;
    log(Logged {
        addr: sym("DrawASCII"),
        gpr: vec![r.r(3), read_color(ctx, r.r(4))],
        fpr: vec![r.f(1).to_bits(), r.f(2).to_bits()],
    });
}

fn log_fog(ctx: &Ctx) {
    log(Logged {
        addr: sym("HSD_FogSet"),
        gpr: vec![ctx.regs.r(3)],
        fpr: vec![],
    });
}

fn log_cobj(ctx: &Ctx) {
    let is_devtext = ctx.regs.r(3) == ctx.read_u32(sym("devtext_cobj"));
    log(Logged {
        addr: sym("HSD_CObjSetCurrent"),
        gpr: vec![u32::from(is_devtext)],
        fpr: vec![],
    });
    ctx.regs.set_r(3, 1);
}

fn draw_natives() -> Vec<calls::Native> {
    vec![
        (sym("hsd_80391A04"), log_text_scale as fn(&Ctx)),
        (sym("DrawRectangle"), log_rect),
        (sym("DrawASCII"), log_ascii),
        (sym("HSD_FogSet"), log_fog),
        (sym("HSD_CObjSetCurrent"), log_cobj),
    ]
}

/// Theirs: the same operations through ssbm-rs's ports, each screen's buffer in scratch
/// memory.
fn run_theirs(ctx: &Ctx, ops: &[Op]) {
    let pool = sym("devtext_pool");
    let list = sym("devtext_drawlist");
    let strings = SCRATCH + 0x10_0000;
    let color = SCRATCH + 0x10_1000;
    let ret = SCRATCH + 0x10_1010;
    let mut at = strings;
    let mut string_at = Vec::new();
    for s in STRINGS {
        string_at.push(at);
        for (i, &b) in s.iter().enumerate() {
            ctx.write_u8(at + i as u32, b);
        }
        at += s.len() as u32;
    }
    let mut made: Vec<u32> = Vec::new();
    for &op in ops {
        let text = |i: usize| made.get(i).copied().filter(|&t| t != 0);
        let h = |t: u32| handle(ctx, t);
        let mut shown = Vec::new();
        let mut cur = ctx.read_u32(list);
        while cur != 0 {
            if shown.len() > POOL {
                return;
            }
            shown.push(cur);
            cur = ctx.read_u32(cur + 0x30);
        }
        match op {
            Op::Create { id, x, y, w, h: ht } => {
                let buf = SCRATCH + 0x1000 * made.len() as u32;
                let t = fns::DevText_Create(ctx, id, x, y, w, ht, handle(ctx, buf));
                made.push(ssbm_rt::Handle::addr(t));
            }
            Op::Show(i) => {
                if let Some(t) = text(i).filter(|t| !shown.contains(t)) {
                    fns::DevText_Show(ctx, handle(ctx, 0), h(t));
                }
            }
            Op::Remove(next) => {
                let head = ctx.read_u32(list);
                let link = match (next, head) {
                    (_, 0) => continue,
                    (false, _) => list,
                    (true, h) if ctx.read_u32(h + 0x30) != 0 => h + 0x30,
                    _ => continue,
                };
                fns::DevText_Remove(ctx, handle(ctx, link));
            }
            op => {
                let Some(t) = text(op_target(op)) else {
                    continue;
                };
                match op {
                    Op::SetCursorXY(_, x, y) => fns::DevText_SetCursorXY(ctx, h(t), x, y),
                    Op::SetCursorX(_, x) => fns::DevText_SetCursorX(ctx, h(t), x),
                    Op::Print(_, k) => fns::DevText_Print(ctx, h(t), handle(ctx, string_at[k])),
                    Op::PrintInt(_, v) => fns::DevText_PrintInt(ctx, h(t), v),
                    Op::SetScale(_, x, y) => {
                        fns::DevText_SetScale(ctx, h(t), leaf_check::arg(x), leaf_check::arg(y))
                    }
                    Op::SetXY(_, x, y) => fns::DevText_SetXY(ctx, h(t), x, y),
                    Op::StoreColor(_, c) => {
                        fns::DevText_StoreColorIndex(ctx, h(t), c);
                    }
                    Op::SetTextColor(_, c) => {
                        ctx.write_u32(color, u32::from_be_bytes(c));
                        fns::DevText_SetTextColor(ctx, handle(ctx, ret), h(t), handle(ctx, color));
                    }
                    Op::SetBgColor(_, c) => {
                        ctx.write_u32(color, u32::from_be_bytes(c));
                        fns::DevText_SetBGColor(ctx, handle(ctx, ret), h(t), handle(ctx, color));
                    }
                    Op::Flags(_, f) => {
                        if f & 0x10 != 0 {
                            fns::DevText_HideCursor(ctx, h(t));
                        }
                        if f & 0x20 != 0 {
                            fns::DevText_80302AC0(ctx, h(t));
                        }
                        if f & 0x40 != 0 {
                            fns::DevText_HideBackground(ctx, h(t));
                        } else {
                            fns::DevText_ShowBackground(ctx, h(t));
                        }
                        if f & 0x80 != 0 {
                            fns::DevText_HideText(ctx, h(t));
                        } else {
                            fns::DevText_ShowText(ctx, h(t));
                        }
                    }
                    Op::Erase(_) => fns::DevText_Erase(ctx, h(t)),
                    Op::EraseFirstLine(_) => fns::DevText_EraseFirstLine(ctx, h(t)),
                    Op::Draw(_, n) => {
                        for _ in 0..n {
                            fns::DevText_Draw(ctx, h(t));
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    let _ = pool;
}

#[test]
fn devtext_ops() {
    let natives = draw_natives();
    Check {
        variants: 0,
        perturb_all: true,
        perturb: &|f| f.regs.gpr[3] = f.below(u64::from(u32::MAX)),
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let drawn = run_ours(r, &ops(a.r(3)));
            vec![Out::Logged(drawn)]
        })
    }
    .with_theirs(&|ctx, call| {
        run_theirs(ctx, &ops(call.regs.gpr[3]));
        (0, 0)
    })
    .run_n(8);
}

#[test]
fn devtext_draw_all() {
    // A few screens made and shown, then the drawing GObj's callback in a random pass.
    let natives = draw_natives();
    Check {
        perturb_all: true,
        perturb: &|f| {
            f.regs.gpr[3] = f.below(u64::from(u32::MAX));
            f.regs.gpr[4] = [2, 2, 1, 3][f.below(4) as usize];
        },
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let shown: Vec<Op> = ops(a.r(3))
                .into_iter()
                .filter(|o| matches!(o, Op::Create { .. } | Op::Show(_) | Op::Print(..)))
                .collect();
            let mut drawn = run_ours(r, &shown);
            let out = r.game.devtext_draw_all(r.data, a.i(4));
            drawn.extend(out.iter().map(logged));
            vec![Out::Logged(drawn)]
        })
    }
    .with_theirs(&|ctx, call| {
        let shown: Vec<Op> = ops(call.regs.gpr[3])
            .into_iter()
            .filter(|o| matches!(o, Op::Create { .. } | Op::Show(_) | Op::Print(..)))
            .collect();
        run_theirs(ctx, &shown);
        fns::DevText_DrawAll(ctx, handle(ctx, 0), call.regs.gpr[4] as i32);
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn devtext_strings() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let mut r = Rand(7);
    for i in 0..2000 {
        let n = match i {
            0 => 0,
            1 => -1,
            2 => i32::MIN,
            3 => i32::MAX,
            _ => r.bits() as i32 >> r.below(32),
        };
        fns::DevText_NumToStr(&ctx, n, handle(&ctx, SCRATCH));
        let ours = devtext::num_to_str(n);
        let theirs: Vec<u8> = (0..)
            .map(|k| ctx.read_u8(SCRATCH + k))
            .take_while(|&b| b != 0)
            .collect();
        assert_eq!(ours, theirs, "DevText_NumToStr({n})");
        let len = fns::DevText_StrLen(&ctx, handle(&ctx, SCRATCH));
        assert_eq!(devtext::str_len(Some(&ours)), len);
    }
    assert_eq!(
        fns::DevText_StrLen(&ctx, handle(&ctx, 0)),
        devtext::str_len(None)
    );
}

#[test]
fn devtext_get_gobj() {
    Check::new("fn_8016C7D0", &|r, _| {
        let g = r.game.hud.devtext.get_gobj();
        vec![Out::Gobj(g)]
    })
    .with_theirs(&|ctx, _| (ssbm_rt::Handle::addr(fns::DevText_GetGObj(ctx)), 0))
    .run();
}

#[test]
fn devtext_setup() {
    Check::new("DevText_Setup", &|r, a| {
        let g = r
            .game
            .devtext_setup(r.data, a.i(3), a.i(4), a.i(5), a.i(6), a.i(7), a.r(8) as u8);
        vec![Out::Gobj(g)]
    })
    .with_theirs(&|ctx, c| {
        let g = &c.regs.gpr;
        let r = fns::DevText_Setup(
            ctx,
            g[3] as i32,
            g[4] as i32,
            g[5] as i32,
            g[6] as i32,
            g[7] as i32,
            g[8] as u8,
        );
        (ssbm_rt::Handle::addr(r), 0)
    })
    .run();
}

#[test]
fn devtext_create_cobj() {
    Check::new("DevText_CreateCObj", &|r, a| {
        r.game
            .devtext_create_cobj(r.data, a.i(3), a.i(4), a.i(5), a.i(6), a.r(7) as u8);
        vec![]
    })
    .with_theirs(&|ctx, c| {
        let g = &c.regs.gpr;
        fns::DevText_CreateCObj(
            ctx,
            g[3] as i32,
            g[4] as i32,
            g[5] as i32,
            g[6] as i32,
            g[7] as u8,
        );
        (0, 0)
    })
    .run();
}

#[test]
fn devtext_init_pool() {
    Check::new("DevText_InitPool", &|r, _| {
        r.game.hud.devtext.init_pool();
        vec![]
    })
    .with_theirs(&|ctx, _| {
        fns::DevText_InitPool(ctx);
        (0, 0)
    })
    .run();
}

#[test]
fn init_debug_menus() {
    Check::new("un_80304138", &|r, _| {
        r.game.init_debug_menus();
        vec![]
    })
    .with_theirs(&|ctx, _| {
        fns::un_80304138(ctx);
        (0, 0)
    })
    .run();
}

// The HUD.

use calls::{Ret, Stub, nothing};
use melee_sim::game::handles::CodeRef;
use melee_sim::game::hud::Draw as HudDraw;
use melee_sim::hsd::gobj::GObjId;

/// The units the HUD reaches that no port covers yet, stood in for on both sides: the
/// magnifier's setup, the results' scoring, and the stage's, Pokemon Stadium's and the
/// stage's start hooks the match's callbacks reach.
const HUD_STUBS: [Stub; 10] = [
    nothing(0x802F_C870),
    (0x8016_C658, Ret::Int(7)),
    nothing(0x801C_1154),
    nothing(0x801C_1158),
    nothing(0x801D_4150),
    nothing(0x801D_4084),
    nothing(0x801D_410C),
    nothing(0x801D_4040),
    nothing(0x801D_40C8),
    nothing(0x8022_52E4),
];

/// [`HUD_STUBS`] with `ftLib_EnableAllInput`.
fn hud_stubs() -> Vec<Stub> {
    let mut v = HUD_STUBS.to_vec();
    v.push(nothing(0x8008_68A4));
    v
}

fn gobj_arg(a: &calls::Args, r: usize) -> GObjId {
    a.gobj(a.r(r))
}

/// The address of one of our GObjs in the capture.
fn gobj_addr(a: &calls::Args, g: GObjId) -> u32 {
    *a.maps
        .gobjs
        .iter()
        .find(|(_, v)| **v == g)
        .expect("a captured GObj")
        .0
}

/// A HUD render callback's draw, as ssbm-rs's loggers record it.
fn hud_logged(a: &calls::Args, d: Option<HudDraw>) -> Vec<Logged> {
    match d {
        None => vec![],
        Some(HudDraw::JObjs(g, pass)) => vec![Logged {
            addr: sym("HSD_GObj_JObjCallback"),
            gpr: vec![gobj_addr(a, g), pass as u32],
            fpr: vec![],
        }],
        Some(HudDraw::Camera(g, mask)) => vec![Logged {
            addr: sym("HSD_GObj_80390ED0"),
            gpr: vec![gobj_addr(a, g), mask],
            fpr: vec![],
        }],
    }
}

fn log_jobj_render(ctx: &Ctx) {
    log(Logged {
        addr: sym("HSD_GObj_JObjCallback"),
        gpr: vec![ctx.regs.r(3), ctx.regs.r(4)],
        fpr: vec![],
    });
}

fn log_links_render(ctx: &Ctx) {
    log(Logged {
        addr: sym("HSD_GObj_80390ED0"),
        gpr: vec![ctx.regs.r(3), ctx.regs.r(4)],
        fpr: vec![],
    });
}

fn camera_current(ctx: &Ctx) {
    ctx.regs.set_r(3, 1);
}

fn nothing_native(_: &Ctx) {}

fn render_natives() -> Vec<calls::Native> {
    vec![
        (sym("HSD_GObj_JObjCallback"), log_jobj_render as fn(&Ctx)),
        (sym("HSD_GObj_80390ED0"), log_links_render),
        (sym("HSD_CObjSetCurrent"), camera_current),
        (sym("HSD_CObjEndCurrent"), nothing_native),
    ]
}

/// A render callback checked from its captures, with the HUD hidden or shown at random.
fn check_render(func: &str, ours: &dyn Fn(&mut Running, &calls::Args) -> Option<HudDraw>) {
    let natives = render_natives();
    Check {
        variants: 4,
        perturb: &|f| {
            {
                let v = f.below(2) as u8;
                f.put8(0x804D_6D58, v);
            }
            let s = 0x804A_10C8;
            for i in 0..6 {
                let b = f.u8(s + 0x64 * i + 0x10) & !0x08 | (f.below(2) as u8) << 3;
                f.put8(s + 0x64 * i + 0x10, b);
            }
        },
        natives: &natives,
        ..Check::new(func, &|r, a| vec![Out::Logged(hud_logged(a, ours(r, a)))])
    }
    .run();
}

#[test]
fn hud_camera_render() {
    check_render("fn_802F36B8", &|r, a| {
        r.game.hud_camera_render(gobj_arg(a, 3))
    });
}

#[test]
fn damage_digits_render() {
    check_render("ifStatus_802F5DE0", &|r, a| {
        r.game.damage_digits_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn damage_mark_render() {
    check_render("ifStatus_802F5E50", &|r, a| {
        r.game.damage_mark_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn ko_mark_render() {
    check_render("fn_802F770C", &|r, a| {
        r.game.ko_mark_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn hud_hidden() {
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[3] = f.below(3),
        ..Check::new("ifAll_IsHUDHidden", &|r, a| match a.r(3) {
            0 => vec![Out::R3(u32::from(r.game.is_hud_hidden()))],
            1 => {
                r.game.hide_hud();
                vec![]
            }
            _ => {
                r.game.show_hud();
                vec![]
            }
        })
    }
    .with_theirs(&|ctx, c| match c.regs.gpr[3] {
        0 => (fns::ifAll_IsHUDHidden(ctx) as u32, 0),
        1 => {
            fns::ifAll_HideHUD(ctx);
            (0, 0)
        }
        _ => {
            fns::ifAll_ShowHUD(ctx);
            (0, 0)
        }
    })
    .run();
}

#[test]
fn hud_positions() {
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[3] = f.below(6),
        ..Check::new("ifAll_GetPlayerHUDPosition", &|r, a| {
            let p = r.game.player_hud_position(a.i(3));
            let t = r.game.timer_position();
            let g = r.game.hud_gobj();
            vec![
                Out::Words(SCRATCH, calls::vec_words(p)),
                Out::Words(SCRATCH + 12, calls::vec_words(t)),
                Out::Gobj(g),
            ]
        })
    }
    .with_theirs(&|ctx, c| {
        let p = ssbm_rt::Handle::addr(fns::ifAll_GetPlayerHUDPosition(ctx, c.regs.gpr[3] as i32));
        let t = ssbm_rt::Handle::addr(fns::ifAll_GetTimerPosition(ctx));
        for k in 0..3 {
            ctx.write_u32(SCRATCH + 4 * k, ctx.read_u32(p + 4 * k));
            ctx.write_u32(SCRATCH + 12 + 4 * k, ctx.read_u32(t + 4 * k));
        }
        (ssbm_rt::Handle::addr(fns::ifAll_GetHUDGObj(ctx)), 0)
    })
    .run();
}

#[test]
fn place_player_displays() {
    // Every count in turn, the places after each written out.
    Check {
        perturb_all: true,
        ..Check::new("ifAll_802F343C", &|r, _| {
            let mut words = Vec::new();
            for count in 0..8 {
                r.place_player_displays(count);
                for p in r.game.hud.all.x18 {
                    words.extend(calls::vec_words(p));
                }
            }
            vec![Out::Words(SCRATCH, words)]
        })
    }
    .with_theirs(&|ctx, _| {
        let mut at = SCRATCH;
        for count in 0..8 {
            fns::ifAll_802F343C(ctx, count);
            for k in 0..18 {
                ctx.write_u32(at, ctx.read_u32(0x804A_0FD8 + 0x18 + 4 * k));
                at += 4;
            }
        }
        (0, 0)
    })
    .run();
}

#[test]
fn read_hud_layout() {
    Check::new("ifAll_802F370C", &|r, a| {
        r.read_hud_layout(a.data_ref(a.r(3)));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        ssbm_types::tu::melee__if__ifall::ifAll_802F370C(ctx, handle(ctx, c.regs.gpr[3]));
        (0, 0)
    })
    .run();
}

/// Their stand-in for `lbArchive_80016F80` (loading `IfAll` by name): the match has it
/// loaded, in the slot it writes.
fn ifall_loaded(_: &Ctx) {}

/// Their stand-in for `lbArchive_80016DBC` (loading `IfCoGet.dat` with its scene): the
/// match has it loaded, the scene in the slot it writes.
fn coget_loaded(ctx: &Ctx) {
    ctx.regs.set_r(3, ctx.read_u32(0x804D_6DA0));
}

/// Their stand-in for `HSD_SisLib_803A945C` (loading `SdIntro.dat`): the match has it
/// loaded as font 2.
fn sdintro_loaded(ctx: &Ctx) {
    ctx.regs
        .set_r(3, ctx.read_u32(sym("HSD_SisLib_804D1110") + 8));
}

#[test]
fn create_hud() {
    // `ifAll_802F390C` loads `IfAll.dat`, so no capture saves it; it runs from the match
    // process's states, the archive the match loaded standing in, the parts no port covers
    // stood in for on both sides.
    let natives = [
        (sym("lbArchive_80016F80"), ifall_loaded as fn(&Ctx)),
        (sym("lbArchive_80016DBC"), coget_loaded),
        (sym("HSD_SisLib_803A945C"), sdintro_loaded),
    ];
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        natives: &natives,
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let files = melee_sim::game::hud::all::HudArchives {
                if_all: a.archive("IfAll"),
                if_coget: a.archive("IfCoGet"),
                sd_intro: a.archive("SdIntro"),
            };
            r.create_hud(files);
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        fns::ifAll_802F390C(ctx);
        (0, 0)
    })
    .run_n(4);
}

#[test]
fn hud_parts_hidden() {
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[3] = f.below(2),
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            if a.r(3) == 0 {
                r.hide_hud_parts();
            } else {
                r.show_hud_parts();
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        if c.regs.gpr[3] == 0 {
            fns::ifAll_802F3394(ctx);
        } else {
            fns::ifAll_802F33CC(ctx);
        }
        (0, 0)
    })
    .run_n(6);
}

// The damage displays.

/// The match's state near the displays' decisions: damage, HP, the displays' flags and
/// shakes, and the player blocks' HP flag.
fn perturb_displays(f: &mut calls::Fuzz) {
    let s = 0x804A_10C8;
    let players = sym("player_slots");
    for i in 0..6u32 {
        let d = s + 0x64 * i;
        if f.below(3) == 0 {
            let v = [0u16, 1, 9, 10, 11, 99, 100, 101, 111, 300, 999, 0xFFFF][f.below(12) as usize];
            f.put16(d + 0xA, v);
        }
        if f.below(3) == 0 {
            let v = [0xFFFFu16, 0, 5, 150][f.below(4) as usize];
            f.put16(d + 0xC, v);
        }
        if f.below(4) == 0 {
            {
                let v = [0, 1, 2, 10][f.below(4) as usize];
                f.put8(d + 0xF, v);
            }
            {
                let v = f.below(80) as u8;
                f.put8(d + 0xE, v);
            }
        }
        if f.below(5) == 0 {
            let flags = f.below(256) as u8 & !0x08;
            f.put8(d + 0x10, flags);
        }
        if f.below(6) == 0 {
            {
                let v = [0, 1, 0xC7, 0xC8, 0xC9][f.below(5) as usize];
                f.put8(d + 9, v);
            }
        }
        let p = players + 0xE90 * i;
        if f.below(4) == 0 {
            // `more_flags` bit 2: stamina (HP) rules.
            let b = f.u8(p + 0x8C) ^ 0x20;
            f.put8(p + 0x8C, b);
        }
    }
    if f.below(3) == 0 {
        {
            let v = [2, 4, 5, 6][f.below(4) as usize];
            f.put8(0x804D_6D60, v);
        }
    }
}

/// [`perturb_displays`], and the called display's damage set about the colour's and digits'
/// edges, its flags left so the digits update.
fn perturb_called_display(f: &mut calls::Fuzz) {
    perturb_displays(f);
    let s = 0x804A_10C8;
    let g = f.regs.gpr[3];
    if let Some(i) = (0..6u32).find(|&i| f.u32(s + 0x64 * i) == g) {
        let d = s + 0x64 * i;
        let v = [300u16, 999, 0, 150, 1, 11, 111, 100, 301][f.below(9) as usize];
        f.put16(d + 0xA, v);
        let v = [0xFFFFu16, 0, 5][f.below(3) as usize];
        f.put16(d + 0xC, v);
        let b = f.u8(d + 0x10) & !0x80;
        f.put8(d + 0x10, b);
    }
}

#[test]
fn damage_digits_proc() {
    Check {
        variants: 12,
        perturb: &perturb_called_display,
        ..Check::new("ifStatus_802F4EDC", &|r, a| {
            r.damage_digits_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn damage_proc() {
    let stubs = hud_stubs();
    Check {
        variants: 6,
        perturb: &perturb_displays,
        stubs: &stubs,
        ..Check::new("ifStatus_802F5B48", &|r, a| {
            r.damage_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

/// A display's index from its captured state pointer.
fn display_arg(a: &calls::Args, r: usize) -> usize {
    ((a.r(r) - 0x804A_10C8) / 0x64) as usize
}

#[test]
fn shake_damage_digits() {
    Check {
        variants: 6,
        perturb: &perturb_displays,
        ..Check::new("ifStatus_802F4B84", &|r, a| {
            r.shake_damage_digits(display_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn damage_digits_fly() {
    Check {
        variants: 6,
        perturb: &|f| {
            let s = f.regs.gpr[3];
            let b = f.u8(s + 0x10) ^ (f.below(2) as u8) << 6;
            f.put8(s + 0x10, b);
            {
                let v = f.below(2) as u8;
                f.put8(0x804D_6D61, v);
            }
        },
        ..Check::new("ifStatus_PercentOnDeathAnimationThink", &|r, a| {
            r.damage_digits_fly(display_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn create_damage_digits() {
    Check::new("ifStatus_802F5EC0", &|r, a| {
        let g = r.create_damage_digits(display_arg(a, 3), a.i(4));
        vec![Out::Gobj(g)]
    })
    .run();
}

#[test]
fn create_damage_mark() {
    Check::new("ifStatus_802F61FC", &|r, a| {
        let g = r.create_damage_mark(display_arg(a, 3), a.i(4));
        vec![Out::Gobj(g)]
    })
    .run();
}

#[test]
fn nth_child() {
    Check {
        perturb_all: true,
        variants: 4,
        perturb: &|f| f.regs.gpr[4] = [0u32, 1, 3, 4, 9, u32::MAX][f.below(6) as usize],
        ..Check::new("ifStatus_802F6194", &|r, a| {
            let n = melee_sim::game::hud::status::nth_child(
                &r.game.objs.jobjs,
                (a.r(3) != 0).then(|| a.maps.jobjs[&a.r(3)]),
                a.i(4),
            );
            let addr = n.map_or(0, |j| {
                *a.maps
                    .jobjs
                    .iter()
                    .find(|(_, v)| **v == j)
                    .expect("a captured joint")
                    .0
            });
            vec![Out::R3(addr)]
        })
    }
    .with_theirs(&|ctx, c| {
        let j = fns::ifStatus_802F6194(ctx, handle(ctx, c.regs.gpr[3]), c.regs.gpr[4] as i32);
        (ssbm_rt::Handle::addr(j), 0)
    })
    .run();
}

#[test]
fn show_damage_display() {
    let stubs = hud_stubs();
    Check {
        variants: 4,
        perturb: &|f| {
            let start = sym("controller") + 0x24C8;
            // `x2_6` (show damage), `x2_5`, `x3_0` (scores) and the match kind.
            let b = f.u8(start + 2) ^ (f.below(2) as u8) << 1;
            f.put8(start + 2, b);
            let b = f.u8(start + 3) ^ (f.below(2) as u8) << 7;
            f.put8(start + 3, b);
        },
        stubs: &stubs,
        ..Check::new("ifStatus_802F6508", &|r, a| {
            r.show_damage_display(a.i(3));
            vec![]
        })
    }
    .run();
}

#[test]
fn show_damage_displays() {
    let stubs = hud_stubs();
    Check {
        stubs: &stubs,
        ..Check::new("ifStatus_802F665C", &|r, a| {
            r.show_damage_displays(a.i(3));
            vec![]
        })
    }
    .run();
}

#[test]
fn load_damage_models() {
    Check::new("ifStatus_802F66A4", &|r, _| {
        r.load_damage_models();
        vec![]
    })
    .run();
}

/// A KO's display and marks, from the match process's states with a random slot and rules.
#[test]
fn damage_display_teammate_ko() {
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(6);
            f.regs.gpr[21] = f.below(2);
            f.regs.gpr[22] = f.below(6);
            let start = sym("controller") + 0x24C8;
            let b = f.u8(start) & !0xE0 | [0x20u8, 0x40, 0x00][f.below(3) as usize];
            f.put8(start, b);
            let b = f.u8(start + 2) | 0x04;
            f.put8(start + 2, b);
            let pen = [0xFEu8, 0xFF, 0x00][f.below(3) as usize];
            f.put8(start + 0xC, pen);
        },
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let (slot, by) = (a.i(20), a.i(22));
            match a.r(21) {
                0 => r.damage_display_teammate_ko(slot, by),
                _ => r.damage_display_fall(slot),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let (slot, by) = (c.regs.gpr[20] as i32, c.regs.gpr[22] as i32);
        match c.regs.gpr[21] {
            0 => fns::ifStatus_802F69C0(ctx, slot, by),
            _ => fns::ifStatus_802F6D10(ctx, slot),
        }
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn damage_display_ko() {
    let stubs = hud_stubs();
    Check {
        stubs: &stubs,
        ..Check::new("ifStatus_802F6AF8", &|r, a| {
            r.damage_display_ko(a.i(3));
            vec![]
        })
    }
    .run();
}

#[test]
fn damage_display_self_destruct() {
    let stubs = hud_stubs();
    Check {
        stubs: &stubs,
        ..Check::new("ifStatus_802F6C04", &|r, a| {
            r.damage_display_self_destruct(a.i(3));
            vec![]
        })
    }
    .run();
}

#[test]
fn damage_display_respawn() {
    let stubs = hud_stubs();
    Check {
        stubs: &stubs,
        ..Check::new("ifStatus_802F6E1C", &|r, a| {
            r.damage_display_respawn(a.i(3));
            vec![]
        })
    }
    .run();
}

#[test]
fn damage_display_misc() {
    // Hiding, showing, exploding, pausing and freeing displays from the match's states.
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(6);
            f.regs.gpr[21] = f.below(6);
        },
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let slot = a.i(21);
            match a.r(20) {
                0 => r.game.hide_damage_digits(),
                1 => r.game.show_damage_digits(),
                2 => r.game.explode_damage_display(slot),
                3 => r.pause_damage_display(slot),
                4 => r.free_damage_display(slot as u8),
                _ => r.free_damage_displays(),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let slot = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::ifStatus_802F6898(ctx),
            1 => fns::ifStatus_802F68F0(ctx),
            2 => fns::ifStatus_802F6948(ctx, slot),
            3 => fns::ifStatus_802F6E3C(ctx, slot),
            4 => fns::ifStatus_802F6788(ctx, slot as u8),
            _ => fns::ifStatus_802F6804(ctx),
        }
        (0, 0)
    })
    .run_n(6);
}

// The announcements and KO marks.

#[test]
fn announce() {
    let stubs = hud_stubs();
    Check {
        variants: 6,
        perturb: &|f| {
            f.regs.gpr[3] = [0u32, 3, 4, 5, 8][f.below(5) as usize];
            f.regs.gpr[4] = [u32::MAX, 0x7C85D][f.below(2) as usize];
            f.regs.gpr[5] = [u32::MAX, 0x7C855][f.below(2) as usize];
        },
        stubs: &stubs,
        ..Check::new("ifStatus_802F6EA4", &|r, a| {
            let cb = |x: u32| (x != 0).then_some(CodeRef(x));
            r.announce(a.i(3), a.i(4), a.i(5), a.i(6), cb(a.r(7)), cb(a.r(8)));
            vec![]
        })
    }
    .run();
}

#[test]
fn announce_match_end() {
    let stubs = hud_stubs();
    Check {
        variants: 6,
        perturb: &|f| {
            let c = sym("controller");
            {
                let v = [0u8, 0, 2, 6][f.below(4) as usize];
                f.put8(c + 0xB, v);
            }
            {
                let v = [1u8, 2, 6][f.below(3) as usize];
                f.put8(c + 0x5, v);
            }
            {
                let v = [0u32, 0x7C860][f.below(2) as usize];
                f.put(c + 0x10, v);
            }
        },
        stubs: &stubs,
        ..Check::new("ifStatus_802F7034", &|r, a| {
            r.announce_match_end((a.r(3) != 0).then_some(CodeRef(a.r(3))));
            vec![]
        })
    }
    .run();
}

#[test]
fn load_announcements() {
    Check::new("ifStatus_802F7134", &|r, _| {
        r.load_announcements();
        vec![]
    })
    .run();
}

#[test]
fn free_announcements() {
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        stubs: &stubs,
        ..Check::new("if_802F74D0", &|r, _| {
            r.free_announcements();
            r.free_ko_marks();
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        fns::ifStatus_802F7220(ctx);
        fns::if_802F7E7C(ctx);
        (0, 0)
    })
    .run();
}

#[test]
fn announcement_procs() {
    let stubs = hud_stubs();
    for (k, func) in ["if_802F73C4", "if_802F74D0"].into_iter().enumerate() {
        Check {
            stubs: &stubs,
            ..Check::new(func, &|r, a| {
                if k == 0 {
                    r.announcement_once(gobj_arg(a, 3));
                } else {
                    r.announcement_wait(gobj_arg(a, 3));
                }
                vec![]
            })
        }
        .run();
    }
}

#[test]
fn announcement_step() {
    Check::new("fn_802F7288", &|r, a| {
        let k = ((a.r(4) - 0x803F_9628) / 0x28) as usize;
        vec![Out::R3(u32::from(r.announcement_step(gobj_arg(a, 3), k)))]
    })
    .run();
}

#[test]
fn ko_mark_procs() {
    for (k, func) in ["fn_802F75D4", "fn_802F7670", "fn_802F7994"]
        .into_iter()
        .enumerate()
    {
        Check::new(func, &|r, a| {
            match k {
                0 => r.ko_mark_proc(gobj_arg(a, 3)),
                1 => r.ko_mark_proc_2(gobj_arg(a, 3)),
                _ => r.ko_mark_proc_spawning(gobj_arg(a, 3)),
            }
            vec![]
        })
        .run();
    }
}

#[test]
fn create_ko_mark() {
    Check::new("fn_802F77F8", &|r, a| {
        let old = (a.r(3) != 0).then(|| gobj_arg(a, 3));
        let g = r.create_ko_mark(old, a.r(4) as u8, a.r(5) as u16);
        vec![Out::Gobj(g)]
    })
    .run();
}

#[test]
fn ko_marks() {
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(4);
            f.regs.gpr[21] = f.below(6);
            let start = sym("controller") + 0x24C8;
            let pen = [0xFEu8, 0xFF, 0x00][f.below(3) as usize];
            f.put8(start + 0xC, pen);
        },
        ..Check::new("fn_8016C7D0", &|r, a| {
            let f = [0x802F_7AF8, 0x802F_7BB4, 0x802F_7C30, 0x802F_7D08][a.r(20) as usize];
            r.ko_mark(f, a.i(21));
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let slot = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::if_802F7AF8(ctx, slot),
            1 => fns::if_802F7BB4(ctx, slot),
            2 => fns::if_802F7C30(ctx, slot),
            _ => fns::if_802F7D08(ctx, slot),
        }
        (0, 0)
    })
    .run_n(6);
}

/// A KO mark played out: each frame, each slot's marks run their processes.
#[test]
fn ko_mark_run() {
    use melee_sim::game::hud::announce::{MARK_FADE, MARK_FADE_2, MARK_SPAWN_2};
    const SLOTS: u32 = 0x804A_1340 + 4;
    Check {
        perturb_all: true,
        variants: 6,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(4);
            f.regs.gpr[21] = f.below(6);
        },
        ..Check::new("fn_8016C7D0", &|r, a| {
            let f = [0x802F_7AF8, 0x802F_7BB4, 0x802F_7C30, 0x802F_7D08][a.r(20) as usize];
            r.ko_mark(f, a.i(21));
            for _ in 0..50 {
                for slot in 0..6 {
                    for second in [false, true] {
                        let s = &r.game.hud.ko_marks.slots[slot];
                        let Some(g) = (if second { s.gobj2 } else { s.gobj }) else {
                            continue;
                        };
                        let p = r.game.world.gobj(g).proc.expect("a mark's process");
                        match r.game.world.proc(p).on_invoke.0 {
                            MARK_FADE => r.ko_mark_proc(g),
                            MARK_FADE_2 => r.ko_mark_proc_2(g),
                            MARK_SPAWN_2 => r.ko_mark_proc_spawning(g),
                            other => panic!("a mark with process {other:#x}"),
                        }
                    }
                }
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let slot = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::if_802F7AF8(ctx, slot),
            1 => fns::if_802F7BB4(ctx, slot),
            2 => fns::if_802F7C30(ctx, slot),
            _ => fns::if_802F7D08(ctx, slot),
        }
        for _ in 0..50 {
            for slot in 0..6u32 {
                for second in [false, true] {
                    let g = ctx.read_u32(SLOTS + 8 * slot + 4 * u32::from(second));
                    if g == 0 {
                        continue;
                    }
                    let proc = ctx.read_u32(g + 0x18);
                    let gh = handle(ctx, g);
                    match ctx.read_u32(proc + 0x14) {
                        MARK_FADE => fns::fn_802F75D4(ctx, gh),
                        MARK_FADE_2 => fns::fn_802F7670(ctx, gh),
                        MARK_SPAWN_2 => fns::fn_802F7994(ctx, gh),
                        other => panic!("a mark with process {other:#x}"),
                    }
                }
            }
        }
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn load_ko_marks() {
    Check::new("if_802F7E24", &|r, _| {
        r.load_ko_marks();
        vec![]
    })
    .run();
}

// The timer.

/// The timer's state near its decisions: seconds and frames about the countdown, the
/// countdown's second, and the timer's direction and hour column.
fn perturb_timer(f: &mut calls::Fuzz) {
    let c = sym("controller");
    let secs = [0u32, 4, 5, 6, 59, 60, 3599, 3600, 36000, 359_999][f.below(10) as usize];
    f.put(c + 0x28, secs);
    {
        let v = [0u16, 1, 58, 59][f.below(4) as usize];
        f.put16(c + 0x2C, v);
    }
    let start = c + 0x24C8;
    let b = f.u8(start) ^ (f.below(2) as u8) << 1;
    f.put8(start, b);
    {
        let v = f.below(6) as u8;
        f.put8(0x804A_1078 + 0x30, v);
    }
}

#[test]
fn timer_proc() {
    Check {
        variants: 8,
        perturb: &perturb_timer,
        ..Check::new("ifTime_UpdateTimers", &|r, a| {
            r.timer_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn countdown_proc() {
    Check {
        variants: 8,
        perturb: &perturb_timer,
        ..Check::new("ifTime_UpdateCountdown", &|r, a| {
            r.countdown_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn set_timer_digits() {
    Check {
        variants: 8,
        perturb: &|f| {
            perturb_timer(f);
            f.regs.gpr[4] =
                [0u32, 59, 60, 3599, 3600, 36_000, 359_999, u32::MAX][f.below(8) as usize];
            f.regs.gpr[5] = f.below(100);
        },
        ..Check::new("ifTime_SetTime", &|r, a| {
            let j = a.maps.jobjs.get(&a.r(3)).copied();
            r.game.set_timer_digits(j, a.i(4), a.i(5));
            vec![]
        })
    }
    .run();
}

#[test]
fn timer_queries() {
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            perturb_timer(f);
            f.regs.gpr[20] = f.below(5);
        },
        ..Check::new("ifTime_IsTimerHidden", &|r, a| match a.r(20) {
            0 => vec![Out::R3(u32::from(r.game.is_timer_hidden()))],
            1 => vec![Out::R3(r.game.countdown_seconds() as u32)],
            2 => vec![Out::R3(u32::from(r.game.vs_timer_centiseconds()))],
            3 => {
                r.hide_timers();
                vec![]
            }
            _ => {
                r.show_timers();
                vec![]
            }
        })
    }
    .with_theirs(&|ctx, c| match c.regs.gpr[20] {
        0 => (fns::ifTime_IsTimerHidden(ctx) as u32, 0),
        1 => (fns::ifTime_GetCountdownSeconds(ctx) as u32, 0),
        2 => (u32::from(fns::gm_8016AF0C(ctx)), 0),
        3 => {
            fns::ifTime_HideTimers(ctx);
            (0, 0)
        }
        _ => {
            fns::ifTime_ShowTimers(ctx);
            (0, 0)
        }
    })
    .run();
}

#[test]
fn create_timers() {
    Check::new("ifTime_CreateTimers", &|r, _| {
        r.create_timers();
        vec![]
    })
    .run();
}

#[test]
fn reset_timers() {
    Check::new("ifTime_Reset", &|r, _| {
        r.game.reset_timers();
        vec![]
    })
    .run();
}

#[test]
fn free_timers() {
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[20] = f.below(2),
        ..Check::new("ifTime_FreeCountdown", &|r, a| {
            if a.r(20) == 0 {
                r.free_countdown();
            } else {
                r.free_timers();
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        if c.regs.gpr[20] == 0 {
            fns::ifTime_FreeCountdown(ctx);
        } else {
            fns::ifTime_FreeTimers(ctx);
        }
        (0, 0)
    })
    .run();
}

#[test]
fn sd_penalty() {
    Check {
        perturb_all: true,
        perturb: &|f| {
            let start = sym("controller") + 0x24C8;
            {
                let v = [0xFEu8, 0xFF, 0x00, 0x7F][f.below(4) as usize];
                f.put8(start + 0xC, v);
            }
        },
        ..Check::new("fn_8016C7D0", &|r, _| {
            vec![Out::R3(r.game.vs_sd_penalty() as u32)]
        })
    }
    .with_theirs(&|ctx, _| (fns::gm_8016AEC8(ctx) as u32, 0))
    .run_n(3);
}

#[test]
fn stock_icon_frame() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    for ck in -1..0x24 {
        for ft in [0, 7, 0x13, -1] {
            for costume in 0..6 {
                let ours = melee_sim::game::gm::vs::stock_icon_frame(ck, ft, costume);
                let theirs = fns::gm_80168B34(&ctx, ck, ft, costume);
                assert_eq!(ours.to_bits(), theirs.to_bits(), "{ck} {ft} {costume}");
            }
        }
    }
}

#[test]
fn hud_color() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    for c in 0..9 {
        fns::gm_80160968(&ctx, handle(&ctx, SCRATCH), c);
        let theirs = ctx.read_u32(SCRATCH).to_be_bytes();
        assert_eq!(melee_sim::game::gm::vs::hud_color(c), theirs);
    }
}

#[test]
fn vs_hud_callbacks() {
    // The announcements' callbacks into the match: GO!'s end, Ready's start and end, and
    // GAME!'s start.
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 6,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(4);
            f.regs.gpr[21] = [u32::MAX, 0, 3, 5][f.below(4) as usize];
            let start = sym("controller") + 0x24C8;
            let b = f.u8(start + 1) ^ (f.below(2) as u8) << 4;
            f.put8(start + 1, b);
        },
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let k = a.i(21);
            match a.r(20) {
                0 => r.vs_hud_on(k),
                1 => r.vs_ready(k),
                2 => r.vs_go(k),
                _ => r.vs_game_end(k),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let k = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::fn_8016B784(ctx, k),
            1 => fns::fn_8016B7B4(ctx, k),
            2 => fns::fn_8016B7F8(ctx, k),
            _ => fns::fn_8016B88C(ctx, k),
        }
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn anim_helpers() {
    // lb_8000B09C, lb_8000C07C and lb_8000C0E8 on the HUD's models.
    Check {
        perturb_all: true,
        variants: 4,
        perturb: &|f| f.regs.gpr[20] = f.below(3),
        ..Check::new("ifTime_UpdateTimers", &|r, a| {
            let j = match r.game.world.gobj(gobj_arg(a, 3)).obj {
                Some(melee_sim::game::state::Obj::JObj(j)) => j,
                _ => unreachable!(),
            };
            let m = r.game.hud.timer_models;
            match a.r(20) {
                0 => vec![Out::R3(u32::from(melee_sim::lb::anim::any_animated(
                    &r.game.objs,
                    Some(j),
                )))],
                1 => {
                    melee_sim::lb::anim::add_anim_index(
                        &mut r.game.objs,
                        r.data,
                        Some(j),
                        0,
                        m.anims,
                        m.matanims,
                        m.shapeanims,
                    );
                    vec![]
                }
                _ => {
                    let models = r.game.hud.time.countdown_timer_models.unwrap();
                    let model = r.data.ptr(models).unwrap();
                    melee_sim::lb::anim::add_model_anim(
                        &mut r.game.objs,
                        r.data,
                        Some(j),
                        2,
                        model,
                    );
                    vec![]
                }
            }
        })
    }
    .with_theirs(&|ctx, c| {
        let g = c.regs.gpr[3];
        let j = ctx.read_u32(g + 0x28);
        match c.regs.gpr[20] {
            0 => (fns::lb_8000B09C(ctx, handle(ctx, j)) as u32, 0),
            1 => {
                let m = 0x804A_10B4;
                fns::lb_8000C07C(
                    ctx,
                    handle(ctx, j),
                    0,
                    handle(ctx, ctx.read_u32(m + 4)),
                    handle(ctx, ctx.read_u32(m + 8)),
                    handle(ctx, ctx.read_u32(m + 12)),
                );
                (0, 0)
            }
            _ => {
                let models = ctx.read_u32(0x804A_1078 + 0x38);
                fns::lb_8000C0E8(ctx, handle(ctx, j), 2, handle(ctx, ctx.read_u32(models)));
                (0, 0)
            }
        }
    })
    .run_n(4);
}

/// Logs a sound the game plays on no track (`lbAudioAx_800237A8`), with the volume and pan
/// the driver gets (doubled, as `fn_80023750` hands them on).
fn log_sfx(ctx: &Ctx) {
    let r = &ctx.regs;
    let driver = |v: u32| (v as i32).wrapping_mul(2).clamp(0, 0xFF) as u32;
    log(Logged {
        addr: sym("lbAudioAx_800237A8"),
        gpr: vec![r.r(3), driver(r.r(4)), driver(r.r(5))],
        fpr: vec![],
    });
}

/// Our sounds on no track as [`log_sfx`] records them.
fn sfx_logged(events: &[melee_sim::game::state::Event]) -> Vec<Logged> {
    events
        .iter()
        .filter_map(|e| match *e {
            melee_sim::game::state::Event::SfxStart {
                id,
                volume,
                pan,
                track: 0,
                ..
            } => Some(Logged {
                addr: sym("lbAudioAx_800237A8"),
                gpr: vec![id as u32, u32::from(volume), u32::from(pan)],
                fpr: vec![],
            }),
            _ => None,
        })
        .collect()
}

/// An announcement played out frame by frame: its flags each frame and the sounds it plays.
#[test]
fn announcement_run() {
    let natives = [(sym("lbAudioAx_800237A8"), log_sfx as fn(&Ctx))];
    Check {
        perturb_all: true,
        variants: 7,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(8);
            f.regs.gpr[21] = [u32::MAX, 0x7C85D][f.below(2) as usize];
            f.regs.gpr[22] = [u32::MAX, 0x7C855][f.below(2) as usize];
            f.regs.gpr[23] = [0, 5, 30][f.below(3) as usize];
        },
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let k = a.r(20) as usize;
            r.game.events.clear();
            r.announce(k as i32, a.i(21), a.i(22), a.i(23), None, None);
            let mut flags = Vec::new();
            for _ in 0..90 {
                let Some(g) = r.game.hud.announcements[k].x0 else {
                    break;
                };
                if r.game.hud.announcements[k].x8 == Some(CodeRef(0x802F_73C4)) {
                    r.announcement_once(g);
                } else {
                    r.announcement_wait(g);
                }
                let x = &r.game.hud.announcements[k].x12;
                flags.push(u32::from(x.x2) | u32::from(x.x1) << 1 | u32::from(x.x0) << 2);
            }
            vec![
                Out::Words(SCRATCH, flags),
                Out::Logged(sfx_logged(&r.game.events)),
            ]
        })
    }
    .with_theirs(&|ctx, c| {
        let g = &c.regs.gpr;
        let k = g[20];
        let e = 0x803F_9628 + 0x28 * k;
        fns::ifStatus_802F6EA4(
            ctx,
            k as i32,
            g[21] as i32,
            g[22] as i32,
            g[23] as i32,
            handle(ctx, 0),
            handle(ctx, 0),
        );
        let mut at = SCRATCH;
        for _ in 0..90 {
            let o = ctx.read_u32(e);
            if o == 0 {
                break;
            }
            if ctx.read_u32(e + 8) == 0x802F_73C4 {
                fns::if_802F73C4(ctx, handle(ctx, o));
            } else {
                fns::if_802F74D0(ctx, handle(ctx, o));
            }
            ctx.write_u32(at, u32::from(ctx.read_u8(e + 0x12) >> 5));
            at += 4;
        }
        (0, 0)
    })
    .run_n(4);
}

// The stock icons.

/// The stock rows' state near their decisions: stocks, coins, the icons' animation steps and
/// flights, and the rows' modes.
fn perturb_stocks(f: &mut calls::Fuzz) {
    let players = sym("player_slots");
    for i in 0..6u32 {
        let v = [0u8, 1, 2, 4, 5, 6, 9, 10, 12, 99, 100, 127, 0xFF][f.below(13) as usize];
        f.put8(players + 0xE90 * i + 0x8E, v);
        let u = 0x804A_157C + 0x54 * i;
        // The flying icons' paths: start, middle and end of each.
        if f.below(2) == 0 {
            for k in 0..18u32 {
                let v = [-30.5f32, 0.0, 12.25, 47.0][f.below(4) as usize];
                f.put(u + 0xC + 4 * k, v.to_bits());
            }
        }
        if f.below(4) == 0 {
            let v = f.below(2) as u8;
            f.put8(u + 2, v);
        }
        for k in 0..7 {
            if f.below(5) == 0 {
                let v = [0u8, 1, 5, 9, 10, 11][f.below(6) as usize];
                f.put8(u + 5 + k, v);
            }
        }
        // Mostly a stock match's row (the captures are a time match's).
        let v = [0u8, 0, 1, 2][f.below(4) as usize];
        f.put8(u + 1, v);
    }
}

#[test]
fn stock_proc() {
    let stubs = hud_stubs();
    Check {
        variants: 26,
        perturb: &perturb_stocks,
        stubs: &stubs,
        ..Check::new("fn_802F9410", &|r, a| {
            r.stock_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn stock_icons_render() {
    check_render("fn_802F94E0", &|r, a| {
        r.game.stock_icons_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn create_stock_icons() {
    let stubs = hud_stubs();
    Check {
        variants: 6,
        perturb: &|f| {
            f.regs.gpr[4] = f.below(3);
            let c = sym("controller") + 0x38 + 0xE * f.regs.gpr[3] + 4;
            let v = [0u8, 0x40, 0x80][f.below(3) as usize];
            f.put8(c, v);
        },
        stubs: &stubs,
        ..Check::new("ifStock_802F98E8", &|r, a| {
            r.create_stock_icons(a.r(3) as u8, a.r(4) as u8);
            vec![]
        })
    }
    .run();
}

#[test]
fn load_stock_icons() {
    Check::new("ifStock_802FAEC4", &|r, _| {
        r.load_stock_icons();
        vec![]
    })
    .run();
}

#[test]
fn stock_tallies() {
    for (k, func) in ["fn_802FA6C4", "fn_802FAC34"].into_iter().enumerate() {
        Check::new(func, &|r, a| {
            if k == 0 {
                r.stock_tally_proc(gobj_arg(a, 3));
            } else {
                r.stock_tally_proc_2(gobj_arg(a, 3));
            }
            vec![]
        })
        .run();
    }
}

#[test]
fn steal_stock() {
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 10,
        perturb: &|f| {
            perturb_stocks(f);
            f.regs.gpr[20] = f.below(2);
            f.regs.gpr[21] = f.below(2);
        },
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let (t, d) = (a.i(20), a.i(21));
            if r.game.hud.stock.player[t as usize].x0.is_none()
                || r.game.hud.stock.player[d as usize].x0.is_none()
            {
                return vec![Out::R3(9)];
            }
            vec![Out::R3(r.steal_stock(t, d) as u32)]
        })
    }
    .with_theirs(&|ctx, c| {
        let (t, d) = (c.regs.gpr[20], c.regs.gpr[21]);
        let row = |p: u32| ctx.read_u32(0x804A_1378 + 8 + 0x50 * p);
        if row(t) == 0 || row(d) == 0 {
            return (9, 0);
        }
        (fns::ifStock_802F7EFC(ctx, t as i32, d as i32) as u32, 0)
    })
    .run_n(6);
}

#[test]
fn stock_misc() {
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(7);
            f.regs.gpr[21] = f.below(2);
            f.regs.gpr[22] = f.below(1 << 24);
        },
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let p = a.i(21);
            match a.r(20) {
                0 => r.free_stock_tally(),
                1 => r.free_stock_bonus_icons(),
                2 => r.free_stock_tally_2(),
                3 => {
                    if r.game.hud.stock.player[p as usize].x0.is_some() {
                        let c = a.r(22).to_be_bytes();
                        r.game.color_stock_icons(p, [c[1], c[2], c[3]]);
                    }
                }
                4 => r.free_stock_icons(p),
                5 => r.stock_icons_flags(p),
                _ => {}
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let p = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::ifStock_802FB390(ctx),
            1 => fns::ifStock_802FB41C(ctx),
            2 => fns::ifStock_802FB484(ctx),
            3 => {
                if ctx.read_u32(0x804A_1378 + 8 + 0x50 * p as u32) != 0 {
                    ctx.write_u32(SCRATCH, c.regs.gpr[22] << 8);
                    fns::ifStock_802FB4EC(ctx, p, handle(ctx, SCRATCH));
                }
            }
            4 => fns::ifStock_802FB650(ctx, p),
            5 => fns::ifStock_802FB6AC(ctx, p),
            _ => {}
        }
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn player_stock_icon() {
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[20] = f.below(4),
        ..Check::new("fn_8016C7D0", &|r, a| {
            vec![Out::F1(r.game.player_stock_icon(a.i(20)))]
        })
    }
    .with_theirs(&|ctx, c| {
        let v = fns::gm_80168BF8(ctx, c.regs.gpr[20] as i32);
        ctx.regs.set_f(1, v);
        (0, 0)
    })
    .run_n(4);
}

// The players' tags.

#[test]
fn nametag_proc() {
    let stubs = [(0x8008_6F80, Ret::Float(12.5))];
    Check {
        variants: 8,
        perturb: &|f| {
            let i = f.below(6);
            {
                let v = f.u8(0x804D_6D70 + i) ^ 1;
                f.put8(0x804D_6D70 + i, v);
            }
            let players = sym("player_slots");
            let s = f.below(6);
            let v = [0u8, 1, 3][f.below(3) as usize];
            f.put8(players + 0xE90 * s + 0x8E, v);
        },
        stubs: &stubs,
        ..Check::new("fn_802FCC44", &|r, a| {
            r.nametag_proc(gobj_arg(a, 3));
            vec![]
        })
    }
    .run();
}

#[test]
fn nametag_render() {
    check_render("NameTag_RenderCallback", &|r, a| {
        r.game.nametag_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn nametag_camera_render() {
    let natives = render_natives();
    Check {
        variants: 4,
        perturb: &|f| {
            {
                let v = f.below(2) as u8;
                f.put8(0x804D_6D58, v);
            }
            {
                let v = f.below(2) as u8;
                f.put8(0x804D_6D6C, v);
            }
        },
        natives: &natives,
        ..Check::new("fn_802FCAC4", &|r, a| {
            let d = r.nametag_camera_render(gobj_arg(a, 3));
            vec![Out::Logged(hud_logged(a, d))]
        })
    }
    .run();
}

#[test]
fn nametag_frame() {
    Check {
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[4] = f.below(5);
            f.regs.gpr[5] = f.below(2);
            f.regs.gpr[6] = f.below(2);
        },
        ..Check::new("un_802FC9B4", &|r, a| {
            let v = r
                .game
                .nametag_frame(a.r(3) as u8, a.r(4) as u8, a.r(5) as u8, a.r(6) as u8);
            vec![Out::F1(f64::from(v))]
        })
    }
    .run();
}

#[test]
fn create_nametag() {
    Check::new("NameTag_Create", &|r, a| {
        r.create_nametag(a.i(3));
        vec![]
    })
    .run();
}

/// `un_802FD28C` on the slot of a running tag's process.
#[test]
fn refresh_nametag() {
    Check::new("fn_802FCC44", &|r, a| {
        let g = gobj_arg(a, 3);
        let Some(melee_sim::game::state::UserData::NameTagSlot(s)) = r.game.world.gobj(g).user_data
        else {
            panic!("a tag's process without its slot");
        };
        r.refresh_nametag(i32::from(s));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        let ud = ctx.read_u32(c.regs.gpr[3] + 0x2C);
        fns::un_802FD28C(ctx, i32::from(ctx.read_u8(ud)));
        (0, 0)
    })
    .run();
}

#[test]
fn load_nametag_model() {
    Check::new("un_802FCBA0", &|r, _| {
        r.game.load_nametag_model(r.data);
        vec![]
    })
    .run();
}

#[test]
fn nametag_flags() {
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[20] = f.below(4),
        ..Check::new("fn_8016C7D0", &|r, a| {
            match a.r(20) {
                0 => r.game.show_nametags(),
                1 => r.game.unshow_nametags(),
                2 => r.game.pause_nametags(),
                _ => r.game.unpause_nametags(),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        match c.regs.gpr[20] {
            0 => fns::un_802FD404(ctx),
            1 => fns::un_802FD428(ctx),
            2 => fns::un_802FD450(ctx),
            _ => fns::un_802FD45C(ctx),
        }
        (0, 0)
    })
    .run_n(4);
}

#[test]
fn nametags_init_and_free() {
    let natives = [(sym("HSD_SisLib_803A945C"), sdintro_loaded as fn(&Ctx))];
    Check {
        perturb_all: true,
        perturb: &|f| f.regs.gpr[20] = f.below(2),
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            if a.r(20) == 0 {
                r.init_nametags(a.archive("SdIntro"));
            } else {
                r.free_nametags();
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        if c.regs.gpr[20] == 0 {
            fns::un_802FD4C8(ctx);
        } else {
            fns::un_802FD468(ctx);
        }
        (0, 0)
    })
    .run_n(4);
}

// The hazard sign, the coin counter, the arrows and the scores.

#[test]
fn hazard() {
    // A sign made from the stock icons' model (any `DynamicModelDesc`), shown, and run.
    Check {
        perturb_all: true,
        variants: 4,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(3);
            f.regs.gpr[21] = [1, 5, 30][f.below(3) as usize];
            f.regs.gpr[22] = f.below(2);
        },
        ..Check::new("fn_8016C7D0", &|r, a| {
            let models = r.game.hud.stock.x0.expect("the icons' models");
            let desc = r.data.ptr(models).unwrap();
            r.create_hazard(desc, a.i(20));
            r.game.show_hazard(r.data, a.i(21));
            if a.r(22) == 1 {
                r.game.pause_hazard();
            }
            let g = r.game.hud.hazard.gobj.unwrap();
            let mut drawn = Vec::new();
            for _ in 0..40 {
                r.hazard_proc(g);
                drawn.push(u32::from(r.game.hazard_render(g, 0).is_some()));
            }
            r.game.unpause_hazard();
            r.game.hide_hazard();
            r.game.init_hazard();
            r.game.free_hazard();
            vec![Out::Words(SCRATCH, drawn)]
        })
    }
    .with_theirs(&|ctx, c| {
        let models = ctx.read_u32(0x804A_1378);
        let desc = ctx.read_u32(models);
        let k = c.regs.gpr[20];
        match k {
            0 => fns::un_802FD8A0(ctx, handle(ctx, desc)),
            1 => fns::un_802FD8C4(ctx, handle(ctx, desc)),
            _ => fns::un_802FD8E8(ctx, handle(ctx, desc)),
        }
        fns::un_802FD604(ctx, c.regs.gpr[21] as i32);
        if c.regs.gpr[22] == 1 {
            fns::un_802FD668(ctx);
        }
        let g = ctx.read_u32(0x804D_6D80);
        for i in 0..40u32 {
            fns::fn_802FD680(ctx, handle(ctx, g));
            let shown = ctx.read_u32(0x804D_6D8C) == 0 && ctx.read_u32(0x804D_6D88) != 0;
            ctx.write_u32(SCRATCH + 4 * i, u32::from(shown));
        }
        fns::un_802FD674(ctx);
        fns::un_802FD65C(ctx);
        fns::un_802FD704(ctx);
        fns::un_802FD90C(ctx);
        (0, 0)
    })
    .run_n(4);
}

#[test]
fn coin_proc() {
    Check {
        variants: 16,
        perturb: &|f| {
            let s = 0x803F_9E08;
            let b = f.u8(s) & 0x80
                | u8::from(f.below(4) != 0) << 6
                | (f.below(8) as u8) << 3
                | [0u8, 1, 1, 5][f.below(4) as usize];
            f.put8(s, b);
            {
                let v = [0u8, 9, 10, 19, 50][f.below(5) as usize];
                f.put8(s + 1, v);
            }
            {
                let v = [0u8, 10, 11, 100][f.below(4) as usize];
                f.put8(s + 2, v);
            }
            {
                let v = [0u32, 100, 320][f.below(3) as usize];
                f.put(s + 4, v);
            }
            {
                let v = [0u32, (-200i32) as u32][f.below(2) as usize];
                f.put(s + 8, v);
            }
        },
        natives: &[(sym("lbAudioAx_800237A8"), log_sfx as fn(&Ctx))],
        ..Check::new("fn_802FED14", &|r, a| {
            r.game.events.clear();
            r.coin_proc(gobj_arg(a, 3));
            vec![Out::Logged(sfx_logged(&r.game.events))]
        })
    }
    .run();
}

#[test]
fn coin_counter() {
    let natives = [(sym("lbArchive_80016DBC"), coget_loaded as fn(&Ctx))];
    Check {
        perturb_all: true,
        variants: 4,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(3);
            f.regs.gpr[21] = [0u32, 1, 9, 200][f.below(4) as usize];
        },
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            match a.r(20) {
                0 => r.init_coin_counter(a.archive("IfCoGet")),
                1 => r.game.start_coin_counter(100, 50, a.i(21), a.i(21) / 3),
                _ => r.game.stop_coin_counter(),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let n = c.regs.gpr[21] as i32;
        match c.regs.gpr[20] {
            0 => fns::un_802FF1B4(ctx),
            1 => fns::un_802FF128(ctx, 100, 50, n, n / 3),
            _ => fns::un_802FF190(ctx),
        }
        (0, 0)
    })
    .run_n(4);
}

#[test]
fn create_coin_counter() {
    Check::new("un_802FEFAC", &|r, _| {
        r.create_coin_counter();
        vec![]
    })
    .run();
}

fn count_native(ctx: &Ctx) {
    let n = ctx.read_u32(SCRATCH + 0x100);
    ctx.write_u32(SCRATCH + 0x100, n + 1);
}

#[test]
fn arrow_proc() {
    let natives = [(sym("lbAudioAx_80024030"), count_native as fn(&Ctx))];
    Check {
        natives: &natives,
        variants: 10,
        perturb: &|f| {
            let p = 0x804A_1F10;
            for i in 0..4 {
                let v = [0u8, 0, 1, 2][f.below(4) as usize];
                f.put8(p + 0x28 + i, v);
            }
            {
                let v = [0xFFu8, 0, 1, 2, 3][f.below(5) as usize];
                f.put8(p + 0x31, v);
            }
            {
                let v = [0u8, 1, 50][f.below(3) as usize];
                f.put8(p + 0x32, v);
            }
            {
                let v = f.below(2) as u8;
                f.put8(p + 0x30, v);
            }
        },
        ..Check::new("fn_802FDA78", &|r, a| {
            r.game.events.clear();
            r.arrow_proc(gobj_arg(a, 3));
            let n = r.game.events.len() as u32;
            vec![Out::R3(n)]
        })
    }
    .with_theirs(&|ctx, c| {
        ctx.write_u32(SCRATCH + 0x100, 0);
        fns::fn_802FDA78(ctx, handle(ctx, c.regs.gpr[3]));
        (ctx.read_u32(SCRATCH + 0x100), 0)
    })
    .run();
}

#[test]
fn arrow_render() {
    check_render("fn_802FDA4C", &|r, a| {
        r.game.arrow_render(gobj_arg(a, 3), a.i(4))
    });
}

#[test]
fn arrows() {
    Check {
        perturb_all: true,
        variants: 8,
        perturb: &|f| {
            f.regs.gpr[20] = f.below(6);
            f.regs.gpr[21] = f.below(5);
            f.regs.gpr[22] = f.below(3);
            let players = sym("player_slots");
            let s = f.regs.gpr[21].min(3);
            let k = f.below(2);
            f.put(players + 0xE90 * s + 0xC, k);
        },
        ..Check::new("fn_8016C7D0", &|r, a| {
            let slot = a.r(21) as u8;
            match a.r(20) {
                0 => r.game.show_arrow(slot, a.r(22) as u8, [1, 2, 3, 4]),
                1 => r.game.hide_arrow(slot),
                2 => r.game.pause_arrows(),
                3 => r.game.unpause_arrows(),
                4 => r.free_arrows(),
                _ => r.create_arrows(),
            }
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let slot = c.regs.gpr[21] as u8;
        match c.regs.gpr[20] {
            0 => {
                ctx.write_u32(SCRATCH, 0x0102_0304);
                fns::un_802FD928(ctx, slot, c.regs.gpr[22] as u8, handle(ctx, SCRATCH));
            }
            1 => fns::un_802FD9D8(ctx, slot),
            2 => fns::un_802FD910(ctx),
            3 => fns::un_802FD91C(ctx),
            4 => fns::un_802FE390(ctx),
            _ => fns::un_802FE260(ctx),
        }
        (0, 0)
    })
    .run_n(6);
}

#[test]
fn scores() {
    // The scores' canvas, a score made for a slot and run, hidden, shown and freed, from the
    // match's states; the results' scoring stood in for on both sides.
    let stubs = hud_stubs();
    Check {
        perturb_all: true,
        variants: 4,
        perturb: &|f| f.regs.gpr[20] = f.below(2),
        stubs: &stubs,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let slot = a.i(20);
            r.init_scores();
            r.create_score(slot);
            let g = r.game.hud.scores.x8[slot as usize].x0.unwrap();
            r.score_proc(g);
            r.game.hide_scores();
            r.score_proc(g);
            r.show_scores();
            r.free_scores();
            vec![]
        })
    }
    .with_theirs(&|ctx, c| {
        let slot = c.regs.gpr[20] as i32;
        fns::un_802FF498(ctx);
        fns::un_802FF364(ctx, slot);
        let g = ctx.read_u32(0x804A_1F58 + 8 + 0x14 * slot as u32);
        fns::fn_802FF218(ctx, handle(ctx, g));
        fns::un_802FF570(ctx);
        fns::fn_802FF218(ctx, handle(ctx, g));
        fns::un_802FF620(ctx);
        fns::un_802FF4FC(ctx);
        (0, 0)
    })
    .run_n(4);
}
