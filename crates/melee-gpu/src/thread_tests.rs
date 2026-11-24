// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that a threaded renderer let go without `into_renderer` waits for its threads: the
//! render thread holds the device, and one left running dropped its last reference while the
//! player exited on Linux, where NVIDIA's driver crashed in `vkDestroyDevice` (`free(): invalid
//! size`). Skips without a GPU adapter, or on a software one.

use melee_gx::State;

use crate::Threaded;

#[test]
fn a_threaded_renderer_let_go_waits_for_its_render_thread() {
    let Some(r) = crate::testing::renderer() else {
        return;
    };
    let mut t = Threaded::spawn(r);
    // Work that keeps the render thread busy well after it's handed over: EFBs of 8x and back,
    // made and cleared, tens of milliseconds where letting the renderer go takes microseconds.
    let state = State::new();
    for _ in 0..4 {
        t.resize_efb(&state, 8, 640 * 8);
        t.resize_efb(&state, 1, 640);
    }
    t.send_now();
    let share = t.render_share();
    drop(t);
    assert!(
        share.upgrade().is_none(),
        "the render thread still runs after its renderer was let go"
    );
}
