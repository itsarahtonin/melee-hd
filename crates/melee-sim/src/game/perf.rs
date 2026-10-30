//! The scene loop's debug reports at the debug ROM's level (`gm_801A4970`: X held and D-pad
//! right on a controller): the memory report, which only prints to the debugger's console, and
//! the performance monitor the same press turns on and off (`gm_80165388`, `hsd_80398310`,
//! `hsd_392A.c`, `hsd_3924.c`).
//!
//! The report (`lbHeap_80015DF8`, `hsdDumpClassStat`, `HSD_ObjDumpStat`,
//! `db_PrintEntityCounts`, `db_PrintThreadInfo`, `HSD_Leak_80387DF8`) reads the heaps, the
//! classes, the pools, the GObj lists and the stack and hands lines to `OSReport`, which
//! the simulation leaves out ([`super::code::os_report`]); it writes nothing else
//! (`docs/engine.md`, "The scene loop's reports").
//!
//! The monitor is a camera GObj whose render callback (`fn_803982E4`) draws a strip at the
//! bottom of the screen from a list of display callbacks (`hsd_804D7850`, `hsd_80392528`):
//! the perf timer's bars and times (`fn_80392A3C`), the frame rate and the scene's game
//! ticks (`fn_801652D8`), and in the scenes that hook it (`gm_804D6724`) the movie's counts
//! (`fn_801A9FCC`). The GObj, its camera and the list's blocks are state; the times it draws
//! are the host's. On the console they are CPU and GPU time; here the timeline's (the clock's
//! `perf_last`, as ssbm-rs's virtual time gives them) feed the drawing in checks, and the
//! player hands the renderer its own times instead ([`Shown`]), which never reach the state.

use gekko_fp as fp;

use super::handles::CodeRef;
use super::hud::devtext::Draw;
use super::state::{OBJ_CAMERA, Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::cobj::{self, StaticDesc};
use crate::hsd::gobj::GObjId;
use crate::hsd::heap::{self, Kind};
use crate::msl::{self, Arg};
use crate::vec::Vec3;

/// `fn_803982E4`, the monitor's camera's render callback.
pub const RENDER: u32 = 0x8039_82E4;

/// `lbl_8040BF70`: the monitor's orthographic camera, with its eye (`lbl_8040BF48`) and
/// interest (`lbl_8040BF5C`).
pub const CAMERA_DESC: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 3,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 1.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.0,
    far: 32768.0,
    projection: [-445.0, 35.0, -20.0, 620.0],
};

/// A display callback on the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Item {
    /// `fn_80392A3C`: the perf timer's bars and times.
    Bars,
    /// `fn_801652D8`: the frame rate and the scene's game ticks.
    Scene,
    /// `fn_801A9FCC`: the movie player's counts (the opening's hook, `fn_801AA0E8`).
    Movie,
}

impl Item {
    /// The callback's address.
    pub fn code(self) -> u32 {
        match self {
            Item::Bars => 0x8039_2A3C,
            Item::Scene => 0x8016_52D8,
            Item::Movie => 0x801A_9FCC,
        }
    }
}

/// A list entry (`hsd_804D7850`'s nodes): its callback, and the original's addresses of its
/// event and priority block (`HSD_MemAlloc`) and its list node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Entry {
    pub item: Item,
    pub block: u32,
    pub node: u32,
}

/// The times the monitor last drew (`hsd_804D7858` to `7860`: CPU, draw, total), each one's
/// highest over the last 60 draws and its countdown (`hsd_804D7864` to `786C`, `7870` to
/// `7878`), and the highest since `fn_80392A08` enabled them (`hsd_804D787C` to `7884`, shown
/// while `hsd_804D7888` is set, which nothing does).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Stats {
    pub now: [f32; 3],
    pub peak: [f32; 3],
    pub hold: [i32; 3],
    pub best: [f32; 3],
}

impl Stats {
    /// `fn_80392934`: the last frame's times taken, each peak held for 60 draws unless beaten.
    #[doc(alias = "fn_80392934")]
    pub fn update(&mut self, times: [f32; 3]) {
        self.now = times;
        for ((peak, hold), &t) in self.peak.iter_mut().zip(&mut self.hold).zip(&times) {
            let beaten = *peak < t || {
                let h = *hold;
                *hold = h.wrapping_sub(1);
                h < 0
            };
            if beaten {
                *peak = t;
                *hold = 60;
            }
        }
        for (best, &t) in self.best.iter_mut().zip(&times) {
            if *best < t {
                *best = t;
            }
        }
    }
}

/// The monitor's state: its GObj (`gm_804D6728`), the display callbacks' list and the drawn
/// times.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Monitor {
    pub gobj: Option<GObjId>,
    pub events: Vec<Entry>,
    pub stats: Stats,
}

/// What the monitor's drawing shows besides its list: the times and frame rate it draws, and
/// the game's values its lines print.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub stats: Stats,
    /// `HSD_VIData.perf.frame_renew`: new frames shown over the last 60 retraces.
    pub frame_renew: i32,
}

/// One drawing of the monitor (`fn_803982E4`, after its camera): its list, what the timeline
/// gives it, and the game's values its lines print.
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct PerfDraw {
    pub items: Vec<Item>,
    /// The drawn times and frame rate as the timeline gives them.
    pub stats: Stats,
    pub frame_renew: i32,
    /// `gm_801A4BB8`: the scene's game ticks.
    pub game_ticks: u32,
    /// The movie player's frames buffered at their fewest and now (`lbMthp_8001F5F4`,
    /// `lbMthp_8001F5E4`) and its last read's time (`lbMthp_8001F5D4`).
    pub movie: [i32; 3],
}

impl PerfDraw {
    /// The timeline's times and frame rate.
    pub fn timeline(&self) -> Shown {
        Shown {
            stats: self.stats,
            frame_renew: self.frame_renew,
        }
    }
}

/// `lbl_804D6088`, `lbl_804D608C`: the bars' frames and the times' line, as `fn_80392A08(4, 1,
/// 0)` sets them (their start values too).
const FRAMES: i32 = 4;

/// `lbl_804D6094` to `60A4`: red, green, cyan, blue-grey and yellow; `lbl_804D6090` white.
const RED: u32 = 0xFF00_00FF;
const GREEN: u32 = 0x00FF_00FF;
const CYAN: u32 = 0x00FF_FFFF;
const GREY_BLUE: u32 = 0x0080_80FF;
const YELLOW: u32 = 0xC0C0_00FF;
const WHITE: u32 = 0xFFFF_FFFF;

/// A display item (`PerfDispItem`, `PerfLabelLine`).
enum DispItem {
    /// Type 0: a line of text.
    Text(Vec<u8>),
    /// Type 1: bars, each a count of units and a colour.
    Bars(Vec<(i32, u32)>),
    /// Type 2: a line coloured in runs to each position (a fraction of its length).
    Gradient(Vec<(f32, u32)>),
}

fn rgba(c: u32) -> [u8; 4] {
    c.to_be_bytes()
}

fn sprintf(format: &[u8], args: &[Arg]) -> Vec<u8> {
    msl::vsprintf(format, args)
}

/// `fn_80392A3C`: the bars of [`FRAMES`] frames (one green, up to three yellow, the rest
/// red), the total, draw and CPU times as lines over them, and the times' text.
#[doc(alias = "fn_80392A3C")]
fn bars_items(stats: &Stats) -> Vec<DispItem> {
    let n = FRAMES;
    let frames = reg(n as f32);
    let [cpu, draw, total] = stats.now.map(reg);
    let mut bars = vec![(1, GREEN)];
    if n > 1 {
        bars.push(((n - 1).min(3), YELLOW));
    }
    if n > 4 {
        bars.push((n - 4, RED));
    }
    let whole = fp::fctiwz(fp::fadds(reg(0.9999), total));
    let mut out = vec![
        DispItem::Bars(bars),
        DispItem::Gradient(vec![
            (mem(fp::fdivs(total, frames)), CYAN),
            (
                mem(fp::fdivs(fp::frsp(f64::from(whole)), frames)),
                GREY_BLUE,
            ),
        ]),
        DispItem::Gradient(vec![(mem(fp::fdivs(draw, frames)), WHITE)]),
        DispItem::Gradient(vec![(mem(fp::fdivs(cpu, frames)), GREEN)]),
    ];
    let f = |v: f32| Arg::Float(reg(v));
    let [pc, pd, pt] = stats.peak;
    out.push(DispItem::Text(sprintf(
        b"\\c00ff00%2.3f \\cffffff%2.3f \\c00ffff%2.3f  \\c00ff00%2.3f \\cffffff%2.3f \\c00ffff%2.3f",
        &[
            f(stats.now[0]),
            f(stats.now[1]),
            f(stats.now[2]),
            f(pc),
            f(pd),
            f(pt),
        ],
    )));
    out
}

/// `fn_801652D8`: the frame rate (yellow) and the scene's game ticks. Its second line's type
/// is never set: it reads 0, a text line, from `.bss` ([Melee's bugs](docs/melee-bugs.md)).
#[doc(alias = "fn_801652D8")]
fn scene_items(frame_renew: i32, game_ticks: u32) -> Vec<DispItem> {
    vec![
        DispItem::Text(sprintf(b"\\cffff00%2d", &[Arg::Int(frame_renew)])),
        DispItem::Text(sprintf(b"\\ce0e0ff%5d", &[Arg::Int(game_ticks as i32)])),
    ]
}

/// `fn_801A9FCC`: the movie's frames buffered at their fewest and now, and its last read's
/// time in milliseconds.
#[doc(alias = "fn_801A9FCC")]
#[doc(alias = "lbMthp_8001F5D4")]
#[doc(alias = "lbMthp_8001F5E4")]
#[doc(alias = "lbMthp_8001F5F4")]
fn movie_items([fewest, buffered, read]: [i32; 3]) -> Vec<DispItem> {
    let ms = fp::fdivs(fp::frsp(f64::from(read as u32)), 40_500.0);
    let ms = fp::fctiwz(ms) as u32;
    vec![
        DispItem::Text(sprintf(b"\\cffff00%2d", &[Arg::Int(fewest)])),
        DispItem::Text(sprintf(b"\\cffff00%2d", &[Arg::Int(buffered)])),
        DispItem::Text(sprintf(b"\\cffff00%3d", &[Arg::Int(ms as i32)])),
    ]
}

/// The characters a line draws, past its colour codes.
fn text_chars(text: &[u8]) -> i32 {
    let mut count = 0;
    let mut i = 0;
    while i < text.len() {
        if text[i] != b'\\' {
            count += 1;
        } else {
            i += 1;
            if matches!(text.get(i), Some(b'c' | b'C')) {
                i += 6;
            }
        }
        i += 1;
    }
    count
}

/// `hsd_80391AC8`'s hex digit, as its code reads one: a decimal digit counts as 0 (it tests
/// the character itself against 10, [Melee's bugs](docs/melee-bugs.md)).
fn hexval(ch: i32) -> i32 {
    if ch < 0x47 {
        if ch < 0x0A {
            if ch >= 0 { ch - 0x30 } else { 0 }
        } else if ch >= 0x41 {
            ch - 0x37
        } else {
            0
        }
    } else if (0x61..0x67).contains(&ch) {
        ch - 0x57
    } else {
        0
    }
}

/// `hsd_80391AC8`: a line's glyphs from (`x`, `y`) on, `\cRRGGBB` setting the colour.
#[doc(alias = "hsd_80391AC8")]
fn text_draws(text: &[u8], color: &mut [u8; 4], x: f32, y: f32, out: &mut Vec<Draw>) {
    let mut x = reg(x);
    let mut i = 0;
    let at = |i: usize| text.get(i).map_or(0, |&c| c as i8 as i32);
    while at(i) != 0 {
        if at(i) != i32::from(b'\\') {
            out.push(Draw::Ascii {
                chr: at(i),
                x,
                y: reg(y),
                color: *color,
            });
            // `DrawASCII` returns the cell's width, `lbl_804D6070` (10 here).
            x = fp::fadds(x, 10.0);
        } else {
            i += 1;
            if matches!(at(i), 0x63 | 0x43) {
                let h = |k: usize| hexval(at(i + k));
                color[0] = (h(2) + h(1) * 16) as u8;
                color[1] = (h(4) + h(3) * 16) as u8;
                color[2] = (h(6) + h(5) * 16) as u8;
                i += 6;
                color[3] = 0xFF;
            }
        }
        i += 1;
    }
}

/// `hsd_80391E18`: a line from (`x1`, `y1`) to (`x2`, `y2`) in runs, each to its position
/// along it in its colour, until a negative position.
#[doc(alias = "hsd_80391E18")]
fn gradient_draws(list: &[(f32, u32)], x1: f32, y1: f32, x2: f32, y2: f32, out: &mut Vec<Draw>) {
    let (mut px, mut py) = (x1, y1);
    let dx = mem(fp::fsubs(reg(x2), reg(x1)));
    let dy = mem(fp::fsubs(reg(y2), reg(y1)));
    for &(t, c) in list {
        let t = reg(t);
        if t < 0.0 {
            break;
        }
        let a = [px, py];
        px = mem(fp::fmadds(t, reg(dx), reg(x1)));
        py = mem(fp::fmadds(t, reg(dy), reg(y1)));
        out.push(Draw::Line {
            a,
            b: [px, py],
            color: rgba(c),
        });
    }
}

/// `hsd_80391F28`'s length: `sqrtf` by three Newton steps from the estimate.
fn length(dy: f64, dx: f64) -> f64 {
    let len = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
    if len > 0.0 {
        let mut e = fp::frsqrte(len);
        for _ in 0..3 {
            e = fp::fmul(fp::fmul(0.5, e), fp::fnmsub(len, fp::fmul(e, e), 3.0));
        }
        fp::frsp(fp::fmul(len, e))
    } else {
        len
    }
}

/// `hsd_80391F28`: a bar from (`x1`, `y1`) to (`x2`, `y2`) with `count` steps marked across
/// it, each fifth longer.
#[doc(alias = "hsd_80391F28")]
fn bar_draws(color: [u8; 4], x1: f32, y1: f32, x2: f32, y2: f32, count: f32, out: &mut Vec<Draw>) {
    let dy = fp::fsubs(reg(y2), reg(y1));
    let dx = fp::fsubs(reg(x2), reg(x1));
    let len = length(dy, dx);
    let perp_x = fp::fdivs(fp::fneg(dy), len);
    let perp_y = fp::fdivs(fp::fneg(dx), len);
    let t6y = fp::fmuls(6.0, perp_y);
    let t6x = fp::fmuls(6.0, perp_x);
    let t4x = fp::fmuls(4.0, perp_x);
    let t4y = fp::fmuls(4.0, perp_y);
    out.push(Draw::Line {
        a: [x1, y1],
        b: [x2, y2],
        color,
    });
    let (mut x, mut y) = (reg(x1), reg(y1));
    let count = reg(count);
    let mut i = 0i32;
    while fp::frsp(f64::from(i)) <= count {
        let (tx, ty) = if i % 5 == 0 { (t6x, t6y) } else { (t4x, t4y) };
        out.push(Draw::Line {
            a: [mem(x), mem(y)],
            b: [mem(fp::fadds(x, tx)), mem(fp::fadds(y, ty))],
            color,
        });
        x = fp::fadds(x, fp::fdivs(dx, count));
        y = fp::fadds(y, fp::fdivs(dy, count));
        i += 1;
    }
}

/// `hsd_8039254C`: each display callback's items in list order, laid out in rows up from the
/// bottom: text in columns of ten units, wrapping at 60 characters; a bar or a line a half
/// row each, across 600 units. The rows' background (`lbl_804D6080`) has alpha 0, so none is
/// drawn.
#[doc(alias = "hsd_8039254C")]
#[doc(alias = "fn_803982E4")]
pub fn layout(d: &PerfDraw, shown: &Shown) -> Vec<Draw> {
    let mut out = Vec::new();
    let mut col = 60i32;
    let mut line = reg(1.0);
    for item in &d.items {
        let items = match item {
            Item::Bars => bars_items(&shown.stats),
            Item::Scene => scene_items(shown.frame_renew, d.game_ticks),
            Item::Movie => movie_items(d.movie),
        };
        for it in items {
            match it {
                DispItem::Text(text) => {
                    let n = text_chars(&text);
                    if col + n > 60 {
                        line = fp::fsubs(line, 1.0);
                        col = 0;
                    }
                    out.push(Draw::TextScale {
                        x: 10.0,
                        y: 10.0,
                        line_width: 6,
                    });
                    let mut color = [0xFF; 4];
                    let x = mem(fp::frsp(f64::from(col * 10)));
                    text_draws(&text, &mut color, x, mem(fp::fmuls(10.0, line)), &mut out);
                    col += n + 2;
                }
                DispItem::Gradient(list) => {
                    if col != 0 {
                        line = fp::frsp(fp::fsub(line, 0.5));
                    }
                    out.push(Draw::TextScale {
                        x: 10.0,
                        y: 10.0,
                        line_width: 24,
                    });
                    let y = mem(fp::fmadds(10.0, line, 2.0));
                    gradient_draws(&list, 0.0, y, 600.0, y, &mut out);
                    col = 60;
                }
                DispItem::Bars(bars) => {
                    let units: i32 = bars.iter().map(|b| b.0).take_while(|&c| c > 0).sum();
                    if units > 0 {
                        if col != 0 {
                            line = fp::frsp(fp::fsub(line, 0.5));
                        }
                        out.push(Draw::TextScale {
                            x: 10.0,
                            y: 10.0,
                            line_width: 12,
                        });
                        let y = mem(fp::fmadds(10.0, line, 2.0));
                        let mut x = reg(0.0);
                        let step = fp::fdivs(600.0, fp::frsp(f64::from(units)));
                        for &(count, c) in bars.iter().take_while(|b| b.0 > 0) {
                            let prev = mem(x);
                            x = fp::fmadds(step, fp::frsp(f64::from(count)), x);
                            let count = mem(fp::frsp(f64::from(count)));
                            bar_draws(rgba(c), prev, y, mem(x), y, count, &mut out);
                        }
                        col = 60;
                    }
                }
            }
        }
    }
    out
}

impl Running<'_> {
    /// `gm_801A4970`'s report, for each controller that pressed D-pad right with X held:
    /// the memory report (printed only, [`super::code::os_report`]), then the monitor off if
    /// it's on, else on, with the scene's hook's lines (`gm_804D6724`).
    #[doc(alias = "lbHeap_80015DF8")]
    #[doc(alias = "lbMemory_80014F7C")]
    #[doc(alias = "hsdDumpClassStat")]
    #[doc(alias = "OSReport_PrintSpaces")]
    #[doc(alias = "HSD_ObjDumpStat")]
    #[doc(alias = "db_PrintEntityCounts")]
    #[doc(alias = "db_PrintThreadInfo")]
    #[doc(alias = "HSD_Leak_80387DF8")]
    #[doc(alias = "OSCheckHeap")]
    #[doc(alias = "HSD_FObjGetAllocData")]
    #[doc(alias = "HSD_IDGetAllocData")]
    #[doc(alias = "HSD_SListGetAllocData")]
    #[doc(alias = "HSD_DListGetAllocData")]
    #[doc(alias = "HSD_VecGetAllocData")]
    #[doc(alias = "HSD_MtxGetAllocData")]
    #[doc(alias = "HSD_RObjGetAllocData")]
    #[doc(alias = "HSD_RvalueObjGetAllocData")]
    #[doc(alias = "HSD_ShadowGetAllocData")]
    #[doc(alias = "HSD_RenderGetAllocData")]
    #[doc(alias = "HSD_ChanGetAllocData")]
    #[doc(alias = "HSD_TevRegGetAllocData")]
    #[doc(alias = "fn_801AA0E8")]
    pub fn db_scene_report(&mut self) {
        use crate::input::pad::{BUTTON_RIGHT, BUTTON_X};
        for i in 0..4 {
            let p = &self.game.pads.master[i];
            if p.trigger & u32::from(BUTTON_RIGHT) == 0 || p.button & u32::from(BUTTON_X) == 0 {
                continue;
            }
            // The report: every line to `OSReport`; the leak checker was never set up
            // (`HSD_Leak_80407B58.table` is null), so it says so and returns.
            super::code::os_report();
            match self.game.perf.gobj.take() {
                Some(g) => self.perf_free(g),
                None => {
                    let g = self.perf_create();
                    self.game.perf.gobj = Some(g);
                    // `fn_801AA0E8`, the hook the opening (and the movies' scenes) set.
                    if self.game.opening.perf_display {
                        self.game.perf_register(Item::Movie);
                    }
                }
            }
        }
    }

    /// `gm_80165388(0x19, 0x3F, 0, 0xFE)`: the monitor's camera GObj (`hsd_80398310`: its
    /// camera from [`CAMERA_DESC`], drawn last at priority 0xFE, the bars' callback listed and
    /// `fn_80392A08(4, 1, 0)`, which sets what its statics start as), then the scene's line.
    #[doc(alias = "gm_80165388")]
    #[doc(alias = "hsd_80398310")]
    #[doc(alias = "fn_80392A08")]
    fn perf_create(&mut self) -> GObjId {
        let g = self.game.world.create(0x19, 0x3F, 0);
        let c = cobj::load_static_desc(&mut self.game.objs, self.data, &CAMERA_DESC);
        self.game.world.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
        self.game
            .world
            .setup_gx_link_max(g, CodeRef(RENDER), 0xFE, false);
        self.game.perf_register(Item::Bars);
        self.game.perf_register(Item::Scene);
        g
    }

    /// `gm_801653C8`: the monitor's GObj freed. Its callbacks stay listed.
    #[doc(alias = "gm_801653C8")]
    fn perf_free(&mut self, g: GObjId) {
        crate::hsd::gobj::free(self, g);
    }

    /// The monitor's render callback, `fn_803982E4`: its camera current (never ended), the
    /// timeline's last frame's times taken (`fn_80392934`), and the list drawn.
    pub fn render_perf(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            other => panic!("the performance monitor on {other:?}"),
        };
        crate::hsd::display::camera_begin(&mut self.game.objs, self.data, c);
        let (last, frame_renew) = self
            .game
            .audio
            .clock
            .as_ref()
            .map_or(([0.0; 3], 0), |c| (c.perf_last(), c.frame_renew));
        self.game.perf.stats.update(last);
        let m = &self.game.audio.movie;
        let d = PerfDraw {
            items: self.game.perf.events.iter().map(|e| e.item).collect(),
            stats: self.game.perf.stats,
            frame_renew,
            game_ticks: self.game.scene_game_ticks(),
            movie: [m.min_buffered, m.buffered, m.read_time],
        };
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::Perf(Box::new(d)));
    }
}

impl super::state::Game {
    /// Lists a display callback once, after those of no higher priority (all are 0x80):
    /// `hsd_80392528`, `fn_80392480`. A new entry takes an event and priority block
    /// (`HSD_MemAlloc`) and a list node.
    #[doc(alias = "hsd_80392528")]
    #[doc(alias = "fn_80392480")]
    pub fn perf_register(&mut self, item: Item) {
        if self.perf.events.iter().any(|e| e.item == item) {
            return;
        }
        let block = heap::mem_alloc(8);
        let node = heap::obj_alloc(Kind::SList);
        self.perf.events.push(Entry { item, block, node });
    }

    /// A scene's setup (`gm_801A4BD4`): the list emptied (`hsd_80392474`; its blocks and
    /// nodes stay where they were) and the monitor's GObj forgotten (`gm_804D6728`).
    #[doc(alias = "hsd_80392474")]
    pub fn perf_scene_setup(&mut self) {
        self.perf.events.clear();
        self.perf.gobj = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The peaks hold 60 draws, then take the time drawn.
    #[test]
    fn peaks_hold_sixty_draws() {
        let mut s = Stats::default();
        s.update([0.5, 0.5, 0.5]);
        assert_eq!(s.peak, [0.5; 3]);
        for _ in 0..61 {
            s.update([0.25, 0.25, 0.25]);
        }
        assert_eq!(s.peak, [0.5; 3]);
        s.update([0.25, 0.25, 0.25]);
        assert_eq!(s.peak, [0.25; 3]);
        assert_eq!(s.best, [0.5; 3]);
    }

    /// The line's characters leave out its colour codes; decimal digits in a code read as 0.
    #[test]
    fn text_and_codes() {
        assert_eq!(text_chars(b"\\c00ff00 0.000 1"), 8);
        let mut c = [0; 4];
        let mut out = Vec::new();
        text_draws(b"\\c80e0ff1", &mut c, 0.0, 0.0, &mut out);
        assert_eq!(c, [0x00, 0xE0, 0xFF, 0xFF]);
        assert_eq!(out.len(), 1);
    }
}
