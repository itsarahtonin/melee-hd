//! Frames between two ticks, for displays faster than the game's 60 ticks a second
//! (docs/architecture.md, "Frame pacing and high refresh rates").
//!
//! A frame is drawn from two states and a fraction: A, what a tick's display pass left
//! ([`Capture`], kept from the tick before), B, the state the display pass of the latest tick
//! left (drawn live, as every frame is), and t, how far between them the frame shows. Only
//! transforms blend, and only for what both passes drew: joints' world matrices (keyed by the
//! joint's handle and the descriptor it was loaded from), cameras (by handle and the how-manyth
//! time this frame the camera draws), instance joints' view matrices, particles' app-SRT views
//! and particles' positions. Everything else is B's as it is: colors, textures and their
//! matrices, materials, lights, the HUD and other 2D (drawn through cameras of their own that
//! cover the screen, [`camera_blends`]), text, quads, afterimages, and what only one pass drew
//! (spawned, freed or shown in one), which is drawn where B has it, seen through the blended
//! camera. Discontinuities snap to B too: a joint or particle that moved more than [`JUMP`]
//! in a tick, a joint whose handedness flipped, and a camera that cut ([`CUT_DISTANCE`],
//! [`CUT_ANGLE`]) or changed its projection's kind.
//!
//! **Blend method.** A joint's world matrix is decomposed as `Q U + T`: `Q` a rotation, `U`
//! upper triangular (scale and shear, from Gram-Schmidt on its columns, the third column's sign
//! moved into `U` so `Q` stays a rotation), `T` its translation. `Q` is slerped (the shorter
//! way), `U` and `T` are lerped, so a limb turning 60 degrees in a tick keeps its length
//! midway, where lerping the matrix would shrink it by a seventh. Composing the world matrix
//! from blended local SRTs would need the hierarchy rerun, which is simulation; the world
//! matrix holds the same motion. A matrix too degenerate to decompose (a joint scaled to
//! nothing, which is how models hide parts) lerps component by component. A camera's view
//! decomposes the same way about its eye, so the eye moves in a straight line and the view
//! turns by slerp; its projection (field of view or bounds, aspect, near and far) lerps; its
//! viewport and scissor are B's.
//!
//! Every blend returns A exactly at t = 0 and B exactly at t = 1, so a frame at t = 1 is the
//! frame Off draws, command for command.

use melee_data::archive::DataRef;
use melee_sim::game::state::{Game, Obj};
use melee_sim::hsd::Objects;
use melee_sim::hsd::appsrt::AppSrtId;
use melee_sim::hsd::cobj::CObjId;
use melee_sim::hsd::display::{CameraView, Draw};
use melee_sim::hsd::dobj::DObjId;
use melee_sim::hsd::jobj::{self, JObjId};
use melee_sim::hsd::particle::ParticleId;
use melee_sim::hsd::pobj::{Binding, PObjId};
use melee_sim::hsd::tobj::TObjId;
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;
use rustc_hash::FxHashMap;

/// How far (in world units) a joint or particle may move in one tick and still blend. Melee's
/// fastest continuous motion stays under it: running and falling move 2 to 4 units a tick, a
/// strong launch about 6, a sword's tip turning 90 degrees about 20; a respawn, a teleport's
/// end or a reused handle jump further.
pub const JUMP: f32 = 40.0;

/// How far a camera's eye may move in one tick and still blend; further is a cut.
pub const CUT_DISTANCE: f32 = 60.0;

/// How far (in radians) a camera may turn in one tick and still blend; further is a cut.
pub const CUT_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

/// A shape as the display pass names it: its joint, display object and shape.
pub type ShapeKey = (JObjId, DObjId, PObjId);

/// The view a shape drew in: one the display pass named for it, or the current camera's.
#[derive(Clone, Copy)]
struct ShapeView {
    named: bool,
    view: Mtx34,
}

/// A joint's world matrix as A had it.
#[derive(Clone, Copy, Default)]
struct Joint {
    /// The capture this entry belongs to (entries of older captures are stale).
    stamp: u32,
    /// The descriptor the joint was loaded from, which a reused handle rarely shares.
    id: Option<DataRef>,
    mtx: Mtx34,
    /// Its rotation about z, which a rotating billboard turns by.
    rotate_z: f32,
}

/// A particle as A had it.
#[derive(Clone, Copy, Default)]
struct Particle {
    stamp: u32,
    idnum: u16,
    bank: u8,
    motion: ParticleMotion,
}

/// What of a particle blends: where it is and its velocity (its trail's direction and length),
/// its size and its rotation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleMotion {
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub rotate: f32,
}

impl ParticleMotion {
    pub fn of(p: &melee_sim::hsd::particle::Particle) -> ParticleMotion {
        ParticleMotion {
            pos: p.pos,
            vel: p.vel,
            size: p.size,
            rotate: p.rotate,
        }
    }
}

/// What a frame between two ticks takes from the earlier one (A): its transforms, keyed by the
/// display pass's handles. Filling it allocates nothing once warmed up.
#[derive(Default)]
pub struct Capture {
    stamp: u32,
    joints: Vec<Joint>,
    particles: Vec<Particle>,
    /// The shapes the pass drew, by the how-manyth time this frame each was drawn (a fighter
    /// draws into its shadow's texture and then onto the screen), with the view each drew in:
    /// the one the pass named (inside an instance joint), or the current camera's.
    shapes: FxHashMap<(ShapeKey, u16), ShapeView>,
    /// How many times each shape was drawn, while filling.
    drawn: FxHashMap<ShapeKey, u16>,
    /// Each camera's views in the order the pass drew them.
    cameras: Vec<(CObjId, CameraView)>,
    /// Each app-SRT view named, in order, with its app-SRT's ID number.
    app_srts: Vec<(AppSrtId, u16, Mtx34, [f32; 2])>,
    /// The shadow textures' matrices (each shadow camera's projection onto the stage).
    shadow_tobjs: Vec<(TObjId, Mtx34)>,
    /// Whether it holds a tick.
    filled: bool,
}

impl Capture {
    /// Keeps what blending needs of a tick's display pass: `draws` as `game` was left by it.
    pub fn fill(&mut self, game: &Game, draws: &[Draw]) {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            // Wrapped: forget every entry rather than mistake an old one for new.
            self.joints.iter_mut().for_each(|j| j.stamp = 0);
            self.particles.iter_mut().for_each(|p| p.stamp = 0);
            self.stamp = 1;
        }
        self.shapes.clear();
        self.drawn.clear();
        self.cameras.clear();
        self.app_srts.clear();
        self.shadow_tobjs.clear();
        let objs = &game.objs;
        let mut current = Mtx34::IDENTITY;
        for d in draws {
            match d {
                Draw::Camera(c, i) => {
                    let v = objs.display.cameras[usize::from(*i)];
                    current = v.view_mtx;
                    self.cameras.push((*c, v));
                }
                Draw::Shape {
                    jobj,
                    dobj,
                    pobj,
                    vmtx,
                    ..
                } => {
                    let key = (*jobj, *dobj, *pobj);
                    let n = self.drawn.entry(key).or_default();
                    let view = ShapeView {
                        named: vmtx.is_some(),
                        view: vmtx.unwrap_or(current),
                    };
                    self.shapes.insert((key, *n), view);
                    *n += 1;
                    if *n > 1 {
                        continue;
                    }
                    self.joint(objs, *jobj);
                    match &objs.pobjs[*pobj].u {
                        Binding::Envelopes(envs) => {
                            for j in envs.iter().flatten().filter_map(|e| e.jobj) {
                                self.joint(objs, j);
                            }
                            // The skeleton joint the envelopes are relative to.
                            let skeleton =
                                std::iter::successors(Some(*jobj), |&x| objs.jobjs[x].parent).find(
                                    |&x| {
                                        objs.jobjs[x].flags & (jobj::SKELETON | jobj::SKELETON_ROOT)
                                            != 0
                                    },
                                );
                            if let Some(x) = skeleton {
                                self.joint(objs, x);
                            }
                        }
                        Binding::Skin(Some(s)) => self.joint(objs, *s),
                        _ => {}
                    }
                }
                Draw::SpecularInit { jobj, .. } | Draw::FighterJoint { jobj, .. } => {
                    self.joint(objs, *jobj)
                }
                Draw::AppSrt { srt, view, scale } => {
                    self.app_srts
                        .push((*srt, objs.app_srts[*srt].idnum, *view, *scale));
                }
                Draw::Particles { first, .. } => {
                    let mut cur = *first;
                    while let Some(id) = cur {
                        let p = &objs.particles[id];
                        cur = p.next;
                        // Too small to draw (or NaN, which draws): not drawn, not kept.
                        if p.size < f32::EPSILON {
                            continue;
                        }
                        let slot = id.0 as usize;
                        if self.particles.len() <= slot {
                            self.particles.resize(slot + 1, Particle::default());
                        }
                        self.particles[slot] = Particle {
                            stamp: self.stamp,
                            idnum: p.idnum,
                            bank: p.bank,
                            motion: ParticleMotion::of(p),
                        };
                    }
                }
                Draw::Shadows { start, len } => {
                    let start = usize::from(*start);
                    for &t in &objs.display.shadow_tobjs[start..start + usize::from(*len)] {
                        if !self.shadow_tobjs.iter().any(|(x, _)| *x == t) {
                            self.shadow_tobjs.push((t, objs.tobjs[t].mtx));
                        }
                    }
                }
                _ => {}
            }
        }
        self.filled = true;
    }

    /// Keeps only the cameras: a frame between two ticks then draws everything where B has it,
    /// seen through the blended cameras, as it draws what A didn't (for checks).
    pub fn keep_only_cameras(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
        self.shapes.clear();
        self.app_srts.clear();
        self.shadow_tobjs.clear();
    }

    /// Forgets the tick it holds: the next frames draw B as it is until it's filled again.
    pub fn clear(&mut self) {
        self.filled = false;
    }

    /// Whether it holds a tick.
    pub fn is_filled(&self) -> bool {
        self.filled
    }

    fn joint(&mut self, objs: &Objects, j: JObjId) {
        let slot = j.0 as usize;
        if self.joints.len() <= slot {
            self.joints.resize(slot + 1, Joint::default());
        }
        if self.joints[slot].stamp == self.stamp {
            return;
        }
        let o = &objs.jobjs[j];
        self.joints[slot] = Joint {
            stamp: self.stamp,
            id: o.id,
            mtx: o.mtx,
            rotate_z: o.rotate.z,
        };
    }

    fn joint_of(&self, j: JObjId) -> Option<&Joint> {
        self.joints
            .get(j.0 as usize)
            .filter(|x| x.stamp == self.stamp)
    }
}

/// A frame between two ticks: A's capture and how far toward B (the state drawn) it shows.
#[derive(Clone, Copy)]
pub struct Blend<'a> {
    pub from: &'a Capture,
    /// 0 shows A, 1 shows B; clamped.
    pub t: f32,
}

impl Blend<'_> {
    /// Whether a shape B draws for the `n`th time this frame (from 0) blends: A drew it that
    /// many times too.
    pub fn has_shape(&self, key: ShapeKey, n: u16) -> bool {
        self.from.shapes.contains_key(&(key, n))
    }

    /// Whether a joint has A's matrix for blending: A drew something of it or through it, and
    /// it's still the joint loaded from the same descriptor.
    pub fn has_joint(&self, objs: &Objects, j: JObjId) -> bool {
        self.from
            .joint_of(j)
            .is_some_and(|a| a.id == objs.jobjs[j].id)
    }

    /// A joint's world matrix at t: blended with A's, or B's where A has none or it jumped.
    pub fn joint(&self, objs: &Objects, j: JObjId) -> Mtx34 {
        self.joint_and_turn(objs, j).0
    }

    /// A joint's world matrix and its rotation about z (a rotating billboard's) at t.
    pub fn joint_and_turn(&self, objs: &Objects, j: JObjId) -> (Mtx34, f32) {
        let b = &objs.jobjs[j];
        match self.from.joint_of(j) {
            Some(a)
                if a.id == b.id && distance(translation(&a.mtx), translation(&b.mtx)) <= JUMP =>
            {
                (
                    blend_mtx(&a.mtx, &b.mtx, self.t),
                    lerp(a.rotate_z, b.rotate.z, self.t),
                )
            }
            _ => (b.mtx, b.rotate.z),
        }
    }

    /// The view the `n`th drawing of a shape draws in at t, given the view the pass named for
    /// it in B (inside an instance joint), the current camera's as blended and as B has it.
    /// Where both ticks drew it in the current camera, the blended camera's.
    pub fn shape_view(
        &self,
        key: ShapeKey,
        n: u16,
        named: Option<Mtx34>,
        current: &Mtx34,
        current_b: &Mtx34,
    ) -> Mtx34 {
        match (self.from.shapes.get(&(key, n)), named) {
            (Some(a), _) if a.named || named.is_some() => {
                moving_mtx(&a.view, &named.unwrap_or(*current_b), self.t)
            }
            (_, Some(b)) => b,
            _ => *current,
        }
    }

    /// The `n`th view camera `c` draws in this frame (from 0), at t: none on a cut or where A
    /// drew the camera fewer times.
    pub fn camera(&self, c: CObjId, n: u16, b: &CameraView) -> Option<CameraView> {
        let a = self
            .from
            .cameras
            .iter()
            .filter(|(x, _)| *x == c)
            .nth(usize::from(n))?;
        blend_camera(&a.1, b, self.t)
    }

    /// The `n`th view named for app-SRT `srt` this frame, at t.
    pub fn app_srt(
        &self,
        srt: AppSrtId,
        idnum: u16,
        n: u16,
        view: &Mtx34,
        scale: [f32; 2],
    ) -> (Mtx34, [f32; 2]) {
        let a = self
            .from
            .app_srts
            .iter()
            .filter(|x| x.0 == srt)
            .nth(usize::from(n));
        match a {
            Some(&(_, id, ref av, asc)) if id == idnum => (
                moving_mtx(av, view, self.t),
                [
                    lerp(asc[0], scale[0], self.t),
                    lerp(asc[1], scale[1], self.t),
                ],
            ),
            _ => (*view, scale),
        }
    }

    /// A particle's motion at t: blended with A's, or B's where A didn't draw it, it's
    /// another particle in the slot, or it jumped.
    pub fn particle(
        &self,
        id: ParticleId,
        idnum: u16,
        bank: u8,
        b: ParticleMotion,
    ) -> ParticleMotion {
        match self.from.particles.get(id.0 as usize) {
            Some(a)
                if a.stamp == self.from.stamp
                    && a.idnum == idnum
                    && a.bank == bank
                    && distance(a.motion.pos, b.pos) <= JUMP =>
            {
                let (a, t) = (a.motion, self.t);
                ParticleMotion {
                    pos: lerp_vec(a.pos, b.pos, t),
                    vel: lerp_vec(a.vel, b.vel, t),
                    size: lerp(a.size, b.size, t),
                    rotate: lerp(a.rotate, b.rotate, t),
                }
            }
            _ => b,
        }
    }

    /// A shadow texture's matrix at t (its camera's projection onto the stage).
    pub fn shadow_tobj(&self, t: TObjId, b: &Mtx34) -> Mtx34 {
        match self.from.shadow_tobjs.iter().find(|(x, _)| *x == t) {
            Some((_, a)) => lerp_mtx(a, b, self.t),
            None => *b,
        }
    }
}

/// Whether camera `c`'s draws blend: the game camera's, and those of the cameras that draw
/// into part of the frame or a texture (shadows, Fountain of Dreams' reflection, Pokémon
/// Stadium's screen, the magnifier's bubbles), not those of the other cameras that cover the
/// screen: the HUD's and other 2D (damage, timer, text, the background flash), which draw
/// through cameras of their own, some of them perspective.
pub fn camera_blends(game: &Game, c: CObjId, v: &CameraView) -> bool {
    v.viewport != [0.0, 640.0, 0.0, 480.0] || game_camera(game) == Some(c)
}

/// The game camera's CObj (`Camera_Create`'s).
pub fn game_camera(game: &Game) -> Option<CObjId> {
    let g = game.camera.cam.gobj?;
    match game.world.gobj(g).obj {
        Some(Obj::CObj(c)) => Some(c),
        _ => None,
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        a + (b - a) * t
    }
}

fn lerp_vec(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.z, b.z, t))
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    let (x, y, z) = (b.x - a.x, b.y - a.y, b.z - a.z);
    (x * x + y * y + z * z).sqrt()
}

fn translation(m: &Mtx34) -> Vec3 {
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// Component by component.
pub fn lerp_mtx(a: &Mtx34, b: &Mtx34, t: f32) -> Mtx34 {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    let mut m = *b;
    for (r, row) in m.m.iter_mut().enumerate() {
        for (c, x) in row.iter_mut().enumerate() {
            *x = lerp(a.m[r][c], b.m[r][c], t);
        }
    }
    m
}

/// A moving object's matrix at t: B's if it moved more than [`JUMP`], else [`blend_mtx`].
pub fn moving_mtx(a: &Mtx34, b: &Mtx34, t: f32) -> Mtx34 {
    if distance(translation(a), translation(b)) > JUMP {
        *b
    } else {
        blend_mtx(a, b, t)
    }
}

/// An affine matrix at t between `a` and `b`: rotation slerped, scale and shear and
/// translation lerped (see the module's notes); component by component where either is too
/// degenerate to decompose; B's where the handedness differs.
pub fn blend_mtx(a: &Mtx34, b: &Mtx34, t: f32) -> Mtx34 {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    let (Some(da), Some(db)) = (Decomposed::of(a), Decomposed::of(b)) else {
        return lerp_mtx(a, b, t);
    };
    if da.mirrored != db.mirrored {
        return *b;
    }
    let q = slerp(da.q, db.q, t);
    let u: [f32; 6] = std::array::from_fn(|i| lerp(da.u[i], db.u[i], t));
    let tr = lerp_vec(translation(a), translation(b), t);
    compose(q, u, tr)
}

/// A camera's view at t, or none on a cut (see the module's notes).
pub fn blend_camera(a: &CameraView, b: &CameraView, t: f32) -> Option<CameraView> {
    if a.projection_type != b.projection_type {
        return None;
    }
    if t >= 1.0 {
        return Some(*b);
    }
    let (da, db) = (Decomposed::of(&a.view_mtx), Decomposed::of(&b.view_mtx));
    let mut v = *b;
    match (da, db) {
        (Some(da), Some(db)) if da.mirrored == db.mirrored => {
            let (ea, eb) = (da.eye(&a.view_mtx)?, db.eye(&b.view_mtx)?);
            let dot = quat_dot(da.q, db.q).abs().min(1.0);
            if distance(ea, eb) > CUT_DISTANCE || 2.0 * dot.acos() > CUT_ANGLE {
                return None;
            }
            if t <= 0.0 {
                return Some(CameraView {
                    viewport: b.viewport,
                    scissor: b.scissor,
                    ..*a
                });
            }
            let q = slerp(da.q, db.q, t);
            let u: [f32; 6] = std::array::from_fn(|i| lerp(da.u[i], db.u[i], t));
            let eye = lerp_vec(ea, eb, t);
            // The view's translation puts the eye at the origin: -M eye.
            let m = compose(q, u, Vec3::default());
            let r = m.rotate(eye);
            v.view_mtx = m;
            v.view_mtx.m[0][3] = -r.x;
            v.view_mtx.m[1][3] = -r.y;
            v.view_mtx.m[2][3] = -r.z;
        }
        _ => {
            if distance(translation(&a.view_mtx), translation(&b.view_mtx)) > CUT_DISTANCE {
                return None;
            }
            if t <= 0.0 {
                return Some(CameraView {
                    viewport: b.viewport,
                    scissor: b.scissor,
                    ..*a
                });
            }
            v.view_mtx = lerp_mtx(&a.view_mtx, &b.view_mtx, t);
        }
    }
    for i in 0..4 {
        v.projection[i] = lerp(a.projection[i], b.projection[i], t);
    }
    v.near = lerp(a.near, b.near, t);
    v.far = lerp(a.far, b.far, t);
    Some(v)
}

/// A 3 by 3 linear part as a rotation (quaternion `q`) times an upper triangular `u`
/// (`u00 u01 u02 u11 u12 u22`), with the third axis's sign moved into `u22` when `mirrored`.
#[derive(Clone, Copy, Debug)]
struct Decomposed {
    q: [f32; 4],
    u: [f32; 6],
    mirrored: bool,
}

/// Smaller than this (relative to the first axis, at least 1), an axis counts as collapsed.
const DEGENERATE: f32 = 1e-6;

/// Whether an axis of length `len` is long enough to decompose, `first` the first axis's.
fn kept(len: f32, first: f32) -> bool {
    len.is_finite() && len > DEGENERATE * first.max(1.0)
}

impl Decomposed {
    fn of(m: &Mtx34) -> Option<Decomposed> {
        let col = |c: usize| [m.m[0][c], m.m[1][c], m.m[2][c]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let len = |a: [f32; 3]| dot(a, a).sqrt();
        let sub =
            |a: [f32; 3], b: [f32; 3], s: f32| [a[0] - b[0] * s, a[1] - b[1] * s, a[2] - b[2] * s];
        let div = |a: [f32; 3], s: f32| [a[0] / s, a[1] / s, a[2] / s];
        let (c0, c1, c2) = (col(0), col(1), col(2));
        let u00 = len(c0);
        if !kept(u00, 1.0) {
            return None;
        }
        let q0 = div(c0, u00);
        let u01 = dot(q0, c1);
        let v1 = sub(c1, q0, u01);
        let u11 = len(v1);
        if !kept(u11, u00) {
            return None;
        }
        let q1 = div(v1, u11);
        let u02 = dot(q0, c2);
        let u12 = dot(q1, c2);
        let v2 = sub(sub(c2, q0, u02), q1, u12);
        let mut u22 = len(v2);
        if !kept(u22, u00) {
            return None;
        }
        let mut q2 = div(v2, u22);
        let cross = [
            q0[1] * q1[2] - q0[2] * q1[1],
            q0[2] * q1[0] - q0[0] * q1[2],
            q0[0] * q1[1] - q0[1] * q1[0],
        ];
        let mirrored = dot(cross, q2) < 0.0;
        if mirrored {
            q2 = [-q2[0], -q2[1], -q2[2]];
            u22 = -u22;
        }
        let q = quat_from_columns(q0, q1, q2);
        if !q.iter().all(|x| x.is_finite()) {
            return None;
        }
        Some(Decomposed {
            q,
            u: [u00, u01, u02, u11, u12, u22],
            mirrored,
        })
    }

    /// Where the camera whose view `m` is sits: `-M⁻¹ T`.
    fn eye(&self, m: &Mtx34) -> Option<Vec3> {
        let mut linear = *m;
        for r in 0..3 {
            linear.m[r][3] = 0.0;
        }
        let inv = linear.inverse()?;
        let p = inv.rotate(translation(m));
        Some(Vec3::new(-p.x, -p.y, -p.z))
    }
}

/// The rotation whose columns are `x`, `y` and `z` as a unit quaternion `[x, y, z, w]`.
fn quat_from_columns(x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> [f32; 4] {
    // m[r][c]: row r of column c.
    let m = [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]];
    let trace = m[0][0] + m[1][1] + m[2][2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
            0.25 * s,
        ]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [
            0.25 * s,
            (m[0][1] + m[1][0]) / s,
            (m[0][2] + m[2][0]) / s,
            (m[2][1] - m[1][2]) / s,
        ]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [
            (m[0][1] + m[1][0]) / s,
            0.25 * s,
            (m[1][2] + m[2][1]) / s,
            (m[0][2] - m[2][0]) / s,
        ]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [
            (m[0][2] + m[2][0]) / s,
            (m[1][2] + m[2][1]) / s,
            0.25 * s,
            (m[1][0] - m[0][1]) / s,
        ]
    };
    normalize(q)
}

fn normalize(q: [f32; 4]) -> [f32; 4] {
    let n = quat_dot(q, q).sqrt();
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

fn quat_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// Spherical interpolation the shorter way round.
fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut d = quat_dot(a, b);
    let b = if d < 0.0 {
        d = -d;
        [-b[0], -b[1], -b[2], -b[3]]
    } else {
        b
    };
    if d > 0.9995 {
        return normalize(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t));
    }
    let theta = d.min(1.0).acos();
    let s = theta.sin();
    let (wa, wb) = (((1.0 - t) * theta).sin() / s, (t * theta).sin() / s);
    normalize(std::array::from_fn(|i| a[i] * wa + b[i] * wb))
}

/// `R(q) U` with translation `tr`.
fn compose(q: [f32; 4], u: [f32; 6], tr: Vec3) -> Mtx34 {
    let [x, y, z, w] = q;
    let r = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let [u00, u01, u02, u11, u12, u22] = u;
    let mut m = Mtx34::default();
    for (i, row) in r.iter().enumerate() {
        m.m[i][0] = row[0] * u00;
        m.m[i][1] = row[0] * u01 + row[1] * u11;
        m.m[i][2] = row[0] * u02 + row[1] * u12 + row[2] * u22;
        m.m[i][3] = [tr.x, tr.y, tr.z][i];
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::hsd::cobj;
    use melee_sim::mtx::Axis;

    fn close(a: &Mtx34, b: &Mtx34, eps: f32) -> bool {
        (0..3).all(|r| (0..4).all(|c| (a.m[r][c] - b.m[r][c]).abs() <= eps))
    }

    fn trs(x: f32, angle: f32, s: [f32; 3]) -> Mtx34 {
        let mut m = Mtx34::rotation(Axis::Y, angle).concat(&Mtx34::scale(s[0], s[1], s[2]));
        m.m[0][3] = x;
        m.m[1][3] = 2.0;
        m.m[2][3] = -3.0;
        m
    }

    #[test]
    fn decomposing_and_composing_gives_the_matrix_back() {
        let mut sheared = trs(1.0, 0.7, [1.5, 0.5, 2.0]);
        sheared.m[0][1] += 0.3;
        let mut mirrored = trs(-4.0, 2.5, [1.0, 1.0, 1.0]);
        for r in 0..3 {
            mirrored.m[r][0] = -mirrored.m[r][0];
        }
        for m in [
            trs(1.0, 0.0, [1.0; 3]),
            trs(5.0, 3.0, [0.2, 3.0, 1.0]),
            sheared,
            mirrored,
        ] {
            let d = Decomposed::of(&m).expect("a decomposition");
            assert!(
                close(&compose(d.q, d.u, translation(&m)), &m, 1e-5),
                "{m:?}"
            );
        }
    }

    #[test]
    fn the_ends_are_exact_and_a_turn_keeps_its_length() {
        let a = trs(0.0, 0.0, [2.0; 3]);
        let b = trs(4.0, std::f32::consts::FRAC_PI_2, [2.0; 3]);
        assert_eq!(blend_mtx(&a, &b, 0.0), a);
        assert_eq!(blend_mtx(&a, &b, 1.0), b);
        let mid = blend_mtx(&a, &b, 0.5);
        assert!(close(
            &mid,
            &trs(2.0, std::f32::consts::FRAC_PI_4, [2.0; 3]),
            1e-5
        ));
        // Lerping the components would have shrunk it to cos 45 degrees.
        let lerped = lerp_mtx(&a, &b, 0.5);
        let col0 = |m: &Mtx34| (m.m[0][0].powi(2) + m.m[1][0].powi(2) + m.m[2][0].powi(2)).sqrt();
        assert!((col0(&mid) - 2.0).abs() < 1e-5);
        assert!((col0(&lerped) - 2.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    }

    #[test]
    fn collapsed_matrices_lerp_and_flips_snap() {
        let a = trs(0.0, 0.0, [0.0; 3]);
        let b = trs(2.0, 0.0, [1.0; 3]);
        assert!(close(&blend_mtx(&a, &b, 0.5), &lerp_mtx(&a, &b, 0.5), 0.0));
        let mut flipped = b;
        for r in 0..3 {
            flipped.m[r][2] = -flipped.m[r][2];
        }
        assert_eq!(blend_mtx(&b, &flipped, 0.5), flipped);
    }

    #[test]
    fn jumps_snap() {
        let a = trs(0.0, 0.0, [1.0; 3]);
        let near = trs(JUMP * 0.9, 0.0, [1.0; 3]);
        let far = trs(JUMP * 1.1, 0.0, [1.0; 3]);
        assert!((moving_mtx(&a, &near, 0.5).m[0][3] - JUMP * 0.45).abs() < 1e-4);
        assert_eq!(moving_mtx(&a, &far, 0.5), far);
    }

    fn camera(eye: Vec3, yaw: f32, fov: f32) -> CameraView {
        // A view looking along -z turned by `yaw`, from `eye`.
        let r = Mtx34::rotation(Axis::Y, -yaw);
        let p = r.rotate(eye);
        let mut view = r;
        view.m[0][3] = -p.x;
        view.m[1][3] = -p.y;
        view.m[2][3] = -p.z;
        CameraView {
            viewport: [0.0, 640.0, 0.0, 480.0],
            scissor: [0, 640, 0, 480],
            projection_type: cobj::PERSPECTIVE,
            projection: [fov, 1.2, 0.0, 0.0],
            near: 1.0,
            far: 1000.0,
            view_mtx: view,
        }
    }

    #[test]
    fn cameras_move_their_eye_in_a_line_and_cuts_snap() {
        let a = camera(Vec3::new(0.0, 10.0, 100.0), 0.0, 30.0);
        let b = camera(Vec3::new(10.0, 10.0, 90.0), 0.2, 40.0);
        assert_eq!(blend_camera(&a, &b, 0.0), Some(a));
        let narrow = CameraView {
            viewport: [0.0, 320.0, 0.0, 480.0],
            ..b
        };
        let from_a = blend_camera(&a, &narrow, 0.0).expect("no cut");
        assert_eq!(
            (from_a.view_mtx, from_a.viewport),
            (a.view_mtx, narrow.viewport)
        );
        assert_eq!(blend_camera(&a, &b, 1.0), Some(b));
        let mid = blend_camera(&a, &b, 0.5).expect("no cut");
        let want = camera(Vec3::new(5.0, 10.0, 95.0), 0.1, 35.0);
        assert!(close(&mid.view_mtx, &want.view_mtx, 1e-4), "{mid:?}");
        assert_eq!(mid.projection[0], 35.0);
        let moved = camera(Vec3::new(0.0, 10.0, 100.0 - CUT_DISTANCE - 1.0), 0.0, 30.0);
        assert_eq!(blend_camera(&a, &moved, 0.5), None);
        let turned = camera(Vec3::new(0.0, 10.0, 100.0), CUT_ANGLE + 0.05, 30.0);
        assert_eq!(blend_camera(&a, &turned, 0.5), None);
        let ortho = CameraView {
            projection_type: cobj::ORTHO,
            ..b
        };
        assert_eq!(blend_camera(&a, &ortho, 0.5), None);
    }

    /// The snap rules for joints: what A drew blends; a shape A didn't draw (spawned or shown
    /// this tick), a joint that jumped and a handle reused by another joint take B's.
    #[test]
    fn spawned_reused_and_jumping_joints_snap() {
        use melee_data::archive::ArchiveId;
        use melee_sim::hsd::jobj::JObj;
        use melee_sim::hsd::pobj::PObj;
        let mut game = Game::default();
        let objs = &mut game.objs;
        let desc = |offset| {
            Some(DataRef {
                archive: ArchiveId(0),
                offset,
            })
        };
        let joint = |objs: &mut Objects, x: f32, id| {
            JObjId(objs.jobjs.pool.alloc(JObj {
                mtx: trs(x, 0.0, [1.0; 3]),
                id: desc(id),
                ..JObj::default()
            }))
        };
        let pobj = PObjId(objs.pobjs.pool.alloc(PObj {
            next: None,
            verts: None,
            flags: 0,
            n_display: 0,
            display: None,
            u: Binding::Skin(None),
            class: Default::default(),
        }));
        let (moving, jumping, reused) = (
            joint(objs, 0.0, 0x10),
            joint(objs, 0.0, 0x20),
            joint(objs, 0.0, 0x30),
        );
        let shape = |jobj| Draw::Shape {
            jobj,
            dobj: DObjId(0),
            pobj,
            trsp: 1,
            rendermode: 0,
            vmtx: None,
        };
        let a_draws = [shape(moving), shape(jumping), shape(reused)];
        let mut capture = Capture::default();
        capture.fill(&game, &a_draws);
        // The next tick: one joint moves 2 units, one jumps (a respawn), one handle now holds
        // another joint, and a new joint's shape draws.
        let objs = &mut game.objs;
        objs.jobjs[moving].mtx = trs(2.0, 0.0, [1.0; 3]);
        objs.jobjs[jumping].mtx = trs(JUMP + 10.0, 0.0, [1.0; 3]);
        objs.jobjs[reused].id = desc(0x40);
        objs.jobjs[reused].mtx = trs(1.0, 0.0, [1.0; 3]);
        let spawned = joint(objs, 3.0, 0x50);
        let blend = Blend {
            from: &capture,
            t: 0.5,
        };
        let objs = &game.objs;
        let key = |j| (j, DObjId(0), pobj);
        assert!(blend.has_shape(key(moving), 0));
        assert!(blend.has_shape(key(jumping), 0));
        assert!(!blend.has_shape(key(moving), 1));
        assert!(!blend.has_shape(key(spawned), 0));
        assert!(blend.has_joint(objs, moving));
        assert!(!blend.has_joint(objs, reused));
        assert!(!blend.has_joint(objs, spawned));
        assert!(close(
            &blend.joint(objs, moving),
            &trs(1.0, 0.0, [1.0; 3]),
            1e-6
        ));
        assert_eq!(blend.joint(objs, jumping), objs.jobjs[jumping].mtx);
        assert_eq!(blend.joint(objs, reused), objs.jobjs[reused].mtx);
        assert_eq!(blend.joint(objs, spawned), objs.jobjs[spawned].mtx);
        // At the ends, A's and B's own.
        let at = |t| Blend { from: &capture, t };
        assert_eq!(at(0.0).joint(objs, moving), trs(0.0, 0.0, [1.0; 3]));
        assert_eq!(at(1.0).joint(objs, moving), objs.jobjs[moving].mtx);
    }

    /// The same for particles: another particle in A's slot, or a jump, takes B's position.
    #[test]
    fn particles_blend_unless_replaced_or_jumping() {
        use melee_sim::hsd::particle::Particle as P;
        let mut game = Game::default();
        let id = ParticleId(game.objs.particles.pool.alloc(P {
            idnum: 7,
            bank: 1,
            pos: Vec3::new(0.0, 0.0, 0.0),
            size: 1.0,
            ..P::default()
        }));
        let draws = [Draw::Particles {
            link: 0,
            first: Some(id),
        }];
        let mut capture = Capture::default();
        capture.fill(&game, &draws);
        let blend = Blend {
            from: &capture,
            t: 0.25,
        };
        let at = |x: f32, size: f32| ParticleMotion {
            pos: Vec3::new(x, 0.0, 0.0),
            vel: Vec3::default(),
            size,
            rotate: 0.0,
        };
        let b = at(4.0, 3.0);
        assert_eq!(blend.particle(id, 7, 1, b), at(1.0, 1.5));
        assert_eq!(blend.particle(id, 8, 1, b), b);
        assert_eq!(blend.particle(id, 7, 2, b), b);
        let far = at(JUMP + 1.0, 3.0);
        assert_eq!(blend.particle(id, 7, 1, far), far);
        assert_eq!(blend.particle(ParticleId(id.0 + 1), 7, 1, b), b);
    }

    #[test]
    fn only_the_game_camera_and_partial_cameras_blend() {
        use melee_sim::hsd::cobj::CObj;
        let mut game = Game::default();
        let hud = CObjId(game.objs.cobjs.pool.alloc(CObj::default()));
        let v = camera(Vec3::default(), 0.0, 30.0);
        // No game camera yet: a full-screen camera is the HUD's.
        assert!(!camera_blends(&game, hud, &v));
        let shadow = CameraView {
            projection_type: cobj::ORTHO,
            viewport: [0.0, 128.0, 0.0, 128.0],
            ..v
        };
        assert!(camera_blends(&game, hud, &shadow));
        melee_sim::game::camera::create(&mut game);
        let main = game_camera(&game).expect("the game camera");
        assert!(camera_blends(&game, main, &v));
        assert!(!camera_blends(&game, hud, &v));
    }
}
