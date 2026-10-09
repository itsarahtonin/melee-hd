//! melee-sim's hit collision geometry (`melee/lb/lbcollision.c`) against ssbm-rs's ports, bit
//! for bit, in both float modes: return values and everything written through pointers,
//! including what a rejected test leaves unwritten.

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{SCRATCH, arg, bits, handle, inputs, read_f32s, reference, write_f32s};
use melee_sim::lb::collision::{
    self, HitShape, SweptCircle, closest_on_segment, closest_on_segment_xy, hit_hurt,
    segment_sphere, sphere_entry,
};
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;
use ssbm_rt::{ArrV, Ctx, F32, Val};
use ssbm_types::fns;
use ssbm_types::records::{Fighter_x1614_t, HitCapsule, Vec as V};
// 80006E58 is static to its unit, so ssbm-types lists it there rather than in `fns`.
use ssbm_types::tu::melee__lb__lbcollision as lbcollision;

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];
const A: u32 = SCRATCH;
const B: u32 = SCRATCH + 0x10;
const C: u32 = SCRATCH + 0x20;
const D: u32 = SCRATCH + 0x30;
const OUT1: u32 = SCRATCH + 0x40;
const OUT2: u32 = SCRATCH + 0x50;
const OUT3: u32 = SCRATCH + 0x60;
const OUT4: u32 = SCRATCH + 0x70;
const MTX: u32 = SCRATCH + 0x80;
const MTX_OUT: u32 = SCRATCH + 0xC0;
const SHAPE1: u32 = SCRATCH + 0x100;
const SHAPE2: u32 = SCRATCH + 0x300;
/// What output memory holds before a call, so an unwritten output shows.
const UNWRITTEN: u32 = 0xDEAD_BEEF;

fn put(ctx: &Ctx, addr: u32, v: Vec3) {
    write_f32s(ctx, addr, &[v.x, v.y, v.z]);
}

fn got(ctx: &Ctx, addr: u32) -> Vec<u32> {
    read_f32s(ctx, addr, 3)
}

fn b3(v: Vec3) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn clear(ctx: &Ctx, addr: u32, words: usize) {
    for i in 0..words as u32 {
        ctx.write_u32(addr + 4 * i, UNWRITTEN);
    }
}

fn v<'a>(ctx: &'a Ctx, addr: u32) -> V<'a> {
    handle(ctx, addr)
}

fn f<'a>(ctx: &'a Ctx, addr: u32) -> Val<'a, F32> {
    handle(ctx, addr)
}

fn m<'a>(ctx: &'a Ctx, addr: u32) -> ArrV<'a, F32, 4> {
    handle(ctx, addr)
}

fn put_mtx(ctx: &Ctx, addr: u32, mtx: &Mtx34) {
    write_f32s(ctx, addr, mtx.m.as_flattened());
}

fn mtx_bits(mtx: &Mtx34) -> Vec<u32> {
    mtx.m.as_flattened().iter().map(|x| x.to_bits()).collect()
}

/// Geometry for the tests: random scenes, plus the cases the branches turn on: segments that
/// touch, just miss or are parallel, zero-length segments, coincident points, values on a
/// coarse grid that give exact ties and zero determinants, and special values.
struct Gen {
    s: u64,
    special: Vec<f32>,
}

/// Two segments and two radii.
#[derive(Clone, Copy, Debug)]
struct Scene {
    a0: Vec3,
    a1: Vec3,
    b0: Vec3,
    b1: Vec3,
    r0: f32,
    r1: f32,
}

impl Gen {
    fn new(seed: u64) -> Self {
        Gen {
            s: seed,
            special: inputs(100.0, 50_000),
        }
    }

    fn next(&mut self) -> u64 {
        self.s ^= self.s << 13;
        self.s ^= self.s >> 7;
        self.s ^= self.s << 17;
        self.s
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn signed(&mut self, range: f32) -> f32 {
        (self.unit() * 2.0 - 1.0) * range
    }

    fn special(&mut self) -> f32 {
        let i = self.below(self.special.len() as u64) as usize;
        self.special[i]
    }

    /// A coordinate: mostly in a stage-sized range, sometimes on a grid, tiny or special.
    fn coord(&mut self) -> f32 {
        match self.below(20) {
            0..=13 => self.signed(80.0),
            14 | 15 => (self.below(17) as f32 - 8.0) * 0.5,
            16 => self.signed(1.0e-3),
            17 => 0.0,
            _ => self.special(),
        }
    }

    fn vec(&mut self) -> Vec3 {
        Vec3::new(self.coord(), self.coord(), self.coord())
    }

    fn radius(&mut self) -> f32 {
        match self.below(20) {
            0..=15 => self.unit() * 15.0,
            16 => 0.0,
            17 => (self.below(8) as f32) * 0.5,
            18 => self.signed(1.0e-3),
            _ => self.special(),
        }
    }

    fn near(&mut self, p: Vec3, spread: f32) -> Vec3 {
        Vec3::new(
            p.x + self.signed(spread),
            p.y + self.signed(spread),
            p.z + self.signed(spread),
        )
    }

    /// A unit vector perpendicular to `d` (any unit vector when `d` is zero).
    fn perpendicular(&mut self, d: Vec3) -> Vec3 {
        let r = Vec3::new(self.signed(1.0), self.signed(1.0), self.signed(1.0));
        let c = Vec3::new(
            d.y * r.z - d.z * r.y,
            d.z * r.x - d.x * r.z,
            d.x * r.y - d.y * r.x,
        );
        let c = if c.x == 0.0 && c.y == 0.0 && c.z == 0.0 {
            r
        } else {
            c
        };
        let len = (c.x * c.x + c.y * c.y + c.z * c.z).sqrt();
        Vec3::new(c.x / len, c.y / len, c.z / len)
    }

    fn scene(&mut self) -> Scene {
        let (r0, r1) = (self.radius(), self.radius());
        let a0 = self.vec();
        let short = self.unit() * 30.0;
        let mut s = Scene {
            a0,
            a1: self.near(a0, short),
            b0: self.near(a0, 40.0),
            b1: self.vec(),
            r0,
            r1,
        };
        s.b1 = self.near(s.b0, short);
        match self.below(12) {
            // Independent points anywhere.
            0 => {
                s.a1 = self.vec();
                s.b0 = self.vec();
                s.b1 = self.vec();
            }
            // Second segment at about the radii's sum from the first: touching or just missing.
            1 | 2 => {
                let d = Vec3::new(s.a1.x - a0.x, s.a1.y - a0.y, s.a1.z - a0.z);
                let n = self.perpendicular(d);
                let gap = (r0 + r1) * (1.0 + self.signed(1.0e-5));
                let t = self.unit();
                let base = Vec3::new(
                    a0.x + d.x * t + n.x * gap,
                    a0.y + d.y * t + n.y * gap,
                    a0.z + d.z * t + n.z * gap,
                );
                let e = self.perpendicular(n);
                let half = self.unit() * 10.0;
                s.b0 = Vec3::new(
                    base.x - e.x * half,
                    base.y - e.y * half,
                    base.z - e.z * half,
                );
                s.b1 = Vec3::new(
                    base.x + e.x * half,
                    base.y + e.y * half,
                    base.z + e.z * half,
                );
            }
            // Parallel, or on the same line.
            3 | 4 => {
                let off = if self.below(3) == 0 {
                    Vec3::ZERO
                } else {
                    Vec3::new(self.signed(5.0), self.signed(5.0), self.signed(5.0))
                };
                let (t0, t1) = (self.signed(2.0), self.signed(2.0));
                let d = Vec3::new(s.a1.x - a0.x, s.a1.y - a0.y, s.a1.z - a0.z);
                s.b0 = Vec3::new(
                    a0.x + d.x * t0 + off.x,
                    a0.y + d.y * t0 + off.y,
                    a0.z + d.z * t0 + off.z,
                );
                s.b1 = Vec3::new(
                    a0.x + d.x * t1 + off.x,
                    a0.y + d.y * t1 + off.y,
                    a0.z + d.z * t1 + off.z,
                );
            }
            // Zero-length segments and coincident points.
            5 => s.a1 = a0,
            6 => s.b1 = s.b0,
            7 => {
                s.a1 = a0;
                s.b1 = s.b0;
                if self.below(2) == 0 {
                    s.b0 = a0;
                    s.b1 = a0;
                }
            }
            8 => {
                s.b0 = a0;
                if self.below(2) == 0 {
                    s.b1 = s.a1;
                }
            }
            // Segments shorter than the near-zero bound.
            9 => {
                s.a1 = self.near(a0, 1.0e-3);
                s.b1 = self.near(s.b0, 1.0e-3);
            }
            // A coarse grid, for exact ties.
            10 => {
                let grid = |g: &mut Gen| {
                    Vec3::new(
                        g.below(5) as f32 - 2.0,
                        g.below(5) as f32 - 2.0,
                        g.below(5) as f32 - 2.0,
                    )
                };
                s.a0 = grid(self);
                s.a1 = grid(self);
                s.b0 = grid(self);
                s.b1 = grid(self);
                s.r0 = self.below(4) as f32 * 0.5;
                s.r1 = self.below(4) as f32 * 0.5;
            }
            // Special values anywhere.
            _ => {
                let sp = |g: &mut Gen| Vec3::new(g.special(), g.special(), g.special());
                match self.below(4) {
                    0 => s.a0 = sp(self),
                    1 => s.a1 = sp(self),
                    2 => s.b0 = sp(self),
                    _ => s.b1 = sp(self),
                }
                if self.below(2) == 0 {
                    s.r0 = self.special();
                }
            }
        }
        s
    }

    /// A bone's matrix: a rotation, scale and translation, or the identity, a singular or
    /// nearly singular matrix, random floats or special values.
    fn mtx(&mut self) -> Mtx34 {
        let mut out = Mtx34::IDENTITY;
        match self.below(10) {
            0 => {}
            1 => {
                for row in &mut out.m {
                    for x in row {
                        *x = self.coord();
                    }
                }
            }
            2 => {
                let tiny = self.signed(1.0e-4);
                out = self.rotation(tiny);
            }
            3 => {
                out = self.rotation(1.0);
                let r = self.below(3) as usize;
                out.m[r] = [0.0, 0.0, 0.0, out.m[r][3]];
            }
            4 => {
                out = self.rotation(1.0);
                let (i, j) = (self.below(3) as usize, self.below(4) as usize);
                out.m[i][j] = self.special();
            }
            _ => {
                let scale = 0.2 + self.unit() * 3.0;
                out = self.rotation(scale);
            }
        }
        out
    }

    fn rotation(&mut self, scale: f32) -> Mtx34 {
        let (a, b, c) = (self.signed(3.2), self.signed(3.2), self.signed(3.2));
        let (sa, ca, sb, cb, sc, cc) = (a.sin(), a.cos(), b.sin(), b.cos(), c.sin(), c.cos());
        let s = [
            scale * (0.5 + self.unit()),
            scale * (0.5 + self.unit()),
            scale * (0.5 + self.unit()),
        ];
        let rot = [
            [cb * cc, sa * sb * cc - ca * sc, ca * sb * cc + sa * sc],
            [cb * sc, sa * sb * sc + ca * cc, ca * sb * sc - sa * cc],
            [-sb, sa * cb, ca * cb],
        ];
        let mut out = Mtx34::default();
        for (row, rot) in out.m.iter_mut().zip(rot) {
            for ((x, r), s) in row.iter_mut().zip(rot).zip(s) {
                *x = r * s;
            }
            row[3] = self.coord();
        }
        out
    }
}

/// Runs `check` over `n` scenes in each float mode.
fn each_mode(n: usize, mut check: impl FnMut(&Ctx, &mut Gen, FpMode)) {
    let Some(ctx) = reference() else { return };
    for mode in MODES {
        set_fp_mode(mode);
        let mut g = Gen::new(0x2545_F491_4F6C_DD1D);
        for _ in 0..n {
            check(&ctx, &mut g, mode);
        }
    }
    set_fp_mode(FpMode::Hardware);
}

/// Counts of each outcome, so a test shows it reached every one.
#[derive(Default, Debug)]
struct Outcomes {
    rejected: usize,
    touching: usize,
    apart: usize,
}

impl Outcomes {
    fn add(&mut self, result: Option<bool>) {
        match result {
            None => self.rejected += 1,
            Some(true) => self.touching += 1,
            Some(false) => self.apart += 1,
        }
    }

    fn check(&self, name: &str) {
        eprintln!("{name}: {self:?}");
        if std::env::var("SSBM_DISC").is_ok() {
            assert!(
                self.rejected > 100 && self.touching > 100 && self.apart > 100,
                "{name} misses an outcome: {self:?}"
            );
        }
    }
}

#[test]
fn segment_point_tests_match() {
    let mut sphere = Outcomes::default();
    each_mode(60_000, |ctx, g, mode| {
        let s = g.scene();
        put(ctx, A, s.a0);
        put(ctx, B, s.a1);
        put(ctx, C, s.b0);

        clear(ctx, OUT1, 3);
        let hit = fns::lbColl_80005C44(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            v(ctx, OUT1),
            arg(s.r0),
            arg(s.r1),
        );
        let ours = segment_sphere(s.a0, s.a1, s.b0, s.r0, s.r1);
        sphere.add(ours.map(|o| o.touching));
        match ours {
            None => {
                assert_eq!(hit, 0, "80005C44 rejects {s:?} {mode:?}");
                assert_eq!(
                    got(ctx, OUT1),
                    vec![UNWRITTEN; 3],
                    "80005C44 {s:?} {mode:?}"
                );
            }
            Some(o) => {
                assert_eq!(hit, i32::from(o.touching), "80005C44 {s:?} {mode:?}");
                assert_eq!(
                    got(ctx, OUT1),
                    b3(o.closest),
                    "80005C44 point {s:?} {mode:?}"
                );
            }
        }

        clear(ctx, OUT1, 1);
        let d = bits(fns::lbColl_80005EBC(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            f(ctx, OUT1),
        ));
        let ours = closest_on_segment(s.a0, s.a1, s.b0);
        assert_eq!(ours.distance_sq.to_bits(), d, "80005EBC {s:?} {mode:?}");
        assert_eq!(
            ctx.read_u32(OUT1),
            ours.t.to_bits(),
            "80005EBC t {s:?} {mode:?}"
        );

        clear(ctx, OUT1, 1);
        let d = bits(fns::lbColl_80005FC0(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            f(ctx, OUT1),
        ));
        let ours = closest_on_segment_xy(s.a0, s.a1, s.b0);
        assert_eq!(ours.distance_sq.to_bits(), d, "80005FC0 {s:?} {mode:?}");
        assert_eq!(
            ctx.read_u32(OUT1),
            ours.t.to_bits(),
            "80005FC0 t {s:?} {mode:?}"
        );
    });
    sphere.check("80005C44");
}

#[test]
fn capsule_tests_match() {
    let (mut solid, mut flat) = (Outcomes::default(), Outcomes::default());
    each_mode(60_000, |ctx, g, mode| {
        let mut s = g.scene();
        put(ctx, A, s.a0);
        put(ctx, B, s.a1);
        put(ctx, C, s.b0);
        put(ctx, D, s.b1);

        clear(ctx, OUT1, 3);
        clear(ctx, OUT2, 3);
        let hit = fns::lbColl_80006094(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            v(ctx, D),
            v(ctx, OUT1),
            v(ctx, OUT2),
            arg(s.r0),
            arg(s.r1),
        );
        let ours = collision::capsules(s.a0, s.a1, s.b0, s.b1, s.r0, s.r1);
        solid.add(ours.map(|o| o.touching));
        compare_closest(ctx, hit, ours, "80006094", &s, mode);

        // The flat test mostly sees what the game gives it: points in the XY plane.
        if g.below(4) != 0 {
            for p in [&mut s.a0, &mut s.a1, &mut s.b0, &mut s.b1] {
                p.z = 0.0;
            }
            put(ctx, A, s.a0);
            put(ctx, B, s.a1);
            put(ctx, C, s.b0);
            put(ctx, D, s.b1);
        }
        clear(ctx, OUT1, 3);
        clear(ctx, OUT2, 3);
        let hit = fns::lbColl_800067F8(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            v(ctx, D),
            v(ctx, OUT1),
            v(ctx, OUT2),
            arg(s.r0),
            arg(s.r1),
        );
        let ours = collision::capsules_xy(s.a0, s.a1, s.b0, s.b1, s.r0, s.r1);
        flat.add(ours.map(|o| o.touching));
        compare_closest(ctx, hit, ours, "800067F8", &s, mode);
    });
    solid.check("80006094");
    flat.check("800067F8");
}

fn compare_closest(
    ctx: &Ctx,
    hit: i32,
    ours: Option<collision::ClosestPoints>,
    name: &str,
    s: &Scene,
    mode: FpMode,
) {
    match ours {
        None => {
            assert_eq!(hit, 0, "{name} rejects {s:?} {mode:?}");
            assert_eq!(got(ctx, OUT1), vec![UNWRITTEN; 3], "{name} {s:?} {mode:?}");
            assert_eq!(got(ctx, OUT2), vec![UNWRITTEN; 3], "{name} {s:?} {mode:?}");
        }
        Some(o) => {
            assert_eq!(hit, i32::from(o.touching), "{name} {s:?} {mode:?}");
            assert_eq!(
                got(ctx, OUT1),
                b3(o.on_first),
                "{name} first {s:?} {mode:?}"
            );
            assert_eq!(
                got(ctx, OUT2),
                b3(o.on_second),
                "{name} second {s:?} {mode:?}"
            );
        }
    }
}

#[test]
fn hit_hurt_test_matches() {
    let mut outcomes = Outcomes::default();
    let mut coincident = 0;
    each_mode(60_000, |ctx, g, mode| {
        let s = g.scene();
        let mtx = g.mtx();
        let scale = match g.below(4) {
            0 => 3.0,
            1 => 20.0,
            2 => g.radius(),
            _ => g.unit() * 3.0,
        };
        put(ctx, A, s.a0);
        put(ctx, B, s.a1);
        put(ctx, C, s.b0);
        put(ctx, D, s.b1);
        put_mtx(ctx, MTX, &mtx);
        for out in [OUT1, OUT2, OUT3, OUT4] {
            clear(ctx, out, 3);
        }
        let hit = lbcollision::lbColl_80006E58(
            ctx,
            v(ctx, A),
            v(ctx, B),
            v(ctx, C),
            v(ctx, D),
            v(ctx, OUT1),
            v(ctx, OUT2),
            m(ctx, MTX),
            v(ctx, OUT3),
            f(ctx, OUT4),
            arg(s.r0),
            arg(s.r1),
            arg(scale),
        );
        let ours = hit_hurt(s.a0, s.a1, s.b0, s.b1, &mtx, s.r0, s.r1, scale);
        outcomes.add(ours.map(|o| o.touching));
        let ctx_ = (&s, &mtx, scale, mode);
        match ours {
            None => {
                assert_eq!(hit, 0, "80006E58 rejects {ctx_:?}");
                for out in [OUT1, OUT2, OUT3, OUT4] {
                    assert_eq!(got(ctx, out), vec![UNWRITTEN; 3], "80006E58 {ctx_:?}");
                }
            }
            Some(o) => {
                if o.contact == o.hit_closest && o.touching {
                    coincident += 1;
                }
                assert_eq!(hit, i32::from(o.touching), "80006E58 {ctx_:?}");
                assert_eq!(got(ctx, OUT1), b3(o.hit_closest), "80006E58 hit {ctx_:?}");
                assert_eq!(got(ctx, OUT2), b3(o.hurt_closest), "80006E58 hurt {ctx_:?}");
                assert_eq!(got(ctx, OUT3), b3(o.contact), "80006E58 contact {ctx_:?}");
                assert_eq!(
                    ctx.read_u32(OUT4),
                    o.overlap.to_bits(),
                    "80006E58 overlap {ctx_:?}"
                );
            }
        }
    });
    outcomes.check("80006E58");
    eprintln!("80006E58 at zero distance: {coincident}");
}

#[test]
fn hsd_inverse_matches() {
    each_mode(60_000, |ctx, g, mode| {
        let mtx = g.mtx();
        put_mtx(ctx, MTX, &mtx);
        clear(ctx, MTX_OUT, 12);
        fns::HSD_MtxInverse(ctx, m(ctx, MTX), m(ctx, MTX_OUT));
        assert_eq!(
            read_f32s(ctx, MTX_OUT, 12),
            mtx_bits(&mtx.hsd_inverse()),
            "HSD_MtxInverse {mtx:?} {mode:?}"
        );
    });
}

#[test]
fn sphere_entry_matches() {
    let (mut entered, mut degenerate) = (0, 0);
    each_mode(60_000, |ctx, g, mode| {
        let s = g.scene();
        let mtx = g.mtx();
        let extra = g.radius();
        put(ctx, A, s.a0);
        put(ctx, B, s.b0);
        put(ctx, C, s.b1);
        put_mtx(ctx, MTX, &mtx);
        for out in [OUT1, OUT2, OUT3] {
            clear(ctx, out, 3);
        }
        fns::lbColl_800077A0(
            ctx,
            v(ctx, A),
            m(ctx, MTX),
            v(ctx, B),
            v(ctx, C),
            v(ctx, OUT1),
            v(ctx, OUT2),
            f(ctx, OUT3),
            arg(s.r0),
            arg(extra),
        );
        let ours = sphere_entry(s.a0, &mtx, s.b0, s.b1, s.r0, extra);
        let ctx_ = (&s, &mtx, extra, mode);
        match ours.point {
            None => {
                degenerate += 1;
                assert_eq!(
                    got(ctx, OUT1),
                    vec![UNWRITTEN; 3],
                    "800077A0 point {ctx_:?}"
                );
            }
            Some(p) => {
                entered += 1;
                assert_eq!(got(ctx, OUT1), b3(p), "800077A0 point {ctx_:?}");
            }
        }
        assert_eq!(got(ctx, OUT2), b3(ours.normal), "800077A0 normal {ctx_:?}");
        assert_eq!(
            ctx.read_u32(OUT3),
            ours.angle.to_bits(),
            "800077A0 angle {ctx_:?}"
        );
    });
    eprintln!("800077A0: {entered} entered, {degenerate} zero-length");
}

/// `HitCapsule`'s fields that 80007AFC reads and writes.
fn put_hit_shape(ctx: &Ctx, addr: u32, shape: &HitShape) {
    let capsule: HitCapsule = handle(ctx, addr);
    capsule.set_x43_b1(u8::from(shape.fixed_radius));
    ctx.write_u32(addr + 0x1C, shape.radius.to_bits());
    put(ctx, addr + 0x4C, shape.pos);
    put(ctx, addr + 0x58, shape.prev_pos);
    put(ctx, addr + 0x64, shape.contact);
}

#[test]
fn hitbox_clash_matches() {
    let mut outcomes = Outcomes::default();
    each_mode(60_000, |ctx, g, mode| {
        let s = g.scene();
        let unwritten = Vec3::new(
            f32::from_bits(UNWRITTEN),
            f32::from_bits(UNWRITTEN),
            f32::from_bits(UNWRITTEN),
        );
        let mut a = HitShape {
            radius: s.r0,
            fixed_radius: g.below(3) == 0,
            pos: s.a1,
            prev_pos: s.a0,
            contact: unwritten,
        };
        let mut b = HitShape {
            radius: s.r1,
            fixed_radius: g.below(3) == 0,
            pos: s.b1,
            prev_pos: s.b0,
            contact: unwritten,
        };
        let (x, y) = (g.radius(), g.radius());
        put_hit_shape(ctx, SHAPE1, &a);
        put_hit_shape(ctx, SHAPE2, &b);
        let hit = fns::lbColl_80007AFC(
            ctx,
            handle(ctx, SHAPE1),
            handle(ctx, SHAPE2),
            arg(x),
            arg(y),
        );
        let touching = collision::hitboxes_touch(&mut a, &mut b, x, y);
        outcomes.add(
            if a.contact == unwritten && b.contact == unwritten && !touching {
                None
            } else {
                Some(touching)
            },
        );
        let ctx_ = (&s, x, y, mode);
        assert_eq!(hit, i32::from(touching), "80007AFC {ctx_:?}");
        assert_eq!(
            got(ctx, SHAPE1 + 0x64),
            b3(a.contact),
            "80007AFC a {ctx_:?}"
        );
        assert_eq!(
            got(ctx, SHAPE2 + 0x64),
            b3(b.contact),
            "80007AFC b {ctx_:?}"
        );
    });
    outcomes.check("80007AFC");
}

fn put_circle(ctx: &Ctx, addr: u32, circle: &SweptCircle) {
    ctx.write_u32(addr, circle.radius.to_bits());
    put(ctx, addr + 0x8, circle.pos);
    put(ctx, addr + 0x14, circle.prev_pos);
    put(ctx, addr + 0x20, circle.contact);
}

#[test]
fn swept_circles_match() {
    let mut outcomes = Outcomes::default();
    each_mode(60_000, |ctx, g, mode| {
        let mut s = g.scene();
        if g.below(4) != 0 {
            for p in [&mut s.a0, &mut s.a1, &mut s.b0, &mut s.b1] {
                p.z = 0.0;
            }
        }
        let unwritten = Vec3::new(
            f32::from_bits(UNWRITTEN),
            f32::from_bits(UNWRITTEN),
            f32::from_bits(UNWRITTEN),
        );
        let mut a = SweptCircle {
            radius: s.r0,
            pos: s.a1,
            prev_pos: s.a0,
            contact: unwritten,
        };
        let mut b = SweptCircle {
            radius: s.r1,
            pos: s.b1,
            prev_pos: s.b0,
            contact: unwritten,
        };
        let (x, y) = (g.radius(), g.radius());
        put_circle(ctx, SHAPE1, &a);
        put_circle(ctx, SHAPE2, &b);
        let (ha, hb): (Fighter_x1614_t, Fighter_x1614_t) =
            (handle(ctx, SHAPE1), handle(ctx, SHAPE2));
        let hit = fns::lbColl_80007B78(ctx, ha, hb, arg(x), arg(y));
        let touching = collision::swept_circles_touch(&mut a, &mut b, x, y);
        outcomes.add(
            if a.contact == unwritten && b.contact == unwritten && !touching {
                None
            } else {
                Some(touching)
            },
        );
        let ctx_ = (&s, x, y, mode);
        assert_eq!(hit, i32::from(touching), "80007B78 {ctx_:?}");
        assert_eq!(
            got(ctx, SHAPE1 + 0x20),
            b3(a.contact),
            "80007B78 a {ctx_:?}"
        );
        assert_eq!(
            got(ctx, SHAPE2 + 0x20),
            b3(b.contact),
            "80007B78 b {ctx_:?}"
        );
    });
    outcomes.check("80007B78");
}
