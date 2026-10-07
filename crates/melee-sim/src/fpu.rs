//! Moving floats between memory and registers as the Gekko does (see `docs/porting.md`).

use gekko_fp as fp;

/// A float as a register holds it after `lfs`.
pub fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs`.
pub fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

/// A float copied through a register: `lfs`, then `stfs`.
pub fn copy(x: f32) -> f32 {
    mem(reg(x))
}
