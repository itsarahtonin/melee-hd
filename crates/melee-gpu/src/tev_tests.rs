// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks the specialized TEV shaders against the uber shader: random configurations (stages,
//! inputs, operations, swaps, konstants, indirect stages, texture modes, alpha tests, fog, z
//! textures, EFB formats) with random data, drawn with each, must leave the EFB's color and
//! depth bit for bit the same. Skips without a GPU adapter, or on a software one.

use melee_gx::State;

use crate::{
    EFB_HEIGHT, EFB_WIDTH, GpuVertex, MODE_DEPTH_ONLY, MODE_DRAW, PendingClear, PipelineKey,
    Renderer, SharedSpecialized, TevMode, tev,
};

pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    pub(crate) fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
}

pub(crate) fn renderer() -> Option<Renderer> {
    let adapter = crate::testing::adapter()?;
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
pub(crate) fn state(rng: &mut Rng) -> State {
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
pub(crate) fn quad(rng: &mut Rng) -> [GpuVertex; 6] {
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
pub(crate) fn read_depth(r: &Renderer) -> Vec<u8> {
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
    r.queue.submit(encoder);
    r.queue.drain();
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
        vertices: Default::default(),
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
    assert_eq!(tev != tev::UBER, tev_mode != TevMode::Off);
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
        eprintln!("skipped: no hardware GPU adapter");
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

/// Two renderers on one device sharing their specialized pipelines (the player's menus' and a
/// match's): the second takes every pipeline the first made, making none, and draws with them
/// as the first did, bit for bit, with its own bind groups.
#[test]
fn a_renderer_draws_with_the_specialized_pipelines_another_made() {
    let Some(mut a) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    let mut b = Renderer::with_device(a.device.clone(), a.queue().clone());
    let shared = SharedSpecialized::default();
    a.share_specialized(Some(shared.clone()));
    b.share_specialized(Some(shared.clone()));
    let seed = 0x2545_F491_4F6C_DD1D;
    let (ids_a, levels) = textures(&mut a, &mut Rng(seed));
    let (ids_b, _) = textures(&mut b, &mut Rng(seed));
    let mut rng = Rng(seed ^ 1);
    let configs = 12;
    for i in 0..configs {
        let s = state(&mut rng);
        let vs = quad(&mut rng);
        let made = draw(&mut a, &s, &vs, ids_a, levels, MODE_DRAW, TevMode::Sync);
        // Sync makes any pipeline the shared ones lack: none must be.
        let taken = draw(&mut b, &s, &vs, ids_b, levels, MODE_DRAW, TevMode::Sync);
        assert!(
            made == taken,
            "config {i}: the shared pipeline drew differently"
        );
    }
    assert_eq!(b.tev.stats.made, 0, "the second renderer made its own");
    assert_eq!(b.tev.stats.shared as usize, shared.len());
    assert_eq!(a.tev.stats.made as usize, shared.len());
    assert!(shared.len() > configs / 2);
}

/// The saved list keeps the order the pipelines were first asked for (so the next run makes
/// first what it's likely to need first), through a renderer warmed from it; a bounded wait for
/// them returns with what's made, and the rest are made after.
#[test]
fn the_saved_list_keeps_the_order_first_asked_for() {
    let Some(mut a) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    let mut rng = Rng(0x5DEE_CE66);
    let mut asked = Vec::new();
    for _ in 0..8 {
        let s = state(&mut rng);
        let key = Renderer::pipeline_key(&s);
        a.tev.request(key, tev::Uid::of(&s.bp, MODE_DRAW), false);
        let mut words = key.pack().to_vec();
        words.extend_from_slice(&tev::Uid::of(&s.bp, MODE_DRAW).0);
        if !asked.contains(&words) {
            asked.push(words);
        }
    }
    asked.reverse();
    let mut b = Renderer::with_device(a.device.clone(), a.queue().clone());
    b.warm_specialized(&asked);
    assert_eq!(b.specialized_keys(), asked, "in the order asked for");
    let (made, of) = b.finish_specialized_within(std::time::Duration::ZERO);
    assert!(made <= of && of == asked.len());
    b.finish_specialized();
    assert_eq!(b.tev.made_of(), (asked.len(), asked.len()));
}

/// A draw whose uber pipeline isn't made yet never waits for it (on the worker's NVIDIA driver
/// one took 1.4 to 2.8 s): the flush makes the draw's specialized pipeline on the spot (here
/// with the shader threads paused, so it makes it itself) and draws with it, the uber pipeline
/// still on the threads, and the frame is the uber shader's. A clear drawn as a rectangle the
/// same, with a clear's specialized pipeline. Then the threads make the uber pipelines.
#[test]
fn a_draw_whose_uber_pipeline_isnt_made_draws_with_its_specialized_one_made_on_the_spot() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    let mut uber = Renderer::with_device(r.device.clone(), r.queue().clone());
    let seed = 0x6A09_E667_F3BC_C908;
    let (ids, levels) = textures(&mut r, &mut Rng(seed));
    let (uber_ids, _) = textures(&mut uber, &mut Rng(seed));
    r.tev.mode = TevMode::Async;
    r.tev.pause(true);
    let mut rng = Rng(seed ^ 1);
    let s = state(&mut rng);
    let vs = quad(&mut rng);
    let drawn = draw(&mut r, &s, &vs, ids, levels, MODE_DRAW, TevMode::Async);
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
    assert!(
        r.tev.uber(key).is_none(),
        "the uber pipeline was made at the draw"
    );
    let st = r.tev.stats;
    assert_eq!(
        (
            st.on_the_spot,
            st.specialized_calls,
            st.uber_calls,
            st.uber_waited
        ),
        (1, 1, 0, 0),
        "drawn with its specialized pipeline made on the spot"
    );
    let expected = draw(
        &mut uber,
        &s,
        &vs,
        uber_ids,
        levels,
        MODE_DRAW,
        TevMode::Off,
    );
    assert!(
        drawn == expected,
        "drawn otherwise than the uber shader draws it"
    );

    // A clear of part of the EFB, drawn as a rectangle.
    let mut c = State::new();
    c.bp[0x40] = 0x17;
    c.bp[0x41] = 1 << 3 | 1 << 4;
    c.bp[0x43] = 1;
    c.bp[0x4F] = 0x80FF;
    c.bp[0x50] = 0x4012;
    c.bp[0x51] = 0x34_5678;
    let rect = [24, 40, 200, 120];
    let clear_key = PipelineKey {
        depth_compare: Some(7),
        ..Renderer::pipeline_key(&c)
    };
    for x in [&mut r, &mut uber] {
        x.rect_clears = true;
        x.clear(&c, rect);
        x.flush();
    }
    assert!(r.tev.uber(clear_key).is_none());
    assert_eq!(r.tev.stats.on_the_spot, 2, "the clear's made on the spot");
    assert_eq!(r.tev.stats.uber_waited, 0);
    assert!(
        (r.read_efb(), read_depth(&r)) == (uber.read_efb(), read_depth(&uber)),
        "the clear drawn otherwise than the uber shader draws it"
    );

    // The threads make the uber pipelines.
    r.tev.pause(false);
    r.finish_specialized();
    assert!(r.tev.uber(key).is_some() && r.tev.uber(clear_key).is_some());
    assert_eq!(r.tev.stats.uber_waited, 0);
}

/// Warming a list's uber pipelines doesn't make them before the first frame: they're made on
/// the threads, after the specialized ones, and the run's keys hold them all the same.
#[test]
fn a_list_s_uber_pipelines_are_made_on_the_threads() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    r.tev.mode = TevMode::Async;
    r.tev.pause(true);
    let mut rng = Rng(0xBB67_AE85);
    let mut keys = Vec::new();
    for _ in 0..4 {
        let key = PipelineKey {
            early_z: false,
            ..Renderer::pipeline_key(&state(&mut rng))
        };
        keys.push(key.pack());
    }
    keys.sort_unstable();
    keys.dedup();
    r.warm(&keys);
    assert_eq!(r.tev.stats.uber_made, 0, "made before the first frame");
    assert_eq!(r.pipeline_keys(), keys);
    r.tev.pause(false);
    r.finish_specialized();
    assert_eq!(r.tev.stats.uber_made as usize, keys.len());
    assert_eq!(r.tev.stats.uber_waited, 0);
}

/// A list's entries are taken only where this build reads them: one whose key's mark or
/// configuration isn't this build's, or cut short, is left out, and one made for early z is made
/// without it where the renderer's draws can't test depth early (this test's device has no
/// `SHADER_EARLY_DEPTH_TEST`). The shipped list's are made, but kept out of the saved list until
/// a draw asks for one.
#[test]
fn a_list_s_entries_are_taken_where_this_build_reads_them() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    assert!(!r.early_z());
    r.tev.mode = TevMode::Async;
    r.tev.pause(true);
    let entry = |k: PipelineKey, u: &tev::Uid| {
        let mut v = k.pack().to_vec();
        v.extend_from_slice(&u.0);
        v
    };
    let mut rng = Rng(0x3C6E_F372);
    let mut config = || {
        let s = state(&mut rng);
        (Renderer::pipeline_key(&s), tev::Uid::of(&s.bp, MODE_DRAW))
    };
    let (a, ua) = config();
    let (b, ub) = config();
    let (c, uc) = config();
    let early = |k: PipelineKey| PipelineKey { early_z: true, ..k };
    let mut bad_mark = entry(a, &ua);
    bad_mark[7] ^= 1;
    let mut silhouettes = entry(a, &ua);
    *silhouettes.last_mut().unwrap() = 3;
    let mut unmasked = entry(a, &ua);
    // A bit of the configuration's words no draw's has (a stage the stage count leaves out).
    let at = 8
        + (0..ua.0.len())
            .find(|&i| {
                let mut w = ua.0;
                w[i] ^= 1 << 31;
                tev::Uid::from_words(&w).is_none()
            })
            .expect("a bit no configuration has");
    unmasked[at] ^= 1 << 31;
    let mut short = entry(a, &ua);
    short.pop();
    let list = [
        entry(a, &ua),
        entry(early(a), &ua),
        entry(early(b), &ub),
        bad_mark,
        silhouettes,
        unmasked,
        short,
    ];
    assert_eq!(r.warm_specialized(&list), 2);
    assert_eq!(r.specialized_keys(), vec![entry(a, &ua), entry(b, &ub)]);
    assert_eq!(r.warm_shipped(&[entry(c, &uc), entry(a, &ua)]), 1);
    assert_eq!(
        r.specialized_keys().len(),
        2,
        "a shipped one listed before a draw asked"
    );
    r.tev.request(c, uc, true);
    assert_eq!(
        r.specialized_keys(),
        vec![entry(a, &ua), entry(b, &ub), entry(c, &uc)]
    );
    r.tev.pause(false);
    r.finish_specialized();
    assert_eq!(r.tev.made_of(), (3, 3));
}

/// A match's renderer makes what its first frame would otherwise make: the transforms'
/// pipelines on a thread of their own from when the renderer is made
/// (`Renderer::make_ahead`), taken at the first mesh drawn, and the copy to the XFB's with the
/// textures prepared before the match.
#[test]
fn a_match_s_first_frame_finds_its_renderer_s_own_pipelines_made() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    r.make_ahead();
    assert!(r.making.0.is_some(), "the transforms not made ahead");
    r.gpu_meshes();
    assert!(r.making.0.is_none() && r.gpu_meshes.is_some());
    r.xfb_on_gpu = true;
    assert!(r.gpu_xfb.is_none());
    r.prepare_textures(&[]);
    assert!(
        r.gpu_xfb.is_some(),
        "the copy to the XFB's pipelines not made before the match"
    );
}

/// A replay's wait before its first frame waits for the player's own lists, not the shipped
/// one's (1,847 pipelines took the worker's driver 3.6 s with its cache warm): here, with the
/// shader threads paused, the own pipeline made by hand, the wait returns at once with the
/// shipped one still to make, and counts only the player's own.
#[test]
fn a_replay_s_wait_is_for_the_player_s_own_pipelines() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    r.tev.mode = TevMode::Async;
    r.tev.pause(true);
    let mut rng = Rng(0x510E_527F);
    let mut config = || {
        let s = state(&mut rng);
        (Renderer::pipeline_key(&s), tev::Uid::of(&s.bp, MODE_DRAW))
    };
    let (a, ua) = config();
    let (c, uc) = config();
    let entry = |k: PipelineKey, u: &tev::Uid| {
        let mut v = k.pack().to_vec();
        v.extend_from_slice(&u.0);
        v
    };
    r.warm_specialized(&[entry(a, &ua)]);
    r.warm_shipped(&[entry(c, &uc)]);
    r.warm_shipped_pipelines(&[c.pack()]);
    let id = r.tev.request(a, ua, false);
    r.tev.make_now(&[(a, id)]);
    let start = std::time::Instant::now();
    let made = r.finish_specialized_within(std::time::Duration::from_secs(5));
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "waited for the shipped list"
    );
    assert_eq!(made, (1, 1));
    assert!(r.tev.uber(c).is_none());
    r.tev.pause(false);
    r.finish_specialized();
    assert!(r.tev.uber(c).is_some());
}
