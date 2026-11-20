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

/// What gx.wgsl writes, on the GPU, for flat quads at random rasterized depths with random z
/// ranges and far z: the CPU's depth, unfused, bit for bit. gx.wgsl keeps the product apart from
/// the add (`opaque`), so no compiler fuses it, and Metal's fast maths (which may also reorder)
/// is checked on CI's macOS runner. Skips without a GPU adapter, or on a software one.
#[test]
fn the_gpu_writes_the_cpu_s_depth() {
    use crate::tev_tests::{Rng, read_depth, renderer};
    use crate::{EFB_HEIGHT, EFB_WIDTH, GpuVertex, PipelineKey, Renderer};
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    // Cells of 20 by 20 pixels, a quad in each.
    const CELL: u32 = 20;
    let (cols, rows) = (EFB_WIDTH / CELL, EFB_HEIGHT / CELL);
    let key = PipelineKey {
        blend: None,
        const_alpha: false,
        write_color: true,
        write_alpha: true,
        depth_compare: Some(7),
        depth_write: true,
        early_z: false,
        vertices: Default::default(),
    };
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut checked = 0;
    for round in 0..8 {
        let state = melee_gx::State::new();
        let mut want = Vec::new();
        for cell in 0..cols * rows {
            let (cx, cy) = (cell % cols, cell / cols);
            let p = match rng.below(4) {
                0 => f32::from_bits(rng.next() % 0x3F80_0001),
                _ => rng.next() as f32 / u32::MAX as f32,
            };
            let range = (rng.next() % (1 << 24)) as f32;
            let far = range + (rng.next() % (1 << 24)) as f32 * (1.0 - range / 16_777_216.0);
            let far = far.min(16_777_215.0);
            // The pixels' centres, in clip space.
            let x = |c: u32| (c as f32 * CELL as f32 + 2.0) / EFB_WIDTH as f32 * 2.0 - 1.0;
            let y = |c: u32| 1.0 - (c as f32 * CELL as f32 + 2.0) / EFB_HEIGHT as f32 * 2.0;
            let (x0, x1, y0, y1) = (x(cx), x(cx) + 0.04, y(cy), y(cy) - 0.04);
            let corner = |x: f32, y: f32| {
                let mut v = GpuVertex::zeroed();
                v.pos = [x, y, p, 1.0];
                v
            };
            let (a, b, c, d) = (
                corner(x0, y0),
                corner(x1, y0),
                corner(x1, y1),
                corner(x0, y1),
            );
            let first = r.indices.len() as u32;
            r.push_in_order(&[a, b, c, a, c, d], 0);
            let mut u = Renderer::uniforms_of(&state, [1; 8], [3, 0, 0, 0]);
            u.depth = [range, far, 0.0, 0.0];
            let uniform = r.push_uniforms(&u);
            r.pipeline(key);
            r.bind_group([0; 8]);
            r.push_command(crate::Command {
                pipeline: key,
                tev: crate::tev::UBER,
                uniform,
                textures: [0; 8],
                texgens: 0,
                first,
                count: 6,
                scissor: [0, 0, EFB_WIDTH, EFB_HEIGHT],
            });
            let z = (far + range * (p - 1.0)).clamp(0.0, 16_777_215.0) as u32;
            want.push(((cx * CELL + 5, cy * CELL + 5), p, range, far, efb_depth(z)));
        }
        r.flush();
        let depth = read_depth(&r);
        for ((x, y), p, range, far, d) in want {
            let at = 4 * (y * EFB_WIDTH + x) as usize;
            let got = f32::from_le_bytes(depth[at..at + 4].try_into().unwrap());
            assert!(
                got.to_bits() == d.to_bits(),
                "round {round}: depth {p:e} ({:#x}), z range {range}, far z {far}: the GPU wrote {got} ({}), the CPU {d} ({})",
                p.to_bits(),
                (got * 16_777_216.0) as u32,
                (d * 16_777_216.0) as u32
            );
            checked += 1;
        }
    }
    eprintln!("{checked} depths checked");
}

/// Every f32 depth from 0 to 1 (about 20 s in a release build).
#[test]
#[ignore]
fn written_depth_is_never_nearer_than_rasterized_every_depth() {
    check_depths(1);
}
