//! What compiling a material's TEV expressions allocates (`HSD_MObjCompileTev`), for the
//! allocator model (`hsd::heap`): the original allocates each expression a material makes as
//! a memory piece, in the order it makes them, then a piece for each stage the compile
//! schedules; it frees the operations, newest first, and the constants no stage reads, newest
//! first, and keeps the stages and the other constants with the material until it goes.

use super::Objects;
use super::mobj::MObjId;
use super::texp::{Exp, Node, TExpList, dag::Dag, make};
use super::tobj::TObjId;

/// `sizeof(HSD_TETev)`, `sizeof(HSD_TECnst)` and `sizeof(HSD_TExpTevDesc)`.
pub const TEV_SIZE: u32 = 0x6C;
pub const CNST_SIZE: u32 = 0x18;
pub const TEVDESC_SIZE: u32 = 0x78;

/// Room a compile keeps from one material to the next: no state, so a restore keeps the
/// target's and a snapshot starts without.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    pub list: TExpList,
    pub dag: Vec<Dag>,
    pub addrs: Vec<u32>,
}

/// The pieces compiling the material's TEV allocates and frees, with its textures from
/// `first` (as `HSD_MObjCompileTev` chains them): first the pieces its last compile kept go
/// back (`HSD_TExpFreeTevDesc`, `HSD_TExpFreeList`), then the compile's, and what it keeps is
/// recorded in the material's [`super::mobj::MObjHeap`].
pub fn compile(objs: &mut Objects, m: MObjId, first: Option<TObjId>) {
    use super::heap;
    free_kept(objs, m);
    let mut chain = [TObjId(0); 32];
    let mut n = 0;
    let mut cur = first;
    while let Some(t) = cur {
        chain[n] = t;
        n += 1;
        cur = objs.tobjs[t].next;
    }
    let mut s = std::mem::take(&mut objs.mobjs.texp_scratch);
    s.list.clear();
    let root: Exp = make::mobj_make_texp(objs, m, &chain[..n], &mut s.list);
    s.addrs.clear();
    for node in &s.list.nodes {
        let size = match node {
            Node::Tev(_) => TEV_SIZE,
            Node::Cnst(_) => CNST_SIZE,
        };
        s.addrs.push(heap::piece_alloc(size));
    }
    let (num, _) = s.list.schedule_stages(root, &mut s.dag);
    let h = &mut objs.mobjs[m].heap;
    assert!(num <= h.tevdesc.len(), "{num} TEV stages");
    // Each stage goes on the front of the list.
    for i in 0..num {
        h.tevdesc[num - 1 - i] = heap::piece_alloc(TEVDESC_SIZE);
    }
    h.n_tevdesc = num as u8;
    // `HSD_TExpFreeList(list, HSD_TE_TEV, 1)`, then `(list, HSD_TE_CNST, 0)`: the list runs
    // newest first.
    for (node, &addr) in s.list.nodes.iter().zip(&s.addrs).rev() {
        if let Node::Tev(_) = node {
            heap::piece_free(addr, TEV_SIZE);
        }
    }
    h.n_cnsts = 0;
    for (node, &addr) in s.list.nodes.iter().zip(&s.addrs).rev() {
        if let Node::Cnst(c) = node {
            if c.ref_ == 0 {
                heap::piece_free(addr, CNST_SIZE);
            } else {
                h.cnsts[usize::from(h.n_cnsts)] = addr;
                h.n_cnsts += 1;
            }
        }
    }
    objs.mobjs.texp_scratch = s;
}

/// The pieces the material's last compile kept, back to the model.
pub fn free_kept(objs: &mut Objects, m: MObjId) {
    let h = &mut objs.mobjs[m].heap;
    for &p in &h.tevdesc[..usize::from(h.n_tevdesc)] {
        super::heap::piece_free(p, TEVDESC_SIZE);
    }
    for &p in &h.cnsts[..usize::from(h.n_cnsts)] {
        super::heap::piece_free(p, CNST_SIZE);
    }
    h.n_tevdesc = 0;
    h.n_cnsts = 0;
}
