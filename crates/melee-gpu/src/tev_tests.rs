// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks the specialized TEV shaders against the uber shader: random configurations (stages,
//! inputs, operations, swaps, konstants, indirect stages, texture modes, alpha tests, fog, z
//! textures, EFB formats) with random data, drawn with each, must leave the EFB's color and
//! depth bit for bit the same. Skips without a GPU adapter.

use melee_gx::State;

use crate::{
    EFB_HEIGHT, EFB_WIDTH, GpuVertex, MODE_DEPTH_ONLY, MODE_DRAW, PendingClear, PipelineKey,
    Renderer, TevMode, tev,
};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
}

fn renderer() -> Option<Renderer> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::DUAL_SOURCE_BLENDING,
        ..Default::default()
    }))
    .ok()?;
    Some(Renderer::with_device(device, queue))
}

/// The textures' sizes, as the maps' image registers name them.
const SIZES: [(u32, u32); 8] = [
    (16, 16),
    (8, 32),
    (32, 8),
    (4, 4),
    (64, 64),
    (16, 8),
    (1, 1),
    (32, 32),
];

/// Random textures, the larger with mip levels.
fn textures(r: &mut Renderer, rng: &mut Rng) -> ([u32; 8], [u32; 8]) {
    let mut ids = [0; 8];
    let mut levels = [1; 8];
    for (map, &(w, h)) in SIZES.iter().enumerate() {
        let n = if w >= 16 && h >= 8 { 3 } else { 1 };
        let data: Vec<Vec<u8>> = (0..n)
            .map(|l| {
                let (lw, lh) = (((w - 1) >> l) + 1, ((h - 1) >> l) + 1);
                (0..lw * lh * 4).map(|_| rng.next() as u8).collect()
            })
            .collect();
        ids[map] = r.add_texture(w, h, &data);
        levels[map] = n;
    }
    (ids, levels)
}

/// A random configuration, with random data in the registers beside it.
fn state(rng: &mut Rng) -> State {
    let mut s = State::new();
    let ntex = 1 + rng.below(8);
    let nstages = rng.below(8);
    let nind = if rng.below(4) == 0 { rng.below(3) } else { 0 };
    s.bp[0x00] = ntex | nstages << 10 | nind << 16;
    // Indirect matrices, stages, scales and orders.
    for r in 0x06..=0x0E {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    for n in 0..16 {
        // Mostly no indirect texturing, as in the game.
        s.bp[0x10 + n] = if rng.below(3) == 0 {
            rng.next() & 0x1F_FFFF
        } else {
            0
        };
    }
    for r in 0x25..=0x27 {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    for r in 0x28..=0x2F {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    s.bp[0x41] = rng.next() & 0xFFFF;
    s.bp[0x42] = rng.next() & 0x1FF;
    s.bp[0x43] = rng.below(2) | rng.below(2) << 6;
    for (map, &(w, h)) in SIZES.iter().enumerate() {
        let at = (map & 3) + if map >= 4 { 0x20 } else { 0 };
        s.bp[0x80 + at] = rng.next() & 0x1_FFFF;
        s.bp[0x84 + at] = rng.below(64) | (rng.below(96) + 16) << 8;
        s.bp[0x88 + at] = (w - 1) | (h - 1) << 10;
    }
    for r in 0xC0..=0xDF {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    // Fog: A (0.5 to 4) and C (-1 to 1) as floats (sign 19, exponent 11 to 18, mantissa 0 to
    // 10), and the perspective fog's magnitude (2^23 to 2^24) and shift (at least 1), so its
    // value mostly lies between 0 and 1, where a rounding shows.
    let float = |sign: u32, exp: u32, mantissa: u32| sign << 19 | exp << 11 | mantissa;
    let a = float(0, 126 + rng.below(3), rng.below(2048));
    let c = float(rng.below(2), 120 + rng.below(7), rng.below(2048));
    s.bp[0xEE] = a;
    s.bp[0xEF] = 0x80_0000 | rng.next() & 0x7F_FFFF;
    s.bp[0xF0] = 1 + rng.below(4);
    s.bp[0xF1] = c | rng.below(2) << 20 | rng.below(8) << 21;
    s.bp[0xF2] = rng.next() & 0xFF_FFFF;
    s.bp[0xE8] = rng.next() & 0x7FF;
    for r in 0xE9..=0xED {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    s.bp[0xF3] = rng.next() & 0xFF_FFFF;
    s.bp[0xF4] = rng.next() & 0xFF_FFFF;
    s.bp[0xF5] = rng.below(16);
    for r in 0xF6..=0xFD {
        s.bp[r] = rng.next() & 0xFF_FFFF;
    }
    for i in 0..4 {
        s.color_ra[i] = rng.next() & 0x3F_FFFF;
        s.color_bg[i] = rng.next() & 0x3F_FFFF;
        s.konst_ra[i] = rng.next() & 0x3F_FFFF;
        s.konst_bg[i] = rng.next() & 0x3F_FFFF;
    }
    s
}

/// A quad over part of the EFB with random colors, depths and texture coordinates.
fn quad(rng: &mut Rng) -> [GpuVertex; 6] {
    let scale = 2f32.powi(rng.below(12) as i32 - 3);
    let perspective = rng.below(2) == 0;
    let corner = |rng: &mut Rng, x: f32, y: f32| {
        let mut v = GpuVertex::zeroed();
        v.pos = [x, y, rng.unit(), 1.0];
        v.c0 = (rng.next()).to_le_bytes();
        v.c1 = (rng.next()).to_le_bytes();
        for t in &mut v.tex {
            let q = if perspective {
                0.5 + rng.unit() * 1.5
            } else {
                1.0
            };
            *t = [
                (rng.unit() - 0.3) * 64.0 * scale * q,
                (rng.unit() - 0.3) * 64.0 * scale * q,
                q,
            ];
        }
        v
    };
    let (x0, y0) = (-0.95 + rng.unit() * 0.2, 0.95 - rng.unit() * 0.2);
    let (x1, y1) = (x0 + 1.0 + rng.unit() * 0.8, y0 - 1.0 - rng.unit() * 0.8);
    let a = corner(rng, x0, y0);
    let b = corner(rng, x1, y0);
    let c = corner(rng, x1, y1);
    let d = corner(rng, x0, y1);
    [a, b, c, a, c, d]
}

/// The EFB's depth as stored, read back.
fn read_depth(r: &Renderer) -> Vec<u8> {
    let (width, height) = (EFB_WIDTH, EFB_HEIGHT);
    let row = width * 4;
    let buffer = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = r.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: r.depth_view.texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    r.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    r.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback");
    slice.get_mapped_range().expect("mapped").to_vec()
}

/// Draws the quad with the state in `mode` (uber or specialized) on a cleared EFB, and reads
/// back its color and depth.
fn draw(
    r: &mut Renderer,
    s: &State,
    vs: &[GpuVertex],
    ids: [u32; 8],
    levels: [u32; 8],
    draw_mode: u32,
    tev_mode: TevMode,
) -> (Vec<u8>, Vec<u8>) {
    r.tev.mode = tev_mode;
    r.pending_clear = PendingClear {
        color: Some([10, 200, 30, 140]),
        depth: Some(0x80_0000),
    };
    let key = PipelineKey {
        blend: None,
        const_alpha: false,
        write_color: true,
        write_alpha: true,
        depth_compare: Some(7),
        depth_write: true,
        early_z: false,
    };
    let first = r.indices.len() as u32;
    let tg = (s.bp[0x00] & 0xF).min(8) as usize;
    r.push_in_order(vs, tg);
    let mut u = Renderer::uniforms_of(s, levels, [draw_mode, 0, 0, 0]);
    u.depth = [16_777_215.0, 16_777_215.0, 0.0, 0.0];
    u.fog[2] = 320.0;
    let uniform = r.push_uniforms(&u);
    r.pipeline(key);
    r.bind_group(ids);
    let tev = r.specialized(s, key, draw_mode);
    assert_eq!(tev != tev::UBER, tev_mode == TevMode::Sync);
    r.push_command(crate::Command {
        pipeline: key,
        tev,
        uniform,
        textures: ids,
        texgens: tg as u32,
        first,
        count: vs.len() as u32,
        scissor: [0, 0, EFB_WIDTH, EFB_HEIGHT],
    });
    r.flush();
    (r.read_efb(), read_depth(r))
}

fn check(seed: u64, configs: u32) {
    let Some(mut r) = renderer() else {
        eprintln!("no GPU adapter: skipped");
        return;
    };
    let mut rng = Rng(seed);
    let (ids, levels) = textures(&mut r, &mut rng);
    let mut drawn = 0;
    for i in 0..configs {
        let s = state(&mut rng);
        let vs = quad(&mut rng);
        let mode = if rng.below(6) == 0 {
            MODE_DEPTH_ONLY
        } else {
            MODE_DRAW
        };
        let uber = draw(&mut r, &s, &vs, ids, levels, mode, TevMode::Off);
        let special = draw(&mut r, &s, &vs, ids, levels, mode, TevMode::Sync);
        if let Some(at) = (0..uber.0.len()).find(|&j| uber.0[j] != special.0[j]) {
            let p = at / 4;
            panic!(
                "config {i} (seed {seed}): color differs at ({}, {}): uber {:?}, specialized {:?}\n{}",
                p % EFB_WIDTH as usize,
                p / EFB_WIDTH as usize,
                &uber.0[p * 4..p * 4 + 4],
                &special.0[p * 4..p * 4 + 4],
                tev::Uid::of(&s.bp, mode).source(false)
            );
        }
        if let Some(at) = (0..uber.1.len()).find(|&j| uber.1[j] != special.1[j]) {
            let p = at / 4;
            panic!(
                "config {i} (seed {seed}): depth differs at ({}, {})",
                p % EFB_WIDTH as usize,
                p / EFB_WIDTH as usize
            );
        }
        // Something drew: not everything discarded.
        if uber.0.chunks(4).any(|c| c != [10, 200, 30, 140]) {
            drawn += 1;
        }
    }
    let info = r.device.adapter_info();
    eprintln!(
        "{drawn} of {configs} drew; {} pipelines made in {:?}, longest {:?} ({} {:?})",
        r.tev.stats.made, r.tev.stats.compile, r.tev.stats.longest, info.name, info.backend
    );
    assert!(drawn > configs / 4, "only {drawn} of {configs} drew");
}

#[test]
fn specialized_shaders_draw_as_the_uber_shader() {
    check(0x9E37_79B9_7F4A_7C15, 48);
}

/// Many more configurations (minutes: each specialized pipeline compiles).
#[test]
#[ignore]
fn specialized_shaders_draw_as_the_uber_shader_many() {
    check(0xD1B5_4A32_D192_ED03, 1000);
}

/// A uid's words round trip, and two draws with the same configuration and other data share
/// one.
#[test]
fn uids_round_trip_and_ignore_data() {
    let mut rng = Rng(7);
    for _ in 0..200 {
        let mut s = state(&mut rng);
        let uid = tev::Uid::of(&s.bp, MODE_DRAW);
        assert_eq!(tev::Uid::from_words(&uid.0), Some(uid));
        // Data: konstant and register values, reference values, fog's constants, sizes.
        s.bp[0xF3] ^= 0xFFFF;
        s.bp[0xEE] ^= 0x1234;
        s.bp[0x88] ^= 0x3FF;
        s.bp[0xF4] ^= 0xFF;
        assert_eq!(tev::Uid::of(&s.bp, MODE_DRAW), uid);
        s.bp[0xC0] ^= 1;
        assert_ne!(tev::Uid::of(&s.bp, MODE_DRAW), uid);
    }
}
