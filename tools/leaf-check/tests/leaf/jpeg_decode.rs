//! melee-sim's port of HAL's JPEG decoder (`sysdolphin/baselib/hsd_3B5C.c`, which the album
//! decodes a snapshot read from the card with) against ssbm-rs's, in both float modes: the
//! encoder's streams of whole images (noise, gradients, flat colours, the extremes of each
//! channel) at three qualities, the same cut short and with bytes changed, each decode from
//! the same random statics on both sides, comparing the image, the result and every static
//! the decode leaves; and the inverse DCT (`fn_803B61B4`) alone on random blocks.

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, SCRATCH, handle, reference};
use melee_sim::hsd::{jpeg, jpeg_decode};
use ssbm_rt::{Addr, Val};
use ssbm_types::fns;

const SRC: u32 = SCRATCH;
const DST: u32 = SCRATCH + 0x8_0000;
const BLOCK: u32 = SCRATCH + 0x18_0000;
/// The decoder's quantization tables (`lbl_80431090`), its work area past the `jmp_buf`
/// (`hsd_804D2E70`) and its bit reader.
const QUANT: u32 = 0x8043_1090;
const WORK: u32 = 0x804D_2E70 + 0x118;
const NEXT: u32 = 0x804D_79B8;
const START: u32 = 0x804D_79BC;
const SIZE: u32 = 0x804D_79C0;
const BITS: u32 = 0x804D_79C4;
const BYTE: u32 = 0x804D_79C8;

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

/// The streams the checks decode: (name, stream), the encoder's of each image at each
/// quality, then some cut short and some with bytes changed.
fn streams(r: &mut Rand) -> Vec<(String, Vec<u8>)> {
    let mut images = Vec::new();
    for (w, h) in [(16usize, 16usize), (64, 32), (640, 480)] {
        let mut n = Rand(r.bits());
        images.push((
            format!("noise {w}x{h}"),
            w,
            h,
            image(w, h, |_, _| n.bits() as u16),
        ));
        images.push((
            format!("gradient {w}x{h}"),
            w,
            h,
            image(w, h, |x, y| {
                (((x * 31 / w) as u16) << 11) | (((y * 63 / h) as u16) << 5) | 7
            }),
        ));
        for p in [0x0000, 0xFFFF, 0xF800, 0x07E0, 0x001F] {
            images.push((
                format!("flat {p:#06x} {w}x{h}"),
                w,
                h,
                image(w, h, |_, _| p),
            ));
        }
    }
    let mut v = Vec::new();
    for (name, w, h, pixels) in &images {
        for q in [1, 3, 10] {
            let (n, bytes) = jpeg::encode(pixels, *w as i32, *h as i32, 256_000, q);
            // Noise at high quality doesn't fit a snapshot's room.
            if n == 0 {
                continue;
            }
            v.push((format!("{name} at quality {q}"), bytes));
        }
    }
    let whole: Vec<_> = v.clone();
    for (name, bytes) in whole.iter().filter(|(n, _)| !n.contains("640x480")) {
        for cut in [1usize, 0x30, 0x150, 0x270, bytes.len() / 2, bytes.len() - 3] {
            v.push((
                format!("{name} cut to {cut}"),
                bytes[..cut.min(bytes.len())].to_vec(),
            ));
        }
        for k in 0..4 {
            let mut b = bytes.clone();
            let at = 0x288 + r.below((b.len() - 0x288) as u64) as usize;
            b[at] ^= 1 << (k * 2);
            v.push((format!("{name} with byte {at:#x} changed"), b));
        }
    }
    v
}

/// The same random statics on both sides.
fn statics(ctx: &ssbm_rt::Ctx, r: &mut Rand) -> jpeg_decode::Decoder {
    let mut d = jpeg_decode::Decoder::default();
    for (i, q) in d.quant.iter_mut().enumerate() {
        *q = r.bits() as u8;
        ctx.write_u8(QUANT + i as u32, *q);
    }
    let mut word = |at: u32| {
        let v = r.bits() as i32 >> (r.below(24));
        ctx.write_u32(WORK + at, v as u32);
        v
    };
    for (i, v) in d.luma.iter_mut().enumerate() {
        *v = word(4 * i as u32);
    }
    for (i, v) in d.cb.iter_mut().enumerate() {
        *v = word(0x400 + 4 * i as u32);
    }
    for (i, v) in d.cr.iter_mut().enumerate() {
        *v = word(0x500 + 4 * i as u32);
    }
    for (i, v) in d.coef.iter_mut().enumerate() {
        *v = word(0x600 + 4 * i as u32);
    }
    for (i, v) in d.prev_dc.iter_mut().enumerate() {
        *v = word(0x700 + 4 * i as u32);
    }
    d.bits_left = r.below(8) as i32;
    d.byte = r.bits() as u8;
    ctx.write_u32(BITS, d.bits_left as u32);
    ctx.write_u8(BYTE, d.byte);
    d
}

#[test]
fn decoder_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_04E6_0D01);
    let cases = streams(&mut r);
    let image_size = 640 * 480 * 2;
    let mut skipped = 0;
    for mode in MODES {
        set_fp_mode(mode);
        for (name, stream) in &cases {
            let what = format!("{name}, {mode:?}");
            let mut d = statics(&ctx, &mut r);
            let fill = r.bits() as u8;
            let mut src = stream.clone();
            // Past the stream, what the buffer holds, which the markers' readers may look at.
            src.extend((0..0x100).map(|_| r.bits() as u8));
            ctx.write_bytes(SRC, &src);
            let mut ours_img = vec![fill; image_size];
            ctx.write_bytes(DST, &ours_img);
            let size = stream.len() as i32;
            let ours = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                jpeg_decode::decode(&mut d, |i| src[i as usize], size, &mut ours_img)
            }));
            let Ok(ours) = ours else {
                // A broken stream the port stops on as unported: a run of zeros past the
                // block's end writes past the decoder's work area. No snapshot the game makes
                // has one (the card's checksums keep its bytes as the encoder wrote them).
                assert!(name.contains("changed"), "an unported stop on {what}");
                skipped += 1;
                continue;
            };
            let theirs = fns::hsd_803B6BE4(
                &ctx,
                handle::<Val<'_, i8>>(&ctx, SRC),
                size,
                handle::<Addr>(&ctx, DST),
            );
            assert_eq!(ours, theirs, "the result: {what}");
            let mem: Vec<u8> = (0..image_size as u32)
                .map(|i| ctx.read_u8(DST + i))
                .collect();
            if let Some(k) = (0..image_size).find(|&k| ours_img[k] != mem[k]) {
                panic!(
                    "the image's byte {k:#x}: ours {:#04x}, theirs {:#04x}: {what}",
                    ours_img[k], mem[k]
                );
            }
            let quant: Vec<u8> = (0..0x80).map(|i| ctx.read_u8(QUANT + i)).collect();
            assert_eq!(d.quant, quant, "the quantization tables: {what}");
            let words = |at: u32, n: u32| -> Vec<i32> {
                (0..n)
                    .map(|i| ctx.read_u32(WORK + at + 4 * i) as i32)
                    .collect()
            };
            assert_eq!(d.luma, words(0, 0x100), "the luma blocks: {what}");
            assert_eq!(d.cb, words(0x400, 0x40), "the Cb block: {what}");
            assert_eq!(d.cr, words(0x500, 0x40), "the Cr block: {what}");
            assert_eq!(d.coef, words(0x600, 0x40), "the coefficients: {what}");
            assert_eq!(d.prev_dc.to_vec(), words(0x700, 3), "the last DCs: {what}");
            assert_eq!(
                (SRC + d.next, SRC, d.size, d.bits_left, d.byte),
                (
                    ctx.read_u32(NEXT),
                    ctx.read_u32(START),
                    ctx.read_u32(SIZE) as i32,
                    ctx.read_u32(BITS) as i32,
                    ctx.read_u8(BYTE),
                ),
                "the bit reader: {what}"
            );
        }
    }
    // Most broken streams end before their runs reach past a block.
    assert!(
        skipped < cases.len(),
        "{skipped} streams stopped as unported"
    );
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn idct_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_04E6_0D02);
    for mode in MODES {
        set_fp_mode(mode);
        for case in 0..3000 {
            let mut block = [0i32; 64];
            for v in &mut block {
                *v = match case % 4 {
                    // Dequantized coefficients: a block's range at qualities 1 to 10.
                    0 => (r.below(4001) as i32) - 2000,
                    1 => (r.below(65_001) as i32) - 32_500,
                    2 => r.bits() as i32 >> 8,
                    _ => {
                        if r.below(2) == 0 {
                            1016
                        } else {
                            -1024
                        }
                    }
                };
            }
            for (i, v) in block.iter().enumerate() {
                ctx.write_u32(BLOCK + 4 * i as u32, *v as u32);
            }
            ctx.call::<_, ()>(0x803B_61B4, (handle::<Val<'_, i32>>(&ctx, BLOCK),));
            let mut ours = block;
            jpeg_decode::idct(&mut ours);
            let theirs: Vec<i32> = (0..64)
                .map(|i| ctx.read_u32(BLOCK + 4 * i) as i32)
                .collect();
            assert_eq!(ours.to_vec(), theirs, "case {case}, {mode:?}: {block:?}");
        }
    }
    set_fp_mode(FpMode::Hardware);
}
