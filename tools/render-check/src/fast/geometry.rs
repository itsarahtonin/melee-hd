//! The harness's geometry: each draw's transformed vertices as a renderer reports them, keyed
//! by the display pass's draw they came from, and the comparison of two frames' worth within a
//! tolerance.
//!
//! A renderer reports clip-space positions (what a vertex shader outputs) and the viewport
//! they're mapped to the screen by, as triangles, lines and points in the order GX expands its
//! primitives (melee-gpu's: quads as (0, 1, 2) and (0, 2, 3), strips alternating their winding,
//! fans around their first vertex), whatever primitives it draws them with. Each draw names
//! the display pass's draw it came from (its index in the pass's list, `Tick::draws`); the
//! exact renderer, which sends several of the pass's draws at once (a joint's shapes in a
//! pass), names the first, and [`compare`] keys the other renderer's draws the same way.

use melee_gx::vertex::{self, NormalCache};
use melee_gx::xform::{self, Viewport};
use melee_gx::{Draw, Memory, State};
use melee_render::gpu::{Mesh, MeshKind};
use std::cell::{Cell, RefCell};

/// What a run of vertices draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Triangles,
    Lines,
    Points,
}

impl Kind {
    fn per(self) -> usize {
        match self {
            Kind::Triangles => 3,
            Kind::Lines => 2,
            Kind::Points => 1,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Triangles => "triangles",
            Kind::Lines => "lines",
            Kind::Points => "points",
        }
    }
}

/// One draw's vertices.
#[derive(Clone, Debug)]
pub struct DrawGeometry {
    /// The display pass's draw it came from.
    pub source: u32,
    pub kind: Kind,
    pub viewport: Viewport,
    /// Clip-space positions, [`Kind::per`] an element.
    pub clip: Vec<[f32; 4]>,
    /// Whether its color shows its material color ([`reads_material_color`]): the
    /// reference's, for picking a draw whose material color shows.
    pub material_color: bool,
}

/// A frame's draws' vertices.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub draws: Vec<DrawGeometry>,
    /// The display pass's draws a reference names its draws by, in order: each first of what it
    /// sends at once. A renderer under test that names another of them is keyed by the one
    /// before it here. Empty for a renderer under test.
    pub groups: Vec<u32>,
}

/// Expands a GX primitive's `n` vertices into elements: calls `out` with each element's
/// vertex indices.
pub fn expand(primitive: u8, n: usize, mut out: impl FnMut(&[usize])) {
    match primitive {
        0x80 | 0x88 => {
            let mut i = 3;
            while i < n {
                out(&[i - 3, i - 2, i - 1]);
                out(&[i - 3, i - 1, i]);
                i += 4;
            }
            if i == n {
                out(&[n - 3, n - 2, n - 1]);
            }
        }
        0x90 => {
            let mut i = 2;
            while i < n {
                out(&[i - 2, i - 1, i]);
                i += 3;
            }
        }
        0x98 => {
            for i in 2..n {
                if i % 2 == 0 {
                    out(&[i - 2, i - 1, i]);
                } else {
                    out(&[i - 2, i, i - 1]);
                }
            }
        }
        0xA0 => {
            for i in 2..n {
                out(&[0, i - 1, i]);
            }
        }
        0xA8 => {
            for k in 0..n / 2 {
                out(&[2 * k, 2 * k + 1]);
            }
        }
        0xB0 => {
            for k in 0..n.saturating_sub(1) {
                out(&[k, k + 1]);
            }
        }
        _ => {
            for i in 0..n {
                out(&[i]);
            }
        }
    }
}

/// XF's count of color channels.
const NUM_CHANS: usize = 0x1009;

/// Whether a draw's color shows its material color: color channel 0 takes the material color
/// register, and a TEV stage rasterizing that channel reads its color.
fn reads_material_color(state: &State) -> bool {
    if state.xf[NUM_CHANS] & 3 == 0 || state.xf[xform::COLOR_CHAN] & 1 != 0 {
        return false;
    }
    let bits = |v: u32, at: u32, n: u32| (v >> at) & ((1 << n) - 1);
    let stages = bits(state.bp[0x00], 10, 4) as usize + 1;
    (0..stages).any(|s| {
        let tref = state.bp[0x28 + s / 2];
        let chan = if s % 2 == 0 {
            bits(tref, 7, 3)
        } else {
            bits(tref, 19, 3)
        };
        let cc = state.bp[0xC0 + 2 * s];
        chan == 0
            && [12, 8, 4, 0]
                .iter()
                .any(|&at| matches!(bits(cc, at, 4), 10 | 11))
    })
}

fn kind_of(primitive: u8) -> Kind {
    match primitive {
        0x80..=0xA7 => Kind::Triangles,
        0xA8..=0xB7 => Kind::Lines,
        _ => Kind::Points,
    }
}

/// The display pass's draw the stream's command at `offset` came from, by the stream's marks
/// (`melee_render::frame::Frames::marks`): the last starting at or before it.
pub fn source_at(marks: &[(u32, u32)], offset: usize) -> u32 {
    let i = marks.partition_point(|&(at, _)| at as usize <= offset);
    if i == 0 { 0 } else { marks[i - 1].1 }
}

/// Records each GX draw's transformed vertices as the exact renderer draws it, keyed by the
/// display pass's draw it came from: as an observer of melee-render's stream (it follows the
/// stream's offset by its commands), or fed by a host that knows the source.
pub struct Recorder<'a> {
    marks: &'a [(u32, u32)],
    offset: Cell<usize>,
    normals: RefCell<NormalCache>,
    inputs: RefCell<Vec<vertex::Input>>,
    pub draws: RefCell<Vec<DrawGeometry>>,
}

impl<'a> Recorder<'a> {
    pub fn new(marks: &'a [(u32, u32)]) -> Self {
        Recorder {
            marks,
            offset: Cell::new(0),
            normals: RefCell::new([[0.0; 3]; 3]),
            inputs: RefCell::new(Vec::new()),
            draws: RefCell::new(Vec::new()),
        }
    }

    /// The display pass's draw the stream's next command comes from.
    pub fn source(&self) -> u32 {
        source_at(self.marks, self.offset.get())
    }

    /// A command of the stream (not of a display list it calls) has run.
    pub fn command(&self, bytes: &[u8]) {
        self.offset.set(self.offset.get() + bytes.len());
    }

    /// Records a draw, its vertices fetched, from the display pass's draw `source`.
    pub fn record(&self, source: u32, state: &State, draw: &Draw<'_>) {
        assert!(
            draw.fetched,
            "the geometry needs the draws' vertices fetched"
        );
        let mut inputs = self.inputs.borrow_mut();
        inputs.clear();
        let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
        vertex::decode(
            &state.cp,
            xf_index,
            usize::from(draw.vat),
            draw.data,
            usize::from(draw.count),
            &mut self.normals.borrow_mut(),
            &mut inputs,
        );
        let setup = xform::Setup::new(&state.xf, &state.bp);
        let mut o = xform::Output::default();
        let clips: Vec<[f32; 4]> = inputs
            .iter()
            .map(|v| {
                setup.transform(v, &mut o);
                o.clip
            })
            .collect();
        let mut clip = Vec::with_capacity(clips.len() * 3);
        expand(draw.primitive, clips.len(), |e| {
            clip.extend(e.iter().map(|&i| clips[i]));
        });
        self.draws.borrow_mut().push(DrawGeometry {
            source,
            kind: kind_of(draw.primitive),
            viewport: Viewport::of(&state.xf),
            clip,
            material_color: reads_material_color(state),
        });
    }

    /// Records a display list drawn from its mesh, from the display pass's draw `source`: its
    /// vertices as the GPU's transforms made them (`gpu`, F3), or as the mesh transforms them on
    /// the CPU (`Mesh::transform`, as F2's renderer does), its elements in the list's order and
    /// GX's.
    pub fn record_mesh(&self, source: u32, state: &State, mesh: &Mesh, gpu: Option<&[[f32; 4]]>) {
        let mut normals = self.normals.borrow_mut();
        let clips = match gpu {
            Some(c) => c.to_vec(),
            None => {
                let mut clips = vec![[0.0; 4]; mesh.len()];
                mesh.transform(state, &normals, |i, o| clips[i] = o.clip);
                clips
            }
        };
        mesh.leave_normals(&mut normals);
        let mut draws = self.draws.borrow_mut();
        for (kind, run) in &mesh.runs {
            let els = &mesh.elements[run.start as usize..run.end as usize];
            draws.push(DrawGeometry {
                source,
                kind: match kind {
                    MeshKind::Triangles => Kind::Triangles,
                    MeshKind::Lines => Kind::Lines,
                    MeshKind::Points => Kind::Points,
                },
                viewport: Viewport::of(&state.xf),
                clip: els.iter().map(|&i| clips[i as usize]).collect(),
                material_color: reads_material_color(state),
            });
        }
    }

    pub fn into_geometry(self) -> Geometry {
        let groups = self.marks.iter().map(|&(_, s)| s).collect();
        Geometry {
            draws: self.draws.into_inner(),
            groups,
        }
    }
}

impl melee_render::gpu::Observer for Recorder<'_> {
    fn draw(&self, state: &State, draw: &Draw<'_>, _mem: &dyn Memory) {
        self.record(self.source(), state, draw);
    }

    fn copy(&self, _state: &State, _value: u32) {}

    fn command(&self, bytes: &[u8]) {
        Recorder::command(self, bytes);
    }

    fn mesh(&self, state: &State, mesh: &Mesh, clips: Option<&[[f32; 4]]>) {
        self.record_mesh(self.source(), state, mesh, clips);
    }
}

/// How far apart two positions may be. On screen, in the console's pixels, wherever the
/// vertex is in front of the eye: a renderer's own rounding to the rasterizer's sixteenths
/// moves a vertex up to 1/32 of a pixel each way, float maths in another order a few ulps, both
/// far below this; a joint's matrix or a draw's vertices wrong move them by whole pixels.
pub const POSITION_PX: f32 = 0.125;
/// The depth's tolerance, in the EFB's 24-bit units, where the vertex is in front of the eye:
/// 1/16384 of the range.
pub const DEPTH: f32 = 1024.0;
/// For a vertex near the eye's plane or behind it, whose screen position isn't stable, the
/// largest angle between the two positions' directions from the eye (radians, in clip space's
/// x, y and w).
pub const ANGLE: f64 = 1e-4;

/// A vertex's difference, as a multiple of its tolerance (above 1 is out of it), with what
/// was measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexError {
    pub ratio: f64,
    /// Pixels apart on screen, depth units apart, or radians apart for a vertex near the eye.
    pub px: f64,
    pub depth: f64,
    pub angle: f64,
    /// Where the reference's vertex is on screen (in front of the eye).
    pub at: Option<[f32; 2]>,
    /// Whether one is inside the depth range and the other outside: the GPU clips there, so
    /// a draw lying on the range's edge (an erase quad on the near plane) is drawn by one and
    /// not the other however small the difference. Noted, not out of tolerance by itself: a
    /// triangle crossing the edge loses only a sliver; the image measures what it changed.
    pub crossed: bool,
}

impl VertexError {
    fn text(&self) -> String {
        match self.at {
            Some(at) => format!(
                "{:.4} px (depth {:.1}) at ({:.1}, {:.1})",
                self.px, self.depth, at[0], at[1]
            ),
            None => format!("{:.2e} rad, near or behind the eye", self.angle),
        }
    }
}

/// Whether a clip-space position is inside the depth range GX clips to (z from -w to 0).
fn in_depth_range(c: [f32; 4]) -> bool {
    -c[3] <= c[2] && c[2] <= 0.0
}

/// Whether a clip-space position is in front of the eye, clear of its plane: then its screen
/// position is stable to compare.
fn in_front(c: [f32; 4]) -> bool {
    let n = c[0].abs().max(c[1].abs()).max(c[3].abs());
    c[3] > 1e-3 * n && c.iter().all(|v| v.is_finite())
}

fn same_viewport(a: &Viewport, b: &Viewport) -> bool {
    let bits =
        |v: &Viewport| [v.wd, v.ht, v.z_range, v.x_orig, v.y_orig, v.far_z].map(f32::to_bits);
    bits(a) == bits(b)
}

/// How far apart `test` (mapped to the screen by `vp_test`) is from `reference` (by `vp`).
pub fn vertex_error(
    reference: [f32; 4],
    test: [f32; 4],
    vp: &Viewport,
    vp_test: &Viewport,
) -> VertexError {
    let none = VertexError {
        ratio: 0.0,
        px: 0.0,
        depth: 0.0,
        angle: 0.0,
        at: None,
        crossed: false,
    };
    if reference.map(f32::to_bits) == test.map(f32::to_bits) && same_viewport(vp, vp_test) {
        return none;
    }
    let none = VertexError {
        crossed: in_depth_range(reference) != in_depth_range(test),
        ..none
    };
    if in_front(reference) && in_front(test) {
        let (a, b) = (vp.screen(reference), vp_test.screen(test));
        // Off screen, a vertex k half-diagonals from the middle turns its edges' parts on
        // screen k times less than it moves: the tolerance grows with it.
        let half = f64::from(vp.wd).hypot(f64::from(vp.ht)).max(1.0);
        let from_middle = f64::from(a[0] - vp.x_orig).hypot(f64::from(a[1] - vp.y_orig));
        let k = (from_middle / half).max(1.0);
        let px = f64::from(a[0] - b[0]).hypot(f64::from(a[1] - b[1]));
        let depth = f64::from((a[2] - b[2]).abs());
        let ratio = (px / (f64::from(POSITION_PX) * k)).max(depth / (f64::from(DEPTH) * k));
        return VertexError {
            ratio,
            px,
            depth,
            at: Some([a[0], a[1]]),
            ..none
        };
    }
    let dir = |c: [f32; 4]| {
        let v = [f64::from(c[0]), f64::from(c[1]), f64::from(c[3])];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|x| x / n)
    };
    let (p, q) = (dir(reference), dir(test));
    let angle = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
    let angle = if angle.is_finite() {
        angle
    } else {
        f64::INFINITY
    };
    VertexError {
        ratio: angle / ANGLE,
        angle,
        ..none
    }
}

/// One display draw's elements, as either renderer sent them: its draws' runs joined.
#[derive(Debug)]
struct Group<'a> {
    source: u32,
    /// The draws, in order.
    draws: Vec<&'a DrawGeometry>,
}

/// Groups a renderer's draws by the display draw they came from, each run of the same source
/// one group, keyed by `key`.
fn groups<'a>(g: &'a Geometry, key: impl Fn(u32) -> u32) -> Vec<Group<'a>> {
    let mut out: Vec<Group<'a>> = Vec::new();
    for d in &g.draws {
        let s = key(d.source);
        match out.last_mut() {
            Some(last) if last.source == s => last.draws.push(d),
            _ => out.push(Group {
                source: s,
                draws: vec![d],
            }),
        }
    }
    out
}

/// What the comparison of two frames' geometry found.
#[derive(Debug, Default)]
pub struct GeometryReport {
    /// Display draws the reference drew, and their vertices.
    pub draws: usize,
    pub vertices: usize,
    /// Whether every position is the same, bit for bit, in the same draws and order.
    pub identical: bool,
    /// Display draws the reference drew and the renderer under test didn't, and the reverse.
    pub missing: Vec<u32>,
    pub extra: Vec<u32>,
    /// Display draws both drew, drawn out of the reference's order.
    pub out_of_order: Vec<u32>,
    /// Display draws whose elements differ in kind or number: the source, the reference's and
    /// the other's.
    pub shape: Vec<(u32, String, String)>,
    /// Display draws with vertices out of tolerance: the source, how many, and the worst.
    pub over: Vec<(u32, usize, VertexError)>,
    /// Display draws with vertices inside the depth range in one and outside in the other: the
    /// source and how many ([`VertexError::crossed`]).
    pub crossed: Vec<(u32, usize)>,
    /// The worst vertex of all and its display draw.
    pub worst: Option<(u32, VertexError)>,
}

impl GeometryReport {
    pub fn within(&self) -> bool {
        self.missing.is_empty()
            && self.extra.is_empty()
            && self.out_of_order.is_empty()
            && self.shape.is_empty()
            && self.over.is_empty()
    }

    /// The report's lines: its summary, then a line for each finding (the first few of each).
    pub fn lines(&self, describe: &dyn Fn(u32) -> String) -> Vec<String> {
        let mut out = Vec::new();
        let worst = match self.worst {
            Some((s, e)) if e.ratio > 0.0 => format!(
                ", worst vertex {:.3}x the tolerance ({}) in display draw {s}",
                e.ratio,
                e.text()
            ),
            _ => String::new(),
        };
        out.push(format!(
            "geometry: {} display draws, {} vertices: {}{worst}",
            self.draws,
            self.vertices,
            if self.identical {
                "identical"
            } else if self.within() {
                "within tolerance"
            } else {
                "OUT OF TOLERANCE"
            }
        ));
        const SHOWN: usize = 8;
        let list = |out: &mut Vec<String>, what: &str, v: &[u32]| {
            for &s in v.iter().take(SHOWN) {
                out.push(format!("  {what}: display draw {s}, {}", describe(s)));
            }
            if v.len() > SHOWN {
                out.push(format!("  ... {} more {what}", v.len() - SHOWN));
            }
        };
        list(&mut out, "missing", &self.missing);
        list(&mut out, "extra", &self.extra);
        list(&mut out, "out of order", &self.out_of_order);
        for (s, a, b) in self.shape.iter().take(SHOWN) {
            out.push(format!(
                "  elements differ: display draw {s}, {}: {a} in the reference, {b} here",
                describe(*s)
            ));
        }
        for (s, n, e) in self.over.iter().take(SHOWN) {
            out.push(format!(
                "  moved: display draw {s}, {}: {n} vertices out of tolerance, the worst {}",
                describe(*s),
                e.text()
            ));
        }
        if self.over.len() > SHOWN {
            out.push(format!(
                "  ... {} more display draws moved",
                self.over.len() - SHOWN
            ));
        }
        for (s, n) in self.crossed.iter().take(SHOWN) {
            out.push(format!(
                "  note: display draw {s}, {}: {n} vertices on the other side of the depth range's edge, where the GPU clips",
                describe(*s)
            ));
        }
        out
    }
}

/// The positions of `rest` whose values form the longest increasing run (patience sorting):
/// the draws both drew in the reference's order.
fn longest_increasing(v: &[usize]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut prev = vec![usize::MAX; v.len()];
    for (i, &x) in v.iter().enumerate() {
        let k = tails.partition_point(|&t| v[t] < x);
        if k > 0 {
            prev[i] = tails[k - 1];
        }
        if k == tails.len() {
            tails.push(i);
        } else {
            tails[k] = i;
        }
    }
    let mut keep = vec![false; v.len()];
    let mut at = tails.last().copied();
    while let Some(i) = at {
        keep[i] = true;
        at = (prev[i] != usize::MAX).then(|| prev[i]);
    }
    keep
}

/// Compares the geometry of a renderer under test with the reference's.
pub fn compare(reference: &Geometry, test: &Geometry) -> GeometryReport {
    let groups_ref = groups(reference, |s| s);
    let starts = &reference.groups;
    let key = |s: u32| {
        let i = starts.partition_point(|&g| g <= s);
        if i == 0 { s } else { starts[i - 1] }
    };
    let groups_test = groups(test, key);
    let mut r = GeometryReport {
        draws: groups_ref.len(),
        vertices: reference.draws.iter().map(|d| d.clip.len()).sum(),
        ..Default::default()
    };
    // Each group by its source and its run's number (a source drawn in two runs is two).
    let keyed = |gs: &[Group<'_>]| -> Vec<(u32, u32)> {
        let mut seen = std::collections::HashMap::<u32, u32>::new();
        gs.iter()
            .map(|g| {
                let n = seen.entry(g.source).or_default();
                *n += 1;
                (g.source, *n)
            })
            .collect()
    };
    let (kr, kt) = (keyed(&groups_ref), keyed(&groups_test));
    let at_ref: std::collections::HashMap<(u32, u32), usize> =
        kr.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    let at_test: std::collections::HashMap<(u32, u32), usize> =
        kt.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    r.missing = kr
        .iter()
        .filter(|k| !at_test.contains_key(k))
        .map(|k| k.0)
        .collect();
    r.extra = kt
        .iter()
        .filter(|k| !at_ref.contains_key(k))
        .map(|k| k.0)
        .collect();
    let both: Vec<(usize, usize)> = kt
        .iter()
        .enumerate()
        .filter_map(|(j, k)| at_ref.get(k).map(|&i| (i, j)))
        .collect();
    let keep = longest_increasing(&both.iter().map(|p| p.0).collect::<Vec<_>>());
    r.out_of_order = both
        .iter()
        .zip(&keep)
        .filter(|(_, k)| !**k)
        .map(|(p, _)| kr[p.0].0)
        .collect();
    let mut identical = r.missing.is_empty() && r.extra.is_empty() && r.out_of_order.is_empty();
    for &(i, j) in &both {
        let (gr, gt) = (&groups_ref[i], &groups_test[j]);
        let elements = |g: &Group<'_>| -> Vec<(Kind, usize)> {
            let mut v: Vec<(Kind, usize)> = Vec::new();
            for d in &g.draws {
                let n = d.clip.len() / d.kind.per();
                match v.last_mut() {
                    Some((k, m)) if *k == d.kind => *m += n,
                    _ => v.push((d.kind, n)),
                }
            }
            v
        };
        let (er, et) = (elements(gr), elements(gt));
        if er != et {
            let text = |e: &[(Kind, usize)]| {
                e.iter()
                    .map(|(k, n)| format!("{n} {}", k.name()))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            r.shape.push((gr.source, text(&er), text(&et)));
            identical = false;
            continue;
        }
        // The vertices in order, each with its draw's viewport.
        let vertices = |g: &Group<'_>| {
            g.draws
                .iter()
                .flat_map(|d| d.clip.iter().map(move |c| (*c, d.viewport)))
                .collect::<Vec<_>>()
        };
        let (mut n, mut crossed, mut worst): (usize, usize, Option<VertexError>) = (0, 0, None);
        for ((a, vp), (b, vp_test)) in vertices(gr).into_iter().zip(vertices(gt)) {
            let e = vertex_error(a, b, &vp, &vp_test);
            if e.ratio > 0.0 {
                identical = false;
            }
            crossed += usize::from(e.crossed);
            if e.ratio > 1.0 {
                n += 1;
            }
            if worst.is_none_or(|w| e.ratio > w.ratio) {
                worst = Some(e);
            }
        }
        if crossed > 0 {
            r.crossed.push((gr.source, crossed));
        }
        if let Some(w) = worst {
            if r.worst.is_none_or(|(_, x)| w.ratio > x.ratio) {
                r.worst = Some((gr.source, w));
            }
            if n > 0 {
                r.over.push((gr.source, n, w));
            }
        }
    }
    r.identical = identical;
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> Viewport {
        // The console's 640 by 480 at the scissor's offset, as HSD sets it.
        Viewport {
            wd: 320.0,
            ht: -240.0,
            z_range: 16_777_215.0,
            x_orig: 662.0,
            y_orig: 582.0,
            far_z: 16_777_215.0,
        }
    }

    fn tri(source: u32, at: [f32; 2]) -> DrawGeometry {
        let c = |x: f32, y: f32| [x, y, -0.5, 1.0];
        DrawGeometry {
            source,
            kind: Kind::Triangles,
            viewport: vp(),
            clip: vec![
                c(at[0], at[1]),
                c(at[0] + 0.1, at[1]),
                c(at[0], at[1] + 0.1),
            ],
            material_color: false,
        }
    }

    fn frame(draws: Vec<DrawGeometry>) -> Geometry {
        let groups = draws.iter().map(|d| d.source).collect();
        Geometry { draws, groups }
    }

    #[test]
    fn expands_as_melee_gpu_does() {
        let run = |p: u8, n: usize| {
            let mut v = Vec::new();
            expand(p, n, |e| v.push(e.to_vec()));
            v
        };
        assert_eq!(run(0x80, 4), [vec![0, 1, 2], vec![0, 2, 3]]);
        assert_eq!(run(0x98, 5), [vec![0, 1, 2], vec![1, 3, 2], vec![2, 3, 4]]);
        assert_eq!(run(0xA0, 4), [vec![0, 1, 2], vec![0, 2, 3]]);
        assert_eq!(run(0x90, 6), [vec![0, 1, 2], vec![3, 4, 5]]);
        assert_eq!(run(0xB0, 3), [vec![0, 1], vec![1, 2]]);
        assert_eq!(run(0xA8, 4), [vec![0, 1], vec![2, 3]]);
        assert_eq!(run(0xB8, 2), [vec![0], vec![1]]);
    }

    #[test]
    fn sources_by_marks() {
        let marks = [(10, 0), (40, 3), (40, 4), (90, 9)];
        assert_eq!(source_at(&marks, 5), 0);
        assert_eq!(source_at(&marks, 10), 0);
        assert_eq!(source_at(&marks, 39), 0);
        // Draw 3 sent nothing: what starts at 40 is draw 4's.
        assert_eq!(source_at(&marks, 40), 4);
        assert_eq!(source_at(&marks, 1000), 9);
    }

    #[test]
    fn identical_frames() {
        let a = frame(vec![tri(0, [0.0, 0.0]), tri(5, [0.2, 0.2])]);
        let r = compare(&a, &a.clone());
        assert!(r.identical && r.within(), "{r:?}");
    }

    #[test]
    fn a_draw_missing_extra_or_out_of_order() {
        let a = frame(vec![
            tri(0, [0.0, 0.0]),
            tri(5, [0.2, 0.2]),
            tri(7, [0.4, 0.4]),
        ]);
        let mut b = a.clone();
        b.draws.remove(1);
        b.groups.clear();
        let r = compare(&a, &b);
        assert_eq!(r.missing, [5]);
        assert!(!r.within());
        let r = compare(&b, &a);
        assert_eq!(r.extra, [5]);
        let mut c = a.clone();
        c.draws.swap(1, 2);
        let r = compare(&a, &c);
        assert_eq!(r.out_of_order.len(), 1);
        assert!(r.missing.is_empty() && !r.within());
    }

    /// The renderer under test may name any of the display draws the reference sends at once.
    #[test]
    fn keys_by_the_reference_s_groups() {
        let mut a = frame(vec![tri(0, [0.0, 0.0]), tri(5, [0.2, 0.2])]);
        a.groups = vec![0, 5, 9];
        let mut b = a.clone();
        b.groups.clear();
        b.draws[1].source = 6;
        let r = compare(&a, &b);
        assert!(r.identical, "{r:?}");
    }

    #[test]
    fn tolerances() {
        let v = vp();
        let a = [0.25, -0.5, -0.5, 1.0];
        // A sixteenth of a pixel: within.
        let b = [0.25 + 1.0 / 16.0 / 320.0, -0.5, -0.5, 1.0];
        let e = vertex_error(a, b, &v, &v);
        assert!(e.ratio < 1.0 && (e.px - 0.0625).abs() < 1e-3, "{e:?}");
        // Half a pixel: out.
        let b = [0.25 + 0.5 / 320.0, -0.5, -0.5, 1.0];
        assert!(vertex_error(a, b, &v, &v).ratio > 1.0);
        // The same move a hundred screens off: within.
        let far = [200.0, -0.5, -0.5, 1.0];
        let b = [200.0 + 0.5 / 320.0, -0.5, -0.5, 1.0];
        assert!(vertex_error(far, b, &v, &v).ratio < 1.0);
        // Depth: 1/16 of the range apart is out, a few units in.
        let b = [0.25, -0.5, -0.5 + 1.0 / 16.0, 1.0];
        assert!(vertex_error(a, b, &v, &v).ratio > 1.0);
        let b = [0.25, -0.5, -0.5 + 1e-6, 1.0];
        assert!(vertex_error(a, b, &v, &v).ratio < 1.0);
        // Behind the eye: by direction.
        let behind = [1.0, 2.0, 3.0, -0.0004];
        let e = vertex_error(behind, [1.0, 2.0, 3.0, -0.000_400_1], &v, &v);
        assert!(e.ratio < 1.0 && e.angle > 0.0, "{e:?}");
        let e = vertex_error(behind, [1.1, 2.0, 3.0, -0.0004], &v, &v);
        assert!(e.ratio > 1.0, "{e:?}");
        // The viewport moved a sixteenth: seen, within.
        let mut moved = v;
        moved.x_orig += 1.0 / 16.0;
        let e = vertex_error(a, a, &v, &moved);
        assert!(
            e.ratio > 0.0 && e.ratio < 1.0 && (e.px - 0.0625).abs() < 1e-3,
            "{e:?}"
        );
        // On the near plane and an ulp past it: within, but noted.
        let edge = [0.5, 0.5, -1.0, 1.0];
        let e = vertex_error(edge, [0.5, 0.5, -1.000_000_1, 1.0], &v, &v);
        assert!(e.crossed && e.ratio < 1.0, "{e:?}");
        assert!(!vertex_error(a, [0.25, -0.5, -0.5 + 1e-6, 1.0], &v, &v).crossed);
    }

    #[test]
    fn a_vertex_moved_and_elements_changed() {
        let a = frame(vec![tri(0, [0.0, 0.0]), tri(5, [0.2, 0.2])]);
        let mut b = a.clone();
        b.draws[1].clip[0][0] += 2.0 / 320.0;
        let r = compare(&a, &b);
        assert_eq!(r.over.len(), 1);
        assert_eq!(r.over[0].0, 5);
        assert!((r.over[0].2.px - 2.0).abs() < 1e-3);
        let mut c = a.clone();
        c.draws[1].clip.clear();
        let r = compare(&a, &c);
        assert_eq!(r.shape.len(), 1);
        assert!(!r.within());
    }

    #[test]
    fn longest_increasing_keeps_the_order() {
        assert_eq!(longest_increasing(&[0, 2, 1, 3]), [true, false, true, true]);
        assert_eq!(longest_increasing(&[]), Vec::<bool>::new());
    }
}
