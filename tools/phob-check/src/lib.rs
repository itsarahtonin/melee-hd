// SPDX-License-Identifier: GPL-3.0-only

//! A differential check of `crates/melee-phob` against PhobGCC-SW's own source, the way
//! leaf-check compares the simulation's leaves with ssbm-rs: `build.rs` builds upstream's
//! `PhobGCC/common` math (and the functions it needs from `phobGCC.h` and the RP2040 board's
//! `readHardware.h`) from a checkout at the pinned commit, with the host's C++ compiler, into a
//! test-only library; the tests drive both with the same random and edge inputs and compare
//! every float bit for bit (both call the platform's `atan2f`, `sinf`, `pow` and the rest).
//!
//! `PHOBGCC_SW` names the checkout (`git clone https://github.com/PhobGCC/PhobGCC-SW` and
//! `git checkout b4f175ea`). Without it the tests skip, or under `MELEE_CHECKS_REQUIRE_DATA`
//! fail. Nothing of upstream's reaches the player: only this tool's tests link it.

/// Upstream's functions, as `oracle/api.cpp` exposes them.
#[cfg(phob_oracle)]
pub mod ffi {
    unsafe extern "C" {
        pub fn phob_reset();
        pub fn phob_set_controls(v: *const i32);
        pub fn phob_norm_gains(out: *mut f32);
        pub fn phob_get_params(stick: i32, out: *mut f32);
        pub fn phob_params_from(
            stick: i32,
            px: *const f32,
            py: *const f32,
            angles: *const f32,
            status: *mut i32,
        );
        pub fn phob_calc_stick_values(angle: f32, out: *mut f32);
        pub fn phob_angle_on_sphere(x: f32, y: f32) -> f32;
        pub fn phob_notch_remap(x: f32, y: f32, stick: i32, cal_step: i32, out: *mut f32);
        pub fn phob_fit_curve(px: *const f64, py: *const f64, coeffs: *mut f64) -> i32;
        pub fn phob_legalize_notches(step: i32, measured: *const f32, angles: *mut f32);
        pub fn phob_read_sticks(
            raw: *const u8,
            read_a: i32,
            read_c: i32,
            cal_step: i32,
            currently_raw: i32,
            out: *mut u8,
            raw_out: *mut f32,
        );
        pub fn phob_buttons(buttons: i32, la: i32, ra: i32, triggers: *mut u8) -> i32;
        pub fn phob_cal_store(stick: i32, px: *const f32, py: *const f32, angles: *const f32);
        pub fn phob_cal_start(stick: i32);
        pub fn phob_cal_advance(xs: *const u8, ys: *const u8);
        pub fn phob_cal_undo();
        pub fn phob_cal_skip();
        pub fn phob_cal_adjust(dt: f32, cw: i32, ccw: i32, reset: i32);
        pub fn phob_cal_get(step_undo: *mut i32, floats: *mut f32, status: *mut i32);
    }
}
