//! The transforms on the GPU (F3, xform.wgsl) as the CPU's, over mesh_tests' random display
//! lists, transform states and normals left by the draws before:
//!
//! - each vertex's clip-space position is the CPU's bit for bit (no multiply-add fused, no sum
//!   reordered: Metal's fast maths reassociates, NVIDIA's Vulkan compiler reorders), so what
//!   lies on the depth range's edge stays exactly there, on every backend (Metal is checked by
//!   CI's macOS runner alone);
//! - the words each draw's vertex shader reads (triangles' vertices, lines' and points' quads)
//!   are those `crate::build` writes, within float noise: positions within a 64th of a pixel
//!   (or both off the snap's grid), depth and w exact, colors within a level, texture
//!   coordinates within 1e-4 of their size, where the CPU's are finite.

use std::sync::Arc;

use melee_gx::vertex::NormalCache;
use melee_gx::xform::{Output, Viewport};

use crate::mesh::{self, Kind, Mesh};
use crate::mesh_tests::{Flat, Rng, format, list, state};
use crate::{Built, Renderer};

/// The words the CPU writes for the mesh's output vertices, in the transforms' order, and
/// whether each is a line's.
fn cpu_words(s: &melee_gx::State, m: &Mesh, normals: &NormalCache) -> (Vec<u32>, Vec<bool>) {
    let vp = Viewport::of(&s.xf);
    let (_, off) = Renderer::scissor(s, &vp);
    let tg = crate::texgens(s);
    let mut outs: Vec<Output> = Vec::new();
    m.transform(s, normals, |_, o| outs.push(*o));
    let screens: Vec<[f32; 3]> = outs.iter().map(|o| vp.screen(o.clip)).collect();
    let mut built = Built::default();
    let mut of_lines = Vec::new();
    if m.runs.iter().any(|r| r.0 == Kind::Triangles) {
        for (o, sc) in outs.iter().zip(&screens) {
            Renderer::gpu_vertex(o, *sc, off, &vp).write(tg, &mut built.words);
            of_lines.push(false);
        }
    }
    for (kind, run) in &m.runs {
        let els = &m.elements[run.start as usize..run.end as usize];
        match kind {
            Kind::Triangles => {}
            Kind::Lines => {
                for l in els.as_chunks::<2>().0 {
                    let (a, b) = (l[0] as usize, l[1] as usize);
                    crate::line(
                        s,
                        &vp,
                        off,
                        (&outs[a], screens[a]),
                        (&outs[b], screens[b]),
                        0,
                        &mut built,
                    );
                    of_lines.extend([true; 4]);
                }
            }
            Kind::Points => {
                for &i in els {
                    let i = i as usize;
                    mesh::point(s, &vp, off, &outs[i], screens[i], 0, &mut built);
                    of_lines.extend([false; 4]);
                }
            }
        }
    }
    (built.words, of_lines)
}

#[derive(Default, Debug)]
struct Counts {
    compared: u64,
    positions: u64,
    /// Lines' corners: float noise turns a line's caps the other way where its slope is near
    /// 45 degrees or it's nearly a point.
    line_corners: u64,
    depths: u64,
    colors: u64,
    tex: u64,
}

/// Compares one vertex's words; counts what's out of tolerance.
fn compare(want: &[u32], got: &[u32], line: bool, c: &mut Counts) {
    let f = |w: u32| f32::from_bits(w);
    if want[..4].iter().any(|&w| !f(w).is_finite()) || got[..4].iter().any(|&w| !f(w).is_finite()) {
        return;
    }
    c.compared += 1;
    let w = f(want[3]).abs().max(1e-6);
    // A sixty-fourth of a pixel, in clip space (x and y are NDC times w); or the snap's grid,
    // where float noise moves a position across a sixteenth's boundary.
    let tol = w * (1.0 / 64.0) / 264.0;
    let grid = w * (1.0 / 16.0) / 264.0 * 1.001;
    for k in 0..2 {
        let d = (f(want[k]) - f(got[k])).abs();
        // Far off screen, a few ulps of the position (the GPU divides otherwise).
        let ulps = 4.0 * f32::EPSILON * f(want[k]).abs();
        if d > tol && d > grid && d > ulps {
            if line {
                c.line_corners += 1;
            } else {
                c.positions += 1;
            }
        }
    }
    if want[2..4] != got[2..4] {
        c.depths += 1;
    }
    for k in 4..6 {
        let (a, b) = (want[k].to_le_bytes(), got[k].to_le_bytes());
        if a.iter().zip(&b).any(|(x, y)| x.abs_diff(*y) > 1) {
            c.colors += 1;
        }
    }
    for k in 6..want.len() {
        let (a, b) = (f(want[k]), f(got[k]));
        if a.is_finite() && (a - b).abs() > 1e-4 * a.abs().max(1.0) {
            c.tex += 1;
        }
    }
}

#[test]
fn the_gpu_transforms_as_the_cpu_does() {
    let Some(mut renderer) = crate::testing::renderer() else {
        return;
    };
    // The writes and submissions on a thread of their own, as the render thread makes them.
    let device = renderer.device.clone();
    let mut queue = crate::submit::Ordered::new(renderer.queue.raw().clone());
    queue.start();
    let layout = renderer.vertex_layout.clone();
    let meshes = renderer.gpu_meshes();
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let mem = Flat((0..8192).map(|_| r.next() as u8).collect());
    let mut c = Counts::default();
    let (mut clips_compared, mut lines, mut points) = (0u64, 0, 0);
    for round in 0..1500 {
        let cp = format(&mut r);
        let list = list(&mut r, &cp);
        let s = state(&mut r, cp);
        let normals: NormalCache = std::array::from_fn(|_| std::array::from_fn(|_| r.float()));
        let m = Arc::new(Mesh::decode(&s.cp, &list, &mem).expect("a mesh"));
        if m.is_empty() {
            continue;
        }
        lines += usize::from(m.runs.iter().any(|r| r.0 == Kind::Lines));
        points += usize::from(m.runs.iter().any(|r| r.0 == Kind::Points));
        let mut want = Vec::new();
        m.transform(&s, &normals, |_, o| want.push(o.clip));
        let got = meshes.clips(&device, &queue, &s, &m, &normals, 0);
        let bits = |v: &[[f32; 4]]| -> Vec<[u32; 4]> {
            v.iter()
                .map(|c| c.map(|x| if x.is_nan() { u32::MAX } else { x.to_bits() }))
                .collect()
        };
        assert_eq!(
            bits(&got),
            bits(&want),
            "round {round}: the clip-space positions"
        );
        clips_compared += want.len() as u64;

        let vp = Viewport::of(&s.xf);
        let (_, off) = Renderer::scissor(&s, &vp);
        let (want, of_lines) = cpu_words(&s, &m, &normals);
        let got = meshes.words(&device, &queue, &layout, &s, &m, &normals, off);
        assert_eq!(got.len(), want.len(), "round {round}: the words");
        let n = 6 + 3 * crate::texgens(&s);
        for ((a, b), &line) in want.chunks(n).zip(got.chunks(n)).zip(&of_lines) {
            compare(a, b, line, &mut c);
        }
    }
    println!("{clips_compared} positions; {c:?}; {lines} with lines, {points} with points");
    assert!(lines > 100 && points > 100, "lines and points reached");
    assert!(c.compared > 10_000, "{c:?}");
    assert_eq!(c.depths, 0, "depths: {c:?}");
    // Float noise only: sqrt and divisions (normals, lighting, a post-transform's
    // normalization) round otherwise on the GPU.
    let limit = c.compared / 200;
    assert!(
        c.positions <= limit && c.colors <= limit && c.tex <= limit,
        "{c:?}"
    );
    assert!(c.line_corners <= c.compared / 50, "{c:?}");
}
