//! melee-sim's FObj (animation curves) against ssbm-rs's port, on every keyframe chain the
//! disc's `*animjoint` roots reach: the same values handed to the update function and the
//! same FObj state after every frame, at varied start frames and rates, in both float modes.

use std::cell::RefCell;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{
    arg, bits, handle, inputs, logged, reference, same_updates, stage_anim_roots, sym,
};
use melee_data::archive::{Archive, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::fobj::{self, FObjId, FObjs};
use melee_sim::hsd::spline;
use ssbm_rt::{Addr, Ctx, Handle};
use ssbm_types::fns;
use ssbm_types::records::{_HSD_FObjDesc, HSD_Archive, HSD_FObj};

const ARCHIVE: u32 = 0x8070_0000;
const FILE: u32 = 0x8080_0000;
const FILE_MAX: usize = 0x0070_0000;
const OBJ_HEAP: u32 = 0x8100_0000;
const UPDATE: u32 = 0x817E_0400;

thread_local! {
    static UPDATES: RefCell<Vec<(u8, u32)>> = const { RefCell::new(Vec::new()) };
}

fn native_update(ctx: &Ctx) {
    let ty = ctx.regs.r(4) as u8;
    let value = ctx.read_u32(ctx.regs.r(5));
    UPDATES.with(|u| u.borrow_mut().push((ty, value)));
}

/// Every FObjDesc chain reachable from the archive's AnimJoint roots.
fn fobj_chains(a: &Archive) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack: Vec<u32> = a
        .publics()
        .filter(|(n, _)| n.to_ascii_lowercase().contains("animjoint") && !n.contains("matanim"))
        .map(|(_, o)| o)
        .chain(stage_anim_roots(a).0)
        .collect();
    let mut seen = std::collections::HashSet::new();
    while let Some(aj) = stack.pop() {
        if !seen.insert(aj) || aj as usize + 0x14 > a.data().len() {
            continue;
        }
        if let Some(c) = a.ptr(aj) {
            stack.push(c);
        }
        if let Some(n) = a.ptr(aj + 4) {
            stack.push(n);
        }
        if let Some(aobj) = a.ptr(aj + 8)
            && let Some(f) = a.ptr(aobj + 8)
        {
            out.push(f);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// One FObj's fields on ssbm-rs's side, as ours holds them.
fn theirs(ctx: &Ctx, a: u32, data: u32) -> String {
    let f = |o| ctx.read_u32(a + o);
    format!(
        "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
        f(4).wrapping_sub(data),
        f(8).wrapping_sub(data),
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

fn ours(fobjs: &FObjs, id: FObjId) -> String {
    let o = &fobjs[id.0];
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

#[test]
fn hermite_matches() {
    let Some(ctx) = reference() else { return };
    let v = inputs(4.0, 20_000);
    for mode in [FpMode::Hardware, FpMode::Slippi] {
        set_fp_mode(mode);
        for i in 0..v.len() {
            let x: Vec<f64> = (0..6).map(|k| arg(v[(i * 7 + k * 13) % v.len()])).collect();
            let theirs = bits(fns::splGetHelmite(&ctx, x[0], x[1], x[2], x[3], x[4], x[5]));
            let ours = bits(spline::hermite(x[0], x[1], x[2], x[3], x[4], x[5]));
            assert_eq!(ours, theirs, "splGetHelmite{x:?} in {mode:?}");
        }
    }
    set_fp_mode(FpMode::Hardware);
}

#[test]
fn fobjs_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    ctx.register(UPDATE, native_update);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut chains_checked = 0;
    let mut frames_checked = 0u64;
    let mut stale = 0u64;
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
        let chains = fobj_chains(&archive);
        if chains.is_empty() || bytes.len() > FILE_MAX {
            continue;
        }
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
        for (ci, &chain) in chains.iter().enumerate().take(40) {
            let mode = if ci % 2 == 0 {
                FpMode::Hardware
            } else {
                FpMode::Slippi
            };
            set_fp_mode(mode);
            // A fresh pool on both sides for each chain.
            fns::HSD_ObjSetHeap(ctx, 0x0040_0000, handle::<Addr>(ctx, OBJ_HEAP));
            fns::HSD_FObjInitAllocData(ctx);
            let mut fobjs = FObjs::default();
            let theirs_f =
                fns::HSD_FObjLoadDesc(ctx, handle::<_HSD_FObjDesc>(ctx, data_addr + chain)).addr();
            let ours_f = fobj::load_desc(
                &mut fobjs,
                &archives,
                Some(DataRef {
                    archive: id,
                    offset: chain,
                }),
            );
            let startframe = [0.0f32, 0.0, 1.0, 3.5, -2.0][(rand() % 5) as usize];
            fns::HSD_FObjReqAnimAll(ctx, handle::<HSD_FObj>(ctx, theirs_f), arg(startframe));
            fobj::req_anim_all(&mut fobjs, ours_f, startframe);
            let mut ours_log = Vec::new();
            for frame in 0..400 {
                let rate = match rand() % 8 {
                    0 => 0.5,
                    1 => 2.0,
                    2 => 1.25,
                    3 => 0.0,
                    _ => 1.0,
                };
                fns::HSD_FObjInterpretAnimAll(
                    ctx,
                    handle(ctx, theirs_f),
                    handle(ctx, 0),
                    handle(ctx, UPDATE),
                    arg(rate),
                );
                ours_log.clear();
                fobj::interpret_all(
                    &mut fobjs,
                    &archives,
                    ours_f,
                    rate,
                    Some(&mut |t, v| ours_log.push((t, logged(v)))),
                );
                let theirs_log = UPDATES.with(|u| std::mem::take(&mut *u.borrow_mut()));
                assert!(
                    same_updates(&ours_log, &theirs_log),
                    "{name} chain {chain:#x} frame {frame} rate {rate} in {mode:?}: updates: {ours_log:?} vs {theirs_log:?}"
                );
                let (mut ta, mut oa) = (theirs_f, ours_f);
                while let Some(o) = oa {
                    assert_ne!(ta, 0, "{name} chain {chain:#x}: chain lengths");
                    assert_eq!(
                        ours(&fobjs, o),
                        theirs(ctx, ta, data_addr),
                        "{name} chain {chain:#x} frame {frame} rate {rate} in {mode:?}"
                    );
                    ta = ctx.read_u32(ta);
                    oa = fobjs[o.0].next;
                }
                stale += ours_log.iter().filter(|(_, v)| v.is_none()).count() as u64;
                frames_checked += 1;
            }
            let rate = 1.0;
            fns::HSD_FObjStopAnimAll(
                ctx,
                handle(ctx, theirs_f),
                handle(ctx, 0),
                handle(ctx, UPDATE),
                arg(rate),
            );
            ours_log.clear();
            fobj::stop_anim_all(
                &mut fobjs,
                &archives,
                ours_f,
                rate,
                Some(&mut |t, v| ours_log.push((t, logged(v)))),
            );
            let theirs_log = UPDATES.with(|u| std::mem::take(&mut *u.borrow_mut()));
            assert!(
                same_updates(&ours_log, &theirs_log),
                "{name} chain {chain:#x}: stop: {ours_log:?} vs {theirs_log:?}"
            );
            // Freed and reloaded, both sides hand out the same objects in the same order.
            let pairs = |t: u32, o: Option<FObjId>, fobjs: &FObjs| {
                let (mut t, mut o, mut v) = (t, o, Vec::new());
                while let Some(x) = o {
                    v.push((t, x.0));
                    t = ctx.read_u32(t);
                    o = fobjs[x.0].next;
                }
                v
            };
            let before = pairs(theirs_f, ours_f, &fobjs);
            fns::HSD_FObjRemoveAll(ctx, handle::<HSD_FObj>(ctx, theirs_f));
            fobj::remove_all(&mut fobjs, ours_f);
            let t2 =
                fns::HSD_FObjLoadDesc(ctx, handle::<_HSD_FObjDesc>(ctx, data_addr + chain)).addr();
            let o2 = fobj::load_desc(
                &mut fobjs,
                &archives,
                Some(DataRef {
                    archive: id,
                    offset: chain,
                }),
            );
            let mut after = pairs(t2, o2, &fobjs);
            let mut before_sorted = before.clone();
            before_sorted.sort_unstable();
            after.sort_unstable();
            assert_eq!(
                after, before_sorted,
                "{name} chain {chain:#x}: reuse after RemoveAll"
            );
            chains_checked += 1;
        }
    }
    set_fp_mode(FpMode::Hardware);
    let _ = sym;
    eprintln!("{chains_checked} chains, {frames_checked} frames compared, {stale} stale values");
    assert!(
        stale > 0,
        "no stale values: the stage models' single-key channels weren't reached"
    );
    assert!(chains_checked >= 150, "only {chains_checked} chains");
}
