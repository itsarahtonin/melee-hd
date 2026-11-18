//! The harness's image comparison, by CIEDE2000 (ΔE₀₀), the perceptual difference between two
//! colors (about 2.3 is just noticeable).
//!
//! A renderer that draws the same scene with other vertex float maths, without the
//! rasterizer's snap to sixteenths or with other depth precision moves triangles' edges by
//! fractions of a pixel. Pixel by pixel that changes a lot: edge pixels take the color of the
//! triangle on their other side, a thin line (Final Destination's edges, a grid) moves a whole
//! run of them, textures minified at a grazing angle sample other texels, and a copy magnified
//! onto a surface magnifies its own flips (Fountain of Dreams' reflection: 80 by 60 texels over
//! the water). A wrong frame changes what a region looks like: a material's color off, a draw
//! missing, moved by pixels or blended in another order. So the measure that decides is each
//! block's mean color (blocks of 8 by 8 of the console's pixels, averaged in linear light, as
//! the eye does from far enough away), against the other frame's block there or a pixel off
//! ([`block_differences`]).
//!
//! How much a block may differ depends on the block: a flat wall doesn't change under a
//! sub-pixel shift, the reflection or a grid seen edge-on changes a lot. So each frame is also
//! drawn by the exact renderer with every vertex shifted by 1/32 of a pixel diagonally, twice
//! (`exact::Mutation::Spread`), and a block may differ by [`BLOCK_LIMIT`] plus [`SPREAD`] times
//! the most either shifted frame's block differs from the reference's: its own sensitivity to
//! a sub-pixel shift. Scattered differences are held to a share of the pixels far from every
//! pixel around them in the other frame ([`shift_tolerant`]). How far vertices may move is the
//! geometry's to check, to an eighth of a pixel. The plain ΔE₀₀, pixel against pixel, is
//! reported beside them. docs/fast-renderer.md has the measurements behind the limits.

/// The side of a block, in the console's pixels.
pub const BLOCK: u32 = 8;
/// How far a block's mean colors may differ (ΔE₀₀) where a sub-pixel shift doesn't change it.
pub const BLOCK_LIMIT: f64 = 3.0;
/// How many times the block's own spread under a sub-pixel shift is allowed on top.
pub const SPREAD: f64 = 2.0;
/// ΔE₀₀ above which two pixels are far apart: clearly a different color, not a shade off.
pub const FAR: f64 = 10.0;
/// The largest share of the pixels allowed to be [`FAR`] from every pixel around them in the
/// other frame, in percent.
pub const FAR_LIMIT: f64 = 1.0;
/// A just noticeable difference (ΔE₀₀ about 2.3), for the report.
pub const JND: f64 = 2.3;

/// What the comparison of two frames found.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageReport {
    pub width: u32,
    pub height: u32,
    /// Whether every pixel is the same.
    pub identical: bool,
    /// The plain ΔE₀₀, pixel against pixel: its mean, its largest, and the share of the pixels
    /// more than [`JND`] apart, in percent.
    pub mean: f64,
    pub max: f64,
    pub over_jnd: f64,
    /// The shift-tolerant difference ([`shift_tolerant`]): the share of the pixels more than
    /// [`FAR`] apart, in percent.
    pub over_far: f64,
    /// The block furthest over (or nearest to) what it's allowed ([`Block::excess`]) and its
    /// top left corner in the console's pixels; and how many blocks are over.
    pub block: Block,
    pub block_at: (u32, u32),
    pub blocks_over: usize,
}

impl ImageReport {
    pub fn within(&self) -> bool {
        self.blocks_over == 0 && self.over_far <= FAR_LIMIT
    }

    /// The report for two frames of other sizes.
    pub fn sizes_differ(width: u32, height: u32) -> ImageReport {
        ImageReport {
            width,
            height,
            identical: false,
            mean: f64::INFINITY,
            max: f64::INFINITY,
            over_jnd: 100.0,
            over_far: 100.0,
            block: Block {
                mean: f64::INFINITY,
                pixels: f64::INFINITY,
                ..Block::default()
            },
            block_at: (0, 0),
            blocks_over: usize::MAX,
        }
    }

    pub fn line(&self) -> String {
        if self.identical {
            return format!("image {}x{}: identical", self.width, self.height);
        }
        format!(
            "image {}x{}: ΔE00 mean {:.3}, max {:.1}, {:.3}% over {JND}; {:.3}% far (limit {FAR_LIMIT}%); {} {BLOCK}x{BLOCK} blocks over (limit {BLOCK_LIMIT} + {SPREAD} x the spread), the worst at ({}, {}): mean color {:.2} apart (spread {:.2}), pixels {:.2} (spread {:.2}): {}",
            self.width,
            self.height,
            self.mean,
            self.max,
            self.over_jnd,
            self.over_far,
            self.blocks_over,
            self.block_at.0,
            self.block_at.1,
            self.block.mean,
            self.block.mean_spread,
            self.block.pixels,
            self.block.pixels_spread,
            if self.within() {
                "within tolerance"
            } else {
                "OUT OF TOLERANCE"
            }
        )
    }
}

/// sRGB levels to linear light.
fn linear_table() -> [f64; 256] {
    std::array::from_fn(|i| {
        let c = i as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// CIELAB (D65) of an sRGB color, its levels made linear by `lin`.
fn lab(lin: &[f64; 256], p: [u8; 3]) -> [f64; 3] {
    lab_linear(p.map(|c| lin[usize::from(c)]))
}

/// CIELAB (D65) of a linear sRGB color.
fn lab_linear([r, g, b]: [f64; 3]) -> [f64; 3] {
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b;
    let z = (0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83;
    let f = |t: f64| {
        const D: f64 = 6.0 / 29.0;
        if t > D * D * D {
            t.cbrt()
        } else {
            t / (3.0 * D * D) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 between two CIELAB colors (Sharma, Wu and Dalal, 2005), with kL = kC = kH = 1.
pub fn ciede2000(p: [f64; 3], q: [f64; 3]) -> f64 {
    let [l1, a1, b1] = p;
    let [l2, a2, b2] = q;
    let pow7 = |c: f64| c.powi(7);
    let k25 = pow7(25.0);
    let cbar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (pow7(cbar) / (pow7(cbar) + k25)).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |a: f64, b: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1p, h2p) = (hue(a1p, b1), hue(a2p, b2));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let product = c1p * c2p;
    let dh = if product == 0.0 {
        0.0
    } else {
        let d = h2p - h1p;
        if d.abs() <= 180.0 {
            d
        } else if d > 180.0 {
            d - 360.0
        } else {
            d + 360.0
        }
    };
    let dhh = 2.0 * product.sqrt() * (dh / 2.0).to_radians().sin();
    let lbar = (l1 + l2) / 2.0;
    let cbarp = (c1p + c2p) / 2.0;
    let hbar = if product == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let cos = |deg: f64| deg.to_radians().cos();
    let t = 1.0 - 0.17 * cos(hbar - 30.0) + 0.24 * cos(2.0 * hbar) + 0.32 * cos(3.0 * hbar + 6.0)
        - 0.20 * cos(4.0 * hbar - 63.0);
    let dtheta = 30.0 * (-((hbar - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (pow7(cbarp) / (pow7(cbarp) + k25)).sqrt();
    let l50 = (lbar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * l50 / (20.0 + l50).sqrt();
    let sc = 1.0 + 0.045 * cbarp;
    let sh = 1.0 + 0.015 * cbarp * t;
    let rt = -(2.0 * dtheta).to_radians().sin() * rc;
    let (x, y, z) = (dl / sl, dc / sc, dhh / sh);
    (x * x + y * y + z * z + rt * y * z).max(0.0).sqrt()
}

/// Each pixel's ΔE₀₀ between two RGB images of the same size (0 where they're the same).
pub fn delta_e(a: &[u8], b: &[u8]) -> Vec<f32> {
    let lin = linear_table();
    a.as_chunks::<3>()
        .0
        .iter()
        .zip(b.as_chunks::<3>().0)
        .map(|(p, q)| {
            if p == q {
                0.0
            } else {
                ciede2000(lab(&lin, *p), lab(&lin, *q)) as f32
            }
        })
        .collect()
}

/// Each pixel's shift-tolerant difference between two RGB images `w` wide: the smallest ΔE₀₀
/// from `a`'s pixel to `b`'s pixel there and its eight neighbors, and from `b`'s to `a`'s, the
/// larger of the two. `de` is the plain ΔE₀₀ ([`delta_e`]): a pixel where it's 0 is 0.
pub fn shift_tolerant(a: &[u8], b: &[u8], w: u32, de: &[f32]) -> Vec<f32> {
    let lin = linear_table();
    let (pa, pb) = (a.as_chunks::<3>().0, b.as_chunks::<3>().0);
    let h = (pa.len() / w as usize) as i64;
    let w = i64::from(w);
    let nearest = |from: &[u8; 3], other: &[[u8; 3]], x: i64, y: i64| -> f64 {
        let l = lab(&lin, *from);
        let mut best = f64::INFINITY;
        for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
            for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                let q = &other[(ny * w + nx) as usize];
                if q == from {
                    return 0.0;
                }
                best = best.min(ciede2000(l, lab(&lin, *q)));
            }
        }
        best
    };
    de.iter()
        .enumerate()
        .map(|(i, &d)| {
            if d == 0.0 {
                return 0.0;
            }
            let (x, y) = (i as i64 % w, i as i64 / w);
            nearest(&pa[i], pb, x, y).max(nearest(&pb[i], pa, x, y)) as f32
        })
        .collect()
}

/// An image's linear light summed over every rectangle from its top left (a summed-area
/// table), to average blocks anywhere.
struct Sums {
    w: usize,
    s: Vec<[f64; 3]>,
}

impl Sums {
    fn new(rgb: &[u8], w: u32, lin: &[f64; 256]) -> Sums {
        let w = w as usize;
        let h = rgb.len() / 3 / w;
        let mut s = vec![[0f64; 3]; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = [0f64; 3];
            for x in 0..w {
                let p = &rgb[(y * w + x) * 3..][..3];
                for c in 0..3 {
                    row[c] += lin[usize::from(p[c])];
                    s[(y + 1) * (w + 1) + x + 1][c] = s[y * (w + 1) + x + 1][c] + row[c];
                }
            }
        }
        Sums { w, s }
    }

    /// The mean linear color of the `side` square from (`x`, `y`).
    fn mean(&self, x: usize, y: usize, side: usize) -> [f64; 3] {
        let at = |x: usize, y: usize| self.s[y * (self.w + 1) + x];
        let (a, b, c, d) = (
            at(x, y),
            at(x + side, y),
            at(x, y + side),
            at(x + side, y + side),
        );
        let n = (side * side) as f64;
        std::array::from_fn(|k| (d[k] - b[k] - c[k] + a[k]) / n)
    }
}

/// Each block's difference: the ΔE₀₀ between the two frames' mean colors over it (in linear
/// light, as the eye averages them from far enough away), the smallest with the other frame's
/// block where it is or a pixel off in any direction. Sub-pixel shifts move color within a
/// block, or a line across a block's edge, and leave this small; a region drawn in another
/// color, a draw missing or moved by pixels don't. Blocks of [`BLOCK`] of the console's pixels,
/// row by row.
pub fn block_differences(a: &[u8], b: &[u8], w: u32, h: u32, scale: u32) -> Vec<f32> {
    let lin = linear_table();
    let (sa, sb) = (Sums::new(a, w, &lin), Sums::new(b, w, &lin));
    let side = (BLOCK * scale.max(1)) as usize;
    let (w, h) = (w as usize, h as usize);
    let mut out = Vec::with_capacity((w / side) * (h / side));
    for by in (0..h / side).map(|y| y * side) {
        for bx in (0..w / side).map(|x| x * side) {
            let ma = sa.mean(bx, by, side);
            let la = lab_linear(ma);
            let mut best = f64::INFINITY;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let x = (bx as i64 + dx).clamp(0, (w - side) as i64) as usize;
                    let y = (by as i64 + dy).clamp(0, (h - side) as i64) as usize;
                    let mb = sb.mean(x, y, side);
                    best = best.min(if ma == mb {
                        0.0
                    } else {
                        ciede2000(la, lab_linear(mb))
                    });
                }
            }
            out.push(best as f32);
        }
    }
    out
}

/// The mean of `v` (an image `w` by `h` at `scale`) over each block of [`BLOCK`] of the
/// console's pixels, row by row.
fn block_means(v: &[f32], w: u32, h: u32, scale: u32) -> Vec<f32> {
    let side = BLOCK * scale.max(1);
    let mut out = Vec::with_capacity(((w / side) * (h / side)) as usize);
    for by in (0..h / side).map(|y| y * side) {
        for bx in (0..w / side).map(|x| x * side) {
            let mut sum = 0f64;
            for y in by..by + side {
                let row = (y * w + bx) as usize;
                sum += v[row..row + side as usize]
                    .iter()
                    .map(|&d| f64::from(d))
                    .sum::<f64>();
            }
            out.push((sum / f64::from(side * side)) as f32);
        }
    }
    out
}

/// A block's two differences between the frames, each with its spread: how much it differs
/// between the reference and the reference shifted by sub-pixel amounts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Block {
    /// Its mean colors' difference ([`block_differences`]).
    pub mean: f64,
    pub mean_spread: f64,
    /// Its pixels' mean shift-tolerant difference ([`shift_tolerant`]).
    pub pixels: f64,
    pub pixels_spread: f64,
}

impl Block {
    /// How far over what it's allowed the block is (over when positive): within if either
    /// difference is within [`BLOCK_LIMIT`] plus [`SPREAD`] times its spread. A sub-pixel shift
    /// that flips a run of pixels along an edge changes the block's mean but leaves each pixel
    /// near a pixel of its color; one that makes a minified texture sample other texels or a
    /// magnified copy show other texels leaves pixels far from their neighbors, but the shifted
    /// references show the same. A wrong color does neither.
    pub fn excess(&self) -> f64 {
        let over = |d: f64, s: f64| d - (BLOCK_LIMIT + SPREAD * s);
        over(self.mean, self.mean_spread).min(over(self.pixels, self.pixels_spread))
    }
}

/// Each block's differences between `a` and `b`, with their spreads: the most they differ
/// between `a` and each of `spreads`.
pub fn blocks(a: &[u8], b: &[u8], spreads: &[&[u8]], w: u32, h: u32, scale: u32) -> Vec<Block> {
    let pixels = |x: &[u8]| block_means(&shift_tolerant(a, x, w, &delta_e(a, x)), w, h, scale);
    let mut out: Vec<Block> = block_differences(a, b, w, h, scale)
        .into_iter()
        .zip(pixels(b))
        .map(|(m, p)| Block {
            mean: f64::from(m),
            pixels: f64::from(p),
            ..Block::default()
        })
        .collect();
    for s in spreads {
        let means = block_differences(a, s, w, h, scale);
        for ((o, m), p) in out.iter_mut().zip(means).zip(pixels(s)) {
            o.mean_spread = o.mean_spread.max(f64::from(m));
            o.pixels_spread = o.pixels_spread.max(f64::from(p));
        }
    }
    out
}

/// Compares `b` with `a`, two RGB images `w` by `h` drawn at `scale` times the console's size,
/// each block allowed what `spreads` (`a`'s frame shifted by sub-pixel amounts) say.
pub fn compare(a: &[u8], b: &[u8], spreads: &[&[u8]], w: u32, h: u32, scale: u32) -> ImageReport {
    assert_eq!(a.len(), b.len(), "the two frames' sizes differ");
    let de = delta_e(a, b);
    let tolerant = shift_tolerant(a, b, w, &de);
    let n = de.len().max(1) as f64;
    let share = |v: &[f32], limit: f64| {
        100.0 * v.iter().filter(|&&d| f64::from(d) > limit).count() as f64 / n
    };
    let all = blocks(a, b, spreads, w, h, scale);
    let cols = w / (BLOCK * scale.max(1));
    // The block nearest its limit among those that differ at all.
    let mut worst = (Block::default(), (0, 0));
    let mut found = false;
    for (i, b) in (0u32..).zip(&all) {
        if (b.mean > 0.0 || b.pixels > 0.0) && (!found || b.excess() > worst.0.excess()) {
            found = true;
            worst = (*b, (i % cols * BLOCK, i / cols * BLOCK));
        }
    }
    ImageReport {
        width: w,
        height: h,
        identical: a == b,
        mean: de.iter().map(|&d| f64::from(d)).sum::<f64>() / n,
        max: de.iter().fold(0f32, |m, &d| m.max(d)).into(),
        over_jnd: share(&de, JND),
        over_far: share(&tolerant, FAR),
        block: worst.0,
        block_at: worst.1,
        blocks_over: all.iter().filter(|b| b.excess() > 0.0).count(),
    }
}

/// A heatmap for a person to look at: each pixel's plain ΔE₀₀ in gray, black at 0 and white at
/// [`FAR`] and above, and its shift-tolerant difference in red over it; blocks over what
/// they're allowed framed in red. RGB.
pub fn heatmap(a: &[u8], b: &[u8], spreads: &[&[u8]], w: u32, h: u32, scale: u32) -> Vec<u8> {
    let de = delta_e(a, b);
    let tolerant = shift_tolerant(a, b, w, &de);
    let all = blocks(a, b, spreads, w, h, scale);
    let side = BLOCK * scale.max(1);
    let cols = w / side;
    let level = |d: f32| ((f64::from(d) / FAR).min(1.0) * 255.0) as u8;
    let mut out = Vec::with_capacity(de.len() * 3);
    for (i, (&d, &t)) in de.iter().zip(&tolerant).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let over = x / side < cols
            && y / side < h / side
            && all[(y / side * cols + x / side) as usize].excess() > 0.0;
        let edge = x % side == 0 || y % side == 0 || x % side == side - 1 || y % side == side - 1;
        if over && edge {
            out.extend_from_slice(&[255, 0, 0]);
        } else {
            let (g, r) = (level(d) / 2, level(t));
            out.extend_from_slice(&[g.max(r), g, g]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pairs from Sharma, Wu and Dalal's test data for CIEDE2000, with their published values.
    #[test]
    fn ciede2000_matches_sharma_s_data() {
        let pairs: [([f64; 3], [f64; 3], f64); 8] = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
            ([50.0, 2.8361, -74.0200], [50.0, 0.0, -82.7485], 3.4412),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            (
                [60.2574, -34.0099, 36.2677],
                [60.4626, -34.1751, 39.4387],
                1.2644,
            ),
            (
                [2.0776, 0.0795, -1.1350],
                [0.9033, -0.0636, -0.5514],
                0.9082,
            ),
            (
                [90.8027, -2.0831, 1.4410],
                [91.1528, -1.6435, 0.0447],
                1.4441,
            ),
        ];
        for (p, q, want) in pairs {
            let got = ciede2000(p, q);
            assert!((got - want).abs() < 1e-4, "{p:?} {q:?}: {got} for {want}");
            assert!((ciede2000(q, p) - want).abs() < 1e-4, "symmetric");
        }
    }

    #[test]
    fn lab_of_white_and_black() {
        let lin = linear_table();
        let w = lab(&lin, [255; 3]);
        assert!((w[0] - 100.0).abs() < 1e-3 && w[1].abs() < 1e-2 && w[2].abs() < 1e-2);
        assert_eq!(lab(&lin, [0; 3])[0], 0.0);
        // About ΔE00 0.3 to 0.5 for one level of a mid gray: below noticeable.
        let d = ciede2000(lab(&lin, [128; 3]), lab(&lin, [129; 3]));
        assert!(d > 0.2 && d < 0.6, "{d}");
    }

    fn gray(w: u32, h: u32, v: u8) -> Vec<u8> {
        vec![v; (w * h * 3) as usize]
    }

    #[test]
    fn identical_frames_are_identical() {
        let a = gray(64, 48, 90);
        let r = compare(&a, &a, &[], 64, 48, 1);
        assert!(r.identical && r.within() && r.block.mean == 0.0);
    }

    /// Single edge pixels far apart pass; a region off fails, by its block; many scattered far
    /// pixels fail by their share.
    #[test]
    fn edges_pass_regions_fail() {
        let (w, h) = (640, 480);
        let a = gray(w, h, 90);
        let mut edge = a.clone();
        // A one-pixel line along a diagonal, every 7th pixel flipped to white.
        for i in (0..400).step_by(7) {
            let at = ((i * w + i) * 3) as usize;
            edge[at..at + 3].copy_from_slice(&[255; 3]);
        }
        let r = compare(&a, &edge, &[], w, h, 1);
        assert!(r.within(), "{}", r.line());
        // A thin bright line moved down a pixel: every pixel of it differs, but each has a
        // neighbor of its color in the other frame.
        let (mut line_a, mut line_b) = (a.clone(), a.clone());
        for x in 100..500 {
            let at = |y: u32| ((y * w + x) * 3) as usize;
            line_a[at(240)..at(240) + 3].copy_from_slice(&[255, 0, 255]);
            line_b[at(241)..at(241) + 3].copy_from_slice(&[255, 0, 255]);
        }
        let r = compare(&line_a, &line_b, &[], w, h, 1);
        assert!(
            r.within() && r.max > 30.0 && r.block.pixels == 0.0,
            "{}",
            r.line()
        );
        // A 6 by 6 square a few levels off: a material's color, a small draw's.
        let mut region = a.clone();
        for y in 100..106 {
            for x in 200..206 {
                let at = ((y * w + x) * 3) as usize;
                region[at..at + 3].copy_from_slice(&[90, 110, 90]);
            }
        }
        let r = compare(&a, &region, &[], w, h, 1);
        assert!(!r.within() && r.block.excess() > 0.0, "{}", r.line());
        assert_eq!(r.block_at, (200, 96));
        // Scattered far pixels, one a block: too many.
        let mut scattered = a.clone();
        for by in 0..h / 8 {
            for bx in 0..w / 8 {
                let at = (((by * 8) * w + bx * 8) * 3) as usize;
                scattered[at..at + 3].copy_from_slice(&[255; 3]);
            }
        }
        let r = compare(&a, &scattered, &[], w, h, 1);
        assert!(r.over_far > FAR_LIMIT && !r.within(), "{}", r.line());
    }

    /// A block may differ as much as the reference's own shifted frames do (twice that and
    /// [`BLOCK_LIMIT`]): a region that changes under a sub-pixel shift passes where it would
    /// fail on a flat one.
    #[test]
    fn spreads_widen_a_block_s_limit() {
        let (w, h) = (640, 480);
        let a = gray(w, h, 90);
        let paint = |v: u8| {
            let mut x = a.clone();
            for y in 16..24 {
                for x0 in 16..24 {
                    let at = ((y * w + x0) * 3) as usize;
                    x[at..at + 3].copy_from_slice(&[v, 90, 90]);
                }
            }
            x
        };
        let (off, shifted) = (paint(120), paint(110));
        let r = compare(&a, &off, &[], w, h, 1);
        assert!(!r.within(), "{}", r.line());
        let r = compare(&a, &off, &[&shifted], w, h, 1);
        assert!(r.within() && r.block.mean_spread > 0.0, "{}", r.line());
        // Far past twice the spread: out again.
        let r = compare(&a, &paint(250), &[&shifted], w, h, 1);
        assert!(!r.within(), "{}", r.line());
    }

    /// At 2x a block covers 16 by 16 of the frame's pixels and its corner is reported in the
    /// console's pixels.
    #[test]
    fn blocks_scale() {
        let (w, h) = (1280, 960);
        let a = gray(w, h, 90);
        let mut b = a.clone();
        for y in 400..416 {
            for x in 320..336 {
                let at = ((y * w + x) * 3) as usize;
                b[at..at + 3].copy_from_slice(&[200, 90, 90]);
            }
        }
        let r = compare(&a, &b, &[], w, h, 2);
        assert_eq!(r.block_at, (160, 200));
        assert!(r.block.mean > 10.0, "{}", r.line());
    }
}
