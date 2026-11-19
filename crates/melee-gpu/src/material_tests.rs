// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks the fast renderer's materials as records (material.rs) on the GPU: random
//! configurations (tev_tests' states) with random values in every register the pixels don't
//! read, drawn with the uniforms' registers cut to [`material::READ`] and with all of them, and
//! drawn from records and from the registers (`push_draw`), must leave the EFB's color and
//! depth bit for bit the same. Skips without a GPU adapter, or on a software one.

use melee_gx::{Memory, State, xform};

use crate::tev_tests::{Rng, quad, read_depth, renderer, state};
use crate::{
    EFB_HEIGHT, EFB_WIDTH, Geometry, GpuVertex, PendingClear, Renderer, TevMode, material,
};

/// Texture bytes for every map: the states' images all start at address 0.
struct Bytes(Vec<u8>);

impl Memory for Bytes {
    fn read(&self, phys: u32, out: &mut [u8]) {
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.0[(phys as usize + i) % self.0.len()];
        }
    }

    fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
        self.0.get(phys as usize..phys as usize + len)
    }
}

/// Whether register `r` is one a draw's pixels read.
fn read(r: usize) -> bool {
    material::READ.iter().any(|range| range.contains(&r))
}

/// The registers that name a map's texture in memory (`SETIMAGE3` and `SETTLUT`), which the
/// states leave at 0.
fn names_texture(r: usize) -> bool {
    matches!(r, 0x94..=0x9B | 0xB4..=0xBB)
}

/// A random configuration with random values in the registers the pixels don't read, and a
/// viewport: a z range and far z that let the depth test run early or not, and a width fog's
/// range adjustment reads.
fn material_state(rng: &mut Rng) -> State {
    let mut s = state(rng);
    for r in 0..256 {
        if !read(r) && !names_texture(r) {
            s.bp[r] = rng.next() & 0xFF_FFFF;
        }
    }
    let vp = xform::VIEWPORT;
    let depth = if rng.below(4) == 0 {
        8_388_607.0f32
    } else {
        16_777_215.0
    };
    s.xf[vp] = (240.0 + rng.below(160) as f32).to_bits();
    s.xf[vp + 1] = (-240.0f32).to_bits();
    s.xf[vp + 2] = depth.to_bits();
    s.xf[vp + 3] = 662.0f32.to_bits();
    s.xf[vp + 4] = 582.0f32.to_bits();
    s.xf[vp + 5] = depth.to_bits();
    s
}

/// Draws each quad with its state on a cleared EFB, from records or not, and reads back its
/// color and depth.
fn draw(
    r: &mut Renderer,
    draws: &[(State, [GpuVertex; 6])],
    mem: &Bytes,
    records: bool,
    tev: TevMode,
) -> (Vec<u8>, Vec<u8>) {
    r.tev.mode = tev;
    r.material_records = records;
    r.pending_clear = PendingClear {
        color: Some([10, 200, 30, 140]),
        depth: Some(0x80_0000),
    };
    for (s, vs) in draws {
        let tg = (s.bp[0x00] & 0xF).min(8);
        let first = r.indices.len() as u32;
        r.push_in_order(vs, tg as usize);
        let g = Geometry {
            first,
            count: vs.len() as u32,
            vertices: Default::default(),
        };
        r.push_state(s, tg, [0, 0, EFB_WIDTH, EFB_HEIGHT], &[g], mem);
    }
    r.flush();
    (r.read_efb(), read_depth(r))
}

fn same(a: &(Vec<u8>, Vec<u8>), b: &(Vec<u8>, Vec<u8>), what: &str) {
    if let Some(at) = (0..a.0.len()).find(|&j| a.0[j] != b.0[j]) {
        let p = at / 4;
        panic!(
            "{what}: color differs at ({}, {}): {:?} against {:?}",
            p % EFB_WIDTH as usize,
            p / EFB_WIDTH as usize,
            &a.0[p * 4..p * 4 + 4],
            &b.0[p * 4..p * 4 + 4]
        );
    }
    if let Some(at) = (0..a.1.len()).find(|&j| a.1[j] != b.1[j]) {
        let p = at / 4;
        panic!(
            "{what}: depth differs at ({}, {})",
            p % EFB_WIDTH as usize,
            p / EFB_WIDTH as usize
        );
    }
}

fn memory(rng: &mut Rng) -> Bytes {
    Bytes((0..1 << 16).map(|_| rng.next() as u8).collect())
}

/// The fragment shader and the pipelines read no register outside [`material::READ`]: a state
/// drawn as it is and with every other register zero leaves the same EFB.
#[test]
fn the_pixels_read_only_the_material_s_registers() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mem = memory(&mut rng);
    let mut drawn = 0;
    for i in 0..64 {
        let s = material_state(&mut rng);
        let mut cut = s.clone_state();
        cut.bp = material::read_registers(&s.bp);
        for reg in (0..256).filter(|&reg| names_texture(reg)) {
            cut.bp[reg] = s.bp[reg];
        }
        let vs = quad(&mut rng);
        let whole = draw(&mut r, &[(s, vs)], &mem, false, TevMode::Off);
        let read = draw(&mut r, &[(cut, vs)], &mem, false, TevMode::Off);
        same(&whole, &read, &format!("config {i}"));
        if whole.0.chunks(4).any(|c| c != [10, 200, 30, 140]) {
            drawn += 1;
        }
    }
    assert!(drawn > 16, "only {drawn} of 64 drew");
}

/// Draws from records leave what draws from their registers leave: each config drawn twice in
/// a flush (the second time with other colors, konstants, texture sizes and viewport width:
/// the same record with other data) and once more in the next, with the uber shader and with
/// the specialized ones.
#[test]
fn draws_from_records_draw_as_from_their_registers() {
    let Some(mut r) = renderer() else {
        eprintln!("skipped: no hardware GPU adapter");
        return;
    };
    let mut rng = Rng(0x9E6C_63D0_676A_9A99);
    let mem = memory(&mut rng);
    let mut drawn = 0;
    for i in 0..40 {
        let s = material_state(&mut rng);
        let mut other = s.clone_state();
        for c in 0..4 {
            other.color_ra[c] = rng.next() & 0x3F_FFFF;
            other.konst_bg[c] = rng.next() & 0x3F_FFFF;
        }
        for map in 0..8 {
            let at = 0x88 + (map & 3) + if map >= 4 { 0x20 } else { 0 };
            let (w, h) = (1 + rng.below(64), 1 + rng.below(64));
            other.bp[at] = other.bp[at] & !0xF_FFFF | (w - 1) | (h - 1) << 10;
        }
        other.xf[xform::VIEWPORT] = (240.0 + rng.below(160) as f32).to_bits();
        for reg in (0..256).filter(|&reg| !read(reg) && !names_texture(reg)) {
            other.bp[reg] = rng.next() & 0xFF_FFFF;
        }
        let draws = [(s.clone_state(), quad(&mut rng)), (other, quad(&mut rng))];
        let tev = if i % 5 == 0 {
            TevMode::Sync
        } else {
            TevMode::Off
        };
        let from_registers = draw(&mut r, &draws, &mem, false, tev);
        let from_records = draw(&mut r, &draws, &mem, true, tev);
        same(&from_registers, &from_records, &format!("config {i}"));
        let again = draw(&mut r, &draws, &mem, true, tev);
        same(&from_registers, &again, &format!("config {i}, again"));
        if from_registers.0.chunks(4).any(|c| c != [10, 200, 30, 140]) {
            drawn += 1;
        }
    }
    assert!(drawn > 10, "only {drawn} of 40 drew");
}

trait CloneState {
    fn clone_state(&self) -> State;
}

impl CloneState for State {
    fn clone_state(&self) -> State {
        let mut s = State::new();
        s.bp = self.bp;
        s.cp = self.cp;
        s.xf.clone_from(&self.xf);
        s.tmem.clone_from(&self.tmem);
        (s.color_ra, s.color_bg, s.konst_ra, s.konst_bg) =
            (self.color_ra, self.color_bg, self.konst_ra, self.konst_bg);
        s
    }
}
