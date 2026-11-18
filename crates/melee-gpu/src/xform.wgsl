// SPDX-License-Identifier: GPL-3.0-or-later
// Follows Dolphin's software renderer's TransformUnit (GPL-2.0-or-later), as melee-gx's xform.rs
// does: the same operations in the same order.

// The fast renderer's transforms (docs/fast-renderer.md, F3): each mesh draw's vertices through
// the transform unit on the GPU, from the mesh's vertices kept on the GPU (`arena`) and the
// draw's XF state (`xfd`), into the words the draw's vertex shader reads (gx.wgsl's `vs_mesh`,
// as `GpuVertex::write` writes them): its position snapped to the rasterizer's sixteenths and
// back in clip space, its colors and its texture coordinates. Lines and points become quads
// here, as melee-gpu's `line` and `point` make them.
//
// Every product feeding an add goes through `o`, which the compiler can't see through, so no
// multiply-add is fused: the positions and depths come out as the CPU's IEEE operations make
// them, and what lies on the depth range's edge (the shadow cameras' erases, on the near plane)
// stays exactly there.

@group(0) @binding(0) var<storage, read> arena: array<u32>;
@group(0) @binding(1) var<storage, read> records: array<u32>;
@group(0) @binding(2) var<storage, read> xfd: array<u32>;
// The first entry's x is the count; then each workgroup's record and first item.
@group(0) @binding(3) var<storage, read> work: array<vec2<u32>>;
@group(0) @binding(4) var<storage, read_write> out: array<u32>;
// The indices' copies (`cs_indices`): the meshes' indices kept on the GPU; the first entry's x
// is the count, then each workgroup's from, to, count and the draw's first output vertex; the
// flush's indices.
@group(0) @binding(5) var<storage, read> pool: array<u32>;
@group(0) @binding(6) var<storage, read> copies: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read_write> drawn: array<u32>;

// A draw's record (`gpu_mesh::Record`), in words.
const REC: u32 = 40u;
const R_IN: u32 = 0u;
const R_TRI: u32 = 1u;
const R_STRIDE: u32 = 2u;
const R_FORMAT: u32 = 3u;
const R_VCD: u32 = 4u;
const R_DEFAULTS: u32 = 5u;
const R_OUT: u32 = 8u;
const R_TEXGENS: u32 = 9u;
const R_XF: u32 = 10u;
const R_OFF: u32 = 14u;
const R_NORMALS: u32 = 16u;
const R_SIZES: u32 = 25u;
const R_SCALES: u32 = 26u;
const R_LP: u32 = 34u;
const R_LP_COUNT: u32 = 35u;
const R_ZERO: u32 = 36u;

// The XF sections' sizes in `xfd` (`gpu_mesh::SECTIONS`): position and texture matrices from
// 0x000, normal matrices from 0x400 after them.
const NORMALS_AT: u32 = 0x110u;

var<private> rec: u32;
var<private> zero: u32;
var<private> xf_mtx: u32;
var<private> xf_post: u32;
var<private> xf_lights: u32;
var<private> xf_regs: u32;

/// `x`, through an operation the compiler can't see through (`zero` is 0), so no multiply-add
/// is fused across it.
fn o(x: f32) -> f32 {
    return bitcast<f32>(bitcast<u32>(x) ^ zero);
}

fn r(i: u32) -> u32 {
    return records[rec + i];
}

fn bits(v: u32, at: u32, width: u32) -> u32 {
    return (v >> at) & ((1u << width) - 1u);
}

fn f(w: u32) -> f32 {
    return bitcast<f32>(w);
}

/// Position and texture matrix memory, from XF 0x000.
fn pm(i: u32) -> f32 {
    return f(xfd[xf_mtx + i]);
}

/// Normal matrix memory, from XF 0x400.
fn nm(i: u32) -> f32 {
    return f(xfd[xf_mtx + NORMALS_AT + i]);
}

/// Post-transform matrix memory, from XF 0x500.
fn post(i: u32) -> f32 {
    return f(xfd[xf_post + i]);
}

/// Light memory, from XF 0x600.
fn lw(i: u32) -> u32 {
    return xfd[xf_lights + i];
}

/// XF register `i` (0x1000 on).
fn reg(i: u32) -> u32 {
    return xfd[xf_regs + i - 0x1000u];
}

fn dot3(a: vec3<f32>, b: vec3<f32>) -> f32 {
    return o(a.x * b.x) + o(a.y * b.y) + o(a.z * b.z);
}

fn normalized(a: vec3<f32>) -> vec3<f32> {
    let len = sqrt(dot3(a, a));
    return vec3<f32>(a.x / len, a.y / len, a.z / len);
}

/// A 3x4 matrix of position memory at `at` times (v, 1).
fn mat34(at: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        o(pm(at) * v.x) + o(pm(at + 1u) * v.y) + o(pm(at + 2u) * v.z) + pm(at + 3u),
        o(pm(at + 4u) * v.x) + o(pm(at + 5u) * v.y) + o(pm(at + 6u) * v.z) + pm(at + 7u),
        o(pm(at + 8u) * v.x) + o(pm(at + 9u) * v.y) + o(pm(at + 10u) * v.z) + pm(at + 11u),
    );
}

fn post34(at: u32, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        o(post(at) * v.x) + o(post(at + 1u) * v.y) + o(post(at + 2u) * v.z) + post(at + 3u),
        o(post(at + 4u) * v.x) + o(post(at + 5u) * v.y) + o(post(at + 6u) * v.z) + post(at + 7u),
        o(post(at + 8u) * v.x) + o(post(at + 9u) * v.y) + o(post(at + 10u) * v.z) + post(at + 11u),
    );
}

fn is_nan(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7FFFFFFFu) > 0x7F800000u;
}

fn is_finite(x: f32) -> bool {
    return (bitcast<u32>(x) & 0x7F800000u) != 0x7F800000u;
}

/// A vertex as the transform unit takes it.
struct Input {
    pos_mtx: u32,
    tex_mtx: array<u32, 8>,
    position: vec3<f32>,
    normal: array<vec3<f32>, 3>,
    // RGBA, 0 to 255.
    color: array<vec4<u32>, 2>,
    tex: array<vec2<f32>, 8>,
}

/// A transformed vertex.
struct Output {
    mv: vec3<f32>,
    clip: vec4<f32>,
    normal: array<vec3<f32>, 3>,
    color: array<vec4<u32>, 2>,
    tex: array<vec3<f32>, 8>,
}

fn word_vec3(at: u32) -> vec3<f32> {
    return vec3<f32>(f(arena[at]), f(arena[at + 1u]), f(arena[at + 2u]));
}

fn le_color(w: u32) -> vec4<u32> {
    return vec4<u32>(w & 0xFFu, (w >> 8u) & 0xFFu, (w >> 16u) & 0xFFu, w >> 24u);
}

fn be_color(w: u32) -> vec4<u32> {
    return vec4<u32>(w >> 24u, (w >> 16u) & 0xFFu, (w >> 8u) & 0xFFu, w & 0xFFu);
}

/// Vertex `v` of the record's mesh, as `Mesh::input` makes it.
fn input(v: u32) -> Input {
    var i: Input;
    let format = r(R_FORMAT);
    let nc = bits(format, 0u, 2u);
    let tc = bits(format, 2u, 4u);
    let at = r(R_IN) + v * r(R_STRIDE);
    let d0 = r(R_DEFAULTS);
    let d1 = r(R_DEFAULTS + 1u);
    let d2 = r(R_DEFAULTS + 2u);
    i.pos_mtx = d0 & 0xFFu;
    i.tex_mtx = array<u32, 8>(
        (d0 >> 8u) & 0xFFu, (d0 >> 16u) & 0xFFu, d0 >> 24u, d1 & 0xFFu,
        (d1 >> 8u) & 0xFFu, (d1 >> 16u) & 0xFFu, d1 >> 24u, d2 & 0xFFu,
    );
    if bits(format, 6u, 1u) != 0u {
        let m = at + 5u + 3u * nc + 2u * tc;
        let m0 = arena[m];
        let m1 = arena[m + 1u];
        let m2 = arena[m + 2u];
        let lo = r(R_VCD);
        if bits(lo, 0u, 1u) != 0u {
            i.pos_mtx = m0 & 0xFFu;
        }
        let carried = array<u32, 8>(
            (m0 >> 8u) & 0xFFu, (m0 >> 16u) & 0xFFu, m0 >> 24u, m1 & 0xFFu,
            (m1 >> 8u) & 0xFFu, (m1 >> 16u) & 0xFFu, m1 >> 24u, m2 & 0xFFu,
        );
        if bits(lo, 1u, 1u) != 0u { i.tex_mtx[0] = carried[0]; }
        if bits(lo, 2u, 1u) != 0u { i.tex_mtx[1] = carried[1]; }
        if bits(lo, 3u, 1u) != 0u { i.tex_mtx[2] = carried[2]; }
        if bits(lo, 4u, 1u) != 0u { i.tex_mtx[3] = carried[3]; }
        if bits(lo, 5u, 1u) != 0u { i.tex_mtx[4] = carried[4]; }
        if bits(lo, 6u, 1u) != 0u { i.tex_mtx[5] = carried[5]; }
        if bits(lo, 7u, 1u) != 0u { i.tex_mtx[6] = carried[6]; }
        if bits(lo, 8u, 1u) != 0u { i.tex_mtx[7] = carried[7]; }
    }
    i.position = word_vec3(at);
    // The normals the vertex doesn't carry are those the draws before left.
    let n = R_NORMALS;
    var normals = array<vec3<f32>, 3>(
        vec3<f32>(f(r(n)), f(r(n + 1u)), f(r(n + 2u))),
        vec3<f32>(f(r(n + 3u)), f(r(n + 4u)), f(r(n + 5u))),
        vec3<f32>(f(r(n + 6u)), f(r(n + 7u)), f(r(n + 8u))),
    );
    if nc > 0u { normals[0] = word_vec3(at + 3u); }
    if nc > 1u {
        normals[1] = word_vec3(at + 6u);
        normals[2] = word_vec3(at + 9u);
    }
    i.normal = normals;
    let c = at + 3u + 3u * nc;
    i.color = array<vec4<u32>, 2>(le_color(arena[c]), le_color(arena[c + 1u]));
    let t = c + 2u;
    var tex: array<vec2<f32>, 8>;
    if tc > 0u { tex[0] = vec2<f32>(f(arena[t]), f(arena[t + 1u])); }
    if tc > 1u { tex[1] = vec2<f32>(f(arena[t + 2u]), f(arena[t + 3u])); }
    if tc > 2u { tex[2] = vec2<f32>(f(arena[t + 4u]), f(arena[t + 5u])); }
    if tc > 3u { tex[3] = vec2<f32>(f(arena[t + 6u]), f(arena[t + 7u])); }
    if tc > 4u { tex[4] = vec2<f32>(f(arena[t + 8u]), f(arena[t + 9u])); }
    if tc > 5u { tex[5] = vec2<f32>(f(arena[t + 10u]), f(arena[t + 11u])); }
    if tc > 6u { tex[6] = vec2<f32>(f(arena[t + 12u]), f(arena[t + 13u])); }
    if tc > 7u { tex[7] = vec2<f32>(f(arena[t + 14u]), f(arena[t + 15u])); }
    i.tex = tex;
    return i;
}

/// A light: its color (RGBA), angle and distance attenuation, position and direction.
struct Light {
    color: vec4<u32>,
    cos_att: vec3<f32>,
    dist_att: vec3<f32>,
    pos: vec3<f32>,
    dir: vec3<f32>,
}

fn light_vec3(at: u32) -> vec3<f32> {
    return vec3<f32>(f(lw(at)), f(lw(at + 1u)), f(lw(at + 2u)));
}

fn light(n: u32) -> Light {
    let at = n * 16u;
    var l: Light;
    l.color = be_color(lw(at + 3u));
    l.cos_att = light_vec3(at + 4u);
    l.dist_att = light_vec3(at + 7u);
    l.pos = light_vec3(at + 10u);
    l.dir = light_vec3(at + 13u);
    return l;
}

fn safe_divide(n: f32, d: f32) -> f32 {
    if d == 0.0 {
        return select(0.0, 1.0, n > 0.0);
    }
    return n / d;
}

/// A light's contribution's scale to channel `chan`, for a vertex at `mv` with normal `normal`.
fn contribution(l: Light, mv: vec3<f32>, normal: vec3<f32>, chan: u32) -> f32 {
    var ldir = l.pos - mv;
    var attn = 1.0;
    switch bits(chan, 9u, 2u) {
        // Spec
        case 1u: {
            ldir = normalized(ldir);
            var a = 0.0;
            if dot3(ldir, normal) >= 0.0 {
                a = max(dot3(l.dir, normal), 0.0);
            }
            let len = vec3<f32>(1.0, a, o(a * a));
            var dist = l.dist_att;
            if bits(chan, 7u, 2u) != 0u {
                dist = normalized(l.dist_att);
            }
            attn = safe_divide(max(dot3(len, l.cos_att), 0.0), dot3(len, dist));
        }
        // Spot
        case 3u: {
            let dist2 = dot3(ldir, ldir);
            let dist = sqrt(dist2);
            let inv = 1.0 / dist;
            ldir = vec3<f32>(ldir.x * inv, ldir.y * inv, ldir.z * inv);
            let a = max(dot3(ldir, l.dir), 0.0);
            let c = l.cos_att;
            let d = l.dist_att;
            let cos_att = c.x + o(c.y * a) + o(o(c.z * a) * a);
            let dist_att = d.x + o(d.y * dist) + o(d.z * dist2);
            attn = safe_divide(max(cos_att, 0.0), dist_att);
        }
        // None and Dir
        default: {
            ldir = normalized(ldir);
            if all(ldir == vec3<f32>(0.0)) {
                ldir = normal;
            }
        }
    }
    let dif = dot3(ldir, normal);
    switch bits(chan, 7u, 2u) {
        case 0u: { return attn; }
        case 1u: { return attn * dif; }
        default: { return attn * max(dif, 0.0); }
    }
}

fn light_mask(chan: u32) -> u32 {
    if bits(chan, 1u, 1u) != 0u {
        return bits(chan, 2u, 4u) | (bits(chan, 11u, 4u) << 4u);
    }
    return 0u;
}

/// A material level lit: as the transform unit's integers (`xform::lit`), a NaN as 0.
fn lit(material: u32, light: f32) -> u32 {
    var l = 0;
    if light >= 0.0 {
        l = i32(min(light, 255.0));
    }
    return (material * u32(l + (l >> 7u))) >> 8u;
}

/// Color channel `ch` of a vertex, as `Setup::colors` makes it.
fn channel(ch: u32, vc: vec4<u32>, mv: vec3<f32>, normal: vec3<f32>) -> vec4<u32> {
    let cc = reg(0x100Eu + ch);
    let ac = reg(0x1010u + ch);
    let mat_reg = be_color(reg(0x100Cu + ch));
    let amb_reg = be_color(reg(0x100Au + ch));
    var color = vec4<u32>(0u);
    var mat = mat_reg;
    if bits(cc, 0u, 1u) != 0u {
        mat = vc;
    }
    if bits(cc, 1u, 1u) != 0u {
        var amb = amb_reg;
        if bits(cc, 6u, 1u) != 0u {
            amb = vc;
        }
        var light_sum = vec3<f32>(f32(amb.x), f32(amb.y), f32(amb.z));
        let mask = light_mask(cc);
        for (var i = 0u; i < 8u; i++) {
            if (mask & (1u << i)) != 0u {
                let l = light(i);
                let s = contribution(l, mv, normal, cc);
                light_sum = vec3<f32>(
                    light_sum.x + o(f32(l.color.x) * s),
                    light_sum.y + o(f32(l.color.y) * s),
                    light_sum.z + o(f32(l.color.z) * s),
                );
            }
        }
        color = vec4<u32>(lit(mat.x, light_sum.x), lit(mat.y, light_sum.y), lit(mat.z, light_sum.z), 0u);
    } else {
        color = vec4<u32>(mat.xyz, 0u);
    }
    var mat_a = mat_reg.w;
    if bits(ac, 0u, 1u) != 0u {
        mat_a = vc.w;
    }
    if bits(ac, 1u, 1u) != 0u {
        var light_a = f32(amb_reg.w);
        if bits(ac, 6u, 1u) != 0u {
            light_a = f32(vc.w);
        }
        let mask = light_mask(ac);
        for (var i = 0u; i < 8u; i++) {
            if (mask & (1u << i)) != 0u {
                let l = light(i);
                light_a = light_a + o(f32(l.color.w) * contribution(l, mv, normal, ac));
            }
        }
        color.w = lit(mat_a, light_a);
    } else {
        color.w = mat_a;
    }
    return color;
}

/// A regular texture coordinate `n` (`xform::regular`).
fn regular(info: u32, n: u32, v: Input) -> vec3<f32> {
    var src = vec3<f32>(0.0, 0.0, 1.0);
    let row = bits(info, 7u, 5u);
    switch row {
        case 0u: { src = v.position; }
        case 1u: { src = v.normal[0]; }
        case 3u: { src = v.normal[1]; }
        case 4u: { src = v.normal[2]; }
        case 5u, 6u, 7u, 8u, 9u, 10u, 11u, 12u: {
            let t = v.tex[row - 5u];
            src = vec3<f32>(t.x, t.y, 1.0);
        }
        default: {}
    }
    if is_nan(src.x) { src.x = 1.0; }
    if is_nan(src.y) { src.y = 1.0; }
    if is_nan(src.z) { src.z = 1.0; }
    let m = v.tex_mtx[n] * 4u;
    // AB11 takes (a, b, 1, 1); ABC1 (a, b, c, 1).
    let ab11 = bits(info, 2u, 1u) == 0u;
    var third = vec3<f32>(pm(m + 2u), pm(m + 6u), pm(m + 10u));
    if !ab11 {
        third = vec3<f32>(o(pm(m + 2u) * src.z), o(pm(m + 6u) * src.z), o(pm(m + 10u) * src.z));
    }
    let row0 = o(pm(m) * src.x) + o(pm(m + 1u) * src.y) + third.x + pm(m + 3u);
    let row1 = o(pm(m + 4u) * src.x) + o(pm(m + 5u) * src.y) + third.y + pm(m + 7u);
    var dst = vec3<f32>(row0, row1, 1.0);
    if bits(info, 1u, 1u) != 0u {
        dst.z = o(pm(m + 8u) * src.x) + o(pm(m + 9u) * src.y) + third.z + pm(m + 11u);
    }
    if (reg(0x1012u) & 1u) != 0u {
        let p = reg(0x1050u + n);
        var tmp = dst;
        if bits(p, 8u, 1u) != 0u {
            tmp = normalized(dst);
        }
        dst = post34(bits(p, 0u, 6u) * 4u, tmp);
    }
    if dst.z == 0.0 {
        dst.x = clamp(dst.x / 2.0, -1.0, 1.0);
        dst.y = clamp(dst.y / 2.0, -1.0, 1.0);
    }
    return dst;
}

/// Texture coordinate `n`, the earlier ones in `t` and the later ones zeros (`Setup::tex_coords`).
fn texgen(n: u32, t: array<vec3<f32>, 8>, v: Input, out: Output) -> vec3<f32> {
    let info = reg(0x1040u + n);
    switch bits(info, 4u, 3u) {
        case 1u: {
            let l = light(bits(info, 15u, 3u));
            let ldir = normalized(l.pos - out.mv);
            let src = t[bits(info, 12u, 3u)];
            return vec3<f32>(src.x + dot3(ldir, out.normal[1]), src.y + dot3(ldir, out.normal[2]), src.z);
        }
        case 2u, 3u: {
            let c = out.color[bits(info, 4u, 3u) - 2u];
            return vec3<f32>(f32(c.x) / 255.0, f32(c.y) / 255.0, 1.0);
        }
        default: {
            return regular(info, n, v);
        }
    }
}

/// Vertex `v` of the record's mesh through the transform unit (`Setup::transform`).
fn transform(v: u32) -> Output {
    let i = input(v);
    var out: Output;
    out.mv = mat34(i.pos_mtx * 4u, i.position);
    let mv = out.mv;
    let p = 0x1020u;
    if reg(p + 6u) == 0u {
        out.clip = vec4<f32>(
            o(f(reg(p)) * mv.x) + o(f(reg(p + 1u)) * mv.z),
            o(f(reg(p + 2u)) * mv.y) + o(f(reg(p + 3u)) * mv.z),
            // (p4 mv.z + p5) (1 - 1e-7), as f32 rounds 1 - 1e-7.
            o((o(f(reg(p + 4u)) * mv.z) + f(reg(p + 5u))) * bitcast<f32>(0x3F7FFFFEu)),
            -mv.z,
        );
    } else {
        out.clip = vec4<f32>(
            o(f(reg(p)) * mv.x) + f(reg(p + 1u)),
            o(f(reg(p + 2u)) * mv.y) + f(reg(p + 3u)),
            o(f(reg(p + 4u)) * mv.z) + f(reg(p + 5u)),
            1.0,
        );
    }
    // The binormals too, which only an emboss coordinate reads (then the CPU transforms them).
    let nmat = (i.pos_mtx & 31u) * 3u;
    out.normal[0] = normalized(normal_mtx(nmat, i.normal[0]));
    out.normal[1] = normal_mtx(nmat, i.normal[1]);
    out.normal[2] = normal_mtx(nmat, i.normal[2]);
    out.color[0] = channel(0u, i.color[0], out.mv, out.normal[0]);
    out.color[1] = channel(1u, i.color[1], out.mv, out.normal[0]);
    let count = min(reg(0x103Fu) & 0xFu, 8u);
    var t: array<vec3<f32>, 8>;
    if count > 0u { t[0] = texgen(0u, t, i, out); }
    if count > 1u { t[1] = texgen(1u, t, i, out); }
    if count > 2u { t[2] = texgen(2u, t, i, out); }
    if count > 3u { t[3] = texgen(3u, t, i, out); }
    if count > 4u { t[4] = texgen(4u, t, i, out); }
    if count > 5u { t[5] = texgen(5u, t, i, out); }
    if count > 6u { t[6] = texgen(6u, t, i, out); }
    if count > 7u { t[7] = texgen(7u, t, i, out); }
    // Scaled to texels, by the BP's sizes. Written out rather than looped: arrays written by a
    // variable index need FXC (D3D12) to unroll the loop (see gx.wgsl).
    out.tex = array<vec3<f32>, 8>(
        scaled(0u, count, t[0]), scaled(1u, count, t[1]), scaled(2u, count, t[2]), scaled(3u, count, t[3]),
        scaled(4u, count, t[4]), scaled(5u, count, t[5]), scaled(6u, count, t[6]), scaled(7u, count, t[7]),
    );
    return out;
}

fn normal_mtx(at: u32, n: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        o(nm(at) * n.x) + o(nm(at + 1u) * n.y) + o(nm(at + 2u) * n.z),
        o(nm(at + 3u) * n.x) + o(nm(at + 4u) * n.y) + o(nm(at + 5u) * n.z),
        o(nm(at + 6u) * n.x) + o(nm(at + 7u) * n.y) + o(nm(at + 8u) * n.z),
    );
}

/// Texture coordinate `n` of `count` scaled to texels.
fn scaled(n: u32, count: u32, t: vec3<f32>) -> vec3<f32> {
    if n >= count {
        return t;
    }
    let s = r(R_SCALES + n);
    return vec3<f32>(t.x * (f32(s & 0xFFFFu) + 1.0), t.y * (f32(s >> 16u) + 1.0), t.z);
}

/// The viewport: width and height halves, depth range, origin, and far depth.
struct Viewport {
    wd: f32,
    ht: f32,
    z_range: f32,
    x_orig: f32,
    y_orig: f32,
    far_z: f32,
}

fn viewport() -> Viewport {
    let at = 0x101Au;
    return Viewport(
        f(reg(at)), f(reg(at + 1u)), f(reg(at + 2u)),
        f(reg(at + 3u)), f(reg(at + 4u)), f(reg(at + 5u)),
    );
}

/// A clip-space position on the screen (`Viewport::screen`).
fn screen(vp: Viewport, clip: vec4<f32>) -> vec3<f32> {
    let inv = 1.0 / clip.w;
    return vec3<f32>(
        o(o(clip.x * inv) * vp.wd) + vp.x_orig,
        o(o(clip.y * inv) * vp.ht) + vp.y_orig,
        o(o(clip.z * inv) * vp.z_range) + vp.far_z,
    );
}

/// Dolphin's rounding of a float to an integer (`iround`).
fn iround(x: f32) -> f32 {
    let t = trunc(x);
    if x - t >= 0.5 {
        return t + 1.0;
    }
    return t;
}

fn snap(v: f32) -> f32 {
    return (iround(16.0 * v) - 1.0) / 16.0;
}

/// A vertex at `at` in the EFB, back in clip space for the GPU to clip (`gpu_position`).
fn gpu_position(vp: Viewport, clip: vec4<f32>, at: vec3<f32>) -> vec4<f32> {
    let w = clip.w;
    let off = vec2<f32>(f(r(R_OFF)), f(r(R_OFF + 1u)));
    var x = snap(at.x - off.x) / 320.0 - 1.0;
    var y = 1.0 - snap(at.y - off.y) / 264.0;
    if is_finite(x) && is_finite(y) {
        x = x * w;
        y = y * w;
    } else {
        x = o(clip.x * vp.wd) / 320.0;
        y = o(-clip.y * vp.ht) / 264.0;
    }
    return vec4<f32>(x, y, clip.z + w, w);
}

fn le_word(c: vec4<u32>) -> u32 {
    return c.x | (c.y << 8u) | (c.z << 16u) | (c.w << 24u);
}

/// Writes output vertex `k` of the draw (`GpuVertex::write`).
fn write(k: u32, pos: vec4<f32>, v: Output) {
    let tg = r(R_TEXGENS);
    let at = (r(R_OUT) + k) * (6u + 3u * tg);
    out[at] = bitcast<u32>(pos.x);
    out[at + 1u] = bitcast<u32>(pos.y);
    out[at + 2u] = bitcast<u32>(pos.z);
    out[at + 3u] = bitcast<u32>(pos.w);
    out[at + 4u] = le_word(v.color[0]);
    out[at + 5u] = le_word(v.color[1]);
    for (var n = 0u; n < 8u; n++) {
        if n < tg {
            let b = at + 6u + 3u * n;
            out[b] = bitcast<u32>(v.tex[n].x);
            out[b + 1u] = bitcast<u32>(v.tex[n].y);
            out[b + 2u] = bitcast<u32>(v.tex[n].z);
        }
    }
}

@compute @workgroup_size(64)
fn cs_main(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let g = wg.y * groups.x + wg.x;
    if g >= work[0].x {
        return;
    }
    let w = work[g + 1u];
    rec = w.x * REC;
    zero = r(R_ZERO);
    xf_mtx = r(R_XF);
    xf_post = r(R_XF + 1u);
    xf_lights = r(R_XF + 2u);
    xf_regs = r(R_XF + 3u);
    let item = w.y + li;
    let tri = r(R_TRI);
    if item >= tri + r(R_LP_COUNT) {
        return;
    }
    // The harness's geometry: each vertex's clip-space position (`Renderer::mesh_clips`).
    if bits(r(R_FORMAT), 7u, 1u) != 0u {
        let c = transform(item).clip;
        let at = (r(R_OUT) + item) * 4u;
        out[at] = bitcast<u32>(c.x);
        out[at + 1u] = bitcast<u32>(c.y);
        out[at + 2u] = bitcast<u32>(c.z);
        out[at + 3u] = bitcast<u32>(c.w);
        return;
    }
    let vp = viewport();
    if item < tri {
        let v = transform(item);
        write(item, gpu_position(vp, v.clip, screen(vp, v.clip)), v);
        return;
    }
    // A line's or point's quad, after the triangles' vertices.
    let e = item - tri;
    let a = arena[r(R_LP) + 2u * e];
    let b = arena[r(R_LP) + 2u * e + 1u];
    let sizes = r(R_SIZES);
    let base = tri + 4u * e;
    let va = transform(a);
    let sa = screen(vp, va.clip);
    if b == 0xFFFFFFFFu {
        // Corners (ll, ul, lr, ur), as `point` makes them.
        let rad = f32(bits(sizes, 8u, 8u)) / 12.0;
        write(base, gpu_position(vp, va.clip, vec3<f32>(sa.x - rad, sa.y - rad, sa.z)), va);
        write(base + 1u, gpu_position(vp, va.clip, vec3<f32>(sa.x - rad, sa.y + rad, sa.z)), va);
        write(base + 2u, gpu_position(vp, va.clip, vec3<f32>(sa.x + rad, sa.y - rad, sa.z)), va);
        write(base + 3u, gpu_position(vp, va.clip, vec3<f32>(sa.x + rad, sa.y + rad, sa.z)), va);
        return;
    }
    // (b1, b0, a0, a1), as `line` makes them: its caps vertical or horizontal by its slope.
    let vb = transform(b);
    let sb = screen(vp, vb.clip);
    let half = f32(bits(sizes, 0u, 8u)) / 12.0;
    let dx = sb.x - sa.x;
    let dy = sb.y - sa.y;
    var px = 0.0;
    var py = 0.0;
    if abs(dx) > abs(dy) {
        py = select(1.0, -1.0, dx > 0.0);
    } else {
        px = select(-1.0, 1.0, dy > 0.0);
    }
    let ox = px * half;
    let oy = py * half;
    write(base, gpu_position(vp, vb.clip, vec3<f32>(sb.x - ox, sb.y - oy, sb.z)), vb);
    write(base + 1u, gpu_position(vp, vb.clip, vec3<f32>(sb.x + ox, sb.y + oy, sb.z)), vb);
    write(base + 2u, gpu_position(vp, va.clip, vec3<f32>(sa.x + ox, sa.y + oy, sa.z)), va);
    write(base + 3u, gpu_position(vp, va.clip, vec3<f32>(sa.x - ox, sa.y - oy, sa.z)), va);
}

/// Copies the flush's draws' indices from the meshes' kept on the GPU, each plus its draw's first
/// output vertex, so a run of draws with the same state is drawn as one.
@compute @workgroup_size(64)
fn cs_indices(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let g = wg.y * groups.x + wg.x;
    if g >= copies[0].x {
        return;
    }
    let c = copies[g + 1u];
    if li < c.z {
        drawn[c.y + li] = pool[c.x + li] + c.w;
    }
}
