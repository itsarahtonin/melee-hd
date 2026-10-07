//! melee-sim's AObj (animation objects) against ssbm-rs's port, on every AObjDesc the disc's
//! `*animjoint` and `*matanim_joint` roots reach: random sequences of every AObj operation
//! (interpreting with and without an update function, rates, flags, requests, current, rewind
//! and end frames, stops, FObj swaps, removal and reloading), comparing the values handed to
//! the update function, the AObj, its FObj chain, the pools' identities and the end-callback
//! counts after every step, in both float modes. Also `fmodf` and the runtime conversions it
//! calls, input by input.

use std::cell::RefCell;
use std::collections::HashMap;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{arg, handle, inputs, reference, sym};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::aobj::{self, AObjId, AObjs};
use melee_sim::hsd::fobj::{self, FObjId, FObjs};
use melee_sim::{math, ppc};
use ssbm_rt::{Addr, Ctx, Handle, Single};
use ssbm_types::fns;
use ssbm_types::records::{_HSD_FObjDesc, HSD_AObj, HSD_AObjDesc, HSD_Archive, HSD_FObj};

const ARCHIVE: u32 = 0x8070_0000;
const FILE: u32 = 0x8080_0000;
const FILE_MAX: usize = 0x0070_0000;
const OBJ_HEAP: u32 = 0x8100_0000;
const UPDATE: u32 = 0x817E_0400;
/// The animated object the update function is handed; only passed through.
const OBJ: u32 = 0x817E_0800;

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];

thread_local! {
    static UPDATES: RefCell<Vec<(u8, u32)>> = const { RefCell::new(Vec::new()) };
}

fn native_update(ctx: &Ctx) {
    assert_eq!(ctx.regs.r(3), OBJ, "update handed another object");
    let ty = ctx.regs.r(4) as u8;
    let value = ctx.read_u32(ctx.regs.r(5));
    UPDATES.with(|u| u.borrow_mut().push((ty, value)));
}

fn take_updates() -> Vec<(u8, u32)> {
    UPDATES.with(|u| std::mem::take(&mut *u.borrow_mut()))
}

#[test]
fn runtime_conversions_match() {
    let Some(ctx) = reference() else { return };
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut ints: Vec<i64> = vec![0, 1, -1, i64::MAX, i64::MIN, 1 << 24, (1 << 24) + 1];
    // Halfway cases at both roundings, to double and then to single, which random values
    // almost never hit: the top bit, a single-precision half, a double-precision half.
    for top in 54..63 {
        for single_half in [0u64, 1 << (top - 24), 3 << (top - 25)] {
            for double_half in [0u64, 1 << (top - 53), 3 << (top - 54)] {
                for low in [-1i64, 0, 1] {
                    let x = ((1u64 << top) | single_half | double_half) as i64 + low;
                    ints.extend([x, -x]);
                }
            }
        }
    }
    let mut doubles: Vec<f64> = inputs(1.0e6, 20_000).into_iter().map(arg).collect();
    doubles.extend([
        9.3e18,
        -9.3e18,
        1.0e19,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.999_999,
        -0.999_999,
        4_503_599_627_370_497.0,
    ]);
    for _ in 0..50_000 {
        let r = next();
        ints.push(r as i64);
        ints.push((r >> (r % 64)) as i64);
        doubles.push(f64::from_bits(next()));
    }
    for mode in MODES {
        set_fp_mode(mode);
        for &x in &ints {
            assert_eq!(
                ppc::cvt_sll_flt(x).to_bits(),
                fns::__cvt_sll_flt(&ctx, x).to_bits(),
                "__cvt_sll_flt({x}) in {mode:?}"
            );
        }
        for &d in &doubles {
            assert_eq!(
                ppc::cvt_dbl_usll(d),
                fns::__cvt_dbl_usll(&ctx, d),
                "__cvt_dbl_usll({d:e} = {:#018x}) in {mode:?}",
                d.to_bits()
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn fmodf_matches() {
    let Some(ctx) = reference() else { return };
    let fmodf = sym("fmodf");
    let v = inputs(300.0, 20_000);
    for mode in MODES {
        set_fp_mode(mode);
        // Random pairs, then equal magnitudes and exact multiples, where the comparison and
        // the remainder's sign decide.
        let pairs = (0..v.len())
            .map(|i| (v[i], v[(i * 7 + 3) % v.len()]))
            .chain(
                v.iter()
                    .flat_map(|&x| [(x, x), (x, -x), (x * 3.0, x), (-x, x * 0.5)]),
            );
        for (x, y) in pairs {
            let (x, y) = (arg(x), arg(y));
            let theirs: f64 = ctx.call(fmodf, (Single::round(x), Single::round(y)));
            let ours = math::fmod(x, y);
            assert_eq!(
                ours.to_bits(),
                theirs.to_bits(),
                "fmodf({x:e}, {y:e}) in {mode:?}"
            );
        }
    }
    set_fp_mode(FpMode::Hardware);
}

/// Every AObjDesc reachable from the archive's `*animjoint` and `*matanim_joint` publics.
///
/// A stage's `map_head` model groups aren't walked yet: some of their channels (GrCn.dat's)
/// end right after their first key, and the original then hands the update function stale
/// stack contents, which melee-sim's FObj doesn't reproduce (see `FObjUpdateAnim` in
/// `hsd::fobj`).
fn aobj_descs(a: &Archive) -> Vec<u32> {
    let fits = |o: u32, n: u32| o as usize + n as usize <= a.data().len();
    let lower = |n: &str| n.to_ascii_lowercase();
    let mut anim: Vec<u32> = a
        .publics()
        .filter(|(n, _)| lower(n).contains("animjoint") && !lower(n).contains("matanim"))
        .map(|(_, o)| o)
        .collect();
    let mut matanim: Vec<u32> = a
        .publics()
        .filter(|(n, _)| lower(n).contains("matanim_joint"))
        .map(|(_, o)| o)
        .collect();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // AnimJoint: child, next, aobjdesc, robj_anim, flags.
    while let Some(aj) = anim.pop() {
        if !seen.insert(aj) || !fits(aj, 0x14) {
            continue;
        }
        anim.extend(a.ptr(aj));
        anim.extend(a.ptr(aj + 4));
        out.extend(a.ptr(aj + 8));
    }
    // MatAnimJoint: child, next, matanim. MatAnim: next, aobjdesc, texanim, renderanim.
    // TexAnim: next, id, aobjdesc, ...
    seen.clear();
    while let Some(mj) = matanim.pop() {
        if !seen.insert(mj) || !fits(mj, 0xC) {
            continue;
        }
        matanim.extend(a.ptr(mj));
        matanim.extend(a.ptr(mj + 4));
        let mut ma = a.ptr(mj + 8);
        while let Some(m) = ma.filter(|&m| fits(m, 0x10)) {
            out.extend(a.ptr(m + 4));
            let mut ta = a.ptr(m + 8);
            while let Some(t) = ta.filter(|&t| fits(t, 0xC)) {
                out.extend(a.ptr(t + 8));
                ta = a.ptr(t);
            }
            ma = a.ptr(m);
        }
    }
    out.retain(|&d| fits(d, 0x10));
    out.sort_unstable();
    out.dedup();
    out
}

/// A one-to-one map between ssbm-rs's object addresses and our slots: both pools hand out
/// and reuse objects in the same order, so an address always pairs with the same slot.
#[derive(Default)]
struct Identity {
    to_ours: HashMap<u32, u32>,
    to_theirs: HashMap<u32, u32>,
}

impl Identity {
    fn check(&mut self, theirs: u32, ours: Option<u32>, what: &str) {
        match ours {
            None => assert_eq!(theirs, 0, "{what}: theirs {theirs:#x}, ours null"),
            Some(o) => {
                assert_ne!(theirs, 0, "{what}: theirs null, ours slot {o}");
                let t = *self.to_theirs.entry(o).or_insert(theirs);
                let s = *self.to_ours.entry(theirs).or_insert(o);
                assert_eq!(
                    (t, s),
                    (theirs, o),
                    "{what}: {theirs:#x} and slot {o} don't pair"
                );
            }
        }
    }
}

struct Sides<'c> {
    ctx: &'c Ctx,
    data_addr: u32,
    archives: Archives,
    id: ArchiveId,
    aobjs: AObjs,
    fobjs: FObjs,
    aobj_ids: Identity,
    fobj_ids: Identity,
}

impl Sides<'_> {
    fn desc(&self, offset: u32) -> DataRef {
        DataRef {
            archive: self.id,
            offset,
        }
    }

    fn fobj_fields_theirs(&self, a: u32) -> String {
        let ctx = self.ctx;
        let f = |o| ctx.read_u32(a + o);
        // A loaded FObj's cursor is null until its first request sets it, which nothing reads
        // before; ours holds offset 0 there.
        let ad = match f(4) {
            0 => 0,
            p => p.wrapping_sub(self.data_addr),
        };
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            ad,
            f(8).wrapping_sub(self.data_addr),
            f(0xC),
            ctx.read_u8(a + 0x10),
            ctx.read_u8(a + 0x11),
            ctx.read_u8(a + 0x12),
            ctx.read_u8(a + 0x13),
            ctx.read_u8(a + 0x14),
            ctx.read_u8(a + 0x15),
            ctx.read_u16(a + 0x16),
            ctx.read_u16(a + 0x18) as i16,
            ctx.read_u16(a + 0x1A),
            f(0x1C),
            f(0x20),
            f(0x24),
            f(0x28),
            f(0x2C),
        )
    }

    fn fobj_fields_ours(&self, id: FObjId) -> String {
        let o = &self.fobjs[id.0];
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            o.ad,
            o.ad_head,
            o.length,
            o.flags,
            o.op,
            o.op_intrp,
            o.obj_type,
            o.frac_value,
            o.frac_slope,
            o.nb_pack,
            o.startframe,
            o.fterm,
            o.time.to_bits(),
            o.p0.to_bits(),
            o.p1.to_bits(),
            o.d0.to_bits(),
            o.d1.to_bits(),
        )
    }

    /// Compares an AObj, its whole FObj chain, and the end-callback counts.
    fn compare(&mut self, theirs: u32, ours: Option<AObjId>, what: &str) {
        let ctx = self.ctx;
        self.aobj_ids.check(theirs, ours.map(|a| a.0), what);
        assert_eq!(
            (
                ctx.read_u32(sym("HSD_AObj_804D762C")) as i32,
                ctx.read_u32(sym("HSD_AObj_804D7630")) as i32
            ),
            (self.aobjs.stopped, self.aobjs.playing),
            "{what}: end-callback counts"
        );
        let Some(a) = ours else { return };
        let o = &self.aobjs[a];
        let r = |off| ctx.read_u32(theirs + off);
        assert_eq!(
            [r(0), r(4), r(8), r(0xC), r(0x10), r(0x18)],
            [
                o.flags,
                o.curr_frame.to_bits(),
                o.rewind_frame.to_bits(),
                o.end_frame.to_bits(),
                o.framerate.to_bits(),
                0
            ],
            "{what}: flags, curr, rewind, end, rate, hsd_obj"
        );
        let (mut tf, mut of) = (r(0x14), o.fobj);
        loop {
            self.fobj_ids.check(tf, of.map(|f| f.0), what);
            let Some(f) = of else { break };
            assert_eq!(
                self.fobj_fields_ours(f),
                self.fobj_fields_theirs(tf),
                "{what}: FObj"
            );
            tf = ctx.read_u32(tf);
            of = self.fobjs[f.0].next;
        }
    }

    fn load(&mut self, desc: u32) -> (u32, Option<AObjId>) {
        let t = fns::HSD_AObjLoadDesc(
            self.ctx,
            handle::<HSD_AObjDesc>(self.ctx, self.data_addr + desc),
        )
        .addr();
        let d = self.desc(desc);
        let o = aobj::load_desc(&mut self.aobjs, &mut self.fobjs, &self.archives, Some(d));
        (t, o)
    }

    fn load_fobjs(&mut self, fdesc: Option<u32>) -> (u32, Option<FObjId>) {
        let ctx = self.ctx;
        let t = match fdesc {
            Some(d) => {
                fns::HSD_FObjLoadDesc(ctx, handle::<_HSD_FObjDesc>(ctx, self.data_addr + d)).addr()
            }
            None => 0,
        };
        let d = fdesc.map(|d| self.desc(d));
        let o = fobj::load_desc(&mut self.fobjs, &self.archives, d);
        (t, o)
    }
}

#[derive(Default)]
struct Counts {
    descs: u64,
    bound_skipped: u64,
    steps: u64,
    updates: u64,
    /// Interprets that looped back to the rewind frame, held at the end frame, or stopped.
    loops: u64,
    holds: u64,
    ends: u64,
    /// Steps that left a keyed channel (`HSD_A_OP_KEY`) current.
    keyed: u64,
    ops: HashMap<&'static str, u64>,
}

#[test]
fn aobjs_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    ctx.register(UPDATE, native_update);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut counts = Counts::default();
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for name in disc.files().unwrap().iter().filter(|f| f.ends_with(".dat")) {
        let bytes = disc.read_file(name).unwrap();
        let Ok(archive) = Archive::parse(&bytes) else {
            continue;
        };
        let mut descs = aobj_descs(&archive);
        if descs.is_empty() || bytes.len() > FILE_MAX {
            continue;
        }
        // An AObjDesc that binds a joint needs JObj, which isn't ported. Only the Target Test
        // stages' map_head groups have them, which this doesn't walk; this counts any.
        let before = descs.len();
        descs.retain(|&d| archive.u32(d + 0xC) == 0);
        counts.bound_skipped += (before - descs.len()) as u64;
        let fdescs: Vec<u32> = descs.iter().filter_map(|&d| archive.ptr(d + 8)).collect();
        ctx.write_bytes(FILE, &bytes);
        assert_eq!(
            fns::HSD_ArchiveParse(
                ctx,
                handle::<HSD_Archive>(ctx, ARCHIVE),
                handle(ctx, FILE),
                bytes.len() as u32
            ),
            0
        );
        let data_addr = ctx.read_u32(ARCHIVE + 0x20);
        let mut archives = Archives::default();
        let id = archives.add(archive);
        let mut s = Sides {
            ctx,
            data_addr,
            archives,
            id,
            aobjs: AObjs::default(),
            fobjs: FObjs::default(),
            aobj_ids: Identity::default(),
            fobj_ids: Identity::default(),
        };
        for (di, &desc) in descs.iter().enumerate() {
            let mode = MODES[di % 2];
            set_fp_mode(mode);
            // Fresh pools and counts on both sides for each descriptor.
            fns::HSD_ObjSetHeap(ctx, 0x0040_0000, handle::<Addr>(ctx, OBJ_HEAP));
            fns::HSD_AObjInitAllocData(ctx);
            fns::HSD_FObjInitAllocData(ctx);
            assert_eq!(
                fns::HSD_AObjGetAllocData(ctx).addr(),
                sym("aobj_alloc_data")
            );
            s.aobjs = AObjs::default();
            s.fobjs = FObjs::default();
            s.aobj_ids = Identity::default();
            s.fobj_ids = Identity::default();
            fns::HSD_AObjInitEndCallBack(ctx);
            let (mut ta, mut oa) = s.load(desc);
            let what = format!("{name} desc {desc:#x} in {mode:?}");
            s.compare(ta, oa, &format!("{what}: load"));
            let end = s.aobjs[oa.unwrap()].end_frame;
            let frame = |r: u64| -> f32 {
                let unit = (r >> 40) as f32 / (1u64 << 24) as f32;
                match r % 10 {
                    0 => 0.0,
                    1 => 1.0,
                    2 => 0.5,
                    3 => end,
                    4 => end - 1.0,
                    5 => end * 0.5,
                    6 => end + 3.0,
                    7 => -2.0,
                    _ => unit * (end + 10.0),
                }
            };
            // As the game starts one: optionally looping, then requested at a start frame.
            if rand() % 2 == 0 {
                fns::HSD_AObjSetFlags(ctx, handle(ctx, ta), aobj::LOOP);
                aobj::set_flags(&mut s.aobjs, oa, aobj::LOOP);
            }
            let start = frame(rand());
            fns::HSD_AObjReqAnim(ctx, handle(ctx, ta), arg(start));
            aobj::req_anim(&mut s.aobjs, &mut s.fobjs, oa, start);
            s.compare(ta, oa, &format!("{what}: request at {start}"));
            let mut extras: Vec<(u32, Option<AObjId>)> = Vec::new();
            for n in 0..300 {
                // A stopped AObj only interprets again once requested, as the game does.
                let stopped = aobj::flags(&s.aobjs, oa) & aobj::NO_ANIM != 0;
                let r = if stopped && rand() % 4 == 0 {
                    80
                } else {
                    rand()
                };
                let mut ours_log = Vec::new();
                let op: &'static str = match r % 100 {
                    0..=69 => {
                        let null = rand() % 16 == 0;
                        let was_playing = !stopped;
                        fns::HSD_AObjInterpretAnim(
                            ctx,
                            handle(ctx, ta),
                            handle(ctx, OBJ),
                            handle(ctx, if null { 0 } else { UPDATE }),
                        );
                        let mut log = |t: u8, v: f32| ours_log.push((t, v.to_bits()));
                        aobj::interpret(
                            &mut s.aobjs,
                            &mut s.fobjs,
                            &s.archives,
                            oa,
                            if null { None } else { Some(&mut log) },
                        );
                        let flags = aobj::flags(&s.aobjs, oa);
                        if was_playing && flags & aobj::REWINDED != 0 {
                            let o = &s.aobjs[oa.unwrap()];
                            if o.rewind_frame < o.end_frame {
                                counts.loops += 1;
                            } else {
                                counts.holds += 1;
                            }
                        } else if was_playing && flags & aobj::NO_ANIM != 0 {
                            counts.ends += 1;
                        }
                        if null {
                            "interpret, no update"
                        } else {
                            "interpret"
                        }
                    }
                    70..=73 => {
                        // Rates the game sets: stopped, slowed, normal and sped up.
                        let rate = [0.0f32, 0.5, 1.0, 2.0, 1.25, 0.25][(rand() % 6) as usize];
                        fns::HSD_AObjSetRate(ctx, handle(ctx, ta), arg(rate));
                        aobj::set_rate(&mut s.aobjs, oa, rate);
                        "rate"
                    }
                    74..=76 => {
                        let f = rand() as u32;
                        fns::HSD_AObjSetFlags(ctx, handle(ctx, ta), f);
                        aobj::set_flags(&mut s.aobjs, oa, f);
                        assert_eq!(
                            fns::HSD_AObjGetFlags(ctx, handle(ctx, ta)),
                            aobj::flags(&s.aobjs, oa)
                        );
                        "set flags"
                    }
                    77..=79 => {
                        let f = rand() as u32;
                        fns::HSD_AObjClearFlags(ctx, handle(ctx, ta), f);
                        aobj::clear_flags(&mut s.aobjs, oa, f);
                        "clear flags"
                    }
                    80..=82 => {
                        let f = frame(rand());
                        fns::HSD_AObjReqAnim(ctx, handle(ctx, ta), arg(f));
                        aobj::req_anim(&mut s.aobjs, &mut s.fobjs, oa, f);
                        "request"
                    }
                    83..=84 => {
                        let f = frame(rand());
                        fns::HSD_AObjSetCurrentFrame(ctx, handle(ctx, ta), arg(f));
                        aobj::set_current_frame(&mut s.aobjs, &mut s.fobjs, oa, f);
                        "current frame"
                    }
                    85..=86 => {
                        let f = frame(rand());
                        fns::HSD_AObjSetRewindFrame(ctx, handle(ctx, ta), arg(f));
                        aobj::set_rewind_frame(&mut s.aobjs, oa, f);
                        "rewind frame"
                    }
                    87..=88 => {
                        let f = frame(rand());
                        fns::HSD_AObjSetEndFrame(ctx, handle(ctx, ta), arg(f));
                        aobj::set_end_frame(&mut s.aobjs, oa, f);
                        "end frame"
                    }
                    89..=91 => {
                        // Stops through HSD_ForeachAnim pass a null update function.
                        let null = rand() % 2 == 0;
                        fns::HSD_AObjStopAnim(
                            ctx,
                            handle(ctx, ta),
                            handle(ctx, OBJ),
                            handle(ctx, if null { 0 } else { UPDATE }),
                        );
                        let mut log = |t: u8, v: f32| ours_log.push((t, v.to_bits()));
                        aobj::stop_anim(
                            &mut s.aobjs,
                            &mut s.fobjs,
                            &s.archives,
                            oa,
                            if null { None } else { Some(&mut log) },
                        );
                        "stop"
                    }
                    92 => {
                        let fdesc = if rand() % 4 == 0 {
                            None
                        } else {
                            Some(fdescs[(rand() as usize) % fdescs.len()])
                        };
                        let (tf, of) = s.load_fobjs(fdesc);
                        fns::HSD_AObjSetFObj(ctx, handle(ctx, ta), handle::<HSD_FObj>(ctx, tf));
                        aobj::set_fobj(&mut s.aobjs, &mut s.fobjs, oa, of);
                        "set fobj"
                    }
                    93 => {
                        fns::HSD_AObjRemove(ctx, handle(ctx, ta));
                        aobj::remove(&mut s.aobjs, &mut s.fobjs, oa);
                        (ta, oa) = s.load(desc);
                        let f = frame(rand());
                        fns::HSD_AObjReqAnim(ctx, handle(ctx, ta), arg(f));
                        aobj::req_anim(&mut s.aobjs, &mut s.fobjs, oa, f);
                        "remove and reload"
                    }
                    94 => {
                        let t = fns::HSD_AObjAlloc(ctx).addr();
                        let o = Some(aobj::alloc(&mut s.aobjs));
                        s.compare(t, o, &format!("{what} step {n}: alloc"));
                        extras.push((t, o));
                        "alloc"
                    }
                    95 => {
                        if !extras.is_empty() {
                            let (t, o) = extras.swap_remove((rand() as usize) % extras.len());
                            fns::HSD_AObjFree(ctx, handle(ctx, t));
                            aobj::free(&mut s.aobjs, o);
                        }
                        "free"
                    }
                    96 => {
                        fns::HSD_AObjInvokeCallBacks(ctx);
                        aobj::invoke_end_callbacks(&s.aobjs);
                        "invoke end callbacks"
                    }
                    97 => {
                        fns::HSD_AObjInitEndCallBack(ctx);
                        aobj::init_end_callback(&mut s.aobjs);
                        "init end callbacks"
                    }
                    _ => {
                        // A null AObj does nothing anywhere.
                        let z = || handle::<HSD_AObj>(ctx, 0);
                        assert_eq!(fns::HSD_AObjGetFlags(ctx, z()), 0);
                        fns::HSD_AObjSetFlags(ctx, z(), aobj::LOOP);
                        fns::HSD_AObjClearFlags(ctx, z(), aobj::LOOP);
                        fns::HSD_AObjSetFObj(ctx, z(), handle(ctx, 0));
                        fns::HSD_AObjReqAnim(ctx, z(), 1.0);
                        fns::HSD_AObjStopAnim(ctx, z(), handle(ctx, OBJ), handle(ctx, UPDATE));
                        fns::HSD_AObjInterpretAnim(ctx, z(), handle(ctx, OBJ), handle(ctx, UPDATE));
                        fns::HSD_AObjSetRate(ctx, z(), 1.0);
                        fns::HSD_AObjSetRewindFrame(ctx, z(), 1.0);
                        fns::HSD_AObjSetEndFrame(ctx, z(), 1.0);
                        fns::HSD_AObjSetCurrentFrame(ctx, z(), 1.0);
                        fns::HSD_AObjRemove(ctx, z());
                        fns::HSD_AObjFree(ctx, z());
                        assert!(fns::HSD_AObjLoadDesc(ctx, handle(ctx, 0)).addr() == 0);
                        let (a, f) = (&mut s.aobjs, &mut s.fobjs);
                        let mut log = |t: u8, v: f32| ours_log.push((t, v.to_bits()));
                        assert_eq!(aobj::flags(a, None), 0);
                        aobj::set_flags(a, None, aobj::LOOP);
                        aobj::clear_flags(a, None, aobj::LOOP);
                        aobj::set_fobj(a, f, None, None);
                        aobj::req_anim(a, f, None, 1.0);
                        aobj::stop_anim(a, f, &s.archives, None, Some(&mut log));
                        aobj::interpret(a, f, &s.archives, None, Some(&mut log));
                        aobj::set_rate(a, None, 1.0);
                        aobj::set_rewind_frame(a, None, 1.0);
                        aobj::set_end_frame(a, None, 1.0);
                        aobj::set_current_frame(a, f, None, 1.0);
                        aobj::remove(a, f, None);
                        aobj::free(a, None);
                        assert!(aobj::load_desc(a, f, &s.archives, None).is_none());
                        "null"
                    }
                };
                let theirs_log = take_updates();
                assert_eq!(
                    ours_log, theirs_log,
                    "{what} step {n} ({op}): update values"
                );
                counts.updates += ours_log.len() as u64;
                *counts.ops.entry(op).or_default() += 1;
                s.compare(ta, oa, &format!("{what} step {n} ({op})"));
                counts.steps += 1;
                let mut f = oa.and_then(|a| s.aobjs[a].fobj);
                while let Some(id) = f {
                    if s.fobjs[id.0].op_intrp == 6 {
                        counts.keyed += 1;
                        break;
                    }
                    f = s.fobjs[id.0].next;
                }
            }
            fns::HSD_AObjRemove(ctx, handle(ctx, ta));
            aobj::remove(&mut s.aobjs, &mut s.fobjs, oa);
            // Freed and reloaded, both sides hand out the same objects.
            let (ta, oa) = s.load(desc);
            s.compare(ta, oa, &format!("{what}: reload after remove"));
            counts.descs += 1;
        }
    }
    set_fp_mode(FpMode::Hardware);
    let mut ops: Vec<_> = counts.ops.iter().collect();
    ops.sort();
    eprintln!(
        "{} descs, {} steps, {} update values compared ({} binding descs skipped); {} loops, {} holds, {} ends, {} keyed; ops: {ops:?}",
        counts.descs,
        counts.steps,
        counts.updates,
        counts.bound_skipped,
        counts.loops,
        counts.holds,
        counts.ends,
        counts.keyed
    );
    assert_eq!(counts.bound_skipped, 0, "descs binding objects by ID");
    assert!(counts.descs >= 1000, "only {} descs", counts.descs);
}
