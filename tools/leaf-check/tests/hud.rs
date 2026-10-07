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

fn draw_natives() -> Vec<(u32, fn(&Ctx))> {
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
