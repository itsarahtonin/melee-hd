// SPDX-License-Identifier: GPL-3.0-or-later

//! The float mode and the fused multiply-add path are per thread, though they're read without
//! a thread-local while no thread has changed them. Alone in its own process, so no other
//! test's threads are counted.

use gekko_fp::{FpMode, set_fp_mode, set_portable_fma};

/// One thread's mode and fused multiply-add path don't reach another's: while a thread is in
/// Slippi's mode on the portable path, a new thread computes as the hardware does, on the
/// hardware path, and the first keeps its own until it sets them back.
#[test]
fn modes_are_per_thread() {
    // A sum whose sign of zero differs between the modes: -(1 * 1 - 1).
    let ours = || gekko_fp::fnmsubs(1.0, 1.0, 1.0).to_bits();
    set_fp_mode(FpMode::Slippi);
    set_portable_fma(true);
    assert!(gekko_fp::portable_fma());
    let slippi = ours();
    std::thread::spawn(move || {
        assert_eq!(gekko_fp::fp_mode(), FpMode::Hardware);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(
            gekko_fp::portable_fma(),
            !std::arch::is_x86_feature_detected!("fma")
        );
        assert_ne!(ours(), slippi);
        set_fp_mode(FpMode::Slippi);
        assert_eq!(ours(), slippi);
        set_fp_mode(FpMode::Hardware);
    })
    .join()
    .unwrap();
    assert_eq!(gekko_fp::fp_mode(), FpMode::Slippi);
    assert!(gekko_fp::portable_fma());
    assert_eq!(ours(), slippi);
    set_portable_fma(false);
    set_fp_mode(FpMode::Hardware);
    assert_ne!(ours(), slippi);
}
