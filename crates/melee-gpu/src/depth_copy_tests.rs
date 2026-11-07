// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks that a depth copy (a Z24 EFB to Z24X8) writes each pixel's 24-bit z into memory as
//! encode.rs lays Z24X8 out, alpha 255 and z's high byte, then its middle and low bytes, read
//! from the EFB at the console's size whatever the render scale, width and samples: the depth
//! buffer's encoding of z (`efb_depth`) read back exactly (`copy::DepthReader`). Skips without
//! a GPU adapter, or on a software one.

use melee_gx::{Memory, Sink, State};

use crate::{EFB_HEIGHT, EFB_WIDTH, Renderer};

struct NoMemory;

impl Memory for NoMemory {
    fn read(&self, _phys: u32, out: &mut [u8]) {
        out.fill(0);
    }
}

fn renderer(scale: u32, width: u32, samples: u32) -> Option<Renderer> {
    let adapter = crate::testing::adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::DUAL_SOURCE_BLENDING,
        ..Default::default()
    }))
    .ok()?;
    Some(Renderer::with_device_efb(
        device, queue, scale, width, samples,
    ))
}

/// A clear of depth alone to `z`.
fn clear_state(z: u32) -> State {
    let mut s = State::new();
    s.bp[0x40] = 0x17;
    s.bp[0x51] = z;
    s
}

/// Depths around the encoding's change at 2^23 - 1, its ends, and others.
const ZS: [u32; 8] = [
    0, 0x7F_FFFE, 0x7F_FFFF, 0xFF_FFFF, 0x12_3456, 0x80_0000, 1, 0xAB_CDEF,
];

#[test]
fn depth_copies_write_the_efbs_z() {
    for (scale, width, samples) in [
        (1, EFB_WIDTH, 1),
        (1, EFB_WIDTH, 4),
        (3, 3 * EFB_WIDTH, 1),
        (2, 2 * EFB_WIDTH, 4),
        // 16:9 at 2x: columns of fractional widths.
        (2, 1556, 1),
    ] {
        let Some(mut r) = renderer(scale, width, samples) else {
            eprintln!("skipped: no hardware GPU adapter");
            return;
        };
        // A band of each depth, 80 pixels wide, over the whole EFB.
        for (i, &z) in ZS.iter().enumerate() {
            r.clear(&clear_state(z), [i as u32 * 80, 0, 80, EFB_HEIGHT]);
        }
        // A depth copy of 168 by 20 from (36, 100), across the bands, to Z24X8 (RGBA8's
        // layout: 4 by 4 texel blocks of 64 bytes).
        let (left, top, w, h) = (36u32, 100u32, 168u32, 20u32);
        let blocks = w.div_ceil(4);
        let mut s = State::new();
        s.bp[0x43] = 3;
        s.bp[0x49] = left | top << 10;
        s.bp[0x4A] = (w - 1) | (h - 1) << 10;
        s.bp[0x4B] = 0x1000 >> 5;
        s.bp[0x4D] = (blocks * 64) >> 5;
        let value = (6 * 2) << 3 | 0x52 << 24;
        r.copy(&s, value, &NoMemory);
        let rows = r.take_written();
        assert_eq!(rows.len(), h.div_ceil(4) as usize);
        let label = format!("scale {scale}, width {width}, {samples} samples");
        for (tb, row) in rows.iter().enumerate() {
            assert_eq!(row.address, 0x1000 + tb as u32 * blocks * 64, "{label}");
            for t in 0..4 {
                for s in 0..w {
                    let block = &row.bytes[(s / 4 * 64) as usize..][..64];
                    let i = (t * 4 + s % 4) as usize;
                    let [a, hi] = [block[2 * i], block[2 * i + 1]];
                    let [mid, lo] = [block[32 + 2 * i], block[33 + 2 * i]];
                    let z = u32::from(hi) << 16 | u32::from(mid) << 8 | u32::from(lo);
                    let want = ZS[((left + s) / 80) as usize];
                    assert_eq!(
                        (a, z),
                        (255, want),
                        "{label}: texel ({s}, {})",
                        tb as u32 * 4 + t
                    );
                }
            }
        }
    }
}
