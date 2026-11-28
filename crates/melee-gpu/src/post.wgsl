// SPDX-License-Identifier: GPL-3.0-or-later
// The present's filters (post.rs). Sharpening follows the contrast-adaptive sharpening that
// AMD's FidelityFX CAS (MIT) describes: the cross of neighbours weighted by the headroom the
// local contrast leaves.

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> p: Params;

struct Params {
    // The frame's texels a target pixel spans, in each direction (supersampling's filter).
    span: vec2<f32>,
    // Where the picture starts in the target, in pixels (sharpening's).
    origin: vec2<f32>,
    // Sharpening's strength, 0 to 1.
    sharpness: f32,
}

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(q.x, 1.0 - q.y);
    return o;
}

/// The frame scaled as the plain blit scales it (bilinear), for sharpening alone.
@fragment
fn fs_plain(i: Out) -> @location(0) vec4<f32> {
    return textureSampleLevel(frame, smp, i.uv, 0.0);
}

/// Supersampling's filter down: the frame's texels under the pixel averaged, on a grid of
/// bilinear samples as fine as the texels (a box filter where the pixel covers whole texels).
@fragment
fn fs_down(i: Out) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(frame));
    let n = vec2<u32>(clamp(ceil(p.span - 0.001), vec2<f32>(1.0), vec2<f32>(16.0)));
    var sum = vec4<f32>(0.0);
    for (var y = 0u; y < n.y; y++) {
        for (var x = 0u; x < n.x; x++) {
            let o = (vec2<f32>(f32(x), f32(y)) + 0.5) / vec2<f32>(n) - 0.5;
            sum += textureSampleLevel(frame, smp, i.uv + o * p.span / size, 0.0);
        }
    }
    return sum / f32(n.x * n.y);
}

/// Contrast-adaptive sharpening of the picture (drawn at the target's size before): each pixel
/// pushed away from its four neighbours, by less where they already differ much or where the
/// result would clip.
@fragment
fn fs_sharpen(i: Out) -> @location(0) vec4<f32> {
    let last = vec2<i32>(textureDimensions(frame)) - 1;
    let c = clamp(vec2<i32>(floor(i.pos.xy - p.origin)), vec2<i32>(0), last);
    let b = textureLoad(frame, clamp(c + vec2<i32>(0, -1), vec2<i32>(0), last), 0).rgb;
    let d = textureLoad(frame, clamp(c + vec2<i32>(-1, 0), vec2<i32>(0), last), 0).rgb;
    let e = textureLoad(frame, c, 0).rgb;
    let f = textureLoad(frame, clamp(c + vec2<i32>(1, 0), vec2<i32>(0), last), 0).rgb;
    let h = textureLoad(frame, clamp(c + vec2<i32>(0, 1), vec2<i32>(0), last), 0).rgb;
    let lo = min(min(min(b, d), min(f, h)), e);
    let hi = max(max(max(b, d), max(f, h)), e);
    // The headroom left at the darker and the brighter end, over the brightest: 1 on flat
    // areas, toward 0 at strong edges and near black or white.
    let amount = sqrt(clamp(min(lo, 1.0 - hi) / max(hi, vec3<f32>(1.0 / 1024.0)), vec3<f32>(0.0), vec3<f32>(1.0)));
    let w = amount * (-1.0 / mix(8.0, 5.0, p.sharpness));
    let rgb = (e + (b + d + f + h) * w) / (1.0 + 4.0 * w);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
