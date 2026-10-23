//! The game's own heaps over ranges of the arena or ARAM (`melee/lb/lbmemory.c`): each a
//! handle with a range and an address-ordered list of blocks, taken from a fixed pool of
//! block entries, best fit. The preload cache keeps its files in them (`lbheap`'s `Seq`,
//! `Stay`, `AllM` and `AllA` heaps, and the ARAM heap).
//!
//! The original links entries and handles by pointer into its static block
//! (`lbMemory_804318B0`); melee-sim links them by index, and [`entry_addr`] and
//! [`handle_addr`] give the addresses those pointers have.

/// Block entries in the pool (`mem`).
pub const ENTRIES: usize = 0x83;
/// Handles in the pool (`heap`).
pub const HANDLES: usize = 6;

/// `lbMemory_804318B0`'s address, and where its entries and handles start in it.
pub const BASE: u32 = 0x8043_18B0;
const ENTRIES_AT: u32 = BASE + 8;
const ENTRY_SIZE: u32 = 0xC;
const HANDLES_AT: u32 = ENTRIES_AT + ENTRY_SIZE * ENTRIES as u32 + 0xC;
const HANDLE_SIZE: u32 = 0x10;

/// The address of block entry `i` (an `HSD_AllocEntry*`).
pub fn entry_addr(i: u8) -> u32 {
    ENTRIES_AT + ENTRY_SIZE * u32::from(i)
}

/// The address of handle `i` (a `Handle*`).
pub fn handle_addr(i: u8) -> u32 {
    HANDLES_AT + HANDLE_SIZE * u32::from(i)
}

/// `HSD_AllocEntry`: a block of a heap, or a free entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct AllocEntry {
    pub next: Option<u8>,
    pub addr: u32,
    pub size: u32,
}

/// `Handle`: a heap over `[lo, hi)`, its blocks in address order, or a free handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Handle {
    pub next: Option<u8>,
    pub lo: u32,
    pub hi: u32,
    pub blocks: Option<u8>,
}

/// `lbMemory_804318B0` (`struct Allocator`), but for its copy manager's alarm, which only a
/// heap's compaction sets (`lbMemory_8001529C`), and the compaction's callback state.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct LbMemory {
    /// ARAM's range the game's heaps may use (`a_arenaLo`, `a_arenaHi`).
    pub aram_lo: u32,
    pub aram_hi: u32,
    pub entries: [AllocEntry; ENTRIES],
    pub free_mem: Option<u8>,
    pub num_allocs: i32,
    pub max_num_allocs: i32,
    pub heaps: [Handle; HANDLES],
    pub free_heap: Option<u8>,
    pub aram_heap: Option<u8>,
    /// The copy manager's bytes left to copy (`mgr.size`): 0 when idle.
    pub copy_size: u32,
}

impl Default for LbMemory {
    fn default() -> Self {
        LbMemory {
            aram_lo: 0,
            aram_hi: 0,
            entries: [AllocEntry::default(); ENTRIES],
            free_mem: None,
            num_allocs: 0,
            max_num_allocs: 0,
            heaps: [Handle::default(); HANDLES],
            free_heap: None,
            aram_heap: None,
            copy_size: 0,
        }
    }
}

/// ARAM's size on a console (`ARGetSize`): 16 MB.
const ARAM_SIZE: u32 = 0x100_0000;

impl LbMemory {
    /// `lbMemory_8001564C`, at boot: ARAM from where its allocator stands after the audio
    /// library's (`aram_next`, an `ARAlloc` given straight back) to its end, every entry and
    /// handle free, and the ARAM heap made.
    #[doc(alias = "lbMemory_8001564C")]
    pub fn init(&mut self, aram_next: u32) {
        self.aram_lo = aram_next;
        self.aram_hi = ARAM_SIZE.min(0x100_0000);
        for i in 0..ENTRIES {
            self.entries[i].next = (i + 1 < ENTRIES).then_some(i as u8 + 1);
        }
        self.free_mem = Some(0);
        self.max_num_allocs = 0;
        self.num_allocs = 0;
        for i in 0..HANDLES {
            self.heaps[i].next = (i + 1 < HANDLES).then_some(i as u8 + 1);
        }
        self.free_heap = Some(0);
        self.aram_heap = None;
        self.aram_heap = Some(self.create(self.aram_lo, self.aram_hi));
        self.copy_size = 0;
    }

    /// `lbMemory_80014E24`: a free handle over `[lo, hi)`, with no blocks.
    #[doc(alias = "lbMemory_80014E24")]
    pub fn create(&mut self, lo: u32, hi: u32) -> u8 {
        let h = self.free_heap.expect("lbMemory: no free heap handle");
        self.free_heap = self.heaps[usize::from(h)].next;
        self.heaps[usize::from(h)] = Handle {
            next: None,
            lo,
            hi,
            blocks: None,
        };
        h
    }

    /// `lbMemory_80014EEC`: the heap's blocks and its handle back to their free lists, the
    /// blocks in order, each ahead of the last.
    #[doc(alias = "lbMemory_80014EEC")]
    pub fn destroy(&mut self, h: u8) {
        let mut b = self.heaps[usize::from(h)].blocks;
        while let Some(i) = b {
            b = self.entries[usize::from(i)].next;
            self.entries[usize::from(i)].next = self.free_mem;
            self.free_mem = Some(i);
            self.num_allocs -= 1;
        }
        self.heaps[usize::from(h)].next = self.free_heap;
        self.free_heap = Some(h);
    }

    /// `lbMemory_80014FC8`: a block of `size` bytes (rounded up to 32) in the gap that
    /// leaves the least over, the last such gap; the block's entry.
    #[doc(alias = "lbMemory_80014FC8")]
    pub fn alloc(&mut self, h: u8, size: u32) -> u8 {
        let size = size.next_multiple_of(32);
        let heap = self.heaps[usize::from(h)];
        let mut least = 0x4000_0000u32;
        let mut start = heap.lo;
        // Where the block goes: its address, and the link (the heap's head, or the entry
        // whose `next` it takes) before it.
        let mut choice: Option<(u32, Option<u8>)> = None;
        let mut prev: Option<u8> = None;
        let mut link = heap.blocks;
        loop {
            let end = link.map_or(heap.hi, |i| self.entries[usize::from(i)].addr);
            let space = end.wrapping_sub(start);
            if space >= size {
                let left = space - size;
                if left <= least {
                    least = left;
                    choice = Some((start, prev));
                }
            }
            let Some(i) = link else { break };
            let e = self.entries[usize::from(i)];
            start = e.addr.wrapping_add(e.size);
            prev = Some(i);
            link = e.next;
        }
        let (lo, after) = choice.expect("lbMemory: no room in the heap");
        let r = self.free_mem.expect("lbMemory: no free block entry");
        self.free_mem = self.entries[usize::from(r)].next;
        let next = match after {
            None => self.heaps[usize::from(h)].blocks,
            Some(p) => self.entries[usize::from(p)].next,
        };
        self.entries[usize::from(r)] = AllocEntry {
            next,
            addr: lo,
            size,
        };
        match after {
            None => self.heaps[usize::from(h)].blocks = Some(r),
            Some(p) => self.entries[usize::from(p)].next = Some(r),
        }
        self.num_allocs += 1;
        self.max_num_allocs = self.max_num_allocs.max(self.num_allocs);
        r
    }

    /// `lbMemFreeToHeap`: the block at `addr` back on the free list.
    #[doc(alias = "lbMemFreeToHeap")]
    pub fn free(&mut self, h: u8, addr: u32) {
        let mut prev: Option<u8> = None;
        let mut b = self.heaps[usize::from(h)].blocks;
        while let Some(i) = b {
            let e = self.entries[usize::from(i)];
            if e.addr == addr {
                match prev {
                    None => self.heaps[usize::from(h)].blocks = e.next,
                    Some(p) => self.entries[usize::from(p)].next = e.next,
                }
                self.entries[usize::from(i)].next = self.free_mem;
                self.free_mem = Some(i);
                self.num_allocs -= 1;
                return;
            }
            prev = b;
            b = e.next;
        }
        panic!("[LbMem] Error: lbMemFreeToHeap {addr:x}.");
    }

    /// `lbMemory_8001529C`'s first part: whether the heap has a gap before a block, which a
    /// compaction would close by moving blocks down with alarms and ARAM copies. The boot's
    /// heaps never have one; a compaction that would start stops.
    #[doc(alias = "lbMemory_8001529C")]
    pub fn needs_compaction(&self, h: u8) -> bool {
        let heap = self.heaps[usize::from(h)];
        let mut cursor = heap.lo;
        let mut b = heap.blocks;
        while let Some(i) = b {
            let e = self.entries[usize::from(i)];
            if e.addr != cursor {
                return true;
            }
            cursor = e.addr + e.size;
            b = e.next;
        }
        false
    }

    /// `lbMemory_800154D4`: the ARAM heap over `[lo, hi)`.
    #[doc(alias = "lbMemory_800154D4")]
    pub fn create_aram(&mut self, lo: u32, hi: u32) -> u8 {
        let h = self.create(lo, hi);
        self.aram_heap = Some(h);
        h
    }

    /// `lbMemory_800155A4`: the ARAM heap destroyed.
    #[doc(alias = "lbMemory_800155A4")]
    pub fn destroy_aram(&mut self) {
        let h = self.aram_heap.take().expect("the ARAM heap");
        self.destroy(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pools_sit_where_the_original_keeps_them() {
        // The tick 0 of every recorded session: entry 0's next is entry 1, handle 1 is the
        // `Seq` heap's.
        assert_eq!(entry_addr(1), 0x8043_18C4);
        assert_eq!(handle_addr(0), 0x8043_1EE8);
        assert_eq!(handle_addr(2), 0x8043_1F08);
    }

    #[test]
    fn a_block_takes_the_tightest_gap() {
        let mut m = LbMemory::default();
        m.init(0x1000);
        let h = m.create(0, 0x1000);
        let a = m.alloc(h, 0x100);
        let b = m.alloc(h, 0x100);
        let c = m.alloc(h, 0x200);
        m.free(h, m.entries[usize::from(b)].addr);
        // The gap between a and c fits 0x100 exactly: it wins over the end.
        let d = m.alloc(h, 0xE0);
        assert_eq!(m.entries[usize::from(d)].addr, 0x100);
        assert!(!m.needs_compaction(h));
        let _ = (a, c);
    }
}
