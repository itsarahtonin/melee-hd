//! The game's heaps (`melee/lb/lbheap.c`): HSD's main heap, which each scene makes anew, and
//! the preload cache's heaps around it, which a scene keeps or lets go: `Seq` and `Stay` at
//! the arena's bottom, `AllM` at its top, `AllA` in ARAM, and the ARAM heap over the rest of
//! ARAM ([`super::memory`] keeps their blocks).

use super::memory::LbMemory;
use crate::hsd::heap::Layout;

/// The heaps by `LbHeapKind`.
pub const HSD: usize = 0;
pub const ARAM: usize = 1;
pub const SEQ: usize = 2;
pub const STAY: usize = 3;
pub const ALL_M: usize = 4;
pub const ALL_A: usize = 5;
pub const COUNT: usize = 6;

/// `LbHeapStatus`.
pub const CREATED: i32 = 0;
pub const DESTROYED: i32 = 1;

/// What each heap's kind lays it out as (`type`): an OS heap (HSD's), from the arena's
/// bottom up, from its top down, the ARAM heap, from ARAM's bottom up.
const TYPE_OS: i32 = 0;
const TYPE_LOW: i32 = 1;
const TYPE_HIGH: i32 = 2;
const TYPE_ARAM_HEAP: i32 = 3;
const TYPE_ARAM: i32 = 4;

/// `struct Heap`: a heap's OS heap id or handle, range, kind, whether a scene may let it go,
/// and whether it's made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct LbHeap {
    pub id: i32,
    /// Its `lbMemory` handle, `None` for the original's -1.
    pub handle: Option<u8>,
    pub start: u32,
    pub size: u32,
    pub kind: i32,
    pub transient: i32,
    pub status: i32,
}

impl Default for LbHeap {
    /// `resetHeap`.
    fn default() -> Self {
        LbHeap {
            id: -1,
            handle: None,
            start: 0,
            size: 0,
            kind: 1,
            transient: 1,
            status: DESTROYED,
        }
    }
}

/// `lbHeap_80431FA0`: the arena's and ARAM's ranges the heaps share, and the heaps.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct LbHeaps {
    pub arena_lo: u32,
    pub arena_hi: u32,
    pub aram_lo: u32,
    pub aram_hi: u32,
    pub heaps: [LbHeap; COUNT],
}

/// `lbHeap_803BA380`: each preload heap's kind, the heap it follows, and its size.
const DESCS: [(usize, i32, Option<usize>, u32); 4] = [
    (SEQ, TYPE_LOW, None, 0x800),
    (STAY, TYPE_LOW, Some(SEQ), 0x4F_8800),
    (ALL_M, TYPE_HIGH, None, 0x64_B400),
    (ALL_A, TYPE_ARAM, None, 0x96_C800),
];

impl LbHeaps {
    /// `lbHeap_80015F3C`, at boot: the ranges HSD and `lbMemory` leave (`HSD_GetNextArena`:
    /// the arena past the audio heap), every heap reset, and the preload heaps laid out.
    #[doc(alias = "lbHeap_80015F3C")]
    pub fn init(&mut self, arena: (u32, u32), mem: &LbMemory) {
        (self.arena_lo, self.arena_hi) = arena;
        (self.aram_lo, self.aram_hi) = (mem.aram_lo, mem.aram_hi);
        self.heaps = [LbHeap::default(); COUNT];
        for (idx, kind, prev, size) in DESCS {
            let start = match (kind, prev) {
                (TYPE_LOW, None) => self.arena_lo,
                (TYPE_HIGH, None) => self.arena_hi - size,
                (TYPE_ARAM, None) => self.aram_lo,
                (TYPE_LOW | TYPE_ARAM, Some(p)) => self.heaps[p].start + self.heaps[p].size,
                (TYPE_HIGH, Some(p)) => self.heaps[p].start - size,
                _ => 0,
            };
            let h = &mut self.heaps[idx];
            h.kind = kind;
            h.size = size;
            h.start = start;
        }
    }

    /// `lbHeap_800158D0`.
    #[doc(alias = "lbHeap_800158D0")]
    pub fn set_transient(&mut self, heap: usize, transient: i32) {
        self.heaps[heap].transient = transient;
    }

    /// `lbHeap_800158E8`.
    #[doc(alias = "lbHeap_800158E8")]
    pub fn transient(&self, heap: usize) -> i32 {
        self.heaps[heap].transient
    }

    /// `lbHeap_80015BB8`.
    #[doc(alias = "lbHeap_80015BB8")]
    pub fn status(&self, heap: usize) -> i32 {
        self.heaps[heap].status
    }

    /// `lbHeap_80015900`, a scene's heaps: the transient preload heaps destroyed, HSD's main
    /// heap made anew over the arena the kept ones leave, the ARAM heap made anew over ARAM
    /// the same way, and the kept preload heaps made if they aren't. The main heap's
    /// allocator model starts over from the layout it returns (`HSD_CreateMainHeap`); the
    /// caller keeps the model's fighter size and class sizes.
    #[doc(alias = "lbHeap_80015900")]
    pub fn make_scene_heaps(&mut self, mem: &mut LbMemory) -> Layout {
        for i in SEQ..COUNT {
            if self.heaps[i].transient == 1 {
                self.destroy(i, mem);
            }
        }
        let (mut lo, mut hi) = (self.arena_lo, self.arena_hi);
        let (mut aram_lo, aram_hi) = (self.aram_lo, self.aram_hi);
        for h in &self.heaps[SEQ..] {
            if h.transient == 0 {
                match h.kind {
                    TYPE_LOW => lo = lo.max(h.start + h.size),
                    TYPE_HIGH => hi = hi.min(h.start),
                    TYPE_ARAM => aram_lo = aram_lo.max(h.start + h.size),
                    _ => {}
                }
            }
        }
        let main = &mut self.heaps[HSD];
        // `OSCreateHeap`'s index: the audio heap is the first, the main heap the second.
        main.id = 1;
        main.start = lo;
        main.size = hi - lo;
        main.status = CREATED;
        main.kind = TYPE_OS;
        mem.destroy_aram();
        let aram = &mut self.heaps[ARAM];
        aram.handle = Some(mem.create_aram(aram_lo, aram_hi));
        aram.start = aram_lo;
        aram.size = aram_hi - aram_lo;
        aram.status = CREATED;
        aram.kind = TYPE_ARAM_HEAP;
        for i in SEQ..COUNT {
            if self.heaps[i].transient == 0 {
                self.create(i, mem);
            }
        }
        Layout {
            lo,
            hi,
            index: 1,
            fighter_size: crate::hsd::heap::FIGHTER_SIZE,
            from_menus: false,
        }
    }

    /// `destroyHeap`.
    fn destroy(&mut self, i: usize, mem: &mut LbMemory) {
        let h = &mut self.heaps[i];
        if h.status == CREATED {
            assert!(h.kind != TYPE_OS, "an OS preload heap");
            mem.destroy(h.handle.expect("a made heap's handle"));
            h.handle = None;
            h.status = DESTROYED;
        }
    }

    /// `createHeap`.
    fn create(&mut self, i: usize, mem: &mut LbMemory) {
        let h = &mut self.heaps[i];
        if h.status == DESTROYED {
            assert!(h.kind != TYPE_OS, "an OS preload heap");
            h.handle = Some(mem.create(h.start, h.start + h.size));
            h.status = CREATED;
        }
    }

    /// `lbHeap_80015BD0` for a preload heap: a block of `size` bytes, its entry, or none when
    /// the heap isn't made. HSD's main heap's blocks are the allocator model's.
    #[doc(alias = "lbHeap_80015BD0")]
    pub fn alloc(&self, heap: usize, size: u32, mem: &mut LbMemory) -> Option<u8> {
        let h = &self.heaps[heap];
        if h.status != CREATED {
            return None;
        }
        assert!(h.kind != TYPE_OS, "a preload block from HSD's heap");
        Some(mem.alloc(h.handle.expect("a made heap's handle"), size))
    }

    /// `lbHeap_80015CA8` for a preload heap.
    // Ports `lbHeap_80015CA8`; aliased once a check reaches it.
    pub fn free(&self, heap: usize, addr: u32, mem: &mut LbMemory) {
        let h = &self.heaps[heap];
        assert!(h.status == CREATED, "p->status == LbHeapStatus_Create");
        assert!(h.kind != TYPE_OS, "a preload block from HSD's heap");
        mem.free(h.handle.expect("a made heap's handle"), addr);
    }

    /// `lbHeap_80015D6C`: whether a compaction of the heap is under way, which
    /// `lbMemory_8001529C` starts when a gap opened in it. The first two heaps never compact.
    #[doc(alias = "lbHeap_80015D6C")]
    pub fn compact(&self, heap: usize, mem: &LbMemory) -> bool {
        if heap <= ARAM {
            return false;
        }
        let h = self.heaps[heap].handle.expect("a made heap's handle");
        assert!(
            !mem.needs_compaction(h),
            "a preload heap's compaction (lbMemory_80015320) isn't ported"
        );
        false
    }
}
