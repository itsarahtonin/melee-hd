//! melee-sim's port of HAL's JPEG encoder (`sysdolphin/baselib/hsd_3B34.c`, which Camera
//! Mode's snapshots compress the frame with) against ssbm-rs's, byte for byte, in both float
//! modes: whole images (noise, gradients, flat colours, the extremes of each channel) into
//! the snapshot's room and into rooms too small for them, and the DCT (`fn_803B376C`) alone on
//! random blocks.

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, SCRATCH, handle, reference};
use melee_sim::hsd::jpeg;
use ssbm_rt::Val;
use ssbm_types::fns;

const IMAGE: u32 = SCRATCH;
const OUT: u32 = SCRATCH + 0x10_0000;
const BLOCK: u32 = SCRATCH + 0x18_0000;
const QUALITY: u32 = 0x804D_6398;

/// A `w` by `h` RGB565 image in GX's 4 by 4 tiles, each pixel from `f(x, y)`.
fn image(w: usize, h: usize, mut f: impl FnMut(usize, usize) -> u16) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 2];
    for ty in 0..h / 4 {
        for tx in 0..w / 4 {
            for y in 0..4 {
                for x in 0..4 {
                    let at = ((ty * (w / 4) + tx) * 16 + y * 4 + x) * 2;
                    let p = f(tx * 4 + x, ty * 4 + y);
                    out[at..at + 2].copy_from_slice(&p.to_be_bytes());
                }
            }
        }
    }
    out
}

/// The images the checks compress: (name, width, height, pixels).
fn images(r: &mut Rand) -> Vec<(String, usize, usize, Vec<u8>)> {
    let mut v = Vec::new();
    for (w, h) in [(16, 16), (64, 32), (640, 480)] {
        let mut n = Rand(r.bits());
        v.push((
            format!("noise {w}x{h}"),
            w,
            h,
            image(w, h, |_, _| n.bits() as u16),
        ));
        v.push((
            format!("gradient {w}x{h}"),
            w,
            h,
            image(w, h, |x, y| {
                (((x * 31 / w) as u16) << 11) | (((y * 63 / h) as u16) << 5) | 7
            }),
        ));
        for p in [0x0000, 0xFFFF, 0xF800, 0x07E0, 0x001F] {
            v.push((
                format!("flat {p:#06x} {w}x{h}"),
                w,
                h,
                image(w, h, |_, _| p),
            ));
        }
        let mut n = Rand(r.bits());
        v.push((
            format!("blocks {w}x{h}"),
            w,
            h,
            image(w, h, |x, y| {
                if (x / 8 + y / 8) % 3 == 0 {
                    n.bits() as u16
                } else {
                    0x7BEF
                }
            }),
        ));
    }
    v
}

#[test]
fn encoder_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_04E6_0001);
    let cases = images(&mut r);
    for mode in MODES {
        set_fp_mode(mode);
        for (name, w, h, pixels) in &cases {
            for (room, quality) in [
                (256_000usize, 3),
                (256_000, 1),
                (256_000, 10),
                (600, 3),
                (5_000, 3),
            ] {
                ctx.write_bytes(IMAGE, pixels);
                ctx.write_bytes(OUT, &vec![0xA5; room.min(0x8_0000) + 0x10]);
                fns::hsd_803B5C2C(&ctx, quality);
                let q = ctx.read_u32(QUALITY) as i32;
                assert_eq!(q, jpeg::quality(quality), "hsd_803B5C2C({quality})");
                let theirs = fns::hsd_803B51C8(
                    &ctx,
                    IMAGE as i32,
                    *w as i32,
                    *h as i32,
                    handle::<Val<'_, i8>>(&ctx, OUT),
                    room as i32,
                );
                let (ours, bytes) = jpeg::encode(pixels, *w as i32, *h as i32, room, q);
                let what = format!("{name}, room {room}, quality {quality}, {mode:?}");
                assert_eq!(ours as i32, theirs, "the encoded size: {what}");
                let mem: Vec<u8> = (0..bytes.len() as u32)
                    .map(|i| ctx.read_u8(OUT + i))
                    .collect();
                if let Some(k) = (0..bytes.len()).find(|&k| bytes[k] != mem[k]) {
                    panic!(
                        "byte {k:#x} of {:#x}: ours {:#04x}, theirs {:#04x}: {what}",
                        bytes.len(),
                        bytes[k],
                        mem[k]
                    );
                }
                // Nothing past what ours wrote.
                assert_eq!(
                    ctx.read_u8(OUT + bytes.len() as u32),
                    0xA5,
                    "a byte past the output: {what}"
                );
            }
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn dct_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_04E6_0002);
    for mode in MODES {
        set_fp_mode(mode);
        for case in 0..3000 {
            let mut block = [0i32; 64];
            for v in &mut block {
                *v = match case % 4 {
                    // The encoder's range: luma and chroma samples.
                    0 => (r.below(256) as i32) - 128,
                    1 => (r.below(2001) as i32) - 1000,
                    2 => r.bits() as i32 >> 8,
                    _ => {
                        if r.below(2) == 0 {
                            127
                        } else {
                            -128
                        }
                    }
                };
            }
            for (i, v) in block.iter().enumerate() {
                ctx.write_u32(BLOCK + 4 * i as u32, *v as u32);
            }
            ctx.call::<_, ()>(0x803B_376C, (handle::<Val<'_, u8>>(&ctx, BLOCK),));
            let mut ours = block;
            jpeg::dct(&mut ours);
            let theirs: Vec<i32> = (0..64)
                .map(|i| ctx.read_u32(BLOCK + 4 * i) as i32)
                .collect();
            assert_eq!(ours.to_vec(), theirs, "case {case}, {mode:?}: {block:?}");
        }
    }
    set_fp_mode(FpMode::Hardware);
}
