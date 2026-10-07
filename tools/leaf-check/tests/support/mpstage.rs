//! A stage's collision geometry loaded on both sides, shared by the stage collision tests
//! (`mplib`, `mpcoll`). Each binary has a `state` module (`mplib/state.rs`) at its root.

use std::collections::HashMap;

use gekko_fp::FpMode;
use leaf_check::{handle, sym};
use melee_data::archive::Archive;
use melee_sim::mp::{Geometry, Kinds};
use ssbm_rt::{Addr, Ctx};
use ssbm_types::fns;
use ssbm_types::records::{GroundParam, HSD_Archive, MapCollData};

/// Where the stage archive's header and file go in ssbm-rs's memory, clear of the HSD heap.
pub const ARCHIVE: u32 = 0x8070_0000;
pub const FILE: u32 = 0x8080_0000;
pub const FILE_MAX: usize = 0x0070_0000;

/// The test's handles are ssbm-rs addresses, so both sides' joints and callbacks compare
/// directly.
#[derive(Clone, Debug)]
pub struct T;

impl Kinds for T {
    type JObj = u32;
    type Callback = u32;
    type UserData = u32;
}

/// Prints what is about to run when `MPLIB_TRACE` is set, to find a hang.
pub fn trace(what: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("MPLIB_TRACE").is_some()) {
        eprintln!("  {what}");
    }
}

/// A deterministic random source, so failures reproduce.
pub struct Rng(pub u64);

impl Rng {
    pub fn bits(&mut self) -> u64 {
        let s = &mut self.0;
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.bits() % n.max(1)
    }

    /// A float in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.bits() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * unit
    }

    pub fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

/// One stage loaded on both sides.
pub struct Stage<'a> {
    pub ctx: &'a Ctx,
    pub name: String,
    pub grkind: i32,
    pub g: Geometry<T>,
    /// `coll_data` in ssbm-rs's memory.
    pub coll: u32,
    /// `mpColl_804D64AC` when the stage loaded: ours counts from zero.
    pub gen0: i32,
    /// ssbm-rs's island addresses and ours, paired as they're first seen.
    pub islands: HashMap<u32, u32>,
    pub islands_back: HashMap<u32, u32>,
    pub rng: Rng,
    pub mode: FpMode,
}

impl Stage<'_> {
    pub fn what(&self) -> String {
        format!("{} ({:?})", self.name, self.mode)
    }
}

/// The stage files, by stage kind, as the game's stage table names them.
pub fn stage_files(ctx: &Ctx) -> Vec<(i32, String)> {
    let table = sym("stage_datas");
    let mut out = Vec::new();
    for kind in 0..0x47 {
        let data = ctx.read_u32(table + 4 * kind as u32);
        if data == 0 {
            continue;
        }
        let name_at = ctx.read_u32(data + 8);
        if name_at == 0 {
            continue;
        }
        let mut name = String::new();
        let mut at = name_at;
        loop {
            let c = ctx.read_u8(at);
            if c == 0 {
                break;
            }
            name.push(c as char);
            at += 1;
        }
        out.push((kind, name));
    }
    out
}

/// Parses a stage file into ssbm-rs's memory and loads its collision as the game does
/// (`Ground_801C0800`): stage kind and ground parameters set, then `mpLibLoad`. Returns
/// `coll_data`'s address, or 0 for a stage loaded without collision data.
pub fn load_theirs(
    ctx: &Ctx,
    bytes: &[u8],
    archive: &Archive,
    grkind: i32,
    with_data: bool,
) -> u32 {
    ctx.write_bytes(FILE, bytes);
    assert_eq!(
        fns::HSD_ArchiveParse(
            ctx,
            handle::<HSD_Archive>(ctx, ARCHIVE),
            handle(ctx, FILE),
            bytes.len() as u32
        ),
        0
    );
    let data = ctx.read_u32(ARCHIVE + 0x20);
    // `mpLibLoad` leaves the floor box (`mpLib_80458868[1]`) as the last stage had it; the
    // game resets both boxes right after (`mpLib_80058820`). A new geometry starts it at zero.
    let boxes = sym("mpLib_80458868");
    for k in 4..8 {
        ctx.write_u32(boxes + 4 * k, 0);
    }
    let info = fns::stage_info(ctx);
    info.set_grkind(grkind);
    let param = archive.public("grGroundParam").map_or(0, |o| data + o);
    info.set_param(handle::<GroundParam>(ctx, param));
    let coll = if with_data {
        data + archive.public("coll_data").unwrap()
    } else {
        0
    };
    fns::mpLibLoad(ctx, handle::<MapCollData>(ctx, coll));
    coll
}

/// Frees what the stage allocated on ssbm-rs's heap (the three arrays and every island), so
/// the heap lasts the whole disc.
pub fn release(s: &Stage) {
    let ctx = s.ctx;
    for name in ["groundCollVtx", "groundCollLine", "groundCollJoint"] {
        fns::HSD_Free(ctx, handle::<Addr>(ctx, ctx.read_u32(sym(name))));
    }
    for &island in s.islands.keys() {
        fns::HSD_Free(ctx, handle::<Addr>(ctx, island));
    }
}

/// What `mpLib_80058820` does to the boxes after the stage loads (it also creates the GObj
/// whose process measures the floors each tick, which needs the scheduler).
pub fn reset_boxes(s: &mut Stage) {
    let boxes = sym("mpLib_80458868");
    for b in 0..2 {
        for (k, v) in [10000.0f32, -10000.0, -10000.0, 10000.0].iter().enumerate() {
            s.ctx
                .write_u32(boxes + 0x10 * b + 4 * k as u32, v.to_bits());
        }
    }
    s.g.reset_bounds();
    crate::state::check(s, "after resetting the boxes");
}
