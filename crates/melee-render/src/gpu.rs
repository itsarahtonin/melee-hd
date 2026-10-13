//! Draws a frame's GX stream with melee-gpu (ported from ssbm-rs's renderer), whose frames match
//! Dolphin's software renderer within tolerance: `melee_gx` runs the stream and hands each draw
//! and EFB copy, with the GPU's state, to the renderer, which reads textures, vertex arrays
//! and display lists through the address space ([`Space`]).

use std::cell::RefCell;

use melee_data::archive::Archives;
use melee_gpu::{Renderer, Threaded};
use melee_gx::{Draw, Memory, Produced, Sink, State};

use crate::mem::Space;

pub use melee_gpu::Frame;

/// The GPU's side of a frame: the stream's state, the renderer, and the memory it reads.
pub struct Gpu {
    pub state: State,
    renderer: RefCell<Backend>,
    produced: Vec<Produced>,
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
}

/// Sees each draw and copy with the GPU's state, as the renderer gets them.
pub trait Observer {
    fn draw(&self, state: &State, draw: &Draw<'_>, mem: &dyn Memory);
    fn copy(&self, state: &State, value: u32);
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
        self.renderer.borrow_mut().sink().copy(state, value, self);
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
        renderer.clear_efb(&state);
        Gpu {
            state,
            renderer: RefCell::new(Backend::Here(Box::new(renderer))),
            produced: Vec::new(),
        }
    }

    /// The renderer on threads of its own (`melee_gpu::Threaded::spawn_synced`): the same
    /// frames, the work on a frame split across three threads.
    pub fn threaded(mut renderer: Renderer) -> Gpu {
        let mut state = State::new();
        renderer.clear_efb(&state);
        state.track_changes();
        Gpu {
            state,
            renderer: RefCell::new(Backend::Threaded(Box::new(Threaded::spawn_synced(renderer)))),
            produced: Vec::new(),
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

    /// Runs a frame's stream; returns the frames it copied to the XFB.
    pub fn run(
        &mut self,
        space: &Space,
        data: &Archives,
        fifo: &[u8],
        observer: Option<&dyn Observer>,
    ) -> Vec<Frame> {
        let host = Host {
            space,
            data,
            renderer: &self.renderer,
            observer,
        };
        self.state.feed(&host, fifo, &mut self.produced, false);
        self.produced.clear();
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
