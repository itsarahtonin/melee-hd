//! The depth buffer's encoding of the EFB's depth (`efb_depth`, as gx.wgsl's): exact and
//! strictly increasing, so depth tests compare as the EFB's integers do, and never nearer than
//! the rasterized depth gx.wgsl computes it from, which the early z pipelines promise the GPU
//! (`early_depth_test(greater_equal)`).

use super::efb_depth;

/// GXSetViewport's z range and far z for near 0 and far 1, the only ones the early z
/// pipelines draw with.
const Z_RANGE: f32 = 16_777_215.0;
const FAR_Z: f32 = 16_777_215.0;

/// What gx.wgsl writes for the rasterized depth `p`: unfused, or with the multiply-add fused,
/// as the GPU's compiler may make it.
fn written(p: f32, fused: bool) -> f32 {
    let screen_z = if fused {
        Z_RANGE.mul_add(p - 1.0, FAR_Z)
    } else {
        FAR_Z + Z_RANGE * (p - 1.0)
    };
    efb_depth(screen_z.clamp(0.0, 16_777_215.0) as u32)
}

#[test]
fn encoding_is_exact_and_increasing() {
    let mut last = -1.0f32;
    for z in 0..1u32 << 24 {
        let d = efb_depth(z);
        let exact = (f64::from(z) + if z < 0x7F_FFFF { 1.5 } else { 1.0 }) / 16_777_216.0;
        assert!(f64::from(d) == exact && d > last && d <= 1.0, "z {z:#x}");
        last = d;
    }
}

fn check_depths(step: usize) {
    for bits in (0..=1.0f32.to_bits()).step_by(step) {
        let p = f32::from_bits(bits);
        for fused in [false, true] {
            assert!(written(p, fused) >= p, "depth {p:e} (fused {fused})");
        }
    }
}

#[test]
fn written_depth_is_never_nearer_than_rasterized() {
    check_depths(61);
}

/// Every f32 depth from 0 to 1 (about 20 s in a release build).
#[test]
#[ignore]
fn written_depth_is_never_nearer_than_rasterized_every_depth() {
    check_depths(1);
}
