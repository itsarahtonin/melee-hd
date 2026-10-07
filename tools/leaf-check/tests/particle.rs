//! The particle system against ssbm-rs: every effect bank on the disc (the effect files' and
//! the stages'), and random banks built to reach every command, generator shape and branch,
//! loaded on both sides; generators of every command list spawned plain, on joints and
//! oriented by joints; then frame after frame of particle and generator runs and display
//! sorting, with random kills, joint moves, point joints, app-SRT edits and user functions,
//! comparing every particle, generator and app-SRT, the random seed and the live counts after
//! each step, in both float modes.

use std::cell::RefCell;
use std::collections::HashMap;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{Rand, arg, handle, hsd_reference, sym};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::Objects;
use melee_sim::hsd::appsrt::{self, AppSrtId};
use melee_sim::hsd::generator::{self, GeneratorId};
use melee_sim::hsd::jobj::{self, JObjId};
use melee_sim::hsd::particle::{
    self, Callback, GeneratorHook, Hooks, ParticleId, UserData, UserFunc,
};
use melee_sim::hsd::psdisp;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];

/// Native stand-ins for the game's functions, at free addresses.
const NATIVE_CREATE: u32 = 0x817E_0400;
const NATIVE_DELETE: u32 = 0x817E_0410;
const NATIVE_SET_UD: u32 = 0x817E_0420;
const NATIVE_CB1: u32 = 0x817E_0430;
const NATIVE_CB2: u32 = 0x817E_0440;
/// Test user functions: create and delete hooks (A), and a `setUserData` too (B).
const UF_A: u32 = 0x817E_2000;
const UF_B: u32 = 0x817E_2010;
/// A callback table (`psCallback`), for the random banks.
const CALLBACKS: u32 = 0x817E_2100;
/// Where `particleSort` returns its lists.
const OUT: u32 = 0x817E_2200;
/// A particle's user data, 256 floats.
const USERDATA: u32 = 0x817E_3000;
/// Archives: their `HSD_Archive`s, then their files.
const ARCHIVE_STRUCTS: u32 = 0x8070_0000;
const FILES: u32 = 0x8080_0000;
const FILES_END: u32 = 0x80F0_0000;

/// Our tokens for the same.
const EF_HOOK: GeneratorHook = melee_sim::game::ef::PTCL_APPSRT_HOOK;
const UF_EF: UserFunc = melee_sim::game::ef::PARTICLE_RENDER_FUNCS;
const UF_A_T: UserFunc = UserFunc(1);
const UF_B_T: UserFunc = UserFunc(2);
const USERDATA_T: UserData = UserData(0);
const CB1: Callback = Callback(1);
const CB2: Callback = Callback(2);

/// A particle's user data starts as this test's array for one family in three.
fn gets_userdata(idnum: u16) -> bool {
    idnum.is_multiple_of(3)
}

/// The test callbacks end a particle whose x position's bits are a multiple of 5 (or 7).
fn callback_kills(c: u32, pos_x: u32) -> bool {
    pos_x.is_multiple_of(if c == 1 { 5 } else { 7 })
}

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(s: String) {
    LOG.with(|l| l.borrow_mut().push(s));
}

fn native_create(ctx: &Ctx) {
    let p = ctx.regs.r(3);
    let idnum = ctx.read_u16(p + 0x1E);
    log(format!("create {idnum} {:#x}", ctx.read_u32(p + 4)));
    if gets_userdata(idnum) {
        ctx.write_u32(p + 0x90, USERDATA);
    }
    ctx.regs.set_r(3, 0);
}

fn native_delete(ctx: &Ctx) {
    let p = ctx.regs.r(3);
    log(format!(
        "delete {} {} {:#x}",
        ctx.read_u16(p + 0x1E),
        ctx.read_u16(p + 0x2A),
        ctx.read_u32(p + 0x40)
    ));
    ctx.regs.set_r(3, 0);
}

fn native_set_ud(ctx: &Ctx) {
    let p = ctx.regs.r(3);
    let idx = ctx.regs.r(4) & 0xFF;
    let v = gekko_fp::stfs(ctx.regs.f(1));
    log(format!("setud {} {idx} {v:#x}", ctx.read_u16(p + 0x1E)));
    ctx.write_u32(USERDATA + 4 * idx, v);
    ctx.regs.set_r(3, 0);
}

fn native_cb(ctx: &Ctx, c: u32) {
    let p = ctx.regs.r(3);
    let x = ctx.read_u32(p + 0x40);
    log(format!("cb{c} {} {x:#x}", ctx.read_u16(p + 0x1E)));
    ctx.regs
        .set_r(3, if callback_kills(c, x) { u32::MAX } else { 0 });
}

fn native_cb1(ctx: &Ctx) {
    native_cb(ctx, 1);
}

fn native_cb2(ctx: &Ctx) {
    native_cb(ctx, 2);
}

/// Our side of the game's functions: efLib's app-SRT hook and particle render hook, as the
/// game has them, and the test's.
struct TestHooks {
    userdata: Vec<f32>,
    log: Vec<String>,
}

impl Hooks for TestHooks {
    fn hook_create(&mut self, objs: &mut Objects, _: &Archives, f: UserFunc, p: ParticleId) {
        if f == UF_EF {
            // efLib_Cb_ParticleRender, the game's port.
            melee_sim::game::ef::cb_particle_render(objs, p);
            return;
        }
        let o = &objs.particles[p];
        self.log.push(format!("create {} {:#x}", o.idnum, o.kind));
        if gets_userdata(o.idnum) {
            objs.particles[p].userdata = Some(USERDATA_T);
        }
    }

    fn hook_delete(&mut self, objs: &mut Objects, _: &Archives, f: UserFunc, p: ParticleId) {
        if f != UF_EF {
            let o = &objs.particles[p];
            self.log.push(format!(
                "delete {} {} {:#x}",
                o.idnum,
                o.life,
                o.pos.x.to_bits()
            ));
        }
    }

    fn set_user_data(
        &mut self,
        objs: &mut Objects,
        _: &Archives,
        f: UserFunc,
        p: ParticleId,
        index: u8,
        value: f32,
    ) -> bool {
        if f != UF_B_T {
            return false;
        }
        self.log.push(format!(
            "setud {} {index} {:#x}",
            objs.particles[p].idnum,
            value.to_bits()
        ));
        self.userdata[usize::from(index)] = value;
        true
    }

    fn write_user_data(&mut self, _: &mut Objects, d: UserData, index: u8, value: f32) {
        assert_eq!(d, USERDATA_T);
        self.userdata[usize::from(index)] = value;
    }

    fn callback(&mut self, objs: &mut Objects, _: &Archives, c: Callback, p: ParticleId) -> i32 {
        let o = &objs.particles[p];
        let x = o.pos.x.to_bits();
        self.log.push(format!("cb{} {} {x:#x}", c.0, o.idnum));
        if callback_kills(c.0, x) { -1 } else { 0 }
    }

    fn generator_created(
        &mut self,
        objs: &mut Objects,
        data: &Archives,
        h: GeneratorHook,
        g: GeneratorId,
    ) {
        assert_eq!(h, EF_HOOK);
        // efLib_Cb_PtclAppSRTHook, the game's port.
        melee_sim::game::ef::cb_ptcl_appsrt_hook(objs, data, g);
    }

    fn joint_particles(
        &mut self,
        _: &mut Objects,
        _: &Archives,
        _: bool,
        _: i32,
        _: i32,
        _: JObjId,
    ) {
        unreachable!("the test's joints have no particle events")
    }
}

/// A bank to load: its archive and where its command and texture banks are.
#[derive(Clone)]
struct BankSpec {
    bank: usize,
    archive: usize,
    cmd: u32,
    tex: u32,
    remap: Option<u32>,
    form: Option<u32>,
    /// Located when its file loads and registered later (`psInitDataBankLocate`, then
    /// `psInitDataBankLoad`), as stages do, rather than by `psInitDataBank`.
    game_way: bool,
}

struct Sides<'c> {
    ctx: &'c Ctx,
    archives: Archives,
    /// Each archive's data address on their side, size and id.
    bases: Vec<(u32, u32, ArchiveId)>,
    objs: Objects,
    hooks: TestHooks,
    /// The test's joints, each held by the test.
    joints: Vec<(u32, JObjId)>,
    /// Banks loaded: number, first index and index count.
    banks: Vec<(usize, i32, i32)>,
    jobj_baseline: (u32, u32),
}

/// Runs a step on both sides, the original first; where it halts, ours must too.
fn both(what: &str, theirs: impl FnOnce(), ours: impl FnOnce()) -> bool {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    if catch_unwind(AssertUnwindSafe(theirs)).is_ok() {
        ours();
        return true;
    }
    assert!(
        catch_unwind(AssertUnwindSafe(ours)).is_err(),
        "{what}: the original halts here, ours carries on"
    );
    false
}

const NONE: u32 = u32::MAX;
const DANGLING: u32 = u32::MAX - 1;

impl Sides<'_> {
    fn dref_theirs(&self, addr: u32) -> (u32, u32) {
        if addr == 0 {
            return (NONE, 0);
        }
        for &(base, len, id) in &self.bases {
            if addr >= base && addr < base + len {
                return (u32::from(id.0), addr - base);
            }
        }
        panic!("{addr:#x} isn't in a loaded archive")
    }

    fn dref_ours(r: Option<DataRef>) -> (u32, u32) {
        r.map_or((NONE, 0), |r| (u32::from(r.archive.0), r.offset))
    }

    fn their_gens(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut g = self.ctx.read_u32(sym("hsd_804D78FC"));
        while g != 0 {
            out.push(g);
            g = self.ctx.read_u32(g);
            assert!(out.len() < 100_000, "their generator list loops");
        }
        out
    }

    fn our_gens(&self) -> Vec<GeneratorId> {
        std::iter::successors(self.objs.generators.head, |&g| self.objs.generators[g].next)
            .collect()
    }

    fn their_link(&self, i: usize) -> Vec<u32> {
        let mut out = Vec::new();
        let mut p = self.ctx.read_u32(sym("hsd_804D0908") + 4 * i as u32);
        while p != 0 {
            out.push(p);
            p = self.ctx.read_u32(p);
            assert!(out.len() < 1_000_000, "their particle list loops");
        }
        out
    }

    fn our_link(&self, i: usize) -> Vec<ParticleId> {
        std::iter::successors(self.objs.particles.links[i], |&p| {
            self.objs.particles[p].next
        })
        .collect()
    }

    /// A joint: the test's (1 up), or 0.
    fn jobj_theirs(&self, j: u32) -> u32 {
        if j == 0 {
            return 0;
        }
        match self.joints.iter().position(|&(t, _)| t == j) {
            Some(i) => i as u32 + 1,
            None => 0x1000 + (j & 0xFFFF),
        }
    }

    fn jobj_ours(&self, j: Option<JObjId>) -> u32 {
        let Some(j) = j else { return 0 };
        match self.joints.iter().position(|&(_, o)| o == j) {
            Some(i) => i as u32 + 1,
            None => 0x2000 + j.0,
        }
    }

    fn userfunc_theirs(f: u32) -> u32 {
        match f {
            0 => 0,
            UF_A => 2,
            UF_B => 3,
            f if f == sym("lbl_803BF810") => 1,
            f => panic!("user functions at {f:#x}"),
        }
    }

    fn userfunc_ours(f: Option<UserFunc>) -> u32 {
        match f {
            None => 0,
            Some(UF_EF) => 1,
            Some(UF_A_T) => 2,
            Some(UF_B_T) => 3,
            Some(f) => panic!("user functions {f:?}"),
        }
    }

    fn floats(&self, at: u32, n: u32) -> Vec<u32> {
        (0..n).map(|i| self.ctx.read_u32(at + 4 * i)).collect()
    }

    /// Compares both sides entire; panics with `what` at the first difference.
    fn compare(&mut self, what: &str) {
        let ctx = self.ctx;
        assert_eq!(
            ctx.read_u32(sym("seed")),
            self.objs.rng.seed,
            "{what}: random seed"
        );
        assert_eq!(self.objs.stale_reads, 0, "{what}: stale reads");
        // Generators.
        let tg = self.their_gens();
        let og = self.our_gens();
        assert_eq!(tg.len(), og.len(), "{what}: generator counts");
        let tgi: HashMap<u32, u32> = tg.iter().enumerate().map(|(i, &a)| (a, i as u32)).collect();
        let ogi: HashMap<GeneratorId, u32> =
            og.iter().enumerate().map(|(i, &a)| (a, i as u32)).collect();
        let gen_t = |a: u32| {
            if a == 0 {
                NONE
            } else {
                tgi.get(&a).copied().unwrap_or(DANGLING)
            }
        };
        let gen_o =
            |g: Option<GeneratorId>| g.map_or(NONE, |g| ogi.get(&g).copied().unwrap_or(DANGLING));
        // App-SRTs by first sight, compared once each.
        let mut srt_t: HashMap<u32, u32> = HashMap::new();
        let mut srt_o: HashMap<AppSrtId, u32> = HashMap::new();
        let mut srts: Vec<(u32, AppSrtId)> = Vec::new();
        let mut srt_label = |t: u32, o: Option<AppSrtId>, srts: &mut Vec<(u32, AppSrtId)>| {
            let (Some(o), true) = (o, t != 0) else {
                assert_eq!((t != 0), o.is_some(), "{what}: app-SRT presence");
                return NONE;
            };
            let next = srts.len() as u32;
            let lt = *srt_t.entry(t).or_insert(next);
            let lo = *srt_o.entry(o).or_insert(next);
            assert_eq!(lt, lo, "{what}: app-SRT sharing");
            if lt == next {
                srts.push((t, o));
            }
            lt
        };
        for (i, (&t, &o)) in tg.iter().zip(&og).enumerate() {
            let g = &self.objs.generators[o];
            let r16 = |off| u32::from(ctx.read_u16(t + off));
            let r8 = |off| u32::from(ctx.read_u8(t + off));
            let (tl, tlo) = self.dref_theirs(ctx.read_u32(t + 0x20));
            let mut theirs = vec![
                ctx.read_u32(t + 4),
                ctx.read_u32(t + 8),
                ctx.read_u32(t + 0xC),
                self.jobj_theirs(ctx.read_u32(t + 0x10)),
                r16(0x14),
                r16(0x16),
                r8(0x18),
                r8(0x19),
                r8(0x1A),
                r8(0x1B),
                r16(0x1C),
                r16(0x1E),
                tl,
                tlo,
            ];
            theirs.extend(self.floats(t + 0x24, 11));
            theirs.extend([
                ctx.read_u32(t + 0x50),
                srt_label(ctx.read_u32(t + 0x54), g.appsrt, &mut srts),
                Self::userfunc_theirs(ctx.read_u32(t + 0x58)),
                ctx.read_u32(t + 0x5C),
            ]);
            theirs.extend(self.floats(t + 0x60, 12));
            theirs.push(r16(0x90));
            let (ol, olo) = Self::dref_ours(g.cmd_list);
            let mut ours = vec![
                g.kind,
                g.random.to_bits(),
                g.count.to_bits(),
                self.jobj_ours(g.jobj),
                u32::from(g.gen_life),
                u32::from(g.ty),
                u32::from(g.bank),
                u32::from(g.link_no),
                u32::from(g.tex_group),
                0,
                u32::from(g.idnum),
                u32::from(g.life),
                ol,
                olo,
            ];
            for v in [g.pos.x, g.pos.y, g.pos.z, g.vel.x, g.vel.y, g.vel.z] {
                ours.push(v.to_bits());
            }
            for v in [g.grav, g.fric, g.size, g.radius, g.angle] {
                ours.push(v.to_bits());
            }
            ours.extend([g.num_child, theirs[26], Self::userfunc_ours(g.userfunc), 0]);
            ours.extend(g.aux.iter().map(|v| v.to_bits()));
            ours.push(u32::from(g.rect_flag));
            assert_eq!(theirs, ours, "{what}: generator {i}");
        }
        // Particles, link by link.
        let mut where_t: HashMap<u32, u32> = HashMap::new();
        let mut where_o: HashMap<ParticleId, u32> = HashMap::new();
        let mut total = 0;
        for link in 0..particle::LINKS {
            let tl = self.their_link(link);
            let ol = self.our_link(link);
            assert_eq!(tl.len(), ol.len(), "{what}: link {link} lengths");
            total += tl.len();
            for (i, (&t, &o)) in tl.iter().zip(&ol).enumerate() {
                where_t.insert(t, (link as u32) << 20 | i as u32);
                where_o.insert(o, (link as u32) << 20 | i as u32);
                let p = &self.objs.particles[o];
                let r16 = |off| u32::from(ctx.read_u16(t + off));
                let r8 = |off| u32::from(ctx.read_u8(t + off));
                let rgba = |off| {
                    u32::from_be_bytes([
                        ctx.read_u8(t + off),
                        ctx.read_u8(t + off + 1),
                        ctx.read_u8(t + off + 2),
                        ctx.read_u8(t + off + 3),
                    ])
                };
                let (tl_, tlo) = self.dref_theirs(ctx.read_u32(t + 0x20));
                let cb = match ctx.read_u32(t + 0x94) {
                    0 => 0,
                    NATIVE_CB1 => 1,
                    NATIVE_CB2 => 2,
                    c => panic!("callback {c:#x}"),
                };
                let ud = match ctx.read_u32(t + 0x90) {
                    0 => 0,
                    USERDATA => 1,
                    u => panic!("user data {u:#x}"),
                };
                let mut theirs = vec![
                    ctx.read_u32(t + 4),
                    r8(8),
                    r8(9),
                    r8(0xA),
                    r8(0xB),
                    r16(0xC),
                    r16(0xE),
                    r16(0x10),
                    rgba(0x12),
                    rgba(0x16),
                    r16(0x1A),
                    r8(0x1C),
                    r8(0x1D),
                    r16(0x1E),
                    tl_,
                    tlo,
                    r16(0x24),
                    r16(0x26),
                    r16(0x28),
                    r16(0x2A),
                ];
                theirs.extend(self.floats(t + 0x2C, 10));
                theirs.extend([
                    r16(0x54),
                    r8(0x56),
                    r8(0x57),
                    r8(0x58),
                    r8(0x59),
                    r16(0x5A),
                    r16(0x5C),
                    r16(0x5E),
                ]);
                theirs.extend(self.floats(t + 0x60, 2));
                theirs.extend([r16(0x68), r16(0x6A), rgba(0x6C), rgba(0x70)]);
                theirs.extend([r16(0x74), r16(0x76), r16(0x78)]);
                theirs.extend((0x7A..0x84).map(r8));
                theirs.push(ctx.read_u32(t + 0x84));
                theirs.push(gen_t(ctx.read_u32(t + 0x88)));
                theirs.push(srt_label(ctx.read_u32(t + 0x8C), p.appsrt, &mut srts));
                theirs.extend([ud, cb]);
                let c = |c: particle::Rgba| u32::from_be_bytes([c.r, c.g, c.b, c.a]);
                let (ol_, olo) = Self::dref_ours(p.cmd_list);
                let mut ours = vec![
                    p.kind,
                    u32::from(p.bank),
                    u32::from(p.tex_group),
                    u32::from(p.pose_num),
                    u32::from(p.pal_num),
                    u32::from(p.size_count),
                    u32::from(p.prim_col_count),
                    u32::from(p.env_col_count),
                    c(p.prim_col),
                    c(p.env_col),
                    u32::from(p.cmd_wait),
                    u32::from(p.loop_count),
                    u32::from(p.link_no),
                    u32::from(p.idnum),
                    ol_,
                    olo,
                    u32::from(p.cmd_ptr),
                    u32::from(p.cmd_mark_ptr),
                    u32::from(p.cmd_loop_ptr),
                    u32::from(p.life),
                ];
                for v in [
                    p.vel.x, p.vel.y, p.vel.z, p.grav, p.fric, p.pos.x, p.pos.y, p.pos.z,
                ] {
                    ours.push(v.to_bits());
                }
                ours.extend([p.size.to_bits(), p.rotate.to_bits()]);
                ours.extend([
                    u32::from(p.a_cmp_count),
                    u32::from(p.a_cmp_mode),
                    u32::from(p.a_cmp_param1),
                    u32::from(p.a_cmp_param2),
                    u32::from(p.p_jobj_ofs),
                    u32::from(p.mat_col_count),
                    u32::from(p.amb_col_count),
                    u32::from(p.rotate_count),
                    p.size_target.to_bits(),
                    p.rotate_target.to_bits(),
                    u32::from(p.prim_col_remain),
                    u32::from(p.env_col_remain),
                    c(p.prim_col_target),
                    c(p.env_col_target),
                    u32::from(p.mat_col_remain),
                    u32::from(p.amb_col_remain),
                    u32::from(p.a_cmp_remain),
                ]);
                ours.extend(
                    [
                        p.a_cmp_param1_target,
                        p.a_cmp_param2_target,
                        p.mat_rgb,
                        p.mat_a,
                        p.amb_rgb,
                        p.amb_a,
                        p.mat_rgb_target,
                        p.mat_a_target,
                        p.amb_rgb_target,
                        p.amb_a_target,
                    ]
                    .map(u32::from),
                );
                ours.push(p.trail.to_bits());
                ours.push(gen_o(p.generator));
                ours.push(theirs[theirs.len() - 3]);
                ours.push(u32::from(p.userdata.is_some()));
                ours.push(p.callback.map_or(0, |c| c.0));
                assert_eq!(theirs, ours, "{what}: link {link} particle {i}");
            }
        }
        // App-SRTs.
        for (i, &(t, o)) in srts.iter().enumerate() {
            let s = &self.objs.app_srts[o];
            let mut theirs = vec![ctx.read_u32(t), gen_t(ctx.read_u32(t + 4))];
            theirs.extend(self.floats(t + 8, 10));
            theirs.extend([
                u32::from(ctx.read_u8(t + 0x30)),
                u32::from(ctx.read_u8(t + 0x31)),
                u32::from(ctx.read_u16(t + 0x32)),
            ]);
            theirs.extend(self.floats(t + 0x34, 26));
            theirs.extend([
                ctx.read_u32(t + 0x9C),
                u32::from(ctx.read_u16(t + 0xA0)),
                u32::from(ctx.read_u8(t + 0xA2)),
            ]);
            let mut ours = vec![0, gen_o(s.gp)];
            ours.extend(
                [
                    s.translate.x,
                    s.translate.y,
                    s.translate.z,
                    s.rot.x,
                    s.rot.y,
                    s.rot.z,
                    s.rot.w,
                    s.scale.x,
                    s.scale.y,
                    s.scale.z,
                ]
                .map(f32::to_bits),
            );
            ours.extend([
                u32::from(s.status),
                u32::from(s.frame_num),
                u32::from(s.used_count),
            ]);
            ours.extend(s.mmtx.m.as_flattened().iter().map(|v| v.to_bits()));
            ours.extend(s.view.m.as_flattened().iter().map(|v| v.to_bits()));
            ours.extend(s.view_scale.map(f32::to_bits));
            ours.extend([0, u32::from(s.idnum), u32::from(s.xa2)]);
            assert_eq!(theirs, ours, "{what}: app-SRT {i}");
        }
        // Counts.
        let used = |data: &str| ctx.read_u32(sym(data) + 8);
        let u16s = |name: &str| u32::from(ctx.read_u16(sym(name)));
        let ps = &self.objs.particles;
        let gs = &self.objs.generators;
        let ss = &self.objs.app_srts;
        assert_eq!(
            [
                u16s("hsd_804D78E2"),
                u16s("numPeakParticles"),
                used("hsd_804D0F60"),
                total as u32,
                u16s("hsd_804D78E0"),
                u16s("hsd_804D78DA"),
                used("hsd_804D0F90"),
                u16s("lbl_804D6368"),
                u16s("hsd_804D78DE"),
                u16s("hsd_804D78D8"),
                used("HSD_PSAppSrt_804D10B0"),
                ctx.read_u32(sym("hsdJObj") + 0x20) - self.jobj_baseline.0,
            ],
            [
                u32::from(ps.live),
                u32::from(ps.peak),
                ps.pool.used(),
                u32::from(ps.live),
                u32::from(gs.live),
                u32::from(gs.peak),
                gs.pool.used(),
                u32::from(gs.idnum),
                u32::from(ss.live),
                u32::from(ss.peak),
                ss.pool.used(),
                self.objs.jobjs.pool.used() - self.jobj_baseline.1,
            ],
            "{what}: live and peak particles, generators, family ids, app-SRTs and joints"
        );
        assert_eq!(
            gen_t(ctx.read_u32(sym("hsd_804D78F8"))),
            gen_o(gs.cursor),
            "{what}: generator cursor"
        );
        let mut pending_t = Vec::new();
        let mut n = ctx.read_u32(sym("hsd_804D78F4"));
        while n != 0 {
            pending_t.push(gen_t(ctx.read_u32(n + 4)));
            n = ctx.read_u32(n);
        }
        let mut pending_o = Vec::new();
        let mut n = gs.pending;
        while let Some(id) = n {
            pending_o.push(gen_o(Some(gs.nodes[id.0].data)));
            n = gs.nodes[id.0].next;
        }
        assert_eq!(pending_t, pending_o, "{what}: generators to orient");
        // Point joints and the test's joints.
        let mut joints: Vec<(u32, JObjId)> = self.joints.clone();
        for slot in 0..particle::POINT_JOBJS {
            let t = ctx.read_u32(sym("hsd_804D08E8") + 4 * slot as u32);
            let o = self.objs.particles.point_jobjs[slot];
            assert_eq!(t != 0, o.is_some(), "{what}: point joint {slot}");
            if let Some(o) = o {
                let (tl, ol) = (self.jobj_theirs(t), self.jobj_ours(Some(o)));
                if tl < 0x1000 || ol < 0x1000 {
                    assert_eq!(tl, ol, "{what}: point joint {slot}");
                } else {
                    joints.push((t, o));
                }
            }
        }
        for (i, &(t, o)) in joints.iter().enumerate() {
            let j = &self.objs.jobjs[o];
            let mut theirs = vec![
                u32::from(ctx.read_u16(t + 4)),
                u32::from(ctx.read_u16(t + 6)),
                ctx.read_u32(t + 0x14),
            ];
            theirs.extend(self.floats(t + 0x1C, 10));
            theirs.extend(self.floats(t + 0x44, 12));
            let mut ours = vec![
                u32::from(j.ref_count),
                u32::from(j.ref_count_individual),
                j.flags,
            ];
            let q = &j.rotate;
            ours.extend(
                [
                    q.x,
                    q.y,
                    q.z,
                    q.w,
                    j.scale.x,
                    j.scale.y,
                    j.scale.z,
                    j.translate.x,
                    j.translate.y,
                    j.translate.z,
                ]
                .map(f32::to_bits),
            );
            ours.extend(j.mtx.m.as_flattened().iter().map(|v| v.to_bits()));
            assert_eq!(theirs, ours, "{what}: joint {i}");
        }
        // The display pass's sorting state.
        let at = |w: &HashMap<u32, u32>, a: u32| {
            if a == 0 {
                NONE
            } else {
                w.get(&a).copied().unwrap_or(DANGLING)
            }
        };
        let d = &self.objs.psdisp;
        let mut theirs = vec![u32::from(ctx.read_u8(sym("psFrameNum")))];
        theirs.extend((0..16).map(|i| u32::from(ctx.read_u8(sym("HSD_PSDisp_8040C360") + i))));
        theirs.extend((0..16).map(|i| at(&where_t, ctx.read_u32(sym("particle_list") + 4 * i))));
        let mut ours = vec![u32::from(d.frame)];
        ours.extend(d.sorted.map(u32::from));
        ours.extend(
            d.non_edge
                .map(|p| p.map_or(NONE, |p| where_o.get(&p).copied().unwrap_or(DANGLING))),
        );
        assert_eq!(theirs, ours, "{what}: display sorting");
        assert_eq!(
            self.floats(USERDATA, 256),
            self.hooks
                .userdata
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "{what}: user data"
        );
        let logged = LOG.with(|l| std::mem::take(&mut *l.borrow_mut()));
        assert_eq!(
            logged,
            std::mem::take(&mut self.hooks.log),
            "{what}: user function calls"
        );
    }
}

#[derive(Default)]
struct Counts {
    sessions: u64,
    halted: u64,
    frames: u64,
    steps: u64,
    most_particles: usize,
    most_generators: usize,
    /// Spawns that made a generator, by bank and index.
    spawned: HashMap<(usize, i32), u64>,
    shapes: [u64; 16],
    ops: HashMap<&'static str, u64>,
}

/// A session's banks and what to spawn first.
struct Plan {
    name: String,
    files: Vec<Vec<u8>>,
    specs: Vec<BankSpec>,
    /// Entries to spawn before random ones: (bank, index).
    first: Vec<(usize, i32)>,
    frames: u32,
    synthetic: bool,
}

fn put_f32s(ctx: &Ctx, at: u32, v: &[f32]) {
    leaf_check::write_f32s(ctx, at, v);
}

/// Sets up both sides for a plan.
fn setup<'c>(ctx: &'c Ctx, plan: &Plan, rand: &mut Rand) -> Sides<'c> {
    ctx.register(NATIVE_CREATE, native_create);
    ctx.register(NATIVE_DELETE, native_delete);
    ctx.register(NATIVE_SET_UD, native_set_ud);
    ctx.register(NATIVE_CB1, native_cb1);
    ctx.register(NATIVE_CB2, native_cb2);
    for (i, w) in [NATIVE_CREATE, NATIVE_DELETE, 0].into_iter().enumerate() {
        ctx.write_u32(UF_A + 4 * i as u32, w);
    }
    for (i, w) in [NATIVE_CREATE, NATIVE_DELETE, NATIVE_SET_UD]
        .into_iter()
        .enumerate()
    {
        ctx.write_u32(UF_B + 4 * i as u32, w);
    }
    ctx.write_u32(CALLBACKS, NATIVE_CB1);
    ctx.write_u32(CALLBACKS + 4, NATIVE_CB2);
    for i in 0..256 {
        ctx.write_u32(USERDATA + 4 * i, 0);
    }
    LOG.with(|l| l.borrow_mut().clear());
    let mut archives = Archives::default();
    let mut bases = Vec::new();
    let mut file_at = FILES;
    for (i, f) in plan.files.iter().enumerate() {
        let a = ARCHIVE_STRUCTS + 0x100 * i as u32;
        assert!(
            file_at as usize + f.len() <= FILES_END as usize,
            "{}: files too big",
            plan.name
        );
        ctx.write_bytes(file_at, f);
        let parsed =
            fns::HSD_ArchiveParse(ctx, handle(ctx, a), handle(ctx, file_at), f.len() as u32);
        assert_eq!(parsed, 0, "HSD_ArchiveParse");
        let base = ctx.read_u32(a + 0x20);
        let archive = Archive::parse(f).unwrap();
        let len = archive.data().len() as u32;
        let id = archives.add(archive);
        bases.push((base, len, id));
        file_at = (file_at + f.len() as u32 + 31) & !31;
    }
    let mut s = Sides {
        ctx,
        archives,
        bases,
        objs: Objects::default(),
        hooks: TestHooks {
            userdata: vec![0.0; 256],
            log: Vec::new(),
        },
        joints: Vec::new(),
        banks: Vec::new(),
        jobj_baseline: (0, 0),
    };
    fns::hsd_8039D354(ctx, 0);
    fns::hsd_80398A08(ctx, 0);
    fns::psInitAppSRT(ctx, 0, 0xA4);
    ctx.write_u32(sym("hsd_804D7900"), sym("efLib_Cb_PtclAppSRTHook"));
    generator::init(&mut s.objs);
    particle::init(&mut s.objs);
    appsrt::init(&mut s.objs, 0, 0xA4);
    s.objs.generators.on_create = Some(EF_HOOK);
    for b in &plan.specs {
        let (base, _, id) = s.bases[b.archive];
        let at = |o: Option<u32>| o.map_or(0, |o| base + o);
        let r = |o: u32| DataRef {
            archive: id,
            offset: o,
        };
        let (cmd, tex, remap, form) = (base + b.cmd, base + b.tex, at(b.remap), at(b.form));
        if b.game_way {
            fns::psInitDataBankLocate(ctx, handle(ctx, cmd), handle(ctx, tex), handle(ctx, form));
            fns::psInitDataBankLoad(
                ctx,
                b.bank as i32,
                handle(ctx, cmd),
                handle(ctx, tex),
                handle(ctx, remap),
                handle(ctx, form),
            );
            particle::load_data_bank(
                &mut s.objs,
                &s.archives,
                b.bank,
                r(b.cmd),
                r(b.tex),
                b.remap.map(r),
                b.form.map(r),
            );
        } else {
            fns::psInitDataBank(
                ctx,
                b.bank as i32,
                handle(ctx, cmd),
                handle(ctx, tex),
                handle(ctx, remap),
                handle(ctx, form),
            );
            particle::init_data_bank(
                &mut s.objs,
                &s.archives,
                b.bank,
                r(b.cmd),
                r(b.tex),
                b.remap.map(r),
                b.form.map(r),
            );
        }
        let ob = s.objs.particles.banks[b.bank].unwrap();
        let bank = b.bank as u32;
        assert_eq!(
            ctx.read_u32(sym("psCmdListArray") + 4 * bank) as i32,
            ob.count,
            "{}: bank {bank}'s size",
            plan.name
        );
        let table = ctx.read_u32(sym("ptclref_804D0E5C") + 4 * bank);
        assert_eq!(
            s.dref_theirs(table.wrapping_add(4 * ob.first as u32)),
            Sides::dref_ours(Some(ob.table)),
            "{}: bank {bank}'s table",
            plan.name
        );
        assert_eq!(
            s.dref_theirs(ctx.read_u32(sym("psNumCmdList") + 4 * bank)),
            Sides::dref_ours(ob.form),
            "{}: bank {bank}'s shapes",
            plan.name
        );
        s.banks.push((b.bank, ob.first, ob.count));
    }
    if plan.synthetic {
        ctx.write_u32(sym("psCallback"), CALLBACKS);
        s.objs.particles.callbacks = vec![CB1, CB2];
    }
    let seed = rand.bits() as u32;
    ctx.write_u32(sym("seed"), seed);
    s.objs.rng.seed = seed;
    // The test's joints, held by many references so the point-joint table's unreferencing
    // never frees them.
    for _ in 0..6 {
        let t = fns::HSD_JObjAlloc(ctx).addr();
        let o = jobj::alloc(&mut s.objs.jobjs);
        ctx.write_u16(t + 4, 100);
        s.objs.jobjs[o].ref_count = 100;
        let rot = [
            rand.range(-3.0, 3.0),
            rand.range(-3.0, 3.0),
            rand.range(-3.0, 3.0),
        ];
        let scale = [
            rand.range(0.5, 2.0),
            rand.range(0.5, 2.0),
            rand.range(0.5, 2.0),
        ];
        let tr = [
            rand.range(-40.0, 40.0),
            rand.range(-40.0, 40.0),
            rand.range(-40.0, 40.0),
        ];
        put_f32s(ctx, t + 0x1C, &rot);
        put_f32s(ctx, t + 0x2C, &scale);
        put_f32s(ctx, t + 0x38, &tr);
        let j = &mut s.objs.jobjs[o];
        (j.rotate.x, j.rotate.y, j.rotate.z) = (rot[0], rot[1], rot[2]);
        (j.scale.x, j.scale.y, j.scale.z) = (scale[0], scale[1], scale[2]);
        (j.translate.x, j.translate.y, j.translate.z) = (tr[0], tr[1], tr[2]);
        s.joints.push((t, o));
    }
    s.jobj_baseline = (
        ctx.read_u32(sym("hsdJObj") + 0x20),
        s.objs.jobjs.pool.used(),
    );
    s
}

/// Runs a plan; returns false if the original halted.
fn run(plan: &Plan, rand: &mut Rand, counts: &mut Counts, mode: FpMode) -> bool {
    let ctx = hsd_reference().unwrap();
    let ctx = &ctx;
    set_fp_mode(mode);
    let mut s = setup(ctx, plan, rand);
    let name = format!("{} in {mode:?}", plan.name);
    s.compare(&format!("{name}: setup"));
    counts.sessions += 1;
    let mut first = plan.first.clone();
    first.reverse();
    for frame in 0..plan.frames {
        let what = format!("{name}: frame {frame}");
        let n = if first.is_empty() { rand.below(3) } else { 3 };
        for _ in 0..n {
            if !spawn_op(&mut s, rand, &mut first, counts, &what) {
                return false;
            }
        }
        for _ in 0..rand.below(3) {
            let Some(op) = random_op(&mut s, rand, &what) else {
                return false;
            };
            s.compare(&format!("{what}: {op}"));
            *counts.ops.entry(op).or_default() += 1;
            counts.steps += 1;
        }
        // efLib's two processes: every link but 1 and 2, then every link but 0.
        for mask in [0x6_0000u32, 0x1_0000] {
            let (objs, data, hooks) = (&mut s.objs, &s.archives, &mut s.hooks);
            if !both(
                &format!("{what}: particles {mask:#x}"),
                || fns::hsd_8039CEAC(ctx, mask),
                || particle::execute_all(objs, data, hooks, mask),
            ) {
                return false;
            }
            s.compare(&format!("{what}: particles {mask:#x}"));
            let (objs, data, hooks) = (&mut s.objs, &s.archives, &mut s.hooks);
            if !both(
                &format!("{what}: generators {mask:#x}"),
                || fns::hsd_8039EE24(ctx, mask),
                || generator::run_all(objs, data, hooks, mask),
            ) {
                return false;
            }
            s.compare(&format!("{what}: generators {mask:#x}"));
        }
        // The display pass: the frame counter, then each drawn link's sort.
        fns::psDispParticles(ctx, 0, 0);
        psdisp::disp_particles(&mut s.objs, 0, 0);
        let frame_num = ctx.read_u8(sym("psFrameNum"));
        for link in [0usize, 2, 1] {
            let t = fns::particleSort(
                ctx,
                link as i32,
                frame_num,
                handle(ctx, OUT),
                handle(ctx, OUT + 4),
            )
            .addr();
            // The edge pass sorts, the other reuses this frame's sort, as efLib draws them.
            let o = psdisp::disp_particles(&mut s.objs, 1 << link, 1)[0].1;
            let o_rest = psdisp::disp_particles(&mut s.objs, 1 << link, 2)[0].1;
            let lt = s.their_link(link);
            let lo = s.our_link(link);
            let pos_t = |a: u32| {
                if a == 0 {
                    NONE
                } else {
                    lt.iter()
                        .position(|&x| x == a)
                        .map_or(DANGLING, |p| p as u32)
                }
            };
            let pos_o = |a: Option<ParticleId>| {
                a.map_or(NONE, |a| {
                    lo.iter()
                        .position(|&x| x == a)
                        .map_or(DANGLING, |p| p as u32)
                })
            };
            assert_eq!(
                (
                    pos_t(t),
                    pos_t(ctx.read_u32(OUT)),
                    pos_t(ctx.read_u32(OUT + 4))
                ),
                (pos_o(o), pos_o(o), pos_o(o_rest)),
                "{what}: link {link}'s sort"
            );
        }
        s.compare(&format!("{what}: display"));
        let tp: usize = (0..particle::LINKS).map(|l| s.their_link(l).len()).sum();
        counts.most_particles = counts.most_particles.max(tp);
        counts.most_generators = counts.most_generators.max(s.their_gens().len());
        counts.frames += 1;
    }
    true
}

/// Spawns a generator on both sides: the next planned one, else a random bank entry.
fn spawn_op(
    s: &mut Sides,
    rand: &mut Rand,
    first: &mut Vec<(usize, i32)>,
    counts: &mut Counts,
    what: &str,
) -> bool {
    let ctx = s.ctx;
    let planned = !first.is_empty();
    let (bank, idx) = first.pop().unwrap_or_else(|| {
        let (bank, lo, hi) = s.banks[rand.below(s.banks.len() as u64) as usize];
        // Now and then one past the end, which makes nothing.
        let idx = lo + rand.below((hi - lo + 1) as u64) as i32;
        (bank, idx)
    });
    // Planned spawns use the game's links; random ones now and then a link past them.
    let link = if planned {
        rand.below(3) as i32
    } else {
        [0, 0, 0, 0, 1, 2, 3, 7, 8][rand.below(9) as usize]
    };
    let way = rand.below(4);
    let (jt, jo) = pick_joint(s, rand);
    let (mut t, mut o) = (0u32, None);
    let (objs, data, hooks) = (&mut s.objs, &s.archives, &mut s.hooks);
    let carried_on = both(
        &format!("{what}: spawn {bank}:{idx}"),
        || {
            t = match way {
                0 | 1 => fns::hsd_8039F05C(ctx, link, bank as i32, idx).addr(),
                2 => fns::hsd_8039EFAC(ctx, link, bank as i32, idx, handle(ctx, jt)).addr(),
                _ => fns::hsd_8039F6CC(ctx, link, bank as i32, idx, handle(ctx, jt)).addr(),
            }
        },
        || {
            o = match way {
                0 | 1 => generator::spawn(objs, data, hooks, link, bank as i32, idx),
                2 => generator::spawn_on_jobj(objs, data, hooks, link, bank as i32, idx, jo),
                _ => {
                    generator::spawn_oriented_on_jobj(objs, data, hooks, link, bank as i32, idx, jo)
                }
            }
        },
    );
    if !carried_on {
        return false;
    }
    assert_eq!(
        t != 0,
        o.is_some(),
        "{what}: spawn {bank}:{idx} made a generator"
    );
    if let Some(o) = o {
        *counts.spawned.entry((bank, idx)).or_default() += 1;
        counts.shapes[usize::from(s.objs.generators[o].ty & 0xF)] += 1;
        if way <= 1 {
            // Placed somewhere, as efLib places what it spawns.
            let p = [
                rand.range(-50.0, 50.0),
                rand.range(-30.0, 60.0),
                rand.range(-10.0, 10.0),
            ];
            put_f32s(ctx, t + 0x24, &p);
            let g = &mut s.objs.generators[o];
            (g.pos.x, g.pos.y, g.pos.z) = (p[0], p[1], p[2]);
        }
    }
    s.compare(&format!("{what}: spawn {bank}:{idx} by {way}"));
    true
}

/// A random particle on both sides.
fn pick_particle(s: &Sides, rand: &mut Rand) -> Option<(u32, ParticleId)> {
    let all: Vec<(u32, ParticleId)> = (0..particle::LINKS)
        .flat_map(|l| s.their_link(l).into_iter().zip(s.our_link(l)))
        .collect();
    (!all.is_empty()).then(|| all[rand.below(all.len() as u64) as usize])
}

fn pick_generator(s: &Sides, rand: &mut Rand) -> Option<(u32, GeneratorId)> {
    let (t, o) = (s.their_gens(), s.our_gens());
    (!t.is_empty()).then(|| {
        let i = rand.below(t.len() as u64) as usize;
        (t[i], o[i])
    })
}

fn pick_joint(s: &Sides, rand: &mut Rand) -> (u32, Option<JObjId>) {
    let j = rand.below(s.joints.len() as u64 + 1) as usize;
    if j < s.joints.len() {
        (s.joints[j].0, Some(s.joints[j].1))
    } else {
        (0, None)
    }
}

/// An app-SRT someone holds, on both sides.
fn pick_app_srt(s: &Sides, rand: &mut Rand) -> Option<(u32, AppSrtId)> {
    let ctx = s.ctx;
    let mut all = Vec::new();
    for (t, o) in s.their_gens().into_iter().zip(s.our_gens()) {
        if let Some(srt) = s.objs.generators[o].appsrt {
            all.push((ctx.read_u32(t + 0x54), srt));
        }
    }
    for l in 0..particle::LINKS {
        for (t, o) in s.their_link(l).into_iter().zip(s.our_link(l)) {
            if let Some(srt) = s.objs.particles[o].appsrt {
                all.push((ctx.read_u32(t + 0x8C), srt));
            }
        }
    }
    (!all.is_empty()).then(|| all[rand.below(all.len() as u64) as usize])
}

/// One random edit on both sides; its name, or `None` where the original halted.
fn random_op(s: &mut Sides, rand: &mut Rand, what: &str) -> Option<&'static str> {
    let ctx = s.ctx;
    let op = rand.below(27);
    let name = match op {
        0 => "kill",
        1 => "kill on joint",
        2 => "kill on tree",
        3 => "detach joint",
        4 => "kill particles",
        5 => "user functions",
        6 => "point joint",
        7 => "scatter",
        8 => "attract",
        9 => "particle app-SRT",
        10 => "generator app-SRT",
        11 => "follow unless paused",
        12 => "follow",
        13 => "orient",
        14 => "pause",
        15 | 16 => "move joint",
        17 => "emit",
        18 => "generate",
        19 => "run first",
        20 => "alloc generator",
        21 => "family id",
        22 => "remove generator",
        23 => "loose app-SRT",
        24 => "generate for",
        25 => "release point joint",
        _ => "nothing",
    };
    let g = pick_generator(s, rand);
    let p = pick_particle(s, rand);
    let (jt, jo) = pick_joint(s, rand);
    let srt = pick_app_srt(s, rand);
    let r1 = rand.bits();
    let f1 = rand.range(-3.0, 3.0);
    let f2 = rand.range(-2.0, 12.0);
    let banks = s.banks.clone();
    let bases = s.bases.clone();
    let (objs, data, hooks) = (&mut s.objs, &s.archives, &mut s.hooks);
    let what = format!("{what}: {name}");
    let (mut rt, mut ro) = (0i64, 0i64);
    let carried_on = match op {
        0 | 4 | 5 | 11..=14 | 17 => {
            let Some((gt, go)) = g else {
                return Some("no generator");
            };
            match op {
                0 => both(
                    &what,
                    || fns::hsd_8039D4DC(ctx, handle(ctx, gt)),
                    || generator::kill(objs, data, hooks, go),
                ),
                4 => both(
                    &what,
                    || fns::hsd_8039D0A0(ctx, handle(ctx, gt)),
                    || particle::kill_generator_particles(objs, data, hooks, go),
                ),
                5 => {
                    let (ft, fo) =
                        [(0, None), (UF_A, Some(UF_A_T)), (UF_B, Some(UF_B_T))][(r1 % 3) as usize];
                    both(
                        &what,
                        || fns::hsd_8039D1E4(ctx, handle(ctx, gt), handle(ctx, ft)),
                        || generator::set_user_func(objs, go, fo),
                    )
                }
                11 => {
                    let none = r1.is_multiple_of(4);
                    both(
                        &what,
                        || fns::hsd_803983A4(ctx, handle(ctx, if none { 0 } else { gt })),
                        || generator::follow_joint_unless_paused(objs, data, (!none).then_some(go)),
                    )
                }
                12 => both(
                    &what,
                    || fns::hsd_8039D214(ctx, handle(ctx, gt)),
                    || generator::follow_joint(objs, data, go),
                ),
                13 => both(
                    &what,
                    || fns::hsd_8039D71C(ctx, handle(ctx, gt)),
                    || generator::orient_to_jobj(objs, data, go),
                ),
                14 => {
                    // Nothing in the game freezes a generator, but both sides must agree.
                    if !r1.is_multiple_of(8) {
                        return Some("skipped pause");
                    }
                    let k = ctx.read_u32(gt + 4) ^ particle::PAUSED;
                    ctx.write_u32(gt + 4, k);
                    objs.generators[go].kind = k;
                    true
                }
                _ => both(
                    &what,
                    || rt = i64::from(gekko_fp::stfs(fns::hsd_8039DAD4(ctx, handle(ctx, gt)))),
                    || ro = i64::from(generator::emit(objs, data, hooks, go).to_bits()),
                ),
            }
        }
        1 => both(
            &what,
            || fns::hsd_8039D580(ctx, handle(ctx, jt)),
            || generator::kill_on_jobj(objs, data, hooks, jo),
        ),
        2 => both(
            &what,
            || fns::hsd_8039D5DC(ctx, handle(ctx, jt)),
            || generator::kill_on_tree(objs, data, hooks, jo),
        ),
        3 => {
            let Some(jo) = jo else {
                return Some("no joint");
            };
            both(
                &what,
                || fns::hsd_8039D688(ctx, handle(ctx, jt), handle(ctx, 0), 0),
                || generator::detach_jobj(objs, data, hooks, jo),
            )
        }
        6 => {
            let index = (r1 % 9) as i32;
            both(
                &what,
                || fns::hsd_8039CF4C(ctx, index, handle(ctx, jt)),
                || particle::set_point_jobj(objs, index, jo),
            )
        }
        7 | 8 => {
            let Some((pt, po)) = p else {
                return Some("no particle");
            };
            if op == 7 {
                both(
                    &what,
                    || fns::hsd_80398F8C(ctx, handle(ctx, pt), arg(f1)),
                    || particle::scatter(objs, po, arg(f1)),
                )
            } else {
                let (force, range) = (arg(f1 * 10.0), arg(f2));
                both(
                    &what,
                    || {
                        rt = i64::from(fns::hsd_803991D8(
                            ctx,
                            handle(ctx, pt),
                            handle(ctx, jt),
                            force,
                            range,
                        ))
                    },
                    || ro = i64::from(particle::attract(objs, data, po, jo, force, range)),
                )
            }
        }
        9 => {
            let Some((pt, po)) = p else {
                return Some("no particle");
            };
            match r1 % 3 {
                0 => both(
                    &what,
                    || rt = i64::from(fns::psRemoveParticleAppSRT(ctx, handle(ctx, pt))),
                    || ro = i64::from(appsrt::remove_from_particle(objs, po)),
                ),
                1 if objs.particles[po].appsrt.is_none() => {
                    let status = (r1 / 3 % 3) as i32;
                    both(
                        &what,
                        || {
                            fns::psAddParticleAppSRT_begin(ctx, handle(ctx, pt), status);
                        },
                        || {
                            appsrt::add_to_particle(objs, po, status);
                        },
                    )
                }
                _ => {
                    let (st, so) = srt.map_or((0, None), |(t, o)| (t, Some(o)));
                    both(
                        &what,
                        || {
                            rt = i64::from(fns::psAttachParticleAppSRT(
                                ctx,
                                handle(ctx, pt),
                                handle(ctx, st),
                            ))
                        },
                        || ro = i64::from(appsrt::attach_particle(objs, po, so)),
                    )
                }
            }
        }
        10 => {
            let Some((gt, go)) = g else {
                return Some("no generator");
            };
            match r1 % 3 {
                0 => both(
                    &what,
                    || rt = i64::from(fns::psRemoveGeneratorSRT(ctx, handle(ctx, gt))),
                    || ro = i64::from(appsrt::remove_from_generator(objs, go)),
                ),
                1 if objs.generators[go].appsrt.is_none() => {
                    let status = (r1 / 3 % 3) as i32;
                    both(
                        &what,
                        || {
                            fns::psAddGeneratorAppSRT_begin(ctx, handle(ctx, gt), status);
                        },
                        || {
                            appsrt::add_to_generator(objs, go, status);
                        },
                    )
                }
                _ => {
                    let (st, so) = srt.map_or((0, None), |(t, o)| (t, Some(o)));
                    both(
                        &what,
                        || {
                            rt = i64::from(fns::psAttachGeneratorAppSRT(
                                ctx,
                                handle(ctx, gt),
                                handle(ctx, st),
                            ))
                        },
                        || ro = i64::from(appsrt::attach_generator(objs, go, so)),
                    )
                }
            }
        }
        15 | 16 => {
            let Some(jo) = jo else {
                return Some("no joint");
            };
            let tr = [f1 * 10.0, f2 * 3.0, f1 - f2];
            put_f32s(ctx, jt + 0x38, &tr);
            fns::HSD_JObjSetMtxDirty(ctx, handle(ctx, jt));
            let j = &mut objs.jobjs[jo];
            (j.translate.x, j.translate.y, j.translate.z) = (tr[0], tr[1], tr[2]);
            jobj::set_mtx_dirty(&mut objs.jobjs, Some(jo));
            true
        }
        18 => {
            // A particle made directly: at a link's head, run at once or not, or after
            // another.
            let (bank, lo, hi) = banks[(r1 % banks.len() as u64) as usize];
            let idx = lo + ((r1 >> 8) % (hi - lo).max(1) as u64) as i32;
            let Some(cl) = objs.particles.banks[bank].unwrap().cmd_list(data, idx) else {
                return Some("null command list");
            };
            let (bt, _, _) = bases[cl.0.archive.0 as usize];
            let after = p.filter(|_| (r1 >> 20).is_multiple_of(3));
            // Every particle the game makes comes from a generator (a child takes its
            // parent's); commands and tornado motion reach through it.
            let Some(gen_) = g.map(Some) else {
                return Some("no generator");
            };
            // The game puts a particle after another only for a child, in the parent's link
            // and unrun: run at once it would unlink itself from the link's head.
            let interpret = after.is_none() && (r1 >> 28).is_multiple_of(2);
            let link = match after {
                Some((_, o)) => objs.particles[o].link_no,
                None => ((r1 >> 32) % 3) as u8,
            };
            let spawn = particle::Spawn {
                link_no: link,
                bank: bank as u8,
                kind: cl.kind(data),
                tex_group: cl.tex_group(data),
                list: Some(cl.list()),
                life: i32::from(cl.life(data)),
                palflag: ((r1 >> 40) % 2) as i32,
                pos: melee_sim::vec::Vec3::new(f1, f2, f1 * f2),
                vel: melee_sim::vec::Vec3::new(f2, f1, 0.5),
                size: 1.0,
                grav: 0.01,
                fric: 0.98,
                generator: gen_.map(|(_, o)| o),
            };
            let sp = spawn;
            both(
                &what,
                || {
                    fns::psGenerateParticle0(
                        ctx,
                        handle(ctx, after.map_or(0, |(t, _)| t)),
                        i32::from(sp.link_no),
                        i32::from(sp.bank),
                        sp.kind,
                        sp.tex_group,
                        handle(ctx, bt + cl.list().offset),
                        sp.life,
                        sp.palflag,
                        arg(sp.pos.x),
                        arg(sp.pos.y),
                        arg(sp.pos.z),
                        arg(sp.vel.x),
                        arg(sp.vel.y),
                        arg(sp.vel.z),
                        arg(sp.size),
                        arg(sp.grav),
                        arg(sp.fric),
                        handle(ctx, gen_.map_or(0, |(t, _)| t)),
                        i32::from(interpret),
                    );
                },
                || {
                    particle::generate(objs, data, hooks, after.map(|(_, o)| o), &spawn, interpret);
                },
            )
        }
        19 => {
            // A link's first particle run directly.
            let link = (r1 % 3) as usize;
            let (tl, ol) = (s_link(ctx, link), objs.particles.links[link]);
            let Some(o) = ol else {
                return Some("empty link");
            };
            let mut t_next = 0;
            let mut o_next = None;
            let carried_on = both(
                &what,
                || t_next = fns::hsd_8039930C(ctx, handle(ctx, tl), handle(ctx, 0)).addr(),
                || o_next = particle::execute(objs, data, hooks, o, None),
            );
            // The next particle is the link's head or one after it; compare where.
            if carried_on {
                rt = i64::from(t_next == 0);
                ro = i64::from(o_next.is_none());
            }
            carried_on
        }
        20 => both(
            &what,
            || {
                fns::hsd_8039D9C8(ctx);
            },
            || {
                generator::alloc(objs);
            },
        ),
        21 => both(
            &what,
            || rt = i64::from(fns::hsd_8039D1EC(ctx)),
            || ro = i64::from(generator::next_idnum(objs)),
        ),
        22 => {
            let Some((gt, go)) = g else {
                return Some("no generator");
            };
            let list = std::iter::successors(objs.generators.head, |&x| objs.generators[x].next)
                .collect::<Vec<_>>();
            let at = list.iter().position(|&x| x == go).unwrap();
            let po = at.checked_sub(1).map(|i| list[i]);
            let pt = if at == 0 {
                0
            } else {
                let mut t = ctx.read_u32(sym("hsd_804D78FC"));
                for _ in 1..at {
                    t = ctx.read_u32(t);
                }
                t
            };
            let (mut tr, mut or) = (0u32, None);
            let carried_on = both(
                &what,
                || tr = fns::hsd_8039D3AC(ctx, handle(ctx, gt), handle(ctx, pt)).addr(),
                || or = generator::remove(objs, data, hooks, go, po),
            );
            rt = i64::from(if tr == 0 {
                0
            } else if tr == gt {
                1
            } else {
                2
            });
            ro = i64::from(match or {
                None => 0,
                Some(x) if x == go => 1,
                Some(_) => 2,
            });
            carried_on
        }
        23 => {
            let status = (r1 % 3) as i32;
            let idnum = (r1 >> 8) as u16;
            both(
                &what,
                || {
                    fns::psAddGeneratorAppSRT(ctx, status, idnum);
                },
                || {
                    appsrt::add(objs, status, idnum);
                },
            )
        }
        24 => {
            let Some((gt, go)) = g else {
                return Some("no generator");
            };
            let o = &objs.generators[go];
            let Some(list) = o.cmd_list else {
                return Some("generator without commands");
            };
            let (bt, _, _) = bases[list.archive.0 as usize];
            let spawn = particle::Spawn {
                link_no: o.link_no,
                bank: o.bank,
                kind: o.kind,
                tex_group: u16::from(o.tex_group),
                list: Some(list),
                life: i32::from(o.life),
                palflag: 0,
                pos: melee_sim::vec::Vec3::new(f1, f2, 0.0),
                vel: melee_sim::vec::Vec3::new(f2 * 0.1, f1, -f1),
                size: o.size,
                grav: o.grav,
                fric: o.fric,
                generator: Some(go),
            };
            let sp = spawn;
            both(
                &what,
                || {
                    fns::hsd_80398F0C(
                        ctx,
                        i32::from(sp.link_no),
                        i32::from(sp.bank),
                        sp.kind as i32,
                        sp.tex_group,
                        handle(ctx, bt + list.offset),
                        sp.life,
                        0,
                        handle(ctx, gt),
                        arg(sp.pos.x),
                        arg(sp.pos.y),
                        arg(sp.pos.z),
                        arg(sp.vel.x),
                        arg(sp.vel.y),
                        arg(sp.vel.z),
                        arg(sp.size),
                        arg(sp.grav),
                        arg(sp.fric),
                    )
                },
                || particle::generate_for(objs, data, hooks, &spawn),
            )
        }
        25 => {
            let Some((pt, po)) = p else {
                return Some("no particle");
            };
            both(
                &what,
                || fns::hsd_8039D048(ctx, handle(ctx, pt)),
                || particle::release_point_jobj(objs, po),
            )
        }
        _ => true,
    };
    if !carried_on {
        return None;
    }
    assert_eq!(rt, ro, "{what}: result");
    Some(name)
}

/// A link's first particle on their side.
fn s_link(ctx: &Ctx, link: usize) -> u32 {
    ctx.read_u32(sym("hsd_804D0908") + 4 * link as u32)
}

/// A DAT under construction.
#[derive(Default)]
struct Builder {
    data: Vec<u8>,
}

impl Builder {
    fn alloc(&mut self, n: usize) -> u32 {
        let at = self.data.len() as u32;
        self.data.resize(self.data.len() + n.next_multiple_of(4), 0);
        at
    }

    fn u32(&mut self, at: u32, v: u32) {
        self.data[at as usize..at as usize + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn u16(&mut self, at: u32, v: u16) {
        self.data[at as usize..at as usize + 2].copy_from_slice(&v.to_be_bytes());
    }

    fn f32(&mut self, at: u32, v: f32) {
        self.u32(at, v.to_bits());
    }
}

/// A random float's bytes in `lo..hi`, now and then one of `special`.
fn float(r: &mut Rand, lo: f32, hi: f32, special: &[f32], out: &mut Vec<u8>) {
    let v = if !special.is_empty() && r.below(4) == 0 {
        special[r.below(special.len() as u64) as usize]
    } else {
        r.range(lo, hi)
    };
    out.extend(v.to_be_bytes());
}

/// A count: one byte under 0x80, or two with the first's top bit set.
fn count(r: &mut Rand, out: &mut Vec<u8>) {
    let c = if r.below(4) == 0 {
        0
    } else {
        r.below(40) as u16
    };
    if r.below(4) == 0 {
        out.extend([0x80 | (c >> 8) as u8, c as u8]);
    } else {
        out.push(c as u8);
    }
}

/// A wait of a few frames.
fn wait(r: &mut Rand, out: &mut Vec<u8>) {
    if r.below(5) == 0 {
        out.extend([0x20, 1 + r.below(20) as u8]);
    } else {
        out.push(1 + r.below(8) as u8);
    }
}

/// One random command for list `i` of `n` (spawning only later lists, so spawning ends).
fn command(r: &mut Rand, i: usize, n: usize, out: &mut Vec<u8>) {
    let later = |r: &mut Rand| (i + 1 + r.below((n - i - 1) as u64) as usize) as u16;
    let can_spawn = i + 1 < n;
    let b = |r: &mut Rand| r.below(256) as u8;
    match r.below(64) {
        0..=2 => {
            out.push(0x40 | r.below(4) as u8);
            out.push(if r.below(8) == 0 {
                255
            } else {
                r.below(10) as u8
            });
        }
        3..=6 => {
            let op = [0x80u8, 0x88, 0x90, 0x98][r.below(4) as usize] | (1 + r.below(7) as u8);
            out.push(op);
            for k in 0..3 {
                if op & (1 << k) != 0 {
                    float(r, -3.0, 3.0, &[0.0], out);
                }
            }
        }
        7 => {
            out.push(0xA0);
            count(r, out);
            float(r, 0.0, 3.0, &[0.0], out);
        }
        8 => out.push(0xA1),
        9 => {
            out.push(0xA2);
            float(r, -0.05, 0.05, &[0.0], out);
        }
        10 => {
            out.push(0xA3);
            float(r, 0.85, 1.0, &[1.0], out);
        }
        11..=13 if can_spawn => {
            out.push([0xA4, 0xF1, 0xB9, 0xF2][r.below(4) as usize]);
            out.extend(later(r).to_be_bytes());
        }
        14 | 15 if can_spawn => {
            let op = [0xA5, 0xEF, 0xF0][r.below(3) as usize];
            out.push(op);
            out.extend(later(r).to_be_bytes());
            if op != 0xA5 {
                out.push(b(r));
            }
        }
        16 => {
            out.push(0xA6);
            out.extend((1 + r.below(60) as u16).to_be_bytes());
            out.extend((r.below(30) as u16).to_be_bytes());
        }
        17 => out.extend([0xA7, r.below(12) as u8]),
        18 => {
            out.push(0xA8);
            for _ in 0..3 {
                float(r, 0.0, 3.0, &[0.0], out);
            }
        }
        19 => {
            out.push(0xA9);
            float(r, 0.0, 1.5, &[0.0], out);
        }
        20 if can_spawn => {
            let j = later(r);
            out.push(0xAA);
            out.extend(j.to_be_bytes());
            out.extend((r.below((n - j as usize + 1) as u64) as u16).to_be_bytes());
        }
        21 => {
            out.push(0xAB);
            float(r, 0.5, 1.5, &[], out);
        }
        22 => {
            out.push(0xAC);
            count(r, out);
            float(r, 0.0, 3.0, &[], out);
            float(r, -1.0, 1.0, &[], out);
        }
        23 => out.push(0xAD + r.below(6) as u8),
        24 => out.push(0xB2),
        25 => {
            out.push(0xB3);
            count(r, out);
            out.extend([b(r), b(r), b(r)]);
        }
        26 => out.push(0xB4 + r.below(2) as u8),
        27 => {
            out.push(0xB6);
            count(r, out);
            float(r, -3.0, 3.0, &[], out);
        }
        28 => out.extend([0xB7, r.below(8) as u8]),
        29 => {
            out.extend([0xB8, r.below(8) as u8]);
            float(r, -5.0, 5.0, &[0.0], out);
            float(r, -1.0, 10.0, &[], out);
        }
        30 | 31 => {
            out.push(0xBA + r.below(2) as u8);
            out.extend([b(r), b(r), b(r), b(r)]);
        }
        32 => out.extend([0xBC, r.below(6) as u8, r.below(5) as u8]),
        33 => {
            out.push(0xBD);
            float(r, 0.0, 3.0, &[], out);
            float(r, -1.0, 1.0, &[0.0], out);
        }
        34 => {
            out.push(0xBE);
            for _ in 0..3 {
                float(r, 0.5, 1.5, &[], out);
            }
        }
        35 => out.extend([0xBF, b(r)]),
        36 | 37 => {
            let mask = r.below(16) as u8;
            out.push([0xC0, 0xD0][r.below(2) as usize] | mask);
            count(r, out);
            for k in 0..4 {
                if mask & (1 << k) != 0 {
                    out.push(b(r));
                }
            }
        }
        38 => {
            out.push(0xE0);
            out.extend([b(r), b(r), b(r), b(r)]);
        }
        39 => {
            let flags = r.below(64) as u8;
            out.extend([0xE9, flags, r.below(5) as u8]);
            for k in 0..4 {
                if flags & (1 << k) != 0 {
                    out.push(b(r));
                }
            }
        }
        40 => out.extend([0xE1, r.below(3) as u8]),
        41 => {
            out.extend([0xEC, b(r)]);
            float(r, -10.0, 10.0, &[], out);
        }
        42 => out.push(0xE2),
        43 => out.extend([0xE3, b(r)]),
        44 => out.extend([0xE4 + r.below(2) as u8, b(r)]),
        45 => out.push(0xE6 + r.below(2) as u8),
        46 => {
            out.push(0xE8);
            float(r, -1.0, 2.0, &[], out);
        }
        47 => {
            let flags = [0, 1, 8, 9][r.below(4) as usize];
            out.push(0xEA + r.below(2) as u8);
            count(r, out);
            out.push(flags);
            if flags & 1 != 0 {
                out.push(b(r));
            }
            if flags & 8 != 0 {
                out.push(b(r));
            }
        }
        48 => {
            out.push(0xED);
            float(r, -1.0, 1.0, &[], out);
            float(r, -1.0, 1.0, &[], out);
            out.push(r.below(5) as u8);
        }
        // Commands nothing handles.
        49 => out.push([0xEE, 0xF3, 0xF9, 0xF5][r.below(4) as usize]),
        _ => wait(r, out),
    }
}

/// A random command stream for list `i` of `n`.
fn stream(r: &mut Rand, i: usize, n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..1 + r.below(10) {
        if r.below(10) == 0 {
            out.extend([0xFA, 2 + r.below(3) as u8]);
            command(r, i, n, &mut out);
            wait(r, &mut out);
            out.push(0xFB);
        } else {
            command(r, i, n, &mut out);
        }
    }
    match r.below(6) {
        0 => {
            // A loop that lasts until the particle's life runs out.
            out.push(0xFC);
            command(r, i, n, &mut out);
            wait(r, &mut out);
            out.push(0xFD);
        }
        1 => out.push(0xFE),
        _ => out.push(0xFF),
    }
    out
}

/// Offsets of a built bank's parts.
struct Built {
    file: Vec<u8>,
    cmd: u32,
    tex: u32,
    remap: u32,
    form: u32,
}

/// A random effect bank reaching every command, generator shape and branch: `n` command
/// lists (indices from 0) whose streams spawn only later lists, texture groups with null and
/// non-null poses, a shape bank and an index remap that maps each index to it or a later one.
fn synthetic_bank(r: &mut Rand, n: usize) -> Built {
    let mut b = Builder::default();
    let groups = 1 + r.below(5) as u32;
    let cmd = b.alloc(12 + 4 * n);
    b.u16(cmd, 0x42);
    b.u32(cmd + 4, 0);
    b.u32(cmd + 8, n as u32);
    let bits = [1u32, 2, 0x100, 0x20000, 0x40_0000, 8, 0x20, 4];
    for i in 0..n {
        let s = stream(r, i, n);
        let cl = b.alloc(0x3C + s.len());
        b.u32(cmd + 12 + 4 * i as u32, cl - cmd);
        let shape = if r.below(12) == 0 {
            9
        } else {
            r.below(9) as u16
        };
        b.u16(cl, shape | if r.below(6) == 0 { 0x80 } else { 0 });
        b.u16(cl + 2, r.below(u64::from(groups)) as u16);
        b.u16(
            cl + 4,
            if r.below(10) == 0 {
                0
            } else {
                1 + r.below(40) as u16
            },
        );
        b.u16(cl + 6, 1 + r.below(80) as u16);
        let mut kind = 0;
        for (k, &bit) in bits.iter().enumerate() {
            if r.below(if k < 5 { 3 } else { 12 }) == 0 {
                kind |= bit;
            }
        }
        b.u32(cl + 8, kind);
        let mut f = |at: u32, v: f32| b.f32(cl + at, v);
        f(
            0xC,
            if r.below(3) == 0 {
                0.0
            } else {
                r.range(-0.05, 0.05)
            },
        );
        f(
            0x10,
            if r.below(3) == 0 {
                1.0
            } else {
                r.range(0.85, 1.0)
            },
        );
        let still = r.below(8) == 0;
        for at in [0x14, 0x18, 0x1C] {
            f(at, if still { 0.0 } else { r.range(-2.0, 2.0) });
        }
        f(0x20, r.range(-3.0, 3.0));
        f(
            0x24,
            if r.below(5) == 0 {
                0.0
            } else {
                r.range(-1.5, 1.5)
            },
        );
        f(0x28, r.range(-1.5, 1.5));
        f(0x2C, r.range(0.1, 3.0));
        let pi = std::f32::consts::PI;
        let params: [f32; 3] = match shape {
            0 | 3 | 4 | 6 | 7 if r.below(3) == 0 => [0.0, 0.0, r.range(-5.0, 5.0)],
            0 | 3 | 4 | 6 | 7 => [r.range(0.0, 3.0), r.range(3.0, 6.3), r.range(-5.0, 5.0)],
            8 => [
                [0.0, pi, -pi, r.range(-1.5, 1.5)][r.below(4) as usize],
                r.range(-1.5, 1.5),
                r.range(0.0, 3.0),
            ],
            _ => [r.range(-4.0, 4.0), r.range(-4.0, 4.0), r.range(-4.0, 4.0)],
        };
        for (k, v) in params.into_iter().enumerate() {
            f(0x30 + 4 * k as u32, v);
        }
        let at = cl as usize + 0x3C;
        b.data[at..at + s.len()].copy_from_slice(&s);
    }
    let tex = b.alloc(4 + 4 * groups as usize);
    b.u32(tex, groups);
    for g in 0..groups {
        let num = 1 + r.below(6) as u32;
        let tg = b.alloc(0x18 + 4 * (2 * num as usize + 1));
        b.u32(tex + 4 + 4 * g, tg - tex);
        b.u32(tg, num);
        let palette = r.below(3) == 0;
        b.u32(
            tg + 4,
            if palette {
                8 + r.below(2) as u32
            } else {
                [0, 1, 3, 4, 5, 6, 14][r.below(7) as usize]
            },
        );
        b.u32(tg + 0xC, 8);
        b.u32(tg + 0x10, 8);
        b.u16(tg + 0x16, if r.below(3) == 0 { 1 } else { 0 });
        for k in 0..2 * num + 1 {
            if r.below(3) != 0 {
                b.u32(tg + 0x18 + 4 * k, tg - tex + 4);
            }
        }
    }
    let form = b.alloc(4 + 4 * groups as usize);
    b.u32(form, groups);
    for g in 0..groups {
        let num = 1 + r.below(3) as u32;
        let fg = b.alloc(4 + 4 * num as usize);
        b.u32(form + 4 + 4 * g, fg - form);
        b.u32(fg, num);
        for k in 0..num {
            b.u32(fg + 4 + 4 * k, 4);
        }
    }
    let remap = b.alloc(4 * n);
    for k in 0..n {
        b.u32(
            remap + 4 * k as u32,
            (k + r.below((n - k) as u64) as usize) as u32,
        );
    }
    // Room for texture poses read past their group.
    b.alloc(0x1000);
    Built {
        file: leaf_check::dat(&b.data, &[]),
        cmd,
        tex,
        remap,
        form,
    }
}

/// The common bank, which efLib's hooks read: bank 0.
fn common(co: &Archive) -> BankSpec {
    let t = co.public("effCommonDataTable").expect("effCommonDataTable");
    BankSpec {
        bank: 0,
        archive: 0,
        cmd: co.ptr(t).unwrap(),
        tex: co.ptr(t + 4).unwrap(),
        remap: None,
        form: None,
        game_way: false,
    }
}

/// A bank's first index and how many, from its header.
fn bank_range(a: &Archive, cmd: u32) -> (i32, i32) {
    let first = a.u32(cmd + 4) as i32;
    (first, first + a.u32(cmd + 8) as i32)
}

#[test]
fn particles_match() {
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(check_all)
        .unwrap()
        .join()
        .unwrap();
}

fn check_all() {
    if hsd_reference().is_none() {
        return;
    }
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let co_bytes = disc.read_file("EfCoData.dat").unwrap();
    let co = Archive::parse(&co_bytes).unwrap();
    let co_bank = common(&co);
    let (_, co_count) = bank_range(&co, co_bank.cmd);
    let mut plans = Vec::new();
    // Every effect file's bank and every stage's, each with the common bank.
    for name in disc.files().unwrap() {
        let is_effect = name.starts_with("Ef") && name.ends_with(".dat");
        let is_stage = name.starts_with("Gr") && name.ends_with(".dat");
        if !(is_effect || is_stage) || name == "EfCoData.dat" {
            continue;
        }
        let bytes = disc.read_file(&name).unwrap();
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let banks: Vec<(u32, u32, bool)> = a
            .publics()
            .filter_map(|(p, o)| {
                if p.ends_with("DataTable") {
                    Some((a.ptr(o)?, a.ptr(o + 4)?, false))
                } else if p == "map_ptcl" {
                    Some((o, a.public("map_texg")?, true))
                } else {
                    None
                }
            })
            .collect();
        if let Some((cmd, tex, stage)) = banks.into_iter().next() {
            let (first, end) = bank_range(&a, cmd);
            let bank = (first / 1000) as usize;
            plans.push(Plan {
                name,
                files: vec![co_bytes.clone(), bytes.clone()],
                specs: vec![
                    co_bank.clone(),
                    BankSpec {
                        bank,
                        archive: 1,
                        cmd,
                        tex,
                        remap: None,
                        form: None,
                        // Stages locate their bank when the file loads and register it later.
                        game_way: stage,
                    },
                ],
                first: (first..end).map(|i| (bank, i)).collect(),
                frames: 150,
                synthetic: false,
            });
        }
    }
    plans.push(Plan {
        name: "EfCoData.dat".into(),
        files: vec![co_bytes.clone()],
        specs: vec![co_bank.clone()],
        first: Vec::new(),
        frames: 150,
        synthetic: false,
    });
    // The common bank's lists spread over every plan.
    let per = (co_count as usize).div_ceil(plans.len());
    for (k, plan) in plans.iter_mut().enumerate() {
        let lo = (k * per) as i32;
        let hi = ((k + 1) * per).min(co_count as usize) as i32;
        plan.first.extend((lo..hi).map(|i| (0, i)));
    }
    let mut rand = Rand(0xA076_1D64_78BD_642F);
    for k in 0..16 {
        let n = 20 + rand.below(20) as usize;
        let built = synthetic_bank(&mut rand, n);
        let bank = 50;
        plans.push(Plan {
            name: format!("random bank {k}"),
            files: vec![co_bytes.clone(), built.file],
            specs: vec![
                co_bank.clone(),
                BankSpec {
                    bank,
                    archive: 1,
                    cmd: built.cmd,
                    tex: built.tex,
                    remap: Some(built.remap),
                    form: Some(built.form),
                    game_way: k % 2 == 1,
                },
            ],
            first: (0..n as i32)
                .chain(0..n as i32)
                .map(|i| (bank, i))
                .collect(),
            frames: 250,
            synthetic: true,
        });
    }
    let mut counts = Counts::default();
    for (k, plan) in plans.iter().enumerate() {
        if let Ok(only) = std::env::var("PTCL_ONLY")
            && !plan.name.contains(&only)
        {
            continue;
        }
        if !run(plan, &mut rand, &mut counts, MODES[k % 2]) {
            counts.halted += 1;
        }
    }
    set_fp_mode(FpMode::Hardware);
    let mut ops: Vec<_> = counts.ops.iter().collect();
    ops.sort();
    eprintln!(
        "{} sessions ({} halted where the original does), {} frames, {} edits, up to {} particles and {} generators; shapes {:?}; {} lists spawned; {ops:?}",
        counts.sessions,
        counts.halted,
        counts.frames,
        counts.steps,
        counts.most_particles,
        counts.most_generators,
        counts.shapes,
        counts.spawned.len(),
    );
    if std::env::var_os("PTCL_ONLY").is_none() {
        // Every command list on the disc made its generator.
        for plan in plans.iter().filter(|p| !p.synthetic) {
            let a = Archive::parse(plan.files.last().unwrap()).unwrap();
            let spec = plan.specs.last().unwrap();
            let b = &plan.specs[plan.specs.len() - 1];
            let (first, end) = bank_range(&a, b.cmd);
            for i in first..end {
                let w = a.u32(b.cmd + 12 + 4 * (i - first) as u32);
                if w != 0 {
                    assert!(
                        counts.spawned.contains_key(&(spec.bank, i)),
                        "{}: list {i} never spawned",
                        plan.name
                    );
                }
            }
        }
        assert!(
            counts.shapes[..10].iter().all(|&c| c > 0),
            "shapes {:?}",
            counts.shapes
        );
    }
}
