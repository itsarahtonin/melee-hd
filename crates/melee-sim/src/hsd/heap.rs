//! The original's allocators, as plain data: where each object the game makes would live.
//!
//! melee-sim keeps objects in per-type pools ([`super::pool`]); this model computes, beside
//! them, the addresses the original's allocators hand out, for objects whose address reaches
//! gameplay and for the checks against ssbm-rs's memory. Three layers, as in the original:
//!
//! - the scene's main heap, a Dolphin OS heap (`OSAllocFromHeap`, `OSFreeToHeap`): first fit
//!   over an address-ordered free list, splitting a block when what is left is 64 bytes or
//!   more, coalescing with both neighbours when a block comes back ([`OsHeap`]);
//! - HAL's memory pieces (`hsdAllocMemPiece`, `hsdFreeMemPiece`), which the HSD classes
//!   (joints, display objects, materials, textures, shapes, cameras, lights) allocate from:
//!   size classes of 32 bytes shared by every class, each a last-in-first-out free list,
//!   split from a larger class's free piece or from a new block of the main heap
//!   ([`MemPieces`]);
//! - `HSD_ObjAlloc`'s per-type pools (GObjs, processes, animations, fighters, items,
//!   effects, particles...), each a last-in-first-out free list grown one object at a time
//!   from the main heap ([`ObjAlloc`]).
//!
//! The layout differs per ruleset: Slippi's codes make heaps of their own before the main
//! heap and lengthen `Fighter`, so the main heap starts higher and holds other sizes; a state
//! taken from memory carries whichever it had.

use std::collections::BTreeMap;

/// An OS heap's header before each block (`Cell`, padded to 32 bytes).
pub const HEADER: u32 = 0x20;
/// The smallest leftover `OSAllocFromHeap` splits off (`MINOBJSIZE`).
pub const MIN_SPLIT: u32 = 64;

/// A Dolphin OS heap: its free blocks by address and its allocated blocks, each with its size
/// (header included), as the cells in memory hold them. Allocated blocks are a set: the
/// original's allocated list keeps them newest first, an order only `OSCheckHeap`'s walk sees.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OsHeap {
    /// The heap's index in `HeapArray`.
    pub index: i32,
    pub size: u32,
    pub free: BTreeMap<u32, u32>,
    pub allocated: BTreeMap<u32, u32>,
}

impl OsHeap {
    /// A heap over `[start, end)`, rounded in to 32 bytes, `OSCreateHeap`.
    pub fn new(index: i32, start: u32, end: u32) -> OsHeap {
        let start = (start + 31) & !31;
        let end = end & !31;
        let mut free = BTreeMap::new();
        free.insert(start, end - start);
        OsHeap {
            index,
            size: end - start,
            free,
            allocated: BTreeMap::new(),
        }
    }

    /// `OSAllocFromHeap`: the first free block that fits, split when 64 bytes or more are
    /// left; the address after its header, or `None` when nothing fits.
    pub fn alloc(&mut self, size: u32) -> Option<u32> {
        let size = (size.wrapping_add(HEADER) + 31) & !31;
        let (&cell, &have) = self
            .free
            .iter()
            .find(|&(_, &have)| size as i32 <= have as i32)?;
        self.free.remove(&cell);
        let left = have.wrapping_sub(size);
        let size = if left < MIN_SPLIT {
            have
        } else {
            self.free.insert(cell + size, left);
            size
        };
        self.allocated.insert(cell, size);
        Some(cell + HEADER)
    }

    /// `OSFreeToHeap`: the block back on the free list, merged with the blocks either side
    /// when they touch.
    pub fn free(&mut self, ptr: u32) {
        let mut cell = ptr - HEADER;
        let mut size = self
            .allocated
            .remove(&cell)
            .unwrap_or_else(|| panic!("freeing {ptr:#x}, which the heap didn't hand out"));
        if let Some(next) = self.free.remove(&(cell + size)) {
            size += next;
        }
        if let Some((&prev, &prev_size)) = self.free.range(..cell).next_back()
            && prev + prev_size == cell
        {
            cell = prev;
            size += prev_size;
        }
        self.free.insert(cell, size);
    }

    /// The free bytes, headers included, as `OSCheckHeap` counts them.
    pub fn free_bytes(&self) -> u32 {
        self.free.values().sum()
    }
}

/// One size class of HAL's memory pieces (`HSD_MemoryEntry`): its pieces' size, how many
/// pieces it has made and how many are free, its free list (last in, first out: the list's
/// head is the vector's end), the next larger class that exists, and where the entry itself
/// lives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemEntry {
    pub addr: u32,
    pub size: u32,
    pub nb_alloc: u32,
    pub nb_free: u32,
    pub free: Vec<u32>,
    pub next: Option<u32>,
}

/// HAL's memory pieces (`class.c`'s `memory_list`): the size classes by index (pieces of
/// `32 * (index + 1)` bytes) and where the array of them lives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemPieces {
    /// `memory_list`'s address, 0 before the first piece.
    pub array: u32,
    /// The entries, `nb_memory_list` of them.
    pub list: Vec<Option<MemEntry>>,
}

/// `HSD_ObjAllocData`: one object type's pool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObjAlloc {
    pub num_limit_flag: bool,
    pub heap_limit_flag: bool,
    /// The free objects, last in first out: the list's head (`freehead`) is the vector's end.
    pub free: Vec<u32>,
    pub used: u32,
    pub peak: u32,
    pub num_limit: u32,
    pub heap_limit_size: u32,
    pub heap_limit_num: u32,
    pub size: u32,
    /// The alignment less one, as the original keeps it.
    pub align: u32,
}

/// `obj_heap`: a range objects are carved from in order instead of the main heap. Melee
/// gives it no range (`top` 0), so pools grow from the main heap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjHeap {
    pub top: u32,
    pub curr: u32,
    pub remain: u32,
    pub size: u32,
}

/// The pools `HSD_ObjAlloc` serves that the model follows, by their allocation data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    GObj,
    GObjProc,
    AObj,
    FObj,
    RObj,
    Rvalue,
    IdEntry,
    SList,
    DList,
    Fighter,
    FighterAttrs,
    FighterParts,
    FighterDObjList,
    FighterX2040,
    FighterX59C,
    Player,
    Item,
    ItemDynamicBones,
    ItemLink,
    Effect,
    EffectQueued,
    Particle,
    Generator,
    AppSrt,
    SObj,
    BgFlash,
    AudioUserData,
    DebugMenu,
    Shadow,
    ZList,
    Vec,
    Mtx,
    Render,
    TevReg,
    Chan,
}

impl Kind {
    pub const ALL: [Kind; 35] = [
        Kind::GObj,
        Kind::GObjProc,
        Kind::AObj,
        Kind::FObj,
        Kind::RObj,
        Kind::Rvalue,
        Kind::IdEntry,
        Kind::SList,
        Kind::DList,
        Kind::Fighter,
        Kind::FighterAttrs,
        Kind::FighterParts,
        Kind::FighterDObjList,
        Kind::FighterX2040,
        Kind::FighterX59C,
        Kind::Player,
        Kind::Item,
        Kind::ItemDynamicBones,
        Kind::ItemLink,
        Kind::Effect,
        Kind::EffectQueued,
        Kind::Particle,
        Kind::Generator,
        Kind::AppSrt,
        Kind::SObj,
        Kind::BgFlash,
        Kind::AudioUserData,
        Kind::DebugMenu,
        Kind::Shadow,
        Kind::ZList,
        Kind::Vec,
        Kind::Mtx,
        Kind::Render,
        Kind::TevReg,
        Kind::Chan,
    ];
}

/// The HSD classes, whose objects are memory pieces of their class's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    JObj,
    DObj,
    MObj,
    TObj,
    PObj,
    CObj,
    LObj,
    WObj,
    Fog,
}

impl Class {
    pub const ALL: [Class; 9] = [
        Class::JObj,
        Class::DObj,
        Class::MObj,
        Class::TObj,
        Class::PObj,
        Class::CObj,
        Class::LObj,
        Class::WObj,
        Class::Fog,
    ];
}

/// A class's size and counts (`HSD_ClassInfoHead`'s `obj_size`, `nb_exist`, `nb_peak`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassInfo {
    pub obj_size: u32,
    pub nb_exist: u32,
    pub nb_peak: u32,
}

/// The allocators of the scene's main heap. Until a state brings them in (`active`), nothing
/// is modelled and objects' addresses are unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Heap {
    pub active: bool,
    pub os: OsHeap,
    pub pieces: MemPieces,
    pub obj_heap: ObjHeap,
    pub allocs: BTreeMap<Kind, ObjAlloc>,
    pub classes: BTreeMap<Class, ClassInfo>,
}

/// `HSD_MemoryEntry`'s size.
const ENTRY_SIZE: u32 = 0x14;

impl Heap {
    /// `HSD_MemAlloc`: a block of the main heap. The original asserts it got one.
    pub fn mem_alloc(&mut self, size: u32) -> u32 {
        assert!(size as i32 > 0, "HSD_MemAlloc of {size:#x} bytes");
        self.os
            .alloc(size)
            .unwrap_or_else(|| panic!("the main heap has no {size:#x} bytes free"))
    }

    /// `HSD_Free`.
    pub fn mem_free(&mut self, ptr: u32) {
        self.os.free(ptr);
    }

    fn entry(&mut self, idx: usize) -> &mut MemEntry {
        self.pieces.list[idx].as_mut().expect("a memory entry")
    }

    /// `GetMemoryEntry`: the size class `idx`, made (and the array grown) if missing.
    pub fn memory_entry(&mut self, idx: usize) -> usize {
        let nb = self.pieces.list.len();
        if idx >= nb {
            if nb == 0 {
                let mut new_nb = 32;
                while idx >= new_nb {
                    new_nb *= 2;
                }
                self.pieces.array = self.mem_alloc(new_nb as u32 * 4);
                self.pieces.list = vec![None; new_nb];
            } else {
                let mut new_nb = nb * 2;
                while idx >= new_nb {
                    new_nb *= 2;
                }
                let new_array = self.mem_alloc(4 * new_nb as u32);
                let old = self.pieces.array;
                let old_nb = (nb as u32 * 4) & !31;
                self.pieces.array = new_array;
                self.pieces.list.resize(new_nb, None);
                self.piece_free(old, old_nb);
                let i = (((old_nb + 31) & !31) / 32 - 1) as usize;
                self.entry(i).nb_alloc += 1;
            }
        }
        if self.pieces.list[idx].is_none() {
            let addr = self.mem_alloc(ENTRY_SIZE);
            let mut e = MemEntry {
                addr,
                size: (idx as u32 + 1) * 32,
                ..MemEntry::default()
            };
            let list = &mut self.pieces.list;
            if let Some(i) = (0..idx).rev().find(|&i| list[i].is_some()) {
                let below = list[i].as_mut().unwrap();
                e.next = below.next;
                below.next = Some(idx as u32);
            } else if let Some(i) = (idx + 1..list.len()).find(|&i| list[i].is_some()) {
                e.next = Some(i as u32);
            }
            list[idx] = Some(e);
        }
        idx
    }

    /// `hsdAllocMemPiece`: a piece of `size` bytes rounded up to 32: its class's last freed
    /// piece, else one split from the first larger class with a free piece (the rest going
    /// to the class of its size), else a new block of the main heap split the same way.
    pub fn piece_alloc(&mut self, size: u32) -> u32 {
        let idx = ((size as i32 + 0x1F) / 32 - 1) as usize;
        let e = self.memory_entry(idx);
        if let Some(p) = self.entry(e).free.pop() {
            self.entry(e).nb_free -= 1;
            return p;
        }
        let esize = self.entry(e).size;
        let mut var = self.entry(e).next;
        while let Some(v) = var {
            let v = v as usize;
            if !self.entry(v).free.is_empty() {
                let vsize = self.entry(v).size;
                let e2 = self.memory_entry(((vsize - esize + 0x1F) as i32 / 32 - 1) as usize);
                let ve = self.entry(v);
                let p = ve.free.pop().unwrap();
                ve.nb_free -= 1;
                ve.nb_alloc -= 1;
                let rest = self.entry(e2);
                rest.free.push(p + esize);
                rest.nb_alloc += 1;
                rest.nb_free += 1;
                self.entry(e).nb_alloc += 1;
                return p;
            }
            var = self.entry(v).next;
        }
        let nb = self.pieces.list.len() as i32;
        let t = nb - idx as i32 - 2;
        let rest = (t >= 0).then(|| self.memory_entry(t as usize));
        let p = self.mem_alloc(nb as u32 * 32);
        if let Some(r) = rest {
            let r = self.entry(r);
            r.free.push(p + esize);
            r.nb_alloc += 1;
            r.nb_free += 1;
        }
        self.entry(e).nb_alloc += 1;
        p
    }

    /// `hsdFreeMemPiece`: the piece back on its class's free list.
    pub fn piece_free(&mut self, ptr: u32, size: u32) {
        if ptr == 0 {
            return;
        }
        let e = self.memory_entry(((size as i32 + 31) / 32 - 1) as usize);
        let e = self.entry(e);
        e.free.push(ptr);
        e.nb_free += 1;
    }

    /// `_hsdClassAlloc` for a class's object: a piece of its size, counted.
    pub fn class_alloc(&mut self, class: Class) -> u32 {
        let size = self.classes[&class].obj_size;
        let p = self.piece_alloc(size);
        let c = self.classes.get_mut(&class).unwrap();
        c.nb_exist += 1;
        c.nb_peak = c.nb_peak.max(c.nb_exist);
        p
    }

    /// `_hsdClassDestroy` and `hsdFreeMemPiece` for a class's object.
    pub fn class_free(&mut self, class: Class, ptr: u32) {
        let c = self.classes.get_mut(&class).unwrap();
        c.nb_exist -= 1;
        let size = c.obj_size;
        self.piece_free(ptr, size);
    }

    /// `HSD_ObjAllocAddFree`: `num` new objects for a pool, carved from `obj_heap` if it has
    /// a range, else one block of the main heap; how many it added.
    pub fn obj_add_free(&mut self, kind: Kind, num: u32) -> u32 {
        let (size, align) = {
            let d = &self.allocs[&kind];
            (d.size, d.align)
        };
        let mut pool_size = size.wrapping_mul(num);
        let mut num = num;
        let start;
        if self.obj_heap.top != 0 {
            let end = self.obj_heap.top.wrapping_add(self.obj_heap.size);
            start = self.obj_heap.curr.wrapping_add(align) & !align;
            if start > end {
                return 0;
            }
            if end - start < pool_size {
                pool_size = (end - start) - (end - start) % size;
            }
            num = pool_size / size;
            if num == 0 {
                return 0;
            }
            self.obj_heap.curr = start + pool_size;
            self.obj_heap.remain = end - self.obj_heap.curr;
        } else {
            start = self.mem_alloc(pool_size);
            self.obj_heap.remain = self.obj_heap.remain.wrapping_sub(pool_size);
        }
        let d = self.allocs.get_mut(&kind).unwrap();
        // The new objects link in address order ahead of the old free list.
        d.free.extend((0..num).rev().map(|i| start + size * i));
        num
    }

    /// `HSD_ObjAlloc`: the pool's last freed object, growing the pool by one when none is
    /// free. Pools with limits aren't modelled (none of Melee's has one set).
    pub fn obj_alloc(&mut self, kind: Kind) -> u32 {
        let d = &self.allocs[&kind];
        assert!(
            !d.num_limit_flag && !d.heap_limit_flag,
            "{kind:?}'s pool has limits, which the model doesn't follow"
        );
        if d.free.is_empty() {
            let added = self.obj_add_free(kind, 1);
            assert!(added != 0, "{kind:?}'s pool couldn't grow");
        }
        let d = self.allocs.get_mut(&kind).unwrap();
        let p = d.free.pop().unwrap();
        d.used += 1;
        d.peak = d.peak.max(d.used);
        p
    }

    /// `HSD_ObjFree`.
    pub fn obj_free(&mut self, kind: Kind, ptr: u32) {
        let d = self.allocs.get_mut(&kind).unwrap();
        d.free.push(ptr);
        d.used -= 1;
    }
}
