//! melee-sim's stage collision library (`mp`) against ssbm-rs's `mplib.c` and `mpisland.c`
//! ports, on every stage on the disc. Each stage's `coll_data` is loaded into both, in both
//! float modes, and both are driven through the same random queries (points, segments and
//! boxes around and across the geometry, every line id) and the same random mutations
//! (joints hidden, moved, enabled and disabled, lines toggled, vertices moved), comparing every
//! result and every output written through a pointer bit for bit, and the full state (every
//! vertex, line, joint, island list and global) after each mutation.

#[path = "../support/mpstage.rs"]
mod mpstage;
mod mutations;
mod queries;
mod state;

use std::collections::HashMap;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{hsd_reference, sym};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::mp::Geometry;
#[allow(unused_imports)]
pub use mpstage::trace;
use mpstage::{FILE_MAX, Rng, Stage, T, load_theirs, release, reset_boxes, stage_files};
use ssbm_types::fns;

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
