// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks copies from the EFB at the render's scale (`copy::Copied::scaled`): at a scale above
//! 1 every copy kept on the GPU gets a texture that many times its size each way, which draws
//! sample, beside the one at the game's size, which reads back to memory; at scale 1 there is
//! none. Where the EFB is a whole number of pixels a console pixel (640 times the scale wide)
//! and its content is constant over each block of console pixels a texel of the game's copy
//! covers (as a frame drawn at 1x and scaled up is), each texel of the scaled copy is exactly
//! the game's texel it lies in, format quantization and all. At 73:60 and 16:9 a console
//! column's pixels aren't whole, so a scaled texel can take a pixel the game's block gives
//! the next column; there, and on smooth content (gradients across the EFB) at every size, the
//! scaled copy averaged back over each game texel comes within the format's quantization of
//! the game's copy. Skips without a GPU adapter, or on a software one.

use melee_gx::{Memory, Sink, State};

use crate::{EFB_HEIGHT, EFB_WIDTH, Renderer};

struct NoMemory;

impl Memory for NoMemory {
    fn read(&self, _phys: u32, out: &mut [u8]) {
        out.fill(0);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
}

/// The copy's state and BP 0x52 value: `format` (I4, RGB565 or RGB5A3), halved or not, of the
/// `w` by `h` rectangle at (`left`, `top`) into `addr`, from an RGBA6 EFB when `six`.
fn copy(format: u32, half: u32, six: bool, rect: [u32; 4], addr: u32) -> (State, u32) {
    let [left, top, w, h] = rect;
    let mut s = State::new();
    s.bp[0x43] = u32::from(six);
    s.bp[0x49] = left | top << 10;
    s.bp[0x4A] = (w - 1) | (h - 1) << 10;
    s.bp[0x4B] = addr >> 5;
    s.bp[0x4D] = 32;
    let tp = format * 2;
    (s, tp << 3 | half << 9 | 0x52 << 24)
}

/// The console column whose block of the EFB's `width` scaled pixels holds pixel `x`, as the
/// copy's blocks lie (from one column's start to the next's).
fn column(x: u32, width: u32) -> u32 {
    let mut c = (x * EFB_WIDTH / width).min(EFB_WIDTH - 1);
    while c + 1 < EFB_WIDTH && (c + 1) * width / EFB_WIDTH <= x {
        c += 1;
    }
    while c > 0 && c * width / EFB_WIDTH > x {
        c -= 1;
    }
    c
}

/// Fills the renderer's EFB: with `block` > 0, each block of `block` by `block` console pixels
/// one random color; with 0, gradients across it, each channel its own.
fn fill(r: &Renderer, rng: &mut Rng, block: u32) {
    let (w, h) = (r.width, EFB_HEIGHT * r.scale);
    let colors: Vec<u32> = (0..EFB_WIDTH * EFB_HEIGHT).map(|_| rng.next()).collect();
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = match (
                column(x, w).checked_div(block),
                (y / r.scale).checked_div(block),
            ) {
                (Some(cx), Some(cy)) => colors[(cy * EFB_WIDTH + cx) as usize],
                _ => {
                    let (fx, fy) = (x * 255 / w, y * 255 / h);
                    u32::from_le_bytes([
                        fx as u8,
                        fy as u8,
                        ((fx + fy) / 2) as u8,
                        (255 - fx) as u8,
                    ])
                }
            };
            rgba.extend_from_slice(&v.to_le_bytes());
        }
    }
    r.queue.write_texture(
        r.efb.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}

/// A copy's two textures, read back: the game's (RGBA, width, height) and the scaled one's.
type Read = ((Vec<u8>, u32, u32), Option<(Vec<u8>, u32, u32)>);

fn read(r: &Renderer, addr: u32) -> Read {
    let c = r.gpu_copies.get(&addr).expect("the copy kept on the GPU");
    let tex = |id: u32| {
        &r.textures[id as usize]
            .as_ref()
            .expect("its texture")
            ._texture
    };
    let native = (
        r.read_texture(tex(c.id), c.width, c.height),
        c.width,
        c.height,
    );
    let scaled = c.scaled.map(|(id, k)| {
        let size = tex(id).size();
        assert_eq!(k, r.scale);
        (
            r.read_texture(tex(id), size.width, size.height),
            size.width,
            size.height,
        )
    });
    (native, scaled)
}

/// Renderers at the console's 640 columns, at 73:60 and at 16:9, scaled by 1 to 3.
const VIEWS: [(u32, u32); 6] = [
    (1, 640),
    (1, 584),
    (2, 1280),
    (2, 1708),
    (3, 1752),
    (3, 2560),
];

#[test]
fn copies_follow_the_render_scale() {
    let Some(mut r) = crate::testing::renderer() else {
        return;
    };
    let mut rng = Rng(0x005C_A1ED_00C0_B1E5);
    let mut cases = 0;
    let mut worst = [0u32; 6];
    for (scale, width) in VIEWS {
        r.resize_efb(scale, width);
        for format in [0, 4, 5] {
            for half in 0..2 {
                for (n, &(w, h)) in [(80u32, 60u32), (64, 64), (250, 160), (37, 21), (640, 406)]
                    .iter()
                    .enumerate()
                {
                    let (w, h) = (w << half, h << half);
                    if w > EFB_WIDTH || h > EFB_HEIGHT {
                        continue;
                    }
                    // Halving copies start on even pixels, so their boxes are the blocks.
                    let left = (rng.next() % (EFB_WIDTH - w + 1)) >> half << half;
                    let top = (rng.next() % (EFB_HEIGHT - h + 1)) >> half << half;
                    let six = n % 2 == 1;
                    let addr = 0x0040_0000 + 0x0010_0000 * n as u32;
                    let label = format!(
                        "{scale}x {width} wide, format {format} half {half} six {six}, \
                         {w}x{h} at ({left}, {top})"
                    );
                    let (state, value) = copy(format, half, six, [left, top, w, h], addr);
                    let whole = width == EFB_WIDTH * scale;
                    for block in [1 << half, 0] {
                        fill(&r, &mut rng, block);
                        r.copy(&state, value, &NoMemory);
                        let ((nat, nw, nh), scaled) = read(&r, addr);
                        assert_eq!((nw, nh), (w >> half, h >> half), "{label}");
                        let Some((sc, sw, sh)) = scaled else {
                            assert_eq!(scale, 1, "{label}: no copy at the render's scale");
                            continue;
                        };
                        assert!(scale > 1, "{label}: a scaled copy at 1x");
                        assert_eq!((sw, sh), (nw * scale, nh * scale), "{label}");
                        let k = scale;
                        let at = |buf: &[u8], bw: u32, x: u32, y: u32, ch: usize| {
                            u32::from(buf[((y * bw + x) * 4) as usize + ch])
                        };
                        for y in 0..nh {
                            for x in 0..nw {
                                for ch in 0..4 {
                                    let want = at(&nat, nw, x, y, ch);
                                    if block != 0 && whole {
                                        for j in 0..k * k {
                                            let got = at(&sc, sw, x * k + j % k, y * k + j / k, ch);
                                            assert_eq!(
                                                got, want,
                                                "{label}: blocks of {block}: texel ({x}, {y}) \
                                                 channel {ch}, part {j}"
                                            );
                                        }
                                    } else if block == 0 {
                                        let sum: u32 = (0..k * k)
                                            .map(|j| at(&sc, sw, x * k + j % k, y * k + j / k, ch))
                                            .sum();
                                        let mean = (sum + k * k / 2) / (k * k);
                                        let d = mean.abs_diff(want);
                                        let slot =
                                            [0, 4, 5].iter().position(|&f| f == format).unwrap()
                                                * 2
                                                + half as usize;
                                        worst[slot] = worst[slot].max(d);
                                    }
                                }
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    eprintln!(
        "{cases} copies; gradients' scaled copy averaged against the game's, worst \
         channel difference by format and half: {worst:?}"
    );
    // A channel's quantization step: I4's 17, RGB565's and RGB5A3's 8 (4 bits' 17 and 3
    // bits' 36 for RGB5A3's translucent texels, which the alpha gradient makes), and the copies'
    // own rounding on top.
    for (slot, &d) in worst.iter().enumerate() {
        let format = [0, 4, 5][slot / 2];
        let step = match format {
            0 => 17,
            4 => 8,
            _ => 36,
        };
        assert!(
            d <= step,
            "format {format}: averaged back {d} levels from the game's copy"
        );
    }
}
