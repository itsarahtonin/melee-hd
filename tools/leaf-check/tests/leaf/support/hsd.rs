//! HSD objects on both sides in one raw form, for the camera, light and fog tests: identities,
//! and AObjs with their FObj chains and bound joints, and WObjs with their RObjs, compared
//! field by field.

#![allow(dead_code)]

use std::collections::HashMap;

use leaf_check::sym;
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_sim::hsd::Objects;
use melee_sim::hsd::aobj::AObjId;
use melee_sim::hsd::fobj::FObjId;
use melee_sim::hsd::robj::Ref;
use melee_sim::hsd::wobj::WObjId;
use ssbm_rt::Ctx;

/// A one-to-one map between ssbm-rs's object addresses and our slots.
///
/// Objects from `HSD_ObjAlloc` pools (AObjs, FObjs) keep one map for the whole test: both
/// sides hand out and reuse them in the same order. Class objects (WObjs, CObjs, LObjs, fogs)
/// come from `hsdAllocMemPiece`, which shares freed memory among all classes of a size and
/// splits larger pieces, so an address freed by one class can come back as another's; melee-sim
/// keeps a typed pool per class (`docs/engine.md`). Their maps are built afresh for each
/// comparison, checking that references between live objects agree.
#[derive(Default)]
pub struct Identity {
    to_ours: HashMap<u32, u32>,
    to_theirs: HashMap<u32, u32>,
}

impl Identity {
    pub fn check(&mut self, theirs: u32, ours: Option<u32>, what: &str) {
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

/// One archive loaded on both sides.
pub struct Data {
    pub archives: Archives,
    pub id: ArchiveId,
    /// Where the reference's copy of the data block is.
    pub addr: u32,
}

impl Data {
    pub fn load(ctx: &Ctx, bytes: &[u8]) -> Data {
        let addr = leaf_check::load_archive(ctx, bytes);
        let mut archives = Archives::default();
        let id = archives.add(Archive::parse(bytes).expect("archive"));
        Data { archives, id, addr }
    }

    pub fn at(&self, offset: u32) -> DataRef {
        DataRef {
            archive: self.id,
            offset,
        }
    }

    pub fn theirs(&self, offset: u32) -> u32 {
        self.addr + offset
    }

    /// A data reference as the reference holds it: the address, or 0.
    pub fn theirs_of(&self, r: Option<DataRef>) -> u32 {
        r.map_or(0, |r| self.addr + r.offset)
    }
}

/// Our object state, and the persistent identities of its AObjs and FObjs.
#[derive(Default)]
pub struct Hsd {
    pub objs: Objects,
    pub aobj_ids: Identity,
    pub fobj_ids: Identity,
}

pub fn words(ctx: &Ctx, addr: u32, n: u32) -> Vec<u32> {
    (0..n).map(|i| ctx.read_u32(addr + 4 * i)).collect()
}

pub fn f32_bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

impl Hsd {
    /// The AObj end-callback counts, and how many joints, RObjs, AObjs and FObjs each side
    /// holds, so every allocation and release agrees.
    pub fn compare_counts(&self, ctx: &Ctx, what: &str) {
        let o = &self.objs;
        assert_eq!(
            (
                ctx.read_u32(sym("HSD_AObj_804D762C")) as i32,
                ctx.read_u32(sym("HSD_AObj_804D7630")) as i32
            ),
            (o.aobjs.stopped, o.aobjs.playing),
            "{what}: end-callback counts"
        );
        let used = |data: &str| ctx.read_u32(sym(data) + 8);
        assert_eq!(
            [
                live(ctx, sym("hsdJObj")),
                used("robj_alloc_data"),
                used("aobj_alloc_data"),
                used("fobj_alloc_data"),
            ],
            [
                o.jobjs.pool.used(),
                o.robjs.pool.used(),
                o.aobjs.pool.used(),
                o.fobjs.used(),
            ],
            "{what}: live joints, RObjs, AObjs and FObjs"
        );
    }

    /// An AObj, its FObj chain, and the joint it binds (by the descriptor it was loaded from,
    /// with its flags, translation and matrix).
    pub fn compare_aobj(
        &mut self,
        ctx: &Ctx,
        data: &Data,
        theirs: u32,
        ours: Option<AObjId>,
        what: &str,
    ) {
        self.aobj_ids.check(theirs, ours.map(|a| a.0), what);
        let Some(a) = ours else { return };
        let o = &self.objs.aobjs[a];
        assert_eq!(
            words(ctx, theirs, 5),
            [
                o.flags,
                o.curr_frame.to_bits(),
                o.rewind_frame.to_bits(),
                o.end_frame.to_bits(),
                o.framerate.to_bits(),
            ],
            "{what}: AObj flags, curr, rewind, end, rate"
        );
        let tj = ctx.read_u32(theirs + 0x18);
        match o.hsd_obj {
            None => assert_eq!(tj, 0, "{what}: theirs binds a joint, ours none"),
            Some(j) => {
                assert_ne!(tj, 0, "{what}: ours binds a joint, theirs none");
                let jo = &self.objs.jobjs[j];
                let mut ours = vec![
                    jo.id.map_or(0, |d| data.addr + d.offset),
                    u32::from(jo.ref_count),
                    jo.flags,
                ];
                ours.extend(f32_bits(&[jo.translate.x, jo.translate.y, jo.translate.z]));
                ours.extend(jo.mtx.m.as_flattened().iter().map(|v| v.to_bits()));
                let mut theirs_j = vec![
                    ctx.read_u32(tj + 0x84),
                    u32::from(ctx.read_u16(tj + 4)),
                    ctx.read_u32(tj + 0x14),
                ];
                theirs_j.extend(words(ctx, tj + 0x38, 3));
                theirs_j.extend(words(ctx, tj + 0x44, 12));
                assert_eq!(
                    theirs_j, ours,
                    "{what}: bound joint's descriptor, ref count, flags, translation and matrix"
                );
            }
        }
        let (mut tf, mut of) = (ctx.read_u32(theirs + 0x14), o.fobj);
        loop {
            self.fobj_ids.check(tf, of.map(|f| f.0), what);
            let Some(f) = of else { break };
            assert_eq!(
                self.fobj_ours(f),
                fobj_theirs(ctx, data, tf),
                "{what}: FObj"
            );
            tf = ctx.read_u32(tf);
            of = self.objs.fobjs[f.0].next;
        }
    }

    fn fobj_ours(&self, id: FObjId) -> String {
        let o = &self.objs.fobjs[id.0];
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

    /// A WObj, its animation and its RObjs (flags, limits and their animations). `ids` is the
    /// comparison's WObj identity.
    pub fn compare_wobj(
        &mut self,
        ctx: &Ctx,
        data: &Data,
        ids: &mut Identity,
        theirs: u32,
        ours: Option<WObjId>,
        what: &str,
    ) {
        ids.check(theirs, ours.map(|w| w.0), what);
        let Some(w) = ours else { return };
        let o = self.objs.wobjs[w.0].clone();
        assert_eq!(
            (ctx.read_u16(theirs + 4), words(ctx, theirs + 8, 4)),
            (
                o.ref_count.0,
                vec![
                    o.flags,
                    o.pos.x.to_bits(),
                    o.pos.y.to_bits(),
                    o.pos.z.to_bits()
                ],
            ),
            "{what}: WObj ref_count, flags, pos"
        );
        self.compare_aobj(
            ctx,
            data,
            ctx.read_u32(theirs + 0x18),
            o.aobj,
            &format!("{what}: WObj aobj"),
        );
        let (mut tr, mut or) = (ctx.read_u32(theirs + 0x1C), o.robj);
        let mut i = 0;
        loop {
            let Some(r) = or else {
                assert_eq!(tr, 0, "{what}: RObj lists' lengths");
                break;
            };
            assert_ne!(tr, 0, "{what}: RObj lists' lengths");
            let ro = self.objs.robjs[r].clone();
            let limit = match ro.u {
                Ref::Limit(l) => l.to_bits(),
                _ => 0,
            };
            assert_eq!(
                (ctx.read_u32(tr + 4), ctx.read_u32(tr + 8)),
                (ro.flags, limit),
                "{what}: RObj {i} flags and limit"
            );
            self.compare_aobj(
                ctx,
                data,
                ctx.read_u32(tr + 0x18),
                ro.aobj,
                &format!("{what}: RObj {i} aobj"),
            );
            tr = ctx.read_u32(tr);
            or = ro.next;
            i += 1;
        }
    }
}

fn fobj_theirs(ctx: &Ctx, data: &Data, a: u32) -> String {
    let f = |o| ctx.read_u32(a + o);
    // A loaded FObj's cursor is null until its first request sets it; ours holds offset 0.
    let ad = match f(4) {
        0 => 0,
        p => p.wrapping_sub(data.addr),
    };
    format!(
        "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
        ad,
        f(8).wrapping_sub(data.addr),
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

/// Whether an AObjDesc (or any in a chain of WObjAnim-like structs) binds a joint by ID, which
/// melee-sim's AObj can't load yet.
pub fn binds_joint(a: &Archive, aobjdesc: Option<u32>) -> bool {
    aobjdesc.is_some_and(|d| a.u32(d + 0xC) != 0)
}

/// Appends an AObjDesc animating each channel type in `channels` (type, low, high) linearly
/// between two random values over 30 frames, its FObjDescs and their key data; returns its
/// offset. `relocs` collects the pointers.
pub fn push_aobjdesc(
    data: &mut Vec<u8>,
    relocs: &mut Vec<u32>,
    rand: &mut leaf_check::Rand,
    channels: &[(u8, f32, f32)],
) -> u32 {
    let mut keys = Vec::new();
    for &(_, lo, hi) in channels {
        let at = data.len() as u32;
        // HSD_A_OP_LIN with two keys: each a float (little-endian, as parseFloat reads one
        // with no fraction format) and a wait.
        data.push(0x12);
        data.extend_from_slice(&rand.range(lo, hi).to_le_bytes());
        data.push(10);
        data.extend_from_slice(&rand.range(lo, hi).to_le_bytes());
        data.push(20);
        let len = data.len() as u32 - at;
        while !data.len().is_multiple_of(4) {
            data.push(0);
        }
        keys.push((at, len));
    }
    let first = data.len() as u32;
    for (i, (&(ty, _, _), &(at, len))) in channels.iter().zip(&keys).enumerate() {
        let next = first + 0x14 * (i as u32 + 1);
        if i + 1 < channels.len() {
            relocs.push(data.len() as u32);
            data.extend_from_slice(&next.to_be_bytes());
        } else {
            data.extend_from_slice(&0u32.to_be_bytes());
        }
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(&0.0f32.to_be_bytes());
        data.extend_from_slice(&[ty, 0, 0, 0]);
        relocs.push(data.len() as u32);
        data.extend_from_slice(&at.to_be_bytes());
    }
    let desc = data.len() as u32;
    let flags: u32 = if rand.below(2) == 0 { 1 << 29 } else { 0 };
    data.extend_from_slice(&flags.to_be_bytes());
    data.extend_from_slice(&40.0f32.to_be_bytes());
    if channels.is_empty() {
        data.extend_from_slice(&0u32.to_be_bytes());
    } else {
        relocs.push(data.len() as u32);
        data.extend_from_slice(&first.to_be_bytes());
    }
    data.extend_from_slice(&0u32.to_be_bytes());
    desc
}

/// Appends a pointer word to `target`, or a null.
pub fn push_ptr(data: &mut Vec<u8>, relocs: &mut Vec<u32>, target: Option<u32>) {
    if let Some(t) = target {
        relocs.push(data.len() as u32);
        data.extend_from_slice(&t.to_be_bytes());
    } else {
        data.extend_from_slice(&0u32.to_be_bytes());
    }
}

/// How many objects of a class the reference holds (`nb_exist` in its class info), to compare
/// with a pool's count: every allocation and release must agree, not only the live objects'
/// contents.
pub fn live(ctx: &Ctx, info: u32) -> u32 {
    ctx.read_u32(info + 0x20)
}

/// Color-channel values where rounding the product `255 * value` to single precision or not
/// decides the integer it truncates to: within [0, 1] none does, so these are the values just
/// past 1 (and -1) where it does, or values next to `k / 255`.
pub fn channel_edge(rand: &mut leaf_check::Rand) -> f32 {
    if rand.below(2) == 0 {
        let v = f32::from_bits(0x3F80_0000 + 0x1_0101 * (1 + rand.below(25) as u32));
        return if rand.below(4) == 0 { -v } else { v };
    }
    let k = rand.below(256) as f32 / 255.0;
    let d = rand.below(5) as i32 - 2;
    f32::from_bits((k.to_bits() as i32 + d).max(0) as u32)
}
