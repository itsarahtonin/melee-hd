//! melee-sim's singly linked lists against ssbm-rs's port of HSD's, on random sequences of
//! appends, prepends and removals over several lists: same contents, same node reuse.

use std::collections::HashMap;

use leaf_check::{handle, hsd_reference};
use melee_sim::hsd::list::{self, SListId, SLists};
use ssbm_rt::Handle;
use ssbm_types::fns;
use ssbm_types::records::_HSD_SList;

#[test]
fn lists_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let mut nodes: SLists<u32> = SLists::default();
    let (mut theirs, mut ours) = ([0u32; 4], [None::<SListId>; 4]);
    let mut slot_of: HashMap<u32, u32> = HashMap::new();
    let mut s = 0x0123_4567_89AB_CDEFu64;
    let mut rand = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for step in 0..20_000 {
        let i = (rand() % 4) as usize;
        let data = 0x8000_0000 | (rand() as u32 & 0xFFFF);
        let node = |a: u32| handle::<_HSD_SList>(ctx, a);
        match rand() % 5 {
            0 | 1 => {
                theirs[i] =
                    fns::HSD_SListAllocAndAppend(ctx, node(theirs[i]), handle(ctx, data)).addr();
                ours[i] = Some(list::alloc_and_append(&mut nodes, ours[i], data));
            }
            2 | 3 => {
                theirs[i] =
                    fns::HSD_SListAllocAndPrepend(ctx, node(theirs[i]), handle(ctx, data)).addr();
                ours[i] = Some(list::alloc_and_prepend(&mut nodes, ours[i], data));
            }
            _ => {
                theirs[i] = fns::HSD_SListRemove(ctx, node(theirs[i])).addr();
                ours[i] = list::remove(&mut nodes, ours[i]);
            }
        }
        for k in 0..4 {
            let (mut t, mut o) = (theirs[k], ours[k]);
            while let Some(n) = o {
                assert_ne!(t, 0, "step {step}: list {k} shorter in ssbm-rs");
                assert_eq!(
                    nodes[n.0].data,
                    ctx.read_u32(t + 4),
                    "step {step}: list {k} data"
                );
                let slot = *slot_of.entry(t).or_insert(n.0);
                assert_eq!(
                    slot, n.0,
                    "step {step}: node at {t:#x} is slot {} here",
                    n.0
                );
                t = ctx.read_u32(t);
                o = nodes[n.0].next;
            }
            assert_eq!(t, 0, "step {step}: list {k} longer in ssbm-rs");
        }
    }
}
