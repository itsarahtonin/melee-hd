//! Particle generators, HAL's `HSD_Generator` (`sysdolphin/baselib/generator.c`): emitters
//! that spawn particles from an effect bank's command list each frame, in a shape (a disc, a
//! line, a tornado, a cone, a box, a sphere...), optionally following a joint.

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::appsrt::{self, AppSrtId};
use super::jobj::{self, JObjId};
use super::list::{self, SListId, SLists};
use super::particle::{self, FLT_MIN, GeneratorHook, HALF_PI_F, Hooks, PAUSED, Spawn, UserFunc};
use super::pool::Pool;
use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::math;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// A generator's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeneratorId(pub u32);

/// `type`'s low bits: the shape it emits in.
pub const SHAPE: u16 = 0xF;
/// Its particles go with it when it is removed.
pub const KILL_PARTICLES: u16 = 1 << 7;
/// It follows its joint.
pub const FOLLOW_JOINT: u16 = 1 << 8;
/// Its position is its joint's.
pub const JOINT_POSITION: u16 = 1 << 9;
/// It emits in its joint's orientation.
pub const JOINT_ROTATION: u16 = 1 << 10;
/// Its app-SRT takes its joint's translation and scale.
pub const JOINT_APP_TRANSLATE: u16 = 1 << 11;
pub const JOINT_APP_SCALE: u16 = 1 << 12;

/// `HSD_Generator`. The shape parameters (`aux`) are a union the original reads under more
/// than one name, so they stay twelve words: disc and cone angles (0, 1) and cone height (2),
/// a line's end (0 to 2), a tornado's speed (0), a box's size (0 to 2) and axes (3 to 11,
/// by row), a sphere's speed and latitude and longitude ranges (0 to 4). `callback` is never
/// read or set, so it isn't kept.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Generator {
    pub next: Option<GeneratorId>,
    pub kind: u32,
    pub random: f32,
    pub count: f32,
    pub jobj: Option<JObjId>,
    pub gen_life: u16,
    pub ty: u16,
    pub bank: u8,
    pub link_no: u8,
    pub tex_group: u8,
    pub idnum: u16,
    pub life: u16,
    pub cmd_list: Option<DataRef>,
    pub pos: Vec3,
    pub vel: Vec3,
    pub grav: f32,
    pub fric: f32,
    pub size: f32,
    pub radius: f32,
    pub angle: f32,
    /// Live particles it spawned.
    pub num_child: u32,
    pub appsrt: Option<AppSrtId>,
    pub userfunc: Option<UserFunc>,
    pub aux: [f32; 12],
    /// A box's flag: which faces it emits from.
    pub rect_flag: u16,
}

/// The generator pool (`hsd_804D0F90`) and generator.c's globals.
#[derive(melee_derive::CloneFrom, Debug)]
pub struct Generators {
    pub pool: Pool<Generator, { pool_kind(Kind::Generator) }>,
    /// The generator list (`hsd_804D78FC`).
    pub head: Option<GeneratorId>,
    /// The one before the generator being handled (`hsd_804D78F8`); new generators go in
    /// after it.
    pub cursor: Option<GeneratorId>,
    /// Live generators (`hsd_804D78E0`) and the most at once (`hsd_804D78DA`).
    pub live: u16,
    pub peak: u16,
    /// The last family id handed out (`lbl_804D6368`).
    pub idnum: u16,
    /// Generators to orient by their joint before their first frame (`hsd_804D78F4`), and the
    /// list's nodes.
    pub pending: Option<SListId>,
    pub nodes: SLists<GeneratorId>,
    /// What the game runs on each new generator (`hsd_804D7900`).
    pub on_create: Option<GeneratorHook>,
}

impl Default for Generators {
    fn default() -> Self {
        Generators {
            pool: Pool::default(),
            head: None,
            cursor: None,
            live: 0,
            peak: 0,
            idnum: 0x100,
            pending: None,
            nodes: SLists::default(),
            on_create: None,
        }
    }
}

impl Index<GeneratorId> for Generators {
    type Output = Generator;
    fn index(&self, g: GeneratorId) -> &Generator {
        &self.pool[g.0]
    }
}

impl IndexMut<GeneratorId> for Generators {
    fn index_mut(&mut self, g: GeneratorId) -> &mut Generator {
        &mut self.pool[g.0]
    }
}

/// Sets a generator's user functions, `hsd_8039D1E4`.
#[doc(alias = "hsd_8039D1E4")]
pub fn set_user_func(objs: &mut Objects, g: GeneratorId, f: Option<UserFunc>) {
    objs.generators[g].userfunc = f;
}

/// A new family id, from 0x100 up, `hsd_8039D1EC`.
#[doc(alias = "hsd_8039D1EC")]
pub fn next_idnum(objs: &mut Objects) -> u16 {
    let gs = &mut objs.generators;
    gs.idnum = gs.idnum.wrapping_add(1);
    if gs.idnum < 0x100 {
        gs.idnum = 0x100;
    }
    gs.idnum
}

/// Moves a generator, and the app-SRT it drives, to its joint; returns the joint.
fn follow(objs: &mut Objects, data: &Archives, g: GeneratorId) -> Option<JObjId> {
    let o = &objs.generators[g];
    if o.ty & FOLLOW_JOINT == 0 {
        return None;
    }
    let j = o.jobj?;
    jobj::setup_matrix(objs, data, Some(j));
    let m = objs.jobjs[j].mtx;
    let t = Vec3::new(copy(m.m[0][3]), copy(m.m[1][3]), copy(m.m[2][3]));
    let o = &mut objs.generators[g];
    if o.ty & JOINT_POSITION != 0 {
        o.pos = t;
    }
    let driven = o.appsrt.filter(|&s| objs.app_srts[s].gp == Some(g));
    if o.ty & JOINT_APP_TRANSLATE != 0
        && let Some(s) = driven
    {
        objs.app_srts[s].translate = t;
    }
    if objs.generators[g].ty & JOINT_APP_SCALE != 0
        && let Some(s) = driven
    {
        objs.app_srts[s].scale = m.scale_part();
    }
    Some(j)
}

/// Moves a generator to its joint, `hsd_8039D214`.
#[doc(alias = "hsd_8039D214")]
pub fn follow_joint(objs: &mut Objects, data: &Archives, g: GeneratorId) {
    follow(objs, data, g);
}

/// Moves a generator to its joint unless it is paused, `hsd_803983A4`.
#[doc(alias = "hsd_803983A4")]
pub fn follow_joint_unless_paused(objs: &mut Objects, data: &Archives, g: Option<GeneratorId>) {
    let Some(g) = g else { return };
    if objs.generators[g].kind & PAUSED != 0 {
        return;
    }
    follow(objs, data, g);
}

/// Empties the generator list, `hsd_8039D354`. The family ids carry on, and the pending
/// list's nodes stay allocated, as in the original.
#[doc(alias = "hsd_8039D354")]
pub fn init(objs: &mut Objects) {
    let gs = &mut objs.generators;
    gs.pool = Pool::default();
    gs.head = None;
    gs.live = 0;
    gs.peak = 0;
    gs.pending = None;
    gs.cursor = None;
    gs.on_create = None;
}

/// Frees a generator after `prev` once it has no live particles (and no particle shares its
/// joint-driven app-SRT), else ends it after this frame; returns the generator now before
/// the next, `hsd_8039D3AC`.
#[doc(alias = "hsd_8039D3AC")]
pub fn remove(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    g: GeneratorId,
    prev: Option<GeneratorId>,
) -> Option<GeneratorId> {
    if objs.generators[g].ty & KILL_PARTICLES != 0 {
        particle::kill_generator_particles(objs, data, hooks, g);
    }
    let o = &objs.generators[g];
    let shared = o.ty & FOLLOW_JOINT != 0
        && o.jobj.is_some()
        && o.ty & (JOINT_APP_TRANSLATE | JOINT_APP_SCALE) != 0
        && o.appsrt.is_some_and(|s| {
            let srt = &objs.app_srts[s];
            srt.gp == Some(g) && srt.used_count != 1
        });
    if o.num_child != 0 || shared {
        let o = &mut objs.generators[g];
        o.random = 0.0;
        o.gen_life = 1;
        return Some(g);
    }
    let next = o.next;
    match prev {
        None => objs.generators.head = next,
        Some(p) => objs.generators[p].next = next,
    }
    if objs.generators[g].appsrt.is_some() {
        appsrt::remove_from_generator(objs, g);
    }
    if let Some(j) = objs.generators[g].jobj.take() {
        jobj::unref(objs, Some(j));
    }
    objs.generators.pool.free(g.0);
    objs.generators.live = objs.generators.live.wrapping_sub(1);
    prev
}

/// The last generator in the list from `g` on.
fn last_from(objs: &Objects, mut g: GeneratorId) -> GeneratorId {
    while let Some(n) = objs.generators[g].next {
        g = n;
    }
    g
}

/// Removes a generator, leaving the cursor on the list's last, `hsd_8039D4DC`.
#[doc(alias = "hsd_8039D4DC")]
pub fn kill(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, g: GeneratorId) {
    let mut cur = objs.generators.head;
    objs.generators.cursor = None;
    while let Some(c) = cur {
        if c == g {
            let cursor = remove(objs, data, hooks, g, objs.generators.cursor);
            objs.generators.cursor = cursor
                .or(objs.generators.head)
                .map(|from| last_from(objs, from));
            return;
        }
        objs.generators.cursor = Some(c);
        cur = objs.generators[c].next;
    }
}

/// Removes the generators following a joint, `hsd_8039D580`.
#[doc(alias = "hsd_8039D580")]
pub fn kill_on_jobj(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    jobj: Option<JObjId>,
) {
    if jobj.is_none() {
        return;
    }
    let mut cur = objs.generators.head;
    while let Some(c) = cur {
        let next = objs.generators[c].next;
        if objs.generators[c].jobj == jobj {
            kill(objs, data, hooks, c);
        }
        cur = next;
    }
}

/// Removes the generators following any joint of a tree, `hsd_8039D5DC`.
#[doc(alias = "hsd_8039D5DC")]
pub fn kill_on_tree(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    jobj: Option<JObjId>,
) {
    let Some(j) = jobj else { return };
    kill_on_jobj(objs, data, hooks, Some(j));
    if objs.jobjs[j].flags & jobj::INSTANCE != 0 {
        return;
    }
    let mut child = objs.jobjs[j].child;
    while let Some(c) = child {
        kill_on_tree(objs, data, hooks, Some(c));
        child = objs.jobjs[c].next;
    }
}

/// Removes the generators following a joint, their particles with them if they share their
/// app-SRT, `hsd_8039D688` (which the game passes to `HSD_JObjWalkTree`).
#[doc(alias = "hsd_8039D688")]
pub fn detach_jobj(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, jobj: JObjId) {
    let mut cur = objs.generators.head;
    objs.generators.cursor = None;
    while let Some(g) = cur {
        let next = objs.generators[g].next;
        if objs.generators[g].jobj == Some(jobj) {
            let o = &mut objs.generators[g];
            if o.appsrt.is_some() && o.ty & FOLLOW_JOINT != 0 {
                o.ty |= KILL_PARTICLES;
            }
            let cursor = objs.generators.cursor;
            objs.generators.cursor = remove(objs, data, hooks, g, cursor);
        } else {
            objs.generators.cursor = Some(g);
        }
        cur = next;
    }
}

/// Follows the joint, then turns the generator's velocity (and a line's end) into the
/// joint's frame, `hsd_8039D71C`.
#[doc(alias = "hsd_8039D71C")]
pub fn orient_to_jobj(objs: &mut Objects, data: &Archives, g: GeneratorId) {
    let Some(j) = follow(objs, data, g) else {
        return;
    };
    let m = objs.jobjs[j].mtx.m;
    let col = |c: usize| {
        let v = Vec3::new(copy(m[0][c]), copy(m[1][c]), copy(m[2][c])).sdk_normalize();
        (reg(v.x), reg(v.y), reg(v.z))
    };
    let (c0, c1, c2) = (col(0), col(1), col(2));
    let o = &mut objs.generators[g];
    let (vx, vy, vz) = (reg(o.vel.x), reg(o.vel.y), reg(o.vel.z));
    o.vel.x = mem(fp::fmadds(
        c2.0,
        vz,
        fp::fmadds(c0.0, vx, fp::fmuls(c1.0, vy)),
    ));
    o.vel.y = mem(fp::fmadds(
        c2.1,
        vz,
        fp::fmadds(c0.1, vx, fp::fmuls(c1.1, vy)),
    ));
    o.vel.z = mem(fp::fmadds(
        c2.2,
        vz,
        fp::fmadds(c0.2, vx, fp::fmuls(c1.2, vy)),
    ));
    if o.ty & SHAPE == 1 {
        let (ax, ay, az) = (reg(o.aux[0]), reg(o.aux[1]), reg(o.aux[2]));
        for (r, row) in m.iter().enumerate() {
            o.aux[r] = mem(fp::fmadds(
                reg(row[2]),
                az,
                fp::fmadds(reg(row[0]), ax, fp::fmuls(reg(row[1]), ay)),
            ));
        }
    }
}

/// A new generator, linked in after the cursor's successor (or the head), with a new
/// family id, `hsd_8039D9C8`.
#[doc(alias = "hsd_8039D9C8")]
pub fn alloc(objs: &mut Objects) -> GeneratorId {
    let g = GeneratorId(objs.generators.pool.alloc(Generator::default()));
    let gs = &mut objs.generators;
    gs.live = gs.live.wrapping_add(1);
    if gs.live > gs.peak {
        gs.peak = gs.live;
    }
    match gs.cursor.and_then(|c| gs[c].next) {
        None => match gs.head {
            None => {
                gs[g].next = None;
                gs.head = Some(g);
            }
            Some(h) => {
                gs[g].next = gs[h].next;
                gs[h].next = Some(g);
            }
        },
        Some(c) => {
            gs[g].next = gs[c].next;
            gs[c].next = Some(g);
        }
    }
    let idnum = next_idnum(objs);
    let o = &mut objs.generators[g];
    o.idnum = idnum;
    o.appsrt = None;
    o.userfunc = None;
    g
}

/// An integer converted to single precision as MWCC does it, through the 2^52 bias.
fn int_to_single(i: i32) -> f64 {
    fp::fsubs(biased(i), BIAS)
}

/// The same in double precision.
fn int_to_double(i: i32) -> f64 {
    fp::fsub(biased(i), BIAS)
}

const BIAS: f64 = 4503601774854144.0;

fn biased(i: i32) -> f64 {
    f64::from_bits(0x4330_0000_0000_0000 | u64::from(i as u32 ^ 0x8000_0000))
}

/// The inline `sqrtf`, its result stored and reloaded.
fn sqrtf(x: f64) -> f64 {
    if x > 0.0 {
        reg(mem(math::sqrt_reg(x)))
    } else {
        x
    }
}

/// `x > 0.5` as 1 or 0: a box's sample moved onto a face.
fn snap(v: f32) -> f32 {
    if reg(v) > 0.5 { 1.0 } else { 0.0 }
}

/// An angle toward a direction: `atan2(y, x)`, or a right angle when `x` is too small,
/// testing `x` as stored.
fn angle_to(y: f32, x: f64) -> f64 {
    if particle::abs_stored(x) < FLT_MIN {
        if reg(y) >= 0.0 { HALF_PI_F } else { -HALF_PI_F }
    } else {
        reg(math::atan2(y, mem(x)))
    }
}

/// A draw in a register.
fn randf(objs: &mut Objects) -> f64 {
    reg(objs.rng.draw_f32())
}

const PI: f64 = std::f64::consts::PI;
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
/// `eps` in `hsd_8039DAD4`: 0.001f in double precision.
const SPHERE_EPS: f64 = 0.0010000000474974513;

/// Spawns one of a generator's particles, `hsd_80398F0C` with the generator's settings; a
/// tornado's carry its tilt in their gravity and friction.
fn spawn_particle(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    g: GeneratorId,
    pos: Vec3,
    vel: Vec3,
    tornado: Option<(f32, f32)>,
) {
    let o = &objs.generators[g];
    let (kind, grav, fric) = match tornado {
        Some((tilt_x, tilt_y)) => (o.kind | particle::TORNADO, tilt_x, tilt_y),
        None => (o.kind, o.grav, o.fric),
    };
    let s = Spawn {
        link_no: o.link_no,
        bank: o.bank,
        kind,
        tex_group: u16::from(o.tex_group),
        list: o.cmd_list,
        life: i32::from(o.life),
        palflag: 0,
        pos,
        vel,
        size: o.size,
        grav,
        fric,
        generator: Some(g),
    };
    particle::generate_for(objs, data, hooks, &s);
}

/// The particles a generator's count has reached, one for each whole unit, in its shape;
/// returns the count left, `hsd_8039DAD4`.
///
/// Its machine code keeps the stepped angle in registers that it sets only when the
/// generator's angle is negative, and reads them only then; nothing changes the angle
/// meanwhile, so a read of them unset is counted as a stale read.
#[doc(alias = "hsd_8039DAD4")]
pub fn emit(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, g: GeneratorId) -> f32 {
    let o = &objs.generators[g];
    if reg(o.count) < 1.0 {
        return o.count;
    }
    let vel = Vec3::new(copy(o.vel.x), copy(o.vel.y), copy(o.vel.z));
    let (x, y, z) = (reg(vel.x), reg(vel.y), reg(vel.z));
    let speed = sqrtf(fp::fadds(
        fp::fmuls(z, z),
        fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
    ));
    let mut rot = Mtx34::IDENTITY;
    if o.ty & FOLLOW_JOINT != 0
        && let Some(j) = o.jobj
        && o.ty & JOINT_ROTATION != 0
        && o.kind & 0x30000 == 0
    {
        let jm = objs.jobjs[j].mtx.copied();
        for c in 0..3 {
            let v = Vec3::new(copy(jm.m[0][c]), copy(jm.m[1][c]), copy(jm.m[2][c]));
            let v = v.sdk_normalize();
            rot.m[0][c] = copy(v.x);
            rot.m[1][c] = copy(v.y);
            rot.m[2][c] = copy(v.z);
        }
        rot.m[0][3] = 0.0;
        rot.m[1][3] = 0.0;
        rot.m[2][3] = 0.0;
    }
    // Facing the camera needs `psCamera`, which nothing in the game sets.
    assert!(o.kind & 0x10000 == 0, "psCamera");
    if o.ty & SHAPE != 1 && speed > reg(1.1920929e-7) {
        let v = Vec3::new(copy(o.vel.x), copy(o.vel.y), copy(o.vel.z)).sdk_normalize();
        let az = angle_to(v.y, reg(v.z));
        let sin_az = reg(math::sin(mem(az)));
        let cos_az = reg(math::cos(mem(az)));
        let proj = fp::fmadds(reg(v.y), sin_az, fp::fmuls(reg(v.z), cos_az));
        let el = angle_to(v.x, proj);
        let sin_el = reg(math::sin(mem(el)));
        let cos_el = reg(math::cos(mem(el)));
        let trig = Mtx34 {
            m: [
                [mem(cos_el), 0.0, mem(sin_el), 0.0],
                [
                    mem(fp::fmuls(fp::fneg(sin_az), sin_el)),
                    mem(cos_az),
                    mem(fp::fmuls(sin_az, cos_el)),
                    0.0,
                ],
                [
                    mem(fp::fmuls(fp::fneg(cos_az), sin_el)),
                    mem(fp::fneg(sin_az)),
                    mem(fp::fmuls(cos_az, cos_el)),
                    0.0,
                ],
            ],
        };
        rot = rot.concat(&trig);
    }
    // A tornado's tilt, its particles' gravity and friction.
    let (mut tilt_x, mut tilt_y) = (0.0, 0.0);
    if o.ty & SHAPE == 2 {
        let (r02, r12, r22) = (rot.m[0][2], rot.m[1][2], rot.m[2][2]);
        tilt_x = angle_to(r12, reg(r22));
        let first = fp::fmuls(reg(r22), reg(math::cos(mem(tilt_x))));
        let comb = fp::fmadds(reg(r12), reg(math::sin(mem(tilt_x))), first);
        tilt_y = angle_to(r02, comb);
    }
    let mut cur: Option<f64> = None;
    let mut step: Option<f64> = None;
    if reg(o.angle) < 0.0 {
        match o.ty & SHAPE {
            0 | 3 | 4 | 6 | 7 => {
                let min = reg(o.aux[0]);
                let rnd = randf(objs);
                let o = &objs.generators[g];
                let n = int_to_single(fp::fctiwz(reg(o.count)));
                let s = fp::fdivs(fp::fsubs(reg(o.aux[1]), min), n);
                step = Some(s);
                cur = Some(fp::fmadds(s, rnd, min));
            }
            _ => {
                let rnd = randf(objs);
                let o = &objs.generators[g];
                cur = Some(fp::frsp(fp::fmul(2.0, fp::fmul(PI, rnd))));
                let n = int_to_double(fp::fctiwz(reg(o.count)));
                step = Some(fp::frsp(fp::fdiv(TWO_PI, n)));
            }
        }
    }
    // The angle stepped on, read only where the angle is negative, as set.
    let stepped = |objs: &mut Objects, cur: Option<f64>| match (cur, step) {
        (Some(c), Some(s)) => fp::fadds(c, s),
        _ => {
            objs.stale_reads += 1;
            0.0
        }
    };
    while reg(objs.generators[g].count) >= 1.0 {
        let o = &objs.generators[g];
        match o.ty & SHAPE {
            0 | 3 | 4 | 6 | 7 => {
                let r = reg(o.radius);
                let (dist, radius) = if r < 0.0 {
                    (fp::fneg(r), 1.0)
                } else {
                    let mut f = randf(objs);
                    let shape = objs.generators[g].ty & SHAPE;
                    if shape == 3 || shape == 4 {
                        f = sqrtf(f);
                    }
                    (fp::fmuls(f, reg(objs.generators[g].radius)), f)
                };
                let o = &objs.generators[g];
                let angle = reg(o.angle);
                let cone = match o.ty & SHAPE {
                    6 => {
                        let near = particle::abs_stored(dist) < FLT_MIN;
                        if angle < 0.0 {
                            cur = Some(stepped(objs, cur));
                            let o = &objs.generators[g];
                            if near {
                                if reg(o.aux[2]) >= 0.0 {
                                    fp::fneg(reg(o.angle))
                                } else {
                                    fp::frsp(fp::fsub(PI, reg(o.angle)))
                                }
                            } else {
                                let a = reg(math::atan2(o.aux[2], mem(dist)));
                                fp::frsp(fp::fsub(fp::fsub(HALF_PI, a), reg(o.angle)))
                            }
                        } else {
                            let min = reg(o.aux[0]);
                            let rnd = randf(objs);
                            let o = &objs.generators[g];
                            cur = Some(fp::fmadds(fp::fsubs(reg(o.aux[1]), min), rnd, min));
                            if near {
                                if reg(o.aux[2]) >= 0.0 {
                                    reg(o.angle)
                                } else {
                                    fp::frsp(fp::fadd(PI, reg(o.angle)))
                                }
                            } else {
                                let a = reg(math::atan2(o.aux[2], mem(dist)));
                                fp::frsp(fp::fadd(reg(o.angle), fp::fsub(HALF_PI, a)))
                            }
                        }
                    }
                    7 => {
                        if angle < 0.0 {
                            let c = fp::frsp(fp::fsub(HALF_PI, angle));
                            cur = Some(stepped(objs, cur));
                            c
                        } else {
                            let min = reg(o.aux[0]);
                            let rnd = randf(objs);
                            let o = &objs.generators[g];
                            let c = fp::frsp(fp::fadd(HALF_PI, reg(o.angle)));
                            cur = Some(fp::fmadds(fp::fsubs(reg(o.aux[1]), min), rnd, min));
                            c
                        }
                    }
                    _ => {
                        if angle < 0.0 {
                            let a = fp::fneg(angle);
                            cur = Some(stepped(objs, cur));
                            fp::fmuls(radius, a)
                        } else {
                            let min = reg(o.aux[0]);
                            let rnd = randf(objs);
                            let o = &objs.generators[g];
                            let c = fp::fmuls(radius, reg(o.angle));
                            cur = Some(fp::fmadds(fp::fsubs(reg(o.aux[1]), min), rnd, min));
                            c
                        }
                    }
                };
                let around = mem(cur.expect("set above"));
                let mut e = Vec3::new(
                    mem(fp::fmuls(dist, reg(math::cos(around)))),
                    mem(fp::fmuls(dist, reg(math::sin(around)))),
                    0.0,
                );
                let shape = objs.generators[g].ty & SHAPE;
                if shape == 6 || shape == 7 {
                    e.z = mem(randf(objs));
                    if objs.generators[g].ty & SHAPE == 6 {
                        e.x = mem(fp::fmuls(reg(e.x), fp::fsubs(1.0, reg(e.z))));
                        e.y = mem(fp::fmuls(reg(e.y), fp::fsubs(1.0, reg(e.z))));
                    }
                    e.z = mem(fp::fmuls(reg(e.z), reg(objs.generators[g].aux[2])));
                }
                let lateral = fp::fmuls(speed, reg(math::sin(mem(cone))));
                let mut v = Vec3::new(
                    mem(fp::fmuls(lateral, reg(math::cos(around)))),
                    mem(fp::fmuls(lateral, reg(math::sin(around)))),
                    mem(fp::fmuls(speed, reg(math::cos(mem(cone))))),
                );
                if objs.generators[g].ty & SHAPE == 3 {
                    v.x = mem(fp::fmuls(reg(v.x), radius));
                    v.y = mem(fp::fmuls(reg(v.y), radius));
                    v.z = mem(fp::fmuls(reg(v.z), radius));
                }
                let e = offset(rot.transform(e), objs.generators[g].pos);
                let v = rot.transform(v);
                spawn_particle(objs, data, hooks, g, e, v, None);
            }
            1 => {
                let rnd = randf(objs);
                let o = &objs.generators[g];
                let e = Vec3::new(
                    mem(fp::fmuls(rnd, reg(o.aux[0]))),
                    mem(fp::fmuls(rnd, reg(o.aux[1]))),
                    mem(fp::fmuls(rnd, reg(o.aux[2]))),
                );
                let e = offset(rot.transform(e), o.pos);
                let v = rot.transform(vel);
                spawn_particle(objs, data, hooks, g, e, v, None);
            }
            2 => {
                let dist = if reg(o.radius) < 0.0 {
                    1.0
                } else {
                    randf(objs)
                };
                if reg(objs.generators[g].angle) < 0.0 {
                    cur = Some(stepped(objs, cur));
                } else {
                    let rnd = randf(objs);
                    cur = Some(fp::frsp(fp::fmul(2.0, fp::fmul(PI, rnd))));
                }
                objs.generators[g].aux[0] = mem(speed);
                let v = Vec3::new(mem(cur.expect("set above")), mem(dist), 0.0);
                let tilt = (mem(tilt_x), mem(tilt_y));
                spawn_particle(objs, data, hooks, g, Vec3::default(), v, Some(tilt));
            }
            5 => {
                let mut e = Vec3::new(mem(randf(objs)), mem(randf(objs)), mem(randf(objs)));
                box_face(objs, g, &mut e);
                e.x = mem(fp::fsubs(reg(e.x), 0.5));
                e.y = mem(fp::fsubs(reg(e.y), 0.5));
                e.z = mem(fp::fsubs(reg(e.z), 0.5));
                let o = &objs.generators[g];
                let a = |i: usize| reg(o.aux[i]);
                let (ex, ey, ez) = (reg(e.x), reg(e.y), reg(e.z));
                let t = Vec3::new(
                    mem(fp::fmadds(
                        a(9),
                        ez,
                        fp::fmadds(a(3), ex, fp::fmuls(a(6), ey)),
                    )),
                    mem(fp::fmadds(
                        a(10),
                        ez,
                        fp::fmadds(a(4), ex, fp::fmuls(a(7), ey)),
                    )),
                    mem(fp::fmadds(
                        a(11),
                        ez,
                        fp::fmadds(a(5), ex, fp::fmuls(a(8), ey)),
                    )),
                );
                let e = offset(rot.transform(t), o.pos);
                let mag = sqrtf(fp::fadds(
                    fp::fmuls(a(11), a(11)),
                    fp::fadds(fp::fmuls(a(9), a(9)), fp::fmuls(a(10), a(10))),
                ));
                let scale = fp::fdivs(speed, mag);
                let v = Vec3::new(
                    mem(fp::fmuls(a(9), scale)),
                    mem(fp::fmuls(a(10), scale)),
                    mem(fp::fmuls(a(11), scale)),
                );
                let v = rot.transform(v);
                spawn_particle(objs, data, hooks, g, e, v, None);
            }
            8 => {
                let h = reg(o.aux[2]);
                let lat = if h == 0.0 || fp::fabs(fp::fsub(h, PI)) < SPHERE_EPS {
                    let r = sqrtf(randf(objs));
                    let mut lat = fp::frsp(fp::fmul(HALF_PI, r));
                    if randf(objs) < 0.5 {
                        lat = fp::frsp(fp::fsub(PI, lat));
                    }
                    lat
                } else {
                    let r = sqrtf(randf(objs));
                    fp::fmuls(reg(objs.generators[g].aux[2]), r)
                };
                let rnd = randf(objs);
                let lon = fp::frsp(fp::fmul(2.0, fp::fmul(PI, rnd)));
                let r = reg(objs.generators[g].radius);
                let dist = if r < 0.0 {
                    fp::fneg(r)
                } else {
                    fp::fmuls(r, sqrtf(randf(objs)))
                };
                let (lat, lon) = (mem(lat), mem(lon));
                let n = Vec3::new(
                    mem(fp::fmuls(reg(math::sin(lat)), reg(math::cos(lon)))),
                    mem(fp::fmuls(reg(math::sin(lat)), reg(math::sin(lon)))),
                    math::cos(lat),
                );
                let e = rot.transform(n);
                let o = &objs.generators[g];
                let mut v = Vec3::new(
                    mem(fp::fmuls(reg(e.x), reg(o.aux[0]))),
                    mem(fp::fmuls(reg(e.y), reg(o.aux[0]))),
                    mem(fp::fmuls(reg(e.z), reg(o.aux[0]))),
                );
                if reg(o.radius) >= 0.0 && reg(o.aux[0]) < 0.0 {
                    let s = fp::fdivs(dist, reg(o.radius));
                    v.x = mem(fp::fmuls(reg(v.x), s));
                    v.y = mem(fp::fmuls(reg(v.y), s));
                    v.z = mem(fp::fmuls(reg(v.z), s));
                }
                let e = Vec3::new(
                    mem(fp::fmadds(dist, reg(e.x), reg(o.pos.x))),
                    mem(fp::fmadds(dist, reg(e.y), reg(o.pos.y))),
                    mem(fp::fmadds(dist, reg(e.z), reg(o.pos.z))),
                );
                spawn_particle(objs, data, hooks, g, e, v, None);
            }
            // Shapes past 8 go to `hsd_804D78E8`, which nothing in the game sets.
            _ => {}
        }
        let o = &mut objs.generators[g];
        o.count = mem(fp::fsubs(reg(o.count), 1.0));
    }
    objs.generators[g].count
}

/// A point moved by `pos`, through memory.
fn offset(e: Vec3, pos: Vec3) -> Vec3 {
    Vec3::new(
        mem(fp::fadds(reg(e.x), reg(pos.x))),
        mem(fp::fadds(reg(e.y), reg(pos.y))),
        mem(fp::fadds(reg(e.z), reg(pos.z))),
    )
}

/// Moves a box's sample onto one of the faces its flag names, choosing among them by area.
fn box_face(objs: &mut Objects, g: GeneratorId, e: &mut Vec3) {
    let flag = objs.generators[g].rect_flag;
    // Of two faces, the second axis by `a / (a + b)`.
    let split = |objs: &mut Objects, a: usize, b: usize| {
        let rnd = randf(objs);
        let o = &objs.generators[g];
        let (fa, fb) = (reg(o.aux[a]), reg(o.aux[b]));
        rnd > fp::fdivs(fa, fp::fadds(fa, fb))
    };
    match flag {
        1 => e.x = snap(e.x),
        2 => e.y = snap(e.y),
        3 => {
            if split(objs, 0, 1) {
                e.y = snap(e.y);
            } else {
                e.x = snap(e.x);
            }
        }
        4 => e.z = snap(e.z),
        5 => {
            if split(objs, 0, 2) {
                e.z = snap(e.z);
            } else {
                e.x = snap(e.x);
            }
        }
        6 => {
            if split(objs, 1, 2) {
                e.z = snap(e.z);
            } else {
                e.y = snap(e.y);
            }
        }
        7 => {
            let rnd = randf(objs);
            let o = &objs.generators[g];
            let (x, y, z) = (reg(o.aux[0]), reg(o.aux[1]), reg(o.aux[2]));
            let inv = fp::fdivs(1.0, fp::fmadds(x, fp::fadds(y, z), fp::fmuls(y, z)));
            if rnd < fp::fmuls(inv, fp::fmuls(x, y)) {
                e.z = snap(e.z);
            } else if rnd > fp::fnmsubs(inv, fp::fmuls(x, z), 1.0) {
                e.y = snap(e.y);
            } else {
                e.x = snap(e.x);
            }
        }
        _ => {}
    }
}

/// Runs every generator but those `mask`'s bits 16 and up skip: follows its joint, adds to
/// its count, emits, and ages it, after orienting new ones by their joints, `hsd_8039EE24`.
#[doc(alias = "hsd_8039EE24")]
pub fn run_all(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, mask: u32) {
    while let Some(n) = objs.generators.pending {
        let g = objs.generators.nodes[n.0].data;
        orient_to_jobj(objs, data, g);
        let gs = &mut objs.generators;
        gs.pending = list::remove(&mut gs.nodes, gs.pending);
        if let Some(j) = objs.generators[g].jobj.take() {
            jobj::unref(objs, Some(j));
        }
    }
    let mut cur = objs.generators.head;
    objs.generators.cursor = None;
    while let Some(g) = cur {
        let o = &objs.generators[g];
        let skip = 1u32
            .checked_shl(u32::from(o.link_no) + 16)
            .is_some_and(|bit| mask & bit != 0);
        if skip || o.kind & PAUSED != 0 {
            objs.generators.cursor = Some(g);
            cur = objs.generators[g].next;
            continue;
        }
        follow(objs, data, g);
        let o = &mut objs.generators[g];
        let random = reg(o.random);
        if random < 0.0 {
            o.count = mem(fp::fsubs(reg(o.count), random));
        } else {
            let rnd = randf(objs);
            let o = &mut objs.generators[g];
            o.count = mem(fp::fmadds(reg(o.random), rnd, reg(o.count)));
        }
        if reg(objs.generators[g].count) >= 1.0 {
            let left = emit(objs, data, hooks, g);
            objs.generators[g].count = copy(left);
        }
        let o = &mut objs.generators[g];
        if o.gen_life != 0 {
            o.gen_life -= 1;
            if o.gen_life == 0 {
                let cursor = objs.generators.cursor;
                let cursor = remove(objs, data, hooks, g, cursor);
                objs.generators.cursor = cursor;
                cur = match cursor {
                    Some(c) => objs.generators[c].next,
                    None => objs.generators.head,
                };
                continue;
            }
        }
        objs.generators.cursor = Some(g);
        cur = objs.generators[g].next;
    }
}

/// A generator following `jobj`, referencing it, `hsd_8039EFAC`.
#[doc(alias = "hsd_8039EFAC")]
pub fn spawn_on_jobj(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    link_no: i32,
    bank: i32,
    idx: i32,
    jobj: Option<JObjId>,
) -> Option<GeneratorId> {
    let g = spawn(objs, data, hooks, link_no, bank, idx)?;
    objs.generators[g].jobj = jobj;
    jobj::ref_(&mut objs.jobjs, jobj);
    let o = &mut objs.generators[g];
    o.ty |= if o.kind & 0x20000 != 0 { 0x500 } else { 0x700 };
    Some(g)
}

/// A generator from a bank's command list `idx`, linked in and set up for its shape,
/// `hsd_8039F05C`.
#[doc(alias = "hsd_8039F05C")]
pub fn spawn(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    link_no: i32,
    bank: i32,
    idx: i32,
) -> Option<GeneratorId> {
    if bank >= particle::BANKS as i32 || link_no >= 8 {
        return None;
    }
    // An unloaded bank has no command lists (`psCmdListArray` is 0).
    let b = objs.particles.banks[bank as usize]?;
    if idx >= b.count {
        return None;
    }
    let cl = b.cmd_list(data, idx)?;
    let g = alloc(objs);
    let f = |at| copy(cl.f32(data, at));
    let o = &mut objs.generators[g];
    o.ty = cl.ty(data);
    o.bank = bank as u8;
    o.link_no = link_no as u8;
    o.kind = cl.kind(data);
    o.tex_group = cl.tex_group(data) as u8;
    o.life = cl.life(data);
    o.gen_life = cl.gen_life(data);
    o.pos = Vec3::default();
    o.vel = Vec3::new(f(0x14), f(0x18), f(0x1C));
    o.grav = f(0xC);
    o.fric = f(0x10);
    o.size = f(0x2C);
    o.cmd_list = Some(cl.list());
    o.radius = f(0x20);
    o.angle = f(0x24);
    o.random = f(0x28);
    let random = reg(o.random);
    if o.kind & 0x100 != 0 {
        o.count = if random < 0.0 {
            if fp::fadds(1.0, random) > 1.1920928955078125e-07 {
                1.0
            } else {
                0.0
            }
        } else {
            0.9999999
        };
    } else if random < 0.0 {
        o.count = 0.0;
    } else {
        let rnd = randf(objs);
        objs.generators[g].count = mem(rnd);
    }
    let o = &mut objs.generators[g];
    if let Some(tg) = b.tex_group(data, u32::from(o.tex_group))
        && data.u16(tg.at(0x16)) != 0
    {
        o.kind |= 0x10;
    }
    o.jobj = None;
    o.num_child = 0;
    let p = |i: u32| reg(cl.f32(data, 0x30 + 4 * i));
    match o.ty & SHAPE {
        0 | 3 | 4 | 6 | 7 => {
            if p(0) == 0.0 && p(1) == 0.0 {
                o.aux[0] = 0.0;
                o.aux[1] = 6.2831855;
            } else {
                o.aux[0] = mem(p(0));
                o.aux[1] = mem(p(1));
            }
            if o.ty & SHAPE >= 6 {
                o.aux[2] = mem(p(2));
            }
        }
        1 => {
            for i in 0..3 {
                o.aux[i] = mem(p(i as u32));
            }
        }
        2 => {}
        5 => {
            o.aux = [0.0; 12];
            for i in 0..3 {
                o.aux[i] = mem(p(i as u32));
                o.aux[3 + 4 * i] = mem(p(i as u32));
            }
            o.rect_flag = 0;
            for i in 0..3 {
                if p(i) < 0.0 {
                    o.rect_flag |= 1 << i;
                }
            }
        }
        8 => {
            let (vx, vy, vz) = (reg(o.vel.x), reg(o.vel.y), reg(o.vel.z));
            o.aux[0] = mem(math::sqrt_reg(fp::fadds(
                fp::fmuls(vz, vz),
                fp::fadds(fp::fmuls(vx, vx), fp::fmuls(vy, vy)),
            )));
            let mag = math::sqrt_reg(fp::fadds(fp::fmuls(vx, vx), fp::fmuls(vz, vz)));
            o.aux[1] = if mag < FLT_MIN {
                mem(if vy >= 0.0 { HALF_PI_F } else { -HALF_PI_F })
            } else {
                math::atan2(o.vel.y, mem(mag))
            };
            o.aux[3] = mem(angle_to(o.vel.z, vx));
            o.aux[2] = mem(p(0));
            if reg(o.aux[2]) < 0.0 {
                o.aux[2] = mem(fp::fneg(reg(o.aux[2])));
                o.aux[0] = mem(fp::fneg(reg(o.aux[0])));
            }
            o.aux[4] = mem(p(1));
        }
        // Shapes past 8 go to `hsd_804D78EC`, which nothing in the game sets.
        _ => {}
    }
    if objs.generators[g].kind & 0x20000 != 0 {
        objs.generators[g].ty |= JOINT_APP_TRANSLATE;
        let s = appsrt::add_to_generator(objs, g, 0);
        objs.app_srts[s].xa2 = 1;
        objs.app_srts[s].gp = Some(g);
    }
    if let Some(h) = objs.generators.on_create {
        hooks.generator_created(objs, data, h, g);
    }
    Some(g)
}

/// A generator following `jobj` that also turns into its frame before its first frame,
/// `hsd_8039F6CC`.
#[doc(alias = "hsd_8039F6CC")]
pub fn spawn_oriented_on_jobj(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    link_no: i32,
    bank: i32,
    idx: i32,
    jobj: Option<JObjId>,
) -> Option<GeneratorId> {
    let g = spawn_on_jobj(objs, data, hooks, link_no, bank, idx, jobj)?;
    let gs = &mut objs.generators;
    gs.pending = Some(list::alloc_and_append(&mut gs.nodes, gs.pending, g));
    Some(g)
}
