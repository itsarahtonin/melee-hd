//! The game camera (`melee/cm/camera.c`) against ssbm-rs's port, run from real game states:
//! each of ssbm-rs's lockstep captures of a camera function is imported into melee-sim, run on
//! both sides from the same state, in both float modes, and then the whole state and whatever
//! the call returned or wrote outside it (a stack local, a result register) are compared.
//!
//! The captures are game memory, so they live outside the repository, in `CAMERA_CALLS`
//! (default `C:/Projects/ssbm-rs-publication/local/calls/camera`), one directory per replay;
//! the tests skip without them. ssbm-rs's runner (branch `feat/state-trace`) makes them, with
//! `LOCKSTEP_DONE` listing every function outside the camera's unit so only it is checked:
//!
//! ```text
//! CAPTURE=$CAMERA_CALLS/bf CAPTURE_NESTED=1 CAPTURE_FUNCS=@camera-funcs.txt \
//!   LOCKSTEP_DONE=done-noncamera.txt ssbm-run DISC --replay REPLAY.slp --fields 6000 \
//!   --port all --lockstep --known KNOWN.txt
//! ```
//!
//! over a VS replay per legal stage (`tools/run/src/calls.rs` in ssbm-rs has the options;
//! `camera-funcs.txt` lists the unit's functions by address, one per line).

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::OnceLock;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, handle, hsd_reference};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps, box_index, sym};
use melee_capture::{Capture, DataMap, DiscIndex, MEM1, disc_index, find_archives_in};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_sim::game::camera::{self, Bounds, Which};
use melee_sim::game::handles::CameraBoxId;
use melee_sim::game::state::{Game, Running};
use melee_sim::hsd::cobj::CObjId;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

fn calls_dir() -> PathBuf {
    std::env::var("CAMERA_CALLS")
        .unwrap_or_else(|_| "C:/Projects/ssbm-rs-publication/local/calls/camera".into())
        .into()
}

/// The saved real calls of `func` in the captures' directories.
fn calls(func: &str) -> Vec<PathBuf> {
    let dir = calls_dir();
    let prefix = format!("{func}-call");
    let mut dirs = vec![dir.clone()];
    dirs.extend(
        std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir()),
    );
    let mut found: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".call")
        })
        .collect();
    found.sort();
    if found.is_empty() {
        eprintln!(
            "no captures of {func} in {}: make them with ssbm-rs's runner (see this file's docs)",
            dir.display()
        );
    }
    found
}

/// The disc's archives, read once for every test.
fn disc_archives() -> &'static DiscIndex {
    static ARCHIVES: OnceLock<DiscIndex> = OnceLock::new();
    ARCHIVES.get_or_init(|| disc_index(&Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap()))
}

/// The call's arguments, mapped to melee-sim's handles.
struct Args<'a> {
    cap: &'a Capture,
    maps: &'a Maps,
}

impl Args<'_> {
    fn r(&self, i: usize) -> u32 {
        self.cap.call.regs.gpr[i]
    }

    fn f(&self, i: usize) -> f64 {
        self.cap.call.regs.fpr[i].ps0
    }

    fn word(&self, a: u32) -> u32 {
        self.cap.u32(a)
    }

    fn vec(&self, a: u32) -> Vec3 {
        let f = |o| f32::from_bits(self.word(a + o));
        Vec3::new(f(0), f(4), f(8))
    }

    fn gobj(&self, a: u32) -> GObjId {
        self.maps.gobjs[&a]
    }

    fn cobj(&self, a: u32) -> CObjId {
        self.maps.cobjs[&a]
    }

    fn camera_box(&self, a: u32) -> CameraBoxId {
        CameraBoxId(box_index(self.word(sym("cm_804D645C")), a))
    }

    /// Which transform a `CameraTransformState*` points at.
    fn which(&self, a: u32) -> Which {
        match a - sym("game_camera") {
            0x14 => Which::Main,
            0x4C => Which::Copy,
            off => panic!("a transform at game_camera+{off:#x}"),
        }
    }

    fn bounds(&self, a: u32) -> Bounds {
        let f = |o| f32::from_bits(self.word(a + o));
        Bounds {
            x_min: f(0),
            y_min: f(4),
            x_max: f(8),
            y_max: f(0xC),
            total_subjects: self.word(a + 0x10) as i32,
            z_pos: f(0x14),
        }
    }
}

/// What a call returned or wrote outside the state, compared with ssbm-rs's.
enum Out {
    R3(u32),
    F1(f32),
    GObj(Option<GObjId>),
    /// Words at an address: a stack local or an out parameter.
    Words(u32, Vec<u32>),
    /// A camera box returned in r3.
    Box(Option<CameraBoxId>),
    /// The address of a camera box's bone position, returned in r3.
    Bone(CameraBoxId),
    /// Compares only the camera afterwards: a call that reallocates the boxes leaves the
    /// pointers fighters hold to the old ones on both sides.
    OnlyCamera,
}

fn bounds_words(b: &Bounds) -> Vec<u32> {
    vec![
        b.x_min.to_bits(),
        b.y_min.to_bits(),
        b.x_max.to_bits(),
        b.y_max.to_bits(),
        b.total_subjects as u32,
        b.z_pos.to_bits(),
    ]
}

/// Runs the capture in the reference and returns memory after it, r3 and f1.
fn theirs(
    ctx: &Ctx,
    call: &ssbm_rt::capture::Call,
    run: Option<&Theirs<'_>>,
) -> (Vec<u8>, u32, f64) {
    call.load(ctx);
    match run {
        Some(run) => {
            let (r3, f1) = run(ctx, call);
            ctx.regs.set_r(3, r3);
            ctx.regs.set_f(1, f1);
        }
        None => ctx.invoke(call.addr),
    }
    let mut out = vec![0; call.mem1.len()];
    let _ = ctx.mem.read_bytes(MEM1, &mut out);
    (out, ctx.regs.r(3), ctx.regs.f(1))
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// The `file.c:LINE` of an assertion in a panic's message.
fn assert_line(text: &str) -> Option<String> {
    let at = text.find(".c:")?;
    let start = text[..at]
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map_or(0, |i| i + 1);
    let digits: String = text[at + 3..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let stem = &text[start..at];
    let line = if digits.is_empty() {
        // ssbm-rs passes the line as a number after the file.
        text[at + 3..]
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .chars()
            .take_while(char::is_ascii_digit)
            .collect()
    } else {
        digits
    };
    Some(format!("{stem}.c:{line}"))
}

/// How many of a function's captures [`check_fuzzed`] also perturbs.
const FUZZED_CAPTURES: usize = 6;

fn copy_capture(c: &Capture) -> Capture {
    let k = &c.call;
    Capture {
        call: ssbm_rt::capture::Call {
            addr: k.addr,
            mutated: k.mutated,
            stub: k.stub,
            regs: k.regs.clone(),
            mem1: k.mem1.clone(),
            locked: k.locked.clone(),
        },
    }
}

/// A capture's memory and registers being perturbed.
struct Fuzz<'a> {
    mem: &'a mut Vec<u8>,
    regs: &'a mut ssbm_rt::RegsSnapshot,
    rand: &'a mut Rand,
}

impl Fuzz<'_> {
    fn u32(&self, a: u32) -> u32 {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
    }

    fn put(&mut self, a: u32, v: u32) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn put_f32(&mut self, a: u32, v: f32) {
        self.put(a, v.to_bits());
    }

    /// A float near `around`, within `range`; now and then a special value or random bits.
    fn float(&mut self, around: f32, range: f32) -> f32 {
        match self.rand.below(40) {
            0 => [0.0, -0.0, 1.0, -1.0, f32::MIN_POSITIVE, f32::NAN][self.rand.below(6) as usize],
            1 => f32::from_bits(self.rand.bits() as u32),
            _ => around + self.rand.range(-range, range),
        }
    }

    /// Moves the float at `a` by up to `range`.
    fn nudge(&mut self, a: u32, range: f32) {
        let v = f32::from_bits(self.u32(a));
        let v = if v.is_finite() { v } else { 0.0 };
        let n = self.float(v, range);
        self.put_f32(a, n);
    }

    /// Sets the float at `a` to a value in [lo, hi].
    fn uniform(&mut self, a: u32, lo: f32, hi: f32) {
        let v = self.rand.range(lo, hi);
        self.put_f32(a, v);
    }

    fn put16(&mut self, a: u32, v: u16) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 2].copy_from_slice(&v.to_be_bytes());
    }

    /// A camera transform's points moved and its fields of view anywhere a camera puts them.
    fn transform(&mut self, t: u32) {
        for o in (0..0x30).step_by(4) {
            self.nudge(t + o, 80.0);
        }
        self.uniform(t + 0x30, 15.0, 60.0);
        self.uniform(t + 0x34, 15.0, 60.0);
    }

    fn camera(&mut self) {
        let c = sym("game_camera");
        self.transform(c + 0x14);
        self.transform(c + 0x4C);
    }

    /// A box of any shape, sometimes inside out, with zero to four boxes framed.
    fn bounds(&mut self, b: u32) {
        let x = self.rand.range(-250.0, 150.0);
        let y = self.rand.range(-150.0, 150.0);
        let w = self.rand.range(-20.0, 400.0);
        let h = self.rand.range(-20.0, 300.0);
        self.put_f32(b, x);
        self.put_f32(b + 4, y);
        self.put_f32(b + 8, x + w);
        self.put_f32(b + 0xC, y + h);
        let n = self.rand.below(5) as u32;
        self.put(b + 0x10, n);
        self.uniform(b + 0x14, 20.0, 800.0);
    }

    /// Every camera box in use moved, resized, and now and then switched to another state.
    fn boxes(&mut self) {
        let mut b = self.u32(sym("cm_804D6468"));
        while b != 0 {
            if self.rand.below(3) == 0 {
                let state = self.rand.below(3) as u32;
                self.put(b + 8, state);
                let timer = self.rand.below(3) as u16;
                self.put16(b + 0xE, timer);
                let flags = self.rand.bits() as u8 & 0xE0;
                let i = (b + 0xC - MEM1) as usize;
                self.mem[i] = flags;
            }
            for o in [0x10, 0x14, 0x1C, 0x20] {
                self.nudge(b + o, 120.0);
            }
            for o in (0x2C..0x54).step_by(4) {
                self.nudge(b + o, 12.0);
            }
            b = self.u32(b + 4);
        }
    }

    fn set_f(&mut self, i: usize, v: f32) {
        self.regs.fpr[i].ps0 = leaf_check::arg(v);
        self.regs.fpr[i].ps1 = leaf_check::arg(v);
    }
}

/// Runs every capture of `func` on both sides, `ours` on the imported state, and requires the
/// same state and outputs after.
///
/// What already differs before the call (the importer's limits, such as a fighter's pointer to
/// a freed GObj) isn't the call's doing and is set aside, but nothing of the camera may be
/// among it. A capture the importer can't take at all (game data it doesn't find loaded, as in
/// Pokémon Stadium's transformations) is skipped and counted; most must import.
fn check(func: &str, ours: impl Fn(&mut Running, &Args) -> Vec<Out>) {
    check_fuzzed(func, 0, |_| {}, ours);
}

/// [`check`], and for the first captures also `variants` copies of each with `perturb`
/// changing the state (both sides start from the changed memory), to reach values and branches
/// the captured states don't.
fn check_fuzzed(
    func: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
) {
    check_runs(func, variants, false, &perturb, &ours, None);
}

/// How ssbm-rs's side runs a call the captures didn't save: its port called on the captured
/// state, returning r3 and f1.
type Theirs<'a> = dyn Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, f64) + 'a;

/// Calls a function the captures didn't save from the states of another's captures (`base`):
/// `theirs` calls ssbm-rs's port on the loaded state, `ours` ours on the imported one.
fn check_calls(
    base: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
    theirs: impl Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, f64),
) {
    check_runs(base, variants, true, &perturb, &ours, Some(&theirs));
}

/// [`check_fuzzed`] with every run perturbed, the captured one too: for a state the captures
/// never reach (one the stage's unported code would change).
fn check_perturbed(
    func: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
) {
    check_runs(func, variants, true, &perturb, &ours, None);
}

fn check_runs(
    func: &str,
    variants: usize,
    perturb_all: bool,
    perturb: &dyn Fn(&mut Fuzz),
    ours: &dyn Fn(&mut Running, &Args) -> Vec<Out>,
    run: Option<&Theirs<'_>>,
) {
    let paths = calls(func);
    if paths.is_empty() {
        return;
    }
    let Some(ctx) = hsd_reference() else { return };
    let disc = disc_archives();
    let mut skipped = Vec::new();
    let mut asserted = 0;
    let mut rand = Rand(0x5EED_CA3E_u64 ^ func.len() as u64);
    for (n, path) in paths.iter().enumerate() {
        let original = Capture::read(path);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(disc, &original.call.mem1, &mut archives),
        };
        let copies = if n < FUZZED_CAPTURES { variants } else { 0 };
        for variant in 0..=copies {
            let cap = if variant == 0 && !perturb_all {
                copy_capture(&original)
            } else {
                let mut c = copy_capture(&original);
                perturb(&mut Fuzz {
                    mem: &mut c.call.mem1,
                    regs: &mut c.call.regs,
                    rand: &mut rand,
                });
                c
            };
            for mode in MODES {
                set_fp_mode(mode);
                let imported = catch_unwind(AssertUnwindSafe(|| {
                    let mut imp = Importer::new(&cap, &data);
                    imp.world();
                    imp.stage();
                    imp.ids();
                    let Importer { game, maps, .. } = imp;
                    (game, maps)
                }));
                let Ok((mut game, maps)) = imported else {
                    if variant == 0 {
                        skipped.push(path.clone());
                    }
                    break;
                };
                let mut round_trip = Comparer::new(&cap.call.mem1, &data, &game, maps.clone());
                round_trip.world();
                round_trip.stage();
                round_trip.ids();
                let before: HashSet<String> = round_trip.diffs.into_iter().collect();
                assert!(
                    !before.iter().any(|d| d.contains("camera")),
                    "{}: the camera doesn't round-trip: {before:?}",
                    path.display()
                );
                // An assertion of the original's (a perturbed state can reach one) must fail
                // on both sides, at the same line.
                let ours = catch_unwind(AssertUnwindSafe(|| {
                    ours(
                        &mut Running {
                            game: &mut game,
                            data: &archives,
                        },
                        &Args {
                            cap: &cap,
                            maps: &maps,
                        },
                    )
                }));
                let theirs = catch_unwind(AssertUnwindSafe(|| theirs(&ctx, &cap.call, run)));
                let (outs, (after, r3, f1)) = match (ours, theirs) {
                    (Ok(o), Ok(t)) => (o, t),
                    (Err(o), Err(t)) => {
                        let (o, t) = (panic_text(&o), panic_text(&t));
                        assert_eq!(
                            assert_line(&o),
                            assert_line(&t),
                            "{} (variant {variant}): ours {o:?}, theirs {t:?}",
                            path.display()
                        );
                        assert!(assert_line(&o).is_some(), "{}: {o}", path.display());
                        asserted += 1;
                        continue;
                    }
                    (Err(o), Ok(_)) => {
                        panic!(
                            "{} (variant {variant}): ours panicked: {}",
                            path.display(),
                            panic_text(&o)
                        )
                    }
                    (Ok(_), Err(t)) => {
                        panic!(
                            "{} (variant {variant}): theirs panicked: {}",
                            path.display(),
                            panic_text(&t)
                        )
                    }
                };
                let word = |a: u32| {
                    let i = (a - MEM1) as usize;
                    u32::from_be_bytes(after[i..i + 4].try_into().unwrap())
                };
                let mut diffs = Vec::new();
                for o in &outs {
                    match o {
                        Out::R3(v) if *v != r3 => {
                            diffs.push(format!("r3: ours {v:#x}, theirs {r3:#x}"))
                        }
                        Out::F1(v) if leaf_check::arg(*v).to_bits() != f1.to_bits() => {
                            diffs.push(format!("f1: ours {v:?}, theirs {f1:?}"))
                        }
                        Out::GObj(g) if *g != (r3 != 0).then(|| maps.gobjs[&r3]) => {
                            diffs.push(format!("r3: ours {g:?}, theirs {r3:#x}"))
                        }
                        Out::Box(b) => {
                            let base = word(sym("cm_804D645C"));
                            let t = (r3 != 0).then(|| CameraBoxId(box_index(base, r3)));
                            if *b != t {
                                diffs.push(format!("r3: ours {b:?}, theirs {r3:#x}"));
                            }
                        }
                        Out::Bone(b) => {
                            let at = word(sym("cm_804D645C")) + b.0 * 0x6C + 0x1C;
                            if at != r3 {
                                diffs.push(format!("r3: ours {at:#x}, theirs {r3:#x}"));
                            }
                        }
                        Out::Words(a, ws) => {
                            let t: Vec<u32> =
                                (0..ws.len() as u32).map(|i| word(a + 4 * i)).collect();
                            if &t != ws {
                                diffs.push(format!("{a:#x}: ours {ws:x?}, theirs {t:x?}"));
                            }
                        }
                        _ => {}
                    }
                }
                let mut cmp = Comparer::new(&after, &data, &game, maps);
                if outs.iter().any(|o| matches!(o, Out::OnlyCamera)) {
                    cmp.camera();
                } else {
                    cmp.world();
                    cmp.stage();
                    cmp.ids();
                }
                diffs.extend(cmp.diffs.into_iter().filter(|d| !before.contains(d)));
                for d in diffs.iter().take(20) {
                    eprintln!("  {d}");
                }
                assert!(
                    diffs.is_empty(),
                    "{} (variant {variant}) in {mode:?}: {} differences",
                    path.display(),
                    diffs.len()
                );
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
    for s in &skipped {
        eprintln!("  skipped (the importer can't take it): {}", s.display());
    }
    let matched = paths.len() - skipped.len();
    assert!(
        matched * 4 >= paths.len() * 3,
        "{func}: only {matched} of {} captures import",
        paths.len()
    );
    eprintln!(
        "{func}: {matched} captures match, {} skipped, {asserted} runs assert on both sides",
        skipped.len()
    );
}

fn transform(game: &mut Game, which: Which) -> &mut melee_sim::game::structs::CameraTransformState {
    game.camera.transform(which)
}

#[test]
fn process() {
    check_fuzzed(
        "fn_8002F360",
        8,
        |f| {
            f.camera();
            f.boxes();
        },
        |r, a| {
            camera::proc(r, a.gobj(a.r(3)));
            vec![]
        },
    );
}

#[test]
fn standard() {
    check_fuzzed(
        "Camera_8002B3D4",
        8,
        |f| {
            f.camera();
            f.boxes();
        },
        |r, _| {
            camera::update_standard(r);
            vec![]
        },
    );
}

#[test]
fn cobj_update() {
    check_fuzzed(
        "Camera_8002A4AC",
        8,
        |f| {
            f.camera();
            let c = sym("game_camera");
            f.nudge(c + 0x84, 5.0);
            f.nudge(c + 0x88, 5.0);
        },
        |r, a| {
            camera::update_cobj(r.game, a.gobj(a.r(3)));
            vec![]
        },
    );
}

#[test]
fn aim() {
    check_fuzzed(
        "Camera_8002AF68",
        8,
        |f| {
            f.camera();
            let c = sym("game_camera");
            f.nudge(c + 0x84, 5.0);
            f.nudge(c + 0x88, 5.0);
        },
        |r, a| {
            camera::aim(r.game, Some(a.cobj(a.r(3))), a.which(a.r(4)));
            vec![]
        },
    );
}

#[test]
fn keep_in_bounds() {
    check_fuzzed(
        "Camera_8002A768",
        8,
        |f| f.camera(),
        |r, a| {
            let g = &mut *r.game;
            let t = match a.which(a.r(3)) {
                Which::Main => &mut g.camera.cam.transform,
                Which::Copy => &mut g.camera.cam.transform_copy,
            };
            camera::keep_in_bounds(&g.stage_info, t, a.r(4) != 0);
            vec![]
        },
    );
}

#[test]
fn track_bounds() {
    check_fuzzed(
        "Camera_8002958C",
        8,
        |f| {
            f.camera();
            f.boxes();
        },
        |r, a| {
            let g = &mut *r.game;
            let t = match a.which(a.r(4)) {
                Which::Main => g.camera.cam.transform.clone(),
                Which::Copy => g.camera.cam.transform_copy.clone(),
            };
            let b = camera::track_bounds(&mut g.camera.boxes, &g.stage_info, &t);
            vec![Out::Words(a.r(3), bounds_words(&b))]
        },
    );
}

#[test]
fn outside_bounds() {
    check_fuzzed(
        "Camera_80029124",
        8,
        |f| {
            let v = f.regs.gpr[3];
            for o in [0, 4, 8] {
                f.nudge(v + o, 250.0);
            }
            f.regs.gpr[4] = f.rand.below(60) as u32;
        },
        |r, a| {
            let flags = camera::outside_bounds(&r.game.stage_info, a.vec(a.r(3)), a.r(4) as i32);
            vec![Out::R3(flags)]
        },
    );
}

#[test]
fn frame() {
    check_fuzzed(
        "Camera_80029CF8",
        8,
        |f| {
            f.camera();
            let b = f.regs.gpr[3];
            f.bounds(b);
        },
        |r, a| {
            let g = &mut *r.game;
            let t = match a.which(a.r(4)) {
                Which::Main => &mut g.camera.cam.transform,
                Which::Copy => &mut g.camera.cam.transform_copy,
            };
            camera::frame(&a.bounds(a.r(3)), &g.stage_info, t);
            vec![]
        },
    );
}

#[test]
fn update_quakes() {
    check("Camera_UpdateQuakes", |r, _| {
        camera::update_quakes(r);
        vec![]
    });
}

#[test]
fn ease_interest() {
    check_fuzzed(
        "Camera_80029AAC",
        8,
        |f| {
            f.camera();
            let b = f.regs.gpr[3];
            f.bounds(b);
            let s = f.rand.range(0.0, 1.5);
            f.set_f(1, s);
            let z = sym("game_camera") + 0x2BC;
            f.uniform(z, 0.0, 2.0);
        },
        |r, a| {
            let zoom = r.game.camera.cam.x2BC;
            let t = transform(r.game, a.which(a.r(4)));
            camera::ease_interest(&a.bounds(a.r(3)), t, a.f(1), zoom);
            vec![]
        },
    );
}

#[test]
fn ease_eye() {
    check_fuzzed(
        "Camera_80029C88",
        8,
        |f| {
            f.camera();
            let s = f.float(0.5, 1.0);
            f.set_f(1, s);
        },
        |r, a| {
            camera::ease_eye(transform(r.game, a.which(a.r(4))), a.f(1));
            vec![]
        },
    );
}

#[test]
fn fit_depth() {
    check_fuzzed(
        "Camera_80029BC4",
        8,
        |f| {
            f.camera();
            let b = f.regs.gpr[3];
            f.bounds(b);
        },
        |r, a| {
            let mut b = a.bounds(a.r(3));
            let g = &mut *r.game;
            let t = match a.which(a.r(4)) {
                Which::Main => &g.camera.cam.transform,
                Which::Copy => &g.camera.cam.transform_copy,
            };
            camera::fit_depth(&mut b, &g.stage_info, t);
            vec![Out::Words(a.r(3), bounds_words(&b))]
        },
    );
}

#[test]
fn ease_extents() {
    check_fuzzed(
        "Camera_800293E0",
        8,
        |f| f.boxes(),
        |r, _| {
            let g = &mut *r.game;
            camera::ease_extents(&mut g.camera.boxes, &g.stage_info);
            vec![]
        },
    );
}

#[test]
fn is_framed() {
    check_fuzzed(
        "Camera_8002928C",
        8,
        |f| f.boxes(),
        |r, a| {
            let g = &mut *r.game;
            let b = a.camera_box(a.r(3));
            let framed = camera::is_framed(&g.stage_info, &mut g.camera.boxes[b]);
            vec![Out::R3(u32::from(framed))]
        },
    );
}

#[test]
fn apply_quake() {
    check_fuzzed(
        "Camera_ApplyQuake",
        8,
        |f| {
            f.camera();
            let b = f.regs.gpr[3];
            f.bounds(b);
            let c = sym("game_camera");
            f.nudge(c + 0xA4, 3.0);
            f.nudge(c + 0xA8, 3.0);
            f.uniform(c + 0xAC, 0.0, 2.0);
        },
        |r, a| {
            camera::apply_quake(r.game, &a.bounds(a.r(3)), a.which(a.r(4)));
            vec![]
        },
    );
}

#[test]
fn zoom_on_player() {
    check("Camera_8002B1F8", |r, a| {
        camera::zoom_on_player(r.game, a.which(a.r(3)));
        vec![]
    });
}

#[test]
fn update_zoom() {
    check("Camera_8002B0E0", |r, _| {
        camera::update_zoom(r.game);
        vec![]
    });
}

#[test]
fn quake_offset_and_translation() {
    check("Camera_SetQuakeOffset", |r, a| {
        camera::set_quake_offset(&mut r.game.camera.cam, a.f(1), a.f(2));
        vec![]
    });
    check("Camera_80030DE4", |r, a| {
        camera::set_translation(&mut r.game.camera.cam, a.f(1), a.f(2));
        vec![]
    });
    check("Camera_80030DF8", |r, _| {
        camera::clear_translation(&mut r.game.camera.cam);
        vec![]
    });
}

#[test]
fn flags() {
    check("Camera_80031144", |r, _| {
        vec![Out::F1(camera::zoom(&r.game.camera.cam))]
    });
    check("Camera_8003108C", |r, _| {
        vec![Out::R3(camera::layer(&r.game.camera.cam).into())]
    });
    check("Camera_800310A0", |r, a| {
        camera::set_layer(&mut r.game.camera.cam, a.r(3) as u8);
        vec![]
    });
    check("Camera_80031060", |r, _| {
        vec![Out::R3(camera::pass(&r.game.camera.cam).into())]
    });
    check("Camera_80031074", |r, a| {
        camera::set_pass(&mut r.game.camera.cam, a.r(3) as u8);
        vec![]
    });
    check("Camera_80030AF8", |r, _| {
        vec![Out::R3(camera::hold_off_plane(&r.game.camera.cam).into())]
    });
    check("Camera_80030130", |r, _| {
        vec![Out::R3(camera::is_boss_intro(&r.game.camera.cam).into())]
    });
    check("Camera_8003010C", |r, _| {
        vec![Out::R3(camera::is_free(&r.game.camera.cam).into())]
    });
    check("Camera_80030AC4", |r, _| {
        vec![Out::R3(camera::stage_visible(&r.game.camera.cam).into())]
    });
    check("Camera_80030A78", |r, _| {
        vec![Out::R3(camera::draw_collisions(&r.game.camera.cam).into())]
    });
    check("Camera_80030B24", |r, _| {
        vec![Out::R3(camera::shadows(&r.game.camera.cam).into())]
    });
    check("Camera_80030BA8", |r, _| {
        vec![Out::R3(
            camera::draw_special_points(&r.game.camera.cam).into(),
        )]
    });
    check("Camera_80030A50", |r, _| {
        vec![Out::GObj(camera::gobj(&r.game.camera.cam))]
    });
}

/// `Camera_80030BBC`'s outputs: its result, and the pixel when it wrote one, into the
/// fighter when the pointer is a fighter's `x2188` (as `ftLib` passes it).
fn screen_outs(game: &mut Game, a: &Args, res: Option<([i32; 2], bool)>, out: u32) -> Vec<Out> {
    let mut v = vec![Out::R3(u32::from(res.is_some_and(|(_, inside)| inside)))];
    if let (Some(([x, y], _)), true) = (res, out != 0) {
        let fighter = a
            .maps
            .fighters
            .iter()
            .find(|(addr, _)| *addr + 0x2188 == out);
        match fighter {
            Some((_, f)) => {
                let p = &mut game.fighters[f.0].x2188;
                (p.x, p.y) = (x, y);
            }
            None => v.push(Out::Words(out, vec![x as u32, y as u32])),
        }
    }
    v
}

#[test]
fn to_screen() {
    check_fuzzed(
        "Camera_80030BBC",
        8,
        |f| {
            f.camera();
            let v = f.regs.gpr[3];
            f.nudge(v, 200.0);
            f.nudge(v + 4, 200.0);
            // Behind the eye too, which pulls the point in front of it.
            f.nudge(v + 8, 700.0);
        },
        |r, a| {
            let res = camera::to_screen(r.game, r.data, a.vec(a.r(3)));
            screen_outs(r.game, a, res, a.r(4))
        },
    );
    check("Camera_80030CD8", |r, a| {
        let res = camera::box_to_screen(r.game, r.data, a.camera_box(a.r(3)));
        screen_outs(r.game, a, res, a.r(4))
    });
}

#[test]
fn box_on_screen() {
    check_fuzzed(
        "Camera_80030CFC",
        8,
        |f| {
            f.boxes();
            let t = f.rand.range(0.0, 40.0);
            f.set_f(1, t);
        },
        |r, a| {
            let tolerance = leaf_check::bits(a.f(1));
            let on = camera::box_on_screen(
                r.game,
                r.data,
                a.camera_box(a.r(3)),
                f32::from_bits(tolerance),
            );
            vec![Out::R3(u32::from(on))]
        },
    );
}

#[test]
fn ground_span() {
    check("Camera_800307D0", |r, a| {
        let ([left, center, right], ok) = camera::ground_span(r.game, r.data);
        vec![
            Out::R3(u32::from(ok)),
            Out::Words(a.r(3), vec![left.to_bits()]),
            Out::Words(a.r(4), vec![center.to_bits()]),
            Out::Words(a.r(5), vec![right.to_bits()]),
        ]
    });
}

// Calls the captures don't save, made from the states of the camera process's captures.

const BASE: &str = "fn_8002F360";

/// Scratch below the captured stack for arguments passed by pointer.
fn scratch(call: &ssbm_rt::capture::Call) -> u32 {
    call.regs.gpr[1] - 0x2000
}

/// A random point around the stage, in the captured call's f1 to f3 (where the tests keep it).
fn random_point(f: &mut Fuzz) {
    for i in 1..=3 {
        let v = f.float(0.0, 300.0);
        f.set_f(i, v);
    }
}

fn point(regs: &ssbm_rt::RegsSnapshot) -> Vec3 {
    let f = |i: usize| f32::from_bits(leaf_check::bits(regs.fpr[i].ps0));
    Vec3::new(f(1), f(2), f(3))
}

fn f1(a: &Args) -> f32 {
    f32::from_bits(leaf_check::bits(a.f(1)))
}

#[test]
fn quakes() {
    // With no quake model on the stage: loading one is the stage's to port.
    check_perturbed(
        "Camera_RequestQuake",
        8,
        |f| {
            let set = sym("stage_info") + 0x6CC;
            f.put(set, 0);
            if f.rand.below(2) == 0 {
                f.regs.gpr[3] = f.rand.below(5) as u32;
            }
            if f.rand.below(4) == 0 {
                f.regs.gpr[4] = 0;
            }
        },
        |r, a| {
            let pos = (a.r(4) != 0).then(|| a.vec(a.r(4)));
            let (kind, stale) = (a.r(3) as i32, r.game.objs.stale_reads);
            camera::request_quake(r.game, kind, pos);
            if !(1..=4).contains(&kind) && pos.is_some() {
                // The original's length is then the epicenter's address, which our state
                // can't hold: the port counts a stale read, and the address is put back here.
                assert_eq!(r.game.objs.stale_reads, stale + 1);
                r.game.camera.cam.quake_frames_left[kind as usize] = a.r(4) as i32;
            }
            vec![]
        },
    );
    check_calls(
        BASE,
        4,
        |f| {
            f.regs.gpr[3] = f.rand.below(5) as u32;
            let s = f.float(1.0, 1.0);
            f.set_f(1, s);
        },
        |r, a| {
            camera::stop_quake(&mut r.game.camera.cam, a.r(3) as i32);
            camera::set_quake_scale(&mut r.game.camera.cam, f1(a));
            vec![]
        },
        |ctx, call| {
            fns::Camera_StopQuake(ctx, call.regs.gpr[3] as i32);
            fns::Camera_SetQuakeScale(ctx, call.regs.fpr[1].ps0);
            (0, 0.0)
        },
    );
}

#[test]
fn average_width() {
    check_calls(
        BASE,
        8,
        |f| {
            let c = sym("game_camera");
            let n = f.rand.below(4) as u16;
            f.put16(c + 0x2B8, n.wrapping_sub(1));
            f.nudge(c + 0x2B0, 100.0);
        },
        |r, _| vec![Out::F1(camera::average_width(&r.game.camera.cam))],
        |ctx, _| (0, fns::Camera_80030E10(ctx)),
    );
}

#[test]
fn bounds_tests() {
    check_calls(
        BASE,
        12,
        random_point,
        |r, a| {
            let p = point(&a.cap.call.regs);
            vec![Out::R3(camera::in_bounds(&r.game.stage_info, p).into())]
        },
        |ctx, call| {
            let at = scratch(call);
            leaf_check::put_vec(ctx, at, point(&call.regs));
            (fns::Camera_80031154(ctx, handle(ctx, at)) as u32, 0.0)
        },
    );
    check_calls(
        BASE,
        12,
        |f| {
            random_point(f);
            let m = f.float(10.0, 40.0);
            f.set_f(4, m);
        },
        |r, a| {
            let p = point(&a.cap.call.regs);
            let m = f32::from_bits(leaf_check::bits(a.f(4)));
            vec![Out::R3(
                camera::in_bounds_by(&r.game.stage_info, p, m).into(),
            )]
        },
        |ctx, call| {
            let at = scratch(call);
            leaf_check::put_vec(ctx, at, point(&call.regs));
            let r = fns::Camera_8003118C(ctx, handle(ctx, at), call.regs.fpr[4].ps0);
            (r as u32, 0.0)
        },
    );
    check_calls(
        BASE,
        0,
        |_| {},
        |r, a| {
            let v = camera::stage_bounds(&r.game.stage_info);
            vec![Out::Words(
                scratch(&a.cap.call),
                v.map(f32::to_bits).to_vec(),
            )]
        },
        |ctx, call| {
            let r = fns::Camera_SetBounds(ctx, handle(ctx, scratch(call)));
            (r as u32, 0.0)
        },
    );
}

#[test]
fn setters_and_getters() {
    check_calls(
        BASE,
        6,
        |f| {
            f.regs.gpr[3] = f.rand.below(3) as u32;
            let v = f.float(1.0, 100.0);
            f.set_f(1, v);
        },
        |r, a| {
            let (b, v) = (a.r(3) & 1 != 0, f1(a));
            let c = &mut r.game.camera.cam;
            camera::set_far(c, v);
            camera::set_near(c, v);
            camera::set_draw_collisions(c, b);
            camera::set_draw_zones(c, b);
            camera::set_hold_off_plane(c, b);
            camera::set_shadows(c, b);
            camera::set_x39a_b0(c, b);
            camera::set_x39a_b1(c, b);
            camera::set_draw_special_points(c, b);
            camera::set_stage_visible(c, a.r(3) as i32);
            camera::set_background_color(c, 1, a.r(3) as u8, 3);
            camera::set_default_fov(&mut r.game.camera, v);
            vec![]
        },
        |ctx, call| {
            let (b, v) = (call.regs.gpr[3] as i32, call.regs.fpr[1].ps0);
            fns::Camera_800311CC(ctx, v);
            fns::Camera_800311DC(ctx, v);
            fns::Camera_80030A60(ctx, b & 1);
            fns::Camera_80030A8C(ctx, b & 1);
            fns::Camera_80030AE0(ctx, b & 1);
            fns::Camera_80030B0C(ctx, b & 1);
            fns::Camera_80030B38(ctx, b & 1);
            fns::Camera_80030B64(ctx, b & 1);
            fns::Camera_80030B90(ctx, b & 1);
            fns::Camera_SetStageVisible(ctx, b);
            fns::Camera_SetBackgroundColor(ctx, 1, b as u8, 3);
            fns::Camera_80030730(ctx, v);
            (0, 0.0)
        },
    );
    check_calls(
        BASE,
        6,
        |f| {
            let flags = f.rand.bits() as u8;
            let i = (sym("game_camera") + 0x39A - MEM1) as usize;
            f.mem[i] = flags;
        },
        |r, _| {
            let c = &r.game.camera.cam;
            let bits = u32::from(camera::x39a_b0(c)) | u32::from(camera::x39a_b1(c)) << 1;
            vec![Out::R3(bits)]
        },
        |ctx, _| {
            let bits = fns::Camera_80030B50(ctx) as u32 | (fns::Camera_80030B7C(ctx) as u32) << 1;
            (bits, 0.0)
        },
    );
    check_calls(
        BASE,
        4,
        |f| {
            let mode = f.rand.below(9) as u32;
            f.put(sym("game_camera") + 4, mode);
            let last = f.rand.below(9) as u32;
            f.put(sym("cm_80453004"), last);
        },
        |r, a| {
            let c = &r.game.camera.cam;
            let modes =
                u32::from(camera::is_debug_follow(c)) | u32::from(camera::is_debug_free(c)) << 1;
            let color = camera::background_color(c);
            let at = scratch(&a.cap.call);
            let (p, i) = (camera::position(c), camera::interest(c));
            camera::show_all_layers(&mut r.game.camera.cam);
            camera::leave_debug(&mut r.game.camera);
            vec![
                Out::R3(modes),
                Out::Words(
                    at,
                    vec![u32::from_be_bytes([color[0], color[1], color[2], 0])],
                ),
                Out::Words(at + 0x10, leaf_check::vec_bits(p)),
                Out::Words(at + 0x20, leaf_check::vec_bits(i)),
            ]
        },
        |ctx, call| {
            let modes = fns::Camera_80030154(ctx) as u32 | (fns::Camera_80030178(ctx) as u32) << 1;
            let at = scratch(call);
            ctx.write_u32(at, 0);
            fns::Camera_GetBackgroundColor(ctx, handle(ctx, at));
            // The original leaves alpha unset: only red, green and blue are compared.
            ctx.write_u8(at + 3, 0);
            fns::Camera_GetTransformPosition(ctx, handle(ctx, at + 0x10));
            fns::Camera_GetTransformInterest(ctx, handle(ctx, at + 0x20));
            fns::Camera_800310E8(ctx);
            fns::Camera_800300F0(ctx);
            (modes, 0.0)
        },
    );
    check_calls(
        BASE,
        0,
        |_| {},
        |r, _| {
            camera::set_standard(&mut r.game.camera.cam);
            let s = r.game.camera.cam.mode;
            camera::set_fixed(&mut r.game.camera.cam);
            vec![Out::R3(s as u32)]
        },
        |ctx, _| {
            fns::Camera_SetModeToStandard(ctx);
            let s = ctx.read_u32(sym("game_camera") + 4);
            fns::Camera_SetModeToFixed(ctx);
            (s, 0.0)
        },
    );
}

#[test]
fn one_player_mode() {
    check_calls(
        BASE,
        16,
        |f| {
            let m = f.rand.below(0x2D) as u8;
            let i = (sym("state_machine") - MEM1) as usize;
            f.mem[i] = m;
        },
        |r, _| vec![Out::R3(melee_sim::game::gm::is_1p_mode(r.game).into())],
        |ctx, _| (fns::gm_IsCurrently1PMode_inline(ctx) as u32, 0.0),
    );
}

#[test]
fn cobj_copy_view() {
    check_calls(
        BASE,
        4,
        |f| f.camera(),
        |r, _| {
            let g = &mut *r.game;
            let copy = g.camera.cobj_copy;
            camera::aim(g, copy, Which::Copy);
            camera::cobj_copy_view(g, r.data);
            vec![]
        },
        |ctx, _| {
            let copy = ctx.read_u32(sym("cm_804D6464"));
            let t = handle(ctx, sym("game_camera") + 0x4C);
            fns::Camera_8002AF68(ctx, handle(ctx, copy), t);
            fns::Camera_800310B8(ctx);
            (0, 0.0)
        },
    );
}

#[test]
fn camera_boxes() {
    check_calls(
        BASE,
        6,
        |f| f.regs.gpr[3] = f.rand.below(3) as u32,
        |r, a| {
            let b = camera::alloc_box(&mut r.game.camera.boxes, a.r(3) as i32);
            vec![Out::Box(Some(b))]
        },
        |ctx, call| {
            let b = fns::Camera_80029044(ctx, call.regs.gpr[3] as i32);
            (b.addr(), 0.0)
        },
    );
    check_calls(
        BASE,
        0,
        |_| {},
        |r, _| {
            let b = camera::alloc_box(&mut r.game.camera.boxes, camera::ACTIVE);
            vec![Out::Box(Some(b))]
        },
        |ctx, _| (fns::Camera_80029020(ctx).addr(), 0.0),
    );
    // Frees one of the boxes in use, then resets another.
    check_calls(
        BASE,
        8,
        |f| {
            let mut boxes = Vec::new();
            let mut b = f.u32(sym("cm_804D6468"));
            while b != 0 {
                boxes.push(b);
                b = f.u32(b + 4);
            }
            let n = boxes.len() as u64;
            f.regs.gpr[3] = boxes[f.rand.below(n) as usize];
            f.regs.gpr[4] = boxes[f.rand.below(n) as usize];
            f.regs.gpr[5] = f.rand.below(3) as u32;
        },
        |r, a| {
            let boxes = &mut r.game.camera.boxes;
            camera::free_box(boxes, a.camera_box(a.r(3)));
            camera::reset_box(boxes, Some(a.camera_box(a.r(4))), a.r(5) as i32);
            vec![]
        },
        |ctx, call| {
            let g = &call.regs.gpr;
            fns::Camera_800290D4(ctx, handle(ctx, g[3]));
            fns::Camera_80028F5C(ctx, handle(ctx, g[4]), g[5] as i32);
            (0, 0.0)
        },
    );
}

#[test]
fn camera_init() {
    check_calls(
        BASE,
        4,
        |f| f.regs.gpr[3] = 1 + f.rand.below(4) as u32,
        |r, a| {
            camera::init(&mut r.game.camera, a.r(3) as i32);
            vec![Out::OnlyCamera]
        },
        |ctx, call| {
            fns::Camera_Init(ctx, call.regs.gpr[3] as i32);
            (0, 0.0)
        },
    );
}

#[test]
fn debug_cameras() {
    check_calls(
        BASE,
        8,
        |f| {
            f.regs.gpr[3] = f.rand.below(4) as u32;
            // Any mode but the follow camera, whose followed box the test reads back.
            let mode = [0, 1, 2, 3, 4, 5, 6, 8][f.rand.below(8) as usize];
            f.put(sym("game_camera") + 4, mode);
        },
        |r, a| {
            let slot = a.r(3) as i32;
            camera::debug_follow(r.game, r.data, slot);
            let state = &r.game.camera;
            let mut outs = vec![Out::R3(0)];
            if state.cam.mode == camera::DEBUG_FOLLOW && state.debug.ply_slot == slot {
                outs = vec![Out::Bone(camera::debug_followed(r.game))];
            }
            camera::debug_free(r.game, r.data);
            outs
        },
        |ctx, call| {
            let slot = call.regs.gpr[3] as i32;
            fns::Camera_8002FEEC(ctx, slot);
            let d = sym("cm_80453004");
            let follows =
                ctx.read_u32(sym("game_camera") + 4) == 7 && ctx.read_u32(d + 4) as i32 == slot;
            let bone = if follows {
                fns::Camera_8003019C(ctx).addr()
            } else {
                0
            };
            fns::Camera_8003006C(ctx);
            (bone, 0.0)
        },
    );
}
