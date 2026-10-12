//! Draws a frame's GX stream with ssbm-rs's renderer (ssbm-render), whose frames match
//! Dolphin's software renderer within tolerance: ssbm-gx runs the stream and hands each draw
//! and EFB copy, with the GPU's state, to the renderer, which reads textures, vertex arrays
//! and display lists through the address space ([`Space`]).

use std::cell::RefCell;

use melee_data::archive::Archives;
use ssbm_gx::{Draw, Memory, Produced, Sink, State};
use ssbm_render::Renderer;

use crate::mem::Space;

pub use ssbm_render::Frame;

/// The GPU's side of a frame: the stream's state, the renderer, and the memory it reads.
pub struct Gpu {
    pub state: State,
    pub renderer: RefCell<Renderer>,
    produced: Vec<Produced>,
}

struct Host<'a> {
    space: &'a Space,
    data: &'a Archives,
    renderer: &'a RefCell<Renderer>,
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

    fn draws(&self) -> bool {
        true
    }

    fn draw(&self, state: &State, draw: &Draw<'_>) {
        if let Some(o) = self.observer {
            o.draw(state, draw, self);
        }
        self.renderer.borrow_mut().draw(state, draw, self);
    }

    fn copy(&self, state: &State, value: u32) {
        if let Some(o) = self.observer {
            o.copy(state, value);
        }
        self.renderer.borrow_mut().copy(state, value, self);
    }
}

impl Gpu {
    pub fn new(mut renderer: Renderer) -> Gpu {
        let state = State::new();
        renderer.clear_efb(&state);
        Gpu {
            state,
            renderer: RefCell::new(renderer),
            produced: Vec::new(),
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
        for w in r.take_written() {
            space.write(w.address, &w.bytes);
        }
        r.take_frames()
    }
}
