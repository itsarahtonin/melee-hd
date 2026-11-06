// SPDX-License-Identifier: GPL-3.0-or-later

// The EFB's depth read back for a depth copy: each of the console's 640 by 528 pixels its
// 24-bit z, from the middle of its block of scaled pixels as `Renderer::read_efb` takes color
// (a multisampled EFB's first sample), as (z >> 16, z >> 8 & 0xFF, z & 0xFF, 255), the bytes
// encode.rs lays out as Z24X8. DEPTH is `texture_depth_2d` or, for a multisampled EFB,
// `texture_depth_multisampled_2d`.

struct Params {
    // How many scaled pixels tall an EFB pixel is, and the scaled EFB's width for the
    // console's 640.
    scale: u32,
    width: u32,
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0) var depth: DEPTH;
@group(0) @binding(1) var<uniform> p: Params;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let x = u32(pos.x);
    let y = u32(pos.y);
    let px = (x * p.width / 640u + (x + 1u) * p.width / 640u) / 2u;
    let py = y * p.scale + p.scale / 2u;
    let d = textureLoad(depth, vec2<i32>(i32(px), i32(py)), 0);
    // The depth buffer holds z as (z + 1.5) / 2^24 below 2^23 - 1 and (z + 1) / 2^24 from
    // there (gx.wgsl's `efb_depth`), exactly, so this is z.
    let z = u32(clamp(floor(d * 16777216.0 - 1.0), 0.0, 16777215.0));
    return vec4<u32>(z >> 16u, (z >> 8u) & 0xFFu, z & 0xFFu, 255u);
}
