//! Imports a saved call's game state and summarizes it: `import CALL`.
use melee_capture::compare::Comparer;
use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, find_archives};
use melee_data::archive::Archives;
use melee_data::disc::Disc;

fn main() {
    let path = std::env::args().nth(1).expect("import CALL");
    let cap = Capture::read(path.as_ref());
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut archives = Archives::default();
    let loaded = find_archives(&cap.call.mem1, &disc, &mut archives);
    let data = DataMap { loaded };
    let mut imp = Importer::new(&cap, &data);
    imp.world();
    imp.stage();
    imp.ids();
    let g = &imp.game;
    println!(
        "{} GObjs, {} procs, {} fighters, {} stage objects, {} joints, {} AObjs, {} FObjs, {} DObjs, {} PObjs, {} RObjs",
        g.world.gobjs.used(),
        g.world.procs.used(),
        g.fighters.used(),
        g.grounds.used(),
        g.objs.jobjs.pool.used(),
        g.objs.aobjs.pool.used(),
        g.objs.fobjs.used(),
        g.objs.dobjs.pool.used(),
        g.objs.pobjs.pool.used(),
        g.objs.robjs.pool.used(),
    );
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for u in &imp.unhandled {
        let key = u.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
        *kinds
            .entry(if key.is_empty() { u.clone() } else { key })
            .or_default() += 1;
    }
    for (k, n) in kinds {
        println!("  unhandled x{n}: {k}");
    }
    // Round trip: compared with the memory it came from, nothing differs.
    let Importer { mut game, maps, .. } = imp;
    if std::env::var_os("PERTURB").is_some() {
        let j = melee_sim::hsd::jobj::JObjId(5);
        game.objs.jobjs[j].mtx.m[1][3] += 1.0;
        game.fighters[0].dmg.x1830_percent += 1.0;
    }
    let mut cmp = Comparer::new(&cap.call.mem1, &data, &game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    println!("round trip: {} differences", cmp.diffs.len());
    for d in cmp.diffs.iter().take(20) {
        println!("  {d}");
    }
}
