//! melee-sim's ID table against ssbm-rs's port of HSD's, on random inserts, replacements,
//! removals and lookups that collide in the original's 101 buckets.

use leaf_check::{SCRATCH, handle, hsd_reference};
use melee_sim::hsd::id::IdTable;
use ssbm_rt::Handle;
use ssbm_types::fns;

#[test]
fn id_table_matches() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    fns::HSD_IDSetup(ctx);
    let mut ours: IdTable<u32, u32> = IdTable::default();
    let success = SCRATCH;
    let mut s = 0x1234_5678_9ABC_DEF1u64;
    let mut rand = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    // Keys like descriptor addresses, many sharing a bucket (multiples of 101 apart).
    let keys: Vec<u32> = (0..200)
        .map(|i| 0x8040_0000 + (i % 40) * 101 * 4 + (i / 40) * 8)
        .collect();
    for step in 0..50_000 {
        let key = keys[(rand() % keys.len() as u64) as usize];
        match rand() % 4 {
            0 | 1 => {
                let data = 0x8100_0000 + (rand() as u32 & 0xFFFF) * 4;
                fns::HSD_IDInsertToTable(ctx, handle(ctx, 0), key, handle(ctx, data));
                ours.insert(key, data);
            }
            2 => {
                fns::HSD_IDRemoveByIDFromTable(ctx, handle(ctx, 0), key);
                ours.remove(key);
            }
            _ => {
                ctx.write_u32(success, 0xFFFF_FFFF);
                let got =
                    fns::HSD_IDGetDataFromTable(ctx, handle(ctx, 0), key, handle(ctx, success))
                        .addr();
                let found = ctx.read_u32(success);
                let theirs = (found == 1).then_some(got);
                assert!(found <= 1, "step {step}: success flag {found}");
                assert_eq!(ours.get(key), theirs, "step {step}: lookup {key:#x}");
                if theirs.is_none() {
                    assert_eq!(got, 0, "step {step}: a miss returns null");
                }
            }
        }
    }
}
