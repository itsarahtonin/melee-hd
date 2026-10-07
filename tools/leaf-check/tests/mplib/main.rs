//! melee-sim's stage collision library (`mp`) against ssbm-rs's `mplib.c` and `mpisland.c`
//! ports, on every stage on the disc. Each stage's `coll_data` is loaded into both, in both
//! float modes, and both are driven through the same random queries (points, segments and
//! boxes around and across the geometry, every line id) and the same random mutations
//! (joints hidden, moved, enabled and disabled, lines toggled, vertices moved), comparing every
//! result and every output written through a pointer bit for bit, and the full state (every
//! vertex, line, joint, island list and global) after each mutation.

mod mutations;
mod queries;
mod state;

use std::collections::HashMap;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{handle, hsd_reference, sym};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::mp::{Geometry, Kinds};
use ssbm_rt::{Addr, Ctx};
use ssbm_types::fns;
use ssbm_types::records::{GroundParam, HSD_Archive, MapCollData};

/// Where the stage archive's header and file go in ssbm-rs's memory, clear of the HSD heap.
const ARCHIVE: u32 = 0x8070_0000;
const FILE: u32 = 0x8080_0000;
const FILE_MAX: usize = 0x0070_0000;

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
pub struct Rng(u64);

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
fn stage_files(ctx: &Ctx) -> Vec<(i32, String)> {
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
fn load_theirs(ctx: &Ctx, bytes: &[u8], archive: &Archive, grkind: i32, with_data: bool) -> u32 {
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
fn release(s: &Stage) {
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
fn reset_boxes(s: &mut Stage) {
    let boxes = sym("mpLib_80458868");
    for b in 0..2 {
        for (k, v) in [10000.0f32, -10000.0, -10000.0, 10000.0].iter().enumerate() {
            s.ctx
                .write_u32(boxes + 0x10 * b + 4 * k as u32, v.to_bits());
        }
    }
    s.g.reset_bounds();
    state::check(s, "after resetting the boxes");
}

#[test]
fn stages_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    queries::register_natives(ctx);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let files = disc.files().unwrap();
    let mut totals = queries::Counts::default();
    let mut stages = 0;
    for (grkind, file) in stage_files(ctx) {
        let mut file = file.trim_start_matches('/').to_owned();
        if !file.contains('.') {
            file.push_str(".dat");
        }
        let Some(path) = files
            .iter()
            .find(|f| f.trim_start_matches('/').eq_ignore_ascii_case(&file))
        else {
            eprintln!("{file}: not on the disc");
            continue;
        };
        if let Ok(only) = std::env::var("MPLIB_ONLY")
            && !file.contains(&only)
        {
            continue;
        }
        let bytes = disc.read_file(path).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        let Some(coll_off) = archive.public("coll_data") else {
            eprintln!("{file}: no coll_data");
            continue;
        };
        assert!(
            bytes.len() <= FILE_MAX,
            "{file} is too big for the test's buffer"
        );
        let scale = archive
            .public("grGroundParam")
            .map_or(1.0, |o| archive.f32(o));
        let mut archives = Archives::default();
        let id: ArchiveId = archives.add(archive.clone());
        for mode in [FpMode::Hardware, FpMode::Slippi] {
            set_fp_mode(mode);
            // Each stage's own seed, so one stage's run reproduces alone (`MPLIB_ONLY`).
            let seed = file
                .bytes()
                .fold(0x9E37_79B9_7F4A_7C15u64 ^ mode as u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
                });
            let gen0 = fns::mpColl_804D64AC(ctx).get();
            let coll = load_theirs(ctx, &bytes, &archive, grkind, true);
            let g = Geometry::<T>::load(
                &archives,
                Some(DataRef {
                    archive: id,
                    offset: coll_off,
                }),
                scale,
                grkind,
            );
            let mut stage = Stage {
                ctx,
                name: format!("{file} (kind {grkind})"),
                grkind,
                g,
                coll,
                gen0,
                islands: HashMap::new(),
                islands_back: HashMap::new(),
                rng: Rng(seed | 1),
                mode,
            };
            eprintln!("{}", stage.what());
            state::check_load(&stage, &archive, coll_off);
            state::check(&mut stage, "after loading");
            reset_boxes(&mut stage);
            if mode == FpMode::Hardware {
                queries::terrain_queries(&mut stage, &mut totals);
            }
            queries::run(&mut stage, &mut totals, 400);
            mutations::run(&mut stage, &mut totals, 300);
            release(&stage);
        }
        stages += 1;
    }
    // A stage without collision data gets the default floor. Its line record is static data
    // the original writes into (terrain changes), so a second load would start from the first
    // one's changes; melee-hd starts from the data each time. Every stage the game loads has
    // collision data, so the test restores the record before each load.
    let default_at = sym("mpLib_803BF718");
    let default_bytes: Vec<u8> = (0..0x78).map(|i| ctx.read_u8(default_at + i)).collect();
    for mode in [FpMode::Hardware, FpMode::Slippi] {
        set_fp_mode(mode);
        ctx.write_bytes(default_at, &default_bytes);
        let (file, bytes) = {
            let path = files
                .iter()
                .find(|f| f.trim_start_matches('/').eq_ignore_ascii_case("GrNBa.dat"))
                .unwrap();
            (path.clone(), disc.read_file(path).unwrap())
        };
        let archive = Archive::parse(&bytes).unwrap();
        let gen0 = fns::mpColl_804D64AC(ctx).get();
        load_theirs(ctx, &bytes, &archive, 36, false);
        let scale = archive
            .public("grGroundParam")
            .map_or(1.0, |o| archive.f32(o));
        let g = Geometry::<T>::load(&Archives::default(), None, scale, 36);
        let mut stage = Stage {
            ctx,
            name: format!("default floor ({file})"),
            grkind: 36,
            g,
            coll: sym("mpLib_803BF760"),
            gen0,
            islands: HashMap::new(),
            islands_back: HashMap::new(),
            rng: Rng(0x1234_5678_9ABC_DEF1),
            mode,
        };
        state::check(&mut stage, "after loading");
        reset_boxes(&mut stage);
        queries::run(&mut stage, &mut totals, 100);
        mutations::run(&mut stage, &mut totals, 50);
        release(&stage);
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!("{stages} stages; {totals}");
    assert!(stages >= 25, "only {stages} stages have collision data");
}
