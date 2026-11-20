//! Draws a frame's GX stream with melee-gpu (ported from ssbm-rs's renderer), whose frames match
//! Dolphin's software renderer within tolerance: `melee_gx` runs the stream and hands each draw
//! and EFB copy, with the GPU's state, to the renderer, which reads textures, vertex arrays
//! and display lists through the address space ([`Space`]).

use std::cell::RefCell;

use melee_data::archive::Archives;
use melee_gpu::{Renderer, Threaded};
use melee_gx::{Draw, Memory, Produced, Sink, State};

use crate::mem::Space;
use crate::meshes::Meshes;

pub use melee_gpu::Frame;
pub use melee_gpu::mesh::{Kind as MeshKind, Mesh};

/// The GPU's side of a frame: the stream's state, the renderer, and the memory it reads.
pub struct Gpu {
    pub state: State,
    renderer: RefCell<Backend>,
    produced: Vec<Produced>,
    /// How many of the address space's ranges had gone back when the renderer last forgot
    /// theirs ([`Space::reclaims`]).
    reclaims: u64,
    /// The fast renderer's mesh cache ([`Gpu::with_meshes`]): display lists drawn from meshes
    /// decoded once.
    meshes: Option<RefCell<Meshes>>,
    /// Each frame run as it's built ([`Gpu::fast`]).
    streamed: bool,
}

/// Where the renderer runs: on the game's thread, or on threads of its own that work on a frame
/// at once (decoding and transforming the vertices on one, drawing on another), each frame
/// done when the stream's copy to the XFB returns.
enum Backend {
    Here(Box<Renderer>),
    Threaded(Box<Threaded>),
}

impl Backend {
    fn sink(&mut self) -> &mut dyn Sink {
        match self {
            Backend::Here(r) => &mut **r,
            Backend::Threaded(t) => &mut **t,
        }
    }
}

struct Host<'a> {
    space: &'a Space,
    data: &'a Archives,
    renderer: &'a RefCell<Backend>,
    /// Something observing each draw (a check's recorder).
    observer: Option<&'a dyn Observer>,
    meshes: Option<&'a RefCell<Meshes>>,
}

/// Sees each draw and copy with the GPU's state, as the renderer gets them.
pub trait Observer {
    fn draw(&self, state: &State, draw: &Draw<'_>, mem: &dyn Memory);
    fn copy(&self, state: &State, value: u32);
    /// Each command of the stream itself (not those of the display lists it calls).
    fn command(&self, _bytes: &[u8]) {}
    /// A display list drawn from its mesh ([`Gpu::with_meshes`]) in place of its draws, with
    /// its vertices' clip-space positions as the GPU's transforms made them, where they ran on
    /// this thread (F3).
    fn mesh(&self, _state: &State, _mesh: &Mesh, _clips: Option<&[[f32; 4]]>) {}
}

/// An observer of a streamed frame ([`Gpu::run_streamed`]), told the frame's marks so far
/// (`Frames::marks`) before each piece runs.
pub trait StreamObserver: Observer {
    fn marks(&self, marks: &[(u32, u32)]);
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

    /// The threaded renderer fetches vertices on its own threads, unless something observes
    /// the draws.
    fn fetches(&self) -> bool {
        self.observer.is_some() || !matches!(*self.renderer.borrow(), Backend::Threaded(_))
    }

    fn command(&self, bytes: &[u8]) {
        if let Some(o) = self.observer {
            o.command(bytes);
        }
    }

    fn draw(&self, state: &State, draw: &Draw<'_>) {
        if let Some(o) = self.observer {
            o.draw(state, draw, self);
        }
        self.renderer.borrow_mut().sink().draw(state, draw, self);
    }

    fn copy(&self, state: &State, value: u32) {
        if let Some(o) = self.observer {
            o.copy(state, value);
        }
        let mut r = self.renderer.borrow_mut();
        r.sink().copy(state, value, self);
        // A check observing the draws sees memory as each copy leaves it, as the console's
        // draws after it do, rather than once the frame's stream has run.
        if self.observer.is_some() {
            for w in r.sink().take_written() {
                self.space.write(w.address, &w.bytes);
            }
        }
    }

    /// With the mesh cache, a display list drawn from its mesh where it has one.
    fn call(&self, state: &State, phys: u32, size: u32) -> bool {
        let Some(meshes) = self.meshes else {
            return false;
        };
        let Some(parts) = meshes
            .borrow_mut()
            .get(self.space, self.data, state, phys, size, self)
        else {
            return false;
        };
        let mut r = self.renderer.borrow_mut();
        for mesh in parts.iter() {
            if let Some(o) = self.observer {
                let clips = match &mut *r {
                    Backend::Here(r) if r.gpu_transforms => Some(r.mesh_clips(state, mesh)),
                    _ => None,
                };
                o.mesh(state, mesh, clips.as_deref());
            }
            match &mut *r {
                Backend::Here(r) => r.draw_mesh(state, mesh, self),
                Backend::Threaded(t) => t.draw_mesh(state, mesh, self),
            }
        }
        true
    }
}

/// Memory for a stream run without drawing: the GPU's state follows it, nothing draws.
struct Quiet<'a> {
    space: &'a Space,
    data: &'a Archives,
}

impl Memory for Quiet<'_> {
    fn read(&self, phys: u32, out: &mut [u8]) {
        self.space.read(self.data, phys, out);
    }
}

/// Clears a new renderer's EFB as `GXInit` sets the copy's clear up: black and the farthest
/// depth, color, alpha and depth all written. A new state has none of `GXInit`'s registers
/// yet, and a clear with it writes no depth, leaving the depth buffer as it was made, all
/// nearest: every depth-tested draw of the first frame would fail.
fn clear_new(renderer: &mut Renderer) {
    let mut s = State::new();
    // `GXSetZMode(GX_TRUE, GX_LEQUAL, GX_TRUE)`, color and alpha updates, the clear's depth.
    s.bp[0x40] = 0x17;
    s.bp[0x41] = 0x18;
    s.bp[0x51] = 0xFF_FFFF;
    renderer.clear_efb(&s);
}

impl Gpu {
    /// Runs a frame's stream through the GPU's state without drawing it: for frames nobody
    /// looks at, which the frames after still depend on for the registers they set.
    pub fn follow(&mut self, space: &Space, data: &Archives, fifo: &[u8]) {
        let host = Quiet { space, data };
        self.state.feed(&host, fifo, &mut self.produced, false);
        self.produced.clear();
    }

    pub fn new(mut renderer: Renderer) -> Gpu {
        let state = State::new();
        clear_new(&mut renderer);
        Gpu {
            state,
            renderer: RefCell::new(Backend::Here(Box::new(renderer))),
            produced: Vec::new(),
            reclaims: 0,
            meshes: None,
            streamed: false,
        }
    }

    /// The fast renderer's first parts (docs/fast-renderer.md, F2 and F3): each display list
    /// drawn from a mesh decoded once and kept ([`Meshes`]), rather than fetched and decoded
    /// from the stream every frame, its vertices transformed on the GPU unless the renderer's
    /// `gpu_transforms` is off (F2, whose frames are the exact renderer's).
    pub fn with_meshes(mut self) -> Gpu {
        // A large list in parts, for the transform threads to share out, unless the GPU
        // transforms it.
        let split = match self.renderer.get_mut() {
            Backend::Here(r) => !r.gpu_transforms,
            Backend::Threaded(t) => !t.gpu_transforms(),
        };
        self.meshes = Some(RefCell::new(Meshes::new(split)));
        self
    }

    /// The fast renderer (docs/fast-renderer.md): the meshes ([`Gpu::with_meshes`]) transformed
    /// on the GPU, each draw's state from its material's record, and each frame run as it's
    /// built ([`Gpu::draw`]), on the renderer's own threads unless `threaded` is false.
    pub fn fast(mut renderer: Renderer, threaded: bool) -> Gpu {
        renderer.gpu_transforms = true;
        renderer.material_records = true;
        let gpu = if threaded {
            Gpu::threaded(renderer)
        } else {
            Gpu::new(renderer)
        };
        let mut gpu = gpu.with_meshes();
        gpu.streamed = true;
        gpu
    }

    /// Whether this is the fast renderer ([`Gpu::fast`]).
    pub fn is_fast(&self) -> bool {
        self.streamed
    }

    /// Builds and draws a frame with `frames`: the fast renderer runs it as it's built
    /// ([`Gpu::run_streamed`]), the exact one once `Frames::frame_blended` has built it. Returns
    /// the frames it copied to the XFB.
    pub fn draw(
        &mut self,
        frames: &mut crate::frame::Frames,
        game: &melee_sim::game::state::Game,
        data: &Archives,
        draws: &[melee_sim::hsd::display::Draw],
        blend: Option<crate::blend::Blend<'_>>,
    ) -> Vec<Frame> {
        if self.streamed {
            self.run_streamed(frames, game, data, draws, blend, None)
        } else {
            let fifo = frames.frame_blended(game, data, draws, blend);
            self.run(&frames.space, data, &fifo, None)
        }
    }

    /// The mesh cache, with [`Gpu::with_meshes`].
    pub fn meshes(&self) -> Option<std::cell::Ref<'_, Meshes>> {
        self.meshes.as_ref().map(RefCell::borrow)
    }

    /// The renderer on threads of its own (`melee_gpu::Threaded::spawn_synced`): the same
    /// frames, the work on a frame split across three threads.
    pub fn threaded(mut renderer: Renderer) -> Gpu {
        let mut state = State::new();
        clear_new(&mut renderer);
        state.track_changes();
        Gpu {
            state,
            renderer: RefCell::new(Backend::Threaded(Box::new(Threaded::spawn_synced(
                renderer,
            )))),
            produced: Vec::new(),
            reclaims: 0,
            meshes: None,
            streamed: false,
        }
    }

    /// Makes the renderer's EFB `scale` times the console's tall and `width` wide for its 640
    /// (a resized window's), between frames, in place (`melee_gpu::Renderer::resize_efb`),
    /// and clears it as the last frame's copy to the XFB left the old one. The renderer, its
    /// textures, pipelines and threads stay, and so does the GPU's state: the stream sets much
    /// of it once (`GXInit`, at the first frame) or only when it changes (HSD's state cache),
    /// so the frames after need it as it is.
    pub fn resize_efb(&mut self, scale: u32, width: u32) {
        match self.renderer.get_mut() {
            Backend::Here(r) => {
                r.resize_efb(scale, width);
                r.clear_efb(&self.state);
            }
            Backend::Threaded(t) => t.resize_efb(&self.state, scale, width),
        }
    }

    /// Waits for what was sent to be drawn and every specialized pipeline asked for to be made,
    /// and counts the draw calls with and without them afresh: a warm-up's end
    /// (`melee_gpu::Renderer::finish_specialized`).
    pub fn finish_specialized(&mut self) {
        match self.renderer.get_mut() {
            Backend::Here(r) => r.finish_specialized(),
            Backend::Threaded(t) => t.finish_specialized(),
        }
    }

    /// The renderer on this thread, to set or read directly; none while it runs on its own.
    pub fn renderer(&self) -> Option<std::cell::RefMut<'_, Renderer>> {
        std::cell::RefMut::filter_map(self.renderer.borrow_mut(), |b| match b {
            Backend::Here(r) => Some(&mut **r),
            Backend::Threaded(_) => None,
        })
        .ok()
    }

    /// The renderer, its threads stopped once they've drawn what they were sent.
    pub fn into_renderer(self) -> Renderer {
        match self.renderer.into_inner() {
            Backend::Here(r) => *r,
            Backend::Threaded(t) => t.into_renderer(),
        }
    }

    /// Has the EFB copies in texture formats `formats` also written to memory from the next
    /// copy on (`Renderer::copies_to_memory`), still sampled from the GPU: the player's
    /// Camera Mode snapshot, whose pixels the game takes from memory.
    pub fn set_copies_to_memory(&mut self, formats: Vec<u32>) {
        match &mut *self.renderer.borrow_mut() {
            Backend::Here(r) => r.copies_to_memory = formats,
            Backend::Threaded(t) => t.set_copies_to_memory(formats),
        }
    }

    /// Has the renderer forget what it kept by address of the ranges the address space gave
    /// back since it last did: their addresses may hold other archives' bytes now.
    fn forget_reclaimed(&mut self, space: &Space) {
        let now = space.reclaims();
        if now == self.reclaims {
            return;
        }
        let gone = space
            .reclaimed_since(self.reclaims)
            .unwrap_or_else(|| vec![(crate::mem::ARCHIVES.start, crate::mem::ARCHIVES.end)]);
        let sink = self.renderer.get_mut().sink();
        for (start, end) in gone {
            sink.forget_memory(start, end);
        }
        self.reclaims = now;
    }

    /// Runs a frame's stream; returns the frames it copied to the XFB.
    pub fn run(
        &mut self,
        space: &Space,
        data: &Archives,
        fifo: &[u8],
        observer: Option<&dyn Observer>,
    ) -> Vec<Frame> {
        self.begin(space);
        self.feed(space, data, fifo, observer);
        self.end(space)
    }

    /// Builds a frame with `frames` and runs its stream in pieces as it's built
    /// (`Frames::frame_streamed`), so the renderer's threads draw its first draws while the
    /// rest are built; returns the frames it copied to the XFB. The frame is the one
    /// [`Gpu::run`] draws from `Frames::frame_blended`'s stream.
    pub fn run_streamed(
        &mut self,
        frames: &mut crate::frame::Frames,
        game: &melee_sim::game::state::Game,
        data: &Archives,
        draws: &[melee_sim::hsd::display::Draw],
        blend: Option<crate::blend::Blend<'_>>,
        observer: Option<&dyn StreamObserver>,
    ) -> Vec<Frame> {
        struct To<'a, 'g> {
            gpu: &'g mut Gpu,
            data: &'a Archives,
            observer: Option<&'a dyn StreamObserver>,
        }
        impl crate::frame::Stream for To<'_, '_> {
            fn start(&mut self, space: &Space) {
                self.gpu.begin(space);
            }
            fn piece(&mut self, space: &Space, bytes: &[u8], marks: &[(u32, u32)]) {
                if let Some(o) = self.observer {
                    o.marks(marks);
                }
                let o = self.observer.map(|o| o as &dyn Observer);
                self.gpu.feed(space, self.data, bytes, o);
            }
        }
        frames.frame_streamed(
            game,
            data,
            draws,
            blend,
            &mut To {
                gpu: self,
                data,
                observer,
            },
        );
        self.end(&frames.space)
    }

    /// A frame's run starts: the renderer forgets the ranges the address space gave back, the
    /// mesh cache starts its frame.
    fn begin(&mut self, space: &Space) {
        self.forget_reclaimed(space);
        if let Some(m) = &mut self.meshes {
            m.get_mut().begin_frame();
        }
    }

    /// Runs the frame's stream's next bytes.
    fn feed(
        &mut self,
        space: &Space,
        data: &Archives,
        fifo: &[u8],
        observer: Option<&dyn Observer>,
    ) {
        let host = Host {
            space,
            data,
            renderer: &self.renderer,
            observer,
            meshes: self.meshes.as_ref(),
        };
        self.state.feed(&host, fifo, &mut self.produced, false);
        self.produced.clear();
    }

    /// The frame's run ends: what its copies wrote goes to memory, and the frames it copied to
    /// the XFB are handed back.
    fn end(&mut self, space: &Space) -> Vec<Frame> {
        let mut r = self.renderer.borrow_mut();
        for w in r.sink().take_written() {
            space.write(w.address, &w.bytes);
        }
        match &mut *r {
            Backend::Here(r) => r.take_frames(),
            Backend::Threaded(t) => t.take_frames(),
        }
    }
}
