//! Particles, HAL's particle system (`sysdolphin/baselib/particle.c`): sprites that generators
//! emit, each running a command stream from its effect bank every frame (moving, fading,
//! spawning children and generators, drawing from the game's random numbers) until its life
//! runs out.
//!
//! Effect banks live in loaded game data. The original relocates a bank's offsets, which are
//! relative to the bank, when it loads (`psInitDataBankLocate`), and fixes each command
//! list's draw-order bits; here a bank is resolved as it is read, and the fix is applied to
//! every kind read from it.
//!
//! The display pass (`psdisp.c`) draws from these particles and sorts their lists; its
//! simulation side is in [`super::psdisp`].

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::appsrt::{self, AppSrtId};
use super::generator::{self, GeneratorId};
use super::jobj::{self, JObjId};
use super::pool::Pool;
use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::math;
use crate::vec::Vec3;

/// A particle's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct ParticleId(pub u32);

/// Gravity pulls the velocity down each frame.
pub const GRAVITY: u32 = 1 << 0;
/// Friction scales the velocity each frame.
pub const FRICTION: u32 = 1 << 1;
/// The particle circles its generator (`Tornado`).
pub const TORNADO: u32 = 1 << 2;
pub const TEX_EDGE: u32 = 1 << 3;
pub const COM_TLUT: u32 = 1 << 4;
pub const MIRROR_S: u32 = 1 << 5;
pub const MIRROR_T: u32 = 1 << 6;
pub const PRIM_ENV: u32 = 1 << 7;
pub const TEX_INTERP_NEAR: u32 = 1 << 9;
pub const DISP_TEXTURE: u32 = 1 << 10;
/// Frozen: neither runs nor ages (generators too).
pub const PAUSED: u32 = 1 << 11;
/// Which point joint the particle drags along, with [`POINT_JOBJ`].
pub const POINT_JOBJ_INDEX: u32 = 7 << 12;
pub const POINT_JOBJ: u32 = 1 << 15;
pub const TEX_FLIP_S: u32 = 1 << 18;
pub const TEX_FLIP_T: u32 = 1 << 19;
pub const TRAIL: u32 = 1 << 20;
pub const DIR_VEC: u32 = 1 << 21;
pub const DISP_FOG: u32 = 1 << 24;
pub const NO_Z_COMP: u32 = 1 << 28;
pub const DISP_POINT: u32 = 1 << 30;
pub const DISP_LIGHTING: u32 = 1 << 31;

/// Particle links (`hsd_804D0908`); spawning takes the first 8.
pub const LINKS: usize = 16;
/// Effect banks (`psCmdListArray` and its siblings).
pub const BANKS: usize = 65;
/// Point joints (`hsd_804D08E8`).
pub const POINT_JOBJS: usize = 8;

/// `GXColor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// The game's user functions for a generator's particles (`HSD_PSUserFunc`): a token the
/// game's [`Hooks`] dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct UserFunc(pub u32);

/// A particle's user data array (`userdata`), a token for [`Hooks::write_user_data`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct UserData(pub u32);

/// A particle callback from the game's table (`psCallback`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct Callback(pub u32);

/// The function the game runs on every new generator (`hsd_804D7900`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct GeneratorHook(pub u32);

/// The game's functions the particle system calls, by token. Each runs with the whole object
/// state, as the original's could reach anything; none is stored.
pub trait Hooks {
    /// The user functions' `hookCreate`, if they have one, on a new particle.
    fn hook_create(&mut self, objs: &mut Objects, data: &Archives, f: UserFunc, p: ParticleId);
    /// Their `hookDelete`, if they have one, on a particle about to be freed.
    fn hook_delete(&mut self, objs: &mut Objects, data: &Archives, f: UserFunc, p: ParticleId);
    /// Their `setUserData`; false if they have none.
    fn set_user_data(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        f: UserFunc,
        p: ParticleId,
        index: u8,
        value: f32,
    ) -> bool;
    /// Stores `value` at `index` of a particle's user data.
    fn write_user_data(&mut self, objs: &mut Objects, d: UserData, index: u8, value: f32);
    /// A particle's callback, after it moves: -1 frees it.
    fn callback(&mut self, objs: &mut Objects, data: &Archives, c: Callback, p: ParticleId) -> i32;
    /// `hsd_804D7900` on a new generator.
    fn generator_created(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        h: GeneratorHook,
        g: GeneratorId,
    );
    /// A joint's particles from effect bank `bank`, index `idx`: an animated particle event
    /// (`dptcl_callback`, `dynamic`) or a particle list entry on its first display
    /// (`sptcl_callback`). Joints call it only once the game has set those callbacks
    /// ([`super::jobj::Globals::dptcl`] and `sptcl`).
    fn joint_particles(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        dynamic: bool,
        bank: i32,
        idx: i32,
        jobj: JObjId,
    );
}

/// `HSD_Particle`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Particle {
    pub next: Option<ParticleId>,
    pub kind: u32,
    pub bank: u8,
    pub tex_group: u8,
    pub pose_num: u8,
    pub pal_num: u8,
    pub size_count: u16,
    pub prim_col_count: u16,
    pub env_col_count: u16,
    pub prim_col: Rgba,
    pub env_col: Rgba,
    pub cmd_wait: u16,
    pub loop_count: u8,
    pub link_no: u8,
    pub idnum: u16,
    /// The command stream.
    pub cmd_list: Option<DataRef>,
    pub cmd_ptr: u16,
    pub cmd_mark_ptr: u16,
    pub cmd_loop_ptr: u16,
    pub life: u16,
    pub vel: Vec3,
    pub grav: f32,
    pub fric: f32,
    pub pos: Vec3,
    pub size: f32,
    pub rotate: f32,
    pub a_cmp_count: u16,
    pub a_cmp_mode: u8,
    pub a_cmp_param1: u8,
    pub a_cmp_param2: u8,
    pub p_jobj_ofs: u8,
    pub mat_col_count: u16,
    pub amb_col_count: u16,
    pub rotate_count: u16,
    pub size_target: f32,
    pub rotate_target: f32,
    pub prim_col_remain: u16,
    pub env_col_remain: u16,
    pub prim_col_target: Rgba,
    pub env_col_target: Rgba,
    pub mat_col_remain: u16,
    pub amb_col_remain: u16,
    pub a_cmp_remain: u16,
    pub a_cmp_param1_target: u8,
    pub a_cmp_param2_target: u8,
    pub mat_rgb: u8,
    pub mat_a: u8,
    pub amb_rgb: u8,
    pub amb_a: u8,
    pub mat_rgb_target: u8,
    pub mat_a_target: u8,
    pub amb_rgb_target: u8,
    pub amb_a_target: u8,
    pub trail: f32,
    pub generator: Option<GeneratorId>,
    pub appsrt: Option<AppSrtId>,
    pub userdata: Option<UserData>,
    pub callback: Option<Callback>,
}

/// An effect bank: command lists (`ptclref_804D0E5C`, `psCmdListArray`) and texture groups
/// (`psTexGroupArray`) in loaded data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Bank {
    /// The command bank, which its offsets are relative to.
    pub cmd: DataRef,
    /// Its entry table and the index of its first entry: a bank's indices start at a base.
    pub table: DataRef,
    pub first: i32,
    /// Indices from here on have no command list (`psCmdListArray`).
    pub count: i32,
    /// The texture bank, which its offsets are relative to.
    pub tex: DataRef,
    /// Index remapping for some spawn commands (`hsd_804D0948`).
    pub remap: Option<DataRef>,
    /// Shapes per texture group (`psNumCmdList`), for the display pass.
    pub form: Option<DataRef>,
}

impl Bank {
    /// Command list `idx`, which must be below `count` (`ptclref_804D0E5C[bank][idx]`).
    pub fn cmd_list(&self, data: &Archives, idx: i32) -> Option<CmdList> {
        // An index under the base reads before the table, from memory the bank doesn't own;
        // no data does it.
        assert!(
            idx >= self.first,
            "command list {idx} reads before the bank's table"
        );
        let w = data.u32(self.table.at(4 * (idx - self.first) as u32));
        (w != 0).then(|| CmdList(self.cmd.at(w)))
    }

    /// Texture group `g` (`psTexGroupArray[bank][g]`).
    pub fn tex_group(&self, data: &Archives, g: u32) -> Option<DataRef> {
        // Past the table, the original takes a word the load didn't relocate for an address.
        assert!(
            g < data.u32(self.tex),
            "texture group {g} past the bank's table"
        );
        let w = data.u32(self.tex.at(4 + 4 * g));
        (w != 0).then(|| self.tex.at(w))
    }

    /// The palette flag of a command list's texture group, 0 without one.
    fn palflag(&self, data: &Archives, g: u16) -> u16 {
        self.tex_group(data, u32::from(g))
            .map_or(0, |t| data.u16(t.at(0x16)))
    }
}

/// `HSD_PSCmdList` in a loaded bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CmdList(pub DataRef);

impl CmdList {
    pub fn ty(self, data: &Archives) -> u16 {
        data.u16(self.0)
    }
    pub fn tex_group(self, data: &Archives) -> u16 {
        data.u16(self.0.at(2))
    }
    pub fn gen_life(self, data: &Archives) -> u16 {
        data.u16(self.0.at(4))
    }
    pub fn life(self, data: &Archives) -> u16 {
        data.u16(self.0.at(6))
    }
    /// The kind, with the draw order `psInitDataBankLocate` gives every loaded list.
    pub fn kind(self, data: &Archives) -> u32 {
        data.u32(self.0.at(8)) & 0xF1FF_FFFF | 0x0800_0000
    }
    /// The float at `offset`: `grav` (0xC), `fric`, `vx`, `vy`, `vz`, `radius`, `angle`,
    /// `random`, `size`, `param1` to `param3` (0x38).
    pub fn f32(self, data: &Archives, offset: u32) -> f32 {
        data.f32(self.0.at(offset))
    }
    /// The command stream.
    pub fn list(self) -> DataRef {
        self.0.at(0x3C)
    }
}

/// The particle pool (`hsd_804D0F60`) and particle.c's globals.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Particles {
    pub pool: Pool<Particle, { pool_kind(Kind::Particle) }>,
    /// Each link's particles (`hsd_804D0908`).
    pub links: [Option<ParticleId>; LINKS],
    /// Live particles (`hsd_804D78E2`) and the most at once (`numPeakParticles`).
    pub live: u16,
    pub peak: u16,
    pub banks: [Option<Bank>; BANKS],
    /// Joints particles aim at and drag (`hsd_804D08E8`), each referenced.
    pub point_jobjs: [Option<JObjId>; POINT_JOBJS],
    /// The callback table (`psCallback`): empty, since the game never sets one.
    pub callbacks: Vec<Callback>,
}

impl Default for Particles {
    fn default() -> Self {
        Particles {
            pool: Pool::default(),
            links: [None; LINKS],
            live: 0,
            peak: 0,
            banks: [None; BANKS],
            point_jobjs: [None; POINT_JOBJS],
            callbacks: Vec::new(),
        }
    }
}

impl Index<ParticleId> for Particles {
    type Output = Particle;
    fn index(&self, p: ParticleId) -> &Particle {
        &self.pool[p.0]
    }
}

impl IndexMut<ParticleId> for Particles {
    fn index_mut(&mut self, p: ParticleId) -> &mut Particle {
        &mut self.pool[p.0]
    }
}

/// Registers a bank's data, `psInitDataBankLoad`. `remap` and `form` are null in every call
/// the game makes.
#[doc(alias = "psInitDataBankLoad")]
pub fn load_data_bank(
    objs: &mut Objects,
    data: &Archives,
    bank: usize,
    cmd: DataRef,
    tex: DataRef,
    remap: Option<DataRef>,
    form: Option<DataRef>,
) {
    if let Some(f) = form {
        assert_eq!(
            data.u32(f),
            data.u32(tex),
            "illigal form data (strange number of group)"
        );
    }
    let (table, first, count) = match data.u16(cmd) {
        0 => {
            // Version 0 relocates even a null entry, to the bank itself, then fixes the kind
            // bits of the bank's own table there; no bank on the disc is version 0.
            let n = data.u32(cmd.at(4));
            assert!(
                (0..n).all(|i| data.u32(cmd.at(8 + 4 * i)) != 0),
                "a version 0 bank with a null entry"
            );
            (cmd.at(8), 0, n as i32)
        }
        0x40..=0x43 => {
            let first = data.u32(cmd.at(4)) as i32;
            let n = data.u32(cmd.at(8)) as i32;
            (cmd.at(12), first, n.wrapping_add(first))
        }
        _ => panic!("psInitDataBanks: unknown version"),
    };
    objs.particles.banks[bank] = Some(Bank {
        cmd,
        table,
        first,
        count,
        tex,
        remap,
        form: form.map(|f| f.at(4)),
    });
}

/// Locates and registers a bank, `psInitDataBank` (see the module's notes on locating).
#[doc(alias = "psInitDataBank")]
pub fn init_data_bank(
    objs: &mut Objects,
    data: &Archives,
    bank: usize,
    cmd: DataRef,
    tex: DataRef,
    remap: Option<DataRef>,
    form: Option<DataRef>,
) {
    if bank < BANKS {
        load_data_bank(objs, data, bank, cmd, tex, remap, form);
    }
}

/// Empties the particle system, `hsd_80398A08`. Point joints are dropped without releasing
/// their references, as in the original.
#[doc(alias = "hsd_80398A08")]
pub fn init(objs: &mut Objects) {
    objs.particles = Particles::default();
}

/// What `psGenerateParticle0` takes for a new particle.
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct Spawn {
    pub link_no: u8,
    pub bank: u8,
    pub kind: u32,
    pub tex_group: u16,
    pub list: Option<DataRef>,
    pub life: i32,
    pub palflag: i32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub grav: f32,
    pub fric: f32,
    pub generator: Option<GeneratorId>,
}

/// Makes a particle at the front of its link, or right after `after`, and runs its first
/// commands if `interpret`, `psGenerateParticle0`. The particle may be gone again once they
/// have run.
#[doc(alias = "psGenerateParticle0")]
pub fn generate(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    after: Option<ParticleId>,
    s: &Spawn,
    interpret: bool,
) -> ParticleId {
    let p = ParticleId(objs.particles.pool.alloc(Particle::default()));
    let ps = &mut objs.particles;
    ps.live = ps.live.wrapping_add(1);
    if ps.live > ps.peak {
        ps.peak = ps.live;
    }
    let idnum = match s.generator {
        Some(g) => objs.generators[g].idnum,
        None => generator::next_idnum(objs),
    };
    objs.particles[p].idnum = idnum;
    if let Some(g) = s.generator
        && let Some(srt) = objs.generators[g].appsrt
    {
        appsrt::attach_particle(objs, p, Some(srt));
    }
    let ps = &mut objs.particles;
    match after {
        None => {
            ps.pool[p.0].next = ps.links[s.link_no as usize];
            ps.links[s.link_no as usize] = Some(p);
        }
        Some(a) => {
            ps.pool[p.0].next = ps[a].next;
            ps[a].next = Some(p);
        }
    }
    let o = &mut ps[p];
    o.bank = s.bank;
    o.link_no = s.link_no;
    o.kind = s.kind;
    o.tex_group = s.tex_group as u8;
    o.pos = Vec3::new(copy(s.pos.x), copy(s.pos.y), copy(s.pos.z));
    o.vel = Vec3::new(copy(s.vel.x), copy(s.vel.y), copy(s.vel.z));
    o.size = copy(s.size);
    o.grav = copy(s.grav);
    o.fric = copy(s.fric);
    o.life = s.life.wrapping_add(1) as u16;
    o.cmd_list = s.list;
    if s.palflag != 0 {
        o.kind |= COM_TLUT;
    }
    o.cmd_wait = u16::from(s.list.is_some());
    o.pal_num = 0xFF;
    o.prim_col = Rgba {
        r: 0xFF,
        g: 0xFF,
        b: 0xFF,
        a: 0xFF,
    };
    o.a_cmp_mode = 0x33;
    o.a_cmp_param1 = 1;
    o.a_cmp_param2 = 0xFF;
    o.generator = s.generator;
    if let Some(g) = s.generator {
        let owner = &mut objs.generators[g];
        owner.num_child = owner.num_child.wrapping_add(1);
    }
    let o = &mut objs.particles[p];
    o.mat_rgb = 0xFF;
    o.mat_a = 0xFF;
    o.amb_rgb = 0xFF;
    o.amb_a = 0xFF;
    o.trail = 1.0;
    if let Some(f) = s.generator.and_then(|g| objs.generators[g].userfunc) {
        hooks.hook_create(objs, data, f, p);
    }
    objs.particles[p].callback = None;
    if interpret {
        execute(objs, data, hooks, p, None);
    }
    p
}

/// A generator's particle, run at once, `hsd_80398F0C`.
#[doc(alias = "hsd_80398F0C")]
pub fn generate_for(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, s: &Spawn) {
    generate(objs, data, hooks, None, s, true);
}

/// A float's magnitude through memory, as the original masks a stored float's sign.
pub(crate) fn abs_stored(r: f64) -> f64 {
    reg(f32::from_bits(fp::stfs(r) & 0x7FFF_FFFF))
}

/// `FLT_MIN` and pi/2 as singles: atan2's callers test against the first and use the second
/// where the divisor is too small.
pub(crate) const FLT_MIN: f64 = 1.1754943508222875e-38;
pub(crate) const HALF_PI_F: f64 = 1.5707963705062866;

/// Turns the velocity by `angle` from where it points, in a random direction around it,
/// `hsd_80398F8C`.
#[doc(alias = "hsd_80398F8C")]
pub fn scatter(objs: &mut Objects, p: ParticleId, angle: f64) {
    let o = &objs.particles[p];
    let (mut vx, vz, vy) = (reg(o.vel.x), reg(o.vel.z), reg(o.vel.y));
    let azimuth = if abs_stored(vz) < FLT_MIN {
        if vy >= 0.0 { HALF_PI_F } else { -HALF_PI_F }
    } else {
        reg(math::atan2(mem(vy), mem(vz)))
    };
    let sin_a = reg(math::sin(mem(azimuth)));
    let cos_a = reg(math::cos(mem(azimuth)));
    let temp = fp::fmadds(vy, sin_a, fp::fmuls(vz, cos_a));
    let elevation = if abs_stored(temp) < FLT_MIN {
        if vx >= 0.0 { HALF_PI_F } else { -HALF_PI_F }
    } else {
        reg(math::atan2(mem(vx), mem(temp)))
    };
    let sin_e = reg(math::sin(mem(elevation)));
    let cos_e = reg(math::cos(mem(elevation)));
    vx = fp::fmadds(vz, vz, fp::fmadds(vx, vx, fp::fmuls(vy, vy)));
    if vx > 0.0 {
        vx = math::sqrt_reg(vx);
    }
    let rnd = reg(objs.rng.draw_f32());
    let rand_angle = fp::frsp(fp::fmul(2.0, fp::fmul(std::f64::consts::PI, rnd)));
    let sin_angle = fp::fmuls(vx, reg(math::sin(mem(angle))));
    let cos_rand = fp::fmuls(sin_angle, reg(math::cos(mem(rand_angle))));
    let sin_rand = fp::fmuls(sin_angle, reg(math::sin(mem(rand_angle))));
    let cos_angle = fp::fmuls(vx, reg(math::cos(mem(angle))));
    let v = &mut objs.particles[p].vel;
    v.x = mem(fp::fmadds(cos_rand, cos_e, fp::fmuls(cos_angle, sin_e)));
    v.y = mem(fp::fmadds(
        cos_e,
        fp::fmuls(cos_angle, sin_a),
        fp::fmadds(
            sin_e,
            fp::fmuls(fp::fneg(cos_rand), sin_a),
            fp::fmuls(sin_rand, cos_a),
        ),
    ));
    v.z = mem(fp::fmadds(
        cos_e,
        fp::fmuls(cos_angle, cos_a),
        fp::fmsubs(
            sin_e,
            fp::fmuls(fp::fneg(cos_rand), cos_a),
            fp::fmuls(sin_rand, sin_a),
        ),
    ));
}

/// Pulls a particle toward a joint with `force` over the squared distance; true once it is
/// within `range`, `hsd_803991D8`. The original takes a generator, but its only caller passes
/// a particle, whose position and velocity lie where the generator's fields it uses are.
#[doc(alias = "hsd_803991D8")]
pub fn attract(
    objs: &mut Objects,
    data: &Archives,
    p: ParticleId,
    jobj: Option<JObjId>,
    force: f64,
    range: f64,
) -> bool {
    let Some(j) = jobj else { return false };
    if range < 0.0 {
        return false;
    }
    jobj::setup_matrix(objs, data, Some(j));
    let m = objs.jobjs[j].mtx.m;
    let o = &mut objs.particles[p];
    let dx = fp::fsubs(reg(m[0][3]), reg(o.pos.x));
    let dy = fp::fsubs(reg(m[1][3]), reg(o.pos.y));
    let dz = fp::fsubs(reg(m[2][3]), reg(o.pos.z));
    let dist_sq = fp::fmadds(dz, dz, fp::fmadds(dx, dx, fp::fmuls(dy, dy)));
    if dist_sq <= fp::fmuls(range, range) {
        return true;
    }
    if dist_sq == 0.0 {
        return false;
    }
    let scale = fp::fdivs(force, dist_sq);
    o.vel.x = mem(fp::fmadds(scale, dx, reg(o.vel.x)));
    o.vel.y = mem(fp::fmadds(scale, dy, reg(o.vel.y)));
    o.vel.z = mem(fp::fmadds(scale, dz, reg(o.vel.z)));
    false
}

/// A command stream being read.
struct Stream {
    list: DataRef,
    pc: u32,
}

impl Stream {
    fn u8(&mut self, data: &Archives) -> u8 {
        let b = data.u8(self.list.at(self.pc));
        self.pc += 1;
        b
    }

    /// A big-endian 16-bit index.
    fn index(&mut self, data: &Archives) -> i32 {
        let hi = i32::from(self.u8(data)) << 8;
        hi + i32::from(self.u8(data))
    }

    /// A float as `psReadFloat` assembles it, in a register.
    fn float(&mut self, data: &Archives) -> f64 {
        let b = [self.u8(data), self.u8(data), self.u8(data), self.u8(data)];
        fp::lfs(u32::from_be_bytes(b))
    }

    /// A count: one byte, or two when the first has its top bit set.
    fn count(&mut self, data: &Archives) -> u16 {
        let c = u16::from(self.u8(data));
        if c & 0x80 != 0 {
            ((c & 0x7F) << 8) + u16::from(self.u8(data))
        } else {
            c
        }
    }
}

/// A draw from the game's random numbers, in a register (`HSD_Randf`).
fn randf(objs: &mut Objects) -> f64 {
    reg(objs.rng.draw_f32())
}

/// A color channel partway to its target: `remain` of `count` steps left.
fn lerp_channel(cur: u8, target: u8, step: i32) -> u8 {
    ((i32::from(target) << 16).wrapping_add(step.wrapping_mul(i32::from(cur) - i32::from(target)))
        >> 16) as u8
}

/// A color's current value, as far along as its countdown, before a new target replaces it.
fn settle(cur: &mut Rgba, target: Rgba, remain: u16, count: u16) {
    if count != 0 {
        let step = (i32::from(remain) << 16) / i32::from(count);
        cur.r = lerp_channel(cur.r, target.r, step);
        cur.g = lerp_channel(cur.g, target.g, step);
        cur.b = lerp_channel(cur.b, target.b, step);
        cur.a = lerp_channel(cur.a, target.a, step);
    }
}

/// A channel moved by `delta` and clamped to 0..=255.
fn offset_channel(c: u8, delta: f64) -> u8 {
    // Like the original's two tests, `clamp` leaves a NaN as it is.
    fp::fctiwz(fp::fadds(fp::frsp(f64::from(c)), delta).clamp(0.0, 255.0)) as u8
}

/// A random step for a color channel: `delta` doubled, scaled by a fresh draw.
fn random_delta(objs: &mut Objects, s: &mut Stream, data: &Archives) -> f64 {
    let r = randf(objs);
    let delta = s.u8(data) as i8;
    fp::fmuls(fp::frsp(f64::from(i32::from(delta) << 1)), r)
}

/// Where a spawn command reads its command list: a remapped index, if the bank has a map.
fn remapped(objs: &Objects, data: &Archives, bank: u8, idx: i32) -> i32 {
    match objs.particles.banks[bank as usize].and_then(|b| b.remap) {
        Some(r) => data.u32(r.at(4 * idx as u32)) as i32,
        None => idx,
    }
}

fn bank_of(objs: &Objects, bank: u8) -> &Bank {
    objs.particles.banks[bank as usize]
        .as_ref()
        .expect("a particle from a bank that isn't loaded")
}

/// A child particle from command list `idx` of `bank`, right after `parent`, unrun (the
/// inline `psSpawnChild`).
fn spawn_child(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    parent: ParticleId,
    link_no: u8,
    bank: u8,
    idx: i32,
) -> Option<ParticleId> {
    if link_no >= 8 || bank as usize >= BANKS {
        return None;
    }
    // An unloaded bank has no command lists (`psCmdListArray` is 0).
    let b = objs.particles.banks[bank as usize]?;
    if idx >= b.count {
        return None;
    }
    let cl = b.cmd_list(data, idx)?;
    let tex_group = cl.tex_group(data);
    let s = Spawn {
        link_no,
        bank,
        kind: cl.kind(data),
        tex_group,
        list: Some(cl.list()),
        life: i32::from(cl.life(data)),
        palflag: i32::from(b.palflag(data, tex_group)),
        pos: Vec3::default(),
        vel: Vec3::new(cl.f32(data, 0x14), cl.f32(data, 0x18), cl.f32(data, 0x1C)),
        size: cl.f32(data, 0x2C),
        grav: cl.f32(data, 0xC),
        fric: cl.f32(data, 0x10),
        generator: None,
    };
    Some(generate(objs, data, hooks, Some(parent), &s, false))
}

/// Gives a spawned child its parent's family, transform and place (and velocity, with
/// `with_vel`), then runs it.
fn adopt(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    pp: ParticleId,
    c: ParticleId,
    with_vel: bool,
) {
    let (pos, vel, idnum, owner, srt) = {
        let o = &objs.particles[pp];
        (o.pos, o.vel, o.idnum, o.generator, o.appsrt)
    };
    let co = &mut objs.particles[c];
    co.pos = Vec3::new(copy(pos.x), copy(pos.y), copy(pos.z));
    if with_vel {
        co.vel = Vec3::new(copy(vel.x), copy(vel.y), copy(vel.z));
    }
    co.idnum = idnum;
    co.generator = owner;
    if let Some(g) = owner {
        let owner = &mut objs.generators[g];
        owner.num_child = owner.num_child.wrapping_add(1);
    }
    appsrt::attach_particle(objs, c, srt);
    execute(objs, data, hooks, c, Some(pp));
}

/// A generator from command list `idx`, given the particle's family, joint and transform,
/// and placed where the particle is; `kind_bits` replaces its draw order (commands EF, F0).
fn spawn_generator(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    pp: ParticleId,
    idx: i32,
    bank: u8,
    kind_bits: Option<u8>,
) {
    let link_no = objs.particles[pp].link_no;
    let Some(g) = generator::spawn(objs, data, hooks, i32::from(link_no), i32::from(bank), idx)
    else {
        return;
    };
    let (idnum, owner, srt) = {
        let o = &objs.particles[pp];
        (o.idnum, o.generator, o.appsrt)
    };
    objs.generators[g].idnum = idnum;
    if let Some(pg) = owner {
        let j = objs.generators[pg].jobj;
        objs.generators[g].jobj = j;
        jobj::ref_(&mut objs.jobjs, j);
    }
    objs.generators[g].ty |= 0x100;
    if let Some(pg) = owner {
        let inherited = objs.generators[pg].ty & 0x1E00;
        let o = &mut objs.generators[g];
        o.ty |= inherited;
        if o.kind & (1 << 17) != 0 {
            o.ty &= !(1 << 9);
        }
    }
    if srt.is_some() {
        appsrt::attach_generator(objs, g, srt);
    }
    if let Some(bits) = kind_bits {
        let o = &mut objs.generators[g];
        o.kind &= 0xF1FF_FFFF;
        o.kind |= u32::from(bits & 7) << 25;
    }
    let pos = objs.particles[pp].pos;
    let pos = Vec3::new(copy(pos.x), copy(pos.y), copy(pos.z));
    match (srt, objs.generators[g].appsrt) {
        (Some(_), None) => {}
        (Some(ps), Some(gs)) if ps == gs => objs.generators[g].pos = pos,
        (Some(ps), Some(gs)) => {
            let t = objs.app_srts[ps].translate;
            objs.app_srts[gs].translate = Vec3::new(copy(t.x), copy(t.y), copy(t.z));
        }
        (None, Some(gs)) => objs.app_srts[gs].translate = pos,
        (None, None) => objs.generators[g].pos = pos,
    }
}

/// Points the particle's velocity at a joint, keeping its speed (command B7).
fn aim(objs: &mut Objects, data: &Archives, pp: ParticleId, j: JObjId) {
    jobj::setup_matrix(objs, data, Some(j));
    let m = objs.jobjs[j].mtx.m;
    let o = &mut objs.particles[pp];
    let val = fp::fadds(
        fp::fmuls(reg(o.vel.x), reg(o.vel.x)),
        fp::fmuls(reg(o.vel.y), reg(o.vel.y)),
    );
    let mut speed = fp::fmuls(reg(o.vel.z), reg(o.vel.z));
    speed = fp::fadds(speed, val);
    let dx = fp::fsubs(reg(m[0][3]), reg(o.pos.x));
    let dy = fp::fsubs(reg(m[1][3]), reg(o.pos.y));
    let dz = fp::fsubs(reg(m[2][3]), reg(o.pos.z));
    if speed > 0.0 {
        speed = math::sqrt_reg(speed);
    }
    let val = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
    let mut dist = fp::fmadds(dz, dz, val);
    if dist == 0.0 {
        return;
    }
    if dist > 0.0 {
        dist = math::sqrt_reg(dist);
    }
    let scale = fp::fdivs(speed, dist);
    o.vel.x = mem(fp::fmuls(dx, scale));
    o.vel.y = mem(fp::fmuls(dy, scale));
    o.vel.z = mem(fp::fmuls(dz, scale));
}

/// A point joint by a command's index.
fn point_jobj(objs: &Objects, index: usize) -> Option<JObjId> {
    // Past the table the original reads the particle links as joints; no data does it.
    assert!(index < POINT_JOBJS, "point joint {index} past the table");
    objs.particles.point_jobjs[index]
}

/// Sets the texture bit if the particle's pose has a texture (the inline `psEnableTexture`).
fn enable_texture(objs: &mut Objects, data: &Archives, pp: ParticleId) {
    let o = &objs.particles[pp];
    let b = bank_of(objs, o.bank);
    if let Some(tg) = b.tex_group(data, u32::from(o.tex_group)) {
        let at = tg.at(0x18 + 4 * u32::from(o.pose_num));
        // The load relocates the table's non-null entries, so only null stays null.
        if data.get(at.archive).is_pointer(at.offset) || data.u32(at) != 0 {
            objs.particles[pp].kind |= DISP_TEXTURE;
        }
    }
}

/// Adds to a joint's translation and dirties it (the inline `HSD_JObjAddTranslation*`).
fn add_translation(objs: &mut Objects, j: JObjId, axis: usize, d: f64) {
    let o = &mut objs.jobjs[j];
    let t = match axis {
        0 => &mut o.translate.x,
        1 => &mut o.translate.y,
        _ => &mut o.translate.z,
    };
    *t = mem(fp::fadds(reg(*t), d));
    if o.flags & jobj::MTX_INDEP_SRT == 0 {
        jobj::set_mtx_dirty(&mut objs.jobjs, Some(j));
    }
}

/// Runs a particle's frame: its color, size and rotation fades, its commands when their wait
/// is over, its aging, and its motion; `prev` is the particle before it in its link. Returns
/// the particle to run next, `hsd_8039930C`.
#[doc(alias = "hsd_8039930C")]
pub fn execute(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    pp: ParticleId,
    prev: Option<ParticleId>,
) -> Option<ParticleId> {
    if objs.particles[pp].kind & PAUSED != 0 {
        return objs.particles[pp].next;
    }
    fade(&mut objs.particles[pp]);
    let o = &mut objs.particles[pp];
    if o.cmd_wait != 0 {
        o.cmd_wait -= 1;
        if o.cmd_wait == 0 {
            run_commands(objs, data, hooks, pp);
        }
    }
    let o = &mut objs.particles[pp];
    o.life = o.life.wrapping_sub(1);
    if o.life == 0 {
        return free(objs, data, hooks, pp, prev);
    }
    motion(objs, pp);
    drag_point_jobj(objs, data, pp);
    if let Some(c) = objs.particles[pp].callback
        && hooks.callback(objs, data, c, pp) == -1
    {
        return free(objs, data, hooks, pp, prev);
    }
    objs.particles[pp].next
}

/// The countdowns `hsd_8039930C` runs before the commands.
fn fade(o: &mut Particle) {
    if o.size_count != 0 {
        o.size = mem(fp::fadds(
            reg(o.size),
            fp::fdivs(
                fp::fsubs(reg(o.size_target), reg(o.size)),
                fp::frsp(f64::from(o.size_count)),
            ),
        ));
        o.size_count -= 1;
    }
    if o.prim_col_count != 0 {
        o.prim_col_remain = o.prim_col_remain.wrapping_sub(1);
        if o.prim_col_remain == 0 {
            o.prim_col_count = 0;
            o.prim_col = o.prim_col_target;
        }
    }
    if o.env_col_count != 0 {
        o.env_col_remain = o.env_col_remain.wrapping_sub(1);
        if o.env_col_remain == 0 {
            o.env_col_count = 0;
            o.env_col = o.env_col_target;
        }
    }
    if o.mat_col_count != 0 {
        o.mat_col_remain = o.mat_col_remain.wrapping_sub(1);
        if o.mat_col_remain == 0 {
            o.mat_col_count = 0;
            o.mat_rgb = o.mat_rgb_target;
            o.mat_a = o.mat_a_target;
        }
    }
    if o.amb_col_count != 0 {
        o.amb_col_remain = o.amb_col_remain.wrapping_sub(1);
        if o.amb_col_remain == 0 {
            o.amb_col_count = 0;
            o.amb_rgb = o.amb_rgb_target;
            o.amb_a = o.amb_a_target;
        }
    }
    if o.a_cmp_count != 0 {
        o.a_cmp_remain = o.a_cmp_remain.wrapping_sub(1);
        if o.a_cmp_remain == 0 {
            o.a_cmp_count = 0;
            o.a_cmp_param1 = o.a_cmp_param1_target;
            o.a_cmp_param2 = o.a_cmp_param2_target;
        }
    }
    if o.rotate_count != 0 {
        o.rotate = mem(fp::fadds(
            reg(o.rotate),
            fp::fdivs(
                fp::fsubs(reg(o.rotate_target), reg(o.rotate)),
                fp::frsp(f64::from(o.rotate_count)),
            ),
        ));
        o.rotate_count -= 1;
    }
}

/// Frees a particle that has aged out or that its callback ends, unlinking it after `prev`;
/// returns the particle to run next.
fn free(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    pp: ParticleId,
    prev: Option<ParticleId>,
) -> Option<ParticleId> {
    if let Some(f) = objs.particles[pp]
        .generator
        .and_then(|g| objs.generators[g].userfunc)
    {
        hooks.hook_delete(objs, data, f, pp);
    }
    let o = &objs.particles[pp];
    let (next, link) = (o.next, o.link_no as usize);
    match prev {
        None => objs.particles.links[link] = next,
        Some(pr) => objs.particles[pr].next = next,
    }
    let mut next_pp = next;
    if let Some(g) = objs.particles[pp].generator {
        let owner = &mut objs.generators[g];
        owner.num_child = owner.num_child.wrapping_sub(1);
    }
    if objs.particles[pp].appsrt.is_some()
        && appsrt::remove_from_particle(objs, pp) == 0
        && prev.is_none()
    {
        let head = objs.particles.links[link];
        if head != next_pp {
            next_pp = head;
        }
    }
    release_point_jobj(objs, pp);
    objs.particles.pool.free(pp.0);
    objs.particles.live = objs.particles.live.wrapping_sub(1);
    next_pp
}

/// The particle's motion: around its generator if it is a tornado, else by its velocity,
/// gravity and friction.
fn motion(objs: &mut Objects, pp: ParticleId) {
    let o = &objs.particles[pp];
    if o.kind & TORNADO == 0 {
        let o = &mut objs.particles[pp];
        if o.kind & GRAVITY != 0 {
            o.vel.y = mem(fp::fsubs(reg(o.vel.y), reg(o.grav)));
        }
        if o.kind & FRICTION != 0 {
            o.vel.x = mem(fp::fmuls(reg(o.vel.x), reg(o.fric)));
            o.vel.y = mem(fp::fmuls(reg(o.vel.y), reg(o.fric)));
            o.vel.z = mem(fp::fmuls(reg(o.vel.z), reg(o.fric)));
        }
        o.pos.x = mem(fp::fadds(reg(o.pos.x), reg(o.vel.x)));
        o.pos.y = mem(fp::fadds(reg(o.pos.y), reg(o.vel.y)));
        o.pos.z = mem(fp::fadds(reg(o.pos.z), reg(o.vel.z)));
        return;
    }
    // A tornado's velocity holds its angle around the axis (x), its distance factor (y) and
    // its height (z); gravity and friction hold the axis's tilt.
    let gp = o
        .generator
        .expect("a tornado particle without its generator");
    let sin_a = reg(math::sin(o.grav));
    let sin_b = reg(math::sin(o.fric));
    let cos_a = reg(math::cos(o.grav));
    let cos_b = reg(math::cos(o.fric));
    let g = &objs.generators[gp];
    let (aux_vel, radius, angle, ggrav, gpos) =
        (g.aux[0], reg(g.radius), reg(g.angle), g.grav, g.pos);
    let o = &mut objs.particles[pp];
    o.vel.z = mem(fp::fadds(reg(o.vel.z), reg(aux_vel)));
    let mut r = if radius < 0.0 {
        fp::fneg(radius)
    } else {
        radius
    };
    let ang = if angle < 0.0 { fp::fneg(angle) } else { angle };
    r = fp::fmadds(reg(o.vel.z), reg(math::tan(mem(ang))), r);
    r = fp::fmuls(r, reg(o.vel.y));
    o.vel.x = mem(fp::fadds(reg(o.vel.x), reg(ggrav)));
    let d = fp::fmuls(r, reg(math::cos(o.vel.x)));
    let e = fp::fmuls(r, reg(math::sin(o.vel.x)));
    let nd = fp::fneg(d);
    let vz = reg(o.vel.z);
    o.pos.x = mem(fp::fadds(
        reg(gpos.x),
        fp::fmadds(d, cos_b, fp::fmuls(vz, sin_b)),
    ));
    o.pos.y = mem(fp::fadds(
        reg(gpos.y),
        fp::fmadds(
            cos_b,
            fp::fmuls(vz, sin_a),
            fp::fmadds(sin_b, fp::fmuls(nd, sin_a), fp::fmuls(e, cos_a)),
        ),
    ));
    o.pos.z = mem(fp::fadds(
        reg(gpos.z),
        fp::fmadds(
            cos_b,
            fp::fmuls(vz, cos_a),
            fp::fmsubs(sin_b, fp::fmuls(nd, cos_a), fp::fmuls(e, sin_a)),
        ),
    ));
}

/// Moves the particle's point joint, making one if its slot is empty, to the particle.
fn drag_point_jobj(objs: &mut Objects, data: &Archives, pp: ParticleId) {
    let kind = objs.particles[pp].kind;
    if kind & POINT_JOBJ == 0 {
        return;
    }
    let idx = ((kind & POINT_JOBJ_INDEX) >> 12) as usize;
    if objs.particles.point_jobjs[idx].is_none() {
        let j = jobj::alloc(objs);
        set_point_jobj(objs, idx as i32 + 1, Some(j));
        jobj::unref(objs, Some(j));
    }
    if objs.particles.point_jobjs[idx].is_none() {
        return;
    }
    jobj::setup_matrix(objs, data, objs.particles.point_jobjs[idx]);
    let pos = objs.particles[pp].pos;
    for (axis, p) in [pos.x, pos.y, pos.z].into_iter().enumerate() {
        let j = objs.particles.point_jobjs[idx].expect("the point joint");
        let d = fp::fsubs(reg(p), reg(objs.jobjs[j].mtx.m[axis][3]));
        add_translation(objs, j, axis, d);
    }
}

/// Runs a particle's commands until one waits or ends it, saving where the next run picks up.
fn run_commands(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, pp: ParticleId) {
    let mut operand: u16;
    let o = &objs.particles[pp];
    let mut s = Stream {
        list: o.cmd_list.expect("commands without a command list"),
        pc: u32::from(o.cmd_ptr),
    };
    loop {
        let opcode = s.u8(data);
        if opcode < 0x80 {
            operand = u16::from(opcode & 0x1F);
            if opcode & 0x20 != 0 {
                operand = (operand << 8) + u16::from(s.u8(data));
            }
            if opcode & 0xC0 == 0x40 {
                objs.particles[pp].pose_num = s.u8(data);
                enable_texture(objs, data, pp);
            }
        } else {
            operand = 0;
            let mut cls = opcode & 0xF8;
            if cls > 0x98 {
                cls = opcode & 0xF0;
                if cls != 0xC0 && cls != 0xD0 {
                    cls = opcode;
                }
            }
            if !command(objs, data, hooks, pp, &mut s, opcode, cls) {
                objs.particles[pp].life = 1;
                break;
            }
        }
        if operand != 0 {
            break;
        }
    }
    let o = &mut objs.particles[pp];
    o.cmd_ptr = s.pc as u16;
    o.cmd_wait = operand;
}

/// One of `hsd_8039930C`'s commands from 0x80 up; false where the command ends the particle.
fn command(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    pp: ParticleId,
    s: &mut Stream,
    opcode: u8,
    cls: u8,
) -> bool {
    match cls {
        0x80 | 0x88 | 0x90 | 0x98 => {
            for axis in 0..3 {
                if opcode & (1 << axis) == 0 {
                    continue;
                }
                let v = s.float(data);
                let o = &mut objs.particles[pp];
                let target = if cls <= 0x88 { &mut o.pos } else { &mut o.vel };
                let c = match axis {
                    0 => &mut target.x,
                    1 => &mut target.y,
                    _ => &mut target.z,
                };
                *c = if cls & 8 == 0 {
                    mem(v)
                } else {
                    mem(fp::fadds(reg(*c), v))
                };
            }
        }
        0xA0 => {
            let count = s.count(data);
            let v = s.float(data);
            let o = &mut objs.particles[pp];
            o.size_count = count;
            o.size_target = mem(v);
            if o.size_count == 0 {
                o.size = copy(o.size_target);
            }
        }
        0xA1 => objs.particles[pp].kind &= !DISP_TEXTURE,
        0xA2 => {
            let v = s.float(data);
            let o = &mut objs.particles[pp];
            o.grav = mem(v);
            if reg(o.grav) == 0.0 {
                o.kind &= !GRAVITY;
            } else {
                o.kind |= GRAVITY;
            }
        }
        0xA3 => {
            let v = s.float(data);
            let o = &mut objs.particles[pp];
            o.fric = mem(v);
            if reg(o.fric) == 1.0 {
                o.kind &= !FRICTION;
            } else {
                o.kind |= FRICTION;
            }
        }
        0xA4 | 0xF1 | 0xB9 | 0xF2 => {
            let (link_no, bank) = (objs.particles[pp].link_no, objs.particles[pp].bank);
            let mut idx = s.index(data);
            if cls == 0xF1 || cls == 0xF2 {
                idx = remapped(objs, data, bank, idx);
            }
            if let Some(c) = spawn_child(objs, data, hooks, pp, link_no, bank, idx) {
                adopt(objs, data, hooks, pp, c, cls == 0xB9 || cls == 0xF2);
            }
        }
        0xA5 | 0xEF | 0xF0 => {
            let idx = s.index(data);
            let bits = (cls != 0xA5).then(|| s.u8(data));
            let bank = objs.particles[pp].bank;
            let idx = if cls == 0xF0 {
                remapped(objs, data, bank, idx)
            } else {
                idx
            };
            spawn_generator(objs, data, hooks, pp, idx, bank, bits);
        }
        0xA6 => {
            let base = s.index(data);
            let range = s.index(data);
            let r = randf(objs);
            let add = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(range)), r));
            objs.particles[pp].life = base.wrapping_add(add) as u16;
        }
        0xA7 => {
            let threshold = i32::from(s.u8(data));
            if threshold >= fp::fctiwz(fp::fmuls(100.0, randf(objs))) {
                return false;
            }
        }
        0xA8 => {
            for axis in 0..3 {
                let v = s.float(data);
                let d = fp::fmsubs(fp::fmuls(2.0, v), randf(objs), v);
                let p = &mut objs.particles[pp].pos;
                let c = match axis {
                    0 => &mut p.x,
                    1 => &mut p.y,
                    _ => &mut p.z,
                };
                *c = mem(fp::fadds(reg(*c), d));
            }
        }
        0xA9 => {
            let v = s.float(data);
            scatter(objs, pp, v);
        }
        0xAA => {
            let mut idx = s.index(data);
            let range = s.index(data);
            idx = idx.wrapping_add(fp::fctiwz(fp::fmuls(
                fp::frsp(f64::from(range)),
                randf(objs),
            )));
            let (link_no, bank) = (objs.particles[pp].link_no, objs.particles[pp].bank);
            let idx = remapped(objs, data, bank, idx);
            if let Some(c) = spawn_child(objs, data, hooks, pp, link_no, bank, idx) {
                adopt(objs, data, hooks, pp, c, false);
            }
        }
        0xAB => {
            let scale = s.float(data);
            let v = &mut objs.particles[pp].vel;
            v.x = mem(fp::fmuls(reg(v.x), scale));
            v.y = mem(fp::fmuls(reg(v.y), scale));
            v.z = mem(fp::fmuls(reg(v.z), scale));
        }
        0xAC => {
            let count = s.count(data);
            let target = s.float(data);
            let range = s.float(data);
            let r = randf(objs);
            let o = &mut objs.particles[pp];
            o.size_count = count;
            o.size_target = mem(target);
            o.size_target = mem(fp::fmadds(range, r, reg(o.size_target)));
            if o.size_count == 0 {
                o.size = copy(o.size_target);
            }
        }
        0xAD => objs.particles[pp].kind |= PRIM_ENV,
        0xAE => objs.particles[pp].kind &= !(MIRROR_S | MIRROR_T),
        0xAF => {
            let o = &mut objs.particles[pp];
            o.kind &= !MIRROR_T;
            o.kind |= MIRROR_S;
        }
        0xB0 => {
            let o = &mut objs.particles[pp];
            o.kind &= !MIRROR_S;
            o.kind |= MIRROR_T;
        }
        0xB1 => objs.particles[pp].kind |= MIRROR_S | MIRROR_T,
        0xB2 => detach_app_srt(objs, data, pp),
        0xB3 => {
            let o = &mut objs.particles[pp];
            if o.a_cmp_count != 0 {
                let step = (i32::from(o.a_cmp_remain) << 16) / i32::from(o.a_cmp_count);
                o.a_cmp_param1 = lerp_channel(o.a_cmp_param1, o.a_cmp_param1_target, step);
                o.a_cmp_param2 = lerp_channel(o.a_cmp_param2, o.a_cmp_param2_target, step);
            }
            let count = s.count(data);
            let (mode, p1, p2) = (s.u8(data), s.u8(data), s.u8(data));
            let o = &mut objs.particles[pp];
            o.a_cmp_count = count;
            o.a_cmp_mode = mode;
            o.a_cmp_param1_target = p1;
            o.a_cmp_param2_target = p2;
            if o.a_cmp_count == 0 {
                o.a_cmp_param1 = o.a_cmp_param1_target;
                o.a_cmp_param2 = o.a_cmp_param2_target;
                o.a_cmp_remain = 0;
                o.a_cmp_count = 0;
            } else {
                o.a_cmp_remain = o.a_cmp_count;
            }
        }
        0xB4 => objs.particles[pp].kind |= TEX_INTERP_NEAR,
        0xB5 => objs.particles[pp].kind &= !TEX_INTERP_NEAR,
        0xB6 => {
            let count = s.count(data);
            let v = s.float(data);
            let o = &mut objs.particles[pp];
            o.rotate_count = count;
            o.rotate_target = mem(fp::fadds(reg(o.rotate_target), v));
            if o.rotate_count == 0 {
                o.rotate = copy(o.rotate_target);
            }
        }
        0xB7 => {
            let index = usize::from(s.u8(data)) + usize::from(objs.particles[pp].p_jobj_ofs);
            if let Some(j) = point_jobj(objs, index) {
                aim(objs, data, pp, j);
            }
        }
        0xB8 => {
            let index = usize::from(s.u8(data)) + usize::from(objs.particles[pp].p_jobj_ofs);
            let force = s.float(data);
            let range = s.float(data);
            let j = point_jobj(objs, index);
            if attract(objs, data, pp, j, force, range) {
                return false;
            }
        }
        0xBA | 0xBB => {
            let o = &mut objs.particles[pp];
            let (cur, target, remain, count) = if cls == 0xBA {
                (
                    &mut o.prim_col,
                    o.prim_col_target,
                    o.prim_col_remain,
                    o.prim_col_count,
                )
            } else {
                (
                    &mut o.env_col,
                    o.env_col_target,
                    o.env_col_remain,
                    o.env_col_count,
                )
            };
            settle(cur, target, remain, count);
            let mut t = target;
            for c in [&mut t.r, &mut t.g, &mut t.b, &mut t.a] {
                let d = random_delta(objs, s, data);
                *c = offset_channel(*c, d);
            }
            let o = &mut objs.particles[pp];
            if cls == 0xBA {
                o.prim_col_target = t;
                if o.prim_col_count == 0 {
                    o.prim_col = o.prim_col_target;
                } else {
                    o.prim_col_remain = o.prim_col_count;
                }
            } else {
                o.env_col_target = t;
                if o.env_col_count == 0 {
                    o.env_col = o.env_col_target;
                } else {
                    o.env_col_remain = o.env_col_count;
                }
            }
        }
        0xBC => {
            let pose = s.u8(data);
            let range = fp::frsp(f64::from(s.u8(data)));
            let r = randf(objs);
            let o = &mut objs.particles[pp];
            o.pose_num = pose;
            o.pose_num = fp::fctiwz(fp::fmadds(range, r, fp::frsp(f64::from(o.pose_num)))) as u8;
            enable_texture(objs, data, pp);
        }
        0xBD => {
            let base = s.float(data);
            let range = s.float(data);
            let mut speed = fp::fmadds(range, randf(objs), base);
            let v = objs.particles[pp].vel;
            let mut mag = fp::fadds(
                fp::fmuls(reg(v.z), reg(v.z)),
                fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y))),
            );
            if mag > 0.0 {
                mag = math::sqrt_reg(mag);
            }
            if mag > 1.000000013351432e-10 {
                speed = fp::fdivs(speed, mag);
                let v = &mut objs.particles[pp].vel;
                v.x = mem(fp::fmuls(reg(v.x), speed));
                v.y = mem(fp::fmuls(reg(v.y), speed));
                v.z = mem(fp::fmuls(reg(v.z), speed));
            }
        }
        0xBE => {
            for axis in 0..3 {
                let f = s.float(data);
                let v = &mut objs.particles[pp].vel;
                let c = match axis {
                    0 => &mut v.x,
                    1 => &mut v.y,
                    _ => &mut v.z,
                };
                *c = mem(fp::fmuls(reg(*c), f));
            }
        }
        0xBF => {
            let idx = u32::from(s.u8(data));
            let o = &mut objs.particles[pp];
            o.kind |= ((idx + u32::from(o.p_jobj_ofs)) & 7) << 12 | POINT_JOBJ;
        }
        0xC0 | 0xD0 => {
            let o = &mut objs.particles[pp];
            let (cur, target, remain, count) = if cls == 0xC0 {
                (
                    &mut o.prim_col,
                    o.prim_col_target,
                    o.prim_col_remain,
                    o.prim_col_count,
                )
            } else {
                (
                    &mut o.env_col,
                    o.env_col_target,
                    o.env_col_remain,
                    o.env_col_count,
                )
            };
            settle(cur, target, remain, count);
            let count = s.count(data);
            let o = &mut objs.particles[pp];
            let mut t = if cls == 0xC0 { o.prim_col } else { o.env_col };
            for (bit, c) in [&mut t.r, &mut t.g, &mut t.b, &mut t.a]
                .into_iter()
                .enumerate()
            {
                if opcode & (1 << bit) != 0 {
                    *c = s.u8(data);
                }
            }
            let (cur, target, remain, cnt) = if cls == 0xC0 {
                (
                    &mut o.prim_col,
                    &mut o.prim_col_target,
                    &mut o.prim_col_remain,
                    &mut o.prim_col_count,
                )
            } else {
                (
                    &mut o.env_col,
                    &mut o.env_col_target,
                    &mut o.env_col_remain,
                    &mut o.env_col_count,
                )
            };
            *cnt = count;
            *target = t;
            if *cnt == 0 {
                *cur = *target;
                *remain = 0;
            } else {
                *remain = *cnt;
            }
        }
        0xE0 => {
            let o = &mut objs.particles[pp];
            settle(
                &mut o.prim_col,
                o.prim_col_target,
                o.prim_col_remain,
                o.prim_col_count,
            );
            settle(
                &mut o.env_col,
                o.env_col_target,
                o.env_col_remain,
                o.env_col_count,
            );
            for channel in 0..4 {
                let d = random_delta(objs, s, data);
                let o = &mut objs.particles[pp];
                let (p, e) = match channel {
                    0 => (&mut o.prim_col_target.r, &mut o.env_col_target.r),
                    1 => (&mut o.prim_col_target.g, &mut o.env_col_target.g),
                    2 => (&mut o.prim_col_target.b, &mut o.env_col_target.b),
                    _ => (&mut o.prim_col_target.a, &mut o.env_col_target.a),
                };
                *p = offset_channel(*p, d);
                *e = offset_channel(*e, d);
            }
            sync_colors(&mut objs.particles[pp]);
        }
        0xE9 => {
            let o = &mut objs.particles[pp];
            settle(
                &mut o.prim_col,
                o.prim_col_target,
                o.prim_col_remain,
                o.prim_col_count,
            );
            settle(
                &mut o.env_col,
                o.env_col_target,
                o.env_col_remain,
                o.env_col_count,
            );
            let flags = s.u8(data);
            let timing = i32::from(s.u8(data));
            let scale = if timing != 0 {
                let r = randf(objs);
                fp::fdivs(
                    fp::frsp(f64::from(fp::fctiwz(fp::fmuls(
                        fp::frsp(f64::from(timing + 1)),
                        r,
                    )))),
                    fp::frsp(f64::from(timing)),
                )
            } else {
                randf(objs)
            };
            for channel in 0..4 {
                if flags & (1 << channel) == 0 {
                    continue;
                }
                let d = if channel < 3 {
                    let delta = s.u8(data) as i8;
                    fp::fmuls(scale, fp::frsp(f64::from(i32::from(delta) << 1)))
                } else {
                    // Alpha draws its own scale.
                    let a = randf(objs);
                    let delta = s.u8(data) as i8;
                    let a = fp::frsp(f64::from(fp::fctiwz(fp::fmuls(
                        fp::frsp(f64::from(timing + 1)),
                        a,
                    ))));
                    fp::fdivs(
                        fp::fmuls(fp::frsp(f64::from(i32::from(delta) << 1)), a),
                        fp::frsp(f64::from(timing)),
                    )
                };
                let o = &mut objs.particles[pp];
                let (p, e) = match channel {
                    0 => (&mut o.prim_col_target.r, &mut o.env_col_target.r),
                    1 => (&mut o.prim_col_target.g, &mut o.env_col_target.g),
                    2 => (&mut o.prim_col_target.b, &mut o.env_col_target.b),
                    _ => (&mut o.prim_col_target.a, &mut o.env_col_target.a),
                };
                if flags & 0x10 != 0 {
                    *p = offset_channel(*p, d);
                }
                if flags & 0x20 != 0 {
                    *e = offset_channel(*e, d);
                }
            }
            sync_colors(&mut objs.particles[pp]);
        }
        0xE1 => {
            let idx = s.u8(data);
            objs.particles[pp].callback = if idx == 0 {
                None
            } else {
                // The game never sets a table, so the original faults here.
                Some(
                    *objs
                        .particles
                        .callbacks
                        .get(usize::from(idx) - 1)
                        .expect("psCallback is null"),
                )
            };
        }
        0xEC => {
            let idx = s.u8(data);
            let v = mem(s.float(data));
            let o = &objs.particles[pp];
            let g = o.generator.expect("pp->gen");
            let userdata = o.userdata;
            let handled = match objs.generators[g].userfunc {
                Some(f) => hooks.set_user_data(objs, data, f, pp, idx, v),
                None => false,
            };
            if !handled && let Some(d) = userdata {
                hooks.write_user_data(objs, d, idx, v);
            }
        }
        0xE2 => objs.particles[pp].kind |= TEX_EDGE,
        0xE3 => objs.particles[pp].pal_num = s.u8(data),
        0xE4 | 0xE5 => {
            let bit = if cls == 0xE4 { TEX_FLIP_S } else { TEX_FLIP_T };
            match s.u8(data) & 3 {
                0 => objs.particles[pp].kind &= !bit,
                1 => objs.particles[pp].kind |= bit,
                2 => objs.particles[pp].kind ^= bit,
                _ => {
                    if randf(objs) < 0.5 {
                        objs.particles[pp].kind &= !bit;
                    } else {
                        objs.particles[pp].kind |= bit;
                    }
                }
            }
        }
        0xE6 => objs.particles[pp].kind |= DIR_VEC,
        0xE7 => objs.particles[pp].kind &= !DIR_VEC,
        0xE8 => {
            let trail = s.float(data);
            let o = &mut objs.particles[pp];
            if trail < 0.0 {
                o.kind &= !TRAIL;
            } else {
                o.kind |= TRAIL;
                o.trail = mem(trail);
            }
        }
        0xEA | 0xEB => {
            let o = &mut objs.particles[pp];
            let (rgb, a, rgb_t, a_t, remain, count) = if cls == 0xEA {
                (
                    &mut o.mat_rgb,
                    &mut o.mat_a,
                    &mut o.mat_rgb_target,
                    &mut o.mat_a_target,
                    &mut o.mat_col_remain,
                    &mut o.mat_col_count,
                )
            } else {
                (
                    &mut o.amb_rgb,
                    &mut o.amb_a,
                    &mut o.amb_rgb_target,
                    &mut o.amb_a_target,
                    &mut o.amb_col_remain,
                    &mut o.amb_col_count,
                )
            };
            if *count != 0 {
                let step = (i32::from(*remain) << 16) / i32::from(*count);
                *rgb = lerp_channel(*rgb, *rgb_t, step);
                *a = lerp_channel(*a, *a_t, step);
            }
            *count = s.count(data);
            let flags = s.u8(data);
            *rgb_t = *rgb;
            if flags & 1 != 0 {
                *rgb_t = s.u8(data);
            }
            if flags & 8 != 0 {
                *a_t = s.u8(data);
            }
            if *count == 0 {
                *rgb = *rgb_t;
                *remain = 0;
            } else {
                *remain = *count;
            }
        }
        0xED => {
            let mut base = s.float(data);
            let range = s.float(data);
            let timing = i32::from(s.u8(data));
            if timing != 0 {
                let randi = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(timing + 1)), randf(objs)));
                base = fp::fadds(
                    base,
                    fp::fdivs(
                        fp::fmuls(range, fp::frsp(f64::from(randi))),
                        fp::frsp(f64::from(timing)),
                    ),
                );
            } else {
                base = fp::fmadds(range, randf(objs), base);
            }
            let o = &mut objs.particles[pp];
            o.rotate_target = mem(fp::fadds(reg(o.rotate_target), base));
            o.rotate = mem(fp::fadds(reg(o.rotate), base));
        }
        0xFA => {
            let o = &mut objs.particles[pp];
            o.loop_count = s.u8(data);
            o.cmd_loop_ptr = s.pc as u16;
        }
        0xFB => {
            let o = &mut objs.particles[pp];
            o.loop_count = o.loop_count.wrapping_sub(1);
            if o.loop_count != 0 {
                s.pc = u32::from(o.cmd_loop_ptr);
            }
        }
        0xFC => objs.particles[pp].cmd_mark_ptr = s.pc as u16,
        0xFD => s.pc = u32::from(objs.particles[pp].cmd_mark_ptr),
        0xFE | 0xFF => return false,
        _ => {}
    }
    true
}

/// Primary and environment colors after a random step: current where there is no fade,
/// the countdowns restarted (commands E0, E9).
fn sync_colors(o: &mut Particle) {
    if o.prim_col_count == 0 {
        o.prim_col = o.prim_col_target;
    }
    o.prim_col_remain = o.prim_col_count;
    if o.env_col_count == 0 {
        o.env_col = o.env_col_target;
    }
    o.env_col_remain = o.env_col_count;
}

/// Moves the particle into its app-SRT's frame and drops the app-SRT (command B2).
fn detach_app_srt(objs: &mut Objects, data: &Archives, pp: ParticleId) {
    let Some(srt) = objs.particles[pp].appsrt else {
        return;
    };
    if objs.app_srts[srt].xa2 != 0 {
        return;
    }
    generator::follow_joint_unless_paused(objs, data, objs.app_srts[srt].gp);
    let srt = objs.particles[pp].appsrt.expect("the app-SRT");
    let a = &mut objs.app_srts[srt];
    let rot = Vec3::new(a.rot.x, a.rot.y, a.rot.z);
    a.mmtx = crate::mtx::Mtx34::srt(a.scale, rot, a.translate, None);
    let m = a.mmtx.m;
    let p = &mut objs.particles[pp].pos;
    for (r, row) in m.iter().enumerate() {
        let v = mem(fp::fadds(
            reg(row[3]),
            fp::fmadds(
                reg(row[2]),
                reg(p.z),
                fp::fmadds(reg(row[0]), reg(p.x), fp::fmuls(reg(row[1]), reg(p.y))),
            ),
        ));
        match r {
            0 => p.x = v,
            1 => p.y = v,
            _ => p.z = v,
        }
    }
    appsrt::remove_from_particle(objs, pp);
}

/// Runs every link's particles but those `mask`'s bits 16 and up skip, `hsd_8039CEAC`.
#[doc(alias = "hsd_8039CEAC")]
pub fn execute_all(objs: &mut Objects, data: &Archives, hooks: &mut dyn Hooks, mask: u32) {
    let mut bits = mask;
    for i in 0..LINKS {
        if bits & 0x10000 == 0 {
            let mut cur = objs.particles.links[i];
            let mut prev: Option<ParticleId> = None;
            while let Some(c) = cur {
                let next = execute(objs, data, hooks, c, prev);
                // The particle stays the one before the next unless it unlinked itself.
                let unlinked = match prev {
                    None => objs.particles.links[i] == next,
                    Some(p) => objs.particles[p].next == next,
                };
                if !unlinked {
                    prev = cur;
                }
                cur = next;
            }
        }
        bits >>= 1;
    }
}

/// Sets point joint `index - 1`, referencing it, or with 0 clears every slot holding `jobj`,
/// `hsd_8039CF4C`. Replacing a joint unreferences the new one rather than the old, as the
/// original does.
#[doc(alias = "hsd_8039CF4C")]
pub fn set_point_jobj(objs: &mut Objects, index: i32, jobj: Option<JObjId>) {
    if !(0..=8).contains(&index) {
        return;
    }
    if index != 0 {
        let slot = (index - 1) as usize;
        let old = objs.particles.point_jobjs[slot];
        if old != jobj {
            if old.is_some() {
                jobj::unref(objs, jobj);
            }
            objs.particles.point_jobjs[slot] = jobj;
            jobj::ref_(&mut objs.jobjs, jobj);
        }
    } else {
        for i in 0..POINT_JOBJS {
            if objs.particles.point_jobjs[i] == jobj {
                jobj::unref(objs, objs.particles.point_jobjs[i]);
                objs.particles.point_jobjs[i] = None;
            }
        }
    }
}

/// Releases the point joint a particle drags, `hsd_8039D048`.
#[doc(alias = "hsd_8039D048")]
pub fn release_point_jobj(objs: &mut Objects, pp: ParticleId) {
    let kind = objs.particles[pp].kind;
    if kind & POINT_JOBJ != 0 {
        let slot = ((kind >> 12) & 7) as usize;
        if let Some(j) = objs.particles.point_jobjs[slot] {
            jobj::unref(objs, Some(j));
            objs.particles.point_jobjs[slot] = None;
        }
    }
}

/// Frees a generator's particles in its link, `hsd_8039D0A0`.
#[doc(alias = "hsd_8039D0A0")]
pub fn kill_generator_particles(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    g: GeneratorId,
) {
    let (idnum, link) = (
        objs.generators[g].idnum,
        objs.generators[g].link_no as usize,
    );
    let mut prev: Option<ParticleId> = None;
    let mut cur = objs.particles.links[link];
    while let Some(p) = cur {
        let next = objs.particles[p].next;
        if objs.particles[p].idnum == idnum && objs.particles[p].generator == Some(g) {
            if let Some(f) = objs.generators[g].userfunc {
                hooks.hook_delete(objs, data, f, p);
            }
            match prev {
                None => objs.particles.links[link] = objs.particles[p].next,
                Some(pr) => objs.particles[pr].next = objs.particles[p].next,
            }
            let owner = &mut objs.generators[g];
            owner.num_child = owner.num_child.wrapping_sub(1);
            if objs.particles[p].appsrt.is_some() {
                appsrt::remove_from_particle(objs, p);
            }
            release_point_jobj(objs, p);
            objs.particles.pool.free(p.0);
            objs.particles.live = objs.particles.live.wrapping_sub(1);
        } else {
            prev = Some(p);
        }
        cur = next;
    }
}
