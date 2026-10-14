//! The capture harness end to end: a real call runs in the reference from its captured state,
//! and melee-sim's state imported from the same capture is compared with memory afterwards.

use leaf_check::{reference, run_capture};
use melee_capture::compare::Comparer;
use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, find_archives};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_local_data::ssbm_rs_local;

const CALLS: &str = "calls/fid";

/// With no port run on our side, the differences are exactly what the call changed: they must
/// be there (the comparison sees changes) and our unchanged state must round-trip first.
#[test]
fn captured_call_changes_are_seen() {
    let Some(ctx) = reference() else { return };
    let path = std::path::Path::new(&ssbm_rs_local(CALLS)).join("Fighter_procAnim-call1.call");
    if !path.exists() {
        return;
    }
    let cap = Capture::read(&path);
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives(&cap.call.mem1, &disc, &mut archives),
    };
    let mut imp = Importer::new(&cap, &data).with_archives(&archives);
    imp.world();
    let Importer { game, maps, .. } = imp;
    let after = run_capture(&ctx, &cap.call);
    let mut cmp = Comparer::new(&after, &data, &game, maps);
    cmp.world();
    eprintln!("Fighter_procAnim changed {} fields:", cmp.diffs.len());
    for d in cmp.diffs.iter().take(15) {
        eprintln!("  {d}");
    }
    assert!(!cmp.diffs.is_empty(), "the call changed nothing we compare");
}
