// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that when the game writes over the memory under a copy from the EFB, the renderer
//! forgets that copy and those made before it there, and keeps the copies made since inside
//! its span (`copy::Gone`; ssbm-rs a90755e): Classic's versus splash copies a portrait into
//! memory a full-screen copy held a frame before. Skips without a GPU adapter, or on a
//! software one.

use std::cell::RefCell;

use melee_gx::{Memory, Sink, State};

use crate::{Renderer, Threaded};

struct Ram(RefCell<Vec<u8>>);

impl Memory for Ram {
    fn read(&self, phys: u32, out: &mut [u8]) {
        let at = phys as usize;
        out.copy_from_slice(&self.0.borrow()[at..at + out.len()]);
    }
}

/// A copy of the `w` by `h` rectangle at the EFB's corner to `addr`, in `format` (5 RGB5A3,
/// kept on the GPU; 6 from a Z24 EFB, Z24X8, encoded into memory), `stride` bytes a row of
/// blocks: its state, every register marked changed for a threaded renderer
/// (`State::track_changes`), and BP 0x52's value.
fn copy(format: u32, depth: bool, w: u32, h: u32, addr: u32, stride: u32) -> (State, u32) {
    let mut s = State::new();
    s.bp[0x43] = if depth { 3 } else { 0 };
    s.bp[0x4A] = (w - 1) | (h - 1) << 10;
    s.bp[0x4B] = addr >> 5;
    s.bp[0x4D] = stride >> 5;
    s.track_changes();
    (s, (format * 2) << 3 | 0x52 << 24)
}

/// A full-screen RGB5A3 copy (160 blocks of 32 bytes a row, 120 rows); the game takes its
/// memory back, then copies two portraits into it, color (kept on the GPU) and depth
/// (encoded into memory); then the frame ends. Returns the three copies' addresses.
fn copies(sink: &mut dyn Sink) -> (u32, u32, u32) {
    let ram = Ram(RefCell::new(vec![0; 0x20_0000]));
    let old = 0x4_0000;
    let (s, v) = copy(5, false, 640, 480, old, 160 * 32);
    sink.copy(&s, v, &ram);
    ram.0.borrow_mut()[old as usize + 0x100] = 1;
    let color = old + 0x1_0000;
    let (s, v) = copy(5, false, 380, 400, color, 95 * 32);
    sink.copy(&s, v, &ram);
    let depth = old + 0x6_0000;
    let (s, v) = copy(6, true, 380, 400, depth, 95 * 64);
    sink.copy(&s, v, &ram);
    // The frame's end finds the old copy written over.
    let (s, _) = copy(5, false, 640, 480, 0, 640 * 2);
    sink.copy(&s, 1 << 14 | 0x52 << 24, &ram);
    (old, color, depth)
}

fn check(r: &Renderer, (old, color, depth): (u32, u32, u32), how: &str) {
    assert!(
        !r.gpu_copies.contains_key(&old),
        "{how}: the old copy is gone"
    );
    assert!(
        r.gpu_copies.contains_key(&color),
        "{how}: the newer color copy inside it stays"
    );
    assert!(
        r.copies.iter().any(|(at, _, _)| *at == depth),
        "{how}: the newer depth copy's bytes inside it stay"
    );
}

#[test]
fn a_newer_copy_outlives_an_older_one_written_over() {
    let Some(mut r) = crate::testing::renderer() else {
        return;
    };
    let at = copies(&mut r);
    check(&r, at, "on the game's thread");
    // On the renderer's own threads, the game thread numbering the copies it finds written
    // over.
    let Some(r) = crate::testing::renderer() else {
        return;
    };
    let mut t = Threaded::spawn_synced(r);
    let at = copies(&mut t);
    check(&t.into_renderer(), at, "threaded");
}
