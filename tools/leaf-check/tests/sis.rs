//! HAL's text system, SisLib (`sysdolphin/baselib/sislib.c`, `hsd_3A64.c`), against
//! ssbm-rs's ports from real game states: the HUD's captures (see `hud.rs`) save its calls in
//! the menus and matches. Each check compares the whole state, the text arena's lists, texts,
//! strings and canvases included.

#[path = "support/calls.rs"]
mod calls;

use calls::{Args, Check, Out};
use leaf_check::{Rand, SCRATCH, handle};
use melee_capture::import::sym;
use melee_sim::hsd::sis::{self, SisPtr, TextId};
use ssbm_types::fns;

/// The text arena's address in a capture.
fn sis_base(a: &Args) -> u32 {
    a.word(sym("HSD_SisLib_804D796C"))
}

fn text_arg(a: &Args, r: usize) -> TextId {
    TextId(a.r(r) - sis_base(a))
}

#[test]
fn alloc() {
    Check {
        variants: 6,
        perturb: &|f| f.regs.gpr[3] = [1, 4, 0x10, 0x80, 0xA0, 0x7F][f.below(6) as usize],
        ..Check::new("HSD_SisLib_Alloc", &|r, a| {
            let d = r.game.sis.alloc(a.i(3));
            vec![Out::R3(sis_base(a) + d)]
        })
    }
    .with_theirs(&|ctx, c| {
        let p = fns::HSD_SisLib_Alloc(ctx, c.regs.gpr[3] as i32);
        (ssbm_rt::Handle::addr(p), 0)
    })
    .run();
}

#[test]
fn free() {
    Check::new("HSD_SisLib_Free", &|r, a| {
        r.game.sis.free(a.r(3).wrapping_sub(sis_base(a)));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        fns::HSD_SisLib_Free(ctx, handle(ctx, c.regs.gpr[3]));
        (0, 0)
    })
    .run();
}

#[test]
fn free_text() {
    Check::new("HSD_SisLib_803A5A2C", &|r, a| {
        r.game.sis.free_text(text_arg(a, 3));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        fns::HSD_SisLib_803A5A2C(ctx, handle(ctx, c.regs.gpr[3]));
        (0, 0)
    })
    .run();
}

#[test]
fn create_text() {
    Check::new("HSD_SisLib_803A5ACC", &|r, a| {
        let pos = melee_sim::vec::Vec3::new(a.f(1) as f32, a.f(2) as f32, a.f(3) as f32);
        let t = r.create_text(a.i(3), a.i(4), pos, a.f(4) as f32, a.f(5) as f32);
        vec![Out::R3(sis_base(a) + t.0)]
    })
    .with_theirs(&|ctx, c| {
        let (g, f) = (&c.regs.gpr, &c.regs.fpr);
        let t = fns::HSD_SisLib_803A5ACC(
            ctx,
            g[3] as i32,
            g[4] as i32,
            f[1].ps0,
            f[2].ps0,
            f[3].ps0,
            f[4].ps0,
            f[5].ps0,
        );
        (ssbm_rt::Handle::addr(t), 0)
    })
    .run();
}

#[test]
fn destroy_text() {
    Check::new("HSD_SisLib_803A5CC4", &|r, a| {
        r.destroy_text(text_arg(a, 3));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        fns::HSD_SisLib_803A5CC4(ctx, handle(ctx, c.regs.gpr[3]));
        (0, 0)
    })
    .run();
}

#[test]
fn destroy_all_texts() {
    Check {
        perturb_all: true,
        ..Check::new("HSD_SisLib_803A5CC4", &|r, _| {
            r.destroy_all_texts();
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        fns::HSD_SisLib_803A5D30(ctx);
        (0, 0)
    })
    .run();
}

#[test]
fn add_canvas() {
    Check::new("HSD_SisLib_803A611C", &|r, a| {
        let parent = (a.r(4) != 0).then(|| a.gobj(a.r(4)));
        let n = r.add_canvas(
            a.i(3),
            parent,
            a.r(5) as u16,
            a.r(6) as u8,
            a.r(7) as u8,
            a.r(8) as u8,
            a.r(9) as u8,
            a.r(10),
        );
        vec![Out::R3(n as u32)]
    })
    .with_theirs(&|ctx, c| {
        let g = &c.regs.gpr;
        let n = fns::HSD_SisLib_803A611C(
            ctx,
            g[3] as i32,
            handle(ctx, g[4]),
            g[5] as u16,
            g[6] as u8,
            g[7] as u8,
            g[8] as u8,
            g[9] as u8,
            g[10],
        );
        (n as u32, 0)
    })
    .run();
}

#[test]
fn reset_text() {
    Check::new("HSD_SisLib_803A6368", &|r, a| {
        r.game.sis.reset_text(r.data, text_arg(a, 3), a.i(4));
        vec![]
    })
    .with_theirs(&|ctx, c| {
        fns::HSD_SisLib_803A6368(ctx, handle(ctx, c.regs.gpr[3]), c.regs.gpr[4] as i32);
        (0, 0)
    })
    .run();
}

#[test]
fn create_text_buffer() {
    Check::new("HSD_SisLib_803A6754", &|r, a| {
        let t = r.create_text_buffer(a.i(3), a.i(4));
        vec![Out::R3(sis_base(a) + t.0)]
    })
    .with_theirs(&|ctx, c| {
        let t = fns::HSD_SisLib_803A6754(ctx, c.regs.gpr[3] as i32, c.regs.gpr[4] as i32);
        (ssbm_rt::Handle::addr(t), 0)
    })
    .run();
}

#[test]
fn encode_strings() {
    // Every printable ASCII character, runs of kerned and unkerned ones, and Shift-JIS pairs.
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    let marks = [b'.', b':', b',', b'-', b'"', 0x27];
    let mut r = Rand(11);
    for i in 0..3000 {
        let n = r.below(40) as usize;
        let mut s: Vec<u8> = (0..n)
            .map(|_| match r.below(6) {
                0 => b' ',
                1 => b'0' + r.below(10) as u8,
                2 => marks[r.below(6) as usize],
                3 => 0x20 + r.below(0x5F) as u8,
                4 => [0x81, 0x82][r.below(2) as usize],
                _ => b'a' + r.below(26) as u8,
            })
            .collect();
        if i == 0 {
            s = (0x20..0x7F).collect();
        }
        s.push(0);
        let at = SCRATCH + 0x1000;
        // The original reads on past a trailing lead byte; ours reads zeros there.
        for k in 0..0x100 {
            ctx.write_u8(at + k, 0);
        }
        for (k, &b) in s.iter().enumerate() {
            ctx.write_u8(at + k as u32, b);
        }
        let n = fns::HSD_SisLib_803A67EC(&ctx, handle(&ctx, SCRATCH), handle(&ctx, at)) as usize;
        let theirs: Vec<u8> = (0..=n as u32).map(|k| ctx.read_u8(SCRATCH + k)).collect();
        let mut out = [0u8; 0x200];
        let m = sis::encode(&mut out, &s);
        assert_eq!((m, &out[..=m]), (n, &theirs[..]), "{s:x?}");
    }
}

/// The captured match's name tag text (`un_804D6D78`), which every VS HUD makes.
fn match_text(a: &Args) -> TextId {
    TextId(a.word(sym("un_804D6D78")) - sis_base(a))
}

/// A new arena (`HSD_SisLib_803A6048`) in a match.
#[test]
fn init() {
    Check {
        perturb_all: true,
        // The texts go first (their GObjs would keep pointers into the old arena), as a
        // scene's end takes them.
        ..Check::new("fn_8016C7D0", &|r, _| {
            r.destroy_all_texts();
            r.game.sis.init(0x4800);
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        fns::HSD_SisLib_803A5D30(ctx);
        fns::HSD_SisLib_803A6048(ctx, 0x4800);
        (0, 0)
    })
    .run_n(4);
}

/// An operation on a text's strings.
#[derive(Clone, Debug)]
enum Op {
    /// Appends a string at (x, y), given as the format `HSD_SisLib_803A6B98` takes (no
    /// conversions, or one `%d` with its argument).
    Add(Vec<u8>, Option<i32>, f32, f32),
    Replace(i32, Option<i32>),
    Position(i32, f32, f32),
    Scale(i32, f32, f32),
    Color(i32, [u8; 3]),
    Clear(i32),
    ClearText,
    /// `fn_803A6FEC` on the text's string, with and without the size.
    Find(i32, bool),
}

/// A random string of what names and HUD strings hold: letters, digits, marks, spaces and
/// Shift-JIS pairs from the font's table.
fn string(r: &mut Rand) -> Vec<u8> {
    let marks = [b'.', b':', b',', b'-', b'"', 0x27, b' '];
    let n = r.below(24) as usize;
    let mut s = Vec::new();
    for _ in 0..n {
        match r.below(5) {
            0 => s.push(b'0' + r.below(10) as u8),
            1 => s.push(marks[r.below(7) as usize]),
            2 => s.extend_from_slice(&sis::SJIS[r.below(287) as usize]),
            3 => s.push(b'A' + r.below(26) as u8),
            _ => s.push(b'a' + r.below(26) as u8),
        }
    }
    s
}

fn ops(seed: u32) -> Vec<Op> {
    let mut r = Rand(u64::from(seed) | 1);
    let xs = [-5000.0f32, 0.0, 37.5, 639.9, -1.5, 33000.0];
    // Strings that grow and shrink in place and past their buffer.
    let mut out = vec![
        Op::Add(b"x%d".to_vec(), Some(1), 0.0, 0.0),
        Op::Add(b"tail".to_vec(), None, 1.0, 1.0),
        Op::Replace(0, Some(123_456_789)),
        Op::Replace(0, Some(5)),
        Op::Add([b'w'; 60].to_vec(), None, 2.0, 2.0),
        Op::Replace(1, Some(-123_456_789)),
        Op::Replace(0, Some(-123_456_789)),
    ];
    for _ in 0..40 {
        let k = r.below(4) as i32 - 1;
        let n = [0, 7, 99_999, 12, 123_456_789, -4][r.below(6) as usize];
        out.push(match r.below(10) {
            0..=2 => {
                let fmt = r.below(3) == 0;
                let mut s = string(&mut r);
                if fmt {
                    s.extend_from_slice(b"%d");
                }
                Op::Add(
                    s,
                    fmt.then_some(n),
                    xs[r.below(6) as usize],
                    xs[r.below(6) as usize],
                )
            }
            3 => Op::Replace(k, (r.below(4) != 0).then_some(n)),
            4 => Op::Position(k, xs[r.below(6) as usize], xs[r.below(6) as usize]),
            5 => Op::Scale(k, r.range(0.0, 4.0), r.range(0.0, 4.0)),
            6 => Op::Color(k, (r.bits() as u32).to_be_bytes()[..3].try_into().unwrap()),
            7 => Op::Clear(k),
            8 if r.below(4) == 0 => Op::ClearText,
            _ => Op::Find(k, r.below(2) == 0),
        });
    }
    out
}

/// Where ssbm-rs's side writes the operations' results, which ours reports.
const RESULTS: u32 = SCRATCH + 0x8000;
const FORMAT: u32 = SCRATCH + 0x9000;

#[test]
fn text_ops() {
    Check {
        perturb_all: true,
        variants: 6,
        perturb: &|f| f.regs.gpr[20] = f.below(u64::from(u32::MAX)),
        ..Check::new("fn_8016C7D0", &|r, a| {
            let t = match_text(a);
            let base = sis_base(a);
            let d = r.data;
            let s = &mut r.game.sis;
            let mut results = Vec::new();
            for op in ops(a.r(20)) {
                match op {
                    Op::Add(fmt, n, x, y) => {
                        let v = sis::format(&fmt, &n.into_iter().collect::<Vec<_>>());
                        let k = s.add_string(t, f64::from(x), f64::from(y), Some(&v));
                        results.push(k as u32);
                    }
                    Op::Replace(k, n) => {
                        let v = n.map(|n| sis::format(b"%d", &[n]));
                        results.push(s.replace_string(d, t, k, v.as_deref()) as u32);
                    }
                    Op::Position(k, x, y) => s.set_position(d, t, k, f64::from(x), f64::from(y)),
                    Op::Scale(k, x, y) => s.set_scale(d, t, k, f64::from(x), f64::from(y)),
                    Op::Color(k, c) => s.set_color(d, t, k, c),
                    Op::Clear(k) => s.clear_string(d, t, k),
                    Op::ClearText => s.clear_text(t),
                    Op::Find(k, size) => {
                        let p = s.text(t).sis_buffer.expect("a string");
                        let found = s.find_entry(d, p, k, size);
                        let at = found.map_or(0, |(p, _)| match p {
                            SisPtr::Arena(o) => base + o,
                            SisPtr::Data(x) => a.data.addr(x),
                            SisPtr::Static(x) => x,
                        });
                        results.push(at);
                        if size {
                            results.push(found.map_or(0, |(_, n)| n as u32));
                        }
                    }
                }
            }
            vec![Out::Words(RESULTS, results)]
        })
    }
    .with_theirs(&|ctx, c| {
        let t = handle(ctx, ctx.read_u32(sym("un_804D6D78")));
        let mut at = RESULTS;
        let mut put = |v: u32| {
            ctx.write_u32(at, v);
            at += 4;
        };
        let size_out = SCRATCH + 0x9800;
        for op in ops(c.regs.gpr[20]) {
            match op {
                Op::Add(fmt, n, x, y) => {
                    for (i, &b) in fmt.iter().chain(&[0]).enumerate() {
                        ctx.write_u8(FORMAT + i as u32, b);
                    }
                    let args: Vec<ssbm_rt::VarArg> = n
                        .into_iter()
                        .map(|n| ssbm_rt::VarArg::Int(n as u32))
                        .collect();
                    let k = fns::HSD_SisLib_803A6B98(
                        ctx,
                        t,
                        f64::from(x),
                        f64::from(y),
                        handle(ctx, FORMAT),
                        &args,
                    );
                    put(k as u32);
                }
                Op::Replace(k, n) => {
                    for (i, &b) in b"%d\0".iter().enumerate() {
                        ctx.write_u8(FORMAT + i as u32, b);
                    }
                    let fmt = if n.is_some() { FORMAT } else { 0 };
                    let args: Vec<ssbm_rt::VarArg> = n
                        .into_iter()
                        .map(|n| ssbm_rt::VarArg::Int(n as u32))
                        .collect();
                    let r = fns::HSD_SisLib_803A70A0(ctx, t, k, handle(ctx, fmt), &args);
                    put(r as u32);
                }
                Op::Position(k, x, y) => {
                    fns::HSD_SisLib_803A746C(ctx, t, k, f64::from(x), f64::from(y))
                }
                Op::Scale(k, x, y) => {
                    fns::HSD_SisLib_803A7548(ctx, t, k, f64::from(x), f64::from(y))
                }
                Op::Color(k, rgb) => {
                    let col = SCRATCH + 0x9900;
                    for (i, &b) in rgb.iter().enumerate() {
                        ctx.write_u8(col + i as u32, b);
                    }
                    fns::HSD_SisLib_803A74F0(ctx, t, k, handle(ctx, col));
                }
                Op::Clear(k) => fns::HSD_SisLib_803A75E0(ctx, t, k),
                Op::ClearText => fns::HSD_SisLib_803A7664(ctx, t),
                Op::Find(k, size) => {
                    let buf = ctx.read_u32(ssbm_rt::Handle::addr(t) + 0x5C);
                    let out = if size { size_out } else { 0 };
                    let p = fns::fn_803A6FEC(ctx, handle(ctx, buf), k, handle(ctx, out));
                    put(ssbm_rt::Handle::addr(p));
                    if size {
                        put(if ssbm_rt::Handle::addr(p) != 0 {
                            ctx.read_u32(size_out)
                        } else {
                            0
                        });
                    }
                }
            }
        }
        (0, 0)
    })
    .run_n(6);
}

/// Random allocations and frees in a match's text arena: sizes that tie, so best fits
/// choose between equal blocks.
#[test]
fn heap_ops() {
    fn steps(seed: u32) -> Vec<(bool, i32, usize)> {
        let mut r = Rand(u64::from(seed) | 1);
        (0..80)
            .map(|_| {
                (
                    r.below(3) != 0,
                    [4, 8, 12, 16, 0x20, 0x80, 0xA0][r.below(7) as usize],
                    r.below(64) as usize,
                )
            })
            .collect()
    }
    Check {
        perturb_all: true,
        variants: 6,
        perturb: &|f| f.regs.gpr[20] = f.below(u64::from(u32::MAX)),
        ..Check::new("fn_8016C7D0", &|r, a| {
            let base = sis_base(a);
            let s = &mut r.game.sis;
            let mut live: Vec<u32> = Vec::new();
            let mut got = Vec::new();
            for (alloc, size, pick) in steps(a.r(20)) {
                if alloc || live.is_empty() {
                    let d = s.alloc(size);
                    got.push(base + d);
                    live.push(d);
                } else {
                    let d = live.remove(pick % live.len());
                    s.free(d);
                }
            }
            vec![Out::Words(RESULTS, got)]
        })
    }
    .with_theirs(&|ctx, c| {
        let mut live: Vec<u32> = Vec::new();
        let mut at = RESULTS;
        for (alloc, size, pick) in steps(c.regs.gpr[20]) {
            if alloc || live.is_empty() {
                let p = ssbm_rt::Handle::addr(fns::HSD_SisLib_Alloc(ctx, size));
                ctx.write_u32(at, p);
                at += 4;
                live.push(p);
            } else {
                let p = live.remove(pick % live.len());
                fns::HSD_SisLib_Free(ctx, handle(ctx, p));
            }
        }
        (0, 0)
    })
    .run_n(6);
}

/// A canvas with a camera of its own, whose GObj is then freed (`fn_803A60EC` forgets it).
#[test]
fn canvas_camera() {
    Check {
        perturb_all: true,
        ..Check::new("fn_8016C7D0", &|r, _| {
            r.add_canvas(3, None, 0x13, 0x14, 0, 0x11, 0, 0x50);
            let c = r.game.sis.first_canvas;
            let mut last = c.unwrap();
            while let Some(n) = r.game.sis.canvas(last).next {
                last = n;
            }
            let g = r.game.sis.canvas(last).gobj.expect("a camera");
            melee_sim::hsd::gobj::free(r, g);
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        fns::HSD_SisLib_803A611C(ctx, 3, handle(ctx, 0), 0x13, 0x14, 0, 0x11, 0, 0x50);
        let mut c = ctx.read_u32(sym("HSD_SisLib_804D797C"));
        while ctx.read_u32(c) != 0 {
            c = ctx.read_u32(c);
        }
        let g = ctx.read_u32(c + 4);
        fns::HSD_GObjFree(ctx, handle(ctx, g));
        (0, 0)
    })
    .run_n(3);
}

/// Their stand-in for `HSD_SisLib_803A945C` (loading a font's archive by name): the match has
/// loaded `SdIntro.dat` as font 2 already, so it returns that archive.
fn loaded_font(ctx: &ssbm_rt::Ctx) {
    let a = ctx.read_u32(SCRATCH + 0xA200);
    ctx.regs.set_r(3, a);
}

#[test]
fn load_font() {
    let natives = [(sym("HSD_SisLib_803A945C"), loaded_font as fn(&ssbm_rt::Ctx))];
    Check {
        perturb_all: true,
        natives: &natives,
        ..Check::new("fn_8016C7D0", &|r, a| {
            let sd = a.archive("SdIntro");
            r.load_font(2, sd, "SIS_IntroData");
            vec![]
        })
    }
    .with_theirs(&|ctx, _| {
        let name = SCRATCH + 0xA000;
        let sym_at = SCRATCH + 0xA100;
        for (i, b) in b"SdIntro.dat\0".iter().enumerate() {
            ctx.write_u8(name + i as u32, *b);
        }
        for (i, b) in b"SIS_IntroData\0".iter().enumerate() {
            ctx.write_u8(sym_at + i as u32, *b);
        }
        // A fresh slot, as before the original loads it.
        let a = ctx.read_u32(sym("HSD_SisLib_804D1110") + 8);
        ctx.write_u32(sym("HSD_SisLib_804D1110") + 8, 0);
        ctx.write_u32(sym("HSD_SisLib_804D1124") + 8, 0);
        ctx.write_u32(SCRATCH + 0xA200, a);
        fns::HSD_SisLib_803A62A0(ctx, 2, handle(ctx, name), handle(ctx, sym_at));
        (0, 0)
    })
    .run_n(3);
}
