// SPDX-License-Identifier: GPL-3.0-or-later

// A copy from the EFB to a texture, kept on the GPU: each texel as the texture decoder would
// read back what the copy encoder (encode.rs) writes, for the formats copied this way (I4,
// RGB565 and RGB5A3).

struct Params {
    left: u32,
    top: u32,
    half: u32,
    format: u32,
    yuv: u32,
    six: u32,
    // How many scaled pixels tall an EFB pixel is, and the scaled EFB's width for the
    // console's 640.
    scale: u32,
    width: u32,
    // How many texels the texture has for each of the copy's in each direction: 1, or the
    // EFB's scale for a copy at the render's scale.
    factor: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var efb: texture_2d<f32>;
@group(0) @binding(1) var<uniform> p: Params;

const EFB_WIDTH: u32 = 640u;
const EFB_HEIGHT: u32 = 528u;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// Part (`jx`, `jy`) of EFB pixel `i` (counting along rows of 640 as the encoder does), cut
// into `factor` by `factor` parts: the scaled pixels in it. With a factor of 1, the whole
// pixel's: the scaled pixels from its column's start to the next column's, its rows'.
fn pixel(i: u32, jx: u32, jy: u32) -> vec4<u32> {
    if i >= EFB_WIDTH * EFB_HEIGHT {
        return vec4<u32>(0u);
    }
    let s = max(p.scale, 1u);
    let w = max(p.width, 1u);
    let f = max(p.factor, 1u);
    let rows = max(s / f, 1u);
    let ex = (i % EFB_WIDTH) * f + jx;
    let cols = EFB_WIDTH * f;
    let left = ex * w / cols;
    let n = max((ex + 1u) * w / cols, left + 1u) - left;
    let at = vec2<u32>(left, i / EFB_WIDTH * s + jy * rows);
    var sum = vec4<f32>(0.0);
    for (var y = 0u; y < rows; y++) {
        for (var x = 0u; x < n; x++) {
            sum += textureLoad(efb, at + vec2<u32>(x, y), 0);
        }
    }
    let c = vec4<u32>(round(sum / f32(n * rows) * 255.0));
    return vec4<u32>(c.rgb, select(255u, c.a, p.six != 0u));
}

// The part (`x`, `y`) of the copied rectangle cut into `factor` by `factor` parts per pixel.
fn part(x: u32, y: u32) -> vec4<u32> {
    let f = max(p.factor, 1u);
    let i = p.top * EFB_WIDTH + p.left + y / f * EFB_WIDTH + x / f;
    return pixel(i, x % f, y % f);
}

// A texel's color: its part of the rectangle, or the 2x2 box of parts at it at half scale.
fn texel(s: u32, t: u32) -> vec4<u32> {
    let x = s << p.half;
    let y = t << p.half;
    if p.half == 0u {
        return part(x, y);
    }
    let a = part(x, y);
    let b = part(x + 1u, y);
    let c = part(x, y + 1u);
    let d = part(x + 1u, y + 1u);
    if p.six != 0u {
        // Sums of six-bit values, widened.
        let sum = (a >> vec4<u32>(2u)) + (b >> vec4<u32>(2u)) + (c >> vec4<u32>(2u)) + (d >> vec4<u32>(2u));
        return (sum + (sum >> vec4<u32>(6u))) & vec4<u32>(0xFFu);
    }
    let avg = (a + b + c + d) >> vec4<u32>(2u);
    return vec4<u32>(avg.rgb, 255u);
}

fn c3(v: u32) -> u32 {
    return (v << 5u | v << 2u | v >> 1u) & 0xFFu;
}

fn c4(v: u32) -> u32 {
    return (v << 4u | v) & 0xFFu;
}

fn c5(v: u32) -> u32 {
    return (v << 3u | v >> 2u) & 0xFFu;
}

fn c6(v: u32) -> u32 {
    return (v << 2u | v >> 4u) & 0xFFu;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let c = texel(u32(pos.x), u32(pos.y));
    if p.format == 0u {
        // I4: the red channel, or the intensity, to four bits.
        var x = c.r;
        if p.yuv != 0u {
            x = (4096u + 66u * c.r + 129u * c.g + 25u * c.b) >> 8u;
        }
        let i = c4(x >> 4u);
        return vec4<u32>(i, i, i, i);
    }
    if p.format == 4u {
        // RGB565: opaque, 5:6:5.
        return vec4<u32>(c5(c.r >> 3u), c6(c.g >> 2u), c5(c.b >> 3u), 255u);
    }
    // RGB5A3: opaque as 5:5:5, otherwise 4:4:4 with three bits of alpha.
    if c.a >> 5u == 7u {
        return vec4<u32>(c5(c.r >> 3u), c5(c.g >> 3u), c5(c.b >> 3u), 255u);
    }
    return vec4<u32>(c4(c.r >> 4u), c4(c.g >> 4u), c4(c.b >> 4u), c3(c.a >> 5u));
}
