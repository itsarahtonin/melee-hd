// SPDX-License-Identifier: GPL-3.0-or-later

//! Anisotropic filtering (the player's Texture filtering): the choices, what the draws'
//! uniforms carry, and what gx.wgsl samples with it. A texture whose levels are each one color
//! shows which level a pixel sampled: on a surface whose footprint is 8 texels long and 1 wide,
//! GX takes level 3, and each doubling of the samples one level finer, down to level 0 at 8.
//! Skips without a GPU adapter, or on a software one.

use melee_gx::State;

use crate::tev_tests::{Rng, draw, quad, renderer, state, textures};
use crate::{Anisotropy, EFB_WIDTH, GpuVertex, MODE_DRAW, Renderer, TevMode};

/// The choices are off, 2, 4, 8 and 16; anything else is the largest not above it.
#[test]
fn the_choices_and_their_samples() {
    let samples: Vec<u32> = Anisotropy::CHOICES.iter().map(|a| a.samples()).collect();
    assert_eq!(samples, [1, 2, 4, 8, 16]);
    assert_eq!(Anisotropy::default().samples(), 1);
    assert!(Anisotropy::default().is_off() && Anisotropy::OFF.is_off());
    assert_eq!(Anisotropy::new(0), Anisotropy::OFF);
    assert_eq!(Anisotropy::new(3), Anisotropy::new(2));
    assert_eq!(Anisotropy::new(15).samples(), 8);
    assert_eq!(Anisotropy::new(99).samples(), 16);
    assert!(!Anisotropy::new(2).is_off());
}

/// The uniforms' filtering word: 0 when off, as every draw's was before the setting existed, and
/// the samples when on, set directly or through the shared setting the player changes.
#[test]
fn the_uniforms_carry_the_setting() {
    let Some(mut r) = renderer() else {
        return;
    };
    let word = |r: &mut Renderer| {
        let u = Renderer::uniforms_of(&State::new(), [1; 8], [MODE_DRAW, 0, 0, 0]);
        let at = r.push_uniforms(&u) as usize;
        let size = std::mem::size_of::<crate::Uniforms>();
        let pushed: crate::Uniforms = bytemuck::pod_read_unaligned(&r.uniforms[at..at + size]);
        pushed.filtering
    };
    assert_eq!(word(&mut r), [0; 4]);
    r.set_anisotropy(Anisotropy::new(16));
    assert_eq!(word(&mut r), [16, 0, 0, 0]);
    let shared = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(4));
    r.share_anisotropy(shared.clone());
    assert_eq!(word(&mut r), [4, 0, 0, 0]);
    shared.store(1, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(word(&mut r), [0; 4]);
    assert_eq!(r.anisotropy(), Anisotropy::OFF);
}

/// Each level's color.
const LEVEL_COLORS: [[u8; 4]; 7] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 0, 255],
    [0, 255, 255, 255],
    [255, 0, 255, 255],
    [255, 255, 255, 255],
];

/// One stage drawing texture map 0's color, opaque, with map 0 a 64 by 64 texture of 7 levels
/// repeating, filtered linearly (`min_linear`) or nearest as it shrinks, its levels nearest.
fn level_state(min_linear: bool) -> State {
    let mut s = State::new();
    s.bp[0x00] = 1;
    // Stage 0: map 0, coordinate 0, enabled, no color channel.
    s.bp[0x28] = 1 << 6 | 7 << 7;
    // Color: d = the texture's; alpha: d = konstant 0 (255); clamped.
    s.bp[0xC0] = 15 << 12 | 15 << 8 | 15 << 4 | 8 | 1 << 19;
    s.bp[0xC1] = 7 << 13 | 7 << 10 | 7 << 7 | 6 << 4 | 1 << 19;
    // Swap table 0 as it is.
    s.bp[0xF6] = 1 << 2;
    s.bp[0xF7] = 2 | 3 << 2;
    // The alpha test always passes.
    s.bp[0xF3] = 7 << 16 | 7 << 19;
    // Map 0: repeat, magnified linear, levels nearest, shrunk linear or nearest; levels 0 to 6.
    s.bp[0x80] = 1 | 1 << 2 | 1 << 4 | 1 << 5 | u32::from(min_linear) << 7;
    s.bp[0x84] = (6 * 16) << 8;
    s.bp[0x88] = 63 | 63 << 10;
    s
}

/// A quad over the middle of the EFB whose texture coordinates (all eight) change by `per_x`
/// texels a pixel across and `per_y` down.
pub(crate) fn level_quad(per_x: f32, per_y: f32) -> [GpuVertex; 6] {
    let (w, h) = (320.0, 264.0);
    let corner = |x: f32, y: f32, s: f32, t: f32| {
        let mut v = GpuVertex::zeroed();
        v.pos = [x, y, 0.5, 1.0];
        v.c0 = [255; 4];
        v.c1 = [255; 4];
        v.tex = [[s, t, 1.0]; 8];
        v
    };
    let a = corner(-0.5, 0.5, 0.0, 0.0);
    let b = corner(0.5, 0.5, w * per_x, 0.0);
    let c = corner(0.5, -0.5, w * per_x, h * per_y);
    let d = corner(-0.5, -0.5, 0.0, h * per_y);
    [a, b, c, a, c, d]
}

/// The color drawn at the EFB's middle with `a`, by the uber shader and by the specialized one
/// (which must agree).
fn drawn(r: &mut Renderer, s: &State, vs: &[GpuVertex], a: Anisotropy) -> [u8; 4] {
    let levels: Vec<Vec<u8>> = (0..7)
        .map(|l| {
            let side = 64usize >> l;
            LEVEL_COLORS[l].repeat(side * side)
        })
        .collect();
    let id = r.add_texture(64, 64, &levels);
    let (ids, counts) = ([id; 8], [7; 8]);
    r.set_anisotropy(a);
    let mut colors = Vec::new();
    for mode in [TevMode::Off, TevMode::Sync] {
        let (efb, _) = draw(r, s, vs, ids, counts, MODE_DRAW, mode);
        let at = (264 * EFB_WIDTH as usize + 320) * 4;
        colors.push([efb[at], efb[at + 1], efb[at + 2], efb[at + 3]]);
    }
    assert_eq!(
        colors[0], colors[1],
        "the specialized shader drew otherwise"
    );
    colors[0]
}

/// The level sampled on a footprint 8 texels long and 1 wide: GX's level 3 off, then one finer
/// for each doubling of the samples, down to level 0 at 8 and 16 (which take 8: the footprint's
/// length over its width).
#[test]
fn more_samples_take_finer_levels_along_a_long_footprint() {
    let Some(mut r) = renderer() else {
        return;
    };
    for (per_x, per_y) in [(1.0, 8.0), (8.0, 1.0)] {
        let s = level_state(true);
        let vs = level_quad(per_x, per_y);
        for (a, level) in Anisotropy::CHOICES.into_iter().zip([3, 2, 1, 0, 0]) {
            assert_eq!(
                drawn(&mut r, &s, &vs, a)[..3],
                LEVEL_COLORS[level][..3],
                "{a:?} along ({per_x}, {per_y})"
            );
        }
    }
}

/// GX's own sampling wherever filtering doesn't apply: a texture shrunk nearest, and a
/// footprint as wide as it's long.
#[test]
fn nearest_textures_and_square_footprints_sample_as_gx() {
    let Some(mut r) = renderer() else {
        return;
    };
    let cases = [
        (level_state(false), level_quad(1.0, 8.0), 3),
        (level_state(true), level_quad(8.0, 8.0), 3),
        (level_state(true), level_quad(2.0, 2.0), 1),
    ];
    for (s, vs, level) in cases {
        for a in Anisotropy::CHOICES {
            assert_eq!(
                drawn(&mut r, &s, &vs, a)[..3],
                LEVEL_COLORS[level][..3],
                "{a:?}"
            );
        }
    }
}

/// With anisotropic filtering on, the specialized shaders draw as the uber shader, bit for bit,
/// on random configurations (`tev_tests`' check, with 16 samples).
#[test]
fn specialized_shaders_filter_as_the_uber_shader() {
    let Some(mut r) = renderer() else {
        return;
    };
    let mut rng = Rng(0x4F1B_BCDC_BFA5_3E0B);
    let (ids, levels) = textures(&mut r, &mut rng);
    r.set_anisotropy(Anisotropy::new(16));
    for i in 0..24 {
        let s = state(&mut rng);
        let vs = quad(&mut rng);
        let uber = draw(&mut r, &s, &vs, ids, levels, MODE_DRAW, TevMode::Off);
        let special = draw(&mut r, &s, &vs, ids, levels, MODE_DRAW, TevMode::Sync);
        assert!(
            uber == special,
            "config {i}: the specialized shader drew otherwise"
        );
    }
}
