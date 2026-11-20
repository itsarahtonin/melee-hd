//! The renderers the harness runs today: the exact renderer itself (melee-render's stream drawn
//! by melee-gpu, as the player draws it), the reference; and the exact renderer with a change
//! made on purpose ([`Mutation`]), standing in for a renderer under test until the fast one
//! exists, to show the harness fails a wrong frame and passes float noise.
//!
//! The stand-in runs melee-render's stream through melee-gx itself, as `melee_render::gpu::Gpu`
//! does, so it can change what reaches melee-gpu: a draw's state (its material color, its
//! position matrices, the viewport), whether a draw is drawn, and in what order. With no
//! change its frames and geometry are the reference's bit for bit (the harness's own check).

use std::cell::{Cell, RefCell};
use std::time::Instant;

use melee_data::archive::Archives;
use melee_gpu::Renderer;
use melee_gx::vertex::Attr;
use melee_gx::xform;
use melee_gx::{Draw, Memory, Produced, Sink, State};
use melee_render::frame::Frames;
use melee_render::gpu::{Gpu, Observer, StreamObserver};
use melee_render::mem::Space;
use melee_sim::hsd::display::Draw as DisplayDraw;

use super::geometry::{Geometry, Kind, Recorder, source_at};
use super::{FrameRenderer, Rendered};
use crate::native::Tick;

/// The exact renderer: melee-render's stream, drawn by melee-gpu on this thread or on its own
/// threads (as the player does), the reference every other renderer is held against.
pub struct Exact {
    frames: Frames,
    gpu: Gpu,
    device: wgpu::Device,
    loaded: bool,
    gpu_transforms: bool,
    material_records: bool,
    /// Each frame run as it's built (F5).
    streamed: bool,
}

impl Exact {
    pub fn new(renderer: Renderer, threaded: bool) -> Exact {
        let device = renderer.device().clone();
        let gpu_transforms = renderer.gpu_transforms;
        let material_records = renderer.material_records;
        let gpu = if threaded {
            Gpu::threaded(renderer)
        } else {
            Gpu::new(renderer)
        };
        Exact {
            frames: Frames::new(),
            gpu,
            device,
            loaded: false,
            gpu_transforms,
            material_records,
            streamed: false,
        }
    }

    /// The fast renderer as far as it goes (F5): F4, each frame run as it's built
    /// (`Gpu::run_streamed`), so the renderer's threads draw beside the game thread.
    pub fn fast(renderer: Renderer, threaded: bool) -> Exact {
        let mut r = Exact::f4(renderer, threaded);
        r.streamed = true;
        r
    }

    /// F4: the exact renderer with its shapes' display lists drawn from meshes decoded once
    /// (`Gpu::with_meshes`), their vertices transformed on the GPU, and each draw's state from
    /// its material's record.
    pub fn f4(mut renderer: Renderer, threaded: bool) -> Exact {
        renderer.gpu_transforms = true;
        renderer.material_records = true;
        Exact::with_meshes(renderer, threaded)
    }

    /// F3: F2's meshes with their vertices transformed on the GPU, each draw's state built
    /// from the registers.
    pub fn f3(mut renderer: Renderer, threaded: bool) -> Exact {
        renderer.gpu_transforms = true;
        renderer.material_records = false;
        Exact::with_meshes(renderer, threaded)
    }

    /// F2, the mesh cache alone: the meshes' vertices transformed on the CPU as the exact
    /// renderer's are.
    pub fn f2(mut renderer: Renderer, threaded: bool) -> Exact {
        renderer.gpu_transforms = false;
        Exact::with_meshes(renderer, threaded)
    }

    fn with_meshes(renderer: Renderer, threaded: bool) -> Exact {
        let mut r = Exact::new(renderer, threaded);
        r.gpu = r.gpu.with_meshes();
        r
    }

    /// The mesh cache's counts and size, for the fast renderer.
    pub fn meshes(&self) -> Option<(melee_render::meshes::Stats, (usize, usize))> {
        self.gpu.meshes().map(|m| (m.stats, m.size()))
    }

    /// The renderer, its threads stopped.
    pub fn into_renderer(self) -> Renderer {
        self.gpu.into_renderer()
    }
}

impl FrameRenderer for Exact {
    fn name(&self) -> String {
        if self.gpu.meshes().is_some() && self.material_records && self.streamed {
            "fast (F5: the frame run as it's built)".to_owned()
        } else if self.gpu.meshes().is_some() && self.gpu_transforms && self.material_records {
            "F4 (materials as records)".to_owned()
        } else if self.gpu.meshes().is_some() && self.gpu_transforms {
            "F3 (GPU transforms)".to_owned()
        } else if self.gpu.meshes().is_some() {
            "F2 (mesh cache)".to_owned()
        } else {
            "exact".to_owned()
        }
    }

    fn frame(&mut self, t: &Tick<'_>, draw: bool, geometry: bool) -> Rendered {
        if !self.loaded {
            self.frames.space.load_dol(t.dol);
            self.loaded = true;
        }
        let start = Instant::now();
        if self.streamed && draw {
            // Built and run at once: the build is within the submit.
            let rec = geometry.then(|| Recorder::new(&[]));
            let images = self.gpu.run_streamed(
                &mut self.frames,
                t.game,
                t.data,
                t.draws,
                None,
                rec.as_ref().map(|r| r as &dyn StreamObserver),
            );
            return Rendered {
                image: images.into_iter().last(),
                geometry: rec.map(Recorder::into_geometry),
                build: std::time::Duration::ZERO,
                submit: start.elapsed(),
            };
        }
        let fifo = self.frames.frame(t.game, t.data, t.draws);
        let build = start.elapsed();
        if !draw {
            self.gpu.follow(&self.frames.space, t.data, &fifo);
            return Rendered {
                build,
                ..Rendered::default()
            };
        }
        let rec = geometry.then(|| Recorder::new(&self.frames.marks));
        let images = self.gpu.run(
            &self.frames.space,
            t.data,
            &fifo,
            rec.as_ref().map(|r| r as &dyn Observer),
        );
        let submit = start.elapsed() - build;
        Rendered {
            image: images.into_iter().last(),
            geometry: rec.map(Recorder::into_geometry),
            build,
            submit,
        }
    }

    fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    fn warm(&mut self) {
        self.gpu.finish_specialized();
    }
}

/// A change made on purpose to the exact renderer's frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutation {
    /// None: the stand-in's frames are the reference's.
    None,
    /// Every draw drawn from a copy of its state, unchanged: the copy is faithful.
    Restate,
    /// A wrong frame: the largest draw whose color channel takes the material color has each
    /// of its material colors' red, green and blue 64 levels off.
    Color,
    /// A wrong frame: the largest draw missing.
    Drop,
    /// A wrong frame: the largest draw drawn after the one that follows it.
    Order,
    /// A wrong frame: every draw of the largest draw's joint with its position matrices moved
    /// sideways, by 1% of their depth (a few pixels).
    Joint,
    /// Float noise: every draw's position matrices one ulp off, alternately up and down.
    Ulp,
    /// A wrong frame from one ulp: the projection's depth offset one ulp further from zero,
    /// which puts the shadow cameras' erase quads, exactly on the near plane, outside the depth
    /// range, so the GPU clips them and the shadows' textures aren't erased.
    Edge,
    /// Float noise: every vertex 1/32 of a pixel right and down before the rasterizer's snap
    /// to sixteenths, so each moves 0 or 1/16 of a pixel (a renderer that snaps otherwise).
    Snap,
    /// Not a renderer under test: the reference's frame with every vertex 1/32 of a pixel
    /// right and up (`true`) or left and down before the snap, for how much each block of the
    /// frame changes under a sub-pixel shift (`image::blocks`). Across the other diagonal from
    /// [`Mutation::Snap`], so that one isn't measured against itself.
    Spread(bool),
}

impl Mutation {
    pub const ALL: [(&'static str, Mutation); 9] = [
        ("none", Mutation::None),
        ("restate", Mutation::Restate),
        ("color", Mutation::Color),
        ("drop", Mutation::Drop),
        ("order", Mutation::Order),
        ("joint", Mutation::Joint),
        ("ulp", Mutation::Ulp),
        ("edge", Mutation::Edge),
        ("snap", Mutation::Snap),
    ];

    pub fn parse(name: &str) -> Option<Mutation> {
        Self::ALL.iter().find(|(n, _)| *n == name).map(|(_, m)| *m)
    }

    pub fn name(self) -> &'static str {
        match self {
            Mutation::Spread(true) => "spread right and up",
            Mutation::Spread(false) => "spread left and down",
            _ => Self::ALL
                .iter()
                .find(|(_, m)| *m == self)
                .map_or("?", |(n, _)| n),
        }
    }
}

/// Which display draws a frame's mutation changes, picked from the reference's frame.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Sorted.
    pub targets: Vec<u32>,
    /// For [`Mutation::Order`]: the draw held back, and the one it's drawn after.
    pub swap: Option<(u32, u32)>,
}

/// Each display draw's area on screen in the reference's frame (its triangles', each clamped
/// to the viewport, in front of the eye), in the reference's order, with whether it takes the
/// material color.
pub fn areas(g: &Geometry) -> Vec<(u32, f64, bool)> {
    let mut out: Vec<(u32, f64, bool)> = Vec::new();
    for d in &g.draws {
        let mut area = 0f64;
        if d.kind == Kind::Triangles {
            let vp = d.viewport;
            let (x0, x1) = (vp.x_orig - vp.wd.abs(), vp.x_orig + vp.wd.abs());
            let (y0, y1) = (vp.y_orig - vp.ht.abs(), vp.y_orig + vp.ht.abs());
            for t in d.clip.as_chunks::<3>().0 {
                if t.iter().any(|c| c[3] <= 0.0) {
                    continue;
                }
                let p = t.map(|c| {
                    let s = vp.screen(c);
                    [f64::from(s[0].clamp(x0, x1)), f64::from(s[1].clamp(y0, y1))]
                });
                let cross = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                    - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
                if cross.is_finite() {
                    area += cross.abs() / 2.0;
                }
            }
        }
        match out.last_mut() {
            Some(last) if last.0 == d.source => {
                last.1 += area;
                last.2 |= d.material_color;
            }
            _ => out.push((d.source, area, d.material_color)),
        }
    }
    out
}

/// Picks what `mutation` changes in a frame whose reference geometry is `g` and display draws
/// `draws`.
pub fn plan(mutation: Mutation, g: &Geometry, draws: &[DisplayDraw]) -> Plan {
    let a = areas(g);
    let largest = |only_material: bool| {
        a.iter()
            .enumerate()
            .filter(|(_, x)| !only_material || x.2)
            .max_by(|x, y| x.1.1.total_cmp(&y.1.1))
            .map(|(i, x)| (i, x.0))
    };
    match mutation {
        Mutation::Color => Plan {
            targets: largest(true).map(|x| x.1).into_iter().collect(),
            swap: None,
        },
        Mutation::Drop => Plan {
            targets: largest(false).map(|x| x.1).into_iter().collect(),
            swap: None,
        },
        Mutation::Order => {
            let swap = largest(false)
                .and_then(|(i, s)| a[i + 1..].iter().find(|x| x.0 != s).map(|x| (s, x.0)));
            Plan {
                targets: Vec::new(),
                swap,
            }
        }
        Mutation::Joint => {
            let jobj_of = |s: u32| match draws.get(s as usize) {
                Some(DisplayDraw::Shape { jobj, .. }) => Some(*jobj),
                _ => None,
            };
            // A joint whose shapes are all in front of the eye, so that moving it moves what
            // the screen shows (not a background around the camera).
            let in_front = |s: u32| {
                g.draws
                    .iter()
                    .filter(|d| d.source == s)
                    .all(|d| d.clip.iter().all(|c| c[3] > 0.0))
            };
            let joint = a
                .iter()
                .filter(|x| jobj_of(x.0).is_some() && in_front(x.0))
                .max_by(|x, y| x.1.total_cmp(&y.1))
                .and_then(|x| jobj_of(x.0));
            let mut targets: Vec<u32> = a
                .iter()
                .map(|x| x.0)
                .filter(|&s| joint.is_some() && jobj_of(s) == joint)
                .collect();
            targets.sort_unstable();
            Plan {
                targets,
                swap: None,
            }
        }
        _ => Plan::default(),
    }
}

/// A short description of display draw `s`, for reports.
pub fn describe(draws: &[DisplayDraw], s: u32) -> String {
    match draws.get(s as usize) {
        Some(d) => {
            let text = format!("{d:?}");
            if text.len() > 120 {
                format!("{}...", &text[..text.floor_char_boundary(117)])
            } else {
                text
            }
        }
        None => "the frame's end".to_owned(),
    }
}

/// XF's count of position matrices (`GX_PNMTX0` to `GX_PNMTX9`), 12 words each.
const POS_MATRICES: usize = 10;

/// Copies what melee-gpu reads of a state.
fn copy_state(from: &State, to: &mut State) {
    to.bp = from.bp;
    to.color_ra = from.color_ra;
    to.color_bg = from.color_bg;
    to.konst_ra = from.konst_ra;
    to.konst_bg = from.konst_bg;
    to.cp = from.cp;
    to.xf.clone_from(&from.xf);
    to.tmem.clone_from(&from.tmem);
}

fn nudge(word: &mut u32, up: bool) {
    let v = f32::from_bits(*word);
    if v != 0.0 && v.is_finite() {
        *word = if up { *word + 1 } else { *word - 1 };
    }
}

/// Changes a draw's state as `m` does to the draws it changes.
fn change(m: Mutation, s: &mut State) {
    match m {
        Mutation::Color => {
            for r in [xform::MAT_COLOR, xform::MAT_COLOR + 1] {
                s.xf[r] ^= 0x4040_4000;
            }
        }
        Mutation::Joint => {
            for k in 0..POS_MATRICES {
                let tz = f32::from_bits(s.xf[12 * k + 11]).abs();
                let shift = if tz > 0.0 { 0.01 * tz } else { 1.0 };
                let x = &mut s.xf[12 * k + 3];
                *x = (f32::from_bits(*x) + shift).to_bits();
            }
        }
        Mutation::Ulp => {
            for (i, w) in s.xf[..12 * POS_MATRICES].iter_mut().enumerate() {
                nudge(w, i % 2 == 0);
            }
        }
        Mutation::Edge => nudge(&mut s.xf[xform::PROJECTION + 5], true),
        Mutation::Snap => {
            for r in [xform::VIEWPORT + 3, xform::VIEWPORT + 4] {
                s.xf[r] = (f32::from_bits(s.xf[r]) + 1.0 / 32.0).to_bits();
            }
        }
        Mutation::Spread(right) => {
            let d = if right { 1.0 / 32.0 } else { -1.0 / 32.0 };
            for (r, d) in [(xform::VIEWPORT + 3, d), (xform::VIEWPORT + 4, -d)] {
                s.xf[r] = (f32::from_bits(s.xf[r]) + d).to_bits();
            }
        }
        Mutation::None | Mutation::Restate | Mutation::Drop | Mutation::Order => {}
    }
}

/// A draw held back to be drawn later, with its state.
struct Held {
    source: u32,
    state: Box<State>,
    primitive: u8,
    vat: u8,
    count: u16,
    layout: Vec<Attr>,
    data: Vec<u8>,
}

/// The stand-in's host: the GPU's memory, and what each draw and copy goes through on the way
/// to melee-gpu.
struct Host<'a> {
    space: &'a Space,
    data: &'a Archives,
    renderer: &'a RefCell<Renderer>,
    rec: Option<&'a Recorder>,
    marks: &'a [(u32, u32)],
    offset: Cell<usize>,
    mutation: Mutation,
    plan: &'a Plan,
    scratch: RefCell<Box<State>>,
    held: RefCell<Vec<Held>>,
}

impl Host<'_> {
    fn pass(&self, source: u32, state: &State, draw: &Draw<'_>) {
        if let Some(r) = self.rec {
            r.record(source, state, draw);
        }
        self.renderer.borrow_mut().draw(state, draw, self);
    }

    /// Draws what was held back.
    fn release(&self) {
        let held = std::mem::take(&mut *self.held.borrow_mut());
        for h in held {
            let d = Draw {
                primitive: h.primitive,
                vat: h.vat,
                count: h.count,
                layout: &h.layout,
                data: &h.data,
                fetched: true,
            };
            self.pass(h.source, &h.state, &d);
        }
    }
}

impl Memory for Host<'_> {
    fn read(&self, phys: u32, out: &mut [u8]) {
        self.space.read(self.data, phys, out);
    }

    fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
        self.space.view(self.data, phys, len)
    }

    fn draws(&self) -> bool {
        true
    }

    fn command(&self, bytes: &[u8]) {
        self.offset.set(self.offset.get() + bytes.len());
    }

    fn draw(&self, state: &State, draw: &Draw<'_>) {
        let source = source_at(self.marks, self.offset.get());
        if let Some((a, b)) = self.plan.swap
            && source != a
            && source != b
        {
            self.release();
        }
        let targeted = self.plan.targets.binary_search(&source).is_ok();
        match self.mutation {
            Mutation::None => self.pass(source, state, draw),
            Mutation::Drop if targeted => {}
            Mutation::Order if self.plan.swap.is_some_and(|(a, _)| a == source) => {
                let mut copy = Box::new(State::new());
                copy_state(state, &mut copy);
                self.held.borrow_mut().push(Held {
                    source,
                    state: copy,
                    primitive: draw.primitive,
                    vat: draw.vat,
                    count: draw.count,
                    layout: draw.layout.to_vec(),
                    data: draw.data.to_vec(),
                });
            }
            Mutation::Restate
            | Mutation::Ulp
            | Mutation::Edge
            | Mutation::Snap
            | Mutation::Spread(_) => {
                let mut s = self.scratch.borrow_mut();
                copy_state(state, &mut s);
                change(self.mutation, &mut s);
                self.pass(source, &s, draw);
            }
            Mutation::Color | Mutation::Joint if targeted => {
                let mut s = self.scratch.borrow_mut();
                copy_state(state, &mut s);
                change(self.mutation, &mut s);
                self.pass(source, &s, draw);
            }
            _ => self.pass(source, state, draw),
        }
    }

    fn copy(&self, state: &State, value: u32) {
        self.release();
        let mut r = self.renderer.borrow_mut();
        r.copy(state, value, self);
        // As melee-render's GPU does while something observes the draws.
        if self.rec.is_some() {
            for w in r.take_written() {
                self.space.write(w.address, &w.bytes);
            }
        }
    }
}

/// Memory for a stream run without drawing.
struct Quiet<'a> {
    space: &'a Space,
    data: &'a Archives,
}

impl Memory for Quiet<'_> {
    fn read(&self, phys: u32, out: &mut [u8]) {
        self.space.read(self.data, phys, out);
    }
}

/// The exact renderer with a [`Mutation`], on this thread.
pub struct Mutated {
    frames: Frames,
    state: State,
    renderer: RefCell<Renderer>,
    produced: Vec<Produced>,
    reclaims: u64,
    loaded: bool,
    mutation: Mutation,
    plan: Plan,
    device: wgpu::Device,
}

impl Mutated {
    pub fn new(mut renderer: Renderer, mutation: Mutation) -> Mutated {
        // As `melee_render::gpu::Gpu::new` clears a new renderer's EFB: `GXInit`'s depth mode,
        // color and alpha updates and the clear's depth.
        let mut s = State::new();
        s.bp[0x40] = 0x17;
        s.bp[0x41] = 0x18;
        s.bp[0x51] = 0xFF_FFFF;
        renderer.clear_efb(&s);
        let device = renderer.device().clone();
        Mutated {
            frames: Frames::new(),
            state: State::new(),
            renderer: RefCell::new(renderer),
            produced: Vec::new(),
            reclaims: 0,
            loaded: false,
            mutation,
            plan: Plan::default(),
            device,
        }
    }

    /// As `Gpu` does: the renderer forgets what it kept of ranges the address space gave back.
    fn forget_reclaimed(&mut self) {
        let space = &self.frames.space;
        let now = space.reclaims();
        if now == self.reclaims {
            return;
        }
        let gone = space.reclaimed_since(self.reclaims).unwrap_or_else(|| {
            vec![(
                melee_render::mem::ARCHIVES.start,
                melee_render::mem::ARCHIVES.end,
            )]
        });
        let r = self.renderer.get_mut();
        for (start, end) in gone {
            r.forget_memory(start, end);
        }
        self.reclaims = now;
    }
}

impl FrameRenderer for Mutated {
    fn name(&self) -> String {
        format!("exact, {}", self.mutation.name())
    }

    fn aim(&mut self, reference: &Geometry, draws: &[DisplayDraw]) {
        self.plan = plan(self.mutation, reference, draws);
    }

    fn frame(&mut self, t: &Tick<'_>, draw: bool, geometry: bool) -> Rendered {
        if !self.loaded {
            self.frames.space.load_dol(t.dol);
            self.loaded = true;
        }
        let start = Instant::now();
        let fifo = self.frames.frame(t.game, t.data, t.draws);
        let build = start.elapsed();
        let plan = std::mem::take(&mut self.plan);
        if !draw {
            let quiet = Quiet {
                space: &self.frames.space,
                data: t.data,
            };
            self.state.feed(&quiet, &fifo, &mut self.produced, false);
            self.produced.clear();
            return Rendered {
                build,
                ..Rendered::default()
            };
        }
        self.forget_reclaimed();
        let rec = geometry.then(|| Recorder::new(&self.frames.marks));
        let host = Host {
            space: &self.frames.space,
            data: t.data,
            renderer: &self.renderer,
            rec: rec.as_ref(),
            marks: &self.frames.marks,
            offset: Cell::new(0),
            mutation: self.mutation,
            plan: &plan,
            scratch: RefCell::new(Box::new(State::new())),
            held: RefCell::new(Vec::new()),
        };
        self.state.feed(&host, &fifo, &mut self.produced, false);
        host.release();
        drop(host);
        self.produced.clear();
        let r = self.renderer.get_mut();
        for w in r.take_written() {
            self.frames.space.write(w.address, &w.bytes);
        }
        let image = r.take_frames().into_iter().last();
        let submit = start.elapsed() - build;
        Rendered {
            image,
            geometry: rec.map(Recorder::into_geometry),
            build,
            submit,
        }
    }

    fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}
