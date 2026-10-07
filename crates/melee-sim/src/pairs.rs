//! Loads and stores for paired-single code, as the Gekko does them with GQR0 (plain floats).

use gekko_fp::{self as fp, Ps};

/// `psq_l` of two floats.
pub fn load2(a: f32, b: f32) -> Ps {
    Ps::new(fp::lfs(a.to_bits()), fp::lfs(b.to_bits()))
}

/// `psq_l` of one float: the second lane reads 1.0.
pub fn load1(a: f32) -> Ps {
    Ps::new(fp::lfs(a.to_bits()), 1.0)
}

/// `lfs`, which fills both lanes.
pub fn splat(a: f32) -> Ps {
    Ps::splat(fp::lfs(a.to_bits()))
}

/// A register as `stfs` stores it.
pub fn store(lane: f64) -> f32 {
    f32::from_bits(fp::stfs(lane))
}

/// A lane as `psq_st` stores it: on hardware a subnormal result becomes zero.
pub fn store_paired(lane: f64) -> f32 {
    f32::from_bits(fp::stfs_ftz(lane))
}

/// A float moved through a register unchanged: loaded, then stored with `stfs`.
pub fn moved(a: f32) -> f32 {
    store(fp::lfs(a.to_bits()))
}

/// The same, stored with `psq_st`.
pub fn moved_paired(a: f32) -> f32 {
    store_paired(fp::lfs(a.to_bits()))
}
